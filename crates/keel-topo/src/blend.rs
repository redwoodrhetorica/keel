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
            let lp = b
                .faces
                .get(cap)
                .map(|f| f.loops[0])
                .ok_or(TopoError::StaleKey)?;
            let fin_a = b.fin_ending_at_vertex(lp, a_end)?;
            let fin_b = b.fin_ending_at_vertex(lp, b_end)?;
            let split = b.split_face(fin_a, fin_b, None)?;
            if let Some(surf) = b.faces.get(cap).and_then(|f| f.surface)
                && let Some(nf) = b.faces.get_mut(split.face_new)
            {
                nf.surface = Some(surf);
            }
            let ell = cone_plane_ellipse(&cone, cap_p, cap_n)
                .ok_or(TopoError::Precondition("fillet: cap ellipse"))?;
            b.attach_edge_curve(split.edge, Curve3::Ellipse(ell), true);
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
            let radius = match self.face_surface3(face) {
                Some(Surface3::Cylinder(c)) => c.radius,
                Some(Surface3::Torus(t)) => t.minor,
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
        if let (Some(p0), Some(p1)) = (p0, p1)
            && let Ok(line) = keel_geom::curve::Line3::new(p0, p1 - p0)
        {
            work.attach_edge_curve(e2, Curve3::Line(line), true);
        }
        // Honesty gates.
        if work.validate().is_err() {
            return Err(TopoError::Precondition(
                "unblend: result invalid (declined)",
            ));
        }
        let v = work
            .mass_properties()
            .map(|m| m.volume)
            .map_err(|_| TopoError::Precondition("unblend: massprops failed"))?;
        let mv = work.mesh_volume();
        if !v.is_finite() || (v - mv).abs() > tol.max(1e-6) * (1.0 + v.abs()) {
            return Err(TopoError::Precondition(
                "unblend: result inconsistent (declined)",
            ));
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
