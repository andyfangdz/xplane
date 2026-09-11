//! Display geometry only. The design plane is 1920 by 1080, with Y down.
use crate::guidance::{rad, wrap};
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
pub fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}
#[derive(Debug, Clone, Copy)]
pub struct Projection {
    pub point: Point,
    pub visible: bool,
}
pub fn project(
    bearing: f64,
    elevation: f64,
    heading: f64,
    pitch: f64,
    roll: f64,
    fov: f64,
) -> Projection {
    let a = rad(wrap(bearing - heading));
    let e = rad(elevation);
    let p = rad(pitch);
    let r = rad(roll);
    let forward = e.cos() * a.cos() * p.cos() + e.sin() * p.sin();
    let right = e.cos() * a.sin();
    let up = e.sin() * p.cos() - e.cos() * a.cos() * p.sin();
    if forward <= 0.01 || fov <= 1.0 || fov >= 175.0 {
        return Projection {
            point: point(0.0, 0.0),
            visible: false,
        };
    }
    let focal = 960.0 / rad(fov * 0.5).tan();
    Projection {
        point: point(
            960.0 + focal * (right * r.cos() - up * r.sin()) / forward,
            540.0 - focal * (right * r.sin() + up * r.cos()) / forward,
        ),
        visible: true,
    }
}
#[derive(Debug, Clone, Copy)]
pub struct FlightDirector {
    pub center: Point,
    pub roll: f64,
}
pub fn director(bank_command: f64, bank: f64, pitch_command: f64, pitch: f64) -> FlightDirector {
    FlightDirector {
        center: point(
            960.0,
            540.0 - ((pitch_command - pitch) * 22.0).clamp(-170.0, 170.0),
        ),
        roll: wrap(bank_command - bank).clamp(-35.0, 35.0),
    }
}
pub fn rotate(p: Point, center: Point, degrees: f64) -> Point {
    let a = rad(degrees);
    let x = p.x - center.x;
    let y = p.y - center.y;
    point(
        center.x + x * a.cos() - y * a.sin(),
        center.y + x * a.sin() + y * a.cos(),
    )
}
#[derive(Debug, Clone, Copy)]
pub struct Drum {
    pub digit: i32,
    pub fraction: f64,
}
pub fn drum(value: f64, place: i32, minor: i32) -> Drum {
    let value = value.max(0.0);
    let units = value / f64::from(place);
    let remainder = value % f64::from(place);
    Drum {
        digit: units.floor() as i32 % 10,
        fraction: if place == minor {
            units - units.floor()
        } else {
            ((remainder - f64::from(place - minor)) / f64::from(minor)).clamp(0.0, 1.0)
        },
    }
}
pub fn tape_y(value: f64, mark: f64, span: f64) -> f64 {
    540.0 + (value - mark) * 580.0 / span
}
pub fn cdi_offset(dots: f64) -> f64 {
    dots.clamp(-2.0, 2.0) * 30.0
}
pub fn wind_label(speed: f64, offset: f64) -> String {
    if speed < 0.1 {
        return "CALM".into();
    }
    let label = if offset.abs() < 1.0 {
        "HEADWIND"
    } else if (offset.abs() - 180.0).abs() < 1.0 {
        "TAILWIND"
    } else if offset < 0.0 {
        "LEFT CROSSWIND"
    } else {
        "RIGHT CROSSWIND"
    };
    format!("{speed:.0} KT {label}")
}
#[derive(Debug, Clone, Copy)]
pub struct Trend {
    pub time: f64,
    pub ias: f64,
    pub heading: f64,
    pub acceleration: f64,
    pub turn_rate: f64,
}
impl Default for Trend {
    fn default() -> Self {
        Self {
            time: -1.0,
            ias: 0.0,
            heading: 0.0,
            acceleration: 0.0,
            turn_rate: 0.0,
        }
    }
}
impl Trend {
    pub fn update(&mut self, t: f64, speed: f64, hdg: f64) {
        if !speed.is_finite() || !hdg.is_finite() {
            return;
        }
        if self.time < 0.0 || t < self.time || t - self.time > 2.0 {
            *self = Self {
                time: t,
                ias: speed,
                heading: hdg,
                ..Self::default()
            };
            return;
        }
        let dt = t - self.time;
        if dt < 0.05 {
            return;
        }
        let blend = 1.0 - (-dt / 0.7).exp();
        self.acceleration += blend * ((speed - self.ias) / dt - self.acceleration);
        self.turn_rate += blend * (wrap(hdg - self.heading) / dt - self.turn_rate);
        self.time = t;
        self.ias = speed;
        self.heading = hdg;
    }
}
