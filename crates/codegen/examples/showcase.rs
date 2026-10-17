//! Writes a small showcase project (`.lvglab`) and exports it as library:
//! `cargo run -p lvglab-codegen --example showcase -- <out-dir> [font.ttf]`

use lvglab_model::coord::{Align, Coord};
use lvglab_model::flags::{ObjState, StateMask};
use lvglab_model::project::{FontAsset, ImageAsset};
use lvglab_model::prop::PropValue;
use lvglab_model::style::{StyleProp, StyleValue};
use lvglab_model::{Project, WidgetKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().ok_or("usage: showcase <out-dir> [font.ttf]")?);
    let font = args.next();
    std::fs::create_dir_all(&out)?;

    let mut project = Project::new("Showcase");
    project.display.width = 480;
    project.display.height = 320;
    let screen = project.screens[0].id;

    let png = out.join("gradient.png");
    image::RgbaImage::from_fn(64, 64, |x, y| {
        image::Rgba([x as u8 * 4, y as u8 * 4, 160, if (x / 8 + y / 8) % 2 == 0 { 255 } else { 90 }])
    })
    .save(&png)?;
    // Asset paths are stored relative to the project file.
    project.assets.images.push(ImageAsset { name: "gradient".into(), path: "gradient.png".into() });
    if let Some(path) = &font {
        project.assets.fonts.push(FontAsset {
            name: "Title28".into(),
            path: path.clone(),
            size: 28,
            ..FontAsset::default()
        });
    }

    let add = |project: &mut Project,
               kind: WidgetKind,
               align: Align,
               x: i32,
               y: i32,
               edit: &dyn Fn(&mut lvglab_model::Widget)| {
        let mut w = project.new_widget(kind);
        w.geometry.align = align;
        w.geometry.x = Coord::px(x);
        w.geometry.y = Coord::px(y);
        edit(&mut w);
        project.insert(screen, usize::MAX, w).unwrap();
    };
    add(&mut project, WidgetKind::Label, Align::TopMid, 0, 12, &|w| {
        w.set_prop("text", PropValue::Text("Grüße aus LVGL °C".into()));
        let font = if font.is_some() { "Title28" } else { "montserrat_28" };
        w.styles.set("main", StateMask::DEFAULT, StyleProp::TextFont, StyleValue::Font(font.into()));
    });
    add(&mut project, WidgetKind::Image, Align::TopLeft, 20, 60, &|w| {
        w.set_prop("src", PropValue::Image("gradient".into()))
    });
    add(&mut project, WidgetKind::Arc, Align::TopRight, -20, 60, &|w| {
        w.geometry.width = Coord::px(110);
        w.geometry.height = Coord::px(110);
        w.set_prop("value", PropValue::Int(65));
    });
    add(&mut project, WidgetKind::Slider, Align::Center, 0, -10, &|w| w.set_prop("value", PropValue::Int(40)));
    add(&mut project, WidgetKind::Bar, Align::Center, 0, 25, &|w| w.set_prop("value", PropValue::Int(70)));
    add(&mut project, WidgetKind::Switch, Align::BottomLeft, 20, -70, &|w| {
        w.states.insert(ObjState::Checked);
    });
    add(&mut project, WidgetKind::Checkbox, Align::BottomLeft, 90, -70, &|w| {
        w.states.insert(ObjState::Checked);
    });
    add(&mut project, WidgetKind::Button, Align::BottomMid, 0, -15, &|w| w.geometry.width = Coord::px(140));
    add(&mut project, WidgetKind::Dropdown, Align::BottomRight, -20, -70, &|w| w.geometry.width = Coord::px(130));

    project.save(&out.join("showcase.lvglab"))?;
    let output = lvglab_codegen::generate_library(&project, Some(&out));
    for warning in &output.warnings {
        eprintln!("warning: {warning}");
    }
    lvglab_codegen::write(&out.join("ui"), &output, &["src"])?;
    Ok(())
}
