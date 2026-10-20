//! The dockable panels of the studio.

mod assistant;
mod events;
mod fonts;
pub mod images;
mod layers;
mod palette;
mod parameters;
mod screens;
mod style;
mod themes;
mod timeline;

pub use fonts::relative as relative_path;
pub use timeline::ago;

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::app::Studio;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Panel {
    Canvas,
    Screens,
    Layers,
    Widgets,
    Parameters,
    Themes,
    Style,
    Events,
    Timeline,
    Fonts,
    Images,
    Assistant,
}

impl Panel {
    pub const ALL: [Self; 12] = [
        Self::Canvas,
        Self::Screens,
        Self::Layers,
        Self::Widgets,
        Self::Parameters,
        Self::Themes,
        Self::Style,
        Self::Events,
        Self::Timeline,
        Self::Fonts,
        Self::Images,
        Self::Assistant,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Canvas => "Canvas",
            Self::Screens => "Screens",
            Self::Layers => "Layers",
            Self::Widgets => "Widgets",
            Self::Parameters => "Parameters",
            Self::Themes => "Themes",
            Self::Style => "Style",
            Self::Events => "Events",
            Self::Timeline => "Timeline",
            Self::Fonts => "Fonts",
            Self::Images => "Assets",
            Self::Assistant => "AI assistant",
        }
    }
}

pub fn show(panel: Panel, ui: &mut egui::Ui, studio: &mut Studio) {
    match panel {
        Panel::Canvas => crate::canvas::show(ui, studio),
        Panel::Screens => screens::show(ui, studio),
        Panel::Layers => layers::show(ui, studio),
        Panel::Widgets => palette::show(ui, studio),
        Panel::Parameters => parameters::show(ui, studio),
        Panel::Themes => themes::show(ui, studio),
        Panel::Style => style::show(ui, studio),
        Panel::Events => events::show(ui, studio),
        Panel::Timeline => timeline::show(ui, studio),
        Panel::Fonts => fonts::show(ui, studio),
        Panel::Images => images::show(ui, studio),
        Panel::Assistant => assistant::show(ui, studio),
    }
}

/// Header shared by the panels that edit the selected widget.
fn selection_header(ui: &mut egui::Ui, studio: &Studio) -> bool {
    match studio.selected_widget() {
        Some(widget) => {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(widget.def().icon).size(16.0));
                ui.label(egui::RichText::new(&widget.name).strong());
                crate::widgets::muted(ui, crate::i18n::tr(widget.def().name));
            });
            ui.separator();
            true
        }
        None => {
            crate::widgets::empty_hint(ui, crate::i18n::tr("Select a widget on the canvas or in the layers."));
            false
        }
    }
}
