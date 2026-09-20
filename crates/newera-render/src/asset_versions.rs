use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use newera_core::vfs::{AssetRevision, revision};

#[derive(Debug, Default)]
pub(crate) struct AssetVersions {
    sources: HashMap<PathBuf, Source>,
}

#[derive(Debug)]
struct Source {
    files: Vec<(PathBuf, AssetRevision)>,
    fingerprint: u64,
}

impl AssetVersions {
    /// Discovery parses a model only when it or a known companion changes.
    /// Ordinary cache lookups inspect revisions, without re-reading meshes
    /// or decoding images. Missing companions remain tracked for recovery.
    pub(crate) fn get(&mut self, path: &Path) -> u64 {
        if let Some(source) = self.sources.get(path)
            && source.files.iter().all(|(p, was)| revision(p) == *was)
        {
            return source.fingerprint;
        }
        // The core discovery already includes transitive OBJ material textures.
        // Do not recursively interpret texture names as models.
        let mut seen = BTreeSet::from([path.to_path_buf()]);
        if let Some(dir) = path.parent() {
            for file in newera_core::model_companions(path) {
                seen.insert(dir.join(file).components().collect());
            }
        }
        let files: Vec<_> = seen
            .into_iter()
            .map(|p| {
                let stamp = revision(&p);
                (p, stamp)
            })
            .collect();
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        files.hash(&mut hash);
        let fingerprint = hash.finish();
        self.sources
            .insert(path.to_path_buf(), Source { files, fingerprint });
        fingerprint
    }
}
