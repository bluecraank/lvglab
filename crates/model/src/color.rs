use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An sRGB color as used by `lv_color_hex()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_hex(hex: u32) -> Self {
        Self::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    pub const fn to_hex(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }

    /// Parses `#RRGGBB`, `0xRRGGBB` or `RRGGBB`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let digits = text
            .strip_prefix('#')
            .or_else(|| text.strip_prefix("0x"))
            .or_else(|| text.strip_prefix("0X"))
            .unwrap_or(text);
        if digits.len() != 6 {
            return None;
        }
        u32::from_str_radix(digits, 16).ok().map(Self::from_hex)
    }

    /// Mixes `self` with `other`; `ratio` 255 yields `self`, 0 yields `other`
    /// (same semantics as `lv_color_mix`).
    pub fn mix(self, other: Self, ratio: u8) -> Self {
        let blend = |a: u8, b: u8| ((a as u16 * ratio as u16 + b as u16 * (255 - ratio as u16)) / 255) as u8;
        Self::rgb(blend(self.r, other.r), blend(self.g, other.g), blend(self.b, other.b))
    }

    pub fn lighten(self, ratio: u8) -> Self {
        Self::WHITE.mix(self, ratio)
    }

    pub fn darken(self, ratio: u8) -> Self {
        Self::BLACK.mix(self, ratio)
    }

    pub fn to_hex_string(self) -> String {
        format!("#{:06X}", self.to_hex())
    }

    /// C expression for this color.
    pub fn c_expr(self) -> String {
        format!("lv_color_hex(0x{:06X})", self.to_hex())
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex_string())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("invalid color `{text}`")))
    }
}

lv_enum! {
    /// The Material palette built into LVGL (`lv_palette_t`).
    pub enum Palette: "LV_PALETTE_" {
        Red = "RED", "Red";
        Pink = "PINK", "Pink";
        Purple = "PURPLE", "Purple";
        DeepPurple = "DEEP_PURPLE", "Deep purple";
        Indigo = "INDIGO", "Indigo";
        Blue = "BLUE", "Blue";
        LightBlue = "LIGHT_BLUE", "Light blue";
        Cyan = "CYAN", "Cyan";
        Teal = "TEAL", "Teal";
        Green = "GREEN", "Green";
        LightGreen = "LIGHT_GREEN", "Light green";
        Lime = "LIME", "Lime";
        Yellow = "YELLOW", "Yellow";
        Amber = "AMBER", "Amber";
        Orange = "ORANGE", "Orange";
        DeepOrange = "DEEP_ORANGE", "Deep orange";
        Brown = "BROWN", "Brown";
        BlueGrey = "BLUE_GREY", "Blue grey";
        Grey = "GREY", "Grey";
    }
}

impl Palette {
    /// `lv_palette_main()`.
    pub const fn main(self) -> Color {
        Color::from_hex(match self {
            Self::Red => 0xF44336,
            Self::Pink => 0xE91E63,
            Self::Purple => 0x9C27B0,
            Self::DeepPurple => 0x673AB7,
            Self::Indigo => 0x3F51B5,
            Self::Blue => 0x2196F3,
            Self::LightBlue => 0x03A9F4,
            Self::Cyan => 0x00BCD4,
            Self::Teal => 0x009688,
            Self::Green => 0x4CAF50,
            Self::LightGreen => 0x8BC34A,
            Self::Lime => 0xCDDC39,
            Self::Yellow => 0xFFEB3B,
            Self::Amber => 0xFFC107,
            Self::Orange => 0xFF9800,
            Self::DeepOrange => 0xFF5722,
            Self::Brown => 0x795548,
            Self::BlueGrey => 0x607D8B,
            Self::Grey => 0x9E9E9E,
        })
    }

    /// Index as used by `lv_palette_t` (and by SquareLine's theme settings).
    pub fn from_index(index: i64) -> Option<Self> {
        usize::try_from(index).ok().and_then(|i| Self::ALL.get(i).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_hex() {
        let c = Color::parse("#F15C00").unwrap();
        assert_eq!(c, Color::rgb(0xF1, 0x5C, 0x00));
        assert_eq!(c.to_hex_string(), "#F15C00");
        assert_eq!(c.c_expr(), "lv_color_hex(0xF15C00)");
        assert_eq!(Color::parse("0x00ff00"), Some(Color::rgb(0, 255, 0)));
        assert_eq!(Color::parse("nope"), None);
    }

    #[test]
    fn mix_matches_lvgl_semantics() {
        assert_eq!(Color::WHITE.mix(Color::BLACK, 255), Color::WHITE);
        assert_eq!(Color::WHITE.mix(Color::BLACK, 0), Color::BLACK);
    }
}
