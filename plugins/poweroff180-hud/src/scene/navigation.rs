use super::*;
use crate::hud::cdi_offset;
use poweroff180::guidance::wrap;
use uom::si::{angle::degree, f64::Angle, f64::Length, length::meter, length::nautical_mile};
use xplane_airports::{GeoPoint, LocalProjection};
use xplane_hud::rotate;
fn heading_text(n: f64) -> String {
    if !n.is_finite() {
        return "---".into();
    }
    let degrees = n.round() as i32 % 360;
    format!("{:03}", if degrees == 0 { 360 } else { degrees })
}
impl Scene {
    fn degree_text(&mut self, x: f64, y: f64, heading: f64, size: f64, c: Color) {
        self.text(x, y, heading_text(heading), size, c, 1);
        self.circle(
            x + size * 0.96,
            y + size * 0.14,
            size * 0.065,
            c,
            1.4,
            false,
        );
    }
}
impl Hud {
    pub fn leg_cross_track(&self, v: &Values) -> f64 {
        if !self.nav_ready {
            return f64::NAN;
        }
        let [lat, lon, end_lat, end_lon] = self.leg;
        // The HUD reports nautical miles using a fixed leg-midpoint latitude.
        let projection = LocalProjection::new(
            GeoPoint {
                lat,
                lon,
                elevation: Length::new::<meter>(0.0),
            },
            Angle::new::<degree>((lat + end_lat) * 0.5),
            Length::new::<nautical_mile>(60.0),
        );
        let Some(axis) = projection.axis_to(GeoPoint {
            lat: end_lat,
            lon: end_lon,
            elevation: Length::new::<meter>(0.0),
        }) else {
            return f64::NAN;
        };
        let (east, north) = projection.project(GeoPoint {
            lat: v.get("sim/flightmodel/position/latitude"),
            lon: v.get("sim/flightmodel/position/longitude"),
            elevation: Length::new::<meter>(0.0),
        });
        axis.offsets(east, north).1.get::<nautical_mile>()
    }
    pub(super) fn navigation(&self, d: &mut Scene, s: &[f64; LENGTH], v: &Values) {
        let xtk = self.leg_cross_track(v);
        let hdg = v.get("sim/cockpit2/gauges/indicators/heading_AHARS_deg_mag_pilot");
        let course = v.get("sim/cockpit/radios/gps_course_degtm");
        let dots = v.get("sim/cockpit/radios/gps_hdef_dot");
        let selected = v.get("sim/cockpit2/autopilot/heading_dial_deg_mag_pilot");
        let track = v.get("sim/cockpit2/gauges/indicators/ground_track_mag_pilot");
        let fromto = v.get("sim/cockpit/radios/gps_fromto");
        let valid = self.nav_ready
            && fromto != 0.0
            && hdg.is_finite()
            && course.is_finite()
            && dots.is_finite();
        let (cx, cy, r) = (960.0, 879.0, 148.0);
        d.circle(cx, cy, r, TAPE, 1.0, true);
        let polar = |angle: f64, radius: f64| {
            let a = rad(angle);
            p(cx + a.sin() * radius, cy - a.cos() * radius)
        };
        if hdg.is_finite() {
            for degrees in (0..360).step_by(5) {
                let a = wrap(f64::from(degrees) - hdg);
                let major = degrees % 10 == 0;
                d.line(
                    polar(a, r - if major { 17.0 } else { 9.0 }),
                    polar(a, r),
                    GRAY,
                    if major { 2.1 } else { 1.6 },
                );
                if degrees % 30 == 0 {
                    let q = polar(a, r - 36.0);
                    let label = match degrees {
                        0 => "N".into(),
                        90 => "E".into(),
                        180 => "S".into(),
                        270 => "W".into(),
                        _ => num(f64::from(degrees / 10)),
                    };
                    d.rotated_text(q, label, 27.0, GRAY, a);
                }
            }
            if selected.is_finite() {
                let a = wrap(selected - hdg);
                let b = |x, y| rotate(p(cx + x, cy + y), p(cx, cy), a);
                d.poly(
                    &[
                        b(-13.0, -r - 2.0),
                        b(13.0, -r - 2.0),
                        b(13.0, -r + 10.0),
                        b(5.0, -r + 10.0),
                        b(0.0, -r + 4.0),
                        b(-5.0, -r + 10.0),
                        b(-13.0, -r + 10.0),
                    ],
                    CYAN,
                    true,
                    2.0,
                );
            }
            if track.is_finite() {
                let q = polar(wrap(track - hdg), r - 8.0);
                d.poly(
                    &[
                        p(q.x, q.y - 7.0),
                        p(q.x + 5.0, q.y),
                        p(q.x, q.y + 7.0),
                        p(q.x - 5.0, q.y),
                    ],
                    PINK,
                    true,
                    2.0,
                );
            }
        }
        for a in [-18.0, -9.0, 9.0, 18.0] {
            d.line(polar(a, r + 5.0), polar(a, r + 16.0), GRAY, 2.0);
        }
        let turn = (self.trend.turn_rate * 6.0).clamp(-24.0, 24.0);
        if turn.abs() > 0.3 {
            d.arc(cx, cy, r + 8.0, -90.0, -90.0 + turn, PINK, 5.0);
            if self.trend.turn_rate.abs() > 4.0 {
                let q = polar(turn, r + 8.0);
                let sign = if turn > 0.0 { 1.0 } else { -1.0 };
                d.poly(
                    &[
                        p(q.x + sign * 8.0, q.y),
                        p(q.x - sign * 5.0, q.y - 6.0),
                        p(q.x - sign * 5.0, q.y + 6.0),
                    ],
                    PINK,
                    true,
                    2.0,
                );
            }
        }
        d.rect(cx - 48.0, cy - r - 57.0, 96.0, 39.0, GBLACK);
        d.poly(
            &[
                p(cx - 48.0, cy - r - 57.0),
                p(cx + 48.0, cy - r - 57.0),
                p(cx + 48.0, cy - r - 18.0),
                p(cx - 48.0, cy - r - 18.0),
            ],
            GRAY,
            false,
            1.3,
        );
        d.degree_text(cx - 2.0, cy - r - 56.0, hdg, 33.0, WHITE);
        d.poly(
            &[
                p(cx - 9.0, cy - r - 17.0),
                p(cx + 9.0, cy - r - 17.0),
                p(cx, cy - r + 2.0),
            ],
            WHITE,
            true,
            2.0,
        );
        d.rect(cx - 262.0, cy - r - 21.0, 140.0, 31.0, GBLACK);
        d.text(cx - 256.0, cy - r - 15.0, "HDG", 18.0, GRAY, 0);
        d.degree_text(cx - 167.0, cy - r - 19.0, selected, 24.0, CYAN);
        d.rect(cx + 122.0, cy - r - 21.0, 140.0, 31.0, GBLACK);
        d.text(cx + 128.0, cy - r - 15.0, "CRS", 18.0, GRAY, 0);
        d.degree_text(
            cx + 217.0,
            cy - r - 19.0,
            if valid { course } else { f64::NAN },
            24.0,
            PINK,
        );
        if valid {
            let a = wrap(course - hdg);
            let c = |x, y| rotate(p(cx + x, cy + y), p(cx, cy), a);
            for dot in [-2.0, -1.0, 1.0, 2.0] {
                let q = c(dot * 30.0, 0.0);
                d.circle(q.x, q.y, 4.2, GRAY, 1.7, false);
            }
            d.line(c(0.0, -124.0), c(0.0, -59.0), PINK, 4.0);
            d.line(c(0.0, 59.0), c(0.0, 133.0), PINK, 4.0);
            d.poly(
                &[
                    c(0.0, -134.0),
                    c(-12.0, -119.0),
                    c(-3.0, -119.0),
                    c(-3.0, -112.0),
                    c(3.0, -112.0),
                    c(3.0, -119.0),
                    c(12.0, -119.0),
                ],
                PINK,
                true,
                2.0,
            );
            let shift = cdi_offset(dots);
            d.line(c(shift, -54.0), c(shift, 54.0), GBLACK, 8.0);
            d.line(c(shift, -54.0), c(shift, 54.0), PINK, 4.5);
            let sense = if fromto == 1.0 { -1.0 } else { 1.0 };
            d.poly(
                &[
                    c(0.0, sense * 64.0),
                    c(-8.0, sense * 51.0),
                    c(8.0, sense * 51.0),
                ],
                PINK,
                true,
                2.0,
            );
            d.text(cx - 36.0, cy - 51.0, "GPS", 20.0, PINK, 1);
            const PHASES: [&str; 14] = [
                "OCN", "ENR", "TERM", "DPRT", "MAPR", "APR", "RNPAR", "LNAV", "LNAV+V", "L/VNAV",
                "LP", "LPV", "LP+V", "GLS",
            ];
            let mode = v.get("sim/cockpit/radios/gps_cdi_sensitivity");
            if mode.is_finite() && (0.0..14.0).contains(&mode) {
                d.text(cx + 38.0, cy - 51.0, PHASES[mode as usize], 18.0, PINK, 1);
            }
            if dots.abs() > 2.0 && xtk.is_finite() {
                d.text(
                    cx,
                    cy + 31.0,
                    format!("XTK {} NM", number(xtk.abs(), 2, false)),
                    17.0,
                    PINK,
                    1,
                );
            }
            let seq = v.get("sim/cockpit/radios/gps_sequencing");
            if seq == 1.0 || seq == 2.0 {
                d.text(
                    cx + 38.0,
                    cy + 58.0,
                    if seq == 1.0 { "OBS" } else { "SUSP" },
                    17.0,
                    PINK,
                    1,
                );
            }
        } else {
            d.text(cx, cy - 48.0, "NO NAV", 21.0, MUTED, 1);
        }
        d.line(p(cx, cy - 19.0), p(cx, cy + 18.0), GBLACK, 10.0);
        d.line(p(cx, cy - 19.0), p(cx, cy + 18.0), WHITE, 6.0);
        d.poly(
            &[
                p(cx, cy - 10.0),
                p(cx - 19.0, cy + 1.0),
                p(cx - 19.0, cy + 5.0),
                p(cx + 19.0, cy + 5.0),
                p(cx + 19.0, cy + 1.0),
            ],
            WHITE,
            true,
            2.0,
        );
        d.poly(
            &[
                p(cx, cy + 9.0),
                p(cx - 9.0, cy + 17.0),
                p(cx - 9.0, cy + 20.0),
                p(cx + 9.0, cy + 20.0),
                p(cx + 9.0, cy + 17.0),
            ],
            WHITE,
            true,
            2.0,
        );
        d.text(
            1150.0,
            924.0,
            format!(
                "LEG XTK {} NM {}",
                number(xtk.abs(), 3, false),
                if xtk >= 0.0 { "R" } else { "L" }
            ),
            23.0,
            PINK,
            0,
        );
        d.text(
            1150.0,
            959.0,
            format!(
                "CENTERLINE {} FT",
                number(s[field::RUNWAY_CROSS_FT], 1, true)
            ),
            22.0,
            WHITE,
            0,
        );
        d.text(
            1150.0,
            994.0,
            "CALCULATED LEG XTK / LIVE GPS CDI",
            14.0,
            MUTED,
            0,
        );
    }
}
