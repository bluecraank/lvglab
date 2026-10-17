//! The studio's look: a calm, modern dark and light theme with one accent.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme, Visuals,
};

use crate::settings::Appearance;

pub const ACCENT: Color32 = Color32::from_rgb(0x6C, 0x63, 0xFF);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(0x36, 0x32, 0x80, 0x80);
pub const WARNING: Color32 = Color32::from_rgb(0xF5, 0xA5, 0x24);
pub const DANGER: Color32 = Color32::from_rgb(0xEF, 0x53, 0x50);

/// Colors that depend on the theme but are not part of egui's visuals.
pub struct Palette {
    pub canvas: Color32,
    pub canvas_grid: Color32,
    pub muted: Color32,
}

pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.style().visuals.dark_mode {
        Palette {
            canvas: Color32::from_rgb(0x0F, 0x11, 0x15),
            canvas_grid: Color32::from_rgba_unmultiplied(255, 255, 255, 14),
            muted: Color32::from_rgb(0x8A, 0x90, 0x9C),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(0xE4, 0xE7, 0xEC),
            canvas_grid: Color32::from_rgba_unmultiplied(0, 0, 0, 18),
            muted: Color32::from_rgb(0x6B, 0x72, 0x80),
        }
    }
}

pub fn install(ctx: &egui::Context) {
    ctx.set_visuals_of(Theme::Dark, dark());
    ctx.set_visuals_of(Theme::Light, light());
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.spacing.combo_width = 160.0;
        style.spacing.window_margin = Margin::same(14);
        style.spacing.menu_margin = Margin::same(6);
        style.text_styles = [
            (TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();
    });
}

pub fn apply_appearance(ctx: &egui::Context, appearance: Appearance) {
    ctx.set_theme(match appearance {
        Appearance::System => egui::ThemePreference::System,
        Appearance::Dark => egui::ThemePreference::Dark,
        Appearance::Light => egui::ThemePreference::Light,
    });
}

fn common(mut v: Visuals) -> Visuals {
    let radius = CornerRadius::same(6);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.hyperlink_color = ACCENT;
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(70) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(50) };
    v.slider_trailing_fill = true;
    v.striped = true;
    v
}

fn dark() -> Visuals {
    let mut v = Visuals::dark();
    let bg = Color32::from_rgb(0x17, 0x19, 0x1E);
    let panel = Color32::from_rgb(0x1C, 0x1F, 0x25);
    let widget = Color32::from_rgb(0x27, 0x2B, 0x33);
    let hover = Color32::from_rgb(0x31, 0x36, 0x40);
    let border = Color32::from_rgb(0x2E, 0x33, 0x3C);
    v.panel_fill = panel;
    v.window_fill = panel;
    v.extreme_bg_color = bg;
    v.faint_bg_color = Color32::from_rgb(0x21, 0x24, 0x2B);
    v.code_bg_color = bg;
    v.window_stroke = Stroke::new(1.0_f32, border);
    v.widgets.noninteractive.bg_fill = panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xC9, 0xCE, 0xD6));
    v.widgets.inactive.bg_fill = widget;
    v.widgets.inactive.weak_bg_fill = widget;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xDD, 0xE1, 0xE8));
    v.widgets.hovered.bg_fill = hover;
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    v.widgets.open.bg_fill = hover;
    common(v)
}

fn light() -> Visuals {
    let mut v = Visuals::light();
    let panel = Color32::from_rgb(0xFA, 0xFB, 0xFC);
    let widget = Color32::from_rgb(0xEC, 0xEE, 0xF2);
    let hover = Color32::from_rgb(0xE1, 0xE4, 0xEA);
    let border = Color32::from_rgb(0xDD, 0xE1, 0xE7);
    v.panel_fill = panel;
    v.window_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = Color32::from_rgb(0xF2, 0xF4, 0xF7);
    v.window_stroke = Stroke::new(1.0_f32, border);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x2B, 0x30, 0x3A));
    v.widgets.inactive.bg_fill = widget;
    v.widgets.inactive.weak_bg_fill = widget;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x1F, 0x24, 0x2C));
    v.widgets.hovered.bg_fill = hover;
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    v.widgets.open.bg_fill = hover;
    common(v)
}
