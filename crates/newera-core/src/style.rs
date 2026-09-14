//! Text styles, polylines, cameras, 3D environment and print settings.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::geometry::Point2;
use crate::ids::{LevelId, PolylineId};
use crate::materials::Material;

pub(crate) fn yes() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if signature
pub(crate) fn is_true(value: &bool) -> bool {
    *value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// A technical project drawn over the architectural plan.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Discipline {
    Electrical,
    Plumbing,
}

impl Discipline {
    pub const ALL: [Self; 2] = [Self::Electrical, Self::Plumbing];

    pub fn name(self) -> &'static str {
        match self {
            Self::Electrical => "Elétrica",
            Self::Plumbing => "Hidráulica",
        }
    }
}

/// Free-form key/value metadata kept with an element.
pub type Properties = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// How a text is drawn. Sizes are in centimeters, so text scales with the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextStyle {
    /// Font family; `None` uses the default font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    pub size: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: TextAlign,
}

impl TextStyle {
    pub fn new(size: f64) -> Self {
        Self {
            font: None,
            size,
            bold: false,
            italic: false,
            align: TextAlign::Center,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    Bevel,
    #[default]
    Miter,
    Round,
    /// Points are control points of a smooth curve.
    Curved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DashStyle {
    #[default]
    Solid,
    Dot,
    Dash,
    DashDot,
    DashDotDot,
    /// Uses [`Polyline::dash_pattern`].
    Custom,
}

impl DashStyle {
    /// Dash pattern in multiples of the line thickness.
    pub fn pattern(self) -> &'static [f64] {
        match self {
            Self::Solid | Self::Custom => &[],
            Self::Dot => &[1.0, 1.0],
            Self::Dash => &[4.0, 2.0],
            Self::DashDot => &[8.0, 2.0, 2.0, 2.0],
            Self::DashDotDot => &[8.0, 2.0, 2.0, 2.0, 2.0, 2.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ArrowStyle {
    #[default]
    None,
    Delta,
    Open,
    Disc,
}

/// A free line on the plan: annotations, arrows, electrical runs…
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Polyline {
    pub id: PolylineId,
    pub points: Vec<Point2>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
    /// Line width, cm.
    pub thickness: f64,
    pub color: [u8; 3],
    #[serde(default, skip_serializing_if = "is_default")]
    pub cap: LineCap,
    #[serde(default, skip_serializing_if = "is_default")]
    pub join: LineJoin,
    #[serde(default, skip_serializing_if = "is_default")]
    pub dash: DashStyle,
    /// Custom dash lengths, in multiples of the thickness.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dash_pattern: Vec<f64>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dash_offset: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub start_arrow: ArrowStyle,
    #[serde(default, skip_serializing_if = "is_default")]
    pub end_arrow: ArrowStyle,
    /// Shown in 3D at this height above the floor, cm; plan only when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<LevelId>,
    /// Technical project it belongs to; `None` is the architectural plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discipline: Option<Discipline>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: Properties,
}

impl Polyline {
    pub fn new(id: PolylineId, points: Vec<Point2>) -> Self {
        Self {
            id,
            points,
            closed: false,
            thickness: 1.0,
            color: [0, 0, 0],
            cap: LineCap::default(),
            join: LineJoin::default(),
            dash: DashStyle::default(),
            dash_pattern: Vec::new(),
            dash_offset: 0.0,
            start_arrow: ArrowStyle::default(),
            end_arrow: ArrowStyle::default(),
            elevation: None,
            level: None,
            discipline: None,
            properties: Properties::new(),
        }
    }

    pub(crate) fn validate(&self) -> CoreResult<()> {
        if self.points.len() < 2 || self.points.iter().any(|p| !p.is_finite()) {
            return Err(CoreError::InvalidGeometry(
                "polyline needs at least 2 finite points".into(),
            ));
        }
        if !(self.thickness.is_finite() && self.thickness > 0.0) {
            return Err(CoreError::InvalidGeometry(
                "polyline thickness must be positive".into(),
            ));
        }
        Ok(())
    }
}

/// A point of view. Angles in degrees; position in plan cm plus height.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Camera {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub x: f64,
    pub y: f64,
    /// Eye height, cm.
    pub z: f64,
    /// Heading; the view direction on the plan is [`Camera::direction`].
    pub yaw: f64,
    /// Looking down is positive.
    pub pitch: f64,
    /// Horizontal field of view.
    pub fov: f64,
    /// Date and time for sunlight, milliseconds since the Unix epoch (UTC).
    #[serde(default)]
    pub time: i64,
    /// Lens name (`pinhole`, `normal`, `fisheye`, `spherical`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
}

impl Camera {
    /// Plan direction the camera looks at, `(dx, dy)` unit vector.
    pub fn direction(&self) -> (f64, f64) {
        let yaw = self.yaw.to_radians();
        (-yaw.sin(), yaw.cos())
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            name: None,
            x: 50.0,
            y: 1050.0,
            z: 1010.0,
            yaw: 180.0,
            pitch: 45.0,
            fov: 63.0,
            time: 0,
            lens: None,
            renderer: None,
        }
    }
}

/// Aerial and visitor cameras plus saved points of view.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Cameras {
    pub top: Camera,
    pub observer: Camera,
    /// The 3D view shows the visitor instead of the aerial view.
    #[serde(default, skip_serializing_if = "is_false")]
    pub observer_active: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub observer_fixed_size: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stored: Vec<Camera>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DrawingMode {
    #[default]
    Fill,
    Outline,
    FillAndOutline,
}

/// Look of the 3D world around the home.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Environment {
    pub ground_color: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_texture: Option<Material>,
    pub sky_color: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky_texture: Option<Material>,
    pub light_color: [u8; 3],
    pub ceiling_light_color: [u8; 3],
    /// Wall transparency in the 3D view, 0 opaque to 1 invisible.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub walls_alpha: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub drawing_mode: DrawingMode,
    #[serde(default, skip_serializing_if = "is_false")]
    pub all_levels_visible: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub background_on_ground: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub observer_elevation_adjusted: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub subpart_size_under_light: f64,
    pub photo: PhotoSettings,
    pub video: VideoSettings,
    /// Keyframes of the video camera path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub camera_path: Vec<Camera>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            ground_color: [0xA8, 0xA8, 0x98],
            ground_texture: None,
            sky_color: [0xCC, 0xE4, 0xFC],
            sky_texture: None,
            light_color: [0xD0, 0xD0, 0xD0],
            ceiling_light_color: [0xD0, 0xD0, 0xD0],
            walls_alpha: 0.0,
            drawing_mode: DrawingMode::Fill,
            all_levels_visible: false,
            background_on_ground: false,
            observer_elevation_adjusted: true,
            subpart_size_under_light: 0.0,
            photo: PhotoSettings::default(),
            video: VideoSettings::default(),
            camera_path: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PhotoSettings {
    pub width: u32,
    pub height: u32,
    /// 0 fastest to 3 best.
    #[serde(default)]
    pub quality: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
}

impl Default for PhotoSettings {
    fn default() -> Self {
        Self {
            width: 400,
            height: 300,
            quality: 0,
            aspect_ratio: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VideoSettings {
    pub width: u32,
    #[serde(default)]
    pub quality: u8,
    pub frame_rate: u32,
    /// Camera speed along the path, m/s.
    pub speed: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            width: 320,
            quality: 0,
            frame_rate: 25,
            speed: 2.4 / 3.6,
            aspect_ratio: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaperOrientation {
    #[default]
    Portrait,
    Landscape,
    ReverseLandscape,
}

/// Page setup for printing and PDF export. Paper sizes in points (1/72 in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PrintSettings {
    pub orientation: PaperOrientation,
    pub paper_width: f64,
    pub paper_height: f64,
    /// Top, left, bottom, right.
    pub margins: [f64; 4],
    #[serde(default = "yes")]
    pub furniture: bool,
    #[serde(default = "yes")]
    pub plan: bool,
    #[serde(default = "yes")]
    pub view_3d: bool,
    /// Plan scale (e.g. 0.01 for 1:100); `None` fits the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    /// Levels to print; `None` prints all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<LevelId>>,
}
