//! Headless observation of a single panel pass. Never hold the SDK lock across a
//! plugin callback: it may destroy another window, or invalidate its own screen.
use crate::state::{fault, handle, state};
use serde_json::{Value, json};
use std::collections::HashMap;
use xphost_native::guard::{xp_draw_builtin, xp_draw_screen, xp_draw_window};
use xplane_sdk_sys::*;

pub enum Screen {
    Custom {
        draw: XPLMAvionicsScreenCallback_f,
        on_demand: bool,
        dirty: bool,
    },
    Builtin {
        device: i32,
        before: XPLMAvionicsCallback_f,
        after: XPLMAvionicsCallback_f,
    },
}
pub struct Avionics {
    pub screen: Screen,
    pub refcon: usize,
    pub name: String,
}
pub struct Instance {
    pub object: String,
    pub datarefs: Vec<String>,
    pub pose: Option<[f32; 6]>,
    pub values: Vec<f32>,
}
impl Instance {
    fn json(&self) -> Value {
        json!({"object":self.object,"datarefs":self.datarefs,"pose":self.pose,"values":self.values})
    }
}

#[derive(Clone, Copy)]
pub struct Transform {
    pub matrix: [f32; 6],
    pub rotated: bool,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            matrix: [1., 0., 0., 1., 0., 0.],
            rotated: false,
        }
    }
}
impl Transform {
    pub fn point(self, [x, y]: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d, e, f] = self.matrix;
        [a * x + c * y + e, b * x + d * y + f]
    }
    pub fn rect(self, r: [f32; 4]) -> [f32; 4] {
        let [x, y] = self.point([r[0], r[1]]);
        let [u, v] = self.point([r[2], r[3]]);
        [x.min(u), y.min(v), x.max(u), y.max(v)]
    }
    pub fn translate(&mut self, x: f32, y: f32) {
        let p = self.point([x, y]);
        self.matrix[4..].copy_from_slice(&p);
    }
    pub fn scale(&mut self, x: f32, y: f32) {
        self.matrix[0] *= x;
        self.matrix[1] *= x;
        self.matrix[2] *= y;
        self.matrix[3] *= y;
    }
    pub fn rotate(&mut self, angle: f32) {
        let (s, c) = angle.to_radians().sin_cos();
        let [a, b, d, e, _, _] = self.matrix;
        self.matrix[..4].copy_from_slice(&[
            a * c + d * s,
            b * c + e * s,
            -a * s + d * c,
            -b * s + e * c,
        ]);
        self.rotated = true;
    }
}
pub fn intersection(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}
#[derive(Default)]
pub struct Drawing {
    pub transform: Transform,
    pub transforms: Vec<Transform>,
    pub clip: Option<[f32; 4]>,
    pub scissors: Vec<Option<[f32; 4]>>,
    pub commands: Vec<Value>,
}
impl Drawing {
    pub fn record(&mut self, op: &str, args: Value) {
        self.commands
            .push(json!({"op":op,"args":args,"transform":self.transform.matrix,"clip":self.clip}));
    }
}
#[derive(Default)]
pub struct Panel {
    pub textures: HashMap<usize, Value>,
    pub active: Option<Drawing>,
    pub errors: Vec<String>,
}
impl Panel {
    pub fn draw(&mut self, op: &str, call: impl FnOnce(&mut Drawing) -> Result<(), &'static str>) {
        match self
            .active
            .as_mut()
            .ok_or("outside a native panel draw callback")
            .and_then(call)
        {
            Ok(()) => (),
            Err(error) => self.error(op, error),
        }
    }
    pub fn error(&mut self, op: &str, error: &str) {
        if self.errors.len() < 32 {
            self.errors.push(format!("{op}: {error}"));
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    Window(usize),
    Screen(usize),
    Before(usize),
    After(usize),
}
enum Callback {
    Window(
        unsafe extern "C" fn(XPLMWindowID, *mut std::ffi::c_void),
        usize,
    ),
    Screen(unsafe extern "C" fn(*mut std::ffi::c_void)),
    Builtin(
        unsafe extern "C" fn(i32, i32, *mut std::ffi::c_void) -> i32,
        i32,
        bool,
    ),
}
fn dispatch(target: Target) -> Option<(Value, Callback, usize)> {
    let mut s = state();
    if let Target::Window(id) = target {
        let w = s.windows.get(&id)?;
        if !w.visible {
            return None;
        }
        return Some((
            json!({"window":id,"title":w.title,"bounds":[w.bounds.left,w.bounds.top,w.bounds.right,w.bounds.bottom]}),
            Callback::Window(w.draw?, id),
            w.refcon,
        ));
    }
    let (id, phase) = match target {
        Target::Screen(id) => (id, "screen"),
        Target::Before(id) => (id, "before"),
        Target::After(id) => (id, "after"),
        Target::Window(_) => unreachable!(),
    };
    let a = s.avionics.get_mut(&id)?;
    let callback = match &mut a.screen {
        Screen::Custom {
            draw,
            on_demand,
            dirty,
        } => {
            if *on_demand && !*dirty {
                return None;
            }
            *dirty = false;
            Callback::Screen((*draw)?)
        }
        Screen::Builtin {
            device,
            before,
            after,
        } => {
            let first = matches!(target, Target::Before(_));
            Callback::Builtin(if first { *before } else { *after }?, *device, first)
        }
    };
    Some((
        json!({"avionics":id,"device":a.name,"phase":phase}),
        callback,
        a.refcon,
    ))
}
pub fn capture() -> Result<Value, String> {
    let targets = {
        let s = state();
        if !s.enabled || s.faulted {
            return Err("plugin unavailable for panel drawing".into());
        }
        let mut targets = Vec::new();
        for (&id, a) in &s.avionics {
            match &a.screen {
                Screen::Custom { .. } => targets.push(Target::Screen(id)),
                Screen::Builtin { .. } => targets.extend([Target::Before(id), Target::After(id)]),
            }
        }
        let mut windows: Vec<_> = s
            .windows
            .iter()
            .filter(|(_, w)| w.content_type == xplm_WindowContentTypePanelGraphics && w.visible)
            .map(|(&id, w)| (w.layer, w.order, id))
            .collect();
        windows.sort_unstable();
        targets.extend(windows.into_iter().map(|(_, _, id)| Target::Window(id)));
        targets
    };
    let mut surfaces = Vec::new();
    for target in targets {
        let Some((mut surface, callback, refcon)) = dispatch(target) else {
            continue;
        };
        state().panel.active = Some(Drawing::default());
        // SAFETY: each registration is still live and belongs to the loaded
        // plugin. Its SDK ABI and refcon are preserved across the SEH boundary.
        let code = unsafe {
            match callback {
                Callback::Window(f, id) => xp_draw_window(f, handle(id), refcon as _),
                Callback::Screen(f) => xp_draw_screen(f, refcon as _),
                Callback::Builtin(f, device, before) => {
                    xp_draw_builtin(f, device, i32::from(before), refcon as _)
                }
            }
        };
        fault(code);
        let mut s = state();
        let draw = s.panel.active.take().expect("draw scope");
        if code != 0 {
            return Err("plugin fault during panel drawing".into());
        }
        if !draw.transforms.is_empty() || !draw.scissors.is_empty() {
            s.panel
                .error("end callback", "unbalanced transform or scissor stack");
        }
        surface["commands"] = Value::Array(draw.commands);
        surfaces.push(surface);
    }
    let mut s = state();
    let instances: Vec<_> = s.instances.values().map(Instance::json).collect();
    Ok(
        json!({"frame":s.frame,"surfaces":surfaces,"instances":instances,"errors":std::mem::take(&mut s.panel.errors)}),
    )
}
