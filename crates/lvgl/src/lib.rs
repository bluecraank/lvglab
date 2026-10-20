//! LVGL 8.3 as a preview renderer: the code the export generates for a screen
//! runs in a small interpreter against the LVGL that is compiled into the
//! program. LVGL draws it in software into a buffer and the editor shows that
//! buffer.

mod assets;
mod ffi;
mod fonts;
mod interp;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

use lvglab_model::Project;

pub use interp::{Interpreter, Tables};

/// A rendered screen: `width` × `height` pixels, RGBA, row by row.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Statements of the generated code the preview could not run.
    pub skipped: Vec<String>,
    /// Objects LVGL created on the screen and widgets the project has there.
    pub created: usize,
    pub expected: usize,
}

/// LVGL has global state, so only one render runs at a time. The value is the
/// size of the display and the empty screen that stays loaded between renders.
static LVGL: Mutex<Option<((i32, i32), usize)>> = Mutex::new(None);
/// The same for LVGL 9, which is a separate library in the same program.
static LVGL9: Mutex<Option<((i32, i32), usize)>> = Mutex::new(None);

type Cache<T> = Mutex<Option<HashMap<u64, Vec<T>>>>;

static IMAGES: Cache<assets::Image> = Mutex::new(None);
static FONTS: Cache<fonts::Font> = Mutex::new(None);
static IMAGES9: Cache<assets::Image9> = Mutex::new(None);

/// Parses each generated file once: images and fonts are large tables of
/// numbers that rarely change, while the preview renders on every edit.
fn parsed<T: Clone>(cache: &Cache<T>, sources: &[&str], salt: u64, parse: impl Fn(&str) -> Vec<T>) -> Vec<T> {
    let mut out = Vec::new();
    let Ok(mut guard) = cache.lock() else { return sources.iter().flat_map(|s| parse(s)).collect() };
    let map = guard.get_or_insert_with(HashMap::new);
    if map.len() > 256 {
        map.clear();
    }
    for source in sources {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hasher);
        salt.hash(&mut hasher);
        let key = hasher.finish();
        out.extend(map.entry(key).or_insert_with(|| parse(source)).iter().cloned());
    }
    out
}

/// Files of the export that are of no use to the preview (assets, callbacks).
fn is_code(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.ends_with(".c")
        && !name.starts_with("ui_img_")
        && !name.starts_with("ui_font_")
        && !name.starts_with("ui_events")
        && name != "ui_helpers.c"
}

/// Renders screen number `index` of the project with the LVGL release the
/// project is made for: LVGL 8.3, or LVGL 9.6 for all LVGL 9 projects.
pub fn render_screen(project: &Project, index: usize, project_dir: Option<&Path>) -> Result<Frame, String> {
    let output = lvglab_codegen::generate_ui(project, project_dir);
    let screen_init =
        screen_init_names(&output).get(index).cloned().ok_or_else(|| "this screen has no generated code".to_owned())?;
    let expected = project.screens.get(index).map_or(0, |screen| {
        let mut count = 0;
        project.expand_instances(screen).walk(&mut |_| count += 1);
        count
    });
    let (width, height) = (project.display.width as i32, project.display.height as i32);
    if project.lvgl_version.is_v9() {
        return render_v9(&output, screen_init, expected, width, height);
    }
    let mut state = LVGL.lock().map_err(|_| "LVGL is busy".to_owned())?;
    // SAFETY: LVGL is only used while the lock above is held.
    unsafe {
        if ffi::lvglab_init(width, height) == 0 {
            return Err("out of memory".to_owned());
        }
        if state.is_none_or(|(size, _)| size != (width, height)) {
            *state = Some(((width, height), 0));
        }
        let mut interpreter = Interpreter::new(Tables::v8());
        for file in output.files.iter().filter(|f| is_code(&f.path)) {
            interpreter.load(file.contents_str());
        }
        let sources: Vec<&str> = output
            .files
            .iter()
            .filter(|f| f.path.rsplit('/').next().is_some_and(|n| n.starts_with("ui_img_") && n.ends_with(".c")))
            .map(lvglab_codegen::GeneratedFile::contents_str)
            .collect();
        let mut images = parsed(&IMAGES, &sources, u64::from(project.export.swap_bytes), |s| {
            assets::parse_images(&[s], project.export.swap_bytes)
        });
        for image in &mut images {
            let address = image.address();
            interpreter.add_symbol(&image.name, address);
        }
        let font_sources: Vec<&str> = output
            .files
            .iter()
            .filter(|f| f.path.rsplit('/').next().is_some_and(|n| n.starts_with("ui_font_") && n.ends_with(".c")))
            .map(lvglab_codegen::GeneratedFile::contents_str)
            .collect();
        let mut fonts = parsed(&FONTS, &font_sources, 0, |s| fonts::parse_fonts(&[s]));
        if let (Some(glyph_dsc), Some(glyph_bitmap)) =
            (interpreter.symbol("lv_font_get_glyph_dsc_fmt_txt"), interpreter.symbol("lv_font_get_bitmap_fmt_txt"))
        {
            for font in &mut fonts {
                let address = font.address(glyph_dsc, glyph_bitmap);
                interpreter.add_symbol(&font.name, address);
            }
        }
        interpreter.only_screen = Some(screen_init);
        interpreter.call("ui_init", &[]);
        let screen = *interpreter.screens.first().ok_or_else(|| "the screen was not created".to_owned())?;
        interpreter.call_lvgl("lv_disp_load_scr", &[screen]);
        let created = interpreter.count_objects(screen);
        ffi::lvglab_refresh();
        let pixels = std::slice::from_raw_parts(ffi::lvglab_pixels(), width as usize * height as usize * 4);
        let mut rgba = Vec::with_capacity(pixels.len());
        for p in pixels.chunks_exact(4) {
            rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
        }
        // Show an empty screen, then free the objects of this render.
        let blank = interpreter.call_lvgl("lv_obj_create", &[0]).unwrap_or(0);
        interpreter.call_lvgl("lv_disp_load_scr", &[blank]);
        for screen in interpreter.screens.clone() {
            interpreter.call_lvgl("lv_obj_del", &[screen]);
        }
        if let Some(entry) = state.as_mut() {
            let old = std::mem::replace(&mut entry.1, blank as usize);
            if old != 0 {
                interpreter.call_lvgl("lv_obj_del", &[old as u64]);
            }
        }
        Ok(Frame { width: width as u32, height: height as u32, rgba, skipped: interpreter.skipped, created, expected })
    }
}

fn sources_of<'a>(output: &'a lvglab_codegen::Output, prefix: &str) -> Vec<&'a str> {
    output
        .files
        .iter()
        .filter(|f| f.path.rsplit('/').next().is_some_and(|n| n.starts_with(prefix) && n.ends_with(".c")))
        .map(lvglab_codegen::GeneratedFile::contents_str)
        .collect()
}

/// LVGL 9: the same steps with the names and structs of LVGL 9.
fn render_v9(
    output: &lvglab_codegen::Output,
    screen_init: String,
    expected: usize,
    width: i32,
    height: i32,
) -> Result<Frame, String> {
    let mut state = LVGL9.lock().map_err(|_| "LVGL is busy".to_owned())?;
    // SAFETY: LVGL 9 is only used while the lock above is held. Fonts and
    // images are freed after the objects that use them were deleted.
    unsafe {
        if ffi::lvglab9_init(width, height) == 0 {
            return Err("out of memory".to_owned());
        }
        if state.is_none_or(|(size, _)| size != (width, height)) {
            *state = Some(((width, height), 0));
        }
        let mut interpreter = Interpreter::new(Tables::v9());
        for file in output.files.iter().filter(|f| is_code(&f.path)) {
            interpreter.load(file.contents_str());
        }
        let images = parsed(&IMAGES9, &sources_of(output, "ui_img_"), 9, assets::parse_images_v9);
        let mut handles = Vec::new();
        for image in &images {
            let handle = ffi::lvglab9_image_new(
                image.cf,
                image.width,
                image.height,
                image.stride,
                image.data.as_ptr(),
                image.data.len() as u32,
            );
            if !handle.is_null() {
                interpreter.add_symbol(&image.name, handle as usize);
                handles.push((handle, false));
            }
        }
        let fonts = parsed(&FONTS, &sources_of(output, "ui_font_"), 0, |s| fonts::parse_fonts(&[s]));
        for font in &fonts {
            let (line_height, base_line, position, thickness, bpp) = font.metrics();
            let handle = ffi::lvglab9_font_new(
                font.bitmap().as_ptr(),
                font.glyph_list.as_ptr(),
                font.glyph_list.len() as u32,
                font.cmap_list.as_ptr(),
                font.cmap_list.len() as u32,
                line_height,
                base_line,
                position,
                thickness,
                bpp,
            );
            if !handle.is_null() {
                interpreter.add_symbol(&font.name, handle as usize);
                handles.push((handle, true));
            }
        }
        interpreter.only_screen = Some(screen_init);
        interpreter.call("ui_init", &[]);
        let screen = interpreter.screens.first().copied();
        let result = match screen {
            None => Err("the screen was not created".to_owned()),
            Some(screen) => {
                interpreter.call_lvgl("lv_screen_load", &[screen]);
                let created = interpreter.count_objects(screen);
                ffi::lvglab9_refresh();
                let stride = ffi::lvglab9_stride().max(width * 4) as usize;
                let pixels = std::slice::from_raw_parts(ffi::lvglab9_pixels(), stride * height as usize);
                let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
                for row in pixels.chunks_exact(stride) {
                    for p in row[..width as usize * 4].chunks_exact(4) {
                        rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
                    }
                }
                Ok((rgba, created))
            }
        };
        // Show an empty screen, then free the objects, fonts and images of this render.
        let blank = interpreter.call_lvgl("lv_obj_create", &[0]).unwrap_or(0);
        interpreter.call_lvgl("lv_screen_load", &[blank]);
        for screen in interpreter.screens.clone() {
            interpreter.call_lvgl("lv_obj_delete", &[screen]);
        }
        if let Some(entry) = state.as_mut() {
            let old = std::mem::replace(&mut entry.1, blank as usize);
            if old != 0 {
                interpreter.call_lvgl("lv_obj_delete", &[old as u64]);
            }
        }
        for (handle, is_font) in handles {
            if is_font {
                ffi::lvglab9_font_free(handle);
            } else {
                ffi::lvglab9_image_free(handle);
            }
        }
        let (rgba, created) = result?;
        Ok(Frame { width: width as u32, height: height as u32, rgba, skipped: interpreter.skipped, created, expected })
    }
}

/// Names of the `<screen>_screen_init` functions in the order of the screens.
fn screen_init_names(output: &lvglab_codegen::Output) -> Vec<String> {
    let Some(ui) = output.files.iter().find(|f| f.path.ends_with("ui.c")) else { return Vec::new() };
    let text = ui.contents_str();
    let Some(start) = text.find("void ui_init(void)") else { return Vec::new() };
    let body = &text[start..];
    let body = &body[..body.find("\n}").unwrap_or(body.len())];
    body.lines()
        .filter_map(|line| {
            let name = line.trim().strip_suffix("();")?;
            name.ends_with("_screen_init").then(|| name.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::color::Color;
    use lvglab_model::coord::{Align, Coord};
    use lvglab_model::flags::StateMask;
    use lvglab_model::style::{StyleProp, StyleValue};
    use lvglab_model::{Project, WidgetKind};

    fn project() -> Project {
        let mut p = Project::new("P");
        p.display.width = 200;
        p.display.height = 100;
        let screen = p.screens[0].id;
        let mut button = p.new_widget(WidgetKind::Button);
        button.geometry.align = Align::TopLeft;
        button.geometry.x = Coord::px(20);
        button.geometry.y = Coord::px(20);
        button.geometry.width = Coord::px(100);
        button.geometry.height = Coord::px(40);
        button.styles.set("main", StateMask::DEFAULT, StyleProp::BgColor, StyleValue::Color(Color::from_hex(0xFF0000)));
        p.insert(screen, 0, button).unwrap();
        p
    }

    #[test]
    fn the_export_of_a_screen_renders_with_lvgl() {
        let frame = render_screen(&project(), 0, None).expect("rendered");
        assert_eq!((frame.width, frame.height), (200, 100));
        let at = |x: usize, y: usize| {
            let i = (y * 200 + x) * 4;
            [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
        };
        assert_eq!(at(30, 30), [255, 0, 0], "the button is red; skipped: {:?}", frame.skipped);
        assert_ne!(at(180, 90), [255, 0, 0]);
        // A second render works as well.
        assert!(render_screen(&project(), 0, None).is_ok());
    }

    #[test]
    fn lvgl_9_projects_render_with_lvgl_9() {
        let mut p = project();
        p.lvgl_version = lvglab_model::project::LvglVersion::V9_6_0;
        let frame = render_screen(&p, 0, None).expect("rendered");
        let at = |x: usize, y: usize| {
            let i = (y * 200 + x) * 4;
            [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
        };
        assert_eq!(at(30, 30), [255, 0, 0], "the button is red; skipped: {:?}", frame.skipped);
        assert_ne!(at(180, 90), [255, 0, 0]);
        // LVGL 8 still works next to it.
        assert!(render_screen(&project(), 0, None).is_ok());
    }

    #[test]
    fn component_instances_render_through_their_generated_functions() {
        let mut p = project();
        let component = p.add_component();
        let root = p.widget(component).unwrap().children[0].id;
        p.widget_mut(root).unwrap().styles.set(
            "main",
            StateMask::DEFAULT,
            StyleProp::BgColor,
            StyleValue::Color(Color::from_hex(0x00FF00)),
        );
        p.widget_mut(root).unwrap().styles.set("main", StateMask::DEFAULT, StyleProp::BgOpa, StyleValue::Int(255));
        let name = p.widget(component).unwrap().name.clone();
        let screen = p.screens[0].id;
        let mut instance = p.new_widget(WidgetKind::Component);
        instance.set_prop("component", lvglab_model::prop::PropValue::Text(name));
        instance.geometry.align = Align::TopLeft;
        instance.geometry.x = Coord::px(10);
        instance.geometry.y = Coord::px(70);
        instance.geometry.width = Coord::px(60);
        instance.geometry.height = Coord::px(20);
        p.insert(screen, 1, instance).unwrap();
        let frame = render_screen(&p, 0, None).expect("rendered");
        let i = (80 * 200 + 20) * 4;
        assert_eq!(&frame.rgba[i..i + 3], &[0, 255, 0], "the component is green; skipped: {:?}", frame.skipped);
    }

    #[test]
    fn translated_text_and_theme_colors_are_drawn() {
        let mut p = project();
        p.export.multilang = true;
        p.color_themes
            .add_variable("Main", lvglab_model::color_theme::ThemeColor { color: Color::from_hex(0x00FF00), opa: 255 });
        let screen = p.screens[0].id;
        let mut label = p.new_widget(WidgetKind::Label);
        label.set_prop("text", lvglab_model::prop::PropValue::Text("Hello".into()));
        label.set_prop("translate", lvglab_model::prop::PropValue::Bool(true));
        label.geometry.align = Align::TopLeft;
        label.geometry.x = Coord::px(10);
        label.geometry.y = Coord::px(65);
        label.styles.set("main", StateMask::DEFAULT, StyleProp::TextColor, StyleValue::Theme("Main".into()));
        p.insert(screen, 1, label).unwrap();
        let frame = render_screen(&p, 0, None).expect("rendered");
        let green =
            frame.rgba.chunks_exact(4).enumerate().filter(|(i, px)| i / 200 >= 60 && px[1] > 200 && px[0] < 60).count();
        assert!(green > 20, "green text is drawn ({green} pixels); skipped: {:?}", frame.skipped);
    }

    /// With `LVGLAB_TEST_FONT=<a TTF file>` a label in a custom font is drawn.
    #[test]
    fn a_custom_font_draws_text() {
        let Some(font_path) = std::env::var_os("LVGLAB_TEST_FONT") else { return };
        let mut p = project();
        p.assets.fonts.push(lvglab_model::project::FontAsset {
            name: "Test24".into(),
            path: font_path.to_string_lossy().into_owned(),
            size: 24,
            ..Default::default()
        });
        let screen = p.screens[0].id;
        let mut label = p.new_widget(WidgetKind::Label);
        label.set_prop("text", lvglab_model::prop::PropValue::Text("Grüße 123".into()));
        label.geometry.align = Align::TopLeft;
        label.geometry.x = Coord::px(10);
        label.geometry.y = Coord::px(65);
        label.styles.set("main", StateMask::DEFAULT, StyleProp::TextFont, StyleValue::Font("Test24".into()));
        label.styles.set(
            "main",
            StateMask::DEFAULT,
            StyleProp::TextColor,
            StyleValue::Color(Color::from_hex(0xFFFFFF)),
        );
        p.insert(screen, 1, label).unwrap();
        if let Some(out) = std::env::var_os("LVGLAB_TEST_SAVE") {
            p.save(std::path::Path::new(&out)).unwrap();
        }
        let frame = render_screen(&p, 0, None).expect("rendered");
        let lit = frame.rgba.chunks_exact(4).enumerate().filter(|(i, px)| i / 200 >= 62 && px[0] > 128).count();
        assert!(!frame.skipped.iter().any(|l| l.contains("ui_font")), "{:?}", frame.skipped);
        assert!(lit > 50, "the text is drawn ({lit} bright pixels); skipped: {:?}", frame.skipped);
    }
}
