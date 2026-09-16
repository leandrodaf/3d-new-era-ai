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
/// Where a point keeps its voltage, V, when it is not the supply's: a 220 V
/// outlet in a 127 V flat.
pub const VOLTS_KEY: &str = "elec:volts";
/// Where an automation point keeps its standby consumption, W.
pub const STANDBY_KEY: &str = "elec:standby_w";
/// Where a dimmer keeps the most lighting it can take, W.
pub const MAX_W_KEY: &str = "elec:max_w";

/// Standby a device of the catalog draws when none is written, W: what
/// Wi-Fi and Zigbee modules of the kind usually declare.
pub fn standby_default(catalog: &str) -> f64 {
    match catalog {
        "smart-switch" => 0.8,
        "smart-relay" | "dimmer" | "presence-sensor" => 0.5,
        _ => 0.0,
    }
}

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
    /// A relay, smart switch, dimmer, presence sensor or electronic lock:
    /// draws only its standby.
    Automation,
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
            Self::Automation => "Automação",
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
        "smart-relay" | "smart-switch" | "dimmer" | "presence-sensor" | "smart-lock" => {
            PointKind::Automation
        }
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

/// A room's class by what it holds first, then by its name.
///
/// A toilet, a shower, a basin or a bath make a bathroom whatever it is
/// called: "Banho suíte" has "suíte" in its name and is no bedroom.
fn class_in(home: &Home, room: &Room) -> Wet {
    let fixtures = home.furniture.iter().flat_map(Furniture::flatten).any(|f| {
        let name = crate::annotations::fold(&f.name);
        let bathroom_piece = matches!(
            f.catalog.as_str(),
            "toilet" | "shower" | "shower-glass" | "bathtub" | "basin-cabinet"
        ) || ["vaso", "box ", "lavatorio", "chuveiro", "banheira", "bide"]
            .iter()
            .any(|w| name.contains(w));
        bathroom_piece && room.points.len() >= 3 && inside(&room.points, f.position)
    });
    if fixtures {
        Wet::Bathroom
    } else {
        room_class(room)
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

pub(crate) fn inside(points: &[Point2], p: Point2) -> bool {
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

/// Where a cable run keeps what it carries.
pub const CABLE_KEY: &str = "elec:cable";

/// What a cable run carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cable {
    /// Power conduit: phase, neutral and earth.
    Power,
    /// Network cable (Cat 6), from a network point to the telecom panel.
    Data,
    /// Coaxial TV cable.
    Tv,
}

impl Cable {
    pub const ALL: [Self; 3] = [Self::Power, Self::Data, Self::Tv];

    pub fn key(self) -> &'static str {
        match self {
            Self::Power => "power",
            Self::Data => "data",
            Self::Tv => "tv",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.key() == raw.trim())
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Power => "Eletroduto de força",
            Self::Data => "Cabo de rede (Cat 6)",
            Self::Tv => "Cabo coaxial de TV",
        }
    }

    /// How the run is drawn, so the three read apart on one plan: power
    /// solid, network dashed, TV dash-dot.
    pub fn style(self) -> (crate::style::DashStyle, [u8; 3]) {
        match self {
            Self::Power => (crate::style::DashStyle::Solid, [200, 90, 30]),
            Self::Data => (crate::style::DashStyle::Dash, [40, 120, 200]),
            Self::Tv => (crate::style::DashStyle::DashDot, [120, 70, 170]),
        }
    }
}

/// A cable run: what it carries and how long it is, cm.
pub fn cable_of(line: &crate::style::Polyline) -> Option<Cable> {
    line.properties.get(CABLE_KEY).and_then(|k| Cable::parse(k))
}

/// Where a laid-out run keeps its full length, cm — horizontal and vertical,
/// stubs and drops — on the polylines that draw it, and the name that groups
/// them.
pub const RUN_KEY: &str = "elec:run";
pub const RUN_CM_KEY: &str = "elec:run_cm";

/// Length of every run by what it carries, m — the list that goes to the
/// purchase. A laid-out run counts its real length, drops to every box
/// included; a line drawn by hand counts its plan length with a tenth added
/// for the drops.
pub fn cable_lengths(home: &Home) -> Vec<(Cable, f64)> {
    let view = home.level_view(home.current_level());
    Cable::ALL
        .into_iter()
        .map(|cable| {
            let mut routed: std::collections::BTreeMap<String, f64> =
                std::collections::BTreeMap::new();
            let mut drawn = 0.0;
            for line in view.polylines.iter().filter(|l| cable_of(l) == Some(cable)) {
                match (
                    line.properties.get(RUN_KEY),
                    line.properties
                        .get(RUN_CM_KEY)
                        .and_then(|v| v.parse::<f64>().ok()),
                ) {
                    (Some(run), Some(cm)) => {
                        routed.insert(run.clone(), cm);
                    }
                    _ => {
                        drawn += line
                            .points
                            .windows(2)
                            .map(|w| w[0].distance(w[1]))
                            .sum::<f64>()
                            * 1.1;
                    }
                }
            }
            let cm = drawn + routed.values().sum::<f64>();
            (cable, (cm / 100.0 * 10.0).round() / 10.0)
        })
        .filter(|(_, m)| *m > 0.0)
        .collect()
}

/// Where a routed data run keeps its cable category.
pub const CATEGORY_KEY: &str = "elec:cat";

/// Cable for data points: category of twisted pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    Cat5e,
    Cat6,
    Cat6a,
}

impl Category {
    pub fn parse(raw: &str) -> Option<Self> {
        match crate::annotations::fold(raw)
            .replace([' ', '-'], "")
            .as_str()
        {
            "cat5e" | "cat5" => Some(Self::Cat5e),
            "cat6" => Some(Self::Cat6),
            "cat6a" => Some(Self::Cat6a),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Cat5e => "Cabo de rede U/UTP Cat 5e (1 Gbps; 2,5 Gbps até 100 m)",
            Self::Cat6 => "Cabo de rede U/UTP Cat 6 (1 Gbps; 10 Gbps até 55 m)",
            Self::Cat6a => "Cabo de rede F/UTP Cat 6A (10 Gbps até 100 m)",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Cat5e => "cat5e",
            Self::Cat6 => "cat6",
            Self::Cat6a => "cat6a",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Cat5e => "Cat 5e",
            Self::Cat6 => "Cat 6",
            Self::Cat6a => "Cat 6A (Cat 6 leva 10 GbE só até 55 m)",
        }
    }

    /// Higher carries more.
    pub fn rank(self) -> u8 {
        match self {
            Self::Cat5e => 0,
            Self::Cat6 => 1,
            Self::Cat6a => 2,
        }
    }
}

/// One line of a bill of materials.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Material {
    pub item: String,
    pub quantity: f64,
    pub unit: &'static str,
}

pub(crate) fn metres(cm: f64) -> f64 {
    (cm / 100.0 * 10.0).round() / 10.0
}

/// What a laid-out run takes to build.
///
/// Power: flexible corrugated conduit (25 mm, rolls of 50 m), three
/// conductors (phase, neutral, earth) of the circuit's section with 30 cm
/// left in every box, a box per point — octagonal in the ceiling, 4×2 in
/// the walls — and a bend at each turn. Data: its own conduit, never shared
/// with power, the cable with 3 m left at the rack and 30 cm at each point,
/// an RJ45 keystone per point and as many patch panel ports — one whole
/// cable from the rack to each point, never spliced along the trunk. TV: coaxial
/// RG6 with F connectors at both ends of each point's cable.
#[allow(clippy::cast_precision_loss)] // counts of boxes and bends, far below 2^52
pub fn materials(
    route: &crate::routing::Route,
    cable: Cable,
    section_mm2: f64,
    category: Category,
    kinds: &[PointKind],
) -> Vec<Material> {
    let points = route.terminals.len().saturating_sub(1);
    let length = route.length();
    let conduit = metres(length * 1.05);
    // Network and TV go in star: a whole cable from the panel to each point.
    let star: f64 = route.reach.iter().sum();
    let mut out = vec![Material {
        item: "Eletroduto corrugado flexível 25 mm (3/4\")".into(),
        quantity: conduit,
        unit: "m",
    }];
    out.push(Material {
        item: "Rolos de eletroduto (50 m)".into(),
        quantity: (conduit / 50.0).ceil(),
        unit: "un",
    });
    out.push(Material {
        item: "Curva 90° para eletroduto 25 mm".into(),
        quantity: route.bends as f64,
        unit: "un",
    });
    match cable {
        Cable::Power => {
            let per_conductor = length + 30.0 * route.terminals.len() as f64;
            for colour in ["fase", "neutro", "terra"] {
                out.push(Material {
                    item: format!(
                        "Cabo flexível {} mm² 750 V ({colour})",
                        format!("{section_mm2}").replace('.', ",")
                    ),
                    quantity: metres(per_conductor),
                    unit: "m",
                });
            }
            let ceiling = kinds.iter().filter(|k| **k == PointKind::Lighting).count();
            out.push(Material {
                item: "Caixa octogonal 4×4 de teto".into(),
                quantity: ceiling as f64,
                unit: "un",
            });
            out.push(Material {
                item: "Caixa 4×2 de embutir".into(),
                quantity: (points - ceiling.min(points)) as f64,
                unit: "un",
            });
        }
        Cable::Data => {
            out.push(Material {
                item: category.name().into(),
                quantity: metres(star + 300.0 + 30.0 * points as f64),
                unit: "m",
            });
            out.push(Material {
                item: "Conector fêmea RJ45 (keystone)".into(),
                quantity: points as f64,
                unit: "un",
            });
            out.push(Material {
                item: "Portas de patch panel".into(),
                quantity: points as f64,
                unit: "un",
            });
            out.push(Material {
                item: "Caixa 4×2 de embutir".into(),
                quantity: points as f64,
                unit: "un",
            });
        }
        Cable::Tv => {
            out.push(Material {
                item: "Cabo coaxial RG6 (75 Ω)".into(),
                quantity: metres(star + 100.0 + 30.0 * points as f64),
                unit: "m",
            });
            out.push(Material {
                item: "Conector F de compressão".into(),
                quantity: (2 * points) as f64,
                unit: "un",
            });
            out.push(Material {
                item: "Caixa 4×2 de embutir".into(),
                quantity: points as f64,
                unit: "un",
            });
        }
    }
    out
}

/// Whether a run touches a point: an end or a vertex within reach of it.
pub(crate) fn reaches(line: &crate::style::Polyline, at: Point2) -> bool {
    const REACH: f64 = 30.0;
    line.points.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = (dx * dx + dy * dy).max(1e-9);
        let t = (((at.x - a.x) * dx + (at.y - a.y) * dy) / len2).clamp(0.0, 1.0);
        Point2::new(a.x + t * dx, a.y + t * dy).distance(at) <= REACH
    })
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
                    room.is_some_and(|r| matches!(class_in(home, r), Wet::Kitchen | Wet::Bathroom));
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
            PointKind::Automation => piece
                .properties
                .get(STANDBY_KEY)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or_else(|| standby_default(&piece.catalog)),
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
            // A point written for 220 V sets its circuit; otherwise a dedicated
            // load over 4.4 kVA (a shower) runs on 220 V.
            let written = mine.iter().find_map(|p| {
                view.find_piece(p.id)
                    .and_then(|f| f.properties.get(VOLTS_KEY))
                    .and_then(|v| v.parse::<f64>().ok())
            });
            let volts = if let Some(v) = written {
                v
            } else if supply < 200.0 && kinds == [PointKind::Dedicated] && va > 4400.0 {
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
            // DR 30 mA: every circuit reaching a room with a shower or a
            // bath, and the outlet circuits of kitchens, laundries and
            // balconies.
            let rcd = mine.iter().any(|p| {
                let class = p
                    .room
                    .and_then(|id| view.rooms.iter().find(|r| r.id == id))
                    .map(|r| class_in(&view, r));
                match class {
                    Some(Wet::Bathroom) => p.kind.loads(),
                    Some(Wet::Kitchen | Wet::Balcony) => {
                        matches!(p.kind, PointKind::Outlet | PointKind::Dedicated)
                    }
                    _ => false,
                }
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

/// A number with one decimal, written the Brazilian way: `13,4`.
pub fn decimal(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
}

/// Acceptances of electrical findings no current finding answers to.
pub fn orphaned(home: &Home) -> Vec<(String, String)> {
    let live: std::collections::BTreeSet<String> = check(home).into_iter().map(|f| f.key).collect();
    home.accepted
        .iter()
        .filter(|(key, _)| key.starts_with("elec:") && !live.contains(*key))
        .map(|(key, why)| (key.clone(), why.clone()))
        .collect()
}

/// What each automation device asks of the installation.
#[allow(clippy::too_many_lines)]
fn automation(home: &Home, all: &[Point], out: &mut Vec<Finding>) {
    let view = home.level_view(home.current_level());
    for point in all.iter().filter(|p| p.kind == PointKind::Automation) {
        let Some(piece) = view.find_piece(point.id) else {
            continue;
        };
        let place = format!("{} {}", piece.name, piece.id);
        let room = point
            .room
            .and_then(|id| view.rooms.iter().find(|r| r.id == id));
        match piece.catalog.as_str() {
            "smart-relay" | "smart-switch" => out.push(Finding {
                key: format!("elec:neutral:{}", piece.id),
                accepted: None,
                severity: Severity::Dica,
                place,
                message: "Precisa de neutro na caixa: numa reforma confira, porque a instalação antiga costuma levar só fase e retorno ao interruptor; no projeto, leve o neutro até ela. Aceite quando o neutro estiver garantido.".into(),
                source: "nbr5410",
            }),
            "dimmer" => {
                let max = piece
                    .properties
                    .get(MAX_W_KEY)
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(200.0);
                let load: f64 = room.map_or(0.0, |r| {
                    view.furniture
                        .iter()
                        .flat_map(Furniture::flatten)
                        .filter(|f| inside(&r.points, f.position))
                        .filter_map(|f| f.light.as_ref())
                        .map(crate::Light::electrical_watts)
                        .sum()
                });
                let load = (load * 10.0).round() / 10.0;
                if load > max {
                    out.push(Finding {
                        key: format!("elec:dimmer-max:{}", piece.id),
                        accepted: None,
                        severity: Severity::Alerta,
                        place,
                        message: format!(
                            "{} W de iluminação no cômodo e o dimmer aguenta {} W: divida as luminárias em dois dimmers ou use um de maior capacidade.",
                            decimal(load),
                            decimal(max)
                        ),
                        source: "nbr5410",
                    });
                } else if load < 10.0 {
                    out.push(Finding {
                        key: format!("elec:dimmer-min:{}", piece.id),
                        accepted: None,
                        severity: Severity::Dica,
                        place,
                        message: format!(
                            "{} W de iluminação para dimerizar: abaixo da carga mínima (em geral 10 W) o LED pisca ou não apaga de todo; confira se as lâmpadas são dimerizáveis.",
                            decimal(load)
                        ),
                        source: "nbr5410",
                    });
                }
            }
            "presence-sensor" => {
                let height = piece.elevation + piece.height / 2.0;
                if height < 220.0 {
                    out.push(Finding {
                        key: format!("elec:sensor-height:{}", piece.id),
                        accepted: None,
                        severity: Severity::Dica,
                        place: place.clone(),
                        message: format!(
                            "Sensor de teto a {} cm: abaixo de 2,2 m o alcance e o ângulo não fecham; na parede, use o sensor de parede a 1,1 m.",
                            decimal(height)
                        ),
                        source: "nbr5410",
                    });
                }
                // A 360° ceiling sensor sees about 1.2 × its height around it.
                let reach = height * 1.2;
                if let Some(r) = room {
                    let far = r
                        .points
                        .iter()
                        .map(|c| c.distance(piece.position))
                        .fold(0.0, f64::max);
                    if far > reach {
                        out.push(Finding {
                            key: format!("elec:sensor-reach:{}", piece.id),
                            accepted: None,
                            severity: Severity::Dica,
                            place,
                            message: format!(
                                "Não vê o cômodo inteiro: {} m até o canto mais longe e alcance de uns {} m; centralize-o ou ponha um segundo sensor.",
                                decimal(far / 100.0),
                                decimal(reach / 100.0)
                            ),
                            source: "nbr5410",
                        });
                    }
                }
            }
            "smart-lock" => {
                let on_door = view.furniture.iter().flat_map(Furniture::flatten).any(|f| {
                    f.opening
                        .as_ref()
                        .is_some_and(|o| o.kind == crate::furniture::OpeningKind::Door)
                        && f.position.distance(piece.position) <= f.width / 2.0 + 30.0
                });
                if !on_door {
                    out.push(Finding {
                        key: format!("elec:lock-door:{}", piece.id),
                        accepted: None,
                        severity: Severity::Alerta,
                        place,
                        message: "Fechadura eletrônica fora de uma porta: ponha-a na folha que ela tranca.".into(),
                        source: "nbr5410",
                    });
                }
            }
            _ => {}
        }
    }
}

/// The supply a panel asks the utility for.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Supply {
    /// 1 (monofásico), 2 (bifásico) or 3 (trifásico).
    pub phases: u8,
    /// The main breaker, A, per phase.
    pub breaker_a: u32,
    /// The installed load's current on each phase, balanced, A.
    pub amps_per_phase: f64,
    /// The installed load, VA.
    pub va: f64,
}

/// The supply and main breaker suggested for the installed load, with no
/// demand factor — the utility's rules may allow less, never ask for less
/// protection. One phase up to 8 kVA with no 220 V circuit in a 127 V
/// supply (a 220 V circuit there is between two phases); two up to 20 kVA;
/// three above. Each utility sets its own limits: confirm with it.
pub fn main_breaker(home: &Home) -> Option<Supply> {
    let circuits = circuits(home);
    if circuits.is_empty() {
        return None;
    }
    let supply = home
        .properties
        .get(VOLTAGE_KEY)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(127.0);
    let va: f64 = circuits.iter().map(|c| c.va).sum();
    let between_phases = supply < 200.0 && circuits.iter().any(|c| c.volts > supply + 1.0);
    let phases: u8 = if va <= 8000.0 && !between_phases {
        1
    } else if va <= 20_000.0 {
        2
    } else {
        3
    };
    let amps = va / (f64::from(phases) * supply);
    let breaker = BREAKERS
        .iter()
        .copied()
        .find(|b| f64::from(*b) >= amps)
        .unwrap_or(BREAKERS[BREAKERS.len() - 1]);
    Some(Supply {
        phases,
        breaker_a: breaker,
        amps_per_phase: (amps * 10.0).round() / 10.0,
        va,
    })
}

/// Where a distribution panel keeps how many DIN modules it holds.
pub const MODULES_KEY: &str = "elec:modules";
/// Where the project keeps the presumed short-circuit current at the
/// delivery point, kA, as the utility informs it.
pub const SHORT_KA_KEY: &str = "elec:short_ka";
/// Where the project keeps its earthing scheme: `TN-S`, `TN-C-S` or `TT`.
pub const EARTHING_KEY: &str = "elec:earthing";

/// What goes into the distribution panel, and whether it fits.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Panel {
    /// Devices and the DIN modules each line takes: `[device, count, modules]`.
    pub devices: Vec<(String, u32, u32)>,
    /// Modules the devices take, spare ways included.
    pub used: u32,
    /// Modules the panel holds: written on it, or a guess from its size.
    pub capacity: u32,
    /// Whether the capacity was written or guessed.
    pub capacity_written: bool,
    /// Spare ways NBR 5410 asks for the number of circuits.
    pub spare: u32,
    /// The surge protector: class and poles.
    pub dps: String,
    pub earthing: String,
    /// Breaking capacity the breakers need, kA.
    pub icn_ka: f64,
    /// Whether the short-circuit level was informed or assumed.
    pub icn_written: bool,
    /// Largest partial breaker and the main one, A.
    pub largest_partial_a: u32,
    pub main_a: u32,
}

/// Spare ways a panel keeps for the circuits it has, as NBR 5410 sets them:
/// two up to six circuits, three up to twelve, four up to thirty, and 15 %
/// above.
pub fn spare_ways(circuits: usize) -> u32 {
    match circuits {
        0 => 0,
        1..=6 => 2,
        7..=12 => 3,
        13..=30 => 4,
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        n => (n as f64 * 0.15).ceil() as u32,
    }
}

/// The distribution panel's fill and protection for the load schedule: a
/// one-pole breaker per 127 V circuit and two poles for 220 V between
/// phases, a two-pole DR per circuit that needs one, the main breaker with a
/// pole per phase, the surge protector (one module per phase and one for
/// neutral) and the spare ways. A panel's capacity is its written
/// `elec:modules`, or a guess from its size (a 40 × 60 cm box, 24).
pub fn panel(home: &Home) -> Option<Panel> {
    let circuits = circuits(home);
    let supply = main_breaker(home)?;
    let voltage = home
        .properties
        .get(VOLTAGE_KEY)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(127.0);
    let mut devices: Vec<(String, u32, u32)> = Vec::new();
    let mut add = |name: String, count: u32, poles: u32| {
        if count > 0 {
            devices.push((name, count, count * poles));
        }
    };
    let count = |f: &dyn Fn(&Circuit) -> bool| {
        u32::try_from(circuits.iter().filter(|c| f(c)).count()).unwrap_or(u32::MAX)
    };
    let between_phases = |c: &Circuit| voltage < 200.0 && c.volts > voltage + 1.0;
    add(
        "Disjuntor monopolar".into(),
        count(&|c| !between_phases(c)),
        1,
    );
    add(
        "Disjuntor bipolar (220 V entre fases)".into(),
        count(&between_phases),
        2,
    );
    add("DR bipolar 30 mA".into(), count(&|c| c.rcd), 2);
    add(
        format!("Disjuntor geral {}P {} A", supply.phases, supply.breaker_a),
        1,
        u32::from(supply.phases),
    );
    add(
        format!("DPS classe II, {} fase(s) + neutro", supply.phases),
        1,
        u32::from(supply.phases) + 1,
    );
    let spare = spare_ways(circuits.len());
    add("Espaço reserva".into(), spare, 1);
    let used = devices.iter().map(|d| d.2).sum();
    let view = home.level_view(home.current_level());
    let box_ = view
        .furniture
        .iter()
        .flat_map(Furniture::flatten)
        .find(|f| point_kind(f) == PointKind::Panel);
    let written = box_
        .and_then(|f| f.properties.get(MODULES_KEY))
        .and_then(|v| v.parse::<u32>().ok());
    // A DIN module is 1.8 cm; the box loses some 12 cm of width to its
    // sides and wiring, and takes a row per 30 cm of height — a 40 × 60 cm
    // box holds 30, where the market sells 24 to 32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let guessed = box_.map_or(24, |f| {
        let per_row = ((f.width - 12.0) / 1.8).floor().max(4.0) as u32;
        let rows = (f.height / 30.0).floor().max(1.0) as u32;
        per_row.min(18) * rows
    });
    let short = home
        .properties
        .get(SHORT_KA_KEY)
        .and_then(|v| v.replace(',', ".").parse::<f64>().ok());
    let icn = [3.0, 4.5, 6.0, 10.0]
        .into_iter()
        .find(|k| *k >= short.unwrap_or(6.0))
        .unwrap_or(10.0);
    Some(Panel {
        devices,
        used,
        capacity: written.unwrap_or(guessed),
        capacity_written: written.is_some(),
        spare,
        dps: format!(
            "Classe II (In ≥ 5 kA), {} fase(s) + neutro, junto ao geral",
            supply.phases
        ),
        earthing: home
            .properties
            .get(EARTHING_KEY)
            .cloned()
            .unwrap_or_else(|| "TN-S".into()),
        icn_ka: icn,
        icn_written: short.is_some(),
        largest_partial_a: circuits.iter().map(|c| c.breaker_a).max().unwrap_or(0),
        main_a: supply.breaker_a,
    })
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
    /// The name it is accepted by: `elec:` + the rule + where, stable while
    /// the numbers in the message move.
    pub key: String,
    /// The reason it was accepted with, when someone looked and decided.
    pub accepted: Option<String>,
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
    // No point at all is the worst case, not a neutral one: every room is
    // held to the norm whether its project was started or not.
    for room in view.rooms.iter().filter(|r| r.points.len() >= 3) {
        let place = if room.name.trim().is_empty() {
            room.id.to_string()
        } else {
            format!("{} {}", room.name, room.id)
        };
        let class = class_in(home, room);
        if count(room.id, PointKind::Lighting) == 0 {
            out.push(Finding {
                key: format!("elec:light:{}", room.id),
                accepted: None,
                severity: Severity::Erro,
                place: place.clone(),
                message: "Sem ponto de luz: a norma pede ao menos um ponto de iluminação no teto de cada cômodo, comandado por interruptor.".into(),
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
                Wet::Kitchen => format!("um a cada 3,5 m de perímetro ({} m)", decimal(per)),
                Wet::Bathroom => "um junto ao lavatório".into(),
                Wet::Balcony => "ao menos um".into(),
                _ if area <= 6.0 => "ao menos um".into(),
                _ => format!("um a cada 5 m de perímetro ({} m)", decimal(per)),
            };
            out.push(Finding {
                key: format!("elec:outlets:{}", room.id),
                accepted: None,
                severity: Severity::Erro,
                place: place.clone(),
                message: format!("{have} de {needed} tomadas de uso geral: a norma pede {why}."),
                source: "nbr5410",
            });
        }
        if long_stay(room)
            && class != Wet::Bathroom
            && count(room.id, PointKind::Network) == 0
            && count(room.id, PointKind::Wifi) == 0
        {
            out.push(Finding {
                key: format!("elec:network:{}", room.id),
                accepted: None,
                severity: Severity::Alerta,
                place: place.clone(),
                message: "Sem ponto de rede: um cômodo de permanência pede ao menos uma tomada RJ45 (ou cobertura de Wi-Fi) ligada ao quadro de telecom.".into(),
                source: "nbr14565",
            });
        }
        let name = crate::annotations::fold(&room.name);
        if class != Wet::Bathroom
            && (name.contains("sala")
                || name.contains("quarto")
                || name.contains("dormit")
                || name.contains("suite"))
            && count(room.id, PointKind::Tv) == 0
        {
            out.push(Finding {
                key: format!("elec:tv:{}", room.id),
                accepted: None,
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
            key: "elec:telecom-panel".into(),
            accepted: None,
            severity: Severity::Alerta,
            place: "Projeto".into(),
            message: "Há pontos de rede, TV ou Wi-Fi e nenhum quadro de telecom: os cabos precisam de um ponto de distribuição que os reúna.".into(),
            source: "nbr14565",
        });
    }
    if all.iter().any(|p| p.kind.loads()) && !all.iter().any(|p| p.kind == PointKind::Panel) {
        out.push(Finding {
            key: "elec:panel".into(),
            accepted: None,
            severity: Severity::Erro,
            place: "Projeto".into(),
            message: "Há cargas e nenhum quadro de distribuição.".into(),
            source: "nbr5410",
        });
    }
    // Where cables are drawn, each network and TV point needs one reaching
    // it, and the telecom panel one reaching it too.
    let view_lines = &view.polylines;
    // An access point is a network point too: it needs its cable.
    for (kinds, cable) in [
        (&[PointKind::Network, PointKind::Wifi][..], Cable::Data),
        (&[PointKind::Tv][..], Cable::Tv),
    ] {
        let runs: Vec<&crate::style::Polyline> = view_lines
            .iter()
            .filter(|l| cable_of(l) == Some(cable))
            .collect();
        if runs.is_empty() {
            continue;
        }
        let unreached: Vec<String> = all
            .iter()
            .filter(|p| kinds.contains(&p.kind))
            .filter(|p| {
                home.find_piece(p.id)
                    .is_some_and(|f| !runs.iter().any(|l| reaches(l, f.position)))
            })
            .map(|p| p.id.to_string())
            .collect();
        if !unreached.is_empty() {
            out.push(Finding {
                key: format!("elec:unreached:{}", cable.key()),
                accepted: None,
                severity: Severity::Alerta,
                place: cable.name().into(),
                message: format!("Pontos sem cabo chegando: {}.", unreached.join(", ")),
                source: "nbr14565",
            });
        }
        let panel_reached = all
            .iter()
            .filter(|p| p.kind == PointKind::TelecomPanel)
            .any(|p| {
                home.find_piece(p.id)
                    .is_some_and(|f| runs.iter().any(|l| reaches(l, f.position)))
            });
        if !panel_reached {
            out.push(Finding {
                key: format!("elec:panel-unreached:{}", cable.key()),
                accepted: None,
                severity: Severity::Alerta,
                place: cable.name().into(),
                message: "Nenhum cabo chega ao quadro de telecom: os pontos precisam ser levados até ele.".into(),
                source: "nbr14565",
            });
        }
    }
    // An access point is fed by its data cable (PoE) or by an outlet beside
    // it, and its cable must carry the uplink its standard is sold with.
    let outlets: Vec<&Furniture> = all
        .iter()
        .filter(|p| p.kind == PointKind::Outlet)
        .filter_map(|p| view.find_piece(p.id))
        .collect();
    for ap in crate::wifi::access_points(home) {
        let Some(id) = ap.id else { continue };
        let Some(piece) = view.find_piece(id) else {
            continue;
        };
        let place = format!("{} {id}", piece.name);
        let poe = piece
            .properties
            .get(crate::wifi::POE_KEY)
            .map(String::as_str);
        let beside = outlets.iter().any(|o| {
            o.position
                .distance(ap.at)
                .hypot(o.elevation + o.height / 2.0 - ap.z)
                <= 150.0
        });
        if poe != Some("true") && !beside {
            out.push(Finding {
                key: format!("elec:wifi-power:{id}"),
                accepted: None,
                severity: Severity::Alerta,
                place: place.clone(),
                message: "Access point sem alimentação: nenhuma tomada a até 1,5 m. Alimente por PoE (switch ou injetor 802.3af/at no rack, e marque poe) ou ponha uma tomada no forro junto a ele.".into(),
                source: "nbr14565",
            });
        }
        let (uplink, needs) = ap.standard.uplink();
        let carried = view_lines
            .iter()
            .filter(|l| cable_of(l) == Some(Cable::Data) && reaches(l, ap.at))
            .filter_map(|l| {
                l.properties
                    .get(CATEGORY_KEY)
                    .and_then(|c| Category::parse(c))
            })
            .min_by_key(|c| c.rank());
        if let Some(cat) = carried
            && cat.rank() < needs.rank()
        {
            out.push(Finding {
                key: format!("elec:wifi-uplink:{id}"),
                accepted: None,
                severity: Severity::Dica,
                place,
                message: format!(
                    "{} sai com uplink de {uplink}: o cabo que chega é {} e o que sustenta essa velocidade é {}.",
                    ap.standard.name(),
                    cat.short(),
                    needs.short()
                ),
                source: "nbr14565",
            });
        }
    }
    automation(home, &all, &mut out);
    if let Some(panel) = panel(home) {
        let place = "Quadro de distribuição".to_owned();
        if panel.used > panel.capacity {
            out.push(Finding {
                key: "elec:panel-full".into(),
                accepted: None,
                severity: Severity::Erro,
                place: place.clone(),
                message: format!(
                    "Não cabe: {} módulos DIN (com {} de reserva) num quadro de {} módulos{}. Use um quadro maior ou divida em dois, antes de a parede ser fechada.",
                    panel.used,
                    panel.spare,
                    panel.capacity,
                    if panel.capacity_written { "" } else { " (estimado pelo tamanho; informe elec:modules)" }
                ),
                source: "nbr5410",
            });
        }
        if panel.main_a < panel.largest_partial_a * 2 {
            out.push(Finding {
                key: "elec:selectivity".into(),
                accepted: None,
                severity: Severity::Dica,
                place: place.clone(),
                message: format!(
                    "Geral de {} A e parcial de {} A: com menos de o dobro, uma falta no circuito maior pode desarmar o geral junto. Confira a seletividade nas curvas do fabricante (geral curva C, parciais curva B ajudam).",
                    panel.main_a, panel.largest_partial_a
                ),
                source: "nbr5410",
            });
        }
        if !panel.icn_written {
            out.push(Finding {
                key: "elec:short-circuit".into(),
                accepted: None,
                severity: Severity::Dica,
                place,
                message: format!(
                    "Capacidade de interrupção dos disjuntores assumida em {} kA: peça à concessionária a corrente de curto presumida no ponto de entrega e informe elec:short_ka.",
                    decimal(panel.icn_ka)
                ),
                source: "nbr5410",
            });
        }
    }
    let loose: Vec<String> = all
        .iter()
        .filter(|p| (p.kind.loads() || p.kind == PointKind::Automation) && p.circuit.is_none())
        .map(|p| p.id.to_string())
        .collect();
    if !loose.is_empty() {
        out.push(Finding {
            key: "elec:no-circuit".into(),
            accepted: None,
            severity: Severity::Alerta,
            place: "Circuitos".into(),
            message: format!("{} pontos sem circuito: {}.", loose.len(), loose.join(", ")),
            source: "nbr5410",
        });
    }
    for circuit in circuits(home) {
        if circuit.kinds.contains(&PointKind::Lighting) && circuit.kinds.len() > 1 {
            out.push(Finding {
                key: format!("elec:mixed:{}", circuit.name),
                accepted: None,
                severity: Severity::Erro,
                place: format!("Circuito {}", circuit.name),
                message:
                    "Iluminação e tomadas no mesmo circuito: a norma pede circuitos distintos."
                        .into(),
                source: "nbr5410",
            });
        }
        if circuit.kinds.contains(&PointKind::Dedicated) && circuit.points.len() > 1 {
            out.push(Finding {
                key: format!("elec:dedicated:{}", circuit.name),
                accepted: None,
                severity: Severity::Erro,
                place: format!("Circuito {}", circuit.name),
                message: "Um equipamento de uso específico (chuveiro, ar-condicionado) pede circuito exclusivo.".into(),
                source: "nbr5410",
            });
        }
    }
    for finding in &mut out {
        finding.accepted = home.accepted.get(&finding.key).cloned();
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
    fn a_bathroom_is_a_bathroom_whatever_its_name_says() {
        let mut home = Home::default();
        home.rooms = vec![
            room(1, "Suíte", 0.0, 300.0, 400.0),
            room(2, "Banho suíte", 300.0, 200.0, 200.0),
        ];
        let mut toilet = Furniture {
            id: FurnitureId(30),
            catalog: "imported".into(),
            name: "Vaso suíte".into(),
            position: Point2::new(350.0, 50.0),
            width: 40.0,
            depth: 60.0,
            height: 40.0,
            ..Furniture::default()
        };
        toilet.properties.clear();
        home.furniture = vec![
            point(10, "electrical-panel", (10.0, 10.0), None),
            point(11, "light-ceiling", (150.0, 200.0), Some("C1")),
            point(12, "light-ceiling", (400.0, 100.0), Some("C1")),
            point(13, "outlet-low", (320.0, 100.0), Some("C2")),
            toilet,
        ];
        let findings = check(&home);
        let about = |place: &str, text: &str| {
            findings
                .iter()
                .any(|f| f.place.starts_with(place) && f.message.contains(text))
        };
        assert!(!about("Banho suíte", "ponto de rede"), "{findings:#?}");
        assert!(!about("Banho suíte", "ponto de TV"), "{findings:#?}");
        assert!(
            about("Suíte r1", "ponto de rede"),
            "the bedroom still asks: {findings:#?}"
        );
        // And its outlet circuit is a wet room's: DR.
        let c2 = circuits(&home)
            .into_iter()
            .find(|c| c.name == "C2")
            .unwrap();
        assert!(c2.rcd && (c2.va - 600.0).abs() < 1e-9, "{c2:?}");
    }

    #[test]
    fn network_cables_are_measured_and_their_points_reached() {
        let mut home = Home::default();
        home.furniture = vec![
            point(1, "telecom-panel", (0.0, 0.0), None),
            point(2, "network-outlet", (400.0, 0.0), None),
            point(3, "network-outlet", (400.0, 300.0), None),
        ];
        let mut run = crate::style::Polyline::new(
            crate::ids::PolylineId(9),
            vec![Point2::new(0.0, 0.0), Point2::new(400.0, 0.0)],
        );
        run.properties.insert(CABLE_KEY.into(), "data".into());
        home.polylines.push(run);
        // 4 m drawn, a tenth more for the drops.
        assert_eq!(cable_lengths(&home), vec![(Cable::Data, 4.4)]);
        let findings = check(&home);
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("sem cabo chegando: f3")),
            "the second point has no cable: {findings:#?}"
        );
        assert!(
            !findings
                .iter()
                .any(|f| f.message.contains("Nenhum cabo chega")),
            "{findings:#?}"
        );
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
        assert!(
            !c1.rcd,
            "a kitchen's lighting needs no DR, its outlets do: {c1:?}"
        );
        assert!(
            findings.iter().any(|f| f.message.contains("(12,0 m)")),
            "{findings:#?}"
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

    #[test]
    fn a_routed_run_lists_what_to_buy() {
        use crate::routing::{Terminal, Via, lay_out};
        let mut home = Home::default();
        let pts = [(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)];
        for k in 0..4 {
            let (a, b) = (pts[k], pts[(k + 1) % 4]);
            home.walls.push(crate::elements::Wall::new(
                crate::ids::WallId(k as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            ));
        }
        let at = |id: u64, x: f64, y: f64, z: f64| Terminal {
            id: Some(FurnitureId(id)),
            at: Point2::new(x, y),
            z,
        };
        let panel = at(1, 0.0, 150.0, 150.0);
        let points = [at(2, 100.0, 0.0, 30.0), at(3, 300.0, 0.0, 30.0)];
        let route = lay_out(&home, panel, &points, Via::Ceiling, 280.0);
        let find = |bill: &[Material], item: &str| -> f64 {
            bill.iter()
                .find(|m| m.item.contains(item))
                .unwrap_or_else(|| panic!("{item} in {bill:#?}"))
                .quantity
        };

        let power = materials(
            &route,
            Cable::Power,
            2.5,
            Category::Cat6,
            &[PointKind::Outlet; 2],
        );
        let conduit = find(&power, "Eletroduto");
        assert!(conduit >= route.length() / 100.0, "{power:#?}");
        // Three conductors, each the run plus what is left in the three boxes.
        let phase = find(&power, "2,5 mm² 750 V (fase)");
        assert!(
            (phase - (route.length() + 90.0) / 100.0).abs() < 0.11,
            "{power:#?}"
        );
        assert_eq!(find(&power, "(terra)"), phase);
        assert_eq!(find(&power, "Caixa 4×2"), 2.0);
        assert_eq!(find(&power, "octogonal"), 0.0);
        assert!(
            (find(&power, "Curva 90°") - f64::from(u32::try_from(route.bends).unwrap())).abs()
                < 1e-9
        );

        let data = materials(
            &route,
            Cable::Data,
            0.0,
            Category::Cat6,
            &[PointKind::Network; 2],
        );
        let star: f64 = route.reach.iter().sum();
        let cable = find(&data, "Cat 6 (");
        assert!(
            cable * 100.0 >= star + 300.0,
            "a whole cable to each point and 3 m at the rack: {data:#?}"
        );
        assert_eq!(find(&data, "RJ45"), 2.0);
        assert!(!data.iter().any(|m| m.item.contains("750 V")), "{data:#?}");

        let tv = materials(&route, Cable::Tv, 0.0, Category::Cat6, &[PointKind::Tv; 2]);
        assert_eq!(find(&tv, "Conector F"), 4.0);
    }

    #[test]
    fn a_220_volt_point_sets_its_circuit_and_the_main_breaker_covers_the_load() {
        let mut home = Home::default();
        home.rooms = vec![room(1, "Cozinha", 0.0, 300.0, 300.0)];
        let mut cooktop = point(12, "outlet-high", (100.0, 10.0), Some("C2"));
        cooktop.properties.insert(VA_KEY.into(), "4000".into());
        cooktop.properties.insert(VOLTS_KEY.into(), "220".into());
        home.furniture = vec![
            point(10, "electrical-panel", (10.0, 10.0), None),
            point(11, "light-ceiling", (150.0, 150.0), Some("C1")),
            cooktop,
        ];
        let all = circuits(&home);
        let c2 = all.iter().find(|c| c.name == "C2").unwrap();
        assert_eq!(c2.volts, 220.0, "{all:#?}");
        let c1 = all.iter().find(|c| c.name == "C1").unwrap();
        assert_eq!(c1.volts, 127.0);
        let supply = main_breaker(&home).unwrap();
        assert_eq!(
            supply.phases, 2,
            "a 220 V circuit in a 127 V flat takes two phases: {supply:?}"
        );
        assert!(
            f64::from(supply.breaker_a) >= supply.amps_per_phase,
            "{supply:?}"
        );
        // A big load spreads over more phases instead of one huge breaker.
        let mut big = home.clone();
        for (k, f) in big.furniture.iter_mut().enumerate() {
            if f.properties.contains_key(CIRCUIT_KEY) {
                f.properties.insert(VA_KEY.into(), (12_000 + k).to_string());
            }
        }
        let three = main_breaker(&big).unwrap();
        assert_eq!(three.phases, 3, "{three:?}");
        assert!(three.breaker_a <= 100, "{three:?}");
        assert!(main_breaker(&Home::default()).is_none());
    }

    #[test]
    fn automation_asks_for_neutral_load_height_and_a_door_and_draws_its_standby() {
        let mut home = Home::default();
        home.rooms = vec![room(1, "Sala", 0.0, 600.0, 500.0)];
        let mut lamp = point(11, "light-ceiling", (300.0, 250.0), Some("C1"));
        lamp.light = Some(crate::Light::led(3000.0, 3000.0, (0.0, 0.0, 0.0)));
        let mut dimmer = point(12, "dimmer", (10.0, 100.0), Some("C1"));
        dimmer.elevation = 104.0;
        let relay = point(13, "smart-relay", (10.0, 120.0), Some("C1"));
        let mut sensor = point(14, "presence-sensor", (100.0, 100.0), Some("C1"));
        sensor.elevation = 180.0;
        let lock = point(15, "smart-lock", (590.0, 400.0), None);
        home.furniture = vec![
            point(10, "electrical-panel", (10.0, 10.0), None),
            lamp,
            dimmer,
            relay,
            sensor,
            lock,
        ];
        let keys: Vec<String> = check(&home).into_iter().map(|f| f.key).collect();
        let has = |k: &str| keys.iter().any(|x| x == k);
        assert!(has("elec:neutral:f13"), "{keys:?}");
        assert!(!has("elec:neutral:f12"), "{keys:?}");
        assert!(has("elec:sensor-height:f14"), "{keys:?}");
        assert!(
            has("elec:sensor-reach:f14"),
            "5 m to the far corner at 1.8 m high: {keys:?}"
        );
        assert!(has("elec:lock-door:f15"), "{keys:?}");
        assert!(
            keys.iter().any(|k| k == "elec:no-circuit"),
            "the lock has no circuit: {keys:?}"
        );
        // 3000 lm of LED is some 30 W: within a 200 W dimmer.
        assert!(!has("elec:dimmer-max:f12"), "{keys:?}");
        home.furniture[2]
            .properties
            .insert(MAX_W_KEY.into(), "20".into());
        assert!(check(&home).iter().any(|f| f.key == "elec:dimmer-max:f12"));

        // Standby rides on the circuit, never as a kind of its own.
        let c1 = circuits(&home)
            .into_iter()
            .find(|c| c.name == "C1")
            .unwrap();
        assert_eq!(c1.kinds, vec![PointKind::Lighting], "{c1:?}");
        assert!(
            (c1.va - 101.5).abs() < 1e-9,
            "100 VA of light + 1.5 W standby: {c1:?}"
        );
    }

    #[test]
    fn the_panel_counts_its_modules_with_spare_ways_and_says_when_it_does_not_fit() {
        assert_eq!(spare_ways(6), 2);
        assert_eq!(spare_ways(10), 3);
        assert_eq!(spare_ways(20), 4);
        assert_eq!(spare_ways(40), 6);
        let mut home = Home::default();
        home.rooms = vec![room(1, "Cozinha", 0.0, 400.0, 400.0)];
        let mut furniture = vec![{
            let mut p = point(10, "electrical-panel", (10.0, 10.0), None);
            p.width = 40.0;
            p.height = 60.0;
            p
        }];
        for k in 0..10_u32 {
            furniture.push(point(
                20 + u64::from(k),
                "outlet-low",
                (50.0 + 30.0 * f64::from(k), 10.0),
                Some(&format!("C{k}")),
            ));
        }
        home.furniture = furniture;
        let panel = panel(&home).unwrap();
        // Ten one-pole breakers, ten two-pole DR (a kitchen), main, DPS, 3 spare.
        assert_eq!(panel.spare, 3);
        let dr = panel
            .devices
            .iter()
            .find(|d| d.0.starts_with("DR"))
            .unwrap();
        assert_eq!((dr.1, dr.2), (10, 20), "{panel:?}");
        assert_eq!(panel.used, 10 + 20 + 1 + 2 + 3, "{panel:?}");
        assert!(!panel.capacity_written);
        let keys: Vec<String> = check(&home).into_iter().map(|f| f.key).collect();
        assert!(
            keys.contains(&"elec:panel-full".to_owned()),
            "36 modules in a 40 × 60 box: {keys:?} {panel:?}"
        );
        assert!(keys.contains(&"elec:short-circuit".to_owned()));

        home.furniture[0]
            .properties
            .insert(MODULES_KEY.into(), "48".into());
        home.properties.insert(SHORT_KA_KEY.into(), "4,5".into());
        let keys: Vec<String> = check(&home).into_iter().map(|f| f.key).collect();
        assert!(!keys.contains(&"elec:panel-full".to_owned()), "{keys:?}");
        assert!(!keys.contains(&"elec:short-circuit".to_owned()));
        assert!((super::panel(&home).unwrap().icn_ka - 4.5).abs() < 1e-9);
    }
}
