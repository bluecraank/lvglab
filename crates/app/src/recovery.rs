//! Recovery copies of projects that were never saved. Autosave cannot write
//! those to a project file, so it writes them to `recovery/` in the data
//! folder of the editor. Saving, closing the tab or quitting the editor
//! removes the copy; one left over after a crash is offered on the next start.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use lvglab_model::Project;
use lvglab_model::project::PROJECT_EXTENSION;

/// A recovery copy found on start.
pub struct Found {
    pub path: PathBuf,
    pub name: String,
    pub modified: Option<SystemTime>,
}

#[cfg(test)]
static TEST_DIR: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Lets a test use `dir` instead of the data folder.
#[cfg(test)]
pub fn use_dir_for_test(dir: &Path) {
    *TEST_DIR.lock().unwrap() = Some(dir.to_path_buf());
}

/// The folder of the recovery copies (next to the settings; `LVGLAB_DATA_DIR`
/// moves both). `None` in the sandbox.
fn dir() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(dir) = TEST_DIR.lock().ok().and_then(|d| d.clone()) {
        return Some(dir);
    }
    if crate::sandbox::active() {
        return None;
    }
    let base = std::env::var_os("LVGLAB_DATA_DIR").map(PathBuf::from).or_else(|| eframe::storage_dir("LVGLab"))?;
    Some(base.join("recovery"))
}

/// A new file name for the recovery copy of one document.
pub fn new_path() -> Option<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    Some(dir()?.join(format!("untitled-{stamp}-{}-{n}.{PROJECT_EXTENSION}", std::process::id())))
}

pub fn save(path: &Path, project: &Project) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    project.save(path).map_err(|e| e.to_string())
}

pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Recovery copies left by earlier sessions, newest first.
pub fn found() -> Vec<Found> {
    let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) else { return Vec::new() };
    let mut found: Vec<Found> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == PROJECT_EXTENSION))
        .filter_map(|path| {
            let project = Project::load(&path).ok()?;
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            Some(Found { path, name: project.name, modified })
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.modified));
    found
}
