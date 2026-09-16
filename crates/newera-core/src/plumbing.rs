//! The plumbing project as an installer reads it: the water and sewer points
//! each fixture needs, where the supply comes from and where the sewer goes,
//! and what a branch takes to build.
//!
//! A point is a piece of the plumbing project and is what its catalog entry
//! says (`cold-water`, `sewer`, `floor-drain`…), never what it is named after.
//! A fixture is the piece it serves: a toilet, a basin, a sink. NBR 5626 and
//! NBR 8160 are paid standards we hold only in part, so the rules check that
//! a thing exists and is reachable, and the figures they use — branch
//! diameters, slopes — are the ones every installer's table repeats, said as
//! such.

use serde::Serialize;

use crate::electrical::{Finding, Material, Severity, inside, metres, reaches};
use crate::elements::Room;
use crate::furniture::Furniture;
use crate::geometry::Point2;
use crate::home::Home;
use crate::ids::{FurnitureId, RoomId};
use crate::routing::{Route, Terminal, Via};
use crate::style::Polyline;

/// What a pipe run carries.
pub const PIPE_KEY: &str = "plumb:pipe";
/// The run a laid-out pipe belongs to, so laying it again replaces it.
pub const RUN_KEY: &str = "plumb:run";
/// The run's real length, drops included, cm.
pub const RUN_CM_KEY: &str = "plumb:run_cm";

/// What a plumbing point is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum PointKind {
    Cold,
    Hot,
    Sewer,
    Drain,
    Valve,
    GreaseTrap,
    InspectionBox,
    WaterMeter,
    Gas,
}

impl PointKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "Água fria",
            Self::Hot => "Água quente",
            Self::Sewer => "Esgoto",
            Self::Drain => "Ralo",
            Self::Valve => "Registro",
            Self::GreaseTrap => "Caixa de gordura",
            Self::InspectionBox => "Caixa de inspeção",
            Self::WaterMeter => "Hidrômetro",
            Self::Gas => "Gás",
        }
    }

    fn of(catalog: &str) -> Option<Self> {
        Some(match catalog {
            "cold-water" => Self::Cold,
            "hot-water" => Self::Hot,
            "sewer" => Self::Sewer,
            "floor-drain" => Self::Drain,
            "valve" => Self::Valve,
            "grease-trap" => Self::GreaseTrap,
            "inspection-box" => Self::InspectionBox,
            "water-meter" => Self::WaterMeter,
            "gas-point" => Self::Gas,
            _ => return None,
        })
    }
}

/// A point of the project, located.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Point {
    pub id: FurnitureId,
    pub kind: PointKind,
    pub name: String,
    pub room: Option<RoomId>,
    pub at: Point2,
    /// Height of its centre above the floor, cm.
    pub z: f64,
}

/// A piece that uses water.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Fixture {
    Toilet,
    Basin,
    KitchenSink,
    Shower,
    Bathtub,
    Washer,
    LaundrySink,
    Dishwasher,
}

impl Fixture {
    pub fn name(self) -> &'static str {
        match self {
            Self::Toilet => "vaso sanitário",
            Self::Basin => "lavatório",
            Self::KitchenSink => "pia de cozinha",
            Self::Shower => "chuveiro",
            Self::Bathtub => "banheira",
            Self::Washer => "máquina de lavar",
            Self::LaundrySink => "tanque",
            Self::Dishwasher => "lava-louças",
        }
    }

    /// Whether it takes hot water where the home has it.
    fn takes_hot(self) -> bool {
        matches!(
            self,
            Self::Basin | Self::KitchenSink | Self::Shower | Self::Bathtub
        )
    }

    /// Whether a floor drain (a trap box) may take its waste instead of a
    /// sewer point of its own.
    fn drains_to_floor(self) -> bool {
        matches!(
            self,
            Self::Basin | Self::Shower | Self::Bathtub | Self::Washer | Self::LaundrySink
        )
    }

    /// The smallest discharge branch, mm, as the installers' tables from
    /// NBR 8160 give it: 100 for a toilet, 50 for a kitchen sink or a
    /// machine, 40 for the rest.
    pub fn sewer_mm(self) -> u32 {
        match self {
            Self::Toilet => 100,
            Self::KitchenSink | Self::Washer | Self::Dishwasher => 50,
            Self::Basin | Self::Shower | Self::Bathtub | Self::LaundrySink => 40,
        }
    }

    fn of(piece: &Furniture) -> Option<Self> {
        if piece.discipline.is_some() {
            return None;
        }
        let by_catalog = match piece.catalog.as_str() {
            "toilet" => Some(Self::Toilet),
            "basin-cabinet" => Some(Self::Basin),
            "sink-counter" => Some(Self::KitchenSink),
            "shower" | "shower-glass" => Some(Self::Shower),
            "bathtub" => Some(Self::Bathtub),
            "washer" => Some(Self::Washer),
            "laundry-sink" => Some(Self::LaundrySink),
            "dishwasher" => Some(Self::Dishwasher),
            _ => None,
        };
        if by_catalog.is_some() {
            return by_catalog;
        }
        let name = crate::annotations::fold(&piece.name);
        let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
        if has(&["vaso sanit", "bacia sanit", "vaso "]) || name == "vaso" {
            Some(Self::Toilet)
        } else if has(&["lavatorio", "cuba"]) {
            Some(Self::Basin)
        } else if has(&["pia"]) {
            Some(Self::KitchenSink)
        } else if has(&["chuveiro", "ducha"]) {
            Some(Self::Shower)
        } else if has(&["banheira"]) {
            Some(Self::Bathtub)
        } else if has(&["lava-loucas", "lava loucas"]) {
            Some(Self::Dishwasher)
        } else if has(&["maquina de lavar", "lava e seca", "lavadora"]) {
            Some(Self::Washer)
        } else if has(&["tanque"]) {
            Some(Self::LaundrySink)
        } else {
            None
        }
    }
}

/// Whether a piece heats water: a heater, a boiler, by catalog or name.
pub fn is_heater(piece: &Furniture) -> bool {
    let name = crate::annotations::fold(&piece.name);
    piece.catalog.contains("heater")
        || ["aquecedor", "boiler", "caldeira"]
            .iter()
            .any(|w| name.contains(w))
}

/// What a pipe carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Pipe {
    Cold,
    Hot,
    Sewer,
}

impl Pipe {
    pub const ALL: [Self; 3] = [Self::Cold, Self::Hot, Self::Sewer];

    pub fn key(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Hot => "hot",
            Self::Sewer => "sewer",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == raw.trim())
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "Água fria",
            Self::Hot => "Água quente",
            Self::Sewer => "Esgoto",
        }
    }

    /// Drawn as plumbing drawings are: cold water blue, hot red dashed, sewer
    /// brown.
    pub fn style(self) -> (crate::style::DashStyle, [u8; 3]) {
        match self {
            Self::Cold => (crate::style::DashStyle::Solid, [40, 110, 210]),
            Self::Hot => (crate::style::DashStyle::Dash, [210, 60, 40]),
            Self::Sewer => (crate::style::DashStyle::Solid, [120, 90, 60]),
        }
    }

    /// The points a run of it ends at.
    pub fn serves(self, kind: PointKind) -> bool {
        match self {
            Self::Cold => kind == PointKind::Cold,
            Self::Hot => kind == PointKind::Hot,
            Self::Sewer => matches!(kind, PointKind::Sewer | PointKind::Drain),
        }
    }
}

/// The pipe a line of the plan is, when it says.
pub fn pipe_of(line: &Polyline) -> Option<Pipe> {
    Pipe::parse(line.properties.get(PIPE_KEY)?)
}

fn room_of(rooms: &[Room], p: Point2) -> Option<&Room> {
    rooms
        .iter()
        .filter(|r| r.points.len() >= 3 && inside(&r.points, p))
        .min_by(|a, b| a.area().total_cmp(&b.area()))
}

/// Every plumbing point on the storey shown.
pub fn points(home: &Home) -> Vec<Point> {
    let view = home.level_view(home.current_level());
    let mut out: Vec<Point> = view
        .furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter_map(|f| {
            let kind = PointKind::of(&f.catalog)?;
            Some(Point {
                id: f.id,
                kind,
                name: f.name.clone(),
                room: room_of(&view.rooms, f.position).map(|r| r.id),
                at: f.position,
                z: f.elevation + f.height / 2.0,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        (a.at.y, a.at.x)
            .partial_cmp(&(b.at.y, b.at.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Every fixture on the storey shown, with the piece it is.
pub fn fixtures(home: &Home) -> Vec<(Fixture, Furniture)> {
    let view = home.level_view(home.current_level());
    view.furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter_map(|f| Fixture::of(f).map(|x| (x, f.clone())))
        .collect()
}

/// How far from a fixture's centre its points may stand: half its size and
/// the length of a flexible hose or a short stub.
fn reach_of(piece: &Furniture) -> f64 {
    piece.width.max(piece.depth) / 2.0 + 60.0
}

/// The fixture a point serves: the nearest one in reach.
pub fn served_by(home: &Home, at: Point2) -> Option<Fixture> {
    fixtures(home)
        .into_iter()
        .filter(|(_, f)| f.position.distance(at) <= reach_of(f))
        .min_by(|a, b| {
            a.1.position
                .distance(at)
                .total_cmp(&b.1.position.distance(at))
        })
        .map(|(x, _)| x)
}

/// The discharge diameter a sewer point takes, mm: its fixture's, a trap box
/// for a floor drain (outlet 50), and 50 when nothing says.
pub fn sewer_mm(home: &Home, point: &Point) -> u32 {
    if point.kind == PointKind::Drain {
        return 50;
    }
    let name = crate::annotations::fold(&point.name);
    if name.contains("vaso") || name.contains("bacia") {
        return 100;
    }
    served_by(home, point.at).map_or(50, Fixture::sewer_mm)
}

/// Minimum slope of a horizontal sewer pipe, per unit: 2 % up to 75 mm, 1 %
/// above.
pub fn slope(mm: u32) -> f64 {
    if mm <= 75 { 0.02 } else { 0.01 }
}

fn place(room: Option<&Room>, piece: &Furniture) -> String {
    match room {
        Some(r) if !r.name.trim().is_empty() => format!("{} {} ({})", piece.name, piece.id, r.name),
        _ => format!("{} {}", piece.name, piece.id),
    }
}

/// What each fixture and the project as a whole lack.
#[allow(clippy::too_many_lines)]
pub fn check(home: &Home) -> Vec<Finding> {
    let view = home.level_view(home.current_level());
    let all = points(home);
    let fixtures = fixtures(home);
    let mut out = Vec::new();
    let near = |piece: &Furniture, kinds: &[PointKind]| {
        all.iter()
            .any(|p| kinds.contains(&p.kind) && p.at.distance(piece.position) <= reach_of(piece))
    };
    let has = |kind: PointKind| all.iter().any(|p| p.kind == kind);
    let hot_system = has(PointKind::Hot);
    for (fixture, piece) in &fixtures {
        let room = room_of(&view.rooms, piece.position);
        let at = place(room, piece);
        let reach = reach_of(piece).round();
        if !near(piece, &[PointKind::Cold]) {
            out.push(Finding {
                key: format!("plumb:cold:{}", piece.id),
                accepted: None,
                severity: Severity::Erro,
                place: at.clone(),
                message: format!(
                    "Sem ponto de água fria a até {reach} cm: o {} não tem de onde ser alimentado.",
                    fixture.name()
                ),
                source: "nbr5626",
            });
        }
        if hot_system && fixture.takes_hot() && !near(piece, &[PointKind::Hot]) {
            out.push(Finding {
                key: format!("plumb:hot:{}", piece.id),
                accepted: None,
                severity: Severity::Alerta,
                place: at.clone(),
                message: format!(
                    "O projeto tem água quente e o {} não recebe: falta o ponto a até {reach} cm.",
                    fixture.name()
                ),
                source: "nbr5626",
            });
        }
        let outlets: &[PointKind] = if fixture.drains_to_floor() {
            &[PointKind::Sewer, PointKind::Drain]
        } else {
            &[PointKind::Sewer]
        };
        if !near(piece, outlets) {
            let how = if fixture.drains_to_floor() {
                "ponto de esgoto ou ralo sifonado"
            } else {
                "ponto de esgoto próprio"
            };
            out.push(Finding {
                key: format!("plumb:sewer:{}", piece.id),
                accepted: None,
                severity: Severity::Erro,
                place: at,
                message: format!(
                    "Sem {how} a até {reach} cm: o {} não tem para onde escoar (ramal de {} mm).",
                    fixture.name(),
                    fixture.sewer_mm()
                ),
                source: "nbr8160",
            });
        }
    }
    // A bathroom's wet floor drains through a trap box.
    for room in view.rooms.iter().filter(|r| r.points.len() >= 3) {
        let wet = fixtures.iter().any(|(x, f)| {
            matches!(x, Fixture::Shower | Fixture::Bathtub) && inside(&room.points, f.position)
        });
        if wet
            && !all
                .iter()
                .any(|p| p.kind == PointKind::Drain && p.room == Some(room.id))
        {
            out.push(Finding {
                key: format!("plumb:drain:{}", room.id),
                accepted: None,
                severity: Severity::Alerta,
                place: format!("{} {}", room.name, room.id),
                message:
                    "Sem ralo: a área do box pede um ralo sifonado, que também recebe o lavatório."
                        .into(),
                source: "nbr8160",
            });
        }
    }
    if fixtures.iter().any(|(x, _)| *x == Fixture::KitchenSink) && !has(PointKind::GreaseTrap) {
        out.push(Finding {
            key: "plumb:grease".into(),
            accepted: None,
            severity: Severity::Dica,
            place: "Pia de cozinha".into(),
            message: "Sem caixa de gordura: numa casa ela fica entre a pia e a rede; em prédio costuma ser a coletiva do térreo — se for o caso, aceite com esse motivo.".into(),
            source: "nbr8160",
        });
    }
    // The premises: where the water comes from and where the sewer goes.
    if all
        .iter()
        .any(|p| matches!(p.kind, PointKind::Cold | PointKind::Hot))
        && !has(PointKind::WaterMeter)
        && !has(PointKind::Valve)
    {
        out.push(Finding {
            key: "plumb:source:water".into(),
            accepted: None,
            severity: Severity::Alerta,
            place: "Água fria".into(),
            message: "De onde vem a água? Coloque o hidrômetro ou o registro geral (em prédio, junto à prumada no shaft): é deles que os ramais saem.".into(),
            source: "nbr5626",
        });
    }
    if all
        .iter()
        .any(|p| matches!(p.kind, PointKind::Sewer | PointKind::Drain))
        && !has(PointKind::InspectionBox)
    {
        out.push(Finding {
            key: "plumb:source:sewer".into(),
            accepted: None,
            severity: Severity::Alerta,
            place: "Esgoto".into(),
            message: "Para onde vai o esgoto? Coloque a caixa de inspeção (em prédio, o tubo de queda no shaft) — é a ela que os ramais descem com caimento.".into(),
            source: "nbr8160",
        });
    }
    // Where pipes are drawn, each point of their kind needs one reaching it.
    for pipe in Pipe::ALL {
        let runs: Vec<&Polyline> = view
            .polylines
            .iter()
            .filter(|l| pipe_of(l) == Some(pipe))
            .collect();
        if runs.is_empty() {
            continue;
        }
        let unreached: Vec<String> = all
            .iter()
            .filter(|p| pipe.serves(p.kind) && !runs.iter().any(|l| reaches(l, p.at)))
            .map(|p| p.id.to_string())
            .collect();
        if !unreached.is_empty() {
            out.push(Finding {
                key: format!("plumb:unreached:{}", pipe.key()),
                accepted: None,
                severity: Severity::Alerta,
                place: pipe.name().into(),
                message: format!("Pontos sem tubulação chegando: {}.", unreached.join(", ")),
                source: if pipe == Pipe::Sewer {
                    "nbr8160"
                } else {
                    "nbr5626"
                },
            });
        }
    }
    for f in &mut out {
        f.accepted = home.accepted.get(&f.key).cloned();
    }
    out.sort_by_key(|f| f.severity);
    out
}

/// Acceptances whose finding is gone.
pub fn orphaned(home: &Home) -> Vec<(String, String)> {
    let live: std::collections::BTreeSet<String> = check(home).into_iter().map(|f| f.key).collect();
    home.accepted
        .iter()
        .filter(|(key, _)| key.starts_with("plumb:") && !live.contains(*key))
        .map(|(key, why)| (key.clone(), why.clone()))
        .collect()
}

/// Pipe metres by kind, for the purchase: a laid-out run counts its real
/// length, a line drawn by hand its plan length with a tenth for the drops.
pub fn pipe_lengths(home: &Home) -> Vec<(Pipe, f64)> {
    let view = home.level_view(home.current_level());
    Pipe::ALL
        .into_iter()
        .map(|pipe| {
            let mut routed: std::collections::BTreeMap<String, f64> =
                std::collections::BTreeMap::new();
            let mut drawn = 0.0;
            for line in view.polylines.iter().filter(|l| pipe_of(l) == Some(pipe)) {
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
            (pipe, metres(drawn + routed.values().sum::<f64>()))
        })
        .filter(|(_, m)| *m > 0.0)
        .collect()
}

/// Why a premise cannot carry a pipe, if it cannot.
///
/// Sewer runs by gravity: it never climbs to this storey's ceiling, and a
/// wall only takes it going down — lying in a wall it would have no fall.
/// Under the floor it is the slab's recess, or the ceiling of the storey
/// below, measured from this floor.
pub fn refuses(pipe: Pipe, via: Via) -> Option<&'static str> {
    match (pipe, via) {
        (Pipe::Sewer, Via::Ceiling) => Some(
            "esgoto corre por gravidade e não sobe ao forro deste andar: use floor (rebaixo da laje ou forro do andar de baixo)",
        ),
        (Pipe::Sewer, Via::Wall) => Some(
            "esgoto não corre deitado dentro da parede, sem caimento: na parede ele só desce; use floor",
        ),
        _ => None,
    }
}

/// The height a sewer run needs under the floor, cm: the fall of its longest
/// branch at its slope, the pipe itself and 2 cm to lay it on.
pub fn sewer_depth(route: &Route, trunk_mm: u32) -> f64 {
    let longest = route
        .terminals
        .iter()
        .zip(&route.reach)
        .skip(1)
        .map(|(t, cm)| cm - t.z.max(0.0) - route.terminals[0].z.max(0.0))
        .fold(0.0, f64::max);
    let cm = longest * slope(trunk_mm) + f64::from(trunk_mm) / 10.0 + 2.0;
    (cm * 10.0).round() / 10.0
}

/// A point to lay a run to.
pub fn terminal(point: &Point) -> Terminal {
    Terminal {
        id: Some(point.id),
        at: point.at,
        z: point.z,
    }
}

/// What a laid-out run takes to build.
///
/// Cold water: PVC solvent-weld 25 mm in 6 m bars, a 90° elbow at each turn,
/// a tee at each branch, an elbow with a brass thread at each point and a
/// gate valve per room served. Hot water: CPVC 22 mm in 3 m bars with the
/// same fittings. Sewer: the trunk at the largest diameter it receives, a
/// point's own diameter from the trunk down to it, a 45° junction (a Y) at
/// each branch — a 90° tee does not join sewer lying down —, two 45° elbows
/// for each turn, a short 90° bend under each point, a sealing ring per
/// toilet and a trap box per floor drain.
#[allow(clippy::cast_precision_loss)] // counts of fittings, far below 2^52
pub fn materials(
    route: &Route,
    pipe: Pipe,
    points: &[(PointKind, u32)],
    rooms: usize,
) -> Vec<Material> {
    let n = points.len();
    let bends = route.bends.saturating_sub(if matches!(pipe, Pipe::Sewer) {
        n + 1
    } else {
        0
    });
    let item = |item: String, quantity: f64, unit: &'static str| Material {
        item,
        quantity,
        unit,
    };
    let mut out = Vec::new();
    match pipe {
        Pipe::Cold | Pipe::Hot => {
            let (tube, bar, size, thread) = match pipe {
                Pipe::Cold => (
                    "Tubo PVC soldável 25 mm (3/4\")",
                    6.0,
                    "PVC soldável 25 mm",
                    "Joelho 90° soldável com bucha de latão 25 mm × 1/2\"",
                ),
                _ => (
                    "Tubo CPVC 22 mm (água quente)",
                    3.0,
                    "CPVC 22 mm",
                    "Conector CPVC 22 mm × 1/2\" com rosca metálica",
                ),
            };
            let metres = metres(route.length() * 1.1);
            out.push(item(tube.into(), metres, "m"));
            out.push(item(
                format!("Barras de {bar} m"),
                (metres / bar).ceil(),
                "un",
            ));
            out.push(item(format!("Joelho 90° {size}"), route.bends as f64, "un"));
            out.push(item(format!("Tê {size}"), route.branches as f64, "un"));
            out.push(item(thread.into(), n as f64, "un"));
            out.push(item(
                format!("Registro de gaveta {size} (um por ambiente)"),
                rooms.max(1) as f64,
                "un",
            ));
            let joints = 2 * route.bends + 3 * route.branches + n + 2 * rooms.max(1);
            out.push(item(
                if pipe == Pipe::Cold {
                    "Adesivo para PVC 175 g".into()
                } else {
                    "Adesivo para CPVC 175 g".into()
                },
                ((joints as f64) / 40.0).ceil().max(1.0),
                "un",
            ));
        }
        Pipe::Sewer => {
            let trunk = points.iter().map(|(_, mm)| *mm).max().unwrap_or(50);
            let drops: f64 = route
                .terminals
                .iter()
                .skip(1)
                .map(|t| t.z.max(0.0) + 30.0)
                .sum();
            let horizontal = metres((route.length() - drops).max(0.0) * 1.05);
            out.push(item(
                format!("Tubo PVC esgoto série normal {trunk} mm (ramal)"),
                horizontal,
                "m",
            ));
            let mut by_mm: std::collections::BTreeMap<u32, f64> = std::collections::BTreeMap::new();
            for ((_, mm), t) in points.iter().zip(route.terminals.iter().skip(1)) {
                *by_mm.entry(*mm).or_default() += t.z.max(0.0) + 30.0;
            }
            let mut bars = horizontal;
            for (mm, cm) in &by_mm {
                out.push(item(
                    format!("Tubo PVC esgoto série normal {mm} mm (descidas aos pontos)"),
                    metres(*cm),
                    "m",
                ));
                bars += metres(*cm);
            }
            out.push(item("Barras de 6 m".into(), (bars / 6.0).ceil(), "un"));
            let mut junctions: std::collections::BTreeMap<u32, usize> =
                std::collections::BTreeMap::new();
            for ((_, mm), degree) in points.iter().zip(route.degrees.iter().skip(1)) {
                if *degree > 1 {
                    *junctions.entry(*mm).or_default() += degree - 1;
                }
            }
            let source_split = route.degrees.first().map_or(0, |d| d.saturating_sub(1));
            if source_split > 0 {
                *junctions.entry(trunk).or_default() += source_split;
            }
            for (mm, count) in junctions {
                out.push(item(
                    format!("Junção simples 45° (Y) {trunk} × {mm} mm"),
                    count as f64,
                    "un",
                ));
            }
            out.push(item(
                format!("Joelho 45° {trunk} mm (dois por curva)"),
                (2 * bends) as f64,
                "un",
            ));
            for (mm, count) in points.iter().fold(
                std::collections::BTreeMap::<u32, usize>::new(),
                |mut m, (_, mm)| {
                    *m.entry(*mm).or_default() += 1;
                    m
                },
            ) {
                out.push(item(
                    format!("Curva 90° curta {mm} mm (sob o ponto)"),
                    count as f64,
                    "un",
                ));
            }
            let toilets = points
                .iter()
                .filter(|(k, mm)| *k == PointKind::Sewer && *mm >= 100)
                .count();
            if toilets > 0 {
                out.push(item(
                    "Anel de vedação para bacia sanitária".into(),
                    toilets as f64,
                    "un",
                ));
            }
            let drains = points
                .iter()
                .filter(|(k, _)| *k == PointKind::Drain)
                .count();
            if drains > 0 {
                out.push(item(
                    "Caixa sifonada 150×150×50 mm com grelha".into(),
                    drains as f64,
                    "un",
                ));
            }
            out.push(item(
                "Pasta lubrificante para junta elástica 400 g".into(),
                1.0,
                "un",
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::cast_precision_loss)] // counts and table values are exact

    use super::*;
    use crate::elements::Wall;
    use crate::ids::WallId;
    use crate::routing::lay_out;
    use crate::style::Discipline;

    fn piece(
        id: u64,
        catalog: &str,
        name: &str,
        at: (f64, f64),
        size: (f64, f64, f64),
    ) -> Furniture {
        Furniture {
            id: FurnitureId(id),
            catalog: catalog.into(),
            name: name.into(),
            position: Point2::new(at.0, at.1),
            width: size.0,
            depth: size.1,
            height: size.2,
            ..Furniture::default()
        }
    }

    fn point(id: u64, catalog: &str, name: &str, at: (f64, f64), elevation: f64) -> Furniture {
        let mut f = piece(id, catalog, name, at, (8.0, 8.0, 8.0));
        f.elevation = elevation;
        f.discipline = Some(Discipline::Plumbing);
        f
    }

    fn bathroom() -> Home {
        let mut home = Home::default();
        let pts = [(0.0, 0.0), (200.0, 0.0), (200.0, 250.0), (0.0, 250.0)];
        for k in 0..4 {
            let (a, b) = (pts[k], pts[(k + 1) % 4]);
            home.walls.push(Wall::new(
                WallId(k as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            ));
        }
        home.rooms.push(Room::new(
            RoomId(9),
            "Banho suíte",
            pts.iter().map(|p| Point2::new(p.0, p.1)).collect(),
        ));
        home.furniture = vec![
            piece(20, "toilet", "Vaso", (50.0, 35.0), (40.0, 63.0, 62.0)),
            piece(
                21,
                "basin-cabinet",
                "Gabinete",
                (150.0, 25.0),
                (60.0, 45.0, 85.0),
            ),
            piece(22, "shower", "Box", (150.0, 200.0), (90.0, 90.0, 200.0)),
        ];
        home
    }

    #[test]
    fn every_fixture_asks_for_its_water_and_its_sewer_and_the_premises_are_asked() {
        let mut home = bathroom();
        let keys = |home: &Home| check(home).into_iter().map(|f| f.key).collect::<Vec<_>>();
        let bare = keys(&home);
        for key in [
            "plumb:cold:f20",
            "plumb:sewer:f20",
            "plumb:cold:f22",
            "plumb:sewer:f22",
            "plumb:drain:r9",
        ] {
            assert!(bare.contains(&key.to_owned()), "{key} in {bare:?}");
        }
        // No point at all: no premise asked yet, nothing to feed.
        assert!(
            !bare.iter().any(|k| k.starts_with("plumb:source")),
            "{bare:?}"
        );

        home.furniture.extend([
            point(30, "cold-water", "AF — vaso", (30.0, 5.0), 20.0),
            point(31, "sewer", "Esgoto — vaso banho suíte", (50.0, 10.0), 0.0),
            point(32, "cold-water", "AF — lavatório", (150.0, 5.0), 60.0),
            point(33, "cold-water", "AF — chuveiro", (195.0, 200.0), 210.0),
            point(34, "floor-drain", "Ralo", (150.0, 200.0), 0.0),
            point(38, "sewer", "Esgoto — lavatório", (150.0, 10.0), 50.0),
        ]);
        let wired = keys(&home);
        assert!(
            !wired
                .iter()
                .any(|k| k.starts_with("plumb:cold") || k.starts_with("plumb:sewer:")),
            "{wired:?}"
        );
        // The shower drains through the floor drain, which the room had lacked.
        assert!(!wired.contains(&"plumb:drain:r9".to_owned()), "{wired:?}");
        assert!(
            wired.contains(&"plumb:source:water".to_owned()),
            "{wired:?}"
        );
        assert!(
            wired.contains(&"plumb:source:sewer".to_owned()),
            "{wired:?}"
        );
        // No hot water anywhere: an electric shower, nothing to ask.
        assert!(
            !wired.iter().any(|k| k.starts_with("plumb:hot")),
            "{wired:?}"
        );

        // Hot water in the project: the shower and the basin need it.
        home.furniture.push(point(
            35,
            "hot-water",
            "AQ — chuveiro",
            (180.0, 200.0),
            210.0,
        ));
        let hot = keys(&home);
        assert!(hot.contains(&"plumb:hot:f21".to_owned()), "{hot:?}");
        assert!(!hot.contains(&"plumb:hot:f22".to_owned()), "{hot:?}");
        assert!(
            !hot.contains(&"plumb:hot:f20".to_owned()),
            "a toilet takes no hot water: {hot:?}"
        );

        // The point named after the toilet is the toilet's 100 mm outlet.
        let points = points(&home);
        let sewer = points.iter().find(|p| p.id == FurnitureId(31)).unwrap();
        assert_eq!(sewer_mm(&home, sewer), 100);
        let drain = points.iter().find(|p| p.id == FurnitureId(34)).unwrap();
        assert_eq!(sewer_mm(&home, drain), 50);
    }

    #[test]
    fn sewer_runs_under_the_floor_with_its_fall_and_joins_with_a_y() {
        let mut home = bathroom();
        home.furniture.extend([
            point(31, "sewer", "Esgoto — vaso", (50.0, 10.0), 0.0),
            point(34, "floor-drain", "Ralo", (150.0, 200.0), 0.0),
            point(36, "inspection-box", "Prumada", (0.0, 125.0), 0.0),
        ]);
        assert!(refuses(Pipe::Sewer, Via::Ceiling).is_some());
        assert!(refuses(Pipe::Sewer, Via::Wall).is_some());
        assert!(refuses(Pipe::Cold, Via::Ceiling).is_none());
        let all = points(&home);
        let source = terminal(
            all.iter()
                .find(|p| p.kind == PointKind::InspectionBox)
                .unwrap(),
        );
        let ends: Vec<&Point> = all.iter().filter(|p| Pipe::Sewer.serves(p.kind)).collect();
        let route = lay_out(
            &home,
            source,
            &ends.iter().map(|p| terminal(p)).collect::<Vec<_>>(),
            Via::Floor,
            280.0,
        );
        let sizes: Vec<(PointKind, u32)> =
            ends.iter().map(|p| (p.kind, sewer_mm(&home, p))).collect();
        let depth = sewer_depth(&route, 100);
        // 1 % over the longest branch, the 10 cm pipe and 2 cm under it.
        let longest = route.reach.iter().copied().fold(0.0, f64::max);
        assert!(
            depth >= 12.0 && depth <= longest * 0.01 + 12.1,
            "{depth} for {longest}"
        );
        let bill = materials(&route, Pipe::Sewer, &sizes, 1);
        let text = format!("{bill:?}");
        assert!(text.contains("série normal 100 mm (ramal)"), "{bill:#?}");
        assert!(text.contains("Anel de vedação"), "{bill:#?}");
        assert!(text.contains("Caixa sifonada"), "{bill:#?}");
        assert!(!text.contains("Tê"), "no 90° tee joins sewer: {bill:#?}");
        assert!(
            route.branches == 0 || text.contains("Junção simples 45° (Y)"),
            "{bill:#?}"
        );
    }

    #[test]
    fn a_cold_water_branch_lists_tees_elbows_and_a_valve_per_room() {
        let mut home = bathroom();
        home.furniture.extend([
            point(30, "cold-water", "AF — vaso", (30.0, 5.0), 20.0),
            point(32, "cold-water", "AF — lavatório", (150.0, 5.0), 60.0),
            point(33, "cold-water", "AF — chuveiro", (195.0, 200.0), 210.0),
            point(37, "water-meter", "Hidrômetro", (0.0, 125.0), 100.0),
        ]);
        let all = points(&home);
        let source = terminal(
            all.iter()
                .find(|p| p.kind == PointKind::WaterMeter)
                .unwrap(),
        );
        let ends: Vec<Terminal> = all
            .iter()
            .filter(|p| p.kind == PointKind::Cold)
            .map(terminal)
            .collect();
        let route = lay_out(&home, source, &ends, Via::Ceiling, 280.0);
        let sizes = vec![(PointKind::Cold, 25); 3];
        let bill = materials(&route, Pipe::Cold, &sizes, 1);
        let get = |item: &str| {
            bill.iter()
                .find(|m| m.item.starts_with(item))
                .unwrap_or_else(|| panic!("{item}: {bill:#?}"))
                .quantity
        };
        assert!(get("Tubo PVC soldável 25 mm") >= route.length() / 100.0);
        assert_eq!(get("Joelho 90° soldável com bucha"), 3.0);
        assert_eq!(get("Tê PVC"), route.branches as f64);
        assert_eq!(get("Registro de gaveta"), 1.0);
        assert!(get("Barras de 6 m") * 6.0 >= get("Tubo PVC soldável 25 mm"));
    }
}
