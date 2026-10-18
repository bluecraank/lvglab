//! Writes the export of a project: `cargo run -p lvglab-lvgl --example dump -- project.lvglab <out-dir>`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().ok_or("usage: dump <project> <dir>")?);
    let out = std::path::PathBuf::from(args.next().ok_or("directory missing")?);
    let project = lvglab_model::Project::load(&path)?;
    let output = lvglab_codegen::generate_ui(&project, path.parent());
    lvglab_codegen::write(&out, &output, &[])?;
    Ok(())
}
