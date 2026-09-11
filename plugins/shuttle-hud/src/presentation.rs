//! Handbook display sequencing; geometry-derived phases are a local approximation.
use crate::math::{advance_fade, clutter};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum HudPhase {
    #[default]
    Acq,
    Hdg,
    Prfnl,
    Capt,
    Ogs,
    Flare,
    Fnlfl,
}
impl HudPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Acq => "ACQ",
            Self::Hdg => "HDG",
            Self::Prfnl => "PRFNL",
            Self::Capt => "CAPT",
            Self::Ogs => "OGS",
            Self::Flare => "FLARE",
            Self::Fnlfl => "FNLFL",
        }
    }
}
pub fn digital_height(h: f64) -> i32 {
    let h = h.clamp(0.0, 32767.0);
    let step = if h > 1000.0 {
        200.0
    } else if h > 400.0 {
        100.0
    } else if h > 50.0 {
        10.0
    } else {
        1.0
    };
    ((h / step).floor() * step) as i32
}
pub fn altitude_step(h: f64) -> f64 {
    if h > 100000.0 {
        10000.0
    } else if h > 1000.0 {
        1000.0
    } else if h > 500.0 {
        100.0
    } else {
        50.0
    }
}
pub fn indicated_speed(s: f64) -> i32 {
    s.clamp(0.0, 500.0) as i32
}
#[derive(Clone, Copy, Debug, Default)]
pub struct HudInput {
    pub time: f64,
    pub height_ft: f64,
    pub eas: f64,
    pub groundspeed: f64,
    pub heading_error: f64,
    pub cross_ft: f64,
    pub path_error_ft: f64,
    pub gamma_error: f64,
    pub bank: f64,
    pub stop_distance: f64,
    pub along: f64,
    pub gear: [f64; 3],
    pub main: bool,
    pub nose: bool,
    pub final_flare: bool,
    pub automatic: bool,
    pub replay: bool,
}
#[derive(Clone, Debug)]
pub struct HudPresentation {
    pub phase: HudPhase,
    pub main: bool,
    pub nose: bool,
    pub replay: bool,
    pub automatic: bool,
    pub last_time: f64,
    pub last_height: f64,
    pub last_along: f64,
    pub last_speed: f64,
    pub fade: f64,
    pub gear_lock_time: f64,
    pub capture_seconds: f64,
    pub deceleration: f64,
    pub required_deceleration: f64,
    pub ground_declutter: i32,
    pub gear_cue: i32,
}
impl Default for HudPresentation {
    fn default() -> Self {
        Self {
            phase: HudPhase::Acq,
            main: false,
            nose: false,
            replay: false,
            automatic: false,
            last_time: -1.0,
            last_height: 0.0,
            last_along: 0.0,
            last_speed: 0.0,
            fade: 0.0,
            gear_lock_time: -1.0,
            capture_seconds: 0.0,
            deceleration: 0.0,
            required_deceleration: 0.0,
            ground_declutter: 0,
            gear_cue: 0,
        }
    }
}
impl HudPresentation {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(&mut self, i: HudInput) {
        if self.last_time >= 0.0
            && (i.time < self.last_time
                || i.replay != self.replay
                || (i.height_ft - self.last_height).abs() > 500.0
                || (i.along - self.last_along).abs() > 500.0
                || (self.main
                    && !i.main
                    && (i.height_ft > 50.0 || (i.time < 3.0 && i.height_ft > 5.0))))
        {
            self.reset();
        }
        let dt = if self.last_time < 0.0 {
            0.0
        } else {
            (i.time - self.last_time).clamp(0.0, 0.2)
        };
        self.replay = i.replay;
        self.automatic = i.automatic;
        if !self.main && (i.main || i.nose) {
            self.main = true;
            self.ground_declutter = 0;
        }
        self.nose |= self.main && i.nose;
        let prefinal = i.heading_error.abs() < 35.0 && i.height_ft < 18000.0;
        if !self.main {
            if i.final_flare {
                self.phase = HudPhase::Fnlfl;
            } else if i.height_ft <= 2000.0 {
                self.phase = HudPhase::Flare;
            } else if self.phase < HudPhase::Capt {
                self.phase = if prefinal {
                    HudPhase::Prfnl
                } else if i.bank.abs() > 10.0 {
                    HudPhase::Hdg
                } else {
                    HudPhase::Acq
                };
                if i.height_ft <= 5000.0
                    || (prefinal
                        && i.height_ft < 10000.0
                        && i.path_error_ft.abs() < 1000.0
                        && i.cross_ft.abs() < 1000.0
                        && i.gamma_error.abs() < 4.0
                        && i.eas >= 288.0
                        && i.eas <= 312.0)
                {
                    self.phase = HudPhase::Capt;
                }
            }
            if self.phase == HudPhase::Capt {
                self.capture_seconds = if i.gamma_error.abs() < 2.0 {
                    self.capture_seconds + dt
                } else {
                    0.0
                };
                if (i.path_error_ft.abs() < 50.0 && i.gamma_error.abs() < 2.0)
                    || self.capture_seconds >= 4.0
                {
                    self.phase = HudPhase::Ogs;
                }
            }
        }
        self.fade = advance_fade(
            self.fade,
            self.last_time,
            i.time,
            prefinal || self.phase >= HudPhase::Capt,
        );
        let locked = i.gear.iter().all(|g| *g >= 0.99);
        let up = i.gear.iter().all(|g| *g <= 0.01);
        if locked {
            if self.gear_lock_time < 0.0 {
                self.gear_lock_time = i.time;
            }
        } else {
            self.gear_lock_time = -1.0;
        }
        self.gear_cue = if self.main {
            0
        } else if locked {
            if i.time - self.gear_lock_time < 5.0 {
                3
            } else {
                0
            }
        } else if !up {
            2
        } else if i.height_ft < 300.0 && i.eas < 300.0 {
            1
        } else {
            0
        };
        if self.main {
            if dt > 0.0 {
                let a = ((self.last_speed - i.groundspeed) / (dt * 9.80665)).clamp(-0.4, 0.8);
                self.deceleration += (a - self.deceleration) * (1.0 - (-dt / 0.5).exp());
            }
            self.required_deceleration =
                i.groundspeed * i.groundspeed / (2.0 * i.stop_distance.max(1.0) * 9.80665);
        }
        self.last_time = i.time;
        self.last_height = i.height_ft;
        self.last_along = i.along;
        self.last_speed = i.groundspeed;
    }
    pub fn level(&self, mode: i32, height: f64, visible: bool) -> i32 {
        if self.main {
            if self.ground_declutter == 2 {
                3
            } else {
                self.ground_declutter
            }
        } else if mode < 0 {
            clutter(height, visible, mode)
        } else {
            mode.clamp(0, 3)
        }
    }
    pub fn guidance_visible(&self) -> bool {
        !self.main && (self.phase != HudPhase::Fnlfl || self.automatic)
    }
    pub fn nz_visible(&self) -> bool {
        !self.main && self.phase < HudPhase::Prfnl
    }
    pub fn attitude_visible(&self, level: i32) -> bool {
        !self.nose && (if self.main { level == 0 } else { level < 2 })
    }
    pub fn horizon_visible(&self, level: i32) -> bool {
        !self.nose && level < 3 && (!self.main || level == 0)
    }
    pub fn flags(&self, level: i32) -> i32 {
        if level == 3 {
            return 1;
        }
        1 | (i32::from(!self.main) * 2)
            | (i32::from(self.guidance_visible()) * 4)
            | (i32::from(self.horizon_visible(level)) * 8)
            | (i32::from(self.attitude_visible(level)) * 16)
            | (i32::from(!self.main && level < 2) * 32)
            | (i32::from(self.main) * 64)
            | (i32::from(self.nz_visible()) * 128)
            | (i32::from(!self.main && level == 0) * 256)
    }
}
