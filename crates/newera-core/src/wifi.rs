//! Wi-Fi coverage as a site survey estimates it: the signal of each access
//! point at every place of a room, by band, falling with distance and with
//! each wall, door or window it crosses.
//!
//! The model is the indoor one every planning tool starts from — free-space
//! loss to one metre, then a distance exponent, plus a loss per obstacle by
//! material and band. The per-wall figures are the usual planning values
//! (a 15 cm rendered brick wall costs about 8 dB at 2.4 GHz, 15 at 5 and 18
//! at 6), scaled by the wall's thickness; a real survey decides.

use serde::Serialize;

use crate::electrical::inside;
use crate::furniture::{Furniture, OpeningKind};
use crate::geometry::Point2;
use crate::home::Home;
use crate::ids::{FurnitureId, RoomId};
use crate::materials::WallFamily;

/// Where an access point keeps its standard: `wifi5`, `wifi6`, `wifi6e`,
/// `wifi7`.
pub const STANDARD_KEY: &str = "wifi:standard";
/// Whether an access point is fed by its data cable (PoE): `true`/`false`.
pub const POE_KEY: &str = "wifi:poe";

/// A radio band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Band {
    G2_4,
    G5,
    G6,
}

impl Band {
    pub const ALL: [Self; 3] = [Self::G2_4, Self::G5, Self::G6];

    pub fn key(self) -> &'static str {
        match self {
            Self::G2_4 => "2.4",
            Self::G5 => "5",
            Self::G6 => "6",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw
            .trim()
            .trim_end_matches("GHz")
            .trim_end_matches("ghz")
            .trim();
        Self::ALL
            .into_iter()
            .find(|b| b.key() == raw.replace(',', "."))
    }

    fn mhz(self) -> f64 {
        match self {
            Self::G2_4 => 2437.0,
            Self::G5 => 5500.0,
            Self::G6 => 6200.0,
        }
    }

    /// A ceiling access point's radiated power, dBm.
    fn eirp(self) -> f64 {
        match self {
            Self::G2_4 => 20.0,
            Self::G5 => 23.0,
            Self::G6 => 21.0,
        }
    }

    /// Loss of a 15 cm wall of the family, and of a door and a window, dB.
    fn wall_db(self, family: WallFamily) -> f64 {
        let [a, b, c] = match family {
            WallFamily::Drywall => [3.0, 4.0, 5.0],
            WallFamily::Masonry => [8.0, 15.0, 18.0],
            WallFamily::Concrete => [12.0, 20.0, 25.0],
            WallFamily::Glass => [2.0, 4.0, 6.0],
            WallFamily::Wood => [3.0, 5.0, 6.0],
        };
        match self {
            Self::G2_4 => a,
            Self::G5 => b,
            Self::G6 => c,
        }
    }
}

/// A Wi-Fi generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Standard {
    Wifi5,
    Wifi6,
    Wifi6e,
    Wifi7,
}

impl Standard {
    pub const ALL: [Self; 4] = [Self::Wifi5, Self::Wifi6, Self::Wifi6e, Self::Wifi7];

    pub fn key(self) -> &'static str {
        match self {
            Self::Wifi5 => "wifi5",
            Self::Wifi6 => "wifi6",
            Self::Wifi6e => "wifi6e",
            Self::Wifi7 => "wifi7",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_lowercase().replace(['-', ' '], "");
        Self::ALL.into_iter().find(|s| s.key() == raw)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Wifi5 => "Wi-Fi 5",
            Self::Wifi6 => "Wi-Fi 6",
            Self::Wifi6e => "Wi-Fi 6E",
            Self::Wifi7 => "Wi-Fi 7",
        }
    }

    pub fn bands(self) -> &'static [Band] {
        match self {
            Self::Wifi5 | Self::Wifi6 => &[Band::G2_4, Band::G5],
            Self::Wifi6e | Self::Wifi7 => &[Band::G2_4, Band::G5, Band::G6],
        }
    }

    /// The uplink it wants and the cable category that carries it: a
    /// Wi-Fi 6E access point is sold with 2.5 gigabit (Cat 5e holds it), a Wi-Fi 7
    /// one with 10 gigabit (Cat 6A for the full 100 m).
    pub fn uplink(self) -> (&'static str, crate::electrical::Category) {
        use crate::electrical::Category;
        match self {
            Self::Wifi5 | Self::Wifi6 => ("1 GbE", Category::Cat5e),
            Self::Wifi6e => ("2.5 GbE", Category::Cat5e),
            Self::Wifi7 => ("10 GbE", Category::Cat6a),
        }
    }
}

/// An access point placed on the plan.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccessPoint {
    pub id: Option<FurnitureId>,
    pub at: Point2,
    pub z: f64,
    pub standard: Standard,
}

/// The access points on the storey shown.
pub fn access_points(home: &Home) -> Vec<AccessPoint> {
    let view = home.level_view(home.current_level());
    view.furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| f.catalog == "wifi-point")
        .map(|f| AccessPoint {
            id: Some(f.id),
            at: f.position,
            z: f.elevation + f.height / 2.0,
            standard: f
                .properties
                .get(STANDARD_KEY)
                .and_then(|s| Standard::parse(s))
                .unwrap_or(Standard::Wifi6),
        })
        .collect()
}

/// Signal at a place from one access point, dBm.
pub fn signal(home: &Home, ap: &AccessPoint, at: Point2, z: f64, band: Band) -> f64 {
    let metres = ((ap.at.distance(at)).hypot(ap.z - z) / 100.0).max(1.0);
    let at_one_metre = 20.0 * band.mhz().log10() - 27.55;
    // Past the first metre indoors the signal falls a little faster than in
    // free space (exponent 2.2); the walls are counted apart.
    band.eirp() - at_one_metre - 22.0 * metres.log10() - obstacles(home, ap.at, at, band)
}

/// What the straight path between two places crosses, dB: each wall by its
/// family and thickness, or the door or window in it where the path passes
/// through one.
fn obstacles(home: &Home, a: Point2, b: Point2, band: Band) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let openings: Vec<&Furniture> = home
        .furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| f.is_opening())
        .collect();
    home.walls
        .iter()
        .filter(|w| !w.is_arc())
        .filter_map(|w| {
            let (ex, ey) = (w.end.x - w.start.x, w.end.y - w.start.y);
            let den = dx * ey - dy * ex;
            if den.abs() < 1e-9 {
                return None;
            }
            let (sx, sy) = (w.start.x - a.x, w.start.y - a.y);
            let s = (sx * ey - sy * ex) / den;
            let t = (sx * dy - sy * dx) / den;
            if !(0.0..=1.0).contains(&s) || !(0.0..=1.0).contains(&t) {
                return None;
            }
            let cross = Point2::new(a.x + s * dx, a.y + s * dy);
            let through = openings.iter().find(|o| {
                let mut widened = (**o).clone();
                widened.depth = widened.depth.max(w.thickness + 10.0);
                widened.contains(cross)
            });
            Some(
                match through.and_then(|o| o.opening.as_ref().map(|op| op.kind)) {
                    Some(OpeningKind::Passage) => 0.0,
                    Some(OpeningKind::Door) => band.wall_db(WallFamily::Wood),
                    Some(OpeningKind::Window) => band.wall_db(WallFamily::Glass),
                    None => {
                        let family = w
                            .wall_type
                            .as_deref()
                            .and_then(crate::materials::wall_type)
                            .map_or(WallFamily::Masonry, |t| t.family);
                        let scale = match family {
                            WallFamily::Masonry | WallFamily::Concrete => {
                                (w.thickness / 15.0).clamp(0.5, 2.0)
                            }
                            _ => 1.0,
                        };
                        band.wall_db(family) * scale
                    }
                },
            )
        })
        .sum()
}

/// How well a signal serves, as planning guides grade it.
pub fn grade(dbm: f64) -> &'static str {
    if dbm >= -67.0 {
        "bom: vídeo e chamada"
    } else if dbm >= -75.0 {
        "fraco: navega, chamada falha"
    } else {
        "sem cobertura útil"
    }
}

/// Coverage of one room at one band.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoomCoverage {
    pub room: RoomId,
    pub name: String,
    pub band: Band,
    /// Signal at the middle of the samples, dBm.
    pub median: f64,
    /// Signal nine places in ten reach or pass, dBm.
    pub worst: f64,
    /// Share of the room at -67 dBm or better.
    pub good: f64,
}

/// Places a room is sampled at, 1 m above the floor: a grid every 50 cm
/// inside it, or its centre when it is too small.
fn samples(points: &[Point2]) -> Vec<Point2> {
    let (min_x, max_x) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.x), hi.max(p.x))
    });
    let (min_y, max_y) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.y), hi.max(p.y))
    });
    let step = ((max_x - min_x) * (max_y - min_y) / 400.0).sqrt().max(50.0);
    let mut out = Vec::new();
    let mut y = min_y + step / 2.0;
    while y < max_y {
        let mut x = min_x + step / 2.0;
        while x < max_x {
            let p = Point2::new(x, y);
            if inside(points, p) {
                out.push(p);
            }
            x += step;
        }
        y += step;
    }
    if out.is_empty()
        && let Some(c) = crate::geometry::polygon_centroid(points)
    {
        out.push(c);
    }
    out
}

/// Every room's coverage by the given access points, per band they carry.
pub fn coverage(home: &Home, aps: &[AccessPoint]) -> Vec<RoomCoverage> {
    let view = home.level_view(home.current_level());
    let mut out = Vec::new();
    let bands: Vec<Band> = Band::ALL
        .into_iter()
        .filter(|b| aps.iter().any(|ap| ap.standard.bands().contains(b)))
        .collect();
    for room in view.rooms.iter().filter(|r| r.points.len() >= 3) {
        let places = samples(&room.points);
        if places.is_empty() {
            continue;
        }
        for band in &bands {
            let mut levels: Vec<f64> = places
                .iter()
                .map(|p| {
                    aps.iter()
                        .filter(|ap| ap.standard.bands().contains(band))
                        .map(|ap| signal(&view, ap, *p, 100.0, *band))
                        .fold(f64::MIN, f64::max)
                })
                .collect();
            levels.sort_by(f64::total_cmp);
            let at = |q: f64| {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let i = ((levels.len() - 1) as f64 * q).round() as usize;
                (levels[i] * 10.0).round() / 10.0
            };
            #[allow(clippy::cast_precision_loss)]
            let good = levels.iter().filter(|l| **l >= -67.0).count() as f64 / levels.len() as f64;
            out.push(RoomCoverage {
                room: room.id,
                name: room.name.clone(),
                band: *band,
                median: at(0.5),
                worst: at(0.1),
                good: (good * 100.0).round() / 100.0,
            });
        }
    }
    out
}

/// Rooms a Wi-Fi signal should reach well: every room but the small wet
/// ones and the outside.
fn wants_coverage(name: &str) -> bool {
    let name = crate::annotations::fold(name);
    ![
        "banh",
        "wc",
        "lavabo",
        "shaft",
        "deposito",
        "varanda",
        "sacada",
        "area tecnica",
    ]
    .iter()
    .any(|w| name.contains(w))
}

/// Where to put access points so the rooms people use get a good signal at
/// `band`: the fewest ceiling points (up to four), each at the centre of a
/// room that wants coverage,
/// chosen one at a time for the most rooms covered, then the best worst
/// room. Returns the points with the rooms still short.
pub fn suggest(
    home: &Home,
    standard: Standard,
    band: Band,
    storey: f64,
) -> (Vec<(AccessPoint, String)>, Vec<String>) {
    let view = home.level_view(home.current_level());
    let rooms: Vec<&crate::elements::Room> =
        view.rooms.iter().filter(|r| r.points.len() >= 3).collect();
    let wanted: Vec<&crate::elements::Room> = rooms
        .iter()
        .copied()
        .filter(|r| wants_coverage(&r.name))
        .collect();
    // Not in a bathroom (damp, a lowered ceiling), outside or in a shaft.
    let candidates: Vec<(AccessPoint, String)> = wanted
        .iter()
        .map(|r| {
            (
                AccessPoint {
                    id: None,
                    at: crate::geometry::polygon_centroid(&r.points).unwrap_or(r.points[0]),
                    z: storey - 10.0,
                    standard,
                },
                r.name.clone(),
            )
        })
        .collect();
    let places: Vec<Vec<Point2>> = wanted.iter().map(|r| samples(&r.points)).collect();
    // The signal nine places in ten of each wanted room get from a set.
    let score = |set: &[&AccessPoint]| -> Vec<f64> {
        places
            .iter()
            .map(|ps| {
                let mut levels: Vec<f64> = ps
                    .iter()
                    .map(|p| {
                        set.iter()
                            .map(|ap| signal(&view, ap, *p, 100.0, band))
                            .fold(f64::MIN, f64::max)
                    })
                    .collect();
                levels.sort_by(f64::total_cmp);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let i = ((levels.len() - 1) as f64 * 0.1).round() as usize;
                levels[i]
            })
            .collect()
    };
    let mut chosen: Vec<usize> = Vec::new();
    for _ in 0..4 {
        let best = (0..candidates.len())
            .filter(|i| !chosen.contains(i))
            .map(|i| {
                let set: Vec<&AccessPoint> = chosen
                    .iter()
                    .chain(std::iter::once(&i))
                    .map(|k| &candidates[*k].0)
                    .collect();
                let worst = score(&set);
                let covered = worst.iter().filter(|w| **w >= -67.0).count();
                let floor = worst.iter().copied().fold(f64::MAX, f64::min);
                (i, covered, floor)
            })
            .max_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)));
        let Some((i, covered, _)) = best else { break };
        chosen.push(i);
        if covered == wanted.len() {
            break;
        }
    }
    let set: Vec<&AccessPoint> = chosen.iter().map(|k| &candidates[*k].0).collect();
    let short = score(&set)
        .iter()
        .zip(&wanted)
        .filter(|(w, _)| **w < -67.0)
        .map(|(_, r)| r.name.clone())
        .collect();
    (
        chosen.into_iter().map(|k| candidates[k].clone()).collect(),
        short,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{Room, Wall};
    use crate::ids::WallId;

    /// Two 4 × 4 m rooms side by side, the wall between them of `family`.
    fn flat(between: Option<&str>) -> Home {
        let mut home = Home::default();
        let pts = [(0.0, 0.0), (800.0, 0.0), (800.0, 400.0), (0.0, 400.0)];
        for k in 0..4 {
            let (a, b) = (pts[k], pts[(k + 1) % 4]);
            home.walls.push(Wall::new(
                WallId(k as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            ));
        }
        let mut middle = Wall::new(
            WallId(5),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 400.0),
        );
        if let Some(id) = between {
            middle.apply_type(crate::materials::wall_type(id).unwrap());
        }
        home.walls.push(middle);
        let square = |x: f64| {
            vec![
                Point2::new(x, 0.0),
                Point2::new(x + 400.0, 0.0),
                Point2::new(x + 400.0, 400.0),
                Point2::new(x, 400.0),
            ]
        };
        home.rooms = vec![
            Room::new(RoomId(10), "Sala", square(0.0)),
            Room::new(RoomId(11), "Quarto", square(400.0)),
        ];
        home
    }

    fn ap(x: f64, y: f64, standard: Standard) -> AccessPoint {
        AccessPoint {
            id: None,
            at: Point2::new(x, y),
            z: 270.0,
            standard,
        }
    }

    #[test]
    fn a_wall_costs_more_at_higher_bands_and_the_signal_falls_with_distance() {
        let home = flat(None);
        let a = ap(200.0, 200.0, Standard::Wifi6e);
        let near = signal(&home, &a, Point2::new(250.0, 200.0), 100.0, Band::G5);
        let far = signal(&home, &a, Point2::new(380.0, 200.0), 100.0, Band::G5);
        assert!(near > far, "{near} > {far}");
        // Across the masonry wall the higher band loses more.
        let loss = |band| {
            signal(&home, &a, Point2::new(380.0, 200.0), 100.0, band)
                - signal(&home, &a, Point2::new(420.0, 200.0), 100.0, band)
        };
        assert!(
            loss(Band::G6) > loss(Band::G5) && loss(Band::G5) > loss(Band::G2_4),
            "{} {} {}",
            loss(Band::G2_4),
            loss(Band::G5),
            loss(Band::G6)
        );
    }

    #[test]
    fn a_door_lets_more_through_than_the_wall_around_it() {
        let mut home = flat(None);
        let mut door = Furniture {
            id: FurnitureId(30),
            catalog: "door".into(),
            name: "Porta".into(),
            position: Point2::new(400.0, 200.0),
            angle: 90.0,
            width: 80.0,
            depth: 15.0,
            height: 210.0,
            ..Furniture::default()
        };
        door.opening = Some(crate::furniture::Opening::default());
        home.furniture.push(door);
        let a = ap(200.0, 200.0, Standard::Wifi6);
        let through_door = signal(&home, &a, Point2::new(600.0, 200.0), 100.0, Band::G5);
        let through_wall = signal(&home, &a, Point2::new(600.0, 60.0), 100.0, Band::G5);
        // Nearly the same distance; the door costs 5 dB where the wall costs 15.
        assert!(
            through_door > through_wall + 8.0,
            "{through_door} vs {through_wall}"
        );
    }

    #[test]
    fn coverage_is_given_per_room_and_band_and_a_drywall_flat_needs_one_point() {
        let masonry = flat(None);
        let one = [ap(200.0, 200.0, Standard::Wifi6)];
        let rooms = coverage(&masonry, &one);
        assert_eq!(rooms.len(), 4, "two rooms × two bands: {rooms:#?}");
        let sala = rooms
            .iter()
            .find(|r| r.name == "Sala" && r.band == Band::G5)
            .unwrap();
        let quarto = rooms
            .iter()
            .find(|r| r.name == "Quarto" && r.band == Band::G5)
            .unwrap();
        assert!(sala.median > quarto.median, "{rooms:#?}");
        assert!(sala.good > 0.9, "{sala:?}");

        let mut with_bath = masonry.clone();
        with_bath.rooms[1].name = "Banho".into();
        let (points, _) = suggest(&with_bath, Standard::Wifi6e, Band::G6, 280.0);
        assert!(points.iter().all(|(_, room)| room != "Banho"), "{points:?}");

        let (points, short) = suggest(&masonry, Standard::Wifi6, Band::G5, 280.0);
        assert!(
            !points.is_empty() && short.is_empty(),
            "{points:?} {short:?}"
        );
        assert!(points.len() <= 2);
    }
}
