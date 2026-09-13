//! Furniture, doors and windows: real objects with real dimensions.
//!
//! A piece is a box of `width × depth × height` cm placed on the plan by its
//! center, rotated by `angle` and lifted by `elevation`. What it looks like
//! comes from its catalog entry (or an imported model); the core only knows
//! its size, which is what layout, collisions and wall openings need.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::elements::Wall;
use crate::error::{CoreError, CoreResult};
use crate::geometry::Point2;
use crate::ids::FurnitureId;

fn yes() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if signature
fn is_true(value: &bool) -> bool {
    *value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OpeningKind {
    #[default]
    Door,
    Window,
    /// An open passage without a leaf.
    Passage,
}

/// Makes a piece cut a hole through the wall it sits in.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Opening {
    pub kind: OpeningKind,
    /// Hinges on the right side (seen from the front) instead of the left.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hinge_right: bool,
    /// Number of leaves (1 or 2).
    #[serde(default = "Opening::one", skip_serializing_if = "Opening::is_one")]
    pub leaves: u8,
    /// Sliding leaves: no swing area.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sliding: bool,
}

impl Opening {
    fn one() -> u8 {
        1
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn is_one(value: &u8) -> bool {
        *value == 1
    }
}

/// A piece of furniture, a door or a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Furniture {
    pub id: FurnitureId,
    /// Catalog entry that defines its look, e.g. `bed-double`.
    pub catalog: String,
    pub name: String,
    /// Center on the plan, cm.
    pub position: Point2,
    /// Height of its bottom above the floor, cm.
    #[serde(default)]
    pub elevation: f64,
    /// Clockwise rotation in degrees.
    #[serde(default)]
    pub angle: f64,
    /// Size along its local x axis, cm.
    pub width: f64,
    /// Size along its local y axis (front to back), cm.
    pub depth: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mirrored: bool,
    /// Main color override, `[r, g, b]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<Opening>,
    /// Imported 3D model file, when not built from the catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

impl Furniture {
    /// Maps a point in the piece's local frame (cm, origin at its center, x
    /// along the width, y along the depth) to plan coordinates.
    pub fn to_plan(&self, local: (f64, f64)) -> Point2 {
        let x = if self.mirrored { -local.0 } else { local.0 };
        let a = self.angle.to_radians();
        let (sin, cos) = a.sin_cos();
        Point2::new(
            self.position.x + x * cos - local.1 * sin,
            self.position.y + x * sin + local.1 * cos,
        )
    }

    /// Maps a plan point into the piece's local frame (inverse of [`Self::to_plan`]).
    pub fn to_local(&self, p: Point2) -> (f64, f64) {
        let (dx, dy) = (p.x - self.position.x, p.y - self.position.y);
        let a = self.angle.to_radians();
        let (sin, cos) = a.sin_cos();
        let x = dx * cos + dy * sin;
        let y = -dx * sin + dy * cos;
        (if self.mirrored { -x } else { x }, y)
    }

    /// Plan corners of its bounding box, counter-clockwise on screen.
    pub fn footprint(&self) -> [Point2; 4] {
        let (hw, hd) = (self.width / 2.0, self.depth / 2.0);
        [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)].map(|p| self.to_plan(p))
    }

    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    pub fn contains(&self, p: Point2) -> bool {
        let (x, y) = self.to_local(p);
        x.abs() <= self.width / 2.0 && y.abs() <= self.depth / 2.0
    }

    pub(crate) fn validate(&self) -> CoreResult<()> {
        if !(self.position.is_finite() && self.elevation.is_finite() && self.angle.is_finite()) {
            return Err(CoreError::InvalidGeometry(
                "furniture position, elevation and angle must be finite".into(),
            ));
        }
        if !(self.width > 0.0 && self.depth > 0.0 && self.height > 0.0) {
            return Err(CoreError::InvalidGeometry(
                "furniture width, depth and height must be positive".into(),
            ));
        }
        if self.width > 100_000.0 || self.depth > 100_000.0 || self.height > 100_000.0 {
            return Err(CoreError::InvalidGeometry(
                "furniture is larger than 1 km".into(),
            ));
        }
        if self.catalog.trim().is_empty() {
            return Err(CoreError::InvalidGeometry(
                "furniture needs a catalog id".into(),
            ));
        }
        Ok(())
    }
}

/// The hole an opening cuts through a wall, in the wall's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallCut {
    pub furniture: FurnitureId,
    /// Distance along the wall from its start, cm.
    pub from: f64,
    pub to: f64,
    /// Heights above the floor, cm.
    pub bottom: f64,
    pub top: f64,
}

/// Openings cut into each straight wall, in `walls` order. A door or window
/// belongs to the nearest wall it is aligned with and centered in.
pub fn wall_cuts(walls: &[Wall], furniture: &[Furniture]) -> Vec<Vec<WallCut>> {
    let mut cuts = vec![Vec::new(); walls.len()];
    for piece in furniture.iter().filter(|f| f.is_opening() && f.visible) {
        let best = walls
            .iter()
            .enumerate()
            .filter(|(_, w)| !w.is_arc())
            .filter_map(|(i, w)| {
                let len = w.start.distance(w.end);
                if len < 1e-9 {
                    return None;
                }
                let dir = ((w.end.x - w.start.x) / len, (w.end.y - w.start.y) / len);
                let wall_angle = dir.1.atan2(dir.0).to_degrees();
                let diff = (piece.angle - wall_angle).rem_euclid(180.0);
                if diff.min(180.0 - diff) > 5.0 {
                    return None;
                }
                let (dx, dy) = (piece.position.x - w.start.x, piece.position.y - w.start.y);
                let along = dx * dir.0 + dy * dir.1;
                let across = (-dx * dir.1 + dy * dir.0).abs();
                let reach = w.thickness.max(piece.depth) / 2.0 + 1.0;
                (across <= reach && along >= 0.0 && along <= len).then_some((i, along, across, len))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((i, along, _, len)) = best {
            let wall = &walls[i];
            let cut = WallCut {
                furniture: piece.id,
                from: (along - piece.width / 2.0).max(0.0),
                to: (along + piece.width / 2.0).min(len),
                bottom: piece.elevation.clamp(0.0, wall.height),
                top: (piece.elevation + piece.height).clamp(0.0, wall.height),
            };
            if cut.to - cut.from > 0.1 && cut.top - cut.bottom > 0.1 {
                cuts[i].push(cut);
            }
        }
    }
    for list in &mut cuts {
        list.sort_by(|a, b| a.from.total_cmp(&b.from));
    }
    cuts
}

/// Plan polygons of a wall with its door and window holes removed.
///
/// `outline` is the joined outline of `wall`; openings split it into pieces.
pub fn cut_outline(outline: &[Point2], wall: &Wall, cuts: &[WallCut]) -> Vec<Vec<Point2>> {
    use geo::{BooleanOps, Coord, LineString, Polygon};

    if cuts.is_empty() || outline.len() < 3 {
        return vec![outline.to_vec()];
    }
    let ring = |pts: &[Point2]| {
        let mut coords: Vec<Coord<f64>> = pts.iter().map(|p| Coord { x: p.x, y: p.y }).collect();
        coords.push(coords[0]);
        LineString::new(coords)
    };
    let len = wall.start.distance(wall.end).max(1e-9);
    let dir = (
        (wall.end.x - wall.start.x) / len,
        (wall.end.y - wall.start.y) / len,
    );
    let normal = (-dir.1, dir.0);
    let reach = wall.thickness; // generous: beyond both faces
    let mut shape = geo::MultiPolygon::new(vec![Polygon::new(ring(outline), vec![])]);
    for cut in cuts {
        let at = |along: f64, side: f64| {
            Point2::new(
                wall.start.x + dir.0 * along + normal.0 * side,
                wall.start.y + dir.1 * along + normal.1 * side,
            )
        };
        let hole = [
            at(cut.from, -reach),
            at(cut.to, -reach),
            at(cut.to, reach),
            at(cut.from, reach),
        ];
        shape = shape.difference(&Polygon::new(ring(&hole), vec![]));
    }
    shape
        .iter()
        .map(|poly| {
            let mut pts: Vec<Point2> = poly
                .exterior()
                .coords()
                .map(|c| Point2::new(c.x, c.y))
                .collect();
            pts.pop();
            pts
        })
        .filter(|pts| pts.len() >= 3)
        .collect()
}

/// Places a piece in a wall: centered on it at `along` cm from its start,
/// aligned with it and, for openings, as deep as the wall is thick.
pub fn align_to_wall(piece: &mut Furniture, wall: &Wall, along: f64) {
    let len = wall.start.distance(wall.end).max(1e-9);
    let t = (along / len).clamp(0.0, 1.0);
    piece.position = wall.point_at(t);
    let (a, b) = (
        wall.point_at((t - 0.01).max(0.0)),
        wall.point_at((t + 0.01).min(1.0)),
    );
    piece.angle = (b.y - a.y).atan2(b.x - a.x).to_degrees();
    if piece.is_opening() {
        piece.depth = wall.thickness;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::WallId;

    fn piece(id: u64, at: (f64, f64), size: (f64, f64, f64)) -> Furniture {
        Furniture {
            id: FurnitureId(id),
            catalog: "box".into(),
            name: "Caixa".into(),
            position: Point2::new(at.0, at.1),
            elevation: 0.0,
            angle: 0.0,
            width: size.0,
            depth: size.1,
            height: size.2,
            mirrored: false,
            color: None,
            opening: None,
            model: None,
            visible: true,
        }
    }

    #[test]
    fn local_and_plan_frames_are_inverse_with_rotation_and_mirror() {
        let mut f = piece(1, (100.0, 50.0), (200.0, 90.0, 80.0));
        f.angle = 90.0;
        f.mirrored = true;
        let p = f.to_plan((40.0, 10.0));
        let back = f.to_local(p);
        assert!((back.0 - 40.0).abs() < 1e-9 && (back.1 - 10.0).abs() < 1e-9);
        // 90° clockwise on screen: local +x points down (+y).
        let mut g = piece(2, (0.0, 0.0), (10.0, 10.0, 10.0));
        g.angle = 90.0;
        let q = g.to_plan((10.0, 0.0));
        assert!(q.x.abs() < 1e-9 && (q.y - 10.0).abs() < 1e-9, "{q:?}");
    }

    #[test]
    fn doors_cut_the_wall_they_sit_in() {
        let walls = vec![
            Wall::new(WallId(1), Point2::new(0.0, 0.0), Point2::new(500.0, 0.0)),
            Wall::new(
                WallId(2),
                Point2::new(500.0, 0.0),
                Point2::new(500.0, 400.0),
            ),
        ];
        let mut door = piece(3, (0.0, 0.0), (80.0, 15.0, 210.0));
        door.opening = Some(Opening::default());
        align_to_wall(&mut door, &walls[1], 100.0);
        assert!((door.angle - 90.0).abs() < 1e-6);
        assert_eq!(door.position, Point2::new(500.0, 100.0));

        let mut window = piece(4, (250.0, 3.0), (120.0, 15.0, 110.0));
        window.elevation = 100.0;
        window.opening = Some(Opening {
            kind: OpeningKind::Window,
            ..Opening::default()
        });
        let unrelated = piece(5, (250.0, 200.0), (80.0, 60.0, 70.0));

        let cuts = wall_cuts(&walls, &[door, window, unrelated]);
        assert_eq!(cuts[0].len(), 1);
        assert_eq!((cuts[0][0].from, cuts[0][0].to), (190.0, 310.0));
        assert_eq!((cuts[0][0].bottom, cuts[0][0].top), (100.0, 210.0));
        assert_eq!(cuts[1].len(), 1);
        assert_eq!(
            (cuts[1][0].from, cuts[1][0].to, cuts[1][0].top),
            (60.0, 140.0, 210.0)
        );

        // The window splits the free-standing wall outline in two pieces.
        let lone = Wall::new(WallId(9), Point2::new(0.0, 0.0), Point2::new(500.0, 0.0));
        let outline = crate::joins::wall_outlines(std::slice::from_ref(&lone)).remove(0);
        let pieces = cut_outline(&outline, &lone, &cuts[0]);
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        let area: f64 = pieces
            .iter()
            .map(|p| crate::geometry::polygon_area(p))
            .sum();
        assert!((area - (500.0 - 120.0) * 15.0).abs() < 1e-6, "{area}");
    }
}
