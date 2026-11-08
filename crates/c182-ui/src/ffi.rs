//! C++ supplies immutable display snapshots; source actions are returned in FIFO order.
use crate::{
    theme,
    view::{Action, Page, Readout, Row, ViewState},
};
use std::{
    collections::VecDeque,
    ffi::{c_char, CStr},
    slice,
};
use xplane_egui::{Geometry, Surface};
use xplane_sdk_sys::{XPLMDebugString, XPLMWindowID};

#[repr(C)]
struct RawRow {
    id: u32,
    plus: u32,
    flags: u32,
    column: u32,
    mask: u32,
    plus_mask: u32,
    tone: u32,
    label: *const c_char,
    section: *const c_char,
    reason: *const c_char,
    value: f64,
    low: f64,
    high: f64,
}
#[repr(C)]
struct RawReadout {
    text: *const c_char,
    tone: u32,
}
#[repr(C)]
pub struct RawPage {
    tab: u32,
    section: u32,
    ready: u32,
    opacity: f32,
    title: *const c_char,
    rows: *const RawRow,
    row_count: usize,
    readouts: *const RawReadout,
    readout_count: usize,
    report: *const c_char,
}
#[repr(C)]
pub struct RawAction {
    kind: u32,
    id: u32,
    flag: u32,
    reserved: u32,
    value: f64,
}
pub struct State {
    surface: Surface,
    view: ViewState,
    actions: VecDeque<Action>,
    last_error: String,
    last_window: XPLMWindowID,
}
unsafe fn text(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}
unsafe fn page(raw: &RawPage) -> Option<Page> {
    if raw.row_count > 8192
        || raw.readout_count > 8192
        || raw.row_count > 0 && raw.rows.is_null()
        || raw.readout_count > 0 && raw.readouts.is_null()
    {
        return None;
    }
    let rows = if raw.row_count == 0 {
        &[][..]
    } else {
        slice::from_raw_parts(raw.rows, raw.row_count)
    };
    let readouts = if raw.readout_count == 0 {
        &[][..]
    } else {
        slice::from_raw_parts(raw.readouts, raw.readout_count)
    };
    Some(Page {
        title: text(raw.title),
        tab: if raw.tab < 8 {
            Some(raw.tab as usize)
        } else {
            None
        },
        section: raw.section,
        ready: raw.ready != 0,
        opacity: raw.opacity,
        rows: rows
            .iter()
            .map(|r| Row {
                id: r.id,
                plus: if r.plus == u32::MAX {
                    None
                } else {
                    Some(r.plus)
                },
                label: text(r.label),
                section: text(r.section),
                reason: text(r.reason),
                column: (r.column as usize).min(1),
                visible: r.flags & 1 != 0,
                enabled: r.flags & 2 != 0 && raw.ready != 0,
                plus_enabled: r.flags & 4 != 0 && raw.ready != 0,
                mask: r.mask,
                plus_mask: r.plus_mask,
                tone: r.tone,
                value: if r.flags & 8 != 0
                    && r.value.is_finite()
                    && r.low.is_finite()
                    && r.high.is_finite()
                    && r.low <= r.high
                {
                    Some((r.value, r.low, r.high))
                } else {
                    None
                },
            })
            .collect(),
        readouts: readouts
            .iter()
            .map(|r| Readout {
                text: text(r.text),
                tone: r.tone,
            })
            .collect(),
        report: text(raw.report),
    })
}
#[no_mangle]
pub extern "C" fn c182_egui_new() -> *mut State {
    let surface = Surface::new();
    theme::apply(&surface.context);
    Box::into_raw(Box::new(State {
        surface,
        view: ViewState::default(),
        actions: VecDeque::new(),
        last_error: String::new(),
        last_window: std::ptr::null_mut(),
    }))
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_free(s: *mut State) {
    if !s.is_null() {
        let mut state = Box::from_raw(s);
        state.surface.hide(state.last_window);
    }
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_draw(
    s: *mut State,
    window: XPLMWindowID,
    raw: *const RawPage,
    l: i32,
    t: i32,
    r: i32,
    b: i32,
) -> i32 {
    if s.is_null() || raw.is_null() || r <= l || t <= b {
        return 0;
    }
    let Some(page) = page(&*raw) else {
        return 0;
    };
    let state = &mut *s;
    state.last_window = window;
    let saved = state.view.clone();
    let mut final_view = saved.clone();
    let geometry = Geometry {
        left: l,
        top: t,
        right: r,
        bottom: b,
    };
    let output = state.surface.frame(geometry, |ui| {
        let mut next = saved.clone();
        next.show(ui, &page);
        final_view = next;
    });
    state.view = final_view;
    state.actions.extend(state.view.take_actions());
    if let Err(error) = state.surface.paint(window, geometry, output) {
        if error != state.last_error {
            state.last_error = error.clone();
            let line = xplane_plugin::c_string(&format!("C182 egui: {error}\n"));
            XPLMDebugString(line.as_ptr());
        }
        return 1;
    }
    state.last_error.clear();
    2
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_mouse(s: *mut State, x: f32, y: f32, phase: i32, button: i32) {
    if let Some(s) = s.as_mut() {
        s.surface.mouse(
            x,
            y,
            phase,
            if button == 1 {
                egui::PointerButton::Secondary
            } else {
                egui::PointerButton::Primary
            },
        );
    }
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_wheel(s: *mut State, x: f32, y: f32, clicks: i32) {
    if let Some(s) = s.as_mut() {
        s.view.wheel_clicks = (s.view.wheel_clicks + clicks).clamp(-32, 32);
        s.surface.scroll(x, y, clicks);
    }
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_key(
    s: *mut State,
    key: u8,
    virtual_key: u8,
    flags: i32,
    losing: i32,
) {
    if let Some(s) = s.as_mut() {
        s.surface.key(key, virtual_key, flags, losing != 0);
    }
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_hide(s: *mut State, window: XPLMWindowID) {
    if let Some(s) = s.as_mut() {
        s.view.release_all();
        s.view.take_actions();
        s.actions.clear();
        s.surface.hide(window);
    }
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_wants_keyboard(s: *mut State) -> i32 {
    s.as_ref()
        .map(|s| i32::from(s.surface.wants_keyboard()))
        .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "C" fn c182_egui_action(s: *mut State, out: *mut RawAction) -> i32 {
    if s.is_null() || out.is_null() {
        return 0;
    }
    let Some(action) = (*s).actions.pop_front() else {
        return 0;
    };
    *out = match action {
        Action::Source { id, flag } => RawAction {
            kind: 0,
            id,
            flag,
            reserved: 0,
            value: 0.,
        },
        Action::Tab(tab) => RawAction {
            kind: 1,
            id: tab as u32,
            flag: 0,
            reserved: 0,
            value: 0.,
        },
        Action::Slider { id, value } => RawAction {
            kind: 2,
            id,
            flag: 0,
            reserved: 0,
            value,
        },
        Action::Section(section) => RawAction {
            kind: 3,
            id: section,
            flag: 0,
            reserved: 0,
            value: 0.,
        },
        Action::Close => RawAction {
            kind: 4,
            id: 0,
            flag: 0,
            reserved: 0,
            value: 0.,
        },
    };
    1
}
