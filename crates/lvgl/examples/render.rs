//! Renders a screen of a project with LVGL into a PNG:
//! `cargo run -p lvglab-lvgl --example render -- project.lvglab <screen number> out.png`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().ok_or("usage: render <project> <screen> <out.png>")?);
    let index: usize = args.next().ok_or("screen number missing")?.parse()?;
    let out = args.next().ok_or("output file missing")?;
    let mut project = lvglab_model::Project::load(&path)?;
    if std::env::var("LVGL9").is_ok() {
        project.lvgl_version = lvglab_model::project::LvglVersion::V9_6_0;
    }
    if std::env::var("LVGL8").is_ok() {
        project.lvgl_version = lvglab_model::project::LvglVersion::V8_3_11;
    }
    let frame = lvglab_lvgl::render_screen(&project, index, path.parent())?;
    eprintln!("created {} of {} objects", frame.created, frame.expected);
    for line in &frame.skipped {
        eprintln!("skipped: {line}");
    }
    image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba).ok_or("bad frame")?.save(out)?;
    Ok(())
}
