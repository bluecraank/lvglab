//! File → Merge project: takes screens of another project over. The wizard
//! first asks which screens to take, then how to resolve names that already
//! exist in this project.

use std::path::PathBuf;

use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText};
use lvglab_model::merge::{self, Conflict, Resolution};
use lvglab_model::{Project, WidgetId};

use crate::app::{Report, Studio};
use crate::i18n::{tr, trf};
use crate::modal::Tone;
use crate::visuals::{self, ACCENT};

pub struct MergeWizard {
    pub source: Project,
    pub source_dir: Option<PathBuf>,
    pub file: String,
    /// Screens of the other project and whether they are taken over.
    pub selected: Vec<(WidgetId, bool)>,
    /// Set in the second step.
    pub conflicts: Option<Vec<Conflict>>,
}

impl MergeWizard {
    pub fn new(source: Project, path: &std::path::Path) -> Self {
        let selected = source.screens.iter().map(|s| (s.id, true)).collect();
        Self {
            source,
            source_dir: path.parent().map(std::path::Path::to_path_buf),
            file: path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            selected,
            conflicts: None,
        }
    }

    fn screens(&self) -> Vec<WidgetId> {
        self.selected.iter().filter(|(_, on)| *on).map(|(id, _)| *id).collect()
    }
}

enum Step {
    None,
    Next,
    Back,
    Cancel,
    Merge,
}

pub fn show(ctx: &egui::Context, studio: &mut Studio) {
    let target = &studio.doc.project;
    let Some(wizard) = &mut studio.merge else { return };
    let mut step = Step::None;
    let palette = visuals::palette(ctx);
    let frame = Frame::new()
        .fill(ctx.style().visuals.window_fill)
        .stroke(ctx.style().visuals.window_stroke)
        .corner_radius(14)
        .inner_margin(Margin::same(24));
    egui::Modal::new(egui::Id::new("merge_wizard")).frame(frame).show(ctx, |ui| {
        ui.set_width(620.0);
        ui.label(RichText::new(tr("Merge project")).size(18.0).strong());
        ui.label(RichText::new(&wizard.file).color(palette.muted));
        ui.add_space(12.0);
        let screens = wizard.screens();
        match &mut wizard.conflicts {
            None => {
                ui.label(tr("Choose the screens to take over. Components, fonts, images and theme colors they use come along."));
                ui.add_space(8.0);
                let (s, t) = (&wizard.source.display, &target.display);
                if (s.width, s.height) != (t.width, t.height) {
                    note(ui, &trf("The other project is {} × {} px, this one {} × {} px. Positions are kept.", &[&s.width, &s.height, &t.width, &t.height]));
                }
                if wizard.source.lvgl_version != target.lvgl_version {
                    note(
                        ui,
                        &trf(
                            "The other project targets LVGL {}, this one LVGL {}. What LVGL {} does not support is removed.",
                            &[&wizard.source.lvgl_version.suffix(), &target.lvgl_version.suffix(), &target.lvgl_version.suffix()],
                        ),
                    );
                }
                ui.horizontal(|ui| {
                    if ui.small_button(tr("All")).clicked() {
                        wizard.selected.iter_mut().for_each(|(_, on)| *on = true);
                    }
                    if ui.small_button(tr("None")).clicked() {
                        wizard.selected.iter_mut().for_each(|(_, on)| *on = false);
                    }
                });
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (id, on) in &mut wizard.selected {
                        let Some(screen) = wizard.source.widget(*id) else { continue };
                        let mut count = 0;
                        screen.walk(&mut |_| count += 1);
                        ui.horizontal(|ui| {
                            ui.checkbox(on, RichText::new(&screen.name).strong());
                            ui.label(RichText::new(trf("{} widgets", &[&(count - 1)])).small().color(palette.muted));
                        });
                    }
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        step = Step::Cancel;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let any = wizard.selected.iter().any(|(_, on)| *on);
                        if ui.add_enabled(any, primary(tr("Next"))).clicked() {
                            step = Step::Next;
                        }
                    });
                });
            }
            Some(conflicts) => {
                if conflicts.is_empty() {
                    ui.label(tr("No name conflicts: everything is taken over as it is."));
                } else {
                    ui.label(tr("These names already exist in this project. Rename the incoming entry or use the existing one."));
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        egui::Grid::new("merge_conflicts").num_columns(3).spacing([12.0, 8.0]).striped(true).show(ui, |ui| {
                            for (i, conflict) in conflicts.iter_mut().enumerate() {
                                ui.label(RichText::new(tr(conflict.kind.label())).small().color(palette.muted));
                                ui.label(RichText::new(&conflict.name).strong());
                                ui.horizontal(|ui| resolution(ui, i, conflict));
                                ui.end_row();
                            }
                        });
                    });
                }
                let problems = merge::problems(target, &wizard.source, &screens, conflicts);
                for problem in &problems {
                    ui.colored_label(visuals::WARNING, problem);
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        step = Step::Cancel;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_enabled(problems.is_empty(), primary(tr("Merge"))).clicked() {
                            step = Step::Merge;
                        }
                        if ui.button(tr("Back")).clicked() {
                            step = Step::Back;
                        }
                    });
                });
            }
        }
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        step = Step::Cancel;
    }
    match step {
        Step::None => {}
        Step::Cancel => studio.merge = None,
        Step::Back => wizard.conflicts = None,
        Step::Next => next_step(studio),
        Step::Merge => {
            if let Some(wizard) = studio.merge.take() {
                apply(studio, wizard);
            }
        }
    }
}

/// Goes to the conflicts, as the Next button does.
pub fn next_step(studio: &mut Studio) {
    let target_dir = studio.doc.dir().map(std::path::Path::to_path_buf);
    let Some(wizard) = &mut studio.merge else { return };
    let screens = wizard.screens();
    wizard.conflicts = Some(merge::conflicts(
        &studio.doc.project,
        target_dir.as_deref(),
        &wizard.source,
        wizard.source_dir.as_deref(),
        &screens,
    ));
}

/// Takes a copied screen over. Asks only if names clash.
pub fn paste_screen(studio: &mut Studio, clip: crate::commands::ScreenClip) {
    let mut wizard = MergeWizard::new(clip.project, std::path::Path::new(&clip.name));
    wizard.source_dir = clip.dir;
    wizard.file = clip.name;
    let screens = wizard.screens();
    let target_dir = studio.doc.dir().map(std::path::Path::to_path_buf);
    let conflicts = merge::conflicts(
        &studio.doc.project,
        target_dir.as_deref(),
        &wizard.source,
        wizard.source_dir.as_deref(),
        &screens,
    );
    let clean = conflicts.is_empty();
    wizard.conflicts = Some(conflicts);
    if clean {
        apply(studio, wizard);
    } else {
        studio.merge = Some(wizard);
    }
}

fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT).min_size(egui::vec2(96.0, 30.0))
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.colored_label(visuals::WARNING, format!("⚠ {text}"));
}

/// Rename or use the existing entry.
fn resolution(ui: &mut egui::Ui, index: usize, conflict: &mut Conflict) {
    let renaming = matches!(conflict.resolution, Resolution::Rename(_));
    if ui.radio(renaming, tr("Rename to")).clicked() && !renaming {
        conflict.resolution = Resolution::Rename(format!("{}_2", conflict.name));
    }
    if let Resolution::Rename(name) = &mut conflict.resolution {
        ui.add(egui::TextEdit::singleline(name).id_salt(("merge_name", index)).desired_width(150.0));
    }
    if conflict.can_use_existing {
        let label = if conflict.identical { tr("Use existing (identical)") } else { tr("Use existing") };
        if ui.radio(!renaming, label).clicked() {
            conflict.resolution = Resolution::UseExisting;
        }
    }
}

fn apply(studio: &mut Studio, wizard: MergeWizard) {
    let screens = wizard.screens();
    let conflicts = wizard.conflicts.unwrap_or_default();
    let merged =
        merge::merge(&mut studio.doc.project, &wizard.source, wizard.source_dir.as_deref(), &screens, &conflicts);
    studio.remove_unsupported(false);
    let label = trf("Merged {}", &[&wizard.file]);
    studio.timeline.record(&studio.doc.project, label.clone());
    if let Some(first) = merged.screens.first() {
        studio.select_screen(*first);
    }
    studio.notify(trf("{} screens merged from {}", &[&merged.screens.len(), &wizard.file]));
    if merged.cleared_references > 0 && studio.report.is_none() {
        studio.report = Some(Report {
            tone: Tone::Info,
            title: label,
            message: trf(
                "{} references pointed at widgets that were not taken over and were cleared (events, keyboard targets).",
                &[&merged.cleared_references],
            ),
            lines: Vec::new(),
        });
    }
}
