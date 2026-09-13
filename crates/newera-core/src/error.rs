use thiserror::Error;

use crate::ids::ElementId;

/// Errors produced when a command cannot be applied to a home.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum CoreError {
    #[error("{0} not found")]
    NotFound(ElementId),

    #[error("{0} already exists")]
    AlreadyExists(ElementId),

    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),

    #[error("nothing to undo")]
    NothingToUndo,

    #[error("nothing to redo")]
    NothingToRedo,
}

pub type CoreResult<T> = Result<T, CoreError>;
