//! Applies a design JSON (as returned by the model) to a new project and
//! exports it as library: `cargo run -p lvglab-ai --example apply_design -- <design.json> <out-dir>`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(out)) = (args.next(), args.next()) else {
        return Err("usage: apply_design <design.json> <out-dir>".into());
    };
    let design: lvglab_ai::Design = serde_json::from_str(&std::fs::read_to_string(input)?)?;
    let mut project = lvglab_model::Project::new("AiDemo");
    project.display.width = 480;
    project.display.height = 320;
    let applied = lvglab_ai::apply(&mut project, &design);
    for warning in applied.warnings {
        eprintln!("warning: {warning}");
    }
    // The design's first screen becomes the start screen.
    if let Some(index) = project.screens.iter().position(|s| Some(&s.name) == design.screens.first().map(|s| &s.name)) {
        let screen = project.screens.remove(index);
        project.screens.insert(0, screen);
    }
    let output = lvglab_codegen::generate_library(&project, None);
    lvglab_codegen::write(std::path::Path::new(&out), &output, &[lvglab_codegen::SRC_DIR])?;
    Ok(())
}
