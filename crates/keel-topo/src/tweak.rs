//! Local operations / tweak (parity Phase 1, items 35-37, 39): the
//! foundation local operation per research file 03 -- change a face's
//! surface, then re-intersect the affected edges and corners so the
//! solid stays valid, WITHOUT changing topology. OCCT lacks this; it is
//! a deliberate differentiator.
//!
//! PLANAR case: moving a planar face's plane (offset along the outward
//! normal, translate, or replace outright) recomputes each incident
//! corner as the intersection of the new plane with its two neighbour
//! planes, then rebuilds the incident straight edges. CURVED case:
//! offsetting a cylindrical face changes its radius, recomputing the
//! cap circles (cylinder ^ cap-plane) at the new radius and moving the
//! seam. TAPER/DRAFT rotates the plane about a parting line. DELETE-FACE
//! heals by merging a face into a coplanar neighbour (kef). Deferred:
//! sphere/cone/torus tweak, and the extend-and-reintersect delete-heal
//! (needs edge contraction the Euler op set lacks).

use crate::Body;
use crate::body::TopoError;
use crate::entity::{EdgeKey, FaceKey, SurfaceGeom, VertexKey};
use keel_geom::curve::{Circle3, Curve3, Line3};
use keel_geom::surface::{Cylinder3, Frame3, Plane3, Surface3};
use keel_math::transform::Transform3;
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

    /// Ordered distinct edges of a face's outer loop.
    fn face_loop_edges(&self, face: FaceKey) -> Option<Vec<EdgeKey>> {
        let lp = self.faces.get(face)?.loops.first().copied()?;
        let first = self.loops.get(lp)?.fin?;
        let mut out = Vec::new();
        let mut fk = first;
        loop {
            let e = self.fins.get(fk)?.edge;
            if !out.contains(&e) {
                out.push(e);
            }
            fk = self.fins.get(fk)?.next;
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

    /// Offset a face outward by `distance` (item 37), re-intersecting its
    /// boundary. Planar faces shift along the outward normal; cylindrical
    /// faces change radius (the curved slice). Negative moves inward.
    pub fn offset_face(&mut self, face: FaceKey, distance: f64) -> Result<(), TopoError> {
        match self.face_surface_geom(face) {
            Some(SurfaceGeom::Analytic(Surface3::Plane(_))) => {
                let (plane, sense) = self.face_plane(face).ok_or(TopoError::StaleKey)?;
                let outward = if sense {
                    plane.frame.z
                } else {
                    plane.frame.z * -1.0
                };
                let mut np = plane;
                np.frame.origin = np.frame.origin + outward * distance;
                self.tweak_face_to_plane(face, np, sense)
            }
            Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => {
                let sense = self
                    .faces
                    .get(face)
                    .and_then(|f| f.surface)
                    .map(|(_, s)| s)
                    .unwrap_or(true);
                self.tweak_cylinder_radius(face, c, distance, sense)
            }
            _ => Err(TopoError::Precondition("offset_face: unsupported surface")),
        }
    }

    /// Curved tweak: change a cylindrical face's radius (offset along the
    /// outward radial normal). Each circular boundary edge (the
    /// cylinder ^ cap-plane circles) is recomputed at the new radius and
    /// the seam line + corner vertices move radially. Topology unchanged.
    fn tweak_cylinder_radius(
        &mut self,
        face: FaceKey,
        cyl: Cylinder3,
        distance: f64,
        sense: bool,
    ) -> Result<(), TopoError> {
        // Outward is radially out for a solid (sense true); a hole's wall
        // (sense false) offsets the other way.
        let new_r = cyl.radius + if sense { distance } else { -distance };
        if new_r <= 1e-9 {
            return Err(TopoError::Precondition(
                "offset_face: cylinder radius -> <= 0",
            ));
        }
        let (origin, axis) = (cyl.frame.origin, cyl.frame.z);

        // Move each corner radially to the new radius, keeping its height.
        let verts = self.face_loop_vertices(face).ok_or(TopoError::StaleKey)?;
        for &v in &verts {
            let p = self.vertices.get(v).ok_or(TopoError::StaleKey)?.point;
            let t = (p - origin).dot(axis);
            let axis_pt = origin + axis * t;
            if let Some(ru) = (p - axis_pt).try_normalize()
                && let Some(vx) = self.vertices.get_mut(v)
            {
                vx.point = axis_pt + ru * new_r;
            }
        }

        // Recompute boundary edges: cap circles at the new radius, the
        // seam line through its moved endpoints.
        let edges = self.face_loop_edges(face).ok_or(TopoError::StaleKey)?;
        for e in edges {
            let ck = self.edges.get(e).and_then(|x| x.curve).map(|(k, _)| k);
            match ck.and_then(|k| self.curves.get(k).cloned()) {
                Some(Curve3::Circle(c)) => {
                    if let Ok(nc) = Circle3::new(c.center, c.x_axis, c.y_axis, new_r) {
                        self.attach_edge_curve(e, Curve3::Circle(nc), true);
                    }
                }
                Some(Curve3::Line(_)) => {
                    let (v0, v1) = self.edges.get(e).ok_or(TopoError::StaleKey)?.bounds;
                    let p0 = self.vertices.get(v0).ok_or(TopoError::StaleKey)?.point;
                    let p1 = self.vertices.get(v1).ok_or(TopoError::StaleKey)?.point;
                    if let Ok(l) = Line3::new(p0, p1 - p0) {
                        self.attach_edge_curve(e, Curve3::Line(l), true);
                    }
                }
                _ => {}
            }
        }

        self.attach_face_surface(
            face,
            SurfaceGeom::Analytic(Surface3::Cylinder(Cylinder3 {
                frame: cyl.frame,
                radius: new_r,
            })),
            sense,
        );
        Ok(())
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

    /// Taper / draft a planar face (items 38, 78): rotate its plane by
    /// `angle` about the parting line through `pivot` along `axis`, then
    /// re-intersect. Corners on the parting line stay fixed; the rest
    /// follow the tilt. The parting line is the neutral/parting
    /// reference; the draft angle is `angle`.
    pub fn taper_face(
        &mut self,
        face: FaceKey,
        pivot: Vec3,
        axis: Vec3,
        angle: f64,
    ) -> Result<(), TopoError> {
        let (plane, sense) = self
            .face_plane(face)
            .ok_or(TopoError::Precondition("taper_face: non-planar"))?;
        let rot = Transform3::from_rotation(axis, angle)
            .ok_or(TopoError::Precondition("taper_face: bad axis"))?;
        let new_normal = rot.apply_vector(plane.frame.z);
        let new_origin = rot.apply_point(plane.frame.origin - pivot) + pivot;
        let frame = Frame3::from_z(new_origin, new_normal)
            .map_err(|_| TopoError::Precondition("taper_face: degenerate normal"))?;
        self.tweak_face_to_plane(face, Plane3::new(frame), sense)
    }

    /// Offset the whole body outward by `distance` (item 45): move every
    /// planar face along its outward normal and re-intersect all corners.
    /// Each vertex is recomputed as the intersection of its three incident
    /// (offset) face planes. Convex polyhedra only; self-intersection
    /// resolution for concave/curved bodies is deferred. Negative shrinks.
    pub fn offset_body(&mut self, distance: f64) -> Result<(), TopoError> {
        self.offset_body_with(|_| distance)
    }

    /// Per-face offset (item 43, multi-thickness): each face moves along its
    /// outward normal by `dist(face)` instead of a single global distance.
    /// The convex-planar / 3-valent-corner reintersect of `offset_body`; the
    /// three-plane vertex meet handles faces offset by different amounts
    /// (their offset planes simply meet at a new corner). Negative shrinks.
    pub(crate) fn offset_body_with(
        &mut self,
        dist: impl Fn(FaceKey) -> f64,
    ) -> Result<(), TopoError> {
        use std::collections::HashMap;
        // New offset plane (+ sense) per face.
        let mut planes: HashMap<FaceKey, (Plane3, bool)> = HashMap::new();
        for f in self.face_keys() {
            let (plane, sense) = self
                .face_plane(f)
                .ok_or(TopoError::Precondition("offset_body: non-planar face"))?;
            let outward = if sense {
                plane.frame.z
            } else {
                plane.frame.z * -1.0
            };
            let mut np = plane;
            np.frame.origin = np.frame.origin + outward * dist(f);
            planes.insert(f, (np, sense));
        }
        // Recompute every vertex as the meet of its three incident planes.
        let vkeys: Vec<VertexKey> = self.vertices.iter().map(|(k, _)| k).collect();
        let mut moves: Vec<(VertexKey, Vec3)> = Vec::new();
        for v in vkeys {
            let mut faces: Vec<FaceKey> = Vec::new();
            for e in self.edges_of_vertex(v) {
                for f in self.faces_around_edge(e) {
                    if !faces.contains(&f) {
                        faces.push(f);
                    }
                }
            }
            if faces.len() != 3 {
                return Err(TopoError::Precondition("offset_body: non-simple vertex"));
            }
            let pl: Vec<(Vec3, f64)> = faces
                .iter()
                .map(|f| {
                    let (p, _) = &planes[f];
                    (p.frame.z, p.frame.z.dot(p.frame.origin))
                })
                .collect();
            let p = three_plane_point(pl[0].0, pl[0].1, pl[1].0, pl[1].1, pl[2].0, pl[2].1)
                .ok_or(TopoError::Precondition("offset_body: degenerate corner"))?;
            moves.push((v, p));
        }
        for &(v, p) in &moves {
            if let Some(vx) = self.vertices.get_mut(v) {
                vx.point = p;
            }
        }
        // Rebuild every edge as the line through its updated endpoints.
        let ekeys: Vec<EdgeKey> = self.edges.iter().map(|(k, _)| k).collect();
        for e in ekeys {
            let (v0, v1) = self.edges.get(e).ok_or(TopoError::StaleKey)?.bounds;
            let p0 = self.vertices.get(v0).ok_or(TopoError::StaleKey)?.point;
            let p1 = self.vertices.get(v1).ok_or(TopoError::StaleKey)?.point;
            if let Ok(line) = Line3::new(p0, p1 - p0) {
                self.attach_edge_curve(e, Curve3::Line(line), true);
            }
        }
        // Set each face's offset surface.
        for (f, (np, sense)) in planes {
            self.attach_face_surface(f, SurfaceGeom::Analytic(Surface3::Plane(np)), sense);
        }
        Ok(())
    }

    /// Delete a face, healing the wound (item 40). This is the
    /// MERGE/absorb heal mode: the face is merged into a coplanar
    /// neighbour by killing their shared edge (`kef`), so the neighbour
    /// absorbs the face's region with no geometric change. This handles
    /// redundant/sliver coplanar faces and undoes a coplanar split (also
    /// item 133, face merging). The EXTEND-AND-REINTERSECT heal (delete a
    /// non-coplanar face so its neighbours grow back to meet) needs
    /// edge-contraction surgery the current Euler op set does not provide
    /// (file 03: delete-face is a dedicated, non-decomposable primitive);
    /// that mode is deferred.
    pub fn delete_face(&mut self, face: FaceKey) -> Result<(), TopoError> {
        let (fplane, _) = self.face_plane(face).ok_or(TopoError::Precondition(
            "delete_face: non-planar (only the coplanar-merge heal is supported)",
        ))?;
        let edges = self.face_loop_edges(face).ok_or(TopoError::StaleKey)?;
        for e in edges {
            for nbr in self.faces_around_edge(e) {
                if nbr == face {
                    continue;
                }
                if let Some((nplane, _)) = self.face_plane(nbr) {
                    let parallel = fplane.frame.z.cross(nplane.frame.z).norm() < 1e-9;
                    let coincident = (nplane.frame.origin - fplane.frame.origin)
                        .dot(fplane.frame.z)
                        .abs()
                        < 1e-9;
                    if parallel && coincident {
                        self.kef(e)?;
                        return Ok(());
                    }
                }
            }
        }
        Err(TopoError::Precondition(
            "delete_face: no coplanar neighbour to merge into (extend-reintersect heal deferred)",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 80: surface/face geometry DEFORMATION as the composed
    /// tweak/offset/taper workflow (the capability-map row is the
    /// workflow over items 35-39/78). One body, deformations in
    /// sequence, exact volume at each step, valid + mass == mesh
    /// throughout.
    #[test]
    fn deformation_workflow_composes_tweaks() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap(); // vol 8
        // 1. Offset the top outward by 1: vol 12.
        let top = top_face(&b);
        b.offset_face(top, 1.0).unwrap();
        assert!(b.validate().is_ok());
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 12.0).abs() < 1e-9, "after offset: {v}");
        // 2. Move the top up another 1: vol 16.
        let top = top_face(&b);
        b.move_face(top, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert!(b.validate().is_ok());
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 16.0).abs() < 1e-9, "after move: {v}");
        // 3. Taper the +x side inward about its bottom edge: the body
        // stays valid and self-consistent and loses a wedge.
        let side = {
            let mut best = (f64::NEG_INFINITY, None);
            for f in b.face_keys() {
                if let Some(vs) = b.face_loop_vertices(f) {
                    let n = vs.len() as f64;
                    let x: f64 = vs
                        .iter()
                        .filter_map(|&v| b.vertices.get(v))
                        .map(|p| p.point.x)
                        .sum::<f64>()
                        / n;
                    if x > best.0 {
                        best = (x, Some(f));
                    }
                }
            }
            best.1.unwrap()
        };
        // Negative angle = lean inward about the pivot edge: the 2x2x4
        // prism loses a wedge of 0.5 * 4 * (4 * 0.25) * ... = 4.
        b.taper_face(
            side,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            -(0.25_f64.atan()),
        )
        .unwrap();
        assert!(b.validate().is_ok(), "after taper: {:?}", b.validate());
        let v = b.mass_properties().unwrap().volume;
        let mv = b.mesh_volume();
        assert!(
            (v - mv).abs() < 1e-9,
            "deformed body must stay mass == mesh ({v} vs {mv})"
        );
        assert!(
            (v - 12.0).abs() < 1e-9,
            "inward taper must remove the 4-wedge (got {v})"
        );
    }

    /// Item 140 (thread-safety half): a Body is Send + Sync by
    /// construction (owned data, no interior mutability), so concurrent
    /// READS are safe; mutation requires &mut (exclusive) by the borrow
    /// checker. SMP-parallel algorithms are the opportunistic half and
    /// remain follow-ups.
    #[test]
    fn concurrent_queries_are_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Body>();
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        let b = &b;
        std::thread::scope(|s| {
            let h1 = s.spawn(move || b.mass_properties().unwrap().volume);
            let h2 = s.spawn(move || b.mesh_volume());
            let h3 = s.spawn(move || b.facets(None).len());
            let h4 = s.spawn(move || b.surface_area());
            assert!((h1.join().unwrap() - 24.0).abs() < 1e-9);
            assert!((h2.join().unwrap() - 24.0).abs() < 1e-9);
            assert!(h3.join().unwrap() >= 12);
            assert!((h4.join().unwrap() - 52.0).abs() < 1e-9);
        });
    }

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

    #[test]
    fn taper_face_tilts_and_revalidates() {
        // Draft the top face of a 2x2x2 block about its y=0 top edge by
        // ~0.2 rad: corners on that edge stay, the others tilt down; the
        // solid stays valid and the volume changes.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let top = top_face(&b);
        b.taper_face(top, Vec3::new(0.0, 0.0, 2.0), Vec3::new(1.0, 0.0, 0.0), 0.2)
            .unwrap();
        assert!(b.validate().is_ok(), "tapered block invalid");
        let v = b.mass_properties().unwrap().volume;
        assert!(
            v.is_finite() && v > 0.0 && (v - 8.0).abs() > 1e-6,
            "taper volume {v} unchanged/invalid"
        );
    }

    #[test]
    fn offset_body_grows_block() {
        // A 2x2x2 block offset outward by 0.5 -> a 3x3x3 block (vol 27).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        b.offset_body(0.5).unwrap();
        assert!(b.validate().is_ok(), "offset body invalid");
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 27.0).abs() < 1e-9, "offset-body volume {v} != 27");
    }

    #[test]
    fn delete_face_merges_coplanar_split() {
        // Split a block's top face into two coplanar halves, then delete
        // one: it merges back (kef), restoring the original face count and
        // volume. (The merge/absorb heal mode of delete-face.)
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let f0 = b.counts().f;
        let top = top_face(&b);
        let (sk, sense) = b.faces.get(top).unwrap().surface.unwrap();
        // Collect the top loop's fins, split along a diagonal.
        let lp = b.faces.get(top).unwrap().loops[0];
        let first = b.loops.get(lp).unwrap().fin.unwrap();
        let mut fins = vec![first];
        let mut fk = b.fins.get(first).unwrap().next;
        while fk != first {
            fins.push(fk);
            fk = b.fins.get(fk).unwrap().next;
        }
        assert!(fins.len() >= 4, "top should be a quad");
        let split = b.split_face(fins[0], fins[2], Some((sk, sense))).unwrap();
        assert_eq!(b.counts().f, f0 + 1, "split should add a face");
        b.delete_face(split.face_new).unwrap();
        assert_eq!(b.counts().f, f0, "delete-merge should restore the count");
        assert!(b.validate().is_ok(), "merged body invalid");
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 8.0).abs() < 1e-9, "volume {v} != 8");
    }

    #[test]
    fn offset_cylinder_face_changes_radius() {
        // CURVED tweak: a cylinder r=1 h=2 (vol 2*pi); offsetting its
        // lateral face out by 0.5 -> r=1.5 (vol 4.5*pi), re-intersecting
        // the cap circles, staying valid.
        use keel_geom::surface::Frame3;
        let mut b = Body::new();
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        b.cylinder(frame, 1.0, 2.0).unwrap();
        let lateral = b
            .face_keys()
            .into_iter()
            .find(|&f| {
                matches!(
                    b.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                )
            })
            .unwrap();
        b.offset_face(lateral, 0.5).unwrap();
        assert!(b.validate().is_ok(), "tweaked cylinder invalid");
        let v = b.tessellated_volume();
        let expect = core::f64::consts::PI * 1.5 * 1.5 * 2.0;
        assert!(
            (v - expect).abs() < 0.05 * expect,
            "cyl vol {v} vs {expect}"
        );
    }
}
