//! Application frame: studio state, docking layout, menus and shortcuts.

use std::path::PathBuf;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, ViewportCommand};
use egui_dock::{DockArea, DockState, NodeIndex, TabIndex, TabViewer};
use lvglab_model::compat::Removed;
use lvglab_model::flags::StateMask;
use lvglab_model::{Project, Widget, WidgetId};

use crate::LaunchOptions;
use crate::canvas::CanvasState;
use crate::canvas::assets::AssetCache;
use crate::commands::Command;
use crate::dialogs;
use crate::document::{Document, Timeline};
use crate::i18n::{self, tr, trf};
use crate::modal::{self, Action, Tone};
use crate::panels::{self, Panel};
use crate::settings::{AppSettings, STORAGE_KEY};
use crate::visuals;

const DOCK_KEY: &str = "lvglab.dock.v3";
/// Layouts saved before the Themes tab existed.
const DOCK_KEY_V2: &str = "lvglab.dock.v2";

/// A list of messages shown in a dialog (import/export results, errors).
pub struct Report {
    pub tone: crate::modal::Tone,
    pub title: String,
    pub message: String,
    pub lines: Vec<String>,
}

/// Everything the panels work on.
pub struct Studio {
    pub doc: Document,
    pub timeline: Timeline,
    pub selected: Option<WidgetId>,
    /// The screen shown on the canvas.
    pub screen: WidgetId,
    pub canvas: CanvasState,
    pub style_slot: String,
    pub style_state: StateMask,
    pub clipboard: Option<Widget>,
    /// A copied screen with what it needs (components, fonts, images).
    pub screen_clip: Option<crate::commands::ScreenClip>,
    /// The parked projects of the tabs; the entry of the active tab is `None`.
    pub tabs: Vec<Option<crate::tabs::Workspace>>,
    pub active_tab: usize,
    /// The widget whose look *Copy properties* took.
    pub props_clipboard: Option<Widget>,
    /// When this editor last copied; another editor's later copy wins on paste.
    pub clipboard_time: Option<std::time::SystemTime>,
    pub commands: Vec<Command>,
    pub status: Option<String>,
    pub report: Option<Report>,
    pub confirm_discard: Option<Command>,
    /// The open "New project" dialog.
    pub new_project: Option<crate::commands::NewProject>,
    /// Current step of the welcome tour.
    pub tour: Option<usize>,
    /// The tour starts once the start page and the dialogs are closed.
    pub tour_pending: bool,
    /// The open start page with the recent projects.
    pub start_page: Option<Vec<crate::start::Entry>>,
    /// The open merge wizard.
    pub merge: Option<crate::merge_dialog::MergeWizard>,
    /// The timeline shows only the steps that changed the current screen.
    pub timeline_this_screen: bool,
    /// The picture of the LVGL preview.
    pub lvgl_preview: crate::lvgl_preview::LvglPreview,
    /// The open search window (Ctrl+F).
    pub search: Option<crate::search_dialog::SearchState>,
    /// A running screenshot export.
    pub shots: Option<crate::screenshots::ShotJob>,
    /// Close the window as soon as possible (command line export done).
    pub exit_requested: bool,
    /// An action of the start page runs; show the page again unless it
    /// opens or creates a project.
    pub return_to_start: bool,
    /// Where the areas the tour points at were drawn this frame.
    pub anchors: crate::tour::Anchors,
    pub settings: AppSettings,
    pub assets: AssetCache,
    pub show_project_settings: bool,
    pub show_preferences: bool,
    pub show_about: bool,
    pub layer_filter: String,
    pub palette_filter: String,
    /// Set by the preferences to restore the default panel layout.
    pub reset_layout: bool,
    pub assistant: crate::assistant::Assistant,
    /// Running simulation of the project on the canvas.
    pub simulation: Option<crate::simulator::Simulation>,
    /// Image header cache of the memory estimate.
    pub stats_cache: crate::stats::StatsCache,
    /// Further selected widgets (Ctrl+click), for aligning.
    pub extra_selection: Vec<WidgetId>,
    /// GIF recording of the simulation.
    pub recorder: Option<crate::recorder::Recorder>,
    /// GIFs being encoded in the background.
    pub gif_jobs: Vec<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
}

impl Studio {
    /// The selected widget followed by the widgets added with Ctrl+click.
    pub fn selection(&self) -> Vec<WidgetId> {
        let mut ids: Vec<WidgetId> = self.selected.into_iter().collect();
        for id in &self.extra_selection {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        ids
    }

    /// Adds `id` to the selection or removes it again (Ctrl+click).
    pub fn toggle_in_selection(&mut self, id: WidgetId) {
        if self.selected.is_none() {
            self.select(Some(id));
        } else if self.selected == Some(id) {
            let next = self.extra_selection.first().copied();
            let rest: Vec<WidgetId> = self.extra_selection.iter().skip(1).copied().collect();
            self.select(next);
            self.extra_selection = rest;
        } else if let Some(index) = self.extra_selection.iter().position(|e| *e == id) {
            self.extra_selection.remove(index);
        } else {
            self.extra_selection.push(id);
        }
    }

    /// Starts recording the simulated display into a GIF.
    pub fn start_recording(&mut self, now: f64) {
        if self.simulation.is_some() {
            self.recorder = Some(crate::recorder::Recorder::new(now));
        }
    }

    /// Stops the recording and asks where to save the GIF.
    pub fn stop_recording(&mut self) {
        let Some(recorder) = self.recorder.take() else { return };
        let name = format!("{}.gif", self.doc.project.name);
        let dialog = crate::sandbox::FileDialog::new().add_filter("GIF", &["gif"]).set_file_name(name);
        if let Some(path) = dialog.save_file() {
            self.gif_jobs.push(recorder.finish(path));
            self.notify(tr("Writing the GIF…"));
        }
    }

    fn poll_gif_jobs(&mut self) {
        let mut done = Vec::new();
        self.gif_jobs.retain(|job| match job.try_recv() {
            Ok(result) => {
                done.push(result);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for result in done {
            match result {
                Ok(path) => self.notify(crate::i18n::trf("GIF saved to {}", &[&path.display()])),
                Err(err) => self.show_error(tr("Recording failed"), err),
            }
        }
    }

    /// Starts the simulation on the current screen or stops it.
    pub fn toggle_simulation(&mut self) {
        if self.simulation.is_some() {
            self.stop_recording();
        }
        self.simulation = match self.simulation {
            Some(_) => None,
            None => Some(crate::simulator::Simulation::new(&self.doc.project, self.screen)),
        };
    }

    pub(crate) fn new(settings: AppSettings) -> Self {
        let project = Project::new("MyProject");
        let screen = project.screens[0].id;
        Self {
            timeline: Timeline::new(&project, tr("New project")),
            doc: Document::new(project, None),
            selected: Some(screen),
            screen,
            canvas: CanvasState::default(),
            style_slot: "main".to_owned(),
            style_state: StateMask::DEFAULT,
            clipboard: None,
            screen_clip: None,
            tabs: vec![None],
            active_tab: 0,
            props_clipboard: None,
            clipboard_time: None,
            commands: Vec::new(),
            status: None,
            report: None,
            confirm_discard: None,
            new_project: None,
            tour: None,
            tour_pending: false,
            start_page: None,
            merge: None,
            search: None,
            lvgl_preview: Default::default(),
            timeline_this_screen: false,
            shots: None,
            exit_requested: false,
            return_to_start: false,
            anchors: crate::tour::Anchors::new(),
            settings,
            assets: AssetCache::default(),
            show_project_settings: false,
            show_preferences: false,
            show_about: false,
            layer_filter: String::new(),
            palette_filter: String::new(),
            reset_layout: false,
            assistant: crate::assistant::Assistant::default(),
            simulation: None,
            recorder: None,
            extra_selection: Vec::new(),
            stats_cache: crate::stats::StatsCache::default(),
            gif_jobs: Vec::new(),
        }
    }

    pub fn push(&mut self, command: Command) {
        self.commands.push(command);
    }

    pub fn notify(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    pub fn project(&self) -> &Project {
        &self.doc.project
    }

    pub fn selected_widget(&self) -> Option<&Widget> {
        self.selected.and_then(|id| self.doc.project.widget(id))
    }

    /// Selects a widget and shows its screen on the canvas.
    pub fn select(&mut self, id: Option<WidgetId>) {
        self.selected = id;
        self.extra_selection.clear();
        if let Some(screen) = id.and_then(|id| self.doc.project.screen_of(id)) {
            if screen.id != self.screen {
                self.screen = screen.id;
            }
        }
        let has_slot = self.selected_widget().is_some_and(|w| w.def().slot(&self.style_slot).is_some());
        if !has_slot {
            self.style_slot = "main".to_owned();
        }
    }

    pub fn select_screen(&mut self, id: WidgetId) {
        self.screen = id;
        self.select(Some(id));
    }

    /// Removes what the project's LVGL release does not support and lists it
    /// in a report. `switched` says the release was just changed.
    pub fn remove_unsupported(&mut self, switched: bool) {
        let removed = self.doc.project.remove_unsupported();
        if removed.is_empty() {
            return;
        }
        if self.selected.is_some_and(|id| self.doc.project.widget(id).is_none()) {
            self.select(None);
        }
        let version = self.doc.project.lvgl_version.suffix();
        let lines: Vec<String> = removed
            .iter()
            .map(|r| match r {
                Removed::Widget { name, kind } => trf("{}: widget removed ({})", &[name, &tr(kind.def().name)]),
                Removed::Prop { widget, label } => trf("{}: {} reset", &[widget, &tr(label)]),
                Removed::Style { widget, prop } => trf("{}: style {} removed", &[widget, &prop.name()]),
                Removed::Part { widget, label } => trf("{}: styles of the part {} removed", &[widget, &tr(label)]),
            })
            .collect();
        let title = if switched { trf("Switched to LVGL {}", &[&version]) } else { trf("LVGL {}", &[&version]) };
        self.notify(trf("{} unsupported entries removed", &[&lines.len()]));
        self.report = Some(Report {
            tone: Tone::Warning,
            title,
            message: trf("LVGL {} does not support the following, so it was removed. Undo restores it.", &[&version]),
            lines,
        });
    }
}

/// Memory estimate and widget counts in the toolbar.
fn stats_label(ui: &mut egui::Ui, studio: &mut Studio) {
    use crate::stats::format_bytes;
    let dir = studio.doc.dir().map(std::path::Path::to_path_buf);
    let stats = crate::stats::compute(&studio.doc.project, dir.as_deref(), studio.screen, &mut studio.stats_cache);
    let text = crate::i18n::trf(
        "RAM ~{} · Flash ~{} · {} / {} widgets",
        &[&format_bytes(stats.ram), &format_bytes(stats.flash()), &stats.widgets_screen, &stats.widgets_total],
    );
    let details = crate::i18n::trf(
        "Estimate of the exported UI.
RAM after ui_init(): {} (LVGL objects and local styles; temporary screens count once).
Flash: {} images, {} fonts, {} code.
Widgets: {} on this screen, {} on all screens.
Not included: display buffers, LVGL and the rest of the firmware.",
        &[
            &format_bytes(stats.ram),
            &format_bytes(stats.flash_images),
            &format_bytes(stats.flash_fonts),
            &format_bytes(stats.flash_code),
            &stats.widgets_screen,
            &stats.widgets_total,
        ],
    );
    crate::widgets::muted(ui, text).on_hover_text(details);
}

pub struct StudioApp {
    studio: Studio,
    dock: DockState<Panel>,
    screenshot: Option<PathBuf>,
    frame: u64,
    last_typing: f64,
    close_confirmed: bool,
    show_close_confirm: bool,
    title: String,
    last_autosave: f64,
    /// The selection of the last frame; text fields forget their undo history when it changes.
    last_selection: Option<WidgetId>,
}

impl StudioApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: LaunchOptions) -> Self {
        visuals::install(&cc.egui_ctx);
        let mut settings: AppSettings = cc.storage.and_then(|s| eframe::get_value(s, STORAGE_KEY)).unwrap_or_default();
        settings.sanitize();
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        cc.egui_ctx.set_zoom_factor(settings.ui_scale);
        let mut dock = cc
            .storage
            .and_then(|s| {
                eframe::get_value::<DockState<Panel>>(s, DOCK_KEY).or_else(|| {
                    eframe::get_value::<DockState<Panel>>(s, DOCK_KEY_V2).map(|mut dock| {
                        add_themes_tab(&mut dock);
                        dock
                    })
                })
            })
            .filter(|d| d.find_tab(&Panel::Canvas).is_some())
            .unwrap_or_else(default_layout);
        i18n::set_language(settings.language);
        visuals::apply_appearance(&cc.egui_ctx, settings.appearance);
        if let Some(name) = options.panel.as_deref() {
            let panel = Panel::ALL
                .iter()
                .find(|p| p.title().eq_ignore_ascii_case(name) || format!("{p:?}").eq_ignore_ascii_case(name));
            if let Some(location) = panel.and_then(|p| dock.find_tab(p)) {
                dock.set_active_tab(location);
            }
        }
        let reopen = settings.reopen_last.then(|| settings.recent.first().cloned()).flatten().filter(|p| p.exists());
        match options.language.as_deref() {
            Some("en") => settings.language = i18n::Language::English,
            Some("de") => settings.language = i18n::Language::German,
            Some("es") => settings.language = i18n::Language::Spanish,
            Some("fr") => settings.language = i18n::Language::French,
            _ => {}
        }
        match options.appearance.as_deref() {
            Some("dark") => settings.appearance = crate::settings::Appearance::Dark,
            Some("light") => settings.appearance = crate::settings::Appearance::Light,
            _ => {}
        }
        i18n::set_language(settings.language);
        visuals::apply_appearance(&cc.egui_ctx, settings.appearance);
        match options.renderer.as_deref() {
            Some("lvgl") => settings.preview_renderer = crate::settings::PreviewRenderer::Lvgl,
            Some("editor") => settings.preview_renderer = crate::settings::PreviewRenderer::Editor,
            _ => {}
        }
        let mut studio = Studio::new(settings);
        let dialog = options.dialog.as_deref();
        let files = if options.files.is_empty() { reopen.into_iter().collect() } else { options.files.clone() };
        match files.first().cloned() {
            Some(file) => {
                studio.execute(Command::OpenPath(file), true);
                // More projects open in further tabs.
                for file in files.into_iter().skip(1) {
                    studio.execute(Command::OpenPath(file), true);
                }
            }
            // Without a project the editor starts with the start page.
            None if (dialog.is_none() && options.screenshot.is_none()) || dialog == Some("start") => {
                studio.start_page = Some(crate::start::entries(&studio.settings.recent));
            }
            None => {}
        }
        if let Some(name) = options.select {
            let id = studio.doc.project.all_widgets().iter().find(|w| w.name == name).map(|w| w.id);
            studio.select(id);
            studio.report = None;
        }
        if options.simulate {
            studio.toggle_simulation();
            if let Some(sim) = &mut studio.simulation {
                sim.record = true;
            }
        }
        studio.show_preferences = dialog == Some("preferences");
        studio.show_about = dialog == Some("about");
        if dialog == Some("discard") {
            studio.confirm_discard = Some(Command::NewProject);
        }
        if dialog == Some("new") {
            studio.execute(Command::NewProject, true);
        }
        if let Some(query) = dialog.and_then(|d| d.strip_prefix("search:")) {
            studio.search = Some(crate::search_dialog::SearchState { query: query.to_owned(), ..Default::default() });
        }
        studio.canvas.overview = dialog == Some("overview");
        if options.randomize {
            studio.randomize(true);
        }
        if let Some(dir) = options.export_screenshots.clone() {
            studio.start_screenshots(dir, true);
        }
        // `merge:<file>` opens the merge wizard, `merge-conflicts:<file>` its second step.
        if let Some((step, file)) = dialog.and_then(|d| d.split_once(':')).filter(|(s, _)| s.starts_with("merge")) {
            studio.execute(Command::MergeProjectPath(PathBuf::from(file)), true);
            if step == "merge-conflicts" {
                crate::merge_dialog::next_step(&mut studio);
            }
        }
        if let Some(step) = dialog.and_then(|d| d.strip_prefix("tour")) {
            studio.tour = Some(step.trim_start_matches(':').parse::<usize>().map_or(0, |s| s.saturating_sub(1)));
        } else if dialog.is_none()
            && options.screenshot.is_none()
            && options.export_screenshots.is_none()
            && !studio.settings.tour_done
        {
            studio.tour_pending = true;
        }
        Self {
            studio,
            dock,
            screenshot: options.screenshot,
            frame: 0,
            last_typing: 0.0,
            close_confirmed: false,
            show_close_confirm: dialog == Some("close"),
            title: String::new(),
            last_autosave: 0.0,
            last_selection: None,
        }
    }

    /// Applies preferences that live outside the studio state.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        let settings = &self.studio.settings;
        if (ctx.zoom_factor() - settings.ui_scale).abs() > f32::EPSILON {
            ctx.set_zoom_factor(settings.ui_scale);
        }
        self.studio.timeline.set_limit(settings.history_limit);
        if std::mem::take(&mut self.studio.reset_layout) {
            self.dock = default_layout();
        }
    }

    /// Saves a project with unsaved changes periodically: the project file
    /// itself (marked in the timeline) or `<name>.backup.lvglab` next to it.
    fn autosave(&mut self, ctx: &egui::Context) {
        let minutes = self.studio.settings.autosave_minutes;
        let now = ctx.input(|i| i.time);
        if minutes == 0 || !self.studio.doc.is_dirty() || now - self.last_autosave < f64::from(minutes) * 60.0 {
            return;
        }
        // Wait until the current interaction is finished.
        if self.studio.canvas.is_dragging() || ctx.wants_keyboard_input() || ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        self.last_autosave = now;
        let Some(path) = self.studio.doc.path.clone() else { return };
        if self.studio.settings.autosave_project {
            self.studio.save_to(path, crate::document::EntryKind::Autosaved);
            return;
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let backup = path.with_file_name(format!("{stem}.backup.{}", lvglab_model::project::PROJECT_EXTENSION));
        match self.studio.doc.project.save(&backup) {
            Ok(()) => self.studio.notify(crate::i18n::trf("Backup saved to {}", &[&backup.display()])),
            Err(err) => self.studio.notify(err.to_string()),
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                self.file_menu(ui);
                self.edit_menu(ui);
                self.view_menu(ui);
                ui.menu_button(tr("Help"), |ui| {
                    if ui.button(tr("Welcome tour")).clicked() {
                        self.studio.tour = Some(0);
                        ui.close_menu();
                    }
                    if ui.button(tr("About LVGLab")).clicked() {
                        self.studio.show_about = true;
                        ui.close_menu();
                    }
                });
                ui.separator();
                // As small as the menu entries, so it fits into the menu bar.
                let export = egui::Button::new(egui::RichText::new(tr("Export library")).color(egui::Color32::WHITE))
                    .fill(visuals::ACCENT)
                    .small()
                    .corner_radius(4);
                let export = ui.add(export).on_hover_text("Ctrl+E");
                self.studio.anchors.insert(crate::tour::Anchor::Export, export.rect);
                if export.clicked() {
                    self.studio.push(Command::ExportLibrary);
                }
            });
        });
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(2.0);
            crate::tabs::show(ui, &mut self.studio);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let s = &mut self.studio;
                let (undo, redo) = (s.timeline.can_undo(), s.timeline.can_redo());
                let mut button = |ui: &mut egui::Ui, text: &str, tip: &str, command: Command| {
                    if ui.button(text).on_hover_text(tip).clicked() {
                        s.push(command);
                    }
                };
                let mut undo_clicked = false;
                let mut redo_clicked = false;
                let files = ui.horizontal(|ui| {
                    button(ui, tr("New"), "Ctrl+N", Command::NewProject);
                    button(ui, tr("Open"), "Ctrl+O", Command::Open);
                    button(ui, tr("Save"), "Ctrl+S", Command::Save);
                    ui.separator();
                    undo_clicked = ui
                        .add_enabled(undo, egui::Button::new("⟲"))
                        .on_hover_text(format!("{} (Ctrl+Z)", tr("Undo")))
                        .clicked();
                    redo_clicked = ui
                        .add_enabled(redo, egui::Button::new("⟳"))
                        .on_hover_text(format!("{} (Ctrl+Y)", tr("Redo")))
                        .clicked();
                });
                s.anchors.insert(crate::tour::Anchor::FileButtons, files.response.rect);
                if undo_clicked {
                    s.push(Command::Undo);
                }
                if redo_clicked {
                    s.push(Command::Redo);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let settings = ui.button(tr("Project settings"));
                    s.anchors.insert(crate::tour::Anchor::ProjectSettings, settings.rect);
                    if settings.clicked() {
                        s.show_project_settings = true;
                    }
                    ui.label(egui::RichText::new(s.doc.title()).strong());
                    ui.separator();
                    stats_label(ui, s);
                });
            });
            ui.add_space(2.0);
        });
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.studio;
        ui.menu_button(tr("File"), |ui| {
            let mut item = |ui: &mut egui::Ui, text: &str, shortcut: &str, command: Command| {
                if ui.add(egui::Button::new(text).shortcut_text(shortcut)).clicked() {
                    s.push(command);
                    ui.close_menu();
                }
            };
            item(ui, tr("New project"), "Ctrl+N", Command::NewProject);
            item(ui, tr("Open…"), "Ctrl+O", Command::Open);
            item(ui, tr("Start page"), "", Command::ShowStartPage);
            item(ui, tr("Import SquareLine project…"), "Ctrl+I", Command::ImportSquareLine);
            item(ui, tr("Merge project…"), "", Command::MergeProject);
            item(ui, tr("Export screenshots…"), "", Command::ExportScreenshots);
            ui.separator();
            item(ui, tr("Save"), "Ctrl+S", Command::Save);
            item(ui, tr("Save as…"), "Ctrl+Shift+S", Command::SaveAs);
            item(ui, tr("Export library"), "Ctrl+E", Command::ExportLibrary);
            ui.separator();
            let recent = s.settings.recent.clone();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button(tr("Open recent"), |ui| {
                    for path in recent {
                        if ui.button(path.display().to_string()).clicked() {
                            s.push(Command::OpenPath(path));
                            ui.close_menu();
                        }
                    }
                });
            });
            ui.separator();
            if ui.button(tr("Project settings…")).clicked() {
                s.show_project_settings = true;
                ui.close_menu();
            }
            if ui.button(tr("Quit")).clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                ui.close_menu();
            }
        });
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.studio;
        let selected = s.selected;
        ui.menu_button(tr("Edit"), |ui| {
            let mut item = |ui: &mut egui::Ui, text: &str, shortcut: &str, enabled: bool, command: Option<Command>| {
                if ui.add_enabled(enabled, egui::Button::new(text).shortcut_text(shortcut)).clicked() {
                    if let Some(command) = command {
                        s.push(command);
                    }
                    ui.close_menu();
                }
            };
            item(ui, tr("Undo"), "Ctrl+Z", true, Some(Command::Undo));
            item(ui, tr("Redo"), "Ctrl+Y", true, Some(Command::Redo));
            ui.separator();
            item(ui, tr("Cut"), "Ctrl+X", selected.is_some(), selected.map(Command::Cut));
            item(ui, tr("Copy"), "Ctrl+C", selected.is_some(), selected.map(Command::Copy));
            item(ui, tr("Paste"), "Ctrl+V", true, Some(Command::Paste));
            item(ui, tr("Copy properties"), "Ctrl+Shift+C", selected.is_some(), selected.map(Command::CopyProps));
            item(ui, tr("Paste properties"), "Ctrl+Shift+V", true, Some(Command::PasteProps));
            item(ui, tr("Duplicate"), "Ctrl+D", selected.is_some(), selected.map(Command::Duplicate));
            item(ui, tr("Delete"), "Del", selected.is_some(), selected.map(Command::Delete));
            ui.separator();
            item(ui, tr("Add screen"), "", true, Some(Command::AddScreen));
            item(ui, tr("Find…"), "Ctrl+F", true, Some(Command::Find));
            ui.separator();
            item(ui, tr("Randomize values of this screen"), "", true, Some(Command::Randomize { all: false }));
            item(ui, tr("Randomize values of all screens"), "", true, Some(Command::Randomize { all: true }));
            ui.separator();
            if ui.button(tr("Preferences…")).clicked() {
                s.show_preferences = true;
                ui.close_menu();
            }
        });
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(tr("View"), |ui| {
            for panel in Panel::ALL.iter().copied().filter(|p| *p != Panel::Canvas) {
                let found = self.dock.find_tab(&panel);
                let mut visible = found.is_some();
                if ui.checkbox(&mut visible, tr(panel.title())).changed() {
                    match found {
                        Some(location) => {
                            self.dock.remove_tab(location);
                        }
                        None => self.dock.push_to_focused_leaf(panel),
                    }
                }
            }
            ui.separator();
            if ui.button(tr("Reset layout")).clicked() {
                self.dock = default_layout();
                ui.close_menu();
            }
            ui.separator();
            let s = &mut self.studio;
            ui.checkbox(&mut s.settings.show_grid, tr("Grid"));
            ui.checkbox(&mut s.settings.snap_to_grid, tr("Snap to grid"));
            if ui.button(tr("Fit display into view")).clicked() {
                s.canvas.fit_pending = true;
                ui.close_menu();
            }
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let s = &self.studio;
                ui.label(egui::RichText::new(s.status.as_deref().unwrap_or(tr("Ready"))).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let p = s.project();
                    let widgets: usize = p.screens.iter().map(|s| s.descendant_count()).sum();
                    let shape = tr(p.display.shape.label());
                    ui.label(
                        egui::RichText::new(format!(
                            "LVGL {} · {}×{} {} · {} {} · {} {}",
                            p.lvgl_version.suffix(),
                            p.display.width,
                            p.display.height,
                            shape,
                            p.screens.len(),
                            tr("screens"),
                            widgets,
                            tr("widgets")
                        ))
                        .small(),
                    );
                });
            });
        });
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.wants_keyboard_input();
        let shortcut = |modifiers: Modifiers, key: Key| {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key)))
        };
        let s = &mut self.studio;
        let ctrl = Modifiers::COMMAND;
        let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        if shortcut(ctrl, Key::S) {
            s.push(Command::Save);
        }
        if shortcut(ctrl_shift, Key::S) {
            s.push(Command::SaveAs);
        }
        if shortcut(ctrl, Key::O) {
            s.push(Command::Open);
        }
        if shortcut(ctrl, Key::N) {
            s.push(Command::NewProject);
        }
        if shortcut(ctrl, Key::I) {
            s.push(Command::ImportSquareLine);
        }
        if shortcut(ctrl, Key::E) {
            s.push(Command::ExportLibrary);
        }
        if shortcut(ctrl, Key::W) {
            s.push(Command::CloseTab(s.active_tab));
        }
        if shortcut(ctrl_shift, Key::Tab) {
            s.push(Command::SwitchTab((s.active_tab + s.tab_count() - 1) % s.tab_count()));
        }
        if shortcut(ctrl, Key::Tab) {
            s.push(Command::SwitchTab((s.active_tab + 1) % s.tab_count()));
        }
        if shortcut(ctrl, Key::F) {
            s.search = Some(crate::search_dialog::SearchState::new());
        }
        if shortcut(Modifiers::NONE, Key::F5) {
            s.toggle_simulation();
        }
        if s.simulation.is_some() {
            if shortcut(Modifiers::NONE, Key::Escape) {
                s.simulation = None;
            }
            return;
        }
        if typing {
            return;
        }
        if shortcut(ctrl_shift, Key::Z) || shortcut(ctrl, Key::Y) {
            s.push(Command::Redo);
        }
        if shortcut(ctrl, Key::Z) {
            s.push(Command::Undo);
        }
        // The window system turns Ctrl+C, Ctrl+X and Ctrl+V into copy, cut and paste
        // events instead of key presses.
        let (copied, cut, pasted, shift) = ctx.input(|i| {
            let has = |f: fn(&egui::Event) -> bool| i.events.iter().any(f);
            (
                has(|e| matches!(e, egui::Event::Copy)),
                has(|e| matches!(e, egui::Event::Cut)),
                has(|e| matches!(e, egui::Event::Paste(_))),
                i.modifiers.shift,
            )
        });
        if shortcut(ctrl_shift, Key::V) || (pasted && shift) {
            s.push(Command::PasteProps);
        } else if shortcut(ctrl, Key::V) || pasted {
            s.push(Command::Paste);
        }
        let Some(id) = s.selected else { return };
        if shortcut(ctrl_shift, Key::C) || (copied && shift) {
            s.push(Command::CopyProps(id));
        } else if shortcut(ctrl, Key::C) || copied {
            s.push(Command::Copy(id));
        }
        if shortcut(ctrl, Key::X) || cut {
            s.push(Command::Cut(id));
        }
        if shortcut(ctrl, Key::D) {
            s.push(Command::Duplicate(id));
        }
        if shortcut(Modifiers::NONE, Key::Delete) {
            s.push(Command::Delete(id));
        }
        let step = if ctx.input(|i| i.modifiers.shift) { s.settings.nudge_large as i32 } else { 1 };
        let mut nudge = |key: Key, dx: i32, dy: i32| {
            let pressed =
                ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key) || i.consume_key(Modifiers::SHIFT, key));
            if pressed {
                if let Some(geometry) = s.canvas.nudged(&s.doc.project, id, dx * step, dy * step) {
                    if let Some(widget) = s.doc.project.widget_mut(id) {
                        widget.geometry = geometry;
                    }
                }
            }
        };
        nudge(Key::ArrowLeft, -1, 0);
        nudge(Key::ArrowRight, 1, 0);
        nudge(Key::ArrowUp, 0, -1);
        nudge(Key::ArrowDown, 0, 1);
    }

    /// Records a timeline entry once an interaction has finished.
    fn observe_changes(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let typed = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(_) | egui::Event::Key { .. })));
        if typed {
            self.last_typing = now;
        }
        let pointer_down = ctx.input(|i| i.pointer.any_down());
        let editing_text = ctx.wants_keyboard_input() && now - self.last_typing < 0.8;
        if !pointer_down && !editing_text && !self.studio.canvas.is_dragging() {
            self.studio.timeline.observe(&self.studio.doc.project);
        } else if editing_text {
            ctx.request_repaint_after(std::time::Duration::from_millis(900));
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.studio.any_tab_dirty() && !self.close_confirmed {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.show_close_confirm = true;
        }
        if !self.show_close_confirm {
            return;
        }
        let actions =
            [Action::primary(tr("Save")), Action::danger(tr("Quit without saving")), Action::secondary(tr("Cancel"))];
        let message = tr("Your changes to this project will be lost if you quit without saving.");
        let choice = modal::show(
            ctx,
            "close_confirm",
            Tone::Warning,
            tr("Save changes before quitting?"),
            message,
            &actions,
            |_| {},
        );
        match choice {
            Some(0) => {
                // Save every project with changes, one tab after the other.
                for tab in 0..self.studio.tab_count() {
                    if self.studio.tab_dirty(tab) {
                        self.studio.switch_tab(tab);
                        self.studio.execute(Command::Save, false);
                    }
                }
                self.show_close_confirm = false;
                if !self.studio.any_tab_dirty() {
                    self.close_confirmed = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Some(1) => {
                self.show_close_confirm = false;
                self.close_confirmed = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Some(_) => self.show_close_confirm = false,
            None => {}
        }
    }

    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = &self.screenshot else { return };
        if self.frame == 20 {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            if let Some(buffer) = ::image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                if let Err(err) = buffer.save(path) {
                    eprintln!("screenshot failed: {err}");
                }
            }
            self.close_confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.run_frame(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.studio.settings);
        eframe::set_value(storage, DOCK_KEY, &self.dock);
    }
}

#[cfg(test)]
impl StudioApp {
    /// An editor without a window for the monkey test.
    pub fn for_test(studio: Studio) -> Self {
        Self {
            studio,
            dock: default_layout(),
            screenshot: None,
            frame: 0,
            last_typing: 0.0,
            close_confirmed: false,
            show_close_confirm: false,
            title: String::new(),
            last_autosave: 0.0,
            last_selection: None,
        }
    }

    pub fn run_frame_for_test(&mut self, ctx: &egui::Context) {
        self.run_frame(ctx);
    }

    pub fn studio(&self) -> &Studio {
        &self.studio
    }
}

impl StudioApp {
    /// One frame of the editor; needs no window, so tests can drive it.
    fn run_frame(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        self.studio.anchors.clear();
        // Ctrl+Z in a text field must not bring back what another widget's field held.
        if self.last_selection != self.studio.selected {
            self.last_selection = self.studio.selected;
            ctx.memory_mut(|m| m.data.remove_by_type::<egui::text_edit::TextEditState>());
        }
        self.apply_settings(ctx);
        self.studio.poll_assistant();
        let dir = self.studio.doc.dir().map(std::path::Path::to_path_buf);
        self.studio.assets.begin_frame(ctx, &self.studio.doc.project, dir.as_deref());

        self.top_bar(ctx);
        self.status_bar(ctx);
        // The tour brings the tab it explains to the front; the panels about the
        // selection show the first widget of the screen if nothing is selected.
        if let Some(panel) = crate::tour::current_panel(&self.studio) {
            if let Some(location) = self.dock.find_tab(&panel) {
                self.dock.set_active_tab(location);
            }
            let about_selection = matches!(panel, Panel::Layers | Panel::Parameters | Panel::Style | Panel::Events);
            if about_selection && self.studio.selected.is_none() {
                let first = self.studio.doc.project.widget(self.studio.screen).and_then(|s| s.children.first());
                let first = first.map(|w| w.id);
                if first.is_some() {
                    self.studio.select(first);
                }
            }
        }
        let style = dock_style(ctx);
        DockArea::new(&mut self.dock)
            .style(style)
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show(ctx, &mut Viewer { studio: &mut self.studio });
        dialogs::show(ctx, &mut self.studio);
        crate::start::show(ctx, &mut self.studio);
        crate::merge_dialog::show(ctx, &mut self.studio);
        crate::search_dialog::show(ctx, &mut self.studio);
        crate::screenshots::step(ctx, &mut self.studio);
        if self.studio.exit_requested {
            self.close_confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        let s = &mut self.studio;
        let busy = s.start_page.is_some()
            || s.new_project.is_some()
            || s.confirm_discard.is_some()
            || s.report.is_some()
            || s.merge.is_some();
        if s.tour_pending && !busy {
            s.tour_pending = false;
            s.tour = Some(0);
        }
        crate::tour::show(ctx, &mut self.studio);
        self.shortcuts(ctx);

        for command in std::mem::take(&mut self.studio.commands) {
            // Paste only reaches the editor when the system clipboard holds some text.
            if matches!(command, Command::Copy(_) | Command::Cut(_) | Command::CopyProps(_)) {
                ctx.copy_text("LVGLab".to_owned());
            }
            self.studio.execute(command, false);
        }
        let s = &mut self.studio;
        let waiting = s.new_project.is_some() || s.confirm_discard.is_some() || s.report.is_some();
        if s.return_to_start && !waiting && s.commands.is_empty() {
            s.return_to_start = false;
            s.start_page = Some(crate::start::entries(&s.settings.recent));
        }
        crate::panels::images::add_pending(ctx, &mut self.studio);
        if let Some(recorder) = &mut self.studio.recorder {
            recorder.receive(ctx);
        }
        self.studio.poll_gif_jobs();
        self.observe_changes(ctx);
        self.autosave(ctx);
        self.handle_close(ctx);

        let title = format!("{} — LVGLab", self.studio.doc.title());
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        self.screenshot(ctx);
    }
}

struct Viewer<'a> {
    studio: &'a mut Studio,
}

impl TabViewer for Viewer<'_> {
    type Tab = Panel;

    fn title(&mut self, tab: &mut Panel) -> egui::WidgetText {
        tr(tab.title()).into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Panel) {
        self.studio.anchors.insert(crate::tour::Anchor::Panel(*tab), ui.clip_rect());
        panels::show(*tab, ui, self.studio);
    }

    fn closeable(&mut self, tab: &mut Panel) -> bool {
        *tab != Panel::Canvas
    }

    fn scroll_bars(&self, tab: &Panel) -> [bool; 2] {
        match tab {
            Panel::Canvas | Panel::Timeline | Panel::Assistant => [false, false],
            _ => [false, true],
        }
    }
}

fn dock_style(ctx: &egui::Context) -> egui_dock::Style {
    let mut style = egui_dock::Style::from_egui(&ctx.style());
    style.tab_bar.height = 28.0;
    style.tab_bar.corner_radius = egui::CornerRadius::ZERO;
    style.tab.tab_body.inner_margin = egui::Margin::same(8);
    style.tab.active.text_color = ctx.style().visuals.strong_text_color();
    style.tab.focused.text_color = ctx.style().visuals.strong_text_color();
    style.tab.hline_below_active_tab_name = true;
    style.overlay.selection_color = visuals::ACCENT.gamma_multiply(0.4);
    // Panels may be dragged down to a tab bar and a row (egui_dock keeps 175 px by default).
    style.separator.extra = 70.0;
    style
}

/// Default arrangement: structure on the left, canvas with timeline in the
/// middle, parameters on the right.
pub fn default_layout() -> DockState<Panel> {
    let mut dock = DockState::new(vec![Panel::Canvas]);
    let tree = dock.main_surface_mut();
    let [center, right] = tree.split_right(
        NodeIndex::root(),
        0.76,
        vec![Panel::Parameters, Panel::Themes, Panel::Style, Panel::Events, Panel::Assistant],
    );
    // The fraction is the share of the first (left/top) node of a split.
    let [center, left] = tree.split_left(center, 0.24, vec![Panel::Screens]);
    let [_, layers] = tree.split_below(left, 0.22, vec![Panel::Layers]);
    tree.split_below(layers, 0.5, vec![Panel::Widgets]);
    tree.split_below(center, 0.85, vec![Panel::Timeline]);
    tree.split_below(right, 0.66, vec![Panel::Fonts, Panel::Images]);
    dock
}

/// Puts the Themes tab next to the parameters in a layout saved before it existed.
fn add_themes_tab(dock: &mut DockState<Panel>) {
    if dock.find_tab(&Panel::Themes).is_some() {
        return;
    }
    if let Some((surface, node, tab)) = dock.find_tab(&Panel::Parameters) {
        dock[surface][node].insert_tab(TabIndex(tab.0 + 1), Panel::Themes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::WidgetKind;
    use lvglab_model::project::LvglVersion;

    #[test]
    fn switching_to_lvgl_9_removes_the_color_wheel_in_one_undoable_step() {
        let mut studio = Studio::new(AppSettings::default());
        let screen = studio.doc.project.screens[0].id;
        let wheel = studio.doc.project.new_widget(WidgetKind::Colorwheel);
        let id = wheel.id;
        studio.doc.project.insert(screen, 0, wheel).unwrap();
        studio.timeline.observe(&studio.doc.project);
        studio.select(Some(id));

        studio.doc.project.lvgl_version = LvglVersion::V9_6_0;
        studio.remove_unsupported(true);
        assert!(studio.doc.project.widget(id).is_none());
        assert_eq!(studio.selected, None);
        let report = studio.report.as_ref().expect("the removal is reported");
        assert_eq!(report.lines, vec!["Colorwheel1: widget removed (Color wheel)".to_owned()]);

        assert!(studio.timeline.observe(&studio.doc.project));
        assert_eq!(studio.timeline.entries().last().unwrap().label, "LVGL 9.6.0");
        let back = studio.timeline.undo().unwrap();
        assert_eq!(back.lvgl_version, LvglVersion::V8_3_11);
        assert!(back.widget(id).is_some());
    }

    #[test]
    fn the_lvgl_preview_draws_lvgl_8_projects_and_gives_way_for_lvgl_9() {
        crate::sandbox::enable();
        let ctx = egui::Context::default();
        let frame = |app: &mut StudioApp| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.run_frame_for_test(ctx));
        };
        let settings = AppSettings {
            tour_done: true,
            preview_renderer: crate::settings::PreviewRenderer::Lvgl,
            ..AppSettings::default()
        };
        let mut studio = Studio::new(settings);
        studio.start_page = None;
        let mut app = StudioApp::for_test(studio);
        for _ in 0..3 {
            frame(&mut app);
        }
        assert_eq!(app.studio.lvgl_preview.error, None, "{:?}", app.studio.lvgl_preview.skipped);
        assert!(app.studio.lvgl_preview.has_picture(), "LVGL drew the screen");
        app.studio.doc.project.lvgl_version = LvglVersion::V9_6_0;
        for _ in 0..3 {
            frame(&mut app);
        }
        // LVGL 9 is not rendered by the preview; the editor draws the screen.
        assert_eq!(app.studio.lvgl_preview.error, None);
    }

    #[test]
    fn projects_open_in_tabs_that_keep_their_own_state() {
        crate::sandbox::enable();
        let dir = std::env::temp_dir().join("lvglab_tabs_test");
        let _ = std::fs::create_dir_all(&dir);
        let (first, second) = (dir.join("one.lvglab"), dir.join("two.lvglab"));
        for (path, name) in [(&first, "One"), (&second, "Two")] {
            Project::new(name).save(path).unwrap();
        }
        let mut studio = Studio::new(AppSettings { tour_done: true, ..AppSettings::default() });
        studio.execute(Command::OpenPath(first.clone()), false);
        assert_eq!(studio.tab_count(), 1, "the untouched start project is replaced");
        studio.execute(Command::OpenPath(second.clone()), false);
        assert_eq!((studio.tab_count(), studio.active_tab), (2, 1));
        assert_eq!(studio.doc.project.name, "Two");
        // Changing the second project does not touch the first.
        studio.doc.project.name = "Two changed".into();
        studio.doc.mark_unsaved();
        studio.execute(Command::SwitchTab(0), false);
        assert_eq!(studio.doc.project.name, "One");
        assert!(studio.tab_dirty(1) && !studio.tab_dirty(0));
        // Opening a project again shows its tab.
        studio.execute(Command::OpenPath(second.clone()), false);
        assert_eq!((studio.tab_count(), studio.active_tab), (2, 1));
        // A project with changes is closed only after asking.
        studio.execute(Command::CloseTab(1), false);
        assert!(studio.confirm_discard.is_some() && studio.tab_count() == 2);
        studio.confirm_discard = None;
        studio.execute(Command::CloseTab(1), true);
        assert_eq!((studio.tab_count(), studio.active_tab), (1, 0));
        assert_eq!(studio.doc.project.name, "One");
        // The last tab leaves an empty project and the start page.
        studio.execute(Command::CloseTab(0), false);
        assert_eq!((studio.tab_count(), studio.doc.project.name.as_str()), (1, "MyProject"));
        assert!(studio.start_page.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_copied_screen_can_be_pasted_into_another_project_with_its_components() {
        crate::sandbox::enable();
        let dir = std::env::temp_dir().join("lvglab_screen_clip_test");
        let _ = std::fs::create_dir_all(&dir);
        let (first, second) = (dir.join("a.lvglab"), dir.join("b.lvglab"));
        let mut a = Project::new("A");
        let screen = a.screens[0].id;
        a.widget_mut(screen).unwrap().name = "Dash".into();
        let component = a.add_component();
        let component_name = a.widget(component).unwrap().name.clone();
        let mut instance = a.new_widget(lvglab_model::WidgetKind::Component);
        instance.name = "Gauge".into();
        instance.set_prop("component", lvglab_model::prop::PropValue::Text(component_name.clone()));
        a.insert(screen, 0, instance).unwrap();
        a.save(&first).unwrap();
        Project::new("B").save(&second).unwrap();

        let mut studio = Studio::new(AppSettings { tour_done: true, ..AppSettings::default() });
        studio.execute(Command::OpenPath(first), false);
        studio.execute(Command::OpenPath(second), false);
        assert_eq!(studio.tab_count(), 2);
        studio.execute(Command::SwitchTab(0), false);
        studio.execute(Command::Copy(screen), false);
        studio.execute(Command::SwitchTab(1), false);
        assert_eq!((studio.doc.project.screens.len(), studio.doc.project.components.len()), (1, 0));
        studio.execute(Command::Paste, false);
        assert!(studio.merge.is_none(), "no name clashes, so nothing to ask");
        let project = &studio.doc.project;
        assert_eq!(project.screens.len(), 2, "the screen arrived");
        assert!(project.component(&component_name).is_some(), "with its component");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn copy_and_paste_arrive_as_events_and_work() {
        crate::sandbox::enable();
        let ctx = egui::Context::default();
        let frame = |app: &mut StudioApp, event: Option<egui::Event>| {
            let mut input = egui::RawInput::default();
            input.events.extend(event);
            let _ = ctx.run(input, |ctx| app.run_frame_for_test(ctx));
        };
        let mut studio = Studio::new(AppSettings { tour_done: true, ..AppSettings::default() });
        studio.start_page = None;
        let screen = studio.screen;
        let button = studio.doc.project.new_widget(lvglab_model::WidgetKind::Button);
        let id = button.id;
        studio.doc.project.insert(screen, 0, button).unwrap();
        studio.select(Some(id));
        let mut app = StudioApp::for_test(studio);
        frame(&mut app, None);
        frame(&mut app, Some(egui::Event::Copy));
        assert!(app.studio.clipboard.is_some(), "Ctrl+C copies the selected widget");
        let count = |app: &StudioApp| {
            let mut n = 0;
            app.studio.doc.project.widget(screen).unwrap().walk(&mut |_| n += 1);
            n
        };
        let before = count(&app);
        frame(&mut app, Some(egui::Event::Paste("LVGLab".into())));
        assert!(count(&app) > before, "Ctrl+V pastes it");
        // A screen travels as a screen.
        app.studio.select(Some(screen));
        frame(&mut app, Some(egui::Event::Copy));
        assert!(app.studio.screen_clip.is_some() && app.studio.clipboard.is_none());
    }

    /// Timing of tab switches and LVGL preview frames:
    /// `LVGLAB_BENCH=a.lvglab;b.lvglab cargo test --release -p lvglab bench_frames -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_frames() {
        let Ok(files) = std::env::var("LVGLAB_BENCH") else { return };
        let ctx = egui::Context::default();
        let frame = |app: &mut StudioApp| {
            let start = std::time::Instant::now();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| app.run_frame_for_test(ctx));
            start.elapsed()
        };
        for renderer in [crate::settings::PreviewRenderer::Editor, crate::settings::PreviewRenderer::Lvgl] {
            let settings = AppSettings { tour_done: true, preview_renderer: renderer, ..AppSettings::default() };
            let mut app = StudioApp::for_test(Studio::new(settings));
            app.studio.start_page = None;
            for file in files.split(';') {
                app.studio.execute(Command::OpenPath(file.into()), true);
            }
            for _ in 0..5 {
                frame(&mut app);
            }
            eprintln!("tabs: {} ({:?})", app.studio.tab_count(), app.studio.status);
            for round in 0..3 {
                for tab in 0..app.studio.tab_count() {
                    app.studio.execute(Command::SwitchTab(tab), false);
                    let first = frame(&mut app);
                    let second = frame(&mut app);
                    eprintln!("{renderer:?} round {round} tab {tab}: {first:?} then {second:?}");
                }
            }
            // A color change on every frame, like dragging in the color picker.
            app.studio.execute(Command::SwitchTab(0), false);
            frame(&mut app);
            let id = app.studio.doc.project.screens[0].id;
            let mut total = std::time::Duration::ZERO;
            for i in 0..10_u32 {
                if let Some(screen) = app.studio.doc.project.widget_mut(id) {
                    screen.styles.set(
                        "main",
                        lvglab_model::flags::StateMask::DEFAULT,
                        lvglab_model::style::StyleProp::BgColor,
                        lvglab_model::style::StyleValue::Color(lvglab_model::Color::from_hex(0x101010 * (i % 15))),
                    );
                }
                total += frame(&mut app);
            }
            eprintln!("{renderer:?} color drag: {:?} per frame", total / 10);
        }
    }

    #[test]
    fn cancelled_start_page_actions_return_to_the_start_page() {
        crate::sandbox::enable();
        let ctx = egui::Context::default();
        let frame = |app: &mut StudioApp| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.run_frame_for_test(ctx));
        };
        let mut app = StudioApp::for_test(Studio::new(AppSettings { tour_done: true, ..AppSettings::default() }));
        // Import, then cancel the file dialog (the sandbox cancels it).
        app.studio.return_to_start = true;
        app.studio.push(Command::ImportSquareLine);
        frame(&mut app);
        assert!(app.studio.start_page.is_some(), "the start page is back after a cancelled import");

        // New project, then cancel the dialog.
        app.studio.start_page = None;
        app.studio.return_to_start = true;
        app.studio.push(Command::NewProject);
        frame(&mut app);
        assert!(app.studio.new_project.is_some() && app.studio.start_page.is_none());
        app.studio.new_project = None;
        frame(&mut app);
        assert!(app.studio.start_page.is_some(), "the start page is back after a cancelled new project");

        // Creating a project does not bring it back.
        app.studio.start_page = None;
        app.studio.return_to_start = true;
        app.studio.push(Command::NewProject);
        frame(&mut app);
        let draft = app.studio.new_project.take().unwrap();
        app.studio.push(Command::CreateProject(Box::new(draft)));
        frame(&mut app);
        frame(&mut app);
        assert!(app.studio.start_page.is_none());
    }

    #[test]
    fn new_asks_for_the_settings_and_remembers_them() {
        let mut studio = Studio::new(AppSettings::default());
        studio.doc.project.name = "Changed".into();
        studio.timeline.observe(&studio.doc.project);
        // Opening the dialog replaces nothing yet.
        studio.execute(Command::NewProject, false);
        assert_eq!(studio.doc.project.name, "Changed");
        let mut draft = studio.new_project.clone().expect("the dialog is open");
        draft.name = "Dash".into();
        draft.display.width = 466;
        draft.display.height = 466;
        draft.display.shape = lvglab_model::project::DisplayShape::Round;
        draft.lvgl_version = LvglVersion::V9_6_0;

        let create = Command::CreateProject(Box::new(draft));
        studio.execute(create.clone(), false);
        if studio.doc.is_dirty() {
            assert!(studio.confirm_discard.is_some(), "unsaved changes are not dropped silently");
            studio.execute(create, true);
        }
        let project = &studio.doc.project;
        assert_eq!(
            (project.name.as_str(), project.display.width, project.lvgl_version),
            ("Dash", 466, LvglVersion::V9_6_0)
        );
        studio.execute(Command::NewProject, true);
        let next = studio.new_project.as_ref().unwrap();
        assert_eq!(
            (next.display.width, next.lvgl_version),
            (466, LvglVersion::V9_6_0),
            "the choices are offered again"
        );
        assert_eq!(next.name, "MyProject");
    }

    #[test]
    fn opening_an_lvgl_9_project_removes_what_it_lacks() {
        let mut project = Project::new("Old");
        project.lvgl_version = LvglVersion::V9_2_2;
        let screen = project.screens[0].id;
        let wheel = project.new_widget(WidgetKind::Colorwheel);
        project.insert(screen, 0, wheel).unwrap();
        let path = std::env::temp_dir().join(format!("lvglab-open-{}.lvglab", std::process::id()));
        project.save(&path).unwrap();

        let mut studio = Studio::new(AppSettings { save_timeline: false, ..AppSettings::default() });
        studio.execute(Command::OpenPath(path.clone()), true);
        std::fs::remove_file(&path).ok();
        assert!(studio.doc.project.screens[0].children.is_empty());
        assert_eq!(studio.report.as_ref().map(|r| r.lines.len()), Some(1));
    }
}
