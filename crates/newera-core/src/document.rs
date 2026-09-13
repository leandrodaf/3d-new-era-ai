use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::command::Command;
use crate::error::{CoreError, CoreResult};
use crate::home::Home;
use crate::ids::{DimensionId, FurnitureId, LabelId, RoomId, WallId};

/// One version of the project, with its own edit history.
#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    home: Home,
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
}

impl Variant {
    fn new(name: impl Into<String>, home: Home) -> Self {
        Self {
            name: name.into(),
            home,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn home(&self) -> &Home {
        &self.home
    }
}

/// Summary of a variant, for tabs and comparisons.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantInfo {
    pub index: usize,
    pub name: String,
    pub active: bool,
}

/// A project: one or more variants (versions of the same plan), each a
/// [`Home`] with its own undo history. Every edit applies to the active one.
///
/// `revision` increases on every successful change (including undo/redo and
/// switching variants), so views can cheaply detect when they need to rebuild
/// derived data such as 3D meshes.
#[derive(Debug)]
pub struct Document {
    variants: Vec<Variant>,
    active: usize,
    revision: u64,
    saved_revision: u64,
    /// Project file this document was loaded from or saved to.
    path: Option<PathBuf>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(Home::default())
    }
}

pub const FIRST_VARIANT_NAME: &str = "Versão 1";

impl Document {
    pub fn new(home: Home) -> Self {
        Self {
            variants: vec![Variant::new(FIRST_VARIANT_NAME, home)],
            active: 0,
            revision: 0,
            saved_revision: 0,
            path: None,
        }
    }

    fn current(&self) -> &Variant {
        &self.variants[self.active]
    }

    fn current_mut(&mut self) -> &mut Variant {
        &mut self.variants[self.active]
    }

    /// The active variant's home.
    pub fn home(&self) -> &Home {
        &self.current().home
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Allocates a fresh wall id. Ids are never reused, even after undo.
    pub fn new_wall_id(&mut self) -> WallId {
        self.current_mut().home.new_wall_id()
    }

    /// Allocates a fresh room id. Ids are never reused, even after undo.
    pub fn new_room_id(&mut self) -> RoomId {
        self.current_mut().home.new_room_id()
    }

    pub fn new_dimension_id(&mut self) -> DimensionId {
        self.current_mut().home.new_dimension_id()
    }

    pub fn new_label_id(&mut self) -> LabelId {
        self.current_mut().home.new_label_id()
    }

    pub fn new_furniture_id(&mut self) -> FurnitureId {
        self.current_mut().home.new_furniture_id()
    }

    pub fn can_undo(&self) -> bool {
        !self.current().undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.current().redo_stack.is_empty()
    }

    /// Executes a command on the active variant, recording it in its history.
    pub fn execute(&mut self, command: Command) -> CoreResult<()> {
        let variant = self.current_mut();
        let inverse = command.apply(&mut variant.home)?;
        variant.undo_stack.push(inverse);
        variant.redo_stack.clear();
        self.revision += 1;
        Ok(())
    }

    pub fn undo(&mut self) -> CoreResult<()> {
        let variant = self.current_mut();
        let command = variant.undo_stack.pop().ok_or(CoreError::NothingToUndo)?;
        let inverse = command.apply(&mut variant.home)?;
        variant.redo_stack.push(inverse);
        self.revision += 1;
        Ok(())
    }

    pub fn redo(&mut self) -> CoreResult<()> {
        let variant = self.current_mut();
        let command = variant.redo_stack.pop().ok_or(CoreError::NothingToRedo)?;
        let inverse = command.apply(&mut variant.home)?;
        variant.undo_stack.push(inverse);
        self.revision += 1;
        Ok(())
    }

    /// Replaces the whole project with a single variant. Counts as saved.
    pub fn load(&mut self, home: Home) {
        self.load_variants(vec![(FIRST_VARIANT_NAME.to_owned(), home)], 0);
    }

    /// Replaces the whole project with these variants. Counts as saved.
    pub fn load_variants(&mut self, variants: Vec<(String, Home)>, active: usize) {
        self.variants = variants
            .into_iter()
            .map(|(name, home)| Variant::new(name, home))
            .collect();
        if self.variants.is_empty() {
            self.variants
                .push(Variant::new(FIRST_VARIANT_NAME, Home::default()));
        }
        self.active = active.min(self.variants.len() - 1);
        self.revision += 1;
        self.saved_revision = self.revision;
    }

    // --- Variants ---------------------------------------------------------------

    pub fn variants(&self) -> impl Iterator<Item = &Variant> {
        self.variants.iter()
    }

    pub fn variant_infos(&self) -> Vec<VariantInfo> {
        self.variants
            .iter()
            .enumerate()
            .map(|(index, v)| VariantInfo {
                index,
                name: v.name.clone(),
                active: index == self.active,
            })
            .collect()
    }

    pub fn active_variant(&self) -> usize {
        self.active
    }

    pub fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// Adds a variant after the active one and switches to it. With
    /// `duplicate`, it starts as a copy of the active home; otherwise empty.
    /// Returns its index.
    pub fn add_variant(&mut self, name: Option<String>, duplicate: bool) -> usize {
        let home = if duplicate {
            self.home().clone()
        } else {
            let mut home = Home::new(self.home().name.clone());
            home.compass = self.home().compass;
            home
        };
        let name = name.unwrap_or_else(|| self.next_variant_name(duplicate));
        let index = self.active + 1;
        self.variants.insert(index, Variant::new(name, home));
        self.active = index;
        self.revision += 1;
        index
    }

    fn next_variant_name(&self, duplicate: bool) -> String {
        if duplicate {
            let base = format!("{} (cópia)", self.current().name);
            let mut name = base.clone();
            let mut n = 2;
            while self.variants.iter().any(|v| v.name == name) {
                name = format!("{base} {n}");
                n += 1;
            }
            name
        } else {
            let mut n = self.variants.len() + 1;
            loop {
                let name = format!("Versão {n}");
                if !self.variants.iter().any(|v| v.name == name) {
                    return name;
                }
                n += 1;
            }
        }
    }

    pub fn switch_variant(&mut self, index: usize) -> CoreResult<()> {
        if index >= self.variants.len() {
            return Err(CoreError::NoSuchVariant(index));
        }
        if index != self.active {
            self.active = index;
            self.revision += 1;
        }
        Ok(())
    }

    pub fn rename_variant(&mut self, index: usize, name: impl Into<String>) -> CoreResult<()> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(CoreError::InvalidGeometry(
                "variant name must not be empty".into(),
            ));
        }
        let variant = self
            .variants
            .get_mut(index)
            .ok_or(CoreError::NoSuchVariant(index))?;
        variant.name = name;
        self.revision += 1;
        Ok(())
    }

    /// Removes a variant; the last one can't be removed.
    pub fn remove_variant(&mut self, index: usize) -> CoreResult<()> {
        if index >= self.variants.len() {
            return Err(CoreError::NoSuchVariant(index));
        }
        if self.variants.len() == 1 {
            return Err(CoreError::InvalidGeometry(
                "a project needs at least one variant".into(),
            ));
        }
        self.variants.remove(index);
        if self.active > index || self.active >= self.variants.len() {
            self.active = self.active.saturating_sub(1);
        }
        self.revision += 1;
        Ok(())
    }

    // --- Files ------------------------------------------------------------------

    /// Marks the current state as saved to `path`.
    pub fn mark_saved(&mut self, path: impl Into<PathBuf>) {
        self.saved_revision = self.revision;
        self.path = Some(path.into());
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Sets the project file path (e.g. after opening a file).
    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    /// True when there are changes since the last save or load.
    pub fn is_modified(&self) -> bool {
        self.revision != self.saved_revision
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point2, Wall};

    fn with_wall(doc: &mut Document) {
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
    }

    #[test]
    fn duplicated_variants_are_independent_with_their_own_history() {
        let mut doc = Document::default();
        with_wall(&mut doc);
        let copy = doc.add_variant(None, true);
        assert_eq!((copy, doc.active_variant()), (1, 1));
        assert_eq!(doc.variant_infos()[1].name, "Versão 1 (cópia)");
        assert_eq!(doc.home().walls.len(), 1, "copy starts equal");
        assert!(!doc.can_undo(), "copy starts with a clean history");

        with_wall(&mut doc);
        assert_eq!(doc.home().walls.len(), 2);
        doc.switch_variant(0).unwrap();
        assert_eq!(doc.home().walls.len(), 1, "original untouched");
        assert!(doc.can_undo(), "original keeps its history");

        let empty = doc.add_variant(None, false);
        assert_eq!(doc.variant_infos()[empty].name, "Versão 3");
        assert!(doc.home().walls.is_empty());
    }

    #[test]
    fn removing_and_renaming_variants_keeps_a_valid_active_one() {
        let mut doc = Document::default();
        doc.add_variant(Some("B".into()), true);
        doc.add_variant(Some("C".into()), true);
        assert_eq!(doc.active_variant(), 2);
        doc.remove_variant(2).unwrap();
        assert_eq!(doc.active_variant(), 1);
        doc.rename_variant(0, "Térreo").unwrap();
        assert_eq!(doc.variant_infos()[0].name, "Térreo");
        doc.remove_variant(0).unwrap();
        assert_eq!((doc.variant_count(), doc.active_variant()), (1, 0));
        assert!(doc.remove_variant(0).is_err(), "last variant stays");
        assert!(doc.switch_variant(5).is_err());
    }

    #[test]
    fn variant_changes_mark_the_project_modified() {
        let mut doc = Document::default();
        doc.mark_saved("/tmp/x.newera");
        doc.add_variant(None, true);
        assert!(doc.is_modified());
    }
}
