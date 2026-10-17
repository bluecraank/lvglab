//! Widget specific properties (everything that is not a style or a common
//! object property), described by data so that the editor, the importer and
//! the code generator can all be driven by the widget catalog.

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::widget::WidgetId;

/// One choice of an enumerated property; `c` is the C constant (or snippet)
/// emitted for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnumOption {
    pub c: &'static str,
    pub label: &'static str,
}

/// Shorthand for building option tables.
pub const fn opt(c: &'static str, label: &'static str) -> EnumOption {
    EnumOption { c, label }
}

/// Finds the option for a value given either as full constant or as suffix
/// (`WRAP` matches `LV_LABEL_LONG_WRAP`), ignoring ASCII case.
pub fn find_option<'a>(options: &'a [EnumOption], text: &str) -> Option<&'a EnumOption> {
    let text = text.trim();
    options.iter().find(|o| o.c.eq_ignore_ascii_case(text)).or_else(|| {
        let suffix = format!("_{}", text.to_ascii_uppercase());
        options.iter().find(|o| o.c.to_ascii_uppercase().ends_with(&suffix))
    })
}

/// The editor/type information of a widget property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Bool,
    Int {
        min: i32,
        max: i32,
    },
    /// Single line text.
    Text,
    /// Multi line text, one entry per line (options of a dropdown, ...).
    Lines,
    Enum(&'static [EnumOption]),
    Color,
    Pair {
        min: i32,
        max: i32,
        first: &'static str,
        second: &'static str,
    },
    /// Name of an image asset.
    Image,
    /// Another widget of the project.
    WidgetRef,
    /// A calendar date.
    Date,
    /// Points of a line: `x,y x,y ...`.
    Points,
    /// A static string array; with `rows` lines become rows separated by
    /// `"\n"` entries and `|` separates entries within a row.
    StrArray {
        rows: bool,
    },
    /// A static array of image assets, one per line.
    ImageArray,
    /// Chart series, one per line: `#RRGGBB: v1, v2, ...`.
    ChartSeries,
}

/// A property value. The variant is determined by the [`PropKind`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropValue {
    Bool(bool),
    Int(i32),
    /// Used by `Text`, `Lines`, `Points`, `StrArray`, `ImageArray` and `ChartSeries`.
    Text(String),
    Enum(String),
    Color(Color),
    Pair(i32, i32),
    Image(String),
    WidgetRef(Option<WidgetId>),
    Date {
        year: i32,
        month: u8,
        day: u8,
    },
}

impl PropValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(v) | Self::Enum(v) | Self::Image(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_pair(&self) -> Option<(i32, i32)> {
        match self {
            Self::Pair(a, b) => Some((*a, *b)),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<Color> {
        match self {
            Self::Color(c) => Some(*c),
            _ => None,
        }
    }

    pub fn as_widget_ref(&self) -> Option<WidgetId> {
        match self {
            Self::WidgetRef(id) => *id,
            _ => None,
        }
    }

    /// True if the value has the variant expected for `kind`.
    pub fn fits(&self, kind: PropKind) -> bool {
        matches!(
            (self, kind),
            (Self::Bool(_), PropKind::Bool)
                | (Self::Int(_), PropKind::Int { .. })
                | (
                    Self::Text(_),
                    PropKind::Text
                        | PropKind::Lines
                        | PropKind::Points
                        | PropKind::StrArray { .. }
                        | PropKind::ImageArray
                        | PropKind::ChartSeries
                )
                | (Self::Enum(_), PropKind::Enum(_))
                | (Self::Color(_), PropKind::Color)
                | (Self::Pair(..), PropKind::Pair { .. })
                | (Self::Image(_), PropKind::Image)
                | (Self::WidgetRef(_), PropKind::WidgetRef)
                | (Self::Date { .. }, PropKind::Date)
        )
    }
}

/// Compile time default of a property, converted into a [`PropValue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropDefault {
    Bool(bool),
    Int(i32),
    Text(&'static str),
    Enum(&'static str),
    Color(u32),
    Pair(i32, i32),
    Image,
    WidgetRef,
    Date(i32, u8, u8),
}

impl PropDefault {
    pub fn value(self) -> PropValue {
        match self {
            Self::Bool(v) => PropValue::Bool(v),
            Self::Int(v) => PropValue::Int(v),
            Self::Text(v) => PropValue::Text(v.to_owned()),
            Self::Enum(v) => PropValue::Enum(v.to_owned()),
            Self::Color(v) => PropValue::Color(Color::from_hex(v)),
            Self::Pair(a, b) => PropValue::Pair(a, b),
            Self::Image => PropValue::Image(String::new()),
            Self::WidgetRef => PropValue::WidgetRef(None),
            Self::Date(year, month, day) => PropValue::Date { year, month, day },
        }
    }
}

/// How a property turns into C code. Templates use `{obj}` for the object,
/// `{v}` for the formatted value, `{v.0}`/`{v.1}` for pair components,
/// `{v.year}`/`{v.month}`/`{v.day}` for dates, `{v.len}` for array lengths and
/// `{other_key}` for other properties of the same widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emit {
    /// Only consumed by other templates (e.g. creation arguments).
    None,
    /// One statement.
    Call(&'static str),
    /// One statement per non-empty line; `{line}` is the C string literal of
    /// the line and `{i}` its index.
    EachLine(&'static str),
    /// One statement per `|` separated cell; `{row}`, `{col}` and `{cell}`.
    EachCell(&'static str),
    /// The selected [`EnumOption::c`] is itself the statement template.
    OptionTemplate,
    /// Chart series need a dedicated code path.
    ChartSeries,
}

/// When a property is emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitWhen {
    /// Only if the value differs from the LVGL default.
    Changed,
    /// Always (the LVGL default depends on the theme or similar).
    Always,
    /// Only if the value is not empty (text, image, reference).
    NotEmpty,
}

/// Static description of one widget property.
#[derive(Clone, Copy, Debug)]
pub struct PropDef {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: PropKind,
    pub default: PropDefault,
    pub emit: Emit,
    pub when: EmitWhen,
    /// Alternative names used by SquareLine Studio. A trailing `#0`/`#1`
    /// addresses a component of a pair property.
    pub squareline: &'static [&'static str],
    pub help: &'static str,
}

impl PropDef {
    pub const fn new(key: &'static str, label: &'static str, kind: PropKind, default: PropDefault) -> Self {
        Self { key, label, kind, default, emit: Emit::None, when: EmitWhen::Changed, squareline: &[], help: "" }
    }

    pub const fn call(mut self, template: &'static str) -> Self {
        self.emit = Emit::Call(template);
        self
    }

    pub const fn emit(mut self, emit: Emit) -> Self {
        self.emit = emit;
        self
    }

    pub const fn always(mut self) -> Self {
        self.when = EmitWhen::Always;
        self
    }

    pub const fn not_empty(mut self) -> Self {
        self.when = EmitWhen::NotEmpty;
        self
    }

    pub const fn sl(mut self, names: &'static [&'static str]) -> Self {
        self.squareline = names;
        self
    }

    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    pub fn default_value(&self) -> PropValue {
        self.default.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONS: &[EnumOption] =
        &[opt("LV_LABEL_LONG_WRAP", "Wrap"), opt("LV_LABEL_LONG_SCROLL_CIRCULAR", "Circular")];

    #[test]
    fn finds_options_by_suffix_and_constant() {
        assert_eq!(find_option(OPTIONS, "WRAP").unwrap().c, "LV_LABEL_LONG_WRAP");
        assert_eq!(find_option(OPTIONS, "scroll_circular").unwrap().label, "Circular");
        assert_eq!(find_option(OPTIONS, "LV_LABEL_LONG_WRAP").unwrap().label, "Wrap");
        assert!(find_option(OPTIONS, "DOT").is_none());
    }

    #[test]
    fn value_fits_kind() {
        assert!(PropValue::Text("a".into()).fits(PropKind::Lines));
        assert!(!PropValue::Int(1).fits(PropKind::Bool));
    }
}
