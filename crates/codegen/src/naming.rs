//! C identifiers and file names of the generated code.
//!
//! Everything derives from the project's prefix (`ui` by default) the same
//! way SquareLine Studio names its exports, so a LVGLab export can replace a
//! SquareLine export without changing application code:
//! `ui_Main01`, `ui_Main01_screen_init`, `ui_img_logo_png`, `ui_font_Big40`,
//! `_ui_flag_modify`, `UI_THEME_DEFAULT`, `_ui_theme_color_Main`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

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

pub struct Naming {
    prefix: String,
    vars: HashMap<WidgetId, String>,
    /// Image asset name → C symbol.
    images: HashMap<String, String>,
}

impl Naming {
    pub fn new(project: &Project) -> Self {
        let prefix = project.export.symbol_prefix();
        let mut vars = HashMap::new();
        let mut used = HashSet::new();
        for widget in project.all_widgets() {
            let base = format!("{prefix}_{}", c_ident(&widget.name));
            let mut name = base.clone();
            let mut n = 2;
            while !used.insert(name.clone()) {
                name = format!("{base}_{n}");
                n += 1;
            }
            vars.insert(widget.id, name);
        }
        // Objects of components are locals of their create function (`cui_Icon`).
        for component in &project.components {
            let mut local = HashSet::new();
            for root in &component.children {
                root.walk(&mut |w| {
                    let base = format!("c{prefix}_{}", c_ident(&w.name));
                    let mut name = base.clone();
                    let mut n = 2;
                    while !local.insert(name.clone()) {
                        name = format!("{base}_{n}");
                        n += 1;
                    }
                    vars.insert(w.id, name);
                });
            }
        }
        let images = project
            .assets
            .images
            .iter()
            .map(|asset| {
                // SquareLine names images after their file: `logo.png` → `ui_img_logo_png`.
                let file = Path::new(&asset.path).file_name().and_then(|f| f.to_str()).unwrap_or_default();
                let stem = if file.contains('.') { file.to_owned() } else { format!("{}.png", asset.name) };
                (asset.name.clone(), format!("{prefix}_img_{}", c_ident(&stem)))
            })
            .collect();
        Self { prefix, vars, images }
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Upper case prefix for macros (`UI`).
    pub fn upper(&self) -> String {
        self.prefix.to_ascii_uppercase()
    }

    /// The variable of a widget; unknown ids yield `NULL`.
    pub fn var(&self, id: WidgetId) -> &str {
        self.vars.get(&id).map_or("NULL", String::as_str)
    }

    /// `ui_<name>` for functions and globals (`ui_init`, `ui_theme_idx`).
    pub fn global(&self, name: &str) -> String {
        format!("{}_{name}", self.prefix)
    }

    /// `_ui_<name>` for helper functions (`_ui_flag_modify`).
    pub fn helper(&self, name: &str) -> String {
        format!("_{}_{name}", self.prefix)
    }

    /// `_UI_<NAME>` for helper constants (`_UI_MODIFY_FLAG_ADD`).
    pub fn helper_const(&self, name: &str) -> String {
        format!("_{}_{name}", self.upper())
    }

    /// File name relative to the source folder (`ui_helpers.h`).
    pub fn file(&self, suffix: &str) -> String {
        if suffix.is_empty() { self.prefix.clone() } else { format!("{}_{suffix}", self.prefix) }
    }

    pub fn event_fn(&self, id: WidgetId) -> String {
        let var = self.var(id);
        format!("{}_event_{}", self.prefix, var.strip_prefix(&format!("{}_", self.prefix)).unwrap_or(var))
    }

    pub fn image(&self, name: &str) -> String {
        self.images.get(name).cloned().unwrap_or_else(|| format!("{}_img_{}_png", self.prefix, c_ident(name)))
    }

    /// C expression for an image reference (`NULL` if empty).
    pub fn image_ref(&self, name: &str) -> String {
        if name.trim().is_empty() { "NULL".to_owned() } else { format!("&{}", self.image(name.trim())) }
    }

    pub fn font(&self, name: &str) -> String {
        format!("{}_font_{}", self.prefix, c_ident(name))
    }

    /// C expression for a font reference used in styles.
    pub fn font_ref(&self, name: &str) -> String {
        match builtin_font_size(name) {
            Some(size) => format!("&lv_font_montserrat_{size}"),
            None => format!("&{}", self.font(name)),
        }
    }

    /// `ui_<Name>_create` of a component.
    pub fn component_create(&self, component: &str) -> String {
        format!("{}_{}_create", self.prefix, c_ident(component))
    }

    /// `ui_comp_<name>` file stem of a component.
    pub fn component_file(&self, component: &str) -> String {
        format!("{}_comp_{}", self.prefix, c_ident(component).to_ascii_lowercase())
    }

    /// `UI_COMP_<COMPONENT>_<CHILD>` index of an object in a component.
    pub fn component_child(&self, component: &str, child: &str) -> String {
        format!(
            "{}_COMP_{}_{}",
            self.upper(),
            c_ident(component).to_ascii_uppercase(),
            c_ident(child).to_ascii_uppercase()
        )
    }

    /// `_UI_COMP_<COMPONENT>_NUM`: number of objects in a component.
    pub fn component_count(&self, component: &str) -> String {
        format!("_{}_COMP_{}_NUM", self.upper(), c_ident(component).to_ascii_uppercase())
    }

    /// Event that asks a component for one of its objects. SquareLine's
    /// name is kept for the `ui` prefix.
    pub fn component_event(&self) -> String {
        if self.prefix == "ui" {
            "LV_EVENT_GET_COMP_CHILD".to_owned()
        } else {
            format!("{}_EVENT_GET_COMP_CHILD", self.upper())
        }
    }

    pub fn theme_color(&self, variable: &str) -> String {
        format!("_{}_theme_color_{}", self.prefix, c_ident(variable))
    }

    pub fn theme_alpha(&self, variable: &str) -> String {
        format!("_{}_theme_alpha_{}", self.prefix, c_ident(variable))
    }

    /// `UI_THEME_<NAME>` index constant of a theme.
    pub fn theme_const(&self, theme: &str) -> String {
        format!("{}_THEME_{}", self.upper(), c_ident(theme).to_ascii_uppercase())
    }

    /// `UI_THEME_COLOR_<NAME>` index constant of a theme variable.
    pub fn theme_variable_const(&self, variable: &str) -> String {
        format!("{}_THEME_COLOR_{}", self.upper(), c_ident(variable).to_ascii_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::WidgetKind;
    use lvglab_model::project::ImageAsset;

    #[test]
    fn identifiers_are_valid() {
        assert_eq!(c_ident("Main Screen"), "Main_Screen");
        assert_eq!(c_ident("1st"), "_1st");
        assert_eq!(c_ident("Größe"), "Gr__e");
    }

    #[test]
    fn names_follow_squareline_conventions() {
        let mut project = Project::new("P");
        project.assets.images.push(ImageAsset { name: "background".into(), path: "assets/background.png".into() });
        let naming = Naming::new(&project);
        assert_eq!(naming.image("background"), "ui_img_background_png");
        assert_eq!(naming.image_ref(""), "NULL");
        assert_eq!(naming.font_ref("montserrat_20"), "&lv_font_montserrat_20");
        assert_eq!(naming.font_ref("FontCBI40"), "&ui_font_FontCBI40");
        assert_eq!(naming.helper("flag_modify"), "_ui_flag_modify");
        assert_eq!(naming.theme_const("BmwOrange"), "UI_THEME_BMWORANGE");
        assert_eq!(naming.theme_color("Main"), "_ui_theme_color_Main");
        assert_eq!(naming.var(project.screens[0].id), "ui_Screen1");

        project.export.prefix = "sett".into();
        let naming = Naming::new(&project);
        assert_eq!(naming.var(project.screens[0].id), "sett_Screen1");
        assert_eq!(naming.event_fn(project.screens[0].id), "sett_event_Screen1");
        assert_eq!(naming.file("helpers.h"), "sett_helpers.h");
        assert_eq!(naming.helper_const("MODIFY_FLAG_ADD"), "_SETT_MODIFY_FLAG_ADD");
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
        let naming = Naming::new(&project);
        let mut vars: Vec<_> = project.screens[0].children.iter().map(|c| naming.var(c.id).to_owned()).collect();
        vars.sort();
        assert_eq!(vars, vec!["ui_Close", "ui_Close_2"]);
    }
}
