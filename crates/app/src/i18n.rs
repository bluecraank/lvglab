//! User interface translations. Texts are written in English in the code
//! and translated through [`tr`]; missing translations fall back to English.
//!
//! Translations live in `locales/<language>.json` as `{"English": "Translation"}`
//! and are embedded at build time.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    German,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::German];

    /// Name of the language in itself.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::German => "Deutsch",
        }
    }

    pub fn from_system() -> Self {
        match sys_locale::get_locale() {
            Some(locale) if locale.to_ascii_lowercase().starts_with("de") => Self::German,
            _ => Self::English,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_language(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::German,
        _ => Language::English,
    }
}

/// Translates an English UI text into the current language.
pub fn tr(text: &'static str) -> &'static str {
    match language() {
        Language::English => text,
        Language::German => german().get(text).map_or(text, String::as_str),
    }
}

/// Translates a text with `{}` placeholders and fills them in order.
pub fn trf(text: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut parts = tr(text).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, part) in parts.enumerate() {
        if let Some(arg) = args.get(i) {
            out.push_str(&arg.to_string());
        }
        out.push_str(part);
    }
    out
}

fn german() -> &'static HashMap<String, String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    TABLE.get_or_init(|| serde_json::from_str(GERMAN).expect("locales/de.json is valid JSON"))
}

const GERMAN: &str = include_str!("../locales/de.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_and_formatting() {
        set_language(Language::German);
        assert_eq!(tr("Save"), "Speichern");
        assert_eq!(tr("Not translated text"), "Not translated text");
        assert_eq!(trf("{} warnings", &[&3]), "3 Warnungen");
        set_language(Language::English);
        assert_eq!(tr("Save"), "Save");
    }

    #[test]
    fn german_file_keeps_placeholders() {
        for (english, german) in super::german() {
            assert_eq!(english.matches("{}").count(), german.matches("{}").count(), "{english}");
        }
    }
}
