use crate::{Geometry, RenderTransform, transform_primitives};
use egui::{
    Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, vec2,
};
use egui_glow::{Painter, glow};
use glow::HasContext;
use std::{
    ffi::{CString, c_void},
    mem, ptr,
    sync::{Arc, OnceLock},
    thread::{self, ThreadId},
    time::Instant,
};
use windows_sys::Win32::{
    Graphics::OpenGL::{wglGetCurrentContext, wglGetProcAddress},
    System::LibraryLoader::{GetProcAddress, LoadLibraryA},
};
use xplane_plugin::{DataRefCache, set_2d_graphics_state};
use xplane_sdk_sys::*;

pub struct Surface {
    pub context: egui::Context,
    painter: Option<Painter>,
    events: Vec<Event>,
    modifiers: Modifiers,
    pointer: Pos2,
    captured: [bool; 2],
    focused: bool,
    start: Instant,
    owner: ThreadId,
    datarefs: DataRefCache,
    textures: egui::TexturesDelta,
}
impl Default for Surface {
    fn default() -> Self {
        Self::new()
    }
}
impl Surface {
    pub fn new() -> Self {
        Self {
            context: egui::Context::default(),
            painter: None,
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer: Pos2::ZERO,
            captured: [false; 2],
            focused: false,
            start: Instant::now(),
            owner: thread::current().id(),
            datarefs: DataRefCache::default(),
            textures: Default::default(),
        }
    }
    pub fn mouse(&mut self, x: f32, y: f32, phase: i32, button: PointerButton) {
        if thread::current().id() != self.owner {
            return;
        }
        self.pointer = Pos2::new(x, y);
        self.events.push(Event::PointerMoved(self.pointer));
        let index = usize::from(button == PointerButton::Secondary);
        if phase == 1 {
            self.captured[index] = true;
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button,
                pressed: true,
                modifiers: self.modifiers,
            });
        }
        if phase == 3 && self.captured[index] {
            self.captured[index] = false;
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button,
                pressed: false,
                modifiers: self.modifiers,
            });
        }
    }
    pub fn scroll(&mut self, x: f32, y: f32, clicks: i32) {
        self.pointer = Pos2::new(x, y);
        self.events.push(Event::PointerMoved(self.pointer));
        self.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0., clicks as f32),
            phase: TouchPhase::Move,
            modifiers: self.modifiers,
        });
    }
    pub fn key(&mut self, key: u8, virtual_key: u8, flags: i32, losing: bool) {
        if losing {
            self.events.push(Event::WindowFocused(false));
            self.context.memory_mut(|m| m.stop_text_input());
            self.focused = false;
            return;
        }
        self.modifiers = modifiers_from_flags(flags);
        self.events.push(Event::ModifiersChanged(self.modifiers));
        let pressed = flags & xplm_DownFlag != 0;
        let released = flags & xplm_UpFlag != 0;
        if !pressed && !released {
            return;
        }
        if let Some(key) = map_key(key, virtual_key) {
            self.events.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: self.modifiers,
            });
        }
        if pressed && !self.modifiers.ctrl && !self.modifiers.alt && (32..=126).contains(&key) {
            self.events.push(Event::Text((key as char).to_string()));
        }
    }
    pub fn hide(&mut self, window: XPLMWindowID) {
        self.events.clear();
        for (index, button) in [PointerButton::Primary, PointerButton::Secondary]
            .into_iter()
            .enumerate()
        {
            if self.captured[index] {
                self.events.push(Event::PointerButton {
                    pos: self.pointer,
                    button,
                    pressed: false,
                    modifiers: self.modifiers,
                });
            }
        }
        self.events.push(Event::PointerGone);
        self.events.push(Event::WindowFocused(false));
        self.captured = [false; 2];
        self.context.memory_mut(|m| m.stop_text_input());
        if self.focused && !window.is_null() && unsafe { XPLMHasKeyboardFocus(window) } != 0 {
            unsafe {
                XPLMTakeKeyboardFocus(ptr::null_mut());
            }
        }
        self.focused = false;
    }
    pub fn wants_keyboard(&self) -> bool {
        self.context.egui_wants_keyboard_input()
    }
    pub fn frame(
        &mut self,
        geometry: Geometry,
        mut build: impl FnMut(&mut egui::Ui),
    ) -> egui::FullOutput {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                vec2(geometry.width() as f32, geometry.height() as f32),
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            events: mem::take(&mut self.events),
            focused: true,
            ..Default::default()
        };
        self.context.run_ui(raw, |ui| build(ui))
    }
    pub fn paint(
        &mut self,
        window: XPLMWindowID,
        geometry: Geometry,
        mut output: egui::FullOutput,
    ) -> Result<(), String> {
        if thread::current().id() != self.owner {
            return Err("egui requires its owner thread".into());
        }
        self.textures.append(output.textures_delta);
        let keyboard = self.context.egui_wants_keyboard_input();
        if keyboard != self.focused {
            self.focused = keyboard;
            unsafe {
                if keyboard {
                    XPLMTakeKeyboardFocus(window);
                } else if XPLMHasKeyboardFocus(window) != 0 {
                    XPLMTakeKeyboardFocus(ptr::null_mut());
                }
            }
        }
        let mut mv = [0.; 16];
        let mut pr = [0.; 16];
        let mut vp = [0; 4];
        if self
            .datarefs
            .find("sim/graphics/view/modelview_matrix")
            .map(|r| r.read_f32(&mut mv))
            != Some(16)
            || self
                .datarefs
                .find("sim/graphics/view/projection_matrix")
                .map(|r| r.read_f32(&mut pr))
                != Some(16)
            || self
                .datarefs
                .find("sim/graphics/view/viewport")
                .map(|r| r.read_i32(&mut vp))
                != Some(4)
        {
            return Err("X-Plane viewport is unavailable".into());
        }
        let transform =
            RenderTransform::new(mv, pr, vp, geometry).ok_or("X-Plane viewport is invalid")?;
        if unsafe { wglGetCurrentContext() }.is_null() {
            return Err("OpenGL context is not current".into());
        }
        if self.painter.is_none() {
            let gl = Arc::new(unsafe { glow::Context::from_loader_function(gl_proc_address) });
            self.painter = Some(Painter::new(gl, "", None, false).map_err(|e| e.to_string())?);
        }
        let mut primitives = self
            .context
            .tessellate(mem::take(&mut output.shapes), output.pixels_per_point);
        transform_primitives(&mut primitives, &transform);
        set_2d_graphics_state();
        let painter = self.painter.as_mut().unwrap();
        painter.paint_and_update_textures(
            transform.render_size,
            1.,
            &primitives,
            &mut self.textures,
        );
        restore_xplane_gl_state(painter, &transform.viewport);
        Ok(())
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(mut p) = self.painter.take() {
            p.destroy();
        }
        self.textures.clear();
    }
}
pub fn restore_xplane_gl_state(painter: &Painter, viewport: &[i32; 4]) {
    let gl = painter.gl();
    unsafe {
        gl.use_program(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.active_texture(glow::TEXTURE0);
        gl.disable(glow::SCISSOR_TEST);
        gl.viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
    }
    set_2d_graphics_state();
}
pub fn gl_proc_address(name: &str) -> *const c_void {
    let Ok(name) = CString::new(name) else {
        return ptr::null();
    };
    unsafe {
        let p = wglGetProcAddress(name.as_ptr().cast())
            .map(|f| f as usize)
            .unwrap_or(0);
        if p > 3 && p != usize::MAX {
            return p as *const c_void;
        }
        static MODULE: OnceLock<usize> = OnceLock::new();
        let module = *MODULE.get_or_init(|| LoadLibraryA(c"opengl32.dll".as_ptr().cast()) as usize)
            as *mut c_void;
        if module.is_null() {
            ptr::null()
        } else {
            GetProcAddress(module, name.as_ptr().cast())
                .map(|f| f as *const () as *const c_void)
                .unwrap_or(ptr::null())
        }
    }
}
pub fn modifiers_from_flags(flags: i32) -> Modifiers {
    let ctrl = flags & xplm_ControlFlag != 0;
    Modifiers {
        alt: flags & xplm_OptionAltFlag != 0,
        ctrl,
        shift: flags & xplm_ShiftFlag != 0,
        mac_cmd: false,
        command: ctrl,
    }
}
pub fn map_key(key: u8, v: u8) -> Option<Key> {
    match v {
        8 => Some(Key::Backspace),
        9 => Some(Key::Tab),
        13 => Some(Key::Enter),
        27 => Some(Key::Escape),
        33 => Some(Key::PageUp),
        34 => Some(Key::PageDown),
        35 => Some(Key::End),
        36 => Some(Key::Home),
        37 => Some(Key::ArrowLeft),
        38 => Some(Key::ArrowUp),
        39 => Some(Key::ArrowRight),
        40 => Some(Key::ArrowDown),
        45 => Some(Key::Insert),
        46 | 127 => Some(Key::Delete),
        b'0'..=b'9' | b'A'..=b'Z' => Key::from_name(&(v as char).to_string()),
        _ => Key::from_name(&(key as char).to_string()),
    }
}
