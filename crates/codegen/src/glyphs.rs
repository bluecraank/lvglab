//! Checks that the fonts contain every character of the texts that use
//! them. Characters missing from an LVGL font are simply not drawn on the
//! device, which is easy to miss.

use std::collections::{BTreeSet, HashSet};

use lvglab_model::flags::StateMask;
use lvglab_model::prop::PropKind;
use lvglab_model::style::builtin_font_size;
use lvglab_model::theme::{StyleResolver, Theme};
use lvglab_model::{Project, Widget, WidgetId};

use crate::font::code_points;

/// Characters of a widget's texts that its font does not contain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingGlyphs {
    pub widget: WidgetId,
    pub font: String,
    pub chars: Vec<char>,
}

/// Code points of LVGL's built-in Montserrat fonts: ASCII, `°`, `•` and
/// the `LV_SYMBOL_*` icons.
fn builtin_covers(c: u32) -> bool {
    (0x20..=0x7E).contains(&c) || c == 0xB0 || c == 0x2022 || (0xF000..=0xF8FF).contains(&c)
}

/// The texts of a widget: all text, line and string list properties.
fn texts(widget: &Widget) -> String {
    let mut out = String::new();
    for prop in widget.def().props {
        if matches!(prop.kind, PropKind::Text | PropKind::Lines | PropKind::StrArray { .. }) && prop.key != "component"
        {
            out.push_str(&widget.prop_text(prop.key));
        }
    }
    out
}

/// Checks one widget; `path` goes from its screen to the widget.
fn check(project: &Project, resolver: &StyleResolver<'_>, path: &[&Widget]) -> Option<MissingGlyphs> {
    let widget = path.last()?;
    let text = texts(widget);
    if text.trim().is_empty() {
        return None;
    }
    let font = resolver.font(path, "main", StateMask::DEFAULT);
    let covered: Box<dyn Fn(u32) -> bool> = if builtin_font_size(&font).is_some() {
        Box::new(builtin_covers)
    } else {
        let asset = project.assets.font(&font)?;
        let points: HashSet<u32> = code_points(&asset.ranges, &asset.symbols).ok()?.into_iter().collect();
        Box::new(move |c| points.contains(&c))
    };
    let chars: BTreeSet<char> = text.chars().filter(|c| !c.is_control() && !covered(*c as u32)).collect();
    (!chars.is_empty()).then(|| MissingGlyphs { widget: widget.id, font, chars: chars.into_iter().collect() })
}

/// All widgets of the project with characters their font lacks.
pub fn missing_glyphs(project: &Project) -> Vec<MissingGlyphs> {
    let theme = Theme::new(&project.theme, &project.display);
    let resolver = StyleResolver::new(&theme).with_color_themes(&project.color_themes);
    let mut out = Vec::new();
    for root in project.screens.iter().chain(&project.components) {
        visit(project, &resolver, &mut vec![root], &mut out);
    }
    out
}

fn visit(project: &Project, resolver: &StyleResolver<'_>, path: &mut Vec<&Widget>, out: &mut Vec<MissingGlyphs>) {
    if let Some(missing) = check(project, resolver, path) {
        out.push(missing);
    }
    let widget = *path.last().expect("path is not empty");
    for child in &widget.children {
        path.push(child);
        visit(project, resolver, path, out);
        path.pop();
    }
}

/// The check for a single widget (e.g. the selection).
pub fn missing_glyphs_of(project: &Project, id: WidgetId) -> Option<MissingGlyphs> {
    let path = project.path_to(id)?;
    let theme = Theme::new(&project.theme, &project.display);
    let resolver = StyleResolver::new(&theme).with_color_themes(&project.color_themes);
    check(project, &resolver, &path)
}

pub fn warnings(project: &Project) -> Vec<String> {
    missing_glyphs(project)
        .into_iter()
        .filter_map(|m| {
            let name = project.widget(m.widget)?.name.clone();
            let chars: String = m.chars.iter().collect();
            Some(format!(
                "{name}: font {} has no glyphs for \"{chars}\"; add them to the font's ranges or symbols",
                m.font
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::WidgetKind;
    use lvglab_model::project::FontAsset;
    use lvglab_model::prop::PropValue;
    use lvglab_model::style::{StyleProp, StyleValue};

    #[test]
    fn finds_characters_the_font_lacks() {
        let mut project = Project::new("G");
        project.assets.fonts.push(FontAsset {
            name: "Digits".into(),
            ranges: "0x30-0x39".into(),
            symbols: "°".into(),
            ..FontAsset::default()
        });
        let screen = project.screens[0].id;
        let mut temp = project.new_widget(WidgetKind::Label);
        temp.set_prop("text", PropValue::Text("12°C".into()));
        temp.styles.set("main", StateMask::DEFAULT, StyleProp::TextFont, StyleValue::Font("Digits".into()));
        let temp_id = temp.id;
        project.insert(screen, 0, temp).unwrap();
        let mut umlaut = project.new_widget(WidgetKind::Label);
        umlaut.set_prop("text", PropValue::Text("Öl °".into()));
        project.insert(screen, 1, umlaut).unwrap();

        let missing = missing_glyphs(&project);
        assert_eq!(missing.len(), 2);
        assert_eq!(missing[0], MissingGlyphs { widget: temp_id, font: "Digits".into(), chars: vec!['C'] });
        assert_eq!(missing[1].chars, vec!['Ö']);
        assert!(warnings(&project)[1].contains("montserrat_14"));
        assert_eq!(missing_glyphs_of(&project, temp_id).unwrap().chars, vec!['C']);
    }
}
