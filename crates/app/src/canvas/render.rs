//! Paints an LVGL layout with egui, approximating LVGL's own drawing.

use eframe::egui::epaint::{CornerRadius, Mesh, PathShape, Shadow, Vertex};
use eframe::egui::text::{LayoutJob, TextFormat, TextWrapping};
use eframe::egui::{self, Align, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};
use lvglab_model::flags::{ObjState, StateMask};
use lvglab_model::layout::{self as lay, Layout, parse_points};
use lvglab_model::style::StyleProp as P;
use lvglab_model::theme::StyleResolver;
use lvglab_model::{Color, Project, Widget, WidgetId, WidgetKind};

use super::assets::AssetCache;

/// Everything needed to paint a screen.
pub struct Renderer<'a> {
    pub painter: Painter,
    pub origin: Pos2,
    pub zoom: f32,
    pub project: &'a Project,
    pub assets: &'a AssetCache,
    pub resolver: &'a StyleResolver<'a>,
    /// Extra state shown for one widget (the style editor's state).
    pub preview_state: Option<(WidgetId, StateMask)>,
}

/// Resolved drawing parameters of one part.
#[derive(Clone)]
struct Look {
    opa: f32,
    bg: Color32,
    grad: Option<(Color32, bool, f32, f32)>,
    bg_image: Option<String>,
    radius: f32,
    border_width: f32,
    border_color: Color32,
    border_side: String,
    outline_width: f32,
    outline_color: Color32,
    outline_pad: f32,
    shadow: Option<(f32, Vec2, f32, Color32)>,
    text_color: Color32,
    font: FontId,
    /// LVGL line height of the font in screen points.
    line_h: f32,
    letter_space: f32,
    line_space: f32,
    text_align: Align,
    pad: (f32, f32, f32, f32),
    line_width: f32,
    line_color: Color32,
    line_rounded: bool,
    arc_width: f32,
    arc_color: Color32,
    arc_rounded: bool,
    arc_image: Option<String>,
    img_opa: f32,
    img_recolor: Option<Color32>,
}

fn rgba(color: Color, opa: u8, mul: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, (f32::from(opa) * mul).round() as u8)
}

impl<'a> Renderer<'a> {
    fn pos(&self, x: i32, y: i32) -> Pos2 {
        self.origin + vec2(x as f32, y as f32) * self.zoom
    }

    pub fn rect(&self, r: lay::Rect) -> Rect {
        Rect::from_min_size(self.pos(r.x, r.y), vec2(r.w as f32, r.h as f32) * self.zoom)
    }

    fn state_of(&self, widget: &Widget) -> StateMask {
        let mut state = widget.state_mask();
        if let Some((id, extra)) = self.preview_state {
            if id == widget.id {
                state = StateMask(state.0 | extra.0);
            }
        }
        state
    }

    fn look(&self, path: &[&Widget], slot: &str, state: StateMask) -> Look {
        let r = self.resolver;
        let z = self.zoom;
        let opa = f32::from(r.opa(path, "main", path.last().map_or(state, |w| self.state_of(w)), P::Opa)) / 255.0;
        let color = |prop| r.color(path, slot, state, prop);
        let o = |prop| r.opa(path, slot, state, prop);
        let int = |prop| r.int(path, slot, state, prop) as f32;
        let text_color = color(P::TextColor).unwrap_or(Color::BLACK);
        let grad_dir = r.get(path, slot, state, P::BgGradDir).and_then(|v| v.as_str().map(str::to_owned));
        let bg_color = color(P::BgColor).unwrap_or(Color::WHITE);
        let bg = rgba(bg_color, o(P::BgOpa), opa);
        let grad = match grad_dir.as_deref() {
            Some("LV_GRAD_DIR_VER") | Some("LV_GRAD_DIR_HOR") => Some((
                rgba(color(P::BgGradColor).unwrap_or(Color::BLACK), o(P::BgOpa), opa),
                grad_dir.as_deref() == Some("LV_GRAD_DIR_VER"),
                r.get(path, slot, state, P::BgMainStop).and_then(|v| v.as_int()).unwrap_or(0) as f32 / 255.0,
                r.get(path, slot, state, P::BgGradStop).and_then(|v| v.as_int()).unwrap_or(255) as f32 / 255.0,
            )),
            _ => None,
        };
        let shadow_width = int(P::ShadowWidth);
        let shadow = (shadow_width > 0.0 && o(P::ShadowOpa) > 0).then(|| {
            (
                shadow_width * z,
                vec2(int(P::ShadowOfsX), int(P::ShadowOfsY)) * z,
                int(P::ShadowSpread) * z,
                rgba(color(P::ShadowColor).unwrap_or(Color::BLACK), o(P::ShadowOpa), opa),
            )
        });
        let text_align =
            match r.get(path, slot, state, P::TextAlign).and_then(|v| v.as_str().map(str::to_owned)).as_deref() {
                Some("LV_TEXT_ALIGN_CENTER") => Align::Center,
                Some("LV_TEXT_ALIGN_RIGHT") => Align::RIGHT,
                _ => Align::LEFT,
            };
        let recolor_opa = o(P::ImgRecolorOpa);
        Look {
            opa,
            bg,
            grad,
            bg_image: r
                .get(path, slot, state, P::BgImgSrc)
                .and_then(|v| v.as_str().map(str::to_owned))
                .filter(|s| !s.is_empty()),
            radius: int(P::Radius),
            border_width: int(P::BorderWidth) * z,
            border_color: rgba(color(P::BorderColor).unwrap_or(Color::BLACK), o(P::BorderOpa), opa),
            border_side: r
                .get(path, slot, state, P::BorderSide)
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            outline_width: int(P::OutlineWidth) * z,
            outline_color: rgba(color(P::OutlineColor).unwrap_or(Color::BLACK), o(P::OutlineOpa), opa),
            outline_pad: int(P::OutlinePad) * z,
            shadow,
            text_color: rgba(text_color, o(P::TextOpa), opa),
            font: self.assets.font_id(self.project, &r.font(path, slot, state), z),
            line_h: self.assets.line_height(self.project, &r.font(path, slot, state)) * z,
            letter_space: int(P::TextLetterSpace) * z,
            line_space: int(P::TextLineSpace) * z,
            text_align,
            pad: (int(P::PadLeft) * z, int(P::PadTop) * z, int(P::PadRight) * z, int(P::PadBottom) * z),
            line_width: int(P::LineWidth).max(1.0) * z,
            line_color: rgba(color(P::LineColor).unwrap_or(text_color), o(P::LineOpa), opa),
            line_rounded: r.bool(path, slot, state, P::LineRounded),
            arc_width: int(P::ArcWidth) * z,
            arc_color: rgba(color(P::ArcColor).unwrap_or(Color::BLACK), o(P::ArcOpa), opa),
            arc_rounded: r.bool(path, slot, state, P::ArcRounded),
            arc_image: r
                .get(path, slot, state, P::ArcImgSrc)
                .and_then(|v| v.as_str().map(str::to_owned))
                .filter(|s| !s.is_empty()),
            img_opa: f32::from(o(P::ImgOpa)) / 255.0 * opa,
            img_recolor: (recolor_opa > 0)
                .then(|| rgba(color(P::ImgRecolor).unwrap_or(Color::BLACK), recolor_opa, 1.0)),
        }
    }

    // -- primitives --------------------------------------------------------

    fn corner(&self, look: &Look, rect: Rect) -> CornerRadius {
        let max = rect.width().min(rect.height()) / 2.0;
        CornerRadius::same((look.radius * self.zoom).min(max).clamp(0.0, 255.0) as u8)
    }

    fn draw_box(&self, rect: Rect, look: &Look) {
        let painter = &self.painter;
        let corner = self.corner(look, rect);
        if let Some((width, offset, spread, color)) = look.shadow {
            let shadow = Shadow {
                offset: [offset.x.clamp(-127.0, 127.0) as i8, offset.y.clamp(-127.0, 127.0) as i8],
                blur: width.clamp(0.0, 255.0) as u8,
                spread: spread.clamp(0.0, 255.0) as u8,
                color,
            };
            painter.add(shadow.as_shape(rect, corner));
        }
        if let Some((to, vertical, main_stop, grad_stop)) = look.grad {
            painter.add(gradient(rect, look.bg, to, vertical, main_stop, grad_stop));
        } else if look.bg.a() > 0 {
            painter.rect_filled(rect, corner, look.bg);
        }
        if let Some(image) = &look.bg_image {
            if let Some(texture) = self.assets.image(image) {
                let size = vec2(texture.size[0] as f32, texture.size[1] as f32) * self.zoom;
                let target = Rect::from_center_size(rect.center(), size);
                let tint = Color32::from_white_alpha((look.opa * 255.0) as u8);
                painter.image(texture.handle.id(), target, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), tint);
            }
        }
        if look.border_width > 0.0 && look.border_color.a() > 0 {
            self.draw_border(rect, corner, look);
        }
        if look.outline_width > 0.0 && look.outline_color.a() > 0 {
            let outer = rect.expand(look.outline_pad);
            painter.rect_stroke(
                outer,
                corner,
                Stroke::new(look.outline_width, look.outline_color),
                StrokeKind::Outside,
            );
        }
    }

    fn draw_border(&self, rect: Rect, corner: CornerRadius, look: &Look) {
        let stroke = Stroke::new(look.border_width, look.border_color);
        let side = look.border_side.as_str();
        if side.is_empty() || side.contains("FULL") {
            self.painter.rect_stroke(rect, corner, stroke, StrokeKind::Inside);
            return;
        }
        let half = look.border_width / 2.0;
        let inner = rect.shrink(half);
        let line = |a: Pos2, b: Pos2| {
            self.painter.line_segment([a, b], stroke);
        };
        if side.contains("TOP") {
            line(inner.left_top(), inner.right_top());
        }
        if side.contains("BOTTOM") {
            line(inner.left_bottom(), inner.right_bottom());
        }
        if side.contains("LEFT") {
            line(inner.left_top(), inner.left_bottom());
        }
        if side.contains("RIGHT") {
            line(inner.right_top(), inner.right_bottom());
        }
    }

    fn content(rect: Rect, look: &Look) -> Rect {
        let (l, t, r, b) = look.pad;
        Rect::from_min_max(pos2(rect.left() + l, rect.top() + t), pos2(rect.right() - r, rect.bottom() - b))
    }

    fn text_job(&self, text: &str, look: &Look, wrap_width: f32, recolor: bool) -> LayoutJob {
        let mut job = LayoutJob::default();
        let format = |color: Color32| TextFormat {
            font_id: look.font.clone(),
            color,
            extra_letter_spacing: look.letter_space,
            line_height: Some(look.line_h + look.line_space),
            ..TextFormat::default()
        };
        if recolor {
            for (segment, color) in recolor_segments(text) {
                let color = color.map_or(look.text_color, |c| rgba(c, look.text_color.a(), 1.0));
                job.append(&segment, 0.0, format(color));
            }
        } else {
            job.append(text, 0.0, format(look.text_color));
        }
        job.wrap = TextWrapping { max_width: wrap_width.max(1.0), ..TextWrapping::default() };
        job.halign = look.text_align;
        job
    }

    /// Draws text inside `rect`. `rows` limits the lines (with ellipsis).
    fn text(&self, rect: Rect, text: &str, look: &Look, wrap: bool, rows: Option<usize>, recolor: bool) {
        if text.is_empty() || look.text_color.a() == 0 {
            return;
        }
        let mut job = self.text_job(text, look, if wrap { rect.width() } else { f32::INFINITY }, recolor);
        if let Some(rows) = rows {
            job.wrap.max_rows = rows.max(1);
            job.wrap.overflow_character = Some('…');
        }
        let galley = self.painter.layout_job(job);
        let x = match look.text_align {
            Align::Center => rect.center().x,
            Align::Max => rect.right(),
            Align::Min => rect.left(),
        };
        self.painter.galley(pos2(x, rect.top()), galley, look.text_color);
    }

    fn text_size(&self, text: &str, look: &Look) -> Vec2 {
        let mut job = self.text_job(text, look, f32::INFINITY, false);
        job.halign = Align::LEFT;
        self.painter.layout_job(job).size()
    }

    /// Stroke parameters of an arc part, including its image (`arc_img_src`).
    fn stroke_of(&self, look: &Look) -> ArcStroke {
        let image = look
            .arc_image
            .as_deref()
            .and_then(|name| self.assets.image(name))
            .map(|t| (t.handle.id(), vec2(t.size[0] as f32, t.size[1] as f32) * self.zoom));
        ArcStroke { width: look.arc_width, color: look.arc_color, rounded: look.arc_rounded, image }
    }

    fn arc(&self, center: Pos2, radius: f32, start: f32, end: f32, stroke: ArcStroke) {
        let ArcStroke { width, color, rounded, image } = stroke;
        if width <= 0.0 || color.a() == 0 || radius <= 0.0 {
            return;
        }
        let mut end = end;
        while end < start {
            end += 360.0;
        }
        let span = (end - start).min(360.0);
        let steps = ((span / 3.0).ceil() as usize).max(2);
        let dirs: Vec<Vec2> = (0..=steps)
            .map(|i| {
                let angle = (start + span * i as f32 / steps as f32).to_radians();
                vec2(angle.cos(), angle.sin())
            })
            .collect();
        if let Some((texture, size)) = image {
            let caps = rounded && span < 360.0;
            self.textured_arc(center, radius, width, &dirs, (texture, size), color.a(), caps);
            return;
        }
        let points: Vec<Pos2> = dirs.iter().map(|d| center + *d * radius).collect();
        if rounded && span < 360.0 {
            self.painter.circle_filled(points[0], width / 2.0, color);
            self.painter.circle_filled(points[points.len() - 1], width / 2.0, color);
        }
        self.painter.add(PathShape::line(points, Stroke::new(width, color)));
    }

    /// LVGL draws `arc_img_src` aligned to the top left of the arc's outer
    /// bounding box and masks it with the arc (including rounded ends).
    #[allow(clippy::too_many_arguments)]
    fn textured_arc(
        &self,
        center: Pos2,
        radius: f32,
        width: f32,
        dirs: &[Vec2],
        (texture, size): (egui::TextureId, Vec2),
        alpha: u8,
        rounded: bool,
    ) {
        let (inner, outer) = ((radius - width / 2.0).max(0.0), radius + width / 2.0);
        let origin = center - vec2(outer, outer);
        let tint = Color32::from_white_alpha(alpha);
        let vertex = |pos: Pos2| Vertex { pos, uv: ((pos - origin) / size).to_pos2(), color: tint };
        let mut mesh = Mesh::with_texture(texture);
        for dir in dirs {
            for r in [inner, outer] {
                mesh.vertices.push(vertex(center + *dir * r));
            }
        }
        for i in 0..dirs.len().saturating_sub(1) as u32 {
            let a = 2 * i;
            mesh.add_triangle(a, a + 1, a + 2);
            mesh.add_triangle(a + 1, a + 3, a + 2);
        }
        if rounded {
            // A textured disc at each end, sampled from the same image position.
            const SEGMENTS: u32 = 24;
            for dir in [dirs.first(), dirs.last()].into_iter().flatten() {
                let cap = center + *dir * radius;
                let base = mesh.vertices.len() as u32;
                mesh.vertices.push(vertex(cap));
                for i in 0..SEGMENTS {
                    let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
                    mesh.vertices.push(vertex(cap + vec2(angle.cos(), angle.sin()) * (width / 2.0)));
                }
                for i in 0..SEGMENTS {
                    mesh.add_triangle(base, base + 1 + i, base + 1 + (i + 1) % SEGMENTS);
                }
            }
        }
        self.painter.add(mesh);
    }

    fn image(&self, name: &str, rect: Rect, look: &Look, zoom: f32, angle: f32) -> bool {
        let Some(texture) = self.assets.image(name) else { return false };
        let size = vec2(texture.size[0] as f32, texture.size[1] as f32) * self.zoom * zoom;
        let target = Rect::from_center_size(rect.center(), size);
        let mut tint = Color32::from_white_alpha((look.img_opa * 255.0) as u8);
        if let Some(recolor) = look.img_recolor {
            tint = blend(tint, recolor);
        }
        let mut mesh = Mesh::with_texture(texture.handle.id());
        mesh.add_rect_with_uv(target, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), tint);
        if angle != 0.0 {
            let (sin, cos) = angle.to_radians().sin_cos();
            let c = target.center();
            for v in &mut mesh.vertices {
                let d = v.pos - c;
                v.pos = c + vec2(d.x * cos - d.y * sin, d.x * sin + d.y * cos);
            }
        }
        self.painter.add(mesh);
        true
    }

    fn placeholder(&self, rect: Rect, label: &str) {
        let color = Color32::from_rgba_unmultiplied(128, 128, 128, 160);
        self.painter.rect_stroke(rect, 2, Stroke::new(1.0_f32, color), StrokeKind::Inside);
        self.painter.line_segment([rect.left_top(), rect.right_bottom()], Stroke::new(1.0_f32, color));
        self.painter.line_segment([rect.right_top(), rect.left_bottom()], Stroke::new(1.0_f32, color));
        self.painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0 * self.zoom.max(0.6)),
            color,
        );
    }

    // -- widgets -------------------------------------------------------------

    pub fn draw(&self, layout: &Layout<'_>, canvas_clip: Rect) {
        for (index, node) in layout.nodes.iter().enumerate() {
            if node.hidden {
                continue;
            }
            let clip = self.rect(node.clip).intersect(canvas_clip);
            if clip.width() <= 0.0 || clip.height() <= 0.0 {
                continue;
            }
            let renderer = Renderer { painter: self.painter.with_clip_rect(clip), ..*self };
            renderer.widget(&layout.path(index), self.rect(node.rect));
        }
    }

    fn widget(&self, path: &[&Widget], rect: Rect) {
        let widget = *path.last().expect("path is never empty");
        let state = self.state_of(widget);
        let main = self.look(path, "main", state);
        let part = |slot: &str| self.look(path, slot, state);
        if widget.kind != WidgetKind::Line {
            self.draw_box(rect, &main);
        }
        let content = Self::content(rect, &main);
        let z = self.zoom;
        let line_h = main.line_h;
        match widget.kind {
            WidgetKind::Label => {
                let mode = widget.prop_text("long_mode");
                let wrap = widget.geometry.width.unit != lvglab_model::Unit::Content;
                let rows = match mode.as_str() {
                    "LV_LABEL_LONG_DOT" => Some((content.height() / line_h).floor().max(1.0) as usize),
                    "LV_LABEL_LONG_SCROLL" | "LV_LABEL_LONG_SCROLL_CIRCULAR" | "LV_LABEL_LONG_CLIP" => None,
                    _ => None,
                };
                let wrap = wrap && !matches!(mode.as_str(), "LV_LABEL_LONG_SCROLL" | "LV_LABEL_LONG_SCROLL_CIRCULAR");
                let painter = self.painter.with_clip_rect(rect.intersect(self.painter.clip_rect()));
                let clipped = Renderer { painter, ..*self };
                clipped.text(content, &widget.prop_text("text"), &main, wrap, rows, widget.prop_bool("recolor"));
            }
            WidgetKind::Spangroup => {
                let text: String = lvglab_codegen::template::lines(&widget.prop_text("spans")).collect();
                self.text(content, &text, &main, true, None, false);
            }
            WidgetKind::Image => {
                let zoom = widget.prop_int("zoom") as f32 / 256.0;
                let angle = widget.prop_int("angle") as f32 / 10.0;
                if !self.image(&widget.prop_text("src"), rect, &main, zoom, angle) {
                    self.placeholder(rect, "IMG");
                }
            }
            WidgetKind::ImgButton => {
                let key = if state.contains(ObjState::Checked) { "checked_released" } else { "released" };
                let name = widget.prop_text(key);
                if !self.image(&name, rect, &main, 1.0, 0.0)
                    && !self.image(&widget.prop_text("released"), rect, &main, 1.0, 0.0)
                {
                    self.placeholder(rect, "IMGBTN");
                }
            }
            WidgetKind::AnimImg => {
                let first = widget.prop_text("images").lines().next().unwrap_or_default().trim().to_owned();
                if !self.image(&first, rect, &main, 1.0, 0.0) {
                    self.placeholder(rect, "ANIM");
                }
            }
            WidgetKind::Line => {
                let points: Vec<Pos2> = parse_points(&widget.prop_text("points"))
                    .iter()
                    .map(|(x, y)| rect.min + vec2(*x as f32, *y as f32) * z)
                    .collect();
                if points.len() >= 2 {
                    if main.line_rounded {
                        for p in &points {
                            self.painter.circle_filled(*p, main.line_width / 2.0, main.line_color);
                        }
                    }
                    self.painter.add(PathShape::line(points, Stroke::new(main.line_width, main.line_color)));
                }
            }
            WidgetKind::Bar | WidgetKind::Slider => self.bar(
                widget,
                rect,
                state,
                &part("indicator"),
                (widget.kind == WidgetKind::Slider).then(|| part("knob")),
            ),
            WidgetKind::Arc => self.arc_widget(widget, rect, &main, &part("indicator"), &part("knob")),
            WidgetKind::Spinner => {
                let indicator = part("indicator");
                let (c, r) = arc_geometry(content, main.arc_width.max(indicator.arc_width));
                self.arc(c, r, 0.0, 360.0, ArcStroke { rounded: false, ..self.stroke_of(&main) });
                let len = widget.prop_int("arc_length") as f32;
                self.arc(c, r, 270.0, 270.0 + len, self.stroke_of(&indicator));
            }
            WidgetKind::Switch => {
                let indicator = part("indicator");
                self.draw_box(rect, &indicator);
                let knob = part("knob");
                let d = (rect.height() * 0.76).max(2.0);
                let inset = (rect.height() - d) / 2.0;
                let x = if state.contains(ObjState::Checked) { rect.right() - inset - d } else { rect.left() + inset };
                self.draw_box(Rect::from_min_size(pos2(x, rect.top() + inset), vec2(d, d)), &knob);
            }
            WidgetKind::Checkbox => {
                let indicator = part("indicator");
                let size = line_h;
                let gap = self.resolver.int(path, "main", state, P::PadColumn) as f32 * z;
                let ind = Rect::from_min_size(
                    pos2(content.left(), content.top() + (content.height() - size).max(0.0) / 2.0),
                    vec2(size, size),
                );
                self.draw_box(ind, &indicator);
                if state.contains(ObjState::Checked) {
                    let s = size;
                    let p = |x: f32, y: f32| ind.min + vec2(x * s, y * s);
                    self.painter.add(PathShape::line(
                        vec![p(0.22, 0.52), p(0.42, 0.72), p(0.78, 0.3)],
                        Stroke::new((s * 0.12).max(1.0), indicator.text_color),
                    ));
                }
                let text_rect = Rect::from_min_max(pos2(ind.right() + gap, content.top()), content.max);
                self.text(text_rect, &widget.prop_text("text"), &main, false, None, false);
            }
            WidgetKind::Dropdown => {
                let fixed = widget.prop_text("text");
                let options = widget.prop_text("options");
                let selected = widget.prop_int("selected").max(0) as usize;
                let text =
                    if fixed.is_empty() { options.lines().nth(selected).unwrap_or_default().to_owned() } else { fixed };
                self.text(content, &text, &main, false, Some(1), false);
                let s = line_h * 0.3;
                let c = pos2(content.right() - s, content.top() + line_h / 2.0);
                let indicator = part("indicator");
                self.painter.add(PathShape::line(
                    vec![c + vec2(-s, -s * 0.5), c + vec2(0.0, s * 0.5), c + vec2(s, -s * 0.5)],
                    Stroke::new((1.6 * z).max(1.0), indicator.text_color),
                ));
            }
            WidgetKind::Roller => self.roller(widget, rect, content, &main, &part("selected")),
            WidgetKind::Textarea => {
                let text = widget.prop_text("text");
                if text.is_empty() {
                    self.text(content, &widget.prop_text("placeholder"), &part("placeholder"), true, None, false);
                } else if widget.prop_bool("password_mode") {
                    self.text(content, &"•".repeat(text.chars().count()), &main, true, None, false);
                } else {
                    self.text(content, &text, &main, !widget.prop_bool("one_line"), None, false);
                }
                if state.contains(ObjState::Focused) {
                    let cursor = part("cursor");
                    let x = content.left() + self.text_size(&text, &main).x;
                    self.painter.line_segment(
                        [pos2(x, content.top()), pos2(x, content.top() + line_h)],
                        Stroke::new(cursor.border_width.max(1.0), cursor.border_color),
                    );
                }
            }
            WidgetKind::Keyboard => self.keyboard(widget, content, path, state),
            WidgetKind::BtnMatrix => {
                let rows: Vec<Vec<String>> = lvglab_codegen::template::lines(&widget.prop_text("map"))
                    .map(|l| l.split('|').map(|s| s.trim().to_owned()).collect())
                    .collect();
                self.key_grid(content, &rows, &part("items"), main.pad.1.max(2.0 * z));
            }
            WidgetKind::Spinbox => {
                let text = spinbox_text(widget);
                self.text(content, &text, &main, false, Some(1), false);
            }
            WidgetKind::Colorwheel => {
                let (c, r) = arc_geometry(content, content.width().min(content.height()) * 0.12);
                let width = content.width().min(content.height()) * 0.12;
                let steps = 72;
                for i in 0..steps {
                    let a0 = i as f32 / steps as f32 * 360.0;
                    let color = hsv(a0 / 360.0, 1.0, 1.0);
                    self.arc(
                        c,
                        r,
                        a0,
                        a0 + 360.0 / steps as f32 + 0.8,
                        ArcStroke {
                            width,
                            color: Color32::from_rgba_unmultiplied(color.r, color.g, color.b, (255.0 * main.opa) as u8),
                            rounded: false,
                            image: None,
                        },
                    );
                }
                let knob = part("knob");
                let pos = c + vec2(r, 0.0);
                self.draw_box(Rect::from_center_size(pos, vec2(width, width) * 1.1), &knob);
            }
            WidgetKind::Led => {
                let led = widget.prop("color").and_then(|v| v.as_color()).unwrap_or(Color::rgb(33, 150, 243));
                let bright = widget.prop_int("brightness").clamp(0, 255) as u8;
                let color = led.mix(Color::BLACK, bright.max(80));
                let r = rect.width().min(rect.height()) / 2.0;
                let glow =
                    Color32::from_rgba_unmultiplied(led.r, led.g, led.b, (f32::from(bright) * 0.35 * main.opa) as u8);
                self.painter.circle_filled(rect.center(), r + 6.0 * z, glow);
                self.painter.circle_filled(rect.center(), r, rgba(color, 255, main.opa));
            }
            WidgetKind::Meter => self.meter(widget, content, &main, &part("ticks"), &part("indicator")),
            WidgetKind::Chart => self.chart(widget, content, &main),
            WidgetKind::Table => self.table(widget, rect, &part("items")),
            WidgetKind::Calendar => self.calendar(widget, content, &main, &part("days")),
            WidgetKind::Canvas => {
                let fill = widget.prop("fill").and_then(|v| v.as_color()).unwrap_or(Color::WHITE);
                self.painter.rect_filled(rect, 0, rgba(fill, 255, main.opa));
            }
            WidgetKind::Tabview => self.tabview(widget, path, rect, state),
            WidgetKind::Win => {
                let header_h = widget.prop_int("header_height") as f32 * z;
                let header = Rect::from_min_size(rect.min, vec2(rect.width(), header_h));
                let look = part("header");
                self.draw_box(header, &look);
                let title_rect = header.shrink2(vec2(10.0 * z, (header_h - line_h).max(0.0) / 2.0));
                self.text(title_rect, &widget.prop_text("title"), &look, false, Some(1), false);
            }
            WidgetKind::List | WidgetKind::Menu => {
                let items = widget.prop_text("items");
                let row_h = line_h + 20.0 * z;
                for (i, item) in lvglab_codegen::template::lines(&items).enumerate() {
                    let row = Rect::from_min_size(
                        pos2(content.left(), content.top() + i as f32 * row_h),
                        vec2(content.width(), row_h),
                    );
                    let text = row.shrink2(vec2(12.0 * z, (row_h - line_h) / 2.0));
                    self.text(text, item, &main, false, Some(1), false);
                    self.painter.line_segment(
                        [row.left_bottom(), row.right_bottom()],
                        Stroke::new(1.0_f32, main.border_color.gamma_multiply(0.6)),
                    );
                }
            }
            WidgetKind::Msgbox => {
                let title = widget.prop_text("title");
                self.text(content, &title, &main, false, Some(1), false);
                let body = Rect::from_min_max(pos2(content.left(), content.top() + line_h * 1.4), content.max);
                self.text(body, &widget.prop_text("text"), &main, true, None, false);
                let buttons = lvglab_codegen::template::str_array_entries(&widget.prop_text("buttons"), false);
                if !buttons.is_empty() {
                    let row = Rect::from_min_max(pos2(content.left(), content.bottom() - line_h * 1.6), content.max);
                    self.key_grid(row, &[buttons], &part("buttons"), 6.0 * z);
                }
                if widget.prop_bool("close_btn") {
                    self.painter.text(
                        pos2(content.right(), content.top()),
                        egui::Align2::RIGHT_TOP,
                        "✖",
                        main.font.clone(),
                        main.text_color,
                    );
                }
            }
            _ => {}
        }
    }

    fn bar(&self, widget: &Widget, rect: Rect, _state: StateMask, indicator: &Look, knob: Option<Look>) {
        let (min, max) = widget.prop_pair("range");
        let span = (max - min).max(1) as f32;
        let fraction = |v: i32| ((v - min) as f32 / span).clamp(0.0, 1.0);
        let value = fraction(widget.prop_int("value"));
        let range_mode = widget.prop_text("mode").ends_with("RANGE");
        let start = if range_mode {
            fraction(widget.prop_int(if widget.kind == WidgetKind::Slider { "value_left" } else { "start_value" }))
        } else {
            0.0
        };
        let horizontal = rect.width() >= rect.height();
        let ind = if horizontal {
            Rect::from_min_max(
                pos2(rect.left() + rect.width() * start, rect.top()),
                pos2(rect.left() + rect.width() * value, rect.bottom()),
            )
        } else {
            Rect::from_min_max(
                pos2(rect.left(), rect.bottom() - rect.height() * value),
                pos2(rect.right(), rect.bottom() - rect.height() * start),
            )
        };
        self.draw_box(ind, indicator);
        if let Some(knob) = knob {
            let size = rect.width().min(rect.height());
            let (l, t, r, b) = knob.pad;
            let center = if horizontal { pos2(ind.right(), rect.center().y) } else { pos2(rect.center().x, ind.top()) };
            let base = Rect::from_center_size(center, vec2(size, size));
            let knob_rect = Rect::from_min_max(base.min - vec2(l, t), base.max + vec2(r, b));
            self.draw_box(knob_rect, &knob);
        }
    }

    fn arc_widget(&self, widget: &Widget, rect: Rect, main: &Look, indicator: &Look, knob: &Look) {
        let content = Self::content(rect, main);
        let (c, r) = arc_geometry(content, main.arc_width.max(indicator.arc_width));
        let rotation = widget.prop_int("rotation") as f32;
        let (bg_start, mut bg_end) = widget.prop_pair("bg_angles");
        let bg_start = bg_start as f32 + rotation;
        if (bg_end as f32 + rotation) < bg_start {
            bg_end += 360;
        }
        let bg_end = bg_end as f32 + rotation;
        self.arc(c, r, bg_start, bg_end, self.stroke_of(main));
        let (min, max) = widget.prop_pair("range");
        let fraction = ((widget.prop_int("value") - min) as f32 / (max - min).max(1) as f32).clamp(0.0, 1.0);
        let span = bg_end - bg_start;
        let (from, to) = match widget.prop_text("mode").as_str() {
            "LV_ARC_MODE_REVERSE" => (bg_end - span * fraction, bg_end),
            "LV_ARC_MODE_SYMMETRICAL" => {
                let mid = bg_start + span / 2.0;
                let v = bg_start + span * fraction;
                (mid.min(v), mid.max(v))
            }
            _ => (bg_start, bg_start + span * fraction),
        };
        if to > from {
            self.arc(c, r, from, to, self.stroke_of(indicator));
        }
        let knob_angle = if widget.prop_text("mode") == "LV_ARC_MODE_REVERSE" { from } else { to };
        let pos = c + vec2(knob_angle.to_radians().cos(), knob_angle.to_radians().sin()) * r;
        let size = indicator.arc_width.max(2.0);
        let (l, t, rr, b) = knob.pad;
        let base = Rect::from_center_size(pos, vec2(size, size));
        self.draw_box(Rect::from_min_max(base.min - vec2(l, t), base.max + vec2(rr, b)), knob);
    }

    fn roller(&self, widget: &Widget, rect: Rect, content: Rect, main: &Look, selected: &Look) {
        let options: Vec<String> = widget.prop_text("options").lines().map(str::to_owned).collect();
        let row_h = main.line_h + main.line_space.max(4.0 * self.zoom);
        let sel = (widget.prop_int("selected").max(0) as usize).min(options.len().saturating_sub(1));
        let center_y = rect.center().y;
        let band = Rect::from_center_size(pos2(rect.center().x, center_y), vec2(rect.width(), row_h));
        self.draw_box(band, selected);
        for (i, option) in options.iter().enumerate() {
            let y = center_y + (i as f32 - sel as f32) * row_h - main.font.size * 0.6;
            let look = if i == sel { selected } else { main };
            let row = Rect::from_min_max(pos2(content.left(), y), pos2(content.right(), y + row_h));
            self.text(row, option, look, false, Some(1), false);
        }
    }

    fn key_grid(&self, area: Rect, rows: &[Vec<String>], items: &Look, gap: f32) {
        if rows.is_empty() {
            return;
        }
        let row_h = (area.height() - gap * (rows.len() as f32 - 1.0)) / rows.len() as f32;
        for (r, row) in rows.iter().enumerate() {
            let weights: Vec<f32> =
                row.iter().map(|k| if k.trim().is_empty() || k == "Space" { 4.0 } else { 1.0 }).collect();
            let total: f32 = weights.iter().sum();
            let width = area.width() - gap * (row.len() as f32 - 1.0);
            let mut x = area.left();
            for (key, weight) in row.iter().zip(&weights) {
                let w = width * weight / total;
                let key_rect = Rect::from_min_size(pos2(x, area.top() + r as f32 * (row_h + gap)), vec2(w, row_h));
                self.draw_box(key_rect, items);
                let size = self.text_size(key, items);
                let text_rect = Rect::from_min_size(key_rect.center() - size / 2.0, size);
                let mut centered = Look { text_align: Align::LEFT, ..items.clone() };
                centered.font = items.font.clone();
                self.text(text_rect, key, &centered, false, Some(1), false);
                x += w + gap;
            }
        }
    }

    fn keyboard(&self, widget: &Widget, content: Rect, path: &[&Widget], state: StateMask) {
        let mode = widget.prop_text("mode");
        let rows: &[&[&str]] = match mode.as_str() {
            "LV_KEYBOARD_MODE_TEXT_UPPER" => &[
                &["1#", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "Del"],
                &["abc", "A", "S", "D", "F", "G", "H", "J", "K", "L", "Enter"],
                &["_", "-", "Z", "X", "C", "V", "B", "N", "M", ".", ",", ":"],
                &["Kb", "<", " ", ">", "OK"],
            ],
            "LV_KEYBOARD_MODE_SPECIAL" => &[
                &["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "Del"],
                &["abc", "+", "-", "/", "*", "=", "%", "!", "?", "#", "<", ">"],
                &["\\", "@", "$", "(", ")", "{", "}", "[", "]", ";", "\"", "'"],
                &["Kb", "<", " ", ">", "OK"],
            ],
            "LV_KEYBOARD_MODE_TEXT_LOWER" => &[
                &["1#", "q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "Del"],
                &["ABC", "a", "s", "d", "f", "g", "h", "j", "k", "l", "Enter"],
                &["_", "-", "z", "x", "c", "v", "b", "n", "m", ".", ",", ":"],
                &["Kb", "<", " ", ">", "OK"],
            ],
            _ => {
                &[&["1", "2", "3", "Kb"], &["4", "5", "6", "OK"], &["7", "8", "9", "Del"], &["+/-", "0", ".", "<", ">"]]
            }
        };
        let rows: Vec<Vec<String>> = rows.iter().map(|r| r.iter().map(|k| (*k).to_owned()).collect()).collect();
        let items = self.look(path, "items", state);
        let gap = self.resolver.int(path, "main", state, P::PadRow).max(2) as f32 * self.zoom;
        self.key_grid(content, &rows, &items, gap);
    }

    fn meter(&self, widget: &Widget, content: Rect, main: &Look, ticks: &Look, indicator: &Look) {
        let (c, r) = arc_geometry(content, 0.0);
        let rotation = widget.prop_int("rotation") as f32;
        let angle_range = widget.prop_int("angle_range") as f32;
        let count = widget.prop_int("ticks").max(2);
        let major = widget.prop_int("major_every").max(1);
        let major_color = widget.prop("major_color").and_then(|v| v.as_color()).unwrap_or(Color::BLACK);
        let (min, max) = widget.prop_pair("range");
        let z = self.zoom;
        for i in 0..count {
            let a = (rotation + angle_range * i as f32 / (count - 1) as f32).to_radians();
            let dir = vec2(a.cos(), a.sin());
            let is_major = i % major == 0;
            let len = if is_major { 15.0 } else { 10.0 } * z;
            let color = if is_major { rgba(major_color, 255, main.opa) } else { ticks.line_color.gamma_multiply(0.7) };
            let width = if is_major { 3.0 } else { 1.5 } * z;
            self.painter.line_segment([c + dir * r, c + dir * (r - len)], Stroke::new(width, color));
            if is_major {
                let value = min + (max - min) * i / (count - 1);
                let label_pos = c + dir * (r - len - 12.0 * z);
                self.painter.text(
                    label_pos,
                    egui::Align2::CENTER_CENTER,
                    value.to_string(),
                    FontId::proportional(main.font.size * 0.8),
                    main.text_color,
                );
            }
        }
        let fraction = ((widget.prop_int("value") - min) as f32 / (max - min).max(1) as f32).clamp(0.0, 1.0);
        let a = (rotation + angle_range * fraction).to_radians();
        let needle = widget.prop("needle_color").and_then(|v| v.as_color()).unwrap_or(Color::rgb(33, 150, 243));
        self.painter.line_segment(
            [c, c + vec2(a.cos(), a.sin()) * (r - 10.0 * z)],
            Stroke::new(4.0 * z, rgba(needle, 255, main.opa)),
        );
        let hub = if indicator.bg.a() > 0 { indicator.bg } else { rgba(needle, 255, main.opa) };
        self.painter.circle_filled(c, 6.0 * z, hub);
    }

    fn chart(&self, widget: &Widget, content: Rect, main: &Look) {
        let (hdiv, vdiv) = widget.prop_pair("div_lines");
        let grid = Stroke::new(1.0_f32, main.border_color.gamma_multiply(0.7));
        for i in 1..=hdiv {
            let y = content.top() + content.height() * i as f32 / (hdiv + 1) as f32;
            self.painter.line_segment([pos2(content.left(), y), pos2(content.right(), y)], grid);
        }
        for i in 1..=vdiv {
            let x = content.left() + content.width() * i as f32 / (vdiv + 1) as f32;
            self.painter.line_segment([pos2(x, content.top()), pos2(x, content.bottom())], grid);
        }
        let (min, max) = widget.prop_pair("y_range");
        let count = widget.prop_int("point_count").max(1) as usize;
        let kind = widget.prop_text("chart_type");
        let (min2, max2) = widget.prop_pair("y2_range");
        let series = lvglab_model::series::parse(&widget.prop_text("series"));
        for (s, entry) in series.iter().enumerate() {
            let (lo, hi) = if entry.secondary { (min2, max2) } else { (min, max) };
            let y_of = |v: f32| content.bottom() - (v - lo as f32) / (hi - lo).max(1) as f32 * content.height();
            let values: Vec<f32> = entry.values.iter().map(|v| *v as f32).collect();
            let color = rgba(entry.color, 255, main.opa);
            if kind == "LV_CHART_TYPE_BAR" {
                let slot = content.width() / count as f32;
                let bar_w = slot / (series.len() as f32 + 1.0);
                for (i, v) in values.iter().take(count).enumerate() {
                    let x = content.left() + slot * i as f32 + bar_w * (s as f32 + 0.5);
                    self.painter.rect_filled(
                        Rect::from_min_max(pos2(x, y_of(*v)), pos2(x + bar_w * 0.9, content.bottom())),
                        2,
                        color,
                    );
                }
            } else {
                let step = content.width() / (count.max(2) - 1) as f32;
                let points: Vec<Pos2> = values
                    .iter()
                    .take(count)
                    .enumerate()
                    .map(|(i, v)| pos2(content.left() + step * i as f32, y_of(*v)))
                    .collect();
                if kind != "LV_CHART_TYPE_SCATTER" && points.len() > 1 {
                    self.painter.add(PathShape::line(points.clone(), Stroke::new(2.0 * self.zoom, color)));
                }
                for p in points {
                    self.painter.circle_filled(p, 3.0 * self.zoom, color);
                }
            }
        }
    }

    fn table(&self, widget: &Widget, rect: Rect, items: &Look) {
        let cells: Vec<Vec<String>> = lvglab_codegen::template::lines(&widget.prop_text("cells"))
            .map(|l| l.split('|').map(|c| c.trim().to_owned()).collect())
            .collect();
        let cols = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let col_w = match widget.prop_int("col_width") {
            0 => rect.width() / cols as f32,
            w => w as f32 * self.zoom,
        };
        let row_h = items.line_h + items.pad.1 + items.pad.3;
        for (r, row) in cells.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let cell_rect =
                    Rect::from_min_size(rect.min + vec2(c as f32 * col_w, r as f32 * row_h), vec2(col_w, row_h));
                self.draw_box(cell_rect, items);
                self.text(Self::content(cell_rect, items), cell, items, false, Some(1), false);
            }
        }
    }

    fn calendar(&self, widget: &Widget, content: Rect, main: &Look, days: &Look) {
        let (year, month) = match widget.prop("shown") {
            Some(lvglab_model::prop::PropValue::Date { year, month, .. }) => (year, month.clamp(1, 12)),
            _ => (2024, 1),
        };
        let today = match widget.prop("today") {
            Some(lvglab_model::prop::PropValue::Date { year: y, month: m, day }) if y == year && m == month => {
                Some(day)
            }
            _ => None,
        };
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let header_h = main.font.size * 2.0;
        let mut look = main.clone();
        look.text_align = Align::Center;
        let has_header = !widget.prop_text("header").is_empty();
        let mut top = content.top();
        if has_header {
            self.text(
                Rect::from_min_size(content.min, vec2(content.width(), header_h)),
                &format!("{year} {}", MONTHS[usize::from(month) - 1]),
                &look,
                false,
                Some(1),
                false,
            );
            top += header_h;
        }
        let cell_w = content.width() / 7.0;
        let cell_h = (content.bottom() - top) / 7.0;
        for (i, name) in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"].iter().enumerate() {
            self.text(
                Rect::from_min_size(pos2(content.left() + i as f32 * cell_w, top), vec2(cell_w, cell_h)),
                name,
                &look,
                false,
                Some(1),
                false,
            );
        }
        let first = day_of_week(year, month, 1);
        let count = days_in_month(year, month);
        for day in 1..=count {
            let index = first + usize::from(day) - 1;
            let (col, row) = (index % 7, index / 7 + 1);
            let cell = Rect::from_min_size(
                pos2(content.left() + col as f32 * cell_w, top + row as f32 * cell_h),
                vec2(cell_w, cell_h),
            );
            if today == Some(day) {
                self.painter.rect_stroke(
                    cell.shrink(2.0),
                    4,
                    Stroke::new(2.0 * self.zoom, self.resolver.theme_primary()),
                    StrokeKind::Inside,
                );
            }
            let mut day_look = days.clone();
            day_look.text_align = Align::Center;
            day_look.font = main.font.clone();
            if day_look.text_color.a() == 0 {
                day_look.text_color = main.text_color;
            }
            self.text(
                cell.shrink2(vec2(0.0, (cell_h - main.line_h).max(0.0) / 2.0)),
                &day.to_string(),
                &day_look,
                false,
                Some(1),
                false,
            );
        }
    }

    fn tabview(&self, widget: &Widget, path: &[&Widget], rect: Rect, state: StateMask) {
        let size = widget.prop_int("tab_size") as f32 * self.zoom;
        let pos = widget.prop_text("tab_pos");
        let bar = match pos.as_str() {
            "LV_DIR_BOTTOM" => Rect::from_min_max(pos2(rect.left(), rect.bottom() - size), rect.max),
            "LV_DIR_LEFT" => Rect::from_min_size(rect.min, vec2(size, rect.height())),
            "LV_DIR_RIGHT" => Rect::from_min_max(pos2(rect.right() - size, rect.top()), rect.max),
            _ => Rect::from_min_size(rect.min, vec2(rect.width(), size)),
        };
        let bar_look = self.look(path, "tab_bar", state);
        self.draw_box(bar, &bar_look);
        let tabs: Vec<String> = widget.children.iter().map(|c| c.prop_text("title")).collect();
        if tabs.is_empty() {
            return;
        }
        let vertical = matches!(pos.as_str(), "LV_DIR_LEFT" | "LV_DIR_RIGHT");
        let primary = self.resolver.theme_primary();
        for (i, title) in tabs.iter().enumerate() {
            let n = tabs.len() as f32;
            let cell = if vertical {
                Rect::from_min_size(
                    pos2(bar.left(), bar.top() + bar.height() * i as f32 / n),
                    vec2(bar.width(), bar.height() / n),
                )
            } else {
                Rect::from_min_size(
                    pos2(bar.left() + bar.width() * i as f32 / n, bar.top()),
                    vec2(bar.width() / n, bar.height()),
                )
            };
            let active = i == 0;
            let mut look = bar_look.clone();
            look.text_align = Align::Center;
            if active {
                look.text_color = primary;
                self.painter.rect_filled(
                    Rect::from_min_max(pos2(cell.left(), cell.bottom() - 3.0 * self.zoom), cell.max),
                    0,
                    primary,
                );
            }
            let text_h = look.line_h;
            self.text(
                cell.shrink2(vec2(2.0, (cell.height() - text_h).max(0.0) / 2.0)),
                title,
                &look,
                false,
                Some(1),
                false,
            );
        }
    }
}

/// Stroke parameters of an arc.
#[derive(Clone, Copy)]
struct ArcStroke {
    width: f32,
    color: Color32,
    rounded: bool,
    /// Texture and its on-screen size, for `arc_img_src`.
    image: Option<(egui::TextureId, Vec2)>,
}

/// Center and centerline radius of an arc of `width` filling `area`.
fn arc_geometry(area: Rect, width: f32) -> (Pos2, f32) {
    let r = area.width().min(area.height()) / 2.0 - width / 2.0;
    (area.center(), r.max(1.0))
}

fn gradient(rect: Rect, from: Color32, to: Color32, vertical: bool, main_stop: f32, grad_stop: f32) -> Shape {
    let mut mesh = Mesh::default();
    let stops = [(0.0, from), (main_stop, from), (grad_stop.max(main_stop), to), (1.0, to)];
    for (t, color) in stops {
        let (a, b) = if vertical {
            (pos2(rect.left(), rect.top() + rect.height() * t), pos2(rect.right(), rect.top() + rect.height() * t))
        } else {
            (pos2(rect.left() + rect.width() * t, rect.top()), pos2(rect.left() + rect.width() * t, rect.bottom()))
        };
        mesh.vertices.push(Vertex { pos: a, uv: Pos2::ZERO, color });
        mesh.vertices.push(Vertex { pos: b, uv: Pos2::ZERO, color });
    }
    for i in 0..3u32 {
        let base = i * 2;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 2, base + 3);
    }
    Shape::mesh(mesh)
}

fn blend(base: Color32, over: Color32) -> Color32 {
    let a = f32::from(over.a()) / 255.0;
    let mix = |x: u8, y: u8| (f32::from(x) * (1.0 - a) + f32::from(y) * a) as u8;
    Color32::from_rgba_unmultiplied(mix(base.r(), over.r()), mix(base.g(), over.g()), mix(base.b(), over.b()), base.a())
}

/// Splits LVGL recolor markup (`#ff0000 red# text`) into colored segments.
fn recolor_segments(text: &str) -> Vec<(String, Option<Color>)> {
    let mut segments = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('#') {
        let after = &rest[start + 1..];
        let color = after.get(..6).and_then(Color::parse);
        match (color, after.get(6..).and_then(|s| s.find('#'))) {
            (Some(color), Some(end)) => {
                segments.push((rest[..start].to_owned(), None));
                let body = &after[6..6 + end];
                segments.push((body.strip_prefix(' ').unwrap_or(body).to_owned(), Some(color)));
                rest = &after[6 + end + 1..];
            }
            _ => {
                segments.push((rest[..=start].to_owned(), None));
                rest = after;
            }
        }
    }
    segments.push((rest.to_owned(), None));
    // Merge neighbouring plain segments produced by stray '#' characters.
    let mut merged: Vec<(String, Option<Color>)> = Vec::new();
    for (text, color) in segments.into_iter().filter(|(s, _)| !s.is_empty()) {
        match merged.last_mut() {
            Some((last, None)) if color.is_none() => last.push_str(&text),
            _ => merged.push((text, color)),
        }
    }
    merged
}

fn spinbox_text(widget: &Widget) -> String {
    let digits = widget.prop_int("digit_count").clamp(1, 10) as usize;
    let separator = widget.prop_int("separator_position").max(0) as usize;
    let value = widget.prop_int("value");
    let (min, _) = widget.prop_pair("range");
    let mut text = format!("{:0digits$}", value.unsigned_abs());
    if separator > 0 && separator < digits {
        text.insert(separator, '.');
    }
    if min < 0 {
        text.insert(0, if value < 0 { '-' } else { '+' });
    }
    text
}

fn hsv(h: f32, s: f32, v: f32) -> Color {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    let (r, g, b) = match i as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// 0 = Sunday.
fn day_of_week(year: i32, month: u8, day: u8) -> usize {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    ((y + y / 4 - y / 100 + y / 400 + t[usize::from(month) - 1] + i32::from(day)).rem_euclid(7)) as usize
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

trait PrimaryColor {
    fn theme_primary(&self) -> Color32;
}

impl PrimaryColor for StyleResolver<'_> {
    fn theme_primary(&self) -> Color32 {
        let c = self.theme.primary;
        Color32::from_rgb(c.r, c.g, c.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recolor_markup() {
        let segments = recolor_segments("Hello #ff0000 red# world");
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[1], ("red".to_owned(), Some(Color::rgb(255, 0, 0))));
        assert_eq!(recolor_segments("50# off"), vec![("50# off".to_owned(), None)]);
    }

    #[test]
    fn calendar_math() {
        assert_eq!(day_of_week(2024, 1, 1), 1);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
    }

    #[test]
    fn spinbox_formatting() {
        let mut w = Widget::new(WidgetKind::Spinbox, WidgetId(1), "S");
        w.set_prop("value", lvglab_model::prop::PropValue::Int(1234));
        w.set_prop("separator_position", lvglab_model::prop::PropValue::Int(3));
        assert_eq!(spinbox_text(&w), "+012.34");
    }
}
