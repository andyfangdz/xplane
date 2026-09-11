//! Scoped compatibility-OpenGL state for native draw callbacks.
//!
//! These guards restore raw GL stacks, not XPLM's cached graphics state.
//! Set the SDK graphics state before capturing them, and keep drawing policy
//! (projection transforms, clipping, blending, textures) in each renderer.
use std::{marker::PhantomData, rc::Rc};
use windows_sys::Win32::Graphics::OpenGL::*;

/// Restores selected GL attributes on scope exit. Bound to the creating thread.
#[must_use = "keep the guard alive until drawing is complete"]
pub struct AttributeGuard {
    _thread: PhantomData<Rc<()>>,
}

impl AttributeGuard {
    /// Pushes the groups selected by a valid `glPushAttrib` mask.
    ///
    /// # Safety
    /// A compatibility GL context with attribute stack space must stay current
    /// on this thread until drop. Do not call within glBegin/glEnd. Nested
    /// pushes must be balanced before drop; do not pop this guard's entry.
    pub unsafe fn push(mask: u32) -> Self {
        // SAFETY: the caller provides the context, mask, and stack lifetime.
        unsafe { glPushAttrib(mask) };
        Self {
            _thread: PhantomData,
        }
    }
}

impl Drop for AttributeGuard {
    fn drop(&mut self) {
        // SAFETY: construction requires the same current context and LIFO use.
        unsafe { glPopAttrib() };
    }
}

/// Restores a matrix stack and the previous matrix mode on scope exit.
#[must_use = "keep the guard alive until drawing is complete"]
pub struct MatrixGuard {
    mode: u32,
    previous_mode: u32,
    _thread: PhantomData<Rc<()>>,
}

impl MatrixGuard {
    /// Selects and pushes the modelview matrix without changing its contents.
    ///
    /// # Safety
    /// A compatibility GL context with matrix stack space must stay current
    /// on this thread until drop. Do not call within glBegin/glEnd. Balance any
    /// nested pushes before drop and do not pop this guard's entry.
    pub unsafe fn modelview() -> Self {
        // SAFETY: forwarded context and stack preconditions.
        unsafe { Self::push(GL_MODELVIEW) }
    }

    /// Selects and pushes the projection matrix without changing its contents.
    ///
    /// # Safety
    /// The context and stack requirements of [`Self::modelview`] apply.
    pub unsafe fn projection() -> Self {
        // SAFETY: forwarded context and stack preconditions.
        unsafe { Self::push(GL_PROJECTION) }
    }

    unsafe fn push(mode: u32) -> Self {
        let mut previous_mode = GL_MODELVIEW as i32;
        // SAFETY: caller guarantees an active context and available stack space.
        unsafe {
            glGetIntegerv(GL_MATRIX_MODE, &mut previous_mode);
            glMatrixMode(mode);
            glPushMatrix();
        }
        Self {
            mode,
            previous_mode: previous_mode as u32,
            _thread: PhantomData,
        }
    }
}

impl Drop for MatrixGuard {
    fn drop(&mut self) {
        // SAFETY: construction requires this context and balanced nested use.
        // Select our stack explicitly even if drawing changed the matrix mode.
        unsafe {
            glMatrixMode(self.mode);
            glPopMatrix();
            glMatrixMode(self.previous_mode);
        }
    }
}
