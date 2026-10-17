//! The UI description exchanged with the model.
//!
//! Structured outputs do not allow recursive schemas or free-form maps, so a
//! screen is a flat list of widgets that reference their parent by name, and
//! properties and styles are lists of string key/value pairs. Values use the
//! same notation as the editor (`50%`, `content`, `#RRGGBB`, `LV_...`).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use lvglab_model::catalog;
use lvglab_model::coord::Align;
use lvglab_model::event::{EventTrigger, ScreenAnim};
use lvglab_model::flags::ObjState;
use lvglab_model::style::StyleProp;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Design {
    /// One or two sentences describing what was built or changed.
    pub summary: String,
    pub screens: Vec<ScreenSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenSpec {
    pub name: String,
    /// Main background color (`#RRGGBB`), empty for the theme default.
    pub background: String,
    pub widgets: Vec<WidgetSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WidgetSpec {
    pub name: String,
    /// Widget type key, e.g. `button`, `label`, `slider`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Name of the parent widget; empty for the screen.
    pub parent: String,
    pub x: String,
    pub y: String,
    pub width: String,
    pub height: String,
    /// Alignment suffix, e.g. `CENTER`, `TOP_MID`.
    pub align: String,
    pub props: Vec<PropSpec>,
    pub styles: Vec<StyleSpec>,
    /// Initial states, e.g. `CHECKED`, `DISABLED`.
    pub states: Vec<String>,
    pub hidden: bool,
    /// Flex layout flow (`ROW`, `COLUMN`, `ROW_WRAP`, ...); empty for none.
    pub flex: String,
    pub events: Vec<EventSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PropSpec {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleSpec {
    /// Style part key of the widget, e.g. `main`, `indicator`, `knob`.
    pub part: String,
    /// `DEFAULT`, `PRESSED`, `CHECKED`, ... (combine with `|`).
    pub state: String,
    /// LVGL style property name, e.g. `bg_color`, `radius`.
    pub property: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EventSpec {
    /// Trigger suffix, e.g. `CLICKED`, `VALUE_CHANGED`.
    pub trigger: String,
    /// `change_screen` or `call_function`.
    pub action: String,
    /// Screen name for `change_screen`, C function name for `call_function`.
    pub target: String,
}

fn string() -> Value {
    json!({ "type": "string" })
}

fn object(properties: Value) -> Value {
    let required: Vec<String> = properties.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

fn string_enum(values: impl IntoIterator<Item = String>) -> Value {
    json!({ "type": "string", "enum": values.into_iter().collect::<Vec<_>>() })
}

/// Serde name of a widget kind (`button`, `img_button`, ...).
pub fn kind_key(kind: lvglab_model::WidgetKind) -> String {
    serde_json::to_value(kind).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

/// JSON schema of [`Design`] for `output_config.format`.
pub fn schema() -> Value {
    let kinds = catalog::ALL.iter().filter(|d| d.kind != lvglab_model::WidgetKind::Screen).map(|d| kind_key(d.kind));
    let aligns = Align::ALL.iter().map(|a| a.suffix().to_owned());
    let triggers = EventTrigger::ALL.iter().map(|t| t.suffix().to_owned());
    let widget = object(json!({
        "name": string(),
        "type": string_enum(kinds),
        "parent": string(),
        "x": string(),
        "y": string(),
        "width": string(),
        "height": string(),
        "align": string_enum(aligns),
        "props": { "type": "array", "items": object(json!({ "key": string(), "value": string() })) },
        "styles": { "type": "array", "items": object(json!({
            "part": string(), "state": string(), "property": string(), "value": string()
        })) },
        "states": { "type": "array", "items": string_enum(ObjState::ALL.iter().map(|s| s.suffix().to_owned())) },
        "hidden": { "type": "boolean" },
        "flex": string(),
        "events": { "type": "array", "items": object(json!({
            "trigger": string_enum(triggers),
            "action": string_enum(["change_screen".to_owned(), "call_function".to_owned()]),
            "target": string()
        })) }
    }));
    object(json!({
        "summary": string(),
        "screens": { "type": "array", "items": object(json!({
            "name": string(),
            "background": string(),
            "widgets": { "type": "array", "items": widget }
        })) }
    }))
}

/// Names accepted for style properties, for the prompt.
pub fn style_names() -> Vec<&'static str> {
    StyleProp::ALL.iter().map(|p| p.name()).collect()
}

/// Names accepted for screen animations, for the prompt.
pub fn animation_names() -> Vec<&'static str> {
    ScreenAnim::ALL.iter().map(|a| a.suffix()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Structured outputs require `additionalProperties: false` and every
    /// property listed in `required` on every object, and no recursion.
    fn check(value: &Value, path: &str) {
        if let Some(obj) = value.as_object() {
            if obj.get("type") == Some(&json!("object")) {
                assert_eq!(obj.get("additionalProperties"), Some(&json!(false)), "{path}");
                let props: Vec<_> = obj["properties"].as_object().unwrap().keys().collect();
                let required: Vec<_> =
                    obj["required"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
                assert_eq!(props.len(), required.len(), "{path}");
            }
            assert!(!obj.contains_key("$ref"), "no recursion at {path}");
            for (key, child) in obj {
                check(child, &format!("{path}.{key}"));
            }
        } else if let Some(items) = value.as_array() {
            for item in items {
                check(item, path);
            }
        }
    }

    #[test]
    fn schema_follows_structured_output_rules() {
        let schema = schema();
        check(&schema, "$");
        let kinds =
            &schema["properties"]["screens"]["items"]["properties"]["widgets"]["items"]["properties"]["type"]["enum"];
        assert!(kinds.as_array().unwrap().iter().any(|k| k == "button"));
        assert!(!kinds.as_array().unwrap().iter().any(|k| k == "screen"));
    }

    #[test]
    fn design_round_trips_through_json() {
        let design = Design {
            summary: "s".into(),
            screens: vec![ScreenSpec {
                name: "Main".into(),
                widgets: vec![WidgetSpec::default()],
                ..Default::default()
            }],
        };
        let text = serde_json::to_string(&design).unwrap();
        assert_eq!(serde_json::from_str::<Design>(&text).unwrap(), design);
    }
}
