use crate::{DISPLAY_SIZE, Page, texture::Texture};
use std::{marker::PhantomData, rc::Rc};
use xplane_plugin::AvionicsDraw;
use xplane_sdk_sys as sdk;

/// A built-in Garmin display that can be enhanced with a page or overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    PilotPfd,
    CopilotPfd,
    Mfd,
}

impl Display {
    /// SDK identifier, also usable with custom before-draw handlers.
    pub const fn sdk_id(self) -> i32 {
        match self {
            Self::PilotPfd => sdk::xplm_device_G1000_PFD_1 as _,
            Self::CopilotPfd => sdk::xplm_device_G1000_PFD_2 as _,
            Self::Mfd => sdk::xplm_device_G1000_MFD as _,
        }
    }

    /// Fully qualified X-Plane GPS command for this unit, e.g. `clr` or `fms_outer_up`.
    pub fn command_name(self, suffix: &str) -> String {
        let unit = match self {
            Self::PilotPfd => 1,
            Self::CopilotPfd => 2,
            Self::Mfd => 3,
        };
        format!("sim/GPS/g1000n{unit}_{suffix}")
    }

    /// The twelve assignable softkeys in left-to-right order with their one-based numbers.
    pub fn softkeys(self) -> impl Iterator<Item = (u8, String)> {
        (1..=12).map(move |n| (n, self.command_name(&format!("softkey{n}"))))
    }

    /// Runs after each stock draw. Hold the returned registration for the lifetime of the overlay.
    pub fn overlay(
        self,
        label: &'static str,
        draw: impl Fn(&mut DisplayTarget) + 'static,
    ) -> Option<AvionicsDraw> {
        AvionicsDraw::register(self.sdk_id(), label, move |_, before| {
            if !before {
                draw(&mut DisplayTarget(PhantomData));
            }
            1
        })
    }
}

/// Borrowed for the duration of an avionics drawing callback.
pub struct DisplayTarget(PhantomData<Rc<()>>);
impl DisplayTarget {
    /// Copy a page rectangle [left, top, right, bottom) to a native display position.
    /// Both rectangles must fit. Returns false if the frame or graphics context is unavailable.
    pub fn blit(&mut self, page: &mut Page, source: [u16; 4], destination: [u16; 2]) -> bool {
        if page.pixels().is_none() {
            return false;
        }
        let Some(dest) = bounds(page.size, source, destination) else {
            return false;
        };
        if page.texture.is_none() {
            let Some(texture) = Texture::new() else {
                return false;
            };
            page.texture = Some(texture);
            page.dirty = true;
        }
        let tex = page.texture.as_mut().unwrap();
        if page.dirty {
            if !tex.upload(page.size, &page.frame) {
                return false;
            }
            page.dirty = false;
        }
        tex.blit(source, dest)
    }
}

fn bounds(size: [u16; 2], [l, t, r, b]: [u16; 4], [x, y]: [u16; 2]) -> Option<[u16; 4]> {
    if l >= r || t >= b || r > size[0] || b > size[1] {
        return None;
    }
    let right = x.checked_add(r - l)?;
    let bottom = y.checked_add(b - t)?;
    (right <= DISPLAY_SIZE[0] && bottom <= DISPLAY_SIZE[1]).then_some([x, y, right, bottom])
}

pub(crate) fn viewport([l, t, r, b]: [u16; 4], [x, y, w, h]: [i32; 4]) -> Option<[i32; 4]> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let scale = |v: u16, origin: i32, extent: i32, native: u16| {
        i32::try_from(i64::from(origin) + i64::from(v) * i64::from(extent) / i64::from(native)).ok()
    };
    let [width, height] = DISPLAY_SIZE;
    Some([
        scale(l, x, w, width)?,
        scale(height - t, y, h, height)?,
        scale(r, x, w, width)?,
        scale(height - b, y, h, height)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_names_distinguish_all_three_units() {
        for (display, unit) in [
            (Display::PilotPfd, 1),
            (Display::CopilotPfd, 2),
            (Display::Mfd, 3),
        ] {
            let keys: Vec<_> = display.softkeys().collect();
            assert_eq!(keys.len(), 12);
            for (index, (number, name)) in keys.into_iter().enumerate() {
                assert_eq!(number as usize, index + 1);
                assert_eq!(name, format!("sim/GPS/g1000n{unit}_softkey{number}"));
            }
        }
    }

    #[test]
    fn crop_on_shifted_half_size_popup() {
        let dest = bounds([151, 768], [0, 670, 151, 740], [42, 545]).unwrap();
        assert_eq!(viewport(dest, [7, 11, 512, 384]), Some([28, 122, 103, 87]));
        assert_eq!(
            viewport([0, 0, 1024, 768], [17, 31, 2048, 1536]),
            Some([17, 1567, 2065, 31])
        );
        assert_eq!(bounds([5, 5], [5, 0, 5, 5], [0, 0]), None);
        assert_eq!(bounds([5, 5], [0, 0, 6, 5], [0, 0]), None);
        assert_eq!(bounds([5, 5], [0, 0, 5, 5], [1020, 764]), None);
        assert_eq!(bounds([5, 5], [0, 0, 5, 5], [65535, 0]), None);
    }
}
