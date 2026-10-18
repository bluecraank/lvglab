//! The widget catalog: a declarative description of every LVGL 8.3 widget,
//! its properties, style parts and how it is created in C.
//!
//! Templates use `{parent}` for the parent object expression, `{obj}` for the
//! widget's C variable, `{w}`/`{h}` for its pixel size and `{key}` for the
//! formatted value of any property of the widget (see [`Emit`]).

use crate::coord::{Align, Coord};
use crate::flags::ObjFlag;
use crate::prop::{Emit, EnumOption, PropDef, PropDefault as D, PropKind as K, opt};
use crate::style::{Part, TEXT_ALIGNS};
use crate::widget::WidgetKind;

/// Palette category of a widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Screen,
    Basic,
    Controls,
    Visualization,
    Containers,
}

impl Category {
    pub const PALETTE: [Self; 4] = [Self::Basic, Self::Controls, Self::Visualization, Self::Containers];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Screen => "Screen",
            Self::Basic => "Basic",
            Self::Controls => "Controls",
            Self::Visualization => "Visualization",
            Self::Containers => "Containers",
        }
    }
}

/// A stylable part of a widget, optionally on a sub-object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleSlot {
    pub key: &'static str,
    pub label: &'static str,
    pub part: Part,
    /// Expression returning the sub-object to style, e.g. the list of a dropdown.
    pub getter: Option<&'static str>,
    /// Names used by SquareLine (`Style_<name>`), lower case.
    pub squareline: &'static [&'static str],
}

impl StyleSlot {
    const fn new(key: &'static str, label: &'static str, part: Part) -> Self {
        Self { key, label, part, getter: None, squareline: &[] }
    }

    const fn on(mut self, getter: &'static str) -> Self {
        self.getter = Some(getter);
        self
    }

    const fn sl(mut self, names: &'static [&'static str]) -> Self {
        self.squareline = names;
        self
    }
}

const MAIN: StyleSlot = StyleSlot::new("main", "Main", Part::Main);
const SCROLLBAR: StyleSlot = StyleSlot::new("scrollbar", "Scrollbar", Part::Scrollbar);
const INDICATOR: StyleSlot = StyleSlot::new("indicator", "Indicator", Part::Indicator);
const KNOB: StyleSlot = StyleSlot::new("knob", "Knob", Part::Knob);
const SELECTED: StyleSlot = StyleSlot::new("selected", "Selected", Part::Selected);
const ITEMS: StyleSlot = StyleSlot::new("items", "Items", Part::Items);
const TICKS: StyleSlot = StyleSlot::new("ticks", "Ticks", Part::Ticks);
const CURSOR: StyleSlot = StyleSlot::new("cursor", "Cursor", Part::Cursor);

/// Static description of a widget type.
#[derive(Debug)]
pub struct WidgetDef {
    pub kind: WidgetKind,
    pub name: &'static str,
    pub category: Category,
    /// Short glyph shown in the palette and the hierarchy.
    pub icon: &'static str,
    pub description: &'static str,
    /// C expression creating the object.
    pub create: &'static str,
    /// Statements right after creation.
    pub post_create: &'static str,
    /// Statements after all properties were applied.
    pub post_props: &'static str,
    /// Expression used as `{parent}` for children (e.g. a window's content).
    pub child_parent: Option<&'static str>,
    pub container: bool,
    /// Widgets that may only live inside a specific parent kind.
    pub required_parent: Option<WidgetKind>,
    /// Widgets that may only contain a specific child kind.
    pub required_child: Option<WidgetKind>,
    /// Position and size are managed by the parent (tabs, tiles).
    pub managed_geometry: bool,
    pub width: Coord,
    pub height: Coord,
    pub align: Align,
    pub slots: &'static [StyleSlot],
    pub props: &'static [PropDef],
    /// Flags added/cleared by the LVGL constructor relative to `lv_obj`.
    pub flags_on: &'static [ObjFlag],
    pub flags_off: &'static [ObjFlag],
    /// Flag overrides applied to new widgets created in the editor.
    pub initial_flags: &'static [(ObjFlag, bool)],
    /// Object type names used by SquareLine Studio.
    pub squareline: &'static [&'static str],
}

impl WidgetDef {
    pub fn prop(&self, key: &str) -> Option<&'static PropDef> {
        self.props.iter().find(|p| p.key == key)
    }

    pub fn slot(&self, key: &str) -> Option<&'static StyleSlot> {
        self.slots.iter().find(|s| s.key == key)
    }

    pub fn default_flag(&self, flag: ObjFlag) -> bool {
        if self.flags_on.contains(&flag) {
            return true;
        }
        if self.flags_off.contains(&flag) {
            return false;
        }
        ObjFlag::OBJ_DEFAULTS.contains(&flag)
    }

    pub fn accepts_child(&self, child: WidgetKind) -> bool {
        if !self.container {
            return false;
        }
        if let Some(required) = self.required_child {
            return child == required;
        }
        match child.def().required_parent {
            Some(parent) => parent == self.kind,
            None => child != WidgetKind::Screen,
        }
    }
}

const BASE: WidgetDef = WidgetDef {
    kind: WidgetKind::Panel,
    name: "",
    category: Category::Basic,
    icon: "🔲",
    description: "",
    create: "lv_obj_create({parent})",
    post_create: "",
    post_props: "",
    child_parent: None,
    container: false,
    required_parent: None,
    required_child: None,
    managed_geometry: false,
    width: Coord::px(100),
    height: Coord::px(50),
    align: Align::Center,
    slots: &[MAIN],
    props: &[],
    flags_on: &[],
    flags_off: &[],
    initial_flags: &[],
    squareline: &[],
};

// ---------------------------------------------------------------------------
// Shared option tables and property kinds
// ---------------------------------------------------------------------------

const VALUE: K = K::Int { min: -32768, max: 32767 };
const COUNT: K = K::Int { min: 0, max: 1000 };
const RANGE: K = K::Pair { min: -32768, max: 32767, first: "Min", second: "Max" };
const ANGLE_PAIR: K = K::Pair { min: 0, max: 360, first: "Start", second: "End" };
const TICK_PAIR: K = K::Pair { min: 0, max: 100, first: "Length", second: "Count" };

const LONG_MODES: &[EnumOption] = &[
    opt("LV_LABEL_LONG_WRAP", "Wrap"),
    opt("LV_LABEL_LONG_DOT", "Dots"),
    opt("LV_LABEL_LONG_SCROLL", "Scroll"),
    opt("LV_LABEL_LONG_SCROLL_CIRCULAR", "Scroll circular"),
    opt("LV_LABEL_LONG_CLIP", "Clip"),
];
const ARC_MODES: &[EnumOption] = &[
    opt("LV_ARC_MODE_NORMAL", "Normal"),
    opt("LV_ARC_MODE_SYMMETRICAL", "Symmetrical"),
    opt("LV_ARC_MODE_REVERSE", "Reverse"),
];
const BAR_MODES: &[EnumOption] = &[
    opt("LV_BAR_MODE_NORMAL", "Normal"),
    opt("LV_BAR_MODE_SYMMETRICAL", "Symmetrical"),
    opt("LV_BAR_MODE_RANGE", "Range"),
];
const SLIDER_MODES: &[EnumOption] = &[
    opt("LV_SLIDER_MODE_NORMAL", "Normal"),
    opt("LV_SLIDER_MODE_SYMMETRICAL", "Symmetrical"),
    opt("LV_SLIDER_MODE_RANGE", "Range"),
];
const LIST_DIRS: &[EnumOption] = &[
    opt("LV_DIR_BOTTOM", "Bottom"),
    opt("LV_DIR_TOP", "Top"),
    opt("LV_DIR_LEFT", "Left"),
    opt("LV_DIR_RIGHT", "Right"),
];
const ROLLER_MODES: &[EnumOption] =
    &[opt("LV_ROLLER_MODE_NORMAL", "Normal"), opt("LV_ROLLER_MODE_INFINITE", "Infinite")];
const KEYBOARD_MODES: &[EnumOption] = &[
    opt("LV_KEYBOARD_MODE_TEXT_LOWER", "Text lower"),
    opt("LV_KEYBOARD_MODE_TEXT_UPPER", "Text upper"),
    opt("LV_KEYBOARD_MODE_SPECIAL", "Special"),
    opt("LV_KEYBOARD_MODE_NUMBER", "Number"),
    opt("LV_KEYBOARD_MODE_USER_1", "User 1"),
    opt("LV_KEYBOARD_MODE_USER_2", "User 2"),
    opt("LV_KEYBOARD_MODE_USER_3", "User 3"),
    opt("LV_KEYBOARD_MODE_USER_4", "User 4"),
];
const COLORWHEEL_MODES: &[EnumOption] = &[
    opt("LV_COLORWHEEL_MODE_HUE", "Hue"),
    opt("LV_COLORWHEEL_MODE_SATURATION", "Saturation"),
    opt("LV_COLORWHEEL_MODE_VALUE", "Value"),
];
const CHART_TYPES: &[EnumOption] = &[
    opt("LV_CHART_TYPE_LINE", "Line"),
    opt("LV_CHART_TYPE_BAR", "Bar"),
    opt("LV_CHART_TYPE_SCATTER", "Scatter"),
    opt("LV_CHART_TYPE_NONE", "None"),
];
const CHART_UPDATE_MODES: &[EnumOption] =
    &[opt("LV_CHART_UPDATE_MODE_SHIFT", "Shift"), opt("LV_CHART_UPDATE_MODE_CIRCULAR", "Circular")];
const CALENDAR_HEADERS: &[EnumOption] = &[
    opt("", "None"),
    opt("lv_calendar_header_arrow_create({obj});", "Arrows"),
    opt("lv_calendar_header_dropdown_create({obj});", "Dropdowns"),
];
const SPAN_OVERFLOWS: &[EnumOption] =
    &[opt("LV_SPAN_OVERFLOW_CLIP", "Clip"), opt("LV_SPAN_OVERFLOW_ELLIPSIS", "Ellipsis")];
const SPAN_MODES: &[EnumOption] =
    &[opt("LV_SPAN_MODE_FIXED", "Fixed"), opt("LV_SPAN_MODE_EXPAND", "Expand"), opt("LV_SPAN_MODE_BREAK", "Break")];
const TILE_DIRS: &[EnumOption] = &[
    opt("LV_DIR_ALL", "All"),
    opt("LV_DIR_HOR", "Horizontal"),
    opt("LV_DIR_VER", "Vertical"),
    opt("LV_DIR_LEFT", "Left"),
    opt("LV_DIR_RIGHT", "Right"),
    opt("LV_DIR_TOP", "Top"),
    opt("LV_DIR_BOTTOM", "Bottom"),
    opt("LV_DIR_NONE", "None"),
];
const MENU_HEADER_MODES: &[EnumOption] = &[
    opt("LV_MENU_HEADER_TOP_FIXED", "Top, fixed"),
    opt("LV_MENU_HEADER_TOP_UNFIXED", "Top, unfixed"),
    opt("LV_MENU_HEADER_BOTTOM_FIXED", "Bottom, fixed"),
];
const MENU_BACK_BTN: &[EnumOption] =
    &[opt("LV_MENU_ROOT_BACK_BTN_DISABLED", "Disabled"), opt("LV_MENU_ROOT_BACK_BTN_ENABLED", "Enabled")];

// ---------------------------------------------------------------------------
// Widget definitions
// ---------------------------------------------------------------------------

const SCREEN: WidgetDef = WidgetDef {
    kind: WidgetKind::Screen,
    name: "Screen",
    category: Category::Screen,
    icon: "▣",
    description: "A top level screen (lv_obj_create(NULL)).",
    create: "lv_obj_create(NULL)",
    container: true,
    slots: &[MAIN, SCROLLBAR],
    props: &[PropDef::new("temporary", "Temporary", K::Bool, D::Bool(false))
        .help("Created when loaded and deleted when unloaded to save memory.")],
    width: Coord::pct(100),
    height: Coord::pct(100),
    align: Align::TopLeft,
    squareline: &["SCREEN"],
    ..BASE
};

const COMPONENT: WidgetDef = WidgetDef {
    kind: WidgetKind::Component,
    name: "Component",
    category: Category::Containers,
    icon: "📦",
    description: "An instance of one of the project's components.",
    create: "{component_create}({parent})",
    width: Coord::content(),
    height: Coord::content(),
    props: &[PropDef::new("component", "Component", K::Text, D::Text(""))
        .help("Name of the component; edit it under Screens → Components.")],
    ..BASE
};

const PANEL: WidgetDef = WidgetDef {
    kind: WidgetKind::Panel,
    name: "Panel",
    icon: "🔲",
    description: "A plain lv_obj with the theme's card style.",
    container: true,
    slots: &[MAIN, SCROLLBAR],
    squareline: &["PANEL", "OBJECT"],
    ..BASE
};

const CONTAINER: WidgetDef = WidgetDef {
    kind: WidgetKind::Container,
    name: "Container",
    icon: "⛶",
    description: "An invisible lv_obj without theme styles, for grouping and layouts.",
    post_create: "lv_obj_remove_style_all({obj});",
    container: true,
    slots: &[MAIN, SCROLLBAR],
    initial_flags: &[(ObjFlag::Clickable, false), (ObjFlag::Scrollable, false)],
    squareline: &["CONTAINER"],
    ..BASE
};

const LABEL: WidgetDef = WidgetDef {
    kind: WidgetKind::Label,
    name: "Label",
    icon: "T",
    description: "Displays text.",
    create: "lv_label_create({parent})",
    width: Coord::content(),
    height: Coord::content(),
    slots: &[MAIN, SCROLLBAR, SELECTED],
    props: &[
        PropDef::new("long_mode", "Long mode", K::Enum(LONG_MODES), D::Enum("LV_LABEL_LONG_WRAP"))
            .call("lv_label_set_long_mode({obj}, {v});"),
        PropDef::new("text", "Text", K::Lines, D::Text("Text")).call("lv_label_set_text({obj}, {v});").always(),
        PropDef::new("recolor", "Recolor", K::Bool, D::Bool(false))
            .call("lv_label_set_recolor({obj}, {v});")
            .help("Enables `#RRGGBB text#` color commands."),
        PropDef::new("translate", "Translate", K::Bool, D::Bool(false))
            .help("With multi-language export, the text is translated with lv_i18n."),
    ],
    flags_off: &[ObjFlag::Clickable, ObjFlag::ClickFocusable],
    squareline: &["LABEL"],
    ..BASE
};

const BUTTON: WidgetDef = WidgetDef {
    kind: WidgetKind::Button,
    name: "Button",
    icon: "⏺",
    description: "A clickable lv_btn; usually contains a label.",
    create: "lv_btn_create({parent})",
    container: true,
    flags_on: &[ObjFlag::ScrollOnFocus],
    flags_off: &[ObjFlag::Scrollable],
    squareline: &["BUTTON"],
    ..BASE
};

const IMAGE: WidgetDef = WidgetDef {
    kind: WidgetKind::Image,
    name: "Image",
    icon: "🖼",
    description: "Displays an image asset.",
    create: "lv_img_create({parent})",
    width: Coord::content(),
    height: Coord::content(),
    props: &[
        PropDef::new("src", "Image", K::Image, D::Image).call("lv_img_set_src({obj}, {v});").not_empty().sl(&["Asset"]),
        PropDef::new("pivot", "Pivot", K::Pair { min: -2000, max: 2000, first: "X", second: "Y" }, D::Pair(0, 0))
            .call("lv_img_set_pivot({obj}, {v.0}, {v.1});")
            .sl(&["Pivot_x#0", "Pivot_y#1"])
            .help("0, 0 keeps the default pivot (image center)."),
        PropDef::new("angle", "Rotation (0.1°)", K::Int { min: 0, max: 3600 }, D::Int(0))
            .call("lv_img_set_angle({obj}, {v});")
            .sl(&["Rotation"]),
        PropDef::new("zoom", "Zoom (256 = 100%)", K::Int { min: 1, max: 2560 }, D::Int(256))
            .call("lv_img_set_zoom({obj}, {v});")
            .sl(&["Scale"]),
        PropDef::new("antialias", "Anti-aliasing", K::Bool, D::Bool(true)).call("lv_img_set_antialias({obj}, {v});"),
    ],
    flags_on: &[ObjFlag::AdvHittest],
    flags_off: &[ObjFlag::Clickable],
    squareline: &["IMAGE"],
    ..BASE
};

const IMG_BUTTON: WidgetDef = WidgetDef {
    kind: WidgetKind::ImgButton,
    name: "Image button",
    icon: "🔳",
    description: "A button drawn from images per state.",
    create: "lv_imgbtn_create({parent})",
    width: Coord::px(100),
    height: Coord::px(50),
    props: &[
        PropDef::new("released", "Released", K::Image, D::Image)
            .call("lv_imgbtn_set_src({obj}, LV_IMGBTN_STATE_RELEASED, NULL, {v}, NULL);")
            .not_empty()
            .sl(&["Released_image"]),
        PropDef::new("pressed", "Pressed", K::Image, D::Image)
            .call("lv_imgbtn_set_src({obj}, LV_IMGBTN_STATE_PRESSED, NULL, {v}, NULL);")
            .not_empty()
            .sl(&["Pressed_image"]),
        PropDef::new("disabled", "Disabled", K::Image, D::Image)
            .call("lv_imgbtn_set_src({obj}, LV_IMGBTN_STATE_DISABLED, NULL, {v}, NULL);")
            .not_empty()
            .sl(&["Disabled_image"]),
        PropDef::new("checked_released", "Checked released", K::Image, D::Image)
            .call("lv_imgbtn_set_src({obj}, LV_IMGBTN_STATE_CHECKED_RELEASED, NULL, {v}, NULL);")
            .not_empty()
            .sl(&["Checked_released_image"]),
        PropDef::new("checked_pressed", "Checked pressed", K::Image, D::Image)
            .call("lv_imgbtn_set_src({obj}, LV_IMGBTN_STATE_CHECKED_PRESSED, NULL, {v}, NULL);")
            .not_empty()
            .sl(&["Checked_pressed_image"]),
    ],
    squareline: &["IMGBUTTON", "IMAGEBUTTON"],
    ..BASE
};

const LINE: WidgetDef = WidgetDef {
    kind: WidgetKind::Line,
    name: "Line",
    icon: "📏",
    description: "A polyline through a list of points.",
    create: "lv_line_create({parent})",
    width: Coord::content(),
    height: Coord::content(),
    props: &[
        PropDef::new("points", "Points", K::Points, D::Text("0,0 50,50 100,0"))
            .call("lv_line_set_points({obj}, {v}, {v.len});")
            .not_empty()
            .help("Pairs of x,y separated by spaces."),
        PropDef::new("y_invert", "Invert Y", K::Bool, D::Bool(false)).call("lv_line_set_y_invert({obj}, {v});"),
    ],
    flags_off: &[ObjFlag::Clickable],
    squareline: &["LINE"],
    ..BASE
};

const ARC: WidgetDef = WidgetDef {
    kind: WidgetKind::Arc,
    name: "Arc",
    category: Category::Controls,
    icon: "↻",
    description: "An adjustable arc with a knob.",
    create: "lv_arc_create({parent})",
    width: Coord::px(150),
    height: Coord::px(150),
    slots: &[MAIN, INDICATOR, KNOB],
    props: &[
        PropDef::new("mode", "Mode", K::Enum(ARC_MODES), D::Enum("LV_ARC_MODE_NORMAL"))
            .call("lv_arc_set_mode({obj}, {v});"),
        PropDef::new("range", "Range", RANGE, D::Pair(0, 100)).call("lv_arc_set_range({obj}, {v.0}, {v.1});"),
        PropDef::new("bg_angles", "Background angles", ANGLE_PAIR, D::Pair(135, 45))
            .call("lv_arc_set_bg_angles({obj}, {v.0}, {v.1});"),
        PropDef::new("rotation", "Rotation", K::Int { min: 0, max: 360 }, D::Int(0))
            .call("lv_arc_set_rotation({obj}, {v});"),
        PropDef::new("value", "Value", VALUE, D::Int(0)).call("lv_arc_set_value({obj}, {v});").always(),
        PropDef::new("change_rate", "Change rate (°/s)", K::Int { min: 1, max: 3600 }, D::Int(720))
            .call("lv_arc_set_change_rate({obj}, {v});"),
    ],
    flags_off: &[ObjFlag::ScrollChainHor, ObjFlag::ScrollChainVer],
    squareline: &["ARC"],
    ..BASE
};

const BAR: WidgetDef = WidgetDef {
    kind: WidgetKind::Bar,
    name: "Bar",
    category: Category::Visualization,
    icon: "⬜",
    description: "A progress bar.",
    create: "lv_bar_create({parent})",
    width: Coord::px(150),
    height: Coord::px(10),
    slots: &[MAIN, INDICATOR],
    props: &[
        PropDef::new("mode", "Mode", K::Enum(BAR_MODES), D::Enum("LV_BAR_MODE_NORMAL"))
            .call("lv_bar_set_mode({obj}, {v});"),
        PropDef::new("range", "Range", RANGE, D::Pair(0, 100)).call("lv_bar_set_range({obj}, {v.0}, {v.1});"),
        PropDef::new("value", "Value", VALUE, D::Int(0)).call("lv_bar_set_value({obj}, {v}, LV_ANIM_OFF);"),
        PropDef::new("start_value", "Start value", VALUE, D::Int(0))
            .call("lv_bar_set_start_value({obj}, {v}, LV_ANIM_OFF);")
            .sl(&["Value_start"])
            .help("Only used in range mode."),
    ],
    flags_off: &[ObjFlag::Checkable, ObjFlag::Scrollable],
    squareline: &["BAR"],
    ..BASE
};

const SLIDER: WidgetDef = WidgetDef {
    kind: WidgetKind::Slider,
    name: "Slider",
    category: Category::Controls,
    icon: "↔",
    description: "A bar with a knob that can be dragged.",
    create: "lv_slider_create({parent})",
    width: Coord::px(150),
    height: Coord::px(10),
    slots: &[MAIN, INDICATOR, KNOB],
    props: &[
        PropDef::new("mode", "Mode", K::Enum(SLIDER_MODES), D::Enum("LV_SLIDER_MODE_NORMAL"))
            .call("lv_slider_set_mode({obj}, {v});"),
        PropDef::new("range", "Range", RANGE, D::Pair(0, 100)).call("lv_slider_set_range({obj}, {v.0}, {v.1});"),
        PropDef::new("value", "Value", VALUE, D::Int(0)).call("lv_slider_set_value({obj}, {v}, LV_ANIM_OFF);"),
        PropDef::new("value_left", "Left value", VALUE, D::Int(0))
            .call("lv_slider_set_left_value({obj}, {v}, LV_ANIM_OFF);")
            .help("Only used in range mode."),
    ],
    flags_on: &[ObjFlag::ScrollOnFocus],
    flags_off: &[ObjFlag::Scrollable, ObjFlag::ScrollChainHor, ObjFlag::ScrollChainVer],
    squareline: &["SLIDER"],
    ..BASE
};

const SWITCH: WidgetDef = WidgetDef {
    kind: WidgetKind::Switch,
    name: "Switch",
    category: Category::Controls,
    icon: "🔘",
    description: "A toggle switch; use the Checked state for on.",
    create: "lv_switch_create({parent})",
    width: Coord::px(50),
    height: Coord::px(25),
    slots: &[MAIN, INDICATOR, KNOB],
    flags_on: &[ObjFlag::Checkable, ObjFlag::ScrollOnFocus],
    flags_off: &[ObjFlag::Scrollable],
    squareline: &["SWITCH"],
    ..BASE
};

const CHECKBOX: WidgetDef = WidgetDef {
    kind: WidgetKind::Checkbox,
    name: "Checkbox",
    category: Category::Controls,
    icon: "☑",
    description: "A tick box with a label.",
    create: "lv_checkbox_create({parent})",
    width: Coord::content(),
    height: Coord::content(),
    slots: &[MAIN, INDICATOR.sl(&["bullet"])],
    props: &[
        PropDef::new("text", "Text", K::Text, D::Text("Check box"))
            .call("lv_checkbox_set_text({obj}, {v});")
            .sl(&["Title"]),
        PropDef::new("translate", "Translate", K::Bool, D::Bool(false))
            .help("With multi-language export, the text is translated with lv_i18n."),
    ],
    flags_on: &[ObjFlag::Checkable, ObjFlag::ScrollOnFocus],
    flags_off: &[ObjFlag::Scrollable],
    squareline: &["CHECKBOX"],
    ..BASE
};

const DROPDOWN: WidgetDef = WidgetDef {
    kind: WidgetKind::Dropdown,
    name: "Dropdown",
    category: Category::Controls,
    icon: "⏷",
    description: "A button that opens a list of options.",
    create: "lv_dropdown_create({parent})",
    width: Coord::px(150),
    height: Coord::content(),
    slots: &[
        MAIN,
        INDICATOR,
        StyleSlot::new("list_main", "List", Part::Main).on("lv_dropdown_get_list({obj})"),
        StyleSlot::new("list_scrollbar", "List scrollbar", Part::Scrollbar).on("lv_dropdown_get_list({obj})"),
        StyleSlot::new("list_selected", "List selected", Part::Selected).on("lv_dropdown_get_list({obj})"),
    ],
    props: &[
        PropDef::new("options", "Options", K::Lines, D::Text("Option 1\nOption 2\nOption 3"))
            .call("lv_dropdown_set_options({obj}, {v});")
            .always(),
        PropDef::new("selected", "Selected index", COUNT, D::Int(0)).call("lv_dropdown_set_selected({obj}, {v});"),
        PropDef::new("dir", "List direction", K::Enum(LIST_DIRS), D::Enum("LV_DIR_BOTTOM"))
            .call("lv_dropdown_set_dir({obj}, {v});")
            .sl(&["List_align"]),
        PropDef::new("show_selected", "Highlight selected", K::Bool, D::Bool(true))
            .call("lv_dropdown_set_selected_highlight({obj}, {v});"),
        PropDef::new("text", "Fixed text", K::Text, D::Text(""))
            .call("lv_dropdown_set_text({obj}, {v});")
            .not_empty()
            .sl(&["Base_text"])
            .help("Shown instead of the selected option if set."),
    ],
    flags_off: &[ObjFlag::Scrollable],
    squareline: &["DROPDOWN"],
    ..BASE
};

const ROLLER: WidgetDef = WidgetDef {
    kind: WidgetKind::Roller,
    name: "Roller",
    category: Category::Controls,
    icon: "☰",
    description: "A scrollable wheel of options.",
    create: "lv_roller_create({parent})",
    width: Coord::content(),
    height: Coord::content(),
    slots: &[MAIN, SELECTED],
    props: &[
        PropDef::new("options", "Options", K::Lines, D::Text("Option 1\nOption 2\nOption 3")),
        PropDef::new("mode", "Mode", K::Enum(ROLLER_MODES), D::Enum("LV_ROLLER_MODE_NORMAL"))
            .call("lv_roller_set_options({obj}, {options}, {v});")
            .always(),
        PropDef::new("selected", "Selected index", COUNT, D::Int(0))
            .call("lv_roller_set_selected({obj}, {v}, LV_ANIM_OFF);"),
        PropDef::new("visible_rows", "Visible rows", K::Int { min: 1, max: 20 }, D::Int(4))
            .call("lv_roller_set_visible_row_count({obj}, {v});")
            .sl(&["Visible_row_count"]),
    ],
    flags_off: &[ObjFlag::ScrollChainHor, ObjFlag::ScrollChainVer],
    squareline: &["ROLLER"],
    ..BASE
};

const TEXTAREA: WidgetDef = WidgetDef {
    kind: WidgetKind::Textarea,
    name: "Text area",
    category: Category::Controls,
    icon: "📝",
    description: "An editable text field.",
    create: "lv_textarea_create({parent})",
    width: Coord::px(150),
    height: Coord::px(70),
    slots: &[
        MAIN,
        SCROLLBAR,
        SELECTED,
        CURSOR,
        StyleSlot::new("placeholder", "Placeholder", Part::TextareaPlaceholder),
    ],
    props: &[
        PropDef::new("one_line", "One line", K::Bool, D::Bool(false))
            .call("lv_textarea_set_one_line({obj}, {v});")
            .sl(&["One_line_mode"]),
        PropDef::new("password_mode", "Password mode", K::Bool, D::Bool(false))
            .call("lv_textarea_set_password_mode({obj}, {v});"),
        PropDef::new("max_length", "Max. length", COUNT, D::Int(0))
            .call("lv_textarea_set_max_length({obj}, {v});")
            .sl(&["Max_text_length"]),
        PropDef::new("accepted_chars", "Accepted characters", K::Text, D::Text(""))
            .call("lv_textarea_set_accepted_chars({obj}, {v});")
            .not_empty()
            .sl(&["Accepted_characters"]),
        PropDef::new("text", "Text", K::Lines, D::Text("")).call("lv_textarea_set_text({obj}, {v});").not_empty(),
        PropDef::new("placeholder", "Placeholder", K::Text, D::Text(""))
            .call("lv_textarea_set_placeholder_text({obj}, {v});")
            .not_empty(),
        PropDef::new("text_selection", "Text selection", K::Bool, D::Bool(false))
            .call("lv_textarea_set_text_selection({obj}, {v});"),
    ],
    flags_on: &[ObjFlag::ScrollOnFocus],
    squareline: &["TEXTAREA"],
    ..BASE
};

const KEYBOARD: WidgetDef = WidgetDef {
    kind: WidgetKind::Keyboard,
    name: "Keyboard",
    category: Category::Controls,
    icon: "⌨",
    description: "An on-screen keyboard bound to a text area.",
    create: "lv_keyboard_create({parent})",
    width: Coord::px(300),
    height: Coord::px(120),
    align: Align::BottomMid,
    slots: &[MAIN, ITEMS],
    props: &[
        PropDef::new("mode", "Mode", K::Enum(KEYBOARD_MODES), D::Enum("LV_KEYBOARD_MODE_TEXT_LOWER"))
            .call("lv_keyboard_set_mode({obj}, {v});"),
        PropDef::new("popovers", "Popovers", K::Bool, D::Bool(false)).call("lv_keyboard_set_popovers({obj}, {v});"),
        PropDef::new("textarea", "Target text area", K::WidgetRef, D::WidgetRef)
            .call("lv_keyboard_set_textarea({obj}, {v});")
            .not_empty()
            .sl(&["Target_textarea"]),
    ],
    flags_off: &[ObjFlag::ClickFocusable],
    squareline: &["KEYBOARD"],
    ..BASE
};

const BTN_MATRIX: WidgetDef = WidgetDef {
    kind: WidgetKind::BtnMatrix,
    name: "Button matrix",
    category: Category::Controls,
    icon: "🔢",
    description: "A grid of lightweight buttons.",
    create: "lv_btnmatrix_create({parent})",
    width: Coord::px(200),
    height: Coord::px(100),
    slots: &[MAIN, ITEMS],
    props: &[
        PropDef::new("map", "Buttons", K::StrArray { rows: true }, D::Text("1|2|3\n4|5|6"))
            .call("lv_btnmatrix_set_map({obj}, {v});")
            .not_empty()
            .help("One row per line, buttons separated by |."),
        PropDef::new("one_checked", "One checked", K::Bool, D::Bool(false))
            .call("lv_btnmatrix_set_one_checked({obj}, {v});"),
    ],
    squareline: &["BUTTONMATRIX", "BTNMATRIX"],
    ..BASE
};

const SPINBOX: WidgetDef = WidgetDef {
    kind: WidgetKind::Spinbox,
    name: "Spinbox",
    category: Category::Controls,
    icon: "±",
    description: "A numeric input with digit cursor.",
    create: "lv_spinbox_create({parent})",
    width: Coord::px(100),
    height: Coord::content(),
    slots: &[MAIN, CURSOR, SELECTED],
    props: &[
        PropDef::new("digit_count", "Digits", K::Int { min: 1, max: 10 }, D::Int(5))
            .call("lv_spinbox_set_digit_format({obj}, {v}, {separator_position});")
            .always()
            .sl(&["Digits"]),
        PropDef::new("separator_position", "Decimal point position", K::Int { min: 0, max: 10 }, D::Int(0))
            .sl(&["Decimal_position", "Separator"]),
        PropDef::new(
            "range",
            "Range",
            K::Pair { min: -999_999_999, max: 999_999_999, first: "Min", second: "Max" },
            D::Pair(-99999, 99999),
        )
        .call("lv_spinbox_set_range({obj}, {v.0}, {v.1});"),
        PropDef::new("step", "Step", K::Int { min: 1, max: 1_000_000 }, D::Int(1))
            .call("lv_spinbox_set_step({obj}, {v});"),
        PropDef::new("value", "Value", K::Int { min: -999_999_999, max: 999_999_999 }, D::Int(0))
            .call("lv_spinbox_set_value({obj}, {v});"),
        PropDef::new("rollover", "Rollover", K::Bool, D::Bool(false)).call("lv_spinbox_set_rollover({obj}, {v});"),
    ],
    squareline: &["SPINBOX"],
    ..BASE
};

const COLORWHEEL: WidgetDef = WidgetDef {
    kind: WidgetKind::Colorwheel,
    name: "Color wheel",
    category: Category::Controls,
    icon: "🎨",
    description: "Pick a color in HSV space.",
    create: "lv_colorwheel_create({parent}, {knob_recolor})",
    width: Coord::px(150),
    height: Coord::px(150),
    slots: &[MAIN, KNOB],
    props: &[
        PropDef::new("knob_recolor", "Recolor knob", K::Bool, D::Bool(true)),
        PropDef::new("mode", "Mode", K::Enum(COLORWHEEL_MODES), D::Enum("LV_COLORWHEEL_MODE_HUE"))
            .call("lv_colorwheel_set_mode({obj}, {v});"),
        PropDef::new("fixed_mode", "Fixed mode", K::Bool, D::Bool(false))
            .call("lv_colorwheel_set_mode_fixed({obj}, {v});"),
        PropDef::new("color", "Color", K::Color, D::Color(0xFF0000)).call("lv_colorwheel_set_rgb({obj}, {v});"),
    ],
    squareline: &["COLORWHEEL"],
    ..BASE
};

const SPINNER: WidgetDef = WidgetDef {
    kind: WidgetKind::Spinner,
    name: "Spinner",
    category: Category::Visualization,
    icon: "🔄",
    description: "A rotating arc as busy indicator.",
    create: "lv_spinner_create({parent}, {time}, {arc_length})",
    width: Coord::px(80),
    height: Coord::px(80),
    slots: &[MAIN, INDICATOR],
    props: &[
        PropDef::new("time", "Period (ms)", K::Int { min: 100, max: 60000 }, D::Int(1000)).sl(&["Spinner_time"]),
        PropDef::new("arc_length", "Arc length (°)", K::Int { min: 1, max: 360 }, D::Int(90)),
    ],
    flags_off: &[ObjFlag::Clickable],
    squareline: &["SPINNER"],
    ..BASE
};

const LED: WidgetDef = WidgetDef {
    kind: WidgetKind::Led,
    name: "LED",
    category: Category::Visualization,
    icon: "💡",
    description: "A round light with adjustable brightness.",
    create: "lv_led_create({parent})",
    width: Coord::px(30),
    height: Coord::px(30),
    props: &[
        PropDef::new("color", "Color", K::Color, D::Color(0x2196F3)).call("lv_led_set_color({obj}, {v});").always(),
        PropDef::new("brightness", "Brightness", K::Int { min: 0, max: 255 }, D::Int(255))
            .call("lv_led_set_brightness({obj}, {v});"),
    ],
    squareline: &["LED"],
    ..BASE
};

const METER: WidgetDef = WidgetDef {
    kind: WidgetKind::Meter,
    name: "Meter",
    category: Category::Visualization,
    icon: "◔",
    description: "A gauge with a scale and a needle.",
    create: "lv_meter_create({parent})",
    post_props: "lv_meter_scale_t * {obj}_scale = lv_meter_add_scale({obj});\n\
lv_meter_set_scale_ticks({obj}, {obj}_scale, {ticks}, 2, 10, lv_palette_main(LV_PALETTE_GREY));\n\
lv_meter_set_scale_major_ticks({obj}, {obj}_scale, {major_every}, 4, 15, {major_color}, 10);\n\
lv_meter_set_scale_range({obj}, {obj}_scale, {range.0}, {range.1}, {angle_range}, {rotation});\n\
lv_meter_indicator_t * {obj}_needle = lv_meter_add_needle_line({obj}, {obj}_scale, 4, {needle_color}, -10);\n\
lv_meter_set_indicator_value({obj}, {obj}_needle, {value});",
    width: Coord::px(200),
    height: Coord::px(200),
    slots: &[MAIN, TICKS, INDICATOR, ITEMS],
    props: &[
        PropDef::new("range", "Scale range", RANGE, D::Pair(0, 100)),
        PropDef::new("angle_range", "Angle range (°)", K::Int { min: 1, max: 360 }, D::Int(270)),
        PropDef::new("rotation", "Rotation (°)", K::Int { min: 0, max: 360 }, D::Int(135)),
        PropDef::new("ticks", "Tick count", K::Int { min: 2, max: 200 }, D::Int(41)),
        PropDef::new("major_every", "Major tick every", K::Int { min: 1, max: 100 }, D::Int(8)),
        PropDef::new("major_color", "Major tick color", K::Color, D::Color(0x000000)),
        PropDef::new("value", "Needle value", VALUE, D::Int(0)),
        PropDef::new("needle_color", "Needle color", K::Color, D::Color(0x2196F3)),
    ],
    squareline: &["METER"],
    ..BASE
};

const CHART: WidgetDef = WidgetDef {
    kind: WidgetKind::Chart,
    name: "Chart",
    category: Category::Visualization,
    icon: "📈",
    description: "Line, bar or scatter chart.",
    create: "lv_chart_create({parent})",
    width: Coord::px(180),
    height: Coord::px(100),
    slots: &[MAIN, SCROLLBAR, ITEMS, INDICATOR, TICKS],
    props: &[
        PropDef::new("chart_type", "Type", K::Enum(CHART_TYPES), D::Enum("LV_CHART_TYPE_LINE"))
            .call("lv_chart_set_type({obj}, {v});")
            .sl(&["Type"]),
        PropDef::new("point_count", "Point count", K::Int { min: 1, max: 1000 }, D::Int(10))
            .call("lv_chart_set_point_count({obj}, {v});")
            .sl(&["Number_of_points"]),
        PropDef::new("y_range", "Y range", RANGE, D::Pair(0, 100))
            .call("lv_chart_set_range({obj}, LV_CHART_AXIS_PRIMARY_Y, {v.0}, {v.1});")
            .sl(&["Primary_Y_range", "Range"]),
        PropDef::new("y2_range", "Secondary Y range", RANGE, D::Pair(0, 100))
            .call("lv_chart_set_range({obj}, LV_CHART_AXIS_SECONDARY_Y, {v.0}, {v.1});")
            .sl(&["Secondary_Y_range"]),
        PropDef::new("div_lines", "Division lines", K::Pair { min: 0, max: 50, first: "Horizontal", second: "Vertical" }, D::Pair(3, 5))
            .call("lv_chart_set_div_line_count({obj}, {v.0}, {v.1});")
            .sl(&["Division_lines"]),
        PropDef::new("zoom", "Zoom (256 = 100%)", K::Pair { min: 256, max: 2560, first: "X", second: "Y" }, D::Pair(256, 256))
            .call("lv_chart_set_zoom_x({obj}, {v.0});\nlv_chart_set_zoom_y({obj}, {v.1});"),
        PropDef::new("update_mode", "Update mode", K::Enum(CHART_UPDATE_MODES), D::Enum("LV_CHART_UPDATE_MODE_SHIFT"))
            .call("lv_chart_set_update_mode({obj}, {v});"),
        // Axis ticks: (length, count) for major and minor ticks, labels and label space.
        PropDef::new("x_major", "X major ticks", TICK_PAIR, D::Pair(0, 0))
            .call("lv_chart_set_axis_tick({obj}, LV_CHART_AXIS_PRIMARY_X, {v.0}, {x_minor.0}, {v.1}, {x_minor.1}, {x_labels}, {x_label_size});")
            .always()
            .sl(&["XMajor"]),
        PropDef::new("x_minor", "X minor ticks", TICK_PAIR, D::Pair(0, 0)).sl(&["XMinor"]),
        PropDef::new("x_labels", "X labels", K::Bool, D::Bool(false)).sl(&["Labels_on_X_axis"]),
        PropDef::new("x_label_size", "X label space", K::Int { min: 0, max: 200 }, D::Int(0)).sl(&["Font_size_on_X_axis"]),
        PropDef::new("y_major", "Y major ticks", TICK_PAIR, D::Pair(0, 0))
            .call("lv_chart_set_axis_tick({obj}, LV_CHART_AXIS_PRIMARY_Y, {v.0}, {y_minor.0}, {v.1}, {y_minor.1}, {y_labels}, {y_label_size});")
            .always()
            .sl(&["PrimaryYMajor"]),
        PropDef::new("y_minor", "Y minor ticks", TICK_PAIR, D::Pair(0, 0)).sl(&["PrimaryYMinor"]),
        PropDef::new("y_labels", "Y labels", K::Bool, D::Bool(false)).sl(&["Labels_on_Primary_Y_axis"]),
        PropDef::new("y_label_size", "Y label space", K::Int { min: 0, max: 200 }, D::Int(0))
            .sl(&["Font_size_on_Primary_Y_axis"]),
        PropDef::new("y2_major", "Secondary Y major ticks", TICK_PAIR, D::Pair(0, 0))
            .call("lv_chart_set_axis_tick({obj}, LV_CHART_AXIS_SECONDARY_Y, {v.0}, {y2_minor.0}, {v.1}, {y2_minor.1}, {y2_labels}, {y2_label_size});")
            .always()
            .sl(&["SecondaryYMajor"]),
        PropDef::new("y2_minor", "Secondary Y minor ticks", TICK_PAIR, D::Pair(0, 0)).sl(&["SecondaryYMinor"]),
        PropDef::new("y2_labels", "Secondary Y labels", K::Bool, D::Bool(false)).sl(&["Labels_on_Secondary_Y_axis"]),
        PropDef::new("y2_label_size", "Secondary Y label space", K::Int { min: 0, max: 200 }, D::Int(0))
            .sl(&["Font_size_on_Secondary_Y_axis"]),
        PropDef::new("series", "Series", K::ChartSeries, D::Text("#2196F3: 10, 30, 20, 60, 40, 80, 50, 90, 70, 60"))
            .emit(Emit::ChartSeries)
            .sl(&["ChartData"])
            .help("One series per line: #RRGGBB: v1, v2, ... (write #RRGGBB secondary: ... for the secondary Y axis)"),
    ],
    squareline: &["CHART"],
    ..BASE
};

const TABLE: WidgetDef = WidgetDef {
    kind: WidgetKind::Table,
    name: "Table",
    category: Category::Visualization,
    icon: "📊",
    description: "Rows and columns of text cells.",
    create: "lv_table_create({parent})",
    width: Coord::content(),
    height: Coord::content(),
    slots: &[MAIN, SCROLLBAR, ITEMS],
    props: &[
        PropDef::new("cells", "Cells", K::Lines, D::Text("Name|Value\nA|1\nB|2"))
            .emit(Emit::EachCell("lv_table_set_cell_value({obj}, {row}, {col}, {cell});"))
            .not_empty()
            .help("One row per line, cells separated by |."),
        PropDef::new("col_width", "Column width", K::Int { min: 0, max: 2000 }, D::Int(0))
            .call("for(uint16_t col = 0; col < lv_table_get_col_cnt({obj}); col++) lv_table_set_col_width({obj}, col, {v});")
            .help("0 keeps the default width."),
    ],
    squareline: &["TABLE"],
    ..BASE
};

const CALENDAR: WidgetDef = WidgetDef {
    kind: WidgetKind::Calendar,
    name: "Calendar",
    category: Category::Visualization,
    icon: "📅",
    description: "A month view with optional header.",
    create: "lv_calendar_create({parent})",
    width: Coord::px(230),
    height: Coord::px(240),
    slots: &[MAIN, StyleSlot::new("days", "Days", Part::Items).on("lv_calendar_get_btnmatrix({obj})")],
    props: &[
        PropDef::new("today", "Today", K::Date, D::Date(2024, 1, 1))
            .call("lv_calendar_set_today_date({obj}, {v.year}, {v.month}, {v.day});")
            .always(),
        PropDef::new("shown", "Shown month", K::Date, D::Date(2024, 1, 1))
            .call("lv_calendar_set_showed_date({obj}, {v.year}, {v.month});")
            .always(),
        PropDef::new("header", "Header", K::Enum(CALENDAR_HEADERS), D::Enum("")).emit(Emit::OptionTemplate),
    ],
    squareline: &["CALENDAR"],
    ..BASE
};

const CANVAS: WidgetDef = WidgetDef {
    kind: WidgetKind::Canvas,
    name: "Canvas",
    category: Category::Visualization,
    icon: "📐",
    description: "A pixel buffer to draw on at runtime. Uses the pixel size of the widget.",
    create: "lv_canvas_create({parent})",
    post_create: "static lv_color_t {obj}_buf[LV_CANVAS_BUF_SIZE_TRUE_COLOR({w}, {h})];\n\
lv_canvas_set_buffer({obj}, {obj}_buf, {w}, {h}, LV_IMG_CF_TRUE_COLOR);\n\
lv_canvas_fill_bg({obj}, {fill}, LV_OPA_COVER);",
    width: Coord::px(100),
    height: Coord::px(100),
    props: &[PropDef::new("fill", "Fill color", K::Color, D::Color(0xFFFFFF))],
    flags_off: &[ObjFlag::Clickable],
    squareline: &["CANVAS"],
    ..BASE
};

const ANIM_IMG: WidgetDef = WidgetDef {
    kind: WidgetKind::AnimImg,
    name: "Animated image",
    category: Category::Visualization,
    icon: "🎞",
    description: "Plays a sequence of images.",
    create: "lv_animimg_create({parent})",
    post_props: "lv_animimg_start({obj});",
    width: Coord::content(),
    height: Coord::content(),
    props: &[
        PropDef::new("images", "Frames", K::ImageArray, D::Text(""))
            .call("lv_animimg_set_src({obj}, (const void **){v}, {v.len});")
            .not_empty()
            .help("One image asset per line."),
        PropDef::new("duration", "Duration (ms)", K::Int { min: 1, max: 60000 }, D::Int(1000))
            .call("lv_animimg_set_duration({obj}, {v});")
            .always(),
        PropDef::new("repeat", "Repeat count (-1 = infinite)", K::Int { min: -1, max: 1000 }, D::Int(-1))
            .call("lv_animimg_set_repeat_count({obj}, {v});")
            .always(),
    ],
    flags_off: &[ObjFlag::Clickable],
    squareline: &["ANIMIMG"],
    ..BASE
};

const SPANGROUP: WidgetDef = WidgetDef {
    kind: WidgetKind::Spangroup,
    name: "Span group",
    category: Category::Visualization,
    icon: "¶",
    description: "Rich text made of spans.",
    create: "lv_spangroup_create({parent})",
    post_props: "lv_spangroup_refr_mode({obj});",
    width: Coord::px(200),
    height: Coord::content(),
    props: &[
        PropDef::new("align", "Alignment", K::Enum(TEXT_ALIGNS), D::Enum("LV_TEXT_ALIGN_LEFT"))
            .call("lv_spangroup_set_align({obj}, {v});"),
        PropDef::new("overflow", "Overflow", K::Enum(SPAN_OVERFLOWS), D::Enum("LV_SPAN_OVERFLOW_CLIP"))
            .call("lv_spangroup_set_overflow({obj}, {v});"),
        PropDef::new("mode", "Mode", K::Enum(SPAN_MODES), D::Enum("LV_SPAN_MODE_FIXED"))
            .call("lv_spangroup_set_mode({obj}, {v});"),
        PropDef::new("indent", "Indent", K::Int { min: 0, max: 500 }, D::Int(0))
            .call("lv_spangroup_set_indent({obj}, {v});"),
        PropDef::new("spans", "Spans", K::Lines, D::Text("Rich text"))
            .emit(Emit::EachLine("lv_span_set_text(lv_spangroup_new_span({obj}), {line});"))
            .not_empty()
            .help("One span per line."),
    ],
    squareline: &["SPANGROUP"],
    ..BASE
};

const TABVIEW: WidgetDef = WidgetDef {
    kind: WidgetKind::Tabview,
    name: "Tab view",
    category: Category::Containers,
    icon: "📁",
    description: "Tabs with a button bar; add Tab pages as children.",
    create: "lv_tabview_create({parent}, {tab_pos}, {tab_size})",
    container: true,
    required_child: Some(WidgetKind::TabPage),
    width: Coord::px(300),
    height: Coord::px(200),
    slots: &[
        MAIN,
        StyleSlot::new("tab_bar", "Tab bar", Part::Main).on("lv_tabview_get_tab_btns({obj})").sl(&["tab_main"]),
        StyleSlot::new("tab_buttons", "Tab buttons", Part::Items)
            .on("lv_tabview_get_tab_btns({obj})")
            .sl(&["tab_items"]),
        StyleSlot::new("content", "Content", Part::Main).on("lv_tabview_get_content({obj})"),
    ],
    props: &[
        PropDef::new("tab_pos", "Tab position", K::Enum(LIST_DIRS), D::Enum("LV_DIR_TOP")).sl(&["Tab_position"]),
        PropDef::new("tab_size", "Tab bar size", K::Int { min: 10, max: 500 }, D::Int(50)),
    ],
    squareline: &["TABVIEW"],
    ..BASE
};

const TAB_PAGE: WidgetDef = WidgetDef {
    kind: WidgetKind::TabPage,
    name: "Tab page",
    category: Category::Containers,
    icon: "📄",
    description: "One tab of a Tab view.",
    create: "lv_tabview_add_tab({parent}, {title})",
    container: true,
    required_parent: Some(WidgetKind::Tabview),
    managed_geometry: true,
    width: Coord::pct(100),
    height: Coord::pct(100),
    align: Align::TopLeft,
    slots: &[MAIN, SCROLLBAR],
    props: &[PropDef::new("title", "Title", K::Text, D::Text("Tab"))],
    squareline: &["TABPAGE"],
    ..BASE
};

const TILEVIEW: WidgetDef = WidgetDef {
    kind: WidgetKind::Tileview,
    name: "Tile view",
    category: Category::Containers,
    icon: "⊞",
    description: "Swipeable grid of tiles; add Tiles as children.",
    create: "lv_tileview_create({parent})",
    container: true,
    required_child: Some(WidgetKind::Tile),
    width: Coord::pct(100),
    height: Coord::pct(100),
    slots: &[MAIN, SCROLLBAR],
    squareline: &["TILEVIEW"],
    ..BASE
};

const TILE: WidgetDef = WidgetDef {
    kind: WidgetKind::Tile,
    name: "Tile",
    category: Category::Containers,
    icon: "⬛",
    description: "One tile of a Tile view.",
    create: "lv_tileview_add_tile({parent}, {col}, {row}, {dir})",
    container: true,
    required_parent: Some(WidgetKind::Tileview),
    managed_geometry: true,
    width: Coord::pct(100),
    height: Coord::pct(100),
    align: Align::TopLeft,
    slots: &[MAIN, SCROLLBAR],
    props: &[
        PropDef::new("col", "Column", K::Int { min: 0, max: 50 }, D::Int(0)),
        PropDef::new("row", "Row", K::Int { min: 0, max: 50 }, D::Int(0)),
        PropDef::new("dir", "Allowed directions", K::Enum(TILE_DIRS), D::Enum("LV_DIR_ALL")),
    ],
    ..BASE
};

const WIN: WidgetDef = WidgetDef {
    kind: WidgetKind::Win,
    name: "Window",
    category: Category::Containers,
    icon: "📂",
    description: "A header with title and a content area for children.",
    create: "lv_win_create({parent}, {header_height})",
    child_parent: Some("lv_win_get_content({obj})"),
    container: true,
    width: Coord::px(300),
    height: Coord::px(200),
    slots: &[
        MAIN,
        StyleSlot::new("header", "Header", Part::Main).on("lv_win_get_header({obj})"),
        StyleSlot::new("content", "Content", Part::Main).on("lv_win_get_content({obj})"),
    ],
    props: &[
        PropDef::new("header_height", "Header height", K::Int { min: 0, max: 500 }, D::Int(40)),
        PropDef::new("title", "Title", K::Text, D::Text("Window")).call("lv_win_add_title({obj}, {v});").not_empty(),
    ],
    squareline: &["WINDOW", "WIN"],
    ..BASE
};

const LIST: WidgetDef = WidgetDef {
    kind: WidgetKind::List,
    name: "List",
    category: Category::Containers,
    icon: "📋",
    description: "A vertical list of buttons.",
    create: "lv_list_create({parent})",
    width: Coord::px(180),
    height: Coord::px(200),
    slots: &[MAIN, SCROLLBAR],
    props: &[PropDef::new("items", "Items", K::Lines, D::Text("Item 1\nItem 2\nItem 3"))
        .emit(Emit::EachLine("lv_list_add_btn({obj}, NULL, {line});"))
        .not_empty()],
    squareline: &["LIST"],
    ..BASE
};

const MENU: WidgetDef = WidgetDef {
    kind: WidgetKind::Menu,
    name: "Menu",
    category: Category::Containers,
    icon: "⊟",
    description: "A menu with a main page of entries.",
    create: "lv_menu_create({parent})",
    post_create: "lv_obj_t * {obj}_page = lv_menu_page_create({obj}, NULL);",
    post_props: "lv_menu_set_page({obj}, {obj}_page);",
    width: Coord::px(300),
    height: Coord::px(200),
    props: &[
        PropDef::new("header_mode", "Header mode", K::Enum(MENU_HEADER_MODES), D::Enum("LV_MENU_HEADER_TOP_FIXED"))
            .call("lv_menu_set_mode_header({obj}, {v});"),
        PropDef::new(
            "root_back_btn",
            "Root back button",
            K::Enum(MENU_BACK_BTN),
            D::Enum("LV_MENU_ROOT_BACK_BTN_DISABLED"),
        )
        .call("lv_menu_set_mode_root_back_btn({obj}, {v});"),
        PropDef::new("items", "Entries", K::Lines, D::Text("Entry 1\nEntry 2\nEntry 3"))
            .emit(Emit::EachLine("lv_label_set_text(lv_label_create(lv_menu_cont_create({obj}_page)), {line});"))
            .not_empty(),
    ],
    squareline: &["MENU"],
    ..BASE
};

const MSGBOX: WidgetDef = WidgetDef {
    kind: WidgetKind::Msgbox,
    name: "Message box",
    category: Category::Containers,
    icon: "💬",
    description: "A dialog with title, text and buttons.",
    create: "lv_msgbox_create({parent}, {title}, {text}, {buttons}, {close_btn})",
    width: Coord::px(260),
    height: Coord::content(),
    slots: &[
        MAIN,
        StyleSlot::new("title", "Title", Part::Main).on("lv_msgbox_get_title({obj})"),
        StyleSlot::new("text", "Text", Part::Main).on("lv_msgbox_get_text({obj})"),
        StyleSlot::new("buttons", "Buttons", Part::Items).on("lv_msgbox_get_btns({obj})"),
    ],
    props: &[
        PropDef::new("title", "Title", K::Text, D::Text("Title")),
        PropDef::new("text", "Text", K::Lines, D::Text("Message")),
        PropDef::new("buttons", "Buttons", K::StrArray { rows: false }, D::Text("Ok|Cancel")).help("Separated by |."),
        PropDef::new("close_btn", "Close button", K::Bool, D::Bool(true)),
    ],
    squareline: &["MSGBOX"],
    ..BASE
};

/// All widget definitions in palette order.
pub const ALL: &[&WidgetDef] = &[
    &SCREEN,
    &PANEL,
    &CONTAINER,
    &LABEL,
    &BUTTON,
    &IMAGE,
    &IMG_BUTTON,
    &LINE,
    &ARC,
    &SLIDER,
    &SWITCH,
    &CHECKBOX,
    &DROPDOWN,
    &ROLLER,
    &TEXTAREA,
    &KEYBOARD,
    &BTN_MATRIX,
    &SPINBOX,
    &COLORWHEEL,
    &BAR,
    &SPINNER,
    &LED,
    &METER,
    &CHART,
    &TABLE,
    &CALENDAR,
    &CANVAS,
    &ANIM_IMG,
    &SPANGROUP,
    &TABVIEW,
    &TAB_PAGE,
    &TILEVIEW,
    &TILE,
    &WIN,
    &LIST,
    &MENU,
    &MSGBOX,
    &COMPONENT,
];

pub fn def(kind: WidgetKind) -> &'static WidgetDef {
    match kind {
        WidgetKind::Screen => &SCREEN,
        WidgetKind::Panel => &PANEL,
        WidgetKind::Container => &CONTAINER,
        WidgetKind::Label => &LABEL,
        WidgetKind::Button => &BUTTON,
        WidgetKind::Image => &IMAGE,
        WidgetKind::ImgButton => &IMG_BUTTON,
        WidgetKind::Line => &LINE,
        WidgetKind::Arc => &ARC,
        WidgetKind::Bar => &BAR,
        WidgetKind::Slider => &SLIDER,
        WidgetKind::Switch => &SWITCH,
        WidgetKind::Checkbox => &CHECKBOX,
        WidgetKind::Dropdown => &DROPDOWN,
        WidgetKind::Roller => &ROLLER,
        WidgetKind::Textarea => &TEXTAREA,
        WidgetKind::Keyboard => &KEYBOARD,
        WidgetKind::BtnMatrix => &BTN_MATRIX,
        WidgetKind::Spinbox => &SPINBOX,
        WidgetKind::Colorwheel => &COLORWHEEL,
        WidgetKind::Spinner => &SPINNER,
        WidgetKind::Led => &LED,
        WidgetKind::Meter => &METER,
        WidgetKind::Chart => &CHART,
        WidgetKind::Table => &TABLE,
        WidgetKind::Calendar => &CALENDAR,
        WidgetKind::Canvas => &CANVAS,
        WidgetKind::AnimImg => &ANIM_IMG,
        WidgetKind::Spangroup => &SPANGROUP,
        WidgetKind::Tabview => &TABVIEW,
        WidgetKind::TabPage => &TAB_PAGE,
        WidgetKind::Tileview => &TILEVIEW,
        WidgetKind::Tile => &TILE,
        WidgetKind::Win => &WIN,
        WidgetKind::List => &LIST,
        WidgetKind::Menu => &MENU,
        WidgetKind::Msgbox => &MSGBOX,
        WidgetKind::Component => &COMPONENT,
    }
}

/// Looks up a widget kind by its SquareLine object type (`BUTTON`, ...).
pub fn by_squareline_name(name: &str) -> Option<WidgetKind> {
    ALL.iter().find(|d| d.squareline.iter().any(|n| n.eq_ignore_ascii_case(name))).map(|d| d.kind)
}

/// Widgets offered in a palette category.
pub fn in_category(category: Category) -> impl Iterator<Item = &'static WidgetDef> {
    ALL.iter().copied().filter(move |d| d.category == category)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::{EmitWhen, PropKind};

    #[test]
    fn every_kind_has_a_consistent_definition() {
        for def in ALL {
            assert_eq!(super::def(def.kind).kind, def.kind, "{}", def.name);
            assert!(!def.name.is_empty());
            assert!(def.slot("main").is_some(), "{} has no main slot", def.name);
            for prop in def.props {
                assert!(prop.default_value().fits(prop.kind), "{}.{}", def.name, prop.key);
                if let PropKind::Enum(options) = prop.kind {
                    let default = prop.default_value();
                    let default = default.as_text().unwrap();
                    assert!(options.iter().any(|o| o.c == default), "{}.{} default", def.name, prop.key);
                }
                if prop.when == EmitWhen::NotEmpty {
                    assert!(matches!(
                        prop.kind,
                        PropKind::Text
                            | PropKind::Lines
                            | PropKind::Image
                            | PropKind::WidgetRef
                            | PropKind::Points
                            | PropKind::StrArray { .. }
                            | PropKind::ImageArray
                    ));
                }
            }
        }
    }

    #[test]
    fn templates_reference_known_props() {
        for def in ALL {
            for template in [def.create, def.post_create, def.post_props] {
                for key in placeholders(template) {
                    let key = key.split('.').next().unwrap();
                    let builtin = matches!(key, "parent" | "obj" | "w" | "h" | "component_create");
                    assert!(builtin || def.prop(key).is_some(), "{}: unknown placeholder {key}", def.name);
                }
            }
        }
    }

    fn placeholders(template: &str) -> Vec<&str> {
        template.split('{').skip(1).filter_map(|s| s.split_once('}').map(|(k, _)| k)).collect()
    }

    #[test]
    fn squareline_names_resolve() {
        assert_eq!(by_squareline_name("BUTTON"), Some(WidgetKind::Button));
        assert_eq!(by_squareline_name("container"), Some(WidgetKind::Container));
        assert_eq!(by_squareline_name("FOO"), None);
    }

    #[test]
    fn parent_child_rules() {
        assert!(def(WidgetKind::Tabview).accepts_child(WidgetKind::TabPage));
        assert!(!def(WidgetKind::Tabview).accepts_child(WidgetKind::Label));
        assert!(!def(WidgetKind::Panel).accepts_child(WidgetKind::TabPage));
        assert!(def(WidgetKind::Panel).accepts_child(WidgetKind::Label));
        assert!(!def(WidgetKind::Label).accepts_child(WidgetKind::Label));
        assert!(!def(WidgetKind::Screen).accepts_child(WidgetKind::Screen));
    }
}
