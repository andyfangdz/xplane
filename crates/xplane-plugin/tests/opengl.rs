//! Uses a real compatibility context in an invisible window; no simulator needed.
#![cfg(all(windows, feature = "opengl"))]
mod support;
use std::panic::{catch_unwind, AssertUnwindSafe};
use support::Context;
use windows_sys::Win32::Graphics::OpenGL::*;
use xplane_plugin::opengl::{Attribute, Capability, DrawContext, Matrix, Primitive};

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
fn drawing_scopes_restore_state_and_reject_stack_overflow() {
    let _context = Context::new();
    // SAFETY: this test owns the current context; raw setup and readback remain
    // outside primitives and do not alter any scope's stack entries.
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
        DrawContext::with_current::<()>(|gl| {
            assert!(DrawContext::with_current::<()>(|_| panic!("reentrant entry")).is_none());
            gl.attributes(
                &[Attribute::Line, Attribute::Current, Attribute::Scissor],
                |gl| {
                    gl.enable(Capability::Scissor, false);
                    gl.matrix(Matrix::Modelview, |gl| {
                        gl.translate(0.0, 1080.0);
                        gl.scale(1.0, -1.0);
                        let inside = state();
                        gl.attributes(&[Attribute::Scissor], |gl| {
                            gl.enable(Capability::Scissor, true);
                            gl.scissor([10, 20, 100, 200]);
                            gl.matrix(Matrix::Modelview, |gl| gl.rotate(30.0)).unwrap();
                        })
                        .unwrap();
                        assert_eq!(state(), inside);
                        gl.color([1.0, 0.0, 0.0, 0.5]);
                    })
                    .unwrap();
                },
            )
            .unwrap();
            assert_eq!(state(), before);
            for panel in [false, true] {
                gl.attributes(
                    &[Attribute::Current, Attribute::Scissor, Attribute::Enable],
                    |gl| {
                        gl.matrix(Matrix::Projection, |gl| {
                            if !panel {
                                gl.load_identity();
                                gl.ortho(1920.0, 1080.0);
                            }
                            gl.matrix(Matrix::Modelview, |gl| {
                                gl.translate(2500.0, 1200.0);
                                gl.scale(0.5, -0.5);
                                gl.enable(Capability::Blend, false);
                                gl.enable(Capability::Scissor, false);
                                gl.color([0.0, 1.0, 0.0, 0.8]);
                                if panel {
                                    return;
                                }
                                gl.load_identity();
                            })
                            .unwrap();
                        })
                        .unwrap();
                    },
                )
                .unwrap();
                assert_eq!(state(), before);
            }
            fn fill_attributes(gl: &mut DrawContext) -> usize {
                gl.attributes(&[Attribute::Scissor], fill_attributes)
                    .map_or(0, |depth| depth + 1)
            }
            fn fill_matrix(gl: &mut DrawContext, matrix: Matrix) -> usize {
                gl.matrix(matrix, |gl| fill_matrix(gl, matrix))
                    .map_or(0, |depth| depth + 1)
            }
            assert!(fill_attributes(gl) >= 1);
            assert!(fill_matrix(gl, Matrix::Projection) >= 1);
            assert!(fill_matrix(gl, Matrix::Modelview) >= 1);
        })
        .unwrap();
        assert_eq!(state(), before);
        assert_eq!(glGetError(), GL_NO_ERROR);
    }
}

#[test]
fn unwinding_ends_the_primitive_restores_stacks_and_releases_the_context() {
    let _context = Context::new();
    // SAFETY: raw reads are outside drawing; the owned context remains current
    // across the intentional Rust unwind, including every guard's destructor.
    unsafe {
        let before = state();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            DrawContext::with_current::<()>(|gl| {
                gl.attributes(&[Attribute::Current], |gl| {
                    gl.color([1.0, 0.0, 0.0, 0.5]);
                    gl.matrix(Matrix::Modelview, |gl| {
                        gl.translate(1.0, 2.0);
                        gl.vertices(Primitive::Quads, |vertices| {
                            vertices.vertex(0.0, 0.0);
                            panic!("exercise primitive cleanup");
                        });
                    });
                });
            });
        }))
        .is_err());
        assert_eq!(state(), before);
        DrawContext::with_current::<()>(|gl| {
            gl.vertices(Primitive::Quads, |_| {});
        })
        .unwrap();
        assert_eq!(glGetError(), GL_NO_ERROR);
    }
}

#[test]
fn safe_primitives_match_raw_gl_pixels_under_scissoring() {
    let _context = Context::new();
    // SAFETY: the context is current, drawing is to its private back buffer,
    // and readback allocates exactly 64*64 RGBA bytes with default packing.
    unsafe {
        glViewport(0, 0, 64, 64);
        glDrawBuffer(GL_BACK);
        glReadBuffer(GL_BACK);
        glMatrixMode(GL_PROJECTION);
        glLoadIdentity();
        glOrtho(0.0, 64.0, 0.0, 64.0, -1.0, 1.0);
        glMatrixMode(GL_MODELVIEW);
        glLoadIdentity();
        glDisable(GL_DITHER);
        glClearColor(0.0, 0.0, 0.0, 1.0);
        let read = || {
            let mut bytes = vec![0_u8; 64 * 64 * 4];
            glReadPixels(
                0,
                0,
                64,
                64,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                bytes.as_mut_ptr().cast(),
            );
            bytes
        };
        glClear(GL_COLOR_BUFFER_BIT);
        glPushAttrib(GL_CURRENT_BIT | GL_SCISSOR_BIT);
        glEnable(GL_SCISSOR_TEST);
        glScissor(8, 12, 16, 20);
        glColor4f(0.0, 1.0, 0.0, 1.0);
        glBegin(GL_QUADS);
        for (x, y) in [(0.0, 0.0), (48.0, 0.0), (48.0, 48.0), (0.0, 48.0)] {
            glVertex2d(x, y);
        }
        glEnd();
        glPopAttrib();
        let expected = read();
        glClear(GL_COLOR_BUFFER_BIT);
        DrawContext::with_current::<()>(|gl| {
            gl.attributes(&[Attribute::Current, Attribute::Scissor], |gl| {
                gl.enable(Capability::Scissor, true);
                gl.scissor([8, 12, 16, 20]);
                gl.color([0.0, 1.0, 0.0, 1.0]);
                gl.vertices(Primitive::Quads, |vertices| {
                    for (x, y) in [(0.0, 0.0), (48.0, 0.0), (48.0, 48.0), (0.0, 48.0)] {
                        vertices.vertex(x, y);
                    }
                });
            })
            .unwrap();
        })
        .unwrap();
        assert_eq!(read(), expected);
        assert_eq!(
            expected
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[1] == 255)
                .count(),
            16 * 20
        );
        assert_eq!(glGetError(), GL_NO_ERROR);
    }
}

#[test]
fn absent_context_never_enters_drawing() {
    // SAFETY: no GL operations occur when no context is current on this thread.
    assert!(unsafe { DrawContext::with_current::<()>(|_| panic!("missing context")) }.is_none());
}
