//! Computes widget rectangles the way LVGL 8 positions objects: sizes in
//! px / % / content, alignment relative to the parent's content area and
//! flex layouts. Used by the editor preview and for hit testing.

use crate::coord::{Align, Coord, Unit, clamp_px};
use crate::flags::{FlexAlign, ObjFlag, StateMask};
use crate::style::{StyleProp, StyleValue};
use crate::theme::StyleResolver;
use crate::widget::{FlexLayout, Widget, WidgetId, WidgetKind};

/// Integer rectangle in display pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn right(self) -> i32 {
        self.x + self.w
    }

    pub const fn bottom(self) -> i32 {
        self.y + self.h
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    pub fn inset(self, left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self::new(self.x + left, self.y + top, (self.w - left - right).max(0), (self.h - top - bottom).max(0))
    }

    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = (self.right().min(other.right()) - x).max(0);
        let h = (self.bottom().min(other.bottom()) - y).max(0);
        Self::new(x, y, w, h)
    }

    pub const fn tuple(self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.w, self.h)
    }
}

/// Measures text and images for content sized widgets.
pub trait Measure {
    /// Size of `text` rendered with `font`; wraps at `max_width` if given.
    fn text_size(
        &self,
        text: &str,
        font: &str,
        letter_space: i32,
        line_space: i32,
        max_width: Option<i32>,
    ) -> (i32, i32);
    fn line_height(&self, font: &str) -> i32;
    fn image_size(&self, image: &str) -> Option<(i32, i32)>;
}

/// One placed widget.
#[derive(Clone, Debug)]
pub struct Node<'a> {
    pub widget: &'a Widget,
    pub rect: Rect,
    /// Area for children (inside border and padding).
    pub content: Rect,
    /// Area children are aligned in (differs from `content` for windows and tab views).
    pub children_area: Rect,
    /// Visible area (intersection of all ancestors unless overflow is visible).
    pub clip: Rect,
    pub parent: Option<usize>,
    pub depth: usize,
    /// Hidden itself or below a hidden ancestor.
    pub hidden: bool,
    /// Alignment in effect: the style align of the displayed state overrides the geometry's.
    pub align: Align,
}

/// The layout of one screen, in paint order.
pub struct Layout<'a> {
    pub nodes: Vec<Node<'a>>,
}

impl<'a> Layout<'a> {
    /// Lays out `screen` on a display of `width` × `height`. Inside tab and
    /// tile views only the page containing `focus` (or the first) is shown.
    pub fn compute(
        screen: &'a Widget,
        width: i32,
        height: i32,
        resolver: &StyleResolver<'_>,
        measure: &dyn Measure,
        focus: Option<WidgetId>,
    ) -> Self {
        let mut engine = Engine { resolver, measure, focus, nodes: Vec::new() };
        let rect = Rect::new(0, 0, width, height);
        engine.place(screen, &mut vec![screen], rect, rect, None, 0, false);
        Self { nodes: engine.nodes }
    }

    pub fn node(&self, id: WidgetId) -> Option<&Node<'a>> {
        self.nodes.iter().find(|n| n.widget.id == id)
    }

    /// Ancestors of a node from the screen down to the node's widget.
    pub fn path(&self, index: usize) -> Vec<&'a Widget> {
        let mut path = Vec::new();
        let mut current = Some(index);
        while let Some(i) = current {
            path.push(self.nodes[i].widget);
            current = self.nodes[i].parent;
        }
        path.reverse();
        path
    }

    /// Topmost visible widget at a display position.
    pub fn hit_test(&self, x: i32, y: i32) -> Option<WidgetId> {
        self.nodes
            .iter()
            .rev()
            .find(|n| !n.hidden && n.rect.contains(x, y) && n.clip.contains(x, y))
            .map(|n| n.widget.id)
    }
}

struct Engine<'r, 'a> {
    resolver: &'r StyleResolver<'r>,
    measure: &'r dyn Measure,
    focus: Option<WidgetId>,
    nodes: Vec<Node<'a>>,
}

impl<'a> Engine<'_, 'a> {
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        widget: &'a Widget,
        path: &mut Vec<&'a Widget>,
        rect: Rect,
        parent_clip: Rect,
        parent: Option<usize>,
        depth: usize,
        parent_hidden: bool,
    ) {
        let rect = self.translate(path, rect);
        let hidden = parent_hidden || widget.is_hidden();
        let content = self.content_rect(path, rect);
        let clip = if widget.flag(ObjFlag::OverflowVisible) { parent_clip } else { parent_clip.intersect(rect) };
        let area = Self::children_area(widget, rect, content);
        let index = self.nodes.len();
        let align = self.align(path);
        self.nodes.push(Node {
            widget,
            rect,
            content,
            children_area: area,
            clip: parent_clip,
            parent,
            depth,
            hidden,
            align,
        });

        let visible: Vec<&'a Widget> = self.visible_children(widget);
        let rects = match widget.layout {
            Some(flex) => self.flex(&visible, path, area, rect, flex),
            None => visible.iter().map(|c| self.child_rect(c, path, area, rect)).collect(),
        };
        for (child, child_rect) in visible.into_iter().zip(rects) {
            path.push(child);
            self.place(child, path, child_rect, clip, Some(index), depth + 1, hidden);
            path.pop();
        }
    }

    /// Tab views and tile views only show one page.
    fn visible_children(&self, widget: &'a Widget) -> Vec<&'a Widget> {
        if !matches!(widget.kind, WidgetKind::Tabview | WidgetKind::Tileview) || widget.children.is_empty() {
            return widget.children.iter().collect();
        }
        let active = self
            .focus
            .and_then(|f| widget.children.iter().find(|c| c.find(f).is_some()))
            .or_else(|| widget.children.get(widget.prop_int("active_tab").max(0) as usize))
            .unwrap_or(&widget.children[0]);
        vec![active]
    }

    /// Alignment of the last widget of `path`. `lv_obj_set_style_align` is
    /// emitted after `lv_obj_set_align`, so a style align wins.
    fn align(&self, path: &[&Widget]) -> Align {
        let Some(widget) = path.last() else { return Align::TopLeft };
        match self.resolver.get(path, "main", widget.state_mask(), StyleProp::Align) {
            Some(StyleValue::Enum(value)) => Align::from_suffix(&value).unwrap_or(widget.geometry.align),
            _ => widget.geometry.align,
        }
    }

    /// Alignment of `child`, a child of the last widget of `path`.
    fn child_align(&self, child: &Widget, path: &[&Widget]) -> Align {
        let mut path = path.to_vec();
        path.push(child);
        self.align(&path)
    }

    fn translate(&self, path: &[&Widget], rect: Rect) -> Rect {
        let state = path.last().map_or(StateMask::DEFAULT, |w| w.state_mask());
        let dx = self.resolver.coord(path, "main", state, StyleProp::TranslateX).map_or(0, |c| c.resolve(rect.w, 0));
        let dy = self.resolver.coord(path, "main", state, StyleProp::TranslateY).map_or(0, |c| c.resolve(rect.h, 0));
        Rect { x: rect.x + dx, y: rect.y + dy, ..rect }
    }

    fn content_rect(&self, path: &[&Widget], rect: Rect) -> Rect {
        let state = path.last().map_or(StateMask::DEFAULT, |w| w.state_mask());
        let border = self.resolver.int(path, "main", state, StyleProp::BorderWidth);
        let (l, t, r, b) = self.resolver.padding(path, "main", state);
        rect.inset(l + border, t + border, r + border, b + border)
    }

    /// The area children are positioned in, accounting for sub-objects such
    /// as a window header or the tab bar.
    fn children_area(widget: &Widget, rect: Rect, content: Rect) -> Rect {
        match widget.kind {
            WidgetKind::Win => {
                let header = clamp_px(widget.prop_int("header_height"));
                Rect::new(rect.x, rect.y + header, rect.w, (rect.h - header).max(0))
            }
            WidgetKind::Tabview => {
                let size = clamp_px(widget.prop_int("tab_size"));
                match widget.prop_text("tab_pos").as_str() {
                    "LV_DIR_BOTTOM" => Rect::new(rect.x, rect.y, rect.w, rect.h - size),
                    "LV_DIR_LEFT" => Rect::new(rect.x + size, rect.y, rect.w - size, rect.h),
                    "LV_DIR_RIGHT" => Rect::new(rect.x, rect.y, rect.w - size, rect.h),
                    _ => Rect::new(rect.x, rect.y + size, rect.w, rect.h - size),
                }
            }
            WidgetKind::Tileview => rect,
            _ => content,
        }
    }

    fn child_size(&self, child: &Widget, path: &[&Widget], area: Rect) -> (i32, i32) {
        if child.def().managed_geometry {
            return (area.w, area.h);
        }
        let g = child.geometry;
        let fixed_w = (g.width.unit != Unit::Content).then(|| g.width.resolve(area.w, 0));
        let w = fixed_w.unwrap_or_else(|| self.content_size(child, path, area, None).0);
        let h = match g.height.unit {
            Unit::Content => self.content_size(child, path, area, Some(w)).1,
            _ => g.height.resolve(area.h, 0),
        };
        (w.max(0), h.max(0))
    }

    fn child_rect(&self, child: &Widget, path: &[&Widget], area: Rect, parent_rect: Rect) -> Rect {
        let (w, h) = self.child_size(child, path, area);
        if child.def().managed_geometry {
            return Rect::new(area.x, area.y, w, h);
        }
        let g = child.geometry;
        let (ax, ay) = self.child_align(child, path).anchor(area.tuple(), parent_rect.tuple(), (w, h));
        let dx = g.x.resolve(area.w, 0);
        let dy = g.y.resolve(area.h, 0);
        Rect::new(ax + dx, ay + dy, w, h)
    }

    /// Size of a content sized widget. `fixed_width` is known when only the
    /// height is content sized (wrapping text).
    fn content_size(
        &self,
        widget: &Widget,
        parent_path: &[&Widget],
        area: Rect,
        fixed_width: Option<i32>,
    ) -> (i32, i32) {
        let mut path: Vec<&Widget> = parent_path.to_vec();
        path.push(widget);
        let state = widget.state_mask();
        let r = self.resolver;
        let (pl, pt, pr, pb) = r.padding(&path, "main", state);
        let border = r.int(&path, "main", state, StyleProp::BorderWidth);
        let (ph, pv) = (pl + pr + 2 * border, pt + pb + 2 * border);
        let font = r.font(&path, "main", state);
        let letter = r.int(&path, "main", state, StyleProp::TextLetterSpace);
        let line_space = r.int(&path, "main", state, StyleProp::TextLineSpace);
        let line_h = clamp_px(self.measure.line_height(&font));
        let text = |text: &str| {
            let max = fixed_width.map(|w| (w - ph).max(1));
            let (w, h) = self.measure.text_size(text, &font, letter, line_space, max);
            (clamp_px(w), clamp_px(h))
        };
        let (cw, ch) = match widget.kind {
            WidgetKind::Label => text(&widget.prop_text("text")),
            WidgetKind::Spangroup => text(&widget.prop_text("spans").replace('\n', "")),
            WidgetKind::Checkbox => {
                let (tw, th) = text(&widget.prop_text("text"));
                let (il, it, ir, ib) = r.padding(&path, "indicator", state);
                let indicator = line_h + il.max(it) + ir.max(ib);
                let gap = r.int(&path, "main", state, StyleProp::PadColumn);
                (indicator + gap + tw, th.max(indicator))
            }
            WidgetKind::Textarea => {
                // The text and the placeholder are labels inside; the taller one counts, at least one line.
                let one_line = widget.prop_bool("one_line");
                let measure = |s: String| {
                    if one_line { (text(&s.replace('\n', " ")).0, line_h) } else { text(&s) }
                };
                let (tw, th) = measure(widget.prop_text("text"));
                let (pw, ph2) = measure(widget.prop_text("placeholder"));
                (tw.max(pw), th.max(ph2).max(line_h))
            }
            WidgetKind::Dropdown | WidgetKind::Spinbox => {
                let (tw, _) = text(widget.prop_text("options").lines().next().unwrap_or("0"));
                (tw + line_h, line_h)
            }
            WidgetKind::Roller => {
                let options = widget.prop_text("options");
                let width = options.lines().map(|l| text(l).0).max().unwrap_or(0);
                let rows = i64::from(widget.prop_int("visible_rows").clamp(1, 1000));
                (width, clamp_px(rows * i64::from(line_h + line_space) - i64::from(line_space)))
            }
            WidgetKind::Image => {
                let (w, h) = self.measure.image_size(&widget.prop_text("src")).unwrap_or((50, 50));
                (clamp_px(w), clamp_px(h))
            }
            WidgetKind::AnimImg => {
                let first = widget.prop_text("images").lines().next().unwrap_or_default().trim().to_owned();
                self.measure.image_size(&first).unwrap_or((50, 50))
            }
            WidgetKind::Line => {
                let points = parse_points(&widget.prop_text("points"));
                let line_w = r.int(&path, "main", state, StyleProp::LineWidth).max(1);
                let max_x = points.iter().map(|p| p.0).max().unwrap_or(0);
                let max_y = points.iter().map(|p| p.1).max().unwrap_or(0);
                (max_x + line_w, max_y + line_w)
            }
            WidgetKind::Table => {
                let cells = widget.prop_text("cells");
                let rows = cells.lines().count().max(1) as i32;
                let cols = cells.lines().map(|l| l.split('|').count()).max().unwrap_or(1) as i32;
                let col_w = match widget.prop_int("col_width") {
                    0 => 130,
                    w => clamp_px(w),
                };
                let (_, ipt, _, ipb) = r.padding(&path, "items", state);
                (
                    clamp_px(i64::from(cols) * i64::from(col_w)),
                    clamp_px(i64::from(rows) * i64::from(line_h + ipt + ipb)),
                )
            }
            WidgetKind::Msgbox if r.theme.lvgl9 => {
                // Header and footer are LV_DPI_DEF / 3 high; the content has small padding.
                let bar = 130 / 3;
                let header = !widget.prop_text("title").is_empty() || widget.prop_bool("close_btn");
                let footer = !widget.prop_text("buttons").trim().is_empty();
                let (tw, th) = text(&widget.prop_text("text"));
                let bars = bar * (i32::from(header) + i32::from(footer));
                (tw.max(200), th + 2 * r.theme.pad_small + bars)
            }
            WidgetKind::Msgbox => {
                let (tw, th) = text(&widget.prop_text("text"));
                (tw.max(200), line_h * 3 + th)
            }
            _ => self.children_extent(widget, &path, area),
        };
        let w = fixed_width.unwrap_or(cw + ph);
        (w, ch + pv)
    }

    /// Bounding box of the children of a content sized container.
    fn children_extent(&self, widget: &Widget, path: &[&Widget], area: Rect) -> (i32, i32) {
        if let Some(flex) = widget.layout {
            return self.flex_extent(widget, flex, path, area);
        }
        let mut extent = (0, 0);
        for child in widget.children.iter().filter(|c| !c.is_hidden()) {
            let g = child.geometry;
            let (w, h) = self.child_size(child, path, area);
            let align = self.child_align(child, path);
            let centered_x = matches!(align, Align::Center | Align::TopMid | Align::BottomMid);
            let centered_y = matches!(align, Align::Center | Align::LeftMid | Align::RightMid);
            let x = if g.x.unit == Unit::Px { g.x.value.abs() } else { 0 };
            let y = if g.y.unit == Unit::Px { g.y.value.abs() } else { 0 };
            extent.0 = extent.0.max(if centered_x { w + 2 * x } else { w + x });
            extent.1 = extent.1.max(if centered_y { h + 2 * y } else { h + y });
        }
        extent
    }

    /// Content size of a flex container: items laid out in one track along
    /// the main axis, separated by the row/column gap.
    fn flex_extent(&self, widget: &Widget, flex: FlexLayout, path: &[&Widget], area: Rect) -> (i32, i32) {
        let row = flex.flow.is_row();
        let gap_prop = if row { StyleProp::PadColumn } else { StyleProp::PadRow };
        let gap = self.resolver.int(path, "main", widget.state_mask(), gap_prop);
        let sizes: Vec<(i32, i32)> = widget
            .children
            .iter()
            .filter(|c| !c.is_hidden() && !c.flag(ObjFlag::IgnoreLayout) && !c.flag(ObjFlag::Floating))
            .map(|c| self.child_size(c, path, area))
            .collect();
        let count = sizes.len() as i32;
        let main = |(w, h): (i32, i32)| if row { w } else { h };
        let cross = |(w, h): (i32, i32)| if row { h } else { w };
        let main_total = sizes.iter().copied().map(main).sum::<i32>() + gap * (count - 1).max(0);
        let cross_max = sizes.iter().copied().map(cross).max().unwrap_or(0);
        if row { (main_total, cross_max) } else { (cross_max, main_total) }
    }

    fn flex(
        &self,
        children: &[&Widget],
        path: &[&Widget],
        area: Rect,
        parent_rect: Rect,
        flex: FlexLayout,
    ) -> Vec<Rect> {
        let parent = path.last().copied();
        let state = parent.map_or(StateMask::DEFAULT, |p| p.state_mask());
        let row = flex.flow.is_row();
        let gap_main =
            self.resolver.int(path, "main", state, if row { StyleProp::PadColumn } else { StyleProp::PadRow });
        let gap_cross =
            self.resolver.int(path, "main", state, if row { StyleProp::PadRow } else { StyleProp::PadColumn });
        let main_len = if row { area.w } else { area.h };
        let cross_len = if row { area.h } else { area.w };

        let mut rects = vec![Rect::default(); children.len()];
        let in_flow: Vec<usize> = (0..children.len())
            .filter(|&i| !children[i].flag(ObjFlag::IgnoreLayout) && !children[i].flag(ObjFlag::Floating))
            .collect();
        for (i, child) in children.iter().enumerate() {
            if !in_flow.contains(&i) {
                rects[i] = self.child_rect(child, path, area, parent_rect);
            }
        }
        let sizes: Vec<(i32, i32)> = in_flow
            .iter()
            .map(|&i| {
                let (w, h) = self.child_size(children[i], path, area);
                if row { (w, h) } else { (h, w) }
            })
            .collect();

        // Break into tracks.
        let mut tracks: Vec<Vec<usize>> = vec![Vec::new()];
        let mut used = 0;
        for (k, (main, _)) in sizes.iter().enumerate() {
            let new_track = children[in_flow[k]].flag(ObjFlag::FlexInNewTrack);
            let track = tracks.last_mut().expect("never empty");
            let needed = if track.is_empty() { *main } else { used + gap_main + main };
            if !track.is_empty() && ((flex.flow.wraps() && needed > main_len) || new_track) {
                tracks.push(vec![k]);
                used = *main;
            } else {
                track.push(k);
                used = needed;
            }
        }
        let track_cross: Vec<i32> = tracks.iter().map(|t| t.iter().map(|&k| sizes[k].1).max().unwrap_or(0)).collect();
        let total_cross = track_cross.iter().sum::<i32>() + gap_cross * (tracks.len() as i32 - 1).max(0);
        let (mut cross_pos, cross_gap) = distribute(flex.track_place, cross_len - total_cross, tracks.len(), gap_cross);
        if !flex.flow.wraps() {
            cross_pos = 0;
        }

        for (t, track) in tracks.iter().enumerate() {
            let mut order = track.clone();
            if flex.flow.is_reversed() {
                order.reverse();
            }
            let total_main =
                order.iter().map(|&k| sizes[k].0).sum::<i32>() + gap_main * (order.len() as i32 - 1).max(0);
            let (mut main_pos, main_gap) = distribute(flex.main_place, main_len - total_main, order.len(), gap_main);
            let line_cross = if flex.flow.wraps() { track_cross[t] } else { cross_len };
            for &k in &order {
                let (main, cross) = sizes[k];
                let offset = match flex.cross_place {
                    FlexAlign::End => line_cross - cross,
                    FlexAlign::Center => (line_cross - cross) / 2,
                    _ => 0,
                };
                let (x, y, w, h) = if row {
                    (area.x + main_pos, area.y + cross_pos + offset, main, cross)
                } else {
                    (area.x + cross_pos + offset, area.y + main_pos, cross, main)
                };
                // LVGL ignores x/y of flex items; the layout sets the position.
                rects[in_flow[k]] = Rect::new(x, y, w, h);
                main_pos += main + main_gap;
            }
            cross_pos += track_cross[t] + cross_gap;
        }
        rects
    }
}

/// Start offset and gap for distributing `free` space among `count` items.
fn distribute(place: FlexAlign, free: i32, count: usize, gap: i32) -> (i32, i32) {
    let n = count.max(1) as i32;
    match place {
        FlexAlign::Start => (0, gap),
        FlexAlign::End => (free, gap),
        FlexAlign::Center => (free / 2, gap),
        FlexAlign::SpaceEvenly => {
            let space = (free + gap * (n - 1)) / (n + 1);
            (space, space)
        }
        FlexAlign::SpaceAround => {
            let space = (free + gap * (n - 1)) / n;
            (space / 2, space)
        }
        FlexAlign::SpaceBetween if n > 1 => (0, (free + gap * (n - 1)) / (n - 1)),
        FlexAlign::SpaceBetween => (0, gap),
    }
}

/// Parses `x,y x,y ...` (also accepts `;` as separator).
pub fn parse_points(text: &str) -> Vec<(i32, i32)> {
    text.split([' ', ';', '\n'])
        .filter_map(|pair| {
            let (x, y) = pair.trim().split_once(',')?;
            let (x, y): (i32, i32) = (x.trim().parse().ok()?, y.trim().parse().ok()?);
            Some((clamp_px(x), clamp_px(y)))
        })
        .collect()
}

/// Resolves a size of a widget that is not content sized, for templates
/// that need pixel values (e.g. a canvas buffer).
pub fn pixel_size(coord: Coord, fallback: i32) -> i32 {
    match coord.unit {
        Unit::Px => clamp_px(coord.value),
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::FlexFlow;
    use crate::project::{DisplaySettings, ThemeSettings};
    use crate::theme::Theme;

    struct FixedMeasure;

    impl Measure for FixedMeasure {
        fn text_size(&self, text: &str, _: &str, _: i32, _: i32, _: Option<i32>) -> (i32, i32) {
            let lines = text.lines().count().max(1) as i32;
            (text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as i32 * 10, lines * 16)
        }
        fn line_height(&self, _: &str) -> i32 {
            16
        }
        fn image_size(&self, _: &str) -> Option<(i32, i32)> {
            None
        }
    }

    fn container(id: u64) -> Widget {
        let mut w = Widget::new(WidgetKind::Container, WidgetId(id), format!("C{id}"));
        w.geometry.align = Align::TopLeft;
        w
    }

    fn layout_of(screen: &Widget) -> Vec<Rect> {
        let theme = Theme::new(&ThemeSettings::default(), &DisplaySettings::default(), Default::default());
        let resolver = StyleResolver::new(&theme);
        Layout::compute(screen, 320, 240, &resolver, &FixedMeasure, None).nodes.iter().map(|n| n.rect).collect()
    }

    #[test]
    fn centered_and_percent_children() {
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
        let mut child = container(2);
        child.geometry.align = Align::Center;
        child.geometry.width = Coord::pct(50);
        child.geometry.height = Coord::px(40);
        child.geometry.y = Coord::px(10);
        screen.children.push(child);
        let rects = layout_of(&screen);
        assert_eq!(rects[0], Rect::new(0, 0, 320, 240));
        assert_eq!(rects[1], Rect::new(80, 110, 160, 40));
    }

    #[test]
    fn style_align_overrides_the_geometry() {
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
        let mut child = container(2);
        child.geometry.width = Coord::px(100);
        child.geometry.height = Coord::px(40);
        child.styles.set("main", StateMask::DEFAULT, StyleProp::Align, StyleValue::Enum("LV_ALIGN_CENTER".into()));
        screen.children.push(child);
        assert_eq!(layout_of(&screen)[1], Rect::new(110, 100, 100, 40));
    }

    #[test]
    fn content_sized_label() {
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
        let mut label = Widget::new(WidgetKind::Label, WidgetId(2), "L");
        label.geometry.align = Align::TopLeft;
        label.set_prop("text", crate::prop::PropValue::Text("abcd".into()));
        screen.children.push(label);
        assert_eq!(layout_of(&screen)[1], Rect::new(0, 0, 40, 16));
    }

    #[test]
    fn content_sized_text_area_has_the_height_of_its_text() {
        let build = |text: &str, one_line: bool| {
            let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
            let mut area = Widget::new(WidgetKind::Textarea, WidgetId(2), "T");
            area.geometry.align = Align::TopLeft;
            area.geometry.width = Coord::px(200);
            area.geometry.height = Coord::content();
            area.set_prop("text", crate::prop::PropValue::Text(text.into()));
            area.set_prop("one_line", crate::prop::PropValue::Bool(one_line));
            screen.children.push(area);
            layout_of(&screen)[1]
        };
        let pad = build("", true).h - 16;
        assert!(pad > 0, "padding and border are added to one line of text");
        assert_eq!(build("one", false).h, build("", false).h, "an empty text area is one line high");
        assert_eq!(
            build(
                "a
b
c", false
            )
            .h,
            pad + 3 * 16,
            "every line adds its height"
        );
        assert_eq!(
            build(
                "a
b
c", true
            )
            .h,
            pad + 16,
            "one line mode ignores line breaks"
        );
    }

    #[test]
    fn flex_row_with_gap_and_center() {
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
        let mut row = container(2);
        row.geometry.width = Coord::px(100);
        row.geometry.height = Coord::px(20);
        row.layout = Some(FlexLayout { flow: FlexFlow::Row, main_place: FlexAlign::Center, ..FlexLayout::default() });
        row.styles.set("main", StateMask::DEFAULT, StyleProp::PadColumn, crate::style::StyleValue::Int(10));
        for id in 3..5 {
            let mut c = container(id);
            c.geometry.width = Coord::px(20);
            c.geometry.height = Coord::px(20);
            row.children.push(c);
        }
        row.children[1].geometry.y = Coord::px(3);
        screen.children.push(row);
        let rects = layout_of(&screen);
        assert_eq!(rects[2], Rect::new(25, 0, 20, 20));
        assert_eq!(rects[3], Rect::new(55, 0, 20, 20), "x/y are ignored inside flex");
    }

    #[test]
    fn content_sized_flex_container_sums_children() {
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
        let mut row = container(2);
        row.geometry.width = Coord::content();
        row.geometry.height = Coord::content();
        row.layout = Some(FlexLayout::default());
        row.styles.set("main", StateMask::DEFAULT, StyleProp::PadColumn, crate::style::StyleValue::Int(10));
        for (id, w) in [(3, 20), (4, 30)] {
            let mut c = container(id);
            c.geometry.width = Coord::px(w);
            c.geometry.height = Coord::px(15);
            row.children.push(c);
        }
        screen.children.push(row);
        assert_eq!(layout_of(&screen)[1], Rect::new(0, 0, 60, 15));
    }

    #[test]
    fn hit_test_returns_topmost() {
        let mut screen = Widget::new(WidgetKind::Screen, WidgetId(1), "S");
        let mut a = container(2);
        a.geometry.width = Coord::px(50);
        let b = a.clone();
        screen.children.push(a);
        screen.children.push(Widget { id: WidgetId(3), ..b });
        let theme = Theme::new(&ThemeSettings::default(), &DisplaySettings::default(), Default::default());
        let resolver = StyleResolver::new(&theme);
        let layout = Layout::compute(&screen, 320, 240, &resolver, &FixedMeasure, None);
        assert_eq!(layout.hit_test(10, 10), Some(WidgetId(3)));
        assert_eq!(layout.hit_test(200, 200), Some(WidgetId(1)));
        assert_eq!(layout.path(2).len(), 2);
    }

    #[test]
    fn points_parse() {
        assert_eq!(parse_points("0,0 10,5;3,4"), vec![(0, 0), (10, 5), (3, 4)]);
    }
}
