//! Diamond DA40 XLS extended-tank engine page and reversionary strip.
mod canvas;
use crate::widgets::egui;
use crate::widgets::{
    BarGauge, DialTicks, GaugePointer, LinearScale, RadialGauge, TemperatureColumn,
};
pub use canvas::{Atlas, BG, Canvas, RED, WHITE, YELLOW};
use canvas::{BLACK, CYAN, GREEN, GREY};
/// First full-width row of the engine page, including the divider below the radio fields.
pub const ENGINE_BODY_TOP: u16 = 54;
type Bands = &'static [(f32, f32, u32)];
const MAP: Bands = &[(13., 30., GREEN)];
const RPM: Bands = &[
    (500., 2400., GREEN),
    (2400., 2700., YELLOW),
    (2700., 3000., RED),
];
const OIL_T: Bands = &[(149., 230., GREEN), (230., 245., YELLOW), (245., 260., RED)];
const OIL_P: Bands = &[
    (0., 25., RED),
    (25., 55., YELLOW),
    (55., 95., GREEN),
    (95., 97., YELLOW),
    (97., 110., RED),
];
const FLOW: Bands = &[(1., 20., GREEN), (20., 25., RED)];
const FUEL_P: Bands = &[(0., 14., RED), (14., 35., GREEN), (35., 40., RED)];
const CHT: Bands = &[(150., 475., GREEN), (475., 500., YELLOW), (500., 520., RED)];
const AMPS: Bands = &[(2., 75., GREEN)];
const VOLTS: Bands = &[
    (18., 24.1, RED),
    (24.1, 25., YELLOW),
    (25., 30., GREEN),
    (30., 32., YELLOW),
    (32., 34., RED),
];
const TANK: Bands = &[(0., 3., YELLOW), (3., 16., GREEN), (19., 24., GREEN)];
fn fuel_color(g: f32) -> u32 {
    if g <= 0. {
        RED
    } else if g <= 3. {
        YELLOW
    } else {
        WHITE
    }
}
#[derive(Copy, Clone)]
pub struct Readout {
    pub color: u32,
    pub visible: bool,
}
impl Default for Readout {
    fn default() -> Self {
        Self {
            color: WHITE,
            visible: true,
        }
    }
}
impl Readout {
    pub fn oil(p: f32, running: bool, time: f32) -> Self {
        if !running || !p.is_finite() {
            return Self::default();
        }
        let color = if !(25.0..=97.).contains(&p) {
            RED
        } else if p <= 55. || p > 95. {
            YELLOW
        } else {
            WHITE
        };
        Self {
            color,
            visible: color != RED || time % 1. < 0.5,
        }
    }
}
/// DA40 display inputs. Supply manifold pressure in inHg, oil and fuel pressure in psi,
/// temperatures in °F, fuel flow in US gal/h, quantities in US gal and groundspeed in knots.
/// `tanks` contains the left and right sender indications after the extended-tank transition.
#[derive(Default)]
pub struct Sample {
    pub map: f32,
    pub rpm: f32,
    pub oil: f32,
    pub op: f32,
    pub flow: f32,
    pub fp: f32,
    pub amps: f32,
    pub volt: f32,
    pub tanks: [f32; 2],
    pub egt: [f32; 4],
    pub cht: [f32; 4],
    pub remaining: f32,
    pub used: f32,
    pub speed: f32,
    pub hours: f32,
    pub sensors: bool,
    pub pressure: Readout,
}
#[derive(Copy, Clone, Default)]
pub struct Lean {
    pub active: bool,
    peaked: [bool; 4],
    high: [f32; 4],
    peak_flow: [f32; 4],
    first: Option<usize>,
    last: Option<usize>,
}
impl Lean {
    pub fn toggle(&mut self) {
        *self = Self {
            active: !self.active,
            ..Default::default()
        }
    }
    pub fn update(&mut self, s: &Sample) {
        if !self.active || !s.sensors || s.rpm < 500. {
            return;
        }
        for (i, &t) in s.egt.iter().enumerate() {
            if self.peaked[i] {
                continue;
            }
            if t >= self.high[i] {
                self.high[i] = t;
                self.peak_flow[i] = s.flow;
            } else if self.high[i] - t >= 8. && s.flow < self.peak_flow[i] - 0.08 {
                self.peaked[i] = true;
                self.first.get_or_insert(i);
                self.last = Some(i);
            }
        }
    }
}
fn num(v: f32, dp: usize) -> String {
    if v.is_finite() {
        format!("{v:.dp$}")
    } else {
        "---".into()
    }
}
fn invalid(c: &Canvas, l: f32, t: f32, r: f32, b: f32) {
    c.segments([[[l, t], [r, b]], [[l, b], [r, t]]], 0xff6b0e1d, 3.);
}
fn dial(
    c: &mut Canvas,
    p: [f32; 2],
    name: &str,
    value: Option<f32>,
    [lo, hi]: [f32; 2],
    bands: Bands,
    [first, step, divisor, dp]: [i32; 4],
) {
    let [x, y] = p;
    let r = 100.;
    let gauge = RadialGauge {
        radius: r,
        range: [lo, hi],
        start_degrees: 150.,
        sweep_degrees: 230.,
        bands,
        tick_intervals: ((hi - lo) / (step as f32 / 5.)) as u16,
        needle_inset: 2.,
        value,
        layer: c.layer(),
    };
    gauge.paint_at(c, p);
    if c.base {
        for v in (first..=hi as i32).step_by(step as usize) {
            let [u, w] = gauge.point_at(p, r - 23., v as f32);
            c.text(u, w, num(v as f32 / divisor as f32, 0), 15, WHITE, 1);
        }
        c.text(x, y + r * 0.6, name, 16, WHITE, 1);
        if divisor == 100 {
            c.text(x, y - 27., "X100", 15, WHITE, 1);
        }
    } else if let Some(value) = value {
        c.text(x, y + r * 0.83, num(value, dp as usize), 28, WHITE, 1);
    } else {
        invalid(c, x - r, y - r, x + r, y + r)
    }
}
fn dual(c: &mut Canvas, s: &Sample, x: f32, oil: bool) {
    let y = 423.;
    let r = 92.;
    let p = [x, y];
    if c.base {
        c.text(x, y - 68., if oil { "OIL" } else { "FUEL" }, 17, WHITE, 1);
    }
    for right in [false, true] {
        let (lo, hi, v, bands, label) = match (oil, right) {
            (true, false) => (60., 260., s.oil, OIL_T, "ºF"),
            (true, true) => (0., 110., s.op, OIL_P, "PSI"),
            (false, false) => (0., 25., s.flow, FLOW, "GPH"),
            (false, true) => (0., 40., s.fp, FUEL_P, "PSI"),
        };
        let (a, sweep, dx) = if right {
            (40., -98., 40.)
        } else {
            (140., 98., -40.)
        };
        RadialGauge {
            radius: r,
            range: [lo, hi],
            start_degrees: a,
            sweep_degrees: sweep,
            bands,
            tick_intervals: 5,
            needle_inset: 3.,
            value: s.sensors.then_some(v),
            layer: c.layer(),
        }
        .paint_at(c, p);
        if c.base {
            c.text(x + dx, y + 52., label, 15, WHITE, 1);
        } else if s.sensors {
            let style = if oil && right {
                s.pressure
            } else {
                Readout::default()
            };
            if style.visible {
                c.text(
                    x + dx,
                    y + 75.,
                    num(v, usize::from(!oil || right)),
                    24,
                    style.color,
                    1,
                );
            }
        }
    }
    if !c.base && !s.sensors {
        invalid(c, x - r, y - r, x + r, y + r)
    }
}
fn rail(
    c: &mut Canvas,
    [x, y, w]: [f32; 3],
    v: f32,
    [lo, hi]: [f32; 2],
    bands: Bands,
    valid: bool,
    index: usize,
) {
    let gauge = BarGauge {
        scale: LinearScale {
            range: [lo, hi],
            positions: [x, x + w],
        },
        bands,
        frame_color: GREY,
    };
    let ink = bands
        .iter()
        .rev()
        .find(|&&(l, h, band)| (l..=h).contains(&v) && matches!(band, RED | YELLOW))
        .map_or(WHITE, |band| band.2);
    if let Some(p) = gauge.paint_at(c, y, c.layer(), valid.then_some((v, ink)))
        && index != 0
    {
        c.text(p, y - 9., index.to_string(), 10, BLACK, 1);
    }
    if !c.base && !valid {
        invalid(c, x, y - 12., x + w, y + 5.);
    }
}
fn fuel_dial(c: &mut Canvas, x: f32, side: &str, g: f32, valid: bool) {
    let y = 644.;
    let r = 74.;
    let p = [x, y];
    let gauge = RadialGauge {
        radius: r,
        range: [0., 25.],
        start_degrees: 150.,
        sweep_degrees: 230.,
        bands: TANK,
        tick_intervals: 0,
        needle_inset: 2.,
        value: valid.then_some(g),
        layer: c.layer(),
    };
    const MAJOR: &[f32] = &[0., 5., 10., 15., 20., 25.];
    gauge.paint_marked(
        c,
        p,
        &[
            DialTicks {
                values: MAJOR,
                outer: r + 4.,
                inner: r - 7.,
                color: WHITE,
                width: 1.7,
            },
            DialTicks {
                values: &[0.],
                outer: r + 3.,
                inner: r - 4.,
                color: RED,
                width: 3.,
            },
            DialTicks {
                values: &[16., 19., 24.],
                outer: r + 4.,
                inner: r - 4.,
                color: WHITE,
                width: 1.4,
            },
        ],
        fuel_color(g),
    );
    if c.base {
        for &v in MAJOR {
            let [u, w] = gauge.point_at(p, r - 21., v);
            c.text(u, w, v.to_string(), 14, WHITE, 1);
        }
        c.text(x, y + 42., format!("{side} FUEL QTY"), 14, WHITE, 1);
    } else if valid {
        let col = fuel_color(g);
        c.text(x, y + 63., format!("{} GAL", num(g, 0)), 19, col, 1);
    } else {
        invalid(c, x - r, y - r, x + r, y + 70.)
    }
}
fn hottest(arr: &[f32; 4]) -> usize {
    (1..4).fold(0, |best, i| if arr[i] > arr[best] { i } else { best })
}
fn strip(c: &mut Canvas, s: &Sample) {
    let hot = hottest(&s.cht);
    for (y, label, v, lo, hi, bands, dp, index) in [
        (340, "FFLOW GPH", s.flow, 0., 25., FLOW, 1, 0),
        (391, "F PRES PSI", s.fp, 0., 40., FUEL_P, 1, 0),
        (442, "CHT ºF", s.cht[hot], 0., 520., CHT, 0, hot + 1),
        (493, "OIL ºF", s.oil, 60., 260., OIL_T, 0, 0),
        (544, "OIL PSI", s.op, 0., 110., OIL_P, 0, 0),
        (595, "AMPS", s.amps, 0., 80., AMPS, 0, 0),
        (646, "VOLTS", s.volt, 18., 34., VOLTS, 1, 0),
    ] {
        let style = if y == 544 {
            s.pressure
        } else {
            Readout::default()
        };
        if c.base {
            c.baseline(8., y - 19, label, 16, WHITE, 0);
        } else if s.sensors && style.visible {
            c.baseline(143., y - 19, &num(v, dp), 19, style.color, 2);
        }
        rail(
            c,
            [7., y as f32, 137.],
            v,
            [lo, hi],
            bands,
            s.sensors,
            index,
        );
    }
    let tank_scale = LinearScale {
        range: [0., 25.],
        positions: [10., 142.],
    };
    let x = |g| tank_scale.position(g);
    if c.base {
        for &(l, h, col) in TANK {
            c.rect(x(l), 706., x(h) - x(l), 3., col);
        }
        c.segments(
            std::iter::once([[10., 710.], [142., 710.]]).chain((0..=25).step_by(5).map(|g| {
                let p = x(g as f32);
                [[p, 701.], [p, 715.]]
            })),
            GREY,
            1.,
        );
        c.line([10., 705.], [10., 710.], RED, 2.);
        c.segments(
            [16., 19., 24.].map(|g| [[x(g), 704.], [x(g), 712.]]),
            WHITE,
            1.,
        );
        c.text(11., 735., "0", 12, WHITE, 1);
        c.text(139., 735., "25", 12, WHITE, 1);
    } else {
        c.text(
            76.,
            681.,
            "FUEL QTY GAL",
            16,
            if s.sensors {
                fuel_color(s.tanks[0].min(s.tanks[1]))
            } else {
                WHITE
            },
            1,
        );
        if !s.sensors {
            invalid(c, 7., 690., 144., 727.);
            return;
        }
        for (i, &g) in s.tanks.iter().enumerate() {
            let x = x(g);
            let (y, dy, side) = if i == 0 {
                (705., -16., "L")
            } else {
                (710., 16., "R")
            };
            c.tri(
                GaugePointer {
                    x,
                    tip: y,
                    base: y + dy,
                    half_width: 11.,
                }
                .vertices(),
                fuel_color(g),
            );
            c.text(x, y + dy * 0.70, side, 11, BLACK, 1);
        }
    }
}
fn group(c: &mut Canvas, [x, y, w, h]: [f32; 4], title: &str) {
    if !c.base {
        return;
    }
    c.rect(x, y, w, h, GREY);
    c.rect(x + 2., y + 2., w - 4., h - 4., BLACK);
    let tw = title.len() as f32 * 8.;
    c.rect(x + 15., y, tw + 10., 12., GREY);
    c.rect(x + 17., y - 7., tw + 6., 17., BG);
    c.text(x + 20., y + 1., title, 16, WHITE, 0);
}
fn chart(c: &mut Canvas, s: &Sample, lean: &Lean, egt: bool) {
    let (top, bottom, mut lo, mut hi) = if egt {
        (139., 351., 1300., 1700.)
    } else {
        (496., 708., 100., 500.)
    };
    if egt && s.sensors {
        let low = s.egt.into_iter().fold(f32::INFINITY, f32::min);
        let mut high = s.egt[hottest(&s.egt)];
        if lean.active {
            high = high.max(lean.high[hottest(&lean.high)])
        }
        if low < lo || high > hi {
            lo = (100. * ((low - 120.) / 100.).floor()).max(0.);
            hi = (lo + 400.).max(100. * ((high + 30.) / 100.).ceil());
        }
    }
    let scale = LinearScale {
        range: [lo, hi],
        positions: [bottom, top],
    };
    let height = |v| scale.position(v);
    if c.base {
        c.text(
            520.,
            top - 26.,
            if egt { "EGT  ºF" } else { "CHT  ºF" },
            16,
            WHITE,
            0,
        );
        c.line([571., top], [571., bottom], WHITE, 1.);
        if !egt {
            for &(l, h, col) in CHT {
                c.rect(572., height(h), 6., height(l) - height(h), col);
            }
        }
        for i in 0..4 {
            c.text(
                616. + i as f32 * 38.,
                bottom + 12.,
                (i + 1).to_string(),
                14,
                WHITE,
                1,
            );
        }
        return;
    }
    let ticks = [0., 1., 2., 3., 4.].map(|i| {
        let v = lo + i * (hi - lo) / 4.;
        (v, height(v))
    });
    for (v, y) in ticks {
        c.text(567., y, num(v, 0), 16, WHITE, 2);
    }
    c.segments(ticks.map(|(_, y)| [[571., y], [584., y]]), WHITE, 1.);
    let all = lean.peaked.iter().all(|v| *v);
    for i in 0..4 {
        let v = if egt { s.egt[i] } else { s.cht[i] };
        let x = 616. + i as f32 * 38.;
        let label = top - 14. - (i % 2) as f32 * 24.;
        let col = if !egt && v > 500. {
            RED
        } else if !egt && v > 475. {
            YELLOW
        } else if egt && lean.active && lean.first.unwrap_or_else(|| hottest(&s.egt)) == i {
            CYAN
        } else {
            WHITE
        };
        c.text(
            x,
            label,
            if s.sensors { num(v, 0) } else { "X".into() },
            18,
            if s.sensors { col } else { RED },
            1,
        );
        if !s.sensors {
            continue;
        }
        TemperatureColumn {
            x,
            scale,
            leader_top: label + 13.,
            leader_color: GREY,
            width: 26.,
        }
        .paint(
            c,
            v,
            col,
            (egt && lean.peaked[i]).then_some((lean.high[i], CYAN)),
        );
        if egt && lean.peaked[i] && (lean.first == Some(i) || (all && lean.last == Some(i))) {
            c.text(
                x,
                bottom + 32.,
                if lean.first == Some(i) { "1ST" } else { "LAST" },
                14,
                CYAN,
                1,
            );
        }
    }
    if egt
        && lean.active
        && s.sensors
        && let Some(i) = lean.first
    {
        c.text(
            634.,
            408.,
            format!("DELTA PEAK  {} ºF", num(s.egt[i] - lean.high[i], 0)),
            16,
            CYAN,
            1,
        );
    }
}
fn page(c: &mut Canvas, s: &Sample, lean: &Lean) {
    if c.base {
        c.text(511., 43., "EIS - ENGINE", 17, CYAN, 1);
        c.rect(0., f32::from(ENGINE_BODY_TOP), 1024., 2., GREY);
    }
    dial(
        c,
        [121., 179.],
        "MAN IN HG",
        s.sensors.then_some(s.map),
        [7., 32.],
        MAP,
        [10, 5, 1, 1],
    );
    dial(
        c,
        [368., 179.],
        "RPM",
        s.sensors.then_some(s.rpm),
        [0., 3000.],
        RPM,
        [0, 500, 100, 0],
    );
    dual(c, s, 121., true);
    dual(c, s, 368., false);
    fuel_dial(c, 125., "L", s.tanks[0], s.sensors);
    fuel_dial(c, 367., "R", s.tanks[1], s.sensors);
    group(c, [510., 77., 253., 651.], "TEMPERATURE");
    chart(c, s, lean, true);
    chart(c, s, lean, false);
    group(c, [794., 77., 221., 201.], "ELECTRICAL");
    for (i, (y, v, l, h, bands, title)) in [
        (153., s.amps, 0., 80., AMPS, "AMPS"),
        (256., s.volt, 18., 34., VOLTS, "VOLTS"),
    ]
    .into_iter()
    .enumerate()
    {
        if c.base {
            c.text(905., y - 47., title, 16, WHITE, 1);
        } else {
            c.text(
                905.,
                y - 29.,
                if s.sensors { num(v, i) } else { "X".into() },
                21,
                if s.sensors { WHITE } else { RED },
                1,
            );
        }
        rail(c, [815., y, 180.], v, [l, h], bands, s.sensors, 0);
    }
    group(c, [794., 508., 221., 136.], "FUEL CALCULATOR");
    if c.base {
        for (i, label) in ["GALLONS REMAIN", "GALLONS USED", "ENDURANCE", "RANGE NM"]
            .iter()
            .enumerate()
        {
            c.baseline(804., 537 + 30 * i as i32, label, 15, WHITE, 0);
        }
    } else {
        let h = if s.flow > 0.1 {
            s.remaining / s.flow
        } else {
            0.
        };
        let minutes = (h.min(999.) * 60.) as i32;
        let vals = [
            num(s.remaining, 1),
            num(s.used, 1),
            if h > 0. {
                format!("{}:{:02}", minutes / 60, minutes % 60)
            } else {
                "--:--".into()
            },
            if h > 0. {
                num(h * s.speed, 0)
            } else {
                "---".into()
            },
        ];
        for (i, v) in vals.iter().enumerate() {
            c.baseline(
                1007.,
                537 + 30 * i as i32,
                if s.sensors { v } else { "X" },
                19,
                if s.sensors { WHITE } else { RED },
                2,
            );
        }
    }
    group(c, [794., 676., 221., 52.], "TOTAL TIME IN SERVICE");
    if !c.base {
        c.text(908., 704., format!("{} HRS", num(s.hours, 1)), 21, WHITE, 1);
    }
    if c.base || lean.active {
        c.keys(!c.base);
    }
}

/// Paint one layer of the full 1024 × 768 engine page or the compact 151 × 768 strip.
pub struct EngineDisplay<'a> {
    pub atlas: &'a Atlas,
    pub sample: &'a Sample,
    pub lean: &'a Lean,
    pub full: bool,
    pub layer: crate::Layer,
}
impl egui::Widget for EngineDisplay<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let rect = ui.available_rect_before_wrap();
        let mut canvas = Canvas::new(ui, self.atlas, self.layer);
        if self.full {
            page(&mut canvas, self.sample, self.lean);
        } else {
            strip(&mut canvas, self.sample);
        }
        ui.allocate_rect(rect, egui::Sense::hover())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Page, ProFont, widgets::Renderer};
    use std::{path::PathBuf, time::Duration};

    #[test]
    #[ignore = "requires XPLANE_PATH for the installed ProFont"]
    fn render_softkeys_from_font_without_aircraft_artwork() {
        let system = PathBuf::from(std::env::var_os("XPLANE_PATH").unwrap());
        let font = ProFont::load(&system).unwrap();
        let mut renderer = Renderer::default();
        let atlas = Atlas::new(renderer.context(), &font);
        let sample = Sample::default();
        let mut lean = Lean::default();
        let mut surface = Page::new([1024, 768], BG, Duration::ZERO).unwrap();
        for active in [false, true, false] {
            if lean.active != active {
                lean.toggle();
            }
            assert!(renderer.update(&mut surface, |ui, layer| {
                ui.add(EngineDisplay {
                    atlas: &atlas,
                    sample: &sample,
                    lean: &lean,
                    full: true,
                    layer,
                });
            }));
            let pixels = surface.pixels().unwrap().as_chunks::<4>().0;
            let pixel = |x, y| -> [u8; 4] { pixels[1024usize * y + x] };
            let key_background = if active { WHITE } else { BG };
            assert_eq!(pixel(430usize, 745usize), key_background.to_le_bytes());
            assert!(
                (749usize..763)
                    .any(|y| (445usize..494).any(|x| pixel(x, y) != key_background.to_le_bytes()))
            );
            assert_eq!(pixel(5usize, 745usize), WHITE.to_le_bytes());
            assert_eq!(pixel(90usize, 745usize), BG.to_le_bytes());
        }
    }
}
