//! The start page: recent projects and the ways to begin, shown when the
//! editor starts without a project.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Sense, Stroke, Vec2, vec2};
use lvglab_model::Project;
use lvglab_model::project::DisplayShape;

use crate::app::Studio;
use crate::commands::Command;
use crate::i18n::{tr, trf};
use crate::visuals::{self, ACCENT};

/// One recent project as the start page lists it.
pub struct Entry {
    pub path: PathBuf,
    /// Name, display and release; `None` if the file is missing or unreadable.
    pub info: Option<Info>,
    pub modified: Option<SystemTime>,
}

pub struct Info {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub round: bool,
    pub version: &'static str,
    pub screens: usize,
}

/// Reads the recent projects once when the page opens.
pub fn entries(recent: &[PathBuf]) -> Vec<Entry> {
    recent
        .iter()
        .map(|path| {
            let info = Project::load(path).ok().map(|p| Info {
                name: p.name.clone(),
                width: p.display.width,
                height: p.display.height,
                round: p.display.shape == DisplayShape::Round,
                version: p.lvgl_version.suffix(),
                screens: p.screens.len(),
            });
            let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            Entry { path: path.clone(), info, modified }
        })
        .collect()
}

pub fn show(ctx: &egui::Context, studio: &mut Studio) {
    let Some(entries) = &studio.start_page else { return };
    let mut chosen: Option<Command> = None;
    let mut remove: Option<PathBuf> = None;
    let mut close = false;
    let palette = visuals::palette(ctx);
    let frame = Frame::new()
        .fill(ctx.style().visuals.window_fill)
        .stroke(ctx.style().visuals.window_stroke)
        .corner_radius(16)
        .inner_margin(Margin::same(28))
        .shadow(egui::Shadow { offset: [0, 16], blur: 48, spread: 0, color: Color32::from_black_alpha(110) });
    let backdrop = Color32::from_black_alpha(if ctx.style().visuals.dark_mode { 170 } else { 110 });
    egui::Modal::new(egui::Id::new("start_page")).frame(frame).backdrop_color(backdrop).show(ctx, |ui| {
        ui.set_width(780.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("LVGLab").size(26.0).strong().color(ACCENT));
            ui.label(RichText::new(env!("CARGO_PKG_VERSION")).color(palette.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("✖").on_hover_text(tr("Close")).clicked() {
                    close = true;
                }
            });
        });
        ui.label(RichText::new(tr("UI designer for LVGL 8 and 9 on ESP32.")).color(palette.muted));
        ui.add_space(18.0);
        ui.horizontal_top(|ui| {
            // Ways to begin.
            ui.vertical(|ui| {
                ui.set_width(220.0);
                ui.label(RichText::new(tr("Start")).strong());
                ui.add_space(6.0);
                let mut action = |ui: &mut egui::Ui, icon: &str, text: &str, command: Command| {
                    let button = egui::Button::new(RichText::new(format!("{icon}  {text}")).size(15.0))
                        .min_size(vec2(220.0, 40.0))
                        .corner_radius(8);
                    if ui.add(button).clicked() {
                        chosen = Some(command);
                    }
                };
                action(ui, "➕", tr("New project…"), Command::NewProject);
                action(ui, "📂", tr("Open…"), Command::Open);
                action(ui, "⬇", tr("Import SquareLine project…"), Command::ImportSquareLine);
            });
            ui.add_space(24.0);
            // Recent projects.
            ui.vertical(|ui| {
                ui.label(RichText::new(tr("Recent projects")).strong());
                ui.add_space(6.0);
                if entries.is_empty() {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(tr("No recent projects yet. Create a project or open one.")).color(palette.muted),
                    );
                    return;
                }
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    for entry in entries {
                        match row(ui, entry) {
                            RowAction::Open => chosen = Some(Command::OpenPath(entry.path.clone())),
                            RowAction::Remove => remove = Some(entry.path.clone()),
                            RowAction::None => {}
                        }
                    }
                });
            });
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if let Some(path) = remove {
        studio.settings.recent.retain(|p| p != &path);
        if let Some(entries) = &mut studio.start_page {
            entries.retain(|e| e.path != path);
        }
    }
    if let Some(command) = chosen {
        studio.push(command);
        close = true;
    }
    if close {
        studio.start_page = None;
    }
}

enum RowAction {
    None,
    Open,
    Remove,
}

fn row(ui: &mut egui::Ui, entry: &Entry) -> RowAction {
    let palette = visuals::palette(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 8, ui.visuals().widgets.hovered.bg_fill);
    }
    let mut action = RowAction::None;
    let inner = rect.shrink2(vec2(10.0, 8.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)));
    // A small picture of the display shape.
    let (icon, _) = child.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
    let stroke = Stroke::new(2.0_f32, if entry.info.is_some() { ACCENT } else { palette.muted });
    match &entry.info {
        Some(info) if info.round => {
            child.painter().circle_stroke(icon.center(), 15.0, stroke);
        }
        Some(info) => {
            let aspect = info.width as f32 / info.height.max(1) as f32;
            let size = if aspect >= 1.0 { vec2(30.0, 30.0 / aspect) } else { vec2(30.0 * aspect, 30.0) };
            child.painter().rect_stroke(
                egui::Rect::from_center_size(icon.center(), size),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        None => {
            child.painter().text(
                icon.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(18.0),
                palette.muted,
            );
        }
    }
    child.add_space(10.0);
    child.vertical(|ui| {
        let name = entry.info.as_ref().map_or_else(
            || entry.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            |i| i.name.clone(),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).strong().size(15.0));
            let details = match &entry.info {
                Some(i) => trf("{} × {} · LVGL {} · {} screens", &[&i.width, &i.height, &i.version, &i.screens]),
                None => tr("File not found").to_owned(),
            };
            let color = if entry.info.is_some() { palette.muted } else { visuals::WARNING };
            ui.label(RichText::new(details).small().color(color));
        });
        ui.horizontal(|ui| {
            let mut text = entry.path.display().to_string();
            if let Some(time) = entry.modified {
                text = format!("{} · {text}", crate::panels::ago(time));
            }
            ui.add(egui::Label::new(RichText::new(text).small().color(palette.muted)).truncate())
                .on_hover_text(entry.path.display().to_string());
        });
    });
    child.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if (hovered || entry.info.is_none()) && ui.small_button("✖").on_hover_text(tr("Remove from the list")).clicked()
        {
            action = RowAction::Remove;
        }
    });
    if matches!(action, RowAction::None) && response.clicked() && entry.info.is_some() {
        action = RowAction::Open;
    }
    action
}
