//! Icons of the editor: the Phosphor icon font (MIT), one consistent set
//! instead of emoji and symbol characters.

use eframe::egui::FontDefinitions;
pub use egui_phosphor::regular::*;
use lvglab_model::WidgetKind;

/// egui's fonts plus the icon font; every `set_fonts` of the editor starts here.
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts
}

/// The icon of a widget type (palette, layers, parameters).
pub fn widget(kind: WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Screen => MONITOR,
        WidgetKind::Panel => SQUARE,
        WidgetKind::Container => SELECTION,
        WidgetKind::Label => TEXT_T,
        WidgetKind::Button => CURSOR_CLICK,
        WidgetKind::Image => IMAGE,
        WidgetKind::ImgButton => IMAGE_SQUARE,
        WidgetKind::Line => LINE_SEGMENT,
        WidgetKind::Arc => CIRCLE_NOTCH,
        WidgetKind::Bar => CHART_BAR,
        WidgetKind::Slider => SLIDERS_HORIZONTAL,
        WidgetKind::Switch => TOGGLE_RIGHT,
        WidgetKind::Checkbox => CHECK_SQUARE,
        WidgetKind::Dropdown => CARET_DOWN,
        WidgetKind::Roller => LIST,
        WidgetKind::Textarea => TEXTBOX,
        WidgetKind::Keyboard => KEYBOARD,
        WidgetKind::BtnMatrix => DOTS_NINE,
        WidgetKind::Spinbox => PLUS_MINUS,
        WidgetKind::Colorwheel => PALETTE,
        WidgetKind::Spinner => SPINNER,
        WidgetKind::Led => LIGHTBULB,
        WidgetKind::Meter => GAUGE,
        WidgetKind::Chart => CHART_LINE,
        WidgetKind::Table => TABLE,
        WidgetKind::Calendar => CALENDAR,
        WidgetKind::Canvas => PAINT_BRUSH,
        WidgetKind::AnimImg => FILM_STRIP,
        WidgetKind::Spangroup => PARAGRAPH,
        WidgetKind::Tabview => TABS,
        WidgetKind::TabPage => FILE,
        WidgetKind::Tileview => SQUARES_FOUR,
        WidgetKind::Tile => GRID_FOUR,
        WidgetKind::Win => APP_WINDOW,
        WidgetKind::List => LIST_BULLETS,
        WidgetKind::Menu => CARDS,
        WidgetKind::Msgbox => CHAT_TEXT,
        WidgetKind::Component => PACKAGE,
    }
}
