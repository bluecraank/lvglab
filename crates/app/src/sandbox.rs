//! Side effects of the editor that tests must not trigger: native file
//! dialogs, network requests and starting other programs. The fuzz tests
//! switch the sandbox on; the dialogs then behave as if they were cancelled.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// True while side effects are disabled.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

#[cfg(test)]
pub fn enable() {
    ACTIVE.store(true, Ordering::Relaxed);
}

/// `rfd::FileDialog` that is never shown in the sandbox.
pub struct FileDialog(rfd::FileDialog);

impl FileDialog {
    pub fn new() -> Self {
        Self(rfd::FileDialog::new())
    }

    pub fn add_filter(self, name: impl Into<String>, extensions: &[impl ToString]) -> Self {
        Self(self.0.add_filter(name, extensions))
    }

    pub fn set_title(self, title: impl Into<String>) -> Self {
        Self(self.0.set_title(title))
    }

    pub fn set_file_name(self, name: impl Into<String>) -> Self {
        Self(self.0.set_file_name(name))
    }

    pub fn set_directory(self, path: impl AsRef<Path>) -> Self {
        Self(self.0.set_directory(path))
    }

    pub fn pick_file(self) -> Option<PathBuf> {
        if active() { None } else { self.0.pick_file() }
    }

    pub fn pick_files(self) -> Option<Vec<PathBuf>> {
        if active() { None } else { self.0.pick_files() }
    }

    pub fn pick_folder(self) -> Option<PathBuf> {
        if active() { None } else { self.0.pick_folder() }
    }

    pub fn save_file(self) -> Option<PathBuf> {
        if active() { None } else { self.0.save_file() }
    }
}
