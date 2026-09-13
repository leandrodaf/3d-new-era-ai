//! Wall joins: the floor outline of each wall, shaped by its neighbors.
//!
//! Walls whose endpoints coincide form a *node*. At a node the walls are
//! sorted by angle, and each wall's side edge is extended until it meets the
//! facing side edge of the next wall around the node. This single rule covers
//! L corners (miter), straight continuations, T and X junctions, and free
//! ends (square cap), so adjacent walls never overlap nor leave gaps.

use crate::geometry::Point2;
use crate::model::Wall;

/// Endpoints closer than this (cm) are treated as connected.
pub const JOIN_TOLERANCE: f64 = 0.5;

/// Miters longer than this many half-thicknesses are cut square, so very
/// sharp angles don't produce long spikes.
const MITER_LIMIT: f64 = 6.0;

#[derive(Debug, Clone, Copy)]
struct End {
    wall: usize,
    /// True for the wall's start point, false for its end point.
    at_start: bool,
    /// Unit direction pointing away from the node, along the wall.
    dir: (f64, f64),
    half: f64,
    angle: f64,
}

/// Corners of one wall end: `left`/`right` are relative to the direction
/// pointing *away* from the node.
#[derive(Debug, Clone, Copy)]
struct Corners {
    left: Point2,
    right: Point2,
}

/// Floor outline of every wall, in the same order as `walls`.
///
/// Each outline is a simple polygon in plan coordinates (cm). It may include
/// the node point itself, which fills the center of junctions with three or
/// more walls; for two walls that point is collinear and harmless.
pub fn wall_outlines(walls: &[Wall]) -> Vec<Vec<Point2>> {
    let mut corners: Vec<[Option<(Corners, Point2)>; 2]> = vec![[None, None]; walls.len()];

    for node in group_nodes(walls) {
        let mut ends: Vec<End> = node
            .iter()
            .filter_map(|&(i, at_start)| end(walls, i, at_start))
            .collect();
        if ends.is_empty() {
            continue;
        }
        ends.sort_by(|a, b| a.angle.total_cmp(&b.angle));
        let center = node_center(walls, &node);
        let m = ends.len();
        for k in 0..m {
            let this = ends[k];
            let next = ends[(k + 1) % m];
            let prev = ends[(k + m - 1) % m];
            let left = side_corner(center, this, 1.0, next, -1.0);
            let right = side_corner(center, this, -1.0, prev, 1.0);
            let slot = usize::from(!this.at_start);
            corners[this.wall][slot] = Some((Corners { left, right }, center));
        }
    }

    walls
        .iter()
        .zip(corners)
        .map(|(_, [start, end])| {
            let (Some((s, s_node)), Some((e, e_node))) = (start, end) else {
                return Vec::new(); // degenerate wall
            };
            // Walking the outline: start.right -> start node -> start.left is the
            // start cap; along the wall, start.left faces the same side as end.right.
            let mut outline = vec![s.right, s_node, s.left, e.right, e_node, e.left];
            outline.dedup_by(|a, b| a.distance(*b) < 1e-6);
            if outline.len() > 1 && outline[0].distance(outline[outline.len() - 1]) < 1e-6 {
                outline.pop();
            }
            outline
        })
        .collect()
}

fn end(walls: &[Wall], wall: usize, at_start: bool) -> Option<End> {
    let w = &walls[wall];
    let (from, to) = if at_start {
        (w.start, w.end)
    } else {
        (w.end, w.start)
    };
    let len = from.distance(to);
    if len < 1e-9 {
        return None;
    }
    let dir = ((to.x - from.x) / len, (to.y - from.y) / len);
    Some(End {
        wall,
        at_start,
        dir,
        half: w.thickness / 2.0,
        angle: dir.1.atan2(dir.0),
    })
}

/// Groups wall endpoints that coincide within [`JOIN_TOLERANCE`].
fn group_nodes(walls: &[Wall]) -> Vec<Vec<(usize, bool)>> {
    let mut nodes: Vec<(Point2, Vec<(usize, bool)>)> = Vec::new();
    for (i, wall) in walls.iter().enumerate() {
        for (at_start, p) in [(true, wall.start), (false, wall.end)] {
            match nodes
                .iter_mut()
                .find(|(q, _)| q.distance(p) <= JOIN_TOLERANCE)
            {
                Some((_, members)) => members.push((i, at_start)),
                None => nodes.push((p, vec![(i, at_start)])),
            }
        }
    }
    nodes.into_iter().map(|(_, members)| members).collect()
}

fn node_center(walls: &[Wall], node: &[(usize, bool)]) -> Point2 {
    let p = |&(i, at_start): &(usize, bool)| {
        if at_start {
            walls[i].start
        } else {
            walls[i].end
        }
    };
    node.first().map_or_else(Point2::default, p)
}

/// Offset line of `end` on `side` (+1 left, -1 right).
fn offset(center: Point2, end: End, side: f64) -> (Point2, (f64, f64)) {
    let normal = (-end.dir.1 * side, end.dir.0 * side);
    let origin = Point2::new(
        center.x + normal.0 * end.half,
        center.y + normal.1 * end.half,
    );
    (origin, end.dir)
}

/// Where `this`'s side edge meets `other`'s facing edge; square cap when the
/// edges are parallel, the neighbor is the wall itself, or the miter is too long.
fn side_corner(center: Point2, this: End, this_side: f64, other: End, other_side: f64) -> Point2 {
    let (p, d) = offset(center, this, this_side);
    if other.wall == this.wall && other.at_start == this.at_start {
        return p;
    }
    let (q, e) = offset(center, other, other_side);
    let denom = d.0 * e.1 - d.1 * e.0;
    if denom.abs() < 1e-9 {
        return p;
    }
    let t = ((q.x - p.x) * e.1 - (q.y - p.y) * e.0) / denom;
    let corner = Point2::new(p.x + d.0 * t, p.y + d.1 * t);
    if corner.distance(center) > MITER_LIMIT * this.half.max(other.half) {
        return p;
    }
    corner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{polygon_area, signed_area};
    use crate::model::WallId;

    fn wall(id: u64, a: (f64, f64), b: (f64, f64), thickness: f64) -> Wall {
        Wall {
            thickness,
            ..Wall::new(WallId(id), Point2::new(a.0, a.1), Point2::new(b.0, b.1))
        }
    }

    fn has_point(outline: &[Point2], x: f64, y: f64) -> bool {
        outline.iter().any(|p| p.distance(Point2::new(x, y)) < 1e-6)
    }

    #[test]
    fn free_wall_is_a_rectangle() {
        let outlines = wall_outlines(&[wall(1, (0.0, 0.0), (400.0, 0.0), 20.0)]);
        let o = &outlines[0];
        assert!((polygon_area(o) - 400.0 * 20.0).abs() < 1e-6);
        for (x, y) in [(0.0, 10.0), (0.0, -10.0), (400.0, 10.0), (400.0, -10.0)] {
            assert!(has_point(o, x, y), "missing corner ({x}, {y}) in {o:?}");
        }
    }

    #[test]
    fn l_corner_is_mitered_without_overlap() {
        // Outer corner of a 20 cm L at (400, 0) lands on (410, -10).
        let walls = [
            wall(1, (0.0, 0.0), (400.0, 0.0), 20.0),
            wall(2, (400.0, 0.0), (400.0, 300.0), 20.0),
        ];
        let outlines = wall_outlines(&walls);
        assert!(has_point(&outlines[0], 410.0, -10.0));
        assert!(has_point(&outlines[0], 390.0, 10.0));
        assert!(has_point(&outlines[1], 410.0, -10.0));
        assert!(has_point(&outlines[1], 390.0, 10.0));
        // Together they cover exactly the L footprint (no overlap, no gap).
        let total: f64 = outlines.iter().map(|o| polygon_area(o)).sum();
        // Bar [0,410]x[-10,10] plus stem [390,410]x[10,300].
        let footprint = 410.0 * 20.0 + 20.0 * 290.0;
        assert!((total - footprint).abs() < 1e-6, "{total} vs {footprint}");
    }

    #[test]
    fn closed_square_has_matching_corners_and_consistent_outlines() {
        let s = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let walls: Vec<Wall> = (0..4_usize)
            .map(|i| wall(i as u64 + 1, s[i], s[(i + 1) % 4], 10.0))
            .collect();
        let outlines = wall_outlines(&walls);
        let total: f64 = outlines.iter().map(|o| polygon_area(o)).sum();
        // Outer 410x410 minus inner 390x390.
        assert!(
            (total - (410.0 * 410.0 - 390.0 * 390.0)).abs() < 1e-6,
            "{total}"
        );
        for o in &outlines {
            assert!(signed_area(o).abs() > 0.0);
        }
    }

    #[test]
    fn t_junction_leaves_no_gap_at_the_center() {
        let walls = [
            wall(1, (-200.0, 0.0), (0.0, 0.0), 20.0),
            wall(2, (0.0, 0.0), (200.0, 0.0), 20.0),
            wall(3, (0.0, 0.0), (0.0, 200.0), 20.0),
        ];
        let total: f64 = wall_outlines(&walls).iter().map(|o| polygon_area(o)).sum();
        // A 400x20 bar plus a 20x190 stem.
        assert!(
            (total - (400.0 * 20.0 + 20.0 * 190.0)).abs() < 1e-6,
            "{total}"
        );
    }

    #[test]
    fn straight_continuation_stays_flat() {
        let walls = [
            wall(1, (0.0, 0.0), (200.0, 0.0), 20.0),
            wall(2, (200.0, 0.0), (500.0, 0.0), 20.0),
        ];
        let outlines = wall_outlines(&walls);
        assert!(has_point(&outlines[0], 200.0, 10.0) && has_point(&outlines[0], 200.0, -10.0));
        assert!((polygon_area(&outlines[1]) - 300.0 * 20.0).abs() < 1e-6);
    }
}
