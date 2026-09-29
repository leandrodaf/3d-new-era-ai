//! Imported 3D models, looked at from the inside: what a file became once
//! loaded, so a piece that looks wrong can be told apart from a file that
//! is wrong or a feature that is not drawn.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fmt::Write as _;

use super::NewEraMcp;
use super::reply::{core, invalid, ok};

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelParams {
    /// A piece that uses an imported model, e.g. `f12`.
    id: Option<String>,
    /// Instead of `id`: an .obj/.gltf/.glb file, before placing it.
    file: Option<String>,
    /// Only the parts whose name has these words (`braco`), all of them.
    part: Option<String>,
    /// `clashes`: which parts pass through one another, and faces with no area.
    #[schemars(extend("enum" = ["clashes"]))]
    check: Option<String>,
    /// check: how far a surface must pass the other to count, cm (default 0.05); touching is not a clash.
    tol: Option<f64>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditModelParams {
    /// `replace`: put another file in the pieces, keeping each one's id and all the rest.
    #[schemars(extend("enum" = ["replace"]))]
    action: String,
    /// Pieces whose model changes, e.g. `["f12"]`.
    ids: Vec<String>,
    /// replace: the new .obj/.gltf/.glb file.
    file: String,
    /// replace: also every other piece using the same file as `ids` (default: only `ids`).
    #[serde(default)]
    every: bool,
    /// replace: `keep` each piece's w/d/h (default) or take the new file's `natural` size.
    #[schemars(extend("enum" = ["keep", "natural"]))]
    size: Option<String>,
    /// Revision the change was prepared on; refused if the plan has moved on since.
    rev: Option<u64>,
}

/// A short, stable name for the bytes of a file: which version was loaded.
fn fingerprint(path: &std::path::Path) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let bytes = newera_core::vfs::read(path).ok()?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hash);
    Some(format!(
        "{}B #{:08x}",
        bytes.len(),
        hash.finish() & 0xffff_ffff
    ))
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

/// The named parts of a model: `[name, triangles, [x, y, z] min, [x, y, z]
/// max, materials]`, in cm of the piece — x across its width from the
/// center, y from the center to its front, z up from its bottom — scaled
/// by `scale` (the piece's size over the file's).
pub(crate) fn parts(
    mesh: &newera_catalog::Mesh,
    scale: [f64; 3],
    only: Option<&str>,
) -> Vec<Value> {
    let words: Vec<String> = only
        .map(newera_core::fold)
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    mesh.parts
        .iter()
        .filter(|p| {
            let name = newera_core::fold(&p.name);
            words.iter().all(|w| name.contains(w.as_str()))
        })
        .filter_map(|p| {
            let (min, max) = mesh.part_bounds(p)?;
            // Mesh axes: x width, y up, z depth (front +z).
            let at = |v: [f32; 3]| {
                [
                    cm(f64::from(v[0]) * scale[0]),
                    cm(f64::from(v[2]) * scale[1]),
                    cm(f64::from(v[1]) * scale[2]),
                ]
            };
            let mut materials: Vec<&str> = mesh
                .indices
                .get(p.start * 3..(p.start + p.count) * 3)?
                .iter()
                .filter_map(|&i| mesh.material_of(i as usize).map(|m| m.name.as_str()))
                .collect();
            materials.sort_unstable();
            materials.dedup();
            Some(json!([p.name, p.count, at(min), at(max), materials]))
        })
        .collect()
}

/// Answers `check=clashes`: the parts whose surfaces cross, where and how
/// deep, the faces with no area, and what was and was not looked at.
fn check(out: &mut Value, mesh: &newera_catalog::Mesh, scale: [f64; 3], tol: f32) {
    // Stretched to the piece first: an uneven scale changes how deep a crossing is.
    let mut placed = mesh.clone();
    #[allow(clippy::cast_possible_truncation)]
    let [w, d, h] = scale.map(|s| s as f32);
    for v in &mut placed.positions {
        *v = [v[0] * w, v[1] * h, v[2] * d];
    }
    let mesh = &placed;
    let name = |k: usize| mesh.parts.get(k).map_or("(no part)", |p| p.name.as_str());
    let at = |v: [f32; 3]| {
        [
            cm(f64::from(v[0])),
            cm(f64::from(v[2])),
            cm(f64::from(v[1])),
        ]
    };
    let clashes: Vec<Value> = newera_catalog::clashes(mesh, tol)
        .iter()
        .map(|c| {
            json!([
                name(c.a),
                name(c.b),
                c.pairs,
                at(c.at),
                cm(f64::from(c.depth))
            ])
        })
        .collect();
    let degenerate: Vec<Value> = newera_catalog::degenerate(mesh)
        .into_iter()
        .map(|(k, n)| json!([name(k), n]))
        .collect();
    out["clashes"] = json!(clashes);
    out["degenerate"] = json!(degenerate);
    out["checked"] = json!(format!(
        "surfaces of different parts crossing by more than {tol} cm (touching is not a clash), and faces with no area; not checked: a part wholly inside another, clearances, flipped normals, open meshes{}",
        if mesh.parts.len() < 2 {
            " — this file has one part, so there is nothing to cross"
        } else {
            ""
        }
    ));
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
        "parts": parts(mesh, [1.0; 3], None),
        "warnings": r.warnings,
    })
}

#[tool_router(router = model_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Inspect an imported 3D model, a piece's (id) or a file before placing it: the unit taken for its numbers and its natural size, triangles, its materials with their images, its named parts (OBJ objects and groups, glTF nodes: arms, seat, frame) with their bounds and materials, and warnings for what is not drawn as the file says (a texture or material library not found, normal and roughness maps, alpha masks, extensions). For a piece that looks wrong after place model=…, instead of reading the file. part=<words> keeps the parts named so; check=clashes finds parts passing through one another (a cushion through a rail), not those that touch. Reply {file, format, unit, raw (file units), size cm, tris, materials [[name, color, image, tris]], images, uv, parts [[name, tris, min, max, materials]] in cm of the piece (x across, y to the front, z up), warnings}; with id also piece {size, scale per axis}; check adds clashes [[part, part, triangle pairs, at, depth cm]], degenerate [[part, faces]] and checked (what was and was not looked at). place model=… imports one."
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
        let scale = piece.as_ref().map_or([1.0; 3], |piece| {
            [piece.width, piece.depth, piece.height]
                .iter()
                .zip(loaded.size)
                .map(|(s, natural)| s / natural)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or([1.0; 3])
        });
        out["parts"] = json!(parts(&loaded.mesh, scale, p.part.as_deref()));
        match p.check.as_deref() {
            None => {}
            Some("clashes") => {
                #[allow(clippy::cast_possible_truncation)]
                let tol = p.tol.unwrap_or(0.05).max(0.0) as f32;
                check(&mut out, &loaded.mesh, scale, tol);
            }
            Some(other) => return Err(invalid(format!("check `{other}`: clashes"))),
        }
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

#[tool_router(router = edit_model_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Change the model file of placed pieces. replace swaps the file of ids (every=true: of every piece using the same file) in one undo step, keeping each piece's id, position, angle, storey, size (size=natural takes the file's), finish, material overrides, names and links; the new file is read first and nothing changes if it fails. rev refuses the change when the plan moved on since that revision. Reply ok with the ids, then a line per piece: old file → new, the version loaded (bytes, hash), overrides that no longer match a material, and what the new file does not draw. model inspects a file; place imports a new piece."
    )]
    pub(crate) fn edit_model(
        &self,
        Parameters(p): Parameters<EditModelParams>,
    ) -> Result<String, ErrorData> {
        if p.action != "replace" {
            return Err(invalid(format!("action `{}`: replace", p.action)));
        }
        let natural = match p.size.as_deref() {
            None | Some("keep") => false,
            Some("natural") => true,
            Some(other) => return Err(invalid(format!("size `{other}`: keep or natural"))),
        };
        let mut doc = self.document.write();
        if let Some(rev) = p.rev
            && rev != doc.revision()
        {
            return Err(invalid(format!(
                "the plan is at rev {} now, not {rev}: read it again (home ids=…) and decide on what is there",
                doc.revision()
            )));
        }
        let path = doc.resolve_asset(&p.file);
        let loaded = newera_catalog::load_model(&path)
            .map_err(|e| invalid(format!("{}: {e} — nothing was replaced", path.display())))?;
        let home = doc.home();
        let mut targets: Vec<newera_core::Furniture> = Vec::new();
        for raw in &p.ids {
            let id = raw
                .parse::<newera_core::FurnitureId>()
                .map_err(|e| invalid(format!("ids: {e}")))?;
            let piece = home
                .find_piece(id)
                .ok_or_else(|| invalid(format!("{raw} not found")))?;
            if piece.model.is_none() {
                return Err(invalid(format!(
                    "{raw} is built from the catalog (`{}`), not an imported model",
                    piece.catalog
                )));
            }
            if !targets.iter().any(|t| t.id == piece.id) {
                targets.push(piece.clone());
            }
        }
        if targets.is_empty() {
            return Err(invalid("ids: the pieces whose model changes"));
        }
        if p.every {
            let files: Vec<Option<String>> = targets.iter().map(|t| t.model.clone()).collect();
            for piece in home
                .furniture
                .iter()
                .flat_map(newera_core::Furniture::flatten)
            {
                if files.contains(&piece.model) && !targets.iter().any(|t| t.id == piece.id) {
                    targets.push(piece.clone());
                }
            }
        }
        // The materials some face uses: a library can define more.
        let used: std::collections::BTreeSet<u16> =
            loaded.mesh.vertex_materials.iter().copied().collect();
        let names: Vec<&str> = loaded
            .mesh
            .materials
            .iter()
            .enumerate()
            .filter(|(k, _)| u16::try_from(*k).is_ok_and(|k| used.contains(&k)))
            .map(|(_, m)| m.name.as_str())
            .collect();
        let stem = |file: &str| {
            std::path::Path::new(file)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        };
        let version = fingerprint(&path).unwrap_or_default();
        let mut commands = Vec::with_capacity(targets.len());
        let mut lines = Vec::with_capacity(targets.len());
        for mut piece in targets {
            let old = piece.model.replace(p.file.clone()).unwrap_or_default();
            // A name that was only the old file's is the new file's now.
            if stem(&old).as_deref() == Some(piece.name.as_str())
                && let Some(new) = stem(&p.file)
            {
                piece.name = new;
            }
            if natural {
                piece.width = loaded.size[0];
                piece.depth = loaded.size[1];
                piece.height = loaded.size[2];
            }
            let mut line = format!("{}: {old} → {} ({version})", piece.id, p.file);
            let orphans: Vec<&str> = piece
                .materials
                .iter()
                .map(|m| m.name.as_str())
                .filter(|n| !names.contains(n))
                .collect();
            if !orphans.is_empty() {
                let _ = write!(
                    line,
                    "; overrides matching no material now: {}",
                    orphans.join(", ")
                );
            }
            let scale = [piece.width, piece.depth, piece.height]
                .iter()
                .zip(loaded.size)
                .map(|(s, n)| s / n)
                .collect::<Vec<_>>();
            let (lo, hi) = scale
                .iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
            if hi > lo * 1.02 {
                let _ = write!(
                    line,
                    "; the kept size {:?} stretches the file's {:?} unevenly",
                    [piece.width, piece.depth, piece.height].map(cm),
                    loaded.size.map(cm)
                );
            }
            if !loaded.report.warnings.is_empty() {
                let _ = write!(line, "; not drawn: {}", loaded.report.warnings.join("; "));
            }
            lines.push(line);
            commands.push(newera_core::Command::update(piece));
        }
        doc.execute(newera_core::Command::Batch { commands })
            .map_err(core)?;
        let ids: Vec<String> = lines
            .iter()
            .filter_map(|l| l.split(':').next().map(str::to_owned))
            .collect();
        Ok(format!("{}\n{}", ok(&doc, &ids), lines.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::server;

    /// A file's path to write inside JSON: `\\` would escape, `/` works everywhere.
    fn path(p: &std::path::Path) -> String {
        p.display().to_string().replace('\\', "/")
    }

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
        let file = path(&dir.join("chair.obj"));
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

    #[test]
    fn a_new_version_of_a_file_takes_the_place_of_the_old_one_under_the_same_id() {
        let dir = std::env::temp_dir().join(format!("newera-edit-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let obj = |w: f64, material: &str| {
            format!(
                "mtllib m.mtl\nusemtl {material}\nv 0 0 0\nv {w} 0 0\nv {w} 0.8 0\nv 0 0 0.6\nf 1 2 3\nf 1 3 4\n"
            )
        };
        std::fs::write(
            dir.join("m.mtl"),
            "newmtl tecido\nKd 1 0 0\nnewmtl linho\nKd 0 1 0\n",
        )
        .unwrap();
        std::fs::write(dir.join("win-v3.obj"), obj(0.8, "tecido")).unwrap();
        std::fs::write(dir.join("win-v4.obj"), obj(0.7, "linho")).unwrap();
        std::fs::write(dir.join("broken.obj"), "not a model").unwrap();
        let v3 = path(&dir.join("win-v3.obj"));
        let v4 = path(&dir.join("win-v4.obj"));
        let s = server();
        let place = |json: String| {
            s.place(Parameters(serde_json::from_str(&json).unwrap()))
                .unwrap()
        };
        place(format!(
            r#"{{"items":[{{"model":"{v3}","at":[100,100],"facing":"+x"}},{{"model":"{v3}","at":[300,100]}}]}}"#
        ));
        let piece = |id: &str| {
            let doc = s.document.read();
            doc.home().find_piece(id.parse().unwrap()).unwrap().clone()
        };
        // A material override and some metadata, set between versions.
        {
            let mut changed = piece("f1");
            changed.info.brand = Some("Example Furniture".into());
            changed.materials.push(newera_core::ModelMaterial {
                name: "tecido".into(),
                key: None,
                color: Some([0, 0, 255]),
                texture: None,
                shininess: None,
            });
            s.document
                .write()
                .execute(newera_core::Command::update(changed))
                .unwrap();
        }
        let before = piece("f1");
        let rev = s.document.read().revision();
        let edit = |json: String| s.edit_model(Parameters(serde_json::from_str(&json).unwrap()));

        // A file that does not load changes nothing.
        let broken = path(&dir.join("broken.obj"));
        let refused = edit(format!(
            r#"{{"action":"replace","ids":["f1"],"file":"{broken}"}}"#
        ))
        .unwrap_err();
        assert!(
            refused.message.contains("nothing was replaced"),
            "{refused:?}"
        );
        // Nor does one prepared on an older plan.
        let stale = edit(format!(
            r#"{{"action":"replace","ids":["f1"],"file":"{v4}","rev":{}}}"#,
            rev - 1
        ))
        .unwrap_err();
        assert!(stale.message.contains("read it again"), "{stale:?}");
        assert_eq!(piece("f1"), before);

        let reply = edit(format!(
            r#"{{"action":"replace","ids":["f1"],"file":"{v4}","rev":{rev}}}"#
        ))
        .unwrap();
        let after = piece("f1");
        assert_eq!(after.model.as_deref(), Some(v4.as_str()));
        assert_eq!(after.name, "win-v4", "the name was the old file's");
        assert_eq!(
            (
                after.position,
                after.angle,
                after.width,
                after.info.brand.as_deref()
            ),
            (
                before.position,
                before.angle,
                before.width,
                Some("Example Furniture")
            )
        );
        assert!(reply.contains(&format!("f1: {v3} → {v4} (")), "{reply}");
        assert!(
            reply.contains("overrides matching no material now: tecido"),
            "{reply}"
        );
        assert!(reply.contains("stretches the file's"), "{reply}");
        // Only f1: the other copy of v3 stays, and one undo brings v3 back.
        assert_eq!(piece("f2").model.as_deref(), Some(v3.as_str()));
        assert_eq!(s.document.read().home().furniture.len(), 2);
        s.document.write().undo().unwrap();
        assert_eq!(piece("f1"), before);

        // A repeated id is one piece: one change, one line.
        let reply = edit(format!(
            r#"{{"action":"replace","ids":["f1","f1"],"file":"{v4}"}}"#
        ))
        .unwrap();
        assert_eq!(
            reply.matches(&format!("f1: {v3} → {v4}")).count(),
            1,
            "{reply}"
        );
        s.document.write().undo().unwrap();
        assert_eq!(piece("f1"), before);

        // every=true: every piece on the same file, at the new file's size.
        edit(format!(
            r#"{{"action":"replace","ids":["f1"],"file":"{v4}","every":true,"size":"natural"}}"#
        ))
        .unwrap();
        for id in ["f1", "f2"] {
            let p = piece(id);
            assert_eq!(p.model.as_deref(), Some(v4.as_str()));
            assert!((p.width - 70.0).abs() < 1e-3, "{}", p.width);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_arms_and_cushions_answer_by_name_after_the_project_is_reopened() {
        let dir = std::env::temp_dir().join(format!("newera-model-parts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut obj = String::from("mtllib win.mtl\n");
        let mut n = 0;
        for (name, material, x0, x1, z0, z1) in [
            ("braco_esquerdo", "madeira", 0.0, 0.1, 0.0, 0.6),
            ("braco_direito", "madeira", 0.7, 0.8, 0.0, 0.6),
            ("almofada_assento", "tecido", 0.1, 0.7, 0.0, 0.45),
            ("almofada_encosto", "tecido", 0.1, 0.7, 0.45, 0.8),
        ] {
            let _ = write!(
                obj,
                "o {name}\nusemtl {material}\nv {x0} {z0} 0\nv {x1} {z0} 0\nv {x1} {z1} 0.7\nf {} {} {}\n",
                n + 1,
                n + 2,
                n + 3
            );
            n += 3;
        }
        std::fs::write(dir.join("win.obj"), obj).unwrap();
        std::fs::write(
            dir.join("win.mtl"),
            "newmtl madeira\nKd 0.5 0.3 0.1\nnewmtl tecido\nKd 0.2 0.4 0.3\n",
        )
        .unwrap();
        let s = server();
        let file = path(&dir.join("win.obj"));
        s.place(Parameters(
            serde_json::from_str(&format!(r#"{{"items":[{{"model":"{file}","at":[0,0]}}]}}"#))
                .unwrap(),
        ))
        .unwrap();
        let arms = call(&s, r#"{"id":"f1","part":"braco"}"#).unwrap();
        assert_eq!(
            arms["parts"],
            json!([
                [
                    "braco_esquerdo",
                    1,
                    [-40.0, -35.0, 0.0],
                    [-30.0, 35.0, 60.0],
                    ["madeira"]
                ],
                [
                    "braco_direito",
                    1,
                    [30.0, -35.0, 0.0],
                    [40.0, 35.0, 60.0],
                    ["madeira"]
                ]
            ]),
            "{arms}"
        );
        // Saved and opened again, the same names come back.
        let saved = dir.join("win.newera");
        let file_call = |args: Value| crate::call(s.document.clone(), "file", args).unwrap();
        file_call(json!({"action": "save", "path": saved}));
        file_call(json!({"action": "new"}));
        file_call(json!({"action": "open", "path": saved}));
        let back = call(&s, r#"{"id":"f1","part":"almofada"}"#).unwrap();
        let names: Vec<&str> = back["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p[0].as_str().unwrap())
            .collect();
        assert_eq!(names, ["almofada_assento", "almofada_encosto"], "{back}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// OBJ text for axis-aligned boxes, one object each, in cm.
    fn boxes(parts: &[(&str, [f64; 3], [f64; 3])]) -> String {
        let mut out = String::new();
        let mut n = 0;
        for (name, lo, hi) in parts {
            let _ = writeln!(out, "o {name}");
            for k in 0..8 {
                let pick = |axis: usize| {
                    if k >> axis & 1 == 0 {
                        lo[axis]
                    } else {
                        hi[axis]
                    }
                };
                let _ = writeln!(out, "v {} {} {}", pick(0), pick(1), pick(2));
            }
            for face in [
                [0, 2, 3, 1],
                [4, 5, 7, 6],
                [0, 1, 5, 4],
                [2, 6, 7, 3],
                [0, 4, 6, 2],
                [1, 3, 7, 5],
            ] {
                let _ = writeln!(
                    out,
                    "f {} {} {} {}",
                    face[0] + n + 1,
                    face[1] + n + 1,
                    face[2] + n + 1,
                    face[3] + n + 1
                );
            }
            n += 8;
        }
        out
    }

    #[test]
    fn a_cushion_through_a_rail_is_a_clash_and_one_resting_on_it_is_not() {
        let dir = std::env::temp_dir().join(format!("newera-model-clash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Heights up (y), in cm: a 60 × 4 rail, a cushion 3 cm into it, one on top of it.
        std::fs::write(
            dir.join("poltrona.obj"),
            boxes(&[
                ("travessa", [0.0, 40.0, 0.0], [60.0, 44.0, 5.0]),
                ("almofada", [5.0, 41.0, 6.0], [55.0, 50.0, 30.0]),
                ("almofada_apoiada", [5.0, 44.0, -30.0], [55.0, 50.0, 2.0]),
                ("encosto", [0.0, 0.0, 31.0], [60.0, 90.0, 35.0]),
            ])
            .replace("v 5 41 6", "v 5 41 3")
            .replace("v 55 41 6", "v 55 41 3")
            .replace("v 5 50 6", "v 5 50 3")
            .replace("v 55 50 6", "v 55 50 3"),
        )
        .unwrap();
        let file = path(&dir.join("poltrona.obj"));
        let s = server();
        let seen = call(&s, &format!(r#"{{"file":"{file}","check":"clashes"}}"#)).unwrap();
        let clashes = seen["clashes"].as_array().unwrap();
        assert_eq!(clashes.len(), 1, "{seen}");
        assert_eq!(
            (clashes[0][0].as_str(), clashes[0][1].as_str()),
            (Some("travessa"), Some("almofada"))
        );
        assert!(
            seen["checked"].as_str().unwrap().contains("not checked"),
            "{seen}"
        );
        assert_eq!(seen["degenerate"], json!([]));
        // Touching only: a looser tolerance than the 3 cm finds nothing.
        let loose = call(
            &s,
            &format!(r#"{{"file":"{file}","check":"clashes","tol":4}}"#),
        )
        .unwrap();
        assert_eq!(loose["clashes"], json!([]), "{loose}");
        // Placed at twice its size, the crossing is twice as deep, in the piece's cm.
        s.place(Parameters(
            serde_json::from_str(&format!(
                r#"{{"items":[{{"model":"{file}","at":[100,100]}}]}}"#
            ))
            .unwrap(),
        ))
        .unwrap();
        {
            let mut doc = s.document.write();
            let mut piece = doc
                .home()
                .find_piece("f1".parse().unwrap())
                .unwrap()
                .clone();
            piece.width *= 2.0;
            piece.depth *= 2.0;
            piece.height *= 2.0;
            doc.execute(newera_core::Command::update(piece)).unwrap();
        }
        let placed = call(&s, r#"{"id":"f1","check":"clashes"}"#).unwrap();
        let depth = |v: &Value| v["clashes"][0][4].as_f64().unwrap();
        assert!(
            (depth(&placed) - 2.0 * depth(&seen)).abs() < 0.1,
            "{placed}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
