//! Imprinting (M5b): split a face along a curve that lies on its
//! surface, turning an SSI curve into real topology with pcurves.
//! This is steps 1-3 of the M3 boolean pipeline (intersect, imprint,
//! glue); M6 adds classify + select.
//!
//! Coincidence judgement happens HERE (the M3 deferral): imprint
//! verifies the curve lies on the face surface within tolerance before
//! mutating, and computes the pcurve by inversion + fit.
//!
//! M5b ships the CLOSED-INTERIOR-LOOP case (a cylinder cutting a
//! planar face yields a circle: the face splits into a disc + an
//! annulus sharing the circular edge) and the BOUNDARY-CROSSING case.
//! Both are built from the M3 Euler/ring operators, so validity and
//! lineage are preserved by construction.

use crate::body::{Body, TopoError};
use crate::entity::{FaceKey, FinKey, SurfaceGeom};
use crate::euler::MevSite;
use keel_geom::curve::Curve3;
use keel_math::vec::Vec3;

#[derive(Clone, Debug)]
pub struct ImprintReport {
    /// The edge created along the imprinted curve.
    pub edge: crate::entity::EdgeKey,
    /// Faces resulting from the split (the original may be one of them).
    pub faces: Vec<FaceKey>,
}

impl Body {
    /// Imprint a CLOSED curve (a loop interior to `face`) onto the
    /// face. The curve must lie on the face's surface within `tol`.
    /// Splits the face into the interior-disc face and the surrounding
    /// face, sharing a new closed edge along the curve.
    pub fn imprint_closed_curve(
        &mut self,
        face: FaceKey,
        curve: &Curve3,
        tol: f64,
    ) -> Result<ImprintReport, TopoError> {
        // Verify the curve lies on the face surface and build the
        // pcurve by inversion.
        let surf = self.face_analytic_surface(face)?;
        let (pcurve, seam3) = self.curve_pcurve_on(face, curve, &surf, tol)?;

        // Outer loop and a fin to spur from.
        let lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())
            .ok_or(TopoError::StaleKey)?;
        let outer_fin = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("imprint: face has no outer fin"))?;

        // 1. Spur from the outer loop's end vertex to a seam vertex on
        //    the curve.
        let spur = self.mev(MevSite::AfterFin(outer_fin), seam3)?;
        let spur_fin = self.fin_ending_at_vertex(lp, spur.vertex)?;

        // 2. Closed self-loop edge at the seam vertex (mef with
        //    fin_a == fin_b): makes the circle edge + a new disc face.
        let mef = self.mef(spur_fin, spur_fin, None)?;
        let circle_edge = mef.edge;
        let disc_face = mef.face;

        // 3. Kill the spur as a bridge (kemr): the circle becomes an
        //    inner ring of the surrounding face.
        let spur_fin2 = self.fin_ending_at_vertex(lp, spur.vertex)?;
        self.kemr(spur_fin2)?;

        // 4. Attach geometry: the circle edge gets the 3D curve; both
        //    its fins get the pcurve; both faces inherit the surface.
        let ckey = self.add_curve(curve.clone());
        if let Some(e) = self.edges.get_mut(circle_edge) {
            e.curve = Some((ckey, true));
        }
        let pkey = self.add_curve(Curve3::Nurbs(pcurve));
        let radial = self
            .edges
            .get(circle_edge)
            .map(|e| e.radial.clone())
            .unwrap_or_default();
        for fk in radial {
            if let Some(f) = self.fins.get_mut(fk) {
                f.pcurve = Some((pkey, true));
            }
        }
        // The disc face shares the parent surface.
        if let Some((sk, sense)) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(df) = self.faces.get_mut(disc_face)
        {
            df.surface = Some((sk, sense));
        }
        self.debug_validate();
        Ok(ImprintReport {
            edge: circle_edge,
            faces: vec![face, disc_face],
        })
    }

    /// Imprint an OPEN curve crossing `face` from one boundary point to
    /// another (the SSI of two bounded planar faces is such a segment).
    /// The curve's endpoints must lie on the face's outer-loop edges
    /// within `tol`. Splits the boundary edges at the endpoints and the
    /// face along the curve into two faces sharing the new edge.
    pub fn imprint_open_curve(
        &mut self,
        face: FaceKey,
        curve: &Curve3,
        tol: f64,
    ) -> Result<ImprintReport, TopoError> {
        let surf = self.face_analytic_surface(face)?;
        let (pcurve, p_start, p_end) = self.open_curve_pcurve_on(face, curve, &surf, tol)?;

        // Locate the boundary edges containing the two endpoints.
        let lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())
            .ok_or(TopoError::StaleKey)?;
        let start_edge = self.boundary_edge_containing(lp, p_start, tol)?;
        let end_edge = self.boundary_edge_containing(lp, p_end, tol)?;
        if start_edge == end_edge {
            return Err(TopoError::Precondition(
                "imprint_open: both endpoints on one edge (unsupported)",
            ));
        }
        // Split the two boundary edges at the endpoints.
        let se = self.split_edge(start_edge, p_start)?;
        let ee = self.split_edge(end_edge, p_end)?;
        // Find the fins of the (now split) boundary ending at the new
        // vertices, in the outer loop, and split the face between them.
        let fin_a = self.fin_ending_at_vertex(lp, se.vertex)?;
        let fin_b = self.fin_ending_at_vertex(lp, ee.vertex)?;
        let split = self.split_face(fin_a, fin_b, None)?;
        let new_edge = split.edge;

        // Geometry: 3D curve + pcurves on both fins; new face inherits
        // the surface.
        let ckey = self.add_curve(curve.clone());
        if let Some(e) = self.edges.get_mut(new_edge) {
            e.curve = Some((ckey, true));
        }
        let pkey = self.add_curve(Curve3::Nurbs(pcurve));
        let radial = self
            .edges
            .get(new_edge)
            .map(|e| e.radial.clone())
            .unwrap_or_default();
        for fk in radial {
            if let Some(f) = self.fins.get_mut(fk) {
                f.pcurve = Some((pkey, true));
            }
        }
        if let Some((sk, sense)) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(nf) = self.faces.get_mut(split.face_new)
        {
            nf.surface = Some((sk, sense));
        }
        self.debug_validate();
        Ok(ImprintReport {
            edge: new_edge,
            faces: vec![split.face_old, split.face_new],
        })
    }

    /// True iff the closed planar `curve` crosses one of `face`'s
    /// boundary line edges (the periodic-surface case: an SSI circle/
    /// ellipse wrapping a cylinder crosses its vertical seam line). The
    /// dispatch in the boolean uses this to choose the crossing imprint
    /// over the interior-ring imprint.
    pub(crate) fn closed_curve_crosses_boundary(
        &self,
        face: FaceKey,
        curve: &Curve3,
        tol: f64,
    ) -> bool {
        closed_curve_plane(curve)
            .and_then(|(pt, n)| {
                self.find_planar_seam_crossing(face, pt, n, tol.max(1e-7))
                    .ok()
            })
            .is_some()
    }

    /// Imprint a CLOSED planar curve that CROSSES a boundary line edge
    /// of `face` (a cylinder's vertical seam): split the seam at the
    /// crossing point P, then base the closed curve edge at P (`mef`
    /// closing a loop there), splitting the face into the two portions.
    pub fn imprint_closed_curve_crossing(
        &mut self,
        face: FaceKey,
        curve: &Curve3,
        tol: f64,
    ) -> Result<ImprintReport, TopoError> {
        let surf = self.face_analytic_surface(face)?;
        let (pcurve, _seam3) = self.curve_pcurve_on(face, curve, &surf, tol)?;
        let (pt, n) = closed_curve_plane(curve).ok_or(TopoError::Precondition(
            "crossing imprint: non-planar curve",
        ))?;
        let (crossed_edge, p) = self.find_planar_seam_crossing(face, pt, n, tol.max(1e-7))?;
        // Split the seam line at the crossing.
        let se = self.split_edge(crossed_edge, p)?;
        // Base the closed curve edge at the new vertex (mef closing a
        // loop there), splitting the face.
        let lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())
            .ok_or(TopoError::StaleKey)?;
        let fin_at_p = self.fin_ending_at_vertex(lp, se.vertex)?;
        let surf_key = self.faces.get(face).and_then(|f| f.surface);
        let mef = self.mef(fin_at_p, fin_at_p, surf_key)?;
        let circle_edge = mef.edge;
        let new_face = mef.face;
        // Geometry: 3D curve + pcurves on both fins; both faces keep the
        // surface.
        let ckey = self.add_curve(curve.clone());
        if let Some(e) = self.edges.get_mut(circle_edge) {
            e.curve = Some((ckey, true));
        }
        let pkey = self.add_curve(Curve3::Nurbs(pcurve));
        let radial = self
            .edges
            .get(circle_edge)
            .map(|e| e.radial.clone())
            .unwrap_or_default();
        for fk in radial {
            if let Some(f) = self.fins.get_mut(fk) {
                f.pcurve = Some((pkey, true));
            }
        }
        if let Some((sk, sense)) = surf_key
            && let Some(nf) = self.faces.get_mut(new_face)
        {
            nf.surface = Some((sk, sense));
        }
        self.debug_validate();
        Ok(ImprintReport {
            edge: circle_edge,
            faces: vec![face, new_face],
        })
    }

    /// Find where the plane (`pt`, unit normal `n`) of a closed curve
    /// crosses one of `face`'s boundary LINE edges, strictly interior to
    /// the edge. Returns (edge, crossing point).
    fn find_planar_seam_crossing(
        &self,
        face: FaceKey,
        pt: Vec3,
        n: Vec3,
        tol: f64,
    ) -> Result<(crate::entity::EdgeKey, Vec3), TopoError> {
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        for lk in loops {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            loop {
                let edge = self.fins.get(cur).map(|f| f.edge);
                if let Some(ek) = edge
                    && let Some(e) = self.edges.get(ek)
                {
                    let is_line = matches!(
                        e.curve.and_then(|(ck, _)| self.curves.get(ck)),
                        Some(Curve3::Line(_))
                    );
                    let (a, b) = (
                        self.vertices.get(e.bounds.0).map(|v| v.point),
                        self.vertices.get(e.bounds.1).map(|v| v.point),
                    );
                    if is_line && let (Some(a), Some(b)) = (a, b) {
                        let denom = (b - a).dot(n);
                        if denom.abs() > 1e-12 {
                            let t = (pt - a).dot(n) / denom;
                            if t > 1e-6 && t < 1.0 - 1e-6 {
                                let cross = a + (b - a) * t;
                                return Ok((ek, cross));
                            }
                        }
                    }
                }
                cur = self
                    .fins
                    .get(cur)
                    .map(|f| f.next)
                    .ok_or(TopoError::StaleKey)?;
                if cur == entry {
                    break;
                }
            }
        }
        let _ = tol;
        Err(TopoError::Precondition(
            "crossing imprint: no seam crossing",
        ))
    }

    // ---- helpers ---------------------------------------------------------

    /// Pcurve + 3D endpoints for an open curve.
    fn open_curve_pcurve_on(
        &self,
        _face: FaceKey,
        curve: &Curve3,
        surf: &keel_geom::surface::Surface3,
        tol: f64,
    ) -> Result<(keel_geom::nurbs_curve::NurbsCurve, Vec3, Vec3), TopoError> {
        let sample = |t: f64| -> Vec3 {
            match curve {
                Curve3::Line(l) => l.point(t),
                Curve3::Circle(c) => c.point(core::f64::consts::TAU * t),
                Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
                Curve3::Nurbs(n) => {
                    let (a, b) = n.domain();
                    n.point(a + t * (b - a))
                }
            }
        };
        // For an open imprint the curve is provided pre-bounded: t in
        // [0,1] maps to the segment. Verify on-surface.
        for k in 0..=12 {
            let p = sample(k as f64 / 12.0);
            let pr = surf
                .project(p)
                .map_err(|_| TopoError::Precondition("imprint_open: projection failed"))?;
            if pr.distance > tol {
                return Err(TopoError::Precondition(
                    "imprint_open: curve not on face surface",
                ));
            }
        }
        let fit = keel_geom::fit::pcurve_on_analytic(curve, surf, 64, tol.max(1e-7))
            .map_err(|_| TopoError::Precondition("imprint_open: pcurve fit failed"))?;
        Ok((fit.curve, sample(0.0), sample(1.0)))
    }

    /// The outer-loop edge whose curve passes within tol of `p` (and p
    /// is between its endpoints). Returns the edge key.
    fn boundary_edge_containing(
        &self,
        lp: crate::entity::LoopKey,
        p: Vec3,
        tol: f64,
    ) -> Result<crate::entity::EdgeKey, TopoError> {
        let entry = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("no boundary"))?;
        let mut cur = entry;
        loop {
            let edge = self
                .fins
                .get(cur)
                .map(|f| f.edge)
                .ok_or(TopoError::StaleKey)?;
            if let Some(e) = self.edges.get(edge) {
                let p0 = self.vertices.get(e.bounds.0).map(|v| v.point);
                let p1 = self.vertices.get(e.bounds.1).map(|v| v.point);
                if let (Some(a), Some(b)) = (p0, p1) {
                    // Distance from p to the segment [a, b].
                    let ab = b - a;
                    let len2 = ab.dot(ab);
                    let t = if len2 > 0.0 {
                        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let foot = a + ab * t;
                    if (foot - p).norm() <= tol && t > 1e-9 && t < 1.0 - 1e-9 {
                        return Ok(edge);
                    }
                }
            }
            cur = self
                .fins
                .get(cur)
                .map(|f| f.next)
                .ok_or(TopoError::StaleKey)?;
            if cur == entry {
                return Err(TopoError::Precondition(
                    "imprint_open: endpoint not on any boundary edge interior",
                ));
            }
        }
    }

    fn face_analytic_surface(
        &self,
        face: FaceKey,
    ) -> Result<keel_geom::surface::Surface3, TopoError> {
        let (sk, _) = self
            .faces
            .get(face)
            .and_then(|f| f.surface)
            .ok_or(TopoError::Precondition("imprint: face has no surface"))?;
        match self.surfaces.get(sk) {
            Some(SurfaceGeom::Analytic(a)) => Ok(a.clone()),
            _ => Err(TopoError::Precondition("imprint: non-analytic face (M5b)")),
        }
    }

    /// Verify the curve lies on the surface within tol, compute its
    /// pcurve, and return (pcurve, a representative seam point on the
    /// curve in 3D).
    fn curve_pcurve_on(
        &self,
        _face: FaceKey,
        curve: &Curve3,
        surf: &keel_geom::surface::Surface3,
        tol: f64,
    ) -> Result<(keel_geom::nurbs_curve::NurbsCurve, Vec3), TopoError> {
        // Sample-check on-surface.
        let sample = |t: f64| -> Vec3 {
            match curve {
                Curve3::Line(l) => l.point(t),
                Curve3::Circle(c) => c.point(core::f64::consts::TAU * t),
                Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
                Curve3::Nurbs(n) => {
                    let (a, b) = n.domain();
                    n.point(a + t * (b - a))
                }
            }
        };
        for k in 0..=12 {
            let p = sample(k as f64 / 12.0);
            let pr = surf
                .project(p)
                .map_err(|_| TopoError::Precondition("imprint: projection failed"))?;
            if pr.distance > tol {
                return Err(TopoError::Precondition(
                    "imprint: curve not on face surface",
                ));
            }
        }
        let fit = keel_geom::fit::pcurve_on_analytic(curve, surf, 64, tol.max(1e-7))
            .map_err(|_| TopoError::Precondition("imprint: pcurve fit failed"))?;
        Ok((fit.curve, sample(0.0)))
    }

    fn fin_ending_at_vertex(
        &self,
        lp: crate::entity::LoopKey,
        v: crate::entity::VertexKey,
    ) -> Result<FinKey, TopoError> {
        let entry = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("no fins"))?;
        let mut cur = entry;
        loop {
            if self.fin_end_vertex(cur) == Some(v) {
                return Ok(cur);
            }
            cur = self
                .fins
                .get(cur)
                .map(|f| f.next)
                .ok_or(TopoError::StaleKey)?;
            if cur == entry {
                return Err(TopoError::Precondition("no fin ends at vertex"));
            }
        }
    }
}

/// The plane (a point on it, unit normal) of a closed planar curve.
/// None for non-planar / unsupported curve kinds.
fn closed_curve_plane(curve: &Curve3) -> Option<(Vec3, Vec3)> {
    match curve {
        Curve3::Circle(c) => Some((c.center, c.x_axis.cross(c.y_axis).try_normalize()?)),
        Curve3::Ellipse(e) => Some((e.center, e.x_axis.cross(e.y_axis).try_normalize()?)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::curve::Circle3;
    use keel_geom::surface::{Cylinder3, Frame3, Surface3};

    #[test]
    fn imprint_crossing_circle_on_cylinder_lateral() {
        // A horizontal slice of the cylinder at mid-height is a circle
        // wrapping the lateral face and crossing its vertical seam. The
        // crossing imprint splits the lateral into upper + lower.
        use keel_geom::curve::Ellipse3;
        let mut b = Body::new();
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let out = b.cylinder(frame, 2.0, 5.0).unwrap();
        let lateral = out.faces[0];
        let before = b.counts();
        let slice = Curve3::Ellipse(
            Ellipse3::new(
                Vec3::new(0.0, 0.0, 2.5),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                2.0,
                2.0,
            )
            .unwrap(),
        );
        assert!(b.closed_curve_crosses_boundary(lateral, &slice, 1e-7));
        let rep = b
            .imprint_closed_curve_crossing(lateral, &slice, 1e-7)
            .unwrap();
        assert!(b.validate().is_ok(), "crossing imprint invalid");
        let after = b.counts();
        // One new vertex (seam crossing), the split seam (+1 edge) and
        // the slice circle (+1 edge), and one new face.
        assert_eq!(after.v, before.v + 1);
        assert_eq!(after.f, before.f + 1);
        let radial = b.edge(rep.edge).map(|e| e.radial.clone()).unwrap();
        assert_eq!(radial.len(), 2, "slice circle should be manifold");
        for fk in radial {
            assert!(b.fin(fk).and_then(|f| f.pcurve).is_some());
        }
    }

    #[test]
    fn imprint_circle_on_cube_top_face() {
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        // The top face (z = 4) is the last entry.
        let top = *out.faces.last().unwrap();
        let before = b.counts();
        // A circle of radius 1 centered on the top face.
        let circle = Curve3::Circle(
            Circle3::new(
                Vec3::new(2.0, 2.0, 4.0),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                1.0,
            )
            .unwrap(),
        );
        let rep = b.imprint_closed_curve(top, &circle, 1e-9).unwrap();
        assert!(b.validate().is_ok());
        let after = b.counts();
        // One new vertex (seam), one new edge (circle), one new face
        // (disc), one new inner ring.
        assert_eq!(after.v, before.v + 1);
        assert_eq!(after.e, before.e + 1);
        assert_eq!(after.f, before.f + 1);
        assert_eq!(after.inner_rings, before.inner_rings + 1);
        // The circle edge is manifold (radial 2) and both fins carry a
        // pcurve.
        let radial = b.edge(rep.edge).map(|e| e.radial.clone()).unwrap();
        assert_eq!(radial.len(), 2);
        for fk in radial {
            assert!(b.fin(fk).and_then(|f| f.pcurve).is_some());
        }
        // The disc face is classified as inside the body interior.
        assert!(matches!(
            b.classify_point(Vec3::new(2.0, 2.0, 3.5)).unwrap(),
            crate::pmc::Containment::In(_)
        ));
    }

    #[test]
    fn imprint_open_curve_crossing_cube_face() {
        use keel_geom::curve::Line3;
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let top = *out.faces.last().unwrap();
        let before = b.counts();
        // A segment crossing the top face (z = 4) from the y=0 edge to
        // the y=4 edge at x = 2: endpoints (2,0,4) and (2,4,4).
        let seg = Curve3::Nurbs(
            keel_geom::nurbs_curve::NurbsCurve::new(
                1,
                vec![0., 0., 1., 1.],
                vec![Vec3::new(2., 0., 4.), Vec3::new(2., 4., 4.)],
                None,
            )
            .unwrap(),
        );
        let _ = Line3::new;
        let rep = b.imprint_open_curve(top, &seg, 1e-9).unwrap();
        assert!(b.validate().is_ok());
        let after = b.counts();
        // Two new boundary vertices, the split edges (+2 edges from the
        // two split_edges), the imprint edge (+1), one new face.
        assert_eq!(after.v, before.v + 2);
        assert_eq!(after.f, before.f + 1);
        let radial = b.edge(rep.edge).map(|e| e.radial.clone()).unwrap();
        assert_eq!(radial.len(), 2);
        for fk in radial {
            assert!(b.fin(fk).and_then(|f| f.pcurve).is_some());
        }
    }

    #[test]
    fn imprint_rejects_off_surface_curve() {
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let top = *out.faces.last().unwrap();
        // A circle floating above the top face (z = 5, not on z = 4).
        let circle = Curve3::Circle(
            Circle3::new(
                Vec3::new(2.0, 2.0, 5.0),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                1.0,
            )
            .unwrap(),
        );
        assert!(b.imprint_closed_curve(top, &circle, 1e-9).is_err());
        // Body unchanged (atomic precondition failure).
        assert!(b.validate().is_ok());
    }

    #[test]
    fn imprint_then_mass_properties_consistent() {
        // Imprinting must not change the volume (it only adds an edge
        // splitting a coplanar face).
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let top = *out.faces.last().unwrap();
        let v_before = b.mass_properties().unwrap().volume;
        let circle = Curve3::Circle(
            Circle3::new(
                Vec3::new(2.0, 2.0, 4.0),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                1.0,
            )
            .unwrap(),
        );
        b.imprint_closed_curve(top, &circle, 1e-9).unwrap();
        assert!(b.validate().is_ok());
        // Trimmed-face mass properties (Green's theorem, Task 6): the
        // imprint splits a coplanar face, so the volume is unchanged.
        let v_after = b.mass_properties().unwrap().volume;
        assert!(
            (v_after - v_before).abs() < 1e-9 * v_before,
            "volume changed by imprint: {v_before} -> {v_after}"
        );
        let _ = (Surface3::Cylinder, Cylinder3::new, Frame3::from_z);
    }
}
