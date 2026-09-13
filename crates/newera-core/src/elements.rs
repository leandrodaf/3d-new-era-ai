//! Everything that can be placed in a home.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::furniture::Furniture;
use crate::geometry::{Point2, polygon_area};
use crate::ids::{DimensionId, ElementId, LabelId, LevelId, RoomId, WallId};
use crate::materials::Material;

fn yes() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if signature
fn is_true(value: &bool) -> bool {
    *value
}

/// A wall on the floor plan, straight or arced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Wall {
    pub id: WallId,
    pub start: Point2,
    pub end: Point2,
    /// Thickness in centimeters.
    pub thickness: f64,
    /// Height in centimeters.
    pub height: f64,
    /// Arc extent in degrees; `None` for a straight wall. Positive values bulge
    /// to the left of `start → end` (in plan axes, y down).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_extent: Option<f64>,
    /// Storey it belongs to; `None` means the lowest level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<LevelId>,
    /// Standard construction ([`crate::WALL_TYPES`] id), e.g. `drywall-95`.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub wall_type: Option<String>,
    /// Finish of the side on the left of `start → end` (plan axes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_side: Option<Material>,
    /// Finish of the side on the right of `start → end`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_side: Option<Material>,
}

impl Wall {
    pub const DEFAULT_THICKNESS: f64 = 15.0;
    pub const DEFAULT_HEIGHT: f64 = 250.0;

    pub fn new(id: WallId, start: Point2, end: Point2) -> Self {
        Self {
            id,
            start,
            end,
            thickness: Self::DEFAULT_THICKNESS,
            height: Self::DEFAULT_HEIGHT,
            arc_extent: None,
            level: None,
            wall_type: None,
            left_side: None,
            right_side: None,
        }
    }

    /// Applies a standard construction: sets the type and its thickness.
    pub fn apply_type(&mut self, wall_type: &crate::WallType) {
        self.wall_type = Some(wall_type.id.to_owned());
        self.thickness = wall_type.thickness;
    }

    /// True when the wall has a meaningful curvature.
    pub fn is_arc(&self) -> bool {
        self.arc_extent.is_some_and(|a| a.abs() > 0.5)
    }

    /// Length along the centerline (arc length for arced walls).
    pub fn length(&self) -> f64 {
        let chord = self.start.distance(self.end);
        match self.arc_extent.filter(|_| self.is_arc()) {
            Some(extent) => {
                let angle = extent.to_radians().abs();
                chord * (angle / 2.0) / (angle / 2.0).sin()
            }
            None => chord,
        }
    }

    /// Circle `(center, radius, start angle, signed sweep)` of an arced wall.
    fn arc_circle(&self) -> Option<(Point2, f64, f64, f64)> {
        let extent = self.arc_extent.filter(|_| self.is_arc())?;
        let angle = extent.to_radians();
        let chord = self.start.distance(self.end);
        if chord < 1e-9 {
            return None;
        }
        let radius = chord / (2.0 * (angle / 2.0).sin().abs());
        let mid = Point2::new(
            self.start.x.midpoint(self.end.x),
            self.start.y.midpoint(self.end.y),
        );
        let (dx, dy) = (
            (self.end.x - self.start.x) / chord,
            (self.end.y - self.start.y) / chord,
        );
        // Left normal of start → end; the center lies opposite to the bulge.
        let (nx, ny) = (dy, -dx);
        let sagitta_center = (radius * radius - chord * chord / 4.0).max(0.0).sqrt();
        let sign = if angle.abs() > std::f64::consts::PI {
            1.0
        } else {
            -1.0
        } * angle.signum();
        let center = Point2::new(
            mid.x + nx * sagitta_center * sign,
            mid.y + ny * sagitta_center * sign,
        );
        let a0 = (self.start.y - center.y).atan2(self.start.x - center.x);
        Some((center, radius, a0, angle))
    }

    /// Point at parameter `t` in `0..=1` along the centerline.
    pub fn point_at(&self, t: f64) -> Point2 {
        let t = t.clamp(0.0, 1.0);
        match self.arc_circle() {
            _ if t <= 0.0 => self.start,
            _ if t >= 1.0 => self.end,
            Some((center, radius, a0, sweep)) => {
                let a = a0 + sweep * t;
                Point2::new(center.x + radius * a.cos(), center.y + radius * a.sin())
            }
            None => Point2::new(
                self.start.x + (self.end.x - self.start.x) * t,
                self.start.y + (self.end.y - self.start.y) * t,
            ),
        }
    }

    /// Centerline points from `start` to `end`: two for straight walls, a
    /// sampled arc otherwise.
    pub fn centerline(&self) -> Vec<Point2> {
        let Some((_, _, _, sweep)) = self.arc_circle() else {
            return vec![self.start, self.end];
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = ((sweep.abs().to_degrees() / 6.0).ceil() as usize).clamp(4, 64);
        #[allow(clippy::cast_precision_loss)]
        (0..=steps)
            .map(|i| self.point_at(i as f64 / steps as f64))
            .collect()
    }

    fn validate(&self) -> CoreResult<()> {
        if !self.start.is_finite() || !self.end.is_finite() {
            return invalid("wall points must be finite");
        }
        if self.start.distance(self.end) < 1.0 {
            return invalid("wall must be at least 1 cm long");
        }
        if !(self.thickness > 0.0 && self.height > 0.0) {
            return invalid("wall thickness and height must be positive");
        }
        if let Some(extent) = self.arc_extent
            && !(extent.is_finite() && extent.abs() < 360.0)
        {
            return invalid("arc extent must be between -360 and 360 degrees");
        }
        if let Some(id) = &self.wall_type
            && crate::wall_type(id).is_none()
        {
            return invalid(&format!("unknown wall type `{id}`"));
        }
        for side in [&self.left_side, &self.right_side].into_iter().flatten() {
            side.validate().map_err(CoreError::InvalidGeometry)?;
        }
        Ok(())
    }
}

/// A named floor area delimited by a polygon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
    pub points: Vec<Point2>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub floor_visible: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub ceiling_visible: bool,
    /// Show the area label on the plan.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub area_visible: bool,
    /// Storey it belongs to; `None` means the lowest level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<LevelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_material: Option<Material>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ceiling_material: Option<Material>,
}

impl Room {
    pub fn new(id: RoomId, name: impl Into<String>, points: Vec<Point2>) -> Self {
        Self {
            id,
            name: name.into(),
            points,
            floor_visible: true,
            ceiling_visible: true,
            area_visible: true,
            level: None,
            floor_material: None,
            ceiling_material: None,
        }
    }

    /// Floor area in square centimeters.
    pub fn area(&self) -> f64 {
        polygon_area(&self.points)
    }

    fn validate(&self) -> CoreResult<()> {
        if self.points.len() < 3 {
            return invalid("room needs at least 3 points");
        }
        if self.points.iter().any(|p| !p.is_finite()) {
            return invalid("room points must be finite");
        }
        if self.area() < 1.0 {
            return invalid("room area must be positive");
        }
        for material in [&self.floor_material, &self.ceiling_material]
            .into_iter()
            .flatten()
        {
            material.validate().map_err(CoreError::InvalidGeometry)?;
        }
        Ok(())
    }
}

/// A measured distance drawn on the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Dimension {
    pub id: DimensionId,
    pub start: Point2,
    pub end: Point2,
    /// Distance of the dimension line from the measured points, in cm. Positive
    /// values move it to the left of `start → end`.
    #[serde(default)]
    pub offset: f64,
    /// Storey it belongs to; `None` means the lowest level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<LevelId>,
}

impl Dimension {
    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }

    fn validate(&self) -> CoreResult<()> {
        if !(self.start.is_finite() && self.end.is_finite() && self.offset.is_finite()) {
            return invalid("dimension values must be finite");
        }
        if self.length() < 0.1 {
            return invalid("dimension must measure a positive distance");
        }
        Ok(())
    }
}

/// Free text on the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    pub id: LabelId,
    pub text: String,
    pub position: Point2,
    /// Text height in centimeters, so labels scale with the drawing.
    #[serde(default = "Label::default_size")]
    pub size: f64,
    /// Rotation in degrees, clockwise.
    #[serde(default)]
    pub angle: f64,
    /// Storey it belongs to; `None` means the lowest level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<LevelId>,
}

impl Label {
    pub const DEFAULT_SIZE: f64 = 24.0;

    fn default_size() -> f64 {
        Self::DEFAULT_SIZE
    }

    fn validate(&self) -> CoreResult<()> {
        if self.text.trim().is_empty() {
            return invalid("label text must not be empty");
        }
        if !(self.position.is_finite() && self.size > 0.0 && self.angle.is_finite()) {
            return invalid("label needs a finite position and angle and a positive size");
        }
        Ok(())
    }
}

/// The compass rose drawn on the plan. Its north direction will also drive
/// sun position for lighting and renders.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Compass {
    /// Center on the plan, in centimeters.
    pub center: Point2,
    /// Diameter in centimeters.
    pub diameter: f64,
    /// Clockwise angle from the plan's "up" (-y) to geographic north, in degrees.
    pub north_degrees: f64,
    pub visible: bool,
}

impl Default for Compass {
    fn default() -> Self {
        Self {
            center: Point2::new(-100.0, 50.0),
            diameter: 100.0,
            north_degrees: 0.0,
            visible: true,
        }
    }
}

impl Compass {
    pub(crate) fn validate(&self) -> CoreResult<()> {
        if !(self.center.is_finite() && self.diameter > 0.0 && self.north_degrees.is_finite()) {
            return invalid("compass needs a finite center and angle and a positive diameter");
        }
        Ok(())
    }
}

/// A scanned or exported plan drawn under the drawing, at real scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BackgroundImage {
    /// Image file path (absolute, or relative to the project file).
    pub path: String,
    /// Image size in pixels.
    pub size_px: [u32; 2],
    /// Real-world size of one image pixel, in centimeters.
    pub cm_per_px: f64,
    /// Plan position (cm) of the image's top-left corner.
    pub offset: Point2,
    /// 0 (invisible) to 1 (opaque).
    #[serde(default = "BackgroundImage::default_opacity")]
    pub opacity: f64,
    #[serde(default = "yes")]
    pub visible: bool,
}

impl BackgroundImage {
    fn default_opacity() -> f64 {
        0.5
    }

    /// Scale from a calibration: two image pixels that are `distance_cm` apart
    /// in reality.
    pub fn cm_per_px_from(a_px: Point2, b_px: Point2, distance_cm: f64) -> Option<f64> {
        let px = a_px.distance(b_px);
        (px > 0.0 && distance_cm > 0.0).then(|| distance_cm / px)
    }

    /// Plan-space rectangle `(min, max)` covered by the image.
    pub fn bounds(&self) -> (Point2, Point2) {
        let w = f64::from(self.size_px[0]) * self.cm_per_px;
        let h = f64::from(self.size_px[1]) * self.cm_per_px;
        (
            self.offset,
            Point2::new(self.offset.x + w, self.offset.y + h),
        )
    }

    pub(crate) fn validate(&self) -> CoreResult<()> {
        if self.path.trim().is_empty() {
            return invalid("background image path must not be empty");
        }
        if self.size_px[0] == 0 || self.size_px[1] == 0 {
            return invalid("background image size must be positive");
        }
        if !(self.cm_per_px > 0.0 && self.cm_per_px.is_finite() && self.offset.is_finite()) {
            return invalid("background scale must be positive and offset finite");
        }
        if !(0.0..=1.0).contains(&self.opacity) {
            return invalid("background opacity must be between 0 and 1");
        }
        Ok(())
    }
}

/// A storey: a floor of the house at some elevation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Level {
    pub id: LevelId,
    pub name: String,
    /// Height of its floor above the ground, cm.
    pub elevation: f64,
    /// Default wall height for this storey (floor to ceiling), cm.
    pub height: f64,
    /// Slab under this storey's floor, cm.
    #[serde(default = "Level::default_floor_thickness")]
    pub floor_thickness: f64,
}

impl Level {
    pub const DEFAULT_HEIGHT: f64 = 250.0;
    pub const DEFAULT_FLOOR_THICKNESS: f64 = 12.0;

    fn default_floor_thickness() -> f64 {
        Self::DEFAULT_FLOOR_THICKNESS
    }

    fn validate(&self) -> CoreResult<()> {
        if self.name.trim().is_empty() {
            return invalid("level name must not be empty");
        }
        if !(self.elevation.is_finite() && self.height > 0.0 && self.floor_thickness >= 0.0) {
            return invalid(
                "level needs a finite elevation, positive height and non-negative slab",
            );
        }
        Ok(())
    }
}

/// Any element stored in a [`crate::Home`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Element {
    Wall(Wall),
    Room(Room),
    Dimension(Dimension),
    Label(Label),
    Furniture(Furniture),
    Level(Level),
}

impl Element {
    pub fn id(&self) -> ElementId {
        match self {
            Self::Wall(e) => e.id.into(),
            Self::Room(e) => e.id.into(),
            Self::Dimension(e) => e.id.into(),
            Self::Label(e) => e.id.into(),
            Self::Furniture(e) => e.id.into(),
            Self::Level(e) => e.id.into(),
        }
    }

    pub fn validate(&self) -> CoreResult<()> {
        match self {
            Self::Wall(e) => e.validate(),
            Self::Room(e) => e.validate(),
            Self::Dimension(e) => e.validate(),
            Self::Label(e) => e.validate(),
            Self::Furniture(e) => e.validate(),
            Self::Level(e) => e.validate(),
        }
    }
}

macro_rules! element_from {
    ($($variant:ident),+) => {
        $(impl From<$variant> for Element {
            fn from(e: $variant) -> Self {
                Self::$variant(e)
            }
        })+
    };
}
element_from!(Wall, Room, Dimension, Label, Furniture, Level);

impl Element {
    /// Storey of an element; levels themselves have none.
    pub fn level(&self) -> Option<LevelId> {
        match self {
            Self::Wall(e) => e.level,
            Self::Room(e) => e.level,
            Self::Dimension(e) => e.level,
            Self::Label(e) => e.level,
            Self::Furniture(e) => e.level,
            Self::Level(_) => None,
        }
    }

    pub fn set_level(&mut self, level: Option<LevelId>) {
        match self {
            Self::Wall(e) => e.level = level,
            Self::Room(e) => e.level = level,
            Self::Dimension(e) => e.level = level,
            Self::Label(e) => e.level = level,
            Self::Furniture(e) => e.level = level,
            Self::Level(_) => {}
        }
    }
}

fn invalid<T>(message: &str) -> CoreResult<T> {
    Err(CoreError::InvalidGeometry(message.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_wall_centerline_bulges_left_and_keeps_endpoints() {
        let mut wall = Wall::new(WallId(1), Point2::new(0.0, 0.0), Point2::new(400.0, 0.0));
        wall.arc_extent = Some(90.0);
        let line = wall.centerline();
        assert_eq!(line.first(), Some(&wall.start));
        assert_eq!(line.last(), Some(&wall.end));
        // Left of +x in plan axes (y down) is -y.
        let mid = line[line.len() / 2];
        assert!(mid.y < -50.0, "{mid:?}");
        // Quarter circle over a 400 cm chord: radius 282.8, sagitta 82.8.
        assert!((mid.y + 82.84).abs() < 1.0, "{mid:?}");
        assert!((wall.length() - 444.3).abs() < 0.5, "{}", wall.length());
    }

    #[test]
    fn background_calibration() {
        let scale = BackgroundImage::cm_per_px_from(
            Point2::new(10.0, 10.0),
            Point2::new(210.0, 10.0),
            500.0,
        );
        assert_eq!(scale, Some(2.5));
        assert_eq!(
            BackgroundImage::cm_per_px_from(Point2::default(), Point2::default(), 1.0),
            None
        );
    }

    #[test]
    fn default_flags_are_omitted_from_json() {
        let room = Room::new(
            RoomId(1),
            "Sala",
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(0.0, 1.0),
            ],
        );
        let json = serde_json::to_string(&room).unwrap();
        assert!(!json.contains("visible"), "{json}");
        let back: Room = serde_json::from_str(&json).unwrap();
        assert!(back.floor_visible && back.ceiling_visible && back.area_visible);
    }
}
