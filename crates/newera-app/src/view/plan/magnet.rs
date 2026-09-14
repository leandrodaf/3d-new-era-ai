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

/// What a measuring point locked onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapKind {
    /// A corner of a wall face (inside or outside), an opening edge or a
    /// room/furniture corner.
    Corner,
    /// A point on a wall face.
    Face,
    /// A wall axis end.
    Axis,
    /// Grid, angle or length rounding.
    Free,
}

/// Snaps a point for measuring: the nearest corner of any wall face (inner
/// and outer), opening edge, wall axis end, room or furniture corner wins;
/// otherwise the nearest wall face; otherwise angle/length rounding from
/// `anchor` (or the grid). Radii are in screen pixels, so precision holds at
/// every zoom.
pub(crate) fn snap_measure(
    home: &Home,
    outlines: &[Vec<Point2>],
    p: Point2,
    zoom: f32,
    anchor: Option<Point2>,
) -> (Point2, SnapKind) {
    let corner_radius = f64::from(12.0 / zoom);
    let face_radius = f64::from(8.0 / zoom);
    let nearest = |candidates: &mut dyn Iterator<Item = (Point2, SnapKind)>| {
        candidates
            .filter(|(q, _)| q.distance(p) <= corner_radius)
            .min_by(|a, b| a.0.distance(p).total_cmp(&b.0.distance(p)))
    };

    // Corners of the joined wall outlines are the true inner/outer corners.
    let mut corners: Vec<(Point2, SnapKind)> = outlines
        .iter()
        .flatten()
        .map(|q| (*q, SnapKind::Corner))
        .collect();
    for (wall, cuts) in home.walls.iter().zip(home.wall_cuts()) {
        corners.push((wall.start, SnapKind::Axis));
        corners.push((wall.end, SnapKind::Axis));
        let len = wall.start.distance(wall.end).max(1e-9);
        let (dx, dy) = (
            (wall.end.x - wall.start.x) / len,
            (wall.end.y - wall.start.y) / len,
        );
        let half = wall.thickness / 2.0;
        for cut in cuts {
            for s in [cut.from, cut.to] {
                for side in [-half, half] {
                    corners.push((
                        Point2::new(
                            wall.start.x + dx * s - dy * side,
                            wall.start.y + dy * s + dx * side,
                        ),
                        SnapKind::Corner,
                    ));
                }
            }
        }
    }
    corners.extend(
        home.rooms
            .iter()
            .flat_map(|r| r.points.iter().map(|q| (*q, SnapKind::Corner))),
    );
    corners.extend(
        home.furniture
            .iter()
            .filter(|f| f.visible && !f.is_opening())
            .flat_map(|f| f.footprint().map(|q| (q, SnapKind::Corner))),
    );
    // Prefer real corners over axis ends when both are equally close.
    if let Some(hit) = nearest(&mut corners.iter().copied()) {
        return hit;
    }

    let face = outlines
        .iter()
        .flat_map(|outline| {
            let n = outline.len();
            (0..n).map(move |i| (outline[i], outline[(i + 1) % n]))
        })
        .filter_map(|(a, b)| {
            let (ux, uy) = (b.x - a.x, b.y - a.y);
            let len2 = ux * ux + uy * uy;
            if len2 < 1e-9 {
                return None;
            }
            let t = (((p.x - a.x) * ux + (p.y - a.y) * uy) / len2).clamp(0.0, 1.0);
            let q = Point2::new(a.x + ux * t, a.y + uy * t);
            (q.distance(p) <= face_radius).then_some(q)
        })
        .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)));
    if let Some(q) = face {
        // Along a face, keep measurements square to the anchor when close.
        if let Some(a) = anchor {
            for aligned in [Point2::new(q.x, a.y), Point2::new(a.x, q.y)] {
                if aligned.distance(q) <= face_radius && aligned.distance(p) <= face_radius * 1.5 {
                    return (aligned, SnapKind::Face);
                }
            }
        }
        return (q, SnapKind::Face);
    }

    match anchor {
        Some(a) => (snap_segment(home, a, p, zoom, &[]), SnapKind::Free),
        None => (snap_point(home, p, zoom, &[]), SnapKind::Free),
    }
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
    fn measuring_snaps_to_inner_and_outer_corners_then_faces() {
        let mut home = Home::default();
        let pts = [(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            home.walls.push(Wall::new(
                WallId(i as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            ));
        }
        let outlines = home.wall_outlines();
        // Walls are 15 cm thick: outer corner (-7.5, -7.5), inner (7.5, 7.5).
        let (outer, kind) = snap_measure(&home, &outlines, Point2::new(-5.0, -9.0), 1.0, None);
        assert_eq!((outer, kind), (Point2::new(-7.5, -7.5), SnapKind::Corner));
        let (inner, _) = snap_measure(&home, &outlines, Point2::new(9.0, 6.0), 1.0, None);
        assert_eq!(inner, Point2::new(7.5, 7.5));
        // Mid-wall on the inner face.
        let (face, kind) = snap_measure(&home, &outlines, Point2::new(200.0, 12.0), 1.0, None);
        assert_eq!((face, kind), (Point2::new(200.0, 7.5), SnapKind::Face));
        // Far from everything: free rounding.
        let (_, kind) = snap_measure(&home, &outlines, Point2::new(200.0, 150.0), 1.0, None);
        assert_eq!(kind, SnapKind::Free);
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
