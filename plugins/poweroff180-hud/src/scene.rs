//! Pure display commands in the original 1920 × 1080 design coordinates.
//! Geometry follows Garmin G1000 190-00498-07 Rev A figures 2-3, 2-8,
//! 2-11, 2-12, 2-17 and 2-19 and the accepted native HUD v5 reference.
use xplane_units::{
    feet, kilograms,
    length::nautical_mile,
    mass_rate::kilogram_per_second,
    meters_per_second,
    power::{horsepower, watt},
    ratio::ratio,
    velocity::knot,
    volume::gallon,
    volume_rate::gallon_per_hour,
    MassRate, Power, Volume,
};
mod attitude;
mod instruments;
mod navigation;
use crate::values::Values;
use poweroff180::{
    guidance::rad,
    hud::{self, point as p, Point, Trend},
    protocol::{field, Snapshot, LENGTH},
    Config,
};

pub type Color = [f32; 4];
pub const WHITE: Color = [0.97, 0.98, 0.99, 1.0];
pub const MUTED: Color = [0.72, 0.77, 0.80, 1.0];
pub const GREEN: Color = [0.26, 0.93, 0.27, 1.0];
pub const MAGENTA: Color = [0.92, 0.41, 0.94, 1.0];
pub const GOLD: Color = [1.0, 0.87, 0.19, 1.0];
pub const ORANGE: Color = [1.0, 0.71, 0.13, 1.0];
pub const GRID: Color = [0.28, 0.35, 0.38, 1.0];
pub const PANEL: Color = [0.10, 0.115, 0.12, 0.96];
pub const BLACK: Color = [0.035, 0.045, 0.055, 0.94];
pub const PINK: Color = [1.0, 0.24, 1.0, 1.0];
pub const CYAN: Color = [0.22, 0.93, 1.0, 1.0];
pub const GRAY: Color = [0.82, 0.85, 0.84, 1.0];
pub const TAPE: Color = [0.04, 0.065, 0.07, 0.46];
pub const GBLACK: Color = [0.008, 0.014, 0.018, 0.98];
pub const RED: Color = [1.0, 0.16, 0.07, 1.0];

#[derive(Debug, Clone)]
pub enum Draw {
    Line(Point, Point, Color, f64),
    Polygon(Vec<Point>, Color, bool, f64),
    Circle(Point, f64, Color, f64, bool),
    Arc(Point, f64, f64, f64, Color, f64),
    Text {
        at: Point,
        text: String,
        size: f64,
        color: Color,
        anchor: u8,
        rotation: Option<f64>,
    },
    Clip {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
    Unclip,
}
#[derive(Debug, Default)]
pub struct Scene {
    pub commands: Vec<Draw>,
}
impl Scene {
    pub fn line(&mut self, a: Point, b: Point, c: Color, w: f64) {
        self.commands.push(Draw::Line(a, b, c, w));
    }
    pub fn poly(&mut self, points: &[Point], c: Color, fill: bool, w: f64) {
        self.commands
            .push(Draw::Polygon(points.to_vec(), c, fill, w));
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, c: Color) {
        self.poly(
            &[p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h)],
            c,
            true,
            2.0,
        );
    }
    pub fn circle(&mut self, x: f64, y: f64, r: f64, c: Color, w: f64, fill: bool) {
        self.commands.push(Draw::Circle(p(x, y), r, c, w, fill));
    }
    #[allow(clippy::too_many_arguments)] // Design coordinates and style mirror the reference drawing API.
    pub fn arc(&mut self, x: f64, y: f64, r: f64, from: f64, to: f64, c: Color, w: f64) {
        self.commands.push(Draw::Arc(p(x, y), r, from, to, c, w));
    }
    pub fn text(
        &mut self,
        x: f64,
        y: f64,
        text: impl Into<String>,
        size: f64,
        color: Color,
        anchor: u8,
    ) {
        self.commands.push(Draw::Text {
            at: p(x, y),
            text: text.into(),
            size,
            color,
            anchor,
            rotation: None,
        });
    }
    pub fn rotated_text(
        &mut self,
        at: Point,
        text: impl Into<String>,
        size: f64,
        color: Color,
        rotation: f64,
    ) {
        self.commands.push(Draw::Text {
            at,
            text: text.into(),
            size,
            color,
            anchor: 1,
            rotation: Some(rotation),
        });
    }
    pub fn clip(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.commands.push(Draw::Clip { x, y, w, h });
    }
    pub fn unclip(&mut self) {
        self.commands.push(Draw::Unclip);
    }
}
pub fn number(v: f64, precision: usize, sign: bool) -> String {
    if !v.is_finite() {
        return "--".into();
    }
    if sign {
        format!("{v:+.precision$}")
    } else {
        format!("{v:.precision$}")
    }
}
fn num(v: f64) -> String {
    number(v, 0, false)
}
#[derive(Debug, Clone, Copy)]
pub struct Trail {
    pub along: f64,
    pub cross: f64,
    pub agl: f64,
    pub distance: f64,
}
pub struct Hud {
    pub config: Config,
    pub trail: Vec<Trail>,
    pub first_time: f64,
    pub last_time: f64,
    pub path_distance: f64,
    pub trend: Trend,
    pub full_flap_limit: f64,
    pub nav_ready: bool,
    pub leg: [f64; 4],
}
impl Default for Hud {
    fn default() -> Self {
        Self {
            config: Config::default(),
            trail: Vec::new(),
            first_time: -1.0,
            last_time: -1.0,
            path_distance: 0.0,
            trend: Trend::default(),
            full_flap_limit: f64::NAN,
            nav_ready: false,
            leg: [0.0; 4],
        }
    }
}
impl Hud {
    pub fn new_flight(&mut self, config: Config) {
        self.config = config;
        self.trail.clear();
        self.first_time = -1.0;
        self.last_time = -1.0;
        self.path_distance = 0.0;
        self.trend = Trend::default();
    }
    pub fn frame(&mut self, snapshot: &Snapshot, v: &Values) -> Scene {
        let s = snapshot.map(f64::from);
        let mut draw = Scene::default();
        self.instruments(&mut draw, &s, v);
        self.attitude(&mut draw, &s, v);
        self.navigation(&mut draw, &s, v);
        self.controls(&mut draw, &s);
        draw.rect(258.0, 1038.0, 1662.0, 42.0, PANEL);
        const PHASES: [&str; 11] = [
            "IDLE",
            "READY",
            "DOWNWIND",
            "POWER OFF",
            "TURN TO BASE",
            "BASE",
            "TURN TO FINAL",
            "FINAL",
            "ROLLOUT",
            "COMPLETE",
            "ABORTED",
        ];
        let status = if s[field::CONTACT_LATCHED] != 0.0 {
            format!(
                "TOUCHDOWN {} FT | {} FPM SINK | {} KIAS",
                num(s[field::FIRST_ALONG_FT]),
                num(s[field::FIRST_PHYSICAL_FPM].abs()),
                number(s[field::FIRST_KIAS], 1, false)
            )
        } else {
            PHASES[(s[field::PHASE_ID] as usize).min(10)].into()
        };
        draw.text(284.0, 1048.0, status, 20.0, WHITE, 0);
        draw.text(
            1890.0,
            1048.0,
            format!(
                "{} s",
                number(
                    if self.first_time >= 0.0 {
                        s[field::SIM_TIME] - self.first_time
                    } else {
                        0.0
                    },
                    1,
                    false
                )
            ),
            20.0,
            WHITE,
            2,
        );
        draw
    }
    fn paths(&mut self, d: &mut Scene, s: &[f64; LENGTH]) {
        if s[field::PHASE_ID] >= 2.0
            && s[field::RUNWAY_ALONG_FT] <= 2800.0
            && (self.last_time < 0.0 || s[field::SIM_TIME] - self.last_time >= 0.1)
        {
            if let Some(last) = self.trail.last() {
                self.path_distance += feet(s[field::RUNWAY_ALONG_FT] - last.along)
                    .hypot(feet(s[field::RUNWAY_CROSS_FT] - last.cross))
                    .get::<nautical_mile>();
            }
            self.trail.push(Trail {
                along: s[field::RUNWAY_ALONG_FT],
                cross: s[field::RUNWAY_CROSS_FT],
                agl: s[field::AGL_FT],
                distance: self.path_distance,
            });
            self.last_time = s[field::SIM_TIME];
            if self.first_time < 0.0 {
                self.first_time = s[field::SIM_TIME];
            }
            if self.trail.len() > 3000 {
                self.trail.drain(..1000);
            }
        }
        d.rect(0.0, 712.0, 522.0, 326.0, PANEL);
        d.text(18.0, 730.0, "HORIZONTAL PATH", 20.0, WHITE, 0);
        d.text(281.0, 730.0, "VERTICAL PATH", 20.0, WHITE, 0);
        d.line(p(265.0, 758.0), p(265.0, 1020.0), GRID, 1.0);
        let plan =
            |along: f64, cross: f64| p(235.0 - cross * 0.030, 984.0 - (along + 3000.0) * 0.027);
        let profile = |distance: f64, h: f64| {
            p(
                322.0 + distance / 3.5 * 176.0,
                990.0 - h.clamp(0.0, 1200.0) / 1200.0 * 190.0,
            )
        };
        d.line(plan(0.0, 0.0), plan(4000.0, 0.0), WHITE, 5.0);
        let target = plan(self.config.path_target_touchdown_ft, 0.0);
        d.line(
            p(target.x - 7.0, target.y),
            p(target.x + 7.0, target.y),
            GOLD,
            3.0,
        );
        d.text(18.0, 1008.0, "RWY 22 ^", 18.0, MUTED, 0);
        d.text(282.0, 770.0, "AGL FT", 16.0, MUTED, 0);
        for h in [0.0, 500.0, 1000.0] {
            let y = profile(0.0, h).y;
            d.line(p(322.0, y), p(498.0, y), GRID, 1.0);
            d.text(313.0, y - 9.0, num(h), 16.0, MUTED, 2);
        }
        for pair in self.trail.windows(2) {
            let a = pair[0];
            let b = pair[1];
            d.line(plan(a.along, a.cross), plan(b.along, b.cross), MAGENTA, 3.0);
            d.line(
                profile(a.distance, a.agl),
                profile(b.distance, b.agl),
                MAGENTA,
                3.0,
            );
        }
        if let Some(q) = self.trail.last() {
            for point in [plan(q.along, q.cross), profile(q.distance, q.agl)] {
                d.circle(point.x, point.y, 5.0, GOLD, 1.0, true);
            }
        }
        d.text(320.0, 1008.0, "0", 16.0, MUTED, 0);
        d.text(498.0, 1008.0, "3.5 NM", 16.0, MUTED, 2);
    }
    fn instruments(&mut self, d: &mut Scene, s: &[f64; LENGTH], v: &Values) {
        d.rect(0.0, 0.0, 258.0, 1080.0, PANEL);
        d.rect(258.0, 0.0, 1662.0, 55.0, PANEL);
        d.rect(0.0, 55.0, 1920.0, 59.0, PANEL);
        d.text(152.0, 6.0, "TORQUESIM", 14.0, WHITE, 0);
        d.text(152.0, 25.0, "SR20", 20.0, WHITE, 0);
        d.text(286.0, 15.0, "POWER-OFF 180  |  KCDW RWY 22", 24.0, WHITE, 0);
        let gps_dist = v.get("sim/cockpit2/radios/indicators/gps_dme_distance_nm");
        let course = v.get("sim/cockpit/radios/gps_course_degtm");
        let gps = self.nav_ready && v.get("sim/cockpit/radios/gps_fromto") != 0.0;
        d.text(
            990.0,
            15.0,
            if gps {
                format!(
                    "KOLLI > RW22   {} NM   CRS {} M",
                    number(gps_dist, 1, false),
                    num(course)
                )
            } else {
                "GPS: NO ACTIVE APPROACH".into()
            },
            24.0,
            if gps { MAGENTA } else { MUTED },
            0,
        );
        d.text(1660.0, 15.0, "GPS / RNAV 22", 24.0, WHITE, 0);
        d.text(22.0, 72.0, "FULL FUEL + 400 LB FRONT", 22.0, WHITE, 0);
        d.text(
            520.0,
            62.0,
            hud::wind_label(self.config.wind_speed_kt, self.config.wind_offset_deg),
            20.0,
            WHITE,
            0,
        );
        d.text(
            520.0,
            88.0,
            format!(
                "LOCAL {} KT",
                number(
                    meters_per_second(s[field::WIND_SPEED_MPS]).get::<knot>(),
                    1,
                    false
                )
            ),
            16.0,
            MUTED,
            0,
        );
        d.text(
            850.0,
            72.0,
            format!("ROUNDOUT {} FT", num(self.config.flare_height_ft)),
            22.0,
            GOLD,
            0,
        );
        d.text(
            1220.0,
            72.0,
            format!(
                "TARGET {} FT / SINK <= 200 FPM",
                num(self.config.path_target_touchdown_ft)
            ),
            22.0,
            GOLD,
            0,
        );
        // The aircraft's 215 hp rating, expressed as physical power.
        let rated_power = Power::new::<horsepower>(215.0);
        let power = (Power::new::<watt>(s[field::ENGINE_POWER_W].max(0.0)) / rated_power)
            .get::<ratio>()
            * 100.0;
        d.arc(128.0, 235.0, 96.0, 180.0, 360.0, GRID, 8.0);
        d.arc(
            128.0,
            235.0,
            96.0,
            180.0,
            180.0 + power.clamp(0.0, 100.0) * 1.8,
            GREEN,
            8.0,
        );
        let a = rad(180.0 + power.clamp(0.0, 100.0) * 1.8);
        d.circle(
            128.0 + 96.0 * a.cos(),
            235.0 + 96.0 * a.sin(),
            5.0,
            WHITE,
            1.0,
            true,
        );
        d.text(128.0, 192.0, format!("{}%", num(power)), 48.0, WHITE, 1);
        d.text(128.0, 251.0, "POWER", 24.0, MUTED, 1);
        d.text(18.0, 308.0, "RPM", 20.0, MUTED, 0);
        d.text(239.0, 304.0, num(s[field::ENGINE_RPM]), 26.0, WHITE, 2);
        d.line(p(18.0, 344.0), p(240.0, 344.0), GRID, 1.0);
        d.text(18.0, 365.0, "MP", 20.0, MUTED, 0);
        d.text(
            239.0,
            361.0,
            format!(
                "{} inHg",
                number(v.get("sim/cockpit2/engine/indicators/MPR_in_hg"), 1, false)
            ),
            26.0,
            WHITE,
            2,
        );
        d.line(p(18.0, 401.0), p(240.0, 401.0), GRID, 1.0);
        d.text(18.0, 422.0, "FF", 20.0, MUTED, 0);
        d.text(
            239.0,
            418.0,
            format!(
                "{} gph",
                number(
                    // Modelled avgas density: 2.72155 kg per US gallon.
                    (MassRate::new::<kilogram_per_second>(s[field::FUEL_FLOW_KG_S])
                        / (kilograms(2.72155) / Volume::new::<gallon>(1.0)))
                    .get::<gallon_per_hour>(),
                    1,
                    false,
                )
            ),
            26.0,
            WHITE,
            2,
        );
        d.line(p(18.0, 458.0), p(240.0, 458.0), GRID, 1.0);
        d.text(18.0, 482.0, "FLAPS", 22.0, WHITE, 0);
        for (j, label) in ["UP", "50%", "FULL"].iter().enumerate() {
            let y = 540.0 + j as f64 * 52.0;
            d.text(161.0, y - 12.0, *label, 22.0, MUTED, 0);
            d.line(p(142.0, y), p(153.0, y), MUTED, 2.0);
        }
        let fy = 540.0 + s[field::FLAP_HANDLE_RATIO].clamp(0.0, 1.0) * 104.0;
        d.circle(64.0, 540.0, 5.0, WHITE, 1.0, true);
        d.line(p(64.0, 540.0), p(135.0, fy), WHITE, 4.0);
        let fa = (fy - 540.0).atan2(71.0);
        d.poly(
            &[
                p(143.0, fy),
                p(
                    143.0 - 16.0 * fa.cos() + 6.0 * fa.sin(),
                    fy - 16.0 * fa.sin() - 6.0 * fa.cos(),
                ),
                p(
                    143.0 - 16.0 * fa.cos() - 6.0 * fa.sin(),
                    fy - 16.0 * fa.sin() + 6.0 * fa.cos(),
                ),
            ],
            WHITE,
            true,
            2.0,
        );
        d.text(
            20.0,
            685.0,
            format!(
                "SELECTED {}% / ACTUAL {}%",
                num(s[field::FLAP_HANDLE_RATIO] * 100.0),
                num(s[field::FLAP_ACTUAL_RATIO] * 100.0)
            ),
            14.0,
            MUTED,
            0,
        );
        self.paths(d, s);
        self.tapes(d, s, v);
    }
    fn controls(&self, d: &mut Scene, s: &[f64; LENGTH]) {
        d.rect(1640.0, 355.0, 263.0, 548.0, PANEL);
        d.text(1660.0, 374.0, "CONTROL INPUT", 22.0, WHITE, 0);
        let roll = s[field::AILERON_INPUT].clamp(-1.0, 1.0);
        let elev = s[field::ELEVATOR_INPUT].clamp(-1.0, 1.0);
        let yaw = s[field::RUDDER_INPUT].clamp(-1.0, 1.0);
        d.line(p(1702.0, 543.0), p(1702.0, 465.0), GRID, 11.0);
        let a = rad(roll * 35.0);
        let ex = 1702.0 + a.sin() * 78.0;
        let ey = 543.0 - a.cos() * 78.0;
        d.line(p(1702.0, 543.0), p(ex, ey), ORANGE, 13.0);
        d.circle(ex, ey, 7.0, ORANGE, 1.0, true);
        d.text(
            1680.0,
            575.0,
            format!("AIL {}%", number(roll * 100.0, 0, true)),
            20.0,
            WHITE,
            0,
        );
        d.rect(1830.0, 453.0, 14.0, 95.0, BLACK);
        d.line(p(1811.0, 500.0), p(1863.0, 500.0), MUTED, 1.0);
        d.rect(1818.0, 494.0 + elev * 45.0, 38.0, 12.0, ORANGE);
        d.text(1810.0, 425.0, "PUSH", 18.0, MUTED, 0);
        d.text(1810.0, 553.0, "PULL", 18.0, MUTED, 0);
        d.text(
            1795.0,
            575.0,
            format!("ELE {}%", number(elev * 100.0, 0, true)),
            20.0,
            WHITE,
            0,
        );
        for sign in [-1.0, 1.0] {
            let x = 1774.0 + sign * 64.0;
            let y = 706.0 - sign * yaw * 34.0;
            d.poly(
                &[
                    p(x - 25.0, 665.0),
                    p(x + 25.0, 665.0),
                    p(x + 25.0, 747.0),
                    p(x - 25.0, 747.0),
                ],
                MUTED,
                false,
                1.0,
            );
            d.rect(
                x - 22.0,
                y - 36.0,
                44.0,
                72.0,
                if sign * yaw > 0.01 { ORANGE } else { BLACK },
            );
            d.poly(
                &[
                    p(x - 22.0, y - 36.0),
                    p(x + 22.0, y - 36.0),
                    p(x + 22.0, y + 36.0),
                    p(x - 22.0, y + 36.0),
                ],
                ORANGE,
                false,
                2.0,
            );
        }
        d.text(
            1770.0,
            788.0,
            format!("RUDDER {}%", number(yaw * 100.0, 0, true)),
            22.0,
            WHITE,
            1,
        );
        d.text(1660.0, 850.0, "NEUTRAL 0%   +/-100%", 18.0, MUTED, 0);
        d.text(
            1840.0,
            964.0,
            format!("{} G", number(s[field::NORMAL_G], 2, false)),
            28.0,
            WHITE,
            2,
        );
    }
}
