//! Test data for the simulator: values that move on their own, so gauges
//! and numeric labels can be checked without a device.

use lvglab_model::prop::PropValue;
use lvglab_model::{Project, Widget, WidgetId, WidgetKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveMode {
    Off,
    /// Every value follows its own slow sine wave.
    Wave,
    /// The widest values: maximum of every range, labels full of `8`.
    Maximum,
}

impl LiveMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::Wave, Self::Maximum];

    /// Untranslated label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Wave => "Wave",
            Self::Maximum => "Maximum",
        }
    }
}

/// A label whose text is a number, e.g. `12.5` or `-3`.
#[derive(Clone, Debug, PartialEq)]
struct NumericLabel {
    id: WidgetId,
    decimals: usize,
    int_digits: usize,
    negative: bool,
    comma: bool,
}

/// The values that are animated, collected when live data starts.
#[derive(Clone, Debug, Default)]
pub struct LiveData {
    values: Vec<WidgetId>,
    labels: Vec<NumericLabel>,
}

impl LiveData {
    pub fn collect(project: &Project) -> Self {
        let mut data = Self::default();
        for root in project.screens.iter() {
            root.walk(&mut |w| match w.kind {
                WidgetKind::Arc | WidgetKind::Bar | WidgetKind::Slider | WidgetKind::Meter => data.values.push(w.id),
                WidgetKind::Label => {
                    if let Some(label) = numeric(w) {
                        data.labels.push(label);
                    }
                }
                _ => {}
            });
        }
        data
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.labels.is_empty()
    }

    /// Writes the values for time `t` (seconds); `skip` is left alone
    /// (the widget the user is dragging).
    pub fn apply(&self, project: &mut Project, mode: LiveMode, t: f64, skip: Option<WidgetId>) {
        for id in &self.values {
            if Some(*id) == skip {
                continue;
            }
            let Some(widget) = project.widget_mut(*id) else { continue };
            let (min, max) = range(widget);
            let fraction = match mode {
                LiveMode::Maximum => 1.0,
                _ => wave(t, *id),
            };
            let value = min + ((max - min) as f64 * fraction).round() as i32;
            widget.set_prop("value", PropValue::Int(value));
        }
        for label in &self.labels {
            let Some(widget) = project.widget_mut(label.id) else { continue };
            let text = match mode {
                LiveMode::Maximum => widest(label),
                _ => {
                    let max = 10f64.powi(label.int_digits.max(2) as i32) - 1.0;
                    let mut value = max * wave(t, label.id);
                    if label.negative {
                        value -= max / 2.0;
                    }
                    let text = format!("{value:.*}", label.decimals);
                    if label.comma { text.replace('.', ",") } else { text }
                }
            };
            widget.set_prop("text", PropValue::Text(text));
        }
    }
}

fn range(widget: &Widget) -> (i32, i32) {
    let (min, max) = widget.prop_pair("range");
    if max > min { (min, max) } else { (0, 100) }
}

/// 0..1, a slow sine with a phase per widget so values do not move in step.
fn wave(t: f64, id: WidgetId) -> f64 {
    let phase = (id.0 % 97) as f64 * 0.7;
    let speed = 0.35 + (id.0 % 5) as f64 * 0.08;
    0.5 + 0.5 * (t * speed + phase).sin()
}

fn numeric(widget: &Widget) -> Option<NumericLabel> {
    let text = widget.prop_text("text");
    let text = text.trim();
    let negative = text.starts_with('-');
    let digits = text.trim_start_matches(['-', '+']);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        return None;
    }
    let comma = digits.contains(',');
    let mut parts = digits.split(['.', ',']);
    let int_part = parts.next().unwrap_or_default();
    let decimals = parts.next().map_or(0, str::len);
    if int_part.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(NumericLabel { id: widget.id, decimals, int_digits: int_part.len(), negative, comma })
}

/// The widest text a value of this label can take.
fn widest(label: &NumericLabel) -> String {
    let int = "8".repeat(label.int_digits.max(4));
    let sign = if label.negative { "-" } else { "" };
    let separator = if label.comma { "," } else { "." };
    if label.decimals == 0 {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}{separator}{}", "8".repeat(label.decimals))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_labels_and_values_move() {
        let mut project = Project::new("Live");
        let screen = project.screens[0].id;
        let mut label = project.new_widget(WidgetKind::Label);
        label.set_prop("text", PropValue::Text("12,5".into()));
        let label_id = label.id;
        project.insert(screen, 0, label).unwrap();
        let mut name = project.new_widget(WidgetKind::Label);
        name.set_prop("text", PropValue::Text("Oil".into()));
        project.insert(screen, 1, name).unwrap();
        let arc = project.new_widget(WidgetKind::Arc);
        let arc_id = arc.id;
        project.insert(screen, 2, arc).unwrap();

        let live = LiveData::collect(&project);
        assert_eq!((live.values.len(), live.labels.len()), (1, 1));
        live.apply(&mut project, LiveMode::Maximum, 0.0, None);
        assert_eq!(project.widget(label_id).unwrap().prop_text("text"), "8888,8");
        assert_eq!(project.widget(arc_id).unwrap().prop_int("value"), 100);
        live.apply(&mut project, LiveMode::Wave, 3.0, None);
        let text = project.widget(label_id).unwrap().prop_text("text");
        assert!(text.contains(',') && text.split(',').nth(1).unwrap().len() == 1, "{text}");
    }
}
