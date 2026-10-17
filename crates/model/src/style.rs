//! Local styles: the complete LVGL 8.3 style property set, parts and the
//! per-widget style table.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::color::Color;
use crate::coord::Coord;
use crate::flags::StateMask;
use crate::prop::{EnumOption, opt};

lv_enum! {
    /// `lv_part_t`.
    pub enum Part: "LV_PART_" {
        Main = "MAIN", "Main";
        Scrollbar = "SCROLLBAR", "Scrollbar";
        Indicator = "INDICATOR", "Indicator";
        Knob = "KNOB", "Knob";
        Selected = "SELECTED", "Selected";
        Items = "ITEMS", "Items";
        Ticks = "TICKS", "Ticks";
        Cursor = "CURSOR", "Cursor";
        TextareaPlaceholder = "TEXTAREA_PLACEHOLDER", "Placeholder";
    }
}

impl Part {
    /// Parses SquareLine's notation (`lv.PART.MAIN`, `lv.PART_TEXTAREA.PLACEHOLDER`).
    pub fn from_squareline(text: &str) -> Option<Self> {
        let normalized = text.trim().replace('.', "_").to_ascii_uppercase();
        Self::from_suffix(&normalized)
    }
}

/// Grouping used by the style editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StyleGroup {
    Size,
    Padding,
    Background,
    BackgroundImage,
    Border,
    Outline,
    Shadow,
    Text,
    Image,
    Line,
    Arc,
    Transform,
    Misc,
    Flex,
    Grid,
}

impl StyleGroup {
    pub const ALL: [Self; 15] = [
        Self::Background,
        Self::Border,
        Self::Text,
        Self::Padding,
        Self::Shadow,
        Self::Outline,
        Self::BackgroundImage,
        Self::Image,
        Self::Line,
        Self::Arc,
        Self::Size,
        Self::Transform,
        Self::Misc,
        Self::Flex,
        Self::Grid,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Size => "Size & position",
            Self::Padding => "Padding",
            Self::Background => "Background",
            Self::BackgroundImage => "Background image",
            Self::Border => "Border",
            Self::Outline => "Outline",
            Self::Shadow => "Shadow",
            Self::Text => "Text",
            Self::Image => "Image",
            Self::Line => "Line",
            Self::Arc => "Arc",
            Self::Transform => "Transform",
            Self::Misc => "Miscellaneous",
            Self::Flex => "Flex",
            Self::Grid => "Grid",
        }
    }
}

/// Value type of a style property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleKind {
    /// A coordinate that may use `lv_pct()` / `LV_SIZE_CONTENT`.
    Coord,
    Int {
        min: i32,
        max: i32,
    },
    /// Opacity 0..=255.
    Opa,
    Color,
    Bool,
    Enum(&'static [EnumOption]),
    Font,
    Image,
}

const PX: StyleKind = StyleKind::Int { min: -2000, max: 2000 };
const WIDTH_PX: StyleKind = StyleKind::Int { min: 0, max: 500 };
const STOP: StyleKind = StyleKind::Int { min: 0, max: 255 };
const ZOOM: StyleKind = StyleKind::Int { min: 0, max: 2560 };
const ANGLE: StyleKind = StyleKind::Int { min: -3600, max: 3600 };
const TIME: StyleKind = StyleKind::Int { min: 0, max: 60000 };
const GRID_INDEX: StyleKind = StyleKind::Int { min: 0, max: 64 };

const ALIGNS: &[EnumOption] = &[
    opt("LV_ALIGN_DEFAULT", "Default"),
    opt("LV_ALIGN_TOP_LEFT", "Top left"),
    opt("LV_ALIGN_TOP_MID", "Top middle"),
    opt("LV_ALIGN_TOP_RIGHT", "Top right"),
    opt("LV_ALIGN_BOTTOM_LEFT", "Bottom left"),
    opt("LV_ALIGN_BOTTOM_MID", "Bottom middle"),
    opt("LV_ALIGN_BOTTOM_RIGHT", "Bottom right"),
    opt("LV_ALIGN_LEFT_MID", "Left middle"),
    opt("LV_ALIGN_RIGHT_MID", "Right middle"),
    opt("LV_ALIGN_CENTER", "Center"),
];
const GRAD_DIRS: &[EnumOption] =
    &[opt("LV_GRAD_DIR_NONE", "None"), opt("LV_GRAD_DIR_VER", "Vertical"), opt("LV_GRAD_DIR_HOR", "Horizontal")];
const DITHER_MODES: &[EnumOption] =
    &[opt("LV_DITHER_NONE", "None"), opt("LV_DITHER_ORDERED", "Ordered"), opt("LV_DITHER_ERR_DIFF", "Error diffusion")];
const BORDER_SIDES: &[EnumOption] = &[
    opt("LV_BORDER_SIDE_FULL", "Full"),
    opt("LV_BORDER_SIDE_NONE", "None"),
    opt("LV_BORDER_SIDE_TOP", "Top"),
    opt("LV_BORDER_SIDE_BOTTOM", "Bottom"),
    opt("LV_BORDER_SIDE_LEFT", "Left"),
    opt("LV_BORDER_SIDE_RIGHT", "Right"),
    opt("LV_BORDER_SIDE_TOP | LV_BORDER_SIDE_BOTTOM", "Top + bottom"),
    opt("LV_BORDER_SIDE_LEFT | LV_BORDER_SIDE_RIGHT", "Left + right"),
    opt("LV_BORDER_SIDE_INTERNAL", "Internal"),
];
const TEXT_DECORS: &[EnumOption] = &[
    opt("LV_TEXT_DECOR_NONE", "None"),
    opt("LV_TEXT_DECOR_UNDERLINE", "Underline"),
    opt("LV_TEXT_DECOR_STRIKETHROUGH", "Strikethrough"),
];
pub(crate) const TEXT_ALIGNS: &[EnumOption] = &[
    opt("LV_TEXT_ALIGN_AUTO", "Auto"),
    opt("LV_TEXT_ALIGN_LEFT", "Left"),
    opt("LV_TEXT_ALIGN_CENTER", "Center"),
    opt("LV_TEXT_ALIGN_RIGHT", "Right"),
];
const BLEND_MODES: &[EnumOption] = &[
    opt("LV_BLEND_MODE_NORMAL", "Normal"),
    opt("LV_BLEND_MODE_ADDITIVE", "Additive"),
    opt("LV_BLEND_MODE_SUBTRACTIVE", "Subtractive"),
    opt("LV_BLEND_MODE_MULTIPLY", "Multiply"),
    opt("LV_BLEND_MODE_REPLACE", "Replace"),
];
const BASE_DIRS: &[EnumOption] = &[
    opt("LV_BASE_DIR_LTR", "Left to right"),
    opt("LV_BASE_DIR_RTL", "Right to left"),
    opt("LV_BASE_DIR_AUTO", "Auto"),
];
const FLEX_FLOWS: &[EnumOption] = &[
    opt("LV_FLEX_FLOW_ROW", "Row"),
    opt("LV_FLEX_FLOW_COLUMN", "Column"),
    opt("LV_FLEX_FLOW_ROW_WRAP", "Row, wrap"),
    opt("LV_FLEX_FLOW_ROW_REVERSE", "Row, reverse"),
    opt("LV_FLEX_FLOW_ROW_WRAP_REVERSE", "Row, wrap, reverse"),
    opt("LV_FLEX_FLOW_COLUMN_WRAP", "Column, wrap"),
    opt("LV_FLEX_FLOW_COLUMN_REVERSE", "Column, reverse"),
    opt("LV_FLEX_FLOW_COLUMN_WRAP_REVERSE", "Column, wrap, reverse"),
];
const FLEX_ALIGNS: &[EnumOption] = &[
    opt("LV_FLEX_ALIGN_START", "Start"),
    opt("LV_FLEX_ALIGN_END", "End"),
    opt("LV_FLEX_ALIGN_CENTER", "Center"),
    opt("LV_FLEX_ALIGN_SPACE_EVENLY", "Space evenly"),
    opt("LV_FLEX_ALIGN_SPACE_AROUND", "Space around"),
    opt("LV_FLEX_ALIGN_SPACE_BETWEEN", "Space between"),
];
const GRID_ALIGNS: &[EnumOption] = &[
    opt("LV_GRID_ALIGN_START", "Start"),
    opt("LV_GRID_ALIGN_CENTER", "Center"),
    opt("LV_GRID_ALIGN_END", "End"),
    opt("LV_GRID_ALIGN_STRETCH", "Stretch"),
    opt("LV_GRID_ALIGN_SPACE_EVENLY", "Space evenly"),
    opt("LV_GRID_ALIGN_SPACE_AROUND", "Space around"),
    opt("LV_GRID_ALIGN_SPACE_BETWEEN", "Space between"),
];

macro_rules! style_props {
    ($($variant:ident => $name:literal, $label:literal, $group:ident, $kind:expr;)+) => {
        /// Every style property of LVGL 8.3 that can be set as a value.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum StyleProp { $($variant),+ }

        impl StyleProp {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// LVGL name, used in `lv_obj_set_style_<name>()`.
            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),+ }
            }

            pub const fn label(self) -> &'static str {
                match self { $(Self::$variant => $label),+ }
            }

            pub const fn group(self) -> StyleGroup {
                match self { $(Self::$variant => StyleGroup::$group),+ }
            }

            pub const fn kind(self) -> StyleKind {
                match self { $(Self::$variant => $kind),+ }
            }
        }
    };
}

style_props! {
    Width => "width", "Width", Size, StyleKind::Coord;
    MinWidth => "min_width", "Min. width", Size, StyleKind::Coord;
    MaxWidth => "max_width", "Max. width", Size, StyleKind::Coord;
    Height => "height", "Height", Size, StyleKind::Coord;
    MinHeight => "min_height", "Min. height", Size, StyleKind::Coord;
    MaxHeight => "max_height", "Max. height", Size, StyleKind::Coord;
    X => "x", "X", Size, StyleKind::Coord;
    Y => "y", "Y", Size, StyleKind::Coord;
    Align => "align", "Align", Size, StyleKind::Enum(ALIGNS);
    TransformWidth => "transform_width", "Transform width", Transform, PX;
    TransformHeight => "transform_height", "Transform height", Transform, PX;
    TranslateX => "translate_x", "Translate X", Transform, StyleKind::Coord;
    TranslateY => "translate_y", "Translate Y", Transform, StyleKind::Coord;
    TransformZoom => "transform_zoom", "Zoom (256 = 100%)", Transform, ZOOM;
    TransformAngle => "transform_angle", "Angle (0.1°)", Transform, ANGLE;
    TransformPivotX => "transform_pivot_x", "Pivot X", Transform, PX;
    TransformPivotY => "transform_pivot_y", "Pivot Y", Transform, PX;
    PadTop => "pad_top", "Top", Padding, PX;
    PadBottom => "pad_bottom", "Bottom", Padding, PX;
    PadLeft => "pad_left", "Left", Padding, PX;
    PadRight => "pad_right", "Right", Padding, PX;
    PadRow => "pad_row", "Row gap", Padding, PX;
    PadColumn => "pad_column", "Column gap", Padding, PX;
    BgColor => "bg_color", "Color", Background, StyleKind::Color;
    BgOpa => "bg_opa", "Opacity", Background, StyleKind::Opa;
    BgGradColor => "bg_grad_color", "Gradient color", Background, StyleKind::Color;
    BgGradDir => "bg_grad_dir", "Gradient direction", Background, StyleKind::Enum(GRAD_DIRS);
    BgMainStop => "bg_main_stop", "Main stop", Background, STOP;
    BgGradStop => "bg_grad_stop", "Gradient stop", Background, STOP;
    BgDitherMode => "bg_dither_mode", "Dither mode", Background, StyleKind::Enum(DITHER_MODES);
    BgImgSrc => "bg_img_src", "Image", BackgroundImage, StyleKind::Image;
    BgImgOpa => "bg_img_opa", "Opacity", BackgroundImage, StyleKind::Opa;
    BgImgRecolor => "bg_img_recolor", "Recolor", BackgroundImage, StyleKind::Color;
    BgImgRecolorOpa => "bg_img_recolor_opa", "Recolor opacity", BackgroundImage, StyleKind::Opa;
    BgImgTiled => "bg_img_tiled", "Tiled", BackgroundImage, StyleKind::Bool;
    BorderColor => "border_color", "Color", Border, StyleKind::Color;
    BorderOpa => "border_opa", "Opacity", Border, StyleKind::Opa;
    BorderWidth => "border_width", "Width", Border, WIDTH_PX;
    BorderSide => "border_side", "Side", Border, StyleKind::Enum(BORDER_SIDES);
    BorderPost => "border_post", "Draw after children", Border, StyleKind::Bool;
    OutlineWidth => "outline_width", "Width", Outline, WIDTH_PX;
    OutlineColor => "outline_color", "Color", Outline, StyleKind::Color;
    OutlineOpa => "outline_opa", "Opacity", Outline, StyleKind::Opa;
    OutlinePad => "outline_pad", "Padding", Outline, PX;
    ShadowWidth => "shadow_width", "Width", Shadow, WIDTH_PX;
    ShadowOfsX => "shadow_ofs_x", "Offset X", Shadow, PX;
    ShadowOfsY => "shadow_ofs_y", "Offset Y", Shadow, PX;
    ShadowSpread => "shadow_spread", "Spread", Shadow, PX;
    ShadowColor => "shadow_color", "Color", Shadow, StyleKind::Color;
    ShadowOpa => "shadow_opa", "Opacity", Shadow, StyleKind::Opa;
    ImgOpa => "img_opa", "Opacity", Image, StyleKind::Opa;
    ImgRecolor => "img_recolor", "Recolor", Image, StyleKind::Color;
    ImgRecolorOpa => "img_recolor_opa", "Recolor opacity", Image, StyleKind::Opa;
    LineWidth => "line_width", "Width", Line, WIDTH_PX;
    LineDashWidth => "line_dash_width", "Dash width", Line, WIDTH_PX;
    LineDashGap => "line_dash_gap", "Dash gap", Line, WIDTH_PX;
    LineRounded => "line_rounded", "Rounded", Line, StyleKind::Bool;
    LineColor => "line_color", "Color", Line, StyleKind::Color;
    LineOpa => "line_opa", "Opacity", Line, StyleKind::Opa;
    ArcWidth => "arc_width", "Width", Arc, WIDTH_PX;
    ArcRounded => "arc_rounded", "Rounded", Arc, StyleKind::Bool;
    ArcColor => "arc_color", "Color", Arc, StyleKind::Color;
    ArcOpa => "arc_opa", "Opacity", Arc, StyleKind::Opa;
    ArcImgSrc => "arc_img_src", "Image", Arc, StyleKind::Image;
    TextColor => "text_color", "Color", Text, StyleKind::Color;
    TextOpa => "text_opa", "Opacity", Text, StyleKind::Opa;
    TextFont => "text_font", "Font", Text, StyleKind::Font;
    TextLetterSpace => "text_letter_space", "Letter spacing", Text, PX;
    TextLineSpace => "text_line_space", "Line spacing", Text, PX;
    TextDecor => "text_decor", "Decoration", Text, StyleKind::Enum(TEXT_DECORS);
    TextAlign => "text_align", "Alignment", Text, StyleKind::Enum(TEXT_ALIGNS);
    Radius => "radius", "Radius", Misc, StyleKind::Int { min: 0, max: 0x7FFF };
    ClipCorner => "clip_corner", "Clip corner", Misc, StyleKind::Bool;
    Opa => "opa", "Opacity", Misc, StyleKind::Opa;
    ColorFilterOpa => "color_filter_opa", "Color filter opacity", Misc, StyleKind::Opa;
    AnimTime => "anim_time", "Animation time (ms)", Misc, TIME;
    AnimSpeed => "anim_speed", "Animation speed", Misc, TIME;
    BlendMode => "blend_mode", "Blend mode", Misc, StyleKind::Enum(BLEND_MODES);
    BaseDir => "base_dir", "Base direction", Misc, StyleKind::Enum(BASE_DIRS);
    FlexFlow => "flex_flow", "Flow", Flex, StyleKind::Enum(FLEX_FLOWS);
    FlexMainPlace => "flex_main_place", "Main place", Flex, StyleKind::Enum(FLEX_ALIGNS);
    FlexCrossPlace => "flex_cross_place", "Cross place", Flex, StyleKind::Enum(FLEX_ALIGNS);
    FlexTrackPlace => "flex_track_place", "Track place", Flex, StyleKind::Enum(FLEX_ALIGNS);
    FlexGrow => "flex_grow", "Grow", Flex, StyleKind::Int { min: 0, max: 255 };
    GridCellColumnPos => "grid_cell_column_pos", "Column", Grid, GRID_INDEX;
    GridCellColumnSpan => "grid_cell_column_span", "Column span", Grid, GRID_INDEX;
    GridCellRowPos => "grid_cell_row_pos", "Row", Grid, GRID_INDEX;
    GridCellRowSpan => "grid_cell_row_span", "Row span", Grid, GRID_INDEX;
    GridCellXAlign => "grid_cell_x_align", "Cell X align", Grid, StyleKind::Enum(GRID_ALIGNS);
    GridCellYAlign => "grid_cell_y_align", "Cell Y align", Grid, StyleKind::Enum(GRID_ALIGNS);
    GridRowAlign => "grid_row_align", "Row align", Grid, StyleKind::Enum(GRID_ALIGNS);
    GridColumnAlign => "grid_column_align", "Column align", Grid, StyleKind::Enum(GRID_ALIGNS);
}

impl StyleProp {
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL.iter().copied().find(|p| p.name().eq_ignore_ascii_case(name))
    }

    /// Maps SquareLine's style names (`Bg_Color`, `Bg_Radius`, ...).
    pub fn from_squareline(name: &str) -> Option<Self> {
        let lower = name.trim().to_ascii_lowercase().replace(' ', "_");
        let alias = match lower.as_str() {
            "bg_gradiens_color" | "bg_gradient_color" => "bg_grad_color",
            "gradient_direction" | "bg_gradient_direction" => "bg_grad_dir",
            "bg_radius" => "radius",
            "bg_image" => "bg_img_src",
            "bg_image_opa" => "bg_img_opa",
            "bg_image_recolor" => "bg_img_recolor",
            "bg_image_recolor_opa" => "bg_img_recolor_opa",
            "bg_image_tiled" => "bg_img_tiled",
            "arc_image" => "arc_img_src",
            "image_opa" => "img_opa",
            "image_recolor" => "img_recolor",
            "image_recolor_opa" => "img_recolor_opa",
            "transform_rotation" => "transform_angle",
            "transform_scale" => "transform_zoom",
            "letter_space" => "text_letter_space",
            "line_space" => "text_line_space",
            other => other,
        };
        Self::from_name(alias)
    }

    /// SquareLine stores some related properties as one value with several
    /// components (`Padding` = `[left, right, top, bottom]`, ...).
    pub fn squareline_composite(name: &str) -> Option<&'static [Self]> {
        let lower = name.trim().to_ascii_lowercase().replace(' ', "_");
        Some(match lower.as_str() {
            "padding" => &[Self::PadLeft, Self::PadRight, Self::PadTop, Self::PadBottom],
            "padding_rowcol" => &[Self::PadRow, Self::PadColumn],
            "shadow_offset" => &[Self::ShadowOfsX, Self::ShadowOfsY],
            "shadow_params" => &[Self::ShadowWidth, Self::ShadowSpread],
            "transform_pivot" => &[Self::TransformPivotX, Self::TransformPivotY],
            "outline_params" => &[Self::OutlineWidth, Self::OutlinePad],
            "text_spacing" => &[Self::TextLetterSpace, Self::TextLineSpace],
            _ => return None,
        })
    }

    /// Text properties are inherited from the parent if not set locally.
    pub const fn is_inherited(self) -> bool {
        matches!(
            self,
            Self::TextColor
                | Self::TextOpa
                | Self::TextFont
                | Self::TextLetterSpace
                | Self::TextLineSpace
                | Self::TextDecor
                | Self::TextAlign
                | Self::BaseDir
        )
    }

    /// The opacity property belonging to a color property. SquareLine stores
    /// the opacity as the alpha channel of the color.
    pub const fn opa_companion(self) -> Option<Self> {
        match self {
            Self::BgColor => Some(Self::BgOpa),
            Self::BorderColor => Some(Self::BorderOpa),
            Self::OutlineColor => Some(Self::OutlineOpa),
            Self::ShadowColor => Some(Self::ShadowOpa),
            Self::TextColor => Some(Self::TextOpa),
            Self::LineColor => Some(Self::LineOpa),
            Self::ArcColor => Some(Self::ArcOpa),
            Self::ImgRecolor => Some(Self::ImgRecolorOpa),
            Self::BgImgRecolor => Some(Self::BgImgRecolorOpa),
            _ => None,
        }
    }

    /// The value a freshly added property starts with in the editor.
    pub fn initial_value(self) -> StyleValue {
        match self.kind() {
            StyleKind::Coord => StyleValue::Coord(Coord::px(0)),
            StyleKind::Int { min, .. } => StyleValue::Int(min.max(0)),
            StyleKind::Opa => StyleValue::Int(255),
            StyleKind::Color => StyleValue::Color(Color::WHITE),
            StyleKind::Bool => StyleValue::Bool(false),
            StyleKind::Enum(options) => StyleValue::Enum(options[0].c.to_owned()),
            StyleKind::Font => StyleValue::Font("montserrat_14".to_owned()),
            StyleKind::Image => StyleValue::Image(String::new()),
        }
    }
}

impl Serialize for StyleProp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for StyleProp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::from_name(&text).ok_or_else(|| serde::de::Error::custom(format!("unknown style property `{text}`")))
    }
}

/// Value of a style property.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleValue {
    Int(i32),
    Coord(Coord),
    Color(Color),
    Bool(bool),
    /// The C constant (or expression) of the selected option.
    Enum(String),
    /// Font name: a built-in Montserrat font (`montserrat_14`) or a font asset.
    Font(String),
    /// Image asset name.
    Image(String),
}

impl StyleValue {
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Coord(c) => Some(c.value),
            Self::Bool(b) => Some(i32::from(*b)),
            _ => None,
        }
    }

    pub fn as_coord(&self) -> Option<Coord> {
        match self {
            Self::Coord(c) => Some(*c),
            Self::Int(v) => Some(Coord::px(*v)),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<Color> {
        match self {
            Self::Color(c) => Some(*c),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Int(v) => Some(*v != 0),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Enum(s) | Self::Font(s) | Self::Image(s) => Some(s),
            _ => None,
        }
    }
}

/// Built-in Montserrat sizes shipped with LVGL 8.
pub const MONTSERRAT_SIZES: &[u16] =
    &[8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48];

/// `(size, line_height)` of LVGL 8.3's built-in Montserrat fonts.
const MONTSERRAT_LINE_HEIGHTS: &[(u16, u16)] = &[
    (8, 10),
    (10, 11),
    (12, 15),
    (14, 16),
    (16, 18),
    (18, 21),
    (20, 22),
    (22, 24),
    (24, 27),
    (26, 29),
    (28, 30),
    (30, 33),
    (32, 35),
    (34, 38),
    (36, 40),
    (38, 41),
    (40, 44),
    (42, 46),
    (44, 49),
    (46, 50),
    (48, 52),
];

/// Line height LVGL uses for a built-in Montserrat size.
pub fn builtin_font_line_height(size: u16) -> Option<u16> {
    MONTSERRAT_LINE_HEIGHTS.iter().find(|(s, _)| *s == size).map(|(_, h)| *h)
}

/// Returns the size if `font` names a built-in Montserrat font
/// (`montserrat_14` or `lv_font_montserrat_14`).
pub fn builtin_font_size(font: &str) -> Option<u16> {
    let rest = font.trim().trim_start_matches('&');
    let rest = rest.strip_prefix("lv_font_").unwrap_or(rest);
    let size: u16 = rest.strip_prefix("montserrat_")?.parse().ok()?;
    MONTSERRAT_SIZES.contains(&size).then_some(size)
}

/// All local style properties set for one part/state combination.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleBlock {
    /// Key of a style slot of the widget (see `catalog::StyleSlot`).
    pub slot: String,
    #[serde(default)]
    pub state: StateMask,
    pub props: BTreeMap<StyleProp, StyleValue>,
}

/// The local styles of a widget.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Styles(Vec<StyleBlock>);

impl Styles {
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|b| b.props.is_empty())
    }

    /// Non-empty blocks in a stable order (slot order of the widget is
    /// applied by the code generator).
    pub fn blocks(&self) -> impl Iterator<Item = &StyleBlock> {
        self.0.iter().filter(|b| !b.props.is_empty())
    }

    pub fn blocks_mut(&mut self) -> impl Iterator<Item = &mut StyleBlock> {
        self.0.iter_mut()
    }

    pub fn block(&self, slot: &str, state: StateMask) -> Option<&StyleBlock> {
        self.0.iter().find(|b| b.slot == slot && b.state == state)
    }

    pub fn get(&self, slot: &str, state: StateMask, prop: StyleProp) -> Option<&StyleValue> {
        self.block(slot, state).and_then(|b| b.props.get(&prop))
    }

    pub fn set(&mut self, slot: &str, state: StateMask, prop: StyleProp, value: StyleValue) {
        self.block_mut(slot, state).props.insert(prop, value);
    }

    pub fn remove(&mut self, slot: &str, state: StateMask, prop: StyleProp) -> Option<StyleValue> {
        let removed = self.0.iter_mut().find(|b| b.slot == slot && b.state == state)?.props.remove(&prop);
        self.0.retain(|b| !b.props.is_empty());
        removed
    }

    fn block_mut(&mut self, slot: &str, state: StateMask) -> &mut StyleBlock {
        let index = match self.0.iter().position(|b| b.slot == slot && b.state == state) {
            Some(index) => index,
            None => {
                self.0.push(StyleBlock { slot: slot.to_owned(), state, props: BTreeMap::new() });
                self.0.len() - 1
            }
        };
        &mut self.0[index]
    }

    /// The value LVGL would use for `prop` of `slot` while the object is in
    /// state `active`: the most specific matching selector wins.
    pub fn effective(&self, slot: &str, active: StateMask, prop: StyleProp) -> Option<&StyleValue> {
        self.0
            .iter()
            .filter(|b| b.slot == slot && b.state.matches(active))
            .filter_map(|b| b.props.get(&prop).map(|v| (b.state, v)))
            .max_by_key(|(state, _)| (state.specificity(), state.0))
            .map(|(_, v)| v)
    }

    /// States that have at least one property for `slot`.
    pub fn states_of(&self, slot: &str) -> Vec<StateMask> {
        let mut states: Vec<_> = self.blocks().filter(|b| b.slot == slot).map(|b| b.state).collect();
        states.sort();
        states.dedup();
        states
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::ObjState;

    #[test]
    fn style_names_are_unique_and_parse() {
        for prop in StyleProp::ALL {
            assert_eq!(StyleProp::from_name(prop.name()), Some(*prop));
        }
        let mut names: Vec<_> = StyleProp::ALL.iter().map(|p| p.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), StyleProp::ALL.len());
    }

    #[test]
    fn squareline_aliases() {
        assert_eq!(StyleProp::from_squareline("Bg_Color"), Some(StyleProp::BgColor));
        assert_eq!(StyleProp::from_squareline("Bg_Radius"), Some(StyleProp::Radius));
        assert_eq!(StyleProp::from_squareline("Text_Font"), Some(StyleProp::TextFont));
        assert_eq!(StyleProp::from_squareline("Border width"), Some(StyleProp::BorderWidth));
        assert_eq!(StyleProp::from_squareline("Bg_gradiens_Color"), Some(StyleProp::BgGradColor));
        assert_eq!(StyleProp::from_squareline("Gradient direction"), Some(StyleProp::BgGradDir));
        assert_eq!(StyleProp::squareline_composite("Padding").map(<[_]>::len), Some(4));
        assert_eq!(Part::from_squareline("lv.PART.MAIN"), Some(Part::Main));
        assert_eq!(Part::from_squareline(" lv.PART.SELECTED"), Some(Part::Selected));
        assert_eq!(Part::from_squareline("lv.PART_TEXTAREA.PLACEHOLDER"), Some(Part::TextareaPlaceholder));
    }

    #[test]
    fn effective_prefers_specific_state() {
        let mut styles = Styles::default();
        let checked = StateMask::DEFAULT.with(ObjState::Checked);
        styles.set("main", StateMask::DEFAULT, StyleProp::BgOpa, StyleValue::Int(10));
        styles.set("main", checked, StyleProp::BgOpa, StyleValue::Int(20));
        assert_eq!(styles.effective("main", StateMask::DEFAULT, StyleProp::BgOpa), Some(&StyleValue::Int(10)));
        assert_eq!(styles.effective("main", checked, StyleProp::BgOpa), Some(&StyleValue::Int(20)));
        assert_eq!(styles.effective("knob", checked, StyleProp::BgOpa), None);
        styles.remove("main", checked, StyleProp::BgOpa);
        assert_eq!(styles.states_of("main"), vec![StateMask::DEFAULT]);
    }

    #[test]
    fn builtin_fonts() {
        assert_eq!(builtin_font_size("montserrat_14"), Some(14));
        assert_eq!(builtin_font_size("&lv_font_montserrat_48"), Some(48));
        assert_eq!(builtin_font_size("montserrat_15"), None);
        assert_eq!(builtin_font_size("FontCBI40"), None);
    }
}
