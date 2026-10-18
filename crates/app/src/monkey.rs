//! Monkey test: drives the real editor UI without a window and feeds it
//! random clicks, drags, scrolling, shortcuts and text. Nothing may panic.
//! File dialogs, network requests and other programs are disabled by the
//! sandbox. `LVGLAB_FUZZ_SEED` repeats one run, `LVGLAB_FUZZ_ITERATIONS` sets
//! the number of runs.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2, pos2, vec2};
use lvglab_model::catalog::{self, Category};
use lvglab_model::project::LvglVersion;

use crate::app::{Studio, StudioApp};
use crate::settings::AppSettings;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn pos(&mut self, size: Vec2) -> Pos2 {
        pos2(self.below(size.x as usize) as f32, self.below(size.y as usize) as f32)
    }
}

const KEYS: &[Key] = &[
    Key::Enter,
    Key::Escape,
    Key::Delete,
    Key::Backspace,
    Key::Tab,
    Key::Space,
    Key::ArrowUp,
    Key::ArrowDown,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::F5,
    Key::Z,
    Key::Y,
    Key::C,
    Key::V,
    Key::X,
    Key::D,
    Key::A,
    Key::S,
    Key::N,
    Key::O,
    Key::I,
    Key::E,
    Key::Plus,
    Key::Minus,
    Key::Num0,
    Key::Home,
    Key::End,
    Key::PageDown,
];

const TEXTS: &[&str] = &["a", "Hallo", "äöü€😀", "0", "-1", "99999999999", "{v}", "\"", "\\", " ", "\n", "|", "1e9"];

/// A project with every widget kind, so all panels and previews get used.
fn project(rng: &mut Rng) -> lvglab_model::Project {
    let mut p = lvglab_model::Project::new("Monkey");
    p.lvgl_version = LvglVersion::ALL[rng.below(LvglVersion::ALL.len())];
    p.add_screen();
    let screen = p.screens[0].id;
    for def in catalog::ALL.iter().filter(|d| d.category != Category::Screen) {
        let widget = p.new_widget(def.kind);
        if p.insert(screen, usize::MAX, widget).is_err() {
            continue;
        }
    }
    p.remove_unsupported();
    p
}

fn app(rng: &mut Rng) -> StudioApp {
    let settings = AppSettings { tour_done: rng.chance(70), ..AppSettings::default() };
    let mut studio = Studio::new(settings);
    studio.doc.project = project(rng);
    studio.screen = studio.doc.project.screens[0].id;
    if rng.chance(20) {
        studio.start_page = Some(Vec::new());
    }
    if !studio.settings.tour_done {
        studio.tour = Some(rng.below(crate::tour::STEPS.len()));
    }
    StudioApp::for_test(studio)
}

/// Random input of one frame; presses are released in a later frame.
fn events(rng: &mut Rng, size: Vec2, pressed: &mut Option<(Pos2, PointerButton)>) -> Vec<Event> {
    let mut events = Vec::new();
    let modifiers = Modifiers { ctrl: rng.chance(15), shift: rng.chance(10), command: false, ..Modifiers::NONE };
    let modifiers = Modifiers { command: modifiers.ctrl, ..modifiers };
    match rng.below(10) {
        0..=3 => {
            let pos = rng.pos(size);
            events.push(Event::PointerMoved(pos));
            match pressed.take() {
                Some((_, button)) => {
                    events.push(Event::PointerButton { pos, button, pressed: false, modifiers });
                }
                None => {
                    let button = if rng.chance(85) { PointerButton::Primary } else { PointerButton::Secondary };
                    events.push(Event::PointerButton { pos, button, pressed: true, modifiers });
                    *pressed = Some((pos, button));
                }
            }
        }
        4 => {
            // Drag: move while pressed.
            if let Some((from, _)) = *pressed {
                let to = from + vec2(rng.below(200) as f32 - 100.0, rng.below(200) as f32 - 100.0);
                events.push(Event::PointerMoved(to));
            } else {
                events.push(Event::PointerMoved(rng.pos(size)));
            }
        }
        5 => events.push(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: vec2(0.0, rng.below(7) as f32 - 3.0),
            modifiers,
        }),
        6 | 7 => {
            let key = KEYS[rng.below(KEYS.len())];
            events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
            events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
        }
        8 => events.push(Event::Text(TEXTS[rng.below(TEXTS.len())].to_owned())),
        _ => events.push(Event::Paste(TEXTS[rng.below(TEXTS.len())].to_owned())),
    }
    events
}

/// Returns the number of timeline steps, i.e. how much the input changed.
fn run(seed: u64, frames: u64) -> usize {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let ctx = egui::Context::default();
    crate::visuals::install(&ctx);
    let mut app = app(&mut rng);
    let mut size = vec2(1600.0, 900.0);
    let mut pressed = None;
    for frame in 0..frames {
        if rng.chance(1) {
            size = vec2(400.0 + rng.below(1800) as f32, 300.0 + rng.below(1000) as f32);
        }
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(frame as f64 * 0.05),
            events: events(&mut rng, size, &mut pressed),
            ..RawInput::default()
        };
        let _ = ctx.run(input, |ctx| app.run_frame_for_test(ctx));
    }
    app.studio().timeline.entries().len()
}

#[test]
fn random_input_never_crashes_the_editor() {
    crate::sandbox::enable();
    let frames = 400;
    let seeds: Vec<u64> = match std::env::var("LVGLAB_FUZZ_SEED").ok().and_then(|v| v.parse().ok()) {
        Some(seed) => vec![seed],
        None => {
            let runs = std::env::var("LVGLAB_FUZZ_ITERATIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
            (0..runs).map(|i| 7_000 + i).collect()
        }
    };
    let mut steps = 0;
    for seed in &seeds {
        match catch_unwind(AssertUnwindSafe(|| run(*seed, frames))) {
            Ok(n) => steps += n,
            Err(panic) => {
                eprintln!("random_input_never_crashes_the_editor failed with LVGLAB_FUZZ_SEED={seed}");
                resume_unwind(panic);
            }
        }
    }
    // The input must reach the editor, not only empty space.
    assert!(steps > seeds.len(), "the random input changed nothing ({steps} timeline steps)");
}
