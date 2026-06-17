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
use crate::entity::{EdgeKey, FaceKey, FinKey, SurfaceGeom, VertexKey};
use crate::lineage::Derivation;
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
    /// PUSH/PULL (task 37, the Plasticity-signature interaction): sweep
    /// a planar `face` along its OUTWARD normal by `d` (negative =
    /// inward pocket), CREATING the side walls: the op offset_face
    /// cannot be, because an imprinted region's neighbours are COPLANAR
    /// and give it no walls to re-solve. Euler mechanics mirror the
    /// prism construction (one mev per boundary vertex, one mef per
    /// boundary edge), in place on the existing body; wall planes come
    /// from each wall's own Newell normal (the Euler construction
    /// guarantees consistent winding). Slice scope: single-loop planar
    /// faces with straight boundary edges; the caller bounds `d` (no
    /// self-intersection guard, the tweak-family contract).
    pub fn push_face(&mut self, face: FaceKey, d: f64) -> Result<(), TopoError> {
        if !(d.is_finite() && d != 0.0) {
            return Err(TopoError::Precondition("push: bad distance"));
        }
        let (pl, sense) = self
            .face_plane(face)
            .ok_or(TopoError::Precondition("push: non-planar face"))?;
        let n_out = if sense { pl.frame.z } else { pl.frame.z * -1.0 };
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .ok_or(TopoError::StaleKey)?;
        if loops.len() != 1 {
            return Err(TopoError::Precondition(
                "push: ring-carrying face (follow-up)",
            ));
        }
        let lp = loops[0];
        // Ordered boundary vertices; straight edges only (the slice).
        let entry = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("push: face has no boundary"))?;
        let mut vs: Vec<VertexKey> = Vec::new();
        let mut cur = entry;
        loop {
            let fin = self.fins.get(cur).ok_or(TopoError::StaleKey)?;
            let straight = matches!(
                self.edges
                    .get(fin.edge)
                    .and_then(|e| e.curve)
                    .and_then(|(ck, _)| self.curves.get(ck)),
                None | Some(Curve3::Line(_))
            );
            if !straight {
                return Err(TopoError::Precondition("push: curved boundary (follow-up)"));
            }
            if let Some(v) = self.fin_start_vertex(cur)
                && !vs.contains(&v)
            {
                vs.push(v);
            }
            cur = fin.next;
            if cur == entry {
                break;
            }
        }
        let n = vs.len();
        if n < 3 {
            return Err(TopoError::Precondition("push: degenerate boundary"));
        }
        let lift = n_out * d;
        // One mev per vertex (the lifted rim), then one mef per original
        // boundary edge (the walls); the face itself becomes the moved cap.
        let mut tops: Vec<VertexKey> = Vec::new();
        let mut new_edges: Vec<EdgeKey> = Vec::new();
        for &v in &vs {
            let at = self.fin_ending_at(lp, v)?;
            let p = self
                .vertices
                .get(v)
                .map(|x| x.point + lift)
                .ok_or(TopoError::StaleKey)?;
            let m = self.mev(crate::euler::MevSite::AfterFin(at), p)?;
            tops.push(m.vertex);
            new_edges.push(m.edge);
        }
        let mut walls: Vec<FaceKey> = Vec::new();
        for i in 0..n {
            let a = self.fin_ending_at(lp, tops[i])?;
            let c = self.fin_ending_at(lp, tops[(i + 1) % n])?;
            let mef = self.mef(a, c, None)?;
            walls.push(mef.face);
            new_edges.push(mef.edge);
        }
        // Geometry: the cap re-seats on the offset plane (sense kept).
        let cap_frame = Frame3 {
            origin: pl.frame.origin + lift,
            x: pl.frame.x,
            y: pl.frame.y,
            z: pl.frame.z,
        };
        self.attach_face_surface(
            face,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(cap_frame))),
            sense,
        );
        // Walls: plane from each wall's own loop (Newell), sense true.
        for w in walls {
            let pts = self.face_outer_loop_points(w);
            let m = pts.len();
            if m < 3 {
                continue;
            }
            let mut nw = Vec3::ZERO;
            for i in 0..m {
                let a = pts[i];
                let b = pts[(i + 1) % m];
                nw = nw
                    + Vec3::new(
                        (a.y - b.y) * (a.z + b.z),
                        (a.z - b.z) * (a.x + b.x),
                        (a.x - b.x) * (a.y + b.y),
                    );
            }
            let Some(nw) = nw.try_normalize() else {
                continue;
            };
            if let Ok(fr) = Frame3::from_z(pts[0], nw) {
                self.attach_face_surface(
                    w,
                    SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(fr))),
                    true,
                );
            }
        }
        // New edges are straight: attach their lines.
        for e in new_edges {
            let Some(ed) = self.edges.get(e) else {
                continue;
            };
            let (Some(p0), Some(p1)) = (
                self.vertices.get(ed.bounds.0).map(|v| v.point),
                self.vertices.get(ed.bounds.1).map(|v| v.point),
            ) else {
                continue;
            };
            if let Ok(l) = Line3::new(p0, p1 - p0) {
                self.attach_edge_curve(e, Curve3::Line(l), true);
            }
        }
        self.debug_validate();
        Ok(())
    }

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
    pub(crate) fn face_loop_edges(&self, face: FaceKey) -> Option<Vec<EdgeKey>> {
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

    /// DRAFT a planar face (swap task 24; the mold-release taper,
    /// parity items 38/78 deferred from the tweak slices): tilt the
    /// face's plane by `angle` about its HINGE line, the intersection
    /// of the face plane with the `neutral` plane, then re-solve the
    /// boundary through the unchanged neighbours (tweak_face_to_plane).
    /// The hinge stays exactly put: geometry on the neutral plane is
    /// invariant, the face leans by `angle`.
    ///
    /// Sign convention: a POSITIVE angle rotates the outward normal
    /// about `axis = normalize(n_outward x n_neutral)` by `angle`,
    /// which tilts the wall so material is REMOVED on the far side of
    /// the neutral plane (the standard pull-direction draft when
    /// `neutral.frame.z` is the pull direction).
    ///
    /// Declines (never wrong): non-planar faces, faces parallel to the
    /// neutral plane (no hinge), and every tweak_face_to_plane
    /// precondition (non-planar neighbours, non-simple corners).
    pub fn draft_face(
        &mut self,
        face: FaceKey,
        neutral: &Plane3,
        angle: f64,
    ) -> Result<(), TopoError> {
        if !angle.is_finite() || angle.abs() >= core::f64::consts::FRAC_PI_2 {
            return Err(TopoError::Precondition("draft: bad angle"));
        }
        let (pl, sense) = self
            .face_plane(face)
            .ok_or(TopoError::Precondition("draft: face not planar"))?;
        let nw = self
            .face_outward_normal(face)
            .ok_or(TopoError::Precondition("draft: no outward normal"))?;
        let np = neutral.frame.z;
        let axis = nw
            .cross(np)
            .try_normalize()
            .ok_or(TopoError::Precondition("draft: face parallel to neutral"))?;
        // A point on the hinge line (face plane ^ neutral plane):
        // solve the 2-plane system, third constraint along the hinge.
        let d_f = nw.dot(pl.frame.origin);
        let d_n = np.dot(neutral.frame.origin);
        let h = three_plane_point(nw, d_f, np, d_n, axis, 0.0)
            .or_else(|| three_plane_point(nw, d_f, np, d_n, axis, 1.0))
            .ok_or(TopoError::Precondition("draft: degenerate hinge"))?;
        // Rodrigues rotation of the outward normal about the hinge axis.
        let (s, c) = angle.sin_cos();
        let rot = |v: Vec3| v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c));
        let nw_new = rot(nw);
        // The tilted plane passes through the hinge with the rotated
        // normal; preserve the face's plane-vs-outward sense.
        let frame_z = if sense { nw_new } else { nw_new * -1.0 };
        let new_plane = Plane3::new(
            keel_geom::surface::Frame3::from_z(h, frame_z)
                .map_err(|_| TopoError::Precondition("draft: degenerate tilted frame"))?,
        );
        self.tweak_face_to_plane(face, new_plane, sense)
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

    /// UNTRIM a face: discard a trimmed analytic face's outer loop and
    /// rebuild the body that the surface's NATURAL full boundary bounds.
    /// A boolean may leave a cylinder's lateral face carrying an
    /// arbitrary notched loop; untrim recovers the full tube band the
    /// raw surface defines over the face's current axial extent (a full
    /// 2*pi revolution, seam + two rim circles).
    ///
    /// MVP scope (cylinder lateral faces): the result is a fresh full
    /// cylinder SOLID spanning [v_min, v_max], where v_min/v_max are the
    /// axial coordinates of the face's current boundary vertices along
    /// the cylinder axis. Its lateral face is, by construction, the
    /// natural full boundary (area exactly 2*pi*r*(v_max - v_min)); the
    /// body is valid and mass == mesh. Rebuilding (rather than splicing
    /// the natural loop into the existing notched topology) is what makes
    /// the validity guarantee unconditional.
    ///
    /// Declines (never wrong): planes (a plane has no natural finite
    /// bound), and -- as deferred follow-ups -- spheres, cones, tori, and
    /// NURBS faces. A degenerate axial extent (the boundary collapses to
    /// a single height) is also declined.
    pub fn untrim_face(&self, face: FaceKey) -> Result<Body, TopoError> {
        match self.face_surface_geom(face) {
            Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => {
                self.untrim_cylinder_lateral(face, c)
            }
            Some(SurfaceGeom::Analytic(Surface3::Plane(_))) => Err(TopoError::Precondition(
                "untrim_face: plane has no natural finite bound",
            )),
            Some(_) => Err(TopoError::Precondition(
                "untrim_face: only cylinder lateral faces (MVP)",
            )),
            None => Err(TopoError::Precondition("untrim_face: no surface")),
        }
    }

    /// Rebuild the full cylindrical band a lateral face's surface defines
    /// over that face's current axial extent (the cylinder MVP of
    /// `untrim_face`).
    fn untrim_cylinder_lateral(&self, face: FaceKey, cyl: Cylinder3) -> Result<Body, TopoError> {
        let (origin, axis) = (cyl.frame.origin, cyl.frame.z);
        // Axial extent = span of the face's boundary vertices projected
        // onto the cylinder axis. The loop vertices are the trimmed
        // boundary; the natural band spans their full height range.
        let verts = self
            .face_loop_vertices(face)
            .ok_or(TopoError::Precondition("untrim_face: face has no loop"))?;
        let mut v_min = f64::INFINITY;
        let mut v_max = f64::NEG_INFINITY;
        for &v in &verts {
            let p = self.vertices.get(v).ok_or(TopoError::StaleKey)?.point;
            let t = (p - origin).dot(axis);
            v_min = v_min.min(t);
            v_max = v_max.max(t);
        }
        let height = v_max - v_min;
        if !(height.is_finite() && height > 1e-9) {
            return Err(TopoError::Precondition(
                "untrim_face: degenerate axial extent",
            ));
        }
        // Fresh full cylinder over [v_min, v_max]: same axis + radius,
        // frame translated so its base disc sits at v_min. Its lateral
        // face is the surface's natural full boundary by construction.
        let base = Frame3 {
            origin: origin + axis * v_min,
            x: cyl.frame.x,
            y: cyl.frame.y,
            z: cyl.frame.z,
        };
        let mut out = Body::new();
        out.cylinder(base, cyl.radius, height)?;
        out.debug_validate();
        Ok(out)
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

    /// Remove REDUNDANT topology introduced by imprints/booleans without
    /// changing the geometry (parity item 133, the global face-merge
    /// heal): repeatedly (1) merge any two adjacent COPLANAR faces that
    /// share an edge into one (killing the shared edge with `kef`), then
    /// (2) dissolve any valence-2 vertex whose two incident edges are
    /// COLLINEAR (the inverse of `split_edge`), so a face split into
    /// coplanar pieces and a boundary edge split into collinear pieces
    /// both collapse back. Geometry is untouched: a merge only fires when
    /// the two faces lie in the same plane and the two edges on the same
    /// line, so mass and mesh are invariant. Idempotent. Returns the
    /// number of (faces merged, vertices dissolved).
    ///
    /// Slice (the honest MVP): planar coplanar-face merges via `kef`
    /// (both faces single-loop on the shared edge -- the imprint/boolean
    /// scar case) and manifold valence-2 collinear-vertex dissolves.
    /// Curved-face coalescing (two arcs of one cylinder), ring-carrying
    /// faces, and non-manifold rims are DECLINED (skipped), never guessed:
    /// a `kef`/dissolve whose preconditions fail is transactional, so the
    /// body is left untouched at that site and the pass moves on.
    pub fn delete_redundant_topology(&mut self) -> Result<(usize, usize), TopoError> {
        let coincident_planes = |a: &Plane3, b: &Plane3| -> bool {
            a.frame.z.cross(b.frame.z).norm() < 1e-9
                && (b.frame.origin - a.frame.origin).dot(a.frame.z).abs() < 1e-9
        };

        // 1. Coalesce coplanar faces. Re-scan from scratch after each
        //    merge: `kef` deletes a face and an edge, invalidating the
        //    iteration, and a fresh scan keeps the logic obviously correct
        //    (the body shrinks by one face each pass, so this terminates).
        let mut faces_merged = 0usize;
        'merge: loop {
            for e in self.edges.iter().map(|(k, _)| k).collect::<Vec<_>>() {
                // Manifold shared edge between two DISTINCT planar faces.
                if self.edges.get(e).map(|x| x.radial.len()) != Some(2) {
                    continue;
                }
                let nbrs = self.faces_around_edge(e);
                if nbrs.len() != 2 {
                    continue;
                }
                let (fa, fb) = (nbrs[0], nbrs[1]);
                let (Some((pa, _)), Some((pb, _))) =
                    (self.face_plane(fa), self.face_plane(fb))
                else {
                    continue;
                };
                if !coincident_planes(&pa, &pb) {
                    continue;
                }
                // `kef` kills the face whose loop owns the edge's SECOND
                // radial fin and requires that face be single-loop; try as
                // is, and if it declines, flip the radial pair so the
                // single-loop face becomes the victim. Either failing
                // leaves the body untouched (transactional): skip the site.
                if self.kef(e).is_ok() {
                    faces_merged += 1;
                    continue 'merge;
                }
                if let Some(ed) = self.edges.get_mut(e) {
                    ed.radial.reverse();
                }
                if self.kef(e).is_ok() {
                    faces_merged += 1;
                    continue 'merge;
                }
                // Restore the radial order we perturbed (the merge was not
                // applicable from either side).
                if let Some(ed) = self.edges.get_mut(e) {
                    ed.radial.reverse();
                }
            }
            break;
        }

        // 2. Dissolve valence-2 collinear vertices (merge the two edges).
        let mut verts_dissolved = 0usize;
        'dissolve: loop {
            for v in self.vertices.iter().map(|(k, _)| k).collect::<Vec<_>>() {
                if self.merge_collinear_edges_at(v).unwrap_or(false) {
                    verts_dissolved += 1;
                    continue 'dissolve;
                }
            }
            break;
        }
        self.debug_validate();
        Ok((faces_merged, verts_dissolved))
    }

    /// If `v` is a manifold valence-2 vertex whose two incident edges are
    /// COLLINEAR, fuse them into one edge spanning the far endpoints (the
    /// inverse of `split_edge`) and return `Ok(true)`. `Ok(false)` when
    /// the vertex does not qualify (wrong valence, non-collinear, closed
    /// edge, non-manifold, or a vertex-loop anchor): the body is left
    /// untouched. The merged edge inherits a straight Line carrier when
    /// both originals were straight/curveless (the planar-scar case).
    fn merge_collinear_edges_at(&mut self, v: VertexKey) -> Result<bool, TopoError> {
        // Manifold only: a vertex with PES groups is non-manifold.
        if self.vertices.get(v).map(|x| !x.groups.is_empty()) != Some(false) {
            return Ok(false);
        }
        // Not a vertex-loop anchor (an isolated vertex inside a face).
        if self
            .loops
            .iter()
            .any(|(_, l)| l.vertex == Some(v))
        {
            return Ok(false);
        }
        let inc = self.edges_of_vertex(v);
        if inc.len() != 2 {
            return Ok(false);
        }
        let (e1, e2) = (inc[0], inc[1]);
        // Both edges must be open (no seam) and manifold radial-2.
        let (b1, b2) = match (self.edges.get(e1), self.edges.get(e2)) {
            (Some(x), Some(y)) => (x.bounds, y.bounds),
            _ => return Ok(false),
        };
        if b1.0 == b1.1 || b2.0 == b2.1 {
            return Ok(false);
        }
        if self.edges.get(e1).map(|x| x.radial.len()) != Some(2)
            || self.edges.get(e2).map(|x| x.radial.len()) != Some(2)
        {
            return Ok(false);
        }
        // Far endpoints (the vertices that are not v), and they must be
        // distinct from each other (else the pair is a degenerate needle).
        let a = if b1.0 == v { b1.1 } else { b1.0 };
        let b = if b2.0 == v { b2.1 } else { b2.0 };
        if a == v || b == v || a == b {
            return Ok(false);
        }
        let (pa, pv, pb) = match (
            self.vertices.get(a).map(|x| x.point),
            self.vertices.get(v).map(|x| x.point),
            self.vertices.get(b).map(|x| x.point),
        ) {
            (Some(x), Some(y), Some(z)) => (x, y, z),
            _ => return Ok(false),
        };
        // Collinearity: a->v parallel to v->b, same direction (the chain
        // continues straight through v, not a spike doubling back).
        let d1 = pv - pa;
        let d2 = pb - pv;
        let (n1, n2) = (d1.norm(), d2.norm());
        if n1 < 1e-12 || n2 < 1e-12 {
            return Ok(false);
        }
        let scale = n1.max(n2);
        if d1.cross(d2).norm() > 1e-9 * scale * scale || d1.dot(d2) <= 0.0 {
            return Ok(false);
        }

        // Both edges straight/curveless -> the merged edge is the line
        // a->b; otherwise keep it curveless (curved coalescing is out of
        // slice and would need a fitted carrier).
        let straight = |body: &Self, e: EdgeKey| -> bool {
            matches!(
                body.edges
                    .get(e)
                    .and_then(|x| x.curve)
                    .and_then(|(ck, _)| body.curves.get(ck)),
                None | Some(Curve3::Line(_))
            )
        };
        let line_carrier = straight(self, e1) && straight(self, e2);

        // Pair up the four fins by loop. e1 has two fins; for each, the
        // matching e2 fin is its ring neighbour at v. Build (f_in, f_out)
        // per loop, where f_in ends at v and f_out starts at v, with
        // f_in.next == f_out.
        let e1_fins = self
            .edges
            .get(e1)
            .map(|x| x.radial.clone())
            .unwrap_or_default();
        let mut pairs: Vec<(FinKey, FinKey)> = Vec::new();
        for x in e1_fins {
            let pair = if self.fin_end_vertex(x) == Some(v) {
                // x ends at v; its successor leaves v on e2.
                let nx = self.fins.get(x).map(|f| f.next);
                nx.map(|nx| (x, nx))
            } else if self.fin_start_vertex(x) == Some(v) {
                // x starts at v; its predecessor arrives at v on e2.
                let px = self.fins.get(x).map(|f| f.prev);
                px.map(|px| (px, x))
            } else {
                None
            };
            let Some((f_in, f_out)) = pair else {
                return Ok(false);
            };
            // The pair must be two distinct fins in ONE ring spanning the
            // two edges (one on e1, one on e2 -- in EITHER order, since
            // which of f_in/f_out lands on e1 depends on the loop's sense).
            let edges = (
                self.fins.get(f_in).map(|f| f.edge),
                self.fins.get(f_out).map(|f| f.edge),
            );
            let spans_both = edges == (Some(e1), Some(e2)) || edges == (Some(e2), Some(e1));
            if !spans_both
                || f_in == f_out
                || self.fins.get(f_in).map(|f| f.owner) != self.fins.get(f_out).map(|f| f.owner)
            {
                return Ok(false);
            }
            pairs.push((f_in, f_out));
        }
        if pairs.len() != 2 {
            return Ok(false);
        }

        // Commit point: preconditions hold; the surgery is infallible.
        let (id1, id2) = (
            self.edges.get(e1).map(|x| x.id),
            self.edges.get(e2).map(|x| x.id),
        );
        let mut rec = self.begin_op();
        let derivation = match (id1, id2) {
            (Some(a), Some(b)) => Derivation::MergeResult { from: vec![a, b] },
            _ => Derivation::Created,
        };
        let e_new = self.new_edge(&mut rec, (a, b), derivation);
        let mut new_fins: Vec<FinKey> = Vec::new();
        for &(f_in, f_out) in &pairs {
            let owner = self.fins.get(f_in).map(|f| f.owner).ok_or(TopoError::StaleKey)?;
            // The merged fin traverses start(f_in) -> end(f_out). It is
            // FORWARD on e_new (a -> b) iff start(f_in) == a.
            let forward = self.fin_start_vertex(f_in) == Some(a);
            let g = self.new_fin(&mut rec, e_new, forward, owner, Derivation::Created);
            let in_prev = self.fins.get(f_in).map(|f| f.prev).ok_or(TopoError::StaleKey)?;
            let out_next = self.fins.get(f_out).map(|f| f.next).ok_or(TopoError::StaleKey)?;
            // Splice g where the pair sat: in_prev -> g -> out_next.
            if let Some(x) = self.fins.get_mut(in_prev) {
                x.next = g;
            }
            if let Some(x) = self.fins.get_mut(g) {
                x.prev = in_prev;
                x.next = out_next;
            }
            if let Some(x) = self.fins.get_mut(out_next) {
                x.prev = g;
            }
            // Loop entry off the dying fins.
            if let Some(l) = self.loops.get_mut(owner)
                && (l.fin == Some(f_in) || l.fin == Some(f_out))
            {
                l.fin = Some(g);
            }
            new_fins.push(g);
        }
        if let Some(ed) = self.edges.get_mut(e_new) {
            ed.radial = new_fins.clone();
        }
        // Re-anchor the far vertices' fin references off any dying fin, and
        // give the (now fin-less) merged-away vertex v no anchor.
        let dead: Vec<FinKey> = pairs.iter().flat_map(|&(i, o)| [i, o]).collect();
        for &fv in &[a, b] {
            let needs = self
                .vertices
                .get(fv)
                .and_then(|x| x.fin)
                .map(|fk| dead.contains(&fk))
                .unwrap_or(false);
            if needs {
                // Pick a surviving fin incident to fv: a new merged fin, or
                // any radial fin of fv's other edges.
                let repl = new_fins
                    .iter()
                    .copied()
                    .find(|&g| self.fin_start_vertex(g) == Some(fv) || self.fin_end_vertex(g) == Some(fv))
                    .or_else(|| {
                        self.edges_of_vertex(fv).into_iter().find_map(|ek| {
                            self.edges
                                .get(ek)
                                .and_then(|e| e.radial.iter().copied().find(|g| !dead.contains(g)))
                        })
                    });
                if let Some(x) = self.vertices.get_mut(fv) {
                    x.fin = repl;
                }
            }
        }
        // Tear down the two old edges, their four fins, and vertex v.
        for &(f_in, f_out) in &pairs {
            for fk in [f_in, f_out] {
                if let Some(id) = self.fins.get(fk).map(|x| x.id) {
                    self.unregister(&mut rec, id);
                }
                self.fins.remove(fk);
            }
        }
        for ek in [e1, e2] {
            if let Some(id) = self.edges.get(ek).map(|x| x.id) {
                self.unregister(&mut rec, id);
            }
            self.edges.remove(ek);
        }
        if let Some(id) = self.vertices.get(v).map(|x| x.id) {
            self.unregister(&mut rec, id);
        }
        self.vertices.remove(v);
        let _ = rec.finish();
        // Attach the straight carrier now that e_new is wired in.
        if line_carrier
            && let Ok(l) = Line3::new(pa, pb - pa)
        {
            self.attach_edge_curve(e_new, Curve3::Line(l), true);
        }
        self.debug_validate();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Swap task 24: face draft about a neutral plane, the exact
    /// oracle. Block 2x2x2, draft the +x wall by atan(1/4) about the
    /// bottom plane (pull +z): the hinge (bottom edge) stays put, the
    /// wall leans inward, and the volume is EXACTLY
    /// 2 * integral_0^2 (2 - z/4) dz = 7. The negative angle leans
    /// outward: exactly 9. Both mass == mesh at 1e-9, valid.
    #[test]
    fn draft_face_exact_volumes() {
        use keel_geom::surface::Frame3;
        let neutral = Plane3::new(Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap());
        let angle = 0.25f64.atan();
        for (sign, expect) in [(1.0, 7.0), (-1.0, 9.0)] {
            let mut b = Body::new();
            b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
            let face = b
                .face_keys()
                .into_iter()
                .find(|&f| {
                    b.face_outward_normal(f)
                        .is_some_and(|n| (n - Vec3::new(1.0, 0.0, 0.0)).norm() < 1e-9)
                })
                .expect("+x face");
            b.draft_face(face, &neutral, sign * angle).unwrap();
            assert!(b.validate().is_ok(), "draft invalid: {:?}", b.validate());
            let v = b.mass_properties().unwrap().volume;
            assert!(
                (v - expect).abs() < 1e-9 * (1.0 + expect),
                "draft sign {sign}: {v} != {expect}"
            );
            let mesh = b.mesh_volume();
            assert!(
                (mesh - v).abs() < 1e-9 * (1.0 + v),
                "draft sign {sign}: mass {v} != mesh {mesh}"
            );
        }
        // The decline rails: a face PARALLEL to the neutral plane has
        // no hinge and must decline, not guess.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let top = b
            .face_keys()
            .into_iter()
            .find(|&f| {
                b.face_outward_normal(f)
                    .is_some_and(|n| (n - Vec3::new(0.0, 0.0, 1.0)).norm() < 1e-9)
            })
            .expect("+z face");
        assert!(b.draft_face(top, &neutral, angle).is_err());
    }

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

    #[test]
    fn untrim_cylinder_lateral_restores_full_band() {
        // Build a cylinder r=1 h=2 (axis +z), then drill a thin cross-bore
        // (a perpendicular r=0.4 cylinder) through its wall: the main
        // cylinder's lateral face is now TRIMMED -- it carries an inner
        // ring (the bore hole bitten out of the side wall), so its boundary
        // is no longer the surface's full natural boundary. untrim_face
        // discards that trimmed boundary and rebuilds the band the raw
        // cylinder defines over the face's axial extent [0, 2]: a full
        // 2*pi tube of area 2*pi*r*h = 4*pi, no hole, valid, mass == mesh.
        use crate::boolean::{BoolOp, boolean};
        use keel_geom::surface::Frame3;
        let r = 1.0;
        let h = 2.0;
        let mut main = Body::new();
        main.cylinder(Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap(), r, h)
            .unwrap();
        let mut bore = Body::new();
        bore.cylinder(
            Frame3::from_z(Vec3::new(-2.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0)).unwrap(),
            0.4,
            4.0,
        )
        .unwrap();
        let trimmed = boolean(&main, &bore, BoolOp::Difference, 1e-7)
            .unwrap()
            .body;
        assert!(trimmed.validate().is_ok(), "cross-bored cylinder invalid");

        // Helper: the radius of a face's cylinder surface (None if not a
        // cylinder) -- used to pick the MAIN wall (r=1), not the bore (0.4).
        let cyl_radius = |b: &Body, f: FaceKey| match b.face_surface_geom(f) {
            Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => Some(c.radius),
            _ => None,
        };
        let lateral = trimmed
            .face_keys()
            .into_iter()
            .find(|&f| cyl_radius(&trimmed, f).is_some_and(|cr| (cr - r).abs() < 1e-6))
            .expect("main lateral cylinder face");
        // The bore really did trim this wall: it carries an inner ring.
        let nloops = trimmed.faces.get(lateral).map(|f| f.loops.len()).unwrap();
        assert!(
            nloops >= 2,
            "expected the main lateral to carry the bore hole (an inner ring), got {nloops} loop(s)"
        );

        let restored = trimmed.untrim_face(lateral).unwrap();
        assert!(
            restored.validate().is_ok(),
            "untrimmed band invalid: {:?}",
            restored.validate()
        );
        // The restored lateral is the full natural band: area = 2*pi*r*h,
        // and it carries a single (outer) loop -- the hole is gone.
        let lat = restored
            .face_keys()
            .into_iter()
            .find(|&f| cyl_radius(&restored, f).is_some_and(|cr| (cr - r).abs() < 1e-6))
            .unwrap();
        assert_eq!(
            restored.faces.get(lat).map(|f| f.loops.len()),
            Some(1),
            "restored lateral must be the single full-boundary loop"
        );
        let area = restored.face_area(lat);
        let want = core::f64::consts::TAU * r * h;
        assert!(
            (area - want).abs() < 1e-6 * want,
            "untrimmed lateral area {area} != 2*pi*r*h {want}"
        );
        // Self-consistency gate: mass == mesh on the rebuilt solid (it is a
        // plain cylinder of volume pi*r^2*h = 2*pi).
        let mass = restored.mass_properties().unwrap().volume;
        let mesh = restored.mesh_volume();
        assert!(
            (mass - mesh).abs() < 0.02 * mass.max(1.0),
            "untrimmed band mass {mass} != mesh {mesh}"
        );
        assert!(
            (mass - core::f64::consts::PI * r * r * h).abs() < 1e-6,
            "restored solid volume {mass} != pi*r^2*h"
        );

        // Decline rail (never wrong): a plane has no natural finite bound.
        let plane_face = restored
            .face_keys()
            .into_iter()
            .find(|&f| {
                matches!(
                    restored.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
                )
            })
            .unwrap();
        assert!(restored.untrim_face(plane_face).is_err());
    }

    #[test]
    fn delete_redundant_topology_undoes_an_imprint() {
        // Block 2x2x2; imprint a straight line across its top face from
        // one boundary edge to the opposite one. That splits the top into
        // two coplanar quads (7 faces) AND splits the two crossed boundary
        // edges into collinear halves (valence-2 scar vertices).
        // delete_redundant_topology must collapse all of it: back to the
        // pristine cube -- 6 faces, 12 edges, 8 vertices, volume 8, valid,
        // mass == mesh. And it must be idempotent.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let c0 = b.counts();
        assert_eq!((c0.v, c0.e, c0.f), (8, 12, 6), "pristine cube counts");

        let top = top_face(&b);
        // Midpoints of two OPPOSITE top edges (y=0 and y=2 at z=2).
        b.imprint_open_polyline(
            top,
            &[Vec3::new(1.0, 0.0, 2.0), Vec3::new(1.0, 2.0, 2.0)],
            1e-7,
        )
        .unwrap();
        assert!(b.validate().is_ok(), "imprinted body invalid");
        let c1 = b.counts();
        assert!(c1.f > 6, "imprint must add a face (got {} faces)", c1.f);
        assert_eq!(c1.f, 7, "one straight imprint -> 7 faces");
        assert_eq!(c1.v, 10, "two boundary splits -> 10 vertices");
        let v_imprint = b.mass_properties().unwrap().volume;
        assert!((v_imprint - 8.0).abs() < 1e-9, "imprint changed volume");

        let (merged, dissolved) = b.delete_redundant_topology().unwrap();
        assert_eq!(merged, 1, "the two coplanar top halves merge (1 kef)");
        assert_eq!(dissolved, 2, "the two collinear scar vertices dissolve");
        assert!(b.validate().is_ok(), "cleaned body invalid: {:?}", b.validate());
        let c2 = b.counts();
        assert_eq!(
            (c2.v, c2.e, c2.f),
            (8, 12, 6),
            "redundancy removal restores the pristine cube"
        );
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 8.0).abs() < 1e-9, "volume {v} != 8 after cleanup");
        let mesh = b.mesh_volume();
        assert!(
            (mesh - v).abs() < 1e-9 * (1.0 + v),
            "mass {v} != mesh {mesh}"
        );

        // Idempotence: a clean body has nothing left to remove.
        let (m2, d2) = b.delete_redundant_topology().unwrap();
        assert_eq!((m2, d2), (0, 0), "second pass is a no-op");
        assert_eq!(b.counts(), c2, "idempotent counts");
    }
}
