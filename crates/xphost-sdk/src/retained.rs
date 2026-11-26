//! SDK panel recordings hold owned, untransformed operations. Replaying these
//! through the normal capture path applies the destination's transform and clip.
use crate::{
    panel::{Drawing, Panel},
    state::{handle, index, state},
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    sync::Arc,
};
use xplane_sdk_sys::XPLMRetainedDrawing_t;

// Accessors can re-enter the plugin while an outer callback is recording. Only
// the callback that began a recording may leak it across its return boundary.
thread_local! {
    static CALLBACK_DEPTH: Cell<usize> = const { Cell::new(0) };
    static RECORDING_DEPTH: Cell<Option<usize>> = const { Cell::new(None) };
}
pub struct CallbackScope(usize);
impl CallbackScope {
    pub fn enter() -> Self {
        let depth = CALLBACK_DEPTH.get() + 1;
        CALLBACK_DEPTH.set(depth);
        Self(depth)
    }
}
impl Drop for CallbackScope {
    fn drop(&mut self) {
        if RECORDING_DEPTH.get() == Some(self.0) {
            state().panel.end_callback();
        }
        CALLBACK_DEPTH.set(self.0 - 1);
    }
}
type Apply = dyn Fn(&mut Drawing) -> Result<(), &'static str> + Send + Sync;
#[derive(Clone, Default)]
struct List {
    ops: Vec<Arc<Apply>>,
    resources: HashSet<usize>,
    invalid: bool,
    axis_aligned: bool,
}
#[derive(Default)]
struct Recording {
    list: List,
    check: Drawing,
}
#[derive(Default)]
pub struct Store {
    lists: HashMap<usize, Arc<List>>,
    open: Option<Recording>,
    recorded: u64,
    replayed: u64,
    destroyed: u64,
}
impl Store {
    pub fn stats(&self) -> Value {
        json!({"recorded":self.recorded,"replayed":self.replayed,
               "destroyed":self.destroyed,"live":self.lists.len()})
    }
    pub fn mark_recording_invalid(&mut self) {
        if let Some(open) = &mut self.open {
            open.list.invalid = true;
        }
    }
}
impl Panel {
    pub fn draw(
        &mut self,
        op: &'static str,
        call: impl Fn(&mut Drawing) -> Result<(), &'static str> + Send + Sync + 'static,
    ) {
        if let Some(open) = &mut self.retained.open {
            // Check the recording by itself, without the surrounding callback's state.
            // The check's rendered JSON is never emitted or retained.
            match call(&mut open.check) {
                Ok(()) => {
                    open.check.commands.clear();
                    open.list.axis_aligned |= matches!(op, "scissor" | "mesh" | "map" | "svt");
                    open.list.ops.push(Arc::new(call));
                }
                Err(error) => self.error(op, error),
            }
        } else if let Err(error) = self
            .active
            .as_mut()
            .ok_or("outside a native panel draw callback")
            .and_then(call)
        {
            self.error(op, error);
        }
    }
    pub fn draw_resource(
        &mut self,
        op: &'static str,
        resource: usize,
        call: impl Fn(&mut Drawing) -> Result<(), &'static str> + Send + Sync + 'static,
    ) {
        if let Some(open) = &mut self.retained.open {
            open.list.resources.insert(resource);
        }
        self.draw(op, call);
    }
    /// SDK resource handles come from the same monotonic allocator. A retained
    /// drawing must be destroyed before any resource it references.
    pub fn retire_resource(&mut self, resource: usize) {
        let in_use = self
            .retained
            .open
            .as_ref()
            .is_some_and(|open| open.list.resources.contains(&resource))
            || self
                .retained
                .lists
                .values()
                .any(|list| list.resources.contains(&resource));
        if in_use {
            self.error("destroy resource", "referenced by a retained drawing");
            for list in self
                .retained
                .lists
                .values_mut()
                .filter(|list| list.resources.contains(&resource))
            {
                // Dependencies are flattened at recording, including nested lists.
                Arc::make_mut(list).invalid = true;
            }
        }
    }
    pub fn end_callback(&mut self) {
        RECORDING_DEPTH.set(None);
        if self.retained.open.take().is_some() {
            self.error("end callback", "discarded unclosed retained drawing");
        }
    }
    fn begin_retained(&mut self) {
        if self.retained.open.is_some() {
            self.error("begin retained", "recordings cannot nest");
        } else {
            self.retained.open = Some(Recording::default());
            RECORDING_DEPTH.set(Some(CALLBACK_DEPTH.get()));
        }
    }
    fn end_retained(&mut self, id: usize) -> XPLMRetainedDrawing_t {
        if RECORDING_DEPTH
            .get()
            .is_some_and(|owner| owner != CALLBACK_DEPTH.get())
        {
            self.error("end retained", "recording belongs to an enclosing callback");
            return std::ptr::null_mut();
        }
        RECORDING_DEPTH.set(None);
        let Some(open) = self.retained.open.take() else {
            self.error("end retained", "no recording");
            return std::ptr::null_mut();
        };
        if open.list.invalid {
            return std::ptr::null_mut();
        }
        if !open.check.transforms.is_empty() || !open.check.scissors.is_empty() {
            self.error("end retained", "unbalanced transform or scissor stack");
            return std::ptr::null_mut();
        }
        self.retained.lists.insert(id, Arc::new(open.list));
        self.retained.recorded += 1;
        handle(id)
    }
    fn replay(&mut self, id: usize) {
        let Some(list) = self.retained.lists.get(&id).cloned() else {
            self.error("draw retained", "unknown drawing");
            return;
        };
        if list.invalid {
            self.error("draw retained", "referenced resource was destroyed");
            return;
        }
        if let Some(open) = &mut self.retained.open {
            open.list.resources.extend(&list.resources);
            open.list.axis_aligned |= list.axis_aligned;
        } else if let Some(d) = &self.active
            && !(list.axis_aligned && d.transform.rotated)
        {
            self.retained.replayed += 1;
        }
        self.draw_resource("draw retained", id, move |destination| {
            if list.axis_aligned && destination.transform.rotated {
                return Err("axis-aligned drawing under rotation");
            }
            let prior_clip = destination.clip;
            let prior_limit = destination.clip_limit;
            destination.clip_limit = prior_clip;
            let result = list.ops.iter().try_for_each(|op| op(destination));
            destination.clip_limit = prior_limit;
            destination.clip = prior_clip;
            result
        });
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMBeginRetainedDrawing() {
    state().panel.begin_retained();
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMEndRetainedDrawing() -> XPLMRetainedDrawing_t {
    let mut s = state();
    let id = s.handle();
    s.panel.end_retained(id)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDrawRetained(drawing: XPLMRetainedDrawing_t) {
    state().panel.replay(index(drawing));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyRetainedDrawing(drawing: XPLMRetainedDrawing_t) {
    let mut s = state();
    let id = index(drawing);
    if s.panel.retained.lists.remove(&id).is_some() {
        s.panel.retained.destroyed += 1;
        s.panel.retire_resource(id);
    } else {
        s.panel.error("destroy retained", "unknown drawing");
    }
}
