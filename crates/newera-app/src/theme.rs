//! Visual identity: a drafting studio.
//!
//! Warm graphite around a sheet of paper, one blueprint blue to point with,
//! technical labels set in mono — the same language the site speaks, at the
//! density a tool needs. Two things set this window apart from the cold gray
//! of the CAD programs it sits next to: every neutral here is warm (more red
//! than blue, like paper and pencil), and there is a single accent instead of
//! a box of colored icons. Color means "look here", never decoration.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, RichText, Stroke, TextStyle, Visuals,
    epaint::Shadow,
};
use newera_draw::Palette;

/// The colors of one theme, named by the job they do and not by the shade
/// they are, so day and night can disagree on the shade and still agree on
/// the meaning.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tokens {
    pub(crate) dark: bool,
    /// The desk: what shows between and behind panels.
    pub(crate) deep: Color32,
    /// Panels — the working chrome.
    pub(crate) surface: Color32,
    /// What lifts off the chrome: dialogs, menus, hovered rows.
    pub(crate) raised: Color32,
    /// What is carved into it: text fields, wells, the code block.
    pub(crate) inset: Color32,
    /// Hairline between things that belong together.
    pub(crate) rule: Color32,
    /// Hairline that has to be seen: widget frames, active edges.
    pub(crate) rule_strong: Color32,
    pub(crate) ink: Color32,
    /// Text that supports the line it is on.
    pub(crate) ink_dim: Color32,
    /// Technical notes, units, hints.
    pub(crate) ink_faint: Color32,
    /// Blueprint blue: selection, dimensions, links, the one accent.
    pub(crate) accent: Color32,
    /// The accent under the pointer or the pen.
    pub(crate) accent_strong: Color32,
    /// The accent as a surface, quiet enough to read text on.
    pub(crate) accent_soft: Color32,
    pub(crate) ok: Color32,
    pub(crate) warn: Color32,
    pub(crate) danger: Color32,
}

/// Night: the studio after hours, warm charcoal and a blue that glows.
pub(crate) const NIGHT: Tokens = Tokens {
    dark: true,
    deep: Color32::from_rgb(14, 13, 11),
    surface: Color32::from_rgb(23, 22, 19),
    raised: Color32::from_rgb(33, 31, 27),
    inset: Color32::from_rgb(17, 16, 14),
    rule: Color32::from_rgb(43, 41, 36),
    rule_strong: Color32::from_rgb(61, 58, 51),
    ink: Color32::from_rgb(241, 237, 228),
    ink_dim: Color32::from_rgb(167, 162, 149),
    ink_faint: Color32::from_rgb(126, 121, 110),
    accent: Color32::from_rgb(125, 147, 255),
    accent_strong: Color32::from_rgb(159, 176, 255),
    accent_soft: Color32::from_rgb(45, 48, 68),
    ok: Color32::from_rgb(92, 196, 143),
    warn: Color32::from_rgb(232, 177, 101),
    danger: Color32::from_rgb(226, 112, 95),
};

/// Day: the same studio with the blinds open — paper, ink, blueprint.
pub(crate) const DAY: Tokens = Tokens {
    dark: false,
    deep: Color32::from_rgb(228, 223, 212),
    surface: Color32::from_rgb(244, 241, 234),
    raised: Color32::from_rgb(251, 250, 246),
    inset: Color32::from_rgb(255, 255, 255),
    rule: Color32::from_rgb(217, 211, 197),
    rule_strong: Color32::from_rgb(196, 189, 173),
    ink: Color32::from_rgb(22, 21, 19),
    ink_dim: Color32::from_rgb(86, 83, 76),
    ink_faint: Color32::from_rgb(120, 116, 106),
    accent: Color32::from_rgb(36, 70, 216),
    accent_strong: Color32::from_rgb(25, 52, 175),
    accent_soft: Color32::from_rgb(219, 222, 240),
    ok: Color32::from_rgb(31, 122, 77),
    warn: Color32::from_rgb(168, 98, 15),
    danger: Color32::from_rgb(176, 58, 43),
};

pub(crate) const fn tokens(dark: bool) -> Tokens {
    if dark { NIGHT } else { DAY }
}

/// The colors of whatever theme is on screen right now.
pub(crate) fn of(visuals: &Visuals) -> Tokens {
    tokens(visuals.dark_mode)
}

/// Widget corners: enough to look drawn, not enough to look like a toy.
const ROUND: CornerRadius = CornerRadius::same(6);

/// Installs both themes. Which one shows is up to [`set_preference`].
pub(crate) fn apply(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Dark, visuals(NIGHT));
    ctx.set_visuals_of(egui::Theme::Light, visuals(DAY));
    ctx.all_styles_mut(|style| {
        typography(style);
        spacing(style);
    });
}

/// Day, night, or whatever the system is set to.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash,
)]
pub(crate) enum Mode {
    #[default]
    System,
    Night,
    Day,
}

impl Mode {
    pub(crate) const ALL: [Self; 3] = [Self::System, Self::Night, Self::Day];

    /// The name to show, in the language of the window.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => crate::i18n::tr("Do sistema"),
            Self::Night => crate::i18n::tr("Noite"),
            Self::Day => crate::i18n::tr("Dia"),
        }
    }
}

pub(crate) fn set_mode(ctx: &egui::Context, mode: Mode) {
    ctx.set_theme(match mode {
        Mode::System => egui::ThemePreference::System,
        Mode::Night => egui::ThemePreference::Dark,
        Mode::Day => egui::ThemePreference::Light,
    });
}

/// A dialog over a dimmed window. The scrim is the warm near-black of the
/// desk rather than plain black, and by day it only veils: a paper interface
/// under 40% black reads as mud.
pub(crate) fn modal(ctx: &egui::Context, id: egui::Id) -> egui::Modal {
    let t = of(&ctx.style_of(ctx.theme()).visuals);
    egui::Modal::new(id).backdrop_color(Color32::from_rgba_unmultiplied(
        20,
        16,
        10,
        if t.dark { 130 } else { 70 },
    ))
}

/// The heading of a dialog, with the rule that separates it from the body.
/// Every dialog opens the same way, so they read as one program.
pub(crate) fn title(ui: &mut egui::Ui, text: &str) {
    let t = of(ui.visuals());
    ui.label(RichText::new(text).heading().color(t.ink));
    ui.add_space(8.0);
    hairline(ui);
    ui.add_space(10.0);
}

/// A line across the whole width of what it is in.
pub(crate) fn hairline(ui: &mut egui::Ui) {
    let t = of(ui.visuals());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, t.rule));
}

/// A heading inside a dialog: names the group of fields under it, in the
/// same mono the drawings use for their notes.
pub(crate) fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(12.0);
    ui.label(fig(ui.visuals(), text));
    ui.add_space(6.0);
}

/// The body of a dialog: as tall as it needs, never taller than the window,
/// and it scrolls when the content asks for more.
pub(crate) fn body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let room = ui.ctx().content_rect().height() * 0.62;
    egui::ScrollArea::vertical()
        .max_height(room)
        .auto_shrink([false, true])
        .show(ui, add)
        .inner
}

/// The foot of a dialog: a rule, then the actions from the right, the way
/// every dialog in the window ends.
pub(crate) fn footer<R>(ui: &mut egui::Ui, actions: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.add_space(14.0);
    hairline(ui);
    ui.add_space(10.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        actions(ui)
    })
    .inner
}

/// The button that does the thing the dialog was opened for: filled with the
/// accent, one per dialog.
pub(crate) fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let t = of(ui.visuals());
    action(ui, text, t.accent, if t.dark { t.deep } else { t.raised })
}

/// The button that undoes something for good: the same shape, in the color
/// the window keeps for damage.
pub(crate) fn destructive(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let t = of(ui.visuals());
    action(ui, text, t.danger, if t.dark { t.deep } else { t.raised })
}

/// The way out of a dialog: no fill, a hairline, quiet text.
pub(crate) fn secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let t = of(ui.visuals());
    ui.add(
        egui::Button::new(RichText::new(text).color(t.ink_dim))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, t.rule_strong))
            .corner_radius(ROUND)
            .min_size(egui::vec2(92.0, 28.0)),
    )
}

fn action(ui: &mut egui::Ui, text: &str, fill: Color32, ink: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(ink).strong())
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(ROUND)
            .min_size(egui::vec2(92.0, 28.0)),
    )
}

/// A technical label: mono, upper case, quiet — the note in the margin of a
/// drawing, not a heading.
pub(crate) fn fig(visuals: &Visuals, text: &str) -> RichText {
    RichText::new(text.to_uppercase())
        .font(FontId::new(10.0, FontFamily::Monospace))
        .color(of(visuals).ink_faint)
}

fn typography(style: &mut egui::Style) {
    use FontFamily::{Monospace, Proportional};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(15.0, Proportional)),
        (TextStyle::Body, FontId::new(13.0, Proportional)),
        (TextStyle::Button, FontId::new(13.0, Proportional)),
        (TextStyle::Small, FontId::new(10.5, Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, Monospace)),
    ]
    .into();
}

fn spacing(style: &mut egui::Style) {
    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(8.0, 6.0);
    s.button_padding = egui::vec2(9.0, 4.0);
    s.interact_size.y = 24.0;
    s.indent = 18.0;
    s.icon_width = 15.0;
    s.icon_width_inner = 8.0;
    s.menu_margin = Margin::symmetric(6, 6);
    s.window_margin = Margin::same(12);
    // Menus that breathe are taller: a drop-down of ten items has to open
    // whole instead of hiding half of itself behind a scroll bar.
    s.combo_height = 460.0;
    s.scroll.bar_width = 8.0;
    s.scroll.bar_inner_margin = 3.0;
    s.scroll.floating = true;
    s.tooltip_width = 460.0;
}

fn visuals(t: Tokens) -> Visuals {
    let mut visuals = if t.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.panel_fill = t.surface;
    visuals.window_fill = t.raised;
    visuals.window_stroke = Stroke::new(1.0, t.rule_strong);
    visuals.extreme_bg_color = t.inset;
    visuals.faint_bg_color = if t.dark {
        Color32::from_rgba_unmultiplied(255, 255, 255, 8)
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 8)
    };
    visuals.code_bg_color = t.inset;
    visuals.hyperlink_color = t.accent;
    visuals.warn_fg_color = t.warn;
    visuals.error_fg_color = t.danger;
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    // Zebra stripes are for tables of numbers, and each of those asks for
    // them: a form of properties reads better on one plain surface.
    visuals.striped = false;
    visuals.indent_has_left_vline = true;
    visuals.slider_trailing_fill = true;
    visuals.resize_corner_size = 10.0;

    // A drawing's shadow on the desk: wide, soft, warm, barely there.
    let shade = |blur: u8, alpha: u8| Shadow {
        offset: [0, 8],
        blur,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(20, 16, 10, alpha),
    };
    visuals.window_shadow = shade(28, if t.dark { 120 } else { 40 });
    visuals.popup_shadow = shade(16, if t.dark { 96 } else { 30 });

    // Selection reads as a surface with text on it, never as a paint splash.
    visuals.selection.bg_fill = t.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0, if t.dark { t.ink } else { t.accent_strong });

    let w = &mut visuals.widgets;
    // Frames, separators and everything that is only there to be read.
    w.noninteractive.bg_fill = t.surface;
    w.noninteractive.weak_bg_fill = t.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.rule);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.ink_dim);
    w.noninteractive.corner_radius = ROUND;
    w.noninteractive.expansion = 0.0;

    // At rest a button is flat: the icons carry the toolbar, not their boxes.
    w.inactive.bg_fill = t.raised;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, t.ink_dim);
    w.inactive.corner_radius = ROUND;
    w.inactive.expansion = 0.0;

    // Under the pointer it lifts off the panel instead of changing size.
    w.hovered.bg_fill = t.raised;
    w.hovered.weak_bg_fill = t.raised;
    w.hovered.bg_stroke = Stroke::new(1.0, t.rule_strong);
    w.hovered.fg_stroke = Stroke::new(1.0, t.ink);
    w.hovered.corner_radius = ROUND;
    w.hovered.expansion = 0.0;

    // Pressed: the accent, quietly.
    w.active.bg_fill = t.accent_soft;
    w.active.weak_bg_fill = t.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0, t.accent);
    w.active.fg_stroke = Stroke::new(1.0, if t.dark { t.ink } else { t.accent_strong });
    w.active.corner_radius = ROUND;
    w.active.expansion = 0.0;

    // Held open — a menu, a combo box.
    w.open.bg_fill = t.inset;
    w.open.weak_bg_fill = t.raised;
    w.open.bg_stroke = Stroke::new(1.0, t.rule_strong);
    w.open.fg_stroke = Stroke::new(1.0, t.ink);
    w.open.corner_radius = ROUND;
    w.open.expansion = 0.0;

    visuals
}

/// The plan as a sheet: warm paper by day, a drafting board lit from within
/// by night. Screen only — what goes to PDF, SVG or PNG keeps its own paper.
pub(crate) fn plan_palette(dark: bool) -> Palette {
    use newera_draw::Color;
    if dark {
        Palette {
            paper: Color::rgb(18, 17, 15),
            grid_minor: Color::rgb(30, 28, 25),
            grid_major: Color::rgb(45, 42, 37),
            wall: Color::rgb(222, 217, 206),
            wall_background: Color::rgb(26, 25, 22),
            wall_drywall: Color::rgb(140, 135, 124),
            wall_concrete: Color::rgb(245, 241, 232),
            wall_glass: Color::rgb(138, 170, 205),
            wall_wood: Color::rgb(176, 132, 86),
            room_fill: Color::rgb(33, 31, 26),
            room_line: Color::rgb(78, 73, 63),
            room_text: Color::rgb(176, 170, 157),
            dimension: Color::rgb(125, 147, 255),
            label: Color::rgb(241, 237, 228),
            compass: Color::rgb(141, 136, 124),
            selection: Color::rgb(125, 147, 255),
            furniture_fill: Color::rgb(28, 27, 24),
            furniture_detail: Color::rgb(45, 42, 37),
            furniture_line: Color::rgb(173, 168, 155),
        }
    } else {
        Palette {
            paper: Color::rgb(246, 243, 236),
            grid_minor: Color::rgb(232, 227, 216),
            grid_major: Color::rgb(213, 206, 190),
            wall: Color::rgb(46, 44, 39),
            wall_background: Color::rgb(251, 250, 246),
            wall_drywall: Color::rgb(137, 131, 118),
            wall_concrete: Color::rgb(28, 27, 24),
            wall_glass: Color::rgb(138, 170, 205),
            wall_wood: Color::rgb(156, 116, 74),
            room_fill: Color::rgb(236, 228, 212),
            room_line: Color::rgb(193, 182, 161),
            room_text: Color::rgb(109, 106, 98),
            dimension: Color::rgb(36, 70, 216),
            label: Color::rgb(22, 21, 19),
            compass: Color::rgb(109, 106, 98),
            selection: Color::rgb(36, 70, 216),
            furniture_fill: Color::rgb(251, 250, 246),
            furniture_detail: Color::rgb(226, 220, 208),
            furniture_line: Color::rgb(58, 56, 51),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text has to be readable on the surface it sits on. WCAG's contrast
    /// ratio, at the 4.5:1 a paragraph needs and 3:1 for the quiet notes.
    #[test]
    fn every_text_color_can_be_read_on_its_surface() {
        fn luminance(c: Color32) -> f32 {
            let channel = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
        }
        fn ratio(a: Color32, b: Color32) -> f32 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for t in [NIGHT, DAY] {
            for surface in [t.surface, t.raised, t.inset, t.accent_soft] {
                assert!(
                    ratio(t.ink, surface) >= 4.5,
                    "ink on {surface:?}: {:.2}",
                    ratio(t.ink, surface)
                );
                assert!(
                    ratio(t.ink_dim, surface) >= 3.0,
                    "dim ink on {surface:?}: {:.2}",
                    ratio(t.ink_dim, surface)
                );
            }
            for color in [t.accent, t.ok, t.warn, t.danger] {
                assert!(
                    ratio(color, t.surface) >= 3.0,
                    "{color:?} on the panel: {:.2}",
                    ratio(color, t.surface)
                );
            }
        }
    }

    /// The plan is drawn on its own paper, and its ink has to hold there too.
    #[test]
    fn the_plan_keeps_its_ink_off_its_paper() {
        for dark in [true, false] {
            let p = plan_palette(dark);
            let far = |a: newera_draw::Color, b: newera_draw::Color| {
                let apart: i32 = (0..3)
                    .map(|i| (i32::from(a.0[i]) - i32::from(b.0[i])).abs())
                    .sum();
                apart > 180
            };
            assert!(far(p.wall, p.paper), "walls vanish into the paper");
            assert!(far(p.label, p.paper), "texts vanish into the paper");
            assert!(
                far(p.dimension, p.paper),
                "dimensions vanish into the paper"
            );
        }
    }
}
