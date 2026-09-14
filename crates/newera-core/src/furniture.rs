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
use crate::ids::{FurnitureId, LevelId};
use crate::materials::Material;
use crate::style::{Properties, TextStyle, is_default, is_zero};

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

/// A door or window leaf that turns around a vertical axis. Values are
/// fractions of the piece's width (x, leaf width) and depth (y).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Sash {
    pub x_axis: f64,
    pub y_axis: f64,
    pub width: f64,
    /// Degrees.
    pub start_angle: f64,
    pub end_angle: f64,
}

/// Where and how a door or window cuts the wall, as fractions of the
/// piece's size (depth for thickness/distance, width for width/left, height
/// for height/top) plus an optional outline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WallCutOut {
    pub wall_thickness: f64,
    pub wall_distance: f64,
    pub wall_width: f64,
    pub wall_left: f64,
    pub wall_height: f64,
    pub wall_top: f64,
    /// SVG path of the hole in a unit square (front view), when not rectangular.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// Cut through the whole wall, not only the part the piece overlaps.
    #[serde(default)]
    pub both_sides: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bound_to_wall: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub width_depth_deformable: bool,
}

impl Default for WallCutOut {
    fn default() -> Self {
        Self {
            wall_thickness: 1.0,
            wall_distance: 0.0,
            wall_width: 1.0,
            wall_left: 0.0,
            wall_height: 1.0,
            wall_top: 0.0,
            shape: None,
            both_sides: false,
            bound_to_wall: false,
            width_depth_deformable: true,
        }
    }
}

/// Makes a piece cut a hole through the wall it sits in.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
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
    /// Explicit leaves (imported doors); overrides `leaves`/`hinge_right` when set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sashes: Vec<Sash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_out: Option<WallCutOut>,
}

/// A light emitter inside a piece, in fractions of its size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LightSource {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub color: [u8; 3],
    /// Fraction of the piece's width; `None` for a point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter: Option<f64>,
}

/// Makes a piece emit light.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Light {
    /// 0 to 1.
    pub power: f64,
    pub sources: Vec<LightSource>,
    /// Model materials that glow.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_materials: Vec<String>,
    /// Luminous flux, lm (overrides `watts` and `power`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lumens: Option<f64>,
    /// Electrical power, W; flux follows from the lamp's efficacy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watts: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lamp: Option<crate::lighting::LampType>,
    /// Color temperature, K (overrides the sources' color).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kelvin: Option<f64>,
    /// Full beam angle of a spot pointing down, degrees (at half intensity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beam: Option<f64>,
    /// Emitting panel facing down, width × depth cm (LED panels and strips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<[f64; 2]>,
}

/// Override of one material of an imported model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelMaterial {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<Material>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shininess: Option<f64>,
}

/// Descriptive data that doesn't affect geometry.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct PieceInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Id in the catalog it came from (e.g. `eTeks#frontDoor`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_catalog_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vat_percentage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    /// Commercial model name or code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    /// Product page or reference link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Catalog icon image file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Top view image drawn in the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_icon: Option<String>,
}

/// What the user may change on a piece.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PieceLocks {
    pub movable: bool,
    pub resizable: bool,
    /// Width, depth and height can change independently.
    pub deformable: bool,
    pub texturable: bool,
    /// Can be tilted (pitch/roll).
    pub horizontally_rotatable: bool,
}

impl Default for PieceLocks {
    fn default() -> Self {
        Self {
            movable: true,
            resizable: true,
            deformable: true,
            texturable: true,
            horizontally_rotatable: true,
        }
    }
}

/// How an imported model file maps onto the piece's box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelTransform {
    /// Rotation applied to the model before fitting it to the box, rows of
    /// a 3×3 matrix in model axes (y up).
    pub rotation: [[f64; 3]; 3],
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub centered_at_origin: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub back_face_shown: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub flags: u32,
    /// Size of the model file in bytes, as reported by its source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
}

impl Default for ModelTransform {
    fn default() -> Self {
        Self {
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            centered_at_origin: true,
            back_face_shown: false,
            flags: 0,
            file_size: None,
        }
    }
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

/// A solid made from a polygon instead of a catalog model. Points are in cm,
/// relative to the piece's center, and are scaled with its box when resized.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SolidShape {
    /// Plan outline `[x, y]` raised by the piece height: slabs, mezzanines, decks.
    Outline(Vec<[f64; 2]>),
    /// Cross-section `[x across the width, z up]` swept along the depth:
    /// triangular gables, profiles, ramps.
    Profile(Vec<[f64; 2]>),
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
    /// Storey it belongs to; `None` means the lowest level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<LevelId>,
    /// Technical project it belongs to; `None` is the architectural plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discipline: Option<crate::style::Discipline>,
    /// Tilt around its width axis, degrees.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pitch: f64,
    /// Tilt around its depth axis, degrees.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub roll: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub info: PieceInfo,
    #[serde(default, skip_serializing_if = "is_default")]
    pub locks: PieceLocks,
    #[serde(default, skip_serializing_if = "is_default")]
    pub model_transform: ModelTransform,
    /// Texture applied to the whole model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<Material>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shininess: Option<f64>,
    /// 0 (invisible) to 1 (opaque); glass panels, water, reference boards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// Built from a polygon instead of its catalog model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<SolidShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<ModelMaterial>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<Light>,
    /// Pieces of a group, positioned in plan coordinates like top-level ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Furniture>,
    /// Fraction of the height where pieces dropped on top rest.
    #[serde(
        default = "Furniture::default_drop_on_top",
        skip_serializing_if = "Furniture::is_default_drop_on_top"
    )]
    pub drop_on_top: f64,
    /// Outline of the hole stairs cut in the floor above (SVG path, unit square).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staircase_cut_out: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub name_visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_style: Option<TextStyle>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub name_offset: [f64; 2],
    #[serde(default, skip_serializing_if = "is_zero")]
    pub name_angle: f64,
    #[serde(default, skip_serializing_if = "Properties::is_empty")]
    pub properties: Properties,
}

impl Default for Furniture {
    fn default() -> Self {
        Self {
            id: FurnitureId(0),
            catalog: String::new(),
            name: String::new(),
            position: Point2::new(0.0, 0.0),
            elevation: 0.0,
            angle: 0.0,
            width: 100.0,
            depth: 100.0,
            height: 100.0,
            mirrored: false,
            color: None,
            opening: None,
            model: None,
            visible: true,
            level: None,
            pitch: 0.0,
            roll: 0.0,
            discipline: None,
            info: PieceInfo::default(),
            locks: PieceLocks::default(),
            model_transform: ModelTransform::default(),
            texture: None,
            shininess: None,
            opacity: None,
            shape: None,
            materials: Vec::new(),
            light: None,
            children: Vec::new(),
            drop_on_top: 1.0,
            staircase_cut_out: None,
            name_visible: false,
            name_style: None,
            name_offset: [0.0, 0.0],
            name_angle: 0.0,
            properties: Properties::new(),
        }
    }
}

impl Furniture {
    fn default_drop_on_top() -> f64 {
        1.0
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn is_default_drop_on_top(value: &f64) -> bool {
        (*value - 1.0).abs() < f64::EPSILON
    }

    /// Moves the piece and, for groups, every piece inside.
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.position = Point2::new(self.position.x + dx, self.position.y + dy);
        for child in &mut self.children {
            child.translate(dx, dy);
        }
    }

    /// Visible pieces to draw: the piece itself, or for a group the visible
    /// non-group pieces inside it.
    pub fn visible_leaves(&self) -> Vec<&Self> {
        if !self.visible {
            return Vec::new();
        }
        if self.children.is_empty() {
            return vec![self];
        }
        self.children
            .iter()
            .flat_map(Self::visible_leaves)
            .collect()
    }

    /// After a group's box was moved, turned or resized from `before`, carries
    /// its pieces along: positions, angles, sizes and elevations follow.
    pub fn follow_group_change(&mut self, before: &Self) {
        fn carry(
            piece: &mut Furniture,
            old: &Furniture,
            new: &Furniture,
            s: (f64, f64, f64),
            turn: f64,
        ) {
            let (lx, ly) = old.to_local(piece.position);
            let (lx, ly) = (lx * s.0, ly * s.1);
            // `to_local` undoes mirroring; `to_plan` redoes it for the new box.
            piece.position = new.to_plan((lx, ly));
            piece.angle += turn;
            piece.width *= s.0;
            piece.depth *= s.1;
            piece.height *= s.2;
            piece.elevation = new.elevation + (piece.elevation - old.elevation) * s.2;
            for child in &mut piece.children {
                carry(child, old, new, s, turn);
            }
        }
        if self.children.is_empty() {
            return;
        }
        let ratio = |new: f64, old: f64| if old.abs() > 1e-9 { new / old } else { 1.0 };
        let (sx, sy, sz) = (
            ratio(self.width, before.width),
            ratio(self.depth, before.depth),
            ratio(self.height, before.height),
        );
        let turn = self.angle - before.angle;
        let (old_center, new) = (before.clone(), self.clone());
        for child in &mut self.children {
            carry(child, &old_center, &new, (sx, sy, sz), turn);
        }
    }

    pub fn is_group(&self) -> bool {
        !self.children.is_empty()
    }

    /// This piece and every piece nested in its groups, depth first.
    pub fn flatten(&self) -> Vec<&Self> {
        let mut out = vec![self];
        for child in &self.children {
            out.extend(child.flatten());
        }
        out
    }

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

    /// Corners of its box after pitch and roll, in the local frame
    /// `(x, y up from its bottom, depth)`, cm.
    fn tilted_corners(&self) -> Vec<[f64; 3]> {
        let (hw, hd, hh) = (self.width / 2.0, self.depth / 2.0, self.height / 2.0);
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let (sr, cr) = self.roll.to_radians().sin_cos();
        let mut out = Vec::with_capacity(8);
        for x in [-hw, hw] {
            for y in [-hh, hh] {
                for z in [-hd, hd] {
                    // Roll around the depth axis, then pitch around the width axis.
                    let (x1, y1) = (x * cr - y * sr, x * sr + y * cr);
                    let (y2, z2) = (y1 * cp - z * sp, y1 * sp + z * cp);
                    out.push([x1, y2 + hh, z2]);
                }
            }
        }
        out
    }

    /// Plan footprint of what it really covers once tilted.
    pub fn projected_footprint(&self) -> [Point2; 4] {
        if self.pitch == 0.0 && self.roll == 0.0 {
            return self.footprint();
        }
        let corners = self.tilted_corners();
        let (x0, x1, z0, z1) = corners.iter().fold(
            (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
            |(x0, x1, z0, z1), c| (x0.min(c[0]), x1.max(c[0]), z0.min(c[2]), z1.max(c[2])),
        );
        [(x0, z0), (x1, z0), (x1, z1), (x0, z1)].map(|p| self.to_plan(p))
    }

    /// Lowest and highest point above the floor, cm.
    pub fn height_range(&self) -> (f64, f64) {
        if self.pitch == 0.0 && self.roll == 0.0 {
            return (self.elevation, self.elevation + self.height);
        }
        let (lo, hi) = self
            .tilted_corners()
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), c| {
                (lo.min(c[1]), hi.max(c[1]))
            });
        (self.elevation + lo, self.elevation + hi)
    }

    /// Height of its underside above the floor over a plan point (tilted
    /// pieces are lower at one end), cm.
    pub fn underside_at(&self, p: Point2) -> f64 {
        if self.pitch == 0.0 && self.roll == 0.0 {
            return self.elevation;
        }
        let (x, depth) = self.to_local(p);
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let (sr, cr) = self.roll.to_radians().sin_cos();
        // Centerline height under that point, minus half the thickness.
        let z = if cp.abs() > 1e-6 { depth / cp } else { 0.0 };
        let x = if cr.abs() > 1e-6 { x / cr } else { 0.0 };
        let mid = self.elevation + self.height / 2.0 + x * sr * cp - z * sp;
        mid - (self.height / 2.0 * cp * cr).abs()
    }

    /// Height of its top above the floor over a plan point, cm.
    pub fn top_at(&self, p: Point2) -> f64 {
        if self.pitch == 0.0 && self.roll == 0.0 {
            return self.elevation + self.height;
        }
        let tilt = (self.pitch.to_radians().cos() * self.roll.to_radians().cos()).abs();
        self.underside_at(p) + self.height * tilt
    }

    /// Plan corners of its bounding box, counter-clockwise on screen.
    pub fn footprint(&self) -> [Point2; 4] {
        let (hw, hd) = (self.width / 2.0, self.depth / 2.0);
        [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)].map(|p| self.to_plan(p))
    }

    /// Stairs open a hole in the floor of the storey they climb to.
    /// Catalog ids starting with `stairs` are stairs.
    pub fn is_stairs(&self) -> bool {
        self.catalog.starts_with("stairs")
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
/// cuts every wall it is aligned with and lies in: one that sits where a
/// gable is split in two walls opens both halves.
pub fn wall_cuts(walls: &[Wall], furniture: &[Furniture]) -> Vec<Vec<WallCut>> {
    let mut cuts = vec![Vec::new(); walls.len()];
    for piece in furniture.iter().filter(|f| f.is_opening() && f.visible) {
        let hits: Vec<(usize, f64, f64)> = walls
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
                let half = piece.width / 2.0;
                (across <= reach && along + half > 0.1 && along - half < len - 0.1)
                    .then_some((i, along, len))
            })
            .collect();
        for (i, along, len) in hits {
            let wall = &walls[i];
            let (from, to) = (
                (along - piece.width / 2.0).max(0.0),
                (along + piece.width / 2.0).min(len),
            );
            // Sloping walls (gables): the opening fits under the lower side.
            let height_at = |s: f64| {
                wall.height + (wall.height_at_end.unwrap_or(wall.height) - wall.height) * (s / len)
            };
            let limit = height_at(from).min(height_at(to));
            let cut = WallCut {
                furniture: piece.id,
                from,
                to,
                bottom: piece.elevation.clamp(0.0, limit),
                top: (piece.elevation + piece.height).clamp(0.0, limit),
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
            level: None,
            ..Default::default()
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

    #[test]
    fn a_door_where_a_gable_is_split_opens_both_halves() {
        let mut left = Wall::new(WallId(1), Point2::new(0.0, 0.0), Point2::new(300.0, 0.0));
        left.height = 55.0;
        left.height_at_end = Some(675.0);
        let mut right = Wall::new(WallId(2), Point2::new(300.0, 0.0), Point2::new(600.0, 0.0));
        right.height = 675.0;
        right.height_at_end = Some(55.0);
        let mut door = piece(3, (300.0, 0.0), (90.0, 10.0, 210.0));
        door.elevation = 55.0;
        door.opening = Some(Opening::default());
        let cuts = wall_cuts(&[left, right], &[door]);
        assert_eq!((cuts[0][0].from, cuts[0][0].to), (255.0, 300.0));
        assert_eq!((cuts[1][0].from, cuts[1][0].to), (0.0, 45.0));
        assert!(cuts.iter().all(|c| (c[0].top - 265.0).abs() < 1e-9));
    }

    #[cfg(test)]
    mod group_tests {
        use super::*;

        #[test]
        fn groups_carry_their_pieces() {
            let child = Furniture {
                position: Point2::new(110.0, 100.0),
                width: 20.0,
                ..Furniture::default()
            };
            let before = Furniture {
                position: Point2::new(100.0, 100.0),
                width: 40.0,
                children: vec![child],
                ..Furniture::default()
            };
            let mut after = before.clone();
            after.position = Point2::new(200.0, 100.0);
            after.width = 80.0;
            after.angle = 90.0;
            after.follow_group_change(&before);
            let moved = &after.children[0];
            assert!(
                (moved.position.x - 200.0).abs() < 1e-9,
                "{:?}",
                moved.position
            );
            assert!(
                (moved.position.y - 120.0).abs() < 1e-9,
                "{:?}",
                moved.position
            );
            assert!((moved.width - 40.0).abs() < 1e-9 && (moved.angle - 90.0).abs() < 1e-9);
        }
    }
}
