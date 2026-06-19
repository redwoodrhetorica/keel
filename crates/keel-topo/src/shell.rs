//! Shell / hollow (parity item 41): a thin-walled solid produced by
//! offsetting a solid's boundary inward by a wall thickness `t` and
//! enclosing the interior as a void. Research: dossier 50 (shell = a
//! maximal multi-face tweak; the closed shell encloses a void = a third
//! region). The inner shell is the whole-body inward offset
//! (`offset_body(-t)`), enclosed as a void by `combine_containment` (the
//! exact nested two-shell assembly; the inward offset guarantees
//! containment, so no re-intersection is needed) -- with a boolean
//! difference fallback. CURVED faces are supported: a box carrying
//! cylindrical FILLET faces shells (the extrude->fillet->shell workflow),
//! because `offset_body_with` offsets plane/cylinder/sphere/torus faces
//! and re-solves the plane/cylinder corners (see `tweak.rs`). Pierce,
//! per-face thickness, and thicken ride the same void-assembly
//! foundation; concave and higher-valence-corner shells are follow-ups.

use crate::Body;
use crate::body::TopoError;
use crate::boolean::{BoolOp, boolean};
use keel_math::vec::Vec3;

/// Self-consistency gate for a directly-assembled shell: the analytic
/// mass must agree with the independent render mesh. An ALL-PLANAR shell
/// tessellates exactly (tight 1e-6 absolute / 1e-9 relative band); any
/// curved face carries chordal mesh deficit, so the band loosens to 1%
/// of the volume (the documented tessellation note -- still far tighter
/// than the soak's 25% curved-WRONG criterion, and the wall must also be
/// positive). A mismatch means the containment assembly produced a
/// non-watertight or mis-oriented body, so the caller declines it.
fn shell_mass_matches_mesh(b: &Body) -> bool {
    let mass = match b.mass_properties() {
        Ok(m) => m.volume,
        Err(_) => return false,
    };
    let mesh = b.mesh_volume();
    if !(mass.is_finite() && mesh.is_finite() && mass > 0.0 && mesh > 0.0) {
        return false;
    }
    let curved = b.face_keys().iter().any(|&f| {
        !matches!(
            b.face_surface3(f),
            Some(keel_geom::surface::Surface3::Plane(_))
        )
    });
    let band = if curved { 1e-2 } else { 1e-6_f64.max(1e-9 * mass) };
    (mass - mesh).abs() <= band * mass.max(mesh).max(1.0)
}

impl Body {
    /// Hollow this solid to a uniform wall thickness `t`, leaving a fully
    /// enclosed interior void (closed shell, no pierced faces). Returns a
    /// thin-walled solid whose boundary is two nested shells (the original
    /// outer boundary and the inward-offset inner boundary).
    ///
    /// The inner shell is the body shrunk inward by `t` via the whole-body
    /// face-offset-and-reintersect (`offset_body(-t)`, the Forsyth shell
    /// algorithm of dossier 50 sec 1), then enclosed as a void: the two
    /// nested shells are stitched verbatim by `combine_containment` (the
    /// inward offset guarantees the inner shell is strictly inside, so no
    /// re-intersection is needed), falling back to a boolean difference if
    /// that precondition or the mass==mesh gate is not met.
    ///
    /// Scope: convex solids whose faces are planes OR cylinders/spheres/tori
    /// with 3-valent corners -- so a box carrying cylindrical FILLET faces
    /// shells (the extrude->fillet->shell workflow), not just an all-planar
    /// box/prism. `offset_body_with` declines faces it cannot offset (cones,
    /// NURBS), non-3-valent corners, and corners whose offset surfaces do not
    /// meet, so hollow declines honestly there (concave and higher-valence
    /// shells remain a follow-up). Errors if `t <= 0` or `t` is so large the
    /// inner shell collapses past the medial axis (the containment
    /// precondition + mass==mesh post-condition reject the degenerate inner
    /// body).
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
    /// `offset_body_with` and enclosed as a void; the differently-offset faces
    /// meet at inner edges/steps automatically (dossier 50 sec 3.1). Same scope
    /// (planar + cylinder/sphere/torus faces, 3-valent corners) and honest-
    /// decline behaviour as [`hollow`](Self::hollow).
    pub fn hollow_per_face(
        &self,
        thickness: impl Fn(crate::entity::FaceKey) -> f64,
    ) -> Result<Body, TopoError> {
        let mut inner = self.clone();
        inner.offset_body_with(|f| -thickness(f))?;
        // CONTAINMENT assembly first (the curved-shell path): an INWARD
        // offset by a thickness under the medial-axis limit leaves `inner`
        // strictly inside `self` with no boundary crossings, so the hollow
        // is the two nested shells stitched verbatim -- no SSI/seam. This is
        // what lets a box carrying cylindrical FILLET faces shell (the
        // extrude->fillet->hollow workflow), where the nested-curved SSI
        // difference declines with an unassemblable seam.
        //
        // PRECONDITION GATE (DECLINE-never-WRONG): the containment shortcut
        // is only valid when `inner` is a non-degenerate solid STRICTLY
        // INSIDE `self`. An over-thick offset collapses the inner shell past
        // the medial axis -- its offset faces cross, inverting the body
        // (mesh volume <= 0) -- which would otherwise stitch into a wrong
        // "shell". Reject any inner that is not a positive solid smaller
        // than the outer, and additionally gate the assembled body on
        // validate() + mass==mesh. On rejection, fall back to the exact
        // boolean difference (the planar/general path, itself gated), whose
        // mass==mesh post-condition declines the degenerate case honestly.
        let inner_v = inner.mesh_volume();
        let outer_v = self.mesh_volume();
        let inner_contained = inner.validate().is_ok()
            && inner_v.is_finite()
            && inner_v > 0.0
            && inner_v < outer_v;
        if inner_contained
            && let Ok(body) = crate::boolean::combine_containment(self, &inner, 1e-7)
            && body.validate().is_ok()
            && shell_mass_matches_mesh(&body)
        {
            return Ok(body);
        }
        boolean(self, &inner, BoolOp::Difference, 1e-7)
            .map(|r| r.body)
            .map_err(|_| {
                TopoError::Precondition("hollow: shell assembly failed (thickness >= t_max?)")
            })
    }

    /// Hollow with PIERCED (open) faces (parity item 42): the faces for
    /// which `pierced(face)` is true are opened so the interior void
    /// communicates with the outside through them (a cup / open tray rather
    /// than a sealed cavity). Mechanism: the inner shell's counterpart of a
    /// pierced face is pushed OUTWARD past the original face instead of
    /// inward, so the subtracted inner pocket pokes through it and the
    /// difference opens that side (the rim of the opening is the wall's edge,
    /// produced transversally by the boolean -- no separate rim-wall surgery).
    /// Non-pierced faces shell inward by `t` as usual. Convex planar scope,
    /// like [`hollow`](Self::hollow); at least one face must remain
    /// un-pierced (an all-pierced body has no wall). The classic case is
    /// `pierced = |f| top(f)` -> an open box (tray).
    pub fn hollow_pierce(
        &self,
        t: f64,
        pierced: impl Fn(crate::entity::FaceKey) -> bool,
    ) -> Result<Body, TopoError> {
        if t <= 0.0 || !t.is_finite() {
            return Err(TopoError::Precondition("hollow: thickness must be > 0"));
        }
        let bb = self.bounding_box();
        // Push pierced faces well outside the body so their inner-pocket
        // counterpart pokes fully through the original face.
        let margin = (bb.max - bb.min).norm() + 1.0;
        let mut inner = self.clone();
        inner.offset_body_with(|f| if pierced(f) { margin } else { -t })?;
        boolean(self, &inner, BoolOp::Difference, 1e-7)
            .map(|r| r.body)
            .map_err(|_| TopoError::Precondition("hollow_pierce: shell assembly failed"))
    }

    /// Split a solid by a cutting plane into two pieces (parity item 76):
    /// the BACK piece (where `(p - point) . normal <= 0`) and the FRONT
    /// piece. Each piece is the body with the OTHER half-space removed by a
    /// guillotine difference against a large oriented slab (the well-tested
    /// transversal-cut path), so the cut cap is produced by the ordinary
    /// boolean -- no new machinery. Returns `(back, front)`. Errors if either
    /// difference fails (e.g. the plane misses the body, leaving a piece
    /// equal to the whole body or empty).
    pub fn split_by_plane(&self, point: Vec3, normal: Vec3) -> Result<(Body, Body), TopoError> {
        let n = normal
            .try_normalize()
            .ok_or(TopoError::Precondition("split: degenerate plane normal"))?;
        let bb = self.bounding_box();
        let big = (bb.max - bb.min).norm() * 2.0 + 10.0;
        // In-plane orthonormal axes.
        let seed = if n.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let u = (seed - n * seed.dot(n))
            .try_normalize()
            .ok_or(TopoError::Precondition("split: axis"))?;
        let v = n.cross(u);
        // A half-space slab on `side` of the plane (-1 back, +1 front): a big
        // square at the far face extruded back toward the plane. prism
        // auto-orients the base to dir, so either winding yields a consistent
        // solid (mass == mesh).
        let slab = |side: f64| -> Result<Body, TopoError> {
            let far = point + n * (side * big);
            let q = |a: f64, b: f64| far + u * a + v * b;
            let dir = n * (-side * big);
            let base = vec![q(-big, -big), q(big, -big), q(big, big), q(-big, big)];
            let mut s = Body::new();
            s.prism(&base, dir)?;
            Ok(s)
        };
        let back = slab(-1.0)?;
        let front = slab(1.0)?;
        // Each piece is the body with the OTHER half-space removed (a
        // guillotine difference, the well-tested transversal-cut case): the
        // back piece = body minus the front slab, and vice versa.
        let pb = boolean(self, &front, BoolOp::Difference, 1e-7)
            .map(|r| r.body)
            .map_err(|_| TopoError::Precondition("split: back piece failed"))?;
        let pf = boolean(self, &back, BoolOp::Difference, 1e-7)
            .map(|r| r.body)
            .map_err(|_| TopoError::Precondition("split: front piece failed"))?;
        Ok((pb, pf))
    }

    /// Slice a solid by a list of parallel cutting planes (parity item 77):
    /// planes at `point + off * normal` for each `off` in `offsets`, yielding
    /// the ordered pieces between consecutive planes (N offsets -> N+1
    /// pieces). Implemented as a repeated [`split_by_plane`](Self::split_by_plane)
    /// (split off the back piece at each plane in increasing order, carry the
    /// front forward), so it inherits split's coverage. A piece that an
    /// offset does not actually cut comes back equal to the carried body.
    pub fn slice(
        &self,
        point: Vec3,
        normal: Vec3,
        offsets: &[f64],
    ) -> Result<Vec<Body>, TopoError> {
        let n = normal
            .try_normalize()
            .ok_or(TopoError::Precondition("slice: degenerate normal"))?;
        let mut sorted: Vec<f64> = offsets.iter().copied().filter(|o| o.is_finite()).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut pieces = Vec::with_capacity(sorted.len() + 1);
        let mut remaining = self.clone();
        for &off in &sorted {
            let (back, front) = remaining.split_by_plane(point + n * off, n)?;
            pieces.push(back);
            remaining = front;
        }
        pieces.push(remaining);
        Ok(pieces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prism_cw_base_is_consistent() {
        // A prism whose base is wound CW about the extrude direction must
        // still produce a CONSISTENT solid (mass_properties == mesh_volume).
        // Before the auto-orient fix this gave mass > 0 but mesh < 0, which
        // inverted the generalized winding number and broke intersection with
        // an oriented half-slab (the symptom that masqueraded as a boolean
        // asymmetry). Square base, CW about +z; extrude +z; volume 2*2*3 = 12.
        let mut p = Body::new();
        p.prism(
            // CW about +z (clockwise) -- the auto-orient must fix it.
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(2.0, 2.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
            ],
            Vec3::new(0.0, 0.0, 3.0),
        )
        .unwrap();
        assert!(p.validate().is_ok(), "CW-base prism invalid");
        let mass = p.mass_properties().unwrap().volume;
        let mesh = p.mesh_volume();
        assert!(
            (mass - 12.0).abs() < 1e-9 && (mesh - 12.0).abs() < 1e-9,
            "CW-base prism must be consistent: mass {mass}, mesh {mesh} (both 12)"
        );
    }

    #[test]
    fn intersection_with_oriented_slabs_is_symmetric() {
        // Regression for the prism CW-base fix via split's oriented slabs:
        // a 4^3 box split by x=2 must give two valid 32-volume halves (the
        // front half previously failed because its slab was GWN-inverted).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let (back, front) = b
            .split_by_plane(Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        for (p, who) in [(back, "back"), (front, "front")] {
            assert!(p.validate().is_ok(), "{who} piece invalid");
            let v = p.mass_properties().unwrap().volume;
            assert!((v - 32.0).abs() < 1e-6, "{who} half must be 32 (got {v})");
        }
    }

    #[test]
    fn split_box_by_plane_halves() {
        // Split a 4^3 box by x=2 (normal +x): back piece x<=2 = 2x4x4 = 32,
        // front piece x>=2 = 32; both valid solids summing to the original.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let (back, front) = b
            .split_by_plane(Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        assert!(
            back.validate().is_ok() && front.validate().is_ok(),
            "split pieces invalid"
        );
        let vb = back.mass_properties().unwrap().volume;
        let vf = front.mass_properties().unwrap().volume;
        assert!(
            (vb - 32.0).abs() < 1e-6 && (vf - 32.0).abs() < 1e-6,
            "split halves must each be 32 (got back {vb}, front {vf})"
        );
    }

    #[test]
    fn slice_box_into_three() {
        // Slice a 6^3 box at x=2 and x=4 -> three 2x6x6 = 72 slabs.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 6.0, 6.0, 6.0).unwrap();
        let pieces = b
            .slice(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), &[2.0, 4.0])
            .unwrap();
        assert_eq!(pieces.len(), 3, "two offsets must give three pieces");
        for p in &pieces {
            assert!(p.validate().is_ok(), "slice piece invalid");
            let v = p.mass_properties().unwrap().volume;
            assert!((v - 72.0).abs() < 1e-6, "each slab must be 72 (got {v})");
        }
    }

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
    fn hollow_pierce_top_makes_a_tray() {
        // Pierce the top of a 4^3 box at wall thickness 1 -> an open tray:
        // floor z in [0,1] (4x4) plus walls around a 2x2 opening from z=1 to
        // 4. Removed volume = the 2x2x3 pocket = 12, so the tray volume is
        // 64 - 12 = 52. Single connected shell (the void opens through the
        // top), no enclosed void (dossier 50 sec 6.2).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let top = b
            .face_keys()
            .into_iter()
            .find(|&f| b.face_outward_normal(f).map(|n| n.z > 0.9).unwrap_or(false))
            .expect("top face");
        let tray = b.hollow_pierce(1.0, |f| f == top).unwrap();
        assert!(tray.validate().is_ok(), "tray invalid");
        let v = tray.mass_properties().unwrap().volume;
        let mv = tray.mesh_volume();
        assert!(
            (v - 52.0).abs() < 1e-6 && (mv - 52.0).abs() < 1e-6,
            "open tray volume must be 52 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn hollow_filleted_box_shells() {
        // The extrude->fillet->shell workflow: a 40x40x30 block whose four
        // TOP edges are filleted (r=4), then hollowed to a 3 mm wall. The
        // offset must handle the upstream cylindrical fillet faces (offset
        // each plane inward, shrink each fillet cylinder r 4->1, re-solve the
        // plane/cylinder/cylinder corners), and the two nested filleted
        // shells must assemble into a watertight wall (mass == mesh). This is
        // the curved counterpart of `hollow_box_encloses_void`.
        use keel_math::vec::Vec3;
        let mut b = Body::new();
        b.sweep(
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(40.0, 0.0, 0.0),
                Vec3::new(40.0, 40.0, 0.0),
                Vec3::new(0.0, 40.0, 0.0),
            ],
            Vec3::new(0.0, 0.0, 30.0),
        )
        .unwrap();
        // Fillet the four top edges (z = 30) at r = 4.
        let top_mids = [
            Vec3::new(20.0, 0.0, 30.0),
            Vec3::new(40.0, 20.0, 30.0),
            Vec3::new(20.0, 40.0, 30.0),
            Vec3::new(0.0, 20.0, 30.0),
        ];
        let mut cur = b;
        for m in top_mids {
            let e = cur
                .entity_ids()
                .filter_map(|id| match cur.lookup(id) {
                    Some(crate::entity::AnyKey::Edge(k)) => Some(k),
                    _ => None,
                })
                .filter(|&e| cur.faces_around_edge(e).len() == 2)
                .filter_map(|e| {
                    let (va, vb) = cur.edge(e)?.bounds;
                    let (pa, pb) = (cur.vertex(va)?.point, cur.vertex(vb)?.point);
                    if (pb - pa).norm() <= 1e-6 {
                        return None;
                    }
                    Some((e, (((pa + pb) * 0.5) - m).norm()))
                })
                .min_by(|a, c| a.1.partial_cmp(&c.1).unwrap())
                .map(|(e, _)| e)
                .expect("top edge");
            cur = cur.fillet_edge(e, 4.0).expect("fillet top edge");
        }
        // Curved solid present; now shell it.
        let shell = cur.hollow(3.0).expect("hollow filleted box");
        assert!(shell.validate().is_ok(), "filleted shell invalid");
        let mass = shell.mass_properties().unwrap().volume;
        let mesh = shell.mesh_volume();
        // A 3 mm wall on the filleted ~47.5k solid: a real hollow (well below
        // the solid, above zero), with the analytic mass tracking the mesh
        // within the curved chordal band.
        assert!(
            mass.is_finite() && mass > 8_000.0 && mass < 30_000.0,
            "filleted-shell wall volume {mass} out of range"
        );
        assert!(
            (mass - mesh).abs() <= 0.03 * mass.max(mesh),
            "filleted shell mass {mass} != mesh {mesh}"
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
