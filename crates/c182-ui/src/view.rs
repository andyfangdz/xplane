use crate::theme;
use egui::{Button, Rect, RichText, Sense, Slider, Vec2};
use std::collections::{BTreeMap, BTreeSet};
pub const PRESS: u32 = 0x20000000;
pub const RELEASE: u32 = 0x00020000;
pub const RIGHT: u32 = 0x80000000;
pub const RIGHT_RELEASE: u32 = 0x00080000;
pub const RIGHT_DRAG: u32 = 0x02000000;
pub const WHEEL_UP: u32 = 0x00004000;
pub const WHEEL_DOWN: u32 = 0x00002000;
pub const TABS: [&str; 8] = [
    "Controls",
    "Payload & fuel",
    "Maintenance",
    "Preflight",
    "Engine",
    "Radios",
    "Pause rules",
    "Notes",
];

#[derive(Clone, Debug)]
pub struct Row {
    pub id: u32,
    pub plus: Option<u32>,
    pub label: String,
    pub section: String,
    pub reason: String,
    pub column: usize,
    pub enabled: bool,
    pub plus_enabled: bool,
    pub visible: bool,
    pub mask: u32,
    pub plus_mask: u32,
    pub tone: u32,
    pub value: Option<(f64, f64, f64)>,
}
#[derive(Clone, Debug)]
pub struct Readout {
    pub text: String,
    pub tone: u32,
}
#[derive(Clone, Debug)]
pub struct Page {
    pub title: String,
    pub tab: Option<usize>,
    pub section: u32,
    pub ready: bool,
    pub opacity: f32,
    pub rows: Vec<Row>,
    pub readouts: Vec<Readout>,
    pub report: String,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Source { id: u32, flag: u32 },
    Slider { id: u32, value: f64 },
    Tab(usize),
    Section(u32),
    Close,
}
#[derive(Clone, Default)]
pub struct ViewState {
    pub filter: String,
    pub actions: Vec<Action>,
    pub hit_regions: BTreeMap<u32, Rect>,
    pressed: BTreeSet<u32>,
    secondary: BTreeSet<u32>,
    seen: BTreeSet<u32>,
    pub wheel_clicks: i32,
}
impl ViewState {
    pub fn release_all(&mut self) {
        for id in std::mem::take(&mut self.pressed) {
            self.actions.push(Action::Source { id, flag: RELEASE });
        }
        for id in std::mem::take(&mut self.secondary) {
            self.actions.push(Action::Source {
                id,
                flag: RIGHT_RELEASE,
            });
        }
    }
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }
    fn button(
        &mut self,
        ui: &mut egui::Ui,
        id: u32,
        label: &str,
        enabled: bool,
        mask: u32,
        width: f32,
    ) {
        self.seen.insert(id);
        let was = self.pressed.contains(&id);
        let response = ui.add_enabled(
            enabled,
            Button::new(label)
                .min_size(Vec2::new(width, 32.))
                .sense(Sense::click_and_drag()),
        );
        self.hit_regions.insert(id, response.rect);
        let down = enabled
            && response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        if down && !was {
            self.pressed.insert(id);
            self.actions.push(Action::Source { id, flag: PRESS });
        }
        if !down && was {
            self.pressed.remove(&id);
            self.actions.push(Action::Source { id, flag: RELEASE });
        }
        if enabled && response.clicked() && !down && !was {
            self.actions.push(Action::Source { id, flag: PRESS });
            self.actions.push(Action::Source { id, flag: RELEASE });
        }
        let secondary = enabled
            && mask & RIGHT != 0
            && response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
        let was_secondary = self.secondary.contains(&id);
        if secondary && !was_secondary {
            self.secondary.insert(id);
            self.actions.push(Action::Source { id, flag: RIGHT });
        }
        if !secondary && was_secondary {
            self.secondary.remove(&id);
            self.actions.push(Action::Source {
                id,
                flag: RIGHT_RELEASE,
            });
        }
        if enabled
            && response.secondary_clicked()
            && !secondary
            && !was_secondary
            && mask & RIGHT != 0
        {
            self.actions.push(Action::Source { id, flag: RIGHT });
            self.actions.push(Action::Source {
                id,
                flag: RIGHT_RELEASE,
            });
        }
        if secondary
            && response.dragged_by(egui::PointerButton::Secondary)
            && mask & RIGHT_DRAG != 0
        {
            self.actions.push(Action::Source {
                id,
                flag: RIGHT_DRAG,
            });
        }
        if enabled
            && response.hovered()
            && self.wheel_clicks != 0
            && mask & (WHEEL_UP | WHEEL_DOWN) != 0
        {
            let flag = if self.wheel_clicks > 0 {
                WHEEL_UP
            } else {
                WHEEL_DOWN
            };
            if mask & flag != 0 {
                for _ in 0..self.wheel_clicks.unsigned_abs().min(32) {
                    self.actions.push(Action::Source { id, flag });
                }
            }
            self.wheel_clicks = 0;
            ui.input_mut(|i| {
                i.smooth_scroll_delta = Vec2::ZERO;
            });
        }
    }
    fn row(&mut self, ui: &mut egui::Ui, row: &Row) {
        ui.push_id(row.id, |ui| {
            if let Some((mut value, low, high)) = row.value {
                ui.horizontal(|ui| {
                    ui.label(&row.label);
                    let r = ui.add_enabled(
                        row.enabled,
                        Slider::new(&mut value, low..=high).integer().suffix(" %"),
                    );
                    self.hit_regions.insert(row.id, r.rect);
                    if r.changed() {
                        self.actions.push(Action::Slider { id: row.id, value });
                    }
                });
            } else if let Some(plus) = row.plus {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [ui.available_width() - 96., 32.],
                        egui::Label::new(RichText::new(&row.label).color(theme::tone(row.tone)))
                            .wrap(),
                    );
                    self.button(ui, row.id, "−", row.enabled, row.mask, 36.);
                    self.button(ui, plus, "+", row.plus_enabled, row.plus_mask, 36.);
                });
            } else {
                let width = ui.available_width();
                self.button(ui, row.id, &row.label, row.enabled, row.mask, width);
            }
            if !row.reason.is_empty() {
                ui.label(RichText::new(&row.reason).small().color(theme::MUTED));
            }
        });
    }
    fn rows(&mut self, ui: &mut egui::Ui, page: &Page, column: Option<usize>) {
        let needle = self.filter.to_lowercase();
        let mut section = String::new();
        let mut shown = 0;
        for row in &page.rows {
            if !row.visible
                || column.is_some_and(|c| row.column != c)
                || (!needle.is_empty()
                    && !format!("{} {}", row.label, row.section)
                        .to_lowercase()
                        .contains(&needle))
            {
                continue;
            }
            if row.section != section {
                section = row.section.clone();
                if !section.is_empty() {
                    ui.add_space(10.);
                    ui.label(RichText::new(&section).strong().color(theme::BLUE));
                    ui.separator();
                }
            }
            self.row(ui, row);
            shown += 1;
        }
        if shown == 0 {
            ui.label(
                RichText::new(if self.filter.is_empty() {
                    "No actions are available in this section."
                } else {
                    "No matching controls."
                })
                .color(theme::MUTED),
            );
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, page: &Page) {
        self.actions.clear();
        self.hit_regions.clear();
        self.seen.clear();
        ui.painter().rect_filled(ui.max_rect(), 0., theme::CANVAS);
        ui.set_opacity(page.opacity.clamp(0.2, 1.));
        egui::Frame::new().inner_margin(16.).show(ui, |ui| {
            if let Some(tab) = page.tab {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("C182").size(22.).strong());
                    ui.label(RichText::new("Aircraft & systems").color(theme::MUTED));
                });
                ui.add_space(7.);
                ui.horizontal_wrapped(|ui| {
                    for (index, label) in TABS.iter().enumerate() {
                        if ui.selectable_label(index == tab, *label).clicked() {
                            self.actions.push(Action::Tab(index));
                        }
                    }
                });
                ui.separator();
            }
            ui.horizontal(|ui| {
                ui.heading(&page.title);
                if !page.ready {
                    ui.label(RichText::new("Aircraft not ready").color(theme::AMBER));
                }
            });
            if page.tab.is_none() {
                egui::ScrollArea::vertical()
                    .id_salt("notice")
                    .max_height(ui.available_height() - 48.)
                    .show(ui, |ui| {
                        ui.label(&page.report);
                    });
                if ui.button("Dismiss").clicked() {
                    self.actions.push(Action::Close);
                }
                return;
            }
            if page.tab == Some(2) {
                ui.horizontal(|ui| {
                    for (index, label) in ["Condition", "Equipment"].into_iter().enumerate() {
                        if ui
                            .selectable_label(page.section == index as u32, label)
                            .clicked()
                        {
                            self.actions.push(Action::Section(index as u32));
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text("Find a control")
                        .desired_width(260.),
                );
                if !self.filter.is_empty() && ui.button("Clear").clicked() {
                    self.filter.clear();
                }
            });
            ui.add_space(7.);
            let with_readouts = !page.readouts.is_empty() || !page.report.is_empty();
            let height = ui.available_height().max(120.);
            if with_readouts {
                ui.columns(2, |columns| {
                    egui::ScrollArea::vertical()
                        .id_salt((page.tab, "actions"))
                        .max_height(height)
                        .show(&mut columns[0], |ui| {
                            self.rows(ui, page, None);
                        });
                    columns[1].label(
                        RichText::new(if page.tab == Some(1) {
                            "Load summary"
                        } else if page.tab == Some(2) {
                            "Inspection report"
                        } else {
                            "Current information"
                        })
                        .strong()
                        .color(theme::BLUE),
                    );
                    columns[1].separator();
                    egui::ScrollArea::vertical()
                        .id_salt((page.tab, "readouts"))
                        .max_height(height - 30.)
                        .show(&mut columns[1], |ui| {
                            for r in &page.readouts {
                                if !r.text.trim().is_empty() {
                                    ui.label(RichText::new(&r.text).color(theme::tone(r.tone)));
                                }
                            }
                            if !page.report.is_empty() {
                                ui.add_space(8.);
                                ui.label(&page.report);
                            }
                        });
                });
            } else {
                ui.columns(2, |columns| {
                    for (c, ui) in columns.iter_mut().enumerate() {
                        egui::ScrollArea::vertical()
                            .id_salt((page.tab, "controls", c))
                            .max_height(height)
                            .show(ui, |ui| {
                                self.rows(ui, page, Some(c));
                            });
                    }
                });
            }
        });
        let hidden: Vec<_> = self.pressed.difference(&self.seen).copied().collect();
        for id in hidden {
            self.pressed.remove(&id);
            self.actions.push(Action::Source { id, flag: RELEASE });
        }
        let hidden: Vec<_> = self.secondary.difference(&self.seen).copied().collect();
        for id in hidden {
            self.secondary.remove(&id);
            self.actions.push(Action::Source {
                id,
                flag: RIGHT_RELEASE,
            });
        }
        self.wheel_clicks = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page() -> Page {
        Page {
            title: "Controls".into(),
            tab: Some(0),
            section: 0,
            ready: true,
            opacity: 1.,
            rows: vec![Row {
                id: 17,
                plus: None,
                label: "Hold starter".into(),
                section: "Engine".into(),
                reason: String::new(),
                column: 0,
                enabled: true,
                plus_enabled: true,
                visible: true,
                mask: PRESS | RELEASE,
                plus_mask: 0,
                tone: 0,
                value: None,
            }],
            readouts: vec![],
            report: String::new(),
        }
    }
    fn frame(ctx: &egui::Context, state: &mut ViewState, page: &Page, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            events,
            ..Default::default()
        };
        let saved = state.clone();
        let mut final_state = saved.clone();
        let mut output = ctx.run_ui(input, |ui| {
            let mut next = saved.clone();
            next.show(ui, page);
            final_state = next;
        });
        output.textures_delta.clear();
        *state = final_state;
    }
    #[test]
    fn held_actions_release_outside_the_widget_and_when_hidden() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = ViewState::default();
        let mut p = page();
        frame(&ctx, &mut state, &p, vec![]);
        let position = state.hit_regions[&17].center();
        frame(
            &ctx,
            &mut state,
            &p,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(state.actions.contains(&Action::Source {
            id: 17,
            flag: PRESS
        }));
        p.rows[0].visible = false;
        frame(&ctx, &mut state, &p, vec![]);
        assert!(state.actions.contains(&Action::Source {
            id: 17,
            flag: RELEASE
        }));
        assert!(state.pressed.is_empty());
    }
    #[test]
    fn locked_controls_cannot_enqueue_source_actions() {
        let ctx = egui::Context::default();
        let mut state = ViewState::default();
        let mut p = page();
        p.rows[0].enabled = false;
        frame(&ctx, &mut state, &p, vec![]);
        let pos = state.hit_regions[&17].center();
        frame(
            &ctx,
            &mut state,
            &p,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(state.actions.is_empty());
    }
    #[test]
    fn secondary_click_preserves_its_own_press_and_release_flags() {
        let ctx = egui::Context::default();
        let mut state = ViewState::default();
        let mut p = page();
        p.rows[0].mask |= RIGHT;
        frame(&ctx, &mut state, &p, vec![]);
        let pos = state.hit_regions[&17].center();
        frame(
            &ctx,
            &mut state,
            &p,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        assert_eq!(
            state.actions,
            vec![Action::Source {
                id: 17,
                flag: RIGHT
            }]
        );
        frame(
            &ctx,
            &mut state,
            &p,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(
            state.actions,
            vec![Action::Source {
                id: 17,
                flag: RIGHT_RELEASE
            }]
        );
    }
    #[test]
    fn wheel_uses_the_declared_source_contract_and_caps_the_burst() {
        let ctx = egui::Context::default();
        let mut state = ViewState::default();
        let mut p = page();
        p.rows[0].mask |= WHEEL_UP;
        frame(&ctx, &mut state, &p, vec![]);
        let pos = state.hit_regions[&17].center();
        state.wheel_clicks = 100;
        frame(&ctx, &mut state, &p, vec![egui::Event::PointerMoved(pos)]);
        assert_eq!(
            state.actions,
            vec![
                Action::Source {
                    id: 17,
                    flag: WHEEL_UP
                };
                32
            ]
        );
        state.wheel_clicks = -3;
        frame(&ctx, &mut state, &p, vec![]);
        assert!(state.actions.is_empty());
    }
    #[test]
    fn slider_reports_a_value_without_running_a_source_action() {
        let ctx = egui::Context::default();
        let mut state = ViewState::default();
        let mut p = page();
        p.rows[0].value = Some((0., 0., 100.));
        frame(&ctx, &mut state, &p, vec![]);
        let rect = state.hit_regions[&17];
        let pos = egui::pos2(rect.left() + rect.width() * 0.65, rect.center().y);
        frame(
            &ctx,
            &mut state,
            &p,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(
            matches!(state.actions.as_slice(),[Action::Slider{id:17,value}] if *value>0.&&*value<=100.)
        );
    }
}
