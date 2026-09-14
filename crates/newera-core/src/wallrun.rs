//! A run along one face of a wall: how long it is and what blocks it at a
//! given depth and height — joined walls, doors and their swing, windows,
//! appliances. Built-in joinery fills the free stretches in between.

use geo::{BooleanOps, BoundingRect, Coord, LineString, Polygon};

use crate::analysis::door_swing;
use crate::elements::Wall;
use crate::furniture::{Furniture, OpeningKind};
use crate::geometry::Point2;
use crate::home::Home;
use crate::ids::{FurnitureId, WallId};

/// What occupies part of a run.
#[derive(Debug, Clone, PartialEq)]
pub enum RunBlock {
    /// Another wall crossing the band (a corner, a partition).
    Wall(WallId),
    /// A door or window in the run's wall, or a door swinging into the band.
    Opening { id: FurnitureId, window: bool },
    /// A piece standing in the band, with its catalog id.
    Piece { id: FurnitureId, catalog: String },
}

/// An occupied stretch, cm along the wall from its start.
#[derive(Debug, Clone, PartialEq)]
pub struct RunObstacle {
    pub from: f64,
    pub to: f64,
    pub block: RunBlock,
}

/// A wall face seen as a run.
#[derive(Debug, Clone, PartialEq)]
pub struct WallRun {
    /// Centerline length, cm.
    pub length: f64,
    /// `1.0` for the face on the left of start → end, `-1.0` for the right.
    pub side: f64,
    /// Sorted by `from`.
    pub obstacles: Vec<RunObstacle>,
}

impl WallRun {
    /// Free stretches `(from, to, before, after)`: the obstacles bounding each
    /// one (`None` at a free wall end). Stretches shorter than `min` are kept
    /// too — deciding what to do with them is the caller's job.
    pub fn gaps(&self) -> Vec<(f64, f64, Option<&RunObstacle>, Option<&RunObstacle>)> {
        let mut out = Vec::new();
        let mut cursor = 0.0;
        let mut before: Option<&RunObstacle> = None;
        for o in &self.obstacles {
            if o.from > cursor + 0.05 {
                out.push((cursor, o.from.min(self.length), before, Some(o)));
            }
            if o.to >= cursor {
                cursor = o.to;
                before = Some(o);
            }
        }
        if self.length > cursor + 0.05 {
            out.push((cursor, self.length, before, None));
        }
        out
    }
}

fn polygon(points: &[Point2]) -> Polygon<f64> {
    let mut coords: Vec<Coord<f64>> = points.iter().map(|p| Coord { x: p.x, y: p.y }).collect();
    if let Some(first) = coords.first().copied() {
        coords.push(first);
    }
    Polygon::new(LineString::new(coords), vec![])
}

/// Measures the run on one face of `wall` for joinery `depth` cm deep
/// between heights `z` (cm above the floor). `skip` leaves out pieces that
/// the run itself will replace. Straight walls of the home's current level
/// only; `None` for arcs or unknown walls.
pub fn wall_run(
    home: &Home,
    wall: WallId,
    side: f64,
    depth: f64,
    z: (f64, f64),
    skip: &dyn Fn(&Furniture) -> bool,
) -> Option<WallRun> {
    let view = home.level_view(home.current_level());
    let w: &Wall = view.walls.iter().find(|w| w.id == wall)?;
    if w.is_arc() {
        return None;
    }
    let length = w.start.distance(w.end);
    if length < 1.0 {
        return None;
    }
    let u = (
        (w.end.x - w.start.x) / length,
        (w.end.y - w.start.y) / length,
    );
    let n = (-u.1 * side, u.0 * side);
    // Wall frame: along `a`, away from the face `b` (0 at the centerline).
    let to_frame = |p: &Point2| {
        let (dx, dy) = (p.x - w.start.x, p.y - w.start.y);
        (dx * u.0 + dy * u.1, dx * n.0 + dy * n.1)
    };
    let face = w.thickness / 2.0;
    // Keep 1 cm off the face so the wall's own outline never counts.
    let (b0, b1) = (face + 1.0, face + depth.max(2.0));
    let band = Polygon::new(
        LineString::from(vec![
            (-10_000.0, b0),
            (length + 10_000.0, b0),
            (length + 10_000.0, b1),
            (-10_000.0, b1),
            (-10_000.0, b0),
        ]),
        vec![],
    );
    let mut obstacles = Vec::new();
    let clip = |points: &[Point2], block: RunBlock| -> Option<RunObstacle> {
        let local: Vec<Point2> = points
            .iter()
            .map(|p| {
                let (a, b) = to_frame(p);
                Point2::new(a, b)
            })
            .collect();
        let shared = polygon(&local).intersection(&band);
        let rect = shared.bounding_rect()?;
        let (from, to) = (rect.min().x.max(0.0), rect.max().x.min(length));
        (rect.width() > 0.5 && rect.height() > 0.5 && to > from).then_some(RunObstacle {
            from,
            to,
            block,
        })
    };
    for (other, outline) in view.walls.iter().zip(view.wall_outlines()) {
        if other.id != wall && outline.len() >= 3 {
            obstacles.extend(clip(&outline, RunBlock::Wall(other.id)));
        }
    }
    for top in &view.furniture {
        if skip(top) {
            continue;
        }
        for piece in top.visible_leaves() {
            let (lo, hi) = piece.height_range();
            if let Some(opening) = &piece.opening {
                let (a, b) = to_frame(&piece.position);
                let in_this_wall = b.abs() <= face + 2.0 && (-1.0..=length + 1.0).contains(&a);
                let window = opening.kind != OpeningKind::Door;
                if in_this_wall && lo < z.1 - 1.0 && z.0 + 1.0 < hi {
                    let half = piece.width / 2.0;
                    obstacles.push(RunObstacle {
                        from: (a - half).max(0.0),
                        to: (a + half).min(length),
                        block: RunBlock::Opening {
                            id: piece.id,
                            window,
                        },
                    });
                }
                // A door elsewhere whose leaf sweeps through the band.
                if !in_this_wall
                    && z.0 < 200.0
                    && let Some(swing) = door_swing(piece)
                {
                    obstacles.extend(clip(
                        &swing,
                        RunBlock::Opening {
                            id: piece.id,
                            window: false,
                        },
                    ));
                }
                continue;
            }
            if piece.height <= 2.0 || hi <= z.0 + 0.5 || lo >= z.1 - 0.5 {
                continue;
            }
            obstacles.extend(clip(
                &piece.projected_footprint(),
                RunBlock::Piece {
                    id: top.id,
                    catalog: piece.catalog.clone(),
                },
            ));
        }
    }
    obstacles.sort_by(|a, b| a.from.total_cmp(&b.from));
    // One obstacle per piece of a group: merge overlapping stretches of it.
    let mut merged: Vec<RunObstacle> = Vec::new();
    for o in obstacles {
        match merged.last_mut() {
            Some(last) if last.block == o.block && o.from <= last.to + 0.5 => {
                last.to = last.to.max(o.to);
            }
            _ => merged.push(o),
        }
    }
    Some(WallRun {
        length,
        side,
        obstacles: merged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::furniture::Opening;

    fn wall(id: u64, a: (f64, f64), b: (f64, f64)) -> Wall {
        let mut w = Wall::new(WallId(id), Point2::new(a.0, a.1), Point2::new(b.0, b.1));
        w.thickness = 15.0;
        w.height = 260.0;
        w
    }

    #[test]
    fn corners_doors_windows_and_appliances_block_the_run() {
        let mut home = Home::default();
        home.walls = vec![
            wall(1, (0.0, 0.0), (400.0, 0.0)),
            wall(2, (400.0, 0.0), (400.0, 300.0)),
            wall(3, (400.0, 300.0), (0.0, 300.0)),
            wall(4, (0.0, 300.0), (0.0, 0.0)),
        ];
        // Inside is +y: the left normal of w1.
        let side = 1.0;
        let fridge = Furniture {
            id: FurnitureId(10),
            catalog: "fridge".into(),
            position: Point2::new(360.0, 7.5 + 35.0),
            width: 70.0,
            depth: 70.0,
            height: 180.0,
            ..Furniture::default()
        };
        let window = Furniture {
            id: FurnitureId(11),
            catalog: "window".into(),
            position: Point2::new(150.0, 0.0),
            elevation: 100.0,
            width: 100.0,
            depth: 15.0,
            height: 110.0,
            opening: Some(Opening {
                kind: OpeningKind::Window,
                ..Opening::default()
            }),
            ..Furniture::default()
        };
        home.furniture = vec![fridge, window];
        // Base cabinets: the window sits above them.
        let base = wall_run(&home, WallId(1), side, 60.0, (0.0, 90.0), &|_| false).unwrap();
        let spans: Vec<(f64, f64)> = base.gaps().iter().map(|g| (g.0, g.1)).collect();
        // Free from the inner face of w4 (7.5) to the fridge (325).
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert!(
            (spans[0].0 - 7.5).abs() < 0.6 && (spans[0].1 - 325.0).abs() < 0.6,
            "{spans:?}"
        );
        // Wall cabinets: split by the window, still stopped by the fridge.
        let upper = wall_run(&home, WallId(1), side, 35.0, (150.0, 220.0), &|_| false).unwrap();
        let spans: Vec<(f64, f64)> = upper.gaps().iter().map(|g| (g.0, g.1)).collect();
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert!((spans[0].1 - 100.0).abs() < 0.6 && (spans[1].0 - 200.0).abs() < 0.6);
        assert!(matches!(
            upper.gaps()[1].3.map(|o| &o.block),
            Some(RunBlock::Piece { catalog, .. }) if catalog == "fridge"
        ));
        // Skipped pieces (the ones being replaced) don't block.
        let all = wall_run(&home, WallId(1), side, 60.0, (0.0, 90.0), &|f| {
            f.id == FurnitureId(10)
        })
        .unwrap();
        assert!((all.gaps()[0].1 - 392.5).abs() < 0.6);
    }
}
