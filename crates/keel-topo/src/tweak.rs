//! Local operations / tweak (parity Phase 1, items 35-37, 39): the
//! foundation local operation per research file 03 -- change a face's
//! surface, then re-intersect the affected edges and corners so the
//! solid stays valid, WITHOUT changing topology. OCCT lacks this; it is
//! a deliberate differentiator.
//!
//! This first cut covers the PLANAR / polyhedral case: moving a planar
//! face's plane (offset along the outward normal, translate, or replace
//! outright) recomputes each incident corner as the intersection of the
//! new plane with its two neighbour planes, then rebuilds the incident
//! straight edges. Curved tweak (cylinders/spheres re-intersected by
//! SSI) is the next slice.

use crate::Body;
use crate::body::TopoError;
use crate::entity::{FaceKey, SurfaceGeom, VertexKey};
use keel_geom::curve::{Curve3, Line3};
use keel_geom::surface::{Plane3, Surface3};
use keel_math::vec::Vec3;

/// Intersection point of three planes `n_i . x = d_i`, or None if they
/// do not meet in a single point (near-parallel).
fn three_plane_point(n0: Vec3, d0: f64, n1: Vec3, d1: f64, n2: Vec3, d2: f64) -> Option<Vec3> {
    let det = n0.dot(n1.cross(n2));
    if det.abs() < 1e-12 {
        return None;
    }
    Some((n1.cross(n2) * d0 + n2.cross(n0) * d1 + n0.cross(n1) * d2) * (1.0 / det))
}

impl Body {
    /// The (plane, sense) of a planar face, or None if non-planar.
    fn face_plane(&self, face: FaceKey) -> Option<(Plane3, bool)> {
        let (sk, sense) = self.faces.get(face).and_then(|f| f.surface)?;
        match self.surfaces.get(sk) {
            Some(SurfaceGeom::Analytic(Surface3::Plane(p))) => Some((p.clone(), sense)),
            _ => None,
        }
    }

    /// Ordered distinct vertices of a face's outer loop.
    fn face_loop_vertices(&self, face: FaceKey) -> Option<Vec<VertexKey>> {
        let lp = self.faces.get(face)?.loops.first().copied()?;
        let first = self.loops.get(lp)?.fin?;
        let mut out = Vec::new();
        let mut fk = first;
        loop {
            let fin = self.fins.get(fk)?;
            let e = self.edges.get(fin.edge)?;
            let v = if fin.forward { e.bounds.0 } else { e.bounds.1 };
            if !out.contains(&v) {
                out.push(v);
            }
            fk = fin.next;
            if fk == first {
                break;
            }
        }
        Some(out)
    }

    /// Change a planar face's plane to `new_plane` and re-solve the
    /// affected geometry (items 35/39: change/replace surface). Each
    /// corner of the face is recomputed as `new_plane` intersected with
    /// its two neighbour planes; the incident straight edges are rebuilt
    /// through the moved corners. Topology is unchanged. Errors if the
    /// face or a neighbour is non-planar, or a corner is non-simple
    /// (not exactly two neighbour planes) or degenerate.
    pub fn tweak_face_to_plane(
        &mut self,
        face: FaceKey,
        new_plane: Plane3,
        sense: bool,
    ) -> Result<(), TopoError> {
        let verts = self.face_loop_vertices(face).ok_or(TopoError::StaleKey)?;
        let nf = new_plane.frame.z;
        let df = nf.dot(new_plane.frame.origin);

        // Compute every new corner first (neighbour planes are unchanged,
        // so order is irrelevant), then apply.
        let mut moves: Vec<(VertexKey, Vec3)> = Vec::new();
        for &v in &verts {
            let mut planes: Vec<(Vec3, f64)> = Vec::new();
            let mut seen: Vec<FaceKey> = Vec::new();
            for e in self.edges_of_vertex(v) {
                for nbr in self.faces_around_edge(e) {
                    if nbr == face || seen.contains(&nbr) {
                        continue;
                    }
                    seen.push(nbr);
                    let (pl, _) = self
                        .face_plane(nbr)
                        .ok_or(TopoError::Precondition("tweak: non-planar neighbour"))?;
                    planes.push((pl.frame.z, pl.frame.z.dot(pl.frame.origin)));
                }
            }
            if planes.len() != 2 {
                return Err(TopoError::Precondition("tweak: non-simple corner"));
            }
            let p = three_plane_point(nf, df, planes[0].0, planes[0].1, planes[1].0, planes[1].1)
                .ok_or(TopoError::Precondition("tweak: degenerate corner"))?;
            moves.push((v, p));
        }

        // Apply corner moves.
        for &(v, p) in &moves {
            if let Some(vx) = self.vertices.get_mut(v) {
                vx.point = p;
            }
        }

        // Rebuild every edge incident to a moved corner as the straight
        // line through its (updated) endpoints.
        let mut edges = Vec::new();
        for &v in &verts {
            for e in self.edges_of_vertex(v) {
                if !edges.contains(&e) {
                    edges.push(e);
                }
            }
        }
        for e in edges {
            let (v0, v1) = self.edges.get(e).ok_or(TopoError::StaleKey)?.bounds;
            let p0 = self.vertices.get(v0).ok_or(TopoError::StaleKey)?.point;
            let p1 = self.vertices.get(v1).ok_or(TopoError::StaleKey)?.point;
            if let Ok(line) = Line3::new(p0, p1 - p0) {
                self.attach_edge_curve(e, Curve3::Line(line), true);
            }
        }

        // Swap the face surface (orientation preserved).
        self.attach_face_surface(
            face,
            SurfaceGeom::Analytic(Surface3::Plane(new_plane)),
            sense,
        );
        Ok(())
    }

    /// Offset a planar face outward by `distance` (item 37): shift its
    /// plane along the outward normal and re-intersect. Negative moves
    /// inward.
    pub fn offset_face(&mut self, face: FaceKey, distance: f64) -> Result<(), TopoError> {
        let (plane, sense) = self
            .face_plane(face)
            .ok_or(TopoError::Precondition("offset_face: non-planar"))?;
        let outward = if sense {
            plane.frame.z
        } else {
            plane.frame.z * -1.0
        };
        let mut np = plane;
        np.frame.origin = np.frame.origin + outward * distance;
        self.tweak_face_to_plane(face, np, sense)
    }

    /// Translate a planar face by `t` (item 36): shift its plane origin
    /// and re-intersect (only the normal component changes the plane).
    pub fn move_face(&mut self, face: FaceKey, t: Vec3) -> Result<(), TopoError> {
        let (plane, sense) = self
            .face_plane(face)
            .ok_or(TopoError::Precondition("move_face: non-planar"))?;
        let mut np = plane;
        np.frame.origin = np.frame.origin + t;
        self.tweak_face_to_plane(face, np, sense)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The planar face whose loop vertices have the greatest mean z (the
    /// "top" of an axis-aligned block).
    fn top_face(b: &Body) -> FaceKey {
        let mut best = (f64::NEG_INFINITY, None);
        for f in b.face_keys() {
            if let Some(vs) = b.face_loop_vertices(f) {
                let n = vs.len() as f64;
                let z: f64 = vs
                    .iter()
                    .filter_map(|&v| b.vertices.get(v))
                    .map(|x| x.point.z)
                    .sum();
                let mean = z / n;
                if mean > best.0 {
                    best = (mean, Some(f));
                }
            }
        }
        best.1.unwrap()
    }

    #[test]
    fn offset_face_grows_block() {
        // A 2x2x2 block (volume 8); offsetting the top face outward by 1
        // makes it 2x2x3 (volume 12), staying valid.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let top = top_face(&b);
        b.offset_face(top, 1.0).unwrap();
        assert!(b.validate().is_ok(), "tweaked block invalid");
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 12.0).abs() < 1e-9, "volume {v} != 12");
    }

    #[test]
    fn offset_face_inward_shrinks_block() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let top = top_face(&b);
        b.offset_face(top, -0.5).unwrap(); // top down 0.5 -> 2x2x1.5
        assert!(b.validate().is_ok());
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 6.0).abs() < 1e-9, "volume {v} != 6");
    }

    #[test]
    fn move_face_translates_top() {
        // Moving the top face up by 2 grows the block to volume 16.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let top = top_face(&b);
        b.move_face(top, Vec3::new(0.0, 0.0, 2.0)).unwrap();
        assert!(b.validate().is_ok());
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 16.0).abs() < 1e-9, "volume {v} != 16");
    }
}
