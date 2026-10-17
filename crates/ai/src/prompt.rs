//! System prompt (stable, cacheable) and per-request context.

use std::fmt::Write as _;

use lvglab_model::catalog::{self, WidgetDef};
use lvglab_model::prop::PropKind;
use lvglab_model::style::{MONTSERRAT_SIZES, StyleKind, StyleProp};
use lvglab_model::{Project, WidgetId, WidgetKind};

use crate::convert::export_screen;
use crate::spec::{animation_names, kind_key, style_names};

/// Instructions plus a reference of every widget, property and style.
/// Depends only on the catalog, so it is identical for every request and
/// can be cached.
pub fn system_prompt() -> String {
    let mut out = String::new();
    out.push_str(
        "You design user interfaces for LVGL 8.3 on small embedded displays (ESP32). \
You answer with a JSON document that the editor turns into LVGL widgets.\n\n\
How to design:\n\
- Fit everything inside the display resolution given in the request; on round displays keep content inside the circle (avoid the corners).\n\
- Prefer clear hierarchy, generous touch targets (at least 40 px high), consistent spacing (multiples of 4 or 8 px) and good contrast.\n\
- Use the theme colors unless the user asks for others; set colors with styles.\n\
- Use flex layouts (`flex` = ROW / COLUMN on a container or panel) for rows and lists of items; children of a flex container ignore x/y.\n\
- Give every widget a short, unique PascalCase name that describes it (e.g. `SaveButton`, `VolumeSlider`).\n\
- A button needs a child label for its text.\n\n\
The JSON document:\n\
- `summary`: one or two sentences in the user's language describing what you built or changed.\n\
- `screens`: every screen you create or change. A screen with the name of an existing screen replaces that screen completely, \
so include all widgets that should remain. New names create new screens. Do not list screens you leave unchanged.\n\
- `widgets` is a flat list. `parent` is the name of the parent widget or \"\" for the screen. List parents before their children.\n\
- `x`, `y`, `width`, `height`: pixels (`12`, `-20`), percent of the parent (`50%`) or `content` (size only).\n\
- `align`: position relative to the parent, then shifted by x/y (e.g. CENTER with y -40 is 40 px above the center).\n\
- `props`: widget properties as key/value strings (only keys listed for the widget type).\n\
- `styles`: `part` (listed per widget type), `state` (DEFAULT, PRESSED, CHECKED, FOCUSED, DISABLED, combine with |), \
`property` (a style property below) and `value`.\n\
- `events`: `change_screen` with the screen name as target, or `call_function` with a C function name.\n\n\
Value formats: colors `#RRGGBB`; opacity 0-255 or `50%`; booleans `true`/`false`; pairs `min,max`; dates `YYYY-MM-DD`; \
line breaks in texts as \\n; enumerations as listed. Radius `circle` makes pill shapes.\n",
    );
    let _ = writeln!(
        out,
        "\nFonts: built-in `montserrat_<size>` with sizes {}, or a project font listed in the request.",
        {
            let sizes: Vec<String> = MONTSERRAT_SIZES.iter().map(u16::to_string).collect();
            sizes.join(", ")
        }
    );
    let _ = writeln!(out, "Screen animations: {}", animation_names().join(", "));
    out.push_str("\nWidget types (key: description; parts; properties):\n");
    for def in catalog::ALL.iter().filter(|d| d.kind != WidgetKind::Screen) {
        widget_reference(&mut out, def);
    }
    out.push_str("\nStyle properties (name: value):\n");
    for prop in StyleProp::ALL {
        let _ = writeln!(out, "- {}: {}", prop.name(), style_kind(prop.kind()));
    }
    let _ = writeln!(out, "\nAll style property names: {}", style_names().join(", "));
    out
}

fn widget_reference(out: &mut String, def: &WidgetDef) {
    let parts: Vec<&str> = def.slots.iter().map(|s| s.key).collect();
    let mut line = format!("- {}: {} Parts: {}.", kind_key(def.kind), def.description, parts.join(", "));
    if let Some(parent) = def.required_parent {
        let _ = write!(line, " Must be a child of {}.", kind_key(parent));
    }
    if let Some(child) = def.required_child {
        let _ = write!(line, " Children must be {}.", kind_key(child));
    }
    if def.container {
        line.push_str(" Can contain children.");
    }
    let props: Vec<String> = def.props.iter().map(|p| format!("{} ({})", p.key, prop_kind(p.kind))).collect();
    if !props.is_empty() {
        let _ = write!(line, " Props: {}.", props.join("; "));
    }
    out.push_str(&line);
    out.push('\n');
}

fn prop_kind(kind: PropKind) -> String {
    match kind {
        PropKind::Bool => "bool".into(),
        PropKind::Int { min, max } => format!("int {min}..{max}"),
        PropKind::Text => "text".into(),
        PropKind::Lines => "text, one entry per line".into(),
        PropKind::Enum(options) => options.iter().map(|o| o.c).filter(|c| !c.is_empty()).collect::<Vec<_>>().join("|"),
        PropKind::Color => "color".into(),
        PropKind::Pair { first, second, .. } => format!("pair {first},{second}"),
        PropKind::Image => "image asset name".into(),
        PropKind::WidgetRef => "name of another widget".into(),
        PropKind::Date => "date".into(),
        PropKind::Points => "points `x,y x,y ...`".into(),
        PropKind::StrArray { rows: true } => "rows per line, entries separated by |".into(),
        PropKind::StrArray { rows: false } => "entries separated by |".into(),
        PropKind::ImageArray => "image asset names, one per line".into(),
        PropKind::ChartSeries => "series per line `#RRGGBB: v1, v2, ...`".into(),
    }
}

fn style_kind(kind: StyleKind) -> String {
    match kind {
        StyleKind::Coord => "px, % or content".into(),
        StyleKind::Int { min, max } => format!("int {min}..{max}"),
        StyleKind::Opa => "opacity".into(),
        StyleKind::Color => "color".into(),
        StyleKind::Bool => "bool".into(),
        StyleKind::Enum(options) => options.iter().map(|o| o.c).collect::<Vec<_>>().join("|"),
        StyleKind::Font => "font".into(),
        StyleKind::Image => "image asset name".into(),
    }
}

/// Where the result of a request goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Change the given screen (it is sent as context).
    EditScreen(WidgetId),
    /// Create one or more new screens.
    NewScreens,
}

/// The user message: project context plus the request.
pub fn user_message(project: &Project, scope: Scope, request: &str) -> String {
    let mut out = String::new();
    let d = &project.display;
    let shape = match d.shape {
        lvglab_model::project::DisplayShape::Round => "round (content must stay inside the circle)",
        lvglab_model::project::DisplayShape::Rectangle => "rectangular",
    };
    let _ = writeln!(out, "Display: {} x {} px, {shape}.", d.width, d.height);
    let theme = &project.theme;
    let _ = writeln!(
        out,
        "Theme: {} mode, primary {} ({}), secondary {} ({}).",
        if theme.dark { "dark" } else { "light" },
        theme.primary.label(),
        theme.primary.main().to_hex_string(),
        theme.secondary.label(),
        theme.secondary.main().to_hex_string()
    );
    let themes = &project.color_themes;
    if !themes.is_empty() {
        let variables: Vec<String> = themes
            .variables
            .iter()
            .map(|v| format!("{v} ({})", themes.value(v).map(|c| c.color.to_hex_string()).unwrap_or_default()))
            .collect();
        let _ = writeln!(
            out,
            "Theme colors: {}. Prefer them for colors: write `theme:<name>` as value of a color property and of its opa property, so the colors follow the active theme.",
            variables.join(", ")
        );
    }
    let screens: Vec<&str> = project.screens.iter().map(|s| s.name.as_str()).collect();
    let _ = writeln!(out, "Existing screens: {}.", screens.join(", "));
    if !project.assets.fonts.is_empty() {
        let fonts: Vec<String> = project.assets.fonts.iter().map(|f| format!("{} ({} px)", f.name, f.size)).collect();
        let _ = writeln!(out, "Project fonts: {}.", fonts.join(", "));
    }
    if !project.assets.images.is_empty() {
        let images: Vec<&str> = project.assets.images.iter().map(|i| i.name.as_str()).collect();
        let _ = writeln!(out, "Image assets: {}.", images.join(", "));
    }
    match scope {
        Scope::EditScreen(id) => {
            if let Some(screen) = project.widget(id) {
                let spec = export_screen(project, screen);
                let json = serde_json::to_string(&spec).unwrap_or_default();
                let _ = writeln!(
                    out,
                    "\nChange the screen `{}`. Its current content (same format as your answer):\n{json}",
                    screen.name
                );
                out.push_str(
                    "Return the complete changed screen under the same name; keep the names of widgets you keep.\n",
                );
            }
        }
        Scope::NewScreens => out.push_str("\nCreate new screens with new names.\n"),
    }
    let _ = write!(out, "\nRequest:\n{}", request.trim());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_lists_all_widgets_and_styles() {
        let prompt = system_prompt();
        for def in catalog::ALL.iter().filter(|d| d.kind != WidgetKind::Screen) {
            assert!(prompt.contains(&format!("- {}:", kind_key(def.kind))), "{}", def.name);
        }
        assert!(prompt.contains("- bg_color: color"));
        assert!(prompt.contains("LV_LABEL_LONG_WRAP"));
    }

    #[test]
    fn user_message_contains_context() {
        let mut project = Project::new("P");
        let screen = project.screens[0].id;
        let button = project.new_widget(WidgetKind::Button);
        project.insert(screen, 0, button).unwrap();
        let message = user_message(&project, Scope::EditScreen(screen), "Make it blue");
        assert!(message.contains("Display: 320 x 240 px"));
        assert!(message.contains("\"name\":\"Button1\""));
        assert!(message.ends_with("Make it blue"));
    }
}
