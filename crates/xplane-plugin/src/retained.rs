//! Owned SDK 4.4 retained panel commands.
use std::{cell::Cell, ffi::c_void, ptr::NonNull};
use xplane_sdk_sys as sdk;

thread_local! {
    static RECORDING: Cell<bool> = const { Cell::new(false) };
}

// End the recording in the same callback, including when its builder unwinds.
struct Session;
impl Session {
    fn begin() -> Option<Self> {
        if RECORDING.replace(true) {
            return None;
        }
        unsafe { sdk::XPLMBeginRetainedDrawing() };
        Some(Self)
    }
    fn finish(self) -> Option<NonNull<c_void>> {
        let handle = NonNull::new(unsafe { sdk::XPLMEndRetainedDrawing() });
        std::mem::forget(self);
        RECORDING.set(false);
        handle
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let unfinished = unsafe { sdk::XPLMEndRetainedDrawing() };
        if !unfinished.is_null() {
            unsafe { sdk::XPLMDestroyRetainedDrawing(unfinished) };
        }
        RECORDING.set(false);
    }
}

/// A recorded drawing and the owners of every font, atlas, texture and child recording it references.
/// Create, replay and drop on X-Plane's main thread.
pub struct RetainedDrawing<R = ()> {
    handle: NonNull<c_void>,
    _resources: R,
}

impl<R> RetainedDrawing<R> {
    /// Record in a simulator callback. Replay explicitly with [`Self::draw`].
    /// Returns `None` on a nested recording or allocation failure.
    ///
    /// # Safety
    /// All SDK resources drawn by `record` must remain live until this owner is
    /// dropped. Give their owners or shared owners to `resources`. A mutable
    /// owner that can destroy or replace its referenced handle is insufficient.
    /// Balance transform and scissor scopes in the closure. Use the safe
    /// [`RetainedDrawing::record_primitives`] for pure geometry.
    pub unsafe fn record(resources: R, record: impl FnOnce(&R)) -> Option<Self> {
        let session = Session::begin()?;
        record(&resources);
        let handle = session.finish()?;
        Some(Self {
            handle,
            _resources: resources,
        })
    }

    /// Replay in a panel draw callback under the current transform, clip and mask.
    pub fn draw(&self) {
        unsafe { sdk::XPLMDrawRetained(self.handle.as_ptr()) };
    }
}

impl<R> Drop for RetainedDrawing<R> {
    fn drop(&mut self) {
        // Release the command buffer before Rust drops any owners in R.
        unsafe { sdk::XPLMDestroyRetainedDrawing(self.handle.as_ptr()) }
    }
}

/// Geometry copied into a retained drawing. Coordinates and widths are in panel units.
pub enum RetainedPrimitive<'a> {
    Polygon {
        rgba: u32,
        vertices: &'a [sdk::XPLMVertex_t],
    },
    LineStrip {
        rgba: u32,
        width: f32,
        vertices: &'a [sdk::XPLMVertex_t],
    },
    ColoredQuadStrip(&'a [sdk::XPLMVertexColor_t]),
}

impl RetainedDrawing<()> {
    /// Record resource-free geometry on the simulator thread. The SDK copies all vertex arrays.
    pub fn record_primitives(primitives: &[RetainedPrimitive<'_>]) -> Option<Self> {
        let count = |n: usize| i32::try_from(n).ok();
        for p in primitives {
            let n = match p {
                RetainedPrimitive::Polygon { vertices, .. } => vertices.len(),
                RetainedPrimitive::LineStrip { vertices, .. } => vertices.len(),
                RetainedPrimitive::ColoredQuadStrip(vertices) => vertices.len(),
            };
            count(n)?;
        }
        // SAFETY: these commands have no referenced resources and the slice is live during recording.
        unsafe {
            Self::record((), |_| {
                for p in primitives {
                    match p {
                        RetainedPrimitive::Polygon { rgba, vertices } => {
                            sdk::XPLMPolygon(*rgba, vertices.as_ptr(), vertices.len() as i32)
                        }
                        RetainedPrimitive::LineStrip {
                            rgba,
                            width,
                            vertices,
                        } => sdk::XPLMLineStripWithWidth(
                            *rgba,
                            *width,
                            vertices.as_ptr(),
                            vertices.len() as i32,
                        ),
                        RetainedPrimitive::ColoredQuadStrip(vertices) => {
                            sdk::XPLMQuadstripc(vertices.as_ptr(), vertices.len() as i32)
                        }
                    }
                }
            })
        }
    }
}
