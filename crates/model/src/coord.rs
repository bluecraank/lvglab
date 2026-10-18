use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Largest coordinate the layout works with; LVGL 8 coordinates are 16 bit.
pub const COORD_LIMIT: i32 = 32_767;

/// Clamps a pixel value into `±COORD_LIMIT`, so sums and products of a few
/// coordinates cannot overflow.
pub fn clamp_px(value: impl Into<i64>) -> i32 {
    value.into().clamp(-i64::from(COORD_LIMIT), i64::from(COORD_LIMIT)) as i32
}

/// Unit of a position or size value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Unit {
    #[default]
    Px,
    /// Percentage of the parent's content area (`lv_pct()`).
    Percent,
    /// `LV_SIZE_CONTENT`; only meaningful for sizes.
    Content,
}

impl Unit {
    pub const ALL: [Self; 3] = [Self::Px, Self::Percent, Self::Content];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Px => "px",
            Self::Percent => "%",
            Self::Content => "content",
        }
    }
}

/// A coordinate or size together with its unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Coord {
    pub value: i32,
    pub unit: Unit,
}

impl Coord {
    pub const fn px(value: i32) -> Self {
        Self { value, unit: Unit::Px }
    }

    pub const fn pct(value: i32) -> Self {
        Self { value, unit: Unit::Percent }
    }

    pub const fn content() -> Self {
        Self { value: 0, unit: Unit::Content }
    }

    /// Resolves the coordinate against a reference length in pixels.
    /// `content` must be supplied by the caller for [`Unit::Content`]. The
    /// result stays within [`COORD_LIMIT`], whatever a project file holds.
    pub fn resolve(self, reference: i32, content: i32) -> i32 {
        let px = match self.unit {
            Unit::Px => i64::from(self.value),
            Unit::Percent => i64::from(reference) * i64::from(self.value) / 100,
            Unit::Content => i64::from(content),
        };
        clamp_px(px)
    }

    /// C expression for the value (`42`, `lv_pct(50)`, `LV_SIZE_CONTENT`).
    pub fn c_expr(self) -> String {
        match self.unit {
            Unit::Px => self.value.to_string(),
            Unit::Percent => format!("lv_pct({})", self.value),
            Unit::Content => "LV_SIZE_CONTENT".to_owned(),
        }
    }
}

impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.unit {
            Unit::Px => write!(f, "{}", self.value),
            Unit::Percent => write!(f, "{}%", self.value),
            Unit::Content => f.write_str("content"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid coordinate `{0}`")]
pub struct ParseCoordError(String);

impl FromStr for Coord {
    type Err = ParseCoordError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("content") {
            return Ok(Self::content());
        }
        let (number, unit) = match text.strip_suffix('%') {
            Some(number) => (number, Unit::Percent),
            None => (text.strip_suffix("px").unwrap_or(text), Unit::Px),
        };
        number.trim().parse().map(|value| Self { value, unit }).map_err(|_| ParseCoordError(text.to_owned()))
    }
}

impl Serialize for Coord {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Coord {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

lv_enum! {
    /// `lv_align_t`.
    #[derive(Default)]
    pub enum Align: "LV_ALIGN_" {
        Default = "DEFAULT", "Default";
        #[default]
        TopLeft = "TOP_LEFT", "Top left";
        TopMid = "TOP_MID", "Top middle";
        TopRight = "TOP_RIGHT", "Top right";
        BottomLeft = "BOTTOM_LEFT", "Bottom left";
        BottomMid = "BOTTOM_MID", "Bottom middle";
        BottomRight = "BOTTOM_RIGHT", "Bottom right";
        LeftMid = "LEFT_MID", "Left middle";
        RightMid = "RIGHT_MID", "Right middle";
        Center = "CENTER", "Center";
        OutTopLeft = "OUT_TOP_LEFT", "Outside top left";
        OutTopMid = "OUT_TOP_MID", "Outside top middle";
        OutTopRight = "OUT_TOP_RIGHT", "Outside top right";
        OutBottomLeft = "OUT_BOTTOM_LEFT", "Outside bottom left";
        OutBottomMid = "OUT_BOTTOM_MID", "Outside bottom middle";
        OutBottomRight = "OUT_BOTTOM_RIGHT", "Outside bottom right";
        OutLeftTop = "OUT_LEFT_TOP", "Outside left top";
        OutLeftMid = "OUT_LEFT_MID", "Outside left middle";
        OutLeftBottom = "OUT_LEFT_BOTTOM", "Outside left bottom";
        OutRightTop = "OUT_RIGHT_TOP", "Outside right top";
        OutRightMid = "OUT_RIGHT_MID", "Outside right middle";
        OutRightBottom = "OUT_RIGHT_BOTTOM", "Outside right bottom";
    }
}

impl Align {
    /// The nine alignments inside the parent, row by row.
    pub const INSIDE_GRID: [Self; 9] = [
        Self::TopLeft,
        Self::TopMid,
        Self::TopRight,
        Self::LeftMid,
        Self::Center,
        Self::RightMid,
        Self::BottomLeft,
        Self::BottomMid,
        Self::BottomRight,
    ];

    /// Computes the top-left corner of a child of size `child` aligned within
    /// (or around) a parent area, before the child's own offset is applied.
    /// `inner` is the parent's content area, `outer` its full area; both are
    /// `(x, y, w, h)` tuples.
    pub fn anchor(self, inner: (i32, i32, i32, i32), outer: (i32, i32, i32, i32), child: (i32, i32)) -> (i32, i32) {
        let (ix, iy, iw, ih) = inner;
        let (ox, oy, ow, oh) = outer;
        let (cw, ch) = child;
        let center_x = |x: i32, w: i32| x + (w - cw) / 2;
        let center_y = |y: i32, h: i32| y + (h - ch) / 2;
        match self {
            Self::Default | Self::TopLeft => (ix, iy),
            Self::TopMid => (center_x(ix, iw), iy),
            Self::TopRight => (ix + iw - cw, iy),
            Self::BottomLeft => (ix, iy + ih - ch),
            Self::BottomMid => (center_x(ix, iw), iy + ih - ch),
            Self::BottomRight => (ix + iw - cw, iy + ih - ch),
            Self::LeftMid => (ix, center_y(iy, ih)),
            Self::RightMid => (ix + iw - cw, center_y(iy, ih)),
            Self::Center => (center_x(ix, iw), center_y(iy, ih)),
            Self::OutTopLeft => (ox, oy - ch),
            Self::OutTopMid => (center_x(ox, ow), oy - ch),
            Self::OutTopRight => (ox + ow - cw, oy - ch),
            Self::OutBottomLeft => (ox, oy + oh),
            Self::OutBottomMid => (center_x(ox, ow), oy + oh),
            Self::OutBottomRight => (ox + ow - cw, oy + oh),
            Self::OutLeftTop => (ox - cw, oy),
            Self::OutLeftMid => (ox - cw, center_y(oy, oh)),
            Self::OutLeftBottom => (ox - cw, oy + oh - ch),
            Self::OutRightTop => (ox + ow, oy),
            Self::OutRightMid => (ox + ow, center_y(oy, oh)),
            Self::OutRightBottom => (ox + ow, oy + oh - ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_round_trips_through_text() {
        for text in ["12", "-7", "50%", "content"] {
            let coord: Coord = text.parse().unwrap();
            assert_eq!(coord.to_string(), text);
        }
        assert_eq!("10px".parse::<Coord>().unwrap(), Coord::px(10));
        assert!("abc".parse::<Coord>().is_err());
    }

    #[test]
    fn coord_c_expressions() {
        assert_eq!(Coord::px(3).c_expr(), "3");
        assert_eq!(Coord::pct(50).c_expr(), "lv_pct(50)");
        assert_eq!(Coord::content().c_expr(), "LV_SIZE_CONTENT");
    }

    #[test]
    fn align_parses_squareline_names() {
        assert_eq!(Align::from_suffix("CENTER"), Some(Align::Center));
        assert_eq!(Align::from_suffix("LV_ALIGN_BOTTOM_MID"), Some(Align::BottomMid));
        assert_eq!(Align::Center.c_const(), "LV_ALIGN_CENTER");
    }

    #[test]
    fn align_anchor_centers_child() {
        let area = (0, 0, 100, 50);
        assert_eq!(Align::Center.anchor(area, area, (20, 10)), (40, 20));
        assert_eq!(Align::BottomRight.anchor(area, area, (20, 10)), (80, 40));
        assert_eq!(Align::OutTopLeft.anchor(area, area, (20, 10)), (0, -10));
    }
}
