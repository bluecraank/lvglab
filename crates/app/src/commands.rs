//! User commands. Panels only queue commands; they are executed in one place
//! after the frame's UI pass, which keeps borrowing simple and behavior
//! consistent between menus, shortcuts and context menus.

use std::path::{Path, PathBuf};

use lvglab_model::project::{DisplaySettings, LvglVersion, PROJECT_EXTENSION, ThemeSettings};
use lvglab_model::{Project, WidgetId, WidgetKind};

use crate::app::{Report, Studio};
use crate::document::{Document, EntryKind, Timeline};
use crate::i18n::{tr, trf};
use crate::modal::Tone;
use crate::settings::NewProjectDefaults;

/// The settings asked for when creating a project.
#[derive(Clone, Debug, PartialEq)]
pub struct NewProject {
    pub name: String,
    pub display: DisplaySettings,
    pub lvgl_version: LvglVersion,
    pub theme: ThemeSettings,
}

impl NewProject {
    pub fn from_defaults(defaults: &NewProjectDefaults) -> Self {
        Self {
            name: "MyProject".to_owned(),
            display: defaults.display.clone(),
            lvgl_version: defaults.lvgl_version,
            theme: defaults.theme.clone(),
        }
    }

    pub fn defaults(&self) -> NewProjectDefaults {
        NewProjectDefaults { display: self.display.clone(), lvgl_version: self.lvgl_version, theme: self.theme.clone() }
    }

    pub fn build(&self) -> Project {
        let name = self.name.trim();
        let mut project = Project::new(if name.is_empty() { "MyProject" } else { name });
        project.display = self.display.clone();
        project.lvgl_version = self.lvgl_version;
        project.theme = self.theme.clone();
        project
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    /// Asks for the settings of a new project.
    NewProject,
    CreateProject(Box<NewProject>),
    /// The start page with the recent projects.
    ShowStartPage,
    /// Saves a PNG of every screen (asks for the folder).
    ExportScreenshots,
    /// Opens the search window.
    Find,
    SwitchTab(usize),
    CloseTab(usize),
    /// Remembers the look of a widget (properties, styles, events).
    CopyProps(WidgetId),
    /// Gives it to the selected widgets of the same kind.
    PasteProps,
    /// Random values for the widgets of the current screen or of all screens.
    Randomize {
        all: bool,
    },
    /// Takes screens of another project over (asks for the file).
    MergeProject,
    MergeProjectPath(PathBuf),
    Open,
    OpenPath(PathBuf),
    /// Opens a recovery copy left by a crash as unsaved project.
    RestoreRecovery(PathBuf),
    DiscardRecovery(PathBuf),
    ImportSquareLine,
    ImportSquareLinePath(PathBuf),
    Save,
    SaveAs,
    ExportLibrary,
    Undo,
    Redo,
    JumpTo(usize),
    /// Takes the current screen from timeline step `index`; all other screens stay as they are.
    RestoreScreen(usize),
    AddScreen,
    AddComponent,
    /// Puts the top level objects of a component into one container.
    GroupComponent(WidgetId),
    /// Adds a widget to `parent` (or next to/into the selection).
    AddWidget {
        kind: WidgetKind,
        parent: Option<WidgetId>,
    },
    Delete(WidgetId),
    Duplicate(WidgetId),
    Copy(WidgetId),
    Cut(WidgetId),
    Paste,
    Move {
        id: WidgetId,
        parent: WidgetId,
        index: usize,
    },
    Shift {
        id: WidgetId,
        delta: isize,
    },
}

/// A screen on the clipboard: a project that holds only this screen, with the
/// components, fonts, images and theme colors it uses, and where its files are.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenClip {
    pub project: Project,
    pub dir: Option<PathBuf>,
    pub name: String,
    pub screen: WidgetId,
}

/// What the clipboard file of the editors holds.
#[derive(serde::Serialize, serde::Deserialize)]
enum Clip {
    Widget(lvglab_model::Widget),
    Screen(ScreenClip),
}

impl Studio {
    pub fn execute(&mut self, command: Command, confirmed: bool) {
        // A project with unsaved changes is closed only after asking.
        if let Command::CloseTab(index) = command {
            if !confirmed && self.tab_dirty(index) {
                self.switch_tab(index);
                self.confirm_discard = Some(command);
                return;
            }
        }
        match command {
            Command::NewProject => self.new_project = Some(NewProject::from_defaults(&self.settings.new_project)),
            Command::Randomize { all } => self.randomize(all),
            Command::CopyProps(id) => {
                self.props_clipboard = self.doc.project.widget(id).cloned();
                if self.props_clipboard.is_some() {
                    self.notify(tr("Properties copied. Select widgets of the same type and paste them."));
                }
            }
            Command::PasteProps => self.paste_props(),
            Command::SwitchTab(index) => self.switch_tab(index),
            Command::CloseTab(index) => self.close_tab(index),
            Command::Find => self.search = Some(crate::search_dialog::SearchState::new()),
            Command::ExportScreenshots => {
                let project_dir = self.doc.dir().map(Path::to_path_buf);
                let default = crate::screenshots::default_dir(project_dir.as_deref(), &self.doc.project.name);
                let parent = default.parent().map(Path::to_path_buf).unwrap_or_default();
                let dialog = crate::sandbox::FileDialog::new().set_title(tr("Screenshot folder")).set_directory(parent);
                if let Some(dir) = dialog.pick_folder() {
                    self.start_screenshots(dir, false);
                }
            }
            Command::ShowStartPage => self.start_page = Some(crate::start::entries(&self.settings.recent)),
            Command::MergeProject => {
                let dialog = self.file_dialog().add_filter(tr("Projects"), &[PROJECT_EXTENSION, "spj"]);
                if let Some(path) = dialog.pick_file() {
                    self.execute(Command::MergeProjectPath(path), true);
                }
            }
            Command::MergeProjectPath(path) => {
                let loaded = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("spj")) {
                    lvglab_spj::import_file(&path).map(|i| i.project).map_err(|e| e.to_string())
                } else {
                    Project::load(&path).map_err(|e| e.to_string())
                };
                match loaded {
                    Ok(source) => self.merge = Some(crate::merge_dialog::MergeWizard::new(source, &path)),
                    Err(err) => self.show_error(tr("Merge failed"), err),
                }
            }
            Command::CreateProject(project) => {
                self.settings.new_project = project.defaults();
                self.load_document(Document::new(project.build(), None), tr("New project"));
            }
            Command::Open => {
                if let Some(path) =
                    self.file_dialog().add_filter(tr("Studio project"), &[PROJECT_EXTENSION]).pick_file()
                {
                    self.execute(Command::OpenPath(path), true);
                }
            }
            Command::OpenPath(path) => self.open(path),
            Command::RestoreRecovery(path) => self.restore_recovery(path),
            Command::DiscardRecovery(path) => {
                crate::recovery::remove(&path);
                self.recovered.retain(|f| f.path != path);
            }
            Command::ImportSquareLine => {
                if let Some(path) = self.file_dialog().add_filter("SquareLine Studio", &["spj"]).pick_file() {
                    self.execute(Command::ImportSquareLinePath(path), true);
                }
            }
            Command::ImportSquareLinePath(path) => self.import_squareline(path),
            Command::Save => match self.doc.path.clone() {
                Some(path) => self.save_to(path, EntryKind::Saved),
                None => self.execute(Command::SaveAs, false),
            },
            Command::SaveAs => {
                let dialog = self
                    .file_dialog()
                    .add_filter(tr("Studio project"), &[PROJECT_EXTENSION])
                    .set_file_name(format!("{}.{PROJECT_EXTENSION}", self.doc.project.name));
                if let Some(path) = dialog.save_file() {
                    self.save_to(path, EntryKind::Saved);
                }
            }
            Command::ExportLibrary => self.export_library(),
            Command::Undo => {
                if let Some(project) = self.timeline.undo() {
                    self.restore(project);
                }
            }
            Command::Redo => {
                if let Some(project) = self.timeline.redo() {
                    self.restore(project);
                }
            }
            Command::JumpTo(index) => {
                if let Some(project) = self.timeline.jump(index) {
                    self.restore(project);
                }
            }
            Command::RestoreScreen(index) => {
                let screen = self.screen;
                let restored = self
                    .timeline
                    .entries()
                    .get(index)
                    .is_some_and(|entry| self.doc.project.restore_root(&entry.project, screen));
                if restored {
                    self.notify(trf("Restored this screen from step {}.", &[&(index + 1)]));
                }
            }
            Command::AddScreen => {
                let id = self.doc.project.add_screen();
                self.select_screen(id);
            }
            Command::GroupComponent(id) => {
                if self.doc.project.group_component_roots(id) {
                    self.notify(tr("Grouped the objects of the component into one container."));
                }
            }
            Command::AddComponent => {
                let id = self.doc.project.add_component();
                self.select_screen(id);
            }
            Command::AddWidget { kind, parent } => self.add_widget(kind, parent),
            Command::Delete(id) => self.delete(id),
            Command::Duplicate(id) => {
                if let Some(copy) = self.doc.project.duplicate(id) {
                    self.select(Some(copy));
                }
            }
            Command::Copy(id) => self.copy(id),
            Command::Cut(id) => {
                self.copy(id);
                self.delete(id);
            }
            Command::Paste => self.paste(),
            Command::Move { id, parent, index } => {
                if !self.doc.project.move_widget(id, parent, index) {
                    self.notify(tr("The widget cannot be moved there."));
                }
            }
            Command::Shift { id, delta } => {
                self.doc.project.shift(id, delta);
            }
        }
    }

    fn file_dialog(&self) -> crate::sandbox::FileDialog {
        let dialog = crate::sandbox::FileDialog::new();
        match self.doc.dir() {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        }
    }

    fn load_document(&mut self, doc: Document, label: &str) {
        // A project was opened or created, so the start page has done its job.
        self.return_to_start = false;
        let timeline = Timeline::new(&doc.project, label);
        self.open_in_tab(doc, timeline);
    }

    fn open(&mut self, path: PathBuf) {
        let is_squareline = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("spj"));
        if is_squareline {
            self.import_squareline(path);
            return;
        }
        // A project that is open already is shown instead of opened twice.
        if let Some(tab) = self.tab_of(&path) {
            self.switch_tab(tab);
            return;
        }
        match Project::load(&path) {
            Ok(project) => {
                self.settings.add_recent(path.clone());
                let saved = self
                    .settings
                    .save_timeline
                    .then(|| Timeline::load(&crate::document::timeline_path(&path), self.settings.history_limit))
                    .flatten();
                let loaded = project.clone();
                self.load_document(Document::new(project, Some(path.clone())), tr("Opened"));
                if let Some(mut timeline) = saved {
                    timeline.resume(&loaded, tr("Changed outside LVGLab"));
                    self.timeline = timeline;
                }
                self.notify(trf("Opened {}", &[&path.display()]));
                // Files from older versions may hold what the project's LVGL release lacks.
                self.remove_unsupported(false);
            }
            Err(err) => self.show_error(tr("Open failed"), err.to_string()),
        }
    }

    fn restore_recovery(&mut self, path: PathBuf) {
        self.recovered.retain(|f| f.path != path);
        match Project::load(&path) {
            Ok(project) => {
                let mut doc = Document::new(project, None);
                doc.mark_unsaved();
                // Autosave keeps writing the same copy until the project is saved.
                doc.recovery = Some(path);
                self.load_document(doc, tr("Restored"));
                self.notify(tr("Restored the unsaved project — save it to keep it."));
            }
            Err(err) => self.show_error(tr("Open failed"), err.to_string()),
        }
    }

    /// Writes the recovery copy of a project that has no file yet.
    pub fn save_recovery(&mut self) {
        if self.doc.recovery.is_none() {
            self.doc.recovery = crate::recovery::new_path();
        }
        let Some(path) = self.doc.recovery.clone() else { return };
        if let Err(err) = crate::recovery::save(&path, &self.doc.project) {
            self.notify(trf("The recovery copy could not be saved: {}", &[&err]));
        }
    }

    fn import_squareline(&mut self, path: PathBuf) {
        match lvglab_spj::import_file(&path) {
            Ok(imported) => {
                let mut doc = Document::new(imported.project, None);
                doc.mark_unsaved();
                self.load_document(doc, tr("Imported from SquareLine"));
                self.notify(trf("Imported {} — save it as studio project to keep it.", &[&path.display()]));
                if !imported.warnings.is_empty() {
                    self.report = Some(Report {
                        tone: Tone::Warning,
                        title: tr("Imported with notes").to_owned(),
                        message: tr("Some parts of the SquareLine project could not be taken over exactly:").to_owned(),
                        lines: imported.warnings,
                    });
                }
            }
            Err(err) => self.show_error(tr("Import failed"), err.to_string()),
        }
    }

    /// Saves the project to `path` and marks the state in the timeline.
    pub fn save_to(&mut self, path: PathBuf, kind: EntryKind) {
        // Images and fonts are copied into `assets/` next to the project file.
        let old_dir = self.doc.dir().map(Path::to_path_buf);
        let dir = path.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let missing = match self.doc.project.collect_assets(old_dir.as_deref(), &dir) {
            Ok(missing) => missing,
            Err(err) => return self.show_error(tr("Save failed"), err.to_string()),
        };
        match self.doc.project.save(&path) {
            Ok(()) => {
                self.settings.add_recent(path.clone());
                self.doc.mark_saved(path.clone());
                let label = if kind == EntryKind::Autosaved { tr("Saved automatically") } else { tr("Saved") };
                self.timeline.mark_save(&self.doc.project, kind, label);
                if self.settings.save_timeline {
                    if let Err(err) = self.timeline.save(&crate::document::timeline_path(&path)) {
                        self.notify(trf("The timeline could not be saved: {}", &[&err]));
                    }
                }
                self.notify(trf("Saved {}", &[&path.display()]));
                if !missing.is_empty() {
                    self.report = Some(Report {
                        tone: Tone::Warning,
                        title: tr("Saved with missing assets").to_owned(),
                        message: tr("These asset files were not found and could not be copied into the project's assets folder:")
                            .to_owned(),
                        lines: missing,
                    });
                }
            }
            Err(err) => self.show_error(tr("Save failed"), err.to_string()),
        }
    }

    fn export_library(&mut self) {
        let project = &self.doc.project;
        let base = match self.doc.dir() {
            Some(dir) => Project::resolve_path(Some(dir), &project.export.directory),
            None => match crate::sandbox::FileDialog::new().set_title(tr("Export folder")).pick_folder() {
                Some(dir) => dir,
                None => return,
            },
        };
        let name = if project.export.library_name.trim().is_empty() {
            "ui".to_owned()
        } else {
            project.export.library_name.clone()
        };
        let root = base.join(&name);
        let output = lvglab_codegen::generate_library(project, self.doc.dir());
        match lvglab_codegen::write(&root, &output, &[lvglab_codegen::SRC_DIR]) {
            Ok(summary) => {
                let message = trf(
                    "{} written, {} unchanged, {} kept, {} removed.",
                    &[&summary.written, &summary.unchanged, &summary.kept, &summary.removed],
                );
                let tone = if output.warnings.is_empty() { Tone::Success } else { Tone::Warning };
                let lines = output.warnings;
                self.notify(trf("Library exported to {}", &[&root.display()]));
                if self.settings.open_export_folder {
                    open_folder(&root);
                }
                self.report = Some(Report { tone, title: trf("Exported to {}", &[&root.display()]), message, lines });
            }
            Err(err) => self.show_error(tr("Export failed"), err.to_string()),
        }
    }

    fn restore(&mut self, project: Project) {
        self.doc.project = project;
        if self.selected.is_some_and(|id| self.doc.project.widget(id).is_none()) {
            self.selected = None;
        }
        if self.doc.project.widget(self.screen).is_none() {
            self.screen = self.doc.project.screens.first().map_or(WidgetId(0), |s| s.id);
        }
    }

    /// The container new widgets go into: the selection if it accepts the
    /// kind, else its parent chain, else the current screen.
    fn insertion_parent(&self, kind: WidgetKind) -> Option<WidgetId> {
        let start = self.selected.unwrap_or(self.screen);
        let path = self.doc.project.path_to(start)?;
        path.iter().rev().find(|w| w.def().accepts_child(kind)).map(|w| w.id)
    }

    fn add_widget(&mut self, kind: WidgetKind, parent: Option<WidgetId>) {
        let Some(parent) = parent.or_else(|| self.insertion_parent(kind)) else {
            let needed = kind.def().required_parent.map_or("", |p| p.name());
            self.notify(trf("{} needs a parent of type {}.", &[&tr(kind.name()), &tr(needed)]));
            return;
        };
        let widget = self.doc.project.new_widget(kind);
        let id = widget.id;
        if self.doc.project.insert(parent, usize::MAX, widget).is_ok() {
            self.select(Some(id));
        }
    }

    fn delete(&mut self, id: WidgetId) {
        if self.doc.project.screens.len() == 1 && self.doc.project.screens[0].id == id {
            self.notify(tr("A project needs at least one screen."));
            return;
        }
        let location = self.doc.project.location(id);
        self.doc.project.remove(id);
        let next = location.and_then(|l| l.parent).or_else(|| self.doc.project.screens.first().map(|s| s.id));
        if self.doc.project.widget(self.screen).is_none() {
            self.screen = self.doc.project.screens.first().map_or(WidgetId(0), |s| s.id);
        }
        self.select(next);
    }

    /// File that carries the clipboard to other running editors.
    fn shared_clipboard_path() -> std::path::PathBuf {
        std::env::temp_dir().join("lvglab-clipboard.json")
    }

    /// Copies a widget, or a screen with the components, fonts and images it uses.
    fn copy(&mut self, id: WidgetId) {
        self.clipboard = None;
        self.screen_clip = None;
        let Some(widget) = self.doc.project.widget(id).cloned() else { return };
        if self.doc.project.screens.iter().any(|s| s.id == id) {
            let mut project = self.doc.project.clone();
            project.screens.retain(|s| s.id == id);
            self.screen_clip = Some(ScreenClip {
                project,
                dir: self.doc.dir().map(std::path::Path::to_path_buf),
                name: widget.name.clone(),
                screen: id,
            });
        } else {
            self.clipboard = Some(widget);
        }
        self.share_clipboard();
    }

    /// Writes the copy where other editors find it.
    fn share_clipboard(&mut self) {
        let clip = match (&self.screen_clip, &self.clipboard) {
            (Some(screen), _) => Clip::Screen(screen.clone()),
            (None, Some(widget)) => Clip::Widget(widget.clone()),
            (None, None) => return,
        };
        if crate::sandbox::active() {
            return;
        }
        if let Ok(text) = serde_json::to_string(&clip) {
            let _ = std::fs::write(Self::shared_clipboard_path(), text);
        }
        self.clipboard_time = Some(std::time::SystemTime::now());
    }

    /// Something copied in another editor after this editor's own copy.
    fn foreign_clipboard(&self) -> Option<Clip> {
        if crate::sandbox::active() {
            return None;
        }
        let path = Self::shared_clipboard_path();
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
        // An editor reads its own copy from memory.
        if self.clipboard_time.is_some_and(|own| modified <= own + std::time::Duration::from_millis(50)) {
            return None;
        }
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// Pastes a copied screen. Into the project it came from it is simply
    /// duplicated; into another project it is merged, with the components,
    /// fonts and images it uses (and a question for names that exist already).
    fn paste_screen(&mut self, clip: ScreenClip) {
        let known = self.doc.project.widget(clip.screen).is_some_and(|w| w.name == clip.name);
        if known {
            if let Some(source) = clip.project.widget(clip.screen) {
                let copy = self.doc.project.clone_with_new_ids(source);
                let id = copy.id;
                self.doc.project.screens.push(copy);
                self.select_screen(id);
                self.remove_unsupported(false);
            }
            return;
        }
        crate::merge_dialog::paste_screen(self, clip);
    }

    /// Applies the copied look to the selected widgets of the same kind.
    fn paste_props(&mut self) {
        let Some(source) = self.props_clipboard.clone() else { return };
        let (mut changed, mut other) = (0, 0);
        for id in self.selection() {
            let Some(widget) = self.doc.project.widget_mut(id) else { continue };
            if widget.id == source.id {
                continue;
            }
            if widget.copy_look_from(&source) {
                changed += 1;
            } else {
                other += 1;
            }
        }
        if changed > 0 {
            self.notify(trf("Properties pasted to {} widgets.", &[&changed]));
        } else if other > 0 {
            self.notify(tr("The selected widgets have another type than the copied one."));
        }
    }

    fn paste(&mut self) {
        let own = match (&self.screen_clip, &self.clipboard) {
            (Some(screen), _) => Some(Clip::Screen(screen.clone())),
            (None, Some(widget)) => Some(Clip::Widget(widget.clone())),
            (None, None) => None,
        };
        let clip = match self.foreign_clipboard().or(own) {
            Some(Clip::Widget(widget)) => widget,
            Some(Clip::Screen(screen)) => return self.paste_screen(screen),
            None => return,
        };
        let copy = self.doc.project.clone_with_new_ids(&clip);
        let id = copy.id;
        if copy.kind == lvglab_model::WidgetKind::Screen {
            self.doc.project.screens.push(copy);
            self.select_screen(id);
            self.remove_unsupported(false);
            return;
        }
        let Some(parent) = self.insertion_parent(copy.kind) else { return };
        if self.doc.project.insert(parent, usize::MAX, copy).is_ok() {
            self.select(Some(id));
            // The copy may come from a project for another LVGL release.
            self.remove_unsupported(false);
        }
    }

    /// Fills the widgets with random plausible values (one undoable step).
    pub fn randomize(&mut self, all: bool) {
        let seed =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        let screens = if all { Vec::new() } else { vec![self.screen] };
        let changed = lvglab_model::randomize::randomize(&mut self.doc.project, &screens, seed);
        self.notify(trf("{} widgets got random values", &[&changed]));
    }

    pub fn show_error(&mut self, title: &str, message: String) {
        self.notify(format!("{title}: {message}"));
        self.report = Some(Report { tone: Tone::Danger, title: title.to_owned(), message, lines: Vec::new() });
    }
}

/// Shows a folder in the system file manager.
pub(crate) fn open_folder(path: &std::path::Path) {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // The file manager is a convenience; failing to start it is not an error.
    if !crate::sandbox::active() {
        let _ = std::process::Command::new(program).arg(path).spawn();
    }
}
