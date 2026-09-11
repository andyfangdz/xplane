use std::collections::VecDeque;
use xplane_units::{
    acceleration::standard_gravity,
    angle::degree,
    degrees,
    length::{foot, meter},
    meters, meters_per_second,
    ratio::ratio,
    seconds,
    velocity::meter_per_second,
    Time, Velocity,
};

use super::config::RatingScale;
use super::datarefs::DataRefs;
use super::support::angular_delta;
use xplane_airports::{GeoPoint, RunwayDatabase, RunwayMatch, TouchdownMetrics};

const FIFTY_FEET_M: f32 = 15.24;
const MAX_RESULT_LINES: usize = 9;

#[derive(Copy, Clone, Debug, Default)]
struct Sample {
    time: Time,
    vertical_speed: Velocity,
    g: f64,
    filtered_g: f64,
}

#[derive(Copy, Clone, Debug)]
struct ApproachObservation {
    height_agl_m: f32,
    ias: f32,
    pitch_deg: f32,
}

#[derive(Copy, Clone, Debug)]
pub(super) struct FiftyFootMetrics {
    pub(super) ias: f32,
    pub(super) pitch_deg: f32,
}

#[derive(Clone, Debug)]
pub(super) struct LandingResult {
    pub(super) vertical_speed_mps: f32,
    pub(super) touchdown_pitch_deg: f32,
    pub(super) crab_angle_deg: f32,
    pub(super) fifty_foot: Option<FiftyFootMetrics>,
    pub(super) g: f32,
    pub(super) ias: f32,
    pub(super) vls: Option<f32>,
    pub(super) metrics: Option<TouchdownMetrics>,
    pub(super) crossing_height_m: Option<f64>,
    pub(super) nose_wheel_distance_m: Option<f64>,
}

impl LandingResult {
    pub(super) fn lines(
        &self,
        ratings: &RatingScale,
        ias_multiplier: f32,
        ias_unit: &str,
        toliss: bool,
    ) -> Vec<String> {
        let mut lines = vec![ratings.text_for(self.vertical_speed_mps).to_owned()];
        let ias_label = if ias_unit == "kts" {
            "KIAS".to_owned()
        } else {
            format!("{ias_unit} IAS")
        };
        if let Some(fifty_foot) = self.fifty_foot {
            lines.push(format!(
                "50'  |  {:.0} {ias_label}  |  {:.1}° pitch",
                fifty_foot.ias * ias_multiplier,
                fifty_foot.pitch_deg
            ));
        } else {
            lines.push("50'  |  IAS / pitch unavailable".to_owned());
        }
        if let Some(metrics) = &self.metrics {
            let crossing_height = self.crossing_height_m.unwrap_or(0.0);
            lines.push(format!(
                "Threshold {}/{}  |  {:.0} ft",
                metrics.airport,
                metrics.runway,
                meters(crossing_height).get::<foot>()
            ));
        } else {
            lines.push("Threshold  |  runway unavailable".to_owned());
        }
        lines.push(format!(
            "Touchdown  |  {:.0} fpm  |  {:.2} G",
            super::config::report_fpm(xplane_units::f32::Velocity::new::<meter_per_second>(
                self.vertical_speed_mps
            )),
            self.g
        ));
        lines.push(format!(
            "TD attitude  |  {:.1}° pitch  |  {:+.1}° crab",
            self.touchdown_pitch_deg, self.crab_angle_deg
        ));
        if self.ias > 0.0 {
            if let Some(vls) = self.vls.filter(|value| *value > 0.0) {
                lines.push(format!(
                    "TD speed  |  {:.0} {ias_label}  |  VLS {:.0}",
                    self.ias * ias_multiplier,
                    vls
                ));
            } else {
                lines.push(format!(
                    "TD speed  |  {:.0} {ias_label}",
                    self.ias * ias_multiplier
                ));
            }
        }
        if let Some(metrics) = &self.metrics {
            lines.push(format!(
                "{}  |  {:.0} ft from threshold",
                if toliss { "Main wheels" } else { "TD point" },
                metrics.distance_from_threshold.get::<foot>()
            ));
            if let Some(distance) = self
                .nose_wheel_distance_m
                .filter(|distance| *distance > 0.0)
            {
                lines.push(format!(
                    "Nose wheel  |  {:.0} ft from threshold",
                    meters(distance).get::<foot>()
                ));
            }
            lines.push(format!(
                "Centerline  |  {:+.0} ft  |  {:+.1}°",
                metrics.centerline_deviation.get::<foot>(),
                metrics.centerline_angle.get::<degree>()
            ));
        }
        lines.truncate(MAX_RESULT_LINES);
        lines
    }
}

#[derive(Default)]
pub(super) struct LandingUpdate {
    pub(super) show_overlay: bool,
    pub(super) result_changed: bool,
    pub(super) finished: bool,
    pub(super) teleported: bool,
}

pub(super) struct LandingTracker {
    last_on_ground: bool,
    air_time: f32,
    touchdown_captured: bool,
    update_remaining: f32,
    loops_after_touchdown: u32,
    samples: VecDeque<Sample>,
    active_runway: Option<RunwayMatch>,
    crossing_height_m: Option<f64>,
    last_approach_observation: Option<ApproachObservation>,
    fifty_foot: Option<FiftyFootMetrics>,
    last_position: Option<GeoPoint>,
    pub(super) result: Option<LandingResult>,
}

impl Default for LandingTracker {
    fn default() -> Self {
        Self {
            last_on_ground: true,
            air_time: 0.0,
            touchdown_captured: false,
            update_remaining: 0.0,
            loops_after_touchdown: 0,
            samples: VecDeque::with_capacity(4),
            active_runway: None,
            crossing_height_m: None,
            last_approach_observation: None,
            fifty_foot: None,
            last_position: None,
            result: None,
        }
    }
}

impl LandingTracker {
    pub(super) fn reset(&mut self, on_ground: bool) {
        *self = Self::default();
        self.last_on_ground = on_ground;
    }

    pub(super) fn tick(
        &mut self,
        datarefs: &DataRefs,
        runways: Option<&RunwayDatabase>,
        elapsed: f32,
    ) -> (f32, LandingUpdate) {
        let elapsed = elapsed.max(0.001);
        let position = GeoPoint {
            lat: datarefs.latitude.get_f32() as f64,
            lon: datarefs.longitude.get_f32() as f64,
            elevation: meters(datarefs.elevation.get_f32() as f64),
        };
        let height_agl = datarefs.height_agl.get_f32();
        let heading = datarefs.true_heading.get_f32() as f64;
        let ground_track = datarefs.ground_track.get_f32() as f64;
        let on_ground = datarefs.on_ground();
        let teleported = self.last_position.is_some_and(|last| {
            geo_distance(last, position) / seconds(elapsed as f64) > meters_per_second(3.0 * 340.0)
        });
        self.last_position = Some(position);
        let mut update = LandingUpdate {
            teleported,
            ..LandingUpdate::default()
        };

        if !on_ground {
            if height_agl > 10.0 {
                self.air_time += elapsed;
            }
            if self.air_time > 15.0 {
                let observation = ApproachObservation {
                    height_agl_m: height_agl,
                    ias: datarefs.ias.get_f32(),
                    pitch_deg: datarefs.pitch.get_f32(),
                };
                if height_agl > FIFTY_FEET_M
                    && self
                        .last_approach_observation
                        .is_some_and(|previous| previous.height_agl_m <= FIFTY_FEET_M)
                {
                    self.fifty_foot = None;
                }
                if self.fifty_foot.is_none() {
                    self.fifty_foot = self
                        .last_approach_observation
                        .and_then(|previous| fifty_foot_crossing(previous, observation));
                }
                self.last_approach_observation = Some(observation);
            }
            if height_agl < 150.0 {
                if self.active_runway.is_none() {
                    if let Some(database) = runways {
                        self.active_runway = database.find_approach(position, degrees(heading));
                        if let Some(runway_match) = self.active_runway {
                            let metrics =
                                database.metrics(runway_match, position, degrees(heading));
                            self.crossing_height_m = Some(
                                (position.elevation - metrics.threshold_elevation).get::<meter>(),
                            );
                        }
                    }
                }
            } else if height_agl > 200.0 {
                self.active_runway = None;
                self.crossing_height_m = None;
                self.last_approach_observation = None;
                self.fifty_foot = None;
                self.touchdown_captured = false;
            }
        }

        let mut next_interval = if !on_ground && height_agl > 500.0 {
            1.0
        } else {
            0.025
        };
        if self.air_time > 15.0 && height_agl < 20.0 {
            self.push_sample(
                seconds(datarefs.flight_time.get_f32() as f64),
                meters_per_second(datarefs.local_vy.get_f32() as f64)
                    * (datarefs.pitch.get_f32() as f64).to_radians().cos(),
            );

            if self.update_remaining > 0.0 {
                self.update_remaining -= elapsed;
                if self.loops_after_touchdown >= 1 {
                    if let (Some(sample), Some(result)) =
                        (self.samples.iter().rev().nth(2), self.result.as_mut())
                    {
                        if sample.vertical_speed
                            < meters_per_second(result.vertical_speed_mps as f64)
                        {
                            result.vertical_speed_mps =
                                sample.vertical_speed.get::<meter_per_second>() as f32;
                            update.result_changed = true;
                        }
                        if sample.filtered_g > result.g as f64 {
                            result.g = sample.filtered_g as f32;
                            update.result_changed = true;
                        }
                    }
                    if datarefs.nose_wheel_down() {
                        if let (Some(database), Some(runway_match), Some(result)) =
                            (runways, self.active_runway, self.result.as_mut())
                        {
                            if result.nose_wheel_distance_m.is_none() {
                                result.nose_wheel_distance_m = Some(
                                    database
                                        .metrics(runway_match, position, degrees(heading))
                                        .distance_from_threshold
                                        .get::<meter>(),
                                );
                                update.result_changed = true;
                            }
                        }
                    }
                }
                if self.loops_after_touchdown == 20 {
                    update.show_overlay = true;
                }
                self.loops_after_touchdown += 1;
                next_interval = -1.0;
                if self.update_remaining <= 0.0 {
                    self.update_remaining = 0.0;
                    update.finished = true;
                }
            }

            if !self.touchdown_captured && !self.last_on_ground && on_ground {
                self.touchdown_captured = true;
                let metrics = match (runways, self.active_runway) {
                    (Some(database), Some(runway_match)) => {
                        Some(database.metrics(runway_match, position, degrees(heading)))
                    }
                    _ => None,
                };
                let sample = self
                    .samples
                    .iter()
                    .rev()
                    .nth(2)
                    .copied()
                    .unwrap_or_default();
                self.result = Some(LandingResult {
                    vertical_speed_mps: sample.vertical_speed.get::<meter_per_second>() as f32,
                    touchdown_pitch_deg: datarefs.pitch.get_f32(),
                    crab_angle_deg: crab_angle(ground_track, heading),
                    fifty_foot: self.fifty_foot,
                    g: sample.filtered_g as f32,
                    ias: datarefs.ias.get_f32(),
                    vls: datarefs.toliss_vls.map(|dataref| dataref.get_f32()),
                    metrics,
                    crossing_height_m: self.crossing_height_m,
                    nose_wheel_distance_m: None,
                });
                self.update_remaining = 10.0;
                self.loops_after_touchdown = 0;
                next_interval = -1.0;
                update.result_changed = true;
            }
        }

        self.last_on_ground = on_ground;
        (next_interval, update)
    }

    fn push_sample(&mut self, time: Time, vertical_speed: Velocity) {
        self.samples.push_back(Sample {
            time,
            vertical_speed,
            ..Sample::default()
        });
        while self.samples.len() > 4 {
            self.samples.pop_front();
        }
        if self.samples.len() < 3 {
            return;
        }
        let len = self.samples.len();
        let p0 = self.samples[len - 3];
        let p1 = self.samples[len - 2];
        let p2 = self.samples[len - 1];
        let h10 = p1.time - p0.time;
        let h20 = p2.time - p0.time;
        let h21 = p2.time - p1.time;
        if h10 > seconds(0.0) && h20 > seconds(0.0) && h21 > seconds(0.0) {
            let g = 1.0
                + (-p0.vertical_speed * h21 / (h10 * h20) + p1.vertical_speed / h10
                    - p1.vertical_speed / h21
                    + p2.vertical_speed * h10 / (h21 * h20))
                    .get::<standard_gravity>();
            self.samples[len - 2].g = g;
        }
        if self.samples.len() == 4 {
            let duration = self.samples[3].time - self.samples[0].time;
            if duration > seconds(0.0) {
                let filtered = (0..3)
                    .map(|index| {
                        self.samples[index].g
                            * (self.samples[index + 1].time - self.samples[index].time)
                    })
                    .sum::<Time>();
                let filtered = (filtered / duration).get::<ratio>();
                self.samples[1].filtered_g = filtered;
            }
        }
    }
}

fn fifty_foot_crossing(
    previous: ApproachObservation,
    current: ApproachObservation,
) -> Option<FiftyFootMetrics> {
    if previous.height_agl_m < FIFTY_FEET_M
        || current.height_agl_m > FIFTY_FEET_M
        || current.height_agl_m >= previous.height_agl_m
    {
        return None;
    }
    let fraction = ((previous.height_agl_m - FIFTY_FEET_M)
        / (previous.height_agl_m - current.height_agl_m))
        .clamp(0.0, 1.0);
    Some(FiftyFootMetrics {
        ias: previous.ias + (current.ias - previous.ias) * fraction,
        pitch_deg: previous.pitch_deg + (current.pitch_deg - previous.pitch_deg) * fraction,
    })
}

fn crab_angle(ground_track_deg: f64, heading_deg: f64) -> f32 {
    let angle = angular_delta(ground_track_deg, heading_deg);
    if angle.is_finite() {
        angle as f32
    } else {
        0.0
    }
}

fn geo_distance(a: GeoPoint, b: GeoPoint) -> xplane_units::Length {
    let (east, north) = xplane_airports::project(a, b);
    let vertical = b.elevation - a.elevation;
    (east * east + north * north + vertical * vertical).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceleration_and_filtered_g_match_known_motion_at_irregular_intervals() {
        for (acceleration, expected_g) in [(-9.80665, 0.0), (0.0, 1.0), (4.903325, 1.5)] {
            let mut tracker = LandingTracker::default();
            for time in [0.0, 0.2, 0.5, 0.7, 1.1, 1.4] {
                tracker.push_sample(seconds(time), meters_per_second(-2.0 + acceleration * time));
            }
            assert!((tracker.samples[2].g - expected_g).abs() < 1e-12);
            assert!((tracker.samples[1].filtered_g - expected_g).abs() < 1e-12);
        }
    }

    #[test]
    fn landing_lines_include_rating_and_runway_data() {
        let result = LandingResult {
            vertical_speed_mps: -0.7,
            touchdown_pitch_deg: 4.0,
            crab_angle_deg: -2.5,
            fifty_foot: Some(FiftyFootMetrics {
                ias: 75.0,
                pitch_deg: 3.5,
            }),
            g: 1.1,
            ias: 72.0,
            vls: None,
            metrics: Some(TouchdownMetrics {
                airport: "KPHL".to_owned(),
                runway: "27R".to_owned(),
                threshold_elevation: meters(10.0),
                distance_from_threshold: meters(350.0),
                centerline_deviation: meters(-1.5),
                centerline_angle: degrees(0.8),
            }),
            crossing_height_m: Some(15.0),
            nose_wheel_distance_m: None,
        };
        let lines = result.lines(&RatingScale::default(), 1.0, "kts", false);
        assert_eq!(
            lines,
            vec![
                "good landing",
                "50'  |  75 KIAS  |  3.5° pitch",
                "Threshold KPHL/27R  |  49 ft",
                "Touchdown  |  -138 fpm  |  1.10 G",
                "TD attitude  |  4.0° pitch  |  -2.5° crab",
                "TD speed  |  72 KIAS",
                "TD point  |  1148 ft from threshold",
                "Centerline  |  -5 ft  |  +0.8°",
            ]
        );
    }

    #[test]
    fn interpolates_approach_values_at_fifty_feet() {
        let previous = ApproachObservation {
            height_agl_m: 20.0,
            ias: 80.0,
            pitch_deg: 5.0,
        };
        let current = ApproachObservation {
            height_agl_m: 10.0,
            ias: 70.0,
            pitch_deg: 3.0,
        };
        let result = fifty_foot_crossing(previous, current).unwrap();
        assert!((result.ias - 75.24).abs() < 0.01);
        assert!((result.pitch_deg - 4.048).abs() < 0.001);
    }

    #[test]
    fn crab_angle_uses_the_shortest_signed_difference() {
        assert!((crab_angle(359.0, 1.0) - 2.0).abs() < f32::EPSILON);
        assert!((crab_angle(1.0, 359.0) + 2.0).abs() < f32::EPSILON);
    }
}
