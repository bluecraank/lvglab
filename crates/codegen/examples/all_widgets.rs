//! Exports a project containing every widget type, used to compile-check the
//! generated code against LVGL:
//! `cargo run -p lvglab-codegen --example all_widgets -- <out-dir> [lvgl-version]`

use lvglab_model::catalog;
use lvglab_model::color_theme::ThemeColor;
use lvglab_model::event::{Action, EventHandler, EventTrigger, ModifyOp, ScreenAnim};
use lvglab_model::flags::{ObjFlag, ObjState, StateMask};
use lvglab_model::project::LvglVersion;
use lvglab_model::prop::{PropKind, PropValue};
use lvglab_model::style::{StyleKind, StyleProp, StyleValue};
use lvglab_model::{Color, Project, WidgetKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).ok_or("usage: all_widgets <out-dir> [lvgl-version]")?;
    let mut project = Project::new("AllWidgets");
    if let Some(version) = std::env::args().nth(2) {
        project.lvgl_version = LvglVersion::from_suffix(&version).ok_or("unknown LVGL version")?;
    }
    let screen = project.screens[0].id;
    let second = project.add_screen();
    project.widget_mut(second).unwrap().set_prop("temporary", PropValue::Bool(true));
    let mut textarea = None;
    for def in catalog::ALL {
        if def.kind == WidgetKind::Screen || def.required_parent.is_some() {
            continue;
        }
        let mut widget = project.new_widget(def.kind);
        widget.styles.set("main", StateMask::DEFAULT, StyleProp::BgColor, StyleValue::Color(Color::from_hex(0x336699)));
        widget.styles.set(
            "main",
            StateMask::DEFAULT.with(ObjState::Pressed),
            StyleProp::TextFont,
            StyleValue::Font("montserrat_20".into()),
        );
        for slot in def.slots {
            widget.styles.set(slot.key, StateMask::DEFAULT, StyleProp::Radius, StyleValue::Int(3));
        }
        match def.kind {
            WidgetKind::Textarea => textarea = Some(widget.id),
            WidgetKind::Keyboard => widget.set_prop("textarea", PropValue::WidgetRef(textarea)),
            WidgetKind::Calendar => {
                widget.set_prop("header", PropValue::Enum("lv_calendar_header_arrow_create({obj});".into()))
            }
            WidgetKind::Tabview => {
                for _ in 0..2 {
                    let mut page = project.new_widget(WidgetKind::TabPage);
                    page.children.push(project.new_widget(WidgetKind::Label));
                    widget.children.push(page);
                }
            }
            WidgetKind::Tileview => widget.children.push(project.new_widget(WidgetKind::Tile)),
            WidgetKind::Win | WidgetKind::Panel => widget.children.push(project.new_widget(WidgetKind::Button)),
            WidgetKind::Button => {
                widget.events = vec![
                    EventHandler {
                        trigger: EventTrigger::Clicked,
                        action: Action::ChangeScreen {
                            screen: Some(second),
                            anim: ScreenAnim::MoveLeft,
                            time_ms: 200,
                            delay_ms: 0,
                        },
                    },
                    EventHandler {
                        trigger: EventTrigger::GestureLeft,
                        action: Action::CallFunction { name: "on_swipe".into() },
                    },
                    EventHandler {
                        trigger: EventTrigger::Checked,
                        action: Action::ModifyFlag { target: textarea, flag: ObjFlag::Hidden, op: ModifyOp::Toggle },
                    },
                    EventHandler {
                        trigger: EventTrigger::Released,
                        action: Action::ModifyState { target: textarea, state: ObjState::Disabled, op: ModifyOp::Add },
                    },
                    EventHandler {
                        trigger: EventTrigger::Pressed,
                        action: Action::SetText { target: textarea, text: "Hi \"there\"".into() },
                    },
                ];
                widget.set_flag(ObjFlag::Checkable, true);
                widget.states.insert(ObjState::Checked);
            }
            _ => {}
        }
        project.insert(screen, usize::MAX, widget).map_err(|_| "insert failed")?;
    }
    // Optional real assets: a system font and a generated PNG.
    let font = std::path::Path::new("C:/Windows/Fonts/arial.ttf");
    if font.exists() {
        project.assets.fonts.push(lvglab_model::project::FontAsset {
            name: "Arial24".into(),
            path: font.display().to_string(),
            size: 24,
            ..Default::default()
        });
        let label = project.screens[0].children.iter_mut().find(|w| w.kind == WidgetKind::Label).unwrap();
        label.styles.set("main", StateMask::DEFAULT, StyleProp::TextFont, StyleValue::Font("Arial24".into()));
    }
    let png = std::env::temp_dir().join("lvglab_logo.png");
    image::RgbaImage::from_fn(8, 4, |x, _| image::Rgba([x as u8 * 30, 100, 200, if x < 4 { 255 } else { 128 }]))
        .save(&png)?;
    project
        .assets
        .images
        .push(lvglab_model::project::ImageAsset { name: "logo".into(), path: png.display().to_string() });
    let img = project.screens[0].children.iter_mut().find(|w| w.kind == WidgetKind::Image).unwrap();
    img.set_prop("src", PropValue::Image("logo".into()));

    change_every_property(&mut project);
    project.color_themes.add_variable("Accent", ThemeColor { color: Color::from_hex(0xF15C00), opa: 255 });
    let img = project.screens[0].children.iter_mut().find(|w| w.kind == WidgetKind::Image).unwrap();
    img.styles.set("main", StateMask::DEFAULT, StyleProp::ImgRecolor, StyleValue::Theme("Accent".into()));

    let output = lvglab_codegen::generate_library(&project, None);
    for warning in &output.warnings {
        eprintln!("warning: {warning}");
    }
    // For trying the project in the editor: LVGLAB_SAVE_PROJECT=1
    if std::env::var_os("LVGLAB_SAVE_PROJECT").is_some() {
        std::fs::create_dir_all(&out)?;
        project.save(&std::path::Path::new(&out).join("all.lvglab"))?;
    }
    lvglab_codegen::write(std::path::Path::new(&out), &output, &["src"])?;
    Ok(())
}

/// Gives every property a non-default value so each setter is emitted, and
/// sets every style property on the first panel.
fn change_every_property(project: &mut Project) {
    let mut first_panel = true;
    for widget in &mut project.screens[0].children {
        let def = widget.def();
        for prop in def.props {
            let value = match (prop.kind, prop.default_value()) {
                (PropKind::Bool, PropValue::Bool(b)) => PropValue::Bool(!b),
                (PropKind::Int { min, max }, PropValue::Int(v)) => {
                    PropValue::Int(if v == min { max.min(min + 1) } else { min })
                }
                (PropKind::Pair { min, max, .. }, _) => PropValue::Pair(min.max(0), max.min(100)),
                (PropKind::Enum(options), _) => PropValue::Enum(options.last().unwrap().c.to_owned()),
                (PropKind::Color, _) => PropValue::Color(Color::from_hex(0x123456)),
                _ => continue,
            };
            widget.set_prop(prop.key, value);
        }
        if widget.kind == WidgetKind::Panel && first_panel {
            first_panel = false;
            for prop in StyleProp::ALL.iter().copied() {
                let value = match prop.kind() {
                    StyleKind::Coord => StyleValue::Coord(lvglab_model::Coord::px(1)),
                    StyleKind::Int { min, max } => StyleValue::Int(1.clamp(min, max)),
                    StyleKind::Opa => StyleValue::Int(128),
                    StyleKind::Color => StyleValue::Color(Color::from_hex(0x654321)),
                    StyleKind::Bool => StyleValue::Bool(true),
                    StyleKind::Enum(options) => StyleValue::Enum(options[0].c.to_owned()),
                    StyleKind::Font => StyleValue::Font("montserrat_20".into()),
                    StyleKind::Image => StyleValue::Image("logo".into()),
                };
                widget.styles.set("main", StateMask::DEFAULT, prop, value);
            }
        }
    }
}
