use egui::{Color32, Context, CornerRadius, Stroke, TextStyle, Theme, Vec2, Visuals};

pub(super) const CANVAS: Color32 = Color32::from_rgb(20, 27, 33);
pub(super) const PANEL: Color32 = Color32::from_rgb(31, 41, 49);
pub(super) const FIELD: Color32 = Color32::from_rgb(11, 20, 27);
pub(super) const BORDER: Color32 = Color32::from_rgb(67, 82, 92);
pub(super) const TEXT: Color32 = Color32::from_rgb(233, 239, 243);
pub(super) const MUTED: Color32 = Color32::from_rgb(151, 164, 174);
pub(super) const SKY: Color32 = Color32::from_rgb(93, 151, 177);
pub(super) const SKY_HOVER: Color32 = Color32::from_rgb(116, 174, 199);
pub(super) const AMBER: Color32 = Color32::from_rgb(222, 174, 79);
pub(super) const AMBER_HOVER: Color32 = Color32::from_rgb(240, 194, 99);
pub(super) const GOOD: Color32 = Color32::from_rgb(111, 198, 153);
pub(super) const ERROR: Color32 = Color32::from_rgb(239, 119, 103);

pub(super) fn apply(context: &Context) {
    let mut style = (*context.style_of(Theme::Dark)).clone();
    let mut visuals = Visuals::dark();

    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size = Vec2::new(42.0, 30.0);
    style.spacing.combo_width = 220.0;
    style.spacing.combo_height = 250.0;

    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(MUTED);
    visuals.panel_fill = CANVAS;
    visuals.window_fill = PANEL;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CornerRadius::same(5);
    visuals.menu_corner_radius = CornerRadius::same(5);
    visuals.faint_bg_color = Color32::from_rgb(27, 36, 43);
    visuals.extreme_bg_color = FIELD;
    visuals.text_edit_bg_color = Some(FIELD);
    visuals.selection.bg_fill = SKY;
    visuals.selection.stroke = Stroke::new(1.0, TEXT);
    visuals.hyperlink_color = SKY_HOVER;
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = ERROR;

    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(42, 54, 63);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(42, 54, 63);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(4);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(53, 68, 78);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(53, 68, 78);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, SKY_HOVER);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(4);

    visuals.widgets.active.bg_fill = Color32::from_rgb(38, 91, 114);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(38, 91, 114);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, SKY_HOVER);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, TEXT);
    visuals.widgets.active.corner_radius = CornerRadius::same(4);
    visuals.widgets.open = visuals.widgets.active;

    style.visuals = visuals;
    style.text_styles.get_mut(&TextStyle::Heading).unwrap().size = 19.0;
    style.text_styles.get_mut(&TextStyle::Body).unwrap().size = 15.0;
    style.text_styles.get_mut(&TextStyle::Button).unwrap().size = 14.0;
    style.text_styles.get_mut(&TextStyle::Small).unwrap().size = 12.0;
    context.set_style_of(Theme::Dark, style);
    context.set_theme(Theme::Dark);
}
