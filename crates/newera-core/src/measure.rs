//! Measuring a plan the way someone with a tape measure would.
//!
//! Three questions come up over and over while reviewing a layout, and all
//! three used to force whoever asked them to rebuild the geometry by hand:
//!
//! * *Where exactly is this piece?* — [`plan_bounds`] resolves `angle`,
//!   mirroring and tilt into an axis-aligned box, and [`facing`] says which
//!   way its front looks.
//! * *How much room is there in front of it?* — [`clearance`].
//! * *What do I run into crossing the room here?* — [`free_span`].
//!
//! Everything works on one storey: pass a [`Home::level_view`] (or a home
//! without levels) so a piece upstairs never blocks a corridor downstairs.

use geo::{Area, BooleanOps, Polygon};

use crate::elements::Wall;
use crate::furniture::Furniture;
use crate::geometry::Point2;
use crate::home::Home;
use crate::ids::{ElementId, FurnitureId, WallId};

/// Pieces this thin (rugs, mats, glass panes seen edge on) never block.
const FLAT: f64 = 2.0;
/// How far a measurement looks before giving up, cm.
pub const MAX_REACH: f64 = 2000.0;

/// A plan axis. `X` grows right, `Y` grows down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }

    fn of(self, p: Point2) -> f64 {
        match self {
            Self::X => p.x,
            Self::Y => p.y,
        }
    }

    fn across(self) -> Self {
        match self {
            Self::X => Self::Y,
            Self::Y => Self::X,
        }
    }
}

/// One of the four plan directions, as an agent writes it: `+x`, `-y`…
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dir {
    pub axis: Axis,
    /// `1.0` toward growing coordinates, `-1.0` the other way.
    pub sign: f64,
}

impl Dir {
    pub const PLAN: [Self; 4] = [
        Self {
            axis: Axis::X,
            sign: 1.0,
        },
        Self {
            axis: Axis::X,
            sign: -1.0,
        },
        Self {
            axis: Axis::Y,
            sign: 1.0,
        },
        Self {
            axis: Axis::Y,
            sign: -1.0,
        },
    ];

    /// `+x`, `-x`, `+y`, `-y`, or a piece-relative `front`, `back`, `left`,
    /// `right` resolved against `piece`'s angle.
    pub fn parse(raw: &str, piece: Option<&Furniture>) -> Option<Self> {
        let raw = raw.trim().to_ascii_lowercase();
        let plan = |axis: Axis, sign: f64| Some(Self { axis, sign });
        match raw.as_str() {
            "+x" | "x" | "right_plan" | "east" => return plan(Axis::X, 1.0),
            "-x" | "left_plan" | "west" => return plan(Axis::X, -1.0),
            "+y" | "y" | "down" | "south" => return plan(Axis::Y, 1.0),
            "-y" | "up" | "north" => return plan(Axis::Y, -1.0),
            _ => {}
        }
        // Piece-relative: its front is local +y, turned by its angle.
        let piece = piece?;
        let local = match raw.as_str() {
            "front" => 0.0,
            "back" => 180.0,
            "left" => 90.0,
            "right" => -90.0,
            _ => return None,
        };
        Some(snap(piece.angle + local))
    }

    pub fn name(self) -> &'static str {
        match (self.axis, self.sign >= 0.0) {
            (Axis::X, true) => "+x",
            (Axis::X, false) => "-x",
            (Axis::Y, true) => "+y",
            (Axis::Y, false) => "-y",
        }
    }
}

/// The plan direction a piece's front looks at, snapped to the nearest
/// quarter turn. A piece's front is its local `+y` before rotation.
fn snap(angle_degrees: f64) -> Dir {
    let a = angle_degrees.rem_euclid(360.0);
    // angle 0 → front at +y; 90 → front at -x (plan y grows down, x right,
    // and `to_plan` turns local +y toward -x for a clockwise angle).
    #[allow(clippy::cast_possible_truncation)]
    let quarter = ((a / 90.0).round() as i64).rem_euclid(4);
    match quarter {
        0 => Dir {
            axis: Axis::Y,
            sign: 1.0,
        },
        1 => Dir {
            axis: Axis::X,
            sign: -1.0,
        },
        2 => Dir {
            axis: Axis::Y,
            sign: -1.0,
        },
        _ => Dir {
            axis: Axis::X,
            sign: 1.0,
        },
    }
}

/// Which way a piece's front looks, as `+x`, `-x`, `+y` or `-y`.
///
/// For a cabinet this is the side its doors open toward, which decides
/// whether it can be used at all — and is invisible in `angle` alone.
pub fn facing(piece: &Furniture) -> &'static str {
    // Mirroring flips the piece's local x (a left-hinged door becomes
    // right-hinged); the front stays local +y, so it does not move.
    snap(piece.angle).name()
}

/// Axis-aligned plan box of a piece, `angle`, mirroring and tilt applied.
///
/// This is the `bounds` every read returns, so nobody has to swap width and
/// depth by hand when a piece is turned a quarter turn.
pub fn plan_bounds(piece: &Furniture) -> (Point2, Point2) {
    let corners = piece.projected_footprint();
    let mut min = Point2::new(f64::MAX, f64::MAX);
    let mut max = Point2::new(f64::MIN, f64::MIN);
    for c in corners {
        min = Point2::new(min.x.min(c.x), min.y.min(c.y));
        max = Point2::new(max.x.max(c.x), max.y.max(c.y));
    }
    (min, max)
}

/// Axis-aligned plan box of a wall, its thickness and corner joins applied.
pub fn wall_bounds(home: &Home, wall: &Wall) -> (Point2, Point2) {
    let outlines = home.wall_outlines();
    let outline = home
        .walls
        .iter()
        .position(|w| w.id == wall.id)
        .and_then(|i| outlines.get(i))
        .filter(|o| o.len() >= 3);
    // A wall with no outline (zero length, or not joined yet) still has a
    // footprint: its two ends grown by half its thickness.
    let half = wall.thickness / 2.0;
    let ends = vec![
        Point2::new(wall.start.x - half, wall.start.y - half),
        Point2::new(wall.start.x + half, wall.start.y + half),
        Point2::new(wall.end.x - half, wall.end.y - half),
        Point2::new(wall.end.x + half, wall.end.y + half),
    ];
    let mut min = Point2::new(f64::MAX, f64::MAX);
    let mut max = Point2::new(f64::MIN, f64::MIN);
    for p in outline.unwrap_or(&ends) {
        min = Point2::new(min.x.min(p.x), min.y.min(p.y));
        max = Point2::new(max.x.max(p.x), max.y.max(p.y));
    }
    (min, max)
}

/// Plan box of any element, for `measure` and for enriched reports.
pub fn element_bounds(home: &Home, id: ElementId) -> Option<(Point2, Point2)> {
    let hull = |points: &[Point2]| {
        let mut min = Point2::new(f64::MAX, f64::MAX);
        let mut max = Point2::new(f64::MIN, f64::MIN);
        for p in points {
            min = Point2::new(min.x.min(p.x), min.y.min(p.y));
            max = Point2::new(max.x.max(p.x), max.y.max(p.y));
        }
        (min, max)
    };
    match id {
        ElementId::Furniture(f) => home.piece(f).map(plan_bounds),
        ElementId::Wall(w) => home.wall(w).map(|w| wall_bounds(home, w)),
        ElementId::Room(r) => home.room(r).map(|r| hull(&r.points)),
        ElementId::Dimension(d) => home.dimension(d).map(|d| hull(&[d.start, d.end])),
        ElementId::Label(l) => home.label(l).map(|l| hull(&[l.position])),
        ElementId::Polyline(p) => home.polyline(p).map(|p| hull(&p.points)),
        ElementId::Level(_) => None,
    }
}

/// What a probe ran into.
#[derive(Debug, Clone, PartialEq)]
pub enum Solid {
    Wall(WallId),
    Piece(FurnitureId),
}

impl Solid {
    pub fn id(&self) -> ElementId {
        match *self {
            Self::Wall(w) => ElementId::Wall(w),
            Self::Piece(f) => ElementId::Furniture(f),
        }
    }
}

/// Something that stops a tape measure: its outline and height range.
#[derive(Debug, Clone)]
pub struct Obstacle {
    pub what: Solid,
    pub name: String,
    pub outline: Vec<Point2>,
    /// Bottom and top above this storey's floor, cm.
    pub z: (f64, f64),
}

impl Obstacle {
    fn polygon(&self) -> Polygon<f64> {
        crate::geometry::to_polygon(&self.outline)
    }

    fn spans(&self, axis: Axis) -> (f64, f64) {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for p in &self.outline {
            lo = lo.min(axis.of(*p));
            hi = hi.max(axis.of(*p));
        }
        (lo, hi)
    }
}

/// Whether a piece is worth measuring against: not an opening, not a rug,
/// not a part built into its host.
fn blocks(piece: &Furniture) -> bool {
    piece.visible
        && !piece.is_opening()
        && piece.height > FLAT
        && piece.width.min(piece.depth) > FLAT
        && piece.discipline.is_none()
        && !piece.properties.contains_key("joinery:embedded")
}

/// Every solid of one storey, walls first. `skip` leaves pieces out — the one
/// being measured from, typically.
pub fn obstacles(home: &Home, skip: &dyn Fn(&Furniture) -> bool) -> Vec<Obstacle> {
    let mut out = Vec::new();
    for (wall, outline) in home.walls.iter().zip(home.wall_outlines()) {
        if outline.len() < 3 {
            continue;
        }
        out.push(Obstacle {
            what: Solid::Wall(wall.id),
            name: wall
                .wall_type
                .clone()
                .unwrap_or_else(|| "parede".to_owned()),
            outline,
            z: (0.0, wall.height.max(wall.height_at_end.unwrap_or(0.0))),
        });
    }
    for top in &home.furniture {
        for leaf in top.visible_leaves() {
            if !blocks(leaf) || skip(leaf) {
                continue;
            }
            out.push(Obstacle {
                what: Solid::Piece(leaf.id),
                name: leaf.name.clone(),
                outline: leaf.projected_footprint().to_vec(),
                z: leaf.height_range(),
            });
        }
    }
    out
}

/// A stretch of a straight probe: either free floor or one solid.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    /// Start along the axis, cm.
    pub from: f64,
    /// End along the axis, cm.
    pub to: f64,
    /// `None` for free floor.
    pub what: Option<Solid>,
    /// Name of the solid, empty for free floor.
    pub name: String,
}

impl Span {
    pub fn cm(&self) -> f64 {
        self.to - self.from
    }

    pub fn is_free(&self) -> bool {
        self.what.is_none()
    }
}

/// Where a line at `at` (the other axis' coordinate) enters and leaves a
/// polygon, as intervals along `axis`. Even-odd crossings of the edges.
fn crossings(outline: &[Point2], axis: Axis, at: f64) -> Vec<(f64, f64)> {
    let across = axis.across();
    let mut hits: Vec<f64> = Vec::new();
    for k in 0..outline.len() {
        let (a, b) = (outline[k], outline[(k + 1) % outline.len()]);
        let (ca, cb) = (across.of(a), across.of(b));
        // Half-open rule: a vertex counts for the edge below it only, so a
        // line grazing a corner crosses once, not twice.
        if (ca <= at && cb > at) || (cb <= at && ca > at) {
            let t = (at - ca) / (cb - ca);
            hits.push(axis.of(a) + t * (axis.of(b) - axis.of(a)));
        }
    }
    hits.sort_by(f64::total_cmp);
    hits.as_chunks::<2>()
        .0
        .iter()
        .map(|c| (c[0], c[1]))
        .collect()
}

/// What a straight probe runs into, in order.
///
/// The probe runs along `axis` at `at` (the other axis' coordinate), between
/// `range` if given (otherwise the whole drawing), counting only solids whose
/// height range meets `z` — so `z = (0, 200)` ignores a wall cabinet at 150
/// only if it starts above 200, and `z = (90, 91)` follows a countertop line.
///
/// This is the answer to "how wide is the corridor here, and between what?"
/// in a single call.
pub fn free_span(
    home: &Home,
    axis: Axis,
    at: f64,
    range: Option<(f64, f64)>,
    z: (f64, f64),
) -> Vec<Span> {
    let (lo, hi) = range
        .or_else(|| home.bounds().map(|(a, b)| (axis.of(a), axis.of(b))))
        .unwrap_or((0.0, 0.0));
    if hi <= lo {
        return Vec::new();
    }
    let mut hits: Vec<Span> = Vec::new();
    for o in obstacles(home, &|_| false) {
        if o.z.1 <= z.0 || o.z.0 >= z.1 {
            continue;
        }
        for (a, b) in crossings(&o.outline, axis, at) {
            let (a, b) = (a.max(lo), b.min(hi));
            if b - a > 0.05 {
                hits.push(Span {
                    from: a,
                    to: b,
                    what: Some(o.what.clone()),
                    name: o.name.clone(),
                });
            }
        }
    }
    hits.sort_by(|a, b| a.from.total_cmp(&b.from));
    let mut out: Vec<Span> = Vec::new();
    let mut cursor = lo;
    for hit in hits {
        if hit.from > cursor + 0.05 {
            out.push(Span {
                from: cursor,
                to: hit.from,
                what: None,
                name: String::new(),
            });
        }
        // Solids that overlap along the probe are reported as they come; the
        // cursor only ever moves forward so the spans stay in order.
        if hit.to > cursor {
            let from = hit.from.max(cursor);
            cursor = hit.to;
            out.push(Span {
                from,
                to: hit.to,
                ..hit
            });
        }
    }
    if hi > cursor + 0.05 {
        out.push(Span {
            from: cursor,
            to: hi,
            what: None,
            name: String::new(),
        });
    }
    out
}

/// Free floor on one side of a piece, and what stops it.
#[derive(Debug, Clone, PartialEq)]
pub struct Clearance {
    pub dir: Dir,
    /// Free centimeters from that face, capped at [`MAX_REACH`].
    pub cm: f64,
    /// `None` when nothing was found within reach.
    pub against: Option<Solid>,
    pub name: String,
}

/// Free floor in front of (or beside, or behind) a piece.
///
/// Measured over the whole face, from the piece's axis-aligned box, ignoring
/// whatever is built into it (a cooktop in its countertop) and whatever
/// passes over a person's head.
pub fn clearance(home: &Home, piece: &Furniture, dir: Dir, max: f64) -> Clearance {
    let own: Vec<crate::ids::FurnitureId> = piece.flatten().iter().map(|f| f.id).collect();
    let solids = obstacles(home, &|f| own.contains(&f.id));
    clearance_against(&solids, piece, dir, max)
}

/// [`clearance`] against solids already gathered.
///
/// Building the list means walking every wall and every piece of the storey,
/// so a caller measuring many pieces of the same storey — a layout check, a
/// dry run — gathers it once instead of once per piece.
///
/// The piece being measured is expected to be absent from `solids`; it would
/// otherwise be its own obstacle.
pub fn clearance_against(solids: &[Obstacle], piece: &Furniture, dir: Dir, max: f64) -> Clearance {
    let (min, max_pt) = plan_bounds(piece);
    let z = piece.height_range();
    let across = dir.axis.across();
    // The band in front of that face, 2 cm in from the corners so a
    // neighbour that merely touches a corner does not count.
    let (b0, b1) = match across {
        Axis::X => (min.x + 2.0, max_pt.x - 2.0),
        Axis::Y => (min.y + 2.0, max_pt.y - 2.0),
    };
    let face = if dir.sign >= 0.0 {
        dir.axis.of(max_pt)
    } else {
        dir.axis.of(min)
    };
    let (f0, f1) = if dir.sign >= 0.0 {
        (face + 0.1, face + max)
    } else {
        (face - max, face - 0.1)
    };
    let band = match dir.axis {
        Axis::X => [
            Point2::new(f0, b0),
            Point2::new(f1, b0),
            Point2::new(f1, b1),
            Point2::new(f0, b1),
        ],
        Axis::Y => [
            Point2::new(b0, f0),
            Point2::new(b1, f0),
            Point2::new(b1, f1),
            Point2::new(b0, f1),
        ],
    };
    if b1 <= b0 {
        return Clearance {
            dir,
            cm: max,
            against: None,
            name: String::new(),
        };
    }
    let band_poly = crate::geometry::to_polygon(&band);
    let mut best = max;
    let mut against = None;
    let mut name = String::new();
    let own: Vec<crate::ids::FurnitureId> = piece.flatten().iter().map(|f| f.id).collect();
    for o in solids {
        if matches!(o.what, Solid::Piece(id) if own.contains(&id)) {
            continue;
        }
        // Over your head or under your feet: not in the way.
        if o.z.1 <= z.0 + 1.0 || o.z.0 >= z.1 - 1.0 {
            continue;
        }
        let shared = o.polygon().intersection(&band_poly);
        if shared.unsigned_area() <= 25.0 {
            continue;
        }
        let (lo, hi) = o.spans(dir.axis);
        let d = if dir.sign >= 0.0 {
            lo - face
        } else {
            face - hi
        };
        if d.max(0.0) < best {
            best = d.max(0.0);
            against = Some(o.what.clone());
            name.clone_from(&o.name);
        }
    }
    Clearance {
        dir,
        cm: (best * 10.0).round() / 10.0,
        against,
        name,
    }
}

/// Distance between two boxes along an axis: the gap between their facing
/// sides, or `0` when they overlap along it.
pub fn gap(a: (Point2, Point2), b: (Point2, Point2), axis: Axis) -> f64 {
    let (a0, a1) = (axis.of(a.0), axis.of(a.1));
    let (b0, b1) = (axis.of(b.0), axis.of(b.1));
    if b0 >= a1 {
        b0 - a1
    } else if a0 >= b1 {
        a0 - b1
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::Wall;
    use crate::ids::{FurnitureId, WallId};

    fn piece(id: u64, at: (f64, f64), size: (f64, f64, f64), angle: f64) -> Furniture {
        Furniture {
            id: FurnitureId(id),
            catalog: "box".into(),
            name: format!("peça {id}"),
            position: Point2::new(at.0, at.1),
            width: size.0,
            depth: size.1,
            height: size.2,
            angle,
            ..Furniture::default()
        }
    }

    #[test]
    fn bounds_resolve_a_quarter_turn() {
        let turned = piece(1, (100.0, 100.0), (200.0, 60.0, 90.0), 90.0);
        let (min, max) = plan_bounds(&turned);
        assert!((max.x - min.x - 60.0).abs() < 1e-6, "{min:?} {max:?}");
        assert!((max.y - min.y - 200.0).abs() < 1e-6, "{min:?} {max:?}");
        assert_eq!(facing(&piece(1, (0.0, 0.0), (10.0, 10.0, 10.0), 0.0)), "+y");
        assert_eq!(
            facing(&piece(1, (0.0, 0.0), (10.0, 10.0, 10.0), 90.0)),
            "-x"
        );
        assert_eq!(
            facing(&piece(1, (0.0, 0.0), (10.0, 10.0, 10.0), 180.0)),
            "-y"
        );
        assert_eq!(
            facing(&piece(1, (0.0, 0.0), (10.0, 10.0, 10.0), 270.0)),
            "+x"
        );
    }

    fn room(home: &mut Home, w: f64, d: f64) {
        let corners = [(0.0, 0.0), (w, 0.0), (w, d), (0.0, d)];
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            let mut wall = Wall::new(
                WallId(k as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            );
            wall.thickness = 20.0;
            home.walls.push(wall);
        }
    }

    #[test]
    fn free_span_reads_a_corridor_between_two_counters() {
        let mut home = Home::default();
        room(&mut home, 400.0, 300.0);
        // Two 60 cm counters facing each other, 100 cm apart.
        home.furniture
            .push(piece(10, (200.0, 40.0), (300.0, 60.0, 90.0), 0.0));
        home.furniture
            .push(piece(11, (200.0, 230.0), (300.0, 60.0, 90.0), 0.0));
        let spans = free_span(&home, Axis::Y, 200.0, Some((0.0, 300.0)), (0.0, 200.0));
        // Wall, counter, corridor, counter, the strip behind it, wall.
        let names: Vec<&str> = spans
            .iter()
            .map(|s| {
                if s.is_free() {
                    "livre"
                } else {
                    s.name.as_str()
                }
            })
            .collect();
        assert_eq!(
            names,
            ["parede", "peça 10", "livre", "peça 11", "livre", "parede"],
            "{spans:#?}"
        );
        let corridor = spans
            .iter()
            .filter(|s| s.is_free())
            .map(Span::cm)
            .fold(0.0, f64::max);
        assert!((corridor - 130.0).abs() < 0.5, "{spans:#?}");
        // And the same answer through `clearance`.
        let piece = home.piece(FurnitureId(10)).unwrap();
        let c = clearance(
            &home,
            piece,
            Dir {
                axis: Axis::Y,
                sign: 1.0,
            },
            MAX_REACH,
        );
        assert!((c.cm - 130.0).abs() < 0.5, "{c:?}");
        assert_eq!(c.against, Some(Solid::Piece(FurnitureId(11))));
    }

    #[test]
    fn overlapping_solids_do_not_make_the_probe_walk_backwards() {
        // Two pieces that overlap along the probe: the spans must still come
        // out in order, covering the run once, with no negative stretch.
        let mut home = Home::default();
        home.furniture = vec![
            piece(1, (100.0, 100.0), (100.0, 100.0, 90.0), 0.0),
            piece(2, (160.0, 100.0), (100.0, 100.0, 90.0), 0.0),
        ];

        let spans = free_span(&home, Axis::X, 100.0, Some((0.0, 300.0)), (0.0, 200.0));
        assert!(
            spans.iter().all(|s| s.cm() > 0.0),
            "no stretch runs backwards: {spans:?}"
        );
        for pair in spans.windows(2) {
            assert!(
                (pair[1].from - pair[0].to).abs() < 0.01,
                "the spans join end to end: {spans:?}"
            );
        }
        let covered: f64 = spans.iter().filter(|s| !s.is_free()).map(Span::cm).sum();
        // 50 → 210 covered once, not 100 + 100 counted twice.
        assert!((covered - 160.0).abs() < 0.1, "{spans:?}");
    }

    #[test]
    fn clearance_stops_at_the_wall_behind() {
        let mut home = Home::default();
        room(&mut home, 400.0, 300.0);
        home.furniture
            .push(piece(10, (200.0, 45.0), (300.0, 60.0, 90.0), 0.0));
        let piece = home.piece(FurnitureId(10)).unwrap();
        let c = clearance(
            &home,
            piece,
            Dir {
                axis: Axis::Y,
                sign: -1.0,
            },
            MAX_REACH,
        );
        assert!((c.cm - 5.0).abs() < 0.5, "{c:?}");
        assert!(matches!(c.against, Some(Solid::Wall(_))), "{c:?}");
    }

    #[test]
    fn a_wall_cabinet_does_not_shrink_the_floor_below_it() {
        let mut home = Home::default();
        room(&mut home, 400.0, 300.0);
        let mut high = piece(12, (200.0, 100.0), (200.0, 35.0, 70.0), 0.0);
        high.elevation = 150.0;
        home.furniture.push(high);
        let low = free_span(&home, Axis::Y, 200.0, Some((0.0, 300.0)), (0.0, 140.0));
        assert!(
            low.iter()
                .all(|s| s.is_free() || matches!(s.what, Some(Solid::Wall(_)))),
            "{low:#?}"
        );
        let high = free_span(&home, Axis::Y, 200.0, Some((0.0, 300.0)), (140.0, 220.0));
        assert!(
            high.iter()
                .any(|s| s.what == Some(Solid::Piece(FurnitureId(12)))),
            "{high:#?}"
        );
    }
}

/// An annotation that no longer matches the drawing under it.
///
/// A plan of joinery lives on its dimensions and notes; one that still reads
/// 66,5 over a corridor that is now 86 is worse than no note at all, and the
/// only way to catch it used to be reading every one of them by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct Stale {
    pub id: ElementId,
    /// What the annotation says, cm.
    pub drawn: f64,
    /// What the drawing measures there now, cm.
    pub measured: f64,
    /// The element the measurement was taken against, when there is one.
    pub against: Option<ElementId>,
    pub text: String,
}

/// How far a dimension may be off before it counts as stale, cm.
const STALE_TOLERANCE: f64 = 1.0;

/// Dimensions whose length no longer matches the run they mark, and labels
/// whose written sizes no longer match the piece they sit on.
///
/// Only axis-aligned dimensions are checked: a slanted one has no single
/// run to compare against, and guessing would cost more than it saves.
pub fn stale_annotations(home: &Home) -> Vec<Stale> {
    let mut out = Vec::new();
    for dim in &home.dimensions {
        let (dx, dy) = (dim.end.x - dim.start.x, dim.end.y - dim.start.y);
        let axis = if dy.abs() <= 0.5 && dx.abs() > 0.5 {
            Axis::X
        } else if dx.abs() <= 0.5 && dy.abs() > 0.5 {
            Axis::Y
        } else {
            continue;
        };
        let across = axis.across();
        let at = across.of(dim.start);
        let (lo, hi) = {
            let (a, b) = (axis.of(dim.start), axis.of(dim.end));
            (a.min(b), a.max(b))
        };
        // The run the dimension sits in: the stretch of the probe that its
        // own middle falls into.
        let middle = f64::midpoint(lo, hi);
        let Some(span) = free_span(home, axis, at, None, (0.0, 200.0))
            .into_iter()
            .find(|s| s.from <= middle && middle <= s.to)
        else {
            continue;
        };
        // A dimension over a solid measures that piece, not a gap; either
        // way the comparison is the same.
        let measured = span.cm();
        if (measured - dim.length()).abs() > STALE_TOLERANCE {
            out.push(Stale {
                id: dim.id.into(),
                drawn: (dim.length() * 10.0).round() / 10.0,
                measured: (measured * 10.0).round() / 10.0,
                against: span.what.as_ref().map(Solid::id),
                text: span.name.clone(),
            });
        }
    }

    for label in &home.labels {
        let sizes = written_sizes(&label.text);
        if sizes.is_empty() {
            continue;
        }
        // Only labels standing on a piece are checked: for those the piece
        // they are about is not a guess.
        let Some(piece) = home
            .furniture
            .iter()
            .flat_map(Furniture::flatten)
            .find(|f| {
                let (min, max) = plan_bounds(f);
                (min.x..=max.x).contains(&label.position.x)
                    && (min.y..=max.y).contains(&label.position.y)
            })
        else {
            continue;
        };
        let actual = [piece.width, piece.depth, piece.height];
        for written in sizes {
            // Any order: a note may read width × height × depth.
            if written
                .iter()
                .all(|w| actual.iter().any(|a| (a - w).abs() <= STALE_TOLERANCE))
            {
                continue;
            }
            // A note of three sizes is written width × depth × height, so
            // the pair to report is the one that moved in place — not the
            // nearest number, which would name the wrong dimension.
            let pairs: Vec<(f64, f64)> = if written.len() == actual.len() {
                written.iter().copied().zip(actual).collect()
            } else {
                written
                    .iter()
                    .map(|w| {
                        let closest = actual
                            .iter()
                            .copied()
                            .min_by(|a, b| (a - w).abs().total_cmp(&(b - w).abs()))
                            .unwrap_or(0.0);
                        (*w, closest)
                    })
                    .collect()
            };
            let (drawn, measured) = pairs
                .into_iter()
                .max_by(|a, b| (a.0 - a.1).abs().total_cmp(&(b.0 - b.1).abs()))
                .unwrap_or((0.0, 0.0));
            out.push(Stale {
                id: label.id.into(),
                drawn,
                measured: (measured * 10.0).round() / 10.0,
                against: Some(piece.id.into()),
                text: label.text.clone(),
            });
            break;
        }
    }
    out
}

/// Size groups written in a note: `80 × 65 × 280`, `58x50`, `0,80 × 0,65`.
///
/// Only numbers joined by `×` or `x` are read, so a note that merely mentions
/// a number ("53A", "2 portas") is left alone.
fn written_sizes(text: &str) -> Vec<Vec<f64>> {
    // `80×65` and `58x50` are written without spaces as often as with them.
    let chars: Vec<char> = text.chars().collect();
    let mut spaced = String::with_capacity(text.len());
    for (i, c) in chars.iter().enumerate() {
        let times = *c == '×'
            || ((*c == 'x' || *c == 'X')
                && i > 0
                && chars[i - 1].is_ascii_digit()
                && chars.get(i + 1).is_some_and(char::is_ascii_digit));
        if times {
            spaced.push_str(" × ");
        } else {
            spaced.push(*c);
        }
    }

    let mut out: Vec<Vec<f64>> = Vec::new();
    let mut group: Vec<f64> = Vec::new();
    let mut linked = false;
    let flush = |group: &mut Vec<f64>, out: &mut Vec<Vec<f64>>| {
        if group.len() > 1 {
            out.push(std::mem::take(group));
        }
        group.clear();
    };
    for token in spaced.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_ascii_digit());
        if let Ok(value) = clean.replace(',', ".").parse::<f64>() {
            if !linked {
                flush(&mut group, &mut out);
            }
            group.push(value);
            linked = false;
        } else if token == "×" {
            linked = true;
        } else {
            flush(&mut group, &mut out);
            linked = false;
        }
    }
    flush(&mut group, &mut out);

    // A note in meters ("0,80 × 0,65") describes the same piece in other
    // units. Whole small numbers ("2 × 3 gavetas") are counts, not sizes.
    out.into_iter()
        .map(|g| {
            if g.iter().all(|v| *v < 10.0 && v.fract() > 0.0) {
                g.into_iter().map(|v| v * 100.0).collect()
            } else {
                g
            }
        })
        .collect()
}

#[cfg(test)]
mod stale_tests {
    use super::*;

    #[test]
    fn written_sizes_reads_notes_the_way_a_joiner_writes_them() {
        assert_eq!(
            written_sizes("TORRE 80 × 65 × 280 | 53A 30 × 65 × 280"),
            vec![vec![80.0, 65.0, 280.0], vec![30.0, 65.0, 280.0]]
        );
        assert_eq!(
            written_sizes("Forno: vão interno 58x50x63 P"),
            vec![vec![58.0, 50.0, 63.0]]
        );
        assert_eq!(written_sizes("Bancada 0,80 × 0,65"), vec![vec![80.0, 65.0]]);
        // Numbers that are not sizes are left where they are.
        assert!(written_sizes("Cozinha: corredor 72 cm").is_empty());
        assert!(written_sizes("53A e 2 portas").is_empty());
    }
}
