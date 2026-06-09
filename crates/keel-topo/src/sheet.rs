//! Sheet bodies (kernel/51) and THICKEN (parity item 44).
//!
//! A SHEET body is an open, double-sided lamina: one (here planar) face
//! whose boundary edges are FREE (one coedge / radial-1 each) and which
//! borders the ambient void on BOTH sides (no enclosed solid region). This
//! is the first non-solid body kind in Keel. The existing validator already
//! admits it with no change: free edges make the body non-manifold, so the
//! closed-shell Euler check auto-skips, and `check_shells_regions` requires
//! only the single infinite void region (not a solid one), which the
//! double-sided face borders on both sides.
//!
//! THICKEN (dossier 50 sec 5) turns a sheet into a solid of wall thickness
//! `t`. This first increment covers the single PLANAR face: a two-sided
//! thicken is the boundary profile extruded to thickness `t` centred on the
//! sheet plane. Multi-face / curved sheets (offset-both-sides + a rim band)
//! and the other sheet ops that share this representation -- extend (70),
//! knit/sew (71), trim (72), split (76) -- are follow-ups.

use crate::body::{Body, TopoError};
use crate::entity::{LoopKind, Side, SurfaceGeom};
use crate::lineage::Derivation;
use keel_geom::curve::{Curve3, Line3};
use keel_geom::surface::{Frame3, Plane3, Surface3};
use keel_math::vec::Vec3;

impl Body {
    /// Construct a planar SHEET body (lamina) from a closed planar profile
    /// (counterclockwise about the desired up-normal; planarity and
    /// simplicity are the caller's contract). The result is one double-sided
    /// planar face bounded by `n` free edges, with no solid region.
    pub fn planar_sheet(profile: &[Vec3]) -> Result<Body, TopoError> {
        let n = profile.len();
        if n < 3 || profile.iter().any(|p| !p.is_finite()) {
            return Err(TopoError::Precondition(
                "planar_sheet: need 3+ finite points",
            ));
        }
        // Newell's normal (robust to non-flat input, sign = winding).
        let mut nrm = Vec3::ZERO;
        for i in 0..n {
            let a = profile[i];
            let b = profile[(i + 1) % n];
            nrm = nrm
                + Vec3::new(
                    (a.y - b.y) * (a.z + b.z),
                    (a.z - b.z) * (a.x + b.x),
                    (a.x - b.x) * (a.y + b.y),
                );
        }
        let normal = nrm
            .try_normalize()
            .ok_or(TopoError::Precondition("planar_sheet: degenerate profile"))?;

        let mut b = Body::new();
        let void = b.infinite_region();
        let mut rec = b.begin_op();
        // Double-sided face: both sides border the ambient void.
        let face = b.new_face(&mut rec, void, void, Derivation::Created);
        let lp = b.new_loop(&mut rec, face, LoopKind::Outer, Derivation::Created);
        if let Some(f) = b.faces.get_mut(face) {
            f.loops.push(lp);
        }
        let verts: Vec<_> = profile.iter().map(|&p| b.new_vertex(&mut rec, p)).collect();
        let mut edges = Vec::with_capacity(n);
        let mut fins = Vec::with_capacity(n);
        for i in 0..n {
            let e = b.new_edge(
                &mut rec,
                (verts[i], verts[(i + 1) % n]),
                Derivation::Created,
            );
            let fin = b.new_fin(&mut rec, e, true, lp, Derivation::Created);
            if let Some(ed) = b.edges.get_mut(e) {
                ed.radial.push(fin); // free edge: a single coedge
            }
            edges.push(e);
            fins.push(fin);
        }
        // Splice the fin ring around the loop.
        for i in 0..n {
            let nxt = fins[(i + 1) % n];
            let prv = fins[(i + n - 1) % n];
            if let Some(f) = b.fins.get_mut(fins[i]) {
                f.next = nxt;
                f.prev = prv;
            }
        }
        if let Some(l) = b.loops.get_mut(lp) {
            l.fin = Some(fins[0]);
        }
        for i in 0..n {
            if let Some(v) = b.vertices.get_mut(verts[i]) {
                v.fin = Some(fins[i]);
            }
        }
        // One shell in the void holding both sides of the double-sided face.
        let shell = b.new_shell(&mut rec, void, Derivation::Created);
        if let Some(s) = b.shells.get_mut(shell) {
            s.faces.push((face, Side::Front));
            s.faces.push((face, Side::Back));
        }
        if let Some(r) = b.regions.get_mut(void) {
            r.shells.push(shell);
        }
        let _ = rec.finish();

        // Geometry: the plane and the boundary lines.
        let frame = Frame3::from_z(profile[0], normal)
            .map_err(|_| TopoError::Precondition("planar_sheet: degenerate frame"))?;
        b.attach_face_surface(
            face,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(frame))),
            true,
        );
        for i in 0..n {
            if let Ok(line) = Line3::new(profile[i], profile[(i + 1) % n] - profile[i]) {
                b.attach_edge_curve(edges[i], Curve3::Line(line), true);
            }
        }
        Ok(b)
    }

    /// Thicken a planar sheet body into a solid of wall thickness `t`
    /// (parity item 44), two-sided (+/- t/2 about the sheet plane). MVP: a
    /// single planar face. The thickened solid is the sheet's boundary
    /// profile extruded to thickness `t`, centred on the sheet plane.
    pub fn thicken(&self, t: f64) -> Result<Body, TopoError> {
        if t <= 0.0 || !t.is_finite() {
            return Err(TopoError::Precondition("thicken: thickness must be > 0"));
        }
        let faces = self.face_keys();
        let [face] = faces[..] else {
            return Err(TopoError::Precondition(
                "thicken: only a single-face planar sheet is supported (MVP)",
            ));
        };
        if !matches!(self.face_surface3(face), Some(Surface3::Plane(_))) {
            return Err(TopoError::Precondition("thicken: non-planar sheet (MVP)"));
        }
        let normal = self
            .face_outward_normal(face)
            .ok_or(TopoError::Precondition("thicken: no face normal"))?;
        // Boundary profile: the outer-loop vertices in fin order.
        let profile = self.face_outer_loop_points(face);
        if profile.len() < 3 {
            return Err(TopoError::Precondition("thicken: degenerate boundary"));
        }
        // Extrude centred: base shifted by -t/2 along the normal, swept +t.
        let base: Vec<Vec3> = profile.iter().map(|&p| p - normal * (t * 0.5)).collect();
        let mut solid = Body::new();
        solid.prism(&base, normal * t)?;
        Ok(solid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_sheet_is_a_valid_open_body() {
        // A unit square sheet: one double-sided face, four free edges, no
        // solid region. It must validate (the non-manifold free edges make
        // the closed-shell Euler check auto-skip).
        let s = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 3.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ])
        .unwrap();
        assert!(
            s.validate().is_ok(),
            "planar sheet invalid: {:?}",
            s.validate()
        );
        // Open body: no enclosed solid, so zero solid volume by the mesh.
        assert_eq!(s.face_keys().len(), 1, "sheet must have one face");
    }

    #[test]
    fn thicken_sheet_to_slab() {
        // Thicken a 2x3 planar sheet by t=0.5 -> a slab of volume
        // 2*3*0.5 = 3.0, centred on the sheet plane.
        let s = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 3.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ])
        .unwrap();
        let solid = s.thicken(0.5).unwrap();
        assert!(solid.validate().is_ok(), "thickened slab invalid");
        let v = solid.mass_properties().unwrap().volume;
        let mv = solid.mesh_volume();
        assert!(
            (v - 3.0).abs() < 1e-9 && (mv - 3.0).abs() < 1e-9,
            "thickened slab volume must be 3.0 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }
}
