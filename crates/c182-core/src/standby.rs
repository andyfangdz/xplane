//! The existing NAV III standby-pack model, including transfer and TEST history.
pub const CAPACITY_AH: f64 = 2.4;
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct State {
    pub remaining_ah: f64,
    pub test_elapsed: f64,
    pub flags: u32,
    pub previous_mode: i32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            remaining_ah: CAPACITY_AH,
            test_elapsed: 0.0,
            flags: 1,
            previous_mode: 0,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub mode: i32,
    pub main_bus_volts: f64,
    pub dt_seconds: f64,
    pub essential_load_amps: f64,
    pub enabled: bool,
    pub paused: bool,
    pub replay: bool,
}
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Output {
    /// valid, active, powering essentials, TEST lamp, TEST passed.
    pub flags: u32,
    pub essential_bus_volts: f64,
    pub battery_volts: f64,
    pub current_amps: f64,
    pub state_of_charge: f64,
    pub test_elapsed_seconds: f64,
}
impl State {
    pub fn reset(&mut self, soc: f64) {
        *self = Self {
            remaining_ah: CAPACITY_AH
                * if soc.is_finite() {
                    soc.clamp(0.0, 1.0)
                } else {
                    0.0
                },
            ..Self::default()
        };
    }
    fn battery_volts(&self) -> f64 {
        let soc = self.remaining_ah / CAPACITY_AH;
        if soc >= 0.2 {
            24.0 + 1.2 * (soc - 0.2) / 0.8
        } else {
            20.0 + 20.0 * soc
        }
    }
    fn discharge(&mut self, amps: f64, seconds: f64) {
        if amps <= 0.0 {
            return;
        }
        let empty_seconds = self.remaining_ah * 3600.0 / amps;
        if seconds >= empty_seconds {
            self.remaining_ah = 0.0;
            return;
        }
        self.remaining_ah = (self.remaining_ah - amps * (seconds / 3600.0)).max(0.0);
    }
    fn publish_state(&self, out: &mut Output) {
        out.battery_volts = self.battery_volts();
        out.state_of_charge = self.remaining_ah / CAPACITY_AH;
    }
    pub fn tick(&mut self, input: Inputs) -> Output {
        let mut out = Output::default();
        self.publish_state(&mut out);
        if !(0..=2).contains(&input.mode)
            || !input.main_bus_volts.is_finite()
            || input.main_bus_volts < 0.0
            || input.main_bus_volts > 100.0
            || !input.dt_seconds.is_finite()
            || input.dt_seconds < 0.0
            || !input.essential_load_amps.is_finite()
            || input.essential_load_amps < 0.0
            || input.essential_load_amps >= 4.0
        {
            if self.previous_mode == 2 {
                self.flags &= !1;
            }
            return out;
        }
        out.flags = 1;
        out.essential_bus_volts = input.main_bus_volts;
        if !input.enabled || input.paused || input.replay {
            return out;
        }
        out.flags |= 2;
        if input.mode != 2 || self.previous_mode != 2 {
            self.test_elapsed = 0.0;
            self.flags |= 1;
        }
        self.previous_mode = input.mode;
        if input.mode == 0 {
            self.flags &= !2;
            return out;
        }
        if input.mode == 2 {
            self.flags &= !2;
            let before = self.remaining_ah >= 2.0 && self.battery_volts() >= 24.0;
            self.discharge(4.0, input.dt_seconds);
            let after = self.remaining_ah >= 2.0 && self.battery_volts() >= 24.0;
            if !before || !after {
                self.flags &= !1;
            }
            self.test_elapsed = (self.test_elapsed + input.dt_seconds.min(20.0)).min(20.0);
            out.current_amps = if self.remaining_ah > 0.0 { -4.0 } else { 0.0 };
            if after {
                out.flags |= 8;
            }
            out.test_elapsed_seconds = self.test_elapsed;
            if self.test_elapsed >= 20.0 && self.flags & 1 != 0 {
                out.flags |= 16;
            }
            self.publish_state(&mut out);
            return out;
        }
        if self.flags & 2 != 0 {
            if input.main_bus_volts >= 20.5 {
                self.flags &= !2;
            }
        } else if input.main_bus_volts < 20.0 {
            self.flags |= 2;
        }
        if self.flags & 2 != 0 {
            self.discharge(input.essential_load_amps, input.dt_seconds);
            if self.remaining_ah > 0.0 {
                out.flags |= 4;
                out.essential_bus_volts = self.battery_volts();
                out.current_amps = -input.essential_load_amps;
            }
        } else if input.main_bus_volts >= 25.5 && self.remaining_ah < CAPACITY_AH {
            let full_seconds = (CAPACITY_AH - self.remaining_ah) * 3600.0 / 1.0;
            if input.dt_seconds >= full_seconds {
                self.remaining_ah = CAPACITY_AH;
            } else {
                self.remaining_ah =
                    (self.remaining_ah + 1.0 * (input.dt_seconds / 3600.0)).min(CAPACITY_AH);
            }
            out.current_amps = if self.remaining_ah < CAPACITY_AH {
                1.0
            } else {
                0.0
            };
        }
        self.publish_state(&mut out);
        out
    }
}
