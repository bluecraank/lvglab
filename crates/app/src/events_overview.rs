//! A window with every event of the project: which widget reacts to what and
//! what happens. A click on a row selects the widget.

use eframe::egui::{self, RichText};
use lvglab_model::event::{Action, EventTrigger, ModifyOp};
use lvglab_model::flags::{ObjFlag, ObjState};
use lvglab_model::{Project, Widget, WidgetId};

use crate::app::Studio;
use crate::i18n::{tr, trf};
use crate::visuals::{self, ACCENT};

/// The open window and its filter text.
#[derive(Default)]
pub struct EventsOverview {
    pub filter: String,
}

struct Row {
    root: WidgetId,
    widget: WidgetId,
    widget_name: String,
    trigger: String,
    action: String,
    detail: String,
    /// The action points at nothing (no target or screen chosen).
    incomplete: bool,
}

fn target_name(project: &Project, id: Option<WidgetId>) -> Option<String> {
    id.and_then(|id| project.widget(id)).map(|w| w.name.clone())
}

fn op_label(op: ModifyOp) -> &'static str {
    tr(op.label())
}

/// What the action does, and whether it is complete.
fn describe(project: &Project, action: &Action) -> (String, bool) {
    let missing = || tr("(no target)").to_owned();
    match action {
        Action::ChangeScreen { screen, time_ms, delay_ms, .. } => {
            let name = target_name(project, *screen);
            let mut text = format!("{} {}", crate::icons::ARROW_RIGHT, name.clone().unwrap_or_else(missing));
            if *time_ms > 0 || *delay_ms > 0 {
                text.push_str(&format!(" · {time_ms} ms"));
                if *delay_ms > 0 {
                    text.push_str(&format!(" + {delay_ms} ms"));
                }
            }
            (text, name.is_none())
        }
        Action::CallFunction { name } => (format!("{name}(e)"), name.trim().is_empty()),
        Action::ModifyFlag { target, flag, op } => {
            let name = target_name(project, *target);
            let text =
                format!("{} {} · {}", op_label(*op), tr(ObjFlag::label(*flag)), name.clone().unwrap_or_else(missing));
            (text, name.is_none())
        }
        Action::ModifyState { target, state, op } => {
            let name = target_name(project, *target);
            let text =
                format!("{} {} · {}", op_label(*op), tr(ObjState::label(*state)), name.clone().unwrap_or_else(missing));
            (text, name.is_none())
        }
        Action::SetText { target, text } => {
            let name = target_name(project, *target);
            (format!("{} = \"{}\"", name.clone().unwrap_or_else(missing), text.replace('\n', " ")), name.is_none())
        }
        Action::SetValue { target, value } => {
            let name = target_name(project, *target);
            (format!("{} = {value}", name.clone().unwrap_or_else(missing)), name.is_none())
        }
    }
}

fn rows(project: &Project) -> Vec<(String, bool, Vec<Row>)> {
    let mut groups = Vec::new();
    let roots = project.screens.iter().map(|s| (s, false)).chain(project.components.iter().map(|c| (c, true)));
    for (root, is_component) in roots {
        let mut list = Vec::new();
        root.walk(&mut |w: &Widget| {
            for handler in &w.events {
                let (detail, incomplete) = describe(project, &handler.action);
                list.push(Row {
                    root: root.id,
                    widget: w.id,
                    widget_name: w.name.clone(),
                    trigger: tr(EventTrigger::label(handler.trigger)).to_owned(),
                    action: tr(Action::KINDS[handler.action.kind_index()]).to_owned(),
                    detail,
                    incomplete,
                });
            }
        });
        if !list.is_empty() {
            groups.push((root.name.clone(), is_component, list));
        }
    }
    groups
}

pub fn show(ctx: &egui::Context, studio: &mut Studio) {
    let Some(state) = &mut studio.events_overview else { return };
    let groups = rows(&studio.doc.project);
    let total: usize = groups.iter().map(|(_, _, r)| r.len()).sum();
    let needle = state.filter.trim().to_lowercase();
    let palette = visuals::palette(ctx);
    let mut open = true;
    let mut chosen: Option<(WidgetId, WidgetId)> = None;
    egui::Window::new(trf("All events ({})", &[&total]))
        .id(egui::Id::new("events_overview"))
        .open(&mut open)
        .default_width(820.0)
        .default_height(480.0)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .hint_text(tr("Filter by widget, trigger, action or target…"))
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            if total == 0 {
                ui.label(RichText::new(tr("This project has no events yet.")).color(palette.muted));
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (group, is_component, list) in &groups {
                    let matching: Vec<&Row> = list
                        .iter()
                        .filter(|r| {
                            needle.is_empty()
                                || format!("{} {} {} {} {group}", r.widget_name, r.trigger, r.action, r.detail)
                                    .to_lowercase()
                                    .contains(&needle)
                        })
                        .collect();
                    if matching.is_empty() {
                        continue;
                    }
                    let icon = if *is_component { crate::icons::PACKAGE } else { crate::icons::MONITOR };
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("{icon} {group}")).strong().color(ACCENT));
                    egui::Grid::new(("events_grid", group.as_str()))
                        .num_columns(4)
                        .striped(true)
                        .spacing([18.0, 6.0])
                        .min_col_width(150.0)
                        .show(ui, |ui| {
                            for header in [tr("Widget"), tr("Trigger"), tr("Action"), tr("Details")] {
                                ui.label(RichText::new(header).small().color(palette.muted));
                            }
                            ui.end_row();
                            for row in matching {
                                let name = ui
                                    .add(
                                        egui::Label::new(RichText::new(&row.widget_name).strong())
                                            .sense(egui::Sense::click()),
                                    )
                                    .on_hover_text(tr("Select the widget"));
                                ui.label(&row.trigger);
                                ui.label(&row.action);
                                let detail = if row.incomplete {
                                    RichText::new(format!("{} {}", crate::icons::WARNING, row.detail))
                                        .color(visuals::WARNING)
                                } else {
                                    RichText::new(&row.detail)
                                };
                                let detail = ui.add(egui::Label::new(detail).sense(egui::Sense::click()));
                                if name.clicked() || detail.clicked() {
                                    chosen = Some((row.root, row.widget));
                                }
                                ui.end_row();
                            }
                        });
                }
            });
        });
    if let Some((root, widget)) = chosen {
        studio.select_screen(root);
        studio.select(Some(widget));
        studio.canvas.overview = false;
    }
    if !open {
        studio.events_overview = None;
    }
}
