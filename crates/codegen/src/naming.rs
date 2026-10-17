//! C identifiers for widgets and assets.

use std::collections::{HashMap, HashSet};

use lvglab_model::style::builtin_font_size;
use lvglab_model::{Project, WidgetId};

/// Turns arbitrary text into a valid C identifier fragment.
pub fn c_ident(text: &str) -> String {
    let mut ident: String = text.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    if ident.is_empty() || ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    ident
}

pub fn image_symbol(name: &str) -> String {
    format!("ui_img_{}", c_ident(name))
}

pub fn font_symbol(name: &str) -> String {
    format!("ui_font_{}", c_ident(name))
}

/// C expression for a font reference used in styles.
pub fn font_ref(name: &str) -> String {
    match builtin_font_size(name) {
        Some(size) => format!("&lv_font_montserrat_{size}"),
        None => format!("&{}", font_symbol(name)),
    }
}

/// C expression for an image reference (`NULL` if empty).
pub fn image_ref(name: &str) -> String {
    if name.trim().is_empty() { "NULL".to_owned() } else { format!("&{}", image_symbol(name.trim())) }
}

/// Unique C variable names (`ui_<Name>`) for every widget of a project.
pub struct NameTable {
    names: HashMap<WidgetId, String>,
}

impl NameTable {
    pub fn new(project: &Project) -> Self {
        let mut names = HashMap::new();
        let mut used = HashSet::new();
        for widget in project.all_widgets() {
            let base = format!("ui_{}", c_ident(&widget.name));
            let mut name = base.clone();
            let mut n = 2;
            while !used.insert(name.clone()) {
                name = format!("{base}_{n}");
                n += 1;
            }
            names.insert(widget.id, name);
        }
        Self { names }
    }

    /// The variable of a widget; unknown ids yield `NULL`.
    pub fn var(&self, id: WidgetId) -> &str {
        self.names.get(&id).map_or("NULL", String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::WidgetKind;

    #[test]
    fn identifiers_are_valid() {
        assert_eq!(c_ident("Main Screen"), "Main_Screen");
        assert_eq!(c_ident("1st"), "_1st");
        assert_eq!(c_ident("Größe"), "Gr__e");
        assert_eq!(font_ref("montserrat_20"), "&lv_font_montserrat_20");
        assert_eq!(font_ref("FontCBI40"), "&ui_font_FontCBI40");
        assert_eq!(image_ref(""), "NULL");
    }

    #[test]
    fn duplicate_names_get_suffixes() {
        let mut project = Project::new("P");
        let screen = project.screens[0].id;
        for _ in 0..2 {
            let mut label = project.new_widget(WidgetKind::Label);
            label.name = "Close".into();
            project.insert(screen, 0, label).unwrap();
        }
        let table = NameTable::new(&project);
        let mut vars: Vec<_> = project.screens[0].children.iter().map(|c| table.var(c.id).to_owned()).collect();
        vars.sort();
        assert_eq!(vars, vec!["ui_Close", "ui_Close_2"]);
    }
}
