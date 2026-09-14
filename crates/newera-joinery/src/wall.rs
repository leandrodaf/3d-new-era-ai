//! `cabinet_run`: fills one wall with cabinets sized for it. The wall is
//! measured in `newera-core`, the modules are planned in `newera-joinery`,
//! and here the old cabinets make way for the new ones. Where the run meets
//! another wall's run in an L, the cabinet under the other run's fronts
//! becomes a blind corner; the other wall is planned again in the same undo
//! step so the corner works from whichever side was drawn first.

use crate::{Build, EndKind, PARAMS_KEY, RunGap, RunOver, RunParams, RunRow};
use newera_core::{
    Command, Document, Furniture, FurnitureId, Home, Point2, RunBlock, RunObstacle, WallId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A number rounded to a tenth for replies.
fn num(v: f64) -> Value {
    let rounded = (v * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 && rounded.abs() < 1e15 {
        json!(rounded as i64)
    } else {
        json!(rounded)
    }
}

/// Property marking the modules of a run: `w3:base`.
pub const RUN_KEY: &str = "joinery:run";
/// Property holding the run's request, to plan it again after a neighbor changes.
const REQUEST_KEY: &str = "joinery:run_request";
/// Filler between a blind panel's edge and the doors beside it, cm.
const CORNER_FILLER: f64 = 3.0;
/// How far in front of a run another run's fronts still block its doors, cm.
const DOOR_SWING: f64 = 60.0;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct CabinetRunParams {
    /// Wall whose face gets the cabinets.
    pub wall: Option<String>,
    /// Or a piece against the wall (`f12`, the fridge): its nearest wall, on its side.
    pub near: Option<String>,
    /// Room the fronts face (default: the side toward the middle of the house).
    pub room: Option<String>,
    /// Flat choices, all optional: row base|wall|tall, h, d, elev, t (mm), front,
    /// color [r,g,b], drawers (drawer units), max (widest module, 90), target (60),
    /// top (base countertop, true), `top_material`, sink / cooktop (center cm along
    /// the wall: cabinet under it and the countertop cutout), `sink_w` (80), `cooktop_w` (60),
    /// interior shelves|hanging|wardrobe (tall rows; wardrobe by default facing a bedroom).
    pub p: Option<serde_json::Map<String, Value>>,
    /// Stretch to fill, cm from the wall start (default: all of it).
    pub from: Option<f64>,
    pub to: Option<f64>,
    /// Cabinets on that wall that stay as they are.
    pub keep: Option<Vec<String>>,
    /// Plan and report only.
    #[serde(default)]
    pub dry: bool,
}

/// A resolved run: one wall face and the choices for it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Request {
    wall: WallId,
    side: f64,
    params: RunParams,
    /// Keys the caller set explicitly (the rest may follow replaced cabinets).
    given: Vec<String>,
    from: Option<f64>,
    to: Option<f64>,
    #[serde(default)]
    keep: Vec<String>,
}

/// A module ready to become a furniture group.
struct Pending {
    build: Build,
    role: crate::Role,
    from: f64,
    width: f64,
    position: Point2,
    angle: f64,
    elevation: f64,
}

struct Planned {
    request: Request,
    removed: Vec<FurnitureId>,
    modules: Vec<Pending>,
    notes: Vec<String>,
    /// Runs of other walls meeting this one in a corner, by tag.
    neighbors: Vec<String>,
}

fn joinery_kind(f: &Furniture) -> Option<String> {
    let stored = f.properties.get(PARAMS_KEY)?;
    let value: Value = serde_json::from_str(stored).ok()?;
    value["kind"].as_str().map(str::to_owned)
}

fn has_cooktop(f: &Furniture) -> bool {
    f.properties.get(PARAMS_KEY).is_some_and(|stored| {
        serde_json::from_str::<Value>(stored).is_ok_and(|v| {
            v["cooktop"] == true
                || v["cutouts"]
                    .as_array()
                    .is_some_and(|c| c.iter().any(|c| c["kind"] == "cooktop"))
        })
    })
}

fn row_key(row: RunRow) -> &'static str {
    match row {
        RunRow::Base => "base",
        RunRow::Wall => "wall",
        RunRow::Tall => "tall",
    }
}

/// Lowercase without accents, for matching names.
fn plain(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Cabinets drawn by hand or imported, recognized by name: they belong to
/// the run being redone. Appliances and fixtures never do.
fn named_cabinet(f: &Furniture, row: RunRow) -> bool {
    let full = plain(&f.name);
    // Designers number their modules: `7 — Armário portas ao lado do cooktop`.
    let name = full
        .split_once(" — ")
        .filter(|(head, _)| head.chars().all(|c| c.is_ascii_alphanumeric()))
        .map_or(full.as_str(), |(_, rest)| rest);
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    let cabinet = has(&[
        "armario",
        "gaveteiro",
        "gavetao",
        "gavetoes",
        "balcao",
        "gabinete",
        "modulo",
        "nicho",
        "paneleiro",
        "tamponamento",
        "arremate",
        "separador",
        "rodape",
    ]);
    let appliance = [
        "geladeira",
        "refrigerador",
        "fogao",
        "cooktop",
        "forno",
        "micro",
        "lava",
        "maquina",
        "cuba",
        "coifa",
        "tanque",
        "torneira",
        "pia",
    ]
    .iter()
    .any(|w| name.starts_with(w));
    if appliance && !cabinet {
        return false;
    }
    match row {
        RunRow::Base => cabinet || has(&["bancada", "tampo", "peninsula"]),
        RunRow::Wall => cabinet || name.starts_with("aereo"),
        RunRow::Tall => cabinet || has(&["torre", "despensa", "guarda-roupa", "roupeiro"]),
    }
}

/// Whether `p` is inside a polygon (ray casting).
fn inside(points: &[Point2], p: Point2) -> bool {
    let mut odd = false;
    let n = points.len();
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            odd = !odd;
        }
    }
    odd
}

/// Distance from a point to a wall's centerline segment, cm.
fn distance_to(wall: &newera_core::Wall, p: Point2) -> f64 {
    let (dx, dy) = (wall.end.x - wall.start.x, wall.end.y - wall.start.y);
    let len2 = (dx * dx + dy * dy).max(1e-9);
    let t = (((p.x - wall.start.x) * dx + (p.y - wall.start.y) * dy) / len2).clamp(0.0, 1.0);
    Point2::new(wall.start.x + dx * t, wall.start.y + dy * t).distance(p)
}

fn brazilian(v: f64) -> String {
    let rounded = (v * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}").replace('.', ",")
    }
}

/// Which wall, which face, which choices.
fn resolve(home: &Home, p: &CabinetRunParams) -> Result<Request, String> {
    let near = match &p.near {
        Some(id) => Some(
            home.furniture
                .iter()
                .find(|f| f.id.to_string() == *id)
                .ok_or_else(|| format!("{id} not found"))?
                .position,
        ),
        None => None,
    };
    let wall_id: WallId = match (&p.wall, near) {
        (Some(wall), _) => wall.parse().map_err(|e| format!("{e}"))?,
        (None, Some(at)) => {
            let view = home.level_view(home.current_level());
            view.walls
                .iter()
                .filter(|w| !w.is_arc())
                .min_by(|a, b| distance_to(a, at).total_cmp(&distance_to(b, at)))
                .ok_or("there are no straight walls")?
                .id
        }
        (None, None) => return Err("give `wall`, or `near` with a piece against it".into()),
    };
    let wall = home
        .wall(wall_id)
        .ok_or_else(|| format!("{wall_id} not found"))?;
    if wall.is_arc() {
        return Err(format!(
            "{wall_id} is curved: cabinets need a straight wall"
        ));
    }
    let given = p.p.clone().unwrap_or_default();
    let params: RunParams = serde_json::from_value(Value::Object(given.clone()))
        .map_err(|e| format!("invalid parameters: {e}"))?;
    let length = wall.start.distance(wall.end);
    let u = (
        (wall.end.x - wall.start.x) / length,
        (wall.end.y - wall.start.y) / length,
    );
    let mid = Point2::new(
        wall.start.x.midpoint(wall.end.x),
        wall.start.y.midpoint(wall.end.y),
    );
    let facing = match (&p.room, near) {
        (Some(room), _) => {
            let id: newera_core::RoomId = room.parse().map_err(|e| format!("{e}"))?;
            let room = home
                .rooms
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| format!("{room} not found"))?;
            newera_core::polygon_centroid(&room.points).unwrap_or(mid)
        }
        (None, Some(at)) => at,
        (None, None) => home.bounds().map_or(mid, |(a, b)| {
            Point2::new(a.x.midpoint(b.x), a.y.midpoint(b.y))
        }),
    };
    let side = if -u.1 * (facing.x - mid.x) + u.0 * (facing.y - mid.y) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    // Tall cabinets facing a bedroom are a wardrobe.
    let mut params = params;
    if params.row == RunRow::Tall && params.interior.is_none() {
        let normal = (-u.1 * side, u.0 * side);
        let front = Point2::new(mid.x + normal.0 * 60.0, mid.y + normal.1 * 60.0);
        let bedroom = home
            .rooms
            .iter()
            .find(|r| inside(&r.points, front))
            .is_some_and(|r| {
                let name = r.name.to_lowercase();
                ["quarto", "dormit", "suíte", "suite", "closet", "bedroom"]
                    .iter()
                    .any(|w| name.contains(w))
                    || home
                        .furniture
                        .iter()
                        .any(|f| f.catalog.starts_with("bed-") && inside(&r.points, f.position))
            });
        if bedroom {
            params.interior = Some(crate::Interior::Wardrobe);
        }
    }
    Ok(Request {
        wall: wall_id,
        side,
        params,
        given: given.keys().cloned().collect(),
        from: p.from,
        to: p.to,
        keep: p.keep.clone().unwrap_or_default(),
    })
}

/// Plans one run against `home` without changing it.
#[allow(clippy::too_many_lines)]
fn plan(home: &Home, request: &Request) -> Result<Planned, String> {
    let wall_id = request.wall;
    let side = request.side;
    let wall = home
        .wall(wall_id)
        .cloned()
        .ok_or_else(|| format!("{wall_id} not found"))?;
    let mut params = request.params.clone();
    let length = wall.start.distance(wall.end);
    let u = (
        (wall.end.x - wall.start.x) / length,
        (wall.end.y - wall.start.y) / length,
    );
    let wall_angle = u.1.atan2(u.0).to_degrees();
    let (from, to) = (
        request.from.unwrap_or(0.0).clamp(0.0, length),
        request.to.unwrap_or(length).clamp(0.0, length),
    );
    if to - from < 1.0 {
        return Err(format!(
            "from/to leave nothing of {wall_id} ({} cm long)",
            brazilian(length)
        ));
    }
    let row = row_key(params.row);
    let tag = format!("{wall_id}:{row}");
    let other_run = |f: &Furniture| {
        f.properties
            .get(RUN_KEY)
            .filter(|t| t.ends_with(&format!(":{row}")) && **t != tag)
            .cloned()
    };
    let catalog_cabinet = match params.row {
        RunRow::Base => "base-cabinet",
        RunRow::Wall => "wall-cabinet",
        RunRow::Tall => "wardrobe",
    };
    let (h, d, elev) = params.sizes();
    let z = match params.row {
        RunRow::Base => (0.0, h + 3.0),
        RunRow::Wall => (elev, elev + h),
        RunRow::Tall => (0.0, elev + h),
    };
    let band = if params.row == RunRow::Base {
        d + 3.0
    } else {
        d
    };
    // Pieces this run replaces: its own modules and loose cabinets in the band.
    let replaceable = |f: &Furniture| {
        if request.keep.contains(&f.id.to_string()) {
            return false;
        }
        // Another wall's run meeting this one stays, and so does anything
        // not parallel to this wall.
        if f.properties.get(RUN_KEY).is_some_and(|t| *t != tag) {
            return false;
        }
        if (f.angle - wall_angle).to_radians().sin().abs() > 0.1 {
            return false;
        }
        // Only pieces of this row: a tower is not a wall cabinet.
        let (lo, hi) = f.height_range();
        let this_row = match params.row {
            RunRow::Base => hi <= 100.0,
            RunRow::Wall => lo >= 100.0,
            RunRow::Tall => lo < 50.0 && hi > 150.0,
        };
        this_row
            && match joinery_kind(f).as_deref() {
                Some("cabinet" | "filler") => true,
                Some("countertop") => params.row == RunRow::Base,
                _ => f.catalog == catalog_cabinet || named_cabinet(f, params.row),
            }
    };
    let piece = |id: FurnitureId| home.furniture.iter().find(|f| f.id == id);
    let first = newera_core::wall_run(home, wall_id, side, band, z, &|_| false)
        .ok_or_else(|| format!("{wall_id} can't hold a run"))?;
    let inside = |o: &RunObstacle| {
        let c = o.from.midpoint(o.to);
        c >= from - 0.5 && c <= to + 0.5
    };
    let mut removed: Vec<FurnitureId> = Vec::new();
    for o in first.obstacles.iter().filter(|o| inside(o)) {
        if let RunBlock::Piece { id, .. } = &o.block
            && !removed.contains(id)
            && piece(*id).is_some_and(&replaceable)
        {
            removed.push(*id);
        }
    }
    // Keep the finish of the cabinets being replaced unless told otherwise.
    if let Some(Build::Cabinet(old)) = removed
        .iter()
        .filter_map(|id| piece(*id))
        .filter_map(|f| serde_json::from_str::<Build>(f.properties.get(PARAMS_KEY)?).ok())
        .find(|b| matches!(b, Build::Cabinet(_)))
    {
        let given = |key: &str| request.given.iter().any(|g| g == key);
        if !given("t") {
            params.t = old.t;
        }
        if !given("front") {
            params.front = old.front;
        }
        if !given("color") {
            params.color = old.color;
        }
    }
    // A sink or cooktop already set into the old countertop keeps its place:
    // the new run puts its cabinet and cutout right there.
    let mut embedded: Vec<FurnitureId> = Vec::new();
    let mut found_notes = Vec::new();
    if params.row == RunRow::Base && !removed.is_empty() {
        let along = |p: Point2| (p.x - wall.start.x) * u.0 + (p.y - wall.start.y) * u.1;
        for o in first.obstacles.iter().filter(|o| inside(o)) {
            let RunBlock::Piece { id, .. } = &o.block else {
                continue;
            };
            let Some(f) = piece(*id).filter(|f| !removed.contains(&f.id)) else {
                continue;
            };
            let (lo, hi) = f.height_range();
            let name = plain(&f.name);
            if lo < 50.0 || hi - lo > 45.0 || joinery_kind(f).is_some() {
                continue;
            }
            let center = along(f.position);
            if name.contains("cooktop") && params.cooktop.is_none() {
                params.cooktop = Some(center);
                params.cooktop_w = ((f.width + 6.0) / 5.0).ceil().max(12.0) * 5.0;
                embedded.push(f.id);
                found_notes.push("cooktop".to_owned());
            } else if (name.contains("pia") || name.contains("cuba") || name.starts_with("tanque"))
                && params.sink.is_none()
                && !name.contains("gavet")
                && !name.contains("armario")
            {
                params.sink = Some(center);
                params.sink_w = ((f.width + 10.0) / 5.0).ceil().max(12.0) * 5.0;
                embedded.push(f.id);
                found_notes.push("sink".to_owned());
            }
        }
    }
    let skip_top = params.row == RunRow::Base && !params.top;
    let skip = |f: &Furniture| {
        removed.contains(&f.id)
            || embedded.contains(&f.id)
            || (skip_top && joinery_kind(f).as_deref() == Some("countertop"))
    };
    let mut run = newera_core::wall_run(home, wall_id, side, band, z, &skip)
        .ok_or_else(|| format!("{wall_id} can't hold a run"))?;
    let named = |id: FurnitureId, words: &[&str]| {
        piece(id).is_some_and(|f| {
            let n = plain(&f.name);
            words.iter().any(|w| n.starts_with(w))
        })
    };
    // A stove or a cooktop cabinet; a countertop only holds its cooktop in one
    // place, handled where it matters (the hood gap).
    let heat = |id: FurnitureId, catalog: &str| {
        catalog == "stove"
            || catalog == "cooktop"
            || named(id, &["fogao"])
            || piece(id)
                .is_some_and(|f| has_cooktop(f) && joinery_kind(f).as_deref() != Some("countertop"))
    };
    let fridge = |id: FurnitureId, catalog: &str| {
        catalog == "fridge" || named(id, &["geladeira", "refrigerador"])
    };
    let hot_top = |id: FurnitureId| {
        piece(id)
            .is_some_and(|f| has_cooktop(f) && joinery_kind(f).as_deref() == Some("countertop"))
    };
    let mut notes = Vec::new();
    let mut neighbors: Vec<String> = Vec::new();
    let mut over = Vec::new();
    if params.row == RunRow::Wall {
        // Leave the hood's width free over a stove or cooktop.
        let below = newera_core::wall_run(home, wall_id, side, 60.0, (0.0, 95.0), &skip);
        let along = |p: Point2| (p.x - wall.start.x) * u.0 + (p.y - wall.start.y) * u.1;
        for o in below.iter().flat_map(|b| b.obstacles.iter()) {
            let RunBlock::Piece { id, catalog } = &o.block else {
                continue;
            };
            if !(heat(*id, catalog) || hot_top(*id))
                || run.obstacles.iter().any(|r| r.block == o.block)
            {
                continue;
            }
            // A countertop holds its cooktop somewhere along it: only there.
            let spans: Vec<(f64, f64)> =
                match piece(*id).filter(|f| joinery_kind(f).as_deref() == Some("countertop")) {
                    Some(top) => serde_json::from_str::<Value>(&top.properties[PARAMS_KEY])
                        .ok()
                        .and_then(|v| v["cutouts"].as_array().cloned())
                        .unwrap_or_default()
                        .iter()
                        .filter(|c| c["kind"] == "cooktop")
                        .filter_map(|c| {
                            let x = c["x"].as_f64()? - top.width / 2.0;
                            let half = c["w"].as_f64().unwrap_or(56.0).max(60.0) / 2.0;
                            let center = along(top.to_plan((x, 0.0)));
                            Some((center - half, center + half))
                        })
                        .collect(),
                    None => vec![(o.from, o.to)],
                };
            for (a, b) in spans {
                // A cooktop cabinet under its countertop's cutout is one hood.
                if run.obstacles.iter().any(|r| {
                    r.from < b - 5.0
                        && r.to > a + 5.0
                        && matches!(&r.block, RunBlock::Piece { id, catalog } if heat(*id, catalog) || hot_top(*id))
                }) {
                    continue;
                }
                run.obstacles.push(RunObstacle {
                    from: a.max(0.0),
                    to: b.min(length),
                    block: o.block.clone(),
                });
                notes.push(format!(
                    "Vão de {} cm deixado para a coifa sobre o fogão.",
                    brazilian(b - a)
                ));
            }
        }
        run.obstacles.sort_by(|a, b| a.from.total_cmp(&b.from));
        for o in &run.obstacles {
            if let RunBlock::Piece { id, catalog } = &o.block
                && fridge(*id, catalog)
                && let Some(f) = piece(*id)
            {
                over.push(RunOver {
                    from: o.from,
                    to: o.to,
                    bottom: f.height_range().1,
                });
            }
        }
    }
    let end_kind = |o: Option<&RunObstacle>| match o.map(|o| &o.block) {
        None => EndKind::Free,
        Some(RunBlock::Wall(_)) => EndKind::Wall,
        Some(RunBlock::Opening { .. }) => EndKind::Frame,
        Some(RunBlock::Piece { id, catalog }) => {
            if fridge(*id, catalog) {
                EndKind::Fridge
            } else if heat(*id, catalog) {
                // Over the stove the hood gap is exact; beside it, air.
                if params.row == RunRow::Wall {
                    EndKind::Kept
                } else {
                    EndKind::Heat
                }
            } else if let Some(f) = piece(*id).filter(|f| f.properties.contains_key(PARAMS_KEY)) {
                // Another wall's run (any row) owns the corner: fronts need a filler.
                if f.properties
                    .get(RUN_KEY)
                    .is_some_and(|t| !t.starts_with(&format!("{wall_id}:")))
                {
                    EndKind::Wall
                } else {
                    EndKind::Kept
                }
            } else {
                EndKind::Piece
            }
        }
    };
    for o in &run.obstacles {
        if let RunBlock::Piece { id, .. } = &o.block
            && let Some(t) = piece(*id).and_then(other_run)
            && !neighbors.contains(&t)
        {
            neighbors.push(t);
        }
    }
    // Other runs' cabinets just in front of this one: their fronts cover ours.
    let front = newera_core::wall_run(home, wall_id, side, d + DOOR_SWING, z, &skip);
    let covering: Vec<(f64, f64)> = front
        .iter()
        .flat_map(|r| r.obstacles.iter())
        .filter_map(|o| {
            let RunBlock::Piece { id, .. } = &o.block else {
                return None;
            };
            let f = piece(*id)?;
            let t = other_run(f)?;
            if !matches!(joinery_kind(f).as_deref(), Some("cabinet" | "filler")) {
                return None;
            }
            if !neighbors.contains(&t) {
                neighbors.push(t);
            }
            Some((o.from, o.to))
        })
        .collect();
    let gaps: Vec<RunGap> = run
        .gaps()
        .into_iter()
        .filter_map(|(a, b, before, after)| {
            let (a2, b2) = (a.max(from), b.min(to));
            if b2 - a2 <= 0.5 {
                return None;
            }
            let start = if a2 > a {
                EndKind::Kept
            } else {
                end_kind(before)
            };
            let end = if b2 < b {
                EndKind::Kept
            } else {
                end_kind(after)
            };
            let blind_start = if start == EndKind::Wall {
                covering
                    .iter()
                    .filter(|(f, t)| *f <= a2 + 1.0 && *t > a2 + 1.0)
                    .map(|(_, t)| t - a2 + CORNER_FILLER)
                    .fold(0.0, f64::max)
            } else {
                0.0
            };
            let blind_end = if end == EndKind::Wall {
                covering
                    .iter()
                    .filter(|(f, t)| *t >= b2 - 1.0 && *f < b2 - 1.0)
                    .map(|(f, _)| b2 - f + CORNER_FILLER)
                    .fold(0.0, f64::max)
            } else {
                0.0
            };
            Some(RunGap {
                from: a2,
                to: b2,
                start,
                end,
                blind_start,
                blind_end,
            })
        })
        .collect();
    let over: Vec<RunOver> = over
        .into_iter()
        .filter(|o| o.from >= from - 0.5 && o.to <= to + 0.5)
        .collect();
    // Joints of the other row on this wall, to line the modules up with.
    let other_row = match params.row {
        RunRow::Base => Some("wall"),
        RunRow::Wall => Some("base"),
        RunRow::Tall => None,
    };
    let joints: Vec<f64> = other_row
        .map(|r| format!("{wall_id}:{r}"))
        .map(|t| {
            home.furniture
                .iter()
                .filter(|f| f.properties.get(RUN_KEY) == Some(&t))
                .filter(|f| matches!(joinery_kind(f).as_deref(), Some("cabinet" | "filler")))
                .flat_map(|f| {
                    let c =
                        (f.position.x - wall.start.x) * u.0 + (f.position.y - wall.start.y) * u.1;
                    [c - f.width / 2.0, c + f.width / 2.0]
                })
                .collect()
        })
        .unwrap_or_default();
    // Old cabinets outside every free stretch (behind a shaft, beside the
    // laundry sink) stay: nothing would take their place.
    let along_of = |id: &FurnitureId| {
        piece(*id)
            .map(|f| (f.position.x - wall.start.x) * u.0 + (f.position.y - wall.start.y) * u.1)
    };
    let kept_back: Vec<FurnitureId> = removed
        .iter()
        .filter(|id| {
            along_of(id).is_some_and(|c| {
                !gaps.iter().any(|g| c >= g.from - 1.0 && c <= g.to + 1.0)
                    && !over.iter().any(|o| c >= o.from - 1.0 && c <= o.to + 1.0)
            })
        })
        .copied()
        .collect();
    let (modules, plan_notes) = crate::plan_run(&gaps, &over, &joints, &params)?;
    let removed: Vec<FurnitureId> = removed
        .into_iter()
        .filter(|id| !kept_back.contains(id))
        .collect();
    // Say where the sink or cooktop already there got its cabinet.
    for m in &modules {
        let (kind, text) = match m.role {
            crate::Role::Sink => ("sink", "Pia existente mantida"),
            crate::Role::Cooktop => ("cooktop", "Cooktop existente mantido"),
            _ => continue,
        };
        if found_notes.iter().any(|f| f == kind) {
            notes.insert(
                0,
                format!(
                    "{text}: módulo de {} cm embaixo, de {} a {} cm.",
                    brazilian(m.width),
                    brazilian(m.from),
                    brazilian(m.from + m.width)
                ),
            );
        }
    }
    notes.extend(plan_notes);
    let mut seen = std::collections::HashSet::new();
    notes.retain(|n| seen.insert(n.clone()));

    let angle = wall_angle + if side < 0.0 { 180.0 } else { 0.0 };
    let normal = (-u.1 * side, u.0 * side);
    let pending = modules
        .into_iter()
        .map(|m| {
            let mut build = m.build;
            // Turned around, the cabinet's left is the run's end.
            if side < 0.0
                && let Build::Cabinet(c) = &mut build
            {
                std::mem::swap(&mut c.blind_left, &mut c.blind_right);
            }
            let along = m.from + m.width / 2.0;
            let out = wall.thickness / 2.0 + m.depth / 2.0;
            Pending {
                build,
                role: m.role,
                from: m.from,
                width: m.width,
                position: Point2::new(
                    wall.start.x + u.0 * along + normal.0 * out,
                    wall.start.y + u.1 * along + normal.1 * out,
                ),
                angle,
                elevation: m.elevation,
            }
        })
        .collect();
    let request = Request {
        params,
        given: request.given.clone(),
        ..request.clone()
    };
    Ok(Planned {
        request,
        removed,
        modules: pending,
        notes,
        neighbors,
    })
}

/// Turns a plan into commands, taking furniture ids from `doc`.
fn commands(doc: &mut Document, planned: &Planned) -> Result<(Vec<Command>, Value), String> {
    let tag = format!(
        "{}:{}",
        planned.request.wall,
        row_key(planned.request.params.row)
    );
    // Replanned later, every explicit choice is already in `params`.
    let stored = serde_json::to_string(&Request {
        given: vec!["t".into(), "front".into(), "color".into()],
        keep: Vec::new(),
        ..planned.request.clone()
    })
    .map_err(|e| e.to_string())?;
    let mut list: Vec<Command> = planned
        .removed
        .iter()
        .map(|id| Command::remove(*id))
        .collect();
    let mut rows = Vec::new();
    for m in &planned.modules {
        let output = crate::generate(&m.build)?;
        let group_id = doc.new_furniture_id();
        let mut next = || doc.new_furniture_id();
        let mut group = crate::assemble(
            &m.build,
            &output,
            group_id,
            m.position,
            m.angle,
            m.elevation,
            &mut next,
        );
        group.properties.insert(RUN_KEY.into(), tag.clone());
        group.properties.insert(REQUEST_KEY.into(), stored.clone());
        list.push(Command::insert(group));
        rows.push(json!([
            group_id.to_string(),
            m.role,
            num(m.from),
            num(m.width)
        ]));
    }
    Ok((list, Value::Array(rows)))
}

/// Whether a new plan for a run builds exactly what is already there.
fn unchanged(home: &Home, tag: &str, planned: &Planned) -> bool {
    let key = |build: &str, p: Point2, elevation: f64| {
        format!("{build}@{:.1},{:.1},{:.1}", p.x, p.y, elevation)
    };
    let mut before: Vec<String> = home
        .furniture
        .iter()
        .filter(|f| f.properties.get(RUN_KEY).is_some_and(|t| t == tag))
        .filter_map(|f| Some(key(f.properties.get(PARAMS_KEY)?, f.position, f.elevation)))
        .collect();
    let mut after: Vec<String> = planned
        .modules
        .iter()
        .filter_map(|m| {
            Some(key(
                &serde_json::to_string(&m.build).ok()?,
                m.position,
                m.elevation,
            ))
        })
        .collect();
    before.sort();
    after.sort();
    before == after
}

#[allow(clippy::needless_pass_by_value)] // built for the reply
fn summary(planned: &Planned, modules: Value) -> Value {
    json!({
        "wall": planned.request.wall.to_string(),
        "row": row_key(planned.request.params.row),
        "modules": modules,
        "removed": planned.removed.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "notes": planned.notes,
    })
}

/// Plans (and unless `dry`, builds) the cabinets for one wall, and plans
/// again the runs it meets in a corner. Replies with the modules as
/// `[id, role, from, width]`, what was removed, notes and adjusted neighbors.
///
/// # Errors
/// Unknown walls or pieces, curved walls, and parameters no module can satisfy.
pub fn cabinet_run(doc: &mut Document, p: &CabinetRunParams) -> Result<Value, String> {
    let request = resolve(doc.home(), p)?;
    let planned = plan(doc.home(), &request)?;
    if p.dry {
        let rows: Vec<Value> = planned
            .modules
            .iter()
            .map(|m| json!(["", m.role, num(m.from), num(m.width)]))
            .collect();
        return Ok(summary(&planned, Value::Array(rows)));
    }
    // Work on a copy first: neighbors are planned against the new run.
    let mut scratch = Document::new(doc.home().clone());
    let (mut all, rows) = commands(&mut scratch, &planned)?;
    scratch
        .execute(Command::Batch {
            commands: all.clone(),
        })
        .map_err(|e| e.to_string())?;
    let mut reply = summary(&planned, rows);
    let mut adjusted = Vec::new();
    for tag in &planned.neighbors {
        let Some(stored) = scratch
            .home()
            .furniture
            .iter()
            .find(|f| f.properties.get(RUN_KEY) == Some(tag))
            .and_then(|f| f.properties.get(REQUEST_KEY))
            .and_then(|s| serde_json::from_str::<Request>(s).ok())
        else {
            continue;
        };
        let again = plan(scratch.home(), &stored)?;
        if unchanged(scratch.home(), tag, &again) {
            continue;
        }
        let (list, rows) = commands(&mut scratch, &again)?;
        scratch
            .execute(Command::Batch {
                commands: list.clone(),
            })
            .map_err(|e| e.to_string())?;
        all.extend(list);
        adjusted.push(summary(&again, rows));
    }
    // Same ids on the real document, then one undoable step.
    let last = scratch.new_furniture_id();
    while doc.new_furniture_id().0 + 1 < last.0 {}
    doc.execute(Command::Batch { commands: all })
        .map_err(|e| e.to_string())?;
    if !adjusted.is_empty() {
        reply["adjusted"] = Value::Array(adjusted);
    }
    Ok(reply)
}
