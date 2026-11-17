use crate::theme;
use egui::{
    Align2, Color32, FontId, Rect, Response, RichText, Sense, Slider, Stroke, StrokeKind, Ui, Vec2,
    pos2, vec2,
};
use std::collections::{BTreeMap, BTreeSet};

pub use crate::page::{
    Bar, Choice, Control, Dialog, Item, Notice, Page, Panel, Readout, Row, tidy,
};

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

/// Height of one item row and of the buttons drawn inside it.
const ROW: f32 = 30.;
const BUTTON: f32 = 26.;
/// Width kept at the right of a status row for its action.
const ACTION: f32 = 96.;
const TEXT_SIZE: f32 = 14.5;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Source { id: u32, flag: u32 },
    Slider { id: u32, value: f64 },
    Tab(usize),
    Close,
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Plain,
    Primary,
    Selected,
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

fn width_of(ui: &Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(size), theme::TEXT)
        .size()
        .x
}

/// Draws one line of text inside `rect`, cut with an ellipsis when too long.
fn line(ui: &Ui, rect: Rect, align: Align2, text: &str, size: f32, color: Color32) -> f32 {
    let mut job = egui::text::LayoutJob::simple_singleline(
        text.to_owned(),
        FontId::proportional(size),
        color,
    );
    job.wrap = egui::text::TextWrapping {
        max_width: rect.width().max(0.),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.painter().layout_job(job);
    let size = galley.size();
    let x = match align.x() {
        egui::Align::Min => rect.left(),
        egui::Align::Center => rect.center().x - size.x / 2.,
        egui::Align::Max => rect.right() - size.x,
    };
    ui.painter()
        .galley(pos2(x, rect.center().y - size.y / 2.), galley, color);
    size.x
}

/// A label that ends in a parenthesis carries its state there, as in
/// "Battery (Off)": the name and the state are drawn apart.
fn split_state(label: &str) -> (&str, &str) {
    let trimmed = label.trim_end();
    if let Some(body) = trimmed.strip_suffix(')')
        && let Some(open) = body.rfind(" (")
        && !body[open + 2..].contains(['(', ')'])
        && !body[..open].trim().is_empty()
    {
        (body[..open].trim_end(), &body[open + 2..])
    } else {
        (trimmed, "")
    }
}

fn state_color(state: &str) -> Color32 {
    match state.to_ascii_lowercase().as_str() {
        "on" | "selected" | "installed" => theme::GREEN,
        "off" => theme::MUTED,
        _ => theme::BLUE,
    }
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

    /// Turns pointer and wheel input on one control into its source actions.
    fn report(&mut self, ui: &Ui, id: u32, enabled: bool, mask: u32, response: &Response) {
        let was = self.pressed.contains(&id);
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

    /// Registers the control at `rect` and reports its input. Returns the
    /// response and whether it can be used now.
    fn control(&mut self, ui: &Ui, rect: Rect, c: &Control) -> (Response, bool) {
        self.seen.insert(c.id);
        let enabled = c.enabled && ui.is_enabled();
        let response = ui.interact(
            rect,
            ui.id().with(("control", c.id)),
            if enabled {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        self.hit_regions.insert(c.id, rect);
        self.report(ui, c.id, enabled, c.mask, &response);
        (response, enabled)
    }

    fn face(ui: &Ui, rect: Rect, response: &Response, enabled: bool, look: Look) -> Color32 {
        let widgets = &ui.visuals().widgets;
        let (fill, text) = if !enabled {
            (theme::DISABLED, theme::MUTED.gamma_multiply(0.6))
        } else if response.is_pointer_button_down_on() {
            (widgets.active.bg_fill, theme::TEXT)
        } else {
            let (rest, hover) = match look {
                Look::Plain => (widgets.inactive.bg_fill, widgets.hovered.bg_fill),
                Look::Primary | Look::Selected => (theme::ACCENT, theme::ACCENT_HOVER),
            };
            (if response.hovered() { hover } else { rest }, theme::TEXT)
        };
        ui.painter().rect_filled(rect, 4., fill);
        if look == Look::Selected && enabled {
            ui.painter()
                .rect_stroke(rect, 4., Stroke::new(1., theme::BLUE), StrokeKind::Inside);
        }
        text
    }

    /// A button with its label centred.
    fn button(&mut self, ui: &Ui, rect: Rect, c: &Control, look: Look) -> Response {
        let (response, enabled) = self.control(ui, rect, c);
        let text = Self::face(ui, rect, &response, enabled, look);
        line(
            ui,
            rect.shrink2(vec2(6., 0.)),
            Align2::CENTER_CENTER,
            &c.label,
            14.,
            text,
        );
        response
    }

    fn row(&mut self, ui: &mut Ui, row: &Row) {
        if let Some((mut value, low, high)) = row.value {
            ui.push_id(row.id, |ui| {
                ui.horizontal(|ui| {
                    ui.label(split_state(&row.label).0);
                    let r = ui.add_enabled(
                        row.enabled,
                        Slider::new(&mut value, low..=high).integer().suffix(" %"),
                    );
                    self.hit_regions.insert(row.id, r.rect);
                    if r.changed() {
                        self.actions.push(Action::Slider { id: row.id, value });
                    }
                });
            });
        } else {
            let (name, state) = split_state(&row.label);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.), Sense::hover());
            if let Some(plus) = row.plus {
                let mut right = rect.right();
                for (id, label, enabled, mask) in [
                    (plus, "+", row.plus_enabled, row.plus_mask),
                    (row.id, "−", row.enabled, row.mask),
                ] {
                    let at =
                        Rect::from_min_size(pos2(right - 36., rect.top() + 2.), vec2(36., 28.));
                    self.button(
                        ui,
                        at,
                        &Control {
                            id,
                            label: label.into(),
                            enabled,
                            mask,
                        },
                        Look::Plain,
                    );
                    right -= 42.;
                }
                let text = Rect::from_min_max(rect.min, pos2(right - 4., rect.bottom()));
                let used = line(
                    ui,
                    text,
                    Align2::RIGHT_CENTER,
                    state,
                    TEXT_SIZE,
                    theme::BLUE,
                );
                line(
                    ui,
                    text.with_max_x(text.right() - used - 10.),
                    Align2::LEFT_CENTER,
                    name,
                    TEXT_SIZE,
                    theme::tone(row.tone),
                );
            } else {
                let control = Control {
                    id: row.id,
                    label: row.label.clone(),
                    enabled: row.enabled,
                    mask: row.mask,
                };
                let (response, enabled) = self.control(ui, rect, &control);
                let text = Self::face(ui, rect, &response, enabled, Look::Plain);
                let inner = rect.shrink2(vec2(12., 0.));
                let color = if enabled { state_color(state) } else { text };
                let used = line(ui, inner, Align2::RIGHT_CENTER, state, 14., color);
                line(
                    ui,
                    inner.with_max_x(inner.right() - used - 10.),
                    Align2::LEFT_CENTER,
                    name,
                    14.,
                    text,
                );
            }
        }
        if !row.reason.is_empty() {
            ui.label(RichText::new(&row.reason).small().color(theme::MUTED));
        }
    }

    fn rows(&mut self, ui: &mut Ui, page: &Page, column: Option<usize>) {
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
        if shown == 0 && (column.is_none() || column == Some(0)) {
            ui.label(
                RichText::new(if self.filter.is_empty() {
                    "Nothing to do here right now."
                } else {
                    "No matching controls."
                })
                .color(theme::MUTED),
            );
        }
    }

    /// The plain layout: actions beside current information, or actions in
    /// two columns when there is nothing to read.
    fn list(&mut self, ui: &mut Ui, page: &Page) {
        if page.rows.len() > 12 {
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
        } else {
            self.filter.clear();
        }
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
                let ui = &mut columns[1];
                ui.label(
                    RichText::new(if page.tab == Some(1) {
                        "Load summary"
                    } else {
                        "Current information"
                    })
                    .strong()
                    .color(theme::BLUE),
                );
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt((page.tab, "readouts"))
                    .max_height(height - 30.)
                    .show(ui, |ui| {
                        for r in &page.readouts {
                            for text in tidy(&r.text).split('\n') {
                                if let Some((label, value)) = text.split_once(": ") {
                                    Self::field(ui, label, value, r.tone);
                                } else if text.is_empty() {
                                    ui.add_space(6.);
                                } else {
                                    ui.label(RichText::new(text).color(theme::tone(r.tone)));
                                }
                            }
                        }
                        if !page.report.is_empty() {
                            ui.add_space(8.);
                            ui.label(tidy(&page.report));
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
    }

    fn field(ui: &mut Ui, label: &str, value: &str, tone: u32) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.), Sense::hover());
        let used = line(
            ui,
            rect,
            Align2::RIGHT_CENTER,
            value,
            TEXT_SIZE,
            theme::tone(tone),
        );
        line(
            ui,
            rect.with_max_x(rect.right() - used - 10.),
            Align2::LEFT_CENTER,
            label,
            TEXT_SIZE,
            theme::MUTED,
        );
    }

    fn status(&mut self, ui: &mut Ui, item: &Item, reserve: f32) {
        let Item::Status {
            label,
            state,
            tone,
            note,
            action,
        } = item
        else {
            return;
        };
        let tone = *tone;
        let height = if note.is_empty() { ROW } else { ROW + 15. };
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let middle = rect.top() + ROW / 2.;
        let mut left = rect.left() + 2.;
        if tone != 0 {
            ui.painter()
                .circle_filled(pos2(left + 5., middle), 4.5, theme::tone(tone));
            left += 18.;
        }
        let mut right = rect.right();
        if let Some(action) = action {
            let at = Rect::from_min_size(
                pos2(right - ACTION, middle - BUTTON / 2.),
                vec2(ACTION, BUTTON),
            );
            self.button(
                ui,
                at,
                action,
                if matches!(tone, 1 | 2) {
                    Look::Primary
                } else {
                    Look::Plain
                },
            );
        }
        if reserve > 0. {
            right -= reserve + 10.;
        }
        let text = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.top() + ROW));
        let used = line(
            ui,
            text,
            Align2::RIGHT_CENTER,
            state,
            TEXT_SIZE,
            if tone == 0 {
                theme::BLUE
            } else {
                theme::tone(tone)
            },
        );
        line(
            ui,
            text.with_max_x(text.right() - used - 10.),
            Align2::LEFT_CENTER,
            label,
            TEXT_SIZE,
            theme::TEXT,
        );
        if !note.is_empty() {
            line(
                ui,
                Rect::from_min_max(
                    pos2(left, rect.top() + ROW - 6.),
                    pos2(right, rect.bottom()),
                ),
                Align2::LEFT_CENTER,
                note,
                12.,
                theme::MUTED,
            );
        }
    }

    fn toggle(&mut self, ui: &mut Ui, label: &str, on: bool, state: &str, control: &Control) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
        let (response, enabled) = self.control(ui, rect, control);
        if enabled && response.hovered() {
            ui.painter()
                .rect_filled(rect.expand2(vec2(4., 0.)), 4., theme::HOVER_ROW);
        }
        let track = Rect::from_min_size(
            pos2(rect.right() - 40., rect.center().y - 10.),
            vec2(38., 20.),
        );
        let fade = if enabled { 1. } else { 0.45 };
        ui.painter().rect_filled(
            track,
            10.,
            if on {
                theme::SWITCH_ON
            } else {
                theme::SWITCH_OFF
            }
            .gamma_multiply(fade),
        );
        ui.painter().circle_filled(
            pos2(
                if on {
                    track.right() - 10.
                } else {
                    track.left() + 10.
                },
                track.center().y,
            ),
            7.,
            theme::TEXT.gamma_multiply(fade),
        );
        let text = rect
            .with_max_x(track.left() - 10.)
            .with_min_x(rect.left() + 2.);
        let used = line(
            ui,
            text,
            Align2::RIGHT_CENTER,
            state,
            TEXT_SIZE,
            if on { theme::GREEN } else { theme::MUTED }.gamma_multiply(fade),
        );
        line(
            ui,
            text.with_max_x(text.right() - used - 10.),
            Align2::LEFT_CENTER,
            label,
            TEXT_SIZE,
            if enabled {
                theme::TEXT
            } else {
                theme::MUTED.gamma_multiply(0.6)
            },
        );
    }

    fn choice(&mut self, ui: &mut Ui, label: &str, options: &[Choice]) {
        if label.is_empty() {
            let all: Vec<_> = options
                .iter()
                .map(|o| {
                    (
                        &o.control,
                        if o.selected {
                            Look::Selected
                        } else {
                            Look::Plain
                        },
                    )
                })
                .collect();
            self.buttons(ui, &all);
            return;
        }
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
        let widths: Vec<f32> = options
            .iter()
            .map(|o| (width_of(ui, &o.control.label, 14.) + 24.).max(84.))
            .collect();
        let total = widths.iter().sum::<f32>() + 4. * options.len().saturating_sub(1) as f32;
        let mut x = if label.is_empty() {
            rect.left()
        } else {
            (rect.right() - total).max(rect.left())
        };
        line(
            ui,
            rect.with_max_x(x - 10.).with_min_x(rect.left() + 2.),
            Align2::LEFT_CENTER,
            label,
            TEXT_SIZE,
            theme::TEXT,
        );
        for (option, width) in options.iter().zip(widths) {
            let at =
                Rect::from_min_size(pos2(x, rect.center().y - BUTTON / 2.), vec2(width, BUTTON));
            self.button(
                ui,
                at,
                &option.control,
                if option.selected {
                    Look::Selected
                } else {
                    Look::Plain
                },
            );
            x += width + 4.;
        }
    }

    fn stepper(&mut self, ui: &mut Ui, label: &str, value: &str, minus: &Control, plus: &Control) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
        let mut right = rect.right();
        for control in [plus, minus] {
            let at = Rect::from_min_size(
                pos2(right - 34., rect.center().y - BUTTON / 2.),
                vec2(34., BUTTON),
            );
            self.button(ui, at, control, Look::Plain);
            right -= 38.;
        }
        let text = rect.with_max_x(right - 6.).with_min_x(rect.left() + 2.);
        let used = line(
            ui,
            text,
            Align2::RIGHT_CENTER,
            value,
            TEXT_SIZE,
            theme::BLUE,
        );
        line(
            ui,
            text.with_max_x(text.right() - used - 10.),
            Align2::LEFT_CENTER,
            label,
            TEXT_SIZE,
            theme::TEXT,
        );
    }

    /// Buttons side by side, wrapped onto further lines when they do not fit.
    fn buttons(&mut self, ui: &mut Ui, controls: &[(&Control, Look)]) {
        let available = ui.available_width();
        let mut places = vec![];
        let (mut x, mut y) = (0., 0.);
        for (control, _) in controls {
            let width = (width_of(ui, &control.label, 14.) + 28.).clamp(110., available.max(110.));
            if x > 0. && x + width > available {
                x = 0.;
                y += ROW + 6.;
            }
            places.push((x, y, width));
            x += width + 8.;
        }
        let (rect, _) = ui.allocate_exact_size(vec2(available, y + ROW), Sense::hover());
        for ((control, look), (x, y, width)) in controls.iter().zip(places) {
            let at = Rect::from_min_size(rect.min + vec2(x, y), vec2(width, ROW));
            self.button(ui, at, control, *look);
        }
    }

    fn bars(&mut self, ui: &mut Ui, bars: &[Bar]) {
        if bars.is_empty() {
            return;
        }
        let acting = bars.iter().any(|b| b.action.is_some());
        let chart = 92.;
        let height = 20. + chart + 20. + if acting { BUTTON + 6. } else { 0. };
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let gap = 8.;
        let width = ((rect.width() - gap * (bars.len() - 1) as f32) / bars.len() as f32).min(88.);
        for (i, bar) in bars.iter().enumerate() {
            let left = rect.left() + i as f32 * (width + gap);
            let column = Rect::from_min_size(pos2(left, rect.top()), vec2(width, height));
            let color = theme::tone(bar.tone);
            line(
                ui,
                Rect::from_min_size(column.min, vec2(width, 18.)),
                Align2::CENTER_CENTER,
                &bar.text,
                13.,
                color,
            );
            let track = Rect::from_min_size(
                pos2(column.center().x - 13., column.top() + 20.),
                vec2(26., chart),
            );
            ui.painter().rect_filled(track, 3., theme::SWITCH_OFF);
            let level = bar.level.clamp(0., 1.);
            if level > 0. {
                ui.painter().rect_filled(
                    track.with_min_y(track.bottom() - chart * level),
                    3.,
                    color,
                );
            }
            line(
                ui,
                Rect::from_min_size(pos2(left, track.bottom() + 2.), vec2(width, 18.)),
                Align2::CENTER_CENTER,
                &bar.label,
                13.,
                theme::MUTED,
            );
            if let Some(action) = &bar.action {
                let at = Rect::from_min_size(pos2(left, track.bottom() + 24.), vec2(width, BUTTON));
                self.button(
                    ui,
                    at,
                    action,
                    if matches!(bar.tone, 1 | 2) {
                        Look::Primary
                    } else {
                        Look::Plain
                    },
                );
            }
        }
    }

    fn item(&mut self, ui: &mut Ui, item: &Item, reserve: f32) {
        match item {
            Item::Status { .. } => self.status(ui, item, reserve),
            Item::Toggle {
                label,
                on,
                state,
                control,
            } => self.toggle(ui, label, *on, state, control),
            Item::Choice { label, options } => self.choice(ui, label, options),
            Item::Stepper {
                label,
                value,
                minus,
                plus,
            } => self.stepper(ui, label, value, minus, plus),
            Item::Actions(controls) => {
                let all: Vec<_> = controls.iter().map(|c| (c, Look::Plain)).collect();
                self.buttons(ui, &all);
            }
            Item::Bars(bars) => self.bars(ui, bars),
            Item::Field { label, value, tone } => Self::field(ui, label, value, *tone),
            Item::Text { text, tone } => {
                let text = tidy(text);
                if !text.is_empty() {
                    ui.label(RichText::new(text).color(theme::tone(*tone)));
                }
            }
        }
    }

    fn items(&mut self, ui: &mut Ui, items: &[Item]) {
        let reserve = if items.iter().any(|i| {
            matches!(
                i,
                Item::Status {
                    action: Some(_),
                    ..
                }
            )
        }) {
            ACTION
        } else {
            0.
        };
        ui.spacing_mut().item_spacing.y = 3.;
        for item in items {
            self.item(ui, item, reserve);
        }
    }

    fn card(ui: &mut Ui, stroke: Stroke, add: impl FnOnce(&mut Ui)) {
        egui::Frame::new()
            .fill(theme::PANEL)
            .stroke(stroke)
            .corner_radius(6.)
            .inner_margin(12.)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                add(ui);
            });
    }

    fn banner(&mut self, ui: &mut Ui, notice: &Notice) {
        let color = if notice.tone == 0 {
            theme::BLUE
        } else {
            theme::tone(notice.tone)
        };
        Self::card(ui, Stroke::new(1., color), |ui| {
            let width = ui.available_width();
            let button = notice
                .action
                .as_ref()
                .map(|a| (width_of(ui, &a.label, 14.) + 28.).max(110.));
            let text = width - button.map_or(0., |w| w + 14.);
            let galley = ui.painter().layout(
                tidy(&notice.text),
                FontId::proportional(TEXT_SIZE),
                theme::TEXT,
                text,
            );
            let (rect, _) =
                ui.allocate_exact_size(vec2(width, galley.size().y.max(ROW)), Sense::hover());
            ui.painter().galley(
                pos2(rect.left(), rect.center().y - galley.size().y / 2.),
                galley,
                theme::TEXT,
            );
            if let (Some(action), Some(button)) = (&notice.action, button) {
                let at = Rect::from_min_size(
                    pos2(rect.right() - button, rect.center().y - ROW / 2.),
                    vec2(button, ROW),
                );
                self.button(ui, at, action, Look::Primary);
            }
        });
        ui.add_space(8.);
    }

    fn panel(&mut self, ui: &mut Ui, panel: &Panel) {
        Self::card(ui, Stroke::NONE, |ui| {
            if !panel.title.is_empty() {
                ui.label(RichText::new(&panel.title).strong().color(theme::BLUE));
                ui.add_space(4.);
            }
            self.items(ui, &panel.items);
        });
        ui.add_space(8.);
    }

    /// The structured layout: an optional banner, an optional dialog, then
    /// the panels in one or two scrolling columns.
    fn panels(&mut self, ui: &mut Ui, page: &Page) {
        if let Some(notice) = &page.notice {
            self.banner(ui, notice);
        }
        if let Some(dialog) = &page.dialog {
            ui.scope(|ui| {
                ui.set_max_width(ui.available_width().min(620.));
                Self::card(ui, Stroke::new(1.5, theme::BLUE), |ui| {
                    ui.label(RichText::new(&dialog.title).size(17.).strong());
                    ui.add_space(6.);
                    self.items(ui, &dialog.items);
                    ui.add_space(8.);
                    self.buttons(
                        ui,
                        &[
                            (&dialog.confirm, Look::Primary),
                            (&dialog.cancel, Look::Plain),
                        ],
                    );
                });
            });
            ui.add_space(8.);
        }
        let height = ui.available_height().max(120.);
        let two = page.panels.iter().any(|p| p.column > 0);
        ui.add_enabled_ui(page.dialog.is_none(), |ui| {
            if two {
                ui.columns(2, |columns| {
                    for (c, ui) in columns.iter_mut().enumerate() {
                        egui::ScrollArea::vertical()
                            .id_salt((page.tab, "panels", c))
                            .auto_shrink([false, true])
                            .max_height(height)
                            .show(ui, |ui| {
                                for panel in page.panels.iter().filter(|p| p.column.min(1) == c) {
                                    self.panel(ui, panel);
                                }
                            });
                    }
                });
            } else {
                egui::ScrollArea::vertical()
                    .id_salt((page.tab, "panels"))
                    .auto_shrink([false, true])
                    .max_height(height)
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width().min(620.));
                        for panel in &page.panels {
                            self.panel(ui, panel);
                        }
                    });
            }
        });
    }

    fn tabs(&mut self, ui: &mut Ui, tab: usize) {
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

    pub fn show(&mut self, ui: &mut Ui, page: &Page) {
        self.actions.clear();
        self.hit_regions.clear();
        self.seen.clear();
        ui.painter().rect_filled(ui.max_rect(), 0., theme::CANVAS);
        ui.set_opacity(page.opacity.clamp(0.2, 1.));
        egui::Frame::new().inner_margin(16.).show(ui, |ui| {
            if let Some(tab) = page.tab {
                self.tabs(ui, tab);
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
            ui.add_space(4.);
            if page.structured() {
                self.panels(ui, page);
            } else {
                self.list(ui, page);
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
            ..Page::default()
        }
    }
    fn control(id: u32, label: &str) -> Control {
        Control {
            id,
            label: label.into(),
            enabled: true,
            mask: PRESS | RELEASE,
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
    fn press(pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        }
    }
    /// Presses and releases the primary button on control `id`, returning the
    /// actions of both frames.
    fn click(ctx: &egui::Context, state: &mut ViewState, page: &Page, id: u32) -> Vec<Action> {
        frame(ctx, state, page, vec![]);
        let pos = state.hit_regions[&id].center();
        frame(
            ctx,
            state,
            page,
            vec![
                egui::Event::PointerMoved(pos),
                press(pos, egui::PointerButton::Primary, true),
            ],
        );
        let mut actions = state.actions.clone();
        frame(
            ctx,
            state,
            page,
            vec![press(pos, egui::PointerButton::Primary, false)],
        );
        actions.extend(state.actions.clone());
        actions
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
                press(position, egui::PointerButton::Primary, true),
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
        assert!(click(&ctx, &mut state, &p, 17).is_empty());
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
                press(pos, egui::PointerButton::Secondary, true),
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
            vec![press(pos, egui::PointerButton::Secondary, false)],
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
                press(pos, egui::PointerButton::Primary, true),
            ],
        );
        assert!(
            matches!(state.actions.as_slice(),[Action::Slider{id:17,value}] if *value>0.&&*value<=100.)
        );
    }
    #[test]
    fn paired_rows_report_the_minus_and_plus_actions_apart() {
        let ctx = egui::Context::default();
        let mut state = ViewState::default();
        let mut p = page();
        p.rows[0].plus = Some(18);
        p.rows[0].plus_mask = PRESS | RELEASE;
        p.rows[0].label = "Left tank (38 US gal)".into();
        for id in [17, 18] {
            assert_eq!(
                click(&ctx, &mut state, &p, id),
                vec![
                    Action::Source { id, flag: PRESS },
                    Action::Source { id, flag: RELEASE }
                ]
            );
        }
    }
    #[test]
    fn a_state_in_parentheses_is_split_from_the_name() {
        assert_eq!(split_state("Battery (Off)"), ("Battery", "Off"));
        assert_eq!(
            split_state("Avionics (GNS 400) (set by the G1000)"),
            ("Avionics (GNS 400)", "set by the G1000")
        );
        assert_eq!(split_state("Left door"), ("Left door", ""));
        assert_eq!(split_state("(Off)"), ("(Off)", ""));
    }
    fn structured() -> Page {
        Page {
            title: "Maintenance".into(),
            tab: Some(2),
            panels: vec![
                Panel {
                    title: "Condition".into(),
                    column: 0,
                    items: vec![
                        Item::Status {
                            label: "Left brake".into(),
                            state: "Worn".into(),
                            tone: 2,
                            note: String::new(),
                            action: Some(control(6, "Repair")),
                        },
                        Item::Status {
                            label: "Right brake".into(),
                            state: "Good".into(),
                            tone: 3,
                            note: String::new(),
                            action: None,
                        },
                    ],
                },
                Panel {
                    title: "Equipment".into(),
                    column: 1,
                    items: vec![
                        Item::Toggle {
                            label: "Flap gap seals".into(),
                            on: true,
                            state: "Installed".into(),
                            control: control(20, "Flap gap seals"),
                        },
                        Item::Choice {
                            label: "Battery".into(),
                            options: vec![
                                Choice {
                                    control: control(28, "RG24-11M"),
                                    selected: true,
                                },
                                Choice {
                                    control: control(29, "RG24-15M"),
                                    selected: false,
                                },
                            ],
                        },
                        Item::Stepper {
                            label: "Altitude".into(),
                            value: "5000 ft".into(),
                            minus: control(40, "−"),
                            plus: control(41, "+"),
                        },
                        Item::Bars(vec![Bar {
                            label: "1".into(),
                            level: 0.9,
                            text: "90 %".into(),
                            tone: 3,
                            action: Some(control(50, "Replace")),
                        }]),
                        Item::Actions(vec![control(60, "Complete overhaul")]),
                    ],
                },
            ],
            ..Page::default()
        }
    }
    #[test]
    fn every_structured_control_reports_its_own_action() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = ViewState::default();
        let p = structured();
        for id in [6, 20, 28, 29, 40, 41, 50, 60] {
            assert_eq!(
                click(&ctx, &mut state, &p, id),
                vec![
                    Action::Source { id, flag: PRESS },
                    Action::Source { id, flag: RELEASE }
                ],
                "control {id}"
            );
        }
    }
    #[test]
    fn a_dialog_and_a_banner_answer_while_the_panels_beneath_do_not() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = ViewState::default();
        let mut p = structured();
        p.notice = Some(Notice {
            text: "On the ground with the engine stopped.".into(),
            tone: 2,
            action: Some(control(13, "Bypass")),
        });
        p.dialog = Some(Dialog {
            title: "Select oil".into(),
            items: vec![Item::Choice {
                label: String::new(),
                options: vec![Choice {
                    control: control(26, "100AW"),
                    selected: true,
                }],
            }],
            confirm: control(33, "OK"),
            cancel: control(34, "Cancel"),
        });
        for id in [13, 26, 33, 34] {
            assert_eq!(click(&ctx, &mut state, &p, id).len(), 2, "control {id}");
        }
        for id in [6, 20, 28, 60] {
            assert!(click(&ctx, &mut state, &p, id).is_empty(), "control {id}");
        }
    }
    #[test]
    fn a_held_structured_control_releases_when_its_panel_goes() {
        let ctx = egui::Context::default();
        let mut state = ViewState::default();
        let mut p = structured();
        frame(&ctx, &mut state, &p, vec![]);
        let pos = state.hit_regions[&60].center();
        frame(
            &ctx,
            &mut state,
            &p,
            vec![
                egui::Event::PointerMoved(pos),
                press(pos, egui::PointerButton::Primary, true),
            ],
        );
        assert_eq!(
            state.actions,
            vec![Action::Source {
                id: 60,
                flag: PRESS
            }]
        );
        p.panels.pop();
        frame(&ctx, &mut state, &p, vec![]);
        assert_eq!(
            state.actions,
            vec![Action::Source {
                id: 60,
                flag: RELEASE
            }]
        );
    }
}
