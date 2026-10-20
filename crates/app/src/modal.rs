//! Modal dialogs with a consistent look: a centered card over a dimmed
//! backdrop, a tone icon, title, message and right-aligned buttons.

use eframe::egui::{self, Color32, CornerRadius, Frame, Key, Margin, RichText, Sense, Stroke, vec2};

use crate::visuals::{self, ACCENT, DANGER, WARNING};

const WIDTH: f32 = 420.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Success,
    Warning,
    Danger,
}

impl Tone {
    fn color(self) -> Color32 {
        match self {
            Self::Info => ACCENT,
            Self::Success => Color32::from_rgb(0x3D, 0xC9, 0x8A),
            Self::Warning => WARNING,
            Self::Danger => DANGER,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Info => crate::icons::INFO,
            Self::Success => crate::icons::CHECK,
            Self::Warning | Self::Danger => crate::icons::WARNING,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Filled with the accent color; triggered by Enter.
    Primary,
    /// Filled red, for destructive actions.
    Danger,
    /// Neutral; the last secondary button is triggered by Escape.
    Secondary,
}

#[derive(Clone, Copy, Debug)]
pub struct Action<'a> {
    pub label: &'a str,
    pub kind: Kind,
}

impl<'a> Action<'a> {
    pub const fn primary(label: &'a str) -> Self {
        Self { label, kind: Kind::Primary }
    }

    pub const fn danger(label: &'a str) -> Self {
        Self { label, kind: Kind::Danger }
    }

    pub const fn secondary(label: &'a str) -> Self {
        Self { label, kind: Kind::Secondary }
    }
}

/// Shows a modal card. `body` draws optional content below the message.
/// Returns the index of the clicked action, if any.
pub fn show(
    ctx: &egui::Context,
    id: &str,
    tone: Tone,
    title: &str,
    message: &str,
    actions: &[Action<'_>],
    body: impl FnOnce(&mut egui::Ui),
) -> Option<usize> {
    let dark = ctx.style().visuals.dark_mode;
    let frame = Frame::new()
        .fill(ctx.style().visuals.window_fill)
        .stroke(ctx.style().visuals.window_stroke)
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(24))
        .shadow(egui::Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(90) });
    let backdrop = Color32::from_black_alpha(if dark { 150 } else { 90 });
    let response = egui::Modal::new(egui::Id::new(id)).frame(frame).backdrop_color(backdrop).show(ctx, |ui| {
        ui.set_width(WIDTH);
        ui.horizontal_top(|ui| {
            icon(ui, tone);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(RichText::new(title).size(17.0).strong());
                if !message.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(message).color(visuals::palette(ui.ctx()).muted));
                }
            });
        });
        body(ui);
        ui.add_space(20.0);
        button_row(ui, actions)
    });
    let clicked = response.inner;
    clicked.or_else(|| keyboard_choice(ctx, actions))
}

fn icon(ui: &mut egui::Ui, tone: Tone) {
    let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
    let color = tone.color();
    ui.painter().circle_filled(rect.center(), 20.0, color.gamma_multiply(0.18));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        tone.glyph(),
        egui::FontId::proportional(20.0),
        color,
    );
}

fn button_row(ui: &mut egui::Ui, actions: &[Action<'_>]) -> Option<usize> {
    let mut clicked = None;
    // Fixed row height: a bare with_layout would take all remaining height of the modal.
    let size = vec2(ui.available_width(), 34.0);
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        // Right to left: the first action ends up rightmost.
        for (index, action) in actions.iter().enumerate() {
            if button(ui, action).clicked() {
                clicked = Some(index);
            }
        }
    });
    clicked
}

fn button(ui: &mut egui::Ui, action: &Action<'_>) -> egui::Response {
    let (fill, text, stroke) = match action.kind {
        Kind::Primary => (ACCENT, Color32::WHITE, Stroke::NONE),
        Kind::Danger => (DANGER, Color32::WHITE, Stroke::NONE),
        Kind::Secondary => {
            (Color32::TRANSPARENT, ui.visuals().text_color(), ui.visuals().widgets.noninteractive.bg_stroke)
        }
    };
    let label = RichText::new(action.label).color(text).strong();
    ui.add(egui::Button::new(label).fill(fill).stroke(stroke).corner_radius(8).min_size(vec2(96.0, 34.0)))
}

/// Enter triggers the primary action, Escape the last secondary one.
fn keyboard_choice(ctx: &egui::Context, actions: &[Action<'_>]) -> Option<usize> {
    let (enter, escape) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if enter {
        return actions.iter().position(|a| a.kind == Kind::Primary);
    }
    if escape {
        return actions.iter().rposition(|a| a.kind == Kind::Secondary);
    }
    None
}

/// A bordered, scrollable list of lines for dialog bodies.
pub fn line_list(ui: &mut egui::Ui, lines: &[String], tone: Tone) {
    ui.add_space(14.0);
    Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(8)
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for line in lines {
                    ui.horizontal_top(|ui| {
                        ui.label(RichText::new("•").color(tone.color()));
                        ui.add(egui::Label::new(line).wrap());
                    });
                }
            });
        });
}
