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
        // pcurve by inversion (analytic or NURBS).
        let (pcurve, seam3) = self.curve_pcurve_on_any(face, curve, tol)?;

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

    /// Imprint a CLOSED POLYGON ring lying strictly in `face`'s
    /// interior (the ENCLOSED-POCKET coincidence: the other operand's
    /// mating face sits wholly inside this one, so the overlap
    /// boundary never reaches this face's edges). Generalizes the
    /// closed-circle imprint: spur from the outer loop to the first
    /// corner, mev around the polygon, close with mef back to the
    /// spur vertex (the spur-out fin, so the pocket cycle is the
    /// forward chain), then kemr the spur bridge: the polygon becomes
    /// an inner RING of `face` and the pocket a new face on the same
    /// surface. Polygon edges are straight and carry no curve
    /// geometry (the planar-coincidence application).
    pub fn imprint_closed_polyline(
        &mut self,
        face: FaceKey,
        pts: &[keel_math::vec::Vec3],
    ) -> Result<(), TopoError> {
        if pts.len() < 3 {
            return Err(TopoError::Precondition("imprint_ring: too few corners"));
        }
        let lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())
            .ok_or(TopoError::StaleKey)?;
        let outer_fin = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("imprint_ring: no outer fin"))?;
        // Spur to the first corner, then chain the rest. Chain edges
        // carry their exact Line geometry (the house convention;
        // curveless edges are invisible to fin_curve_samples and the
        // UV interior-point machinery).
        let spur = self.mev(MevSite::AfterFin(outer_fin), pts[0])?;
        let v0 = spur.vertex;
        let spur_edge = spur.edge;
        let mut prev = v0;
        let mut prev_pt = pts[0];
        for &p in &pts[1..] {
            let fin_prev = self.fin_ending_at_vertex(lp, prev)?;
            let mv = self.mev(MevSite::AfterFin(fin_prev), p)?;
            if let Ok(l) = keel_geom::curve::Line3::new(prev_pt, p - prev_pt) {
                self.attach_edge_curve(mv.edge, Curve3::Line(l), true);
            }
            prev = mv.vertex;
            prev_pt = p;
        }
        // Close the ring. mef moves fin_a.next ..= fin_b onto the NEW
        // face, so fin_a = the SPUR-OUT fin (whose next is the first
        // chain fin) and fin_b = the chain tip's in-fin: the new face
        // is exactly the forward polygon cycle (the pocket), and the
        // host keeps the outer boundary, the back chain, and the spur.
        let fin_b = self.fin_ending_at_vertex(lp, prev)?;
        let fin_a = {
            let radial = self
                .edges
                .get(spur_edge)
                .map(|e| e.radial.clone())
                .unwrap_or_default();
            radial
                .into_iter()
                .find(|&fk| self.fin_end_vertex(fk) == Some(v0))
                .ok_or(TopoError::Precondition("imprint_ring: no spur fin"))?
        };
        let mef = self.mef(fin_a, fin_b, None)?;
        let pocket = mef.face;
        if let Ok(l) = keel_geom::curve::Line3::new(pts[0], pts[pts.len() - 1] - pts[0]) {
            self.attach_edge_curve(mef.edge, Curve3::Line(l), true);
        }
        // Kill the spur bridge: the polygon becomes an inner ring.
        let bridge = {
            let radial = self
                .edges
                .get(spur_edge)
                .map(|e| e.radial.clone())
                .unwrap_or_default();
            radial
                .into_iter()
                .find(|&fk| {
                    self.fins
                        .get(fk)
                        .map(|f| self.loops.get(f.owner).map(|l| l.face) == Some(face))
                        .unwrap_or(false)
                })
                .ok_or(TopoError::Precondition("imprint_ring: no bridge fin"))?
        };
        self.kemr(bridge)?;
        if let Some((sk, sense)) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(pf) = self.faces.get_mut(pocket)
        {
            pf.surface = Some((sk, sense));
        }
        self.debug_validate();
        Ok(())
    }

    /// Imprint an OPEN POLYLINE chain whose two END points lie on the
    /// face's outer-loop edges and whose interior corners lie in the
    /// face interior (the partial-overlap coincidence cuts of research
    /// file 39 sec 1: the overlap boundary enters and leaves the
    /// subject face through its boundary, cornering in between).
    /// Surgery: split the boundary at the entry point, spur (mev)
    /// through the interior corners, and close with a split_face to
    /// the exit point: the proven spur-chain pattern of the setback
    /// corner. Chain edges are straight and carry no curve geometry
    /// (the planar-coincidence application).
    pub fn imprint_open_polyline(
        &mut self,
        face: FaceKey,
        pts: &[keel_math::vec::Vec3],
        tol: f64,
    ) -> Result<(), TopoError> {
        if pts.len() < 2 {
            return Err(TopoError::Precondition("imprint_polyline: too short"));
        }
        let lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())
            .ok_or(TopoError::StaleKey)?;
        let start_edge = self.boundary_edge_containing(lp, pts[0], tol)?;
        let end_edge = self.boundary_edge_containing(lp, pts[pts.len() - 1], tol)?;
        if start_edge == end_edge {
            return Err(TopoError::Precondition(
                "imprint_polyline: both ends on one edge (unsupported)",
            ));
        }
        let se = self.split_edge(start_edge, pts[0])?;
        let ee = self.split_edge(end_edge, pts[pts.len() - 1])?;
        let mut prev = se.vertex;
        let mut prev_pt = pts[0];
        for &p in &pts[1..pts.len() - 1] {
            let fin_prev = self.fin_ending_at_vertex(lp, prev)?;
            let mv = self.mev(crate::euler::MevSite::AfterFin(fin_prev), p)?;
            if let Ok(l) = keel_geom::curve::Line3::new(prev_pt, p - prev_pt) {
                self.attach_edge_curve(mv.edge, Curve3::Line(l), true);
            }
            prev = mv.vertex;
            prev_pt = p;
        }
        let fin_a = self.fin_ending_at_vertex(lp, prev)?;
        let fin_b = self.fin_ending_at_vertex(lp, ee.vertex)?;
        let split = self.split_face(fin_a, fin_b, None)?;
        if let Ok(l) = keel_geom::curve::Line3::new(prev_pt, pts[pts.len() - 1] - prev_pt) {
            self.attach_edge_curve(split.edge, Curve3::Line(l), true);
        }
        if let Some((sk, sense)) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(nf) = self.faces.get_mut(split.face_new)
        {
            nf.surface = Some((sk, sense));
        }
        self.debug_validate();
        Ok(())
    }

    /// SYNTHESIZE the missing lateral seam on a TUBE face (a cylinder
    /// band bounded by two closed rims and nothing else, the primitive
    /// construction): bridge the rims' seam vertices with the ruling
    /// edge (mekr), giving wrap-circle imprints a seam line to cross.
    /// Without it the drill-difference imprint silently no-opped (the
    /// crossing imprint found no seam; the interior-ring imprint is
    /// topologically wrong for a non-contractible wrap) and the weak
    /// curved-result gate let the unsplit body through.
    pub(crate) fn synthesize_lateral_seam(&mut self, face: FaceKey) -> Result<(), TopoError> {
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .ok_or(TopoError::StaleKey)?;
        if loops.len() != 2 {
            return Err(TopoError::Precondition(
                "seam synthesis: needs a two-rim tube face",
            ));
        }
        let rim_fin = |body: &Self, lk: crate::entity::LoopKey| -> Option<FinKey> {
            let entry = body.loops.get(lk)?.fin?;
            let fin = body.fins.get(entry)?;
            // A rim loop is a single closed edge.
            (fin.next == entry && body.edges.get(fin.edge).map(|e| e.is_closed()) == Some(true))
                .then_some(entry)
        };
        let f0 =
            rim_fin(self, loops[0]).ok_or(TopoError::Precondition("seam synthesis: rim loop 0"))?;
        let f1 =
            rim_fin(self, loops[1]).ok_or(TopoError::Precondition("seam synthesis: rim loop 1"))?;
        let v_of = |body: &Self, fk: FinKey| -> Option<Vec3> {
            let e = body.fins.get(fk)?.edge;
            let v = body.edges.get(e)?.bounds.0;
            Some(body.vertices.get(v)?.point)
        };
        let p0 = v_of(self, f0).ok_or(TopoError::StaleKey)?;
        let p1 = v_of(self, f1).ok_or(TopoError::StaleKey)?;
        // The seam vertices must share a ruling (the primitive puts
        // both rim seams at the frame's zero angle). On a cylinder the
        // ruling is the axis; on a cone it is the slanted generator
        // through p0 (radial * tan(half_angle) + axis).
        let d = p1 - p0;
        let axis = match self.face_surface3(face) {
            Some(keel_geom::surface::Surface3::Cylinder(c)) => c.frame.z,
            Some(keel_geom::surface::Surface3::Cone(c)) => {
                let z = c.frame.z;
                let w = p0 - c.frame.origin;
                let rad = (w - z * w.dot(z))
                    .try_normalize()
                    .ok_or(TopoError::Precondition("seam synthesis: apex seam vertex"))?;
                (rad * c.half_angle.tan() + z)
                    .try_normalize()
                    .ok_or(TopoError::Precondition("seam synthesis: degenerate ruling"))?
            }
            _ => {
                return Err(TopoError::Precondition(
                    "seam synthesis: not a cylinder/cone lateral",
                ));
            }
        };
        if d.cross(axis).norm() > 1e-6 * d.norm().max(1.0) {
            return Err(TopoError::Precondition(
                "seam synthesis: rim seams not on one ruling",
            ));
        }
        let out = self.mekr(f0, f1)?;
        if let Ok(l) = keel_geom::curve::Line3::new(p0, d) {
            self.attach_edge_curve(out.edge, Curve3::Line(l), true);
        }
        self.debug_validate();
        Ok(())
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
        // The wrap circle is a NON-contractible loop, so a single closed
        // mef would bound a degenerate disc. Split it into TWO ARCS at
        // the seam crossing P and its antipode Q (= 2*center - P, on the
        // circle): the lateral then splits cleanly into the two bands,
        // each sharing both arcs, and the result circle carries two
        // vertices P, Q (which the cross-operand glue matches against).
        let q = pt * 2.0 - p;
        // Split the seam line at P.
        let se = self.split_edge(crossed_edge, p)?;
        let lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())
            .ok_or(TopoError::StaleKey)?;
        let seam_fins = self.loop_fins_ending_at_vertex(lp, se.vertex);
        if seam_fins.len() != 2 {
            return Err(TopoError::Precondition(
                "crossing imprint: expected two seam fins at the crossing",
            ));
        }
        let (fin_a, fin_b) = (seam_fins[0], seam_fins[1]);
        // Spur arc1 (P -> Q).
        let m = self.mev(MevSite::AfterFin(fin_a), q)?;
        let arc1_edge = m.edge;
        let fin_arc1_out = self.fin_ending_at_vertex(lp, m.vertex)?;
        // Arc2 (Q -> P) closes the circle and splits the face into bands.
        let surf_key = self.faces.get(face).and_then(|f| f.surface);
        let mef = self.mef(fin_arc1_out, fin_b, surf_key)?;
        let arc2_edge = mef.edge;
        let new_face = mef.face;
        // Geometry: the full circle curve + pcurve on both arc edges'
        // fins; both faces keep the surface. (TASK 29 NOTE: recording
        // arc_sweep = pi here is what arc identity wants, but
        // cyl_angular_span's first-arc shortcut reads sweeps as AZIMUTH
        // spans and HALVED every face carrying them: the torus fillets
        // regressed. The recording lives in imprint_crossing_pair until
        // the sweep semantic and the span machinery are reconciled.)
        let pkey = self.add_curve(Curve3::Nurbs(pcurve));
        for arc in [arc1_edge, arc2_edge] {
            let ckey = self.add_curve(curve.clone());
            if let Some(e) = self.edges.get_mut(arc) {
                e.curve = Some((ckey, true));
            }
            let radial = self
                .edges
                .get(arc)
                .map(|e| e.radial.clone())
                .unwrap_or_default();
            for fk in radial {
                if let Some(f) = self.fins.get_mut(fk) {
                    f.pcurve = Some((pkey, true));
                }
            }
        }
        if let Some((sk, sense)) = surf_key
            && let Some(nf) = self.faces.get_mut(new_face)
        {
            nf.surface = Some((sk, sense));
        }
        self.debug_validate();
        Ok(ImprintReport {
            edge: arc1_edge,
            faces: vec![face, new_face],
        })
    }

    /// The CROSSING-PAIR arrangement imprint (task 29, the equal-radii
    /// crossing-cylinder slice): two closed planar seam curves on one
    /// cylinder lateral, mutually crossing at exactly two points `xs`.
    /// Sequential reduction to proven primitives: curve 1 imprints as
    /// the standard wrap (seam synthesis + antipodal crossing split,
    /// each arc RECORDING its half via arc_sweep); its arcs then split
    /// at the exact crossings; curve 2 splits into two arcs at the
    /// crossings and each imprints as an open boundary-to-boundary
    /// curve between the crossing vertices on whichever piece hosts it.
    pub(crate) fn imprint_crossing_pair(
        &mut self,
        face: FaceKey,
        c1: &Curve3,
        c2: &Curve3,
        xs: &[Vec3],
        tol: f64,
    ) -> Result<Vec<crate::entity::EdgeKey>, TopoError> {
        use crate::entity::EdgeKey;
        let tau = core::f64::consts::TAU;
        if xs.len() != 2 {
            return Err(TopoError::Precondition("crossing pair: need two crossings"));
        }
        let ang_on = |c: &Curve3, p: Vec3| -> Option<f64> {
            match c {
                Curve3::Circle(ci) => {
                    let w = p - ci.center;
                    Some(w.dot(ci.y_axis).atan2(w.dot(ci.x_axis)).rem_euclid(tau))
                }
                Curve3::Ellipse(el) => {
                    let w = p - el.center;
                    Some(
                        (w.dot(el.y_axis) / el.b)
                            .atan2(w.dot(el.x_axis) / el.a)
                            .rem_euclid(tau),
                    )
                }
                _ => None,
            }
        };
        // Carrier identity INCLUDING the plane: the symmetric Steinmetz
        // pair shares center and semi-axes exactly (only the planes
        // differ), so a center+axes test CONFLATES e1 with e2 and the
        // half-fix pass mangles both.
        let same_conic = |cv: &Curve3, target: &Curve3| -> bool {
            match (cv, target) {
                (Curve3::Circle(a), Curve3::Circle(b)) => {
                    (a.center - b.center).norm() < 1e-9
                        && (a.radius - b.radius).abs() < 1e-9
                        && (a.x_axis.cross(a.y_axis) - b.x_axis.cross(b.y_axis)).norm() < 1e-9
                }
                (Curve3::Ellipse(a), Curve3::Ellipse(b)) => {
                    (a.center - b.center).norm() < 1e-9
                        && (a.a - b.a).abs() < 1e-9
                        && (a.b - b.b).abs() < 1e-9
                        && (a.x_axis.cross(a.y_axis) - b.x_axis.cross(b.y_axis)).norm() < 1e-9
                }
                _ => false,
            }
        };
        // 1. Curve 1: the standard closed-wrap dispatch.
        if !self.closed_curve_crosses_boundary(face, c1, tol) {
            self.synthesize_lateral_seam(face)?;
        }
        let rep1 = self.imprint_closed_curve_crossing(face, c1, tol)?;
        let mut out_edges: Vec<EdgeKey> = vec![rep1.edge];
        // Curve 1's two antipodal OPEN arcs (carrier matches c1):
        // record their halves HERE (arc_sweep = pi from each start
        // vertex), local to this gated path.
        let mut arcs: Vec<EdgeKey> = self
            .edges
            .iter()
            .filter(|(_, e)| {
                e.bounds.0 != e.bounds.1
                    && e.curve
                        .and_then(|(ck, _)| self.curves.get(ck))
                        .map(|cv| same_conic(cv, c1))
                        .unwrap_or(false)
            })
            .map(|(k, _)| k)
            .collect();
        let arcs_snapshot = arcs.clone();
        for arc in arcs_snapshot {
            self.set_edge_arc_sweep(arc, core::f64::consts::PI);
        }
        // 2. Split the containing arc at each crossing (the recorded
        //    sweeps make containment decidable).
        let mut xverts = Vec::new();
        for &x in xs {
            // The crossing may COINCIDE with an existing arc endpoint
            // (the Steinmetz seam placement lands the wrap split exactly
            // on the mutual crossings): reuse that vertex, no split.
            let existing = arcs.iter().find_map(|&e| {
                let ed = self.edges.get(e)?;
                for v in [ed.bounds.0, ed.bounds.1] {
                    if let Some(vp) = self.vertices.get(v)
                        && (vp.point - x).norm() < 1e-9
                    {
                        return Some(v);
                    }
                }
                None
            });
            if let Some(v) = existing {
                xverts.push(v);
                continue;
            }
            let tx =
                ang_on(c1, x).ok_or(TopoError::Precondition("crossing pair: non-conic curve 1"))?;
            let arc_data = |e: EdgeKey| -> Option<(f64, f64)> {
                let ed = self.edges.get(e)?;
                let p0 = self.vertices.get(ed.bounds.0)?.point;
                Some((ang_on(c1, p0)?, ed.arc_sweep?))
            };
            let host = arcs
                .iter()
                .copied()
                .find(|&e| {
                    arc_data(e)
                        .map(|(t0, sw)| {
                            let rel = (tx - t0).rem_euclid(tau);
                            rel > 1e-9 && rel < sw - 1e-9
                        })
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    if std::env::var("KEEL_STEINMETZ_DEBUG").is_ok() {
                        eprintln!("  crossing tx {tx} x {x:?}");
                        for &e in &arcs {
                            eprintln!("    arc {e:?} data {:?}", arc_data(e));
                        }
                    }
                    TopoError::Precondition("crossing pair: crossing not interior to any arc")
                })?;
            let (t0, sw) =
                arc_data(host).ok_or(TopoError::Precondition("crossing pair: arc data"))?;
            let rel = (tx - t0).rem_euclid(tau);
            let split = self.split_edge(host, x)?;
            self.set_edge_arc_sweep(split.edge_a, rel);
            self.set_edge_arc_sweep(split.edge_b, sw - rel);
            arcs.retain(|&e| e != host);
            arcs.push(split.edge_a);
            arcs.push(split.edge_b);
            out_edges.push(split.edge_a);
            out_edges.push(split.edge_b);
            xverts.push(split.vertex);
        }
        // 3. Curve 2 as two exact rational arcs between the crossings.
        let ta =
            ang_on(c2, xs[0]).ok_or(TopoError::Precondition("crossing pair: non-conic curve 2"))?;
        let tb =
            ang_on(c2, xs[1]).ok_or(TopoError::Precondition("crossing pair: non-conic curve 2"))?;
        let mk_arc =
            |start: f64, sweep: f64| -> Result<keel_geom::nurbs_curve::NurbsCurve, TopoError> {
                let bad = |_| TopoError::Precondition("crossing pair: arc construction");
                match c2 {
                    Curve3::Circle(ci) => {
                        let xr = ci.x_axis * start.cos() + ci.y_axis * start.sin();
                        let yr = ci.x_axis * (-start.sin()) + ci.y_axis * start.cos();
                        keel_geom::nurbs_curve::NurbsCurve::circular_arc(
                            ci.center, xr, yr, ci.radius, sweep,
                        )
                        .map_err(bad)
                    }
                    Curve3::Ellipse(el) => keel_geom::nurbs_curve::NurbsCurve::elliptic_arc(
                        el.center, el.x_axis, el.y_axis, el.a, el.b, start, sweep,
                    )
                    .map_err(bad),
                    _ => Err(TopoError::Precondition("crossing pair: non-conic curve 2")),
                }
            };
        let s_ab = (tb - ta).rem_euclid(tau);
        let mut c2_edges: Vec<(EdgeKey, f64)> = Vec::new();
        for (start, sweep, va, vb) in [
            (ta, s_ab, xverts[0], xverts[1]),
            (tb, tau - s_ab, xverts[1], xverts[0]),
        ] {
            if sweep < 1e-9 {
                return Err(TopoError::Precondition("crossing pair: degenerate arc"));
            }
            let arc = mk_arc(start, sweep)?;
            let (d0, d1) = arc.domain();
            let mid = arc.point(0.5 * (d0 + d1));
            let host = crate::boolean::curved_face_containing(self, mid, tol.max(1e-7))
                .ok_or(TopoError::Precondition("crossing pair: unlocated arc host"))?;
            let e = self.imprint_open_arc_between(host, &Curve3::Nurbs(arc), va, vb, tol)?;
            // The edge's CARRIER is the conic itself with its recorded
            // sweep (the bounded NURBS arc served the pcurve fit): the
            // Green boundary integral and the wireframe samplers read
            // conic + arc_sweep exactly, where an opaque NURBS chords.
            let ckey = self.add_curve(c2.clone());
            if let Some(ed) = self.edges.get_mut(e) {
                ed.curve = Some((ckey, true));
            }
            self.set_edge_arc_sweep(e, sweep);
            out_edges.push(e);
            c2_edges.push((e, start + 0.5 * sweep));
        }
        // Step 5 (e1-half consistency), AFTER both splits: each e1 edge
        // belongs to exactly ONE bowtie (the face whose loop also holds
        // a c2 arc); its half must lie on the SAME azimuth side of the
        // crossings as that c2 arc's midpoint. The uniform +pi
        // declarations from the wrap split are wrong for one bowtie in
        // the DEGENERATE case (the wrap split landing exactly on the
        // crossings), and assignment is only decidable from the final
        // loop structure.
        if let Some(keel_geom::surface::Surface3::Cylinder(cyl)) = self.face_surface3(face) {
            let az = |p: Vec3| -> f64 {
                let w = p - cyl.frame.origin;
                let w = w - cyl.frame.z * w.dot(cyl.frame.z);
                w.dot(cyl.frame.y).atan2(w.dot(cyl.frame.x))
            };
            let in_ccw = |a1: f64, a2: f64, x: f64| -> bool {
                (x - a1).rem_euclid(tau) < (a2 - a1).rem_euclid(tau)
            };
            let (a1, a2) = (az(xs[0]), az(xs[1]));
            for &(c2e, c2_mid_t) in &c2_edges {
                let mid3 = match c2 {
                    Curve3::Circle(ci) => ci.point(c2_mid_t),
                    Curve3::Ellipse(el) => el.point(c2_mid_t),
                    _ => continue,
                };
                let want = in_ccw(a1, a2, az(mid3));
                // The BOWTIE loop(s) holding this c2 edge: loops whose
                // edges are exclusively conic carriers (c1/c2). The
                // band-rest loop also holds the c2 edge but carries
                // rim/seam-line edges, excluding it: its e1 edge
                // belongs to the OTHER bowtie.
                for rf in self
                    .edges
                    .get(c2e)
                    .map(|e| e.radial.clone())
                    .unwrap_or_default()
                {
                    let Some(lk) = self.fins.get(rf).map(|f| f.owner) else {
                        continue;
                    };
                    let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                        continue;
                    };
                    // Pass 1: pure-conic loop test.
                    let mut conic_only = true;
                    let mut cur2 = entry;
                    while let Some(fin) = self.fins.get(cur2) {
                        let on_conic = self
                            .edges
                            .get(fin.edge)
                            .and_then(|e| e.curve)
                            .and_then(|(ck, _)| self.curves.get(ck))
                            .map(|cv| same_conic(cv, c1) || same_conic(cv, c2))
                            .unwrap_or(false);
                        if !on_conic {
                            conic_only = false;
                            break;
                        }
                        cur2 = fin.next;
                        if cur2 == entry {
                            break;
                        }
                    }
                    if std::env::var("KEEL_STEINMETZ_DEBUG").is_ok() {
                        eprintln!(
                            "  step5: c2e {c2e:?} loop {lk:?} conic_only {conic_only} want {want}"
                        );
                    }
                    if !conic_only {
                        continue;
                    }
                    // Pass 2: fix this bowtie loop's e1 edges to `want`
                    // (collect first: the walk borrows immutably).
                    let mut fixes: Vec<(EdgeKey, f64)> = Vec::new();
                    let mut cur2 = entry;
                    while let Some(fin) = self.fins.get(cur2) {
                        let ek = fin.edge;
                        let is_e1 = self
                            .edges
                            .get(ek)
                            .and_then(|e| e.curve)
                            .and_then(|(ck, _)| self.curves.get(ck))
                            .map(|cv| same_conic(cv, c1))
                            .unwrap_or(false);
                        if is_e1
                            && let Some(ed) = self.edges.get(ek)
                            && let Some(s_cur) = ed.arc_sweep
                            && let Some(p0) = self.vertices.get(ed.bounds.0).map(|v| v.point)
                            && let Some(t0) = ang_on(c1, p0)
                        {
                            let mid_of = |sw: f64| -> Vec3 {
                                let t = t0 + sw * 0.5;
                                match c1 {
                                    Curve3::Circle(ci) => ci.point(t),
                                    Curve3::Ellipse(el) => el.point(t),
                                    _ => p0,
                                }
                            };
                            if std::env::var("KEEL_STEINMETZ_DEBUG").is_ok() {
                                let alt = -(tau - s_cur.abs()) * s_cur.signum();
                                eprintln!(
                                    "  step5: e1 {ek:?} t0 {t0:.3} s {s_cur:.3} cur {} alt {} want {want}",
                                    in_ccw(a1, a2, az(mid_of(s_cur))),
                                    in_ccw(a1, a2, az(mid_of(alt))),
                                );
                            }
                            if in_ccw(a1, a2, az(mid_of(s_cur))) != want {
                                let alt = -(tau - s_cur.abs()) * s_cur.signum();
                                if in_ccw(a1, a2, az(mid_of(alt))) == want {
                                    fixes.push((ek, alt));
                                }
                            }
                        }
                        cur2 = fin.next;
                        if cur2 == entry {
                            break;
                        }
                    }
                    for (ek, alt) in fixes {
                        self.set_edge_arc_sweep(ek, alt);
                    }
                }
            }
        }
        self.debug_validate();
        Ok(out_edges)
    }

    /// Imprint an OPEN on-surface curve between two EXISTING boundary
    /// vertices of `face` (both already on its outer loop): split the
    /// face between the fins ending at the two vertices, attach the
    /// curve and pcurves. The between-vertices variant of
    /// `imprint_open_curve` (which splits boundary EDGES at the
    /// endpoints instead).
    pub(crate) fn imprint_open_arc_between(
        &mut self,
        face: FaceKey,
        curve: &Curve3,
        va: crate::entity::VertexKey,
        vb: crate::entity::VertexKey,
        tol: f64,
    ) -> Result<crate::entity::EdgeKey, TopoError> {
        let surf = self.face_analytic_surface(face)?;
        let (pcurve, _) = self.curve_pcurve_on(face, curve, &surf, tol)?;
        // The loop holding both vertices.
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .ok_or(TopoError::StaleKey)?;
        let lp = loops
            .into_iter()
            .find(|&lk| {
                self.fin_ending_at_vertex(lk, va).is_ok()
                    && self.fin_ending_at_vertex(lk, vb).is_ok()
            })
            .ok_or(TopoError::Precondition(
                "open arc: vertices not on one loop of the face",
            ))?;
        let fin_a = self.fin_ending_at_vertex(lp, va)?;
        let fin_b = self.fin_ending_at_vertex(lp, vb)?;
        let split = self.split_face(fin_a, fin_b, None)?;
        let new_edge = split.edge;
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
        Ok(new_edge)
    }

    /// All fins of loop `lp` whose end vertex is `v` (in loop order).
    fn loop_fins_ending_at_vertex(
        &self,
        lp: crate::entity::LoopKey,
        v: crate::entity::VertexKey,
    ) -> Vec<FinKey> {
        let mut out = Vec::new();
        let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
            return out;
        };
        let mut cur = entry;
        loop {
            if self.fin_end_vertex(cur) == Some(v) {
                out.push(cur);
            }
            let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                break;
            };
            cur = next;
            if cur == entry {
                break;
            }
        }
        out
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
                    let ecurve = e.curve.and_then(|(ck, _)| self.curves.get(ck)).cloned();
                    let (a, b) = (
                        self.vertices.get(e.bounds.0).map(|v| v.point),
                        self.vertices.get(e.bounds.1).map(|v| v.point),
                    );
                    if matches!(ecurve, Some(Curve3::Line(_)) | None)
                        && let (Some(a), Some(b)) = (a, b)
                    {
                        let denom = (b - a).dot(n);
                        if denom.abs() > 1e-12 {
                            let t = (pt - a).dot(n) / denom;
                            if t > 1e-6 && t < 1.0 - 1e-6 {
                                let cross = a + (b - a) * t;
                                return Ok((ek, cross));
                            }
                        }
                    }
                    // CIRCLE seam edge (a sphere's meridian): the cutting
                    // plane meets the carrier circle where
                    // A cos(t) + B sin(t) = C; the solution strictly
                    // interior to THIS arc (its angular span unwrapped
                    // from fin samples) is the crossing.
                    if let Some(Curve3::Circle(ci)) = ecurve {
                        let ca = ci.x_axis.dot(n) * ci.radius;
                        let cb = ci.y_axis.dot(n) * ci.radius;
                        let cc = (pt - ci.center).dot(n);
                        let rr = ca.hypot(cb);
                        if rr > 1e-12 && cc.abs() < rr {
                            let tau = core::f64::consts::TAU;
                            let phi = cb.atan2(ca);
                            let dc = (cc / rr).clamp(-1.0, 1.0).acos();
                            let ang = |p: Vec3| {
                                let w = p - ci.center;
                                w.dot(ci.y_axis).atan2(w.dot(ci.x_axis))
                            };
                            let samples = self.fin_curve_samples(cur, 16).unwrap_or_default();
                            if samples.len() >= 2 {
                                let mut th: Vec<f64> = samples.iter().map(|p| ang(*p)).collect();
                                for i in 1..th.len() {
                                    while th[i] - th[i - 1] > core::f64::consts::PI {
                                        th[i] -= tau;
                                    }
                                    while th[i - 1] - th[i] > core::f64::consts::PI {
                                        th[i] += tau;
                                    }
                                }
                                let lo = th.iter().cloned().fold(f64::INFINITY, f64::min);
                                let hi = th.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                                for cand in [phi + dc, phi - dc] {
                                    let mut t = cand;
                                    while t < lo {
                                        t += tau;
                                    }
                                    while t > hi {
                                        t -= tau;
                                    }
                                    if t > lo + 1e-6 && t < hi - 1e-6 {
                                        return Ok((ek, ci.point(t)));
                                    }
                                }
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

    /// Verify `curve` lies on `face`'s surface (analytic OR NURBS) within
    /// tol, compute its pcurve, and return (pcurve, a seam point on the
    /// curve). The NURBS path projects samples via `project_point_surface`
    /// and fits a pcurve through the (u, v) parameters (M7b).
    pub(crate) fn curve_pcurve_on_any(
        &self,
        face: FaceKey,
        curve: &Curve3,
        tol: f64,
    ) -> Result<(keel_geom::nurbs_curve::NurbsCurve, Vec3), TopoError> {
        let (sk, _) = self
            .faces
            .get(face)
            .and_then(|f| f.surface)
            .ok_or(TopoError::Precondition("imprint: face has no surface"))?;
        match self.surfaces.get(sk) {
            Some(SurfaceGeom::Analytic(a)) => {
                let a = a.clone();
                self.curve_pcurve_on(face, curve, &a, tol)
            }
            Some(SurfaceGeom::Nurbs(n)) => {
                let n = n.clone();
                let sample = |t: f64| -> Vec3 {
                    match curve {
                        Curve3::Line(l) => l.point(t),
                        Curve3::Circle(c) => c.point(core::f64::consts::TAU * t),
                        Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
                        Curve3::Nurbs(nc) => {
                            let (a, b) = nc.domain();
                            nc.point(a + t * (b - a))
                        }
                    }
                };
                let ptol = tol.max(1e-7);
                let ((u0, u1), _) = n.domain();
                let uperiod = u1 - u0;
                // Project a modest number of samples (project_point_surface
                // is heavy: it re-decomposes Bezier patches each call), and
                // UNWRAP the periodic u so the fit sees a continuous curve
                // instead of a 2pi seam jump (which makes fit_cubic
                // escalate control points without bound).
                const N: usize = 24;
                let mut uvpts: Vec<Vec3> = Vec::new();
                let mut prev_u: Option<f64> = None;
                for k in 0..=N {
                    let p = sample(k as f64 / N as f64);
                    // Fast local inversion: the SSI curve is on the
                    // surface, so a grid-seeded Newton suffices (the
                    // certified global projector is ~100x slower).
                    let pr = keel_geom::project::project_point_surface_fast(&n, p);
                    if pr.distance > ptol * 100.0 {
                        return Err(TopoError::Precondition(
                            "nurbs imprint: curve not on face surface",
                        ));
                    }
                    let mut u = pr.u;
                    if let Some(pu) = prev_u
                        && uperiod > 0.0
                    {
                        while u - pu > 0.5 * uperiod {
                            u -= uperiod;
                        }
                        while pu - u > 0.5 * uperiod {
                            u += uperiod;
                        }
                    }
                    prev_u = Some(u);
                    uvpts.push(Vec3::new(u, pr.v, 0.0));
                }
                let fit = keel_geom::fit::fit_cubic(&uvpts, ptol.max(1e-4))
                    .map_err(|_| TopoError::Precondition("nurbs imprint: pcurve fit failed"))?;
                Ok((fit.curve, sample(0.0)))
            }
            None => Err(TopoError::StaleKey),
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
        // EXACT analytic pcurves (OPT-M3/M4): the general fit below
        // escalates a cubic against corners (polylines) or constant
        // curvature (circles) at 1e-7, costing tens of milliseconds
        // per imprint for results these closed forms give exactly.
        //
        // Circle on its own PLANE: the UV image is the circle through
        // the projected center with the projected in-plane axes; the
        // exact rational-quadratic NURBS circle represents it at
        // machine precision (the 12-sample on-surface check above has
        // already certified coplanarity).
        if let (Curve3::Circle(ci), keel_geom::surface::Surface3::Plane(pl)) = (curve, surf) {
            let o = pl.frame.origin;
            let uv = |p: Vec3| {
                let w = p - o;
                Vec3::new(w.dot(pl.frame.x), w.dot(pl.frame.y), 0.0)
            };
            let xa = Vec3::new(ci.x_axis.dot(pl.frame.x), ci.x_axis.dot(pl.frame.y), 0.0);
            let ya = Vec3::new(ci.y_axis.dot(pl.frame.x), ci.y_axis.dot(pl.frame.y), 0.0);
            if let Ok(puv) = keel_geom::nurbs_curve::NurbsCurve::circular_arc(
                uv(ci.center),
                xa,
                ya,
                ci.radius,
                core::f64::consts::TAU,
            ) {
                return Ok((puv, sample(0.0)));
            }
        }
        // COAXIAL circle on a CYLINDER lateral: the UV image is the
        // straight line v = height swept once around theta; exact as a
        // degree-1 segment (orientation from the circle normal against
        // the cylinder axis).
        // (Cones share the parameterization: u = angle, v = height, so a
        // coaxial rim circle is the same straight pcurve.)
        if let (Curve3::Circle(ci), Some(z), Some(o)) = (
            curve,
            match surf {
                keel_geom::surface::Surface3::Cylinder(cy) => Some(cy.frame.z),
                keel_geom::surface::Surface3::Cone(co) => Some(co.frame.z),
                _ => None,
            },
            match surf {
                keel_geom::surface::Surface3::Cylinder(cy) => Some(cy.frame.origin),
                keel_geom::surface::Surface3::Cone(co) => Some(co.frame.origin),
                _ => None,
            },
        ) {
            let d = ci.center - o;
            let coaxial = ci.x_axis.cross(ci.y_axis).cross(z).norm() < 1e-9
                && (d - z * d.dot(z)).norm() < 1e-9;
            if coaxial && let Ok(pr0) = surf.project(sample(0.0)) {
                let h = pr0.v;
                let s = if ci.x_axis.cross(ci.y_axis).dot(z) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                if let Ok(puv) = keel_geom::nurbs_curve::NurbsCurve::new(
                    1,
                    vec![0., 0., 1., 1.],
                    vec![
                        Vec3::new(pr0.u, h, 0.0),
                        Vec3::new(pr0.u + s * core::f64::consts::TAU, h, 0.0),
                    ],
                    None,
                ) {
                    return Ok((puv, sample(0.0)));
                }
            }
        }
        // Degree-1 polyline on a PLANE (OPT-M3): maps control-point-
        // for-control-point under the affine UV projection, with the
        // SAME knot vector: exact.
        if let (Curve3::Nurbs(n), keel_geom::surface::Surface3::Plane(pl)) = (curve, surf)
            && n.degree() == 1
            && !n.is_rational()
        {
            let o = pl.frame.origin;
            let uv_pts: Vec<Vec3> = n
                .control_points()
                .iter()
                .map(|p| {
                    let w = *p - o;
                    Vec3::new(w.dot(pl.frame.x), w.dot(pl.frame.y), 0.0)
                })
                .collect();
            if let Ok(puv) = keel_geom::nurbs_curve::NurbsCurve::new(
                1,
                n.knot_vector().knots().to_vec(),
                uv_pts,
                None,
            ) {
                return Ok((puv, sample(0.0)));
            }
        }
        let fit = keel_geom::fit::pcurve_on_analytic(curve, surf, 64, tol.max(1e-7))
            .map_err(|_| TopoError::Precondition("imprint: pcurve fit failed"))?;
        Ok((fit.curve, sample(0.0)))
    }

    pub(crate) fn fin_ending_at_vertex(
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
/// Mutual intersection points of two PLANAR closed curves lying on one
/// cylinder carrier (the crossing-pair arrangement, task 29): the two
/// planes meet in a LINE, and the line meets the cylinder where an
/// exact QUADRATIC vanishes; points are verified on both curves. Empty
/// for parallel/coplanar planes or a missing real root.
pub(crate) fn planar_curve_crossings(
    c1: &Curve3,
    c2: &Curve3,
    cyl: &keel_geom::surface::Cylinder3,
) -> Vec<Vec3> {
    let Some((p1, n1)) = closed_curve_plane(c1) else {
        return Vec::new();
    };
    let Some((p2, n2)) = closed_curve_plane(c2) else {
        return Vec::new();
    };
    let dir = n1.cross(n2);
    let Some(d) = dir.try_normalize() else {
        return Vec::new();
    };
    // A point on the plane-plane line (the 2x2 normal-plane solve).
    let (da, db) = (n1.dot(p1), n2.dot(p2));
    let (naa, nab, nbb) = (n1.dot(n1), n1.dot(n2), n2.dot(n2));
    let det = naa * nbb - nab * nab;
    if det.abs() < 1e-300 {
        return Vec::new();
    }
    let o = n1 * ((da * nbb - db * nab) / det) + n2 * ((db * naa - da * nab) / det);
    // Line (o + t d) on the cylinder: |perp-axis component|^2 = r^2.
    let z = cyl.frame.z;
    let w = o - cyl.frame.origin;
    let wp = w - z * w.dot(z);
    let dp = d - z * d.dot(z);
    let (qa, qb, qc) = (
        dp.dot(dp),
        2.0 * wp.dot(dp),
        wp.dot(wp) - cyl.radius * cyl.radius,
    );
    let mut out = Vec::new();
    if qa.abs() < 1e-300 {
        return out;
    }
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return out;
    }
    let s = disc.sqrt();
    for t in [(-qb - s) / (2.0 * qa), (-qb + s) / (2.0 * qa)] {
        let p = o + d * t;
        if out.iter().all(|q: &Vec3| (*q - p).norm() > 1e-9) {
            out.push(p);
        }
    }
    out
}

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
    fn imprint_circle_on_nurbs_sphere() {
        // The NURBS imprint path (M7b Task 1): a latitude circle on a
        // NURBS sphere splits its single face into a cap + the rest, with
        // a pcurve fitted in NURBS parameter space.
        use keel_geom::curve::Circle3;
        let mut b = Body::new();
        // Poles on x (frame.z=(1,0,0)) so the seam is in the world z=0
        // plane -- the latitude circle at z=0.5 stays crossing-free.
        let frame = Frame3 {
            origin: Vec3::ZERO,
            x: Vec3::new(0., 1., 0.),
            y: Vec3::new(0., 0., 1.),
            z: Vec3::new(1., 0., 0.),
        };
        let out = b.nurbs_sphere(frame, 1.0).unwrap();
        let face = out.faces[0];
        let r = 0.75f64.sqrt();
        let circle = Curve3::Circle(
            Circle3::new(
                Vec3::new(0., 0., 0.5),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                r,
            )
            .unwrap(),
        );
        let rep = b.imprint_closed_curve(face, &circle, 1e-6).unwrap();
        assert!(b.validate().is_ok(), "nurbs imprint invalid");
        assert_eq!(b.counts().f, 2, "nurbs sphere splits into cap + rest");
        let radial = b.edge(rep.edge).map(|e| e.radial.clone()).unwrap();
        assert_eq!(radial.len(), 2);
        for fk in radial {
            assert!(b.fin(fk).and_then(|f| f.pcurve).is_some());
        }
    }

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
        // Two new vertices (seam crossing P + antipode Q), the slice
        // circle as two arcs, and one new face (the band split).
        assert_eq!(after.v, before.v + 2);
        assert_eq!(after.f, before.f + 1);
        // Each arc is shared by the two bands (radial 2) with pcurves.
        let radial = b.edge(rep.edge).map(|e| e.radial.clone()).unwrap();
        assert_eq!(radial.len(), 2, "arc should be shared by both bands");
        for fk in radial {
            assert!(b.fin(fk).and_then(|f| f.pcurve).is_some());
        }
        // The two lateral fragments are the bands [0,2.5] and [2.5,5]:
        // each interior point sits at its own mid-height.
        let lat_faces: Vec<_> = b
            .face_keys()
            .into_iter()
            .filter(|&fk| matches!(b.face_surface3(fk), Some(Surface3::Cylinder(_))))
            .collect();
        assert_eq!(lat_faces.len(), 2, "lateral split into two band faces");
        let mut zs: Vec<f64> = lat_faces
            .iter()
            .filter_map(|&fk| b.face_interior_point(fk))
            .map(|p| p.z)
            .collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(zs.len(), 2, "both bands have interior points");
        assert!(
            (zs[0] - 1.25).abs() < 1e-6 && (zs[1] - 3.75).abs() < 1e-6,
            "band mid-heights {zs:?}"
        );
    }

    #[test]
    fn imprint_crossing_circle_on_cone_lateral() {
        // The cone analog of the cylinder slice: a coaxial circle at
        // mid-height wraps the frustum lateral. Synthesize the seam if
        // the face is a two-rim tube, then crossing-imprint.
        let mut b = Body::new();
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        b.loft_circles(frame, 0.5, 2.5, 2.0).unwrap();
        let lateral = b
            .face_keys()
            .into_iter()
            .find(|&fk| matches!(b.face_surface3(fk), Some(Surface3::Cone(_))))
            .expect("frustum lateral");
        // r(1) = 0.5 + 1*tan(atan(1)) = 1.5.
        let slice = Curve3::Circle(
            Circle3::new(
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                1.5,
            )
            .unwrap(),
        );
        if !b.closed_curve_crosses_boundary(lateral, &slice, 1e-7) {
            b.synthesize_lateral_seam(lateral).unwrap();
        }
        assert!(
            b.closed_curve_crosses_boundary(lateral, &slice, 1e-7),
            "slice must cross the (synthesized) seam"
        );
        let rep = b
            .imprint_closed_curve_crossing(lateral, &slice, 1e-7)
            .unwrap();
        assert!(b.validate().is_ok(), "cone crossing imprint invalid");
        let radial = b.edge(rep.edge).map(|e| e.radial.clone()).unwrap();
        assert_eq!(radial.len(), 2, "arc shared by both bands");
        let cones: Vec<_> = b
            .face_keys()
            .into_iter()
            .filter(|&fk| matches!(b.face_surface3(fk), Some(Surface3::Cone(_))))
            .collect();
        assert_eq!(cones.len(), 2, "lateral split into two band faces");
        // Classification depends on an interior point for every curved
        // face: both bands must produce one, at the right height.
        let mut zs: Vec<f64> = cones
            .iter()
            .map(|&fk| {
                b.face_interior_point(fk)
                    .expect("cone band interior point")
                    .z
            })
            .collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            zs[0] > 0.0 && zs[0] < 1.0 && zs[1] > 1.0 && zs[1] < 2.0,
            "band interior heights {zs:?}"
        );
    }

    #[test]
    fn imprint_latitude_ring_crossing_sphere_seam() {
        // The socket-carve shape: a latitude circle on a seamed sphere
        // (poles along z) CROSSES the meridian seam arc, so the crossing
        // imprint must split the seam at the crossing and the face into
        // cap + bowl, keeping total mass == mesh == the sphere's.
        let mut b = Body::new();
        let frame = Frame3::from_z(Vec3::new(2.0, 2.0, 1.5), Vec3::new(0., 0., 1.)).unwrap();
        b.sphere(frame, 1.0).unwrap();
        let face = b
            .face_keys()
            .into_iter()
            .find(|&fk| matches!(b.face_surface3(fk), Some(Surface3::Sphere(_))))
            .unwrap();
        // Rim at z = 2: radius sqrt(1 - 0.25).
        let circle = Curve3::Circle(
            Circle3::new(
                Vec3::new(2.0, 2.0, 2.0),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                0.75f64.sqrt(),
            )
            .unwrap(),
        );
        assert!(
            b.closed_curve_crosses_boundary(face, &circle, 1e-7),
            "latitude must register as crossing the meridian seam"
        );
        b.imprint_closed_curve_crossing(face, &circle, 1e-7)
            .unwrap();
        assert!(b.validate().is_ok(), "crossing ring imprint invalid");
        // (Mass integration of split sphere pieces is the recorded
        // follow-up: their pcurve boxes are degenerate and the Green
        // path is side-ambiguous on full-period wraps. Topology and
        // classification inputs are what this slice certifies.)
        // Classification ground truth: each piece's interior point must
        // be on ITS side of the rim plane z = 2 (the cap's at the north
        // pole, the bowl's at the south pole).
        let mut zs: Vec<f64> = b
            .face_keys()
            .into_iter()
            .map(|fk| {
                b.face_interior_point(fk)
                    .expect("split sphere piece interior point")
                    .z
            })
            .collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            zs[0] < 2.0 && zs[1] > 2.0,
            "piece interior points straddle the rim: {zs:?}"
        );
    }

    #[test]
    fn boolean_imprinted_ball_pieces_interior_points() {
        // The socket scenario's ACTUAL imprinted tool operand: the same
        // split must yield straddling interior points there too.
        let mut block = Body::new();
        block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
        let mut ball = Body::new();
        ball.sphere(
            Frame3::from_z(Vec3::new(2.0, 2.0, 1.5), Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
        )
        .unwrap();
        let (_, ib, faults) = crate::boolean::imprint_pair(&block, &ball, 1e-7);
        assert!(faults.is_empty(), "{faults:?}");
        let mut zs: Vec<f64> = ib
            .body
            .face_keys()
            .into_iter()
            .filter(|&fk| matches!(ib.body.face_surface3(fk), Some(Surface3::Sphere(_))))
            .map(|fk| {
                ib.body
                    .face_interior_point(fk)
                    .expect("ball piece interior point")
                    .z
            })
            .collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(zs.len(), 2);
        assert!(
            zs[0] < 2.0 && zs[1] > 2.0,
            "ball piece interior points straddle the rim: {zs:?}"
        );
    }

    #[test]
    fn imprint_latitude_ring_reversed_winding_same_answer() {
        // The same split with the rim circle wound the OTHER way (the
        // SSI circle's axes come from the cutting plane's frame, whose
        // orientation is arbitrary): interior points must not flip.
        let mut b = Body::new();
        let frame = Frame3::from_z(Vec3::new(2.0, 2.0, 1.5), Vec3::new(0., 0., 1.)).unwrap();
        b.sphere(frame, 1.0).unwrap();
        let face = b
            .face_keys()
            .into_iter()
            .find(|&fk| matches!(b.face_surface3(fk), Some(Surface3::Sphere(_))))
            .unwrap();
        let circle = Curve3::Circle(
            Circle3::new(
                Vec3::new(2.0, 2.0, 2.0),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., -1., 0.),
                0.75f64.sqrt(),
            )
            .unwrap(),
        );
        if !b.closed_curve_crosses_boundary(face, &circle, 1e-7) {
            b.synthesize_lateral_seam(face).unwrap();
        }
        b.imprint_closed_curve_crossing(face, &circle, 1e-7)
            .unwrap();
        let mut zs: Vec<f64> = b
            .face_keys()
            .into_iter()
            .map(|fk| {
                b.face_interior_point(fk)
                    .expect("split sphere piece interior point")
                    .z
            })
            .collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            zs[0] < 2.0 && zs[1] > 2.0,
            "piece interior points straddle the rim: {zs:?}"
        );
    }

    #[test]
    #[ignore = "task 29 development harness: needs the crossing-pair seam gate OPEN (see the both_cyl arm in seam_curves) and the metric layer; asserts piece areas/interiors for the Steinmetz operand"]
    fn crossing_pair_pieces_have_area_and_interior_points() {
        // The Steinmetz operand after the crossing-pair imprint: four
        // lateral pieces, every one with positive area and an interior
        // point (classification's inputs).
        let mut a = Body::new();
        a.cylinder(
            Frame3::from_z(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
            1.0,
            4.0,
        )
        .unwrap();
        let mut b = Body::new();
        b.cylinder(
            Frame3::from_z(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)).unwrap(),
            1.0,
            4.0,
        )
        .unwrap();
        let (ia, _, faults) = crate::boolean::imprint_pair(&a, &b, 1e-7);
        assert!(faults.is_empty(), "{faults:?}");
        let laterals: Vec<_> = ia
            .body
            .face_keys()
            .into_iter()
            .filter(|&fk| {
                matches!(
                    ia.body.face_surface3(fk),
                    Some(keel_geom::surface::Surface3::Cylinder(_))
                )
            })
            .collect();
        assert_eq!(laterals.len(), 4, "lateral splits into four pieces");
        for fk in laterals {
            let area = ia.body.face_area(fk);
            eprintln!("piece {fk:?}: tris {}", ia.body.tessellate_face(fk).len());
            eprintln!(
                "piece {fk:?}: area {area} interior {:?} span {:?}",
                ia.body.face_interior_point(fk),
                {
                    let c = match ia.body.face_surface3(fk) {
                        Some(keel_geom::surface::Surface3::Cylinder(c)) => c,
                        _ => unreachable!(),
                    };
                    ia.body
                        .cyl_angular_span(fk, c.frame.origin, c.frame.x, c.frame.y, c.frame.z)
                }
            );
            assert!(area > 0.1, "piece {fk:?} area {area}");
            assert!(ia.body.face_interior_point(fk).is_some());
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
