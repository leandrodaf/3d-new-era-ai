//! Composite edits built from [`Command`]s, shared by the editor and MCP.

use crate::command::Command;
use crate::document::Document;
use crate::elements::{Dimension, Element, Wall};
use crate::error::{CoreError, CoreResult};
use crate::geometry::Point2;
use crate::ids::{ElementId, WallId};
use crate::joins::JOIN_TOLERANCE;

/// How far (cm) a moved door or window looks for a wall to sit in.
pub const OPENING_REACH: f64 = 60.0;

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
            Element::Level(level) => Element::Level(level),
            Element::Polyline(mut p) => {
                p.points = p.points.into_iter().map(shift).collect();
                Element::Polyline(p)
            }
            Element::Furniture(mut f) => {
                f.translate(dx, dy);
                // Doors and windows stay seated in the nearest wall.
                if f.is_opening()
                    && let Some((wall_id, along)) =
                        nearest_wall(&home.level_view(f.level), f.position, OPENING_REACH)
                    && let Some(wall) = home.wall(wall_id)
                {
                    crate::furniture::align_to_wall(&mut f, wall, along);
                }
                Element::Furniture(f)
            }
        };
        commands.push(Command::Update { element: moved });
    }

    if drag_joined {
        let moved_levels: Vec<_> = ids
            .iter()
            .filter_map(|id| home.element(*id))
            .map(|e| home.resolve_level(e.level()))
            .collect();
        for wall in home
            .walls
            .iter()
            .filter(|w| !ids.contains(&w.id.into()))
            .filter(|w| moved_levels.contains(&home.resolve_level(w.level)))
        {
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
    let level = wall.level;
    let commands = home
        .walls
        .iter()
        .filter(|w| home.on_level(w.level, level))
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
        level: wall.level,
        ..Dimension::default()
    })
}

/// Nearest straight wall to `p` within `max_distance` cm of its centerline,
/// with the distance along it from its start.
pub fn nearest_wall(
    home: &crate::home::Home,
    p: Point2,
    max_distance: f64,
) -> Option<(WallId, f64)> {
    home.walls
        .iter()
        .filter(|w| !w.is_arc())
        .filter_map(|w| {
            let len = w.start.distance(w.end);
            if len < 1e-9 {
                return None;
            }
            let dir = ((w.end.x - w.start.x) / len, (w.end.y - w.start.y) / len);
            let along = ((p.x - w.start.x) * dir.0 + (p.y - w.start.y) * dir.1).clamp(0.0, len);
            let foot = Point2::new(w.start.x + dir.0 * along, w.start.y + dir.1 * along);
            let distance = foot.distance(p);
            (distance <= max_distance).then_some((w.id, along, distance))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(id, along, _)| (id, along))
}

/// Re-seats doors and windows in the wall nearest to them (after a move).
pub fn snap_openings(doc: &mut Document, ids: &[ElementId], max_distance: f64) -> CoreResult<()> {
    let home = doc.home();
    let commands: Vec<Command> = ids
        .iter()
        .filter_map(|id| match home.element(*id) {
            Some(Element::Furniture(f)) if f.is_opening() => Some(f),
            _ => None,
        })
        .filter_map(|mut f| {
            let (wall_id, along) =
                nearest_wall(&home.level_view(f.level), f.position, max_distance)?;
            let wall = home.wall(wall_id)?;
            crate::furniture::align_to_wall(&mut f, wall, along);
            Some(Command::update(f))
        })
        .collect();
    if commands.is_empty() {
        return Ok(());
    }
    doc.execute(Command::Batch { commands })
}

/// Adds a storey on top of the highest one and selects it. The first call on
/// a single-level house also creates the ground level ("Térreo") for the
/// existing elements. Returns the new level's id.
///
/// # Panics
/// Never: the level list is non-empty once the ground level exists.
pub fn add_level(
    doc: &mut Document,
    name: Option<String>,
    height: Option<f64>,
) -> CoreResult<crate::ids::LevelId> {
    use crate::elements::Level;
    let mut commands = Vec::new();
    let mut levels: Vec<Level> = doc.home().levels.clone();
    if levels.is_empty() {
        let ground = Level {
            id: doc.new_level_id(),
            name: "Térreo".to_owned(),
            elevation: 0.0,
            height: crate::elements::Wall::DEFAULT_HEIGHT,
            floor_thickness: Level::DEFAULT_FLOOR_THICKNESS,
            ..Level::default()
        };
        levels.push(ground.clone());
        commands.push(Command::insert(ground));
    }
    let top = levels
        .iter()
        .max_by(|a, b| a.elevation.total_cmp(&b.elevation))
        .expect("at least one level");
    let floor = Level::DEFAULT_FLOOR_THICKNESS;
    let count = levels.len();
    let level = Level {
        id: doc.new_level_id(),
        name: name.unwrap_or_else(|| format!("{count}º andar")),
        elevation: top.elevation + top.height + floor,
        height: height.unwrap_or(Level::DEFAULT_HEIGHT),
        floor_thickness: floor,
        ..Level::default()
    };
    let id = level.id;
    commands.push(Command::insert(level));
    doc.execute(Command::Batch { commands })?;
    doc.select_level(Some(id));
    Ok(id)
}

/// Deletes a storey and everything on it, as one undoable step.
pub fn delete_level(doc: &mut Document, id: crate::ids::LevelId) -> CoreResult<()> {
    let home = doc.home();
    if home.level(id).is_none() {
        return Err(CoreError::NotFound(id.into()));
    }
    let mut commands: Vec<Command> = home
        .elements()
        .filter(|e| !matches!(e, Element::Level(_)) && home.on_level(e.level(), Some(id)))
        .map(|e| Command::remove(e.id()))
        .collect();
    commands.push(Command::remove(id));
    doc.execute(Command::Batch { commands })?;
    let still_selected = doc
        .home()
        .selected_level
        .is_some_and(|s| doc.home().level(s).is_some());
    if !still_selected {
        let base = doc.home().base_level();
        doc.select_level(base);
    }
    Ok(())
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
            level: None,
            ..Default::default()
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
    fn levels_stack_and_views_keep_storeys_apart() {
        let (mut doc, a, _) = doc_with_l();
        let upper = add_level(&mut doc, None, None).unwrap();
        let home = doc.home();
        assert_eq!(
            home.levels.len(),
            2,
            "ground level created for existing walls"
        );
        let ground = home.base_level().unwrap();
        assert_eq!(home.selected_level, Some(upper));
        assert!((home.level(upper).unwrap().elevation - 262.0).abs() < 1e-9);

        // New wall on the upper level.
        let mut wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(100.0, 0.0),
        );
        wall.level = Some(upper);
        doc.execute(Command::insert(wall)).unwrap();
        let home = doc.home();
        assert_eq!(
            home.level_view(Some(ground)).walls.len(),
            2,
            "unassigned walls sit on the ground"
        );
        assert_eq!(home.level_view(Some(upper)).walls.len(), 1);
        assert!(home.level_view(Some(upper)).wall(a).is_none());

        delete_level(&mut doc, upper).unwrap();
        assert_eq!(
            doc.home().walls.len(),
            2,
            "upper walls removed with their level"
        );
        assert_eq!(doc.home().current_level(), Some(ground));
        doc.undo().unwrap();
        assert_eq!(
            doc.home().walls.len(),
            3,
            "deleting a level is one undo step"
        );
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
