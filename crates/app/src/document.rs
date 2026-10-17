//! The open document and its change timeline.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use lvglab_model::{Project, Widget};

use crate::i18n::tr;

/// The project being edited plus where it lives on disk.
pub struct Document {
    pub project: Project,
    pub path: Option<PathBuf>,
    saved: Project,
}

impl Document {
    pub fn new(project: Project, path: Option<PathBuf>) -> Self {
        Self { saved: project.clone(), project, path }
    }

    pub fn is_dirty(&self) -> bool {
        self.project != self.saved
    }

    pub fn mark_saved(&mut self, path: PathBuf) {
        self.saved = self.project.clone();
        self.path = Some(path);
    }

    /// Marks the current state as unsaved (e.g. after an import).
    pub fn mark_unsaved(&mut self) {
        self.saved = Project::default();
    }

    pub fn dir(&self) -> Option<&Path> {
        self.path.as_deref().and_then(Path::parent)
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| self.project.name.clone(), |n| n.to_string_lossy().into_owned());
        if self.is_dirty() { format!("{name} •") } else { name }
    }
}

/// One state of the project in the timeline.
pub struct Snapshot {
    pub project: Project,
    pub label: String,
    pub time: SystemTime,
}

/// Linear history of project states. Jumping back and making a new change
/// discards the states after the current one, like undo/redo.
pub struct Timeline {
    entries: Vec<Snapshot>,
    current: usize,
    limit: usize,
}

const DEFAULT_LIMIT: usize = 300;

impl Timeline {
    pub fn new(project: &Project, label: impl Into<String>) -> Self {
        Self {
            entries: vec![Snapshot { project: project.clone(), label: label.into(), time: SystemTime::now() }],
            current: 0,
            limit: DEFAULT_LIMIT,
        }
    }

    /// Maximum number of kept steps; the oldest steps are dropped first.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(2);
        self.trim();
    }

    fn trim(&mut self) {
        let excess = self.entries.len().saturating_sub(self.limit);
        if excess > 0 {
            self.entries.drain(..excess);
            self.current = self.current.saturating_sub(excess);
        }
    }

    pub fn entries(&self) -> &[Snapshot] {
        &self.entries
    }

    pub fn current(&self) -> usize {
        self.current
    }

    /// Records `project` if it differs from the current state. Call when
    /// the user is not in the middle of an interaction (dragging, typing).
    pub fn observe(&mut self, project: &Project) -> bool {
        let base = &self.entries[self.current].project;
        if base == project {
            return false;
        }
        let label = describe_change(base, project);
        self.entries.truncate(self.current + 1);
        self.entries.push(Snapshot { project: project.clone(), label, time: SystemTime::now() });
        self.current = self.entries.len() - 1;
        self.trim();
        true
    }

    /// Moves to entry `index` and returns its project.
    pub fn jump(&mut self, index: usize) -> Option<Project> {
        let entry = self.entries.get(index)?;
        self.current = index;
        Some(entry.project.clone())
    }

    pub fn can_undo(&self) -> bool {
        self.current > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current + 1 < self.entries.len()
    }

    pub fn undo(&mut self) -> Option<Project> {
        self.can_undo().then(|| self.jump(self.current - 1)).flatten()
    }

    pub fn redo(&mut self) -> Option<Project> {
        self.can_redo().then(|| self.jump(self.current + 1)).flatten()
    }
}

/// A short human readable description of what changed between two states.
pub fn describe_change(old: &Project, new: &Project) -> String {
    let old_widgets = old.all_widgets();
    let new_widgets = new.all_widgets();
    if new_widgets.len() > old_widgets.len() {
        let added = new_widgets.iter().find(|w| old.widget(w.id).is_none());
        return format!("{} {}", tr("Added"), added.map_or("", |w| w.name.as_str()));
    }
    if new_widgets.len() < old_widgets.len() {
        let removed = old_widgets.iter().find(|w| new.widget(w.id).is_none());
        return format!("{} {}", tr("Deleted"), removed.map_or("", |w| w.name.as_str()));
    }
    for widget in &new_widgets {
        let Some(before) = old.widget(widget.id) else { return tr("Changed widgets").to_owned() };
        if let Some(what) = describe_widget_change(before, widget) {
            return format!("{}: {what}", widget.name);
        }
    }
    if old.location_order() != new.location_order() {
        return tr("Reordered").to_owned();
    }
    if old.assets != new.assets {
        return tr("Assets").to_owned();
    }
    if old.display != new.display || old.theme != new.theme || old.lvgl_version != new.lvgl_version {
        return tr("Project settings").to_owned();
    }
    tr("Changed").to_owned()
}

fn describe_widget_change(old: &Widget, new: &Widget) -> Option<String> {
    if old.name != new.name {
        return Some(format!("{} „{}“", tr("Renamed from"), old.name));
    }
    if old.geometry != new.geometry {
        let (g, h) = (old.geometry, new.geometry);
        let what = if g.width != h.width || g.height != h.height {
            tr("Size")
        } else if g.align != h.align {
            tr("Alignment")
        } else {
            tr("Position")
        };
        return Some(what.to_owned());
    }
    if old.props != new.props {
        let key = new
            .props
            .iter()
            .find(|(k, v)| old.props.get(*k) != Some(v))
            .map(|(k, _)| k.clone())
            .or_else(|| old.props.keys().find(|k| !new.props.contains_key(*k)).cloned())
            .unwrap_or_default();
        return Some(new.def().prop(&key).map_or(key, |p| tr(p.label).to_owned()));
    }
    if old.styles != new.styles {
        let changed = new
            .styles
            .blocks()
            .flat_map(|b| b.props.iter().map(move |(p, v)| (b, p, v)))
            .find(|(b, p, v)| old.styles.get(&b.slot, b.state, **p) != Some(*v))
            .map(|(_, p, _)| p.name());
        return Some(format!("{} {}", tr("Style"), changed.unwrap_or_default()));
    }
    if old.flags != new.flags || old.states != new.states {
        return Some(tr("Flags").to_owned());
    }
    if old.events != new.events {
        return Some(tr("Events").to_owned());
    }
    if old.layout != new.layout || old.scroll != new.scroll || old.ext_click_area != new.ext_click_area {
        return Some(tr("Layout").to_owned());
    }
    None
}

trait LocationOrder {
    fn location_order(&self) -> Vec<lvglab_model::WidgetId>;
}

impl LocationOrder for Project {
    fn location_order(&self) -> Vec<lvglab_model::WidgetId> {
        self.all_widgets().iter().map(|w| w.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::{Coord, WidgetKind};

    #[test]
    fn timeline_records_and_jumps() {
        let mut project = Project::new("T");
        let mut timeline = Timeline::new(&project, "Start");
        assert!(!timeline.observe(&project));
        let screen = project.screens[0].id;
        let label = project.new_widget(WidgetKind::Label);
        let id = label.id;
        project.insert(screen, 0, label).unwrap();
        assert!(timeline.observe(&project));
        assert!(timeline.entries()[1].label.contains("Label1"));
        project.widget_mut(id).unwrap().geometry.x = Coord::px(5);
        timeline.observe(&project);
        assert!(timeline.entries()[2].label.starts_with("Label1:"));

        let back = timeline.jump(0).unwrap();
        assert!(back.widget(id).is_none());
        assert!(timeline.can_redo());
        // A new change after jumping back drops the newer entries.
        let mut changed = back;
        changed.name = "Other".into();
        timeline.observe(&changed);
        assert_eq!(timeline.entries().len(), 2);
        assert!(!timeline.can_redo());
        assert!(timeline.undo().is_some());
    }

    #[test]
    fn timeline_respects_limit() {
        let mut project = Project::new("T");
        let mut timeline = Timeline::new(&project, "Start");
        timeline.set_limit(3);
        for i in 0..5 {
            project.name = format!("P{i}");
            timeline.observe(&project);
        }
        assert_eq!(timeline.entries().len(), 3);
        assert_eq!(timeline.current(), 2);
        assert_eq!(timeline.entries()[2].project.name, "P4");
    }
}
