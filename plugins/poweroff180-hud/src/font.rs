//! SDK-owned face calibrated to the original 44-pixel Arial strike.
use xplane_plugin::panel_ui::Font;

// Measured in the original HUD strike: GDI tmAscent and summed advances of
// the heading. SDK font-size units differ between font engines, so fit once.
const STRIKE: f32 = 44.;
const ASCENT: f32 = 40.;
// Advances of printable ASCII from the original 44px strike, including
// DEL in its last cell. Tapes, anchors and pointer labels use these widths.
const ADVANCES: [u8; 96] = [
    12, 14, 16, 24, 24, 39, 29, 8, 15, 15, 17, 26, 12, 15, 12, 12, 24, 24, 24, 24, 24, 24, 24, 24,
    24, 24, 12, 12, 26, 26, 26, 24, 45, 29, 29, 32, 32, 29, 27, 34, 32, 12, 22, 29, 24, 37, 32, 34,
    29, 34, 32, 29, 26, 32, 29, 44, 29, 28, 27, 12, 12, 12, 19, 24, 15, 24, 25, 22, 24, 24, 13, 25,
    24, 10, 10, 22, 10, 38, 24, 24, 25, 25, 15, 22, 12, 24, 23, 33, 22, 21, 22, 15, 11, 15, 26, 33,
];
pub fn width(text: &[u8], scale: f32) -> f32 {
    text.iter()
        .map(|ch| {
            ADVANCES
                .get(usize::from(ch.wrapping_sub(32)))
                .copied()
                .unwrap_or(24) as u32
        })
        .sum::<u32>() as f32
        * scale
}

pub fn load() -> Option<Font> {
    // XPLMFontAddFace uses the simulator's virtual path namespace. A drive
    // letter in an absolute Windows Fonts path raises an X-Plane error dialog.
    Font::load(c"Resources/fonts/Roboto-Regular.ttf", STRIKE)?.fit_width(
        c"POWER-OFF 180  |  KCDW RWY 22",
        width(b"POWER-OFF 180  |  KCDW RWY 22", 1.),
    )
}

pub fn placement(y: f64, size: f64) -> (f32, f32) {
    let scale = size as f32 / STRIKE;
    (scale, -y as f32 - ASCENT * scale)
}

#[cfg(test)]
mod tests {
    #[test]
    fn original_top_anchors_and_ascenders_scale_together() {
        assert_eq!(super::width(b"POWER-OFF 180  |  KCDW RWY 22", 1.), 727.);
        assert_eq!(super::width(b"CONTROL INPUT", 0.5), 178.5);
        assert_eq!(super::placement(15., 22.), (0.5, -35.));
        assert_eq!(super::placement(-14., 44.), (1., -26.));
    }
}
