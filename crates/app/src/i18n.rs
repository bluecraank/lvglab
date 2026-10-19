//! User interface translations. Texts are written in English in the code
//! and translated through [`tr`]; missing translations fall back to English.
//!
//! Translations live in `locales/<language>.json` as `{"English": "Translation"}`
//! and are embedded at build time.

use std::collections::HashMap;
use std::sync::OnceLock;
#[cfg(not(test))]
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    German,
    Spanish,
    French,
}

impl Language {
    pub const ALL: [Self; 4] = [Self::English, Self::German, Self::Spanish, Self::French];

    /// Name of the language in itself.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::French => "Français",
        }
    }

    /// ISO 639-1 code, as used for the file name in `locales/`.
    fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::German => "de",
            Self::Spanish => "es",
            Self::French => "fr",
        }
    }

    pub fn from_system() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_default().to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| *l != Self::English && locale.starts_with(l.code())).unwrap_or(Self::English)
    }

    /// The embedded translation table; `None` for English.
    pub(crate) fn table(self) -> Option<&'static HashMap<String, String>> {
        static TABLES: OnceLock<[HashMap<String, String>; 3]> = OnceLock::new();
        let tables = TABLES.get_or_init(|| {
            let parse = |code: &str, json: &str| -> HashMap<String, String> {
                serde_json::from_str(json).unwrap_or_else(|err| panic!("locales/{code}.json is invalid: {err}"))
            };
            [
                parse("de", include_str!("../locales/de.json")),
                parse("es", include_str!("../locales/es.json")),
                parse("fr", include_str!("../locales/fr.json")),
            ]
        });
        match self {
            Self::English => None,
            Self::German => Some(&tables[0]),
            Self::Spanish => Some(&tables[1]),
            Self::French => Some(&tables[2]),
        }
    }
}

#[cfg(not(test))]
static CURRENT: AtomicU8 = AtomicU8::new(0);

#[cfg(test)]
thread_local! {
    /// Every test thread has its own language, so tests that switch it cannot disturb others.
    static CURRENT_IN_TEST: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

pub fn set_language(language: Language) {
    #[cfg(test)]
    CURRENT_IN_TEST.with(|c| c.set(language as u8));
    #[cfg(not(test))]
    CURRENT.store(language as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    #[cfg(test)]
    let index = usize::from(CURRENT_IN_TEST.with(std::cell::Cell::get));
    #[cfg(not(test))]
    let index = usize::from(CURRENT.load(Ordering::Relaxed));
    Language::ALL.get(index).copied().unwrap_or_default()
}

/// Translates an English UI text into the current language.
pub fn tr(text: &'static str) -> &'static str {
    language().table().and_then(|t| t.get(text)).map_or(text, String::as_str)
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
    fn translations_are_complete_and_keep_placeholders() {
        let german = Language::German.table().unwrap();
        for language in [Language::Spanish, Language::French] {
            let table = language.table().unwrap();
            for key in german.keys() {
                assert!(table.contains_key(key), "{language:?} lacks {key:?}");
            }
            assert_eq!(table.len(), german.len(), "{language:?} has extra texts");
        }
        for language in [Language::German, Language::Spanish, Language::French] {
            for (english, translated) in language.table().unwrap() {
                assert_eq!(english.matches("{}").count(), translated.matches("{}").count(), "{language:?}: {english}");
            }
        }
        assert_eq!(Language::French.table().unwrap()["Save"], "Enregistrer");
        assert_eq!(Language::Spanish.table().unwrap()["Save"], "Guardar");
    }
}
