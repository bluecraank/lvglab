//! The widget palette: click to add, drag onto the canvas to place.

use eframe::egui::{self, RichText, Sense, Vec2};
use lvglab_model::catalog::{self, Category, WidgetDef};
use lvglab_model::compat;

use crate::app::Studio;
use crate::commands::Command;
use crate::i18n::tr;
use crate::visuals;

const TILE: Vec2 = Vec2::new(78.0, 58.0);

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    ui.add(
        egui::TextEdit::singleline(&mut studio.palette_filter)
            .hint_text(tr("Search widgets…"))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    let filter = studio.palette_filter.to_lowercase();
    let version = studio.doc.project.lvgl_version;
    for category in Category::PALETTE {
        let defs: Vec<&WidgetDef> = catalog::in_category(category)
            .filter(|d| compat::widget_supported(version, d.kind))
            .filter(|d| {
                filter.is_empty()
                    || d.name.to_lowercase().contains(&filter)
                    || tr(d.name).to_lowercase().contains(&filter)
            })
            .collect();
        if defs.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(RichText::new(tr(category.label())).strong())
            .id_salt(("palette", category.label()))
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for def in defs {
                        tile(ui, studio, def);
                    }
                });
            });
    }
}

fn tile(ui: &mut egui::Ui, studio: &mut Studio, def: &'static WidgetDef) {
    let (rect, response) = ui.allocate_exact_size(TILE, Sense::click_and_drag());
    let visuals = ui.style().interact(&response);
    let fill = if response.hovered() { visuals.bg_fill } else { ui.visuals().faint_bg_color };
    ui.painter().rect_filled(rect, 8, fill);
    if response.hovered() {
        ui.painter().rect_stroke(rect, 8, egui::Stroke::new(1.0_f32, visuals::ACCENT), egui::StrokeKind::Inside);
    }
    let text_color = ui.visuals().text_color();
    ui.painter().text(
        rect.center_top() + egui::vec2(0.0, 20.0),
        egui::Align2::CENTER_CENTER,
        crate::icons::widget(def.kind),
        egui::FontId::proportional(20.0),
        text_color,
    );
    ui.painter().text(
        rect.center_bottom() - egui::vec2(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        tr(def.name),
        egui::FontId::proportional(11.0),
        text_color,
    );
    let version = studio.doc.project.lvgl_version;
    let note = if version.is_v9() && def.v9.is_deprecated(version.major_minor().1) {
        catalog::V9::DEPRECATED_NOTE
    } else {
        def.v9.note
    };
    let response = if version.is_v9() && !note.is_empty() {
        response.on_hover_text(format!("{}\n\n{}", tr(def.description), tr(note)))
    } else {
        response.on_hover_text(tr(def.description))
    };
    if response.drag_started() {
        response.dnd_set_drag_payload(def.kind);
    }
    if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if response.clicked() {
        studio.push(Command::AddWidget { kind: def.kind, parent: None });
    }
}
