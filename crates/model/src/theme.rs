//! An approximation of LVGL 8.3's default theme (`lv_theme_default`) and the
//! style resolution used by the editor preview: local style → theme →
//! inherited from the parent.

use crate::color::{Color, Palette};
use crate::coord::Coord;
use crate::flags::{ObjState, StateMask};
use crate::project::{DisplaySettings, ThemeSettings};
use crate::style::{StyleProp, StyleValue};
use crate::widget::{Widget, WidgetKind};

/// `LV_RADIUS_CIRCLE`.
pub const RADIUS_CIRCLE: i32 = 0x7FFF;
const DPI: i32 = 130;

/// `lv_dpx()` for LVGL's default DPI.
const fn dpx(n: i32) -> i32 {
    (n * DPI + 80) / 160
}

/// Concrete values of the default theme for one project.
#[derive(Clone, Debug)]
pub struct Theme {
    pub dark: bool,
    pub primary: Color,
    pub secondary: Color,
    pub screen: Color,
    pub card: Color,
    pub text: Color,
    pub grey: Color,
    pub pad_def: i32,
    pub pad_small: i32,
    pub pad_tiny: i32,
    pub radius: i32,
}

impl Theme {
    pub fn new(settings: &ThemeSettings, display: &DisplaySettings) -> Self {
        let width = display.width.max(display.height) as i32;
        let (small, large) = (width <= 320, width > 720);
        let pick = |s: i32, m: i32, l: i32| {
            dpx(if large {
                l
            } else if small {
                s
            } else {
                m
            })
        };
        let grey = Palette::Grey.main();
        let (screen, card, text, grey) = if settings.dark {
            (Color::from_hex(0x15171A), Color::from_hex(0x282B30), grey.lighten(242), Color::from_hex(0x2F3237))
        } else {
            (grey.lighten(230), Color::WHITE, grey.darken(200), grey.lighten(205))
        };
        Self {
            dark: settings.dark,
            primary: settings.primary.main(),
            secondary: settings.secondary.main(),
            screen,
            card,
            text,
            grey,
            pad_def: pick(16, 20, 24),
            pad_small: pick(10, 12, 14),
            pad_tiny: pick(2, 6, 8),
            radius: dpx(if large { 12 } else { 8 }),
        }
    }

    /// The theme's value for a property, if the theme sets one.
    pub fn value(&self, kind: WidgetKind, slot: &str, state: StateMask, prop: StyleProp) -> Option<StyleValue> {
        use StyleProp as P;
        let checked = state.contains(ObjState::Checked);
        let disabled = state.contains(ObjState::Disabled);
        let int = |v: i32| Some(StyleValue::Int(v));
        let color = |c: Color| Some(StyleValue::Color(c));
        let pad =
            |p: StyleProp, all: i32| matches!(p, P::PadTop | P::PadBottom | P::PadLeft | P::PadRight).then_some(all);

        let card = |prop: StyleProp| -> Option<StyleValue> {
            match prop {
                P::BgColor => color(self.card),
                P::BgOpa => int(255),
                P::BorderColor => color(self.grey),
                P::BorderWidth => int(2),
                P::Radius => int(self.radius),
                P::TextColor => color(self.text),
                P::PadRow | P::PadColumn => int(self.pad_small),
                p => pad(p, self.pad_def).and_then(int),
            }
        };
        let circle_bg = |c: Color, opa: i32, prop: StyleProp| match prop {
            P::BgColor => color(c),
            P::BgOpa => int(opa),
            P::Radius => int(RADIUS_CIRCLE),
            _ => None,
        };
        let arc = |c: Color, prop: StyleProp| match prop {
            P::ArcColor => color(c),
            P::ArcWidth => int(dpx(15)),
            P::ArcRounded => Some(StyleValue::Bool(true)),
            P::ArcOpa => int(255),
            _ => None,
        };
        let knob = |prop: StyleProp| match prop {
            p if pad(p, dpx(6)).is_some() => int(dpx(6)),
            p => circle_bg(self.primary, 255, p),
        };

        let value = match (kind, slot) {
            (WidgetKind::Screen, "main") => match prop {
                P::BgColor => color(self.screen),
                P::BgOpa => int(255),
                P::TextColor => color(self.text),
                P::PadRow | P::PadColumn => int(self.pad_small),
                _ => None,
            },
            (
                WidgetKind::Panel
                | WidgetKind::Dropdown
                | WidgetKind::Textarea
                | WidgetKind::Roller
                | WidgetKind::Chart
                | WidgetKind::Table
                | WidgetKind::Calendar
                | WidgetKind::Msgbox
                | WidgetKind::List
                | WidgetKind::Menu
                | WidgetKind::Win
                | WidgetKind::Spinbox
                | WidgetKind::BtnMatrix,
                "main",
            ) => match (kind, prop) {
                (WidgetKind::Dropdown | WidgetKind::Textarea | WidgetKind::Spinbox, p) if pad(p, 0).is_some() => {
                    int(self.pad_small)
                }
                (WidgetKind::Table | WidgetKind::List | WidgetKind::Menu | WidgetKind::Win, p)
                    if pad(p, 0).is_some() =>
                {
                    int(0)
                }
                (WidgetKind::BtnMatrix, p) if pad(p, 0).is_some() => int(self.pad_tiny),
                (WidgetKind::Roller, P::TextLineSpace) => int(dpx(10)),
                (WidgetKind::Roller | WidgetKind::Spinbox, P::TextAlign) => {
                    Some(StyleValue::Enum("LV_TEXT_ALIGN_CENTER".into()))
                }
                (_, p) => card(p),
            },
            (WidgetKind::Button, "main") => match prop {
                P::BgColor => color(if checked { self.secondary } else { self.primary }),
                P::BgOpa => int(255),
                P::Radius => int(self.radius),
                P::TextColor => color(Color::WHITE),
                P::PadLeft | P::PadRight => int(self.pad_def),
                P::PadTop | P::PadBottom => int(self.pad_small),
                P::PadRow | P::PadColumn => int(self.pad_small),
                P::ShadowWidth if !self.dark => int(dpx(3)),
                P::ShadowOfsY if !self.dark => int(dpx(4)),
                P::ShadowColor if !self.dark => color(self.grey.darken(150)),
                P::ShadowOpa if !self.dark => int(76),
                _ => None,
            },
            (WidgetKind::Slider | WidgetKind::Bar, "main") => circle_bg(self.primary, 51, prop),
            (WidgetKind::Slider | WidgetKind::Bar, "indicator") => circle_bg(self.primary, 255, prop),
            (WidgetKind::Slider | WidgetKind::Arc, "knob") => knob(prop),
            (WidgetKind::Arc | WidgetKind::Spinner, "main") => arc(self.grey, prop),
            (WidgetKind::Arc | WidgetKind::Spinner, "indicator") => arc(self.primary, prop),
            (WidgetKind::Switch, "main") => circle_bg(self.grey, 255, prop),
            (WidgetKind::Switch, "indicator") if checked => circle_bg(self.primary, 255, prop),
            (WidgetKind::Switch, "knob") => match prop {
                p if pad(p, 0).is_some() => int(-dpx(4)),
                p => circle_bg(Color::WHITE, 255, p),
            },
            (WidgetKind::Checkbox, "main") => match prop {
                P::PadColumn => int(dpx(10)),
                P::TextColor => color(self.text),
                _ => None,
            },
            (WidgetKind::Checkbox, "indicator") => match prop {
                P::BorderColor => color(self.primary),
                P::BorderWidth => int(2),
                P::Radius => int(dpx(6) / 2),
                P::BgColor => color(if checked { self.primary } else { self.card }),
                P::BgOpa => int(255),
                P::TextColor => color(Color::WHITE),
                p => pad(p, dpx(3)).and_then(int),
            },
            (WidgetKind::Roller, "selected") | (WidgetKind::Dropdown, "list_selected") => match prop {
                P::BgColor => color(self.primary),
                P::BgOpa => int(255),
                P::TextColor => color(Color::WHITE),
                _ => None,
            },
            (WidgetKind::Dropdown, "list_main") => card(prop),
            (WidgetKind::Textarea, "cursor") => match prop {
                P::BorderColor => color(self.primary),
                P::BorderWidth => int(dpx(2)),
                _ => None,
            },
            (WidgetKind::Textarea, "placeholder") => match prop {
                P::TextOpa => int(128),
                _ => None,
            },
            (WidgetKind::Keyboard, "main") => match prop {
                P::BgColor => color(if self.dark { self.screen } else { self.grey }),
                P::BgOpa => int(255),
                P::PadRow | P::PadColumn => int(dpx(3)),
                p => pad(p, dpx(3)).and_then(int),
            },
            (WidgetKind::Keyboard | WidgetKind::BtnMatrix, "items") => match prop {
                P::BgColor => color(if kind == WidgetKind::Keyboard { self.card } else { self.grey }),
                P::BgOpa => int(255),
                P::Radius => int(self.radius),
                P::TextColor => color(self.text),
                _ => None,
            },
            (WidgetKind::Led, "main") => match prop {
                P::Radius => int(RADIUS_CIRCLE),
                P::ShadowWidth => int(dpx(15)),
                P::ShadowSpread => int(dpx(5)),
                _ => None,
            },
            (WidgetKind::Meter, "main") => match prop {
                P::Radius => int(RADIUS_CIRCLE),
                p => card(p),
            },
            (WidgetKind::Table, "items") => match prop {
                P::BorderColor => color(self.grey),
                P::BorderWidth => int(1),
                P::TextColor => color(self.text),
                p => pad(p, self.pad_small).and_then(int),
            },
            (WidgetKind::Tabview, "main") => match prop {
                P::BgColor => color(self.screen),
                P::BgOpa => int(255),
                _ => None,
            },
            (WidgetKind::Tabview, "tab_bar") | (WidgetKind::Win, "header") => match prop {
                P::BgColor => color(self.card),
                P::BgOpa => int(255),
                P::TextColor => color(self.text),
                _ => None,
            },
            (WidgetKind::TabPage | WidgetKind::Tile, "main") => pad(prop, self.pad_def).and_then(int),
            _ => None,
        };
        if disabled && prop == P::BgColor {
            return value.map(|v| match v {
                StyleValue::Color(c) => StyleValue::Color(c.mix(self.grey, 128)),
                other => other,
            });
        }
        value
    }
}

/// Resolves the value LVGL would use for a style property.
pub struct StyleResolver<'a> {
    pub theme: &'a Theme,
}

impl<'a> StyleResolver<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    /// `path` lists the widget's ancestors from the screen down to the
    /// widget itself; `state` is the state the widget is displayed in.
    pub fn get(&self, path: &[&Widget], slot: &str, state: StateMask, prop: StyleProp) -> Option<StyleValue> {
        let (widget, ancestors) = path.split_last()?;
        if let Some(value) = widget.styles.effective(slot, state, prop) {
            return Some(value.clone());
        }
        if let Some(value) = self.theme.value(widget.kind, slot, state, prop) {
            return Some(value);
        }
        if prop.is_inherited() && !ancestors.is_empty() {
            let parent = ancestors[ancestors.len() - 1];
            return self.get(ancestors, "main", parent.state_mask(), prop);
        }
        None
    }

    pub fn int(&self, path: &[&Widget], slot: &str, state: StateMask, prop: StyleProp) -> i32 {
        self.get(path, slot, state, prop).and_then(|v| v.as_int()).unwrap_or(0)
    }

    pub fn coord(&self, path: &[&Widget], slot: &str, state: StateMask, prop: StyleProp) -> Option<Coord> {
        self.get(path, slot, state, prop).and_then(|v| v.as_coord())
    }

    pub fn color(&self, path: &[&Widget], slot: &str, state: StateMask, prop: StyleProp) -> Option<Color> {
        self.get(path, slot, state, prop).and_then(|v| v.as_color())
    }

    pub fn bool(&self, path: &[&Widget], slot: &str, state: StateMask, prop: StyleProp) -> bool {
        self.get(path, slot, state, prop).and_then(|v| v.as_bool()).unwrap_or(false)
    }

    /// Opacity with LVGL's default of `LV_OPA_COVER` for unset text/line/arc
    /// opacities and `LV_OPA_TRANSP` for backgrounds and borders.
    pub fn opa(&self, path: &[&Widget], slot: &str, state: StateMask, prop: StyleProp) -> u8 {
        let default = match prop {
            StyleProp::BgOpa | StyleProp::BgImgRecolorOpa | StyleProp::ImgRecolorOpa | StyleProp::ColorFilterOpa => 0,
            _ => 255,
        };
        self.get(path, slot, state, prop).and_then(|v| v.as_int()).map_or(default, |v| v.clamp(0, 255) as u8)
    }

    /// Font name used for text; defaults to `montserrat_14`.
    pub fn font(&self, path: &[&Widget], slot: &str, state: StateMask) -> String {
        self.get(path, slot, state, StyleProp::TextFont)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "montserrat_14".to_owned())
    }

    /// Padding as `(left, top, right, bottom)`.
    pub fn padding(&self, path: &[&Widget], slot: &str, state: StateMask) -> (i32, i32, i32, i32) {
        (
            self.int(path, slot, state, StyleProp::PadLeft),
            self.int(path, slot, state, StyleProp::PadTop),
            self.int(path, slot, state, StyleProp::PadRight),
            self.int(path, slot, state, StyleProp::PadBottom),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::WidgetId;

    fn theme(dark: bool) -> Theme {
        Theme::new(&ThemeSettings { dark, ..ThemeSettings::default() }, &DisplaySettings::default())
    }

    #[test]
    fn button_uses_primary_and_secondary_when_checked() {
        let theme = theme(true);
        let checked = StateMask::DEFAULT.with(ObjState::Checked);
        assert_eq!(
            theme.value(WidgetKind::Button, "main", StateMask::DEFAULT, StyleProp::BgColor),
            Some(StyleValue::Color(Palette::Blue.main()))
        );
        assert_eq!(
            theme.value(WidgetKind::Button, "main", checked, StyleProp::BgColor),
            Some(StyleValue::Color(Palette::Red.main()))
        );
    }

    #[test]
    fn text_color_is_inherited() {
        let theme = theme(false);
        let resolver = StyleResolver::new(&theme);
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
        screen.styles.set(
            "main",
            StateMask::DEFAULT,
            StyleProp::TextColor,
            StyleValue::Color(Color::from_hex(0x123456)),
        );
        let label = Widget::new(WidgetKind::Label, WidgetId(2), "L");
        let path = [&screen, &label];
        assert_eq!(
            resolver.color(&path, "main", StateMask::DEFAULT, StyleProp::TextColor),
            Some(Color::from_hex(0x123456))
        );
        assert_eq!(resolver.opa(&path, "main", StateMask::DEFAULT, StyleProp::BgOpa), 0);
    }

    #[test]
    fn local_style_overrides_theme() {
        let theme = theme(true);
        let resolver = StyleResolver::new(&theme);
        let mut button = Widget::new(WidgetKind::Button, WidgetId(1), "B");
        button.styles.set("main", StateMask::DEFAULT, StyleProp::Radius, StyleValue::Int(3));
        assert_eq!(resolver.int(&[&button], "main", StateMask::DEFAULT, StyleProp::Radius), 3);
    }
}
