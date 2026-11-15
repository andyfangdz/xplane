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

/// Methods of [`DrawContext`] for the class of GL calls that take only plain
/// numbers (no pointer) and are legal on the current compatibility context
/// outside `glBegin`/`glEnd`: the context is exclusively borrowed for one
/// drawing scope, which `with_current` enters only outside a primitive and
/// which `vertices` reopens only after its primitive ends. An out-of-range
/// value is a GL error, not memory access. Members: `glColor4f`,
/// `glScissor`, `glLoadIdentity`, `glOrtho`, `glTranslated`, `glScaled`,
/// `glRotated`, `glEnable`, `glDisable`.
macro_rules! value_calls {
    ($($vis:vis fn $name:ident(&mut self $(, $arg:tt: $ty:ty)*) => $call:expr;)+) => {$(
        $vis fn $name(&mut self $(, $arg: $ty)*) {
            // SAFETY: a value call (macro note): plain numbers on the borrowed current context,
            // outside glBegin.
            unsafe { $call };
        }
    )+};
}

/// Methods of [`Vertices`] for the class of GL calls that take only plain
/// numbers and are legal between `glBegin` and `glEnd`: a `Vertices` exists
/// only as the borrow [`DrawContext::vertices`] hands out between its begin
/// and the end its private guard makes. Members: `glVertex2d`, `glTexCoord2d`.
macro_rules! primitive_calls {
    ($(pub fn $name:ident(&mut self $(, $arg:ident: $ty:ty)*) => $call:expr;)+) => {$(
        pub fn $name(&mut self $(, $arg: $ty)*) {
            // SAFETY: a primitive call (macro note): plain numbers inside the active primitive.
            unsafe { $call };
        }
    )+};
}

/// A `glGetIntegerv` query that writes exactly `N` integers. Only the
/// constants below exist, so [`query`] always passes a buffer of the size the
/// query fills.
struct Query<const N: usize>(u32);

const VIEWPORT: Query<4> = Query(GL_VIEWPORT);
const MATRIX_MODE: Query<1> = Query(GL_MATRIX_MODE);
const ATTRIB_STACK_DEPTH: Query<1> = Query(GL_ATTRIB_STACK_DEPTH);
const MAX_ATTRIB_STACK_DEPTH: Query<1> = Query(GL_MAX_ATTRIB_STACK_DEPTH);
const MODELVIEW_STACK_DEPTH: Query<1> = Query(GL_MODELVIEW_STACK_DEPTH);
const MAX_MODELVIEW_STACK_DEPTH: Query<1> = Query(GL_MAX_MODELVIEW_STACK_DEPTH);
const PROJECTION_STACK_DEPTH: Query<1> = Query(GL_PROJECTION_STACK_DEPTH);
const MAX_PROJECTION_STACK_DEPTH: Query<1> = Query(GL_MAX_PROJECTION_STACK_DEPTH);

fn query<const N: usize>(query: Query<N>) -> [i32; N] {
    let mut values = [0; N];
    // SAFETY: `query` writes exactly `N` integers (type note) to this live local array.
    unsafe { glGetIntegerv(query.0, values.as_mut_ptr()) };
    values
}

fn integer(parameter: Query<1>) -> i32 {
    query(parameter)[0]
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
        if integer(ATTRIB_STACK_DEPTH) >= integer(MAX_ATTRIB_STACK_DEPTH) {
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
                MODELVIEW_STACK_DEPTH,
                MAX_MODELVIEW_STACK_DEPTH,
            ),
            Matrix::Projection => (
                GL_PROJECTION,
                PROJECTION_STACK_DEPTH,
                MAX_PROJECTION_STACK_DEPTH,
            ),
        };
        if integer(depth) >= integer(maximum) {
            return None;
        }
        let previous = integer(MATRIX_MODE) as u32;
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

    value_calls! {
        pub fn color(&mut self, color: [f32; 4]) =>
            glColor4f(color[0], color[1], color[2], color[3]);
    }

    pub fn enable(&mut self, capability: Capability, enabled: bool) {
        let capability = match capability {
            Capability::Scissor => GL_SCISSOR_TEST,
            Capability::Stencil => GL_STENCIL_TEST,
            Capability::CullFace => GL_CULL_FACE,
            Capability::AlphaTest => GL_ALPHA_TEST,
            Capability::Blend => GL_BLEND,
        };
        if enabled {
            self.gl_enable(capability);
        } else {
            self.gl_disable(capability);
        }
    }

    /// Compatibility contexts support at least these six clip planes.
    pub fn disable_clip_planes(&mut self) {
        for i in 0..6 {
            self.gl_disable(GL_CLIP_PLANE0 + i);
        }
    }

    value_calls! {
        fn gl_enable(&mut self, capability: u32) => glEnable(capability);
        fn gl_disable(&mut self, capability: u32) => glDisable(capability);
    }

    pub fn viewport(&self) -> [i32; 4] {
        query(VIEWPORT)
    }

    value_calls! {
        pub fn scissor(&mut self, [x, y, width, height]: [i32; 4]) =>
            glScissor(x, y, width.max(0), height.max(0));
        pub fn load_identity(&mut self) => glLoadIdentity();
        pub fn ortho(&mut self, width: f64, height: f64) =>
            glOrtho(0.0, width, 0.0, height, -1.0, 1.0);
        pub fn translate(&mut self, x: f64, y: f64) => glTranslated(x, y, 0.0);
        pub fn scale(&mut self, x: f64, y: f64) => glScaled(x, y, 1.0);
        pub fn rotate(&mut self, degrees: f64) => glRotated(degrees, 0.0, 0.0, 1.0);
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
    primitive_calls! {
        pub fn vertex(&mut self, x: f64, y: f64) => glVertex2d(x, y);
        pub fn tex_coord(&mut self, u: f64, v: f64) => glTexCoord2d(u, v);
    }
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
