//! Events and the actions they trigger.

use serde::{Deserialize, Serialize};

use crate::flags::{ObjFlag, ObjState};
use crate::widget::WidgetId;

lv_enum! {
    /// Event that triggers an action. Gestures are filtered by direction.
    pub enum EventTrigger: "" {
        Clicked = "CLICKED", "Clicked";
        Pressed = "PRESSED", "Pressed";
        Pressing = "PRESSING", "Pressing";
        PressLost = "PRESS_LOST", "Press lost";
        ShortClicked = "SHORT_CLICKED", "Short clicked";
        LongPressed = "LONG_PRESSED", "Long pressed";
        LongPressedRepeat = "LONG_PRESSED_REPEAT", "Long pressed repeat";
        Released = "RELEASED", "Released";
        ValueChanged = "VALUE_CHANGED", "Value changed";
        Focused = "FOCUSED", "Focused";
        Defocused = "DEFOCUSED", "Defocused";
        Ready = "READY", "Ready";
        Cancel = "CANCEL", "Cancel";
        Checked = "CHECKED", "Checked";
        Unchecked = "UNCHECKED", "Unchecked";
        ScreenLoaded = "SCREEN_LOADED", "Screen loaded";
        ScreenUnloaded = "SCREEN_UNLOADED", "Screen unloaded";
        ScreenLoadStart = "SCREEN_LOAD_START", "Screen load start";
        ScreenUnloadStart = "SCREEN_UNLOAD_START", "Screen unload start";
        GestureLeft = "GESTURE_LEFT", "Gesture left";
        GestureRight = "GESTURE_RIGHT", "Gesture right";
        GestureTop = "GESTURE_TOP", "Gesture up";
        GestureBottom = "GESTURE_BOTTOM", "Gesture down";
    }
}

impl EventTrigger {
    /// C condition that is true inside an `lv_event_cb_t` for this trigger.
    pub fn c_condition(self) -> String {
        let gesture = |dir: &str| {
            format!("event_code == LV_EVENT_GESTURE && lv_indev_get_gesture_dir(lv_indev_get_act()) == LV_DIR_{dir}")
        };
        match self {
            Self::Checked => {
                "event_code == LV_EVENT_VALUE_CHANGED && lv_obj_has_state(target, LV_STATE_CHECKED)".into()
            }
            Self::Unchecked => {
                "event_code == LV_EVENT_VALUE_CHANGED && !lv_obj_has_state(target, LV_STATE_CHECKED)".into()
            }
            Self::GestureLeft => gesture("LEFT"),
            Self::GestureRight => gesture("RIGHT"),
            Self::GestureTop => gesture("TOP"),
            Self::GestureBottom => gesture("BOTTOM"),
            other => format!("event_code == LV_EVENT_{}", other.suffix()),
        }
    }

    pub fn is_gesture(self) -> bool {
        matches!(self, Self::GestureLeft | Self::GestureRight | Self::GestureTop | Self::GestureBottom)
    }
}

lv_enum! {
    /// `lv_scr_load_anim_t`.
    pub enum ScreenAnim: "LV_SCR_LOAD_ANIM_" {
        None = "NONE", "None";
        OverLeft = "OVER_LEFT", "Over left";
        OverRight = "OVER_RIGHT", "Over right";
        OverTop = "OVER_TOP", "Over top";
        OverBottom = "OVER_BOTTOM", "Over bottom";
        MoveLeft = "MOVE_LEFT", "Move left";
        MoveRight = "MOVE_RIGHT", "Move right";
        MoveTop = "MOVE_TOP", "Move top";
        MoveBottom = "MOVE_BOTTOM", "Move bottom";
        FadeOn = "FADE_ON", "Fade in";
        FadeOut = "FADE_OUT", "Fade out";
        OutLeft = "OUT_LEFT", "Out left";
        OutRight = "OUT_RIGHT", "Out right";
        OutTop = "OUT_TOP", "Out top";
        OutBottom = "OUT_BOTTOM", "Out bottom";
    }
}

lv_enum! {
    /// What to do with a flag or state.
    pub enum ModifyOp: "" {
        Add = "ADD", "Add";
        Remove = "REMOVE", "Remove";
        Toggle = "TOGGLE", "Toggle";
    }
}

/// An action executed when an event fires.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    ChangeScreen {
        screen: Option<WidgetId>,
        anim: ScreenAnim,
        time_ms: u32,
        delay_ms: u32,
    },
    /// Calls a user function declared in `ui_events.h`.
    CallFunction {
        name: String,
    },
    ModifyFlag {
        target: Option<WidgetId>,
        flag: ObjFlag,
        op: ModifyOp,
    },
    ModifyState {
        target: Option<WidgetId>,
        state: ObjState,
        op: ModifyOp,
    },
    /// Sets the text of a label.
    SetText {
        target: Option<WidgetId>,
        text: String,
    },
    /// Sets the value of a slider, bar or arc.
    SetValue {
        target: Option<WidgetId>,
        value: i32,
    },
}

impl Action {
    pub const KINDS: [&'static str; 6] =
        ["Change screen", "Call function", "Modify flag", "Modify state", "Set text", "Set value"];

    pub fn kind_index(&self) -> usize {
        match self {
            Self::ChangeScreen { .. } => 0,
            Self::CallFunction { .. } => 1,
            Self::ModifyFlag { .. } => 2,
            Self::ModifyState { .. } => 3,
            Self::SetText { .. } => 4,
            Self::SetValue { .. } => 5,
        }
    }

    /// A default action for the given kind index (see [`Action::KINDS`]).
    pub fn from_kind_index(index: usize) -> Self {
        match index {
            1 => Self::CallFunction { name: "on_event".into() },
            2 => Self::ModifyFlag { target: None, flag: ObjFlag::Hidden, op: ModifyOp::Toggle },
            3 => Self::ModifyState { target: None, state: ObjState::Checked, op: ModifyOp::Toggle },
            4 => Self::SetText { target: None, text: String::new() },
            5 => Self::SetValue { target: None, value: 0 },
            _ => Self::ChangeScreen { screen: None, anim: ScreenAnim::FadeOn, time_ms: 300, delay_ms: 0 },
        }
    }

    /// Widgets referenced by the action.
    pub fn references(&self) -> Option<WidgetId> {
        match self {
            Self::ChangeScreen { screen, .. } => *screen,
            Self::ModifyFlag { target, .. }
            | Self::ModifyState { target, .. }
            | Self::SetText { target, .. }
            | Self::SetValue { target, .. } => *target,
            Self::CallFunction { .. } => None,
        }
    }

    /// The widget reference of the action, to rewrite it.
    pub fn references_mut(&mut self) -> Option<&mut Option<WidgetId>> {
        match self {
            Self::ChangeScreen { screen, .. } => Some(screen),
            Self::ModifyFlag { target, .. }
            | Self::ModifyState { target, .. }
            | Self::SetText { target, .. }
            | Self::SetValue { target, .. } => Some(target),
            Self::CallFunction { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventHandler {
    pub trigger: EventTrigger,
    pub action: Action,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_conditions() {
        assert_eq!(EventTrigger::Clicked.c_condition(), "event_code == LV_EVENT_CLICKED");
        assert!(EventTrigger::GestureLeft.c_condition().contains("LV_DIR_LEFT"));
        assert!(EventTrigger::Checked.c_condition().contains("LV_STATE_CHECKED"));
    }

    #[test]
    fn action_kinds_round_trip() {
        for index in 0..Action::KINDS.len() {
            assert_eq!(Action::from_kind_index(index).kind_index(), index);
        }
    }

    #[test]
    fn action_serializes_tagged() {
        let action = Action::CallFunction { name: "beep".into() };
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, r#"{"action":"call_function","name":"beep"}"#);
    }
}
