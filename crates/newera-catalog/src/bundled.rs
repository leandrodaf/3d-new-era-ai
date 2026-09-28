//! Finished product assets embedded in native and browser builds.
use crate::{Mesh, load_model, rgb};
use newera_core::{Furniture, PieceInfo};
use std::{
    path::{Path, PathBuf},
    sync::{Once, OnceLock},
};

#[cfg(windows)]
const ROOT: &str = "C:/__newera_builtin__/tokstok";
#[cfg(not(windows))]
const ROOT: &str = "/__newera_builtin__/tokstok";

fn slug(id: &str) -> Option<&'static str> {
    match id {
        "tokstok-win" => Some("win"),
        "tokstok-ares" => Some("ares"),
        _ => None,
    }
}

fn mount() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let files: &[(&str, &[u8])] = &[
            (
                "win/madeira-foto-normal.png",
                include_bytes!("../assets/tokstok/win/madeira-foto-normal.png"),
            ),
            (
                "win/madeira-foto.png",
                include_bytes!("../assets/tokstok/win/madeira-foto.png"),
            ),
            (
                "win/model.mtl",
                include_bytes!("../assets/tokstok/win/model.mtl"),
            ),
            (
                "win/model.obj",
                include_bytes!("../assets/tokstok/win/model.obj"),
            ),
            (
                "win/tecido-foto-normal.png",
                include_bytes!("../assets/tokstok/win/tecido-foto-normal.png"),
            ),
            (
                "win/tecido-foto.png",
                include_bytes!("../assets/tokstok/win/tecido-foto.png"),
            ),
            (
                "ares/madeira-foto-normal.png",
                include_bytes!("../assets/tokstok/ares/madeira-foto-normal.png"),
            ),
            (
                "ares/madeira-foto.png",
                include_bytes!("../assets/tokstok/ares/madeira-foto.png"),
            ),
            (
                "ares/model.mtl",
                include_bytes!("../assets/tokstok/ares/model.mtl"),
            ),
            (
                "ares/model.obj",
                include_bytes!("../assets/tokstok/ares/model.obj"),
            ),
            (
                "ares/tecido-foto-normal.png",
                include_bytes!("../assets/tokstok/ares/tecido-foto-normal.png"),
            ),
            (
                "ares/tecido-foto.png",
                include_bytes!("../assets/tokstok/ares/tecido-foto.png"),
            ),
        ];
        newera_core::vfs::mount(
            Path::new(ROOT),
            files
                .iter()
                .map(|(name, bytes)| ((*name).to_owned(), bytes.to_vec())),
        );
    });
}

fn path(id: &str) -> Option<PathBuf> {
    let slug = slug(id)?;
    mount();
    Some(Path::new(ROOT).join(slug).join("model.obj"))
}

pub(crate) fn configure(piece: &mut Furniture) {
    let Some(path) = path(&piece.catalog) else {
        return;
    };
    let win = piece.catalog == "tokstok-win";
    piece.model = Some(path.to_string_lossy().into_owned());
    piece.locks.deformable = false;
    piece.info = PieceInfo {
        brand: Some("Tok&Stok".into()),
        model_name: Some(if win { "Win — nozes / English green" } else { "Ares — tauari natural / palhinha" }.into()),
        source_catalog_id: Some(if win { "Tok&Stok#295718" } else { "Tok&Stok#322442" }.into()),
        url: Some(if win { "https://www.tokstok.com.br/poltrona-nozes-english-green-win/p" } else { "https://www.tokstok.com.br/cadeira-com-palhinha-tauari-natural-ares/p" }.into()),
        description: Some(if win { "Poltrona em pinus nozes; estofado English green; assento a 42 cm." } else { "Cadeira em madeira com acabamento tauari natural, encosto de palhinha e assento bege a 49,5 cm." }.into()),
        information: Some("Reconstrução por fotografias do produto; detalhes locais estimados.".into()),
        ..PieceInfo::default()
    };
}

pub(crate) fn mesh(piece: &Furniture) -> Option<Mesh> {
    static WIN: OnceLock<Mesh> = OnceLock::new();
    static ARES: OnceLock<Mesh> = OnceLock::new();
    let path = path(&piece.catalog)?;
    let cache = if piece.catalog == "tokstok-win" {
        &WIN
    } else {
        &ARES
    };
    let mut mesh = cache
        .get_or_init(|| load_model(&path).expect("validated bundled model").mesh)
        .clone();
    mesh.fit_to(piece.width, piece.depth, piece.height);
    if let Some(color) = piece.color {
        let color = rgb(color);
        mesh.colors.fill(color);
        for material in &mut mesh.materials {
            material.color = color;
        }
    }
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn products_are_portable_textured_and_categorized() {
        for (id, category, size) in [
            ("tokstok-win", crate::Category::Living, [69.5, 83.5, 73.0]),
            ("tokstok-ares", crate::Category::Dining, [47.0, 62.0, 81.0]),
        ] {
            let item = crate::find(id).unwrap();
            assert_eq!(item.category, category);
            let piece =
                item.instantiate(newera_core::FurnitureId(1), newera_core::Point2::default());
            assert!(!piece.locks.deformable);
            assert_eq!(piece.info.brand.as_deref(), Some("Tok&Stok"));
            let path = Path::new(piece.model.as_deref().unwrap());
            assert!(path.is_absolute());
            let imported = load_model(path).unwrap();
            for (actual, expected) in imported.size.iter().zip(size) {
                assert!((actual - expected).abs() < 0.01);
            }
            assert!(imported.mesh.indices.len() > 3000);
            assert_eq!(imported.mesh.uvs.len(), imported.mesh.positions.len());
            let textures: Vec<_> = imported
                .mesh
                .materials
                .iter()
                .filter_map(|m| m.texture.as_ref())
                .collect();
            assert_eq!(textures.len(), 2);
            for texture in textures {
                assert!(
                    newera_core::vfs::read(texture)
                        .unwrap()
                        .starts_with(b"\x89PNG")
                );
            }
            for companion in newera_core::model_companions(path) {
                assert!(newera_core::vfs::read(&path.parent().unwrap().join(companion)).is_ok());
            }
        }
    }

    #[test]
    fn saved_products_reopen_with_their_materials_without_source_files() {
        let root =
            std::env::temp_dir().join(format!("newera-bundled-products-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut home = newera_core::Home::default();
        for (i, id) in ["tokstok-win", "tokstok-ares"].iter().enumerate() {
            home.furniture.push(crate::find(id).unwrap().instantiate(
                newera_core::FurnitureId(u64::try_from(i + 1).unwrap()),
                newera_core::Point2::default(),
            ));
        }
        let file = root.join("products.newera");
        newera_core::save_project(&newera_core::Document::new(home), &file).unwrap();
        let (project, dir) = newera_core::open_project(&file, &root.join("cache")).unwrap();
        let dir = dir.unwrap();
        for piece in &project.variants[0].1.furniture {
            let path = newera_core::resolve_asset(Some(&dir), piece.model.as_deref().unwrap());
            assert!(path.starts_with(&root));
            let model = load_model(&path).unwrap();
            assert!(
                model
                    .mesh
                    .materials
                    .iter()
                    .filter_map(|m| m.texture.as_ref())
                    .all(|p| p.is_file())
            );
            assert_eq!(piece.info.brand.as_deref(), Some("Tok&Stok"));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
