//! Smart guides while moving pieces, like design tools do with boxes: the
//! moving pieces' edges and center line up with other pieces, rooms and wall
//! faces nearby, a temporary line shows what they lined up with, and the
//! distances to what is around them are measured.

use newera_core::{ElementId, Home, Point2};

/// A temporary alignment line on the plan.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Guide {
    pub(crate) a: Point2,
    pub(crate) b: Point2,
    /// Center-to-center (drawn differently from edge alignment).
    pub(crate) center: bool,
}

/// The move after alignment, and what to show.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Aligned {
    pub(crate) dx: f64,
    pub(crate) dy: f64,
    pub(crate) guides: Vec<Guide>,
    /// Free distance from the moved pieces to the nearest thing on each side.
    pub(crate) gaps: Vec<(Point2, Point2, f64)>,
}

/// Axis-aligned box: `[min x, min y, max x, max y]`.
type Bounds = [f64; 4];

fn bounds(points: impl IntoIterator<Item = Point2>) -> Option<Bounds> {
    points.into_iter().fold(None, |acc, p| {
        Some(match acc {
            None => [p.x, p.y, p.x, p.y],
            Some([a, b, c, d]) => [a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)],
        })
    })
}

/// A correction on one axis and the lines it lines up with: `(value, target, center)`.
type Snap = (f64, Vec<(f64, usize, bool)>);

/// Something to line up with: its box, and whether its center counts.
struct Target {
    bounds: Bounds,
    center: bool,
}

fn targets(home: &Home, moving: &[ElementId]) -> Vec<Target> {
    let mut out = Vec::new();
    for f in home.furniture.iter().filter(|f| f.visible) {
        if moving.contains(&ElementId::Furniture(f.id)) || f.discipline.is_some() {
            continue;
        }
        if let Some(b) = bounds(f.projected_footprint()) {
            out.push(Target {
                bounds: b,
                center: !f.is_opening(),
            });
        }
    }
    for r in &home.rooms {
        if let Some(b) = bounds(r.points.iter().copied()) {
            out.push(Target {
                bounds: b,
                center: true,
            });
        }
    }
    // Faces of straight walls running along an axis.
    for w in home.walls.iter().filter(|w| !w.is_arc()) {
        let half = w.thickness / 2.0;
        if (w.start.x - w.end.x).abs() < 0.5 {
            let (y0, y1) = (w.start.y.min(w.end.y), w.start.y.max(w.end.y));
            out.push(Target {
                bounds: [w.start.x - half, y0, w.start.x + half, y1],
                center: false,
            });
        } else if (w.start.y - w.end.y).abs() < 0.5 {
            let (x0, x1) = (w.start.x.min(w.end.x), w.start.x.max(w.end.x));
            out.push(Target {
                bounds: [x0, w.start.y - half, x1, w.start.y + half],
                center: false,
            });
        }
    }
    out
}

/// Best alignment of `keys` (moving left, center, right) with the targets'
/// values on one axis within `threshold`: the correction and the guide spans.
fn best_on_axis(keys: [f64; 3], targets: &[Target], axis: usize, threshold: f64) -> Option<Snap> {
    let lo = axis;
    let hi = axis + 2;
    let mut best: Option<Snap> = None;
    for (t, target) in targets.iter().enumerate() {
        let (a, b) = (target.bounds[lo], target.bounds[hi]);
        let mut pairs = vec![
            (keys[0], a, false),
            (keys[0], b, false),
            (keys[2], a, false),
            (keys[2], b, false),
        ];
        if target.center {
            pairs.push((keys[1], f64::midpoint(a, b), true));
        }
        for (key, value, center) in pairs {
            let diff = value - key;
            if diff.abs() > threshold {
                continue;
            }
            match &mut best {
                Some((d, lines)) if (diff - *d).abs() < 0.01 => lines.push((value, t, center)),
                Some((d, _)) if diff.abs() >= d.abs() => {}
                _ => best = Some((diff, vec![(value, t, center)])),
            }
        }
    }
    best
}

/// Nearest edges around a box, along its center lines: left, right, up, down.
/// Boxes that contain it (rooms) don't count: their sides are walls.
fn neighbors(
    targets: &[Target],
    at: Bounds,
) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    let (cx, cy) = (f64::midpoint(at[0], at[2]), f64::midpoint(at[1], at[3]));
    let outside = |b: &Bounds| !(b[0] <= at[0] && b[2] >= at[2] && b[1] <= at[1] && b[3] >= at[3]);
    let pick = |f: &dyn Fn(&Bounds) -> Option<f64>, far: bool| {
        targets
            .iter()
            .map(|t| t.bounds)
            .filter(outside)
            .filter_map(|b| f(&b))
            .reduce(|a, b| if far { a.min(b) } else { a.max(b) })
    };
    let across_row = |b: &Bounds| b[1] <= cy && b[3] >= cy;
    let across_col = |b: &Bounds| b[0] <= cx && b[2] >= cx;
    (
        pick(
            &|b| (across_row(b) && b[2] <= at[0] + 0.01).then_some(b[2]),
            false,
        ),
        pick(
            &|b| (across_row(b) && b[0] >= at[2] - 0.01).then_some(b[0]),
            true,
        ),
        pick(
            &|b| (across_col(b) && b[3] <= at[1] + 0.01).then_some(b[3]),
            false,
        ),
        pick(
            &|b| (across_col(b) && b[1] >= at[3] - 0.01).then_some(b[1]),
            true,
        ),
    )
}

/// Aligns a move of the pieces in `moving` by `(dx, dy)`: snaps to the
/// nearest alignment within `threshold` cm on each axis (else to `step`),
/// and returns the guides and gaps to draw.
pub(crate) fn align(
    home: &Home,
    moving: &[ElementId],
    (dx, dy): (f64, f64),
    threshold: f64,
    step: f64,
) -> Aligned {
    let round = |v: f64| (v / step).round() * step;
    let pieces: Vec<_> = home
        .furniture
        .iter()
        .filter(|f| moving.contains(&ElementId::Furniture(f.id)))
        .collect();
    let Some(start) = bounds(pieces.iter().flat_map(|f| f.projected_footprint())) else {
        return Aligned {
            dx: round(dx),
            dy: round(dy),
            ..Aligned::default()
        };
    };
    let targets = targets(home, moving);
    let moved = |dx: f64, dy: f64| [start[0] + dx, start[1] + dy, start[2] + dx, start[3] + dy];
    let raw = moved(dx, dy);
    let keys_x = [raw[0], f64::midpoint(raw[0], raw[2]), raw[2]];
    let keys_y = [raw[1], f64::midpoint(raw[1], raw[3]), raw[3]];
    let mut snap_x = best_on_axis(keys_x, &targets, 0, threshold);
    let mut snap_y = best_on_axis(keys_y, &targets, 1, threshold);
    // Equal space on both sides, between the nearest things left and right
    // (or above and below), when no alignment is closer.
    let (left, right, up, down) = neighbors(&targets, raw);
    let between = |a: Option<f64>, b: Option<f64>, lo: f64, hi: f64| {
        let (a, b) = (a?, b?);
        let diff = f64::midpoint(a, b) - f64::midpoint(lo, hi);
        (diff.abs() <= threshold).then_some(diff)
    };
    if let Some(diff) = between(left, right, raw[0], raw[2])
        && snap_x.as_ref().is_none_or(|(d, _)| diff.abs() < d.abs())
    {
        snap_x = Some((diff, Vec::new()));
    }
    if let Some(diff) = between(up, down, raw[1], raw[3])
        && snap_y.as_ref().is_none_or(|(d, _)| diff.abs() < d.abs())
    {
        snap_y = Some((diff, Vec::new()));
    }
    let dx = snap_x.as_ref().map_or_else(|| round(dx), |(d, _)| dx + d);
    let dy = snap_y.as_ref().map_or_else(|| round(dy), |(d, _)| dy + d);
    let at = moved(dx, dy);
    let mut guides = Vec::new();
    for (value, t, center) in snap_x.map(|s| s.1).unwrap_or_default() {
        let tb = targets[t].bounds;
        guides.push(Guide {
            a: Point2::new(value, at[1].min(tb[1])),
            b: Point2::new(value, at[3].max(tb[3])),
            center,
        });
    }
    for (value, t, center) in snap_y.map(|s| s.1).unwrap_or_default() {
        let tb = targets[t].bounds;
        guides.push(Guide {
            a: Point2::new(at[0].min(tb[0]), value),
            b: Point2::new(at[2].max(tb[2]), value),
            center,
        });
    }
    // Distances to the nearest thing on each side, along the center lines.
    let (cx, cy) = (f64::midpoint(at[0], at[2]), f64::midpoint(at[1], at[3]));
    let (left, right, up, down) = neighbors(&targets, at);
    let limit = 600.0;
    let mut gaps = Vec::new();
    if let Some(x) = left.filter(|x| at[0] - x < limit) {
        gaps.push((Point2::new(x, cy), Point2::new(at[0], cy), at[0] - x));
    }
    if let Some(x) = right.filter(|x| x - at[2] < limit) {
        gaps.push((Point2::new(at[2], cy), Point2::new(x, cy), x - at[2]));
    }
    if let Some(y) = up.filter(|y| at[1] - y < limit) {
        gaps.push((Point2::new(cx, y), Point2::new(cx, at[1]), at[1] - y));
    }
    if let Some(y) = down.filter(|y| y - at[3] < limit) {
        gaps.push((Point2::new(cx, at[3]), Point2::new(cx, y), y - at[3]));
    }
    // Equal gaps get a line through the middle, like a center.
    for pair in [(left, right, true), (up, down, false)] {
        if let (Some(a), Some(b), horizontal) = pair {
            let (lo, hi) = if horizontal {
                (at[0], at[2])
            } else {
                (at[1], at[3])
            };
            if ((lo - a) - (b - hi)).abs() < 0.05 {
                let mid = f64::midpoint(lo, hi);
                guides.push(if horizontal {
                    Guide {
                        a: Point2::new(mid, at[1] - 15.0),
                        b: Point2::new(mid, at[3] + 15.0),
                        center: true,
                    }
                } else {
                    Guide {
                        a: Point2::new(at[0] - 15.0, mid),
                        b: Point2::new(at[2] + 15.0, mid),
                        center: true,
                    }
                });
            }
        }
    }
    Aligned {
        dx,
        dy,
        guides,
        gaps,
    }
}

#[cfg(test)]
mod tests {
    use newera_core::{Furniture, FurnitureId, Room, RoomId, Wall, WallId};

    use super::*;

    fn home() -> Home {
        let mut home = Home::default();
        let mut wall = Wall::new(WallId(1), Point2::new(0.0, 0.0), Point2::new(400.0, 0.0));
        wall.thickness = 10.0;
        home.walls.push(wall);
        home.rooms.push(Room::new(
            RoomId(2),
            "Sala",
            vec![
                Point2::new(0.0, 5.0),
                Point2::new(400.0, 5.0),
                Point2::new(400.0, 305.0),
                Point2::new(0.0, 305.0),
            ],
        ));
        home.furniture.push(Furniture {
            id: FurnitureId(3),
            catalog: "round-table".into(),
            name: "Mesa".into(),
            position: Point2::new(100.0, 150.0),
            width: 80.0,
            depth: 80.0,
            height: 75.0,
            ..Furniture::default()
        });
        home.furniture.push(Furniture {
            id: FurnitureId(4),
            catalog: "sofa-3".into(),
            name: "Sofá".into(),
            position: Point2::new(300.0, 250.0),
            width: 160.0,
            depth: 80.0,
            height: 85.0,
            ..Furniture::default()
        });
        home
    }

    #[test]
    fn a_round_table_centers_in_the_room_and_shows_why() {
        let home = home();
        let moving = [ElementId::Furniture(FurnitureId(3))];
        // Dragged to x ≈ 196, y ≈ 153: the room's center is 200, 155.
        let a = align(&home, &moving, (96.0, 3.0), 8.0, 5.0);
        assert!(
            (a.dx - 100.0).abs() < 1e-9 && (a.dy - 5.0).abs() < 1e-9,
            "{a:?}"
        );
        assert!(
            a.guides
                .iter()
                .any(|g| g.center && (g.a.x - 200.0).abs() < 1e-9)
        );
        assert!(
            a.guides
                .iter()
                .any(|g| g.center && (g.a.y - 155.0).abs() < 1e-9)
        );
        // Distance up to the wall face: 155 − 40 − 5.
        assert!(
            a.gaps.iter().any(|g| (g.2 - 110.0).abs() < 1e-9),
            "{:?}",
            a.gaps
        );
    }

    #[test]
    fn between_two_pieces_the_gaps_come_out_equal() {
        let mut home = home();
        home.rooms.clear();
        // Pieces at x 0..40 and 300..340 on the table's row; table 80 wide.
        home.furniture[1].position = Point2::new(320.0, 150.0);
        home.furniture[1].width = 40.0;
        home.furniture.push(Furniture {
            id: FurnitureId(5),
            catalog: "box".into(),
            name: "Vaso".into(),
            position: Point2::new(20.0, 150.0),
            width: 40.0,
            depth: 40.0,
            height: 60.0,
            ..Furniture::default()
        });
        let moving = [ElementId::Furniture(FurnitureId(3))];
        // Table centered at 100 + 67 = 167; equal gaps put it at 170.
        let a = align(&home, &moving, (67.0, 0.0), 8.0, 5.0);
        assert!((a.dx - 70.0).abs() < 1e-9, "{a:?}");
        let gaps: Vec<f64> = a.gaps.iter().map(|g| g.2).collect();
        assert!(
            gaps.iter().filter(|g| (**g - 90.0).abs() < 1e-9).count() == 2,
            "{gaps:?}"
        );
    }

    #[test]
    fn edges_line_up_with_other_pieces_and_far_moves_use_the_grid() {
        let home = home();
        let moving = [ElementId::Furniture(FurnitureId(3))];
        // The table's right edge (140 + dx) near the sofa's left edge 220.
        let a = align(&home, &moving, (77.0, -60.0), 8.0, 5.0);
        assert!((a.dx - 80.0).abs() < 1e-9, "{a:?}");
        assert!(
            a.guides
                .iter()
                .any(|g| !g.center && (g.a.x - 220.0).abs() < 1e-9)
        );
        // Nothing within reach on y: grid step.
        assert!((a.dy - -60.0).abs() < 1e-9);
        let far = align(&home, &moving, (33.0, 43.0), 2.0, 5.0);
        assert!(
            (far.dx - 35.0).abs() < 1e-9 && (far.dy - 45.0).abs() < 1e-9,
            "{far:?}"
        );
    }
}
