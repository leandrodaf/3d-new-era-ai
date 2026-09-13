use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

/// A point on the floor plan, in centimeters.
///
/// Serialized as a compact `[x, y]` pair: floor plans are point-heavy and
/// every byte counts when an AI agent reads or writes them.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(from = "[f64; 2]", into = "[f64; 2]")]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl From<[f64; 2]> for Point2 {
    fn from([x, y]: [f64; 2]) -> Self {
        Self { x, y }
    }
}

impl From<Point2> for [f64; 2] {
    fn from(p: Point2) -> Self {
        [p.x, p.y]
    }
}

impl JsonSchema for Point2 {
    fn schema_name() -> Cow<'static, str> {
        "Point2".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = <[f64; 2]>::json_schema(generator);
        schema.insert(
            "description".into(),
            "[x, y] in centimeters; x grows right, y grows down".into(),
        );
        schema
    }
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Distance from `self` to the segment `a`–`b`.
    pub fn distance_to_segment(self, a: Self, b: Self) -> f64 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx.mul_add(dx, dy * dy);
        if len_sq <= f64::EPSILON {
            return self.distance(a);
        }
        let t = (((self.x - a.x) * dx + (self.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
        self.distance(Self::new(a.x + t * dx, a.y + t * dy))
    }
}

/// Area of a simple polygon using the shoelace formula (always positive).
pub(crate) fn polygon_area(points: &[Point2]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let twice_area: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(p, q)| p.x * q.y - q.x * p.y)
        .sum();
    twice_area.abs() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_to_segment_projects_inside_and_clamps_outside() {
        let (a, b) = (Point2::new(0.0, 0.0), Point2::new(100.0, 0.0));
        assert!((Point2::new(50.0, 30.0).distance_to_segment(a, b) - 30.0).abs() < 1e-9);
        assert!((Point2::new(-40.0, 30.0).distance_to_segment(a, b) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn polygon_area_of_rectangle() {
        let rect = [
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 300.0),
            Point2::new(0.0, 300.0),
        ];
        assert!((polygon_area(&rect) - 120_000.0).abs() < 1e-9);
    }
}
