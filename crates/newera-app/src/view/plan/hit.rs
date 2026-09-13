//! Picking elements under the cursor.

use newera_core::{ElementId, Home, Point2, polygon_area};

fn inside(points: &[Point2], p: Point2) -> bool {
    let mut inside = false;
    let n = points.len();
    for i in 0..n {
        let (a, b) = (points[i], points[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Approximate label box half extents (cm).
fn label_half_extents(text: &str, size: f64) -> (f64, f64) {
    let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(1);
    #[allow(clippy::cast_precision_loss)]
    let width = longest as f64 * size * 0.55;
    #[allow(clippy::cast_precision_loss)]
    let height = text.lines().count().max(1) as f64 * size * 1.2;
    (width / 2.0, height / 2.0)
}

/// Topmost element at `p`, with `tolerance` cm of slack. Priority matches
/// draw order: labels, dimensions, walls, then rooms.
pub(crate) fn pick(
    home: &Home,
    outlines: &[Vec<Point2>],
    p: Point2,
    tolerance: f64,
) -> Option<ElementId> {
    for label in home.labels.iter().rev() {
        let (hw, hh) = label_half_extents(&label.text, label.size);
        let a = (-label.angle).to_radians();
        let (dx, dy) = (p.x - label.position.x, p.y - label.position.y);
        let (lx, ly) = (dx * a.cos() - dy * a.sin(), dx * a.sin() + dy * a.cos());
        if lx.abs() <= hw + tolerance && ly.abs() <= hh + tolerance {
            return Some(label.id.into());
        }
    }
    for dim in home.dimensions.iter().rev() {
        let len = dim.length().max(1e-9);
        let n = (
            (dim.end.y - dim.start.y) / len,
            -(dim.end.x - dim.start.x) / len,
        );
        let shift = |q: Point2| Point2::new(q.x + n.0 * dim.offset, q.y + n.1 * dim.offset);
        if p.distance_to_segment(shift(dim.start), shift(dim.end)) <= tolerance.max(4.0) {
            return Some(dim.id.into());
        }
    }
    // Doors and windows sit inside walls, so they win over them.
    for piece in home
        .furniture
        .iter()
        .rev()
        .filter(|f| f.visible && f.is_opening())
    {
        if piece.contains(p) {
            return Some(piece.id.into());
        }
    }
    // Pieces on the floor: the highest (e.g. a lamp on a table) first.
    let mut pieces: Vec<_> = home
        .furniture
        .iter()
        .filter(|f| f.visible && !f.is_opening() && f.contains(p))
        .collect();
    pieces.sort_by(|a, b| (b.elevation + b.height).total_cmp(&(a.elevation + a.height)));
    if let Some(piece) = pieces.first() {
        return Some(piece.id.into());
    }
    for (wall, outline) in home.walls.iter().zip(outlines).rev() {
        let near_line = wall
            .centerline()
            .windows(2)
            .any(|s| p.distance_to_segment(s[0], s[1]) <= wall.thickness / 2.0 + tolerance);
        if near_line || (outline.len() >= 3 && inside(outline, p)) {
            return Some(wall.id.into());
        }
    }
    // Smallest room first, so nested rooms stay selectable.
    home.rooms
        .iter()
        .filter(|r| inside(&r.points, p))
        .min_by(|a, b| polygon_area(&a.points).total_cmp(&polygon_area(&b.points)))
        .map(|r| r.id.into())
}

/// Elements whose geometry lies entirely inside the rectangle `min..max`.
pub(crate) fn in_rect(home: &Home, min: Point2, max: Point2) -> Vec<ElementId> {
    let contains = |p: &Point2| p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y;
    let mut ids: Vec<ElementId> = Vec::new();
    ids.extend(
        home.walls
            .iter()
            .filter(|w| w.centerline().iter().all(contains))
            .map(|w| ElementId::from(w.id)),
    );
    ids.extend(
        home.rooms
            .iter()
            .filter(|r| r.points.iter().all(contains))
            .map(|r| ElementId::from(r.id)),
    );
    ids.extend(
        home.dimensions
            .iter()
            .filter(|d| contains(&d.start) && contains(&d.end))
            .map(|d| ElementId::from(d.id)),
    );
    ids.extend(
        home.labels
            .iter()
            .filter(|l| contains(&l.position))
            .map(|l| ElementId::from(l.id)),
    );
    ids.extend(
        home.furniture
            .iter()
            .filter(|f| f.footprint().iter().all(contains))
            .map(|f| ElementId::from(f.id)),
    );
    ids
}

#[cfg(test)]
mod tests {
    use newera_core::{Label, LabelId, Room, RoomId, Wall, WallId};

    use super::*;

    #[test]
    fn labels_beat_walls_and_walls_beat_rooms() {
        let mut home = Home::default();
        home.walls.push(Wall::new(
            WallId(1),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        ));
        home.rooms.push(Room::new(
            RoomId(2),
            "Sala",
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(400.0, 0.0),
                Point2::new(400.0, 300.0),
                Point2::new(0.0, 300.0),
            ],
        ));
        home.labels.push(Label {
            id: LabelId(3),
            text: "Oi".into(),
            position: Point2::new(200.0, 150.0),
            size: 30.0,
            angle: 0.0,
            level: None,
        });
        let outlines = home.wall_outlines();
        assert_eq!(
            pick(&home, &outlines, Point2::new(100.0, 2.0), 2.0),
            Some(WallId(1).into())
        );
        assert_eq!(
            pick(&home, &outlines, Point2::new(100.0, 100.0), 2.0),
            Some(RoomId(2).into())
        );
        assert_eq!(
            pick(&home, &outlines, Point2::new(205.0, 150.0), 2.0),
            Some(LabelId(3).into())
        );
        assert_eq!(pick(&home, &outlines, Point2::new(900.0, 900.0), 2.0), None);
        let boxed = in_rect(&home, Point2::new(-10.0, -10.0), Point2::new(410.0, 20.0));
        assert_eq!(boxed, vec![WallId(1).into()]);
    }
}
