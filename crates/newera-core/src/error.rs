use thiserror::Error;

use crate::model::{RoomId, WallId};

/// Errors produced when a command cannot be applied to a home.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum CoreError {
    #[error("wall {0} not found")]
    WallNotFound(WallId),

    #[error("room {0} not found")]
    RoomNotFound(RoomId),

    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),

    #[error("nothing to undo")]
    NothingToUndo,

    #[error("nothing to redo")]
    NothingToRedo,
}

pub type CoreResult<T> = Result<T, CoreError>;
