//! The timeline is saved next to the project as `<name>.timeline`, so the
//! project file itself only holds the current state.
//!
//! Steps differ mostly in one screen, so every step is split into parts
//! (project settings, each screen, each component) and each distinct part
//! is stored once; a step lists the hashes of its parts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use lvglab_model::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{EntryKind, Snapshot, Timeline};

const VERSION: u32 = 1;
pub const EXTENSION: &str = "timeline";

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    current: usize,
    entries: Vec<Entry>,
    /// Distinct project parts by hash.
    parts: BTreeMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    label: String,
    /// Seconds since 1970.
    time: u64,
    kind: String,
    /// Hash of the settings, then of every screen and component.
    meta: String,
    screens: Vec<String>,
    components: Vec<String>,
}

/// `<project>.timeline` next to `project_path`.
pub fn path_for(project_path: &Path) -> PathBuf {
    project_path.with_extension(EXTENSION)
}

fn hash(value: &Value) -> String {
    // FNV-1a over the JSON text; stable across runs and platforms.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.to_string().bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn kind_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Change => "change",
        EntryKind::Saved => "saved",
        EntryKind::Autosaved => "autosaved",
    }
}

fn kind_from(name: &str) -> EntryKind {
    match name {
        "saved" => EntryKind::Saved,
        "autosaved" => EntryKind::Autosaved,
        _ => EntryKind::Change,
    }
}

impl Timeline {
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut parts = BTreeMap::new();
        let mut store = |value: Value| {
            let key = hash(&value);
            parts.entry(key.clone()).or_insert(value);
            key
        };
        let mut entries = Vec::new();
        for snapshot in &self.entries {
            let Ok(Value::Object(mut meta)) = serde_json::to_value(&snapshot.project) else { continue };
            let screens = take_array(&mut meta, "screens").into_iter().map(&mut store).collect();
            let components = take_array(&mut meta, "components").into_iter().map(&mut store).collect();
            entries.push(Entry {
                label: snapshot.label.clone(),
                time: snapshot.time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
                kind: kind_name(snapshot.kind).to_owned(),
                meta: store(Value::Object(meta)),
                screens,
                components,
            });
        }
        let file = File { version: VERSION, current: self.current, entries, parts };
        let text = serde_json::to_string(&file).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    /// Reads a saved timeline. Unknown or broken files give `None`.
    pub fn load(path: &Path, limit: usize) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let file: File = serde_json::from_str(&text).ok()?;
        if file.version > VERSION {
            return None;
        }
        let part = |key: &String| file.parts.get(key).cloned();
        let mut entries = Vec::new();
        for entry in &file.entries {
            let Some(Value::Object(mut meta)) = part(&entry.meta) else { return None };
            let screens: Option<Vec<Value>> = entry.screens.iter().map(part).collect();
            let components: Option<Vec<Value>> = entry.components.iter().map(part).collect();
            meta.insert("screens".to_owned(), Value::Array(screens?));
            meta.insert("components".to_owned(), Value::Array(components?));
            let mut project: Project = serde_json::from_value(Value::Object(meta)).ok()?;
            project.repair_ids();
            entries.push(Snapshot {
                project,
                label: entry.label.clone(),
                time: UNIX_EPOCH + Duration::from_secs(entry.time),
                kind: kind_from(&entry.kind),
            });
        }
        if entries.is_empty() {
            return None;
        }
        let current = file.current.min(entries.len() - 1);
        let mut timeline = Self { entries, current, limit: limit.max(2) };
        timeline.trim();
        Some(timeline)
    }

    /// Continues a loaded timeline with `project` as opened from disk. If
    /// the file was changed outside LVGLab, that becomes a new step.
    pub fn resume(&mut self, project: &Project, label: impl Into<String>) {
        if self.entries[self.current].project != *project {
            self.push(project, label.into());
        }
    }
}

fn take_array(object: &mut serde_json::Map<String, Value>, key: &str) -> Vec<Value> {
    match object.remove(key) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::WidgetKind;

    #[test]
    fn round_trips_and_shares_parts() {
        let mut project = Project::new("T");
        let second = project.add_screen();
        let mut timeline = Timeline::new(&project, "Start");
        for i in 0..5 {
            let label = project.new_widget(WidgetKind::Label);
            project.insert(second, i, label).unwrap();
            timeline.observe(&project);
        }
        timeline.mark_save(&project, EntryKind::Saved, "Saved");
        let path = std::env::temp_dir().join(format!("lvglab-{}.timeline", std::process::id()));
        timeline.save(&path).unwrap();

        // The first screen never changed: it is stored once.
        let file: File = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let first_screens: std::collections::HashSet<&String> = file.entries.iter().map(|e| &e.screens[0]).collect();
        assert_eq!(first_screens.len(), 1);

        let loaded = Timeline::load(&path, 300).unwrap();
        assert_eq!(loaded.entries().len(), timeline.entries().len());
        assert_eq!(loaded.current(), timeline.current());
        assert_eq!(loaded.entries()[2].project, timeline.entries()[2].project);
        assert_eq!(loaded.entries().last().unwrap().kind, EntryKind::Saved);

        let mut resumed = loaded;
        let before = resumed.entries().len();
        resumed.resume(&project, "Opened");
        assert_eq!(resumed.entries().len(), before, "same state: no new step");
        project.name = "Edited elsewhere".into();
        resumed.resume(&project, "Changed outside");
        assert_eq!(resumed.entries().len(), before + 1);
        std::fs::remove_file(path).ok();
    }
}
