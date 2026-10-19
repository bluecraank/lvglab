use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, WidgetDef};
use crate::coord::{Align, Coord};
use crate::event::EventHandler;
use crate::flags::{Dir, FlexAlign, FlexFlow, ObjFlag, ObjState, ScrollbarMode, SnapMode, StateMask};
use crate::prop::PropValue;
use crate::style::Styles;

/// Project wide unique and stable identifier of a widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WidgetId(pub u64);

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// All widget types of LVGL 8.3 (core and extra widgets) plus the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    Screen,
    Panel,
    Container,
    Label,
    Button,
    Image,
    ImgButton,
    Line,
    Arc,
    Bar,
    Slider,
    Switch,
    Checkbox,
    Dropdown,
    Roller,
    Textarea,
    Keyboard,
    BtnMatrix,
    Spinbox,
    Colorwheel,
    Spinner,
    Led,
    Meter,
    Chart,
    Table,
    Calendar,
    Canvas,
    AnimImg,
    Spangroup,
    Tabview,
    TabPage,
    Tileview,
    Tile,
    Win,
    List,
    Menu,
    Msgbox,
    /// An instance of a project component.
    Component,
}

impl WidgetKind {
    pub fn def(self) -> &'static WidgetDef {
        catalog::def(self)
    }

    pub fn name(self) -> &'static str {
        self.def().name
    }
}

/// Position, size and alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: Coord,
    pub y: Coord,
    pub width: Coord,
    pub height: Coord,
    pub align: Align,
}

impl Default for Geometry {
    fn default() -> Self {
        Self { x: Coord::px(0), y: Coord::px(0), width: Coord::px(100), height: Coord::px(50), align: Align::TopLeft }
    }
}

/// Scroll settings; `None` keeps the LVGL default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scrollbar_mode: Option<ScrollbarMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<Dir>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snap_x: Option<SnapMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snap_y: Option<SnapMode>,
}

impl ScrollSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Flex layout of a container (`lv_obj_set_flex_flow/align`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexLayout {
    pub flow: FlexFlow,
    pub main_place: FlexAlign,
    pub cross_place: FlexAlign,
    pub track_place: FlexAlign,
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self {
            flow: FlexFlow::Row,
            main_place: FlexAlign::Start,
            cross_place: FlexAlign::Start,
            track_place: FlexAlign::Start,
        }
    }
}

/// One node of the UI tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Widget {
    pub id: WidgetId,
    pub kind: WidgetKind,
    pub name: String,
    #[serde(default)]
    pub geometry: Geometry,
    /// Flags that differ from the widget's LVGL defaults.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub flags: BTreeMap<ObjFlag, bool>,
    /// States added after creation (`lv_obj_add_state`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub states: BTreeSet<ObjState>,
    #[serde(default, skip_serializing_if = "ScrollSettings::is_default")]
    pub scroll: ScrollSettings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<FlexLayout>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ext_click_area: i32,
    /// Widget specific properties; missing keys use the catalog default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, PropValue>,
    #[serde(default, skip_serializing_if = "Styles::is_empty")]
    pub styles: Styles,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EventHandler>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Widget>,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

impl Widget {
    /// Takes the look and behavior of `source` (a widget of the same kind):
    /// properties, flags, states, scrolling, layout, local styles and events.
    /// Identity, name, position, size, children and the component an
    /// instance refers to stay as they are.
    pub fn copy_look_from(&mut self, source: &Widget) -> bool {
        if self.kind != source.kind {
            return false;
        }
        let component = self.props.get("component").cloned();
        self.props = source.props.clone();
        if let Some(component) = component {
            self.props.insert("component".to_owned(), component);
        } else {
            self.props.remove("component");
        }
        self.flags = source.flags.clone();
        self.states = source.states.clone();
        self.scroll = source.scroll;
        self.layout = source.layout;
        self.ext_click_area = source.ext_click_area;
        self.styles = source.styles.clone();
        self.events = source.events.clone();
        true
    }

    /// Creates a widget with the catalog defaults for `kind`.
    pub fn new(kind: WidgetKind, id: WidgetId, name: impl Into<String>) -> Self {
        let def = kind.def();
        Self {
            id,
            kind,
            name: name.into(),
            geometry: Geometry { width: def.width, height: def.height, align: def.align, ..Geometry::default() },
            flags: def.initial_flags.iter().copied().collect(),
            states: BTreeSet::new(),
            scroll: ScrollSettings::default(),
            layout: None,
            ext_click_area: 0,
            props: BTreeMap::new(),
            styles: Styles::default(),
            events: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn def(&self) -> &'static WidgetDef {
        self.kind.def()
    }

    /// The value of a widget property, falling back to its default.
    /// Returns `None` for keys the widget kind does not know.
    pub fn prop(&self, key: &str) -> Option<PropValue> {
        let def = self.def().prop(key)?;
        Some(match self.props.get(key) {
            Some(value) if value.fits(def.kind) => value.clone(),
            _ => def.default_value(),
        })
    }

    pub fn prop_text(&self, key: &str) -> String {
        self.prop(key).and_then(|v| v.as_text().map(str::to_owned)).unwrap_or_default()
    }

    pub fn prop_int(&self, key: &str) -> i32 {
        self.prop(key).and_then(|v| v.as_int()).unwrap_or_default()
    }

    pub fn prop_bool(&self, key: &str) -> bool {
        self.prop(key).and_then(|v| v.as_bool()).unwrap_or_default()
    }

    pub fn prop_pair(&self, key: &str) -> (i32, i32) {
        self.prop(key).and_then(|v| v.as_pair()).unwrap_or_default()
    }

    /// Sets a property; values equal to the default are removed so project
    /// files only contain deliberate changes.
    pub fn set_prop(&mut self, key: &str, value: PropValue) {
        match self.def().prop(key) {
            Some(def) if def.default_value() == value => {
                self.props.remove(key);
            }
            _ => {
                self.props.insert(key.to_owned(), value);
            }
        }
    }

    /// Effective value of an object flag.
    pub fn flag(&self, flag: ObjFlag) -> bool {
        self.flags.get(&flag).copied().unwrap_or_else(|| self.def().default_flag(flag))
    }

    pub fn set_flag(&mut self, flag: ObjFlag, on: bool) {
        if self.def().default_flag(flag) == on {
            self.flags.remove(&flag);
        } else {
            self.flags.insert(flag, on);
        }
    }

    pub fn is_hidden(&self) -> bool {
        self.flag(ObjFlag::Hidden)
    }

    pub fn state_mask(&self) -> StateMask {
        self.states.iter().fold(StateMask::DEFAULT, |mask, s| mask.with(*s))
    }

    pub fn find(&self, id: WidgetId) -> Option<&Widget> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }

    pub fn find_mut(&mut self, id: WidgetId) -> Option<&mut Widget> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter_mut().find_map(|c| c.find_mut(id))
    }

    /// Path of widgets from `self` down to `id` (both inclusive).
    pub fn path_to(&self, id: WidgetId) -> Option<Vec<&Widget>> {
        if self.id == id {
            return Some(vec![self]);
        }
        self.children.iter().find_map(|c| {
            c.path_to(id).map(|mut path| {
                path.insert(0, self);
                path
            })
        })
    }

    /// Visits the widget and all descendants depth first.
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Widget)) {
        visit(self);
        for child in &self.children {
            child.walk(visit);
        }
    }

    pub fn walk_mut(&mut self, visit: &mut impl FnMut(&mut Widget)) {
        visit(self);
        for child in &mut self.children {
            child.walk_mut(visit);
        }
    }

    pub fn descendant_count(&self) -> usize {
        self.children.iter().map(|c| 1 + c.descendant_count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_widget_uses_catalog_defaults() {
        let label = Widget::new(WidgetKind::Label, WidgetId(1), "Label1");
        assert_eq!(label.geometry.width, Coord::content());
        assert_eq!(label.prop_text("text"), "Text");
        assert!(!label.flag(ObjFlag::Clickable));
        assert!(label.prop("unknown").is_none());
    }

    #[test]
    fn copying_the_look_keeps_identity_name_and_size() {
        let mut source = Widget::new(WidgetKind::Label, WidgetId(1), "Source");
        source.set_prop("text", PropValue::Text("Hello".into()));
        source.geometry.x = Coord::px(40);
        source.events.push(crate::event::EventHandler {
            trigger: crate::event::EventTrigger::Clicked,
            action: crate::event::Action::CallFunction { name: "go".into() },
        });
        let mut target = Widget::new(WidgetKind::Label, WidgetId(2), "Target");
        assert!(target.copy_look_from(&source));
        assert_eq!((target.id, target.name.as_str()), (WidgetId(2), "Target"));
        assert_eq!(target.prop_text("text"), "Hello");
        assert_eq!(target.events.len(), 1);
        assert_eq!(target.geometry.x, Coord::px(0), "the position stays");
        let mut button = Widget::new(WidgetKind::Button, WidgetId(3), "Button");
        assert!(!button.copy_look_from(&source), "other kinds are refused");
    }

    #[test]
    fn flags_store_only_overrides() {
        let mut button = Widget::new(WidgetKind::Button, WidgetId(1), "Button1");
        assert!(button.flag(ObjFlag::Clickable));
        button.set_flag(ObjFlag::Clickable, false);
        assert_eq!(button.flags.get(&ObjFlag::Clickable), Some(&false));
        button.set_flag(ObjFlag::Clickable, true);
        assert!(button.flags.is_empty());
    }

    #[test]
    fn props_store_only_changes() {
        let mut slider = Widget::new(WidgetKind::Slider, WidgetId(1), "Slider1");
        slider.set_prop("value", PropValue::Int(42));
        assert_eq!(slider.prop_int("value"), 42);
        slider.set_prop("value", PropValue::Int(0));
        assert!(slider.props.is_empty());
    }

    #[test]
    fn tree_navigation() {
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "Screen1");
        let mut panel = Widget::new(WidgetKind::Panel, WidgetId(2), "Panel1");
        panel.children.push(Widget::new(WidgetKind::Label, WidgetId(3), "Label1"));
        screen.children.push(panel);
        let path: Vec<_> = screen.path_to(WidgetId(3)).unwrap().iter().map(|w| w.id.0).collect();
        assert_eq!(path, vec![1, 2, 3]);
        assert_eq!(screen.descendant_count(), 2);
        assert!(screen.find_mut(WidgetId(3)).is_some());
    }
}
