//! Visual identity: a calm light theme with a blue accent that matches the
//! plan's selection color.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

pub(crate) const ACCENT: Color32 = Color32::from_rgb(40, 120, 230);

pub(crate) fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::light();
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;
    visuals.panel_fill = Color32::from_rgb(246, 247, 249);
    visuals.window_fill = Color32::from_rgb(252, 252, 253);
    visuals.extreme_bg_color = Color32::WHITE;
    visuals.faint_bg_color = Color32::from_rgb(238, 240, 244);
    let radius = CornerRadius::same(6);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open.corner_radius = radius;
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    ctx.set_visuals_of(egui::Theme::Light, visuals);
    ctx.set_theme(egui::Theme::Light);
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    });
}
