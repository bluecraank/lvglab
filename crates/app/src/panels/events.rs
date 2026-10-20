//! Events of the selected widget and the actions they trigger.

use eframe::egui::{self, RichText};
use lvglab_model::event::{Action, EventHandler, EventTrigger, ModifyOp, ScreenAnim};
use lvglab_model::flags::{ObjFlag, ObjState};
use lvglab_model::{Project, WidgetKind};

use crate::app::Studio;
use crate::i18n::tr;
use crate::widgets::{self, grid, label, muted};

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    if !super::selection_header(ui, studio) {
        return;
    }
    let Some(original) = studio.selected_widget().cloned() else { return };
    let mut events = original.events.clone();
    let project = &studio.doc.project;

    if events.is_empty() {
        muted(ui, tr("No events. Events call LVGL actions or your own functions in ui_events.c."));
    }
    let mut remove = None;
    for (index, handler) in events.iter_mut().enumerate() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("#{}", index + 1)).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(crate::icons::X).on_hover_text(tr("Remove")).clicked() {
                        remove = Some(index);
                    }
                });
            });
            editor(ui, project, handler, index);
        });
        ui.add_space(4.0);
    }
    if let Some(index) = remove {
        events.remove(index);
    }
    if ui.button(format!("+ {}", tr("Add event"))).clicked() {
        let trigger =
            if original.kind == WidgetKind::Screen { EventTrigger::ScreenLoaded } else { EventTrigger::Clicked };
        events.push(EventHandler { trigger, action: Action::from_kind_index(1) });
    }

    if events != original.events {
        if let Some(target) = studio.doc.project.widget_mut(original.id) {
            target.events = events;
        }
    }
}

fn editor(ui: &mut egui::Ui, project: &Project, handler: &mut EventHandler, index: usize) {
    grid(ui, ("event", index), |ui| {
        label(ui, tr("Trigger"), "");
        widgets::choice(ui, ("trigger", index), &mut handler.trigger, EventTrigger::ALL, EventTrigger::label);
        ui.end_row();

        label(ui, tr("Action"), "");
        let mut kind = handler.action.kind_index();
        egui::ComboBox::from_id_salt(("action", index)).selected_text(tr(Action::KINDS[kind])).show_ui(ui, |ui| {
            for (i, name) in Action::KINDS.iter().enumerate() {
                ui.selectable_value(&mut kind, i, tr(name));
            }
        });
        if kind != handler.action.kind_index() {
            handler.action = Action::from_kind_index(kind);
        }
        ui.end_row();

        match &mut handler.action {
            Action::ChangeScreen { screen, anim, time_ms, delay_ms } => {
                label(ui, tr("Screen"), "");
                widgets::screen_ref(ui, ("screen", index), screen, project);
                ui.end_row();
                label(ui, tr("Animation"), "");
                widgets::choice(ui, ("anim", index), anim, ScreenAnim::ALL, ScreenAnim::label);
                ui.end_row();
                label(ui, tr("Duration"), "");
                ui.add(egui::DragValue::new(time_ms).range(0..=10000).suffix(" ms"));
                ui.end_row();
                label(ui, tr("Delay"), "");
                ui.add(egui::DragValue::new(delay_ms).range(0..=10000).suffix(" ms"));
                ui.end_row();
            }
            Action::CallFunction { name } => {
                label(ui, tr("Function"), tr("Declared in ui_events.h; implement it in ui_events.c."));
                ui.add(egui::TextEdit::singleline(name).font(egui::TextStyle::Monospace));
                ui.end_row();
            }
            Action::ModifyFlag { target, flag, op } => {
                target_row(ui, project, target, index);
                label(ui, tr("Flag"), "");
                widgets::choice(ui, ("flag", index), flag, ObjFlag::ALL, ObjFlag::label);
                ui.end_row();
                op_row(ui, op, index);
            }
            Action::ModifyState { target, state, op } => {
                target_row(ui, project, target, index);
                label(ui, tr("State"), "");
                widgets::choice(ui, ("state", index), state, ObjState::ALL, ObjState::label);
                ui.end_row();
                op_row(ui, op, index);
            }
            Action::SetText { target, text } => {
                target_row(ui, project, target, index);
                label(ui, tr("Text"), "");
                ui.text_edit_singleline(text);
                ui.end_row();
            }
            Action::SetValue { target, value } => {
                target_row(ui, project, target, index);
                label(ui, tr("Value"), "");
                ui.add(egui::DragValue::new(value));
                ui.end_row();
            }
        }
    });
}

fn target_row(ui: &mut egui::Ui, project: &Project, target: &mut Option<lvglab_model::WidgetId>, index: usize) {
    label(ui, tr("Target"), "");
    widgets::widget_ref(ui, ("target", index), target, project, None);
    ui.end_row();
}

fn op_row(ui: &mut egui::Ui, op: &mut ModifyOp, index: usize) {
    label(ui, tr("Operation"), "");
    widgets::choice(ui, ("op", index), op, ModifyOp::ALL, ModifyOp::label);
    ui.end_row();
}
