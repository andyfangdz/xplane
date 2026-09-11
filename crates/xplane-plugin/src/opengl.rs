//! Scoped compatibility-OpenGL drawing for native callbacks.
//!
//! Enter once at the callback boundary. The borrowed context exposes value-only
//! drawing operations; private guards balance stacks and primitives, including
//! early returns and unwinding. XPLM graphics-state selection stays separate.
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use windows_sys::Win32::Graphics::OpenGL::*;

thread_local! { static DRAWING: Cell<bool> = const { Cell::new(false) }; }

#[derive(Clone, Copy)]
pub enum Attribute {
    Current,
    Enable,
    Scissor,
    Line,
}
#[derive(Clone, Copy)]
pub enum Matrix {
    Modelview,
    Projection,
}
#[derive(Clone, Copy)]
pub enum Capability {
    Scissor,
    Stencil,
    CullFace,
    AlphaTest,
    Blend,
}
#[derive(Clone, Copy)]
pub enum Primitive {
    Quads,
    Polygon,
}

/// A context borrowed for one synchronous drawing callback. It cannot be
/// constructed, cloned, or transferred to another thread by safe client code.
pub struct DrawContext {
    _thread: PhantomData<Rc<()>>,
}

impl DrawContext {
    /// Borrows the current context, or returns `None` if no context is current
    /// or a drawing scope is already active on this thread.
    ///
    /// # Safety
    /// The caller must be outside glBegin/glEnd in a compatibility context and
    /// keep that context current until this call returns. Foreign GL operations
    /// must not disturb an active scope's stacks or primitive. In X-Plane this
    /// entry belongs in its native draw callback, not in scene-building code.
    ///
    /// The context cannot escape the callback:
    /// ```compile_fail
    /// use xplane_plugin::opengl::DrawContext;
    /// let escaped = unsafe { DrawContext::with_current(|draw| draw) };
    /// ```
    pub unsafe fn with_current<R>(draw: impl FnOnce(&mut Self) -> R) -> Option<R> {
        // SAFETY: querying the thread's current context does not dereference it.
        if unsafe { wglGetCurrentContext() }.is_null() || DRAWING.with(Cell::get) {
            return None;
        }
        DRAWING.with(|active| active.set(true));
        struct Active;
        impl Drop for Active {
            fn drop(&mut self) {
                DRAWING.with(|active| active.set(false));
            }
        }
        let _active = Active;
        Some(draw(&mut Self {
            _thread: PhantomData,
        }))
    }

    /// Restores the selected groups after the callback. A full GL stack returns
    /// `None` without drawing or popping another owner's entry.
    pub fn attributes<R>(
        &mut self,
        groups: &[Attribute],
        draw: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        if integer(GL_ATTRIB_STACK_DEPTH) >= integer(GL_MAX_ATTRIB_STACK_DEPTH) {
            return None;
        }
        let mask = groups.iter().fold(0, |mask, group| {
            mask | match group {
                Attribute::Current => GL_CURRENT_BIT,
                Attribute::Enable => GL_ENABLE_BIT,
                Attribute::Scissor => GL_SCISSOR_BIT,
                Attribute::Line => GL_LINE_BIT,
            }
        });
        // SAFETY: the context is exclusively borrowed, mask is valid, and the
        // checked push is balanced by a private guard that clients cannot forget.
        unsafe { glPushAttrib(mask) };
        let _restore = Restore::Attributes;
        Some(draw(self))
    }

    /// Restores the matrix and previous mode after drawing. Returns `None` when
    /// the selected stack is full, without changing either stack or mode.
    pub fn matrix<R>(&mut self, matrix: Matrix, draw: impl FnOnce(&mut Self) -> R) -> Option<R> {
        let (mode, depth, maximum) = match matrix {
            Matrix::Modelview => (
                GL_MODELVIEW,
                GL_MODELVIEW_STACK_DEPTH,
                GL_MAX_MODELVIEW_STACK_DEPTH,
            ),
            Matrix::Projection => (
                GL_PROJECTION,
                GL_PROJECTION_STACK_DEPTH,
                GL_MAX_PROJECTION_STACK_DEPTH,
            ),
        };
        if integer(depth) >= integer(maximum) {
            return None;
        }
        let previous = integer(GL_MATRIX_MODE) as u32;
        // SAFETY: the mode and available stack space are checked; the guard
        // selects this same stack before popping, then restores the previous mode.
        unsafe {
            glMatrixMode(mode);
            glPushMatrix();
        }
        let _restore = Restore::Matrix { mode, previous };
        Some(draw(self))
    }

    /// Draws a primitive through a restricted vertex interface. Matrix, texture
    /// binding, clipping, and nested glBegin calls are unavailable until it ends.
    /// ```compile_fail
    /// use xplane_plugin::opengl::{DrawContext, Primitive};
    /// fn draw(context: &mut DrawContext) {
    ///     context.vertices(Primitive::Quads, |_| context.load_identity());
    /// }
    /// ```
    pub fn vertices<R>(
        &mut self,
        primitive: Primitive,
        draw: impl FnOnce(&mut Vertices) -> R,
    ) -> R {
        let mode = match primitive {
            Primitive::Quads => GL_QUADS,
            Primitive::Polygon => GL_POLYGON,
        };
        // SAFETY: exclusive borrowing prevents nested primitives through this
        // API. Clients cannot construct or keep the private end guard.
        unsafe { glBegin(mode) };
        let _restore = Restore::Primitive;
        draw(&mut Vertices {
            _thread: PhantomData,
        })
    }

    pub fn color(&mut self, color: [f32; 4]) {
        // SAFETY: value-only operation in the borrowed context, outside glBegin.
        unsafe { glColor4f(color[0], color[1], color[2], color[3]) };
    }

    pub fn enable(&mut self, capability: Capability, enabled: bool) {
        let capability = match capability {
            Capability::Scissor => GL_SCISSOR_TEST,
            Capability::Stencil => GL_STENCIL_TEST,
            Capability::CullFace => GL_CULL_FACE,
            Capability::AlphaTest => GL_ALPHA_TEST,
            Capability::Blend => GL_BLEND,
        };
        // SAFETY: a supported capability in the borrowed context, outside glBegin.
        unsafe {
            if enabled {
                glEnable(capability);
            } else {
                glDisable(capability);
            }
        }
    }

    pub fn disable_clip_planes(&mut self) {
        // SAFETY: compatibility contexts support at least these six clip planes.
        unsafe {
            for i in 0..6 {
                glDisable(GL_CLIP_PLANE0 + i);
            }
        }
    }

    pub fn viewport(&self) -> [i32; 4] {
        let mut viewport = [0; 4];
        // SAFETY: this query writes exactly four integers to a live local array.
        unsafe { glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr()) };
        viewport
    }

    pub fn scissor(&mut self, [x, y, width, height]: [i32; 4]) {
        // SAFETY: dimensions are nonnegative and this is outside glBegin.
        unsafe { glScissor(x, y, width.max(0), height.max(0)) };
    }

    pub fn load_identity(&mut self) {
        // SAFETY: value-only matrix operation outside glBegin.
        unsafe { glLoadIdentity() };
    }

    pub fn ortho(&mut self, width: f64, height: f64) {
        // SAFETY: value-only matrix operation outside glBegin.
        unsafe { glOrtho(0.0, width, 0.0, height, -1.0, 1.0) };
    }

    pub fn translate(&mut self, x: f64, y: f64) {
        // SAFETY: value-only matrix operation outside glBegin.
        unsafe { glTranslated(x, y, 0.0) };
    }

    pub fn scale(&mut self, x: f64, y: f64) {
        // SAFETY: value-only matrix operation outside glBegin.
        unsafe { glScaled(x, y, 1.0) };
    }

    pub fn rotate(&mut self, degrees: f64) {
        // SAFETY: value-only matrix operation outside glBegin.
        unsafe { glRotated(degrees, 0.0, 0.0, 1.0) };
    }

    pub fn delete_texture(&mut self, texture: u32) {
        // SAFETY: GL reads one texture name from a live local value.
        unsafe { glDeleteTextures(1, &texture) };
    }
}

/// Only immediate-mode vertex operations are available inside a primitive.
pub struct Vertices {
    _thread: PhantomData<Rc<()>>,
}

impl Vertices {
    pub fn vertex(&mut self, x: f64, y: f64) {
        // SAFETY: only constructed and borrowed between the paired begin/end.
        unsafe { glVertex2d(x, y) };
    }
    pub fn tex_coord(&mut self, u: f64, v: f64) {
        // SAFETY: texture coordinates are legal within the active primitive.
        unsafe { glTexCoord2d(u, v) };
    }
}

fn integer(parameter: u32) -> i32 {
    let mut value = 0;
    // SAFETY: private callers only supply queries that return one integer.
    unsafe { glGetIntegerv(parameter, &mut value) };
    value
}

enum Restore {
    Attributes,
    Matrix { mode: u32, previous: u32 },
    Primitive,
}

impl Drop for Restore {
    fn drop(&mut self) {
        // SAFETY: these guards are private, created only after successful pushes
        // or begins, and dropped in LIFO order while the borrowed context is live.
        unsafe {
            match *self {
                Self::Attributes => glPopAttrib(),
                Self::Matrix { mode, previous } => {
                    glMatrixMode(mode);
                    glPopMatrix();
                    glMatrixMode(previous);
                }
                Self::Primitive => glEnd(),
            }
        }
    }
}
