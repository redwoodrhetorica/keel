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

/// A straight pcurve segment in the parameter cover: (start, end) UV.
pub type UvSegment = ((f64, f64), (f64, f64));

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

    /// Wire body (items 8, 18): a single straight wire edge between two
    /// points, in the infinite region (no faces). Wraps `embed_wire` and
    /// attaches the line geometry.
    pub fn wire(&mut self, p0: Vec3, p1: Vec3) -> Result<crate::ops::EmbedWireOut, TopoError> {
        let r = self.infinite_region();
        let out = self.embed_wire(r, None, p0, p1)?;
        if let Ok(line) = Line3::new(p0, p1 - p0) {
            self.attach_edge_curve(out.edge, Curve3::Line(line), true);
        }
        Ok(out)
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
        for &fk in &faces {
            self.attach_plane_pcurves(fk)?;
        }
        Ok(PrimitiveOut {
            faces,
            edges: all_edges,
            vertices,
            reports,
        })
    }

    /// Ruled loft (sweep/loft family, parity items 62-69): a solid
    /// bounded by two parallel-ordered profile loops (`bottom`, `top`,
    /// same vertex count) joined by quadrilateral side faces, plus the
    /// two cap faces. The corresponding vertices `bottom[i]`-`top[i]`
    /// must form PLANAR side quads (so each side has an exact plane);
    /// this covers tapered boxes, frusta, and prisms. Twisted/ruled
    /// lofts whose sides are non-planar need NURBS sides -- a follow-up.
    pub fn loft(&mut self, bottom: &[Vec3], top: &[Vec3]) -> Result<PrimitiveOut, TopoError> {
        let n = bottom.len();
        if n < 3 || top.len() != n {
            return Err(TopoError::Precondition(
                "loft: profiles need 3+ matching points",
            ));
        }
        if bottom.iter().chain(top).any(|p| !p.is_finite()) {
            return Err(TopoError::Precondition("loft: non-finite point"));
        }
        // Each side quad [b_i, b_{i+1}, t_{i+1}, t_i] must be planar.
        for i in 0..n {
            let (b0, b1) = (bottom[i], bottom[(i + 1) % n]);
            let (t0, t1) = (top[i], top[(i + 1) % n]);
            let nrm = (b1 - b0).cross(t0 - b0);
            if let Some(un) = nrm.try_normalize()
                && (t1 - b0).dot(un).abs() > 1e-7
            {
                return Err(TopoError::Precondition("loft: non-planar side quad"));
            }
        }
        let cb: Vec3 = bottom.iter().fold(Vec3::ZERO, |a, &p| a + p) / n as f64;
        let ct: Vec3 = top.iter().fold(Vec3::ZERO, |a, &p| a + p) / n as f64;
        let up = (ct - cb)
            .try_normalize()
            .ok_or(TopoError::Precondition("loft: degenerate profile offset"))?;

        let mut reports = Vec::new();
        let r = self.infinite_region();
        let seed = self.mvfs(r, bottom[0])?;
        reports.push(seed.report.clone());
        let lp = self
            .faces
            .get(seed.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        // Bottom rim.
        let mut rim = vec![seed.vertex];
        let mut rim_edges = Vec::new();
        let m1 = self.mev(MevSite::VertexLoop(lp), bottom[1])?;
        rim.push(m1.vertex);
        rim_edges.push(m1.edge);
        reports.push(m1.report);
        for pt in &bottom[2..] {
            let at = self.fin_ending_at(lp, *rim.last().ok_or(TopoError::StaleKey)?)?;
            let m = self.mev(MevSite::AfterFin(at), *pt)?;
            rim.push(m.vertex);
            rim_edges.push(m.edge);
            reports.push(m.report);
        }
        let fa = self.fin_ending_at(lp, rim[n - 1])?;
        let fb = self.fin_ending_at(lp, rim[0])?;
        let bottom_face = self.mef(fa, fb, None)?;
        rim_edges.push(bottom_face.edge);
        reports.push(bottom_face.report.clone());
        // Verticals to the top profile.
        let mut tops = Vec::new();
        let mut vert_edges = Vec::new();
        for (i, &rv) in rim.iter().enumerate() {
            let at = self.fin_ending_at(lp, rv)?;
            let m = self.mev(MevSite::AfterFin(at), top[i])?;
            tops.push(m.vertex);
            vert_edges.push(m.edge);
            reports.push(m.report);
        }
        // Side faces; the last mef also closes the top (seed face).
        let mut faces = vec![bottom_face.face];
        let mut top_edges = Vec::new();
        for i in 0..n {
            let a = self.fin_ending_at(lp, tops[i])?;
            let c = self.fin_ending_at(lp, tops[(i + 1) % n])?;
            let side = self.mef(a, c, None)?;
            faces.push(side.face);
            top_edges.push(side.edge);
            reports.push(side.report.clone());
        }
        faces.push(seed.face); // top cap
        self.debug_validate();

        // ---- geometry attachment ----
        // Caps: bottom outward = -up, top outward = +up.
        let bottom_frame = Frame3::from_z(bottom[0], up * -1.0).map_err(geom_err)?;
        let top_frame = Frame3::from_z(top[0], up).map_err(geom_err)?;
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
        // Side planes: normal from the quad, oriented outward (away from
        // the loft axis).
        for i in 0..n {
            let (b0, b1) = (bottom[i], bottom[(i + 1) % n]);
            let t0 = top[i];
            let mut normal = (b1 - b0)
                .cross(t0 - b0)
                .try_normalize()
                .ok_or(TopoError::Precondition("loft: degenerate side quad"))?;
            let mid = (b0 + b1 + t0) / 3.0;
            let axis_pt = cb + up * (mid - cb).dot(up);
            if normal.dot(mid - axis_pt) < 0.0 {
                normal = normal * -1.0;
            }
            let frame = Frame3::from_z(b0, normal).map_err(geom_err)?;
            self.attach_face_surface(
                faces[1 + i],
                SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(frame))),
                true,
            );
        }
        // Straight edges -> lines.
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
        for &fk in &faces {
            self.attach_plane_pcurves(fk)?;
        }
        Ok(PrimitiveOut {
            faces,
            edges: all_edges,
            vertices,
            reports,
        })
    }

    /// Full 360-degree solid of revolution (sweep/loft family, parity
    /// items 62-69) of a meridian `profile` of (radius, height) points
    /// about `frame.z`, with `frame.x` the seam-meridian reference. The
    /// profile must meet the axis (radius 0) at BOTH ends and stay off
    /// the axis (radius > 0) between; each segment revolves to a cone
    /// (changing radius) or cylinder (constant radius) band, with the
    /// two end points becoming poles. Topology: m points -> (m+1)
    /// vertices, (2m-1) edges, m faces (Euler 2).
    ///
    /// Scope: the end segments must be NON-horizontal (so the pole bands
    /// are true cones, not flat discs) and no interior segment may be
    /// horizontal (which would revolve to a holed washer). Flat end-caps
    /// and washer faces are follow-ups; pcurves are not attached yet
    /// (analytic mass_properties needs them -- use mesh_volume meanwhile;
    /// tessellation and booleans work without them).
    pub fn revolve(
        &mut self,
        frame: Frame3,
        profile: &[(f64, f64)],
    ) -> Result<PrimitiveOut, TopoError> {
        let m = profile.len();
        if m < 3 {
            return Err(TopoError::Precondition("revolve: need 3+ profile points"));
        }
        if profile
            .iter()
            .any(|&(r, h)| !r.is_finite() || !h.is_finite())
        {
            return Err(TopoError::Precondition("revolve: non-finite profile"));
        }
        if profile[0].0 != 0.0 || profile[m - 1].0 != 0.0 {
            return Err(TopoError::Precondition(
                "revolve: ends must lie on the axis",
            ));
        }
        if profile[1..m - 1].iter().any(|&(r, _)| r <= 0.0) {
            return Err(TopoError::Precondition(
                "revolve: interior must be off-axis",
            ));
        }
        // End segments non-horizontal (pole bands are cones, not discs).
        if (profile[0].1 - profile[1].1).abs() < 1e-12
            || (profile[m - 1].1 - profile[m - 2].1).abs() < 1e-12
        {
            return Err(TopoError::Precondition(
                "revolve: horizontal end-cap (follow-up)",
            ));
        }
        // Interior horizontal segments revolve to holed washers (follow-up).
        for i in 1..m - 2 {
            if (profile[i].1 - profile[i + 1].1).abs() < 1e-12 {
                return Err(TopoError::Precondition(
                    "revolve: interior washer (follow-up)",
                ));
            }
        }

        let (o, ex, ey, ez) = (frame.origin, frame.x, frame.y, frame.z);
        let s = |i: usize| o + ex * profile[i].0 + ez * profile[i].1;

        let mut reports = Vec::new();
        let r = self.infinite_region();
        // Seed at the first off-axis point s_1; build its circle and the
        // bottom cone down to the bottom pole.
        let seed = self.mvfs(r, s(1))?;
        reports.push(seed.report.clone());
        let lp = self
            .faces
            .get(seed.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let bot = self.mef_on_vertex_loop(lp, None)?;
        reports.push(bot.report.clone());
        let bot_lp = self
            .faces
            .get(bot.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let fin = self.fin_ending_at(bot_lp, seed.vertex)?;
        let mdown = self.mev(MevSite::AfterFin(fin), s(0))?;
        reports.push(mdown.report);

        // bands: (face, lower_index, upper_index).
        let mut bands = vec![(bot.face, 0usize, 1usize)];
        let mut cur_face = seed.face;
        let mut cur_lp = lp;
        let mut cur_vertex = seed.vertex;
        for i in 1..=m.saturating_sub(3) {
            let fin = self.fin_ending_at(cur_lp, cur_vertex)?;
            let up = self.mev(MevSite::AfterFin(fin), s(i + 1))?;
            reports.push(up.report);
            let at = self.fin_ending_at(cur_lp, up.vertex)?;
            let cap = self.mef(at, at, None)?;
            reports.push(cap.report.clone());
            bands.push((cur_face, i, i + 1));
            cur_face = cap.face;
            cur_lp = self
                .faces
                .get(cap.face)
                .map(|f| f.loops[0])
                .ok_or(TopoError::StaleKey)?;
            cur_vertex = up.vertex;
        }
        // Top cone: extend the working face up to the top pole.
        let fin = self.fin_ending_at(cur_lp, cur_vertex)?;
        let mtop = self.mev(MevSite::AfterFin(fin), s(m - 1))?;
        reports.push(mtop.report);
        bands.push((cur_face, m - 2, m - 1));
        self.debug_validate();

        // ---- geometry attach: a cone or cylinder per band ----
        let axis_at = |h: f64| o + ez * h;
        for &(fk, lo, hi) in &bands {
            let (r_lo, h_lo) = profile[lo];
            let (r_hi, h_hi) = profile[hi];
            // The band surface's v parameter is height along ez from its
            // frame origin; capture that origin height for the pcurves.
            let origin_h;
            if (r_hi - r_lo).abs() < 1e-12 {
                // Cylinder band (constant radius).
                origin_h = h_lo;
                let cyl_frame = Frame3 {
                    origin: axis_at(origin_h),
                    x: ex,
                    y: ey,
                    z: ez,
                };
                self.attach_face_surface(
                    fk,
                    SurfaceGeom::Analytic(Surface3::Cylinder(
                        Cylinder3::new(cyl_frame, r_lo).map_err(geom_err)?,
                    )),
                    true,
                );
            } else {
                // Cone band: anchor v=0 at the end with radius > 0.
                let (anchor_r, anchor_h) = if r_lo > 0.0 {
                    (r_lo, h_lo)
                } else {
                    (r_hi, h_hi)
                };
                origin_h = anchor_h;
                let slope = (r_hi - r_lo) / (h_hi - h_lo);
                let cone_frame = Frame3 {
                    origin: axis_at(anchor_h),
                    x: ex,
                    y: ey,
                    z: ez,
                };
                self.attach_face_surface(
                    fk,
                    SurfaceGeom::Analytic(Surface3::Cone(
                        Cone3::new(cone_frame, anchor_r, slope.atan()).map_err(geom_err)?,
                    )),
                    true,
                );
            }
            self.attach_revolve_band_pcurves(fk, origin_h, o, ez)?;
        }

        // Edges: closed -> latitude circle, open -> seam line.
        let edge_keys: Vec<EdgeKey> = self.edges.iter().map(|(k, _)| k).collect();
        let mut all_edges = Vec::new();
        for ek in edge_keys {
            let edge = self.edges.get(ek).ok_or(TopoError::StaleKey)?;
            let (va, vb) = edge.bounds;
            let closed = edge.is_closed();
            let pa = self
                .vertices
                .get(va)
                .map(|v| v.point)
                .ok_or(TopoError::StaleKey)?;
            if closed {
                let h = (pa - o).dot(ez);
                let center = axis_at(h);
                let radius = (pa - center).norm();
                let circle = Circle3::new(center, ex, ey, radius).map_err(geom_err)?;
                self.attach_edge_curve(ek, Curve3::Circle(circle), true);
            } else {
                let pb = self
                    .vertices
                    .get(vb)
                    .map(|v| v.point)
                    .ok_or(TopoError::StaleKey)?;
                let line = Line3::new(pa, pb - pa).map_err(geom_err)?;
                self.attach_edge_curve(ek, Curve3::Line(line), true);
            }
            all_edges.push(ek);
        }

        let faces: Vec<FaceKey> = bands.iter().map(|&(f, _, _)| f).collect();
        let vertices: Vec<VertexKey> = self.vertices.iter().map(|(k, _)| k).collect();
        Ok(PrimitiveOut {
            faces,
            edges: all_edges,
            vertices,
            reports,
        })
    }

    /// Partial (wedge) solid of revolution through angle `theta` of a
    /// CLOSED meridian `profile` of (radius, height) points about
    /// `frame.z`, with `frame.x` the phi=0 reference. Unlike the full
    /// `revolve`, the profile is a closed polygon held OFF the axis
    /// (radius > 0 everywhere); the two angular ends become planar cap
    /// faces (the meridian region at phi=0 and phi=theta) and each profile
    /// segment sweeps a partial cylinder (constant radius), planar
    /// annular-sector (constant height), or CONE sector (slanted) band.
    /// Topology is loft-like: n points -> 2n vertices, 3n edges, n+2 faces
    /// (Euler 2).
    ///
    /// Scope: `theta` is limited to (0, pi] for now (the arc-edge
    /// tessellation samples the short angular span). Profiles meeting the
    /// axis (true poles) and pcurves are follow-ups; use mesh_volume.
    pub fn revolve_partial(
        &mut self,
        frame: Frame3,
        profile: &[(f64, f64)],
        theta: f64,
    ) -> Result<PrimitiveOut, TopoError> {
        let n = profile.len();
        if n < 3 {
            return Err(TopoError::Precondition("revolve_partial: need 3+ points"));
        }
        if !theta.is_finite() || theta <= 1e-9 || theta > core::f64::consts::PI + 1e-12 {
            return Err(TopoError::Precondition(
                "revolve_partial: theta must be in (0, pi]",
            ));
        }
        if profile
            .iter()
            .any(|&(r, z)| !r.is_finite() || !z.is_finite() || r <= 0.0)
        {
            return Err(TopoError::Precondition(
                "revolve_partial: profile radius must be finite and > 0",
            ));
        }
        // Enforce CCW winding in the (r, z) meridian plane so the outward
        // band normals below come out consistently.
        let signed: f64 = (0..n)
            .map(|i| {
                let (r0, z0) = profile[i];
                let (r1, z1) = profile[(i + 1) % n];
                r0 * z1 - r1 * z0
            })
            .sum();
        let prof: Vec<(f64, f64)> = if signed < 0.0 {
            profile.iter().rev().copied().collect()
        } else {
            profile.to_vec()
        };

        let (o, ex, ey, ez) = (frame.origin, frame.x, frame.y, frame.z);
        let (ct, st) = (theta.cos(), theta.sin());
        let rot = ex * ct + ey * st; // phi=theta radial direction
        let bottom: Vec<Vec3> = prof.iter().map(|&(r, z)| o + ex * r + ez * z).collect();
        let top: Vec<Vec3> = prof.iter().map(|&(r, z)| o + rot * r + ez * z).collect();

        // ---- topology (identical structure to loft) ----
        let mut reports = Vec::new();
        let r = self.infinite_region();
        let seed = self.mvfs(r, bottom[0])?;
        reports.push(seed.report.clone());
        let lp = self
            .faces
            .get(seed.face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let mut rim = vec![seed.vertex];
        let mut rim_edges = Vec::new();
        let m1 = self.mev(MevSite::VertexLoop(lp), bottom[1])?;
        rim.push(m1.vertex);
        rim_edges.push(m1.edge);
        reports.push(m1.report);
        for pt in &bottom[2..] {
            let at = self.fin_ending_at(lp, *rim.last().ok_or(TopoError::StaleKey)?)?;
            let m = self.mev(MevSite::AfterFin(at), *pt)?;
            rim.push(m.vertex);
            rim_edges.push(m.edge);
            reports.push(m.report);
        }
        let fa = self.fin_ending_at(lp, rim[n - 1])?;
        let fb = self.fin_ending_at(lp, rim[0])?;
        let cap0 = self.mef(fa, fb, None)?;
        rim_edges.push(cap0.edge);
        reports.push(cap0.report.clone());
        // Verticals (arcs) to the phi=theta profile.
        let mut tops = Vec::new();
        let mut vert_edges = Vec::new();
        for (i, &rv) in rim.iter().enumerate() {
            let at = self.fin_ending_at(lp, rv)?;
            let m = self.mev(MevSite::AfterFin(at), top[i])?;
            tops.push(m.vertex);
            vert_edges.push(m.edge);
            reports.push(m.report);
        }
        // Side bands; the last mef also closes the phi=theta cap (seed face).
        let mut faces = vec![cap0.face];
        let mut top_edges = Vec::new();
        for i in 0..n {
            let a = self.fin_ending_at(lp, tops[i])?;
            let c = self.fin_ending_at(lp, tops[(i + 1) % n])?;
            let side = self.mef(a, c, None)?;
            faces.push(side.face);
            top_edges.push(side.edge);
            reports.push(side.report.clone());
        }
        faces.push(seed.face); // phi=theta cap
        self.debug_validate();

        // ---- geometry attach ----
        // Caps: phi=0 outward = -ey; phi=theta outward = -sin th ex + cos th ey.
        self.attach_face_surface(
            faces[0],
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(
                Frame3::from_z(bottom[0], ey * -1.0).map_err(geom_err)?,
            ))),
            true,
        );
        self.attach_face_surface(
            *faces.last().ok_or(TopoError::StaleKey)?,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(
                Frame3::from_z(top[0], ex * -st + ey * ct).map_err(geom_err)?,
            ))),
            true,
        );
        // Side bands: cylinder (constant r) or planar sector (constant z).
        for i in 0..n {
            let (ra, za) = prof[i];
            let (rb, zb) = prof[(i + 1) % n];
            let fk = faces[1 + i];
            if (rb - ra).abs() < 1e-12 {
                // Cylinder sector; natural normal is radial-out, so the
                // outer wall (going up, zb > za on a CCW profile) keeps
                // sense=true and the inner wall flips.
                let cyl_frame = Frame3 {
                    origin: o,
                    x: ex,
                    y: ey,
                    z: ez,
                };
                self.attach_face_surface(
                    fk,
                    SurfaceGeom::Analytic(Surface3::Cylinder(
                        Cylinder3::new(cyl_frame, ra).map_err(geom_err)?,
                    )),
                    zb > za,
                );
            } else if (zb - za).abs() < 1e-12 {
                // Planar annular sector at z = za; outward z = -(rb - ra).
                let nz = if rb > ra { ez * -1.0 } else { ez };
                self.attach_face_surface(
                    fk,
                    SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(
                        Frame3::from_z(o + ex * ra + ez * za, nz).map_err(geom_err)?,
                    ))),
                    true,
                );
            } else {
                // Cone sector (slanted segment); anchor v=0 at this end
                // (both ends off-axis) and trim angularly. As with the
                // cylinder, going up (zb > za) on a CCW meridian faces
                // radially outward.
                let slope = (rb - ra) / (zb - za);
                let cone_frame = Frame3 {
                    origin: o + ez * za,
                    x: ex,
                    y: ey,
                    z: ez,
                };
                self.attach_face_surface(
                    fk,
                    SurfaceGeom::Analytic(Surface3::Cone(
                        Cone3::new(cone_frame, ra, slope.atan()).map_err(geom_err)?,
                    )),
                    zb > za,
                );
            }
        }
        // Edges: rim/top profile segments -> lines; verticals -> arcs.
        for &ek in rim_edges.iter().chain(top_edges.iter()) {
            let (va, vb) = {
                let e = self.edges.get(ek).ok_or(TopoError::StaleKey)?;
                e.bounds
            };
            let pa = self
                .vertices
                .get(va)
                .map(|v| v.point)
                .ok_or(TopoError::StaleKey)?;
            let pb = self
                .vertices
                .get(vb)
                .map(|v| v.point)
                .ok_or(TopoError::StaleKey)?;
            let line = Line3::new(pa, pb - pa).map_err(geom_err)?;
            self.attach_edge_curve(ek, Curve3::Line(line), true);
        }
        for (i, &ek) in vert_edges.iter().enumerate() {
            let (ra, za) = prof[i];
            let circle = Circle3::new(o + ez * za, ex, ey, ra).map_err(geom_err)?;
            self.attach_edge_curve(ek, Curve3::Circle(circle), true);
        }

        let mut all_edges = Vec::new();
        all_edges.extend(rim_edges.iter().copied());
        all_edges.extend(vert_edges.iter().copied());
        all_edges.extend(top_edges.iter().copied());
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
        // Pcurves: lateral rectangle in the unwrapped cover; planar caps.
        let tau = core::f64::consts::TAU;
        self.attach_loop_uv_path(
            lp,
            &[
                ((0.0, 0.0), (tau, 0.0)),
                ((tau, 0.0), (tau, h)),
                ((tau, h), (0.0, h)),
                ((0.0, h), (0.0, 0.0)),
            ],
        )?;
        self.attach_plane_pcurves(bot.face)?;
        self.attach_plane_pcurves(top.face)?;
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
        // Pcurves: lateral path with the implicit apex closure; cap.
        let tau = core::f64::consts::TAU;
        self.attach_loop_uv_path(
            lp,
            &[
                ((0.0, 0.0), (tau, 0.0)),
                ((tau, 0.0), (tau, h)),
                ((0.0, h), (0.0, 0.0)),
            ],
        )?;
        self.attach_plane_pcurves(bot.face)?;
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

    /// Solid sphere with a NURBS (revolved rational) surface instead of
    /// the analytic `Surface3::Sphere`. Same topology as `sphere()`
    /// (V2 E1 F1, poles at +/-frame.z, seam meridian in the
    /// (frame.z, frame.x) plane). The first NURBS-faced solid (M7a).
    pub fn nurbs_sphere(&mut self, frame: Frame3, radius: f64) -> Result<PrimitiveOut, TopoError> {
        if radius <= 0.0 || !radius.is_finite() {
            return Err(TopoError::Precondition("nurbs_sphere: bad radius"));
        }
        let (center, az, fx) = (frame.origin, frame.z, frame.x);
        let s = core::f64::consts::FRAC_1_SQRT_2;
        // Rational quadratic semicircle (the meridian profile) from the
        // south pole through the +x equator point to the north pole.
        let ctrl = vec![
            center - az * radius,
            center - az * radius + fx * radius,
            center + fx * radius,
            center + az * radius + fx * radius,
            center + az * radius,
        ];
        let profile = keel_geom::nurbs_curve::NurbsCurve::new(
            2,
            vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
            ctrl,
            Some(vec![1.0, s, 1.0, s, 1.0]),
        )
        .map_err(geom_err)?;
        let surf =
            keel_geom::nurbs_surface::revolve_full(&profile, center, az).map_err(geom_err)?;

        let mut reports = Vec::new();
        let r = self.infinite_region();
        let south = center - az * radius;
        let north = center + az * radius;
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

        self.attach_face_surface(seed.face, SurfaceGeom::Nurbs(surf), true);
        self.attach_edge_curve(seam.edge, Curve3::Nurbs(profile), true);
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

    /// Attach pcurves to every fin of a PLANAR face: per fin, the UV
    /// segment between its vertices in the plane frame; closed
    /// circular rim edges get a full-circle UV pcurve. (M4 Task 4.)
    pub(crate) fn attach_plane_pcurves(&mut self, face: FaceKey) -> Result<(), TopoError> {
        let Some((sk, _)) = self.faces.get(face).and_then(|f| f.surface) else {
            return Err(TopoError::Precondition("attach_plane_pcurves: no surface"));
        };
        let Some(crate::entity::SurfaceGeom::Analytic(Surface3::Plane(plane))) =
            self.surfaces.get(sk).cloned()
        else {
            return Err(TopoError::Precondition("attach_plane_pcurves: not a plane"));
        };
        let frame = plane.frame.clone();
        let uv = |p: Vec3| -> (f64, f64) {
            let w = p - frame.origin;
            (w.dot(frame.x), w.dot(frame.y))
        };
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
                let (edge, next) = match self.fins.get(cur) {
                    Some(f) => (f.edge, f.next),
                    None => return Err(TopoError::StaleKey),
                };
                let e = self.edges.get(edge).ok_or(TopoError::StaleKey)?;
                if e.is_closed() {
                    // Circular rim: full circle in UV.
                    let Some((ck, _)) = e.curve else {
                        return Err(TopoError::Precondition("closed edge without curve"));
                    };
                    let Some(keel_geom::curve::Curve3::Circle(c)) = self.curves.get(ck).cloned()
                    else {
                        return Err(TopoError::Precondition("closed edge curve not a circle"));
                    };
                    let center = uv(c.center);
                    let Ok(uv_circle) = keel_geom::curve::Circle3::new(
                        Vec3::new(center.0, center.1, 0.0),
                        Vec3::new(1., 0., 0.),
                        Vec3::new(0., 1., 0.),
                        c.radius,
                    ) else {
                        return Err(TopoError::Precondition("bad rim circle"));
                    };
                    self.attach_pcurve(cur, keel_geom::curve::Curve3::Circle(uv_circle), true);
                } else {
                    let p0 = self
                        .fin_start_vertex(cur)
                        .and_then(|v| self.vertices.get(v).map(|x| x.point))
                        .ok_or(TopoError::StaleKey)?;
                    let p1 = self
                        .fin_end_vertex(cur)
                        .and_then(|v| self.vertices.get(v).map(|x| x.point))
                        .ok_or(TopoError::StaleKey)?;
                    self.attach_pcurve_segment(cur, uv(p0), uv(p1));
                }
                cur = next;
                if cur == entry {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Assign explicit UV segments to a loop's fins in walk order
    /// (used for periodic lateral faces where vertex projection cannot
    /// distinguish seam representatives).
    pub(crate) fn attach_loop_uv_path(
        &mut self,
        lp: crate::entity::LoopKey,
        path: &[UvSegment],
    ) -> Result<(), TopoError> {
        let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
            return Err(TopoError::Precondition("attach_loop_uv_path: vertex loop"));
        };
        let mut cur = entry;
        let mut i = 0usize;
        loop {
            if i >= path.len() {
                return Err(TopoError::Precondition(
                    "attach_loop_uv_path: too few entries",
                ));
            }
            let (a, b) = path[i];
            self.attach_pcurve_segment(cur, a, b);
            i += 1;
            cur = self
                .fins
                .get(cur)
                .map(|f| f.next)
                .ok_or(TopoError::StaleKey)?;
            if cur == entry {
                break;
            }
        }
        if i != path.len() {
            return Err(TopoError::Precondition(
                "attach_loop_uv_path: count mismatch",
            ));
        }
        Ok(())
    }

    /// Attach UV pcurves to a revolve band face so analytic
    /// mass_properties has its parameter-domain bounds. The band surface
    /// (cone/cylinder) is parameterized u = angle [0, tau], v = height
    /// along `ez` from the band frame origin (at height `origin_h` on the
    /// axis through `o`). Each latitude-circle fin spans u 0->tau at its
    /// height; each seam (open-edge) fin is vertical at u = tau when it
    /// ascends, u = 0 when it descends (matching the cone/cylinder
    /// primitives' seam convention). Collapsed pole edges contribute no
    /// fin and participate implicitly.
    fn attach_revolve_band_pcurves(
        &mut self,
        face: FaceKey,
        origin_h: f64,
        o: Vec3,
        ez: Vec3,
    ) -> Result<(), TopoError> {
        let tau = core::f64::consts::TAU;
        let lp = self
            .faces
            .get(face)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;
        let entry = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("revolve pcurves: vertex loop"))?;
        let v_of = |p: Vec3| (p - o).dot(ez) - origin_h;
        // Collect (fin, uv0, uv1) first to avoid a borrow conflict with
        // the mutating attach.
        let mut segs: Vec<(crate::entity::FinKey, UvSegment)> = Vec::new();
        let mut cur = entry;
        loop {
            let fin = self.fins.get(cur).ok_or(TopoError::StaleKey)?;
            let edge = self.edges.get(fin.edge).ok_or(TopoError::StaleKey)?;
            let (va, vb) = edge.bounds;
            let closed = edge.is_closed();
            let end_v = self.fin_end_vertex(cur).ok_or(TopoError::StaleKey)?;
            let start_v = if closed {
                end_v
            } else if end_v == vb {
                va
            } else {
                vb
            };
            let pe = self
                .vertices
                .get(end_v)
                .map(|v| v.point)
                .ok_or(TopoError::StaleKey)?;
            let (uv0, uv1) = if closed {
                let vc = v_of(pe);
                ((0.0, vc), (tau, vc))
            } else {
                let ps = self
                    .vertices
                    .get(start_v)
                    .map(|v| v.point)
                    .ok_or(TopoError::StaleKey)?;
                let (vs, ve) = (v_of(ps), v_of(pe));
                let u = if ve > vs { tau } else { 0.0 };
                ((u, vs), (u, ve))
            };
            segs.push((cur, (uv0, uv1)));
            cur = self
                .fins
                .get(cur)
                .map(|f| f.next)
                .ok_or(TopoError::StaleKey)?;
            if cur == entry {
                break;
            }
        }
        for (fin, (uv0, uv1)) in segs {
            self.attach_pcurve_segment(fin, uv0, uv1);
        }
        Ok(())
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

    #[test]
    fn wire_constructor_makes_wire_body() {
        use crate::query::BodyClass;
        let mut b = Body::new();
        b.wire(Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0)).unwrap();
        assert_eq!(b.body_class(), BodyClass::Wire, "should be a wire body");
        assert!(b.validate().is_ok(), "wire invalid: {:?}", b.validate());
    }

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
    fn loft_square_frustum() {
        // 2x2 base -> 1x1 top, height 2. Planar trapezoidal sides.
        // Frustum volume = (h/3)(A1 + A2 + sqrt(A1*A2)) = (2/3)(4+1+2)=14/3.
        let bottom = vec![
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
        ];
        let top = vec![
            Vec3::new(-0.5, -0.5, 2.0),
            Vec3::new(0.5, -0.5, 2.0),
            Vec3::new(0.5, 0.5, 2.0),
            Vec3::new(-0.5, 0.5, 2.0),
        ];
        let mut b = Body::new();
        let out = b.loft(&bottom, &top).unwrap();
        assert!(b.validate().is_ok(), "loft frustum invalid");
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6));
        assert_eq!(out.faces.len(), 6);
        edges_lie_on_adjacent_surfaces(&b, 1e-9);
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 14.0 / 3.0).abs() < 1e-9, "frustum volume {v} != 14/3");
    }

    #[test]
    fn loft_rejects_nonplanar_side() {
        // A 90-degree twist makes the side quads non-planar -> rejected.
        let bottom = vec![
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
        ];
        let top = vec![
            Vec3::new(0.0, -1.4, 2.0),
            Vec3::new(1.4, 0.0, 2.0),
            Vec3::new(0.0, 1.4, 2.0),
            Vec3::new(-1.4, 0.0, 2.0),
        ];
        let mut b = Body::new();
        assert!(b.loft(&bottom, &top).is_err(), "twisted loft should reject");
    }

    fn z_up() -> Frame3 {
        Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap()
    }

    #[test]
    fn revolve_bicone() {
        // Profile pole(-1) -> equator r1 at 0 -> pole(+1): two cones,
        // base-to-base. Each cone vol (1/3)*pi*r^2*h = pi/3; total 2pi/3.
        let mut b = Body::new();
        let out = b
            .revolve(z_up(), &[(0.0, -1.0), (1.0, 0.0), (0.0, 1.0)])
            .unwrap();
        assert!(b.validate().is_ok(), "bicone invalid");
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (3, 3, 2), "bicone counts");
        assert_eq!(out.faces.len(), 2);
        let v = b.mesh_volume();
        let expect = 2.0 * core::f64::consts::PI / 3.0;
        // Tessellation undershoot (inscribed 64-gon): within ~1%.
        assert!(
            (v - expect).abs() < expect * 0.01,
            "bicone mesh_volume {v} != ~{expect}"
        );
        // Analytic mass_properties (via the attached pcurves) is exact.
        let mv = b.mass_properties().unwrap().volume;
        assert!(
            (mv - expect).abs() < 1e-9,
            "bicone mass_properties vol {mv} != {expect}"
        );
    }

    #[test]
    fn revolve_barrel() {
        // pole(-1) -> r1 at -0.5 -> r1 at 0.5 -> pole(1): two cones + a
        // cylinder. Cones: 2*(1/3)pi*1*0.5 = pi/3; cylinder pi*1*1 = pi;
        // total 4pi/3.
        let mut b = Body::new();
        b.revolve(z_up(), &[(0.0, -1.0), (1.0, -0.5), (1.0, 0.5), (0.0, 1.0)])
            .unwrap();
        assert!(b.validate().is_ok(), "barrel invalid");
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (4, 5, 3), "barrel counts");
        let v = b.mesh_volume();
        let expect = 4.0 * core::f64::consts::PI / 3.0;
        assert!(
            (v - expect).abs() < expect * 0.01,
            "barrel mesh_volume {v} != ~{expect}"
        );
        let mv = b.mass_properties().unwrap().volume;
        assert!(
            (mv - expect).abs() < 1e-9,
            "barrel mass_properties vol {mv} != {expect}"
        );
    }

    #[test]
    fn revolve_partial_annular_sector() {
        // Rectangle meridian r in [1,2], z in [0,1], revolved by pi/2.
        // Annular-sector volume = (theta/2)(r1^2 - r0^2) h
        //   = (pi/4)(4 - 1)(1) = 3pi/4.
        let mut b = Body::new();
        let out = b
            .revolve_partial(
                z_up(),
                &[(1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0)],
                core::f64::consts::FRAC_PI_2,
            )
            .unwrap();
        assert!(b.validate().is_ok(), "partial revolve invalid");
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "annular-sector counts");
        assert_eq!(out.faces.len(), 6);
        let v = b.mesh_volume();
        let expect = 3.0 * core::f64::consts::PI / 4.0;
        // Curved (cylinder/arc) faces tessellate with chord undershoot.
        assert!(
            (v - expect).abs() < expect * 0.03,
            "annular-sector mesh_volume {v} != ~{expect}"
        );
    }

    #[test]
    fn revolve_partial_cone_sector() {
        // Off-axis triangle meridian [(1,0),(2,0),(1,1)] revolved pi/2.
        // The (2,0)->(1,1) segment is a slanted CONE-sector band. By the
        // partial Pappus theorem V = theta * R_centroid * Area:
        //   Area = 1/2, R_c = (1+2+1)/3 = 4/3, theta = pi/2
        //   V = (pi/2)(4/3)(1/2) = pi/3.
        let mut b = Body::new();
        let out = b
            .revolve_partial(
                z_up(),
                &[(1.0, 0.0), (2.0, 0.0), (1.0, 1.0)],
                core::f64::consts::FRAC_PI_2,
            )
            .unwrap();
        assert!(b.validate().is_ok(), "cone-sector revolve invalid");
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (6, 9, 5), "cone-sector counts");
        assert_eq!(out.faces.len(), 5);
        let v = b.mesh_volume();
        let expect = core::f64::consts::PI / 3.0;
        assert!(
            (v - expect).abs() < expect * 0.03,
            "cone-sector mesh_volume {v} != ~{expect}"
        );
    }

    #[test]
    fn revolve_partial_rejects_pole_and_big_angle() {
        let mut b = Body::new();
        // Profile touching the axis (r = 0) is out of scope.
        assert!(
            b.revolve_partial(
                z_up(),
                &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
                core::f64::consts::FRAC_PI_2
            )
            .is_err(),
            "pole profile should reject"
        );
        // theta > pi is out of scope for now.
        assert!(
            b.revolve_partial(
                z_up(),
                &[(1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0)],
                4.0
            )
            .is_err(),
            "theta > pi should reject"
        );
    }

    #[test]
    fn revolve_rejects_open_profile() {
        let mut b = Body::new();
        // Both ends off-axis -> not a closed solid of revolution.
        assert!(
            b.revolve(z_up(), &[(1.0, 0.0), (1.0, 1.0), (0.5, 2.0)])
                .is_err()
        );
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
