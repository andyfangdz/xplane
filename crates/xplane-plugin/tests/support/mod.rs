// An invisible window supplies a driver compatibility context, including
// non-power-of-two textures used by the native HUD font atlas.
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

pub struct Context {
    window: HWND,
    dc: HDC,
    gl: HGLRC,
}

impl Context {
    pub fn new() -> Self {
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
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
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
