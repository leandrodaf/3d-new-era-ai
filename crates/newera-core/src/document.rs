use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::command::Command;
use crate::error::{CoreError, CoreResult};
use crate::model::{Home, RoomId, WallId};

/// A [`Home`] plus its edit history.
///
/// `revision` increases on every successful change (including undo/redo), so
/// views can cheaply detect when they need to rebuild derived data such as
/// 3D meshes.
#[derive(Debug, Default)]
pub struct Document {
    home: Home,
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
    revision: u64,
}

impl Document {
    pub fn new(home: Home) -> Self {
        Self {
            home,
            ..Self::default()
        }
    }

    pub fn home(&self) -> &Home {
        &self.home
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Allocates a fresh wall id. Ids are never reused, even after undo.
    pub fn new_wall_id(&mut self) -> WallId {
        self.home.new_wall_id()
    }

    /// Allocates a fresh room id. Ids are never reused, even after undo.
    pub fn new_room_id(&mut self) -> RoomId {
        self.home.new_room_id()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Executes a command, recording it in the undo history.
    pub fn execute(&mut self, command: Command) -> CoreResult<()> {
        let inverse = command.apply(&mut self.home)?;
        self.undo_stack.push(inverse);
        self.redo_stack.clear();
        self.revision += 1;
        Ok(())
    }

    pub fn undo(&mut self) -> CoreResult<()> {
        let command = self.undo_stack.pop().ok_or(CoreError::NothingToUndo)?;
        let inverse = self.apply_from_history(command)?;
        self.redo_stack.push(inverse);
        Ok(())
    }

    pub fn redo(&mut self) -> CoreResult<()> {
        let command = self.redo_stack.pop().ok_or(CoreError::NothingToRedo)?;
        let inverse = self.apply_from_history(command)?;
        self.undo_stack.push(inverse);
        Ok(())
    }

    /// Replaces the home and clears the history.
    pub fn load(&mut self, home: Home) {
        self.home = home;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.revision += 1;
    }

    fn apply_from_history(&mut self, command: Command) -> CoreResult<Command> {
        let inverse = command.apply(&mut self.home)?;
        self.revision += 1;
        Ok(inverse)
    }
}

/// Thread-safe handle shared by the UI thread and the async servers.
#[derive(Debug, Clone, Default)]
pub struct SharedDocument(Arc<RwLock<Document>>);

impl SharedDocument {
    pub fn new(document: Document) -> Self {
        Self(Arc::new(RwLock::new(document)))
    }

    pub fn read(&self) -> RwLockReadGuard<'_, Document> {
        // A panic while holding the lock cannot leave a half-applied command
        // (commands roll back on error), so recovering from poison is safe.
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, Document> {
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
