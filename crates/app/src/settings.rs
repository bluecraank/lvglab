//! Per-user application preferences, persisted by eframe.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::i18n::Language;

pub const STORAGE_KEY: &str = "lvglab.settings";
const MAX_RECENT: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Dark,
    Light,
}

impl Appearance {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

/// What is drawn around the display on the canvas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasBackground {
    /// Follows the editor appearance.
    #[default]
    Theme,
    Dark,
    Light,
    Checkerboard,
}

impl CanvasBackground {
    pub const ALL: [Self; 4] = [Self::Theme, Self::Dark, Self::Light, Self::Checkerboard];

    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "Like the editor",
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Checkerboard => "Checkerboard",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    // General
    pub language: Language,
    pub appearance: Appearance,
    /// Scale of the whole editor UI (1.0 = 100 %).
    pub ui_scale: f32,
    /// Opens the most recent project when started without a file.
    pub reopen_last: bool,
    // Canvas
    pub canvas_background: CanvasBackground,
    pub grid_size: u32,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    /// Thin outlines around every widget.
    pub show_bounds: bool,
    /// Dashed outlines for hidden widgets.
    pub show_hidden: bool,
    /// Name and size next to the selection.
    pub show_size_label: bool,
    /// Pixels moved by Shift + arrow key.
    pub nudge_large: u32,
    // Editing
    /// Maximum number of steps in the timeline.
    pub history_limit: usize,
    /// Minutes between automatic backups (0 = off).
    pub autosave_minutes: u32,
    // Export
    pub open_export_folder: bool,
    pub recent: Vec<PathBuf>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::from_system(),
            appearance: Appearance::System,
            ui_scale: 1.0,
            reopen_last: false,
            canvas_background: CanvasBackground::Theme,
            grid_size: 10,
            show_grid: false,
            snap_to_grid: false,
            show_bounds: false,
            show_hidden: true,
            show_size_label: true,
            nudge_large: 10,
            history_limit: 300,
            autosave_minutes: 5,
            open_export_folder: false,
            recent: Vec::new(),
        }
    }
}

impl AppSettings {
    pub fn add_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }

    /// Restores the defaults but keeps the list of recent files.
    pub fn reset(&mut self) {
        let recent = std::mem::take(&mut self.recent);
        *self = Self { recent, ..Self::default() };
    }

    /// Clamps values that may come from an older or edited settings file.
    pub fn sanitize(&mut self) {
        self.ui_scale = self.ui_scale.clamp(0.6, 2.0);
        self.grid_size = self.grid_size.clamp(2, 200);
        self.nudge_large = self.nudge_large.clamp(2, 200);
        self.history_limit = self.history_limit.clamp(20, 5000);
        self.autosave_minutes = self.autosave_minutes.min(120);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_keeps_recent_files() {
        let mut settings = AppSettings { show_bounds: true, ..AppSettings::default() };
        settings.add_recent(PathBuf::from("a.lvglab"));
        settings.reset();
        assert!(!settings.show_bounds);
        assert_eq!(settings.recent.len(), 1);
    }

    #[test]
    fn sanitize_clamps_values() {
        let mut settings = AppSettings { ui_scale: 9.0, history_limit: 1, ..AppSettings::default() };
        settings.sanitize();
        assert_eq!(settings.ui_scale, 2.0);
        assert_eq!(settings.history_limit, 20);
    }

    #[test]
    fn old_settings_files_still_load() {
        let settings: AppSettings = serde_json::from_str(r#"{"grid_size": 8}"#).unwrap();
        assert_eq!(settings.grid_size, 8);
        assert_eq!(settings.history_limit, 300);
    }
}
