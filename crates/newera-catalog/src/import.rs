//! Loading user 3D models (OBJ, glTF/GLB) into catalog-style meshes.
//!
//! Imported meshes are normalized to the local frame (`y` up, centered on the
//! floor) so a piece can scale them to its exact width, depth and height.

use std::path::Path;

use crate::mesh::{Mesh, Rgb};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("unsupported model format `{0}` (use .obj, .gltf or .glb)")]
    Format(String),
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
    let (models, materials) = tobj::load_obj(path, &tobj::GPU_LOAD_OPTIONS)?;
    let materials = materials.unwrap_or_default();
    let mut mesh = Mesh::default();
    for model in models {
        let m = &model.mesh;
        let color = m
            .material_id
            .and_then(|i| materials.get(i))
            .and_then(|mat| mat.diffuse)
            .unwrap_or(DEFAULT_COLOR);
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
        let triangles: Vec<[u32; 3]> = m
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [t[0], t[1], t[2]])
            .collect();
        append_triangles(&mut mesh, &positions, &normals, &triangles, color);
    }
    if mesh.indices.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(mesh)
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
