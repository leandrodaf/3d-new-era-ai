//! The electrical and telecom project as an engineer reads it: points by
//! kind, circuits with their load, and what NBR 5410 asks of each room.
//!
//! A point is a piece of the electrical project; what it is comes from its
//! catalog entry, and the circuit it belongs to is written on it
//! (`elec:circuit`), like its power when it is not the norm's default
//! (`elec:va`). Nothing is invented where the norm's figure is not held: a
//! rule backed by a paid standard only checks that the thing exists.

use serde::Serialize;

use crate::elements::Room;
use crate::furniture::Furniture;
use crate::geometry::Point2;
use crate::home::Home;
use crate::ids::{FurnitureId, RoomId};
use crate::style::Discipline;

/// Where a point keeps its circuit.
pub const CIRCUIT_KEY: &str = "elec:circuit";
/// Where a point keeps its power, VA, when it is not the default.
pub const VA_KEY: &str = "elec:va";
/// Where the project keeps its supply voltage, V (default 127).
pub const VOLTAGE_KEY: &str = "elec:voltage";

/// What an electrical or telecom point is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointKind {
    /// A lighting point (ceiling, wall, a fixture that lights).
    Lighting,
    /// General-use outlet (TUG).
    Outlet,
    /// Specific-use outlet (TUE): shower, air conditioning.
    Dedicated,
    Switch,
    /// Distribution board (QDC).
    Panel,
    /// RJ45 network point.
    Network,
    /// Coaxial TV point.
    Tv,
    Wifi,
    /// Telecom panel / rack.
    TelecomPanel,
    Other,
}

impl PointKind {
    /// Whether it draws power on a circuit.
    pub fn loads(self) -> bool {
        matches!(self, Self::Lighting | Self::Outlet | Self::Dedicated)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Lighting => "Iluminação",
            Self::Outlet => "TUG",
            Self::Dedicated => "TUE",
            Self::Switch => "Interruptor",
            Self::Panel => "Quadro de distribuição",
            Self::Network => "Rede",
            Self::Tv => "TV",
            Self::Wifi => "Wi-Fi",
            Self::TelecomPanel => "Quadro de telecom",
            Self::Other => "Outro",
        }
    }
}

/// The kind of a piece of the electrical project, from its catalog entry.
pub fn point_kind(piece: &Furniture) -> PointKind {
    match piece.catalog.as_str() {
        "light-ceiling" | "light-wall" | "downlight" | "pendant" | "led-panel" | "led-strip" => {
            PointKind::Lighting
        }
        "outlet-low" | "outlet-mid" | "outlet-high" => PointKind::Outlet,
        "ac-point" | "shower-point" => PointKind::Dedicated,
        "switch" | "switch-double" | "switch-3way" => PointKind::Switch,
        "electrical-panel" => PointKind::Panel,
        "network-outlet" | "data-outlet" => PointKind::Network,
        "tv-outlet" => PointKind::Tv,
        "wifi-point" => PointKind::Wifi,
        "telecom-panel" => PointKind::TelecomPanel,
        _ if piece.light.is_some() => PointKind::Lighting,
        _ => PointKind::Other,
    }
}

/// What a room is, as far as NBR 5410 cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wet {
    /// Kitchens, pantries, laundries, service areas: 1 outlet per 3.5 m.
    Kitchen,
    /// Bathrooms: 1 outlet near the basin.
    Bathroom,
    /// Living rooms and bedrooms: 1 outlet per 5 m.
    Living,
    /// Balconies: at least 1 outlet.
    Balcony,
    Other,
}

fn room_class(room: &Room) -> Wet {
    let name = crate::annotations::fold(&room.name);
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if has(&["cozinha", "copa", "lavanderia", "servico", "gourmet"]) {
        Wet::Kitchen
    } else if has(&["banh", "wc", "lavabo", "sanitario"]) {
        Wet::Bathroom
    } else if has(&[
        "sala",
        "quarto",
        "dormit",
        "suite",
        "escritorio",
        "home office",
        "estar",
        "jantar",
    ]) {
        Wet::Living
    } else if has(&["varanda", "sacada", "terraco"]) {
        Wet::Balcony
    } else {
        Wet::Other
    }
}

/// Whether a room is one people stay in, where a network point belongs.
fn long_stay(room: &Room) -> bool {
    let name = crate::annotations::fold(&room.name);
    [
        "sala",
        "quarto",
        "dormit",
        "suite",
        "escritorio",
        "home office",
        "estar",
    ]
    .iter()
    .any(|w| name.contains(w))
}

fn perimeter(room: &Room) -> f64 {
    let p = &room.points;
    p.iter()
        .zip(p.iter().cycle().skip(1))
        .take(p.len())
        .map(|(a, b)| a.distance(*b))
        .sum()
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

/// One point of the project, located and loaded.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Point {
    pub id: FurnitureId,
    pub kind: PointKind,
    pub name: String,
    pub room: Option<RoomId>,
    pub circuit: Option<String>,
    /// Power, VA (0 for points that draw none).
    pub va: f64,
}

/// Every point of the electrical project on the storey shown, with the power
/// NBR 5410 assigns it when none is written: 100 VA per lighting point; in
/// kitchens, laundries and bathrooms 600 VA for each of the first three
/// outlets and 100 VA after; 100 VA elsewhere; a dedicated point its own.
pub fn points(home: &Home) -> Vec<Point> {
    let view = home.level_view(home.current_level());
    let room_of = |p: Point2| {
        view.rooms
            .iter()
            .filter(|r| r.points.len() >= 3 && inside(&r.points, p))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
    };
    let mut outlets_in: std::collections::BTreeMap<RoomId, usize> =
        std::collections::BTreeMap::new();
    let mut out: Vec<Point> = Vec::new();
    let mut pieces: Vec<&Furniture> = view
        .furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| f.discipline == Some(Discipline::Electrical) || f.light.is_some())
        .collect();
    pieces.sort_by(|a, b| {
        (a.position.y, a.position.x)
            .partial_cmp(&(b.position.y, b.position.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for piece in pieces {
        let kind = point_kind(piece);
        let room = room_of(piece.position);
        let written = piece
            .properties
            .get(VA_KEY)
            .and_then(|v| v.parse::<f64>().ok());
        let va = written.unwrap_or_else(|| match kind {
            PointKind::Lighting => 100.0,
            PointKind::Outlet => {
                let wet =
                    room.is_some_and(|r| matches!(room_class(r), Wet::Kitchen | Wet::Bathroom));
                let n = room.map_or(0, |r| {
                    let count = outlets_in.entry(r.id).or_default();
                    *count += 1;
                    *count
                });
                if wet && n <= 3 { 600.0 } else { 100.0 }
            }
            PointKind::Dedicated => match piece.catalog.as_str() {
                "shower-point" => 5500.0,
                _ => 1500.0,
            },
            _ => 0.0,
        });
        out.push(Point {
            id: piece.id,
            kind,
            name: piece.name.clone(),
            room: room.map(|r| r.id),
            circuit: piece
                .properties
                .get(CIRCUIT_KEY)
                .cloned()
                .filter(|c| !c.trim().is_empty()),
            va,
        });
    }
    out
}

/// One circuit of the load schedule.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Circuit {
    pub name: String,
    /// What it feeds: lighting, TUG, TUE — more than one is a finding.
    pub kinds: Vec<PointKind>,
    pub points: Vec<FurnitureId>,
    pub va: f64,
    pub volts: f64,
    pub amps: f64,
    /// Conductor section, mm²: the larger of what the current needs and the
    /// minimum NBR 5410 sets (1.5 for lighting, 2.5 for power).
    pub wire_mm2: f64,
    /// Breaker, A: the smallest standard size carrying the current that the
    /// conductor still protects.
    pub breaker_a: u32,
    /// Needs a residual-current device (DR 30 mA): it serves a bathroom, a
    /// kitchen, a laundry or a balcony.
    pub rcd: bool,
}

/// Conductor sections, mm², with what two loaded copper conductors in PVC
/// carry in conduit in masonry (reference method B1), A.
const SECTIONS: [(f64, f64); 7] = [
    (1.5, 17.5),
    (2.5, 24.0),
    (4.0, 32.0),
    (6.0, 41.0),
    (10.0, 57.0),
    (16.0, 76.0),
    (25.0, 101.0),
];
const BREAKERS: [u32; 10] = [10, 16, 20, 25, 32, 40, 50, 63, 80, 100];

/// The load schedule: every circuit written on a point, sized.
pub fn circuits(home: &Home) -> Vec<Circuit> {
    let supply = home
        .properties
        .get(VOLTAGE_KEY)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(127.0);
    let view = home.level_view(home.current_level());
    let all = points(home);
    let mut names: Vec<String> = all.iter().filter_map(|p| p.circuit.clone()).collect();
    names.sort_by_key(|n| natural(n));
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let mine: Vec<&Point> = all
                .iter()
                .filter(|p| p.circuit.as_deref() == Some(name.as_str()))
                .collect();
            let mut kinds: Vec<PointKind> =
                mine.iter().map(|p| p.kind).filter(|k| k.loads()).collect();
            kinds.sort();
            kinds.dedup();
            let va: f64 = mine.iter().map(|p| p.va).sum();
            // A dedicated load over 4.4 kVA (a shower) runs on 220 V.
            let volts = if supply < 200.0 && kinds == [PointKind::Dedicated] && va > 4400.0 {
                220.0
            } else {
                supply
            };
            let amps = va / volts;
            let minimum = if kinds.iter().all(|k| *k == PointKind::Lighting) {
                1.5
            } else {
                2.5
            };
            let (wire, capacity) = SECTIONS
                .iter()
                .copied()
                .find(|(mm2, carries)| *mm2 >= minimum && *carries >= amps * 1.0)
                .unwrap_or(SECTIONS[SECTIONS.len() - 1]);
            let breaker = BREAKERS
                .iter()
                .copied()
                .find(|b| f64::from(*b) >= amps && f64::from(*b) <= capacity)
                .unwrap_or(BREAKERS[BREAKERS.len() - 1]);
            let rcd = mine.iter().any(|p| {
                p.kind.loads()
                    && p.room
                        .and_then(|id| view.rooms.iter().find(|r| r.id == id))
                        .is_some_and(|r| {
                            matches!(room_class(r), Wet::Kitchen | Wet::Bathroom | Wet::Balcony)
                        })
            });
            Circuit {
                name,
                kinds,
                points: mine.iter().map(|p| p.id).collect(),
                va,
                volts,
                amps: (amps * 10.0).round() / 10.0,
                wire_mm2: wire,
                breaker_a: breaker,
                rcd,
            }
        })
        .collect()
}

/// Sorts `C2` before `C10`.
fn natural(name: &str) -> (String, u64) {
    let digits: String = name.chars().filter(char::is_ascii_digit).collect();
    let letters: String = name.chars().filter(|c| !c.is_ascii_digit()).collect();
    (letters, digits.parse().unwrap_or(0))
}

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Erro,
    Alerta,
    Dica,
}

/// Something the project lacks, with the source it stands on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub place: String,
    pub message: String,
    pub source: &'static str,
}

/// What NBR 5410 asks of each room, and what a home's network needs.
pub fn check(home: &Home) -> Vec<Finding> {
    let view = home.level_view(home.current_level());
    let all = points(home);
    let mut out = Vec::new();
    let count = |room: RoomId, kind: PointKind| {
        all.iter()
            .filter(|p| p.room == Some(room) && p.kind == kind)
            .count()
    };
    let any_electrical = !all.is_empty();
    if !any_electrical {
        return out;
    }
    for room in view.rooms.iter().filter(|r| r.points.len() >= 3) {
        let place = if room.name.trim().is_empty() {
            room.id.to_string()
        } else {
            format!("{} {}", room.name, room.id)
        };
        let class = room_class(room);
        if count(room.id, PointKind::Lighting) == 0 {
            out.push(Finding {
                severity: Severity::Erro,
                place: place.clone(),
                message: "Sem ponto de luz: a NBR 5410 pede ao menos um ponto de iluminação no teto de cada cômodo, comandado por interruptor.".into(),
                source: "nbr5410",
            });
        }
        let per = perimeter(room) / 100.0;
        let area = room.area() / 10_000.0;
        // Outlets the room asks for: one per so many metres of perimeter, or
        // one. Rooms are metres long, so the count fits comfortably.
        let per_metres = |metres: f64| -> usize {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = (per / metres).ceil().max(1.0) as usize;
            n
        };
        let needed = match class {
            Wet::Kitchen => per_metres(3.5),
            Wet::Living => per_metres(5.0),
            Wet::Other if area > 6.0 => per_metres(5.0),
            Wet::Bathroom | Wet::Balcony | Wet::Other => 1,
        };
        let have = count(room.id, PointKind::Outlet);
        if have < needed {
            let why = match class {
                Wet::Kitchen => format!("um a cada 3,5 m de perímetro ({per:.1} m)"),
                Wet::Bathroom => "um junto ao lavatório".into(),
                Wet::Balcony => "ao menos um".into(),
                _ if area <= 6.0 => "ao menos um".into(),
                _ => format!("um a cada 5 m de perímetro ({per:.1} m)"),
            };
            out.push(Finding {
                severity: Severity::Erro,
                place: place.clone(),
                message: format!("{have} de {needed} tomadas de uso geral: a NBR 5410 pede {why}."),
                source: "nbr5410",
            });
        }
        if long_stay(room)
            && count(room.id, PointKind::Network) == 0
            && count(room.id, PointKind::Wifi) == 0
        {
            out.push(Finding {
                severity: Severity::Alerta,
                place: place.clone(),
                message: "Sem ponto de rede: um cômodo de permanência pede ao menos uma tomada RJ45 (ou cobertura de Wi-Fi) ligada ao quadro de telecom.".into(),
                source: "nbr14565",
            });
        }
        let name = crate::annotations::fold(&room.name);
        if (name.contains("sala")
            || name.contains("quarto")
            || name.contains("dormit")
            || name.contains("suite"))
            && count(room.id, PointKind::Tv) == 0
        {
            out.push(Finding {
                severity: Severity::Dica,
                place,
                message: "Sem ponto de TV: salas e dormitórios costumam ter um ponto coaxial junto ao rack ou à parede da cama.".into(),
                source: "nbr14565",
            });
        }
    }
    let network = all
        .iter()
        .any(|p| matches!(p.kind, PointKind::Network | PointKind::Wifi | PointKind::Tv));
    if network && !all.iter().any(|p| p.kind == PointKind::TelecomPanel) {
        out.push(Finding {
            severity: Severity::Alerta,
            place: "Projeto".into(),
            message: "Há pontos de rede, TV ou Wi-Fi e nenhum quadro de telecom: os cabos precisam de um ponto de distribuição que os reúna.".into(),
            source: "nbr14565",
        });
    }
    if all.iter().any(|p| p.kind.loads()) && !all.iter().any(|p| p.kind == PointKind::Panel) {
        out.push(Finding {
            severity: Severity::Erro,
            place: "Projeto".into(),
            message: "Há cargas e nenhum quadro de distribuição.".into(),
            source: "nbr5410",
        });
    }
    let loose: Vec<String> = all
        .iter()
        .filter(|p| p.kind.loads() && p.circuit.is_none())
        .map(|p| p.id.to_string())
        .collect();
    if !loose.is_empty() {
        out.push(Finding {
            severity: Severity::Alerta,
            place: "Circuitos".into(),
            message: format!("{} pontos sem circuito: {}.", loose.len(), loose.join(", ")),
            source: "nbr5410",
        });
    }
    for circuit in circuits(home) {
        if circuit.kinds.contains(&PointKind::Lighting) && circuit.kinds.len() > 1 {
            out.push(Finding {
                severity: Severity::Erro,
                place: format!("Circuito {}", circuit.name),
                message:
                    "Iluminação e tomadas no mesmo circuito: a NBR 5410 pede circuitos distintos."
                        .into(),
                source: "nbr5410",
            });
        }
        if circuit.kinds.contains(&PointKind::Dedicated) && circuit.points.len() > 1 {
            out.push(Finding {
                severity: Severity::Erro,
                place: format!("Circuito {}", circuit.name),
                message: "Um equipamento de uso específico (chuveiro, ar-condicionado) pede circuito exclusivo.".into(),
                source: "nbr5410",
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // sections and voltages are exact table values

    use super::*;
    use crate::elements::Room;

    fn point(id: u64, catalog: &str, at: (f64, f64), circuit: Option<&str>) -> Furniture {
        let mut f = Furniture {
            id: FurnitureId(id),
            catalog: catalog.into(),
            name: catalog.into(),
            position: Point2::new(at.0, at.1),
            width: 10.0,
            depth: 4.0,
            height: 10.0,
            discipline: Some(Discipline::Electrical),
            ..Furniture::default()
        };
        if let Some(c) = circuit {
            f.properties.insert(CIRCUIT_KEY.into(), c.into());
        }
        f
    }

    fn room(id: u64, name: &str, x: f64, w: f64, d: f64) -> Room {
        Room::new(
            RoomId(id),
            name,
            vec![
                Point2::new(x, 0.0),
                Point2::new(x + w, 0.0),
                Point2::new(x + w, d),
                Point2::new(x, d),
            ],
        )
    }

    #[test]
    fn a_kitchen_and_a_bedroom_are_checked_and_their_circuits_sized() {
        let mut home = Home::default();
        // A 3 × 3 m kitchen (12 m of perimeter: 4 outlets) and a 3 × 4 m bedroom.
        home.rooms = vec![
            room(1, "Cozinha", 0.0, 300.0, 300.0),
            room(2, "Quarto", 300.0, 300.0, 400.0),
        ];
        home.furniture = vec![
            point(10, "electrical-panel", (10.0, 10.0), None),
            point(11, "light-ceiling", (150.0, 150.0), Some("C1")),
            point(12, "outlet-mid", (50.0, 10.0), Some("C2")),
            point(13, "outlet-mid", (150.0, 10.0), Some("C2")),
            point(14, "light-ceiling", (450.0, 200.0), Some("C1")),
            point(15, "outlet-low", (310.0, 100.0), Some("C3")),
            point(16, "shower-point", (100.0, 290.0), Some("C4")),
            point(17, "network-outlet", (590.0, 200.0), None),
        ];
        let findings = check(&home);
        let said = |text: &str| findings.iter().any(|f| f.message.contains(text));
        assert!(said("2 de 4 tomadas"), "kitchen: {findings:#?}");
        assert!(said("1 de 3 tomadas"), "bedroom, 14 m: {findings:#?}");
        assert!(said("nenhum quadro de telecom"), "{findings:#?}");
        assert!(!said("Sem ponto de luz"), "{findings:#?}");
        assert!(
            !said("Sem ponto de rede"),
            "the bedroom has one: {findings:#?}"
        );

        let schedule = circuits(&home);
        let c2 = schedule.iter().find(|c| c.name == "C2").unwrap();
        // Two kitchen outlets at 600 VA each, 127 V: 9.4 A, 2.5 mm², 10 A, with DR.
        assert!(
            (c2.va - 1200.0).abs() < 1e-9 && c2.wire_mm2 == 2.5 && c2.breaker_a == 10 && c2.rcd,
            "{c2:?}"
        );
        let c1 = schedule.iter().find(|c| c.name == "C1").unwrap();
        assert!(
            c1.wire_mm2 == 1.5 && c1.kinds == [PointKind::Lighting],
            "{c1:?}"
        );
        let shower = schedule.iter().find(|c| c.name == "C4").unwrap();
        // 5500 W at 220 V: 25 A, 4 mm², 25 A breaker.
        assert!(
            shower.volts == 220.0 && shower.wire_mm2 == 4.0 && shower.breaker_a == 25,
            "{shower:?}"
        );

        // Lighting and an outlet on one circuit is said.
        home.furniture[5]
            .properties
            .insert(CIRCUIT_KEY.into(), "C1".into());
        assert!(
            check(&home)
                .iter()
                .any(|f| f.message.contains("Iluminação e tomadas no mesmo circuito"))
        );
    }
}
