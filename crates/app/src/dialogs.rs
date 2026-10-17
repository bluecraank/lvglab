//! Modal and floating dialogs: project settings, preferences, reports.

use eframe::egui;
use lvglab_model::color_theme::{ColorTheme, ThemeColor};
use lvglab_model::project::{DisplayShape, LvglVersion};
use lvglab_model::{Palette, Project};

use crate::app::Studio;
use crate::assistant::effort_label;
use crate::commands::Command;
use crate::i18n::{self, Language, tr};
use crate::modal::{self, Action, Tone};
use crate::settings::{Appearance, CanvasBackground};
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
    project_settings(ctx, studio);
    preferences(ctx, studio);
    about(ctx, studio);
    report(ctx, studio);
    confirm_discard(ctx, studio);
}

fn project_settings(ctx: &egui::Context, studio: &mut Studio) {
    let mut open = studio.show_project_settings;
    let dir = studio.doc.dir().map(std::path::Path::to_path_buf);
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
                    label(ui, tr("LVGL version"), tr("The generated code targets LVGL 8.3."));
                    widgets::choice(ui, "lvgl_version", &mut p.lvgl_version, LvglVersion::ALL, LvglVersion::label);
                    ui.end_row();
                });
            });
            section(ui, "ps_display", tr("Display"), true, |ui| {
                grid(ui, "ps_display_grid", |ui| {
                    label(ui, tr("Preset"), "");
                    egui::ComboBox::from_id_salt("preset").selected_text(tr("Choose…")).width(220.0).show_ui(
                        ui,
                        |ui| {
                            for (name, w, h, shape) in PRESETS {
                                if ui.selectable_label(false, *name).clicked() {
                                    p.display.width = *w;
                                    p.display.height = *h;
                                    p.display.shape = *shape;
                                }
                            }
                        },
                    );
                    ui.end_row();
                    label(ui, tr("Resolution"), tr("Width × height of the LVGL display in pixels."));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut p.display.width).range(32..=2048).suffix(" px"));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut p.display.height).range(32..=2048).suffix(" px"));
                        if ui.small_button("↔").on_hover_text(tr("Swap width and height")).clicked() {
                            std::mem::swap(&mut p.display.width, &mut p.display.height);
                        }
                    });
                    ui.end_row();
                    label(ui, tr("Shape"), "");
                    ui.horizontal(|ui| {
                        for shape in DisplayShape::ALL {
                            ui.selectable_value(&mut p.display.shape, *shape, tr(shape.label()));
                        }
                    });
                    ui.end_row();
                });
            });
            section(ui, "ps_theme", tr("Theme (lv_theme_default)"), true, |ui| {
                grid(ui, "ps_theme_grid", |ui| {
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
                    label(ui, tr("Byte swap"), tr("Must match LV_COLOR_16_SWAP in lv_conf.h (TFT_eSPI usually uses 1)."));
                    ui.checkbox(&mut p.export.swap_bytes, "LV_COLOR_16_SWAP 1");
                    ui.end_row();
                    label(
                        ui,
                        tr("Export folder"),
                        tr("Relative to the project file, e.g. the lib folder of your PlatformIO project."),
                    );
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut p.export.directory);
                        if ui.small_button(tr("Choose…")).clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
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
                    tr("Requirements in your lv_conf.h: LV_COLOR_DEPTH 16 and the Montserrat fonts the UI uses."),
                );
            });
            section(ui, "ps_color_themes", tr("Color themes"), false, |ui| color_themes(ui, p));
            if p.display != before {
                studio.canvas.fit_pending = true;
            }
        });
    studio.show_project_settings = open;
}

/// Theme colors: variables with one value per theme. Styles bound to a
/// variable follow `ui_theme_set()` at runtime.
fn color_themes(ui: &mut egui::Ui, p: &mut Project) {
    let themes = &mut p.color_themes;
    if themes.themes.is_empty() {
        themes.themes.push(ColorTheme { name: "Default".to_owned(), values: Vec::new() });
    }
    themes.normalize();
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
    grid(ui, "ps_theme_colors", |ui| {
        for index in 0..themes.variables.len() {
            let mut name = themes.variables[index].clone();
            if ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0)).changed() {
                rename = Some((themes.variables[index].clone(), name));
            }
            let value = &mut themes.themes[active].values[index];
            ui.horizontal(|ui| {
                widgets::color(ui, &mut value.color);
                ui.add(egui::DragValue::new(&mut value.opa).range(0..=255).prefix("α "));
                if ui.small_button("✕").on_hover_text(tr("Remove color")).clicked() {
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
    if let Some(index) = remove {
        themes.remove_variable(index);
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

fn preferences(ctx: &egui::Context, studio: &mut Studio) {
    let mut open = studio.show_preferences;
    egui::Window::new(tr("Preferences")).open(&mut open).resizable(false).collapsible(false).default_width(460.0).show(
        ctx,
        |ui| {
            let s = &mut studio.settings;
            section(ui, "prefs_general", tr("General"), true, |ui| {
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
            section(ui, "prefs_canvas", tr("Canvas"), true, |ui| {
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
            section(ui, "prefs_editing", tr("Editing"), true, |ui| {
                grid(ui, "prefs_editing_grid", |ui| {
                    label(ui, tr("Timeline length"), tr("Older steps are dropped."));
                    ui.add(egui::DragValue::new(&mut s.history_limit).range(20..=5000).suffix(tr(" steps")));
                    ui.end_row();
                    label(ui, tr("Automatic backup"), tr("Writes <name>.backup.lvglab next to a saved project."));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut s.autosave_minutes).range(0..=120).suffix(" min"));
                        if s.autosave_minutes == 0 {
                            muted(ui, tr("off"));
                        }
                    });
                    ui.end_row();
                });
            });
            section(ui, "prefs_ai", tr("AI assistant"), true, |ui| {
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
            section(ui, "prefs_export", tr("Export"), true, |ui| {
                ui.checkbox(&mut s.open_export_folder, tr("Open the folder after exporting"));
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

fn about(ctx: &egui::Context, studio: &mut Studio) {
    if !studio.show_about {
        return;
    }
    let title = format!("LVGLab {}", env!("CARGO_PKG_VERSION"));
    let actions = [Action::primary(tr("Close"))];
    let choice = modal::show(
        ctx,
        "about",
        Tone::Info,
        &title,
        tr("UI designer for LVGL 8 on ESP32 with TFT_eSPI."),
        &actions,
        |ui| {
            ui.add_space(10.0);
            muted(ui, tr("Imports SquareLine Studio projects. Not affiliated with LVGL or SquareLine."));
            ui.hyperlink(env!("CARGO_PKG_REPOSITORY"));
        },
    );
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
