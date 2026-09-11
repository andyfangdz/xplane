use super::*;
use poweroff180::{
    guidance::PI,
    hud::{director, project, rotate},
};
use uom::si::{angle::degree, f64::Velocity, velocity::knot, velocity::meter_per_second};
impl Hud {
    pub(super) fn attitude(&self, d: &mut Scene, s: &[f64; LENGTH], v: &Values) {
        let mut pitch = v.get("sim/graphics/view/view_pitch");
        let mut roll = v.get("sim/graphics/view/view_roll");
        let mut heading = v.get("sim/graphics/view/view_heading");
        if !pitch.is_finite() || !roll.is_finite() || !heading.is_finite() {
            pitch = s[field::PITCH_DEG];
            roll = s[field::BANK_DEG];
            heading = s[field::HEADING_TRUE_DEG];
        }
        let fov = v.get("sim/graphics/view/field_of_view_deg");
        let fov = if fov.is_finite() { fov } else { 75.0 };
        let project = |az, el| project(az, el, heading, pitch, roll, fov);
        d.clip(665.0, 300.0, 630.0, 425.0);
        let focal = 960.0 / rad(fov * 0.5).tan();
        const SCALE: f64 = 1.4;
        for half in -12_i32..=12 {
            let degrees = f64::from(half) * 2.5;
            let len = (if half == 0 {
                218.0
            } else if half % 4 == 0 {
                61.0
            } else if half % 2 == 0 {
                30.0
            } else {
                15.0
            }) * SCALE;
            let a = project(heading - (len / focal).atan() * 180.0 / PI, degrees);
            let b = project(heading + (len / focal).atan() * 180.0 / PI, degrees);
            if a.visible && b.visible {
                d.line(a.point, b.point, [0.86, 0.88, 0.85, 1.0], 2.8);
            }
            if half != 0 && half % 2 == 0 {
                for side in [-1.0, 1.0] {
                    let q = project(
                        heading + side * ((len + 16.0 * SCALE) / focal).atan() * 180.0 / PI,
                        degrees,
                    );
                    if q.visible && q.point.y > 318.0 && q.point.y < 700.0 {
                        d.text(
                            q.point.x,
                            q.point.y - 14.0,
                            num(degrees.abs()),
                            26.0,
                            WHITE,
                            1,
                        );
                    }
                }
            }
        }
        d.unclip();
        let fixed = |x: f64, y: f64| p(960.0 + x * SCALE, 540.0 + y * SCALE);
        let outline = [0.025, 0.025, 0.015, 1.0];
        let bank_white = [0.94, 0.95, 0.92, 1.0];
        let bank_gray = [0.76, 0.79, 0.76, 1.0];
        d.arc(960.0, 540.0, 216.0 * SCALE, -150.0, -30.0, bank_gray, 2.8);
        for degrees in [-60_i32, -45, -30, -20, -10, 10, 20, 30, 45, 60] {
            let a = rad(f64::from(degrees));
            let outer = if degrees.abs() == 60 || degrees.abs() == 30 {
                239.0
            } else {
                230.0
            };
            d.line(
                fixed(216.0 * a.sin(), -216.0 * a.cos()),
                fixed(outer * a.sin(), -outer * a.cos()),
                bank_gray,
                2.8,
            );
        }
        d.poly(
            &[
                fixed(-10.0, -240.0),
                fixed(10.0, -240.0),
                fixed(0.0, -219.0),
            ],
            bank_gray,
            true,
            2.0,
        );
        d.poly(
            &[fixed(-8.0, -238.0), fixed(8.0, -238.0), fixed(0.0, -221.0)],
            bank_white,
            true,
            2.0,
        );
        let bank_point = |x, y| rotate(fixed(x, y), p(960.0, 540.0), s[field::BANK_DEG]);
        d.poly(
            &[
                bank_point(0.0, -213.0),
                bank_point(-10.0, -191.0),
                bank_point(10.0, -191.0),
            ],
            bank_white,
            true,
            2.0,
        );
        let slip = (s[field::NATIVE_SLIP_DEG] / 8.53 * 19.0).clamp(-19.0, 19.0);
        d.poly(
            &[
                bank_point(slip - 10.0, -187.0),
                bank_point(slip + 10.0, -187.0),
                bank_point(slip + 13.0, -180.0),
                bank_point(slip - 13.0, -180.0),
            ],
            bank_white,
            true,
            2.0,
        );
        let fd = director(
            s[field::BANK_COMMAND],
            s[field::BANK_DEG],
            s[field::PITCH_COMMAND],
            s[field::PITCH_DEG],
        );
        let bar = |x, y| {
            rotate(
                p(fd.center.x + x * SCALE, fd.center.y + y * SCALE),
                fd.center,
                fd.roll,
            )
        };
        let pink = [1.0, 0.16, 1.0, 1.0];
        let cap = [0.86, 0.12, 0.88, 1.0];
        for side in [-1.0, 1.0] {
            d.poly(
                &[
                    bar(side * 158.0, 20.0),
                    bar(0.0, 0.0),
                    bar(side * 134.0, 32.0),
                    bar(side * 158.0, 32.0),
                ],
                pink,
                true,
                2.0,
            );
            d.poly(
                &[
                    bar(side * 158.0, 20.0),
                    bar(side * 140.0, 30.0),
                    bar(side * 146.0, 32.0),
                    bar(side * 158.0, 32.0),
                ],
                cap,
                true,
                2.0,
            );
            d.poly(
                &[
                    bar(side * 158.0, 20.0),
                    bar(0.0, 0.0),
                    bar(side * 134.0, 32.0),
                    bar(side * 158.0, 32.0),
                ],
                outline,
                false,
                2.2,
            );
            d.line(
                bar(side * 158.0, 20.0),
                bar(side * 143.0, 32.0),
                outline,
                1.8,
            );
        }
        for side in [-1.0, 1.0] {
            d.poly(
                &[
                    fixed(0.0, 0.0),
                    fixed(side * 138.0, 32.0),
                    fixed(side * 74.0, 32.0),
                ],
                [0.97, 0.92, 0.0, 1.0],
                true,
                2.0,
            );
            d.poly(
                &[
                    fixed(0.0, 0.0),
                    fixed(side * 138.0, 32.0),
                    fixed(side * 114.0, 32.0),
                ],
                [1.0, 0.99, 0.05, 1.0],
                true,
                2.0,
            );
            d.poly(
                &[
                    fixed(0.0, 0.0),
                    fixed(side * 92.0, 29.0),
                    fixed(side * 74.0, 32.0),
                ],
                [0.46, 0.43, 0.015, 1.0],
                true,
                2.0,
            );
            d.poly(
                &[
                    fixed(0.0, 0.0),
                    fixed(side * 138.0, 32.0),
                    fixed(side * 74.0, 32.0),
                ],
                outline,
                false,
                2.2,
            );
            let a = fixed(side * 184.0, 0.0);
            let b = fixed(side * 218.0, 0.0);
            d.line(a, b, outline, 11.0);
            d.circle(a.x, a.y, 5.5, outline, 1.0, true);
            d.circle(b.x, b.y, 5.5, outline, 1.0, true);
            let yellow = [0.78, 0.72, 0.0, 1.0];
            d.line(a, b, yellow, 7.0);
            d.circle(a.x, a.y, 3.5, yellow, 1.0, true);
            d.circle(b.x, b.y, 3.5, yellow, 1.0, true);
            d.line(
                p(a.x, a.y - 1.3),
                p(b.x, b.y - 1.3),
                [1.0, 0.98, 0.02, 1.0],
                2.4,
            );
        }
        let gamma = Velocity::new::<meter_per_second>(s[field::VERTICAL_SPEED_MPS])
            .atan2(
                Velocity::new::<knot>(s[field::GROUNDSPEED_KT])
                    .max(Velocity::new::<meter_per_second>(1.0)),
            )
            .get::<degree>();
        let fpv = project(s[field::GROUND_TRACK_TRUE_DEG], gamma);
        if fpv.visible
            && fpv.point.x > 680.0
            && fpv.point.x < 1280.0
            && fpv.point.y > 200.0
            && fpv.point.y < 735.0
        {
            let q = fpv.point;
            let radius = 13.0 * SCALE;
            for (c, w) in [(outline, 6.6), ([0.39, 0.75, 0.0, 1.0], 3.0)] {
                d.circle(q.x, q.y, radius, c, w, false);
                d.line(p(q.x - 27.0 * SCALE, q.y), p(q.x - radius, q.y), c, w);
                d.line(p(q.x + radius, q.y), p(q.x + 23.0 * SCALE, q.y), c, w);
                d.line(p(q.x, q.y - 25.0 * SCALE), p(q.x, q.y - radius), c, w);
            }
        } else {
            d.text(960.0, 711.0, "FLIGHT PATH OUT OF VIEW", 15.0, GREEN, 1);
        }
    }
}
