//! A run of cabinets along a wall: the free stretches between corners,
//! doors, windows and appliances are split into modules of good proportion,
//! so no stretch is left too small to use. Clearances the workshop expects
//! (fillers at corners, air beside the fridge, trim beside frames) are kept.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Build, CabinetParams, CountertopParams, DoorType, Output, Part, cm, num};

/// Which row of a kitchen or wardrobe wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunRow {
    /// Floor cabinets under a countertop.
    #[default]
    Base,
    /// Wall-hung cabinets.
    Wall,
    /// Floor-to-top towers (pantry, oven tower, wardrobe).
    Tall,
}

/// What bounds a free stretch at one end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndKind {
    /// Nothing: the wall just ends.
    Free,
    /// A wall corner: fronts need a filler to open past it.
    Wall,
    /// A door or window frame: trim around it.
    Frame,
    /// A fridge: air on its sides.
    Fridge,
    /// A stove or cooktop: heat.
    Heat,
    /// Any other appliance or piece.
    Piece,
    /// Joinery that stays as it is.
    Kept,
}

impl EndKind {
    /// Space left empty, cm.
    fn clearance(self) -> f64 {
        match self {
            Self::Fridge => FRIDGE_AIR,
            Self::Frame => FRAME_TRIM,
            Self::Heat => HEAT_GAP,
            Self::Piece => 1.0,
            Self::Free | Self::Wall | Self::Kept => 0.0,
        }
    }

    /// Closed filler panel, cm.
    fn filler(self) -> f64 {
        if self == Self::Wall {
            CORNER_FILLER
        } else {
            0.0
        }
    }
}

/// A free stretch, cm along the wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunGap {
    pub from: f64,
    pub to: f64,
    pub start: EndKind,
    pub end: EndKind,
    /// Blind corner at the start: cm covered by another wall's run in front.
    pub blind_start: f64,
    /// Blind corner at the end, cm.
    pub blind_end: f64,
}

/// A stretch where the row passes over something (a fridge) and a shorter
/// cabinet fits above it: `from`, `to` along the wall, `bottom` its top, cm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunOver {
    pub from: f64,
    pub to: f64,
    pub bottom: f64,
}

/// Choices for a run. Sizes in cm; unset ones follow the row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RunParams {
    pub row: RunRow,
    /// Cabinet height (base 87 under a 3 cm top, wall 70, tall 220).
    pub h: Option<f64>,
    /// Depth including doors (base 55, wall 35, tall 58).
    pub d: Option<f64>,
    /// Bottom above the floor (wall 150, others 0).
    pub elev: Option<f64>,
    /// Board thickness, mm (default 18).
    pub t: f64,
    /// Front finish, e.g. `wood` or `#5f6e4a`.
    pub front: Option<String>,
    /// Carcass color `[r,g,b]`.
    pub color: Option<[u8; 3]>,
    /// Modules made drawer units (default: 1 on a base row, next to the stove).
    pub drawers: Option<u32>,
    /// Widest module, cm (default 90).
    pub max: f64,
    /// Preferred module width, cm (default 60).
    pub target: f64,
    /// Base row: one countertop over each stretch (default true).
    pub top: bool,
    /// Countertop finish (default `stone`).
    pub top_material: Option<String>,
}

impl Default for RunParams {
    fn default() -> Self {
        Self {
            row: RunRow::Base,
            h: None,
            d: None,
            elev: None,
            t: 18.0,
            front: None,
            color: None,
            drawers: None,
            max: 90.0,
            target: 60.0,
            top: true,
            top_material: None,
        }
    }
}

impl RunParams {
    /// Height, depth and elevation for the row.
    pub fn sizes(&self) -> (f64, f64, f64) {
        let (h, d, elev) = match self.row {
            RunRow::Base => (87.0, 55.0, 0.0),
            RunRow::Wall => (70.0, 35.0, 150.0),
            RunRow::Tall => (220.0, 58.0, 0.0),
        };
        (
            self.h.unwrap_or(h),
            self.d.unwrap_or(d),
            self.elev.unwrap_or(elev),
        )
    }
}

/// Air beside a fridge, cm.
const FRIDGE_AIR: f64 = 5.0;
/// Frame trim (alizar) beside doors and windows, cm.
const FRAME_TRIM: f64 = 5.0;
/// Gap beside a freestanding stove, cm.
const HEAT_GAP: f64 = 2.0;
/// Filler at a wall corner so doors and handles clear the other wall, cm.
const CORNER_FILLER: f64 = 3.0;
/// Narrowest useful module (spice pull-out), cm.
const SLIM: f64 = 15.0;
/// Narrowest regular module, cm.
const NARROW: f64 = 30.0;
/// Countertop overhang past the fronts, cm.
const TOP_OVERHANG: f64 = 3.0;
/// Countertop thickness, cm.
const TOP_T: f64 = 3.0;

/// What a module is for, as reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Doors,
    Drawers,
    /// 15–30 cm: spice or tray pull-out.
    Slim,
    /// Blind corner: a fixed panel where the other wall's run meets it.
    Corner,
    /// Above a fridge.
    Over,
    Filler,
    Countertop,
}

/// One build of the run, placed along the wall.
#[derive(Debug, Clone, PartialEq)]
pub struct RunModule {
    /// Left end along the wall, cm.
    pub from: f64,
    pub width: f64,
    pub depth: f64,
    pub elevation: f64,
    pub role: Role,
    pub build: Build,
}

/// A filler panel (tamponamento) closing a stretch too small for a module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct FillerParams {
    /// Width, cm.
    pub w: f64,
    /// Height, cm.
    pub h: f64,
    /// Depth to the front plane, cm.
    pub d: f64,
    /// Board, mm (default 18).
    pub t: f64,
    pub color: Option<[u8; 3]>,
    pub front: Option<String>,
}

impl Default for FillerParams {
    fn default() -> Self {
        Self {
            w: 3.0,
            h: 87.0,
            d: 55.0,
            t: 18.0,
            color: None,
            front: None,
        }
    }
}

pub(crate) fn filler(p: &FillerParams) -> Result<Output, String> {
    if p.w <= 0.0 || p.h <= 0.0 || p.d < cm(p.t) {
        return Err("O tamponamento precisa de largura e altura positivas e profundidade maior que a chapa.".into());
    }
    let t = cm(p.t);
    let mut part = Part::board(
        "Tamponamento",
        [0.0, p.d - t, 0.0],
        [p.w, t, p.h],
        &format!("MDF {}", num(p.t)),
        p.color.unwrap_or(crate::MDF_WHITE),
    )
    .banded(2, 0);
    part.finish = p
        .front
        .as_deref()
        .map(str::parse::<newera_core::Material>)
        .transpose()?;
    Ok(Output {
        parts: vec![part],
        size: [p.w, p.d, p.h],
        hardware: Vec::new(),
        notes: Vec::new(),
        extra_cuts: Vec::new(),
        name: format!("Tamponamento {} cm", num(p.w)),
    })
}

/// How many equal modules split `length`: widths within `NARROW..=max`,
/// closest to `target`, fewer modules when it is a tie.
fn split(length: f64, target: f64, max: f64) -> u32 {
    let lo = (length / max).ceil().max(1.0) as u32;
    let hi = ((length / NARROW).floor() as u32).max(lo);
    (lo..=hi)
        .min_by(|a, b| {
            let score = |n: u32| (length / f64::from(n) - target).abs() + 2.0 * f64::from(n);
            score(*a).total_cmp(&score(*b))
        })
        .unwrap_or(1)
}

/// Plans the modules for the free stretches of one row.
///
/// # Errors
/// Parameters that make no module possible, as a sentence with the fix.
#[allow(clippy::too_many_lines)]
pub fn plan_run(
    gaps: &[RunGap],
    over: &[RunOver],
    p: &RunParams,
) -> Result<(Vec<RunModule>, Vec<String>), String> {
    let (h, d, elev) = p.sizes();
    if !(NARROW..=120.0).contains(&p.max) || p.target < NARROW || p.target > p.max {
        return Err(format!(
            "Use max entre {} e 120 cm e target entre {} e max (hoje max = {}, target = {}).",
            num(NARROW),
            num(NARROW),
            num(p.max),
            num(p.target)
        ));
    }
    let plinth = if p.row == RunRow::Wall { 0.0 } else { 10.0 };
    let shelves = match p.row {
        RunRow::Base | RunRow::Wall => 1,
        RunRow::Tall => 4,
    };
    let cabinet = |w: f64, door: DoorType, drawers: u32, h: f64, shelves: u32| CabinetParams {
        w,
        h,
        d,
        t: p.t,
        door,
        drawers,
        shelves,
        plinth,
        color: p.color,
        front: p.front.clone(),
        ..CabinetParams::default()
    };
    let mut modules: Vec<RunModule> = Vec::new();
    let mut notes = Vec::new();
    let mut tops: Vec<(f64, f64)> = Vec::new();
    // Where a drawer unit helps most: next to the stove.
    let mut heat_ends: Vec<f64> = Vec::new();
    for g in gaps {
        if g.start == EndKind::Heat {
            heat_ends.push(g.from);
        }
        if g.end == EndKind::Heat {
            heat_ends.push(g.to);
        }
        let (c0, c1) = (g.start.clearance(), g.end.clearance());
        let (b0, b1) = (g.blind_start.max(0.0), g.blind_end.max(0.0));
        // A blind panel closes the corner by itself.
        let f0 = if b0 > 0.0 { 0.0 } else { g.start.filler() };
        let f1 = if b1 > 0.0 { 0.0 } else { g.end.filler() };
        let usable = g.to - g.from - c0 - c1 - f0 - f1;
        let mut x = g.from + c0;
        if c0 + c1 > 0.0 && g.to - g.from > c0 + c1 {
            for (kind, c) in [(g.start, c0), (g.end, c1)] {
                if c > 0.0 {
                    let why = match kind {
                        EndKind::Fridge => "ventilação da geladeira",
                        EndKind::Frame => "guarnição do vão",
                        EndKind::Heat => "folga do fogão",
                        _ => "folga",
                    };
                    let note = format!("{} cm livres para {why}.", num(c));
                    if !notes.contains(&note) {
                        notes.push(note);
                    }
                }
            }
        }
        if usable <= 0.5 {
            if g.to - g.from - c0 - c1 > 0.5 {
                // Only room for the fillers: close it whole.
                let w = g.to - g.from - c0 - c1;
                modules.push(filler_module(x, w, h, d, elev, p));
            }
            continue;
        }
        if usable < SLIM {
            // Too small for a module: one filler closes it, corner fillers included.
            let w = usable + f0 + f1;
            modules.push(filler_module(x, w, h, d, elev, p));
            notes.push(format!(
                "Vão de {} cm em {} cm fechado com tamponamento: um módulo útil precisa de {} cm.",
                num(w),
                num(x),
                num(SLIM)
            ));
            tops.push((g.from + c0, x + w));
            continue;
        }
        if f0 > 0.0 {
            modules.push(filler_module(x, f0, h, d, elev, p));
            x += f0;
        }
        if b0 + b1 > 0.0 && usable - b0 - b1 < NARROW {
            // No room for a door beside the blind panel: close the corner.
            modules.push(filler_module(x, usable, h, d, elev, p));
            notes.push(format!(
                "Canto de {} cm em {} cm fechado: não sobra porta de {} cm ao lado da outra bancada.",
                num(usable),
                num(x),
                num(NARROW)
            ));
            x += usable;
        } else if b0 + b1 > 0.0 {
            let open = usable - b0 - b1;
            let n = split(open, p.target, p.max);
            let each = (open / f64::from(n) * 10.0).round() / 10.0;
            for k in 0..n {
                let door = if k + 1 == n {
                    open - each * f64::from(n - 1)
                } else {
                    each
                };
                let left = if k == 0 { b0 } else { 0.0 };
                let right = if k + 1 == n { b1 } else { 0.0 };
                let w = door + left + right;
                let mut params = cabinet(w, DoorType::Hinged, 0, h, shelves);
                params.blind_left = left;
                params.blind_right = right;
                modules.push(RunModule {
                    from: x,
                    width: w,
                    depth: d,
                    elevation: elev,
                    role: if left + right > 0.0 {
                        Role::Corner
                    } else {
                        Role::Doors
                    },
                    build: Build::Cabinet(params),
                });
                x += w;
            }
            notes.push(
                "Canto em L: módulo de canto cego com painel fixo onde a outra bancada encosta."
                    .into(),
            );
        } else if usable < NARROW {
            modules.push(RunModule {
                from: x,
                width: usable,
                depth: d,
                elevation: elev,
                role: Role::Slim,
                build: Build::Cabinet(cabinet(
                    usable,
                    DoorType::Hinged,
                    0,
                    h,
                    if p.row == RunRow::Wall { 1 } else { 2 },
                )),
            });
            x += usable;
        } else {
            let n = split(usable, p.target, p.max);
            // Tenths of a millimeter never add up exactly: the last one takes the rest.
            let each = (usable / f64::from(n) * 10.0).round() / 10.0;
            for k in 0..n {
                let w = if k + 1 == n {
                    usable - each * f64::from(n - 1)
                } else {
                    each
                };
                modules.push(RunModule {
                    from: x,
                    width: w,
                    depth: d,
                    elevation: elev,
                    role: Role::Doors,
                    build: Build::Cabinet(cabinet(w, DoorType::Hinged, 0, h, shelves)),
                });
                x += w;
            }
        }
        if f1 > 0.0 {
            modules.push(filler_module(x, f1, h, d, elev, p));
            x += f1;
        }
        tops.push((g.from + c0, x));
    }

    // Drawer units: on a base row, the modules closest to the stove.
    let wanted = p.drawers.unwrap_or(u32::from(p.row == RunRow::Base));
    if wanted > 0 {
        let mut eligible: Vec<usize> = modules
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role == Role::Doors && (40.0..=90.0).contains(&m.width))
            .map(|(i, _)| i)
            .collect();
        let distance = |m: &RunModule| {
            heat_ends
                .iter()
                .map(|e| (m.from + m.width / 2.0 - e).abs())
                .fold(f64::MAX, f64::min)
        };
        eligible.sort_by(|a, b| distance(&modules[*a]).total_cmp(&distance(&modules[*b])));
        let drawer_rows = if h - plinth >= 70.0 { 3 } else { 2 };
        for &i in eligible.iter().take(wanted as usize) {
            let m = &mut modules[i];
            m.role = Role::Drawers;
            m.build = Build::Cabinet(cabinet(m.width, DoorType::Drawers, drawer_rows, h, 0));
        }
        if eligible.len() < wanted as usize {
            notes.push(format!(
                "Só {} módulo(s) têm entre 40 e 90 cm para virar gaveteiro; pedidos {wanted}.",
                eligible.len()
            ));
        }
    }

    // Above a fridge, a shorter cabinet up to the row's top line.
    let top_line = elev + h;
    for o in over {
        let height = top_line - o.bottom - 2.0;
        if o.to - o.from < NARROW || height < 25.0 {
            continue;
        }
        let w = o.to - o.from;
        let mut params = cabinet(w, DoorType::Hinged, 0, height, 0);
        params.plinth = 0.0;
        params.doors = Some(if w > 60.0 { 2 } else { 1 });
        modules.push(RunModule {
            from: o.from,
            width: w,
            depth: d,
            elevation: top_line - height,
            role: Role::Over,
            build: Build::Cabinet(params),
        });
    }

    if p.row == RunRow::Base && p.top {
        for (from, to) in tops.into_iter().filter(|(a, b)| b - a >= 20.0) {
            modules.push(RunModule {
                from,
                width: to - from,
                depth: d + TOP_OVERHANG,
                elevation: 0.0,
                role: Role::Countertop,
                build: Build::Countertop(CountertopParams {
                    length: to - from,
                    depth: d + TOP_OVERHANG,
                    height: h + TOP_T,
                    thickness: TOP_T,
                    material: p.top_material.clone(),
                    ..CountertopParams::default()
                }),
            });
        }
    }
    // Every build must stand on its own rules.
    for m in &modules {
        crate::generate(&m.build)
            .map_err(|e| format!("Módulo de {} cm em {} cm: {e}", num(m.width), num(m.from)))?;
    }
    modules.sort_by(|a, b| a.from.total_cmp(&b.from));
    Ok((modules, notes))
}

fn filler_module(from: f64, w: f64, h: f64, d: f64, elev: f64, p: &RunParams) -> RunModule {
    RunModule {
        from,
        width: w,
        depth: d,
        elevation: elev,
        role: Role::Filler,
        build: Build::Filler(FillerParams {
            w,
            h,
            d,
            t: p.t,
            color: p.color,
            front: p.front.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    fn gap(from: f64, to: f64, start: EndKind, end: EndKind) -> RunGap {
        RunGap {
            from,
            to,
            start,
            end,
            blind_start: 0.0,
            blind_end: 0.0,
        }
    }

    fn cabinets(modules: &[RunModule]) -> Vec<(Role, f64)> {
        modules
            .iter()
            .filter(|m| m.role != Role::Countertop)
            .map(|m| (m.role, (m.width * 10.0).round() / 10.0))
            .collect()
    }

    #[test]
    fn stretches_split_into_even_modules_without_leftovers() {
        // Corner at 0, fridge from 250: 3 cm filler, 5 cm air, 242 cm of modules.
        let (modules, notes) = plan_run(
            &[gap(0.0, 250.0, EndKind::Wall, EndKind::Fridge)],
            &[],
            &RunParams::default(),
        )
        .unwrap();
        let sizes = cabinets(&modules);
        assert_eq!(sizes[0], (Role::Filler, 3.0));
        let widths: f64 = sizes.iter().map(|s| s.1).sum();
        assert!((widths - 245.0).abs() < 1e-6, "{sizes:?}");
        // 242 cm → four modules of 60,5 cm, one of them a drawer unit.
        assert_eq!(sizes.len(), 5, "{sizes:?}");
        assert!(sizes[1..].iter().all(|s| (s.1 - 60.5).abs() < 0.11));
        assert_eq!(sizes.iter().filter(|s| s.0 == Role::Drawers).count(), 1);
        assert!(notes.iter().any(|n| n.contains("ventilação")), "{notes:?}");
        // One countertop over modules and filler, stopping at the fridge air.
        let top = modules.iter().find(|m| m.role == Role::Countertop).unwrap();
        assert!((top.from - 0.0).abs() < 1e-9 && (top.width - 245.0).abs() < 1e-6);
        let last = modules
            .iter()
            .rfind(|m| m.role != Role::Countertop)
            .unwrap();
        assert!((last.from + last.width - 245.0).abs() < 1e-6);
    }

    #[test]
    fn the_drawer_unit_goes_beside_the_stove_and_small_stretches_are_used() {
        let (modules, notes) = plan_run(
            &[
                gap(0.0, 200.0, EndKind::Free, EndKind::Heat),
                gap(276.0, 300.0, EndKind::Heat, EndKind::Wall),
                gap(400.0, 410.0, EndKind::Frame, EndKind::Free),
            ],
            &[],
            &RunParams::default(),
        )
        .unwrap();
        let sizes = cabinets(&modules);
        // Modules before the stove: the one touching it has the drawers.
        let before: Vec<&RunModule> = modules
            .iter()
            .filter(|m| m.role != Role::Countertop && m.from < 200.0)
            .collect();
        assert_eq!(before.last().unwrap().role, Role::Drawers, "{sizes:?}");
        // 24 cm after the stove − 2 cm heat gap − 3 cm filler = 19 cm pull-out.
        assert!(sizes.contains(&(Role::Slim, 19.0)), "{sizes:?}");
        // 10 cm − 5 cm trim → closed with a filler, and said so.
        assert!(sizes.contains(&(Role::Filler, 5.0)), "{sizes:?}");
        assert!(
            notes.iter().any(|n| n.contains("tamponamento")),
            "{notes:?}"
        );
    }

    #[test]
    fn corners_under_another_run_become_blind_modules() {
        // 7,5..300 with the other run's fronts covering 58 cm from the corner.
        let mut g = gap(7.5, 300.0, EndKind::Wall, EndKind::Wall);
        g.blind_start = 58.0;
        let (modules, notes) = plan_run(&[g], &[], &RunParams::default()).unwrap();
        let sizes = cabinets(&modules);
        // No filler at the blind end, one at the far corner.
        assert_eq!(sizes.first().unwrap().0, Role::Corner, "{sizes:?}");
        assert_eq!(*sizes.last().unwrap(), (Role::Filler, 3.0));
        let Build::Cabinet(corner) = &modules
            .iter()
            .find(|m| m.role == Role::Corner)
            .unwrap()
            .build
        else {
            panic!("corner is a cabinet")
        };
        assert_eq!(corner.blind_left, 58.0);
        // Doors all the same width: the corner's door equals its neighbours'.
        let doors: Vec<f64> = sizes
            .iter()
            .filter(|s| s.0 == Role::Doors || s.0 == Role::Drawers)
            .map(|s| s.1)
            .collect();
        assert!(
            doors.iter().all(|w| (w - (corner.w - 58.0)).abs() < 0.2),
            "{sizes:?}"
        );
        assert!(notes.iter().any(|n| n.contains("canto cego")));
        let total: f64 = sizes.iter().map(|s| s.1).sum();
        assert!((total - 292.5).abs() < 1e-6);
    }

    #[test]
    fn wall_rows_add_a_cabinet_over_the_fridge() {
        let params = RunParams {
            row: RunRow::Wall,
            ..RunParams::default()
        };
        let (modules, _) = plan_run(
            &[gap(0.0, 90.0, EndKind::Free, EndKind::Free)],
            &[RunOver {
                from: 100.0,
                to: 170.0,
                bottom: 180.0,
            }],
            &params,
        )
        .unwrap();
        let over = modules.iter().find(|m| m.role == Role::Over).unwrap();
        // Top line 220, fridge top 180, 2 cm air: 38 cm tall from 182.
        assert!((over.elevation - 182.0).abs() < 1e-9 && over.width == 70.0);
        assert!(modules.iter().all(|m| m.role != Role::Countertop));
        assert!(modules.iter().all(|m| m.role != Role::Drawers));
        assert!(
            plan_run(
                &[],
                &[],
                &RunParams {
                    max: 20.0,
                    ..RunParams::default()
                }
            )
            .is_err()
        );
    }
}
