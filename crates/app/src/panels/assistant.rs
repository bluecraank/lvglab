//! The AI assistant panel: describe a UI, Claude builds it.

use eframe::egui::{self, Color32, Frame, Key, Margin, RichText};

use crate::app::Studio;
use crate::assistant::{Outcome, Target, effort_label};
use crate::i18n::{tr, trf};
use crate::visuals::{ACCENT, DANGER, WARNING};
use crate::widgets::muted;

const EXAMPLES: &[&str] = &[
    "Settings screen with three switches (Wi-Fi, Bluetooth, Night mode) and a back button",
    "Dashboard with a speed arc in the center and two values below",
    "PIN entry with a text area and a numeric keyboard",
    "Make all buttons rounder and use a dark blue background",
];

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    let model = lvglab_ai::MODELS.iter().find(|(id, _)| *id == studio.settings.ai_model).map_or("", |(_, name)| *name);
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("AI assistant")).strong());
        muted(ui, format!("{model} · {}", tr(effort_label(&studio.settings.ai_effort))));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(crate::icons::GEAR).on_hover_text(tr("Preferences")).clicked() {
                studio.show_preferences = true;
            }
            if !studio.assistant.history.is_empty() && ui.small_button(tr("Clear")).clicked() {
                studio.assistant.history.clear();
            }
        });
    });
    ui.separator();

    // The composer stays at the bottom; the conversation scrolls above it.
    egui::TopBottomPanel::bottom("assistant_composer")
        .frame(Frame::new().inner_margin(Margin::symmetric(0, 6)))
        .show_inside(ui, |ui| composer(ui, studio));
    if studio.settings.api_key().is_empty() {
        egui::TopBottomPanel::top("assistant_key_hint").frame(Frame::new()).show_inside(ui, |ui| key_hint(ui, studio));
    }
    egui::CentralPanel::default().frame(Frame::new()).show_inside(ui, |ui| {
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| history(ui, studio));
    });
}

fn key_hint(ui: &mut egui::Ui, studio: &mut Studio) {
    Frame::new().fill(ACCENT.gamma_multiply(0.12)).corner_radius(8).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(tr("Connect the assistant")).strong());
        ui.label(tr("The assistant uses Claude by Anthropic. Enter your API key in the preferences, or set the environment variable ANTHROPIC_API_KEY."));
        ui.horizontal(|ui| {
            if ui.button(tr("Open preferences")).clicked() {
                studio.show_preferences = true;
            }
            ui.hyperlink_to(tr("Get an API key"), "https://console.anthropic.com/settings/keys");
        });
    });
    ui.add_space(6.0);
}

fn history(ui: &mut egui::Ui, studio: &mut Studio) {
    if studio.assistant.history.is_empty() {
        ui.add_space(8.0);
        muted(
            ui,
            tr(
                "Describe a screen or a change. The assistant builds it in the current project; every result is a step in the timeline, so you can undo it.",
            ),
        );
        ui.add_space(8.0);
        for example in EXAMPLES {
            let text = tr(example);
            if ui.add(egui::Button::new(RichText::new(text).small()).wrap()).clicked() {
                studio.assistant.input = text.to_owned();
            }
        }
        return;
    }
    let progress = studio.assistant.progress();
    let mut retry = None;
    for (index, exchange) in studio.assistant.history.iter().enumerate() {
        bubble(ui, &exchange.request);
        ui.add_space(4.0);
        match &exchange.outcome {
            Outcome::Running => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    let (received, seconds) = progress.unwrap_or_default();
                    if received == 0 {
                        ui.label(trf("Thinking… {} s", &[&seconds]));
                    } else {
                        ui.label(trf("Building the interface… {} characters", &[&received]));
                    }
                });
            }
            Outcome::Done { summary, warnings, usage, model } => {
                answer_card(ui, summary, warnings, usage, model);
            }
            Outcome::Failed(message) => {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(DANGER, message);
                    if ui.small_button(tr("Try again")).clicked() {
                        retry = Some(index);
                    }
                });
            }
            Outcome::Cancelled => {
                muted(ui, tr("Cancelled."));
            }
        }
        ui.add_space(10.0);
    }
    if let Some(index) = retry {
        studio.assistant.input = studio.assistant.history[index].request.clone();
    }
}

fn bubble(ui: &mut egui::Ui, text: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        Frame::new().fill(ACCENT).corner_radius(10).inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_max_width(ui.available_width() * 0.85);
            ui.add(egui::Label::new(RichText::new(text).color(Color32::WHITE)).wrap());
        });
    });
}

fn answer_card(ui: &mut egui::Ui, summary: &str, warnings: &[String], usage: &lvglab_ai::Usage, model: &str) {
    Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(10)
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(summary).wrap());
            if !warnings.is_empty() {
                ui.add_space(4.0);
                egui::CollapsingHeader::new(RichText::new(trf("{} notes", &[&warnings.len()])).color(WARNING))
                    .id_salt(("ai_notes", summary))
                    .show(ui, |ui| {
                        for warning in warnings {
                            ui.add(egui::Label::new(RichText::new(format!("• {warning}")).small()).wrap());
                        }
                    });
            }
            ui.add_space(2.0);
            let tokens = trf("{} in / {} out tokens", &[&usage.input_tokens, &usage.output_tokens]);
            muted(ui, format!("{model} · {tokens}"));
        });
}

fn composer(ui: &mut egui::Ui, studio: &mut Studio) {
    ui.horizontal(|ui| {
        let assistant = &mut studio.assistant;
        ui.selectable_value(&mut assistant.target, Target::CurrentScreen, tr("Change current screen"));
        ui.selectable_value(&mut assistant.target, Target::NewScreens, tr("New screen"));
    });
    let running = studio.assistant.is_running();
    let edit = egui::TextEdit::multiline(&mut studio.assistant.input)
        .hint_text(tr("Describe the interface… (Ctrl+Enter sends)"))
        .desired_rows(3)
        .desired_width(f32::INFINITY);
    let response = ui.add_enabled(!running, edit);
    let send_shortcut = response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command);
    ui.horizontal(|ui| {
        if running {
            if ui.button(tr("Cancel")).clicked() {
                studio.assistant.cancel();
            }
        } else {
            let ready = !studio.assistant.input.trim().is_empty();
            let send = egui::Button::new(RichText::new(tr("Send")).color(Color32::WHITE)).fill(ACCENT);
            if (ui.add_enabled(ready, send).clicked() || (send_shortcut && ready)) && !running {
                studio.start_assistant(ui.ctx());
            }
        }
        muted(ui, tr("Project data is sent to Anthropic."));
    });
}
