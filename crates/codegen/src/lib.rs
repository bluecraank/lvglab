//! Code generation for LVGLab projects.
//!
//! * [`generate_ui`] produces the LVGL 8 C sources of the UI (screens,
//!   events, helpers, converted images and fonts).
//! * [`generate_library`] wraps them into a library folder (`library.json`,
//!   `library.properties`, `src/`) that drops into the `lib` folder of a
//!   PlatformIO project or the Arduino libraries folder.
//!
//! Both are pure functions returning [`GeneratedFile`]s; [`write`] puts them
//! on disk without touching files the user owns.

pub mod font;
pub mod glyphs;
pub mod image;
pub mod library;
pub mod naming;
pub mod output;
pub mod template;
pub mod ui;

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use lvglab_model::Project;
use lvglab_model::style::{MONTSERRAT_SIZES, StyleValue, builtin_font_size};

pub use output::{GeneratedFile, WritePolicy, WriteSummary};

/// Folder of the sources inside the library.
pub const SRC_DIR: &str = "src";

/// Result of a generation run.
#[derive(Debug, Default)]
pub struct Output {
    pub files: Vec<GeneratedFile>,
    pub warnings: Vec<String>,
}

/// Generates the UI sources; paths are relative to the UI folder.
/// `project_dir` resolves relative asset paths.
pub fn generate_ui(project: &Project, project_dir: Option<&Path>) -> Output {
    build_ui(project, project_dir)
}

/// Generates the UI as a library; paths are relative to the library folder.
pub fn generate_library(project: &Project, project_dir: Option<&Path>) -> Output {
    let mut output = build_ui(project, project_dir);
    for file in &mut output.files {
        file.path = format!("{SRC_DIR}/{}", file.path);
    }
    output.files.extend(library::manifests(project));
    output
}

/// Writes generated files; stale generated files in `managed_dirs` are removed.
pub fn write(root: &Path, output: &Output, managed_dirs: &[&str]) -> io::Result<WriteSummary> {
    output::write_all(root, &output.files, managed_dirs)
}

fn build_ui(project: &Project, project_dir: Option<&Path>) -> Output {
    let header = ui::header(project);
    let names = naming::Naming::new(project);
    let include = format!("{}.h", names.prefix());
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    let mut decls = ui::AssetDecls { montserrat: used_builtin_fonts(project), ..ui::AssetDecls::default() };

    for asset in &project.assets.images {
        let path = Project::resolve_path(project_dir, &asset.path);
        let converted = image::load(&path, project.export.swap_bytes).unwrap_or_else(|err| {
            warnings.push(format!("{err}; a transparent placeholder is used"));
            image::placeholder()
        });
        let symbol = names.image(&asset.name);
        files.push(GeneratedFile::text(
            format!("{symbol}.c"),
            image::to_c(&asset.name, &symbol, &converted, &header, &include),
            WritePolicy::Overwrite,
        ));
        decls.images.push((symbol, asset.path.clone()));
    }

    for asset in &project.assets.fonts {
        let symbol = names.font(&asset.name);
        match font::convert(asset, project_dir, &symbol, &header, &include) {
            Ok(code) => {
                files.push(GeneratedFile::text(format!("{symbol}.c"), code, WritePolicy::Overwrite));
                decls.fonts.push(symbol);
            }
            Err(err) => {
                let fallback = nearest_montserrat(asset.size);
                decls.montserrat.insert(fallback);
                warnings.push(format!("{err}; Montserrat {fallback} is used instead"));
                decls.font_fallbacks.push(format!(
                    "LV_FONT_DECLARE(lv_font_montserrat_{fallback});\n#define {symbol} lv_font_montserrat_{fallback}"
                ));
            }
        }
    }

    let sources = ui::generate(project, &decls);
    warnings.extend(sources.warnings);
    warnings.extend(missing_asset_warnings(project));
    warnings.extend(glyphs::warnings(project));
    files.extend(sources.files);
    Output { files, warnings }
}

fn used_builtin_fonts(project: &Project) -> BTreeSet<u16> {
    let mut sizes = BTreeSet::new();
    for widget in project.all_widgets() {
        for block in widget.styles.blocks() {
            for value in block.props.values() {
                if let StyleValue::Font(name) = value {
                    if let Some(size) = builtin_font_size(name) {
                        sizes.insert(size);
                    }
                }
            }
        }
    }
    sizes
}

fn nearest_montserrat(size: u16) -> u16 {
    *MONTSERRAT_SIZES.iter().min_by_key(|s| s.abs_diff(size)).expect("not empty")
}

/// Fonts and images referenced by styles or properties but not defined as assets.
fn missing_asset_warnings(project: &Project) -> Vec<String> {
    let mut missing = BTreeSet::new();
    for widget in project.all_widgets() {
        for block in widget.styles.blocks() {
            for value in block.props.values() {
                match value {
                    StyleValue::Font(f) if builtin_font_size(f).is_none() && project.assets.font(f).is_none() => {
                        missing.insert(format!("font `{f}` used by {} is not defined in the assets", widget.name));
                    }
                    StyleValue::Image(i) if !i.is_empty() && project.assets.image(i).is_none() => {
                        missing.insert(format!("image `{i}` used by {} is not defined in the assets", widget.name));
                    }
                    _ => {}
                }
            }
        }
        for (key, value) in &widget.props {
            if let lvglab_model::prop::PropValue::Image(i) = value {
                if !i.is_empty() && project.assets.image(i).is_none() {
                    missing.insert(format!("image `{i}` ({}.{key}) is not defined in the assets", widget.name));
                }
            }
        }
    }
    missing.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::project::FontAsset;

    #[test]
    fn library_contains_all_parts() {
        let mut project = Project::new("Demo");
        project.assets.fonts.push(FontAsset { name: "Big".into(), size: 41, ..FontAsset::default() });
        let output = generate_library(&project, None);
        let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        for expected in ["library.json", "library.properties", "src/ui.h", "src/ui.c", "src/ui_Screen1.c"] {
            assert!(paths.contains(&expected), "{expected} missing in {paths:?}");
        }
        assert!(output.warnings.iter().any(|w| w.contains("Montserrat 40")), "{:?}", output.warnings);
        let file = |path: &str| output.files.iter().find(|f| f.path == path).unwrap().contents_str();
        assert!(file("src/ui.h").contains("#define ui_font_Big lv_font_montserrat_40"));
        assert!(file("src/ui.c").contains("#if LV_FONT_MONTSERRAT_40 == 0"));
    }

    #[test]
    fn exports_the_example_projects() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spj");
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for path in entries.filter_map(Result::ok).map(|e| e.path()) {
            if path.extension().is_some_and(|e| e == "spj") {
                let imported = lvglab_spj::import_file(&path).unwrap();
                let output = generate_library(&imported.project, Some(&dir));
                assert!(output.files.len() > 5);
                for file in &output.files {
                    assert!(!file.contents_str().contains("/* unknown {"), "{} has unresolved placeholders", file.path);
                }
            }
        }
    }
}
