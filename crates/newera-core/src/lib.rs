//! Core domain of 3D New Era AI.
//!
//! This crate knows nothing about windows, GPUs or networks. Every client —
//! the desktop UI, the MCP server, the HTTP API — mutates a [`Home`] through
//! the same [`Command`]s executed by a [`Document`], so undo/redo and change
//! tracking behave identically no matter who made the change.
//!
//! All lengths are in **centimeters**, matching Sweet Home 3D.

mod command;
mod document;
mod error;
mod geometry;
mod model;

pub use command::Command;
pub use document::{Document, SharedDocument};
pub use error::{CoreError, CoreResult};
pub use geometry::Point2;
pub use model::{Compass, Home, ParseIdError, Room, RoomId, Wall, WallId};
