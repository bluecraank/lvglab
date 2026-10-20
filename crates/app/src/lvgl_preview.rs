//! The preview rendered by LVGL itself (LVGL 8): the export of the screen runs
//! against the library that is compiled into the editor and the picture it
//! draws replaces the editor's own rendering on the canvas.

use std::path::Path;

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use lvglab_model::{Project, WidgetId};

/// The last rendered screen. It is rendered again when the project, the
/// screen or the folder of the project changes.
#[derive(Default)]
pub struct LvglPreview {
    key: Option<(Project, usize)>,
    texture: Option<TextureHandle>,
    /// Why the last render failed; the editor draws the screen itself then.
    pub error: Option<String>,
    /// Statements of the export that LVGL could not run (diagnostics).
    pub skipped: Vec<String>,
    /// A hint when LVGL drew fewer objects than the screen has widgets.
    pub missing: Option<String>,
    /// When LVGL last drew; edits in quick succession (dragging a color) are drawn at most every `THROTTLE`.
    last_render: Option<std::time::Instant>,
}

const THROTTLE: std::time::Duration = std::time::Duration::from_millis(40);

impl LvglPreview {
    #[cfg(test)]
    pub fn has_picture(&self) -> bool {
        self.texture.is_some()
    }

    /// The picture of `screen`, or `None` if the editor has to draw it.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        project: &Project,
        screen: WidgetId,
        dir: Option<&Path>,
    ) -> Option<TextureHandle> {
        let Some(index) = project.screens.iter().position(|s| s.id == screen) else {
            self.error = Some("components are drawn by the editor".to_owned());
            return None;
        };
        let same_screen = self.key.as_ref().is_some_and(|(_, i)| *i == index);
        if self.key.as_ref().is_none_or(|(p, i)| *i != index || p != project) {
            // While the screen is being edited, the last picture stays until the next slot.
            if let (true, Some(last), Some(_)) = (same_screen, self.last_render, &self.texture) {
                let elapsed = last.elapsed();
                if elapsed < THROTTLE {
                    ctx.request_repaint_after(THROTTLE - elapsed);
                    return self.texture.clone();
                }
            }
            self.last_render = Some(std::time::Instant::now());
            self.key = Some((project.clone(), index));
            match lvglab_lvgl::render_screen(project, index, dir) {
                Ok(frame) => {
                    let image =
                        ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
                    match &mut self.texture {
                        Some(texture) => texture.set(image, TextureOptions::NEAREST),
                        None => self.texture = Some(ctx.load_texture("lvgl_preview", image, TextureOptions::NEAREST)),
                    }
                    self.missing = (frame.created < frame.expected).then(|| {
                        format!("LVGL created {} of {} objects of this screen", frame.created, frame.expected)
                    });
                    self.skipped = frame.skipped;
                    self.error = None;
                }
                Err(err) => {
                    self.texture = None;
                    self.error = Some(err);
                }
            }
        }
        self.texture.clone()
    }
}
