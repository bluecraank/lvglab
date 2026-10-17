//! The change timeline: every edit is a snapshot you can jump to.

use std::time::SystemTime;

use eframe::egui::{self, Color32, Sense, Stroke, vec2};

use crate::app::Studio;
use crate::commands::Command;
use crate::document::EntryKind;
use crate::i18n::{tr, trf};
use crate::visuals::{self, ACCENT, SUCCESS};

const CARD_WIDTH: f32 = 150.0;

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    let current = studio.timeline.current();
    let count = studio.timeline.entries().len();
    ui.horizontal(|ui| {
        if ui.add_enabled(studio.timeline.can_undo(), egui::Button::new("◀")).on_hover_text(tr("Undo")).clicked() {
            studio.push(Command::Undo);
        }
        if ui.add_enabled(studio.timeline.can_redo(), egui::Button::new("▶")).on_hover_text(tr("Redo")).clicked() {
            studio.push(Command::Redo);
        }
        ui.label(trf("Step {} of {}", &[&(current + 1), &count]));
        if current + 1 < count {
            ui.colored_label(visuals::WARNING, tr("A new change discards the later steps."));
        }
    });
    ui.add_space(4.0);
    let muted = visuals::palette(ui.ctx()).muted;
    let scroll_to_current = ui.memory(|m| m.data.get_temp::<usize>(ui.id()).is_none_or(|last| last != count));
    egui::ScrollArea::horizontal().auto_shrink([false, true]).stick_to_right(true).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (index, entry) in studio.timeline.entries().iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(vec2(CARD_WIDTH, 52.0), Sense::click());
                let is_current = index == current;
                let future = index > current;
                let saved = entry.kind != EntryKind::Change;
                let visuals = ui.style().interact(&response);
                let fill = match (is_current, saved) {
                    (true, _) => visuals::ACCENT_SOFT,
                    (false, true) => SUCCESS.gamma_multiply(0.18),
                    (false, false) => visuals.weak_bg_fill,
                };
                ui.painter().rect_filled(rect, 8, fill);
                let stroke = match (is_current, saved) {
                    (true, _) => Stroke::new(1.5_f32, ACCENT),
                    (false, true) => Stroke::new(1.5_f32, SUCCESS),
                    (false, false) => Stroke::new(1.0_f32, visuals.bg_stroke.color),
                };
                ui.painter().rect_stroke(rect, 8, stroke, egui::StrokeKind::Inside);
                let text_color = if future { muted } else { ui.visuals().text_color() };
                let dot = rect.left_center() + vec2(12.0, 0.0);
                if saved {
                    // Save markers: a ring with a check mark.
                    let color = if future { muted } else { SUCCESS };
                    ui.painter().circle_stroke(dot, 5.5, Stroke::new(1.5_f32, color));
                    let check = [dot + vec2(-2.5, 0.0), dot + vec2(-0.5, 2.0), dot + vec2(3.0, -2.0)];
                    ui.painter().line(check.to_vec(), Stroke::new(1.5_f32, color));
                } else {
                    ui.painter().circle_filled(dot, 4.0, if future { muted } else { ACCENT });
                }
                let label = elide(&entry.label, 20);
                ui.painter().text(
                    rect.left_top() + vec2(24.0, 10.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(12.5),
                    text_color,
                );
                ui.painter().text(
                    rect.left_bottom() + vec2(24.0, -10.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("#{} · {}", index + 1, ago(entry.time)),
                    egui::FontId::proportional(10.5),
                    muted,
                );
                let response = response.on_hover_text(&entry.label);
                if response.clicked() && !is_current {
                    studio.commands.push(Command::JumpTo(index));
                }
                if is_current && scroll_to_current {
                    response.scroll_to_me(Some(egui::Align::Max));
                }
                if index + 1 < count {
                    let (line, _) = ui.allocate_exact_size(vec2(10.0, 52.0), Sense::hover());
                    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0_f32, Color32::from_gray(110)));
                }
            }
        });
    });
    ui.memory_mut(|m| m.data.insert_temp(ui.id(), count));
}

fn elide(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(max - 1).collect::<String>())
    }
}

fn ago(time: SystemTime) -> String {
    let seconds = time.elapsed().map_or(0, |d| d.as_secs());
    match seconds {
        0..=9 => tr("now").to_owned(),
        10..=59 => trf("{} s ago", &[&seconds]),
        60..=3599 => trf("{} min ago", &[&(seconds / 60)]),
        _ => trf("{} h ago", &[&(seconds / 3600)]),
    }
}
