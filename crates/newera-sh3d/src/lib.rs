//! Import of Sweet Home 3D projects (`.sh3d`).
//!
//! A `.sh3d` file is a ZIP archive holding the home (a Java serialization
//! stream named `Home`, and in recent versions also `Home.xml`) plus the
//! models, textures and icons it references. This crate reads that format
//! from its public description; it contains no Sweet Home 3D code.

mod import;
pub mod javaser;

pub use import::{BundledFiles, ImportError, Imported, import_bytes, import_file};

use std::path::{Path, PathBuf};

use newera_core::Document;

/// Whether a path names a Sweet Home 3D file.
pub fn is_sh3d(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sh3d"))
}

/// What opening a file did.
#[derive(Debug, Default)]
pub struct Opened {
    /// The file was imported from another format and not yet saved as a project.
    pub imported: bool,
    pub warnings: Vec<String>,
}

/// A file read and ready to replace the open document.
///
/// Reading is the slow half — unzipping, parsing, writing out every texture
/// and model — and it touches no document, so it can be done off the thread
/// that draws the window while [`Loaded::into_document`] takes the fast half.
#[derive(Debug)]
pub enum Loaded {
    /// A project of this app, with the directory its assets were unpacked to.
    Project {
        project: newera_core::Project,
        path: PathBuf,
        assets: Option<PathBuf>,
    },
    /// A home brought in from another format, not a project yet.
    Imported {
        home: newera_core::Home,
        assets: PathBuf,
        warnings: Vec<String>,
    },
}

impl Loaded {
    /// Puts what was read into `doc`, replacing its content.
    pub fn into_document(self, doc: &mut Document) -> Opened {
        match self {
            Self::Project {
                project,
                path,
                assets,
            } => {
                project.load_into(doc);
                doc.mark_saved(&path);
                doc.set_asset_dir(assets);
                Opened::default()
            }
            Self::Imported {
                home,
                assets,
                warnings,
            } => {
                doc.load(home);
                doc.set_path(None);
                doc.set_asset_dir(Some(assets));
                Opened {
                    imported: true,
                    warnings,
                }
            }
        }
    }
}

/// Reads a `.newera` project or a `.sh3d` file, without touching any
/// document. Reports what it is doing through [`newera_core::progress`].
///
/// # Errors
/// When the file cannot be read, is not one of these formats, or whoever is
/// waiting gave up.
pub fn read_file(path: &Path) -> Result<Loaded, String> {
    let cache = newera_core::cache_dir();
    if is_sh3d(path) {
        let stem = path
            .file_stem()
            .map_or_else(|| "home".to_owned(), |s| s.to_string_lossy().into_owned());
        let stamp = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let assets = cache.join("imports").join(format!("{stem}-{stamp}"));
        let imported = import_file(path, &assets).map_err(|e| e.to_string())?;
        return Ok(Loaded::Imported {
            home: imported.home,
            assets,
            warnings: imported.warnings,
        });
    }
    let (project, assets) =
        newera_core::open_project(path, &cache.join("bundles")).map_err(|e| e.to_string())?;
    Ok(Loaded::Project {
        project,
        path: path.to_owned(),
        assets,
    })
}

/// Opens a `.newera` project or imports a `.sh3d` file into `doc`,
/// replacing its content. Imports stay unsaved (no project path) so the
/// next save asks where to put the new project.
///
/// # Errors
/// See [`read_file`].
pub fn open_file(doc: &mut Document, path: &Path) -> Result<Opened, String> {
    Ok(read_file(path)?.into_document(doc))
}
