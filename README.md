# LVGLab

[![CI](https://github.com/bluecraank/lvglab/actions/workflows/ci.yml/badge.svg)](https://github.com/bluecraank/lvglab/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A modern, open source UI designer for **LVGL 8** on **ESP32 with TFT_eSPI**, written in Rust.
Design screens visually, import existing **SquareLine Studio** projects and export the UI as a
ready-to-use library for PlatformIO or the Arduino IDE.

![LVGLab](docs/screenshot.png)

> LVGLab is not affiliated with LVGL LLC or SquareLine Studio.

---

## Contents

- [Features](#features)
- [Compatibility](#compatibility)
- [Getting started](#getting-started)
- [Working with the editor](#working-with-the-editor)
- [AI assistant](#ai-assistant)
- [Importing SquareLine Studio projects](#importing-squareline-studio-projects)
- [Exporting the UI library](#exporting-the-ui-library)
- [Using the library on an ESP32 with TFT_eSPI](#using-the-library-on-an-esp32-with-tft_espi)
- [Project file format](#project-file-format)
- [Translations](#translations)
- [Architecture](#architecture)
- [Development](#development)
- [Star history](#star-history)
- [License](#license)

---

## Features

**All LVGL 8.3 widgets and parameters**

- 36 widget types: every core and extra widget of LVGL 8.3. These are Panel (`lv_obj`), Container, Label,
  Button, Image, Image button, Line, Arc, Slider, Switch, Checkbox, Dropdown, Roller, Text area,
  Keyboard, Button matrix, Spinbox, Color wheel, Bar, Spinner, LED, Meter, Chart, Table, Calendar,
  Canvas, Animated image, Span group, Tab view/Tab page, Tile view/Tile, Window, List, Menu and Message box.
- The complete set of LVGL 8.3 style properties (90 properties: size, padding, background and gradients,
  background image, border, outline, shadow, text, image, line, arc, transform, flex, grid and misc).
  You can set each of them per part (main, indicator, knob, items, list of a dropdown, …) and per state
  (default, checked, pressed, focused, disabled, … and combinations).
- Object flags, initial states, scrolling (scrollbar mode, direction, snapping), extended click area,
  flex layouts.
- Events with actions: change screen (with animation), call your own C function, modify a flag or a state,
  set a text, set a value. Gestures are supported.

**Editor**

- WYSIWYG canvas that approximates LVGL's default theme (dark/light, primary/secondary color), with
  round display masking, zoom, pan, grid and snapping.
- Click to select. Drag to move and use the handles to resize. Arrow keys nudge by 1 px (10 px with Shift).
  Drag widgets from the palette onto the canvas.
- Freely dockable panels: Screens, Layers, Widgets, Parameters, Style, Events, Timeline, Fonts and Images.
  Snap them anywhere, stack them as tabs or float them as windows. The layout is remembered.
- **Timeline**: every change becomes a named step. Click any step to jump back or forward
  (Ctrl+Z / Ctrl+Y move one step).
- Layers with drag & drop reordering and reparenting, visibility toggle, filtering and context menu.
- Image assets (PNG/JPEG/BMP) and font assets (TTF/OTF) appear in the preview and are converted when
  you export.
- **Simulator** (F5): play the UI on the canvas. Buttons show their pressed style, switches and check
  boxes toggle, sliders and arcs follow the mouse, and the project's events run (screen changes, flags,
  states, texts, values). The **Record** mode draws the pointer and every click for screen recordings.
- **Color themes**: named theme colors with one value per theme (e.g. Default and BmwOrange). Styles bound
  to a theme color follow `ui_theme_set()` at runtime.
- **AI assistant**: describe a screen and Claude builds it, validated against the widget catalog.
- Multi language UI (English, German), light and dark appearance.
- Project settings: LVGL version, resolution (with presets), round or rectangular display, theme.

**Export**

- Clean, readable LVGL 8 C code with the **same file layout and API as a SquareLine export** (`ui_init`,
  `ui_<Screen>_screen_init/destroy/relocalize`, `ui_img_<file>_png`, `ui_font_<name>`, `ui_theme_set`,
  `ui_object_set_themeable_style_property`, `ui____initial_actions0`), so it replaces a SquareLine export
  without changes to the firmware.
- Configurable symbol prefix (`ui`, `sett`, …) for several UI libraries in one firmware.
- Multi-language texts with [lv_i18n](https://github.com/lvgl/lv_i18n) (`_("text")`, `ui_relocalize()`).
- Byte-swapped images for `LV_COLOR_16_SWAP 1` (checked at compile time).
- Images are converted to `lv_img_dsc_t` (RGB565, with alpha channel when needed).
- Fonts are rendered to LVGL bitmap fonts (`lv_font_fmt_txt`, 1/2/4/8 bpp, custom Unicode ranges).
  All font and image data is `const`, so it stays in flash.
- The export only rewrites generated files. It never touches your own code and removes stale generated
  files, for example of deleted screens.

## Compatibility

| Component | Supported |
|---|---|
| Generated code | **LVGL 8.3.x** (8.3.0 – 8.3.11). Verified by compiling against LVGL 8.3.11 with `-Wall -Wextra`. |
| LVGL 9 | Not supported (different API). |
| Target | ESP32 family (ESP32, S2, S3, C3) with a display driven by [TFT_eSPI](https://github.com/Bodmer/TFT_eSPI). The generated UI code itself is platform independent. |
| Color depth | 16 bit (`LV_COLOR_DEPTH 16`, RGB565). |
| SquareLine Studio import | `.spj` projects of SquareLine Studio 1.x for LVGL 8 (tested with editor 1.6.2 / LVGL 8.3.11, board "Arduino with TFT_eSPI"). |
| Editor platforms | Windows, Linux and macOS (anything [eframe](https://github.com/emilk/egui) supports). |

## Getting started

### Build from source

Requirements: [Rust](https://rustup.rs) 1.85 or newer.

```sh
git clone https://github.com/bluecraank/lvglab
cd lvglab
cargo run --release
```

Platform notes:

- **Windows (MSVC toolchain)**: install the "Desktop development with C++" workload of the
  Visual Studio Build Tools. This is the default rustup setup.
- **Windows (GNU toolchain)**: the Windows API crates need `dlltool`. Put the `bin` folder of a
  MinGW-w64 distribution (for example [WinLibs](https://winlibs.com)) on your `PATH`.
- **Linux**: install the development packages for GTK 3 (file dialogs) and X11/Wayland, e.g. on Debian/Ubuntu:
  `sudo apt install libgtk-3-dev libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev`

### Command line

```text
lvglab [project.lvglab | squareline.spj] [--select <widget>] [--dialog <name>] [--simulate] [--screenshot <out.png>]
```

| Option | Meaning |
|---|---|
| `project.lvglab` / `file.spj` | Opens a studio project, or imports a SquareLine project. |
| `--select <widget>` | Selects a widget by name after opening. |
| `--simulate` | Starts the simulator with the record overlay. |
| `--dialog <name>` | Opens `preferences`, `about`, `discard` or `close` at start (documentation). |
| `--screenshot <out.png>` | Renders the window into a PNG and exits (documentation, CI). |

Try the bundled example: `cargo run --release -- examples/showcase/showcase.lvglab`

## Working with the editor

| Panel | Purpose |
|---|---|
| **Canvas** | Preview of the current screen. Click to select, drag to move, use the handles to resize. Ctrl + mouse wheel zooms, the wheel or the middle mouse button pans. **▶ Simulate** (F5) plays the UI; **Record** shows pointer and clicks; **↺** restarts with the current project. |
| **Screens** | All screens. The first one is loaded at start. Right-click to reorder, duplicate or delete. |
| **Layers** | Widget tree of the current screen. Drag rows to reorder or reparent. The eye toggles the hidden flag. |
| **Widgets** | Palette grouped into Basic, Controls, Visualization and Containers. Click to add to the selection, or drag onto the canvas. |
| **Parameters** | Name, position & size (px, %, content), alignment, widget properties, flex layout, flags, states, scrolling. |
| **Style** | Local styles per part and state. While a state is selected, the canvas previews the widget in that state. |
| **Events** | Trigger → action list of the selected widget. |
| **Timeline** | History of all changes. Click a step to jump there. |
| **Fonts / Images** | Assets of the project. They are converted on export. |

Use the **View** menu to show hidden panels again or to reset the layout.

**Edit → Preferences** has these editor settings:

| Group | Settings |
|---|---|
| General | Language, appearance (system/dark/light), UI scale (60–200 %), open the last project on start |
| Canvas | Background (like the editor, dark, light, checkerboard), grid size, show grid, snap to grid, outline all widgets, dashed outlines for hidden widgets, name and size of the selection, Shift + arrow step |
| Editing | Timeline length, automatic backup interval (writes `<name>.backup.lvglab` next to a saved project) |
| Export | Open the folder after exporting |

You can also clear the recent files, reset the panel layout and reset all settings there.

Keyboard shortcuts:

| Shortcut | Action |
|---|---|
| Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S | New, open, save, save as |
| Ctrl+I | Import SquareLine project |
| Ctrl+E | Export library |
| Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) | Step back / forward in the timeline |
| Ctrl+C / Ctrl+X / Ctrl+V / Ctrl+D | Copy, cut, paste, duplicate |
| Del | Delete selection |
| Arrow keys (+ Shift) | Move selection by 1 px (Shift: configurable, default 10 px) |
| Space + drag, middle mouse | Pan the canvas |
| F5 / Esc | Start or stop the simulator |

## AI assistant

The **AI assistant** panel builds interfaces from a description, e.g. *"Settings screen with three
switches and a back button"* or *"make all buttons rounder"*. It uses Claude by Anthropic.

1. Create an API key at [console.anthropic.com](https://console.anthropic.com/settings/keys).
2. Enter it under **Edit → Preferences → AI assistant**, or set the environment variable `ANTHROPIC_API_KEY`.
3. Choose **Change current screen** or **New screen**, describe what you want and press **Send** (Ctrl+Enter).

How it works:

- The request contains the display size and shape, the theme, the project's fonts and images, the
  screen names and, when changing a screen, that screen's current content. A reference of all widgets,
  properties and style properties is sent as a cached system prompt.
- The model answers with a JSON document in a fixed schema (structured output). LVGLab validates every
  widget, property, style, parent and event against the widget catalog. Problems are listed as notes,
  never applied blindly.
- Every answer becomes one step in the timeline (`AI: …`), so Ctrl+Z undoes it.
- Model (default Claude Opus 5.5, also Sonnet 5.5 and Haiku 5.5) and effort can be changed in the preferences.
  For safety declines, the server-side fallback is enabled.
- Requests are billed to your Anthropic account. The key is stored in the editor settings on your computer.

## Importing SquareLine Studio projects

Use **File → Import SquareLine project…** (Ctrl+I) or pass the `.spj` file on the command line.
The import creates a new, unsaved studio project. Save it as `.lvglab` to keep it. The original
`.spj` is never modified.

What is imported:

- Display resolution, round/rectangular shape, theme mode and colors, LVGL version
- Export settings: variable prefix (`custom_variable_prefix`), multi-language mode, byte swap of the
  TFT_eSPI board. The library is named after the project folder (e.g. `ui_round`).
- **Color themes** from `Themes.slt`. Styles that use a theme color stay bound to it.
- Texts marked as translatable (labels, check boxes)
- All screens and widgets with name, position and size (including % and content units), alignment
- Widget properties (texts, options, ranges, values, modes, keyboard target, …)
- Flags, states, scroll settings, extended click area, flex layouts
- All local styles with parts and states, including SquareLine's combined values (`Padding`,
  `Padding_RowCol`, `Shadow_params`, `Shadow_offset`, `Outline_params`, `Text_Spacing`, `Transform_pivot`).
  Colors include their alpha channel, which becomes the matching `*_opa` property.
- Image styles such as `Bg_Image` and `Arc_Image` (arcs with an image as indicator)
- Charts: type, point count, primary and secondary Y range, division lines, zoom, axis ticks and labels, data
- **The complete `assets` folder** next to the `.spj`:
  - all images (PNG/JPEG/BMP)
  - all fonts defined by SquareLine's `ui_font_<name>.fcfg` files, with their TTF file, size,
    bits per pixel, Unicode ranges and extra symbols. They are converted again on export, so no
    SquareLine-generated font code is needed.

Things to know:

- Asset paths point to the original files of the SquareLine project. Keep that folder, or replace
  the files in the **Fonts** / **Images** panels.
- A font that is referenced but has no `.fcfg` gets its size from the trailing digits of its name
  (`FontCBI40` → 40 px). Assign the TTF file in the **Fonts** panel. Until then the export falls back
  to the nearest built-in Montserrat size, so the project still compiles.
- **Animations**, SquareLine **events** and **components** (`.ecomp`) are not imported yet. The import report lists everything that was skipped.
- Unknown object types are imported as Panels and listed in the report.
- Tested with real SquareLine 1.6 projects (round 466×466 and rectangular). The exports compile against LVGL 8.3.11,
  and when rendered with LVGL they match SquareLine's own screenshots.

## Exporting the UI library

**File → Export library** (Ctrl+E) writes a library folder. You configure its name and location under
**Project settings → Library export**. The location is relative to the project file, so it can point
directly into the `lib` folder of your PlatformIO project.

```text
ui_round/                   library name (project settings)
├── library.json            PlatformIO library manifest
├── library.properties      Arduino IDE library manifest
└── src/
    ├── ui.h / ui.c                 ui_init(), ui_destroy(), ui_relocalize(), images, fonts
    ├── ui_<Screen>.h / .c          one pair per screen: _screen_init/_destroy/_relocalize
    ├── ui_helpers.h/.c             screen change and flag/state helpers
    ├── ui_theme_manager.h/.c       ui_object_set_themeable_style_property(), ...
    ├── ui_themes.h/.c              UI_THEME_*, theme colors, ui_theme_set(), ui_theme_idx
    ├── ui_events.h                 declarations of your event functions
    ├── ui_events_weak.c            empty weak defaults so the project always links
    ├── ui_events.c                 YOUR code: created once, never overwritten
    ├── ui_img_<file>_png.c
    └── ui_font_<name>.c
```

All names start with the **symbol prefix** (`ui` above). Settings under **Project settings → Library export**:

| Setting | Meaning |
|---|---|
| Library name | Folder and library name, e.g. `ui_round` (the name used in `lib_ignore`). |
| Symbol prefix | Prefix of all files and symbols. Use different prefixes for libraries in one firmware (`ui`, `sett`). |
| Multiple languages | Texts marked as *Translate* are wrapped in `_()` of lv_i18n; `ui.h` includes `lv_i18n.h`. |
| Byte swap | Image pixels for `LV_COLOR_16_SWAP 1`. `ui.c` stops the build with `#error` if `lv_conf.h` differs. |

Generated files start with `// Generated by LVGLab`. Only those files are overwritten or
removed on the next export.

## Using the library on an ESP32 with TFT_eSPI

1. Export the library into the `lib` folder of your PlatformIO project (export folder `lib`, library name `ui`).
2. Add LVGL and TFT_eSPI to `platformio.ini` and point LVGL to your `lv_conf.h`:

```ini
[env:esp32dev]
platform = espressif32
board = esp32dev
framework = arduino
lib_deps =
    lvgl/lvgl@8.3.11
    bodmer/TFT_eSPI@^2.5.43
build_flags =
    -D LV_CONF_INCLUDE_SIMPLE
    -I include                 ; folder containing lv_conf.h
    ; TFT_eSPI configuration, e.g.:
    -D USER_SETUP_LOADED=1
    -D ILI9341_DRIVER=1
    -D TFT_MOSI=23 -D TFT_SCLK=18 -D TFT_CS=15 -D TFT_DC=2 -D TFT_RST=4
    -D SPI_FREQUENCY=40000000
```

3. Your `lv_conf.h` must set `LV_COLOR_DEPTH 16` and enable every built-in Montserrat size the UI uses
   (`LV_FONT_MONTSERRAT_<n> 1`). The generated `ui.c` stops the build with a clear `#error` if
   something is missing.
4. Initialize LVGL and the display, then call `ui_init()`:

```cpp
#include <Arduino.h>
#include <lvgl.h>
#include <TFT_eSPI.h>
#include <ui.h>

static const uint16_t screen_width = 320, screen_height = 240;
static lv_disp_draw_buf_t draw_buf;
static lv_color_t buf[screen_width * 40];
TFT_eSPI tft;

static void flush(lv_disp_drv_t * disp, const lv_area_t * area, lv_color_t * color_p)
{
    uint32_t w = area->x2 - area->x1 + 1, h = area->y2 - area->y1 + 1;
    tft.startWrite();
    tft.setAddrWindow(area->x1, area->y1, w, h);
    tft.pushColors((uint16_t *)&color_p->full, w * h, true);  // swap bytes for SPI
    tft.endWrite();
    lv_disp_flush_ready(disp);
}

void setup()
{
    lv_init();
    tft.begin();
    tft.setRotation(1);
    lv_disp_draw_buf_init(&draw_buf, buf, NULL, screen_width * 40);
    static lv_disp_drv_t disp_drv;
    lv_disp_drv_init(&disp_drv);
    disp_drv.hor_res = screen_width;
    disp_drv.ver_res = screen_height;
    disp_drv.flush_cb = flush;
    disp_drv.draw_buf = &draw_buf;
    lv_disp_drv_register(&disp_drv);
    ui_init();
}

void loop()
{
    lv_timer_handler();
    delay(5);
}
```

The LVGL tick must run, e.g. with `LV_TICK_CUSTOM 1` and `LV_TICK_CUSTOM_SYS_TIME_EXPR (millis())` in `lv_conf.h`.

## Project file format

Studio projects (`.lvglab`) are pretty-printed JSON and work well with git. Values that equal
the LVGL defaults are left out, so diffs stay small.

Saving makes the project folder self-contained: every image and TTF/OTF file the project uses is
copied into an `assets/` folder next to the `.lvglab` file, and the paths are stored relative to it.
Files with the same name get a suffix (`logo_2.png`). **Save as** into another folder copies the assets
again. Missing files are listed after saving.

```text
MyProject/
├── MyProject.lvglab
└── assets/
    ├── background.png
    └── calibri-bold-italic.ttf
```

```json
{
  "name": "Showcase",
  "lvgl_version": "8.3.11",
  "display": { "width": 480, "height": 320, "shape": "RECTANGLE" },
  "screens": [
    {
      "id": 1, "kind": "screen", "name": "Screen1",
      "children": [
        {
          "id": 5, "kind": "slider", "name": "Slider1",
          "geometry": { "x": "0", "y": "-10", "width": "150", "height": "10", "align": "CENTER" },
          "props": { "value": { "int": 40 } },
          "styles": [ { "slot": "indicator", "state": "DEFAULT", "props": { "bg_color": { "color": "#F15C00" } } } ]
        }
      ]
    }
  ]
}
```

## Translations

UI texts are written in English in the code and translated through
[`crates/app/locales/<language>.json`](crates/app/locales/de.json). Each file maps English to the
target language and is embedded at build time. Missing entries fall back to English.

To add a language:

1. Copy `crates/app/locales/de.json` to e.g. `fr.json` and translate the values. Keep `{}` placeholders.
2. Add the language to `Language` in [`crates/app/src/i18n.rs`](crates/app/src/i18n.rs).

## Architecture

The code is a Cargo workspace with one responsibility per crate:

```text
crates/
├── model     lvglab-model    document model: widgets, catalog, styles, events, theme, layout
├── spj       lvglab-spj      SquareLine Studio importer
├── codegen   lvglab-codegen  C code, image and font conversion, library export
├── ai        lvglab-ai     AI assistant: prompt, Claude API client, design schema and validation
└── app       lvglab  egui desktop application
```

Design principles:

- **Data-driven widget catalog** ([`catalog.rs`](crates/model/src/catalog.rs)). Every widget is described
  once: properties, defaults, style parts, C templates and SquareLine names. The editor, the importer and the
  code generator all work from this description. Adding a widget means adding a catalog entry, not touching
  three code paths.
- **Declarative LVGL enums**. One `lv_enum!` macro maps Rust enums to C constants, labels and
  serialization.
- **Pure code generation**. `generate_library()` returns files in memory. Writing them is a separate,
  conservative step, which makes the generator easy to test.
- **Commands and a single mutation point** in the app. Panels queue commands; the frame executes them, and
  the timeline records the resulting state.
- **No `unsafe`**: the workspace forbids it.

## Development

```sh
cargo test --workspace                         # unit tests of all crates
cargo clippy --workspace --all-targets         # lints
cargo fmt --all                                # formatting
cargo run -p lvglab-codegen --example export_spj -- my.spj out/ui   # headless SquareLine → library export
cargo run -p lvglab-codegen --example all_widgets -- out/ui         # every widget type, for compile checks
```

The generated C code is checked by compiling an export with every widget type against LVGL 8.3.11
(`gcc -std=c99 -Wall -Wextra`) and by rendering it with LVGL on the desktop.

Contributions are welcome: issues, translations, widget previews and importer improvements.

## Star history

[![Star History Chart](https://api.star-history.com/svg?repos=bluecraank/lvglab&type=Date)](https://www.star-history.com/#bluecraank/lvglab&Date)

## License

[MIT](LICENSE). LVGL is MIT licensed as well. SquareLine Studio is a product of its respective owners.
This project only reads its project files.
