//! Drawing annotations derived from the model: engineering dimension chains
//! and room reference schedules. Nothing here is stored; it is recomputed
//! from walls, openings, rooms and furniture whenever the plan is drawn.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::elements::Dimension;
use crate::furniture::Furniture;
use crate::geometry::{Point2, polygon_area};
use crate::home::Home;
use crate::ids::{FurnitureId, RoomId};

/// Which derived annotations the plan shows. Saved with the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct PlanAnnotations {
    /// Dimension chains around the building and inside rooms.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_dimensions: bool,
    /// Numbered tags on furniture and a schedule of rooms and their contents.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub references: bool,
    /// Add brand, model and link to the schedule.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reference_details: bool,
    /// Legend of the electrical and plumbing symbols used, with counts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub legend: bool,
}

/// Distance of the first chain from the outer wall faces, cm.
const FIRST_CHAIN: f64 = 60.0;
/// Distance between successive chains, cm.
const CHAIN_GAP: f64 = 45.0;
/// How far from the outer face a wall end may stop and still meet the façade
/// (covers thick walls and T junctions), cm.
const FACADE_REACH: f64 = 40.0;
/// Stops closer than this are merged, cm.
const MIN_STEP: f64 = 1.0;

/// Lowercased and stripped of accents, so `porta` finds `Portão` and a
/// query typed without accents still matches a plan written with them.
#[must_use]
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'ë' => 'e',
            'í' | 'î' | 'ï' => 'i',
            'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

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

fn bounds(points: impl IntoIterator<Item = Point2>) -> Option<(Point2, Point2)> {
    points.into_iter().fold(None, |acc, p| {
        let (lo, hi) = acc.unwrap_or((p, p));
        Some((
            Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
        ))
    })
}

/// A dimension chain along one side of a rectangle-like footprint.
fn chain(stops: &mut Vec<f64>, make: &dyn Fn(f64, f64) -> Dimension, out: &mut Vec<Dimension>) {
    stops.sort_by(f64::total_cmp);
    stops.dedup_by(|a, b| (*a - *b).abs() < MIN_STEP);
    for pair in stops.windows(2) {
        if pair[1] - pair[0] >= MIN_STEP {
            out.push(make(pair[0], pair[1]));
        }
    }
}

/// Engineering dimensions for one level view: per façade a chain of wall
/// openings, a chain of wall axes and the overall size, plus the clear inner
/// width and depth of every rectangular room.
pub fn auto_dimensions(home: &Home) -> Vec<Dimension> {
    let mut out = Vec::new();
    let outlines = home.wall_outlines();
    let Some((min, max)) = bounds(outlines.iter().flatten().copied()) else {
        return out;
    };
    let cuts = home.wall_cuts();
    let dim = |start: Point2, end: Point2, offset: f64| Dimension {
        start,
        end,
        offset,
        level: None,
        ..Dimension::default()
    };

    // Sides: (is horizontal, coordinate of the outer face, sign of "outside").
    for (horizontal, face, outward) in [
        (true, min.y, -1.0),
        (true, max.y, 1.0),
        (false, min.x, -1.0),
        (false, max.x, 1.0),
    ] {
        let (lo, hi) = if horizontal {
            (min.x, max.x)
        } else {
            (min.y, max.y)
        };
        let at = |along: f64| {
            if horizontal {
                Point2::new(along, face)
            } else {
                Point2::new(face, along)
            }
        };
        // Runs go +x (horizontal) or +y (vertical); positive offsets lie to
        // the left of the run: -y for +x runs, +x for +y runs.
        let offset_sign = if horizontal { -outward } else { outward };
        let offset = |k: usize| {
            #[allow(clippy::cast_precision_loss)]
            let distance = FIRST_CHAIN + CHAIN_GAP * k as f64;
            offset_sign * distance
        };
        let make = |k: usize| move |a: f64, b: f64| dim(at(a), at(b), offset(k));

        // Walls running along this façade (their outer face on the side).
        let mut opening_stops = vec![lo, hi];
        let mut axis_stops = vec![lo, hi];
        let mut has_openings = false;
        for ((wall, outline), wall_cuts) in home.walls.iter().zip(&outlines).zip(&cuts) {
            let across = |p: Point2| if horizontal { p.y } else { p.x };
            let along = |p: Point2| if horizontal { p.x } else { p.y };
            let (a, b) = (along(wall.start), along(wall.end));
            let parallel = (across(wall.start) - across(wall.end)).abs() < 1.0;
            if parallel {
                if !outline.iter().any(|p| (across(*p) - face).abs() < 1.0) {
                    continue;
                }
                let len = wall.start.distance(wall.end).max(1e-9);
                for cut in wall_cuts {
                    has_openings = true;
                    for s in [cut.from, cut.to] {
                        let t = s / len;
                        opening_stops.push(a + (b - a) * t);
                    }
                }
            } else {
                // A wall meeting the façade (an end within reach of the outer
                // face): dimension to its axis. Corner walls are the totals.
                let reach = FACADE_REACH;
                let meets = [wall.start, wall.end]
                    .iter()
                    .any(|p| (across(*p) - face).abs() <= reach);
                let axis = along(wall.start).midpoint(along(wall.end));
                if meets && axis - lo > reach && hi - axis > reach {
                    axis_stops.push(axis);
                }
            }
        }
        let mut k = 0;
        if has_openings {
            chain(&mut opening_stops, &make(k), &mut out);
            k += 1;
        }
        if axis_stops.len() > 2 {
            chain(&mut axis_stops, &make(k), &mut out);
            k += 1;
        }
        out.push(dim(at(lo), at(hi), offset(k)));
    }

    // Clear sizes inside rectangular rooms, crossing at the room center.
    for room in &home.rooms {
        let Some((lo, hi)) = bounds(room.points.iter().copied()) else {
            continue;
        };
        let box_area = (hi.x - lo.x) * (hi.y - lo.y);
        if box_area < 1.0 || (polygon_area(&room.points) - box_area).abs() > box_area * 0.03 {
            continue;
        }
        let center = Point2::new(lo.x.midpoint(hi.x), lo.y.midpoint(hi.y));
        let y = center.y + (hi.y - lo.y) * 0.25;
        let x = center.x + (hi.x - lo.x) * 0.25;
        out.push(dim(Point2::new(lo.x, y), Point2::new(hi.x, y), 0.0));
        out.push(dim(Point2::new(x, lo.y), Point2::new(x, hi.y), 0.0));
    }
    out
}

/// One piece listed in a room schedule.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ReferenceItem {
    pub piece: FurnitureId,
    /// Number shown on the plan next to the piece.
    pub tag: usize,
    pub name: String,
    /// Width, depth, height, cm.
    pub size: [f64; 3],
    pub position: Point2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A room and the pieces standing in it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct RoomReference {
    /// `None` gathers pieces outside every room.
    pub room: Option<RoomId>,
    pub name: String,
    /// Floor area, cm² (`None` for the pieces outside every room).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<f64>,
    pub items: Vec<ReferenceItem>,
}

/// Where a piece keeps its reference number once it has one.
pub const TAG_KEY: &str = "ref:tag";

/// The reference number stored on a piece.
fn stored_tag(piece: &Furniture) -> Option<usize> {
    piece.properties.get(TAG_KEY)?.parse().ok()
}

/// Pieces listed in the schedule that have no number yet, numbered: the next
/// free numbers after the highest in use, in reading order.
///
/// The number is what goes to the joiner and into the quote, so it has to
/// stay with the piece. Derived from the list it moved whenever anything
/// before it came or went — item 92, the kitchen wall cabinet, became 88 —
/// and two prints a week apart disagreed in silence. Once given, a number is
/// the piece's: new pieces take the next one, removed pieces leave a gap.
#[must_use]
pub fn untagged_references(home: &Home) -> Vec<Furniture> {
    if !home.annotations.references {
        return Vec::new();
    }
    let mut next = home
        .furniture
        .iter()
        .filter_map(stored_tag)
        .max()
        .unwrap_or(0);
    let mut out = Vec::new();
    for group in derived_references(home) {
        for item in group.items {
            let Some(piece) = home.piece(item.piece) else {
                continue;
            };
            if stored_tag(piece).is_some() {
                continue;
            }
            next += 1;
            let mut tagged = piece.clone();
            tagged
                .properties
                .insert(TAG_KEY.to_owned(), next.to_string());
            out.push(tagged);
        }
    }
    out
}

/// Every listed piece with its number taken away, to number them again in
/// reading order — the explicit, visible way to close the gaps.
#[must_use]
pub fn cleared_references(home: &Home) -> Vec<Furniture> {
    home.furniture
        .iter()
        .filter(|f| f.properties.contains_key(TAG_KEY))
        .map(|f| {
            let mut cleared = f.clone();
            cleared.properties.remove(TAG_KEY);
            cleared
        })
        .collect()
}

/// Rooms (smallest containing room wins) with the furniture inside, tagged
/// in reading order. Doors, windows and technical points are left out.
///
/// A piece that keeps a number ([`untagged_references`]) is listed with it;
/// the others are numbered after the highest kept, in reading order.
pub fn room_references(home: &Home) -> Vec<RoomReference> {
    let mut groups = derived_references(home);
    let mut next = home
        .furniture
        .iter()
        .filter_map(stored_tag)
        .max()
        .unwrap_or(0);
    let kept = next > 0;
    for group in &mut groups {
        for item in &mut group.items {
            match home.piece(item.piece).and_then(stored_tag) {
                Some(tag) => item.tag = tag,
                None if kept => {
                    next += 1;
                    item.tag = next;
                }
                None => {}
            }
        }
    }
    groups
}

/// The schedule numbered by reading order alone.
fn derived_references(home: &Home) -> Vec<RoomReference> {
    let mut groups: Vec<RoomReference> = home
        .rooms
        .iter()
        .map(|r| RoomReference {
            room: Some(r.id),
            name: if r.name.trim().is_empty() {
                "Cômodo".to_owned()
            } else {
                r.name.clone()
            },
            area: Some(r.area()),
            items: Vec::new(),
        })
        .collect();
    let mut others = RoomReference {
        room: None,
        name: "Outros".to_owned(),
        area: None,
        items: Vec::new(),
    };
    let listed = |f: &&Furniture| f.visible && !f.is_opening() && f.discipline.is_none();
    for piece in home.furniture.iter().filter(listed) {
        let item = ReferenceItem {
            piece: piece.id,
            tag: 0,
            name: piece.name.clone(),
            size: [piece.width, piece.depth, piece.height],
            position: piece.position,
            brand: piece.info.brand.clone(),
            model: piece.info.model_name.clone(),
            url: piece.info.url.clone(),
        };
        let room = home
            .rooms
            .iter()
            .enumerate()
            .filter(|(_, r)| r.points.len() >= 3 && inside(&r.points, piece.position))
            .min_by(|(_, a), (_, b)| polygon_area(&a.points).total_cmp(&polygon_area(&b.points)))
            .map(|(i, _)| i);
        match room {
            Some(i) => groups[i].items.push(item),
            None => others.items.push(item),
        }
    }
    groups.push(others);
    groups.retain(|g| !g.items.is_empty());
    let mut tag = 0;
    for group in &mut groups {
        group.items.sort_by(|a, b| {
            (a.position.y, a.position.x)
                .partial_cmp(&(b.position.y, b.position.x))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for item in &mut group.items {
            tag += 1;
            item.tag = tag;
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{Room, Wall};
    use crate::furniture::{Opening, align_to_wall};
    use crate::ids::WallId;

    fn house() -> Home {
        let mut home = Home::default();
        let pts = [(0.0, 0.0), (600.0, 0.0), (600.0, 400.0), (0.0, 400.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let id = home.new_wall_id();
            home.walls
                .push(Wall::new(id, Point2::new(a.0, a.1), Point2::new(b.0, b.1)));
        }
        // Partition at x = 250.
        let id = home.new_wall_id();
        home.walls.push(Wall::new(
            id,
            Point2::new(250.0, 0.0),
            Point2::new(250.0, 400.0),
        ));
        let id = home.new_room_id();
        home.rooms.push(Room::new(
            id,
            "Sala de estar",
            vec![
                Point2::new(257.5, 7.5),
                Point2::new(592.5, 7.5),
                Point2::new(592.5, 392.5),
                Point2::new(257.5, 392.5),
            ],
        ));
        home
    }

    #[test]
    fn facades_get_opening_axis_and_total_chains() {
        let mut home = house();
        let mut window = Furniture {
            id: home.new_furniture_id(),
            width: 120.0,
            depth: 15.0,
            height: 120.0,
            elevation: 100.0,
            opening: Some(Opening::default()),
            ..Furniture::default()
        };
        let top = home.wall(WallId(1)).unwrap().clone();
        align_to_wall(&mut window, &top, 400.0);
        home.furniture.push(window);

        let dims = auto_dimensions(&home);
        // Total outer width 615 cm (walls 15 cm thick) on top and bottom.
        let totals = dims
            .iter()
            .filter(|d| (d.length() - 615.0).abs() < 0.5)
            .count();
        assert!(totals >= 2, "{dims:?}");
        // The window splits the top opening chain: its 120 cm width appears.
        assert!(
            dims.iter().any(|d| (d.length() - 120.0).abs() < 0.5),
            "{dims:?}"
        );
        // The partition splits an axis chain at x = 250.
        assert!(
            dims.iter()
                .any(|d| (d.end.x - 250.0).abs() < 0.5 && d.start.y < 1.0),
            "{dims:?}"
        );
        // Outer chains sit outside: top ones above the building.
        assert!(
            dims.iter()
                .filter(|d| d.start.y < -1.0 && (d.start.y - d.end.y).abs() < 0.1)
                .all(|d| d.offset > 0.0)
        );
        // Room clear size 335 × 385.
        assert!(dims.iter().any(|d| (d.length() - 335.0).abs() < 0.5));
        assert!(dims.iter().any(|d| (d.length() - 385.0).abs() < 0.5));
    }

    #[test]
    fn references_list_pieces_per_room_with_tags() {
        let mut home = house();
        for (name, x, y) in [
            ("Sofá", 400.0, 300.0),
            ("Poltrona", 300.0, 100.0),
            ("Vaso", 100.0, 100.0),
        ] {
            let id = home.new_furniture_id();
            home.furniture.push(Furniture {
                id,
                name: name.into(),
                position: Point2::new(x, y),
                ..Furniture::default()
            });
        }
        home.furniture[0].info.brand = Some("Tok&Stok".into());
        let refs = room_references(&home);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "Sala de estar");
        let names: Vec<_> = refs[0]
            .items
            .iter()
            .map(|i| (i.tag, i.name.as_str()))
            .collect();
        assert_eq!(names, vec![(1, "Poltrona"), (2, "Sofá")]);
        assert_eq!(refs[0].items[1].brand.as_deref(), Some("Tok&Stok"));
        assert_eq!((refs[1].name.as_str(), refs[1].items[0].tag), ("Outros", 3));
    }
}
