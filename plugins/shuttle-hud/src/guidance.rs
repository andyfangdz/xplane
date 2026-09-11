//! The accepted landing calibration, translated without tuning changes.
use crate::math::{deg, rad, FT};
pub fn chute_area_ratio(seconds: f64) -> f64 {
    fn smooth(v: f64) -> f64 {
        let v = v.clamp(0.0, 1.0);
        v * v * (3.0 - 2.0 * v)
    }
    if seconds < 1.5 {
        0.001 + 0.159 * smooth(seconds / 1.5)
    } else {
        0.16 + 0.84 * smooth((seconds - 5.0) / 0.5)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PathPoint {
    pub height: f64,
    pub slope: f64,
    pub curvature: f64,
    pub segment: i32,
}
pub struct LandingPath;
impl LandingPath {
    pub const OUTER_AIM: f64 = -7500.0 / FT;
    pub const INNER_AIM: f64 = 1000.0 / FT;
    pub const CIRCLE_START: f64 = -12170.711613072857 / FT;
    pub const EXP_START: f64 = -4562.0 / FT;
    pub const RADIUS: f64 = 26069.178277669853 / FT;
    pub const EXP_HEIGHT: f64 = 14.99818857362294 / FT;
    pub const EXP_LENGTH: f64 = 624.112267429558 / FT;
    pub fn at(x: f64) -> PathPoint {
        let (mo, mi) = (rad(20.0).tan(), rad(1.5).tan());
        if x <= Self::CIRCLE_START {
            return PathPoint {
                height: (Self::OUTER_AIM - x) * mo,
                slope: -mo,
                curvature: 0.0,
                segment: 1,
            };
        }
        if x < Self::EXP_START {
            let xc = Self::CIRCLE_START + Self::RADIUS * rad(20.0).sin();
            let yc = 1700.0 / FT + Self::RADIUS * rad(20.0).cos();
            let dx = x - xc;
            let z = (Self::RADIUS * Self::RADIUS - dx * dx).sqrt();
            return PathPoint {
                height: yc - z,
                slope: dx / z,
                curvature: Self::RADIUS * Self::RADIUS / (z * z * z),
                segment: 2,
            };
        }
        let e = Self::EXP_HEIGHT * (-(x - Self::EXP_START) / Self::EXP_LENGTH).exp();
        PathPoint {
            height: (Self::INNER_AIM - x) * mi + e,
            slope: -mi - e / Self::EXP_LENGTH,
            curvature: e / (Self::EXP_LENGTH * Self::EXP_LENGTH),
            segment: 3,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct GuidanceInput {
    pub time: f64,
    pub along: f64,
    pub height: f64,
    pub groundspeed: f64,
    pub vy: f64,
    pub eas: f64,
    pub mass_lb: f64,
    pub main_wow: bool,
}
impl Default for GuidanceInput {
    fn default() -> Self {
        Self {
            time: 0.0,
            along: 0.0,
            height: 0.0,
            groundspeed: 0.0,
            vy: 0.0,
            eas: 0.0,
            mass_lb: 184000.0,
            main_wow: false,
        }
    }
}
#[derive(Clone, Debug)]
pub struct LandingGuidance {
    pub last_time: f64,
    pub speedbrake: f64,
    pub gear: f64,
    pub target_height: f64,
    pub gamma: f64,
    pub flare_height: f64,
    pub touch_time: f64,
    pub retract_3000: bool,
    pub adjust_500: bool,
    pub final_flare: bool,
    pub phase: i32,
}
impl Default for LandingGuidance {
    fn default() -> Self {
        Self {
            last_time: -1.0,
            speedbrake: 0.0,
            gear: 0.0,
            target_height: 0.0,
            gamma: -20.0,
            flare_height: 50.0 / FT,
            touch_time: -1.0,
            retract_3000: false,
            adjust_500: false,
            final_flare: false,
            phase: 1,
        }
    }
}
impl LandingGuidance {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(&mut self, i: GuidanceInput) {
        if self.last_time < 0.0 || i.time < self.last_time {
            self.reset();
        }
        let dt = if self.last_time < 0.0 {
            0.0
        } else {
            (i.time - self.last_time).clamp(0.0, 0.2)
        };
        self.last_time = i.time;
        let mut p = LandingPath::at(i.along);
        let heavy = ((i.mass_lb - 184000.0) / 42040.0).clamp(0.0, 1.0);
        let u = ((400.0 - p.height * FT) / 300.0).clamp(0.0, 1.0);
        let offset_feet = 28.0 - 8.0 * heavy;
        let offset = offset_feet / FT * u * u * u * (10.0 - 15.0 * u + 6.0 * u * u);
        p.height -= offset;
        p.slope *= 1.0 + (offset_feet / 300.0) * 30.0 * u * u * (1.0 - u) * (1.0 - u);
        self.target_height = p.height;
        let gs = i.groundspeed.max(35.0);
        let mut vcmd = gs * p.slope + (0.18 * (p.height - i.height)).clamp(-12.0, 12.0);
        self.phase = if i.height > 2000.0 / FT { 1 } else { 2 };
        if !self.final_flare {
            self.flare_height = (-i.vy * 4.2).clamp(30.0 / FT, 80.0 / FT);
        }
        if i.height <= self.flare_height && i.along > -2500.0 / FT {
            self.final_flare = true;
        }
        if self.final_flare {
            self.phase = 3;
            let upper_sink = 1.5 + 0.1 * heavy;
            vcmd = -0.9144 - (upper_sink - 0.9144) * (i.height / (15.0 / FT)).clamp(0.0, 1.0);
        }
        self.gamma = deg(vcmd.atan2(gs)).clamp(-24.0, 2.0);
        if i.height <= 300.0 / FT {
            self.gear = 1.0;
        }
        if !self.retract_3000 && i.height > 3000.0 / FT {
            let cmd = (0.45 + 0.035 * (i.eas - 300.0)).clamp(0.0, 0.78);
            self.speedbrake += (cmd - self.speedbrake).clamp(-0.5 * dt, 0.5 * dt);
        } else if !self.retract_3000 {
            self.retract_3000 = true;
            self.speedbrake = 0.105 + 0.145 * heavy;
        }
        if !self.adjust_500 && i.height <= 500.0 / FT {
            self.adjust_500 = true;
            self.speedbrake = 0.105 + 0.145 * heavy;
        }
        if i.main_wow {
            self.phase = 4;
            self.speedbrake = 1.0;
            self.gear = 1.0;
            if self.touch_time < 0.0 {
                self.touch_time = i.time;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_path_and_chute_stages() {
        assert_eq!(chute_area_ratio(0.0), 0.001);
        assert!((chute_area_ratio(1.5) - 0.16).abs() < 1e-9);
        assert_eq!(chute_area_ratio(4.0), 0.16);
        assert_eq!(chute_area_ratio(6.0), 1.0);
        for x in [LandingPath::CIRCLE_START, LandingPath::EXP_START] {
            let a = LandingPath::at(x - 1e-5);
            let b = LandingPath::at(x + 1e-5);
            assert!((a.height - b.height).abs() < 1e-4);
            assert!((a.slope - b.slope).abs() < 1e-7);
            if x == LandingPath::EXP_START {
                assert!((a.curvature - b.curvature).abs() < 1e-9);
            }
        }
        assert!((LandingPath::at(LandingPath::CIRCLE_START).height * FT - 1700.0).abs() < 0.001);
        assert!((LandingPath::at(0.0).height * FT - 26.2).abs() < 0.02);
        let mut last = f64::INFINITY;
        for x in -7000..100 {
            let p = LandingPath::at(f64::from(x));
            assert!(p.height.is_finite() && p.height < last && p.slope < 0.0);
            last = p.height;
        }
    }
    #[test]
    fn event_latches_and_time_reset() {
        let mut g = LandingGuidance::default();
        let mut i = GuidanceInput {
            time: 1.0,
            height: 4000.0 / FT,
            along: -6500.0,
            eas: 300.0,
            groundspeed: 165.0,
            vy: -55.0,
            ..Default::default()
        };
        g.update(i);
        i.time = 2.0;
        i.height = 2900.0 / FT;
        g.update(i);
        let sb = g.speedbrake;
        i.time = 3.0;
        i.height = 1500.0 / FT;
        i.eas = 315.0;
        g.update(i);
        assert_eq!(g.speedbrake, sb);
        i.time = 4.0;
        i.height = 490.0 / FT;
        g.update(i);
        let sb = g.speedbrake;
        i.time = 5.0;
        i.height = 250.0 / FT;
        g.update(i);
        assert_eq!(g.speedbrake, sb);
        assert_eq!(g.gear, 1.0);
        i.time = 6.0;
        i.along = -200.0;
        i.height = 35.0 / FT;
        i.vy = -3.0;
        g.update(i);
        assert_eq!(g.phase, 3);
        i.time = 7.0;
        i.main_wow = true;
        g.update(i);
        assert_eq!(g.phase, 4);
        i.time = 0.0;
        i.main_wow = false;
        i.height = 4000.0 / FT;
        i.along = -6500.0;
        g.update(i);
        assert!(!g.final_flare && !g.retract_3000);
        assert_eq!(g.gear, 0.0);
    }
}
