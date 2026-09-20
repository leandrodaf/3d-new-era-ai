use std::hash::{Hash, Hasher};
use std::path::Path;

fn hash_tree(path: &Path, hash: &mut impl Hasher) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .expect("source directory")
            .map(|entry| entry.expect("source entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            hash_tree(&entry, hash);
        }
    } else {
        path.hash(hash);
        std::fs::read(path).expect("source file").hash(hash);
    }
}

fn main() {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for path in ["src", "Cargo.toml", "build.rs"] {
        println!("cargo:rerun-if-changed={path}");
        hash_tree(Path::new(path), &mut hash);
    }
    // Workspace builds also invalidate cached images when dependencies change.
    if Path::new("../../Cargo.lock").exists() {
        println!("cargo:rerun-if-changed=../../Cargo.lock");
        hash_tree(Path::new("../../Cargo.lock"), &mut hash);
    }
    println!(
        "cargo:rustc-env=NEWERA_SOURCE_REVISION={:016x}",
        hash.finish()
    );
}
