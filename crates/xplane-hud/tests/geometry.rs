use xplane_hud::{point, rotate, Point, Segment, View};

fn near(actual: Point, expected: Point) {
    assert!(
        (actual.x - expected.x).abs() < 1e-8,
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual.y - expected.y).abs() < 1e-8,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn projection_respects_independent_focal_lengths_and_roll() {
    let view = View {
        center: point(960.0, 300.0),
        fx: 1000.0,
        fy: 800.0,
        heading: 359.0,
        ..View::default()
    };
    near(view.project(359.0, 0.0).p, view.center);
    near(view.project(44.0, 0.0).p, point(1960.0, 300.0));
    near(view.project(359.0, 45.0).p, point(960.0, -500.0));
    near(
        View { roll: 90.0, ..view }.project(44.0, 0.0).p,
        point(960.0, -500.0),
    );
    near(
        View {
            pitch: -20.0,
            ..view
        }
        .project(359.0, -20.0)
        .p,
        view.center,
    );
    for turns in [-3.0, -1.0, 0.0, 2.0] {
        near(
            view.project(44.0 + turns * 360.0, 0.0).p,
            point(1960.0, 300.0),
        );
    }
}

#[test]
fn near_plane_marks_limited_rays_and_keeps_cage_coordinates() {
    let view = View {
        fx: 1000.0,
        fy: 800.0,
        ..View::default()
    };
    for (forward, limited) in [(0.0100001_f64, false), (0.0099999, true), (-0.1, true)] {
        let result = view.project(forward.acos().to_degrees(), 0.0);
        assert_eq!(result.limited, limited);
        assert!(result.p.x.is_finite() && result.p.x > 99000.0);
        assert_eq!(result.p.y, 0.0);
    }
    assert!(view.project(180.0, 0.0).limited);
}

#[test]
fn camera_reprojection_preserves_rays_with_off_center_optics() {
    let body = View {
        fx: 1200.0,
        fy: 900.0,
        center: point(820.0, 420.0),
        heading: 355.0,
        pitch: -18.0,
        roll: 23.0,
    };
    let camera = View {
        fx: 800.0,
        fy: 1100.0,
        center: point(960.0, 540.0),
        heading: 8.0,
        pitch: -3.0,
        roll: -12.0,
    };
    for az in [330.0, 350.0, 5.0, 20.0] {
        for el in [-35.0, -10.0, 15.0] {
            let original = body.project(az, el);
            assert!(!original.limited);
            near(body.camera_point(body, original.p).p, original.p);
            let expected = camera.project(az, el);
            let actual = body.camera_point(camera, original.p);
            assert_eq!(actual.limited, expected.limited);
            near(actual.p, expected.p);
        }
    }
    assert!(
        body.camera_point(
            View {
                heading: 175.0,
                ..body
            },
            body.center
        )
        .limited
    );
}

#[test]
fn rotation_and_strokes_preserve_centers_lengths_and_widths() {
    let center = point(12.0, 34.0);
    near(rotate(point(22.0, 34.0), center, 90.0), point(12.0, 44.0));
    for b in [point(10.0, 0.0), point(0.0, 10.0), point(3.0, -4.0)] {
        let segment = Segment {
            a: point(0.0, 0.0),
            b,
        };
        let q = segment.quad(4.0).unwrap();
        near(
            point((q[0].x + q[3].x) / 2.0, (q[0].y + q[3].y) / 2.0),
            segment.a,
        );
        near(
            point((q[1].x + q[2].x) / 2.0, (q[1].y + q[2].y) / 2.0),
            segment.b,
        );
        assert!(((q[0].x - q[3].x).hypot(q[0].y - q[3].y) - 4.0).abs() < 1e-8);
        assert!(((q[1].x - q[0].x).hypot(q[1].y - q[0].y) - b.x.hypot(b.y)).abs() < 1e-8);
        let reverse = Segment { a: b, b: segment.a }.quad(4.0).unwrap();
        assert_eq!(reverse, [q[2], q[3], q[0], q[1]]);
    }
    for x in [0.0, 0.5e-9] {
        assert!(Segment {
            a: Point::default(),
            b: point(x, 0.0)
        }
        .quad(4.0)
        .is_none());
    }
    assert!(Segment {
        a: Point::default(),
        b: point(1e-9, 0.0)
    }
    .quad(4.0)
    .is_some());
}

#[test]
fn clipping_keeps_edges_and_corners_and_rejects_parallel_outside_lines() {
    let clip = |a, b| Segment { a, b }.clipped(0.0, 0.0, 10.0, 10.0);
    let inside = Segment {
        a: point(0.0, 5.0),
        b: point(10.0, 5.0),
    };
    assert_eq!(clip(point(-5.0, 5.0), point(15.0, 5.0)), Some(inside));
    assert_eq!(
        clip(point(15.0, 5.0), point(-5.0, 5.0)),
        Some(Segment {
            a: inside.b,
            b: inside.a
        })
    );
    assert!(clip(point(-5.0, -1.0), point(15.0, -1.0)).is_none());
    assert!(clip(point(11.0, -5.0), point(11.0, 15.0)).is_none());
    assert_eq!(
        clip(point(0.0, 0.0), point(0.0, 10.0)),
        Some(Segment {
            a: point(0.0, 0.0),
            b: point(0.0, 10.0)
        })
    );
    assert_eq!(
        clip(point(-5.0, -5.0), point(0.0, 0.0)),
        Some(Segment {
            a: Point::default(),
            b: Point::default()
        })
    );
    assert!(clip(point(15.0, 15.0), point(15.0, 15.0)).is_none());
}
