//! Writes the dashboard example project (a round display with LVGL 9):
//! `cargo run -p lvglab-codegen --example dashboard -- examples/dashboard`

use lvglab_model::color::Palette;
use lvglab_model::coord::{Align, Coord};
use lvglab_model::event::{Action, EventHandler, EventTrigger, ScreenAnim};
use lvglab_model::flags::{ObjState, StateMask};
use lvglab_model::project::{DisplayShape, LvglVersion};
use lvglab_model::prop::PropValue;
use lvglab_model::style::{StyleProp, StyleValue};
use lvglab_model::{Color, Project, Widget, WidgetId, WidgetKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("usage: dashboard <out-dir>")?);
    std::fs::create_dir_all(&out)?;

    let mut project = Project::new("Dashboard");
    project.display.width = 466;
    project.display.height = 466;
    project.display.shape = DisplayShape::Round;
    project.lvgl_version = LvglVersion::V9_6_0;
    project.theme.dark = true;
    project.theme.primary = Palette::Orange;
    project.theme.secondary = Palette::Blue;
    let main = project.screens[0].id;
    project.widget_mut(main).unwrap().name = "Main".into();
    let settings = project.add_screen();
    project.widget_mut(settings).unwrap().name = "Settings".into();

    let font = |name: &str| StyleValue::Font(name.into());
    let color = |hex: u32| StyleValue::Color(Color::from_hex(hex));
    let style = |w: &mut Widget, slot: &str, prop: StyleProp, value: StyleValue| {
        w.styles.set(slot, StateMask::DEFAULT, prop, value);
    };
    let add = |project: &mut Project, screen: WidgetId, kind: WidgetKind, name: &str, align: Align, x: i32, y: i32| {
        let mut w = project.new_widget(kind);
        w.name = name.into();
        w.geometry.align = align;
        w.geometry.x = Coord::px(x);
        w.geometry.y = Coord::px(y);
        let id = w.id;
        project.insert(screen, usize::MAX, w).unwrap();
        id
    };
    let edit = |project: &mut Project, id: WidgetId, f: &dyn Fn(&mut Widget)| f(project.widget_mut(id).unwrap());

    // Main screen: speed gauge with value, fuel bar and a warning light.
    let arc = add(&mut project, main, WidgetKind::Arc, "SpeedArc", Align::Center, 0, 0);
    edit(&mut project, arc, &|w| {
        w.geometry.width = Coord::px(410);
        w.geometry.height = Coord::px(410);
        w.set_prop("range", PropValue::Pair(0, 240));
        w.set_prop("value", PropValue::Int(132));
        style(w, "main", StyleProp::ArcWidth, StyleValue::Int(22));
        style(w, "indicator", StyleProp::ArcWidth, StyleValue::Int(22));
        style(w, "knob", StyleProp::BgOpa, StyleValue::Int(0));
    });
    let speed = add(&mut project, main, WidgetKind::Label, "Speed", Align::Center, 0, -18);
    edit(&mut project, speed, &|w| {
        w.set_prop("text", PropValue::Text("132".into()));
        style(w, "main", StyleProp::TextFont, font("montserrat_48"));
    });
    let unit = add(&mut project, main, WidgetKind::Label, "Unit", Align::Center, 0, 30);
    edit(&mut project, unit, &|w| {
        w.set_prop("text", PropValue::Text("km/h".into()));
        style(w, "main", StyleProp::TextFont, font("montserrat_20"));
        style(w, "main", StyleProp::TextColor, color(0x9E9E9E));
    });
    let led = add(&mut project, main, WidgetKind::Led, "Warning", Align::TopMid, 0, 82);
    edit(&mut project, led, &|w| {
        w.geometry.width = Coord::px(16);
        w.geometry.height = Coord::px(16);
        w.set_prop("color", PropValue::Color(Color::from_hex(0xF44336)));
    });
    let fuel_label = add(&mut project, main, WidgetKind::Label, "FuelLabel", Align::BottomMid, 0, -132);
    edit(&mut project, fuel_label, &|w| {
        w.set_prop("text", PropValue::Text("Fuel".into()));
        style(w, "main", StyleProp::TextColor, color(0x9E9E9E));
    });
    let fuel = add(&mut project, main, WidgetKind::Bar, "Fuel", Align::BottomMid, 0, -110);
    edit(&mut project, fuel, &|w| {
        w.geometry.width = Coord::px(170);
        w.geometry.height = Coord::px(12);
        w.set_prop("value", PropValue::Int(65));
    });
    let button = add(&mut project, main, WidgetKind::Button, "SettingsButton", Align::BottomMid, 0, -48);
    edit(&mut project, button, &|w| {
        w.geometry.width = Coord::px(120);
        w.geometry.height = Coord::px(40);
        if let Some(label) = w.children.first_mut() {
            label.name = "SettingsLabel".into();
            label.set_prop("text", PropValue::Text("Settings".into()));
        }
        w.events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::ChangeScreen {
                screen: Some(settings),
                anim: ScreenAnim::MoveLeft,
                time_ms: 300,
                delay_ms: 0,
            },
        });
    });

    // Settings screen.
    let title = add(&mut project, settings, WidgetKind::Label, "Title", Align::TopMid, 0, 70);
    edit(&mut project, title, &|w| {
        w.set_prop("text", PropValue::Text("Settings".into()));
        style(w, "main", StyleProp::TextFont, font("montserrat_28"));
    });
    let rows = [("Bluetooth", true), ("Night mode", false)];
    for (i, (text, on)) in rows.into_iter().enumerate() {
        let y = -70 + i as i32 * 50;
        let label = add(
            &mut project,
            settings,
            WidgetKind::Label,
            &format!("{}Label", text.replace(' ', "")),
            Align::Center,
            -60,
            y,
        );
        edit(&mut project, label, &|w| w.set_prop("text", PropValue::Text(text.into())));
        let switch = add(&mut project, settings, WidgetKind::Switch, &text.replace(' ', ""), Align::Center, 90, y);
        edit(&mut project, switch, &|w| {
            if on {
                w.states.insert(ObjState::Checked);
            }
        });
    }
    let brightness = add(&mut project, settings, WidgetKind::Slider, "Brightness", Align::Center, 0, 40);
    edit(&mut project, brightness, &|w| {
        w.geometry.width = Coord::px(220);
        w.set_prop("value", PropValue::Int(70));
    });
    let units = add(&mut project, settings, WidgetKind::Dropdown, "Units", Align::Center, 0, 95);
    edit(&mut project, units, &|w| {
        w.geometry.width = Coord::px(160);
        w.set_prop("options", PropValue::Text("km/h\nmph".into()));
    });
    let back = add(&mut project, settings, WidgetKind::Button, "BackButton", Align::BottomMid, 0, -48);
    edit(&mut project, back, &|w| {
        w.geometry.width = Coord::px(120);
        w.geometry.height = Coord::px(40);
        if let Some(label) = w.children.first_mut() {
            label.name = "BackLabel".into();
            label.set_prop("text", PropValue::Text("Back".into()));
        }
        w.events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::ChangeScreen { screen: Some(main), anim: ScreenAnim::MoveRight, time_ms: 300, delay_ms: 0 },
        });
    });

    project.save(&out.join("dashboard.lvglab"))?;
    Ok(())
}
