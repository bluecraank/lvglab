//! The change timeline: every edit is a snapshot you can jump to.

use std::time::SystemTime;

use eframe::egui::{self, Color32, Sense, Stroke, vec2};

use crate::app::Studio;
use crate::commands::Command;
use crate::document::EntryKind;
use crate::i18n::{tr, trf};
use crate::visuals::{self, ACCENT, SUCCESS};

const CHIP_HEIGHT: f32 = 28.0;
const CHIP_MIN: f32 = 70.0;
const CHIP_MAX: f32 = 240.0;

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    let current = studio.timeline.current();
    let count = studio.timeline.entries().len();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(studio.timeline.can_undo(), egui::Button::new(crate::icons::CARET_LEFT))
            .on_hover_text(tr("Undo"))
            .clicked()
        {
            studio.push(Command::Undo);
        }
        if ui
            .add_enabled(studio.timeline.can_redo(), egui::Button::new(crate::icons::CARET_RIGHT))
            .on_hover_text(tr("Redo"))
            .clicked()
        {
            studio.push(Command::Redo);
        }
        ui.label(trf("Step {} of {}", &[&(current + 1), &count]));
        ui.checkbox(&mut studio.timeline_this_screen, tr("This screen only"))
            .on_hover_text(tr("Shows the steps that changed this screen. A click restores only this screen."));
        if current + 1 < count && !studio.timeline_this_screen {
            ui.colored_label(visuals::WARNING, tr("A new change discards the later steps."));
        }
    });
    ui.add_space(4.0);
    let muted = visuals::palette(ui.ctx()).muted;
    let scroll_to_current = ui.memory(|m| m.data.get_temp::<usize>(ui.id()).is_none_or(|last| last != count));
    // The mouse wheel scrolls the steps sideways.
    if ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
        ui.ctx().input_mut(|i| {
            let delta = i.smooth_scroll_delta;
            if delta.x == 0.0 && delta.y != 0.0 {
                i.smooth_scroll_delta = vec2(delta.y, 0.0);
            }
        });
    }
    egui::ScrollArea::horizontal().auto_shrink([false, true]).stick_to_right(true).show(ui, |ui| {
        ui.horizontal(|ui| {
            let screen = studio.screen;
            let entries = studio.timeline.entries();
            let this_screen = studio.timeline_this_screen;
            // Steps that changed the screen, and the one that matches its state now.
            let changed =
                |i: usize| i == 0 || entries[i].project.widget(screen) != entries[i - 1].project.widget(screen);
            let visible: Vec<usize> = (0..count).filter(|&i| !this_screen || changed(i)).collect();
            let shown_current = visible.iter().copied().rfind(|&i| i <= current).unwrap_or(current);
            for (position, &index) in visible.iter().enumerate() {
                let entry = &entries[index];
                let is_current = index == if this_screen { shown_current } else { current };
                let future = index > current;
                let saved = entry.kind != EntryKind::Change;
                // A chip as wide as its text: number badge, then the label.
                let label = elide(&entry.label, 28);
                let font = egui::FontId::proportional(12.5);
                let text_color = if is_current {
                    Color32::WHITE
                } else if future {
                    muted
                } else {
                    ui.visuals().text_color()
                };
                let galley = ui.painter().layout_no_wrap(label, font, text_color);
                let width = (galley.size().x + 40.0).clamp(CHIP_MIN, CHIP_MAX);
                let (rect, response) = ui.allocate_exact_size(vec2(width, CHIP_HEIGHT), Sense::click());
                let hovered = response.hovered();
                let fill = if is_current {
                    ACCENT
                } else if hovered {
                    ui.visuals().widgets.hovered.weak_bg_fill
                } else {
                    ui.visuals().widgets.inactive.weak_bg_fill
                };
                let painter = ui.painter();
                painter.rect_filled(rect, CHIP_HEIGHT / 2.0, fill);
                if saved && !is_current {
                    painter.rect_stroke(
                        rect,
                        CHIP_HEIGHT / 2.0,
                        Stroke::new(1.5_f32, SUCCESS),
                        egui::StrokeKind::Inside,
                    );
                }
                // The badge: the step number, or a check mark for saves.
                let badge = rect.left_center() + vec2(CHIP_HEIGHT / 2.0, 0.0);
                let badge_color = if is_current {
                    Color32::WHITE
                } else if saved {
                    SUCCESS
                } else if future {
                    muted
                } else {
                    ACCENT
                };
                if saved {
                    let check = [badge + vec2(-4.0, 0.0), badge + vec2(-1.0, 3.0), badge + vec2(4.5, -3.0)];
                    painter.line(check.to_vec(), Stroke::new(2.0_f32, badge_color));
                } else {
                    painter.circle_filled(badge, 4.0, badge_color);
                }
                let text_pos = egui::pos2(rect.left() + CHIP_HEIGHT - 2.0, rect.center().y - galley.size().y / 2.0);
                painter.galley(text_pos, galley, text_color);
                let response = response.on_hover_text(format!("#{} · {}", index + 1, entry.label));
                if response.clicked() && !is_current {
                    studio.commands.push(if this_screen {
                        Command::RestoreScreen(index)
                    } else {
                        Command::JumpTo(index)
                    });
                }
                if is_current && scroll_to_current {
                    response.scroll_to_me(Some(egui::Align::Max));
                }
                if position + 1 < visible.len() {
                    let (line, _) = ui.allocate_exact_size(vec2(12.0, CHIP_HEIGHT), Sense::hover());
                    ui.painter().hline(
                        line.x_range(),
                        line.center().y,
                        Stroke::new(1.5_f32, muted.gamma_multiply(0.6)),
                    );
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

/// "now", "5 min ago", "3 h ago", "2 d ago".
pub fn ago(time: SystemTime) -> String {
    let seconds = time.elapsed().map_or(0, |d| d.as_secs());
    match seconds {
        0..=9 => tr("now").to_owned(),
        10..=59 => trf("{} s ago", &[&seconds]),
        60..=3599 => trf("{} min ago", &[&(seconds / 60)]),
        3600..=172_799 => trf("{} h ago", &[&(seconds / 3600)]),
        _ => trf("{} d ago", &[&(seconds / 86_400)]),
    }
}
