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
    Open,
    OpenPath(PathBuf),
    ImportSquareLine,
    ImportSquareLinePath(PathBuf),
    Save,
    SaveAs,
    ExportLibrary,
    Undo,
    Redo,
    JumpTo(usize),
    AddScreen,
    AddComponent,
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

impl Command {
    /// Commands that replace the document ask before discarding changes.
    fn replaces_document(&self) -> bool {
        matches!(
            self,
            Self::CreateProject(_)
                | Self::Open
                | Self::OpenPath(_)
                | Self::ImportSquareLine
                | Self::ImportSquareLinePath(_)
        )
    }
}

impl Studio {
    pub fn execute(&mut self, command: Command, confirmed: bool) {
        if command.replaces_document() && self.doc.is_dirty() && !confirmed {
            self.confirm_discard = Some(command);
            return;
        }
        match command {
            Command::NewProject => self.new_project = Some(NewProject::from_defaults(&self.settings.new_project)),
            Command::ShowStartPage => self.start_page = Some(crate::start::entries(&self.settings.recent)),
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
            Command::AddScreen => {
                let id = self.doc.project.add_screen();
                self.select_screen(id);
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
            Command::Copy(id) => self.clipboard = self.doc.project.widget(id).cloned(),
            Command::Cut(id) => {
                self.clipboard = self.doc.project.widget(id).cloned();
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
        self.timeline = Timeline::new(&doc.project, label);
        self.doc = doc;
        self.selected = None;
        self.screen = self.doc.project.screens.first().map_or(WidgetId(0), |s| s.id);
        self.canvas.fit_pending = true;
    }

    fn open(&mut self, path: PathBuf) {
        let is_squareline = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("spj"));
        if is_squareline {
            self.import_squareline(path);
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

    fn paste(&mut self) {
        let Some(clip) = self.clipboard.clone() else { return };
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

    pub fn show_error(&mut self, title: &str, message: String) {
        self.notify(format!("{title}: {message}"));
        self.report = Some(Report { tone: Tone::Danger, title: title.to_owned(), message, lines: Vec::new() });
    }
}

/// Shows a folder in the system file manager.
fn open_folder(path: &std::path::Path) {
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
