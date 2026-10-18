//! Draws the program icon: `cargo run -p lvglab --example icon -- crates/app/assets`
//! writes `icon.png` (256 px, used by the window) and `icon.ico` (all sizes,
//! embedded into the Windows executable).

use std::io::Write;

/// Distance to a rounded rectangle (negative inside).
fn rounded_box(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let (qx, qy) = ((px - cx).abs() - hw + r, (py - cy).abs() - hh + r);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Source over: paints `color` with `coverage` onto `base` (premultiplied-free RGBA).
fn over(base: [f32; 4], color: [f32; 3], coverage: f32) -> [f32; 4] {
    let a = coverage + base[3] * (1.0 - coverage);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let channel = |i: usize| (color[i] * coverage + base[i] * base[3] * (1.0 - coverage)) / a;
    [channel(0), channel(1), channel(2), a]
}

/// The icon in a 0..1 coordinate system; returns RGBA.
fn pixel(x: f32, y: f32) -> [f32; 4] {
    let violet = [0.42, 0.39, 1.0];
    let teal = [0.18, 0.77, 0.71];
    let white = [1.0, 1.0, 1.0];
    let orange = [1.0, 0.62, 0.0];

    let mut out = [0.0, 0.0, 0.0, 0.0];
    // Background: rounded square with a diagonal gradient.
    let inside = (-rounded_box(x, y, 0.5, 0.5, 0.5, 0.5, 0.22) * 256.0).clamp(0.0, 1.0);
    out = over(out, mix(violet, teal, (x + y) / 2.0), inside);
    if inside <= 0.0 {
        return out;
    }
    // A soft shadow below the letter.
    let shadow = |dx: f32, dy: f32, hw: f32, hh: f32, r: f32| {
        let d = rounded_box(x, y, dx, dy, hw, hh, r);
        (0.5 - d * 24.0).clamp(0.0, 1.0) * 0.18
    };
    let stem = (0.335, 0.455, 0.075, 0.275, 0.075); // center x, y, half width, half height, radius
    let foot = (0.52, 0.675, 0.245, 0.055, 0.055);
    let shade = shadow(stem.0 + 0.012, stem.1 + 0.018, stem.2, stem.3, stem.4).max(shadow(
        foot.0 + 0.012,
        foot.1 + 0.018,
        foot.2,
        foot.3,
        foot.4,
    ));
    out = over(out, [0.0, 0.0, 0.0], shade);
    // The letter L: a bar (stem) and a slider (foot) with an orange knob.
    for (cx, cy, hw, hh, r) in [stem, foot] {
        let c = (-rounded_box(x, y, cx, cy, hw, hh, r) * 256.0).clamp(0.0, 1.0);
        out = over(out, white, c);
    }
    let knob = (0.5 - (((x - 0.69).powi(2) + (y - 0.675).powi(2)).sqrt() - 0.105) * 256.0).clamp(0.0, 1.0);
    out = over(out, white, knob);
    let dot = (0.5 - (((x - 0.69).powi(2) + (y - 0.675).powi(2)).sqrt() - 0.068) * 256.0).clamp(0.0, 1.0);
    out = over(out, orange, dot);
    out
}

/// Renders `size` × `size` pixels with 4 × 4 supersampling.
fn render(size: u32) -> image::RgbaImage {
    const SAMPLES: u32 = 4;
    image::RgbaImage::from_fn(size, size, |px, py| {
        let (mut sum, mut alpha) = ([0.0_f32; 3], 0.0_f32);
        for sy in 0..SAMPLES {
            for sx in 0..SAMPLES {
                let x = (px as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / size as f32;
                let y = (py as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / size as f32;
                let p = pixel(x, y);
                for i in 0..3 {
                    sum[i] += p[i] * p[3];
                }
                alpha += p[3];
            }
        }
        let n = (SAMPLES * SAMPLES) as f32;
        let a = alpha / n;
        let channel = |i: usize| if alpha > 0.0 { (sum[i] / alpha * 255.0).round() as u8 } else { 0 };
        image::Rgba([channel(0), channel(1), channel(2), (a * 255.0).round() as u8])
    })
}

fn png(image: &image::RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).expect("PNG encoding");
    bytes
}

/// An ICO file with one PNG compressed entry per size.
fn ico(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|s| png(&render(*s))).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (size, data) in sizes.iter().zip(&images) {
        let byte = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[byte, byte, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for data in &images {
        out.write_all(data).expect("memory write");
    }
    out
}

fn main() -> std::io::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "crates/app/assets".into()));
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("icon.png"), png(&render(256)))?;
    std::fs::write(dir.join("icon.ico"), ico(&[16, 24, 32, 48, 64, 128, 256]))?;
    Ok(())
}
