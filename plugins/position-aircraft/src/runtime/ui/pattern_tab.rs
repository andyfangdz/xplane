use egui::{
    vec2, Align2, Button, Color32, ComboBox, CornerRadius, DragValue, FontId, Pos2, Rect, RichText,
    Sense, Shape, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};

use crate::runtime::{PatternDirection, PatternLocation, PluginState};

use super::theme::*;
use super::view::{
    action_button_sized, card, section_header, small_button, Action, ButtonTone, HitCursor,
    ViewOutput,
};

pub(super) fn show(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    airport_and_configuration(ui, state, output);
    ui.add_space(10.0);
    geometry_controls(ui, state, output);
    ui.add_space(10.0);
    location_and_diagram(ui, state, output);
    ui.add_space(10.0);
    placement_actions(ui, state, output);
}

fn airport_and_configuration(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    card(ui, |ui| {
        section_header(
            ui,
            "AIRPORT + AIRCRAFT",
            "Runway geometry from active X-Plane scenery",
        );
        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Airport").color(MUTED));
            let response = ui.add_sized(
                [102.0, 30.0],
                TextEdit::singleline(&mut state.pattern.airport_input)
                    .char_limit(8)
                    .hint_text("ICAO / ID")
                    .background_color(FIELD),
            );
            output.track(&response, HitCursor::Text);
            let submit =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if small_button(ui, "Use", "Find this airport in active scenery", output) || submit {
                output.actions.push(Action::ResolvePatternAirport);
            }
            if small_button(
                ui,
                "Nearest",
                "Use the airport nearest the aircraft's current position",
                output,
            ) {
                output.actions.push(Action::NearestPatternAirport);
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Runway").color(MUTED));
            let runway_ids = state
                .airports
                .as_ref()
                .map(|database| database.runway_ids(&state.pattern.settings.airport_id))
                .unwrap_or_default();
            let combo = ComboBox::from_id_salt("pattern-runway")
                .width(82.0)
                .selected_text(if state.pattern.settings.runway_id.is_empty() {
                    "Choose"
                } else {
                    &state.pattern.settings.runway_id
                })
                .show_ui(ui, |ui| {
                    for id in runway_ids {
                        let response = ui.selectable_value(
                            &mut state.pattern.settings.runway_id,
                            id.clone(),
                            format!("RWY {id}"),
                        );
                        output.track(&response, HitCursor::Arrow);
                    }
                });
            output.track(&combo.response, HitCursor::Arrow);
        });

        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Configuration").color(MUTED));
            let selected = if state.pattern.settings.configuration_pad.is_empty() {
                "No PAD files found".to_owned()
            } else {
                state.pattern.settings.configuration_pad.clone()
            };
            let combo = ComboBox::from_id_salt("pattern-configuration-pad")
                .width((ui.available_width() * 0.48).max(220.0))
                .height(250.0)
                .selected_text(selected)
                .truncate()
                .show_ui(ui, |ui| {
                    for name in &state.pads {
                        let response = ui.selectable_label(
                            *name == state.pattern.settings.configuration_pad,
                            name,
                        );
                        output.track(&response, HitCursor::Arrow);
                        if response.clicked() {
                            output.actions.push(Action::SelectPatternPad(name.clone()));
                        }
                    }
                });
            output.track(&combo.response, HitCursor::Arrow);

            let airport = state
                .airports
                .as_ref()
                .and_then(|database| database.airport(&state.pattern.settings.airport_id));
            if let Some(airport) = airport {
                ui.label(
                    RichText::new(format!("{} · {}", airport.id, airport.name))
                        .small()
                        .color(MUTED),
                );
            }
        });

        if let Some(preview) = state.pattern.preview.as_ref() {
            ui.add_space(5.0);
            ui.label(
                RichText::new(format!(
                    "Loaded state: {:.0} KIAS · {:.0}% throttle · {:.0}% flaps · gear {}{}",
                    preview.data.speed,
                    preview.data.throttle * 100.0,
                    preview.data.flaps * 100.0,
                    if preview.data.gear != 0 { "down" } else { "up" },
                    if preview.data.use_ap {
                        " · autopilot data"
                    } else {
                        ""
                    },
                ))
                .small()
                .color(MUTED),
            );
        }
    });
}

fn geometry_controls(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    card(ui, |ui| {
        section_header(
            ui,
            "PATTERN GEOMETRY",
            "Distances are measured from the displaced threshold",
        );
        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Direction").color(MUTED));
            for direction in [PatternDirection::Left, PatternDirection::Right] {
                let selected = state.pattern.settings.direction == direction;
                let response = ui.selectable_label(selected, direction.label());
                output.track(&response, HitCursor::Arrow);
                if response.clicked() {
                    state.pattern.settings.direction = direction;
                }
            }
            ui.add_space(15.0);
            number_control(
                ui,
                "Approach",
                &mut state.pattern.settings.approach_angle_deg,
                1.0..=10.0,
                "°",
                "Glide angle used to calculate altitude",
                output,
            );
        });
        ui.add_space(7.0);
        ui.columns(3, |columns| {
            number_control(
                &mut columns[0],
                "Downwind offset",
                &mut state.pattern.settings.downwind_offset_nm,
                0.2..=10.0,
                " NM",
                "Lateral distance between downwind and runway centerline",
                output,
            );
            number_control(
                &mut columns[1],
                "Base intercept",
                &mut state.pattern.settings.base_intercept_nm,
                0.2..=20.0,
                " NM",
                "Final-leg distance from the threshold at the base intersection",
                output,
            );
            number_control(
                &mut columns[2],
                "Final distance",
                &mut state.pattern.settings.final_distance_nm,
                0.2..=30.0,
                " NM",
                "Distance from threshold for the On final location",
                output,
            );
        });
    });
}

fn number_control(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    help: &str,
    output: &mut ViewOutput,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(MUTED));
        let response = ui
            .add(
                DragValue::new(value)
                    .range(range)
                    .speed(0.1)
                    .fixed_decimals(1)
                    .suffix(suffix),
            )
            .on_hover_text(help);
        output.track(&response, HitCursor::Text);
    });
}

fn location_and_diagram(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    card(ui, |ui| {
        section_header(
            ui,
            "SELECT A STARTING POINT",
            state.pattern.settings.location.detail(),
        );
        ui.add_space(7.0);
        ui.columns(PatternLocation::ALL.len(), |columns| {
            for (column, location) in columns.iter_mut().zip(PatternLocation::ALL) {
                let selected = state.pattern.settings.location == location;
                let response = column.add_sized(
                    [column.available_width(), 29.0],
                    Button::new(location.label())
                        .selected(selected)
                        .fill(if selected { MAGENTA_MUTED } else { PANEL }),
                );
                output.track(&response, HitCursor::Arrow);
                if response.clicked() {
                    state.pattern.settings.location = location;
                }
            }
        });
        ui.add_space(7.0);
        pattern_diagram(ui, state, output);

        if let Some(preview) = state.pattern.preview.as_ref() {
            ui.add_space(7.0);
            ui.horizontal_wrapped(|ui| {
                summary_value(
                    ui,
                    "ALTITUDE",
                    format!("{:.0} ft MSL", preview.data.altitude),
                );
                summary_value(
                    ui,
                    "HEIGHT",
                    format!("{:.0} ft AGL", preview.altitude_agl_ft),
                );
                summary_value(
                    ui,
                    "HEADING",
                    format!(
                        "{:03.0}°M · {:03.0}°T",
                        preview.data.heading, preview.true_heading_deg
                    ),
                );
                summary_value(
                    ui,
                    "PATH LEFT",
                    format!("{:.1} NM", preview.remaining_path_nm),
                );
            });
        } else if let Some(error) = state.pattern.preview_error.as_deref() {
            ui.add_space(7.0);
            ui.label(RichText::new(error).color(ERROR));
        }
    });
}

fn summary_value(ui: &mut Ui, label: &str, value: String) {
    ui.label(RichText::new(label).small().strong().color(MAGENTA));
    ui.label(RichText::new(value).color(TEXT));
    ui.add_space(8.0);
}

fn placement_actions(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    card(ui, |ui| {
        section_header(
            ui,
            "POSITION OR SAVE",
            "The generated PAD keeps the selected aircraft configuration",
        );
        ui.add_space(7.0);
        ui.horizontal(|ui| {
            let position_width = 150.0;
            let save_width = 105.0;
            let edit_width = (ui.available_width() - position_width - save_width - 16.0).max(150.0);
            let response = ui.add_sized(
                [edit_width, 32.0],
                TextEdit::singleline(&mut state.pattern.save_name)
                    .char_limit(63)
                    .hint_text("New PAD filename")
                    .background_color(FIELD),
            );
            output.track(&response, HitCursor::Text);
            if action_button_sized(
                ui,
                "Save PAD",
                "Save this generated location as a standard PAD file",
                ButtonTone::Primary,
                vec2(save_width, 32.0),
                output,
            ) {
                output.actions.push(Action::SavePatternPad);
            }
            if action_button_sized(
                ui,
                "Position aircraft",
                "Move the aircraft to the selected pattern point",
                ButtonTone::Movement,
                vec2(position_width, 32.0),
                output,
            ) {
                output.actions.push(Action::PositionPattern);
            }
        });
    });
}

fn pattern_diagram(ui: &mut Ui, state: &mut PluginState, output: &mut ViewOutput) {
    // Keep the complete pattern and selected aircraft visible at the minimum
    // supported window height; the details/actions below can still scroll.
    let desired = vec2(ui.available_width(), 238.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(5), DIAGRAM);
    painter.rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );

    for fraction in [0.25, 0.5, 0.75] {
        let y = egui::lerp(rect.top()..=rect.bottom(), fraction);
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, Color32::from_rgb(24, 39, 47)),
        );
    }

    let center_x = rect.center().x;
    let runway_top = rect.top() + 31.0;
    let runway_bottom = rect.bottom() - 83.0;
    let runway_width = 42.0;
    let displacement_ratio = state
        .pattern
        .preview
        .as_ref()
        .map(|preview| {
            (preview.runway.end.displaced_threshold_m / preview.runway.length_m.max(1.0)) as f32
        })
        .unwrap_or(0.0);
    let displacement_px = if displacement_ratio > 0.0 {
        (displacement_ratio * (runway_bottom - runway_top)).clamp(8.0, 35.0)
    } else {
        0.0
    };
    let threshold_y = runway_bottom - displacement_px;
    let runway_rect = Rect::from_min_max(
        Pos2::new(center_x - runway_width * 0.5, runway_top),
        Pos2::new(center_x + runway_width * 0.5, runway_bottom),
    );
    painter.rect_filled(runway_rect, CornerRadius::same(2), RUNWAY);
    painter.line_segment(
        [
            Pos2::new(runway_rect.left(), threshold_y),
            Pos2::new(runway_rect.right(), threshold_y),
        ],
        Stroke::new(3.0, AMBER),
    );
    if displacement_px > 0.0 {
        let mut y = threshold_y + 5.0;
        while y < runway_bottom {
            painter.line_segment(
                [
                    Pos2::new(runway_rect.left() + 3.0, y),
                    Pos2::new(runway_rect.right() - 3.0, (y + 8.0).min(runway_bottom)),
                ],
                Stroke::new(1.0, Color32::from_rgb(164, 139, 87)),
            );
            y += 8.0;
        }
    }
    let runway_id = state
        .pattern
        .preview
        .as_ref()
        .map(|preview| preview.runway.end.id.as_str())
        .unwrap_or("--");
    let opposite_id = state
        .pattern
        .preview
        .as_ref()
        .map(|preview| preview.runway.opposite.id.as_str())
        .unwrap_or("--");
    painter.text(
        Pos2::new(center_x, threshold_y - 7.0),
        Align2::CENTER_BOTTOM,
        runway_id,
        FontId::proportional(12.0),
        TEXT,
    );
    painter.text(
        Pos2::new(center_x, runway_top + 7.0),
        Align2::CENTER_TOP,
        opposite_id,
        FontId::proportional(11.0),
        MUTED,
    );

    let side = match state.pattern.settings.direction {
        PatternDirection::Left => -1.0,
        PatternDirection::Right => 1.0,
    };
    let side_x = center_x + side * (rect.width() * 0.29).min(185.0);
    let crosswind_y = runway_top + 14.0;
    let base_y = rect.bottom() - 51.0;
    let final_y = rect.bottom() - 19.0;
    let downwind_y = (crosswind_y + base_y) * 0.5;
    let join_y = downwind_y - 5.0;
    let pattern_points = vec![
        Pos2::new(center_x, final_y),
        Pos2::new(center_x, runway_top),
        Pos2::new(side_x, crosswind_y),
        Pos2::new(side_x, base_y),
        Pos2::new(center_x, base_y),
        Pos2::new(center_x, final_y),
    ];
    painter.add(Shape::line(pattern_points, Stroke::new(2.5, MAGENTA)));

    let entry = Pos2::new(side_x + side * 58.0, join_y - 43.0);
    painter.line_segment(
        [entry, Pos2::new(side_x, join_y)],
        Stroke::new(1.5, MAGENTA_MUTED),
    );
    let intercept = Pos2::new(center_x + side * 73.0, base_y + 8.0);
    painter.line_segment(
        [intercept, Pos2::new(center_x, base_y - 25.0)],
        Stroke::new(1.5, MAGENTA_MUTED),
    );

    let locations = [
        (
            PatternLocation::OnFinal,
            Pos2::new(center_x, final_y),
            vec2(0.0, -1.0),
        ),
        (
            PatternLocation::InterceptFinal,
            intercept,
            vec2(-side, -1.0).normalized(),
        ),
        (
            PatternLocation::Base,
            Pos2::new((side_x + center_x) * 0.5, base_y),
            vec2(-side, 0.0),
        ),
        (
            PatternLocation::Downwind,
            Pos2::new(side_x, downwind_y),
            vec2(0.0, 1.0),
        ),
        (PatternLocation::Entry, entry, vec2(-side, 1.0).normalized()),
    ];

    for (location, point, direction) in locations {
        let hit_rect = Rect::from_center_size(point, vec2(34.0, 34.0));
        let response = ui.interact(
            hit_rect,
            ui.id().with(("pattern-location", location as u8)),
            Sense::click(),
        );
        output.track(&response, HitCursor::Arrow);
        if response.clicked() {
            state.pattern.settings.location = location;
        }
        let selected = state.pattern.settings.location == location;
        if selected || response.hovered() {
            painter.circle_filled(
                point,
                if selected { 18.0 } else { 16.0 },
                if selected {
                    Color32::from_rgba_unmultiplied(218, 92, 188, 55)
                } else {
                    Color32::from_rgba_unmultiplied(116, 174, 199, 35)
                },
            );
        }
        draw_aircraft(
            &painter,
            point,
            direction,
            if selected {
                AMBER
            } else if response.hovered() {
                SKY_HOVER
            } else {
                MUTED
            },
        );
        let label_offset = match location {
            PatternLocation::OnFinal => vec2(14.0, 0.0),
            PatternLocation::InterceptFinal => vec2(-side * 15.0, 17.0),
            PatternLocation::Base => vec2(0.0, -15.0),
            PatternLocation::Downwind => vec2(-side * 15.0, 0.0),
            PatternLocation::Entry => vec2(-side * 15.0, -13.0),
        };
        painter.text(
            point + label_offset,
            if side < 0.0 && location != PatternLocation::OnFinal {
                Align2::LEFT_CENTER
            } else {
                Align2::RIGHT_CENTER
            },
            location.label(),
            FontId::proportional(11.0),
            if selected { TEXT } else { MUTED },
        );
    }

    if let Some(preview) = state.pattern.preview.as_ref() {
        painter.text(
            rect.left_top() + vec2(10.0, 9.0),
            Align2::LEFT_TOP,
            format!(
                "{}  RWY {}  ·  {:03.0}°M  ·  {}",
                preview.runway.airport_id,
                preview.runway.end.id,
                preview.data.heading,
                state.pattern.settings.direction.label()
            ),
            FontId::proportional(12.0),
            TEXT,
        );
        if preview.runway.end.displaced_threshold_m > 0.5 {
            painter.text(
                Pos2::new(runway_rect.right() + 7.0, threshold_y),
                Align2::LEFT_CENTER,
                format!(
                    "threshold +{:.0} m",
                    preview.runway.end.displaced_threshold_m
                ),
                FontId::proportional(10.0),
                AMBER,
            );
        }
    }
}

fn draw_aircraft(painter: &egui::Painter, center: Pos2, direction: Vec2, color: Color32) {
    let normal = vec2(-direction.y, direction.x);
    painter.line_segment(
        [center - direction * 10.0, center + direction * 11.0],
        Stroke::new(2.1, color),
    );
    painter.line_segment(
        [center - normal * 9.0, center + normal * 9.0],
        Stroke::new(2.6, color),
    );
    let tail = center - direction * 7.0;
    painter.line_segment(
        [tail - normal * 4.0, tail + normal * 4.0],
        Stroke::new(1.8, color),
    );
    let nose = center + direction * 11.0;
    painter.add(Shape::convex_polygon(
        vec![nose, center + normal * 3.0, center - normal * 3.0],
        color,
        Stroke::NONE,
    ));
}
