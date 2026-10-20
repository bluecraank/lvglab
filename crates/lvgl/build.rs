use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).expect("vendor directory").flatten().collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "c") {
            out.push(path);
        }
    }
}

fn main() {
    let vendor = PathBuf::from("vendor");
    let lvgl = vendor.join("lvgl83");
    let mut files = Vec::new();
    sources(&lvgl.join("src"), &mut files);
    files.push(vendor.join("shim.c"));
    files.push(vendor.join("names.c"));
    let mut build = cc::Build::new();
    build
        .files(&files)
        .include(&vendor)
        .include(&lvgl)
        .define("LV_CONF_INCLUDE_SIMPLE", None)
        .warnings(false)
        .opt_level(2);
    build.compile("lvgl83");

    // LVGL 9: every symbol gets the prefix lv9_ (rename9.h), so both releases link into one program.
    let v9 = vendor.join("lv9");
    let lvgl9 = vendor.join("lvgl96");
    let mut files = Vec::new();
    sources(&lvgl9.join("src"), &mut files);
    files.push(v9.join("shim9.c"));
    files.push(v9.join("names9.c"));
    files.push(v9.join("colors9.c"));
    let mut build = cc::Build::new();
    let rename = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join(&v9).join("rename9.h");
    if build.get_compiler().is_like_msvc() {
        build.flag(format!("/FI{}", rename.display()));
    } else {
        build.flag("-include").flag(rename.display().to_string());
    }
    build
        .files(&files)
        .include(&v9)
        .include(&lvgl9)
        .define("LV_CONF_INCLUDE_SIMPLE", None)
        .warnings(false)
        .opt_level(2);
    build.compile("lvgl96");
    println!("cargo:rerun-if-changed=vendor");
}
