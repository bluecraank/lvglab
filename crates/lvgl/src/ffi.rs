//! The few C functions of the shim that Rust calls directly.

use std::ffi::c_int;

unsafe extern "C" {
    pub fn lvglab_init(w: c_int, h: c_int) -> c_int;
    pub fn lvglab_pixels() -> *const u8;
    pub fn lvglab_refresh();
}
