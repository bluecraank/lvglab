//! Small reusable editor controls.

use eframe::egui::{self, Color32, Response, RichText, Ui};
use lvglab_model::coord::{Coord, Unit};
use lvglab_model::prop::EnumOption;
use lvglab_model::style::MONTSERRAT_SIZES;
use lvglab_model::{Color, Project, WidgetId, WidgetKind};

use crate::i18n::tr;
use crate::visuals;

/// A collapsible section with a bold title.
pub fn section<R>(ui: &mut Ui, id: &str, title: &str, open: bool, add: impl FnOnce(&mut Ui) -> R) {
    egui::CollapsingHeader::new(RichText::new(title).strong()).id_salt(id).default_open(open).show(ui, |ui| {
        add(ui);
    });
}

/// Two column grid for label/value rows.
pub fn grid<R>(ui: &mut Ui, id: impl std::hash::Hash, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Grid::new(id).num_columns(2).spacing([12.0, 6.0]).min_col_width(96.0).show(ui, add).inner
}

/// Row label with optional help tooltip.
pub fn label(ui: &mut Ui, text: &str, help: &str) -> Response {
    let response = ui.label(text);
    if help.is_empty() { response } else { response.on_hover_text(help) }
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) -> Response {
    let color = visuals::palette(ui.ctx()).muted;
    ui.label(RichText::new(text.into()).color(color).small())
}

/// Hint shown in empty panels.
pub fn empty_hint(ui: &mut Ui, text: &str) {
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        muted(ui, text);
    });
}

/// Edits a coordinate value and its unit.
pub fn coord(ui: &mut Ui, id: impl std::hash::Hash, value: &mut Coord, allow_content: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(value.unit != Unit::Content, |ui| {
            let suffix = if value.unit == Unit::Percent { "%" } else { "" };
            changed |= ui.add(egui::DragValue::new(&mut value.value).range(-10000..=10000).suffix(suffix)).changed();
        });
        let units: &[Unit] = if allow_content { &Unit::ALL } else { &[Unit::Px, Unit::Percent] };
        egui::ComboBox::from_id_salt(id).width(72.0).selected_text(tr(value.unit.label())).show_ui(ui, |ui| {
            for unit in units {
                if ui.selectable_value(&mut value.unit, *unit, tr(unit.label())).changed() {
                    changed = true;
                }
            }
        });
    });
    changed
}

pub fn color(ui: &mut Ui, value: &mut Color) -> bool {
    let mut rgb = [value.r, value.g, value.b];
    let mut changed = ui.color_edit_button_srgb(&mut rgb).changed();
    let mut hex = value.to_hex_string();
    let response = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(70.0).font(egui::TextStyle::Monospace));
    if response.changed() {
        if let Some(parsed) = Color::parse(&hex) {
            rgb = [parsed.r, parsed.g, parsed.b];
            changed = true;
        }
    }
    if changed {
        *value = Color::rgb(rgb[0], rgb[1], rgb[2]);
    }
    changed
}

/// Combo box over catalog options (stores the C constant).
pub fn options(ui: &mut Ui, id: impl std::hash::Hash, value: &mut String, options: &[EnumOption]) -> bool {
    let current = options.iter().find(|o| o.c == value.as_str()).map_or(value.as_str(), |o| tr(o.label)).to_owned();
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(current).show_ui(ui, |ui| {
        for option in options {
            if ui.selectable_label(value == option.c, tr(option.label)).clicked() {
                *value = option.c.to_owned();
                changed = true;
            }
        }
    });
    changed
}

/// Combo box over a Rust enum with labels.
pub fn choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    value: &mut T,
    all: &[T],
    label: impl Fn(T) -> &'static str,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(tr(label(*value))).show_ui(ui, |ui| {
        for item in all {
            if ui.selectable_label(*value == *item, tr(label(*item))).clicked() {
                *value = *item;
                changed = true;
            }
        }
    });
    changed
}

/// Optional variant of [`choice`]; `None` means "LVGL default".
pub fn optional_choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    value: &mut Option<T>,
    all: &[T],
    label: impl Fn(T) -> &'static str,
) -> bool {
    let mut changed = false;
    let text = value.map_or(tr("Default"), &label);
    egui::ComboBox::from_id_salt(id).selected_text(tr(text)).show_ui(ui, |ui| {
        if ui.selectable_label(value.is_none(), tr("Default")).clicked() {
            *value = None;
            changed = true;
        }
        for item in all {
            if ui.selectable_label(*value == Some(*item), tr(label(*item))).clicked() {
                *value = Some(*item);
                changed = true;
            }
        }
    });
    changed
}

/// Picks any widget (optionally restricted to one kind).
pub fn widget_ref(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    value: &mut Option<WidgetId>,
    project: &Project,
    kind: Option<WidgetKind>,
) -> bool {
    let name = value.and_then(|id| project.widget(id)).map_or_else(|| tr("None").to_owned(), |w| w.name.clone());
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(name).height(320.0).show_ui(ui, |ui| {
        if ui.selectable_label(value.is_none(), tr("None")).clicked() {
            *value = None;
            changed = true;
        }
        for screen in &project.screens {
            ui.label(RichText::new(&screen.name).small().strong());
            screen.walk(&mut |w| {
                if kind.is_none_or(|k| k == w.kind)
                    && ui.selectable_label(*value == Some(w.id), format!("   {}", w.name)).clicked()
                {
                    *value = Some(w.id);
                    changed = true;
                }
            });
        }
    });
    changed
}

pub fn screen_ref(ui: &mut Ui, id: impl std::hash::Hash, value: &mut Option<WidgetId>, project: &Project) -> bool {
    let name = value.and_then(|id| project.widget(id)).map_or_else(|| tr("None").to_owned(), |w| w.name.clone());
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(name).show_ui(ui, |ui| {
        for screen in &project.screens {
            if ui.selectable_label(*value == Some(screen.id), &screen.name).clicked() {
                *value = Some(screen.id);
                changed = true;
            }
        }
    });
    changed
}

pub fn image_ref(ui: &mut Ui, id: impl std::hash::Hash, value: &mut String, project: &Project) -> bool {
    let missing = !value.is_empty() && project.assets.image(value).is_none();
    let text = if value.is_empty() { tr("None").to_owned() } else { value.clone() };
    let text = if missing { RichText::new(text).color(visuals::DANGER) } else { RichText::new(text) };
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(text).show_ui(ui, |ui| {
        if ui.selectable_label(value.is_empty(), tr("None")).clicked() {
            value.clear();
            changed = true;
        }
        for image in &project.assets.images {
            if ui.selectable_label(*value == image.name, &image.name).clicked() {
                value.clone_from(&image.name);
                changed = true;
            }
        }
        ui.separator();
        if ui.button(tr("Add image…")).clicked() {
            if let Some(name) = crate::panels::images::pick_for_field(ui.ctx(), project) {
                *value = name;
                changed = true;
            }
        }
    });
    changed
}

pub fn font_ref(ui: &mut Ui, id: impl std::hash::Hash, value: &mut String, project: &Project) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(value.as_str()).height(360.0).show_ui(ui, |ui| {
        if !project.assets.fonts.is_empty() {
            ui.label(RichText::new(tr("Project fonts")).small().strong());
            for font in &project.assets.fonts {
                let text = format!("{}  ({} px)", font.name, font.size);
                if ui.selectable_label(*value == font.name, text).clicked() {
                    value.clone_from(&font.name);
                    changed = true;
                }
            }
            ui.separator();
        }
        ui.label(RichText::new(tr("Built-in Montserrat")).small().strong());
        for size in MONTSERRAT_SIZES {
            let name = format!("montserrat_{size}");
            if ui.selectable_label(*value == name, &name).clicked() {
                *value = name;
                changed = true;
            }
        }
    });
    changed
}

/// A small colored square used as legend.
pub fn swatch(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3, color);
}
