//! Native project file: pretty, versioned JSON (`.newera`).
//!
//! JSON keeps projects diff-friendly and readable by agents and scripts.
//! Paths inside the home (background images) are stored as written; callers
//! resolve relative paths against the project file's directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::home::Home;

pub const PROJECT_EXTENSION: &str = "newera";
const FORMAT: &str = "3d-new-era-ai";
const VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("not a 3D New Era AI project")]
    WrongFormat,
    #[error("project version {0} is newer than this app supports ({VERSION})")]
    TooNew(u32),
    #[error("invalid project file: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize, Deserialize)]
struct FileRef<'a> {
    format: &'a str,
    version: u32,
    home: HomeField<'a>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum HomeField<'a> {
    #[serde(skip_deserializing)]
    Borrowed(&'a Home),
    Owned(Box<Home>),
}

/// Serializes a home as a project file.
///
/// # Panics
///
/// Never in practice: every [`Home`] is representable as JSON.
pub fn to_project_json(home: &Home) -> String {
    let file = FileRef {
        format: FORMAT,
        version: VERSION,
        home: HomeField::Borrowed(home),
    };
    serde_json::to_string_pretty(&file).expect("home is always serializable")
}

pub fn from_project_json(json: &str) -> Result<Home, ProjectError> {
    #[derive(Deserialize)]
    struct Header {
        format: Option<String>,
        version: Option<u32>,
    }
    let header: Header = serde_json::from_str(json)?;
    if header.format.as_deref() != Some(FORMAT) {
        return Err(ProjectError::WrongFormat);
    }
    let version = header.version.unwrap_or(0);
    if version > VERSION {
        return Err(ProjectError::TooNew(version));
    }
    let file: FileRef<'_> = serde_json::from_str(json)?;
    match file.home {
        HomeField::Owned(home) => Ok(*home),
        HomeField::Borrowed(_) => unreachable!("borrowed variant is never deserialized"),
    }
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
    use crate::{Command, Document, Point2, Wall};

    #[test]
    fn round_trips_and_keeps_id_counter() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
        let json = to_project_json(doc.home());
        assert!(json.contains("\"format\": \"3d-new-era-ai\""));

        let mut loaded = from_project_json(&json).unwrap();
        assert_eq!(&loaded, doc.home());
        assert_eq!(loaded.new_wall_id().0, 2, "counter survives saving");
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
