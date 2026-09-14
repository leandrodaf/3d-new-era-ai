//! Formatting lengths and areas for display.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Unit system used to display (never to store) measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    #[default]
    Centimeter,
    Meter,
    Millimeter,
    /// Feet and inches.
    Imperial,
}

impl LengthUnit {
    pub const ALL: [Self; 4] = [
        Self::Centimeter,
        Self::Meter,
        Self::Millimeter,
        Self::Imperial,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Centimeter => "cm",
            Self::Meter => "m",
            Self::Millimeter => "mm",
            Self::Imperial => "ft/in",
        }
    }

    /// Formats a length given in centimeters, e.g. `352 cm`, `3.52 m`, `11'6½"`.
    pub fn format_length(self, cm: f64) -> String {
        match self {
            Self::Centimeter => format!("{} cm", trim(cm, 1)),
            Self::Meter => format!("{} m", trim(cm / 100.0, 2)),
            Self::Millimeter => format!("{} mm", trim(cm * 10.0, 0)),
            Self::Imperial => {
                let total_inches = cm / 2.54;
                let sign = if total_inches < 0.0 { "-" } else { "" };
                let eighths = (total_inches.abs() * 8.0).round();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let eighths = eighths as u64;
                let feet = eighths / (12 * 8);
                let inches = (eighths / 8) % 12;
                let frac = match eighths % 8 {
                    0 => "",
                    1 => "⅛",
                    2 => "¼",
                    3 => "⅜",
                    4 => "½",
                    5 => "⅝",
                    6 => "¾",
                    _ => "⅞",
                };
                if feet > 0 {
                    format!("{sign}{feet}'{inches}{frac}\"")
                } else {
                    format!("{sign}{inches}{frac}\"")
                }
            }
        }
    }

    /// The number written on a dimension line: no unit, as drawings state
    /// it once in a note ("cotas em cm"), with a decimal comma like Brazilian
    /// drawings and Sweet Home 3D in Portuguese: `434`, `65,5`, `3,52`.
    pub fn format_dimension(self, cm: f64) -> String {
        match self {
            Self::Centimeter => trim(cm, 1).replace('.', ","),
            Self::Meter => trim(cm / 100.0, 2).replace('.', ","),
            Self::Millimeter => trim(cm * 10.0, 0),
            Self::Imperial => self.format_length(cm),
        }
    }

    /// Formats a `width × depth × height` size with the unit written once,
    /// e.g. `210 × 90 × 85 cm`.
    pub fn format_size(self, size: [f64; 3]) -> String {
        if self == Self::Imperial {
            return size.map(|v| self.format_length(v)).join(" × ");
        }
        let suffix = format!(" {}", self.label());
        let numbers = size.map(|v| self.format_length(v).trim_end_matches(&suffix).to_owned());
        format!("{}{suffix}", numbers.join(" × "))
    }

    /// Formats an area given in square centimeters.
    pub fn format_area(self, cm2: f64) -> String {
        match self {
            Self::Imperial => format!("{} ft²", trim(cm2 / 929.0304, 1)),
            _ => format!("{} m²", trim(cm2 / 10_000.0, 2)),
        }
    }
}

/// Fixed decimals without trailing zeros: `3.50` → `3.5`, `4.00` → `4`.
fn trim(value: f64, decimals: usize) -> String {
    // Round half away from zero first; `format!` alone would round 12.25 down
    // because it is stored as 12.2499….
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let factor = 10f64.powi(decimals as i32);
    let value = (value * factor).round() / factor;
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        let text = text.trim_end_matches('0').trim_end_matches('.');
        if text == "-0" {
            "0".to_owned()
        } else {
            text.to_owned()
        }
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_every_unit() {
        assert_eq!(LengthUnit::Centimeter.format_length(352.0), "352 cm");
        assert_eq!(LengthUnit::Centimeter.format_length(12.25), "12.3 cm");
        assert_eq!(LengthUnit::Meter.format_length(352.0), "3.52 m");
        assert_eq!(LengthUnit::Meter.format_length(400.0), "4 m");
        assert_eq!(LengthUnit::Millimeter.format_length(1.5), "15 mm");
        assert_eq!(LengthUnit::Imperial.format_length(350.52), "11'6\"");
        assert_eq!(LengthUnit::Imperial.format_length(3.81), "1½\"");
        assert_eq!(LengthUnit::Meter.format_area(270_000.0), "27 m²");
        assert_eq!(
            LengthUnit::Centimeter.format_size([210.0, 90.0, 85.0]),
            "210 × 90 × 85 cm"
        );
        assert_eq!(
            LengthUnit::Meter.format_size([210.0, 90.0, 85.0]),
            "2.1 × 0.9 × 0.85 m"
        );
    }

    #[test]
    fn dimension_numbers_have_no_unit_and_a_decimal_comma() {
        assert_eq!(LengthUnit::Centimeter.format_dimension(434.0), "434");
        assert_eq!(LengthUnit::Centimeter.format_dimension(65.5), "65,5");
        assert_eq!(LengthUnit::Meter.format_dimension(352.0), "3,52");
        assert_eq!(LengthUnit::Millimeter.format_dimension(12.34), "123");
        assert_eq!(LengthUnit::Imperial.format_dimension(30.48), "1'0\"");
    }
}
