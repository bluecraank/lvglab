//! Conversion between project screens and the model's [`Design`] format.

use std::collections::HashMap;

use lvglab_model::catalog::WidgetDef;
use lvglab_model::coord::{Align, Coord};
use lvglab_model::event::{Action, EventHandler, EventTrigger, ScreenAnim};
use lvglab_model::flags::{FlexFlow, ObjFlag, ObjState, StateMask};
use lvglab_model::prop::{PropKind, PropValue, find_option};
use lvglab_model::style::{StyleKind, StyleProp, StyleValue, builtin_font_size};
use lvglab_model::widget::FlexLayout;
use lvglab_model::{Color, Project, Widget, WidgetId, WidgetKind};

use crate::spec::{Design, EventSpec, PropSpec, ScreenSpec, StyleSpec, WidgetSpec, kind_key};

// -- export ---------------------------------------------------------------

/// Describes a screen in the model's format (used as context for edits).
pub fn export_screen(project: &Project, screen: &Widget) -> ScreenSpec {
    let mut widgets = Vec::new();
    for child in &screen.children {
        export_widget(project, child, "", &mut widgets);
    }
    let background = screen
        .styles
        .get("main", StateMask::DEFAULT, StyleProp::BgColor)
        .and_then(StyleValue::as_color)
        .map(Color::to_hex_string)
        .unwrap_or_default();
    ScreenSpec { name: screen.name.clone(), background, widgets }
}

fn export_widget(project: &Project, widget: &Widget, parent: &str, out: &mut Vec<WidgetSpec>) {
    let def = widget.def();
    let g = widget.geometry;
    let props = def
        .props
        .iter()
        .filter_map(|p| {
            widget.props.get(p.key).map(|v| PropSpec { key: p.key.to_owned(), value: format_prop(project, v) })
        })
        .collect();
    let styles = widget
        .styles
        .blocks()
        .flat_map(|b| {
            b.props.iter().map(|(prop, value)| StyleSpec {
                part: b.slot.clone(),
                state: b.state.to_string(),
                property: prop.name().to_owned(),
                value: format_style(value),
            })
        })
        .collect();
    let events = widget.events.iter().filter_map(|e| export_event(project, e)).collect();
    out.push(WidgetSpec {
        name: widget.name.clone(),
        kind: kind_key(widget.kind),
        parent: parent.to_owned(),
        x: g.x.to_string(),
        y: g.y.to_string(),
        width: g.width.to_string(),
        height: g.height.to_string(),
        align: g.align.suffix().to_owned(),
        props,
        styles,
        states: widget.states.iter().map(|s| s.suffix().to_owned()).collect(),
        hidden: widget.is_hidden(),
        flex: widget.layout.map(|l| l.flow.suffix().to_owned()).unwrap_or_default(),
        events,
    });
    for child in &widget.children {
        export_widget(project, child, &widget.name, out);
    }
}

fn export_event(project: &Project, event: &EventHandler) -> Option<EventSpec> {
    let (action, target) = match &event.action {
        Action::ChangeScreen { screen, .. } => ("change_screen", project.widget((*screen)?)?.name.clone()),
        Action::CallFunction { name } => ("call_function", name.clone()),
        _ => return None,
    };
    Some(EventSpec { trigger: event.trigger.suffix().to_owned(), action: action.to_owned(), target })
}

fn format_prop(project: &Project, value: &PropValue) -> String {
    match value {
        PropValue::Bool(b) => b.to_string(),
        PropValue::Int(i) => i.to_string(),
        PropValue::Text(t) | PropValue::Enum(t) | PropValue::Image(t) => t.clone(),
        PropValue::Color(c) => c.to_hex_string(),
        PropValue::Pair(a, b) => format!("{a},{b}"),
        PropValue::WidgetRef(id) => id.and_then(|id| project.widget(id)).map(|w| w.name.clone()).unwrap_or_default(),
        PropValue::Date { year, month, day } => format!("{year:04}-{month:02}-{day:02}"),
    }
}

fn format_style(value: &StyleValue) -> String {
    match value {
        StyleValue::Int(i) => i.to_string(),
        StyleValue::Coord(c) => c.to_string(),
        StyleValue::Color(c) => c.to_hex_string(),
        StyleValue::Bool(b) => b.to_string(),
        StyleValue::Enum(s) | StyleValue::Font(s) | StyleValue::Image(s) => s.clone(),
    }
}

// -- apply ----------------------------------------------------------------

/// What applying a design changed.
#[derive(Debug, Default)]
pub struct Applied {
    /// Screens that were created or replaced.
    pub screens: Vec<WidgetId>,
    pub warnings: Vec<String>,
}

/// Applies a design: screens with the name of an existing screen are
/// rebuilt in place (keeping their id), others are added.
pub fn apply(project: &mut Project, design: &Design) -> Applied {
    let mut applied = Applied::default();
    // Screen names first, so change-screen events can reference new screens.
    for spec in &design.screens {
        if find_screen(project, &spec.name).is_none() {
            let id = project.add_screen();
            let name = if spec.name.trim().is_empty() { project.unique_name("Screen") } else { spec.name.clone() };
            project.widget_mut(id).expect("just added").name = name;
        }
    }
    for spec in &design.screens {
        let Some(screen_id) = find_screen(project, &spec.name).or_else(|| project.screens.last().map(|s| s.id)) else {
            continue;
        };
        let mut builder = Builder { project, warnings: &mut applied.warnings, ids: HashMap::new(), refs: Vec::new() };
        let screen = builder.build_screen(screen_id, spec);
        let refs = std::mem::take(&mut builder.refs);
        let ids = std::mem::take(&mut builder.ids);
        if let Some(target) = project.widget_mut(screen_id) {
            *target = screen;
        }
        resolve_refs(project, &ids, refs, &mut applied.warnings);
        applied.screens.push(screen_id);
    }
    project.repair_ids();
    applied
}

fn find_screen(project: &Project, name: &str) -> Option<WidgetId> {
    project.screens.iter().find(|s| s.name.eq_ignore_ascii_case(name.trim())).map(|s| s.id)
}

struct PendingRef {
    widget: WidgetId,
    key: &'static str,
    target: String,
}

struct Builder<'a> {
    project: &'a mut Project,
    warnings: &'a mut Vec<String>,
    /// Widget name → id within the screen being built.
    ids: HashMap<String, WidgetId>,
    refs: Vec<PendingRef>,
}

impl Builder<'_> {
    fn warn(&mut self, message: String) {
        self.warnings.push(message);
    }

    fn build_screen(&mut self, id: WidgetId, spec: &ScreenSpec) -> Widget {
        let name = self.project.widget(id).map(|w| w.name.clone()).unwrap_or_else(|| spec.name.clone());
        let mut screen = Widget::new(WidgetKind::Screen, id, name);
        if let Some(color) = Color::parse(&spec.background) {
            screen.styles.set("main", StateMask::DEFAULT, StyleProp::BgColor, StyleValue::Color(color));
            screen.styles.set("main", StateMask::DEFAULT, StyleProp::BgOpa, StyleValue::Int(255));
        }
        // Widgets reference parents by name; build them all, then nest.
        let mut built: Vec<(String, Widget)> = Vec::new();
        for widget_spec in &spec.widgets {
            if let Some(widget) = self.build_widget(widget_spec) {
                built.push((widget_spec.parent.trim().to_owned(), widget));
            }
        }
        let names: Vec<String> = built.iter().map(|(_, w)| w.name.clone()).collect();
        // Attach children before their parents are attached (reverse order).
        while let Some((parent, widget)) = built.pop() {
            let target = if parent.is_empty() { None } else { built.iter_mut().find(|(_, w)| w.name == parent) };
            match target {
                Some((_, parent_widget)) if parent_widget.def().accepts_child(widget.kind) => {
                    parent_widget.children.insert(0, widget);
                }
                found => {
                    if found.is_some() || (!parent.is_empty() && !names.contains(&parent)) {
                        self.warn(format!("{}: parent `{parent}` not usable, placed on the screen", widget.name));
                    }
                    if screen.def().accepts_child(widget.kind) {
                        screen.children.insert(0, widget);
                    } else {
                        self.warn(format!(
                            "{}: a {} needs a matching parent and was skipped",
                            widget.name,
                            widget.kind.name()
                        ));
                    }
                }
            }
        }
        screen
    }

    fn build_widget(&mut self, spec: &WidgetSpec) -> Option<Widget> {
        let Some(kind) = parse_kind(&spec.kind) else {
            self.warn(format!("{}: unknown widget type `{}`", spec.name, spec.kind));
            return None;
        };
        let mut widget = self.project.new_widget(kind);
        widget.children.clear(); // A new button comes with a label; the design states its own.
        if !spec.name.trim().is_empty() {
            let name: String =
                spec.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
            widget.name = if self.ids.contains_key(&name) { self.project.unique_name(&name) } else { name };
        }
        self.ids.insert(widget.name.clone(), widget.id);
        let def = kind.def();
        let g = &mut widget.geometry;
        for (text, target) in
            [(&spec.x, &mut g.x), (&spec.y, &mut g.y), (&spec.width, &mut g.width), (&spec.height, &mut g.height)]
        {
            if let Ok(coord) = text.parse::<Coord>() {
                *target = coord;
            }
        }
        if let Some(align) = Align::from_suffix(&spec.align) {
            g.align = align;
        }
        for prop in &spec.props {
            self.apply_prop(&mut widget, def, prop);
        }
        for style in &spec.styles {
            self.apply_style(&mut widget, def, style);
        }
        for state in &spec.states {
            match ObjState::from_suffix(state) {
                Some(state) => {
                    widget.states.insert(state);
                }
                None => self.warn(format!("{}: unknown state `{state}`", widget.name)),
            }
        }
        if spec.hidden {
            widget.set_flag(ObjFlag::Hidden, true);
        }
        if let Some(flow) = FlexFlow::from_suffix(&spec.flex) {
            widget.layout = Some(FlexLayout { flow, ..FlexLayout::default() });
        }
        for event in &spec.events {
            self.apply_event(&mut widget, event);
        }
        Some(widget)
    }

    fn apply_prop(&mut self, widget: &mut Widget, def: &WidgetDef, spec: &PropSpec) {
        let Some(prop) = def.prop(spec.key.trim()) else {
            self.warn(format!("{}: `{}` is not a property of {}", widget.name, spec.key, def.name));
            return;
        };
        let text = spec.value.trim();
        let value = match prop.kind {
            PropKind::Bool => text.parse().ok().map(PropValue::Bool),
            PropKind::Int { min, max } => text.parse::<i32>().ok().map(|v| PropValue::Int(v.clamp(min, max))),
            PropKind::Enum(options) => find_option(options, text).map(|o| PropValue::Enum(o.c.to_owned())),
            PropKind::Color => Color::parse(text).map(PropValue::Color),
            PropKind::Pair { .. } => {
                let mut parts = text.split(',').map(|p| p.trim().parse::<i32>());
                match (parts.next(), parts.next()) {
                    (Some(Ok(a)), Some(Ok(b))) => Some(PropValue::Pair(a, b)),
                    _ => None,
                }
            }
            PropKind::Image => {
                if !text.is_empty() && self.project.assets.image(text).is_none() {
                    self.warn(format!("{}: image `{text}` is not an asset of the project", widget.name));
                }
                Some(PropValue::Image(text.to_owned()))
            }
            PropKind::WidgetRef => {
                self.refs.push(PendingRef { widget: widget.id, key: prop.key, target: text.to_owned() });
                return;
            }
            PropKind::Date => {
                let parts: Vec<i32> = text.split('-').filter_map(|p| p.parse().ok()).collect();
                (parts.len() == 3).then(|| PropValue::Date {
                    year: parts[0],
                    month: parts[1] as u8,
                    day: parts[2] as u8,
                })
            }
            PropKind::Text
            | PropKind::Lines
            | PropKind::Points
            | PropKind::StrArray { .. }
            | PropKind::ImageArray
            | PropKind::ChartSeries => Some(PropValue::Text(spec.value.replace("\\n", "\n"))),
        };
        match value {
            Some(value) => widget.set_prop(prop.key, value),
            None => self.warn(format!("{}: invalid value `{text}` for {}", widget.name, prop.key)),
        }
    }

    fn apply_style(&mut self, widget: &mut Widget, def: &WidgetDef, spec: &StyleSpec) {
        let slot = if spec.part.trim().is_empty() { "main" } else { spec.part.trim() };
        if def.slot(slot).is_none() {
            self.warn(format!("{}: {} has no style part `{slot}`", widget.name, def.name));
            return;
        }
        let Some(state) = StateMask::parse(if spec.state.trim().is_empty() { "DEFAULT" } else { &spec.state }) else {
            self.warn(format!("{}: unknown state `{}`", widget.name, spec.state));
            return;
        };
        let Some(prop) = StyleProp::from_name(&spec.property) else {
            self.warn(format!("{}: unknown style property `{}`", widget.name, spec.property));
            return;
        };
        let text = spec.value.trim();
        let value = match prop.kind() {
            StyleKind::Coord => text.parse::<Coord>().ok().map(StyleValue::Coord),
            StyleKind::Int { min, max } => {
                let circle = prop == StyleProp::Radius && text.eq_ignore_ascii_case("circle");
                let parsed = if circle { Some(lvglab_model::theme::RADIUS_CIRCLE) } else { text.parse::<i32>().ok() };
                parsed.map(|v| StyleValue::Int(v.clamp(min, max)))
            }
            StyleKind::Opa => parse_opa(text).map(StyleValue::Int),
            StyleKind::Color => Color::parse(text).map(StyleValue::Color),
            StyleKind::Bool => text.parse().ok().map(StyleValue::Bool),
            StyleKind::Enum(options) => find_option(options, text).map(|o| StyleValue::Enum(o.c.to_owned())),
            StyleKind::Font => {
                if builtin_font_size(text).is_none() && self.project.assets.font(text).is_none() {
                    self.warn(format!("{}: font `{text}` does not exist, using montserrat_14", widget.name));
                    Some(StyleValue::Font("montserrat_14".to_owned()))
                } else {
                    Some(StyleValue::Font(text.to_owned()))
                }
            }
            StyleKind::Image => Some(StyleValue::Image(text.to_owned())),
        };
        match value {
            Some(value) => widget.styles.set(slot, state, prop, value),
            None => self.warn(format!("{}: invalid value `{text}` for style {}", widget.name, prop.name())),
        }
    }

    fn apply_event(&mut self, widget: &mut Widget, spec: &EventSpec) {
        let Some(trigger) = EventTrigger::from_suffix(&spec.trigger) else {
            self.warn(format!("{}: unknown event trigger `{}`", widget.name, spec.trigger));
            return;
        };
        let action = match spec.action.as_str() {
            "change_screen" => match find_screen(self.project, &spec.target) {
                Some(screen) => {
                    Action::ChangeScreen { screen: Some(screen), anim: ScreenAnim::FadeOn, time_ms: 300, delay_ms: 0 }
                }
                None => {
                    self.warn(format!("{}: screen `{}` does not exist", widget.name, spec.target));
                    return;
                }
            },
            "call_function" => {
                let name: String = spec.target.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                Action::CallFunction { name }
            }
            other => {
                self.warn(format!("{}: unknown action `{other}`", widget.name));
                return;
            }
        };
        widget.events.push(EventHandler { trigger, action });
    }
}

fn resolve_refs(
    project: &mut Project,
    ids: &HashMap<String, WidgetId>,
    refs: Vec<PendingRef>,
    warnings: &mut Vec<String>,
) {
    for pending in refs {
        if pending.target.is_empty() {
            continue;
        }
        let target = ids.get(&pending.target).copied();
        if target.is_none() {
            warnings.push(format!("reference to unknown widget `{}`", pending.target));
        }
        if let Some(widget) = project.widget_mut(pending.widget) {
            widget.set_prop(pending.key, PropValue::WidgetRef(target));
        }
    }
}

fn parse_kind(text: &str) -> Option<WidgetKind> {
    let key = text.trim().to_ascii_lowercase();
    serde_json::from_value::<WidgetKind>(serde_json::Value::String(key.clone()))
        .ok()
        .or_else(|| lvglab_model::catalog::by_squareline_name(&key))
        .filter(|k| *k != WidgetKind::Screen)
}

/// Opacity as `0`–`255` or percentage (`50%`).
fn parse_opa(text: &str) -> Option<i32> {
    match text.strip_suffix('%') {
        Some(percent) => percent.trim().parse::<f32>().ok().map(|p| (p.clamp(0.0, 100.0) * 2.55).round() as i32),
        None => text.parse::<i32>().ok().map(|v| v.clamp(0, 255)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget(name: &str, kind: &str, parent: &str) -> WidgetSpec {
        WidgetSpec {
            name: name.into(),
            kind: kind.into(),
            parent: parent.into(),
            x: "0".into(),
            y: "0".into(),
            width: "content".into(),
            height: "content".into(),
            align: "CENTER".into(),
            ..Default::default()
        }
    }

    fn design() -> Design {
        let mut button = widget("OkButton", "button", "");
        button.width = "120".into();
        button.styles.push(StyleSpec {
            part: "main".into(),
            state: "PRESSED".into(),
            property: "bg_color".into(),
            value: "#FF5500".into(),
        });
        button.styles.push(StyleSpec {
            part: "main".into(),
            state: "".into(),
            property: "radius".into(),
            value: "circle".into(),
        });
        button.events.push(EventSpec {
            trigger: "CLICKED".into(),
            action: "change_screen".into(),
            target: "Details".into(),
        });
        let mut label = widget("OkLabel", "label", "OkButton");
        label.props.push(PropSpec { key: "text".into(), value: "OK".into() });
        let mut slider = widget("Volume", "slider", "");
        slider.props.push(PropSpec { key: "value".into(), value: "40".into() });
        slider.props.push(PropSpec { key: "range".into(), value: "0, 100".into() });
        Design {
            summary: "test".into(),
            screens: vec![
                ScreenSpec { name: "Main".into(), background: "#101010".into(), widgets: vec![button, label, slider] },
                ScreenSpec { name: "Details".into(), background: String::new(), widgets: vec![] },
            ],
        }
    }

    #[test]
    fn applies_widgets_styles_and_events() {
        let mut project = Project::new("P");
        let applied = apply(&mut project, &design());
        assert!(applied.warnings.is_empty(), "{:?}", applied.warnings);
        assert_eq!(project.screens.len(), 3);
        let main = project.screens.iter().find(|s| s.name == "Main").unwrap();
        let button = &main.children[0];
        assert_eq!(button.kind, WidgetKind::Button);
        assert_eq!(button.geometry.width, Coord::px(120));
        assert_eq!(button.children[0].prop_text("text"), "OK");
        let pressed = StateMask::DEFAULT.with(ObjState::Pressed);
        assert_eq!(
            button.styles.get("main", pressed, StyleProp::BgColor),
            Some(&StyleValue::Color(Color::from_hex(0xFF5500)))
        );
        assert!(matches!(button.events[0].action, Action::ChangeScreen { screen: Some(_), .. }));
        assert_eq!(main.children[1].prop_int("value"), 40);
    }

    #[test]
    fn replaces_existing_screen_in_place() {
        let mut project = Project::new("P");
        let original = project.screens[0].id;
        let name = project.screens[0].name.clone();
        let mut design = design();
        design.screens.truncate(1);
        design.screens[0].name = name;
        design.screens[0].widgets[0].events.clear();
        apply(&mut project, &design);
        assert_eq!(project.screens.len(), 1);
        assert_eq!(project.screens[0].id, original);
        assert_eq!(project.screens[0].children.len(), 2);
    }

    #[test]
    fn reports_invalid_content() {
        let mut project = Project::new("P");
        let mut design = design();
        design.screens[0].widgets.push(widget("Ghost", "hologram", ""));
        design.screens[0].widgets[2].props.push(PropSpec { key: "colour".into(), value: "red".into() });
        design.screens[0].widgets.push(widget("Tab", "tab_page", ""));
        let applied = apply(&mut project, &design);
        assert_eq!(applied.warnings.len(), 3, "{:?}", applied.warnings);
    }

    #[test]
    fn export_and_apply_round_trip() {
        let mut project = Project::new("P");
        apply(&mut project, &design());
        let main = project.screens.iter().find(|s| s.name == "Main").unwrap().clone();
        let spec = export_screen(&project, &main);
        assert_eq!(spec.widgets.len(), 3);
        assert_eq!(spec.widgets[1].parent, "OkButton");
        let mut copy = project.clone();
        apply(&mut copy, &Design { summary: String::new(), screens: vec![spec] });
        let rebuilt = copy.screens.iter().find(|s| s.name == "Main").unwrap();
        assert_eq!(rebuilt.children.len(), main.children.len());
        assert_eq!(rebuilt.children[0].styles, main.children[0].styles);
        assert_eq!(rebuilt.children[0].events.len(), 1);
    }

    #[test]
    fn opacity_accepts_percent() {
        assert_eq!(parse_opa("50%"), Some(128));
        assert_eq!(parse_opa("300"), Some(255));
    }
}
