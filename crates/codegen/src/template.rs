//! Formatting of property values as C and expansion of catalog templates.

use std::collections::HashMap;

use lvglab_model::layout::parse_points;
use lvglab_model::prop::{PropDef, PropKind, PropValue};
use lvglab_model::style::StyleValue;

use crate::naming::Naming;

/// Escapes text as a C string literal. Non-ASCII characters stay UTF-8.
pub fn c_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\{:03o}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A formatted value: the main expression, named fields (`.0`, `.len`, ...)
/// and declarations that must precede its use.
#[derive(Debug, Default, Clone)]
pub struct Formatted {
    pub expr: String,
    pub fields: HashMap<&'static str, String>,
    pub prelude: Vec<String>,
}

impl Formatted {
    fn expr(expr: impl Into<String>) -> Self {
        Self { expr: expr.into(), ..Self::default() }
    }

    fn field(mut self, name: &'static str, value: impl ToString) -> Self {
        self.fields.insert(name, value.to_string());
        self
    }
}

/// Formats a widget property. `array_name` names static arrays.
pub fn format_prop(def: &PropDef, value: &PropValue, array_name: &str, names: &Naming) -> Formatted {
    match (def.kind, value) {
        (_, PropValue::Bool(v)) => Formatted::expr(v.to_string()),
        (_, PropValue::Int(v)) => Formatted::expr(v.to_string()),
        (_, PropValue::Enum(v)) => Formatted::expr(v.clone()),
        (_, PropValue::Color(c)) => Formatted::expr(c.c_expr()),
        (_, PropValue::Pair(a, b)) => Formatted::expr(format!("{a}, {b}")).field("0", a).field("1", b),
        (_, PropValue::Image(name)) => Formatted::expr(names.image_ref(name)),
        (_, PropValue::WidgetRef(id)) => Formatted::expr(id.map_or("NULL", |id| names.var(id))),
        (_, PropValue::Date { year, month, day }) => Formatted::expr(format!("{year}, {month}, {day}"))
            .field("year", year)
            .field("month", month)
            .field("day", day),
        (PropKind::Points, PropValue::Text(text)) => {
            let points = parse_points(text);
            let list: Vec<String> = points.iter().map(|(x, y)| format!("{{{x}, {y}}}")).collect();
            static_array(
                format!("static lv_point_t {array_name}[] = {{{}}};", list.join(", ")),
                array_name,
                points.len(),
            )
        }
        (PropKind::StrArray { rows }, PropValue::Text(text)) => {
            let entries = str_array_entries(text, rows);
            if entries.is_empty() {
                return Formatted::expr("NULL").field("len", 0);
            }
            let mut items: Vec<String> = entries.iter().map(|e| c_string(e)).collect();
            items.push("\"\"".to_owned());
            let len = entries.len();
            static_array(format!("static const char * {array_name}[] = {{{}}};", items.join(", ")), array_name, len)
        }
        (PropKind::ImageArray, PropValue::Text(text)) => {
            let refs: Vec<String> = lines(text).map(|l| names.image_ref(l)).collect();
            let len = refs.len();
            static_array(
                format!("static const lv_img_dsc_t * {array_name}[] = {{{}}};", refs.join(", ")),
                array_name,
                len,
            )
        }
        (_, PropValue::Text(text)) => Formatted::expr(c_string(&text.replace("\r\n", "\n"))),
    }
}

fn static_array(declaration: String, name: &str, len: usize) -> Formatted {
    Formatted { expr: name.to_owned(), prelude: vec![declaration], ..Formatted::default() }.field("len", len)
}

/// Entries of a string array; rows are separated by `"\n"` entries.
pub fn str_array_entries(text: &str, rows: bool) -> Vec<String> {
    let mut entries = Vec::new();
    for (i, line) in lines(text).enumerate() {
        if rows && i > 0 {
            entries.push("\n".to_owned());
        }
        entries.extend(line.split('|').map(|e| e.trim().to_owned()));
    }
    entries
}

/// Non-empty, trimmed lines.
pub fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty())
}

/// Formats a style value. Theme variables are not plain values (they are
/// bound with `<prefix>_object_set_themeable_style_property`) and yield `None`.
pub fn format_style(value: &StyleValue, names: &Naming) -> Option<String> {
    Some(match value {
        StyleValue::Int(v) => v.to_string(),
        StyleValue::Coord(c) => c.c_expr(),
        StyleValue::Color(c) => c.c_expr(),
        StyleValue::Bool(b) => b.to_string(),
        StyleValue::Enum(e) => e.clone(),
        StyleValue::Font(f) => names.font_ref(f),
        StyleValue::Image(i) => names.image_ref(i),
        StyleValue::Theme(_) => return None,
    })
}

/// Values available to a template.
#[derive(Default)]
pub struct Scope<'a> {
    pub vars: HashMap<&'a str, String>,
    pub props: HashMap<&'a str, Formatted>,
}

impl<'a> Scope<'a> {
    pub fn var(mut self, name: &'a str, value: impl Into<String>) -> Self {
        self.vars.insert(name, value.into());
        self
    }

    fn lookup(&self, key: &str) -> Option<String> {
        if let Some(v) = self.vars.get(key) {
            return Some(v.clone());
        }
        let (name, field) = match key.split_once('.') {
            Some((name, field)) => (name, Some(field)),
            None => (key, None),
        };
        let formatted = if name == "v" { self.props.get("v") } else { self.props.get(name) }?;
        match field {
            None => Some(formatted.expr.clone()),
            Some(field) => formatted.fields.get(field).cloned(),
        }
    }

    /// Replaces `{name}` placeholders; unknown placeholders are kept and
    /// reported so mistakes are visible in the generated code.
    pub fn expand(&self, template: &str) -> String {
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            match after.find('}') {
                Some(end) if is_placeholder(&after[..end]) => {
                    let key = &after[..end];
                    match self.lookup(key) {
                        Some(value) => out.push_str(&value),
                        None => out.push_str(&format!("/* unknown {{{key}}} */")),
                    }
                    rest = &after[end + 1..];
                }
                _ => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }
}

fn is_placeholder(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::Project;
    use lvglab_model::prop::PropDefault;

    #[test]
    fn escapes_c_strings() {
        assert_eq!(c_string("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
        assert_eq!(c_string("m/s²"), "\"m/s²\"");
    }

    #[test]
    fn expands_templates() {
        let mut scope = Scope::default().var("obj", "ui_Arc1");
        scope.props.insert("v", Formatted::expr("1, 2").field("0", 1).field("1", 2));
        assert_eq!(scope.expand("lv_arc_set_range({obj}, {v.0}, {v.1});"), "lv_arc_set_range(ui_Arc1, 1, 2);");
        assert_eq!(scope.expand("for(;;) { x(); }"), "for(;;) { x(); }");
        assert!(scope.expand("{missing}").contains("unknown"));
    }

    #[test]
    fn formats_arrays() {
        let names = Naming::new(&Project::new("P"));
        let def = PropDef::new("map", "Map", PropKind::StrArray { rows: true }, PropDefault::Text(""));
        let f = format_prop(&def, &PropValue::Text("1|2\n3".into()), "ui_M_map", &names);
        assert_eq!(f.prelude[0], r#"static const char * ui_M_map[] = {"1", "2", "\n", "3", ""};"#);
        assert_eq!(f.fields["len"], "4");
        let def = PropDef::new("points", "Points", PropKind::Points, PropDefault::Text(""));
        let f = format_prop(&def, &PropValue::Text("0,0 5,6".into()), "ui_L_points", &names);
        assert_eq!(f.prelude[0], "static lv_point_t ui_L_points[] = {{0, 0}, {5, 6}};");
        assert_eq!(f.fields["len"], "2");
    }
}
