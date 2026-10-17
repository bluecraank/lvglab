//! Object flags, states and scroll/flex related enumerations of LVGL 8.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

lv_enum! {
    /// `lv_obj_flag_t`.
    pub enum ObjFlag: "LV_OBJ_FLAG_" {
        Hidden = "HIDDEN", "Hidden";
        Clickable = "CLICKABLE", "Clickable";
        ClickFocusable = "CLICK_FOCUSABLE", "Click focusable";
        Checkable = "CHECKABLE", "Checkable";
        Scrollable = "SCROLLABLE", "Scrollable";
        ScrollElastic = "SCROLL_ELASTIC", "Scroll elastic";
        ScrollMomentum = "SCROLL_MOMENTUM", "Scroll momentum";
        ScrollOne = "SCROLL_ONE", "Scroll one";
        ScrollChainHor = "SCROLL_CHAIN_HOR", "Scroll chain horizontal";
        ScrollChainVer = "SCROLL_CHAIN_VER", "Scroll chain vertical";
        ScrollOnFocus = "SCROLL_ON_FOCUS", "Scroll on focus";
        ScrollWithArrow = "SCROLL_WITH_ARROW", "Scroll with arrow";
        Snappable = "SNAPPABLE", "Snappable";
        PressLock = "PRESS_LOCK", "Press lock";
        EventBubble = "EVENT_BUBBLE", "Event bubble";
        GestureBubble = "GESTURE_BUBBLE", "Gesture bubble";
        AdvHittest = "ADV_HITTEST", "Advanced hit test";
        IgnoreLayout = "IGNORE_LAYOUT", "Ignore layout";
        Floating = "FLOATING", "Floating";
        OverflowVisible = "OVERFLOW_VISIBLE", "Overflow visible";
        FlexInNewTrack = "FLEX_IN_NEW_TRACK", "Start new flex track";
    }
}

impl ObjFlag {
    /// Flags every `lv_obj` has after `lv_obj_constructor`.
    pub const OBJ_DEFAULTS: &'static [Self] = &[
        Self::Clickable,
        Self::ClickFocusable,
        Self::Scrollable,
        Self::ScrollElastic,
        Self::ScrollMomentum,
        Self::ScrollWithArrow,
        Self::ScrollChainHor,
        Self::ScrollChainVer,
        Self::Snappable,
        Self::PressLock,
        Self::GestureBubble,
    ];
}

lv_enum! {
    /// `lv_state_t` (single states).
    pub enum ObjState: "LV_STATE_" {
        Checked = "CHECKED", "Checked";
        Focused = "FOCUSED", "Focused";
        FocusKey = "FOCUS_KEY", "Focus key";
        Edited = "EDITED", "Edited";
        Hovered = "HOVERED", "Hovered";
        Pressed = "PRESSED", "Pressed";
        Scrolled = "SCROLLED", "Scrolled";
        Disabled = "DISABLED", "Disabled";
        User1 = "USER_1", "User 1";
        User2 = "USER_2", "User 2";
        User3 = "USER_3", "User 3";
        User4 = "USER_4", "User 4";
    }
}

impl ObjState {
    pub const fn bits(self) -> u16 {
        match self {
            Self::Checked => 0x0001,
            Self::Focused => 0x0002,
            Self::FocusKey => 0x0004,
            Self::Edited => 0x0008,
            Self::Hovered => 0x0010,
            Self::Pressed => 0x0020,
            Self::Scrolled => 0x0040,
            Self::Disabled => 0x0080,
            Self::User1 => 0x1000,
            Self::User2 => 0x2000,
            Self::User3 => 0x4000,
            Self::User4 => 0x8000,
        }
    }
}

/// A combination of states, used as part of a style selector.
/// The empty mask is `LV_STATE_DEFAULT`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateMask(pub u16);

impl StateMask {
    pub const DEFAULT: Self = Self(0);

    pub fn contains(self, state: ObjState) -> bool {
        self.0 & state.bits() != 0
    }

    pub fn with(self, state: ObjState) -> Self {
        Self(self.0 | state.bits())
    }

    pub fn toggled(self, state: ObjState) -> Self {
        Self(self.0 ^ state.bits())
    }

    pub fn states(self) -> impl Iterator<Item = ObjState> {
        ObjState::ALL.iter().copied().filter(move |s| self.contains(*s))
    }

    /// True if every state of `self` is active in `active`, i.e. a style with
    /// this selector applies to an object in state `active`.
    pub fn matches(self, active: Self) -> bool {
        self.0 & !active.0 == 0
    }

    /// Number of states in the mask; LVGL prefers more specific selectors.
    pub fn specificity(self) -> u32 {
        self.0.count_ones()
    }

    /// C expression, e.g. `LV_STATE_DEFAULT` or `LV_STATE_CHECKED | LV_STATE_PRESSED`.
    pub fn c_expr(self) -> String {
        if self.0 == 0 {
            return "LV_STATE_DEFAULT".to_owned();
        }
        self.states().map(ObjState::c_const).collect::<Vec<_>>().join(" | ")
    }

    /// Parses `DEFAULT`, `CHECKED|PRESSED`, `LV_STATE_CHECKED, LV_STATE_FOCUSED`, ...
    pub fn parse(text: &str) -> Option<Self> {
        let mut mask = Self::DEFAULT;
        for token in text.split(['|', ',', '+', ' ']).map(str::trim).filter(|t| !t.is_empty()) {
            if token.eq_ignore_ascii_case("DEFAULT") || token.eq_ignore_ascii_case("LV_STATE_DEFAULT") {
                continue;
            }
            mask = mask.with(ObjState::from_suffix(token)?);
        }
        Some(mask)
    }
}

impl fmt::Display for StateMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("DEFAULT");
        }
        let names: Vec<_> = self.states().map(ObjState::suffix).collect();
        f.write_str(&names.join("|"))
    }
}

impl Serialize for StateMask {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for StateMask {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("invalid state `{text}`")))
    }
}

lv_enum! {
    /// `lv_scrollbar_mode_t`.
    pub enum ScrollbarMode: "LV_SCROLLBAR_MODE_" {
        Off = "OFF", "Off";
        On = "ON", "On";
        Active = "ACTIVE", "Active";
        Auto = "AUTO", "Auto";
    }
}

lv_enum! {
    /// `lv_dir_t` as used for scrolling.
    pub enum Dir: "LV_DIR_" {
        None = "NONE", "None";
        Left = "LEFT", "Left";
        Right = "RIGHT", "Right";
        Top = "TOP", "Top";
        Bottom = "BOTTOM", "Bottom";
        Hor = "HOR", "Horizontal";
        Ver = "VER", "Vertical";
        All = "ALL", "All";
    }
}

lv_enum! {
    /// `lv_scroll_snap_t`.
    pub enum SnapMode: "LV_SCROLL_SNAP_" {
        None = "NONE", "None";
        Start = "START", "Start";
        End = "END", "End";
        Center = "CENTER", "Center";
    }
}

lv_enum! {
    /// `lv_flex_flow_t`.
    pub enum FlexFlow: "LV_FLEX_FLOW_" {
        Row = "ROW", "Row";
        Column = "COLUMN", "Column";
        RowWrap = "ROW_WRAP", "Row, wrap";
        RowReverse = "ROW_REVERSE", "Row, reverse";
        RowWrapReverse = "ROW_WRAP_REVERSE", "Row, wrap, reverse";
        ColumnWrap = "COLUMN_WRAP", "Column, wrap";
        ColumnReverse = "COLUMN_REVERSE", "Column, reverse";
        ColumnWrapReverse = "COLUMN_WRAP_REVERSE", "Column, wrap, reverse";
    }
}

impl FlexFlow {
    pub fn is_row(self) -> bool {
        matches!(self, Self::Row | Self::RowWrap | Self::RowReverse | Self::RowWrapReverse)
    }

    pub fn wraps(self) -> bool {
        matches!(self, Self::RowWrap | Self::RowWrapReverse | Self::ColumnWrap | Self::ColumnWrapReverse)
    }

    pub fn is_reversed(self) -> bool {
        matches!(self, Self::RowReverse | Self::RowWrapReverse | Self::ColumnReverse | Self::ColumnWrapReverse)
    }

    pub fn compose(row: bool, wrap: bool, reverse: bool) -> Self {
        match (row, wrap, reverse) {
            (true, false, false) => Self::Row,
            (true, true, false) => Self::RowWrap,
            (true, false, true) => Self::RowReverse,
            (true, true, true) => Self::RowWrapReverse,
            (false, false, false) => Self::Column,
            (false, true, false) => Self::ColumnWrap,
            (false, false, true) => Self::ColumnReverse,
            (false, true, true) => Self::ColumnWrapReverse,
        }
    }
}

lv_enum! {
    /// `lv_flex_align_t`.
    pub enum FlexAlign: "LV_FLEX_ALIGN_" {
        Start = "START", "Start";
        End = "END", "End";
        Center = "CENTER", "Center";
        SpaceEvenly = "SPACE_EVENLY", "Space evenly";
        SpaceAround = "SPACE_AROUND", "Space around";
        SpaceBetween = "SPACE_BETWEEN", "Space between";
    }
}

impl FlexAlign {
    /// Index as stored in `lv_flex_align_t` (and SquareLine files).
    pub fn from_index(index: i64) -> Self {
        usize::try_from(index).ok().and_then(|i| Self::ALL.get(i).copied()).unwrap_or(Self::Start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_mask_parsing_and_formatting() {
        let mask = StateMask::parse("CHECKED|PRESSED").unwrap();
        assert!(mask.contains(ObjState::Checked));
        assert!(mask.contains(ObjState::Pressed));
        assert_eq!(mask.c_expr(), "LV_STATE_CHECKED | LV_STATE_PRESSED");
        assert_eq!(StateMask::parse("DEFAULT"), Some(StateMask::DEFAULT));
        assert_eq!(StateMask::DEFAULT.c_expr(), "LV_STATE_DEFAULT");
        assert_eq!(StateMask::parse("bogus"), None);
    }

    #[test]
    fn state_mask_matching() {
        let checked = StateMask::DEFAULT.with(ObjState::Checked);
        let checked_pressed = checked.with(ObjState::Pressed);
        assert!(StateMask::DEFAULT.matches(checked));
        assert!(checked.matches(checked_pressed));
        assert!(!checked_pressed.matches(checked));
    }

    #[test]
    fn flex_flow_composition() {
        for flow in FlexFlow::ALL {
            assert_eq!(FlexFlow::compose(flow.is_row(), flow.wraps(), flow.is_reversed()), *flow);
        }
    }
}
