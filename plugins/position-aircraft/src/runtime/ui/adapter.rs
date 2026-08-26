use std::ffi::{c_char, c_int, c_void, CString};
use std::mem;
use std::ptr;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use egui::{
    epaint::Primitive, vec2, Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput,
    Rect, TouchPhase,
};
use egui_glow::{glow, Painter};
use glow::HasContext;
use windows_sys::Win32::Graphics::OpenGL::wglGetProcAddress;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use crate::runtime::support::log;
use crate::runtime::{with_state_mut, CommandAction, PluginState};
use xplane_sdk_sys::*;

use super::theme;
use super::view::{self, Action, HitCursor, HitRegion};

pub(in crate::runtime) struct EguiIntegration {
    context: egui::Context,
    painter: Option<Painter>,
    events: Vec<Event>,
    modifiers: Modifiers,
    pointer_pos: Pos2,
    hit_regions: Vec<HitRegion>,
    popup_open: bool,
    mouse_captured: bool,
    keyboard_focused: bool,
    started_at: Instant,
    renderer_error: Option<String>,
}

impl EguiIntegration {
    pub(in crate::runtime) fn new() -> Self {
        let context = egui::Context::default();
        theme::apply(&context);
        Self {
            context,
            painter: None,
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer_pos: Pos2::ZERO,
            hit_regions: Vec::new(),
            popup_open: false,
            mouse_captured: false,
            keyboard_focused: false,
            started_at: Instant::now(),
            renderer_error: None,
        }
    }

    pub(in crate::runtime) fn hide(&mut self) {
        self.events.push(Event::PointerGone);
        self.events.push(Event::WindowFocused(false));
        self.context.memory_mut(|memory| memory.stop_text_input());
        self.hit_regions.clear();
        self.popup_open = false;
        self.mouse_captured = false;
        self.keyboard_focused = false;
    }

    pub(in crate::runtime) fn destroy_renderer(&mut self) {
        if let Some(mut painter) = self.painter.take() {
            painter.destroy();
        }
    }

    fn hit_cursor(&self, position: Pos2) -> Option<HitCursor> {
        self.hit_regions
            .iter()
            .rev()
            .find(|region| region.rect.contains(position))
            .map(|region| region.cursor)
    }

    fn pointer_moved(&mut self, position: Pos2) {
        self.pointer_pos = position;
        self.events.push(Event::PointerMoved(position));
    }

    fn begin_pointer(&mut self, position: Pos2) -> bool {
        self.pointer_moved(position);
        let handled = self.hit_cursor(position).is_some()
            || self.popup_open
            || self.context.egui_is_using_pointer();
        if handled {
            self.events.push(Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: self.modifiers,
            });
            self.mouse_captured = true;
        }
        handled
    }

    fn continue_pointer(&mut self, position: Pos2, released: bool) -> bool {
        if !self.mouse_captured {
            return false;
        }
        self.pointer_moved(position);
        if released {
            self.events.push(Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: self.modifiers,
            });
            self.mouse_captured = false;
        }
        true
    }

    fn scroll(&mut self, position: Pos2, clicks: i32) -> bool {
        self.pointer_moved(position);
        if self.hit_cursor(position).is_none() && !self.popup_open {
            return false;
        }
        self.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, clicks as f32),
            phase: TouchPhase::Move,
            modifiers: self.modifiers,
        });
        true
    }

    fn key_event(&mut self, key: u8, virtual_key: u8, flags: i32) {
        self.modifiers = modifiers_from_flags(flags);
        self.events.push(Event::ModifiersChanged(self.modifiers));
        let pressed = flags & xplm_DownFlag != 0;
        let released = flags & xplm_UpFlag != 0;
        if !pressed && !released {
            return;
        }
        if let Some(mapped) = map_key(key, virtual_key) {
            self.events.push(Event::Key {
                key: mapped,
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

    fn lose_keyboard_focus(&mut self) {
        self.events.push(Event::WindowFocused(false));
        self.keyboard_focused = false;
    }

    fn cursor_status(&self, position: Pos2) -> XPLMCursorStatus {
        match self.hit_cursor(position) {
            Some(HitCursor::Text | HitCursor::Arrow) => xplm_CursorArrow,
            None if self.popup_open => xplm_CursorArrow,
            None => xplm_CursorDefault,
        }
    }

    fn draw(&mut self, window: XPLMWindowID, state: &mut PluginState) {
        let geometry = WindowGeometry::get(window);
        if geometry.width() <= 0 || geometry.height() <= 0 {
            return;
        }

        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                vec2(geometry.width() as f32, geometry.height() as f32),
            )),
            time: Some(self.started_at.elapsed().as_secs_f64()),
            events: mem::take(&mut self.events),
            focused: true,
            ..Default::default()
        };

        let context = self.context.clone();
        let mut view_output = None;
        let mut full_output = context.run_ui(input, |ui| {
            view_output = Some(view::show(ui, state));
        });
        let view_output = view_output.expect("egui UI closure did not run");
        self.hit_regions = view_output.hit_regions;
        self.popup_open = context.any_popup_open();
        self.update_keyboard_focus(window, context.egui_wants_keyboard_input());
        for action in view_output.actions {
            apply_action(state, action);
        }

        if self.renderer_error.is_some() {
            return;
        }
        if let Err(error) = self.ensure_renderer() {
            self.renderer_error = Some(error.clone());
            state.status = format!("UI renderer unavailable: {error}");
            log(&state.status);
            return;
        }

        let Some(transform) = RenderTransform::capture(state, geometry) else {
            state.status = "UI renderer unavailable: invalid X-Plane viewport".to_owned();
            return;
        };
        let mut primitives = context.tessellate(full_output.shapes, full_output.pixels_per_point);
        transform_primitives(&mut primitives, &transform);

        // SAFETY: this runs only inside XPLM's window draw callback while its
        // graphics context is current.
        unsafe { XPLMSetGraphicsState(0, 0, 0, 0, 1, 0, 0) };
        let painter = self.painter.as_mut().unwrap();
        painter.paint_and_update_textures(
            transform.render_size,
            1.0,
            &primitives,
            &mut full_output.textures_delta,
        );
        restore_xplane_gl_state(painter, &transform);
    }

    fn update_keyboard_focus(&mut self, window: XPLMWindowID, wants_keyboard: bool) {
        if wants_keyboard == self.keyboard_focused {
            return;
        }
        self.keyboard_focused = wants_keyboard;
        // SAFETY: `window` is the live handle supplied to the XPLM callback;
        // null is the documented way to release keyboard focus.
        unsafe {
            XPLMTakeKeyboardFocus(if wants_keyboard {
                window
            } else {
                ptr::null_mut()
            });
        }
    }

    fn ensure_renderer(&mut self) -> Result<(), String> {
        if self.painter.is_some() {
            return Ok(());
        }
        // SAFETY: XPLM invokes renderer creation from its draw callback with a
        // current OpenGL compatibility context; the loader returns addresses
        // from that context or opengl32.dll.
        let gl = Arc::new(unsafe { glow::Context::from_loader_function(gl_proc_address) });
        self.painter = Some(Painter::new(gl, "", None, false).map_err(|error| format!("{error}"))?);
        Ok(())
    }
}

#[derive(Copy, Clone)]
struct WindowGeometry {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl WindowGeometry {
    fn get(window: XPLMWindowID) -> Self {
        let mut geometry = Self {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: `window` is supplied by XPLM and every output pointer refers
        // to a live field in `geometry`.
        unsafe {
            XPLMGetWindowGeometry(
                window,
                &mut geometry.left,
                &mut geometry.top,
                &mut geometry.right,
                &mut geometry.bottom,
            );
        }
        geometry
    }

    fn width(self) -> i32 {
        self.right - self.left
    }

    fn height(self) -> i32 {
        self.top - self.bottom
    }

    fn local(self, x: i32, y: i32) -> Pos2 {
        Pos2::new((x - self.left) as f32, (self.top - y) as f32)
    }
}

struct RenderTransform {
    modelview: [f32; 16],
    projection: [f32; 16],
    viewport: [i32; 4],
    window: WindowGeometry,
    render_size: [u32; 2],
}

impl RenderTransform {
    fn capture(state: &PluginState, window: WindowGeometry) -> Option<Self> {
        let mut modelview = [0.0; 16];
        let mut projection = [0.0; 16];
        let mut viewport = [0; 4];
        if state.datarefs.modelview_matrix.read_f32(&mut modelview) != 16
            || state.datarefs.projection_matrix.read_f32(&mut projection) != 16
            || state.datarefs.viewport.read_i32(&mut viewport) != 4
            || viewport[0] < 0
            || viewport[1] < 0
            || viewport[2] <= 0
            || viewport[3] <= 0
        {
            return None;
        }
        let render_width = viewport[0].checked_add(viewport[2])? as u32;
        let render_height = viewport[1].checked_add(viewport[3])? as u32;
        Some(Self {
            modelview,
            projection,
            viewport,
            window,
            render_size: [render_width, render_height],
        })
    }

    fn point(&self, position: Pos2) -> Pos2 {
        let model = [
            self.window.left as f32 + position.x,
            self.window.top as f32 - position.y,
            0.0,
            1.0,
        ];
        let eye = multiply_matrix_vector(&self.modelview, model);
        let clip = multiply_matrix_vector(&self.projection, eye);
        if clip[3].abs() < f32::EPSILON {
            return position;
        }
        let ndc_x = clip[0] / clip[3];
        let ndc_y = clip[1] / clip[3];
        let pixel_x = self.viewport[0] as f32 + (ndc_x + 1.0) * self.viewport[2] as f32 * 0.5;
        let pixel_y = self.viewport[1] as f32 + (ndc_y + 1.0) * self.viewport[3] as f32 * 0.5;
        Pos2::new(pixel_x, self.render_size[1] as f32 - pixel_y)
    }

    fn rect(&self, rect: Rect) -> Rect {
        let points = [
            self.point(rect.left_top()),
            self.point(rect.right_top()),
            self.point(rect.right_bottom()),
            self.point(rect.left_bottom()),
        ];
        let mut min = points[0];
        let mut max = points[0];
        for point in points.into_iter().skip(1) {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }
        Rect::from_min_max(min, max)
    }
}

fn multiply_matrix_vector(matrix: &[f32; 16], vector: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|row| {
        matrix[row] * vector[0]
            + matrix[4 + row] * vector[1]
            + matrix[8 + row] * vector[2]
            + matrix[12 + row] * vector[3]
    })
}

fn transform_primitives(primitives: &mut [egui::ClippedPrimitive], transform: &RenderTransform) {
    for primitive in primitives {
        primitive.clip_rect = transform.rect(primitive.clip_rect);
        if let Primitive::Mesh(mesh) = &mut primitive.primitive {
            for vertex in &mut mesh.vertices {
                vertex.pos = transform.point(vertex.pos);
            }
        }
    }
}

fn restore_xplane_gl_state(painter: &Painter, transform: &RenderTransform) {
    let gl = painter.gl();
    // SAFETY: this runs in XPLM's draw callback with the painter's GL context
    // current. Values are restored to X-Plane's captured viewport.
    unsafe {
        gl.use_program(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.active_texture(glow::TEXTURE0);
        gl.disable(glow::SCISSOR_TEST);
        gl.viewport(
            transform.viewport[0],
            transform.viewport[1],
            transform.viewport[2],
            transform.viewport[3],
        );
        XPLMSetGraphicsState(0, 0, 0, 0, 1, 0, 0);
    }
}

fn apply_action(state: &mut PluginState, action: Action) {
    match action {
        Action::Command(CommandAction::CaptureCurrent) => {
            state.capture_current();
        }
        Action::Command(CommandAction::PositionLoaded) => state.position_loaded(),
        Action::Command(CommandAction::QuickSave) => state.quick_save(),
        Action::Command(CommandAction::QuickLoadAndPosition) => state.quick_load(true),
        Action::Command(CommandAction::PreviousPad) => state.select_relative(-1, false),
        Action::Command(CommandAction::NextPad) => state.select_relative(1, false),
        Action::Command(CommandAction::ToggleWindow) => state.toggle_window(),
        Action::Command(CommandAction::QuickLoad) => state.quick_load(false),
        Action::Command(CommandAction::PreviousPadAndPosition) => state.select_relative(-1, true),
        Action::Command(CommandAction::NextPadAndPosition) => state.select_relative(1, true),
        Action::LoadSelected(position) => state.load_selected(position),
        Action::Refresh => state.refresh_pads(),
        Action::SelectPad(index) => state.select_pad(index),
        Action::SaveNamed => state.save_named(),
    }
}

fn modifiers_from_flags(flags: i32) -> Modifiers {
    let ctrl = flags & xplm_ControlFlag != 0;
    Modifiers {
        alt: flags & xplm_OptionAltFlag != 0,
        ctrl,
        shift: flags & xplm_ShiftFlag != 0,
        mac_cmd: false,
        command: ctrl,
    }
}

fn map_key(key: u8, virtual_key: u8) -> Option<Key> {
    match virtual_key {
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
        _ => Key::from_name(&(key as char).to_string()),
    }
}

fn gl_proc_address(name: &str) -> *const c_void {
    let Ok(name) = CString::new(name) else {
        return ptr::null();
    };
    unsafe {
        // SAFETY: these are the platform OpenGL loaders. `name` is a live
        // NUL-terminated string, and the returned address is used only by glow.
        let extension = wglGetProcAddress(name.as_ptr().cast());
        let address = extension.map(|function| function as usize).unwrap_or(0);
        if address > 3 && address != usize::MAX {
            return address as *const c_void;
        }
        static OPENGL32: OnceLock<usize> = OnceLock::new();
        let module = *OPENGL32
            .get_or_init(|| LoadLibraryA(c"opengl32.dll".as_ptr().cast()) as usize)
            as *mut c_void;
        if module.is_null() {
            ptr::null()
        } else {
            GetProcAddress(module, name.as_ptr().cast())
                .map(|function| function as *const () as *const c_void)
                .unwrap_or(ptr::null())
        }
    }
}

pub(in crate::runtime) unsafe extern "C" fn draw_window(
    window: XPLMWindowID,
    _refcon: *mut c_void,
) {
    with_state_mut(|state| {
        let Some(mut ui) = state.ui.take() else {
            return;
        };
        ui.draw(window, state);
        state.ui = Some(ui);
    });
}

pub(in crate::runtime) unsafe extern "C" fn handle_mouse(
    window: XPLMWindowID,
    x: c_int,
    y: c_int,
    mouse_status: XPLMMouseStatus,
    _refcon: *mut c_void,
) -> c_int {
    let geometry = WindowGeometry::get(window);
    let position = geometry.local(x, y);
    with_state_mut(|state| {
        let Some(ui) = state.ui.as_mut() else {
            return 0;
        };
        let handled = if mouse_status == xplm_MouseDown {
            ui.begin_pointer(position)
        } else if mouse_status == xplm_MouseDrag {
            ui.continue_pointer(position, false)
        } else if mouse_status == xplm_MouseUp {
            ui.continue_pointer(position, true)
        } else {
            false
        };
        i32::from(handled)
    })
    .unwrap_or(0)
}

pub(in crate::runtime) unsafe extern "C" fn handle_right_click(
    _window: XPLMWindowID,
    _x: c_int,
    _y: c_int,
    _mouse_status: XPLMMouseStatus,
    _refcon: *mut c_void,
) -> c_int {
    0
}

pub(in crate::runtime) unsafe extern "C" fn handle_cursor(
    window: XPLMWindowID,
    x: c_int,
    y: c_int,
    _refcon: *mut c_void,
) -> XPLMCursorStatus {
    let geometry = WindowGeometry::get(window);
    let position = geometry.local(x, y);
    with_state_mut(|state| {
        let Some(ui) = state.ui.as_mut() else {
            return xplm_CursorDefault;
        };
        ui.pointer_moved(position);
        ui.cursor_status(position)
    })
    .unwrap_or(xplm_CursorDefault)
}

pub(in crate::runtime) unsafe extern "C" fn handle_wheel(
    window: XPLMWindowID,
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    _refcon: *mut c_void,
) -> c_int {
    if wheel != 0 || clicks == 0 {
        return 0;
    }
    let position = WindowGeometry::get(window).local(x, y);
    with_state_mut(|state| {
        state
            .ui
            .as_mut()
            .map(|ui| i32::from(ui.scroll(position, clicks)))
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

pub(in crate::runtime) unsafe extern "C" fn handle_key(
    _window: XPLMWindowID,
    key: c_char,
    flags: XPLMKeyFlags,
    virtual_key: c_char,
    _refcon: *mut c_void,
    losing_focus: c_int,
) {
    with_state_mut(|state| {
        let Some(ui) = state.ui.as_mut() else {
            return;
        };
        if losing_focus != 0 {
            ui.lose_keyboard_focus();
        } else {
            ui.key_event(key as u8, virtual_key as u8, flags);
        }
    });
}
