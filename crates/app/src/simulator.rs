//! Interactive simulation of the UI: the canvas plays the project like the
//! device would. Widgets react to the mouse (pressed state, switches, check
//! boxes, sliders, arcs) and the events of the project run (screen changes,
//! flags, states, texts, values). The simulation works on a copy of the
//! project, so nothing in the document changes.
//!
//! The record overlay draws the pointer and every click, for screen
//! recordings of the simulated device.

mod live;

pub use live::LiveMode;

use eframe::egui::{self, Color32, Pos2, Stroke};
use lvglab_model::event::{Action, EventTrigger, ModifyOp, ScreenAnim};
use lvglab_model::flags::{ObjFlag, ObjState, StateMask};
use lvglab_model::layout as lay;
use lvglab_model::prop::PropValue;
use lvglab_model::{Project, WidgetId, WidgetKind};

use crate::canvas::NodeInfo;
use crate::visuals::ACCENT;

/// Press duration that counts as a long press (LVGL default).
const LONG_PRESS_S: f64 = 0.4;
/// Movement in display pixels that makes a press a gesture.
const GESTURE_PX: i32 = 40;
/// How long a click ripple stays visible.
const RIPPLE_S: f64 = 0.5;
const LOG_SIZE: usize = 6;

struct Press {
    id: WidgetId,
    rect: lay::Rect,
    start: (i32, i32),
    time: f64,
    long_fired: bool,
    /// The press changes a slider or arc value instead of clicking.
    adjusts_value: bool,
}

struct ScheduledScreen {
    at: f64,
    screen: WidgetId,
    anim: ScreenAnim,
    duration: f64,
}

/// A running screen load animation (`lv_scr_load_anim`).
#[derive(Clone, Copy, Debug)]
pub struct Transition {
    pub from: WidgetId,
    pub anim: ScreenAnim,
    start: f64,
    duration: f64,
}

/// How the old and the new screen are drawn at one moment of a transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionFrame {
    /// Offsets as fractions of the display size.
    pub old_offset: (f32, f32),
    pub new_offset: (f32, f32),
    pub old_opacity: f32,
    pub new_opacity: f32,
    /// The new screen is drawn above the old one.
    pub new_on_top: bool,
}

impl Transition {
    /// Progress from 0 to 1 with LVGL's ease-out path.
    fn progress(&self, now: f64) -> f32 {
        let t = (((now - self.start) / self.duration.max(0.001)) as f32).clamp(0.0, 1.0);
        1.0 - (1.0 - t) * (1.0 - t)
    }

    pub fn frame(&self, now: f64) -> TransitionFrame {
        let p = self.progress(now);
        let mut frame = TransitionFrame {
            old_offset: (0.0, 0.0),
            new_offset: (0.0, 0.0),
            old_opacity: 1.0,
            new_opacity: 1.0,
            new_on_top: true,
        };
        // Direction the screens move to.
        let dir = |anim: ScreenAnim| match anim {
            ScreenAnim::OverLeft | ScreenAnim::MoveLeft | ScreenAnim::OutLeft => (-1.0, 0.0),
            ScreenAnim::OverRight | ScreenAnim::MoveRight | ScreenAnim::OutRight => (1.0, 0.0),
            ScreenAnim::OverTop | ScreenAnim::MoveTop | ScreenAnim::OutTop => (0.0, -1.0),
            _ => (0.0, 1.0),
        };
        let (dx, dy) = dir(self.anim);
        match self.anim {
            ScreenAnim::OverLeft | ScreenAnim::OverRight | ScreenAnim::OverTop | ScreenAnim::OverBottom => {
                frame.new_offset = (-dx * (1.0 - p), -dy * (1.0 - p));
            }
            ScreenAnim::MoveLeft | ScreenAnim::MoveRight | ScreenAnim::MoveTop | ScreenAnim::MoveBottom => {
                frame.new_offset = (-dx * (1.0 - p), -dy * (1.0 - p));
                frame.old_offset = (dx * p, dy * p);
            }
            ScreenAnim::OutLeft | ScreenAnim::OutRight | ScreenAnim::OutTop | ScreenAnim::OutBottom => {
                frame.old_offset = (dx * p, dy * p);
                frame.new_on_top = false;
            }
            ScreenAnim::FadeOn => frame.new_opacity = p,
            ScreenAnim::FadeOut => {
                frame.old_opacity = 1.0 - p;
                frame.new_on_top = false;
            }
            ScreenAnim::None => {}
        }
        frame
    }

    fn finished(&self, now: f64) -> bool {
        now - self.start >= self.duration
    }
}

pub struct Simulation {
    pub project: Project,
    pub screen: WidgetId,
    /// Draw the pointer and clicks for screen recordings.
    pub record: bool,
    /// Recent events, newest last.
    pub log: Vec<String>,
    press: Option<Press>,
    scheduled: Vec<ScheduledScreen>,
    /// The screen load animation that is running.
    pub transition: Option<Transition>,
    /// Test data for gauges and numeric labels.
    pub live_mode: LiveMode,
    live: live::LiveData,
    /// Click positions (screen coordinates) and times for ripples.
    clicks: Vec<(Pos2, f64)>,
}

impl Simulation {
    pub fn new(project: &Project, screen: WidgetId) -> Self {
        let mut simulation = Self {
            project: project.clone(),
            screen,
            record: false,
            log: Vec::new(),
            press: None,
            scheduled: Vec::new(),
            transition: None,
            live_mode: LiveMode::Off,
            live: live::LiveData::collect(project),
            clicks: Vec::new(),
        };
        simulation.fire(screen, EventTrigger::ScreenLoaded, 0.0);
        simulation
    }

    pub fn pressed(&self) -> Option<WidgetId> {
        self.press.as_ref().map(|p| p.id)
    }

    /// The state preview of the pressed widget for the renderer.
    pub fn preview_state(&self) -> Option<(WidgetId, StateMask)> {
        self.pressed().map(|id| (id, StateMask::DEFAULT.with(ObjState::Pressed)))
    }

    /// True while something changes over time and the canvas must repaint.
    pub fn is_animating(&self, now: f64) -> bool {
        self.press.is_some()
            || !self.scheduled.is_empty()
            || self.transition.is_some()
            || self.live_mode == LiveMode::Wave
            || self.clicks.iter().any(|(_, t)| now - t < RIPPLE_S)
    }

    pub fn show_screen(&mut self, screen: WidgetId, now: f64) {
        self.load_screen(screen, ScreenAnim::None, 0.0, now);
    }

    /// Loads `screen` like `lv_scr_load_anim`.
    fn load_screen(&mut self, screen: WidgetId, anim: ScreenAnim, duration: f64, now: f64) {
        if screen == self.screen || self.project.widget(screen).is_none() {
            return;
        }
        let old = self.screen;
        self.transition = (anim != ScreenAnim::None && duration > 0.0).then_some(Transition {
            from: old,
            anim,
            start: now,
            duration,
        });
        self.fire(old, EventTrigger::ScreenUnloaded, now);
        self.screen = screen;
        self.press = None;
        self.fire(screen, EventTrigger::ScreenLoaded, now);
    }

    fn log(&mut self, message: String) {
        self.log.push(message);
        if self.log.len() > LOG_SIZE {
            self.log.remove(0);
        }
    }

    fn name(&self, id: WidgetId) -> String {
        self.project.widget(id).map(|w| w.name.clone()).unwrap_or_default()
    }

    /// Advances timers: long presses and delayed screen changes.
    /// True if the project has values that live data can animate.
    pub fn has_live_values(&self) -> bool {
        !self.live.is_empty()
    }

    pub fn tick(&mut self, now: f64) {
        if self.live_mode != LiveMode::Off {
            let pressed = self.pressed();
            self.live.apply(&mut self.project, self.live_mode, now, pressed);
        }
        if let Some(press) = &mut self.press {
            if !press.long_fired && !press.adjusts_value && now - press.time >= LONG_PRESS_S {
                press.long_fired = true;
                let id = press.id;
                self.fire(id, EventTrigger::LongPressed, now);
            }
        }
        if let Some(index) = self.scheduled.iter().position(|s| s.at <= now) {
            let change = self.scheduled.remove(index);
            self.load_screen(change.screen, change.anim, change.duration, now);
        }
        if self.transition.is_some_and(|t| t.finished(now)) {
            self.transition = None;
        }
        self.clicks.retain(|(_, t)| now - t < RIPPLE_S);
    }

    /// The object that receives a press at `(x, y)`: the topmost visible
    /// clickable widget, like LVGL's hit test.
    fn target(&self, nodes: &[NodeInfo], x: i32, y: i32) -> Option<(WidgetId, lay::Rect)> {
        let index = nodes.iter().rposition(|n| {
            !n.hidden
                && n.rect.contains(x, y)
                && n.clip.contains(x, y)
                && self
                    .project
                    .widget(n.id)
                    .is_some_and(|w| w.flag(ObjFlag::Clickable) && !w.states.contains(&ObjState::Disabled))
        })?;
        Some((nodes[index].id, nodes[index].rect))
    }

    pub fn is_clickable_at(&self, nodes: &[NodeInfo], x: i32, y: i32) -> bool {
        self.target(nodes, x, y).is_some_and(|(id, _)| id != self.screen)
    }

    pub fn press(&mut self, nodes: &[NodeInfo], x: i32, y: i32, now: f64) {
        if self.transition.is_some() {
            return;
        }
        let Some((id, rect)) = self.target(nodes, x, y) else { return };
        let kind = self.project.widget(id).map(|w| w.kind);
        let adjusts_value = matches!(kind, Some(WidgetKind::Slider | WidgetKind::Arc));
        self.press = Some(Press { id, rect, start: (x, y), time: now, long_fired: false, adjusts_value });
        self.fire(id, EventTrigger::Pressed, now);
        if adjusts_value {
            self.adjust_value(id, rect, x, y, now);
        }
    }

    pub fn drag(&mut self, x: i32, y: i32, now: f64) {
        let Some(press) = &self.press else { return };
        if press.adjusts_value {
            let (id, rect) = (press.id, press.rect);
            self.adjust_value(id, rect, x, y, now);
        }
    }

    /// Ends a press without a click, e.g. when the button is released
    /// outside the window.
    pub fn cancel(&mut self, now: f64) {
        if let Some(press) = self.press.take() {
            self.fire(press.id, EventTrigger::PressLost, now);
            self.fire(press.id, EventTrigger::Released, now);
        }
    }

    pub fn release(&mut self, x: i32, y: i32, pos: Pos2, now: f64) {
        self.clicks.push((pos, now));
        let Some(press) = self.press.take() else { return };
        let id = press.id;
        self.fire(id, EventTrigger::Released, now);
        let (dx, dy) = (x - press.start.0, y - press.start.1);
        if !press.adjusts_value && (dx.abs() >= GESTURE_PX || dy.abs() >= GESTURE_PX) {
            let gesture = if dx.abs() > dy.abs() {
                if dx > 0 { EventTrigger::GestureRight } else { EventTrigger::GestureLeft }
            } else if dy > 0 {
                EventTrigger::GestureBottom
            } else {
                EventTrigger::GestureTop
            };
            // Gestures bubble to the screen in LVGL.
            self.fire(id, gesture, now);
            if id != self.screen {
                self.fire(self.screen, gesture, now);
            }
            return;
        }
        if !press.rect.contains(x, y) {
            self.fire(id, EventTrigger::PressLost, now);
            return;
        }
        if press.adjusts_value {
            return;
        }
        self.click(id, now);
        if !press.long_fired {
            self.fire(id, EventTrigger::ShortClicked, now);
        }
        self.fire(id, EventTrigger::Clicked, now);
    }

    /// What a click does to the widget itself before the events run.
    fn click(&mut self, id: WidgetId, now: f64) {
        let Some(widget) = self.project.widget_mut(id) else { return };
        if widget.flag(ObjFlag::Checkable) {
            let checked = !widget.states.contains(&ObjState::Checked);
            if checked {
                widget.states.insert(ObjState::Checked);
            } else {
                widget.states.remove(&ObjState::Checked);
            }
            self.fire(id, EventTrigger::ValueChanged, now);
            self.fire(id, if checked { EventTrigger::Checked } else { EventTrigger::Unchecked }, now);
            return;
        }
        if matches!(widget.kind, WidgetKind::Dropdown | WidgetKind::Roller) {
            // Without an opened list, a click selects the next option.
            let count = widget.prop_text("options").lines().count().max(1) as i32;
            let next = (widget.prop_int("selected") + 1) % count;
            widget.set_prop("selected", PropValue::Int(next));
            self.fire(id, EventTrigger::ValueChanged, now);
        }
    }

    fn adjust_value(&mut self, id: WidgetId, rect: lay::Rect, x: i32, y: i32, now: f64) {
        let Some(widget) = self.project.widget_mut(id) else { return };
        let (min, max) = widget.prop_pair("range");
        let fraction = match widget.kind {
            WidgetKind::Arc => {
                let rotation = widget.prop_int("rotation");
                let (start, end) = widget.prop_pair("bg_angles");
                arc_fraction(rect, start + rotation, end + rotation, x, y)
            }
            _ => slider_fraction(rect, x, y),
        };
        let value = min + ((max - min) as f32 * fraction).round() as i32;
        if widget.prop_int("value") != value {
            widget.set_prop("value", PropValue::Int(value));
            self.fire(id, EventTrigger::ValueChanged, now);
        }
    }

    /// Runs the actions of `id` for `trigger`.
    fn fire(&mut self, id: WidgetId, trigger: EventTrigger, now: f64) {
        let Some(widget) = self.project.widget(id) else { return };
        let actions: Vec<Action> =
            widget.events.iter().filter(|h| h.trigger == trigger).map(|h| h.action.clone()).collect();
        for action in actions {
            self.run(&action, now);
        }
    }

    fn run(&mut self, action: &Action, now: f64) {
        match action {
            Action::ChangeScreen { screen: Some(screen), anim, time_ms, delay_ms } => {
                self.log(format!("→ {}", self.name(*screen)));
                self.scheduled.push(ScheduledScreen {
                    at: now + f64::from(*delay_ms) / 1000.0,
                    screen: *screen,
                    anim: *anim,
                    duration: f64::from(*time_ms) / 1000.0,
                });
            }
            Action::CallFunction { name } => self.log(format!("{name}(e)")),
            Action::ModifyFlag { target: Some(target), flag, op } => {
                if let Some(widget) = self.project.widget_mut(*target) {
                    let on = apply(*op, widget.flag(*flag));
                    widget.set_flag(*flag, on);
                }
            }
            Action::ModifyState { target: Some(target), state, op } => {
                if let Some(widget) = self.project.widget_mut(*target) {
                    if apply(*op, widget.states.contains(state)) {
                        widget.states.insert(*state);
                    } else {
                        widget.states.remove(state);
                    }
                }
            }
            Action::SetText { target: Some(target), text } => {
                if let Some(widget) = self.project.widget_mut(*target) {
                    widget.set_prop("text", PropValue::Text(text.clone()));
                }
            }
            Action::SetValue { target: Some(target), value } => {
                if let Some(widget) = self.project.widget_mut(*target) {
                    let key = match widget.kind {
                        WidgetKind::Roller | WidgetKind::Dropdown => "selected",
                        WidgetKind::Led => "brightness",
                        _ => "value",
                    };
                    widget.set_prop(key, PropValue::Int(*value));
                }
            }
            _ => {}
        }
    }

    /// Pointer and click ripples for screen recordings.
    pub fn draw_overlay(&self, painter: &egui::Painter, pointer: Option<Pos2>, down: bool, now: f64) {
        for (pos, time) in &self.clicks {
            let t = ((now - time) / RIPPLE_S).clamp(0.0, 1.0) as f32;
            let alpha = 1.0 - t;
            painter.circle_stroke(*pos, 10.0 + 22.0 * t, Stroke::new(3.0 * alpha + 0.5, ACCENT.gamma_multiply(alpha)));
        }
        let Some(pos) = pointer else { return };
        if down {
            painter.circle_filled(pos, 11.0, ACCENT.gamma_multiply(0.55));
            painter.circle_stroke(pos, 11.0, Stroke::new(2.0_f32, Color32::WHITE));
        } else {
            painter.circle_filled(pos, 9.0, Color32::from_white_alpha(70));
            painter.circle_stroke(pos, 9.0, Stroke::new(1.5_f32, Color32::from_black_alpha(160)));
            painter.circle_stroke(pos, 10.5, Stroke::new(1.0_f32, Color32::from_white_alpha(200)));
        }
    }
}

fn apply(op: ModifyOp, current: bool) -> bool {
    match op {
        ModifyOp::Add => true,
        ModifyOp::Remove => false,
        ModifyOp::Toggle => !current,
    }
}

/// Position of the pointer along a slider: horizontal sliders grow to the
/// right, vertical ones upwards.
fn slider_fraction(rect: lay::Rect, x: i32, y: i32) -> f32 {
    let fraction = if rect.w >= rect.h {
        (x - rect.x) as f32 / rect.w.max(1) as f32
    } else {
        (rect.bottom() - y) as f32 / rect.h.max(1) as f32
    };
    fraction.clamp(0.0, 1.0)
}

/// Position of the pointer along an arc from `start` to `end` degrees
/// (clockwise, 0° = right). Outside the arc it snaps to the nearer end.
fn arc_fraction(rect: lay::Rect, start: i32, end: i32, x: i32, y: i32) -> f32 {
    let (cx, cy) = (rect.x as f32 + rect.w as f32 / 2.0, rect.y as f32 + rect.h as f32 / 2.0);
    let angle = (y as f32 - cy).atan2(x as f32 - cx).to_degrees().rem_euclid(360.0);
    let mut span = (end - start).rem_euclid(360) as f32;
    if span == 0.0 {
        span = 360.0;
    }
    let relative = (angle - start as f32).rem_euclid(360.0);
    let relative = if relative <= span {
        relative
    } else if relative - span < 360.0 - relative {
        span
    } else {
        0.0
    };
    relative / span
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvglab_model::event::EventHandler;

    fn node(id: WidgetId, kind: WidgetKind, rect: lay::Rect) -> NodeInfo {
        NodeInfo {
            id,
            kind,
            rect,
            clip: lay::Rect::new(0, 0, 480, 320),
            children_area: rect,
            parent: None,
            hidden: false,
            movable: true,
        }
    }

    #[test]
    fn transitions_move_the_screens() {
        let t = Transition { from: WidgetId(1), anim: ScreenAnim::MoveLeft, start: 0.0, duration: 1.0 };
        let start = t.frame(0.0);
        assert_eq!((start.new_offset, start.old_offset), ((1.0, 0.0), (0.0, 0.0)));
        let end = t.frame(1.0);
        assert_eq!((end.new_offset, end.old_offset), ((0.0, 0.0), (-1.0, 0.0)));
        let fade = Transition { anim: ScreenAnim::FadeOn, ..t }.frame(1.0);
        assert_eq!(fade.new_opacity, 1.0);
        let out = Transition { anim: ScreenAnim::OutTop, ..t }.frame(0.5);
        assert!(!out.new_on_top && out.old_offset.1 < 0.0);
    }

    #[test]
    fn fractions() {
        let r = lay::Rect::new(0, 0, 100, 10);
        assert_eq!(slider_fraction(r, 50, 5), 0.5);
        assert_eq!(slider_fraction(r, -5, 5), 0.0);
        let square = lay::Rect::new(0, 0, 100, 100);
        // 135° → 45° (270° span, gap at the bottom): left is 45° into the arc,
        assert!((arc_fraction(square, 135, 45, 0, 50) - 45.0 / 270.0).abs() < 0.01);
        // and a point in the gap right of the bottom snaps to the end.
        assert_eq!(arc_fraction(square, 135, 45, 60, 99), 1.0);
        assert_eq!(arc_fraction(square, 135, 45, 40, 99), 0.0);
    }

    #[test]
    fn clicks_toggle_switches_and_run_events() {
        let mut project = Project::new("Sim");
        let screen = project.screens[0].id;
        let second = project.add_screen();
        let switch = project.new_widget(WidgetKind::Switch);
        let switch_id = switch.id;
        project.insert(screen, 0, switch).unwrap();
        let mut button = project.new_widget(WidgetKind::Button);
        button.events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::ModifyFlag { target: Some(switch_id), flag: ObjFlag::Hidden, op: ModifyOp::Toggle },
        });
        button.events.push(EventHandler {
            trigger: EventTrigger::Clicked,
            action: Action::ChangeScreen {
                screen: Some(second),
                anim: lvglab_model::event::ScreenAnim::None,
                time_ms: 0,
                delay_ms: 0,
            },
        });
        let button_id = button.id;
        project.insert(screen, 1, button).unwrap();
        let nodes = vec![
            node(screen, WidgetKind::Screen, lay::Rect::new(0, 0, 480, 320)),
            node(switch_id, WidgetKind::Switch, lay::Rect::new(10, 10, 50, 25)),
            node(button_id, WidgetKind::Button, lay::Rect::new(100, 100, 80, 40)),
        ];
        let mut sim = Simulation::new(&project, screen);
        sim.press(&nodes, 20, 20, 0.0);
        assert_eq!(sim.pressed(), Some(switch_id));
        sim.release(20, 20, Pos2::ZERO, 0.1);
        assert!(sim.project.widget(switch_id).unwrap().states.contains(&ObjState::Checked));
        assert!(!project.widget(switch_id).unwrap().states.contains(&ObjState::Checked), "document untouched");

        sim.press(&nodes, 120, 120, 1.0);
        sim.release(120, 120, Pos2::ZERO, 1.1);
        assert!(sim.project.widget(switch_id).unwrap().is_hidden());
        sim.tick(1.2);
        assert_eq!(sim.screen, second);
    }

    #[test]
    fn dragging_a_slider_sets_its_value() {
        let mut project = Project::new("Sim");
        let screen = project.screens[0].id;
        let slider = project.new_widget(WidgetKind::Slider);
        let id = slider.id;
        project.insert(screen, 0, slider).unwrap();
        let nodes = vec![
            node(screen, WidgetKind::Screen, lay::Rect::new(0, 0, 480, 320)),
            node(id, WidgetKind::Slider, lay::Rect::new(0, 100, 200, 10)),
        ];
        let mut sim = Simulation::new(&project, screen);
        sim.press(&nodes, 50, 105, 0.0);
        assert_eq!(sim.project.widget(id).unwrap().prop_int("value"), 25);
        sim.drag(150, 105, 0.1);
        sim.release(150, 105, Pos2::ZERO, 0.2);
        assert_eq!(sim.project.widget(id).unwrap().prop_int("value"), 75);
    }
}
