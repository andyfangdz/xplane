//! Uses a real compatibility context in an invisible window; no simulator needed.
#![cfg(all(windows, feature = "opengl"))]

use std::{mem::size_of, ptr};
use windows_sys::{
    core::w,
    Win32::{
        Foundation::HWND,
        Graphics::{
            Gdi::{GetDC, ReleaseDC, HDC},
            OpenGL::*,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_POPUP},
    },
};
use xplane_plugin::opengl::{AttributeGuard, MatrixGuard};

struct Context {
    window: HWND,
    dc: HDC,
    gl: HGLRC,
}

impl Context {
    fn new() -> Self {
        let mut context = Self {
            window: ptr::null_mut(),
            dc: ptr::null_mut(),
            gl: ptr::null_mut(),
        };
        // SAFETY: uses the built-in STATIC window class with no callbacks or
        // borrowed data; the owned handles are cleaned up even on test failure.
        unsafe {
            context.window = CreateWindowExW(
                0,
                w!("STATIC"),
                w!("HUD GL test"),
                WS_POPUP,
                0,
                0,
                64,
                64,
                ptr::null_mut(),
                ptr::null_mut(),
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            );
            assert!(!context.window.is_null(), "create hidden window");
            context.dc = GetDC(context.window);
            assert!(!context.dc.is_null(), "acquire device context");
            let format = PIXELFORMATDESCRIPTOR {
                nSize: size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 24,
                cDepthBits: 16,
                iLayerType: PFD_MAIN_PLANE as u8,
                ..Default::default()
            };
            let index = ChoosePixelFormat(context.dc, &format);
            assert!(index > 0, "choose GL pixel format");
            assert_ne!(SetPixelFormat(context.dc, index, &format), 0);
            context.gl = wglCreateContext(context.dc);
            assert!(!context.gl.is_null(), "create compatibility context");
            assert_ne!(wglMakeCurrent(context.dc, context.gl), 0);
        }
        context
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: handles are uniquely owned and dropped on the test thread.
        unsafe {
            if !self.gl.is_null() {
                wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
                wglDeleteContext(self.gl);
            }
            if !self.dc.is_null() {
                ReleaseDC(self.window, self.dc);
            }
            if !self.window.is_null() {
                DestroyWindow(self.window);
            }
        }
    }
}

#[derive(Debug, Default, PartialEq)]
struct State {
    modelview: [f64; 16],
    projection: [f64; 16],
    mode: i32,
    modelview_depth: i32,
    projection_depth: i32,
    attribute_depth: i32,
    scissor: [i32; 4],
    color: [f32; 4],
    line_width: f32,
    scissor_enabled: u8,
    blend_enabled: u8,
}

unsafe fn state() -> State {
    let mut s = State::default();
    // SAFETY: the test keeps a current context; each buffer has the size required
    // by its GL query and remains live for the call.
    unsafe {
        glGetDoublev(GL_MODELVIEW_MATRIX, s.modelview.as_mut_ptr());
        glGetDoublev(GL_PROJECTION_MATRIX, s.projection.as_mut_ptr());
        glGetIntegerv(GL_MATRIX_MODE, &mut s.mode);
        glGetIntegerv(GL_MODELVIEW_STACK_DEPTH, &mut s.modelview_depth);
        glGetIntegerv(GL_PROJECTION_STACK_DEPTH, &mut s.projection_depth);
        glGetIntegerv(GL_ATTRIB_STACK_DEPTH, &mut s.attribute_depth);
        glGetIntegerv(GL_SCISSOR_BOX, s.scissor.as_mut_ptr());
        glGetFloatv(GL_CURRENT_COLOR, s.color.as_mut_ptr());
        glGetFloatv(GL_LINE_WIDTH, &mut s.line_width);
        s.scissor_enabled = glIsEnabled(GL_SCISSOR_TEST);
        s.blend_enabled = glIsEnabled(GL_BLEND);
    }
    s
}

#[test]
fn hud_scopes_restore_real_gl_state_including_nested_clipping_and_early_return() {
    let _context = Context::new();
    // SAFETY: the context outlives every guard; stack entries are nested in
    // LIFO order and no guard is dropped inside a primitive.
    unsafe {
        glMatrixMode(GL_PROJECTION);
        glOrtho(0.0, 640.0, 0.0, 480.0, -1.0, 1.0);
        glMatrixMode(GL_MODELVIEW);
        glTranslated(3.0, 7.0, 0.0);
        glMatrixMode(GL_TEXTURE);
        glColor4f(0.25, 0.5, 0.75, 1.0);
        glEnable(GL_SCISSOR_TEST);
        glEnable(GL_BLEND);
        glScissor(1, 2, 30, 40);
        let before = state();
        // Modelview-only scope used by the SR20 renderer, with nested clips
        // and text rotations still controlled by the renderer.
        {
            let _attributes = AttributeGuard::push(GL_LINE_BIT | GL_CURRENT_BIT | GL_SCISSOR_BIT);
            glDisable(GL_SCISSOR_TEST);
            let _modelview = MatrixGuard::modelview();
            glTranslated(0.0, 1080.0, 0.0);
            glScaled(1.0, -1.0, 1.0);
            let inside = state();
            {
                let _clip = AttributeGuard::push(GL_SCISSOR_BIT);
                glEnable(GL_SCISSOR_TEST);
                glScissor(10, 20, 100, 200);
                let _text = MatrixGuard::modelview();
                glRotated(30.0, 0.0, 0.0, 1.0);
            }
            assert_eq!(state(), inside);
            glColor4f(1.0, 0.0, 0.0, 0.5);
            glLineWidth(2.0);
            glMatrixMode(GL_TEXTURE);
        }
        assert_eq!(state(), before);
        // The Shuttle saves both matrices. Exercise screen and atlas transforms
        // and cleanup when a drawing scope returns early.
        for panel in [false, true] {
            (|| {
                let _attributes =
                    AttributeGuard::push(GL_CURRENT_BIT | GL_SCISSOR_BIT | GL_ENABLE_BIT);
                let _projection = MatrixGuard::projection();
                if !panel {
                    glLoadIdentity();
                    glOrtho(0.0, 1920.0, 0.0, 1080.0, -1.0, 1.0);
                }
                let _modelview = MatrixGuard::modelview();
                glTranslated(2500.0, 1200.0, 0.0);
                glScaled(0.5, -0.5, 1.0);
                glDisable(GL_BLEND);
                glDisable(GL_SCISSOR_TEST);
                glColor4f(0.0, 1.0, 0.0, 0.8);
                glMatrixMode(GL_TEXTURE);
                if panel {
                    return;
                }
                glMatrixMode(GL_MODELVIEW);
            })();
            assert_eq!(state(), before);
        }
        assert_eq!(glGetError(), GL_NO_ERROR);
    }
}
