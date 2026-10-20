//! Ctrl+F: search over all widgets, properties, styles and events. A click
//! on a result opens its screen and selects the widget.

use eframe::egui::{self, Align2, Key, RichText, Sense, vec2};
use lvglab_model::search::{self, Hit, HitKind};

use crate::app::Studio;
use crate::i18n::{tr, trf};
use crate::visuals::{self, ACCENT};

const LIMIT: usize = 300;

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    /// Index of the highlighted result (arrow keys).
    pub current: usize,
    /// Focus the text field on the first frame.
    pub fresh: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self { fresh: true, ..Self::default() }
    }
}

fn kind_label(hit: &Hit) -> &'static str {
    match hit.what {
        HitKind::Widget => "Widget",
        HitKind::Property => "Property",
        HitKind::Style => "Style",
        HitKind::Event => "Event",
        HitKind::Flag => "Flag",
    }
}

pub fn show(ctx: &egui::Context, studio: &mut Studio) {
    let Some(state) = &mut studio.search else { return };
    let hits = search::search(&studio.doc.project, &state.query, LIMIT);
    state.current = state.current.min(hits.len().saturating_sub(1));
    let mut open = true;
    let mut chosen: Option<usize> = None;
    let palette = visuals::palette(ctx);
    egui::Window::new(tr("Search"))
        .open(&mut open)
        .anchor(Align2::CENTER_TOP, vec2(0.0, 70.0))
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .show(ctx, |ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut state.query)
                    .hint_text(tr("Widget, property, color, font, event …"))
                    .desired_width(f32::INFINITY),
            );
            if state.fresh {
                field.request_focus();
                state.fresh = false;
            }
            if field.changed() {
                state.current = 0;
            }
            ui.add_space(4.0);
            if state.query.trim().is_empty() {
                ui.label(
                    RichText::new(tr(
                        "Words are combined: \"label title\" finds a label named Title; \"F15C00\" finds a color.",
                    ))
                    .color(palette.muted),
                );
                return;
            }
            if hits.is_empty() {
                ui.label(RichText::new(tr("Nothing found.")).color(palette.muted));
                return;
            }
            let shown = if hits.len() >= LIMIT {
                trf("First {} results", &[&LIMIT])
            } else {
                trf("{} results", &[&hits.len()])
            };
            ui.label(RichText::new(shown).small().color(palette.muted));
            let (up, down, enter) =
                ui.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter)));
            if down {
                state.current = (state.current + 1).min(hits.len() - 1);
            }
            if up {
                state.current = state.current.saturating_sub(1);
            }
            if enter {
                chosen = Some(state.current);
            }
            egui::ScrollArea::vertical().max_height(420.0).auto_shrink([false, true]).show(ui, |ui| {
                for (i, hit) in hits.iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                    let selected = i == state.current;
                    if selected || response.hovered() {
                        ui.painter().rect_filled(rect, 6, ui.visuals().widgets.hovered.bg_fill);
                    }
                    if selected {
                        ui.scroll_to_rect(rect, None);
                    }
                    let inner = rect.shrink2(vec2(8.0, 4.0));
                    let painter = ui.painter();
                    painter.text(
                        inner.left_top(),
                        Align2::LEFT_TOP,
                        &hit.widget_name,
                        egui::FontId::proportional(14.0),
                        if selected { ACCENT } else { ui.visuals().strong_text_color() },
                    );
                    let place = if hit.is_component {
                        format!("\u{E390} {}", hit.root_name)
                    } else {
                        format!("\u{E32E} {}", hit.root_name)
                    };
                    painter.text(
                        inner.right_top(),
                        Align2::RIGHT_TOP,
                        format!("{} · {}", tr(kind_label(hit)), place),
                        egui::FontId::proportional(11.0),
                        palette.muted,
                    );
                    painter.text(
                        inner.left_bottom(),
                        Align2::LEFT_BOTTOM,
                        &hit.detail,
                        egui::FontId::proportional(12.0),
                        palette.muted,
                    );
                    if response.clicked() {
                        chosen = Some(i);
                    }
                }
            });
        });
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        open = false;
    }
    if let Some(hit) = chosen.and_then(|i| hits.get(i)) {
        studio.select_screen(hit.root);
        studio.select(Some(hit.widget));
        studio.canvas.overview = false;
        studio.canvas.fit_pending = true;
        studio.search = None;
        return;
    }
    if !open {
        studio.search = None;
    }
}
