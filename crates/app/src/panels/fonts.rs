//! Font assets: TTF/OTF files rendered to LVGL fonts on export.

use std::collections::HashMap;
use std::path::Path;

use eframe::egui::{self, RichText};
use lvglab_model::Project;
use lvglab_model::project::{AssetKind, FontAsset};
use lvglab_model::style::StyleValue;

use crate::app::Studio;
use crate::i18n::{tr, trf};
use crate::visuals;
use crate::widgets::{grid, label, muted};

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    if ui.button(format!("+ {}", tr("Add font…"))).clicked() {
        add_fonts(studio);
    }
    ui.add_space(4.0);
    if studio.doc.project.assets.fonts.is_empty() {
        muted(ui, tr("Fonts are converted to LVGL bitmap fonts when the library is exported."));
        return;
    }
    let dir = studio.doc.dir().map(Path::to_path_buf);
    let mut remove = None;
    let mut rename = None;
    let usage = font_usage(&studio.doc.project);
    let families: HashMap<String, String> = studio
        .doc
        .project
        .assets
        .fonts
        .iter()
        .filter_map(|f| Some((f.name.clone(), studio.assets.asset_family(&f.name)?)))
        .collect();
    for (index, font) in studio.doc.project.assets.fonts.iter_mut().enumerate() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&font.name).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✖").on_hover_text(tr("Remove")).clicked() {
                        remove = Some(index);
                    }
                });
            });
            let mut name = font.name.clone();
            grid(ui, ("font", index), |ui| {
                label(ui, tr("Name"), tr("Referenced as ui_font_<Name> in C."));
                if ui.text_edit_singleline(&mut name).changed() && !name.is_empty() {
                    rename = Some((font.name.clone(), name.clone()));
                }
                ui.end_row();
                label(ui, tr("File"), "");
                ui.horizontal(|ui| {
                    let missing = font.path.is_empty() || !Project::resolve_path(dir.as_deref(), &font.path).exists();
                    let text = if font.path.is_empty() { tr("No file").to_owned() } else { file_name(&font.path) };
                    let text = if missing { RichText::new(text).color(visuals::WARNING) } else { RichText::new(text) };
                    ui.label(text).on_hover_text(&font.path);
                    if ui.small_button(tr("Choose…")).clicked() {
                        if let Some(path) = pick_font_file().pick_file() {
                            font.path = relative(&path, dir.as_deref());
                        }
                    }
                });
                ui.end_row();
                label(ui, tr("Size"), "");
                ui.add(egui::DragValue::new(&mut font.size).range(6..=200).suffix(" px"));
                ui.end_row();
                label(ui, tr("Bits per pixel"), tr("Higher values give smoother edges but need more flash."));
                ui.horizontal(|ui| {
                    for bpp in [1u8, 2, 4, 8] {
                        ui.selectable_value(&mut font.bpp, bpp, bpp.to_string());
                    }
                });
                ui.end_row();
                label(ui, tr("Ranges"), tr("Unicode ranges, e.g. 0x20-0x7E, 0xB0"));
                ui.text_edit_singleline(&mut font.ranges);
                ui.end_row();
                label(ui, tr("Symbols"), tr("Extra characters to include."));
                ui.text_edit_singleline(&mut font.symbols);
                ui.end_row();
            });
            if let Some(family) = families.get(&font.name).map(|f| egui::FontFamily::Name(f.as_str().into())) {
                ui.label(
                    RichText::new("AaBbCc 0123 äöü °C").font(egui::FontId::new(f32::from(font.size).min(40.0), family)),
                );
            }
            muted(ui, trf("Used by {} widgets", &[&usage.get(&font.name).copied().unwrap_or(0)]));
        });
        ui.add_space(4.0);
    }
    if let Some((old, new)) = rename {
        if !studio.doc.project.assets.fonts.iter().any(|f| f.name == new) {
            studio.doc.project.rename_asset(AssetKind::Font, &old, &new);
        }
    }
    if let Some(index) = remove {
        studio.doc.project.assets.fonts.remove(index);
    }
}

/// Number of widgets using each font.
fn font_usage(project: &Project) -> HashMap<String, usize> {
    let mut usage = HashMap::new();
    for widget in project.all_widgets() {
        let mut fonts: Vec<&str> = widget
            .styles
            .blocks()
            .flat_map(|b| b.props.values())
            .filter_map(|v| match v {
                StyleValue::Font(f) => Some(f.as_str()),
                _ => None,
            })
            .collect();
        fonts.dedup();
        for font in fonts {
            *usage.entry(font.to_owned()).or_default() += 1;
        }
    }
    usage
}

fn pick_font_file() -> crate::sandbox::FileDialog {
    let dialog = crate::sandbox::FileDialog::new().add_filter(tr("Fonts"), &["ttf", "otf"]);
    // Start where the fonts of the system are.
    let system =
        ["C:/Windows/Fonts", "/Library/Fonts", "/usr/share/fonts"].into_iter().map(Path::new).find(|p| p.is_dir());
    match system {
        Some(dir) => dialog.set_directory(dir),
        None => dialog,
    }
}

fn add_fonts(studio: &mut Studio) {
    let Some(paths) = pick_font_file().pick_files() else { return };
    let dir = studio.doc.dir().map(Path::to_path_buf);
    for path in paths {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Font");
        let base: String = stem.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let size = 16;
        let mut name = format!("{base}{size}");
        let mut n = 2;
        while studio.doc.project.assets.font(&name).is_some() {
            name = format!("{base}{size}_{n}");
            n += 1;
        }
        studio.doc.project.assets.fonts.push(FontAsset {
            name,
            path: relative(&path, dir.as_deref()),
            size,
            ..FontAsset::default()
        });
    }
}

pub fn file_name(path: &str) -> String {
    Path::new(path).file_name().map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// Stores paths relative to the project file when possible.
pub fn relative(path: &Path, dir: Option<&Path>) -> String {
    dir.and_then(|d| path.strip_prefix(d).ok()).unwrap_or(path).display().to_string().replace('\\', "/")
}
