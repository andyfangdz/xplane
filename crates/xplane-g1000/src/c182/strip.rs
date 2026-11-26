//! Measured C182T strip layout, alert colours and softkey/menu state.
#![forbid(unsafe_code)]
use super::NumberFormat;
use super::layout::{self, BACKGROUND, CYAN, Kind, PI, Prim, Rgba, WHITE, finite, rgba};
use crate::FontSize;
use crate::widgets::{ColumnStack, DialRay, GaugePointer};
use xplane_numerics::fp as f;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Page {
    #[default]
    Engine,
    Lean,
    System,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Font {
    #[default]
    S,
    L,
    V,
    B,
}
impl Font {
    pub fn size(self) -> FontSize {
        match self {
            Self::S | Self::L => FontSize::Small,
            Self::V => FontSize::Medium,
            Self::B => FontSize::Large,
        }
    }
}

pub const GREEN: Rgba = rgba(0., 128. / 255., 0., 1.);
pub const RED: Rgba = rgba(1., 0., 0., 1.);
pub const YELLOW: Rgba = rgba(1., 1., 0., 1.);
pub const LIMIT: Rgba = rgba(224. / 255., 4. / 255., 4. / 255., 1.);
pub const FRAME: Rgba = rgba(188. / 255., 188. / 255., 188. / 255., 1.);
pub const BLACK: Rgba = rgba(0., 0., 0., 1.);
pub const AMBER: Rgba = rgba(1., 0.75, 0., 1.);
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub x: f32,
    pub y: f32,
    pub s: Vec<u8>,
    pub font: Font,
    pub align: i8,
    pub color: Rgba,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strip {
    pub prims: Vec<Prim>,
    pub texts: Vec<Text>,
}
impl Strip {
    /// Compare only the artwork cached in the background. Engine values can change every frame.
    pub fn same_background(&self, other: &Self) -> bool {
        self.prims
            .iter()
            .filter(|p| p.background)
            .eq(other.prims.iter().filter(|p| p.background))
    }
}

fn fixed_art(s: &mut Strip, paint: impl FnOnce(&mut Strip)) {
    let start = s.prims.len();
    paint(s);
    for prim in &mut s.prims[start..] {
        prim.background = true;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub mp: f32,
    pub rpm: f32,
    /// Trainer-style numeric indication; the needle keeps the raw RPM channel.
    pub rpm_readout: f32,
    pub fflow: f32,
    pub oil_psi: f32,
    pub oil_f: f32,
    pub vac: f32,
    pub hours: f32,
    pub gal_used: f32,
    pub gal_rem: f32,
    pub fuel: [f32; 2],
    pub fuel_low: [bool; 2],
    pub flash_off: bool,
    pub volts: [f32; 2],
    pub amps: [f32; 2],
    pub egt: [f32; 6],
    pub cht: [f32; 6],
    pub selected: i32,
    pub assist: bool,
    pub peak: f32,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            mp: f32::NAN,
            rpm: f32::NAN,
            rpm_readout: f32::NAN,
            fflow: f32::NAN,
            oil_psi: f32::NAN,
            oil_f: f32::NAN,
            vac: f32::NAN,
            hours: f32::NAN,
            gal_used: f32::NAN,
            gal_rem: f32::NAN,
            fuel: [f32::NAN; 2],
            fuel_low: [false; 2],
            flash_off: false,
            volts: [f32::NAN; 2],
            amps: [f32::NAN; 2],
            egt: [f32::NAN; 6],
            cht: [f32::NAN; 6],
            selected: -1,
            assist: false,
            peak: f32::NAN,
        }
    }
}
#[derive(Clone, Copy)]
pub enum Limit {
    Rpm,
    OilPsi,
    OilF,
    Cht,
    Fuel,
}
const LIMITS: [[f32; 4]; 5] = [
    // RPM uses the inclusive raw-speed threshold below.
    [
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::INFINITY,
    ],
    [20., f32::NEG_INFINITY, f32::INFINITY, 115.],
    [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, 245.],
    [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, 500.],
    [1., 8., f32::INFINITY, f32::INFINITY],
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Menu {
    #[default]
    Top,
    Engine,
    Lean,
    Other,
}
pub fn menu_after_key(m: Menu, key: i32) -> Menu {
    let engine = matches!(m, Menu::Engine | Menu::Lean);
    match key {
        1 => {
            if m == Menu::Top {
                Menu::Engine
            } else if engine {
                Menu::Top
            } else {
                m
            }
        }
        2 => {
            if engine {
                Menu::Lean
            } else {
                m
            }
        }
        3 => {
            if m == Menu::Top {
                Menu::Other
            } else if engine {
                Menu::Engine
            } else {
                m
            }
        }
        _ => m,
    }
}
pub fn menu_after_page(m: Menu, from: i32, to: i32) -> Menu {
    if from == to {
        m
    } else if to == 1 {
        Menu::Lean
    } else if m == Menu::Lean {
        Menu::Engine
    } else {
        m
    }
}
pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if f::ordered_gt_f32(lo, v) {
        lo
    } else if f::ordered_gt_f32(v, hi) {
        hi
    } else {
        v
    }
}
pub fn mp_angle(v: f32) -> f32 {
    210. - (v - 10.) * 226. / 25.
}
pub fn rpm_angle(v: f32) -> f32 {
    210. - v * 230. / 2700.
}
/// Cessna NAV III trainer: ten-RPM steps with 40%-of-step hysteresis.
/// A rising 2407 still indicates 2400; above it indicates 2410. Falling from
/// 2410 requires less than 2403 to return to 2400. The alert uses raw speed.
pub fn rpm_readout(raw: f32, previous: f32) -> f32 {
    if !finite(raw) {
        return f32::NAN;
    }
    let previous = if finite(previous) { previous } else { 0. };
    let steps = raw / 10.;
    let low = f::floorf(steps) * 10.;
    let high = f::ceilf(steps) * 10.;
    if f::ordered_gt_f32(previous, raw) {
        if f::ordered_gt_f32(high - 7., raw) {
            low
        } else {
            high
        }
    } else if f::ordered_gt_f32(raw, low + 7.) {
        high
    } else {
        low
    }
}
pub fn fuel_x(gal: f32) -> f32 {
    if !finite(gal) {
        return f32::NAN;
    }
    // The sensor stops at the upper edge of the green arc near 36 usable gallons.
    let gal = gal.clamp(0., 36.);
    if gal <= 30. {
        9. + 3.2 * gal
    } else {
        105. + (gal - 30.) * 2.5
    }
}
pub fn alert(l: Limit, v: f32) -> Rgba {
    if !finite(v) {
        return WHITE;
    }
    if matches!(l, Limit::Rpm) {
        return if v >= 2472. { RED } else { WHITE };
    }
    let k = LIMITS[l as usize];
    if f::ordered_gt_f32(k[0], v) || f::ordered_gt_f32(v, k[3]) {
        RED
    } else if f::ordered_gt_f32(k[1], v) || f::ordered_gt_f32(v, k[2]) {
        YELLOW
    } else {
        WHITE
    }
}
pub fn lean_segments(v: f32, base: f32, max: i32) -> i32 {
    if !finite(v) {
        0
    } else {
        f::truncate_f32_i32(clamp(f::floorf((v - base) / 25.), 0., max as f32))
    }
}
pub fn lean_cylinder(v: &[f32; 6], selected: i32) -> i32 {
    if (0..6).contains(&selected) {
        selected
    } else {
        layout::hottest(v)
    }
}
pub fn next_cylinder(v: &[f32; 6], selected: i32) -> i32 {
    let from = if (0..6).contains(&selected) {
        selected
    } else {
        lean_cylinder(v, -1)
    };
    if from < 0 { 0 } else { (from + 1) % 6 }
}
fn yc(r: f32) -> f32 {
    767.5 - r
}
pub fn box_(s: &mut Strip, x0: f32, r0: f32, x1: f32, r1: f32, color: Rgba) {
    s.prims.push(Prim {
        kind: Kind::Rect,
        x0,
        y0: 767. - r1,
        x1: x1 + 1.,
        y1: 768. - r0,
        width: 1.,
        color,
        ..Prim::default()
    })
}
fn seg(s: &mut Strip, x0: f32, r0: f32, x1: f32, r1: f32, width: f32, color: Rgba) {
    s.prims.push(Prim {
        kind: Kind::Line,
        x0: x0 + 0.5,
        y0: yc(r0),
        x1: x1 + 0.5,
        y1: yc(r1),
        width,
        color,
        ..Prim::default()
    })
}
fn tri(s: &mut Strip, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: Rgba) {
    s.prims.push(Prim {
        kind: Kind::Tri,
        x0: a[0],
        y0: a[1],
        x1: b[0],
        y1: b[1],
        r: c[0],
        start: c[1],
        width: 1.,
        color,
        ..Prim::default()
    })
}
fn arc(s: &mut Strip, [cx, cy]: [f32; 2], r: f32, from: f32, to: f32, width: f32, color: Rgba) {
    s.prims.push(Prim {
        kind: Kind::Arc,
        x0: cx + 0.5,
        y0: yc(cy),
        r,
        start: to,
        sweep: from - to,
        width,
        color,
        ..Prim::default()
    })
}
pub fn put(s: &mut Strip, x: f32, row: f32, t: &[u8], font: Font, align: i8, color: Rgba) {
    let end = t.iter().position(|c| *c == 0).unwrap_or(t.len()).min(15);
    s.texts.push(Text {
        x,
        y: 767. - row,
        s: t[..end].to_vec(),
        font,
        align,
        color,
    })
}
/// A number put at (`x`, `row`) with `format`, or `none` when not finite.
struct Readout<'a> {
    x: f32,
    row: f32,
    format: NumberFormat,
    value: f32,
    font: Font,
    align: i8,
    none: &'a [u8],
    color: Rgba,
}
fn value(s: &mut Strip, r: Readout<'_>) {
    let Readout {
        x,
        row,
        format: fmt,
        value: v,
        font,
        align,
        none,
        color,
    } = r;
    let t = if finite(v) {
        fmt.number(v)
    } else {
        none.to_vec()
    };
    put(s, x, row, &t, font, align, color)
}
/// A triangular pointer whose tip is at (`x`, row `tip`), `half` wide each
/// side and `height` tall, pointing down or up, with an optional label.
struct Pointer {
    x: f32,
    tip: f32,
    down: bool,
    half: f32,
    height: f32,
    label: u8,
    fill: Rgba,
}
fn pointer(s: &mut Strip, p: Pointer) {
    let Pointer {
        x,
        tip,
        down,
        half,
        height,
        label,
        fill,
    } = p;
    let sign = if down { 1. } else { -1. };
    let t = yc(tip) - 0.5 * sign;
    let b = t + sign * (height + 1.);
    for (marker, ink) in [
        (
            GaugePointer {
                x,
                tip: t - sign * 1.6,
                base: b + sign * 1.2,
                half_width: half + 1.5,
            },
            BLACK,
        ),
        (
            GaugePointer {
                x,
                tip: t,
                base: b,
                half_width: half,
            },
            fill,
        ),
    ] {
        let [a, b, c] = marker.vertices();
        tri(s, a, b, c, ink);
    }
    if label != 0 {
        put(
            s,
            x,
            if down { tip - 4. } else { tip + 13. },
            &[label],
            Font::S,
            0,
            BLACK,
        )
    }
}
fn bar_band(s: &mut Strip, base: f32, map: impl Fn(f32) -> f32, lo: f32, hi: f32, c: Rgba) {
    box_(
        s,
        f::roundf(clamp(map(lo), 10., 136.)),
        base - 7.,
        f::roundf(clamp(map(hi), 10., 136.)) - 1.,
        base - 2.,
        c,
    )
}
fn bar_frame(s: &mut Strip, base: f32) {
    box_(s, 10., base - 1., 136., base, WHITE);
    box_(s, 10., base - 13., 10., base, WHITE);
    box_(s, 136., base - 13., 136., base, WHITE)
}
/// A round gauge: an arc of radius `r` about (`cx`, `cy`) from angle `a0` to
/// `a1`, coloured `bands`, end ticks, and a needle at angle `needle`.
struct Dial<'a> {
    cx: f32,
    cy: f32,
    r: f32,
    a0: f32,
    a1: f32,
    bands: &'a [(f32, f32, Rgba)],
    end_color: Rgba,
    needle: f32,
    needle_c: Rgba,
}
fn gauge(s: &mut Strip, d: Dial<'_>) {
    let Dial {
        cx,
        cy,
        r,
        a0,
        a1,
        bands,
        end_color,
        needle,
        needle_c,
    } = d;
    let ray = |angle: f32| {
        let k = angle * PI / 180.;
        DialRay {
            center: [cx, cy],
            direction: [f::cosf(k), -f::sinf(k)],
        }
    };
    fixed_art(s, |s| {
        arc(s, [cx, cy], r, a0, a1, 3.5, WHITE);
        for (a, b, c) in bands {
            arc(s, [cx, cy], r, *a, *b, 3.5, *c)
        }
        for (a, c) in [(a0, WHITE), (a1, end_color)] {
            let [a, b] = ray(a).tick(r + 1., r - 7.);
            seg(s, a[0], a[1], b[0], b[1], 2.5, c);
        }
    });
    if !finite(needle) {
        return;
    }
    let needle = ray(needle).outlined_needle(7., 47., 54.5, r - 0.5, 5.);
    for ([a, b], w) in needle.into_iter().zip([3., 2., 2., 2., 2.]) {
        seg(s, a[0], a[1], b[0], b[1], w, needle_c)
    }
}
fn gauges(s: &mut Strip, v: &Input) {
    gauge(
        s,
        Dial {
            cx: 72.1,
            cy: 124.8,
            r: 62.7,
            a0: mp_angle(10.),
            a1: mp_angle(35.),
            bands: &[(mp_angle(15.), mp_angle(23.), GREEN)],
            end_color: WHITE,
            needle: if finite(v.mp) {
                mp_angle(clamp(v.mp, 10., 35.))
            } else {
                f32::NAN
            },
            needle_c: WHITE,
        },
    );
    put(s, 3., 168., b"10", Font::S, -1, WHITE);
    put(s, 132., 153., b"35", Font::S, -1, WHITE);
    put(s, 72., 146., b"MAN IN", Font::L, 0, WHITE);
    value(
        s,
        Readout {
            x: 72.,
            row: 172.,
            format: NumberFormat::Tenths,
            value: v.mp,
            font: Font::B,
            align: 0,
            none: b"---",
            color: WHITE,
        },
    );
    let red = 2400.;
    let c = alert(Limit::Rpm, v.rpm);
    let rpm_text = if c == RED && v.flash_off {
        BACKGROUND
    } else {
        c
    };
    gauge(
        s,
        Dial {
            cx: 72.75,
            cy: 245.9,
            r: 61.6,
            a0: rpm_angle(0.),
            a1: rpm_angle(2700.),
            bands: &[
                (rpm_angle(2000.), rpm_angle(red), GREEN),
                (rpm_angle(red), rpm_angle(2700.), RED),
            ],
            end_color: RED,
            needle: if finite(v.rpm) {
                rpm_angle(clamp(v.rpm, 0., 2700.))
            } else {
                f32::NAN
            },
            needle_c: c,
        },
    );
    put(s, 1., 291., b"0", Font::S, -1, WHITE);
    put(s, 116., 276., b"2700", Font::S, -1, WHITE);
    put(s, 72., 269., b"RPM", Font::L, 0, rpm_text);
    value(
        s,
        Readout {
            x: 71.,
            row: 295.,
            format: NumberFormat::Whole,
            value: v.rpm_readout,
            font: Font::B,
            align: 0,
            none: b"---",
            color: rpm_text,
        },
    )
}
fn fuel(s: &mut Strip, v: &Input, top: f32, title: bool, numbers: bool) {
    let color = |i: usize| {
        if v.fuel[i].is_finite() && v.fuel[i] <= 0. {
            if v.flash_off { BACKGROUND } else { RED }
        } else if v.fuel_low[i] {
            AMBER
        } else {
            WHITE
        }
    };
    let label = if v.fuel.iter().any(|g| g.is_finite() && *g <= 0.) {
        if v.flash_off { BACKGROUND } else { RED }
    } else if v.fuel_low.contains(&true) {
        AMBER
    } else {
        WHITE
    };
    put(
        s,
        72.,
        top - 17.,
        if title { b"FUEL QTY GAL" } else { b"FUEL QTY" },
        Font::L,
        0,
        label,
    );
    fixed_art(s, |s| {
        for g in [10., 20., 30.] {
            let x = f::roundf(fuel_x(g)) - 1.;
            box_(s, x, top - 4., x, top + 10., FRAME)
        }
        box_(s, 9., top - 4., 9., top + 10., WHITE);
        box_(s, 136., top - 4., 136., top + 11., WHITE);
        box_(s, 121., top + 3., 136., top + 4., WHITE);
        box_(s, 11., top, 12., top + 7., RED);
        box_(s, 14., top, 34., top + 7., YELLOW);
        box_(s, 36., top, 120., top + 7., GREEN);
    });
    if finite(v.fuel[0]) && color(0) != BACKGROUND {
        pointer(
            s,
            Pointer {
                x: fuel_x(v.fuel[0]) + 0.5,
                tip: top + 1.,
                down: true,
                half: 10.,
                height: 14.,
                label: b'L',
                fill: color(0),
            },
        )
    }
    if finite(v.fuel[1]) && color(1) != BACKGROUND {
        pointer(
            s,
            Pointer {
                x: fuel_x(v.fuel[1]) + 0.5,
                tip: top + 7.,
                down: false,
                half: 10.,
                height: 14.,
                label: b'R',
                fill: color(1),
            },
        )
    }
    if numbers {
        for (x, t) in [
            (8.5, b"0".as_slice()),
            (40.5, b"10"),
            (72., b"20"),
            (105., b"30"),
            (131., b"F"),
        ] {
            put(s, x, top + 35., t, Font::S, 0, WHITE)
        }
    }
}
pub fn main_discharge_caution(indicated: f32) -> bool {
    indicated.is_finite() && indicated < -1.
}
fn electrical(s: &mut Strip, v: &Input) {
    let low = |x: f32| finite(x) && f::ordered_ge_f32(24.5, x);
    let main = if low(v.volts[0]) { RED } else { WHITE };
    let essential = if low(v.volts[1]) { RED } else { WHITE };
    put(s, 71.5, 628., b"ELECTRICAL", Font::L, 0, WHITE);
    box_(s, 10., 623., 25., 623., WHITE);
    box_(s, 118., 623., 136., 623., WHITE);
    for (x, t) in [(19., b"M".as_slice()), (72., b"BUS"), (117., b"E")] {
        put(s, x, 651., t, Font::L, if x == 72. { 0 } else { -1 }, WHITE)
    }
    value(
        s,
        Readout {
            x: 22.,
            row: 678.,
            format: NumberFormat::Tenths,
            value: v.volts[0],
            font: Font::V,
            align: 0,
            none: b"---",
            color: main,
        },
    );
    put(
        s,
        73.,
        678.,
        b"VOLTS",
        Font::L,
        0,
        if low(v.volts[0]) || low(v.volts[1]) {
            RED
        } else {
            WHITE
        },
    );
    value(
        s,
        Readout {
            x: 122.5,
            row: 678.,
            format: NumberFormat::Tenths,
            value: v.volts[1],
            font: Font::V,
            align: 0,
            none: b"---",
            color: essential,
        },
    );
    for (x, t) in [(19., b"M".as_slice()), (72., b"BATT"), (117., b"S")] {
        put(s, x, 705., t, Font::L, if x == 72. { 0 } else { -1 }, WHITE)
    }
    for i in 0..2 {
        let a = v.amps[i];
        let zero = f::ordered_gt_f32(0.05, a.abs());
        let discharge_caution = if i == 0 {
            main_discharge_caution(a)
        } else {
            finite(a) && f::ordered_ge_f32(-0.05, a)
        };
        value(
            s,
            Readout {
                x: if i == 0 { 22. } else { 122.5 },
                row: 732.,
                format: if zero {
                    NumberFormat::Zero
                } else {
                    NumberFormat::SignedTenths
                },
                value: if zero { 0. } else { a },
                font: Font::V,
                align: 0,
                none: b"---",
                color: if discharge_caution { AMBER } else { WHITE },
            },
        )
    }
    put(s, 72.5, 732., b"AMPS", Font::L, 0, WHITE)
}
fn hot(v: &[f32; 6]) -> u8 {
    let i = lean_cylinder(v, -1);
    if i < 0 { 0 } else { b'1' + i as u8 }
}
fn hot_value(v: &[f32; 6]) -> f32 {
    let i = lean_cylinder(v, -1);
    if i < 0 { f32::NAN } else { v[i as usize] }
}
/// NAV III engine strip bars share the pointer and label placement. The face
/// closure supplies the measured ticks and band endpoints for each instrument.
struct EngineBar {
    title: &'static [u8],
    title_row: f32,
    base: f32,
    pointer_x: f32,
    pointer_ink: Rgba,
    cylinder: u8,
}
impl EngineBar {
    fn paint(self, s: &mut Strip, scale: impl FnOnce(&mut Strip, f32)) {
        put(s, 72., self.title_row, self.title, Font::L, 0, WHITE);
        fixed_art(s, |s| scale(s, self.base));
        if finite(self.pointer_x) {
            pointer(
                s,
                Pointer {
                    x: clamp(self.pointer_x, 10., 136.) + 0.5,
                    tip: self.base - 3.,
                    down: true,
                    half: 9.,
                    height: 13.,
                    label: self.cylinder,
                    fill: self.pointer_ink,
                },
            );
        }
    }
}
fn engine(s: &mut Strip, v: &Input) {
    let ff = |g| 10. + g * 126. / 22.;
    let op = |p| 10. + p * 1.05;
    let ot = |t| 10. + (t - 75.) * 0.72;
    let ch = |t| 10. + (t - 100.) * 126. / 406.;
    let eg = |t| 10. + (t - 1100.) * 0.315;
    EngineBar {
        title: b"FFLOW GPH",
        title_row: 322.,
        base: 342.,
        pointer_x: ff(v.fflow),
        pointer_ink: WHITE,
        cylinder: 0,
    }
    .paint(s, |s, base| {
        for g in (2..22).step_by(2) {
            let x = f::roundf(ff(g as f32));
            box_(s, x, base - 10., x, base - 2., WHITE);
        }
        bar_frame(s, base);
        box_(s, 12., base - 7., 96., base - 2., GREEN);
    });
    put(s, 8., 356., b"0", Font::S, -1, WHITE);
    put(s, 122., 356., b"22", Font::S, -1, WHITE);
    EngineBar {
        title: b"OIL PRES",
        title_row: 377.,
        base: 397.,
        pointer_x: op(v.oil_psi),
        pointer_ink: alert(Limit::OilPsi, v.oil_psi),
        cylinder: 0,
    }
    .paint(s, |s, base| {
        bar_frame(s, base);
        for (lo, hi, ink) in [(0., 20., RED), (50., 90., GREEN), (115., 121., RED)] {
            bar_band(s, base, op, lo, hi, ink);
        }
    });
    EngineBar {
        title: b"OIL TEMP",
        title_row: 423.,
        base: 443.,
        pointer_x: ot(v.oil_f),
        pointer_ink: alert(Limit::OilF, v.oil_f),
        cylinder: 0,
    }
    .paint(s, |s, base| {
        bar_frame(s, base);
        bar_band(s, base, ot, 100., 245., GREEN);
        bar_band(s, base, ot, 245., 251., RED);
    });
    let t = hot_value(&v.cht);
    EngineBar {
        title: b"CHT",
        title_row: 469.,
        base: 489.,
        pointer_x: ch(t),
        pointer_ink: alert(Limit::Cht, t),
        cylinder: hot(&v.cht),
    }
    .paint(s, |s, base| {
        bar_frame(s, base);
        bar_band(s, base, ch, 200., 500., GREEN);
        bar_band(s, base, ch, 500., 507., RED);
    });
    EngineBar {
        title: b"EGT",
        title_row: 515.,
        base: 535.,
        pointer_x: eg(hot_value(&v.egt)),
        pointer_ink: WHITE,
        cylinder: hot(&v.egt),
    }
    .paint(s, |s, base| {
        for k in 1..8 {
            let x = f::roundf(10. + 15.75 * k as f32);
            box_(s, x, base - 8., x, base - 2., FRAME);
        }
        bar_frame(s, base);
    });
    fuel(s, v, 576., true, true);
    electrical(s, v)
}
fn columns(s: &mut Strip, v: &[f32; 6], base: f32, max: i32, bottom: f32, shown: i32, peak: f32) {
    const LEFT: [f32; 6] = [9., 31., 54., 76., 98., 120.];
    const RIGHT: [f32; 6] = [25., 47., 69., 91., 113., 135.];
    for i in 0..6 {
        let c = if i as i32 == shown { CYAN } else { WHITE };
        let n = lean_segments(v[i], base, max);
        let column = ColumnStack {
            first: [LEFT[i], bottom - 5.],
            size: [RIGHT[i] - LEFT[i] + 1., 6.],
            pitch: -7.,
        };
        for [x, y, w, h] in column.rectangles(n as usize) {
            box_(s, x, y, x + w - 1., y + h - 1., c);
        }
        if i as i32 == shown && finite(peak) {
            let p = lean_segments(peak, base, max);
            if p > 0 {
                let r = bottom - 7. * (p - 1) as f32;
                box_(s, LEFT[i], r - 5., RIGHT[i], r - 2., BACKGROUND);
                box_(s, LEFT[i], r - 5., RIGHT[i], r - 5., CYAN);
                box_(s, LEFT[i], r - 1., RIGHT[i], r, CYAN)
            }
        }
        put(
            s,
            (LEFT[i] + RIGHT[i] + 1.) / 2.,
            bottom + 13.,
            &[b'1' + i as u8],
            Font::S,
            0,
            c,
        )
    }
}
fn five(v: f32) -> f32 {
    if finite(v) {
        f::roundf(v / 5.) * 5.
    } else {
        f32::NAN
    }
}
fn lean(s: &mut Strip, v: &Input) {
    put(s, 8., 327., b"FFLOW GPH", Font::L, -1, WHITE);
    value(
        s,
        Readout {
            x: 137.,
            row: 327.,
            format: NumberFormat::Tenths,
            value: v.fflow,
            font: Font::V,
            align: 1,
            none: b"---",
            color: WHITE,
        },
    );
    let egt = lean_cylinder(&v.egt, v.selected);
    let cht = lean_cylinder(&v.cht, v.selected);
    columns(
        s,
        &v.egt,
        1087.5,
        16,
        456.,
        egt,
        if v.assist { v.peak } else { f32::NAN },
    );
    put(s, 8., 495., b"EGT \xb0F", Font::L, -1, WHITE);
    value(
        s,
        Readout {
            x: 137.,
            row: 495.,
            format: NumberFormat::Whole,
            value: if egt < 0 {
                f32::NAN
            } else {
                five(v.egt[egt as usize])
            },
            font: Font::V,
            align: 1,
            none: b"---",
            color: WHITE,
        },
    );
    if v.assist {
        put(s, 8., 519., b"\x7fPEAK", Font::L, -1, WHITE);
        put(s, 132., 519., b"\xb0F", Font::L, 1, WHITE);
        let d = if egt < 0 || !finite(v.peak) {
            f32::NAN
        } else {
            f::roundf(v.egt[egt as usize] - v.peak)
        };
        value(
            s,
            Readout {
                x: 119.,
                row: 519.,
                format: NumberFormat::SignedWhole,
                value: d,
                font: Font::V,
                align: 1,
                none: b"--",
                color: WHITE,
            },
        )
    }
    columns(s, &v.cht, 195., 12, 623., cht, f32::NAN);
    box_(s, 10., 542., 136., 542., LIMIT);
    let t = if cht < 0 {
        f32::NAN
    } else {
        v.cht[cht as usize]
    };
    put(s, 8., 660., b"CHT \xb0F", Font::L, -1, WHITE);
    value(
        s,
        Readout {
            x: 137.,
            row: 660.,
            format: NumberFormat::Whole,
            value: five(t),
            font: Font::V,
            align: 1,
            none: b"---",
            color: alert(Limit::Cht, t),
        },
    );
    fuel(s, v, 706., false, false)
}
fn system(s: &mut Strip, v: &Input) {
    put(s, 8., 327., b"OIL PSI", Font::L, -1, WHITE);
    value(
        s,
        Readout {
            x: 137.,
            row: 327.,
            format: NumberFormat::Tenths,
            value: v.oil_psi,
            font: Font::V,
            align: 1,
            none: b"---",
            color: alert(Limit::OilPsi, v.oil_psi),
        },
    );
    put(s, 8., 357., b"OIL \xb0F", Font::L, -1, WHITE);
    value(
        s,
        Readout {
            x: 137.,
            row: 357.,
            format: NumberFormat::Whole,
            value: v.oil_f,
            font: Font::V,
            align: 1,
            none: b"---",
            color: alert(Limit::OilF, v.oil_f),
        },
    );
    put(s, 8., 387., b"ENG", Font::L, -1, WHITE);
    value(
        s,
        Readout {
            x: 46.,
            row: 387.,
            format: NumberFormat::Hours,
            value: if finite(v.hours) {
                clamp(v.hours, 0., 9999.9)
            } else {
                f32::NAN
            },
            font: Font::V,
            align: -1,
            none: b"----.-",
            color: WHITE,
        },
    );
    put(s, 137., 387., b"HRS", Font::L, 1, WHITE);
    let vac = |v| 9. + (v - 3.) * 31.75;
    put(s, 72., 410., b"VAC", Font::L, 0, WHITE);
    box_(s, 9., 429., 136., 430., WHITE);
    box_(s, 9., 417., 9., 430., WHITE);
    box_(s, 136., 417., 136., 430., WHITE);
    box_(
        s,
        f::roundf(vac(4.5)),
        423.,
        f::roundf(vac(5.5)),
        428.,
        GREEN,
    );
    if finite(v.vac) {
        pointer(
            s,
            Pointer {
                x: clamp(vac(v.vac), 9., 136.) + 0.5,
                tip: 427.,
                down: true,
                half: 9.,
                height: 13.,
                label: 0,
                fill: WHITE,
            },
        )
    }
    put(s, 71.5, 453., b"FUEL CALC", Font::L, 0, WHITE);
    box_(s, 10., 448., 31., 448., WHITE);
    box_(s, 113., 448., 136., 448., WHITE);
    for (row, label, fmt, val) in [
        (479., b"FFLOW GPH".as_slice(), NumberFormat::Tenths, v.fflow),
        (506., b"GAL USED", NumberFormat::Tenths, v.gal_used),
        (536., b"GAL REM", NumberFormat::Whole, v.gal_rem),
    ] {
        put(s, 8., row, label, Font::L, -1, WHITE);
        value(
            s,
            Readout {
                x: 137.,
                row,
                format: fmt,
                value: val,
                font: Font::V,
                align: 1,
                none: b"---",
                color: WHITE,
            },
        )
    }
    fuel(s, v, 576., true, true);
    electrical(s, v)
}
fn softkey(strip: &mut Strip, [left, top, right, bottom]: [u16; 4], label: &[u8], lit: bool) {
    let x0 = f32::from(left);
    let x1 = f32::from(right);
    strip.prims.push(Prim {
        kind: Kind::Rect,
        x0,
        y0: 768. - f32::from(bottom),
        x1,
        y1: 768. - f32::from(top),
        width: 1.,
        color: if lit { WHITE } else { BACKGROUND },
        ..Prim::default()
    });
    strip.texts.push(Text {
        x: (x0 + x1) / 2.,
        y: 9.,
        s: label.to_vec(),
        font: Font::S,
        align: 0,
        color: if lit {
            layout::rgba(0.5, 0.5, 0.5, 1.)
        } else {
            layout::rgba(0.92, 0.92, 0.92, 1.)
        },
    })
}

/// LEAN softkey overlays in device order. X-Plane draws the other keys.
pub fn lean_softkeys(assist: bool) -> Vec<Strip> {
    let mut select = Strip::default();
    softkey(&mut select, super::LEAN_KEY_BOUNDS[0], b"CYL SLCT", false);
    let mut keys = vec![select];
    if assist {
        let mut key = Strip::default();
        softkey(&mut key, super::LEAN_KEY_BOUNDS[1], b"ASSIST", true);
        keys.push(key);
    }
    keys
}
pub fn build(page: Page, v: &Input, s: &mut Strip) {
    s.prims.clear();
    s.texts.clear();
    fixed_art(s, |s| {
        box_(s, 0., 55., 150., 741., BACKGROUND);
        box_(s, 151., 55., 152., 741., FRAME);
    });
    gauges(s, v);
    match page {
        Page::Engine => engine(s, v),
        Page::Lean => lean(s, v),
        Page::System => system(s, v),
    }
}
