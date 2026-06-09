//! Shell / hollow (parity item 41): a thin-walled solid produced by
//! offsetting a solid's boundary inward by a wall thickness `t` and
//! enclosing the interior as a void. Research: dossier 50 (shell = a
//! maximal multi-face tweak; the closed shell encloses a void = a third
//! region). This first increment covers the axis-aligned BOX base case
//! (dossier 50 sec 6.1): the inner shell is the bounding box shrunk by
//! `t`, subtracted as a nested boolean difference, which exercises the
//! enclosed-void (3-region) assembly. General offset-and-reintersect
//! shells (curved faces, pierce, per-face thickness, thicken) are
//! follow-ups built on the same void-assembly foundation.

use crate::Body;
use crate::body::TopoError;
use crate::boolean::{BoolOp, boolean};

impl Body {
    /// Hollow this solid to a uniform wall thickness `t`, leaving a fully
    /// enclosed interior void (closed shell, no pierced faces). Returns a
    /// thin-walled solid whose boundary is two nested shells (the original
    /// outer boundary and the inward-offset inner boundary).
    ///
    /// The inner shell is the body shrunk inward by `t` via the whole-body
    /// face-offset-and-reintersect (`offset_body(-t)`, the Forsyth shell
    /// algorithm of dossier 50 sec 1 for the convex planar case), then
    /// subtracted as a nested boolean difference, whose enclosed-void
    /// assembly yields the two-shell hollow solid. Scope: convex planar
    /// solids (box, prism); `offset_body` declines non-convex / non-planar /
    /// non-simple-vertex bodies, so hollow declines honestly there (curved
    /// and concave shells are a follow-up). Errors if `t <= 0` or `t` is so
    /// large the inner shell collapses past the medial axis (the difference's
    /// mass==mesh post-condition rejects the degenerate inner body).
    pub fn hollow(&self, t: f64) -> Result<Body, TopoError> {
        if t <= 0.0 || !t.is_finite() {
            return Err(TopoError::Precondition("hollow: thickness must be > 0"));
        }
        self.hollow_per_face(|_| t)
    }

    /// Hollow with a per-face wall thickness (parity item 43, multi-thickness
    /// shell). `thickness(face)` gives the inward wall thickness for each of
    /// the body's faces (keyed by the body's own `FaceKey`s; a deep clone
    /// preserves them, so the closure is queried on the matching faces of the
    /// inner copy). The inner shell is the body shrunk per-face by
    /// `offset_body_with` and subtracted; the differently-offset faces meet at
    /// inner edges/steps automatically (dossier 50 sec 3.1). Same convex-
    /// planar scope and honest-decline behaviour as [`hollow`](Self::hollow).
    pub fn hollow_per_face(
        &self,
        thickness: impl Fn(crate::entity::FaceKey) -> f64,
    ) -> Result<Body, TopoError> {
        let mut inner = self.clone();
        inner.offset_body_with(|f| -thickness(f))?;
        boolean(self, &inner, BoolOp::Difference, 1e-7)
            .map(|r| r.body)
            .map_err(|_| {
                TopoError::Precondition("hollow: shell assembly failed (thickness >= t_max?)")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_math::vec::Vec3;

    #[test]
    fn hollow_box_encloses_void() {
        // A 4^3 box hollowed to wall thickness 1 -> inner void is a 2^3
        // box; the wall volume is 64 - 8 = 56. The result is two nested
        // box shells with one enclosed void (dossier 50 sec 6.1).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let h = b.hollow(1.0).unwrap();
        assert!(h.validate().is_ok(), "hollow box invalid");
        let v = h.mass_properties().unwrap().volume;
        let mv = h.mesh_volume();
        assert!(
            (v - 56.0).abs() < 1e-6 && (mv - 56.0).abs() < 1e-6,
            "hollow-box wall volume must be 56 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn hollow_generalizes_to_a_prism() {
        // Hollowing is not box-special: a triangular prism shells too (the
        // inner shell is its inward face-offset, a smaller similar prism).
        // Outer triangle (legs 6,6) area 18, height 4 -> vol 72. Offsetting
        // the 3 side planes inward by t=1 shrinks the triangle's incircle by
        // 1 and the top/bottom inward by 1; the wall is the outer minus the
        // inner prism. Assert only that it ASSEMBLES to a valid two-shell
        // solid with mass == mesh and 0 < wall < outer (the exact inner
        // volume depends on the incircle offset; the invariants are the
        // oracle).
        let mut b = Body::new();
        b.prism(
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(6.0, 0.0, 0.0),
                Vec3::new(0.0, 6.0, 0.0),
            ],
            Vec3::new(0.0, 0.0, 4.0),
        )
        .unwrap();
        let outer = b.mass_properties().unwrap().volume;
        let h = b.hollow(1.0).unwrap();
        assert!(h.validate().is_ok(), "hollow prism invalid");
        let v = h.mass_properties().unwrap().volume;
        let mv = h.mesh_volume();
        assert!(
            (v - mv).abs() < 1e-6 && v > 0.0 && v < outer,
            "hollow prism must be a valid wall with mass == mesh (mass {v}, mesh {mv}, outer {outer})"
        );
    }

    #[test]
    fn hollow_per_face_multi_thickness() {
        // Multi-thickness shell (item 43): a 4^3 box with the top wall at
        // thickness 2 and all other walls at 1. The inner void is then
        // [1,3]x[1,3]x[1,2] = 2*2*1 = 4, so the wall volume is 64 - 4 = 60.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let top = b
            .face_keys()
            .into_iter()
            .find(|&f| b.face_outward_normal(f).map(|n| n.z > 0.9).unwrap_or(false))
            .expect("top face");
        let h = b
            .hollow_per_face(|f| if f == top { 2.0 } else { 1.0 })
            .unwrap();
        assert!(h.validate().is_ok(), "multi-thickness hollow invalid");
        let v = h.mass_properties().unwrap().volume;
        let mv = h.mesh_volume();
        assert!(
            (v - 60.0).abs() < 1e-6 && (mv - 60.0).abs() < 1e-6,
            "multi-thickness wall volume must be 60 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn hollow_declines_when_too_thick() {
        // t past t_max collapses the inner shell; hollow must DECLINE (Err),
        // never return a wrong body. A 2^3 box has t_max = 1.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        assert!(
            b.hollow(1.5).is_err(),
            "over-thick hollow must decline, not return a wrong body"
        );
    }
}
