use egui::{Color32, CornerRadius, Stroke, TextStyle, Vec2, Visuals};
pub const CANVAS: Color32 = Color32::from_rgb(29, 42, 50);
pub const PANEL: Color32 = Color32::from_rgb(39, 55, 65);
pub const TEXT: Color32 = Color32::from_rgb(231, 238, 242);
pub const MUTED: Color32 = Color32::from_rgb(164, 182, 193);
pub const BLUE: Color32 = Color32::from_rgb(126, 184, 206);
pub const AMBER: Color32 = Color32::from_rgb(233, 181, 83);
pub const GREEN: Color32 = Color32::from_rgb(136, 193, 157);
pub fn apply(context: &egui::Context) {
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    let mut v = Visuals::dark();
    v.panel_fill = CANVAS;
    v.window_fill = PANEL;
    v.override_text_color = Some(TEXT);
    v.weak_text_color = Some(MUTED);
    v.selection.bg_fill = Color32::from_rgb(52, 103, 125);
    v.selection.stroke = Stroke::new(1., TEXT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1., Color32::from_rgb(69, 88, 99));
    v.widgets.inactive.bg_fill = Color32::from_rgb(50, 68, 80);
    v.widgets.hovered.bg_fill = Color32::from_rgb(65, 91, 105);
    v.widgets.active.bg_fill = Color32::from_rgb(47, 102, 123);
    v.widgets.open = v.widgets.active;
    v.widgets.inactive.weak_bg_fill = v.widgets.inactive.bg_fill;
    v.widgets.hovered.weak_bg_fill = v.widgets.hovered.bg_fill;
    v.widgets.active.weak_bg_fill = v.widgets.active.bg_fill;
    v.window_corner_radius = CornerRadius::same(5);
    v.warn_fg_color = AMBER;
    style.visuals = v;
    style.spacing.item_spacing = Vec2::new(8., 7.);
    style.spacing.button_padding = Vec2::new(12., 7.);
    style.spacing.interact_size = Vec2::new(42., 32.);
    for (kind, size) in [
        (TextStyle::Heading, 21.),
        (TextStyle::Body, 15.),
        (TextStyle::Button, 14.),
        (TextStyle::Small, 12.),
    ] {
        style.text_styles.get_mut(&kind).unwrap().size = size;
    }
    context.set_style_of(egui::Theme::Dark, style);
    context.set_theme(egui::Theme::Dark);
}
pub fn tone(id: u32) -> Color32 {
    match id {
        1 => Color32::from_rgb(248, 137, 123),
        2 => AMBER,
        3 => GREEN,
        4 => BLUE,
        5 => Color32::from_gray(190),
        6 => Color32::from_gray(160),
        7 => Color32::from_gray(139),
        8 => Color32::from_gray(116),
        _ => TEXT,
    }
}
