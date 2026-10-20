//! Times the LVGL preview: `cargo run --release -p lvglab-lvgl --example bench -- project.lvglab <screen>`
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().ok_or("usage: bench <project> <screen>")?);
    let index: usize = args.next().unwrap_or_else(|| "0".into()).parse()?;
    let project = lvglab_model::Project::load(&path)?;
    let start = Instant::now();
    let output = lvglab_codegen::generate_ui(&project, path.parent());
    println!("codegen: {:?}", start.elapsed());
    let start = Instant::now();
    let _ = lvglab_codegen::generate_ui(&project, path.parent());
    println!("codegen again: {:?}", start.elapsed());
    for _ in 0..3 {
        let start = Instant::now();
        lvglab_lvgl::render_screen(&project, index, path.parent())?;
        println!("render (all): {:?}", start.elapsed());
    }
    drop(output);
    Ok(())
}
