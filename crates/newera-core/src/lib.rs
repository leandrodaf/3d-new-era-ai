//! Core domain of 3D New Era AI.
//!
//! This crate knows nothing about windows, GPUs or networks. Every client —
//! the desktop UI, the MCP server, the HTTP API — mutates a [`Home`] through
//! the same [`Command`]s executed by a [`Document`], so undo/redo and change
//! tracking behave identically no matter who made the change.
//!
//! All lengths are in **centimeters**, matching architectural drawings.

mod analysis;
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
pub mod ops;
mod project;
mod units;

pub use analysis::{Issue, check_layout, door_swing};
pub use command::Command;
pub use detect::detect_room;
pub use document::{Document, SharedDocument};
pub use elements::{BackgroundImage, Compass, Dimension, Element, Label, Room, Wall};
pub use error::{CoreError, CoreResult};
pub use furniture::{
    Furniture, Opening, OpeningKind, WallCut, align_to_wall, cut_outline, wall_cuts,
};
pub use geometry::{Point2, polygon_area, polygon_centroid, signed_area, triangulate};
pub use home::Home;
pub use ids::{DimensionId, ElementId, FurnitureId, LabelId, ParseIdError, RoomId, WallId};
pub use joins::{JOIN_TOLERANCE, wall_outlines};
pub use project::{
    PROJECT_EXTENSION, ProjectError, from_project_json, resolve_project_path, to_project_json,
};
pub use units::LengthUnit;
