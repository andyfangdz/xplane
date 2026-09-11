//! Pure vector generation. The GL backend and both views consume these same rays.
use crate::glyphs::glyph;
use crate::{
    config::{Optics, Runway},
    guidance::LandingPath,
    math::{constrain, deg, rad, rotate, Point, View, FT, KT, PI},
    presentation::{altitude_step, digital_height, indicated_speed, HudPhase, HudPresentation},
};
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub a: Point,
    pub b: Point,
}
impl Segment {
    pub fn clipped(self, l: f64, t: f64, r: f64, b: f64) -> Option<Self> {
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        let (dx, dy) = (self.b.x - self.a.x, self.b.y - self.a.y);
        for (p, q) in [
            (-dx, self.a.x - l),
            (dx, r - self.a.x),
            (-dy, self.a.y - t),
            (dy, b - self.a.y),
        ] {
            if p.abs() < 1e-12 {
                if q < 0.0 {
                    return None;
                }
            } else {
                let u = q / p;
                if p < 0.0 {
                    lo = lo.max(u);
                } else {
                    hi = hi.min(u);
                }
                if lo > hi {
                    return None;
                }
            }
        }
        Some(Self {
            a: Point::new(self.a.x + lo * dx, self.a.y + lo * dy),
            b: Point::new(self.a.x + hi * dx, self.a.y + hi * dy),
        })
    }
    pub fn quad(self, width: f64) -> Option<[Point; 4]> {
        let length = (self.b.x - self.a.x).hypot(self.b.y - self.a.y);
        if length < 1e-9 {
            return None;
        }
        let x = -(self.b.y - self.a.y) * width / (2.0 * length);
        let y = (self.b.x - self.a.x) * width / (2.0 * length);
        Some([
            Point::new(self.a.x + x, self.a.y + y),
            Point::new(self.b.x + x, self.b.y + y),
            Point::new(self.b.x - x, self.b.y - y),
            Point::new(self.a.x - x, self.a.y - y),
        ])
    }
}
pub struct Frame<'a> {
    pub optics: Optics,
    pub runway: &'a Runway,
    pub display: &'a HudPresentation,
    pub panel: bool,
    pub level: i32,
    pub heading: f64,
    pub pitch: f64,
    pub roll: f64,
    pub along: f64,
    pub cross: f64,
    pub altitude: f64,
    pub height_ft: f64,
    pub radar_height_ft: f64,
    pub use_radar: bool,
    pub equivalent: f64,
    pub groundspeed: f64,
    pub ground_track: f64,
    pub vertical_velocity: f64,
    pub command_gamma: f64,
    pub command_height: f64,
    pub command_speedbrake: f64,
    pub actual_speedbrake: f64,
    pub horizontal_cage: bool,
    pub time: f64,
    pub nz: f64,
}
pub struct Scene {
    pub layers: [Vec<Segment>; 2],
    pub velocity_limited: bool,
    pub guidance_limited: bool,
    pub body: View,
}
struct Canvas {
    layers: [Vec<Segment>; 2],
    group: usize,
    panel: bool,
}
impl Canvas {
    fn new(panel: bool) -> Self {
        Self {
            layers: std::array::from_fn(|_| Vec::with_capacity(2048)),
            group: 0,
            panel,
        }
    }
    fn line(&mut self, a: Point, b: Point) {
        if [a.x, a.y, b.x, b.y].iter().all(|x| x.is_finite())
            && self.layers[self.group].len() < 20000
        {
            self.layers[self.group].push(Segment { a, b });
        }
    }
    fn poly(&mut self, points: &[Point]) {
        if let Some(mut prev) = points.last().copied() {
            for &point in points {
                self.line(prev, point);
                prev = point;
            }
        }
    }
    fn circle(&mut self, p: Point, r: f64) {
        let mut prev = Point::new(p.x + r, p.y);
        for i in 1..=32 {
            let a = 2.0 * PI * f64::from(i) / 32.0;
            let q = Point::new(p.x + r * a.cos(), p.y + r * a.sin());
            self.line(prev, q);
            prev = q;
        }
    }
    fn dash(&mut self, a: Point, b: Point) {
        for i in 0..5 {
            let u = f64::from(i) / 5.0;
            let v = (f64::from(i) + 0.6) / 5.0;
            self.line(
                Point::new(a.x + (b.x - a.x) * u, a.y + (b.y - a.y) * u),
                Point::new(a.x + (b.x - a.x) * v, a.y + (b.y - a.y) * v),
            );
        }
    }
    fn text(&mut self, at: Point, text: &str, mut size: f64, align: i32, angle: f64, pivot: Point) {
        if self.panel {
            size *= 1.4;
        }
        let advance = size * 0.78;
        let mut x = at.x - text.len() as f64 * advance * (f64::from(align) * 0.5);
        for ch in text.chars() {
            if ch == '.' {
                self.circle(
                    rotate(Point::new(x + size * 0.2, at.y + size), pivot, angle),
                    0.8,
                );
                x += advance;
                continue;
            }
            if let Some(path) = glyph(ch) {
                let mut prev = None;
                for token in path.split_whitespace() {
                    let bytes = token.as_bytes();
                    let mut j = 0;
                    while j < bytes.len() {
                        if bytes[j] == b'|' {
                            prev = None;
                            j += 1;
                            continue;
                        }
                        if j + 1 >= bytes.len() {
                            break;
                        }
                        let point = rotate(
                            Point::new(
                                x + f64::from(bytes[j] - b'0') * size / 8.0,
                                at.y + f64::from(bytes[j + 1] - b'0') * size / 8.0,
                            ),
                            pivot,
                            angle,
                        );
                        if let Some(p) = prev {
                            self.line(p, point);
                        }
                        prev = Some(point);
                        j += 2;
                    }
                }
            }
            x += advance;
        }
    }
    fn label(&mut self, x: f64, y: f64, text: &str, size: f64, align: i32) {
        self.text(Point::new(x, y), text, size, align, 0.0, Point::default());
    }
    fn triangles(&mut self, c: Point, half: f64, roll: f64) {
        for sign in [-1.0, 1.0] {
            let tip = Point::new(c.x + sign * half, c.y);
            self.poly(&[
                rotate(tip, c, -roll),
                rotate(Point::new(tip.x + sign * 16.0, tip.y - 9.0), c, -roll),
                rotate(Point::new(tip.x + sign * 16.0, tip.y + 9.0), c, -roll),
            ]);
        }
    }
    fn runway(&mut self, v: View, r: &Runway, along: f64, cross: f64, altitude: f64) {
        let project = |a: f64, c: f64| {
            let dn = (a - along) * r.un - (c - cross) * r.ue;
            let de = (a - along) * r.ue + (c - cross) * r.un;
            v.project(
                deg(de.atan2(dn)),
                deg((r.elev - altitude).atan2(dn.hypot(de))),
            )
        };
        let half = 150.0 / FT;
        let length = (15000.0 / FT).min(r.length - r.displaced);
        let corners = [
            Point::new(0.0, -half),
            Point::new(0.0, half),
            Point::new(length, half),
            Point::new(length, -half),
        ];
        for k in 0..4 {
            let (start, end) = (corners[k], corners[(k + 1) % 4]);
            let (mut a, mut b) = (project(start.x, start.y), project(end.x, end.y));
            if a.limited && b.limited {
                continue;
            }
            if a.limited || b.limited {
                let (mut front, mut back) = if a.limited {
                    (end, start)
                } else {
                    (start, end)
                };
                for _ in 0..32 {
                    let mid = Point::new((front.x + back.x) * 0.5, (front.y + back.y) * 0.5);
                    if project(mid.x, mid.y).limited {
                        back = mid;
                    } else {
                        front = mid;
                    }
                }
                if a.limited {
                    a = project(front.x, front.y);
                } else {
                    b = project(front.x, front.y);
                }
            }
            self.line(a.p, b.p);
        }
        let (inner, outer) = (
            project(LandingPath::INNER_AIM, 0.0),
            project(LandingPath::OUTER_AIM, 0.0),
        );
        if !inner.limited {
            self.circle(inner.p, 7.0);
        }
        if !outer.limited {
            self.circle(outer.p, 7.0);
        }
        if !inner.limited && !outer.limited {
            self.line(inner.p, outer.p);
        }
    }
    fn tapes(&mut self, optics: Optics, speed: f64, altitude: f64, error: f64) {
        let cy = 100.0
            + optics.top * 960.0 / (optics.top - optics.bottom)
            + rad(5.0).tan() * 960.0 / (optics.top - optics.bottom);
        let (l, r) = (590.0, 1330.0);
        let mut n = ((speed - 19.0) / 5.0).floor() as i32 * 5;
        while f64::from(n) < speed + 19.0 {
            let y = cy + (f64::from(n) - speed) * 18.0;
            if n >= 0 {
                self.line(
                    Point::new(l - 9.0, y),
                    Point::new(l + if n % 10 != 0 { 2.0 } else { 9.0 }, y),
                );
                if n % 10 == 0 {
                    self.label(l + 20.0, y - 12.0, &n.to_string(), 26.0, 0);
                }
            }
            n += 5;
        }
        self.poly(&[
            Point::new(l - 10.0, cy - 5.0),
            Point::new(l + 10.0, cy - 5.0),
            Point::new(l + 10.0, cy + 5.0),
            Point::new(l - 10.0, cy + 5.0),
        ]);
        let step = altitude_step(altitude);
        let low = ((altitude - 1.65 * step) / step).ceil() as i32;
        let high = ((altitude + 1.65 * step) / step).floor() as i32;
        for n in low..=high {
            let a = f64::from(n) * step;
            let y = cy + (altitude - a) * 150.0 / step;
            if a < 0.0 {
                continue;
            }
            self.line(Point::new(r - 9.0, y), Point::new(r + 9.0, y));
            self.label(
                r - 22.0,
                y - 12.0,
                &if altitude > 1000.0 {
                    format!("{:.0}K", a / 1000.0)
                } else {
                    format!("{a:.0}")
                },
                26.0,
                2,
            );
        }
        self.poly(&[
            Point::new(r - 10.0, cy - 5.0),
            Point::new(r + 10.0, cy - 5.0),
            Point::new(r + 10.0, cy + 5.0),
            Point::new(r - 10.0, cy + 5.0),
        ]);
        let gsi = cy + (error / (altitude * 0.06).max(50.0)).clamp(-3.0, 3.0) * 55.0;
        self.poly(&[
            Point::new(r + 34.0, gsi),
            Point::new(r + 51.0, gsi - 9.0),
            Point::new(r + 51.0, gsi + 9.0),
        ]);
    }
    fn ladder(&mut self, v: View, numbered: bool) {
        for d in (-85_i32..=85).step_by(5) {
            let pitch = f64::from(d);
            if (!numbered && d != 0) || (pitch - v.pitch).abs() > 45.0 {
                continue;
            }
            let span = if d == 0 { 8.0 } else { 5.0 };
            let (a, b, c, e) = (
                v.project(v.heading - span, pitch),
                v.project(v.heading - 2.0, pitch),
                v.project(v.heading + 2.0, pitch),
                v.project(v.heading + span, pitch),
            );
            if a.limited || b.limited || c.limited || e.limited {
                continue;
            }
            if d < 0 {
                self.dash(a.p, b.p);
                self.dash(c.p, e.p);
            } else {
                self.line(a.p, b.p);
                self.line(c.p, e.p);
            }
            if d == 0 {
                continue;
            }
            let (f, g) = (
                v.project(v.heading - span, pitch + if d < 0 { 0.8 } else { -0.8 }),
                v.project(v.heading + span, pitch + if d < 0 { 0.8 } else { -0.8 }),
            );
            self.line(a.p, f.p);
            self.line(e.p, g.p);
            self.text(
                Point::new(e.p.x + 10.0, e.p.y + if d < 0 { -27.0 } else { 8.0 }),
                &d.abs().to_string(),
                22.0,
                0,
                -v.roll,
                e.p,
            );
        }
    }
}
pub fn build(f: &Frame<'_>) -> Scene {
    let mut c = Canvas::new(f.panel);
    let v = f.optics.body_view(f.heading, f.pitch, f.roll);
    let d = f.display;
    let boresight = v.center;
    let fixed = Point::new(v.center.x, v.center.y + v.fy * rad(5.0).tan());
    let track = if f.groundspeed * KT > 2.0 {
        f.ground_track
    } else {
        f.heading
    };
    let gamma = if f.groundspeed * KT > 2.0 {
        deg(f64::from(
            (f.vertical_velocity as f32).atan2(f.groundspeed as f32),
        ))
    } else {
        0.0
    };
    let vv = constrain(v.project(track, gamma), 710.0, 150.0, 1210.0, 825.0);
    let blend = d.fade / 5.0;
    let flight = Point::new(
        if f.horizontal_cage {
            fixed.x
        } else {
            fixed.x + (vv.p.x - fixed.x) * blend
        },
        fixed.y + (vv.p.y - fixed.y) * blend,
    );
    let guide = constrain(
        v.project(
            f.runway.heading + (-f.cross * 0.012).clamp(-20.0, 20.0),
            f.command_gamma,
        ),
        698.0,
        138.0,
        1222.0,
        837.0,
    );
    let flash = f.time % 1.0 < 0.5;
    if !d.main && f.level == 0 {
        c.runway(v, f.runway, f.along, f.cross, f.altitude);
    }
    if d.horizon_visible(f.level) {
        c.ladder(v, d.attitude_visible(f.level));
    }
    if !d.main && f.level < 2 {
        c.tapes(
            f.optics,
            f.equivalent,
            f.height_ft,
            (f.height_ft / FT - f.command_height) * FT,
        );
    }
    c.group = 1;
    c.line(
        Point::new(boresight.x - 11.0, boresight.y),
        Point::new(boresight.x + 11.0, boresight.y),
    );
    c.line(
        Point::new(boresight.x, boresight.y - 11.0),
        Point::new(boresight.x, boresight.y + 11.0),
    );
    if f.level < 3 {
        if !d.main {
            if d.guidance_visible() {
                if d.phase >= HudPhase::Prfnl {
                    if f.height_ft > 2000.0 {
                        let cue = v.project(track, -20.0);
                        c.triangles(Point::new(flight.x, cue.p.y), 76.0, f.roll);
                    }
                    if f.height_ft <= 3500.0 {
                        let reference = if f.height_ft > 2000.0 {
                            -20.0
                        } else {
                            let (mut lo, mut hi) =
                                (LandingPath::CIRCLE_START, LandingPath::INNER_AIM);
                            for _ in 0..32 {
                                let mid = (lo + hi) * 0.5;
                                if LandingPath::at(mid).height > f.height_ft / FT {
                                    lo = mid;
                                } else {
                                    hi = mid;
                                }
                            }
                            deg(LandingPath::at((lo + hi) * 0.5).slope.atan())
                        };
                        let mut cue = v.project(track, reference);
                        if f.height_ft > 2000.0 {
                            cue.p.y += (835.0 - cue.p.y) * (f.height_ft - 2000.0) / 1500.0;
                        }
                        c.triangles(Point::new(flight.x, cue.p.y), 76.0, f.roll);
                    }
                }
                if !guide.limited || flash {
                    c.poly(&[
                        Point::new(guide.p.x, guide.p.y - 15.0),
                        Point::new(guide.p.x + 15.0, guide.p.y),
                        Point::new(guide.p.x, guide.p.y + 15.0),
                        Point::new(guide.p.x - 15.0, guide.p.y),
                    ]);
                }
            }
            if blend < 1.0 || f.horizontal_cage {
                c.poly(&[
                    Point::new(flight.x - 11.0, flight.y - 11.0),
                    Point::new(flight.x + 11.0, flight.y - 11.0),
                    Point::new(flight.x + 11.0, flight.y + 11.0),
                    Point::new(flight.x - 11.0, flight.y + 11.0),
                ]);
            } else {
                c.circle(flight, 11.0);
            }
            c.line(
                Point::new(flight.x - 44.0, flight.y),
                Point::new(flight.x - 11.0, flight.y),
            );
            c.line(
                Point::new(flight.x + 11.0, flight.y),
                Point::new(flight.x + 44.0, flight.y),
            );
            c.line(
                Point::new(flight.x, flight.y - 28.0),
                Point::new(flight.x, flight.y - 11.0),
            );
            if vv.limited && blend >= 1.0 && !f.horizontal_cage {
                c.line(
                    Point::new(flight.x - 7.0, flight.y - 7.0),
                    Point::new(flight.x + 7.0, flight.y + 7.0),
                );
                c.line(
                    Point::new(flight.x - 7.0, flight.y + 7.0),
                    Point::new(flight.x + 7.0, flight.y - 7.0),
                );
            }
        }
        if f.level == 2 || d.main || f.height_ft <= 1000.0 {
            let numbers = if d.main { boresight } else { flight };
            let s = indicated_speed(if d.nose {
                f.groundspeed * KT
            } else {
                f.equivalent
            });
            c.label(
                numbers.x - 73.0,
                numbers.y - 62.0,
                &format!("{}{s}", if d.nose { "G " } else { "" }),
                30.0,
                2,
            );
        }
        if !d.main {
            c.label(
                flight.x + 73.0,
                flight.y - 62.0,
                &format!(
                    "{}{}",
                    digital_height(if f.use_radar {
                        f.radar_height_ft
                    } else {
                        f.height_ft
                    }),
                    if f.use_radar { " R" } else { "" }
                ),
                30.0,
                0,
            );
        }
        if d.nz_visible() && (f.nz <= 2.0 || flash) {
            c.label(
                flight.x - 74.0,
                flight.y + 32.0,
                &format!("{:.1}G", f.nz),
                22.0,
                2,
            );
        }
        if !d.main {
            c.label(690.0, 912.0, d.phase.label(), 26.0, 0);
            c.label(
                690.0,
                970.0,
                if d.automatic { "AUTO" } else { "CSS" },
                20.0,
                0,
            );
            if d.gear_cue != 0 && (d.gear_cue != 1 || flash) {
                c.label(
                    690.0,
                    390.0,
                    match d.gear_cue {
                        1 => "GEAR",
                        2 => "GR",
                        _ => "GR-DN",
                    },
                    22.0,
                    0,
                );
            }
        }
        let (sx, sy, len) = (1000.0, 923.0, 200.0);
        c.line(Point::new(sx, sy), Point::new(sx + len, sy));
        for i in 0..5 {
            let tick = if i == 0 || i == 4 { 13.0 } else { 5.0 };
            let x = sx + f64::from(i) * len / 4.0;
            c.line(Point::new(x, sy - tick), Point::new(x, sy + tick));
        }
        let actual = f.actual_speedbrake.clamp(0.0, 1.0);
        let px = sx + len * actual;
        let cx = sx + len * f.command_speedbrake;
        if (actual - f.command_speedbrake).abs() <= 20.0 / 98.6 || flash {
            c.poly(&[
                Point::new(px, sy - 3.0),
                Point::new(px - 9.0, sy - 21.0),
                Point::new(px + 9.0, sy - 21.0),
            ]);
        }
        c.line(Point::new(cx, sy + 3.0), Point::new(cx, sy + 27.0));
        c.line(Point::new(cx, sy + 3.0), Point::new(cx - 9.0, sy + 15.0));
        c.line(Point::new(cx, sy + 3.0), Point::new(cx + 9.0, sy + 15.0));
        if d.main {
            let (x, top, length) = (1320.0, 530.0, 220.0);
            c.line(Point::new(x, top), Point::new(x, top + length));
            for k in 0..5 {
                let y = top + f64::from(k) * length / 4.0;
                c.line(Point::new(x - 6.0, y), Point::new(x + 6.0, y));
            }
            let y = top + length * (1.0 - (d.deceleration / 0.4).clamp(0.0, 1.0));
            c.poly(&[
                Point::new(x - 3.0, y),
                Point::new(x - 21.0, y - 9.0),
                Point::new(x - 21.0, y + 9.0),
            ]);
            let y = top + length * (1.0 - (d.required_deceleration / 0.4).clamp(0.0, 1.0));
            c.line(Point::new(x + 3.0, y), Point::new(x + 27.0, y));
            c.line(Point::new(x + 3.0, y), Point::new(x + 15.0, y - 9.0));
            c.line(Point::new(x + 3.0, y), Point::new(x + 15.0, y + 9.0));
        }
    }
    Scene {
        layers: c.layers,
        velocity_limited: vv.limited,
        guidance_limited: guide.limited,
        body: v,
    }
}
