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
    /// Unit of the file's numbers, `m`, `cm`, `mm` or `in` (default: the piece's, or guessed).
    unit: Option<String>,
}

/// A unit named by a caller, or the one a piece was placed with.
fn unit_of(
    asked: Option<&str>,
    piece: Option<&newera_core::Furniture>,
) -> Result<Option<newera_catalog::Unit>, ErrorData> {
    if let Some(raw) = asked {
        return newera_catalog::Unit::parse(raw).map(Some).ok_or_else(|| {
            invalid(format!(
                "unit `{raw}`: {}",
                newera_catalog::Unit::NAMES.join(", ")
            ))
        });
    }
    Ok(piece
        .and_then(|p| p.properties.get(crate::edit::MODEL_UNIT_KEY))
        .and_then(|u| newera_catalog::Unit::parse(u)))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditModelParams {
    /// `replace` puts another file in the pieces, keeping each one's id and all the rest;
    /// `material` changes one of their materials, `part` one of their parts, by name.
    #[schemars(extend("enum" = ["replace", "material", "part"]))]
    action: String,
    /// Pieces whose model changes, e.g. `["f12"]`.
    ids: Vec<String>,
    /// replace: the new .obj/.gltf/.glb file.
    file: Option<String>,
    /// replace: also every other piece using the same file as `ids` (default: only `ids`).
    #[serde(default)]
    every: bool,
    /// replace: `keep` each piece's w/d/h (default) or take the new file's `natural` size.
    #[schemars(extend("enum" = ["keep", "natural"]))]
    size: Option<String>,
    /// Revision the change was prepared on; refused if the plan has moved on since.
    rev: Option<u64>,
    /// replace: unit of the new file's numbers, `m`, `cm`, `mm` or `in` (default: guessed).
    unit: Option<String>,
    /// material: which one, by the name `model` lists (`tecido`).
    material: Option<String>,
    /// material: its color `[r,g,b]`.
    color: Option<[u8; 3]>,
    /// material: an image over it, `img:trama.png`, or `none` for the file's own.
    mat: Option<String>,
    /// material: times its image repeats against the file's mapping (2 draws it at half size).
    repeat: Option<f64>,
    /// material/part: back to the file's own.
    #[serde(default)]
    clear: bool,
    /// part: which one, by the name `model` lists (`almofada_encosto`).
    part: Option<String>,
    /// part: hide it (true) or show it again (false).
    hide: Option<bool>,
    /// part: move it by `[x across, y to the front, z up]` cm from where the file has it.
    offset: Option<[f64; 3]>,
    /// part: resize it about its center, `[x, y, z]` factors.
    scale: Option<[f64; 3]>,
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
/// max, materials, "hidden"?]`, in cm of the mesh — for a piece, fitted to
/// it: x across its width from the center, y from the center to its front,
/// z up from its bottom.
pub(crate) fn parts(
    mesh: &newera_catalog::Mesh,
    hidden: &[String],
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
                    cm(f64::from(v[0])),
                    cm(f64::from(v[2])),
                    cm(f64::from(v[1])),
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
            let mut row = json!([p.name, p.count, at(min), at(max), materials]);
            if hidden.contains(&p.name) {
                row.as_array_mut().expect("row").push(json!("hidden"));
            }
            Some(row)
        })
        .collect()
}

/// Answers `check=clashes`: the parts whose surfaces cross, where and how
/// deep, the faces with no area, and what was and was not looked at.
fn check(out: &mut Value, mesh: &newera_catalog::Mesh, tol: f32) {
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
        "guessed": r.guessed,
        "raw": r.raw_size.map(|v| (v * 1000.0).round() / 1000.0),
        "size": loaded.size.map(cm),
        "tris": mesh.indices.len() / 3,
        "meshes": r.meshes,
        "materials": materials,
        "images": images.len(),
        "uv": mesh.uvs.iter().any(|uv| *uv != [0.0, 0.0]),
        "parts": parts(mesh, &[], None),
        "warnings": r.warnings,
    })
}

#[tool_router(router = model_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        description = "Inspect an imported 3D model, a piece's (id) or a file before placing it: the unit taken for its numbers (guessed from the size unless unit= or the piece says) and its natural size, triangles, its materials with their images, its named parts (OBJ objects and groups, glTF nodes: arms, seat, frame) with their bounds and materials, and warnings for what is not drawn as the file says (a texture or material library not found, normal and roughness maps, alpha masks, extensions). For a piece that looks wrong after place model=…, instead of reading the file. part=<words> keeps the parts named so; check=clashes finds parts passing through one another (a cushion through a rail), not those that touch. Reply {file, format, unit, raw (file units), size cm, tris, materials [[name, color, image, tris]], images, uv, parts [[name, tris, min, max, materials]] in cm of the piece (x across, y to the front, z up), warnings}; with id also piece {size, scale per axis}; check adds clashes [[part, part, triangle pairs, at, depth cm]], degenerate [[part, faces]] and checked (what was and was not looked at). place model=… imports one."
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
        let unit = unit_of(p.unit.as_deref(), piece.as_ref())?;
        let loaded = newera_catalog::load_model_in(&path, unit)
            .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        let mut out = report(&file, &loaded);
        // A piece's model as it is drawn: turned, fitted to its box, its
        // parts moved and resized; hidden ones listed, but not checked.
        let (listed, drawn, hidden) = match &piece {
            Some(piece) => {
                let mut fitted = loaded.mesh.clone();
                fitted.rotate(piece.model_transform.rotation);
                fitted.fit_to(piece.width, piece.depth, piece.height);
                let mut listed = fitted.clone();
                listed.edit_parts(&piece.model_parts, false);
                fitted.edit_parts(&piece.model_parts, true);
                let hidden: Vec<String> = piece
                    .model_parts
                    .iter()
                    .filter(|e| e.hidden)
                    .map(|e| e.name.clone())
                    .collect();
                (listed, fitted, hidden)
            }
            None => (loaded.mesh.clone(), loaded.mesh.clone(), Vec::new()),
        };
        out["parts"] = json!(parts(&listed, &hidden, p.part.as_deref()));
        match p.check.as_deref() {
            None => {}
            Some("clashes") => {
                #[allow(clippy::cast_possible_truncation)]
                let tol = p.tol.unwrap_or(0.05).max(0.0) as f32;
                check(&mut out, &drawn, tol);
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
        description = "Change the model of placed pieces. replace swaps the file of ids (every=true: of every piece using the same file) in one undo step, keeping each piece's id, position, angle, storey, size (size=natural takes the file's), finish, material overrides, names and links; the new file is read first and nothing changes if it fails. rev refuses the change when the plan moved on since that revision. Reply ok with the ids, then a line per piece: old file → new, the version loaded (bytes, hash), overrides that no longer match a material, and what the new file leaves out. part hides, moves (offset cm: x across, y to the front, z up) or resizes one of their parts by the name model lists — the back cushion alone, the arms unchanged. material changes one of their materials by the name model lists — color, mat (an image over it), repeat, its texture scale (2 halves the image, the piece unchanged), clear. model inspects a file; place imports a new piece."
    )]
    pub(crate) fn edit_model(
        &self,
        Parameters(p): Parameters<EditModelParams>,
    ) -> Result<String, ErrorData> {
        match p.action.as_str() {
            "replace" => self.replace_model(&p),
            "material" => self.model_material(p),
            "part" => self.model_part(&p),
            other => Err(invalid(format!(
                "action `{other}`: replace, material or part"
            ))),
        }
    }
}

/// The pieces `ids` names, each built from a model file.
fn model_pieces(
    doc: &newera_core::Document,
    ids: &[String],
) -> Result<Vec<newera_core::Furniture>, ErrorData> {
    let mut out = Vec::new();
    for raw in ids {
        let id = raw
            .parse::<newera_core::FurnitureId>()
            .map_err(|e| invalid(format!("ids: {e}")))?;
        let piece = doc
            .home()
            .find_piece(id)
            .ok_or_else(|| invalid(format!("{raw} not found")))?;
        if piece.model.is_none() {
            return Err(invalid(format!(
                "{raw} is built from the catalog (`{}`), not an imported model",
                piece.catalog
            )));
        }
        out.push(piece.clone());
    }
    if out.is_empty() {
        return Err(invalid("ids: the pieces whose model changes"));
    }
    Ok(out)
}

/// Refuses a change prepared on another revision of the plan.
fn at_revision(doc: &newera_core::Document, rev: Option<u64>) -> Result<(), ErrorData> {
    match rev {
        Some(rev) if rev != doc.revision() => Err(invalid(format!(
            "the plan is at rev {} now, not {rev}: read it again (home ids=…) and decide on what is there",
            doc.revision()
        ))),
        _ => Ok(()),
    }
}

impl NewEraMcp {
    fn model_part(&self, p: &EditModelParams) -> Result<String, ErrorData> {
        let name = p
            .part
            .clone()
            .ok_or_else(|| invalid("part: which one, by the name `model` lists"))?;
        if !p.clear && p.hide.is_none() && p.offset.is_none() && p.scale.is_none() {
            return Err(invalid("part: give hide, offset, scale or clear"));
        }
        if p.scale
            .is_some_and(|s| s.iter().any(|v| !(v.is_finite() && *v > 0.0)))
        {
            return Err(invalid("scale: three numbers above 0"));
        }
        let mut doc = self.document.write();
        at_revision(&doc, p.rev)?;
        let pieces = model_pieces(&doc, &p.ids)?;
        let mut commands = Vec::with_capacity(pieces.len());
        for mut piece in pieces {
            let file = piece.model.clone().unwrap_or_default();
            let path = doc.resolve_asset(&file);
            let unit = unit_of(None, Some(&piece))?;
            let loaded = newera_catalog::load_model_in(&path, unit)
                .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
            if !loaded.mesh.parts.iter().any(|q| q.name == name) {
                let names: Vec<&str> = loaded.mesh.parts.iter().map(|q| q.name.as_str()).collect();
                return Err(invalid(format!(
                    "{}: no part `{name}` in {file} ({})",
                    piece.id,
                    if names.is_empty() {
                        "it has no named parts".to_owned()
                    } else {
                        names.join(", ")
                    }
                )));
            }
            let slot = piece.model_parts.iter().position(|q| q.name == name);
            if p.clear {
                if let Some(k) = slot {
                    piece.model_parts.remove(k);
                }
            } else {
                let k = slot.unwrap_or_else(|| {
                    piece.model_parts.push(newera_core::ModelPart {
                        name: name.clone(),
                        ..newera_core::ModelPart::default()
                    });
                    piece.model_parts.len() - 1
                });
                let edit = &mut piece.model_parts[k];
                if let Some(hide) = p.hide {
                    edit.hidden = hide;
                }
                if let Some(offset) = p.offset {
                    edit.offset = offset;
                }
                if let Some(scale) = p.scale {
                    edit.scale = scale
                        .iter()
                        .any(|v| (v - 1.0).abs() > 1e-9)
                        .then_some(scale);
                }
                if *edit
                    == (newera_core::ModelPart {
                        name: name.clone(),
                        ..Default::default()
                    })
                {
                    piece.model_parts.remove(k);
                }
            }
            commands.push(newera_core::Command::update(piece));
        }
        doc.execute(newera_core::Command::Batch { commands })
            .map_err(core)?;
        Ok(ok(&doc, &p.ids))
    }

    fn model_material(&self, p: EditModelParams) -> Result<String, ErrorData> {
        let name = p
            .material
            .ok_or_else(|| invalid("material: which one, by the name `model` lists"))?;
        if !p.clear && p.color.is_none() && p.mat.is_none() && p.repeat.is_none() {
            return Err(invalid("material: give color, mat, repeat or clear"));
        }
        if p.repeat.is_some_and(|r| !(r.is_finite() && r > 0.0)) {
            return Err(invalid("repeat: a number above 0"));
        }
        let texture = match p.mat.as_deref() {
            None => None,
            Some(raw) => {
                let m = crate::edit::material(raw).map_err(invalid)?;
                if m.as_ref().is_some_and(|m| m.image.is_none()) {
                    return Err(invalid(
                        "mat: an image (`img:file 60x60`) or none; a plain color is `color`",
                    ));
                }
                Some(m)
            }
        };
        let mut doc = self.document.write();
        at_revision(&doc, p.rev)?;
        let pieces = model_pieces(&doc, &p.ids)?;
        let mut commands = Vec::with_capacity(pieces.len());
        for mut piece in pieces {
            let file = piece.model.clone().unwrap_or_default();
            let path = doc.resolve_asset(&file);
            let loaded = newera_catalog::load_model(&path)
                .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
            if !loaded.mesh.materials.iter().any(|m| m.name == name) {
                let names: Vec<&str> = loaded
                    .mesh
                    .materials
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect();
                return Err(invalid(format!(
                    "{}: no material `{name}` in {file} ({})",
                    piece.id,
                    names.join(", ")
                )));
            }
            let slot = piece.materials.iter().position(|m| m.name == name);
            if p.clear {
                if let Some(k) = slot {
                    piece.materials.remove(k);
                }
            } else {
                let k = slot.unwrap_or_else(|| {
                    piece.materials.push(newera_core::ModelMaterial {
                        name: name.clone(),
                        key: None,
                        color: None,
                        texture: None,
                        shininess: None,
                        repeat: None,
                    });
                    piece.materials.len() - 1
                });
                let m = &mut piece.materials[k];
                if let Some(c) = p.color {
                    m.color = Some(c);
                }
                if let Some(t) = texture.clone() {
                    m.texture = t;
                }
                if let Some(r) = p.repeat {
                    m.repeat = ((r - 1.0).abs() > 1e-9).then_some(r);
                }
            }
            commands.push(newera_core::Command::update(piece));
        }
        let ids = p.ids.clone();
        doc.execute(newera_core::Command::Batch { commands })
            .map_err(core)?;
        Ok(ok(&doc, &ids))
    }

    fn replace_model(&self, p: &EditModelParams) -> Result<String, ErrorData> {
        let file = p
            .file
            .clone()
            .ok_or_else(|| invalid("replace: file, the new .obj/.gltf/.glb"))?;
        let natural = match p.size.as_deref() {
            None | Some("keep") => false,
            Some("natural") => true,
            Some(other) => return Err(invalid(format!("size `{other}`: keep or natural"))),
        };
        let mut doc = self.document.write();
        at_revision(&doc, p.rev)?;
        let path = doc.resolve_asset(&file);
        let unit = unit_of(p.unit.as_deref(), None)?;
        let loaded = newera_catalog::load_model_in(&path, unit)
            .map_err(|e| invalid(format!("{}: {e} — nothing was replaced", path.display())))?;
        let mut targets = model_pieces(&doc, &p.ids)?;
        let home = doc.home();
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
            let old = piece.model.replace(file.clone()).unwrap_or_default();
            // A name that was only the old file's is the new file's now.
            if stem(&old).as_deref() == Some(piece.name.as_str())
                && let Some(new) = stem(&file)
            {
                piece.name = new;
            }
            // The unit belongs to the file: a new file keeps none it was not given.
            match unit {
                Some(u) => {
                    piece
                        .properties
                        .insert(crate::edit::MODEL_UNIT_KEY.into(), u.name().into());
                }
                None => {
                    piece.properties.remove(crate::edit::MODEL_UNIT_KEY);
                }
            }
            if natural {
                piece.width = loaded.size[0];
                piece.depth = loaded.size[1];
                piece.height = loaded.size[2];
            }
            let mut line = format!("{}: {old} → {file} ({version})", piece.id);
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
                    r#"{{"items":[{{"model":"{file}","at":[100,100],"h":100,"stretch":true}}]}}"#
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
            reply.contains(&format!("\nimported {id} {file}: 3 warnings"))
                && reply.contains("stretched ×[1.0, 1.0, 1.25]"),
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
                repeat: None,
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
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_small_part_drawn_in_centimeters_comes_in_at_its_size_when_the_unit_is_said() {
        let dir = std::env::temp_dir().join(format!("newera-model-unit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A knob 5 × 4 × 3 cm, its numbers in centimeters.
        std::fs::write(
            dir.join("puxador.obj"),
            boxes(&[("puxador", [0.0, 0.0, 0.0], [5.0, 3.0, 4.0])]),
        )
        .unwrap();
        let file = path(&dir.join("puxador.obj"));
        let s = server();
        let place = |item: &str| {
            s.place(Parameters(
                serde_json::from_str(&format!(
                    r#"{{"items":[{{"model":"{file}","at":[0,0]{item}}}]}}"#
                ))
                .unwrap(),
            ))
        };
        let size = |id: &str| {
            let doc = s.document.read();
            let p = doc.home().find_piece(id.parse().unwrap()).unwrap();
            // Whole centimeters: every size here is one.
            #[allow(clippy::cast_possible_truncation)]
            [p.width, p.depth, p.height].map(|v| v.round() as i64)
        };
        // Guessed, it is 5 m across, and the reply says so.
        let guessed = place("").unwrap();
        assert_eq!(size("f1"), [500, 400, 300]);
        assert!(
            guessed.contains("unit guessed as m, so it is 500 cm across"),
            "{guessed}"
        );
        // Said, it is 5 cm, and `model` reads the piece in the same unit.
        let said = place(r#","unit":"cm""#).unwrap();
        assert_eq!(size("f2"), [5, 4, 3]);
        assert!(!said.contains("imported"), "{said}");
        let seen = call(&s, r#"{"id":"f2"}"#).unwrap();
        assert_eq!(
            (seen["unit"].as_str(), seen["guessed"].as_bool()),
            (Some("cm"), Some(false))
        );
        assert_eq!(seen["piece"]["scale"], json!([1.0, 1.0, 1.0]), "{seen}");
        // One size scales it evenly; two that disagree are refused; stretch lets them.
        place(r#","unit":"cm","h":6"#).unwrap();
        assert_eq!(size("f3"), [10, 8, 6]);
        let refused = place(r#","unit":"cm","w":10,"h":3"#).unwrap_err();
        assert!(refused.message.contains("stretch=true"), "{refused:?}");
        let stretched = place(r#","unit":"cm","w":10,"h":3,"stretch":true"#).unwrap();
        assert!(
            stretched.contains("stretched ×[2.0, 1.0, 1.0]"),
            "{stretched}"
        );
        let catalog = s
            .place(Parameters(
                serde_json::from_str(r#"{"items":[{"cat":"sofa-3","at":[0,0],"unit":"cm"}]}"#)
                    .unwrap(),
            ))
            .unwrap_err();
        assert!(catalog.message.contains("for model="), "{catalog:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_material_changes_by_name_and_its_weave_by_repeat() {
        let dir =
            std::env::temp_dir().join(format!("newera-model-material-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("win.obj"),
            "mtllib win.mtl\nusemtl tecido\nv 0 0 0\nv 0.6 0 0\nv 0.6 0.8 0.5\nf 1 2 3\n",
        )
        .unwrap();
        std::fs::write(dir.join("win.mtl"), "newmtl tecido\nKd 0.2 0.4 0.3\n").unwrap();
        let file = path(&dir.join("win.obj"));
        let s = server();
        s.place(Parameters(
            serde_json::from_str(&format!(r#"{{"items":[{{"model":"{file}","at":[0,0]}}]}}"#))
                .unwrap(),
        ))
        .unwrap();
        let edit = |json: &str| s.edit_model(Parameters(serde_json::from_str(json).unwrap()));
        let overrides = || {
            let doc = s.document.read();
            doc.home()
                .find_piece("f1".parse().unwrap())
                .unwrap()
                .materials
                .clone()
        };
        let size = || {
            let doc = s.document.read();
            let p = doc.home().find_piece("f1".parse().unwrap()).unwrap();
            format!("{:.1} {:.1} {:.1}", p.width, p.depth, p.height)
        };
        let before = size();
        edit(r#"{"action":"material","ids":["f1"],"material":"tecido","color":[20,60,40],"repeat":2}"#).unwrap();
        let set = overrides();
        assert_eq!(
            (set.len(), set[0].color, set[0].repeat),
            (1, Some([20, 60, 40]), Some(2.0))
        );
        assert_eq!(size(), before, "the weave changes, the chair does not");
        edit(r#"{"action":"material","ids":["f1"],"material":"tecido","mat":"img:trama.png 5x5"}"#)
            .unwrap();
        assert!(
            overrides()[0]
                .texture
                .as_ref()
                .is_some_and(|t| t.image.as_deref() == Some("trama.png"))
        );
        assert_eq!(
            overrides()[0].color,
            Some([20, 60, 40]),
            "what was not given stays"
        );
        let unknown =
            edit(r#"{"action":"material","ids":["f1"],"material":"couro","color":[0,0,0]}"#)
                .unwrap_err();
        assert!(
            unknown.message.contains("no material `couro`") && unknown.message.contains("tecido"),
            "{unknown:?}"
        );
        let pattern =
            edit(r##"{"action":"material","ids":["f1"],"material":"tecido","mat":"#ff0000"}"##)
                .unwrap_err();
        assert!(pattern.message.contains("an image"), "{pattern:?}");
        edit(r#"{"action":"material","ids":["f1"],"material":"tecido","clear":true}"#).unwrap();
        assert!(overrides().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_back_cushion_moves_back_and_the_arms_stay_where_they_are() {
        let dir = std::env::temp_dir().join(format!("newera-model-part-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // x across, y up, z toward the front, cm: arms, seat, back cushion.
        std::fs::write(
            dir.join("win.obj"),
            boxes(&[
                ("braco_esquerdo", [0.0, 0.0, 0.0], [10.0, 60.0, 80.0]),
                ("braco_direito", [70.0, 0.0, 0.0], [80.0, 60.0, 80.0]),
                ("assento", [10.0, 0.0, 10.0], [70.0, 45.0, 80.0]),
                ("almofada_encosto", [10.0, 45.0, 10.0], [70.0, 85.0, 25.0]),
            ]),
        )
        .unwrap();
        let file = dir.join("win.obj").display().to_string();
        let s = server();
        s.place(Parameters(
            serde_json::from_str(&format!(
                r#"{{"items":[{{"model":"{file}","at":[0,0],"unit":"cm"}}]}}"#
            ))
            .unwrap(),
        ))
        .unwrap();
        let edit = |json: &str| s.edit_model(Parameters(serde_json::from_str(json).unwrap()));
        let part = |name: &str| {
            let seen = call(&s, &format!(r#"{{"id":"f1","part":"{name}"}}"#)).unwrap();
            seen["parts"][0].clone()
        };
        let arm = part("braco_direito");
        let back = part("almofada_encosto");
        edit(r#"{"action":"part","ids":["f1"],"part":"almofada_encosto","offset":[0,-5,0]}"#)
            .unwrap();
        let moved = part("almofada_encosto");
        let front = |row: &Value| row[2][1].as_f64().unwrap();
        assert!(
            (front(&moved) - (front(&back) - 5.0)).abs() < 1e-6,
            "{moved}"
        );
        assert_eq!(part("braco_direito"), arm, "the arms stay");
        // Seat and arm heights read on their own, whatever the total height.
        assert_eq!(part("assento")[3][2], json!(45.0));
        assert_eq!(arm[3][2], json!(60.0));

        edit(r#"{"action":"part","ids":["f1"],"part":"braco_direito","hide":true}"#).unwrap();
        assert_eq!(part("braco_direito")[5], json!("hidden"));
        let unknown =
            edit(r#"{"action":"part","ids":["f1"],"part":"pe","hide":true}"#).unwrap_err();
        assert!(
            unknown.message.contains("no part `pe`") && unknown.message.contains("assento"),
            "{unknown:?}"
        );
        edit(r#"{"action":"part","ids":["f1"],"part":"almofada_encosto","clear":true}"#).unwrap();
        edit(r#"{"action":"part","ids":["f1"],"part":"braco_direito","hide":false}"#).unwrap();
        assert_eq!(part("almofada_encosto"), back);
        let doc = s.document.read();
        assert!(
            doc.home()
                .find_piece("f1".parse().unwrap())
                .unwrap()
                .model_parts
                .is_empty()
        );
        drop(doc);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
