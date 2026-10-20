//! The WYSIWYG canvas: preview, selection, moving, resizing and dropping
//! widgets from the palette.

mod align;
pub mod assets;
mod render;

use std::collections::HashMap;

use eframe::egui::{self, Color32, CursorIcon, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use lvglab_model::coord::{Coord, Unit};
use lvglab_model::layout::{self as lay, Layout, Measure};
use lvglab_model::project::DisplayShape;
use lvglab_model::theme::{StyleResolver, Theme};
use lvglab_model::widget::Geometry;
use lvglab_model::{Project, WidgetId, WidgetKind};

use crate::app::Studio;
use crate::i18n::{tr, trf};
use crate::settings::{AppSettings, CanvasBackground};
use crate::visuals::{self, ACCENT};
use assets::AssetCache;
use render::Renderer;
pub use render::{key_at, keyboard_rows};

const HANDLE: f32 = 7.0;

/// What the canvas remembers about a placed widget between frames.
#[derive(Clone, Debug)]
pub struct NodeInfo {
    pub id: WidgetId,
    pub kind: WidgetKind,
    pub rect: lay::Rect,
    pub clip: lay::Rect,
    pub children_area: lay::Rect,
    /// The area inside border and padding.
    pub content: lay::Rect,
    pub parent: Option<usize>,
    pub hidden: bool,
    /// Position and size can be edited on the canvas.
    pub movable: bool,
}

#[derive(Clone, Copy)]
enum Drag {
    Move { id: WidgetId, start: lay::Rect, geometry: Geometry },
    Resize { id: WidgetId, handle: usize, start: lay::Rect, geometry: Geometry },
}

pub struct CanvasState {
    pub zoom: f32,
    pub pan: Vec2,
    pub fit_pending: bool,
    /// Refit when the canvas is resized until the user zooms or pans.
    pub auto_fit: bool,
    last_size: Vec2,
    pub nodes: Vec<NodeInfo>,
    drag: Option<Drag>,
    /// All screens side by side instead of the current one.
    pub overview: bool,
    /// Where the display is drawn, in points (for the screenshot export).
    pub display_points: Rect,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            auto_fit: true,
            last_size: Vec2::ZERO,
            nodes: Vec::new(),
            drag: None,
            overview: false,
            display_points: Rect::NOTHING,
        }
    }
}

impl CanvasState {
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    fn node(&self, id: WidgetId) -> Option<(usize, &NodeInfo)> {
        self.nodes.iter().enumerate().find(|(_, n)| n.id == id)
    }

    fn hit(&self, x: i32, y: i32) -> Option<usize> {
        self.nodes.iter().rposition(|n| !n.hidden && n.rect.contains(x, y) && n.clip.contains(x, y))
    }

    /// Geometry of `id` moved by `(dx, dy)` display pixels.
    pub fn nudged(&self, project: &Project, id: WidgetId, dx: i32, dy: i32) -> Option<Geometry> {
        let (_, node) = self.node(id)?;
        if !node.movable {
            return None;
        }
        let widget = project.widget(id)?;
        let parent = &self.nodes[node.parent?];
        let rect = lay::Rect { x: node.rect.x + dx, y: node.rect.y + dy, ..node.rect };
        Some(geometry_for_rect(widget.geometry, rect, parent, false))
    }
}

/// Converts a desired display rectangle back into a geometry that keeps
/// the widget's alignment and units.
fn geometry_for_rect(mut g: Geometry, rect: lay::Rect, parent: &NodeInfo, resized: bool) -> Geometry {
    let area = parent.children_area;
    let (ax, ay) = g.align.anchor(area.tuple(), parent.rect.tuple(), (rect.w, rect.h));
    let to_unit = |unit: Unit, px: i32, reference: i32| match unit {
        Unit::Percent if reference > 0 => Coord::pct((px as f32 * 100.0 / reference as f32).round() as i32),
        _ => Coord::px(px),
    };
    g.x = to_unit(g.x.unit, rect.x - ax, area.w);
    g.y = to_unit(g.y.unit, rect.y - ay, area.h);
    if resized {
        g.width = to_unit(g.width.unit, rect.w.max(1), area.w);
        g.height = to_unit(g.height.unit, rect.h.max(1), area.h);
    }
    g
}

struct EguiMeasure<'a> {
    ctx: &'a egui::Context,
    assets: &'a AssetCache,
    project: &'a Project,
    images: HashMap<String, (i32, i32)>,
}

impl Measure for EguiMeasure<'_> {
    fn text_size(
        &self,
        text: &str,
        font: &str,
        letter_space: i32,
        line_space: i32,
        max_width: Option<i32>,
    ) -> (i32, i32) {
        let font_id = self.assets.font_id(self.project, font, 1.0);
        // LVGL's line height (ascent - descent) instead of egui's row height,
        // which includes the font's line gap.
        let line_height = self.assets.line_height(self.project, font) + line_space as f32;
        let format = egui::TextFormat {
            font_id,
            extra_letter_spacing: letter_space as f32,
            line_height: Some(line_height),
            ..Default::default()
        };
        let mut job = egui::text::LayoutJob::single_section(text.to_owned(), format);
        job.wrap.max_width = max_width.map_or(f32::INFINITY, |w| w as f32);
        let size = self.ctx.fonts(|f| f.layout_job(job)).size();
        (size.x.ceil() as i32, size.y.ceil() as i32)
    }

    fn line_height(&self, font: &str) -> i32 {
        self.assets.line_height(self.project, font).round() as i32
    }

    fn image_size(&self, image: &str) -> Option<(i32, i32)> {
        self.images.get(image).copied()
    }
}

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    toolbar(ui, studio);
    ui.add_space(2.0);
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let palette = visuals::palette(ui.ctx());
    let painter = ui.painter_at(rect);
    let background = draw_background(&painter, rect, studio.settings.canvas_background, palette.canvas);
    if studio.canvas.overview && studio.simulation.is_none() {
        overview(ui, studio, &response, &painter, rect, background);
        return;
    }

    let (width, height) = (studio.doc.project.display.width as i32, studio.doc.project.display.height as i32);
    let view = &mut studio.canvas;
    if view.auto_fit && (rect.size() - view.last_size).length() > 1.0 {
        view.fit_pending = true;
    }
    view.last_size = rect.size();
    if view.fit_pending && rect.width() > 50.0 {
        view.zoom = ((rect.width() - 80.0) / width as f32).min((rect.height() - 80.0) / height as f32).clamp(0.1, 8.0);
        view.pan = Vec2::ZERO;
        view.fit_pending = false;
    }
    handle_zoom_and_pan(ui, &response, rect, view, width, height);
    let zoom = view.zoom;
    let display_size = vec2(width as f32, height as f32) * zoom;
    let origin = (rect.center() - display_size / 2.0 + view.pan).round();
    let display = Rect::from_min_size(origin, display_size);

    if studio.settings.show_grid {
        draw_grid(&painter, display, zoom, studio.settings.grid_size.max(2) as f32, palette.canvas_grid);
    }

    // Lay out and paint the current screen (of the simulation, if one runs).
    let (project, screen_id) = match &studio.simulation {
        Some(sim) => (&sim.project, sim.screen),
        None => (&studio.doc.project, studio.screen),
    };
    let Some(screen) = project.widget(screen_id) else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            tr("No screen"),
            egui::FontId::proportional(16.0),
            palette.muted,
        );
        return;
    };
    let theme = Theme::of(project);
    let resolver = StyleResolver::new(&theme).with_color_themes(&project.color_themes);
    let measure = EguiMeasure { ctx: ui.ctx(), assets: &studio.assets, project, images: studio.assets.image_sizes() };
    let preview_state = match &studio.simulation {
        Some(sim) => sim.preview_state(),
        None => studio.selected.filter(|_| studio.style_state.0 != 0).map(|id| (id, studio.style_state)),
    };
    let clip = display.intersect(rect);
    // LVGL draws the screen itself when the preview is set to it.
    let lvgl_picture = if studio.settings.preview_renderer == crate::settings::PreviewRenderer::Lvgl
        && studio.simulation.is_none()
        && !project.lvgl_version.is_v9()
    {
        let dir = studio.doc.dir().map(std::path::Path::to_path_buf);
        studio.lvgl_preview.texture(ui.ctx(), project, screen_id, dir.as_deref())
    } else {
        None
    };
    // Lays out and paints one screen; offsets are fractions of the display.
    let paint = |widget: &lvglab_model::Widget, offset: (f32, f32), opacity: f32, preview| {
        // Component instances are shown with the content of their component.
        let expanded = project.expand_instances(widget);
        let layout = Layout::compute(&expanded, width, height, &resolver, &measure, studio.selected);
        let mut screen_painter = painter.clone();
        screen_painter.set_opacity(opacity);
        let renderer = Renderer {
            painter: screen_painter,
            origin: origin + vec2(offset.0 * display_size.x, offset.1 * display_size.y),
            zoom,
            project,
            assets: &studio.assets,
            resolver: &resolver,
            preview_state: preview,
        };
        match &lvgl_picture {
            Some(texture) if offset == (0.0, 0.0) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.image(texture.id(), display, uv, Color32::WHITE);
            }
            _ => renderer.draw(&layout, clip),
        }
        node_infos(&layout)
    };
    let now = ui.input(|i| i.time);
    let transition = studio.simulation.as_ref().and_then(|s| s.transition).map(|t| (t, t.frame(now)));
    let nodes = match transition {
        Some((t, frame)) => {
            let old = project.widget(t.from);
            let paint_old = || {
                if let Some(old) = old {
                    paint(old, frame.old_offset, frame.old_opacity, None);
                }
            };
            if frame.new_on_top {
                paint_old();
                paint(screen, frame.new_offset, frame.new_opacity, preview_state)
            } else {
                let nodes = paint(screen, frame.new_offset, frame.new_opacity, preview_state);
                paint_old();
                nodes
            }
        }
        None => paint(screen, (0.0, 0.0), 1.0, preview_state),
    };
    draw_display_frame(&painter, display, project.display.shape, background, rect);
    studio.canvas.nodes = nodes;
    studio.canvas.display_points = display;
    if lvgl_picture.is_some() {
        if let Some(missing) = studio.lvgl_preview.missing.as_deref() {
            let skipped = studio.lvgl_preview.skipped.join("\n");
            let spot = rect.left_bottom() + vec2(10.0, -8.0);
            let text = egui::WidgetText::from(missing).color(visuals::WARNING).into_galley(
                ui,
                None,
                f32::INFINITY,
                egui::TextStyle::Body,
            );
            let area = Rect::from_min_size(spot - vec2(0.0, text.size().y), text.size());
            painter.galley(area.min, text, visuals::WARNING);
            ui.interact(area, ui.id().with("lvgl_missing"), Sense::hover()).on_hover_text(skipped);
        }
    }
    if studio.settings.preview_renderer == crate::settings::PreviewRenderer::Lvgl && lvgl_picture.is_none() {
        if let Some(reason) = studio.lvgl_preview.error.as_deref().filter(|_| studio.simulation.is_none()) {
            painter.text(
                rect.left_bottom() + vec2(10.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{} {reason}", tr("LVGL preview not available:")),
                egui::FontId::proportional(12.0),
                visuals::WARNING,
            );
        }
    }
    if studio.shots.is_some() {
        // The screenshot export wants the plain screen.
        return;
    }

    let to_display = |p: Pos2| -> (i32, i32) {
        let d = (p - origin) / zoom;
        (d.x.floor() as i32, d.y.floor() as i32)
    };
    let screen_rect = |r: lay::Rect| {
        Rect::from_min_size(origin + vec2(r.x as f32, r.y as f32) * zoom, vec2(r.w as f32, r.h as f32) * zoom)
    };

    if studio.simulation.is_some() {
        simulate(ui, studio, &response, &painter, rect, display, &to_display);
        return;
    }

    draw_outlines(&painter, &studio.canvas.nodes, &screen_rect, &studio.settings);

    // Hover and selection overlays.
    let hovered = response.hover_pos().and_then(|p| {
        let (x, y) = to_display(p);
        studio.canvas.hit(x, y)
    });
    if let Some(i) = hovered {
        let node = &studio.canvas.nodes[i];
        if Some(node.id) != studio.selected && i != 0 {
            painter.rect_stroke(
                screen_rect(node.rect),
                0,
                Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.6)),
                StrokeKind::Outside,
            );
        }
    }
    // Further selected widgets (Ctrl+click).
    for id in &studio.extra_selection {
        if let Some((_, node)) = studio.canvas.node(*id) {
            let r = screen_rect(node.rect);
            let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            painter.extend(egui::Shape::dashed_line(&corners, Stroke::new(1.5_f32, ACCENT), 5.0, 3.0));
        }
    }
    let selected = studio.selected.and_then(|id| studio.canvas.node(id).map(|(i, n)| (i, n.clone())));
    let handles = selected.as_ref().filter(|(_, n)| n.movable).map(|(_, n)| handle_rects(screen_rect(n.rect)));
    if let Some((_, node)) = &selected {
        let r = screen_rect(node.rect);
        let stroke = if node.hidden { Stroke::new(1.0_f32, visuals::WARNING) } else { Stroke::new(1.5_f32, ACCENT) };
        painter.rect_stroke(r, 0, stroke, StrokeKind::Outside);
        if let Some(handles) = &handles {
            for h in handles {
                painter.rect_filled(*h, 2, Color32::WHITE);
                painter.rect_stroke(*h, 2, Stroke::new(1.0_f32, ACCENT), StrokeKind::Inside);
            }
        }
        let label = studio
            .doc
            .project
            .widget(node.id)
            .filter(|_| studio.settings.show_size_label)
            .map(|w| format!("{}  {}×{}", w.name, node.rect.w, node.rect.h));
        if let Some(label) = label {
            let pos = r.left_top() - vec2(0.0, 4.0);
            painter.text(pos, egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(11.0), ACCENT);
        }
    }

    // Cursor feedback.
    if let (Some(handles), Some(p)) = (&handles, response.hover_pos()) {
        if let Some(h) = handles.iter().position(|r| r.expand(2.0).contains(p)) {
            ui.ctx().set_cursor_icon(handle_cursor(h));
        }
    }

    interact(ui, studio, &response, handles, to_display, zoom);
    drop_from_palette(ui, studio, &response, &to_display, &screen_rect, &painter);
}

/// Columns of the overview grid that give the largest screens.
fn overview_columns(count: usize, area: Vec2, display: Vec2, gap: f32, label: f32) -> (usize, f32) {
    (1..=count.max(1))
        .map(|cols| {
            let rows = count.max(1).div_ceil(cols);
            let w = (area.x - gap * (cols + 1) as f32) / (cols as f32 * display.x);
            let h = (area.y - gap * (rows + 1) as f32 - label * rows as f32) / (rows as f32 * display.y);
            (cols, w.min(h).max(0.01))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((1, 1.0))
}

/// All screens side by side; a click opens one.
fn overview(
    ui: &egui::Ui,
    studio: &mut Studio,
    response: &egui::Response,
    painter: &egui::Painter,
    rect: Rect,
    background: Color32,
) {
    const GAP: f32 = 24.0;
    const LABEL: f32 = 22.0;
    let project = &studio.doc.project;
    let display = vec2(project.display.width.max(1) as f32, project.display.height.max(1) as f32);
    let count = project.screens.len();
    let (cols, zoom) = overview_columns(count, rect.size(), display, GAP, LABEL);
    let zoom = zoom.min(2.0);
    let tile = display * zoom;
    let rows = count.max(1).div_ceil(cols);
    let used = vec2(cols as f32 * (tile.x + GAP) - GAP, rows as f32 * (tile.y + LABEL + GAP) - GAP);
    let start = rect.center() - used / 2.0;
    let theme = Theme::of(project);
    let resolver = StyleResolver::new(&theme).with_color_themes(&project.color_themes);
    let measure = EguiMeasure { ctx: ui.ctx(), assets: &studio.assets, project, images: studio.assets.image_sizes() };
    let (width, height) = (display.x as i32, display.y as i32);
    let palette = visuals::palette(ui.ctx());
    let mut open = None;
    for (i, screen) in project.screens.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let origin = (start + vec2(col as f32 * (tile.x + GAP), row as f32 * (tile.y + LABEL + GAP))).round();
        let area = Rect::from_min_size(origin, tile);
        let clip = area.intersect(rect);
        let expanded = project.expand_instances(screen);
        let layout = Layout::compute(&expanded, width, height, &resolver, &measure, None);
        let renderer = Renderer {
            painter: painter.with_clip_rect(clip),
            origin,
            zoom,
            project,
            assets: &studio.assets,
            resolver: &resolver,
            preview_state: None,
        };
        renderer.draw(&layout, clip);
        // The round mask stays inside the tile, so it does not cover the neighbors.
        draw_display_frame(&painter.with_clip_rect(clip), area, project.display.shape, background, clip);
        let hovered = response.hover_pos().is_some_and(|p| area.expand(4.0).contains(p));
        let current = screen.id == studio.screen;
        let stroke = match (hovered, current) {
            (true, _) => Stroke::new(2.0_f32, ACCENT),
            (false, true) => Stroke::new(2.0_f32, ACCENT.gamma_multiply(0.7)),
            _ => Stroke::new(1.0_f32, palette.muted.gamma_multiply(0.5)),
        };
        match project.display.shape {
            DisplayShape::Round => {
                painter.circle_stroke(area.center(), area.width().min(area.height()) / 2.0 + 1.0, stroke);
            }
            DisplayShape::Rectangle => {
                painter.rect_stroke(area, 2, stroke, StrokeKind::Outside);
            }
        }
        let name = if i == 0 { format!("{}  · {}", screen.name, tr("start")) } else { screen.name.clone() };
        painter.text(
            area.center_bottom() + vec2(0.0, 6.0),
            egui::Align2::CENTER_TOP,
            name,
            egui::FontId::proportional(13.0),
            if current { ACCENT } else { ui.visuals().text_color() },
        );
        if hovered {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            if response.clicked() {
                open = Some(screen.id);
            }
        }
    }
    if let Some(id) = open {
        studio.select_screen(id);
        studio.canvas.overview = false;
        studio.canvas.fit_pending = true;
    }
}

/// Plays the UI: mouse input goes to the simulation instead of the editor.
fn simulate(
    ui: &egui::Ui,
    studio: &mut Studio,
    response: &egui::Response,
    painter: &egui::Painter,
    rect: Rect,
    display: Rect,
    to_display: &impl Fn(Pos2) -> (i32, i32),
) {
    let now = ui.input(|i| i.time);
    let display_size = (studio.doc.project.display.width, studio.doc.project.display.height);
    if let Some(recorder) = &mut studio.recorder {
        recorder.request(ui.ctx(), now, display.intersect(rect), display_size);
        let seconds = recorder.seconds(now);
        let label = format!("\u{E3EE} REC {}:{:02}", seconds as u32 / 60, seconds as u32 % 60);
        painter.text(
            rect.left_top() + vec2(10.0, 8.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(13.0),
            visuals::DANGER,
        );
        if recorder.is_full(now) {
            studio.stop_recording();
        }
    }
    let Some(sim) = &mut studio.simulation else { return };
    let nodes = &studio.canvas.nodes;
    let now = ui.input(|i| i.time);
    let (pointer, pressed, down, released) = ui.input(|i| {
        (i.pointer.latest_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released())
    });
    let inside = pointer.filter(|p| response.hovered() && display.contains(*p));
    if let Some(p) = inside {
        let (x, y) = to_display(p);
        if pressed {
            sim.press(nodes, x, y, now);
        } else if down {
            sim.drag(x, y, now);
        }
        if sim.record {
            ui.ctx().set_cursor_icon(CursorIcon::None);
        } else if sim.is_clickable_at(nodes, x, y) || sim.pressed().is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }
    if released {
        if let Some(p) = pointer.filter(|p| rect.contains(*p)) {
            let (x, y) = to_display(p);
            sim.release(x, y, p, now);
        }
    } else if !down && sim.pressed().is_some() {
        // The button was released outside the window.
        sim.cancel(now);
    }
    sim.tick(now);
    if sim.record {
        sim.draw_overlay(painter, inside, down, now);
    }
    // Mark the canvas as running.
    painter.rect_stroke(rect.shrink(1.0), 0, Stroke::new(2.0_f32, visuals::SUCCESS), StrokeKind::Inside);
    if sim.is_animating(now) || sim.record {
        ui.ctx().request_repaint();
    }
}

fn interact(
    ui: &egui::Ui,
    studio: &mut Studio,
    response: &egui::Response,
    handles: Option<[Rect; 8]>,
    to_display: impl Fn(Pos2) -> (i32, i32),
    zoom: f32,
) {
    if response.clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            let (x, y) = to_display(p);
            let hit = studio.canvas.hit(x, y).map(|i| studio.canvas.nodes[i].id);
            if ui.input(|i| i.modifiers.command) {
                if let Some(id) = hit.filter(|id| *id != studio.screen) {
                    studio.toggle_in_selection(id);
                }
            } else {
                studio.select(hit.or(Some(studio.screen)));
            }
        }
    }
    if response.drag_started_by(PointerButton::Primary) {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
        let handle = handles.and_then(|h| h.iter().position(|r| r.expand(2.0).contains(origin)));
        let target = match handle {
            Some(_) => studio.selected,
            None => {
                let (x, y) = to_display(origin);
                let hit = studio.canvas.hit(x, y).map(|i| studio.canvas.nodes[i].id);
                if hit != studio.selected {
                    studio.select(hit);
                }
                hit
            }
        };
        let info = target.and_then(|id| studio.canvas.node(id).map(|(_, n)| n.clone()));
        let geometry = target.and_then(|id| studio.doc.project.widget(id)).map(|w| w.geometry);
        if let (Some(node), Some(geometry)) = (info, geometry) {
            if node.movable {
                studio.canvas.drag = Some(match handle {
                    Some(handle) => Drag::Resize { id: node.id, handle, start: node.rect, geometry },
                    None => Drag::Move { id: node.id, start: node.rect, geometry },
                });
            } else if node.parent.is_some() {
                studio.notify(tr("This widget is positioned by its parent (layout, tab or tile)."));
            }
        }
    }
    if let Some(drag) = studio.canvas.drag {
        if response.dragged_by(PointerButton::Primary) {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
            let current = response.interact_pointer_pos().unwrap_or(origin);
            let delta = (current - origin) / zoom;
            let (dx, dy) = (delta.x.round() as i32, delta.y.round() as i32);
            let grid = studio.settings.snap_to_grid.then_some(studio.settings.grid_size.max(1) as i32);
            let snap = |v: i32| grid.map_or(v, |g| (v as f32 / g as f32).round() as i32 * g);
            let (id, rect, geometry, resized) = match drag {
                Drag::Move { id, start, geometry } => {
                    let mut rect = lay::Rect { x: snap(start.x + dx), y: snap(start.y + dy), ..start };
                    let moving = studio.canvas.node(id).map(|(i, _)| i);
                    if let (true, Some(moving)) = (studio.settings.smart_guides, moving) {
                        let (snapped, gx, gy) = align::snap(rect, moving, &studio.canvas.nodes, zoom);
                        rect = snapped;
                        draw_guides(ui, &studio.canvas.nodes, gx, gy, &to_display, zoom);
                    }
                    (id, rect, geometry, false)
                }
                Drag::Resize { id, handle, start, geometry } => {
                    (id, resize(start, handle, dx, dy, &snap), geometry, true)
                }
            };
            let parent = studio.canvas.node(id).and_then(|(_, n)| n.parent).map(|p| studio.canvas.nodes[p].clone());
            if let (Some(parent), Some(widget)) = (parent, studio.doc.project.widget_mut(id)) {
                widget.geometry = geometry_for_rect(geometry, rect, &parent, resized);
            }
        } else {
            studio.canvas.drag = None;
        }
    }
}

fn resize(start: lay::Rect, handle: usize, dx: i32, dy: i32, snap: &impl Fn(i32) -> i32) -> lay::Rect {
    // Handles: 0 top-left, 1 top, 2 top-right, 3 right, 4 bottom-right, 5 bottom, 6 bottom-left, 7 left.
    let (mut left, mut top, mut right, mut bottom) = (start.x, start.y, start.right(), start.bottom());
    if matches!(handle, 0 | 6 | 7) {
        left = snap(left + dx).min(right - 1);
    }
    if matches!(handle, 2..=4) {
        right = snap(right + dx).max(left + 1);
    }
    if matches!(handle, 0..=2) {
        top = snap(top + dy).min(bottom - 1);
    }
    if matches!(handle, 4..=6) {
        bottom = snap(bottom + dy).max(top + 1);
    }
    lay::Rect::new(left, top, right - left, bottom - top)
}

fn handle_rects(r: Rect) -> [Rect; 8] {
    let points = [
        r.left_top(),
        r.center_top(),
        r.right_top(),
        r.right_center(),
        r.right_bottom(),
        r.center_bottom(),
        r.left_bottom(),
        r.left_center(),
    ];
    points.map(|p| Rect::from_center_size(p, vec2(HANDLE, HANDLE)))
}

fn handle_cursor(handle: usize) -> CursorIcon {
    match handle {
        0 | 4 => CursorIcon::ResizeNwSe,
        2 | 6 => CursorIcon::ResizeNeSw,
        1 | 5 => CursorIcon::ResizeVertical,
        _ => CursorIcon::ResizeHorizontal,
    }
}

fn drop_from_palette(
    ui: &egui::Ui,
    studio: &mut Studio,
    response: &egui::Response,
    to_display: &impl Fn(Pos2) -> (i32, i32),
    screen_rect: &impl Fn(lay::Rect) -> Rect,
    painter: &egui::Painter,
) {
    let Some(pos) = ui.ctx().pointer_interact_pos().or_else(|| response.hover_pos()) else { return };
    let target = |studio: &Studio, kind: WidgetKind| -> Option<usize> {
        let (x, y) = to_display(pos);
        let mut index = studio.canvas.hit(x, y).or(Some(0))?;
        loop {
            let node = &studio.canvas.nodes[index];
            if node.kind.def().accepts_child(kind) {
                return Some(index);
            }
            index = node.parent?;
        }
    };
    if let Some(kind) = response.dnd_hover_payload::<WidgetKind>() {
        if let Some(index) = target(studio, *kind) {
            let r = screen_rect(studio.canvas.nodes[index].children_area);
            painter.rect_stroke(r, 0, Stroke::new(2.0_f32, ACCENT), StrokeKind::Inside);
        }
    }
    let Some(kind) = response.dnd_release_payload::<WidgetKind>() else { return };
    let kind = *kind;
    let Some(index) = target(studio, kind) else {
        studio.notify(trf("{} cannot be placed here.", &[&tr(kind.name())]));
        return;
    };
    let parent = studio.canvas.nodes[index].clone();
    let (x, y) = to_display(pos);
    let mut widget = studio.doc.project.new_widget(kind);
    if !kind.def().managed_geometry {
        let size = |c: Coord, reference: i32, fallback: i32| match c.unit {
            Unit::Px => c.value,
            Unit::Percent => reference * c.value / 100,
            Unit::Content => fallback,
        };
        let w = size(widget.geometry.width, parent.children_area.w, 80);
        let h = size(widget.geometry.height, parent.children_area.h, 30);
        let rect = lay::Rect::new(x - w / 2, y - h / 2, w, h);
        widget.geometry = geometry_for_rect(widget.geometry, rect, &parent, false);
    }
    let id = widget.id;
    if studio.doc.project.insert(parent.id, usize::MAX, widget).is_ok() {
        studio.select(Some(id));
    }
}

fn handle_zoom_and_pan(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: Rect,
    view: &mut CanvasState,
    width: i32,
    height: i32,
) {
    let pointer = response.hover_pos();
    let (zoom_delta, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
    if let Some(p) = pointer {
        if zoom_delta != 1.0 {
            view.auto_fit = false;
            let old = view.zoom;
            view.zoom = (view.zoom * zoom_delta).clamp(0.1, 16.0);
            // Keep the point under the cursor fixed.
            let display_center = rect.center() + view.pan;
            let offset = p - display_center;
            view.pan += offset - offset * (view.zoom / old);
        } else if scroll != Vec2::ZERO {
            view.auto_fit = false;
            view.pan += scroll;
        }
    }
    if response.dragged_by(PointerButton::Middle)
        || (response.dragged_by(PointerButton::Primary) && ui.input(|i| i.key_down(egui::Key::Space)))
    {
        view.auto_fit = false;
        view.pan += response.drag_delta();
    }
    let limit = vec2(width as f32, height as f32) * view.zoom + rect.size();
    view.pan = view.pan.clamp(-limit / 2.0, limit / 2.0);
}

/// Paints the area around the display and returns the color used for
/// masking round displays.
fn draw_background(painter: &egui::Painter, rect: Rect, background: CanvasBackground, theme: Color32) -> Color32 {
    let (base, alternate) = match background {
        CanvasBackground::Theme => (theme, None),
        CanvasBackground::Dark => (Color32::from_rgb(0x0F, 0x11, 0x15), None),
        CanvasBackground::Light => (Color32::from_rgb(0xE4, 0xE7, 0xEC), None),
        CanvasBackground::Checkerboard => (Color32::from_gray(0x9A), Some(Color32::from_gray(0x7A))),
    };
    painter.rect_filled(rect, 0, base);
    if let Some(alternate) = alternate {
        const TILE: f32 = 12.0;
        let (columns, rows) = ((rect.width() / TILE).ceil() as i32, (rect.height() / TILE).ceil() as i32);
        for row in 0..rows {
            for column in (0..columns).filter(|c| (c + row) % 2 == 0) {
                let min = rect.min + vec2(column as f32, row as f32) * TILE;
                painter.rect_filled(Rect::from_min_size(min, vec2(TILE, TILE)), 0, alternate);
            }
        }
    }
    base
}

/// Optional outlines of all widgets and of hidden widgets.
fn draw_outlines(
    painter: &egui::Painter,
    nodes: &[NodeInfo],
    screen_rect: &impl Fn(lay::Rect) -> Rect,
    settings: &AppSettings,
) {
    for node in nodes.iter().skip(1) {
        let r = screen_rect(node.rect);
        if node.hidden {
            if settings.show_hidden {
                let stroke = Stroke::new(1.0_f32, visuals::WARNING.gamma_multiply(0.7));
                let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                painter.extend(egui::Shape::dashed_line(&corners, stroke, 4.0, 3.0));
            }
        } else if settings.show_bounds {
            painter.rect_stroke(r, 0, Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.35)), StrokeKind::Inside);
        }
    }
}

fn draw_grid(painter: &egui::Painter, display: Rect, zoom: f32, step: f32, color: Color32) {
    let step = step * zoom;
    if step < 4.0 {
        return;
    }
    let mut x = display.left();
    while x <= display.right() {
        painter.line_segment([pos2(x, display.top()), pos2(x, display.bottom())], Stroke::new(1.0_f32, color));
        x += step;
    }
    let mut y = display.top();
    while y <= display.bottom() {
        painter.line_segment([pos2(display.left(), y), pos2(display.right(), y)], Stroke::new(1.0_f32, color));
        y += step;
    }
}

/// Masks everything outside a round display and draws the bezel outline.
fn draw_display_frame(painter: &egui::Painter, display: Rect, shape: DisplayShape, background: Color32, clip: Rect) {
    let outline = Stroke::new(1.0_f32, Color32::from_gray(110));
    match shape {
        DisplayShape::Round => {
            let center = display.center();
            let inner = display.width().min(display.height()) / 2.0;
            let outer = (clip.size().length() + display.size().length()) * 2.0;
            painter.add(ring(center, inner, outer, background));
            painter.circle_stroke(center, inner, outline);
        }
        DisplayShape::Rectangle => {
            painter.rect_stroke(display, 0, outline, StrokeKind::Outside);
        }
    }
}

/// A filled annulus between two radii.
fn ring(center: Pos2, inner: f32, outer: f32, color: Color32) -> egui::Shape {
    const STEPS: u32 = 128;
    let mut mesh = egui::Mesh::default();
    for i in 0..STEPS {
        let angle = i as f32 / STEPS as f32 * std::f32::consts::TAU;
        let dir = vec2(angle.cos(), angle.sin());
        mesh.colored_vertex(center + dir * inner, color);
        mesh.colored_vertex(center + dir * outer, color);
    }
    for i in 0..STEPS {
        let (a, b) = (2 * i, 2 * ((i + 1) % STEPS));
        mesh.add_triangle(a, a + 1, b);
        mesh.add_triangle(a + 1, b + 1, b);
    }
    egui::Shape::mesh(mesh)
}

fn simulation_controls(ui: &mut egui::Ui, studio: &mut Studio) {
    let running = studio.simulation.is_some();
    let (text, color) =
        if running { (crate::icons::STOP, visuals::DANGER) } else { (crate::icons::PLAY, visuals::SUCCESS) };
    let label = if running { tr("Stop") } else { tr("Simulate") };
    let hint = if running {
        tr("Stop the simulation (F5 or Esc)")
    } else {
        tr("Run the UI: click buttons, switches and sliders, events are executed (F5)")
    };
    let button = egui::Button::new(egui::RichText::new(format!("{text} {label}")).color(color));
    let response = ui.add(button).on_hover_text(hint);
    studio.anchors.insert(crate::tour::Anchor::Simulate, response.rect);
    if response.clicked() {
        studio.toggle_simulation();
    }
    let Some(sim) = &mut studio.simulation else { return };
    ui.toggle_value(&mut sim.record, format!("\u{E3EE} {}", tr("Record")))
        .on_hover_text(tr("Shows the pointer and every click, for screen recordings"));
    if sim.has_live_values() {
        egui::ComboBox::from_id_salt("sim_live")
            .selected_text(format!("{}: {}", tr("Live data"), tr(sim.live_mode.label())))
            .show_ui(ui, |ui| {
                for mode in crate::simulator::LiveMode::ALL {
                    ui.selectable_value(&mut sim.live_mode, mode, tr(mode.label()));
                }
            })
            .response
            .on_hover_text(tr("Moves arcs, bars, sliders, meters and numeric labels; Maximum shows the widest values"));
    }
    if ui
        .button(crate::icons::ARROWS_CLOCKWISE)
        .on_hover_text(tr("Restart the simulation with the current project"))
        .clicked()
    {
        let screen = sim.screen;
        let record = sim.record;
        let mut fresh = crate::simulator::Simulation::new(&studio.doc.project, screen);
        fresh.record = record;
        studio.simulation = Some(fresh);
    }
    let now = ui.input(|i| i.time);
    match &studio.recorder {
        Some(recorder) => {
            let seconds = recorder.seconds(now) as u32;
            let text = egui::RichText::new(format!("\u{E46C} GIF {}:{:02}", seconds / 60, seconds % 60))
                .color(visuals::DANGER);
            if ui.button(text).on_hover_text(tr("Stop and save the recording")).clicked() {
                studio.stop_recording();
            }
        }
        None => {
            let hint =
                trf("Records the display as an animated GIF (up to {} s)", &[&(crate::recorder::MAX_SECONDS as u32)]);
            if ui.button("\u{E3EE} GIF").on_hover_text(hint).clicked() {
                studio.start_recording(now);
            }
        }
    }
    if let Some(last) = studio.simulation.as_ref().and_then(|s| s.log.last()) {
        crate::widgets::muted(ui, last.clone());
    }
}

fn node_infos(layout: &Layout<'_>) -> Vec<NodeInfo> {
    let mut infos: Vec<NodeInfo> = layout
        .nodes
        .iter()
        .map(|n| {
            let parent_flex = n.parent.is_some_and(|p| layout.nodes[p].widget.layout.is_some())
                && !n.widget.flag(lvglab_model::flags::ObjFlag::IgnoreLayout)
                && !n.widget.flag(lvglab_model::flags::ObjFlag::Floating);
            NodeInfo {
                id: n.widget.id,
                kind: n.widget.kind,
                rect: n.rect,
                clip: n.clip,
                children_area: n.children_area,
                content: n.content,
                parent: n.parent,
                hidden: n.hidden,
                movable: n.parent.is_some() && !n.widget.def().managed_geometry && !parent_flex,
            }
        })
        .collect();
    // Objects inside a component instance stand for the instance itself.
    for index in 0..infos.len() {
        if infos[index].id.0 >= lvglab_model::project::EXPANDED_ID_BASE {
            let owner = infos[index].parent.map(|p| infos[p].id);
            if let Some(owner) = owner {
                infos[index].id = owner;
                infos[index].movable = false;
            }
        }
    }
    infos
}

fn toolbar(ui: &mut egui::Ui, studio: &mut Studio) {
    ui.horizontal_wrapped(|ui| {
        let shown = studio.simulation.as_ref().map_or(studio.screen, |s| s.screen);
        let current = studio.doc.project.widget(shown).map(|s| s.name.clone()).unwrap_or_default();
        let screens: Vec<(WidgetId, String)> =
            studio.doc.project.screens.iter().map(|s| (s.id, s.name.clone())).collect();
        egui::ComboBox::from_id_salt("canvas_screen")
            .selected_text(format!("\u{E32E} {current}"))
            .width(150.0)
            .show_ui(ui, |ui| {
                for (id, name) in screens {
                    if ui.selectable_label(shown == id, name).clicked() {
                        match &mut studio.simulation {
                            Some(sim) => sim.show_screen(id, ui.input(|i| i.time)),
                            None => studio.select_screen(id),
                        }
                    }
                }
            });
        if studio.simulation.is_none() && ui.button("+").on_hover_text(tr("Add screen")).clicked() {
            studio.push(crate::commands::Command::AddScreen);
        }
        if studio.simulation.is_none() {
            ui.toggle_value(&mut studio.canvas.overview, tr("Overview"))
                .on_hover_text(tr("All screens at a glance; click one to open it"));
        }
        ui.separator();
        simulation_controls(ui, studio);
        ui.separator();
        let view = &mut studio.canvas;
        if ui.button("-").clicked() {
            view.zoom = (view.zoom / 1.25).max(0.1);
            view.auto_fit = false;
        }
        if ui.button(format!("{:.0}%", view.zoom * 100.0)).on_hover_text("100%").clicked() {
            view.zoom = 1.0;
            view.pan = Vec2::ZERO;
            view.auto_fit = false;
        }
        if ui.button("+").clicked() {
            view.zoom = (view.zoom * 1.25).min(16.0);
            view.auto_fit = false;
        }
        if ui.button(tr("Fit")).on_hover_text(tr("Fit display into view")).clicked() {
            view.fit_pending = true;
            view.auto_fit = true;
        }
        ui.separator();
        let s = &mut studio.settings;
        ui.toggle_value(&mut s.show_grid, tr("Grid"));
        ui.toggle_value(&mut s.snap_to_grid, tr("Snap")).on_hover_text(tr("Snap to grid while moving"));
        ui.toggle_value(&mut s.smart_guides, tr("Guides"))
            .on_hover_text(tr("Snap to the edges and centers of neighbors while moving"));
        ui.toggle_value(&mut s.show_hidden, tr("Hidden")).on_hover_text(tr("Outline hidden widgets in orange"));
        if studio.simulation.is_none()
            && ui
                .button(crate::icons::DICE_FIVE)
                .on_hover_text(tr("Random values for all widgets of this screen (Shift: all screens)"))
                .clicked()
        {
            let all = ui.input(|i| i.modifiers.shift);
            studio.push(crate::commands::Command::Randomize { all });
        }
        if studio.simulation.is_none() && studio.selected.is_some_and(|id| id != studio.screen) {
            ui.separator();
            align::toolbar(ui, studio);
        }
    });
}

/// Draws the snap lines of a move across the whole display.
fn draw_guides(
    ui: &egui::Ui,
    nodes: &[NodeInfo],
    gx: Option<i32>,
    gy: Option<i32>,
    to_display: &impl Fn(Pos2) -> (i32, i32),
    zoom: f32,
) {
    let Some(screen) = nodes.first() else { return };
    // Screen position of the display origin.
    let (ox, oy) = to_display(Pos2::ZERO);
    let origin = pos2(-ox as f32 * zoom, -oy as f32 * zoom);
    let to_screen = |x: i32, y: i32| origin + vec2(x as f32, y as f32) * zoom;
    let r = screen.rect;
    let stroke = Stroke::new(1.0_f32, align::GUIDE);
    let painter = ui.painter();
    if let Some(x) = gx {
        painter.line_segment([to_screen(x, r.y), to_screen(x, r.bottom())], stroke);
    }
    if let Some(y) = gy {
        painter.line_segment([to_screen(r.x, y), to_screen(r.right(), y)], stroke);
    }
}
