//! Edge blend / fillet surface generation (parity items 47-61), the
//! geometry stage of research file 40. This module implements the
//! centerpiece procedure for the highest-value rung of the exact-analytic
//! ladder: a constant-radius rolling-ball fillet on a CONVEX edge between
//! two PLANAR faces, whose spine is a line and whose blend surface is an
//! exact native cylinder.
//!
//! File 40 §1.2 (the single most important procedural fact):
//!   spine = SSI( offset(S_a, r), offset(S_b, r) ).
//! For two planes, each offset is a parallel plane and their intersection
//! is a straight line parallel to the sharp edge; the envelope of equal
//! spheres of radius r centred on that line is a cylinder of radius r
//! about it (§1.4 Case A). The two spring curves are the feet of the
//! perpendiculars from the spine onto each plane (§1.3) -- the spine
//! translated by +r*n -- and the cylinder is tangent to each support
//! plane along its spring line (G1 by construction).
//!
//! This file delivers and certifies that geometry. The trim-and-stitch
//! local operation that inserts the blend face into the body (file 40
//! §3) is the next stage; circle-spine tori, cyclides, and NURBS canals
//! (the higher rungs) are follow-ups.

use crate::Body;
use crate::body::TopoError;
use crate::entity::{EdgeKey, SurfaceGeom};
use keel_geom::curve::{Curve3, Line3};
use keel_geom::surface::{Cylinder3, Frame3, Surface3};
use keel_math::vec::Vec3;

/// The analytic data of a constant-radius rolling-ball fillet on a
/// plane-plane edge: the spine line (ball-centre locus), the two spring
/// lines (tangency loci on each support plane), and the exact cylinder
/// blend surface (axis = spine, radius = r).
#[derive(Clone, Debug)]
pub struct EdgeBlend {
    /// Locus of the rolling-ball centre (the cylinder axis).
    pub spine: Line3,
    /// Tangency line on the first support plane.
    pub spring_a: Line3,
    /// Tangency line on the second support plane.
    pub spring_b: Line3,
    /// The exact blend surface: a cylinder of radius `r` about the spine.
    pub surface: Cylinder3,
}

impl Body {
    /// Is `edge` convex (the material occupies the small dihedral) or
    /// concave/reentrant? Probes the generalized winding number just off
    /// the edge along the in-face bisector (the direction into both
    /// faces' interiors): inside the material means convex. Returns None
    /// if the edge is not a clean two-face manifold edge.
    pub(crate) fn edge_is_convex(&self, edge: EdgeKey) -> Option<bool> {
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return None;
        }
        let (va, vb) = self.edges.get(edge)?.bounds;
        let pa = self.vertices.get(va)?.point;
        let pb = self.vertices.get(vb)?.point;
        let m = (pa + pb) * 0.5;
        let t = (pb - pa).try_normalize()?;
        let n0 = self.face_outward_normal(faces[0])?;
        let n1 = self.face_outward_normal(faces[1])?;
        let cen = |pts: &[Vec3]| -> Vec3 {
            if pts.is_empty() {
                Vec3::ZERO
            } else {
                pts.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / pts.len() as f64)
            }
        };
        let c0 = cen(&self.face_outer_loop_points(faces[0]));
        let c1 = cen(&self.face_outer_loop_points(faces[1]));
        // In-face directions into each face's interior (perp to the edge).
        let mut u0 = t.cross(n0);
        if (c0 - m).dot(u0) < 0.0 {
            u0 = u0 * -1.0;
        }
        let mut u1 = t.cross(n1);
        if (c1 - m).dot(u1) < 0.0 {
            u1 = u1 * -1.0;
        }
        let bis = (u0 + u1).try_normalize()?;
        let scale = (c0 - m).norm().min((c1 - m).norm()).max(1e-3);
        let probe = m + bis * (scale * 0.05);
        Some(self.generalized_winding_number(probe) > 0.5)
    }

    /// Generate the exact-cylinder blend surface for a convex `edge`
    /// between two planar faces, rolling a ball of `radius` r (file 40
    /// rung 1). Returns the spine, both spring lines, and the cylinder.
    /// Errors if the edge is not bounded by exactly two planar faces or
    /// the configuration is degenerate (parallel planes / r <= 0).
    ///
    /// This is the geometry stage only; it does not yet modify the body
    /// (the trim-and-stitch insertion is file 40 §3, a follow-up).
    pub fn blend_cylinder_for_edge(
        &self,
        edge: EdgeKey,
        radius: f64,
    ) -> Result<EdgeBlend, TopoError> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(TopoError::Precondition("blend: radius must be positive"));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("blend: edge needs two faces"));
        }
        // Both supports must be planar (rung 1).
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "blend: non-planar support (follow-up)",
                ));
            }
        }
        // Outward normals and a point on each support plane.
        let n1 = self
            .face_outward_normal(faces[0])
            .ok_or(TopoError::Precondition("blend: face normal"))?;
        let n2 = self
            .face_outward_normal(faces[1])
            .ok_or(TopoError::Precondition("blend: face normal"))?;
        let p1 = self
            .face_outer_loop_points(faces[0])
            .first()
            .copied()
            .ok_or(TopoError::Precondition("blend: face point"))?;
        let p2 = self
            .face_outer_loop_points(faces[1])
            .first()
            .copied()
            .ok_or(TopoError::Precondition("blend: face point"))?;

        // Offset each support plane by r: toward the material for a
        // convex edge ({n.x = n.p - r}, the ball sits in the material), or
        // away from it for a concave/reentrant edge (the ball fills the
        // notch). The spine is the intersection line of the two offsets.
        let convex = self
            .edge_is_convex(edge)
            .ok_or(TopoError::Precondition("blend: cannot determine convexity"))?;
        let off = if convex { -radius } else { radius };
        let d1 = n1.dot(p1) + off;
        let d2 = n2.dot(p2) + off;
        let dir = n1
            .cross(n2)
            .try_normalize()
            .ok_or(TopoError::Precondition("blend: parallel supports"))?;
        // Point on both offset planes nearest the origin (two-plane
        // intersection; n1, n2 unit so denom = 1 - cos^2 = sin^2).
        let c = n1.dot(n2);
        let denom = 1.0 - c * c;
        if denom.abs() < 1e-12 {
            return Err(TopoError::Precondition("blend: parallel supports"));
        }
        let spine_pt = (n1 * ((d1 - d2 * c) / denom)) + (n2 * ((d2 - d1 * c) / denom));
        let spine = Line3::new(spine_pt, dir)
            .map_err(|_| TopoError::Precondition("blend: degenerate spine"))?;

        // Spring lines: feet of perpendiculars from the spine onto each
        // support plane (the spine translated by +r toward the face).
        let foot = |p_plane: Vec3, n: Vec3| spine_pt - n * ((spine_pt - p_plane).dot(n));
        let spring_a = Line3::new(foot(p1, n1), dir)
            .map_err(|_| TopoError::Precondition("blend: degenerate spring"))?;
        let spring_b = Line3::new(foot(p2, n2), dir)
            .map_err(|_| TopoError::Precondition("blend: degenerate spring"))?;

        // Blend surface: cylinder of radius r about the spine. Its frame
        // x-axis points at the first spring line so the kept quarter is
        // unambiguous; z = spine direction.
        let fx = (foot(p1, n1) - spine_pt)
            .try_normalize()
            .ok_or(TopoError::Precondition("blend: degenerate frame"))?;
        let frame = Frame3 {
            origin: spine_pt,
            x: fx,
            y: dir.cross(fx),
            z: dir,
        };
        let surface = Cylinder3::new(frame, radius)
            .map_err(|_| TopoError::Precondition("blend: bad cylinder"))?;

        Ok(EdgeBlend {
            spine,
            spring_a,
            spring_b,
            surface,
        })
    }
}

/// The analytic data of a constant-radius fillet whose spine is a circle
/// (file 40 Case B): the spine circle, the two spring circles on the
/// supports, and the exact torus blend surface.
#[derive(Clone, Debug)]
pub struct EdgeBlendTorus {
    /// The ball-centre locus (a circle coaxial with the supports).
    pub spine: keel_geom::curve::Circle3,
    /// Tangency circle on the planar support.
    pub spring_plane: keel_geom::curve::Circle3,
    /// Tangency circle on the cylindrical support.
    pub spring_cyl: keel_geom::curve::Circle3,
    /// The exact blend surface: torus (major = spine radius, minor = r).
    pub surface: keel_geom::surface::Torus3,
}

impl Body {
    /// Generate the exact-torus blend for a convex `edge` where a PLANAR
    /// face meets a CYLINDRICAL face perpendicular to its axis (a cap rim;
    /// file 40 Case B / rung 2). The spine is the circle where the two
    /// offset surfaces (offset plane, coaxial offset cylinder) intersect,
    /// and the rolling-ball envelope of that circle is a torus of minor
    /// radius r. Requires R > 2r (Torus3 needs major = R - r > minor = r).
    ///
    /// Geometry stage only (mirrors blend_cylinder_for_edge); the torus
    /// trim-and-stitch surgery and the general (non-perpendicular) plane-
    /// cylinder cyclide case are follow-ups.
    pub fn blend_torus_for_edge(
        &self,
        edge: EdgeKey,
        radius: f64,
    ) -> Result<EdgeBlendTorus, TopoError> {
        use keel_geom::curve::Circle3;
        use keel_geom::surface::Torus3;
        if !(radius.is_finite() && radius > 0.0) {
            return Err(TopoError::Precondition("blend: radius must be positive"));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("blend: edge needs two faces"));
        }
        // Identify the planar and the cylindrical support.
        let mut plane_face = None;
        let mut cyl = None;
        for &f in &faces {
            match self.face_surface_geom(f) {
                Some(SurfaceGeom::Analytic(Surface3::Plane(_))) => plane_face = Some(f),
                Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => cyl = Some(c),
                _ => {}
            }
        }
        let (plane_face, cyl) = match (plane_face, cyl) {
            (Some(p), Some(c)) => (p, c),
            _ => {
                return Err(TopoError::Precondition(
                    "blend_torus: needs a plane and a cylinder support",
                ));
            }
        };
        let (axis, axis_o, r_cyl) = (cyl.frame.z, cyl.frame.origin, cyl.radius);
        let np = self
            .face_outward_normal(plane_face)
            .ok_or(TopoError::Precondition("blend_torus: plane normal"))?;
        // Cap case: the plane must be perpendicular to the cylinder axis.
        if np.cross(axis).norm() > 1e-7 {
            return Err(TopoError::Precondition(
                "blend_torus: non-perpendicular plane-cylinder (cyclide, follow-up)",
            ));
        }
        if r_cyl <= 2.0 * radius {
            return Err(TopoError::Precondition(
                "blend_torus: radius too large for cylinder (needs R > 2r)",
            ));
        }
        let pp = self
            .face_outer_loop_points(plane_face)
            .first()
            .copied()
            .ok_or(TopoError::Precondition("blend_torus: plane point"))?;
        let hp = (pp - axis_o).dot(axis); // plane height along the axis
        let sgn = np.dot(axis); // +1 cap faces +axis, -1 faces -axis
        let h_off = hp - sgn * radius; // offset plane, toward material
        let major = r_cyl - radius; // coaxial offset cylinder radius
        let centre = axis_o + axis * h_off;
        let (ex, ey) = (cyl.frame.x, cyl.frame.y);

        let spine = Circle3::new(centre, ex, ey, major)
            .map_err(|_| TopoError::Precondition("blend_torus: bad spine"))?;
        let spring_plane = Circle3::new(axis_o + axis * hp, ex, ey, major)
            .map_err(|_| TopoError::Precondition("blend_torus: bad plane spring"))?;
        let spring_cyl = Circle3::new(centre, ex, ey, r_cyl)
            .map_err(|_| TopoError::Precondition("blend_torus: bad cyl spring"))?;
        let frame = Frame3 {
            origin: centre,
            x: ex,
            y: ey,
            z: axis,
        };
        let surface = Torus3::new(frame, major, radius)
            .map_err(|_| TopoError::Precondition("blend_torus: bad torus"))?;
        Ok(EdgeBlendTorus {
            spine,
            spring_plane,
            spring_cyl,
            surface,
        })
    }

    /// All faces incident to a vertex (a face whose outer loop ends a fin
    /// at `v`).
    fn faces_at_vertex(&self, v: crate::entity::VertexKey) -> Vec<crate::entity::FaceKey> {
        let mut out = Vec::new();
        for (fk, f) in self.faces.iter() {
            'face: for &lk in &f.loops {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                loop {
                    if self.fin_end_vertex(cur) == Some(v) {
                        out.push(fk);
                        break 'face;
                    }
                    let Some(next) = self.fins.get(cur).map(|x| x.next) else {
                        break;
                    };
                    cur = next;
                    if cur == entry {
                        break;
                    }
                }
            }
        }
        out
    }

    /// The edge whose two bound vertices are exactly `{u, w}`, if any.
    fn edge_between(
        &self,
        u: crate::entity::VertexKey,
        w: crate::entity::VertexKey,
    ) -> Option<crate::entity::EdgeKey> {
        self.edges
            .iter()
            .find(|(_, e)| {
                let (a, b) = e.bounds;
                (a == u && b == w) || (a == w && b == u)
            })
            .map(|(k, _)| k)
    }

    /// Does `face` use edge `e` in one of its loops?
    fn face_has_edge(&self, face: crate::entity::FaceKey, e: EdgeKey) -> bool {
        self.edges
            .get(e)
            .map(|edge| {
                edge.radial.iter().any(|&rf| {
                    self.fins
                        .get(rf)
                        .and_then(|x| self.loops.get(x.owner))
                        .map(|l| l.face)
                        == Some(face)
                })
            })
            .unwrap_or(false)
    }

    /// The boundary edge of `face` incident to vertex `v`, excluding
    /// `exclude` (used to find the cap-side edge other than the sharp edge).
    fn boundary_edge_at_vertex_excluding(
        &self,
        face: crate::entity::FaceKey,
        v: crate::entity::VertexKey,
        exclude: EdgeKey,
    ) -> Option<EdgeKey> {
        let lp = self.faces.get(face).map(|f| f.loops[0])?;
        let entry = self.loops.get(lp).and_then(|l| l.fin)?;
        let mut cur = entry;
        loop {
            let fin = self.fins.get(cur)?;
            let e = fin.edge;
            if e != exclude
                && let Some(edge) = self.edges.get(e)
            {
                let (a, c) = edge.bounds;
                if a == v || c == v {
                    return Some(e);
                }
            }
            cur = fin.next;
            if cur == entry {
                break;
            }
        }
        None
    }

    /// Where an in-plane line (origin `o`, in-plane perpendicular `m`)
    /// crosses edge `e`'s segment, if it straddles it.
    fn line_crosses_edge(&self, e: EdgeKey, o: Vec3, m: Vec3) -> Option<Vec3> {
        let edge = self.edges.get(e)?;
        let (u0k, u1k) = edge.bounds;
        let u0 = self.vertices.get(u0k)?.point;
        let u1 = self.vertices.get(u1k)?.point;
        let s0 = (u0 - o).dot(m);
        let s1 = (u1 - o).dot(m);
        if (s0 > 0.0) != (s1 > 0.0) && (s0 - s1).abs() > 1e-12 {
            let t = s0 / (s0 - s1);
            Some(u0 + (u1 - u0) * t)
        } else {
            None
        }
    }

    /// The shared blend CAP-SPLIT skeleton (surgery parameterization,
    /// first extraction): split `cap` between the fins ending at the
    /// two given vertices, copy the support surface onto the new
    /// piece, attach the supplied end-arc curve, and return the new
    /// edge. Each blend engine keeps its own cap SELECTOR and ARC
    /// GEOMETRY (those genuinely differ); the topology skeleton is one.
    fn split_blend_cap(
        &mut self,
        cap: crate::entity::FaceKey,
        a_end: crate::entity::VertexKey,
        b_end: crate::entity::VertexKey,
        arc: Curve3,
    ) -> Result<EdgeKey, TopoError> {
        let lp = self
            .faces
            .get(cap)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let fin_a = self.fin_ending_at_vertex(lp, a_end)?;
        let fin_b = self.fin_ending_at_vertex(lp, b_end)?;
        let split = self.split_face(fin_a, fin_b, None)?;
        if let Some(surf) = self.faces.get(cap).and_then(|f| f.surface)
            && let Some(nf) = self.faces.get_mut(split.face_new)
        {
            nf.surface = Some(surf);
        }
        self.attach_edge_curve(split.edge, arc, true);
        Ok(split.edge)
    }

    /// Trim `face` to `spring` by splitting its two cap-side boundary
    /// edges (at the vertices near `va`/`vb`) at the spring crossings and
    /// splitting the face. Returns (spring edge, strip face still carrying
    /// `sharp`, kept support face, spring vertex near va, near vb).
    #[allow(clippy::type_complexity)]
    fn imprint_spring_line(
        &mut self,
        face: crate::entity::FaceKey,
        sharp: EdgeKey,
        va: crate::entity::VertexKey,
        vb: crate::entity::VertexKey,
        spring: &Line3,
        n: Vec3,
    ) -> Result<
        (
            EdgeKey,
            crate::entity::FaceKey,
            crate::entity::FaceKey,
            crate::entity::VertexKey,
            crate::entity::VertexKey,
        ),
        TopoError,
    > {
        let e_va = self
            .boundary_edge_at_vertex_excluding(face, va, sharp)
            .ok_or(TopoError::Precondition("fillet: no cap edge at va"))?;
        let e_vb = self
            .boundary_edge_at_vertex_excluding(face, vb, sharp)
            .ok_or(TopoError::Precondition("fillet: no cap edge at vb"))?;
        let m = n.cross(spring.dir);
        let aa_pt =
            self.line_crosses_edge(e_va, spring.origin, m)
                .ok_or(TopoError::Precondition(
                    "fillet: spring misses cap edge (overflow?)",
                ))?;
        let ab_pt =
            self.line_crosses_edge(e_vb, spring.origin, m)
                .ok_or(TopoError::Precondition(
                    "fillet: spring misses cap edge (overflow?)",
                ))?;
        let sva = self.split_edge(e_va, aa_pt)?;
        let svb = self.split_edge(e_vb, ab_pt)?;
        let lp = self
            .faces
            .get(face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let fa = self.fin_ending_at_vertex(lp, sva.vertex)?;
        let fb = self.fin_ending_at_vertex(lp, svb.vertex)?;
        let split = self.split_face(fa, fb, None)?;
        // split_face leaves the new face without a surface; both halves
        // lie on the support plane, so copy it across.
        if let Some(surf) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(nf) = self.faces.get_mut(split.face_new)
        {
            nf.surface = Some(surf);
        }
        let (strip, kept) = if self.face_has_edge(split.face_new, sharp) {
            (split.face_new, split.face_old)
        } else {
            (split.face_old, split.face_new)
        };
        self.attach_edge_curve(split.edge, Curve3::Line(*spring), true);
        Ok((split.edge, strip, kept, sva.vertex, svb.vertex))
    }

    /// Imprint a closed ring `curve` onto `face`, dispatching to the
    /// seam-crossing variant for periodic faces (a circle on a cylinder
    /// crosses the seam) and the interior variant otherwise.
    fn imprint_ring(
        &mut self,
        face: crate::entity::FaceKey,
        curve: &Curve3,
        tol: f64,
    ) -> Result<crate::imprint::ImprintReport, TopoError> {
        if self.closed_curve_crosses_boundary(face, curve, tol) {
            self.imprint_closed_curve_crossing(face, curve, tol)
        } else {
            self.imprint_closed_curve(face, curve, tol)
        }
    }

    /// Round a convex cap-rim `edge` (a planar face meeting a cylinder
    /// perpendicular to its axis) with a constant-radius torus fillet
    /// (file 40 Case B / rung 2). Imprints the two spring circles onto the
    /// supports (trimming the cap to radius R-r and the cylinder to the
    /// offset height), removes the sharp rim by merging the cap annulus
    /// and the cylinder top band (kef), and gives the merged ring the
    /// exact torus surface.
    ///
    /// Scope: the cap case (plane perpendicular to the cylinder axis),
    /// R > 2r. Trimmed-torus tessellation (for mesh_volume on the result)
    /// and the general plane-cylinder cyclide case are follow-ups.
    pub fn fillet_cap_rim(&self, edge: EdgeKey, radius: f64) -> Result<Body, TopoError> {
        let blend = self.blend_torus_for_edge(edge, radius)?;
        let faces = self.faces_around_edge(edge);
        let cap = faces
            .iter()
            .copied()
            .find(|&f| {
                matches!(
                    self.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
                )
            })
            .ok_or(TopoError::Precondition("fillet_rim: no cap"))?;
        let lat = faces
            .iter()
            .copied()
            .find(|&f| {
                matches!(
                    self.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                )
            })
            .ok_or(TopoError::Precondition("fillet_rim: no cylinder"))?;
        let tol = 1e-7;
        let mut b = self.clone();

        let rep_cap = b.imprint_ring(cap, &Curve3::Circle(blend.spring_plane), tol)?;
        let annulus = *rep_cap
            .faces
            .iter()
            .find(|&&f| b.face_has_edge(f, edge))
            .ok_or(TopoError::Precondition("fillet_rim: no cap annulus"))?;
        let inner_disc = *rep_cap
            .faces
            .iter()
            .find(|&&f| f != annulus)
            .ok_or(TopoError::Precondition("fillet_rim: no inner disc"))?;
        let spring_plane_edge = rep_cap.edge;

        let rep_lat = b.imprint_ring(lat, &Curve3::Circle(blend.spring_cyl), tol)?;
        let spring_cyl_edge = rep_lat.edge;

        // The cap annulus is a HOLED face (rim outer loop + spring_plane
        // hole). kef needs a one-loop dying face, so first bridge the hole
        // to the outer loop with mekr (the spring_plane becomes part of a
        // single loop -- the seam of the eventual periodic torus ring).
        let alps = b
            .faces
            .get(annulus)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        let loop_has_edge = |b: &Body, lk: crate::entity::LoopKey, e: EdgeKey| -> bool {
            let Some(entry) = b.loops.get(lk).and_then(|l| l.fin) else {
                return false;
            };
            let mut cur = entry;
            while let Some(fin) = b.fins.get(cur) {
                if fin.edge == e {
                    return true;
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
            false
        };
        let rim_loop = alps
            .iter()
            .copied()
            .find(|&lk| loop_has_edge(&b, lk, edge))
            .ok_or(TopoError::Precondition("fillet_rim: no rim loop"))?;
        let ring_loop = alps
            .iter()
            .copied()
            .find(|&lk| lk != rim_loop)
            .ok_or(TopoError::Precondition("fillet_rim: no spring hole loop"))?;
        let fin_outer = b
            .loops
            .get(rim_loop)
            .and_then(|l| l.fin)
            .ok_or(TopoError::StaleKey)?;
        let fin_ring = b
            .loops
            .get(ring_loop)
            .and_then(|l| l.fin)
            .ok_or(TopoError::StaleKey)?;
        b.mekr(fin_outer, fin_ring)?;

        // Merge the cap annulus and the cylinder top band across the rim.
        b.kef(edge)?;
        // The torus ring is the face on the plane-spring edge that is not
        // the kept inner cap disc.
        let ring = b
            .faces_around_edge(spring_plane_edge)
            .into_iter()
            .find(|&f| f != inner_disc)
            .ok_or(TopoError::Precondition("fillet_rim: no torus ring"))?;
        b.attach_face_surface(
            ring,
            SurfaceGeom::Analytic(Surface3::Torus(blend.surface)),
            true,
        );
        b.attach_edge_curve(spring_plane_edge, Curve3::Circle(blend.spring_plane), true);
        b.attach_edge_curve(spring_cyl_edge, Curve3::Circle(blend.spring_cyl), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("fillet_rim: result invalid"))?;
        Ok(b)
    }

    /// Round a CONVEX or CONCAVE `edge` with a constant-radius rolling-ball
    /// fillet (parity items 47-61; research files 40, 44). Dispatches by
    /// support geometry: a plane/cylinder cap rim routes to the exact-torus
    /// rung (fillet_cap_rim); two planar supports use the exact-cylinder
    /// rung here. Returns the filleted body: the cylinder blend face is
    /// inserted by the local trim-and-stitch surgery of file 40 §3 (imprint
    /// the spring lines, split the cap faces along the end arcs, dissolve
    /// the sharp-corner fragments with kef/kev), supports trimmed to the
    /// spring curves and the sharp edge removed.
    ///
    /// Convex (round, material removed) and concave (fillet, material added)
    /// share ONE pipeline, forking only on the convexity sign (file 44): the
    /// offset direction flips (-r into material / +r into the void) and the
    /// blend face's sense flips (radial-out / radial-in); the cap-shrink vs
    /// cap-grow then happens automatically through face adjacency.
    ///
    /// Scope: the edge's two end vertices must each be a simple degree-3
    /// corner (two supports + one cap face), the box-like case. The blend
    /// face carries its exact `Cylinder3` surface; blend-face pcurves (so
    /// analytic mass_properties covers it) and overflow handling (file 41)
    /// are follow-ups, as are non-planar supports and mixed-convexity ends.
    pub fn fillet_edge(&self, edge: EdgeKey, radius: f64) -> Result<Body, TopoError> {
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("fillet: edge needs two faces"));
        }
        // Dispatch by support geometry: a plane/cylinder rim is the torus
        // rung; two planes is the cylinder rung handled below.
        let cylinders = faces
            .iter()
            .filter(|&&f| {
                matches!(
                    self.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                )
            })
            .count();
        if cylinders == 1 {
            return self.fillet_cap_rim(edge, radius);
        }
        // Unify on the convexity sign (file 44): the surgery body is
        // identical; only the cap step and the blend-face sense fork. The
        // blend's outward normal is radial-OUT for a convex round and
        // radial-IN for a concave fillet.
        let convex = self.edge_is_convex(edge).unwrap_or(true);
        let blend = self.blend_cylinder_for_edge(edge, radius)?;
        let (f1, f2) = (faces[0], faces[1]);
        let (va_k, vb_k) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let p_a = self.vertices.get(va_k).ok_or(TopoError::StaleKey)?.point;
        let p_b = self.vertices.get(vb_k).ok_or(TopoError::StaleKey)?.point;
        let dir = blend.spine.dir;
        let spine_pt = blend.spine.origin;

        let mut b = self.clone();

        // --- Phase 1: trim each support to its spring line. ---
        let n1 = b
            .face_outward_normal(f1)
            .ok_or(TopoError::Precondition("fillet: face normal"))?;
        let n2 = b
            .face_outward_normal(f2)
            .ok_or(TopoError::Precondition("fillet: face normal"))?;
        let (spring_a_edge, strip1, f1k, aa, ab) =
            b.imprint_spring_line(f1, edge, va_k, vb_k, &blend.spring_a, n1)?;
        let (spring_b_edge, strip2, _f2k, ba, bb) =
            b.imprint_spring_line(f2, edge, va_k, vb_k, &blend.spring_b, n2)?;
        let _ = (p_a, p_b);

        // --- Phase 2: split each cap face along its end arc. ---
        // Cap at the A end: the face incident to va_k that is neither strip.
        let split_cap =
            |b: &mut Body,
             v_corner: crate::entity::VertexKey,
             a_end: crate::entity::VertexKey,
             b_end: crate::entity::VertexKey|
             -> Result<(crate::entity::FaceKey, crate::entity::EdgeKey), TopoError> {
                let cap = b
                    .faces_at_vertex(v_corner)
                    .into_iter()
                    .find(|f| *f != strip1 && *f != strip2)
                    .ok_or(TopoError::Precondition("fillet: no cap face"))?;
                let lp = b
                    .faces
                    .get(cap)
                    .map(|f| f.loops[0])
                    .ok_or(TopoError::StaleKey)?;
                let fin_a = b.fin_ending_at_vertex(lp, a_end)?;
                let fin_b = b.fin_ending_at_vertex(lp, b_end)?;
                let split = b.split_face(fin_a, fin_b, None)?;
                // split_face leaves the new face surfaceless; both halves
                // lie on the cap plane, so copy it (matches every other
                // split_face caller; needed once the concave path, where
                // the kept cap is the new face, is un-gated).
                if let Some(surf) = b.faces.get(cap).and_then(|f| f.surface)
                    && let Some(nf) = b.faces.get_mut(split.face_new)
                {
                    nf.surface = Some(surf);
                }
                // Arc on the new edge: a quarter circle in the cap plane,
                // centred at the spine projected into that plane.
                let pc = b
                    .vertices
                    .get(v_corner)
                    .map(|x| x.point)
                    .unwrap_or(spine_pt);
                let centre = spine_pt + dir * ((pc - spine_pt).dot(dir));
                let pa_end = b
                    .vertices
                    .get(a_end)
                    .map(|x| x.point)
                    .ok_or(TopoError::StaleKey)?;
                let ex = (pa_end - centre)
                    .try_normalize()
                    .ok_or(TopoError::Precondition("fillet: arc axis"))?;
                let arc = keel_geom::curve::Circle3::new(centre, ex, dir.cross(ex), radius)
                    .map_err(|_| TopoError::Precondition("fillet: bad arc"))?;
                b.attach_edge_curve(split.edge, Curve3::Circle(arc), true);
                Ok((split.face_new, split.edge))
            };
        let (_corner_a, _arc_a) = split_cap(&mut b, va_k, aa, ba)?;
        let (_corner_b, _arc_b) = split_cap(&mut b, vb_k, ab, bb)?;

        // --- Phase 3: dissolve the four corner fragments into one face. ---
        // Merge the two strips across the sharp edge.
        b.kef(edge)?;
        // Merge each cap corner in, then kill the resulting degree-1 spur.
        let e_a = b
            .edge_between(aa, va_k)
            .ok_or(TopoError::Precondition("fillet: no A spring stub"))?;
        b.kef(e_a)?;
        let spur_a = b
            .edge_between(va_k, ba)
            .ok_or(TopoError::Precondition("fillet: no A spur"))?;
        b.kev(spur_a)?;
        let e_b = b
            .edge_between(ab, vb_k)
            .ok_or(TopoError::Precondition("fillet: no B spring stub"))?;
        b.kef(e_b)?;
        let spur_b = b
            .edge_between(vb_k, bb)
            .ok_or(TopoError::Precondition("fillet: no B spur"))?;
        b.kev(spur_b)?;

        // --- Phase 4: the surviving face on the spring-a edge (other than
        // the trimmed support) is the blend face; give it the cylinder. ---
        let blend_face = b
            .faces_around_edge(spring_a_edge)
            .into_iter()
            .find(|f| *f != f1k)
            .ok_or(TopoError::Precondition("fillet: no blend face"))?;
        b.attach_face_surface(
            blend_face,
            SurfaceGeom::Analytic(Surface3::Cylinder(blend.surface.clone())),
            convex,
        );
        b.attach_edge_curve(spring_a_edge, Curve3::Line(blend.spring_a), true);
        b.attach_edge_curve(spring_b_edge, Curve3::Line(blend.spring_b), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("fillet: result invalid"))?;
        Ok(b)
    }
}

/// The analytic data of a VARIABLE-radius (linear r0 -> r1) rolling-
/// ball fillet on a plane-plane edge (parity item 48, rung 1). With a
/// linearly varying radius, each offset "plane" tilts but stays a
/// PLANE (the offset distance is linear in position), so the spine is
/// still a straight line; the envelope of the spheres is an exact
/// CONE (half-angle asin(dr/ds) about the spine, perpendicular radius
/// rho/cos(alpha)); and the tangency loci on the supports stay
/// straight lines. The whole constant-radius rung-1 story generalizes
/// without leaving the analytic island.
#[derive(Clone, Debug)]
pub struct VariableEdgeBlend {
    pub spine: Line3,
    pub spring_a: Line3,
    pub spring_b: Line3,
    /// The exact blend surface: a cone about the spine.
    pub surface: keel_geom::surface::Cone3,
    /// Sphere radius at the edge's two ends (bounds.0, bounds.1).
    pub r: (f64, f64),
}

impl Body {
    /// Generate the exact-cone variable-radius blend for a convex or
    /// concave plane-plane `edge`, the ball radius varying linearly
    /// from `r0` at bounds.0 to `r1` at bounds.1 (item 48 rung 1).
    /// `r0 == r1` belongs to `blend_cylinder_for_edge`.
    pub fn blend_cone_for_edge(
        &self,
        edge: EdgeKey,
        r0: f64,
        r1: f64,
    ) -> Result<VariableEdgeBlend, TopoError> {
        let ok = r0.is_finite() && r0 > 0.0 && r1.is_finite() && r1 > 0.0;
        if !ok {
            return Err(TopoError::Precondition("blend: radii must be positive"));
        }
        if (r1 - r0).abs() < 1e-12 {
            return Err(TopoError::Precondition(
                "blend: constant radius belongs to the cylinder rung",
            ));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("blend: edge needs two faces"));
        }
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "blend: non-planar support (follow-up)",
                ));
            }
        }
        let n1 = self
            .face_outward_normal(faces[0])
            .ok_or(TopoError::Precondition("blend: face normal"))?;
        let n2 = self
            .face_outward_normal(faces[1])
            .ok_or(TopoError::Precondition("blend: face normal"))?;
        let q1 = self
            .face_outer_loop_points(faces[0])
            .first()
            .copied()
            .ok_or(TopoError::Precondition("blend: face point"))?;
        let q2 = self
            .face_outer_loop_points(faces[1])
            .first()
            .copied()
            .ok_or(TopoError::Precondition("blend: face point"))?;
        let (va, vb) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let pa = self.vertices.get(va).ok_or(TopoError::StaleKey)?.point;
        let pb = self.vertices.get(vb).ok_or(TopoError::StaleKey)?.point;
        let len = (pb - pa).norm();
        let e = (pb - pa)
            .try_normalize()
            .ok_or(TopoError::Precondition("blend: degenerate edge"))?;
        let k = (r1 - r0) / len;
        let convex = self
            .edge_is_convex(edge)
            .ok_or(TopoError::Precondition("blend: cannot determine convexity"))?;
        let off_sign = if convex { -1.0 } else { 1.0 };
        // Tilted offset planes: n.x = d + off_sign * r(x) with
        // r(x) = r0 + k (x - pa).e  =>  (n - off_sign k e).x = d + off_sign (r0 - k pa.e).
        let m1 = n1 - e * (off_sign * k);
        let m2 = n2 - e * (off_sign * k);
        let c1 = n1.dot(q1) + off_sign * (r0 - k * pa.dot(e));
        let c2 = n2.dot(q2) + off_sign * (r0 - k * pa.dot(e));
        let d = m1.cross(m2);
        let dd = d.dot(d);
        if dd < 1e-18 {
            return Err(TopoError::Precondition("blend: parallel offset planes"));
        }
        let sp = (m2.cross(d) * c1 + d.cross(m1) * c2) * (1.0 / dd);
        let u = d
            .try_normalize()
            .ok_or(TopoError::Precondition("blend: degenerate spine"))?;
        // Sphere radius along the spine: rho(tau) = rho0 + g tau.
        let rho_at = |x: Vec3| r0 + k * (x - pa).dot(e);
        let rho0 = rho_at(sp);
        let g = k * u.dot(e);
        if g.abs() < 1e-12 || g.abs() >= 1.0 {
            return Err(TopoError::Precondition("blend: degenerate cone slope"));
        }
        // Orient the axis toward growing radius.
        let (u, g) = if g > 0.0 { (u, g) } else { (u * -1.0, -g) };
        let alpha = g.asin();
        let frame =
            Frame3::from_z(sp, u).map_err(|_| TopoError::Precondition("blend: cone frame"))?;
        let surface = keel_geom::surface::Cone3 {
            frame,
            radius: rho0 / alpha.cos(),
            half_angle: alpha,
        };
        // Tangency loci: touch(tau) = spine(tau) - off_sign * n * rho(tau),
        // linear in tau -> straight spring lines.
        let touch = |n: Vec3, t: f64| {
            let c = sp + u * t;
            c - n * (off_sign * rho_at(c))
        };
        let (a0, a1) = (touch(n1, 0.0), touch(n1, 1.0));
        let (b0, b1) = (touch(n2, 0.0), touch(n2, 1.0));
        let spring_a = Line3::new(a0, a1 - a0)
            .map_err(|_| TopoError::Precondition("blend: degenerate spring"))?;
        let spring_b = Line3::new(b0, b1 - b0)
            .map_err(|_| TopoError::Precondition("blend: degenerate spring"))?;
        Ok(VariableEdgeBlend {
            spine: Line3::new(sp, u).map_err(|_| TopoError::Precondition("blend: spine"))?,
            spring_a,
            spring_b,
            surface,
            r: (r0, r1),
        })
    }

    /// Round a convex/concave plane-plane `edge` with a VARIABLE-radius
    /// rolling-ball fillet, r varying linearly from `r0` at the edge's
    /// first vertex to `r1` at its second (parity item 48, rung 1: the
    /// exact-cone blend; see `blend_cone_for_edge`). Same trim-and-
    /// stitch surgery as `fillet_edge`, with the end arcs the EXACT
    /// ellipses where the cone meets each cap plane.
    pub fn fillet_edge_variable(&self, edge: EdgeKey, r0: f64, r1: f64) -> Result<Body, TopoError> {
        if (r1 - r0).abs() < 1e-12 {
            return self.fillet_edge(edge, r0);
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("fillet: edge needs two faces"));
        }
        let convex = self.edge_is_convex(edge).unwrap_or(true);
        let blend = self.blend_cone_for_edge(edge, r0, r1)?;
        let (f1, f2) = (faces[0], faces[1]);
        let (va_k, vb_k) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;

        let mut b = self.clone();
        let n1 = b
            .face_outward_normal(f1)
            .ok_or(TopoError::Precondition("fillet: face normal"))?;
        let n2 = b
            .face_outward_normal(f2)
            .ok_or(TopoError::Precondition("fillet: face normal"))?;
        let (spring_a_edge, strip1, f1k, aa, ab) =
            b.imprint_spring_line(f1, edge, va_k, vb_k, &blend.spring_a, n1)?;
        let (spring_b_edge, strip2, _f2k, ba, bb) =
            b.imprint_spring_line(f2, edge, va_k, vb_k, &blend.spring_b, n2)?;

        // Split each cap along the EXACT cone/cap-plane ellipse arc.
        let cone = blend.surface.clone();
        let split_cap = |b: &mut Body,
                         v_corner: crate::entity::VertexKey,
                         a_end: crate::entity::VertexKey,
                         b_end: crate::entity::VertexKey|
         -> Result<(), TopoError> {
            let cap = b
                .faces_at_vertex(v_corner)
                .into_iter()
                .find(|f| *f != strip1 && *f != strip2)
                .ok_or(TopoError::Precondition("fillet: no cap face"))?;
            let cap_n = b
                .face_outward_normal(cap)
                .ok_or(TopoError::Precondition("fillet: cap normal"))?;
            let cap_p = b
                .vertices
                .get(v_corner)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let ell = cone_plane_ellipse(&cone, cap_p, cap_n)
                .ok_or(TopoError::Precondition("fillet: cap ellipse"))?;
            b.split_blend_cap(cap, a_end, b_end, Curve3::Ellipse(ell))?;
            Ok(())
        };
        split_cap(&mut b, va_k, aa, ba)?;
        split_cap(&mut b, vb_k, ab, bb)?;

        // Dissolve the corner fragments (identical to fillet_edge).
        b.kef(edge)?;
        let e_a = b
            .edge_between(aa, va_k)
            .ok_or(TopoError::Precondition("fillet: no A spring stub"))?;
        b.kef(e_a)?;
        let spur_a = b
            .edge_between(va_k, ba)
            .ok_or(TopoError::Precondition("fillet: no A spur"))?;
        b.kev(spur_a)?;
        let e_b = b
            .edge_between(ab, vb_k)
            .ok_or(TopoError::Precondition("fillet: no B spring stub"))?;
        b.kef(e_b)?;
        let spur_b = b
            .edge_between(vb_k, bb)
            .ok_or(TopoError::Precondition("fillet: no B spur"))?;
        b.kev(spur_b)?;

        let blend_face = b
            .faces_around_edge(spring_a_edge)
            .into_iter()
            .find(|f| *f != f1k)
            .ok_or(TopoError::Precondition("fillet: no blend face"))?;
        b.attach_face_surface(
            blend_face,
            SurfaceGeom::Analytic(Surface3::Cone(blend.surface.clone())),
            convex,
        );
        b.attach_edge_curve(spring_a_edge, Curve3::Line(blend.spring_a), true);
        b.attach_edge_curve(spring_b_edge, Curve3::Line(blend.spring_b), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("fillet: result invalid"))?;
        Ok(b)
    }

    /// Blend a plane-plane `edge` with a G2 (curvature-continuous)
    /// cross-section (parity item 60): springs set back `d` along each
    /// support; the section is a QUINTIC Bezier whose first three and
    /// last three control points are COLLINEAR along the in-plane
    /// tangent directions, so the section's curvature is ZERO at both
    /// springs -- matching the planar supports' zero normal curvature
    /// exactly (true G2, unlike the G1 circular fillet whose curvature
    /// jumps from 0 to 1/r at the spring). `fullness` in (0, 0.5]
    /// scales the collinear leg length as a fraction of `d`. The blend
    /// face carries the exact degree-5x1 strip; cap arcs are the same
    /// quintic per cap plane.
    pub fn fillet_edge_g2(&self, edge: EdgeKey, d: f64, fullness: f64) -> Result<Body, TopoError> {
        let ok =
            d.is_finite() && d > 0.0 && fullness.is_finite() && fullness > 0.0 && fullness <= 0.5;
        if !ok {
            return Err(TopoError::Precondition("g2: bad setback/fullness"));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("g2: edge needs two faces"));
        }
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "g2: non-planar support (follow-up)",
                ));
            }
        }
        let convex = self.edge_is_convex(edge).unwrap_or(true);
        let (f1, f2) = (faces[0], faces[1]);
        let n1 = self
            .face_outward_normal(f1)
            .ok_or(TopoError::Precondition("g2: face normal"))?;
        let n2 = self
            .face_outward_normal(f2)
            .ok_or(TopoError::Precondition("g2: face normal"))?;
        let (va_k, vb_k) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let pa = self.vertices.get(va_k).ok_or(TopoError::StaleKey)?.point;
        let pb = self.vertices.get(vb_k).ok_or(TopoError::StaleKey)?.point;
        let e = (pb - pa)
            .try_normalize()
            .ok_or(TopoError::Precondition("g2: degenerate edge"))?;
        let away = |np: Vec3, nq: Vec3| -> Result<Vec3, TopoError> {
            let u = e
                .cross(np)
                .try_normalize()
                .ok_or(TopoError::Precondition("g2: degenerate setback"))?;
            let s = if convex { 1.0 } else { -1.0 };
            Ok(if u.dot(nq) * s > 0.0 { u * -1.0 } else { u })
        };
        let u_a = away(n1, n2)?;
        let u_b = away(n2, n1)?;
        let lam = fullness * d;
        // Quintic section control rows at each end of the edge: the
        // first/last three collinear -> zero curvature at the springs.
        let section = |base: Vec3| -> [Vec3; 6] {
            let sa = base + u_a * d;
            let sb = base + u_b * d;
            [
                sa,
                sa - u_a * lam,
                sa - u_a * (2.0 * lam),
                sb - u_b * (2.0 * lam),
                sb - u_b * lam,
                sb,
            ]
        };
        let (sec0, sec1) = (section(pa), section(pb));
        let (sa0, sa1) = (sec0[0], sec1[0]);
        let (sb0, sb1) = (sec0[5], sec1[5]);
        let spring_a =
            Line3::new(sa0, sa1 - sa0).map_err(|_| TopoError::Precondition("g2: spring a"))?;
        let spring_b =
            Line3::new(sb0, sb1 - sb0).map_err(|_| TopoError::Precondition("g2: spring b"))?;
        let mut net = Vec::with_capacity(12);
        for k in 0..6 {
            net.push(sec0[k]);
            net.push(sec1[k]);
        }
        let strip = keel_geom::nurbs_surface::NurbsSurface::new(
            5,
            1,
            vec![0.0; 6].into_iter().chain(vec![1.0; 6]).collect(),
            vec![0.0, 0.0, 1.0, 1.0],
            net,
            None,
        )
        .map_err(|_| TopoError::Precondition("g2: bad strip"))?;

        let mut b = self.clone();
        let (spring_a_edge, strip1, f1k, aa, ab) =
            b.imprint_spring_line(f1, edge, va_k, vb_k, &spring_a, n1)?;
        let (spring_b_edge, strip2, _f2k, ba, bb) =
            b.imprint_spring_line(f2, edge, va_k, vb_k, &spring_b, n2)?;

        let split_cap = |b: &mut Body,
                         v_corner: crate::entity::VertexKey,
                         a_end: crate::entity::VertexKey,
                         b_end: crate::entity::VertexKey|
         -> Result<(), TopoError> {
            let cap = b
                .faces_at_vertex(v_corner)
                .into_iter()
                .find(|f| *f != strip1 && *f != strip2)
                .ok_or(TopoError::Precondition("g2: no cap face"))?;
            let corner = b
                .vertices
                .get(v_corner)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let sec = section(corner);
            let arc = keel_geom::nurbs_curve::NurbsCurve::new(
                5,
                vec![0.0; 6].into_iter().chain(vec![1.0; 6]).collect(),
                sec.to_vec(),
                None,
            )
            .map_err(|_| TopoError::Precondition("g2: bad cap arc"))?;
            b.split_blend_cap(cap, a_end, b_end, Curve3::Nurbs(arc))?;
            Ok(())
        };
        split_cap(&mut b, va_k, aa, ba)?;
        split_cap(&mut b, vb_k, ab, bb)?;

        b.kef(edge)?;
        let e_a = b
            .edge_between(aa, va_k)
            .ok_or(TopoError::Precondition("g2: no A spring stub"))?;
        b.kef(e_a)?;
        let spur_a = b
            .edge_between(va_k, ba)
            .ok_or(TopoError::Precondition("g2: no A spur"))?;
        b.kev(spur_a)?;
        let e_b = b
            .edge_between(ab, vb_k)
            .ok_or(TopoError::Precondition("g2: no B spring stub"))?;
        b.kef(e_b)?;
        let spur_b = b
            .edge_between(vb_k, bb)
            .ok_or(TopoError::Precondition("g2: no B spur"))?;
        b.kev(spur_b)?;

        let blend_face = b
            .faces_around_edge(spring_a_edge)
            .into_iter()
            .find(|f| *f != f1k)
            .ok_or(TopoError::Precondition("g2: no blend face"))?;
        b.attach_face_surface(blend_face, SurfaceGeom::Nurbs(strip), convex);
        b.attach_edge_curve(spring_a_edge, Curve3::Line(spring_a), true);
        b.attach_edge_curve(spring_b_edge, Curve3::Line(spring_b), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("g2: result invalid"))?;
        Ok(b)
    }

    /// Blend a plane-plane `edge` with a CONIC cross-section (parity
    /// item 49): springs set back `d` along each support from the
    /// edge, the section a rational quadratic with the shoulder ON the
    /// old sharp edge and weight `w` -- the chamfer-to-circle
    /// continuum. w -> 0 flattens toward the chamfer chord; for
    /// PERPENDICULAR supports w = sqrt(2)/2 is the EXACT circular
    /// fillet of radius d (the oracle); larger w hugs the sharp
    /// corner. The blend face carries the exact rational strip
    /// (degree 2 x 1 NURBS); the end arcs are the same conic in each
    /// cap plane. G2/curvature-continuous sections (item 60) stay a
    /// follow-up; `w` controls fullness, not curvature matching.
    pub fn fillet_edge_conic(&self, edge: EdgeKey, d: f64, w: f64) -> Result<Body, TopoError> {
        let ok = d.is_finite() && d > 0.0 && w.is_finite() && w > 0.0;
        if !ok {
            return Err(TopoError::Precondition("conic: bad setback/weight"));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("conic: edge needs two faces"));
        }
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "conic: non-planar support (follow-up)",
                ));
            }
        }
        let convex = self.edge_is_convex(edge).unwrap_or(true);
        let (f1, f2) = (faces[0], faces[1]);
        let n1 = self
            .face_outward_normal(f1)
            .ok_or(TopoError::Precondition("conic: face normal"))?;
        let n2 = self
            .face_outward_normal(f2)
            .ok_or(TopoError::Precondition("conic: face normal"))?;
        let (va_k, vb_k) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let pa = self.vertices.get(va_k).ok_or(TopoError::StaleKey)?.point;
        let pb = self.vertices.get(vb_k).ok_or(TopoError::StaleKey)?.point;
        let e = (pb - pa)
            .try_normalize()
            .ok_or(TopoError::Precondition("conic: degenerate edge"))?;
        // In-face setback directions: in plane A, perpendicular to the
        // edge, pointing AWAY from support B's outward side (convex; the
        // reverse for concave).
        let away = |np: Vec3, nq: Vec3| -> Result<Vec3, TopoError> {
            let u = e
                .cross(np)
                .try_normalize()
                .ok_or(TopoError::Precondition("conic: degenerate setback"))?;
            // Convex: the setback moves AWAY from the other support's
            // outward side (u . nq < 0); concave: toward it.
            let s = if convex { 1.0 } else { -1.0 };
            Ok(if u.dot(nq) * s > 0.0 { u * -1.0 } else { u })
        };
        let u_a = away(n1, n2)?;
        let u_b = away(n2, n1)?;
        let (sa0, sa1) = (pa + u_a * d, pb + u_a * d);
        let (sb0, sb1) = (pa + u_b * d, pb + u_b * d);
        let spring_a =
            Line3::new(sa0, sa1 - sa0).map_err(|_| TopoError::Precondition("conic: spring a"))?;
        let spring_b =
            Line3::new(sb0, sb1 - sb0).map_err(|_| TopoError::Precondition("conic: spring b"))?;
        // The rational strip: degree 2 section x degree 1 along the
        // edge, shoulder row on the old sharp edge with weight w.
        let strip = keel_geom::nurbs_surface::NurbsSurface::new(
            2,
            1,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
            vec![sa0, sa1, pa, pb, sb0, sb1],
            Some(vec![1.0, 1.0, w, w, 1.0, 1.0]),
        )
        .map_err(|_| TopoError::Precondition("conic: bad strip"))?;

        let mut b = self.clone();
        let (spring_a_edge, strip1, f1k, aa, ab) =
            b.imprint_spring_line(f1, edge, va_k, vb_k, &spring_a, n1)?;
        let (spring_b_edge, strip2, _f2k, ba, bb) =
            b.imprint_spring_line(f2, edge, va_k, vb_k, &spring_b, n2)?;

        // Split each cap along the conic section arc (the same rational
        // quadratic, in that cap plane, shoulder at the old corner).
        let split_cap = |b: &mut Body,
                         v_corner: crate::entity::VertexKey,
                         a_end: crate::entity::VertexKey,
                         b_end: crate::entity::VertexKey|
         -> Result<(), TopoError> {
            let cap = b
                .faces_at_vertex(v_corner)
                .into_iter()
                .find(|f| *f != strip1 && *f != strip2)
                .ok_or(TopoError::Precondition("conic: no cap face"))?;
            let corner = b
                .vertices
                .get(v_corner)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let qa = b
                .vertices
                .get(a_end)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let qb = b
                .vertices
                .get(b_end)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let arc = keel_geom::nurbs_curve::NurbsCurve::new(
                2,
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                vec![qa, corner, qb],
                Some(vec![1.0, w, 1.0]),
            )
            .map_err(|_| TopoError::Precondition("conic: bad cap arc"))?;
            b.split_blend_cap(cap, a_end, b_end, Curve3::Nurbs(arc))?;
            Ok(())
        };
        split_cap(&mut b, va_k, aa, ba)?;
        split_cap(&mut b, vb_k, ab, bb)?;

        // Dissolve the corner fragments (the standard surgery).
        b.kef(edge)?;
        let e_a = b
            .edge_between(aa, va_k)
            .ok_or(TopoError::Precondition("conic: no A spring stub"))?;
        b.kef(e_a)?;
        let spur_a = b
            .edge_between(va_k, ba)
            .ok_or(TopoError::Precondition("conic: no A spur"))?;
        b.kev(spur_a)?;
        let e_b = b
            .edge_between(ab, vb_k)
            .ok_or(TopoError::Precondition("conic: no B spring stub"))?;
        b.kef(e_b)?;
        let spur_b = b
            .edge_between(vb_k, bb)
            .ok_or(TopoError::Precondition("conic: no B spur"))?;
        b.kev(spur_b)?;

        let blend_face = b
            .faces_around_edge(spring_a_edge)
            .into_iter()
            .find(|f| *f != f1k)
            .ok_or(TopoError::Precondition("conic: no blend face"))?;
        b.attach_face_surface(blend_face, SurfaceGeom::Nurbs(strip), convex);
        b.attach_edge_curve(spring_a_edge, Curve3::Line(spring_a), true);
        b.attach_edge_curve(spring_b_edge, Curve3::Line(spring_b), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("conic: result invalid"))?;
        Ok(b)
    }

    /// Blend to a HOLD LINE (parity item 54, the parallel-hold rung):
    /// the spring on the hold support is PINNED to the line `setback`
    /// in from the edge; the radius FLOATS to keep the ball tangent to
    /// the other support. For a hold line parallel to the edge between
    /// planar supports the floating radius is CLOSED FORM: with the
    /// ball centre at hold - n_h r, tangency to the other plane gives
    /// r (1 - n_h . n_o) = dist(hold, other plane), so the blend is
    /// still an exact cylinder and the standard surgery applies.
    /// `hold_on_first` picks which adjacent face carries the hold.
    /// Curved / non-parallel holds and concave edges are follow-ups.
    pub fn fillet_edge_hold_line(
        &self,
        edge: EdgeKey,
        hold_on_first: bool,
        setback: f64,
    ) -> Result<Body, TopoError> {
        if !(setback.is_finite() && setback > 0.0) {
            return Err(TopoError::Precondition("hold: bad setback"));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("hold: edge needs two faces"));
        }
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "hold: non-planar support (follow-up)",
                ));
            }
        }
        let convex = self.edge_is_convex(edge).unwrap_or(true);
        if !convex {
            return Err(TopoError::Precondition("hold: concave edge (follow-up)"));
        }
        let (fh, fo) = if hold_on_first {
            (faces[0], faces[1])
        } else {
            (faces[1], faces[0])
        };
        let nh = self
            .face_outward_normal(fh)
            .ok_or(TopoError::Precondition("hold: face normal"))?;
        let no = self
            .face_outward_normal(fo)
            .ok_or(TopoError::Precondition("hold: face normal"))?;
        let (va_k, vb_k) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let pa = self.vertices.get(va_k).ok_or(TopoError::StaleKey)?.point;
        let pb = self.vertices.get(vb_k).ok_or(TopoError::StaleKey)?.point;
        let e = (pb - pa)
            .try_normalize()
            .ok_or(TopoError::Precondition("hold: degenerate edge"))?;
        // Hold line: `setback` in from the edge along the hold face
        // (away from the other support's outward side).
        let u_h = {
            let u = e
                .cross(nh)
                .try_normalize()
                .ok_or(TopoError::Precondition("hold: degenerate setback"))?;
            if u.dot(no) > 0.0 { u * -1.0 } else { u }
        };
        let h0 = pa + u_h * setback;
        let h1 = pb + u_h * setback;
        // Floating radius: centre = hold - nh r (into the material);
        // tangency to the other plane: d_o - centre.no = r.
        let d_o = no.dot(pa); // the edge lies on the other plane
        let gap = d_o - no.dot(h0);
        let denom = 1.0 - nh.dot(no);
        if denom.abs() < 1e-12 || gap / denom <= 0.0 {
            return Err(TopoError::Precondition("hold: no tangent ball"));
        }
        let r = gap / denom;
        let c0 = h0 - nh * r;
        let c1 = h1 - nh * r;
        let spine = Line3::new(c0, c1 - c0).map_err(|_| TopoError::Precondition("hold: spine"))?;
        let (sb0, sb1) = (c0 + no * r, c1 + no * r);
        let spring_h =
            Line3::new(h0, h1 - h0).map_err(|_| TopoError::Precondition("hold: spring h"))?;
        let spring_o =
            Line3::new(sb0, sb1 - sb0).map_err(|_| TopoError::Precondition("hold: spring o"))?;
        let frame = Frame3::from_z(c0, spine.dir)
            .map_err(|_| TopoError::Precondition("hold: cylinder frame"))?;
        let surface = Cylinder3 { frame, radius: r };

        // The standard surgery, hold face first.
        let mut b = self.clone();
        let (spring_h_edge, strip1, fhk, aa, ab) =
            b.imprint_spring_line(fh, edge, va_k, vb_k, &spring_h, nh)?;
        let (spring_o_edge, strip2, _fok, ba, bb) =
            b.imprint_spring_line(fo, edge, va_k, vb_k, &spring_o, no)?;
        let split_cap = |b: &mut Body,
                         v_corner: crate::entity::VertexKey,
                         a_end: crate::entity::VertexKey,
                         b_end: crate::entity::VertexKey|
         -> Result<(), TopoError> {
            let cap = b
                .faces_at_vertex(v_corner)
                .into_iter()
                .find(|f| *f != strip1 && *f != strip2)
                .ok_or(TopoError::Precondition("hold: no cap face"))?;
            let pc = b
                .vertices
                .get(v_corner)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let centre = c0 + spine.dir * ((pc - c0).dot(spine.dir));
            let pa_end = b
                .vertices
                .get(a_end)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let ex = (pa_end - centre)
                .try_normalize()
                .ok_or(TopoError::Precondition("hold: arc axis"))?;
            let arc = keel_geom::curve::Circle3::new(centre, ex, spine.dir.cross(ex), r)
                .map_err(|_| TopoError::Precondition("hold: bad arc"))?;
            b.split_blend_cap(cap, a_end, b_end, Curve3::Circle(arc))?;
            Ok(())
        };
        split_cap(&mut b, va_k, aa, ba)?;
        split_cap(&mut b, vb_k, ab, bb)?;

        b.kef(edge)?;
        let e_a = b
            .edge_between(aa, va_k)
            .ok_or(TopoError::Precondition("hold: no A spring stub"))?;
        b.kef(e_a)?;
        let spur_a = b
            .edge_between(va_k, ba)
            .ok_or(TopoError::Precondition("hold: no A spur"))?;
        b.kev(spur_a)?;
        let e_b = b
            .edge_between(ab, vb_k)
            .ok_or(TopoError::Precondition("hold: no B spring stub"))?;
        b.kef(e_b)?;
        let spur_b = b
            .edge_between(vb_k, bb)
            .ok_or(TopoError::Precondition("hold: no B spur"))?;
        b.kev(spur_b)?;

        let blend_face = b
            .faces_around_edge(spring_h_edge)
            .into_iter()
            .find(|f| *f != fhk)
            .ok_or(TopoError::Precondition("hold: no blend face"))?;
        b.attach_face_surface(
            blend_face,
            SurfaceGeom::Analytic(Surface3::Cylinder(surface)),
            convex,
        );
        b.attach_edge_curve(spring_h_edge, Curve3::Line(spring_h), true);
        b.attach_edge_curve(spring_o_edge, Curve3::Line(spring_o), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("hold: result invalid"))?;
        Ok(b)
    }

    /// FACE-FACE blend (parity item 50, the parallel-supports rung):
    /// roll the ball between two PARALLEL planar faces that do not
    /// share an edge, consuming the `wall` face joining them -- the
    /// rounded-end slab. The radius is FORCED to half the gap (the
    /// only ball tangent to both); the blend is the exact half-
    /// cylinder along the wall midline. General face-face
    /// (non-parallel supports, free radius, marched spine) is the
    /// follow-up; this rung's spine is a line by symmetry.
    pub fn blend_face_face(&self, wall: crate::entity::FaceKey) -> Result<Body, TopoError> {
        let wall_edges = self.face_edge_set(wall);
        if wall_edges.len() != 4 {
            return Err(TopoError::Precondition("faceface: wall needs 4 edges"));
        }
        let mut nbrs: Vec<(crate::entity::FaceKey, EdgeKey, Vec3)> = Vec::new();
        for &e in &wall_edges {
            let Some(nb) = self.faces_around_edge(e).into_iter().find(|&f| f != wall) else {
                continue;
            };
            let Some(n) = self.face_outward_normal(nb) else {
                continue;
            };
            nbrs.push((nb, e, n));
        }
        if nbrs.len() != 4 {
            return Err(TopoError::Precondition("faceface: wall not interior"));
        }
        let mut pair: Option<(usize, usize)> = None;
        for i in 0..4 {
            for j in (i + 1)..4 {
                if nbrs[i].2.dot(nbrs[j].2) < -1.0 + 1e-9 {
                    pair = Some((i, j));
                }
            }
        }
        let (i, j) = pair.ok_or(TopoError::Precondition(
            "faceface: no parallel opposing supports (general case is a follow-up)",
        ))?;
        let (f1, e1, n1) = nbrs[i];
        let (f2, e2, n2) = nbrs[j];
        let _ = n2;
        let p1 = self
            .face_outer_loop_points(f1)
            .first()
            .copied()
            .ok_or(TopoError::Precondition("faceface: support point"))?;
        let p2 = self
            .face_outer_loop_points(f2)
            .first()
            .copied()
            .ok_or(TopoError::Precondition("faceface: support point"))?;
        let gap = (p2 - p1).dot(n1).abs();
        if gap < 1e-9 {
            return Err(TopoError::Precondition("faceface: zero gap"));
        }
        let r = 0.5 * gap;
        let n_wall = self
            .face_outward_normal(wall)
            .ok_or(TopoError::Precondition("faceface: wall normal"))?;
        let (va1, vb1) = self.edges.get(e1).ok_or(TopoError::StaleKey)?.bounds;
        let (va2, vb2) = self.edges.get(e2).ok_or(TopoError::StaleKey)?.bounds;
        let q1a = self.vertices.get(va1).ok_or(TopoError::StaleKey)?.point;
        let q1b = self.vertices.get(vb1).ok_or(TopoError::StaleKey)?.point;
        let q2a = self.vertices.get(va2).ok_or(TopoError::StaleKey)?.point;
        let spring1 = Line3::new(q1a - n_wall * r, q1b - q1a)
            .map_err(|_| TopoError::Precondition("faceface: spring 1"))?;
        let spring2 = Line3::new(q2a - n_wall * r, q1b - q1a)
            .map_err(|_| TopoError::Precondition("faceface: spring 2"))?;
        let axis_dir = spring1.dir;
        let axis_pt = spring1.origin - n1 * r;
        let frame = Frame3::from_z(axis_pt, axis_dir)
            .map_err(|_| TopoError::Precondition("faceface: cylinder frame"))?;
        let surface = Cylinder3 { frame, radius: r };

        let mut b = self.clone();
        let (spring1_edge, strip1, f1k, aa, ab) =
            b.imprint_spring_line(f1, e1, va1, vb1, &spring1, n1)?;
        let (spring2_edge, strip2, _f2k, ba_raw, bb_raw) =
            b.imprint_spring_line(f2, e2, va2, vb2, &spring2, n2)?;
        let axis_h = |b: &Body, v: crate::entity::VertexKey| -> f64 {
            b.vertices
                .get(v)
                .map(|x| (x.point - axis_pt).dot(axis_dir))
                .unwrap_or(0.0)
        };
        // Pair the spring ends (and wall corners) per cap by axis height.
        let h_a = axis_h(&b, aa);
        let (ba, bb) = if (axis_h(&b, ba_raw) - h_a).abs() <= (axis_h(&b, bb_raw) - h_a).abs() {
            (ba_raw, bb_raw)
        } else {
            (bb_raw, ba_raw)
        };
        let corners = [va1, vb1, va2, vb2];
        let h_b = axis_h(&b, ab);
        let cap_a: Vec<_> = corners
            .iter()
            .copied()
            .filter(|&v| (axis_h(&b, v) - h_a).abs() < 1e-9)
            .collect();
        let cap_b: Vec<_> = corners
            .iter()
            .copied()
            .filter(|&v| (axis_h(&b, v) - h_b).abs() < 1e-9)
            .collect();
        if cap_a.len() != 2 || cap_b.len() != 2 {
            return Err(TopoError::Precondition("faceface: cap corner pairing"));
        }

        // Split each cap along the half-circle arc between the spring
        // ends; record the signed sweep so the OUTWARD half is taken (a
        // pi arc is direction-ambiguous from its endpoints).
        let split_cap = |b: &mut Body,
                         s1_end: crate::entity::VertexKey,
                         s2_end: crate::entity::VertexKey|
         -> Result<(), TopoError> {
            let cap = b
                .faces_at_vertex(s1_end)
                .into_iter()
                .find(|f| {
                    *f != strip1 && *f != strip2 && *f != f1k && {
                        b.faces_at_vertex(s2_end).contains(f)
                    }
                })
                .ok_or(TopoError::Precondition("faceface: no cap face"))?;
            let p_end = b
                .vertices
                .get(s1_end)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let centre = axis_pt + axis_dir * ((p_end - axis_pt).dot(axis_dir));
            let ex = (p_end - centre)
                .try_normalize()
                .ok_or(TopoError::Precondition("faceface: arc axis"))?;
            let ey = axis_dir.cross(ex);
            let arc = keel_geom::curve::Circle3::new(centre, ex, ey, r)
                .map_err(|_| TopoError::Precondition("faceface: bad arc"))?;
            let new_edge = b.split_blend_cap(cap, s1_end, s2_end, Curve3::Circle(arc))?;
            let sweep = if ey.dot(n_wall) >= 0.0 {
                core::f64::consts::PI
            } else {
                -core::f64::consts::PI
            };
            let (b0, _) = b.edges.get(new_edge).ok_or(TopoError::StaleKey)?.bounds;
            let signed = if b0 == s1_end { sweep } else { -sweep };
            b.set_edge_arc_sweep(new_edge, signed);
            Ok(())
        };
        split_cap(&mut b, aa, ba)?;
        split_cap(&mut b, ab, bb)?;

        // Dissolve: strips + wall merge into one face, then each cap's
        // corner chain (stub, wall-cap edge, stub) dies.
        b.kef(e1)?;
        b.kef(e2)?;
        for (s1_end, s2_end, cs) in [(aa, ba, &cap_a), (ab, bb, &cap_b)] {
            let (c1, c2) = (cs[0], cs[1]);
            let (k1, k2) = if b.edge_between(s1_end, c1).is_some() {
                (c1, c2)
            } else {
                (c2, c1)
            };
            let stub1 = b
                .edge_between(s1_end, k1)
                .ok_or(TopoError::Precondition("faceface: no stub 1"))?;
            b.kef(stub1)?;
            let wall_cap = b
                .edge_between(k1, k2)
                .ok_or(TopoError::Precondition("faceface: no wall-cap edge"))?;
            b.kev(wall_cap)?;
            let stub2 = b
                .edge_between(k2, s2_end)
                .ok_or(TopoError::Precondition("faceface: no stub 2"))?;
            b.kev(stub2)?;
        }

        let blend_face = b
            .faces_around_edge(spring1_edge)
            .into_iter()
            .find(|f| *f != f1k)
            .ok_or(TopoError::Precondition("faceface: no blend face"))?;
        b.attach_face_surface(
            blend_face,
            SurfaceGeom::Analytic(Surface3::Cylinder(surface)),
            true,
        );
        b.attach_edge_curve(spring1_edge, Curve3::Line(spring1), true);
        b.attach_edge_curve(spring2_edge, Curve3::Line(spring2), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("faceface: result invalid"))?;
        Ok(b)
    }

    /// CLIFF-overflow blend (parity item 57; dossier 56 secs 1 and 7):
    /// when the requested radius is too wide for one support, the blend
    /// becomes HALF-TANGENT -- a radius-`r` cylinder tangent to the
    /// other support only, passing THROUGH the narrow support's far
    /// boundary edge `b` (Parasolid: "tangent to only one of the faces
    /// ... and runs along an edge in the other face"). The narrow
    /// support is consumed entirely; `b` stays SHARP, promoted to a
    /// shared edge between its outer neighbor and the blend. Exact:
    /// the center line sits on the tangent support's offset plane at
    /// distance `r` from `b` (closed-form quadratic). MVP: convex
    /// plane-plane edges, the narrow support a 4-sided face whose far
    /// edge is `b`, full-span cliff. Creeping cliffs and the smooth /
    /// notch / cap handlers are the dossier-56 follow-ups.
    pub fn fillet_edge_cliff(&self, edge: EdgeKey, radius: f64) -> Result<Body, TopoError> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(TopoError::Precondition("cliff: bad radius"));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("cliff: edge needs two faces"));
        }
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "cliff: non-planar support (follow-up)",
                ));
            }
        }
        let convex = self.edge_is_convex(edge).unwrap_or(true);
        if !convex {
            return Err(TopoError::Precondition("cliff: concave edge (follow-up)"));
        }
        let (va_k, vb_k) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let pa = self.vertices.get(va_k).ok_or(TopoError::StaleKey)?.point;
        let pb = self.vertices.get(vb_k).ok_or(TopoError::StaleKey)?.point;
        let e = (pb - pa)
            .try_normalize()
            .ok_or(TopoError::Precondition("cliff: degenerate edge"))?;
        // Which support is the NARROW (cliff) side: its extent
        // perpendicular to the edge is below the setback radius.
        let width_of = |f: crate::entity::FaceKey, n: Vec3| -> f64 {
            let u = e.cross(n);
            self.face_outer_loop_points(f)
                .iter()
                .map(|&p| (p - pa).dot(u).abs())
                .fold(0.0f64, f64::max)
        };
        let n0 = self
            .face_outward_normal(faces[0])
            .ok_or(TopoError::Precondition("cliff: face normal"))?;
        let n1 = self
            .face_outward_normal(faces[1])
            .ok_or(TopoError::Precondition("cliff: face normal"))?;
        let (w0, w1) = (width_of(faces[0], n0), width_of(faces[1], n1));
        let (f_t, n_t, f_c, n_c, w_c) = if w0 < radius && w1 >= radius {
            (faces[1], n1, faces[0], n0, w0)
        } else if w1 < radius && w0 >= radius {
            (faces[0], n0, faces[1], n1, w1)
        } else if w0 >= radius && w1 >= radius {
            return Err(TopoError::Precondition(
                "cliff: no overflow at this radius (use fillet_edge)",
            ));
        } else {
            return Err(TopoError::Precondition(
                "cliff: both supports overflow (notch territory, follow-up)",
            ));
        };
        // The cliff edge b: the narrow face's far boundary edge.
        let b_edge = self
            .face_edge_set(f_c)
            .into_iter()
            .find(|&be| {
                if be == edge {
                    return false;
                }
                let Some(ed) = self.edges.get(be) else {
                    return false;
                };
                ed.bounds.0 != va_k
                    && ed.bounds.0 != vb_k
                    && ed.bounds.1 != va_k
                    && ed.bounds.1 != vb_k
            })
            .ok_or(TopoError::Precondition(
                "cliff: no far edge on the narrow face",
            ))?;
        let (qb0, _qb1) = self.edges.get(b_edge).ok_or(TopoError::StaleKey)?.bounds;
        let b0 = self.vertices.get(qb0).ok_or(TopoError::StaleKey)?.point;
        // Center line C = pa + s u_c + t w with (C - pa).n_t = -r and
        // |C - B|^2 = r^2 in the section (B = pa + u_c w_c on b's line).
        let u_c = {
            let u = e
                .cross(n_c)
                .try_normalize()
                .ok_or(TopoError::Precondition("cliff: degenerate frame"))?;
            if (b0 - pa).dot(u) < 0.0 { u * -1.0 } else { u }
        };
        let w_dir = u_c.cross(e);
        let (a_lin, b_lin) = (u_c.dot(n_t), w_dir.dot(n_t));
        let (s_sol, t_sol) = if b_lin.abs() > 1e-9 {
            let k0 = -radius / b_lin;
            let k1 = -a_lin / b_lin;
            let qa = 1.0 + k1 * k1;
            let qb = -2.0 * w_c + 2.0 * k0 * k1;
            let qc = w_c * w_c + k0 * k0 - radius * radius;
            let disc = qb * qb - 4.0 * qa * qc;
            if disc < 0.0 {
                return Err(TopoError::Precondition("cliff: no tangent cylinder"));
            }
            let s1 = (-qb + disc.sqrt()) / (2.0 * qa);
            let s2 = (-qb - disc.sqrt()) / (2.0 * qa);
            let s = if (0.0..=w_c + radius).contains(&s1) {
                s1
            } else {
                s2
            };
            (s, k0 + k1 * s)
        } else if a_lin.abs() > 1e-9 {
            let s = -radius / a_lin;
            let t2 = radius * radius - (s - w_c) * (s - w_c);
            if t2 < 0.0 {
                return Err(TopoError::Precondition("cliff: no tangent cylinder"));
            }
            let sign = if w_dir.dot(n_c) > 0.0 { -1.0 } else { 1.0 };
            (s, sign * t2.sqrt())
        } else {
            return Err(TopoError::Precondition("cliff: degenerate supports"));
        };
        let c0 = pa + u_c * s_sol + w_dir * t_sol;
        let spring_pt = c0 + n_t * radius;
        let spring_t =
            Line3::new(spring_pt, e).map_err(|_| TopoError::Precondition("cliff: spring"))?;
        let frame =
            Frame3::from_z(c0, e).map_err(|_| TopoError::Precondition("cliff: cylinder frame"))?;
        let surface = Cylinder3 { frame, radius };

        let mut b = self.clone();
        let (spring_edge, _strip_t, ftk, aa, ab) =
            b.imprint_spring_line(f_t, edge, va_k, vb_k, &spring_t, n_t)?;
        // Per cap: split from the spring end to the EXISTING b corner
        // along the cliff arc (short span: tangent foot to b is < pi).
        let split_cap = |b: &mut Body,
                         v_corner: crate::entity::VertexKey,
                         s_end: crate::entity::VertexKey|
         -> Result<(), TopoError> {
            let pc = b
                .vertices
                .get(v_corner)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let (x0, x1) = b.edges.get(b_edge).ok_or(TopoError::StaleKey)?.bounds;
            let (p0, p1) = (
                b.vertices.get(x0).ok_or(TopoError::StaleKey)?.point,
                b.vertices.get(x1).ok_or(TopoError::StaleKey)?.point,
            );
            let b_corner = if ((p0 - pc).dot(e)).abs() <= ((p1 - pc).dot(e)).abs() {
                x0
            } else {
                x1
            };
            let cap = b
                .faces_at_vertex(v_corner)
                .into_iter()
                .find(|f| *f != f_c && *f != ftk && b.faces_at_vertex(b_corner).contains(f))
                .ok_or(TopoError::Precondition("cliff: no cap face"))?;
            let lp = b
                .faces
                .get(cap)
                .map(|f| f.loops[0])
                .ok_or(TopoError::StaleKey)?;
            let fin_a = b.fin_ending_at_vertex(lp, s_end)?;
            let fin_b = b.fin_ending_at_vertex(lp, b_corner)?;
            let split = b.split_face(fin_a, fin_b, None)?;
            if let Some(surf) = b.faces.get(cap).and_then(|f| f.surface)
                && let Some(nf) = b.faces.get_mut(split.face_new)
            {
                nf.surface = Some(surf);
            }
            let centre = c0 + e * ((pc - c0).dot(e));
            let s_pt = b
                .vertices
                .get(s_end)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let ex = (s_pt - centre)
                .try_normalize()
                .ok_or(TopoError::Precondition("cliff: arc axis"))?;
            let arc = keel_geom::curve::Circle3::new(centre, ex, e.cross(ex), radius)
                .map_err(|_| TopoError::Precondition("cliff: bad arc"))?;
            b.attach_edge_curve(split.edge, Curve3::Circle(arc), true);
            Ok(())
        };
        split_cap(&mut b, va_k, aa)?;
        split_cap(&mut b, vb_k, ab)?;

        // Dissolve: merge the tangent strip with the WHOLE narrow face
        // (consumed), then per cap kill the corner chain.
        b.kef(edge)?;
        for (s_end, vk) in [(aa, va_k), (ab, vb_k)] {
            let stub = b
                .edge_between(s_end, vk)
                .ok_or(TopoError::Precondition("cliff: no spring stub"))?;
            b.kef(stub)?;
            // The e-corner now hangs as a spur on the narrow face's
            // cap-side edge.
            let spur = {
                let mut found = None;
                let eks: Vec<EdgeKey> = b.edges.iter().map(|(k, _)| k).collect();
                for ek in eks {
                    let Some(ed) = b.edges.get(ek) else { continue };
                    if ed.bounds.0 == vk || ed.bounds.1 == vk {
                        found = Some(ek);
                        break;
                    }
                }
                found.ok_or(TopoError::Precondition("cliff: no corner spur"))?
            };
            b.kev(spur)?;
        }

        let blend_face = b
            .faces_around_edge(spring_edge)
            .into_iter()
            .find(|f| *f != ftk)
            .ok_or(TopoError::Precondition("cliff: no blend face"))?;
        b.attach_face_surface(
            blend_face,
            SurfaceGeom::Analytic(Surface3::Cylinder(surface)),
            true,
        );
        b.attach_edge_curve(spring_edge, Curve3::Line(spring_t), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("cliff: result invalid"))?;
        Ok(b)
    }

    /// PARTIAL-SPAN blend (parity item 55; dossier 54 Q3/Q4/Q5): round
    /// only the interior interval `[t0, t1]` of a plane-plane edge
    /// (parameters along the edge, both strictly interior). The blend
    /// tapers linearly from `radius` at `t0` to ZERO at `t1` (the
    /// feathered runout: an EXACT CONE with apex at the vanishing
    /// vertex, dossier 54 Q5); the `t0` end is a SQUARE STOP closed by
    /// a planar transverse cross face. The sharp edge RESUMES on both
    /// sides. End surgery = the dossier's cap-into-supports: split the
    /// edge at the stations, spur the spring landings into the face
    /// interiors, imprint the converging spring lines, dissolve the
    /// interior sharp interval, close the stop with the cross arc --
    /// the face between the arc and the corner IS the planar stop
    /// face, kept. Constant-radius bodies with a taper-start join, two
    /// square stops, and curved spines are the dossier-54 ladder.
    pub fn fillet_edge_partial(
        &self,
        edge: EdgeKey,
        t0: f64,
        t1: f64,
        radius: f64,
    ) -> Result<Body, TopoError> {
        let ok = radius.is_finite()
            && radius > 0.0
            && t0.is_finite()
            && t1.is_finite()
            && t0 > 0.0
            && t1 < 1.0
            && t0 < t1;
        if !ok {
            return Err(TopoError::Precondition(
                "partial: need 0 < t0 < t1 < 1 and radius > 0",
            ));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("partial: edge needs two faces"));
        }
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "partial: non-planar support (follow-up)",
                ));
            }
        }
        let convex = self.edge_is_convex(edge).unwrap_or(true);
        if !convex {
            return Err(TopoError::Precondition("partial: concave edge (follow-up)"));
        }
        let (f1, f2) = (faces[0], faces[1]);
        let n1 = self
            .face_outward_normal(f1)
            .ok_or(TopoError::Precondition("partial: face normal"))?;
        let n2 = self
            .face_outward_normal(f2)
            .ok_or(TopoError::Precondition("partial: face normal"))?;
        let (va_k, vb_k) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let pa = self.vertices.get(va_k).ok_or(TopoError::StaleKey)?.point;
        let pb = self.vertices.get(vb_k).ok_or(TopoError::StaleKey)?.point;
        let _ = va_k;
        let e = (pb - pa)
            .try_normalize()
            .ok_or(TopoError::Precondition("partial: degenerate edge"))?;
        let p_at = |t: f64| pa + (pb - pa) * t;
        let (q0_pt, q1_pt) = (p_at(t0), p_at(t1));
        // Section center at t0: inside the material, distance r from
        // both support planes, in the section plane (3x3 solve via the
        // scalar triple product).
        let c0 = {
            let det = n1.dot(n2.cross(e));
            if det.abs() < 1e-12 {
                return Err(TopoError::Precondition("partial: parallel supports"));
            }
            let rhs = n2.cross(e) * (-radius) + e.cross(n1) * (-radius);
            q0_pt + rhs * (1.0 / det)
        };
        // The runout cone: apex at the vanishing vertex q1.
        let d_spine = (c0 - q1_pt).norm();
        if d_spine <= radius {
            return Err(TopoError::Precondition("partial: runout too steep"));
        }
        let axis = (c0 - q1_pt) * (1.0 / d_spine);
        let alpha = (radius / d_spine).asin();
        let frame =
            Frame3::from_z(c0, axis).map_err(|_| TopoError::Precondition("partial: cone frame"))?;
        let cone = keel_geom::surface::Cone3 {
            frame,
            radius: radius / alpha.cos(),
            half_angle: alpha,
        };
        let p1_pt = c0 + n1 * radius;
        let p2_pt = c0 + n2 * radius;

        let mut b = self.clone();
        let s0 = b.split_edge(edge, q0_pt)?;
        let q0 = s0.vertex;
        let right_piece = if b
            .edges
            .get(s0.edge_a)
            .map(|x| x.bounds.0 == vb_k || x.bounds.1 == vb_k)
            == Some(true)
        {
            s0.edge_a
        } else {
            s0.edge_b
        };
        let s1 = b.split_edge(right_piece, q1_pt)?;
        let q1 = s1.vertex;
        let e_mid = b
            .edge_between(q0, q1)
            .ok_or(TopoError::Precondition("partial: no interior interval"))?;

        // Per support: spur the landing into the interior, imprint the
        // converging spring line.
        let mut spring_edges = Vec::new();
        let mut keeps = Vec::new();
        for (f, p_pt) in [(f1, p1_pt), (f2, p2_pt)] {
            let lp = b
                .faces
                .get(f)
                .map(|x| x.loops[0])
                .ok_or(TopoError::StaleKey)?;
            let fin_q0 = b.fin_ending_at_vertex(lp, q0)?;
            let spur = b.mev(crate::euler::MevSite::AfterFin(fin_q0), p_pt)?;
            let p_v = spur.vertex;
            let fin_p = b.fin_ending_at_vertex(lp, p_v)?;
            let fin_q1 = b.fin_ending_at_vertex(lp, q1)?;
            let split = b.split_face(fin_p, fin_q1, None)?;
            if let Some(surf) = b.faces.get(f).and_then(|x| x.surface)
                && let Some(nf) = b.faces.get_mut(split.face_new)
            {
                nf.surface = Some(surf);
            }
            let q1p = b
                .vertices
                .get(q1)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            if let Ok(line) = Line3::new(p_pt, q1p - p_pt) {
                b.attach_edge_curve(split.edge, Curve3::Line(line), true);
            }
            let around = b.faces_around_edge(e_mid);
            let keep = if around.contains(&split.face_new) {
                split.face_old
            } else {
                split.face_new
            };
            spring_edges.push(split.edge);
            keeps.push(keep);
        }
        // Merge the two trim slivers across the interior sharp interval.
        b.kef(e_mid)?;
        // Close the stop: the cross arc splits the merged sliver into
        // the BLEND (apex side) and the planar STOP face.
        let t_face = b
            .faces_around_edge(spring_edges[0])
            .into_iter()
            .find(|f| !keeps.contains(f))
            .ok_or(TopoError::Precondition("partial: no merged sliver"))?;
        let lp_t = b
            .faces
            .get(t_face)
            .map(|x| x.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let vertex_near =
            |b: &Body, ek: EdgeKey, p: Vec3| -> Result<crate::entity::VertexKey, TopoError> {
                let (x0, x1) = b.edges.get(ek).ok_or(TopoError::StaleKey)?.bounds;
                let d0 = b
                    .vertices
                    .get(x0)
                    .map(|v| (v.point - p).norm())
                    .unwrap_or(f64::INFINITY);
                let d1 = b
                    .vertices
                    .get(x1)
                    .map(|v| (v.point - p).norm())
                    .unwrap_or(f64::INFINITY);
                Ok(if d0 <= d1 { x0 } else { x1 })
            };
        let p1_v = vertex_near(&b, spring_edges[0], p1_pt)?;
        let p2_v = vertex_near(&b, spring_edges[1], p2_pt)?;
        let fin_p1 = b.fin_ending_at_vertex(lp_t, p1_v)?;
        let fin_p2 = b.fin_ending_at_vertex(lp_t, p2_v)?;
        let stop_split = b.split_face(fin_p1, fin_p2, None)?;
        let ex = (p1_pt - c0) * (1.0 / radius);
        if let Ok(arc) = keel_geom::curve::Circle3::new(c0, ex, e.cross(ex), radius) {
            b.attach_edge_curve(stop_split.edge, Curve3::Circle(arc), true);
        }
        let blend_face = b
            .faces_around_edge(spring_edges[0])
            .into_iter()
            .find(|f| !keeps.contains(f))
            .ok_or(TopoError::Precondition("partial: no blend face"))?;
        let stop_face = if stop_split.face_new == blend_face {
            stop_split.face_old
        } else {
            stop_split.face_new
        };
        b.attach_face_surface(
            blend_face,
            SurfaceGeom::Analytic(Surface3::Cone(cone)),
            true,
        );
        let stop_frame =
            Frame3::from_z(q0_pt, e).map_err(|_| TopoError::Precondition("partial: stop frame"))?;
        b.attach_face_surface(
            stop_face,
            SurfaceGeom::Analytic(Surface3::Plane(keel_geom::surface::Plane3::new(stop_frame))),
            true,
        );

        b.validate()
            .map_err(|_| TopoError::Precondition("partial: result invalid"))?;
        Ok(b)
    }
}

impl Body {
    /// MITRED corner of two equal-radius fillets (parity item 56;
    /// dossier 55 milestone 1): blend two convex plane-plane edges
    /// sharing a corner vertex and one support face, joining the two
    /// cylinder blends along their EXACT intersection ellipse (for
    /// equal radii the cylinders' intersection in the bisector plane:
    /// E(theta) = M + (W - M) cos theta + (X - M) sin theta with M the
    /// spine crossing, X the top-spring crossing, W the side-spring
    /// crossing on the shared vertical edge -- on BOTH cylinders for
    /// every theta). Surgery: the partial-span SPUR trick imprints the
    /// crossing top springs; the sides imprint boundary-to-boundary
    /// (side 2 reusing the W vertex); far ends get the standard cap
    /// treatment; and the old corner vertex RELOCATES onto the ellipse,
    /// its two corner edges becoming exact ellipse sub-arcs shared by
    /// the blends. Unequal radii (marched SSI), chains longer than two,
    /// and roll-over junctions are the dossier-55 ladder.
    pub fn mitre_fillet_corner(
        &self,
        e1: EdgeKey,
        e2: EdgeKey,
        radius: f64,
    ) -> Result<Body, TopoError> {
        if !(radius.is_finite() && radius > 0.0) || e1 == e2 {
            return Err(TopoError::Precondition("mitre: bad input"));
        }
        let fs1 = self.faces_around_edge(e1);
        let fs2 = self.faces_around_edge(e2);
        if fs1.len() != 2 || fs2.len() != 2 {
            return Err(TopoError::Precondition("mitre: edges need two faces"));
        }
        let f_top = *fs1
            .iter()
            .find(|f| fs2.contains(f))
            .ok_or(TopoError::Precondition("mitre: no shared support"))?;
        let s1 = *fs1
            .iter()
            .find(|&&f| f != f_top)
            .ok_or(TopoError::Precondition("mitre: side 1"))?;
        let s2 = *fs2
            .iter()
            .find(|&&f| f != f_top)
            .ok_or(TopoError::Precondition("mitre: side 2"))?;
        for &f in &[f_top, s1, s2] {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "mitre: non-planar support (follow-up)",
                ));
            }
        }
        // Shared corner vertex and far ends.
        let (a0, a1) = self.edges.get(e1).ok_or(TopoError::StaleKey)?.bounds;
        let (b0, b1) = self.edges.get(e2).ok_or(TopoError::StaleKey)?.bounds;
        let (corner, far1) = if b0 == a0 || b1 == a0 {
            (a0, a1)
        } else if b0 == a1 || b1 == a1 {
            (a1, a0)
        } else {
            return Err(TopoError::Precondition("mitre: edges share no vertex"));
        };
        let far2 = if b0 == corner { b1 } else { b0 };
        // Blend geometry per edge (exact cylinders).
        let blend1 = self.blend_cylinder_for_edge(e1, radius)?;
        let blend2 = self.blend_cylinder_for_edge(e2, radius)?;
        let n_top = self
            .face_outward_normal(f_top)
            .ok_or(TopoError::Precondition("mitre: top normal"))?;
        // Which spring of each blend lies on the TOP plane.
        let on_top = |l: &Line3| -> bool {
            let p_top = self
                .face_outer_loop_points(f_top)
                .first()
                .copied()
                .unwrap_or(l.origin);
            ((l.origin - p_top).dot(n_top)).abs() < 1e-9
        };
        let (sp1_top, sp1_side) = if on_top(&blend1.spring_a) {
            (blend1.spring_a, blend1.spring_b)
        } else {
            (blend1.spring_b, blend1.spring_a)
        };
        let (sp2_top, sp2_side) = if on_top(&blend2.spring_a) {
            (blend2.spring_a, blend2.spring_b)
        } else {
            (blend2.spring_b, blend2.spring_a)
        };
        // X: the top springs' crossing (solve in the top plane).
        let x_pt = {
            let d = sp1_top.origin - sp2_top.origin;
            let (u, v) = (sp1_top.dir, sp2_top.dir);
            let (uu, uv, vv) = (u.dot(u), u.dot(v), v.dot(v));
            let det = uu * vv - uv * uv;
            if det.abs() < 1e-12 {
                return Err(TopoError::Precondition("mitre: parallel top springs"));
            }
            let s = (uv * d.dot(v) - vv * d.dot(u)) / det;
            sp1_top.origin + u * s
        };
        // M: the spine crossing (closest-point midpoint, must meet).
        let m_pt = {
            let d = blend1.spine.origin - blend2.spine.origin;
            let (u, v) = (blend1.spine.dir, blend2.spine.dir);
            let (uu, uv, vv) = (u.dot(u), u.dot(v), v.dot(v));
            let det = uu * vv - uv * uv;
            if det.abs() < 1e-12 {
                return Err(TopoError::Precondition("mitre: parallel spines"));
            }
            let s = (uv * d.dot(v) - vv * d.dot(u)) / det;
            let t = (uu * d.dot(v) - uv * d.dot(u)) / det;
            let (p, q) = (blend1.spine.origin + u * s, blend2.spine.origin + v * t);
            if (p - q).norm() > 1e-9 {
                return Err(TopoError::Precondition(
                    "mitre: spines do not meet (unequal radii?)",
                ));
            }
            p
        };
        // The shared vertical edge V at the corner (bounds s1 and s2).
        let _v_edge = self
            .face_edge_set(s1)
            .into_iter()
            .find(|&ek| {
                ek != e1
                    && self.faces_around_edge(ek).contains(&s2)
                    && self
                        .edges
                        .get(ek)
                        .map(|x| x.bounds.0 == corner || x.bounds.1 == corner)
                        == Some(true)
            })
            .ok_or(TopoError::Precondition("mitre: no shared vertical edge"))?;
        let _corner_pt = self.vertices.get(corner).ok_or(TopoError::StaleKey)?.point;

        let mut b = self.clone();
        // Side 1: the standard boundary-to-boundary imprint (its corner
        // boundary IS the vertical edge; the crossing there is W).
        let n_s1 = b
            .face_outward_normal(s1)
            .ok_or(TopoError::Precondition("mitre: side normal"))?;
        let n_s2 = b
            .face_outward_normal(s2)
            .ok_or(TopoError::Precondition("mitre: side normal"))?;
        let (spr1s_edge, strip1, s1k, a1far, w_v) =
            b.imprint_spring_line(s1, e1, far1, corner, &sp1_side, n_s1)?;
        let w_pt = b.vertices.get(w_v).ok_or(TopoError::StaleKey)?.point;
        // Side 2: manual (reuse the W vertex on the now-split V).
        let (spr2s_edge, strip2, s2k, a2far) = {
            let e_far = b
                .boundary_edge_at_vertex_excluding(s2, far2, e2)
                .ok_or(TopoError::Precondition("mitre: no far cap edge"))?;
            let m = n_s2.cross(sp2_side.dir);
            let far_pt = b
                .line_crosses_edge(e_far, sp2_side.origin, m)
                .ok_or(TopoError::Precondition("mitre: spring misses far cap"))?;
            let sv = b.split_edge(e_far, far_pt)?;
            let lp = b
                .faces
                .get(s2)
                .map(|f| f.loops[0])
                .ok_or(TopoError::StaleKey)?;
            let fa = b.fin_ending_at_vertex(lp, sv.vertex)?;
            let fb = b.fin_ending_at_vertex(lp, w_v)?;
            let split = b.split_face(fa, fb, None)?;
            if let Some(surf) = b.faces.get(s2).and_then(|f| f.surface)
                && let Some(nf) = b.faces.get_mut(split.face_new)
            {
                nf.surface = Some(surf);
            }
            b.attach_edge_curve(split.edge, Curve3::Line(sp2_side), true);
            let (strip, kept) = if b.face_has_edge(split.face_new, e2) {
                (split.face_new, split.face_old)
            } else {
                (split.face_old, split.face_new)
            };
            (split.edge, strip, kept, sv.vertex)
        };
        // Top: split the two far boundaries, spur corner -> X, then the
        // two crossing spring imprints.
        let (t1far_v, t2far_v) = {
            let ef1 = b
                .boundary_edge_at_vertex_excluding(f_top, far1, e1)
                .ok_or(TopoError::Precondition("mitre: top far edge 1"))?;
            let m1 = n_top.cross(sp1_top.dir);
            let p1 = b
                .line_crosses_edge(ef1, sp1_top.origin, m1)
                .ok_or(TopoError::Precondition("mitre: top spring 1 misses"))?;
            let sv1 = b.split_edge(ef1, p1)?;
            let ef2 = b
                .boundary_edge_at_vertex_excluding(f_top, far2, e2)
                .ok_or(TopoError::Precondition("mitre: top far edge 2"))?;
            let m2 = n_top.cross(sp2_top.dir);
            let p2 = b
                .line_crosses_edge(ef2, sp2_top.origin, m2)
                .ok_or(TopoError::Precondition("mitre: top spring 2 misses"))?;
            let sv2 = b.split_edge(ef2, p2)?;
            (sv1.vertex, sv2.vertex)
        };
        let lp_top = b
            .faces
            .get(f_top)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let fin_c = b.fin_ending_at_vertex(lp_top, corner)?;
        let spur = b.mev(crate::euler::MevSite::AfterFin(fin_c), x_pt)?;
        let x_v = spur.vertex;
        let fin_x = b.fin_ending_at_vertex(lp_top, x_v)?;
        let fin_t1 = b.fin_ending_at_vertex(lp_top, t1far_v)?;
        let split_t1 = b.split_face(fin_x, fin_t1, None)?;
        if let Some(surf) = b.faces.get(f_top).and_then(|f| f.surface)
            && let Some(nf) = b.faces.get_mut(split_t1.face_new)
        {
            nf.surface = Some(surf);
        }
        b.attach_edge_curve(split_t1.edge, Curve3::Line(sp1_top), true);
        // The piece containing far2's split vertex hosts the second cut.
        let host2 = if b.faces_at_vertex(t2far_v).contains(&split_t1.face_new) {
            split_t1.face_new
        } else {
            split_t1.face_old
        };
        let lp_h2 = b
            .faces
            .get(host2)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let fin_x2 = b.fin_ending_at_vertex(lp_h2, x_v)?;
        let fin_t2 = b.fin_ending_at_vertex(lp_h2, t2far_v)?;
        let split_t2 = b.split_face(fin_x2, fin_t2, None)?;
        if let Some(surf) = b.faces.get(host2).and_then(|f| f.surface)
            && let Some(nf) = b.faces.get_mut(split_t2.face_new)
        {
            nf.surface = Some(surf);
        }
        b.attach_edge_curve(split_t2.edge, Curve3::Line(sp2_top), true);

        // Far-end caps (the standard fillet end treatment).
        let split_cap = |b: &mut Body,
                         v_far: crate::entity::VertexKey,
                         spine: &Line3,
                         t_end: crate::entity::VertexKey,
                         s_end: crate::entity::VertexKey,
                         sharp: EdgeKey|
         -> Result<(), TopoError> {
            // The cap is the face at the far vertex NOT carrying the
            // sharp edge (both trim pieces do).
            let cap = b
                .faces_at_vertex(v_far)
                .into_iter()
                .find(|&f| !b.face_has_edge(f, sharp))
                .ok_or(TopoError::Precondition("mitre: no far cap"))?;
            let pc = b
                .vertices
                .get(v_far)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let centre = spine.origin + spine.dir * ((pc - spine.origin).dot(spine.dir));
            let p_t = b
                .vertices
                .get(t_end)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let ex = (p_t - centre)
                .try_normalize()
                .ok_or(TopoError::Precondition("mitre: arc axis"))?;
            let arc = keel_geom::curve::Circle3::new(centre, ex, spine.dir.cross(ex), radius)
                .map_err(|_| TopoError::Precondition("mitre: bad arc"))?;
            b.split_blend_cap(cap, t_end, s_end, Curve3::Circle(arc))?;
            Ok(())
        };
        split_cap(&mut b, far1, &blend1.spine, t1far_v, a1far, e1)?;
        split_cap(&mut b, far2, &blend2.spine, t2far_v, a2far, e2)?;
        let _ = (strip1, strip2, split_t1, split_t2);

        // Dissolve: merge each top trim with its side strip across the
        // sharp edge, then the far corner chains.
        b.kef(e1)?;
        b.kef(e2)?;
        for (t_end, v_far, s_end) in [(t1far_v, far1, a1far), (t2far_v, far2, a2far)] {
            let stub = b
                .edge_between(t_end, v_far)
                .ok_or(TopoError::Precondition("mitre: no far stub"))?;
            b.kef(stub)?;
            let spur_e = b
                .edge_between(v_far, s_end)
                .ok_or(TopoError::Precondition("mitre: no far spur"))?;
            b.kev(spur_e)?;
        }

        // The corner: relocate the old vertex onto the mitre ellipse and
        // recurve its two edges (the top spur and the vertical stub) as
        // exact ellipse sub-arcs shared by the two blends.
        let a_vec = w_pt - m_pt;
        let b_vec = x_pt - m_pt;
        if a_vec.dot(b_vec).abs() > 1e-9 * a_vec.norm() * b_vec.norm() {
            return Err(TopoError::Precondition(
                "mitre: non-perpendicular configuration (follow-up)",
            ));
        }
        let mid = m_pt
            + a_vec * core::f64::consts::FRAC_1_SQRT_2
            + b_vec * core::f64::consts::FRAC_1_SQRT_2;
        if let Some(v) = b.vertices.get_mut(corner) {
            v.point = mid;
        }
        let ellipse = keel_geom::curve::Ellipse3::new(
            m_pt,
            a_vec
                .try_normalize()
                .ok_or(TopoError::Precondition("mitre: ellipse axis"))?,
            b_vec
                .try_normalize()
                .ok_or(TopoError::Precondition("mitre: ellipse axis"))?,
            a_vec.norm(),
            b_vec.norm(),
        )
        .map_err(|_| TopoError::Precondition("mitre: bad ellipse"))?;
        let spur_edge = b
            .edge_between(corner, x_v)
            .ok_or(TopoError::Precondition("mitre: no corner spur"))?;
        b.attach_edge_curve(spur_edge, Curve3::Ellipse(ellipse), true);
        let v_stub = b
            .edge_between(corner, w_v)
            .ok_or(TopoError::Precondition("mitre: no corner stub"))?;
        b.attach_edge_curve(v_stub, Curve3::Ellipse(ellipse), true);

        // Attach the blend cylinders (each face = its trim + strip,
        // reachable from its spring-side edge).
        let blend1_face = b
            .faces_around_edge(spr1s_edge)
            .into_iter()
            .find(|f| *f != s1k)
            .ok_or(TopoError::Precondition("mitre: no blend 1 face"))?;
        let blend2_face = b
            .faces_around_edge(spr2s_edge)
            .into_iter()
            .find(|f| *f != s2k)
            .ok_or(TopoError::Precondition("mitre: no blend 2 face"))?;
        b.attach_face_surface(
            blend1_face,
            SurfaceGeom::Analytic(Surface3::Cylinder(blend1.surface.clone())),
            true,
        );
        b.attach_face_surface(
            blend2_face,
            SurfaceGeom::Analytic(Surface3::Cylinder(blend2.surface.clone())),
            true,
        );

        b.validate()
            .map_err(|_| TopoError::Precondition("mitre: result invalid"))?;
        Ok(b)
    }

    /// Round a convex trihedral corner with the exact SPHERE-OCTANT
    /// vertex blend (item 51, dossier 53 Q1): the three edge fillets
    /// are exact cylinders whose axes all pass through M, the common
    /// point of the three inward offset planes, and they are capped by
    /// the spherical patch centred at M (radius r) tangent to each
    /// support at q_i = M + n_i r. Each cylinder meets the sphere along
    /// the exact quarter circle of radius r about M in the plane
    /// perpendicular to its edge.
    ///
    /// Scope: three mutually perpendicular planar supports (the cube
    /// corner), equal radius. Setbacks, unequal radii, and oblique
    /// dihedrals are follow-ups (dossier 53 Q2/Q3).
    pub fn fillet_corner_octant(
        &self,
        corner: crate::entity::VertexKey,
        radius: f64,
    ) -> Result<Body, TopoError> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(TopoError::Precondition("octant: bad radius"));
        }
        let corner_pt = self.vertices.get(corner).ok_or(TopoError::StaleKey)?.point;
        // The three edges meeting at the corner, with their far vertices.
        let edges: Vec<(EdgeKey, crate::entity::VertexKey)> = self
            .edges
            .iter()
            .filter_map(|(k, e)| {
                if e.bounds.0 == corner {
                    Some((k, e.bounds.1))
                } else if e.bounds.1 == corner {
                    Some((k, e.bounds.0))
                } else {
                    None
                }
            })
            .collect();
        if edges.len() != 3 {
            return Err(TopoError::Precondition("octant: corner needs three edges"));
        }
        // The three support faces; each carries exactly two of the edges.
        let mut faces: Vec<crate::entity::FaceKey> = Vec::new();
        for &(e, _) in &edges {
            for f in self.faces_around_edge(e) {
                if !faces.contains(&f) {
                    faces.push(f);
                }
            }
        }
        if faces.len() != 3 {
            return Err(TopoError::Precondition("octant: corner needs three faces"));
        }
        let mut n = [Vec3::ZERO; 3];
        let mut face_edges = [[0usize; 2]; 3];
        for (i, &f) in faces.iter().enumerate() {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "octant: non-planar support (follow-up)",
                ));
            }
            n[i] = self
                .face_outward_normal(f)
                .ok_or(TopoError::Precondition("octant: support normal"))?;
            let mut slot = 0usize;
            for (j, &(e, _)) in edges.iter().enumerate() {
                if self.faces_around_edge(e).contains(&f) {
                    if slot == 2 {
                        return Err(TopoError::Precondition("octant: face/edge incidence"));
                    }
                    face_edges[i][slot] = j;
                    slot += 1;
                }
            }
            if slot != 2 {
                return Err(TopoError::Precondition("octant: face/edge incidence"));
            }
        }
        // Exact octant: mutually perpendicular supports only.
        for i in 0..3 {
            if n[i].dot(n[(i + 1) % 3]).abs() > 1e-9 {
                return Err(TopoError::Precondition(
                    "octant: non-perpendicular supports (follow-up)",
                ));
            }
        }
        // M: the three INWARD offset planes' (n_i . x = n_i . p_i - r)
        // common point, by triple products.
        let m_pt = {
            let mut d = [0.0f64; 3];
            for i in 0..3 {
                let p = self
                    .face_outer_loop_points(faces[i])
                    .first()
                    .copied()
                    .ok_or(TopoError::Precondition("octant: empty support"))?;
                d[i] = n[i].dot(p) - radius;
            }
            let den = n[0].dot(n[1].cross(n[2]));
            if den.abs() < 1e-12 {
                return Err(TopoError::Precondition("octant: degenerate supports"));
            }
            (n[1].cross(n[2]) * d[0] + n[2].cross(n[0]) * d[1] + n[0].cross(n[1]) * d[2])
                * (1.0 / den)
        };
        // q_i: the sphere's tangency foot on support i (both of that
        // face's springs meet there).
        let q = [
            m_pt + n[0] * radius,
            m_pt + n[1] * radius,
            m_pt + n[2] * radius,
        ];
        // Per-edge exact cylinders (validates convexity); their axes all
        // pass through M. Edge directions point away from the corner.
        let mut blends = Vec::with_capacity(3);
        let mut dir = [Vec3::ZERO; 3];
        for (j, &(e, vfar)) in edges.iter().enumerate() {
            blends.push(self.blend_cylinder_for_edge(e, radius)?);
            let far_pt = self.vertices.get(vfar).ok_or(TopoError::StaleKey)?.point;
            dir[j] = (far_pt - corner_pt)
                .try_normalize()
                .ok_or(TopoError::Precondition("octant: zero edge"))?;
        }

        let mut b = self.clone();
        // Per-face surgery (the mitre-top treatment on all three faces):
        // split the two far boundaries at the spring crossings, spur the
        // corner to q_i, then two face splits along the springs.
        let mut far_v = [[corner; 2]; 3];
        let mut q_v = [corner; 3];
        let mut spring_edge = [[EdgeKey::sentinel(); 2]; 3];
        let mut kept = [crate::entity::FaceKey::sentinel(); 3];
        for i in 0..3 {
            let f = faces[i];
            let springs = [
                Line3 {
                    origin: q[i],
                    dir: dir[face_edges[i][0]],
                },
                Line3 {
                    origin: q[i],
                    dir: dir[face_edges[i][1]],
                },
            ];
            for s in 0..2 {
                let (e, vfar) = edges[face_edges[i][s]];
                let ef = b
                    .boundary_edge_at_vertex_excluding(f, vfar, e)
                    .ok_or(TopoError::Precondition("octant: no far edge"))?;
                let m = n[i].cross(springs[s].dir);
                let p = b.line_crosses_edge(ef, springs[s].origin, m).ok_or(
                    TopoError::Precondition("octant: spring misses far edge (overflow?)"),
                )?;
                far_v[i][s] = b.split_edge(ef, p)?.vertex;
            }
            let lp = b
                .faces
                .get(f)
                .map(|x| x.loops[0])
                .ok_or(TopoError::StaleKey)?;
            let fin_c = b.fin_ending_at_vertex(lp, corner)?;
            q_v[i] = b.mev(crate::euler::MevSite::AfterFin(fin_c), q[i])?.vertex;
            let fin_q = b.fin_ending_at_vertex(lp, q_v[i])?;
            let fin_t = b.fin_ending_at_vertex(lp, far_v[i][0])?;
            let split1 = b.split_face(fin_q, fin_t, None)?;
            if let Some(surf) = b.faces.get(f).and_then(|x| x.surface)
                && let Some(nf) = b.faces.get_mut(split1.face_new)
            {
                nf.surface = Some(surf);
            }
            b.attach_edge_curve(split1.edge, Curve3::Line(springs[0]), true);
            spring_edge[i][0] = split1.edge;
            let host = if b.faces_at_vertex(far_v[i][1]).contains(&split1.face_new) {
                split1.face_new
            } else {
                split1.face_old
            };
            let lp_h = b
                .faces
                .get(host)
                .map(|x| x.loops[0])
                .ok_or(TopoError::StaleKey)?;
            let fin_q2 = b.fin_ending_at_vertex(lp_h, q_v[i])?;
            let fin_t2 = b.fin_ending_at_vertex(lp_h, far_v[i][1])?;
            let split2 = b.split_face(fin_q2, fin_t2, None)?;
            if let Some(surf) = b.faces.get(host).and_then(|x| x.surface)
                && let Some(nf) = b.faces.get_mut(split2.face_new)
            {
                nf.surface = Some(surf);
            }
            b.attach_edge_curve(split2.edge, Curve3::Line(springs[1]), true);
            spring_edge[i][1] = split2.edge;
            // The kept support piece carries neither sharp edge.
            kept[i] = if b.face_has_edge(split2.face_new, edges[face_edges[i][1]].0) {
                split2.face_old
            } else {
                split2.face_new
            };
        }
        // The (face, slot) incidences of each edge, for the far caps and
        // the cylinder bands.
        let edge_inc = |j: usize| -> [(usize, usize); 2] {
            let mut out = [(usize::MAX, usize::MAX); 2];
            let mut k = 0usize;
            for (i, fe) in face_edges.iter().enumerate() {
                for (s, &jj) in fe.iter().enumerate() {
                    if jj == j {
                        out[k] = (i, s);
                        k += 1;
                    }
                }
            }
            out
        };
        // Far-end caps: the standard fillet end treatment per edge.
        for (j, &(e, vfar)) in edges.iter().enumerate() {
            let [(ia, sa), (ib, sb)] = edge_inc(j);
            let (t_end, s_end) = (far_v[ia][sa], far_v[ib][sb]);
            let cap = b
                .faces_at_vertex(vfar)
                .into_iter()
                .find(|&f| !b.face_has_edge(f, e))
                .ok_or(TopoError::Precondition("octant: no far cap"))?;
            let spine = &blends[j].spine;
            let pc = b
                .vertices
                .get(vfar)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let centre = spine.origin + spine.dir * ((pc - spine.origin).dot(spine.dir));
            let p_t = b
                .vertices
                .get(t_end)
                .map(|x| x.point)
                .ok_or(TopoError::StaleKey)?;
            let ex = (p_t - centre)
                .try_normalize()
                .ok_or(TopoError::Precondition("octant: arc axis"))?;
            let arc = keel_geom::curve::Circle3::new(centre, ex, spine.dir.cross(ex), radius)
                .map_err(|_| TopoError::Precondition("octant: bad far arc"))?;
            b.split_blend_cap(cap, t_end, s_end, Curve3::Circle(arc))?;
        }
        // Dissolve the sharp edges (each merges its two support trims
        // into one blend face), then the far corner chains.
        for &(e, _) in &edges {
            b.kef(e)?;
        }
        for (j, &(_, vfar)) in edges.iter().enumerate() {
            let [(ia, sa), (ib, sb)] = edge_inc(j);
            let stub = b
                .edge_between(far_v[ia][sa], vfar)
                .ok_or(TopoError::Precondition("octant: no far stub"))?;
            b.kef(stub)?;
            let spur = b
                .edge_between(vfar, far_v[ib][sb])
                .ok_or(TopoError::Precondition("octant: no far spur"))?;
            b.kev(spur)?;
        }
        // Carve the octant out of each blend face: split along the exact
        // quarter circle (centre M, radius r, plane perpendicular to the
        // edge) between the two q points, and give the band its cylinder.
        for (j, blend) in blends.iter().enumerate() {
            let [(ia, sa), (ib, _)] = edge_inc(j);
            let blend_face = b
                .faces_around_edge(spring_edge[ia][sa])
                .into_iter()
                .find(|&f| f != kept[ia])
                .ok_or(TopoError::Precondition("octant: no blend face"))?;
            let lp = b
                .faces
                .get(blend_face)
                .map(|x| x.loops[0])
                .ok_or(TopoError::StaleKey)?;
            let fa = b.fin_ending_at_vertex(lp, q_v[ia])?;
            let fb = b.fin_ending_at_vertex(lp, q_v[ib])?;
            let split = b.split_face(fa, fb, None)?;
            let arc = keel_geom::curve::Circle3::new(m_pt, n[ia], n[ib], radius)
                .map_err(|_| TopoError::Precondition("octant: bad corner arc"))?;
            b.attach_edge_curve(split.edge, Curve3::Circle(arc), true);
            let band = if b.faces_at_vertex(corner).contains(&split.face_new) {
                split.face_old
            } else {
                split.face_new
            };
            b.attach_face_surface(
                band,
                SurfaceGeom::Analytic(Surface3::Cylinder(blend.surface.clone())),
                true,
            );
        }
        // Merge the three corner pieces into the single octant face:
        // kill two spurs (each borders two pieces), then the last spur
        // (now dangling) together with the old corner vertex.
        for &qv in q_v.iter().take(2) {
            let spur = b
                .edge_between(corner, qv)
                .ok_or(TopoError::Precondition("octant: no corner spur"))?;
            b.kef(spur)?;
        }
        let spur = b
            .edge_between(corner, q_v[2])
            .ok_or(TopoError::Precondition("octant: no corner spur"))?;
        b.kev(spur)?;
        // The octant face is the one touching all three q vertices.
        let oct = b
            .faces_at_vertex(q_v[0])
            .into_iter()
            .find(|&f| {
                b.faces_at_vertex(q_v[1]).contains(&f) && b.faces_at_vertex(q_v[2]).contains(&f)
            })
            .ok_or(TopoError::Precondition("octant: no sphere face"))?;
        let frame = keel_geom::surface::Frame3::from_z(m_pt, n[0] + n[1] + n[2])
            .map_err(|_| TopoError::Precondition("octant: sphere frame"))?;
        let sphere = keel_geom::surface::Sphere3::new(frame, radius)
            .map_err(|_| TopoError::Precondition("octant: bad sphere"))?;
        b.attach_face_surface(oct, SurfaceGeom::Analytic(Surface3::Sphere(sphere)), true);

        b.validate()
            .map_err(|_| TopoError::Precondition("octant: result invalid"))?;
        Ok(b)
    }
}

/// The exact ellipse where a cone meets a plane (the variable-radius
/// fillet's cap arcs): expand the cone's implicit quadric in plane
/// coordinates, solve the 2x2 center system, and diagonalize the
/// quadratic part (closed-form eigen) for the axes. `None` when the
/// section is not an ellipse (plane too steep vs the half-angle).
fn cone_plane_ellipse(
    cone: &keel_geom::surface::Cone3,
    plane_p: Vec3,
    plane_n: Vec3,
) -> Option<keel_geom::curve::Ellipse3> {
    let apex = {
        // The cone's apex: radius(v) = radius + v tan(alpha) hits 0 at
        // v = -radius / tan(alpha).
        let t = cone.half_angle.tan();
        if t.abs() < 1e-15 {
            return None;
        }
        cone.frame.origin + cone.frame.z * (-cone.radius / t)
    };
    let u = cone.frame.z;
    let ta2 = cone.half_angle.tan().powi(2);
    let n = plane_n.try_normalize()?;
    // 2D basis in the plane.
    let seed = if n.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let g1 = (seed - n * seed.dot(n)).try_normalize()?;
    let g2 = n.cross(g1);
    // F(w) = |w|^2 - (1 + ta2)(w.u)^2 with w = X - apex,
    // X = q0 + x g1 + y g2 (q0 = the plane point).
    let w0 = plane_p - apex;
    let c = 1.0 + ta2;
    let (u1, u2, u0) = (g1.dot(u), g2.dot(u), w0.dot(u));
    // Quadratic coefficients of F in (x, y):
    let axx = 1.0 - c * u1 * u1;
    let ayy = 1.0 - c * u2 * u2;
    let axy = -c * u1 * u2; // coefficient of 2xy is 2*axy
    let bx = w0.dot(g1) - c * u0 * u1;
    let by = w0.dot(g2) - c * u0 * u2;
    let f0 = w0.dot(w0) - c * u0 * u0;
    // Center: gradient zero -> [axx axy; axy ayy] [x;y] = -[bx; by].
    let det = axx * ayy - axy * axy;
    if det.abs() < 1e-15 {
        return None;
    }
    let cx = (-bx * ayy + by * axy) / det;
    let cy = (-by * axx + bx * axy) / det;
    let fc = axx * cx * cx + 2.0 * axy * cx * cy + ayy * cy * cy + 2.0 * (bx * cx + by * cy) + f0;
    // Eigen of [[axx, axy],[axy, ayy]] (closed form).
    let tr = axx + ayy;
    let disc = ((axx - ayy) * 0.5).hypot(axy);
    let (l1, l2) = (tr * 0.5 + disc, tr * 0.5 - disc);
    if l1 <= 0.0 || l2 <= 0.0 || fc >= 0.0 {
        return None; // not an ellipse section
    }
    let (a2, b2) = (-fc / l1, -fc / l2);
    // Eigenvector for l1.
    let (ex, ey) = if axy.abs() > 1e-15 {
        (l1 - ayy, axy)
    } else if axx >= ayy {
        (1.0, 0.0)
    } else {
        (0.0, 1.0)
    };
    let el = (ex * ex + ey * ey).sqrt();
    let (ex, ey) = (ex / el, ey / el);
    let center = plane_p + g1 * cx + g2 * cy;
    let ax1 = g1 * ex + g2 * ey;
    let ax2 = n.cross(ax1);
    keel_geom::curve::Ellipse3::new(center, ax1, ax2, a2.sqrt(), b2.sqrt()).ok()
}

/// A recognized rolling-ball blend face (parity item 58).
#[derive(Clone, Debug)]
pub struct RecognizedBlend {
    pub face: crate::entity::FaceKey,
    pub radius: f64,
    /// The two support faces the blend is tangent to.
    pub supports: (crate::entity::FaceKey, crate::entity::FaceKey),
    /// The spring edges shared with the supports.
    pub spring_edges: (EdgeKey, EdgeKey),
}

impl Body {
    /// Blend recognition (item 58; dossier 28 sec 6.3 records the
    /// support data the inverse needs): cylindrical (edge-fillet) or
    /// toroidal (cap-rim) faces TANGENT to exactly two distinct
    /// neighbor faces along shared edges. Tangency is normals-parallel
    /// at the shared-edge midpoint within `angular_tol`. The radius is
    /// the cylinder radius / torus minor radius.
    pub fn recognize_blends(&self, angular_tol: f64) -> Vec<RecognizedBlend> {
        let mut out = Vec::new();
        for face in self.face_keys() {
            // A cylinder blend's springs are straight edges parallel to
            // its axis; tangent ARC junctions (the smooth run into a
            // vertex-blend sphere, item 51) are not springs.
            let (radius, axis) = match self.face_surface3(face) {
                Some(Surface3::Cylinder(c)) => (c.radius, Some(c.frame.z)),
                Some(Surface3::Torus(t)) => (t.minor, None),
                _ => continue,
            };
            let mut tangents: Vec<(crate::entity::FaceKey, EdgeKey)> = Vec::new();
            for e in self.face_edge_set(face) {
                let partners: Vec<crate::entity::FaceKey> = self
                    .faces_around_edge(e)
                    .into_iter()
                    .filter(|&f| f != face)
                    .collect();
                let [p] = partners[..] else { continue };
                let Some(mid) = self.edge_midpoint_point(e) else {
                    continue;
                };
                if let Some(ax) = axis {
                    let along_axis = self.edges.get(e).map(|ed| ed.bounds).is_some_and(|(a, b)| {
                        match (self.vertices.get(a), self.vertices.get(b)) {
                            (Some(va), Some(vb)) => match (vb.point - va.point).try_normalize() {
                                Some(d) => d.cross(ax).norm() <= 1e-7,
                                None => false,
                            },
                            _ => false,
                        }
                    });
                    if !along_axis {
                        continue;
                    }
                }
                let (Some(na), Some(nb)) =
                    (self.face_normal_at(face, mid), self.face_normal_at(p, mid))
                else {
                    continue;
                };
                if na.cross(nb).norm() <= angular_tol {
                    tangents.push((p, e));
                }
            }
            if tangents.len() == 2 && tangents[0].0 != tangents[1].0 {
                out.push(RecognizedBlend {
                    face,
                    radius,
                    supports: (tangents[0].0, tangents[1].0),
                    spring_edges: (tangents[0].1, tangents[1].1),
                });
            }
        }
        out
    }

    /// Suppress ALL recognized blends, LEAVES FIRST (the
    /// Venkataraman-Sohoni chain ordering, dossiers 03 sec 2.2 / 15 sec
    /// 1.4): a blend whose face serves as another blend's SUPPORT is a
    /// junction and must wait until its dependents are gone, or the
    /// dependent's re-intersection target vanishes. Recognition re-runs
    /// after every removal (the topology changed). Returns
    /// `(removed, remaining)`: a nonzero `remaining` means those blends
    /// DECLINED (an unsupported unblend class), with the body left at
    /// the last VALID state, never a partial surgery.
    pub fn unblend_all(&mut self, tol: f64) -> (usize, usize) {
        use std::collections::BTreeSet;
        let mut removed = 0usize;
        let cap = self.recognize_blends(1e-6).len() + 4;
        let mut declined: BTreeSet<crate::entity::FaceKey> = BTreeSet::new();
        for _ in 0..cap {
            let recs = self.recognize_blends(1e-6);
            // Leaves: blends NOT serving as any other blend's support.
            let leaves: Vec<crate::entity::FaceKey> = recs
                .iter()
                .filter(|r| !declined.contains(&r.face))
                .filter(|r| {
                    !recs.iter().any(|o| {
                        o.face != r.face && (o.supports.0 == r.face || o.supports.1 == r.face)
                    })
                })
                .map(|r| r.face)
                .collect();
            let mut progressed = false;
            for f in leaves {
                if self.unblend(f, tol).is_ok() {
                    removed += 1;
                    progressed = true;
                    // A removal can unlock previously declined blends.
                    declined.clear();
                    break;
                }
                declined.insert(f);
            }
            if !progressed {
                break;
            }
        }
        let remaining = self.recognize_blends(1e-6).len();
        (removed, remaining)
    }

    /// Unblend (item 59): remove a recognized PLANE-PLANE edge fillet,
    /// restoring the sharp edge at the support planes' intersection --
    /// the exact inverse of `fillet_edge`. Surgery runs on a clone and
    /// commits only if the result validates with mass == mesh;
    /// otherwise DECLINES and the body is untouched. Toroidal and
    /// curved-support unblends are follow-ups.
    pub fn unblend(&mut self, blend: crate::entity::FaceKey, tol: f64) -> Result<(), TopoError> {
        let rec_list = self.recognize_blends(1e-6);
        let Some(info) = rec_list.iter().find(|r| r.face == blend) else {
            return Err(TopoError::Precondition("unblend: face is not a blend"));
        };
        let (sa, sb) = info.supports;
        let (Some(Surface3::Plane(pa)), Some(Surface3::Plane(pb))) =
            (self.face_surface3(sa), self.face_surface3(sb))
        else {
            return Err(TopoError::Precondition(
                "unblend: only plane-plane fillets (MVP)",
            ));
        };
        // The sharp line L = plane intersection.
        let (n1, d1) = (pa.frame.z, pa.frame.z.dot(pa.frame.origin));
        let (n2, d2) = (pb.frame.z, pb.frame.z.dot(pb.frame.origin));
        let dir = n1.cross(n2);
        let dd = dir.dot(dir);
        if dd < 1e-18 {
            return Err(TopoError::Precondition("unblend: parallel supports"));
        }
        let p_on_l = (n2.cross(dir) * d1 + dir.cross(n1) * d2) * (1.0 / dd);
        let radius = info.radius;
        let v_before = self.mesh_volume();

        let mut work = self.clone();
        let mut rec = work.begin_op();
        let (e1, e2) = info.spring_edges;
        // The blend face's other edges are the end arcs; their partner
        // faces are the caps whose planes cut L at the sharp vertices.
        let arcs: Vec<EdgeKey> = work
            .face_edge_set(blend)
            .into_iter()
            .filter(|&e| e != e1 && e != e2)
            .collect();
        if arcs.len() != 2 {
            return Err(TopoError::Precondition("unblend: need two end arcs"));
        }
        // Per arc: the sharp vertex = L cut by the cap plane; move both
        // arc endpoints there and merge them.
        for &arc in &arcs {
            let cap = work
                .faces_around_edge(arc)
                .into_iter()
                .find(|&f| f != blend)
                .ok_or(TopoError::Precondition("unblend: arc without cap"))?;
            let Some(Surface3::Plane(pc)) = work.face_surface3(cap) else {
                return Err(TopoError::Precondition("unblend: non-planar cap (MVP)"));
            };
            let nc = pc.frame.z;
            let dc = nc.dot(pc.frame.origin);
            let denom = dir.dot(nc);
            if denom.abs() < 1e-12 {
                return Err(TopoError::Precondition("unblend: cap parallel to edge"));
            }
            let t = (dc - p_on_l.dot(nc)) / denom;
            let sharp = p_on_l + dir * t;
            let (va, vb) = work
                .edges
                .get(arc)
                .map(|e| e.bounds)
                .ok_or(TopoError::StaleKey)?;
            if let Some(v) = work.vertices.get_mut(va) {
                v.point = sharp;
            }
            if let Some(v) = work.vertices.get_mut(vb) {
                v.point = sharp;
            }
            // Splice the arc's fins out of their loop rings and drop it.
            let fins: Vec<_> = work
                .edges
                .get(arc)
                .map(|e| e.radial.clone())
                .unwrap_or_default();
            for fk in fins {
                let (prev, next, owner) = match work.fins.get(fk) {
                    Some(f) => (f.prev, f.next, f.owner),
                    None => continue,
                };
                if let Some(pf) = work.fins.get_mut(prev) {
                    pf.next = next;
                }
                if let Some(nf) = work.fins.get_mut(next) {
                    nf.prev = prev;
                }
                if let Some(l) = work.loops.get_mut(owner)
                    && l.fin == Some(fk)
                {
                    l.fin = Some(next);
                }
                if let Some(id) = work.fins.get(fk).map(|f| f.id) {
                    work.unregister(&mut rec, id);
                }
                work.fins.remove(fk);
            }
            if let Some(id) = work.edges.get(arc).map(|e| e.id) {
                work.unregister(&mut rec, id);
            }
            work.edges.remove(arc);
            // Merge vb into va: repoint every edge bound.
            if va != vb {
                let eks: Vec<EdgeKey> = work.edges.iter().map(|(k, _)| k).collect();
                for ek in eks {
                    if let Some(e) = work.edges.get_mut(ek) {
                        if e.bounds.0 == vb {
                            e.bounds.0 = va;
                        }
                        if e.bounds.1 == vb {
                            e.bounds.1 = va;
                        }
                    }
                }
                if let Some(id) = work.vertices.get(vb).map(|v| v.id) {
                    work.unregister(&mut rec, id);
                }
                work.vertices.remove(vb);
            }
        }
        let _ = rec.finish();
        // Fix surviving vertices' fin pointers (their fins may have died).
        let vkeys: Vec<_> = work.vertices.iter().map(|(k, _)| k).collect();
        for v in vkeys {
            let cur = work.vertices.get(v).and_then(|x| x.fin);
            let stale = cur.map(|f| !work.fins.contains(f)).unwrap_or(false);
            if stale {
                let repl = work
                    .fins
                    .iter()
                    .map(|(k, _)| k)
                    .find(|&fk| work.fin_start_vertex(fk) == Some(v));
                if let Some(x) = work.vertices.get_mut(v) {
                    x.fin = repl;
                }
            }
        }
        // Capture support A's surface BEFORE the merge, then kill e1:
        // the blend face merges with support A (kef keeps one of them).
        let a_surface = work.faces.get(sa).and_then(|f| f.surface);
        work.kef(e1)
            .map_err(|_| TopoError::Precondition("unblend: kef failed"))?;
        let survivor = if work.faces.contains(sa) { sa } else { blend };
        if let Some((sk, sense)) = a_surface
            && let Some(f) = work.faces.get_mut(survivor)
        {
            f.surface = Some((sk, sense));
        }
        // The sharp edge: e2 now joins the merged support-A face and
        // support B; carry the intersection line.
        let (b0, b1) = work
            .edges
            .get(e2)
            .map(|e| e.bounds)
            .ok_or(TopoError::StaleKey)?;
        let (p0, p1) = (
            work.vertices.get(b0).map(|v| v.point),
            work.vertices.get(b1).map(|v| v.point),
        );
        let mut edge_len = None;
        if let (Some(p0), Some(p1)) = (p0, p1)
            && let Ok(line) = keel_geom::curve::Line3::new(p0, p1 - p0)
        {
            edge_len = Some((p1 - p0).norm());
            work.attach_edge_curve(e2, Curve3::Line(line), true);
        }
        // Honesty gates (tiered).
        if work.validate().is_err() {
            return Err(TopoError::Precondition(
                "unblend: result invalid (declined)",
            ));
        }
        let mv = work.mesh_volume();
        let all_planar = work
            .face_keys()
            .iter()
            .all(|&f| matches!(work.face_surface3(f), Some(Surface3::Plane(_))));
        match (all_planar, work.mass_properties()) {
            (true, Ok(m)) => {
                // Strong gate: the all-planar mass == mesh identity
                // (polygonal tessellation is exact).
                if !m.volume.is_finite()
                    || (m.volume - mv).abs() > tol.max(1e-6) * (1.0 + m.volume.abs())
                {
                    return Err(TopoError::Precondition(
                        "unblend: result inconsistent (declined)",
                    ));
                }
            }
            (true, Err(_)) => {
                return Err(TopoError::Precondition("unblend: massprops failed"));
            }
            (false, _) => {
                // The candidate still carries CURVED faces (other
                // blends), where the mesh is chordal and cannot equal
                // the analytic mass exactly (the projected-bounds
                // integral now SUCCEEDS there, so the dispatch is by
                // face type, not by massprops failure). The gate is
                // the EXACT WEDGE ORACLE: this unblend must change the
                // mesh volume by exactly the removed fillet's analytic
                // wedge, r^2 (cot(theta/2) - (pi - theta)/2) per unit
                // length at interior dihedral theta; the untouched
                // faces' tessellation error CANCELS in the difference,
                // leaving only the removed band's own chordal error
                // (a couple of percent at default density).
                let Some(len) = edge_len else {
                    return Err(TopoError::Precondition(
                        "unblend: no straight sharp edge for the wedge oracle",
                    ));
                };
                let theta = core::f64::consts::PI - n1.dot(n2).clamp(-1.0, 1.0).acos();
                let half = 0.5 * theta;
                if half.tan().abs() < 1e-12 {
                    return Err(TopoError::Precondition("unblend: degenerate dihedral"));
                }
                let wedge = radius
                    * radius
                    * (1.0 / half.tan() - 0.5 * (core::f64::consts::PI - theta))
                    * len;
                let delta = mv - v_before;
                let ok = (delta - wedge).abs() <= 0.02 * wedge.abs().max(1e-12)
                    || (delta + wedge).abs() <= 0.02 * wedge.abs().max(1e-12);
                if !ok {
                    return Err(TopoError::Precondition(
                        "unblend: wedge oracle mismatch (declined)",
                    ));
                }
            }
        }
        *self = work;
        Ok(())
    }

    /// All edges bounding a face (every loop's fin walk).
    fn face_edge_set(&self, face: crate::entity::FaceKey) -> Vec<EdgeKey> {
        let mut out = Vec::new();
        for lk in self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default()
        {
            for e in self.ring_edges(lk) {
                if !out.contains(&e) {
                    out.push(e);
                }
            }
        }
        out
    }

    /// A point in the middle of an edge (curve midpoint when present,
    /// else the chord midpoint).
    fn edge_midpoint_point(&self, e: EdgeKey) -> Option<Vec3> {
        let ed = self.edges.get(e)?;
        let p0 = self.vertices.get(ed.bounds.0)?.point;
        let p1 = self.vertices.get(ed.bounds.1)?.point;
        Some((p0 + p1) * 0.5)
    }

    /// The face's outward normal at (the projection of) `p`.
    fn face_normal_at(&self, face: crate::entity::FaceKey, p: Vec3) -> Option<Vec3> {
        let s = self.face_surface3(face)?;
        let pr = s.project(p).ok()?;
        let lg = s.local_geometry(pr.u, pr.v).ok()?;
        let sense = self
            .faces
            .get(face)
            .and_then(|f| f.surface)
            .map(|(_, s)| s)
            .unwrap_or(true);
        Some(if sense { lg.normal } else { lg.normal * -1.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The top-right edge of an axis-aligned block (faces with outward
    /// normals +z and +x), as in chamfer's test.
    fn top_right_edge(b: &Body) -> EdgeKey {
        let ekeys: Vec<EdgeKey> = b.edges.iter().map(|(k, _)| k).collect();
        for e in ekeys {
            let fs = b.faces_around_edge(e);
            if fs.len() != 2 {
                continue;
            }
            let (Some(a), Some(c)) = (b.face_outward_normal(fs[0]), b.face_outward_normal(fs[1]))
            else {
                continue;
            };
            if (a.z > 0.9 || c.z > 0.9) && (a.x > 0.9 || c.x > 0.9) {
                return e;
            }
        }
        panic!("no top-right edge");
    }

    #[test]
    fn g2_blend_has_zero_spring_curvature() {
        // Item 60: the quintic section's curvature is ~0 AT the springs
        // (matching the flat supports: true G2) and nonzero mid-strip;
        // the G1 circular fillet by contrast jumps to 1/r = 2 at its
        // spring. Volume sits between the chamfer chord and the sharp
        // box.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let g2 = b.fillet_edge_g2(e, 0.5, 0.4).unwrap();
        assert!(g2.validate().is_ok(), "g2 blend invalid");
        let v = g2.mesh_volume();
        assert!(
            v > 7.74 && v < 8.0,
            "g2 volume {v} must sit between chamfer and sharp"
        );
        // The strip's principal curvature near the springs vs mid.
        let strip_face = g2
            .face_keys()
            .into_iter()
            .find(|&f| matches!(g2.face_surface_geom(f), Some(SurfaceGeom::Nurbs(_))))
            .expect("g2 strip face");
        let Some(SurfaceGeom::Nurbs(strip)) = g2.face_surface_geom(strip_face) else {
            unreachable!()
        };
        let kmax = |u: f64| -> f64 {
            let lg = strip.local_geometry(u, 0.5).unwrap();
            lg.k1.abs().max(lg.k2.abs())
        };
        assert!(
            kmax(0.001) < 1e-2 && kmax(0.999) < 1e-2,
            "spring curvature must vanish ({}, {})",
            kmax(0.001),
            kmax(0.999)
        );
        assert!(
            kmax(0.5) > 0.5,
            "mid-strip curvature must be real ({})",
            kmax(0.5)
        );
    }

    #[test]
    fn mitre_joins_two_fillets_on_the_exact_ellipse() {
        // Item 56 (dossier 55 milestone 1): box [0,2]^3, blend the two
        // top edges at the (2,2,2) corner with r = 0.5, mitred. The two
        // cylinder blends meet along their exact intersection ellipse;
        // recognition finds BOTH blends at r = 0.5; the removed volume
        // = two straight wedges + the corner (grid oracle: kept iff
        // inside either cylinder or off-quadrant).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let find_edge = |fa: Vec3, fb: Vec3| -> EdgeKey {
            b.edges
                .iter()
                .map(|(k, _)| k)
                .find(|&k| {
                    let fs = b.faces_around_edge(k);
                    fs.len() == 2 && {
                        let (Some(na), Some(nb)) =
                            (b.face_outward_normal(fs[0]), b.face_outward_normal(fs[1]))
                        else {
                            return false;
                        };
                        (na - fa).norm() < 1e-9 && (nb - fb).norm() < 1e-9
                            || (na - fb).norm() < 1e-9 && (nb - fa).norm() < 1e-9
                    }
                })
                .expect("edge")
        };
        let e1 = find_edge(Vec3::new(0., 0., 1.), Vec3::new(1., 0., 0.)); // top ^ x=2
        let e2 = find_edge(Vec3::new(0., 0., 1.), Vec3::new(0., 1., 0.)); // top ^ y=2
        let m = b.mitre_fillet_corner(e1, e2, 0.5).unwrap();
        assert!(m.validate().is_ok(), "mitred body invalid");
        let found = m.recognize_blends(1e-6);
        assert_eq!(found.len(), 2, "two blends at the mitre");
        for f in &found {
            assert!((f.radius - 0.5).abs() < 1e-9, "radius {}", f.radius);
        }
        // Grid oracle for the removed volume: a point in either edge's
        // spine quadrant is removed iff outside BOTH cylinders.
        let removed_true = {
            let r = 0.5f64;
            let n = 140usize;
            let d = 2.0 / n as f64;
            let mut count = 0usize;
            for i in 0..n {
                let x = (i as f64 + 0.5) * d;
                for j in 0..n {
                    let y = (j as f64 + 0.5) * d;
                    for k in 0..n {
                        let z = (k as f64 + 0.5) * d;
                        // The MITRE splits territory at the bisector
                        // x = y: each point is governed by ONE blend.
                        let in_q1 = x >= 1.5 && z >= 1.5;
                        let in_q2 = y >= 1.5 && z >= 1.5;
                        let d1 = ((x - 1.5f64).powi(2) + (z - 1.5).powi(2)).sqrt();
                        let d2 = ((y - 1.5f64).powi(2) + (z - 1.5).powi(2)).sqrt();
                        let removed = if x >= y {
                            in_q1 && d1 > r
                        } else {
                            in_q2 && d2 > r
                        };
                        if removed {
                            count += 1;
                        }
                    }
                }
            }
            count as f64 * d * d * d
        };
        let v = m.mesh_volume();
        let want = 8.0 - removed_true;
        assert!(
            (v - want).abs() < 0.02,
            "mitre volume {v} != grid oracle {want} (removed {removed_true})"
        );
    }

    fn edge_between_faces(b: &Body, fa: Vec3, fb: Vec3) -> EdgeKey {
        b.edges
            .iter()
            .map(|(k, _)| k)
            .find(|&k| {
                let fs = b.faces_around_edge(k);
                fs.len() == 2 && {
                    let (Some(na), Some(nb)) =
                        (b.face_outward_normal(fs[0]), b.face_outward_normal(fs[1]))
                    else {
                        return false;
                    };
                    (na - fa).norm() < 1e-9 && (nb - fb).norm() < 1e-9
                        || (na - fb).norm() < 1e-9 && (nb - fa).norm() < 1e-9
                }
            })
            .expect("edge")
    }

    #[test]
    fn unblend_all_suppresses_chains_leaves_first_and_preserves_declines() {
        // Corpus-audit item (dossiers 03 sec 2.2 / 15 sec 1.4): ordered
        // whole-model blend suppression. Two independent fillets remove
        // completely, restoring the exact sharp box; the mitred-corner
        // body's blends are NOT the plane-plane unblend class, so they
        // DECLINE wholesale with the body untouched.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e1 = edge_between_faces(&b, Vec3::new(0., 0., 1.), Vec3::new(1., 0., 0.));
        let f1 = b.fillet_edge(e1, 0.3).unwrap();
        let e2 = edge_between_faces(&f1, Vec3::new(0., 0., 1.), Vec3::new(-1., 0., 0.));
        let mut f2 = f1.fillet_edge(e2, 0.3).unwrap();
        assert_eq!(f2.recognize_blends(1e-6).len(), 2, "two blends present");
        let (removed, remaining) = f2.unblend_all(1e-7);
        assert_eq!((removed, remaining), (2, 0), "both blends suppressed");
        assert!(f2.validate().is_ok(), "restored box invalid");
        let v = f2.mass_properties().unwrap().volume;
        assert!((v - 8.0).abs() < 1e-9, "restored box volume {v}");
        assert_eq!(f2.face_keys().len(), 6, "restored box faces");

        let mut m = Body::new();
        m.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e1 = edge_between_faces(&m, Vec3::new(0., 0., 1.), Vec3::new(1., 0., 0.));
        let e2 = edge_between_faces(&m, Vec3::new(0., 0., 1.), Vec3::new(0., 1., 0.));
        let mut mitred = m.mitre_fillet_corner(e1, e2, 0.5).unwrap();
        let v0 = mitred.mesh_volume();
        let (removed, remaining) = mitred.unblend_all(1e-7);
        assert_eq!(removed, 0, "mitre blends are not the unblend class");
        assert_eq!(remaining, 2, "both still recognized");
        assert!(
            (mitred.mesh_volume() - v0).abs() < 1e-9,
            "declined body must be untouched"
        );
    }

    #[test]
    fn octant_corner_blend_caps_three_fillets_with_a_sphere() {
        // Item 51 (dossier 53 Q1): box [0,2]^3, round the trihedral
        // corner at (2,2,2) with r = 0.5. The three edge cylinders
        // (axes through M = (1.5,1.5,1.5)) are capped by the exact
        // sphere octant centred at M; recognition finds the three
        // cylinder blends; the removed volume is closed-form (three
        // square-minus-quarter-disc prisms of length 2 - r, plus the
        // corner cube minus the ball octant).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let corner = b
            .vertices
            .iter()
            .find(|(_, v)| (v.point - Vec3::new(2., 2., 2.)).norm() < 1e-9)
            .map(|(k, _)| k)
            .expect("corner vertex");
        let r = 0.5f64;
        let o = b.fillet_corner_octant(corner, r).unwrap();
        assert!(o.validate().is_ok(), "octant body invalid");
        // Faces: 3 kept supports + 3 far caps + 3 bands + 1 sphere.
        assert_eq!(o.face_keys().len(), 10, "octant face count");
        let spheres = o
            .face_keys()
            .into_iter()
            .filter(|&fk| matches!(o.face_surface3(fk), Some(Surface3::Sphere(_))))
            .count();
        assert_eq!(spheres, 1, "one sphere octant face");
        let found = o.recognize_blends(1e-6);
        assert_eq!(found.len(), 3, "three cylinder blends at the corner");
        for f in &found {
            assert!((f.radius - r).abs() < 1e-9, "radius {}", f.radius);
        }
        let pi = core::f64::consts::PI;
        let removed =
            3.0 * (r * r - pi * r * r / 4.0) * (2.0 - r) + (r.powi(3) - pi * r.powi(3) / 6.0);
        let want = 8.0 - removed;
        let v = o.mesh_volume();
        assert!(
            (v - want).abs() < 0.02,
            "octant volume {v} != exact {want} (removed {removed})"
        );
        // (Analytic mass_properties over blend faces needs blend-face
        // pcurves, the documented follow-up shared by all fillets; the
        // honesty gate here is the exact closed-form volume above.)
    }

    #[test]
    fn partial_span_blend_resumes_the_sharp_edge() {
        // Item 55 (dossier 54 first milestone, cone-span variant): box
        // 4 x 2 x 2, round only [t0, t1] = [0.25, 0.75] of a top edge,
        // tapering r = 0.5 -> 0 (feathered runout, exact cone) with a
        // planar square stop at t0. Exact oracle: removed = (1 - pi/4)
        // * span * r^2 / 3 (the linear-taper wedge integral). The sharp
        // edge resumes on both sides and the apex vertex joins both
        // springs and the resumed edge.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let f = b.fillet_edge_partial(e, 0.25, 0.75, 0.5).unwrap();
        assert!(f.validate().is_ok(), "partial blend invalid");
        // Faces: 6 box + cone blend + planar stop = 8.
        assert_eq!(f.face_keys().len(), 8, "blend + stop faces");
        let cones = f
            .face_keys()
            .into_iter()
            .filter(|&fk| matches!(f.face_surface3(fk), Some(Surface3::Cone(_))))
            .count();
        assert_eq!(cones, 1, "one exact cone runout");
        let v = f.mesh_volume();
        // Independent oracle: the removed region is exactly the set of
        // points OUTSIDE the envelope cone within the corner quadrant
        // and the span (the tangency means the cone hugs the supports,
        // so no extra bounds are needed). Brute grid over the wedge
        // bbox; the dossier's (1 - pi/4) integral is the UNTILTED
        // idealization and differs by the spine-tilt correction.
        let removed_true = {
            // p is removed iff it lies in the corner quadrant of its
            // station's SPINE point and outside EVERY rolling ball
            // (min over the spine of dist - rho > 0; closed-form
            // critical point since rho is linear).
            let (c0, q1) = (Vec3::new(3.5, 0.5, 1.5), Vec3::new(4.0, 1.5, 2.0));
            let d_len = (q1 - c0).norm();
            let u = (q1 - c0) * (1.0 / d_len);
            let r0 = 0.5f64;
            let rho_slope = -r0 / d_len;
            let outside_all = |p: Vec3| -> bool {
                let w = p - c0;
                let b1 = w.dot(u);
                let q = |tau: f64| (w - u * tau).norm() - (r0 + rho_slope * tau);
                let mut best = q(0.0).min(q(d_len));
                // Critical points: d/dtau sqrt(|w|^2 - 2 b1 tau + tau^2)
                // = rho_slope -> (tau - b1)^2 = rho_slope^2 (|w|^2 - 2 b1
                // tau + tau^2): a quadratic in tau.
                let s2 = rho_slope * rho_slope;
                let qa = 1.0 - s2;
                let qb = -2.0 * b1 * (1.0 - s2);
                let qc = b1 * b1 - s2 * w.dot(w);
                let disc = qb * qb - 4.0 * qa * qc;
                if disc >= 0.0 && qa.abs() > 1e-12 {
                    for s in [1.0, -1.0] {
                        let tau = (-qb + s * disc.sqrt()) / (2.0 * qa);
                        if (0.0..=d_len).contains(&tau) {
                            best = best.min(q(tau));
                        }
                    }
                }
                best > 0.0
            };
            let n = 120usize;
            let (dx, dy, dz) = (0.5 / n as f64, 1.0 / n as f64, 0.5 / n as f64);
            let mut count = 0usize;
            for j in 0..n {
                let y = 0.5 + (j as f64 + 0.5) * dy;
                let sp = c0 + u * ((y - c0.y) / u.y);
                for i in 0..n {
                    let x = 3.5 + (i as f64 + 0.5) * dx;
                    if x < sp.x {
                        continue;
                    }
                    for k in 0..n {
                        let z = 1.5 + (k as f64 + 0.5) * dz;
                        if z < sp.z {
                            continue;
                        }
                        if outside_all(Vec3::new(x, y, z)) {
                            count += 1;
                        }
                    }
                }
            }
            count as f64 * dx * dy * dz
        };
        let want = 16.0 - removed_true;
        assert!(
            (v - want).abs() < 0.01,
            "partial-blend volume {v} != grid oracle {want} (removed {removed_true})"
        );
        // The sharp edge resumes: there are still two collinear sharp
        // edge fragments along the original edge line (z = 2, x = 2 in
        // this block's frame), totalling half the edge length... their
        // existence is the check: two edges whose midpoints lie ON the
        // original sharp line.
        let on_sharp = f
            .edges
            .iter()
            .filter(|(k, _)| {
                let Some(ed) = f.edge(*k) else { return false };
                let (p0, p1) = (
                    f.vertex(ed.bounds.0).map(|v| v.point),
                    f.vertex(ed.bounds.1).map(|v| v.point),
                );
                let (Some(p0), Some(p1)) = (p0, p1) else {
                    return false;
                };
                let m = (p0 + p1) * 0.5;
                (m.z - 2.0).abs() < 1e-9 && (m.x - 2.0).abs() < 1e-9
            })
            .count();
        assert_eq!(on_sharp, 2, "the sharp edge must resume on both sides");
    }

    #[test]
    fn cliff_overflow_clips_at_the_far_edge() {
        // Item 57 (dossier 56 sec 7): box 4 x 1 x 2, fillet the edge
        // between the wall (y = 0) and the 1-wide top with r = 1.25 >
        // width. fillet_edge declines (the overflow guard); the cliff
        // handler builds the half-tangent cylinder through the far
        // edge b, consuming the top. Exact integral oracle; below the
        // trigger radius the ordinary fillet still applies
        // (differential sweep).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 1.0, 2.0).unwrap();
        let e = {
            let ekeys: Vec<EdgeKey> = b.edges.iter().map(|(k, _)| k).collect();
            ekeys
                .into_iter()
                .find(|&k| {
                    let fs = b.faces_around_edge(k);
                    fs.len() == 2 && {
                        let (Some(na), Some(nb)) =
                            (b.face_outward_normal(fs[0]), b.face_outward_normal(fs[1]))
                        else {
                            return false;
                        };
                        (na.z > 0.9 || nb.z > 0.9) && (na.y < -0.9 || nb.y < -0.9)
                    }
                })
                .expect("wall-top edge")
        };
        // Below the trigger: ordinary fillet works, cliff refuses.
        let small = b.fillet_edge(e, 0.8).unwrap();
        let v_small = small.mesh_volume();
        let want_small = 8.0 - (1.0 - core::f64::consts::FRAC_PI_4) * 0.64 * 4.0;
        assert!(
            (v_small - want_small).abs() < 0.02,
            "sub-trigger fillet {v_small} != {want_small}"
        );
        assert!(
            b.fillet_edge_cliff(e, 0.8).is_err(),
            "cliff must refuse below the trigger"
        );
        // Above the trigger: ordinary fillet declines, the cliff handles.
        assert!(
            b.fillet_edge(e, 1.25).is_err(),
            "ordinary fillet must decline past the trigger"
        );
        let cliff = b.fillet_edge_cliff(e, 1.25).unwrap();
        assert!(cliff.validate().is_ok(), "cliff body invalid");
        // Exact oracle: removed = L (H W - Iseg), H = sqrt(2 r W - W^2),
        // Iseg the circle-segment integral from 0 to W about y = r.
        let (r, w, l) = (1.25f64, 1.0f64, 4.0f64);
        let h = (2.0 * r * w - w * w).sqrt();
        let anti = |u: f64| 0.5 * (u * (r * r - u * u).sqrt() + r * r * (u / r).asin());
        let iseg = anti(w - r) - anti(-r);
        let want = 8.0 - l * (h * w - iseg);
        let v = cliff.mesh_volume();
        assert!((v - want).abs() < 0.02, "cliff volume {v} != exact {want}");
        // Half-tangent signature: the cliff blend is tangent to ONE
        // support only, so full blend recognition finds nothing.
        assert!(
            cliff.recognize_blends(1e-6).is_empty(),
            "a half-tangent cliff must not read as a full blend"
        );
        // The narrow top is consumed: no +z-normal face remains.
        assert!(
            cliff.face_keys().into_iter().all(|f| cliff
                .face_outward_normal(f)
                .map(|n| n.z < 0.9)
                .unwrap_or(true)),
            "the narrow top face must be consumed"
        );
    }

    #[test]
    fn face_face_blend_rounds_the_slab_end() {
        // Item 50 (parallel-supports rung): slab 4 x 1 x 2, round the
        // x = 4 end wall between the y = 0 and y = 1 faces. Forced
        // r = 0.5; removed = (2r*r - pi r^2/2) * h = r^2 (2 - pi/2) h
        // = 0.2146; volume ~ 7.7854. Recognition finds the half-
        // cylinder tangent to both side faces at r = 0.5.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 1.0, 2.0).unwrap();
        let wall = b
            .face_keys()
            .into_iter()
            .find(|&f| {
                b.face_outward_normal(f)
                    .map(|n| (n.x - 1.0).abs() < 1e-9)
                    .unwrap_or(false)
            })
            .expect("x = 4 wall");
        let rounded = b.blend_face_face(wall).unwrap();
        assert!(rounded.validate().is_ok(), "rounded slab invalid");
        let v = rounded.mesh_volume();
        let want = 8.0 - 0.25 * (2.0 - core::f64::consts::FRAC_PI_2) * 2.0;
        assert!(
            (v - want).abs() < 0.02,
            "rounded slab volume {v} != ~{want}"
        );
        let found = rounded.recognize_blends(1e-6);
        assert_eq!(found.len(), 1, "one face-face blend");
        assert!(
            (found[0].radius - 0.5).abs() < 1e-9,
            "forced radius {} != 0.5",
            found[0].radius
        );
    }

    #[test]
    fn hold_line_blend_floats_the_radius() {
        // Item 54: perpendicular case first: a hold 0.5 in from the
        // edge floats r = 0.5, the exact circular fillet (volume
        // 7.8927). Then the discriminating 45-degree wedge: hold d = 1
        // on the x = 0 wall of a right-triangle prism floats
        // r = d / (1 + sqrt(2)) = sqrt(2) - 1, verified independently
        // by blend RECOGNITION on the result.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let f = b.fillet_edge_hold_line(e, true, 0.5).unwrap();
        assert!(f.validate().is_ok(), "hold (perpendicular) invalid");
        let v = f.mesh_volume();
        let want = 8.0 - (1.0 - core::f64::consts::FRAC_PI_4) * 0.25 * 2.0;
        assert!(
            (v - want).abs() < 0.02,
            "perpendicular hold must be the r = 0.5 fillet ({v} vs {want})"
        );

        let mut w = Body::new();
        w.prism(
            &[
                Vec3::ZERO,
                Vec3::new(4.0, 0.0, 0.0),
                Vec3::new(0.0, 4.0, 0.0),
            ],
            Vec3::new(0.0, 0.0, 2.0),
        )
        .unwrap();
        let v0 = w.mass_properties().unwrap().volume;
        // The vertical edge at (0, 4): between the x = 0 wall and the
        // hypotenuse (45-degree dihedral).
        let edge = w
            .edges
            .iter()
            .map(|(k, _)| k)
            .find(|&k| {
                let (a, b2) = w.edge(k).unwrap().bounds;
                let (pa, pb) = (w.vertex(a).unwrap().point, w.vertex(b2).unwrap().point);
                pa.x.abs() < 1e-9
                    && (pa.y - 4.0).abs() < 1e-9
                    && pb.x.abs() < 1e-9
                    && (pb.y - 4.0).abs() < 1e-9
            })
            .expect("wedge edge");
        // Hold on whichever support is the x = 0 wall.
        let hold_first = {
            let fs = w.faces_around_edge(edge);
            let n = w.face_outward_normal(fs[0]).unwrap();
            (n.x + 1.0).abs() < 1e-9
        };
        let wf = w.fillet_edge_hold_line(edge, hold_first, 1.0).unwrap();
        assert!(wf.validate().is_ok(), "hold (wedge) invalid");
        assert!(wf.mesh_volume() < v0, "hold blend must remove material");
        let found = wf.recognize_blends(1e-6);
        assert_eq!(found.len(), 1, "one blend on the wedge");
        let want_r = core::f64::consts::SQRT_2 - 1.0;
        assert!(
            (found[0].radius - want_r).abs() < 1e-9,
            "floated radius {} != sqrt(2)-1 = {want_r}",
            found[0].radius
        );
    }

    #[test]
    fn conic_blend_spans_chamfer_to_circle() {
        // Item 49: the rational-quadratic section sweeps the chamfer-
        // to-circle continuum on perpendicular supports. w = sqrt(2)/2
        // IS the circular fillet of radius d (volume 7.8927); w -> 0
        // flattens toward the chamfer chord (7.75); large w hugs the
        // sharp corner (-> 8). Monotone in w.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);

        let circle_like = b
            .fillet_edge_conic(e, 0.5, core::f64::consts::FRAC_1_SQRT_2)
            .unwrap();
        assert!(circle_like.validate().is_ok(), "conic (circle) invalid");
        let v_circle = circle_like.mesh_volume();
        let want_circle = 8.0 - (1.0 - core::f64::consts::FRAC_PI_4) * 0.25 * 2.0;
        assert!(
            (v_circle - want_circle).abs() < 0.02,
            "w = sqrt(2)/2 must reproduce the fillet ({v_circle} vs {want_circle})"
        );

        let flat = b.fillet_edge_conic(e, 0.5, 0.02).unwrap();
        assert!(flat.validate().is_ok(), "conic (flat) invalid");
        let v_flat = flat.mesh_volume();
        assert!(
            (v_flat - 7.75).abs() < 0.02,
            "w -> 0 must approach the chamfer ({v_flat} vs 7.75)"
        );

        let sharp = b.fillet_edge_conic(e, 0.5, 25.0).unwrap();
        assert!(sharp.validate().is_ok(), "conic (sharp) invalid");
        let v_sharp = sharp.mesh_volume();
        assert!(v_sharp > 7.95, "large w must hug the corner ({v_sharp})");

        assert!(
            v_flat < v_circle && v_circle < v_sharp,
            "fullness must be monotone in w ({v_flat}, {v_circle}, {v_sharp})"
        );
    }

    #[test]
    fn cone_blend_geometry_certifies() {
        // Item 48 geometry: spheres centered on the spine with the
        // linear radius are tangent to BOTH support planes, the spring
        // lines lie ON their planes, and the cone reproduces the sphere
        // envelope (perpendicular radius rho/cos(alpha)).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let blend = b.blend_cone_for_edge(e, 0.3, 0.6).unwrap();
        // Supports are z = 2 and x = 2 (outward +z / +x).
        for i in 0..=8 {
            let t = i as f64 / 8.0;
            // A spine point and its sphere radius.
            let c = blend.spine.origin + blend.spine.dir * (t * 2.0 - 0.5);
            let rho = {
                // Reconstruct rho from the cone: perpendicular distance
                // from the axis is rho / cos(alpha) at the tangency, so
                // sphere radius = distance-to-plane.
                (2.0 - c.z).abs()
            };
            // Tangency to BOTH planes: distance to z=2 equals distance
            // to x=2 equals rho.
            assert!(
                ((2.0 - c.x).abs() - rho).abs() < 1e-9,
                "sphere at t={t} not equidistant ({} vs {rho})",
                (2.0 - c.x).abs()
            );
            // Spring lines lie on their planes.
            let sa = blend.spring_a.origin + blend.spring_a.dir * t;
            let sb = blend.spring_b.origin + blend.spring_b.dir * t;
            let (da, db) = ((sa.z - 2.0).abs().min((sa.x - 2.0).abs()), {
                (sb.z - 2.0).abs().min((sb.x - 2.0).abs())
            });
            assert!(da < 1e-9 && db < 1e-9, "spring off-plane ({da}, {db})");
        }
        // Radii at the edge ends match the request.
        assert!((blend.r.0 - 0.3).abs() < 1e-12 && (blend.r.1 - 0.6).abs() < 1e-12);
    }

    #[test]
    fn variable_fillet_carves_the_cone_wedge() {
        // Item 48 surgery: box 2^3, top-right edge rounded with r going
        // 0.3 -> 0.6. Removed material = (1 - pi/4) * integral r(t)^2 dt
        // = (1 - pi/4) * L * (r0^2 + r0 r1 + r1^2)/3 = 0.090133...
        // mesh_volume must land within ~0.01 absolute (chordal cone +
        // ellipse-arc sampling).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let f = b.fillet_edge_variable(e, 0.3, 0.6).unwrap();
        assert!(f.validate().is_ok(), "variable fillet invalid");
        // The blend face is a CONE tangent to both supports; recognition
        // currently targets cylinders/tori, so check directly.
        let cones = f
            .face_keys()
            .into_iter()
            .filter(|&fk| matches!(f.face_surface3(fk), Some(Surface3::Cone(_))))
            .count();
        assert_eq!(cones, 1, "one cone blend face");
        let want = 8.0 - (1.0 - core::f64::consts::FRAC_PI_4) * 2.0 * (0.09 + 0.18 + 0.36) / 3.0;
        let mv = f.mesh_volume();
        assert!(
            (mv - want).abs() < 0.01,
            "variable-fillet volume {mv} != ~{want}"
        );
    }

    #[test]
    fn recognize_and_unblend_round_trips_the_fillet() {
        // Items 58 + 59: box -> fillet_edge(0.5) -> recognize exactly
        // one blend (the cylinder, radius 0.5, two planar supports) ->
        // unblend -> the EXACT original box (V8 E12 F6, volume 8,
        // mass == mesh).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let mut filleted = b.fillet_edge(e, 0.5).unwrap();

        let found = filleted.recognize_blends(1e-6);
        assert_eq!(found.len(), 1, "exactly one blend face");
        assert!((found[0].radius - 0.5).abs() < 1e-12, "blend radius");
        let blend = found[0].face;

        filleted.unblend(blend, 1e-6).unwrap();
        assert!(filleted.validate().is_ok(), "unblended box invalid");
        let c = filleted.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "back to the sharp box");
        let v = filleted.mass_properties().unwrap().volume;
        let mv = filleted.mesh_volume();
        assert!(
            (v - 8.0).abs() < 1e-9 && (mv - 8.0).abs() < 1e-9,
            "unblend must restore volume 8 mass == mesh (got {v}, {mv})"
        );
        // Idempotence of recognition: nothing left to find.
        assert!(filleted.recognize_blends(1e-6).is_empty());
    }

    #[test]
    fn recognition_rejects_non_tangent_cylinders() {
        // A plain cylinder primitive's barrel meets its caps at 90
        // degrees: NOT a blend (planar faces are never candidates, so
        // chamfers are excluded by construction).
        let mut cyl = Body::new();
        cyl.cylinder(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
            2.0,
        )
        .unwrap();
        assert!(
            cyl.recognize_blends(1e-6).is_empty(),
            "perpendicular barrel must not read as a blend"
        );
    }

    #[test]
    fn plane_plane_blend_is_tangent_cylinder() {
        // Block [0,2]^3, fillet the top-right edge with r = 0.5. The
        // supports are z = 2 (n = +z) and x = 2 (n = +x); the spine is the
        // line {x = 1.5, z = 1.5} parallel to y, the cylinder has radius
        // 0.5 about it, tangent to both planes (axis 0.5 from each).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let blend = b.blend_cylinder_for_edge(e, 0.5).unwrap();

        // Spine at x = z = 1.5, direction along y.
        assert!(
            (blend.spine.origin.x - 1.5).abs() < 1e-9,
            "spine x {}",
            blend.spine.origin.x
        );
        assert!(
            (blend.spine.origin.z - 1.5).abs() < 1e-9,
            "spine z {}",
            blend.spine.origin.z
        );
        assert!(
            blend.surface.frame.z.cross(Vec3::new(0.0, 1.0, 0.0)).norm() < 1e-9,
            "spine not along y"
        );
        assert!((blend.surface.radius - 0.5).abs() < 1e-12);

        // Tangency: the axis is exactly r from each support plane.
        let axis_pt = blend.spine.origin;
        assert!((2.0 - axis_pt.z).abs() - 0.5 < 1e-9, "not tangent to z=2");
        assert!((2.0 - axis_pt.x).abs() - 0.5 < 1e-9, "not tangent to x=2");

        // Spring lines lie ON the support planes (z=2 and x=2) at the
        // tangency points (x=1.5 and z=1.5 respectively).
        assert!(
            (blend.spring_a.origin.z - 2.0).abs() < 1e-9,
            "spring_a not on z=2"
        );
        assert!((blend.spring_a.origin.x - 1.5).abs() < 1e-9);
        assert!(
            (blend.spring_b.origin.x - 2.0).abs() < 1e-9,
            "spring_b not on x=2"
        );
        assert!((blend.spring_b.origin.z - 1.5).abs() < 1e-9);

        // The spring points are exactly r from the spine (the ball touches
        // each face at distance r from its centre).
        assert!(((blend.spring_a.origin - axis_pt).norm() - 0.5).abs() < 1e-9);
        assert!(((blend.spring_b.origin - axis_pt).norm() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn fillet_box_edge_produces_valid_cylinder_blend() {
        // Round the top-right edge of a 2x2x2 block by 0.5. The trim-and-
        // stitch surgery removes the sharp edge + its two end vertices,
        // adds 4 spring vertices, 2 spring edges + 2 arc edges, and the
        // cylinder blend face: V8->10, E12->15, F6->7 (Euler 2). The blend
        // face carries the exact rolling-ball cylinder.
        let mut base = Body::new();
        base.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&base);
        let filleted = base.fillet_edge(e, 0.5).unwrap();
        assert!(filleted.validate().is_ok(), "filleted body invalid");
        let c = filleted.counts();
        assert_eq!(
            (c.v, c.e, c.f),
            (10, 15, 7),
            "fillet counts {:?}",
            (c.v, c.e, c.f)
        );
        // Exactly one cylindrical face (the blend), radius 0.5.
        let cyl: Vec<f64> = filleted
            .faces
            .iter()
            .filter_map(|(fk, _)| match filleted.face_surface_geom(fk) {
                Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => Some(c.radius),
                _ => None,
            })
            .collect();
        assert_eq!(cyl.len(), 1, "expected one cylinder blend face");
        assert!((cyl[0] - 0.5).abs() < 1e-12, "blend radius {}", cyl[0]);
        // Volume: a 2^3 block minus the corner the fillet rounds off =
        // 8 - (r^2 - pi r^2/4)*L, r=0.5 L=2 => 8 - 0.10730 = 7.89270.
        // mesh_volume uses the now angularly-trimmed cylinder tessellation.
        let v = filleted.mesh_volume();
        let expect = 8.0 - (0.25 - core::f64::consts::PI * 0.25 / 4.0) * 2.0;
        assert!(
            (v - expect).abs() < expect * 0.002,
            "fillet mesh_volume {v} != ~{expect}"
        );
    }

    /// An edge whose two faces have outward normals matching `na`, `nb`
    /// (each given as a unit-ish axis to test the dominant component of).
    fn edge_with_normals(b: &Body, na: Vec3, nb: Vec3) -> EdgeKey {
        for (e, _) in b.edges.iter() {
            let fs = b.faces_around_edge(e);
            if fs.len() != 2 {
                continue;
            }
            let (Some(x), Some(y)) = (b.face_outward_normal(fs[0]), b.face_outward_normal(fs[1]))
            else {
                continue;
            };
            let m = |u: Vec3, t: Vec3| u.dot(t) > 0.9;
            if (m(x, na) && m(y, nb)) || (m(x, nb) && m(y, na)) {
                return e;
            }
        }
        panic!("no edge with the requested normals");
    }

    #[test]
    fn fillet_vertical_box_edge() {
        // A vertical edge (+x / +y faces): different orientation than the
        // top-right edge, exercising the surgery's generality.
        let mut base = Body::new();
        base.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = edge_with_normals(&base, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let filleted = base.fillet_edge(e, 0.5).unwrap();
        assert!(filleted.validate().is_ok(), "vertical fillet invalid");
        let c = filleted.counts();
        assert_eq!((c.v, c.e, c.f), (10, 15, 7), "counts {:?}", (c.v, c.e, c.f));
        let v = filleted.mesh_volume();
        let expect = 8.0 - (0.25 - core::f64::consts::PI * 0.25 / 4.0) * 2.0;
        assert!(
            (v - expect).abs() < expect * 0.002,
            "vertical fillet volume {v}"
        );
    }

    #[test]
    fn plane_cylinder_blend_is_tangent_torus() {
        // Round the top rim of a cylinder (R=1, h=2) with r=0.3. The cap
        // plane (z=2) meets the lateral (R=1) at the rim circle; the spine
        // is a circle of radius R-r=0.7 at z=2-0.3=1.7, and the blend is a
        // torus major 0.7, minor 0.3, tangent to both (outer equator at
        // R=1, top at z=2).
        use keel_geom::surface::Frame3;
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.cylinder(frame, 1.0, 2.0).unwrap();
        // The top-rim edge: a plane/cylinder edge whose plane is z=+2.
        let rim = b
            .edges
            .iter()
            .map(|(k, _)| k)
            .find(|&e| {
                let fs = b.faces_around_edge(e);
                fs.len() == 2
                    && fs.iter().any(|&f| {
                        matches!(
                            b.face_surface_geom(f),
                            Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                        )
                    })
                    && fs
                        .iter()
                        .any(|&f| matches!(b.face_outward_normal(f), Some(n) if n.z > 0.9))
            })
            .expect("no top rim edge");
        let blend = b.blend_torus_for_edge(rim, 0.3).unwrap();
        assert!(
            (blend.surface.major - 0.7).abs() < 1e-9,
            "major {}",
            blend.surface.major
        );
        assert!(
            (blend.surface.minor - 0.3).abs() < 1e-9,
            "minor {}",
            blend.surface.minor
        );
        // Tangency: outer equator = major+minor = R = 1; top = centre_z+minor = 2.
        assert!(
            (blend.surface.major + blend.surface.minor - 1.0).abs() < 1e-9,
            "not tangent to cyl"
        );
        assert!(
            (blend.surface.frame.origin.z + blend.surface.minor - 2.0).abs() < 1e-9,
            "not tangent to cap"
        );
        // Spine radius 0.7 at z=1.7; plane spring radius 0.7 at z=2; cyl spring radius 1 at z=1.7.
        assert!((blend.spine.radius - 0.7).abs() < 1e-9);
        assert!(
            (blend.spine.center.z - 1.7).abs() < 1e-9,
            "spine z {}",
            blend.spine.center.z
        );
        assert!((blend.spring_plane.center.z - 2.0).abs() < 1e-9);
        assert!((blend.spring_cyl.radius - 1.0).abs() < 1e-9);
    }

    #[test]
    fn blend_faces_integrate_analytically_via_projected_bounds() {
        // The doc-promised blend-pcurve milestone: curved blend faces
        // whose boundaries are ISO-RECTANGULAR now integrate exactly
        // via projected UV bounds (no pcurves needed).
        // 1. Plain fillet: analytic mass == the closed form to 1e-9.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = edge_between_faces(&b, Vec3::new(0., 0., 1.), Vec3::new(1., 0., 0.));
        let f = b.fillet_edge(e, 0.5).unwrap();
        let pi = core::f64::consts::PI;
        let exact = 8.0 - (0.25 - pi * 0.25 / 4.0) * 2.0;
        let mass = f.mass_properties().unwrap().volume;
        assert!(
            (mass - exact).abs() < 1e-9,
            "fillet mass {mass} != exact {exact}"
        );
        assert!((f.mesh_volume() - exact).abs() < 0.02, "chordal sanity");

        // 2. Cap-rim torus ring (full-circle periodic span): analytic
        // mass == the Pappus-exact volume to 1e-9. Removed ring =
        // 2 pi xbar A over the corner region (square minus quarter
        // disc) of the rim section.
        use keel_geom::surface::Frame3;
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut base = Body::new();
        base.cylinder(frame, 1.0, 2.0).unwrap();
        let rim = base
            .edges
            .iter()
            .map(|(k, _)| k)
            .find(|&e| {
                let fs = base.faces_around_edge(e);
                fs.len() == 2
                    && fs.iter().any(|&f| {
                        matches!(
                            base.face_surface_geom(f),
                            Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                        )
                    })
                    && fs
                        .iter()
                        .any(|&f| matches!(base.face_outward_normal(f), Some(n) if n.z > 0.9))
            })
            .expect("no top rim edge");
        let filleted = base.fillet_cap_rim(rim, 0.3).unwrap();
        let (rr, rb) = (1.0f64, 0.3f64);
        let a_sq = rb * rb;
        let x_sq = rr - rb * 0.5;
        let a_q = pi * rb * rb / 4.0;
        let x_q = (rr - rb) + 4.0 * rb / (3.0 * pi);
        let a = a_sq - a_q;
        let xbar = (a_sq * x_sq - a_q * x_q) / a;
        let removed = core::f64::consts::TAU * xbar * a;
        let exact = pi * rr * rr * 2.0 - removed;
        let mass = filleted.mass_properties().unwrap().volume;
        assert!(
            (mass - exact).abs() < 1e-9,
            "rim mass {mass} != Pappus {exact}"
        );

        // 3. The sphere octant is NOT iso-rectangular in UV: that body
        // keeps declining analytic mass properties honestly.
        let mut c = Body::new();
        c.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let corner = c
            .vertices
            .iter()
            .find(|(_, v)| (v.point - Vec3::new(2., 2., 2.)).norm() < 1e-9)
            .map(|(k, _)| k)
            .expect("corner vertex");
        let oct = c.fillet_corner_octant(corner, 0.5).unwrap();
        assert!(oct.mass_properties().is_err(), "octant declines");
    }

    #[test]
    fn fillet_cylinder_cap_rim_to_torus() {
        // Round the top rim of a cylinder (R=1, h=2) with r=0.3: the cap
        // shrinks to radius 0.7, the lateral shortens, and a torus ring
        // (major 0.7, minor 0.3) joins them. Validate the resulting solid
        // and that it carries exactly one torus face.
        use keel_geom::surface::Frame3;
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut base = Body::new();
        base.cylinder(frame, 1.0, 2.0).unwrap();
        let rim = base
            .edges
            .iter()
            .map(|(k, _)| k)
            .find(|&e| {
                let fs = base.faces_around_edge(e);
                fs.len() == 2
                    && fs.iter().any(|&f| {
                        matches!(
                            base.face_surface_geom(f),
                            Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                        )
                    })
                    && fs
                        .iter()
                        .any(|&f| matches!(base.face_outward_normal(f), Some(n) if n.z > 0.9))
            })
            .expect("no top rim edge");
        let filleted = base.fillet_cap_rim(rim, 0.3).unwrap();
        assert!(filleted.validate().is_ok(), "rim fillet invalid");
        let tori: Vec<f64> = filleted
            .faces
            .iter()
            .filter_map(|(fk, _)| match filleted.face_surface_geom(fk) {
                Some(SurfaceGeom::Analytic(Surface3::Torus(t))) => Some(t.minor),
                _ => None,
            })
            .collect();
        assert_eq!(tori.len(), 1, "expected one torus blend face");
        assert!((tori[0] - 0.3).abs() < 1e-12, "torus minor {}", tori[0]);
        // Volume: cylinder up to z=1.7 (1.7 pi) + the rounded top (revolve
        // of the torus quarter), ~6.170. Exercises trimmed-torus tessellation.
        let v = filleted.mesh_volume();
        assert!(
            (v - 6.16998).abs() < 0.08,
            "rim fillet mesh_volume {v} != ~6.170"
        );
    }

    #[test]
    fn fillet_cylinder_bottom_rim_to_torus() {
        // The BOTTOM rim (cap normal -z, sgn = -1): exercises the
        // asymmetric offset (h_off = hp - sgn*r) and confirms the torus
        // surgery generalizes to either cap.
        use keel_geom::surface::Frame3;
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut base = Body::new();
        base.cylinder(frame, 1.0, 2.0).unwrap();
        let rim = base
            .edges
            .iter()
            .map(|(k, _)| k)
            .find(|&e| {
                let fs = base.faces_around_edge(e);
                fs.len() == 2
                    && fs.iter().any(|&f| {
                        matches!(
                            base.face_surface_geom(f),
                            Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                        )
                    })
                    && fs
                        .iter()
                        .any(|&f| matches!(base.face_outward_normal(f), Some(n) if n.z < -0.9))
            })
            .expect("no bottom rim edge");
        let filleted = base.fillet_cap_rim(rim, 0.3).unwrap();
        assert!(filleted.validate().is_ok(), "bottom rim fillet invalid");
        let v = filleted.mesh_volume();
        // Symmetric to the top-rim case: same removed corner, so 6.170.
        assert!((v - 6.16998).abs() < 0.08, "bottom rim fillet volume {v}");
    }

    #[test]
    fn concave_blend_geometry_and_convexity() {
        // L-prism (a non-convex profile) has a reentrant vertical edge at
        // (1,1) and convex corners elsewhere.
        let lprofile = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ];
        let mut b = Body::new();
        b.prism(&lprofile, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert!(b.validate().is_ok(), "L-prism invalid");
        let vert_at = |b: &Body, x: f64, y: f64| {
            b.edges.iter().map(|(k, _)| k).find(move |&e| {
                let bd = b.edges.get(e).unwrap().bounds;
                let (pa, pb) = (
                    b.vertices.get(bd.0).unwrap().point,
                    b.vertices.get(bd.1).unwrap().point,
                );
                (pa.x - x).abs() < 1e-9
                    && (pa.y - y).abs() < 1e-9
                    && (pb.x - x).abs() < 1e-9
                    && (pb.y - y).abs() < 1e-9
            })
        };
        let concave = vert_at(&b, 1.0, 1.0).expect("no reentrant edge");
        let convex = vert_at(&b, 2.0, 0.0).expect("no convex corner edge");
        assert_eq!(
            b.edge_is_convex(concave),
            Some(false),
            "reentrant edge is concave"
        );
        assert_eq!(
            b.edge_is_convex(convex),
            Some(true),
            "corner edge is convex"
        );
        // Concave blend: the ball fills the notch, so the spine sits at
        // (1+r, 1+r) = (1.3, 1.3), with the cylinder tangent to both faces.
        let blend = b.blend_cylinder_for_edge(concave, 0.3).unwrap();
        assert!(
            (blend.spine.origin.x - 1.3).abs() < 1e-9 && (blend.spine.origin.y - 1.3).abs() < 1e-9,
            "concave spine {:?}",
            blend.spine.origin
        );
        assert!((blend.surface.radius - 0.3).abs() < 1e-12);
        // The convex blend (corner edge) still lands inside the material.
        let cb = b.blend_cylinder_for_edge(convex, 0.3).unwrap();
        assert!(
            (cb.spine.origin.x - 1.7).abs() < 1e-9 && (cb.spine.origin.y - 0.3).abs() < 1e-9,
            "convex spine {:?}",
            cb.spine.origin
        );
        // Concave fillet: the surgery fills the reentrant notch with a
        // concave round, ADDING the corner sliver (r^2 - pi r^2/4)*h. The
        // unified surgery (file 44) forks only on the convexity sign; the
        // cap-grow happens automatically via face adjacency.
        let filleted = b.fillet_edge(concave, 0.3).unwrap();
        assert!(filleted.validate().is_ok(), "concave fillet invalid");
        let cyl: Vec<f64> = filleted
            .faces
            .iter()
            .filter_map(|(fk, _)| match filleted.face_surface_geom(fk) {
                Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => Some(c.radius),
                _ => None,
            })
            .collect();
        assert_eq!(cyl.len(), 1, "expected one cylinder blend face");
        assert!((cyl[0] - 0.3).abs() < 1e-12, "blend radius {}", cyl[0]);
        let v = filleted.mesh_volume();
        let expect = 3.0 + (0.09 - core::f64::consts::PI * 0.09 / 4.0) * 1.0;
        // 1% tessellation tolerance (chord-vs-arc on the cap edges), same
        // as the convex fillet tests.
        assert!(
            (v - expect).abs() < expect * 0.002,
            "concave fillet vol {v} != ~{expect}"
        );
    }

    #[test]
    fn blend_rejects_bad_radius() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        assert!(b.blend_cylinder_for_edge(e, 0.0).is_err());
        assert!(b.blend_cylinder_for_edge(e, -1.0).is_err());
    }
}
