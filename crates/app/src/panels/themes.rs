//! Project-wide themes: the LVGL default theme and the color themes.

use eframe::egui;
use lvglab_model::color_theme::{ColorTheme, ThemeColor};
use lvglab_model::{Palette, Project};

use crate::app::Studio;
use crate::i18n::tr;
use crate::widgets::{self, grid, label, section};

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    let p = &mut studio.doc.project;
    section(ui, "themes_default", tr("Theme (lv_theme_default)"), true, |ui| {
        grid(ui, "themes_default_grid", |ui| {
            label(ui, tr("Mode"), "");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.theme.dark, true, tr("Dark"));
                ui.selectable_value(&mut p.theme.dark, false, tr("Light"));
            });
            ui.end_row();
            label(ui, tr("Primary color"), "");
            palette_choice(ui, "primary", &mut p.theme.primary);
            ui.end_row();
            label(ui, tr("Secondary color"), "");
            palette_choice(ui, "secondary", &mut p.theme.secondary);
            ui.end_row();
        });
    });
    section(ui, "themes_colors", tr("Color themes"), true, |ui| color_themes(ui, p));
}

/// Theme colors: variables with one value per theme. Styles bound to a
/// variable follow `ui_theme_set()` at runtime.
fn color_themes(ui: &mut egui::Ui, p: &mut Project) {
    // Edits a normalized copy so that merely showing the panel leaves the
    // project unchanged.
    let mut edited = p.color_themes.clone();
    if edited.themes.is_empty() {
        edited.themes.push(ColorTheme { name: "Default".to_owned(), values: Vec::new() });
    }
    edited.normalize();
    let shown = edited.clone();
    let themes = &mut edited;
    ui.horizontal_wrapped(|ui| {
        label(ui, tr("Theme"), tr("The first theme is the default theme. The editor shows the selected one."));
        for (i, theme) in themes.themes.iter().enumerate() {
            if ui.selectable_label(themes.active == i, &theme.name).clicked() {
                themes.active = i;
            }
        }
        if ui.small_button("+").on_hover_text(tr("Add theme")).clicked() {
            let name = format!("Theme{}", themes.themes.len() + 1);
            themes.add_theme(name);
            themes.active = themes.themes.len() - 1;
        }
    });
    let active = themes.active;
    ui.horizontal(|ui| {
        ui.label(tr("Name"));
        ui.text_edit_singleline(&mut themes.themes[active].name);
        if themes.themes.len() > 1 && ui.small_button(tr("Remove theme")).clicked() {
            themes.remove_theme(active);
        }
    });
    ui.add_space(4.0);
    let mut rename = None;
    let mut remove = None;
    let mut runtime_change = None;
    grid(ui, "themes_color_grid", |ui| {
        for index in 0..themes.variables.len() {
            let mut name = themes.variables[index].clone();
            if ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0)).changed() {
                rename = Some((themes.variables[index].clone(), name));
            }
            let variable = themes.variables[index].clone();
            let mut runtime = themes.is_runtime(&variable);
            let value = &mut themes.themes[active].values[index];
            ui.horizontal(|ui| {
                widgets::color(ui, &mut value.color);
                ui.add(egui::DragValue::new(&mut value.opa).range(0..=255).prefix("α "));
                if ui
                    .checkbox(&mut runtime, tr("Runtime"))
                    .on_hover_text(tr("Exported without const: the application can change the value at runtime and re-apply the theme."))
                    .changed()
                {
                    runtime_change = Some((variable, runtime));
                }
                if ui.small_button(crate::icons::X).on_hover_text(tr("Remove color")).clicked() {
                    remove = Some(index);
                }
            });
            ui.end_row();
        }
    });
    if ui.button(tr("Add color")).clicked() {
        let mut n = themes.variables.len() + 1;
        while themes.index_of(&format!("Color{n}")).is_some() {
            n += 1;
        }
        themes.add_variable(format!("Color{n}"), ThemeColor::default());
    }
    if let Some((variable, runtime)) = runtime_change {
        themes.set_runtime(&variable, runtime);
    }
    if let Some(index) = remove {
        themes.remove_variable(index);
    }
    if edited != shown {
        p.color_themes = edited;
    }
    if let Some((old, new)) = rename {
        p.rename_theme_variable(&old, &new);
    }
}

fn palette_choice(ui: &mut egui::Ui, id: &str, value: &mut Palette) {
    ui.horizontal(|ui| {
        let c = value.main();
        widgets::swatch(ui, egui::Color32::from_rgb(c.r, c.g, c.b));
        widgets::choice(ui, id, value, Palette::ALL, Palette::label);
    });
}
