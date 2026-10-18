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
    println!("cargo:rerun-if-changed=vendor");
}
