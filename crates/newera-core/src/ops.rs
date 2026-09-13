//! Composite edits built from [`Command`]s, shared by the editor and MCP.

use crate::command::Command;
use crate::document::Document;
use crate::elements::{Dimension, Element, Wall};
use crate::error::{CoreError, CoreResult};
use crate::geometry::Point2;
use crate::ids::{ElementId, WallId};
use crate::joins::JOIN_TOLERANCE;

/// Splits a wall at parameter `t` (0..1) into two joined walls. Returns the
/// id of the new second half.
pub fn split_wall(doc: &mut Document, id: WallId, t: f64) -> CoreResult<WallId> {
    let wall = doc
        .home()
        .wall(id)
        .cloned()
        .ok_or(CoreError::NotFound(id.into()))?;
    if !(t > 0.0 && t < 1.0) {
        return Err(CoreError::InvalidGeometry(
            "split point must be strictly inside the wall (0 < t < 1)".into(),
        ));
    }
    let at = wall.point_at(t);
    let new_id = doc.new_wall_id();
    let first = Wall {
        end: at,
        arc_extent: wall.arc_extent.map(|a| a * t),
        ..wall.clone()
    };
    let second = Wall {
        id: new_id,
        start: at,
        arc_extent: wall.arc_extent.map(|a| a * (1.0 - t)),
        ..wall
    };
    doc.execute(Command::Batch {
        commands: vec![Command::update(first), Command::insert(second)],
    })?;
    Ok(new_id)
}

/// Moves elements by `(dx, dy)` cm as one undoable step.
///
/// With `drag_joined`, endpoints of other walls that were joined to a moved
/// wall follow it, so rooms stay closed — the behavior users expect when
/// dragging walls in the editor.
pub fn translate(
    doc: &mut Document,
    ids: &[ElementId],
    dx: f64,
    dy: f64,
    drag_joined: bool,
) -> CoreResult<()> {
    let home = doc.home();
    let shift = |p: Point2| Point2::new(p.x + dx, p.y + dy);
    let mut commands = Vec::with_capacity(ids.len());
    let mut moved_points = Vec::new();

    for id in ids {
        let element = home.element(*id).ok_or(CoreError::NotFound(*id))?;
        let moved = match element {
            Element::Wall(w) => {
                moved_points.extend([w.start, w.end]);
                Element::Wall(Wall {
                    start: shift(w.start),
                    end: shift(w.end),
                    ..w
                })
            }
            Element::Room(mut r) => {
                r.points = r.points.into_iter().map(shift).collect();
                Element::Room(r)
            }
            Element::Dimension(d) => Element::Dimension(Dimension {
                start: shift(d.start),
                end: shift(d.end),
                ..d
            }),
            Element::Label(mut l) => {
                l.position = shift(l.position);
                Element::Label(l)
            }
        };
        commands.push(Command::Update { element: moved });
    }

    if drag_joined {
        for wall in home.walls.iter().filter(|w| !ids.contains(&w.id.into())) {
            let near = |p: Point2| moved_points.iter().any(|q| q.distance(p) <= JOIN_TOLERANCE);
            let (s, e) = (near(wall.start), near(wall.end));
            if s || e {
                let updated = Wall {
                    start: if s { shift(wall.start) } else { wall.start },
                    end: if e { shift(wall.end) } else { wall.end },
                    ..wall.clone()
                };
                commands.push(Command::update(updated));
            }
        }
    }
    doc.execute(Command::Batch { commands })
}

/// Moves one wall endpoint, dragging every other wall endpoint joined to it.
pub fn move_wall_point(
    doc: &mut Document,
    id: WallId,
    at_start: bool,
    to: Point2,
) -> CoreResult<()> {
    let home = doc.home();
    let wall = home.wall(id).ok_or(CoreError::NotFound(id.into()))?;
    let from = if at_start { wall.start } else { wall.end };
    let commands = home
        .walls
        .iter()
        .filter_map(|w| {
            let s = w.start.distance(from) <= JOIN_TOLERANCE;
            let e = w.end.distance(from) <= JOIN_TOLERANCE;
            (s || e).then(|| {
                Command::update(Wall {
                    start: if s { to } else { w.start },
                    end: if e { to } else { w.end },
                    ..w.clone()
                })
            })
        })
        .collect();
    doc.execute(Command::Batch { commands })
}

/// A dimension measuring a wall, placed on the side facing away from the
/// middle of the drawing (outside the house), `gap` cm beyond the wall face.
pub fn wall_dimension(doc: &mut Document, id: WallId, gap: f64) -> CoreResult<Dimension> {
    let home = doc.home();
    let wall = home.wall(id).ok_or(CoreError::NotFound(id.into()))?.clone();
    let center = home.bounds().map_or(wall.start, |(min, max)| {
        Point2::new(min.x.midpoint(max.x), min.y.midpoint(max.y))
    });
    let mid = wall.point_at(0.5);
    let len = wall.start.distance(wall.end).max(1e-9);
    // Left normal in plan axes (y down), matching Dimension::offset.
    let normal = (
        (wall.end.y - wall.start.y) / len,
        -(wall.end.x - wall.start.x) / len,
    );
    let outward = (mid.x - center.x) * normal.0 + (mid.y - center.y) * normal.1;
    let sign = if outward >= 0.0 { 1.0 } else { -1.0 };
    Ok(Dimension {
        id: doc.new_dimension_id(),
        start: wall.start,
        end: wall.end,
        offset: sign * (wall.thickness / 2.0 + gap),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::Label;

    fn doc_with_l() -> (Document, WallId, WallId) {
        let mut doc = Document::default();
        let a = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        );
        let b = Wall::new(
            doc.new_wall_id(),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 300.0),
        );
        let (ia, ib) = (a.id, b.id);
        doc.execute(Command::Batch {
            commands: vec![Command::insert(a), Command::insert(b)],
        })
        .unwrap();
        (doc, ia, ib)
    }

    #[test]
    fn split_keeps_the_join_and_undoes_in_one_step() {
        let (mut doc, a, _) = doc_with_l();
        let second = split_wall(&mut doc, a, 0.25).unwrap();
        let home = doc.home();
        assert_eq!(home.wall(a).unwrap().end, Point2::new(100.0, 0.0));
        assert_eq!(home.wall(second).unwrap().start, Point2::new(100.0, 0.0));
        assert_eq!(home.wall(second).unwrap().end, Point2::new(400.0, 0.0));
        doc.undo().unwrap();
        assert_eq!(doc.home().walls.len(), 2);
        assert!(split_wall(&mut doc, a, 1.0).is_err());
    }

    #[test]
    fn translating_a_wall_drags_joined_walls() {
        let (mut doc, a, b) = doc_with_l();
        translate(&mut doc, &[a.into()], 0.0, -50.0, true).unwrap();
        let home = doc.home();
        assert_eq!(home.wall(a).unwrap().start, Point2::new(0.0, -50.0));
        assert_eq!(
            home.wall(b).unwrap().start,
            Point2::new(400.0, -50.0),
            "joined end followed"
        );
        assert_eq!(
            home.wall(b).unwrap().end,
            Point2::new(400.0, 300.0),
            "far end stayed"
        );
    }

    #[test]
    fn translate_moves_any_element_kind() {
        let mut doc = Document::default();
        let label = Label {
            id: doc.new_label_id(),
            text: "A".into(),
            position: Point2::new(1.0, 2.0),
            size: 20.0,
            angle: 0.0,
        };
        let id = label.id;
        doc.execute(Command::insert(label)).unwrap();
        translate(&mut doc, &[id.into()], 10.0, 20.0, false).unwrap();
        assert_eq!(
            doc.home().label(id).unwrap().position,
            Point2::new(11.0, 22.0)
        );
    }

    #[test]
    fn moving_a_corner_moves_both_walls() {
        let (mut doc, a, b) = doc_with_l();
        move_wall_point(&mut doc, a, false, Point2::new(450.0, 20.0)).unwrap();
        assert_eq!(doc.home().wall(a).unwrap().end, Point2::new(450.0, 20.0));
        assert_eq!(doc.home().wall(b).unwrap().start, Point2::new(450.0, 20.0));
    }

    #[test]
    fn wall_dimensions_go_outside() {
        let (mut doc, a, b) = doc_with_l();
        // Bounds center is (200, 150): wall a (top) should be dimensioned above.
        let dim = wall_dimension(&mut doc, a, 30.0).unwrap();
        assert!(dim.offset > 0.0, "left of +x is up (-y), away from center");
        let dim_b = wall_dimension(&mut doc, b, 30.0).unwrap();
        // b goes down (+y); left normal points +x, away from the center.
        assert!(dim_b.offset > 0.0);
        assert!((dim.offset - 37.5).abs() < 1e-9);
    }
}
