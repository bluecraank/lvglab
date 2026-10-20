//! Image assets: PNG/JPEG/BMP files converted to LVGL images on export.

use std::path::Path;

use eframe::egui::{self, RichText, vec2};
use lvglab_model::Project;
use lvglab_model::project::{AssetKind, ImageAsset};

use super::fonts::{file_name, relative};
use crate::app::Studio;
use crate::i18n::tr;
use crate::visuals;
use crate::widgets::muted;

/// Images per row.
const PER_ROW: usize = 3;

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    if ui.button(format!("+ {}", tr("Add images…"))).clicked() {
        add_images(studio);
    }
    ui.add_space(4.0);
    if studio.doc.project.assets.images.is_empty() {
        muted(ui, tr("Images are converted to RGB565 C arrays (with alpha if needed) on export."));
        return;
    }
    let dir = studio.doc.dir().map(Path::to_path_buf);
    let mut remove = None;
    let mut rename = None;
    let mut replaced = Vec::new();
    let images = studio.doc.project.assets.images.clone();
    for row in (0..images.len()).step_by(PER_ROW) {
        ui.columns(PER_ROW, |cols| {
            for (column, ui) in cols.iter_mut().enumerate() {
                let index = row + column;
                let Some(image) = images.get(index) else { continue };
                egui::Frame::group(ui.style()).corner_radius(8).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    // Size of the image, and the remove button at the right.
                    ui.horizontal(|ui| {
                        let texture = studio.assets.image(&image.name);
                        muted(ui, texture.map(|t| format!("{} × {} px", t.size[0], t.size[1])).unwrap_or_default());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✖").on_hover_text(tr("Remove")).clicked() {
                                remove = Some(index);
                            }
                        });
                    });
                    let side = ui.available_width().min(110.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), side), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 6, ui.visuals().extreme_bg_color);
                    match studio.assets.image(&image.name) {
                        Some(texture) => {
                            let size = vec2(texture.size[0] as f32, texture.size[1] as f32);
                            let fit = ((side - 8.0) / size.x.max(size.y)).min(1.0);
                            let target = egui::Rect::from_center_size(rect.center(), size * fit);
                            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                            ui.painter().image(texture.handle.id(), target, uv, egui::Color32::WHITE);
                        }
                        None => {
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "?",
                                egui::FontId::proportional(22.0),
                                visuals::WARNING,
                            );
                        }
                    }
                    ui.add_space(4.0);
                    let mut name = image.name.clone();
                    let field = ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                    if field.on_hover_text(tr("Referenced as ui_img_<Name> in C.")).changed() && !name.is_empty() {
                        rename = Some((image.name.clone(), name.clone()));
                    }
                    ui.horizontal(|ui| {
                        let exists = Project::resolve_path(dir.as_deref(), &image.path).exists();
                        let text = RichText::new(file_name(&image.path)).small();
                        ui.add(egui::Label::new(if exists { text } else { text.color(visuals::WARNING) }).truncate())
                            .on_hover_text(&image.path);
                    });
                    if ui.small_button(tr("Replace…")).clicked() {
                        if let Some(path) = image_dialog().pick_file() {
                            replaced.push((index, path));
                        }
                    }
                });
            }
        });
        ui.add_space(6.0);
    }
    if let Some((old, new)) = rename {
        if studio.doc.project.assets.image(&new).is_none() {
            studio.doc.project.rename_asset(AssetKind::Image, &old, &new);
        }
    }
    for (index, path) in replaced {
        let old = Project::resolve_path(dir.as_deref(), &studio.doc.project.assets.images[index].path);
        studio.assets.invalidate(&old);
        studio.assets.invalidate(&path);
        studio.doc.project.assets.images[index].path = relative(&path, dir.as_deref());
    }
    if let Some(index) = remove {
        studio.doc.project.assets.images.remove(index);
    }
}

fn image_dialog() -> crate::sandbox::FileDialog {
    crate::sandbox::FileDialog::new().add_filter(tr("Images"), &["png", "jpg", "jpeg", "bmp"])
}

fn add_images(studio: &mut Studio) {
    let Some(paths) = image_dialog().pick_files() else { return };
    for path in paths {
        let name = asset_name(&studio.doc.project, &path, &[]);
        add_image(studio, name, &path);
    }
}

fn add_image(studio: &mut Studio, name: String, path: &Path) {
    let dir = studio.doc.dir().map(Path::to_path_buf);
    studio.doc.project.assets.images.push(ImageAsset { name, path: relative(path, dir.as_deref()) });
}

/// A free asset name derived from the file name; `taken` lists names that
/// are about to be added.
pub fn asset_name(project: &Project, path: &Path, taken: &[String]) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let base: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let mut name = base.clone();
    let mut n = 2;
    while project.assets.image(&name).is_some() || taken.contains(&name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    name
}

/// Images chosen with "Add image…" in an image field, waiting to be added
/// to the project.
pub fn pending_id() -> egui::Id {
    egui::Id::new("lvglab_pending_images")
}

/// Picks an image file for an image field; the asset is added by
/// [`add_pending`] at the end of the frame.
pub fn pick_for_field(ctx: &egui::Context, project: &Project) -> Option<String> {
    let path = image_dialog().pick_file()?;
    let mut pending: Vec<(String, std::path::PathBuf)> = ctx.data_mut(|d| d.get_temp(pending_id()).unwrap_or_default());
    let taken: Vec<String> = pending.iter().map(|(n, _)| n.clone()).collect();
    let name = asset_name(project, &path, &taken);
    pending.push((name.clone(), path));
    ctx.data_mut(|d| d.insert_temp(pending_id(), pending));
    Some(name)
}

pub fn add_pending(ctx: &egui::Context, studio: &mut Studio) {
    let pending: Option<Vec<(String, std::path::PathBuf)>> = ctx.data_mut(|d| d.remove_temp(pending_id()));
    for (name, path) in pending.unwrap_or_default() {
        add_image(studio, name, &path);
    }
}
