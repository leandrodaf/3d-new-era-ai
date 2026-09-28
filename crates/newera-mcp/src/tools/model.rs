//! Imported 3D models, looked at from the inside: what a file became once
//! loaded, so a piece that looks wrong can be told apart from a file that
//! is wrong or a feature that is not drawn.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::NewEraMcp;
use super::reply::invalid;

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelParams {
    /// A piece that uses an imported model, e.g. `f12`.
    id: Option<String>,
    /// Instead of `id`: an .obj/.gltf/.glb file, before placing it.
    file: Option<String>,
}

/// `#rrggbb` of a material color in `0..=1`.
fn hex(c: [f32; 3]) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]))
}

/// Rounded to a millimeter, as every other size the tools answer.
fn cm(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// What `model` answers for a loaded file.
pub(crate) fn report(file: &str, loaded: &newera_catalog::ImportedModel) -> Value {
    let mesh = &loaded.mesh;
    let mut tris = vec![0usize; mesh.materials.len()];
    for tri in mesh.indices.chunks(3) {
        if let Some(k) = mesh.vertex_materials.get(tri[0] as usize)
            && let Some(n) = tris.get_mut(usize::from(*k))
        {
            *n += 1;
        }
    }
    let materials: Vec<Value> = mesh
        .materials
        .iter()
        .zip(&tris)
        .map(|(m, n)| {
            let image = m
                .texture
                .as_ref()
                .and_then(|t| t.file_name())
                .map(|f| f.to_string_lossy().into_owned());
            json!([m.name, hex(m.color), image, n])
        })
        .collect();
    let images: std::collections::BTreeSet<_> = mesh
        .materials
        .iter()
        .filter_map(|m| m.texture.as_ref())
        .collect();
    let r = &loaded.report;
    json!({
        "file": file,
        "format": r.format,
        "unit": r.unit,
        "raw": r.raw_size.map(|v| (v * 1000.0).round() / 1000.0),
        "size": loaded.size.map(cm),
        "tris": mesh.indices.len() / 3,
        "meshes": r.meshes,
        "materials": materials,
        "images": images.len(),
        "uv": mesh.uvs.iter().any(|uv| *uv != [0.0, 0.0]),
        "warnings": r.warnings,
    })
}

#[tool_router(router = model_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Inspect an imported 3D model, a piece's (id) or a file before placing it: the unit taken for its numbers and its natural size, triangles, its materials with their images, and warnings for what is not drawn as the file says (a texture or material library not found, normal and roughness maps, alpha masks, extensions). For a piece that looks wrong after place model=…, instead of reading the file. Reply {file, format, unit, raw (file units), size cm, tris, materials [[name, color, image, tris]], images, uv, warnings}; with id also piece {size, scale per axis}. update's material overrides change a material by its name here."
    )]
    pub(crate) fn model(
        &self,
        Parameters(p): Parameters<ModelParams>,
    ) -> Result<String, ErrorData> {
        let doc = self.document.read();
        let (file, piece) = match (&p.id, &p.file) {
            (Some(raw), None) => {
                let id = raw
                    .parse::<newera_core::FurnitureId>()
                    .map_err(|e| invalid(format!("id: {e}")))?;
                let piece = doc
                    .home()
                    .find_piece(id)
                    .ok_or_else(|| invalid(format!("{raw} not found")))?;
                let model = piece.model.clone().ok_or_else(|| {
                    invalid(format!(
                        "{raw} is built from the catalog (`{}`), not an imported model",
                        piece.catalog
                    ))
                })?;
                (model, Some(piece.clone()))
            }
            (None, Some(file)) => (file.clone(), None),
            _ => return Err(invalid("give `id` (a placed piece) or `file`, one of them")),
        };
        let path = doc.resolve_asset(&file);
        let loaded = newera_catalog::load_model(&path)
            .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        let mut out = report(&file, &loaded);
        if let Some(piece) = piece {
            let size = [piece.width, piece.depth, piece.height];
            let scale: Vec<f64> = size
                .iter()
                .zip(loaded.size)
                .map(|(s, natural)| (s / natural * 1000.0).round() / 1000.0)
                .collect();
            out["piece"] =
                json!({ "id": piece.id.to_string(), "size": size.map(cm), "scale": scale });
            let (lo, hi) = scale
                .iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
            if hi > lo * 1.02 {
                out["warnings"]
                    .as_array_mut()
                    .expect("list")
                    .push(json!(format!(
                        "stretched unevenly (scale {scale:?} for width, depth, height): thicknesses and curves are distorted"
                    )));
            }
        }
        Ok(out.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::server;

    fn call(s: &NewEraMcp, json: &str) -> Result<Value, ErrorData> {
        s.model(Parameters(serde_json::from_str(json).unwrap()))
            .map(|r| serde_json::from_str(&r).unwrap())
    }

    #[test]
    fn a_model_says_what_it_kept_and_what_it_left_out() {
        let dir = std::env::temp_dir().join(format!("newera-model-tool-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("chair.obj"),
            "mtllib chair.mtl\nv 0 0 0\nv 0.5 0 0\nv 0.5 0.8 0\nv 0 0 0.4\nvt 0 0\nvt 1 0\nvt 1 1\n\
             usemtl tecido\nf 1/1 2/2 3/3\nusemtl madeira\nf 1 3 4\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("chair.mtl"),
            "newmtl tecido\nKd 0.2 0.4 0.6\nmap_Kd trama.png\nnewmtl madeira\nKd 0.5 0.3 0.1\nmap_Bump veio.png\n",
        )
        .unwrap();
        let file = dir.join("chair.obj").display().to_string();
        let s = server();
        let seen = call(&s, &format!(r#"{{"file":"{file}"}}"#)).unwrap();
        assert_eq!(seen["unit"], "m", "{seen}");
        assert_eq!(seen["size"], json!([50.0, 40.0, 80.0]), "{seen}");
        assert_eq!(seen["tris"], 2);
        assert_eq!(
            seen["materials"],
            json!([
                ["tecido", "#336699", "trama.png", 1],
                ["madeira", "#804d1a", null, 1]
            ]),
            "{seen}"
        );
        let warnings = seen["warnings"].to_string();
        assert!(
            warnings.contains("texture trama.png of material tecido not found"),
            "{warnings}"
        );
        assert!(
            warnings.contains("normal/bump map of material madeira is not drawn"),
            "{warnings}"
        );

        // Placed, the reply says it too, and the piece answers by id.
        let reply = s
            .place(Parameters(
                serde_json::from_str(&format!(
                    r#"{{"items":[{{"model":"{file}","at":[100,100],"h":100}}]}}"#
                ))
                .unwrap(),
            ))
            .unwrap();
        let id = reply
            .lines()
            .next()
            .and_then(|l| l.rsplit("ids=").next())
            .unwrap()
            .to_owned();
        assert!(
            reply.contains(&format!("\nimported {id} {file}: 2 warnings")),
            "{reply}"
        );
        let piece = call(&s, &format!(r#"{{"id":"{id}"}}"#)).unwrap();
        assert_eq!(piece["piece"]["scale"], json!([1.0, 1.0, 1.25]), "{piece}");
        assert!(
            piece["warnings"].to_string().contains("stretched unevenly"),
            "{piece}"
        );

        assert!(call(&s, "{}").is_err());
        s.place(Parameters(
            serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[300,300]}]}"#).unwrap(),
        ))
        .unwrap();
        let catalog = call(&s, r#"{"id":"f2"}"#).unwrap_err();
        assert!(
            catalog.message.contains("built from the catalog"),
            "{catalog:?}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
