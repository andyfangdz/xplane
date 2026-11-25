use crate::{DISPLAY_SIZE, Page, panel};
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
        AvionicsDraw::register_panel(self.sdk_id(), label, move |_, before| {
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
    /// Both rectangles must fit. When a new image is queued, drawing starts after the next texture upload.
    pub fn blit(&mut self, page: &mut Page, source: [u16; 4], destination: [u16; 2]) -> bool {
        page.blit(source, destination)
    }
}

impl Page {
    /// Queue or present a completed page from a native callback registered with `AvionicsDraw::register_panel`.
    pub fn blit(&mut self, source: [u16; 4], destination: [u16; 2]) -> bool {
        if !AvionicsDraw::in_panel_draw()
            || !self.ready()
            || bounds(self.size, source, destination).is_none()
        {
            return false;
        }
        let [l, t, r, b] = source.map(f32::from);
        let [x, y] = destination.map(f32::from);
        let crop = egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b));
        panel::draw(self, crop, [x - l, f32::from(DISPLAY_SIZE[1]) - y + t])
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
        assert_eq!(dest, [42, 545, 193, 615]);
        assert_eq!(bounds([5, 5], [5, 0, 5, 5], [0, 0]), None);
        assert_eq!(bounds([5, 5], [0, 0, 6, 5], [0, 0]), None);
        assert_eq!(bounds([5, 5], [0, 0, 5, 5], [1020, 764]), None);
        assert_eq!(bounds([5, 5], [0, 0, 5, 5], [65535, 0]), None);
    }
}
