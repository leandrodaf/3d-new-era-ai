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

/// Signed shoelace area. Positive means counter-clockwise in math axes
/// (y up), which is clockwise on screen, where y grows down.
pub fn signed_area(points: &[Point2]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let twice_area: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(p, q)| p.x * q.y - q.x * p.y)
        .sum();
    twice_area / 2.0
}

/// A closed `geo` polygon from an open ring of plan points.
///
/// Every geometric check in the crate starts here, so the closing vertex is
/// added in exactly one place.
pub fn to_polygon(points: &[Point2]) -> geo::Polygon<f64> {
    let mut coords: Vec<geo::Coord<f64>> = points
        .iter()
        .map(|p| geo::Coord { x: p.x, y: p.y })
        .collect();
    if let Some(first) = coords.first().copied() {
        coords.push(first);
    }
    geo::Polygon::new(geo::LineString::new(coords), vec![])
}

/// Area of a simple polygon (always positive).
pub fn polygon_area(points: &[Point2]) -> f64 {
    signed_area(points).abs()
}

/// Area centroid of a simple polygon; falls back to the vertex average for
/// degenerate polygons.
pub fn polygon_centroid(points: &[Point2]) -> Option<Point2> {
    if points.is_empty() {
        return None;
    }
    let area = signed_area(points);
    if area.abs() < f64::EPSILON {
        #[allow(clippy::cast_precision_loss)]
        let n = points.len() as f64;
        let (sx, sy) = points
            .iter()
            .fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
        return Some(Point2::new(sx / n, sy / n));
    }
    let (cx, cy) =
        points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .fold((0.0, 0.0), |(cx, cy), (p, q)| {
                let cross = p.x * q.y - q.x * p.y;
                (cx + (p.x + q.x) * cross, cy + (p.y + q.y) * cross)
            });
    Some(Point2::new(cx / (6.0 * area), cy / (6.0 * area)))
}

/// Twice the signed area of triangle `a b c`.
fn orient(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Triangulates a simple polygon (convex or concave) by ear clipping.
///
/// Returns index triples into `points` that keep the polygon's own winding,
/// so callers control which way the faces point by choosing the input order.
/// Collinear vertices are skipped. Self-intersecting input cannot be
/// triangulated exactly; whatever remains after clipping falls back to a fan.
pub fn triangulate(points: &[Point2]) -> Vec<[usize; 3]> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    let flipped = signed_area(points) < 0.0;
    // Work in positive winding, so convex corners have a positive orientation.
    let mut remaining: Vec<usize> = if flipped {
        (0..n).rev().collect()
    } else {
        (0..n).collect()
    };
    let scale = points
        .iter()
        .map(|p| p.x.abs().max(p.y.abs()))
        .fold(1.0, f64::max);
    let eps = scale * scale * 1e-12;

    let mut triangles = Vec::with_capacity(n - 2);
    while remaining.len() > 3 {
        let m = remaining.len();
        let ear = (0..m).find_map(|i| {
            let (ia, ib, ic) = (
                remaining[(i + m - 1) % m],
                remaining[i],
                remaining[(i + 1) % m],
            );
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            let area = orient(a, b, c);
            if area.abs() <= eps {
                return Some((i, None)); // collinear: drop the vertex, no triangle
            }
            if area < 0.0 {
                return None; // reflex corner
            }
            let blocked = remaining.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let p = points[j];
                p != a
                    && p != b
                    && p != c
                    && orient(a, b, p) >= -eps
                    && orient(b, c, p) >= -eps
                    && orient(c, a, p) >= -eps
            });
            (!blocked).then_some((i, Some([ia, ib, ic])))
        });
        match ear {
            Some((i, triangle)) => {
                triangles.extend(triangle);
                remaining.remove(i);
            }
            None => break,
        }
    }
    for i in 1..remaining.len().saturating_sub(1) {
        let (ia, ib, ic) = (remaining[0], remaining[i], remaining[i + 1]);
        if orient(points[ia], points[ib], points[ic]).abs() > eps {
            triangles.push([ia, ib, ic]);
        }
    }
    if flipped {
        for t in &mut triangles {
            t.swap(1, 2);
        }
    }
    triangles
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

    fn pts(raw: &[(f64, f64)]) -> Vec<Point2> {
        raw.iter().map(|&(x, y)| Point2::new(x, y)).collect()
    }

    /// Triangles must cover the polygon exactly and keep its winding.
    fn assert_valid_triangulation(points: &[Point2]) {
        let triangles = triangulate(points);
        let expected_sign = signed_area(points).signum();
        let mut total = 0.0;
        for [a, b, c] in &triangles {
            let area = orient(points[*a], points[*b], points[*c]) / 2.0;
            assert!(area * expected_sign > 0.0, "triangle {a},{b},{c} flipped");
            total += area.abs();
        }
        assert!(
            (total - polygon_area(points)).abs() < 1e-6,
            "triangles cover {total}, polygon is {}",
            polygon_area(points)
        );
    }

    #[test]
    fn triangulates_concave_l_shape_in_both_windings() {
        let l = pts(&[
            (0.0, 0.0),
            (600.0, 0.0),
            (600.0, 300.0),
            (300.0, 300.0),
            (300.0, 600.0),
            (0.0, 600.0),
        ]);
        assert_valid_triangulation(&l);
        let reversed: Vec<Point2> = l.iter().rev().copied().collect();
        assert_valid_triangulation(&reversed);
    }

    #[test]
    fn triangulates_u_shape_and_skips_collinear_points() {
        let u = pts(&[
            (0.0, 0.0),
            (100.0, 0.0),
            (100.0, 200.0),
            (200.0, 200.0),
            (200.0, 0.0),
            (300.0, 0.0),
            (300.0, 300.0),
            (150.0, 300.0),
            (0.0, 300.0),
        ]);
        assert_valid_triangulation(&u);
    }

    #[test]
    fn centroid_of_rectangle_is_its_center() {
        let c = polygon_centroid(&pts(&[
            (0.0, 0.0),
            (400.0, 0.0),
            (400.0, 200.0),
            (0.0, 200.0),
        ]))
        .unwrap();
        assert!((c.x - 200.0).abs() < 1e-9 && (c.y - 100.0).abs() < 1e-9);
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
