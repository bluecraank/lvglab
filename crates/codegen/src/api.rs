//! The LVGL release the generated code targets. Templates are written for
//! LVGL 8.3; [`Target::port`] translates the names LVGL 9 renamed, so the
//! code uses the native API of the selected release (it also builds with
//! `LV_DISABLE_API_MAPPING`).

use lvglab_model::Project;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub major: u8,
    pub minor: u8,
}

impl Target {
    pub const V8: Self = Self { major: 8, minor: 3 };

    pub fn of(project: &Project) -> Self {
        let (major, minor) = project.lvgl_version.major_minor();
        Self { major, minor }
    }

    pub fn is_v9(self) -> bool {
        self.major >= 9
    }

    /// True if the target is LVGL 9 and at least 9.`minor`.
    pub fn v9_since(self, minor: u8) -> bool {
        self.major > 9 || (self.major == 9 && self.minor >= minor)
    }

    /// Translates LVGL 8 names in C code to the names of the target release.
    /// String literals and comments are left untouched.
    pub fn port(self, code: &str) -> String {
        if !self.is_v9() {
            return code.to_owned();
        }
        let mut out = String::with_capacity(code.len() + code.len() / 16);
        let bytes = code.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            if c == b'"' || c == b'\'' {
                let end = literal_end(bytes, i);
                out.push_str(&code[i..end]);
                i = end;
            } else if code[i..].starts_with("//") {
                let end = code[i..].find('\n').map_or(bytes.len(), |n| i + n);
                out.push_str(&code[i..end]);
                i = end;
            } else if code[i..].starts_with("/*") {
                let end = code[i + 2..].find("*/").map_or(bytes.len(), |n| i + n + 4);
                out.push_str(&code[i..end]);
                i = end;
            } else if c.is_ascii_alphabetic() || c == b'_' {
                let end = bytes[i..]
                    .iter()
                    .position(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
                    .map_or(bytes.len(), |n| i + n);
                out.push_str(&self.rename(&code[i..end]));
                i = end;
            } else {
                let len = code[i..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&code[i..i + len]);
                i += len;
            }
        }
        out
    }

    fn rename(self, ident: &str) -> String {
        if let Some((_, new, _)) = RENAMES.iter().find(|(old, _, since)| *old == ident && self.v9_since(*since)) {
            return (*new).to_owned();
        }
        for (old, new, since) in PREFIX_RENAMES {
            if self.v9_since(*since) {
                if let Some(rest) = ident.strip_prefix(old) {
                    return format!("{new}{rest}");
                }
            }
        }
        ident.to_owned()
    }
}

/// End of the string or character literal starting at `start`.
fn literal_end(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == quote => return i + 1,
            b'\n' => return i,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// Identifier renames as (LVGL 8 name, LVGL 9 name, first LVGL 9 minor
/// release using the new name). Taken from LVGL's `lv_api_map_v8.h` and
/// `lv_api_map_v9_*.h`, limited to names the generator emits.
const RENAMES: &[(&str, &str, u8)] = &[
    // Display and screens
    ("lv_disp_t", "lv_display_t", 0),
    ("lv_disp_get_default", "lv_display_get_default", 0),
    ("lv_disp_set_theme", "lv_display_set_theme", 0),
    ("lv_disp_load_scr", "lv_screen_load", 0),
    ("lv_scr_load", "lv_screen_load", 0),
    ("lv_scr_act", "lv_screen_active", 0),
    ("lv_scr_load_anim", "lv_screen_load_anim", 0),
    ("lv_scr_load_anim_t", "lv_screen_load_anim_t", 0),
    // Objects, events, memory
    ("lv_obj_del", "lv_obj_delete", 0),
    ("lv_obj_del_async", "lv_obj_delete_async", 0),
    ("lv_obj_clear_flag", "lv_obj_remove_flag", 0),
    ("lv_obj_clear_state", "lv_obj_remove_state", 0),
    ("lv_obj_is_valid", "lv_obj_is_in_widget_tree", 6),
    ("lv_indev_get_act", "lv_indev_active", 0),
    ("lv_event_send", "lv_obj_send_event", 0),
    ("lv_mem_alloc", "lv_malloc", 0),
    ("lv_mem_free", "lv_free", 0),
    ("lv_memset_00", "lv_memzero", 0),
    ("lv_coord_t", "int32_t", 0),
    ("lv_point_t", "lv_point_precise_t", 0),
    // Images
    ("lv_img_dsc_t", "lv_image_dsc_t", 0),
    ("LV_IMG_DECLARE", "LV_IMAGE_DECLARE", 0),
    ("lv_img_create", "lv_image_create", 0),
    ("lv_img_set_src", "lv_image_set_src", 0),
    ("lv_img_set_offset_x", "lv_image_set_offset_x", 0),
    ("lv_img_set_offset_y", "lv_image_set_offset_y", 0),
    ("lv_img_set_angle", "lv_image_set_rotation", 0),
    ("lv_img_set_pivot", "lv_image_set_pivot", 0),
    ("lv_img_set_zoom", "lv_image_set_scale", 0),
    ("lv_img_set_antialias", "lv_image_set_antialias", 0),
    ("lv_imgbtn_create", "lv_imagebutton_create", 0),
    ("lv_imgbtn_set_src", "lv_imagebutton_set_src", 0),
    // Widgets
    ("lv_btn_create", "lv_button_create", 0),
    ("lv_list_add_btn", "lv_list_add_button", 0),
    ("lv_tabview_get_tab_btns", "lv_tabview_get_tab_bar", 0),
    ("lv_table_get_col_cnt", "lv_table_get_column_count", 0),
    ("lv_table_set_col_width", "lv_table_set_column_width", 0),
    ("lv_roller_set_visible_row_cnt", "lv_roller_set_visible_row_count", 0),
    ("lv_menu_set_mode_root_back_btn", "lv_menu_set_mode_root_back_button", 0),
    ("LV_MENU_ROOT_BACK_BTN_DISABLED", "LV_MENU_ROOT_BACK_BUTTON_DISABLED", 0),
    ("LV_MENU_ROOT_BACK_BTN_ENABLED", "LV_MENU_ROOT_BACK_BUTTON_ENABLED", 0),
    ("lv_msgbox_get_btns", "lv_msgbox_get_footer", 0),
    ("lv_msgbox_get_text", "lv_msgbox_get_content", 0),
    ("lv_spangroup_new_span", "lv_spangroup_add_span", 3),
    ("lv_spangroup_refr_mode", "lv_spangroup_refresh", 3),
    ("lv_slider_set_left_value", "lv_slider_set_start_value", 3),
    ("lv_calendar_header_arrow_create", "lv_calendar_add_header_arrow", 3),
    ("lv_calendar_header_dropdown_create", "lv_calendar_add_header_dropdown", 3),
    ("lv_calendar_set_showed_date", "lv_calendar_set_month_shown", 3),
    ("lv_chart_set_range", "lv_chart_set_axis_range", 3),
    ("lv_chart_set_ext_y_array", "lv_chart_set_series_ext_y_array", 3),
    ("LV_LABEL_LONG_WRAP", "LV_LABEL_LONG_MODE_WRAP", 3),
    ("LV_LABEL_LONG_DOT", "LV_LABEL_LONG_MODE_DOTS", 3),
    ("LV_LABEL_LONG_SCROLL", "LV_LABEL_LONG_MODE_SCROLL", 3),
    ("LV_LABEL_LONG_SCROLL_CIRCULAR", "LV_LABEL_LONG_MODE_SCROLL_CIRCULAR", 3),
    ("LV_LABEL_LONG_CLIP", "LV_LABEL_LONG_MODE_CLIP", 3),
    // Style properties
    ("lv_obj_set_style_anim_time", "lv_obj_set_style_anim_duration", 0),
    ("lv_obj_set_style_img_opa", "lv_obj_set_style_image_opa", 0),
    ("lv_obj_set_style_img_recolor", "lv_obj_set_style_image_recolor", 0),
    ("lv_obj_set_style_img_recolor_opa", "lv_obj_set_style_image_recolor_opa", 0),
    ("lv_obj_set_style_shadow_ofs_x", "lv_obj_set_style_shadow_offset_x", 0),
    ("lv_obj_set_style_shadow_ofs_y", "lv_obj_set_style_shadow_offset_y", 0),
    ("lv_obj_set_style_transform_zoom", "lv_obj_set_style_transform_scale", 0),
    ("lv_obj_set_style_transform_angle", "lv_obj_set_style_transform_rotation", 0),
    ("lv_obj_set_style_bg_img_src", "lv_obj_set_style_bg_image_src", 0),
    ("lv_obj_set_style_bg_img_opa", "lv_obj_set_style_bg_image_opa", 0),
    ("lv_obj_set_style_bg_img_recolor", "lv_obj_set_style_bg_image_recolor", 0),
    ("lv_obj_set_style_bg_img_recolor_opa", "lv_obj_set_style_bg_image_recolor_opa", 0),
    ("lv_obj_set_style_bg_img_tiled", "lv_obj_set_style_bg_image_tiled", 0),
    ("lv_obj_set_style_arc_img_src", "lv_obj_set_style_arc_image_src", 0),
    ("LV_STYLE_ANIM_TIME", "LV_STYLE_ANIM_DURATION", 0),
    ("LV_STYLE_IMG_OPA", "LV_STYLE_IMAGE_OPA", 0),
    ("LV_STYLE_IMG_RECOLOR", "LV_STYLE_IMAGE_RECOLOR", 0),
    ("LV_STYLE_IMG_RECOLOR_OPA", "LV_STYLE_IMAGE_RECOLOR_OPA", 0),
    ("LV_STYLE_SHADOW_OFS_X", "LV_STYLE_SHADOW_OFFSET_X", 0),
    ("LV_STYLE_SHADOW_OFS_Y", "LV_STYLE_SHADOW_OFFSET_Y", 0),
    ("LV_STYLE_TRANSFORM_ANGLE", "LV_STYLE_TRANSFORM_ROTATION", 0),
    ("LV_STYLE_BG_IMG_SRC", "LV_STYLE_BG_IMAGE_SRC", 0),
    ("LV_STYLE_BG_IMG_OPA", "LV_STYLE_BG_IMAGE_OPA", 0),
    ("LV_STYLE_BG_IMG_RECOLOR", "LV_STYLE_BG_IMAGE_RECOLOR", 0),
    ("LV_STYLE_BG_IMG_RECOLOR_OPA", "LV_STYLE_BG_IMAGE_RECOLOR_OPA", 0),
    ("LV_STYLE_BG_IMG_TILED", "LV_STYLE_BG_IMAGE_TILED", 0),
    ("LV_STYLE_ARC_IMG_SRC", "LV_STYLE_ARC_IMAGE_SRC", 0),
];

/// Renames of whole name families as (old prefix, new prefix, since).
const PREFIX_RENAMES: &[(&str, &str, u8)] = &[
    ("lv_btnmatrix_", "lv_buttonmatrix_", 0),
    ("LV_BTNMATRIX_CTRL_", "LV_BUTTONMATRIX_CTRL_", 0),
    ("LV_IMGBTN_STATE_", "LV_IMAGEBUTTON_STATE_", 0),
    ("LV_SCR_LOAD_ANIM_", "LV_SCREEN_LOAD_ANIM_", 4),
];

#[cfg(test)]
mod tests {
    use super::*;

    const V9_2: Target = Target { major: 9, minor: 2 };
    const V9_6: Target = Target { major: 9, minor: 6 };

    #[test]
    fn ports_names_but_not_strings_or_comments() {
        let code = "lv_obj_t * b = lv_btn_create(p); /* lv_btn_create */ lv_label_set_text(l, \"lv_btn_create\");";
        let ported = V9_2.port(code);
        assert_eq!(
            ported,
            "lv_obj_t * b = lv_button_create(p); /* lv_btn_create */ lv_label_set_text(l, \"lv_btn_create\");"
        );
        assert_eq!(Target::V8.port(code), code);
    }

    #[test]
    fn renames_depend_on_the_minor_release() {
        assert_eq!(V9_2.port("LV_LABEL_LONG_DOT"), "LV_LABEL_LONG_DOT");
        assert_eq!(V9_6.port("LV_LABEL_LONG_DOT"), "LV_LABEL_LONG_MODE_DOTS");
        assert_eq!(V9_2.port("LV_SCR_LOAD_ANIM_FADE_ON"), "LV_SCR_LOAD_ANIM_FADE_ON");
        assert_eq!(V9_6.port("LV_SCR_LOAD_ANIM_FADE_ON"), "LV_SCREEN_LOAD_ANIM_FADE_ON");
        assert_eq!(V9_2.port("lv_btnmatrix_set_map(m, x);"), "lv_buttonmatrix_set_map(m, x);");
        assert_eq!(V9_2.port("lv_scr_load_anim_t a; char c = '\"';"), "lv_screen_load_anim_t a; char c = '\"';");
    }
}
