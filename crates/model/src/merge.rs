//! Merging: takes screens of another project over, with the components,
//! fonts, images and theme colors they use. Names that already exist in the
//! target project are conflicts; each is renamed or mapped to the existing
//! entry before the merge.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use crate::project::{AssetKind, Project};
use crate::prop::PropValue;
use crate::style::StyleValue;
use crate::widget::{Widget, WidgetId, WidgetKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConflictKind {
    Widget,
    Component,
    Font,
    Image,
    ThemeColor,
}

impl ConflictKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Widget => "Widget",
            Self::Component => "Component",
            Self::Font => "Font",
            Self::Image => "Image",
            Self::ThemeColor => "Theme color",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    Rename(String),
    /// Uses the entry of the target project with this name.
    UseExisting,
}

/// A name of the other project that the target project already uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub name: String,
    pub resolution: Resolution,
    /// The target's entry can stand in (not possible for widgets).
    pub can_use_existing: bool,
    /// Both entries are the same (same file and settings).
    pub identical: bool,
}

/// Names of the other project that the selected screens need.
#[derive(Debug, Default)]
struct Used {
    widgets: Vec<String>,
    components: BTreeSet<String>,
    fonts: BTreeSet<String>,
    images: BTreeSet<String>,
    theme_colors: BTreeSet<String>,
}

fn collect(widget: &Widget, used: &mut Used, names: bool) {
    widget.walk(&mut |w| {
        if names {
            used.widgets.push(w.name.clone());
        }
        if w.kind == WidgetKind::Component {
            used.components.insert(w.prop_text("component"));
        }
        for block in w.styles.blocks() {
            for value in block.props.values() {
                match value {
                    StyleValue::Font(f) => {
                        used.fonts.insert(f.clone());
                    }
                    StyleValue::Image(i) => {
                        used.images.insert(i.clone());
                    }
                    StyleValue::Theme(t) => {
                        used.theme_colors.insert(t.clone());
                    }
                    _ => {}
                }
            }
        }
        for (key, value) in &w.props {
            match value {
                PropValue::Image(i) => {
                    used.images.insert(i.clone());
                }
                // Frames of an animated image, one image per line.
                PropValue::Text(text) if key == "images" => {
                    used.images.extend(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned));
                }
                _ => {}
            }
        }
    });
}

/// What the selected screens use, including the components they place
/// (and the components those place).
fn used(source: &Project, screens: &[WidgetId]) -> Used {
    let mut used = Used::default();
    for screen in source.screens.iter().filter(|s| screens.contains(&s.id)) {
        collect(screen, &mut used, true);
    }
    let mut done = BTreeSet::new();
    while let Some(name) = used.components.iter().find(|c| !done.contains(*c)).cloned() {
        done.insert(name.clone());
        if let Some(component) = source.component(&name) {
            collect(component, &mut used, false);
        }
    }
    used.components.retain(|c| source.component(c).is_some());
    used.fonts.retain(|f| source.assets.font(f).is_some());
    used.images.retain(|i| source.assets.image(i).is_some());
    used.theme_colors.retain(|t| source.color_themes.index_of(t).is_some());
    used
}

/// A free name: `name_2`, `name_3`, ...
fn suggest(name: &str, taken: &HashSet<String>) -> String {
    (2..).map(|n| format!("{name}_{n}")).find(|n| !taken.contains(n)).expect("infinite iterator")
}

/// Names in use per kind: the target's and the incoming ones.
fn taken(target: &Project, used: &Used, kind: ConflictKind) -> HashSet<String> {
    let mut names: HashSet<String> = match kind {
        ConflictKind::Widget => target.all_widgets().iter().map(|w| w.name.clone()).collect(),
        ConflictKind::Component => target.components.iter().map(|c| c.name.clone()).collect(),
        ConflictKind::Font => target.assets.fonts.iter().map(|f| f.name.clone()).collect(),
        ConflictKind::Image => target.assets.images.iter().map(|i| i.name.clone()).collect(),
        ConflictKind::ThemeColor => target.color_themes.variables.iter().cloned().collect(),
    };
    match kind {
        ConflictKind::Widget => names.extend(used.widgets.iter().cloned()),
        ConflictKind::Component => names.extend(used.components.iter().cloned()),
        ConflictKind::Font => names.extend(used.fonts.iter().cloned()),
        ConflictKind::Image => names.extend(used.images.iter().cloned()),
        ConflictKind::ThemeColor => names.extend(used.theme_colors.iter().cloned()),
    }
    names
}

/// The conflicts of merging `screens` of `source` into `target`, each with
/// a proposed resolution. `source_dir` and `target_dir` resolve asset paths.
pub fn conflicts(
    target: &Project,
    target_dir: Option<&Path>,
    source: &Project,
    source_dir: Option<&Path>,
    screens: &[WidgetId],
) -> Vec<Conflict> {
    let used = used(source, screens);
    let mut out = Vec::new();
    let mut add = |kind: ConflictKind, name: &str, can_use_existing: bool, identical: bool| {
        let mut taken = taken(target, &used, kind);
        taken.extend(out.iter().filter_map(|c: &Conflict| match &c.resolution {
            Resolution::Rename(n) if c.kind == kind => Some(n.clone()),
            _ => None,
        }));
        let resolution = if identical || (can_use_existing && kind == ConflictKind::ThemeColor) {
            Resolution::UseExisting
        } else {
            Resolution::Rename(suggest(name, &taken))
        };
        out.push(Conflict { kind, name: name.to_owned(), resolution, can_use_existing, identical });
    };
    let widget_names: HashSet<String> = target.all_widgets().iter().map(|w| w.name.clone()).collect();
    let mut seen = HashSet::new();
    for name in &used.widgets {
        if widget_names.contains(name) && seen.insert(name.clone()) {
            add(ConflictKind::Widget, name, false, false);
        }
    }
    for name in &used.components {
        if let Some(existing) = target.component(name) {
            let identical = source.component(name).is_some_and(|c| same_tree(c, existing));
            add(ConflictKind::Component, name, true, identical);
        }
    }
    for name in &used.fonts {
        if let (Some(existing), Some(incoming)) = (target.assets.font(name), source.assets.font(name)) {
            let same_file =
                Project::resolve_path(target_dir, &existing.path) == Project::resolve_path(source_dir, &incoming.path);
            let identical = same_file
                && (existing.size, existing.bpp, &existing.ranges, &existing.symbols)
                    == (incoming.size, incoming.bpp, &incoming.ranges, &incoming.symbols);
            add(ConflictKind::Font, name, true, identical);
        }
    }
    for name in &used.images {
        if let (Some(existing), Some(incoming)) = (target.assets.image(name), source.assets.image(name)) {
            let identical =
                Project::resolve_path(target_dir, &existing.path) == Project::resolve_path(source_dir, &incoming.path);
            add(ConflictKind::Image, name, true, identical);
        }
    }
    for name in &used.theme_colors {
        if target.color_themes.index_of(name).is_some() {
            add(ConflictKind::ThemeColor, name, true, false);
        }
    }
    out
}

/// Same structure and content, ignoring ids.
fn same_tree(a: &Widget, b: &Widget) -> bool {
    let strip = |w: &Widget| {
        let mut w = w.clone();
        w.walk_mut(&mut |x| x.id = WidgetId(0));
        w
    };
    strip(a) == strip(b)
}

/// Problems with the chosen names: invalid identifiers and names that would
/// still collide. Empty if the merge can run.
pub fn problems(target: &Project, source: &Project, screens: &[WidgetId], conflicts: &[Conflict]) -> Vec<String> {
    let used = used(source, screens);
    let mut problems = Vec::new();
    for kind in [
        ConflictKind::Widget,
        ConflictKind::Component,
        ConflictKind::Font,
        ConflictKind::Image,
        ConflictKind::ThemeColor,
    ] {
        // Final names of the incoming entries of this kind.
        let incoming: Vec<&String> = match kind {
            ConflictKind::Widget => used.widgets.iter().collect(),
            ConflictKind::Component => used.components.iter().collect(),
            ConflictKind::Font => used.fonts.iter().collect(),
            ConflictKind::Image => used.images.iter().collect(),
            ConflictKind::ThemeColor => used.theme_colors.iter().collect(),
        };
        let mut finals: HashMap<String, usize> = HashMap::new();
        let mut existing = taken(target, &Used::default(), kind);
        for name in incoming {
            match conflicts.iter().find(|c| c.kind == kind && &c.name == name).map(|c| &c.resolution) {
                Some(Resolution::UseExisting) => {}
                Some(Resolution::Rename(new)) => {
                    let valid = !new.is_empty() && new.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                    if !valid {
                        problems.push(format!("{} {name}: \"{new}\" is not a valid name", kind.label()));
                    }
                    *finals.entry(new.clone()).or_default() += 1;
                }
                None => *finals.entry(name.clone()).or_default() += 1,
            }
        }
        // Widgets may repeat names inside the other project; only clashes
        // with the target and between renamed entries count.
        for (name, count) in finals {
            if existing.remove(&name) || (count > 1 && kind != ConflictKind::Widget) {
                problems.push(format!("{} name \"{name}\" is used twice", kind.label()));
            }
        }
    }
    problems
}

/// What a merge did.
#[derive(Debug, Default)]
pub struct Merged {
    /// The new screens in the target project.
    pub screens: Vec<WidgetId>,
    /// References to widgets that were not merged, now cleared.
    pub cleared_references: usize,
}

/// Merges `screens` of `source` into `target`, resolving `conflicts` first.
/// Asset paths are made absolute against `source_dir`; saving the target
/// copies the files into its `assets` folder.
pub fn merge(
    target: &mut Project,
    source: &Project,
    source_dir: Option<&Path>,
    screens: &[WidgetId],
    conflicts: &[Conflict],
) -> Merged {
    let mut src = source.clone();
    let mut skip: HashSet<(ConflictKind, String)> = HashSet::new();
    for conflict in conflicts {
        let name = conflict.name.as_str();
        match &conflict.resolution {
            Resolution::UseExisting => {
                skip.insert((conflict.kind, conflict.name.clone()));
            }
            Resolution::Rename(new) => match conflict.kind {
                ConflictKind::Widget => {
                    for screen in src.screens.iter_mut().filter(|s| screens.contains(&s.id)) {
                        screen.walk_mut(&mut |w| {
                            if w.name == name {
                                w.name.clone_from(new);
                            }
                        });
                    }
                }
                ConflictKind::Component => {
                    if let Some(component) = src.components.iter_mut().find(|c| c.name == name) {
                        component.name.clone_from(new);
                    }
                    src.rename_component_instances(name, new);
                }
                ConflictKind::Font => src.rename_asset(AssetKind::Font, name, new),
                ConflictKind::Image => src.rename_asset(AssetKind::Image, name, new),
                ConflictKind::ThemeColor => {
                    src.rename_theme_variable(name, new);
                }
            },
        }
    }
    let used = used(&src, screens);

    // New ids for everything that comes over, then the references follow them.
    let incoming: Vec<Widget> = src
        .screens
        .iter()
        .filter(|s| screens.contains(&s.id))
        .chain(src.components.iter().filter(|c| {
            used.components.contains(&c.name) && !skip.contains(&(ConflictKind::Component, c.name.clone()))
        }))
        .cloned()
        .collect();
    let mut ids = HashMap::new();
    for root in &incoming {
        root.walk(&mut |w| {
            ids.insert(w.id, ());
        });
    }
    let mut map: HashMap<WidgetId, WidgetId> = HashMap::new();
    for old in ids.keys() {
        map.insert(*old, target.alloc_id());
    }
    let mut merged = Merged::default();
    for mut root in incoming {
        let is_component = !source.screens.iter().any(|s| s.id == root.id);
        root.walk_mut(&mut |w| {
            w.id = map[&w.id];
            for value in w.props.values_mut() {
                if let PropValue::WidgetRef(Some(target)) = value {
                    match map.get(target) {
                        Some(new) => *target = *new,
                        None => {
                            *value = PropValue::WidgetRef(None);
                            merged.cleared_references += 1;
                        }
                    }
                }
            }
            for handler in &mut w.events {
                if let Some(reference) = handler.action.references_mut() {
                    if let Some(old) = *reference {
                        *reference = map.get(&old).copied();
                        if reference.is_none() {
                            merged.cleared_references += 1;
                        }
                    }
                }
            }
        });
        if is_component {
            target.components.push(root);
        } else {
            merged.screens.push(root.id);
            target.screens.push(root);
        }
    }

    for name in &used.fonts {
        if skip.contains(&(ConflictKind::Font, name.clone())) || target.assets.font(name).is_some() {
            continue;
        }
        if let Some(font) = src.assets.font(name) {
            let mut font = font.clone();
            if !font.path.trim().is_empty() {
                font.path = Project::resolve_path(source_dir, &font.path).display().to_string();
            }
            target.assets.fonts.push(font);
        }
    }
    for name in &used.images {
        if skip.contains(&(ConflictKind::Image, name.clone())) || target.assets.image(name).is_some() {
            continue;
        }
        if let Some(image) = src.assets.image(name) {
            let mut image = image.clone();
            image.path = Project::resolve_path(source_dir, &image.path).display().to_string();
            target.assets.images.push(image);
        }
    }
    for name in &used.theme_colors {
        if skip.contains(&(ConflictKind::ThemeColor, name.clone())) || target.color_themes.index_of(name).is_some() {
            continue;
        }
        // A theme of the target gets the value of the source theme with the
        // same name, or of the source's default theme.
        let fallback = src.color_themes.value_in(0, name).unwrap_or_default();
        target.color_themes.add_variable(name.clone(), fallback);
        let index = target.color_themes.index_of(name).expect("just added");
        let names: Vec<String> = target.color_themes.themes.iter().map(|t| t.name.clone()).collect();
        for (theme, theme_name) in names.iter().enumerate() {
            let source_theme = src.color_themes.themes.iter().position(|t| &t.name == theme_name);
            let value = source_theme.and_then(|t| src.color_themes.value_in(t, name)).unwrap_or(fallback);
            target.color_themes.themes[theme].values[index] = value;
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::color_theme::ThemeColor;
    use crate::event::{Action, EventHandler, EventTrigger, ScreenAnim};
    use crate::flags::StateMask;
    use crate::project::{FontAsset, ImageAsset};
    use crate::style::StyleProp;

    /// A project with two screens: Main (button that opens Second, styled
    /// with a font, an image and a theme color) and Second.
    fn source() -> Project {
        let mut p = Project::new("Other");
        let main = p.screens[0].id;
        p.widget_mut(main).unwrap().name = "Main".into();
        let second = p.add_screen();
        p.widget_mut(second).unwrap().name = "Second".into();
        p.assets.fonts.push(FontAsset {
            name: "Big".into(),
            path: "fonts/big.ttf".into(),
            size: 40,
            ..FontAsset::default()
        });
        p.assets.images.push(ImageAsset { name: "logo".into(), path: "logo.png".into() });
        p.color_themes.add_variable("Accent", ThemeColor { color: Color::from_hex(0x00FF00), opa: 255 });
        let mut button = p.new_widget(WidgetKind::Button);
        button.name = "Go".into();
        button.styles.set("main", StateMask::DEFAULT, StyleProp::TextFont, StyleValue::Font("Big".into()));
        button.styles.set("main", StateMask::DEFAULT, StyleProp::BgImgSrc, StyleValue::Image("logo".into()));
        button.styles.set("main", StateMask::DEFAULT, StyleProp::BgColor, StyleValue::Theme("Accent".into()));
        button.events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::ChangeScreen { screen: Some(second), anim: ScreenAnim::FadeOn, time_ms: 0, delay_ms: 0 },
        });
        p.insert(main, 0, button).unwrap();
        p
    }

    #[test]
    fn merges_screens_with_their_assets_and_rewires_references() {
        let source = source();
        let mut target = Project::new("Target");
        target.screens[0].name = "Main".into();
        target.assets.fonts.push(FontAsset {
            name: "Big".into(),
            path: "other.ttf".into(),
            size: 20,
            ..FontAsset::default()
        });
        let screens: Vec<WidgetId> = source.screens.iter().map(|s| s.id).collect();

        let conflicts = conflicts(&target, None, &source, Some(Path::new("/src")), &screens);
        let kinds: Vec<(ConflictKind, &str)> = conflicts.iter().map(|c| (c.kind, c.name.as_str())).collect();
        assert_eq!(kinds, [(ConflictKind::Widget, "Main"), (ConflictKind::Font, "Big")]);
        assert_eq!(conflicts[0].resolution, Resolution::Rename("Main_2".into()));
        assert!(!conflicts[1].identical);
        assert!(problems(&target, &source, &screens, &conflicts).is_empty());

        let merged = merge(&mut target, &source, Some(Path::new("/src")), &screens, &conflicts);
        assert_eq!(merged.screens.len(), 2);
        assert_eq!(merged.cleared_references, 0);
        let names: Vec<&str> = target.screens.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Main", "Main_2", "Second"]);
        // The renamed font follows into the styles; the event targets the new Second screen.
        let new_second = target.screens[2].id;
        let go = target.screens[1].children.iter().find(|w| w.name == "Go").unwrap();
        assert_eq!(
            go.styles.get("main", StateMask::DEFAULT, StyleProp::TextFont),
            Some(&StyleValue::Font("Big_2".into()))
        );
        assert_eq!(go.events[0].action.references(), Some(new_second));
        assert!(target.assets.font("Big_2").is_some_and(|f| f.path.ends_with("big.ttf") && f.path.contains("src")));
        assert!(target.assets.image("logo").is_some());
        assert_eq!(target.color_themes.value("Accent").map(|v| v.color), Some(Color::from_hex(0x00FF00)));
        // Ids stay unique.
        let mut ids = HashSet::new();
        assert!(target.every_widget().iter().all(|w| ids.insert(w.id)));
    }

    #[test]
    fn only_the_selected_screens_come_over_and_dangling_references_are_cleared() {
        let source = source();
        let mut target = Project::new("Target");
        let main = source.screens[0].id;
        let conflicts = conflicts(&target, None, &source, None, &[main]);
        let merged = merge(&mut target, &source, None, &[main], &conflicts);
        assert_eq!(merged.screens.len(), 1);
        assert_eq!(merged.cleared_references, 1, "the button pointed at the screen that stayed behind");
        assert_eq!(target.screens.len(), 2);
    }

    #[test]
    fn identical_assets_and_theme_colors_use_the_existing_entry() {
        let source = source();
        let mut target = source.clone();
        target.screens.truncate(1);
        target.screens[0].name = "Home".into();
        target.screens[0].children.clear();
        let screens = [source.screens[0].id];
        let conflicts = conflicts(&target, None, &source, None, &screens);
        let font = conflicts.iter().find(|c| c.kind == ConflictKind::Font).unwrap();
        assert!(font.identical);
        assert_eq!(font.resolution, Resolution::UseExisting);
        let theme = conflicts.iter().find(|c| c.kind == ConflictKind::ThemeColor).unwrap();
        assert_eq!(theme.resolution, Resolution::UseExisting);
        merge(&mut target, &source, None, &screens, &conflicts);
        assert_eq!(target.assets.fonts.len(), 1);
        assert_eq!(target.color_themes.variables.len(), 1);
    }

    #[test]
    fn invalid_and_clashing_names_are_problems() {
        let source = source();
        let mut target = Project::new("Target");
        target.screens[0].name = "Main".into();
        let screens = [source.screens[0].id];
        let mut conflicts = conflicts(&target, None, &source, None, &screens);
        conflicts[0].resolution = Resolution::Rename("Main".into());
        assert!(!problems(&target, &source, &screens, &conflicts).is_empty());
        conflicts[0].resolution = Resolution::Rename("Bad name".into());
        assert!(problems(&target, &source, &screens, &conflicts).iter().any(|p| p.contains("not a valid name")));
    }
}
