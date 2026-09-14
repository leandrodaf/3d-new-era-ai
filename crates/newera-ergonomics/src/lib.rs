//! Ergonomics and habitability review.
//!
//! Given who lives in a home — how many people, children, elderly, a
//! wheelchair user — this reads the plan the way a careful architect would:
//! is there room to walk beside the bed and in front of the stove, do the
//! beds, seats, bathrooms and wardrobes serve everyone, does the kitchen
//! work, can a wheelchair turn in the bathroom. Each finding says what is
//! wrong with its numbers, what to change, and which reference it follows.
//!
//! References are Brazilian where one exists: ABNT NBR 9050 (accessibility),
//! ABNT NBR 15575-1 (residential performance: ceiling heights and the
//! informative annex of minimum furniture and circulation), IBGE's crowding
//! criterion, and municipal building codes for areas and windows — which
//! vary by city, so those come as advice to confirm. Ergonomic ranges for
//! kitchens follow common practice (work triangle, counter heights).

// Counts of people and pieces are tiny; cm fit in any float.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

mod scene;

use newera_core::{Home, Issue, OpeningKind, Point2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use scene::{RoomUse, Scene, Side, Space, Unit, Use};

/// Who lives there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Profile {
    /// People living in the home (default 2).
    pub occupants: u32,
    /// Of them, children (sleep in single beds or cribs).
    pub children: u32,
    /// Of them, elderly people.
    pub elderly: u32,
    /// Someone uses a wheelchair: NBR 9050 turning space, doors and reach.
    pub wheelchair: bool,
    /// Height of the main cook, cm, to size the countertop (default 165).
    pub stature: Option<f64>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            occupants: 2,
            children: 0,
            elderly: 0,
            wheelchair: false,
            stature: None,
        }
    }
}

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Something can't be used as drawn.
    Erro,
    /// Below the reference: works badly.
    Alerta,
    /// Would be better.
    Dica,
}

/// One thing to look at.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    /// Room or piece it is about, e.g. `Quarto r5` or `Cama de casal f12`.
    pub place: String,
    /// What is wrong, with the numbers and what to do.
    pub message: String,
    /// A checked change that solves it, as MCP tool arguments:
    /// `{"tool":"move","ids":["f12"],"dx":-20,"dy":0}` or
    /// `{"tool":"update","items":[{"id":"f3","hinge_right":true}]}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<serde_json::Value>,
}

/// What the home offers its people.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Capacity {
    pub beds: u32,
    pub bedrooms: u32,
    pub bathrooms: u32,
    pub dining_seats: u32,
    pub living_seats: u32,
    /// Wardrobe front length in bedrooms, cm.
    pub wardrobe_cm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// 100 minus weighted findings.
    pub score: u32,
    pub capacity: Capacity,
    /// Worst first.
    pub findings: Vec<Finding>,
}

/// Wardrobe front per adult, cm (common practice; children count half).
const WARDROBE_PER_ADULT: f64 = 60.0;

fn cm(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.1}").replace('.', ",")
    }
}

fn m2(v: f64) -> String {
    format!("{:.2}", v / 10_000.0).replace('.', ",")
}

struct Review<'s, 'a> {
    scene: &'s Scene<'a>,
    profile: &'s Profile,
    findings: Vec<Finding>,
}

impl Review<'_, '_> {
    fn push(&mut self, severity: Severity, place: impl Into<String>, message: impl Into<String>) {
        self.findings.push(Finding {
            severity,
            place: place.into(),
            message: message.into(),
            fix: None,
        });
    }

    /// The room a unit stands in.
    fn room_of(&self, i: usize) -> Option<&Space<'_>> {
        self.scene.spaces.iter().find(|s| s.units.contains(&i))
    }

    fn capacity(&self) -> Capacity {
        let scene = self.scene;
        let mut c = Capacity::default();
        for (i, u) in scene.units.iter().enumerate() {
            let room = self.room_of(i).map(|s| s.what);
            match u.what {
                Use::Bed(n) => c.beds += n,
                Use::Crib => c.beds += 1,
                Use::DiningTable(n) | Use::DiningSet(n) => c.dining_seats += n,
                Use::Stool => c.dining_seats += 1,
                Use::Sofa(n) => c.living_seats += n,
                Use::Armchair => c.living_seats += 1,
                Use::Wardrobe if room.is_none_or(|r| r == RoomUse::Bedroom) => {
                    c.wardrobe_cm += u.piece.width;
                }
                _ => {}
            }
        }
        c.wardrobe_cm = c.wardrobe_cm.round();
        c.bedrooms = scene
            .spaces
            .iter()
            .filter(|s| s.what == RoomUse::Bedroom)
            .count() as u32;
        c.bathrooms = scene
            .spaces
            .iter()
            .filter(|s| s.units.iter().any(|&i| scene.units[i].what == Use::Toilet))
            .count() as u32;
        c
    }

    fn occupancy(&mut self, c: &Capacity) {
        let p = self.profile.clone();
        let people = p.occupants.max(1);
        let home = "Casa";
        if c.beds < people {
            self.push(
                Severity::Erro,
                home,
                format!(
                    "{people} moradores e {} lugares para dormir: {} (camas de casal contam 2).",
                    c.beds,
                    match people - c.beds {
                        1 => "falta 1".to_owned(),
                        n => format!("faltam {n}"),
                    }
                ),
            );
        }
        if c.bedrooms > 0 {
            let per = f64::from(people) / f64::from(c.bedrooms);
            if per > 3.0 {
                self.push(
                    Severity::Alerta,
                    home,
                    format!(
                        "{} moradores por dormitório: acima de 3 o IBGE considera adensamento excessivo; são necessários {} dormitórios.",
                        cm(per),
                        people.div_ceil(3)
                    ),
                );
            } else if per > 2.0 {
                self.push(
                    Severity::Dica,
                    home,
                    format!(
                        "{} moradores por dormitório; com {} dormitórios ninguém divide com mais de uma pessoa.",
                        cm(per),
                        people.div_ceil(2)
                    ),
                );
            }
        } else if people > 1 {
            self.push(
                Severity::Alerta,
                home,
                "Nenhum dormitório identificado: nomeie os cômodos (Quarto, Suíte) ou coloque camas.",
            );
        }
        if c.bathrooms == 0 {
            self.push(Severity::Erro, home, "Nenhum banheiro com vaso sanitário.");
        } else if people > 5 * c.bathrooms {
            self.push(
                Severity::Alerta,
                home,
                format!(
                    "{people} moradores para {} banheiro(s): acima de 5 por banheiro as filas de manhã são certas; considere um lavabo ou mais um banheiro.",
                    c.bathrooms
                ),
            );
        }
        if c.dining_seats < people {
            self.push(
                Severity::Alerta,
                home,
                format!(
                    "{} lugares à mesa para {people} moradores: use uma mesa de {} lugares.",
                    c.dining_seats,
                    people.max(4).div_ceil(2) * 2
                ),
            );
        }
        if c.living_seats < people {
            self.push(
                Severity::Dica,
                home,
                format!(
                    "{} lugares na sala para {people} moradores: todos sentam juntos com mais {} lugar(es) (poltrona ou sofá maior).",
                    c.living_seats,
                    people - c.living_seats
                ),
            );
        }
        let adults = people.saturating_sub(p.children);
        let needed = WARDROBE_PER_ADULT * f64::from(adults)
            + WARDROBE_PER_ADULT / 2.0 * f64::from(p.children.min(people));
        if c.wardrobe_cm + 1.0 < needed {
            self.push(
                Severity::Dica,
                home,
                format!(
                    "{} cm de guarda-roupa nos dormitórios; o usual é {} cm por adulto e metade por criança: {} cm no total.",
                    cm(c.wardrobe_cm),
                    cm(WARDROBE_PER_ADULT),
                    cm(needed)
                ),
            );
        }
    }

    /// Circulation around pieces: NBR 15575-1 informative annex (50 cm
    /// between furniture and walls, 85 cm in front of kitchen equipment) and
    /// common practice for the rest.
    #[allow(clippy::too_many_lines)]
    fn clearances(&mut self) {
        let scene = self.scene;
        let wheel = self.profile.wheelchair;
        for (i, u) in scene.units.iter().enumerate() {
            let label = u.label();
            let need = |side: Side, min: f64, span: (f64, f64), severity: Severity, what: &str| {
                let (free, blocker) = scene.free_and_blocker(i, side, min + 1.0, span);
                if free + 0.5 < min {
                    let where_ = match side {
                        Side::Front => "à frente",
                        Side::Left => "à esquerda",
                        Side::Right => "à direita",
                    };
                    let short = min - free;
                    // Sideways, the piece can slide over if the other side keeps its own room.
                    let fix = match side {
                        // In front: push the piece in the way back by the shortfall.
                        Side::Front => blocker.and_then(|j| {
                            let piece = scene.units[i].piece;
                            let way = piece.to_plan((0.0, 1.0));
                            let dir = (way.x - piece.position.x, way.y - piece.position.y);
                            push_away(scene.home, scene.units[j].piece.id, dir, short.ceil())
                        }),
                        Side::Left | Side::Right => {
                            let (other, sign) = if side == Side::Left {
                                (Side::Right, 1.0)
                            } else {
                                (Side::Left, -1.0)
                            };
                            let spare = scene.free(i, other, short + min + 1.0, span);
                            (spare - short + 0.5 >= min).then(|| {
                                let piece = scene.units[i].piece;
                                let to = piece.to_plan((sign * short.ceil(), 0.0));
                                let round = |v: f64| (v * 10.0).round() / 10.0;
                                serde_json::json!({
                                    "tool": "move",
                                    "ids": [piece.id.to_string()],
                                    "dx": round(to.x - piece.position.x),
                                    "dy": round(to.y - piece.position.y),
                                })
                            })
                        }
                    };
                    let advice = if let (Side::Front, Some(f), Some(j)) = (side, &fix, blocker) {
                        format!(
                            "afaste {} {} cm",
                            scene.units[j].label(),
                            cm(f["dx"]
                                .as_f64()
                                .unwrap_or_default()
                                .hypot(f["dy"].as_f64().unwrap_or_default()))
                        )
                    } else if fix.is_some() || side == Side::Front {
                        format!("afaste {} cm", cm(short))
                    } else {
                        "não há espaço do outro lado: use peça menor ou reorganize".to_owned()
                    };
                    Some(Finding {
                        severity,
                        place: label.clone(),
                        message: format!(
                            "{} cm livres {where_} ({what}: mínimo {} cm); {advice}.",
                            cm(free),
                            cm(min)
                        ),
                        fix,
                    })
                } else {
                    None
                }
            };
            let mut found: Vec<Finding> = Vec::new();
            // Beside a bed, the band past the nightstands.
            let beside = (0.25, 1.0);
            let whole = (0.0, 1.0);
            let kitchen_front = if wheel { 150.0 } else { 85.0 };
            let kitchen_source = if wheel {
                "giro de cadeira de rodas, NBR 9050"
            } else {
                "circulação diante de bancada e equipamentos, NBR 15575-1"
            };
            match u.what {
                Use::Bed(n) => {
                    let side_min = if wheel { 90.0 } else { 50.0 };
                    let why = if wheel {
                        "transferência da cadeira, NBR 9050"
                    } else {
                        "circulação ao lado da cama, NBR 15575-1"
                    };
                    let left = need(Side::Left, side_min, beside, Severity::Alerta, why);
                    let right = need(Side::Right, side_min, beside, Severity::Alerta, why);
                    if n >= 2 {
                        // A couple needs both sides.
                        found.extend(left);
                        found.extend(right);
                    } else if let (Some(l), Some(_)) = (left, right) {
                        // A single bed may touch a wall on one side.
                        found.push(l);
                    }
                    found.extend(need(
                        Side::Front,
                        50.0,
                        whole,
                        Severity::Dica,
                        "passagem aos pés da cama",
                    ));
                }
                Use::Crib => found.extend(need(
                    Side::Front,
                    50.0,
                    whole,
                    Severity::Dica,
                    "acesso ao berço",
                )),
                Use::Wardrobe => {
                    // Hinged doors of a joinery build swing their own width.
                    let leaf = u
                        .params
                        .as_ref()
                        .filter(|p| p["door"] != "sliding" && p["door"] != "none")
                        .map_or(0.0, |p| {
                            let w = p["w"].as_f64().unwrap_or(u.piece.width);
                            let doors = p["doors"]
                                .as_f64()
                                .unwrap_or_else(|| (w / 60.0).ceil())
                                .max(1.0);
                            w / doors
                        });
                    let min = (leaf + 10.0).max(60.0);
                    found.extend(need(
                        Side::Front,
                        min,
                        whole,
                        Severity::Alerta,
                        if leaf > 0.0 {
                            "abrir as portas e circular diante do guarda-roupa; portas de correr pedem menos"
                        } else {
                            "abrir as portas e circular diante do guarda-roupa"
                        },
                    ));
                }
                Use::Dresser => found.extend(need(
                    Side::Front,
                    70.0,
                    whole,
                    Severity::Dica,
                    "abrir gavetas e ficar diante delas",
                )),
                Use::Fridge | Use::Stove | Use::Sink | Use::Counter => {
                    found.extend(need(
                        Side::Front,
                        kitchen_front,
                        (0.05, 0.95),
                        Severity::Alerta,
                        kitchen_source,
                    ));
                }
                Use::Island => {
                    found.extend(need(
                        Side::Front,
                        90.0,
                        whole,
                        Severity::Alerta,
                        "circulação em volta da ilha",
                    ));
                }
                Use::Toilet => found.extend(need(
                    Side::Front,
                    if wheel { 120.0 } else { 60.0 },
                    whole,
                    Severity::Alerta,
                    if wheel {
                        "área de transferência, NBR 9050"
                    } else {
                        "uso do vaso"
                    },
                )),
                Use::Basin => found.extend(need(
                    Side::Front,
                    if wheel { 120.0 } else { 60.0 },
                    whole,
                    Severity::Alerta,
                    "uso do lavatório",
                )),
                Use::Shower | Use::Bathtub => found.extend(need(
                    Side::Front,
                    60.0,
                    (0.2, 0.8),
                    Severity::Dica,
                    "entrar e sair do box",
                )),
                Use::Washer | Use::LaundrySink => found.extend(need(
                    Side::Front,
                    60.0,
                    whole,
                    Severity::Alerta,
                    "uso da área de serviço",
                )),
                Use::Desk => found.extend(need(
                    Side::Front,
                    75.0,
                    (0.2, 0.8),
                    Severity::Dica,
                    "cadeira e levantar-se da mesa",
                )),
                Use::DiningTable(_) => {
                    for side in [Side::Front, Side::Left, Side::Right] {
                        found.extend(need(
                            side,
                            75.0,
                            (0.2, 0.8),
                            Severity::Alerta,
                            "puxar a cadeira e sentar",
                        ));
                    }
                }
                Use::DiningSet(_) => {
                    for side in [Side::Front, Side::Left, Side::Right] {
                        found.extend(need(
                            side,
                            40.0,
                            (0.2, 0.8),
                            Severity::Alerta,
                            "passar atrás das cadeiras",
                        ));
                    }
                }
                Use::Sofa(_) | Use::Armchair => {
                    found.extend(need(
                        Side::Front,
                        35.0,
                        (0.2, 0.8),
                        Severity::Dica,
                        "pernas e passagem diante do sofá",
                    ));
                }
                _ => {}
            }
            self.findings.extend(found);
        }
    }

    fn doors(&mut self) {
        let home = self.scene.home;
        let wheel = self.profile.wheelchair || self.profile.elderly > 0;
        for door in home.furniture.iter().filter(|f| f.visible) {
            let Some(opening) = &door.opening else {
                continue;
            };
            if opening.kind == OpeningKind::Window {
                continue;
            }
            let label = format!("{} {}", door.name, door.id);
            // Frame stops take about 4 cm of the nominal width.
            let clear = door.width - 4.0;
            if clear < 60.0 {
                self.push(
                    Severity::Alerta,
                    label,
                    format!(
                        "Vão livre de cerca de {} cm: abaixo de 60 cm não passa um móvel nem uma pessoa com volumes; use porta de 70 cm ou mais.",
                        cm(clear)
                    ),
                );
            } else if wheel && clear < 80.0 {
                self.push(
                    Severity::Alerta,
                    label,
                    format!(
                        "Vão livre de cerca de {} cm: a NBR 9050 pede 80 cm livres para cadeira de rodas e andador; use porta de 90 cm.",
                        cm(clear)
                    ),
                );
            }
        }
        for issue in newera_core::check_layout(home) {
            let name = |id| {
                home.furniture
                    .iter()
                    .flat_map(|f| std::iter::once(f).chain(f.children.iter()))
                    .find(|f| f.id == id)
                    .map_or_else(|| id.to_string(), |f| format!("{} {}", f.name, f.id))
            };
            match issue {
                Issue::BlocksDoor { door, by } => {
                    // Would the leaf clear it swinging from the other jamb?
                    let mut flipped = home.clone();
                    let flips = flipped
                        .furniture
                        .iter_mut()
                        .find(|f| f.id == door)
                        .and_then(|f| f.opening.as_mut())
                        .filter(|o| o.leaves < 2 && !o.sliding && o.sashes.is_empty())
                        .map(|o| {
                            o.hinge_right = !o.hinge_right;
                            o.hinge_right
                        });
                    let fix = flips.filter(|_| {
                        !newera_core::check_layout(&flipped)
                            .iter()
                            .any(|i| matches!(i, Issue::BlocksDoor { door: d, .. } if *d == door))
                    });
                    let moved = if fix.is_none() { nudge(home, by) } else { None };
                    if let Some(m) = moved {
                        self.findings.push(Finding {
                            severity: Severity::Erro,
                            place: name(door),
                            message: format!(
                                "A folha da porta bate em {}: movendo a peça {} cm a porta abre livre.",
                                name(by),
                                cm(m["d"].as_f64().unwrap_or_default())
                            ),
                            fix: Some(without_distance(m)),
                        });
                        continue;
                    }
                    self.findings.push(Finding {
                        severity: Severity::Erro,
                        place: name(door),
                        message: if fix.is_some() {
                            format!(
                                "A folha da porta bate em {}: invertendo o lado da dobradiça ela abre livre.",
                                name(by)
                            )
                        } else {
                            format!(
                                "A folha da porta bate em {}: mova a peça ou use porta de correr.",
                                name(by)
                            )
                        },
                        fix: fix.map(|right| {
                            serde_json::json!({
                                "tool": "update",
                                "items": [{"id": door.to_string(), "hinge_right": right}],
                            })
                        }),
                    });
                }
                Issue::Overlap(a, b) => {
                    let fix = nudge(home, b).or_else(|| nudge(home, a));
                    self.findings.push(Finding {
                        severity: Severity::Erro,
                        place: name(a),
                        message: match &fix {
                            Some(f) => format!(
                                "Ocupa o mesmo lugar que {}: movendo {} {} cm fica livre.",
                                name(b),
                                f["ids"][0].as_str().unwrap_or_default(),
                                cm(f["d"].as_f64().unwrap_or_default())
                            ),
                            None => format!(
                                "Ocupa o mesmo lugar que {}: não há lugar livre por perto, reorganize.",
                                name(b)
                            ),
                        },
                        fix: fix.map(without_distance),
                    });
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn rooms(&mut self) {
        let scene = self.scene;
        let home = scene.home;
        let wheel = self.profile.wheelchair;
        let spaces: Vec<RoomFacts> = scene
            .spaces
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let area = s.room.area();
                let short = short_side(&s.room.points);
                // Ceiling: the lowest wall around the room.
                let ceiling = home
                    .walls
                    .iter()
                    .filter(|w| {
                        near_outline(&s.room.points, w.start.midpoint_with(w.end), w.thickness)
                    })
                    .map(|w| w.height.min(w.height_at_end.unwrap_or(w.height)))
                    .reduce(f64::min)
                    .unwrap_or(home.wall_height);
                let glass: f64 = home
                    .furniture
                    .iter()
                    .filter(|f| {
                        f.visible
                            && f.opening.as_ref().is_some_and(|o| {
                                o.kind == OpeningKind::Window || f.catalog == "french-window"
                            })
                            && near_outline(&s.room.points, f.position, f.depth.max(15.0))
                    })
                    .map(|f| f.width * f.height)
                    .sum();
                let uses = s.units.iter().map(|&i| scene.units[i].what).collect();
                RoomFacts {
                    label: s.label(),
                    what: s.what,
                    area,
                    short,
                    ceiling,
                    glass,
                    uses,
                    index: k,
                }
            })
            .collect();
        for RoomFacts {
            label,
            what,
            area,
            short,
            ceiling,
            glass,
            uses,
            index: k,
        } in spaces
        {
            // Minimum areas: typical municipal codes, which vary by city.
            let minimum = match what {
                RoomUse::Bedroom => Some((70_000.0, 240.0)),
                RoomUse::Living => Some((100_000.0, 260.0)),
                RoomUse::Kitchen => Some((40_000.0, 150.0)),
                RoomUse::Bathroom => Some((22_000.0, 110.0)),
                RoomUse::Laundry => Some((20_000.0, 100.0)),
                RoomUse::Corridor => Some((0.0, if wheel { 120.0 } else { 90.0 })),
                _ => None,
            };
            if let Some((min_area, min_side)) = minimum {
                if area + 1.0 < min_area {
                    self.push(
                        Severity::Dica,
                        &label,
                        format!(
                            "{} m² para {}: códigos de obras costumam pedir ao menos {} m² (confira o do seu município).",
                            m2(area),
                            what.name(),
                            m2(min_area)
                        ),
                    );
                }
                if short + 0.5 < min_side {
                    let severity = if what == RoomUse::Corridor {
                        Severity::Alerta
                    } else {
                        Severity::Dica
                    };
                    self.push(
                        severity,
                        &label,
                        format!(
                            "Menor lado de {} cm; o usual para {} é pelo menos {} cm.",
                            cm(short),
                            what.name(),
                            cm(min_side)
                        ),
                    );
                }
            }
            // NBR 15575-1: 2,50 m in rooms for staying, 2,30 m in bathrooms,
            // kitchens, laundries and corridors.
            let min_ceiling = if what.long_stay() { 250.0 } else { 230.0 };
            if what != RoomUse::Other && ceiling + 0.5 < min_ceiling {
                self.push(
                    Severity::Alerta,
                    &label,
                    format!(
                        "Pé-direito de {} cm; a NBR 15575-1 pede no mínimo {} cm em {}.",
                        cm(ceiling),
                        cm(min_ceiling),
                        what.name()
                    ),
                );
            }
            // Daylight: 1/6 of the floor for rooms to stay, 1/8 elsewhere (codes vary).
            if what != RoomUse::Other && what != RoomUse::Corridor {
                let ratio = if what.long_stay() { 6.0 } else { 8.0 };
                if glass <= 0.0 {
                    let severity = if what.long_stay() {
                        Severity::Alerta
                    } else {
                        Severity::Dica
                    };
                    let what_to_do = if what == RoomUse::Bathroom {
                        "use janela basculante ou ventilação mecânica"
                    } else {
                        "precisa de janela para luz e ventilação"
                    };
                    self.push(severity, &label, format!("Sem janela: {what_to_do}."));
                } else if glass * ratio + 1.0 < area {
                    self.push(
                        Severity::Dica,
                        &label,
                        format!(
                            "Janelas somam {} m² para {} m² de piso; o usual é 1/{ratio:.0} do piso: {} m² (confira o código de obras).",
                            m2(glass),
                            m2(area),
                            m2(area / ratio)
                        ),
                    );
                }
            }
            // Minimum furniture, NBR 15575-1 informative annex.
            let has = |f: &dyn Fn(&Use) -> bool| uses.iter().any(f);
            let missing: Vec<&str> = match what {
                RoomUse::Bedroom => [
                    (has(&|u| matches!(u, Use::Bed(_) | Use::Crib)), "cama"),
                    (has(&|u| matches!(u, Use::Wardrobe)), "guarda-roupa"),
                ]
                .into_iter()
                .filter(|(ok, _)| !ok)
                .map(|(_, n)| n)
                .collect(),
                RoomUse::Kitchen => [
                    (has(&|u| matches!(u, Use::Stove)), "fogão ou cooktop"),
                    (has(&|u| matches!(u, Use::Fridge)), "geladeira"),
                    (has(&|u| matches!(u, Use::Sink)), "pia"),
                ]
                .into_iter()
                .filter(|(ok, _)| !ok)
                .map(|(_, n)| n)
                .collect(),
                RoomUse::Bathroom => [
                    (has(&|u| matches!(u, Use::Toilet)), "vaso sanitário"),
                    (has(&|u| matches!(u, Use::Basin)), "lavatório"),
                    (
                        has(&|u| matches!(u, Use::Shower | Use::Bathtub)),
                        "box ou banheira",
                    ),
                ]
                .into_iter()
                .filter(|(ok, _)| !ok)
                .map(|(_, n)| n)
                .collect(),
                _ => Vec::new(),
            };
            if !missing.is_empty() {
                self.push(
                    Severity::Dica,
                    &label,
                    format!(
                        "Falta {} para testar o uso de {}.",
                        missing.join(", "),
                        what.name()
                    ),
                );
            }
            if wheel && matches!(what, RoomUse::Bathroom | RoomUse::Kitchen) {
                let space = &scene.spaces[k];
                let turn = scene.turning_diameter(space);
                if turn + 1.0 < 150.0 {
                    self.push(
                        Severity::Alerta,
                        &label,
                        format!(
                            "Cabe um giro de {} cm; a NBR 9050 pede círculo livre de 150 cm para girar a cadeira de rodas.",
                            cm(turn)
                        ),
                    );
                }
            }
            if self.profile.elderly > 0 && what == RoomUse::Bathroom {
                self.push(
                    Severity::Dica,
                    &label,
                    "Morador idoso: barras de apoio junto ao vaso e no box, piso antiderrapante e box sem degrau (NBR 9050).",
                );
            }
        }
    }

    fn kitchen(&mut self) {
        let scene = self.scene;
        let stature = self.profile.stature.unwrap_or(165.0);
        // Kitchens, and living rooms with a kitchen in them.
        let cooks = |s: &&Space<'_>| {
            s.what == RoomUse::Kitchen
                || s.units
                    .iter()
                    .filter(|&&i| {
                        matches!(scene.units[i].what, Use::Fridge | Use::Stove | Use::Sink)
                    })
                    .count()
                    >= 2
        };
        for space in scene.spaces.iter().filter(cooks) {
            let label = space.label();
            let find = |w: fn(&Use) -> bool| {
                space
                    .units
                    .iter()
                    .map(|&i| &scene.units[i])
                    .find(|u| w(&u.what))
            };
            let points: Vec<Option<Point2>> = [
                find(|u| matches!(u, Use::Fridge)),
                find(|u| matches!(u, Use::Sink)),
                find(|u| matches!(u, Use::Stove)),
            ]
            .into_iter()
            .map(|u| u.map(|u| u.piece.position))
            .collect();
            if let [Some(fridge), Some(sink), Some(stove)] = points[..] {
                let legs = [
                    fridge.distance(sink),
                    sink.distance(stove),
                    stove.distance(fridge),
                ];
                let total: f64 = legs.iter().sum();
                if total > 700.0 {
                    self.push(
                        Severity::Dica,
                        &label,
                        format!(
                            "Triângulo geladeira–pia–fogão de {} cm; acima de ~660 cm cozinhar vira caminhada: aproxime os três.",
                            cm(total)
                        ),
                    );
                } else if total < 330.0 {
                    self.push(
                        Severity::Dica,
                        &label,
                        format!(
                            "Triângulo geladeira–pia–fogão de só {} cm: falta bancada entre eles para apoiar e preparar.",
                            cm(total)
                        ),
                    );
                }
                if sink.distance(stove) < 60.0 {
                    self.push(
                        Severity::Alerta,
                        &label,
                        "Pia e fogão colados: deixe ao menos 60 cm de bancada entre eles para preparo e segurança.",
                    );
                }
            }
            // Worktop height: about 10 to 15 cm below the elbow (63 % of stature).
            let ideal = stature * 0.63 - 12.0;
            for &i in &space.units {
                let u = &scene.units[i];
                if !matches!(u.what, Use::Sink | Use::Counter) {
                    continue;
                }
                let top = u.piece.height_range().1;
                if top < 70.0 {
                    continue;
                }
                if (top - ideal).abs() > 6.0 {
                    self.push(
                        Severity::Dica,
                        u.label(),
                        format!(
                            "Bancada a {} cm; para quem tem {} cm de altura o conforto fica perto de {} cm.",
                            cm(top),
                            cm(stature),
                            cm(ideal.round())
                        ),
                    );
                    break;
                }
            }
            // Wall cabinets: above the head at the counter, within reach.
            let reach = if self.profile.wheelchair {
                120.0
            } else {
                stature * 1.2
            };
            for &i in &space.units {
                let u = &scene.units[i];
                if u.what != Use::WallCabinet {
                    continue;
                }
                let (lo, hi) = u.piece.height_range();
                if lo < 135.0 {
                    self.push(
                        Severity::Alerta,
                        u.label(),
                        format!(
                            "Aéreo a {} cm do chão: abaixo de ~135 cm (45 cm sobre a bancada) a cabeça bate ao trabalhar.",
                            cm(lo)
                        ),
                    );
                }
                if hi > reach + 30.0 {
                    self.push(
                        Severity::Dica,
                        u.label(),
                        format!(
                            "Topo do aéreo a {} cm: a prateleira de cima fica fora do alcance (~{} cm); guarde ali o que se usa pouco.",
                            cm(hi),
                            cm(reach.round())
                        ),
                    );
                }
                break;
            }
        }
    }

    fn reach(&mut self) {
        if !self.profile.wheelchair {
            return;
        }
        let mut wrong = 0;
        for u in &self.scene.units {
            if !matches!(u.what, Use::Switch | Use::Outlet) {
                continue;
            }
            let center = u.piece.elevation + u.piece.height / 2.0;
            if !(40.0..=120.0).contains(&center) {
                wrong += 1;
            }
        }
        if wrong > 0 {
            self.push(
                Severity::Alerta,
                "Casa",
                format!(
                    "{wrong} interruptor(es)/tomada(s) fora da faixa de 40 a 120 cm de alcance de quem usa cadeira de rodas (NBR 9050)."
                ),
            );
        }
    }
}

/// What a room offers, measured once.
struct RoomFacts {
    label: String,
    what: RoomUse,
    /// cm².
    area: f64,
    short: f64,
    ceiling: f64,
    /// Window area, cm².
    glass: f64,
    uses: Vec<Use>,
    index: usize,
}

trait Midpoint {
    fn midpoint_with(self, other: Self) -> Self;
}

impl Midpoint for Point2 {
    fn midpoint_with(self, other: Self) -> Self {
        Self::new(self.x.midpoint(other.x), self.y.midpoint(other.y))
    }
}

fn piece_after(home: &Home, id: newera_core::FurnitureId) -> Option<&newera_core::Furniture> {
    home.furniture.iter().find(|f| f.id == id)
}

/// How many of a piece's sides rest against a wall (within 2 cm).
fn wall_contacts(home: &Home, piece: &newera_core::Furniture) -> usize {
    use geo::Contains;
    let outlines: Vec<_> = home
        .wall_outlines()
        .iter()
        .filter(|o| o.len() >= 3)
        .map(|o| scene::polygon(o))
        .collect();
    let (hw, hd) = (piece.width / 2.0, piece.depth / 2.0);
    [
        (0.0, -hd - 2.0),
        (0.0, hd + 2.0),
        (-hw - 2.0, 0.0),
        (hw + 2.0, 0.0),
    ]
    .into_iter()
    .filter(|&p| {
        let at = piece.to_plan(p);
        outlines
            .iter()
            .any(|o| o.contains(&geo::Point::new(at.x, at.y)))
    })
    .count()
}

/// Layout problems a piece (or any piece inside it, for a group) is part of.
fn troubles(home: &Home, id: newera_core::FurnitureId) -> usize {
    fn ids(f: &newera_core::Furniture, out: &mut Vec<newera_core::FurnitureId>) {
        out.push(f.id);
        for c in &f.children {
            ids(c, out);
        }
    }
    let mut mine = Vec::new();
    if let Some(top) = home.furniture.iter().find(|f| f.id == id) {
        ids(top, &mut mine);
    } else {
        mine.push(id);
    }
    newera_core::check_layout(home)
        .iter()
        .filter(|i| match i {
            Issue::Overlap(a, b) => mine.contains(a) || mine.contains(b),
            Issue::InWall(f, _) | Issue::OutsideRooms(f) => mine.contains(f),
            Issue::BlocksDoor { door, by } => mine.contains(door) || mine.contains(by),
        })
        .count()
}

/// The shortest slide (along the piece's own axes, up to 1,5 m) that leaves
/// it clear of every layout problem without leaving its room. Returns move
/// arguments plus the distance as `d`.
fn nudge(home: &Home, id: newera_core::FurnitureId) -> Option<serde_json::Value> {
    let top = home
        .furniture
        .iter()
        .find(|f| f.id == id || f.children.iter().any(|c| c.id == id))?;
    if top.is_opening() || !top.locks.movable {
        return None;
    }
    let room = home.rooms.iter().find(|r| {
        use geo::Contains;
        scene::polygon(&r.points).contains(&geo::Point::new(top.position.x, top.position.y))
    });
    let mut step = 5.0;
    while step <= 150.0 {
        for (lx, ly) in [(step, 0.0), (-step, 0.0), (0.0, step), (0.0, -step)] {
            let to = top.to_plan((lx, ly));
            let (dx, dy) = (to.x - top.position.x, to.y - top.position.y);
            let mut moved = home.clone();
            let piece = moved.furniture.iter_mut().find(|f| f.id == top.id)?;
            piece.translate(dx, dy);
            let center = piece.position;
            let stays = room.is_none_or(|r| {
                use geo::Contains;
                scene::polygon(&r.points).contains(&geo::Point::new(center.x, center.y))
            });
            let on_walls = wall_contacts(home, top)
                <= moved
                    .furniture
                    .iter()
                    .find(|f| f.id == top.id)
                    .map_or(0, |f| wall_contacts(&moved, f));
            if stays && on_walls && troubles(&moved, id) == 0 {
                let round = |v: f64| (v * 10.0).round() / 10.0;
                return Some(serde_json::json!({
                    "tool": "move",
                    "ids": [top.id.to_string()],
                    "dx": round(dx),
                    "dy": round(dy),
                    "d": step,
                }));
            }
        }
        step += 5.0;
    }
    None
}

/// Moves a piece `dist` cm along `dir` if that leaves it trouble-free and
/// in its room: move arguments.
fn push_away(
    home: &Home,
    id: newera_core::FurnitureId,
    dir: (f64, f64),
    dist: f64,
) -> Option<serde_json::Value> {
    let top = home.furniture.iter().find(|f| f.id == id)?;
    if top.is_opening() || !top.locks.movable {
        return None;
    }
    let len = dir.0.hypot(dir.1).max(1e-9);
    let (dx, dy) = (dir.0 / len * dist, dir.1 / len * dist);
    let mut moved = home.clone();
    let piece = moved.furniture.iter_mut().find(|f| f.id == id)?;
    let before = piece.position;
    piece.translate(dx, dy);
    let after = piece.position;
    let room_of = |p: Point2| {
        use geo::Contains;
        home.rooms
            .iter()
            .position(|r| scene::polygon(&r.points).contains(&geo::Point::new(p.x, p.y)))
    };
    let round = |v: f64| (v * 10.0).round() / 10.0;
    let stays_on_walls =
        wall_contacts(home, top) <= wall_contacts(&moved, piece_after(&moved, id)?);
    (stays_on_walls
        && room_of(before) == room_of(after)
        && troubles(&moved, id) <= troubles(home, id))
    .then(|| {
        serde_json::json!({
            "tool": "move",
            "ids": [id.to_string()],
            "dx": round(dx),
            "dy": round(dy),
        })
    })
}

fn without_distance(mut fix: serde_json::Value) -> serde_json::Value {
    if let Some(map) = fix.as_object_mut() {
        map.remove("d");
    }
    fix
}

/// Whether `p` lies on the outline of a polygon, within `tolerance` cm.
fn near_outline(points: &[Point2], p: Point2, tolerance: f64) -> bool {
    (0..points.len())
        .any(|k| p.distance_to_segment(points[k], points[(k + 1) % points.len()]) <= tolerance)
}

/// Shorter side of the smallest rectangle around a polygon, cm.
fn short_side(points: &[Point2]) -> f64 {
    use geo::MinimumRotatedRect;
    let Some(rect) = scene::polygon(points).minimum_rotated_rect() else {
        return 0.0;
    };
    let c: Vec<_> = rect.exterior().coords().copied().collect();
    if c.len() < 3 {
        return 0.0;
    }
    let a = (c[1].x - c[0].x).hypot(c[1].y - c[0].y);
    let b = (c[2].x - c[1].x).hypot(c[2].y - c[1].y);
    a.min(b)
}

/// Reviews the current storey of `home` for the people in `profile`.
pub fn review(home: &Home, profile: &Profile) -> Report {
    let view = home.level_view(home.current_level());
    let scene = Scene::new(&view);
    let mut review = Review {
        scene: &scene,
        profile,
        findings: Vec::new(),
    };
    let capacity = review.capacity();
    review.occupancy(&capacity);
    review.clearances();
    review.doors();
    review.rooms();
    review.kitchen();
    review.reach();
    // The same finding on a row of modules is one finding about all of them.
    let mut findings: Vec<Finding> = Vec::new();
    for f in review.findings {
        match findings.iter_mut().find(|g| {
            g.severity == f.severity && g.message == f.message && g.fix.is_none() && f.fix.is_none()
        }) {
            Some(same) => {
                same.place.push_str(", ");
                same.place.push_str(&f.place);
            }
            None => findings.push(f),
        }
    }
    findings.sort_by_key(|f| f.severity);
    let penalty: u32 = findings
        .iter()
        .map(|f| match f.severity {
            Severity::Erro => 12,
            Severity::Alerta => 5,
            Severity::Dica => 1,
        })
        .sum();
    Report {
        score: 100u32.saturating_sub(penalty),
        capacity,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use newera_core::{Furniture, FurnitureId, Room, RoomId, Wall, WallId};

    use super::*;

    fn square(home: &mut Home, name: &str, w: f64, d: f64) {
        let corners = [(0.0, 0.0), (w, 0.0), (w, d), (0.0, d)];
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            let mut wall = Wall::new(
                WallId(k as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            );
            wall.thickness = 15.0;
            wall.height = 260.0;
            home.walls.push(wall);
        }
        let inner = [
            (7.5, 7.5),
            (w - 7.5, 7.5),
            (w - 7.5, d - 7.5),
            (7.5, d - 7.5),
        ];
        home.rooms.push(Room::new(
            RoomId(10),
            name,
            inner.iter().map(|p| Point2::new(p.0, p.1)).collect(),
        ));
    }

    fn piece(
        id: u64,
        catalog: &str,
        at: (f64, f64),
        size: (f64, f64, f64),
        angle: f64,
    ) -> Furniture {
        Furniture {
            id: FurnitureId(id),
            catalog: catalog.into(),
            name: catalog.into(),
            position: Point2::new(at.0, at.1),
            angle,
            width: size.0,
            depth: size.1,
            height: size.2,
            ..Furniture::default()
        }
    }

    fn says(report: &Report, severity: Severity, text: &str) -> bool {
        report
            .findings
            .iter()
            .any(|f| f.severity == severity && f.message.contains(text))
    }

    #[test]
    fn a_bedroom_is_checked_for_its_people_and_the_walk_around_the_bed() {
        let mut home = Home::default();
        square(&mut home, "Quarto", 330.0, 330.0);
        // Double bed, head on the top wall (front toward +y), centered.
        home.furniture.push(piece(
            20,
            "bed-double",
            (165.0, 7.5 + 104.0),
            (158.0, 208.0, 55.0),
            0.0,
        ));
        home.furniture.push(piece(
            21,
            "wardrobe",
            (165.0, 322.5 - 30.0),
            (180.0, 60.0, 220.0),
            180.0,
        ));
        let report = review(&home, &Profile::default());
        // 315 − 158 = 157 cm → 78,5 cm each side; foot: 322,5 − 60 − 215,5 = 47 cm.
        assert!(
            !says(&report, Severity::Alerta, "à esquerda"),
            "{report:#?}"
        );
        assert!(
            says(&report, Severity::Dica, "passagem aos pés"),
            "{report:#?}"
        );
        assert_eq!(report.capacity.beds, 2);
        // Four people, one double bed and no bathroom.
        let crowded = review(
            &home,
            &Profile {
                occupants: 4,
                ..Profile::default()
            },
        );
        assert!(says(&crowded, Severity::Erro, "faltam 2"), "{crowded:#?}");
        assert!(says(&crowded, Severity::Erro, "Nenhum banheiro"));
        assert!(crowded.score < report.score);
        // Pushed against the left wall, the couple loses a side.
        home.furniture[0].position.x = 7.5 + 79.0 + 20.0;
        let pushed = review(&home, &Profile::default());
        assert!(
            says(&pushed, Severity::Alerta, "20 cm livres à"),
            "{pushed:#?}"
        );
        assert!(says(&pushed, Severity::Alerta, "afaste 30 cm"));
        let fix = pushed
            .findings
            .iter()
            .find_map(|f| f.fix.clone())
            .expect("a move that frees the side");
        assert_eq!(fix["tool"], "move");
        assert_eq!(fix["dx"], 30.0, "{fix}");
    }

    #[test]
    fn kitchens_bathrooms_and_doors_follow_the_references() {
        let mut home = Home::default();
        square(&mut home, "Cozinha", 300.0, 240.0);
        home.furniture.push(piece(
            20,
            "stove",
            (100.0, 7.5 + 31.0),
            (60.0, 62.0, 90.0),
            0.0,
        ));
        home.furniture.push(piece(
            21,
            "sink-counter",
            (200.0, 7.5 + 30.0),
            (120.0, 60.0, 90.0),
            0.0,
        ));
        // An island 60 cm in front of the stove.
        home.furniture.push(piece(
            22,
            "kitchen-island",
            (120.0, 69.5 + 60.0 + 45.0),
            (180.0, 90.0, 90.0),
            0.0,
        ));
        let mut door = piece(23, "door", (250.0, 240.0), (64.0, 15.0, 210.0), 0.0);
        door.opening = Some(newera_core::Opening::default());
        home.furniture.push(door);
        let report = review(&home, &Profile::default());
        assert!(
            says(&report, Severity::Alerta, "60 cm livres à frente"),
            "{report:#?}"
        );
        assert!(says(&report, Severity::Alerta, "NBR 15575-1"));
        assert!(
            says(&report, Severity::Dica, "Falta geladeira"),
            "{report:#?}"
        );
        assert!(says(&report, Severity::Dica, "Sem janela"), "{report:#?}");
        assert!(
            !says(&report, Severity::Alerta, "Vão livre"),
            "60 cm clear is enough without a wheelchair"
        );
        // A dresser in the door's swing: the review offers a checked way out.
        let mut blocked = home.clone();
        blocked.furniture.retain(|f| f.id != FurnitureId(22));
        // The door swings into the room.
        blocked
            .furniture
            .iter_mut()
            .find(|f| f.id == FurnitureId(23))
            .unwrap()
            .angle = 180.0;
        blocked.furniture.push(piece(
            24,
            "dresser",
            (228.0, 232.5 - 25.0 - 40.0),
            (60.0, 50.0, 85.0),
            180.0,
        ));
        let report = review(&blocked, &Profile::default());
        let door = report
            .findings
            .iter()
            .find(|f| f.message.contains("A folha da porta bate"))
            .unwrap_or_else(|| panic!("{report:#?}"));
        let fix = door.fix.clone().expect("a fix");
        let mut fixed = blocked.clone();
        match fix["tool"].as_str() {
            Some("update") => {
                let d = fixed
                    .furniture
                    .iter_mut()
                    .find(|f| f.id == FurnitureId(23))
                    .unwrap();
                d.opening.as_mut().unwrap().hinge_right = fix["items"][0]["hinge_right"] == true;
            }
            Some("move") => {
                let id = fix["ids"][0].as_str().unwrap().to_owned();
                let f = fixed
                    .furniture
                    .iter_mut()
                    .find(|f| f.id.to_string() == id)
                    .unwrap();
                f.translate(fix["dx"].as_f64().unwrap(), fix["dy"].as_f64().unwrap());
            }
            _ => panic!("{fix}"),
        }
        assert!(
            !review(&fixed, &Profile::default())
                .findings
                .iter()
                .any(|f| f.message.contains("A folha da porta bate")),
            "{fix}"
        );
        // A wheelchair user needs 150 cm to turn and 80 cm doors.
        let wheel = review(
            &home,
            &Profile {
                wheelchair: true,
                ..Profile::default()
            },
        );
        assert!(
            says(&wheel, Severity::Alerta, "NBR 9050 pede 80 cm"),
            "{wheel:#?}"
        );
        assert!(
            says(&wheel, Severity::Alerta, "giro de cadeira"),
            "{wheel:#?}"
        );
    }
}
