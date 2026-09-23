//! Advisory only: a small empty kitchen corner enclosed by two installations.
//! The geometry cannot establish whether this is deliberate service clearance.

use geo::{Area, BooleanOps};
use newera_core::{OpeningKind, Point2};

use crate::{Finding, RoomUse, Scene, Severity, Use, scene::polygon as to_polygon};

#[allow(clippy::too_many_lines)]
pub(crate) fn review(scene: &Scene<'_>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for space in &scene.spaces {
        if space.what != RoomUse::Kitchen || space.room.points.len() < 3 {
            continue;
        }
        let points = &space.room.points;
        let room = to_polygon(points);
        for (index, &corner) in points.iter().enumerate() {
            let direction = |p: Point2| {
                let length = p.distance(corner);
                ((p.x - corner.x) / length, (p.y - corner.y) / length)
            };
            let u = direction(points[(index + 1) % points.len()]);
            let v = direction(points[(index + points.len() - 1) % points.len()]);
            if !(u.0 * v.0 + u.1 * v.1).is_finite() || (u.0 * v.0 + u.1 * v.1).abs() > 1e-6 {
                continue;
            }
            let at = |x: f64, y: f64| {
                Point2::new(corner.x + x * u.0 + y * v.0, corner.y + x * u.1 + y * v.1)
            };
            // A boundary between open-plan zones is not a walled corner.
            if ![at(20.0, 0.0), at(0.0, 20.0)].iter().all(|p| {
                scene.home.walls.iter().any(|wall| {
                    p.distance_to_segment(wall.start, wall.end) <= wall.thickness / 2.0 + 2.0
                })
            }) {
                continue;
            }
            let project = |p: Point2| {
                let (x, y) = (p.x - corner.x, p.y - corner.y);
                (x * u.0 + y * u.1, x * v.0 + y * v.1)
            };
            let mut solids = Vec::new();
            let mut edges = Vec::new();
            for &i in &space.units {
                let unit = &scene.units[i];
                let (low, high) = unit.piece.height_range();
                // Floor storage/appliances and worktops, not ceiling cabinets,
                // lights or rugs. Every floor solid can occupy the candidate.
                if low >= 95.0 || high <= 5.0 {
                    continue;
                }
                let footprint = unit.piece.projected_footprint();
                solids.push(to_polygon(&footprint));
                if !matches!(
                    unit.what,
                    Use::Fridge | Use::Counter | Use::Sink | Use::Stove | Use::Appliance
                ) {
                    continue;
                }
                let bounds = footprint.iter().map(|p| project(*p)).fold(
                    [
                        f64::INFINITY,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        f64::NEG_INFINITY,
                    ],
                    |[x0, y0, x1, y1], (x, y)| [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                );
                edges.push((unit, bounds));
            }
            let mut candidates = Vec::new();
            for (a, [ax, ay, _, ay1]) in &edges {
                for (b, [bx, by, bx1, _]) in &edges {
                    if a.piece.id == b.piece.id || !(25.0..=120.0).contains(ax)
                        || !(25.0..=120.0).contains(by) || ay.abs() > 15.0 || bx.abs() > 15.0
                        // A normal aisle/open corner is not wasted space.
                        || by - ay1 > 30.0 || ax - bx1 > 30.0 || ax * by < 1600.0
                    {
                        continue;
                    }
                    let zone =
                        to_polygon(&[at(0.0, 0.0), at(*ax, 0.0), at(*ax, *by), at(0.0, *by)]);
                    if zone.difference(&room).unsigned_area() > 1.0
                        || solids
                            .iter()
                            .any(|s| zone.intersection(s).unsigned_area() > 1.0)
                    {
                        continue;
                    }
                    let approach = to_polygon(&[
                        at(-15.0, -15.0),
                        at(ax + 15.0, -15.0),
                        at(ax + 15.0, by + 15.0),
                        at(-15.0, by + 15.0),
                    ]);
                    if scene
                        .home
                        .furniture
                        .iter()
                        .filter(|f| f.visible)
                        .flat_map(|f| f.visible_leaves())
                        .any(|f| {
                            f.opening
                                .as_ref()
                                .is_some_and(|o| o.kind != OpeningKind::Window)
                                && approach
                                    .intersection(&to_polygon(&f.projected_footprint()))
                                    .unsigned_area()
                                    > 1.0
                        })
                    {
                        continue;
                    }
                    candidates.push((ax * by, *ax, *by, a, b));
                }
            }
            if let Some((area, width, depth, a, b)) =
                candidates.into_iter().min_by(|a, b| a.0.total_cmp(&b.0))
            {
                let mut ids = [a.piece.id.to_string(), b.piece.id.to_string()];
                ids.sort();
                findings.push(Finding {
                    severity: Severity::Dica,
                    place: space.label(),
                    key: crate::rule::key_at(
                        crate::rule::Rule::UnusedCorner,
                        &space.label(),
                        &format!("{}+{}", ids[0], ids[1]),
                    ),
                    message: format!(
                        "Possível canto ocioso junto a ({:.1}, {:.1}) cm: {:.1} × {:.1} cm ({:.2} m²), entre {} e {}. Há uma sobra delimitada pelos móveis e pelas paredes, com acesso estreito. Feedback de aproveitamento, não defeito confirmado: confira se é folga de ventilação, manutenção ou abertura antes de decidir usá-la. Nenhuma alteração automática.",
                        corner.x, corner.y, width, depth, area / 10_000.0, a.label(), b.label()
                    ),
                    ..Finding::default()
                });
            }
        }
    }
    findings
}
