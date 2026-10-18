//! Exports every screen as a PNG: the canvas shows one screen after the
//! other without overlays, the window is captured and the display area is
//! cropped and scaled to the resolution of the display. Round displays get
//! a transparent corner.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Rect, UserData, ViewportCommand};
use image::RgbaImage;
use lvglab_model::WidgetId;
use lvglab_model::project::DisplayShape;

use crate::app::Studio;
use crate::i18n::{tr, trf};

/// Frames to wait after a screen change until it is painted and the
/// canvas is fitted.
const SETTLE_FRAMES: u32 = 6;
/// A screenshot that never arrives must not stall the export.
const TIMEOUT_FRAMES: u32 = 120;

/// Marks the screenshots of this export.
struct ExportShot;

pub struct ShotJob {
    dir: PathBuf,
    screens: Vec<(WidgetId, String)>,
    index: usize,
    waited: u32,
    pending: bool,
    saved: Vec<PathBuf>,
    errors: Vec<String>,
    /// What the editor showed before, to restore afterwards.
    before: (WidgetId, Option<WidgetId>, bool),
    /// Exit when done (command line).
    pub exit_when_done: bool,
}

impl ShotJob {
    pub fn new(studio: &Studio, dir: PathBuf, exit_when_done: bool) -> Self {
        let screens = studio.doc.project.screens.iter().map(|s| (s.id, s.name.clone())).collect();
        Self {
            dir,
            screens,
            index: 0,
            waited: 0,
            pending: false,
            saved: Vec::new(),
            errors: Vec::new(),
            before: (studio.screen, studio.selected, studio.canvas.overview),
            exit_when_done,
        }
    }
}

/// File name of a screen: letters, digits, `_` and `-` only.
pub fn file_name(name: &str, taken: &[PathBuf]) -> String {
    let stem: String =
        name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let stem = if stem.is_empty() { "screen".to_owned() } else { stem };
    let exists =
        |candidate: &str| taken.iter().any(|p| p.file_name().is_some_and(|f| f.eq_ignore_ascii_case(candidate)));
    let mut candidate = format!("{stem}.png");
    let mut n = 2;
    while exists(&candidate) {
        candidate = format!("{stem}_{n}.png");
        n += 1;
    }
    candidate
}

/// The display part of a window capture, at the resolution of the display.
/// `crop` is the display area in points.
pub fn display_image(
    window: &RgbaImage,
    crop: Rect,
    scale: f32,
    size: (u32, u32),
    shape: DisplayShape,
) -> Option<RgbaImage> {
    let x = (crop.left() * scale).round().max(0.0) as u32;
    let y = (crop.top() * scale).round().max(0.0) as u32;
    let w = ((crop.width() * scale).round() as u32).min(window.width().saturating_sub(x));
    let h = ((crop.height() * scale).round() as u32).min(window.height().saturating_sub(y));
    if w == 0 || h == 0 {
        return None;
    }
    let cropped = image::imageops::crop_imm(window, x, y, w, h).to_image();
    let mut image = if (w, h) == size {
        cropped
    } else {
        image::imageops::resize(&cropped, size.0, size.1, image::imageops::FilterType::Lanczos3)
    };
    if shape == DisplayShape::Round {
        // The area outside the circle is not part of the display.
        let (cx, cy) = (size.0 as f32 / 2.0, size.1 as f32 / 2.0);
        let radius = size.0.min(size.1) as f32 / 2.0;
        for (px, py, pixel) in image.enumerate_pixels_mut() {
            let distance = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
            let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
            pixel.0[3] = (f32::from(pixel.0[3]) * coverage).round() as u8;
        }
    }
    Some(image)
}

impl Studio {
    /// Starts the export of all screens into `dir`.
    pub fn start_screenshots(&mut self, dir: PathBuf, exit_when_done: bool) {
        if self.shots.is_some() || self.simulation.is_some() {
            return;
        }
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.show_error(tr("Screenshots failed"), err.to_string());
            return;
        }
        self.shots = Some(ShotJob::new(self, dir, exit_when_done));
        self.canvas.overview = false;
        self.canvas.fit_pending = true;
        self.canvas.auto_fit = true;
        self.select(None);
    }
}

/// Advances the export by one frame.
pub fn step(ctx: &egui::Context, studio: &mut Studio) {
    let Some(mut job) = studio.shots.take() else { return };
    ctx.request_repaint();
    let Some(&(id, ref name)) = job.screens.get(job.index) else {
        finish(ctx, studio, job);
        return;
    };
    if studio.screen != id {
        studio.screen = id;
        studio.canvas.fit_pending = true;
        job.waited = 0;
        job.pending = false;
    }
    job.waited += 1;
    let display = studio.canvas.display_points;
    let shot = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, user_data, .. }
                if user_data.data.as_ref().is_some_and(|d| d.is::<ExportShot>()) =>
            {
                Some(image.clone())
            }
            _ => None,
        })
    });
    if let Some(image) = shot {
        let [width, height] = image.size;
        let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
        let project = &studio.doc.project;
        let size = (project.display.width, project.display.height);
        let window = RgbaImage::from_raw(width as u32, height as u32, bytes);
        let result = window
            .and_then(|w| display_image(&w, display, ctx.pixels_per_point(), size, project.display.shape))
            .ok_or_else(|| "the display was not visible".to_owned())
            .and_then(|image| {
                let path = job.dir.join(file_name(name, &job.saved));
                image.save(&path).map(|()| path).map_err(|e| e.to_string())
            });
        match result {
            Ok(path) => job.saved.push(path),
            Err(err) => job.errors.push(format!("{name}: {err}")),
        }
        job.index += 1;
        job.pending = false;
        job.waited = 0;
    } else if !job.pending && job.waited >= SETTLE_FRAMES && display.width() > 1.0 {
        job.pending = true;
        ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(ExportShot)));
    } else if job.pending && job.waited >= TIMEOUT_FRAMES {
        job.errors.push(format!("{name}: no screenshot arrived"));
        job.index += 1;
        job.pending = false;
        job.waited = 0;
    }
    studio.shots = Some(job);
}

fn finish(ctx: &egui::Context, studio: &mut Studio, job: ShotJob) {
    let (screen, selected, overview) = job.before;
    studio.screen = screen;
    studio.select(selected);
    studio.canvas.overview = overview;
    studio.canvas.fit_pending = true;
    let count = job.saved.len();
    if job.errors.is_empty() {
        studio.notify(trf("{} screenshots saved to {}", &[&count, &job.dir.display()]));
        if studio.settings.open_export_folder {
            crate::commands::open_folder(&job.dir);
        }
    } else {
        studio.report = Some(crate::app::Report {
            tone: crate::modal::Tone::Warning,
            title: tr("Screenshots").to_owned(),
            message: trf("{} screenshots saved to {}; these failed:", &[&count, &job.dir.display()]),
            lines: job.errors,
        });
    }
    if job.exit_when_done {
        studio.exit_requested = true;
        ctx.request_repaint();
    }
}

/// Where the screenshots go by default: next to the project.
pub fn default_dir(project_dir: Option<&Path>, name: &str) -> PathBuf {
    project_dir.unwrap_or_else(|| Path::new(".")).join(format!("{name}_screenshots"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe_and_unique() {
        assert_eq!(file_name("Main 01/äb", &[]), "Main_01_äb.png");
        assert_eq!(file_name("", &[]), "screen.png");
        let taken = vec![PathBuf::from("out/Main.png"), PathBuf::from("out/main_2.png")];
        assert_eq!(file_name("Main", &taken), "Main_3.png", "names that differ only in case clash too");
    }

    #[test]
    fn the_display_is_cropped_scaled_and_masked() {
        let window = RgbaImage::from_pixel(200, 200, image::Rgba([10, 20, 30, 255]));
        // A display of 40 × 40 points at 2 pixels per point is 80 px; the device shows 20 px.
        let crop = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(40.0, 40.0));
        let square = display_image(&window, crop, 2.0, (20, 20), DisplayShape::Rectangle).unwrap();
        assert_eq!(square.dimensions(), (20, 20));
        assert_eq!(square.get_pixel(0, 0).0, [10, 20, 30, 255]);
        let round = display_image(&window, crop, 2.0, (20, 20), DisplayShape::Round).unwrap();
        assert_eq!(round.get_pixel(0, 0).0[3], 0, "the corner is transparent");
        assert_eq!(round.get_pixel(10, 10).0[3], 255, "the middle is opaque");
        assert!(
            display_image(
                &window,
                Rect::from_min_size(egui::pos2(500.0, 500.0), egui::vec2(10.0, 10.0)),
                1.0,
                (4, 4),
                DisplayShape::Rectangle
            )
            .is_none()
        );
    }
}
