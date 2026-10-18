//! The project document and all structural edit operations on it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::color::Palette;
use crate::color_theme::ColorThemes;
use crate::prop::PropValue;
use crate::style::StyleValue;
use crate::widget::{Widget, WidgetId, WidgetKind};

/// File extension of studio projects.
pub const PROJECT_EXTENSION: &str = "lvglab";
const FORMAT_VERSION: u32 = 1;

lv_enum! {
    /// LVGL releases the generated code targets.
    #[derive(Default)]
    pub enum LvglVersion: "" {
        V9_6_0 = "9.6.0", "9.6.0";
        V9_5_0 = "9.5.0", "9.5.0";
        V9_4_0 = "9.4.0", "9.4.0";
        V9_3_0 = "9.3.0", "9.3.0";
        V9_2_2 = "9.2.2", "9.2.2";
        #[default]
        V8_3_11 = "8.3.11", "8.3.11";
        V8_3_10 = "8.3.10", "8.3.10";
        V8_3_9 = "8.3.9", "8.3.9";
        V8_3_8 = "8.3.8", "8.3.8";
        V8_3_7 = "8.3.7", "8.3.7";
        V8_3_6 = "8.3.6", "8.3.6";
        V8_3_0 = "8.3.0", "8.3.0";
    }
}

impl LvglVersion {
    /// Major and minor version, e.g. `(9, 2)`.
    pub fn major_minor(self) -> (u8, u8) {
        let mut parts = self.suffix().split('.').map(|p| p.parse().unwrap_or(0));
        (parts.next().unwrap_or(8), parts.next().unwrap_or(0))
    }

    pub fn is_v9(self) -> bool {
        self.major_minor().0 >= 9
    }

    /// The supported release closest to `text` (e.g. `9.1.0` → 9.2.2), for
    /// imported projects that target a release LVGLab does not list.
    pub fn nearest(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.').map(|p| p.parse::<u8>().ok());
        let (major, minor) = (parts.next()??, parts.next().flatten().unwrap_or(0));
        Self::ALL
            .iter()
            .copied()
            .filter(|v| v.major_minor().0 == major)
            .min_by_key(|v| v.major_minor().1.abs_diff(minor))
    }
}

lv_enum! {
    pub enum DisplayShape: "" {
        Rectangle = "RECTANGLE", "Rectangular";
        Round = "CIRCLE", "Round";
    }
}

/// Resolution and shape of the LVGL display.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub width: u32,
    pub height: u32,
    pub shape: DisplayShape,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self { width: 320, height: 240, shape: DisplayShape::Rectangle }
    }
}

/// Settings of LVGL's default theme (`lv_theme_default_init`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    pub dark: bool,
    pub primary: Palette,
    pub secondary: Palette,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self { dark: true, primary: Palette::Blue, secondary: Palette::Red }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageAsset {
    pub name: String,
    /// Path of the PNG/JPEG file, relative to the project file if possible.
    pub path: String,
}

/// A TrueType font rendered to an LVGL bitmap font on export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FontAsset {
    pub name: String,
    /// Path of the TTF/OTF file; empty if the font has to be provided manually.
    pub path: String,
    pub size: u16,
    /// Bits per pixel (1, 2, 4 or 8).
    pub bpp: u8,
    /// Unicode ranges, e.g. `0x20-0x7F, 0xB0`.
    pub ranges: String,
    /// Additional characters to include.
    pub symbols: String,
}

impl Default for FontAsset {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            size: 16,
            bpp: 4,
            ranges: "0x20-0x7E".to_owned(),
            symbols: "°äöüÄÖÜß€".to_owned(),
        }
    }
}

/// Where and how the UI library is exported.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportSettings {
    /// Name of the library folder (and of the PlatformIO/Arduino library).
    pub library_name: String,
    /// Folder the library is written into, relative to the project file
    /// (e.g. the `lib` folder of a PlatformIO project).
    pub directory: String,
    /// Prefix of every generated C symbol and file (`ui` → `ui_Main01`,
    /// `ui_img_logo_png`, `ui_theme_set`). Use different prefixes for UI
    /// libraries that are linked into the same firmware.
    pub prefix: String,
    /// Wrap translatable texts in `_()` for the lv_i18n library and
    /// generate `<prefix>_relocalize()`.
    pub multilang: bool,
    /// Store image pixels byte swapped (`LV_COLOR_16_SWAP 1`).
    pub swap_bytes: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            library_name: "ui".to_owned(),
            directory: "export".to_owned(),
            prefix: "ui".to_owned(),
            multilang: false,
            swap_bytes: false,
        }
    }
}

impl ExportSettings {
    /// The prefix as a valid C identifier (falls back to `ui`).
    pub fn symbol_prefix(&self) -> String {
        let prefix: String = self.prefix.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if prefix.is_empty() || prefix.starts_with(|c: char| c.is_ascii_digit()) { "ui".to_owned() } else { prefix }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Image,
    Font,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Assets {
    pub images: Vec<ImageAsset>,
    pub fonts: Vec<FontAsset>,
}

impl Assets {
    pub fn image(&self, name: &str) -> Option<&ImageAsset> {
        self.images.iter().find(|i| i.name == name)
    }

    pub fn font(&self, name: &str) -> Option<&FontAsset> {
        self.fonts.iter().find(|f| f.name == name)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("cannot access `{path}`: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("invalid project file `{path}`: {source}")]
    Parse { path: PathBuf, source: serde_json::Error },
    #[error("project file `{path}` has format version {found}, this build supports up to {FORMAT_VERSION}")]
    UnsupportedVersion { path: PathBuf, found: u32 },
}

/// Where a widget lives in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub parent: Option<WidgetId>,
    pub index: usize,
}

/// A studio project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub format_version: u32,
    pub name: String,
    pub lvgl_version: LvglVersion,
    pub display: DisplaySettings,
    pub theme: ThemeSettings,
    pub export: ExportSettings,
    pub assets: Assets,
    /// Color themes with variables that styles can bind to.
    pub color_themes: ColorThemes,
    pub screens: Vec<Widget>,
    /// Reusable components. Each one is stored like a screen whose only
    /// child is the component's root object; the screen itself is not
    /// exported. Its name is the component name.
    pub components: Vec<Widget>,
    /// Next free widget id.
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: "MyProject".to_owned(),
            lvgl_version: LvglVersion::default(),
            display: DisplaySettings::default(),
            theme: ThemeSettings::default(),
            export: ExportSettings::default(),
            color_themes: ColorThemes::default(),
            assets: Assets::default(),
            screens: Vec::new(),
            components: Vec::new(),
            next_id: 1,
        }
    }
}

impl Project {
    /// A new project with one empty screen.
    pub fn new(name: impl Into<String>) -> Self {
        let mut project = Self { name: name.into(), ..Self::default() };
        project.add_screen();
        project
    }

    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let text = std::fs::read_to_string(path).map_err(|source| ProjectError::Io { path: path.into(), source })?;
        let mut project: Self =
            serde_json::from_str(&text).map_err(|source| ProjectError::Parse { path: path.into(), source })?;
        if project.format_version > FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion { path: path.into(), found: project.format_version });
        }
        project.repair_ids();
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        let text = serde_json::to_string_pretty(self).expect("project serialization cannot fail");
        std::fs::write(path, text).map_err(|source| ProjectError::Io { path: path.into(), source })
    }

    pub fn alloc_id(&mut self) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Makes sure `next_id` is above every id in use and ids are unique.
    pub fn repair_ids(&mut self) {
        let mut seen = HashSet::new();
        let mut max = 0;
        let mut duplicates = 0;
        for screen in self.screens.iter().chain(&self.components) {
            screen.walk(&mut |w| {
                max = max.max(w.id.0);
                if !seen.insert(w.id) {
                    duplicates += 1;
                }
            });
        }
        self.next_id = self.next_id.max(max + 1);
        if duplicates > 0 {
            let mut seen = HashSet::new();
            let mut next = self.next_id;
            for screen in self.screens.iter_mut().chain(&mut self.components) {
                screen.walk_mut(&mut |w| {
                    if !seen.insert(w.id) {
                        w.id = WidgetId(next);
                        next += 1;
                    }
                });
            }
            self.next_id = next;
        }
    }

    // -- lookup -----------------------------------------------------------

    /// Screens and component definitions.
    pub(crate) fn roots(&self) -> impl Iterator<Item = &Widget> {
        self.screens.iter().chain(&self.components)
    }

    pub(crate) fn roots_mut(&mut self) -> impl Iterator<Item = &mut Widget> {
        self.screens.iter_mut().chain(&mut self.components)
    }

    /// A widget of a screen or of a component.
    pub fn widget(&self, id: WidgetId) -> Option<&Widget> {
        self.roots().find_map(|s| s.find(id))
    }

    pub fn widget_mut(&mut self, id: WidgetId) -> Option<&mut Widget> {
        self.roots_mut().find_map(|s| s.find_mut(id))
    }

    /// The screen (or component) containing `id`.
    pub fn screen_of(&self, id: WidgetId) -> Option<&Widget> {
        self.roots().find(|s| s.find(id).is_some())
    }

    /// Ancestors of `id` from its screen down to the widget itself.
    pub fn path_to(&self, id: WidgetId) -> Option<Vec<&Widget>> {
        self.roots().find_map(|s| s.path_to(id))
    }

    pub fn is_component(&self, id: WidgetId) -> bool {
        self.components.iter().any(|c| c.id == id)
    }

    /// The component definition named `name`.
    pub fn component(&self, name: &str) -> Option<&Widget> {
        self.components.iter().find(|c| c.name == name)
    }

    /// Puts all top level objects of a component into one container, because
    /// an instance is made of a single object. The container has the size of
    /// the display and does not move the objects; resize it as needed.
    pub fn group_component_roots(&mut self, id: WidgetId) -> bool {
        let Some(index) = self.components.iter().position(|c| c.id == id) else { return false };
        if self.components[index].children.len() < 2 {
            return false;
        }
        let name = self.unique_name(&format!("{}Content", self.components[index].name));
        let mut group = Widget::new(WidgetKind::Container, self.alloc_id(), name);
        group.geometry.align = crate::coord::Align::TopLeft;
        group.geometry.width = crate::coord::Coord::px(self.display.width as i32);
        group.geometry.height = crate::coord::Coord::px(self.display.height as i32);
        group.set_flag(crate::flags::ObjFlag::Clickable, false);
        group.set_flag(crate::flags::ObjFlag::Scrollable, false);
        group.children = std::mem::take(&mut self.components[index].children);
        self.components[index].children = vec![group];
        true
    }

    /// Points the instances of component `old` to `new` after a rename.
    pub fn rename_component_instances(&mut self, old: &str, new: &str) {
        for screen in self.roots_mut() {
            screen.walk_mut(&mut |w| {
                if w.kind == WidgetKind::Component && w.prop_text("component") == old {
                    w.set_prop("component", PropValue::Text(new.to_owned()));
                }
            });
        }
    }

    /// Adds an empty component (a container) and returns the id of its
    /// definition.
    pub fn add_component(&mut self) -> WidgetId {
        let id = self.alloc_id();
        let name =
            (1..).map(|n| format!("Component{n}")).find(|n| self.component(n).is_none()).expect("infinite iterator");
        let mut definition = Widget::new(WidgetKind::Screen, id, name.clone());
        let mut root = self.new_widget(WidgetKind::Container);
        root.name = name;
        root.geometry.width = crate::Coord::px(200);
        root.geometry.height = crate::Coord::px(80);
        definition.children.push(root);
        self.components.push(definition);
        id
    }

    /// A copy of `screen` with every component instance replaced by the
    /// component's root object, for the preview. The instance keeps its id,
    /// name, position, alignment and events; its local styles override the
    /// root's. Objects inside the component get ids at or above
    /// [`EXPANDED_ID_BASE`], which do not exist in the project.
    pub fn expand_instances(&self, screen: &Widget) -> Widget {
        let mut copy = screen.clone();
        let mut next = EXPANDED_ID_BASE;
        self.expand_into(&mut copy, &mut next, 0);
        copy
    }

    fn expand_into(&self, widget: &mut Widget, next: &mut u64, depth: usize) {
        if widget.kind == WidgetKind::Component && depth < 8 {
            let name = widget.prop_text("component");
            if let Some(root) = self.component(&name).and_then(|c| c.children.first()) {
                let mut expanded = root.clone();
                expanded.walk_mut(&mut |w| {
                    w.id = WidgetId(*next);
                    *next += 1;
                });
                expanded.id = widget.id;
                expanded.name.clone_from(&widget.name);
                expanded.geometry.x = widget.geometry.x;
                expanded.geometry.y = widget.geometry.y;
                expanded.geometry.align = widget.geometry.align;
                if widget.geometry.width.unit != crate::coord::Unit::Content {
                    expanded.geometry.width = widget.geometry.width;
                }
                if widget.geometry.height.unit != crate::coord::Unit::Content {
                    expanded.geometry.height = widget.geometry.height;
                }
                for (flag, on) in &widget.flags {
                    expanded.set_flag(*flag, *on);
                }
                expanded.states.extend(widget.states.iter().copied());
                for block in widget.styles.blocks() {
                    for (prop, value) in &block.props {
                        expanded.styles.set(&block.slot, block.state, *prop, value.clone());
                    }
                }
                expanded.events.clone_from(&widget.events);
                *widget = expanded;
                for child in &mut widget.children {
                    self.expand_into(child, next, depth + 1);
                }
                return;
            }
        }
        for child in &mut widget.children {
            self.expand_into(child, next, depth);
        }
    }

    pub fn location(&self, id: WidgetId) -> Option<Location> {
        if let Some(index) = self.screens.iter().position(|s| s.id == id) {
            return Some(Location { parent: None, index });
        }
        let path = self.path_to(id)?;
        let parent = path[path.len() - 2];
        let index = parent.children.iter().position(|c| c.id == id)?;
        Some(Location { parent: Some(parent.id), index })
    }

    pub fn all_widgets(&self) -> Vec<&Widget> {
        let mut out = Vec::new();
        for screen in &self.screens {
            screen.walk(&mut |w| out.push(w));
        }
        out
    }

    /// Widgets of all screens and of all component definitions.
    pub fn every_widget(&self) -> Vec<&Widget> {
        let mut out = Vec::new();
        for root in self.roots() {
            root.walk(&mut |w| out.push(w));
        }
        out
    }

    /// True if any widget other than `except` already uses `name`.
    pub fn name_in_use(&self, name: &str, except: Option<WidgetId>) -> bool {
        self.all_widgets().iter().any(|w| w.name == name && Some(w.id) != except)
    }

    /// A free name based on `base`: `Button1`, `Button2`, ...
    pub fn unique_name(&self, base: &str) -> String {
        let base: String = base.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let base = if base.is_empty() { "Widget".to_owned() } else { base };
        (1..).map(|n| format!("{base}{n}")).find(|name| !self.name_in_use(name, None)).expect("infinite iterator")
    }

    // -- edits ------------------------------------------------------------

    pub fn add_screen(&mut self) -> WidgetId {
        let id = self.alloc_id();
        let name = self.unique_name("Screen");
        self.screens.push(Widget::new(WidgetKind::Screen, id, name));
        id
    }

    /// Creates a widget of `kind` with a unique name and default content.
    pub fn new_widget(&mut self, kind: WidgetKind) -> Widget {
        let id = self.alloc_id();
        let base = kind.def().name.replace(' ', "");
        let mut widget = Widget::new(kind, id, self.unique_name(&base));
        if kind == WidgetKind::Component {
            if let Some(component) = self.components.first() {
                widget.set_prop("component", PropValue::Text(component.name.clone()));
            }
        }
        if kind == WidgetKind::Button {
            let mut label = Widget::new(WidgetKind::Label, self.alloc_id(), format!("{}Label", widget.name));
            label.set_prop("text", PropValue::Text(widget.name.clone()));
            widget.children.push(label);
        }
        widget
    }

    /// Inserts `widget` into `parent` at `index` (appends if out of range).
    /// Returns the widget back if the parent does not accept it.
    pub fn insert(&mut self, parent: WidgetId, index: usize, widget: Widget) -> Result<(), Box<Widget>> {
        let Some(target) = self.widget_mut(parent) else {
            return Err(Box::new(widget));
        };
        if !target.def().accepts_child(widget.kind) {
            return Err(Box::new(widget));
        }
        let index = index.min(target.children.len());
        target.children.insert(index, widget);
        Ok(())
    }

    /// Removes a widget (or a screen or component) and clears references to it.
    pub fn remove(&mut self, id: WidgetId) -> Option<Widget> {
        if let Some(index) = self.components.iter().position(|c| c.id == id) {
            return Some(self.components.remove(index));
        }
        let location = self.location(id)?;
        let removed = match location.parent {
            None => self.screens.remove(location.index),
            Some(parent) => self.widget_mut(parent)?.children.remove(location.index),
        };
        let mut gone = HashSet::new();
        removed.walk(&mut |w| {
            gone.insert(w.id);
        });
        self.clear_references(&gone);
        Some(removed)
    }

    fn clear_references(&mut self, gone: &HashSet<WidgetId>) {
        for screen in self.roots_mut() {
            screen.walk_mut(&mut |w| {
                for value in w.props.values_mut() {
                    if let PropValue::WidgetRef(Some(target)) = value {
                        if gone.contains(target) {
                            *value = PropValue::WidgetRef(None);
                        }
                    }
                }
                w.events.retain(|e| e.action.references().is_none_or(|r| !gone.contains(&r)));
            });
        }
    }

    /// Moves a widget to a new parent/index. Fails if the target is the
    /// widget itself, one of its descendants or does not accept it.
    pub fn move_widget(&mut self, id: WidgetId, new_parent: WidgetId, index: usize) -> bool {
        let Some(widget) = self.widget(id) else { return false };
        if widget.find(new_parent).is_some() || widget.kind == WidgetKind::Screen {
            return false;
        }
        let kind = widget.kind;
        if !self.widget(new_parent).is_some_and(|p| p.def().accepts_child(kind)) {
            return false;
        }
        let Some(location) = self.location(id) else { return false };
        let Some(old_parent) = location.parent else { return false };
        let widget = self.widget_mut(old_parent).expect("parent exists").children.remove(location.index);
        let index = if old_parent == new_parent && index > location.index { index - 1 } else { index };
        let parent = self.widget_mut(new_parent).expect("checked above");
        let index = index.min(parent.children.len());
        parent.children.insert(index, widget);
        true
    }

    /// Moves a widget (or screen or component) one step within its siblings.
    pub fn shift(&mut self, id: WidgetId, delta: isize) -> bool {
        if let Some(index) = self.components.iter().position(|c| c.id == id) {
            let Some(new_index) = index.checked_add_signed(delta).filter(|i| *i < self.components.len()) else {
                return false;
            };
            self.components.swap(index, new_index);
            return true;
        }
        let Some(location) = self.location(id) else { return false };
        let siblings = match location.parent {
            None => &mut self.screens,
            Some(parent) => &mut self.widget_mut(parent).expect("parent exists").children,
        };
        let Some(new_index) = location.index.checked_add_signed(delta).filter(|i| *i < siblings.len()) else {
            return false;
        };
        siblings.swap(location.index, new_index);
        true
    }

    /// Deep copy of a widget with fresh ids and names.
    pub fn clone_with_new_ids(&mut self, widget: &Widget) -> Widget {
        let mut copy = widget.clone();
        let mut taken: HashSet<String> = self.all_widgets().iter().map(|w| w.name.clone()).collect();
        copy.walk_mut(&mut |w| {
            w.id = WidgetId(self.next_id);
            self.next_id += 1;
            let stem = w.name.trim_end_matches(|c: char| c.is_ascii_digit()).to_owned();
            let stem = if stem.is_empty() { "Widget".to_owned() } else { stem };
            w.name = (1..).map(|n| format!("{stem}{n}")).find(|n| !taken.contains(n)).expect("infinite iterator");
            taken.insert(w.name.clone());
        });
        copy
    }

    /// Duplicates a widget next to the original; returns the new id.
    pub fn duplicate(&mut self, id: WidgetId) -> Option<WidgetId> {
        if let Some(index) = self.components.iter().position(|c| c.id == id) {
            let original = self.components[index].clone();
            let mut copy = self.clone_with_new_ids(&original);
            copy.name = (2..)
                .map(|n| format!("{}{n}", original.name.trim_end_matches(|c: char| c.is_ascii_digit())))
                .find(|n| self.component(n).is_none())
                .expect("infinite iterator");
            let new_id = copy.id;
            self.components.insert(index + 1, copy);
            return Some(new_id);
        }
        let location = self.location(id)?;
        let original = self.widget(id)?.clone();
        let copy = self.clone_with_new_ids(&original);
        let new_id = copy.id;
        match location.parent {
            None => self.screens.insert(location.index + 1, copy),
            Some(parent) => self.insert(parent, location.index + 1, copy).ok()?,
        }
        Some(new_id)
    }

    /// Renames an image or font asset and updates every reference to it.
    pub fn rename_asset(&mut self, kind: AssetKind, old: &str, new: &str) {
        match kind {
            AssetKind::Image => {
                if let Some(image) = self.assets.images.iter_mut().find(|i| i.name == old) {
                    image.name = new.to_owned();
                }
            }
            AssetKind::Font => {
                if let Some(font) = self.assets.fonts.iter_mut().find(|f| f.name == old) {
                    font.name = new.to_owned();
                }
            }
        }
        for screen in self.roots_mut() {
            screen.walk_mut(&mut |w| {
                for block in w.styles.blocks_mut() {
                    for value in block.props.values_mut() {
                        match (kind, value) {
                            (AssetKind::Image, StyleValue::Image(name)) | (AssetKind::Font, StyleValue::Font(name))
                                if name == old =>
                            {
                                *name = new.to_owned();
                            }
                            _ => {}
                        }
                    }
                }
                if kind == AssetKind::Image {
                    for value in w.props.values_mut() {
                        match value {
                            PropValue::Image(name) if name == old => *name = new.to_owned(),
                            PropValue::Text(lines) if lines.lines().any(|l| l.trim() == old) => {
                                *lines = lines
                                    .lines()
                                    .map(|l| if l.trim() == old { new } else { l })
                                    .collect::<Vec<_>>()
                                    .join(
                                        "
",
                                    );
                            }
                            _ => {}
                        }
                    }
                }
            });
        }
    }

    /// Renames a color theme variable and every style bound to it. Returns
    /// false if `new` is empty or already used.
    pub fn rename_theme_variable(&mut self, old: &str, new: &str) -> bool {
        let new = new.trim();
        let themes = &mut self.color_themes;
        let Some(index) = themes.index_of(old) else { return false };
        if new.is_empty() || themes.index_of(new).is_some() {
            return false;
        }
        themes.variables[index] = new.to_owned();
        for screen in self.roots_mut() {
            screen.walk_mut(&mut |w| {
                for block in w.styles.blocks_mut() {
                    for value in block.props.values_mut() {
                        if value.as_theme() == Some(old) {
                            *value = StyleValue::Theme(new.to_owned());
                        }
                    }
                }
            });
        }
        true
    }

    /// Makes a path absolute relative to the project directory.
    pub fn resolve_path(project_dir: Option<&Path>, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        match project_dir {
            Some(dir) if path.is_relative() => dir.join(path),
            _ => path,
        }
    }

    /// Copies every image and font file the project uses into
    /// `<project_dir>/assets` and makes the paths relative, so the project
    /// folder is self-contained. `old_dir` resolves the current relative
    /// paths. Returns the asset files that do not exist.
    pub fn collect_assets(&mut self, old_dir: Option<&Path>, project_dir: &Path) -> Result<Vec<String>, ProjectError> {
        let assets_dir = project_dir.join(ASSETS_DIR);
        let mut copied: HashMap<PathBuf, String> = HashMap::new();
        let mut used: HashSet<String> = HashSet::new();
        let mut missing = Vec::new();
        let paths =
            self.assets.images.iter_mut().map(|i| &mut i.path).chain(self.assets.fonts.iter_mut().map(|f| &mut f.path));
        for path in paths {
            if path.trim().is_empty() {
                continue;
            }
            let source = Self::resolve_path(old_dir, path);
            if let Some(relative) = copied.get(&source) {
                *path = relative.clone();
                continue;
            }
            if !source.is_file() {
                missing.push(source.display().to_string());
                // Keep the reference valid from the new location.
                *path = source.display().to_string();
                continue;
            }
            let relative = match source.strip_prefix(project_dir) {
                Ok(inside) => inside.to_string_lossy().replace('\\', "/"),
                Err(_) => {
                    let target = unique_target(&assets_dir, &source, &mut used);
                    std::fs::create_dir_all(&assets_dir)
                        .map_err(|e| ProjectError::Io { path: assets_dir.clone(), source: e })?;
                    if !target.exists() || !same_contents(&source, &target) {
                        std::fs::copy(&source, &target)
                            .map_err(|e| ProjectError::Io { path: target.clone(), source: e })?;
                    }
                    let file = target.file_name().unwrap_or_default().to_string_lossy();
                    format!("{ASSETS_DIR}/{file}")
                }
            };
            used.insert(relative.to_ascii_lowercase());
            copied.insert(source, relative.clone());
            *path = relative;
        }
        Ok(missing)
    }
}

/// First id of objects created by [`Project::expand_instances`].
pub const EXPANDED_ID_BASE: u64 = 1 << 48;

/// Folder next to the project file that holds its images and fonts.
pub const ASSETS_DIR: &str = "assets";

/// `assets/<file>`; a different file with the same name gets a suffix.
fn unique_target(assets_dir: &Path, source: &Path, used: &mut HashSet<String>) -> PathBuf {
    let stem = source.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let extension = source.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut n = 1;
    loop {
        let file = if n == 1 { format!("{stem}{extension}") } else { format!("{stem}_{n}{extension}") };
        let target = assets_dir.join(&file);
        let key = format!("{ASSETS_DIR}/{file}").to_ascii_lowercase();
        let free = !used.contains(&key) && (!target.exists() || same_contents(source, &target));
        if free {
            used.insert(key);
            return target;
        }
        n += 1;
    }
}

fn same_contents(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Action, EventHandler, EventTrigger, ScreenAnim};

    #[test]
    fn grouping_puts_the_top_level_objects_of_a_component_into_one_container() {
        let mut p = Project::new("P");
        p.display.width = 600;
        p.display.height = 450;
        let component = p.add_component();
        let panel = p.new_widget(WidgetKind::Panel);
        let panel_id = panel.id;
        p.insert(component, usize::MAX, panel).unwrap();
        assert_eq!(p.components[0].children.len(), 2);
        assert!(p.group_component_roots(component));
        let root = &p.components[0].children;
        assert_eq!(root.len(), 1);
        assert_eq!((root[0].kind, root[0].children.len()), (WidgetKind::Container, 2));
        assert_eq!((root[0].geometry.width.value, root[0].geometry.height.value), (600, 450));
        assert!(p.widget(panel_id).is_some(), "the objects keep their ids");
        assert!(!p.group_component_roots(component), "a single object is left alone");
    }

    #[test]
    fn lvgl_versions() {
        assert_eq!(LvglVersion::V9_3_0.major_minor(), (9, 3));
        assert!(!LvglVersion::default().is_v9());
        assert_eq!(LvglVersion::nearest("9.1.0"), Some(LvglVersion::V9_2_2));
        assert_eq!(LvglVersion::nearest("8.3.2"), Some(LvglVersion::V8_3_11));
        assert_eq!(LvglVersion::nearest("10.0"), None);
    }

    #[test]
    fn instances_expand_to_their_component() {
        let mut project = Project::new("P");
        let definition = project.add_component();
        let root = project.widget(definition).unwrap().children[0].id;
        let label = project.new_widget(WidgetKind::Label);
        project.insert(root, 0, label).unwrap();
        let screen = project.screens[0].id;
        let mut instance = project.new_widget(WidgetKind::Component);
        assert_eq!(instance.prop_text("component"), "Component1");
        instance.geometry.x = crate::Coord::px(30);
        let instance_id = instance.id;
        project.insert(screen, 0, instance).unwrap();

        let expanded = project.expand_instances(&project.screens[0]);
        let shown = &expanded.children[0];
        assert_eq!(
            (shown.id, shown.kind, shown.geometry.x),
            (instance_id, WidgetKind::Container, crate::Coord::px(30))
        );
        assert!(shown.children[0].id.0 >= EXPANDED_ID_BASE);

        project.widget_mut(definition).unwrap().name = "Gauge".into();
        project.rename_component_instances("Component1", "Gauge");
        assert_eq!(project.widget(instance_id).unwrap().prop_text("component"), "Gauge");
        assert!(project.remove(definition).is_some());
        assert!(project.components.is_empty());
    }

    #[test]
    fn collects_assets_into_the_project_folder() {
        let root = std::env::temp_dir().join(format!("lvglab-collect-{}", std::process::id()));
        let source = root.join("source");
        std::fs::create_dir_all(source.join("icons")).unwrap();
        std::fs::write(source.join("logo.png"), b"a").unwrap();
        std::fs::write(source.join("icons/logo.png"), b"b").unwrap();
        std::fs::write(source.join("font.ttf"), b"ttf").unwrap();
        let mut project = Project::new("P");
        let image = |name: &str, path: &Path| ImageAsset { name: name.into(), path: path.display().to_string() };
        project.assets.images.push(image("logo", &source.join("logo.png")));
        project.assets.images.push(image("icon", &source.join("icons/logo.png")));
        project.assets.images.push(image("gone", &source.join("gone.png")));
        for size in [20, 40] {
            project.assets.fonts.push(FontAsset {
                name: format!("F{size}"),
                path: source.join("font.ttf").display().to_string(),
                size,
                ..FontAsset::default()
            });
        }
        let dir = root.join("project");
        let missing = project.collect_assets(None, &dir).unwrap();
        assert_eq!(missing.len(), 1);
        let paths: Vec<&str> = project.assets.images.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(&paths[..2], ["assets/logo.png", "assets/logo_2.png"]);
        assert_eq!(project.assets.fonts[0].path, "assets/font.ttf");
        assert_eq!(project.assets.fonts[1].path, "assets/font.ttf");
        assert_eq!(std::fs::read(dir.join("assets/logo_2.png")).unwrap(), b"b");

        // Saving again in place keeps everything; saving elsewhere copies again.
        project.collect_assets(Some(&dir), &dir).unwrap();
        assert_eq!(project.assets.images[1].path, "assets/logo_2.png");
        let other = root.join("other");
        project.collect_assets(Some(&dir), &other).unwrap();
        assert_eq!(std::fs::read(other.join("assets/logo_2.png")).unwrap(), b"b");
        std::fs::remove_dir_all(root).ok();
    }

    fn sample() -> (Project, WidgetId, WidgetId) {
        let mut project = Project::new("Test");
        let screen = project.screens[0].id;
        let panel = project.new_widget(WidgetKind::Panel);
        let panel_id = panel.id;
        project.insert(screen, 0, panel).unwrap();
        let button = project.new_widget(WidgetKind::Button);
        let button_id = button.id;
        project.insert(panel_id, 0, button).unwrap();
        (project, panel_id, button_id)
    }

    #[test]
    fn new_button_contains_label() {
        let (project, _, button) = sample();
        let button = project.widget(button).unwrap();
        assert_eq!(button.name, "Button1");
        assert_eq!(button.children[0].kind, WidgetKind::Label);
        assert_eq!(button.children[0].prop_text("text"), "Button1");
    }

    #[test]
    fn unique_names_increment() {
        let (project, ..) = sample();
        assert_eq!(project.unique_name("Panel"), "Panel2");
        assert_eq!(project.unique_name("Tab view"), "Tabview1");
    }

    #[test]
    fn move_rejects_cycles_and_invalid_parents() {
        let (mut project, panel, button) = sample();
        assert!(!project.move_widget(panel, button, 0));
        assert!(!project.move_widget(panel, panel, 0));
        let screen = project.screens[0].id;
        assert!(project.move_widget(button, screen, 0));
        assert_eq!(project.location(button), Some(Location { parent: Some(screen), index: 0 }));
    }

    #[test]
    fn remove_clears_references() {
        let (mut project, panel, button) = sample();
        let second = project.add_screen();
        project.widget_mut(button).unwrap().events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::ChangeScreen { screen: Some(second), anim: ScreenAnim::None, time_ms: 0, delay_ms: 0 },
        });
        project.remove(second).unwrap();
        assert!(project.widget(button).unwrap().events.is_empty());
        assert!(project.remove(panel).is_some());
        assert!(project.widget(button).is_none());
    }

    #[test]
    fn duplicate_assigns_fresh_ids_and_names() {
        let (mut project, panel, _) = sample();
        let copy = project.duplicate(panel).unwrap();
        let copy = project.widget(copy).unwrap();
        assert_eq!(copy.name, "Panel2");
        assert_eq!(copy.children[0].name, "Button2");
        let ids: HashSet<_> = project.all_widgets().iter().map(|w| w.id).collect();
        assert_eq!(ids.len(), project.all_widgets().len());
    }

    #[test]
    fn shift_reorders_siblings() {
        let (mut project, ..) = sample();
        let second = project.add_screen();
        assert!(project.shift(second, -1));
        assert_eq!(project.screens[0].id, second);
        assert!(!project.shift(second, -1));
    }

    #[test]
    fn rename_asset_updates_references() {
        let (mut project, panel, _) = sample();
        project.assets.fonts.push(FontAsset { name: "Big".into(), ..FontAsset::default() });
        let state = crate::flags::StateMask::DEFAULT;
        let font = crate::style::StyleProp::TextFont;
        project.widget_mut(panel).unwrap().styles.set("main", state, font, StyleValue::Font("Big".into()));
        project.rename_asset(AssetKind::Font, "Big", "Huge");
        assert_eq!(project.assets.fonts[0].name, "Huge");
        assert_eq!(
            project.widget(panel).unwrap().styles.get("main", state, font),
            Some(&StyleValue::Font("Huge".into()))
        );
    }

    #[test]
    fn save_and_load_round_trip() {
        let (project, ..) = sample();
        let dir = std::env::temp_dir().join(format!("lvglab-model-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.lvglab");
        project.save(&path).unwrap();
        let loaded = Project::load(&path).unwrap();
        assert_eq!(loaded, project);
        std::fs::remove_dir_all(dir).ok();
    }
}
