//! Embeds the program icon and version information into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/icon.ico")
            .set("ProductName", "LVGLab")
            .set("FileDescription", "LVGLab - UI designer for LVGL 8")
            .set("LegalCopyright", "MIT License, blueCraanK");
        if let Err(err) = resource.compile() {
            println!("cargo:warning=the program icon was not embedded: {err}");
        }
    }
}
