use super::*;
#[test]
fn nested_tape_clips_intersect_in_the_designs_y_down_coordinates() {
    assert_eq!(
        intersect(intersect(FIELD, [0, 0, 960, -540]), [480, -270, 1440, -810]),
        [480, -270, 960, -540]
    );
    let [left, top, right, bottom] = intersect([10, 20, 30, 0], [40, -5, 60, -20]);
    assert_eq!((right - left, top - bottom), (0, 0));
}
#[test]
fn sdk_tints_use_straight_rgba() {
    assert_eq!(
        color([0.1, 0.5, 1., 0.25]).to_le_bytes(),
        [26, 128, 255, 64]
    );
}

#[test]
fn nested_clips_affect_pixels_and_restore_the_parent_transform() {
    let area = Draw::Clip {
        x: 0.,
        y: 0.,
        w: 960.,
        h: 540.,
    };
    let commands = [
        area.clone(),
        Draw::Clip {
            x: 480.,
            y: 270.,
            w: 900.,
            h: 600.,
        },
        Draw::Line(point(0., 0.), point(1., 1.), [1.; 4], 1.),
        Draw::Unclip,
        Draw::Line(point(0., 0.), point(1., 1.), [1.; 4], 1.),
        Draw::Unclip,
    ];
    let mut limits = Vec::new();
    assert!(draw_clipped(&commands, |_, clip| limits.push(clip)));
    assert_eq!(limits, [[480, -270, 960, -540], [0, 0, 960, -540]]);
    for row in 0..64 {
        for col in 0..64 {
            let x = (col as f64 + 0.5) * 1920. / 64.;
            let y = -(row as f64 + 0.5) * 1080. / 64.;
            let [l, t, r, b] = limits[0].map(f64::from);
            assert_eq!(
                x >= l && x < r && y <= t && y > b,
                (16..32).contains(&col) && (16..32).contains(&row)
            );
        }
    }
    for malformed in [vec![Draw::Unclip], vec![area.clone()], vec![area; 256]] {
        assert!(!draw_clipped(&malformed, |_, _| panic!(
            "only clip commands"
        )));
    }
}
