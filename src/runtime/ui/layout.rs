use super::*;

pub(super) const UI_MARGIN: i32 = 14;
pub(super) const UI_GAP: i32 = 8;
pub(super) const ACTION_Y: i32 = 44;
pub(super) const PAD_Y: i32 = 112;
pub(super) const POSITION_Y: i32 = 190;
pub(super) const AP_TOGGLE_Y: i32 = 374;
pub(super) const AP_FIELDS_Y: i32 = 416;
pub(super) const SAVE_Y: i32 = 568;
pub(in crate::runtime) const DROPDOWN_ROWS: usize = 8;
pub(super) const DROPDOWN_ROW_HEIGHT: i32 = 28;
pub(super) const DROPDOWN_SCROLLBAR_WIDTH: i32 = 22;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum UiAction {
    Command(CommandAction),
    LoadSelected(bool),
    ToggleAp,
    SaveNamed,
    Refresh,
    ToggleDropdown,
    CloseDropdown,
    ScrollDropdown(isize),
    SelectPad(usize),
    Edit(Field),
}

#[derive(Copy, Clone)]
pub(super) struct Rect {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

impl Rect {
    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Copy, Clone)]
pub(super) struct PadLayout {
    pub(super) previous: Rect,
    pub(super) selector: Rect,
    pub(super) next: Rect,
    pub(super) refresh: Rect,
    pub(super) load: Rect,
    pub(super) load_and_position: Rect,
}

pub(super) fn command_button_rects(width: i32) -> [(Rect, CommandAction); 4] {
    let button_width = (width - UI_MARGIN * 2 - UI_GAP * 3) / 4;
    let actions = [
        CommandAction::CaptureCurrent,
        CommandAction::PositionLoaded,
        CommandAction::QuickSave,
        CommandAction::QuickLoadAndPosition,
    ];
    std::array::from_fn(|index| {
        (
            Rect {
                x: UI_MARGIN + index as i32 * (button_width + UI_GAP),
                y: ACTION_Y,
                width: button_width,
                height: 40,
            },
            actions[index],
        )
    })
}

pub(super) fn pad_layout(width: i32) -> PadLayout {
    let previous_width = 42;
    let next_width = 42;
    let refresh_width = 68;
    let load_width = 72;
    let load_and_position_width = 126;
    let selector_width = width
        - UI_MARGIN * 2
        - UI_GAP * 5
        - previous_width
        - next_width
        - refresh_width
        - load_width
        - load_and_position_width;
    let mut x = UI_MARGIN;
    let mut next_rect = |rect_width| {
        let rect = Rect {
            x,
            y: PAD_Y,
            width: rect_width,
            height: 36,
        };
        x += rect_width + UI_GAP;
        rect
    };
    PadLayout {
        previous: next_rect(previous_width),
        selector: next_rect(selector_width),
        next: next_rect(next_width),
        refresh: next_rect(refresh_width),
        load: next_rect(load_width),
        load_and_position: next_rect(load_and_position_width),
    }
}

pub(super) fn dropdown_list_rect(selector: Rect, row_count: usize) -> Rect {
    Rect {
        x: selector.x,
        y: PAD_Y + selector.height + 2,
        width: selector.width,
        height: row_count.max(1) as i32 * DROPDOWN_ROW_HEIGHT,
    }
}

pub(super) fn dropdown_scrollbar_rects(list: Rect) -> (Rect, Rect, Rect) {
    let column_x = list.x + list.width - DROPDOWN_SCROLLBAR_WIDTH;
    let up = Rect {
        x: column_x,
        y: list.y,
        width: DROPDOWN_SCROLLBAR_WIDTH,
        height: DROPDOWN_ROW_HEIGHT,
    };
    let down = Rect {
        x: column_x,
        y: list.y + list.height - DROPDOWN_ROW_HEIGHT,
        width: DROPDOWN_SCROLLBAR_WIDTH,
        height: DROPDOWN_ROW_HEIGHT,
    };
    let track = Rect {
        x: column_x,
        y: up.y + up.height,
        width: DROPDOWN_SCROLLBAR_WIDTH,
        height: (list.height - up.height - down.height).max(0),
    };
    (up, track, down)
}

pub(super) fn dropdown_thumb_rect(track: Rect, scroll: usize, total_rows: usize) -> Rect {
    if track.height <= 0 {
        return track;
    }
    let max_scroll = total_rows.saturating_sub(DROPDOWN_ROWS);
    let thumb_height = if total_rows <= DROPDOWN_ROWS {
        track.height
    } else {
        (track.height * DROPDOWN_ROWS as i32 / total_rows as i32).clamp(18, track.height)
    };
    let travel = track.height - thumb_height;
    let offset = if max_scroll == 0 {
        0
    } else {
        travel * scroll.min(max_scroll) as i32 / max_scroll as i32
    };
    Rect {
        x: track.x + 5,
        y: track.y + offset,
        width: track.width - 10,
        height: thumb_height,
    }
}

pub(super) fn field_rect(width: i32, top: i32, index: usize) -> Rect {
    let column_width = (width - UI_MARGIN * 2 - UI_GAP) / 2;
    Rect {
        x: UI_MARGIN + (index as i32 % 2) * (column_width + UI_GAP),
        y: top + (index as i32 / 2) * 34,
        width: column_width,
        height: 30,
    }
}

pub(super) fn position_fields() -> [(Field, &'static str); 10] {
    [
        (Field::Latitude, "Latitude"),
        (Field::Longitude, "Longitude"),
        (Field::Altitude, "Altitude / ft"),
        (Field::Heading, "Heading / mag"),
        (Field::Pitch, "Pitch / deg"),
        (Field::Roll, "Roll / deg"),
        (Field::Speed, "Speed / KIAS"),
        (Field::Throttle, "Throttle / 0..1"),
        (Field::Flaps, "Flaps / 0..1"),
        (Field::Gear, "Gear / 0 or 1"),
    ]
}

pub(super) fn autopilot_fields() -> [(Field, &'static str); 7] {
    [
        (Field::ApMode, "AP mode"),
        (Field::ApState, "AP state flags"),
        (Field::ApAltitude, "AP altitude / ft"),
        (Field::ApVerticalVelocity, "AP vertical / fpm"),
        (Field::ApHeading, "AP heading / mag"),
        (Field::ApAirspeed, "AP airspeed / kt"),
        (Field::ApHeadingRollMode, "AP bank limit mode"),
    ]
}

pub(super) fn save_layout(width: i32) -> (Rect, Rect) {
    let button_width = 120;
    (
        Rect {
            x: UI_MARGIN,
            y: SAVE_Y,
            width: width - UI_MARGIN * 2 - UI_GAP - button_width,
            height: 36,
        },
        Rect {
            x: width - UI_MARGIN - button_width,
            y: SAVE_Y,
            width: button_width,
            height: 36,
        },
    )
}

impl PluginState {
    pub(super) fn hit_test(&self, local_x: i32, local_y: i32, width: i32) -> Option<UiAction> {
        let pad = pad_layout(width);
        if self.dropdown_open {
            if pad.selector.contains(local_x, local_y) {
                return Some(UiAction::ToggleDropdown);
            }
            let row_top = PAD_Y + pad.selector.height + 2;
            let visible_rows = self
                .pads
                .len()
                .saturating_sub(self.dropdown_scroll)
                .min(DROPDOWN_ROWS);
            let list = dropdown_list_rect(pad.selector, visible_rows);
            if self.dropdown_max_scroll() > 0 {
                let (up, track, down) = dropdown_scrollbar_rects(list);
                if up.contains(local_x, local_y) {
                    return Some(UiAction::ScrollDropdown(-1));
                }
                if down.contains(local_x, local_y) {
                    return Some(UiAction::ScrollDropdown(1));
                }
                if track.contains(local_x, local_y) {
                    let thumb = dropdown_thumb_rect(track, self.dropdown_scroll, self.pads.len());
                    let page = DROPDOWN_ROWS.saturating_sub(1) as isize;
                    return Some(if local_y < thumb.y {
                        UiAction::ScrollDropdown(-page)
                    } else if local_y >= thumb.y + thumb.height {
                        UiAction::ScrollDropdown(page)
                    } else {
                        UiAction::ScrollDropdown(0)
                    });
                }
            }
            for row in 0..visible_rows {
                let rect = Rect {
                    x: pad.selector.x,
                    y: row_top + row as i32 * DROPDOWN_ROW_HEIGHT,
                    width: pad.selector.width - DROPDOWN_SCROLLBAR_WIDTH,
                    height: DROPDOWN_ROW_HEIGHT,
                };
                if rect.contains(local_x, local_y) {
                    return Some(UiAction::SelectPad(self.dropdown_scroll + row));
                }
            }
            return Some(UiAction::CloseDropdown);
        }

        for (rect, action) in command_button_rects(width) {
            if rect.contains(local_x, local_y) {
                return Some(UiAction::Command(action));
            }
        }

        let file_actions = [
            (pad.previous, UiAction::Command(CommandAction::PreviousPad)),
            (pad.selector, UiAction::ToggleDropdown),
            (pad.next, UiAction::Command(CommandAction::NextPad)),
            (pad.refresh, UiAction::Refresh),
            (pad.load, UiAction::LoadSelected(false)),
            (pad.load_and_position, UiAction::LoadSelected(true)),
        ];
        for (rect, action) in file_actions {
            if rect.contains(local_x, local_y) {
                return Some(action);
            }
        }

        for (index, (field, _)) in position_fields().into_iter().enumerate() {
            let rect = field_rect(width, POSITION_Y, index);
            if rect.contains(local_x, local_y) {
                return Some(UiAction::Edit(field));
            }
        }

        let ap_toggle = Rect {
            x: UI_MARGIN,
            y: AP_TOGGLE_Y,
            width: width - UI_MARGIN * 2,
            height: 32,
        };
        if ap_toggle.contains(local_x, local_y) {
            return Some(UiAction::ToggleAp);
        }

        for (index, (field, _)) in autopilot_fields().into_iter().enumerate() {
            let rect = field_rect(width, AP_FIELDS_Y, index);
            if rect.contains(local_x, local_y) {
                return Some(UiAction::Edit(field));
            }
        }

        let (save_field, save_button) = save_layout(width);
        if save_field.contains(local_x, local_y) {
            return Some(UiAction::Edit(Field::SaveName));
        }
        if save_button.contains(local_x, local_y) {
            return Some(UiAction::SaveNamed);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_width_control_layout_does_not_overlap() {
        let width = 660;
        let pad = pad_layout(width);
        let controls = [
            pad.previous,
            pad.selector,
            pad.next,
            pad.refresh,
            pad.load,
            pad.load_and_position,
        ];
        assert!(controls.iter().all(|rect| rect.width > 0));
        for pair in controls.windows(2) {
            assert!(pair[0].x + pair[0].width <= pair[1].x);
        }
        assert!(pad.load_and_position.x + pad.load_and_position.width <= width - UI_MARGIN);

        let action_buttons = command_button_rects(width);
        for pair in action_buttons.windows(2) {
            assert!(pair[0].0.x + pair[0].0.width <= pair[1].0.x);
        }
    }

    #[test]
    fn dropdown_and_fields_fit_the_minimum_window_height() {
        let dropdown_bottom = PAD_Y + 36 + 2 + DROPDOWN_ROWS as i32 * DROPDOWN_ROW_HEIGHT;
        assert!(dropdown_bottom <= AP_TOGGLE_Y);
        assert!(field_rect(660, AP_FIELDS_Y, autopilot_fields().len() - 1).y + 30 < SAVE_Y);
        let (save_field, _) = save_layout(660);
        assert!(save_field.y + save_field.height < WINDOW_HEIGHT);
    }

    #[test]
    fn dropdown_scrollbar_thumb_tracks_the_visible_page() {
        let selector = pad_layout(660).selector;
        let list = dropdown_list_rect(selector, DROPDOWN_ROWS);
        let (up, track, down) = dropdown_scrollbar_rects(list);
        assert_eq!(up.y + up.height, track.y);
        assert_eq!(track.y + track.height, down.y);
        assert_eq!(down.y + down.height, list.y + list.height);

        let first = dropdown_thumb_rect(track, 0, 46);
        let middle = dropdown_thumb_rect(track, 19, 46);
        let last = dropdown_thumb_rect(track, 38, 46);
        assert_eq!(first.y, track.y);
        assert!(middle.y > first.y);
        assert_eq!(last.y + last.height, track.y + track.height);
    }
}
