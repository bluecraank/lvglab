//! Prints the preview layout of a widget subtree (debugging aid):
//! `cargo run -p lvglab-codegen --example layout_dump -- <file.spj|.lvglab> <widget>`

use lvglab_model::layout::{Layout, Measure};
use lvglab_model::theme::{StyleResolver, Theme};

/// Approximates text with 0.55 em per character.
struct RoughMeasure;

impl Measure for RoughMeasure {
    fn text_size(&self, text: &str, font: &str, _: i32, _: i32, _: Option<i32>) -> (i32, i32) {
        let size = font.chars().filter(char::is_ascii_digit).collect::<String>().parse().unwrap_or(14);
        let lines = text.lines().count().max(1) as i32;
        let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as i32;
        (longest * size * 55 / 100, lines * size * 12 / 10)
    }
    fn line_height(&self, font: &str) -> i32 {
        self.text_size("x", font, 0, 0, None).1
    }
    fn image_size(&self, _: &str) -> Option<(i32, i32)> {
        Some((64, 64))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(file), Some(name)) = (args.next(), args.next()) else {
        return Err("usage: layout_dump <file> <widget>".into());
    };
    let path = std::path::Path::new(&file);
    let project = if path.extension().is_some_and(|e| e == "spj") {
        lvglab_spj::import_file(path)?.project
    } else {
        lvglab_model::Project::load(path)?
    };
    let target = project.all_widgets().into_iter().find(|w| w.name == name).ok_or("widget not found")?.id;
    let screen = project.screen_of(target).ok_or("no screen")?;
    let theme = Theme::new(&project.theme, &project.display);
    let resolver = StyleResolver::new(&theme);
    let (w, h) = (project.display.width as i32, project.display.height as i32);
    let layout = Layout::compute(screen, w, h, &resolver, &RoughMeasure, Some(target));
    let root = layout.nodes.iter().position(|n| n.widget.id == target).ok_or("not laid out")?;
    for (i, node) in layout.nodes.iter().enumerate() {
        let path = layout.path(i);
        if let Some(depth) = path.iter().position(|w| w.id == target) {
            let _ = root;
            println!(
                "{}{} {:?} rect={:?} clip={:?} hidden={}",
                "  ".repeat(path.len() - 1 - depth),
                node.widget.name,
                node.widget.kind,
                node.rect,
                node.clip,
                node.hidden
            );
        }
    }
    Ok(())
}
