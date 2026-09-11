use poweroff180::hud::*;
#[test]
fn camera_projection_director_and_rolling_digits() {
    let p = project(90.0, 0.0, 90.0, 0.0, 0.0, 75.0);
    assert!(p.visible);
    assert_eq!(p.point, point(960.0, 540.0));
    assert!(!project(270.0, 0.0, 90.0, 0.0, 0.0, 75.0).visible);
    assert!(project(100.0, 0.0, 90.0, 0.0, 0.0, 75.0).point.x > 960.0);
    assert!(project(90.0, 0.0, 90.0, 10.0, 0.0, 75.0).point.y > 540.0);
    let fd = director(10.0, 0.0, 5.0, 0.0);
    assert_eq!(fd.roll, 10.0);
    assert!(fd.center.y < 540.0);
    let carry = drum(99.5, 100, 1);
    assert_eq!(carry.digit, 0);
    assert_eq!(carry.fraction, 0.5);
    assert_eq!(drum(100.0, 100, 1).digit, 1);
    assert_eq!(drum(990.0, 1000, 20).fraction, 0.5);
    assert_eq!(tape_y(100.0, 100.0, 80.0), 540.0);
    assert_eq!(cdi_offset(3.0), 60.0);
}
#[test]
fn trends_wrap_heading_reset_on_rewind_and_label_tailwind() {
    let mut t = Trend::default();
    t.update(0.0, 100.0, 359.0);
    t.update(1.0, 102.0, 1.0);
    assert!(t.turn_rate > 0.0 && t.turn_rate < 2.0);
    assert!(t.acceleration > 0.0);
    t.update(0.0, 100.0, 0.0);
    assert_eq!(t.turn_rate, 0.0);
    assert_eq!(wind_label(5.0, 180.0), "5 KT TAILWIND");
    assert_eq!(wind_label(10.0, -90.0), "10 KT LEFT CROSSWIND");
}
