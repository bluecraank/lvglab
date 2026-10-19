//! Several projects open at once, one tab each. The project of the active
//! tab lives in the `Studio` fields the panels work on; the others are
//! parked in a `Workspace` and swapped in when their tab is clicked.

use eframe::egui::{self, RichText, Sense, Stroke, vec2};
use lvglab_model::WidgetId;
use lvglab_model::flags::StateMask;

use crate::app::Studio;
use crate::canvas::CanvasState;
use crate::commands::Command;
use crate::document::{Document, Timeline};
use crate::i18n::tr;
use crate::visuals::{self, ACCENT};

/// Everything that belongs to one open project.
pub struct Workspace {
    pub doc: Document,
    pub timeline: Timeline,
    pub selected: Option<WidgetId>,
    pub extra_selection: Vec<WidgetId>,
    pub screen: WidgetId,
    pub canvas: CanvasState,
    pub style_slot: String,
    pub style_state: StateMask,
    pub timeline_this_screen: bool,
}

impl Workspace {
    fn new(doc: Document, timeline: Timeline) -> Self {
        let screen = doc.project.screens.first().map_or(WidgetId(0), |s| s.id);
        Self {
            doc,
            timeline,
            selected: None,
            extra_selection: Vec::new(),
            screen,
            canvas: CanvasState::default(),
            style_slot: "main".to_owned(),
            style_state: StateMask::DEFAULT,
            timeline_this_screen: false,
        }
    }

    /// No changes and nothing worth keeping: a freshly started or created project.
    fn is_pristine(&self) -> bool {
        self.doc.path.is_none() && !self.doc.is_dirty() && self.timeline.entries().len() <= 1
    }
}

impl Studio {
    /// Swaps the project that is shown with `other`; returns what was shown.
    fn swap_workspace(&mut self, mut other: Workspace) -> Workspace {
        std::mem::swap(&mut self.doc, &mut other.doc);
        std::mem::swap(&mut self.timeline, &mut other.timeline);
        std::mem::swap(&mut self.selected, &mut other.selected);
        std::mem::swap(&mut self.extra_selection, &mut other.extra_selection);
        std::mem::swap(&mut self.screen, &mut other.screen);
        std::mem::swap(&mut self.canvas, &mut other.canvas);
        std::mem::swap(&mut self.style_slot, &mut other.style_slot);
        std::mem::swap(&mut self.style_state, &mut other.style_state);
        std::mem::swap(&mut self.timeline_this_screen, &mut other.timeline_this_screen);
        // What belonged to the other project is closed.
        self.simulation = None;
        self.search = None;
        self.merge = None;
        self.canvas.fit_pending = true;
        other
    }

    /// Shows a project in a new tab (or in the current one if that is untouched).
    pub fn open_in_tab(&mut self, doc: Document, timeline: Timeline) {
        let parked = self.swap_workspace(Workspace::new(doc, timeline));
        if parked.is_pristine() {
            return;
        }
        self.tabs[self.active_tab] = Some(parked);
        self.tabs.insert(self.active_tab + 1, None);
        self.active_tab += 1;
    }

    /// Number of open projects.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    fn tab_doc(&self, index: usize) -> Option<&Document> {
        if index == self.active_tab { Some(&self.doc) } else { self.tabs.get(index)?.as_ref().map(|w| &w.doc) }
    }

    pub fn tab_dirty(&self, index: usize) -> bool {
        self.tab_doc(index).is_some_and(Document::is_dirty)
    }

    pub fn any_tab_dirty(&self) -> bool {
        (0..self.tab_count()).any(|i| self.tab_dirty(i))
    }

    /// The tab that shows `path`, if the project is open already.
    pub fn tab_of(&self, path: &std::path::Path) -> Option<usize> {
        (0..self.tab_count()).find(|&i| self.tab_doc(i).and_then(|d| d.path.as_deref()) == Some(path))
    }

    pub fn switch_tab(&mut self, index: usize) {
        if index == self.active_tab {
            return;
        }
        let Some(target) = self.tabs.get_mut(index).and_then(Option::take) else { return };
        let parked = self.swap_workspace(target);
        self.tabs[self.active_tab] = Some(parked);
        self.active_tab = index;
    }

    /// Closes a tab; the last one is replaced by an empty project and the start page.
    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tab_count() {
            return;
        }
        if self.tab_count() == 1 {
            let project = lvglab_model::Project::new("MyProject");
            let timeline = Timeline::new(&project, tr("New project"));
            self.swap_workspace(Workspace::new(Document::new(project, None), timeline));
            self.start_page = Some(crate::start::entries(&self.settings.recent));
            return;
        }
        if index == self.active_tab {
            let next = if index + 1 < self.tabs.len() { index + 1 } else { index - 1 };
            self.switch_tab(next);
        }
        self.tabs.remove(index);
        if index < self.active_tab {
            self.active_tab -= 1;
        }
    }
}

/// The tab strip below the menu.
pub fn show(ui: &mut egui::Ui, studio: &mut Studio) {
    let palette = visuals::palette(ui.ctx());
    let mut switch = None;
    let mut close = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for index in 0..studio.tab_count() {
            let active = index == studio.active_tab;
            let (title, path) = studio.tab_doc(index).map_or_else(Default::default, |d| (d.title(), d.path.clone()));
            let (rect, response) = ui.allocate_exact_size(vec2(170.0, 26.0), Sense::click());
            let fill = if active {
                visuals::ACCENT_SOFT
            } else if response.hovered() {
                ui.visuals().widgets.hovered.weak_bg_fill
            } else {
                ui.visuals().widgets.inactive.weak_bg_fill
            };
            ui.painter().rect_filled(rect, 6, fill);
            if active {
                ui.painter().rect_stroke(rect, 6, Stroke::new(1.0_f32, ACCENT), egui::StrokeKind::Inside);
            }
            let text_color = if active { ui.visuals().strong_text_color() } else { palette.muted };
            let shown = if title.chars().count() > 18 {
                format!("{}…", title.chars().take(17).collect::<String>())
            } else {
                title
            };
            ui.painter().text(
                rect.left_center() + vec2(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                shown,
                egui::FontId::proportional(13.0),
                text_color,
            );
            // The close square at the right.
            let close_rect = egui::Rect::from_center_size(rect.right_center() - vec2(14.0, 0.0), vec2(18.0, 18.0));
            let close_response = ui.interact(close_rect, ui.id().with(("tab_close", index)), Sense::click());
            if close_response.hovered() {
                ui.painter().rect_filled(close_rect, 4, ui.visuals().widgets.hovered.bg_fill);
            }
            let c = close_rect.center();
            let d = 3.5;
            let stroke = Stroke::new(1.5_f32, text_color);
            ui.painter().line_segment([c + vec2(-d, -d), c + vec2(d, d)], stroke);
            ui.painter().line_segment([c + vec2(-d, d), c + vec2(d, -d)], stroke);
            if close_response.clicked() || response.middle_clicked() {
                close = Some(index);
            } else if response.clicked() && !active {
                switch = Some(index);
            }
            response.on_hover_text(path.map_or_else(|| tr("Not saved yet").to_owned(), |p| p.display().to_string()));
        }
        let plus = egui::Button::new(RichText::new("+").size(15.0)).min_size(vec2(26.0, 26.0));
        if ui.add(plus).on_hover_text(tr("Open another project")).clicked() {
            studio.push(Command::ShowStartPage);
        }
    });
    if let Some(index) = close {
        studio.push(Command::CloseTab(index));
    } else if let Some(index) = switch {
        studio.push(Command::SwitchTab(index));
    }
}
