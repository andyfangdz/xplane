use crate::*;
use uom::si::{
    angle::degree,
    f64::{Angle, Length},
    length::meter,
};
use xplane_airports::{GeoPoint, LocalProjection};

#[derive(Clone, Copy, Debug)]
pub struct LocalFrame {
    origin: GeoPoint,
    projection: LocalProjection,
}
impl LocalFrame {
    pub fn new(lat: f64, lon: f64, elev: f64) -> Self {
        let origin = GeoPoint {
            lat,
            lon,
            elevation: Length::new::<meter>(elev),
        };
        Self {
            origin,
            projection: LocalProjection::new(
                origin,
                Angle::new::<degree>(lat),
                Length::new::<meter>(6371000. * std::f64::consts::PI / 180.),
            ),
        }
    }
    pub fn world_to_local(&self, lat: f64, lon: f64, alt: f64) -> (f64, f64, f64) {
        let (east, north) = self.projection.project(GeoPoint {
            lat,
            lon,
            elevation: Length::new::<meter>(alt),
        });
        (
            east.get::<meter>(),
            alt - self.origin.elevation.get::<meter>(),
            -north.get::<meter>(),
        )
    }
    pub fn local_to_world(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let p = self
            .projection
            .unproject(Length::new::<meter>(x), Length::new::<meter>(-z));
        (p.lat, p.lon, self.origin.elevation.get::<meter>() + y)
    }
}
pub fn isa_temperature(h: f64) -> f64 {
    15. - 0.0065 * h
}
pub fn isa_pressure(h: f64) -> f64 {
    if h < 11000. {
        101325. * (1. - 2.25577e-5 * h).powf(5.2559)
    } else {
        22632.
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Schedule {
    pub dead: bool,
    pub scheduled: bool,
    pub phase: i32,
    pub last_call: Option<f64>,
    due_time: f64,
    due_frame: Option<u64>,
    revision: u64,
}
impl Schedule {
    pub fn new(phase: i32) -> Self {
        Self {
            phase,
            ..Self::default()
        }
    }
    pub fn set(&mut self, interval: f32, now: f64, frame: u64) {
        self.revision = self.revision.wrapping_add(1);
        self.scheduled = interval.is_finite() && interval != 0. && !self.dead;
        self.due_frame = (interval < 0.).then(|| frame.saturating_add((-interval as u64).max(1)));
        self.due_time = now + interval as f64;
    }
    pub fn due(&self, now: f64, frame: u64, phase: i32) -> bool {
        !self.dead
            && self.scheduled
            && self.phase == phase
            && self
                .due_frame
                .map(|f| frame >= f)
                .unwrap_or(now >= self.due_time - 1e-9)
    }
    pub fn begin(&mut self, now: f64, dt: f64) -> (f32, u64) {
        let elapsed = self.last_call.map(|t| now - t).unwrap_or(dt);
        self.last_call = Some(now);
        (elapsed as f32, self.revision)
    }
    pub fn finish(&mut self, interval: f32, now: f64, frame: u64, revision: u64) {
        if !self.dead && self.revision == revision {
            self.set(interval, now, frame);
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Nav {
    pub kind: i32,
    pub lat: f32,
    pub lon: f32,
    pub height: f32,
    pub heading: f32,
    pub freq: i32,
    pub id: String,
    pub name: String,
    pub region: String,
}
pub fn parse_nav(text: &str) -> Result<Vec<Nav>, String> {
    let mut aids = Vec::new();
    for line in text.lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        if p.len() < 8 {
            continue;
        }
        let code = p[0].parse::<i32>().unwrap_or(0);
        let kind = match code {
            1 => 1,
            2 => 2,
            3 => 4,
            4 => 8,
            5 => 16,
            6 => 32,
            7 => 64,
            8 => 128,
            9 => 256,
            12 => 1024,
            13 => 512,
            14 => 4096,
            _ => continue,
        };
        let number = |i: usize| p[i].parse::<f32>().ok().filter(|n| n.is_finite());
        let (Some(lat), Some(lon), Some(height), Some(freq), Some(heading)) =
            (number(1), number(2), number(3), number(4), number(6))
        else {
            continue;
        };
        let at = if (4..=6).contains(&code) { 9 } else { 8 };
        aids.push(Nav {
            kind,
            lat,
            lon,
            height: height * 0.3048,
            heading: if [4, 8, 16, 32].contains(&kind) {
                heading
            } else {
                0.
            },
            freq: freq as i32,
            id: p[7].into(),
            name: p.get(at..).unwrap_or_default().join(" "),
            region: String::new(),
        });
    }
    if aids.is_empty() {
        Err("navdat parsed no rows".into())
    } else {
        Ok(aids)
    }
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub entries: Vec<i32>,
    pub alts: Vec<i32>,
    pub displayed: i32,
    pub direct: i32,
}
impl Default for Plan {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            alts: Vec::new(),
            displayed: -1,
            direct: -1,
        }
    }
}

impl Store {
    pub fn seed_parked(&mut self, c: &Config) -> Result<(), String> {
        for (name, value) in [
            ("latitude", c.lat),
            ("longitude", c.lon),
            ("elevation", c.elev + c.profile.parked_height_m),
            ("local_x", 0.),
            ("local_y", c.profile.parked_height_m),
            ("local_z", 0.),
            ("y_agl", c.profile.parked_height_m),
            ("theta", 0.),
            ("phi", 0.),
            ("psi", c.heading),
            ("mag_psi", c.heading + c.magvar),
        ] {
            self.seed(
                &format!("sim/flightmodel/position/{name}"),
                FLOAT | DOUBLE,
                value,
            );
        }
        for name in [
            "local_vx",
            "local_vy",
            "local_vz",
            "local_ax",
            "local_ay",
            "local_az",
            "P",
            "Q",
            "R",
            "Prad",
            "Qrad",
            "Rrad",
            "vh_ind",
            "hpath",
            "vpath",
            "true_airspeed",
            "groundspeed",
        ] {
            self.seed(
                &format!("sim/flightmodel/position/{name}"),
                FLOAT | DOUBLE,
                0.,
            );
        }
        for (name, value) in [
            ("sim/flightmodel/failures/onground_any", 1),
            ("sim/flightmodel/failures/onground_all", 1),
            ("sim/flightmodel/ground/surface_texture_type", 3),
            ("sim/time/paused", 0),
            ("sim/time/is_in_replay", 0),
            ("sim/time/sim_speed", 1),
            ("sim/time/ground_speed", 1),
            ("sim/time/local_date_days", 302),
            ("sim/time/zulu_date_days", 302),
            ("sim/graphics/view/view_type", 1026),
            ("sim/graphics/view/view_is_external", 0),
            ("sim/graphics/VR/enabled", 0),
            ("sim/graphics/view/hide_yoke", 0),
            ("sim/operation/sound/sound_on", 1),
        ] {
            self.seed(name, INT, value as f64);
        }
        for (name, value) in [
            (
                "sim/weather/aircraft/temperature_ambient_deg_c",
                isa_temperature(c.elev),
            ),
            (
                "sim/weather/aircraft/barometer_current_pas",
                isa_pressure(c.elev),
            ),
            ("sim/weather/aircraft/visibility_reported_sm", 20.),
            ("sim/weather/region/runway_friction", 0.),
            ("sim/time/sim_speed_actual", 1.),
            ("sim/time/sim_speed_actual_ogl", 1.),
            ("sim/time/total_running_time_sec", 0.),
            ("sim/time/total_flight_time_sec", 0.),
            ("sim/time/local_time_sec", 43200.),
            ("sim/time/zulu_time_sec", 43200.),
            ("sim/operation/misc/frame_rate_period", 1. / 60.),
            ("sim/time/framerate_period", 1. / 60.),
            ("sim/graphics/scenery/sun_pitch_degrees", 45.),
            ("sim/graphics/view/field_of_view_deg", 60.),
        ] {
            self.seed(name, FLOAT | DOUBLE, value);
        }
        for name in [
            "wind_now_x_msc",
            "wind_now_y_msc",
            "wind_now_z_msc",
            "wind_now_direction_degt",
            "wind_now_speed_msc",
            "precipitation_on_aircraft_ratio",
            "rain_on_aircraft_ratio",
            "snow_on_aircraft_ratio",
            "hail_on_aircraft_ratio",
        ] {
            self.seed(&format!("sim/weather/aircraft/{name}"), FLOAT | DOUBLE, 0.);
        }
        for name in ["world_matrix", "projection_matrix"] {
            self.ensure(&format!("sim/graphics/view/{name}"), FLOAT_ARRAY)
                .floats = (0..16).map(|i| if i % 5 == 0 { 1. } else { 0. }).collect();
        }
        if let Some(mass) = c.profile.mass_kg {
            self.seed_profile_value(
                "sim/flightmodel/weight/m_total",
                &ProfileValue::Double(mass),
            )?;
        }
        if let Some(tail) = &c.profile.tail_number {
            self.seed_string("sim/aircraft/view/acf_tailnum", tail);
        }
        if c.profile.engine_count > 0 {
            self.seed_profile_value(
                "sim/aircraft/engine/acf_num_engines",
                &ProfileValue::Int(c.profile.engine_count as i32),
            )?;
        }
        for (name, value) in &c.profile.datarefs {
            self.seed_profile_value(name, value)?;
        }
        let path = c.acf_path.replace('\\', "/");
        let parts: Vec<_> = path.rsplit('/').take(2).collect();
        self.seed_string(
            "sim/aircraft/view/acf_relative_path",
            &parts.into_iter().rev().collect::<Vec<_>>().join("/"),
        );
        Ok(())
    }
    pub fn refresh(&mut self, frame: &LocalFrame, previous: Option<[f64; 3]>) -> [f64; 3] {
        let x = self.scalar("sim/flightmodel/position/local_x");
        let y = self.scalar("sim/flightmodel/position/local_y");
        let z = self.scalar("sim/flightmodel/position/local_z");
        let local = [x, y, z];
        if previous == Some(local) {
            return local;
        }
        let (lat, lon, alt) = frame.local_to_world(x, y, z);
        for (name, v) in [
            ("latitude", lat),
            ("longitude", lon),
            ("elevation", alt),
            ("y_agl", y),
        ] {
            self.set(&format!("sim/flightmodel/position/{name}"), v);
        }
        let vx = self.scalar("sim/flightmodel/position/local_vx");
        let vz = self.scalar("sim/flightmodel/position/local_vz");
        self.set("sim/flightmodel/position/groundspeed", vx.hypot(vz));
        local
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_and_nav_units() {
        let frame = LocalFrame::new(40.8, -74.2, 100.);
        let (x, y, z) = frame.world_to_local(40.9, -74.1, 123.);
        let (lat, lon, alt) = frame.local_to_world(x, y, z);
        assert!((lat - 40.9).abs() < 1e-10);
        assert!((lon + 74.1).abs() < 1e-10);
        assert_eq!(alt, 123.);
        let aids = parse_nav("3 40 -74 100 11680 100 -10 TEST Test VOR\n99").unwrap();
        assert!((aids[0].height - 30.48).abs() < 1e-4);
        assert_eq!(aids[0].freq, 11680);
    }
    #[test]
    fn scheduling_and_reentrant_reschedule() {
        let mut s = Schedule::default();
        s.set(-2., 0., 0);
        assert!(!s.due(0.1, 1, 0));
        assert!(s.due(0.2, 2, 0));
        let (_, revision) = s.begin(0.2, 0.1);
        s.set(2., 0.2, 2);
        s.finish(-1., 0.2, 2, revision);
        assert!(!s.due(0.3, 3, 0));
        assert!(s.due(2.2, 22, 0));
    }
}
