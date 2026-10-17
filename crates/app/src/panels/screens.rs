//! List of screens.

use eframe::egui::{self, RichText};
use lvglab_model::WidgetId;

use crate::app::Studio;
use crate::commands::Command;
use crate::i18n::{tr, trf};
use crate::widgets::muted;

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    ui.horizontal(|ui| {
        if ui.button(format!("+ {}", tr("Screen"))).clicked() {
            studio.push(Command::AddScreen);
        }
    });
    ui.add_space(4.0);
    let screens: Vec<(WidgetId, String, usize, bool)> = studio
        .doc
        .project
        .screens
        .iter()
        .map(|s| (s.id, s.name.clone(), s.descendant_count(), s.prop_bool("temporary")))
        .collect();
    let count = screens.len();
    for (index, (id, name, widgets, temporary)) in screens.into_iter().enumerate() {
        let current = studio.screen == id;
        let response = ui
            .horizontal(|ui| {
                let label = ui.selectable_label(current, RichText::new(format!("▣  {name}")).strong());
                muted(ui, trf("{} widgets", &[&widgets]));
                if index == 0 {
                    muted(ui, tr("start"));
                }
                if temporary {
                    muted(ui, tr("temporary"));
                }
                label
            })
            .inner;
        if response.clicked() {
            studio.select_screen(id);
        }
        response.context_menu(|ui| {
            if ui.add_enabled(index > 0, egui::Button::new(tr("Move up"))).clicked() {
                studio.push(Command::Shift { id, delta: -1 });
                ui.close_menu();
            }
            if ui.add_enabled(index + 1 < count, egui::Button::new(tr("Move down"))).clicked() {
                studio.push(Command::Shift { id, delta: 1 });
                ui.close_menu();
            }
            if ui.button(tr("Duplicate")).clicked() {
                studio.push(Command::Duplicate(id));
                ui.close_menu();
            }
            if ui.add_enabled(count > 1, egui::Button::new(tr("Delete"))).clicked() {
                studio.push(Command::Delete(id));
                ui.close_menu();
            }
        });
    }
    ui.add_space(6.0);
    muted(ui, tr("The first screen is loaded at start. Right-click for more."));
}
