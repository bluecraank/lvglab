//! LVGLab: a UI designer for LVGL 8 and 9 on ESP32.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assistant;
mod canvas;
mod commands;
mod dialogs;
mod document;
mod i18n;
mod modal;
mod panels;
mod recorder;
mod settings;
mod simulator;
mod start;
mod stats;
mod tour;
mod visuals;
mod widgets;

use std::path::PathBuf;

/// Command line:
/// `lvglab [project.lvglab | squareline.spj] [--select <widget>] [--dialog <name>] [--panel <name>] [--simulate] [--screenshot out.png]`.
#[derive(Default)]
pub struct LaunchOptions {
    pub file: Option<PathBuf>,
    /// Name of a widget to select after opening.
    pub select: Option<String>,
    /// Opens a dialog at start (`start`, `preferences`, `about`, `new`, `tour`, `tour:<step>`, `discard`, `close`);
    /// used for documentation screenshots.
    pub dialog: Option<String>,
    /// Brings a panel to the front (`assistant`, `style`, ...).
    pub panel: Option<String>,
    /// Starts the simulation with the record overlay.
    pub simulate: bool,
    /// Renders one frame into a PNG and exits (documentation, CI).
    pub screenshot: Option<PathBuf>,
}

impl LaunchOptions {
    fn from_args() -> Self {
        let mut options = Self::default();
        let mut args = std::env::args_os().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--screenshot" {
                options.screenshot = args.next().map(PathBuf::from);
            } else if arg == "--panel" {
                options.panel = args.next().map(|s| s.to_string_lossy().into_owned());
            } else if arg == "--dialog" {
                options.dialog = args.next().map(|s| s.to_string_lossy().into_owned());
            } else if arg == "--simulate" {
                options.simulate = true;
            } else if arg == "--select" {
                options.select = args.next().map(|s| s.to_string_lossy().into_owned());
            } else {
                options.file = Some(PathBuf::from(arg));
            }
        }
        options
    }
}

fn main() -> eframe::Result {
    let options = LaunchOptions::from_args();
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("LVGLab")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let native = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("LVGLab", native, Box::new(|cc| Ok(Box::new(app::StudioApp::new(cc, options)))))
}
