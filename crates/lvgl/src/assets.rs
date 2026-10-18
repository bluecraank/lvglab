//! Images of the export for the preview. The export stores pixels as 16 bit
//! RGB565 for the device; the preview's LVGL draws with 32 bits, so the data
//! is expanded into `LV_IMG_CF_TRUE_COLOR[_ALPHA]` descriptors of that format.

/// An image in the 32 bit format of the preview, with its `lv_img_dsc_t`.
pub struct Image {
    pub name: String,
    /// `lv_img_dsc_t`: header (4 bytes), data size (4), data pointer (8).
    pub descriptor: Box<[u8; 16]>,
    pub pixels: Vec<u8>,
}

const CF_TRUE_COLOR: u32 = 4;
const CF_TRUE_COLOR_ALPHA: u32 = 5;

/// Reads the images of the generated C files (`ui_img_*.c`).
pub fn parse_images(sources: &[&str], swapped: bool) -> Vec<Image> {
    let mut images = Vec::new();
    for source in sources {
        let Some(descriptor) = source.find("const lv_img_dsc_t ") else { continue };
        let head = &source[descriptor + "const lv_img_dsc_t ".len()..];
        let name: String = head.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let number = |key: &str| -> Option<u32> {
            let at = head.find(key)? + key.len();
            let digits: String = head[at..].trim_start().chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        };
        let (Some(width), Some(height)) = (number(".header.w ="), number(".header.h =")) else { continue };
        let alpha = head.contains("LV_IMG_CF_TRUE_COLOR_ALPHA");
        let Some(data_at) = source.find(&format!("{name}_data[] = {{")) else { continue };
        let body = &source[data_at..];
        let Some(end) = body.find("};") else { continue };
        let bytes: Vec<u8> = body[body.find('{').unwrap_or(0) + 1..end]
            .split(',')
            .filter_map(|t| u8::from_str_radix(t.trim().trim_start_matches("0x"), 16).ok())
            .collect();
        let stride = if alpha { 3 } else { 2 };
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for chunk in bytes.chunks_exact(stride).take(width as usize * height as usize) {
            let value = if swapped {
                u16::from_be_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_le_bytes([chunk[0], chunk[1]])
            };
            let (r5, g6, b5) = ((value >> 11) & 31, (value >> 5) & 63, value & 31);
            let expand = |v: u16, bits: u32| ((v << (8 - bits)) | (v >> (2 * bits - 8))) as u8;
            pixels.extend_from_slice(&[
                expand(b5, 5),
                expand(g6, 6),
                expand(r5, 5),
                if alpha { chunk[2] } else { 255 },
            ]);
        }
        if pixels.len() != width as usize * height as usize * 4 {
            continue;
        }
        let cf = if alpha { CF_TRUE_COLOR_ALPHA } else { CF_TRUE_COLOR };
        let header = cf | (width << 10) | (height << 21);
        let mut descriptor = Box::new([0_u8; 16]);
        descriptor[..4].copy_from_slice(&header.to_le_bytes());
        descriptor[4..8].copy_from_slice(&(pixels.len() as u32).to_le_bytes());
        images.push(Image { name, descriptor, pixels });
    }
    images
}

impl Image {
    /// Fills in the data pointer; call once the image sits at its final place.
    pub fn address(&mut self) -> usize {
        let pointer = self.pixels.as_ptr() as u64;
        self.descriptor[8..16].copy_from_slice(&pointer.to_le_bytes());
        self.descriptor.as_ptr() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_pixels_are_expanded_to_32_bit() {
        let source = "static const uint8_t ui_img_a_data[] = {\n    0x1F, 0x00, 0xFF, 0xE0, 0x07, 0x80,\n};\n\
            const lv_img_dsc_t ui_img_a = {\n    .header.w = 2,\n    .header.h = 1,\n    .header.cf = LV_IMG_CF_TRUE_COLOR_ALPHA,\n};\n";
        let mut images = parse_images(&[source], false);
        assert_eq!(images.len(), 1);
        // Blue with full alpha, then green with half alpha (B, G, R, A).
        assert_eq!(images[0].pixels, [255, 0, 0, 255, 0, 255, 0, 128]);
        let address = images[0].address();
        assert_ne!(address, 0);
        let header = u32::from_le_bytes(images[0].descriptor[..4].try_into().unwrap());
        assert_eq!((header & 31, (header >> 10) & 2047, header >> 21), (5, 2, 1));
    }
}
