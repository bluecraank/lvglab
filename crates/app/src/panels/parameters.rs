//! Parameters of the selected widget: geometry, widget properties, layout,
//! flags, states and scrolling.
//!
//! The panel edits a copy of the widget and writes it back when something
//! changed, so pickers can read the rest of the project meanwhile.

use eframe::egui::{self, RichText};
use lvglab_model::coord::Align;
use lvglab_model::flags::{Dir, FlexAlign, FlexFlow, ObjFlag, ObjState, ScrollbarMode, SnapMode};
use lvglab_model::prop::{PropDef, PropKind, PropValue};
use lvglab_model::widget::FlexLayout;
use lvglab_model::{Project, Widget, WidgetKind};

use crate::app::Studio;
use crate::i18n::tr;
use crate::visuals;
use crate::widgets::{self, grid, label, section};

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    if !super::selection_header(ui, studio) {
        return;
    }
    let Some(original) = studio.selected_widget().cloned() else { return };
    let mut widget = original.clone();
    let project = &studio.doc.project;

    identity(ui, project, &mut widget);
    if let Some(missing) = lvglab_codegen::glyphs::missing_glyphs_of(project, widget.id) {
        let chars: String = missing.chars.iter().collect();
        ui.colored_label(
            visuals::WARNING,
            crate::i18n::trf("Font {} has no glyphs for: {}", &[&missing.font, &chars]),
        )
        .on_hover_text(tr("These characters are not drawn on the device. Add them to the font's ranges or symbols in the Fonts panel."));
    }
    if widget.kind != WidgetKind::Screen && !widget.def().managed_geometry {
        section(ui, "geometry", tr("Position & size"), true, |ui| geometry(ui, &mut widget));
    }
    if !widget.def().props.is_empty() {
        section(ui, "props", tr(widget.def().name), true, |ui| properties(ui, project, &mut widget));
    }
    if widget.def().container {
        section(ui, "layout", tr("Layout"), widget.layout.is_some(), |ui| flex(ui, &mut widget));
    }
    section(ui, "flags", tr("Flags"), false, |ui| flags(ui, &mut widget));
    section(ui, "states", tr("States"), false, |ui| states(ui, &mut widget));
    section(ui, "scroll", tr("Scrolling"), false, |ui| scroll(ui, &mut widget));

    if widget != original {
        let renamed = (widget.name != original.name).then(|| widget.name.clone());
        if let Some(target) = studio.doc.project.widget_mut(original.id) {
            *target = widget;
        }
        if let Some(new) = renamed {
            if studio.doc.project.is_component(original.id) {
                studio.doc.project.rename_component_instances(&original.name, &new);
            }
        }
    }
}

fn identity(ui: &mut egui::Ui, project: &Project, widget: &mut Widget) {
    grid(ui, "identity", |ui| {
        label(ui, tr("Name"), tr("Used for the C variable ui_<Name>."));
        ui.add(egui::TextEdit::singleline(&mut widget.name).desired_width(f32::INFINITY));
        ui.end_row();
    });
    let valid = !widget.name.is_empty() && widget.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        ui.colored_label(visuals::WARNING, tr("Use letters, digits and _ only."));
    } else if project.name_in_use(&widget.name, Some(widget.id)) {
        ui.colored_label(visuals::WARNING, tr("Name already used; the export adds a suffix."));
    }
    ui.add_space(4.0);
}

fn geometry(ui: &mut egui::Ui, widget: &mut Widget) {
    let g = &mut widget.geometry;
    ui.horizontal(|ui| {
        align_picker(ui, &mut g.align);
        ui.vertical(|ui| {
            widgets::choice(ui, "align_all", &mut g.align, Align::ALL, Align::label);
        });
    });
    grid(ui, "geometry_grid", |ui| {
        label(ui, "X", "");
        widgets::coord(ui, "x_unit", &mut g.x, false);
        ui.end_row();
        label(ui, "Y", "");
        widgets::coord(ui, "y_unit", &mut g.y, false);
        ui.end_row();
        label(ui, tr("Width"), "");
        widgets::coord(ui, "w_unit", &mut g.width, true);
        ui.end_row();
        label(ui, tr("Height"), "");
        widgets::coord(ui, "h_unit", &mut g.height, true);
        ui.end_row();
        label(ui, tr("Extend click area"), "lv_obj_set_ext_click_area");
        ui.add(egui::DragValue::new(&mut widget.ext_click_area).range(0..=200).suffix(" px"));
        ui.end_row();
    });
}

/// A 3×3 grid of buttons for the inside alignments.
fn align_picker(ui: &mut egui::Ui, align: &mut Align) {
    egui::Grid::new("align_picker").spacing([2.0, 2.0]).show(ui, |ui| {
        for (i, candidate) in Align::INSIDE_GRID.iter().enumerate() {
            let selected = *align == *candidate || (*candidate == Align::TopLeft && *align == Align::Default);
            let button = egui::Button::new(if selected { "⬛" } else { "⬜" })
                .min_size(egui::vec2(20.0, 18.0))
                .selected(selected);
            if ui.add(button).on_hover_text(tr(candidate.label())).clicked() {
                *align = *candidate;
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });
}

fn properties(ui: &mut egui::Ui, project: &Project, widget: &mut Widget) {
    grid(ui, ("props_grid", widget.kind), |ui| {
        for def in widget.def().props {
            let mut value = widget.prop(def.key).unwrap_or_else(|| def.default_value());
            label(ui, tr(def.label), tr(def.help));
            if prop_editor(ui, project, widget, def, &mut value) {
                widget.set_prop(def.key, value);
            }
            ui.end_row();
        }
    });
}

fn prop_editor(ui: &mut egui::Ui, project: &Project, widget: &Widget, def: &PropDef, value: &mut PropValue) -> bool {
    let id = (widget.id, def.key);
    match (def.kind, value) {
        (PropKind::Bool, PropValue::Bool(v)) => ui.checkbox(v, "").changed(),
        (PropKind::Int { min, max }, PropValue::Int(v)) => ui.add(egui::DragValue::new(v).range(min..=max)).changed(),
        (PropKind::Text, PropValue::Text(v)) if def.key == "component" => {
            let mut changed = false;
            egui::ComboBox::from_id_salt(id).selected_text(v.as_str()).show_ui(ui, |ui| {
                for component in &project.components {
                    if ui.selectable_label(*v == component.name, &component.name).clicked() {
                        v.clone_from(&component.name);
                        changed = true;
                    }
                }
                if project.components.is_empty() {
                    crate::widgets::muted(ui, tr("Create a component in the Screens panel first."));
                }
            });
            changed
        }
        (PropKind::Text, PropValue::Text(v)) => {
            ui.add(egui::TextEdit::singleline(v).desired_width(f32::INFINITY)).changed()
        }
        (
            PropKind::Lines
            | PropKind::Points
            | PropKind::StrArray { .. }
            | PropKind::ImageArray
            | PropKind::ChartSeries,
            PropValue::Text(v),
        ) => ui.add(egui::TextEdit::multiline(v).desired_rows(3).desired_width(f32::INFINITY)).changed(),
        (PropKind::Enum(options), PropValue::Enum(v)) => widgets::options(ui, id, v, options),
        (PropKind::Color, PropValue::Color(c)) => ui.horizontal(|ui| widgets::color(ui, c)).inner,
        (PropKind::Pair { min, max, first, second }, PropValue::Pair(a, b)) => {
            ui.horizontal(|ui| {
                let changed_a =
                    ui.add(egui::DragValue::new(a).range(min..=max).prefix(format!("{} ", tr(first)))).changed();
                let changed_b =
                    ui.add(egui::DragValue::new(b).range(min..=max).prefix(format!("{} ", tr(second)))).changed();
                changed_a || changed_b
            })
            .inner
        }
        (PropKind::Image, PropValue::Image(name)) => widgets::image_ref(ui, id, name, project),
        (PropKind::WidgetRef, PropValue::WidgetRef(target)) => {
            let filter = (def.key == "textarea").then_some(WidgetKind::Textarea);
            widgets::widget_ref(ui, id, target, project, filter)
        }
        (PropKind::Date, PropValue::Date { year, month, day }) => {
            ui.horizontal(|ui| {
                let y = ui.add(egui::DragValue::new(year).range(1900..=2200)).changed();
                let m = ui.add(egui::DragValue::new(month).range(1..=12)).changed();
                let d = ui.add(egui::DragValue::new(day).range(1..=31)).changed();
                y || m || d
            })
            .inner
        }
        _ => {
            ui.label(RichText::new(tr("Invalid value")).color(visuals::DANGER));
            false
        }
    }
}

fn flex(ui: &mut egui::Ui, widget: &mut Widget) {
    let mut enabled = widget.layout.is_some();
    if ui.checkbox(&mut enabled, tr("Flex layout")).changed() {
        widget.layout = enabled.then(FlexLayout::default);
    }
    let Some(layout) = &mut widget.layout else {
        widgets::muted(ui, tr("Children are positioned by their own X/Y and alignment."));
        return;
    };
    grid(ui, "flex_grid", |ui| {
        label(ui, tr("Flow"), "");
        widgets::choice(ui, "flex_flow", &mut layout.flow, FlexFlow::ALL, FlexFlow::label);
        ui.end_row();
        label(ui, tr("Main axis"), "");
        widgets::choice(ui, "flex_main", &mut layout.main_place, FlexAlign::ALL, FlexAlign::label);
        ui.end_row();
        label(ui, tr("Cross axis"), "");
        widgets::choice(ui, "flex_cross", &mut layout.cross_place, FlexAlign::ALL, FlexAlign::label);
        ui.end_row();
        label(ui, tr("Tracks"), "");
        widgets::choice(ui, "flex_track", &mut layout.track_place, FlexAlign::ALL, FlexAlign::label);
        ui.end_row();
    });
    widgets::muted(ui, tr("Gaps come from the style properties Row gap / Column gap."));
}

fn flags(ui: &mut egui::Ui, widget: &mut Widget) {
    egui::Grid::new("flags_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
        for (i, flag) in ObjFlag::ALL.iter().enumerate() {
            let mut on = widget.flag(*flag);
            let overridden = widget.flags.contains_key(flag);
            let text =
                if overridden { RichText::new(tr(flag.label())).strong() } else { RichText::new(tr(flag.label())) };
            if ui.checkbox(&mut on, text).on_hover_text(flag.c_const()).changed() {
                widget.set_flag(*flag, on);
            }
            if i % 2 == 1 {
                ui.end_row();
            }
        }
    });
    widgets::muted(ui, tr("Bold flags differ from the LVGL default."));
}

fn states(ui: &mut egui::Ui, widget: &mut Widget) {
    egui::Grid::new("states_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
        for (i, state) in ObjState::ALL.iter().enumerate() {
            let mut on = widget.states.contains(state);
            if ui.checkbox(&mut on, tr(state.label())).on_hover_text(state.c_const()).changed() {
                if on {
                    widget.states.insert(*state);
                } else {
                    widget.states.remove(state);
                }
            }
            if i % 2 == 1 {
                ui.end_row();
            }
        }
    });
    widgets::muted(ui, tr("Initial states, e.g. Checked for a switch that starts on."));
}

fn scroll(ui: &mut egui::Ui, widget: &mut Widget) {
    let s = &mut widget.scroll;
    grid(ui, "scroll_grid", |ui| {
        label(ui, tr("Scrollbar"), "");
        widgets::optional_choice(ui, "scrollbar", &mut s.scrollbar_mode, ScrollbarMode::ALL, ScrollbarMode::label);
        ui.end_row();
        label(ui, tr("Direction"), "");
        widgets::optional_choice(ui, "scroll_dir", &mut s.dir, Dir::ALL, Dir::label);
        ui.end_row();
        label(ui, tr("Snap X"), "");
        widgets::optional_choice(ui, "snap_x", &mut s.snap_x, SnapMode::ALL, SnapMode::label);
        ui.end_row();
        label(ui, tr("Snap Y"), "");
        widgets::optional_choice(ui, "snap_y", &mut s.snap_y, SnapMode::ALL, SnapMode::label);
        ui.end_row();
    });
}
