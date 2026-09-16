//! Where a wall-mounted point can go: into the masonry, never into glass.
//!
//! An outlet, a switch, a network or water point is set in a box inside the
//! wall. A glass wall has nothing to set it in, and neither has the span of
//! a door or a window at the height it takes — an outlet under a window sill
//! is fine, one on the pane is not.

use crate::furniture::{Furniture, OpeningKind};
use crate::geometry::Point2;
use crate::home::Home;
use crate::materials::WallFamily;

/// How far off a wall's centerline a point still sits in it, beyond half
/// its thickness, cm.
const IN_WALL: f64 = 12.0;

/// What a point is fixed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mount {
    /// Set into a wall: outlets, switches, panels, wall lights, water points.
    Wall,
    /// Fixed to the ceiling: lighting points, Wi-Fi, presence sensors.
    Ceiling,
    /// Set into the floor: drains and trap boxes.
    Floor,
    /// Set into furniture: outlet towers in a countertop, a desk box.
    Furniture,
}

/// What an outlet set into furniture takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuiltIn {
    /// Hole or cutout width, mm.
    pub hole_mm: f64,
    /// Body under the top, cm (0: set into a panel).
    pub below_cm: f64,
    /// A cord with a plug to an outlet (else wired to the circuit).
    pub plug: bool,
    /// Also goes in a desk or table, not only a fixed piece.
    pub desk: bool,
}

/// The built-in outlets of the catalog: NEO Avant/Renna 60 mm towers with a
/// plug, Caixa Tomada automatic 85 mm wired ones, a Häfele-type 100 mm tower,
/// a desk box (cutout 120 × 335 mm) and a 35 mm furniture outlet.
pub fn built_in_spec(catalog: &str) -> Option<BuiltIn> {
    Some(match catalog {
        "outlet-tower" => BuiltIn {
            hole_mm: 60.0,
            below_cm: 30.0,
            plug: true,
            desk: false,
        },
        "outlet-tower-auto" => BuiltIn {
            hole_mm: 85.0,
            below_cm: 30.0,
            plug: false,
            desk: false,
        },
        "outlet-tower-4" => BuiltIn {
            hole_mm: 100.0,
            below_cm: 36.0,
            plug: true,
            desk: false,
        },
        "desk-outlet-box" => BuiltIn {
            hole_mm: 120.0,
            below_cm: 8.0,
            plug: false,
            desk: true,
        },
        "furniture-outlet" => BuiltIn {
            hole_mm: 35.0,
            below_cm: 0.0,
            plug: false,
            desk: true,
        },
        _ => return None,
    })
}

/// The piece a built-in outlet is set into: the one under it whose top it
/// sits on, or whose panel it is in.
pub fn host_of<'a>(view: &'a Home, piece: &Furniture) -> Option<&'a Furniture> {
    let spec = built_in_spec(&piece.catalog)?;
    view.furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| {
            f.id != piece.id && f.opening.is_none() && f.discipline.is_none() && !f.is_group()
        })
        .filter(|f| spec.desk || !movable(f) || f.properties.contains_key("joinery:part"))
        .filter(|f| f.width.min(f.depth) >= 20.0)
        .filter(|f| {
            let mut near = (*f).clone();
            near.width += 2.0;
            near.depth += 2.0;
            if !near.contains(piece.position) {
                return false;
            }
            let (_, top) = f.height_range();
            if spec.below_cm > 0.0 {
                (top - piece.elevation).abs() <= 3.0
            } else {
                piece.elevation >= f.elevation - 1.0 && piece.elevation <= top
            }
        })
        .max_by(|a, b| a.height_range().1.total_cmp(&b.height_range().1))
}

/// How far from a wall a point asked for `at` is still taken into it, cm.
const SNAP: f64 = 60.0;
/// How far under the ceiling a ceiling point may hang, cm.
const UNDER_CEILING: f64 = 30.0;

/// What a catalog piece is fixed to, if it is a fixed point.
pub fn mount_of(catalog: &str) -> Option<Mount> {
    if wall_mounted(catalog) {
        Some(Mount::Wall)
    } else if matches!(
        catalog,
        "light-ceiling" | "downlight" | "led-panel" | "wifi-point" | "presence-sensor"
    ) {
        Some(Mount::Ceiling)
    } else if built_in_spec(catalog).is_some() {
        Some(Mount::Furniture)
    } else if crate::plumbing::drain_spec(catalog).is_some() || catalog == "rain-drain" {
        Some(Mount::Floor)
    } else {
        None
    }
}

/// The ceiling over a plan point, cm above the floor: the top of the nearest
/// wall, which is the slab it holds.
fn ceiling_at(view: &Home, at: Point2) -> Option<f64> {
    view.walls
        .iter()
        .filter(|w| !w.is_arc())
        .min_by(|a, b| {
            distance_to_segment(at, a.start, a.end)
                .total_cmp(&distance_to_segment(at, b.start, b.end))
        })
        .map(|w| w.height.max(w.height_at_end.unwrap_or(w.height)))
}

fn in_a_room(view: &Home, at: Point2) -> bool {
    view.rooms
        .iter()
        .any(|r| r.points.len() >= 3 && crate::electrical::inside(&r.points, at))
}

/// Seats a fixed point where it is fixed: a wall point onto the face of the
/// nearest wall, its back to it; a ceiling point at the ceiling height.
/// Refuses one that has no structure to be fixed to.
pub fn seat(home: &Home, piece: &mut Furniture) -> Result<(), String> {
    let Some(mount) = mount_of(&piece.catalog) else {
        return Ok(());
    };
    let view = home.level_view(home.current_level());
    match mount {
        Mount::Wall => {
            let (wall, dist) = view
                .walls
                .iter()
                .filter(|w| !w.is_arc())
                .map(|w| (w, distance_to_segment(piece.position, w.start, w.end)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .ok_or_else(|| {
                    format!(
                        "{} precisa de uma parede, e não há paredes neste pavimento",
                        piece.name
                    )
                })?;
            if dist > wall.thickness / 2.0 + SNAP && built_into(&view, piece).is_some() {
                return Ok(());
            }
            if dist > wall.thickness / 2.0 + SNAP {
                return Err(format!(
                    "{} fica embutido em parede, e a parede mais próxima ({}) está a {} cm: dê at junto a uma parede ou wall=<id>",
                    piece.name,
                    wall.id,
                    (dist - wall.thickness / 2.0).round()
                ));
            }
            let (a, b) = (wall.start, wall.end);
            let len = a.distance(b).max(1e-9);
            let (ux, uy) = ((b.x - a.x) / len, (b.y - a.y) / len);
            let t = ((piece.position.x - a.x) * ux + (piece.position.y - a.y) * uy).clamp(0.0, len);
            let foot = Point2::new(a.x + ux * t, a.y + uy * t);
            // The face on the side the point was asked, or the room's side.
            let side = (piece.position.x - foot.x) * -uy + (piece.position.y - foot.y) * ux;
            let mut n = (-uy, ux);
            if side < 0.0
                || (side.abs() < 1e-6
                    && !in_a_room(
                        &view,
                        Point2::new(foot.x + n.0 * wall.thickness, foot.y + n.1 * wall.thickness),
                    ))
            {
                n = (uy, -ux);
            }
            let off = wall.thickness / 2.0 + piece.depth / 2.0;
            piece.position = Point2::new(foot.x + n.0 * off, foot.y + n.1 * off);
            // Its front faces the room: local +y along the normal.
            piece.angle = (-n.0).atan2(n.1).to_degrees();
            Ok(())
        }
        Mount::Furniture => {
            // Onto the top of the piece under it (or into its panel).
            let Some(spec) = built_in_spec(&piece.catalog) else {
                return Ok(());
            };
            let host = view
                .furniture
                .iter()
                .flat_map(Furniture::flatten)
                .filter(|f| {
                    f.id != piece.id
                        && f.opening.is_none()
                        && f.discipline.is_none()
                        && !f.is_group()
                })
                .filter(|f| spec.desk || !movable(f) || f.properties.contains_key("joinery:part"))
                .filter(|f| f.width.min(f.depth) >= 20.0 && f.contains(piece.position))
                .max_by(|a, b| a.height_range().1.total_cmp(&b.height_range().1))
                .ok_or_else(|| {
                    format!(
                        "{} vai embutida {}: não há um sob {:?}",
                        piece.name,
                        if spec.desk {
                            "numa bancada, móvel fixo ou mesa"
                        } else {
                            "numa bancada, ilha ou móvel fixo"
                        },
                        [piece.position.x.round(), piece.position.y.round()]
                    )
                })?;
            if spec.below_cm > 0.0 {
                piece.elevation = host.height_range().1;
            }
            Ok(())
        }
        Mount::Floor => {
            // Flush with the finished floor.
            piece.elevation = 0.0;
            match blocked(home, piece) {
                Some(why) => Err(why),
                None => Ok(()),
            }
        }
        Mount::Ceiling => {
            if !in_a_room(&view, piece.position) {
                return Err(format!(
                    "{} vai no teto de um cômodo, e {:?} não está dentro de nenhum",
                    piece.name,
                    [piece.position.x.round(), piece.position.y.round()]
                ));
            }
            if let Some(ceiling) = ceiling_at(&view, piece.position) {
                piece.elevation = (ceiling - piece.height).max(0.0);
            }
            Ok(())
        }
    }
}

/// Whether a catalog piece is set into a wall.
pub fn wall_mounted(catalog: &str) -> bool {
    matches!(
        catalog,
        "outlet-low"
            | "outlet-mid"
            | "outlet-high"
            | "switch"
            | "switch-double"
            | "switch-3way"
            | "network-outlet"
            | "data-outlet"
            | "tv-outlet"
            | "doorbell"
            | "electrical-panel"
            | "telecom-panel"
            | "light-wall"
            | "ac-point"
            | "shower-point"
            | "smart-switch"
            | "dimmer"
            | "smart-lock"
            | "cold-water"
            | "hot-water"
            | "gas-point"
            | "valve"
            | "vent-grille"
    )
}

fn distance_to_segment(p: Point2, a: Point2, b: Point2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = (dx * dx + dy * dy).max(1e-9);
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    Point2::new(a.x + t * dx, a.y + t * dy).distance(p)
}

/// Why `piece` cannot be set where it stands, if it cannot: in a glass wall,
/// or in the span of a door, a window or an open passage at its height.
/// Pieces not set into walls, or standing in no wall, are never refused.
pub fn blocked(home: &Home, piece: &Furniture) -> Option<String> {
    let view = home.level_view(home.current_level());
    if mount_of(&piece.catalog) == Some(Mount::Floor) {
        return floor_blocked(&view, piece);
    }
    if mount_of(&piece.catalog) == Some(Mount::Furniture) {
        return host_of(&view, piece).is_none().then(|| {
            format!(
                "{} {} está solta: vai embutida no tampo de uma bancada, ilha ou móvel (ou numa mesa, a caixa de mesa).",
                piece.name, piece.id
            )
        });
    }
    if mount_of(&piece.catalog) == Some(Mount::Ceiling) {
        if !in_a_room(&view, piece.position) {
            return Some(format!(
                "{} {} vai no teto e está fora de qualquer cômodo.",
                piece.name, piece.id
            ));
        }
        let top = piece.elevation + piece.height;
        return ceiling_at(&view, piece.position)
            .filter(|c| top < c - UNDER_CEILING || top > c + 5.0)
            .map(|c| {
                format!(
                    "{} {} está a {} cm do chão, solto no ar: vai fixado no teto, a {} cm.",
                    piece.name,
                    piece.id,
                    top.round(),
                    c.round()
                )
            });
    }
    if !wall_mounted(&piece.catalog) {
        return None;
    }
    if let Some(why) = off_structure(&view, piece) {
        return Some(why);
    }
    let wall = view
        .walls
        .iter()
        .filter(|w| !w.is_arc())
        .map(|w| (w, distance_to_segment(piece.position, w.start, w.end)))
        .filter(|(w, d)| *d <= w.thickness / 2.0 + IN_WALL)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w);
    let (lo, hi) = (piece.elevation, piece.elevation + piece.height);
    let Some(wall) = wall else {
        // Not in a wall: said by the caller unless a glass piece explains it.
        // In no wall: set on a glass piece (a shower screen, a railing, a
        // see-through partition) at its height is set on nothing.
        return view
            .furniture
            .iter()
            .flat_map(Furniture::flatten)
            .filter(|f| f.id != piece.id && !f.is_opening())
            .filter(|f| {
                matches!(f.catalog.as_str(), "shower-glass" | "glass-railing")
                    || f.opacity.is_some_and(|o| o < 0.6)
            })
            .find(|f| {
                let mut near = (*f).clone();
                near.width += 2.0 * IN_WALL;
                near.depth += 2.0 * IN_WALL;
                near.contains(piece.position) && lo < f.elevation + f.height && hi > f.elevation
            })
            .map(|f| {
                format!(
                    "{} {} está sobre o vidro de {} ({}): não há onde embutir a caixa; leve-o a uma parede.",
                    piece.name, piece.id, f.name, f.id
                )
            });
    };
    let family = wall
        .wall_type
        .as_deref()
        .and_then(crate::materials::wall_type)
        .map(|t| t.family);
    if family == Some(WallFamily::Glass) {
        return Some(format!(
            "{} {} está numa parede de vidro ({}): não há onde embutir a caixa; leve-o a uma parede de alvenaria ou drywall.",
            piece.name, piece.id, wall.id
        ));
    }
    view.furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| f.id != piece.id)
        .find_map(|f| {
            let kind = f.opening.as_ref()?.kind;
            let mut span = f.clone();
            span.depth = span.depth.max(wall.thickness + 2.0 * IN_WALL);
            let top = if kind == OpeningKind::Passage {
                f64::MAX
            } else {
                f.elevation + f.height
            };
            let overlaps = lo < top && hi > f.elevation;
            (span.contains(piece.position) && overlaps).then(|| {
                let what = match kind {
                    OpeningKind::Door => "no vão da porta",
                    OpeningKind::Window => "sobre o vidro da janela",
                    OpeningKind::Passage => "no vão aberto",
                };
                format!(
                    "{} {} está {what} {} ({}): não há parede ali para a caixa; mova-o para o lado do vão, ou abaixo do peitoril.",
                    piece.name, piece.id, f.name, f.id
                )
            })
        })
}

fn in_wall<'a>(view: &'a Home, piece: &Furniture) -> Option<&'a crate::elements::Wall> {
    view.walls
        .iter()
        .filter(|w| !w.is_arc())
        .map(|w| (w, distance_to_segment(piece.position, w.start, w.end)))
        .filter(|(w, d)| *d <= w.thickness / 2.0 + IN_WALL)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)
}

/// A drain set where no floor takes it: outside every room, inside a wall,
/// or in a door's span.
fn floor_blocked(view: &Home, piece: &Furniture) -> Option<String> {
    if !in_a_room(view, piece.position) {
        return Some(format!(
            "{} {} vai no piso de um cômodo e está fora de todos.",
            piece.name, piece.id
        ));
    }
    if let Some(w) = view
        .walls
        .iter()
        .filter(|w| !w.is_arc())
        .find(|w| distance_to_segment(piece.position, w.start, w.end) < w.thickness / 2.0)
    {
        return Some(format!(
            "{} {} está dentro da parede {}: o ralo vai no piso, fora dela.",
            piece.name, piece.id, w.id
        ));
    }
    view.furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| f.opening.is_some())
        .find(|f| {
            let mut span = (*f).clone();
            span.depth += 10.0;
            span.contains(piece.position)
        })
        .map(|f| {
            format!(
                "{} {} está no vão de {} ({}): a soleira não leva ralo; ponha-o dentro do cômodo.",
                piece.name, piece.id, f.name, f.id
            )
        })
}

/// A wall point standing in no wall and on no glass either: loose in a room.
fn off_structure(view: &Home, piece: &Furniture) -> Option<String> {
    if in_wall(view, piece).is_some() {
        return None;
    }
    let glass_near = view.furniture.iter().flat_map(Furniture::flatten).any(|f| {
        (matches!(f.catalog.as_str(), "shower-glass" | "glass-railing")
            || f.opacity.is_some_and(|o| o < 0.6))
            && {
                let mut near = f.clone();
                near.width += 2.0 * IN_WALL;
                near.depth += 2.0 * IN_WALL;
                near.contains(piece.position)
            }
    });
    (!glass_near && built_into(view, piece).is_none()).then(|| {
        format!(
            "{} {} está solto no meio do cômodo: vai embutido numa parede, ou no móvel fixo de uma ilha ou bancada.",
            piece.name, piece.id
        )
    })
}

/// A fixed piece a point may be set into instead of a wall: a counter, an
/// island or a cabinet it stands inside of — where an island's sink takes
/// its water from the floor and its outlet on the side. Appliances, seats,
/// beds and tables are not structure.
fn built_into<'a>(view: &'a Home, piece: &Furniture) -> Option<&'a Furniture> {
    view.furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| {
            f.id != piece.id && !f.is_group() && f.opening.is_none() && f.discipline.is_none()
        })
        .filter(|f| f.height >= 60.0 && f.width.min(f.depth) >= 30.0 && !movable(f))
        .find(|f| {
            let mut near = (*f).clone();
            near.width += 20.0;
            near.depth += 20.0;
            near.contains(piece.position)
        })
}

/// Pieces that stand free and are no structure to fix a point to.
fn movable(f: &Furniture) -> bool {
    let name = crate::annotations::fold(&f.name);
    [
        "geladeira",
        "refrigerador",
        "maquina",
        "lava",
        "secadora",
        "forno",
        "micro",
        "fogao",
        "tv",
        "televis",
        "cama",
        "sofa",
        "poltrona",
        "cadeira",
        "mesa",
        "banco",
        "puff",
        "berco",
        "tapete",
    ]
    .iter()
    .any(|w| name.contains(w))
        || matches!(
            f.catalog.as_str(),
            "fridge"
                | "washer"
                | "dryer"
                | "dishwasher"
                | "oven"
                | "microwave"
                | "stove"
                | "tv"
                | "bed-single"
                | "bed-double"
                | "bed-queen"
                | "bed-king"
                | "sofa-2"
                | "sofa-3"
                | "sofa-l"
                | "armchair"
                | "chair"
                | "stool"
                | "crib"
                | "coffee-table"
                | "side-table"
                | "dining-table-4"
                | "dining-table-6"
                | "round-table"
                | "dining-set-4"
                | "dining-set-6"
                | "desk"
                | "office-chair"
        )
}

/// Why a wall point in its wall is in the way, if so: behind the leaf of a
/// hinged door on its hinge side. A point behind furniture or set into it is
/// not a defect; a drain under a piece standing on the floor is, since it
/// cannot be cleaned.
pub fn hidden(home: &Home, piece: &Furniture) -> Option<String> {
    let view = home.level_view(home.current_level());
    if mount_of(&piece.catalog) == Some(Mount::Floor) {
        // Under a piece standing on the floor it cannot be cleaned: a
        // shower's or a tub's own drain is where it belongs.
        return view
            .furniture
            .iter()
            .flat_map(Furniture::flatten)
            .filter(|f| {
                f.id != piece.id
                    && !f.is_group()
                    && f.opening.is_none()
                    && f.discipline.is_none()
                    && f.elevation < 5.0
                    && f.height >= 10.0
            })
            .filter(|f| {
                let name = crate::annotations::fold(&f.name);
                !matches!(
                    f.catalog.as_str(),
                    "shower" | "shower-glass" | "bathtub" | "rug"
                ) && !["box", "chuveiro", "banheira", "tapete", "ducha"]
                    .iter()
                    .any(|w| name.contains(w))
            })
            .find(|f| f.contains(piece.position))
            .map(|f| {
                format!(
                    "{} {} fica embaixo de {} ({}): sem acesso para limpar e desentupir.",
                    piece.name, piece.id, f.name, f.id
                )
            });
    }
    if !wall_mounted(&piece.catalog) {
        return None;
    }
    in_wall(&view, piece)?;
    let lo = piece.elevation;
    for f in view
        .furniture
        .iter()
        .flat_map(Furniture::flatten)
        .filter(|f| f.id != piece.id)
    {
        if let Some(opening) = f.opening.as_ref()
            && opening.kind == OpeningKind::Door
            && !opening.sliding
            && opening.leaves < 2
        {
            let (x, y) = f.to_local(piece.position);
            let half = f.width / 2.0;
            let beyond = if opening.hinge_right {
                x - half
            } else {
                -half - x
            };
            if beyond > 0.0
                && beyond <= f.width
                && y.abs() <= f.depth / 2.0 + IN_WALL
                && lo < f.elevation + f.height
            {
                return Some(format!(
                    "{} {} fica atrás da folha aberta de {} ({}): ponha-o do lado da maçaneta.",
                    piece.name, piece.id, f.name, f.id
                ));
            }
        }
        // Behind or inside furniture is fine: an outlet behind a sofa, set
        // into a cabinet or a countertop is a technique, not a defect.
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::Wall;
    use crate::furniture::Opening;
    use crate::ids::{FurnitureId, WallId};

    fn outlet(elevation: f64, at: (f64, f64)) -> Furniture {
        Furniture {
            id: FurnitureId(10),
            catalog: "outlet-low".into(),
            name: "Tomada".into(),
            position: Point2::new(at.0, at.1),
            elevation,
            width: 10.0,
            depth: 4.0,
            height: 10.0,
            ..Furniture::default()
        }
    }

    #[test]
    fn a_point_on_glass_or_in_an_opening_span_is_refused_and_under_a_sill_is_not() {
        let mut home = Home::default();
        home.walls.push(Wall::new(
            WallId(1),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        ));
        let mut window = Furniture {
            id: FurnitureId(20),
            catalog: "window".into(),
            name: "Janela".into(),
            position: Point2::new(100.0, 0.0),
            elevation: 110.0,
            width: 120.0,
            depth: 15.0,
            height: 100.0,
            ..Furniture::default()
        };
        window.opening = Some(Opening {
            kind: OpeningKind::Window,
            ..Opening::default()
        });
        let mut door = Furniture {
            id: FurnitureId(21),
            catalog: "door".into(),
            name: "Porta".into(),
            position: Point2::new(300.0, 0.0),
            width: 80.0,
            depth: 15.0,
            height: 210.0,
            ..Furniture::default()
        };
        door.opening = Some(Opening::default());
        home.furniture = vec![window, door];

        assert!(
            blocked(&home, &outlet(130.0, (100.0, 3.0))).is_some(),
            "on the pane"
        );
        assert!(
            blocked(&home, &outlet(30.0, (100.0, 3.0))).is_none(),
            "under the sill"
        );
        assert!(
            blocked(&home, &outlet(30.0, (300.0, 3.0))).is_some(),
            "in the door"
        );
        assert!(
            blocked(&home, &outlet(30.0, (200.0, 3.0))).is_none(),
            "on the wall between"
        );
        let loose = blocked(&home, &outlet(30.0, (200.0, 150.0))).expect("in no wall at all");
        assert!(loose.contains("solto"), "{loose}");

        // A glass wall takes nothing.
        home.walls[0].apply_type(
            crate::materials::WALL_TYPES
                .iter()
                .find(|t| t.family == WallFamily::Glass)
                .unwrap(),
        );
        let why = blocked(&home, &outlet(30.0, (200.0, 3.0))).unwrap();
        assert!(why.contains("vidro"), "{why}");
        // A glass railing standing free takes nothing either.
        let mut open = Home::default();
        open.furniture.push(Furniture {
            id: FurnitureId(30),
            catalog: "glass-railing".into(),
            name: "Guarda-corpo".into(),
            position: Point2::new(0.0, 0.0),
            width: 200.0,
            depth: 2.0,
            height: 110.0,
            ..Furniture::default()
        });
        assert!(blocked(&open, &outlet(30.0, (50.0, 4.0))).is_some());
        assert!(
            blocked(&open, &outlet(30.0, (50.0, 80.0)))
                .unwrap()
                .contains("solto")
        );
        // A ceiling light is not set into walls.
        let mut light = outlet(270.0, (200.0, 3.0));
        light.catalog = "light-ceiling".into();
        assert!(
            !blocked(&home, &light).unwrap_or_default().contains("vidro"),
            "a ceiling light is not set in the wall"
        );
    }

    #[test]
    fn points_are_seated_on_their_structure_and_hidden_ones_are_said() {
        use crate::elements::Room;
        let mut home = Home::default();
        let corners = [(0.0, 0.0), (400.0, 0.0), (400.0, 300.0), (0.0, 300.0)];
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            let mut w = Wall::new(
                WallId(k as u64 + 1),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            );
            w.height = 270.0;
            home.walls.push(w);
        }
        home.rooms.push(Room::new(
            crate::ids::RoomId(9),
            "Sala",
            corners.iter().map(|c| Point2::new(c.0, c.1)).collect(),
        ));
        // An outlet asked 30 cm off the wall goes onto its face, back to it.
        let mut o = outlet(30.0, (150.0, 30.0));
        seat(&home, &mut o).unwrap();
        assert!(
            (o.position.y - (7.5 + 2.0)).abs() < 1e-6 && (o.position.x - 150.0).abs() < 1e-6,
            "{:?}",
            o.position
        );
        assert!(blocked(&home, &o).is_none());
        // One asked in the middle of the room is refused.
        let mut middle = outlet(30.0, (200.0, 150.0));
        assert!(
            seat(&home, &mut middle)
                .unwrap_err()
                .contains("parede mais próxima")
        );
        // An island counter is structure: a water point inside it stays.
        let mut island = home.clone();
        island.furniture.push(Furniture {
            id: FurnitureId(60),
            catalog: "imported".into(),
            name: "Ilha".into(),
            position: Point2::new(200.0, 150.0),
            width: 120.0,
            depth: 60.0,
            height: 90.0,
            ..Furniture::default()
        });
        let mut water = outlet(60.0, (200.0, 150.0));
        water.catalog = "cold-water".into();
        seat(&island, &mut water).unwrap();
        assert!(blocked(&island, &water).is_none());
        island.furniture[0].name = "Mesa de jantar".into();
        assert!(
            blocked(&island, &water).unwrap().contains("solto"),
            "a table is no structure"
        );
        // A Wi-Fi point goes up to the ceiling; one left at 1 m is loose.
        let mut ap = outlet(100.0, (200.0, 150.0));
        ap.catalog = "wifi-point".into();
        ap.height = 4.0;
        assert!(blocked(&home, &ap).unwrap().contains("solto no ar"));
        seat(&home, &mut ap).unwrap();
        assert!((ap.elevation - 266.0).abs() < 1e-6 && blocked(&home, &ap).is_none());
        let mut outside = ap.clone();
        outside.position = Point2::new(600.0, 150.0);
        assert!(seat(&home, &mut outside).is_err());

        // Behind a wardrobe or set into it: a technique, not a defect.
        home.furniture.push(Furniture {
            id: FurnitureId(50),
            catalog: "imported".into(),
            name: "Armário".into(),
            position: Point2::new(150.0, 7.5 + 30.0),
            width: 120.0,
            depth: 60.0,
            height: 220.0,
            ..Furniture::default()
        });
        assert!(hidden(&home, &o).is_none());
        assert!(blocked(&home, &o).is_none());

        // check_layout: the outlet in the cabinet is served; a loose one is loose.
        let mut outlet_piece = o.clone();
        outlet_piece.discipline = Some(crate::style::Discipline::Electrical);
        home.furniture.push(outlet_piece);
        let mut floating = outlet(30.0, (200.0, 150.0));
        floating.id = FurnitureId(70);
        floating.discipline = Some(crate::style::Discipline::Electrical);
        home.furniture.push(floating);
        let issues = crate::analysis::check_layout(&home);
        assert!(
            issues.iter().any(|i| matches!(
                i,
                crate::analysis::Issue::Overlap {
                    kind: crate::analysis::Overlap::Served,
                    ..
                }
            )),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|i| matches!(i, crate::analysis::Issue::Loose { piece, .. } if *piece == FurnitureId(70))),
            "{issues:?}"
        );
        assert!(!issues.iter().any(|i| matches!(i, crate::analysis::Issue::Loose { piece, .. } if *piece == FurnitureId(10))));
    }
}
