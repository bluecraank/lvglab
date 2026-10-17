//! Renders candidate icon glyphs with egui's built-in fonts into a PNG to see
//! which ones exist: `cargo run -p lvglab --example glyphs -- out.png`

use eframe::egui;

const CANDIDATES: &str = "↶ ↷ ↺ ↻ ⟲ ⟳ ⎌ ◀ ▶ ▸ ▾ ⏵ ⏷ ▢ ▣ ⬚ ◻ ◼ ■ □ ● ○ ◉ ◌ ◎ ◯ ⏺ ⏻ ☑ ☐ ✔ ✓ ✕ ✖ ❌ ⊶ ◠ ◡ ⌒ ▬ ━ ▭ ▾ ▼ ☰ ≡ ≣ ⌨ ▦ ▤ ▧ ⊞ ⊟ ⊡ ◫ ± ◍ ◔ ¶ T ╱ ⁄ / ⇄ ↔ 🗔 🖼 🖲 📈 📊 📅 🗓 🎞 💬 👁 🔒 🔓 🔘 🎚 🎛 🔢 🔤 🔠 📝 📋 🗂 📁 📂 💾 ➕ ➖ ✚ ⚙ 🛠 🧩 🌙 ☀ 🌓 🖌 🎨 🅰 🔲 🔳 ⬜ ⬛ 🟦 🧭 📐 📏 🕹 ⚪ ⚫ 🔴 🔵 💡 ⏱ ⌛ ⏳ 🔄 🔁 🗑 ✂ 📄 ⧉ ⎘ ⤢ ⛶ 🔍 🔎 🧲 # ⌗ 🏷 ⭐ ✨ 📦 🔗 ⬆ ⬇ ⬅ ➡";

fn main() -> eframe::Result {
    let out = std::env::args().nth(1).unwrap_or_else(|| "glyphs.png".into());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 520.0]),
        ..Default::default()
    };
    let mut frame = 0;
    eframe::run_simple_native("glyphs", options, move |ctx, _| {
        frame += 1;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for glyph in CANDIDATES.split(' ') {
                    ui.label(egui::RichText::new(glyph).size(24.0));
                    ui.label(egui::RichText::new(format!("{:X}", glyph.chars().next().unwrap() as u32)).size(9.0));
                }
            });
        });
        if frame == 5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, bytes).unwrap().save(&out).unwrap();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    })
}
