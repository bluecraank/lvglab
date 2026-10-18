//! Rough memory estimate of the exported UI and widget counts, shown in
//! the toolbar. Flash counts images, fonts and code; RAM counts the LVGL
//! objects and their local styles that exist after `ui_init()`. Display
//! buffers and LVGL itself depend on the firmware and are not included.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lvglab_model::{Project, Widget, WidgetId, WidgetKind};

/// Bytes of one LVGL 8 object on a 32 bit MCU, including its
/// `_lv_obj_spec_attr_t` and allocation overhead (approximation).
const OBJ_BYTES: usize = 100;
/// Bytes per local style property (style entry plus value).
const STYLE_PROP_BYTES: usize = 12;
/// Bytes of generated code per object and per style property.
const CODE_PER_OBJECT: usize = 140;
const CODE_PER_STYLE: usize = 28;
/// Bitmap bytes of a rendered glyph relative to `size² · bpp / 8`
/// (measured on exported fonts).
const GLYPH_FILL: f64 = 0.33;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub ram: usize,
    pub flash_images: usize,
    pub flash_fonts: usize,
    pub flash_code: usize,
    pub widgets_screen: usize,
    pub widgets_total: usize,
}

impl Stats {
    pub fn flash(&self) -> usize {
        self.flash_images + self.flash_fonts + self.flash_code
    }
}

/// Image sizes read from the file headers, cached by path.
#[derive(Default)]
pub struct StatsCache {
    images: HashMap<PathBuf, Option<(u32, u32, bool)>>,
}

impl StatsCache {
    fn image(&mut self, path: &Path) -> Option<(u32, u32, bool)> {
        *self.images.entry(path.to_path_buf()).or_insert_with(|| {
            let reader = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
            let decoder = reader.into_decoder().ok()?;
            let (w, h) = image::ImageDecoder::dimensions(&decoder);
            Some((w, h, image::ImageDecoder::color_type(&decoder).has_alpha()))
        })
    }
}

/// Estimates the memory of `project`; `screen` is the screen on the canvas.
pub fn compute(project: &Project, dir: Option<&Path>, screen: WidgetId, cache: &mut StatsCache) -> Stats {
    let mut stats = Stats::default();
    let mut largest_temporary = 0;
    for root in &project.screens {
        let (objects, styles, text) = count(root, project);
        let ram = objects * OBJ_BYTES + styles * STYLE_PROP_BYTES + text;
        if root.prop_bool("temporary") {
            largest_temporary = largest_temporary.max(ram);
        } else {
            stats.ram += ram;
        }
        stats.flash_code += objects * CODE_PER_OBJECT + styles * CODE_PER_STYLE + text;
        // The screen object itself is not counted as a widget.
        stats.widgets_total += objects - 1;
        if root.id == screen {
            stats.widgets_screen = objects - 1;
        }
    }
    // Temporary screens exist one at a time.
    stats.ram += largest_temporary;
    for component in &project.components {
        let (objects, styles, text) = count(component, project);
        stats.flash_code += objects.saturating_sub(1) * CODE_PER_OBJECT + styles * CODE_PER_STYLE + text;
        if component.id == screen {
            stats.widgets_screen = objects.saturating_sub(2);
        }
    }
    for image in &project.assets.images {
        let path = Project::resolve_path(dir, &image.path);
        if let Some((w, h, alpha)) = cache.image(&path) {
            let bytes_per_pixel = if alpha { 3 } else { 2 };
            stats.flash_images += (w * h) as usize * bytes_per_pixel + 28;
        }
    }
    for font in &project.assets.fonts {
        let glyphs = lvglab_codegen::font::code_points(&font.ranges, &font.symbols).map_or(0, |p| p.len());
        let size = f64::from(font.size.max(1));
        let bpp = f64::from(font.bpp.clamp(1, 8));
        let bitmap = glyphs as f64 * size * size * bpp / 8.0 * GLYPH_FILL;
        stats.flash_fonts += bitmap as usize + glyphs * 8 + 120;
    }
    stats
}

/// Objects, local style properties and text bytes of a tree (component
/// instances count with their component).
fn count(root: &Widget, project: &Project) -> (usize, usize, usize) {
    let mut objects = 0;
    let mut styles = 0;
    let mut text = 0;
    root.walk(&mut |w| {
        objects += 1;
        styles += w.styles.blocks().map(|b| b.props.len()).sum::<usize>();
        text += w.prop_text("text").len();
        if w.kind == WidgetKind::Component {
            if let Some(component) = project.component(&w.prop_text("component")) {
                let (o, s, t) = count(component, project);
                objects += o.saturating_sub(2);
                styles += s;
                text += t;
            }
        }
    });
    (objects, styles, text)
}

/// `12.3 KB` style sizes.
pub fn format_bytes(bytes: usize) -> String {
    match bytes {
        0..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.2} MB", bytes as f64 / 1_048_576.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::project::FontAsset;

    #[test]
    fn counts_widgets_and_estimates_memory() {
        let mut project = Project::new("S");
        let screen = project.screens[0].id;
        let button = project.new_widget(WidgetKind::Button);
        project.insert(screen, 0, button).unwrap();
        let second = project.add_screen();
        project.widget_mut(second).unwrap().set_prop("temporary", lvglab_model::prop::PropValue::Bool(true));
        project.assets.fonts.push(FontAsset {
            name: "Big".into(),
            size: 40,
            bpp: 2,
            ranges: "0x30-0x39".into(),
            symbols: String::new(),
            ..FontAsset::default()
        });
        let stats = compute(&project, None, screen, &mut StatsCache::default());
        // A button and its label; screens are not counted.
        assert_eq!((stats.widgets_screen, stats.widgets_total), (2, 2));
        assert!(stats.ram >= 4 * OBJ_BYTES);
        // 10 glyphs of 40 px at 2 bpp: about 1.3 KB of bitmaps.
        assert!((1200..1800).contains(&stats.flash_fonts), "{}", stats.flash_fonts);
        assert_eq!(format_bytes(1536), "1.5 KB");
    }
}
