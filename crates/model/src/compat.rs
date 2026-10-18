//! What an LVGL release supports. The editor hides the rest, and switching
//! the release of a project removes it (see [`Project::remove_unsupported`]).

use crate::catalog::StyleSlot;
use crate::project::{LvglVersion, Project};
use crate::style::{Part, StyleProp, StyleValue};
use crate::widget::{Widget, WidgetKind};

pub fn widget_supported(version: LvglVersion, kind: WidgetKind) -> bool {
    !(version.is_v9() && kind.def().v9.removed)
}

/// False for properties a widget does not have in `version`.
pub fn prop_supported(version: LvglVersion, kind: WidgetKind, key: &str) -> bool {
    if !version.is_v9() {
        return true;
    }
    match kind.def().v9.prop(key) {
        Some(v9) => v9.emit != crate::prop::Emit::None && version.major_minor().1 >= v9.since,
        None => true,
    }
}

/// LVGL 9 has no ticks part; only the meter keeps it (its scale's items).
pub fn slot_supported(version: LvglVersion, kind: WidgetKind, slot: &StyleSlot) -> bool {
    !(version.is_v9() && slot.part == Part::Ticks && kind != WidgetKind::Meter)
}

pub fn style_supported(version: LvglVersion, prop: StyleProp) -> bool {
    !(version.is_v9() && matches!(prop, StyleProp::AnimSpeed | StyleProp::BgDitherMode))
}

/// Style values `version` lacks, e.g. the blend mode "replace" in LVGL 9.
pub fn style_value_supported(version: LvglVersion, value: &StyleValue) -> bool {
    !(version.is_v9() && matches!(value, StyleValue::Enum(e) if e == "LV_BLEND_MODE_REPLACE"))
}

/// Something [`Project::remove_unsupported`] removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Removed {
    Widget {
        name: String,
        kind: WidgetKind,
    },
    /// A property was reset to its default.
    Prop {
        widget: String,
        label: &'static str,
    },
    Style {
        widget: String,
        prop: StyleProp,
    },
    /// All styles of a part.
    Part {
        widget: String,
        label: &'static str,
    },
}

/// English description for import and assistant reports.
impl std::fmt::Display for Removed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Widget { name, kind } => {
                write!(f, "{name}: the {} widget does not exist in this LVGL release and was removed", kind.def().name)
            }
            Self::Prop { widget, label } => {
                write!(f, "{widget}: {label} is not available in this LVGL release and was reset")
            }
            Self::Style { widget, prop } => {
                write!(f, "{widget}: style {} is not available in this LVGL release and was removed", prop.name())
            }
            Self::Part { widget, label } => {
                write!(f, "{widget}: the {label} part does not exist in this LVGL release; its styles were removed")
            }
        }
    }
}

impl Project {
    /// Removes widgets, properties and styles the project's LVGL release
    /// does not support, e.g. after the release was changed.
    pub fn remove_unsupported(&mut self) -> Vec<Removed> {
        let version = self.lvgl_version;
        let mut removed = Vec::new();
        let mut gone = Vec::new();
        for root in self.roots() {
            root.walk(&mut |w| {
                if !widget_supported(version, w.kind) {
                    gone.push(w.id);
                    removed.push(Removed::Widget { name: w.name.clone(), kind: w.kind });
                }
            });
        }
        for id in gone {
            self.remove(id);
        }
        for root in self.roots_mut() {
            root.walk_mut(&mut |w| clean_widget(version, w, &mut removed));
        }
        removed
    }
}

fn clean_widget(version: LvglVersion, widget: &mut Widget, removed: &mut Vec<Removed>) {
    let def = widget.def();
    for prop in def.props.iter().filter(|p| !prop_supported(version, widget.kind, p.key)) {
        if widget.props.remove(prop.key).is_some_and(|value| value != prop.default_value()) {
            removed.push(Removed::Prop { widget: widget.name.clone(), label: prop.label });
        }
    }
    for slot in def.slots.iter().filter(|s| !slot_supported(version, widget.kind, s)) {
        let states = widget.styles.states_of(slot.key);
        if states.is_empty() {
            continue;
        }
        for state in states {
            for prop in StyleProp::ALL.iter().copied() {
                widget.styles.remove(slot.key, state, prop);
            }
        }
        removed.push(Removed::Part { widget: widget.name.clone(), label: slot.label });
    }
    let mut styles = Vec::new();
    for block in widget.styles.blocks() {
        for (prop, value) in &block.props {
            if !style_supported(version, *prop) || !style_value_supported(version, value) {
                styles.push((block.slot.clone(), block.state, *prop));
            }
        }
    }
    for (slot, state, prop) in styles {
        widget.styles.remove(&slot, state, prop);
        let entry = Removed::Style { widget: widget.name.clone(), prop };
        if !removed.contains(&entry) {
            removed.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::StateMask;
    use crate::prop::PropValue;

    #[test]
    fn switching_to_lvgl_9_removes_what_it_lacks() {
        let mut project = Project::new("P");
        let screen = project.screens[0].id;
        let wheel = project.new_widget(WidgetKind::Colorwheel);
        let mut chart = project.new_widget(WidgetKind::Chart);
        chart.set_prop("zoom", PropValue::Pair(512, 256));
        chart.styles.set("ticks", StateMask::DEFAULT, StyleProp::LineWidth, StyleValue::Int(2));
        let mut label = project.new_widget(WidgetKind::Label);
        label.set_prop("recolor", PropValue::Bool(true));
        label.styles.set("main", StateMask::DEFAULT, StyleProp::AnimSpeed, StyleValue::Int(10));
        label.styles.set("main", StateMask::DEFAULT, StyleProp::Radius, StyleValue::Int(3));
        let meter = project.new_widget(WidgetKind::Meter);
        for widget in [wheel, chart, label, meter] {
            project.insert(screen, usize::MAX, widget).unwrap();
        }
        assert!(project.remove_unsupported().is_empty(), "LVGL 8 supports everything");

        project.lvgl_version = LvglVersion::V9_2_2;
        let removed = project.remove_unsupported();
        assert!(removed.contains(&Removed::Widget { name: "Colorwheel1".into(), kind: WidgetKind::Colorwheel }));
        assert!(removed.contains(&Removed::Prop { widget: "Chart1".into(), label: "Zoom (256 = 100%)" }));
        assert!(removed.contains(&Removed::Part { widget: "Chart1".into(), label: "Ticks" }));
        assert!(removed.contains(&Removed::Prop { widget: "Label1".into(), label: "Recolor" }));
        assert!(removed.contains(&Removed::Style { widget: "Label1".into(), prop: StyleProp::AnimSpeed }));
        assert_eq!(removed.len(), 5);
        let screen = &project.screens[0];
        assert!(screen.children.iter().all(|w| w.kind != WidgetKind::Colorwheel));
        assert!(screen.children.iter().any(|w| w.kind == WidgetKind::Meter), "the meter becomes a scale");
        let label = screen.children.iter().find(|w| w.kind == WidgetKind::Label).unwrap();
        assert_eq!(label.styles.get("main", StateMask::DEFAULT, StyleProp::Radius), Some(&StyleValue::Int(3)));
        assert!(project.remove_unsupported().is_empty());
    }

    #[test]
    fn label_recolor_returns_with_lvgl_9_3() {
        assert!(!prop_supported(LvglVersion::V9_2_2, WidgetKind::Label, "recolor"));
        assert!(prop_supported(LvglVersion::V9_3_0, WidgetKind::Label, "recolor"));
        assert!(prop_supported(LvglVersion::V8_3_11, WidgetKind::Label, "recolor"));
    }
}
