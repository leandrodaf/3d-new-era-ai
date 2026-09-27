//! Every finding the review makes over a fixed corpus, written down.
//!
//! The numbers the rules lean on are leaving the rule bodies for a table that
//! knows who demands each one and where. That move must change nothing for a
//! project in Brazil — not a severity, not a key, not a word — and a handful
//! of hand-written assertions cannot promise that about fifty-odd rules. So the
//! whole output is kept: a corpus of homes built from a seed, plus the projects
//! the repository ships, reviewed for several households at every Brazilian
//! way of saying where the project is.
//!
//! `golden-br.txt` must not move while figures are migrated. `golden-abroad.txt`
//! is the same corpus reviewed elsewhere, and moves on purpose when a foreign
//! code learns to judge. Regenerate with `NEWERA_UPDATE_GOLDEN=1`, and read the
//! diff before committing it: it is the review of the change.

// Seeds and counts are tiny; cm fit in any float.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use std::fmt::Write as _;
use std::path::PathBuf;

use newera_core::{
    Furniture, FurnitureId, Home, Opening, OpeningKind, Point2, Room, RoomId, Wall, WallId,
    project_from_bytes, standard,
};
use newera_ergonomics::{Profile, Report, review};

/// A small deterministic generator: the corpus has to be the same on every
/// machine and every run, or the snapshot is noise.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.below(1000) as f64 / 1000.0)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// Catalog id, width, depth, height and elevation, cm.
type Spec = (&'static str, f64, f64, f64, f64);

const BEDROOM: &[Spec] = &[
    ("bed-double", 160.0, 200.0, 50.0, 0.0),
    ("bed-single", 90.0, 190.0, 50.0, 0.0),
    ("nightstand", 45.0, 40.0, 55.0, 0.0),
    ("nightstand", 45.0, 40.0, 55.0, 0.0),
    ("wardrobe", 180.0, 60.0, 220.0, 0.0),
    ("dresser", 100.0, 45.0, 80.0, 0.0),
    ("desk", 120.0, 60.0, 75.0, 0.0),
    ("crib", 70.0, 130.0, 90.0, 0.0),
    ("tv", 110.0, 8.0, 65.0, 100.0),
];
const KITCHEN: &[Spec] = &[
    ("fridge", 70.0, 70.0, 180.0, 0.0),
    ("stove", 60.0, 60.0, 90.0, 0.0),
    ("sink-counter", 120.0, 60.0, 90.0, 0.0),
    ("base-cabinet", 80.0, 60.0, 90.0, 0.0),
    ("base-cabinet", 60.0, 60.0, 90.0, 0.0),
    ("dishwasher", 60.0, 60.0, 85.0, 0.0),
    ("wall-cabinet", 80.0, 35.0, 70.0, 150.0),
    ("wall-cabinet", 80.0, 35.0, 70.0, 125.0),
    ("hood", 60.0, 50.0, 40.0, 165.0),
    ("kitchen-island", 180.0, 90.0, 90.0, 0.0),
    ("dining-set-4", 160.0, 160.0, 75.0, 0.0),
    ("stool", 40.0, 40.0, 75.0, 0.0),
    ("microwave", 50.0, 40.0, 30.0, 150.0),
];
const BATHROOM: &[Spec] = &[
    ("toilet", 38.0, 65.0, 40.0, 0.0),
    ("basin-cabinet", 60.0, 45.0, 85.0, 0.0),
    ("shower", 90.0, 90.0, 200.0, 0.0),
    ("bathtub", 170.0, 75.0, 55.0, 0.0),
];
const LIVING: &[Spec] = &[
    ("sofa-3", 210.0, 90.0, 85.0, 0.0),
    ("sofa-2", 160.0, 90.0, 85.0, 0.0),
    ("armchair", 80.0, 80.0, 85.0, 0.0),
    ("coffee-table", 100.0, 60.0, 40.0, 0.0),
    ("tv-stand", 160.0, 40.0, 50.0, 0.0),
    ("tv", 130.0, 8.0, 75.0, 90.0),
    ("dining-table-4", 120.0, 80.0, 75.0, 0.0),
    ("dining-table-6", 180.0, 90.0, 75.0, 0.0),
    ("chair", 45.0, 50.0, 90.0, 0.0),
    ("chair", 45.0, 50.0, 90.0, 0.0),
    ("bookcase", 80.0, 30.0, 200.0, 0.0),
];
const LAUNDRY: &[Spec] = &[
    ("washer", 60.0, 60.0, 85.0, 0.0),
    ("dryer", 60.0, 60.0, 85.0, 0.0),
    ("laundry-sink", 60.0, 55.0, 90.0, 0.0),
];
const OFFICE: &[Spec] = &[
    ("desk", 140.0, 70.0, 75.0, 0.0),
    ("office-chair", 60.0, 60.0, 100.0, 0.0),
    ("bookcase", 80.0, 30.0, 200.0, 0.0),
    ("armchair", 80.0, 80.0, 85.0, 0.0),
];
/// Points of the electrical project, which some rules count and others reach.
const POINTS: &[Spec] = &[
    ("outlet", 8.0, 2.0, 8.0, 30.0),
    ("outlet", 8.0, 2.0, 8.0, 110.0),
    ("outlet", 8.0, 2.0, 8.0, 20.0),
    ("switch", 8.0, 2.0, 8.0, 110.0),
    ("switch", 8.0, 2.0, 8.0, 140.0),
];

/// A kind of room: its name, the pieces it draws from, and the sizes it
/// takes, from cramped to generous.
struct Kind {
    name: &'static str,
    pool: &'static [Spec],
    width: (f64, f64),
    depth: (f64, f64),
}

const KINDS: &[Kind] = &[
    Kind {
        name: "Quarto",
        pool: BEDROOM,
        width: (200.0, 420.0),
        depth: (220.0, 420.0),
    },
    Kind {
        name: "Suíte",
        pool: BEDROOM,
        width: (260.0, 460.0),
        depth: (280.0, 420.0),
    },
    Kind {
        name: "Cozinha",
        pool: KITCHEN,
        width: (150.0, 420.0),
        depth: (170.0, 400.0),
    },
    Kind {
        name: "Banheiro",
        pool: BATHROOM,
        width: (100.0, 260.0),
        depth: (150.0, 280.0),
    },
    Kind {
        name: "Sala",
        pool: LIVING,
        width: (230.0, 520.0),
        depth: (230.0, 500.0),
    },
    Kind {
        name: "Área de serviço",
        pool: LAUNDRY,
        width: (80.0, 220.0),
        depth: (120.0, 250.0),
    },
    Kind {
        name: "Corredor",
        pool: &[],
        width: (70.0, 130.0),
        depth: (200.0, 600.0),
    },
    Kind {
        name: "Escritório",
        pool: OFFICE,
        width: (180.0, 360.0),
        depth: (200.0, 360.0),
    },
];

struct Builder {
    home: Home,
    next: u64,
    rng: Rng,
}

impl Builder {
    fn new(seed: u64) -> Self {
        Self {
            home: Home::default(),
            next: 100,
            rng: Rng(seed.wrapping_mul(2_654_435_761).wrapping_add(17)),
        }
    }

    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn wall(&mut self, a: (f64, f64), b: (f64, f64), height: f64) {
        let id = self.id();
        let mut wall = Wall::new(WallId(id), Point2::new(a.0, a.1), Point2::new(b.0, b.1));
        wall.thickness = 15.0;
        wall.height = height;
        self.home.walls.push(wall);
    }

    fn room(&mut self, name: &str, (x, y): (f64, f64), (w, d): (f64, f64)) {
        let id = self.id();
        let h = 7.5;
        self.home.rooms.push(Room::new(
            RoomId(id),
            name.to_owned(),
            vec![
                Point2::new(x + h, y + h),
                Point2::new(x + w - h, y + h),
                Point2::new(x + w - h, y + d - h),
                Point2::new(x + h, y + d - h),
            ],
        ));
    }

    /// A door or window centred at `at` on a wall running along x (`angle`
    /// 0) or y (`angle` 90).
    fn opening(&mut self, kind: OpeningKind, at: (f64, f64), angle: f64, width: f64) {
        let id = self.id();
        let (name, height, elevation) = match kind {
            OpeningKind::Window => ("window", 120.0, 100.0),
            _ => ("door", 210.0, 0.0),
        };
        let hinge_right = self.rng.chance(50);
        let sliding = kind == OpeningKind::Door && self.rng.chance(15);
        self.home.furniture.push(Furniture {
            id: FurnitureId(id),
            catalog: name.into(),
            name: name.into(),
            position: Point2::new(at.0, at.1),
            angle,
            width,
            depth: 15.0,
            height,
            elevation,
            opening: Some(Opening {
                kind,
                hinge_right,
                sliding,
                ..Opening::default()
            }),
            ..Furniture::default()
        });
    }

    /// A piece with its back on one of the four walls of the room at `(x, y)`
    /// of size `(w, d)`, facing in; or free-standing near the middle.
    fn piece(&mut self, spec: Spec, (x, y): (f64, f64), (w, d): (f64, f64)) {
        let (catalog, pw, pd, ph, elevation) = spec;
        let h = 7.5;
        let (ix, iy, iw, id) = (x + h, y + h, w - 2.0 * h, d - 2.0 * h);
        let middle = matches!(
            catalog,
            "kitchen-island"
                | "dining-set-4"
                | "dining-table-4"
                | "dining-table-6"
                | "coffee-table"
        ) && self.rng.chance(70);
        let (cx, cy, angle) = if middle {
            (
                ix + iw / 2.0 + self.rng.range(-30.0, 30.0),
                iy + id / 2.0 + self.rng.range(-30.0, 30.0),
                if self.rng.chance(50) { 0.0 } else { 90.0 },
            )
        } else {
            let along = |rng: &mut Rng, len: f64, size: f64| {
                if len <= size {
                    len / 2.0
                } else {
                    rng.range(size / 2.0, len - size / 2.0)
                }
            };
            match self.rng.below(4) {
                // Back on the top wall, front to +y.
                0 => (ix + along(&mut self.rng, iw, pw), iy + pd / 2.0, 0.0),
                // Back on the bottom wall, front to -y.
                1 => (ix + along(&mut self.rng, iw, pw), iy + id - pd / 2.0, 180.0),
                // Back on the left wall, front to +x.
                2 => (ix + pd / 2.0, iy + along(&mut self.rng, id, pw), 270.0),
                // Back on the right wall, front to -x.
                _ => (ix + iw - pd / 2.0, iy + along(&mut self.rng, id, pw), 90.0),
            }
        };
        let identifier = self.id();
        let point = matches!(catalog, "outlet" | "switch");
        self.home.furniture.push(Furniture {
            id: FurnitureId(identifier),
            catalog: catalog.into(),
            name: catalog.into(),
            position: Point2::new(cx, cy),
            angle,
            width: pw,
            depth: pd,
            height: ph,
            elevation,
            mirrored: self.rng.chance(10),
            discipline: point.then_some(newera_core::Discipline::Electrical),
            ..Furniture::default()
        });
    }

    /// Some of the pieces a room of this kind draws from.
    fn furnish(&mut self, kind: &Kind, at: (f64, f64), size: (f64, f64), electrical: bool) {
        for &spec in kind.pool {
            if self.rng.chance(55) {
                self.piece(spec, at, size);
            }
        }
        if electrical {
            for &spec in POINTS {
                if self.rng.chance(60) {
                    self.piece(spec, at, size);
                }
            }
        }
    }

    fn size(&mut self, kind: &Kind) -> (f64, f64) {
        (
            self.rng.range(kind.width.0, kind.width.1).round(),
            self.rng.range(kind.depth.0, kind.depth.1).round(),
        )
    }
}

/// One room alone, with a door and maybe a window.
fn single(seed: u64, kind: &Kind) -> Home {
    let mut b = Builder::new(seed);
    let (w, d) = b.size(kind);
    let height = if b.rng.chance(25) { 235.0 } else { 260.0 };
    b.wall((0.0, 0.0), (w, 0.0), height);
    b.wall((w, 0.0), (w, d), height);
    b.wall((w, d), (0.0, d), height);
    b.wall((0.0, d), (0.0, 0.0), height);
    b.room(kind.name, (0.0, 0.0), (w, d));
    if b.rng.chance(85) {
        let door = b.rng.range(60.0, 90.0).round();
        let x = (w / 2.0).max(door / 2.0 + 10.0);
        b.opening(OpeningKind::Door, (x, d), 0.0, door);
    }
    if b.rng.chance(70) {
        let window = b.rng.range(40.0, 160.0).round().min(w - 30.0).max(30.0);
        b.opening(OpeningKind::Window, (w / 2.0, 0.0), 0.0, window);
    }
    let electrical = b.rng.chance(50);
    b.furnish(kind, (0.0, 0.0), (w, d), electrical);
    b.home
}

/// A row of rooms along a corridor, doors between them: the rules that read
/// the home as a whole — people, bathrooms, air shared through an opening.
fn apartment(seed: u64) -> Home {
    let mut b = Builder::new(seed ^ 0xA5A5);
    let count = 3 + b.rng.below(2) as usize;
    let depth = b.rng.range(280.0, 420.0).round();
    let corridor = b.rng.range(80.0, 130.0).round();
    let height = if b.rng.chance(20) { 240.0 } else { 260.0 };
    let electrical = b.rng.chance(50);
    let mut x = 0.0;
    let mut rooms = Vec::new();
    for k in 0..count {
        let kind = &KINDS[(b.rng.below(KINDS.len() as u64 - 1) as usize + k) % KINDS.len()];
        let kind = if kind.name == "Corredor" {
            &KINDS[0]
        } else {
            kind
        };
        let w = b.rng.range(kind.width.0, kind.width.1).round();
        rooms.push((kind, x, w));
        x += w;
    }
    let total = x;
    // The outline, and the corridor wall the doors open onto.
    b.wall((0.0, 0.0), (total, 0.0), height);
    b.wall((total, 0.0), (total, depth + corridor), height);
    b.wall((total, depth + corridor), (0.0, depth + corridor), height);
    b.wall((0.0, depth + corridor), (0.0, 0.0), height);
    b.wall((0.0, depth), (total, depth), height);
    b.room("Corredor", (0.0, depth), (total, corridor));
    for (k, &(kind, x, w)) in rooms.iter().enumerate() {
        if k > 0 {
            b.wall((x, 0.0), (x, depth), height);
        }
        b.room(kind.name, (x, 0.0), (w, depth));
        if b.rng.chance(90) {
            let door = b.rng.range(60.0, 90.0).round();
            b.opening(OpeningKind::Door, (x + w / 2.0, depth), 0.0, door);
        } else {
            b.opening(OpeningKind::Passage, (x + w / 2.0, depth), 0.0, 80.0);
        }
        if b.rng.chance(75) {
            let window = b.rng.range(40.0, 200.0).round().min(w - 40.0).max(30.0);
            b.opening(OpeningKind::Window, (x + w / 2.0, 0.0), 0.0, window);
        }
        b.furnish(kind, (x, 0.0), (w, depth), electrical);
    }
    // The way in.
    b.opening(OpeningKind::Door, (40.0, depth + corridor), 0.0, 80.0);
    b.home
}

fn shipped() -> Vec<(String, Home)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for path in [
        "web/demo.newera",
        "docs/projects/upper-west-side-reviewed.newera",
    ] {
        let bytes = std::fs::read(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let (project, _) = project_from_bytes(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
        for (name, home) in project.variants {
            // The project's own place and acceptances are part of what it is,
            // but here the place is what the corpus varies: start from none.
            let mut home = home;
            home.compass.city = None;
            home.compass.country = None;
            home.compass.region = None;
            out.push((format!("{path} [{name}]"), home));
        }
    }
    out
}

/// A closed square room, as the unit tests draw one.
fn square(name: &str, w: f64, d: f64) -> Builder {
    let mut b = Builder::new(0);
    b.wall((0.0, 0.0), (w, 0.0), 260.0);
    b.wall((w, 0.0), (w, d), 260.0);
    b.wall((w, d), (0.0, d), 260.0);
    b.wall((0.0, d), (0.0, 0.0), 260.0);
    b.room(name, (0.0, 0.0), (w, d));
    b
}

/// A piece at an exact spot, with an optional name.
fn at(
    b: &mut Builder,
    catalog: &str,
    name: &str,
    xy: (f64, f64),
    size: (f64, f64, f64),
    angle: f64,
    elevation: f64,
) {
    let id = b.id();
    b.home.furniture.push(Furniture {
        id: FurnitureId(id),
        catalog: catalog.into(),
        name: if name.is_empty() {
            catalog.into()
        } else {
            name.into()
        },
        position: Point2::new(xy.0, xy.1),
        angle,
        width: size.0,
        depth: size.1,
        height: size.2,
        elevation,
        ..Furniture::default()
    });
}

/// The rules a random corpus rarely trips, drawn on purpose: two beds too
/// close, a sink against the stove, a hood narrower than the cooktop, a short
/// worktop, a tight triangle, gas where people sleep and wash.
fn drawn() -> Vec<(String, Home)> {
    let mut out = Vec::new();

    let mut b = square("Quarto", 400.0, 400.0);
    at(
        &mut b,
        "bed-single",
        "",
        (100.0, 107.5),
        (90.0, 200.0, 50.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "bed-single",
        "",
        (230.0, 107.5),
        (90.0, 200.0, 50.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "bed-single",
        "",
        (340.0, 107.5),
        (90.0, 200.0, 50.0),
        0.0,
        0.0,
    );
    out.push(("Quarto de três camas".to_owned(), b.home));

    let mut b = square("Cozinha", 400.0, 300.0);
    at(
        &mut b,
        "sink-counter",
        "",
        (100.0, 37.5),
        (120.0, 60.0, 92.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "stove",
        "Fogão 5 bocas",
        (190.0, 38.5),
        (75.0, 62.0, 90.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "hood",
        "",
        (190.0, 32.5),
        (60.0, 50.0, 60.0),
        0.0,
        150.0,
    );
    at(
        &mut b,
        "fridge",
        "",
        (270.0, 42.5),
        (70.0, 70.0, 180.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "base-cabinet",
        "",
        (340.0, 37.5),
        (40.0, 60.0, 90.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "wall-cabinet",
        "",
        (100.0, 25.0),
        (80.0, 35.0, 70.0),
        0.0,
        120.0,
    );
    at(
        &mut b,
        "outlet",
        "",
        (100.0, 9.0),
        (8.0, 2.0, 8.0),
        0.0,
        110.0,
    );
    out.push(("Cozinha apertada".to_owned(), b.home));

    let mut b = square("Cozinha", 700.0, 500.0);
    at(
        &mut b,
        "sink-counter",
        "",
        (80.0, 37.5),
        (120.0, 60.0, 90.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "stove",
        "",
        (640.0, 38.5),
        (60.0, 62.0, 90.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "fridge",
        "",
        (650.0, 450.0),
        (70.0, 70.0, 180.0),
        180.0,
        0.0,
    );
    at(
        &mut b,
        "base-cabinet",
        "",
        (300.0, 37.5),
        (100.0, 60.0, 90.0),
        0.0,
        0.0,
    );
    out.push(("Cozinha espalhada".to_owned(), b.home));

    let mut b = square("Banheiro", 200.0, 250.0);
    at(
        &mut b,
        "basin-cabinet",
        "Lavatório",
        (100.0, 30.0),
        (60.0, 45.0, 90.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "imported",
        "Aquecedor a gás",
        (30.0, 200.0),
        (40.0, 20.0, 60.0),
        0.0,
        150.0,
    );
    at(
        &mut b,
        "toilet",
        "",
        (160.0, 40.0),
        (38.0, 65.0, 40.0),
        0.0,
        0.0,
    );
    out.push(("Banheiro com aquecedor".to_owned(), b.home));

    let mut b = square("Studio", 500.0, 400.0);
    at(
        &mut b,
        "bed-double",
        "",
        (100.0, 107.5),
        (140.0, 200.0, 50.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "stove",
        "",
        (400.0, 37.5),
        (60.0, 60.0, 90.0),
        0.0,
        0.0,
    );
    out.push(("Studio com fogão".to_owned(), b.home));

    let mut b = square("Cozinha", 380.0, 185.0);
    at(
        &mut b,
        "sink-counter",
        "",
        (80.0, 37.5),
        (120.0, 60.0, 90.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "stove",
        "",
        (200.0, 38.5),
        (60.0, 62.0, 90.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "fridge",
        "",
        (330.0, 42.5),
        (70.0, 70.0, 180.0),
        0.0,
        0.0,
    );
    at(
        &mut b,
        "base-cabinet",
        "",
        (280.0, 37.5),
        (60.0, 60.0, 90.0),
        0.0,
        0.0,
    );
    out.push(("Cozinha estreita".to_owned(), b.home));

    out
}

fn corpus() -> Vec<(String, Home)> {
    let mut out = shipped();
    out.extend(drawn());
    for kind in KINDS {
        for seed in 0..6 {
            out.push((format!("{} #{seed}", kind.name), single(seed, kind)));
        }
    }
    for seed in 0..8 {
        out.push((format!("Apartamento #{seed}"), apartment(seed)));
    }
    out
}

fn households() -> Vec<(&'static str, Profile)> {
    vec![
        ("casal", Profile::default()),
        (
            "cadeirante",
            Profile {
                wheelchair: true,
                ..Profile::default()
            },
        ),
        (
            "família",
            Profile {
                occupants: 7,
                children: 2,
                elderly: 1,
                stature: Some(152.0),
                ..Profile::default()
            },
        ),
    ]
}

/// Where the project says it is: the compass, as `set_home` writes it.
type Where = (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
);

const IN_BRAZIL: &[Where] = &[
    ("sem endereço", None, None, None),
    ("br", Some("br"), None, None),
    ("são paulo", None, None, Some("sao-paulo")),
    ("estado de sp", None, None, Some("estado-sp")),
    ("curitiba", Some("br"), Some("pr"), Some("curitiba")),
];

const ABROAD: &[Where] = &[
    ("miami", Some("us"), Some("fl"), Some("miami")),
    ("berlim", Some("de"), Some("be"), Some("berlin")),
    ("lisboa", Some("pt"), None, None),
];

fn write_report(out: &mut String, report: &Report) {
    let scores: Vec<String> = report
        .scores
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    let _ = writeln!(
        out,
        "  score={} [{}] capacity={:?}",
        report.score,
        scores.join(" "),
        report.capacity
    );
    let refs: Vec<String> = report
        .refs
        .iter()
        .map(|s| format!("{}={}", s.code, s.force(&report.place).letter()))
        .collect();
    let _ = writeln!(out, "  refs {}", refs.join(" "));
    for f in &report.findings {
        let _ = writeln!(
            out,
            "  {:?} {} ref={} w={}{}{} | {}{}",
            f.severity,
            f.key,
            f.reference.unwrap_or("-"),
            f.weight,
            f.accepted
                .as_ref()
                .map_or(String::new(), |a| format!(" accepted={a:?}")),
            f.accepted_as
                .as_ref()
                .map_or(String::new(), |a| format!(" as={a}")),
            f.message,
            f.fix
                .as_ref()
                .map_or(String::new(), |fix| format!(" | fix={fix}")),
        );
    }
}

/// The review at `place`, as text.
fn reviewed(home: &Home, (_, country, region, city): Where, profile: &Profile) -> String {
    let mut home = home.clone();
    home.compass.country = country.map(Into::into);
    home.compass.region = region.map(Into::into);
    home.compass.city = city.map(Into::into);
    let mut out = String::new();
    write_report(&mut out, &review(&home, profile));
    out
}

/// Every home reviewed where nothing is said about the place — in full when
/// `base` asks for it — and then, at each of `places`, only the lines that
/// differ from that: what the place changes, in a file small enough to read.
fn snapshot(places: &[Where], base: bool) -> String {
    let mut out = String::new();
    for (name, home) in corpus() {
        for (who, profile) in households() {
            let home_base = reviewed(&home, IN_BRAZIL[0], &profile);
            let _ = writeln!(out, "{name} / {who}");
            if base {
                out.push_str(&home_base);
            }
            let base = home_base;
            for &place in places {
                let here = reviewed(&home, place, &profile);
                if here == base {
                    continue;
                }
                let _ = writeln!(out, " @ {}", place.0);
                for line in base.lines().filter(|l| !here.lines().any(|h| h == *l)) {
                    let _ = writeln!(out, "  -{line}");
                }
                for line in here.lines().filter(|l| !base.lines().any(|b| b == *l)) {
                    let _ = writeln!(out, "  +{line}");
                }
            }
        }
    }
    out
}

fn check(file: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(file);
    if std::env::var_os("NEWERA_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — run with NEWERA_UPDATE_GOLDEN=1", path.display()));
    if expected != actual {
        let first = expected
            .lines()
            .zip(actual.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map_or_else(
                || {
                    format!(
                        "lengths differ: {} vs {} lines",
                        expected.lines().count(),
                        actual.lines().count()
                    )
                },
                |(n, (a, b))| format!("line {}:\n  was: {a}\n  now: {b}", n + 1),
            );
        panic!(
            "{file} moved. First difference at {first}\nRegenerate with NEWERA_UPDATE_GOLDEN=1 and read the diff."
        );
    }
}

#[test]
#[ignore = "slow in debug: `make golden` runs it optimised"]
fn no_finding_moves_in_brazil() {
    check("golden-br.txt", &snapshot(&IN_BRAZIL[1..], true));
}

#[test]
#[ignore = "slow in debug: `make golden` runs it optimised"]
fn what_the_same_homes_hear_abroad() {
    check("golden-abroad.txt", &snapshot(ABROAD, false));
}

#[test]
#[ignore = "slow in debug: `make golden` runs it optimised"]
fn the_corpus_reaches_most_rules() {
    // A snapshot of a corpus that never trips a rule proves nothing about it.
    let text = snapshot(&IN_BRAZIL[2..3], true);
    let mut seen = std::collections::BTreeSet::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let first = words.next().map(|w| w.trim_start_matches(['-', '+']));
        if let (Some(sev), Some(key)) = (first, words.next())
            && matches!(sev, "Erro" | "Alerta" | "Dica")
        {
            seen.insert(key.split(':').next().unwrap_or_default().to_owned());
        }
    }
    let _ = standard("nbr9050");
    eprintln!("rules reached: {} — {seen:?}", seen.len());
    assert!(
        seen.len() >= 40,
        "only {} rules reached: {seen:?}",
        seen.len()
    );
}

/// The same room, as an American plan would call it.
fn in_english(name: &str) -> String {
    [
        ("Área de serviço", "Laundry"),
        ("Escritório", "Office"),
        ("Corredor", "Hallway"),
        ("Cozinha", "Kitchen"),
        ("Banheiro", "Bathroom"),
        ("Suíte", "Master bedroom"),
        ("Quarto", "Bedroom"),
        ("Sala", "Living room"),
    ]
    .iter()
    .find(|(pt, _)| name == *pt)
    .map_or_else(|| name.to_owned(), |(_, en)| (*en).to_owned())
}

#[test]
#[ignore = "slow in debug: `make golden` runs it optimised"]
fn a_home_named_in_english_is_read_as_the_same_home() {
    // A room is recognised by what its name means, not by a Portuguese word:
    // the same plan with its rooms named in English is the same review — the
    // same rules, the same pieces, the same sources — only the labels differ.
    let what = |report: &Report| {
        let mut out: Vec<String> = report
            .findings
            .iter()
            .map(|f| format!("{:?} {} {}", f.severity, f.key, f.reference.unwrap_or("-")))
            .collect();
        out.sort();
        out
    };
    for (name, home) in corpus().into_iter().filter(|(n, _)| !n.contains(".newera")) {
        let mut english = home.clone();
        for room in &mut english.rooms {
            room.name = in_english(&room.name);
        }
        for (who, profile) in households() {
            assert_eq!(
                what(&review(&english, &profile)),
                what(&review(&home, &profile)),
                "{name} / {who}"
            );
        }
    }
}
