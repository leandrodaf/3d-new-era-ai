//! Import of Sweet Home 3D projects (`.sh3d`).
//!
//! A `.sh3d` file is a ZIP archive holding the home (a Java serialization
//! stream named `Home`, and in recent versions also `Home.xml`) plus the
//! models, textures and icons it references. This crate reads that format
//! from its public description; it contains no Sweet Home 3D code.

mod import;
pub mod javaser;

pub use import::{BundledFiles, ImportError, Imported, import_bytes, import_file};

use std::path::Path;

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

/// Opens a `.newera` project or imports a `.sh3d` file into `doc`,
/// replacing its content. Imports stay unsaved (no project path) so the
/// next save asks where to put the new project.
pub fn open_file(doc: &mut Document, path: &Path) -> Result<Opened, String> {
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
        doc.load(imported.home);
        doc.set_path(None);
        doc.set_asset_dir(Some(assets));
        return Ok(Opened {
            imported: true,
            warnings: imported.warnings,
        });
    }
    let (project, assets) =
        newera_core::open_project(path, &cache.join("bundles")).map_err(|e| e.to_string())?;
    project.load_into(doc);
    doc.mark_saved(path);
    doc.set_asset_dir(assets);
    Ok(Opened::default())
}
