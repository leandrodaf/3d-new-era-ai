//! Loading user 3D models (OBJ, glTF/GLB) into catalog-style meshes.
//!
//! Imported meshes are normalized to the local frame (`y` up, centered on the
//! floor) so a piece can scale them to its exact width, depth and height.

use std::fmt::Write as _;
use std::path::Path;

use crate::mesh::{Mesh, MeshMaterial, Rgb};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("unsupported model format `{0}` (use .obj, .gltf or .glb)")]
    Format(String),
    #[error("could not read the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not read OBJ: {0}")]
    Obj(#[from] tobj::LoadError),
    #[error("could not read glTF: {0}")]
    Gltf(#[from] gltf::Error),
    #[error("the model has no triangles")]
    Empty,
}

/// A loaded model with its natural size in centimeters.
#[derive(Debug, Clone)]
pub struct ImportedModel {
    pub mesh: Mesh,
    /// Width, depth, height in cm, after unit detection.
    pub size: [f64; 3],
}

const DEFAULT_COLOR: Rgb = [0.78, 0.76, 0.72];

/// Loads a model file and guesses its unit from its size: under 20 units it is
/// taken as meters, over 2000 as millimeters, otherwise centimeters.
pub fn load_model(path: &Path) -> Result<ImportedModel, ImportError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut mesh = match ext.as_str() {
        "obj" => load_obj(path)?,
        "gltf" | "glb" => load_gltf(path)?,
        other => return Err(ImportError::Format(other.to_owned())),
    };
    let (min, max) = mesh.bounds().ok_or(ImportError::Empty)?;
    let raw = [
        f64::from(max[0] - min[0]),
        f64::from(max[2] - min[2]),
        f64::from(max[1] - min[1]),
    ];
    let largest = raw.iter().copied().fold(0.0, f64::max);
    let to_cm = if largest < 20.0 {
        100.0
    } else if largest > 2000.0 {
        0.1
    } else {
        1.0
    };
    let size = raw.map(|v| (v * to_cm).max(1.0));
    mesh.fit_to(size[0], size[1], size[2]);
    Ok(ImportedModel { mesh, size })
}

fn load_obj(path: &Path) -> Result<Mesh, ImportError> {
    let text = std::fs::read(path)?;
    let has_library = text
        .split(|&b| b == b'\n')
        .any(|line| line.trim_ascii_start().starts_with(b"mtllib"));
    // Files without a material library name materials from Wavefront's
    // classic default library; supply it so their colors survive.
    let mut source = Vec::with_capacity(text.len() + 32);
    if !has_library {
        source.extend_from_slice(b"mtllib __defaults__.mtl\n");
    }
    source.extend_from_slice(&text);
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let used: Vec<String> = String::from_utf8_lossy(&text)
        .lines()
        .filter_map(|l| l.trim().strip_prefix("usemtl").map(|n| n.trim().to_owned()))
        .collect();
    let (models, materials) = tobj::load_obj_buf(
        &mut std::io::Cursor::new(source),
        &tobj::GPU_LOAD_OPTIONS,
        |mtl| {
            if mtl == Path::new("__defaults__.mtl") {
                return tobj::load_mtl_buf(&mut std::io::Cursor::new(default_library(&used)));
            }
            let file =
                std::fs::File::open(dir.join(mtl)).map_err(|_| tobj::LoadError::OpenFileFailed)?;
            tobj::load_mtl_buf(&mut std::io::BufReader::new(file))
        },
    )?;
    // Missing or broken material files leave the model gray rather than failing.
    let materials = materials.unwrap_or_default();
    let mut mesh = Mesh {
        materials: materials
            .iter()
            .map(|m| MeshMaterial {
                name: m.name.clone(),
                color: m.diffuse.unwrap_or(DEFAULT_COLOR),
                alpha: m.dissolve.unwrap_or(1.0).clamp(0.0, 1.0),
                texture: m
                    .diffuse_texture
                    .as_ref()
                    .filter(|t| !t.trim().is_empty())
                    .map(|t| dir.join(t.trim())),
                shininess: m.shininess.unwrap_or(0.0),
            })
            .collect(),
        ..Mesh::default()
    };
    for model in models {
        let m = &model.mesh;
        let material = m
            .material_id
            .and_then(|i| u16::try_from(i).ok())
            .filter(|&i| usize::from(i) < mesh.materials.len());
        let color = material.map_or(DEFAULT_COLOR, |i| mesh.materials[usize::from(i)].color);
        let positions: Vec<[f32; 3]> = m
            .positions
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        let normals: Vec<[f32; 3]> = m
            .normals
            .as_chunks::<3>()
            .0
            .iter()
            .map(|n| [n[0], n[1], n[2]])
            .collect();
        let uvs: Vec<[f32; 2]> = m
            .texcoords
            .as_chunks::<2>()
            .0
            .iter()
            .map(|t| [t[0], t[1]])
            .collect();
        let triangles: Vec<[u32; 3]> = m
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [t[0], t[1], t[2]])
            .collect();
        let start = mesh.positions.len();
        append_triangles(&mut mesh, &positions, &normals, &triangles, color);
        for tri in &triangles {
            for &k in tri {
                mesh.uvs
                    .push(uvs.get(k as usize).copied().unwrap_or([0.0, 0.0]));
            }
        }
        mesh.uvs.truncate(mesh.positions.len());
        mesh.vertex_materials
            .resize(mesh.positions.len(), material.unwrap_or(u16::MAX));
        debug_assert!(mesh.vertex_materials.len() >= start);
    }
    if mesh.indices.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(mesh)
}

/// MTL text for material names from the classic Wavefront default library
/// (`white`, `flgrey`, `silver`…), with colors derived from their names.
fn default_library(names: &[String]) -> String {
    let mut out = String::new();
    let mut seen = std::collections::HashSet::new();
    for name in names.iter().filter(|n| seen.insert(n.as_str())) {
        let [r, g, b, alpha] = default_material_color(name);
        let _ = writeln!(out, "newmtl {name}\nKd {r} {g} {b}\nd {alpha}");
    }
    out
}

fn default_material_color(name: &str) -> [f32; 4] {
    let key = name.to_ascii_lowercase();
    let base = key.strip_prefix("fl").unwrap_or(&key);
    let bright = base.ends_with("brt");
    let base = base
        .trim_end_matches("brt")
        .trim_end_matches(|c: char| c.is_ascii_digit());
    let grey = |v: f32| [v, v, v, 1.0];
    let mut color = match base {
        "white" | "archwhite" => grey(0.95),
        "black" => grey(0.04),
        "grey" | "gray" => grey(0.5),
        "ltgrey" | "lightgrey" => grey(0.7),
        "dkgrey" | "darkgrey" => grey(0.28),
        "dkdkgrey" => grey(0.14),
        "silver" | "chrome" => grey(0.76),
        "gold" | "brass" => [0.8, 0.65, 0.3, 1.0],
        "amber" => [0.85, 0.55, 0.12, 1.0],
        "bone" => [0.9, 0.87, 0.78, 1.0],
        "yellow" => [0.9, 0.82, 0.2, 1.0],
        "tan" => [0.78, 0.66, 0.5, 1.0],
        "lighttan" | "lttan" => [0.88, 0.8, 0.66, 1.0],
        "blonde" => [0.86, 0.76, 0.54, 1.0],
        "brown" => [0.45, 0.3, 0.18, 1.0],
        "red" => [0.75, 0.1, 0.08, 1.0],
        "green" => [0.15, 0.55, 0.2, 1.0],
        "blue" => [0.15, 0.3, 0.75, 1.0],
        "orange" => [0.9, 0.5, 0.1, 1.0],
        "glass" | "clear" => [0.8, 0.88, 0.9, 0.3],
        _ => [0.78, 0.76, 0.72, 1.0],
    };
    if bright {
        for c in &mut color[..3] {
            *c = (*c * 1.15).min(1.0);
        }
    }
    color
}

type Mat4 = [[f32; 4]; 4];

fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (c, column) in out.iter_mut().enumerate() {
        for (r, cell) in column.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

fn transform_point(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r])
}

fn transform_vector(m: &Mat4, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[0][r] * v[0] + m[1][r] * v[1] + m[2][r] * v[2])
}

fn load_gltf(path: &Path) -> Result<Mesh, ImportError> {
    let (document, buffers, _images) = gltf::import(path)?;
    let mut mesh = Mesh::default();
    let identity: Mat4 = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next());
    let mut stack: Vec<(gltf::Node<'_>, Mat4)> = scene
        .map(|s| s.nodes().map(|n| (n, identity)).collect())
        .unwrap_or_default();
    while let Some((node, parent)) = stack.pop() {
        let world = mul(&parent, &node.transform().matrix());
        if let Some(node_mesh) = node.mesh() {
            for primitive in node_mesh.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                let Some(positions) = reader.read_positions() else {
                    continue;
                };
                let positions: Vec<[f32; 3]> =
                    positions.map(|p| transform_point(&world, p)).collect();
                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .map(|n| n.map(|v| transform_vector(&world, v)).collect())
                    .unwrap_or_default();
                let indices: Vec<u32> = match reader.read_indices() {
                    Some(indices) => indices.into_u32().collect(),
                    None => (0..u32::try_from(positions.len()).unwrap_or(0)).collect(),
                };
                let triangles: Vec<[u32; 3]> = indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|t| [t[0], t[1], t[2]])
                    .collect();
                let [r, g, b, _] = primitive
                    .material()
                    .pbr_metallic_roughness()
                    .base_color_factor();
                append_triangles(&mut mesh, &positions, &normals, &triangles, [r, g, b]);
            }
        }
        stack.extend(node.children().map(|child| (child, world)));
    }
    if mesh.indices.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(mesh)
}

/// Appends triangles as flat-shaded faces when normals are missing.
fn append_triangles(
    mesh: &mut Mesh,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    triangles: &[[u32; 3]],
    color: Rgb,
) {
    for tri in triangles {
        let Some(corners) = tri
            .iter()
            .map(|i| positions.get(*i as usize).copied())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let face = {
            let (a, b, c) = (corners[0], corners[1], corners[2]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
            n.map(|x| x / len)
        };
        let base = u32::try_from(mesh.positions.len()).expect("mesh fits in u32");
        for (k, corner) in corners.iter().enumerate() {
            mesh.positions.push(*corner);
            let normal = normals.get(tri[k] as usize).copied().unwrap_or(face);
            mesh.normals.push(normal);
            mesh.colors.push(color);
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_an_obj_in_meters_as_centimeters() {
        let dir = std::env::temp_dir().join(format!("newera-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cube.obj");
        // A 2 m × 1 m × 0.5 m box (meters) with CCW faces.
        let v = "v -1 0 -0.25\nv 1 0 -0.25\nv 1 1 -0.25\nv -1 1 -0.25\nv -1 0 0.25\nv 1 0 0.25\nv 1 1 0.25\nv -1 1 0.25\n";
        let f = "f 5 6 7 8\nf 2 1 4 3\nf 6 2 3 7\nf 1 5 8 4\nf 8 7 3 4\nf 1 2 6 5\n";
        std::fs::write(&path, format!("{v}{f}")).unwrap();

        let model = load_model(&path).unwrap();
        assert!(
            model
                .size
                .iter()
                .zip([200.0, 50.0, 100.0])
                .all(|(a, b)| (a - b).abs() < 1e-6),
            "{:?}",
            model.size
        );
        let (min, max) = model.mesh.bounds().unwrap();
        assert!((max[0] - min[0] - 200.0).abs() < 1e-3 && min[1].abs() < 1e-3);
        crate::mesh::tests::assert_outward(&model.mesh);
        assert!(matches!(
            load_model(&dir.join("x.fbx")),
            Err(ImportError::Format(_))
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
