//! Ergonomics and habitability review.
//!
//! Nothing here blocks anything. A review reads a drawing and says what it
//! finds; the drawing is the user's, and so is the decision. Somebody laying
//! out a house to learn, to try an idea or to see what it would look like is
//! not stopped by a gas standard, and neither is somebody who knows exactly
//! what they are doing and means it. Findings that were looked at and settled
//! can be accepted by key — see [`Finding::accepted`] — so that a plan which
//! is right reaches zero without anything being swept away.
//!
//! Given who lives in a home — how many people, children, elderly, a
//! wheelchair user — this reads the plan the way a careful architect would:
//! is there room to walk beside the bed and in front of the stove, do the
//! beds, seats, bathrooms and wardrobes serve everyone, does the kitchen
//! work, can a wheelchair turn in the bathroom. Each finding says what is
//! wrong with its numbers, what to change, and which reference it follows.
//!
//! References are Brazilian where one exists and they travel as data, not as
//! prose: a finding carries the short code of a [`newera_core::Standard`], and
//! the report resolves the codes it used once. That is what lets the reply say
//! which edition it followed, lets the interface link it, and lets the
//! reliability tier decide how much a finding may accuse — a survey never
//! accuses, and a figure we could not confirm at the source may warn but never
//! errs. Municipal codes vary by city: give [`Profile::city`] and they judge,
//! omit it and they only advise.

// Counts of people and pieces are tiny; cm fit in any float.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

mod scene;

use newera_core::standards::{self, Confidence, Standard, Tier};
use newera_core::{Home, OpeningKind, Point2};
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
    /// City whose building code applies, e.g. `sao-paulo`. Without it,
    /// municipal rules only advise, because their numbers vary by city and,
    /// against a standard, the more restrictive one wins.
    pub city: Option<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            occupants: 2,
            children: 0,
            elderly: 0,
            wheelchair: false,
            stature: None,
            city: None,
        }
    }
}

/// Where the project keeps who lives there, as JSON in its properties.
pub const PEOPLE: &str = "ergonomics:people";

impl Profile {
    /// The people the project was last reviewed for, or the defaults.
    ///
    /// A dry run scores a change for someone, and a score for two occupants
    /// cannot be compared with the 91 of a review conducted for three: the
    /// people belong to the project, like its city.
    #[must_use]
    pub fn of(home: &Home) -> Self {
        home.properties
            .get(PEOPLE)
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default()
    }
}

/// How much a finding matters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Something can't be used as drawn.
    #[default]
    Erro,
    /// Below the reference: works badly.
    Alerta,
    /// Would be better.
    Dica,
}

impl Severity {
    /// The most a source of this tier may claim on its own: what obliges can
    /// accuse, what merely describes cannot.
    const fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::A => Self::Erro,
            Tier::B | Tier::D => Self::Alerta,
            Tier::C | Tier::E => Self::Dica,
        }
    }
}

/// One thing to look at.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    /// Room or piece it is about, e.g. `Quarto r5` or `Cama de casal f12`.
    pub place: String,
    /// What is wrong, with the numbers and what to do.
    pub message: String,
    /// Code of the [`newera_core::Standard`] behind it, resolved once in
    /// [`Report::refs`]. The citation lives here, not inside the sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<&'static str>,
    /// A checked change that solves it, as MCP tool arguments:
    /// `{"tool":"move","ids":["f12"],"dx":-20,"dy":0}` or
    /// `{"tool":"update","items":[{"id":"f3","hinge_right":true}]}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<serde_json::Value>,
    /// What the score would gain if this one went away. A score that moves
    /// without saying why is a number nobody can act on.
    #[serde(default)]
    pub weight: u32,
    /// Name to accept it by, stable across runs: the rule and the place.
    pub key: String,
    /// Looked at and accepted, with the reason given. It still shows — a
    /// plan where a real finding is silently dropped is worse than one that
    /// carries it — but it no longer costs anything in the score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted: Option<String>,
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
    /// The sources the findings stand on, each one once.
    pub refs: Vec<&'static Standard>,
}

/// Wardrobe front per adult, cm (common practice; children count half).
const WARDROBE_PER_ADULT: f64 = 60.0;

/// Alexander's pattern 184, in centimeters: 12 ft of counter in total, no
/// stretch under 4 ft, and no pair of the four elements over 10 ft apart.
const ALEXANDER_TOTAL: f64 = 366.0;
const ALEXANDER_RUN: f64 = 122.0;
const ALEXANDER_PAIR: f64 = 305.0;

/// Narrowest kitchen a financed unit is delivered with, cm.
const MCMV_KITCHEN_WIDTH: f64 = 180.0;

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

/// Why a clearance is required, and the source behind it. A plain string
/// still works for the clearances that are common practice rather than a
/// published rule.
#[derive(Debug, Clone, Copy)]
struct Why(&'static str, Option<&'static str>);

impl From<&'static str> for Why {
    fn from(text: &'static str) -> Self {
        Self(text, None)
    }
}

impl From<(&'static str, &'static str)> for Why {
    fn from((text, code): (&'static str, &'static str)) -> Self {
        Self(text, Some(code))
    }
}

struct Review<'s, 'a> {
    scene: &'s Scene<'a>,
    profile: &'s Profile,
    findings: Vec<Finding>,
    /// Glass that lights and airs each room, shared across rooms open to
    /// each other — see [`glass_by_room`].
    glass: std::collections::BTreeMap<newera_core::RoomId, f64>,
}

impl Review<'_, '_> {
    fn push(&mut self, severity: Severity, place: impl Into<String>, message: impl Into<String>) {
        self.findings.push(Finding {
            severity,
            place: place.into(),
            message: message.into(),
            reference: None,
            fix: None,
            ..Finding::default()
        });
    }

    /// The same, standing on a published source. A tier E survey says what
    /// people do, never what they should do, so it raises nothing; a figure
    /// marked to confirm at the source may warn, never accuse.
    fn push_ref(
        &mut self,
        severity: Severity,
        place: impl Into<String>,
        message: impl Into<String>,
        code: &'static str,
    ) {
        let Some(source) = standards::standard(code) else {
            debug_assert!(false, "unknown standard `{code}`");
            self.push(severity, place, message);
            return;
        };
        if source.tier == Tier::E {
            return;
        }
        // A finding never claims more than its source can: the tier sets the
        // ceiling, and a figure we could not confirm may warn, never accuse.
        let mut severity = severity.max(Severity::for_tier(source.tier));
        if source.confidence == Confidence::ConfirmBeforeUse {
            severity = severity.max(Severity::Alerta);
        }
        self.findings.push(Finding {
            severity,
            place: place.into(),
            message: message.into(),
            reference: Some(source.code),
            fix: None,
            ..Finding::default()
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
                self.push_ref(
                    Severity::Alerta,
                    home,
                    format!(
                        "{} moradores por dormitório: acima de 3 é adensamento excessivo; são necessários {} dormitórios.",
                        cm(per),
                        people.div_ceil(3)
                    ),
                    "ibge-adensamento",
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

    /// Circulation around pieces: the informative annex of NBR 15575-1 (50 cm
    /// between furniture and walls, 85 cm in front of kitchen equipment) and
    /// common practice for the rest.
    #[allow(clippy::too_many_lines)]
    fn clearances(&mut self) {
        let scene = self.scene;
        let wheel = self.profile.wheelchair;
        for (i, u) in scene.units.iter().enumerate() {
            // Above the head nobody walks in front of it: a crown moulding at
            // 272 cm is passed under, not around.
            if u.piece.height_range().0 >= HEADROOM {
                continue;
            }
            let label = u.label();
            let need = |side: Side, min: f64, span: (f64, f64), severity: Severity, what: Why| {
                let (free, blocker, tight) = scene.free_along(i, side, min + 1.0, span, min);
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
                            let piece = scene.units[i].frame();
                            let way = piece.to_plan((0.0, 1.0));
                            let dir = (way.x - piece.position.x, way.y - piece.position.y);
                            push_away(scene, j, dir, short.ceil())
                        }),
                        Side::Left | Side::Right => {
                            let (other, sign) = if side == Side::Left {
                                (Side::Right, 1.0)
                            } else {
                                (Side::Left, -1.0)
                            };
                            let spare = scene.free(i, other, short + min + 1.0, span);
                            (spare - short + 0.5 >= min).then(|| {
                                let piece = scene.units[i].frame();
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
                    let Why(reason, reference) = what;
                    // How much of the side is that narrow: a corner taken by
                    // a nightstand is not a wardrobe that does not open.
                    let frame = scene.units[i].frame();
                    let whole = match side {
                        Side::Front => frame.width,
                        Side::Left | Side::Right => frame.depth,
                    } * (span.1 - span.0);
                    let stretch = if tight + 2.0 < whole - 4.0 {
                        format!(" em {} dos {} cm", cm(tight), cm(whole))
                    } else {
                        String::new()
                    };
                    Some(Finding {
                        severity,
                        place: label.clone(),
                        message: format!(
                            "{} cm livres {where_}{stretch} ({reason}: mínimo {} cm); {advice}.",
                            cm(free),
                            cm(min)
                        ),
                        reference,
                        fix,
                        ..Finding::default()
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
            let kitchen_source: Why = if wheel {
                ("giro de cadeira de rodas", "nbr9050").into()
            } else {
                ("circulação diante de bancada e equipamentos", "nbr15575g").into()
            };
            match u.what {
                Use::Bed(n) => {
                    let side_min = if wheel { 90.0 } else { 50.0 };
                    let why: Why = if wheel {
                        ("transferência da cadeira", "nbr9050").into()
                    } else {
                        ("circulação ao lado da cama", "nbr15575g").into()
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
                        "passagem aos pés da cama".into(),
                    ));
                }
                Use::Crib => found.extend(need(
                    Side::Front,
                    50.0,
                    whole,
                    Severity::Dica,
                    "acesso ao berço".into(),
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
                            "abrir as portas e circular diante do guarda-roupa; portas de correr pedem menos".into()
                        } else {
                            "abrir as portas e circular diante do guarda-roupa".into()
                        },
                    ));
                }
                Use::Dresser => found.extend(need(
                    Side::Front,
                    70.0,
                    whole,
                    Severity::Dica,
                    "abrir gavetas e ficar diante delas".into(),
                )),
                // The cabinets under a countertop answer for it and for what is set in it.
                Use::Fridge | Use::Stove | Use::Sink | Use::Counter | Use::Appliance
                    if !scene.countertop(i) && !scene.embedded_item(i) =>
                {
                    // A blind corner's panel sits behind the other run: only its door counts.
                    let blind = |key: &str| {
                        u.params
                            .as_ref()
                            .and_then(|p| p[key].as_f64())
                            .unwrap_or(0.0)
                            / u.piece.width.max(1.0)
                    };
                    let span = if u.piece.mirrored {
                        (0.05 + blind("blind_right"), 0.95 - blind("blind_left"))
                    } else {
                        (0.05 + blind("blind_left"), 0.95 - blind("blind_right"))
                    };
                    if span.1 > span.0 {
                        found.extend(need(
                            Side::Front,
                            kitchen_front,
                            span,
                            Severity::Alerta,
                            kitchen_source,
                        ));
                    }
                }
                Use::Island => {
                    found.extend(need(
                        Side::Front,
                        90.0,
                        whole,
                        Severity::Alerta,
                        "circulação em volta da ilha".into(),
                    ));
                }
                Use::Toilet => found.extend(need(
                    Side::Front,
                    if wheel { 120.0 } else { 60.0 },
                    whole,
                    Severity::Alerta,
                    if wheel {
                        Why("área de transferência", Some("nbr9050"))
                    } else {
                        Why("uso do vaso", None)
                    },
                )),
                Use::Basin => found.extend(need(
                    Side::Front,
                    if wheel { 120.0 } else { 60.0 },
                    whole,
                    Severity::Alerta,
                    "uso do lavatório".into(),
                )),
                Use::Shower | Use::Bathtub => found.extend(need(
                    Side::Front,
                    60.0,
                    (0.2, 0.8),
                    Severity::Dica,
                    "entrar e sair do box".into(),
                )),
                Use::Washer | Use::LaundrySink => found.extend(need(
                    Side::Front,
                    60.0,
                    whole,
                    Severity::Alerta,
                    "uso da área de serviço".into(),
                )),
                Use::Desk => found.extend(need(
                    Side::Front,
                    75.0,
                    (0.2, 0.8),
                    Severity::Dica,
                    "cadeira e levantar-se da mesa".into(),
                )),
                Use::DiningTable(_) => {
                    // The sides people sit on: where chairs are, or the front
                    // and both ends of a table drawn without chairs.
                    let piece = u.piece;
                    let chairs: Vec<(f64, f64)> = scene
                        .units
                        .iter()
                        .filter(|c| matches!(c.what, Use::Chair | Use::Stool))
                        .map(|c| piece.to_local(c.piece.position))
                        .filter(|(x, y)| {
                            x.abs() < piece.width / 2.0 + 70.0 && y.abs() < piece.depth / 2.0 + 70.0
                        })
                        .collect();
                    let (hw, hd) = (piece.width / 2.0, piece.depth / 2.0);
                    for side in [Side::Front, Side::Left, Side::Right] {
                        let seated = chairs.is_empty()
                            || chairs.iter().any(|&(x, y)| match side {
                                Side::Front => y > hd - 10.0,
                                Side::Left => x < -hw + 10.0,
                                Side::Right => x > hw - 10.0,
                            });
                        if seated {
                            found.extend(need(
                                side,
                                75.0,
                                (0.2, 0.8),
                                Severity::Alerta,
                                "puxar a cadeira e sentar".into(),
                            ));
                        }
                    }
                }
                Use::DiningSet(_) => {
                    for side in [Side::Front, Side::Left, Side::Right] {
                        found.extend(need(
                            side,
                            40.0,
                            (0.2, 0.8),
                            Severity::Alerta,
                            "passar atrás das cadeiras".into(),
                        ));
                    }
                }
                Use::Sofa(_) | Use::Armchair => {
                    found.extend(need(
                        Side::Front,
                        35.0,
                        (0.2, 0.8),
                        Severity::Dica,
                        "pernas e passagem diante do sofá".into(),
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
                self.push_ref(
                    Severity::Alerta,
                    label,
                    format!(
                        "Vão livre de cerca de {} cm: cadeira de rodas e andador pedem 80 cm livres; use porta de 90 cm.",
                        cm(clear)
                    ),
                    "nbr9050",
                );
            }
        }
        let scene = self.scene;
        for (door, unit) in scene.door_hits() {
            let door_name = format!("{} {}", door.name, door.id);
            let by = scene.units[unit].label();
            // Would the leaf clear it swinging from the other jamb?
            let flip = door
                .opening
                .as_ref()
                .filter(|o| o.leaves < 2 && !o.sliding && o.sashes.is_empty())
                .and_then(|o| {
                    let mut flipped = door.clone();
                    let right = !o.hinge_right;
                    flipped.opening.as_mut()?.hinge_right = right;
                    let swing = newera_core::door_swing(&flipped)?;
                    // Clear by this review's reading and by the rule
                    // check_layout reports blocks_door with: a basin the
                    // review reads as built in still stops a real leaf.
                    (scene.swing_clear(&swing)
                        && newera_core::door_blocked_by(scene.home, &flipped).is_empty())
                    .then_some(right)
                });
            if let Some(right) = flip {
                self.findings.push(Finding {
                    severity: Severity::Erro,
                    place: door_name,
                    message: format!(
                        "A folha da porta bate em {by}: invertendo o lado da dobradiça ela abre livre."
                    ),
                    reference: None,
                    fix: Some(serde_json::json!({
                        "tool": "update",
                        "items": [{"id": door.id.to_string(), "hinge_right": right}],
                    })),
                    ..Finding::default()
                });
                continue;
            }
            let moved = nudge(scene, unit);
            self.findings.push(Finding {
                severity: Severity::Erro,
                place: door_name,
                message: match &moved {
                    Some(m) => format!(
                        "A folha da porta bate em {by}: movendo a peça {} cm a porta abre livre.",
                        cm(m["d"].as_f64().unwrap_or_default())
                    ),
                    None => format!(
                        "A folha da porta bate em {by}: mova a peça ou use porta de correr."
                    ),
                },
                reference: None,
                fix: moved.map(without_distance),
                ..Finding::default()
            });
        }
        for (a, b) in scene.overlaps() {
            let fix = nudge(scene, b).or_else(|| nudge(scene, a));
            self.findings.push(Finding {
                severity: Severity::Erro,
                place: scene.units[a].label(),
                message: match &fix {
                    Some(f) => format!(
                        "Ocupa o mesmo lugar que {}: movendo {} {} cm fica livre.",
                        scene.units[b].label(),
                        f["ids"][0].as_str().unwrap_or_default(),
                        cm(f["d"].as_f64().unwrap_or_default())
                    ),
                    None => format!(
                        "Ocupa o mesmo lugar que {}: não há lugar livre por perto, reorganize.",
                        scene.units[b].label()
                    ),
                },
                reference: None,
                fix: fix.map(without_distance),
                ..Finding::default()
            });
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
                let glass: f64 = self.glass.get(&s.room.id).copied().unwrap_or_default();
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
                    self.push_ref(
                        Severity::Dica,
                        &label,
                        format!(
                            "{} m² para {}: códigos de obras costumam pedir ao menos {} m² (confira o do seu município).",
                            m2(area),
                            what.name(),
                            m2(min_area)
                        ),
                        "coe-municipal",
                    );
                }
                if short + 0.5 < min_side {
                    let severity = if what == RoomUse::Corridor {
                        Severity::Alerta
                    } else {
                        Severity::Dica
                    };
                    self.push_ref(
                        severity,
                        &label,
                        format!(
                            "Menor lado de {} cm; o usual para {} é pelo menos {} cm.",
                            cm(short),
                            what.name(),
                            cm(min_side)
                        ),
                        "coe-municipal",
                    );
                }
            }
            // NBR 15575-1: 2,50 m in rooms for staying, 2,30 m in bathrooms,
            // kitchens, laundries and corridors.
            let min_ceiling = if what.long_stay() { 250.0 } else { 230.0 };
            if what != RoomUse::Other && ceiling + 0.5 < min_ceiling {
                self.push_ref(
                    Severity::Alerta,
                    &label,
                    format!(
                        "Pé-direito de {} cm; o mínimo é {} cm em {}.",
                        cm(ceiling),
                        cm(min_ceiling),
                        what.name()
                    ),
                    "nbr15575",
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
                    self.push_ref(
                        Severity::Dica,
                        &label,
                        format!(
                            "Janelas somam {} m² para {} m² de piso; o usual é 1/{ratio:.0} do piso: {} m² (confira o código de obras).",
                            m2(glass),
                            m2(area),
                            m2(area / ratio)
                        ),
                        "coe-municipal",
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
                self.push_ref(
                    Severity::Dica,
                    &label,
                    format!(
                        "Falta {} para testar o uso de {}.",
                        missing.join(", "),
                        what.name()
                    ),
                    "nbr15575g",
                );
            }
            if wheel && matches!(what, RoomUse::Bathroom | RoomUse::Kitchen) {
                let space = &scene.spaces[k];
                let turn = scene.turning_diameter(space);
                if turn + 1.0 < 150.0 {
                    self.push_ref(
                        Severity::Alerta,
                        &label,
                        format!(
                            "Cabe um giro de {} cm; são necessários 150 cm livres para girar a cadeira de rodas.",
                            cm(turn)
                        ),
                        "nbr9050",
                    );
                }
            }
            if self.profile.elderly > 0 && what == RoomUse::Bathroom {
                self.push_ref(
                    Severity::Dica,
                    &label,
                    "Morador idoso: barras de apoio junto ao vaso e no box, piso antiderrapante e box sem degrau.",
                    "nbr9050",
                );
            }
        }
    }

    /// The kitchen, read against the sources that founded the subject: the
    /// work triangle, Alexander's counter lengths, Blum's five zones, the
    /// electrical and gas standards that make it legal, and what the
    /// laboratory knows about pulling cooking pollutants out of the air.
    #[allow(clippy::too_many_lines)]
    fn kitchen(&mut self) {
        let scene = self.scene;
        let stature = self.profile.stature.unwrap_or(165.0);
        let city = self.profile.city.as_deref().and_then(standards::municipal);
        // Only judge the electrical layout of a plan that has one.
        let has_electrical = scene
            .units
            .iter()
            .any(|u| matches!(u.what, Use::Outlet | Use::Switch));
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
        let cooking: Vec<usize> = scene
            .spaces
            .iter()
            .enumerate()
            .filter(|(_, s)| cooks(s))
            .map(|(k, _)| k)
            .collect();
        for k in cooking {
            let space = &scene.spaces[k];
            let label = space.label();
            let find = |w: fn(&Use) -> bool| {
                space
                    .units
                    .iter()
                    .map(|&i| &scene.units[i])
                    .find(|u| w(&u.what))
            };
            let any = |w: fn(&Use) -> bool| find(w).is_some();
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
                    self.push_ref(
                        Severity::Dica,
                        &label,
                        format!(
                            "Triângulo geladeira–pia–fogão de {} cm; acima de ~660 cm cozinhar vira caminhada: aproxime os três.",
                            cm(total)
                        ),
                        "gilbreth-triangulo",
                    );
                } else if total < 330.0 {
                    self.push_ref(
                        Severity::Dica,
                        &label,
                        format!(
                            "Triângulo geladeira–pia–fogão de só {} cm: falta bancada entre eles para apoiar e preparar.",
                            cm(total)
                        ),
                        "gilbreth-triangulo",
                    );
                }
                if sink.distance(stove) < 60.0 {
                    self.push_ref(
                        Severity::Alerta,
                        &label,
                        "Pia e fogão colados: deixe ao menos 60 cm de bancada entre eles para preparo e segurança.",
                        "nkba",
                    );
                }
            }
            // Worktop height: about 10 to 15 cm below the elbow (63 % of
            // stature). The kitchen has one counter height, and it is the one
            // most of the stone is at — an appliance sitting 6 cm lower under
            // the same stone is not a counter to measure, and reading it as
            // one accused a correct kitchen in a real session.
            let ideal = stature * 0.63 - 12.0;
            let mut tops: Vec<(f64, usize)> = space
                .units
                .iter()
                .filter(|&&i| {
                    let u = &scene.units[i];
                    matches!(u.what, Use::Sink | Use::Counter)
                        && !scene.embedded(i)
                        && !scene.thin(i)
                        && u.piece.height_range().1 >= 70.0
                })
                .map(|&i| (scene.units[i].piece.height_range().1, i))
                .collect();
            tops.sort_by(|a, b| a.0.total_cmp(&b.0));
            if let Some(&(top, which)) = tops.get(tops.len() / 2)
                && (top - ideal).abs() > 6.0
            {
                self.push_ref(
                    Severity::Dica,
                    scene.units[which].label(),
                    format!(
                        "Bancada a {} cm; para quem tem {} cm de altura o conforto fica perto de {} cm.",
                        cm(top),
                        cm(stature),
                        cm(ideal.round())
                    ),
                    "blum-zonas",
                );
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
                    self.push_ref(
                        Severity::Alerta,
                        u.label(),
                        format!(
                            "Aéreo a {} cm do chão: abaixo de ~135 cm (45 cm sobre a bancada) a cabeça bate ao trabalhar.",
                            cm(lo)
                        ),
                        "nkba",
                    );
                }
                if hi > reach + 30.0 {
                    self.push_ref(
                        Severity::Dica,
                        u.label(),
                        format!(
                            "Topo do aéreo a {} cm: a prateleira de cima fica fora do alcance (~{} cm); guarde ali o que se usa pouco.",
                            cm(hi),
                            cm(reach.round())
                        ),
                        if self.profile.wheelchair {
                            "nbr9050"
                        } else {
                            "panero-zelnik"
                        },
                    );
                }
                break;
            }
            // --- Alexander, pattern 184: is there counter, and is it whole? ---
            let runs = counter_runs(scene, space);
            let total: f64 = runs.iter().sum();
            if !runs.is_empty() {
                if total + 1.0 < ALEXANDER_TOTAL {
                    self.push_ref(
                        Severity::Dica,
                        &label,
                        format!(
                            "{} cm de bancada livre fora de pia, fogão e geladeira; abaixo de {} cm falta onde pousar as coisas do preparo (mesa solta também conta).",
                            cm(total),
                            cm(ALEXANDER_TOTAL)
                        ),
                        "alexander184",
                    );
                }
                if let Some(shortest) = runs
                    .iter()
                    .copied()
                    .filter(|r| *r + 1.0 < ALEXANDER_RUN)
                    .reduce(f64::min)
                    && runs.len() > 1
                {
                    self.push_ref(
                        Severity::Dica,
                        &label,
                        format!(
                            "Um trecho de bancada de só {} cm: abaixo de {} cm o pedaço não serve para preparar nada; junte-o a outro trecho.",
                            cm(shortest),
                            cm(ALEXANDER_RUN)
                        ),
                        "alexander184",
                    );
                }
            }
            // The four things you walk between, none of them far from another.
            let corners: Vec<(&str, Point2)> = [
                ("fogão", find(|u| matches!(u, Use::Stove))),
                ("pia", find(|u| matches!(u, Use::Sink))),
                (
                    "armazenagem",
                    find(|u| matches!(u, Use::Fridge | Use::Storage | Use::WallCabinet)),
                ),
                ("bancada", find(|u| matches!(u, Use::Counter | Use::Island))),
            ]
            .into_iter()
            .filter_map(|(name, u)| u.map(|u| (name, u.piece.position)))
            .collect();
            let farthest = corners
                .iter()
                .enumerate()
                .flat_map(|(i, a)| corners[i + 1..].iter().map(move |b| (a, b)))
                .map(|(a, b)| (a.1.distance(b.1), a.0, b.0))
                .max_by(|x, y| x.0.total_cmp(&y.0));
            if let Some((far, a, b)) = farthest
                && far > ALEXANDER_PAIR
            {
                self.push_ref(
                    Severity::Dica,
                    &label,
                    format!(
                        "{} cm entre {a} e {b}: acima de {} cm cada par vira travessia, e o preparo se desfaz em idas e vindas.",
                        cm(far),
                        cm(ALEXANDER_PAIR)
                    ),
                    "alexander184",
                );
            }
            // --- Blum: the five zones, in the order of the work ---
            let zones = [
                ("mantimentos", any(|u| matches!(u, Use::Fridge))),
                (
                    "armazenagem",
                    any(|u| matches!(u, Use::WallCabinet | Use::Storage)),
                ),
                ("lavagem", any(|u| matches!(u, Use::Sink))),
                ("preparo", any(|u| matches!(u, Use::Counter | Use::Island))),
                ("cocção", any(|u| matches!(u, Use::Stove))),
            ];
            let missing: Vec<&str> = zones
                .iter()
                .filter(|(_, there)| !there)
                .map(|(name, _)| *name)
                .collect();
            if !missing.is_empty() && missing.len() < zones.len() {
                self.push_ref(
                    Severity::Dica,
                    &label,
                    format!(
                        "Das cinco zonas de trabalho falta {}: sem ela o fluxo mantimentos → armazenagem → lavagem → preparo → cocção se quebra.",
                        missing.join(" e ")
                    ),
                    "blum-zonas",
                );
            }
            // --- NBR 5410: sockets by perimeter, and two above the counter ---
            if has_electrical {
                let perimeter = outline_length(&space.room.points);
                let sockets = space
                    .units
                    .iter()
                    .filter(|&&i| scene.units[i].what == Use::Outlet)
                    .count();
                let needed = (perimeter / 350.0).ceil().max(1.0) as usize;
                if sockets < needed {
                    self.push_ref(
                        Severity::Erro,
                        &label,
                        format!(
                            "{sockets} ponto(s) de tomada para {} m de perímetro: são necessários {needed} (um a cada 3,5 m ou fração).",
                            m2(perimeter * 100.0)
                        ),
                        "nbr5410",
                    );
                }
                let above = space
                    .units
                    .iter()
                    .filter(|&&i| {
                        let u = &scene.units[i];
                        u.what == Use::Outlet && u.piece.elevation >= 90.0
                    })
                    .count();
                if above < 2 {
                    self.push_ref(
                        Severity::Erro,
                        &label,
                        format!(
                            "{above} tomada(s) acima da bancada: são exigidas pelo menos 2, e forno, cooktop elétrico e lava-louças pedem circuito próprio."
                        ),
                        "nbr5410",
                    );
                }
            }
            // --- NBR 13103: a gas appliance needs permanent ventilation ---
            if let Some(stove) = find(|u| matches!(u, Use::Stove)) {
                let gas = !electric(&stove.piece.name);
                if gas {
                    let glass = self.glass.get(&space.room.id).copied().unwrap_or_default();
                    if glass <= 0.0 {
                        self.push_ref(
                            Severity::Erro,
                            &label,
                            "Aparelho a gás em ambiente sem janela nem abertura permanente: a ventilação é obrigatória, e aqui é segurança, não conforto.",
                            "nbr13103",
                        );
                    } else {
                        self.push_ref(
                            Severity::Dica,
                            &label,
                            "Aparelho a gás: confira a área de ventilação útil e as aberturas inferior e superior na edição vigente da norma — os valores mudaram entre edições.",
                            "nbr13103",
                        );
                    }
                }
                // --- LBNL: pulling the pollutants out before they spread ---
                match find(|u| matches!(u, Use::Hood)) {
                    None => self.push_ref(
                        Severity::Alerta,
                        &label,
                        "Cocção sem coifa: o cozimento gera material particulado e NO₂ dentro de casa, e sem captura eles ficam no ar da sala junto.",
                        "lbnl-coifa",
                    ),
                    Some(hood) if hood.piece.width + 1.0 < stove.piece.width => self.push_ref(
                        Severity::Dica,
                        &label,
                        format!(
                            "Coifa de {} cm sobre cocção de {} cm: a captura já cai à metade nas bocas da frente, e vazão alta não compensa coifa estreita.",
                            cm(hood.piece.width),
                            cm(stove.piece.width)
                        ),
                        "lbnl-coifa",
                    ),
                    Some(_) => {}
                }
            }
            // --- What is built at scale: the floor of the Brazilian market ---
            if space.what == RoomUse::Kitchen {
                let short = short_side(&space.room.points);
                if short + 0.5 < MCMV_KITCHEN_WIDTH {
                    self.push_ref(
                        Severity::Erro,
                        &label,
                        format!(
                            "Cozinha de {} cm de largura; a unidade financiada mais modesta entrega {} cm, com previsão de pia 120×50, fogão 55×60 e geladeira 70×70 cm.",
                            cm(short),
                            cm(MCMV_KITCHEN_WIDTH)
                        ),
                        "caixa-mcmv",
                    );
                }
                // --- The city has the last word, and only where we hold it ---
                match city.and_then(|c| c.kitchen_circle_cm.map(|d| (c, d))) {
                    Some((code, diameter)) => {
                        let turn = scene.turning_diameter(space);
                        if turn + 1.0 < diameter {
                            self.push_ref(
                                Severity::Erro,
                                &label,
                                format!(
                                    "Cabe um círculo de {} cm no piso; {} pede {} cm ({}). Entre norma e lei local prevalece o mais restritivo.",
                                    cm(turn),
                                    code.label,
                                    cm(diameter),
                                    code.source
                                ),
                                "coe-municipal",
                            );
                        }
                    }
                    None => self.push_ref(
                        Severity::Dica,
                        &label,
                        "Código de obras não informado: o círculo livre no piso e as áreas mínimas da cozinha variam por município — informe a cidade para que sejam verificados.",
                        "coe-municipal",
                    ),
                }
            }
        }
    }

    /// Screens: the sofa far enough to see the whole picture, close enough
    /// to read it (about 1,2 to 2,5 times the diagonal).
    fn screens(&mut self) {
        let scene = self.scene;
        for space in &scene.spaces {
            let tvs = space
                .units
                .iter()
                .filter(|&&i| scene.units[i].what == Use::Tv);
            for &tv in tvs {
                let screen = scene.units[tv].piece;
                // Embedded TVs sit inside a panel group: use the group position.
                let diagonal = screen.width.hypot(screen.height);
                let Some(sofa) = space
                    .units
                    .iter()
                    .filter(|&&i| matches!(scene.units[i].what, Use::Sofa(_) | Use::Armchair))
                    .min_by(|a, b| {
                        let d = |i: usize| scene.units[i].piece.position.distance(screen.position);
                        d(**a).total_cmp(&d(**b))
                    })
                else {
                    continue;
                };
                let distance = scene.units[*sofa].piece.position.distance(screen.position);
                let (near, far) = (diagonal * 1.2, diagonal * 2.5);
                let inches = (diagonal / 2.54).round();
                if distance < near || distance > far {
                    self.push(
                        Severity::Dica,
                        scene.units[tv].label(),
                        format!(
                            "{} a {} cm da TV de {inches:.0}\"; para essa tela o conforto fica entre {} e {} cm.",
                            scene.units[*sofa].label(),
                            cm(distance.round()),
                            cm(near.round()),
                            cm(far.round())
                        ),
                    );
                }
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
            self.push_ref(
                Severity::Alerta,
                "Casa",
                format!(
                    "{wrong} interruptor(es)/tomada(s) fora da faixa de 40 a 120 cm de alcance de quem usa cadeira de rodas."
                ),
                "nbr9050",
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

/// The shortest slide (along the piece's own axes, up to 1,5 m) that leaves
/// unit `i` clear of walls, pieces and door swings, in its room and still on
/// the walls it rests against. Returns move arguments plus the distance as `d`.
fn nudge(scene: &Scene<'_>, i: usize) -> Option<serde_json::Value> {
    let piece = scene.units[i].piece;
    if piece.is_opening() || !piece.locks.movable {
        return None;
    }
    let room = scene.room_at(piece.position);
    let on_walls = scene.contacts(i, 0.0, 0.0);
    let held = scene.held(i, 0.0, 0.0);
    let mut step = 5.0;
    while step <= 150.0 {
        for (lx, ly) in [(step, 0.0), (-step, 0.0), (0.0, step), (0.0, -step)] {
            let to = piece.to_plan((lx, ly));
            let (dx, dy) = (to.x - piece.position.x, to.y - piece.position.y);
            if scene.room_at(to) == room
                && scene.contacts(i, dx, dy) >= on_walls
                && scene.held(i, dx, dy) + 1.0 >= held
                && scene.conflicts(i, dx, dy) == 0
            {
                let round = |v: f64| (v * 10.0).round() / 10.0;
                return Some(serde_json::json!({
                    "tool": "move",
                    "ids": [piece.id.to_string()],
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

/// Moves unit `i` `dist` cm along `dir` if that adds no problem, keeps it in
/// its room, on its walls and in its niche: move arguments.
fn push_away(scene: &Scene<'_>, i: usize, dir: (f64, f64), dist: f64) -> Option<serde_json::Value> {
    let piece = scene.units[i].piece;
    if piece.is_opening() || !piece.locks.movable {
        return None;
    }
    let len = dir.0.hypot(dir.1).max(1e-9);
    let (dx, dy) = (dir.0 / len * dist, dir.1 / len * dist);
    let to = Point2::new(piece.position.x + dx, piece.position.y + dy);
    let round = |v: f64| (v * 10.0).round() / 10.0;
    (scene.room_at(to) == scene.room_at(piece.position)
        && scene.contacts(i, dx, dy) >= scene.contacts(i, 0.0, 0.0)
        // Not out of the niche it is built into.
        && scene.held(i, dx, dy) + 1.0 >= scene.held(i, 0.0, 0.0)
        && scene.conflicts(i, dx, dy) <= scene.conflicts(i, 0.0, 0.0))
    .then(|| {
        serde_json::json!({
            "tool": "move",
            "ids": [piece.id.to_string()],
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
/// Does the name say the cooktop burns nothing?
fn electric(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "induç",
        "induc",
        "induction",
        "elétric",
        "eletric",
        "electric",
    ]
    .iter()
    .any(|w| n.contains(w))
}

/// Perimeter of a room outline, cm.
fn outline_length(points: &[Point2]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a.distance(*b))
        .sum()
}

/// Glazed area on a room's outline, cm².
/// Glass that lights and airs each room, by room id.
///
/// A room is not a sealed box. Where two rooms run into each other with no
/// wall between — a living room open to its balcony, a kitchen open to the
/// living room — the light and the air of one are the light and the air of
/// the other, and a check that reads room by room accuses a plan that is
/// right. Which it did, three sessions running, on a kitchen whose balcony
/// carries a window 4,5 m wide.
///
/// Translucent pieces count too: a fixed pane, a fluted glass panel, a glass
/// door. They are drawn as furniture with `opacity`, not as openings, and
/// they are how a room with no facade of its own gets daylight.
fn glass_by_room(scene: &Scene<'_>) -> std::collections::BTreeMap<newera_core::RoomId, f64> {
    let home = scene.home;
    let own: Vec<(newera_core::RoomId, f64)> = home
        .rooms
        .iter()
        .map(|room| {
            let windows = window_area(scene, &room.points);
            let panes: f64 = home
                .furniture
                .iter()
                .flat_map(newera_core::Furniture::flatten)
                .filter(|f| {
                    f.visible
                        && f.opacity.is_some_and(|o| o < 1.0)
                        && near_outline(&room.points, f.position, f.depth.max(15.0) + 5.0)
                })
                .map(|f| f.width * f.height)
                .sum();
            (room.id, windows + panes)
        })
        .collect();

    // Rooms open to each other share what they have, however many hops away:
    // light crosses two open thresholds as readily as one.
    let mut group: Vec<usize> = (0..home.rooms.len()).collect();
    for i in 0..home.rooms.len() {
        for j in (i + 1)..home.rooms.len() {
            if open_between(home, &home.rooms[i], &home.rooms[j]) {
                let (a, b) = (group[i], group[j]);
                let root = a.min(b);
                for g in &mut group {
                    if *g == a || *g == b {
                        *g = root;
                    }
                }
            }
        }
    }
    let mut shared: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
    for (i, (_, glass)) in own.iter().enumerate() {
        *shared.entry(group[i]).or_default() += glass;
    }
    own.iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, shared.get(&group[i]).copied().unwrap_or_default()))
        .collect()
}

/// How wide an opening between two rooms has to be for one to air the other.
const OPEN_ENOUGH: f64 = 80.0;

/// Whether two rooms run into each other with no wall between them.
///
/// Their outlines touch along a stretch — that is what "no wall" looks like
/// in a drawing — and no wall stands on it.
fn open_between(home: &Home, a: &newera_core::Room, b: &newera_core::Room) -> bool {
    const STEP: f64 = 10.0;
    let walls: Vec<geo::Polygon<f64>> = home
        .wall_outlines()
        .iter()
        .filter(|o| o.len() >= 3)
        .map(|o| newera_core::to_polygon(o))
        .collect();
    let in_wall = |p: Point2| {
        use geo::Contains;
        walls.iter().any(|w| w.contains(&geo::Point::new(p.x, p.y)))
    };
    let mut open: f64 = 0.0;
    for k in 0..a.points.len() {
        let (from, to) = (a.points[k], a.points[(k + 1) % a.points.len()]);
        let length = from.distance(to);
        if length < STEP {
            continue;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (length / STEP) as usize;
        let mut run: f64 = 0.0;
        for step in 0..=steps {
            #[allow(clippy::cast_precision_loss)]
            let t = (step as f64 * STEP / length).min(1.0);
            let p = Point2::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            if near_outline(&b.points, p, 5.0) && !in_wall(p) {
                run += STEP;
                open = open.max(run);
            } else {
                run = 0.0;
            }
        }
    }
    open >= OPEN_ENOUGH
}

fn window_area(scene: &Scene<'_>, points: &[Point2]) -> f64 {
    scene
        .home
        .furniture
        .iter()
        .filter(|f| {
            f.visible
                && f.opening
                    .as_ref()
                    .is_some_and(|o| o.kind == OpeningKind::Window || f.catalog == "french-window")
                && near_outline(points, f.position, f.depth.max(15.0))
        })
        .map(|f| f.width * f.height)
        .sum()
}

/// Lengths of the continuous worktop runs in a space, cm. Cabinets that
/// touch are one run, because what matters is the stretch you can work on,
/// not how many boxes it was built from.
fn counter_runs(scene: &Scene<'_>, space: &Space<'_>) -> Vec<f64> {
    let tops: Vec<(Point2, f64)> = space
        .units
        .iter()
        .map(|&i| &scene.units[i])
        .filter(|u| matches!(u.what, Use::Counter | Use::Island))
        .map(|u| (u.piece.position, u.piece.width))
        .collect();
    let mut group: Vec<usize> = (0..tops.len()).collect();
    // Union-find, flattened by a few passes: the sets are tiny.
    for _ in 0..tops.len() {
        for i in 0..tops.len() {
            for j in i + 1..tops.len() {
                let apart = tops[i].0.distance(tops[j].0);
                if apart < f64::midpoint(tops[i].1, tops[j].1) + 15.0 {
                    let root = group[i].min(group[j]);
                    let (a, b) = (group[i], group[j]);
                    for g in &mut group {
                        if *g == a || *g == b {
                            *g = root;
                        }
                    }
                }
            }
        }
    }
    let mut runs: Vec<f64> = Vec::new();
    for root in 0..tops.len() {
        let length: f64 = group
            .iter()
            .enumerate()
            .filter(|(_, g)| **g == root)
            .map(|(i, _)| tops[i].1)
            .sum();
        if length > 0.0 {
            runs.push(length);
        }
    }
    runs
}

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

/// Height above the floor from which a piece is over a person's head, cm:
/// what `measure` leaves out of the band a walking person meets.
const HEADROOM: f64 = 190.0;

/// Reviews the current storey of `home` for the people in `profile`.
/// The name a finding is accepted by: its rule and its place, so the same
/// finding keeps the same name from one run to the next, and a different one
/// about the same room does not inherit an acceptance it never had.
fn key_of(finding: &Finding) -> String {
    let rule = finding.reference.unwrap_or("-");
    // The words of the message, not its numbers: the numbers move with every
    // edit, and an acceptance that lapses whenever a centimetre changes is an
    // acceptance nobody can rely on.
    let words: String = newera_core::fold(&finding.message)
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.len() > 3)
        .take(4)
        .collect::<Vec<_>>()
        .join("-");
    let place = newera_core::fold(&finding.place)
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .to_owned();
    format!("{rule}:{place}:{words}")
}

/// Acceptances of ergonomics findings that no current finding answers to,
/// on any storey, for these people.
///
/// A finding accepted with "corridor of 69 cm" as its reason, and then fixed
/// for real, leaves the reason behind; if the counter ever comes back, the
/// finding returns already silenced. Listing them is what lets them go.
pub fn orphaned(home: &Home, profile: &Profile) -> Vec<(String, String)> {
    let mine: Vec<(&String, &String)> = home
        .accepted
        .iter()
        .filter(|(key, _)| !newera_core::Issue::is_layout_key(key))
        .collect();
    if mine.is_empty() {
        return Vec::new();
    }
    let storeys: Vec<Option<newera_core::LevelId>> = if home.levels.is_empty() {
        vec![None]
    } else {
        home.levels.iter().map(|l| Some(l.id)).collect()
    };
    let mut live = std::collections::BTreeSet::new();
    for storey in storeys {
        let mut shown = home.clone();
        shown.selected_level = storey;
        live.extend(review(&shown, profile).findings.into_iter().map(|f| f.key));
    }
    mine.into_iter()
        .filter(|(key, _)| !live.contains(*key))
        .map(|(key, why)| (key.clone(), why.clone()))
        .collect()
}

pub fn review(home: &Home, profile: &Profile) -> Report {
    let view = home.level_view(home.current_level());
    let scene = Scene::new(&view);
    // The city is the project's, so every caller weighs the same rules; one
    // passed in the call still wins, for asking "and under this code?".
    let profile = &Profile {
        city: profile.city.clone().or_else(|| home.compass.city.clone()),
        ..profile.clone()
    };
    let mut review = Review {
        glass: glass_by_room(&scene),
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
    review.screens();
    review.reach();
    // The same finding on a row of modules is one finding about all of them.
    let mut findings: Vec<Finding> = Vec::new();
    for f in review.findings {
        match findings.iter_mut().find(|g| {
            g.severity == f.severity
                && g.message == f.message
                && g.reference == f.reference
                && g.fix.is_none()
                && f.fix.is_none()
        }) {
            Some(same) => {
                same.place.push_str(", ");
                same.place.push_str(&f.place);
            }
            None => findings.push(f),
        }
    }
    findings.sort_by_key(|f| f.severity);
    // Each finding gets the name it is accepted by, and the ones already
    // looked at carry the reason instead of the cost.
    for finding in &mut findings {
        finding.key = key_of(finding);
        finding.accepted = home.accepted.get(&finding.key).cloned();
    }
    // Errors weigh fully; many alerts or tips of a crowded plan level off.
    let penalty_of = |list: &[Finding]| {
        let count = |sev: Severity| {
            list.iter()
                .filter(|f| f.severity == sev && f.accepted.is_none())
                .count() as u32
        };
        let (errors, alerts, tips) = (
            count(Severity::Erro),
            count(Severity::Alerta),
            count(Severity::Dica),
        );
        errors * 12 + alerts.min(6) * 5 + alerts.saturating_sub(6) + tips.min(10)
    };
    let penalty = penalty_of(&findings);
    // What each one costs: the score without it, minus the score with it.
    // A number that moves without saying why is a number nobody can act on.
    for at in 0..findings.len() {
        let mut without = findings.clone();
        without.remove(at);
        findings[at].weight = penalty.saturating_sub(penalty_of(&without));
    }
    // Each source once, in the order the findings first lean on it.
    let mut refs: Vec<&'static Standard> = Vec::new();
    for code in findings.iter().filter_map(|f| f.reference) {
        if let Some(source) = standards::standard(code)
            && !refs.iter().any(|s| s.code == code)
        {
            refs.push(source);
        }
    }
    Report {
        score: 100u32.saturating_sub(penalty),
        capacity,
        findings,
        refs,
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

    /// Is there a finding of this severity standing on this source?
    fn cites(report: &Report, severity: Severity, code: &str) -> bool {
        report
            .findings
            .iter()
            .any(|f| f.severity == severity && f.reference == Some(code))
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

    fn named_again(home: &mut Home) {
        let mut shower = piece(30, "shower", (40.0, 200.0), (90.0, 90.0, 200.0), 0.0);
        shower.name = "Box".into();
        home.furniture.push(shower);
    }

    #[test]
    fn imported_pieces_are_read_by_name_and_built_in_ones_do_not_collide() {
        let mut home = Home::default();
        square(&mut home, "Banho suíte", 300.0, 240.0);
        let mut named = |id: u64, name: &str, at: (f64, f64), size: (f64, f64, f64), elev: f64| {
            let mut f = piece(id, "imported", at, size, 0.0);
            f.name = name.into();
            f.elevation = elev;
            home.furniture.push(f);
        };
        named(
            20,
            "Bancada contínua junto à geladeira",
            (150.0, 40.0),
            (212.0, 64.0, 4.0),
            87.0,
        );
        named(
            21,
            "Cooktop Brastemp BDS62AE — 4 bocas",
            (150.0, 40.0),
            (59.0, 48.5, 8.6),
            87.0,
        );
        named(
            22,
            "10 — Gavetões sob cooktop — 65 cm",
            (150.0, 40.0),
            (65.0, 60.0, 87.0),
            0.0,
        );
        named(
            23,
            "Mesa Dover — eucalipto",
            (150.0, 170.0),
            (120.0, 80.0, 77.0),
            0.0,
        );
        named(24, "Cadeira Dover", (150.0, 140.0), (51.0, 59.0, 85.0), 0.0);
        named(25, "Vaso suíte", (40.0, 200.0), (40.0, 63.0, 62.0), 0.0);
        named(
            26,
            "Cama suíte",
            (1000.0, 1000.0),
            (140.0, 200.0, 70.0),
            0.0,
        );
        let scene = Scene::new(&home);
        let what = |id: u64| {
            scene
                .units
                .iter()
                .find(|u| u.piece.id == FurnitureId(id))
                .unwrap()
                .what
        };
        assert_eq!(what(21), Use::Stove);
        assert_eq!(what(22), Use::Counter);
        assert_eq!(what(23), Use::DiningTable(4));
        assert_eq!(what(24), Use::Chair);
        assert_eq!(what(25), Use::Toilet);
        assert_eq!(what(26), Use::Bed(2));
        assert_eq!(scene.spaces[0].what, RoomUse::Bathroom);
        // The cooktop sits in its countertop and the chair under the table: no overlap.
        assert!(scene.overlaps().is_empty(), "{:?}", scene.overlaps());
        // A toilet standing inside the shower is not built in.
        named_again(&mut home);
        let scene = Scene::new(&home);
        assert_eq!(scene.overlaps().len(), 1);
    }

    #[test]
    fn the_sofa_sits_at_a_distance_that_suits_the_screen() {
        let mut home = Home::default();
        square(&mut home, "Sala", 400.0, 400.0);
        let mut tv = piece(20, "tv", (200.0, 12.0), (124.0, 8.0, 72.0), 0.0);
        tv.elevation = 70.0;
        home.furniture.push(tv);
        home.furniture.push(piece(
            21,
            "sofa-3",
            (200.0, 160.0),
            (210.0, 90.0, 85.0),
            180.0,
        ));
        let report = review(&home, &Profile::default());
        // 148 cm from a 56" screen (143 cm diagonal): closer than 172 cm.
        assert!(
            says(&report, Severity::Dica, "entre 172 e 358 cm"),
            "{report:#?}"
        );
        home.furniture[1].position.y = 230.0;
        let report = review(&home, &Profile::default());
        assert!(!says(&report, Severity::Dica, "da TV"), "{report:#?}");
    }

    /// The false positive with a standard at its root: a dishwasher is an
    /// appliance in a niche (EN 1116), so its lid is never read as the height
    /// of the worktop it hides under.
    #[test]
    fn a_dishwasher_is_an_appliance_not_a_worktop() {
        let mut home = Home::default();
        square(&mut home, "Cozinha", 400.0, 300.0);
        home.furniture.push(piece(
            20,
            "sink-counter",
            (100.0, 7.5 + 30.0),
            (120.0, 60.0, 92.0),
            0.0,
        ));
        // A 84,5 cm dishwasher beside a 92 cm counter, for a 146 cm cook whose
        // comfortable height is exactly 84 cm: if the dishwasher counted as
        // worktop, the counter would be the one reported as wrong.
        home.furniture.push(piece(
            21,
            "dishwasher",
            (220.0, 7.5 + 30.0),
            (60.0, 60.0, 84.5),
            0.0,
        ));
        home.furniture.push(piece(
            22,
            "stove",
            (300.0, 7.5 + 31.0),
            (60.0, 62.0, 90.0),
            0.0,
        ));
        let tall = Profile {
            stature: 165.0.into(),
            ..Profile::default()
        };
        let report = review(&home, &tall);
        assert!(
            !says(&report, Severity::Dica, "84,5 cm"),
            "the dishwasher's lid is not a countertop: {report:#?}"
        );

        // The reported false positive came the other way: a designer's
        // module, numbered and named, with no catalog id to go by.
        let mut named = home.clone();
        named.furniture.retain(|f| f.id != FurnitureId(21));
        let mut module = piece(24, "", (220.0, 7.5 + 30.0), (60.0, 60.0, 84.5), 0.0);
        module.name = "10 — Lava-louças de embutir".to_owned();
        named.furniture.push(module);
        let report = review(&named, &tall);
        assert!(
            !says(&report, Severity::Dica, "84,5 cm"),
            "read by name, it is still an appliance: {report:#?}"
        );
    }

    #[test]
    fn the_kitchen_is_read_against_alexander_the_gas_standard_and_the_city() {
        let mut home = Home::default();
        // 170 cm wide: narrower than the most modest financed unit.
        square(&mut home, "Cozinha", 380.0, 185.0);
        home.furniture.push(piece(
            20,
            "sink-counter",
            (80.0, 7.5 + 30.0),
            (120.0, 60.0, 90.0),
            0.0,
        ));
        home.furniture.push(piece(
            21,
            "stove",
            (200.0, 7.5 + 31.0),
            (60.0, 62.0, 90.0),
            0.0,
        ));
        home.furniture.push(piece(
            22,
            "fridge",
            (330.0, 7.5 + 35.0),
            (70.0, 70.0, 180.0),
            0.0,
        ));
        // One short stretch of worktop, and nothing else.
        home.furniture.push(piece(
            23,
            "base-cabinet",
            (280.0, 7.5 + 30.0),
            (60.0, 60.0, 90.0),
            0.0,
        ));
        let report = review(&home, &Profile::default());
        // Alexander 184: 60 cm of free counter is not 366.
        assert!(
            cites(&report, Severity::Dica, "alexander184"),
            "{report:#?}"
        );
        // A gas appliance with no window at all is a safety problem.
        assert!(cites(&report, Severity::Erro, "nbr13103"), "{report:#?}");
        // Cooking with nothing to capture what it releases.
        assert!(
            cites(&report, Severity::Alerta, "lbnl-coifa"),
            "{report:#?}"
        );
        // The narrow kitchen: the figure comes from secondary material, so it
        // warns instead of accusing, even though the rule asked for an error.
        assert!(
            cites(&report, Severity::Alerta, "caixa-mcmv"),
            "{report:#?}"
        );
        // No city: the municipal circle is advice, not a verdict.
        assert!(
            cites(&report, Severity::Dica, "coe-municipal"),
            "{report:#?}"
        );

        let paulista = Profile {
            city: Some("sao-paulo".to_owned()),
            ..Profile::default()
        };
        let judged = review(&home, &paulista);
        assert!(
            cites(&judged, Severity::Erro, "coe-municipal"),
            "a kitchen 170 cm wide holds no 120 cm circle: {judged:#?}"
        );
        assert!(
            judged.refs.iter().any(|r| r.code == "coe-municipal"),
            "{:?}",
            judged.refs
        );

        // Induction burns nothing: the gas standard stops applying.
        let mut electric = home.clone();
        electric
            .furniture
            .iter_mut()
            .find(|f| f.id == FurnitureId(21))
            .unwrap()
            .name = "Cooktop de indução".to_owned();
        let clean = review(&electric, &Profile::default());
        assert!(
            !clean
                .findings
                .iter()
                .any(|f| f.reference == Some("nbr13103")),
            "{clean:#?}"
        );
    }

    /// Sockets are only judged where someone drew an electrical project: a
    /// plan without one is unfinished, not wrong.
    #[test]
    fn sockets_are_counted_only_against_an_electrical_project() {
        let mut home = Home::default();
        square(&mut home, "Cozinha", 400.0, 300.0);
        home.furniture.push(piece(
            20,
            "sink-counter",
            (100.0, 7.5 + 30.0),
            (120.0, 60.0, 90.0),
            0.0,
        ));
        home.furniture.push(piece(
            21,
            "stove",
            (250.0, 7.5 + 31.0),
            (60.0, 62.0, 90.0),
            0.0,
        ));
        let quiet = review(&home, &Profile::default());
        assert!(
            !quiet
                .findings
                .iter()
                .any(|f| f.reference == Some("nbr5410")),
            "{quiet:#?}"
        );

        let mut wired = home.clone();
        let mut socket = piece(22, "outlet-low", (40.0, 7.5), (10.0, 5.0, 10.0), 0.0);
        socket.elevation = 30.0;
        wired.furniture.push(socket);
        let report = review(&wired, &Profile::default());
        // Perimeter of 13,7 m needs four points; one is drawn, none of them
        // above the counter.
        assert!(cites(&report, Severity::Erro, "nbr5410"), "{report:#?}");
        assert!(
            says(&report, Severity::Erro, "0 tomada(s) acima da bancada"),
            "{report:#?}"
        );
    }

    /// The severity policy is the whole point of the ladder, so it is tested
    /// on its own rather than only through the rules that lean on it.
    #[test]
    fn a_source_never_claims_more_than_its_tier_allows() {
        let mut home = Home::default();
        square(&mut home, "Sala", 300.0, 300.0);
        let view = home.level_view(home.current_level());
        let scene = Scene::new(&view);
        let profile = Profile::default();
        let mut review = Review {
            scene: &scene,
            profile: &profile,
            findings: Vec::new(),
            glass: std::collections::BTreeMap::new(),
        };
        // Tier A, checked at the source: an error stays an error.
        review.push_ref(Severity::Erro, "Casa", "gás sem abertura", "nbr13103");
        // Tier A, figure not confirmed at the source: it may warn, never accuse.
        review.push_ref(Severity::Erro, "Casa", "cozinha estreita", "caixa-mcmv");
        // Tier C doctrine cannot raise an alert, however the rule asked.
        review.push_ref(Severity::Erro, "Casa", "bancada curta", "alexander184");
        // Tier E describes what people do: it raises nothing at all.
        review.push_ref(Severity::Erro, "Casa", "tendência", "houzz2026");
        let got: Vec<(Severity, Option<&str>)> = review
            .findings
            .iter()
            .map(|f| (f.severity, f.reference))
            .collect();
        assert_eq!(
            got,
            vec![
                (Severity::Erro, Some("nbr13103")),
                (Severity::Alerta, Some("caixa-mcmv")),
                (Severity::Dica, Some("alexander184")),
            ],
            "the survey was dropped and the rest was capped"
        );
    }

    /// Alexander counts the stretch you can work on, not the boxes it was
    /// built from: cabinets that touch are one run.
    #[test]
    fn touching_cabinets_are_one_worktop_run() {
        let mut home = Home::default();
        square(&mut home, "Cozinha", 600.0, 300.0);
        // Three 60 cm cabinets side by side: one run of 180 cm.
        for (n, x) in [(30_u64, 40.0), (31, 100.0), (32, 160.0)] {
            home.furniture.push(piece(
                n,
                "base-cabinet",
                (x, 7.5 + 30.0),
                (60.0, 60.0, 90.0),
                0.0,
            ));
        }
        // One on its own at the far end: a 60 cm stretch nobody can use.
        home.furniture.push(piece(
            33,
            "base-cabinet",
            (540.0, 7.5 + 30.0),
            (60.0, 60.0, 90.0),
            0.0,
        ));
        home.furniture.push(piece(
            34,
            "sink-counter",
            (300.0, 7.5 + 30.0),
            (120.0, 60.0, 90.0),
            0.0,
        ));
        home.furniture.push(piece(
            35,
            "stove",
            (400.0, 7.5 + 31.0),
            (60.0, 62.0, 90.0),
            0.0,
        ));
        let report = review(&home, &Profile::default());
        assert!(
            says(&report, Severity::Dica, "trecho de bancada de só 60 cm"),
            "the lone cabinet is reported, the row of three is not: {report:#?}"
        );
        assert!(
            !says(&report, Severity::Dica, "trecho de bancada de só 180 cm"),
            "{report:#?}"
        );
        // 180 + 60 = 240 cm of free counter, still short of Alexander's 366.
        assert!(
            says(&report, Severity::Dica, "240 cm de bancada livre"),
            "{report:#?}"
        );
    }

    #[test]
    fn a_kitchen_spread_too_wide_and_missing_a_zone_is_reported() {
        let mut home = Home::default();
        square(&mut home, "Cozinha", 600.0, 300.0);
        home.furniture.push(piece(
            40,
            "sink-counter",
            (60.0, 7.5 + 30.0),
            (120.0, 60.0, 90.0),
            0.0,
        ));
        // The stove 4,4 m away from the sink: every trip is a crossing.
        home.furniture.push(piece(
            41,
            "stove",
            (500.0, 7.5 + 31.0),
            (60.0, 62.0, 90.0),
            0.0,
        ));
        home.furniture.push(piece(
            42,
            "fridge",
            (300.0, 7.5 + 35.0),
            (70.0, 70.0, 180.0),
            0.0,
        ));
        let report = review(&home, &Profile::default());
        assert!(
            says(&report, Severity::Dica, "entre fogão e pia")
                && cites(&report, Severity::Dica, "alexander184"),
            "{report:#?}"
        );
        // Nowhere to put the plates and nothing to prepare on.
        assert!(
            says(&report, Severity::Dica, "falta armazenagem e preparo")
                && cites(&report, Severity::Dica, "blum-zonas"),
            "{report:#?}"
        );
    }

    /// The plan that was right and kept being accused: a kitchen open to a
    /// living room, open in turn to a balcony whose facade is one long
    /// window. Nothing between them but air, and every round of review said
    /// "gas appliance with no window" and "living room with no window".
    #[test]
    fn light_and_air_cross_a_room_that_is_open_to_the_next() {
        let mut home = Home::default();
        // Three rooms in a row, sharing their boundaries with no wall on them:
        // kitchen 0..300, living 300..600, balcony 600..800 along x.
        let outer = [(0.0, 0.0), (800.0, 0.0), (800.0, 400.0), (0.0, 400.0)];
        for k in 0..4 {
            let (a, b) = (outer[k], outer[(k + 1) % 4]);
            let mut wall = Wall::new(
                WallId(k as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            );
            wall.thickness = 15.0;
            wall.height = 260.0;
            home.walls.push(wall);
        }
        let room = |id: u64, name: &str, x0: f64, x1: f64| {
            Room::new(
                RoomId(id),
                name,
                vec![
                    Point2::new(x0, 7.5),
                    Point2::new(x1, 7.5),
                    Point2::new(x1, 392.5),
                    Point2::new(x0, 392.5),
                ],
            )
        };
        home.rooms.push(room(1, "Cozinha", 7.5, 300.0));
        home.rooms.push(room(2, "Sala", 300.0, 600.0));
        home.rooms.push(room(3, "Varanda", 600.0, 792.5));
        // The only window in the home is on the balcony's facade.
        let mut window = piece(70, "window", (700.0, 392.5), (452.0, 15.0, 155.0), 0.0);
        window.opening = Some(newera_core::Opening {
            kind: OpeningKind::Window,
            ..newera_core::Opening::default()
        });
        window.elevation = 100.0;
        home.furniture.push(window);
        // A gas cooktop in the kitchen, and the rest of a kitchen around it.
        let mut stove = piece(71, "stove", (150.0, 40.0), (75.0, 62.0, 91.0), 0.0);
        stove.name = "Fogão 5 bocas a gás".to_owned();
        home.furniture.push(stove);
        home.furniture.push(piece(
            72,
            "sink-counter",
            (60.0, 40.0),
            (120.0, 60.0, 91.0),
            0.0,
        ));
        home.furniture
            .push(piece(73, "fridge", (260.0, 45.0), (70.0, 70.0, 180.0), 0.0));

        let report = review(&home, &Profile::default());
        assert!(
            !says(
                &report,
                Severity::Erro,
                "sem janela nem abertura permanente"
            ),
            "the balcony airs the kitchen through the living room: {report:#?}"
        );
        assert!(
            !says(&report, Severity::Alerta, "Sem janela"),
            "and lights it: {report:#?}"
        );

        // Close the kitchen off with a wall and the finding comes back, as
        // it should: then there really is nowhere for the gas to go.
        let mut divider = Wall::new(
            WallId(9),
            Point2::new(300.0, 7.5),
            Point2::new(300.0, 392.5),
        );
        divider.thickness = 15.0;
        divider.height = 260.0;
        home.walls.push(divider);
        home.rooms[0] = room(1, "Cozinha", 7.5, 292.5);
        home.rooms[1] = room(2, "Sala", 307.5, 600.0);
        let closed = review(&home, &Profile::default());
        assert!(
            says(
                &closed,
                Severity::Erro,
                "sem janela nem abertura permanente"
            ),
            "{closed:#?}"
        );
    }

    /// A dishwasher under the stone is 6 cm lower than the stone, by design.
    /// Reading its top as the counter accused a kitchen whose stone was right
    /// — the height of a kitchen is where most of its stone is.
    #[test]
    fn an_appliance_lower_than_the_stone_is_not_read_as_the_counter() {
        let mut home = Home::default();
        square(&mut home, "Cozinha", 400.0, 300.0);
        for (id, x) in [(60_u64, 80.0), (61, 260.0)] {
            home.furniture.push(piece(
                id,
                "sink-counter",
                (x, 7.5 + 30.0),
                (120.0, 60.0, 91.0),
                0.0,
            ));
        }
        home.furniture.push(piece(
            62,
            "stove",
            (180.0, 7.5 + 31.0),
            (60.0, 62.0, 91.0),
            0.0,
        ));
        home.furniture.push(piece(
            63,
            "fridge",
            (350.0, 7.5 + 35.0),
            (70.0, 70.0, 180.0),
            0.0,
        ));
        // The dishwasher: its own 84,5 cm, tucked under the same stone.
        let mut washer = piece(
            64,
            "dishwasher",
            (150.0, 7.5 + 30.0),
            (60.0, 58.0, 84.5),
            0.0,
        );
        washer.name = "LP14V lava-louças".to_owned();
        home.furniture.push(washer);

        // 91 cm is right for someone 165 cm tall, so nothing is said.
        let report = review(&home, &Profile::default());
        assert!(!says(&report, Severity::Dica, "Bancada a"), "{report:#?}");

        // Lower the stone itself and the kitchen is reported, as it should be.
        for piece in &mut home.furniture {
            if piece.catalog == "sink-counter" {
                piece.height = 78.0;
            }
        }
        let report = review(&home, &Profile::default());
        assert!(says(&report, Severity::Dica, "Bancada a 78"), "{report:#?}");
    }

    /// What the laboratory knows: capture is the canopy's job, and a hood
    /// narrower than the burners loses the front ones whatever its airflow.
    #[test]
    fn a_hood_narrower_than_the_cooktop_is_reported_and_a_window_softens_the_gas_rule() {
        let mut home = Home::default();
        square(&mut home, "Cozinha", 400.0, 300.0);
        home.furniture.push(piece(
            50,
            "sink-counter",
            (100.0, 7.5 + 30.0),
            (120.0, 60.0, 90.0),
            0.0,
        ));
        let mut stove = piece(51, "stove", (250.0, 7.5 + 31.0), (75.0, 62.0, 90.0), 0.0);
        stove.name = "Fogão 5 bocas".to_owned();
        home.furniture.push(stove);
        let mut hood = piece(52, "hood", (250.0, 7.5 + 25.0), (60.0, 50.0, 60.0), 0.0);
        hood.elevation = 150.0;
        home.furniture.push(hood);
        let mut window = piece(53, "window", (200.0, 0.0), (120.0, 15.0, 120.0), 0.0);
        window.opening = Some(newera_core::Opening {
            kind: OpeningKind::Window,
            ..newera_core::Opening::default()
        });
        window.elevation = 100.0;
        home.furniture.push(window);
        let report = review(&home, &Profile::default());
        assert!(
            says(
                &report,
                Severity::Dica,
                "Coifa de 60 cm sobre cocção de 75 cm"
            ) && cites(&report, Severity::Dica, "lbnl-coifa"),
            "{report:#?}"
        );
        assert!(
            !says(&report, Severity::Alerta, "Cocção sem coifa"),
            "{report:#?}"
        );
        // With an opening, the gas standard advises instead of accusing.
        assert!(
            cites(&report, Severity::Dica, "nbr13103")
                && !cites(&report, Severity::Erro, "nbr13103"),
            "{report:#?}"
        );
    }

    #[test]
    fn the_front_is_where_the_doors_are_built_not_where_angle_points() {
        // A sink cabinet imported with angle 0 (front at +y) whose doors and
        // drawers are all built on its -y face, toward a 112 cm corridor. At
        // +y it touches the stone of the peninsula behind it.
        let mut home = Home::default();
        square(&mut home, "Cozinha", 300.0, 300.0);
        let mut sink = piece(20, "sink-counter", (150.0, 150.0), (120.0, 60.0, 90.0), 0.0);
        let panel = |id: u64, name: &str, elev: f64| {
            let mut part = piece(id, "panel", (150.0, 121.0), (118.0, 2.0, 30.0), 0.0);
            part.name = name.to_owned();
            part.elevation = elev;
            part
        };
        sink.children = vec![
            panel(21, "Porta esquerda", 0.0),
            panel(22, "Porta direita", 0.0),
            panel(23, "Gaveta", 30.0),
            panel(24, "Frente fixa", 60.0),
        ];
        assert_eq!(newera_core::facing(&sink), "-y");
        home.furniture.push(sink);
        home.furniture.push(piece(
            30,
            "base-cabinet",
            (150.0, 200.0),
            (120.0, 40.0, 90.0),
            180.0,
        ));
        let report = review(&home, &Profile::default());
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.place.contains("f20") && f.message.contains("livres à frente")),
            "the corridor is at the doors, not at the stone behind: {report:#?}"
        );

        // Squeeze the corridor at the doors and the same rule speaks, about
        // the side that is really used.
        home.furniture.push(piece(
            31,
            "base-cabinet",
            (150.0, 90.0),
            (120.0, 30.0, 90.0),
            0.0,
        ));
        let report = review(&home, &Profile::default());
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.place.contains("f20") && f.message.contains("15 cm livres à frente")),
            "{report:#?}"
        );
    }

    #[test]
    fn a_fix_does_not_pull_an_appliance_out_of_its_niche() {
        // A sink run on the top wall, 68 cm from a peninsula whose middle
        // module is a dishwasher in a 60 cm niche between two cabinets.
        let mut home = Home::default();
        square(&mut home, "Cozinha", 400.0, 400.0);
        let peninsula = |id, catalog, x| piece(id, catalog, (x, 165.5), (60.0, 60.0, 85.0), 180.0);
        home.furniture.push(peninsula(20, "dishwasher", 190.0));
        home.furniture.push(peninsula(21, "base-cabinet", 130.0));
        home.furniture.push(peninsula(22, "base-cabinet", 250.0));
        home.furniture.push(piece(
            23,
            "sink-counter",
            (190.0, 37.5),
            (180.0, 60.0, 90.0),
            0.0,
        ));
        let report = review(&home, &Profile::default());
        let narrow = report
            .findings
            .iter()
            .find(|f| f.place.contains("f23") && f.message.contains("68 cm livres à frente"))
            .unwrap_or_else(|| panic!("{report:#?}"));
        assert!(
            narrow.fix.is_none(),
            "17 cm into the corridor takes the dishwasher out of its niche: {narrow:#?}"
        );
        assert!(
            report
                .findings
                .iter()
                .filter_map(|f| f.fix.as_ref())
                .all(|fix| fix["ids"][0] != "f20"),
            "{report:#?}"
        );

        // Standing on its own, the same appliance may still be moved.
        home.furniture.retain(|f| f.id.0 != 21 && f.id.0 != 22);
        let report = review(&home, &Profile::default());
        let narrow = report
            .findings
            .iter()
            .find(|f| f.place.contains("f23") && f.message.contains("livres à frente"))
            .unwrap_or_else(|| panic!("{report:#?}"));
        assert_eq!(
            narrow.fix.as_ref().map(|f| f["ids"][0].clone()),
            Some(serde_json::json!("f20")),
            "{narrow:#?}"
        );
    }

    #[test]
    fn a_moulding_over_the_head_has_no_corridor_to_keep() {
        // A kitchen tower's crown moulding at 272 cm, 82 cm from a sofa.
        let mut home = Home::default();
        square(&mut home, "Cozinha", 400.0, 400.0);
        let mut moulding = piece(20, "base-cabinet", (200.0, 9.0), (80.0, 3.0, 8.0), 0.0);
        moulding.name = "moldura de roda-teto".into();
        moulding.elevation = 272.0;
        home.furniture.push(moulding);
        let sofa = piece(
            21,
            "sofa-3",
            (200.0, 10.5 + 82.0 + 45.0),
            (200.0, 90.0, 80.0),
            180.0,
        );
        home.furniture.push(sofa.clone());
        let report = review(&home, &Profile::default());
        assert!(
            !report.findings.iter().any(|f| f.place.contains("f20")),
            "{report:#?}"
        );

        // The same piece on the floor is a counter with a corridor to keep.
        home.furniture[0].elevation = 0.0;
        home.furniture[0].height = 90.0;
        let report = review(&home, &Profile::default());
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.place.contains("f20") && f.message.contains("livres à frente")),
            "{report:#?}"
        );
    }

    #[test]
    fn a_narrow_corner_says_how_much_of_the_side_it_takes() {
        // A 185 cm wardrobe whose doors face a free room, but for a 38 cm
        // nightstand 54 cm in front of one stretch of it.
        let mut home = Home::default();
        square(&mut home, "Quarto", 400.0, 400.0);
        home.furniture.push(piece(
            20,
            "wardrobe",
            (100.0, 37.5),
            (185.0, 60.0, 220.0),
            0.0,
        ));
        home.furniture.push(piece(
            21,
            "nightstand",
            (69.0, 67.5 + 54.0 + 20.0),
            (38.0, 40.0, 55.0),
            0.0,
        ));
        let report = review(&home, &Profile::default());
        let front = report
            .findings
            .iter()
            .find(|f| f.place.contains("f20") && f.message.contains("livres à frente"))
            .unwrap_or_else(|| panic!("{report:#?}"));
        assert!(
            front
                .message
                .starts_with("54 cm livres à frente em 38 dos 185 cm"),
            "{front:#?}"
        );

        let key = front.key.clone();

        // Across the whole front, the sentence stays as it was — and so does
        // the name it is accepted by.
        home.furniture[1].width = 185.0;
        home.furniture[1].position.x = 100.0;
        let report = review(&home, &Profile::default());
        let front = report
            .findings
            .iter()
            .find(|f| f.place.contains("f20") && f.message.contains("livres à frente"))
            .unwrap_or_else(|| panic!("{report:#?}"));
        assert!(
            front.message.starts_with("54 cm livres à frente ("),
            "{front:#?}"
        );
        assert_eq!(front.key, key);
    }

    #[test]
    fn a_hinge_flip_is_offered_only_when_it_clears_the_leaf_for_check_layout_too() {
        // A bathroom door: its leaf, from the left jamb, sweeps over a small
        // cabinet; from the right jamb it would sweep over a wall-hung basin
        // this review reads as built in, and check_layout does not.
        let mut home = Home::default();
        square(&mut home, "Banheiro", 300.0, 240.0);
        let mut door = piece(20, "door", (150.0, 0.0), (70.0, 15.0, 210.0), 0.0);
        door.opening = Some(newera_core::Opening::default());
        home.furniture.push(door);
        home.furniture.push(piece(
            21,
            "base-cabinet",
            (122.0, 60.0),
            (15.0, 15.0, 80.0),
            0.0,
        ));
        let mut basin = piece(22, "sink", (178.0, 60.0), (15.0, 15.0, 18.0), 0.0);
        basin.name = "Lavatório social".into();
        basin.elevation = 70.0;
        home.furniture.push(basin);

        let door = home.furniture[0].clone();
        assert_eq!(
            newera_core::door_blocked_by(&home, &door),
            vec![FurnitureId(21)]
        );
        let mut flipped = door.clone();
        flipped.opening.as_mut().unwrap().hinge_right = true;
        assert_eq!(
            newera_core::door_blocked_by(&home, &flipped),
            vec![FurnitureId(22)]
        );

        let report = review(&home, &Profile::default());
        let hit = report
            .findings
            .iter()
            .find(|f| f.message.contains("A folha da porta bate"))
            .unwrap_or_else(|| panic!("{report:#?}"));
        assert!(
            hit.fix
                .as_ref()
                .is_none_or(|fix| fix["items"][0].get("hinge_right").is_none()),
            "a flip that check_layout would still call blocked is not offered: {hit:#?}"
        );
        assert!(!hit.message.contains("invertendo"), "{hit:#?}");
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
        assert!(cites(&report, Severity::Alerta, "nbr15575g"), "{report:#?}");
        // The citation travels in the field, not inside the sentence.
        assert!(
            report.findings.iter().all(|f| !f.message.contains("NBR ")),
            "{report:#?}"
        );
        assert!(
            report.refs.iter().any(|r| r.code == "nbr15575g"),
            "the report resolves what it leaned on: {:?}",
            report.refs
        );
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
            says(&wheel, Severity::Alerta, "80 cm livres")
                && cites(&wheel, Severity::Alerta, "nbr9050"),
            "{wheel:#?}"
        );
        assert!(
            says(&wheel, Severity::Alerta, "giro de cadeira"),
            "{wheel:#?}"
        );
    }
}
