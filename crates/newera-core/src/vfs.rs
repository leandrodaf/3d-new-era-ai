//! Files in memory next to the real file system. Where there is no disk
//! (the browser editor), project bundles and imported `.sh3d` files mount
//! their models and textures here, and every loader reads through [`read`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

type Files = HashMap<PathBuf, Arc<[u8]>>;

static FILES: RwLock<Option<Files>> = RwLock::new(None);

fn key(path: &Path) -> PathBuf {
    // Normalize `a/./b` and separators so lookups match however paths were built.
    path.components().collect()
}

/// Makes `files` (relative names) readable under `dir`.
pub fn mount(dir: &Path, files: impl IntoIterator<Item = (String, Vec<u8>)>) {
    let mut guard = FILES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let map = guard.get_or_insert_with(HashMap::new);
    for (name, bytes) in files {
        map.insert(key(&dir.join(name)), bytes.into());
    }
}

/// Forgets every file mounted under `dir`.
pub fn unmount(dir: &Path) {
    let mut guard = FILES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(map) = guard.as_mut() {
        let dir = key(dir);
        map.retain(|path, _| !path.starts_with(&dir));
    }
}

fn mounted(path: &Path) -> Option<Arc<[u8]>> {
    let guard = FILES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.as_ref()?.get(&key(path)).cloned()
}

/// Reads a mounted file, or the file on disk.
///
/// # Errors
/// When it is neither mounted nor readable from disk.
pub fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    match mounted(path) {
        Some(bytes) => Ok(bytes.to_vec()),
        None => std::fs::read(path),
    }
}

/// Whether the file is mounted or exists on disk.
pub fn exists(path: &Path) -> bool {
    mounted(path).is_some() || path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounted_files_shadow_the_disk_until_unmounted() {
        let dir = Path::new("/virtual/project-test");
        mount(dir, [("models/chair.obj".to_owned(), b"v 0 0 0".to_vec())]);
        assert_eq!(read(&dir.join("models/./chair.obj")).unwrap(), b"v 0 0 0");
        assert!(exists(&dir.join("models/chair.obj")));
        assert!(read(&dir.join("missing.png")).is_err());
        unmount(dir);
        assert!(!exists(&dir.join("models/chair.obj")));
    }
}
