//! Color themes: named color variables with one value per theme. Styles can
//! reference a variable instead of a fixed color; switching the theme at
//! runtime recolors every bound property (SquareLine compatible).

use serde::{Deserialize, Serialize};

use crate::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeColor {
    pub color: Color,
    pub opa: u8,
}

impl Default for ThemeColor {
    fn default() -> Self {
        Self { color: Color::WHITE, opa: 255 }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColorTheme {
    pub name: String,
    /// One value per variable, in the order of [`ColorThemes::variables`].
    pub values: Vec<ThemeColor>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorThemes {
    pub variables: Vec<String>,
    /// The first theme is the default theme.
    pub themes: Vec<ColorTheme>,
    /// Theme shown in the editor preview.
    pub active: usize,
}

impl ColorThemes {
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty() || self.themes.is_empty()
    }

    pub fn index_of(&self, variable: &str) -> Option<usize> {
        self.variables.iter().position(|v| v == variable)
    }

    /// Value of a variable in theme `theme`.
    pub fn value_in(&self, theme: usize, variable: &str) -> Option<ThemeColor> {
        let index = self.index_of(variable)?;
        self.themes.get(theme).and_then(|t| t.values.get(index)).copied()
    }

    /// Value of a variable in the active theme.
    pub fn value(&self, variable: &str) -> Option<ThemeColor> {
        self.value_in(self.active.min(self.themes.len().saturating_sub(1)), variable)
    }

    /// Adds a variable with the same value in every theme.
    pub fn add_variable(&mut self, name: impl Into<String>, value: ThemeColor) {
        let name = name.into();
        if self.index_of(&name).is_some() {
            return;
        }
        if self.themes.is_empty() {
            self.themes.push(ColorTheme { name: "Default".to_owned(), values: Vec::new() });
        }
        self.variables.push(name);
        for theme in &mut self.themes {
            theme.values.push(value);
        }
    }

    pub fn remove_variable(&mut self, index: usize) {
        if index < self.variables.len() {
            self.variables.remove(index);
            for theme in &mut self.themes {
                if index < theme.values.len() {
                    theme.values.remove(index);
                }
            }
        }
    }

    /// Adds a theme as a copy of the active one.
    pub fn add_theme(&mut self, name: impl Into<String>) {
        let values = self
            .themes
            .get(self.active)
            .map(|t| t.values.clone())
            .unwrap_or_else(|| vec![ThemeColor::default(); self.variables.len()]);
        self.themes.push(ColorTheme { name: name.into(), values });
    }

    pub fn remove_theme(&mut self, index: usize) {
        if self.themes.len() > 1 && index < self.themes.len() {
            self.themes.remove(index);
            self.active = self.active.min(self.themes.len() - 1);
        }
    }

    /// Keeps every theme's value list as long as the variable list.
    pub fn normalize(&mut self) {
        let count = self.variables.len();
        for theme in &mut self.themes {
            theme.values.resize(count, ThemeColor::default());
        }
        self.active = self.active.min(self.themes.len().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_and_themes() {
        let mut themes = ColorThemes::default();
        themes.add_variable("Main", ThemeColor { color: Color::WHITE, opa: 255 });
        themes.add_theme("Orange");
        themes.themes[1].values[0].color = Color::from_hex(0xF15C00);
        assert_eq!(themes.value("Main").unwrap().color, Color::WHITE);
        themes.active = 1;
        assert_eq!(themes.value("Main").unwrap().color, Color::from_hex(0xF15C00));
        themes.add_variable("Accent", ThemeColor::default());
        assert_eq!(themes.themes[1].values.len(), 2);
        themes.remove_variable(0);
        assert_eq!(themes.variables, vec!["Accent"]);
        themes.remove_theme(1);
        assert_eq!(themes.active, 0);
        assert!(themes.value("Missing").is_none());
    }
}
