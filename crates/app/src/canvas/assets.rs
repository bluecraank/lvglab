//! Textures for image assets and egui fonts for font assets.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use eframe::egui::{
    self, ColorImage, FontData, FontDefinitions, FontFamily, FontId, FontTweak, TextureHandle, TextureOptions,
};
use lvglab_model::Project;
use lvglab_model::style::{builtin_font_line_height, builtin_font_size};

pub struct Texture {
    pub handle: TextureHandle,
    pub size: [usize; 2],
}

#[derive(Default)]
pub struct AssetCache {
    textures: HashMap<PathBuf, Option<Texture>>,
    /// Image asset name → resolved path, rebuilt every frame.
    image_paths: HashMap<String, PathBuf>,
    fonts: FontManager,
}

impl AssetCache {
    /// Call once per frame before drawing.
    pub fn begin_frame(&mut self, ctx: &egui::Context, project: &Project, dir: Option<&Path>) {
        self.fonts.begin_frame();
        self.fonts.sync(ctx, project, dir);
        self.image_paths.clear();
        for asset in &project.assets.images {
            let path = Project::resolve_path(dir, &asset.path);
            if !self.textures.contains_key(&path) {
                let texture = load_texture(ctx, &path);
                self.textures.insert(path.clone(), texture);
            }
            self.image_paths.insert(asset.name.clone(), path);
        }
    }

    /// Forgets a cached file so it is reloaded (after replacing it on disk).
    pub fn invalidate(&mut self, path: &Path) {
        self.textures.remove(path);
    }

    pub fn image(&self, name: &str) -> Option<&Texture> {
        self.textures.get(self.image_paths.get(name)?)?.as_ref()
    }

    pub fn image_sizes(&self) -> HashMap<String, (i32, i32)> {
        self.image_paths
            .keys()
            .filter_map(|name| self.image(name).map(|t| (name.clone(), (t.size[0] as i32, t.size[1] as i32))))
            .collect()
    }

    /// Line height in display pixels as LVGL computes it for `font`.
    pub fn line_height(&self, project: &Project, font: &str) -> f32 {
        if let Some(size) = builtin_font_size(font) {
            return f32::from(builtin_font_line_height(size).unwrap_or(size));
        }
        if let Some(height) = self.fonts.line_heights.get(font) {
            return *height;
        }
        let (_, size) = self.font_family(project, font);
        (size * 1.2).ceil()
    }

    /// egui font for an LVGL font name at `scale` screen points per pixel.
    pub fn font_id(&self, project: &Project, font: &str, scale: f32) -> FontId {
        let (family, size) = self.font_family(project, font);
        FontId::new((size * scale).max(1.0), family)
    }

    fn font_family(&self, project: &Project, font: &str) -> (FontFamily, f32) {
        if let Some(size) = builtin_font_size(font) {
            return (FontFamily::Proportional, f32::from(size));
        }
        if let Some(asset) = project.assets.font(font) {
            let family =
                if self.fonts.active.contains(font) { FontFamily::Name(font.into()) } else { FontFamily::Proportional };
            return (family, f32::from(asset.size.max(1)));
        }
        let digits: String = font.chars().filter(char::is_ascii_digit).collect();
        (FontFamily::Proportional, digits.parse().unwrap_or(14.0))
    }
}

fn load_texture(ctx: &egui::Context, path: &Path) -> Option<Texture> {
    let image = image::open(path).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    let handle = ctx.load_texture(path.display().to_string(), color, TextureOptions::LINEAR);
    Some(Texture { handle, size })
}

/// Registers TTF assets as egui font families. New fonts become usable one
/// frame after `set_fonts`, so families are only used once active.
#[derive(Default)]
struct FontManager {
    /// Loaded fonts: name → (file, size, ranges, symbols).
    loaded: BTreeMap<String, FontKey>,
    /// LVGL line height of each loaded font at its asset size.
    line_heights: HashMap<String, f32>,
    active: HashSet<String>,
    pending: Option<HashSet<String>>,
}

impl FontManager {
    fn begin_frame(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.active = pending;
        }
    }

    fn sync(&mut self, ctx: &egui::Context, project: &Project, dir: Option<&Path>) {
        let wanted: BTreeMap<String, FontKey> = project
            .assets
            .fonts
            .iter()
            .filter(|f| !f.path.trim().is_empty())
            .map(|f| {
                (f.name.clone(), (Project::resolve_path(dir, &f.path), f.size, f.ranges.clone(), f.symbols.clone()))
            })
            .collect();
        if wanted == self.loaded {
            return;
        }
        let mut definitions = FontDefinitions::default();
        let fallback = definitions.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
        let mut names = HashSet::new();
        self.line_heights.clear();
        for (name, (path, size, ranges, symbols)) in &wanted {
            let Ok(bytes) = std::fs::read(path) else { continue };
            let metrics = lvglab_codegen::font::metrics_from_bytes(&bytes, *size, ranges, symbols);
            if let Some(metrics) = metrics {
                self.line_heights.insert(name.clone(), metrics.line_height as f32);
            }
            let key = format!("asset:{name}");
            // The baseline below the top of the line, in fractions of the size.
            let baseline = metrics.map(|m| (m.line_height - m.base_line) as f32 / f32::from((*size).max(1)));
            let tweak = match (lvglab_codegen::font::em_metrics(&bytes), baseline) {
                (Some((a, d)), Some(b)) => lvgl_tweak(a, d, b),
                _ => FontTweak::default(),
            };
            definitions.font_data.insert(key.clone(), FontData::from_owned(bytes).tweak(tweak).into());
            let mut chain = vec![key];
            chain.extend(fallback.iter().cloned());
            definitions.families.insert(FontFamily::Name(name.as_str().into()), chain);
            names.insert(name.clone());
        }
        ctx.set_fonts(definitions);
        self.loaded = wanted;
        self.pending = Some(names);
    }
}

type FontKey = (PathBuf, u16, String, String);

/// egui sizes a font so that ascent − descent fills the font size, LVGL's
/// converter so that the em does. This tweak makes egui draw the glyphs at
/// LVGL's size and on LVGL's baseline, `baseline` × size below the top of the
/// line. All values are fractions of the em.
fn lvgl_tweak(ascent: f32, descent: f32, baseline: f32) -> FontTweak {
    let height = ascent - descent;
    if height <= 0.0 {
        return FontTweak::default();
    }
    // egui: glyphs at size × scale / height; baseline at ascent / height × size
    // plus size × scale × y_offset_factor − (1 − scale) / 2 × (ascent + descent) / height × size.
    let scale = height;
    let y_offset_factor = (baseline - ascent / height + (1.0 - scale) * 0.5 * (ascent + descent) / height) / scale;
    FontTweak { scale, y_offset_factor, ..FontTweak::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tweak_matches_lvgl_size_and_baseline() {
        let (ascent, descent, target) = (0.95_f32, -0.25_f32, 0.82_f32);
        let tweak = lvgl_tweak(ascent, descent, target);
        let size = 40.0_f32;
        let height = ascent - descent;
        // The formulas of epaint's FontImpl::new.
        let em = size * tweak.scale / height;
        let baseline = ascent / height * size + size * tweak.scale * tweak.y_offset_factor
            - (1.0 - tweak.scale) * 0.5 * (ascent + descent) / height * size;
        assert!((em - size).abs() < 1e-4, "{em}");
        assert!((baseline - target * size).abs() < 1e-3, "{baseline}");
    }
}
