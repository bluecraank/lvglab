//! The welcome tour: 15 steps that highlight one area of the editor each and
//! explain it in a bubble next to it. It starts on the first launch and can
//! be repeated from Help → Welcome tour.

use std::collections::HashMap;

use eframe::egui::{self, Align2, Color32, Id, Key, Order, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, vec2};

use crate::app::Studio;
use crate::i18n::{tr, trf};
use crate::panels::Panel;
use crate::visuals::{self, ACCENT};

/// A part of the window a step points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Anchor {
    Panel(Panel),
    FileButtons,
    ProjectSettings,
    Simulate,
    Export,
}

/// Where the areas were drawn in the current frame.
pub type Anchors = HashMap<Anchor, Rect>;

pub struct Step {
    pub anchor: Option<Anchor>,
    pub title: &'static str,
    pub text: &'static str,
}

pub const STEPS: [Step; 15] = [
    Step {
        anchor: None,
        title: "Welcome to LVGLab",
        text: "LVGLab designs user interfaces for LVGL on microcontrollers such as the ESP32 and exports them as C code. This tour shows the main areas in 15 steps. You can leave it at any time and repeat it from the Help menu.",
    },
    Step {
        anchor: Some(Anchor::FileButtons),
        title: "Projects",
        text: "New creates a project and asks for the display and the LVGL version. Open and Save work with .lvglab files; SquareLine projects are imported from the File menu. The arrows undo and redo every change (Ctrl+Z, Ctrl+Y).",
    },
    Step {
        anchor: Some(Anchor::ProjectSettings),
        title: "Project settings",
        text: "Name, resolution and shape of the display, the LVGL version (8.3 or 9.x), the default theme and the settings of the library export. Switching the LVGL version removes what that release does not support.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Screens)),
        title: "Screens",
        text: "Every screen of your UI. The first one is loaded at start. Add one with + Screen; right-click a screen to reorder, duplicate or delete it.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Widgets)),
        title: "Widgets",
        text: "All LVGL widgets, grouped by category. Click one to add it to the selected screen or container, or drag it onto the canvas to place it.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Canvas)),
        title: "Canvas",
        text: "The current screen as it looks on the device. Click to select, drag to move and use the handles to resize. Guides snap to the neighbors; Ctrl + mouse wheel zooms.",
    },
    Step {
        anchor: Some(Anchor::Simulate),
        title: "Simulate",
        text: "Runs the UI in the editor: buttons, switches and sliders react, and events such as screen changes are executed. F5 starts and stops the simulation.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Layers)),
        title: "Layers",
        text: "The widget tree of the current screen. Drag rows to reorder them or to move widgets into containers; the eye hides a widget.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Parameters)),
        title: "Parameters",
        text: "Name, position and size (pixels, percent or content), alignment and the properties of the selected widget, plus layout, flags, states and scrolling.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Style)),
        title: "Style",
        text: "Local styles per part (e.g. indicator or knob) and state (pressed, checked, …): colors, gradients, borders, shadows, fonts and more. The canvas previews the selected state.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Events)),
        title: "Events",
        text: "What happens on a click, a value change or a gesture: change the screen, set flags or states, change texts and values, or call your own C function.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Themes)),
        title: "Themes",
        text: "The LVGL default theme (dark or light, primary and secondary color) and color themes: named colors with one value per theme that the device switches with ui_theme_set().",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Timeline)),
        title: "Timeline",
        text: "Every change is a step. Click a step to go back to it; saves and autosaves are marked. The timeline is kept next to the project file.",
    },
    Step {
        anchor: Some(Anchor::Panel(Panel::Fonts)),
        title: "Fonts and images",
        text: "Add TTF/OTF fonts in any size and character range, and PNG or JPEG images in the tab next to it. The export converts them to C arrays; saving copies them into the project's assets folder.",
    },
    Step {
        anchor: Some(Anchor::Export),
        title: "Export the library",
        text: "Writes the UI as a library for PlatformIO or the Arduino IDE; your own code in ui_events.c is never overwritten. Tip: the AI assistant tab builds screens from a description. Have fun!",
    },
];

/// The panel the current step shows, so its tab can be brought to the front.
pub fn current_panel(studio: &Studio) -> Option<Panel> {
    match STEPS[studio.tour?].anchor {
        Some(Anchor::Panel(panel)) => Some(panel),
        _ => None,
    }
}

/// Draws the dimmed backdrop with the highlighted area and the bubble.
pub fn show(ctx: &egui::Context, studio: &mut Studio) {
    let Some(index) = studio.tour else { return };
    let index = index.min(STEPS.len() - 1);
    let step = &STEPS[index];
    let screen = ctx.screen_rect();
    let target = step
        .anchor
        .and_then(|a| studio.anchors.get(&a))
        .map(|r| r.expand(6.0).intersect(screen.shrink(2.0)))
        .filter(|r| r.width() > 4.0 && r.height() > 4.0);

    egui::Area::new(Id::new("tour_backdrop")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        // Swallows clicks so the editor stays as the tour shows it.
        ui.allocate_rect(screen, Sense::click_and_drag());
        let painter = ui.painter();
        let dim = Color32::from_black_alpha(if ui.visuals().dark_mode { 160 } else { 110 });
        match target {
            Some(t) => {
                for part in [
                    Rect::from_min_max(screen.min, Pos2::new(screen.right(), t.top())),
                    Rect::from_min_max(Pos2::new(screen.left(), t.bottom()), screen.max),
                    Rect::from_min_max(Pos2::new(screen.left(), t.top()), Pos2::new(t.left(), t.bottom())),
                    Rect::from_min_max(Pos2::new(t.right(), t.top()), Pos2::new(screen.right(), t.bottom())),
                ] {
                    painter.rect_filled(part, 0, dim);
                }
                painter.rect_stroke(t, 8, Stroke::new(2.5_f32, ACCENT), StrokeKind::Outside);
            }
            None => {
                painter.rect_filled(screen, 0, dim);
            }
        }
    });

    const WIDTH: f32 = 360.0;
    const GAP: f32 = 18.0;
    let (pivot, pos) = match target {
        None => (Align2::CENTER_CENTER, screen.center()),
        Some(t) if screen.right() - t.right() > WIDTH + 2.0 * GAP + 40.0 => {
            (Align2::LEFT_CENTER, Pos2::new(t.right() + GAP, t.center().y))
        }
        Some(t) if t.left() - screen.left() > WIDTH + 2.0 * GAP + 40.0 => {
            (Align2::RIGHT_CENTER, Pos2::new(t.left() - GAP, t.center().y))
        }
        Some(t) if screen.bottom() - t.bottom() > 240.0 => (Align2::CENTER_TOP, t.center_bottom() + vec2(0.0, GAP)),
        Some(t) => (Align2::CENTER_BOTTOM, t.center_top() - vec2(0.0, GAP)),
    };

    let mut go: Option<isize> = None;
    let mut finish = false;
    egui::Area::new(Id::new("tour_bubble")).order(Order::Tooltip).pivot(pivot).fixed_pos(pos).constrain(true).show(
        ctx,
        |ui| {
            egui::Frame::new()
                .fill(ui.visuals().window_fill)
                .stroke(Stroke::new(1.5_f32, ACCENT))
                .corner_radius(12)
                .inner_margin(18)
                .shadow(egui::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(100) })
                .show(ui, |ui| {
                    ui.set_width(WIDTH);
                    let muted = visuals::palette(ui.ctx()).muted;
                    ui.label(RichText::new(trf("Step {} of {}", &[&(index + 1), &STEPS.len()])).small().color(muted));
                    ui.add_space(2.0);
                    ui.label(RichText::new(tr(step.title)).size(18.0).strong());
                    ui.add_space(6.0);
                    ui.label(tr(step.text));
                    ui.add_space(12.0);
                    progress(ui, index);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let last = index + 1 == STEPS.len();
                        if !last && ui.button(tr("Skip tour")).clicked() {
                            finish = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let next = egui::Button::new(
                                RichText::new(if last { tr("Done") } else { tr("Next") })
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(ACCENT)
                            .corner_radius(8)
                            .min_size(vec2(90.0, 30.0));
                            if ui.add(next).clicked() {
                                go = Some(1);
                            }
                            if index > 0 && ui.button(tr("Back")).clicked() {
                                go = Some(-1);
                            }
                        });
                    });
                });
        },
    );

    ctx.input(|i| {
        if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Enter) {
            go = Some(1);
        } else if i.key_pressed(Key::ArrowLeft) {
            go = Some(-1);
        } else if i.key_pressed(Key::Escape) {
            finish = true;
        }
    });
    if let Some(delta) = go {
        let next = index as isize + delta;
        if next >= STEPS.len() as isize {
            finish = true;
        } else {
            studio.tour = Some(next.max(0) as usize);
        }
    }
    if finish {
        studio.tour = None;
        studio.settings.tour_done = true;
    }
}

/// One dot per step; the current one is wider.
fn progress(ui: &mut egui::Ui, index: usize) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    let painter = ui.painter();
    let mut x = rect.left();
    for i in 0..STEPS.len() {
        let width = if i == index { 22.0 } else { 8.0 };
        let color = if i <= index { ACCENT } else { ui.visuals().widgets.inactive.bg_fill };
        painter.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top()), vec2(width, 8.0)), 4, color);
        x += width + 5.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_is_translated() {
        for language in [crate::i18n::Language::German, crate::i18n::Language::Spanish, crate::i18n::Language::French] {
            let table = language.table().unwrap();
            for step in &STEPS {
                assert!(table.contains_key(step.title), "{language:?} lacks {:?}", step.title);
                assert!(table.contains_key(step.text), "{language:?} lacks {:?}", step.text);
            }
        }
    }
}
