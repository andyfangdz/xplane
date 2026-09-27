//! Longitudinal screening sim for the gust final approach and flare.
//!
//! usage: flaresim <card dir> [start_h_ft] [param=value ...]
//! Replays the recorded flight into the real controller until `start_h`, then
//! flies a point-mass SR20 (lift fitted on 18 gust flights, R^2 0.99) with the
//! attitude-helper response fitted on 24 flares, in the recorded wind series.
use flate2::read::GzDecoder;
use poweroff180::{Config, Controller, Phase, Sample};
use std::{collections::HashMap, fs, io::Read, path::Path};

const G: f64 = 32.174;
const KT: f64 = 1.687_81;

struct Trace {
    cols: HashMap<String, usize>,
    rows: Vec<Vec<f64>>,
}
impl Trace {
    fn load(path: &Path) -> Self {
        let mut text = String::new();
        let gz = path.join("trace.csv.gz");
        if gz.exists() {
            GzDecoder::new(fs::File::open(gz).unwrap()).read_to_string(&mut text).unwrap();
        } else {
            text = fs::read_to_string(path.join("trace.csv")).unwrap();
        }
        let mut lines = text.lines();
        let cols = lines.next().unwrap().split(',').enumerate().map(|(i, k)| (k.to_string(), i)).collect();
        let rows = lines.map(|l| l.split(',').map(|v| v.parse().unwrap_or(f64::NAN)).collect()).collect();
        Self { cols, rows }
    }
    fn get(&self, row: usize, key: &str) -> f64 {
        self.rows[row][self.cols[key]]
    }
}

pub struct Aero {
    pub cl: [f64; 4],
    pub cd0: f64,
    pub k: f64,
    pub span: f64,
}
pub const AERO: Aero = Aero { cl: [0.7335, 0.0698, 0.1657, -0.0193], cd0: 0.0546, k: 0.0515, span: 8.0 };

fn sample_of(t: &Trace, i: usize, heading: f64, mass: f64) -> Sample {
    Sample {
        t: t.get(i, "sim_time"),
        x: t.get(i, "runway_along_ft"),
        y: t.get(i, "runway_cross_ft"),
        h: t.get(i, "agl_ft"),
        ias: t.get(i, "ias_kias"),
        tas_fps: t.get(i, "true_airspeed_mps") / 0.3048,
        gs_fps: t.get(i, "groundspeed_kt") * KT,
        heading: t.get(i, "heading_true_deg"),
        track: t.get(i, "ground_track_true_deg"),
        bank: t.get(i, "bank_deg"),
        pitch: t.get(i, "pitch_deg"),
        vvi: t.get(i, "vvi_fpm"),
        vy: t.get(i, "vertical_speed_mps") / 0.3048,
        wind_kt: t.get(i, "wind_speed_mps") * 1.943_84,
        wind_dir: t.get(i, "wind_direction_true_deg"),
        mass_lb: mass,
        throttle: 0.0,
        ground: false,
    }
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub td_x: f64,
    pub sink_fpm: f64,
    pub kias: f64,
    pub ro_x: f64,
    pub ro_vvi: f64,
    pub ro_kias: f64,
    pub flare_s: f64,
    pub max_rate: f64,
    pub max_pitch: f64,
    /// Flare energy check: ground energy at roundout and touchdown, air feet.
    pub e_ro: f64,
    pub e_td: f64,
    pub air_ft: f64,
}

/// Recorded wind as head/cross components (knots) at simulation time.
fn wind_at(t: &Trace, time: f64, heading: f64) -> (f64, f64, f64, f64) {
    let i = t.rows.partition_point(|r| r[t.cols["sim_time"]] < time).min(t.rows.len() - 1);
    let kt = t.get(i, "wind_speed_mps") * 1.943_84;
    let dir = t.get(i, "wind_direction_true_deg");
    let a = (dir - heading).to_radians();
    (kt * a.cos(), kt * a.sin(), kt, dir)
}

pub fn run(card: &Path, start_h: f64, overrides: &[(String, f64)], verbose: bool) -> Option<Outcome> {
    // Defaults (current source), then the recorded card, then overrides.
    // Only card identity, wind and runway come from the record; guidance
    // parameters come from the current source unless overridden.
    let ini = fs::read_to_string(card.join("native-effective.ini")).ok()?;
    let mut values: Vec<(String, String)> = Config::default()
        .text()
        .lines()
        .map(|l| {
            let (k, v) = l.split_once('=').unwrap();
            (k.to_string(), v.to_string())
        })
        .collect();
    const FROM_RECORD: &[&str] = &[
        "run_token", "threshold_lat", "threshold_lon", "end_lat", "end_lon", "wind_speed_kt",
        "wind_offset_deg", "wind_gust_kt", "mass_target_lb",
    ];
    for line in ini.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if FROM_RECORD.contains(&k) {
                values.iter_mut().filter(|(n, _)| n == k).for_each(|e| e.1 = v.to_string());
            }
        }
    }
    for (k, v) in overrides {
        let entry = values.iter_mut().find(|(n, _)| n == k).unwrap_or_else(|| panic!("unknown {k}"));
        entry.1 = v.to_string();
    }
    let text: String = values.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    let cfg = Config::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    let trace = Trace::load(card);
    let mut c = Controller::new(cfg);
    let heading = c.heading;
    let mass = cfg.mass_target_lb;
    let n = trace.rows.len();
    let phase = |i| trace.get(i, "phase_id") as i64;
    let i0 = (0..n).find(|&i| phase(i) == 7 && trace.get(i, "agl_ft") <= start_h)?;
    // Warm the controller on the recorded flight from the cut.
    let cut = (0..n).find(|&i| trace.get(i, "cut_sim_time") >= 0.0)?;
    c.start(trace.get(cut, "sim_time") - 60.0);
    c.phase = Phase::Delay;
    c.cut_t = trace.get(cut, "cut_sim_time");
    for i in cut..i0 {
        c.step(sample_of(&trace, i, heading, mass));
        if !c.running() {
            eprintln!("warmup aborted {:?} at row {i}", c.reason);
            return None;
        }
    }
    if c.phase != Phase::Final {
        c.phase = Phase::Final;
    }
    if flag("STATE") {
        let g = c.gust.unwrap();
        let (wh, wc, ..) = wind_at(&trace, trace.get(i0, "sim_time"), heading);
        let a = (g.east_kt.atan2(g.north_kt).to_degrees() - heading).to_radians();
        let mean = g.north_kt.hypot(g.east_kt);
        println!("STATE {} integ {:.2} pitch {:.2} cmd {:.2} est_head {:.1} est_cross {:.1} now_head {:.1} now_cross {:.1} ias {:.1} vy {:.1} x {:.0}",
            card.file_name().unwrap().to_string_lossy(), c.speed_integral, trace.get(i0, "pitch_deg"), c.pitch,
            mean * a.cos(), mean * a.sin(), wh, wc, trace.get(i0, "ias_kias"), trace.get(i0, "vertical_speed_mps") / 0.3048, trace.get(i0, "runway_along_ft"));
    }
    // Recorded time at which the flight descended through each height.
    let descent: Vec<(f64, f64)> = (i0..n)
        .take_while(|&i| trace.get(i, "contact_latched") != 1.0)
        .map(|i| (trace.get(i, "agl_ft"), trace.get(i, "sim_time")))
        .collect();
    let height_time = |h: f64| descent.iter().find(|(a, _)| *a <= h).or(descent.last()).map_or(0.0, |d| d.1);
    let mut t = trace.get(i0, "sim_time");
    let mut x = trace.get(i0, "runway_along_ft");
    // Energy offset at the start, emulating a base-timing change; tailwind
    // cards (wind from behind the beam) may take their own.
    let tailwind = cfg.wind_gust_kt > 0.0 && cfg.wind_offset_deg.to_radians().cos() < -0.5;
    let mut h = trace.get(i0, "agl_ft")
        + if tailwind { scale("START_DH_TAIL", scale("START_DH", 0.0)) } else { scale("START_DH", 0.0) };
    let mut gs = trace.get(i0, "groundspeed_kt") * KT;
    let mut vy = trace.get(i0, "vertical_speed_mps") / 0.3048;
    let mut pitch = trace.get(i0, "pitch_deg");
    let mut rate = 0.0;
    let (wh0, wc0, ..) = wind_at(&trace, t, heading);
    // Mean recorded wind over the remaining approach, for gust-free runs.
    let mean_wind = {
        let end = (i0..n).find(|&i| trace.get(i, "contact_latched") == 1.0).unwrap_or(n - 1);
        let k = (end - i0).max(1) as f64;
        let (mut sh, mut sc) = (0.0, 0.0);
        for i in i0..end {
            let (a, b, ..) = wind_at(&trace, trace.get(i, "sim_time"), heading);
            sh += a;
            sc += b;
        }
        let (h, c) = (sh / k, sc / k);
        (h, c, h.hypot(c), (heading + c.atan2(h).to_degrees()).rem_euclid(360.0))
    };
    let mut ias = ((gs + wh0 * KT).hypot(wc0 * KT) / KT) * scale("IAS_RATIO", 1.0);
    let dt = 0.02;
    let mut out = Outcome::default();
    let mut last_pitch = pitch;
    let mut last_t = t;
    let mut ro_seen = false;
    while t < trace.get(i0, "sim_time") + 60.0 {
        // X-Plane decays gusts by height near the surface: below 60 ft use
        // the wind the recorded flight met at the same height (blended in
        // over 60-50 ft), above it the recorded time series.
        let (wh, wc, wkt, wdir) = if flag("NOGUST") {
            mean_wind
        } else if h < 60.0 && !flag("TIME_WIND") {
            let by_height = wind_at(&trace, height_time(h), heading);
            let by_time = wind_at(&trace, t, heading);
            let f = ((h - 50.0) / 10.0).clamp(0.0, 1.0);
            let head = f * by_time.0 + (1.0 - f) * by_height.0;
            let cross = f * by_time.1 + (1.0 - f) * by_height.1;
            (head, cross, head.hypot(cross), (heading + cross.atan2(head).to_degrees()).rem_euclid(360.0))
        } else {
            wind_at(&trace, t, heading)
        };
        let air_x = gs + wh * KT;
        let v = (air_x * air_x + vy * vy + (wc * KT).powi(2)).sqrt();
        let gamma = vy.atan2(air_x).to_degrees();
        let alpha = pitch - gamma;
        let ge = (-h / AERO.span).exp();
        let cl = (AERO.cl[0] + scale("CL_SLOPE", 1.0) * AERO.cl[1] * alpha + AERO.cl[2] * ge + AERO.cl[3] * alpha * ge).min(1.45);
        let q = (v * 0.3048 / 38.0).powi(2);
        let lift = cl * q;
        let drag = (AERO.cd0 + AERO.k * cl * cl) * q * scale("CD_SCALE", 1.08);
        let (sg, cg) = gamma.to_radians().sin_cos();
        let ax = G * (-drag * cg - lift * sg);
        let az = G * (lift * cg - drag * sg) - G;
        let tas_kt = v / KT;
        ias += (tas_kt * scale("IAS_RATIO", 1.0) - ias) * dt / 1.0;
        // Crabbed on the centerline: heading corrects for the crosswind.
        let crab = (wc / tas_kt.max(30.0)).clamp(-0.5, 0.5).asin().to_degrees();
        let s = Sample {
            t,
            x,
            y: 0.0,
            h,
            ias,
            tas_fps: v,
            gs_fps: gs,
            heading: heading + crab,
            track: heading,
            bank: 0.0,
            pitch,
            vvi: vy * 60.0,
            vy,
            wind_kt: wkt,
            wind_dir: wdir,
            mass_lb: mass,
            throttle: 0.0,
            ground: h <= 0.0,
        };
        c.step(s);
        if !ro_seen && c.roundout_t >= 0.0 {
            ro_seen = true;
            out.ro_x = x;
            out.ro_vvi = vy * 60.0;
            out.ro_kias = ias;
            out.e_ro = h + gs * gs / (2.0 * G);
        }
        if ro_seen {
            out.air_ft += v * dt;
        }
        if ro_seen && t > last_t {
            out.max_rate = out.max_rate.max((pitch - last_pitch) / (t - last_t));
            out.max_pitch = out.max_pitch.max(pitch);
        }
        last_pitch = pitch;
        last_t = t;
        if verbose && ((t * 50.0).round() as i64) % 12 == 0 && h < 130.0 {
            println!(
                "{t:7.2} h {h:5.1} x {x:5.0} ias {ias:5.1} vvi {:5.0} pitch {pitch:5.2} cmd {:5.2} des {:5.1} wind {wh:5.1} predTD {:5.0}",
                vy * 60.0, c.pitch, c.desired, c.predicted_touchdown_ft
            );
        }
        if h <= 0.0 {
            out.td_x = x;
            out.e_td = gs * gs / (2.0 * G);
            if flag("ENERGY") {
                eprintln!("ENERGY {:.4}", (out.e_ro - out.e_td) / out.air_ft);
            }
            out.sink_fpm = -vy * 60.0;
            out.kias = ias;
            out.flare_s = if c.roundout_t >= 0.0 { t - c.roundout_t } else { 0.0 };
            return Some(out);
        }
        if !c.running() {
            eprintln!("aborted {:?}", c.reason);
            return None;
        }
        // Attitude helper: rate = (lead - L0(h)) / 0.4 s with a 0.15 s lag.
        let holding = 1.65 * ((33.0 - h) / 33.0).clamp(0.0, 1.0);
        rate += ((c.pitch - pitch - holding) / 0.4 - rate) * dt / 0.15;
        pitch += rate * dt;
        gs += ax * dt;
        vy += az * dt;
        x += gs * dt;
        h += vy * dt;
        t += dt;
    }
    None
}

fn flag(name: &str) -> bool {
    std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false)
}

fn scale(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    const ENVELOPE: &[&str] = &["head5_gust8", "head10_gust10", "cross_left10_gust8", "cross_right10_gust8", "tail5_gust5",
        "head7_gust6", "qleft10_gust6", "qright8_gust8", "cross_right6_gust6", "qtail6_gust4"];
    let args: Vec<String> = std::env::args().skip(1).collect();
    let start_h = scale("START_H", 150.0);
    let overrides: Vec<(String, f64)> = args
        .iter()
        .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.parse().unwrap())))
        .collect();
    let verbose = flag("VERBOSE");
    let quiet = flag("QUIET");
    let mut cards = Vec::new();
    for a in args.iter().filter(|a| !a.contains('=')) {
        let p = Path::new(a);
        if p.join("cards").exists() {
            let mut v: Vec<_> = fs::read_dir(p.join("cards")).unwrap().map(|e| e.unwrap().path()).collect();
            v.sort();
            cards.extend(v);
        } else {
            cards.push(p.to_path_buf());
        }
    }
    let condition = |name: &str| name.rsplit_once('-').unwrap().0.replace("_early", "").replace("_late", "");
    if flag("ENVELOPE") {
        cards.retain(|c| ENVELOPE.contains(&condition(&c.file_name().unwrap().to_string_lossy()).as_str()));
    }
    let mut diffs = Vec::new();
    let (mut passes, mut actual_passes, mut total) = (0, 0, 0);
    let mut reasons: HashMap<&str, usize> = HashMap::new();
    let mut tds = Vec::new();
    if !quiet {
        println!("card                            actual: td    sink  kias  ro_x | sim: td    sink  kias  ro_x  ro_vvi ro_kias flare maxrate pass");
    }
    for card in cards {
        let name = card.file_name().unwrap().to_string_lossy().to_string();
        let a: serde_like::Assessment = serde_like::load(&card);
        let ini = fs::read_to_string(card.join("native-effective.ini")).unwrap_or_default();
        let get = |k: &str| ini.lines().find_map(|l| l.strip_prefix(&format!("{k}=")).map(|v| v.parse::<f64>().unwrap())).unwrap_or(0.0);
        let (offset, gust) = (get("wind_offset_deg"), get("wind_gust_kt"));
        let weight = (1.0 + offset.to_radians().cos()).clamp(0.0, 1.0);
        let limit = 67.0 + 0.7 * gust * weight;
        total += 1;
        actual_passes += usize::from(a.passed);
        match run(&card, start_h, &overrides, verbose) {
            Some(o) => {
                if a.td.is_finite() { diffs.push((o.td_x - a.td, o.sink_fpm - a.sink, o.td_x, a.td, o.kias - a.kias)); }
                // Calibrated against v10 and v11 flights (SINK_BIAS, KIAS_BIAS).
                let (sink, kias) = (o.sink_fpm - scale("SINK_BIAS", 40.0), o.kias + scale("KIAS_BIAS", 0.0));
                let mut why = Vec::new();
                if !(1000.0..=1200.0).contains(&o.td_x) { why.push(if o.td_x < 1000.0 { "short" } else { "long" }); }
                if sink > 300.0 { why.push("sink"); }
                if !(63.0..=limit).contains(&kias) { why.push("speed"); }
                if o.max_rate > 2.8 || o.max_pitch > 7.5 { why.push("motion"); }
                for w in &why { *reasons.entry(w).or_default() += 1; }
                passes += usize::from(why.is_empty());
                tds.push(o.td_x);
                if !quiet {
                    println!(
                        "{name:32} {:6.0} {:5.0} {:5.1} {:5.0} | {:6.0} {:5.0} {:5.1} {:5.0} {:6.0} {:5.1} {:5.1} {:4.2} {}",
                        a.td, a.sink, a.kias, a.ro_x, o.td_x, sink, kias, o.ro_x, o.ro_vvi, o.ro_kias, o.flare_s, o.max_rate, why.join(",")
                    );
                }
            }
            None => {
                reasons.entry("failed").and_modify(|n| *n += 1).or_insert(1);
                println!("{name:32} failed")
            }
        }
    }
    let n = tds.len() as f64;
    let mean = tds.iter().sum::<f64>() / n;
    let sd = (tds.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / n).sqrt();
    println!("sim passes {passes}/{total} (actual {actual_passes}); touchdown mean {mean:.0} sd {sd:.0}; {reasons:?}");
    if diffs.len() > 2 && !quiet {
        let n = diffs.len() as f64;
        let m = |f: &dyn Fn(&(f64, f64, f64, f64, f64)) -> f64| diffs.iter().map(f).sum::<f64>() / n;
        let (dt, ds, dro) = (m(&|d| d.0), m(&|d| d.1), m(&|d| d.4));
        let sd = (m(&|d| (d.0 - dt).powi(2))).sqrt();
        let (ms, ma) = (m(&|d| d.2), m(&|d| d.3));
        let cov = m(&|d| (d.2 - ms) * (d.3 - ma));
        let r = cov / (m(&|d| (d.2 - ms).powi(2)).sqrt() * m(&|d| (d.3 - ma).powi(2)).sqrt());
        println!("sim-actual: touchdown bias {dt:.0} sd {sd:.0} r {r:.2}; sink bias {ds:.0}; touchdown kias bias {dro:.1}");
    }
}
mod serde_like {
    use std::{fs, path::Path};
    pub struct Assessment {
        pub td: f64,
        pub sink: f64,
        pub kias: f64,
        pub ro_x: f64,
        pub passed: bool,
    }
    fn number(text: &str, key: &str) -> f64 {
        text.find(&format!("\"{key}\":"))
            .map(|i| {
                let rest = &text[i + key.len() + 3..];
                let end = rest.find(|c: char| c == ',' || c == '\n' || c == '}').unwrap();
                rest[..end].trim().parse().unwrap_or(f64::NAN)
            })
            .unwrap_or(f64::NAN)
    }
    pub fn load(card: &Path) -> Assessment {
        let text = fs::read_to_string(card.join("assessment.json")).unwrap_or_default();
        Assessment {
            td: number(&text, "touchdown_ft"),
            sink: number(&text, "physical_sink_fpm"),
            kias: number(&text, "kias"),
            ro_x: number(&text, "roundout_entry_along_ft"),
            passed: text.contains("\"passed\": true"),
        }
    }
}
