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
        .try_fold(0usize, |n, b| n.checked_add(b.len()))
        .ok_or("assets exceed render budget")?;
    if size > limit {
        return Err(format!(
            "Os modelos e texturas excedem o limite de {limit} bytes para renderizar neste aparelho."
        ));
    }
    Ok(files
        .iter()
        .map(|(p, b)| (p.to_string_lossy().into_owned(), b.clone()))
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
