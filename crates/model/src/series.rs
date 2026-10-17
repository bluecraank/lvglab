//! Chart series in their text form: one series per line,
//! `#RRGGBB: v1, v2, ...` or `#RRGGBB secondary: v1, v2, ...`.

use crate::color::{Color, Palette};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Series {
    pub color: Color,
    /// Plotted against the secondary Y axis.
    pub secondary: bool,
    pub values: Vec<i32>,
}

impl Series {
    pub fn to_line(&self) -> String {
        let values: Vec<String> = self.values.iter().map(i32::to_string).collect();
        let axis = if self.secondary { " secondary" } else { "" };
        format!("{}{axis}: {}", self.color.to_hex_string(), values.join(", "))
    }
}

/// Parses all non-empty lines; unparsable values are skipped.
pub fn parse(text: &str) -> Vec<Series> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| {
            let (head, values) = line.split_once(':').unwrap_or(("", line));
            let mut words = head.split_whitespace();
            let color = words.next().and_then(Color::parse).unwrap_or(Palette::Blue.main());
            let secondary = words.any(|w| w.eq_ignore_ascii_case("secondary"));
            let values = values.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            Series { color, secondary, values }
        })
        .collect()
}

pub fn format(series: &[Series]) -> String {
    series.iter().map(Series::to_line).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let text = "#FF0000: 1, 2, 3\n#00FF00 secondary: 4";
        let series = parse(text);
        assert_eq!(series.len(), 2);
        assert!(series[1].secondary);
        assert_eq!(series[0].values, vec![1, 2, 3]);
        assert_eq!(format(&series), text);
    }

    #[test]
    fn tolerates_missing_color() {
        let series = parse("5, x, 7");
        assert_eq!(series[0].values, vec![5, 7]);
        assert_eq!(series[0].color, Palette::Blue.main());
    }
}
