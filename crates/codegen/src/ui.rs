//! Generation of the LVGL UI sources (`ui.h`, `ui.c`, one file per screen,
//! helpers and event hooks).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use lvglab_model::catalog::WidgetDef;
use lvglab_model::event::{Action, EventHandler, ModifyOp};
use lvglab_model::layout::pixel_size;
use lvglab_model::prop::{Emit, EmitWhen, PropDef, PropKind, PropValue};
use lvglab_model::style::StyleValue;
use lvglab_model::{Align, Coord, Project, Unit, Widget, WidgetId, WidgetKind};

use crate::naming::{NameTable, c_ident};
use crate::output::{GeneratedFile, MARKER, WritePolicy};
use crate::template::{Scope, c_string, format_prop, format_style, lines};

/// Declarations contributed by asset conversion.
#[derive(Default)]
pub struct AssetDecls {
    pub images: Vec<String>,
    pub fonts: Vec<String>,
    /// `#define` lines mapping missing fonts to built-in ones.
    pub font_fallbacks: Vec<String>,
    /// Built-in Montserrat sizes the application's `lv_conf.h` must enable.
    pub montserrat: BTreeSet<u16>,
}

pub struct UiSources {
    pub files: Vec<GeneratedFile>,
    pub warnings: Vec<String>,
}

pub fn header(project: &Project) -> String {
    format!(
        "// {MARKER}. Do not edit: changes are overwritten on export.\n// Project: {} | LVGL {}\n",
        project.name,
        project.lvgl_version.suffix()
    )
}

/// Small helper for indented C code.
#[derive(Default)]
struct Code {
    text: String,
    indent: usize,
}

impl Code {
    fn line(&mut self, line: &str) {
        for part in line.lines() {
            if part.is_empty() {
                self.text.push('\n');
            } else {
                let _ = writeln!(self.text, "{}{part}", "    ".repeat(self.indent));
            }
        }
    }

    fn blank(&mut self) {
        self.text.push('\n');
    }

    fn open(&mut self, line: &str) {
        self.line(line);
        self.line("{");
        self.indent += 1;
    }

    fn close(&mut self) {
        self.indent -= 1;
        self.line("}");
    }
}

pub fn generate(project: &Project, assets: &AssetDecls) -> UiSources {
    let names = NameTable::new(project);
    let mut ctx =
        Context { project, names: &names, warnings: Vec::new(), functions: BTreeSet::new(), cross_refs: Vec::new() };
    let mut files = Vec::new();
    for screen in &project.screens {
        let code = ScreenGen::new(&mut ctx, screen).run();
        files.push(GeneratedFile::text(format!("screens/{}.c", names.var(screen.id)), code, WritePolicy::Overwrite));
    }
    files.push(GeneratedFile::text("ui.h", ui_h(&ctx, assets), WritePolicy::Overwrite));
    files.push(GeneratedFile::text("ui.c", ui_c(&ctx, assets), WritePolicy::Overwrite));
    files.push(GeneratedFile::text("ui_helpers.h", helpers_h(project), WritePolicy::Overwrite));
    files.push(GeneratedFile::text("ui_helpers.c", helpers_c(project), WritePolicy::Overwrite));
    files.push(GeneratedFile::text("ui_events.h", events_h(&ctx), WritePolicy::Overwrite));
    files.push(GeneratedFile::text("ui_events_weak.c", events_weak_c(&ctx), WritePolicy::Overwrite));
    files.push(GeneratedFile::text("ui_events.c", events_c(), WritePolicy::KeepExisting));
    UiSources { files, warnings: ctx.warnings }
}

struct Context<'a> {
    project: &'a Project,
    names: &'a NameTable,
    warnings: Vec<String>,
    /// User functions referenced by "call function" actions.
    functions: BTreeSet<String>,
    /// Widget references between screens, resolved in `ui_init`.
    cross_refs: Vec<String>,
}

/// True if a creation or post-processing template of `def` uses `key`.
fn used_in_templates(def: &WidgetDef, key: &str) -> bool {
    let pattern = format!("{{{key}");
    [def.create, def.post_create, def.post_props].iter().any(|t| {
        t.match_indices(&pattern).any(|(i, _)| matches!(t[i + pattern.len()..].chars().next(), Some('}' | '.')))
    })
}

fn is_temporary(screen: &Widget) -> bool {
    screen.prop_bool("temporary")
}

fn event_fn(var: &str) -> String {
    format!("ui_event_{}", var.trim_start_matches("ui_"))
}

struct ScreenGen<'c, 'a> {
    ctx: &'c mut Context<'a>,
    screen: &'a Widget,
    init: Code,
    events: Code,
    deferred: Vec<String>,
    vars: Vec<String>,
}

impl<'c, 'a> ScreenGen<'c, 'a> {
    fn new(ctx: &'c mut Context<'a>, screen: &'a Widget) -> Self {
        Self { ctx, screen, init: Code::default(), events: Code::default(), deferred: Vec::new(), vars: Vec::new() }
    }

    fn run(mut self) -> String {
        let screen = self.screen;
        let screen_var = self.ctx.names.var(screen.id).to_owned();
        self.init.indent = 1;
        self.widget(screen, "NULL");
        for line in std::mem::take(&mut self.deferred) {
            self.init.line(&line);
        }
        if is_temporary(screen) {
            self.init.line(&format!(
                "lv_obj_add_event_cb({screen_var}, {screen_var}_unloaded_cb, LV_EVENT_SCREEN_UNLOADED, NULL);"
            ));
        }

        let mut file = Code::default();
        file.line(&header(self.ctx.project));
        file.line("#include \"../ui.h\"");
        file.blank();
        for var in &self.vars {
            file.line(&format!("lv_obj_t * {var} = NULL;"));
        }
        file.blank();
        if is_temporary(screen) {
            file.open(&format!("static void {screen_var}_unloaded_cb(lv_event_t * e)"));
            file.line("LV_UNUSED(e);");
            file.line(&format!("{screen_var}_screen_destroy();"));
            file.close();
            file.blank();
        }
        if !self.events.text.is_empty() {
            file.line("/* Events */");
            file.text.push_str(&self.events.text);
        }
        file.line("/* Screen */");
        file.line(&format!("void {screen_var}_screen_init(void)"));
        file.line("{");
        file.text.push_str(&self.init.text);
        file.line("}");
        if is_temporary(screen) {
            file.blank();
            file.open(&format!("void {screen_var}_screen_destroy(void)"));
            file.line(&format!("if({screen_var}) lv_obj_del_async({screen_var});"));
            for var in &self.vars {
                file.line(&format!("{var} = NULL;"));
            }
            file.close();
        }
        file.text
    }

    fn widget(&mut self, widget: &'a Widget, parent: &str) {
        let def = widget.def();
        let obj = self.ctx.names.var(widget.id).to_owned();
        self.vars.push(obj.clone());
        let scope = self.scope(widget, def, &obj, parent);

        self.init.blank();
        self.init.line(&format!("/* {}: {} */", def.name, widget.name));
        for prop in def.props.iter().filter(|p| used_in_templates(def, p.key)) {
            for prelude in &scope.props[prop.key].prelude {
                self.init.line(prelude);
            }
        }
        self.init.line(&format!("{obj} = {};", scope.expand(def.create)));
        if !def.post_create.is_empty() {
            self.init.line(&scope.expand(def.post_create));
        }
        if widget.kind != WidgetKind::Screen && !def.managed_geometry {
            self.geometry(widget, &obj);
        }
        if let Some(flex) = widget.layout {
            self.init.line(&format!("lv_obj_set_flex_flow({obj}, {});", flex.flow.c_const()));
            self.init.line(&format!(
                "lv_obj_set_flex_align({obj}, {}, {}, {});",
                flex.main_place.c_const(),
                flex.cross_place.c_const(),
                flex.track_place.c_const()
            ));
        }
        for prop in def.props {
            self.prop(widget, prop, &scope, &obj);
        }
        self.flags_and_states(widget, &obj);
        self.styles(widget, def, &scope, &obj);
        if !widget.events.is_empty() {
            let callback = event_fn(&obj);
            self.init.line(&format!("lv_obj_add_event_cb({obj}, {callback}, LV_EVENT_ALL, NULL);"));
            self.event_callback(&callback, &widget.events);
        }
        if !def.post_props.is_empty() {
            self.init.line(&scope.expand(def.post_props));
        }
        let child_parent = def.child_parent.map_or_else(|| obj.clone(), |t| scope.expand(t));
        for child in &widget.children {
            self.widget(child, &child_parent);
        }
    }

    fn scope(&self, widget: &Widget, def: &WidgetDef, obj: &str, parent: &str) -> Scope<'static> {
        let mut scope = Scope::default()
            .var("obj", obj)
            .var("parent", parent)
            .var("w", pixel_size(widget.geometry.width, 100).to_string())
            .var("h", pixel_size(widget.geometry.height, 100).to_string());
        for prop in def.props {
            let value = widget.prop(prop.key).unwrap_or_else(|| prop.default_value());
            let array = format!("{obj}_{}", prop.key);
            scope.props.insert(prop.key, format_prop(prop, &value, &array, self.ctx.names));
        }
        scope
    }

    fn geometry(&mut self, widget: &Widget, obj: &str) {
        let g = widget.geometry;
        self.init.line(&format!("lv_obj_set_width({obj}, {});", g.width.c_expr()));
        self.init.line(&format!("lv_obj_set_height({obj}, {});", g.height.c_expr()));
        let set_pos = |c: Coord| c.value != 0 || c.unit != Unit::Px;
        if set_pos(g.x) {
            self.init.line(&format!("lv_obj_set_x({obj}, {});", g.x.c_expr()));
        }
        if set_pos(g.y) {
            self.init.line(&format!("lv_obj_set_y({obj}, {});", g.y.c_expr()));
        }
        if !matches!(g.align, Align::TopLeft | Align::Default) {
            self.init.line(&format!("lv_obj_set_align({obj}, {});", g.align.c_const()));
        }
    }

    fn prop(&mut self, widget: &Widget, prop: &'static PropDef, scope: &Scope<'static>, obj: &str) {
        let value = widget.prop(prop.key).unwrap_or_else(|| prop.default_value());
        let skip = match prop.when {
            EmitWhen::Always => false,
            EmitWhen::Changed => value == prop.default_value(),
            EmitWhen::NotEmpty => match &value {
                PropValue::Text(t) | PropValue::Image(t) => t.trim().is_empty(),
                PropValue::WidgetRef(r) => r.is_none(),
                _ => false,
            },
        };
        if skip {
            return;
        }
        let formatted = &scope.props[prop.key];
        let with_value = |extra: &[(&'static str, String)]| {
            let mut s = Scope::default();
            s.vars.clone_from(&scope.vars);
            s.props.clone_from(&scope.props);
            s.props.insert("v", formatted.clone());
            for (k, v) in extra {
                s.vars.insert(*k, v.clone());
            }
            s
        };
        if prop.emit != Emit::None && !used_in_templates(widget.def(), prop.key) {
            for prelude in &formatted.prelude {
                self.init.line(prelude);
            }
        }
        match prop.emit {
            Emit::None => {}
            Emit::Call(template) => {
                let statement = with_value(&[]).expand(template);
                if prop.kind == PropKind::WidgetRef {
                    self.defer_reference(widget, value.as_widget_ref(), statement);
                } else {
                    self.init.line(&statement);
                }
            }
            Emit::EachLine(template) => {
                for (i, line) in lines(value.as_text().unwrap_or_default()).enumerate() {
                    self.init.line(&with_value(&[("line", c_string(line)), ("i", i.to_string())]).expand(template));
                }
            }
            Emit::EachCell(template) => {
                for (row, line) in lines(value.as_text().unwrap_or_default()).enumerate() {
                    for (col, cell) in line.split('|').enumerate() {
                        let vars =
                            [("row", row.to_string()), ("col", col.to_string()), ("cell", c_string(cell.trim()))];
                        self.init.line(&with_value(&vars).expand(template));
                    }
                }
            }
            Emit::OptionTemplate => {
                let template = value.as_text().unwrap_or_default().to_owned();
                if !template.is_empty() {
                    self.init.line(&with_value(&[]).expand(&template));
                }
            }
            Emit::ChartSeries => self.chart_series(widget, obj, value.as_text().unwrap_or_default()),
        }
    }

    /// References are applied once all objects exist.
    fn defer_reference(&mut self, widget: &Widget, target: Option<WidgetId>, statement: String) {
        let same_screen = target.is_some_and(|t| self.screen.find(t).is_some());
        if same_screen {
            self.deferred.push(statement);
        } else {
            let source = self.ctx.names.var(widget.id);
            let target = target.map_or("NULL", |t| self.ctx.names.var(t));
            self.ctx.cross_refs.push(format!("if({source} && {target}) {statement}"));
        }
    }

    fn chart_series(&mut self, widget: &Widget, obj: &str, text: &str) {
        let points = widget.prop_int("point_count").max(1) as usize;
        for (i, series) in lvglab_model::series::parse(text).into_iter().enumerate() {
            let mut values: Vec<String> = series.values.iter().map(i32::to_string).collect();
            values.resize(points.max(values.len()), "LV_CHART_POINT_NONE".to_owned());
            let axis = if series.secondary { "LV_CHART_AXIS_SECONDARY_Y" } else { "LV_CHART_AXIS_PRIMARY_Y" };
            let name = format!("{obj}_series_{i}");
            self.init.line(&format!(
                "lv_chart_series_t * {name} = lv_chart_add_series({obj}, {}, {axis});",
                series.color.c_expr()
            ));
            self.init.line(&format!("static lv_coord_t {name}_array[] = {{{}}};", values.join(", ")));
            self.init.line(&format!("lv_chart_set_ext_y_array({obj}, {name}, {name}_array);"));
        }
    }

    fn flags_and_states(&mut self, widget: &Widget, obj: &str) {
        let collect = |on: bool| -> Vec<String> {
            widget.flags.iter().filter(|(_, v)| **v == on).map(|(f, _)| f.c_const()).collect()
        };
        let (add, clear) = (collect(true), collect(false));
        if !add.is_empty() {
            self.init.line(&format!("lv_obj_add_flag({obj}, {});", add.join(" | ")));
        }
        if !clear.is_empty() {
            self.init.line(&format!("lv_obj_clear_flag({obj}, {});", clear.join(" | ")));
        }
        if !widget.states.is_empty() {
            let states: Vec<String> = widget.states.iter().map(|s| s.c_const()).collect();
            self.init.line(&format!("lv_obj_add_state({obj}, {});", states.join(" | ")));
        }
        let scroll = widget.scroll;
        if let Some(mode) = scroll.scrollbar_mode {
            self.init.line(&format!("lv_obj_set_scrollbar_mode({obj}, {});", mode.c_const()));
        }
        if let Some(dir) = scroll.dir {
            self.init.line(&format!("lv_obj_set_scroll_dir({obj}, {});", dir.c_const()));
        }
        if let Some(snap) = scroll.snap_x {
            self.init.line(&format!("lv_obj_set_scroll_snap_x({obj}, {});", snap.c_const()));
        }
        if let Some(snap) = scroll.snap_y {
            self.init.line(&format!("lv_obj_set_scroll_snap_y({obj}, {});", snap.c_const()));
        }
        if widget.ext_click_area != 0 {
            self.init.line(&format!("lv_obj_set_ext_click_area({obj}, {});", widget.ext_click_area));
        }
    }

    fn styles(&mut self, widget: &Widget, def: &WidgetDef, scope: &Scope<'static>, obj: &str) {
        for block in widget.styles.blocks() {
            if def.slot(&block.slot).is_none() {
                self.ctx.warnings.push(format!("{}: unknown style part `{}` was skipped", widget.name, block.slot));
            }
        }
        for slot in def.slots {
            let target = slot.getter.map_or_else(|| obj.to_owned(), |g| scope.expand(g));
            let mut blocks: Vec<_> = widget.styles.blocks().filter(|b| b.slot == slot.key).collect();
            blocks.sort_by_key(|b| b.state);
            for block in blocks {
                let selector = format!("{} | {}", slot.part.c_const(), block.state.c_expr());
                for (prop, value) in &block.props {
                    if matches!(value, StyleValue::Image(i) if i.is_empty()) {
                        continue;
                    }
                    self.init.line(&format!(
                        "lv_obj_set_style_{}({target}, {}, {selector});",
                        prop.name(),
                        format_style(value)
                    ));
                }
            }
        }
    }

    fn event_callback(&mut self, name: &str, handlers: &[EventHandler]) {
        let code = &mut self.events;
        code.open(&format!("void {name}(lv_event_t * e)"));
        code.line("lv_event_code_t event_code = lv_event_get_code(e);");
        code.line("lv_obj_t * target = lv_event_get_target(e);");
        code.line("LV_UNUSED(target);");
        for handler in handlers {
            let statement = action_code(self.ctx, &handler.action);
            let code = &mut self.events;
            code.open(&format!("if({})", handler.trigger.c_condition()));
            if handler.trigger.is_gesture() {
                code.line("lv_indev_wait_release(lv_indev_get_act());");
            }
            code.line(&statement);
            code.close();
        }
        self.events.close();
        self.events.blank();
    }
}

fn action_code(ctx: &mut Context<'_>, action: &Action) -> String {
    let names = ctx.names;
    let target = |id: Option<WidgetId>| id.map(|id| names.var(id).to_owned());
    let op = |op: ModifyOp, kind: &str| format!("_UI_MODIFY_{kind}_{}", op.suffix());
    match action {
        Action::ChangeScreen { screen, anim, time_ms, delay_ms } => match target(*screen) {
            Some(s) => format!("_ui_screen_change(&{s}, {}, {time_ms}, {delay_ms}, &{s}_screen_init);", anim.c_const()),
            None => "/* change screen: no screen selected */".to_owned(),
        },
        Action::CallFunction { name } => {
            let name = c_ident(name);
            ctx.functions.insert(name.clone());
            format!("{name}(e);")
        }
        Action::ModifyFlag { target: t, flag, op: o } => match target(*t) {
            Some(t) => format!("_ui_flag_modify({t}, {}, {});", flag.c_const(), op(*o, "FLAG")),
            None => "/* modify flag: no target selected */".to_owned(),
        },
        Action::ModifyState { target: t, state, op: o } => match target(*t) {
            Some(t) => format!("_ui_state_modify({t}, {}, {});", state.c_const(), op(*o, "STATE")),
            None => "/* modify state: no target selected */".to_owned(),
        },
        Action::SetText { target: t, text } => {
            let kind = t.and_then(|t| ctx.project.widget(t)).map(|w| w.kind);
            let setter = match kind {
                Some(WidgetKind::Label) => "lv_label_set_text",
                Some(WidgetKind::Textarea) => "lv_textarea_set_text",
                Some(WidgetKind::Checkbox) => "lv_checkbox_set_text",
                Some(WidgetKind::Dropdown) => "lv_dropdown_set_text",
                _ => return "/* set text: target must be a label, text area, checkbox or dropdown */".to_owned(),
            };
            let t = target(*t).expect("kind implies target");
            format!("if({t}) {setter}({t}, {});", c_string(text))
        }
        Action::SetValue { target: t, value } => {
            let kind = t.and_then(|t| ctx.project.widget(t)).map(|w| w.kind);
            let call = match kind {
                Some(WidgetKind::Slider) => "lv_slider_set_value({t}, {v}, LV_ANIM_ON)",
                Some(WidgetKind::Bar) => "lv_bar_set_value({t}, {v}, LV_ANIM_ON)",
                Some(WidgetKind::Arc) => "lv_arc_set_value({t}, {v})",
                Some(WidgetKind::Spinbox) => "lv_spinbox_set_value({t}, {v})",
                Some(WidgetKind::Roller) => "lv_roller_set_selected({t}, {v}, LV_ANIM_ON)",
                Some(WidgetKind::Dropdown) => "lv_dropdown_set_selected({t}, {v})",
                Some(WidgetKind::Led) => "lv_led_set_brightness({t}, {v})",
                _ => {
                    return "/* set value: target must be a slider, bar, arc, spinbox, roller, dropdown or LED */"
                        .to_owned();
                }
            };
            let t = target(*t).expect("kind implies target");
            format!("if({t}) {};", call.replace("{t}", &t).replace("{v}", &value.to_string()))
        }
    }
}

fn ui_h(ctx: &Context<'_>, assets: &AssetDecls) -> String {
    let project = ctx.project;
    let mut code = Code::default();
    code.line(&header(project));
    code.line("#ifndef UI_H\n#define UI_H\n\n#ifdef __cplusplus\nextern \"C\" {\n#endif\n");
    code.line("#include \"lvgl.h\"\n#include \"ui_helpers.h\"\n#include \"ui_events.h\"\n");
    for screen in &project.screens {
        let var = ctx.names.var(screen.id);
        code.line(&format!("/* Screen: {} */", screen.name));
        code.line(&format!("void {var}_screen_init(void);"));
        if is_temporary(screen) {
            code.line(&format!("void {var}_screen_destroy(void);"));
        }
        screen.walk(&mut |w| {
            let var = ctx.names.var(w.id);
            code.line(&format!("extern lv_obj_t * {var};"));
            if !w.events.is_empty() {
                code.line(&format!("void {}(lv_event_t * e);", event_fn(var)));
            }
        });
        code.blank();
    }
    if !assets.images.is_empty() {
        code.line("/* Images */");
        for image in &assets.images {
            code.line(&format!("LV_IMG_DECLARE({image});"));
        }
        code.blank();
    }
    if !assets.fonts.is_empty() || !assets.font_fallbacks.is_empty() {
        code.line("/* Fonts */");
        for font in &assets.fonts {
            code.line(&format!("LV_FONT_DECLARE({font});"));
        }
        for fallback in &assets.font_fallbacks {
            code.line(fallback);
        }
        code.blank();
    }
    code.line("void ui_init(void);\n\n#ifdef __cplusplus\n} /* extern \"C\" */\n#endif\n\n#endif /* UI_H */");
    code.text
}

fn ui_c(ctx: &Context<'_>, assets: &AssetDecls) -> String {
    let project = ctx.project;
    let mut code = Code::default();
    code.line(&header(project));
    code.line("#include \"ui.h\"\n");
    code.line("/* Requirements on the application's lv_conf.h */");
    code.line(
        "#if LV_COLOR_DEPTH != 16\n    #error \"LV_COLOR_DEPTH must be 16 to match the generated images\"\n#endif",
    );
    for size in &assets.montserrat {
        code.line(&format!(
            "#if LV_FONT_MONTSERRAT_{size} == 0\n    #error \"Enable LV_FONT_MONTSERRAT_{size} in lv_conf.h\"\n#endif"
        ));
    }
    code.blank();
    code.open("void ui_init(void)");
    code.line("lv_disp_t * dispp = lv_disp_get_default();");
    code.line(&format!(
        "lv_theme_t * theme = lv_theme_default_init(dispp, lv_palette_main({}), lv_palette_main({}), {}, LV_FONT_DEFAULT);",
        project.theme.primary.c_const(),
        project.theme.secondary.c_const(),
        project.theme.dark
    ));
    code.line("lv_disp_set_theme(dispp, theme);");
    for (i, screen) in project.screens.iter().enumerate() {
        if i == 0 || !is_temporary(screen) {
            code.line(&format!("{}_screen_init();", ctx.names.var(screen.id)));
        }
    }
    for reference in &ctx.cross_refs {
        code.line(reference);
    }
    if let Some(first) = project.screens.first() {
        code.line(&format!("lv_disp_load_scr({});", ctx.names.var(first.id)));
    }
    code.close();
    code.text
}

fn helpers_h(project: &Project) -> String {
    format!(
        "{}\n#ifndef UI_HELPERS_H\n#define UI_HELPERS_H\n\n#include \"lvgl.h\"\n\n\
#ifdef __cplusplus\nextern \"C\" {{\n#endif\n\n\
#define _UI_MODIFY_FLAG_ADD 0\n#define _UI_MODIFY_FLAG_REMOVE 1\n#define _UI_MODIFY_FLAG_TOGGLE 2\n\n\
#define _UI_MODIFY_STATE_ADD 0\n#define _UI_MODIFY_STATE_REMOVE 1\n#define _UI_MODIFY_STATE_TOGGLE 2\n\n\
void _ui_flag_modify(lv_obj_t * target, uint32_t flag, int value);\n\
void _ui_state_modify(lv_obj_t * target, lv_state_t state, int value);\n\
void _ui_screen_change(lv_obj_t ** target, lv_scr_load_anim_t anim, uint32_t time, uint32_t delay, void (*target_init)(void));\n\n\
#ifdef __cplusplus\n}} /* extern \"C\" */\n#endif\n\n#endif /* UI_HELPERS_H */\n",
        header(project)
    )
}

fn helpers_c(project: &Project) -> String {
    format!(
        "{}\n#include \"ui_helpers.h\"\n\n\
void _ui_flag_modify(lv_obj_t * target, uint32_t flag, int value)\n{{\n\
    if(target == NULL) return;\n\
    if(value == _UI_MODIFY_FLAG_TOGGLE) value = lv_obj_has_flag(target, flag) ? _UI_MODIFY_FLAG_REMOVE : _UI_MODIFY_FLAG_ADD;\n\
    if(value == _UI_MODIFY_FLAG_ADD) lv_obj_add_flag(target, flag);\n\
    else lv_obj_clear_flag(target, flag);\n}}\n\n\
void _ui_state_modify(lv_obj_t * target, lv_state_t state, int value)\n{{\n\
    if(target == NULL) return;\n\
    if(value == _UI_MODIFY_STATE_TOGGLE) value = lv_obj_has_state(target, state) ? _UI_MODIFY_STATE_REMOVE : _UI_MODIFY_STATE_ADD;\n\
    if(value == _UI_MODIFY_STATE_ADD) lv_obj_add_state(target, state);\n\
    else lv_obj_clear_state(target, state);\n}}\n\n\
void _ui_screen_change(lv_obj_t ** target, lv_scr_load_anim_t anim, uint32_t time, uint32_t delay, void (*target_init)(void))\n{{\n\
    if(*target == NULL) target_init();\n\
    lv_scr_load_anim(*target, anim, time, delay, false);\n}}\n",
        header(project)
    )
}

fn events_h(ctx: &Context<'_>) -> String {
    let mut code = Code::default();
    code.line(&header(ctx.project));
    code.line("#ifndef UI_EVENTS_H\n#define UI_EVENTS_H\n\n#include \"lvgl.h\"\n\n#ifdef __cplusplus\nextern \"C\" {\n#endif\n");
    code.line("/* Implement these in ui_events.c; empty defaults live in ui_events_weak.c. */");
    for function in &ctx.functions {
        code.line(&format!("void {function}(lv_event_t * e);"));
    }
    code.line("\n#ifdef __cplusplus\n} /* extern \"C\" */\n#endif\n\n#endif /* UI_EVENTS_H */");
    code.text
}

fn events_weak_c(ctx: &Context<'_>) -> String {
    let mut code = Code::default();
    code.line(&header(ctx.project));
    code.line("#include \"ui_events.h\"\n");
    code.line("/* Weak defaults so the project always links. Define the functions in ui_events.c to override them. */");
    for function in &ctx.functions {
        code.line(&format!("__attribute__((weak)) void {function}(lv_event_t * e)\n{{\n    LV_UNUSED(e);\n}}\n"));
    }
    code.text
}

fn events_c() -> String {
    "// Created by LVGLab. This file is yours: it is never overwritten.\n\
// Implement the functions declared in ui_events.h here, e.g.\n\
//\n\
// void on_button_clicked(lv_event_t * e)\n\
// {\n\
//     lv_obj_t * target = lv_event_get_target(e);\n\
// }\n\n\
#include \"ui.h\"\n"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::Color;
    use lvglab_model::event::{EventTrigger, ScreenAnim};
    use lvglab_model::flags::{ObjFlag, ObjState, StateMask};
    use lvglab_model::style::StyleProp;

    fn project() -> Project {
        let mut project = Project::new("Demo");
        let screen = project.screens[0].id;
        let second = project.add_screen();
        let mut button = project.new_widget(WidgetKind::Button);
        button.geometry.align = Align::Center;
        button.geometry.y = Coord::pct(10);
        button.set_flag(ObjFlag::Scrollable, true);
        button.states.insert(ObjState::Checked);
        button.styles.set("main", StateMask::DEFAULT, StyleProp::BgColor, StyleValue::Color(Color::from_hex(0xF15C00)));
        button.styles.set(
            "main",
            StateMask::DEFAULT.with(ObjState::Pressed),
            StyleProp::TextFont,
            StyleValue::Font("FontCBI40".into()),
        );
        button.events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::ChangeScreen { screen: Some(second), anim: ScreenAnim::FadeOn, time_ms: 300, delay_ms: 0 },
        });
        button.events.push(EventHandler {
            trigger: EventTrigger::LongPressed,
            action: Action::CallFunction { name: "beep".into() },
        });
        project.insert(screen, 0, button).unwrap();
        let textarea = project.new_widget(WidgetKind::Textarea);
        let textarea_id = textarea.id;
        project.insert(screen, 1, textarea).unwrap();
        let mut keyboard = project.new_widget(WidgetKind::Keyboard);
        keyboard.set_prop("textarea", PropValue::WidgetRef(Some(textarea_id)));
        project.insert(screen, 2, keyboard).unwrap();
        let mut dropdown = project.new_widget(WidgetKind::Dropdown);
        dropdown.styles.set("list_main", StateMask::DEFAULT, StyleProp::Radius, StyleValue::Int(4));
        project.insert(screen, 3, dropdown).unwrap();
        let mut chart = project.new_widget(WidgetKind::Chart);
        chart.set_prop("series", PropValue::Text("#FF0000: 1, 2".into()));
        chart.set_prop("point_count", PropValue::Int(3));
        project.insert(screen, 4, chart).unwrap();
        let meter = project.new_widget(WidgetKind::Meter);
        project.insert(screen, 5, meter).unwrap();
        project
    }

    fn file<'a>(sources: &'a UiSources, path: &str) -> &'a str {
        sources.files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("{path} missing")).contents_str()
    }

    #[test]
    fn screen_file_contains_all_statements() {
        let sources = generate(&project(), &AssetDecls::default());
        let screen = file(&sources, "screens/ui_Screen1.c");
        for expected in [
            "lv_obj_t * ui_Button1 = NULL;",
            "ui_Screen1 = lv_obj_create(NULL);",
            "ui_Button1 = lv_btn_create(ui_Screen1);",
            "lv_obj_set_y(ui_Button1, lv_pct(10));",
            "lv_obj_set_align(ui_Button1, LV_ALIGN_CENTER);",
            "lv_obj_add_flag(ui_Button1, LV_OBJ_FLAG_SCROLLABLE);",
            "lv_obj_add_state(ui_Button1, LV_STATE_CHECKED);",
            "lv_obj_set_style_bg_color(ui_Button1, lv_color_hex(0xF15C00), LV_PART_MAIN | LV_STATE_DEFAULT);",
            "lv_obj_set_style_text_font(ui_Button1, &ui_font_FontCBI40, LV_PART_MAIN | LV_STATE_PRESSED);",
            "ui_Button1Label = lv_label_create(ui_Button1);",
            "lv_label_set_text(ui_Button1Label, \"Button1\");",
            "lv_obj_add_event_cb(ui_Button1, ui_event_Button1, LV_EVENT_ALL, NULL);",
            "_ui_screen_change(&ui_Screen2, LV_SCR_LOAD_ANIM_FADE_ON, 300, 0, &ui_Screen2_screen_init);",
            "beep(e);",
            "lv_obj_set_style_radius(lv_dropdown_get_list(ui_Dropdown1), 4, LV_PART_MAIN | LV_STATE_DEFAULT);",
            "static lv_coord_t ui_Chart1_series_0_array[] = {1, 2, LV_CHART_POINT_NONE};",
            "lv_meter_set_scale_range(ui_Meter1, ui_Meter1_scale, 0, 100, 270, 135);",
        ] {
            assert!(screen.contains(expected), "missing `{expected}` in\n{screen}");
        }
        let keyboard_ref = screen.find("lv_keyboard_set_textarea(ui_Keyboard1, ui_Textarea1);").unwrap();
        assert!(keyboard_ref > screen.find("ui_Meter1 = lv_meter_create").unwrap(), "references are deferred");
    }

    #[test]
    fn header_and_init() {
        let sources = generate(&project(), &AssetDecls::default());
        let ui_h = file(&sources, "ui.h");
        assert!(ui_h.contains("extern lv_obj_t * ui_Button1;"));
        assert!(ui_h.contains("void ui_event_Button1(lv_event_t * e);"));
        assert!(ui_h.contains("void ui_Screen2_screen_init(void);"));
        let ui_c = file(&sources, "ui.c");
        assert!(ui_c.contains("lv_theme_default_init(dispp, lv_palette_main(LV_PALETTE_BLUE), lv_palette_main(LV_PALETTE_RED), true, LV_FONT_DEFAULT);"));
        assert!(ui_c.contains("lv_disp_load_scr(ui_Screen1);"));
        assert!(file(&sources, "ui_events.h").contains("void beep(lv_event_t * e);"));
        assert!(file(&sources, "ui_events_weak.c").contains("__attribute__((weak)) void beep"));
    }

    #[test]
    fn temporary_screens_are_created_on_demand() {
        let mut project = project();
        let second = project.screens[1].id;
        project.widget_mut(second).unwrap().set_prop("temporary", PropValue::Bool(true));
        let sources = generate(&project, &AssetDecls::default());
        assert!(!file(&sources, "ui.c").contains("ui_Screen2_screen_init();"));
        let screen = file(&sources, "screens/ui_Screen2.c");
        assert!(screen.contains("void ui_Screen2_screen_destroy(void)"));
        assert!(screen.contains("LV_EVENT_SCREEN_UNLOADED"));
    }

    #[test]
    fn braces_are_balanced() {
        let sources = generate(&project(), &AssetDecls::default());
        for f in &sources.files {
            let text = f.contents_str();
            let open = text.matches('{').count();
            let close = text.matches('}').count();
            assert_eq!(open, close, "{}", f.path);
        }
    }
}
