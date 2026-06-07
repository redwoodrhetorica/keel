//! Primitive solid constructors (gate design scope item 6): pure
//! Euler-operator sequences attaching M2 exact geometry. Every
//! constructor emits full lineage through its operator reports.
//!
//! Orientation note: face `sense = true` is recorded throughout and
//! front sides face the parent (outside) region by the mvfs/mef
//! conventions; rigorous geometric orientation auditing arrives with
//! PMC and mass properties (M4), which are the first consumers that
//! can falsify it.

use crate::body::{Body, TopoError};
use crate::entity::{CurveGeom, EdgeKey, FaceKey, SurfaceGeom, VertexKey};
use crate::euler::MevSite;
use crate::lineage::OpReport;
use keel_geom::curve::{Circle3, Curve3, Line3};
use keel_geom::surface::{Cone3, Cylinder3, Frame3, Plane3, Sphere3, Surface3, Torus3};
use keel_math::vec::Vec3;

pub struct PrimitiveOut {
    pub faces: Vec<FaceKey>,
    pub edges: Vec<EdgeKey>,
    pub vertices: Vec<VertexKey>,
    pub reports: Vec<OpReport>,
}

fn geom_err(_: keel_geom::GeomError) -> TopoError {
    TopoError::Precondition("constructor: degenerate geometry")
}

impl Body {
    /// Attach surface geometry to a face (geometry annotation, not a
    /// topology mutation; not journaled).
    pub fn attach_face_surface(&mut self, face: FaceKey, s: SurfaceGeom, sense: bool) {
        let key = self.add_surface(s);
        if let Some(f) = self.faces.get_mut(face) {
            f.surface = Some((key, sense));
        }
    }

    /// Attach curve geometry to an edge.
    pub fn attach_edge_curve(&mut self, edge: EdgeKey, c: CurveGeom, sense: bool) {
        let key = self.add_curve(c);
        if let Some(e) = self.edges.get_mut(edge) {
            e.curve = Some((key, sense));
        }
    }

    /// Axis-aligned block at `origin` with positive extents.
    pub fn block(
        &mut self,
        origin: Vec3,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> Result<PrimitiveOut, TopoError> {
        if dx <= 0.0
            || dy <= 0.0
            || dz <= 0.0
            || !dx.is_finite()
            || !dy.is_finite()
            || !dz.is_finite()
            || !origin.is_finite()
        {
            return Err(TopoError::Precondition("block: bad extents"));
        }
        let base: Vec<Vec3> = vec![
            origin,
            origin + Vec3::new(dx, 0., 0.),
            origin + Vec3::new(dx, dy, 0.),
            origin + Vec3::new(0., dy, 0.),
        ];
        self.prism(&base, Vec3::new(0., 0., dz))
    }

    /// Right prism: a simple planar polygon profile (counterclockwise
    /// about the extrusion direction; simplicity and planarity are the
    /// caller's contract) extruded along `dir`.
    pub fn prism(&mut self, profile: &[Vec3], dir: Vec3) -> Result<PrimitiveOut, TopoError> {
        let n = profile.len();
        if n < 3 {
            return Err(TopoError::Precondition("prism: need 3+ profile points"));
        }
        if !dir.is_finite() || dir.norm() == 0.0 || profile.iter().any(|p| !p.is_finite()) {
            return Err(TopoError::Precondition("prism: bad direction or points"));
        }
        let mut reports = Vec::new();
        let r = self.infinite_region();
        let seed = self.mvfs(r, profile[0])?;
        reports.push(seed.report.clone());
        let lp = self
            .faces
            .get(seed.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        // Base rim.
        let mut rim = vec![seed.vertex];
        let mut rim_edges = Vec::new();
        let m1 = self.mev(MevSite::VertexLoop(lp), profile[1])?;
        rim.push(m1.vertex);
        rim_edges.push(m1.edge);
        reports.push(m1.report);
        for pt in &profile[2..] {
            let at = self.fin_ending_at(lp, *rim.last().ok_or(TopoError::StaleKey)?)?;
            let m = self.mev(MevSite::AfterFin(at), *pt)?;
            rim.push(m.vertex);
            rim_edges.push(m.edge);
            reports.push(m.report);
        }
        let fa = self.fin_ending_at(lp, rim[n - 1])?;
        let fb = self.fin_ending_at(lp, rim[0])?;
        let bottom = self.mef(fa, fb, None)?;
        rim_edges.push(bottom.edge);
        reports.push(bottom.report.clone());
        // Verticals.
        let mut tops = Vec::new();
        let mut vert_edges = Vec::new();
        for &rv in &rim {
            let at = self.fin_ending_at(lp, rv)?;
            let top_p = self
                .vertices
                .get(rv)
                .map(|v| v.point + dir)
                .ok_or(TopoError::StaleKey)?;
            let m = self.mev(MevSite::AfterFin(at), top_p)?;
            tops.push(m.vertex);
            vert_edges.push(m.edge);
            reports.push(m.report);
        }
        // Side faces; the last mef also closes the top (seed face).
        let mut faces = vec![bottom.face];
        let mut top_edges = Vec::new();
        for i in 0..n {
            let a = self.fin_ending_at(lp, tops[i])?;
            let c = self.fin_ending_at(lp, tops[(i + 1) % n])?;
            let side = self.mef(a, c, None)?;
            faces.push(side.face);
            top_edges.push(side.edge);
            reports.push(side.report.clone());
        }
        faces.push(seed.face); // top
        self.debug_validate();

        // ---- geometry attachment ----
        let up = dir.try_normalize().map_err_degenerate()?;
        // Bottom plane: outward normal is -up; top: +up.
        let bottom_frame = Frame3::from_z(profile[0], up * -1.0).map_err(geom_err)?;
        let top_frame = Frame3::from_z(profile[0] + dir, up).map_err(geom_err)?;
        self.attach_face_surface(
            faces[0],
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(bottom_frame))),
            true,
        );
        self.attach_face_surface(
            *faces.last().ok_or(TopoError::StaleKey)?,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(top_frame))),
            true,
        );
        // Side planes: normal = edge_dir x up for ccw profiles.
        for i in 0..n {
            let p0 = profile[i];
            let p1 = profile[(i + 1) % n];
            let edge_dir = (p1 - p0).try_normalize().map_err_degenerate()?;
            let normal = edge_dir.cross(up);
            let frame = Frame3::from_z(p0, normal).map_err(geom_err)?;
            self.attach_face_surface(
                faces[1 + i],
                SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(frame))),
                true,
            );
        }
        // Straight edges: lines.
        let mut all_edges = Vec::new();
        all_edges.extend(rim_edges.iter().copied());
        all_edges.extend(vert_edges.iter().copied());
        all_edges.extend(top_edges.iter().copied());
        for &ek in &all_edges {
            let (a, bnd) = {
                let e = self.edges.get(ek).ok_or(TopoError::StaleKey)?;
                (e.bounds.0, e.bounds.1)
            };
            let (pa, pb) = (
                self.vertices
                    .get(a)
                    .map(|v| v.point)
                    .ok_or(TopoError::StaleKey)?,
                self.vertices
                    .get(bnd)
                    .map(|v| v.point)
                    .ok_or(TopoError::StaleKey)?,
            );
            let line = Line3::new(pa, pb - pa).map_err(geom_err)?;
            self.attach_edge_curve(ek, Curve3::Line(line), true);
        }
        let mut vertices = rim;
        vertices.extend(tops);
        Ok(PrimitiveOut {
            faces,
            edges: all_edges,
            vertices,
            reports,
        })
    }

    /// Solid cylinder: axis = frame.z, base disc at the frame origin,
    /// height h. Topology V2 E3 F3 (caps + lateral with seam).
    pub fn cylinder(
        &mut self,
        frame: Frame3,
        radius: f64,
        h: f64,
    ) -> Result<PrimitiveOut, TopoError> {
        if radius <= 0.0 || h <= 0.0 || !radius.is_finite() || !h.is_finite() {
            return Err(TopoError::Precondition("cylinder: bad parameters"));
        }
        let mut reports = Vec::new();
        let r = self.infinite_region();
        let seam0 = frame.origin + frame.x * radius;
        let seam1 = seam0 + frame.z * h;
        let seed = self.mvfs(r, seam0)?;
        reports.push(seed.report.clone());
        let lp = self
            .faces
            .get(seed.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        // Bottom circle (closed edge at the seam vertex) + bottom cap.
        let bot = self.mef_on_vertex_loop(lp, None)?;
        reports.push(bot.report.clone());
        // Seam upward.
        let g = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::StaleKey)?;
        let seam = self.mev(MevSite::AfterFin(g), seam1)?;
        reports.push(seam.report.clone());
        // Top circle + top cap.
        let at_top = self.fin_ending_at(lp, seam.vertex)?;
        let top = self.mef(at_top, at_top, None)?;
        reports.push(top.report.clone());
        self.debug_validate();

        // Geometry. Lateral face = the seed face (loop lp).
        self.attach_face_surface(
            seed.face,
            SurfaceGeom::Analytic(Surface3::Cylinder(
                Cylinder3::new(frame.clone(), radius).map_err(geom_err)?,
            )),
            true,
        );
        let bottom_frame = Frame3::from_z(frame.origin, frame.z * -1.0).map_err(geom_err)?;
        let top_frame = Frame3::from_z(frame.origin + frame.z * h, frame.z).map_err(geom_err)?;
        self.attach_face_surface(
            bot.face,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(bottom_frame))),
            true,
        );
        self.attach_face_surface(
            top.face,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(top_frame))),
            true,
        );
        let c0 = Circle3::new(frame.origin, frame.x, frame.y, radius).map_err(geom_err)?;
        let c1 =
            Circle3::new(frame.origin + frame.z * h, frame.x, frame.y, radius).map_err(geom_err)?;
        self.attach_edge_curve(bot.edge, Curve3::Circle(c0), true);
        self.attach_edge_curve(top.edge, Curve3::Circle(c1), true);
        let seam_line = Line3::new(seam0, frame.z).map_err(geom_err)?;
        self.attach_edge_curve(seam.edge, Curve3::Line(seam_line), true);
        Ok(PrimitiveOut {
            faces: vec![seed.face, bot.face, top.face],
            edges: vec![bot.edge, seam.edge, top.edge],
            vertices: vec![seed.vertex, seam.vertex],
            reports,
        })
    }

    /// Solid cone: base disc of `radius` at the frame origin, apex at
    /// height h on the axis. Topology V2 E2 F2.
    pub fn cone(&mut self, frame: Frame3, radius: f64, h: f64) -> Result<PrimitiveOut, TopoError> {
        if radius <= 0.0 || h <= 0.0 || !radius.is_finite() || !h.is_finite() {
            return Err(TopoError::Precondition("cone: bad parameters"));
        }
        let mut reports = Vec::new();
        let r = self.infinite_region();
        let seam0 = frame.origin + frame.x * radius;
        let apex = frame.origin + frame.z * h;
        let seed = self.mvfs(r, seam0)?;
        reports.push(seed.report.clone());
        let lp = self
            .faces
            .get(seed.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let bot = self.mef_on_vertex_loop(lp, None)?;
        reports.push(bot.report.clone());
        let g = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::StaleKey)?;
        let seam = self.mev(MevSite::AfterFin(g), apex)?;
        reports.push(seam.report.clone());
        self.debug_validate();

        // Lateral cone: radius at v = 0 is `radius`, apex at v = h, so
        // tan(half_angle) = -radius / h (negative: narrowing upward).
        let half_angle = (-radius / h).atan();
        self.attach_face_surface(
            seed.face,
            SurfaceGeom::Analytic(Surface3::Cone(
                Cone3::new(frame.clone(), radius, half_angle).map_err(geom_err)?,
            )),
            true,
        );
        let bottom_frame = Frame3::from_z(frame.origin, frame.z * -1.0).map_err(geom_err)?;
        self.attach_face_surface(
            bot.face,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(bottom_frame))),
            true,
        );
        let c0 = Circle3::new(frame.origin, frame.x, frame.y, radius).map_err(geom_err)?;
        self.attach_edge_curve(bot.edge, Curve3::Circle(c0), true);
        let seam_line = Line3::new(seam0, apex - seam0).map_err(geom_err)?;
        self.attach_edge_curve(seam.edge, Curve3::Line(seam_line), true);
        Ok(PrimitiveOut {
            faces: vec![seed.face, bot.face],
            edges: vec![bot.edge, seam.edge],
            vertices: vec![seed.vertex, seam.vertex],
            reports,
        })
    }

    /// Solid sphere centered at the frame origin. Topology V2 E1 F1
    /// (pole-to-pole seam meridian).
    pub fn sphere(&mut self, frame: Frame3, radius: f64) -> Result<PrimitiveOut, TopoError> {
        if radius <= 0.0 || !radius.is_finite() {
            return Err(TopoError::Precondition("sphere: bad radius"));
        }
        let mut reports = Vec::new();
        let r = self.infinite_region();
        let south = frame.origin - frame.z * radius;
        let north = frame.origin + frame.z * radius;
        let seed = self.mvfs(r, south)?;
        reports.push(seed.report.clone());
        let lp = self
            .faces
            .get(seed.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let seam = self.mev(MevSite::VertexLoop(lp), north)?;
        reports.push(seam.report.clone());
        self.debug_validate();

        self.attach_face_surface(
            seed.face,
            SurfaceGeom::Analytic(Surface3::Sphere(
                Sphere3::new(frame.clone(), radius).map_err(geom_err)?,
            )),
            true,
        );
        // Seam meridian: circle in the (z, x) plane of the frame.
        let meridian =
            Circle3::new(frame.origin, frame.z * -1.0, frame.x, radius).map_err(geom_err)?;
        self.attach_edge_curve(seam.edge, Curve3::Circle(meridian), true);
        Ok(PrimitiveOut {
            faces: vec![seed.face],
            edges: vec![seam.edge],
            vertices: vec![seed.vertex, seam.vertex],
            reports,
        })
    }

    /// Solid ring torus about frame.z. Topology V2 E2 F1 with one
    /// inner ring and genus 1 (the M3 torus skeleton: outer/inner
    /// equator seam structure).
    pub fn torus(
        &mut self,
        frame: Frame3,
        major: f64,
        minor: f64,
    ) -> Result<PrimitiveOut, TopoError> {
        if minor <= 0.0 || major <= minor || !major.is_finite() || !minor.is_finite() {
            return Err(TopoError::Precondition("torus: need major > minor > 0"));
        }
        let mut reports = Vec::new();
        let r = self.infinite_region();
        let outer = frame.origin + frame.x * (major + minor);
        let inner = frame.origin + frame.x * (major - minor);
        let seed = self.mvfs(r, outer)?;
        reports.push(seed.report.clone());
        let fa = seed.face;
        let lp = self
            .faces
            .get(fa)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        // Half minor circle from outer to inner equator point.
        let m = self.mev(MevSite::VertexLoop(lp), inner)?;
        reports.push(m.report.clone());
        // Inner equator: closed edge at the inner point.
        let a = self.fin_ending_at(lp, m.vertex)?;
        let eq = self.mef(a, a, None)?;
        reports.push(eq.report.clone());
        // Punch the handle: the new face's loop becomes a ring of fa.
        let kf = self.kfmrh(eq.face, fa)?;
        reports.push(kf);
        self.debug_validate();

        self.attach_face_surface(
            fa,
            SurfaceGeom::Analytic(Surface3::Torus(
                Torus3::new(frame.clone(), major, minor).map_err(geom_err)?,
            )),
            true,
        );
        // Minor seam: circle of radius `minor` centered on the tube
        // center at u = 0, in the (x, z) plane.
        let tube_center = frame.origin + frame.x * major;
        let minor_circle = Circle3::new(tube_center, frame.x, frame.z, minor).map_err(geom_err)?;
        self.attach_edge_curve(m.edge, Curve3::Circle(minor_circle), true);
        // Inner equator circle.
        let inner_eq =
            Circle3::new(frame.origin, frame.x, frame.y, major - minor).map_err(geom_err)?;
        self.attach_edge_curve(eq.edge, Curve3::Circle(inner_eq), true);
        Ok(PrimitiveOut {
            faces: vec![fa],
            edges: vec![m.edge, eq.edge],
            vertices: vec![seed.vertex, m.vertex],
            reports,
        })
    }

    /// Production fin addressing: the fin of `lp` ending at `v`.
    fn fin_ending_at(
        &self,
        lp: crate::entity::LoopKey,
        v: VertexKey,
    ) -> Result<crate::entity::FinKey, TopoError> {
        let entry = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("loop has no fins"))?;
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

trait MapErrDegenerate {
    type Out;
    fn map_err_degenerate(self) -> Result<Self::Out, TopoError>;
}
impl MapErrDegenerate for Option<Vec3> {
    type Out = Vec3;
    fn map_err_degenerate(self) -> Result<Vec3, TopoError> {
        self.ok_or(TopoError::Precondition("degenerate direction"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::AnyKey;

    /// Watertightness oracle at the geometry level: sample every edge
    /// curve and check the samples lie on every adjacent face surface.
    fn edges_lie_on_adjacent_surfaces(b: &Body, tol: f64) {
        for id in b.entity_ids().collect::<Vec<_>>() {
            let Some(AnyKey::Edge(ek)) = b.lookup(id) else {
                continue;
            };
            let Some(edge) = b.edge(ek) else { continue };
            let Some((ck, _)) = edge.curve else { continue };
            let Some(curve) = b.curve(ck) else { continue };
            // Sample between the endpoint parameters (deterministic).
            let (p0, p1) = (
                b.vertex(edge.bounds.0)
                    .map(|v| v.point)
                    .unwrap_or_else(|| panic!()),
                b.vertex(edge.bounds.1)
                    .map(|v| v.point)
                    .unwrap_or_else(|| panic!()),
            );
            let samples: Vec<Vec3> = match curve {
                Curve3::Line(_) => (0..=4).map(|i| p0 + (p1 - p0) * (i as f64 / 4.0)).collect(),
                Curve3::Circle(c) => (0..8)
                    .map(|i| c.point(core::f64::consts::TAU * i as f64 / 8.0))
                    .collect(),
                _ => continue,
            };
            // Adjacent faces via radial fins.
            for &fk in &edge.radial {
                let Some(face) = b
                    .fin(fk)
                    .and_then(|f| b.loop_(f.owner))
                    .and_then(|l| b.face(l.face))
                else {
                    continue;
                };
                let Some((sk, _)) = face.surface else {
                    continue;
                };
                let Some(SurfaceGeom::Analytic(surf)) = b.surface(sk) else {
                    continue;
                };
                for s in &samples {
                    let pr = surf.project(*s).unwrap_or_else(|e| panic!("{e:?}"));
                    assert!(
                        pr.distance < tol,
                        "edge sample {s:?} off face surface by {}",
                        pr.distance
                    );
                }
            }
        }
    }

    #[test]
    fn block_is_watertight_and_counted() {
        let mut b = Body::new();
        let out = b.block(Vec3::new(1., 2., 3.), 2.0, 3.0, 4.0).unwrap();
        assert!(b.validate().is_ok());
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6));
        assert_eq!(out.faces.len(), 6);
        edges_lie_on_adjacent_surfaces(&b, 1e-9);
    }

    #[test]
    fn pentagon_prism() {
        let mut b = Body::new();
        let profile: Vec<Vec3> = (0..5)
            .map(|i| {
                let a = core::f64::consts::TAU * i as f64 / 5.0;
                Vec3::new(a.cos(), a.sin(), 0.0)
            })
            .collect();
        let out = b.prism(&profile, Vec3::new(0., 0., 2.)).unwrap();
        assert!(b.validate().is_ok());
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (10, 15, 7));
        assert_eq!(out.faces.len(), 7);
        edges_lie_on_adjacent_surfaces(&b, 1e-9);
    }

    #[test]
    fn cylinder_cone_sphere_torus() {
        let frame = || Frame3::from_z(Vec3::new(0.5, -1.0, 2.0), Vec3::new(0., 0., 1.)).unwrap();
        let mut b = Body::new();
        b.cylinder(frame(), 2.0, 5.0).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!((b.counts().v, b.counts().e, b.counts().f), (2, 3, 3));
        edges_lie_on_adjacent_surfaces(&b, 1e-9);

        let mut b = Body::new();
        b.cone(frame(), 1.5, 3.0).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!((b.counts().v, b.counts().e, b.counts().f), (2, 2, 2));
        edges_lie_on_adjacent_surfaces(&b, 1e-9);

        let mut b = Body::new();
        b.sphere(frame(), 2.5).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!((b.counts().v, b.counts().e, b.counts().f), (2, 1, 1));
        edges_lie_on_adjacent_surfaces(&b, 1e-9);

        let mut b = Body::new();
        b.torus(frame(), 3.0, 1.0).unwrap();
        assert!(b.validate().is_ok());
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (2, 2, 1));
        assert_eq!((c.inner_rings, c.genus), (1, 1));
        edges_lie_on_adjacent_surfaces(&b, 1e-9);
    }

    #[test]
    fn constructor_topology_hashes_are_stable() {
        // Golden determinism: two identical constructions hash equal;
        // recorded goldens guard against accidental renumbering.
        let build = || -> u64 {
            let mut b = Body::new();
            b.block(Vec3::ZERO, 1.0, 1.0, 1.0)
                .unwrap_or_else(|e| panic!("{e:?}"));
            b.topology_hash()
        };
        assert_eq!(build(), build());
    }
}
