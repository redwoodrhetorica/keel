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
use keel_math::vec::Vec3;

impl Body {
    /// Hollow this solid to a uniform wall thickness `t`, leaving a fully
    /// enclosed interior void (closed shell, no pierced faces). Returns a
    /// thin-walled solid whose boundary is two nested shells (the original
    /// outer boundary and the inward-offset inner boundary).
    ///
    /// MVP scope: axis-aligned box. The inner shell is the body's bounding
    /// box shrunk by `t` on every side, subtracted via a nested boolean
    /// difference; this is the dossier-50 sec-6.1 base case and the first
    /// consumer of the enclosed-void assembly. Errors if `t` is not
    /// positive or exceeds `t_max = min(extent)/2` (the wall would collapse
    /// onto the medial axis).
    pub fn hollow(&self, t: f64) -> Result<Body, TopoError> {
        if t <= 0.0 || !t.is_finite() {
            return Err(TopoError::Precondition("hollow: thickness must be > 0"));
        }
        let bb = self.bounding_box();
        let ext = bb.max - bb.min;
        // t_max = min(extent)/2 (box medial limit, dossier 50 sec 2.1).
        if ext.x <= 2.0 * t || ext.y <= 2.0 * t || ext.z <= 2.0 * t {
            return Err(TopoError::Precondition(
                "hollow: thickness >= t_max (inner wall collapses)",
            ));
        }
        let inner_min = bb.min + Vec3::new(t, t, t);
        let mut inner = Body::new();
        inner.block(inner_min, ext.x - 2.0 * t, ext.y - 2.0 * t, ext.z - 2.0 * t)?;
        boolean(self, &inner, BoolOp::Difference, 1e-7)
            .map(|r| r.body)
            .map_err(|_| TopoError::Precondition("hollow: shell assembly failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
