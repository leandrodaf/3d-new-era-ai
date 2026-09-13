//! Magnetism: snapping points to wall ends, angles, lengths and the grid.

use newera_core::{Home, Point2};

/// Grid step (cm) that feels right at the current zoom (px per cm).
pub(crate) fn round_step(zoom: f32) -> f64 {
    match zoom {
        z if z >= 4.0 => 1.0,
        z if z >= 0.8 => 5.0,
        z if z >= 0.2 => 10.0,
        _ => 50.0,
    }
}

fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// Nearest wall endpoint within `radius_cm`, ignoring `exclude` points.
pub(crate) fn endpoint_near(
    home: &Home,
    p: Point2,
    radius_cm: f64,
    exclude: &[Point2],
) -> Option<Point2> {
    home.walls
        .iter()
        .flat_map(|w| [w.start, w.end])
        .chain(home.rooms.iter().flat_map(|r| r.points.iter().copied()))
        .filter(|q| !exclude.iter().any(|e| e.distance(*q) < 1e-6))
        .filter(|q| q.distance(p) <= radius_cm)
        .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
}

/// Snaps a free point: wall/room points first, then the grid.
pub(crate) fn snap_point(home: &Home, p: Point2, zoom: f32, exclude: &[Point2]) -> Point2 {
    let radius = f64::from(10.0 / zoom);
    endpoint_near(home, p, radius, exclude).unwrap_or_else(|| {
        let step = round_step(zoom);
        Point2::new(round_to(p.x, step), round_to(p.y, step))
    })
}

/// Snaps the end of a segment that starts at `anchor`: endpoints first, then
/// 15° angle steps with a rounded length.
pub(crate) fn snap_segment(
    home: &Home,
    anchor: Point2,
    p: Point2,
    zoom: f32,
    exclude: &[Point2],
) -> Point2 {
    let radius = f64::from(10.0 / zoom);
    if let Some(q) = endpoint_near(home, p, radius, exclude) {
        return q;
    }
    let (dx, dy) = (p.x - anchor.x, p.y - anchor.y);
    let length = round_to(dx.hypot(dy), round_step(zoom));
    let angle = round_to(dy.atan2(dx).to_degrees(), 15.0).to_radians();
    Point2::new(
        anchor.x + length * angle.cos(),
        anchor.y + length * angle.sin(),
    )
}

/// Point at `length` cm from `anchor` in the direction of `toward`.
pub(crate) fn along(anchor: Point2, toward: Point2, length: f64) -> Point2 {
    let d = anchor.distance(toward);
    if d < 1e-9 {
        return Point2::new(anchor.x + length, anchor.y);
    }
    Point2::new(
        anchor.x + (toward.x - anchor.x) / d * length,
        anchor.y + (toward.y - anchor.y) / d * length,
    )
}

#[cfg(test)]
mod tests {
    use newera_core::{Wall, WallId};

    use super::*;

    #[test]
    fn segments_snap_to_15_degrees_and_round_lengths() {
        let home = Home::default();
        let p = snap_segment(
            &home,
            Point2::new(0.0, 0.0),
            Point2::new(301.0, 12.0),
            1.0,
            &[],
        );
        assert!((p.y).abs() < 1e-9, "{p:?}");
        assert!((p.x - 300.0).abs() < 1e-9, "{p:?}");
        let diag = snap_segment(
            &home,
            Point2::new(0.0, 0.0),
            Point2::new(100.0, 97.0),
            1.0,
            &[],
        );
        assert!((diag.x - diag.y).abs() < 1e-9, "45°: {diag:?}");
    }

    #[test]
    fn endpoints_win_over_the_grid() {
        let mut home = Home::default();
        home.walls.push(Wall::new(
            WallId(1),
            Point2::new(0.0, 0.0),
            Point2::new(103.0, 0.0),
        ));
        let p = snap_point(&home, Point2::new(106.0, 3.0), 1.0, &[]);
        assert_eq!(p, Point2::new(103.0, 0.0));
        let excluded = snap_point(
            &home,
            Point2::new(106.0, 3.0),
            1.0,
            &[Point2::new(103.0, 0.0)],
        );
        assert_eq!(excluded, Point2::new(105.0, 5.0));
    }
}
