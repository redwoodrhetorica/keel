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
            (v - expect).abs() < expect * 0.01,
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
            (v - expect).abs() < expect * 0.01,
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
            (v - expect).abs() < expect * 0.012,
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
