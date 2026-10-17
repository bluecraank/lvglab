//! Import of SquareLine Studio projects (`.spj`, LVGL 8 projects).
//!
//! A `.spj` file is a JSON tree of objects. Every object carries a list of
//! properties identified by `strtype` (`<WIDGET>/<Property>`); styles are
//! nested property trees (`Style_<part>` → `_style/StyleState` →
//! `_style/<Property>`). The importer maps all of this onto the studio model
//! through the widget catalog and reports everything it could not map.

mod format;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use lvglab_model::catalog::{self, StyleSlot, WidgetDef};
use lvglab_model::color_theme::{ColorTheme, ThemeColor};
use lvglab_model::coord::{Align, Coord, Unit};
use lvglab_model::flags::{Dir, FlexAlign, FlexFlow, ObjFlag, ObjState, ScrollbarMode, SnapMode, StateMask};
use lvglab_model::project::{DisplayShape, FontAsset, ImageAsset, LvglVersion};
use lvglab_model::prop::{PropDef, PropKind, PropValue, find_option};
use lvglab_model::style::{Part, StyleKind, StyleProp, StyleValue, builtin_font_size};
use lvglab_model::widget::FlexLayout;
use lvglab_model::{Color, Palette, Project, Widget, WidgetId, WidgetKind};

use format::{SpjFile, SpjNode, SpjProp, ThemesFile};

/// `InheritedType` of property group headers.
const GROUP_HEADER: i64 = 1;
/// `InheritedType` of style properties.
const STYLE: i64 = 11;
/// Property flag of texts that are translated with lv_i18n.
const TRANSLATABLE: i64 = 4_194_304;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("cannot read `{path}`: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("not a SquareLine project: {0}")]
    Format(#[from] serde_json::Error),
}

/// The imported project and everything that could not be mapped.
#[derive(Debug)]
pub struct Imported {
    pub project: Project,
    pub warnings: Vec<String>,
}

pub fn import_file(path: &Path) -> Result<Imported, ImportError> {
    let text =
        std::fs::read_to_string(path).map_err(|source| ImportError::Io { path: path.display().to_string(), source })?;
    let fallback_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported");
    import_str(&text, path.parent(), fallback_name)
}

/// Imports the JSON text of an `.spj` file. `base_dir` is the directory of
/// the file; asset paths are resolved against it.
pub fn import_str(json: &str, base_dir: Option<&Path>, fallback_name: &str) -> Result<Imported, ImportError> {
    let file: SpjFile = serde_json::from_str(json)?;
    let mut importer = Importer::new(base_dir);
    let project = importer.run(file, fallback_name);
    Ok(Imported { project, warnings: importer.warnings() })
}

struct PendingRef {
    widget: WidgetId,
    key: &'static str,
    guid: String,
}

struct Importer<'a> {
    base_dir: Option<&'a Path>,
    project: Project,
    guids: HashMap<String, WidgetId>,
    pending: Vec<PendingRef>,
    /// Theme variable names by SquareLine node id.
    theme_nids: HashMap<i64, String>,
    warnings: BTreeMap<String, usize>,
}

impl<'a> Importer<'a> {
    fn new(base_dir: Option<&'a Path>) -> Self {
        Self {
            base_dir,
            project: Project::default(),
            guids: HashMap::new(),
            pending: Vec::new(),
            theme_nids: HashMap::new(),
            warnings: BTreeMap::new(),
        }
    }

    fn warn(&mut self, message: impl Into<String>) {
        *self.warnings.entry(message.into()).or_default() += 1;
    }

    fn warnings(&self) -> Vec<String> {
        self.warnings
            .iter()
            .map(|(message, count)| if *count > 1 { format!("{message} (×{count})") } else { message.clone() })
            .collect()
    }

    fn run(&mut self, file: SpjFile, fallback_name: &str) -> Project {
        self.apply_info(&file, fallback_name);
        self.import_themes();
        self.import_assets();
        let screens: Vec<&SpjNode> = if file.root.kind.eq_ignore_ascii_case("SCREEN") {
            vec![&file.root]
        } else {
            file.root.children.iter().collect()
        };
        for node in screens {
            if !node.kind.eq_ignore_ascii_case("SCREEN") {
                self.warn(format!("top level object of type {} is not a screen and was skipped", node.kind));
                continue;
            }
            let screen = self.convert(node, WidgetKind::Screen);
            self.project.screens.push(screen);
        }
        if let Some(selected) = &file.selected_screen {
            if let Some(index) = self.project.screens.iter().position(|s| self.guids.get(selected) == Some(&s.id)) {
                let screen = self.project.screens.remove(index);
                self.project.screens.insert(0, screen);
            }
        }
        if !file.animations.is_empty() {
            self.warn(format!("{} animation(s) are not supported and were skipped", file.animations.len()));
        }
        self.resolve_references();
        if self.project.screens.is_empty() {
            self.project.add_screen();
        }
        self.project.repair_ids();
        std::mem::take(&mut self.project)
    }

    fn apply_info(&mut self, file: &SpjFile, fallback_name: &str) {
        let info = &file.info;
        let project = &mut self.project;
        project.name = info
            .project_name
            .clone()
            .or_else(|| info.name.as_deref().map(|n| n.trim_end_matches(".spj").to_owned()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| fallback_name.to_owned());
        if let (Some(w), Some(h)) = (info.width, info.height) {
            project.display.width = w;
            project.display.height = h;
        }
        project.display.shape = match info.shape.as_deref() {
            Some(shape) if shape.eq_ignore_ascii_case("CIRCLE") => DisplayShape::Round,
            _ => DisplayShape::Rectangle,
        };
        if let Some(dark) = info.theme_dark {
            project.theme.dark = dark;
        }
        if let Some(primary) = info.theme_color1.and_then(Palette::from_index) {
            project.theme.primary = primary;
        }
        if let Some(secondary) = info.theme_color2.and_then(Palette::from_index) {
            project.theme.secondary = secondary;
        }
        if let Some(version) = info.lvgl_version.as_deref() {
            match LvglVersion::from_suffix(version) {
                Some(v) => self.project.lvgl_version = v,
                None if version.starts_with("8.") => {}
                None => self.warn(format!("LVGL {version} is not supported; the project targets LVGL 8.3")),
            }
        }
        let export = &mut self.project.export;
        if let Some(prefix) = info.custom_variable_prefix.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            export.prefix = prefix.to_owned();
        }
        export.multilang = info.multilang.as_deref().is_some_and(|m| m.eq_ignore_ascii_case("ENABLE"));
        // The library is named after the project folder, e.g. `ui_round`.
        if let Some(folder) = self.base_dir.and_then(Path::file_name).and_then(|f| f.to_str()) {
            let name: String =
                folder.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
            if !name.trim_matches('_').is_empty() {
                export.library_name = name;
            }
        }
        // SquareLine's TFT_eSPI board exports for LV_COLOR_16_SWAP 1.
        export.swap_bytes = true;
        if let Some(board) = &info.board {
            if !board.contains("TFT_eSPI") {
                self.warn(format!("board `{board}` was replaced by ESP32 + TFT_eSPI"));
            }
        }
    }

    /// Reads the color themes from `Themes.slt` next to the project file.
    fn import_themes(&mut self) {
        let Some(path) = self.base_dir.map(|d| d.join("Themes.slt")) else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let file: ThemesFile = match serde_json::from_str(&text) {
            Ok(file) => file,
            Err(err) => {
                self.warn(format!("Themes.slt could not be read: {err}"));
                return;
            }
        };
        let themes = &mut self.project.color_themes;
        let color = |values: &[i64]| {
            let c = |i: usize, d: i64| values.get(i).copied().unwrap_or(d).clamp(0, 255) as u8;
            ThemeColor { color: Color::rgb(c(0, 255), c(1, 255), c(2, 255)), opa: c(3, 255) }
        };
        for variable in &file.deftheme.properties {
            themes.variables.push(variable.strval.clone());
            self.theme_nids.insert(variable.nid, variable.strval.clone());
        }
        for theme in std::iter::once(&file.deftheme).chain(&file.themes) {
            let values = file
                .deftheme
                .properties
                .iter()
                .map(|variable| {
                    let own = theme.properties.iter().find(|p| p.nid == variable.nid || p.strval == variable.strval);
                    color(&own.unwrap_or(variable).intarray)
                })
                .collect();
            themes.themes.push(ColorTheme { name: theme.name.clone(), values });
        }
        if let Some(selected) = &file.selected_theme {
            themes.active = themes.themes.iter().position(|t| &t.name == selected).unwrap_or(0);
        }
        themes.normalize();
    }

    /// The theme variable a color style refers to, if any.
    fn theme_reference(&self, entry: &SpjProp) -> Option<String> {
        let nid = entry.integer.filter(|n| *n != 0)?;
        self.theme_nids.get(&nid).cloned()
    }

    fn convert(&mut self, node: &SpjNode, kind: WidgetKind) -> Widget {
        let id = self.project.alloc_id();
        self.guids.insert(node.guid.clone(), id);
        let mut widget = Widget::new(kind, id, String::new());
        // Clear editor defaults: imported files state their values explicitly.
        widget.flags.clear();
        widget.props.clear();
        let def = kind.def();
        for prop in &node.properties {
            self.apply_property(&mut widget, def, prop);
        }
        if widget.name.is_empty() {
            widget.name = self.project.unique_name(&def.name.replace(' ', ""));
        }
        for child in &node.children {
            let child_kind = catalog::by_squareline_name(&child.kind).unwrap_or_else(|| {
                self.warn(format!("object type {} is not supported and was imported as Panel", child.kind));
                WidgetKind::Panel
            });
            if !def.accepts_child(child_kind) {
                self.warn(format!("{} cannot contain {}; it was skipped", def.name, child_kind.name()));
                continue;
            }
            let child = self.convert(child, child_kind);
            widget.children.push(child);
        }
        widget
    }

    fn apply_property(&mut self, widget: &mut Widget, def: &'static WidgetDef, prop: &SpjProp) {
        let Some((group, name)) = prop.strtype.split_once('/') else {
            self.warn(format!("property `{}` was ignored", prop.strtype));
            return;
        };
        if prop.inherited_type == GROUP_HEADER {
            return;
        }
        if prop.inherited_type == STYLE || name.starts_with("Style_") {
            self.apply_style(widget, def, name, prop);
        } else if group.eq_ignore_ascii_case("OBJECT") {
            self.apply_object_property(widget, name, prop);
        } else if group.eq_ignore_ascii_case("STARTEVENTS") || name.eq_ignore_ascii_case("Name") {
            if name.eq_ignore_ascii_case("Name") && !group.eq_ignore_ascii_case("STARTEVENTS") {
                widget.name = prop.text();
            }
        } else {
            self.apply_widget_property(widget, def, name, prop);
        }
    }

    fn apply_object_property(&mut self, widget: &mut Widget, name: &str, prop: &SpjProp) {
        match name.to_ascii_lowercase().as_str() {
            "name" => widget.name = prop.text(),
            "position" => {
                let (x, y) = coords(prop);
                widget.geometry.x = x;
                widget.geometry.y = y;
            }
            "size" => {
                let (w, h) = coords(prop);
                widget.geometry.width = w;
                widget.geometry.height = h;
            }
            "align" => match Align::from_suffix(&prop.text()) {
                Some(align) => widget.geometry.align = align,
                None => self.warn(format!("unknown alignment `{}`", prop.text())),
            },
            "layout_type" => widget.layout = flex_layout(prop),
            "extend_click_area" => widget.ext_click_area = prop.int() as i32,
            "scrollbar_mode" => widget.scroll.scrollbar_mode = self.parse_enum(ScrollbarMode::from_suffix, prop, name),
            "scroll_direction" => widget.scroll.dir = self.parse_enum(Dir::from_suffix, prop, name),
            "scroll_snap_x" => widget.scroll.snap_x = self.parse_enum(SnapMode::from_suffix, prop, name),
            "scroll_snap_y" => widget.scroll.snap_y = self.parse_enum(SnapMode::from_suffix, prop, name),
            "scroll_chain" => {
                widget.set_flag(ObjFlag::ScrollChainHor, prop.bool());
                widget.set_flag(ObjFlag::ScrollChainVer, prop.bool());
            }
            other => {
                let upper = other.to_ascii_uppercase();
                if let Some(flag) = ObjFlag::from_suffix(&upper) {
                    widget.set_flag(flag, prop.bool());
                } else if let Some(state) = ObjState::from_suffix(&upper) {
                    if prop.bool() {
                        widget.states.insert(state);
                    }
                } else {
                    self.warn(format!("object property `{name}` is not supported"));
                }
            }
        }
    }

    fn parse_enum<T>(&mut self, parse: fn(&str) -> Option<T>, prop: &SpjProp, name: &str) -> Option<T> {
        let text = prop.text();
        let value = parse(&text);
        if value.is_none() {
            self.warn(format!("unknown value `{text}` for {name}"));
        }
        value
    }

    fn apply_widget_property(&mut self, widget: &mut Widget, def: &'static WidgetDef, name: &str, prop: &SpjProp) {
        let Some((target, component)) = find_prop(def, name) else {
            self.warn(format!("{} property `{name}` is not supported", def.name));
            return;
        };
        let value = match target.kind {
            PropKind::Bool => PropValue::Bool(prop.bool()),
            PropKind::Int { .. } => PropValue::Int(prop.int() as i32),
            PropKind::ChartSeries => match chart_series(&prop.text()) {
                Some(series) => PropValue::Text(series),
                None => {
                    self.warn(format!("{}: chart data `{}` was not recognized", widget.name, prop.text()));
                    return;
                }
            },
            PropKind::Text | PropKind::Lines | PropKind::Points | PropKind::StrArray { .. } => {
                PropValue::Text(unescape(&prop.text()))
            }
            PropKind::ImageArray => {
                let names: Vec<String> = prop.text().split(['\n', ';']).filter_map(|p| self.image_asset(p)).collect();
                PropValue::Text(names.join("\n"))
            }
            PropKind::Enum(options) => match find_option(options, &prop.text()) {
                Some(option) => PropValue::Enum(option.c.to_owned()),
                None => {
                    self.warn(format!("unknown value `{}` for {}.{name}", prop.text(), def.name));
                    return;
                }
            },
            PropKind::Color => PropValue::Color(prop.color()),
            PropKind::Pair { .. } => {
                let (mut a, mut b) = widget.prop_pair(target.key);
                match component {
                    Some(0) => a = prop.int() as i32,
                    Some(_) => b = prop.int() as i32,
                    None => {
                        let values = prop.ints();
                        a = values.first().copied().unwrap_or(a as i64) as i32;
                        b = values.get(1).copied().unwrap_or(b as i64) as i32;
                    }
                }
                PropValue::Pair(a, b)
            }
            PropKind::Image => match self.image_asset(&prop.text()) {
                Some(name) => PropValue::Image(name),
                None => return,
            },
            PropKind::WidgetRef => {
                let guid = prop.text();
                if !guid.is_empty() {
                    self.pending.push(PendingRef { widget: widget.id, key: target.key, guid });
                }
                return;
            }
            PropKind::Date => {
                let v = prop.ints();
                let get = |i: usize, d: i64| v.get(i).copied().unwrap_or(d);
                PropValue::Date { year: get(0, 2024) as i32, month: get(1, 1) as u8, day: get(2, 1) as u8 }
            }
        };
        widget.set_prop(target.key, value);
        if prop.flags.unwrap_or(0) & TRANSLATABLE != 0 && def.prop("translate").is_some() {
            widget.set_prop("translate", PropValue::Bool(true));
        }
    }

    fn apply_style(&mut self, widget: &mut Widget, def: &'static WidgetDef, name: &str, prop: &SpjProp) {
        let Some(slot) = find_slot(def, name, prop.part.as_deref()) else {
            self.warn(format!("{} style part `{name}` is not supported", def.name));
            return;
        };
        for state_node in &prop.childs {
            let Some(state) = StateMask::parse(&state_node.text()) else {
                self.warn(format!("unknown style state `{}`", state_node.text()));
                continue;
            };
            for entry in &state_node.childs {
                let style_name = entry.strtype.rsplit('/').next().unwrap_or_default();
                if let Some(parts) = StyleProp::squareline_composite(style_name) {
                    for (prop, value) in parts.iter().zip(entry.ints()) {
                        let value = match prop.kind() {
                            StyleKind::Coord => StyleValue::Coord(Coord::px(*value as i32)),
                            _ => StyleValue::Int(*value as i32),
                        };
                        widget.styles.set(slot.key, state, *prop, value);
                    }
                    continue;
                }
                let Some(style) = StyleProp::from_squareline(style_name) else {
                    self.warn(format!("style property `{style_name}` is not supported"));
                    continue;
                };
                if matches!(style.kind(), StyleKind::Color) {
                    if let Some(variable) = self.theme_reference(entry) {
                        widget.styles.set(slot.key, state, style, StyleValue::Theme(variable.clone()));
                        if let Some(opa) = style.opa_companion() {
                            widget.styles.set(slot.key, state, opa, StyleValue::Theme(variable));
                        }
                        continue;
                    }
                }
                if let Some(value) = self.style_value(style, entry) {
                    widget.styles.set(slot.key, state, style, value);
                }
                if let (Some(opa), Some(alpha)) = (style.opa_companion(), entry.ints().get(3)) {
                    widget.styles.set(slot.key, state, opa, StyleValue::Int(*alpha as i32));
                }
            }
        }
    }

    fn style_value(&mut self, style: StyleProp, entry: &SpjProp) -> Option<StyleValue> {
        Some(match style.kind() {
            StyleKind::Color => StyleValue::Color(entry.color()),
            StyleKind::Coord => StyleValue::Coord(Coord::px(entry.int() as i32)),
            StyleKind::Int { .. } | StyleKind::Opa => StyleValue::Int(entry.int() as i32),
            StyleKind::Bool => StyleValue::Bool(entry.bool()),
            StyleKind::Enum(options) => match find_option(options, &entry.text()) {
                Some(option) => StyleValue::Enum(option.c.to_owned()),
                None => {
                    self.warn(format!("unknown value `{}` for style {}", entry.text(), style.name()));
                    return None;
                }
            },
            StyleKind::Font => StyleValue::Font(self.font_asset(&entry.text())),
            StyleKind::Image => StyleValue::Image(self.image_asset(&entry.text())?),
        })
    }

    /// Imports everything in the project's `assets` folder: images and the
    /// fonts defined by SquareLine's `.fcfg` files.
    fn import_assets(&mut self) {
        let Some(dir) = self.base_dir.map(|d| d.join("assets")) else { return };
        let mut files = Vec::new();
        collect_files(&dir, &mut files);
        files.sort();
        for path in &files {
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
            match extension.as_str() {
                "png" | "jpg" | "jpeg" | "bmp" => {
                    let relative = path.strip_prefix(self.base_dir.unwrap_or(Path::new(""))).unwrap_or(path);
                    self.image_asset(&relative.display().to_string());
                }
                "fcfg" => self.font_config(path),
                _ => {}
            }
        }
    }

    fn font_config(&mut self, path: &Path) {
        let config =
            std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<format::FontConfig>(&text).ok());
        let Some(config) = config else {
            self.warn(format!("font configuration `{}` could not be read", path.display()));
            return;
        };
        if self.project.assets.font(&config.codename).is_some() {
            return;
        }
        let ttf = config.ttf_path.replace('\\', "/");
        let ttf = Project::resolve_path(self.base_dir, ttf.trim_start_matches('/'));
        if !ttf.exists() {
            self.warn(format!("font file `{}` of {} was not found", ttf.display(), config.codename));
        }
        let ranges: Vec<&str> = config.ranges.iter().map(|r| r.trim()).filter(|r| !r.is_empty()).collect();
        self.project.assets.fonts.push(FontAsset {
            name: config.codename,
            path: ttf.display().to_string(),
            size: config.size,
            bpp: config.bpp,
            ranges: ranges.join(", "),
            symbols: config.symbols,
        });
    }

    /// Registers a font referenced by a style and returns its name.
    fn font_asset(&mut self, name: &str) -> String {
        let name = name.trim().to_owned();
        if builtin_font_size(&name).is_some() || self.project.assets.font(&name).is_some() {
            return name;
        }
        let digits: String =
            name.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
        let size = digits.parse().unwrap_or(16);
        let path = self.find_font_file(&name).unwrap_or_default();
        if path.is_empty() {
            self.warn(format!("font `{name}`: assign a TTF file in the assets panel before exporting"));
        }
        self.project.assets.fonts.push(FontAsset { name: name.clone(), path, size, ..FontAsset::default() });
        name
    }

    fn find_font_file(&self, name: &str) -> Option<String> {
        let dir = self.base_dir?.join("assets");
        let entries = std::fs::read_dir(dir).ok()?;
        entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| {
                let is_font =
                    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"));
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_ascii_lowercase();
                is_font && name.to_ascii_lowercase().starts_with(stem.trim_end_matches(|c: char| c.is_ascii_digit()))
            })
            .map(|p| p.display().to_string())
    }

    /// Registers an image referenced by path and returns its asset name.
    fn image_asset(&mut self, path: &str) -> Option<String> {
        let path = path.trim();
        if path.is_empty() {
            return None;
        }
        let stem = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or(path);
        let name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        if self.project.assets.image(&name).is_none() {
            let resolved = Project::resolve_path(self.base_dir, path);
            if !resolved.exists() {
                self.warn(format!("image `{}` was not found", resolved.display()));
            }
            self.project.assets.images.push(ImageAsset { name: name.clone(), path: resolved.display().to_string() });
        }
        Some(name)
    }

    fn resolve_references(&mut self) {
        for pending in std::mem::take(&mut self.pending) {
            let target = self.guids.get(&pending.guid).copied();
            if target.is_none() {
                self.warn(format!("reference to unknown object {}", pending.guid));
            }
            if let Some(widget) = self.project.widget_mut(pending.widget) {
                widget.set_prop(pending.key, PropValue::WidgetRef(target));
            }
        }
    }
}

/// Finds the property a SquareLine name maps to, plus the pair component
/// addressed by a `#0`/`#1` alias.
fn find_prop(def: &'static WidgetDef, name: &str) -> Option<(&'static PropDef, Option<usize>)> {
    for prop in def.props {
        for alias in prop.squareline {
            let (alias, component) = match alias.split_once('#') {
                Some((alias, c)) => (alias, c.parse().ok()),
                None => (*alias, None),
            };
            if alias.eq_ignore_ascii_case(name) {
                return Some((prop, component));
            }
        }
    }
    def.props.iter().find(|p| p.key.eq_ignore_ascii_case(name)).map(|p| (p, None))
}

fn find_slot(def: &'static WidgetDef, name: &str, part: Option<&str>) -> Option<&'static StyleSlot> {
    let key = name.strip_prefix("Style_").unwrap_or(name).to_ascii_lowercase();
    def.slots.iter().find(|s| s.key == key || s.squareline.contains(&key.as_str())).or_else(|| {
        let part = Part::from_squareline(part?)?;
        def.slots.iter().find(|s| s.part == part && s.getter.is_none())
    })
}

/// Decodes a SquareLine position/size: the low nibble of `flags` is the
/// unit of the first value, the next nibble the unit of the second
/// (1 = px, 2 = %, 3 = content).
fn coords(prop: &SpjProp) -> (Coord, Coord) {
    let values = prop.ints();
    let flags = prop.flags.unwrap_or(0x11);
    let make = |value: i64, unit: i64| {
        let unit = match unit & 0xF {
            2 => Unit::Percent,
            3 => Unit::Content,
            _ => Unit::Px,
        };
        Coord { value: if unit == Unit::Content { 0 } else { value as i32 }, unit }
    };
    (make(values.first().copied().unwrap_or(0), flags), make(values.get(1).copied().unwrap_or(0), flags >> 4))
}

fn flex_layout(prop: &SpjProp) -> Option<FlexLayout> {
    let is_flex = prop.layout.layout_type.unwrap_or(0) != 0 || prop.text().to_ascii_lowercase().contains("flex");
    if !is_flex {
        return None;
    }
    let l = &prop.layout;
    Some(FlexLayout {
        flow: FlexFlow::compose(l.flow.unwrap_or(0) == 0, l.wrap.unwrap_or(false), l.reversed.unwrap_or(false)),
        main_place: FlexAlign::from_index(l.main_alignment.unwrap_or(0)),
        cross_place: FlexAlign::from_index(l.cross_alignment.unwrap_or(0)),
        track_place: FlexAlign::from_index(l.track_alignment.unwrap_or(0)),
    })
}

fn collect_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Converts SquareLine's `ChartData` (a JSON array of series) to the
/// studio's series text. Returns `None` for unrecognized content.
fn chart_series(json: &str) -> Option<String> {
    use lvglab_model::series::{self, Series};
    use serde_json::Value;

    let value: Value = serde_json::from_str(json.trim()).ok()?;
    let numbers = |v: &Value| -> Option<Vec<i32>> {
        v.as_array()?.iter().map(|n| n.as_f64().map(|f| f.round() as i32)).collect()
    };
    let color_of = |v: &Value| -> Option<Color> {
        match v {
            Value::String(s) => Color::parse(s),
            Value::Array(c) if c.len() >= 3 => {
                let channel = |i: usize| c[i].as_u64().map(|x| x.min(255) as u8);
                Some(Color::rgb(channel(0)?, channel(1)?, channel(2)?))
            }
            Value::Number(n) => n.as_u64().map(|x| Color::from_hex(x as u32)),
            _ => None,
        }
    };
    let mut result = Vec::new();
    for entry in value.as_array()? {
        let parsed = match entry {
            Value::Array(_) => {
                numbers(entry).map(|values| Series { color: Palette::Blue.main(), secondary: false, values })
            }
            Value::Object(map) => {
                let is_data = |k: &str| ["data", "values", "points", "array"].iter().any(|d| k.contains(d));
                let is_meta = |k: &str| k.contains("color") || k.contains("axis");
                let values = map
                    .iter()
                    .filter(|(k, _)| is_data(&k.to_ascii_lowercase()))
                    .chain(map.iter().filter(|(k, _)| !is_meta(&k.to_ascii_lowercase())))
                    .find_map(|(_, v)| numbers(v))
                    .unwrap_or_default();
                let color = map
                    .iter()
                    .find(|(k, _)| k.to_ascii_lowercase().contains("color"))
                    .and_then(|(_, v)| color_of(v))
                    .unwrap_or(Palette::Blue.main());
                let secondary = map.iter().any(|(k, v)| {
                    k.to_ascii_lowercase().contains("axis") && v.to_string().to_ascii_uppercase().contains("SECONDARY")
                });
                Some(Series { color, secondary, values })
            }
            _ => None,
        };
        result.push(parsed?);
    }
    Some(series::format(&result))
}

/// SquareLine stores line breaks as the two characters `\n`.
fn unescape(text: &str) -> String {
    text.replace("\\n", "\n").replace("\\t", "\t")
}

impl SpjProp {
    fn text(&self) -> String {
        self.strval.clone().unwrap_or_default()
    }

    fn int(&self) -> i64 {
        self.integer.or_else(|| self.strval.as_deref().and_then(|s| s.trim().parse().ok())).unwrap_or(0)
    }

    fn bool(&self) -> bool {
        self.strval.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("true"))
    }

    fn ints(&self) -> &[i64] {
        self.intarray.as_deref().unwrap_or_default()
    }

    fn color(&self) -> Color {
        let c = self.ints();
        let channel = |i: usize| c.get(i).copied().unwrap_or(0).clamp(0, 255) as u8;
        Color::rgb(channel(0), channel(1), channel(2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "root": { "guid": "ROOT", "saved_objtypeKey": "STARTEVENTS", "properties": [], "children": [
        { "guid": "S1", "saved_objtypeKey": "SCREEN", "isPage": true, "properties": [
            { "nid": 10, "strtype": "OBJECT/Name", "strval": "Main", "InheritedType": 10 },
            { "nid": 1040, "strtype": "SCREEN/Style_main", "part": "lv.PART.MAIN", "InheritedType": 11, "childs": [
              { "strtype": "_style/StyleState", "strval": "DEFAULT", "InheritedType": 1, "childs": [
                { "strtype": "_style/Bg_Color", "intarray": [16, 32, 48, 255], "InheritedType": 7 }
              ]}
            ]}
          ], "children": [
          { "guid": "B1", "saved_objtypeKey": "BUTTON", "properties": [
              { "strtype": "OBJECT/Name", "strval": "Ok", "InheritedType": 10 },
              { "strtype": "OBJECT/Position", "flags": 33, "intarray": [5, 10], "InheritedType": 7 },
              { "strtype": "OBJECT/Size", "flags": 49, "intarray": [120, 1], "InheritedType": 7 },
              { "strtype": "OBJECT/Align", "strval": "BOTTOM_MID", "InheritedType": 3 },
              { "strtype": "OBJECT/Clickable", "strval": "False", "InheritedType": 2 },
              { "strtype": "OBJECT/Checked", "strval": "True", "InheritedType": 2 },
              { "strtype": "OBJECT/Layout_type", "strval": "Flex", "Flow": 1, "Wrap": true, "Reversed": false,
                "MainAlignment": 2, "CrossAlignment": 0, "TrackAlignment": 0, "LayoutType": 1, "InheritedType": 13 },
              { "strtype": "BUTTON/Style_main", "part": "lv.PART.MAIN", "InheritedType": 11, "childs": [
                { "strtype": "_style/StyleState", "strval": "CHECKED", "childs": [
                  { "strtype": "_style/Bg_Color", "intarray": [241, 92, 0, 0] },
                  { "strtype": "_style/Bg_Radius", "integer": 250 },
                  { "strtype": "_style/Text_Font", "strval": "FontCBI40" }
                ]}
              ]}
            ], "children": [
              { "guid": "L1", "saved_objtypeKey": "LABEL", "properties": [
                { "strtype": "OBJECT/Name", "strval": "OkLabel" },
                { "strtype": "LABEL/Text", "strval": "Line1\\nLine2" },
                { "strtype": "LABEL/Long_mode", "strval": "DOT" }
              ]}
          ]},
          { "guid": "T1", "saved_objtypeKey": "TEXTAREA", "properties": [
              { "strtype": "OBJECT/Name", "strval": "Pin" },
              { "strtype": "TEXTAREA/Max_text_length", "integer": 4 },
              { "strtype": "TEXTAREA/One_line_mode", "strval": "True" }
          ]},
          { "guid": "K1", "saved_objtypeKey": "KEYBOARD", "properties": [
              { "strtype": "OBJECT/Name", "strval": "Keys" },
              { "strtype": "KEYBOARD/Target_textarea", "strval": "T1" },
              { "strtype": "KEYBOARD/Style_items", "part": "lv.PART.ITEMS", "InheritedType": 11, "childs": [] }
          ]},
          { "guid": "D1", "saved_objtypeKey": "DROPDOWN", "properties": [
              { "strtype": "DROPDOWN/Options", "strval": "g\\nm/s²" },
              { "strtype": "DROPDOWN/List_align", "strval": "TOP" },
              { "strtype": "DROPDOWN/Style_list_main", "part": " lv.PART.MAIN", "InheritedType": 11, "childs": [
                { "strtype": "_style/StyleState", "strval": "DEFAULT", "childs": [
                  { "strtype": "_style/Text_Align", "strval": "CENTER" }
                ]}
              ]}
          ]},
          { "guid": "X1", "saved_objtypeKey": "WEIRD", "properties": [] }
        ]}
      ]},
      "animations": [],
      "selected_theme": "Default",
      "selected_screen": "S1",
      "info": { "name": "Demo.spj", "width": 466, "height": 466, "shape": "CIRCLE", "theme_dark": false,
                "theme_color1": 14, "theme_color2": 0, "lvgl_version": "8.3.11", "board": "Arduino with TFT_eSPI" }
    }"#;

    fn import() -> Imported {
        import_str(SAMPLE, None, "Fallback").unwrap()
    }

    #[test]
    fn imports_project_settings() {
        let p = import().project;
        assert_eq!(p.name, "Demo");
        assert_eq!((p.display.width, p.display.height), (466, 466));
        assert_eq!(p.display.shape, DisplayShape::Round);
        assert!(!p.theme.dark);
        assert_eq!(p.theme.primary, Palette::Orange);
        assert_eq!(p.lvgl_version, LvglVersion::V8_3_11);
    }

    #[test]
    fn imports_geometry_flags_and_layout() {
        let p = import().project;
        let screen = &p.screens[0];
        assert_eq!(screen.name, "Main");
        let button = &screen.children[0];
        assert_eq!(button.kind, WidgetKind::Button);
        assert_eq!(button.geometry.x, Coord::px(5));
        assert_eq!(button.geometry.y, Coord::pct(10));
        assert_eq!(button.geometry.width, Coord::px(120));
        assert_eq!(button.geometry.height, Coord::content());
        assert_eq!(button.geometry.align, Align::BottomMid);
        assert!(!button.flag(ObjFlag::Clickable));
        assert!(button.states.contains(&ObjState::Checked));
        let flex = button.layout.unwrap();
        assert_eq!(flex.flow, FlexFlow::ColumnWrap);
        assert_eq!(flex.main_place, FlexAlign::Center);
    }

    #[test]
    fn imports_styles_with_alpha_and_fonts() {
        let p = import().project;
        let button = &p.screens[0].children[0];
        let checked = StateMask::DEFAULT.with(ObjState::Checked);
        assert_eq!(
            button.styles.get("main", checked, StyleProp::BgColor),
            Some(&StyleValue::Color(Color::rgb(241, 92, 0)))
        );
        assert_eq!(button.styles.get("main", checked, StyleProp::BgOpa), Some(&StyleValue::Int(0)));
        assert_eq!(button.styles.get("main", checked, StyleProp::Radius), Some(&StyleValue::Int(250)));
        assert_eq!(p.assets.font("FontCBI40").unwrap().size, 40);
        let dropdown = &p.screens[0].children[3];
        assert_eq!(
            dropdown.styles.get("list_main", StateMask::DEFAULT, StyleProp::TextAlign),
            Some(&StyleValue::Enum("LV_TEXT_ALIGN_CENTER".into()))
        );
    }

    #[test]
    fn imports_widget_properties_and_references() {
        let p = import().project;
        let screen = &p.screens[0];
        let label = &screen.children[0].children[0];
        assert_eq!(label.prop_text("text"), "Line1\nLine2");
        assert_eq!(label.prop_text("long_mode"), "LV_LABEL_LONG_DOT");
        let textarea = &screen.children[1];
        assert_eq!(textarea.prop_int("max_length"), 4);
        assert!(textarea.prop_bool("one_line"));
        let keyboard = &screen.children[2];
        assert_eq!(keyboard.prop("textarea"), Some(PropValue::WidgetRef(Some(textarea.id))));
        let dropdown = &screen.children[3];
        assert_eq!(dropdown.prop_text("options"), "g\nm/s²");
        assert_eq!(dropdown.prop_text("dir"), "LV_DIR_TOP");
    }

    #[test]
    fn reports_unsupported_content() {
        let imported = import();
        assert!(imported.warnings.iter().any(|w| w.contains("WEIRD")), "{:?}", imported.warnings);
        assert!(imported.warnings.iter().any(|w| w.contains("FontCBI40")));
        assert_eq!(imported.project.screens[0].children[4].kind, WidgetKind::Panel);
    }

    #[test]
    fn imports_assets_folder_with_font_configs() {
        let dir = std::env::temp_dir().join(format!("lvglab-spj-assets-{}", std::process::id()));
        let assets = dir.join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("logo.png"), b"not decoded by the importer").unwrap();
        std::fs::write(assets.join("calibri.ttf"), b"ttf").unwrap();
        std::fs::write(
            assets.join("ui_font_Big40.fcfg"),
            r#"{"codename": "Big40", "ttf_path": "/assets/calibri.ttf", "size": 40, "bpp": 2,
                "ranges": ["0x2D", "   0x30-0x39"], "symbols": "°%"}"#,
        )
        .unwrap();
        let imported = import_str(SAMPLE, Some(&dir), "x").unwrap();
        let font = imported.project.assets.font("Big40").expect("font from fcfg");
        assert_eq!((font.size, font.bpp), (40, 2));
        assert_eq!(font.ranges, "0x2D, 0x30-0x39");
        assert_eq!(font.symbols, "°%");
        assert!(Path::new(&font.path).ends_with("assets/calibri.ttf"), "{}", font.path);
        assert!(imported.project.assets.image("logo").is_some());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn imports_color_themes_translations_and_export_settings() {
        let dir = std::env::temp_dir().join(format!("lvglab-spj-themes-{}", std::process::id())).join("ui_round");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Themes.slt"),
            r#"{"deftheme": {"name": "Default", "properties": [
                  {"nid": 77, "strtype": "THEME_COLOR", "strval": "Main", "intarray": [255, 255, 255, 255]}]},
                "themes": [{"name": "BmwOrange", "properties": [
                  {"nid": 77, "strtype": "THEME_COLOR", "strval": "Main", "intarray": [241, 92, 0, 200]}]}],
                "selected_theme": "BmwOrange"}"#,
        )
        .unwrap();
        let json = r#"{ "root": { "guid": "s", "saved_objtypeKey": "SCREEN", "properties": [], "children": [
            { "guid": "l", "saved_objtypeKey": "LABEL", "properties": [
              { "strtype": "OBJECT/Name", "strval": "Title" },
              { "strtype": "LABEL/Text", "strval": "Overview", "flags": 4194304 },
              { "strtype": "LABEL/Style_main", "part": "lv.PART.MAIN", "InheritedType": 11, "childs": [
                { "strtype": "_style/StyleState", "strval": "DEFAULT", "childs": [
                  { "strtype": "_style/Text_Color", "strval": "Main", "integer": 77, "intarray": [255, 255, 255, 255] }
                ]}
              ]}
            ]}
          ]},
          "info": { "custom_variable_prefix": "sett", "multilang": "ENABLE", "board": "Arduino with TFT_eSPI" } }"#;
        let project = import_str(json, Some(&dir), "x").unwrap().project;
        let themes = &project.color_themes;
        assert_eq!(themes.variables, vec!["Main"]);
        assert_eq!(themes.themes.len(), 2);
        assert_eq!(themes.active, 1);
        assert_eq!(themes.value("Main").unwrap().opa, 200);
        let label = &project.screens[0].children[0];
        assert!(label.prop_bool("translate"));
        assert_eq!(
            label.styles.get("main", StateMask::DEFAULT, StyleProp::TextColor),
            Some(&StyleValue::Theme("Main".into()))
        );
        assert_eq!(
            label.styles.get("main", StateMask::DEFAULT, StyleProp::TextOpa),
            Some(&StyleValue::Theme("Main".into()))
        );
        let export = &project.export;
        assert_eq!((export.prefix.as_str(), export.library_name.as_str()), ("sett", "ui_round"));
        assert!(export.multilang && export.swap_bytes);
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[test]
    fn imports_composite_styles_and_chart_settings() {
        let json = r#"{
          "root": { "guid": "R", "saved_objtypeKey": "STARTEVENTS", "children": [
            { "guid": "S", "saved_objtypeKey": "SCREEN", "properties": [], "children": [
              { "guid": "C", "saved_objtypeKey": "CHART", "properties": [
                  { "strtype": "CHART/Number_of_points", "integer": 50 },
                  { "strtype": "CHART/Secondary_Y_range", "intarray": [0, 8000] },
                  { "strtype": "CHART/XMajor", "intarray": [10, 1] },
                  { "strtype": "CHART/Labels_on_X_axis", "strval": "True" },
                  { "strtype": "CHART/ChartData", "strval": "[]" },
                  { "strtype": "CHART/Style_main", "part": "lv.PART.MAIN", "InheritedType": 11, "childs": [
                    { "strtype": "_style/StyleState", "strval": "DEFAULT", "childs": [
                      { "strtype": "_style/Padding", "intarray": [2, 7, 0, 1] },
                      { "strtype": "_style/Shadow_params", "intarray": [30, 4] },
                      { "strtype": "_style/Border width", "integer": 2 },
                      { "strtype": "_style/Gradient direction", "strval": "VER" }
                    ]}
                  ]}
              ]}
            ]}
          ]},
          "info": {}
        }"#;
        let imported = import_str(json, None, "x").unwrap();
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let chart = &imported.project.screens[0].children[0];
        assert_eq!(chart.prop_int("point_count"), 50);
        assert_eq!(chart.prop_pair("y2_range"), (0, 8000));
        assert_eq!(chart.prop_pair("x_major"), (10, 1));
        assert!(chart.prop_bool("x_labels"));
        assert_eq!(chart.prop_text("series"), "", "empty ChartData means no series");
        let get = |p| chart.styles.get("main", StateMask::DEFAULT, p).cloned();
        assert_eq!(get(StyleProp::PadLeft), Some(StyleValue::Int(2)));
        assert_eq!(get(StyleProp::PadRight), Some(StyleValue::Int(7)));
        assert_eq!(get(StyleProp::PadBottom), Some(StyleValue::Int(1)));
        assert_eq!(get(StyleProp::ShadowSpread), Some(StyleValue::Int(4)));
        assert_eq!(get(StyleProp::BorderWidth), Some(StyleValue::Int(2)));
        assert_eq!(get(StyleProp::BgGradDir), Some(StyleValue::Enum("LV_GRAD_DIR_VER".into())));
    }

    #[test]
    fn parses_chart_data_variants() {
        assert_eq!(chart_series("[]").as_deref(), Some(""));
        assert_eq!(chart_series("[[1, 2.4, 3]]").as_deref(), Some("#2196F3: 1, 2, 3"));
        let series = chart_series(r#"[{"color": [255, 0, 0], "axis": "SECONDARY_Y", "data": [5, 6]}]"#).unwrap();
        assert_eq!(series, "#FF0000 secondary: 5, 6");
        assert_eq!(chart_series("nonsense"), None);
    }

    #[test]
    fn rejects_non_projects() {
        assert!(import_str("{\"foo\": 1}", None, "x").is_err());
    }

    /// Imports the real SquareLine projects in `spj/` if present.
    #[test]
    fn imports_example_projects() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spj");
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.filter_map(Result::ok).map(|e| e.path()) {
            if path.extension().is_some_and(|e| e == "spj") {
                let imported = import_file(&path).unwrap();
                for warning in &imported.warnings {
                    eprintln!("{}: {warning}", path.display());
                }
                assert!(!imported.project.screens.is_empty());
                let count: usize = imported.project.screens.iter().map(|s| 1 + s.descendant_count()).sum();
                assert!(count > 1, "{} imported no widgets", path.display());
            }
        }
    }
}
