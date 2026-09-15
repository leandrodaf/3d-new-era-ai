//! Layout checks that make a plan usable, not just drawable: pieces that
//! overlap, pieces stuck in walls and doors that cannot open.
//!
//! An overlap on its own says almost nothing — a cooktop set into its
//! countertop and a sink eating into the dishwasher next to it look the same
//! from a pair of ids. So every overlap comes classified ([`Overlap`]) and
//! measured ([`Issue::extent`]), and pairs that only "collide" because they
//! live on two storeys drawn at the same elevation are named for what they
//! are instead of being reported as real problems.

use geo::{Area, BooleanOps, BoundingRect, Polygon};

use crate::furniture::Furniture;
use crate::geometry::{Point2, to_polygon};
use crate::home::Home;
use crate::ids::{FurnitureId, LevelId, WallId};

/// What an overlap between two pieces really is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlap {
    /// Two pieces fight for the same space: a real defect.
    Collision,
    /// One is built into, resting on or tucked under the other — a cooktop in
    /// its countertop, a chair under a table, a face panel on a drawer front.
    Nesting,
    /// The pieces are on different storeys; they never meet in the building.
    CrossLevel,
}

impl Overlap {
    pub fn name(self) -> &'static str {
        match self {
            Self::Collision => "collision",
            Self::Nesting => "nesting",
            Self::CrossLevel => "cross_level",
        }
    }

    /// Only a collision needs fixing.
    pub fn is_defect(self) -> bool {
        self == Self::Collision
    }
}

/// Which storeys a check looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Storeys {
    /// The storey shown in the editor.
    #[default]
    Active,
    /// One named storey.
    One(LevelId),
    /// Every storey that is not a reference layer, with cross-storey pairs
    /// marked [`Overlap::CrossLevel`].
    All,
}

/// Something worth fixing in a layout.
#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    /// Two pieces occupy the same space, classified and measured.
    Overlap {
        a: FurnitureId,
        b: FurnitureId,
        kind: Overlap,
        /// Size of the shared space, `[x, y, z]` cm.
        extent: [f64; 3],
    },
    /// A cabinet, a fridge or a wardrobe whose front is against a solid:
    /// it cannot be opened, and no other check notices.
    Blocked {
        piece: FurnitureId,
        /// What the front runs into.
        against: crate::ids::ElementId,
        /// Free centimeters in front of it.
        cm: f64,
    },
    /// A piece that is not a door or window goes through a wall.
    InWall(FurnitureId, WallId),
    /// A piece sits where a door leaf swings.
    BlocksDoor { door: FurnitureId, by: FurnitureId },
    /// A piece is outside every room (only reported when rooms exist).
    OutsideRooms(FurnitureId),
}

impl Issue {
    /// Ids the issue is about, in report order.
    pub fn ids(&self) -> Vec<crate::ids::ElementId> {
        match self {
            Self::Overlap { a, b, .. } => vec![(*a).into(), (*b).into()],
            Self::Blocked { piece, against, .. } => vec![(*piece).into(), *against],
            Self::InWall(f, w) => vec![(*f).into(), (*w).into()],
            Self::BlocksDoor { door, by } => vec![(*door).into(), (*by).into()],
            Self::OutsideRooms(f) => vec![(*f).into()],
        }
    }

    /// Whether this is a real defect rather than a classified non-problem.
    pub fn is_defect(&self) -> bool {
        match self {
            Self::Overlap { kind, .. } => kind.is_defect(),
            _ => true,
        }
    }
}

/// Area below which a contact is ignored (touching pieces are fine), cm².
const MIN_OVERLAP: f64 = 25.0;
/// How far a piece may press into a wall face and still count as against it, cm.
const WALL_TOLERANCE: f64 = 2.0;
/// Pieces this thin (rugs, mats) never collide.
const FLAT: f64 = 2.0;

fn polygon(points: &[Point2]) -> Polygon<f64> {
    to_polygon(points)
}

fn heights_overlap(a: &Furniture, b: &Furniture) -> bool {
    let ((a0, a1), (b0, b1)) = (a.height_range(), b.height_range());
    a0 < b1 && b0 < a1
}

fn centroid(polygon: &Polygon<f64>) -> Option<Point2> {
    geo::Centroid::centroid(polygon).map(|c| Point2::new(c.x(), c.y()))
}

/// Height of a wall's top above the floor at a plan point.
fn wall_top_at(wall: &crate::elements::Wall, p: Point2) -> f64 {
    let len = wall.start.distance(wall.end).max(1e-9);
    let t = (((p.x - wall.start.x) * (wall.end.x - wall.start.x)
        + (p.y - wall.start.y) * (wall.end.y - wall.start.y))
        / (len * len))
        .clamp(0.0, 1.0);
    wall.height + (wall.height_at_end.unwrap_or(wall.height) - wall.height) * t
}

/// Floor area swept by a hinged door leaf, as a polygon (quarter discs).
pub fn door_swing(door: &Furniture) -> Option<Vec<Point2>> {
    let opening = door.opening.as_ref()?;
    if opening.sliding || opening.kind != crate::furniture::OpeningKind::Door {
        return None;
    }
    let leaves = f64::from(opening.leaves.clamp(1, 2));
    let radius = door.width / leaves;
    let front = door.depth / 2.0;
    let quarter = |hinge_x: f64, toward: f64| -> Vec<(f64, f64)> {
        let mut pts = vec![(hinge_x, front)];
        for i in 0..=8 {
            let a = f64::from(i) / 8.0 * std::f64::consts::FRAC_PI_2;
            pts.push((
                hinge_x + toward * radius * a.cos(),
                front + radius * a.sin(),
            ));
        }
        pts
    };
    let half = door.width / 2.0;
    let local: Vec<(f64, f64)> = if opening.leaves >= 2 {
        let mut pts = quarter(-half, 1.0);
        pts.extend(quarter(half, -1.0).into_iter().rev());
        pts
    } else if opening.hinge_right {
        quarter(half, -1.0)
    } else {
        quarter(-half, 1.0)
    };
    Some(local.into_iter().map(|p| door.to_plan(p)).collect())
}

/// Layout problems on the storey the editor is showing.
///
/// For a plan drawn as stacked layers, or to see every storey at once, use
/// [`check_layout_in`].
pub fn check_layout(home: &Home) -> Vec<Issue> {
    check_layout_in(home, Storeys::Active)
}

/// Layout problems on the storeys `scope` selects.
pub fn check_layout_in(home: &Home, scope: Storeys) -> Vec<Issue> {
    let mut issues = Vec::new();
    // Pieces inside groups are checked one by one; pieces of the same group
    // (roof slopes, a table and its chairs) are meant to touch.
    let mut pieces: Vec<&Furniture> = Vec::new();
    let mut groups: Vec<usize> = Vec::new();
    let mut levels: Vec<Option<LevelId>> = Vec::new();
    let wanted = |level: Option<LevelId>| match scope {
        Storeys::Active => home.on_level(level, home.current_level()),
        Storeys::One(id) => home.on_level(level, Some(id)),
        // Reference layers are drawing, not building: never checked.
        Storeys::All => !home.is_reference_level(level),
    };
    for (g, top) in home.furniture.iter().enumerate() {
        if !wanted(top.level) {
            continue;
        }
        for leaf in top.visible_leaves() {
            // Items embedded in joinery (a cooktop in its countertop) sit in
            // their host's cutout or niche by design.
            if leaf.properties.contains_key("joinery:embedded") {
                continue;
            }
            pieces.push(leaf);
            groups.push(g);
            levels.push(home.resolve_level(top.level));
        }
    }
    let footprints: Vec<Polygon<f64>> = pieces
        .iter()
        .map(|f| polygon(&f.projected_footprint()))
        .collect();

    for (i, a) in pieces.iter().enumerate() {
        if a.is_opening() || a.height <= FLAT {
            continue;
        }
        for (j, b) in pieces.iter().enumerate().skip(i + 1) {
            let cross_level = levels[i] != levels[j];
            if b.is_opening()
                || b.height <= FLAT
                || groups[i] == groups[j]
                // Two storeys are checked against each other only for their
                // heights in the building, not for their drawn elevations.
                || (!cross_level && !heights_overlap(a, b))
            {
                continue;
            }
            // A wall or panel cut to the roof's profile sits under its slopes.
            let tilted = |f: &Furniture| f.pitch != 0.0 || f.roll != 0.0;
            let fitted = |f: &Furniture| f.properties.contains_key(crate::roof_fit::ROOF_FIT_KEY);
            if (fitted(a) && tilted(b)) || (fitted(b) && tilted(a)) {
                continue;
            }
            let shared = footprints[i].intersection(&footprints[j]);
            let area = shared.unsigned_area();
            if area <= MIN_OVERLAP {
                continue;
            }
            // Tilted pieces only collide if they are at the same height there.
            let meet = cross_level
                || shared.iter().next().and_then(centroid).is_none_or(|at| {
                    a.underside_at(at) < b.top_at(at) && b.underside_at(at) < a.top_at(at)
                });
            if !meet {
                continue;
            }
            let extent = extent_of(&shared, a, b);
            let kind = if cross_level {
                Overlap::CrossLevel
            } else {
                classify(a, b, area, extent, &footprints[i], &footprints[j])
            };
            issues.push(Issue::Overlap {
                a: a.id,
                b: b.id,
                kind,
                extent,
            });
        }
    }

    let outlines = outlines_by_level(home);
    for (i, piece) in pieces.iter().enumerate() {
        if piece.is_opening() {
            continue;
        }
        for (wall, outline) in home.walls.iter().zip(&outlines) {
            // Only walls of the piece's own storey can have it inside them.
            if outline.len() < 3 || home.resolve_level(wall.level) != levels[i] {
                continue;
            }
            let shared = footprints[i].intersection(&polygon(outline));
            if shared.unsigned_area() <= MIN_OVERLAP {
                continue;
            }
            // Resting against a wall is not being in it: only count pieces that
            // go more than a couple of centimeters into it.
            let depth = shared
                .iter()
                .filter_map(geo::MinimumRotatedRect::minimum_rotated_rect)
                .map(|r| {
                    let c: Vec<_> = r.exterior().coords().copied().collect();
                    let a = (c[1].x - c[0].x).hypot(c[1].y - c[0].y);
                    let b = (c[2].x - c[1].x).hypot(c[2].y - c[1].y);
                    a.min(b)
                })
                .fold(0.0, f64::max);
            if depth <= WALL_TOLERANCE {
                continue;
            }
            // Over the shared area, is the piece below the wall top? Tilted
            // pieces (rafters, roof slopes) are measured right there.
            let Some(at) = shared.iter().next().and_then(centroid) else {
                continue;
            };
            if piece.underside_at(at) < wall_top_at(wall, at) - 1.0 {
                issues.push(Issue::InWall(piece.id, wall.id));
            }
        }
    }

    for (d, door) in pieces.iter().enumerate() {
        if !door.is_opening() {
            continue;
        }
        let Some(swing) = door_swing(door) else {
            continue;
        };
        let swing = polygon(&swing);
        for (i, piece) in pieces.iter().enumerate() {
            // Below the door's sill (footings under a raised floor) is out of its way.
            // A leaf only ever swings into its own storey.
            if piece.is_opening()
                || levels[i] != levels[d]
                || piece.height <= FLAT
                || piece.height_range().1 <= door.elevation + 1.0
            {
                continue;
            }
            let shared = swing.intersection(&footprints[i]);
            let high_enough = shared
                .iter()
                .next()
                .and_then(centroid)
                .is_some_and(|at| piece.underside_at(at) >= door.elevation + door.height);
            if shared.unsigned_area() > MIN_OVERLAP && !high_enough {
                issues.push(Issue::BlocksDoor {
                    door: door.id,
                    by: piece.id,
                });
            }
        }
    }

    // A cabinet turned the wrong way is a modelling slip that survives every
    // other check: the boxes do not overlap, nothing is in a wall, and the
    // piece is simply impossible to open. `angle` alone does not show it.
    // The solids of a storey are gathered once and measured against many
    // times: rebuilding them per cabinet would walk the whole plan again for
    // each one, and this runs on every read of the issue count.
    let mut solids: Vec<(Option<LevelId>, Vec<crate::measure::Obstacle>)> = Vec::new();
    for (i, piece) in pieces.iter().enumerate() {
        if !opens_at_the_front(piece) {
            continue;
        }
        let Some(front) = crate::measure::Dir::parse("front", Some(piece)) else {
            continue;
        };
        if !solids.iter().any(|(level, _)| *level == levels[i]) {
            let view = home.level_view(levels[i]);
            solids.push((levels[i], crate::measure::obstacles(&view, &|_| false)));
        }
        let here = solids
            .iter()
            .find(|(level, _)| *level == levels[i])
            .map(|(_, list)| list.as_slice())
            .unwrap_or_default();
        let clear = crate::measure::clearance_against(here, piece, front, OPENING_ROOM);
        if clear.cm < OPENING_ROOM
            && let Some(against) = clear.against
        {
            issues.push(Issue::Blocked {
                piece: piece.id,
                against: against.id(),
                cm: (clear.cm * 10.0).round() / 10.0,
            });
        }
    }

    if !home.rooms.is_empty() {
        let rooms: Vec<(Option<LevelId>, Polygon<f64>)> = home
            .rooms
            .iter()
            .map(|r| (home.resolve_level(r.level), polygon(&r.points)))
            .collect();
        for (i, piece) in pieces.iter().enumerate() {
            if piece.is_opening() {
                continue;
            }
            let center = geo::Point::new(piece.position.x, piece.position.y);
            let covered = rooms
                .iter()
                .any(|(level, r)| *level == levels[i] && geo::Contains::contains(r, &center));
            // A storey with no rooms drawn yet has nothing to be outside of.
            let has_rooms = rooms.iter().any(|(level, _)| *level == levels[i]);
            if has_rooms && !covered {
                issues.push(Issue::OutsideRooms(piece.id));
            }
        }
    }
    issues
}

/// Room a door or a drawer needs in front of it before it is unusable, cm.
const OPENING_ROOM: f64 = 5.0;

/// Whether this piece is opened from the front — a cabinet, a fridge, a
/// wardrobe, an oven — so that a solid against its face makes it useless.
///
/// Pieces that are simply approached from the front (a bed, a sofa, a table)
/// are left out: they lose comfort, not function, and ergonomics already
/// says so.
fn opens_at_the_front(piece: &Furniture) -> bool {
    const CATALOGS: [&str; 9] = [
        "base-cabinet",
        "wall-cabinet",
        "tall-cabinet",
        "wardrobe",
        "fridge",
        "dishwasher",
        "washer",
        "oven",
        "cabinet",
    ];
    if piece.is_opening() || piece.height <= FLAT {
        return false;
    }
    CATALOGS.iter().any(|c| piece.catalog.contains(c))
}

/// Wall outlines with corners joined **within each storey**.
///
/// [`Home::wall_outlines`] knows nothing about levels, so in a plan drawn as
/// stacked layers it joins a new wall to the traced copy underneath it and
/// the resulting outline covers ground neither wall does. Joining one storey
/// at a time keeps every outline honest; the result stays in `home.walls`
/// order so callers can zip it.
fn outlines_by_level(home: &Home) -> Vec<Vec<Point2>> {
    let mut levels: Vec<Option<LevelId>> = Vec::new();
    for wall in &home.walls {
        let level = home.resolve_level(wall.level);
        if !levels.contains(&level) {
            levels.push(level);
        }
    }
    if levels.len() < 2 {
        return home.wall_outlines();
    }
    let mut out = vec![Vec::new(); home.walls.len()];
    for level in levels {
        let view = home.level_view(level);
        for (wall, outline) in view.walls.iter().zip(view.wall_outlines()) {
            if let Some(at) = home.walls.iter().position(|w| w.id == wall.id) {
                out[at] = outline;
            }
        }
    }
    out
}

/// Size of what two pieces share, `[x, y, z]` cm.
fn extent_of(shared: &geo::MultiPolygon<f64>, a: &Furniture, b: &Furniture) -> [f64; 3] {
    let plan = shared.bounding_rect().map_or([0.0, 0.0], |r| {
        [r.max().x - r.min().x, r.max().y - r.min().y]
    });
    let ((a0, a1), (b0, b1)) = (a.height_range(), b.height_range());
    let z = (a1.min(b1) - a0.max(b0)).max(0.0);
    [plan[0], plan[1], z].map(|v| (v * 10.0).round() / 10.0)
}

/// Is this overlap a defect, or is one piece simply built into the other?
///
/// The rules are about shape and size only — no catalog names, no guessing
/// what a piece is called — so they hold for imported models and joinery
/// alike:
///
/// * a face panel, a back or a glass pane (3 cm or thinner) is part of what
///   it is stuck to;
/// * a lid over a box — a countertop on its cabinets — rests on it;
/// * a piece mostly inside a bigger one is built into it (an oven in its
///   tower, a cooktop in its worktop, a dishwasher under the counter);
/// * a couple of centimeters of drawing slack (a chair pushed under a table)
///   is how plans are drawn, not a clash.
fn classify(
    a: &Furniture,
    b: &Furniture,
    shared_area: f64,
    extent: [f64; 3],
    fa: &Polygon<f64>,
    fb: &Polygon<f64>,
) -> Overlap {
    let thin = |f: &Furniture| f.width.min(f.depth) <= 3.0;
    if thin(a) || thin(b) {
        return Overlap::Nesting;
    }
    let (area_a, area_b) = (fa.unsigned_area(), fb.unsigned_area());
    // A lid resting on a box: flat, and sitting at the other's top.
    let lid_on = |lid: &Furniture, box_: &Furniture, lid_area: f64, box_area: f64| {
        let (lo, _) = lid.height_range();
        lid.height <= 8.0
            && (lo - box_.height_range().1).abs() <= 6.0
            && lid_area >= 0.5 * box_area
    };
    if lid_on(a, b, area_a, area_b) || lid_on(b, a, area_b, area_a) {
        return Overlap::Nesting;
    }
    // Mostly inside the other: built in.
    if shared_area >= 0.6 * area_a.min(area_b) {
        return Overlap::Nesting;
    }
    // A seat pushed under the table it serves.
    let tucked = |seat: &Furniture, table: &Furniture| seat.is_seat() && table.is_table_height();
    if tucked(a, b) || tucked(b, a) {
        return Overlap::Nesting;
    }
    // Touching with drawing slack, or tucked under.
    if extent[0].min(extent[1]) <= 2.0 || extent[2] <= 1.0 {
        return Overlap::Nesting;
    }
    Overlap::Collision
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{Room, Wall};
    use crate::furniture::{Opening, align_to_wall};
    use crate::ids::RoomId;

    fn piece(id: u64, at: (f64, f64), size: (f64, f64, f64)) -> Furniture {
        Furniture {
            id: FurnitureId(id),
            catalog: "box".into(),
            name: "Caixa".into(),
            position: Point2::new(at.0, at.1),
            elevation: 0.0,
            angle: 0.0,
            width: size.0,
            depth: size.1,
            height: size.2,
            mirrored: false,
            color: None,
            opening: None,
            model: None,
            visible: true,
            level: None,
            ..Default::default()
        }
    }

    fn room_home() -> Home {
        let mut home = Home::default();
        let pts = [(0.0, 0.0), (500.0, 0.0), (500.0, 400.0), (0.0, 400.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let id = home.new_wall_id();
            home.walls
                .push(Wall::new(id, Point2::new(a.0, a.1), Point2::new(b.0, b.1)));
        }
        home.rooms.push(Room::new(
            RoomId(90),
            "Sala",
            pts.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        ));
        home
    }

    #[test]
    fn finds_overlaps_but_not_stacked_or_touching_pieces() {
        let mut home = room_home();
        home.furniture
            .push(piece(10, (200.0, 200.0), (100.0, 100.0, 75.0)));
        home.furniture
            .push(piece(11, (250.0, 200.0), (100.0, 100.0, 75.0))); // overlaps 10
        home.furniture
            .push(piece(12, (350.0, 200.0), (100.0, 100.0, 75.0))); // touches 11
        let mut shelf = piece(13, (200.0, 200.0), (100.0, 40.0, 50.0));
        shelf.elevation = 150.0; // above 10
        home.furniture.push(shelf);
        home.furniture
            .push(piece(14, (200.0, 200.0), (200.0, 140.0, 1.0))); // rug

        let issues = check_layout(&home);
        assert_eq!(
            issues,
            vec![Issue::Overlap {
                a: FurnitureId(10),
                b: FurnitureId(11),
                kind: Overlap::Collision,
                extent: [50.0, 100.0, 75.0],
            }],
            "{issues:?}"
        );
    }

    #[test]
    fn a_cabinet_turned_against_a_wall_is_reported_as_unusable() {
        let mut home = room_home();
        // A base cabinet against the top wall, opening into the room.
        let mut right_way = piece(60, (200.0, 40.0), (80.0, 60.0, 90.0));
        right_way.catalog = "base-cabinet".into();
        right_way.name = "Gabinete".into();
        // The same cabinet turned a half turn: its doors face the wall.
        let mut wrong_way = piece(61, (350.0, 40.0), (80.0, 60.0, 90.0));
        wrong_way.catalog = "base-cabinet".into();
        wrong_way.name = "Gabinete virado".into();
        wrong_way.angle = 180.0;
        home.furniture.extend([right_way, wrong_way]);

        let blocked: Vec<FurnitureId> = check_layout(&home)
            .into_iter()
            .filter_map(|i| match i {
                Issue::Blocked { piece, .. } => Some(piece),
                _ => None,
            })
            .collect();
        assert_eq!(blocked, vec![FurnitureId(61)], "{blocked:?}");
    }

    #[test]
    fn built_in_pieces_and_layers_are_named_instead_of_reported_as_clashes() {
        use crate::elements::Level;
        use crate::ids::LevelId;

        let mut home = room_home();
        // A worktop resting on its cabinet.
        let mut cabinet = piece(40, (100.0, 200.0), (120.0, 60.0, 84.0));
        cabinet.name = "Gabinete".into();
        let mut top = piece(41, (100.0, 200.0), (124.0, 62.0, 4.0));
        top.elevation = 84.0;
        // A cooktop set into it.
        let mut cooktop = piece(42, (100.0, 200.0), (58.0, 50.0, 10.0));
        cooktop.elevation = 82.0;
        // A chair pushed 1 cm under a table.
        let mut table = piece(43, (300.0, 200.0), (140.0, 80.0, 75.0));
        table.elevation = 0.0;
        let mut chair = piece(44, (300.0, 240.5), (45.0, 45.0, 90.0));
        chair.catalog = "chair".into();
        chair.name = "Cadeira".into();
        home.furniture
            .extend([cabinet, top, cooktop, table.clone(), chair]);
        let kinds: Vec<Overlap> = check_layout(&home)
            .into_iter()
            .filter_map(|i| match i {
                Issue::Overlap { kind, .. } => Some(kind),
                _ => None,
            })
            .collect();
        assert!(
            !kinds.is_empty() && kinds.iter().all(|k| *k == Overlap::Nesting),
            "{kinds:?} — built-in pieces are not clashes"
        );

        // But a worktop eating 5 cm into the dishwasher beside it is one.
        let mut sink = piece(45, (410.0, 60.0), (70.0, 65.0, 35.7));
        sink.name = "Pia centralizada".into();
        sink.elevation = 72.0;
        let mut dishwasher = piece(46, (469.5, 60.0), (59.8, 63.5, 84.5));
        dishwasher.name = "Lava-louças".into();
        home.furniture.extend([sink, dishwasher]);
        let clash = check_layout(&home)
            .into_iter()
            .find(|i| matches!(i, Issue::Overlap { a, b, .. }
                if *a == FurnitureId(45) && *b == FurnitureId(46)))
            .expect("the sink invading the dishwasher is reported");
        let Issue::Overlap { kind, extent, .. } = clash else {
            unreachable!()
        };
        assert_eq!(kind, Overlap::Collision);
        assert!((extent[0] - 5.4).abs() < 0.2, "{extent:?}");

        // The same plan drawn twice, as two storeys at the same elevation.
        let mut layered = Home::default();
        layered.levels = vec![
            Level {
                id: LevelId(1),
                name: "Planta reproduzida".into(),
                elevation: 0.0,
                height: 280.0,
                ..Level::default()
            },
            Level {
                id: LevelId(2),
                name: "Novo layout".into(),
                elevation: 0.0,
                height: 280.0,
                elevation_index: 1,
                ..Level::default()
            },
        ];
        let mut old_table = table.clone();
        old_table.id = FurnitureId(50);
        old_table.level = Some(LevelId(1));
        let mut new_table = table;
        new_table.id = FurnitureId(51);
        new_table.level = Some(LevelId(2));
        layered.furniture = vec![old_table, new_table];
        let all = check_layout_in(&layered, Storeys::All);
        assert_eq!(
            all,
            vec![Issue::Overlap {
                a: FurnitureId(50),
                b: FurnitureId(51),
                kind: Overlap::CrossLevel,
                extent: [140.0, 80.0, 75.0],
            }],
            "{all:?}"
        );
        assert!(all.iter().all(|i| !i.is_defect()));
        // Only one storey at a time: nothing to report.
        assert!(check_layout_in(&layered, Storeys::One(LevelId(2))).is_empty());
        // Marking the traced plan as a reference drops it from `All` too.
        layered.levels[0].set_reference(true);
        assert!(check_layout_in(&layered, Storeys::All).is_empty());
        assert_eq!(layered.stacked_levels(), Vec::new());
    }

    #[test]
    fn pieces_against_a_wall_or_below_a_raised_door_are_fine() {
        let mut home = room_home();
        // A wardrobe pressing 1 cm into the top wall's face: against it, not in it.
        let face = home.walls[0].thickness / 2.0;
        home.furniture
            .push(piece(30, (250.0, face - 1.0 + 30.0), (180.0, 60.0, 220.0)));
        // 5 cm in: that one is in the wall.
        home.furniture
            .push(piece(31, (100.0, face - 5.0 + 20.0), (60.0, 40.0, 80.0)));
        let mut door = piece(32, (0.0, 0.0), (80.0, 15.0, 210.0));
        door.opening = Some(Opening::default());
        door.elevation = 55.0;
        let left_wall = home.walls[3].clone();
        align_to_wall(&mut door, &left_wall, 200.0);
        home.furniture.push(door);
        // A footing under the raised floor, right in the swing.
        home.furniture
            .push(piece(33, (40.0, 180.0), (40.0, 40.0, 50.0)));
        let issues = check_layout(&home);
        assert!(
            !issues
                .iter()
                .any(|i| matches!(i, Issue::InWall(FurnitureId(30), _))),
            "{issues:?}"
        );
        assert!(
            issues
                .iter()
                .any(|i| matches!(i, Issue::InWall(FurnitureId(31), _))),
            "{issues:?}"
        );
        assert!(
            !issues.iter().any(|i| matches!(
                i,
                Issue::BlocksDoor {
                    by: FurnitureId(33),
                    ..
                }
            )),
            "{issues:?}"
        );
    }

    #[test]
    fn finds_pieces_in_walls_blocked_doors_and_outside_rooms() {
        let mut home = room_home();
        home.furniture
            .push(piece(20, (250.0, 10.0), (100.0, 60.0, 80.0))); // pokes into top wall
        let mut door = piece(21, (0.0, 0.0), (80.0, 15.0, 210.0));
        door.opening = Some(Opening::default());
        let left_wall = home.walls[3].clone(); // (0,400) -> (0,0)
        align_to_wall(&mut door, &left_wall, 200.0);
        home.furniture.push(door);
        home.furniture
            .push(piece(22, (40.0, 180.0), (40.0, 40.0, 80.0))); // in the swing
        home.furniture
            .push(piece(23, (900.0, 900.0), (40.0, 40.0, 40.0))); // outside

        let issues = check_layout(&home);
        assert!(
            issues.contains(&Issue::InWall(FurnitureId(20), WallId(1))),
            "{issues:?}"
        );
        assert!(
            issues.contains(&Issue::OutsideRooms(FurnitureId(23))),
            "{issues:?}"
        );
        let swing_side = door_swing(&home.furniture[1]).unwrap();
        assert!(
            swing_side.iter().all(|p| p.x >= -8.0),
            "door swings into the room: {swing_side:?}"
        );
        assert!(
            issues.contains(&Issue::BlocksDoor {
                door: FurnitureId(21),
                by: FurnitureId(22)
            }),
            "{issues:?}"
        );
        assert!(
            !issues
                .iter()
                .any(|i| matches!(i, Issue::InWall(FurnitureId(21), _))),
            "doors belong in walls"
        );
    }
}

#[cfg(test)]
mod tilt_tests {
    use super::*;
    use crate::elements::Wall;
    use crate::furniture::{Opening, align_to_wall, wall_cuts};
    use crate::ids::WallId;

    #[test]
    fn openings_fit_sloping_walls_and_raised_slopes_do_not_collide() {
        // Gable wall rising from 1 cm to 675 cm over 300 cm.
        let mut wall = Wall::new(WallId(1), Point2::new(0.0, 0.0), Point2::new(300.0, 0.0));
        wall.height = 1.0;
        wall.height_at_end = Some(675.0);
        let mut door = Furniture {
            id: FurnitureId(2),
            catalog: "door".into(),
            width: 80.0,
            depth: 15.0,
            height: 210.0,
            opening: Some(Opening::default()),
            ..Furniture::default()
        };
        align_to_wall(&mut door, &wall, 200.0);
        let cuts = wall_cuts(std::slice::from_ref(&wall), &[door.clone()]);
        let cut = &cuts[0][0];
        // At 160 cm along, the wall is ~360 cm high: the whole door fits.
        assert!((cut.top - 210.0).abs() < 1e-9, "{cut:?}");

        // A roof slope over the room: 45°, centered 300 cm up.
        let slope = Furniture {
            id: FurnitureId(3),
            catalog: "box".into(),
            position: Point2::new(0.0, 0.0),
            width: 400.0,
            depth: 400.0,
            height: 12.0,
            elevation: 300.0,
            pitch: -45.0,
            ..Furniture::default()
        };
        let low = slope.underside_at(Point2::new(0.0, -100.0));
        let high = slope.underside_at(Point2::new(0.0, 100.0));
        assert!(high > low + 150.0, "{low} {high}");
        let fp = slope.projected_footprint();
        let depth = fp[0].distance(fp[3]);
        assert!(depth < 400.0 * 0.8, "projected depth {depth}");
        let (bottom, top) = slope.height_range();
        assert!(bottom < 200.0 && top > 450.0, "{bottom} {top}");
        // A table under the high end of the slope does not collide with it.
        let table = Furniture {
            id: FurnitureId(4),
            catalog: "box".into(),
            position: Point2::new(0.0, 100.0),
            width: 60.0,
            depth: 60.0,
            height: 75.0,
            ..Furniture::default()
        };
        let mut home = Home::default();
        home.furniture = vec![slope, table];
        assert!(check_layout(&home).is_empty(), "{:?}", check_layout(&home));
    }
}
