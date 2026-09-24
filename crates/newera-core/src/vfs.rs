//! Files in memory next to the real file system. Where there is no disk
//! (the browser editor), project bundles and imported `.sh3d` files mount
//! their models and textures here, and every loader reads through [`read`].

use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

struct MountedFile {
    bytes: Arc<[u8]>,
    fingerprint: u64,
}

type Files = HashMap<PathBuf, MountedFile>;

/// Cheap change identity for cache validation; mounted bytes are hashed once
/// on insertion, never copied or rehashed on each frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetRevision {
    Missing,
    Mounted(u64, usize),
    Disk {
        len: u64,
        modified: Option<std::time::SystemTime>,
        created: Option<std::time::SystemTime>,
        #[cfg(unix)]
        changed: (i64, i64, u64, u64),
    },
}

pub fn revision(path: &Path) -> AssetRevision {
    let guard = FILES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(file) = guard.as_ref().and_then(|files| files.get(&key(path))) {
        return AssetRevision::Mounted(file.fingerprint, file.bytes.len());
    }
    drop(guard);
    let Ok(meta) = std::fs::metadata(path) else {
        return AssetRevision::Missing;
    };
    AssetRevision::Disk {
        len: meta.len(),
        modified: meta.modified().ok(),
        created: meta.created().ok(),
        #[cfg(unix)]
        changed: (meta.ctime(), meta.ctime_nsec(), meta.dev(), meta.ino()),
    }
}

static FILES: RwLock<Option<Files>> = RwLock::new(None);

fn key(path: &Path) -> PathBuf {
    // Normalize `a/./b` and separators so lookups match however paths were built.
    path.components().collect()
}

/// Makes `files` (relative names) readable under `dir`.
pub fn mount(dir: &Path, files: impl IntoIterator<Item = (String, Vec<u8>)>) {
    use std::hash::{Hash, Hasher};
    let prepared: Vec<_> = files
        .into_iter()
        .map(|(name, bytes)| {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            bytes.hash(&mut hash);
            (
                key(&dir.join(name)),
                MountedFile {
                    fingerprint: hash.finish(),
                    bytes: bytes.into(),
                },
            )
        })
        .collect();
    let mut guard = FILES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.get_or_insert_with(HashMap::new).extend(prepared);
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
    guard
        .as_ref()?
        .get(&key(path))
        .map(|file| file.bytes.clone())
}

/// Shared mounted files to copy into an isolated renderer.
pub type AssetSnapshot = Vec<(String, Arc<[u8]>)>;

/// Bounded snapshot for an isolated browser render worker, sharing source bytes.
pub fn snapshot(limit: usize) -> Result<AssetSnapshot, String> {
    let guard = FILES.read().map_err(|_| "assets unavailable")?;
    let Some(files) = guard.as_ref() else {
        return Ok(Vec::new());
    };
    let size = files
        .values()
        .try_fold(0usize, |n, b| n.checked_add(b.bytes.len()))
        .ok_or("assets exceed render budget")?;
    if size > limit {
        return Err(format!(
            "Os modelos e texturas excedem o limite de {limit} bytes para renderizar neste aparelho."
        ));
    }
    Ok(files
        .iter()
        .map(|(p, b)| (p.to_string_lossy().into_owned(), b.bytes.clone()))
        .collect())
}

/// Only mounted files referenced by this scene, plus model dependencies.
/// Unrelated projects and unused imports consume no worker budget. Reads
/// never fall through to the native filesystem while discovering companions.
pub fn snapshot_paths(
    paths: impl IntoIterator<Item = PathBuf>,
    limit: usize,
) -> Result<AssetSnapshot, String> {
    let mut pending: std::collections::BTreeSet<_> = paths.into_iter().map(|p| key(&p)).collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    let mut size = 0usize;
    while let Some(path) = pending.pop_first() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let Some(bytes) = mounted(&path) else {
            continue;
        };
        size = size
            .checked_add(bytes.len())
            .ok_or("assets exceed render budget")?;
        if size > limit {
            return Err(format!(
                "Os modelos e texturas excedem o limite de {limit} bytes para renderizar neste aparelho."
            ));
        }
        for companion in crate::project::model_companions_with(&path, |p| {
            let bytes = mounted(p)?;
            (bytes.len() <= limit).then(|| bytes.to_vec())
        }) {
            if let Some(dir) = path.parent() {
                pending.insert(key(&dir.join(companion)));
            }
        }
        out.push((path.to_string_lossy().into_owned(), bytes));
    }
    Ok(out)
}

/// Reads a mounted file, or the file on disk.
///
/// # Errors
/// When it is neither mounted nor readable from disk.
pub fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    match mounted(path) {
        Some(bytes) => Ok(bytes.to_vec()),
        None if !allowed(path) => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "outside the projects",
        )),
        None => std::fs::read(path),
    }
}

/// Whether the file is mounted or exists on disk.
pub fn exists(path: &Path) -> bool {
    mounted(path).is_some() || (allowed(path) && path.exists())
}

/// Where disk reads may go, once set: a server running projects for many
/// people must not let one of them name a texture like `/proc/self/environ`
/// and have it read. The desktop never sets it.
static JAIL: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Confines every disk read through here to `root`, for the rest of the
/// process. Called once; a second call is ignored.
pub fn jail(root: &Path) {
    let _ = JAIL.set(key(root));
}

/// Whether a disk read of `path` is allowed: always, unless jailed; then
/// only a path inside the jail with no `..` in it.
pub fn allowed(path: &Path) -> bool {
    let Some(root) = JAIL.get() else {
        return true;
    };
    let path = key(path);
    path.is_absolute()
        && path.starts_with(root)
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Outside the jail nothing is read; inside, only without `..`.
    #[test]
    fn a_jail_confines_reads() {
        // The jail is process-wide and set once, so the check is on the rule
        // itself with a root of its own.
        let root = key(Path::new("/srv/newera/projects"));
        let inside = |p: &str| {
            let path = key(Path::new(p));
            path.is_absolute()
                && path.starts_with(&root)
                && !path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
        };
        assert!(inside("/srv/newera/projects/p1/wood.jpg"));
        assert!(!inside("/proc/self/environ"));
        assert!(!inside("/srv/newera/projects/../../etc/passwd"));
        assert!(!inside("relative/texture.png"));
    }

    #[test]
    fn scene_snapshot_keeps_model_dependencies_without_charging_unused_files() {
        let dir = Path::new("/virtual/scene-dependency-test");
        let json = br#"{"buffers":[{"uri":"mesh.bin"}],"images":[{"uri":"image.png"},{"uri":"data:image/png;base64,AA=="}]}"#;
        let mut padded = json.to_vec();
        while !padded.len().is_multiple_of(4) {
            padded.push(b' ');
        }
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&u32::try_from(20 + padded.len()).unwrap().to_le_bytes());
        glb.extend_from_slice(&u32::try_from(padded.len()).unwrap().to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&padded);
        mount(
            dir,
            [
                ("chair.obj".into(), b"mtllib chair.mtl\nv 0 0 0\n".to_vec()),
                (
                    "chair.mtl".into(),
                    b"newmtl wood\nmap_Kd wood grain.png\n".to_vec(),
                ),
                ("wood grain.png".into(), vec![1, 2, 3, 4]),
                ("simple.gltf".into(), json.to_vec()),
                ("external.glb".into(), glb),
                ("mesh.bin".into(), vec![5, 6, 7, 8]),
                ("image.png".into(), vec![9, 10, 11, 12]),
                ("unused-import.bin".into(), vec![0; 16_384]),
            ],
        );
        let roots = || {
            [
                dir.join("chair.obj"),
                dir.join("simple.gltf"),
                dir.join("external.glb"),
                dir.join("image.png"),
                dir.join("chair.obj"),
            ]
        };
        assert!(snapshot(1024).is_err());
        for model in ["chair.obj", "simple.gltf", "external.glb"] {
            assert_eq!(
                snapshot_paths([dir.join(model)], 1024).unwrap().len(),
                3,
                "{model}"
            );
        }
        let selected = snapshot_paths(roots(), 1024).unwrap();
        assert_eq!(selected.len(), 7);
        assert!(
            !selected
                .iter()
                .any(|(p, _)| p.ends_with("unused-import.bin"))
        );
        let total = selected.iter().map(|(_, b)| b.len()).sum();
        assert!(snapshot_paths(roots(), total - 1).is_err());
        assert_eq!(snapshot_paths(roots(), total).unwrap().len(), 7);
        unmount(dir);
        assert_eq!(
            selected
                .iter()
                .find(|(p, _)| p.ends_with("mesh.bin"))
                .unwrap()
                .1
                .as_ref(),
            &[5, 6, 7, 8]
        );
    }

    #[test]
    fn malformed_glb_dependencies_do_not_panic_or_read_outside_the_buffer() {
        let dir = Path::new("/virtual/truncated-glb-test");
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&20u32.to_le_bytes());
        glb.extend_from_slice(&u32::MAX.to_le_bytes());
        glb.extend_from_slice(b"JSON");
        mount(dir, [("broken.glb".into(), glb)]);
        assert_eq!(
            snapshot_paths([dir.join("broken.glb")], 20).unwrap().len(),
            1
        );
        unmount(dir);
    }

    #[test]
    fn worker_snapshot_checks_budget_and_owns_assets_after_unmount() {
        let dir = Path::new("/virtual/render-snapshot-test");
        mount(dir, [("a.bin".into(), vec![1, 2, 3, 4])]);
        assert!(snapshot(0).is_err());
        let files = snapshot(usize::MAX).unwrap();
        unmount(dir);
        let (_, bytes) = files
            .iter()
            .find(|(p, _)| Path::new(p) == dir.join("a.bin"))
            .unwrap();
        assert_eq!(bytes.as_ref(), &[1, 2, 3, 4]);
    }

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
