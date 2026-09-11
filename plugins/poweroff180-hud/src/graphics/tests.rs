use super::*;
use windows_sys::Win32::Graphics::OpenGL::*;

#[test]
fn nested_clip_pixels_and_malformed_commands_preserve_the_gl_stacks() {
    let _context = crate::gl_test_support::Context::new();
    let graphics = Graphics::default();
    let clip = Draw::Clip {
        x: 0.0,
        y: 0.0,
        w: 960.0,
        h: 540.0,
    };
    let commands = [
        clip.clone(),
        Draw::Clip {
            x: 480.0,
            y: 270.0,
            w: 480.0,
            h: 270.0,
        },
        Draw::Polygon(
            vec![
                point(0.0, 0.0),
                point(1920.0, 0.0),
                point(1920.0, 1080.0),
                point(0.0, 1080.0),
            ],
            [0.0, 1.0, 0.0, 1.0],
            true,
            1.0,
        ),
        Draw::Unclip,
        Draw::Unclip,
    ];
    // SAFETY: the test owns the context and a 64*64 bitmap. Raw setup/readback
    // occur outside drawing; all buffers match the corresponding GL query.
    unsafe {
        glViewport(0, 0, 64, 64);
        glDrawBuffer(GL_BACK);
        glReadBuffer(GL_BACK);
        glMatrixMode(GL_PROJECTION);
        glLoadIdentity();
        glOrtho(0.0, 1920.0, 0.0, 1080.0, -1.0, 1.0);
        glMatrixMode(GL_MODELVIEW);
        glLoadIdentity();
        glTranslated(0.0, 1080.0, 0.0);
        glScaled(1.0, -1.0, 1.0);
        glDisable(GL_DITHER);
        glDisable(GL_SCISSOR_TEST);
        glScissor(3, 7, 10, 12);
        glClearColor(0.0, 0.0, 0.0, 1.0);
        glClear(GL_COLOR_BUFFER_BIT);
        DrawContext::with_current(|gl| {
            assert!(graphics
                .commands(gl, &mut commands.iter(), [0, 0, 64, 64], false)
                .is_some());
            let mut pixels = vec![0_u8; 64 * 64 * 4];
            glReadPixels(
                0,
                0,
                64,
                64,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                pixels.as_mut_ptr().cast(),
            );
            for y in 0..64 {
                for x in 0..64 {
                    let expected = if (16..32).contains(&x) && (32..48).contains(&y) {
                        255
                    } else {
                        0
                    };
                    assert_eq!(pixels[(y * 64 + x) * 4 + 1], expected, "pixel ({x}, {y})");
                }
            }
            for malformed in [vec![Draw::Unclip], vec![clip.clone()], vec![clip; 256]] {
                assert!(graphics
                    .commands(gl, &mut malformed.iter(), [0, 0, 64, 64], false)
                    .is_none());
                let mut depth = -1;
                glGetIntegerv(GL_ATTRIB_STACK_DEPTH, &mut depth);
                assert_eq!(depth, 0);
                glGetIntegerv(GL_MODELVIEW_STACK_DEPTH, &mut depth);
                assert_eq!(depth, 1);
                let mut scissor = [0; 4];
                glGetIntegerv(GL_SCISSOR_BOX, scissor.as_mut_ptr());
                assert_eq!(scissor, [3, 7, 10, 12]);
                assert_eq!(glIsEnabled(GL_SCISSOR_TEST), 0);
            }
        })
        .unwrap();
        assert_eq!(glGetError(), GL_NO_ERROR);
    }
}
