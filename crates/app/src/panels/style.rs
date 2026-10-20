//! Style editor: local style properties per part and state.

use eframe::egui::{self, RichText};
use lvglab_model::compat;
use lvglab_model::flags::{ObjState, StateMask};
use lvglab_model::project::LvglVersion;
use lvglab_model::style::{StyleGroup, StyleKind, StyleProp, StyleValue};
use lvglab_model::{Project, Widget};

use crate::app::Studio;
use crate::i18n::{tr, trf};
use crate::visuals;
use crate::widgets::{self, muted};

/// Frequently used properties offered as one-click buttons.
const QUICK: &[(&str, &[StyleProp])] = &[
    ("Background", &[StyleProp::BgColor, StyleProp::BgOpa]),
    ("Background image", &[StyleProp::BgImgSrc]),
    ("Border", &[StyleProp::BorderColor, StyleProp::BorderWidth, StyleProp::BorderOpa]),
    ("Radius", &[StyleProp::Radius]),
    ("Text", &[StyleProp::TextColor, StyleProp::TextFont]),
    ("Padding", &[StyleProp::PadTop, StyleProp::PadBottom, StyleProp::PadLeft, StyleProp::PadRight]),
    ("Shadow", &[StyleProp::ShadowWidth, StyleProp::ShadowColor, StyleProp::ShadowOpa]),
];

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    if !super::selection_header(ui, studio) {
        return;
    }
    let Some(original) = studio.selected_widget().cloned() else { return };
    let mut widget = original.clone();
    let def = widget.def();
    let version = studio.doc.project.lvgl_version;
    let slots: Vec<_> = def.slots.iter().filter(|s| compat::slot_supported(version, widget.kind, s)).collect();
    if !slots.iter().any(|s| s.key == studio.style_slot) {
        studio.style_slot = "main".to_owned();
    }

    ui.label(RichText::new(tr("Part")).small().strong());
    ui.horizontal_wrapped(|ui| {
        for slot in slots {
            let used = !widget.styles.states_of(slot.key).is_empty();
            let text = if used { format!("{} •", tr(slot.label)) } else { tr(slot.label).to_owned() };
            if ui.selectable_label(studio.style_slot == slot.key, text).clicked() {
                studio.style_slot = slot.key.to_owned();
            }
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new(tr("State")).small().strong());
    state_picker(ui, &mut studio.style_state, &widget, &studio.style_slot);
    if studio.style_state != StateMask::DEFAULT {
        ui.colored_label(visuals::ACCENT, trf("Canvas previews {}", &[&studio.style_state]));
    }
    ui.separator();

    let slot = studio.style_slot.clone();
    let state = studio.style_state;
    let project = &studio.doc.project;
    let props: Vec<(StyleProp, StyleValue)> = widget
        .styles
        .block(&slot, state)
        .map(|b| b.props.iter().map(|(p, v)| (*p, v.clone())).collect())
        .unwrap_or_default();

    if props.is_empty() {
        muted(ui, tr("No local style for this part and state. The theme default is used."));
    }
    let mut remove = Vec::new();
    for group in StyleGroup::ALL {
        let in_group: Vec<_> = props.iter().filter(|(p, _)| p.group() == group).collect();
        if in_group.is_empty() {
            continue;
        }
        ui.label(RichText::new(tr(group.label())).strong());
        egui::Grid::new(("style_group", group.label())).num_columns(3).spacing([10.0, 6.0]).min_col_width(90.0).show(
            ui,
            |ui| {
                for (prop, value) in in_group {
                    let mut value = value.clone();
                    ui.label(tr(prop.label())).on_hover_text(format!("lv_obj_set_style_{}", prop.name()));
                    if value_editor(ui, project, *prop, &mut value) {
                        widget.styles.set(&slot, state, *prop, value);
                    }
                    if ui.small_button(crate::icons::X).on_hover_text(tr("Remove")).clicked() {
                        remove.push(*prop);
                    }
                    ui.end_row();
                }
            },
        );
        ui.add_space(4.0);
    }
    for prop in remove {
        widget.styles.remove(&slot, state, prop);
    }

    ui.separator();
    add_property(ui, &mut widget, &slot, state, version);
    ui.horizontal_wrapped(|ui| {
        for (name, group) in QUICK {
            if ui.small_button(format!("+ {}", tr(name))).clicked() {
                for prop in *group {
                    if widget.styles.get(&slot, state, *prop).is_none() {
                        widget.styles.set(&slot, state, *prop, prop.initial_value());
                    }
                }
            }
        }
    });
    if !props.is_empty() && ui.button(tr("Clear this state")).clicked() {
        for (prop, _) in &props {
            widget.styles.remove(&slot, state, *prop);
        }
    }

    if widget != original {
        if let Some(target) = studio.doc.project.widget_mut(original.id) {
            *target = widget;
        }
    }
}

fn state_picker(ui: &mut egui::Ui, mask: &mut StateMask, widget: &Widget, slot: &str) {
    let used = widget.styles.states_of(slot);
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(*mask == StateMask::DEFAULT, tr("Default")).clicked() {
            *mask = StateMask::DEFAULT;
        }
        for state in ObjState::ALL {
            let on = mask.contains(*state);
            let marked = used.iter().any(|m| m.contains(*state));
            let text = if marked { format!("{} •", tr(state.label())) } else { tr(state.label()).to_owned() };
            if ui.selectable_label(on, text).on_hover_text(tr("Click to combine states")).clicked() {
                *mask = mask.toggled(*state);
            }
        }
    });
}

fn add_property(ui: &mut egui::Ui, widget: &mut Widget, slot: &str, state: StateMask, version: LvglVersion) {
    let filter_id = ui.make_persistent_id("style_add_filter");
    let mut filter: String = ui.data_mut(|d| d.get_temp(filter_id).unwrap_or_default());
    egui::ComboBox::from_id_salt("style_add")
        .selected_text(format!("+ {}", tr("Add property")))
        .width(220.0)
        .height(420.0)
        .show_ui(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text(tr("Search…")));
            let needle = filter.to_lowercase();
            for group in StyleGroup::ALL {
                let candidates: Vec<StyleProp> = StyleProp::ALL
                    .iter()
                    .copied()
                    .filter(|p| p.group() == group && widget.styles.get(slot, state, *p).is_none())
                    .filter(|p| compat::style_supported(version, *p))
                    .filter(|p| {
                        needle.is_empty()
                            || p.name().contains(&needle)
                            || tr(p.label()).to_lowercase().contains(&needle)
                    })
                    .collect();
                if candidates.is_empty() {
                    continue;
                }
                ui.label(RichText::new(tr(group.label())).small().strong());
                for prop in candidates {
                    if ui.selectable_label(false, format!("{}  ·  {}", tr(prop.label()), prop.name())).clicked() {
                        widget.styles.set(slot, state, prop, prop.initial_value());
                    }
                }
            }
        });
    ui.data_mut(|d| d.insert_temp(filter_id, filter));
}

fn value_editor(ui: &mut egui::Ui, project: &Project, prop: StyleProp, value: &mut StyleValue) -> bool {
    let themeable = matches!(prop.kind(), StyleKind::Color | StyleKind::Opa);
    if themeable && (!project.color_themes.variables.is_empty() || value.as_theme().is_some()) {
        return ui
            .horizontal(|ui| {
                let mut changed = theme_choice(ui, project, prop, value);
                if value.as_theme().is_none() {
                    changed |= plain_value_editor(ui, project, prop, value);
                }
                changed
            })
            .inner;
    }
    plain_value_editor(ui, project, prop, value)
}

/// Chooses between a fixed value and a color theme variable.
fn theme_choice(ui: &mut egui::Ui, project: &Project, prop: StyleProp, value: &mut StyleValue) -> bool {
    let themes = &project.color_themes;
    let current = value.as_theme().map(str::to_owned);
    let mut changed = false;
    let selected = current.clone().unwrap_or_else(|| tr("Fixed").to_owned());
    egui::ComboBox::from_id_salt(("style_theme", prop.name()))
        .selected_text(selected)
        .width(96.0)
        .show_ui(ui, |ui| {
            if ui.selectable_label(current.is_none(), tr("Fixed value")).clicked() && current.is_some() {
                // Keep what the active theme shows.
                *value = match (prop.kind(), current.as_deref().and_then(|v| themes.value(v))) {
                    (StyleKind::Color, Some(c)) => StyleValue::Color(c.color),
                    (StyleKind::Opa, Some(c)) => StyleValue::Int(i32::from(c.opa)),
                    _ => prop.initial_value(),
                };
                changed = true;
            }
            for variable in &themes.variables {
                let on = current.as_deref() == Some(variable.as_str());
                ui.horizontal(|ui| {
                    if let Some(c) = themes.value(variable) {
                        widgets::swatch(ui, egui::Color32::from_rgb(c.color.r, c.color.g, c.color.b));
                    }
                    if ui.selectable_label(on, variable).clicked() && !on {
                        *value = StyleValue::Theme(variable.clone());
                        changed = true;
                    }
                });
            }
        })
        .response
        .on_hover_text(tr("Bind to a theme color: the value follows the active theme (ui_theme_set)."));
    if current.as_deref().is_some_and(|v| themes.index_of(v).is_none()) {
        ui.colored_label(crate::visuals::WARNING, tr("missing"));
    }
    changed
}

fn plain_value_editor(ui: &mut egui::Ui, project: &Project, prop: StyleProp, value: &mut StyleValue) -> bool {
    let id = ("style_value", prop.name());
    match (prop.kind(), value) {
        (StyleKind::Coord, StyleValue::Coord(c)) => {
            let content = matches!(prop, StyleProp::Width | StyleProp::Height);
            widgets::coord(ui, id, c, content)
        }
        (StyleKind::Int { min, max }, StyleValue::Int(v)) => {
            let text =
                if prop == StyleProp::Radius && *v >= lvglab_model::theme::RADIUS_CIRCLE { tr("circle") } else { "" };
            ui.horizontal(|ui| {
                let changed = ui.add(egui::DragValue::new(v).range(min..=max)).changed();
                if prop == StyleProp::Radius
                    && ui.small_button(crate::icons::CIRCLE).on_hover_text("LV_RADIUS_CIRCLE").clicked()
                {
                    *v = lvglab_model::theme::RADIUS_CIRCLE;
                    return true;
                }
                if !text.is_empty() {
                    muted(ui, text);
                }
                changed
            })
            .inner
        }
        (StyleKind::Opa, StyleValue::Int(v)) => {
            let mut percent = (*v as f32 / 255.0 * 100.0).round() as i32;
            let changed = ui.add(egui::Slider::new(&mut percent, 0..=100).suffix(" %")).changed();
            if changed {
                *v = (percent as f32 * 2.55).round() as i32;
            }
            changed
        }
        (StyleKind::Color, StyleValue::Color(c)) => ui.horizontal(|ui| widgets::color(ui, c)).inner,
        (StyleKind::Bool, StyleValue::Bool(b)) => ui.checkbox(b, "").changed(),
        (StyleKind::Enum(options), StyleValue::Enum(e)) => {
            let version = project.lvgl_version;
            let options: Vec<_> = options
                .iter()
                .copied()
                .filter(|o| compat::style_value_supported(version, &StyleValue::Enum(o.c.to_owned())))
                .collect();
            widgets::options(ui, id, e, &options)
        }
        (StyleKind::Font, StyleValue::Font(f)) => widgets::font_ref(ui, id, f, project),
        (StyleKind::Image, StyleValue::Image(i)) => widgets::image_ref(ui, id, i, project),
        (_, value) => {
            // Values with an unexpected type (e.g. from older files) are reset.
            if ui.button(tr("Reset")).clicked() {
                *value = prop.initial_value();
                return true;
            }
            false
        }
    }
}
