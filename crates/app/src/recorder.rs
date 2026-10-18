//! Records the simulated display into an animated GIF: the canvas is
//! captured with egui screenshots, cropped to the display and scaled to
//! its native resolution.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, Rect, UserData, ViewportCommand};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, RgbaImage};

/// Frames per second of the recording.
const FPS: f64 = 15.0;
/// Recordings stop on their own after this time.
pub const MAX_SECONDS: f64 = 30.0;

/// Marks screenshots that belong to the recording.
struct RecordShot;

pub struct Recorder {
    frames: Vec<(RgbaImage, f64)>,
    started: f64,
    last_request: f64,
    pending: bool,
    /// Display area in points and the display resolution.
    crop: Rect,
    size: (u32, u32),
}

impl Recorder {
    pub fn new(now: f64) -> Self {
        Self {
            frames: Vec::new(),
            started: now,
            last_request: f64::NEG_INFINITY,
            pending: false,
            crop: Rect::NOTHING,
            size: (1, 1),
        }
    }

    pub fn seconds(&self, now: f64) -> f64 {
        now - self.started
    }

    pub fn is_full(&self, now: f64) -> bool {
        self.seconds(now) >= MAX_SECONDS
    }

    /// Asks for the next frame of the display area `crop` (points), shown
    /// at `size` pixels on the device.
    pub fn request(&mut self, ctx: &egui::Context, now: f64, crop: Rect, size: (u32, u32)) {
        self.crop = crop;
        self.size = size;
        // A screenshot that never arrives must not stop the recording.
        if self.pending && now - self.last_request > 1.0 {
            self.pending = false;
        }
        if !self.pending && now - self.last_request >= 1.0 / FPS {
            self.pending = true;
            self.last_request = now;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(RecordShot)));
        }
        ctx.request_repaint();
    }

    /// Picks up a screenshot taken for the recording.
    pub fn receive(&mut self, ctx: &egui::Context) {
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, user_data, .. }
                    if user_data.data.as_ref().is_some_and(|d| d.is::<RecordShot>()) =>
                {
                    Some(image.clone())
                }
                _ => None,
            })
        });
        let Some(image) = shot else { return };
        self.pending = false;
        let now = ctx.input(|i| i.time);
        let scale = ctx.pixels_per_point();
        let [width, height] = image.size;
        let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
        let Some(full) = RgbaImage::from_raw(width as u32, height as u32, bytes) else { return };
        let x = (self.crop.left() * scale).round().max(0.0) as u32;
        let y = (self.crop.top() * scale).round().max(0.0) as u32;
        let w = ((self.crop.width() * scale).round() as u32).min(full.width().saturating_sub(x));
        let h = ((self.crop.height() * scale).round() as u32).min(full.height().saturating_sub(y));
        if w == 0 || h == 0 {
            return;
        }
        let cropped = image::imageops::crop_imm(&full, x, y, w, h).to_image();
        let (tw, th) = self.size;
        let frame = if (w, h) == (tw, th) {
            cropped
        } else {
            image::imageops::resize(&cropped, tw, th, image::imageops::FilterType::Triangle)
        };
        self.frames.push((frame, now));
    }

    /// Encodes the GIF on a background thread; the receiver gets the
    /// result once it is written.
    pub fn finish(self, path: PathBuf) -> Receiver<Result<PathBuf, String>> {
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let _ = sender.send(encode(&self.frames, &path).map(|()| path));
        });
        receiver
    }
}

fn encode(frames: &[(RgbaImage, f64)], path: &std::path::Path) -> Result<(), String> {
    if frames.is_empty() {
        return Err("no frames were recorded".to_owned());
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = GifEncoder::new_with_speed(std::io::BufWriter::new(file), 10);
    encoder.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
    for (index, (image, time)) in frames.iter().enumerate() {
        let next = frames.get(index + 1).map_or(time + 1.0 / FPS, |(_, t)| *t);
        let ms = ((next - time) * 1000.0).round().clamp(20.0, 1000.0) as u32;
        let frame = Frame::from_parts(image.clone(), 0, 0, Delay::from_numer_denom_ms(ms, 1));
        encoder.encode_frame(frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_an_animated_gif() {
        let frames: Vec<(RgbaImage, f64)> = (0..3)
            .map(|i| (RgbaImage::from_pixel(8, 8, image::Rgba([i * 80, 0, 0, 255])), f64::from(i) * 0.1))
            .collect();
        let path = std::env::temp_dir().join(format!("lvglab-record-{}.gif", std::process::id()));
        encode(&frames, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"GIF89a"));
        std::fs::remove_file(path).ok();
        assert!(encode(&[], std::path::Path::new("unused.gif")).is_err());
    }
}
