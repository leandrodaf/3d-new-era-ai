//! Modular cabinets in sheet material: carcass, back in a groove, plinth,
//! dividers, shelves, drawers and doors with workshop clearances.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{MDF_WHITE, Output, Part, cm, num};

/// Front of a cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DoorType {
    /// Open shelving.
    None,
    /// Overlay doors on cup hinges.
    #[default]
    Hinged,
    /// Two or more leaves on a track.
    Sliding,
    /// Drawers over the whole height.
    Drawers,
}

/// A cabinet. Sizes in cm, board thicknesses in mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CabinetParams {
    /// Width, cm (default 100).
    pub w: f64,
    /// Height including plinth, cm (default 210).
    pub h: f64,
    /// Depth including doors, cm (default 55).
    pub d: f64,
    /// Carcass and door board, mm: 15, 18 or 25 (default 18).
    pub t: f64,
    /// Back panel, mm, set in a groove (default 6).
    pub back: f64,
    pub door: DoorType,
    /// Number of doors or leaves (default: one per 60 cm).
    pub doors: Option<u32>,
    /// Shelves per bay (default 2).
    pub shelves: u32,
    /// Drawers at the bottom of each bay (with `door: drawers`, the drawer count).
    pub drawers: u32,
    /// Vertical dividers making bays (default: as many as the shelves need).
    pub dividers: Option<u32>,
    /// Plinth height, cm (default 10; 0 for wall cabinets).
    pub plinth: f64,
    /// Holds a cooktop on top: needs the depth for its niche.
    pub cooktop: bool,
    /// Carcass color `[r,g,b]` (default white MDF).
    pub color: Option<[u8; 3]>,
    /// Door and drawer front finish, e.g. `wood` or `#5f6e4a`.
    pub front: Option<String>,
    /// Blind corner: cm of the front on the left covered by a fixed panel
    /// (where a cabinet run on the other wall meets this one).
    pub blind_left: f64,
    /// Blind corner on the right, cm.
    pub blind_right: f64,
    /// Wardrobe: a hanging rail under a top shelf (maleiro) instead of shelves.
    pub rod: bool,
    /// Open niches for built-in appliances (oven, microwave): no door across them.
    pub niches: Vec<Niche>,
}

/// A doorless opening across the cabinet's inside, cm.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Niche {
    /// Height of its floor above the room floor.
    pub bottom: f64,
    /// Free height inside.
    pub height: f64,
}

impl Default for CabinetParams {
    fn default() -> Self {
        Self {
            w: 100.0,
            h: 210.0,
            d: 55.0,
            t: 18.0,
            back: 6.0,
            door: DoorType::Hinged,
            doors: None,
            shelves: 2,
            drawers: 0,
            dividers: None,
            plinth: 10.0,
            cooktop: false,
            color: None,
            front: None,
            blind_left: 0.0,
            blind_right: 0.0,
            rod: false,
            niches: Vec::new(),
        }
    }
}

/// Gap between doors and around fronts, cm (2 mm, for cup hinges).
const GAP: f64 = 0.2;
/// Groove for the back, cm from the rear edge.
const BACK_INSET: f64 = 1.0;
/// Groove depth into sides, top and bottom, cm.
const GROOVE: f64 = 0.8;
/// Telescopic slide lengths on sale, cm.
const SLIDES: [f64; 8] = [25.0, 30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0];
/// Clearance per side for drawer slides, cm.
const SLIDE_CLEARANCE: f64 = 1.3;
/// Widest single hinged door, cm.
const MAX_DOOR: f64 = 60.0;
/// Cooktops need this much depth for their niche, cm.
const COOKTOP_NICHE: f64 = 50.0;

/// Where shelves go: the whole zone, or with niches the tallest stretch
/// left between them. Returns `(floor, height)`.
fn shelves_zone(niches: &[(f64, f64)], floor: f64, zone: f64, t: f64) -> (f64, f64) {
    let mut best = (floor, if niches.is_empty() { zone } else { 0.0 });
    let mut from = floor;
    for &(nb, nt) in niches {
        let height = (nb - t - from).max(0.0);
        if height > best.1 {
            best = (from, height);
        }
        from = nt + t;
    }
    if !niches.is_empty() {
        let height = (floor + zone - from).max(0.0);
        if height > best.1 {
            best = (from, height);
        }
    }
    best
}

/// Cup hinges a door of `height` cm needs.
fn hinges(height: f64) -> u32 {
    match height {
        h if h <= 90.0 => 2,
        h if h <= 160.0 => 3,
        h if h <= 220.0 => 4,
        _ => 5,
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn generate(p: &CabinetParams) -> Result<Output, String> {
    let (w, h, d) = (p.w, p.h, p.d);
    if ![15.0, 18.0, 25.0].contains(&p.t) {
        return Err(format!(
            "Chapa de {} mm não é padrão para caixaria; use t = 15, 18 ou 25.",
            num(p.t)
        ));
    }
    let t = cm(p.t);
    let back = cm(p.back.clamp(3.0, 18.0));
    let board = format!("MDF {}", num(p.t));
    let back_board = format!("MDF {}", num(p.back));
    let carcass = p.color.unwrap_or(MDF_WHITE);
    let front_finish = p
        .front
        .as_deref()
        .map(str::parse::<newera_core::Material>)
        .transpose()?;
    let front_color = carcass;
    if w < 2.0 * t + 10.0 || h < 20.0 || d < 20.0 {
        return Err(format!(
            "Armário de {} × {} × {} cm é pequeno demais: mínimo {} cm de largura e 20 cm de altura e profundidade.",
            num(w),
            num(d),
            num(h),
            num(2.0 * t + 10.0)
        ));
    }
    if p.cooktop && d < COOKTOP_NICHE {
        return Err(format!(
            "A profundidade do armário é de {} cm, mas o cooktop exige nicho mínimo de {} cm. Ajuste a profundidade para 55 cm.",
            num(d),
            num(COOKTOP_NICHE)
        ));
    }
    let (blind_l, blind_r) = (p.blind_left.max(0.0), p.blind_right.max(0.0));
    if blind_l + blind_r > 0.0 {
        if p.door != DoorType::Hinged || p.drawers > 0 {
            return Err(
                "Canto cego só com portas de giro e sem gavetas: use door = hinged e drawers = 0."
                    .into(),
            );
        }
        if w - blind_l - blind_r < 30.0 {
            return Err(format!(
                "Com {} cm de painel cego sobram {} cm de porta (mínimo 30); use w = {}.",
                num(blind_l + blind_r),
                num(w - blind_l - blind_r),
                num(blind_l + blind_r + 30.0)
            ));
        }
    }
    let plinth = p.plinth.max(0.0);
    let front_t = match p.door {
        DoorType::None => 0.0,
        DoorType::Hinged | DoorType::Drawers => t,
        // Two tracks of leaves.
        DoorType::Sliding => 2.0 * t + 1.0,
    };
    let dc = d - front_t;
    let hc = h - plinth;
    if dc < 20.0 || hc < 2.0 * t + 10.0 {
        return Err(format!(
            "Com frentes de {} cm e rodapé de {} cm sobra caixa de {} × {} cm: aumente d ou h, ou reduza plinth.",
            num(front_t),
            num(plinth),
            num(dc),
            num(hc)
        ));
    }
    let (iw, ih) = (w - 2.0 * t, hc - 2.0 * t);
    let id = dc - BACK_INSET - back;
    let mut parts = Vec::new();
    let mut hardware = Vec::new();
    let mut notes = Vec::new();

    // Carcass.
    parts.push(
        Part::board(
            "Lateral esquerda",
            [0.0, 0.0, plinth],
            [t, dc, hc],
            &board,
            carcass,
        )
        .banded(1, 0),
    );
    parts.push(
        Part::board(
            "Lateral direita",
            [w - t, 0.0, plinth],
            [t, dc, hc],
            &board,
            carcass,
        )
        .banded(1, 0),
    );
    parts.push(Part::board("Base", [t, 0.0, plinth], [iw, dc, t], &board, carcass).banded(1, 0));
    parts.push(Part::board("Tampo", [t, 0.0, h - t], [iw, dc, t], &board, carcass).banded(1, 0));
    parts.push(Part::board(
        "Fundo",
        [t - GROOVE, BACK_INSET, plinth + t - GROOVE],
        [iw + 2.0 * GROOVE, back, ih + 2.0 * GROOVE],
        &back_board,
        carcass,
    ));
    if plinth > 0.0 {
        parts.push(
            Part::board(
                "Rodapé frontal",
                [t, dc - 5.0 - t, 0.0],
                [iw, t, plinth],
                &board,
                carcass,
            )
            .banded(1, 0),
        );
        parts.push(Part::board(
            "Travessa traseira do rodapé",
            [t, BACK_INSET, 0.0],
            [iw, t, plinth],
            &board,
            carcass,
        ));
    }

    // Bays: shelves longer than the board can span get dividers.
    let span_limit = if p.t >= 25.0 {
        120.0
    } else if p.t >= 18.0 {
        90.0
    } else {
        70.0
    };
    // Hanging rails sag past about a meter.
    let span_limit = if p.rod {
        f64::min(span_limit, 100.0)
    } else {
        span_limit
    };
    let needed = if p.shelves > 0 || p.rod {
        (((iw + t) / (span_limit + t)).ceil() as u32).saturating_sub(1)
    } else {
        0
    };
    // A niche takes the whole inside: no automatic dividers through it.
    let dividers = p
        .dividers
        .unwrap_or(if p.niches.is_empty() { needed } else { 0 });
    let bays = dividers + 1;
    let bay = (iw - f64::from(dividers) * t) / f64::from(bays);
    if bay < 15.0 {
        return Err(format!(
            "Com {} divisões cada vão fica com {} cm; use no máximo {} divisões nesta largura.",
            dividers,
            num(bay),
            (((iw + t) / (15.0 + t)).floor() as u32).saturating_sub(1)
        ));
    }
    if bay > span_limit && (p.shelves > 0 || p.rod) {
        return Err(format!(
            "Prateleiras de {} mm vencem no máximo {} cm; cada vão tem {} cm. Use dividers = {} ou chapa mais grossa.",
            num(p.t),
            num(span_limit),
            num(bay),
            needed.max(1)
        ));
    }
    for k in 0..dividers {
        let x = t + f64::from(k + 1) * bay + f64::from(k) * t;
        parts.push(
            Part::board(
                &format!("Divisória {}", k + 1),
                [x, BACK_INSET + back, plinth + t],
                [t, id, ih],
                &board,
                carcass,
            )
            .banded(1, 0),
        );
    }

    // Drawers: the whole front, or a stack at the bottom of each bay.
    let drawer_count = if p.door == DoorType::Drawers {
        p.drawers.max(1)
    } else {
        p.drawers
    };
    let drawer_zone = if p.door == DoorType::Drawers {
        ih
    } else {
        f64::from(drawer_count) * 18.0
    };
    if drawer_zone > ih {
        return Err(format!(
            "{} gavetas de 18 cm não cabem no vão interno de {} cm; use drawers = {} ou door = drawers.",
            drawer_count,
            num(ih),
            (ih / 18.0).floor() as u32
        ));
    }
    if drawer_count > 0 {
        let front_h = drawer_zone / f64::from(drawer_count);
        if front_h < 12.0 {
            return Err(format!(
                "Gavetas com frente de {} cm são baixas demais (mínimo 12 cm); use drawers = {}.",
                num(front_h),
                (drawer_zone / 12.0).floor() as u32
            ));
        }
        let slide = SLIDES.iter().rev().copied().find(|s| *s <= id - 2.0).ok_or_else(|| {
            format!(
                "A profundidade interna de {} cm não comporta corrediça de 25 cm; aumente d para pelo menos {} cm.",
                num(id),
                num(d + (27.0 - id))
            )
        })?;
        let box_w = bay - 2.0 * SLIDE_CLEARANCE;
        let side = cm(15.0);
        for b in 0..bays {
            let x0 = t + f64::from(b) * (bay + t);
            for k in 0..drawer_count {
                let z0 = plinth + t + f64::from(k) * front_h;
                let label = if bays > 1 {
                    format!(" {}.{}", b + 1, k + 1)
                } else {
                    format!(" {}", k + 1)
                };
                let box_h = front_h - 4.0;
                let y0 = dc - slide;
                parts.push(
                    Part::board(
                        &format!("Gaveta{label} lateral esq."),
                        [x0 + SLIDE_CLEARANCE, y0, z0 + 2.0],
                        [side, slide, box_h],
                        "MDF 15",
                        MDF_WHITE,
                    )
                    .banded(1, 0),
                );
                parts.push(
                    Part::board(
                        &format!("Gaveta{label} lateral dir."),
                        [x0 + bay - SLIDE_CLEARANCE - side, y0, z0 + 2.0],
                        [side, slide, box_h],
                        "MDF 15",
                        MDF_WHITE,
                    )
                    .banded(1, 0),
                );
                parts.push(Part::board(
                    &format!("Gaveta{label} traseira"),
                    [x0 + SLIDE_CLEARANCE + side, y0, z0 + 2.0],
                    [box_w - 2.0 * side, side, box_h],
                    "MDF 15",
                    MDF_WHITE,
                ));
                parts.push(Part::board(
                    &format!("Gaveta{label} contrafrente"),
                    [x0 + SLIDE_CLEARANCE + side, dc - side, z0 + 2.0],
                    [box_w - 2.0 * side, side, box_h],
                    "MDF 15",
                    MDF_WHITE,
                ));
                parts.push(Part::board(
                    &format!("Gaveta{label} fundo"),
                    [x0 + SLIDE_CLEARANCE, y0, z0 + 1.4],
                    [box_w, slide, cm(6.0)],
                    "MDF 6",
                    MDF_WHITE,
                ));
                if front_t > 0.0 {
                    let mut front = Part::board(
                        &format!("Frente da gaveta{label}"),
                        [x0 - t / 2.0 + GAP, dc, z0 + GAP],
                        [bay + t - 2.0 * GAP, t, front_h - 2.0 * GAP],
                        &board,
                        front_color,
                    )
                    .banded(2, 2);
                    front.finish.clone_from(&front_finish);
                    parts.push(front);
                }
            }
        }
        hardware.push(format!(
            "{} pares de corrediças telescópicas de {} cm",
            drawer_count * bays,
            num(slide)
        ));
    }

    // Niches: fixed boards around each, no shelves or doors across them.
    let mut niches: Vec<(f64, f64)> = p
        .niches
        .iter()
        .map(|n| (n.bottom, n.bottom + n.height))
        .collect();
    niches.sort_by(|a, b| a.0.total_cmp(&b.0));
    if !niches.is_empty() {
        if p.rod {
            return Err(
                "Cabideiro e nicho no mesmo módulo não combinam: faça dois módulos.".into(),
            );
        }
        if !matches!(p.door, DoorType::Hinged | DoorType::None) {
            return Err(
                "Nicho só em armário com portas de giro ou aberto: use door = hinged.".into(),
            );
        }
        if bays > 1 {
            return Err(format!(
                "O nicho ocupa o vão inteiro: use dividers = 0 (vão interno de {} cm).",
                num(iw)
            ));
        }
    }
    let floor = plinth + t + drawer_zone;
    for (k, &(nb, nt)) in niches.iter().enumerate() {
        if nb < floor - 0.05 {
            return Err(format!(
                "O nicho começa a {} cm do chão, abaixo do fundo útil do armário ({} cm); use bottom ≥ {}.",
                num(nb),
                num(floor),
                num(floor)
            ));
        }
        if nt > h - t + 0.05 {
            return Err(format!(
                "O nicho de {} cm a partir de {} cm passa do topo interno ({} cm); use h = {} ou bottom = {}.",
                num(nt - nb),
                num(nb),
                num(h - t),
                num(nt + t),
                num((h - t - (nt - nb)).max(floor))
            ));
        }
        if let Some(&(next_b, _)) = niches.get(k + 1)
            && next_b < nt + t - 0.05
        {
            return Err(format!(
                "Os nichos a {} e {} cm do chão se sobrepõem; o de cima precisa começar em {} cm.",
                num(nb),
                num(next_b),
                num(nt + t)
            ));
        }
        let below_is_board = k > 0 && (nb - t - (niches[k - 1].1)).abs() < 0.5;
        if nb - t > floor + 0.5 && !below_is_board {
            parts.push(
                Part::board(
                    "Base do nicho",
                    [t, BACK_INSET + back, nb - t],
                    [iw, id, t],
                    &board,
                    carcass,
                )
                .banded(1, 0),
            );
        }
        if nt + t < h - t - 0.5 {
            parts.push(
                Part::board(
                    "Topo do nicho",
                    [t, BACK_INSET + back, nt],
                    [iw, id, t],
                    &board,
                    carcass,
                )
                .banded(1, 0),
            );
        }
        notes.push(format!(
            "Nicho livre de {} × {} × {} cm (largura × altura × profundidade) a {} cm do chão.",
            num(iw),
            num(nt - nb),
            num(id + back + BACK_INSET),
            num(nb)
        ));
    }

    // Shelves above the drawers, evenly spaced.
    let shelf_zone = ih
        - if p.door == DoorType::Drawers {
            ih
        } else {
            drawer_zone
        };
    if p.rod && p.door != DoorType::Drawers {
        // Top shelf for suitcases, the rail 6 cm under it.
        let shelf_z = plinth + hc - t - 35.0;
        let rail_z = shelf_z - 6.0;
        let hanging = rail_z - (plinth + t + drawer_zone);
        if hanging < 95.0 {
            return Err(format!(
                "O cabideiro ficaria com {} cm de altura livre (mínimo 95 cm para camisas); aumente h para {} ou use menos gavetas.",
                num(hanging),
                num(h + 95.0 - hanging)
            ));
        }
        for b in 0..bays {
            let x0 = t + f64::from(b) * (bay + t);
            let suffix = if bays > 1 {
                format!(" {}", b + 1)
            } else {
                String::new()
            };
            parts.push(
                Part::board(
                    &format!("Maleiro{suffix}"),
                    [x0 + 0.1, BACK_INSET + back, shelf_z],
                    [bay - 0.2, id - 1.0, t],
                    &board,
                    carcass,
                )
                .banded(1, 0),
            );
            let mut rail = Part::solid(
                &format!("Tubo cabideiro{suffix}"),
                [x0 + 0.2, BACK_INSET + back + (id - 2.5) / 2.0, rail_z],
                [bay - 0.4, 2.5, 1.5],
                [180, 180, 185],
            );
            rail.finish = "#b4b4b9".parse().ok();
            parts.push(rail);
        }
        hardware.push(format!(
            "{bays} tubo(s) oblongo(s) de {} cm com {} suportes",
            num(bay - 0.4),
            bays * 2
        ));
        if hanging < 140.0 {
            notes.push(format!(
                "Cabideiro com {} cm livres: bom para camisas; vestidos e casacos longos pedem 150 cm.",
                num(hanging)
            ));
        }
    } else if p.shelves > 0 && p.door != DoorType::Drawers {
        let (shelf_floor, shelf_zone) =
            shelves_zone(&niches, plinth + t + drawer_zone, shelf_zone, t);
        let step = shelf_zone / f64::from(p.shelves + 1);
        if step < 15.0 {
            return Err(format!(
                "{} prateleiras deixam {} cm entre elas; o mínimo útil é 15 cm: use shelves = {}.",
                p.shelves,
                num(step),
                ((shelf_zone / 15.0).floor() as u32).saturating_sub(1)
            ));
        }
        for b in 0..bays {
            let x0 = t + f64::from(b) * (bay + t);
            for k in 0..p.shelves {
                let z = shelf_floor + f64::from(k + 1) * step - t / 2.0;
                parts.push(
                    Part::board(
                        &format!(
                            "Prateleira {}",
                            if bays > 1 {
                                format!("{}.{}", b + 1, k + 1)
                            } else {
                                (k + 1).to_string()
                            }
                        ),
                        [x0 + 0.1, BACK_INSET + back, z],
                        [bay - 0.2, id - 1.0, t],
                        &board,
                        carcass,
                    )
                    .banded(1, 0),
                );
            }
        }
        hardware.push(format!("{} suportes de prateleira", p.shelves * bays * 4));
        if bay > span_limit * 0.8 {
            notes.push(format!(
                "Prateleiras de {} cm estão perto do limite de {} cm para {} mm; evite carga pesada.",
                num(bay),
                num(span_limit),
                num(p.t)
            ));
        }
    }

    // Doors over the part without drawers.
    let door_h = hc - drawer_zone;
    match p.door {
        DoorType::Hinged if door_h > 10.0 => {
            let open = w - blind_l - blind_r;
            let n = p
                .doors
                .unwrap_or_else(|| (open / MAX_DOOR).ceil() as u32)
                .max(1);
            let leaf = (open - GAP * f64::from(n + 1)) / f64::from(n);
            if leaf > MAX_DOOR {
                return Err(format!(
                    "Porta de giro com {} cm de largura empena e força as dobradiças; use doors = {} (até {} cm cada).",
                    num(leaf),
                    (open / MAX_DOOR).ceil() as u32,
                    num(MAX_DOOR)
                ));
            }
            // Doors cover the front, except across a niche.
            let segments: Vec<(f64, f64, &str)> = {
                let mut out = Vec::new();
                let mut from = plinth + drawer_zone;
                for (k, &(nb, nt)) in niches.iter().enumerate() {
                    out.push((
                        from,
                        nb,
                        if k == 0 {
                            "Porta"
                        } else {
                            "Porta entre nichos"
                        },
                    ));
                    from = nt;
                }
                out.push((
                    from,
                    plinth + hc,
                    if niches.is_empty() {
                        "Porta"
                    } else {
                        "Porta superior"
                    },
                ));
                out.into_iter().filter(|(a, b, _)| b - a > 10.0).collect()
            };
            if blind_l + blind_r > 0.0 {
                notes.push(format!(
                    "Canto cego: {} cm de painel fixo; acesso pela porta de {} cm.",
                    num(blind_l + blind_r),
                    num(open)
                ));
            }
            let mut hinge_count = 0;
            for (bottom, top, label) in segments {
                let z0 = bottom + GAP;
                let height = top - bottom - 2.0 * GAP;
                for (name, x0, width) in [
                    ("Painel cego esquerdo", 0.0, blind_l),
                    ("Painel cego direito", w - blind_r, blind_r),
                ] {
                    if width > 0.0 {
                        let mut panel = Part::board(
                            name,
                            [x0 + GAP, dc, z0],
                            [width - GAP, t, height],
                            &board,
                            front_color,
                        )
                        .banded(2, 2);
                        panel.finish.clone_from(&front_finish);
                        parts.push(panel);
                    }
                }
                for k in 0..n {
                    let x = blind_l + GAP + f64::from(k) * (leaf + GAP);
                    let mut door = Part::board(
                        &format!("{label} {}", k + 1),
                        [x, dc, z0],
                        [leaf, t, height],
                        &board,
                        front_color,
                    )
                    .banded(2, 2);
                    door.finish.clone_from(&front_finish);
                    parts.push(door);
                }
                hinge_count += n * hinges(height);
            }
            hardware.push(format!(
                "{hinge_count} dobradiças de caneco 35 mm (folga de 2 mm entre portas)"
            ));
        }
        DoorType::Sliding if door_h > 10.0 => {
            let n = p.doors.unwrap_or(2).max(2);
            let overlap = 3.0;
            let leaf = (w + overlap * f64::from(n - 1)) / f64::from(n);
            for k in 0..n {
                let x = f64::from(k) * (leaf - overlap);
                let y = dc + if k % 2 == 0 { t + 1.0 } else { 0.0 };
                let mut door = Part::board(
                    &format!("Folha de correr {}", k + 1),
                    [x, y, plinth + drawer_zone],
                    [leaf, t, door_h],
                    &board,
                    front_color,
                )
                .banded(2, 2);
                door.finish.clone_from(&front_finish);
                parts.push(door);
            }
            hardware.push(format!(
                "trilho de correr duplo de {} cm (superior e inferior)",
                num(w)
            ));
        }
        _ => {}
    }

    Ok(Output {
        parts,
        size: [w, d, h],
        hardware,
        notes,
        extra_cuts: Vec::new(),
        name: format!("Armário {} × {} × {} cm", num(w), num(d), num(h)),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    fn part<'a>(out: &'a Output, name: &str) -> &'a Part {
        out.parts
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no {name}"))
    }

    #[test]
    fn wardrobe_boards_clearances_and_hardware() {
        let out = generate(&CabinetParams {
            w: 200.0,
            h: 240.0,
            d: 60.0,
            dividers: Some(2),
            shelves: 4,
            ..CabinetParams::default()
        })
        .unwrap();
        // 18 mm sides full carcass height (240 − 10 plinth), depth 60 − 1,8 door.
        let side = part(&out, "Lateral esquerda");
        assert_eq!(side.size, [1.8, 58.2, 230.0]);
        // Back 6 mm in 8 mm grooves: 200 − 3,6 + 1,6 wide.
        let back = part(&out, "Fundo");
        assert!((back.size[0] - 198.0).abs() < 1e-9 && (back.size[1] - 0.6).abs() < 1e-9);
        // Four doors of 60 cm max with 2 mm gaps: (200 − 5 × 0,2) / 4 = 49,75.
        let doors: Vec<&Part> = out
            .parts
            .iter()
            .filter(|p| p.name.starts_with("Porta"))
            .collect();
        assert_eq!(doors.len(), 4);
        assert!((doors[0].size[0] - 49.75).abs() < 1e-9);
        assert!(
            (doors[1].at[0] - doors[0].at[0] - 49.95).abs() < 1e-9,
            "2 mm between doors"
        );
        // Door height 230 − 0,4 needs 5 hinges each.
        assert!(
            out.hardware.iter().any(|h| h.starts_with("20 dobradiças")),
            "{:?}",
            out.hardware
        );
        // 3 bays × 4 shelves.
        assert_eq!(
            out.parts
                .iter()
                .filter(|p| p.name.starts_with("Prateleira"))
                .count(),
            12
        );
        let bay = (200.0 - 3.6 - 2.0 * 1.8) / 3.0;
        assert!((part(&out, "Prateleira 1.1").size[0] - (bay - 0.2)).abs() < 1e-9);
    }

    #[test]
    fn drawers_get_slides_that_fit_and_rules_explain_themselves() {
        let out = generate(&CabinetParams {
            w: 60.0,
            h: 85.0,
            d: 55.0,
            door: DoorType::Drawers,
            drawers: 3,
            ..CabinetParams::default()
        })
        .unwrap();
        // Inner depth 55 − 1,8 − 1 − 0,6 = 51,6 → 45 cm slides (51,6 − 2 = 49,6 ≥ 45).
        assert!(
            out.hardware
                .iter()
                .any(|h| h == "3 pares de corrediças telescópicas de 45 cm"),
            "{:?}",
            out.hardware
        );
        let side = part(&out, "Gaveta 1 lateral esq.");
        // Box fits the bay: 60 − 3,6 − 2 × 1,3 wide.
        let box_w = part(&out, "Gaveta 1 fundo").size[0];
        assert!((box_w - (60.0 - 3.6 - 2.6)).abs() < 1e-9);
        assert_eq!(side.size[1], 45.0);

        let cooktop = generate(&CabinetParams {
            d: 35.0,
            cooktop: true,
            ..CabinetParams::default()
        })
        .unwrap_err();
        assert_eq!(
            cooktop,
            "A profundidade do armário é de 35 cm, mas o cooktop exige nicho mínimo de 50 cm. Ajuste a profundidade para 55 cm."
        );
        let wide = generate(&CabinetParams {
            w: 150.0,
            doors: Some(2),
            shelves: 0,
            ..CabinetParams::default()
        })
        .unwrap_err();
        assert!(wide.contains("doors = 3"), "{wide}");
        // Without dividers the shelves get them automatically…
        let auto = generate(&CabinetParams {
            w: 160.0,
            t: 15.0,
            ..CabinetParams::default()
        })
        .unwrap();
        assert_eq!(
            auto.parts
                .iter()
                .filter(|p| p.name.starts_with("Divisória"))
                .count(),
            2
        );
        // …but an explicit choice that can't hold is explained.
        let span = generate(&CabinetParams {
            w: 160.0,
            t: 15.0,
            dividers: Some(0),
            ..CabinetParams::default()
        })
        .unwrap_err();
        assert!(span.contains("dividers = 2"), "{span}");
        let many = generate(&CabinetParams {
            h: 80.0,
            drawers: 5,
            ..CabinetParams::default()
        })
        .unwrap_err();
        assert!(many.contains("drawers = "), "{many}");
        let shallow = generate(&CabinetParams {
            d: 26.0,
            door: DoorType::Drawers,
            ..CabinetParams::default()
        })
        .unwrap_err();
        assert!(shallow.contains("corrediça"), "{shallow}");
        let wardrobe = generate(&CabinetParams {
            w: 90.0,
            h: 230.0,
            d: 58.0,
            rod: true,
            drawers: 2,
            ..CabinetParams::default()
        })
        .unwrap();
        let rail = part(&wardrobe, "Tubo cabideiro");
        let shelf = part(&wardrobe, "Maleiro");
        // Shelf 35 cm under the top inside, rail 6 cm below it, over 2 drawers of 18 cm.
        assert!((shelf.at[2] - (230.0 - 1.8 - 35.0)).abs() < 1e-9);
        assert!((rail.at[2] - (shelf.at[2] - 6.0)).abs() < 1e-9 && rail.board.is_none());
        assert!(
            wardrobe
                .parts
                .iter()
                .all(|p| !p.name.starts_with("Prateleira"))
        );
        assert!(wardrobe.hardware.iter().any(|h| h.contains("tubo")));
        assert!(
            generate(&CabinetParams {
                h: 150.0,
                rod: true,
                drawers: 2,
                ..CabinetParams::default()
            })
            .unwrap_err()
            .contains("cabideiro")
        );
        // A tall cabinet with an oven niche: doors below and above, none across it.
        let tower = generate(&CabinetParams {
            w: 64.0,
            h: 220.0,
            d: 58.0,
            shelves: 2,
            niches: vec![Niche {
                bottom: 80.0,
                height: 62.0,
            }],
            ..CabinetParams::default()
        })
        .unwrap();
        let below = part(&tower, "Porta 1");
        let above = part(&tower, "Porta superior 1");
        assert!(
            (below.at[2] + below.size[2] - (80.0 - 0.2)).abs() < 1e-9,
            "{below:?}"
        );
        assert!((above.at[2] - 142.2).abs() < 1e-9, "{above:?}");
        assert!((part(&tower, "Base do nicho").at[2] - 78.2).abs() < 1e-9);
        assert!((part(&tower, "Topo do nicho").at[2] - 142.0).abs() < 1e-9);
        // Shelves in the taller part, above the niche.
        assert!(
            tower
                .parts
                .iter()
                .filter(|p| p.name.starts_with("Prateleira"))
                .all(|p| p.at[2] > 142.0)
        );
        let high = generate(&CabinetParams {
            w: 60.0,
            h: 120.0,
            niches: vec![Niche {
                bottom: 80.0,
                height: 60.0,
            }],
            ..CabinetParams::default()
        })
        .unwrap_err();
        assert!(high.contains("passa do topo interno"), "{high}");
        let corner = generate(&CabinetParams {
            w: 100.0,
            h: 87.0,
            blind_left: 58.0,
            shelves: 1,
            ..CabinetParams::default()
        })
        .unwrap();
        let panel = part(&corner, "Painel cego esquerdo");
        let door = part(&corner, "Porta 1");
        assert!((panel.size[0] - 57.8).abs() < 1e-9);
        assert!((door.at[0] - 58.2).abs() < 1e-9 && (door.size[0] - 41.6).abs() < 1e-9);
        assert!(
            generate(&CabinetParams {
                w: 80.0,
                blind_left: 58.0,
                ..CabinetParams::default()
            })
            .unwrap_err()
            .contains("use w = 88")
        );
        assert!(
            generate(&CabinetParams {
                t: 16.0,
                ..CabinetParams::default()
            })
            .unwrap_err()
            .contains("15, 18 ou 25")
        );
    }
}
