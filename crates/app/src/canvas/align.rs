//! Aligning and distributing the selection, and snap lines while moving.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use lvglab_model::layout as lay;

use super::{NodeInfo, geometry_for_rect};
use crate::app::Studio;
use crate::i18n::tr;

/// Color of snap lines.
pub const GUIDE: Color32 = Color32::from_rgb(0xFF, 0x3D, 0x9A);
/// Snap distance in screen pixels.
const SNAP_PX: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignOp {
    Left,
    HCenter,
    Right,
    Top,
    VCenter,
    Bottom,
    DistributeH,
    DistributeV,
}

impl AlignOp {
    pub const ALL: [Self; 8] = [
        Self::Left,
        Self::HCenter,
        Self::Right,
        Self::Top,
        Self::VCenter,
        Self::Bottom,
        Self::DistributeH,
        Self::DistributeV,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Left => "Align left",
            Self::HCenter => "Center horizontally",
            Self::Right => "Align right",
            Self::Top => "Align top",
            Self::VCenter => "Center vertically",
            Self::Bottom => "Align bottom",
            Self::DistributeH => "Distribute horizontally",
            Self::DistributeV => "Distribute vertically",
        }
    }

    fn is_distribute(self) -> bool {
        matches!(self, Self::DistributeH | Self::DistributeV)
    }
}

/// New rectangles for `rects`. With one rectangle it is aligned inside
/// `area` (its parent); with several they are aligned to their common
/// bounding box. Distributing needs three or more and keeps the outer ones.
pub fn arrange(op: AlignOp, rects: &[lay::Rect], area: lay::Rect) -> Vec<lay::Rect> {
    let bounds = if rects.len() == 1 {
        area
    } else {
        let left = rects.iter().map(|r| r.x).min().unwrap_or(0);
        let top = rects.iter().map(|r| r.y).min().unwrap_or(0);
        let right = rects.iter().map(|r| r.right()).max().unwrap_or(0);
        let bottom = rects.iter().map(|r| r.bottom()).max().unwrap_or(0);
        lay::Rect::new(left, top, right - left, bottom - top)
    };
    let mut out = rects.to_vec();
    match op {
        AlignOp::Left => out.iter_mut().for_each(|r| r.x = bounds.x),
        AlignOp::HCenter => out.iter_mut().for_each(|r| r.x = bounds.x + (bounds.w - r.w) / 2),
        AlignOp::Right => out.iter_mut().for_each(|r| r.x = bounds.right() - r.w),
        AlignOp::Top => out.iter_mut().for_each(|r| r.y = bounds.y),
        AlignOp::VCenter => out.iter_mut().for_each(|r| r.y = bounds.y + (bounds.h - r.h) / 2),
        AlignOp::Bottom => out.iter_mut().for_each(|r| r.y = bounds.bottom() - r.h),
        AlignOp::DistributeH | AlignOp::DistributeV if rects.len() >= 3 => {
            let horizontal = op == AlignOp::DistributeH;
            let mut order: Vec<usize> = (0..rects.len()).collect();
            order.sort_by_key(|&i| if horizontal { rects[i].x } else { rects[i].y });
            let total: i32 = rects.iter().map(|r| if horizontal { r.w } else { r.h }).sum();
            let span = if horizontal { bounds.w } else { bounds.h };
            let gap = (span - total) as f32 / (rects.len() - 1) as f32;
            let mut position = if horizontal { bounds.x } else { bounds.y } as f32;
            for i in order {
                let r = &mut out[i];
                if horizontal {
                    r.x = position.round() as i32;
                    position += r.w as f32 + gap;
                } else {
                    r.y = position.round() as i32;
                    position += r.h as f32 + gap;
                }
            }
        }
        _ => {}
    }
    out
}

/// Applies `op` to the selection.
pub fn apply(studio: &mut Studio, op: AlignOp) {
    let ids = studio.selection();
    let nodes: Vec<NodeInfo> =
        ids.iter().filter_map(|id| studio.canvas.nodes.iter().find(|n| n.id == *id && n.movable).cloned()).collect();
    let Some(first) = nodes.first() else { return };
    let Some(parent) = first.parent.map(|p| studio.canvas.nodes[p].clone()) else { return };
    let rects: Vec<lay::Rect> = nodes.iter().map(|n| n.rect).collect();
    let arranged = arrange(op, &rects, parent.children_area);
    for (node, rect) in nodes.iter().zip(arranged) {
        let Some(parent) = node.parent.map(|p| studio.canvas.nodes[p].clone()) else { continue };
        if let Some(widget) = studio.doc.project.widget_mut(node.id) {
            widget.geometry = geometry_for_rect(widget.geometry, rect, &parent, false);
        }
    }
}

/// The align buttons of the canvas toolbar.
pub fn toolbar(ui: &mut egui::Ui, studio: &mut Studio) {
    let count = studio.selection().len();
    let movable = studio.selection().iter().any(|id| studio.canvas.nodes.iter().any(|n| n.id == *id && n.movable));
    for op in AlignOp::ALL {
        let enabled = movable && (!op.is_distribute() || count >= 3);
        let hint = if count > 1 || op.is_distribute() {
            tr(op.label()).to_owned()
        } else {
            format!("{} ({})", tr(op.label()), tr("in the parent"))
        };
        if icon_button(ui, op, enabled).on_hover_text(hint).clicked() {
            apply(studio, op);
        }
    }
}

fn icon_button(ui: &mut egui::Ui, op: AlignOp, enabled: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(22.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let visuals = ui.style().interact(&response);
    if response.hovered() && enabled {
        ui.painter().rect_filled(rect, 4, visuals.weak_bg_fill);
    }
    let color = if enabled { visuals.fg_stroke.color } else { ui.visuals().weak_text_color() };
    let p = ui.painter();
    let c = rect.center();
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], Stroke::new(1.5_f32, color));
    let bar = |r: Rect| p.rect_filled(r, 1, color.gamma_multiply(0.75));
    let r = |cx: f32, cy: f32, w: f32, h: f32| Rect::from_center_size(pos2(cx, cy), vec2(w, h));
    match op {
        AlignOp::Left | AlignOp::HCenter | AlignOp::Right => {
            let x = match op {
                AlignOp::Left => c.x - 7.0,
                AlignOp::HCenter => c.x,
                _ => c.x + 7.0,
            };
            line(pos2(x, c.y - 8.0), pos2(x, c.y + 8.0));
            let offset = |w: f32| match op {
                AlignOp::Left => x + w / 2.0 + 1.0,
                AlignOp::HCenter => x,
                _ => x - w / 2.0 - 1.0,
            };
            bar(r(offset(10.0), c.y - 3.5, 10.0, 4.0));
            bar(r(offset(6.0), c.y + 3.5, 6.0, 4.0));
        }
        AlignOp::Top | AlignOp::VCenter | AlignOp::Bottom => {
            let y = match op {
                AlignOp::Top => c.y - 7.0,
                AlignOp::VCenter => c.y,
                _ => c.y + 7.0,
            };
            line(pos2(c.x - 8.0, y), pos2(c.x + 8.0, y));
            let offset = |h: f32| match op {
                AlignOp::Top => y + h / 2.0 + 1.0,
                AlignOp::VCenter => y,
                _ => y - h / 2.0 - 1.0,
            };
            bar(r(c.x - 3.5, offset(10.0), 4.0, 10.0));
            bar(r(c.x + 3.5, offset(6.0), 4.0, 6.0));
        }
        AlignOp::DistributeH => {
            line(pos2(c.x - 8.0, c.y - 7.0), pos2(c.x - 8.0, c.y + 7.0));
            line(pos2(c.x + 8.0, c.y - 7.0), pos2(c.x + 8.0, c.y + 7.0));
            bar(r(c.x, c.y, 5.0, 9.0));
        }
        AlignOp::DistributeV => {
            line(pos2(c.x - 7.0, c.y - 8.0), pos2(c.x + 7.0, c.y - 8.0));
            line(pos2(c.x - 7.0, c.y + 8.0), pos2(c.x + 7.0, c.y + 8.0));
            bar(r(c.x, c.y, 9.0, 5.0));
        }
    }
    response
}

/// Snap lines: the moved rectangle snaps its edges and center to those of
/// its siblings and its parent. Returns the snapped rectangle and the
/// guide lines (display coordinates; x lines and y lines).
pub fn snap(rect: lay::Rect, moving: usize, nodes: &[NodeInfo], zoom: f32) -> (lay::Rect, Option<i32>, Option<i32>) {
    let Some(parent) = nodes.get(moving).and_then(|n| n.parent) else { return (rect, None, None) };
    let threshold = (SNAP_PX / zoom).ceil() as i32;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let area = nodes[parent].children_area;
    xs.extend([area.x, area.x + area.w / 2, area.right()]);
    ys.extend([area.y, area.y + area.h / 2, area.bottom()]);
    for (i, n) in nodes.iter().enumerate() {
        if i == moving || n.parent != Some(parent) || n.hidden || n.id == nodes[moving].id {
            continue;
        }
        xs.extend([n.rect.x, n.rect.x + n.rect.w / 2, n.rect.right()]);
        ys.extend([n.rect.y, n.rect.y + n.rect.h / 2, n.rect.bottom()]);
    }
    let best = |own: [i32; 3], targets: &[i32]| -> Option<(i32, i32)> {
        own.iter()
            .flat_map(|o| targets.iter().map(move |t| (t - o, *t)))
            .filter(|(d, _)| d.abs() <= threshold)
            .min_by_key(|(d, _)| d.abs())
    };
    let mut out = rect;
    let gx = best([rect.x, rect.x + rect.w / 2, rect.right()], &xs).map(|(d, line)| {
        out.x += d;
        line
    });
    let gy = best([rect.y, rect.y + rect.h / 2, rect.bottom()], &ys).map(|(d, line)| {
        out.y += d;
        line
    });
    (out, gx, gy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_and_distributes() {
        let rects = [lay::Rect::new(10, 0, 20, 10), lay::Rect::new(50, 30, 40, 10), lay::Rect::new(120, 5, 10, 10)];
        let area = lay::Rect::new(0, 0, 200, 100);
        let left = arrange(AlignOp::Left, &rects, area);
        assert!(left.iter().all(|r| r.x == 10));
        let right = arrange(AlignOp::Right, &rects, area);
        assert!(right.iter().all(|r| r.right() == 130));
        let spread = arrange(AlignOp::DistributeH, &rects, area);
        // Total width 70 in a span of 120 leaves two gaps of 25.
        assert_eq!(spread.iter().map(|r| r.x).collect::<Vec<_>>(), vec![10, 55, 120]);
        let single = arrange(AlignOp::HCenter, &rects[..1], area);
        assert_eq!(single[0].x, 90);
    }

    #[test]
    fn snaps_to_siblings() {
        let node = |id, rect, parent| NodeInfo {
            id: lvglab_model::WidgetId(id),
            kind: lvglab_model::WidgetKind::Panel,
            rect,
            clip: rect,
            children_area: rect,
            content: rect,
            parent,
            hidden: false,
            movable: true,
        };
        let nodes = vec![
            node(1, lay::Rect::new(0, 0, 400, 300), None),
            node(2, lay::Rect::new(100, 50, 50, 20), Some(0)),
            node(3, lay::Rect::new(0, 0, 30, 30), Some(0)),
        ];
        let (snapped, gx, _) = snap(lay::Rect::new(103, 200, 30, 30), 2, &nodes, 1.0);
        assert_eq!((snapped.x, gx), (100, Some(100)));
    }
}
