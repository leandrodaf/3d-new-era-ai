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
        // Shelly 1 Gen3 under 1.2 W; Shelly Dimmer 2 and Exatron ceiling
        // sensors under 1 W.
        "smart-switch" => 1.2,
        "smart-relay" | "dimmer" | "presence-sensor" => 1.0,
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
    /// Garages: counted like other rooms, and on a DR (NBR 5410 5.1.3.2.2 d).
    Garage,
    /// Yards, gardens, outdoor areas: their outlets on a DR (5.1.3.2.2 b).
    Outdoor,
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
    } else if has(&["garagem"]) {
        Wet::Garage
    } else if has(&["quintal", "jardim", "externa", "piscina", "area de lazer"]) {
        Wet::Outdoor
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

/// The ICT (RJ45) and broadcast (TV) outlets NBR 16264 table 1 recommends
/// for a room: 2 and 1 in bedrooms, living rooms, offices, kitchens and
/// laundries; 3 and 2 in a home theater; 1 and 1 in bathrooms, balconies and
/// the rest. Circulation, closets and storage are not rooms the table counts.
fn telecom_outlets(room: &Room, bathroom: bool) -> Option<(usize, usize)> {
    let name = crate::annotations::fold(&room.name);
    if bathroom {
        return Some((1, 1));
    }
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if has(&[
        "hall",
        "corredor",
        "circulacao",
        "closet",
        "deposito",
        "despensa",
        "shaft",
        "escada",
        "rouparia",
    ]) {
        None
    } else if has(&["home theater", "cinema"]) {
        Some((3, 2))
    } else if has(&[
        "quarto",
        "dormit",
        "suite",
        "sala",
        "estar",
        "jantar",
        "escritorio",
        "home office",
        "gourmet",
        "cozinha",
        "copa",
        "servico",
        "lavanderia",
    ]) && !has(&["banh"])
    {
        Some((2, 1))
    } else {
        Some((1, 1))
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

/// Whether a point is inside a room or within `margin` cm of its outline —
/// a piece set in the room's wall counts as the room's.
pub fn inside_room(points: &[Point2], p: Point2, margin: f64) -> bool {
    if inside(points, p) {
        return true;
    }
    let n = points.len();
    (0..n).any(|i| {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = (dx * dx + dy * dy).max(1e-9);
        let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
        Point2::new(a.x + t * dx, a.y + t * dy).distance(p) <= margin
    })
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

/// Lines drawn by hand that a laid-out run of `cable` replaces: electrical
/// lines of no run that carry this cable and reach one of `ends`, or say no
/// cable and start and finish at them.
pub fn drawn_runs(home: &Home, cable: Cable, ends: &[Point2]) -> Vec<crate::ids::PolylineId> {
    let near = |p: Point2| ends.iter().any(|e| e.distance(p) <= 30.0);
    home.level_view(home.current_level())
        .polylines
        .iter()
        .filter(|l| l.discipline == Some(Discipline::Electrical))
        .filter(|l| !l.properties.contains_key(RUN_KEY) && l.points.len() >= 2)
        .filter(|l| match cable_of(l) {
            Some(c) => c == cable && ends.iter().any(|e| reaches(l, *e)),
            None => near(l.points[0]) && near(l.points[l.points.len() - 1]),
        })
        .map(|l| l.id)
        .collect()
}

/// Where a routed run keeps the length to its farthest point, cm.
pub const RUN_FAR_KEY: &str = "elec:run_far_cm";
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
            Self::Cat6 => "Cabo de rede U/UTP Cat 6 (5 Gbps até 100 m; 10 Gbps até 37 m)",
            Self::Cat6a => {
                "Cabo de rede U/UTP ou F/UTP Cat 6A (10 Gbps até 100 m; F/UTP com blindagem aterrada no rack)"
            }
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
            Self::Cat6a => "Cat 6A (Cat 6 leva 10 GbE só até 37 m)",
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
    // NBR 5410 9.5.2.1.2: a room's lighting load is 100 VA up to 6 m² and
    // 60 VA more per whole 4 m² beyond, shared by its lighting points.
    let lights_in = |room: RoomId| {
        view.furniture
            .iter()
            .flat_map(Furniture::flatten)
            .filter(|f| f.discipline == Some(Discipline::Electrical) || f.light.is_some())
            .filter(|f| point_kind(f) == PointKind::Lighting && !f.properties.contains_key(VA_KEY))
            .filter(|f| room_of(f.position).is_some_and(|r| r.id == room))
            .count()
    };
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
        // Every outlet counts toward its room's first three, written or not.
        let nth = if kind == PointKind::Outlet {
            room.map_or(0, |r| {
                let count = outlets_in.entry(r.id).or_default();
                *count += 1;
                *count
            })
        } else {
            0
        };
        let va = written.unwrap_or_else(|| match kind {
            PointKind::Lighting => room.map_or(100.0, |r| {
                lighting_load(r.area() / 10_000.0)
                    / f64::from(u32::try_from(lights_in(r.id).max(1)).unwrap_or(1))
            }),
            PointKind::Outlet => {
                let wet =
                    room.is_some_and(|r| matches!(class_in(home, r), Wet::Kitchen | Wet::Bathroom));
                if wet && nth <= 3 { 600.0 } else { 100.0 }
            }
            // The norm takes the equipment's rated power (4.2.1.2.1 a); these
            // stand in until it is written: a shower as the 7500 W ones most
            // sold, an air conditioner as a 12 000 BTU/h one.
            PointKind::Dedicated => match piece.catalog.as_str() {
                "shower-point" => 7500.0,
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

/// How many circuits run in the same conduit as `circuit` somewhere: itself
/// and every other power run with a stretch of 50 cm or more along one of
/// its segments.
fn sharing(view: &Home, circuit: &str) -> u32 {
    let run_of = |l: &crate::style::Polyline| {
        l.properties
            .get(RUN_KEY)
            .and_then(|r| r.strip_prefix("power:"))
            .map(str::to_owned)
    };
    let segments = |name: &str| -> Vec<(Point2, Point2)> {
        view.polylines
            .iter()
            .filter(|l| run_of(l).as_deref() == Some(name))
            .flat_map(|l| {
                l.points
                    .windows(2)
                    .map(|w| (w[0], w[1]))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let mine = segments(circuit);
    if mine.is_empty() {
        return 1;
    }
    let mut others: Vec<String> = view
        .polylines
        .iter()
        .filter_map(run_of)
        .filter(|n| n != circuit)
        .collect();
    others.sort();
    others.dedup();
    let overlap = |(a, b): (Point2, Point2), (c, d): (Point2, Point2)| {
        let len = a.distance(b);
        if len < 1e-6 {
            return 0.0;
        }
        let (ux, uy) = ((b.x - a.x) / len, (b.y - a.y) / len);
        let off = |p: Point2| ((p.x - a.x) * uy - (p.y - a.y) * ux).abs();
        if off(c) > 2.0 || off(d) > 2.0 {
            return 0.0;
        }
        let t = |p: Point2| (p.x - a.x) * ux + (p.y - a.y) * uy;
        let (t0, t1) = (t(c).min(t(d)), t(c).max(t(d)));
        (t1.min(len) - t0.max(0.0)).max(0.0)
    };
    let shared = others
        .iter()
        .filter(|name| {
            let theirs = segments(name);
            mine.iter()
                .any(|m| theirs.iter().any(|t| overlap(*m, *t) >= 50.0))
        })
        .count();
    1 + u32::try_from(shared).unwrap_or(u32::MAX)
}

/// A room's lighting load, VA, NBR 5410 9.5.2.1.2: 100 VA for the first
/// 6 m², 60 VA for each whole 4 m² beyond.
pub fn lighting_load(area_m2: f64) -> f64 {
    if area_m2 <= 6.0 {
        100.0
    } else {
        100.0 + 60.0 * ((area_m2 - 6.0) / 4.0).floor()
    }
}

/// Where the project keeps how many circuits share a conduit, for the
/// grouping factor; unwritten, it is read from the laid-out runs.
pub const GROUPING_KEY: &str = "elec:grouping";

/// NBR 5410 table 42: correction for circuits bundled or in one closed
/// conduit.
pub fn grouping_factor(circuits: u32) -> f64 {
    match circuits {
        0 | 1 => 1.0,
        2 => 0.80,
        3 => 0.70,
        4 => 0.65,
        5 => 0.60,
        6 => 0.57,
        7 => 0.54,
        8 => 0.52,
        9..=11 => 0.50,
        12..=15 => 0.45,
        16..=19 => 0.41,
        _ => 0.38,
    }
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
    /// It feeds outlets of a kitchen, copa, laundry or service area.
    pub kitchen_outlets: bool,
    /// It also feeds outlets or lighting outside those rooms.
    pub beyond_kitchen: bool,
    /// Voltage drop at its farthest point, %, when its run is laid out.
    pub drop_pct: Option<f64>,
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
/// NBR NM 60898 5.3.2 preferred ratings, A.
const BREAKERS: [u32; 13] = [6, 10, 13, 16, 20, 25, 32, 40, 50, 63, 80, 100, 125];

/// The load schedule: every circuit written on a point, sized.
pub fn circuits(home: &Home) -> Vec<Circuit> {
    let supply = home
        .properties
        .get(VOLTAGE_KEY)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(127.0);
    let view = home.level_view(home.current_level());
    let all = points(home);
    let written_grouping = home
        .properties
        .get(GROUPING_KEY)
        .and_then(|v| v.parse::<u32>().ok());
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
            // Table 42: as many circuits as share a stretch of its conduit on
            // the laid-out runs, or what the project writes.
            let factor = grouping_factor(written_grouping.unwrap_or_else(|| sharing(&view, &name)));
            // Table 47: 1.5 mm² for lighting, 2.5 for any circuit with outlets.
            let minimum = if kinds.iter().all(|k| *k == PointKind::Lighting) {
                1.5
            } else {
                2.5
            };
            // The smallest section whose corrected capacity (table 36 × table
            // 42) admits a standard breaker between the current and it:
            // IB ≤ In ≤ Iz (5.3.4.1).
            let (wire, breaker) = SECTIONS
                .iter()
                .copied()
                .filter(|(mm2, _)| *mm2 >= minimum)
                .find_map(|(mm2, carries)| {
                    let iz = carries * factor;
                    BREAKERS
                        .iter()
                        .copied()
                        .find(|b| f64::from(*b) >= amps && f64::from(*b) <= iz)
                        .map(|b| (mm2, b))
                })
                .unwrap_or((SECTIONS[SECTIONS.len() - 1].0, BREAKERS[BREAKERS.len() - 1]));
            let class_of = |p: &Point| {
                p.room
                    .and_then(|id| view.rooms.iter().find(|r| r.id == id))
                    .map(|r| class_in(&view, r))
            };
            // DR 30 mA, NBR 5410 5.1.3.2.2: every point of a room with a bath
            // or shower (a); outlets outdoors and on balconies (b, c); in
            // kitchens, laundries, service areas and garages every point,
            // lighting fixtures at 2.50 m or higher excepted (d, note 3).
            let rcd = mine.iter().any(|p| match class_of(p) {
                Some(Wet::Bathroom) => p.kind.loads(),
                Some(Wet::Outdoor | Wet::Balcony) => {
                    matches!(p.kind, PointKind::Outlet | PointKind::Dedicated)
                }
                Some(Wet::Kitchen | Wet::Garage) => match p.kind {
                    PointKind::Lighting => view
                        .find_piece(p.id)
                        .is_some_and(|f| f.elevation + f.height / 2.0 < 250.0),
                    k => k.loads(),
                },
                _ => false,
            });
            let kitchen_outlets = mine
                .iter()
                .any(|p| p.kind == PointKind::Outlet && class_of(p) == Some(Wet::Kitchen));
            let beyond_kitchen = mine.iter().any(|p| {
                p.kind == PointKind::Lighting
                    || (p.kind.loads() && class_of(p) != Some(Wet::Kitchen))
            });
            // Voltage drop to the farthest point of its laid-out run, copper at
            // 70 °C (ρ ≈ 1/46 Ω·mm²/m), the whole load there: on the safe side.
            let far_cm = view
                .polylines
                .iter()
                .filter(|l| {
                    l.properties.get(RUN_KEY).map(String::as_str) == Some(&format!("power:{name}"))
                })
                .filter_map(|l| {
                    l.properties
                        .get(RUN_FAR_KEY)
                        .or_else(|| l.properties.get(RUN_CM_KEY))
                        .and_then(|v| v.parse::<f64>().ok())
                })
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
            let drop_pct = far_cm.map(|cm| {
                let pct = 200.0 * (cm / 100.0) * amps / (46.0 * wire * volts);
                (pct * 10.0).round() / 10.0
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
                kitchen_outlets,
                beyond_kitchen,
                drop_pct,
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
                source: "fabricantes",
            }),
            "dimmer" => {
                let max = piece
                    .properties
                    .get(MAX_W_KEY)
                    .and_then(|v| v.parse::<f64>().ok())
                    // Shelly Dimmer 2: up to 1.1 A of LED, some 140 W at 127 V.
                    .unwrap_or_else(|| 1.1 * home.properties.get(VOLTAGE_KEY).and_then(|v| v.parse::<f64>().ok()).unwrap_or(127.0));
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
                        source: "fabricantes",
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
                        source: "fabricantes",
                    });
                }
            }
            "presence-sensor" => {
                let height = piece.elevation + piece.height / 2.0;
                if !(220.0..=300.0).contains(&height) {
                    out.push(Finding {
                        key: format!("elec:sensor-height:{}", piece.id),
                        accepted: None,
                        severity: Severity::Dica,
                        place: place.clone(),
                        message: format!(
                            "Sensor de teto a {} cm: os fabricantes o instalam por volta de 2,4 m (até 2,9 m); fora disso o alcance muda; na parede, use o sensor de parede.",
                            decimal(height)
                        ),
                        source: "fabricantes",
                    });
                }
                // A 360° ceiling sensor sees a circle of about 7 m at 2.4 m
                // (Exatron): some 1.45 × its height around it.
                let reach = height * 1.45;
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
                            source: "fabricantes",
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
                        source: "fabricantes",
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

/// Main breaker sizes Enel SP fixes at the meter for 127/220 V overhead
/// supply, A (ET GRI-0017 v02, annex B): single-phase A1–A2, two-phase
/// B3–B9, three-phase C3–C12.
const ENEL_SINGLE: [u32; 2] = [50, 63];
const ENEL_TWO: [u32; 7] = [50, 63, 80, 100, 125, 160, 200];
const ENEL_THREE: [u32; 10] = [50, 63, 80, 100, 125, 160, 200, 225, 275, 300];

/// The supply and main breaker suggested for the installed load, with no
/// demand factor (the demand is the engineer's, and never asks for more).
/// On a 127/220 V supply, Enel SP's categories: single-phase (phase and
/// neutral, 127 V only) up to 12 kW, two-phase up to 20 kW, three-phase up
/// to 75 kW — above, medium voltage. A 220 V circuit needs two phases. The
/// breaker is the smallest of the utility's fixed sizes over the current.
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
    let phases: u8 = if va <= 12_000.0 && !between_phases {
        1
    } else if va <= 20_000.0 {
        2
    } else {
        3
    };
    let amps = va / (f64::from(phases) * supply);
    let sizes: &[u32] = match phases {
        1 => &ENEL_SINGLE,
        2 => &ENEL_TWO,
        _ => &ENEL_THREE,
    };
    let breaker = sizes
        .iter()
        .copied()
        .find(|b| f64::from(*b) >= amps)
        .unwrap_or(sizes[sizes.len() - 1]);
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
    // NM 60898 steps; Enel SP asks 10 kA of breakers up to 63 A at 127/220 V.
    let icn = [1.5, 3.0, 4.5, 6.0, 10.0]
        .into_iter()
        .find(|k| *k >= short.unwrap_or(10.0).max(10.0))
        .unwrap_or(10.0);
    Some(Panel {
        devices,
        used,
        capacity: written.unwrap_or(guessed),
        capacity_written: written.is_some(),
        spare,
        dps: format!(
            "Classe II, In ≥ 5 kA (8/20) por modo e Up ≤ 1,5 kV, {} fase(s) + neutro (N-PE ≥ 10 kA), junto ao geral",
            supply.phases
        ),
        earthing: home
            .properties
            .get(EARTHING_KEY)
            .cloned()
            .unwrap_or_else(|| "TN-C-S".into()),
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
            Wet::Other | Wet::Garage | Wet::Outdoor if area > 6.0 => per_metres(5.0),
            Wet::Bathroom | Wet::Balcony | Wet::Other | Wet::Garage | Wet::Outdoor => 1,
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
        // NBR 16264 table 1: the ICT (RJ45) and broadcast (TV) outlets a room
        // is recommended to have. A Wi-Fi point is not an RJ45 outlet.
        if let Some((ict, bct)) = telecom_outlets(room, class == Wet::Bathroom) {
            let have = count(room.id, PointKind::Network);
            if have < ict {
                out.push(Finding {
                    key: format!("elec:network:{}", room.id),
                    accepted: None,
                    severity: if have == 0 && long_stay(room) && class != Wet::Bathroom {
                        Severity::Alerta
                    } else {
                        Severity::Dica
                    },
                    place: place.clone(),
                    message: format!(
                        "{have} de {ict} pontos de rede RJ45: a norma de cabeamento residencial recomenda {ict} neste cômodo, cada um com cabo de 4 pares até o distribuidor e uma tomada de energia ao lado."
                    ),
                    source: "nbr16264",
                });
            }
            let tv = count(room.id, PointKind::Tv);
            if tv < bct {
                out.push(Finding {
                    key: format!("elec:tv:{}", room.id),
                    accepted: None,
                    severity: Severity::Dica,
                    place: place.clone(),
                    message: format!(
                        "{tv} de {bct} pontos de TV: a norma de cabeamento residencial recomenda {bct} neste cômodo (coaxial até 100 m do distribuidor)."
                    ),
                    source: "nbr16264",
                });
            }
        }
    }
    let network = all
        .iter()
        .any(|p| matches!(p.kind, PointKind::Network | PointKind::Wifi | PointKind::Tv));
    for rack in all.iter().filter(|p| p.kind == PointKind::TelecomPanel) {
        let Some(piece) = view.find_piece(rack.id) else {
            continue;
        };
        let powered = all.iter().filter(|p| p.kind == PointKind::Outlet).any(|o| {
            view.find_piece(o.id)
                .is_some_and(|f| f.position.distance(piece.position) <= 150.0)
        });
        if !powered {
            out.push(Finding {
                key: format!("elec:telecom-power:{}", rack.id),
                accepted: None,
                severity: Severity::Alerta,
                place: format!("{} {}", piece.name, rack.id),
                message: "Distribuidor de telecom sem tomada de energia junto: modem, roteador e switch precisam dela.".into(),
                source: "nbr16264",
            });
        }
    }
    if network && !all.iter().any(|p| p.kind == PointKind::TelecomPanel) {
        out.push(Finding {
            key: "elec:telecom-panel".into(),
            accepted: None,
            severity: Severity::Alerta,
            place: "Projeto".into(),
            message: "Há pontos de rede, TV ou Wi-Fi e nenhum quadro de telecom: os cabos precisam de um ponto de distribuição que os reúna.".into(),
            source: "nbr16264",
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
                source: "nbr16264",
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
                source: "nbr16264",
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
                message: "Access point sem alimentação: nenhuma tomada a até 1,5 m. Alimente por PoE (switch ou injetor PoE+ 802.3at no rack para Wi-Fi 6, 6E e 7 doméstico; 802.3bt para Wi-Fi 7 corporativo; e marque poe) ou ponha uma tomada no forro junto a ele (distância de referência, não de norma).".into(),
                source: "nbr16264",
            });
        }
        let uplink = format!("{} GbE", decimal(ap.uplink_gbps).trim_end_matches(",0"));
        let needs = crate::wifi::cable_for(ap.uplink_gbps);
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
                source: "nbr16264",
            });
        }
    }
    // A point set in glass or in an opening's span has no wall to hold it.
    for point in &all {
        if let Some(why) = view
            .find_piece(point.id)
            .and_then(|f| crate::mounting::blocked(home, f))
        {
            out.push(Finding {
                key: format!("elec:mount:{}", point.id),
                accepted: None,
                severity: Severity::Erro,
                place: format!("{} {}", point.name, point.id),
                message: why,
                source: "nbr5410",
            });
        } else if let Some(why) = view
            .find_piece(point.id)
            .and_then(|f| crate::mounting::hidden(home, f))
        {
            out.push(Finding {
                key: format!("elec:hidden:{}", point.id),
                accepted: None,
                severity: Severity::Alerta,
                place: format!("{} {}", point.name, point.id),
                message: why,
                source: "nbr5410",
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
                    "Geral de {} A e parcial de {} A: com menos de o dobro, uma falta no circuito maior pode desarmar o geral junto (regra prática, não da norma). Confira a seletividade nas tabelas do fabricante; geral curva C e parciais curva B ajudam.",
                    panel.main_a, panel.largest_partial_a
                ),
                source: "nm60898",
            });
        }
        if !panel.icn_written {
            out.push(Finding {
                key: "elec:short-circuit".into(),
                accepted: None,
                severity: Severity::Dica,
                place,
                message: format!(
                    "Capacidade de interrupção dos disjuntores assumida em {} kA, o que a Enel SP pede até 63 A: confirme com a concessionária a corrente de curto presumida no ponto de entrega e informe elec:short_ka.",
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
    let schedule = circuits(home);
    let mixed = |c: &Circuit| c.kinds.contains(&PointKind::Lighting) && c.kinds.len() > 1;
    // 9.5.3.3: in a dwelling lighting and outlets may share a circuit when
    // it carries up to 16 A and neither all lighting nor all outlets are on
    // shared circuits.
    let lights_all_mixed = all
        .iter()
        .filter(|p| p.kind == PointKind::Lighting && p.circuit.is_some())
        .all(|p| {
            schedule
                .iter()
                .any(|c| mixed(c) && c.points.contains(&p.id))
        });
    let outlets_all_mixed = all
        .iter()
        .filter(|p| p.kind == PointKind::Outlet && p.circuit.is_some())
        .all(|p| {
            schedule
                .iter()
                .any(|c| mixed(c) && c.points.contains(&p.id))
        });
    for circuit in &schedule {
        let place = format!("Circuito {}", circuit.name);
        if mixed(circuit) {
            let why = if circuit.amps > 16.0 {
                Some(format!(
                    "carrega {} A, acima dos 16 A",
                    decimal(circuit.amps)
                ))
            } else if lights_all_mixed {
                Some("toda a iluminação ficou em circuitos mistos".into())
            } else if outlets_all_mixed {
                Some("todas as tomadas ficaram em circuitos mistos".into())
            } else {
                None
            };
            if let Some(why) = why {
                out.push(Finding {
                    key: format!("elec:mixed:{}", circuit.name),
                    accepted: None,
                    severity: Severity::Erro,
                    place: place.clone(),
                    message: format!(
                        "Iluminação e tomadas no mesmo circuito: a norma só admite em residência até 16 A e sem que toda a iluminação ou todas as tomadas fiquem em circuitos mistos (9.5.3.3); aqui {why}."
                    ),
                    source: "nbr5410",
                });
            }
        }
        if circuit.kitchen_outlets && circuit.beyond_kitchen {
            out.push(Finding {
                key: format!("elec:kitchen-circuit:{}", circuit.name),
                accepted: None,
                severity: Severity::Erro,
                place: place.clone(),
                message: "Tomadas de cozinha, copa, lavanderia ou área de serviço dividem o circuito com iluminação ou com pontos de outros cômodos: elas pedem circuitos só delas (9.5.3.2).".into(),
                source: "nbr5410",
            });
        }
        // 9.5.3.1: equipment over 10 A takes a circuit of its own.
        let big: Vec<&Point> = all
            .iter()
            .filter(|p| p.kind == PointKind::Dedicated && circuit.points.contains(&p.id))
            .filter(|p| p.va / circuit.volts > 10.0)
            .collect();
        if !big.is_empty() && circuit.points.len() > 1 {
            out.push(Finding {
                key: format!("elec:dedicated:{}", circuit.name),
                accepted: None,
                severity: Severity::Erro,
                place: place.clone(),
                message: format!(
                    "{} passa de 10 A e pede circuito exclusivo (9.5.3.1).",
                    big.iter()
                        .map(|p| format!("{} {}", p.name, p.id))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                source: "nbr5410",
            });
        }
        if let Some(pct) = circuit.drop_pct
            && pct > 4.0
        {
            out.push(Finding {
                key: format!("elec:drop:{}", circuit.name),
                accepted: None,
                severity: Severity::Alerta,
                place,
                message: format!(
                    "Queda de tensão de {} % até o ponto mais longe: o circuito terminal pede no máximo 4 % (6.2.7.2); aumente a seção de {} mm² ou divida o circuito.",
                    decimal(pct),
                    decimal(circuit.wire_mm2)
                ),
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
        // NBR 16264 table 1 recommends 1 RJ45 and 1 TV outlet in a bathroom
        // — a tip, never the bedroom's 2 and never an alert.
        let bath = findings
            .iter()
            .find(|f| f.place.starts_with("Banho suíte") && f.message.contains("pontos de rede"))
            .unwrap_or_else(|| panic!("{findings:#?}"));
        assert!(bath.message.starts_with("0 de 1 "), "{bath:?}");
        assert_eq!(bath.severity, Severity::Dica);
        let bedroom = findings
            .iter()
            .find(|f| f.place.starts_with("Suíte r1") && f.message.contains("pontos de rede"))
            .unwrap_or_else(|| panic!("the bedroom still asks: {findings:#?}"));
        assert!(bedroom.message.starts_with("0 de 2 "), "{bedroom:?}");
        assert_eq!(bedroom.severity, Severity::Alerta);
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
        // 5.1.3.2.2 d): in a kitchen every point is on a DR, lighting too,
        // unless the fixture is at 2.50 m or higher (note 3).
        assert!(c1.rcd, "a kitchen's lighting at the ceiling point: {c1:?}");
        assert!(
            findings.iter().any(|f| f.message.contains("(12,0 m)")),
            "{findings:#?}"
        );
        let shower = schedule.iter().find(|c| c.name == "C4").unwrap();
        // A 7500 W shower at 220 V: 34.1 A, 6 mm² (41 A), 40 A breaker.
        assert!(
            shower.volts == 220.0 && shower.wire_mm2 == 6.0 && shower.breaker_a == 40,
            "{shower:?}"
        );
        // Sharing its conduit with two other circuits (table 42: 0.70), 10 mm²
        // carries 39.9 A, short of the 40 A breaker: 16 mm².
        let mut grouped = home.clone();
        grouped.properties.insert(GROUPING_KEY.into(), "3".into());
        let shower = circuits(&grouped)
            .into_iter()
            .find(|c| c.name == "C4")
            .unwrap();
        assert_eq!(shower.wire_mm2, 16.0, "{shower:?}");

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
        // A 30 m² room: 100 VA + 6 × 60 VA of lighting (9.5.2.1.2), plus
        // three devices of 1 W standby.
        assert!(
            (c1.va - (lighting_load(30.0) + 3.0)).abs() < 1e-9
                && (lighting_load(30.0) - 460.0).abs() < 1e-9,
            "{c1:?}"
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
        // Enel SP asks 10 kA of breakers up to 63 A, whatever less is informed.
        assert!((super::panel(&home).unwrap().icn_ka - 10.0).abs() < 1e-9);
    }

    #[test]
    fn the_telecom_distributor_needs_a_power_outlet_beside_it() {
        let mut home = Home::default();
        home.rooms = vec![room(1, "Sala", 0.0, 400.0, 400.0)];
        home.furniture = vec![
            point(10, "telecom-panel", (10.0, 200.0), None),
            point(11, "network-outlet", (390.0, 200.0), None),
        ];
        let keys = |home: &Home| {
            check(home)
                .into_iter()
                .map(|f| (f.key, f.source))
                .collect::<Vec<_>>()
        };
        let k = keys(&home);
        assert!(
            k.contains(&("elec:telecom-power:f10".to_owned(), "nbr16264")),
            "{k:?}"
        );
        home.furniture
            .push(point(12, "outlet-mid", (10.0, 150.0), Some("C1")));
        assert!(
            !keys(&home)
                .iter()
                .any(|(key, _)| key.starts_with("elec:telecom-power"))
        );
    }

    #[test]
    fn nbr_5410_and_enel_sp_figures_hold() {
        // 9.5.2.1.2
        assert!((lighting_load(6.0) - 100.0).abs() < 1e-9);
        assert!((lighting_load(9.9) - 100.0).abs() < 1e-9);
        assert!((lighting_load(10.0) - 160.0).abs() < 1e-9);
        assert!((lighting_load(14.0) - 220.0).abs() < 1e-9);
        // Table 42
        assert!(
            (grouping_factor(1) - 1.0).abs() < 1e-9 && (grouping_factor(3) - 0.70).abs() < 1e-9
        );

        let mut home = Home::default();
        home.rooms = vec![
            room(1, "Sala", 0.0, 400.0, 400.0),
            room(2, "Cozinha", 400.0, 300.0, 400.0),
        ];
        home.furniture = vec![
            point(10, "electrical-panel", (10.0, 10.0), None),
            point(11, "light-ceiling", (200.0, 200.0), Some("C1")),
            point(12, "outlet-low", (10.0, 200.0), Some("C1")),
            point(13, "light-ceiling", (550.0, 200.0), Some("C2")),
            point(14, "outlet-mid", (690.0, 100.0), Some("C3")),
            point(15, "outlet-low", (390.0, 300.0), Some("C3")),
        ];
        let keys = |home: &Home| check(home).into_iter().map(|f| f.key).collect::<Vec<_>>();
        let k = keys(&home);
        // C1 mixes a room's light and outlet under 16 A while C2 keeps lighting
        // apart: 9.5.3.3 admits it.
        assert!(!k.contains(&"elec:mixed:C1".to_owned()), "{k:?}");
        // C3 puts a kitchen outlet with a living room one: 9.5.3.2 forbids.
        assert!(k.contains(&"elec:kitchen-circuit:C3".to_owned()), "{k:?}");
        // Every light on mixed circuits: then it is said.
        home.furniture[3]
            .properties
            .insert(CIRCUIT_KEY.into(), "C1".into());
        assert!(keys(&home).contains(&"elec:mixed:C1".to_owned()));

        // A 1500 VA point (11.8 A) shares a circuit: said; a 1000 VA one is not.
        let mut shared = Home::default();
        shared.rooms = vec![room(1, "Quarto", 0.0, 400.0, 400.0)];
        let mut ac = point(20, "ac-point", (10.0, 100.0), Some("C1"));
        ac.properties.insert(VA_KEY.into(), "1500".into());
        shared.furniture = vec![
            point(10, "electrical-panel", (10.0, 10.0), None),
            ac,
            point(21, "outlet-low", (10.0, 300.0), Some("C1")),
        ];
        assert!(keys(&shared).contains(&"elec:dedicated:C1".to_owned()));
        shared.furniture[1]
            .properties
            .insert(VA_KEY.into(), "1000".into());
        assert!(!keys(&shared).contains(&"elec:dedicated:C1".to_owned()));

        // Enel SP: 10 kW stays single-phase at a fixed 63 A breaker... and
        // 13 kW goes two-phase.
        let mut load = Home::default();
        load.rooms = vec![room(1, "Quarto", 0.0, 400.0, 400.0)];
        // 127 V loads (a dedicated point this big would go to 220 V and two
        // phases by itself).
        let mut big = point(30, "outlet-low", (10.0, 100.0), Some("C1"));
        big.properties.insert(VA_KEY.into(), "7000".into());
        load.furniture = vec![big];
        let supply = main_breaker(&load).unwrap();
        assert_eq!((supply.phases, supply.breaker_a), (1, 63), "{supply:?}");
        load.furniture[0]
            .properties
            .insert(VA_KEY.into(), "13000".into());
        let supply = main_breaker(&load).unwrap();
        assert_eq!(supply.phases, 2, "{supply:?}");
        assert!(
            [50, 63, 80, 100, 125, 160, 200].contains(&supply.breaker_a),
            "{supply:?}"
        );
    }

    #[test]
    fn a_long_run_drops_too_much_voltage_and_shared_conduits_derate() {
        use crate::style::Polyline;
        let mut home = Home::default();
        home.rooms = vec![room(1, "Quarto", 0.0, 400.0, 400.0)];
        let mut heater = point(40, "outlet-low", (10.0, 100.0), Some("C1"));
        heater.properties.insert(VA_KEY.into(), "1900".into());
        home.furniture = vec![heater, point(41, "outlet-low", (10.0, 300.0), Some("C2"))];
        let run = |id: u64, circuit: &str, far: f64| {
            let mut l = Polyline::new(
                crate::ids::PolylineId(id),
                vec![Point2::new(0.0, 0.0), Point2::new(300.0, 0.0)],
            );
            l.properties
                .insert(RUN_KEY.into(), format!("power:{circuit}"));
            l.properties.insert(RUN_CM_KEY.into(), far.to_string());
            l.properties.insert(RUN_FAR_KEY.into(), far.to_string());
            l
        };
        home.polylines = vec![run(1, "C1", 3500.0), run(2, "C2", 300.0)];
        let c1 = circuits(&home)
            .into_iter()
            .find(|c| c.name == "C1")
            .unwrap();
        // 15 A over 35 m of 2.5 mm² at 127 V: some 9 %.
        assert!(c1.drop_pct.unwrap() > 4.0, "{c1:?}");
        assert!(check(&home).iter().any(|f| f.key == "elec:drop:C1"));
        // The two runs share 3 m of conduit: factor 0.80, and 2.5 mm² (19.2 A)
        // still takes 16 A.
        assert_eq!(sharing(&home, "C1"), 2);
        assert_eq!((c1.wire_mm2, c1.breaker_a), (2.5, 16), "{c1:?}");
    }
}
