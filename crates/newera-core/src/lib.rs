//! Core domain of 3D New Era AI.
//!
//! This crate knows nothing about windows, GPUs or networks. Every client —
//! the desktop UI, the MCP server, the HTTP API — mutates a [`Home`] through
//! the same [`Command`]s executed by a [`Document`], so undo/redo and change
//! tracking behave identically no matter who made the change.
//!
//! All lengths are in **centimeters**, matching architectural drawings.

mod analysis;
mod annotations;
pub mod arrange;
pub mod collab;
mod command;
mod detect;
mod document;
mod elements;
mod error;
mod furniture;
mod geometry;
mod home;
mod ids;
mod joins;
mod levels;
pub mod lighting;
mod materials;
pub mod ops;
mod project;
mod roof_fit;
mod style;
mod units;
pub mod vfs;
mod wallrun;

pub use analysis::{Issue, check_layout, door_swing};
pub use annotations::{
    PlanAnnotations, ReferenceItem, RoomReference, auto_dimensions, room_references,
};
pub use command::Command;
pub use detect::{detect_room, detect_room_with_dividers, interior_point, rooms_following_walls};
pub use document::{Document, FIRST_VARIANT_NAME, SharedDocument, Variant, VariantInfo};
pub use elements::{
    BackgroundImage, Baseboard, Compass, Dimension, Element, Label, Level, Room, Wall,
};
pub use error::{CoreError, CoreResult};
pub use furniture::{
    Furniture, Light, LightSource, ModelMaterial, ModelTransform, Opening, OpeningKind, PieceInfo,
    PieceLocks, Sash, SolidShape, WallCut, WallCutOut, align_to_wall, cut_outline, wall_cuts,
};
pub use geometry::{Point2, polygon_area, polygon_centroid, signed_area, triangulate};
pub use home::Home;
pub use ids::{
    DimensionId, ElementId, FurnitureId, LabelId, LevelId, ParseIdError, PolylineId, RoomId, WallId,
};
pub use joins::{JOIN_TOLERANCE, wall_outlines};
pub use levels::{FloorShape, floor_shapes, stair_holes};
pub use lighting::{LampType, RoomLighting};
pub use materials::{Material, Pattern, WALL_TYPES, WallFamily, WallType, wall_type};
pub use project::{
    BundledFiles, PROJECT_EXTENSION, Project, ProjectError, cache_dir, from_project_json,
    open_project, project_from_bytes, resolve_asset, resolve_project_path, save_project,
    to_project_json,
};
pub use roof_fit::{ROOF_FIT_ABOVE, ROOF_FIT_KEY, fit_commands, fit_to_roof, roof_height_at};
pub use style::{
    ArrowStyle, Camera, Cameras, DashStyle, Discipline, DrawingMode, Environment, LineCap,
    LineJoin, PaperOrientation, PhotoSettings, Polyline, PrintSettings, Properties, TextAlign,
    TextStyle, VideoSettings,
};
pub use units::LengthUnit;
pub use wallrun::{RunBlock, RunObstacle, WallRun, wall_run};
