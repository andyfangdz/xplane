use std::collections::VecDeque;

use super::config::RatingScale;
use super::datarefs::DataRefs;
use super::runway::{GeoPoint, RunwayDatabase, RunwayMatch, TouchdownMetrics};

const METERS_PER_SECOND_TO_FPM: f32 = 196.850;
const METERS_TO_FEET: f64 = 3.2808;
const GRAVITY_MPS2: f64 = 9.80665;

#[derive(Copy, Clone, Debug, Default)]
struct Sample {
    time: f64,
    vertical_speed: f64,
    g: f64,
    filtered_g: f64,
}

#[derive(Clone, Debug)]
pub(super) struct LandingResult {
    pub(super) vertical_speed_mps: f32,
    pub(super) pitch_deg: f32,
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
        let mut lines = vec![
            ratings.text_for(self.vertical_speed_mps).to_owned(),
            format!(
                "Vy: {:.0} fpm / {:.2} m/s / {:.1}°",
                self.vertical_speed_mps * METERS_PER_SECOND_TO_FPM,
                self.vertical_speed_mps,
                self.pitch_deg
            ),
        ];
        if self.ias > 0.0 {
            if let Some(vls) = self.vls.filter(|value| *value > 0.0) {
                lines.push(format!(
                    "IAS / VLS: {:.0} / {:.0} {ias_unit}",
                    self.ias * ias_multiplier,
                    vls
                ));
            } else {
                lines.push(format!("IAS: {:.0} {ias_unit}", self.ias * ias_multiplier));
            }
        }
        lines.push(format!("G:  {:.2}", self.g));
        if let Some(metrics) = &self.metrics {
            lines.push(format!("Threshold {}/{}", metrics.airport, metrics.runway));
            let crossing_height = self.crossing_height_m.unwrap_or(0.0);
            lines.push(format!(
                "Above:         {:.0} ft / {:.0} m",
                crossing_height * METERS_TO_FEET,
                crossing_height
            ));
            lines.push(format!(
                "{}: {:.0} ft / {:.0} m",
                if toliss {
                    "Main wheel TD"
                } else {
                    "Distance     "
                },
                metrics.distance_from_threshold_m * METERS_TO_FEET,
                metrics.distance_from_threshold_m
            ));
            if let Some(distance) = self
                .nose_wheel_distance_m
                .filter(|distance| *distance > 0.0)
            {
                lines.push(format!(
                    "Nose wheel TD: {:.0} ft / {:.0} m",
                    distance * METERS_TO_FEET,
                    distance
                ));
            }
            lines.push(format!(
                "from CL:         {:.0} ft / {:.0} m / {:.1}°",
                metrics.centerline_deviation_m * METERS_TO_FEET,
                metrics.centerline_deviation_m,
                metrics.centerline_angle_deg
            ));
        } else {
            lines.push("Not on a runway!".to_owned());
        }
        lines.truncate(9);
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
            lat: datarefs.latitude.f32() as f64,
            lon: datarefs.longitude.f32() as f64,
            elevation_m: datarefs.elevation.f32() as f64,
        };
        let height_agl = datarefs.height_agl.f32();
        let heading = datarefs.true_heading.f32() as f64;
        let on_ground = datarefs.on_ground();
        let teleported = self
            .last_position
            .is_some_and(|last| geo_distance(last, position) / elapsed as f64 > 3.0 * 340.0);
        self.last_position = Some(position);
        let mut update = LandingUpdate {
            teleported,
            ..LandingUpdate::default()
        };

        if !on_ground {
            if height_agl > 10.0 {
                self.air_time += elapsed;
            }
            if height_agl < 150.0 {
                if self.active_runway.is_none() {
                    if let Some(database) = runways {
                        self.active_runway = database.find_approach(position, heading);
                        if let Some(runway_match) = self.active_runway {
                            let metrics = database.metrics(runway_match, position, heading);
                            self.crossing_height_m =
                                Some(position.elevation_m - metrics.threshold_elevation_m);
                        }
                    }
                }
            } else if height_agl > 200.0 {
                self.active_runway = None;
                self.crossing_height_m = None;
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
                datarefs.flight_time.f32() as f64,
                datarefs.local_vy.f32() as f64 * (datarefs.pitch.f32() as f64).to_radians().cos(),
            );

            if self.update_remaining > 0.0 {
                self.update_remaining -= elapsed;
                if self.loops_after_touchdown >= 1 {
                    if let (Some(sample), Some(result)) =
                        (self.samples.iter().rev().nth(2), self.result.as_mut())
                    {
                        if sample.vertical_speed < result.vertical_speed_mps as f64 {
                            result.vertical_speed_mps = sample.vertical_speed as f32;
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
                                        .metrics(runway_match, position, heading)
                                        .distance_from_threshold_m,
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
                        Some(database.metrics(runway_match, position, heading))
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
                    vertical_speed_mps: sample.vertical_speed as f32,
                    pitch_deg: datarefs.pitch.f32(),
                    g: sample.filtered_g as f32,
                    ias: datarefs.ias.f32(),
                    vls: datarefs.toliss_vls.map(|dataref| dataref.f32()),
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

    fn push_sample(&mut self, time: f64, vertical_speed: f64) {
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
        if h10 > 0.0 && h20 > 0.0 && h21 > 0.0 {
            let g = 1.0
                + (-p0.vertical_speed * h21 / (h10 * h20) + p1.vertical_speed / h10
                    - p1.vertical_speed / h21
                    + p2.vertical_speed * h10 / (h21 * h20))
                    / GRAVITY_MPS2;
            self.samples[len - 2].g = g;
        }
        if self.samples.len() == 4 {
            let duration = self.samples[3].time - self.samples[0].time;
            if duration > 0.0 {
                let filtered = (0..3)
                    .map(|index| {
                        self.samples[index].g
                            * (self.samples[index + 1].time - self.samples[index].time)
                    })
                    .sum::<f64>()
                    / duration;
                self.samples[1].filtered_g = filtered;
            }
        }
    }
}

fn geo_distance(a: GeoPoint, b: GeoPoint) -> f64 {
    let mean_lat = ((a.lat + b.lat) * 0.5).to_radians();
    let east = (b.lon - a.lon).to_radians() * mean_lat.cos() * 6_371_000.0;
    let north = (b.lat - a.lat).to_radians() * 6_371_000.0;
    let vertical = b.elevation_m - a.elevation_m;
    (east * east + north * north + vertical * vertical).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landing_lines_include_rating_and_runway_data() {
        let result = LandingResult {
            vertical_speed_mps: -0.7,
            pitch_deg: 4.0,
            g: 1.1,
            ias: 72.0,
            vls: None,
            metrics: Some(TouchdownMetrics {
                airport: "KPHL".to_owned(),
                runway: "27R".to_owned(),
                threshold_elevation_m: 10.0,
                distance_from_threshold_m: 350.0,
                centerline_deviation_m: -1.5,
                centerline_angle_deg: 0.8,
            }),
            crossing_height_m: Some(15.0),
            nose_wheel_distance_m: None,
        };
        let lines = result.lines(&RatingScale::default(), 1.0, "kts", false);
        assert_eq!(lines[0], "good landing");
        assert!(lines.iter().any(|line| line == "Threshold KPHL/27R"));
    }
}
