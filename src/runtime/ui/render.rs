use super::layout::*;
use super::*;

const COLOR_CANVAS: [f32; 4] = [0.025, 0.040, 0.055, 0.96];
const COLOR_PANEL: [f32; 4] = [0.055, 0.080, 0.105, 0.98];
const COLOR_FIELD: [f32; 4] = [0.018, 0.030, 0.040, 0.98];
const COLOR_FIELD_HOVER: [f32; 4] = [0.055, 0.115, 0.145, 0.98];
const COLOR_BUTTON: [f32; 4] = [0.075, 0.235, 0.325, 0.98];
const COLOR_BUTTON_HOVER: [f32; 4] = [0.105, 0.385, 0.510, 0.98];
const COLOR_PRIMARY: [f32; 4] = [0.780, 0.390, 0.065, 0.98];
const COLOR_PRIMARY_HOVER: [f32; 4] = [1.000, 0.570, 0.100, 0.98];
const COLOR_SELECTED: [f32; 4] = [0.060, 0.290, 0.390, 0.98];
const COLOR_BORDER: [f32; 4] = [0.210, 0.390, 0.485, 1.0];
const COLOR_FOCUS: [f32; 4] = [0.120, 0.820, 1.000, 1.0];
const COLOR_TEXT: [f32; 3] = [0.920, 0.960, 0.980];
const COLOR_MUTED: [f32; 3] = [0.520, 0.670, 0.740];
const COLOR_AMBER_TEXT: [f32; 3] = [1.000, 0.770, 0.350];

unsafe fn draw_text(x: i32, y: i32, text: &str, color: [f32; 3]) {
    let mut color = color;
    let mut text = c_string(text).into_bytes_with_nul();
    XPLMDrawString(
        color.as_mut_ptr(),
        x,
        y,
        text.as_mut_ptr().cast(),
        ptr::null_mut(),
        0,
    );
}

fn global_rect(left: i32, top: i32, rect: Rect) -> (i32, i32, i32, i32) {
    let box_left = left + rect.x;
    let box_top = top - rect.y;
    (
        box_left,
        box_top,
        box_left + rect.width,
        box_top - rect.height,
    )
}

unsafe fn prepare_flat_drawing() {
    XPLMSetGraphicsState(0, 0, 0, 0, 1, 0, 0);
}

unsafe fn draw_rect(left: i32, top: i32, rect: Rect, color: [f32; 4]) {
    let (box_left, box_top, box_right, box_bottom) = global_rect(left, top, rect);
    prepare_flat_drawing();
    glColor4f(color[0], color[1], color[2], color[3]);
    glBegin(GL_QUADS);
    glVertex2i(box_left, box_top);
    glVertex2i(box_right, box_top);
    glVertex2i(box_right, box_bottom);
    glVertex2i(box_left, box_bottom);
    glEnd();
}

unsafe fn draw_outline(left: i32, top: i32, rect: Rect, color: [f32; 4], width: f32) {
    let (box_left, box_top, box_right, box_bottom) = global_rect(left, top, rect);
    prepare_flat_drawing();
    glLineWidth(width);
    glColor4f(color[0], color[1], color[2], color[3]);
    glBegin(GL_LINE_LOOP);
    glVertex2i(box_left, box_top);
    glVertex2i(box_right, box_top);
    glVertex2i(box_right, box_bottom);
    glVertex2i(box_left, box_bottom);
    glEnd();
    glLineWidth(1.0);
}

unsafe fn draw_rule(x1: i32, y1: i32, x2: i32, y2: i32, color: [f32; 4]) {
    prepare_flat_drawing();
    glColor4f(color[0], color[1], color[2], color[3]);
    glBegin(GL_LINES);
    glVertex2i(x1, y1);
    glVertex2i(x2, y2);
    glEnd();
}

unsafe fn text_width(text: &str) -> f32 {
    let text = c_string(text);
    XPLMMeasureString(0, text.as_ptr(), text.as_bytes().len() as i32)
}

unsafe fn truncate_text(text: &str, max_width: f32) -> String {
    if text_width(text) <= max_width {
        return text.to_owned();
    }
    let mut characters: Vec<char> = text.chars().collect();
    while !characters.is_empty() {
        characters.pop();
        let candidate = format!("{}...", characters.iter().collect::<String>());
        if text_width(&candidate) <= max_width {
            return candidate;
        }
    }
    "...".to_owned()
}

unsafe fn draw_centered_text(left: i32, top: i32, rect: Rect, label: &str, color: [f32; 3]) {
    let label = truncate_text(label, (rect.width - 12).max(12) as f32);
    let width = text_width(&label) as i32;
    draw_text(
        left + rect.x + (rect.width - width) / 2,
        top - rect.y - rect.height / 2 - 5,
        &label,
        color,
    );
}

#[derive(Copy, Clone)]
enum ButtonTone {
    Standard,
    Primary,
    Quiet,
}

struct ButtonSpec<'a> {
    rect: Rect,
    label: &'a str,
    action: UiAction,
    tone: ButtonTone,
    selected: bool,
}

unsafe fn draw_button(state: &PluginState, left: i32, top: i32, spec: ButtonSpec<'_>) {
    let ButtonSpec {
        rect,
        label,
        action,
        tone,
        selected,
    } = spec;
    let hovered = state.hovered_action == Some(action);
    let fill = if selected {
        COLOR_SELECTED
    } else {
        match (tone, hovered) {
            (ButtonTone::Primary, true) => COLOR_PRIMARY_HOVER,
            (ButtonTone::Primary, false) => COLOR_PRIMARY,
            (ButtonTone::Standard, true) => COLOR_BUTTON_HOVER,
            (ButtonTone::Standard, false) => COLOR_BUTTON,
            (ButtonTone::Quiet, true) => COLOR_FIELD_HOVER,
            (ButtonTone::Quiet, false) => COLOR_PANEL,
        }
    };
    draw_rect(left, top, rect, fill);
    draw_outline(
        left,
        top,
        rect,
        if hovered || selected {
            COLOR_FOCUS
        } else {
            COLOR_BORDER
        },
        if hovered { 2.0 } else { 1.0 },
    );
    if matches!(tone, ButtonTone::Primary) {
        draw_rect(
            left,
            top,
            Rect {
                x: rect.x,
                y: rect.y,
                width: 4,
                height: rect.height,
            },
            [
                COLOR_AMBER_TEXT[0],
                COLOR_AMBER_TEXT[1],
                COLOR_AMBER_TEXT[2],
                1.0,
            ],
        );
    }
    draw_centered_text(left, top, rect, label, COLOR_TEXT);
}

unsafe fn draw_field(
    state: &PluginState,
    left: i32,
    top: i32,
    rect: Rect,
    field: Field,
    label: &str,
    emphasized: bool,
) {
    let action = UiAction::Edit(field);
    let hovered = state.hovered_action == Some(action);
    let active = state.active_field == Some(field);
    draw_rect(
        left,
        top,
        rect,
        if hovered || active {
            COLOR_FIELD_HOVER
        } else {
            COLOR_FIELD
        },
    );
    draw_outline(
        left,
        top,
        rect,
        if active || hovered {
            COLOR_FOCUS
        } else {
            COLOR_BORDER
        },
        if active { 2.0 } else { 1.0 },
    );
    draw_rect(
        left,
        top,
        Rect {
            x: rect.x,
            y: rect.y,
            width: if active { 4 } else { 2 },
            height: rect.height,
        },
        if active { COLOR_FOCUS } else { COLOR_BORDER },
    );
    let label_color = if emphasized {
        COLOR_MUTED
    } else {
        [0.38, 0.48, 0.53]
    };
    draw_text(
        left + rect.x + 9,
        top - rect.y - rect.height / 2 - 5,
        label,
        label_color,
    );
    let raw_value = if active {
        format!("{} |", state.form.value(field))
    } else {
        state.form.value(field).to_owned()
    };
    let label_width = text_width(label) as i32;
    let available = (rect.width - label_width - 30).max(30) as f32;
    let value = truncate_text(&raw_value, available);
    let value_width = text_width(&value) as i32;
    draw_text(
        left + rect.x + rect.width - value_width - 9,
        top - rect.y - rect.height / 2 - 5,
        &value,
        if active {
            [0.55, 0.92, 1.0]
        } else {
            COLOR_TEXT
        },
    );
}

unsafe fn draw_section_label(left: i32, top: i32, width: i32, y: i32, label: &str) {
    draw_text(left + UI_MARGIN, top - y, label, COLOR_AMBER_TEXT);
    let label_width = text_width(label) as i32;
    draw_rule(
        left + UI_MARGIN + label_width + 10,
        top - y + 4,
        left + width - UI_MARGIN,
        top - y + 4,
        COLOR_BORDER,
    );
}

unsafe fn draw_pad_selector(state: &PluginState, left: i32, top: i32, rect: Rect) {
    let hovered = state.hovered_action == Some(UiAction::ToggleDropdown);
    draw_rect(
        left,
        top,
        rect,
        if hovered || state.dropdown_open {
            COLOR_FIELD_HOVER
        } else {
            COLOR_FIELD
        },
    );
    draw_outline(
        left,
        top,
        rect,
        if hovered || state.dropdown_open {
            COLOR_FOCUS
        } else {
            COLOR_BORDER
        },
        if hovered || state.dropdown_open {
            2.0
        } else {
            1.0
        },
    );
    let count = if state.pads.is_empty() {
        "0 / 0".to_owned()
    } else {
        format!("{} / {}", state.selected_index + 1, state.pads.len())
    };
    let count_width = text_width(&count) as i32;
    let name = truncate_text(
        state.selected_name().unwrap_or("No PAD files"),
        (rect.width - count_width - 54).max(40) as f32,
    );
    draw_text(
        left + rect.x + 10,
        top - rect.y - rect.height / 2 - 5,
        &name,
        COLOR_TEXT,
    );
    draw_text(
        left + rect.x + rect.width - count_width - 28,
        top - rect.y - rect.height / 2 - 5,
        &count,
        COLOR_MUTED,
    );
    draw_text(
        left + rect.x + rect.width - 16,
        top - rect.y - rect.height / 2 - 5,
        if state.dropdown_open { "^" } else { "v" },
        COLOR_AMBER_TEXT,
    );
}

unsafe fn draw_dropdown(state: &PluginState, left: i32, top: i32, selector: Rect) {
    if !state.dropdown_open {
        return;
    }
    let visible_rows = state
        .pads
        .len()
        .saturating_sub(state.dropdown_scroll)
        .min(DROPDOWN_ROWS);
    let row_count = visible_rows.max(1);
    let list = dropdown_list_rect(selector, row_count);
    let row_top = list.y;
    draw_rect(
        left,
        top,
        Rect {
            x: list.x + 4,
            y: list.y + 4,
            width: list.width,
            height: list.height,
        },
        [0.0, 0.0, 0.0, 0.45],
    );
    draw_rect(left, top, list, COLOR_PANEL);
    draw_outline(left, top, list, COLOR_FOCUS, 2.0);

    if visible_rows == 0 {
        draw_centered_text(left, top, list, "No PAD files found", COLOR_MUTED);
        return;
    }
    for row in 0..visible_rows {
        let index = state.dropdown_scroll + row;
        let row_rect = Rect {
            x: selector.x,
            y: row_top + row as i32 * DROPDOWN_ROW_HEIGHT,
            width: selector.width - DROPDOWN_SCROLLBAR_WIDTH,
            height: DROPDOWN_ROW_HEIGHT,
        };
        let action = UiAction::SelectPad(index);
        let hovered = state.hovered_action == Some(action);
        let selected = state.selected_index == index;
        if hovered || selected {
            draw_rect(
                left,
                top,
                row_rect,
                if hovered {
                    COLOR_BUTTON_HOVER
                } else {
                    COLOR_SELECTED
                },
            );
        }
        if row > 0 {
            let (_, row_global_top, _, _) = global_rect(left, top, row_rect);
            draw_rule(
                left + row_rect.x + 8,
                row_global_top,
                left + row_rect.x + row_rect.width - 8,
                row_global_top,
                [COLOR_BORDER[0], COLOR_BORDER[1], COLOR_BORDER[2], 0.55],
            );
        }
        let number = format!("{:02}", index + 1);
        draw_text(
            left + row_rect.x + 9,
            top - row_rect.y - row_rect.height / 2 - 5,
            &number,
            if selected {
                COLOR_AMBER_TEXT
            } else {
                COLOR_MUTED
            },
        );
        let name = truncate_text(&state.pads[index], (row_rect.width - 50) as f32);
        draw_text(
            left + row_rect.x + 37,
            top - row_rect.y - row_rect.height / 2 - 5,
            &name,
            COLOR_TEXT,
        );
    }

    let (up, track, down) = dropdown_scrollbar_rects(list);
    let max_scroll = state.dropdown_max_scroll();
    let can_scroll = max_scroll > 0;
    for (rect, label, action, enabled) in [
        (
            up,
            "^",
            UiAction::ScrollDropdown(-1),
            can_scroll && state.dropdown_scroll > 0,
        ),
        (
            down,
            "v",
            UiAction::ScrollDropdown(1),
            can_scroll && state.dropdown_scroll < max_scroll,
        ),
    ] {
        let hovered = state.hovered_action == Some(action);
        draw_rect(
            left,
            top,
            rect,
            if hovered && enabled {
                COLOR_BUTTON_HOVER
            } else {
                COLOR_FIELD
            },
        );
        draw_outline(
            left,
            top,
            rect,
            if hovered && enabled {
                COLOR_FOCUS
            } else {
                COLOR_BORDER
            },
            if hovered && enabled { 2.0 } else { 1.0 },
        );
        draw_centered_text(
            left,
            top,
            rect,
            label,
            if enabled { COLOR_TEXT } else { COLOR_MUTED },
        );
    }
    draw_rect(left, top, track, COLOR_FIELD);
    draw_outline(left, top, track, COLOR_BORDER, 1.0);
    let thumb = dropdown_thumb_rect(track, state.dropdown_scroll, state.pads.len());
    draw_rect(
        left,
        top,
        thumb,
        if can_scroll {
            COLOR_FOCUS
        } else {
            [COLOR_MUTED[0], COLOR_MUTED[1], COLOR_MUTED[2], 0.45]
        },
    );
}

pub(in crate::runtime) unsafe extern "C" fn draw_window(
    window: XPLMWindowID,
    _refcon: *mut c_void,
) {
    let guard = state_lock();
    let Some(state) = guard.as_ref() else { return };
    let mut left = 0;
    let mut top = 0;
    let mut right = 0;
    let mut bottom = 0;
    XPLMGetWindowGeometry(window, &mut left, &mut top, &mut right, &mut bottom);
    let width = right - left;
    let height = top - bottom;

    draw_rect(
        left,
        top,
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        COLOR_CANVAS,
    );
    draw_text(
        left + UI_MARGIN,
        top - 23,
        "POSITION AIRCRAFT",
        COLOR_AMBER_TEXT,
    );
    let mode = "NATIVE  /  VR  /  PAD";
    draw_text(
        right - UI_MARGIN - text_width(mode) as i32,
        top - 23,
        mode,
        COLOR_MUTED,
    );
    draw_rule(
        left + UI_MARGIN,
        top - 32,
        right - UI_MARGIN,
        top - 32,
        COLOR_BORDER,
    );

    let button_labels = [
        "Capture current",
        "Position aircraft",
        "Quick save",
        "Quick load + position",
    ];
    for (index, (rect, action)) in command_button_rects(width).into_iter().enumerate() {
        draw_button(
            state,
            left,
            top,
            ButtonSpec {
                rect,
                label: button_labels[index],
                action: UiAction::Command(action),
                tone: if matches!(
                    action,
                    CommandAction::PositionLoaded | CommandAction::QuickLoadAndPosition
                ) {
                    ButtonTone::Primary
                } else {
                    ButtonTone::Standard
                },
                selected: false,
            },
        );
    }

    draw_section_label(left, top, width, 103, "PAD LIBRARY");
    let pad = pad_layout(width);
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.previous,
            label: "<",
            action: UiAction::Command(CommandAction::PreviousPad),
            tone: ButtonTone::Quiet,
            selected: false,
        },
    );
    draw_pad_selector(state, left, top, pad.selector);
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.next,
            label: ">",
            action: UiAction::Command(CommandAction::NextPad),
            tone: ButtonTone::Quiet,
            selected: false,
        },
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.refresh,
            label: "Refresh",
            action: UiAction::Refresh,
            tone: ButtonTone::Quiet,
            selected: false,
        },
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.load,
            label: "Load",
            action: UiAction::LoadSelected(false),
            tone: ButtonTone::Standard,
            selected: false,
        },
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.load_and_position,
            label: "Load + position",
            action: UiAction::LoadSelected(true),
            tone: ButtonTone::Primary,
            selected: false,
        },
    );

    draw_section_label(left, top, width, 181, "AIRCRAFT STATE");
    for (index, (field, label)) in position_fields().into_iter().enumerate() {
        draw_field(
            state,
            left,
            top,
            field_rect(width, POSITION_Y, index),
            field,
            label,
            true,
        );
    }

    let ap_rect = Rect {
        x: UI_MARGIN,
        y: AP_TOGGLE_Y,
        width: width - UI_MARGIN * 2,
        height: 32,
    };
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: ap_rect,
            label: if state.form.use_ap {
                "AUTOPILOT DATA    [ APPLIED ON POSITION ]"
            } else {
                "AUTOPILOT DATA    [ NOT APPLIED ]"
            },
            action: UiAction::ToggleAp,
            tone: ButtonTone::Quiet,
            selected: state.form.use_ap,
        },
    );
    for (index, (field, label)) in autopilot_fields().into_iter().enumerate() {
        draw_field(
            state,
            left,
            top,
            field_rect(width, AP_FIELDS_Y, index),
            field,
            label,
            state.form.use_ap,
        );
    }

    let (save_field, save_button) = save_layout(width);
    draw_field(
        state,
        left,
        top,
        save_field,
        Field::SaveName,
        "Save as",
        true,
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: save_button,
            label: "Save PAD",
            action: UiAction::SaveNamed,
            tone: ButtonTone::Primary,
            selected: false,
        },
    );

    let status_is_error = ["Unable", "No ", "Enter ", "Invalid"]
        .iter()
        .any(|prefix| state.status.starts_with(prefix));
    draw_rect(
        left,
        top,
        Rect {
            x: UI_MARGIN,
            y: 618,
            width: 5,
            height: 16,
        },
        if status_is_error {
            [0.95, 0.22, 0.16, 1.0]
        } else {
            [0.16, 0.78, 0.52, 1.0]
        },
    );
    let status = truncate_text(&state.status, (width - UI_MARGIN * 2 - 20) as f32);
    draw_text(
        left + UI_MARGIN + 13,
        top - 629,
        &status,
        if status_is_error {
            [1.0, 0.60, 0.52]
        } else {
            [0.68, 0.92, 0.80]
        },
    );

    draw_dropdown(state, left, top, pad.selector);
}
