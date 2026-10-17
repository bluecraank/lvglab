//! Widget tree of the current screen with drag & drop reordering.

use eframe::egui::{self, RichText, Sense, Stroke};
use lvglab_model::flags::ObjFlag;
use lvglab_model::{Widget, WidgetId};

use crate::app::Studio;
use crate::commands::Command;
use crate::i18n::tr;
use crate::visuals::{self, ACCENT};

/// Payload while dragging a layer.
#[derive(Clone, Copy)]
struct LayerDrag(WidgetId);

enum Drop {
    Before(WidgetId),
    Into(WidgetId),
    After(WidgetId),
}

pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    ui.add(
        egui::TextEdit::singleline(&mut studio.layer_filter)
            .hint_text(tr("Filter layers…"))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    let Some(screen) = studio.doc.project.widget(studio.screen).cloned() else { return };
    let filter = studio.layer_filter.to_lowercase();
    let mut actions = Actions::default();
    node(ui, studio, &screen, 0, &filter, &mut actions);
    apply(studio, actions);
}

#[derive(Default)]
struct Actions {
    select: Option<WidgetId>,
    toggle_hidden: Option<WidgetId>,
    drop: Option<(WidgetId, Drop)>,
    commands: Vec<Command>,
}

fn matches(widget: &Widget, filter: &str) -> bool {
    filter.is_empty()
        || widget.name.to_lowercase().contains(filter)
        || widget.def().name.to_lowercase().contains(filter)
        || widget.children.iter().any(|c| matches(c, filter))
}

fn node(ui: &mut egui::Ui, studio: &Studio, widget: &Widget, depth: usize, filter: &str, actions: &mut Actions) {
    if !matches(widget, filter) {
        return;
    }
    let id = ui.make_persistent_id(("layer", widget.id));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, depth < 2);
    let has_children = !widget.children.is_empty();
    let header = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(depth as f32 * 12.0);
        if has_children {
            let icon = if state.is_open() { "⏷" } else { "⏵" };
            if ui.add(egui::Label::new(icon).sense(Sense::click())).clicked() {
                state.toggle(ui);
            }
        } else {
            ui.add_space(10.0);
        }
        let hidden = widget.is_hidden();
        let selected = studio.selected == Some(widget.id);
        let mut text = RichText::new(format!("{}  {}", widget.def().icon, widget.name));
        if hidden {
            text = text.color(visuals::palette(ui.ctx()).muted).italics();
        }
        let label = ui.selectable_label(selected, text).interact(Sense::click_and_drag());
        if widget.kind != lvglab_model::WidgetKind::Screen {
            let muted = visuals::palette(ui.ctx()).muted;
            let eye = RichText::new("👁").color(if hidden { muted.gamma_multiply(0.4) } else { muted });
            let eye_response =
                ui.add(egui::Label::new(eye).sense(Sense::click())).on_hover_text(tr("Toggle hidden flag"));
            if eye_response.clicked() {
                actions.toggle_hidden = Some(widget.id);
            }
        }
        label
    });
    let label = header.inner;
    if label.clicked() {
        actions.select = Some(widget.id);
    }
    if label.drag_started() && widget.kind != lvglab_model::WidgetKind::Screen {
        label.dnd_set_drag_payload(LayerDrag(widget.id));
    }
    drop_target(ui, &label, widget, actions);
    label.context_menu(|ui| context_menu(ui, widget, actions));
    if has_children {
        state.store(ui.ctx());
        if state.is_open() {
            for child in widget.children.iter().rev() {
                node(ui, studio, child, depth + 1, filter, actions);
            }
        }
    }
}

/// Children are listed topmost first (reverse paint order), like in
/// image editors; dropping above a row therefore means "after" in LVGL order.
fn drop_target(ui: &egui::Ui, row: &egui::Response, widget: &Widget, actions: &mut Actions) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else { return };
    if !row.rect.expand2(egui::vec2(200.0, 0.0)).contains(pointer) {
        return;
    }
    let Some(payload) = row.dnd_hover_payload::<LayerDrag>() else { return };
    if payload.0 == widget.id {
        return;
    }
    let rect = row.rect;
    let t = (pointer.y - rect.top()) / rect.height().max(1.0);
    let can_contain = widget.def().container;
    let is_screen = widget.kind == lvglab_model::WidgetKind::Screen;
    let drop = if is_screen || (can_contain && (0.25..0.75).contains(&t)) {
        Drop::Into(widget.id)
    } else if t < 0.5 {
        Drop::After(widget.id)
    } else {
        Drop::Before(widget.id)
    };
    let painter = ui.painter();
    let stroke = Stroke::new(2.0_f32, ACCENT);
    match drop {
        Drop::Into(_) => {
            painter.rect_stroke(rect, 3, stroke, egui::StrokeKind::Inside);
        }
        Drop::After(_) => {
            painter.hline(rect.x_range(), rect.top(), stroke);
        }
        Drop::Before(_) => {
            painter.hline(rect.x_range(), rect.bottom(), stroke);
        }
    }
    if row.dnd_release_payload::<LayerDrag>().is_some() {
        actions.drop = Some((payload.0, drop));
    }
}

fn context_menu(ui: &mut egui::Ui, widget: &Widget, actions: &mut Actions) {
    let id = widget.id;
    let mut item = |ui: &mut egui::Ui, text: &str, command: Command| {
        if ui.button(text).clicked() {
            actions.commands.push(command);
            ui.close_menu();
        }
    };
    if widget.kind != lvglab_model::WidgetKind::Screen {
        item(ui, tr("Bring forward"), Command::Shift { id, delta: 1 });
        item(ui, tr("Send backward"), Command::Shift { id, delta: -1 });
        ui.separator();
    }
    item(ui, tr("Duplicate"), Command::Duplicate(id));
    item(ui, tr("Copy"), Command::Copy(id));
    item(ui, tr("Cut"), Command::Cut(id));
    item(ui, tr("Paste"), Command::Paste);
    ui.separator();
    item(ui, tr("Delete"), Command::Delete(id));
}

fn apply(studio: &mut Studio, actions: Actions) {
    if let Some(id) = actions.select {
        studio.select(Some(id));
    }
    if let Some(id) = actions.toggle_hidden {
        if let Some(widget) = studio.doc.project.widget_mut(id) {
            let hidden = widget.is_hidden();
            widget.set_flag(ObjFlag::Hidden, !hidden);
        }
    }
    studio.commands.extend(actions.commands);
    let Some((dragged, drop)) = actions.drop else { return };
    let project = &studio.doc.project;
    let target = match drop {
        Drop::Into(parent) => Some((parent, usize::MAX)),
        Drop::Before(sibling) | Drop::After(sibling) => project.location(sibling).and_then(|l| {
            let offset = usize::from(matches!(drop, Drop::After(_)));
            l.parent.map(|p| (p, l.index + offset))
        }),
    };
    if let Some((parent, index)) = target {
        studio.push(Command::Move { id: dragged, parent, index });
    }
}
