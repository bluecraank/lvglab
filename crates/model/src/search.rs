//! Search over everything a project holds: widget names and types,
//! property values, local styles (properties and values such as colors,
//! fonts, images and theme colors), events and flags.
//!
//! The query is a list of words; a hit contains all of them (ignoring case).
//! `color` finds the style property, `F15C00` finds every widget with that
//! color, `Button1` the widget, `ui_events` the calls of that function.

use crate::event::Action;
use crate::project::Project;
use crate::prop::PropValue;
use crate::style::StyleValue;
use crate::widget::{Widget, WidgetId, WidgetKind};

/// What matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitKind {
    /// The widget itself: its name or type.
    Widget,
    /// A property of the widget.
    Property,
    /// A local style of a part and state.
    Style,
    Event,
    Flag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The screen or component the widget belongs to.
    pub root: WidgetId,
    pub root_name: String,
    pub is_component: bool,
    pub widget: WidgetId,
    pub widget_name: String,
    pub kind: WidgetKind,
    pub what: HitKind,
    /// For example `bg_color = #F15C00 (main, pressed)`.
    pub detail: String,
}

/// How a style value reads in a search and in the results.
pub fn style_value_text(value: &StyleValue) -> String {
    match value {
        StyleValue::Int(v) => v.to_string(),
        StyleValue::Coord(c) => c.to_string(),
        StyleValue::Color(c) => c.to_hex_string(),
        StyleValue::Bool(b) => b.to_string(),
        StyleValue::Enum(e) => e.clone(),
        StyleValue::Font(f) => f.clone(),
        StyleValue::Image(i) => i.clone(),
        StyleValue::Theme(t) => format!("theme {t}"),
    }
}

fn prop_value_text(value: &PropValue) -> String {
    match value {
        PropValue::Bool(b) => b.to_string(),
        PropValue::Int(v) => v.to_string(),
        PropValue::Text(t) | PropValue::Enum(t) | PropValue::Image(t) => t.replace('\n', " ⏎ "),
        PropValue::Color(c) => c.to_hex_string(),
        PropValue::Pair(a, b) => format!("{a}, {b}"),
        PropValue::WidgetRef(id) => id.map_or_else(String::new, |id| format!("#{}", id.0)),
        PropValue::Date { year, month, day } => format!("{year}-{month:02}-{day:02}"),
    }
}

fn action_text(project: &Project, action: &Action) -> String {
    let target =
        |id: &Option<WidgetId>| id.and_then(|id| project.widget(id)).map_or_else(String::new, |w| w.name.clone());
    match action {
        Action::ChangeScreen { screen, .. } => format!("change screen {}", target(screen)),
        Action::CallFunction { name } => format!("call {name}"),
        Action::ModifyFlag { target: t, flag, .. } => format!("flag {} {}", flag.c_const(), target(t)),
        Action::ModifyState { target: t, state, .. } => format!("state {} {}", state.c_const(), target(t)),
        Action::SetText { target: t, text } => format!("set text {} {text}", target(t)),
        Action::SetValue { target: t, value } => format!("set value {} {value}", target(t)),
    }
}

fn matches(haystack: &str, words: &[String]) -> bool {
    let haystack = haystack.to_lowercase();
    words.iter().all(|w| haystack.contains(w.as_str()))
}

/// All hits for `query`, at most `limit`. Screens come first, then components.
pub fn search(project: &Project, query: &str, limit: usize) -> Vec<Hit> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let roots = project.screens.iter().map(|s| (s, false)).chain(project.components.iter().map(|c| (c, true)));
    for (root, is_component) in roots {
        root.walk(&mut |w: &Widget| {
            let mut add = |what: HitKind, detail: String| {
                if hits.len() < limit {
                    hits.push(Hit {
                        root: root.id,
                        root_name: root.name.clone(),
                        is_component,
                        widget: w.id,
                        widget_name: w.name.clone(),
                        kind: w.kind,
                        what,
                        detail,
                    });
                }
            };
            // The widget: name and type, together so "label title" works.
            let head = format!("{} {}", w.name, w.def().name);
            if matches(&head, &words) {
                add(HitKind::Widget, w.def().name.to_owned());
            }
            for (key, value) in &w.props {
                let text = prop_value_text(value);
                let label = w.def().prop(key).map_or(key.as_str(), |p| p.label);
                if matches(&format!("{} {key} {label} {text}", w.name), &words) && !matches(&head, &words) {
                    add(HitKind::Property, format!("{label} = {text}"));
                }
            }
            for block in w.styles.blocks() {
                for (prop, value) in &block.props {
                    let text = style_value_text(value);
                    let part = if block.state.0 == 0 {
                        block.slot.clone()
                    } else {
                        format!("{}, {}", block.slot, block.state)
                    };
                    let line = format!("{} = {text} ({part})", prop.name());
                    if matches(&format!("{} {line} {}", w.name, prop.label()), &words) && !matches(&head, &words) {
                        add(HitKind::Style, line);
                    }
                }
            }
            for handler in &w.events {
                let line = format!("{} → {}", handler.trigger.label(), action_text(project, &handler.action));
                if matches(&format!("{} {line}", w.name), &words) && !matches(&head, &words) {
                    add(HitKind::Event, line);
                }
            }
            for (flag, on) in &w.flags {
                let line = format!("{} {}", flag.c_const(), if *on { "on" } else { "off" });
                if matches(&format!("{} {line}", w.name), &words) && !matches(&head, &words) {
                    add(HitKind::Flag, line);
                }
            }
        });
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::event::{EventHandler, EventTrigger};
    use crate::flags::StateMask;
    use crate::style::StyleProp;

    fn project() -> Project {
        let mut p = Project::new("P");
        let screen = p.screens[0].id;
        p.widget_mut(screen).unwrap().name = "Main".into();
        let mut button = p.new_widget(WidgetKind::Button);
        button.name = "Go".into();
        button.styles.set("main", StateMask::DEFAULT, StyleProp::BgColor, StyleValue::Color(Color::from_hex(0xF15C00)));
        button.styles.set("main", StateMask::DEFAULT, StyleProp::TextFont, StyleValue::Font("FontBig".into()));
        button.events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::CallFunction { name: "on_go".into() },
        });
        p.insert(screen, 0, button).unwrap();
        let mut label = p.new_widget(WidgetKind::Label);
        label.name = "Title".into();
        label.set_prop("text", PropValue::Text("Hello world".into()));
        p.insert(screen, 1, label).unwrap();
        let component = p.add_component();
        let mut inner = p.new_widget(WidgetKind::Label);
        inner.name = "InComponent".into();
        inner.set_prop("text", PropValue::Text("Hello component".into()));
        let root = p.widget(component).unwrap().children[0].id;
        p.insert(root, 0, inner).unwrap();
        p
    }

    fn names(hits: &[Hit]) -> Vec<&str> {
        hits.iter().map(|h| h.widget_name.as_str()).collect()
    }

    #[test]
    fn finds_widgets_by_name_and_type() {
        let p = project();
        assert_eq!(names(&search(&p, "go", 50)), ["Go"]);
        assert!(names(&search(&p, "label", 50)).contains(&"Title"), "by type");
        assert!(search(&p, "", 50).is_empty());
        assert!(search(&p, "no such thing anywhere", 50).is_empty());
    }

    #[test]
    fn finds_property_values_style_values_and_events() {
        let p = project();
        let hits = search(&p, "hello", 50);
        assert_eq!(names(&hits), ["Title", "InComponent"]);
        assert!(hits[1].is_component && !hits[0].is_component);
        let hits = search(&p, "f15c00", 50);
        assert_eq!((names(&hits), &hits[0].what), (vec!["Go"], &HitKind::Style));
        assert!(hits[0].detail.contains("bg_color = #F15C00"), "{}", hits[0].detail);
        assert_eq!(names(&search(&p, "fontbig", 50)), ["Go"]);
        let hits = search(&p, "on_go", 50);
        assert_eq!((names(&hits), &hits[0].what), (vec!["Go"], &HitKind::Event));
    }

    #[test]
    fn all_words_must_match_and_the_limit_counts() {
        let p = project();
        assert_eq!(names(&search(&p, "hello world", 50)), ["Title"]);
        assert_eq!(search(&p, "e", 3).len(), 3);
    }
}
