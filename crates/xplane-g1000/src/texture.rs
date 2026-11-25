//! Pixel upload and framebuffer blit. Called with the simulator's draw context current.
use crate::display::viewport;
use std::ffi::CStr;
use windows_sys::Win32::Graphics::OpenGL::*;
use xplane_plugin::xplm;
type Proc = unsafe extern "system" fn() -> isize;
type Bind = unsafe extern "system" fn(u32, u32);
type Gen = unsafe extern "system" fn(i32, *mut u32);
type Del = unsafe extern "system" fn(i32, *const u32);
type Attach = unsafe extern "system" fn(u32, u32, u32, u32, i32);
type Blit = unsafe extern "system" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32);
type Active = unsafe extern "system" fn(u32);
type Status = unsafe extern "system" fn(u32) -> u32;
struct Gl {
    active: Active,
    buffer: Bind,
    status: Status,
    bind: Bind,
    gen_fb: Gen,
    delete: Del,
    attach: Attach,
    blit: Blit,
}
impl Gl {
    fn load() -> Option<Self> {
        fn address(name: &CStr) -> Option<Proc> {
            let p = unsafe { wglGetProcAddress(name.as_ptr().cast()) }?;
            if matches!(p as usize, 0..=3 | usize::MAX) {
                None
            } else {
                Some(p)
            }
        }
        // WGL supplies these signatures for the named OpenGL framebuffer entry points.
        unsafe {
            Some(Self {
                active: std::mem::transmute::<Proc, Active>(address(c"glActiveTexture")?),
                buffer: std::mem::transmute::<Proc, Bind>(address(c"glBindBuffer")?),
                status: std::mem::transmute::<Proc, Status>(address(c"glCheckFramebufferStatus")?),
                bind: std::mem::transmute::<Proc, Bind>(address(c"glBindFramebuffer")?),
                gen_fb: std::mem::transmute::<Proc, Gen>(address(c"glGenFramebuffers")?),
                delete: std::mem::transmute::<Proc, Del>(address(c"glDeleteFramebuffers")?),
                attach: std::mem::transmute::<Proc, Attach>(address(c"glFramebufferTexture2D")?),
                blit: std::mem::transmute::<Proc, Blit>(address(c"glBlitFramebuffer")?),
            })
        }
    }
}
const READ: u32 = 0x8ca8;
const READ_BINDING: u32 = 0x8caa;
const COLOR: u32 = 0x8ce0;

const COMPLETE: u32 = 0x8cd5;
const ACTIVE_TEXTURE: u32 = 0x84e0;
const TEXTURE0: u32 = 0x84c0;
const UNPACK_BUFFER: u32 = 0x88ec;
const UNPACK_BUFFER_BINDING: u32 = 0x88ef;

// Each queried parameter is a scalar GL integer.
fn integer(key: u32) -> i32 {
    let mut v = 0;
    unsafe {
        glGetIntegerv(key, &mut v);
    }
    v
}

pub(crate) struct Texture {
    gl: Gl,
    context: HGLRC,
    id: u32,
    fb: u32,
    allocated: bool,
}
impl Texture {
    pub fn new() -> Option<Self> {
        let context = unsafe { wglGetCurrentContext() };
        if context.is_null() {
            return None;
        }
        let gl = Gl::load()?;
        let mut tex = [0];
        xplm::generate_texture_numbers(&mut tex);
        let mut fb = 0;
        unsafe {
            (gl.gen_fb)(1, &mut fb);
        }
        let result = Self {
            gl,
            context,
            id: tex[0] as _,
            fb,
            allocated: false,
        };
        (result.id != 0 && fb != 0).then_some(result)
    }

    pub fn upload(&mut self, [w, h]: [u16; 2], frame: &[u8]) -> bool {
        if unsafe { wglGetCurrentContext() } != self.context
            || frame.len() != usize::from(w) * usize::from(h) * 4
        {
            return false;
        }
        let gl = &self.gl;
        // CPU input is tightly packed BGRA. Neutralize all 2-D unpack state,
        // including a caller's pixel buffer, before passing a process address.
        let active = integer(ACTIVE_TEXTURE);
        unsafe {
            (gl.active)(TEXTURE0);
        }
        let binding = integer(GL_TEXTURE_BINDING_2D);
        let buffer = integer(UNPACK_BUFFER_BINDING);
        xplm::bind_texture_2d(self.id as _, 0);
        unsafe {
            (gl.buffer)(UNPACK_BUFFER, 0);
            let keys = [
                GL_UNPACK_ROW_LENGTH,
                GL_UNPACK_SKIP_ROWS,
                GL_UNPACK_SKIP_PIXELS,
                GL_UNPACK_ALIGNMENT,
            ];
            let old = keys.map(integer);
            for (k, v) in keys.into_iter().zip([0, 0, 0, 1]) {
                glPixelStorei(k, v);
            }
            if self.allocated {
                glTexSubImage2D(
                    GL_TEXTURE_2D,
                    0,
                    0,
                    0,
                    i32::from(w),
                    i32::from(h),
                    0x80e1,
                    GL_UNSIGNED_BYTE,
                    frame.as_ptr().cast(),
                );
            } else {
                for key in [GL_TEXTURE_MIN_FILTER, GL_TEXTURE_MAG_FILTER] {
                    glTexParameteri(GL_TEXTURE_2D, key, GL_NEAREST as _);
                }
                glTexImage2D(
                    GL_TEXTURE_2D,
                    0,
                    GL_RGBA8 as _,
                    i32::from(w),
                    i32::from(h),
                    0,
                    0x80e1,
                    GL_UNSIGNED_BYTE,
                    frame.as_ptr().cast(),
                );
                let prev = integer(READ_BINDING);
                (gl.bind)(READ, self.fb);
                (gl.attach)(READ, COLOR, GL_TEXTURE_2D, self.id, 0);
                glReadBuffer(COLOR);
                self.allocated = (gl.status)(READ) == COMPLETE;
                (gl.bind)(READ, prev as _);
            }
            for (k, v) in keys.into_iter().zip(old) {
                glPixelStorei(k, v);
            }
            (gl.buffer)(UNPACK_BUFFER, buffer as _);
            xplm::bind_texture_2d(binding, 0);
            (gl.active)(active as _);
        }
        self.allocated
    }

    pub fn blit(&self, source: [u16; 4], dest: [u16; 4]) -> bool {
        if unsafe { wglGetCurrentContext() } != self.context {
            return false;
        }
        let mut vp = [0; 4];
        unsafe {
            glGetIntegerv(GL_VIEWPORT, vp.as_mut_ptr());
        }
        let Some([x0, y0, x1, y1]) = viewport(dest, vp) else {
            return false;
        };
        let [l, t, r, b] = source.map(i32::from);
        let prev = integer(READ_BINDING);
        unsafe {
            (self.gl.bind)(READ, self.fb);
            (self.gl.blit)(l, t, r, b, x0, y0, x1, y1, GL_COLOR_BUFFER_BIT, GL_NEAREST);
            (self.gl.bind)(READ, prev as _);
        }
        true
    }
}
impl Drop for Texture {
    fn drop(&mut self) {
        // Framebuffer names are local to the creating context.
        if unsafe { wglGetCurrentContext() } == self.context {
            unsafe {
                (self.gl.delete)(1, &self.fb);
                glDeleteTextures(1, &self.id);
            }
        }
    }
}
