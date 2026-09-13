//! Native project file: pretty, versioned JSON (`.newera`).
//!
//! JSON keeps projects diff-friendly and readable by agents and scripts.
//! Paths inside the home (background images) are stored as written; callers
//! resolve relative paths against the project file's directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::{Document, FIRST_VARIANT_NAME};
use crate::home::Home;

pub const PROJECT_EXTENSION: &str = "newera";
const FORMAT: &str = "3d-new-era-ai";
/// 1: a single `home`. 2: `variants` (versions of the plan) and `active`.
const VERSION: u32 = 2;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("not a 3D New Era AI project")]
    WrongFormat,
    #[error("project version {0} is newer than this app supports ({VERSION})")]
    TooNew(u32),
    #[error("invalid project file: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize)]
struct VariantOut<'a> {
    name: &'a str,
    home: &'a Home,
}

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'a str,
    version: u32,
    active: usize,
    variants: Vec<VariantOut<'a>>,
}

#[derive(Deserialize)]
struct VariantIn {
    name: String,
    home: Home,
}

#[derive(Deserialize)]
struct FileIn {
    format: Option<String>,
    version: Option<u32>,
    #[serde(default)]
    active: usize,
    #[serde(default)]
    variants: Vec<VariantIn>,
    /// Version 1 files.
    home: Option<Home>,
}

/// A project read from disk: its variants and which one is active.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub variants: Vec<(String, Home)>,
    pub active: usize,
}

impl Project {
    /// Replaces a document's content with this project.
    pub fn load_into(self, doc: &mut Document) {
        doc.load_variants(self.variants, self.active);
    }
}

/// Serializes every variant of a document as a project file.
///
/// # Panics
///
/// Never in practice: every [`Home`] is representable as JSON.
pub fn to_project_json(doc: &Document) -> String {
    let file = FileOut {
        format: FORMAT,
        version: VERSION,
        active: doc.active_variant(),
        variants: doc
            .variants()
            .map(|v| VariantOut {
                name: &v.name,
                home: v.home(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&file).expect("home is always serializable")
}

pub fn from_project_json(json: &str) -> Result<Project, ProjectError> {
    let file: FileIn = serde_json::from_str(json)?;
    if file.format.as_deref() != Some(FORMAT) {
        return Err(ProjectError::WrongFormat);
    }
    let version = file.version.unwrap_or(0);
    if version > VERSION {
        return Err(ProjectError::TooNew(version));
    }
    let variants: Vec<(String, Home)> = if file.variants.is_empty() {
        vec![(FIRST_VARIANT_NAME.to_owned(), file.home.unwrap_or_default())]
    } else {
        file.variants
            .into_iter()
            .map(|v| (v.name, v.home))
            .collect()
    };
    let active = file.active.min(variants.len() - 1);
    Ok(Project { variants, active })
}

/// Resolves a path stored in a project relative to the project file.
pub fn resolve_project_path(project: Option<&Path>, stored: &str) -> PathBuf {
    let stored = Path::new(stored);
    match project.and_then(Path::parent) {
        Some(dir) if stored.is_relative() => dir.join(stored),
        _ => stored.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Point2, Wall};

    #[test]
    fn round_trips_all_variants_and_keeps_id_counters() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
        doc.add_variant(Some("Sem parede".into()), false);
        let json = to_project_json(&doc);
        assert!(json.contains("\"format\": \"3d-new-era-ai\""));
        assert!(json.contains("\"version\": 2"));

        let project = from_project_json(&json).unwrap();
        assert_eq!(project.active, 1);
        assert_eq!(project.variants.len(), 2);
        assert_eq!(project.variants[1].0, "Sem parede");
        let mut loaded = Document::default();
        project.load_into(&mut loaded);
        loaded.switch_variant(0).unwrap();
        assert_eq!(loaded.home().walls.len(), 1);
        assert_eq!(loaded.new_wall_id().0, 2, "counter survives saving");
    }

    #[test]
    fn reads_version_1_files_as_a_single_variant() {
        let v1 = r#"{"format":"3d-new-era-ai","version":1,"home":{"name":"Antiga"}}"#;
        let project = from_project_json(v1).unwrap();
        assert_eq!(project.variants.len(), 1);
        assert_eq!(project.variants[0].0, FIRST_VARIANT_NAME);
        assert_eq!(project.variants[0].1.name, "Antiga");
    }

    #[test]
    fn rejects_foreign_and_future_files() {
        assert!(matches!(
            from_project_json(r#"{"a":1}"#),
            Err(ProjectError::WrongFormat)
        ));
        let future = r#"{"format":"3d-new-era-ai","version":99,"home":{"name":"x"}}"#;
        assert!(matches!(
            from_project_json(future),
            Err(ProjectError::TooNew(99))
        ));
    }

    #[test]
    fn resolves_relative_paths_next_to_the_project() {
        let p = resolve_project_path(Some(Path::new("/tmp/casa/projeto.newera")), "planta.png");
        assert_eq!(p, PathBuf::from("/tmp/casa/planta.png"));
        let abs = resolve_project_path(Some(Path::new("/tmp/casa/projeto.newera")), "/img/a.png");
        assert_eq!(abs, PathBuf::from("/img/a.png"));
    }
}
