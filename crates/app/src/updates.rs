//! Looks for a newer release on GitHub in the background. A found update is
//! shown in the status bar with a link to the release page; nothing is
//! downloaded or installed automatically.

use std::sync::mpsc::{Receiver, channel};

const LATEST: &str = "https://api.github.com/repos/bluecraank/lvglab/releases/latest";

/// A newer release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    pub url: String,
}

/// What the last check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Newer(Update),
    UpToDate,
    Failed(String),
}

#[derive(Default)]
pub struct UpdateCheck {
    running: Option<Receiver<Outcome>>,
    pub outcome: Option<Outcome>,
    /// The check was started from the menu, so "up to date" is reported as well.
    pub manual: bool,
}

impl UpdateCheck {
    /// Starts a check in a background thread (not in tests).
    pub fn start(&mut self, manual: bool) {
        if self.running.is_some() || crate::sandbox::active() {
            return;
        }
        self.manual = manual;
        let (send, receive) = channel();
        self.running = Some(receive);
        std::thread::spawn(move || {
            let _ = send.send(check());
        });
    }

    /// The result, once the background thread is done.
    pub fn poll(&mut self) -> Option<Outcome> {
        let outcome = self.running.as_ref()?.try_recv().ok()?;
        self.running = None;
        self.outcome = Some(outcome.clone());
        Some(outcome)
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }
}

fn check() -> Outcome {
    let response = ureq::get(LATEST)
        .set("User-Agent", concat!("LVGLab/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(10))
        .call();
    let text = match response.map(|r| r.into_string()) {
        Ok(Ok(text)) => text,
        Ok(Err(err)) => return Outcome::Failed(err.to_string()),
        Err(err) => return Outcome::Failed(err.to_string()),
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Outcome::Failed("unexpected answer".to_owned());
    };
    let tag = json["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_owned();
    let url = json["html_url"].as_str().unwrap_or("https://github.com/bluecraank/lvglab/releases").to_owned();
    if is_newer(&tag, env!("CARGO_PKG_VERSION")) {
        Outcome::Newer(Update { version: tag, url })
    } else {
        Outcome::UpToDate
    }
}

/// `1.2.10` is newer than `1.2.9`; anything that is not a version is not newer.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse().ok()).collect() };
    match (parse(candidate), parse(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("0.13.0", "0.12.0"));
        assert!(is_newer("0.12.10", "0.12.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.12.0", "0.12.0"));
        assert!(!is_newer("0.11.5", "0.12.0"));
        assert!(!is_newer("nightly", "0.12.0"));
    }

    /// Asks GitHub: `cargo test -p lvglab github_answers -- --ignored`
    #[test]
    #[ignore]
    fn github_answers() {
        let outcome = check();
        assert!(!matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    }
}
