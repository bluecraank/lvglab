//! Rendering TrueType/OpenType fonts into LVGL 8 and 9 bitmap fonts
//! (`lv_font_fmt_txt`, uncompressed), the format `lv_font_conv` produces.

use std::fmt::Write as _;
use std::path::Path;

use lvglab_model::project::FontAsset;

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("font `{name}` has no TTF/OTF file assigned")]
    NoFile { name: String },
    #[error("cannot read font file `{path}`: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("cannot parse font file `{path}`: {message}")]
    Parse { path: String, message: String },
    #[error("invalid character range `{0}`")]
    Range(String),
}

/// Parses `0x20-0x7E, 0xB0, 65-70` plus the literal `symbols` into a sorted
/// list of code points.
pub fn code_points(ranges: &str, symbols: &str) -> Result<Vec<u32>, FontError> {
    let parse = |s: &str| -> Result<u32, FontError> {
        let s = s.trim();
        let parsed = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            Some(hex) => u32::from_str_radix(hex, 16),
            None => s.parse(),
        };
        parsed.map_err(|_| FontError::Range(s.to_owned()))
    };
    let mut points = Vec::new();
    for part in ranges.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((start, end)) => {
                let (start, end) = (parse(start)?, parse(end)?);
                if start > end || end - start > 0x10000 {
                    return Err(FontError::Range(part.to_owned()));
                }
                points.extend(start..=end);
            }
            None => points.push(parse(part)?),
        }
    }
    points.extend(symbols.chars().filter(|c| !c.is_whitespace()).map(u32::from));
    points.sort_unstable();
    points.dedup();
    Ok(points)
}

/// Vertical metrics of an LVGL font: `line_height` and `base_line` (distance
/// from the bottom of the line to the baseline).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineMetrics {
    pub line_height: i32,
    pub base_line: i32,
}

/// Like `lv_font_conv` (and so SquareLine Studio), the line spans the
/// bitmaps of the glyphs the font contains: from the highest top to the
/// lowest bottom, with the baseline above that bottom. Fonts without any
/// visible glyph fall back to the font's ascent and descent.
fn line_metrics(font: &fontdue::Font, size: f32, points: &[u32]) -> LineMetrics {
    let (mut top, mut bottom) = (i32::MIN, i32::MAX);
    for ch in points.iter().filter_map(|&cp| char::from_u32(cp)) {
        if font.lookup_glyph_index(ch) == 0 {
            continue;
        }
        let m = font.metrics(ch, size);
        if m.width == 0 || m.height == 0 {
            continue;
        }
        let y = m.ymin.clamp(-128, 127);
        top = top.max(y + m.height.min(255) as i32);
        bottom = bottom.min(y);
    }
    if top > bottom {
        return LineMetrics { line_height: top - bottom, base_line: -bottom };
    }
    let (ascent, descent) = font.horizontal_line_metrics(size).map_or((size, 0.0), |m| (m.ascent, m.descent));
    LineMetrics { line_height: (ascent - descent).ceil() as i32, base_line: (-descent).ceil() as i32 }
}

/// The metrics LVGL will use for a TTF/OTF with the given character ranges
/// at `size` px, e.g. for an editor preview that matches the device.
pub fn metrics_from_bytes(bytes: &[u8], size: u16, ranges: &str, symbols: &str) -> Option<LineMetrics> {
    let size = f32::from(size.max(1));
    let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings { scale: size, ..Default::default() }).ok()?;
    let points = code_points(ranges, symbols).ok()?;
    Some(line_metrics(&font, size, &points))
}

/// Ascent and descent (negative) of a TTF/OTF in fractions of the em, as
/// LVGL's converter uses them.
pub fn em_metrics(bytes: &[u8]) -> Option<(f32, f32)> {
    const PX: f32 = 1000.0;
    let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings { scale: PX, ..Default::default() }).ok()?;
    let m = font.horizontal_line_metrics(PX)?;
    Some((m.ascent / PX, m.descent / PX))
}

struct Glyph {
    bitmap_index: usize,
    adv_w: u32,
    box_w: usize,
    box_h: usize,
    ofs_x: i32,
    ofs_y: i32,
}

/// C source of a font. `symbol` names the `lv_font_t`, `include` is the
/// header that provides LVGL.
pub fn convert(
    font: &FontAsset,
    base_dir: Option<&Path>,
    symbol: &str,
    header: &str,
    include: &str,
) -> Result<String, FontError> {
    if font.path.trim().is_empty() {
        return Err(FontError::NoFile { name: font.name.clone() });
    }
    let path = lvglab_model::Project::resolve_path(base_dir, &font.path);
    let bytes = std::fs::read(&path).map_err(|source| FontError::Io { path: path.display().to_string(), source })?;
    let size = f32::from(font.size.max(1));
    let parsed = fontdue::Font::from_bytes(bytes, fontdue::FontSettings { scale: size, ..Default::default() })
        .map_err(|message| FontError::Parse { path: path.display().to_string(), message: message.to_owned() })?;
    let bpp = match font.bpp {
        1 | 2 | 4 | 8 => font.bpp,
        _ => 4,
    };
    let points = code_points(&font.ranges, &font.symbols)?;
    Ok(render(&font.name, symbol, &parsed, size, bpp, &points, header, include))
}

#[allow(clippy::too_many_arguments)]
fn render(
    name: &str,
    symbol: &str,
    font: &fontdue::Font,
    size: f32,
    bpp: u8,
    points: &[u32],
    header: &str,
    include: &str,
) -> String {
    let mut bitmap: Vec<u8> = Vec::new();
    let mut glyphs = vec![Glyph { bitmap_index: 0, adv_w: 0, box_w: 0, box_h: 0, ofs_x: 0, ofs_y: 0 }];
    let mut included = Vec::new();
    for &cp in points {
        let Some(ch) = char::from_u32(cp) else { continue };
        if font.lookup_glyph_index(ch) == 0 && ch != ' ' {
            continue;
        }
        let (metrics, coverage) = font.rasterize(ch, size);
        let bitmap_index = bitmap.len();
        bitmap.extend(pack(&coverage, bpp));
        glyphs.push(Glyph {
            bitmap_index,
            adv_w: (metrics.advance_width * 16.0).round().max(0.0) as u32,
            box_w: metrics.width.min(255),
            box_h: metrics.height.min(255),
            ofs_x: metrics.xmin.clamp(-128, 127),
            ofs_y: metrics.ymin.clamp(-128, 127),
        });
        included.push(cp);
    }
    let LineMetrics { line_height, base_line } = line_metrics(font, size, &included);

    let mut out = String::new();
    out.push_str(header);
    let _ =
        writeln!(out, "// Font {}: {size} px, {bpp} bpp, {} glyphs\n", crate::template::comment(name), included.len());
    let _ = write!(
        out,
        "#include \"{include}\"\n\n#ifndef LV_ATTRIBUTE_LARGE_CONST\n    #define LV_ATTRIBUTE_LARGE_CONST\n#endif\n\n"
    );
    out.push_str("static LV_ATTRIBUTE_LARGE_CONST const uint8_t glyph_bitmap[] = {\n");
    if bitmap.is_empty() {
        bitmap.push(0);
    }
    for chunk in bitmap.chunks(16) {
        let bytes: Vec<String> = chunk.iter().map(|b| format!("0x{b:02x}")).collect();
        let _ = writeln!(out, "    {},", bytes.join(", "));
    }
    out.push_str("};\n\nstatic const lv_font_fmt_txt_glyph_dsc_t glyph_dsc[] = {\n");
    for g in &glyphs {
        let _ = writeln!(
            out,
            "    {{.bitmap_index = {}, .adv_w = {}, .box_w = {}, .box_h = {}, .ofs_x = {}, .ofs_y = {}}},",
            g.bitmap_index, g.adv_w, g.box_w, g.box_h, g.ofs_x, g.ofs_y
        );
    }
    out.push_str("};\n\nstatic const lv_font_fmt_txt_cmap_t cmaps[] = {\n");
    let runs = runs(&included);
    for (start, len, first_glyph) in &runs {
        let _ = writeln!(
            out,
            "    {{.range_start = {start}, .range_length = {len}, .glyph_id_start = {first_glyph}, .unicode_list = NULL, .glyph_id_ofs_list = NULL, .list_length = 0, .type = LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY}},"
        );
    }
    if runs.is_empty() {
        out.push_str("    {.range_start = 32, .range_length = 1, .glyph_id_start = 0, .unicode_list = NULL, .glyph_id_ofs_list = NULL, .list_length = 0, .type = LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY},\n");
    }
    out.push_str("};\n\n#if LVGL_VERSION_MAJOR == 8\nstatic lv_font_fmt_txt_glyph_cache_t cache;\n#endif\n\n");
    out.push_str("static const lv_font_fmt_txt_dsc_t font_dsc = {\n");
    out.push_str("    .glyph_bitmap = glyph_bitmap,\n    .glyph_dsc = glyph_dsc,\n    .cmaps = cmaps,\n");
    out.push_str("    .kern_dsc = NULL,\n    .kern_scale = 0,\n");
    let _ = writeln!(out, "    .cmap_num = {},", runs.len().max(1));
    let _ = writeln!(out, "    .bpp = {bpp},");
    out.push_str("    .kern_classes = 0,\n    .bitmap_format = 0,\n#if LVGL_VERSION_MAJOR == 8\n    .cache = &cache\n#endif\n};\n\n");
    let _ = writeln!(out, "const lv_font_t {symbol} = {{");
    out.push_str(
        "    .get_glyph_dsc = lv_font_get_glyph_dsc_fmt_txt,\n    .get_glyph_bitmap = lv_font_get_bitmap_fmt_txt,\n",
    );
    let _ = writeln!(out, "    .line_height = {line_height},");
    let _ = writeln!(out, "    .base_line = {base_line},");
    out.push_str("    .subpx = LV_FONT_SUBPX_NONE,\n    .underline_position = -1,\n    .underline_thickness = 1,\n");
    out.push_str("    .dsc = &font_dsc,\n    .fallback = NULL,\n    .user_data = NULL,\n};\n");
    out
}

/// Packs 8 bit coverage values into `bpp` bits per pixel, MSB first,
/// continuous across rows (LVGL's plain bitmap format).
fn pack(coverage: &[u8], bpp: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(coverage.len() * bpp as usize / 8 + 1);
    let mut acc: u16 = 0;
    let mut bits = 0;
    for &c in coverage {
        acc = (acc << bpp) | u16::from(c >> (8 - bpp));
        bits += bpp;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if bits > 0 {
        out.push((acc << (8 - bits)) as u8);
    }
    out
}

/// Consecutive code point runs as `(start, length, first glyph id)`.
fn runs(points: &[u32]) -> Vec<(u32, u32, usize)> {
    let mut runs: Vec<(u32, u32, usize)> = Vec::new();
    for (i, &cp) in points.iter().enumerate() {
        match runs.last_mut() {
            Some((start, len, _)) if *start + *len == cp => *len += 1,
            _ => runs.push((cp, 1, i + 1)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ranges_and_symbols() {
        assert_eq!(code_points("0x41-0x43, 0x30", "°A").unwrap(), vec![0x30, 0x41, 0x42, 0x43, 0xB0]);
        assert!(code_points("zz", "").is_err());
        assert!(code_points("0x50-0x40", "").is_err());
    }

    #[test]
    fn packs_bits_msb_first() {
        assert_eq!(pack(&[0xFF, 0x00, 0x80], 4), vec![0xF0, 0x80]);
        assert_eq!(pack(&[0xFF; 8], 1), vec![0xFF]);
        assert_eq!(pack(&[0xC0, 0x40, 0x00], 2), vec![0xD0]);
    }

    #[test]
    fn groups_runs() {
        assert_eq!(runs(&[32, 33, 34, 176]), vec![(32, 3, 1), (176, 1, 4)]);
    }

    #[test]
    fn renders_a_system_font_if_available() {
        let candidates = [
            "C:/Windows/Fonts/arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ];
        let Some(path) = candidates.iter().find(|p| Path::new(p).exists()) else { return };
        let asset = FontAsset { name: "Test".into(), path: (*path).into(), size: 20, ..FontAsset::default() };
        let c = convert(&asset, None, "ui_font_Test", "// h\n", "ui.h").unwrap();
        assert!(c.contains("const lv_font_t ui_font_Test = {"));
        assert!(c.contains(".bpp = 4,"));
        assert!(c.contains(".range_start = 32, .range_length = 95"));
    }
}
