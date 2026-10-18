//! Fuzz tests: random projects full of unexpected values (wrong value types,
//! extreme numbers, odd names, dangling references, deep nesting) and
//! corrupted project and SquareLine files. Loading, editing, laying out and
//! exporting must never panic.
//!
//! Each case is derived from a seed; a failure prints it, and
//! `LVGLAB_FUZZ_SEED=<seed> cargo test -p lvglab-codegen --test fuzz` repeats
//! exactly that case. `LVGLAB_FUZZ_ITERATIONS=<n>` runs longer campaigns.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use lvglab_model::catalog::{self, Category};
use lvglab_model::color_theme::ThemeColor;
use lvglab_model::coord::{Align, Coord, Unit};
use lvglab_model::event::{Action, EventHandler, EventTrigger, ModifyOp, ScreenAnim};
use lvglab_model::flags::{Dir, FlexAlign, FlexFlow, ObjFlag, ObjState, ScrollbarMode, SnapMode, StateMask};
use lvglab_model::layout::{Layout, Measure};
use lvglab_model::project::{AssetKind, DisplayShape, FontAsset, ImageAsset, LvglVersion};
use lvglab_model::prop::{PropKind, PropValue};
use lvglab_model::style::{StyleKind, StyleProp, StyleValue};
use lvglab_model::theme::{StyleResolver, Theme};
use lvglab_model::widget::FlexLayout;
use lvglab_model::{Color, Project, Widget, WidgetId, WidgetKind};

/// xorshift64*: small, fast and reproducible.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// Mostly ordinary numbers, sometimes the extremes.
    fn int(&mut self) -> i32 {
        match self.below(8) {
            0 => *self.pick(&[i32::MIN, i32::MAX, i32::MIN + 1, i32::MAX - 1, 0, -1]),
            1 => self.next() as i32,
            2 => -(self.below(5000) as i32),
            _ => self.below(1000) as i32,
        }
    }

    fn text(&mut self) -> String {
        const PARTS: &[&str] = &[
            "",
            " ",
            "Button",
            "Wert",
            "äöü ß € °C",
            "😀🎉",
            "\"quoted\" \\ back",
            "line1\nline2\n\n",
            "\t\r",
            "{v}",
            "{obj}",
            "{unknown}",
            "%s %d %n",
            "int",
            "lv_obj_t",
            "__x",
            "1|2|3\n4|5",
            "||\n|",
            "0x20-0x7E",
            "0x41-0x30",
            "#FF0000: 1, 2, x",
            "#zz secondary:",
            "0,0 10,10 abc",
            "../..\\..",
            "/* */",
            "*/",
            "\u{0}",
            "\u{202e}rtl",
            "λ漢字",
        ];
        let mut out = String::new();
        for _ in 0..self.below(3) + 1 {
            out.push_str(self.pick(PARTS));
        }
        if self.chance(3) {
            out = out.repeat(200);
        }
        out
    }
}

/// Approximates text: 0.55 em per character.
struct RoughMeasure;

impl Measure for RoughMeasure {
    fn text_size(&self, text: &str, font: &str, _: i32, _: i32, _: Option<i32>) -> (i32, i32) {
        let size: i32 = font.chars().filter(char::is_ascii_digit).collect::<String>().parse().unwrap_or(14).min(500);
        let lines = text.lines().count().clamp(1, 10_000) as i32;
        let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0).min(100_000) as i32;
        (longest * size * 55 / 100, lines * size * 12 / 10)
    }

    fn line_height(&self, font: &str) -> i32 {
        self.text_size("x", font, 0, 0, None).1
    }

    fn image_size(&self, _: &str) -> Option<(i32, i32)> {
        Some((64, 64))
    }
}

fn kinds() -> Vec<WidgetKind> {
    catalog::ALL.iter().filter(|d| d.category != Category::Screen).map(|d| d.kind).collect()
}

fn coord(rng: &mut Rng) -> Coord {
    let unit = *rng.pick(&[Unit::Px, Unit::Px, Unit::Percent, Unit::Content]);
    Coord { value: rng.int(), unit }
}

fn prop_value(rng: &mut Rng, kind: PropKind) -> PropValue {
    // Now and then a value of the wrong type, as a hand-edited file could hold.
    if rng.chance(10) {
        return match rng.below(5) {
            0 => PropValue::Bool(rng.chance(50)),
            1 => PropValue::Int(rng.int()),
            2 => PropValue::Text(rng.text()),
            3 => PropValue::Pair(rng.int(), rng.int()),
            _ => PropValue::WidgetRef(Some(WidgetId(rng.next() % 500))),
        };
    }
    match kind {
        PropKind::Bool => PropValue::Bool(rng.chance(50)),
        PropKind::Int { .. } => PropValue::Int(rng.int()),
        PropKind::Text | PropKind::Lines | PropKind::Points | PropKind::StrArray { .. } => PropValue::Text(rng.text()),
        PropKind::Enum(options) => {
            let c = if rng.chance(85) && !options.is_empty() { rng.pick(options).c.to_owned() } else { rng.text() };
            PropValue::Enum(c)
        }
        PropKind::Color => PropValue::Color(Color::from_hex(rng.next() as u32 & 0xFF_FFFF)),
        PropKind::Pair { .. } => PropValue::Pair(rng.int(), rng.int()),
        PropKind::Image => PropValue::Image(if rng.chance(50) { "logo".into() } else { rng.text() }),
        PropKind::WidgetRef => PropValue::WidgetRef(rng.chance(80).then(|| WidgetId(rng.next() % 500))),
        PropKind::Date => PropValue::Date { year: rng.int(), month: rng.next() as u8, day: rng.next() as u8 },
        _ => PropValue::Text(rng.text()),
    }
}

fn style_value(rng: &mut Rng, kind: StyleKind) -> StyleValue {
    if rng.chance(10) {
        return match rng.below(4) {
            0 => StyleValue::Bool(true),
            1 => StyleValue::Int(rng.int()),
            2 => StyleValue::Theme(rng.text()),
            _ => StyleValue::Enum(rng.text()),
        };
    }
    match kind {
        StyleKind::Coord => StyleValue::Coord(coord(rng)),
        StyleKind::Int { .. } | StyleKind::Opa => StyleValue::Int(rng.int()),
        StyleKind::Color if rng.chance(20) => {
            StyleValue::Theme(if rng.chance(50) { "Accent".into() } else { rng.text() })
        }
        StyleKind::Color => StyleValue::Color(Color::from_hex(rng.next() as u32 & 0xFF_FFFF)),
        StyleKind::Bool => StyleValue::Bool(rng.chance(50)),
        StyleKind::Enum(options) => StyleValue::Enum(if rng.chance(85) && !options.is_empty() {
            rng.pick(options).c.to_owned()
        } else {
            rng.text()
        }),
        StyleKind::Font => StyleValue::Font(
            rng.pick(&["montserrat_14", "montserrat_48", "montserrat_7", "Big", "missing", ""]).to_string(),
        ),
        StyleKind::Image => StyleValue::Image(if rng.chance(50) { "logo".into() } else { rng.text() }),
    }
}

fn action(rng: &mut Rng, ids: &[WidgetId]) -> Action {
    let target = |rng: &mut Rng| match rng.below(4) {
        0 => None,
        1 => Some(WidgetId(rng.next() % 1000)),
        _ => ids.get(rng.below(ids.len())).copied(),
    };
    match rng.below(6) {
        0 => Action::ChangeScreen {
            screen: target(rng),
            anim: *rng.pick(ScreenAnim::ALL),
            time_ms: rng.next() as u32,
            delay_ms: rng.next() as u32,
        },
        1 => Action::CallFunction { name: rng.text() },
        2 => Action::ModifyFlag { target: target(rng), flag: *rng.pick(ObjFlag::ALL), op: *rng.pick(ModifyOp::ALL) },
        3 => Action::ModifyState { target: target(rng), state: *rng.pick(ObjState::ALL), op: *rng.pick(ModifyOp::ALL) },
        4 => Action::SetText { target: target(rng), text: rng.text() },
        _ => Action::SetValue { target: target(rng), value: rng.int() },
    }
}

/// Changes one random widget in a random way.
fn mutate_widget(rng: &mut Rng, w: &mut Widget, ids: &[WidgetId]) {
    let def = w.def();
    match rng.below(9) {
        0 if !def.props.is_empty() => {
            let prop = rng.pick(def.props);
            let value = prop_value(rng, prop.kind);
            w.set_prop(prop.key, value);
        }
        1 => {
            let key = rng.text();
            let value = prop_value(rng, PropKind::Text);
            w.set_prop(&key, value);
        }
        2 => {
            let slot = if rng.chance(90) { rng.pick(def.slots).key.to_owned() } else { rng.text() };
            let mut state = StateMask::DEFAULT;
            for _ in 0..rng.below(3) {
                state = state.with(*rng.pick(ObjState::ALL));
            }
            let prop = *rng.pick(StyleProp::ALL);
            let value = style_value(rng, prop.kind());
            w.styles.set(&slot, state, prop, value);
        }
        3 => {
            let g = &mut w.geometry;
            match rng.below(5) {
                0 => g.x = coord(rng),
                1 => g.y = coord(rng),
                2 => g.width = coord(rng),
                3 => g.height = coord(rng),
                _ => g.align = *rng.pick(Align::ALL),
            }
        }
        4 => w.set_flag(*rng.pick(ObjFlag::ALL), rng.chance(50)),
        5 => {
            w.states.insert(*rng.pick(ObjState::ALL));
            w.ext_click_area = rng.int();
        }
        6 => {
            w.layout = rng.chance(70).then(|| FlexLayout {
                flow: *rng.pick(FlexFlow::ALL),
                main_place: *rng.pick(FlexAlign::ALL),
                cross_place: *rng.pick(FlexAlign::ALL),
                track_place: *rng.pick(FlexAlign::ALL),
            });
            w.scroll.scrollbar_mode = Some(*rng.pick(ScrollbarMode::ALL));
            w.scroll.dir = Some(*rng.pick(Dir::ALL));
            w.scroll.snap_x = Some(*rng.pick(SnapMode::ALL));
        }
        7 => {
            let handler = EventHandler { trigger: *rng.pick(EventTrigger::ALL), action: action(rng, ids) };
            w.events.push(handler);
        }
        _ => w.name = if rng.chance(70) { format!("{}{}", rng.text(), rng.below(9)) } else { rng.text() },
    }
}

fn random_project(rng: &mut Rng) -> Project {
    let mut p = Project::new(rng.text());
    p.lvgl_version = *rng.pick(LvglVersion::ALL);
    p.display.width = rng.below(3000) as u32;
    p.display.height = rng.below(3000) as u32;
    p.display.shape = *rng.pick(DisplayShape::ALL);
    p.export.prefix = if rng.chance(70) { "ui".into() } else { rng.text() };
    p.export.library_name = rng.text();
    p.export.multilang = rng.chance(30);
    p.export.swap_bytes = rng.chance(50);
    p.color_themes.add_variable("Accent", ThemeColor { color: Color::from_hex(0xF15C00), opa: 255 });
    p.assets.images.push(ImageAsset { name: "logo".into(), path: rng.text() });
    p.assets.fonts.push(FontAsset {
        name: "Big".into(),
        path: rng.text(),
        size: rng.next() as u16,
        bpp: rng.next() as u8,
        ranges: rng.text(),
        symbols: rng.text(),
    });
    let kinds = kinds();
    for _ in 0..rng.below(80) + 5 {
        if p.every_widget().is_empty() {
            p.add_screen();
        }
        let roots: Vec<WidgetId> = p.every_widget().iter().map(|w| w.id).collect();
        let ids = roots.clone();
        match rng.below(20) {
            0 => {
                p.add_screen();
            }
            1 => {
                p.add_component();
            }
            2 if roots.len() > 2 => {
                let id = *rng.pick(&roots);
                p.remove(id);
            }
            3 if roots.len() > 2 => {
                let (id, parent) = (*rng.pick(&roots), *rng.pick(&roots));
                p.move_widget(id, parent, rng.below(5));
            }
            4 => {
                let id = *rng.pick(&roots);
                p.duplicate(id);
                p.shift(id, rng.below(5) as isize - 2);
            }
            5 => match rng.below(4) {
                0 => p.color_themes.add_theme(rng.text()),
                1 => p.color_themes.add_variable(rng.text(), ThemeColor::default()),
                2 => p.color_themes.remove_variable(rng.below(4)),
                _ => {
                    let (old, new) = (rng.text(), rng.text());
                    p.rename_theme_variable(&old, &new);
                }
            },
            6 => {
                let (old, new) = (if rng.chance(50) { "logo".into() } else { rng.text() }, rng.text());
                p.rename_asset(*rng.pick(&[AssetKind::Image, AssetKind::Font]), &old, &new);
            }
            7 => {
                p.lvgl_version = *rng.pick(LvglVersion::ALL);
                p.remove_unsupported();
            }
            8..=12 => {
                let id = *rng.pick(&roots);
                if let Some(w) = p.widget_mut(id) {
                    mutate_widget(rng, w, &ids);
                }
            }
            _ => {
                let kind = *rng.pick(&kinds);
                let mut widget = p.new_widget(kind);
                if kind == WidgetKind::Component && rng.chance(50) {
                    let name = p.components.first().map_or_else(|| rng.text(), |c| c.name.clone());
                    widget.set_prop("component", PropValue::Text(name));
                }
                let parent = *rng.pick(&roots);
                let _ = p.insert(parent, rng.below(4), widget);
            }
        }
    }
    p
}

/// The code without string literals and comments, which hold user text.
fn code_only(code: &str) -> String {
    let (mut out, mut chars) = (String::new(), code.chars().peekable());
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                while let Some(d) = chars.next() {
                    if d == '\\' {
                        chars.next();
                    } else if d == c || d == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for d in chars.by_ref() {
                    if last == '*' && d == '/' {
                        break;
                    }
                    last = d;
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Braces must balance.
fn balanced(code: &str) -> bool {
    let mut depth = 0i64;
    for c in code.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

/// Everything the editor and the export do with a project.
fn exercise(p: &mut Project) {
    let json = serde_json::to_string(p).expect("a project always serializes");
    let back: Project = serde_json::from_str(&json).expect("a saved project loads again");
    assert_eq!(serde_json::to_string(&back).unwrap(), json, "saving is stable");

    let theme = Theme::of(p);
    let resolver = StyleResolver::new(&theme).with_color_themes(&p.color_themes);
    let (w, h) = (p.display.width as i32, p.display.height as i32);
    for screen in &p.screens {
        let expanded = p.expand_instances(screen);
        let layout = Layout::compute(&expanded, w, h, &resolver, &RoughMeasure, None);
        for i in 0..layout.nodes.len() {
            let _ = layout.path(i);
        }
    }
    let _ = lvglab_codegen::glyphs::warnings(p);
    let output = lvglab_codegen::generate_library(p, None);
    for file in &output.files {
        if file.path.ends_with(".c") || file.path.ends_with(".h") {
            let code = code_only(file.contents_str());
            assert!(balanced(&code), "unbalanced braces in {}", file.path);
            if let Some(line) = code.lines().find(|l| l.contains("{unknown")) {
                panic!("unresolved placeholder in {}: {line}", file.path);
            }
        }
    }
    p.remove_unsupported();
    p.repair_ids();
}

/// Corrupts a text in one of several ways.
fn corrupt(rng: &mut Rng, text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    for _ in 0..rng.below(4) + 1 {
        if bytes.is_empty() {
            break;
        }
        let at = rng.below(bytes.len());
        match rng.below(6) {
            0 => bytes[at] = rng.next() as u8,
            1 => {
                let end = (at + rng.below(40)).min(bytes.len());
                bytes.drain(at..end);
            }
            2 => bytes.truncate(at),
            3 => {
                let token = rng.pick(&["null", "-1e309", "1e400", "\"\"", "[]", "{}", "true", "-0", "\"\u{0}\""]);
                bytes.splice(at..at, token.bytes());
            }
            4 => {
                let end = (at + rng.below(60)).min(bytes.len());
                let copy = bytes[at..end].to_vec();
                bytes.splice(at..at, copy);
            }
            _ => {
                let number = rng.int().to_string();
                bytes.splice(at..at, number.bytes());
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

const SPJ: &str = r#"{
  "root": { "guid": "ROOT", "saved_objtypeKey": "STARTEVENTS", "properties": [], "children": [
    { "guid": "S1", "saved_objtypeKey": "SCREEN", "isPage": true, "properties": [
        { "nid": 10, "strtype": "OBJECT/Name", "strval": "Main", "InheritedType": 10 },
        { "strtype": "SCREEN/Style_main", "part": "lv.PART.MAIN", "InheritedType": 11, "childs": [
          { "strtype": "_style/StyleState", "strval": "DEFAULT", "childs": [
            { "strtype": "_style/Bg_Color", "intarray": [16, 32, 48, 255] }
          ]}
        ]}
      ], "children": [
      { "guid": "B1", "saved_objtypeKey": "BUTTON", "properties": [
          { "strtype": "OBJECT/Name", "strval": "Ok" },
          { "strtype": "OBJECT/Position", "flags": 33, "intarray": [5, 10] },
          { "strtype": "OBJECT/Size", "flags": 49, "intarray": [120, 1] },
          { "strtype": "OBJECT/Align", "strval": "BOTTOM_MID" },
          { "strtype": "OBJECT/Layout_type", "strval": "Flex", "Flow": 1, "Wrap": true, "Reversed": false,
            "MainAlignment": 2, "CrossAlignment": 0, "TrackAlignment": 0, "LayoutType": 1 },
          { "strtype": "BUTTON/Style_main", "part": "lv.PART.MAIN", "childs": [
            { "strtype": "_style/StyleState", "strval": "CHECKED", "childs": [
              { "strtype": "_style/Bg_Radius", "integer": 250 },
              { "strtype": "_style/Text_Font", "strval": "FontCBI40" }
            ]}
          ]}
        ], "children": [
          { "guid": "L1", "saved_objtypeKey": "LABEL", "properties": [
            { "strtype": "OBJECT/Name", "strval": "OkLabel" },
            { "strtype": "LABEL/Text", "strval": "Line1\\nLine2" }
          ]}
      ]},
      { "guid": "K1", "saved_objtypeKey": "KEYBOARD", "properties": [
          { "strtype": "KEYBOARD/Target_textarea", "strval": "T1" }
      ]},
      { "guid": "C1", "saved_objtypeKey": "CHART", "properties": [
          { "strtype": "CHART/ChartData", "strval": "[{\"color\": [255, 0, 0], \"axis\": \"SECONDARY_Y\", \"data\": [5, 6]}]" }
      ]},
      { "guid": "X1", "saved_objtypeKey": "WEIRD", "properties": [] }
    ]}
  ]},
  "animations": [],
  "info": { "name": "Demo.spj", "width": 466, "height": 466, "shape": "CIRCLE", "lvgl_version": "9.1.0" }
}"#;

fn iterations(default: u64) -> u64 {
    std::env::var("LVGLAB_FUZZ_ITERATIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Runs `case` for each seed and names the seed of a failure.
fn campaign(name: &str, base: u64, default: u64, case: impl Fn(u64)) {
    let seeds: Vec<u64> = match std::env::var("LVGLAB_FUZZ_SEED").ok().and_then(|v| v.parse().ok()) {
        Some(seed) => vec![seed],
        None => (0..iterations(default)).map(|i| base + i).collect(),
    };
    for seed in seeds {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| case(seed))) {
            eprintln!("{name} failed with LVGLAB_FUZZ_SEED={seed}");
            resume_unwind(panic);
        }
    }
}

#[test]
fn random_projects_never_crash() {
    campaign("random_projects_never_crash", 1_000, 120, |seed| {
        let mut rng = Rng::new(seed);
        let mut project = random_project(&mut rng);
        exercise(&mut project);
    });
}

#[test]
fn corrupted_project_files_are_rejected_or_work() {
    campaign("corrupted_project_files_are_rejected_or_work", 50_000, 200, |seed| {
        let mut rng = Rng::new(seed);
        let json = serde_json::to_string_pretty(&random_project(&mut rng)).unwrap();
        let corrupted = corrupt(&mut rng, &json);
        if let Ok(mut project) = serde_json::from_str::<Project>(&corrupted) {
            project.repair_ids();
            exercise(&mut project);
        }
    });
}

#[test]
fn corrupted_squareline_files_are_rejected_or_work() {
    campaign("corrupted_squareline_files_are_rejected_or_work", 90_000, 300, |seed| {
        let mut rng = Rng::new(seed);
        let corrupted = corrupt(&mut rng, SPJ);
        if let Ok(imported) = lvglab_spj::import_str(&corrupted, None, "Fuzz") {
            let mut project = imported.project;
            exercise(&mut project);
        }
    });
}

#[test]
fn odd_font_ranges_are_rejected_or_parsed() {
    campaign("odd_font_ranges_are_rejected_or_parsed", 7_000, 500, |seed| {
        let mut rng = Rng::new(seed);
        let (ranges, symbols) = (rng.text(), rng.text());
        let _ = lvglab_codegen::font::code_points(&ranges, &symbols);
    });
}

#[test]
fn merging_random_projects_never_crashes() {
    campaign("merging_random_projects_never_crashes", 30_000, 120, |seed| {
        use lvglab_model::merge;
        let mut rng = Rng::new(seed);
        let mut target = random_project(&mut rng);
        let source = if rng.chance(30) { target.clone() } else { random_project(&mut rng) };
        let screens: Vec<WidgetId> = source.screens.iter().filter(|_| rng.chance(70)).map(|s| s.id).collect();
        let mut conflicts = merge::conflicts(&target, None, &source, None, &screens);
        // Sometimes keep the proposals, sometimes choose odd names.
        for conflict in &mut conflicts {
            if rng.chance(20) {
                conflict.resolution = if conflict.can_use_existing && rng.chance(50) {
                    merge::Resolution::UseExisting
                } else {
                    merge::Resolution::Rename(rng.text())
                };
            }
        }
        let _ = merge::problems(&target, &source, &screens, &conflicts);
        merge::merge(&mut target, &source, None, &screens, &conflicts);
        let mut ids = std::collections::HashSet::new();
        assert!(target.every_widget().iter().all(|w| ids.insert(w.id)), "ids stay unique");
        exercise(&mut target);
    });
}
