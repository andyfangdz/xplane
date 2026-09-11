use super::*;
use poweroff180::hud::{drum, tape_y};

impl Scene {
    #[allow(clippy::too_many_arguments)] // Position, carry interval and clipping are explicit per drum.
    fn drum_digit(
        &mut self,
        x: f64,
        y: f64,
        value: f64,
        place: i32,
        minor: i32,
        size: f64,
        height: f64,
        c: Color,
        leading: bool,
    ) {
        let digit = drum(value, place, minor);
        self.clip(x - 17.0, y - height / 2.0, 34.0, height);
        for row in -1..=1 {
            let n = (digit.digit + row + 10) % 10;
            if leading && n == 0 && value + f64::from(row * place) < f64::from(place) {
                continue;
            }
            self.text(
                x,
                y - size * 0.5 + (digit.fraction - f64::from(row)) * size,
                n.to_string(),
                size,
                c,
                1,
            );
        }
        self.unclip();
    }
    fn altitude_drum(&mut self, x: f64, altitude: f64) {
        if !altitude.is_finite() {
            self.text(x + 82.0, 519.0, "----", 36.0, WHITE, 0);
            return;
        }
        let a = altitude.abs();
        self.drum_digit(x + 28.0, 540.0, a, 10000, 20, 36.0, 45.0, WHITE, true);
        self.drum_digit(x + 54.0, 540.0, a, 1000, 20, 39.0, 45.0, WHITE, true);
        self.drum_digit(x + 80.0, 540.0, a, 100, 20, 36.0, 45.0, WHITE, false);
        if altitude < 0.0 {
            self.text(x + 6.0, 524.0, "-", 30.0, WHITE, 0);
        }
        let units = a / 20.0;
        let frac = units - units.floor();
        let low = units.floor() as i32 % 5;
        self.clip(x + 100.0, 492.0, 66.0, 96.0);
        for row in -1..=1 {
            self.text(
                x + 130.0,
                522.0 + (frac - f64::from(row)) * 36.0,
                format!("{:02}", ((low + row + 5) % 5) * 20),
                32.0,
                WHITE,
                1,
            );
        }
        self.unclip();
    }
    fn speed_band(&mut self, speed: f64, lo: f64, hi: f64, x: f64, w: f64, c: Color) {
        if !speed.is_finite() || !lo.is_finite() || !hi.is_finite() || hi <= lo {
            return;
        }
        let top = tape_y(speed, hi, 60.0).clamp(250.0, 830.0);
        let bottom = tape_y(speed, lo, 60.0).clamp(250.0, 830.0);
        if bottom > top {
            self.rect(x, top, w, bottom - top, c);
        }
    }
}
impl Hud {
    pub(super) fn tapes(&mut self, d: &mut Scene, s: &[f64; 75], v: &Values) {
        let speed = v.get("sim/cockpit2/gauges/indicators/airspeed_kts_pilot");
        let alt = v.get("sim/cockpit2/gauges/indicators/altitude_ft_pilot");
        let vsi = v.get("sim/cockpit2/gauges/indicators/vvi_fpm_pilot");
        let selected = v.get("sim/cockpit2/autopilot/altitude_dial_ft");
        let baro = v.get("sim/cockpit2/gauges/actuators/barometer_setting_in_hg_pilot");
        self.trend.update(
            s[0],
            speed,
            v.get("sim/cockpit2/gauges/indicators/heading_AHARS_deg_mag_pilot"),
        );
        let (sx, sw, ax, aw) = (512.0, 132.0, 1320.0, 172.0);
        d.rect(sx, 250.0, sw, 580.0, TAPE);
        d.rect(ax, 250.0, aw, 580.0, TAPE);
        d.line(p(sx, 250.0), p(sx, 830.0), GRAY, 1.0);
        d.line(p(ax + aw, 250.0), p(ax + aw, 830.0), GRAY, 1.0);
        let vso = v.get("sim/aircraft/view/acf_Vso");
        let vs = v.get("sim/aircraft/view/acf_Vs");
        let vfe = self.full_flap_limit;
        let vno = v.get("sim/aircraft/view/acf_Vno");
        let vne = v.get("sim/aircraft/view/acf_Vne");
        d.speed_band(speed, 20.0, vso, sx + sw - 12.0, 12.0, RED);
        d.speed_band(speed, vs, vno, sx + sw - 12.0, 12.0, [0.1, 0.85, 0.08, 1.0]);
        d.speed_band(speed, vso, vfe, sx + sw - 6.0, 6.0, WHITE);
        d.speed_band(speed, vno, vne, sx + sw - 12.0, 12.0, [1.0, 0.94, 0.0, 1.0]);
        d.speed_band(speed, vne, 1000.0, sx + sw - 12.0, 12.0, RED);
        if speed.is_finite() && vne.is_finite() {
            d.clip(sx + sw - 12.0, 250.0, 12.0, 580.0);
            let red_end = tape_y(speed, vne, 60.0).clamp(250.0, 830.0);
            let mut y = 250.0;
            while y < red_end {
                d.poly(
                    &[
                        p(sx + sw - 12.0, y + 9.0),
                        p(sx + sw, y),
                        p(sx + sw, y + 8.0),
                        p(sx + sw - 12.0, y + 17.0),
                    ],
                    WHITE,
                    true,
                    2.0,
                );
                y += 18.0;
            }
            d.unclip();
        }
        if speed.is_finite() {
            d.clip(sx, 250.0, sw, 580.0);
            for k in ((speed - 30.0) / 5.0) as i32 - 1..((speed + 30.0) / 5.0) as i32 + 2 {
                if k * 5 < 20 {
                    continue;
                }
                let y = tape_y(speed, f64::from(k * 5), 60.0);
                let major = k % 2 == 0;
                d.line(
                    p(sx + sw - if major { 29.0 } else { 21.0 }, y),
                    p(sx + sw, y),
                    GRAY,
                    if major { 2.0 } else { 1.6 },
                );
                if major {
                    d.text(
                        sx + sw - 35.0,
                        y - 16.0,
                        num(f64::from(k * 5)),
                        28.0,
                        GRAY,
                        2,
                    );
                }
            }
            d.unclip();
            if self.trend.acceleration.abs() > 0.04 {
                let end =
                    tape_y(speed, speed + self.trend.acceleration * 6.0, 60.0).clamp(250.0, 830.0);
                d.line(p(sx + sw + 5.0, 540.0), p(sx + sw + 5.0, end), PINK, 4.0);
            }
        }
        if alt.is_finite() {
            d.clip(ax, 250.0, aw, 580.0);
            for k in ((alt - 300.0) / 20.0).floor() as i32 - 1..((alt + 300.0) / 20.0) as i32 + 2 {
                let y = tape_y(alt, f64::from(k * 20), 600.0);
                let major = k % 5 == 0;
                d.line(
                    p(ax, y),
                    p(ax + if major { 27.0 } else { 14.0 }, y),
                    GRAY,
                    if major { 2.0 } else { 1.5 },
                );
                if major {
                    d.text(ax + 38.0, y - 16.0, num(f64::from(k * 20)), 28.0, GRAY, 0);
                }
            }
            d.unclip();
            if vsi.is_finite() && vsi.abs() > 20.0 {
                let end = tape_y(alt, alt + vsi * 0.1, 600.0).clamp(250.0, 830.0);
                d.line(p(ax - 5.0, 540.0), p(ax - 5.0, end), PINK, 4.0);
            }
            if selected.is_finite() {
                let y = tape_y(alt, selected, 600.0).clamp(259.0, 821.0);
                d.poly(
                    &[
                        p(ax, y - 13.0),
                        p(ax + 13.0, y - 13.0),
                        p(ax + 13.0, y - 6.0),
                        p(ax + 4.0, y),
                        p(ax + 13.0, y + 6.0),
                        p(ax + 13.0, y + 13.0),
                        p(ax, y + 13.0),
                    ],
                    CYAN,
                    false,
                    3.0,
                );
            }
        }
        let speed_back = if speed.is_finite() && speed >= vne {
            RED
        } else {
            GBLACK
        };
        let right = sx + sw - 8.0;
        d.rect(sx - 2.0, 515.0, sw - 6.0, 50.0, speed_back);
        d.rect(right - 28.0, 492.0, 29.0, 96.0, speed_back);
        d.poly(
            &[p(right, 526.0), p(sx + sw + 3.0, 540.0), p(right, 554.0)],
            speed_back,
            true,
            2.0,
        );
        d.poly(
            &[
                p(sx - 2.0, 515.0),
                p(right - 28.0, 515.0),
                p(right - 28.0, 492.0),
                p(right + 1.0, 492.0),
                p(right + 1.0, 526.0),
                p(sx + sw + 3.0, 540.0),
                p(right + 1.0, 554.0),
                p(right + 1.0, 588.0),
                p(right - 28.0, 588.0),
                p(right - 28.0, 565.0),
                p(sx - 2.0, 565.0),
            ],
            GRAY,
            false,
            1.4,
        );
        let speed_color =
            if speed.is_finite() && speed < vne && speed + self.trend.acceleration * 6.0 >= vne {
                GOLD
            } else {
                WHITE
            };
        if speed.is_finite() && speed >= 20.0 {
            d.drum_digit(
                right - 73.0,
                540.0,
                speed,
                100,
                1,
                38.0,
                46.0,
                speed_color,
                true,
            );
            d.drum_digit(
                right - 43.0,
                540.0,
                speed,
                10,
                1,
                38.0,
                46.0,
                speed_color,
                false,
            );
            d.drum_digit(
                right - 13.0,
                540.0,
                speed,
                1,
                1,
                36.0,
                94.0,
                speed_color,
                false,
            );
        } else {
            d.text(sx + sw / 2.0, 520.0, "--", 36.0, WHITE, 1);
        }
        d.rect(ax + 13.0, 515.0, aw - 13.0, 50.0, GBLACK);
        d.rect(ax + 100.0, 492.0, aw - 100.0, 96.0, GBLACK);
        d.poly(
            &[p(ax + 13.0, 526.0), p(ax, 540.0), p(ax + 13.0, 554.0)],
            GBLACK,
            true,
            2.0,
        );
        d.poly(
            &[
                p(ax + 13.0, 515.0),
                p(ax + 100.0, 515.0),
                p(ax + 100.0, 492.0),
                p(ax + aw, 492.0),
                p(ax + aw, 588.0),
                p(ax + 100.0, 588.0),
                p(ax + 100.0, 565.0),
                p(ax + 13.0, 565.0),
                p(ax + 13.0, 554.0),
                p(ax, 540.0),
                p(ax + 13.0, 526.0),
            ],
            GRAY,
            false,
            1.4,
        );
        d.altitude_drum(ax, alt);
        d.rect(sx, 831.0, sw, 32.0, GBLACK);
        d.line(p(sx, 831.0), p(sx + sw, 831.0), GRAY, 1.0);
        d.text(sx + 5.0, 839.0, "TAS", 15.0, GRAY, 0);
        d.text(
            sx + sw - 4.0,
            835.0,
            format!("{}KT", num(s[26] * 1.94384449)),
            22.0,
            WHITE,
            2,
        );
        d.rect(ax, 831.0, aw, 32.0, GBLACK);
        d.line(p(ax, 831.0), p(ax + aw, 831.0), GRAY, 1.0);
        d.text(
            ax + aw / 2.0,
            835.0,
            format!("{} IN", number(baro, 2, false)),
            25.0,
            CYAN,
            1,
        );
        d.rect(ax, 209.0, aw, 40.0, GBLACK);
        d.poly(
            &[
                p(ax, 209.0),
                p(ax + aw, 209.0),
                p(ax + aw, 249.0),
                p(ax, 249.0),
            ],
            GRAY,
            false,
            1.0,
        );
        d.poly(
            &[
                p(ax + 8.0, 218.0),
                p(ax + 19.0, 218.0),
                p(ax + 13.0, 226.0),
                p(ax + 19.0, 235.0),
                p(ax + 8.0, 235.0),
            ],
            CYAN,
            true,
            2.0,
        );
        d.text(ax + aw / 2.0 + 8.0, 214.0, num(selected), 30.0, CYAN, 1);
        d.text(sx + sw / 2.0, 220.0, "KIAS", 20.0, GRAY, 1);
        d.text(sx, 876.0, format!("GS {} KT", num(s[3])), 21.0, WHITE, 0);
        d.text(ax, 876.0, format!("AGL {} FT", num(s[1])), 21.0, WHITE, 0);
        let (vx, vy, vr, vw) = (ax + aw, 540.0, 218.0, 69.0);
        d.poly(
            &[
                p(vx, 282.0),
                p(vx + vw, 282.0),
                p(vx + vw, 502.0),
                p(vx, 540.0),
            ],
            TAPE,
            true,
            2.0,
        );
        d.poly(
            &[
                p(vx, 540.0),
                p(vx + vw, 578.0),
                p(vx + vw, 798.0),
                p(vx, 798.0),
            ],
            TAPE,
            true,
            2.0,
        );
        d.poly(
            &[
                p(vx, 282.0),
                p(vx + vw, 282.0),
                p(vx + vw, 502.0),
                p(vx, 540.0),
                p(vx + vw, 578.0),
                p(vx + vw, 798.0),
                p(vx, 798.0),
            ],
            GRAY,
            false,
            1.2,
        );
        for value in [-2000_i32, -1500, -1000, -500, 500, 1000, 1500, 2000] {
            let y = vy - f64::from(value) * vr / 2000.0;
            d.line(
                p(vx, y),
                p(vx + if value % 1000 == 0 { 23.0 } else { 12.0 }, y),
                GRAY,
                1.8,
            );
            if value % 1000 == 0 {
                d.text(
                    vx + 40.0,
                    y - 13.0,
                    num(f64::from(value.abs() / 1000)),
                    23.0,
                    WHITE,
                    1,
                );
            }
        }
        if vsi.is_finite() {
            let y = vy - vsi.clamp(-2000.0, 2000.0) * vr / 2000.0;
            let width = if vsi.abs() > 100.0 { 126.0 } else { 69.0 };
            let pointer = [
                p(vx, y),
                p(vx + 22.0, y - 17.0),
                p(vx + width, y - 17.0),
                p(vx + width, y + 17.0),
                p(vx + 22.0, y + 17.0),
            ];
            d.poly(&pointer, GBLACK, true, 2.0);
            d.poly(&pointer, GRAY, false, 1.4);
            if vsi.abs() > 100.0 {
                d.text(
                    vx + width - 7.0,
                    y - 14.0,
                    number((vsi / 10.0).round() * 10.0, 0, true),
                    25.0,
                    WHITE,
                    2,
                );
            }
        }
    }
}
