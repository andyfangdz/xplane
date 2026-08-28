use egui::{
    text::{LayoutJob, TextFormat},
    vec2, Align, Button, CollapsingHeader, Color32, ComboBox, FontId, Frame, Grid, Layout, Margin,
    Rect, Response, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui,
};

use crate::pad::{Field, Form};
use crate::runtime::{CommandAction, PanelTab, PluginState};

use super::pattern_tab;
use super::theme::*;

#[derive(Copy, Clone)]
pub(super) enum HitCursor {
    Arrow,
    Text,
}

#[derive(Copy, Clone)]
pub(super) struct HitRegion {
    pub(super) rect: Rect,
    pub(super) cursor: HitCursor,
}

pub(super) enum Action {
    Command(CommandAction),
    LoadSelected(bool),
    Refresh,
    SelectPad(usize),
    SaveNamed,
    ResolvePatternAirport,
    NearestPatternAirport,
    SelectPatternPad(String),
    PatternSettingsChanged,
    PositionPattern,
    SavePatternPad,
}

pub(super) struct ViewOutput {
    pub(super) actions: Vec<Action>,
    pub(super) hit_regions: Vec<HitRegion>,
}

impl ViewOutput {
    fn new() -> Self {
        Self {
            actions: Vec::new(),
            hit_regions: Vec::new(),
        }
    }

    pub(super) fn track(&mut self, response: &Response, cursor: HitCursor) {
        if response.enabled() {
            self.hit_regions.push(HitRegion {
                rect: response.rect,
                cursor,
            });
        }
    }
}

#[derive(Copy, Clone)]
pub(super) enum ButtonTone {
    Primary,
    Movement,
    Quiet,
}

pub(super) fn show(ui: &mut Ui, state: &mut PluginState) -> ViewOutput {
    let mut output = ViewOutput::new();
    let settings_before = state.pattern.settings.clone();

    egui::CentralPanel::default()
        .frame(Frame::new().fill(CANVAS).inner_margin(Margin::same(15)))
        .show(ui, |ui| {
            header(ui, state);
            ui.add_space(8.0);
            tab_bar(ui, state, &mut output);
            ui.add_space(8.0);
            let body_height = (ui.available_height() - 44.0).max(200.0);
            let scroll_id = match state.pattern.settings.active_tab {
                PanelTab::Pad => "position-aircraft-pad-body",
                PanelTab::Pattern => "position-aircraft-pattern-body",
            };
            let scroll = ScrollArea::vertical()
                .id_salt(scroll_id)
                .max_height(body_height)
                .auto_shrink([false, false])
                .show(ui, |ui| match state.pattern.settings.active_tab {
                    PanelTab::Pad => pad_tab(ui, state, &mut output),
                    PanelTab::Pattern => pattern_tab::show(ui, state, &mut output),
                });
            // XPLM asks the adapter whether wheel input belongs to this window
            // before egui sees it. Track the full viewport so blank space in a
            // diagram or card scrolls just like the controls inside it.
            output.hit_regions.push(HitRegion {
                rect: scroll.inner_rect,
                cursor: HitCursor::Arrow,
            });
            ui.add_space(7.0);
            status_bar(ui, &state.status);
        });

    if state.pattern.settings != settings_before {
        output.actions.push(Action::PatternSettingsChanged);
    }

    output
}

fn pad_tab(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    quick_actions(ui, output);
    ui.add_space(10.0);
    pad_library(ui, state, output);
    ui.add_space(10.0);
    aircraft_state(ui, state, output);
    ui.add_space(10.0);
    autopilot(ui, state, output);
    ui.add_space(10.0);
    save_panel(ui, state, output);
}

fn header(ui: &mut Ui, state: &PluginState) {
    ui.horizontal(|ui| {
        let (marker, _) = ui.allocate_exact_size(vec2(5.0, 34.0), Sense::hover());
        ui.painter().rect_filled(marker, 2.0, AMBER);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("POSITION AIRCRAFT").heading().strong());
            ui.label(
                RichText::new(match state.pattern.settings.active_tab {
                    PanelTab::Pad => "PAD positioning console",
                    PanelTab::Pattern => "Visual traffic-pattern placement",
                })
                .small()
                .color(MUTED),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let count = match state.pattern.settings.active_tab {
                PanelTab::Pad => format!("{} PAD files", state.pads.len()),
                PanelTab::Pattern => state
                    .airports
                    .as_ref()
                    .map(|database| format!("{} airports", database.airport_count()))
                    .unwrap_or_else(|| "Airport data unavailable".to_owned()),
            };
            ui.label(RichText::new(count).small().color(MUTED));
        });
    });
}

fn tab_bar(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(5.0)
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ui.columns(2, |columns| {
                let tabs = [
                    (PanelTab::Pad, "PAD FILE"),
                    (PanelTab::Pattern, "TRAFFIC PATTERN"),
                ];
                for (column, (tab, label)) in columns.iter_mut().zip(tabs) {
                    let selected = state.pattern.settings.active_tab == tab;
                    let response = column.add_sized(
                        [column.available_width(), 36.0],
                        Button::new(RichText::new(label).strong().color(if selected {
                            TEXT
                        } else {
                            MUTED
                        }))
                        .selected(selected)
                        .fill(if selected {
                            Color32::from_rgb(38, 91, 114)
                        } else {
                            PANEL
                        }),
                    );
                    output.track(&response, HitCursor::Arrow);
                    if response.clicked() {
                        state.pattern.settings.active_tab = tab;
                    }
                }
            });
        });
}

fn quick_actions(ui: &mut Ui, output: &mut ViewOutput) {
    ui.columns(4, |columns| {
        if action_button(
            &mut columns[0],
            "Capture current",
            "Read the current aircraft position and state into the fields",
            ButtonTone::Primary,
            output,
        ) {
            output
                .actions
                .push(Action::Command(CommandAction::CaptureCurrent));
        }
        if action_button(
            &mut columns[1],
            "Position loaded",
            "Move the aircraft using the values currently shown",
            ButtonTone::Movement,
            output,
        ) {
            output
                .actions
                .push(Action::Command(CommandAction::PositionLoaded));
        }
        if action_button(
            &mut columns[2],
            "Quick save",
            "Capture and overwrite QuickFile.pad",
            ButtonTone::Quiet,
            output,
        ) {
            output
                .actions
                .push(Action::Command(CommandAction::QuickSave));
        }
        if action_button(
            &mut columns[3],
            "Quick position",
            "Load QuickFile.pad and immediately position the aircraft",
            ButtonTone::Movement,
            output,
        ) {
            output
                .actions
                .push(Action::Command(CommandAction::QuickLoadAndPosition));
        }
    });
}

fn pad_library(ui: &mut Ui, state: &PluginState, output: &mut ViewOutput) {
    let header = collapsible_card(
        ui,
        "pad-library-section",
        "PAD LIBRARY",
        "Choose a saved aircraft state",
        |ui| {
            ui.horizontal(|ui| {
                if compact_button(ui, "◀", "Previous PAD", output) {
                    output
                        .actions
                        .push(Action::Command(CommandAction::PreviousPad));
                }

                let fixed_width = 32.0 + 32.0 + 68.0 + 68.0 + 120.0 + 5.0 * 8.0;
                let combo_width = (ui.available_width() - fixed_width).max(150.0);
                let selected = state
                    .selected_name()
                    .unwrap_or("No PAD files found")
                    .to_owned();
                let combo = ComboBox::from_id_salt("pad-library")
                    .width(combo_width)
                    .height(250.0)
                    .selected_text(selected)
                    .truncate()
                    .show_ui(ui, |ui| {
                        if state.pads.is_empty() {
                            ui.label(RichText::new("No PAD files found").color(MUTED));
                        }
                        for (index, name) in state.pads.iter().enumerate() {
                            let response = ui.selectable_label(index == state.selected_index, name);
                            output.track(&response, HitCursor::Arrow);
                            if response.clicked() {
                                output.actions.push(Action::SelectPad(index));
                            }
                        }
                    });
                output.track(&combo.response, HitCursor::Arrow);

                if compact_button(ui, "▶", "Next PAD", output) {
                    output.actions.push(Action::Command(CommandAction::NextPad));
                }
                if small_button(ui, "Refresh", "Rescan the PAD directory", output) {
                    output.actions.push(Action::Refresh);
                }
                if small_button(ui, "Load", "Load the selected PAD into the fields", output) {
                    output.actions.push(Action::LoadSelected(false));
                }
                if action_button_sized(
                    ui,
                    "Load + position",
                    "Load the selected PAD and immediately move the aircraft",
                    ButtonTone::Movement,
                    vec2(120.0, 30.0),
                    output,
                ) {
                    output.actions.push(Action::LoadSelected(true));
                }
            });
        },
    );
    output.track(&header, HitCursor::Arrow);
}

fn aircraft_state(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    let header = collapsible_card(
        ui,
        "aircraft-state-section",
        "AIRCRAFT STATE",
        "Magnetic heading and indicated airspeed",
        |ui| {
            let fields = [
                (Field::Latitude, "Latitude", "degrees"),
                (Field::Longitude, "Longitude", "degrees"),
                (Field::Altitude, "Altitude", "feet MSL"),
                (Field::Heading, "Heading", "magnetic °"),
                (Field::Pitch, "Pitch", "degrees"),
                (Field::Roll, "Roll", "degrees"),
                (Field::Speed, "Speed", "KIAS"),
                (Field::Throttle, "Throttle", "0 to 1"),
                (Field::Flaps, "Flaps", "0 to 1"),
                (Field::Gear, "Gear", "0 or 1"),
            ];
            field_grid(
                ui,
                "aircraft-fields",
                &mut state.form,
                &fields,
                true,
                output,
            );
        },
    );
    output.track(&header, HitCursor::Arrow);
}

fn autopilot(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    let header = collapsible_card(
        ui,
        "autopilot-section",
        "AUTOPILOT",
        "Optional values restored after positioning",
        |ui| {
            let response =
                right_aligned_checkbox(ui, &mut state.form.use_ap, "Apply autopilot data");
            output.track(&response, HitCursor::Arrow);
            ui.add_space(7.0);
            let fields = [
                (Field::ApMode, "AP mode", "integer"),
                (Field::ApState, "AP state", "flags"),
                (Field::ApAltitude, "Altitude", "feet"),
                (Field::ApVerticalVelocity, "Vertical speed", "fpm"),
                (Field::ApHeading, "Heading", "magnetic °"),
                (Field::ApAirspeed, "Airspeed", "knots"),
                (Field::ApHeadingRollMode, "Bank limit", "mode"),
            ];
            let use_ap = state.form.use_ap;
            field_grid(
                ui,
                "autopilot-fields",
                &mut state.form,
                &fields,
                use_ap,
                output,
            );
        },
    );
    output.track(&header, HitCursor::Arrow);
}

fn right_aligned_checkbox(ui: &mut Ui, checked: &mut bool, label: &str) -> Response {
    // A bare right-to-left child uses all remaining vertical space so that it can center its
    // contents. Give this control an explicit row height to keep the following fields adjacent.
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), 24.0),
        Layout::right_to_left(Align::Center),
        |ui| ui.checkbox(checked, label),
    )
    .inner
}

fn save_panel(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    let header = collapsible_card(
        ui,
        "save-pad-section",
        "SAVE",
        "Write the displayed values to a PAD file",
        |ui| {
            ui.horizontal(|ui| {
                let button_width = 105.0;
                let edit_width = (ui.available_width() - button_width - 8.0).max(180.0);
                let response = ui.add_sized(
                    [edit_width, 30.0],
                    TextEdit::singleline(state.form.value_mut(Field::SaveName))
                        .char_limit(63)
                        .hint_text("PAD filename")
                        .background_color(FIELD),
                );
                output.track(&response, HitCursor::Text);
                if action_button_sized(
                    ui,
                    "Save PAD",
                    "Save the displayed values using this filename",
                    ButtonTone::Primary,
                    vec2(button_width, 30.0),
                    output,
                ) {
                    output.actions.push(Action::SaveNamed);
                }
            });
        },
    );
    output.track(&header, HitCursor::Arrow);
}

fn field_grid(
    ui: &mut Ui,
    id: &'static str,
    form: &mut Form,
    fields: &[(Field, &'static str, &'static str)],
    enabled: bool,
    output: &mut ViewOutput,
) {
    Grid::new(id)
        .num_columns(4)
        .spacing(vec2(9.0, 6.0))
        .show(ui, |ui| {
            for row in fields.chunks(2) {
                for slot in 0..2 {
                    if let Some((field, label, units)) = row.get(slot).copied() {
                        ui.label(RichText::new(label).color(MUTED));
                        ui.add_enabled_ui(enabled, |ui| {
                            let response = ui.add_sized(
                                [190.0, 28.0],
                                TextEdit::singleline(form.value_mut(field))
                                    .char_limit(63)
                                    .hint_text(units)
                                    .background_color(FIELD),
                            );
                            if enabled {
                                output.track(&response, HitCursor::Text);
                            }
                        });
                    } else {
                        ui.label("");
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
}

pub(super) fn card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let available_width = ui.available_width();
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(5.0)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width((available_width - 26.0).max(0.0));
            contents(ui)
        })
        .inner
}

pub(super) fn collapsible_card<R>(
    ui: &mut Ui,
    id: &'static str,
    title: &str,
    detail: &str,
    contents: impl FnOnce(&mut Ui) -> R,
) -> Response {
    card(ui, |ui| {
        ui.scope(|ui| {
            let content_width = ui.available_width();
            ui.visuals_mut().collapsing_header_frame = true;
            CollapsingHeader::new(section_header_text(title, detail))
                .id_salt(id)
                .default_open(true)
                .show_background(true)
                .show_unindented(ui, |ui| {
                    // A collapsing body's child UI sizes itself from its contents. Preserve the
                    // card's width explicitly so horizontal layouts can claim their remaining
                    // space (most visibly, the traffic-pattern diagram beside its selector).
                    ui.set_width(content_width);
                    // Keep the full-row frame local to this section header. A nested collapsing
                    // control may intentionally be narrow, such as the starting-point selector
                    // beside the traffic-pattern diagram.
                    ui.visuals_mut().collapsing_header_frame = false;
                    ui.add_space(7.0);
                    contents(ui)
                })
                .header_response
        })
        .inner
    })
}

fn section_header_text(title: &str, detail: &str) -> LayoutJob {
    let mut text = LayoutJob::default();
    text.append(
        title,
        0.0,
        TextFormat {
            font_id: FontId::proportional(14.0),
            color: AMBER,
            valign: Align::Center,
            ..Default::default()
        },
    );
    text.append(
        &format!("   {detail}"),
        0.0,
        TextFormat {
            font_id: FontId::proportional(11.0),
            color: MUTED,
            valign: Align::Center,
            ..Default::default()
        },
    );
    text
}

fn status_bar(ui: &mut Ui, status: &str) {
    let is_error = ["Unable", "No ", "Enter ", "Invalid", "UI renderer"]
        .iter()
        .any(|prefix| status.starts_with(prefix));
    let color = if is_error { ERROR } else { GOOD };
    Frame::new()
        .fill(Color32::from_rgb(24, 33, 39))
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(4.0)
        .inner_margin(Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(color));
                ui.label(RichText::new(status).color(color));
            });
        });
}

fn action_button(
    ui: &mut Ui,
    label: &str,
    help: &str,
    tone: ButtonTone,
    output: &mut ViewOutput,
) -> bool {
    action_button_sized(
        ui,
        label,
        help,
        tone,
        vec2(ui.available_width(), 36.0),
        output,
    )
}

pub(super) fn action_button_sized(
    ui: &mut Ui,
    label: &str,
    help: &str,
    tone: ButtonTone,
    size: egui::Vec2,
    output: &mut ViewOutput,
) -> bool {
    let (fill, hover, text) = match tone {
        ButtonTone::Primary => (Color32::from_rgb(48, 102, 127), SKY_HOVER, TEXT),
        ButtonTone::Movement => (AMBER, AMBER_HOVER, FIELD),
        ButtonTone::Quiet => (Color32::from_rgb(42, 54, 63), SKY, TEXT),
    };
    let response = ui
        .add_sized(
            size,
            Button::new(RichText::new(label).strong().color(text))
                .fill(fill)
                .stroke(Stroke::new(1.0, hover)),
        )
        .on_hover_text(help);
    output.track(&response, HitCursor::Arrow);
    response.clicked()
}

fn compact_button(ui: &mut Ui, label: &str, help: &str, output: &mut ViewOutput) -> bool {
    let response = ui
        .add_sized([32.0, 30.0], Button::new(label))
        .on_hover_text(help);
    output.track(&response, HitCursor::Arrow);
    response.clicked()
}

pub(super) fn small_button(ui: &mut Ui, label: &str, help: &str, output: &mut ViewOutput) -> bool {
    let response = ui
        .add_sized([68.0, 30.0], Button::new(label))
        .on_hover_text(help);
    output.track(&response, HitCursor::Arrow);
    response.clicked()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use egui::{vec2, CentralPanel, CollapsingHeader, Context, Pos2, RawInput, Rect};

    use super::collapsible_card;

    #[test]
    fn collapsible_card_preserves_width_for_horizontal_contents() {
        let context = Context::default();
        let remaining_width = Cell::new(0.0);
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(760.0, 760.0))),
            ..Default::default()
        };

        let mut output = context.run_ui(input, |ui| {
            CentralPanel::default().show(ui, |ui| {
                collapsible_card(ui, "width-test", "TEST", "Detail", |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            CollapsingHeader::new("Nested")
                                .default_open(true)
                                .show_unindented(ui, |ui| ui.set_width(142.0));
                        });
                        remaining_width.set(ui.available_width());
                    });
                });
            });
        });
        output.textures_delta.clear();

        assert!(
            remaining_width.get() > 500.0,
            "remaining horizontal width was {}",
            remaining_width.get()
        );
    }
}
