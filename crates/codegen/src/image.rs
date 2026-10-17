//! Conversion of PNG/JPEG/BMP files into LVGL 8 image descriptors
//! (`LV_IMG_CF_TRUE_COLOR[_ALPHA]`, 16 bit RGB565).

use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
#[error("cannot load image `{path}`: {source}")]
pub struct ImageError {
    pub path: String,
    #[source]
    pub source: image::ImageError,
}

/// Pixel data ready to be written as C.
pub struct ConvertedImage {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub data: Vec<u8>,
}

pub fn load(path: &Path, swap: bool) -> Result<ConvertedImage, ImageError> {
    let img = image::open(path).map_err(|source| ImageError { path: path.display().to_string(), source })?;
    Ok(convert(&img.to_rgba8(), swap))
}

/// RGBA8 → RGB565, followed by an alpha byte per pixel if any pixel is not
/// fully opaque. With `swap` the color bytes are stored high byte first, as
/// LVGL expects with `LV_COLOR_16_SWAP 1`.
pub fn convert(rgba: &image::RgbaImage, swap: bool) -> ConvertedImage {
    let has_alpha = rgba.pixels().any(|p| p[3] != 255);
    let mut data = Vec::with_capacity(rgba.len());
    for p in rgba.pixels() {
        let [r, g, b, a] = p.0;
        let rgb565 = (u16::from(r >> 3) << 11) | (u16::from(g >> 2) << 5) | u16::from(b >> 3);
        data.extend_from_slice(&if swap { rgb565.to_be_bytes() } else { rgb565.to_le_bytes() });
        if has_alpha {
            data.push(a);
        }
    }
    ConvertedImage { width: rgba.width(), height: rgba.height(), has_alpha, data }
}

/// A fully transparent 1×1 image used when an asset cannot be loaded, so
/// the project still builds.
pub fn placeholder() -> ConvertedImage {
    convert(&image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0])), false)
}

/// C source of an image. `include` is the header that provides LVGL.
pub fn to_c(name: &str, symbol: &str, image: &ConvertedImage, header: &str, include: &str) -> String {
    let cf = if image.has_alpha { "LV_IMG_CF_TRUE_COLOR_ALPHA" } else { "LV_IMG_CF_TRUE_COLOR" };
    let mut out = String::with_capacity(image.data.len() * 6 + 512);
    out.push_str(header);
    let _ = write!(
        out,
        "\n#include \"{include}\"\n\n#ifndef LV_ATTRIBUTE_MEM_ALIGN\n    #define LV_ATTRIBUTE_MEM_ALIGN\n#endif\n\n"
    );
    let _ = writeln!(out, "// {name}: {} x {} px, {cf}", image.width, image.height);
    let _ = writeln!(out, "static const LV_ATTRIBUTE_MEM_ALIGN uint8_t {symbol}_data[] = {{");
    for chunk in image.data.chunks(16) {
        let bytes: Vec<String> = chunk.iter().map(|b| format!("0x{b:02X}")).collect();
        let _ = writeln!(out, "    {},", bytes.join(", "));
    }
    out.push_str("};\n\n");
    let _ = writeln!(out, "const lv_img_dsc_t {symbol} = {{");
    let _ = writeln!(out, "    .header.always_zero = 0,");
    let _ = writeln!(out, "    .header.w = {},", image.width);
    let _ = writeln!(out, "    .header.h = {},", image.height);
    let _ = writeln!(out, "    .header.cf = {cf},");
    let _ = writeln!(out, "    .data_size = sizeof({symbol}_data),");
    let _ = writeln!(out, "    .data = {symbol}_data,");
    out.push_str("};\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red_blue() -> image::RgbaImage {
        image::RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) }
        })
    }

    #[test]
    fn converts_to_rgb565() {
        let converted = convert(&red_blue(), false);
        assert!(!converted.has_alpha);
        assert_eq!(converted.data, vec![0x00, 0xF8, 0x1F, 0x00]);
        let c = to_c("logo", "ui_img_logo_png", &converted, "// header", "ui.h");
        assert!(c.contains("const lv_img_dsc_t ui_img_logo_png = {"));
        assert!(c.contains("#include \"ui.h\""));
        assert!(c.contains(".header.cf = LV_IMG_CF_TRUE_COLOR,"));
    }

    #[test]
    fn swaps_bytes_for_lv_color_16_swap() {
        assert_eq!(convert(&red_blue(), true).data, vec![0xF8, 0x00, 0x00, 0x1F]);
    }

    #[test]
    fn keeps_alpha() {
        let converted = placeholder();
        assert!(converted.has_alpha);
        assert_eq!(converted.data.len(), 3);
    }
}
