//! Random but plausible values for the widgets of a project, so that
//! screenshots do not show zeros and default values everywhere: arcs, bars,
//! sliders and meters move, switches and check boxes toggle, lists select an
//! entry, charts get new data, and numbers in labels and tables change.

use crate::flags::ObjState;
use crate::project::Project;
use crate::prop::PropValue;
use crate::series;
use crate::widget::{Widget, WidgetId, WidgetKind};

/// xorshift64*: small and reproducible for a seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `0.0..1.0`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A value of `min..=max`, kept away from the very ends so gauges look used.
    fn within(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = f64::from(max) - f64::from(min);
        (f64::from(min) + span * (0.12 + 0.78 * self.unit())).round() as i32
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// A number as written in a label: `-12`, `3.5`, `0.00`.
struct Number {
    negative: bool,
    int_digits: usize,
    decimals: usize,
    comma: bool,
}

fn parse_number(text: &str) -> Option<Number> {
    let t = text.trim();
    let digits = t.strip_prefix('-').unwrap_or(t);
    let (int, frac, comma) = match digits.split_once(['.', ',']) {
        Some((i, f)) => (i, f, digits.contains(',')),
        None => (digits, "", false),
    };
    let numeric = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if int.is_empty() || !numeric(int) || !numeric(frac) || (digits.contains(['.', ',']) && frac.is_empty()) {
        return None;
    }
    Some(Number { negative: t.starts_with('-'), int_digits: int.len(), decimals: frac.len(), comma })
}

/// A random number written like `like`. Single digits become up to three
/// digits, longer numbers keep their length.
fn random_number(rng: &mut Rng, like: &Number) -> String {
    let digits = if like.int_digits == 1 { 1 + rng.below(3) } else { like.int_digits };
    let low = if digits == 1 { 0 } else { 10_i64.pow(digits as u32 - 1) };
    let high = 10_i64.pow(digits as u32) - 1;
    let value = low + (rng.next() % (high - low + 1) as u64) as i64;
    let mut text = value.to_string();
    if like.decimals > 0 {
        let fraction: String = (0..like.decimals).map(|_| char::from(b'0' + rng.below(10) as u8)).collect();
        text.push(if like.comma { ',' } else { '.' });
        text.push_str(&fraction);
    }
    if like.negative && rng.chance(50) {
        text.insert(0, '-');
    }
    text
}

fn randomize_widget(w: &mut Widget, rng: &mut Rng) -> bool {
    let before = (w.props.clone(), w.states.clone());
    match w.kind {
        WidgetKind::Arc | WidgetKind::Bar | WidgetKind::Slider | WidgetKind::Meter => {
            let (min, max) = w.prop_pair("range");
            let key = "value";
            w.set_prop(key, PropValue::Int(rng.within(min, max)));
        }
        WidgetKind::Led => {
            w.set_prop("brightness", PropValue::Int(rng.within(60, 255)));
        }
        WidgetKind::Switch | WidgetKind::Checkbox => {
            if rng.chance(50) {
                w.states.insert(ObjState::Checked);
            } else {
                w.states.remove(&ObjState::Checked);
            }
        }
        WidgetKind::Dropdown | WidgetKind::Roller => {
            let count = w.prop_text("options").lines().filter(|l| !l.trim().is_empty()).count();
            if count > 0 {
                w.set_prop("selected", PropValue::Int(rng.below(count) as i32));
            }
        }
        WidgetKind::Spinbox => {
            let (min, max) = w.prop_pair("range");
            let limit = 10_i32.saturating_pow(w.prop_int("digit_count").clamp(1, 9) as u32) - 1;
            w.set_prop("value", PropValue::Int(rng.within(min.max(0), max.min(limit))));
        }
        WidgetKind::Chart => {
            let count = w.prop_int("point_count").clamp(1, 200) as usize;
            let mut lines = series::parse(&w.prop_text("series"));
            for line in &mut lines {
                let (min, max) = w.prop_pair(if line.secondary { "y2_range" } else { "y_range" });
                let span = f64::from(max) - f64::from(min);
                // A smooth random walk, so the curve looks like measured data.
                let mut value = f64::from(min) + span * (0.3 + 0.4 * rng.unit());
                line.values = (0..count)
                    .map(|_| {
                        value = (value + span * (rng.unit() - 0.5) * 0.25).clamp(f64::from(min), f64::from(max));
                        value.round() as i32
                    })
                    .collect();
            }
            w.set_prop("series", PropValue::Text(series::format(&lines)));
        }
        WidgetKind::Label => {
            let text = w.prop_text("text");
            if let Some(number) = parse_number(&text) {
                w.set_prop("text", PropValue::Text(random_number(rng, &number)));
            }
        }
        WidgetKind::Table => {
            let cells = w.prop_text("cells");
            let changed: Vec<String> = cells
                .lines()
                .map(|line| {
                    line.split('|')
                        .map(|cell| match parse_number(cell) {
                            Some(number) => random_number(rng, &number),
                            None => cell.to_owned(),
                        })
                        .collect::<Vec<_>>()
                        .join("|")
                })
                .collect();
            w.set_prop("cells", PropValue::Text(changed.join("\n")));
        }
        _ => {}
    }
    (w.props.clone(), w.states.clone()) != before
}

/// Randomizes the values of the widgets of `screens` (all screens if empty).
/// Returns the number of widgets that changed.
pub fn randomize(project: &mut Project, screens: &[WidgetId], seed: u64) -> usize {
    let mut rng = Rng::new(seed);
    let mut changed = 0;
    for screen in project.screens.iter_mut().filter(|s| screens.is_empty() || screens.contains(&s.id)) {
        screen.walk_mut(&mut |w| {
            if randomize_widget(w, &mut rng) {
                changed += 1;
            }
        });
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::PropValue as V;

    fn project() -> (Project, WidgetId, WidgetId) {
        let mut p = Project::new("P");
        let screen = p.screens[0].id;
        let second = p.add_screen();
        let mut add = |p: &mut Project, parent: WidgetId, kind: WidgetKind, f: &dyn Fn(&mut Widget)| {
            let mut w = p.new_widget(kind);
            f(&mut w);
            p.insert(parent, usize::MAX, w).unwrap();
        };
        add(&mut p, screen, WidgetKind::Arc, &|w| w.set_prop("range", V::Pair(0, 240)));
        add(&mut p, screen, WidgetKind::Label, &|w| w.set_prop("text", V::Text("0.0".into())));
        add(&mut p, screen, WidgetKind::Label, &|w| w.set_prop("text", V::Text("km/h".into())));
        add(&mut p, screen, WidgetKind::Dropdown, &|w| w.set_prop("options", V::Text("a\nb\nc".into())));
        add(&mut p, screen, WidgetKind::Chart, &|w| {
            w.set_prop("series", V::Text("#FF0000: 1, 2\n#00FF00 secondary: 3".into()))
        });
        add(&mut p, second, WidgetKind::Slider, &|w| w.set_prop("range", V::Pair(10, 20)));
        (p, screen, second)
    }

    #[test]
    fn values_change_within_their_ranges_and_text_stays_text() {
        let (mut p, screen, second) = project();
        let before = p.clone();
        let changed = randomize(&mut p, &[screen], 7);
        assert!(changed >= 3, "{changed}");
        let widgets = &p.screens[0].children;
        let arc = widgets.iter().find(|w| w.kind == WidgetKind::Arc).unwrap();
        assert!((0..=240).contains(&arc.prop_int("value")));
        let labels: Vec<String> =
            widgets.iter().filter(|w| w.kind == WidgetKind::Label).map(|w| w.prop_text("text")).collect();
        assert!(parse_number(&labels[0]).is_some_and(|n| n.decimals == 1), "{labels:?}");
        assert_eq!(labels[1], "km/h", "units are not numbers");
        let chart = widgets.iter().find(|w| w.kind == WidgetKind::Chart).unwrap();
        let lines = series::parse(&chart.prop_text("series"));
        assert_eq!(lines.len(), 2);
        assert!(lines[1].secondary && lines[0].color != lines[1].color, "series keep their color and axis");
        assert_eq!(lines[0].values.len(), chart.prop_int("point_count") as usize);
        // The other screen was not touched.
        assert_eq!(p.widget(second), before.widget(second));
    }

    #[test]
    fn the_same_seed_gives_the_same_values_and_another_seed_other_ones() {
        let (mut a, ..) = project();
        let mut b = a.clone();
        let mut c = a.clone();
        randomize(&mut a, &[], 1);
        randomize(&mut b, &[], 1);
        randomize(&mut c, &[], 2);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn numbers_keep_their_shape() {
        let mut rng = Rng::new(3);
        for text in ["0", "5", "42", "-7", "0.00", "12,5"] {
            let like = parse_number(text).unwrap();
            for _ in 0..50 {
                let random = random_number(&mut rng, &like);
                let parsed = parse_number(&random).unwrap_or_else(|| panic!("{random} from {text}"));
                assert_eq!(parsed.decimals, like.decimals, "{random}");
                assert_eq!(parsed.comma, like.comma, "{random}");
                if like.int_digits > 1 {
                    assert_eq!(parsed.int_digits, like.int_digits, "{random}");
                }
            }
        }
        for text in ["", "km/h", "1.", "a1", "1-2", "°C"] {
            assert!(parse_number(text).is_none(), "{text}");
        }
    }
}
