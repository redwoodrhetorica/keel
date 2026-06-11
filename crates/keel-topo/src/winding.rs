//! Generalized winding number (M6b): the robust, surface-type-agnostic
//! point classifier the `d-booleans-tolerant.md` design mandate calls
//! for ("the most robust and most composable classification strategy
//! known"). For an outward-oriented closed boundary `S`,
//! `w(p) = (1/4pi) * sum of signed solid angles` is ~1 inside the
//! solid, ~0 outside, and varies continuously through ~0.5 across the
//! surface (graceful degradation where ray-cast PMC is fragile).

use crate::body::Body;
use keel_math::vec::Vec3;

/// Signed solid angle of triangle (a, b, c) seen from `p`
/// (Van Oosterom-Strackee). Positive when (a, b, c) winds
/// counterclockwise as seen from `p` (i.e. the outward face is toward
/// p's exterior). Zero when p is coplanar within the triangle.
pub fn tri_solid_angle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let (x, y, z) = (a - p, b - p, c - p);
    let (xl, yl, zl) = (x.norm(), y.norm(), z.norm());
    if xl == 0.0 || yl == 0.0 || zl == 0.0 {
        return 0.0;
    }
    let num = x.dot(y.cross(z));
    let den = xl * yl * zl + x.dot(y) * zl + y.dot(z) * xl + z.dot(x) * yl;
    2.0 * num.atan2(den)
}

impl Body {
    /// Is `face` an interior partition wall: a double-sided face whose
    /// BOTH sides bound solid material (the non-regularized boolean's
    /// numReg=2 interface, item 29)? Such a face separates two solid
    /// cells and contributes no net boundary flux: the volume / winding
    /// integrals over the OUTER boundary alone are the body's.
    pub(crate) fn is_interior_wall(&self, face: crate::entity::FaceKey) -> bool {
        let Some(f) = self.faces.get(face) else {
            return false;
        };
        self.regions.get(f.front_region).map(|r| r.solid) == Some(true)
            && self.regions.get(f.back_region).map(|r| r.solid) == Some(true)
    }

    /// Generalized winding number of `p` against this body's boundary:
    /// `(1/4pi) * sum_triangles signed_solid_angle`. ~1 inside, ~0
    /// outside, ~0.5 on the boundary. Surface-type-agnostic (works on
    /// periodic faces with no pcurve dependency). Interior partition
    /// walls (both sides solid) are not part of the OUTER boundary and
    /// are skipped.
    pub fn generalized_winding_number(&self, p: Vec3) -> f64 {
        let _prof = crate::profile::Scope::new(&crate::profile::GWN_NS);
        crate::profile::count(&crate::profile::GWN_CALLS);
        let mut total = 0.0f64;
        for face in self.face_keys() {
            if self.is_interior_wall(face) {
                continue;
            }
            for tri in self.tessellate_face(face) {
                total += tri_solid_angle(p, tri[0], tri[1], tri[2]);
            }
        }
        total / (4.0 * core::f64::consts::PI)
    }

    /// Volume enclosed by the boundary tessellation (signed sum of
    /// tetrahedra from the origin; positive for an outward-oriented
    /// closed surface). Surface-type-agnostic, so it gives a positive
    /// volume for curved results (lenses) where the exact trimmed
    /// mass-properties integral is not yet available; for planar results
    /// it equals the exact volume. Coarse for spheres (the tessellation
    /// is coarse), so it is a guard + approximate oracle, not the exact
    /// mass-properties value.
    pub fn tessellated_volume(&self) -> f64 {
        let mut v = 0.0f64;
        for face in self.face_keys() {
            if self.is_interior_wall(face) {
                continue;
            }
            for tri in self.tessellate_face(face) {
                v += tri[0].dot(tri[1].cross(tri[2])) / 6.0;
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;

    fn box_body(o: Vec3, d: f64) -> Body {
        let mut b = Body::new();
        b.block(o, d, d, d).unwrap();
        b
    }

    fn sphere_body(c: Vec3, r: f64) -> Body {
        let mut b = Body::new();
        b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r)
            .unwrap();
        b
    }

    // ---- Task 0 (MANDATE): winding-number soundness -------------------

    #[test]
    fn gwn_box_inside_is_one_outside_is_zero() {
        // Magnitude ladder of sizes and positions.
        for &(scale, shift) in &[(1.0, 0.0), (10.0, 0.0), (0.5, 50.0), (3.0, -20.0)] {
            let o = Vec3::new(shift, shift, shift);
            let b = box_body(o, scale);
            let inside = o + Vec3::new(scale * 0.5, scale * 0.5, scale * 0.5);
            let outside = o + Vec3::new(scale * 3.0, 0.0, 0.0);
            let wi = b.generalized_winding_number(inside);
            let wo = b.generalized_winding_number(outside);
            assert!((wi - 1.0).abs() < 1e-3, "box inside w={wi} (scale {scale})");
            assert!(wo.abs() < 1e-3, "box outside w={wo} (scale {scale})");
        }
    }

    #[test]
    fn gwn_sphere_inside_is_one_outside_is_zero() {
        for &(r, shift) in &[(1.0, 0.0), (5.0, 0.0), (2.0, 30.0), (0.5, -10.0)] {
            let c = Vec3::new(shift, -shift, shift * 0.5);
            let b = sphere_body(c, r);
            let wi = b.generalized_winding_number(c); // center
            let wo = b.generalized_winding_number(c + Vec3::new(r * 4.0, 0.0, 0.0));
            assert!((wi - 1.0).abs() < 2e-3, "sphere center w={wi} (r {r})");
            assert!(wo.abs() < 2e-3, "sphere outside w={wo} (r {r})");
        }
    }

    fn cylinder_body(c: Vec3, r: f64, h: f64) -> Body {
        let mut b = Body::new();
        b.cylinder(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r, h)
            .unwrap();
        b
    }

    fn nurbs_sphere_body(c: Vec3, r: f64) -> Body {
        let mut b = Body::new();
        b.nurbs_sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r)
            .unwrap();
        b
    }

    #[test]
    fn gwn_nurbs_sphere_inside_is_one_outside_is_zero() {
        // Task 0 (M7a): NURBS winding sound before classifying on it.
        for &(r, shift) in &[(1.0, 0.0), (4.0, 0.0), (2.0, 20.0), (0.5, -12.0)] {
            let c = Vec3::new(shift, -shift * 0.5, shift);
            let b = nurbs_sphere_body(c, r);
            let wi = b.generalized_winding_number(c); // centre
            let wo = b.generalized_winding_number(c + Vec3::new(r * 4.0, 0.0, 0.0));
            assert!(
                (wi - 1.0).abs() < 3e-3,
                "nurbs sphere centre w={wi} (r {r})"
            );
            assert!(wo.abs() < 3e-3, "nurbs sphere outside w={wo} (r {r})");
        }
    }

    #[test]
    fn gwn_nurbs_sphere_orientation_audit() {
        // Outward (local_geometry normal) gives +1 inside, not -1.
        let b = nurbs_sphere_body(Vec3::ZERO, 1.0);
        let w = b.generalized_winding_number(Vec3::new(0.1, 0.0, 0.0));
        assert!(w > 0.9, "nurbs orientation: inside should be +1, got {w}");
    }

    #[test]
    fn gwn_cylinder_inside_is_one_outside_is_zero() {
        // Task 0 (M6c): cylinder winding sound before classifying on it.
        for &(r, h, shift) in &[
            (1.0, 2.0, 0.0),
            (3.0, 8.0, 0.0),
            (0.5, 1.0, 25.0),
            (2.0, 4.0, -15.0),
        ] {
            let c = Vec3::new(shift, shift * 0.5, -shift);
            let b = cylinder_body(c, r, h);
            let inside = c + Vec3::new(0.0, 0.0, h * 0.5); // on the axis, mid-height
            let outside = c + Vec3::new(r * 4.0, 0.0, h * 0.5);
            let wi = b.generalized_winding_number(inside);
            let wo = b.generalized_winding_number(outside);
            assert!((wi - 1.0).abs() < 2e-3, "cyl inside w={wi} (r {r} h {h})");
            assert!(wo.abs() < 2e-3, "cyl outside w={wo} (r {r} h {h})");
        }
    }

    #[test]
    fn gwn_cylinder_orientation_and_graceful() {
        let b = cylinder_body(Vec3::ZERO, 1.0, 2.0); // axis z, [0,2]
        // Inside is +1 (orientation audit).
        let w = b.generalized_winding_number(Vec3::new(0.0, 0.0, 1.0));
        assert!(w > 0.9, "cyl inside winding should be +1, got {w}");
        // Crossing the lateral wall it is finite throughout and passes
        // through the ~0.5 band (the property ray-cast PMC lacks: it
        // would jump 0->1 with no transition). The coarse curved wall is
        // steep, so no per-step continuity bound is asserted.
        let mut crossed = false;
        for k in 0..=80 {
            let x = -2.0 + 4.0 * k as f64 / 80.0;
            let wx = b.generalized_winding_number(Vec3::new(x, 0.0, 1.0));
            assert!(wx.is_finite(), "cyl winding NaN at x={x}");
            assert!(
                (-0.5..=1.5).contains(&wx),
                "cyl winding {wx} blew up at x={x}"
            );
            if (wx - 0.5).abs() < 0.25 {
                crossed = true;
            }
        }
        assert!(crossed, "cyl winding never passed ~0.5 crossing the wall");
    }

    #[test]
    fn gwn_orientation_audit_is_positive_inside() {
        // A consistently outward boundary gives +1 inside, not -1: this
        // cross-checks the M3 face-side conventions (as M4 mass props
        // did) on a fresh, independent computation.
        let b = box_body(Vec3::ZERO, 2.0);
        let w = b.generalized_winding_number(Vec3::new(1.0, 1.0, 1.0));
        assert!(w > 0.9, "orientation: inside winding should be +1, got {w}");
    }

    #[test]
    fn gwn_degrades_gracefully_across_a_face() {
        // Crossing a face, w transitions continuously through ~0.5 with
        // no jumps or NaN (the property ray-cast PMC lacks).
        let b = box_body(Vec3::ZERO, 2.0); // [0,2]^3, face x=2
        let mut prev = b.generalized_winding_number(Vec3::new(-1.0, 1.0, 1.0));
        let mut crossed_half = false;
        for k in 0..=40 {
            let x = -1.0 + 4.0 * k as f64 / 40.0; // -1 .. 3 through x in [0,2]
            let w = b.generalized_winding_number(Vec3::new(x, 1.0, 1.0));
            assert!(w.is_finite(), "w NaN at x={x}");
            assert!((w - prev).abs() < 0.6, "winding jump at x={x}: {prev}->{w}");
            if (w - 0.5).abs() < 0.2 {
                crossed_half = true;
            }
            prev = w;
        }
        assert!(
            crossed_half,
            "winding never passed through ~0.5 crossing the face"
        );
    }

    #[test]
    fn gwn_is_deterministic() {
        let b = sphere_body(Vec3::ZERO, 1.0);
        let p = Vec3::new(0.1, 0.2, 0.3);
        assert_eq!(
            b.generalized_winding_number(p).to_bits(),
            b.generalized_winding_number(p).to_bits()
        );
    }
}
