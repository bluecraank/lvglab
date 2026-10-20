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
                if close_button(ui, 32.0).on_hover_text(tr("Close")).clicked() {
                    close = true;
                }
            });
        });
        ui.label(RichText::new(tr("UI designer for LVGL 8 and 9 on ESP32.")).color(palette.muted));
        ui.add_space(18.0);
        ui.horizontal_top(|ui| {
            // Ways to begin.
            ui.vertical(|ui| {
                ui.set_width(250.0);
                ui.label(RichText::new(tr("Start")).strong());
                ui.add_space(6.0);
                let mut action = |ui: &mut egui::Ui, icon: &str, text: &str, command: Command| {
                    let button = egui::Button::new(RichText::new(format!("{icon}  {text}")).size(15.0))
                        .wrap_mode(egui::TextWrapMode::Truncate)
                        .min_size(vec2(250.0, 40.0))
                        .corner_radius(8);
                    if ui.add(button).clicked() {
                        chosen = Some(command);
                    }
                };
                action(ui, crate::icons::PLUS, tr("New project…"), Command::NewProject);
                action(ui, crate::icons::FOLDER_OPEN, tr("Open…"), Command::Open);
                action(ui, crate::icons::DOWNLOAD_SIMPLE, tr("Import SquareLine project…"), Command::ImportSquareLine);
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
        // If the action is cancelled, the start page comes back.
        studio.return_to_start = true;
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
    // The texts inside must not catch the pointer: the whole row is one target.
    let hovered = ui.rect_contains_pointer(rect);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if hovered {
        ui.painter().rect_filled(rect, 8, ui.visuals().widgets.hovered.bg_fill);
    }
    let mut action = RowAction::None;
    let inner = rect.shrink2(vec2(10.0, 8.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)));
    child.style_mut().interaction.selectable_labels = false;
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
        if (hovered || entry.info.is_none())
            && close_button(ui, 28.0).on_hover_text(tr("Remove from the list")).clicked()
        {
            action = RowAction::Remove;
        }
    });
    let clicked = response.clicked() || (hovered && ui.input(|i| i.pointer.primary_clicked()));
    if matches!(action, RowAction::None) && clicked && entry.info.is_some() {
        action = RowAction::Open;
    }
    action
}

/// A square button with a drawn cross (glyph buttons come out small and uneven).
fn close_button(ui: &mut egui::Ui, side: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(rect, 6, visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    let c = rect.center();
    let d = side * 0.2;
    let stroke = egui::Stroke::new(2.0_f32, visuals.fg_stroke.color);
    ui.painter().line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
    ui.painter().line_segment([c + egui::vec2(-d, d), c + egui::vec2(d, -d)], stroke);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;
    use eframe::egui::{Event, PointerButton, RawInput, pos2};

    fn entries() -> Vec<Entry> {
        let info =
            |name: &str| Info { name: name.into(), width: 466, height: 466, round: true, version: "9.6.0", screens: 2 };
        vec![Entry { path: "a.lvglab".into(), info: Some(info("Dashboard")), modified: None }]
    }

    /// Whether a click at `(x, y)` opens the project.
    fn opens(x: f32, y: f32) -> bool {
        let ctx = egui::Context::default();
        crate::visuals::install(&ctx);
        let mut studio = Studio::new(AppSettings::default());
        studio.start_page = Some(entries());
        let frame = |studio: &mut Studio, events: Vec<Event>| {
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1600.0, 900.0))),
                events,
                ..RawInput::default()
            };
            let _ = ctx.run(input, |ctx| show(ctx, studio));
        };
        let at = pos2(x, y);
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut studio, vec![Event::PointerMoved(at)]);
        frame(&mut studio, vec![button(true)]);
        frame(&mut studio, vec![button(false)]);
        studio.commands.iter().any(|c| matches!(c, Command::OpenPath(_)))
    }

    #[test]
    fn a_whole_row_opens_its_project() {
        // Sweep a horizontal line through the row: where it opens, it opens everywhere.
        let rows: Vec<f32> = (380..520).step_by(4).map(|y| y as f32).collect();
        let y = rows
            .iter()
            .copied()
            .find(|y| (700..1100).step_by(8).any(|x| opens(x as f32, *y)))
            .expect("some click opens the project");
        let hits: Vec<i32> = (500..1300).step_by(4).filter(|x| opens(*x as f32, y)).collect();
        let (first, last) = (hits[0], *hits.last().unwrap());
        let gaps: Vec<i32> = (first..=last).step_by(4).filter(|x| !hits.contains(x)).collect();
        assert!(last - first > 300, "the row is wide: {first}..{last}");
        assert!(gaps.is_empty(), "parts of the row do not react to clicks: {gaps:?}");
    }
}
