//! Code generation for LVGLab projects.
//!
//! * [`generate_ui`] produces the LVGL 8 or 9 C sources of the UI (screens,
//!   events, helpers, converted images and fonts).
//! * [`generate_library`] wraps them into a library folder (`library.json`,
//!   `library.properties`, `src/`) that drops into the `lib` folder of a
//!   PlatformIO project or the Arduino libraries folder.
//!
//! Both are pure functions returning [`GeneratedFile`]s; [`write`] puts them
//! on disk without touching files the user owns.

pub mod api;
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

/// Converted images and fonts by everything that goes into them (file, its
/// modification time, settings, names). Rendering a TrueType font or
/// converting an image takes far longer than generating the rest of the UI,
/// and the preview generates the UI on every change.
static CONVERTED: std::sync::Mutex<Option<std::collections::HashMap<String, Result<String, String>>>> =
    std::sync::Mutex::new(None);

fn converted(key: String, path: &Path, convert: impl FnOnce() -> Result<String, String>) -> Result<String, String> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let key = format!("{key}|{}|{modified:?}", path.display());
    if let Some(hit) = CONVERTED.lock().ok().and_then(|c| c.as_ref().and_then(|c| c.get(&key).cloned())) {
        return hit;
    }
    let result = convert();
    if let Ok(mut cache) = CONVERTED.lock() {
        let cache = cache.get_or_insert_with(Default::default);
        if cache.len() > 256 {
            cache.clear();
        }
        cache.insert(key, result.clone());
    }
    result
}

fn build_ui(project: &Project, project_dir: Option<&Path>) -> Output {
    let header = ui::header(project);
    let names = naming::Naming::new(project);
    let include = format!("{}.h", names.prefix());
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    let mut decls = ui::AssetDecls { montserrat: used_builtin_fonts(project), ..ui::AssetDecls::default() };
    let target = api::Target::of(project);
    // LVGL 9 swaps the bytes in the display driver, not in the images.
    let swap = project.export.swap_bytes && !target.is_v9();

    for asset in &project.assets.images {
        let path = Project::resolve_path(project_dir, &asset.path);
        let symbol = names.image(&asset.name);
        let key = format!("image|{}|{symbol}|{swap}|{target:?}|{header}|{include}", asset.name);
        let code = converted(key, &path, || {
            let (image, warning) = match image::load(&path, swap) {
                Ok(image) => (image, None),
                Err(err) => (image::placeholder(), Some(format!("{err}; a transparent placeholder is used"))),
            };
            let code = image::to_c(&asset.name, &symbol, &image, &header, &include, target);
            // A failed load is remembered as an error that still carries the placeholder.
            match warning {
                None => Ok(code),
                Some(warning) => Err(format!("{warning}\u{0}{code}")),
            }
        });
        let code = match code {
            Ok(code) => code,
            Err(both) => {
                let (warning, code) = both.split_once('\u{0}').unwrap_or((both.as_str(), ""));
                warnings.push(warning.to_owned());
                code.to_owned()
            }
        };
        files.push(GeneratedFile::text(format!("{symbol}.c"), code, WritePolicy::Overwrite));
        decls.images.push((symbol, asset.path.clone()));
    }

    for asset in &project.assets.fonts {
        let symbol = names.font(&asset.name);
        let path = Project::resolve_path(project_dir, &asset.path);
        let key = format!(
            "font|{}|{}|{}|{}|{}|{symbol}|{header}|{include}",
            asset.name, asset.size, asset.bpp, asset.ranges, asset.symbols
        );
        let result = converted(key, &path, || {
            font::convert(asset, project_dir, &symbol, &header, &include).map_err(|e| e.to_string())
        });
        match result {
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
