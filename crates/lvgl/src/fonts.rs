//! Fonts of the export (`ui_font_*.c`) for the preview. The generated C file
//! describes a bitmap font in LVGL's own format; here its tables are read and
//! rebuilt in memory with the layout of LVGL 8.3 (64 bit), so LVGL draws text
//! with them like on the device.

/// A font with the memory LVGL reads it from.
#[derive(Clone)]
pub struct Font {
    pub name: String,
    bitmap: Vec<u8>,
    glyphs: Vec<u8>,
    cmaps: Vec<u8>,
    /// `lv_font_fmt_txt_dsc_t`.
    dsc: Box<[u8; 48]>,
    /// `lv_font_fmt_txt_glyph_cache_t`.
    cache: Box<[u8; 8]>,
    /// `lv_font_t`.
    font: Box<[u8; 48]>,
    line_height: i16,
    base_line: i16,
    underline_position: i8,
    underline_thickness: i8,
    bpp: u16,
    cmap_count: u16,
}

/// The number after `.key =` in `text`.
fn number(text: &str, key: &str) -> Option<i64> {
    let at = text.find(key)? + key.len();
    let rest = text[at..].trim_start().strip_prefix('=')?.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn bytes_of(source: &str, marker: &str) -> Vec<u8> {
    let Some(at) = source.find(marker) else { return Vec::new() };
    let body = &source[at..];
    let (Some(open), Some(close)) = (body.find('{'), body.find("};")) else { return Vec::new() };
    body[open + 1..close]
        .split(',')
        .filter_map(|t| u8::from_str_radix(t.trim().trim_start_matches("0x"), 16).ok())
        .collect()
}

/// Reads the fonts of the generated C files.
pub fn parse_fonts(sources: &[&str]) -> Vec<Font> {
    let mut fonts = Vec::new();
    for source in sources {
        let Some(at) = source.find("const lv_font_t ") else { continue };
        let head = &source[at + "const lv_font_t ".len()..];
        let name: String = head.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let bitmap = bytes_of(source, "glyph_bitmap[] = {");
        let mut glyphs = Vec::new();
        let mut cmaps = Vec::new();
        let mut cmap_count = 0_u16;
        for line in source.lines() {
            let line = line.trim();
            if line.starts_with("{.bitmap_index") {
                let (Some(index), Some(adv), Some(w), Some(h), Some(x), Some(y)) = (
                    number(line, ".bitmap_index"),
                    number(line, ".adv_w"),
                    number(line, ".box_w"),
                    number(line, ".box_h"),
                    number(line, ".ofs_x"),
                    number(line, ".ofs_y"),
                ) else {
                    continue;
                };
                // bitmap_index : 20 bits, adv_w : 12 bits, then four bytes.
                let packed = (index as u32 & 0xF_FFFF) | ((adv as u32 & 0xFFF) << 20);
                glyphs.extend_from_slice(&packed.to_le_bytes());
                glyphs.extend_from_slice(&[w as u8, h as u8, x as i8 as u8, y as i8 as u8]);
            } else if line.starts_with("{.range_start") {
                let (Some(start), Some(length), Some(first)) =
                    (number(line, ".range_start"), number(line, ".range_length"), number(line, ".glyph_id_start"))
                else {
                    continue;
                };
                // range_start, range_length, glyph_id_start, two NULL lists, list_length 0, type TINY.
                let mut entry = [0_u8; 32];
                entry[0..4].copy_from_slice(&(start as u32).to_le_bytes());
                entry[4..6].copy_from_slice(&(length as u16).to_le_bytes());
                entry[6..8].copy_from_slice(&(first as u16).to_le_bytes());
                entry[26] = 2; // LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY
                cmaps.extend_from_slice(&entry);
                cmap_count += 1;
            }
        }
        let (Some(line_height), Some(base_line)) = (number(head, ".line_height"), number(head, ".base_line")) else {
            continue;
        };
        let bpp = number(source, ".bpp").unwrap_or(4) as u16;
        if bitmap.is_empty() || glyphs.is_empty() || cmaps.is_empty() {
            continue;
        }
        fonts.push(Font {
            name,
            bitmap,
            glyphs,
            cmaps,
            dsc: Box::new([0; 48]),
            cache: Box::new([0; 8]),
            font: Box::new([0; 48]),
            line_height: line_height as i16,
            base_line: base_line as i16,
            underline_position: number(head, ".underline_position").unwrap_or(0) as i8,
            underline_thickness: number(head, ".underline_thickness").unwrap_or(0) as i8,
            bpp,
            cmap_count,
        });
    }
    fonts
}

impl Font {
    /// Fills in the pointers and returns the address of the `lv_font_t`.
    /// `glyph_dsc` and `glyph_bitmap` are the addresses of LVGL's functions for this format.
    pub fn address(&mut self, glyph_dsc: usize, glyph_bitmap: usize) -> usize {
        let pointer = |p: *const u8| (p as u64).to_le_bytes();
        let dsc = &mut *self.dsc;
        dsc[0..8].copy_from_slice(&pointer(self.bitmap.as_ptr()));
        dsc[8..16].copy_from_slice(&pointer(self.glyphs.as_ptr()));
        dsc[16..24].copy_from_slice(&pointer(self.cmaps.as_ptr()));
        // kern_dsc (24..32) and kern_scale (32..34) stay zero.
        let flags = (self.cmap_count & 0x1FF) | ((self.bpp & 0xF) << 9);
        dsc[34..36].copy_from_slice(&flags.to_le_bytes());
        dsc[40..48].copy_from_slice(&pointer(self.cache.as_ptr()));
        let font = &mut *self.font;
        font[0..8].copy_from_slice(&(glyph_dsc as u64).to_le_bytes());
        font[8..16].copy_from_slice(&(glyph_bitmap as u64).to_le_bytes());
        font[16..18].copy_from_slice(&self.line_height.to_le_bytes());
        font[18..20].copy_from_slice(&self.base_line.to_le_bytes());
        font[20] = 0; // subpx: none
        font[21] = self.underline_position as u8;
        font[22] = self.underline_thickness as u8;
        font[24..32].copy_from_slice(&pointer(self.dsc.as_ptr()));
        self.font.as_ptr() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_font_is_read() {
        let source = "static const uint8_t glyph_bitmap[] = {\n    0x12, 0x34,\n};\n\
            static const lv_font_fmt_txt_glyph_dsc_t glyph_dsc[] = {\n    {.bitmap_index = 0, .adv_w = 0, .box_w = 0, .box_h = 0, .ofs_x = 0, .ofs_y = 0},\n    {.bitmap_index = 0, .adv_w = 107, .box_w = 3, .box_h = 18, .ofs_x = -2, .ofs_y = 1},\n};\n\
            static const lv_font_fmt_txt_cmap_t cmaps[] = {\n    {.range_start = 32, .range_length = 95, .glyph_id_start = 1, .unicode_list = NULL, .glyph_id_ofs_list = NULL, .list_length = 0, .type = LV_FONT_FMT_TXT_CMAP_FORMAT0_TINY},\n};\n\
            static const lv_font_fmt_txt_dsc_t font_dsc = {\n    .bpp = 4,\n};\n\
            const lv_font_t ui_font_Test = {\n    .line_height = 27,\n    .base_line = 6,\n    .underline_position = -1,\n    .underline_thickness = 1,\n};\n";
        let mut fonts = parse_fonts(&[source]);
        assert_eq!(fonts.len(), 1);
        let font = &mut fonts[0];
        assert_eq!(
            (font.name.as_str(), font.bitmap.len(), font.glyphs.len(), font.cmaps.len()),
            ("ui_font_Test", 2, 16, 32)
        );
        // The second glyph: adv_w 107 in the upper 12 bits, then 3, 18, -2, 1.
        assert_eq!(u32::from_le_bytes(font.glyphs[8..12].try_into().unwrap()) >> 20, 107);
        assert_eq!(&font.glyphs[12..16], &[3, 18, 0xFE, 1]);
        assert_ne!(font.address(1, 2), 0);
        assert_eq!(i16::from_le_bytes(font.font[16..18].try_into().unwrap()), 27);
    }
}
