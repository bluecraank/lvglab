//! Serde mirror of the parts of the `.spj` format the importer reads.
//! Unknown fields are ignored so newer SquareLine versions still load.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SpjFile {
    pub root: SpjNode,
    #[serde(default)]
    pub animations: Vec<serde_json::Value>,
    #[serde(default)]
    pub selected_screen: Option<String>,
    #[serde(default)]
    pub info: SpjInfo,
}

#[derive(Debug, Default, Deserialize)]
pub struct SpjInfo {
    pub name: Option<String>,
    #[serde(rename = "Name")]
    pub project_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub shape: Option<String>,
    pub board: Option<String>,
    pub lvgl_version: Option<String>,
    pub theme_dark: Option<bool>,
    pub theme_color1: Option<i64>,
    pub theme_color2: Option<i64>,
    pub custom_variable_prefix: Option<String>,
    /// `ENABLE` if texts are translated with lv_i18n.
    pub multilang: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SpjNode {
    pub guid: String,
    #[serde(rename = "saved_objtypeKey")]
    pub kind: String,
    #[serde(default)]
    pub properties: Vec<SpjProp>,
    #[serde(default)]
    pub children: Vec<SpjNode>,
}

#[derive(Debug, Deserialize)]
pub struct SpjProp {
    #[serde(default)]
    pub strtype: String,
    pub strval: Option<String>,
    pub integer: Option<i64>,
    pub intarray: Option<Vec<i64>>,
    pub flags: Option<i64>,
    pub part: Option<String>,
    #[serde(rename = "InheritedType", default)]
    pub inherited_type: i64,
    #[serde(default)]
    pub childs: Vec<SpjProp>,
    #[serde(flatten)]
    pub layout: SpjLayout,
}

/// Extra fields of `OBJECT/Layout_type`.
#[derive(Debug, Default, Deserialize)]
pub struct SpjLayout {
    #[serde(rename = "Flow")]
    pub flow: Option<i64>,
    #[serde(rename = "Wrap")]
    pub wrap: Option<bool>,
    #[serde(rename = "Reversed")]
    pub reversed: Option<bool>,
    #[serde(rename = "MainAlignment")]
    pub main_alignment: Option<i64>,
    #[serde(rename = "CrossAlignment")]
    pub cross_alignment: Option<i64>,
    #[serde(rename = "TrackAlignment")]
    pub track_alignment: Option<i64>,
    #[serde(rename = "LayoutType")]
    pub layout_type: Option<i64>,
}

/// A SquareLine font definition (`assets/ui_font_<name>.fcfg`).
#[derive(Debug, Deserialize)]
pub struct FontConfig {
    pub codename: String,
    pub ttf_path: String,
    pub size: u16,
    #[serde(default = "default_bpp")]
    pub bpp: u8,
    #[serde(default)]
    pub ranges: Vec<String>,
    #[serde(default)]
    pub symbols: String,
}

fn default_bpp() -> u8 {
    4
}

/// SquareLine's color themes (`Themes.slt` next to the project file).
#[derive(Debug, Deserialize)]
pub struct ThemesFile {
    pub deftheme: ThemeDef,
    #[serde(default)]
    pub themes: Vec<ThemeDef>,
    #[serde(default)]
    pub selected_theme: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ThemeDef {
    pub name: String,
    #[serde(default)]
    pub properties: Vec<ThemeColorDef>,
}

/// A theme variable; `nid` is what styles reference in `integer`.
#[derive(Debug, Deserialize)]
pub struct ThemeColorDef {
    pub nid: i64,
    #[serde(default)]
    pub strval: String,
    #[serde(default)]
    pub intarray: Vec<i64>,
}
