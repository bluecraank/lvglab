//! AI assistant state: runs requests on a background thread and applies
//! the answers to the project.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

use eframe::egui;
use lvglab_ai::{AiError, Answer, Config, Scope, Usage};
use lvglab_model::WidgetId;

use crate::app::Studio;
use crate::i18n::{tr, trf};

/// What the request works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    CurrentScreen,
    NewScreens,
}

/// One request and its outcome, shown in the assistant panel.
pub struct Exchange {
    pub request: String,
    pub outcome: Outcome,
}

pub enum Outcome {
    Running,
    Done { summary: String, warnings: Vec<String>, usage: Usage, model: String },
    Failed(String),
    Cancelled,
}

enum Message {
    Progress(usize),
    Finished(Result<Answer, AiError>),
}

struct Job {
    receiver: Receiver<Message>,
    started: Instant,
    received: usize,
    target: Target,
}

pub struct Assistant {
    pub input: String,
    pub target: Target,
    pub history: Vec<Exchange>,
    job: Option<Job>,
}

impl Default for Assistant {
    fn default() -> Self {
        Self { input: String::new(), target: Target::CurrentScreen, history: Vec::new(), job: None }
    }
}

impl Assistant {
    pub fn is_running(&self) -> bool {
        self.job.is_some()
    }

    /// Characters received and seconds elapsed of the running request.
    pub fn progress(&self) -> Option<(usize, u64)> {
        self.job.as_ref().map(|j| (j.received, j.started.elapsed().as_secs()))
    }

    /// Stops waiting for the running request. The request itself finishes
    /// in the background and its answer is discarded.
    pub fn cancel(&mut self) {
        if self.job.take().is_some() {
            if let Some(last) = self.history.last_mut() {
                last.outcome = Outcome::Cancelled;
            }
        }
    }
}

impl Studio {
    /// Starts a request with the text in the input field.
    pub fn start_assistant(&mut self, ctx: &egui::Context) {
        let request = self.assistant.input.trim().to_owned();
        if request.is_empty() || self.assistant.is_running() || crate::sandbox::active() {
            return;
        }
        let config = Config {
            api_key: self.settings.api_key(),
            model: self.settings.ai_model.clone(),
            effort: self.settings.ai_effort.clone(),
        };
        let target = self.assistant.target;
        let scope = match target {
            Target::CurrentScreen => Scope::EditScreen(self.screen),
            Target::NewScreens => Scope::NewScreens,
        };
        let project = self.doc.project.clone();
        let (sender, receiver) = channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let progress = sender.clone();
            let progress_repaint = repaint.clone();
            let result = lvglab_ai::request(&config, &project, scope, &request, move |received| {
                let _ = progress.send(Message::Progress(received));
                progress_repaint.request_repaint();
            });
            let _ = sender.send(Message::Finished(result));
            repaint.request_repaint();
        });
        self.assistant
            .history
            .push(Exchange { request: self.assistant.input.trim().to_owned(), outcome: Outcome::Running });
        self.assistant.input.clear();
        self.assistant.job = Some(Job { receiver, started: Instant::now(), received: 0, target });
    }

    /// Picks up progress and results; call once per frame.
    pub fn poll_assistant(&mut self) {
        let Some(job) = &mut self.assistant.job else { return };
        let mut finished = None;
        loop {
            match job.receiver.try_recv() {
                Ok(Message::Progress(received)) => job.received = received,
                Ok(Message::Finished(result)) => {
                    finished = Some(result);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = Some(Err(AiError::Network(tr("the request ended unexpectedly").to_owned())));
                    break;
                }
            }
        }
        let Some(result) = finished else { return };
        let target = job.target;
        self.assistant.job = None;
        let outcome = match result {
            Ok(answer) => self.apply_answer(answer, target),
            Err(err) => Outcome::Failed(error_text(&err)),
        };
        if let Some(last) = self.assistant.history.last_mut() {
            last.outcome = outcome;
        }
    }

    fn apply_answer(&mut self, answer: Answer, target: Target) -> Outcome {
        let summary = answer.design.summary.clone();
        if answer.design.screens.is_empty() {
            return Outcome::Done {
                summary,
                warnings: vec![tr("The answer contained no screens.").to_owned()],
                usage: answer.usage,
                model: answer.model,
            };
        }
        let applied = lvglab_ai::apply(&mut self.doc.project, &answer.design);
        let label = format!("{} {}", tr("AI:"), first_words(&summary, 6));
        self.timeline.record(&self.doc.project, label);
        let shown: Option<WidgetId> = match target {
            Target::CurrentScreen if applied.screens.contains(&self.screen) => Some(self.screen),
            _ => applied.screens.first().copied(),
        };
        if let Some(screen) = shown {
            self.select_screen(screen);
        }
        self.notify(trf("AI changed {} screen(s)", &[&applied.screens.len()]));
        Outcome::Done { summary, warnings: applied.warnings, usage: answer.usage, model: answer.model }
    }
}

fn first_words(text: &str, count: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut short = words.iter().take(count).copied().collect::<Vec<_>>().join(" ");
    if words.len() > count {
        short.push('…');
    }
    short
}

/// User-facing name of an effort level (untranslated, pass through `tr`).
pub fn effort_label(effort: &str) -> &'static str {
    match effort {
        "low" => "Fast",
        "high" => "Thorough",
        "xhigh" => "Very thorough",
        "max" => "Maximum",
        _ => "Balanced",
    }
}

/// Localized description of an assistant error.
pub fn error_text(err: &AiError) -> String {
    match err {
        AiError::MissingKey => tr("No API key is configured. Add it in the preferences under AI assistant.").to_owned(),
        AiError::Unauthorized => tr("The API key was rejected. Check it in the preferences.").to_owned(),
        AiError::RateLimited => tr("Rate limit reached. Please try again in a moment.").to_owned(),
        AiError::Overloaded => tr("The service is busy. Please try again in a moment.").to_owned(),
        AiError::Refused => tr("The request was declined. Rephrase it and try again.").to_owned(),
        AiError::Truncated => tr("The answer was too long and got cut off. Ask for fewer screens at once.").to_owned(),
        AiError::Api { status, message } => trf("API error {}: {}", &[status, message]),
        AiError::Network(message) => trf("Connection problem: {}", &[message]),
        AiError::InvalidAnswer(message) => trf("The answer could not be read: {}", &[message]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_labels() {
        assert_eq!(first_words("a b c", 6), "a b c");
        assert_eq!(first_words("one two three four", 2), "one two…");
    }
}
