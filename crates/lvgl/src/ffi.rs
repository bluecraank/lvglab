//! The few C functions of the shim that Rust calls directly.

use std::ffi::c_int;

unsafe extern "C" {
    pub fn lvglab_init(w: c_int, h: c_int) -> c_int;
    pub fn lvglab_pixels() -> *const u8;
    pub fn lvglab_refresh();
}

// The LVGL 9 display and builders (vendor/lv9/shim9.c).
unsafe extern "C" {
    pub fn lvglab9_init(w: c_int, h: c_int) -> c_int;
    pub fn lvglab9_stride() -> c_int;
    pub fn lvglab9_pixels() -> *const u8;
    pub fn lvglab9_refresh();
    pub fn lvglab9_font_new(
        bitmap: *const u8,
        glyphs: *const crate::fonts::Glyph,
        glyph_count: u32,
        cmaps: *const crate::fonts::Cmap,
        cmap_count: u32,
        line_height: c_int,
        base_line: c_int,
        underline_position: c_int,
        underline_thickness: c_int,
        bpp: c_int,
    ) -> *mut std::ffi::c_void;
    pub fn lvglab9_font_free(font: *mut std::ffi::c_void);
    pub fn lvglab9_image_new(cf: u32, w: u32, h: u32, stride: u32, data: *const u8, size: u32)
    -> *mut std::ffi::c_void;
    pub fn lvglab9_image_free(image: *mut std::ffi::c_void);
}
