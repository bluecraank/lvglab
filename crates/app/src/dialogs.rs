//! Modal and floating dialogs: project settings, preferences, reports.

use eframe::egui;
use lvglab_model::Project;
use lvglab_model::project::{DisplaySettings, DisplayShape, LvglVersion};

use crate::app::Studio;
use crate::assistant::effort_label;
use crate::commands::Command;
use crate::i18n::{self, Language, tr};
use crate::modal::{self, Action, Tone};
use crate::settings::{Appearance, CanvasBackground, PreviewRenderer};
use crate::visuals;
use crate::widgets::{self, grid, label, muted, section};

/// Common display resolutions as quick presets.
const PRESETS: &[(&str, u32, u32, DisplayShape)] = &[
    ("240 × 240 round (GC9A01)", 240, 240, DisplayShape::Round),
    ("240 × 240", 240, 240, DisplayShape::Rectangle),
    ("320 × 240 (ILI9341)", 320, 240, DisplayShape::Rectangle),
    ("320 × 170 (ST7789)", 320, 170, DisplayShape::Rectangle),
    ("480 × 320 (ILI9488 / ST7796)", 480, 320, DisplayShape::Rectangle),
    ("466 × 466 round", 466, 466, DisplayShape::Round),
    ("800 × 480", 800, 480, DisplayShape::Rectangle),
];

pub fn show(ctx: &egui::Context, studio: &mut Studio) {
    new_project(ctx, studio);
    project_settings(ctx, studio);
    preferences(ctx, studio);
    about(ctx, studio);
    report(ctx, studio);
    confirm_discard(ctx, studio);
}

/// File → New: the settings a project needs from the start.
fn new_project(ctx: &egui::Context, studio: &mut Studio) {
    let Some(draft) = &mut studio.new_project else { return };
    let actions = [Action::primary(tr("Create")), Action::secondary(tr("Cancel"))];
    let message = tr("Choose the display and the LVGL release. You can change them later in the project settings.");
    let choice = modal::show(ctx, "new_project", Tone::Info, tr("New project"), message, &actions, |ui| {
        ui.add_space(14.0);
        grid(ui, "new_project_grid", |ui| {
            label(ui, tr("Name"), "");
            ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(240.0));
            ui.end_row();
            display_rows(ui, "new_project", &mut draft.display);
            label(
                ui,
                tr("LVGL version"),
                tr("LVGL 9 has no meter and no color wheel; the export reports what it changes."),
            );
            widgets::choice(ui, "new_lvgl_version", &mut draft.lvgl_version, LvglVersion::ALL, LvglVersion::label);
            ui.end_row();
            label(ui, tr("Theme"), "");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut draft.theme.dark, true, tr("Dark"));
                ui.selectable_value(&mut draft.theme.dark, false, tr("Light"));
            });
            ui.end_row();
        });
    });
    match choice {
        Some(0) => {
            let project = Box::new(draft.clone());
            studio.new_project = None;
            studio.push(Command::CreateProject(project));
        }
        Some(_) => studio.new_project = None,
        None => {}
    }
}

/// Preset, resolution and shape rows of a settings grid.
fn display_rows(ui: &mut egui::Ui, id: &str, display: &mut DisplaySettings) {
    label(ui, tr("Preset"), "");
    egui::ComboBox::from_id_salt((id, "preset")).selected_text(tr("Choose…")).width(220.0).show_ui(ui, |ui| {
        for (name, w, h, shape) in PRESETS {
            if ui.selectable_label(false, *name).clicked() {
                display.width = *w;
                display.height = *h;
                display.shape = *shape;
            }
        }
    });
    ui.end_row();
    label(ui, tr("Resolution"), tr("Width × height of the LVGL display in pixels."));
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut display.width).range(32..=2048).suffix(" px"));
        ui.label("×");
        ui.add(egui::DragValue::new(&mut display.height).range(32..=2048).suffix(" px"));
        if ui.small_button("↔").on_hover_text(tr("Swap width and height")).clicked() {
            std::mem::swap(&mut display.width, &mut display.height);
        }
    });
    ui.end_row();
    label(ui, tr("Shape"), "");
    ui.horizontal(|ui| {
        for shape in DisplayShape::ALL {
            ui.selectable_value(&mut display.shape, *shape, tr(shape.label()));
        }
    });
    ui.end_row();
}

fn project_settings(ctx: &egui::Context, studio: &mut Studio) {
    let mut open = studio.show_project_settings;
    let dir = studio.doc.dir().map(std::path::Path::to_path_buf);
    let version = studio.doc.project.lvgl_version;
    egui::Window::new(tr("Project settings"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            let before = studio.doc.project.display.clone();
            let p: &mut Project = &mut studio.doc.project;
            section(ui, "ps_general", tr("General"), true, |ui| {
                grid(ui, "ps_general_grid", |ui| {
                    label(ui, tr("Name"), "");
                    ui.text_edit_singleline(&mut p.name);
                    ui.end_row();
                    label(
                        ui,
                        tr("LVGL version"),
                        tr("The export uses the API of the selected release. LVGL 9 lacks the meter and the color wheel; the export reports what it changes."),
                    );
                    widgets::choice(ui, "lvgl_version", &mut p.lvgl_version, LvglVersion::ALL, LvglVersion::label);
                    ui.end_row();
                });
            });
            section(ui, "ps_display", tr("Display"), true, |ui| {
                grid(ui, "ps_display_grid", |ui| display_rows(ui, "ps", &mut p.display));
            });
            section(ui, "ps_export", tr("Library export"), true, |ui| {
                grid(ui, "ps_export_grid", |ui| {
                    label(
                        ui,
                        tr("Library name"),
                        tr("Folder and library name, e.g. ui_round. Use it in lib_ignore of platformio.ini."),
                    );
                    ui.add(egui::TextEdit::singleline(&mut p.export.library_name).hint_text("ui"));
                    ui.end_row();
                    label(
                        ui,
                        tr("Symbol prefix"),
                        tr("Prefix of all names and files (ui → ui.h, ui_init, ui_Main01). Libraries linked into the same firmware need different prefixes."),
                    );
                    ui.add(egui::TextEdit::singleline(&mut p.export.prefix).hint_text("ui").desired_width(120.0));
                    ui.end_row();
                    label(
                        ui,
                        tr("Multiple languages"),
                        tr("Wraps texts marked as translatable in _() of lv_i18n and generates ui_relocalize()."),
                    );
                    ui.checkbox(&mut p.export.multilang, tr("Translate with lv_i18n"));
                    ui.end_row();
                    if p.lvgl_version.is_v9() {
                        label(
                            ui,
                            tr("Byte swap"),
                            tr("LVGL 9 swaps the bytes in the flush callback of the display driver, e.g. with lv_draw_sw_rgb565_swap()."),
                        );
                        muted(ui, tr("In the display driver"));
                    } else {
                        label(ui, tr("Byte swap"), tr("Must match LV_COLOR_16_SWAP in lv_conf.h."));
                        ui.checkbox(&mut p.export.swap_bytes, "LV_COLOR_16_SWAP 1");
                    }
                    ui.end_row();
                    label(
                        ui,
                        tr("Export folder"),
                        tr("Relative to the project file, e.g. the lib folder of your PlatformIO project."),
                    );
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut p.export.directory);
                        if ui.small_button(tr("Choose…")).clicked() {
                            if let Some(folder) = crate::sandbox::FileDialog::new().pick_folder() {
                                p.export.directory = crate::panels::relative_path(&folder, dir.as_deref());
                            }
                        }
                    });
                    ui.end_row();
                });
                if dir.is_none() {
                    muted(ui, tr("Save the project first to export into a folder relative to it."));
                }
                muted(
                    ui,
                    if p.lvgl_version.is_v9() {
                        tr("Requirements in your lv_conf.h: the Montserrat fonts the UI uses.")
                    } else {
                        tr("Requirements in your lv_conf.h: LV_COLOR_DEPTH 16 and the Montserrat fonts the UI uses.")
                    },
                );
            });
            if p.display != before {
                studio.canvas.fit_pending = true;
            }
        });
    studio.show_project_settings = open;
    if studio.doc.project.lvgl_version != version {
        studio.remove_unsupported(true);
    }
}

fn preferences(ctx: &egui::Context, studio: &mut Studio) {
    let mut open = studio.show_preferences;
    let max_height = (ctx.screen_rect().height() - 140.0).max(300.0);
    egui::Window::new(tr("Preferences")).open(&mut open).resizable(false).collapsible(false).min_width(960.0).show(
        ctx,
        |ui| {
            let s = &mut studio.settings;
            egui::ScrollArea::vertical().max_height(max_height).show(ui, |ui| {
            ui.columns(2, |cols| {
            card(&mut cols[0], "⚙", tr("General"), |ui| {
                grid(ui, "prefs_general_grid", |ui| {
                    label(ui, tr("Language"), "");
                    ui.horizontal(|ui| {
                        for language in Language::ALL {
                            if ui.selectable_value(&mut s.language, language, language.native_name()).changed() {
                                i18n::set_language(language);
                            }
                        }
                    });
                    ui.end_row();
                    label(ui, tr("Appearance"), "");
                    ui.horizontal(|ui| {
                        for appearance in Appearance::ALL {
                            if ui.selectable_value(&mut s.appearance, appearance, tr(appearance.label())).changed() {
                                visuals::apply_appearance(ui.ctx(), appearance);
                            }
                        }
                    });
                    ui.end_row();
                    label(ui, tr("UI scale"), tr("Size of the whole editor interface."));
                    ui.horizontal(|ui| {
                        // Applied on release so the window does not rescale under the cursor.
                        let pending = ui.id().with("ui_scale_pending");
                        let current = (s.ui_scale * 100.0).round() as i32;
                        let mut percent = ui.data(|d| d.get_temp::<i32>(pending)).unwrap_or(current);
                        let slider = ui.add(egui::Slider::new(&mut percent, 60..=200).suffix(" %").step_by(10.0));
                        if slider.dragged() {
                            ui.data_mut(|d| d.insert_temp(pending, percent));
                        } else {
                            if slider.changed() || slider.drag_stopped() {
                                s.ui_scale = percent as f32 / 100.0;
                            }
                            ui.data_mut(|d| d.remove::<i32>(pending));
                        }
                        if ui.small_button("100 %").clicked() {
                            s.ui_scale = 1.0;
                        }
                    });
                    ui.end_row();
                    label(ui, tr("Start"), "");
                    ui.checkbox(&mut s.reopen_last, tr("Open the last project on start"));
                    ui.end_row();
                });
            });
            card(&mut cols[0], "🎨", tr("Canvas"), |ui| {
                grid(ui, "prefs_canvas_grid", |ui| {
                    label(ui, tr("Background"), "");
                    widgets::choice(
                        ui,
                        "canvas_bg",
                        &mut s.canvas_background,
                        &CanvasBackground::ALL,
                        CanvasBackground::label,
                    );
                    ui.end_row();
                    label(
                        ui,
                        tr("Preview"),
                        tr("LVGL runs the code of the export with the real library, so the canvas shows exactly what the device draws. Only for LVGL 8 projects; selection frames and the simulator still use the editor's drawing."),
                    );
                    widgets::choice(
                        ui,
                        "preview_renderer",
                        &mut s.preview_renderer,
                        &PreviewRenderer::ALL,
                        PreviewRenderer::label,
                    );
                    ui.end_row();
                    label(ui, tr("Grid size"), "");
                    ui.add(egui::DragValue::new(&mut s.grid_size).range(2..=200).suffix(" px"));
                    ui.end_row();
                    label(ui, tr("Grid"), "");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut s.show_grid, tr("Show"));
                        ui.checkbox(&mut s.snap_to_grid, tr("Snap to grid"));
                    });
                    ui.end_row();
                    label(ui, tr("Overlays"), "");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut s.show_bounds, tr("Outline all widgets"));
                        ui.checkbox(&mut s.show_hidden, tr("Outline hidden widgets"));
                        ui.checkbox(&mut s.show_size_label, tr("Show name and size of the selection"));
                    });
                    ui.end_row();
                    label(ui, tr("Shift + arrow keys"), tr("Distance for moving the selection with Shift."));
                    ui.add(egui::DragValue::new(&mut s.nudge_large).range(2..=200).suffix(" px"));
                    ui.end_row();
                });
            });
            card(&mut cols[1], "✏", tr("Editing"), |ui| {
                grid(ui, "prefs_editing_grid", |ui| {
                    label(ui, tr("Timeline length"), tr("Older steps are dropped."));
                    ui.add(egui::DragValue::new(&mut s.history_limit).range(20..=5000).suffix(tr(" steps")));
                    ui.end_row();
                    label(ui, tr("Autosave"), tr("Saves a saved project with unsaved changes after this time."));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut s.autosave_minutes).range(0..=120).suffix(" min"));
                        if s.autosave_minutes == 0 {
                            muted(ui, tr("off"));
                        }
                    });
                    ui.end_row();
                    label(ui, tr("Autosave target"), tr("Automatic saves appear highlighted in the timeline."));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut s.autosave_project, true, tr("Project file"));
                        ui.selectable_value(&mut s.autosave_project, false, tr("Backup copy (.backup.lvglab)"));
                    });
                    ui.end_row();
                    label(
                        ui,
                        tr("Keep the timeline"),
                        tr("Saves the timeline as <name>.timeline next to the project and restores it when the project is opened."),
                    );
                    ui.checkbox(&mut s.save_timeline, "");
                    ui.end_row();
                });
            });
            card(&mut cols[1], "💬", tr("AI assistant"), |ui| {
                grid(ui, "prefs_ai_grid", |ui| {
                    label(ui, tr("API key"), tr("Anthropic API key. Stored in the editor settings on this computer."));
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut s.ai_api_key).password(true).hint_text("sk-ant-…").desired_width(220.0));
                        ui.hyperlink_to(tr("Get an API key"), "https://console.anthropic.com/settings/keys");
                    });
                    ui.end_row();
                    label(ui, tr("Model"), tr("More capable models build better interfaces but cost more."));
                    egui::ComboBox::from_id_salt("ai_model")
                        .selected_text(lvglab_ai::MODELS.iter().find(|(id, _)| *id == s.ai_model).map_or("", |(_, n)| *n))
                        .show_ui(ui, |ui| {
                            for (id, name) in lvglab_ai::MODELS {
                                ui.selectable_value(&mut s.ai_model, (*id).to_owned(), *name);
                            }
                        });
                    ui.end_row();
                    label(ui, tr("Effort"), tr("How thoroughly the model works on a request; higher takes longer."));
                    egui::ComboBox::from_id_salt("ai_effort").selected_text(tr(effort_label(&s.ai_effort))).show_ui(ui, |ui| {
                        for effort in lvglab_ai::EFFORTS {
                            ui.selectable_value(&mut s.ai_effort, (*effort).to_owned(), tr(effort_label(effort)));
                        }
                    });
                    ui.end_row();
                });
                if s.ai_api_key.trim().is_empty() && std::env::var("ANTHROPIC_API_KEY").is_ok_and(|k| !k.is_empty()) {
                    muted(ui, tr("Using the key from the environment variable ANTHROPIC_API_KEY."));
                }
                muted(ui, tr("Requests are billed to your Anthropic account. The project description is sent with each request."));
            });
            card(&mut cols[1], "📤", tr("Export"), |ui| {
                ui.checkbox(&mut s.open_export_folder, tr("Open the folder after exporting"));
            });
            });
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                let recent = &mut studio.settings.recent;
                if ui.add_enabled(!recent.is_empty(), egui::Button::new(tr("Clear recent files"))).clicked() {
                    recent.clear();
                }
                if ui.button(tr("Reset layout")).clicked() {
                    studio.reset_layout = true;
                }
                if ui.button(tr("Reset all settings")).clicked() {
                    studio.settings.reset();
                    i18n::set_language(studio.settings.language);
                    visuals::apply_appearance(ui.ctx(), studio.settings.appearance);
                }
            });
        },
    );
    studio.show_preferences = open;
}

/// A category of the preferences: a framed card with a highlighted title.
fn card(ui: &mut egui::Ui, icon: &str, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).size(18.0).color(visuals::ACCENT));
                ui.label(egui::RichText::new(title).size(17.0).strong().color(visuals::ACCENT));
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(6.0);
            add(ui);
        });
    ui.add_space(10.0);
}

fn about(ctx: &egui::Context, studio: &mut Studio) {
    if !studio.show_about {
        return;
    }
    let title = format!("LVGLab {}", env!("CARGO_PKG_VERSION"));
    let actions = [Action::primary(tr("Close"))];
    let choice =
        modal::show(ctx, "about", Tone::Info, &title, tr("UI designer for LVGL 8 and 9 on ESP32."), &actions, |ui| {
            ui.add_space(10.0);
            muted(ui, tr("Imports SquareLine Studio projects. Not affiliated with LVGL."));
            ui.hyperlink(env!("CARGO_PKG_REPOSITORY"));
        });
    if choice.is_some() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        studio.show_about = false;
    }
}

fn report(ctx: &egui::Context, studio: &mut Studio) {
    let Some(report) = &studio.report else { return };
    let actions = [Action::primary(tr("Close")), Action::secondary(tr("Copy to clipboard"))];
    let choice = modal::show(ctx, "report", report.tone, &report.title, &report.message, &actions, |ui| {
        if !report.lines.is_empty() {
            modal::line_list(ui, &report.lines, report.tone);
        }
    });
    match choice {
        Some(1) => {
            let text = std::iter::once(report.message.clone()).chain(report.lines.iter().cloned()).collect::<Vec<_>>();
            ctx.copy_text(text.join("\n"));
            studio.notify(tr("Copied to clipboard"));
        }
        Some(_) => studio.report = None,
        None => {}
    }
}

fn confirm_discard(ctx: &egui::Context, studio: &mut Studio) {
    let Some(command) = studio.confirm_discard.clone() else { return };
    let actions = [Action::primary(tr("Save")), Action::danger(tr("Discard changes")), Action::secondary(tr("Cancel"))];
    let message = tr("The current project has unsaved changes. Save them before continuing?");
    match modal::show(ctx, "confirm_discard", Tone::Warning, tr("Unsaved changes"), message, &actions, |_| {}) {
        Some(0) => {
            studio.confirm_discard = None;
            studio.execute(Command::Save, false);
            if !studio.doc.is_dirty() {
                studio.execute(command, true);
            }
        }
        Some(1) => {
            studio.confirm_discard = None;
            studio.execute(command, true);
        }
        Some(_) => studio.confirm_discard = None,
        None => {}
    }
}
