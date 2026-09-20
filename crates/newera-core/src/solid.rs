//! Geometry of transformed furniture boxes, shared by spatial checks.

use crate::{Furniture, Point2};

impl Furniture {
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

#[cfg(test)]
mod tests {
    use crate::{Furniture, FurnitureId, Home, Point2};

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
