//! Geometry of transformed furniture boxes, shared by spatial checks.

use crate::{Furniture, Point2};

impl Furniture {
    /// Plan projection of the box portion inside a horizontal height band.
    pub fn footprint_in_band(&self, bottom: f64, top: f64) -> Vec<Point2> {
        use geo::ConvexHull;
        if bottom >= top {
            return Vec::new();
        }
        let vertices = world_corners(self);
        let mut points = Vec::new();
        for p in &vertices {
            if p[1] >= bottom && p[1] <= top {
                points.push((p[0], p[2]));
            }
        }
        for i in 0..8 {
            for bit in [1, 2, 4] {
                let j = i ^ bit;
                if j <= i {
                    continue;
                }
                let (a, b) = (vertices[i], vertices[j]);
                for plane in [bottom, top] {
                    if (a[1] < plane && b[1] > plane) || (a[1] > plane && b[1] < plane) {
                        let t = (plane - a[1]) / (b[1] - a[1]);
                        points.push((a[0] + t * (b[0] - a[0]), a[2] + t * (b[2] - a[2])));
                    }
                }
            }
        }
        if points.len() < 3 {
            return Vec::new();
        }
        let hull = geo::MultiPoint::from(points).convex_hull();
        let ring = &hull.exterior().0;
        ring.iter()
            .take(ring.len().saturating_sub(1))
            .map(|p| Point2::new(p.x, p.y))
            .collect()
    }

    /// Exact vertical interval of the transformed box above a plan point, in cm.
    /// Returns `None` outside its true projection, including empty corners of
    /// the enclosing plan rectangle. Handles vertical panels without division
    /// by a vanishing pitch/roll cosine.
    pub fn vertical_range_at(&self, p: Point2) -> Option<(f64, f64)> {
        let (x, z) = self.to_local(p);
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let (sr, cr) = self.roll.to_radians().sin_cos();
        let mut low = f64::NEG_INFINITY;
        let mut high = f64::INFINITY;
        // Inverse rotation of a vertical line through (x, z), relative to
        // the box center. Each original axis constrains the line parameter.
        for (slope, offset, half) in [
            (sr * cp, cr * x + sr * sp * z, self.width / 2.0),
            (cr * cp, -sr * x + cr * sp * z, self.height / 2.0),
            (-sp, cp * z, self.depth / 2.0),
        ] {
            if slope.abs() < 1e-12 {
                if offset.abs() > half + 1e-9 {
                    return None;
                }
            } else {
                let a = (-half - offset) / slope;
                let b = (half - offset) / slope;
                low = low.max(a.min(b));
                high = high.min(a.max(b));
            }
        }
        let center = self.elevation + self.height / 2.0;
        (low <= high && low.is_finite() && high.is_finite())
            .then_some((center + low, center + high))
    }
}

type Vec3 = [f64; 3];

fn world_corners(piece: &Furniture) -> Vec<Vec3> {
    piece
        .tilted_corners()
        .iter()
        .map(|v| {
            let p = piece.to_plan((v[0], v[2]));
            [p.x, piece.elevation + v[1], p.y]
        })
        .collect()
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

struct ConvexSolid {
    vertices: Vec<Vec3>,
    edges: Vec<Vec3>,
    normals: Vec<Vec3>,
}

impl ConvexSolid {
    fn furniture(piece: &Furniture) -> Self {
        let vertices = world_corners(piece);
        let edges: Vec<_> = [4, 2, 1]
            .iter()
            .map(|&i| sub(vertices[i], vertices[0]))
            .collect();
        let normals = vec![
            cross(edges[0], edges[1]),
            cross(edges[1], edges[2]),
            cross(edges[2], edges[0]),
        ];
        Self {
            vertices,
            edges,
            normals,
        }
    }

    // Separating-axis theorem: both face normals and edge cross products are
    // necessary, especially for skew boxes. Mere surface contact is allowed.
    fn overlaps(&self, other: &Self) -> bool {
        let axes = self
            .normals
            .iter()
            .copied()
            .chain(other.normals.iter().copied())
            .chain(
                self.edges
                    .iter()
                    .flat_map(|&a| other.edges.iter().map(move |&b| cross(a, b))),
            );
        for axis in axes {
            let length = dot(axis, axis).sqrt();
            if length < 1e-12 {
                continue;
            }
            let axis = axis.map(|x| x / length);
            let project = |points: &[Vec3]| {
                points
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| {
                        let v = dot(p, axis);
                        (lo.min(v), hi.max(v))
                    })
            };
            let (a, b) = project(&self.vertices);
            let (c, d) = project(&other.vertices);
            if b.min(d) - a.max(c) <= 1e-7 {
                return false;
            }
        }
        true
    }
}

pub(crate) fn boxes_overlap(a: &Furniture, b: &Furniture) -> bool {
    ConvexSolid::furniture(a).overlaps(&ConvexSolid::furniture(b))
}

/// Test the whole box against a polygon extruded from `bottom` to a planar
/// top. Triangulation preserves concave outlines and holes.
pub(crate) fn hits_prism(
    piece: &Furniture,
    polygon: &geo::Polygon<f64>,
    bottom: f64,
    top: impl Fn(Point2) -> f64,
) -> bool {
    use geo::TriangulateEarcut;
    let solid = ConvexSolid::furniture(piece);
    polygon.earcut_triangles().into_iter().any(|triangle| {
        let coords = triangle.to_array();
        let mut vertices: Vec<_> = coords.iter().map(|p| [p.x, bottom, p.y]).collect();
        vertices.extend(
            coords
                .iter()
                .map(|p| [p.x, top(Point2::new(p.x, p.y)).max(bottom), p.y]),
        );
        let edges = [
            (0, 1),
            (1, 2),
            (2, 0),
            (3, 4),
            (4, 5),
            (5, 3),
            (0, 3),
            (1, 4),
            (2, 5),
        ]
        .iter()
        .map(|&(a, b)| sub(vertices[b], vertices[a]))
        .collect();
        let mut normals: Vec<_> = [(0, 1, 2), (3, 4, 5)]
            .iter()
            .map(|&(a, b, c)| cross(sub(vertices[b], vertices[a]), sub(vertices[c], vertices[a])))
            .collect();
        // A side still has a plane when one end meets the bottom (gable toe).
        normals.extend(
            [(0, 1), (1, 2), (2, 0)]
                .iter()
                .map(|&(a, b)| cross(sub(vertices[b], vertices[a]), [0.0, 1.0, 0.0])),
        );
        solid.overlaps(&ConvexSolid {
            vertices,
            edges,
            normals,
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::{Furniture, FurnitureId, Home, Point2};

    #[test]
    fn oriented_boxes_distinguish_overlap_gap_and_surface_contact() {
        for (angle, mirrored) in [(0.0, false), (37.0, true), (90.0, false)] {
            let a = Furniture {
                width: 200.0,
                depth: 80.0,
                height: 10.0,
                elevation: 100.0,
                roll: 45.0,
                angle,
                mirrored,
                ..Default::default()
            };
            let mut b = a.clone();
            b.elevation += 20.0;
            assert!(!super::boxes_overlap(&a, &b), "parallel slabs have a gap");
            b.elevation = a.elevation + 10.0 * 2.0_f64.sqrt();
            assert!(
                !super::boxes_overlap(&a, &b),
                "surface contact is not penetration"
            );
            b.elevation -= 0.1;
            assert!(super::boxes_overlap(&a, &b), "small actual penetration");
            assert!(super::boxes_overlap(&b, &a), "symmetric");
        }
    }

    #[test]
    fn extruded_regions_preserve_concave_voids_and_vertical_clearance() {
        let region = geo::Polygon::new(
            geo::LineString::from(vec![
                (0.0, 0.0),
                (100.0, 0.0),
                (100.0, 50.0),
                (50.0, 50.0),
                (50.0, 100.0),
                (0.0, 100.0),
                (0.0, 0.0),
            ]),
            vec![],
        );
        let mut piece = Furniture {
            width: 20.0,
            depth: 20.0,
            height: 20.0,
            elevation: 40.0,
            position: Point2::new(75.0, 75.0),
            ..Default::default()
        };
        assert!(!super::hits_prism(&piece, &region, 0.0, |_| 90.0));
        piece.position.x = 25.0;
        assert!(super::hits_prism(&piece, &region, 0.0, |_| 90.0));
        piece.elevation = 90.0;
        assert!(!super::hits_prism(&piece, &region, 0.0, |_| 90.0));
        piece.elevation = -20.0;
        assert!(!super::hits_prism(&piece, &region, 0.0, |_| 90.0));
        piece.elevation = 40.0;
        // A sloping top intersects only the high end of the box.
        assert!(super::hits_prism(&piece, &region, 0.0, |p| 2.0 * p.x));
        assert!(!super::hits_prism(&piece, &region, 0.0, |p| p.x));
        let ring = geo::Polygon::new(
            geo::LineString::from(vec![
                (0.0, 0.0),
                (100.0, 0.0),
                (100.0, 100.0),
                (0.0, 100.0),
                (0.0, 0.0),
            ]),
            vec![geo::LineString::from(vec![
                (10.0, 60.0),
                (40.0, 60.0),
                (40.0, 90.0),
                (10.0, 90.0),
                (10.0, 60.0),
            ])],
        );
        assert!(
            !super::hits_prism(&piece, &ring, 0.0, |_| 90.0),
            "hole stays empty"
        );
    }

    #[test]
    fn intervals_cover_side_faces_and_vertical_panels() {
        let mut piece = Furniture {
            width: 100.0,
            depth: 200.0,
            height: 20.0,
            elevation: 300.0,
            pitch: 90.0,
            angle: 37.0,
            mirrored: true,
            ..Default::default()
        };
        let range = piece.vertical_range_at(piece.to_plan((20.0, 5.0))).unwrap();
        assert!((range.0 - 210.0).abs() < 1e-8);
        assert!((range.1 - 410.0).abs() < 1e-8);
        assert!(
            piece
                .vertical_range_at(piece.to_plan((20.0, 11.0)))
                .is_none()
        );
        piece.pitch = 0.0;
        piece.roll = 90.0;
        let range = piece.vertical_range_at(piece.to_plan((5.0, 20.0))).unwrap();
        assert!((range.0 - 260.0).abs() < 1e-8);
        assert!((range.1 - 360.0).abs() < 1e-8);
        assert!(
            piece
                .vertical_range_at(piece.to_plan((11.0, 20.0)))
                .is_none()
        );
        piece.roll = 0.0;
        assert_eq!(
            piece.vertical_range_at(piece.position),
            Some((300.0, 320.0))
        );
        assert!(
            piece
                .vertical_range_at(piece.to_plan((51.0, 0.0)))
                .is_none()
        );
    }

    #[test]
    fn roof_height_uses_exact_lower_surface_and_threshold() {
        let piece = Furniture {
            width: 600.0,
            depth: 600.0,
            height: 40.0,
            elevation: 200.0,
            pitch: 25.0,
            roll: 30.0,
            ..Default::default()
        };
        let expected = 220.0 - 20.0 / (25.0_f64.to_radians().cos() * 30.0_f64.to_radians().cos());
        let mut home = Home::default();
        home.furniture.push(piece);
        let at = Point2::new(0.0, 0.0);
        assert!((crate::roof_height_at(&home, at, 0.0).unwrap() - expected).abs() < 1e-8);
        assert!(crate::roof_height_at(&home, at, expected + 0.01).is_none());
    }

    #[test]
    fn inclined_box_heights_match_inverse_rotation_not_projected_thickness() {
        let piece = Furniture {
            width: 600.0,
            depth: 600.0,
            height: 40.0,
            elevation: 200.0,
            pitch: 25.0,
            roll: 30.0,
            angle: 37.0,
            mirrored: true,
            ..Default::default()
        };
        let pitch = 25.0_f64.to_radians();
        let roll = 30.0_f64.to_radians();
        for (x, z) in [(0.0, 0.0), (30.0, -50.0), (-20.0, 40.0)] {
            let mid = 220.0 + x * roll.tan() / pitch.cos() - z * pitch.tan();
            let half = 20.0 / (pitch.cos() * roll.cos());
            let at = piece.to_plan((x, z));
            assert!(
                (piece.underside_at(at) - (mid - half)).abs() < 1e-6,
                "lower surface"
            );
            assert!(
                (piece.top_at(at) - (mid + half)).abs() < 1e-6,
                "upper surface"
            );
        }
    }

    #[test]
    fn roof_lookup_ignores_empty_corners_of_the_projected_bounding_rectangle() {
        let piece = Furniture {
            id: FurnitureId(1),
            width: 120.0,
            depth: 160.0,
            height: 20.0,
            elevation: 300.0,
            pitch: 30.0,
            roll: 40.0,
            ..Default::default()
        };
        let corner = piece.projected_footprint()[0];
        let at = Point2::new(corner.x + 0.1, corner.y + 0.1);
        let mut home = Home::default();
        home.furniture.push(piece);
        assert!(
            crate::roof_height_at(&home, at, 0.0).is_none(),
            "no box surface exists at that corner"
        );
    }
}
