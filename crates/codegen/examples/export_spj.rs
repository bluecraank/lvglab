//! Imports a SquareLine project and exports it as LVGL library:
//! `cargo run -p lvglab-codegen --example export_spj -- <file.spj> <out-dir>`

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: export_spj <file.spj> <out-dir>");
        std::process::exit(2);
    };
    let input = PathBuf::from(input);
    let imported = lvglab_spj::import_file(&input)?;
    let output = lvglab_codegen::generate_library(&imported.project, input.parent());
    for warning in imported.warnings.iter().chain(&output.warnings) {
        eprintln!("warning: {warning}");
    }
    let summary = lvglab_codegen::write(PathBuf::from(out).as_path(), &output, &["src"])?;
    println!("{summary:?}");
    Ok(())
}
