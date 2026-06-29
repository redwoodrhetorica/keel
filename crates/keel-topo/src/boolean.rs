//! Regularized boolean operations (M6a): union, intersection,
//! difference on solid bodies, for clean transversal configurations.
//!
//! Pipeline (M3 gate section 4): localize -> intersect (SSI, M5a) ->
//! imprint both operands + glue -> classify fragments (PMC, M4) ->
//! select per regularized r-set tables -> stitch + rebuild regions.
//!
//! Scope (M6a, binding): faces meet TRANSVERSALLY. Coplanar/coincident
//! faces, tangencies, and full NURBS-bounded-solid booleans are M6b/M7
//! and surface here as fault tags (partial-success model: a result plus
//! per-pair faults, never all-or-nothing, never a panic).

use crate::body::{Body, TopoError};
use crate::entity::{AnyKey, FaceKey, FinKey, SurfaceGeom};
use keel_geom::ssi::{SsiResult, SurfaceRef, intersect_surfaces};
use keel_geom::surface::Surface3;

/// The three regularized boolean operations on solid bodies.
///
/// Used as the `op` argument to [`boolean`] and friends. Results are
/// regularized (lower-dimensional slivers are discarded) by default.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolOp {
    /// `a` OR `b`: the volume occupied by either operand.
    Union,
    /// `a` AND `b`: the volume occupied by both operands.
    Intersection,
    /// `a` minus `b`: the part of `a` not occupied by `b`.
    Difference,
}

/// A recoverable or fatal fault encountered during a boolean. Faults
/// accumulate; the operation reports partial success rather than
/// aborting the whole result (synthesis A error model).
#[derive(Clone, Debug, PartialEq)]
pub enum BoolFault {
    /// A face pair's surfaces overlap on a region (coplanar/coincident);
    /// deferred to M6b. Carries the two face EntityIds.
    Coincident(u64, u64),
    /// A face pair touches tangentially (SSI returned isolated points);
    /// deferred to M6b.
    Tangent(u64, u64),
    /// SSI failed on a pair (e.g. unsupported surface combination).
    IntersectionFailed(u64, u64),
    /// A face pair GENUINELY CROSSES (SSI curves exist) but the imprint
    /// cannot yet assemble those seams (e.g. the cylinder-cylinder
    /// crossing pair). A HARD decline: proceeding seamless produced an
    /// Euler-valid but geometrically WRONG body.
    UnassemblableSeam(u64, u64),
    /// A topology operation failed during imprint/stitch.
    Topo(TopoError),
    /// The result could not be assembled into a valid body.
    AssemblyFailed(&'static str),
}

/// How much of an SSI curve lies on a trimmed face's extent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CurveFaceOverlap {
    All,
    None,
    Partial,
}

/// One intersection curve localized to the face pair that produced it.
#[derive(Clone, Debug)]
pub struct SeamCurve {
    pub face_a: FaceKey,
    pub face_b: FaceKey,
    pub curve: keel_geom::curve::Curve3,
    pub closed: bool,
    /// Certified bound on the curve's deviation from the true surface-
    /// surface intersection (`SsiCurve::tol_achieved`): ~0 for exact
    /// conics, ~the fit error for marched NURBS. This rides onto the
    /// imprinted edge's tolerance (the tolerant-geometry contract, M7b).
    pub tol: f64,
    /// The seam already lies ON an existing boundary edge of this
    /// operand's face (file 39 sec 3.2): no imprint needed on that side.
    /// One-sided cases are real (the countersink plug's lateral crosses
    /// the sunk block's EXISTING rim): the other side still imprints.
    pub on_boundary_a: bool,
    pub on_boundary_b: bool,
}

impl Body {
    /// All LIVE faces of the body, as transient [`FaceKey`]s in
    /// deterministic [`EntityId`](crate::entity::EntityId) order.
    ///
    /// This is the face counterpart of
    /// [`Body::edge_keys`](crate::Body::edge_keys): it lets a consumer
    /// enumerate the faces of an ARBITRARY body (e.g. a boolean result),
    /// not just the keys a primitive constructor hands back. Each key
    /// resolves through [`Body::face`](crate::Body::face); keys are
    /// transient (valid only for this body value, invalidated by the next
    /// mutation), while the order is stable across identical inputs.
    /// Returns an empty vector for a body with no faces.
    pub fn face_keys(&self) -> Vec<FaceKey> {
        self.entity_ids()
            .filter_map(|id| match self.lookup(id) {
                Some(AnyKey::Face(k)) => Some(k),
                _ => None,
            })
            .collect()
    }

    /// Axial heights (relative to `origin` along `ez`) of every
    /// circle/ellipse-curved edge of `face` -- the cap circles AND the
    /// SSI arcs that bound a cylindrical band. (Arcs are open edges, so
    /// closed-ness is NOT required.)
    pub(crate) fn cyl_circle_heights(
        &self,
        face: FaceKey,
        origin: keel_math::vec::Vec3,
        ez: keel_math::vec::Vec3,
    ) -> Vec<f64> {
        use keel_geom::curve::Curve3;
        let mut heights = Vec::new();
        for lk in self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default()
        {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            while let Some(fin) = self.fins.get(cur) {
                if let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve) {
                    match self.curves.get(ck) {
                        Some(Curve3::Circle(c)) => heights.push((c.center - origin).dot(ez)),
                        Some(Curve3::Ellipse(e)) => heights.push((e.center - origin).dot(ez)),
                        _ => {}
                    }
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        heights
    }

    /// True iff the curve lies ENTIRELY on the face's trimmed extent
    /// (see `curve_cylinder_face_overlap`).
    pub(crate) fn curve_on_cylinder_face(
        &self,
        face: FaceKey,
        curve: &keel_geom::curve::Curve3,
        tol: f64,
    ) -> bool {
        matches!(
            self.curve_cylinder_face_overlap(face, curve, tol),
            CurveFaceOverlap::All
        )
    }

    /// How much of an SSI curve lies on a CYLINDER face's trimmed
    /// extent: `All` (a usable seam), `None` (skip), or `Partial` (the
    /// seam crosses the face boundary; the imprint cannot yet assemble
    /// such a trim and the boolean must DECLINE: the notch probe showed
    /// a full SSI circle imprinted onto a quarter BAND assembling an
    /// invalid-trim body behind the weak curved gate, the same
    /// wrong-positive class as the crossing-cylinder pair). Checks BOTH
    /// the axial band and the ANGULAR span (the original height-only
    /// test passed full circles onto quarter bands).
    /// Overlap of `curve` with a face's TRIMMED extent, surface-type aware.
    /// `curve_cylinder_face_overlap` defaulted a PLANE face to `All`, so a tool
    /// plane's section of the OTHER operand's pre-existing CURVED feature -- a
    /// phantom seam lying on that curved face but OFF this plane's trimmed
    /// polygon -- was never dropped, and it faulted a multi-feature imprint that
    /// otherwise assembles correctly (drill a hole, then cut a pocket elsewhere:
    /// the pocket's side plane meets the bore cylinder in two phantom rulings).
    /// Conservative: a plane returns `None` only when a LINE seam clips to
    /// nothing on the face (clearly off); other curve types stay `All`.
    pub(crate) fn curve_face_overlap(
        &self,
        face: FaceKey,
        curve: &keel_geom::curve::Curve3,
        tol: f64,
    ) -> CurveFaceOverlap {
        match self.face_surface3(face) {
            Some(Surface3::Cylinder(_)) | Some(Surface3::Cone(_)) => {
                self.curve_cylinder_face_overlap(face, curve, tol)
            }
            Some(Surface3::Plane(p)) => {
                use keel_geom::curve::Curve3;
                let mut poly3 = self.face_outer_loop_points(face);
                if poly3.len() < 3 {
                    // A disk/cap face (one circular edge) has too few loop
                    // VERTICES for a winding test, so it used to default to `All`
                    // -- and a multi-cut tool's cap plane then "kept" the section
                    // circle it cuts on a FAR cylinder/sphere (the unbounded
                    // section lies in the cap's plane but outside its finite
                    // disk). That phantom imprinted onto the far curved feature
                    // and DOUBLED its loop (the green-slab then declines mass !=
                    // mesh: the dominant multi-cut residual). Sample the boundary
                    // edges into a polygon so the winding test below rejects a
                    // section lying off the finite disk.
                    let mut pts: Vec<keel_math::vec::Vec3> = Vec::new();
                    for e in self.face_loop_edges(face).unwrap_or_default() {
                        if let Some(ck) = self.edges.get(e).and_then(|x| x.curve).map(|(k, _)| k)
                            && let Some(cv) = self.curves.get(ck)
                        {
                            for s in 0..32 {
                                pts.push(curve_point(cv, s as f64 / 32.0));
                            }
                        }
                    }
                    poly3 = pts;
                }
                if poly3.len() < 3 {
                    return CurveFaceOverlap::All;
                }
                if let Curve3::Line(line) = curve {
                    // A ruling: the tested line-clip is exact.
                    return if clip_line_to_planar_face(line, &p, &poly3).is_some() {
                        CurveFaceOverlap::All
                    } else {
                        CurveFaceOverlap::None
                    };
                }
                // A circle/ellipse/NURBS section of this plane with another
                // operand's CURVED feature (a prior hole's bore, a dome): sample
                // it and keep only if ANY point lands inside the trimmed polygon.
                // All-off => a phantom on the OTHER surface, off this plane's
                // extent => drop. Conservative (32 samples, ANY-on keeps), so a
                // real in-face arc is never dropped.
                let tau = core::f64::consts::TAU;
                let samples: Vec<keel_math::vec::Vec3> = match curve {
                    Curve3::Circle(ci) => {
                        (0..32).map(|k| ci.point(tau * k as f64 / 32.0)).collect()
                    }
                    Curve3::Ellipse(e) => (0..32).map(|k| e.point(tau * k as f64 / 32.0)).collect(),
                    Curve3::Nurbs(n) => {
                        let (t0, t1) = n.domain();
                        (0..=24)
                            .map(|k| n.point(t0 + (t1 - t0) * k as f64 / 24.0))
                            .collect()
                    }
                    Curve3::Line(_) => unreachable!(),
                };
                let fr = &p.frame;
                let poly2: Vec<(f64, f64)> = poly3
                    .iter()
                    .map(|q| {
                        let qw = *q - fr.origin;
                        (qw.dot(fr.x), qw.dot(fr.y))
                    })
                    .collect();
                let any_on = samples.iter().any(|s| {
                    let w = *s - fr.origin;
                    winding_nonzero(&poly2, (w.dot(fr.x), w.dot(fr.y)))
                });
                if any_on {
                    CurveFaceOverlap::All
                } else {
                    CurveFaceOverlap::None
                }
            }
            _ => CurveFaceOverlap::All,
        }
    }

    pub(crate) fn curve_cylinder_face_overlap(
        &self,
        face: FaceKey,
        curve: &keel_geom::curve::Curve3,
        tol: f64,
    ) -> CurveFaceOverlap {
        use keel_geom::curve::Curve3;
        // Cones share the cylinder's trim story exactly (an axial band
        // between rim circles plus an angular span): without this, a
        // phantom SSI circle on the UNBOUNDED cone's far nappe imprints
        // onto the face and breaks shell closure (the countersink probe).
        let frame = match self.face_surface3(face) {
            Some(Surface3::Cylinder(c)) => c.frame,
            Some(Surface3::Cone(c)) => c.frame,
            _ => return CurveFaceOverlap::All,
        };
        let (origin, ez) = (frame.origin, frame.z);
        let (ex, ey) = (frame.x, frame.y);
        // Axial band from the face's circle/arc edges.
        let heights = self.cyl_circle_heights(face, origin, ez);
        if heights.len() < 2 {
            return CurveFaceOverlap::All; // cannot determine; keep prior behavior
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let t = tol.max(1e-7);
        // Angular span of the trimmed face (full lateral -> [0, tau]).
        let (a0, a1) = self.cyl_angular_span(face, origin, ex, ey, ez);
        let tau = core::f64::consts::TAU;
        let full_ring = (a1 - a0) >= tau - 1e-9;
        let ang_tol = 1e-6;
        // Sample the curve and classify each point.
        let samples: Vec<keel_math::vec::Vec3> = match curve {
            Curve3::Circle(ci) => (0..32).map(|k| ci.point(tau * k as f64 / 32.0)).collect(),
            Curve3::Ellipse(e) => (0..32).map(|k| e.point(tau * k as f64 / 32.0)).collect(),
            Curve3::Line(l) => {
                // A ruling: constant angle; clip to the axial band.
                let h0 = (l.origin - origin).dot(ez);
                (0..5)
                    .map(|k| {
                        let h = hlo + (hhi - hlo) * k as f64 / 4.0;
                        l.point(h - h0)
                    })
                    .collect()
            }
            Curve3::Nurbs(n) => {
                let (t0, t1) = n.domain();
                (0..=16)
                    .map(|k| n.point(t0 + (t1 - t0) * k as f64 / 16.0))
                    .collect()
            }
        };
        let mut on = 0usize;
        for p in &samples {
            let d = *p - origin;
            let h = d.dot(ez);
            let h_ok = h >= hlo - t && h <= hhi + t;
            let ang_ok = full_ring || {
                let ang = d.dot(ey).atan2(d.dot(ex));
                let rel = (ang - a0).rem_euclid(tau);
                rel <= (a1 - a0) + ang_tol || rel >= tau - ang_tol
            };
            if h_ok && ang_ok {
                on += 1;
            }
        }
        if on == samples.len() {
            CurveFaceOverlap::All
        } else if on == 0 {
            CurveFaceOverlap::None
        } else {
            CurveFaceOverlap::Partial
        }
    }

    /// Raise a seam edge's tolerance (and its bound vertices') to at
    /// least `tol` -- the SSI error bound rides onto the topology.
    pub(crate) fn set_seam_edge_tolerance(&mut self, edge: crate::entity::EdgeKey, tol: f64) {
        let bounds = self.edges.get(edge).map(|e| e.bounds);
        if let Some(e) = self.edges.get_mut(edge) {
            e.tolerance = e.tolerance.max(tol);
        }
        if let Some((v0, v1)) = bounds {
            for v in [v0, v1] {
                if let Some(vt) = self.vertices.get_mut(v) {
                    vt.tolerance = vt.tolerance.max(tol);
                }
            }
        }
    }

    /// Epsilon-solidity (Qi/Shapiro): a CHECKABLE validity contract --
    /// the body is epsilon-solid iff every edge and vertex tolerance is
    /// within `eps`. The boolean's geometry may be approximate (an
    /// inexact NURBS SSI), but the topology is exact and every entity's
    /// deviation is bounded by its stored tolerance; epsilon-solidity
    /// asserts that bound is no worse than `eps`. This is the formal
    /// handle the literature says NURBS kernels usually lack.
    pub fn epsilon_solid(&self, eps: f64) -> bool {
        let slack = eps + 1e-15;
        self.edges.iter().all(|(_, e)| e.tolerance <= slack)
            && self.vertices.iter().all(|(_, v)| v.tolerance <= slack)
    }

    /// The largest edge/vertex tolerance in the body -- the achieved
    /// epsilon (the body is epsilon-solid at exactly this value).
    pub fn achieved_tolerance(&self) -> f64 {
        let e = self
            .edges
            .iter()
            .map(|(_, e)| e.tolerance)
            .fold(0.0f64, f64::max);
        let v = self
            .vertices
            .iter()
            .map(|(_, v)| v.tolerance)
            .fold(0.0f64, f64::max);
        e.max(v)
    }

    /// The full surface geometry backing a face (analytic OR NURBS).
    pub(crate) fn face_surface_geom(&self, face: FaceKey) -> Option<SurfaceGeom> {
        let (sk, _) = self.faces.get(face).and_then(|f| f.surface)?;
        self.surfaces.get(sk).cloned()
    }

    /// Outward unit normal of a face at its interior sample (the surface
    /// normal, flipped to the face's sense).
    pub(crate) fn face_outward_normal(&self, face: FaceKey) -> Option<keel_math::vec::Vec3> {
        let p = self.face_interior_point(face)?;
        self.face_outward_normal_at(face, p)
    }

    fn face_outward_normal_at(
        &self,
        face: FaceKey,
        p: keel_math::vec::Vec3,
    ) -> Option<keel_math::vec::Vec3> {
        let (sk, sense) = self.faces.get(face).and_then(|f| f.surface)?;
        // `local_geometry().normal` is `su x dv` (a PSEUDOVECTOR), so its sign
        // tracks the parametrization FRAME'S handedness. A mirrored body
        // (Body::mirrored) reflects every frame axis directly, sending the
        // frame LEFT-handed: `su x dv` then flips and points INWARD relative to
        // the kernel's `sense * natural` orientation authority (research file 46;
        // the natural-outward for an analytic surface is the `+frame.z` sense,
        // which mass_properties/tessellation use directly). Fold the frame
        // handedness back in so this normal AGREES with mass/tessellation on a
        // reflected body -- the SAME dossier-72 correction massprops applies via
        // `frame_handedness`. Without it the boolean classifier reads two
        // face-to-face coincident mirror faces as `OnSense::Same` instead of
        // `Opposite`, so the shared interface wall is retained (mass != mesh)
        // and a mirror+union faults. For every non-mirrored body the frame is
        // right-handed and the factor is +1.0 (an exact no-op).
        let (n, hand) = match self.surfaces.get(sk)? {
            SurfaceGeom::Analytic(s) => {
                let pr = s.project(p).ok()?;
                (s.local_geometry(pr.u, pr.v).ok()?.normal, surface_frame_handedness(s))
            }
            SurfaceGeom::Nurbs(nb) => {
                let pr = keel_geom::project::project_point_surface_fast(nb, p);
                // A NURBS surface carries no analytic frame; its control net is
                // mapped homogeneously by mirror/scale, so its `su x dv` already
                // reflects the geometry (no separate handedness fold needed).
                (nb.local_geometry(pr.u, pr.v).ok()?.normal, 1.0)
            }
        };
        let n = n * hand;
        Some(if sense { n } else { n * -1.0 })
    }

    /// Orientation sense (file 39 §2.1) of THIS body's face that is
    /// coincident at `p` with an external face of outward normal
    /// `n_other`: `Same` if the normals agree, `Opposite` if they oppose.
    /// `Unknown` if no coincident carrier is found here.
    pub(crate) fn coincident_sense_at(
        &self,
        p: keel_math::vec::Vec3,
        n_other: keel_math::vec::Vec3,
        tol: f64,
    ) -> OnSense {
        // The carrier-coincidence distance band is HALF the op
        // tolerance: safely between the contact regimes (a 1e-9 mate
        // reads coincident with 50x margin; a gap of exactly tol
        // reads SEPARATE with 2x margin), so the at-tolerance class
        // is deterministic instead of bistable on probe noise. This
        // band must stay consistent with coincident_face_pairs'
        // detection threshold; the old literal 1e-6 here disagreed
        // with the 1e-7 there, and the mismatch produced the
        // unmatched-coedge decline flips (OPT-M2 diagnosis).
        let band = 0.5 * tol;
        for f in self.face_keys() {
            // The candidate's BOUNDED EXTENT must reach p (an inflated
            // AABB guard): without it a distant point lying on the
            // face's infinite carrier would read as coincident, which
            // matters now that classification consults this test
            // geometrically, before any winding band. The box samples
            // boundary CURVES, not just vertices: a circle-bounded
            // lateral carries only its seam vertices, which is no
            // extent at all.
            {
                let mut lo = keel_math::vec::Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
                let mut hi = keel_math::vec::Vec3::new(
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                );
                for lk in self
                    .faces
                    .get(f)
                    .map(|x| x.loops.clone())
                    .unwrap_or_default()
                {
                    let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                        continue;
                    };
                    let mut cur = entry;
                    loop {
                        if let Some(samples) = self.fin_curve_samples(cur, 16) {
                            for q in samples {
                                lo = keel_math::vec::Vec3::new(
                                    lo.x.min(q.x),
                                    lo.y.min(q.y),
                                    lo.z.min(q.z),
                                );
                                hi = keel_math::vec::Vec3::new(
                                    hi.x.max(q.x),
                                    hi.y.max(q.y),
                                    hi.z.max(q.z),
                                );
                            }
                        } else if let Some(v) = self.fin_start_vertex(cur)
                            && let Some(x) = self.vertices.get(v)
                        {
                            let q = x.point;
                            lo = keel_math::vec::Vec3::new(
                                lo.x.min(q.x),
                                lo.y.min(q.y),
                                lo.z.min(q.z),
                            );
                            hi = keel_math::vec::Vec3::new(
                                hi.x.max(q.x),
                                hi.y.max(q.y),
                                hi.z.max(q.z),
                            );
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
                let pad = 1e-6;
                if !lo.x.is_finite()
                    || p.x < lo.x - pad
                    || p.x > hi.x + pad
                    || p.y < lo.y - pad
                    || p.y > hi.y + pad
                    || p.z < lo.z - pad
                    || p.z > hi.z + pad
                {
                    continue;
                }
            }
            // The face's surface must pass through p AND its normal be
            // parallel to n_other (a coincident carrier, not a transversal
            // face that merely contains p).
            let on_surface = match self.face_surface_geom(f) {
                Some(SurfaceGeom::Analytic(s)) => {
                    s.project(p).map(|pr| pr.distance < band).unwrap_or(false)
                }
                Some(SurfaceGeom::Nurbs(nb)) => {
                    let pr = keel_geom::project::project_point_surface_fast(&nb, p);
                    (pr.point - p).norm() < band
                }
                None => false,
            };
            if !on_surface {
                continue;
            }
            // REAL containment for planar carriers: coplanar faces
            // that TILE (a pin's cap disc against the plate's annulus)
            // share a carrier and an AABB but not a point; the disc
            // centre lies in the annulus's RING hole, not on the face.
            // Curved carriers keep the AABB guard (full-lateral mates).
            if matches!(self.face_surface3(f), Some(Surface3::Plane(_)))
                && !self.planar_face_contains(f, p)
            {
                continue;
            }
            if let Some(n_f) = self.face_outward_normal_at(f, p)
                && n_f.cross(n_other).norm() < 1e-6
            {
                return if n_f.dot(n_other) >= 0.0 {
                    OnSense::Same
                } else {
                    OnSense::Opposite
                };
            }
        }
        OnSense::Unknown
    }

    /// Does the PLANAR `face` contain `p` (projected into its plane):
    /// inside the outer loop and outside every inner ring? Loop
    /// polygons come from fin start vertices (straight planar trims).
    pub(crate) fn planar_face_contains(&self, face: FaceKey, p: keel_math::vec::Vec3) -> bool {
        let Some(Surface3::Plane(pl)) = self.face_surface3(face) else {
            return false;
        };
        let fr = &pl.frame;
        let to2 = |q: keel_math::vec::Vec3| {
            let w = q - fr.origin;
            (w.dot(fr.x), w.dot(fr.y))
        };
        let q = to2(p);
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        let mut polys: Vec<Vec<(f64, f64)>> = Vec::new();
        for lk in loops {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut poly = Vec::new();
            let mut cur = entry;
            loop {
                // CURVE samples, not just vertices: a circular ring
                // edge (the drilled hole's rim) carries one seam
                // vertex, which is no polygon at all.
                if let Some(samples) = self.fin_curve_samples(cur, 24) {
                    for p3 in samples {
                        poly.push(to2(p3));
                    }
                } else if let Some(v) = self.fin_start_vertex(cur)
                    && let Some(x) = self.vertices.get(v)
                {
                    poly.push(to2(x.point));
                }
                let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                    break;
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
            if poly.len() >= 3 {
                polys.push(poly);
            }
        }
        let Some(outer) = polys.first() else {
            return false;
        };
        if !winding_nonzero(outer, q) {
            return false;
        }
        !polys[1..].iter().any(|ring| winding_nonzero(ring, q))
    }

    /// The analytic surface backing a face, if any (M6a is analytic).
    pub(crate) fn face_surface3(&self, face: FaceKey) -> Option<Surface3> {
        let (sk, _) = self.faces.get(face).and_then(|f| f.surface)?;
        match self.surfaces.get(sk) {
            Some(SurfaceGeom::Analytic(a)) => Some(a.clone()),
            _ => None,
        }
    }

    /// Ordered 3D vertices of a face's outer loop (fin start vertices in
    /// loop order). Meaningful for faces bounded by a single simple
    /// loop (the planar primitives); periodic faces are not polygonal.
    pub(crate) fn face_outer_loop_points(&self, face: FaceKey) -> Vec<keel_math::vec::Vec3> {
        let mut pts = Vec::new();
        let Some(lp) = self.faces.get(face).and_then(|f| f.loops.first().copied()) else {
            return pts;
        };
        let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
            return pts;
        };
        let mut cur = entry;
        loop {
            if let Some(p) = self
                .fin_start_vertex(cur)
                .and_then(|v| self.vertices.get(v))
                .map(|v| v.point)
            {
                pts.push(p);
            }
            let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                break;
            };
            cur = next;
            if cur == entry {
                break;
            }
        }
        pts
    }

    /// Clip an in-plane `line` to a planar face whose OUTER boundary is a single
    /// CIRCLE edge (a pristine cylinder/cone cap), returning the line-parameter
    /// range `[t0, t1]` where the line lies inside the disk. The polygon clip
    /// (`clip_line_to_planar_face`) needs >= 3 boundary vertices, but a cap's
    /// rim is ONE closed circle edge = ONE vertex, so it returns None and the
    /// cap-plane section seam (e.g. a flat/slot mill's cap chord) is dropped --
    /// leaving the tool face's trim ring open. An ANALYTIC line/circle solve
    /// gives EXACT endpoints (so the chord corners coincide with the wall
    /// rulings' endpoints, and the trim ring closes), where a sampled polygon
    /// would carry a chord-sagitta error that misplaces the corners.
    pub(crate) fn circle_face_line_clip(
        &self,
        face: FaceKey,
        line: &keel_geom::curve::Line3,
    ) -> Option<(f64, f64)> {
        use keel_geom::curve::Curve3;
        let lp = self.faces.get(face)?.loops.first().copied()?;
        let entry = self.loops.get(lp)?.fin?;
        let ek = self.fins.get(entry)?.edge;
        let Curve3::Circle(c) = self
            .edges
            .get(ek)?
            .curve
            .and_then(|(ck, _)| self.curves.get(ck))?
        else {
            return None;
        };
        // The line lies in the cap plane (it is a plane-plane SSI), so the 3D
        // distance to the circle center IS the in-plane distance. Solve
        // |origin + t*dir - center|^2 = r^2 for the two crossings.
        let oc = line.origin - c.center;
        let aa = line.dir.dot(line.dir);
        if aa <= 0.0 {
            return None;
        }
        let bb = 2.0 * oc.dot(line.dir);
        let cc = oc.dot(oc) - c.radius * c.radius;
        let disc = bb * bb - 4.0 * aa * cc;
        if disc <= 0.0 {
            return None; // misses or grazes the disk: no chord
        }
        let sq = disc.sqrt();
        Some(((-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)))
    }
}

/// Chirality of an analytic surface's parametrization frame: `+1.0` for a
/// right-handed (proper) frame, `-1.0` for a left-handed (REFLECTED, i.e.
/// mirrored) frame. Mirrors massprops' `frame_handedness` (dossier 72) so the
/// boolean classifier's outward normal agrees with the mass/tessellation
/// orientation authority on a mirrored body. (massprops.rs is locked, so this
/// is a local copy rather than a shared import.)
fn surface_frame_handedness(surf: &Surface3) -> f64 {
    let f = match surf {
        Surface3::Plane(p) => &p.frame,
        Surface3::Cylinder(c) => &c.frame,
        Surface3::Cone(c) => &c.frame,
        Surface3::Sphere(s) => &s.frame,
        Surface3::Torus(t) => &t.frame,
    };
    if f.x.dot(f.y.cross(f.z)) >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Cyrus-Beck clip of an infinite 3D line (lying in a convex planar
/// face) to that face's outer polygon. Returns the parameter interval
/// `[t0, t1]` (in the line's own `point(t)` parameterization) where the
/// line is inside, or None if it misses the face. Convex faces only
/// (M6a primitives); non-convex trimmed faces are deferred.
fn clip_line_to_planar_face(
    line: &keel_geom::curve::Line3,
    plane: &keel_geom::surface::Plane3,
    poly3: &[keel_math::vec::Vec3],
) -> Option<(f64, f64)> {
    use keel_math::vec::Vec3;
    if poly3.len() < 3 {
        return None;
    }
    let fr = &plane.frame;
    let to2 = |p: Vec3| -> (f64, f64) {
        let w = p - fr.origin;
        (w.dot(fr.x), w.dot(fr.y))
    };
    let poly: Vec<(f64, f64)> = poly3.iter().map(|p| to2(*p)).collect();
    // Signed area to orient inward normals.
    let mut area2 = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        area2 += a.0 * b.1 - b.0 * a.1;
    }
    let ccw = area2 > 0.0;
    let o = to2(line.origin);
    // Direction in 2D (the line lies in the plane, so this is its full
    // direction; magnitude matches the 3D parameterization).
    let d = (line.dir.dot(fr.x), line.dir.dot(fr.y));
    let dmag = (d.0 * d.0 + d.1 * d.1).sqrt();
    let mut t_lo = f64::NEG_INFINITY;
    let mut t_hi = f64::INFINITY;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let e = (b.0 - a.0, b.1 - a.1);
        // Inward normal: left of edge for CCW, right for CW.
        let n = if ccw { (-e.1, e.0) } else { (e.1, -e.0) };
        let denom = n.0 * d.0 + n.1 * d.1;
        let num = n.0 * (o.0 - a.0) + n.1 * (o.1 - a.1); // n . (P0 - a)
        // PARALLEL test must scale with |n|*|d|, NOT a fixed 1e-300. When the
        // line lies ON (parallel to) a polygon edge -- the common case for the
        // miter seam of an adjacent chamfer, whose SSI chord runs exactly along
        // the cutter chamfer face's own boundary edge -- denom is zero in exact
        // arithmetic but rounds to ~1e-14 here. The old fixed threshold let that
        // garbage through, so t = -num/denom became a spurious finite half-plane
        // bound (it truncated the top-face chord at a phantom mid-edge vertex,
        // orphaning the seam coedges: the `unmatched coedge` shell-closure
        // failure of the 2nd adjacent chamfer). |n| == edge length, |d| == the
        // line's 2D speed; their product is the natural scale of `denom`.
        let nmag = (n.0 * n.0 + n.1 * n.1).sqrt();
        let par_eps = (nmag * dmag).max(1.0) * 1e-9;
        if denom.abs() <= par_eps {
            // Line parallel to this edge. `num` is the inward signed area of the
            // line origin vs the edge; on/inside (num >= -|n|*len_eps) imposes no
            // constraint, strictly outside means the line misses the polygon.
            let on_eps = nmag.max(1.0) * 1e-7;
            if num < -on_eps {
                return None; // parallel and strictly outside this edge
            }
        } else {
            let t = -num / denom;
            if denom > 0.0 {
                t_lo = t_lo.max(t);
            } else {
                t_hi = t_hi.min(t);
            }
        }
    }
    if t_lo <= t_hi {
        Some((t_lo, t_hi))
    } else {
        None
    }
}

/// An imprinted operand: a clone of the input body with the seam
/// curves split into it, plus the seam edges that were created (the
/// boundary along which this operand meets the other).
#[derive(Clone, Debug)]
pub struct ImprintedOperand {
    pub body: Body,
    /// Edges created along the intersection seams (one per seam curve
    /// imprinted on this operand).
    pub seam_edges: Vec<crate::entity::EdgeKey>,
}

/// Which operand a kept face came from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Operand {
    A,
    B,
}

/// A face selected into the result: which operand, its key in that
/// operand's imprinted body, and whether its orientation must be
/// reversed (difference flips the subtracted solid's walls inward).
#[derive(Clone, Copy, Debug)]
pub struct KeptFace {
    pub operand: Operand,
    pub face: FaceKey,
    pub reversed: bool,
}

/// Regularized selection tables (Requicha r-sets). Given each operand's
/// face classifications, choose the result face-set per operation:
/// - Union: A-faces outside B + B-faces outside A.
/// - Intersection: A-faces inside B + B-faces inside A.
/// - Difference A-B: A-faces outside B + B-faces inside A, B reversed.
pub(crate) fn select_faces(
    op: BoolOp,
    class_a: &[(FaceKey, FaceClass)],
    class_b: &[(FaceKey, FaceClass)],
) -> Vec<KeptFace> {
    // The Laidlaw-Trumbore-Hughes / Requicha on-on selection tables
    // (research file 39 §2.3). Coincident overlaps contribute ONE copy,
    // kept from operand A (lower-indexed) by convention; B's duplicate is
    // dropped. Same-sense (on+) survives union/intersection; opposite-
    // sense (on-) survives difference.
    let mut keep = Vec::new();
    let mut emit = |operand, face, reversed| {
        keep.push(KeptFace {
            operand,
            face,
            reversed,
        });
    };
    match op {
        BoolOp::Union => {
            for &(f, c) in class_a {
                if matches!(
                    c,
                    FaceClass::OutsideOther | FaceClass::OnOther(OnSense::Same)
                ) {
                    emit(Operand::A, f, false);
                }
            }
            for &(f, c) in class_b {
                if c == FaceClass::OutsideOther {
                    emit(Operand::B, f, false);
                }
            }
        }
        BoolOp::Intersection => {
            for &(f, c) in class_a {
                if matches!(
                    c,
                    FaceClass::InsideOther | FaceClass::OnOther(OnSense::Same)
                ) {
                    emit(Operand::A, f, false);
                }
            }
            for &(f, c) in class_b {
                if c == FaceClass::InsideOther {
                    emit(Operand::B, f, false);
                }
            }
        }
        BoolOp::Difference => {
            for &(f, c) in class_a {
                if matches!(
                    c,
                    FaceClass::OutsideOther | FaceClass::OnOther(OnSense::Opposite)
                ) {
                    emit(Operand::A, f, false);
                }
            }
            for &(f, c) in class_b {
                if c == FaceClass::InsideOther {
                    emit(Operand::B, f, true);
                }
            }
        }
    }
    keep
}

/// A face fragment's position relative to the OTHER operand solid.
/// Relative orientation of two coincident faces (research file 39 §2.1):
/// the outward normals agree (`Same`, `on+`) or oppose (`Opposite`,
/// `on-`) on the overlap; `Unknown` when the coincident partner / sense
/// could not be determined.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnSense {
    Same,
    Opposite,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaceClass {
    InsideOther,
    OutsideOther,
    /// On the other operand's boundary (coincident), with the relative
    /// orientation of the coincident pair (research file 39).
    OnOther(OnSense),
    /// Could not find an interior sample / classify.
    Unknown,
}

impl Body {
    /// Sample a fin's edge 3D curve in the fin's traversal direction
    /// (`m` points, start inclusive). Returns None if the edge has no
    /// curve.
    pub(crate) fn fin_curve_samples(
        &self,
        fin: FinKey,
        m: usize,
    ) -> Option<Vec<keel_math::vec::Vec3>> {
        use keel_geom::curve::Curve3;
        let f = self.fins.get(fin)?;
        let e = self.edges.get(f.edge)?;
        let (ck, csense) = e.curve?;
        let c = self.curves.get(ck)?;
        let fwd = csense == f.forward;
        // An arc with a RECORDED sweep on a periodic carrier samples its
        // OWN span exactly (arc identity, task 29): from the start
        // vertex's parameter angle, `arc_sweep` forward in the curve's
        // own parameterization, reversed for a backward fin. Without
        // this the sampler sweeps the FULL carrier and every consumer
        // (angular spans, height bands, interior points, UV polygons)
        // mislocates arc-bounded fragments. (Safe now that the
        // azimuth-reading shortcut in cyl_angular_span is gone: the
        // sweep semantic is curve-parameter everywhere.)
        if let Some(sweep) = e.arc_sweep
            && e.bounds.0 != e.bounds.1
            && matches!(c, Curve3::Circle(_) | Curve3::Ellipse(_))
        {
            let ang = |p: keel_math::vec::Vec3| -> f64 {
                match c {
                    Curve3::Circle(ci) => {
                        let w = p - ci.center;
                        w.dot(ci.y_axis).atan2(w.dot(ci.x_axis))
                    }
                    Curve3::Ellipse(el) => {
                        let w = p - el.center;
                        (w.dot(el.y_axis) / el.b).atan2(w.dot(el.x_axis) / el.a)
                    }
                    _ => unreachable!(),
                }
            };
            let evalt = |t: f64| -> keel_math::vec::Vec3 {
                match c {
                    Curve3::Circle(ci) => ci.point(t),
                    Curve3::Ellipse(el) => el.point(t),
                    _ => unreachable!(),
                }
            };
            let t0 = ang(self.vertices.get(e.bounds.0)?.point);
            let mut out = Vec::with_capacity(m);
            for i in 0..m {
                let f01 = i as f64 / m as f64;
                let f01 = if f.forward { f01 } else { 1.0 - f01 };
                out.push(evalt(t0 + sweep * f01));
            }
            return Some(out);
        }
        let eval = |s: f64| -> keel_math::vec::Vec3 {
            match c {
                Curve3::Nurbs(n) => {
                    let (a, b) = n.domain();
                    n.point(a + s * (b - a))
                }
                Curve3::Circle(ci) => ci.point(core::f64::consts::TAU * s),
                Curve3::Line(l) => l.point(s),
                Curve3::Ellipse(el) => el.point(core::f64::consts::TAU * s),
            }
        };
        // Restrict the sampled range to the edge's bounding vertices. A
        // SPLIT edge shares its parent's curve, so sweeping the full curve
        // domain would ride the whole PARENT edge -- corrupting the UV
        // polygon face_interior_point builds for a split fragment (e.g. an
        // asymmetric-chamfer corner triangle whose top/right sides are split
        // children: the sampled polygon then spans the untrimmed box edges,
        // the interior-point sampler escapes the true fragment, and the
        // fragment is mis-classified inside/outside the other operand). For
        // STRAIGHT curves (Line and degree>=1 NURBS) the parameter is
        // monotonic, so projecting the two bound vertices gives the edge's
        // true sub-range, sampled in the fin's traversal direction
        // (bounds.0 -> bounds.1 when forward). Closed edges (one shared
        // vertex) and periodic curves (circle/ellipse arcs, whose seam-wrap
        // direction is ambiguous from endpoints alone) keep the prior
        // full-[0,1] sweep in the fin direction.
        let s_of = |p: keel_math::vec::Vec3| -> Option<f64> {
            match c {
                Curve3::Line(l) => Some(l.project(p)),
                Curve3::Nurbs(n) => {
                    let (a, b) = n.domain();
                    if (b - a).abs() <= f64::MIN_POSITIVE {
                        None
                    } else {
                        Some((keel_geom::project::project_point(n, p, 1e-7).u - a) / (b - a))
                    }
                }
                _ => None,
            }
        };
        let range = if e.bounds.0 == e.bounds.1 {
            None
        } else {
            match (
                self.vertices
                    .get(e.bounds.0)
                    .map(|v| v.point)
                    .and_then(s_of),
                self.vertices
                    .get(e.bounds.1)
                    .map(|v| v.point)
                    .and_then(s_of),
            ) {
                (Some(s0), Some(s1)) => Some(if f.forward { (s0, s1) } else { (s1, s0) }),
                _ => None,
            }
        };
        let mut out = Vec::with_capacity(m);
        match range {
            Some((sa, sb)) => {
                for i in 0..m {
                    let t = i as f64 / m as f64;
                    out.push(eval(sa + (sb - sa) * t));
                }
            }
            None => {
                for i in 0..m {
                    let s = i as f64 / m as f64;
                    out.push(eval(if fwd { s } else { 1.0 - s }));
                }
            }
        }
        Some(out)
    }

    /// A 3D point on `face`'s surface, strictly interior to its trim
    /// loops. Builds each loop's UV polygon by projecting its 3D
    /// vertices onto the surface (exact for planes, no dependency on
    /// stored pcurve completeness), then grid-samples the outer loop's
    /// UV box and winding-tests against every loop. Analytic faces only.
    /// Interior point of a NURBS face fragment. For a NURBS sphere cap
    /// bounded by a CLOSED SSI circle: the cap apex, found by fast-
    /// projecting a far point along the circle axis (side from the
    /// boundary fin's loop kind, mirroring the analytic sphere) onto the
    /// NURBS surface. A whole NURBS sphere (no circle edge): a surface
    /// mid-parameter point.
    fn nurbs_face_interior_point(
        &self,
        face: FaceKey,
        nurbs: &keel_geom::nurbs_surface::NurbsSurface,
    ) -> Option<keel_math::vec::Vec3> {
        for lp in self.faces.get(face).map(|f| f.loops.clone())? {
            let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
                continue;
            };
            let inner = self
                .loops
                .get(lp)
                .map(|l| l.kind == crate::entity::LoopKind::Inner)
                == Some(true);
            let mut cur = entry;
            loop {
                let fin = self.fins.get(cur)?;
                let closed = self.edges.get(fin.edge).map(|e| e.is_closed()) == Some(true);
                if closed
                    && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                    && let Some(cv) = self.curves.get(ck)
                    && let Some((center_c, ax)) = closed_curve_center_axis(cv)
                    && let Some((m, t)) = closed_curve_point_tangent(cv, 0.25)
                {
                    // Outward normal at m via the NURBS surface.
                    let pm = keel_geom::project::project_point_surface_fast(nurbs, m);
                    let n = nurbs
                        .local_geometry(pm.u, pm.v)
                        .ok()
                        .map(|g| g.normal)
                        .unwrap_or(t);
                    let mut into = n.cross(t);
                    if inner {
                        into = into * -1.0;
                    }
                    let sign = if into.dot(ax) >= 0.0 { 1.0 } else { -1.0 };
                    let big = 4.0 * ((m - center_c).norm() + 1.0);
                    let far = center_c + ax * (sign * big);
                    let apex = keel_geom::project::project_point_surface_fast(nurbs, far);
                    return Some(apex.point);
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        // Whole-surface NURBS face: a mid-parameter point.
        let ((u0, u1), (v0, v1)) = nurbs.domain();
        Some(nurbs.point(0.5 * (u0 + u1), 0.5 * (v0 + v1)))
    }

    /// Interior point of a cylindrical lateral fragment: a point on the
    /// lateral at the fragment's mid-height (the band bounded by its
    /// CLOSED circle edges), at the angle opposite the seam (which sits
    /// at angle 0). Robust for the periodic cylinder.
    fn cylinder_face_interior_point(
        &self,
        face: FaceKey,
        cyl: &keel_geom::surface::Cylinder3,
    ) -> Option<keel_math::vec::Vec3> {
        let (origin, ex, ey, ez, r) = (
            cyl.frame.origin,
            cyl.frame.x,
            cyl.frame.y,
            cyl.frame.z,
            cyl.radius,
        );
        let mut heights = self.cyl_circle_heights(face, origin, ez);
        // DISTINCT rim heights only (a single rim reports once per fin).
        heights.sort_by(f64::total_cmp);
        heights.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        if heights.len() < 2 {
            // Fewer than two distinct rims (a crossing-pair piece
            // bounded by ellipse arcs): heights from the fins' curve
            // samples as well (KEEPING any rim height found).
            for lk in self
                .faces
                .get(face)
                .map(|f| f.loops.clone())
                .unwrap_or_default()
            {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                while let Some(fin) = self.fins.get(cur) {
                    for p in self.fin_curve_samples(cur, 8).unwrap_or_default() {
                        heights.push((p - origin).dot(ez));
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
            }
            if heights.len() < 2 {
                return None;
            }
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let hmid = 0.5 * (hlo + hhi);
        // The face's angular-span midpoint (a full ring gives pi, the
        // historic pick opposite the seam).
        let (a0, a1) = self.cyl_angular_span(face, origin, ex, ey, ez);
        let amid = 0.5 * (a0 + a1);
        let to_pt = |th: f64, h: f64| origin + (ex * th.cos() + ey * th.sin()) * r + ez * h;

        // INNER LOOPS (holes): a multi-loop cylinder face is the wall
        // MATERIAL with the bore seams punched out as holes (a thin cyl
        // boring through a thick one leaves the wall as one face with
        // two oval holes). The blind (amid, hmid) pick lands at the ring
        // centre, which for a diametric bore is the dead centre of a
        // hole -- a point INSIDE the other operand, so classify mis-keeps
        // the whole wall (cyl/cyl A n B over-keep, LOG Add. 270/272).
        // Mirror the planar grid path: when holes are present, search
        // (theta, h) for the most-central point OUTSIDE every hole.
        // Single-loop faces (bands, windows) keep the fast pick.
        let loops: Vec<crate::entity::LoopKey> = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        if loops.len() >= 2 {
            use core::f64::consts::{PI, TAU};
            use keel_math::vec::Vec3;
            // Each inner loop -> an unwrapped (theta, h) polygon. The bore
            // ovals are localised in theta and never wrap the ring, so
            // unwrapping relative to the running angle keeps each polygon
            // contiguous and branch-cut free.
            let angle_h = |p: Vec3| -> (f64, f64) {
                let d = p - origin;
                let w = d - ez * d.dot(ez);
                (w.dot(ey).atan2(w.dot(ex)), d.dot(ez))
            };
            let mut holes: Vec<Vec<(f64, f64)>> = Vec::new();
            for &lk in loops.iter().skip(1) {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut poly: Vec<(f64, f64)> = Vec::new();
                let mut prev = f64::NAN;
                let mut cur = entry;
                while let Some(fin) = self.fins.get(cur) {
                    for p in self.fin_curve_samples(cur, 12).unwrap_or_default() {
                        let (mut th, h) = angle_h(p);
                        if !prev.is_nan() {
                            while th - prev > PI {
                                th -= TAU;
                            }
                            while th - prev < -PI {
                                th += TAU;
                            }
                        }
                        prev = th;
                        poly.push((th, h));
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
                if poly.len() >= 3 {
                    holes.push(poly);
                }
            }
            if !holes.is_empty() {
                // Each hole's mean angle (its branch anchor) and an
                // arc-length-scaled copy (theta*r) so theta and h distances
                // are comparable surface lengths.
                let meta: Vec<(f64, Vec<(f64, f64)>)> = holes
                    .iter()
                    .map(|poly| {
                        let mean = poly.iter().map(|q| q.0).sum::<f64>() / poly.len() as f64;
                        (mean, poly.iter().map(|q| (q.0 * r, q.1)).collect())
                    })
                    .collect();
                let branch = |th: f64, mean: f64| {
                    let mut t = th;
                    while t - mean > PI {
                        t -= TAU;
                    }
                    while t - mean < -PI {
                        t += TAU;
                    }
                    t
                };
                const N: usize = 24;
                let mut best: Option<((f64, f64), f64)> = None;
                for i in 0..=N {
                    let th = a0 + (a1 - a0) * (i as f64 / N as f64);
                    for j in 1..N {
                        let h = hlo + (hhi - hlo) * (j as f64 / N as f64);
                        let mut inside = false;
                        let mut dmin = f64::INFINITY;
                        for (k, poly) in holes.iter().enumerate() {
                            let t = branch(th, meta[k].0);
                            if winding_nonzero(poly, (t, h)) {
                                inside = true;
                                break;
                            }
                            dmin = dmin.min(dist_to_polyline(&meta[k].1, (t * r, h)));
                        }
                        if inside {
                            continue;
                        }
                        if best.is_none_or(|(_, bd)| dmin > bd) {
                            best = Some(((th, h), dmin));
                        }
                    }
                }
                if let Some(((th, h), _)) = best {
                    return Some(to_pt(th, h));
                }
            }
        }
        // SINGLE non-periodic loop -- a WINDOW disc cut by a NURBS seam (a
        // cyl/sphere graze, the patch inside the seam loop). The amid/hmid pick
        // is the "opposite the seam" full-ring heuristic and lands OUTSIDE a
        // localized window; use the loop's own (theta, h) bounding-box centre,
        // interior to the convex-ish window. Detected by a theta extent that
        // does NOT wrap the full ring (a full lateral's rim wraps to ~2pi).
        if loops.len() == 1
            && let Some(&lk) = loops.first()
            && let Some(entry) = self.loops.get(lk).and_then(|l| l.fin)
        {
            use core::f64::consts::{PI, TAU};
            let angle_h = |p: keel_math::vec::Vec3| -> (f64, f64) {
                let d = p - origin;
                let w = d - ez * d.dot(ez);
                (w.dot(ey).atan2(w.dot(ex)), d.dot(ez))
            };
            let mut th_lo = f64::INFINITY;
            let mut th_hi = f64::NEG_INFINITY;
            let mut wh_lo = f64::INFINITY;
            let mut wh_hi = f64::NEG_INFINITY;
            let mut prev = f64::NAN;
            let mut cur = entry;
            let mut count = 0u32;
            while let Some(fin) = self.fins.get(cur) {
                for p in self.fin_curve_samples(cur, 16).unwrap_or_default() {
                    let (mut th, h) = angle_h(p);
                    if !prev.is_nan() {
                        while th - prev > PI {
                            th -= TAU;
                        }
                        while th - prev < -PI {
                            th += TAU;
                        }
                    }
                    prev = th;
                    th_lo = th_lo.min(th);
                    th_hi = th_hi.max(th);
                    wh_lo = wh_lo.min(h);
                    wh_hi = wh_hi.max(h);
                    count += 1;
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
            if count >= 3 && th_hi - th_lo < TAU - 0.1 {
                return Some(to_pt(0.5 * (th_lo + th_hi), 0.5 * (wh_lo + wh_hi)));
            }
        }
        Some(to_pt(amid, hmid))
    }

    /// The cone twin of the cylinder rung: rim heights bound the band,
    /// and the surface radius at the mid-height places the point.
    fn cone_face_interior_point(
        &self,
        face: FaceKey,
        cone: &keel_geom::surface::Cone3,
    ) -> Option<keel_math::vec::Vec3> {
        let (origin, ex, ez) = (cone.frame.origin, cone.frame.x, cone.frame.z);
        let ey = cone.frame.y;
        // SINGLE non-periodic loop -- a WINDOW disc cut by a NURBS seam (a
        // cone/sphere graze). The circle-rim path below finds no circle and
        // returns None; use the loop's own (theta, v) bbox centre, interior to
        // the convex window (theta NOT wrapping the full ring => a window).
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        if loops.len() == 1
            && let Some(entry) = self.loops.get(loops[0]).and_then(|l| l.fin)
        {
            use core::f64::consts::{PI, TAU};
            let angle_v = |p: keel_math::vec::Vec3| -> (f64, f64) {
                let d = p - origin;
                let vv = d.dot(ez);
                let w = d - ez * vv;
                (w.dot(ey).atan2(w.dot(ex)), vv)
            };
            let (mut tlo, mut thi) = (f64::INFINITY, f64::NEG_INFINITY);
            let (mut vlo, mut vhi) = (f64::INFINITY, f64::NEG_INFINITY);
            let mut prev = f64::NAN;
            let mut cnt = 0u32;
            let mut cur = entry;
            while let Some(fin) = self.fins.get(cur) {
                for p in self.fin_curve_samples(cur, 16).unwrap_or_default() {
                    let (mut th, vv) = angle_v(p);
                    if !prev.is_nan() {
                        while th - prev > PI {
                            th -= TAU;
                        }
                        while th - prev < -PI {
                            th += TAU;
                        }
                    }
                    prev = th;
                    tlo = tlo.min(th);
                    thi = thi.max(th);
                    vlo = vlo.min(vv);
                    vhi = vhi.max(vv);
                    cnt += 1;
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
            if cnt >= 3 && thi - tlo < TAU - 0.1 {
                let (tc, vc) = (0.5 * (tlo + thi), 0.5 * (vlo + vhi));
                let r = (cone.radius + vc * cone.half_angle.tan()).max(0.0);
                return Some(origin + (ex * tc.cos() + ey * tc.sin()) * r + ez * vc);
            }
        }
        let mut heights = self.cyl_circle_heights(face, origin, ez);
        // A TIP fragment has ONE rim circle and reaches the APEX (a degenerate
        // v-line, not a circle edge), so cyl_circle_heights yields a single
        // height and the midpoint would land ON the rim -- which, for a slab /
        // plane cut, is exactly on the cutting boundary, so the winding probe
        // mis-classifies the whole tip (the disconnected-difference dropped-tip
        // bug, LOG Add. 253). Add the apex height so the midpoint sits strictly
        // between rim and apex (the same apex handling tessellate_cone uses).
        let slope = cone.half_angle.tan();
        let (hlo0, hhi0) = (
            heights.iter().cloned().fold(f64::INFINITY, f64::min),
            heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        if (heights.len() < 2 || hhi0 - hlo0 <= 1e-9) && slope.abs() > 1e-12 {
            heights.push(-cone.radius / slope); // apex axial height
        }
        if heights.len() < 2 {
            return None;
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if hhi - hlo <= 1e-12 {
            return None;
        }
        let hmid = 0.5 * (hlo + hhi);
        let r = cone.radius + hmid * slope;
        Some(origin + ex * (-r) + ez * hmid)
    }

    /// Interior point of a spherical cap fragment: the cap apex, on the
    /// side of the bounding SSI circle that this face occupies (chosen
    /// by the boundary fin's into-face direction). Robust for the
    /// periodic sphere where UV winding fails.
    fn sphere_face_interior_point(&self, face: FaceKey) -> Option<keel_math::vec::Vec3> {
        let Surface3::Sphere(s) = self.face_surface3(face)? else {
            return None;
        };
        let center = s.frame.origin;
        let radius = s.radius;
        // BAND first (LOG Add. 262): a sphere zone bounded by TWO distinct
        // PARALLEL closed circle rims contains NO pole, so the cap/pole logic
        // below returns a pole OUTSIDE the band and classify mis-keeps it (the
        // sph - slab "two caps" read FULL-sphere mass). For such a
        // full-revolution band, ANY point at the mid latitude between the rims
        // is interior. Gated on >=2 DISTINCT parallel CLOSED circle rims, so
        // cap / lens / rest faces (one rim) and non-parallel multi-cut faces
        // fall through BYTE-UNCHANGED -- no regression to the working classes.
        {
            use keel_math::vec::Vec3;
            // Distinct circle rims (ARCS included -- the sphere's meridian seam
            // splits a band's rim into arcs -- grouped by distinct circle, so
            // cap / lens / rest faces (one circle, however many arcs) stay at
            // ONE distinct rim and never enter the band path.
            let mut rims: Vec<(Vec3, Vec3, f64)> = Vec::new();
            for lp in self
                .faces
                .get(face)
                .map(|f| f.loops.clone())
                .into_iter()
                .flatten()
            {
                let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                loop {
                    let Some(fin) = self.fins.get(cur) else { break };
                    let is_seam = self
                        .edges
                        .get(fin.edge)
                        .map(|e| {
                            e.radial.len() >= 2
                                && e.radial.iter().all(|&rf| {
                                    self.fins
                                        .get(rf)
                                        .and_then(|x| self.loops.get(x.owner))
                                        .map(|x| x.face)
                                        == Some(face)
                                })
                        })
                        .unwrap_or(false);
                    if !is_seam
                        && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                        && let Some(keel_geom::curve::Curve3::Circle(c)) = self.curves.get(ck)
                        && let Some(ax) = c.x_axis.cross(c.y_axis).try_normalize()
                        && !rims.iter().any(|(rc, _, rr)| {
                            (*rc - c.center).norm() < 1e-7 && (rr - c.radius).abs() < 1e-7
                        })
                    {
                        rims.push((c.center, ax, c.radius));
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
            }
            // NURBS-rim band fallback (dossier 67, frame-free classify): the
            // non-coaxial sphere-vs-{cyl,cone} WRAP splits the sphere by
            // encircling NON-circular NURBS rims, which the circle collection
            // above misses (so an encircling band fell to the centroid path,
            // whose loop centroid points at a POLE -- wrong for a band that
            // holds no pole). Collect ONE rim per non-seam loop from its 3D
            // samples: the rim's solid-angle axis n = sum_j d_j x d_{j+1}
            // (Van Oosterom-Strackee, d_j the unit sample directions) is the
            // axis it encircles, well-conditioned even near a great circle and
            // INVARIANT to the sphere's parametric frame. The existing band
            // sweep below is frame-free in that axis. Runs ONLY when the circle
            // path found < 2 rims, so every circle-rim band stays byte-unchanged
            // (no regression to the Add.262/267/268 classes). Foundational: the
            // wrap still declines downstream on mass (68) / tessellation (69).
            if rims.len() < 2 {
                let mut nrims: Vec<(Vec3, Vec3, f64)> = Vec::new();
                for lp in self
                    .faces
                    .get(face)
                    .map(|f| f.loops.clone())
                    .into_iter()
                    .flatten()
                {
                    let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
                        continue;
                    };
                    let mut samples: Vec<Vec3> = Vec::new();
                    let mut cur = entry;
                    loop {
                        let Some(fin) = self.fins.get(cur) else { break };
                        let is_seam = self
                            .edges
                            .get(fin.edge)
                            .map(|e| {
                                e.radial.len() >= 2
                                    && e.radial.iter().all(|&rf| {
                                        self.fins
                                            .get(rf)
                                            .and_then(|x| self.loops.get(x.owner))
                                            .map(|x| x.face)
                                            == Some(face)
                                    })
                            })
                            .unwrap_or(false);
                        if !is_seam && let Some(pts) = self.fin_curve_samples(cur, 24) {
                            samples.extend(pts);
                        }
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                    let dirs: Vec<Vec3> = samples
                        .iter()
                        .filter_map(|p| (*p - center).try_normalize())
                        .collect();
                    if dirs.len() < 8 {
                        continue;
                    }
                    let mut area = Vec3::ZERO;
                    for i in 0..dirs.len() {
                        area = area + dirs[i].cross(dirs[(i + 1) % dirs.len()]);
                    }
                    let Some(axis) = area.try_normalize() else {
                        continue;
                    };
                    let h = samples.iter().map(|p| (*p - center).dot(axis)).sum::<f64>()
                        / samples.len() as f64;
                    nrims.push((
                        center + axis * h,
                        axis,
                        (radius * radius - h * h).max(0.0).sqrt(),
                    ));
                }
                if nrims.len() >= 2 {
                    rims = nrims;
                }
            }
            if rims.len() >= 2 {
                let ax = rims[0].1;
                if rims.iter().all(|(_, a, _)| a.cross(ax).norm() < 1e-6) {
                    let mut hs: Vec<f64> =
                        rims.iter().map(|(c, _, _)| (*c - center).dot(ax)).collect();
                    hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
                    let (h_lo, h_hi) = (hs[0], hs[hs.len() - 1]);
                    if h_hi - h_lo > 1e-9 {
                        let h_m = 0.5 * (h_lo + h_hi);
                        let rho = (radius * radius - h_m * h_m).max(0.0).sqrt();
                        let t = if ax.x.abs() < 0.9 {
                            Vec3::new(1.0, 0.0, 0.0)
                        } else {
                            Vec3::new(0.0, 1.0, 0.0)
                        };
                        if let Some(p1) = (t - ax * t.dot(ax)).try_normalize() {
                            let p2 = ax.cross(p1);
                            // Sweep azimuths around the band ring; return the first
                            // candidate the trimmed-domain test confirms INTERIOR,
                            // so a partial (azimuthally trimmed) band can never get
                            // an out-of-face point. Full-ring bands accept any.
                            let mut fallback: Option<(Vec3, f64)> = None;
                            for k in 0..12 {
                                let a = k as f64 * (core::f64::consts::TAU / 12.0);
                                let p = center + ax * h_m + (p1 * a.cos() + p2 * a.sin()) * rho;
                                let w = p - center;
                                let u = w
                                    .dot(s.frame.y)
                                    .atan2(w.dot(s.frame.x))
                                    .rem_euclid(core::f64::consts::TAU);
                                let vlat = (w.dot(s.frame.z) / radius).clamp(-1.0, 1.0).asin();
                                // Pick the candidate farthest from BOTH the
                                // parameterization seam (u=0) AND the poles
                                // (v=+-pi/2): when the band's rim axis is not the
                                // sphere's frame axis (a tilted sphere), the
                                // mid-latitude ring passes through the sphere's
                                // poles, and a pole point is degenerate (u
                                // undefined) and mis-classifies.
                                let to_seam = u.min(core::f64::consts::TAU - u);
                                let to_pole = core::f64::consts::FRAC_PI_2 - vlat.abs();
                                let margin = to_seam.min(to_pole);
                                if fallback.map(|(_, d)| margin > d).unwrap_or(true) {
                                    fallback = Some((p, margin));
                                }
                                if self.point_in_face_uv(face, (u, vlat), 1e-6)
                                    == crate::pmc::UvClass::In
                                {
                                    return Some(p);
                                }
                            }
                            // FALLBACK: a band's interior is at the mid latitude,
                            // never a pole. If no azimuth verified in-domain (a
                            // full-revolution band's wrap-around uv has no simple
                            // winding), return the mid-latitude point farthest from
                            // the u=0 seam.
                            if let Some((p, _)) = fallback {
                                return Some(p);
                            }
                        }
                    }
                }
            }
        }
        // Any loop fin whose edge is a circle (the SSI seam) determines
        // the cap side. The two caps share this edge with the SAME
        // forward flag and pcurve, so the side is fixed by the fin's
        // loop kind: the disc uses the circle as its OUTER loop
        // (interior to the left), the rest face as an INNER ring
        // (interior to the other side).
        for lp in self.faces.get(face).map(|f| f.loops.clone())? {
            let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
                continue;
            };
            let inner = self
                .loops
                .get(lp)
                .map(|l| l.kind == crate::entity::LoopKind::Inner)
                == Some(true);
            let mut cur = entry;
            loop {
                let fin = self.fins.get(cur)?;
                // An SSI rim bounds the cap whether it is the closed
                // circle (M6a) or split arcs (the seam-crossing imprint).
                // Skip the sphere's own seam edges (BOTH fins on this
                // face: they bound nothing) and non-circle geometry.
                let seam_edge = self
                    .edges
                    .get(fin.edge)
                    .map(|e| {
                        e.radial.len() >= 2
                            && e.radial.iter().all(|&rf| {
                                self.fins
                                    .get(rf)
                                    .and_then(|x| self.loops.get(x.owner))
                                    .map(|x| x.face)
                                    == Some(face)
                            })
                    })
                    .unwrap_or(false);
                if !seam_edge
                    && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                    && let Some(cv) = self.curves.get(ck)
                    && matches!(cv, keel_geom::curve::Curve3::Circle(_))
                    && let Some((_center_c, ax)) = closed_curve_center_axis(cv)
                    && let Some((m, t)) = closed_curve_point_tangent(cv, 0.25)
                {
                    // Side WITNESS first: a boundary vertex OFF this rim
                    // circle (a pole at the end of a seam stub, after the
                    // seam-crossing imprint) tells directly which side of
                    // the rim this face occupies. The orientation rule
                    // below cannot: the crossing topology's flags are
                    // indistinguishable from the ring imprint's.
                    if let Some(cir) = match cv {
                        keel_geom::curve::Curve3::Circle(c) => Some(c),
                        _ => None,
                    } {
                        let off_rim = self
                            .faces
                            .get(face)
                            .map(|f| f.loops.clone())
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|lk| self.loops.get(lk).and_then(|l| l.fin))
                            .flat_map(|e2| {
                                let mut vs = Vec::new();
                                let mut c2 = e2;
                                loop {
                                    if let Some(v) = self.fin_start_vertex(c2)
                                        && let Some(x) = self.vertices.get(v)
                                    {
                                        vs.push(x.point);
                                    }
                                    let Some(nx) = self.fins.get(c2).map(|x| x.next) else {
                                        break;
                                    };
                                    c2 = nx;
                                    if c2 == e2 {
                                        break;
                                    }
                                }
                                vs
                            })
                            .find(|p| {
                                let w = *p - cir.center;
                                let nrm = cir.x_axis.cross(cir.y_axis);
                                w.dot(nrm).abs() > 1e-7
                                    || ((w - nrm * w.dot(nrm)).norm() - cir.radius).abs() > 1e-7
                            });
                        if let Some(p) = off_rim {
                            let side = (p - center).dot(ax);
                            if side.abs() > 1e-9 {
                                let apex = center + ax * (radius * side.signum());
                                return Some(apex);
                            }
                        }
                    }
                    let n = (m - center).try_normalize()?; // sphere outward at m
                    // No off-rim witness (the M6a ring topology: the rim
                    // is the face's whole boundary): the orientation rule.
                    // Into-face = surface-tangent to the LEFT of the
                    // curve-ordered traversal, flipped for an inner ring.
                    let mut into = n.cross(t);
                    if inner {
                        into = into * -1.0;
                    }
                    let sign = if into.dot(ax) >= 0.0 { 1.0 } else { -1.0 };
                    let apex = center + ax * (radius * sign);
                    if ((apex - center).norm() - radius).abs() < 1e-6 * radius.max(1.0) {
                        return Some(apex);
                    }
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        // NURBS WINDOW loop (a sphere patch cut by a NON-circular seam, e.g. a
        // cyl/sphere graze): the circle-cap path above found no circle rim.
        // Use the seam loop's centroid DIRECTION from the sphere centre -- the
        // disc patch (loop is the OUTER loop) sits UNDER it, the rest (loop is
        // an INNER ring) opposite. Verified against the trimmed (u, v) domain.
        {
            use keel_math::vec::Vec3;
            for lp in self
                .faces
                .get(face)
                .map(|f| f.loops.clone())
                .into_iter()
                .flatten()
            {
                let Some(entry) = self.loops.get(lp).and_then(|l| l.fin) else {
                    continue;
                };
                let inner = self
                    .loops
                    .get(lp)
                    .map(|l| l.kind == crate::entity::LoopKind::Inner)
                    == Some(true);
                let mut sum = Vec3::ZERO;
                let mut n = 0u32;
                let mut cur = entry;
                loop {
                    if let Some(pts) = self.fin_curve_samples(cur, 16) {
                        for p in pts {
                            sum = sum + p;
                            n += 1;
                        }
                    }
                    let Some(nx) = self.fins.get(cur).map(|f| f.next) else {
                        break;
                    };
                    cur = nx;
                    if cur == entry {
                        break;
                    }
                }
                if n == 0 {
                    continue;
                }
                let cloop = sum * (1.0 / n as f64);
                let Some(dir) = (cloop - center).try_normalize() else {
                    continue;
                };
                let p = center + dir * (radius * if inner { -1.0 } else { 1.0 });
                if inner {
                    // REST face (window as an INNER ring): the antipode of the
                    // window centroid is the big region's interior. The seamed
                    // sphere rest defeats point_in_face_uv's (u,v) winding (it
                    // returns None for every grid point), so trust the antipode
                    // -- the tight cyl/sphere oracle backstops any error.
                    return Some(p);
                }
                let w = p - center;
                let u = w
                    .dot(s.frame.y)
                    .atan2(w.dot(s.frame.x))
                    .rem_euclid(core::f64::consts::TAU);
                let vlat = (w.dot(s.frame.z) / radius).clamp(-1.0, 1.0).asin();
                if self.point_in_face_uv(face, (u, vlat), 1e-6) == crate::pmc::UvClass::In {
                    return Some(p);
                }
            }
        }
        // GENERAL grid fallback: scan the (u, v) domain for any point the
        // trimmed-face test confirms interior. Handles a sphere REST face (a
        // window as an inner ring -> the big region, which the centroid pick
        // and the circle-cap path both miss) and any other loop shape. Poles
        // (v = +-pi/2, where u is degenerate) are skipped.
        {
            use core::f64::consts::{FRAC_PI_2, PI, TAU};
            const NU: usize = 24;
            const NV: usize = 12;
            for iu in 0..NU {
                let u = (iu as f64 + 0.5) * TAU / NU as f64;
                for iv in 1..NV {
                    let vlat = -FRAC_PI_2 + (iv as f64) * PI / NV as f64;
                    if self.point_in_face_uv(face, (u, vlat), 1e-6) == crate::pmc::UvClass::In {
                        return Some(
                            center
                                + (s.frame.x * (u.cos() * vlat.cos())
                                    + s.frame.y * (u.sin() * vlat.cos())
                                    + s.frame.z * vlat.sin())
                                    * radius,
                        );
                    }
                }
            }
        }
        None
    }

    pub(crate) fn face_interior_point(&self, face: FaceKey) -> Option<keel_math::vec::Vec3> {
        let _prof = crate::profile::Scope::new(&crate::profile::INTERIOR_PT_NS);
        crate::profile::count(&crate::profile::INTERIOR_PT_CALLS);
        // NURBS faces (no analytic Surface3): handled via the NURBS
        // surface directly (M7b).
        if let Some((sk, _)) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(crate::entity::SurfaceGeom::Nurbs(n)) = self.surfaces.get(sk)
        {
            return self.nurbs_face_interior_point(face, &n.clone());
        }
        let surf = self.face_surface3(face)?;
        // Curved faces need a 3D interior point: their periodic
        // parameterization defeats the planar UV-winding path.
        if let Surface3::Sphere(_) = surf {
            return self.sphere_face_interior_point(face);
        }
        if let Surface3::Cylinder(c) = &surf {
            return self.cylinder_face_interior_point(face, c);
        }
        if let Surface3::Cone(c) = &surf {
            return self.cone_face_interior_point(face, c);
        }
        let loops: Vec<crate::entity::LoopKey> = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        // Build each loop's UV polygon by sampling its fins' EDGE 3D
        // curves (handles closed-curve edges like the ring, which have
        // only one vertex) and projecting to the surface.
        let mut uv_loops: Vec<Vec<(f64, f64)>> = Vec::new();
        for lk in &loops {
            let Some(entry) = self.loops.get(*lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut poly: Vec<(f64, f64)> = Vec::new();
            let mut cur = entry;
            loop {
                // STRAIGHT fins contribute exactly their start vertex
                // (OPT-M3: sampling a line 16x and projecting each
                // sample dominated this function's residual cost; the
                // chord polygon is identical without the collinear
                // interior points). Curved fins keep the samples.
                let straight = self
                    .fins
                    .get(cur)
                    .and_then(|f| self.edges.get(f.edge))
                    .map(|e| match e.curve.and_then(|(ck, _)| self.curves.get(ck)) {
                        None => true,
                        Some(keel_geom::curve::Curve3::Line(_)) => true,
                        Some(keel_geom::curve::Curve3::Nurbs(n)) => n.degree() <= 1,
                        Some(_) => false,
                    })
                    .unwrap_or(true);
                if straight {
                    if let Some(v) = self.fin_start_vertex(cur)
                        && let Some(x) = self.vertices.get(v)
                        && let Ok(pr) = surf.project(x.point)
                    {
                        poly.push((pr.u, pr.v));
                    }
                } else if let Some(samples) = self.fin_curve_samples(cur, 16) {
                    for p in samples {
                        if let Ok(pr) = surf.project(p) {
                            poly.push((pr.u, pr.v));
                        }
                    }
                }
                let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                    break;
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
            if poly.len() >= 3 {
                uv_loops.push(poly);
            }
        }
        let outer = uv_loops.first()?;
        // CONVEX-CENTROID fast path (OPT-M2): the profile showed this
        // function at 90 percent of the boolean workload, almost all
        // of it the 24x24 most-central grid below. For a CONVEX
        // single-loop face (every box face and most fragments) the
        // vertex centroid is strictly interior with clearance
        // proportional to the face, so it serves every consumer the
        // grid's most-central point served. Concave, degenerate, or
        // ring-carrying faces fall through to the grid unchanged.
        if uv_loops.len() == 1 {
            let m = outer.len();
            let mut crosses = Vec::with_capacity(m);
            let mut scale = 0.0f64;
            for i in 0..m {
                let a = outer[i];
                let b2 = outer[(i + 1) % m];
                let c2 = outer[(i + 2) % m];
                let cr = (b2.0 - a.0) * (c2.1 - b2.1) - (b2.1 - a.1) * (c2.0 - b2.0);
                scale = scale.max(cr.abs());
                crosses.push(cr);
            }
            // Collinear runs (straight edges sampled densely) produce
            // crosses at last-ulp noise around zero; classify against
            // a scale-relative threshold so only genuinely mixed
            // turning (a concave corner) rejects the fast path.
            let eps = 1e-9 * scale;
            let pos = crosses.iter().any(|&c| c > eps);
            let neg = crosses.iter().any(|&c| c < -eps);
            if !(pos && neg) && scale > 0.0 {
                let n = m as f64;
                let (cu, cv) = outer
                    .iter()
                    .fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
                let c = (cu / n, cv / n);
                if winding_nonzero(outer, c) {
                    return Some(surf.point(c.0, c.1));
                }
            }
        }
        let (mut umin, mut umax, mut vmin, mut vmax) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for &(u, v) in outer {
            umin = umin.min(u);
            umax = umax.max(u);
            vmin = vmin.min(v);
            vmax = vmax.max(v);
        }
        // Pick the MOST CENTRAL interior sample (max distance to every
        // loop boundary), so the classification point sits well away
        // from the seams (which lie on the other operand's boundary).
        const N: usize = 24;
        let mut best: Option<((f64, f64), f64)> = None;
        for i in 1..N {
            for j in 1..N {
                let u = umin + (umax - umin) * (i as f64 / N as f64);
                let v = vmin + (vmax - vmin) * (j as f64 / N as f64);
                if !winding_nonzero(&uv_loops[0], (u, v)) {
                    continue;
                }
                if uv_loops[1..].iter().any(|h| winding_nonzero(h, (u, v))) {
                    continue;
                }
                let d = uv_loops
                    .iter()
                    .map(|poly| dist_to_polyline(poly, (u, v)))
                    .fold(f64::INFINITY, f64::min);
                if best.is_none_or(|(_, bd)| d > bd) {
                    best = Some(((u, v), d));
                }
            }
        }
        if best.is_none() {
            // THIN-STRIP fallback: the fixed grid misses a face narrower
            // than its spacing (the L-shaped remainder a contact overlap
            // leaves on a mating face: both strips of the L can be under
            // 1/24 of the bbox, every grid sample lands in the notch, and
            // the face would be dropped as Unknown, orphaning its rim).
            // The largest tessellation triangle's centroid is interior by
            // construction for straight-edge planar faces; verify against
            // the loops before trusting it on curved-boundary faces.
            let tris = self.tessellate_face(face);
            let mut cand: Option<(keel_math::vec::Vec3, f64)> = None;
            for t in &tris {
                let ar = (t[1] - t[0]).cross(t[2] - t[0]).norm();
                if cand.is_none_or(|(_, b)| ar > b) {
                    cand = Some(((t[0] + t[1] + t[2]) * (1.0 / 3.0), ar));
                }
            }
            if let Some((c, _)) = cand
                && let Ok(pr) = surf.project(c)
                && winding_nonzero(&uv_loops[0], (pr.u, pr.v))
                && !uv_loops[1..]
                    .iter()
                    .any(|h| winding_nonzero(h, (pr.u, pr.v)))
            {
                return Some(surf.point(pr.u, pr.v));
            }
        }
        best.map(|((u, v), _)| surf.point(u, v))
    }
}

/// Minimum distance from a point to a closed polyline's edges.
fn dist_to_polyline(poly: &[(f64, f64)], q: (f64, f64)) -> f64 {
    let n = poly.len();
    let mut best = f64::INFINITY;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let (abx, aby) = (b.0 - a.0, b.1 - a.1);
        let len2 = abx * abx + aby * aby;
        let t = if len2 > 0.0 {
            (((q.0 - a.0) * abx + (q.1 - a.1) * aby) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (cx, cy) = (a.0 + t * abx - q.0, a.1 + t * aby - q.1);
        best = best.min((cx * cx + cy * cy).sqrt());
    }
    best
}

/// Center and unit axis of a CLOSED planar seam curve (a Circle3/
/// Ellipse3 exactly, or a fitted NURBS circle via its sample centroid +
/// Newell normal). The NURBS case is what an analytic-vs-spline or
/// spline-vs-spline SSI produces. None for a Line.
pub(crate) fn closed_curve_center_axis(
    curve: &keel_geom::curve::Curve3,
) -> Option<(keel_math::vec::Vec3, keel_math::vec::Vec3)> {
    use keel_geom::curve::Curve3;
    use keel_math::vec::Vec3;
    match curve {
        Curve3::Circle(c) => Some((c.center, c.x_axis.cross(c.y_axis).try_normalize()?)),
        Curve3::Ellipse(e) => Some((e.center, e.x_axis.cross(e.y_axis).try_normalize()?)),
        Curve3::Nurbs(n) => {
            let (a, b) = n.domain();
            const N: usize = 32;
            let pts: Vec<Vec3> = (0..N)
                .map(|k| n.point(a + (b - a) * k as f64 / N as f64))
                .collect();
            let centroid = pts.iter().fold(Vec3::ZERO, |s, p| s + *p) * (1.0 / N as f64);
            // Newell normal of the sample polygon.
            let mut nrm = Vec3::ZERO;
            for i in 0..N {
                let (p, q) = (pts[i], pts[(i + 1) % N]);
                nrm = nrm
                    + Vec3::new(
                        (p.y - q.y) * (p.z + q.z),
                        (p.z - q.z) * (p.x + q.x),
                        (p.x - q.x) * (p.y + q.y),
                    );
            }
            Some((centroid, nrm.try_normalize()?))
        }
        Curve3::Line(_) => None,
    }
}

/// A point on a closed curve at parameter fraction `t` and the unit
/// tangent there (finite difference). For the cap-side sign.
fn closed_curve_point_tangent(
    curve: &keel_geom::curve::Curve3,
    t: f64,
) -> Option<(keel_math::vec::Vec3, keel_math::vec::Vec3)> {
    use keel_geom::curve::Curve3;
    use keel_math::vec::Vec3;
    let s = |x: f64| -> Vec3 {
        match curve {
            Curve3::Circle(c) => c.point(core::f64::consts::TAU * x),
            Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * x),
            Curve3::Nurbs(n) => {
                let (a, b) = n.domain();
                n.point(a + (b - a) * x)
            }
            Curve3::Line(l) => l.point(x),
        }
    };
    let dt = 1e-4;
    let m = s(t);
    let tangent = (s(t + dt) - s(t - dt)).try_normalize()?;
    Some((m, tangent))
}

/// Nonzero-winding point-in-polygon (signed angle sum).
fn winding_nonzero(poly: &[(f64, f64)], q: (f64, f64)) -> bool {
    let mut total = 0.0f64;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let va = (a.0 - q.0, a.1 - q.1);
        let vb = (b.0 - q.0, b.1 - q.1);
        let cross = va.0 * vb.1 - va.1 * vb.0;
        let dot = va.0 * vb.0 + va.1 * vb.1;
        total += cross.atan2(dot);
    }
    (total / core::f64::consts::TAU).abs() > 0.5
}

/// Classify every face of `working` against the `other` operand solid:
/// sample the face interior and test containment in `other`.
pub(crate) fn classify_faces(working: &Body, other: &Body, tol: f64) -> Vec<(FaceKey, FaceClass)> {
    // Generalized winding number is the PRIMARY classifier (the
    // d-booleans-tolerant.md mandate): robust at on-boundary/tangential
    // contacts and surface-type-agnostic (no pcurve/periodicity
    // dependency), where ray-cast PMC was fragile. w ~ 1 inside other,
    // ~ 0 outside; the band around 0.5 means the sample sits on/near
    // other's boundary (coincident -> M6c).
    const COINCIDENCE_BAND: f64 = 0.25;
    // One outer-boundary tessellation of `other` for EVERY probe this
    // classification makes (OPT-M4): `other` is immutably borrowed for
    // the whole call, so the set cannot go stale.
    let other_tris = other.boundary_triangles();
    let mut out = Vec::new();
    for face in working.face_keys() {
        // Reject degenerate (zero-area) fragments before classifying. A thin
        // tool sliver can collapse to a lamina -- e.g. the asymmetric-chamfer
        // cutter's apex-face portion that dips inside the box reduces to the
        // setback line. face_interior_point can still land a point on such a
        // sliver and the winding test then mis-keeps it (a 2-vertex face that
        // breaks mass_properties and inflates the shell). An area within tol^2
        // of zero is not a real face: mark it Unknown (select_faces keeps no
        // Unknown), dropping it from both assembly paths.
        if working.face_area(face).abs() <= tol * tol {
            if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
                eprintln!(
                    "  classify {face:?}: AREA GATE ({})",
                    working.face_area(face)
                );
            }
            out.push((face, FaceClass::Unknown));
            continue;
        }
        let class = match working.face_interior_point(face) {
            Some(p) => {
                // GEOMETRIC coincidence FIRST (file 39 §1.4: the
                // winding number is UNDEFINED on the boundary, so a
                // sample lying on a coincident carrier must never be
                // classified by its numerically noisy winding; the
                // extent-guarded carrier test decides directly).
                let geo_sense = match working.face_outward_normal(face) {
                    Some(n) => other.coincident_sense_at(p, n, tol),
                    None => OnSense::Unknown,
                };
                if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
                    let w = crate::winding::gwn_over(&other_tris, p);
                    eprintln!("  classify {face:?} p {p:?} geo {geo_sense:?} w {w}");
                }
                if geo_sense != OnSense::Unknown {
                    FaceClass::OnOther(geo_sense)
                } else {
                    let w = crate::winding::gwn_over(&other_tris, p);
                    if (w - 0.5).abs() < COINCIDENCE_BAND {
                        // In the band with NO coincident carrier: the
                        // dossier-39 sec 1.4 TWO-SIDED test. A sample
                        // at a feature mouth (a pin cap's centre over
                        // the hole) has an ambiguous one-point winding
                        // but well-defined side limits along the
                        // face's own normal.
                        let eps = (tol * 100.0).max(1e-6);
                        match working.face_outward_normal(face) {
                            Some(nv) => {
                                let wp = crate::winding::gwn_over(&other_tris, p + nv * eps);
                                let wm = crate::winding::gwn_over(&other_tris, p - nv * eps);
                                if wp < 0.25 && wm < 0.25 {
                                    FaceClass::OutsideOther
                                } else if wp > 0.75 && wm > 0.75 {
                                    FaceClass::InsideOther
                                } else {
                                    FaceClass::OnOther(OnSense::Unknown)
                                }
                            }
                            None => FaceClass::OnOther(OnSense::Unknown),
                        }
                    } else if w > 0.5 {
                        FaceClass::InsideOther
                    } else {
                        FaceClass::OutsideOther
                    }
                }
            }
            None => {
                if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
                    eprintln!("  classify {face:?}: NO INTERIOR POINT");
                }
                FaceClass::Unknown
            }
        };
        out.push((face, class));
    }
    out
}

/// A boolean result: the assembled body plus any non-fatal faults
/// gathered along the way.
#[derive(Debug)]
/// The outcome of a [`boolean`] operation.
///
/// Partial-success model: `body` is the best result the kernel could
/// assemble and `faults` lists every face pair it could not handle, so
/// a caller sees what was skipped rather than getting an all-or-nothing
/// answer. An empty `faults` vector means the result is complete. The
/// DECLINE-never-WRONG contract is upstream of this type: the kernel
/// returns `Err(BoolFault)` rather than a geometrically wrong `body`.
pub struct BoolResult {
    /// The resulting body. May be empty (e.g. an empty intersection).
    pub body: Body,
    /// Face pairs the kernel could not resolve (empty = complete).
    pub faults: Vec<BoolFault>,
    /// The operation that produced this result.
    pub op: BoolOp,
}

#[allow(clippy::too_many_arguments)]
fn import_face(
    dst: &mut Body,
    src: &Body,
    face: FaceKey,
    op: Operand,
    reversed: bool,
    rec: &mut crate::body::OpRecorder,
    vmap: &mut std::collections::BTreeMap<(Operand, u64), crate::entity::VertexKey>,
    emap: &mut std::collections::BTreeMap<(Operand, u64), crate::entity::EdgeKey>,
    inf: crate::entity::RegionKey,
    solid: crate::entity::RegionKey,
) -> Option<FaceKey> {
    use crate::lineage::Derivation;
    let sface = src.faces.get(face)?;
    // A solid boundary face always has its (possibly reversed) outward
    // normal on the FRONT side facing the exterior. Reversal flips the
    // surface sense, fin senses, and loop order below -- not the
    // front/back region assignment.
    let dface = dst.new_face(rec, inf, solid, Derivation::Created);
    let sloops = sface.loops.clone();
    let mut dloops = Vec::new();
    for slk in sloops {
        let (kind, entry) = {
            let sl = src.loops.get(slk)?;
            (sl.kind, sl.fin)
        };
        let dlk = dst.new_loop(rec, dface, kind, Derivation::Created);
        let Some(entry) = entry else {
            dloops.push(dlk);
            continue;
        };
        // Source fins in loop order.
        let mut sfins = Vec::new();
        let mut cur = entry;
        loop {
            sfins.push(cur);
            let f = src.fins.get(cur)?;
            cur = f.next;
            if cur == entry {
                break;
            }
        }
        let mut dfins = Vec::new();
        for &sf in &sfins {
            let sfin = src.fins.get(sf)?;
            let dedge = import_edge(dst, src, sfin.edge, op, rec, vmap, emap)?;
            let fwd = sfin.forward ^ reversed;
            let dfin = dst.new_fin(rec, dedge, fwd, dlk, Derivation::Created);
            if let Some((pck, psense)) = sfin.pcurve
                && let Some(pc) = src.curves.get(pck).cloned()
            {
                let dpc = dst.add_curve(pc);
                if let Some(df) = dst.fins.get_mut(dfin) {
                    df.pcurve = Some((dpc, psense));
                }
            }
            if let Some(e) = dst.edges.get_mut(dedge) {
                e.radial.push(dfin);
            }
            dfins.push(dfin);
        }
        if reversed {
            dfins.reverse();
        }
        let n = dfins.len();
        for i in 0..n {
            let (nx, pv) = (dfins[(i + 1) % n], dfins[(i + n - 1) % n]);
            if let Some(f) = dst.fins.get_mut(dfins[i]) {
                f.next = nx;
                f.prev = pv;
            }
        }
        if let Some(l) = dst.loops.get_mut(dlk) {
            l.fin = Some(dfins[0]);
        }
        // Vertex fin back-pointers.
        for &df in &dfins {
            if let Some(sv) = dst.fin_start_vertex(df)
                && let Some(v) = dst.vertices.get_mut(sv)
                && v.fin.is_none()
            {
                v.fin = Some(df);
            }
        }
        dloops.push(dlk);
    }
    if let Some(f) = dst.faces.get_mut(dface) {
        f.loops = dloops;
    }
    if let Some((ssk, ssense)) = sface.surface
        && let Some(sg) = src.surfaces.get(ssk).cloned()
    {
        let dsk = dst.add_surface(sg);
        if let Some(f) = dst.faces.get_mut(dface) {
            f.surface = Some((dsk, ssense ^ reversed));
        }
    }
    Some(dface)
}

fn import_vertex(
    dst: &mut Body,
    src: &Body,
    v: crate::entity::VertexKey,
    op: Operand,
    rec: &mut crate::body::OpRecorder,
    vmap: &mut std::collections::BTreeMap<(Operand, u64), crate::entity::VertexKey>,
) -> Option<crate::entity::VertexKey> {
    let sv = src.vertices.get(v)?;
    let key = (op, sv.id.0);
    if let Some(&dv) = vmap.get(&key) {
        return Some(dv);
    }
    let stol = sv.tolerance;
    let dv = dst.new_vertex(rec, sv.point);
    if let Some(vt) = dst.vertices.get_mut(dv) {
        vt.tolerance = vt.tolerance.max(stol);
    }
    vmap.insert(key, dv);
    Some(dv)
}

fn import_edge(
    dst: &mut Body,
    src: &Body,
    edge: crate::entity::EdgeKey,
    op: Operand,
    rec: &mut crate::body::OpRecorder,
    vmap: &mut std::collections::BTreeMap<(Operand, u64), crate::entity::VertexKey>,
    emap: &mut std::collections::BTreeMap<(Operand, u64), crate::entity::EdgeKey>,
) -> Option<crate::entity::EdgeKey> {
    use crate::lineage::Derivation;
    let sid = src.edges.get(edge)?.id.0;
    if let Some(&de) = emap.get(&(op, sid)) {
        return Some(de);
    }
    let (b0, b1, curve, etol, sweep) = {
        let se = src.edges.get(edge)?;
        (
            se.bounds.0,
            se.bounds.1,
            se.curve,
            se.tolerance,
            se.arc_sweep,
        )
    };
    let dv0 = import_vertex(dst, src, b0, op, rec, vmap)?;
    let dv1 = import_vertex(dst, src, b1, op, rec, vmap)?;
    let de = dst.new_edge(rec, (dv0, dv1), Derivation::Created);
    if let Some(e) = dst.edges.get_mut(de) {
        e.tolerance = e.tolerance.max(etol);
        // Arc identity rides through the stitch: dropping the recorded
        // sweep reverted result-body arcs to ambiguous full carriers
        // (task 29: the assembled bicylinder mis-tessellated while the
        // operand pieces were exact).
        e.arc_sweep = sweep;
    }
    if let Some((sck, scsense)) = curve
        && let Some(c) = src.curves.get(sck).cloned()
    {
        let dc = dst.add_curve(c);
        if let Some(e) = dst.edges.get_mut(de) {
            e.curve = Some((dc, scsense));
        }
    }
    emap.insert((op, sid), de);
    Some(de)
}

/// Stitch kept faces (possibly curved) from both operands into one
/// valid solid: import each face's full topology, merge coincident
/// vertices, glue the coincident seam edges (the SSI curves, radial-1
/// after import) into the shared manifold seam, then build the two-
/// region partition. Built directly in the arenas (operators forbid the
/// intermediate non-solid states), then validated.
fn stitch_by_import(
    ia: &ImprintedOperand,
    ib: &ImprintedOperand,
    kept: &[KeptFace],
    walls: &[KeptFace],
    tol: f64,
) -> Result<Body, BoolFault> {
    use crate::entity::{EdgeKey, VertexKey};
    use crate::lineage::Derivation;
    use std::collections::BTreeMap;
    let vtol = tol.max(1e-7);

    let mut dst = Body::new();
    let inf = dst.infinite_region();
    let mut rec = dst.begin_op();
    let solid = dst.new_region(&mut rec, true, Derivation::Created);

    let mut vmap: BTreeMap<(Operand, u64), VertexKey> = BTreeMap::new();
    let mut emap: BTreeMap<(Operand, u64), EdgeKey> = BTreeMap::new();
    let mut faces = Vec::new();
    for k in kept {
        let src = match k.operand {
            Operand::A => &ia.body,
            Operand::B => &ib.body,
        };
        if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
            let loops = src
                .faces
                .get(k.face)
                .map(|f| f.loops.clone())
                .unwrap_or_default();
            for lk in loops {
                let mut edges = Vec::new();
                if let Some(entry) = src.loops.get(lk).and_then(|l| l.fin) {
                    let mut cur = entry;
                    while let Some(fin) = src.fins.get(cur) {
                        edges.push((fin.edge, fin.forward, imported_edge_midpoint(src, fin.edge)));
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                }
                eprintln!(
                    "  import {:?} face {:?} rev {} loop {lk:?}: {edges:?}",
                    k.operand, k.face, k.reversed
                );
            }
        }
        let f = import_face(
            &mut dst, src, k.face, k.operand, k.reversed, &mut rec, &mut vmap, &mut emap, inf,
            solid,
        )
        .ok_or(BoolFault::AssemblyFailed("import failed"))?;
        faces.push(f);
    }
    // The retained interface walls (non-regularized, item 29) import
    // through the same identity-carrying path; finalize reassigns their
    // sides to the solid cells they separate.
    let mut wall_faces = Vec::new();
    for k in walls {
        let src = match k.operand {
            Operand::A => &ia.body,
            Operand::B => &ib.body,
        };
        let f = import_face(
            &mut dst, src, k.face, k.operand, k.reversed, &mut rec, &mut vmap, &mut emap, inf,
            solid,
        )
        .ok_or(BoolFault::AssemblyFailed("wall import failed"))?;
        wall_faces.push(f);
    }

    finalize_imported_assembly(dst, rec, faces, wall_faces, inf, solid, vtol)
}

/// Disjoint-UNION combine (the broad-phase provably-separated case): the
/// operands' AABBs do not meet, so the union is exactly both lumps in one
/// body. Importing each ORIGINAL operand's faces verbatim -- no imprint,
/// no SSI, no seam glue -- keeps every face byte-faithful, so each lump
/// tessellates exactly as it did standalone and finalize derives one solid
/// cell per connected component. This is the curved-operand fix: the full
/// assembly's imprint/finalize machinery spawned spurious faces and merged
/// the disconnected curved components into a single broken region (a cone
/// lump's lateral split with one half tessellating to zero -> mass != mesh,
/// the dossier-57 disconnected-union-of-curved-bodies decline). Sound only
/// when the operands are PROVABLY non-touching (the caller guarantees it
/// from the AABB gap), so no missed-intersection guard is needed.
///
/// ORIENTATION (dossier 72): a mirror (improper isometry, det = -1) reflects
/// each surface FRAME, so the curved mass integrator's natural normal
/// `du x dv` (a pseudovector) flips to point INWARD while the geometry-based
/// mesh normal `(q - centre)` still points outward. The two then disagree on
/// which `sense` bit is outward, and the curved lump's analytic flux
/// SUBTRACTS (sphere mirror+union collapsed to V_a - V_b ~ 0). This is fixed
/// at the SOURCE -- the curved mass integrator now folds the surface-frame
/// handedness so its natural normal agrees with the mesh for a reflected
/// frame (see `integrate_curved_face`). The disjoint-union import therefore
/// stays byte-faithful (`reversed=false`): both mass and mesh read the same
/// outward orientation, the planar/curved baselines are untouched, and a
/// mirrored curved lump now ADDS.
fn combine_disjoint(a: &Body, b: &Body, tol: f64) -> Result<Body, BoolFault> {
    use crate::entity::{EdgeKey, VertexKey};
    use crate::lineage::Derivation;
    use std::collections::BTreeMap;
    let vtol = tol.max(1e-7);
    let mut dst = Body::new();
    let inf = dst.infinite_region();
    let mut rec = dst.begin_op();
    let solid = dst.new_region(&mut rec, true, Derivation::Created);
    let mut vmap: BTreeMap<(Operand, u64), VertexKey> = BTreeMap::new();
    let mut emap: BTreeMap<(Operand, u64), EdgeKey> = BTreeMap::new();
    let mut faces = Vec::new();
    for (operand, src) in [(Operand::A, a), (Operand::B, b)] {
        for sf in src.face_keys() {
            let f = import_face(
                &mut dst, src, sf, operand, false, &mut rec, &mut vmap, &mut emap, inf, solid,
            )
            .ok_or(BoolFault::AssemblyFailed("disjoint-union import failed"))?;
            faces.push(f);
        }
    }
    finalize_imported_assembly(dst, rec, faces, Vec::new(), inf, solid, vtol)
}

/// CONTAINMENT difference (`outer` MINUS a fully-enclosed `inner`): the
/// exact two-shell hollow assembly that bypasses the SSI/seam machinery.
/// When `inner` is strictly inside `outer` with NO boundary crossings --
/// the invariant of an inward whole-body offset shell (`hollow`) -- the
/// result is simply the outer boundary (outward) plus the inner boundary
/// REVERSED (its front facing the new cavity), stitched into one body.
/// `finalize_imported_assembly`'s enclosed-void-aware region partition
/// then groups the two disconnected shells correctly: the outer encloses
/// the exterior (-> infinite region) and the reversed inner encloses a
/// bounded interior (-> a new void region), giving a wall solid between
/// them.
///
/// This is the curved-shell counterpart of `combine_disjoint`: a clean,
/// verbatim face import (no re-intersection), so a box carrying
/// cylindrical/toroidal fillet faces shells exactly where the nested-
/// curved SSI difference would decline with an unassemblable seam.
///
/// DECLINE-never-WRONG: the caller must have ESTABLISHED containment (an
/// inward offset guarantees it); this routine additionally returns a body
/// that the caller gates on validate() + mass==mesh, and any coincident
/// vertices between the shells (which only arise if containment fails and
/// the shells touch) would make the glue produce a non-watertight body
/// that those gates reject.
pub(crate) fn combine_containment(outer: &Body, inner: &Body, tol: f64) -> Result<Body, BoolFault> {
    use crate::entity::{EdgeKey, VertexKey};
    use crate::lineage::Derivation;
    use std::collections::BTreeMap;
    let vtol = tol.max(1e-7);
    let mut dst = Body::new();
    let inf = dst.infinite_region();
    let mut rec = dst.begin_op();
    let solid = dst.new_region(&mut rec, true, Derivation::Created);
    let mut vmap: BTreeMap<(Operand, u64), VertexKey> = BTreeMap::new();
    let mut emap: BTreeMap<(Operand, u64), EdgeKey> = BTreeMap::new();
    let mut faces = Vec::new();
    // Outer shell: outward (front -> exterior).
    for sf in outer.face_keys() {
        let f = import_face(
            &mut dst, outer, sf, Operand::A, false, &mut rec, &mut vmap, &mut emap, inf, solid,
        )
        .ok_or(BoolFault::AssemblyFailed("containment: outer import failed"))?;
        faces.push(f);
    }
    // Inner shell: REVERSED, so its front faces the cavity it encloses.
    for sf in inner.face_keys() {
        let f = import_face(
            &mut dst, inner, sf, Operand::B, true, &mut rec, &mut vmap, &mut emap, inf, solid,
        )
        .ok_or(BoolFault::AssemblyFailed("containment: inner import failed"))?;
        faces.push(f);
    }
    // No GLUE: the outer and inner shells are disjoint and complete; the
    // vertex merge would collapse each shell's closed-arc seam vertex pair
    // (two distinct vertices at one point) into a single closed edge that
    // the green-slab mass integrator mis-reads as a full-revolution band.
    finalize_imported_assembly_glued(dst, rec, faces, Vec::new(), inf, solid, vtol, false)
}

/// Sheet-solid boolean (parity item 28, sheet-target MVP): trim the
/// open SHEET against the SOLID tool. Intersection keeps the part of
/// the sheet INSIDE the solid, Difference the part OUTSIDE (a tool
/// punching through the sheet interior leaves a holed sheet, riding
/// the interior-ring imprint). Union of mixed dimension is undefined
/// and declines, as do coincident/tangent contacts (general position
/// MVP, the same boundary the solid boolean started from). The result
/// is an open sheet body: faces double-sided in the void, free edges.
/// Sheet-SHEET booleans remain a follow-up (imprint exists; the kept-
/// side semantics need defining).
pub fn boolean_sheet_solid(
    sheet: &Body,
    tool: &Body,
    op: BoolOp,
    tol: f64,
) -> Result<Body, BoolFault> {
    use crate::query::BodyClass;
    use std::collections::BTreeMap;
    if sheet.body_class() != BodyClass::Sheet {
        return Err(BoolFault::AssemblyFailed(
            "boolean_sheet_solid: target must be a sheet body",
        ));
    }
    if tool.body_class() != BodyClass::Solid {
        return Err(BoolFault::AssemblyFailed(
            "boolean_sheet_solid: tool must be a solid body",
        ));
    }
    let want = match op {
        BoolOp::Intersection => FaceClass::InsideOther,
        BoolOp::Difference => FaceClass::OutsideOther,
        BoolOp::Union => {
            return Err(BoolFault::AssemblyFailed(
                "boolean_sheet_solid: union of mixed dimension is undefined",
            ));
        }
    };
    let (seams, mut faults) = seam_curves(sheet, tool, tol);
    let ia = imprint_operand(
        sheet,
        &seams,
        |s| (s.face_a, s.on_boundary_a),
        tol,
        &mut faults,
    );
    if let Some(f) = faults.into_iter().next() {
        return Err(f);
    }
    let ca = classify_faces(&ia.body, tool, tol);
    let mut kept = Vec::new();
    for (f, c) in &ca {
        if *c == want {
            kept.push(*f);
        } else if !matches!(c, FaceClass::InsideOther | FaceClass::OutsideOther) {
            return Err(BoolFault::AssemblyFailed(
                "boolean_sheet_solid: coincident/unclassifiable contact (declined)",
            ));
        }
    }
    if kept.is_empty() {
        return Err(BoolFault::AssemblyFailed(
            "boolean_sheet_solid: empty result",
        ));
    }
    // Import the kept sheet fragments, BOTH sides facing the void.
    let mut dst = Body::new();
    let inf = dst.infinite_region();
    let mut rec = dst.begin_op();
    let mut vmap = BTreeMap::new();
    let mut emap = BTreeMap::new();
    let mut faces = Vec::new();
    for f in kept {
        let nf = import_face(
            &mut dst,
            &ia.body,
            f,
            Operand::A,
            false,
            &mut rec,
            &mut vmap,
            &mut emap,
            inf,
            inf,
        )
        .ok_or(BoolFault::AssemblyFailed("import failed"))?;
        faces.push(nf);
    }
    finalize_imported_sheet(dst, rec, faces, inf, tol.max(1e-7))
}

/// Wire-solid boolean (dossier 57 Rung 4, the NOODLES dimension-by-
/// dimension lowest rung): trim a WIRE body against a SOLID.
/// Intersection keeps the wire portions INSIDE the solid, Difference
/// the portions OUTSIDE; Union of mixed dimension stays undefined and
/// declines (a kept embedded wire is the rung-5 non-regularized
/// ladder). Straight wire edges against planar solids (MVP): each
/// segment splits at its boundary crossings (segment-plane
/// intersections, point-in-face tested), sub-segments classify by the
/// winding number at their midpoints, and the kept pieces rebuild as
/// one wire body.
pub fn boolean_wire_solid(
    wire: &Body,
    solid: &Body,
    op: BoolOp,
    tol: f64,
) -> Result<Body, BoolFault> {
    use crate::query::BodyClass;
    let _ = tol;
    if wire.body_class() != BodyClass::Wire {
        return Err(BoolFault::AssemblyFailed(
            "boolean_wire_solid: target must be a wire body",
        ));
    }
    if solid.body_class() != BodyClass::Solid {
        return Err(BoolFault::AssemblyFailed(
            "boolean_wire_solid: tool must be a solid body",
        ));
    }
    let want_inside = match op {
        BoolOp::Intersection => true,
        BoolOp::Difference => false,
        BoolOp::Union => {
            return Err(BoolFault::AssemblyFailed(
                "boolean_wire_solid: union of mixed dimension is undefined (rung 5)",
            ));
        }
    };
    let mut kept: Vec<(keel_math::vec::Vec3, keel_math::vec::Vec3)> = Vec::new();
    for (_, e) in wire.edges.iter() {
        // Straight segments only (MVP); curved wire edges decline.
        if let Some((ck, _)) = e.curve
            && !matches!(wire.curves.get(ck), Some(keel_geom::curve::Curve3::Line(_)))
        {
            return Err(BoolFault::AssemblyFailed(
                "boolean_wire_solid: curved wire edge (follow-up)",
            ));
        }
        let (a, b) = e.bounds;
        let (Some(pa), Some(pb)) = (
            wire.vertices.get(a).map(|v| v.point),
            wire.vertices.get(b).map(|v| v.point),
        ) else {
            continue;
        };
        let d = pb - pa;
        // Crossing parameters: the segment against every solid face
        // (plane intersection + point-in-face containment).
        let mut ts = vec![0.0f64, 1.0];
        for f in solid.face_keys() {
            let Some(Surface3::Plane(pl)) = solid.face_surface3(f) else {
                return Err(BoolFault::AssemblyFailed(
                    "boolean_wire_solid: curved solid face (follow-up)",
                ));
            };
            let (n, o) = (pl.frame.z, pl.frame.origin);
            let denom = d.dot(n);
            if denom.abs() < 1e-12 {
                continue;
            }
            let t = (o - pa).dot(n) / denom;
            if t <= 1e-9 || t >= 1.0 - 1e-9 {
                continue;
            }
            let p = pa + d * t;
            // Containment in the face: 2D winding in the face plane.
            let ring = solid.face_outer_loop_points(f);
            if ring.len() < 3 {
                continue;
            }
            let bx = (ring[1] - ring[0])
                .try_normalize()
                .unwrap_or(keel_math::vec::Vec3::new(1.0, 0.0, 0.0));
            let by = n.cross(bx);
            let to2d = |q: keel_math::vec::Vec3| ((q - ring[0]).dot(bx), (q - ring[0]).dot(by));
            let poly: Vec<(f64, f64)> = ring.iter().map(|&q| to2d(q)).collect();
            if winding_nonzero(&poly, to2d(p)) {
                ts.push(t);
            }
        }
        ts.sort_by(f64::total_cmp);
        for w in ts.windows(2) {
            let (t0, t1) = (w[0], w[1]);
            if t1 - t0 <= 1e-9 {
                continue;
            }
            let mid = pa + d * (0.5 * (t0 + t1));
            let inside = solid.generalized_winding_number(mid) > 0.5;
            if inside == want_inside {
                kept.push((pa + d * t0, pa + d * t1));
            }
        }
    }
    if kept.is_empty() {
        return Err(BoolFault::AssemblyFailed(
            "boolean_wire_solid: empty result",
        ));
    }
    let mut out = Body::new();
    for (p0, p1) in kept {
        out.wire(p0, p1).map_err(BoolFault::Topo)?;
    }
    if out.validate().is_err() {
        return Err(BoolFault::AssemblyFailed(
            "boolean_wire_solid: result invalid",
        ));
    }
    Ok(out)
}

/// Sheet-sheet boolean (dossier 57 Rung 3, dossier 39 sec 1.2): the 2D
/// arrangement of two COPLANAR planar sheets. The overlap's interior
/// boundary imprints onto each sheet (the same machinery as the
/// solid-solid coincident pre-pass), fragments classify by a 2D
/// point-in-polygon test against the other sheet, selection follows the
/// dimension table (union = all of A + B-outside; intersection =
/// A-inside; difference = A-outside), and the kept fragments knit into
/// one sheet body (shared seam edges glue radial-2). CROSSING
/// (non-coplanar) sheets intersect in a wire and DECLINE here (the
/// mixed-dimension rung-4/5 ladder).
pub fn boolean_sheet_sheet(a: &Body, b: &Body, op: BoolOp, tol: f64) -> Result<Body, BoolFault> {
    use crate::query::BodyClass;
    if a.body_class() != BodyClass::Sheet || b.body_class() != BodyClass::Sheet {
        return Err(BoolFault::AssemblyFailed(
            "boolean_sheet_sheet: both operands must be sheet bodies",
        ));
    }
    let one_planar_face = |s: &Body| -> Option<(FaceKey, keel_math::vec::Vec3)> {
        let faces = s.face_keys();
        let [f] = faces[..] else { return None };
        match s.face_surface3(f) {
            Some(Surface3::Plane(p)) => Some((f, p.frame.z)),
            _ => None,
        }
    };
    let (Some((fa, na)), Some((fb, nb))) = (one_planar_face(a), one_planar_face(b)) else {
        return Err(BoolFault::AssemblyFailed(
            "boolean_sheet_sheet: single planar face per sheet (MVP)",
        ));
    };
    let pa = a.face_outer_loop_points(fa);
    let pb = b.face_outer_loop_points(fb);
    if pa.len() < 3 || pb.len() < 3 {
        return Err(BoolFault::AssemblyFailed("boolean_sheet_sheet: degenerate"));
    }
    // Coplanarity: same carrier within tol (parallel normals, zero
    // offset). Crossing sheets decline.
    let n = na;
    if na.cross(nb).norm() > 1e-9 || ((pb[0] - pa[0]).dot(n)).abs() > tol.max(1e-9) {
        return Err(BoolFault::AssemblyFailed(
            "boolean_sheet_sheet: non-coplanar sheets (follow-up)",
        ));
    }
    // 2D frame on the shared plane for point-in-polygon tests.
    let bx = (pa[1] - pa[0])
        .try_normalize()
        .ok_or(BoolFault::AssemblyFailed("boolean_sheet_sheet: degenerate"))?;
    let by = n.cross(bx);
    let origin = pa[0];
    let to2d = |p: keel_math::vec::Vec3| -> (f64, f64) {
        let d = p - origin;
        (d.dot(bx), d.dot(by))
    };
    let poly_a2: Vec<(f64, f64)> = pa.iter().map(|&p| to2d(p)).collect();
    let poly_b2: Vec<(f64, f64)> = pb.iter().map(|&p| to2d(p)).collect();
    // Imprint the overlap's interior boundary onto an operand, then
    // keep fragments by their interior point's containment in the
    // OTHER sheet (None keeps everything).
    let fragments = |src: &Body,
                     face: FaceKey,
                     own: &[keel_math::vec::Vec3],
                     other: &[keel_math::vec::Vec3],
                     other2: &[(f64, f64)],
                     keep_inside: Option<bool>|
     -> (Body, Vec<FaceKey>) {
        let mut work = src.clone();
        for (s, e) in crate::coincident::overlap_interior_segments(own, other, n) {
            // The open-imprint contract is a PRE-BOUNDED curve (t in
            // [0,1] = the segment); Line3 normalizes its direction, so
            // a degree-1 NURBS carries the exact segment.
            if let Ok(seg) =
                keel_geom::nurbs_curve::NurbsCurve::new(1, vec![0., 0., 1., 1.], vec![s, e], None)
            {
                let _ = work.imprint_open_curve(face, &keel_geom::curve::Curve3::Nurbs(seg), tol);
            }
        }
        let mut kept = Vec::new();
        for f in work.face_keys() {
            let Some(q) = work.face_interior_point(f) else {
                continue;
            };
            let inside = winding_nonzero(other2, to2d(q));
            if keep_inside.is_none_or(|want| want == inside) {
                kept.push(f);
            }
        }
        (work, kept)
    };
    let mut sources: Vec<(Body, Vec<FaceKey>, Operand)> = Vec::new();
    match op {
        BoolOp::Union => {
            // All of A, plus B outside A (one copy of the overlap).
            let (wa, ka) = fragments(a, fa, &pa, &pb, &poly_b2, None);
            let (wb, kb) = fragments(b, fb, &pb, &pa, &poly_a2, Some(false));
            sources.push((wa, ka, Operand::A));
            sources.push((wb, kb, Operand::B));
        }
        BoolOp::Intersection => {
            let (wa, ka) = fragments(a, fa, &pa, &pb, &poly_b2, Some(true));
            sources.push((wa, ka, Operand::A));
        }
        BoolOp::Difference => {
            let (wa, ka) = fragments(a, fa, &pa, &pb, &poly_b2, Some(false));
            sources.push((wa, ka, Operand::A));
        }
    }
    if sources.iter().all(|(_, k, _)| k.is_empty()) {
        return Err(BoolFault::AssemblyFailed(
            "boolean_sheet_sheet: empty result",
        ));
    }
    // Import the kept fragments and finalize as a SHEET (free edges
    // are the nature of an open lamina; the solid closure invariant
    // does not apply).
    use std::collections::BTreeMap;
    let mut dst = Body::new();
    let inf = dst.infinite_region();
    let mut rec = dst.begin_op();
    let mut vmap = BTreeMap::new();
    let mut emap = BTreeMap::new();
    let mut faces = Vec::new();
    for (src_body, kept, tag) in &sources {
        for &f in kept {
            let nf = import_face(
                &mut dst, src_body, f, *tag, false, &mut rec, &mut vmap, &mut emap, inf, inf,
            )
            .ok_or(BoolFault::AssemblyFailed("boolean_sheet_sheet: import"))?;
            faces.push(nf);
        }
    }
    finalize_imported_sheet(dst, rec, faces, inf, tol.max(1e-7))
}

/// Partition a SOLID by a SHEET into a cellular solid (item 29 Rung 2,
/// dossier 57: "sheet-as-knife"): the part of the sheet INSIDE the
/// solid becomes a DOUBLE-SIDED interior wall splitting the region it
/// crosses into two solid cells; the outer boundary is unchanged. The
/// wall is the sheet-solid INTERSECTION (the item-28 trim), the solid
/// is imprinted along the same seams, and the cellular finalize (the
/// Weiler sector walk of Rung 1) extracts the regions. The sheet must
/// cut clean through (a wall whose boundary ends in the interior would
/// leave one cell, declined by the two-solid-cells gate below).
pub fn partition_by_sheet(solid: &Body, sheet: &Body, tol: f64) -> Result<Body, BoolFault> {
    use crate::query::BodyClass;
    if solid.body_class() != BodyClass::Solid {
        return Err(BoolFault::AssemblyFailed(
            "partition_by_sheet: target must be a solid body",
        ));
    }
    // The interior wall: the sheet trimmed to the solid (item 28).
    let wall = boolean_sheet_solid(sheet, solid, BoolOp::Intersection, tol)?;
    // Imprint the solid along the shared seams.
    let (seams, mut faults) = seam_curves(solid, sheet, tol);
    if seams.is_empty() {
        return Err(BoolFault::AssemblyFailed(
            "partition_by_sheet: sheet does not cross the solid boundary",
        ));
    }
    let ia = imprint_operand(
        solid,
        &seams,
        |s| (s.face_a, s.on_boundary_a),
        tol,
        &mut faults,
    );
    if let Some(f) = faults.into_iter().next() {
        return Err(f);
    }
    // Keep EVERY solid fragment (nothing is removed) plus every wall
    // face as a double-sided interior cell boundary.
    let kept: Vec<KeptFace> = ia
        .body
        .face_keys()
        .into_iter()
        .map(|f| KeptFace {
            operand: Operand::A,
            face: f,
            reversed: false,
        })
        .collect();
    let walls: Vec<KeptFace> = wall
        .face_keys()
        .into_iter()
        .map(|f| KeptFace {
            operand: Operand::B,
            face: f,
            reversed: false,
        })
        .collect();
    let ib = ImprintedOperand {
        body: wall,
        seam_edges: Vec::new(),
    };
    let body = stitch_by_import(&ia, &ib, &kept, &walls, tol)?;
    // Honesty gates: the outer boundary is the input's material
    // unchanged, so mass == mesh == the input volume; and the knife
    // must actually have split the material (two or more solid cells).
    let solid_cells = body.regions.iter().filter(|(_, r)| r.solid).count();
    if solid_cells < 2 {
        return Err(BoolFault::AssemblyFailed(
            "partition_by_sheet: wall did not split the region (declined)",
        ));
    }
    let Ok(m) = body.mass_properties() else {
        return Err(BoolFault::AssemblyFailed(
            "partition_by_sheet: mass properties failed",
        ));
    };
    let mv = body.mesh_volume();
    if !(m.volume.is_finite() && (m.volume - mv).abs() <= 1e-3 * (1.0 + m.volume.abs())) {
        return Err(BoolFault::AssemblyFailed(
            "partition_by_sheet: self-inconsistent result (mass != mesh)",
        ));
    }
    Ok(body)
}

/// Sheet-result finalize (item 28): merge + glue, then one shell per
/// connected component holding BOTH face sides in the void (a sheet
/// borders the ambient void on both sides). No solid region, no
/// closure invariant: free edges are the nature of an open sheet.
fn finalize_imported_sheet(
    mut dst: Body,
    mut rec: crate::body::OpRecorder,
    faces: Vec<FaceKey>,
    inf: crate::entity::RegionKey,
    vtol: f64,
) -> Result<Body, BoolFault> {
    use crate::entity::Side;
    use crate::lineage::Derivation;
    merge_and_glue_imported(&mut dst, &mut rec, vtol);
    for comp in connected_face_components(&dst, &faces) {
        let shell = dst.new_shell(&mut rec, inf, Derivation::Created);
        if let Some(s) = dst.shells.get_mut(shell) {
            s.faces = comp
                .iter()
                .flat_map(|&f| [(f, Side::Front), (f, Side::Back)])
                .collect();
        }
        if let Some(r) = dst.regions.get_mut(inf) {
            r.shells.push(shell);
        }
        for &f in &comp {
            if let Some(face) = dst.faces.get_mut(f) {
                face.front_region = inf;
                face.back_region = inf;
            }
        }
    }
    let _ = rec.finish();
    if dst.validate().is_err() {
        return Err(BoolFault::AssemblyFailed("stitched sheet invalid"));
    }
    Ok(dst)
}

/// Merge coincident vertices and glue dangling (radial-1) edges into
/// radial pairs -- the representation-independent half of import-and-
/// glue, shared by the solid finalize below, the SHEET finalize
/// (item 28), and Body::heal (item 130).
pub(crate) fn merge_and_glue_imported(
    dst: &mut Body,
    rec: &mut crate::body::OpRecorder,
    vtol: f64,
) {
    use crate::entity::{EdgeKey, VertexKey};
    // Merge coincident vertices (the operands' independent seam vertices
    // along the shared SSI curve land at the same point).
    let vkeys: Vec<VertexKey> = dst.vertices.iter().map(|(k, _)| k).collect();
    let mut alive = vec![true; vkeys.len()];
    for i in 0..vkeys.len() {
        if !alive[i] {
            continue;
        }
        let pi = match dst.vertices.get(vkeys[i]) {
            Some(v) => v.point,
            None => continue,
        };
        for j in (i + 1)..vkeys.len() {
            if !alive[j] {
                continue;
            }
            let pj = match dst.vertices.get(vkeys[j]) {
                Some(v) => v.point,
                None => continue,
            };
            if (pi - pj).norm() <= vtol {
                // Repoint edges from vkeys[j] to vkeys[i].
                let eks: Vec<EdgeKey> = dst.edges.iter().map(|(k, _)| k).collect();
                for ek in eks {
                    if let Some(e) = dst.edges.get_mut(ek) {
                        if e.bounds.0 == vkeys[j] {
                            e.bounds.0 = vkeys[i];
                        }
                        if e.bounds.1 == vkeys[j] {
                            e.bounds.1 = vkeys[i];
                        }
                    }
                }
                if let Some(id) = dst.vertices.get(vkeys[j]).map(|v| v.id) {
                    dst.unregister(rec, id);
                }
                dst.vertices.remove(vkeys[j]);
                alive[j] = false;
            }
        }
    }

    // Align subdivisions across operands: the two imprints may split
    // the SAME seam conic at different wrap points (task 29: one
    // operand's half arc through its wrap vertex faces the other's two
    // quarter arcs, and no bounds ever match). A merged vertex lying in
    // the INTERIOR of a dangling conic arc splits that arc, so the glue
    // below sees identical subdivisions on both sides.
    //
    // The SAME mismatch arises on STRAIGHT seam edges at a T-junction
    // (dossier 73b): one operand carries the tool's wall-cut on a body face
    // as ONE edge A->C while the other operand, having split that wall at a
    // crossing seam, carries A->B + B->C. Their bounds never match and the
    // line glue below leaves both dangling. A merged vertex lying strictly
    // interior to a dangling straight edge splits it the same way, so the
    // two subdivisions agree and pair. (`split_edge_raw` carries the parent
    // line curve to both children; no arc_sweep applies to a line.)
    loop {
        // (rel, s) carry the conic split params; rel = f64::NAN flags the
        // straight-edge case (no arc_sweep to set on the children).
        let mut job: Option<(EdgeKey, VertexKey, f64, f64)> = None;
        'scan: for (ek, e) in dst.edges.iter() {
            if e.radial.len() != 1 {
                continue;
            }
            for (vk, v) in dst.vertices.iter() {
                if vk == e.bounds.0 || vk == e.bounds.1 {
                    continue;
                }
                if let Some((rel, s)) = conic_arc_split_rel(dst, ek, v.point, vtol) {
                    job = Some((ek, vk, rel, s));
                    break 'scan;
                }
                if straight_edge_contains_interior(dst, ek, v.point, vtol) {
                    job = Some((ek, vk, f64::NAN, 0.0));
                    break 'scan;
                }
            }
        }
        let Some((ek, vk, rel, s)) = job else { break };
        let Some(p) = dst.vertices.get(vk).map(|v| v.point) else {
            break;
        };
        let Ok(out) = dst.split_edge_raw(ek, p) else {
            break;
        };
        if !rel.is_nan() {
            dst.set_edge_arc_sweep(out.edge_a, rel);
            dst.set_edge_arc_sweep(out.edge_b, s - rel);
        }
        // Weld the split's fresh vertex onto the pre-existing one.
        let w = out.vertex;
        if w != vk {
            let eks: Vec<EdgeKey> = dst.edges.iter().map(|(k, _)| k).collect();
            for e2 in eks {
                if let Some(e) = dst.edges.get_mut(e2) {
                    if e.bounds.0 == w {
                        e.bounds.0 = vk;
                    }
                    if e.bounds.1 == w {
                        e.bounds.1 = vk;
                    }
                }
            }
            let wfin = dst.vertices.get(w).and_then(|v| v.fin);
            if let Some(v) = dst.vertices.get_mut(vk)
                && v.fin.is_none()
            {
                v.fin = wfin;
            }
            if let Some(id) = dst.vertices.get(w).map(|v| v.id) {
                dst.unregister(rec, id);
            }
            dst.vertices.remove(w);
        }
    }

    // Glue coincident seam edges: a DANGLING (radial-1) edge whose
    // bounds coincide (post-merge) with another edge's joins that
    // edge's radial cycle. The manifold seam glues pairs of dangling
    // copies; a non-manifold junction (an interior partition wall
    // meeting the outer walls, item 29 / R1) glues a dangling copy into
    // an edge that is ALREADY radial-2, forming the radial-3+ cycle.
    // Only radial-1 edges are ever absorbed: two coincident manifold
    // edges of separate closed shells (kissing solids) stay separate.
    let dangling: Vec<EdgeKey> = dst
        .edges
        .iter()
        .filter(|(_, e)| e.radial.len() == 1)
        .map(|(k, _)| k)
        .collect();
    for j in dangling {
        let Some(bj) = dst.edges.get(j).map(|e| e.bounds) else {
            continue; // already absorbed into an earlier target
        };
        // The dangling list is a snapshot: an entry that already RECEIVED
        // a glued partner is radial-2 by now and must not absorb into a
        // third edge (task 29: that pile-up built a radial-6 ellipse).
        if dst.edges.get(j).is_none_or(|e| e.radial.len() != 1) {
            continue;
        }
        // Bounds alone cannot disambiguate when several distinct arcs
        // share both endpoints (task 29: the four Steinmetz half-
        // ellipses all run pole to pole), so among bound-matched
        // candidates pick the one whose geometric arc midpoint
        // coincides. A single candidate glues unconditionally (the
        // pre-existing manifold-seam behavior).
        let mj = imported_edge_midpoint(dst, j);
        let mut cands: Vec<(EdgeKey, bool, f64)> = Vec::new();
        for (k, e) in dst.edges.iter() {
            if k == j {
                continue;
            }
            let rev = if e.bounds == bj {
                false
            } else if e.bounds == (bj.1, bj.0) {
                true
            } else {
                continue;
            };
            let d = match (mj, imported_edge_midpoint(dst, k)) {
                (Some(a), Some(b)) => (a - b).norm(),
                _ => 0.0,
            };
            cands.push((k, rev, d));
        }
        let target = if cands.len() == 1 {
            Some((cands[0].0, cands[0].1))
        } else {
            cands
                .into_iter()
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(k, r, _)| (k, r))
        };
        let Some((i, reversed)) = target else {
            continue;
        };
        // Move edge j's fin onto edge i; drop edge j.
        let moved: Vec<_> = dst
            .edges
            .get(j)
            .map(|e| e.radial.clone())
            .unwrap_or_default();
        for fk in &moved {
            if let Some(f) = dst.fins.get_mut(*fk) {
                f.edge = i;
                if reversed {
                    f.forward = !f.forward;
                }
            }
        }
        if let Some(e) = dst.edges.get_mut(i) {
            e.radial.extend(moved);
        }
        if let Some(id) = dst.edges.get(j).map(|e| e.id) {
            dst.unregister(rec, id);
        }
        dst.edges.remove(j);
    }
}

/// Geometric midpoint of an imported edge's arc, honoring the recorded
/// arc_sweep (bounds.0-relative, matching the massprops convention).
/// Straight or carrier-less edges fall back to the chord midpoint,
/// which is identical for any pair of bound-matched candidates.
fn imported_edge_midpoint(dst: &Body, ek: crate::entity::EdgeKey) -> Option<keel_math::vec::Vec3> {
    use keel_geom::curve::Curve3;
    let tau = core::f64::consts::TAU;
    let e = dst.edges.get(ek)?;
    let p0 = dst.vertices.get(e.bounds.0)?.point;
    let p1 = dst.vertices.get(e.bounds.1)?.point;
    let wrap = |d: f64| {
        let mut d = d;
        while d <= -core::f64::consts::PI {
            d += tau;
        }
        while d > core::f64::consts::PI {
            d -= tau;
        }
        d
    };
    match e.curve.and_then(|(ck, _)| dst.curves.get(ck)) {
        Some(Curve3::Circle(c)) => {
            let ang = |p: keel_math::vec::Vec3| {
                (p - c.center)
                    .dot(c.y_axis)
                    .atan2((p - c.center).dot(c.x_axis))
            };
            let a0 = ang(p0);
            let s = e.arc_sweep.unwrap_or(if e.bounds.0 == e.bounds.1 {
                tau
            } else {
                wrap(ang(p1) - a0)
            });
            Some(c.point(a0 + 0.5 * s))
        }
        Some(Curve3::Ellipse(el)) => {
            let ang = |p: keel_math::vec::Vec3| {
                let w = p - el.center;
                (w.dot(el.y_axis) / el.b).atan2(w.dot(el.x_axis) / el.a)
            };
            let a0 = ang(p0);
            let s = e.arc_sweep.unwrap_or(if e.bounds.0 == e.bounds.1 {
                tau
            } else {
                wrap(ang(p1) - a0)
            });
            Some(el.point(a0 + 0.5 * s))
        }
        _ => Some((p0 + p1) * 0.5),
    }
}

/// True when `p` lies strictly INTERIOR to edge `ek`'s straight chord
/// within `vtol` (not at either endpoint). Used by the subdivision-alignment
/// pass to split a dangling STRAIGHT seam edge at a crossing vertex the OTHER
/// operand already carries, so a T-junction's `A->C` edge subdivides to match
/// the facing `A->B + B->C`. Restricted to edges with no conic carrier (the
/// conic case is handled by `conic_arc_split_rel`); a line carrier or none.
fn straight_edge_contains_interior(
    dst: &Body,
    ek: crate::entity::EdgeKey,
    p: keel_math::vec::Vec3,
    vtol: f64,
) -> bool {
    use keel_geom::curve::Curve3;
    let Some(e) = dst.edges.get(ek) else {
        return false;
    };
    // Skip conic carriers (handled by the arc path); allow Line or none.
    if matches!(
        e.curve.and_then(|(ck, _)| dst.curves.get(ck)),
        Some(Curve3::Circle(_)) | Some(Curve3::Ellipse(_))
    ) {
        return false;
    }
    let (Some(a), Some(b)) = (
        dst.vertices.get(e.bounds.0).map(|v| v.point),
        dst.vertices.get(e.bounds.1).map(|v| v.point),
    ) else {
        return false;
    };
    if (p - a).norm() <= vtol || (p - b).norm() <= vtol {
        return false; // an endpoint, not an interior split
    }
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 <= vtol * vtol {
        return false;
    }
    let t = (p - a).dot(ab) / len2;
    if t <= 1e-9 || t >= 1.0 - 1e-9 {
        return false;
    }
    (a + ab * t - p).norm() <= vtol
}

/// If `p` lies geometrically in the INTERIOR of edge `ek`'s declared
/// conic arc, the signed angular offset of `p` from bounds.0 (in the
/// sweep direction) together with the full sweep: the split data the
/// subdivision-alignment pass needs. None for endpoints, off-carrier
/// points, or non-conic edges.
fn conic_arc_split_rel(
    dst: &Body,
    ek: crate::entity::EdgeKey,
    p: keel_math::vec::Vec3,
    vtol: f64,
) -> Option<(f64, f64)> {
    use keel_geom::curve::Curve3;
    let tau = core::f64::consts::TAU;
    let e = dst.edges.get(ek)?;
    let p0 = dst.vertices.get(e.bounds.0)?.point;
    let p1 = dst.vertices.get(e.bounds.1)?.point;
    let wrap = |d: f64| {
        let mut d = d;
        while d <= -core::f64::consts::PI {
            d += tau;
        }
        while d > core::f64::consts::PI {
            d -= tau;
        }
        d
    };
    let (a0, a1, av, on) = match e.curve.and_then(|(ck, _)| dst.curves.get(ck))? {
        Curve3::Circle(c) => {
            let ang = |q: keel_math::vec::Vec3| {
                (q - c.center)
                    .dot(c.y_axis)
                    .atan2((q - c.center).dot(c.x_axis))
            };
            let av = ang(p);
            (ang(p0), ang(p1), av, (c.point(av) - p).norm() <= vtol)
        }
        Curve3::Ellipse(el) => {
            let ang = |q: keel_math::vec::Vec3| {
                let w = q - el.center;
                (w.dot(el.y_axis) / el.b).atan2(w.dot(el.x_axis) / el.a)
            };
            let av = ang(p);
            (ang(p0), ang(p1), av, (el.point(av) - p).norm() <= vtol)
        }
        _ => return None,
    };
    if !on {
        return None;
    }
    let s = e.arc_sweep.unwrap_or(if e.bounds.0 == e.bounds.1 {
        tau
    } else {
        wrap(a1 - a0)
    });
    let rel = if s >= 0.0 {
        (av - a0).rem_euclid(tau)
    } else {
        -((a0 - av).rem_euclid(tau))
    };
    let eps = 1e-7;
    if rel.abs() > eps && rel.abs() < s.abs() - eps {
        Some((rel, s))
    } else {
        None
    }
}

/// The shared back half of import-and-glue assembly (used by the boolean
/// stitch AND by `knit`): merge coincident vertices, glue coincident free /
/// dangling edges into radial pairs, assert the planar shell-closure
/// invariant, partition into solid / void / infinite regions (enclosed-void
/// aware), then validate. Takes the body with its kept faces already
/// imported (front -> `inf`, back -> `solid`) and the live op recorder.
pub(crate) fn finalize_imported_assembly(
    dst: Body,
    rec: crate::body::OpRecorder,
    faces: Vec<FaceKey>,
    walls: Vec<FaceKey>,
    inf: crate::entity::RegionKey,
    solid: crate::entity::RegionKey,
    vtol: f64,
) -> Result<Body, BoolFault> {
    finalize_imported_assembly_glued(dst, rec, faces, walls, inf, solid, vtol, true)
}

/// As [`finalize_imported_assembly`], but with explicit control over the
/// coincident-vertex/edge GLUE pass. Pass `glue == false` when the
/// imported shells are already complete and watertight and must NOT be
/// welded to each other -- the nested two-shell hollow of
/// `combine_containment`, whose outer and inner shells are disjoint and
/// each carries its own closed-arc SEAM vertex pair (two distinct
/// vertices at one point). The global merge would collapse those seam
/// pairs into a single closed edge, which the green-slab mass integrator
/// then reads as a spurious full-revolution band (winding +-1) and
/// declines. Skipping the merge preserves each shell's exact topology.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_imported_assembly_glued(
    mut dst: Body,
    mut rec: crate::body::OpRecorder,
    faces: Vec<FaceKey>,
    walls: Vec<FaceKey>,
    inf: crate::entity::RegionKey,
    solid: crate::entity::RegionKey,
    vtol: f64,
    glue: bool,
) -> Result<Body, BoolFault> {
    use crate::entity::Side;
    use crate::lineage::Derivation;
    if glue {
        merge_and_glue_imported(&mut dst, &mut rec, vtol);
    }

    if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
        for (vk, v) in dst.vertices.iter() {
            eprintln!("  stitched vertex {vk:?} {:?}", v.point);
        }
        for (fk, f) in dst.faces.iter() {
            for &lk in &f.loops {
                let mut edges = Vec::new();
                if let Some(entry) = dst.loops.get(lk).and_then(|l| l.fin) {
                    let mut cur = entry;
                    while let Some(fin) = dst.fins.get(cur) {
                        edges.push((fin.edge, fin.forward));
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                }
                eprintln!("  stitched face {fk:?} loop {lk:?} fins {edges:?}");
            }
        }
        for (ek, e) in dst.edges.iter() {
            let kind = match e.curve.and_then(|(ck, _)| dst.curves.get(ck)) {
                Some(keel_geom::curve::Curve3::Circle(_)) => "circle",
                Some(keel_geom::curve::Curve3::Ellipse(_)) => "ellipse",
                Some(keel_geom::curve::Curve3::Line(_)) => "line",
                Some(keel_geom::curve::Curve3::Nurbs(_)) => "nurbs",
                None => "none",
            };
            eprintln!(
                "  stitched edge {ek:?} {kind} sweep {:?} radial {} {:?} -> {:?} mid {:?}",
                e.arc_sweep,
                e.radial.len(),
                dst.vertices.get(e.bounds.0).map(|v| v.point),
                dst.vertices.get(e.bounds.1).map(|v| v.point),
                imported_edge_midpoint(&dst, ek),
            );
        }
    }

    // Shell-closure invariant (dossier 47 Q1 / synthesis step 5): every kept
    // coedge must land in a radial pairing (or a complete radial cycle). After
    // the seam glue above, an edge still carrying a lone fin (radial.len() == 1)
    // is an UNMATCHED coedge -- the exact signature of the silently dropped face
    // the dossier names as a hard error, never a face to omit. validate()'s
    // check_radial_cycles only asserts each fin sits in exactly one cycle, so a
    // dangling edge passes it untouched; this is the orthogonal topological
    // backstop (complementary to the geometric mass==mesh gate of Addendum 102,
    // each catching a class the other misses: a dropped face can survive volume
    // agreement under symmetric error cancellation but not coedge pairing).
    //
    // SCOPE: all-planar results only. That is where the dossier's drop bug lives
    // (the asymmetric-chamfer / L-union oblique-cut class is all-planar) and the
    // polygon-soup path being retired. The curved path legitimately carries
    // radial-1 edges -- periodic-surface seams and degenerate closed circular
    // rims (e.g. a cylinder cap seam represented as a closed edge with a single
    // wrapping fin) -- so a blanket radial check over-fires on correct curved
    // bodies (the plug, the blind hole). A correct closed PLANAR solid never has
    // a radial-1 edge: its polygon boundaries have distinct vertices and every
    // edge is shared by exactly two (or, non-manifold, a full cycle of) faces.
    let all_planar = dst
        .face_keys()
        .iter()
        .all(|&f| matches!(dst.face_surface3(f), Some(Surface3::Plane(_))));
    if all_planar && dst.edges.iter().any(|(_, e)| e.radial.len() < 2) {
        if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
            for (ek, e) in dst.edges.iter().filter(|(_, e)| e.radial.len() < 2) {
                let (p0, p1) = (
                    dst.vertices.get(e.bounds.0).map(|v| v.point),
                    dst.vertices.get(e.bounds.1).map(|v| v.point),
                );
                eprintln!(
                    "  unmatched edge {ek:?} radial {} {p0:?} -> {p1:?}",
                    e.radial.len()
                );
            }
        }
        return Err(BoolFault::AssemblyFailed(
            "unmatched coedge: shell-closure invariant violated",
        ));
    }

    // CELLULAR finalize (item 29 Rung 1, dossier 57): retained interface
    // walls partition the material into multiple solid cells; regions
    // come from the Weiler face-side sector walk, not the manifold
    // two-shell-per-component rule.
    if !walls.is_empty() {
        return finalize_cellular(dst, rec, &faces, &walls, inf, solid);
    }

    // Region partition, detecting ENCLOSED VOIDS (dossier 50: a closed inner
    // shell encloses a void = a third region). Group the kept faces into
    // connected boundary components (shared-edge connectivity); each is a
    // closed shell whose BACK side bounds solid material and whose FRONT side
    // bounds either the unbounded exterior (the infinite region) or an
    // enclosed cavity (a new void region). The sign of the component's
    // front-oriented signed volume distinguishes them: >= 0 means the front
    // normals enclose the component from OUTSIDE (front faces the exterior ->
    // infinite); < 0 means they enclose a bounded interior (front faces a void
    // -> a new non-solid region). The common single-solid boolean is one
    // component with v_front >= 0, recovering exactly the prior solid+infinite
    // partition. (Disconnected solid bodies sharing one solid region is a
    // documented simplification; separate solid regions per disconnected wall
    // is a follow-up.)
    let mut exterior_seen = false;
    for comp in connected_face_components(&dst, &faces) {
        // The component's own Euler characteristic decides the genus of
        // its shell pair (kfmrh convention: both shells carry it; counts()
        // sums shells / 2). A through-hole difference (slab minus an
        // interior block) yields a genus-1 component whose freshly made
        // shells would otherwise claim genus 0 and fail the Euler check.
        // chi = V - E + sum_f(2 - loops(f)) = V - E + F - rings; for a
        // closed orientable shell genus = (2 - chi) / 2.
        let comp_genus: u32 = {
            use std::collections::BTreeSet;
            let mut vs: BTreeSet<crate::entity::VertexKey> = BTreeSet::new();
            let mut es: BTreeSet<crate::entity::EdgeKey> = BTreeSet::new();
            let mut rings = 0i64;
            for &f in &comp {
                let loops = dst
                    .faces
                    .get(f)
                    .map(|x| x.loops.clone())
                    .unwrap_or_default();
                for lk in loops {
                    if dst.loops.get(lk).map(|l| l.kind) == Some(crate::entity::LoopKind::Inner) {
                        rings += 1;
                    }
                    let Some(entry) = dst.loops.get(lk).and_then(|l| l.fin) else {
                        continue;
                    };
                    let mut cur = entry;
                    let cap = dst.fins.len() + 1;
                    for _ in 0..cap {
                        let Some(fin) = dst.fins.get(cur) else { break };
                        es.insert(fin.edge);
                        if let Some(v) = dst.fin_start_vertex(cur) {
                            vs.insert(v);
                        }
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                }
            }
            let chi = vs.len() as i64 - es.len() as i64 + comp.len() as i64 - rings;
            ((2 - chi).max(0) / 2) as u32
        };
        let v_front: f64 = comp
            .iter()
            .flat_map(|&f| dst.tessellate_face(f))
            .map(|t| t[0].dot(t[1].cross(t[2])))
            .sum::<f64>()
            / 6.0;
        // Each DISCONNECTED exterior component is its own solid cell
        // (the first takes the caller's region; Euler-Poincare counts
        // closed shells as regions - 1, so sharing one region across
        // components mis-counts). Cavity components keep the first
        // solid region as their material side (matching the enclosing
        // component when there is one exterior component; with several
        // exterior components the cavity-to-component matching is a
        // documented simplification).
        let comp_solid = if v_front >= 0.0 && exterior_seen {
            dst.new_region(&mut rec, true, Derivation::Created)
        } else {
            solid
        };
        if v_front >= 0.0 {
            exterior_seen = true;
        }
        let front_region = if v_front >= 0.0 {
            inf
        } else {
            dst.new_region(&mut rec, false, Derivation::Created)
        };
        // Keep each face's side->region links consistent with the shell it
        // lands in (check_shells_regions enforces this): back side -> solid,
        // front side -> exterior/void.
        for &f in &comp {
            if let Some(face) = dst.faces.get_mut(f) {
                face.back_region = comp_solid;
                face.front_region = front_region;
            }
        }
        let back_shell = dst.new_shell(&mut rec, comp_solid, Derivation::Created);
        if let Some(s) = dst.shells.get_mut(back_shell) {
            s.faces = comp.iter().map(|&f| (f, Side::Back)).collect();
            s.genus = comp_genus;
        }
        if let Some(r) = dst.regions.get_mut(comp_solid) {
            r.shells.push(back_shell);
        }
        let front_shell = dst.new_shell(&mut rec, front_region, Derivation::Created);
        if let Some(s) = dst.shells.get_mut(front_shell) {
            s.faces = comp.iter().map(|&f| (f, Side::Front)).collect();
            s.genus = comp_genus;
        }
        if let Some(r) = dst.regions.get_mut(front_region) {
            r.shells.push(front_shell);
        }
    }
    // PINCH DETECTION (task 41): a valid assembly may touch itself at
    // isolated VERTICES: the crossing-cylinder difference's boundary
    // is two sphere-like shells pinched at the ellipse crossings (the
    // tunnel's side openings are holes in the lateral, so that IS the
    // geometric truth, not a defect). Such a vertex carries more than
    // one UMBRELLA (cyclic fan of face corners), the manifold
    // Euler-Poincare identity does not apply, and the validator's
    // non-manifold route (structural checks + the boundary-chain
    // oracle) is the correct contract. Record the extra umbrellas in
    // the vertex's `groups` (the PES mechanism merge_vertices uses).
    let vkeys: Vec<crate::entity::VertexKey> = dst.vertices.iter().map(|(k, _)| k).collect();
    for vk in vkeys {
        // One corner per face sector: the fin LEAVING vk.
        let corners: Vec<crate::entity::FinKey> = dst
            .fins
            .iter()
            .map(|(k, _)| k)
            .filter(|&fk| dst.fin_start_vertex(fk) == Some(vk))
            .collect();
        if corners.len() < 2 {
            continue;
        }
        // A corner enters vk on its loop-previous fin's edge and
        // leaves on its own; corners sharing an incident edge are
        // umbrella-adjacent. Union-find over corners keyed by edge.
        let mut parent: Vec<usize> = (0..corners.len()).collect();
        let mut by_edge: std::collections::BTreeMap<crate::entity::EdgeKey, Vec<usize>> =
            std::collections::BTreeMap::new();
        for (ci, &fk) in corners.iter().enumerate() {
            let Some(fin) = dst.fins.get(fk) else {
                continue;
            };
            by_edge.entry(fin.edge).or_default().push(ci);
            if let Some(prev) = dst.fins.get(fin.prev) {
                by_edge.entry(prev.edge).or_default().push(ci);
            }
        }
        for cs in by_edge.values() {
            for w in cs.windows(2) {
                uf_union(&mut parent, w[0], w[1]);
            }
        }
        let mut roots: Vec<usize> = Vec::new();
        for i in 0..corners.len() {
            let r = uf_find(&mut parent, i);
            if !roots.contains(&r) {
                roots.push(r);
            }
        }
        if roots.len() > 1 {
            let extra: Vec<crate::entity::FinKey> =
                roots[1..].iter().map(|&r| corners[r]).collect();
            if let Some(v) = dst.vertices.get_mut(vk) {
                v.groups = extra;
            }
        }
    }
    let _ = rec.finish();

    let _prof = crate::profile::Scope::new(&crate::profile::VALIDATE_NS);
    match dst.validate() {
        Ok(()) => Ok(dst),
        Err(e) => {
            if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
                eprintln!("assembly validate failed: {e:?}; counts {:?}", dst.counts());
            }
            Err(BoolFault::AssemblyFailed("stitched (curved) body invalid"))
        }
    }
}

fn uf_find(p: &mut [usize], mut i: usize) -> usize {
    while p[i] != i {
        p[i] = p[p[i]];
        i = p[i];
    }
    i
}

fn uf_union(p: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (uf_find(p, a), uf_find(p, b));
    if ra != rb {
        p[ra] = rb;
    }
}

/// Cellular region partition (item 29 Rung 1, dossier 57 / SGC select):
/// the body's face SIDES are grouped into cells by the Weiler radial-
/// sector rule, then each bounded cell becomes a region (solid when it
/// is bounded by outer-shell material sides) and the unbounded cell is
/// the infinite region. Manifold (radial-2) edges join like sides
/// directly (all faces import outward-oriented); non-manifold (radial-
/// 3+) edges -- where a retained interface wall meets the outer walls --
/// take the angular sector walk. Scope: straight non-manifold junction
/// edges with planar incident faces (the coincident-abutment class);
/// anything else declines loudly.
fn finalize_cellular(
    mut dst: Body,
    mut rec: crate::body::OpRecorder,
    outer: &[FaceKey],
    walls: &[FaceKey],
    inf: crate::entity::RegionKey,
    solid: crate::entity::RegionKey,
) -> Result<Body, BoolFault> {
    use crate::entity::Side;
    use crate::lineage::Derivation;
    use std::collections::{BTreeMap, BTreeSet};
    let nm_err = BoolFault::AssemblyFailed("cellular: unsupported non-manifold junction");

    let all: Vec<FaceKey> = outer.iter().chain(walls.iter()).copied().collect();
    let mut ids: BTreeMap<(FaceKey, Side), usize> = BTreeMap::new();
    for &f in &all {
        for s in [Side::Front, Side::Back] {
            let n = ids.len();
            ids.insert((f, s), n);
        }
    }
    let mut uf: Vec<usize> = (0..ids.len()).collect();
    let side_id = |f: FaceKey, s: Side| -> Option<usize> { ids.get(&(f, s)).copied() };
    let fin_face = |dst: &Body, fk: crate::entity::FinKey| -> Option<FaceKey> {
        dst.fins
            .get(fk)
            .and_then(|x| dst.loops.get(x.owner))
            .map(|l| l.face)
    };
    // True loop winding vs the stored front normal, per face: solid
    // faces are CCW about the outward normal by construction, but a
    // SHEET-built wall face has no enforced winding, so interior-on-
    // the-left needs the Newell sign to make the sector spokes point
    // into the face.
    let mut winding: BTreeMap<FaceKey, f64> = BTreeMap::new();
    for &fk in &all {
        let pts = dst.face_outer_loop_points(fk);
        let mut nw = keel_math::vec::Vec3::ZERO;
        for i in 0..pts.len() {
            let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
            nw = nw + p.cross(q);
        }
        let s = match dst.face_outward_normal(fk) {
            Some(n) if nw.dot(n) < 0.0 => -1.0,
            _ => 1.0,
        };
        winding.insert(fk, s);
    }
    let tau = core::f64::consts::TAU;
    for (ek, e) in dst.edges.iter() {
        let fins = e.radial.clone();
        if fins.len() == 2 {
            // Manifold: both faces are outward-oriented, so front pairs
            // with front (the shared void/exterior sector) and back with
            // back (the contiguous material sector).
            let (fa, fb) = (
                fin_face(&dst, fins[0]).ok_or(BoolFault::AssemblyFailed("cellular: stale fin"))?,
                fin_face(&dst, fins[1]).ok_or(BoolFault::AssemblyFailed("cellular: stale fin"))?,
            );
            if let (Some(a1), Some(b1), Some(a2), Some(b2)) = (
                side_id(fa, Side::Front),
                side_id(fb, Side::Front),
                side_id(fa, Side::Back),
                side_id(fb, Side::Back),
            ) {
                uf_union(&mut uf, a1, b1);
                uf_union(&mut uf, a2, b2);
            }
            continue;
        }
        if fins.len() < 2 {
            return Err(nm_err);
        }
        // Non-manifold junction: angular sector walk around the edge.
        // Straight edge, planar incident faces only (Rung 1 scope).
        let (p0, p1) = {
            let (v0, v1) = e.bounds;
            match (dst.vertices.get(v0), dst.vertices.get(v1)) {
                (Some(a), Some(b)) => (a.point, b.point),
                _ => return Err(nm_err),
            }
        };
        let Some(t) = (p1 - p0).try_normalize() else {
            return Err(nm_err);
        };
        let _ = ek;
        // Per fin: angle of the into-face direction (interior on the
        // left of traversal about the front normal) and of the front
        // normal itself.
        struct Spoke {
            face: FaceKey,
            theta: f64,
            phi: f64,
        }
        let mut spokes: Vec<Spoke> = Vec::with_capacity(fins.len());
        let mut basis: Option<(keel_math::vec::Vec3, keel_math::vec::Vec3)> = None;
        for &fk in &fins {
            let f = fin_face(&dst, fk).ok_or(BoolFault::AssemblyFailed("cellular: stale fin"))?;
            if !matches!(dst.face_surface3(f), Some(Surface3::Plane(_))) {
                return Err(nm_err);
            }
            let Some(n) = dst.face_outward_normal(f) else {
                return Err(nm_err);
            };
            let forward = dst.fins.get(fk).map(|x| x.forward) == Some(true);
            let d = if forward { t } else { t * -1.0 };
            let w = n.cross(d) * winding.get(&f).copied().unwrap_or(1.0);
            let (bx, by) = match basis {
                Some(b) => b,
                None => {
                    let bx = match w.try_normalize() {
                        Some(x) => x,
                        None => return Err(nm_err),
                    };
                    let by = t.cross(bx);
                    basis = Some((bx, by));
                    (bx, by)
                }
            };
            let theta = w.dot(by).atan2(w.dot(bx)).rem_euclid(tau);
            let phi = n.dot(by).atan2(n.dot(bx)).rem_euclid(tau);
            spokes.push(Spoke {
                face: f,
                theta,
                phi,
            });
        }
        spokes.sort_by(|a, b| a.theta.total_cmp(&b.theta));
        // The sector ccw from spoke i to spoke j is bounded by the side
        // of face i whose normal is +90 deg CCW of its own spoke and the
        // side of face j whose normal is -90 deg (CW) of its spoke; the
        // normal offset is exactly +-pi/2, so the < pi test is robust.
        let pi = core::f64::consts::PI;
        for i in 0..spokes.len() {
            let j = (i + 1) % spokes.len();
            let (si, sj) = (&spokes[i], &spokes[j]);
            let side_start = if (si.phi - si.theta).rem_euclid(tau) < pi {
                Side::Front
            } else {
                Side::Back
            };
            let side_end = if (sj.theta - sj.phi).rem_euclid(tau) < pi {
                Side::Front
            } else {
                Side::Back
            };
            if let (Some(a), Some(b)) = (side_id(si.face, side_start), side_id(sj.face, side_end)) {
                uf_union(&mut uf, a, b);
            }
        }
    }

    // Signed cell volumes: out-of-cell normal is +front for a Back side
    // and -front for a Front side; the unbounded (exterior) cell sums
    // negative.
    let entries: Vec<((FaceKey, Side), usize)> = ids.iter().map(|(k, v)| (*k, *v)).collect();
    let mut vol: BTreeMap<usize, f64> = BTreeMap::new();
    let mut members: BTreeMap<usize, Vec<(FaceKey, Side)>> = BTreeMap::new();
    let outer_set: BTreeSet<FaceKey> = outer.iter().copied().collect();
    for ((f, side), i) in entries {
        let root = uf_find(&mut uf, i);
        let flux: f64 = dst
            .tessellate_face(f)
            .iter()
            .map(|t| t[0].dot(t[1].cross(t[2])))
            .sum::<f64>()
            / 6.0;
        let signed = match side {
            Side::Back => flux,
            Side::Front => -flux,
        };
        *vol.entry(root).or_insert(0.0) += signed;
        members.entry(root).or_default().push((f, side));
    }
    // Exactly one unbounded cell (negative volume); each bounded cell
    // with an outer-shell material (Back) side is SOLID, a bounded cell
    // with none is an enclosed void.
    let exterior = vol
        .iter()
        .filter(|(_, v)| **v < 0.0)
        .map(|(k, _)| *k)
        .collect::<Vec<_>>();
    let [exterior] = exterior[..] else {
        return Err(BoolFault::AssemblyFailed(
            "cellular: exterior cell not unique",
        ));
    };
    let mut next_solid = Some(solid);
    for (root, mem) in &members {
        let region = if *root == exterior {
            inf
        } else {
            let is_solid = mem
                .iter()
                .any(|(f, s)| *s == Side::Back && outer_set.contains(f));
            if is_solid {
                match next_solid.take() {
                    Some(r) => r,
                    None => dst.new_region(&mut rec, true, Derivation::Created),
                }
            } else {
                dst.new_region(&mut rec, false, Derivation::Created)
            }
        };
        let shell = dst.new_shell(&mut rec, region, Derivation::Created);
        if let Some(s) = dst.shells.get_mut(shell) {
            s.faces = mem.clone();
        }
        if let Some(r) = dst.regions.get_mut(region) {
            r.shells.push(shell);
        }
        for &(f, side) in mem {
            if let Some(face) = dst.faces.get_mut(f) {
                match side {
                    Side::Front => face.front_region = region,
                    Side::Back => face.back_region = region,
                }
            }
        }
    }
    if next_solid.is_some() {
        return Err(BoolFault::AssemblyFailed("cellular: no solid cell found"));
    }
    let _ = rec.finish();
    match dst.validate() {
        Ok(()) => Ok(dst),
        Err(_) => Err(BoolFault::AssemblyFailed("cellular body invalid")),
    }
}

/// Knit / sew a set of sheet (or solid) bodies into one (parity item 71):
/// import every face, merge coincident vertices, glue coincident free edges
/// into radial pairs, and -- if the result closes into a watertight shell --
/// promote it to a SOLID (the enclosed-void-aware region partition runs in
/// `finalize_imported_assembly`). Six planar square sheets, each oriented
/// outward, knit into a closed cube -> a solid of the cube's volume. Faces
/// are imported with per-body identity (within-body shared edges dedup by
/// source id; cross-body coincident edges join via the coordinate merge).
/// Returns Err if the knit does not close into a valid solid (a still-open
/// multi-sheet result is a follow-up).
pub fn knit(bodies: &[&Body], tol: f64) -> Result<Body, BoolFault> {
    use crate::lineage::Derivation;
    use std::collections::BTreeMap;
    let vtol = tol.max(1e-7);
    let mut dst = Body::new();
    let inf = dst.infinite_region();
    let mut rec = dst.begin_op();
    let solid = dst.new_region(&mut rec, true, Derivation::Created);
    let mut faces = Vec::new();
    for body in bodies {
        // Fresh maps PER BODY: within-body shared edges dedup by source id;
        // genuinely-coincident topology between bodies joins in the merge.
        let mut vmap: BTreeMap<(Operand, u64), crate::entity::VertexKey> = BTreeMap::new();
        let mut emap: BTreeMap<(Operand, u64), crate::entity::EdgeKey> = BTreeMap::new();
        for fk in body.face_keys() {
            let f = import_face(
                &mut dst,
                body,
                fk,
                Operand::A,
                false,
                &mut rec,
                &mut vmap,
                &mut emap,
                inf,
                solid,
            )
            .ok_or(BoolFault::AssemblyFailed("knit: import failed"))?;
            faces.push(f);
        }
    }
    finalize_imported_assembly(dst, rec, faces, Vec::new(), inf, solid, vtol)
}

/// Explode / unjoin: the inverse of `knit`. Split `body` into one independent
/// SHEET body per face, each carrying that face's trimmed loop and geometry,
/// double-sided over the void (free edges, as an open lamina). A face that
/// cannot be re-imported as a standalone sheet is skipped, so the result has at
/// most one body per face. (Plasticity "Unjoin Shells".)
pub fn explode(body: &Body) -> Vec<Body> {
    use std::collections::BTreeMap;
    let mut out = Vec::new();
    for fk in body.face_keys() {
        let mut dst = Body::new();
        let inf = dst.infinite_region();
        let mut rec = dst.begin_op();
        let mut vmap: BTreeMap<(Operand, u64), crate::entity::VertexKey> = BTreeMap::new();
        let mut emap: BTreeMap<(Operand, u64), crate::entity::EdgeKey> = BTreeMap::new();
        let Some(nf) = import_face(
            &mut dst,
            body,
            fk,
            Operand::A,
            false,
            &mut rec,
            &mut vmap,
            &mut emap,
            inf,
            inf,
        ) else {
            continue;
        };
        if let Ok(sheet) = finalize_imported_sheet(dst, rec, vec![nf], inf, 1e-7) {
            out.push(sheet);
        }
    }
    out
}

/// Partition `faces` (kept fragments of the stitched body) into connected
/// boundary components: faces sharing an edge are in the same component.
/// Each component is one closed boundary shell (e.g. the outer box surface
/// and the inner void surface of a hollow body are two separate components).
fn connected_face_components(dst: &Body, faces: &[FaceKey]) -> Vec<Vec<FaceKey>> {
    use std::collections::BTreeMap;
    let idx: BTreeMap<FaceKey, usize> = faces.iter().enumerate().map(|(i, &f)| (f, i)).collect();
    let mut parent: Vec<usize> = (0..faces.len()).collect();
    // ITERATIVE union-find (was recursive): the recursive variant's depth
    // scaled with the union-tree height, so a pathological component (a long
    // union chain) could drive it deep enough to overflow the stack -- a crash
    // is strictly worse than a decline (it bypasses DECLINE-never-WRONG). The
    // two-pass loop (find root, then path-compress) is byte-identical in result
    // and O(1) stack regardless of body complexity.
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut c = i;
        while p[c] != c {
            let nx = p[c];
            p[c] = r;
            c = nx;
        }
        r
    }
    // Union faces that share an edge (via the edge's radial fins -> owners).
    for (_, e) in dst.edges.iter() {
        let mut members: Vec<usize> = e
            .radial
            .iter()
            .filter_map(|&fk| dst.fins.get(fk))
            .filter_map(|fin| dst.loops.get(fin.owner))
            .filter_map(|l| idx.get(&l.face).copied())
            .collect();
        members.dedup();
        for w in members.windows(2) {
            let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            if a != b {
                parent[a] = b;
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<FaceKey>> = BTreeMap::new();
    for (i, &f) in faces.iter().enumerate() {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(f);
    }
    groups.into_values().collect()
}

/// Coplanar, overlapping planar face pairs `(face_in_a, face_in_b, n_a)`
/// -- the candidates whose overlap boundary must be imprinted before
/// classification (research file 39 §1). Curved coincidence is a follow-up.
fn coincident_face_pairs(
    a: &Body,
    b: &Body,
    tol: f64,
) -> Vec<(FaceKey, FaceKey, keel_math::vec::Vec3)> {
    let is_planar = |body: &Body, f: FaceKey| {
        matches!(
            body.face_surface_geom(f),
            Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
        )
    };
    // Hoist B's per-face data out of the pair loop (OPT-M2: the
    // outward normal rides face_interior_point, and recomputing it
    // per (fa, fb) pair dominated the detection profile).
    let b_faces: Vec<(FaceKey, keel_math::vec::Vec3, keel_math::vec::Vec3)> = b
        .face_keys()
        .into_iter()
        .filter(|&fb| is_planar(b, fb))
        .filter_map(|fb| {
            let nb = b.face_outward_normal(fb)?;
            let pb = b.face_outer_loop_points(fb).first().copied()?;
            Some((fb, nb, pb))
        })
        .collect();
    let mut pairs = Vec::new();
    for fa in a.face_keys() {
        if !is_planar(a, fa) {
            continue;
        }
        let na = match a.face_outward_normal(fa) {
            Some(n) => n,
            None => continue,
        };
        let pa = match a.face_outer_loop_points(fa).first().copied() {
            Some(p) => p,
            None => continue,
        };
        for &(fb, nb, pb) in &b_faces {
            // Same plane: parallel normals and a-point within HALF
            // the op tolerance of b's plane, the SAME band as
            // coincident_sense_at: detection and classification must
            // agree on what is coincident, and tol/2 sits safely
            // between the contact regimes (1e-9 mates in, gaps of
            // exactly tol out, both with wide margin), so the
            // at-tolerance class is deterministic instead of
            // bistable on probe noise.
            if na.cross(nb).norm() < 1e-7 && (pb - pa).dot(na).abs() < 0.5 * tol {
                let poly_a = a.face_outer_loop_points(fa);
                let poly_b = b.face_outer_loop_points(fb);
                if crate::coincident::coplanar_overlap_exists(&poly_a, &poly_b, na) {
                    pairs.push((fa, fb, na));
                }
            }
        }
    }
    pairs
}

/// Clone `a`/`b` and imprint each coincident overlap's interior-boundary
/// cuts onto both, so partial overlaps split into uniformly-classified
/// fragments. Returns `None` (use the originals) when no coincident faces
/// exist. Best-effort: imprint failures are skipped and the final
/// positive-volume post-condition still guards a bad selection.
fn preimprint_coincident_overlaps(a: &Body, b: &Body, tol: f64) -> Option<(Body, Body)> {
    let pairs = {
        let _prof = crate::profile::Scope::new(&crate::profile::PREIMPRINT_DETECT_NS);
        coincident_face_pairs(a, b, tol)
    };
    if pairs.is_empty() {
        return None;
    }
    let _prof = crate::profile::Scope::new(&crate::profile::PREIMPRINT_CUT_NS);
    let mut a = a.clone();
    let mut b = b.clone();
    let imprint_cuts = |body: &mut Body,
                        face: FaceKey,
                        subj: &[keel_math::vec::Vec3],
                        other: &[keel_math::vec::Vec3],
                        n: keel_math::vec::Vec3| {
        // The cuts CHAIN through corners interior to the subject face
        // (research file 39 sec 1: the overlap boundary enters and
        // leaves through the subject's boundary, cornering inside).
        // Boundary-to-boundary single segments take the open-curve
        // imprint (pre-bounded per the open-imprint contract: a Line3
        // normalizes its direction, so t in [0,1] would not span the
        // segment); multi-segment chains take the spur-chain polyline
        // imprint. CLOSED chains (a fully enclosed overlap pocket)
        // are skipped: the gates decline those honestly (follow-up).
        let segs = crate::coincident::overlap_interior_segments(subj, other, n);
        for chain in crate::coincident::chain_segments(&segs) {
            let closed = chain.len() > 2 && (chain[0] - chain[chain.len() - 1]).norm() < 1e-9;
            if closed {
                // The ENCLOSED POCKET: the other face sits wholly
                // inside this one; imprint the polygon as an inner
                // ring with a pocket face. Orient the chain to wind
                // WITH the host's outer loop: the ring (the kemr back
                // cycle) then winds opposite, as an inner ring must,
                // making the rim's radial pairing deterministic.
                let mut ring: Vec<keel_math::vec::Vec3> = chain[..chain.len() - 1].to_vec();
                let signed = |pts: &[keel_math::vec::Vec3]| -> f64 {
                    let mut s = keel_math::vec::Vec3::ZERO;
                    for i in 0..pts.len() {
                        let p = pts[i];
                        let q = pts[(i + 1) % pts.len()];
                        s = s + p.cross(q);
                    }
                    s.dot(n)
                };
                if signed(subj) * signed(&ring) < 0.0 {
                    ring.reverse();
                }
                let _ = body.imprint_closed_polyline(face, &ring);
                continue;
            }
            if chain.len() == 2 {
                if let Ok(seg) = keel_geom::nurbs_curve::NurbsCurve::new(
                    1,
                    vec![0., 0., 1., 1.],
                    vec![chain[0], chain[1]],
                    None,
                ) {
                    let _ =
                        body.imprint_open_curve(face, &keel_geom::curve::Curve3::Nurbs(seg), tol);
                }
            } else {
                let _ = body.imprint_open_polyline(face, &chain, tol);
            }
        }
    };
    for (fa, fb, n) in pairs {
        let poly_a = a.face_outer_loop_points(fa);
        let poly_b = b.face_outer_loop_points(fb);
        imprint_cuts(&mut a, fa, &poly_a, &poly_b, n);
        imprint_cuts(&mut b, fb, &poly_b, &poly_a, n);
    }
    Some((a, b))
}

/// Options for `boolean_with` (item 29 / dossier 57 Q4). `regularize:
/// true` (the default, and what `boolean` does) discards interior and
/// lower-dimensional cells per Requicha. `regularize: false` keeps the
/// numReg=2 interface walls: the union of two solids abutting along a
/// coincident face retains that face as a DOUBLE-SIDED interior
/// partition wall, and the result is a CELLULAR solid whose solid
/// material is partitioned into multiple solid regions.
#[derive(Clone, Copy, Debug)]
/// Tuning knobs for [`boolean_with`]. [`Default`] enables regularization.
pub struct BooleanOptions {
    /// When `true` (the default), discard lower-dimensional artifacts
    /// (dangling faces/edges) so the result is a clean regularized set.
    pub regularize: bool,
}

impl Default for BooleanOptions {
    fn default() -> Self {
        Self { regularize: true }
    }
}

/// Regularized boolean of two solid bodies.
///
/// Computes `a op b` ([`BoolOp::Union`] / [`BoolOp::Intersection`] /
/// [`BoolOp::Difference`]) and returns a [`BoolResult`] holding the
/// resulting body plus any per-face-pair [`BoolFault`]s. `tol` is the
/// linear modeling tolerance (in model units) used for coincidence and
/// snapping decisions; `1e-7` is a typical value for a part on the
/// order of tens of units.
///
/// # The DECLINE-never-WRONG contract
///
/// This is the central robustness guarantee: the kernel returns
/// `Err(BoolFault)` (a DECLINE) rather than ever returning a
/// geometrically WRONG body. Configurations the current pipeline cannot
/// assemble exactly (coplanar/coincident faces, tangencies, certain
/// crossing curved seams) surface as faults instead of corrupt
/// geometry. A successful `Ok` result with empty `faults` is trusted to
/// be correct (mass equals mesh volume).
///
/// # Errors
///
/// Returns `Err(BoolFault)` when an operand is inside-out (negative
/// volume), when intersection geometry cannot be assembled into a valid
/// body, or when a hard configuration is encountered. See [`BoolFault`]
/// for the variants and what each DECLINE means.
///
/// For near-coincident contact that strict mode declines, see
/// [`boolean_tolerant`].
pub fn boolean(a: &Body, b: &Body, op: BoolOp, tol: f64) -> Result<BoolResult, BoolFault> {
    boolean_with(a, b, op, tol, BooleanOptions::default())
}

/// [`boolean`] with explicit [`BooleanOptions`].
///
/// Identical to [`boolean`] except the caller controls regularization.
/// The same DECLINE-never-WRONG contract and `Err(BoolFault)` semantics
/// apply.
pub fn boolean_with(
    a: &Body,
    b: &Body,
    op: BoolOp,
    tol: f64,
    opts: BooleanOptions,
) -> Result<BoolResult, BoolFault> {
    // FRONT-DOOR intake check (dossier 29 Part 6): an INSIDE-OUT
    // operand (negative signed volume; e.g. an over-thick hollow's
    // collapsed inner shell) declines loudly here. Classification
    // would read its inverted winding as legitimate and select a
    // self-consistent but WRONG result. Sheets and wires (volume
    // ~ 0) pass; orientation REPAIR with a report is the follow-up.
    {
        let _prof = crate::profile::Scope::new(&crate::profile::FRONT_DOOR_NS);
        if a.mesh_volume() < -1e-9 || b.mesh_volume() < -1e-9 {
            return Err(BoolFault::AssemblyFailed(
                "operand is inside-out (negative volume)",
            ));
        }
    }
    // BROAD PHASE (task 49, the basic overlap gate): if the operands' AABBs
    // are provably separated, the solids cannot meet, so the boolean is
    // TRIVIAL -- intersection is empty, difference is A unchanged. Handle those
    // here and skip the SSI/seam machinery entirely, which (a) is far faster on
    // non-overlapping pairs and (b) removes any chance of the no-seam shortcut
    // mis-probing a pair that never touched. A disjoint UNION is a disconnected
    // body, so it falls through to the existing assembly (the planar two-shell
    // case it supports; curved disjoint declines via the post-condition). The
    // margin is the op tolerance, so contact / near-contact pairs (gap <= tol)
    // take the normal path; the tolerant snap is unaffected because it runs the
    // strict pipeline on already-snapped (touching) geometry.
    {
        let (ba, bb) = (a.bounding_box(), b.bounding_box());
        // CONSERVATIVE margin: bounding_box() is TESSELLATION-derived, so it
        // under-estimates a curved body's true extent by up to its chord
        // sagitta (< ~1% of the body diagonal at the kernel's >=16-seg/circle
        // density). Cull only when the AABB gap exceeds a safe multiple of
        // that (5% of the combined diagonals) so a grazing CURVED overlap is
        // never mistaken for disjoint (which would silently drop a real
        // sliver). For exact (planar) AABBs this is merely stricter than
        // needed; the culled and the fall-through paths return the same
        // empty/clone for those, so box results are unchanged either way.
        let m = tol.max(0.05 * ((ba.max - ba.min).norm() + (bb.max - bb.min).norm()));
        let separated = ba.min.x - bb.max.x > m
            || bb.min.x - ba.max.x > m
            || ba.min.y - bb.max.y > m
            || bb.min.y - ba.max.y > m
            || ba.min.z - bb.max.z > m
            || bb.min.z - ba.max.z > m;
        if separated {
            match op {
                BoolOp::Intersection => {
                    return Ok(BoolResult {
                        body: Body::new(),
                        faults: Vec::new(),
                        op,
                    });
                }
                BoolOp::Difference => {
                    return Ok(BoolResult {
                        body: a.clone(),
                        faults: Vec::new(),
                        op,
                    });
                }
                BoolOp::Union => {
                    // Disconnected union: both lumps, one body. The clean
                    // verbatim combine (no imprint/SSI) is the curved-operand
                    // fix -- the old fall-through to the full assembly merged
                    // disconnected curved components into one broken region.
                    return combine_disjoint(a, b, tol).and_then(|body| {
                        // DECLINE-never-WRONG: combine_disjoint bypasses the SSI
                        // assembler's mass==mesh gate, and analytic mass_properties
                        // can COLLAPSE on a disjoint CURVED union (sphere
                        // mirror+union, realsoak seed 11400715918834829910: mass
                        // 238 vs the true render volume 14441) while validate()
                        // passes. Cross-check the analytic mass against the
                        // independent render mesh; a gross (>25%, the soak oracle's
                        // curved-WRONG criterion) gap means the reported volume is
                        // wrong, so decline rather than hand it back. A planar
                        // disjoint union (exact mass==mesh) is unaffected. (Real
                        // fix: the disjoint curved mass integration -- a follow-up.)
                        let render_mesh = body.mesh_volume();
                        let inconsistent = body
                            .mass_properties()
                            .map(|m| m.volume)
                            .ok()
                            .is_some_and(|mass| {
                                let denom = mass.abs().max(render_mesh.abs());
                                mass.is_finite()
                                    && render_mesh.is_finite()
                                    && denom > 1e-9
                                    && (mass - render_mesh).abs() / denom > 0.25
                            });
                        if inconsistent {
                            return Err(BoolFault::AssemblyFailed(
                                "disjoint union: analytic mass inconsistent with mesh (mass-integration follow-up)",
                            ));
                        }
                        Ok(BoolResult {
                            body,
                            faults: Vec::new(),
                            op,
                        })
                    });
                }
            }
        }
    }
    // Pre-pass (research file 39 §1): where two coplanar faces partially
    // overlap, imprint the overlap-boundary cuts onto the operands so each
    // resulting fragment is uniformly inside/outside/on the other body --
    // the on-on tables in select_faces then classify them correctly. With
    // no coincident faces this is a no-op and the originals flow through.
    let pre = {
        let _prof = crate::profile::Scope::new(&crate::profile::PREIMPRINT_NS);
        preimprint_coincident_overlaps(a, b, tol)
    };
    let (a, b): (&Body, &Body) = match &pre {
        Some((pa, pb)) => (pa, pb),
        None => (a, b),
    };
    let (seams, faults) = {
        let _prof = crate::profile::Scope::new(&crate::profile::SEAM_NS);
        seam_curves(a, b, tol)
    };
    // Tangential face pairs (touch at a point/curve without crossing) are
    // still declined. COINCIDENT (coplanar overlapping) faces now PROCEED:
    // the winding classifier marks them FaceClass::OnOther and select_faces
    // drops them, which is the correct on-on rule for the common case
    // (abutting/face-to-face solids whose shared face is interior to the
    // union/intersection). The final positive-volume post-condition guards
    // partial-overlap cases that this simplest rule mis-selects, declining
    // them honestly. (Full Requicha on-on tables -> a follow-up.)
    if let Some(f) = faults
        .iter()
        .find(|f| matches!(f, BoolFault::Tangent(..) | BoolFault::UnassemblableSeam(..)))
        .cloned()
    {
        return Err(f);
    }
    // NO BOUNDARY INTERACTION (dossier 29, graceful degradation rung
    // 1): with no seams and no coincidence the operands are disjoint
    // or nested, decided by one unambiguous winding probe per side.
    // These are CLEAN answers (empty body / operand clone), not an
    // error class; only the two cases that genuinely assemble (the
    // disconnected union, the cavity difference) fall through. The
    // shortcut requires POSITIVELY ORIENTED operands: an inside-out
    // operand (dossier 29 Part 6; e.g. an over-thick hollow's
    // collapsed inner shell) reads as "nested" to the winding probe,
    // so it flows to the assembly whose mass==mesh gates decline it.
    if seams.is_empty() && pre.is_none() && a.mesh_volume() > 0.0 && b.mesh_volume() > 0.0 {
        let _prof = crate::profile::Scope::new(&crate::profile::SHORTCUT_NS);
        // Probe with GUARANTEED-INTERIOR points (a face interior point
        // nudged inward), never raw vertices: a mated body's vertices
        // can ALL lie on the other's boundary (the pin's only vertices
        // are its rim seams, on the hole wall), where the winding is
        // noise around one half.
        let probe = |of: &Body, against: &Body| -> Option<bool> {
            let eps = (shortest_edge(of) * 1e-3).clamp(1e-9, 1e-3);
            let against_tris = against.boundary_triangles();
            for f in of.face_keys() {
                let (Some(p), Some(n)) = (of.face_interior_point(f), of.face_outward_normal(f))
                else {
                    continue;
                };
                let w = crate::winding::gwn_over(&against_tris, p - n * eps);
                if (w - 0.5).abs() > 0.25 {
                    return Some(w > 0.5);
                }
            }
            None
        };
        let (Some(a_in_b), Some(b_in_a)) = (probe(a, b), probe(b, a)) else {
            return Err(BoolFault::AssemblyFailed(
                "no seams and no unambiguous containment probe",
            ));
        };
        let done = |body: Body| -> Result<BoolResult, BoolFault> {
            // task 49: the no-seams shortcut trusts the containment probe, but
            // a MISSING seam can mean "SSI failed to find a real intersection"
            // (tilted cyl/cyl), not "disjoint/nested". For curved operands,
            // bound the cloned result by the op-volume inequality from the
            // operand volumes; a violation means the probe picked wrong (it
            // dropped an operand), so DECLINE rather than return a silent WRONG
            // (the soak's tilted cyl/cyl union, which returned b.clone()).
            // Planar operands keep the exact probe (boxes are bit-identical).
            let curved = |x: &Body| {
                x.face_keys()
                    .iter()
                    .any(|&f| !matches!(x.face_surface3(f), Some(Surface3::Plane(_))))
            };
            if curved(a) || curved(b) {
                let opvol = |x: &Body| {
                    x.mass_properties()
                        .map(|m| m.volume)
                        .unwrap_or_else(|_| x.mesh_volume())
                };
                let (va, vb) = (opvol(a), opvol(b));
                if va.is_finite() && vb.is_finite() && va >= 0.0 && vb >= 0.0 {
                    let (lo, hi) = match op {
                        BoolOp::Union => (va.max(vb), va + vb),
                        BoolOp::Intersection => (0.0, va.min(vb)),
                        BoolOp::Difference => ((va - vb).max(0.0), va),
                    };
                    let slack = 5e-2 * (1.0 + hi);
                    let rv = body
                        .mass_properties()
                        .map(|m| m.volume)
                        .unwrap_or_else(|_| body.mesh_volume());
                    if rv < lo - slack || rv > hi + slack {
                        return Err(BoolFault::AssemblyFailed(
                            "no-seam shortcut result violates op-volume bound (declined)",
                        ));
                    }
                }
            }
            // Coplanar-but-empty-overlap pairs flag Coincident even
            // though nothing interacts (the pre-pass found no overlap,
            // or it would have re-imprinted): noise here, dropped.
            Ok(BoolResult {
                body,
                faults: faults
                    .iter()
                    .filter(|f| !matches!(f, BoolFault::Coincident(..)))
                    .cloned()
                    .collect(),
                op,
            })
        };
        match (op, a_in_b, b_in_a) {
            (BoolOp::Intersection, false, false) => return done(Body::new()),
            (BoolOp::Intersection, true, _) => return done(a.clone()),
            (BoolOp::Intersection, _, true) => return done(b.clone()),
            (BoolOp::Difference, false, false) => return done(a.clone()),
            (BoolOp::Difference, true, _) => return done(Body::new()),
            (BoolOp::Union, true, _) => return done(b.clone()),
            (BoolOp::Union, _, true) => return done(a.clone()),
            // The disconnected union and the cavity difference build
            // real bodies: fall through to the assembly, without the
            // empty-overlap Coincident noise.
            (BoolOp::Union, false, false) | (BoolOp::Difference, _, true) => {
                let faults: Vec<BoolFault> = faults
                    .into_iter()
                    .filter(|f| !matches!(f, BoolFault::Coincident(..)))
                    .collect();
                return assemble_boolean(a, b, op, tol, &seams, faults, opts);
            }
        }
    }
    assemble_boolean(a, b, op, tol, &seams, faults, opts)
}

/// Confidence report for the tolerant boolean (dossier 29: honesty as
/// a feature). `salvaged == false` means the strict pipeline ran on
/// the input as given (tier 1, achieved tolerance 0 = exact);
/// `salvaged == true` means the dossier-39 PREPARE phase moved
/// geometry by at most `achieved_tolerance` before the strict
/// pipeline ran (tier 2). Salvage is never silent: the caller always
/// sees the tier and the bound.
#[derive(Clone, Copy, Debug, PartialEq)]
/// Provenance of a [`boolean_tolerant`] result: how much snapping it
/// took and the tolerance actually achieved.
pub struct Confidence {
    /// `true` if near-coincident contact was snapped to assemble the
    /// result (i.e. strict mode alone would have declined).
    pub salvaged: bool,
    /// Which recovery tier produced the result (0 = strict, higher =
    /// more aggressive snapping).
    pub tier: u8,
    /// The effective fuzz (model units) applied, capped by local feature
    /// size; never larger than the caller's requested `fuzz`.
    pub achieved_tolerance: f64,
}

/// TOLERANT boolean (dossier 29 Tier 2; dossier 39 sec 3.4 / 4.2/4.3):
/// snap NEAR-COINCIDENT planar contact to exact coincidence within the
/// caller-bounded `fuzz` (the OCCT fuzzy-value semantics; ACIS's
/// prepare phase, which Spatial reports fixes ~70 percent of failed
/// booleans in one iteration), then run the strict pipeline ONCE on
/// the snapped input. Coincidence is PRESERVED design intent, never
/// perturbed away. The kernel caps the effective fuzz by local feature
/// size (one tenth of the shortest edge, the P5 guardrail) so a
/// generous caller bound cannot eat real features. Strict `boolean`
/// is untouched: a literal 1e-5 gap is two honest components there;
/// declaring it a flush mate is this caller's explicit choice.
///
/// Scope (the narrow honest slice): planar face pairs with straight
/// boundary edges snap; near-mated cylinder lateral pairs (near-
/// coaxial, near-equal radius, flat caps) snap onto the exact carrier
/// (the radial-gap clearance pin); near-mated CONE lateral pairs
/// (near-coaxial same-sense, near-equal taper and radius, flat caps:
/// the countersink clearance plug, task 30) snap the same way; other
/// curved near-contact stays strict-only.
pub fn boolean_tolerant(
    a: &Body,
    b: &Body,
    op: BoolOp,
    tol: f64,
    fuzz: f64,
) -> Result<(BoolResult, Confidence), BoolFault> {
    if !(fuzz.is_finite() && fuzz >= 0.0) {
        return Err(BoolFault::AssemblyFailed("tolerant: bad fuzz"));
    }
    let cap = shortest_edge(a).min(shortest_edge(b)) / 10.0;
    let fuzz_eff = fuzz.min(cap);
    match prepare_snap(a, b, fuzz_eff) {
        Some((b_snapped, moved)) => {
            let r = boolean_with(a, &b_snapped, op, tol, BooleanOptions::default())?;
            Ok((
                r,
                Confidence {
                    salvaged: true,
                    tier: 2,
                    achieved_tolerance: moved,
                },
            ))
        }
        None => {
            let r = boolean_with(a, b, op, tol, BooleanOptions::default())?;
            Ok((
                r,
                Confidence {
                    salvaged: false,
                    tier: 1,
                    achieved_tolerance: 0.0,
                },
            ))
        }
    }
}

/// Shortest finite edge length (the local-feature-size proxy that caps
/// the tolerant fuzz; infinite when the body has no measurable edges).
fn shortest_edge(b: &Body) -> f64 {
    let mut best = f64::INFINITY;
    for (_, e) in b.edges.iter() {
        let (v0, v1) = e.bounds;
        if v0 == v1 {
            continue;
        }
        let (Some(p0), Some(p1)) = (
            b.vertices.get(v0).map(|x| x.point),
            b.vertices.get(v1).map(|x| x.point),
        ) else {
            continue;
        };
        let l = (p1 - p0).norm();
        if l > 0.0 {
            best = best.min(l);
        }
    }
    best
}

/// The dossier-39 PREPARE phase: find B's planar faces whose planes
/// lie within `(0, fuzz]` of a parallel near-mating A plane (inflated
/// AABB overlap required) and snap their vertices and surfaces EXACTLY
/// onto the A plane. Returns the snapped clone of B and the largest
/// single movement applied; `None` when nothing is within fuzz (the
/// clean tier-1 path). Candidates with curved boundary edges or
/// curved surfaces are skipped (the planar slice).
fn prepare_snap(a: &Body, b: &Body, fuzz: f64) -> Option<(Body, f64)> {
    use keel_geom::curve::Curve3;
    use keel_math::vec::Vec3;
    if fuzz <= 0.0 || !fuzz.is_finite() {
        return None;
    }
    let aabb = |body: &Body, f: FaceKey| -> Option<(Vec3, Vec3)> {
        let pts = body.face_outer_loop_points(f);
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in &pts {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        lo.x.is_finite().then_some((lo, hi))
    };
    let mut snapped: Option<Body> = None;
    let mut moved = 0.0f64;
    for fa in a.face_keys() {
        let Some(Surface3::Plane(pl_a)) = a.face_surface3(fa) else {
            continue;
        };
        let Some(na) = a.face_outward_normal(fa) else {
            continue;
        };
        let pa0 = pl_a.frame.origin;
        let Some((alo, ahi)) = aabb(a, fa) else {
            continue;
        };
        for fb in b.face_keys() {
            let Some(Surface3::Plane(_)) = b.face_surface3(fb) else {
                continue;
            };
            let Some(nb) = b.face_outward_normal(fb) else {
                continue;
            };
            if na.cross(nb).norm() > 1e-6 {
                continue; // not parallel in either sense
            }
            let Some(pb0) = b.face_outer_loop_points(fb).first().copied() else {
                continue;
            };
            let off = (pb0 - pa0).dot(na);
            if off.abs() <= 1e-12 || off.abs() > fuzz {
                continue; // exactly coincident already, or out of reach
            }
            let Some((blo, bhi)) = aabb(b, fb) else {
                continue;
            };
            let near = |lo1: Vec3, hi1: Vec3, lo2: Vec3, hi2: Vec3| {
                lo1.x <= hi2.x + fuzz
                    && lo2.x <= hi1.x + fuzz
                    && lo1.y <= hi2.y + fuzz
                    && lo2.y <= hi1.y + fuzz
                    && lo1.z <= hi2.z + fuzz
                    && lo2.z <= hi1.z + fuzz
            };
            if !near(alo, ahi, blo, bhi) {
                continue;
            }
            // The planar slice: every boundary edge of the candidate
            // must be straight (no attached curve geometry to re-fit).
            let curved_rim = b.face_edge_set(fb).into_iter().any(|e| {
                b.edges
                    .get(e)
                    .and_then(|x| x.curve)
                    .and_then(|(ck, _)| b.curves.get(ck))
                    .map(|c| !matches!(c, Curve3::Line(_)))
                    .unwrap_or(false)
            });
            if curved_rim {
                continue;
            }
            // Snap: project every loop vertex of fb onto A's plane and
            // re-seat fb's surface on it, preserving fb's outward sense.
            let body = snapped.get_or_insert_with(|| b.clone());
            let lps = body
                .faces
                .get(fb)
                .map(|f| f.loops.clone())
                .unwrap_or_default();
            let mut vs: Vec<crate::entity::VertexKey> = Vec::new();
            for lk in lps {
                let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                loop {
                    if let Some(v) = body.fin_start_vertex(cur)
                        && !vs.contains(&v)
                    {
                        vs.push(v);
                    }
                    let Some(next) = body.fins.get(cur).map(|x| x.next) else {
                        break;
                    };
                    cur = next;
                    if cur == entry {
                        break;
                    }
                }
            }
            for v in vs {
                if let Some(x) = body.vertices.get_mut(v) {
                    let d = (x.point - pa0).dot(na);
                    x.point = x.point - na * d;
                    moved = moved.max(d.abs());
                }
            }
            if let Ok(frame) = keel_geom::surface::Frame3::from_z(pa0, nb) {
                let plane = keel_geom::surface::Plane3::new(frame);
                body.attach_face_surface(
                    fb,
                    crate::entity::SurfaceGeom::Analytic(Surface3::Plane(plane)),
                    true,
                );
            }
        }
    }
    // CYLINDER prepare (M6): a near-mated lateral pair, near-coaxial
    // axes and near-equal radii within fuzz, snaps B's lateral onto
    // A's EXACT carrier (dossier 39: snap near-coincidence to exact
    // coincidence BEFORE classification; the radial-gap clearance
    // pin). The honest slice: B's lateral must be bounded only by
    // planes perpendicular to the axis (flat caps), so the radial
    // move keeps every snapped vertex on its other carriers.
    let mut cyl_done: Vec<FaceKey> = Vec::new();
    for fa in a.face_keys() {
        let Some(Surface3::Cylinder(ca)) = a.face_surface3(fa) else {
            continue;
        };
        let za = ca.frame.z;
        let pa0 = ca.frame.origin;
        for fb in b.face_keys() {
            if cyl_done.contains(&fb) {
                continue;
            }
            let Some(Surface3::Cylinder(cb)) = b.face_surface3(fb) else {
                continue;
            };
            if za.cross(cb.frame.z).norm() > 1e-6 {
                continue; // axes not parallel in either sense
            }
            let d = cb.frame.origin - pa0;
            let off_ax = (d - za * d.dot(za)).norm();
            let dr = (cb.radius - ca.radius).abs();
            if (off_ax <= 1e-12 && dr <= 1e-12) || off_ax > fuzz || dr > fuzz {
                continue; // exactly mated already (strict territory), or out of reach
            }
            // Axial overlap, from fin CURVE samples (a circle-bounded
            // lateral has only seam vertices: the vertex-only trap).
            let hspan = |body: &Body, f: FaceKey| -> Option<(f64, f64)> {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for lk in body
                    .faces
                    .get(f)
                    .map(|x| x.loops.clone())
                    .unwrap_or_default()
                {
                    let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
                        continue;
                    };
                    let mut cur = entry;
                    while let Some(fin) = body.fins.get(cur) {
                        for p in body.fin_curve_samples(cur, 8).unwrap_or_default() {
                            let h = (p - pa0).dot(za);
                            lo = lo.min(h);
                            hi = hi.max(h);
                        }
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                }
                (lo <= hi).then_some((lo, hi))
            };
            let (Some((alo, ahi)), Some((blo, bhi))) = (hspan(a, fa), hspan(b, fb)) else {
                continue;
            };
            if blo > ahi + fuzz || alo > bhi + fuzz {
                continue;
            }
            // Flat-cap guard: every neighbour sharing an edge with fb
            // must be a plane perpendicular to the axis.
            let fb_edges = b.face_edge_set(fb);
            let caps_ok = b.face_keys().into_iter().filter(|&fo| fo != fb).all(|fo| {
                if !b.face_edge_set(fo).iter().any(|e| fb_edges.contains(e)) {
                    return true;
                }
                matches!(b.face_surface3(fo),
                    Some(Surface3::Plane(p)) if p.frame.z.cross(za).norm() <= 1e-9)
            });
            if !caps_ok {
                continue;
            }
            let body = snapped.get_or_insert_with(|| b.clone());
            // The new surface: A's exact axis and radius, B's axis
            // sense and angular reference preserved.
            let zb = za * za.dot(cb.frame.z).signum();
            let Some(xb) = (cb.frame.x - zb * cb.frame.x.dot(zb)).try_normalize() else {
                continue;
            };
            let org = pa0 + za * d.dot(za);
            let frame = keel_geom::surface::Frame3 {
                origin: org,
                x: xb,
                y: zb.cross(xb),
                z: zb,
            };
            let Ok(cyl) = keel_geom::surface::Cylinder3::new(frame, ca.radius) else {
                continue;
            };
            let sense = body
                .faces
                .get(fb)
                .and_then(|f| f.surface)
                .map(|(_, s)| s)
                .unwrap_or(true);
            // Vertices: radial reprojection onto the exact carrier.
            let lps = body
                .faces
                .get(fb)
                .map(|f| f.loops.clone())
                .unwrap_or_default();
            let mut vs: Vec<crate::entity::VertexKey> = Vec::new();
            for lk in lps {
                let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                loop {
                    if let Some(v) = body.fin_start_vertex(cur)
                        && !vs.contains(&v)
                    {
                        vs.push(v);
                    }
                    let Some(next) = body.fins.get(cur).map(|x| x.next) else {
                        break;
                    };
                    cur = next;
                    if cur == entry {
                        break;
                    }
                }
            }
            for v in vs {
                if let Some(x) = body.vertices.get_mut(v) {
                    let w = x.point - org;
                    let h = w.dot(za);
                    if let Some(rad) = (w - za * h).try_normalize() {
                        let np = org + za * h + rad * ca.radius;
                        moved = moved.max((np - x.point).norm());
                        x.point = np;
                    }
                }
            }
            // Curves: rims become circles of the exact radius about
            // the exact axis (winding preserved); straight rulings
            // re-fit their snapped endpoints.
            for e in fb_edges {
                let Some((ck, csense)) = body.edges.get(e).and_then(|x| x.curve) else {
                    continue;
                };
                let Some(c) = body.curves.get(ck) else {
                    continue;
                };
                match c {
                    Curve3::Circle(circ) => {
                        let h = (circ.center - org).dot(za);
                        let n_old = circ.x_axis.cross(circ.y_axis);
                        let nz = za * za.dot(n_old).signum();
                        let Some(cx) = (circ.x_axis - nz * circ.x_axis.dot(nz)).try_normalize()
                        else {
                            continue;
                        };
                        if let Ok(nc) = keel_geom::curve::Circle3::new(
                            org + za * h,
                            cx,
                            nz.cross(cx),
                            ca.radius,
                        ) {
                            body.attach_edge_curve(e, Curve3::Circle(nc), csense);
                        }
                    }
                    Curve3::Line(_) => {
                        let Some(ed) = body.edges.get(e) else {
                            continue;
                        };
                        let pts = (
                            body.vertices.get(ed.bounds.0).map(|v| v.point),
                            body.vertices.get(ed.bounds.1).map(|v| v.point),
                        );
                        if let (Some(p0), Some(p1)) = pts
                            && let Ok(l) = keel_geom::curve::Line3::new(p0, p1 - p0)
                        {
                            body.attach_edge_curve(e, Curve3::Line(l), csense);
                        }
                    }
                    _ => {}
                }
            }
            body.attach_face_surface(
                fb,
                crate::entity::SurfaceGeom::Analytic(Surface3::Cylinder(cyl)),
                sense,
            );
            moved = moved.max(off_ax).max(dr);
            cyl_done.push(fb);
        }
    }
    // CONE prepare (task 30, the M6 mirror): a near-mated lateral cone
    // pair (near-coaxial, same axis sense, near-equal taper, near-equal
    // radius at a common plane: the countersink clearance plug) snaps
    // B's lateral onto A's EXACT carrier. Same honest slice as the
    // cylinder: B's lateral bounded only by planes perpendicular to the
    // axis, so the radial move keeps every vertex on its other carriers.
    let mut cone_done: Vec<FaceKey> = Vec::new();
    for fa in a.face_keys() {
        let Some(Surface3::Cone(ca)) = a.face_surface3(fa) else {
            continue;
        };
        let za = ca.frame.z;
        let pa0 = ca.frame.origin;
        let tan_a = ca.half_angle.tan();
        for fb in b.face_keys() {
            if cone_done.contains(&fb) {
                continue;
            }
            let Some(Surface3::Cone(cb)) = b.face_surface3(fb) else {
                continue;
            };
            if za.cross(cb.frame.z).norm() > 1e-6 || za.dot(cb.frame.z) <= 0.0 {
                continue; // axes not parallel in the same taper sense
            }
            let d = cb.frame.origin - pa0;
            let off_ax = (d - za * d.dot(za)).norm();
            // B's radius measured at A's origin plane, and the taper gap.
            let dz = d.dot(za);
            let rb_at_a0 = cb.radius - dz * cb.half_angle.tan();
            let dr = (rb_at_a0 - ca.radius).abs();
            let dtan = (cb.half_angle.tan() - tan_a).abs();
            // Axial overlap from fin CURVE samples (the vertex-only trap).
            let hspan = |body: &Body, f: FaceKey| -> Option<(f64, f64)> {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for lk in body
                    .faces
                    .get(f)
                    .map(|x| x.loops.clone())
                    .unwrap_or_default()
                {
                    let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
                        continue;
                    };
                    let mut cur = entry;
                    while let Some(fin) = body.fins.get(cur) {
                        for p in body.fin_curve_samples(cur, 8).unwrap_or_default() {
                            let h = (p - pa0).dot(za);
                            lo = lo.min(h);
                            hi = hi.max(h);
                        }
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                }
                (lo <= hi).then_some((lo, hi))
            };
            let (Some((alo, ahi)), Some((blo, bhi))) = (hspan(a, fa), hspan(b, fb)) else {
                continue;
            };
            if blo > ahi + fuzz || alo > bhi + fuzz {
                continue;
            }
            // The lateral gap over B's actual span (taper mismatch grows
            // with height) bounds the whole snap movement.
            let span_max = blo.abs().max(bhi.abs());
            let gap = off_ax + dr + dtan * span_max;
            if gap <= 1e-12 || gap > fuzz {
                continue; // exactly mated already, or out of reach
            }
            // Flat-cap guard: every neighbour sharing an edge with fb
            // must be a plane perpendicular to the axis.
            let fb_edges = b.face_edge_set(fb);
            let caps_ok = b.face_keys().into_iter().filter(|&fo| fo != fb).all(|fo| {
                if !b.face_edge_set(fo).iter().any(|e| fb_edges.contains(e)) {
                    return true;
                }
                matches!(b.face_surface3(fo),
                    Some(Surface3::Plane(p)) if p.frame.z.cross(za).norm() <= 1e-9)
            });
            if !caps_ok {
                continue;
            }
            let body = snapped.get_or_insert_with(|| b.clone());
            // The new surface: A's exact axis, taper and radius, B's
            // angular reference preserved; B's origin moved onto A's
            // axis at its own height (radius re-derived there).
            let Some(xb) = (cb.frame.x - za * cb.frame.x.dot(za)).try_normalize() else {
                continue;
            };
            let org = pa0 + za * dz;
            let r_at_org = ca.radius + dz * tan_a;
            let frame = keel_geom::surface::Frame3 {
                origin: org,
                x: xb,
                y: za.cross(xb),
                z: za,
            };
            let Ok(cone) = keel_geom::surface::Cone3::new(frame, r_at_org, ca.half_angle) else {
                continue;
            };
            let sense = body
                .faces
                .get(fb)
                .and_then(|f| f.surface)
                .map(|(_, s)| s)
                .unwrap_or(true);
            // Vertices: radial reprojection onto the exact carrier at
            // their own height (caps perpendicular to the axis stay
            // satisfied by construction).
            let lps = body
                .faces
                .get(fb)
                .map(|f| f.loops.clone())
                .unwrap_or_default();
            let mut vs: Vec<crate::entity::VertexKey> = Vec::new();
            for lk in lps {
                let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                loop {
                    if let Some(v) = body.fin_start_vertex(cur)
                        && !vs.contains(&v)
                    {
                        vs.push(v);
                    }
                    let Some(next) = body.fins.get(cur).map(|x| x.next) else {
                        break;
                    };
                    cur = next;
                    if cur == entry {
                        break;
                    }
                }
            }
            for v in vs {
                if let Some(x) = body.vertices.get_mut(v) {
                    let w = x.point - pa0;
                    let h = w.dot(za);
                    if let Some(rad) = (w - za * h).try_normalize() {
                        let np = pa0 + za * h + rad * (ca.radius + h * tan_a);
                        moved = moved.max((np - x.point).norm());
                        x.point = np;
                    }
                }
            }
            // Curves: rims become circles of the exact local radius
            // about the exact axis; straight rulings re-fit their
            // snapped endpoints.
            for e in fb_edges {
                let Some((ck, csense)) = body.edges.get(e).and_then(|x| x.curve) else {
                    continue;
                };
                let Some(c) = body.curves.get(ck) else {
                    continue;
                };
                match c {
                    Curve3::Circle(circ) => {
                        let h = (circ.center - pa0).dot(za);
                        let n_old = circ.x_axis.cross(circ.y_axis);
                        let nz = za * za.dot(n_old).signum();
                        let Some(cx) = (circ.x_axis - nz * circ.x_axis.dot(nz)).try_normalize()
                        else {
                            continue;
                        };
                        if let Ok(nc) = keel_geom::curve::Circle3::new(
                            pa0 + za * h,
                            cx,
                            nz.cross(cx),
                            ca.radius + h * tan_a,
                        ) {
                            body.attach_edge_curve(e, Curve3::Circle(nc), csense);
                        }
                    }
                    Curve3::Line(_) => {
                        let Some(ed) = body.edges.get(e) else {
                            continue;
                        };
                        let pts = (
                            body.vertices.get(ed.bounds.0).map(|v| v.point),
                            body.vertices.get(ed.bounds.1).map(|v| v.point),
                        );
                        if let (Some(p0), Some(p1)) = pts
                            && let Ok(l) = keel_geom::curve::Line3::new(p0, p1 - p0)
                        {
                            body.attach_edge_curve(e, Curve3::Line(l), csense);
                        }
                    }
                    _ => {}
                }
            }
            body.attach_face_surface(
                fb,
                crate::entity::SurfaceGeom::Analytic(Surface3::Cone(cone)),
                sense,
            );
            moved = moved.max(gap);
            cone_done.push(fb);
        }
    }
    snapped.map(|b2| (b2, moved))
}

/// Local / selective face-pair boolean (parity item 31): the boolean
/// restricted to the intersection seams of the given (target-face,
/// tool-face) pairs only. Imprint, classification, selection, stitch,
/// and EVERY honesty gate run unchanged, so an insufficient selection
/// (one whose seams cannot bound a closed result) DECLINES rather than
/// emitting a wrong body. The coincident-overlap pre-pass is skipped
/// (it re-clones the operands and would remap the caller's face keys);
/// coincident contacts decline as in the general-position MVP.
pub fn boolean_selective(
    a: &Body,
    b: &Body,
    op: BoolOp,
    pairs: &[(FaceKey, FaceKey)],
    tol: f64,
) -> Result<BoolResult, BoolFault> {
    let (all, faults) = seam_curves(a, b, tol);
    let seams: Vec<SeamCurve> = all
        .into_iter()
        .filter(|s| pairs.contains(&(s.face_a, s.face_b)))
        .collect();
    if seams.is_empty() {
        return Err(BoolFault::AssemblyFailed(
            "boolean_selective: no seams from the selected pairs",
        ));
    }
    if let Some(f) = faults
        .iter()
        .find(|f| matches!(f, BoolFault::Tangent(..) | BoolFault::UnassemblableSeam(..)))
        .cloned()
    {
        return Err(f);
    }
    assemble_boolean(a, b, op, tol, &seams, faults, BooleanOptions::default())
}

/// 2D area of the intersection of two discs (radii r1, r2, centre distance d).
fn two_disc_lens_area(r1: f64, r2: f64, d: f64) -> f64 {
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= (r1 - r2).abs() {
        let r = r1.min(r2);
        return core::f64::consts::PI * r * r;
    }
    let a1 = ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1))
        .clamp(-1.0, 1.0)
        .acos();
    let a2 = ((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2))
        .clamp(-1.0, 1.0)
        .acos();
    let tri = 0.5
        * ((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2))
            .max(0.0)
            .sqrt();
    r1 * r1 * a1 + r2 * r2 * a2 - tri
}

/// EXACT cylinder-intersect-sphere volume by 1D integration along the cylinder
/// axis of the closed-form two-disc lens area (the cyl cross-section disc and
/// the sphere's cross-section circle, both in the plane perpendicular to the
/// axis at height h, centres delta apart). A tight, op-agnostic ground truth
/// the boolean's mass must match: a non-coaxial cyl/sphere assembly can then
/// NEVER ship a wrong volume -- watertight or not -- even without a closed form
/// for the trimmed result itself (DECLINE-never-WRONG for the window class).
fn cyl_sphere_inter_volume(
    cy: &keel_geom::surface::Cylinder3,
    hlo: f64,
    hhi: f64,
    sp: &keel_geom::surface::Sphere3,
) -> f64 {
    let axis = cy.frame.z;
    let w = sp.frame.origin - cy.frame.origin;
    let hc = w.dot(axis);
    let delta = (w - axis * hc).norm();
    let lo = hlo.max(hc - sp.radius);
    let hi = hhi.min(hc + sp.radius);
    if hi <= lo {
        return 0.0;
    }
    let n = 4000usize;
    let dh = (hi - lo) / n as f64;
    let mut v = 0.0;
    for i in 0..n {
        let h = lo + (i as f64 + 0.5) * dh;
        let rs2 = sp.radius * sp.radius - (h - hc) * (h - hc);
        if rs2 > 0.0 {
            v += two_disc_lens_area(cy.radius, rs2.sqrt(), delta) * dh;
        }
    }
    v
}

/// If `a`/`b` are a cylinder primitive + a sphere primitive, the EXACT op
/// volume (the cylinder axial extent is read from the body's facets). None when
/// the pair is not cyl + sphere. Used as a tight gate bound for the otherwise-
/// oracle-less cyl/sphere window class.
fn cyl_sphere_op_volume(a: &Body, b: &Body, op: BoolOp) -> Option<f64> {
    let cyl_of = |body: &Body| -> Option<(keel_geom::surface::Cylinder3, f64, f64)> {
        let c = body
            .face_keys()
            .into_iter()
            .find_map(|f| match body.face_surface3(f) {
                Some(Surface3::Cylinder(c)) => Some(c),
                _ => None,
            })?;
        let axis = c.frame.z;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for t in body.facets(None) {
            for p in t {
                let h = (p - c.frame.origin).dot(axis);
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        (lo.is_finite() && hi > lo).then_some((c, lo, hi))
    };
    let sph_of = |body: &Body| -> Option<keel_geom::surface::Sphere3> {
        body.face_keys()
            .into_iter()
            .find_map(|f| match body.face_surface3(f) {
                Some(Surface3::Sphere(s)) => Some(s),
                _ => None,
            })
    };
    let pi = core::f64::consts::PI;
    let (cyl, hlo, hhi, sph, va, vb) = if let (Some((c, lo, hi)), Some(s)) = (cyl_of(a), sph_of(b))
    {
        let (vc, vs) = (
            pi * c.radius * c.radius * (hi - lo),
            4.0 / 3.0 * pi * s.radius.powi(3),
        );
        (c, lo, hi, s, vc, vs)
    } else if let (Some(s), Some((c, lo, hi))) = (sph_of(a), cyl_of(b)) {
        let (vc, vs) = (
            pi * c.radius * c.radius * (hi - lo),
            4.0 / 3.0 * pi * s.radius.powi(3),
        );
        (c, lo, hi, s, vs, vc) // operand A is the sphere here
    } else {
        return None;
    };
    // The oracle is an exact LONE cylinder-vs-LONE sphere truth. cyl_of/sph_of
    // match ANY face, so a COMPOUND operand that merely CONTAINS such a face (a
    // plate with a dome, then drilled) would otherwise be scored as if it were
    // the bare primitive -> a nonsense exact volume that false-declines a
    // correct result (root-B). Fire only when each operand's OWN volume matches
    // the primitive it was detected as (va is A's primitive volume, vb is B's).
    // Use the ANALYTIC mass, not the chordal tessellated volume: a coarsely
    // facetted lone sphere/cone reads several percent under its true volume, so
    // a tessellated test would spuriously DISARM the oracle on a genuine lone
    // primitive -- silently dropping a WRONG-catching constraint. Analytic mass
    // is exact for any quadric+planar primitive, so the match is to ~1e-9 for a
    // true primitive while a compound (block+dome 307 vs sphere 14) is rejected
    // by a wide margin.
    let prim_ok = |body: &Body, vprim: f64| {
        body.mass_properties()
            .is_ok_and(|m| m.volume.is_finite() && (m.volume - vprim).abs() <= 1e-2 * (1.0 + vprim))
    };
    if !(prim_ok(a, va) && prim_ok(b, vb)) {
        return None;
    }
    let inter = cyl_sphere_inter_volume(&cyl, hlo, hhi, &sph);
    Some(match op {
        BoolOp::Intersection => inter,
        BoolOp::Union => va + vb - inter,
        BoolOp::Difference => va - inter,
    })
}

/// EXACT cone-intersect-sphere volume by 1D integration along the cone axis of
/// the closed-form two-disc lens area -- the cone cross-section is a disc of
/// v-VARYING radius r0 + v*m (vs the cylinder's constant r). Tight ground truth
/// for the cone/sphere window class (no closed form for the trimmed result).
fn cone_sphere_inter_volume(
    c: &keel_geom::surface::Cone3,
    vlo: f64,
    vhi: f64,
    sp: &keel_geom::surface::Sphere3,
) -> f64 {
    let axis = c.frame.z;
    let w = sp.frame.origin - c.frame.origin;
    let vc = w.dot(axis);
    let delta = (w - axis * vc).norm();
    let m = c.half_angle.tan();
    let lo = vlo.max(vc - sp.radius);
    let hi = vhi.min(vc + sp.radius);
    if hi <= lo {
        return 0.0;
    }
    let n = 4000usize;
    let dv = (hi - lo) / n as f64;
    let mut vol = 0.0;
    for i in 0..n {
        let v = lo + (i as f64 + 0.5) * dv;
        let rcone = (c.radius + v * m).max(0.0);
        let rs2 = sp.radius * sp.radius - (v - vc) * (v - vc);
        if rs2 > 0.0 && rcone > 0.0 {
            vol += two_disc_lens_area(rcone, rs2.sqrt(), delta) * dv;
        }
    }
    vol
}

/// Solid volume of a cone frustum between axial v in [vlo, vhi]: integral of
/// pi*(r0 + v*m)^2 dv (the cone primitive's own volume, for the op bound).
fn cone_solid_volume(c: &keel_geom::surface::Cone3, vlo: f64, vhi: f64) -> f64 {
    let m = c.half_angle.tan();
    let n = 4000usize;
    let dv = (vhi - vlo) / n as f64;
    let mut vol = 0.0;
    for i in 0..n {
        let v = vlo + (i as f64 + 0.5) * dv;
        let r = (c.radius + v * m).max(0.0);
        vol += core::f64::consts::PI * r * r * dv;
    }
    vol
}

/// Tight EXACT op-volume oracle for a quadric + sphere pair: cyl/sphere, else
/// cone/sphere. None for any other pair. The gate requires the boolean's mass
/// to match this, so the window classes (no closed-form trimmed shape) cannot
/// ship a watertight self-consistent WRONG.
fn quadric_sphere_op_volume(a: &Body, b: &Body, op: BoolOp) -> Option<f64> {
    if let Some(v) = cyl_sphere_op_volume(a, b, op) {
        return Some(v);
    }
    let cone_of = |body: &Body| -> Option<(keel_geom::surface::Cone3, f64, f64)> {
        let c = body
            .face_keys()
            .into_iter()
            .find_map(|f| match body.face_surface3(f) {
                Some(Surface3::Cone(c)) => Some(c),
                _ => None,
            })?;
        let axis = c.frame.z;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for t in body.facets(None) {
            for p in t {
                let v = (p - c.frame.origin).dot(axis);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        (lo.is_finite() && hi > lo).then_some((c, lo, hi))
    };
    let sph_of = |body: &Body| -> Option<keel_geom::surface::Sphere3> {
        body.face_keys()
            .into_iter()
            .find_map(|f| match body.face_surface3(f) {
                Some(Surface3::Sphere(s)) => Some(s),
                _ => None,
            })
    };
    let pi = core::f64::consts::PI;
    let (cone, vlo, vhi, sph, va, vb) =
        if let (Some((c, lo, hi)), Some(s)) = (cone_of(a), sph_of(b)) {
            let vs = 4.0 / 3.0 * pi * s.radius.powi(3);
            (c, lo, hi, s, f64::NAN, vs)
        } else if let (Some(s), Some((c, lo, hi))) = (sph_of(a), cone_of(b)) {
            let vs = 4.0 / 3.0 * pi * s.radius.powi(3);
            (c, lo, hi, s, vs, f64::NAN)
        } else {
            return None;
        };
    let inter = cone_sphere_inter_volume(&cone, vlo, vhi, &sph);
    let vcone = cone_solid_volume(&cone, vlo, vhi);
    let (va, vb) = if va.is_nan() {
        (vcone, vb)
    } else {
        (va, vcone)
    };
    // Lone-primitive guard (see cyl_sphere_op_volume): fire only when each
    // operand IS the bare cone/sphere it was detected as, not a compound body
    // that merely contains such a face -- else a correct compound result gets a
    // nonsense exact volume and is false-declined (root-B). Analytic mass, not
    // the chordal tessellated volume (which on a coarse cone/sphere would read
    // several percent low and spuriously disarm the oracle on a real primitive).
    let prim_ok = |body: &Body, vprim: f64| {
        body.mass_properties()
            .is_ok_and(|m| m.volume.is_finite() && (m.volume - vprim).abs() <= 1e-2 * (1.0 + vprim))
    };
    if !(prim_ok(a, va) && prim_ok(b, vb)) {
        return None;
    }
    Some(match op {
        BoolOp::Intersection => inter,
        BoolOp::Union => va + vb - inter,
        BoolOp::Difference => va - inter,
    })
}

/// A fault that is purely ADVISORY on a body the boolean has INDEPENDENTLY
/// verified correct. The caller clears these only when the result passes the
/// post-condition gate AND re-verifies as a watertight solid against the
/// user-facing mesh (validate() + mass == mesh_volume, 2%) -- the gate's own
/// self-consistency runs against tessellated_volume, which a coarse curved
/// union can satisfy while the render mesh disagrees, so the extra mesh check is
/// what makes clearing safe. Faults accumulated during assembly are then process
/// diagnostics, NOT result verdicts: a verified-correct body that still carries a Coincident
/// / Tangent degeneracy note, or a recovered-imprint hiccup (an unlocated
/// multi-cut seam component, or an "open chain" precondition the assembly
/// continued past), is a CORRECT solid wearing a spurious warning -- which every
/// consumer (and the realistic-CAD harnesses) reasonably reads as "suspect" and
/// rejects, inflating the decline set with non-declines. Independent Monte-Carlo
/// truth confirms the dominant such classes are exact (probe_residual: the top-7
/// nested signatures read mass == mesh == MC to <= 0.04%). These drop on success.
///
/// KEPT (plausibly mark an INCOMPLETE result, so still worth reporting):
/// `UnassemblableSeam` (documented WRONG-if-shipped-seamless), `IntersectionFailed`,
/// any other `AssemblyFailed` message, and any other `Topo` error. The body is
/// byte-identical whether or not the advisory fault is reported, so dropping it
/// never risks DECLINE-never-WRONG.
fn fault_advisory_on_success(f: &BoolFault) -> bool {
    matches!(
        f,
        BoolFault::Coincident(..)
            | BoolFault::Tangent(..)
            | BoolFault::AssemblyFailed("unlocated seam component (non-planar multi-cut face)")
            | BoolFault::Topo(TopoError::Precondition("open chain end not on boundary"))
    )
}

/// The shared post-seam boolean tail: imprint both operands along the
/// given seams, classify, select, stitch, and apply the degeneracy /
/// self-consistency gates. Used by `boolean` (all seams) and
/// `boolean_selective` (a face-pair subset).
fn assemble_boolean(
    a: &Body,
    b: &Body,
    op: BoolOp,
    tol: f64,
    seams: &[SeamCurve],
    mut faults: Vec<BoolFault>,
    opts: BooleanOptions,
) -> Result<BoolResult, BoolFault> {
    let (ia, ib) = {
        let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_NS);
        (
            imprint_operand(a, seams, |s| (s.face_a, s.on_boundary_a), tol, &mut faults),
            imprint_operand(b, seams, |s| (s.face_b, s.on_boundary_b), tol, &mut faults),
        )
    };
    let (class_a, class_b) = {
        let _prof = crate::profile::Scope::new(&crate::profile::CLASSIFY_NS);
        (
            classify_faces(&ia.body, b, tol),
            classify_faces(&ib.body, a, tol),
        )
    };
    let _prof_stitch = crate::profile::Scope::new(&crate::profile::STITCH_NS);
    let kept = select_faces(op, &class_a, &class_b);
    if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
        let dump = |tag: &str, body: &Body, cls: &[(FaceKey, FaceClass)]| {
            for (f, c) in cls {
                let pts = body.face_outer_loop_points(*f);
                let n = pts.len().max(1) as f64;
                let ctr = pts.iter().fold(keel_math::vec::Vec3::ZERO, |s, p| s + *p) * (1.0 / n);
                eprintln!("  class {tag} {f:?} {c:?} ctr {ctr:?}");
            }
        };
        dump("A", &ia.body, &class_a);
        dump("B", &ib.body, &class_b);
    }
    // NON-REGULARIZED union (item 29 Rung 1, dossier 57): the on-
    // OPPOSITE interface fragments (two solids abutting along a
    // coincident face) are retained as DOUBLE-SIDED interior partition
    // walls instead of dropped -- one copy, from operand A by the same
    // convention as the on-on tables. With no interface the result is
    // the regularized one.
    let walls: Vec<KeptFace> = if !opts.regularize && op == BoolOp::Union {
        class_a
            .iter()
            .filter(|(_, c)| *c == FaceClass::OnOther(OnSense::Opposite))
            .map(|&(f, _)| KeptFace {
                operand: Operand::A,
                face: f,
                reversed: false,
            })
            .collect()
    } else {
        Vec::new()
    };
    // TOUCH-ONLY contact (dossier 39 sec 3.3): when selection keeps
    // NOTHING (every fragment classified ON and dropped by the
    // regularized tables; e.g. solids abutting on a shared face, or
    // A == B differenced), the regularized intersection / difference
    // is the clean EMPTY result, not a stitch failure. An empty UNION
    // of solid operands is impossible: that stays a decline.
    if kept.is_empty() && walls.is_empty() {
        if op == BoolOp::Union {
            return Err(BoolFault::AssemblyFailed("union selected no faces"));
        }
        // A DIFFERENCE A - B is empty ONLY when A is contained in B (A subset
        // B, including A == B). If an interior point of A lies strictly OUTSIDE
        // B then part of A survives, so an empty selection is a CLASSIFY FAILURE
        // -- the heavy / equal-radius sphere-sphere difference drops every A
        // fragment, and the op-bound below has lo == 0 for equal radii and
        // cannot catch it (KL5). Re-probe containment directly (gwn over B at A
        // face-interior points nudged inward, the classify mechanism, fresh so
        // it does not trust the failed classify). A genuine touch or true
        // containment keeps its clean empty: no A interior point lies outside B
        // when A subset B, and a touch difference correctly keeps A (non-empty)
        // so it never reaches this path. DECLINE rather than return the wrong
        // empty body.
        if op == BoolOp::Difference {
            let b_tris = b.boundary_triangles();
            // Sample A's whole surface (its tessellation), not one interior
            // point: a single point can sit inside B even when A escapes it.
            // Any A surface point CLEARLY outside B (gwn < 0.25) proves A is not
            // contained in B. A == B keeps its surface ON B (gwn ~ 0.5), so it
            // is not flagged and its empty difference stays valid.
            let a_escapes_b = a
                .boundary_triangles()
                .iter()
                .flatten()
                .any(|&p| crate::winding::gwn_over(&b_tris, p) < 0.25);
            if a_escapes_b {
                return Err(BoolFault::AssemblyFailed(
                    "empty difference but A is not contained in B (classify failure, declined)",
                ));
            }
        }
        // An empty Intersection/Difference is only valid when the op truly
        // yields nothing: disjoint intersection, or a difference whose A is
        // swallowed by B (lo == 0). If the EXACT operand volumes REQUIRE a
        // positive result (lo > 0), an empty selection means classification
        // failed -- the near-tangent thin-lens sphere-difference mis-classes
        // every A fragment to one side, so select keeps nothing and the old
        // path returned a malformed EMPTY body whose mass declines and whose
        // mesh is ~0, slipping the post-condition gate entirely (the one
        // residual silent-malformed escape, dossier 62 bucket c). DECLINE it
        // rather than emit the malformed Ok (DECLINE-never-WRONG).
        let opvol = |x: &Body| {
            x.mass_properties()
                .map(|m| m.volume)
                .unwrap_or_else(|_| x.mesh_volume())
        };
        let (va, vb) = (opvol(a), opvol(b));
        let lo = if op == BoolOp::Difference {
            (va - vb).max(0.0)
        } else {
            0.0 // intersection lower bound is always 0
        };
        let slack = 5e-2 * (1.0 + va.max(vb));
        if va.is_finite() && vb.is_finite() && lo > slack {
            return Err(BoolFault::AssemblyFailed(
                "empty selection violates op-volume bound (declined)",
            ));
        }
        return Ok(BoolResult {
            body: Body::new(),
            faults,
            op,
        });
    }
    // PRIMARY assembly: the identity-preserving import-and-glue (research
    // file 47). It imports each kept fragment carrying its operand's edge
    // identity and glues only the genuinely-coincident cross-operand seam;
    // with the matching seam subdivision (subdivide_seam_ring) the planar
    // seam coedges pair 1:1, so it correctly assembles the thin/oblique
    // cuts the old polygon-soup stitcher mis-built. The soup is RETIRED
    // (dossier 47 Rung 0): a stitch failure DECLINES the boolean rather
    // than building a body the closure invariant cannot protect.
    let body = stitch_by_import(&ia, &ib, &kept, &walls, tol)?;
    // Post-condition: a real solid has positive, finite volume. The
    // scalar Euler identity is necessary but not sufficient (a few faces
    // can satisfy it without bounding a solid), so near-degenerate
    // configurations -- thin slivers, near-coincident touches -- can
    // slip through as Euler-valid yet geometrically degenerate. Decline
    // those honestly rather than return a wrong answer. For a PLANAR
    // result the exact mass-properties volume is the gate (it rejects
    // the slivers the coarse tessellation would accept); for a CURVED
    // result (lens) mass-properties cannot yet integrate the trimmed
    // caps, so the surface-agnostic tessellated volume guards it.
    let curved = body
        .face_keys()
        .iter()
        .any(|&f| !matches!(body.face_surface3(f), Some(Surface3::Plane(_))));
    let ok = if curved {
        // CURVED self-consistency (the M5 honesty-net upgrade): the
        // sense-exact mass and the chordal mesh must agree within a
        // CHORDAL band (2 percent: the adaptive tessellation's worst
        // legitimate deviation on small arcs). The drilled-plate
        // wrong-positive (the lateral spanning the whole drill) sat at
        // 100-plus percent and shipped under the old positive-volume-
        // only check; this band converts that whole class from silent
        // to declined. Bodies whose mass legitimately declines (NURBS
        // corner patches) keep the positive-volume floor.
        //
        // OVER-STRICTNESS NOTE (over-strictness-audit): widening this band to
        // the oracle's 25% recovers correct coarse-tessellation curved bodies
        // (+143 on the 10k soak, FAIL 0) BUT is NOT safe by construction -- it
        // also admits any body whose mass and tess DIVERGE by 2-25% with volume
        // in op-bounds, which can include a wrong body this 2% gate conservatively
        // declines (the soak's 25% band + sampling cannot prove their absence).
        // Recovering those passes SAFELY needs an independent closed-form/refined-
        // tess CONFIRMATION per recovered body, not a blanket band widening. Kept
        // at 2% until that safer recovery exists (DECLINE-never-WRONG > +passes).
        let v = body.tessellated_volume();
        let bm = body.mass_properties().map(|m| m.volume);
        if std::env::var("KEEL_BOOL_DEBUG").is_ok() {
            eprintln!(
                "  curved gate: tess {v} mass {bm:?} mesh {} open_ratio {:.5}",
                body.mesh_volume(),
                body.mesh_open_ratio()
            );
        }
        let self_consistent = match bm {
            Ok(mv) => {
                mv.is_finite()
                    && mv > 0.0
                    && v.is_finite()
                    && (mv - v).abs() <= 2e-2 * (1.0 + mv.abs())
            }
            Err(_) => v.is_finite() && v > 1e-9 * (1.0 + v.abs()),
        };
        // INDEPENDENT op-volume bound (research file 47 / task 49, the sphere
        // cluster the explorer surfaced): the self-consistency check above
        // passes a SELF-CONSISTENT WRONG (mass==mesh agreeing on an impossible
        // value, e.g. a disjoint union reading double) and the mass-declined
        // floor passes a MALFORMED body whose tessellation is positive but
        // wrong. A clean result must satisfy vol(A op B) in [lo, hi] from the
        // EXACT operand volumes, regardless of geometry. Decline a violation
        // (DECLINE-never-WRONG). Skipped only if an operand volume is itself
        // undeterminable. Planar results keep the exact gate below (their mesh
        // is exact, so a wrong there already fails self-consistency).
        // Operand volumes for the bound: mass when available, else the
        // tessellated mesh (a clean primitive operand's mesh is a good
        // estimate). The fallback keeps the bound APPLICABLE even when an
        // operand's OWN mass declines -- exactly the sphere case where an
        // otherwise-malformed result would slip the gate.
        let opvol = |x: &Body| {
            x.mass_properties()
                .map(|m| m.volume)
                .unwrap_or_else(|_| x.mesh_volume())
        };
        let (va, vb) = (opvol(a), opvol(b));
        let bound_ok = if va.is_finite() && vb.is_finite() && va >= 0.0 && vb >= 0.0 {
            let (lo, hi) = match op {
                BoolOp::Union => (va.max(vb), va + vb),
                BoolOp::Intersection => (0.0, va.min(vb)),
                BoolOp::Difference => ((va - vb).max(0.0), va),
            };
            let slack = 5e-2 * (1.0 + hi);
            let in_band = |x: f64| x >= lo - slack && x <= hi + slack;
            // Both the authoritative volume (mass, or the tessellation when
            // mass declines) AND the user-facing mesh must lie in band: a
            // malformed body can pass one measure while the other lies.
            in_band(bm.as_ref().ok().copied().unwrap_or(v)) && in_band(body.mesh_volume())
        } else {
            true // operand volumes undeterminable: cannot bound
        };
        // TIGHT cyl/sphere oracle: for a cylinder + sphere pair the exact op
        // volume is an independent 1D-integral ground truth (the trimmed result
        // has no closed form, but its VOLUME does). Require the authoritative
        // mass to match it -- this catches a watertight self-consistent WRONG
        // (the #48 class the loose op-bound and mass==mesh both miss), so the
        // cyl/sphere WINDOW class is safe to assemble: a correct window passes,
        // a mis-classified one declines. Mass-declined on such a pair -> decline.
        let tight_ok = match quadric_sphere_op_volume(a, b, op) {
            Some(exact) => bm
                .as_ref()
                .ok()
                .map(|&m| (m - exact).abs() <= 2e-2 * (1.0 + exact.abs()))
                .unwrap_or(false),
            None => true,
        };
        // USER-FACING mesh self-consistency (the curved-gate hole, seed
        // 11400715918834827198): `self_consistent` above checks mass against
        // `tessellated_volume` (a SINGLE global-reference signed-tetra sum). For
        // a MALFORMED multi-component union -- the shell->mirror->fillet->union
        // chain that leaves three components topologically disjoint with a
        // spurious genus-1 handle -- the single-reference tess can AGREE with
        // mass (both ~10009, the true |A u B|, confirmed by Monte-Carlo) while
        // the USER-FACING `mesh_volume` (summed PER CONNECTED COMPONENT, each
        // recentred on its own centroid) COLLAPSES to ~5242 (a 48% deficit).
        // The op-bound passed it (the collapsed value squeaked above the union
        // floor) and the watertight net passed it (open_ratio 0.03 < 0.05), so
        // the body shipped Ok with mass != mesh -- a DECLINE-never-WRONG
        // violation the realsoak oracle flags `FAIL mass!=mesh-curved`.
        //
        // The gate must DECLINE exactly the bodies that oracle calls WRONG, no
        // more. The oracle's curved-WRONG condition (realsoak.rs `audit`) is a
        // GROSS relative disagreement, `|mass-mesh| / max(|mass|,|mesh|) > 0.25`
        // -- a deficit far beyond any legitimate chordal/junction tessellation
        // loss (the worst correct curved primitive meshes ~4% under; a correct
        // small bicylinder lens reaches ~20%, still < 25%). Mirror it EXACTLY,
        // against the same user-facing `mesh_volume`, so a body the oracle would
        // accept (a coarse-but-correct curved mesh, <=25% deficit, mass in
        // band) still assembles while the malformed >25% collapse declines.
        // A mass-declined curved result keeps the positive-volume floor (mesh
        // cannot be cross-checked; the other gates carry it). PURELY ADDITIVE:
        // can only DECLINE more, never admit a body the prior gates rejected.
        let mesh_consistent = match bm {
            Ok(mv) if mv > 0.0 => {
                let mesh = body.mesh_volume();
                let rel = (mv - mesh).abs() / mv.abs().max(mesh.abs()).max(1e-9);
                mesh.is_finite() && rel <= 0.25
            }
            _ => true,
        };
        self_consistent && bound_ok && tight_ok && mesh_consistent
    } else if let Ok(m) = body.mass_properties() {
        // SELF-CONSISTENCY gate (research file 47): for a well-formed
        // all-planar body the sense-exact mass_properties and the
        // sense-tessellated mesh_volume agree exactly (polygonal
        // tessellation is exact). A disagreement means assembly produced a
        // geometrically WRONG body that happens to be Euler-valid with
        // positive volume -- e.g. an oblique chamfer cut whose cut face was
        // dropped/mis-stitched (mass != mesh). Decline honestly rather than
        // LIE. (Validate is necessary but not sufficient; making such cuts
        // WORK is the boolean-assembly milestone, this stops the kernel
        // returning a wrong-positive body in the meantime.)
        let mv = body.mesh_volume();
        m.volume.is_finite()
            && m.volume > 0.0
            && (m.volume - mv).abs() <= 1e-3 * (1.0 + m.volume.abs())
    } else {
        false
    };
    // A valid solid never has a non-finite or NEGATIVE mesh volume. A NaN or
    // negative mesh is a broken (inverted/degenerate) result that the volume
    // gates above can miss for TINY bodies: the near-tangent sphere^sphere
    // thin lens returned mass 6.3e-4 / mesh -8.3e-4 against a true 1.38e-2, and
    // every gate band's (1 + vol) term dwarfed that gap. Reject it (a valid
    // body, including the clean empty result, has mesh >= 0).
    let mesh_vol = body.mesh_volume();
    let ok = ok && mesh_vol.is_finite() && mesh_vol >= -tol.max(1e-9);
    // WATERTIGHTNESS net: a correct boolean result is a CLOSED oriented mesh
    // (net triangle area-vector ~ 0). A non-watertight result (cracked /
    // dropped-face / mis-stitched) slips the mass==mesh self-consistency gate
    // -- mass and a non-watertight mesh can AGREE on a WRONG value (the #48
    // silent class: large offset sphere/sphere lenses read ~18-33% over the
    // exact lens volume yet mass==mesh, in-bounds). The residual is built from
    // edge vectors so it is translation-invariant (no far-origin cancellation
    // that the per-component volume recenter must otherwise fix). Decline.
    //
    // THRESHOLD (Add 287): a CORRECT curved-compound body carries a small,
    // genuine chordal-junction residual where a curved cap meets a planar face
    // on a multi-loop face -- e.g. a plate with a dome AND a drilled hole reads
    // 0.018, mass exact. The #48 silent WRONGs sit at 0.25+ (a 14x gap). The old
    // 1e-2 (25x below the wrong) false-declined those correct compounds; 5e-2
    // passes them while still catching the wrong class with a 5x margin. Guarded
    // both ways: `large_offset_sphere_intersection_never_silent_wrong` asserts
    // the 0.25 class still declines, and the soak's three-bucket oracle (WRONG=0)
    // catches any wrong that the looser net would admit.
    let ok = ok && body.mesh_open_ratio() <= 5e-2;
    if !ok {
        return Err(BoolFault::AssemblyFailed(
            "degenerate or self-inconsistent result (mass != mesh)",
        ));
    }
    // Drop advisory process-warnings, but ONLY on a body that INDEPENDENTLY
    // verifies as a correct watertight solid: validate() passes AND the analytic
    // mass equals the USER-FACING mesh_volume within a tight 2% band. The gate
    // above proves self-consistency against `tessellated_volume`, which can read
    // consistent (2%) while the render `mesh_volume` disagrees on a coarse
    // curved-union mesh -- exactly the block U cone the soak flags FAIL:mass-mesh.
    // 2% is stricter than the soak oracle's 6%, so any body whose advisory fault
    // we clear here PROVABLY passes that oracle; a body the soak would reject
    // keeps its honest warning (DECLINE-never-WRONG). See `fault_advisory_on_success`.
    let faults: Vec<BoolFault> = if faults.iter().any(fault_advisory_on_success)
        && body.validate().is_ok()
        && body.mass_properties().is_ok_and(|m| {
            m.volume.is_finite()
                && mesh_vol.is_finite()
                && (m.volume - mesh_vol).abs() <= 2e-2 * (1.0 + m.volume.abs())
        }) {
        faults
            .into_iter()
            .filter(|f| !fault_advisory_on_success(f))
            .collect()
    } else {
        faults
    };
    Ok(BoolResult { body, faults, op })
}

/// Apply `op` against `target` for each tool in `tools`, in order
/// (parity item 30: multiple tool bodies per boolean). Union and
/// intersection accumulate; difference subtracts each tool in turn. An
/// empty tool list returns the target unchanged.
pub fn boolean_multi(
    target: &Body,
    tools: &[&Body],
    op: BoolOp,
    tol: f64,
) -> Result<BoolResult, BoolFault> {
    let mut faults = Vec::new();
    let mut acc: Option<Body> = None;
    for &tool in tools {
        let base = acc.as_ref().unwrap_or(target);
        let res = boolean(base, tool, op, tol)?;
        faults.extend(res.faults);
        acc = Some(res.body);
    }
    Ok(BoolResult {
        body: acc.unwrap_or_else(|| target.clone()),
        faults,
        op,
    })
}

/// Imprint the surface-surface intersection of two bodies onto BOTH as
/// shared edges, WITHOUT combining them (parity item 32: boolean
/// imprint-only / PK_BODY_imprint). Returns the two operands, each split
/// along the intersection curve. Coincident/tangent face pairs decline
/// as for a full boolean.
pub fn imprint(a: &Body, b: &Body, tol: f64) -> Result<(Body, Body), BoolFault> {
    let (seams, mut faults) = seam_curves(a, b, tol);
    if let Some(f) = faults
        .iter()
        .find(|f| matches!(f, BoolFault::Coincident(..) | BoolFault::Tangent(..)))
        .cloned()
    {
        return Err(f);
    }
    let ia = imprint_operand(a, &seams, |s| (s.face_a, s.on_boundary_a), tol, &mut faults);
    let ib = imprint_operand(b, &seams, |s| (s.face_b, s.on_boundary_b), tol, &mut faults);
    Ok((ia.body, ib.body))
}

/// Endpoints of a seam curve (sample at the parameter ends; closed
/// curves return the seam point twice).
pub(crate) fn curve_point(c: &keel_geom::curve::Curve3, t: f64) -> keel_math::vec::Vec3 {
    use keel_geom::curve::Curve3;
    match c {
        Curve3::Nurbs(n) => {
            let (a, b) = n.domain();
            n.point(a + t * (b - a))
        }
        Curve3::Line(l) => l.point(t),
        Curve3::Circle(ci) => ci.point(core::f64::consts::TAU * t),
        Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
    }
}

fn curve_endpoints(c: &keel_geom::curve::Curve3) -> (keel_math::vec::Vec3, keel_math::vec::Vec3) {
    (curve_point(c, 0.0), curve_point(c, 1.0))
}

/// True if the closed seam `curve` ENCIRCLES `axis` through `origin` -- a
/// non-contractible wrap (the off-axis cyl/sphere cut is a NON-planar NURBS loop
/// spanning all azimuths, z = +-sqrt(R^2 - delta^2 - r^2 ... ) varying with
/// theta) versus a contractible window/hole. Sums the UNWRAPPED azimuth advance
/// about the axis over the sampled loop: an encircling wrap nets ~+-2pi, a
/// window ~0. `closed_curve_center_axis` only catches a coaxial PLANAR circle,
/// so a tilted sphere-cut wrap was mis-routed to the interior-ring (hole)
/// imprint, leaving the lateral whole-with-holes (winding 0 -> degenerate mass)
/// instead of split into bands.
fn curve_encircles_axis(
    curve: &keel_geom::curve::Curve3,
    origin: keel_math::vec::Vec3,
    axis: keel_math::vec::Vec3,
) -> bool {
    let Some(ax) = axis.try_normalize() else {
        return false;
    };
    let t = if ax.x.abs() < 0.9 {
        keel_math::vec::Vec3::new(1.0, 0.0, 0.0)
    } else {
        keel_math::vec::Vec3::new(0.0, 1.0, 0.0)
    };
    let Some(ex) = (t - ax * t.dot(ax)).try_normalize() else {
        return false;
    };
    let ey = ax.cross(ex);
    let n = 96;
    let mut total = 0.0;
    let mut prev: Option<f64> = None;
    for i in 0..=n {
        let p = curve_point(curve, i as f64 / n as f64);
        let w = p - origin;
        let wp = w - ax * w.dot(ax);
        if wp.norm() < 1e-12 {
            return false; // a point on the axis: azimuth undefined
        }
        let th = wp.dot(ey).atan2(wp.dot(ex));
        if let Some(pv) = prev {
            let mut d = th - pv;
            while d > core::f64::consts::PI {
                d -= core::f64::consts::TAU;
            }
            while d < -core::f64::consts::PI {
                d += core::f64::consts::TAU;
            }
            total += d;
        }
        prev = Some(th);
    }
    total.abs() > core::f64::consts::PI
}

/// Distance from point `p` to the segment `a`-`b` in 3D.
pub(crate) fn seg_dist3(
    p: keel_math::vec::Vec3,
    a: keel_math::vec::Vec3,
    b: keel_math::vec::Vec3,
) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 < 1e-300 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    (p - (a + ab * t)).norm()
}

/// Build a closed degree-1 (polyline) NURBS through `nodes`, returning
/// to the first node. point(0) == point(end) so it reads as a single
/// closed edge to imprint_closed_curve.
fn closed_polyline_nurbs(
    nodes: &[keel_math::vec::Vec3],
) -> Option<keel_geom::nurbs_curve::NurbsCurve> {
    if nodes.len() < 3 {
        return None;
    }
    let mut ctrl = nodes.to_vec();
    ctrl.push(nodes[0]); // close
    let m = ctrl.len();
    let mut knots = vec![0.0, 0.0];
    for i in 1..(m - 1) {
        knots.push(i as f64);
    }
    knots.push((m - 1) as f64);
    knots.push((m - 1) as f64);
    keel_geom::nurbs_curve::NurbsCurve::new(1, knots, ctrl, None).ok()
}

/// A straight degree-1 NURBS segment between two points.
fn seg_curve(
    p0: keel_math::vec::Vec3,
    p1: keel_math::vec::Vec3,
) -> Option<keel_geom::curve::Curve3> {
    keel_geom::nurbs_curve::NurbsCurve::new(1, vec![0., 0., 1., 1.], vec![p0, p1], None)
        .ok()
        .map(keel_geom::curve::Curve3::Nurbs)
}

/// Assemble a face's seam segments into a single open chain (path), if
/// they form one: exactly two degree-1 endpoints, every interior node
/// degree 2. Returns the ordered points end-to-end. A single segment is
/// the trivial two-node chain.
/// Partition a face's seam indices into connected components by
/// open-endpoint adjacency (within `tol`). Closed seams stand alone.
/// Component order and each component's member order follow the input
/// (deterministic). Values are the global seam indices from `members`.
fn seam_components(members: &[usize], seams: &[SeamCurve], tol: f64) -> Vec<Vec<usize>> {
    let n = members.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while parent[r] != r {
            r = parent[r];
        }
        let mut c = i;
        while parent[c] != c {
            let nx = parent[c];
            parent[c] = r;
            c = nx;
        }
        r
    }
    let eps: Vec<_> = members
        .iter()
        .map(|&i| curve_endpoints(&seams[i].curve))
        .collect();
    for i in 0..n {
        if seams[members[i]].closed {
            continue;
        }
        for j in (i + 1)..n {
            if seams[members[j]].closed {
                continue;
            }
            let (ai, bi) = eps[i];
            let (aj, bj) = eps[j];
            let touch = (ai - aj).norm() <= tol
                || (ai - bj).norm() <= tol
                || (bi - aj).norm() <= tol
                || (bi - bj).norm() <= tol;
            if touch {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[rj] = ri;
                }
            }
        }
    }
    let mut comps: Vec<Vec<usize>> = Vec::new();
    let mut slot_of_root: std::collections::BTreeMap<usize, usize> =
        std::collections::BTreeMap::new();
    for (i, &m) in members.iter().enumerate() {
        let r = find(&mut parent, i);
        let slot = *slot_of_root.entry(r).or_insert_with(|| {
            comps.push(Vec::new());
            comps.len() - 1
        });
        comps[slot].push(m);
    }
    comps
}

/// The current face of `body` whose PLANAR region contains `p`:
/// on-plane within `tol`, and inside by 2D crossing number accumulated
/// over ALL the face's loops (outer + inner rings, orientation-free).
/// Used to relocate a seam component after an earlier component's
/// imprint split its original face. Planar faces only; multi-component
/// imprints on curved faces stay a fault.
impl Body {
    /// Does `curve` lie entirely along this face's existing boundary
    /// edges (within a loose tolerance)? Such a "seam" is already
    /// topology: the rim-contact class of file 39 sec 3.2.
    pub(crate) fn curve_on_face_boundary_edges(
        &self,
        face: FaceKey,
        curve: &keel_geom::curve::Curve3,
        tol: f64,
    ) -> bool {
        let etol = tol.max(1e-7) * 10.0;
        // Boundary samples from every edge of every loop.
        let mut bnd: Vec<keel_math::vec::Vec3> = Vec::new();
        for lk in self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default()
        {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            loop {
                if let Some(s) = self.fin_curve_samples(cur, 24) {
                    bnd.extend(s);
                } else if let Some(v) = self.fin_start_vertex(cur)
                    && let Some(x) = self.vertices.get(v)
                {
                    bnd.push(x.point);
                }
                let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                    break;
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
        }
        if bnd.is_empty() {
            return false;
        }
        (0..9).all(|i| {
            let p = curve_point(curve, i as f64 / 8.0);
            bnd.iter().any(|q| (*q - p).norm() <= etol)
        })
    }
}

/// Curved companion of `planar_face_containing` (the drill lateral
/// carries TWO wrap circles: after the first splits it, the second
/// must relocate onto the descendant band): a cylinder face whose
/// surface passes through `p` and whose boundary-vertex height range
/// contains `p`'s height. Wrap circles are full rings, so the angular
/// extent needs no check here.
impl Body {
    /// Pick the face whose trimmed region contains `p` (within `tol` of
    /// its surface): the consumer-facing point-pick (planar faces first,
    /// then curved). `None` when `p` is not on the boundary.
    pub fn pick_face(&self, p: keel_math::vec::Vec3, tol: f64) -> Option<FaceKey> {
        planar_face_containing(self, p, tol).or_else(|| curved_face_containing(self, p, tol))
    }
}

/// Min/max height of a face's loop vertices along `axis` (relative to `org`).
fn face_height_band(
    body: &Body,
    fk: FaceKey,
    org: keel_math::vec::Vec3,
    axis: keel_math::vec::Vec3,
) -> Option<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for lk in body
        .faces
        .get(fk)
        .map(|f| f.loops.clone())
        .unwrap_or_default()
    {
        let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
            continue;
        };
        let mut cur = entry;
        loop {
            if let Some(v) = body.fin_start_vertex(cur)
                && let Some(x) = body.vertices.get(v)
            {
                let hv = (x.point - org).dot(axis);
                lo = lo.min(hv);
                hi = hi.max(hv);
            }
            let Some(next) = body.fins.get(cur).map(|f| f.next) else {
                break;
            };
            cur = next;
            if cur == entry {
                break;
            }
        }
    }
    if lo.is_finite() { Some((lo, hi)) } else { None }
}

/// The axis of the latitude circle(s) bounding a sphere face (the coaxial
/// cut axis), from the first circular loop edge.
/// Signed solid angle a closed loop subtends at the sphere centre about the
/// direction `dir` (Van Oosterom-Strackee / spherical winding). |omega| ~ 2*pi
/// when the loop encircles `dir` (the point lies in the cap the loop bounds);
/// ~ 0 when it does not. Frame-free. `samples` are the loop's 3D points.
pub(crate) fn loop_solid_angle_about(
    samples: &[keel_math::vec::Vec3],
    center: keel_math::vec::Vec3,
    dir: keel_math::vec::Vec3,
) -> f64 {
    use keel_math::vec::Vec3;
    let Some(d0) = dir.try_normalize() else {
        return 0.0;
    };
    // Project each sample direction onto the plane perpendicular to d0 and
    // accumulate the swept azimuth; a full +-2*pi means the loop winds d0.
    let t = if d0.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let Some(ex) = (t - d0 * t.dot(d0)).try_normalize() else {
        return 0.0;
    };
    let ey = d0.cross(ex);
    let mut total = 0.0;
    let mut prev: Option<f64> = None;
    for s in samples {
        let w = *s - center;
        let wp = w - d0 * w.dot(d0);
        if wp.norm() < 1e-12 {
            continue;
        }
        let th = wp.dot(ey).atan2(wp.dot(ex));
        if let Some(pv) = prev {
            let mut delta = th - pv;
            while delta > core::f64::consts::PI {
                delta -= core::f64::consts::TAU;
            }
            while delta < -core::f64::consts::PI {
                delta += core::f64::consts::TAU;
            }
            total += delta;
        }
        prev = Some(th);
    }
    total
}

/// The sphere face (fragment) whose trimmed region contains the on-sphere point
/// `p`, by a FRAME-FREE 3D test: for each loop, the loop bounds a spherical cap
/// (the side its winding marks); `p` is in the region iff it is inside the
/// outer loop's cap (or the outer loop is the degenerate meridian seam, which
/// bounds the whole sphere) and OUTSIDE every inner-hole loop's cap. This is the
/// robust relocation primitive for the cyl/sphere wrap, where a sphere fragment
/// is bounded by the seam plus contractible NURBS rings (the UV winding test is
/// degenerate on the pole-to-pole seam, so it cannot be used here).
fn sphere_face_containing_3d(body: &Body, p: keel_math::vec::Vec3, tol: f64) -> Option<FaceKey> {
    for fk in body.face_keys() {
        let Some(Surface3::Sphere(sp)) = body.face_surface3(fk) else {
            continue;
        };
        let center = sp.frame.origin;
        if ((p - center).norm() - sp.radius).abs() > tol.max(1e-6) {
            continue;
        }
        if body.face_covers_closed_surface(fk) {
            return Some(fk);
        }
        let Some(f) = body.faces.get(fk) else {
            continue;
        };
        let pd = p - center;
        let mut inside = true;
        for (li, &lk) in f.loops.iter().enumerate() {
            let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            // Collect this loop's 3D samples and detect whether it is the
            // pole-to-pole meridian seam (a non-bounding outer loop).
            let mut samples: Vec<keel_math::vec::Vec3> = Vec::new();
            let mut is_seam = true;
            let mut cur = entry;
            loop {
                let Some(fin) = body.fins.get(cur) else { break };
                let seam_fin = body
                    .edges
                    .get(fin.edge)
                    .map(|e| {
                        e.radial.len() >= 2
                            && e.radial.iter().all(|&rf| {
                                body.fins
                                    .get(rf)
                                    .and_then(|x| body.loops.get(x.owner))
                                    .map(|x| x.face)
                                    == Some(fk)
                            })
                    })
                    .unwrap_or(false);
                if !seam_fin {
                    is_seam = false;
                }
                if let Some(pts) = body.fin_curve_samples(cur, 24) {
                    samples.extend(pts);
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
            if samples.len() < 4 {
                continue;
            }
            let omega = loop_solid_angle_about(&samples, center, pd);
            let encircles = omega.abs() > core::f64::consts::PI;
            if li == 0 {
                // Outer loop: a seam outer bounds the whole sphere (p always
                // on the interior side); a real outer ring must encircle p.
                if !is_seam && !encircles {
                    inside = false;
                    break;
                }
            } else if encircles {
                // Inner hole that encircles p: p is in the hole, not the face.
                inside = false;
                break;
            }
        }
        if inside {
            return Some(fk);
        }
    }
    None
}

fn sphere_face_cut_axis(body: &Body, fk: FaceKey) -> Option<keel_math::vec::Vec3> {
    for lk in body
        .faces
        .get(fk)
        .map(|f| f.loops.clone())
        .unwrap_or_default()
    {
        let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
            continue;
        };
        let mut cur = entry;
        loop {
            if let Some((ck, _)) = body
                .fins
                .get(cur)
                .and_then(|x| body.edges.get(x.edge))
                .and_then(|e| e.curve)
                && let Some(cv) = body.curves.get(ck)
                && let Some((_, ax)) = closed_curve_center_axis(cv)
            {
                return Some(ax);
            }
            let Some(next) = body.fins.get(cur).map(|f| f.next) else {
                break;
            };
            cur = next;
            if cur == entry {
                break;
            }
        }
    }
    None
}

/// The curved face (cylinder / cone / sphere) whose trimmed extent contains
/// `p`. Used to relocate a seam component onto the descendant fragment that
/// now holds it after an earlier split (the multi-cut imprint path). Cylinder
/// and cone use a height band along their axis; the sphere uses the band
/// along its latitude-cut axis, extended by the cap interior point.
pub(crate) fn curved_face_containing(
    body: &Body,
    p: keel_math::vec::Vec3,
    tol: f64,
) -> Option<FaceKey> {
    let etol = tol.max(1e-7);
    body.face_keys().into_iter().find(|&fk| {
        match body.face_surface3(fk) {
            Some(Surface3::Cylinder(c)) => {
                let d = p - c.frame.origin;
                let h = d.dot(c.frame.z);
                if ((d - c.frame.z * h).norm() - c.radius).abs() > etol {
                    return false;
                }
                matches!(face_height_band(body, fk, c.frame.origin, c.frame.z),
                    Some((lo, hi)) if h >= lo - tol && h <= hi + tol)
            }
            Some(Surface3::Cone(c)) => {
                let d = p - c.frame.origin;
                let h = d.dot(c.frame.z);
                let r_at = (c.radius + h * c.half_angle.tan()).abs();
                if ((d - c.frame.z * h).norm() - r_at).abs() > etol {
                    return false;
                }
                matches!(face_height_band(body, fk, c.frame.origin, c.frame.z),
                    Some((lo, hi)) if h >= lo - tol && h <= hi + tol)
            }
            Some(Surface3::Sphere(sp)) => {
                if ((p - sp.frame.origin).norm() - sp.radius).abs() > etol {
                    return false;
                }
                // A pristine full sphere (closed cover: its only edge is the
                // meridian seam, appearing twice) contains any on-surface
                // point. Detect it directly so the FIRST wrap-band component
                // locates the un-imprinted sphere (the relocation otherwise
                // failed on sphere_face_cut_axis returning the seam axis, which
                // dropped the cyl/sphere band split entirely).
                if body.face_covers_closed_surface(fk) {
                    return true;
                }
                // NURBS-rim wrap band/cap (dossier 68): a wrap-split sphere face
                // is bounded by an encircling NON-circular NURBS rim plus
                // meridian sub-arcs, so sphere_face_cut_axis reads the meridian
                // axis and the latitude-band test below is unreliable. The
                // winding-number UV containment handles an arbitrary trim
                // directly, so try it first for such a face.
                if let Ok(pr) = Surface3::Sphere(sp.clone()).project(p) {
                    let pin = body.point_in_face_uv(fk, (pr.u, pr.v), 1e-6 + etol);
                    if matches!(
                        pin,
                        crate::pmc::UvClass::In | crate::pmc::UvClass::OnBoundary
                    ) {
                        return true;
                    }
                }
                let Some(axis) = sphere_face_cut_axis(body, fk) else {
                    return false;
                };
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                if let Some((a, b)) = face_height_band(body, fk, sp.frame.origin, axis) {
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
                // The cap interior point bounds the pole side of a single-rim cap.
                if let Some(q) = body.sphere_face_interior_point(fk) {
                    let hq = (q - sp.frame.origin).dot(axis);
                    lo = lo.min(hq);
                    hi = hi.max(hq);
                }
                let h = (p - sp.frame.origin).dot(axis);
                lo.is_finite() && h >= lo - tol && h <= hi + tol
            }
            _ => false,
        }
    })
}

fn planar_face_containing(body: &Body, p: keel_math::vec::Vec3, tol: f64) -> Option<FaceKey> {
    for fk in body.face_keys() {
        let Some(Surface3::Plane(pl)) = body.face_surface3(fk) else {
            continue;
        };
        if (p - pl.frame.origin).dot(pl.frame.z).abs() > tol {
            continue;
        }
        let to2 = |q: keel_math::vec::Vec3| {
            let w = q - pl.frame.origin;
            (w.dot(pl.frame.x), w.dot(pl.frame.y))
        };
        let (px, py) = to2(p);
        let mut inside = false;
        let mut any_loop = false;
        let Some(f) = body.faces.get(fk) else {
            continue;
        };
        for &lk in &f.loops {
            let poly = body.loop_polygon(lk);
            if poly.len() < 3 {
                continue;
            }
            any_loop = true;
            let m = poly.len();
            for k in 0..m {
                let (x1, y1) = to2(poly[k]);
                let (x2, y2) = to2(poly[(k + 1) % m]);
                if (y1 > py) != (y2 > py) && px < x1 + (py - y1) * (x2 - x1) / (y2 - y1) {
                    inside = !inside;
                }
            }
        }
        if any_loop && inside {
            return Some(fk);
        }
    }
    None
}

fn assemble_open_chain(
    segs: &[(keel_math::vec::Vec3, keel_math::vec::Vec3)],
    tol: f64,
) -> Option<Vec<keel_math::vec::Vec3>> {
    use keel_math::vec::Vec3;
    if segs.is_empty() {
        return None;
    }
    let mut nodes: Vec<Vec3> = Vec::new();
    let idx = |p: Vec3, nodes: &mut Vec<Vec3>| -> usize {
        for (i, q) in nodes.iter().enumerate() {
            if (*q - p).norm() <= tol.max(1e-9) {
                return i;
            }
        }
        nodes.push(p);
        nodes.len() - 1
    };
    let mut adj: Vec<(usize, usize)> = Vec::new();
    for (p0, p1) in segs {
        let i = idx(*p0, &mut nodes);
        let j = idx(*p1, &mut nodes);
        if i == j {
            return None;
        }
        adj.push((i, j));
    }
    let n = nodes.len();
    if n != segs.len() + 1 {
        return None; // a simple path has #nodes == #edges + 1
    }
    let mut deg = vec![0usize; n];
    for &(i, j) in &adj {
        deg[i] += 1;
        deg[j] += 1;
    }
    let ends: Vec<usize> = (0..n).filter(|&i| deg[i] == 1).collect();
    if ends.len() != 2 || (0..n).any(|i| deg[i] != 1 && deg[i] != 2) {
        return None;
    }
    // Walk from one endpoint to the other.
    let mut order = vec![ends[0]];
    let mut used = vec![false; adj.len()];
    let mut cur = ends[0];
    for _ in 0..adj.len() {
        let mut moved = false;
        for (e, &(i, j)) in adj.iter().enumerate() {
            if used[e] {
                continue;
            }
            let nxt = if i == cur {
                Some(j)
            } else if j == cur {
                Some(i)
            } else {
                None
            };
            if let Some(nx) = nxt {
                used[e] = true;
                cur = nx;
                order.push(nx);
                moved = true;
                break;
            }
        }
        if !moved {
            break;
        }
    }
    if order.len() == n {
        Some(order.into_iter().map(|i| nodes[i]).collect())
    } else {
        None
    }
}

/// Assemble a face's seam segments into the ordered node cycle of a
/// single closed loop, if they form one. Returns the corner points in
/// order (without the closing repeat). Open or multi-component sets
/// return None.
fn assemble_closed_loop(
    segs: &[(keel_math::vec::Vec3, keel_math::vec::Vec3)],
    tol: f64,
) -> Option<Vec<keel_math::vec::Vec3>> {
    use keel_math::vec::Vec3;
    if segs.len() < 3 {
        return None;
    }
    // Unique nodes.
    let mut nodes: Vec<Vec3> = Vec::new();
    let idx = |p: Vec3, nodes: &mut Vec<Vec3>| -> usize {
        for (i, q) in nodes.iter().enumerate() {
            if (*q - p).norm() <= tol.max(1e-9) {
                return i;
            }
        }
        nodes.push(p);
        nodes.len() - 1
    };
    let mut adj: Vec<(usize, usize)> = Vec::new();
    for (p0, p1) in segs {
        let i = idx(*p0, &mut nodes);
        let j = idx(*p1, &mut nodes);
        if i == j {
            return None;
        }
        adj.push((i, j));
    }
    // Every node must have degree exactly 2 for a single closed loop.
    let n = nodes.len();
    if n != segs.len() {
        return None; // a clean cycle has #nodes == #edges
    }
    let mut deg = vec![0usize; n];
    for &(i, j) in &adj {
        deg[i] += 1;
        deg[j] += 1;
    }
    if deg.iter().any(|&d| d != 2) {
        return None;
    }
    // Walk the cycle.
    let mut order = vec![0usize];
    let mut used = vec![false; adj.len()];
    let mut cur = 0usize;
    for _ in 0..n {
        let mut moved = false;
        for (e, &(i, j)) in adj.iter().enumerate() {
            if used[e] {
                continue;
            }
            let nxt = if i == cur {
                Some(j)
            } else if j == cur {
                Some(i)
            } else {
                None
            };
            if let Some(nx) = nxt {
                used[e] = true;
                cur = nx;
                if order.len() < n {
                    order.push(nx);
                }
                moved = true;
                break;
            }
        }
        if !moved {
            break;
        }
    }
    if order.len() == n && cur == 0 {
        Some(order.into_iter().map(|i| nodes[i]).collect())
    } else {
        None
    }
}

impl Body {
    /// A boundary edge whose 3D segment passes within `tol` of `p`,
    /// with `p` strictly interior to the segment (not at an endpoint).
    pub(crate) fn edge_containing_point(
        &self,
        p: keel_math::vec::Vec3,
        tol: f64,
    ) -> Option<crate::entity::EdgeKey> {
        for id in self.entity_ids() {
            let AnyKey::Edge(ek) = self.lookup(id)? else {
                continue;
            };
            // Curve-aware: a point strictly INTERIOR to a CONIC ARC edge (a
            // cylinder/cap rim circle, a cone rim) -- the straight chord test
            // below cannot see it (a point mid-arc is a chord-sagitta away),
            // and a FULL-circle rim (bounds.0 == bounds.1) it skips entirely
            // (len2 == 0). conic_arc_split_rel uses the edge's stored arc_sweep
            // so the directed arc is unambiguous (and it already enforces the
            // strict-interior / on-curve predicate). This is what lets the
            // imprint presplit land a milling seam endpoint on a curved rim.
            if conic_arc_split_rel(self, ek, p, tol).is_some() {
                return Some(ek);
            }
            let Some(e) = self.edges.get(ek) else {
                continue;
            };
            let (Some(a), Some(b)) = (
                self.vertices.get(e.bounds.0).map(|v| v.point),
                self.vertices.get(e.bounds.1).map(|v| v.point),
            ) else {
                continue;
            };
            let ab = b - a;
            let len2 = ab.dot(ab);
            if len2 <= 0.0 {
                continue;
            }
            let t = (p - a).dot(ab) / len2;
            if t <= 1e-7 || t >= 1.0 - 1e-7 {
                continue;
            }
            if ((a + ab * t) - p).norm() <= tol {
                return Some(ek);
            }
        }
        None
    }

    /// The outer-loop fin of `face` ending at a vertex within `tol` of
    /// `p`.
    pub(crate) fn loop_fin_ending_at_point(
        &self,
        face: FaceKey,
        p: keel_math::vec::Vec3,
        tol: f64,
    ) -> Option<FinKey> {
        let lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())?;
        let entry = self.loops.get(lp).and_then(|l| l.fin)?;
        let mut cur = entry;
        loop {
            if let Some(v) = self.fin_end_vertex(cur)
                && let Some(vp) = self.vertices.get(v).map(|x| x.point)
                && (vp - p).norm() <= tol
            {
                return Some(cur);
            }
            cur = self.fins.get(cur).map(|f| f.next)?;
            if cur == entry {
                return None;
            }
        }
    }

    /// The fin of a SPECIFIC loop `lk` ending at a vertex within `tol` of `p`.
    /// (Loop-scoped companion to `loop_fin_ending_at_point`, which only ever
    /// scans the face's FIRST/outer loop. Used by the seam-crosses-hole split to
    /// find which loop a chain endpoint lands on, outer OR inner.)
    pub(crate) fn loop_fin_on_loop_ending_at(
        &self,
        lk: crate::entity::LoopKey,
        p: keel_math::vec::Vec3,
        tol: f64,
    ) -> Option<FinKey> {
        let entry = self.loops.get(lk).and_then(|l| l.fin)?;
        let mut cur = entry;
        loop {
            if let Some(v) = self.fin_end_vertex(cur)
                && let Some(vp) = self.vertices.get(v).map(|x| x.point)
                && (vp - p).norm() <= tol
            {
                return Some(cur);
            }
            cur = self.fins.get(cur).map(|f| f.next)?;
            if cur == entry {
                return None;
            }
        }
    }

    /// A boundary edge OF `face` (any of its loops) whose 3D segment passes
    /// within `tol` of `p`, with `p` strictly interior to the segment. The
    /// face-scoped companion to `edge_containing_point`, used by the open-chain
    /// T-junction repair so a mid-edge seam endpoint splits only THIS face's
    /// boundary, never an unrelated coincident edge elsewhere in the body.
    pub(crate) fn edge_on_face_containing(
        &self,
        face: FaceKey,
        p: keel_math::vec::Vec3,
        tol: f64,
    ) -> Option<crate::entity::EdgeKey> {
        let loops = self.faces.get(face).map(|f| f.loops.clone())?;
        for lk in loops {
            let entry = self.loops.get(lk).and_then(|l| l.fin)?;
            let mut cur = entry;
            loop {
                if let Some(ek) = self.fins.get(cur).map(|f| f.edge) {
                    // Curve-aware (conic arc) first; chord test for straight edges.
                    if conic_arc_split_rel(self, ek, p, tol).is_some() {
                        return Some(ek);
                    }
                    if let Some(e) = self.edges.get(ek)
                        && let (Some(a), Some(b)) = (
                            self.vertices.get(e.bounds.0).map(|v| v.point),
                            self.vertices.get(e.bounds.1).map(|v| v.point),
                        )
                    {
                        let ab = b - a;
                        let len2 = ab.dot(ab);
                        if len2 > 0.0 {
                            let t = (p - a).dot(ab) / len2;
                            if t > 1e-7 && t < 1.0 - 1e-7 && ((a + ab * t) - p).norm() <= tol {
                                return Some(ek);
                            }
                        }
                    }
                }
                let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                    break;
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
        }
        None
    }

    /// The boundary edge of a SPECIFIC loop `lk` whose 3D segment passes within
    /// `tol` of `p` with `p` strictly interior to the segment. (Loop-scoped
    /// companion to `edge_on_face_containing`, used by the seam-crosses-hole
    /// split to split exactly the inner hole loop's edge at a crossing point.)
    pub(crate) fn loop_edge_containing(
        &self,
        lk: crate::entity::LoopKey,
        p: keel_math::vec::Vec3,
        tol: f64,
    ) -> Option<crate::entity::EdgeKey> {
        let entry = self.loops.get(lk).and_then(|l| l.fin)?;
        let mut cur = entry;
        loop {
            if let Some(ek) = self.fins.get(cur).map(|f| f.edge) {
                // Curve-aware (conic arc) first; chord test for straight edges.
                if conic_arc_split_rel(self, ek, p, tol).is_some() {
                    return Some(ek);
                }
                if let Some(e) = self.edges.get(ek)
                    && let (Some(a), Some(b)) = (
                        self.vertices.get(e.bounds.0).map(|v| v.point),
                        self.vertices.get(e.bounds.1).map(|v| v.point),
                    )
                {
                    let ab = b - a;
                    let len2 = ab.dot(ab);
                    if len2 > 0.0 {
                        let t = (p - a).dot(ab) / len2;
                        if t > 1e-7 && t < 1.0 - 1e-7 && ((a + ab * t) - p).norm() <= tol {
                            return Some(ek);
                        }
                    }
                }
            }
            cur = self.fins.get(cur).map(|f| f.next)?;
            if cur == entry {
                return None;
            }
        }
    }

    /// Reverse the fin-traversal direction of an INNER (hole) loop in place,
    /// flipping a co-wound hole to the conventional counter-wound winding so the
    /// arrangement's forward `mekr` bridge merges it into a SIMPLE loop
    /// (dossier 76 sec 4 option a). The operation is purely combinatorial:
    ///   - swap every loop fin's `next` <-> `prev` (reverse the cycle), and
    ///   - flip every loop fin's `forward` (so it traverses its edge the other
    ///     way; a fin's start/end vertices swap, keeping the now-reversed chain
    ///     end-to-start continuous).
    /// It touches ONLY this loop's own fins. Each shared hole edge keeps its
    /// exact radial set (the same fin keys), so `check_radial_cycles` is
    /// preserved; the adjacent wall faces' loops are untouched (a wall face
    /// reads its OWN fin's direction, never the hole loop's), so no manifold
    /// pairing or downstream orientation is disturbed. Returns `Some(())` on
    /// success, `None` if the loop is not a well-formed inner fin loop (decline).
    /// Re-validates in debug (`debug_validate`).
    fn reverse_inner_loop(&mut self, lp: crate::entity::LoopKey) -> Option<()> {
        use crate::entity::LoopKind;
        // Must be an inner fin loop.
        let entry = match self.loops.get(lp) {
            Some(l) if l.kind == LoopKind::Inner => l.fin?,
            _ => return None,
        };
        // Collect the loop's fins in traversal order (bounded by a cycle guard).
        let mut fins: Vec<crate::entity::FinKey> = Vec::new();
        let mut cur = entry;
        loop {
            fins.push(cur);
            let f = self.fins.get(cur)?;
            if f.owner != lp {
                return None;
            }
            cur = f.next;
            if cur == entry {
                break;
            }
            if fins.len() > self.fins.len() {
                return None; // malformed (non-terminating) ring
            }
        }
        // Reverse each fin: swap next/prev and flip forward. After this the ring
        // is traversed in the opposite sense; the radial sets are unchanged and
        // every fin remains live, so existing vertex `.fin` back-pointers stay
        // valid (they need only reference SOME incident fin, not one that starts
        // at the vertex -- `validate` and the loop walks do not require that).
        for &fk in &fins {
            if let Some(f) = self.fins.get_mut(fk) {
                std::mem::swap(&mut f.next, &mut f.prev);
                f.forward = !f.forward;
            }
        }
        self.debug_validate();
        Some(())
    }

    /// Unified per-face planar-overlay imprint (dossier 76). When an open seam
    /// chain on a PLANAR compound face (an outer loop plus zero-or-more inner
    /// HOLE loops) runs from the outer boundary to the outer boundary, possibly
    /// carrying MATERIAL interior vertices (a sibling-seam T-junction) AND
    /// dipping through one-or-more holes' void interiors, the per-chain
    /// `split_face` cannot resolve the stacked case: it routes each crossed hole
    /// WHOLESALE to one side and strands the hole-boundary fragments (the
    /// seed-715 `unmatched coedge` residual). This computes the planar
    /// arrangement of {outer loop, hole loops, the seam chain} and re-forms the
    /// correct sub-faces in ONE pass, built ENTIRELY from proven Euler operators
    /// (so validity and lineage hold by construction).
    ///
    /// The construction walks the chain once. Each chain vertex is classified
    /// material (outside every hole) or strictly inside exactly one hole; the
    /// two extreme endpoints must be material and on the outer loop. The walk
    /// emits, in chain order, a sequence of EVENTS:
    ///   - a MATERIAL interior vertex -> a `mev` spur on the current loop;
    ///   - a HOLE DIP (the chain enters hole H at P_enter, runs through its void,
    ///     and exits at P_exit) -> a `mekr` bridge from the current tip to
    ///     P_enter, MERGING hole H into the outer/working loop; the in-hole run
    ///     is discarded (void) and the tip resumes at P_exit, now on the merged
    ///     loop.
    /// A final `split_face` from the last tip to the chain's far endpoint divides
    /// the (now simply-connected w.r.t. every crossed hole) face into the two
    /// correct sub-faces, each carrying its portion of every merged hole
    /// boundary. Holes the chain does NOT cross stay inner loops and route
    /// wholesale to whichever sub-face contains them (which `mef` handles).
    ///
    /// This SUBSUMES dossier 73c (one hole, the dip on the first and last chain
    /// segment, no material interior vertex: the walk emits a single hole-dip
    /// event with no spurs, identical to the landed `mekr` + `split_face`) and
    /// dossier 73b's open-chain (no holes crossed: the walk is pure spurs + a
    /// final `split_face`, identical to `imprint_open_chain`'s plain path).
    ///
    /// Returns `Ok(None)` (fall through to the unchanged split / decline) for any
    /// structure it does not certify: a non-planar carrier; an extreme endpoint
    /// not on the outer loop; a chain vertex inside more than one hole at once; a
    /// hole entered more than once; an in-hole run whose midpoint is not actually
    /// inside the hole (a graze, not a traversal); a material run that cannot be
    /// realized vertex-to-vertex; or any missing topological precondition.
    /// DECLINE-never-WRONG: it only ever performs provably-valid Euler re-knits
    /// on a certified structure, and never moves geometry (crossing points lie on
    /// the existing hole edges within tol; spur/bridge/cut endpoints are exact
    /// chain points).
    fn try_imprint_chain_arrangement(
        &mut self,
        face: FaceKey,
        chain: &[keel_math::vec::Vec3],
        tol: f64,
    ) -> Result<Option<Vec<crate::entity::EdgeKey>>, TopoError> {
        use crate::euler::MevSite;
        use keel_math::vec::Vec3;
        let trace = std::env::var("KEEL_ARR_DEBUG").is_ok();
        let etol = tol.max(1e-7);
        let last = chain.len() - 1;
        if last < 1 {
            return Ok(None);
        }
        // PLANAR carrier only: the 2D crossing arithmetic and point-in-polygon
        // tests below assume a plane frame. A curved carrier with a hole is out
        // of this routine's tractable scope (falls through to decline).
        let Some(Surface3::Plane(pl)) = self.face_surface3(face) else {
            if trace {
                eprintln!("ARR: non-planar carrier -> None");
            }
            return Ok(None);
        };
        let o = pl.frame.origin;
        let (ex, ey) = (pl.frame.x, pl.frame.y);
        let to2 = |q: Vec3| -> (f64, f64) {
            let w = q - o;
            (w.dot(ex), w.dot(ey))
        };

        // Both extreme endpoints must be on the OUTER loop.
        let outer_lp = self
            .faces
            .get(face)
            .and_then(|f| f.loops.first().copied())
            .ok_or(TopoError::StaleKey)?;
        if self
            .loop_fin_on_loop_ending_at(outer_lp, chain[0], etol)
            .is_none()
            || self
                .loop_fin_on_loop_ending_at(outer_lp, chain[last], etol)
                .is_none()
        {
            if trace {
                eprintln!("ARR: endpoint not on outer loop -> None");
            }
            if std::env::var("KEEL_CHAIN_PROBE").is_ok() {
                let inner: Vec<crate::entity::LoopKey> = self
                    .faces
                    .get(face)
                    .map(|f| f.loops.iter().skip(1).copied().collect())
                    .unwrap_or_default();
                let where_of = |p: keel_math::vec::Vec3| -> String {
                    if self.loop_fin_on_loop_ending_at(outer_lp, p, etol).is_some() {
                        return "OUTER-vtx".into();
                    }
                    for (h, &lk) in inner.iter().enumerate() {
                        if self.loop_fin_on_loop_ending_at(lk, p, etol).is_some() {
                            return format!("HOLE{h}-vtx");
                        }
                        if self.loop_edge_containing(lk, p, etol).is_some() {
                            return format!("HOLE{h}-edge");
                        }
                    }
                    if self.loop_edge_containing(outer_lp, p, etol).is_some() {
                        return "OUTER-edge".into();
                    }
                    "INTERIOR".into()
                };
                eprintln!(
                    "CHAINPROBE: nloops={} chainlen={} end0={} endL={}",
                    inner.len() + 1,
                    chain.len(),
                    where_of(chain[0]),
                    where_of(chain[last])
                );
            }
            return Ok(None);
        }

        // Gather the inner hole loops and their 2D polygons.
        let inner_loops: Vec<crate::entity::LoopKey> = self
            .faces
            .get(face)
            .map(|f| f.loops.iter().skip(1).copied().collect())
            .unwrap_or_default();
        if inner_loops.is_empty() {
            // No holes: nothing for the arrangement to do beyond the plain
            // open-chain path. Fall through so the caller's spur + split runs.
            return Ok(None);
        }
        let hole_polys: Vec<Vec<(f64, f64)>> = inner_loops
            .iter()
            .map(|&lk| self.loop_polygon(lk).iter().map(|&q| to2(q)).collect())
            .collect();
        // Reject a degenerate hole polygon (a curved hole, or a collapsed loop).
        if hole_polys.iter().any(|p| p.len() < 3) {
            return Ok(None);
        }

        // Signed area of a 2D polygon (shoelace), used to compare each hole's
        // STORED fin-traversal winding to the outer loop's. `loop_polygon` walks
        // the fin ring in next-order, so this sign IS the direction `mekr` will
        // splice. A hole wound OPPOSITE the outer loop (the conventional B-rep
        // inner-ring winding) merges via the plain forward `mekr` into a simple
        // loop; a CO-WOUND hole (same sign as the outer loop, as a boolean-built
        // compound operand can store) cannot be merged by a local Euler re-knit
        // and DECLINES (the tractability boundary; see the decline below).
        let signed_area2 = |poly: &[(f64, f64)]| -> f64 {
            let n = poly.len();
            (0..n)
                .map(|k| {
                    let a = poly[k];
                    let b = poly[(k + 1) % n];
                    a.0 * b.1 - b.0 * a.1
                })
                .sum::<f64>()
                * 0.5
        };
        let outer_sign = signed_area2(
            &self
                .loop_polygon(outer_lp)
                .iter()
                .map(|&q| to2(q))
                .collect::<Vec<_>>(),
        )
        .signum();
        let hole_cowound: Vec<bool> = hole_polys
            .iter()
            .map(|p| signed_area2(p).signum() == outer_sign)
            .collect();

        // Classify each chain vertex: None = material (outside every hole), or
        // Some(h) = strictly inside hole h. A vertex inside MORE than one hole is
        // ambiguous (overlapping holes are not a planar arrangement we certify).
        let classify = |q: Vec3| -> Option<Option<usize>> {
            let q2 = to2(q);
            let mut hit: Option<usize> = None;
            for (h, poly) in hole_polys.iter().enumerate() {
                if winding_nonzero(poly, q2) {
                    if hit.is_some() {
                        return None; // inside two holes -> ambiguous
                    }
                    hit = Some(h);
                }
            }
            Some(hit)
        };
        let mut cls: Vec<Option<usize>> = Vec::with_capacity(chain.len());
        for &p in chain {
            match classify(p) {
                Some(c) => cls.push(c),
                None => {
                    if trace {
                        eprintln!("ARR: vertex inside >1 hole -> None");
                    }
                    return Ok(None);
                }
            }
        }
        if trace {
            eprintln!(
                "ARR: nholes={} chain.len={} cls={:?}",
                hole_polys.len(),
                chain.len(),
                cls
            );
        }
        // The endpoints must be material (they are on the outer loop, so this
        // should hold, but a tiny hole touching the boundary could fool the
        // winding test; certify explicitly).
        if cls[0].is_some() || cls[last].is_some() {
            if trace {
                eprintln!("ARR: endpoint inside a hole -> None");
            }
            return Ok(None);
        }

        // The transversal crossing point of chain segment a->b with hole `poly`.
        // Returns the segment-local t in [0,1] of the FIRST boundary hit going
        // from a (so an inside->outside / outside->inside transition takes the
        // correct crossing). Inclusive within tol so a crossing AT a hole VERTEX
        // is caught (the Phase-1 pre-split typically turns each tool corner that
        // lands on a hole into a hole vertex).
        let edge_hit =
            |a: (f64, f64), b: (f64, f64), poly: &[(f64, f64)], from_inside: bool| -> Option<f64> {
                let r = (b.0 - a.0, b.1 - a.1);
                let m = poly.len();
                let mut ts: Vec<f64> = Vec::new();
                for k in 0..m {
                    let c = poly[k];
                    let d = poly[(k + 1) % m];
                    let s = (d.0 - c.0, d.1 - c.1);
                    let rxs = r.0 * s.1 - r.1 * s.0;
                    if rxs.abs() < 1e-12 {
                        continue;
                    }
                    let qp = (c.0 - a.0, c.1 - a.1);
                    let t = (qp.0 * s.1 - qp.1 * s.0) / rxs;
                    let u = (qp.0 * r.1 - qp.1 * r.0) / rxs;
                    if (-1e-7..=1.0 + 1e-7).contains(&t) && (-1e-7..=1.0 + 1e-7).contains(&u) {
                        ts.push(t.clamp(0.0, 1.0));
                    }
                }
                ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                if from_inside {
                    ts.first().copied()
                } else {
                    ts.last().copied()
                }
            };

        // Walk the chain and build the event list. Events are kept in chain
        // order. A material run is a maximal sub-sequence of material vertices;
        // a hole dip is a maximal sub-sequence inside one hole, flanked by an
        // entry and an exit crossing.
        #[derive(Clone)]
        enum Event {
            // a material interior chain vertex -> a spur to this point
            Spur(Vec3),
            // a hole dip: bridge to P_enter, merge hole `h`, resume at P_exit
            Dip {
                h: usize,
                p_enter: Vec3,
                p_exit: Vec3,
            },
        }
        let mut events: Vec<Event> = Vec::new();
        let mut holes_dipped: Vec<usize> = Vec::new();
        let mut i = 0usize;
        while i < last {
            // Material segment chain[i] -> chain[i+1] (both material): the next
            // vertex is either an interior material spur or the final endpoint.
            if cls[i].is_none() && cls[i + 1].is_none() {
                if i + 1 < last {
                    events.push(Event::Spur(chain[i + 1]));
                }
                i += 1;
                continue;
            }
            // Transition material -> hole h on segment i (entry into a dip).
            if cls[i].is_none()
                && let Some(h) = cls[i + 1]
            {
                // The hole must not have been dipped already (single dip per
                // hole is the certified structure).
                if holes_dipped.contains(&h) {
                    if trace {
                        eprintln!("ARR: hole {h} dipped twice -> None");
                    }
                    return Ok(None);
                }
                let (a2, b2) = (to2(chain[i]), to2(chain[i + 1]));
                let Some(t_in) = edge_hit(a2, b2, &hole_polys[h], false) else {
                    if trace {
                        eprintln!("ARR: no entry crossing on seg {i} hole {h} -> None");
                    }
                    return Ok(None);
                };
                let p_enter = chain[i] + (chain[i + 1] - chain[i]) * t_in;
                // Advance through the in-hole run: every interior vertex must be
                // inside the SAME hole h, until the chain exits back to material.
                let mut j = i + 1;
                while j < last && cls[j] == Some(h) {
                    j += 1;
                }
                // chain[j] must be the first material vertex after the dip; the
                // exit crossing is on segment (j-1) -> j.
                if j > last || cls[j].is_some() {
                    if trace {
                        eprintln!(
                            "ARR: dip exit not material (j={j} cls={:?}) -> None",
                            cls.get(j)
                        );
                    }
                    return Ok(None);
                }
                let (c2, d2) = (to2(chain[j - 1]), to2(chain[j]));
                let Some(t_out) = edge_hit(c2, d2, &hole_polys[h], true) else {
                    if trace {
                        eprintln!("ARR: no exit crossing on seg {} hole {h} -> None", j - 1);
                    }
                    return Ok(None);
                };
                let p_exit = chain[j - 1] + (chain[j] - chain[j - 1]) * t_out;
                // The in-hole run midpoint must actually be inside the hole (a
                // genuine traversal, not a double graze of the boundary).
                let mid = (p_enter + p_exit) * 0.5;
                if !winding_nonzero(&hole_polys[h], to2(mid)) {
                    if trace {
                        eprintln!("ARR: dip midpoint not in hole {h} -> None");
                    }
                    return Ok(None);
                }
                events.push(Event::Dip { h, p_enter, p_exit });
                holes_dipped.push(h);
                i = j; // resume the material walk from the exit material vertex
                continue;
            }
            // Any other transition (a vertex inside a hole that is not flanked by
            // material as above, e.g. the chain STARTS inside a hole) is not a
            // certified structure.
            if trace {
                eprintln!("ARR: uncertified transition at i={i} -> None");
            }
            return Ok(None);
        }
        // Nothing to do if the chain crosses no hole: let the plain path handle
        // it (this routine only earns its keep on a hole-crossing chain).
        if holes_dipped.is_empty() {
            if trace {
                eprintln!("ARR: no hole dipped -> None (plain path)");
            }
            return Ok(None);
        }

        if trace {
            for &h in &holes_dipped {
                eprintln!(
                    "ARR: dipped hole {h} cowound={} (outer_sign={outer_sign})",
                    hole_cowound[h]
                );
            }
        }
        // Winding canonicalization (dossier 76 sec 4 option a, re-derived and
        // RESOLVED here, superseding the prior pre-mutation decline).
        //
        // `mekr` splices the merged outer ring to traverse the bridged hole
        // FORWARD (in fin-next order) from the bridge vertex. The merged loop is
        // a SIMPLE polygon (the hole's void left OUTSIDE the face material) only
        // when that forward traversal runs the hole CLOCKWISE relative to the
        // CCW outer loop -- i.e. the hole is stored COUNTER-wound, the
        // conventional B-rep inner-ring winding. A boolean-built operand can
        // store an inner loop CO-wound (same sense as the outer loop); the
        // forward mekr would then fold the void IN as material (a figure-8 whose
        // two sub-faces' areas sum to outer PLUS hole, measured on seed 715:
        // 713 + 169 ~= 882, the dominant residual of this class).
        //
        // The fix is to CANONICALIZE the co-wound hole loop to counter-wound
        // BEFORE the re-knit, by reversing that loop's fin cycle in place. The
        // prior dossier (sec 4) believed this was unsafe -- that flipping the
        // hole fins' `forward` would break the radial pairing with the adjacent
        // wall faces. The instrumented operand (KEEL_COWOUND_PROBE, seed 715)
        // refuted that: each shared hole edge carries exactly two fins (the
        // +x-face hole fin and the wall-face fin) and the kernel's radial
        // invariant (`check_radial_cycles`) only requires each fin to sit in
        // exactly ONE radial cycle -- it does NOT constrain the two fins'
        // relative `forward`. A wall face reads its OWN fin's direction for its
        // material side, never the hole loop's fin; reversing the hole loop
        // touches only the hole loop's own fins (their `forward`, `next`,
        // `prev`) and leaves every wall loop, and every edge's radial set,
        // byte-identical. So the reversal is a valid local operation that
        // preserves fin-ring continuity, boundary-chain continuity, and the
        // radial cycles; and after the subsequent mekr the hole ceases to be an
        // inner loop at all (it merges into the +x outer loop), so no co-wound
        // inner loop survives to mislead a downstream consumer.
        //
        // DECLINE-never-WRONG: the reversal is purely combinatorial (no geometry
        // moves); if it ever produced a malformed body the all-planar mass==mesh
        // gate and the shell-closure net would still reject it (the floor is
        // unchanged). `reverse_inner_loop` re-validates in debug and the result
        // is gated exactly as the counter-wound case. Seed 715's own +x-face hole
        // is co-wound: this is its rung, now CLOSED.
        for &h in &holes_dipped {
            if hole_cowound[h] {
                if trace {
                    eprintln!("ARR: co-wound dipped hole {h} -> reversing to counter-wound");
                }
                if self.reverse_inner_loop(inner_loops[h]).is_none() {
                    if trace {
                        eprintln!("ARR: hole {h} reversal failed -> None (decline)");
                    }
                    return Ok(None);
                }
            }
        }
        if trace {
            eprintln!(
                "ARR: FIRING events={} dipped={:?}",
                events.len(),
                holes_dipped
            );
        }

        // --- Topological re-knit (all provably-valid Euler ops) -------------
        // Pre-split the OUTER loop at the chain's two extreme endpoints if they
        // fall mid-edge (a fresh tool corner not yet a vertex). Provably on the
        // boundary within etol; never moves geometry.
        for &end in [chain[0], chain[last]].iter() {
            if self
                .loop_fin_on_loop_ending_at(outer_lp, end, etol)
                .is_none()
                && let Some(ek) = self.loop_edge_containing(outer_lp, end, etol)
            {
                let _ = self.split_edge_raw(ek, end);
            }
        }
        // Pre-split each dipped hole loop at its entry/exit crossings (usually a
        // no-op: the Phase-1 pre-split already made each crossing a hole vertex).
        for ev in &events {
            if let Event::Dip { h, p_enter, p_exit } = ev {
                let hlp = inner_loops[*h];
                for &p in [*p_enter, *p_exit].iter() {
                    if self.loop_fin_on_loop_ending_at(hlp, p, etol).is_none()
                        && let Some(ek) = self.loop_edge_containing(hlp, p, etol)
                    {
                        let _ = self.split_edge_raw(ek, p);
                    }
                }
            }
        }

        // Verify every endpoint we need now exists (a missing one means a
        // geometry/tolerance mismatch -> decline; only benign splits have run).
        if self
            .loop_fin_on_loop_ending_at(outer_lp, chain[0], etol)
            .is_none()
            || self
                .loop_fin_on_loop_ending_at(outer_lp, chain[last], etol)
                .is_none()
        {
            return Ok(None);
        }
        for ev in &events {
            if let Event::Dip { h, p_enter, p_exit } = ev {
                let hlp = inner_loops[*h];
                if self
                    .loop_fin_on_loop_ending_at(hlp, *p_enter, etol)
                    .is_none()
                    || self
                        .loop_fin_on_loop_ending_at(hlp, *p_exit, etol)
                        .is_none()
                {
                    return Ok(None);
                }
            }
        }

        let mut edges: Vec<crate::entity::EdgeKey> = Vec::new();
        let mut tip = chain[0];
        for ev in events.iter() {
            match ev {
                Event::Spur(v) => {
                    // A material interior vertex: spur from the current tip on
                    // the working (outer/merged) loop. The tip fin is the
                    // outer-loop fin ending at `tip` (a prior spur far-vertex or
                    // a merged-in hole vertex both live on the outer loop).
                    let fin = self
                        .loop_fin_on_loop_ending_at(outer_lp, tip, etol)
                        .ok_or(TopoError::Precondition("arrangement: spur tip lost"))?;
                    let m = self.mev(MevSite::AfterFin(fin), *v)?;
                    if let Some(curve) = seg_curve(tip, *v) {
                        self.attach_seam_geometry(m.edge, face, &curve, tol);
                    }
                    edges.push(m.edge);
                    tip = *v;
                }
                Event::Dip { h, p_enter, p_exit } => {
                    // Bridge the current tip to the hole entry P_enter via mekr;
                    // the hole merges into the outer/working loop. The discarded
                    // in-hole run carries no edge (it is void). Resume at the exit
                    // crossing P_exit, now a vertex on the merged loop. Only the
                    // counter-wound hole reaches here (the co-wound dip declined
                    // above), so the forward mekr yields a simple merged loop.
                    let hlp = inner_loops[*h];
                    let fin_outer = self
                        .loop_fin_on_loop_ending_at(outer_lp, tip, etol)
                        .ok_or(TopoError::Precondition("arrangement: bridge tip lost"))?;
                    let fin_ring = self
                        .loop_fin_on_loop_ending_at(hlp, *p_enter, etol)
                        .ok_or(TopoError::Precondition("arrangement: hole entry lost"))?;
                    let bridge = self.mekr(fin_outer, fin_ring)?;
                    if let Some(curve) = seg_curve(tip, *p_enter) {
                        self.attach_seam_geometry(bridge.edge, face, &curve, tol);
                    }
                    edges.push(bridge.edge);
                    tip = *p_exit;
                }
            }
        }
        // Final cut: from the last tip to the chain's far endpoint, now both on
        // the single merged loop, dividing the face into the two sub-faces.
        let fa = self
            .loop_fin_on_loop_ending_at(outer_lp, tip, etol)
            .ok_or(TopoError::Precondition("arrangement: final cut start lost"))?;
        let fb = self
            .loop_fin_on_loop_ending_at(outer_lp, chain[last], etol)
            .ok_or(TopoError::Precondition("arrangement: final cut end lost"))?;
        let out = self.split_face(fa, fb, None)?;
        if let Some(surf) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(nf) = self.faces.get_mut(out.face_new)
        {
            nf.surface = Some(surf);
        }
        if let Some(curve) = seg_curve(tip, chain[last]) {
            self.attach_seam_geometry(out.edge, face, &curve, tol);
        }
        edges.push(out.edge);
        if trace {
            eprintln!(
                "ARR: DONE counter-wound arrangement re-knit ({} edges)",
                edges.len()
            );
        }
        Ok(Some(edges))
    }

    /// Imprint an open chain of points onto `face`: `chain[0]` and
    /// `chain[last]` must already be boundary vertices; interior points
    /// are added as spurs (mev), then the face is split (split_face)
    /// from the last interior point to the boundary end. Returns the
    /// edges created along the chain.
    fn imprint_open_chain(
        &mut self,
        face: FaceKey,
        chain: &[keel_math::vec::Vec3],
        tol: f64,
    ) -> Result<Vec<crate::entity::EdgeKey>, TopoError> {
        use crate::euler::MevSite;
        let etol = tol.max(1e-7);
        if chain.len() < 2 {
            return Err(TopoError::Precondition("open chain too short"));
        }
        let last = chain.len() - 1;
        // T-junction repair (dossier 73b): an open seam can END in the
        // INTERIOR of a boundary edge that a SIBLING seam component imprinted
        // earlier in this same face's Phase-2 pass. (Concretely: a tool wall
        // first split by the body's prior cut, then crossed by the body's boss
        // top, lands its second seam's endpoint mid-edge on the first seam's
        // fresh edge.) Phase-1 pre-split only saw the ORIGINAL boundary, not
        // edges created during Phase 2, so that vertex is missing and the
        // boundary-vertex precondition below fails. Split the boundary edge at
        // any such mid-edge endpoint so the chain attaches vertex-to-vertex.
        // This adds a vertex at a point provably on the existing boundary
        // (edge_on_face_containing requires it within etol); it never moves
        // geometry and a point genuinely OFF the boundary still finds no edge
        // and declines below (DECLINE-never-WRONG preserved).
        for &end in [chain[0], chain[last]].iter() {
            if self.loop_fin_ending_at_point(face, end, etol).is_none()
                && let Some(ek) = self.edge_on_face_containing(face, end, etol)
            {
                let _ = self.split_edge_raw(ek, end);
            }
        }
        // Unified per-face planar-overlay (dossier 76): if this face carries an
        // inner hole loop and the chain crosses one-or-more holes (possibly while
        // also carrying MATERIAL interior vertices from a sibling-seam
        // T-junction), the plain boundary-to-boundary split below would route
        // each crossed hole WHOLESALE to one side and strand the hole-boundary
        // fragments (the seed-715 residual). Resolve it by computing the planar
        // arrangement of {outer loop, hole loops, chain} and re-forming the
        // sub-faces in one pass: spur the material interior vertices, mekr-bridge
        // each hole dip into the outer loop, then split_face. This SUBSUMES the
        // dossier-73c single-hole-dip routine. Returns Some(edges) only when the
        // certified structure is present and the re-knit succeeds; otherwise
        // falls through to the unchanged split (DECLINE-never-WRONG: a structure
        // it does not certify is left to decline at the existing checks).
        if self.faces.get(face).map(|f| f.loops.len()).unwrap_or(0) > 1
            && let Some(edges) = self.try_imprint_chain_arrangement(face, chain, tol)?
        {
            self.debug_validate();
            return Ok(edges);
        }
        // Endpoints must be on the boundary.
        if self
            .loop_fin_ending_at_point(face, chain[0], etol)
            .is_none()
            || self
                .loop_fin_ending_at_point(face, chain[last], etol)
                .is_none()
        {
            if std::env::var("KEEL_CHAIN_PROBE").is_ok() {
                let loops: Vec<crate::entity::LoopKey> = self
                    .faces
                    .get(face)
                    .map(|f| f.loops.clone())
                    .unwrap_or_default();
                let where_of = |p: keel_math::vec::Vec3| -> String {
                    for (li, &lk) in loops.iter().enumerate() {
                        if self.loop_fin_on_loop_ending_at(lk, p, etol).is_some() {
                            return format!("L{li}-vtx");
                        }
                        if self.loop_edge_containing(lk, p, etol).is_some() {
                            return format!("L{li}-edge");
                        }
                    }
                    "INTERIOR".into()
                };
                let planar = matches!(self.face_surface3(face), Some(Surface3::Plane(_)));
                eprintln!(
                    "OCPROBE: planar={planar} nloops={} chainlen={} end0={} endL={}",
                    loops.len(),
                    chain.len(),
                    where_of(chain[0]),
                    where_of(chain[last])
                );
            }
            return Err(TopoError::Precondition("open chain end not on boundary"));
        }
        let mut edges = Vec::new();
        let mut tip = chain[0];
        // Build spurs through the interior vertices.
        for &c in &chain[1..last] {
            let fin = self
                .loop_fin_ending_at_point(face, tip, etol)
                .ok_or(TopoError::Precondition("spur start vertex lost"))?;
            let m = {
                let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_MEV_NS);
                self.mev(MevSite::AfterFin(fin), c)?
            };
            if let Some(curve) = seg_curve(tip, c) {
                let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_SEAMGEO_NS);
                self.attach_seam_geometry(m.edge, face, &curve, tol);
            }
            edges.push(m.edge);
            tip = c;
        }
        // Close the cut from the last interior point to the boundary end.
        let fa = self
            .loop_fin_ending_at_point(face, tip, etol)
            .ok_or(TopoError::Precondition("split start vertex lost"))?;
        let fb = self
            .loop_fin_ending_at_point(face, chain[last], etol)
            .ok_or(TopoError::Precondition("split end vertex lost"))?;
        let out = {
            let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_SPLITF_NS);
            self.split_face(fa, fb, None)?
        };
        if let Some(surf) = self.faces.get(face).and_then(|f| f.surface)
            && let Some(nf) = self.faces.get_mut(out.face_new)
        {
            nf.surface = Some(surf);
        }
        if let Some(curve) = seg_curve(tip, chain[last]) {
            self.attach_seam_geometry(out.edge, face, &curve, tol);
        }
        self.debug_validate();
        edges.push(out.edge);
        Ok(edges)
    }

    /// Attach a seam edge's 3D curve and the pcurve (in the face's
    /// analytic surface) to both of its radial fins.
    pub(crate) fn attach_seam_geometry(
        &mut self,
        edge: crate::entity::EdgeKey,
        face: FaceKey,
        curve: &keel_geom::curve::Curve3,
        tol: f64,
    ) {
        let ckey = self.add_curve(curve.clone());
        if let Some(e) = self.edges.get_mut(edge) {
            e.curve = Some((ckey, true));
        }
        // EXACT pcurve fast path (OPT-M3): a straight degree-1 seam
        // segment on a PLANAR carrier projects to the straight UV
        // segment through its two projected endpoints, exactly. The
        // general 64-sample fit below was half the cost of every
        // box-boolean imprint for a result the projection gives in
        // closed form (and exactly, where the fit is approximate).
        if let Some(Surface3::Plane(pl)) = self.face_surface3(face)
            && let keel_geom::curve::Curve3::Nurbs(n) = curve
            && n.degree() == 1
        {
            let (p0, p1) = (curve.point(0.0), curve.point(1.0));
            let mid = curve.point(0.5);
            let chord = (p0 + p1) * 0.5;
            if (mid - chord).norm() <= 1e-12 * (1.0 + (p1 - p0).norm()) {
                let o = pl.frame.origin;
                let uv = |p: keel_math::vec::Vec3| {
                    let w = p - o;
                    keel_math::vec::Vec3::new(w.dot(pl.frame.x), w.dot(pl.frame.y), 0.0)
                };
                if let Ok(puv) = keel_geom::nurbs_curve::NurbsCurve::new(
                    1,
                    vec![0., 0., 1., 1.],
                    vec![uv(p0), uv(p1)],
                    None,
                ) {
                    let pkey = self.add_curve(keel_geom::curve::Curve3::Nurbs(puv));
                    let radial = self
                        .edges
                        .get(edge)
                        .map(|e| e.radial.clone())
                        .unwrap_or_default();
                    for fk in radial {
                        if let Some(f) = self.fins.get_mut(fk) {
                            f.pcurve = Some((pkey, true));
                        }
                    }
                    return;
                }
            }
        }
        if let Some(surf) = self.face_surface3(face)
            && let Ok(fit) = keel_geom::fit::pcurve_on_analytic(curve, &surf, 64, tol.max(1e-7))
        {
            let pkey = self.add_curve(keel_geom::curve::Curve3::Nurbs(fit.curve));
            let radial = self
                .edges
                .get(edge)
                .map(|e| e.radial.clone())
                .unwrap_or_default();
            for fk in radial {
                if let Some(f) = self.fins.get_mut(fk) {
                    f.pcurve = Some((pkey, true));
                }
            }
        }
    }
}

/// Imprint the given seam curves onto a clone of `body`, picking each
/// seam's face via `pick` (face_a for operand A, face_b for operand B).
///
/// Two phases. Phase 1: split every operand boundary edge crossed by a
/// seam endpoint at that point (so a seam loop that wraps across faces
/// gets shared corner vertices). Phase 2 per face: if its seam segments
/// run boundary-vertex to boundary-vertex, split the face along each;
/// if they form a closed loop interior to the face, imprint one ring.
/// Configurations needing in-face corner vertices (multi-segment open
/// chains) are tagged and deferred to the next M6a increment.
/// Attach a straight `Line3` to `ek` from its current endpoint vertices.
fn set_edge_line(body: &mut Body, ek: crate::entity::EdgeKey) {
    use keel_geom::curve::{Curve3, Line3};
    let pts = body
        .edges
        .get(ek)
        .map(|e| (e.bounds.0, e.bounds.1))
        .and_then(|(a, b)| Some((body.vertices.get(a)?.point, body.vertices.get(b)?.point)));
    if let Some((pa, pb)) = pts
        && let Ok(line) = Line3::new(pa, pb - pa)
    {
        body.attach_edge_curve(ek, Curve3::Line(line), true);
    }
}

/// Subdivide a CLOSED seam ring edge (one `imprint_closed_curve` edge built
/// from several straight seam segments) at its corner `nodes`, so THIS
/// operand's seam matches the OTHER operand's per-face open-edge
/// subdivision (research file 47: a shared seam can only stitch when both
/// operands subdivide it the same way -- 1 closed edge can never pair with
/// N open edges). Splits in loop order; each resulting side gets a straight
/// `Line3`. Falls back to the whole ring when it has no corner structure
/// (a smooth sphere/planar SSI circle) or cannot be split.
fn subdivide_seam_ring(
    body: &mut Body,
    edge: crate::entity::EdgeKey,
    nodes: &[keel_math::vec::Vec3],
    tol: f64,
) -> Vec<crate::entity::EdgeKey> {
    let n = nodes.len();
    if n < 3 {
        return vec![edge];
    }
    let etol = tol.max(1e-7);
    let start = match body
        .edges
        .get(edge)
        .map(|e| e.bounds.0)
        .and_then(|v| body.vertices.get(v).map(|x| x.point))
    {
        Some(p) => p,
        None => return vec![edge],
    };
    let Some(si) = nodes.iter().position(|p| (*p - start).norm() <= etol) else {
        return vec![edge];
    };
    let ordered: Vec<keel_math::vec::Vec3> = (0..n).map(|i| nodes[(si + i) % n]).collect();
    let mut out = Vec::new();
    let mut rest = edge;
    for &node in ordered.iter().skip(1) {
        match body.split_edge(rest, node) {
            Ok(sp) => {
                set_edge_line(body, sp.edge_a);
                out.push(sp.edge_a);
                rest = sp.edge_b;
            }
            Err(_) => {
                out.push(rest);
                return out;
            }
        }
    }
    set_edge_line(body, rest);
    out.push(rest);
    out
}

fn imprint_operand(
    body: &Body,
    seams: &[SeamCurve],
    pick: impl Fn(&SeamCurve) -> (FaceKey, bool),
    tol: f64,
    faults: &mut Vec<BoolFault>,
) -> ImprintedOperand {
    use keel_math::vec::Vec3;
    use std::collections::BTreeMap;
    let mut working = body.clone();
    let mut seam_edges = Vec::new();
    let etol = tol.max(1e-7);

    // Group seam indices by their face on this operand. A seam already
    // lying on this side's boundary topology imprints nothing here.
    let mut groups: BTreeMap<u64, (FaceKey, Vec<usize>)> = BTreeMap::new();
    for (i, s) in seams.iter().enumerate() {
        let (face, on_boundary) = pick(s);
        if on_boundary {
            continue;
        }
        let id = working.faces.get(face).map(|f| f.id.0).unwrap_or(u64::MAX);
        groups.entry(id).or_insert((face, Vec::new())).1.push(i);
    }

    // Drop seam segments that lie entirely on the operand face's OWN
    // boundary (research file 39 §3, touching contact). When a tool face
    // meets this face along one of its existing edges, the SSI emits a
    // seam coincident with the boundary. Assembling those boundary-
    // coincident segments together with the genuine INTERIOR cut turns an
    // open splitting chain into a spurious closed inner RING that punches
    // a hole in the face instead of dividing it -- the partial-overlap
    // L-union bug: B's footprint on A's top has three of its four sides on
    // A-top's boundary, so the four seams assembled into a ring and the
    // face kept a phantom inner loop. A seam whose every sample lies on the
    // face boundary within tol is already an edge of this face; the other
    // operand owns the contact, so this face must not re-imprint it. Only
    // the interior portion (the real cut) survives to split the face.
    let _prof_filter = crate::profile::Scope::new(&crate::profile::IMPRINT_FILTER_NS);
    for (face, members) in groups.values_mut() {
        // EVERY loop's polygon counts as boundary, rings included: a
        // pre-imprinted pocket rim (the enclosed-overlap coincidence)
        // is an INNER ring, and seams lying on it are already edges of
        // this face exactly like outer-boundary contact.
        let mut loops_bnd: Vec<Vec<Vec3>> = Vec::new();
        for lk in working
            .faces
            .get(*face)
            .map(|f| f.loops.clone())
            .unwrap_or_default()
        {
            let Some(entry) = working.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut poly = Vec::new();
            let mut cur = entry;
            loop {
                if let Some(v) = working.fin_start_vertex(cur)
                    && let Some(x) = working.vertices.get(v)
                {
                    poly.push(x.point);
                }
                let Some(next) = working.fins.get(cur).map(|f| f.next) else {
                    break;
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
            if poly.len() >= 2 {
                loops_bnd.push(poly);
            }
        }
        let bnd = loops_bnd.first().cloned().unwrap_or_default();
        if bnd.len() < 3 {
            continue;
        }
        let on_boundary = |p: Vec3| -> bool {
            loops_bnd
                .iter()
                .flat_map(|poly| {
                    let m = poly.len();
                    (0..m).map(move |i| seg_dist3(p, poly[i], poly[(i + 1) % m]))
                })
                .fold(f64::INFINITY, f64::min)
                <= etol
        };
        let is_on_boundary_seg = |i: usize| {
            [0.0, 0.25, 0.5, 0.75, 1.0]
                .iter()
                .all(|&t| on_boundary(curve_point(&seams[i].curve, t)))
        };
        // The UNIVERSAL sec 3.2 drop (research file 39): a segment whose
        // EVERY sample lies on this face's existing boundary is already
        // an edge of this face; it separates nothing, and re-imprinting
        // it double-splits the boundary (a duplicate ring edge, Euler
        // off by one: the snapped-contact tail, where a pre-imprinted
        // pocket rim met its own SSI seams as a non-closed group that
        // the old ring-scoped filter let through). A genuine cut always
        // carries interior samples and survives; a partially-touching
        // segment (endpoint on the boundary, body interior) survives
        // because the test requires ALL five samples on the boundary.
        let _ = face;
        members.retain(|&i| !is_on_boundary_seg(i));
    }

    drop(_prof_filter);
    // Phase 1: pre-split boundary edges at unique OPEN-seam endpoints
    // (corners of open chains). Closed curves are skipped: their two
    // "endpoints" are the same degenerate point, which for a cylinder
    // SSI circle is exactly the seam crossing -- pre-splitting there
    // would defeat the crossing imprint.
    let _prof_presplit = crate::profile::Scope::new(&crate::profile::IMPRINT_PRESPLIT_NS);
    let mut corners: Vec<Vec3> = Vec::new();
    for s in seams {
        if s.closed {
            continue;
        }
        let (p0, p1) = curve_endpoints(&s.curve);
        for p in [p0, p1] {
            if !corners.iter().any(|q| (*q - p).norm() <= etol) {
                corners.push(p);
            }
        }
    }
    for &p in &corners {
        if let Some(edge) = working.edge_containing_point(p, etol) {
            // A CONIC rim split must carry the children's arc_sweep, else a
            // SUBSEQUENT corner on the same rim (now two arcs) reads the wrong
            // sweep in conic_arc_split_rel and is not recognized as on-boundary
            // -> the open-chain imprint then fails "end not on boundary". Mirror
            // merge_and_glue_imported's arc_sweep propagation. A straight edge
            // returns None here and splits with no sweep (correct for a line).
            let conic = conic_arc_split_rel(&working, edge, p, etol);
            if let Ok(out) = working.split_edge(edge, p) {
                if let Some((rel, s)) = conic {
                    working.set_edge_arc_sweep(out.edge_a, rel);
                    working.set_edge_arc_sweep(out.edge_b, s - rel);
                }
            }
        }
    }

    // Phase 2: imprint per face.
    drop(_prof_presplit);
    let _prof_dispatch = crate::profile::Scope::new(&crate::profile::IMPRINT_DISPATCH_NS);
    for (_, (face, mut members)) in groups {
        // Dedupe geometrically-coincident imprint curves on THIS face, keeping
        // one canonical representative (dossier 47 Q5/centerpiece: "coincident
        // /duplicate facets are resolved by keeping one canonical representative"
        // and the imprint must "create the shared edge once"). Two operand-B
        // faces that share an edge lying ON this operand-A face each emit a seam
        // here -- e.g. the asymmetric-chamfer cutter's oblique cut face and its
        // apex face share the prism edge that lands on the box side face, so both
        // intersect that box face along the SAME line. Imprinting both
        // double-splits the face and orphans a coedge (the shell-closure
        // violation #21 catches). This is an EXACT shared cutter edge, not a
        // near-tangency, so it is a canonical-dedup, NOT a dossier-39 sec 3.2
        // tangency suppression. The OTHER operand groups by ITS own face, so each
        // B-face keeps its own imprint; only this A-face's duplicate drops.
        if members.len() > 1 {
            let mut keep: Vec<usize> = Vec::with_capacity(members.len());
            for &i in &members {
                let (ai, bi) = curve_endpoints(&seams[i].curve);
                let mi = curve_point(&seams[i].curve, 0.5);
                let dup = keep.iter().any(|&j| {
                    let (aj, bj) = curve_endpoints(&seams[j].curve);
                    let mj = curve_point(&seams[j].curve, 0.5);
                    (mi - mj).norm() <= etol
                        && (((ai - aj).norm() <= etol && (bi - bj).norm() <= etol)
                            || ((ai - bj).norm() <= etol && (bi - aj).norm() <= etol))
                });
                if !dup {
                    keep.push(i);
                }
            }
            members = keep;
        }
        // MILLING SLOT (KEEL_MILL_FLOW): two open STRAIGHT rulings on a CYLINDER
        // lateral = a plane-cut flat/slot. A blind per-ruling split_face doubles
        // the seamed lateral (the seam is a re-used left/right bridge edge), so
        // route the PAIR to the open-ruling band split.
        if members.len() == 2
            && matches!(working.face_surface3(face), Some(Surface3::Cylinder(_)))
            && members.iter().all(|&i| {
                !seams[i].closed && {
                    let (a, b) = curve_endpoints(&seams[i].curve);
                    (curve_point(&seams[i].curve, 0.5) - (a + b) * 0.5).norm() <= etol.max(1e-6)
                }
            })
        {
            match working.imprint_cylinder_lateral_slot(
                face,
                &seams[members[0]].curve,
                &seams[members[1]].curve,
                tol,
            ) {
                Ok(mut es) => seam_edges.append(&mut es),
                Err(e) => faults.push(BoolFault::Topo(e)),
            }
            continue;
        }
        // CROSSING-PAIR arrangement (task 29, the equal-radii slice):
        // exactly two closed seams on one cylinder lateral that
        // mutually cross reduce sequentially to proven primitives (the
        // first as the standard wrap, the second as two open arcs
        // between the EXACT crossing vertices: the seam planes' line
        // meets the cylinder in a closed-form quadratic).
        if members.len() == 2
            && members.iter().all(|&i| seams[i].closed)
            && let Some(Surface3::Cylinder(cyl)) = working.face_surface3(face)
        {
            let xs = crate::imprint::planar_curve_crossings(
                &seams[members[0]].curve,
                &seams[members[1]].curve,
                &cyl,
            );
            if xs.len() == 2 {
                let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_OPS_NS);
                crate::profile::count(&crate::profile::IMPRINT_OPS_CALLS);
                match working.imprint_crossing_pair(
                    face,
                    &seams[members[0]].curve,
                    &seams[members[1]].curve,
                    &xs,
                    tol,
                ) {
                    Ok(mut es) => seam_edges.append(&mut es),
                    Err(e) => faults.push(BoolFault::Topo(e)),
                }
                continue;
            }
        }
        // Partition the face's deduped seams into connected components
        // (open-endpoint adjacency within etol; closed seams stand
        // alone). A face cut by two parallel tool planes carries two
        // DISJOINT open chains -- e.g. each wall of a block pushed
        // through a slab (the interior through-notch) is cut by the
        // slab's top AND bottom: neither one closed loop nor one open
        // chain, so the old single-assembly dispatch faulted and the
        // wall never split. Components imprint sequentially; an earlier
        // split may strand a later component on a descendant piece of
        // `face`, so each later component is relocated onto the planar
        // face that now contains it.
        let comps = seam_components(&members, seams, etol);
        let multi = comps.len() > 1;
        for comp in comps {
            let target = if multi {
                let probe = curve_point(&seams[comp[0]].curve, 0.5);
                let tgt = planar_face_containing(&working, probe, etol)
                    .or_else(|| curved_face_containing(&working, probe, etol))
                    .or_else(|| sphere_face_containing_3d(&working, probe, etol));
                match tgt {
                    Some(fk) => fk,
                    None => {
                        faults.push(BoolFault::AssemblyFailed(
                            "unlocated seam component (non-planar multi-cut face)",
                        ));
                        continue;
                    }
                }
            } else {
                face
            };
            let eps: Vec<(Vec3, Vec3)> = comp
                .iter()
                .map(|&i| curve_endpoints(&seams[i].curve))
                .collect();
            // A single, already-closed curve. A sphere/planar SSI circle is
            // interior to its face (ring imprint); a cylinder SSI circle
            // wraps the lateral face and crosses its seam line (crossing
            // imprint).
            if comp.len() == 1 && seams[comp[0]].closed {
                let curve = &seams[comp[0]].curve;
                // A circle WRAPPING a seamless tube lateral (the drill
                // primitive: two closed rims, no seam line) first gets
                // its seam SYNTHESIZED (mekr between the rim seam
                // vertices): the crossing imprint then applies. The
                // interior-ring imprint is topologically wrong for a
                // non-contractible wrap.
                let wraps = match working.face_surface3(target) {
                    Some(Surface3::Cylinder(c)) => {
                        let coaxial_planar = matches!(
                            closed_curve_center_axis(curve),
                            Some((centre, ax))
                                if ax.cross(c.frame.z).norm() < 1e-6
                                    && (centre - c.frame.origin).cross(c.frame.z).norm() < 1e-6
                        );
                        // A coaxial PLANAR circle, OR a NON-planar NURBS loop that
                        // still ENCIRCLES the axis (the off-axis cyl/sphere cut):
                        // both are non-contractible -> the band split, never an
                        // interior-ring hole. The encircling arm is DORMANT
                        // enabling machinery (gated to the cyl/sphere-wrap dev
                        // flag): it correctly band-splits the cylinder lateral
                        // (mesh then matches), but the cyl/sphere WRAP still
                        // declines on the sphere NURBS-cut mass/classify (KL5,
                        // dossier #60), so it stays off by default = pass-neutral.
                        coaxial_planar
                            || curve_encircles_axis(curve, c.frame.origin, c.frame.z)
                    }
                    // A coaxial circle wraps a cone lateral the same way
                    // (the countersink rim on the frustum face). The off-axis
                    // cone/sphere ENCIRCLING wrap is NOT routed here: the band
                    // split assembles but the cone NURBS-seam mass + tessellation
                    // are not yet correct (probe_conesphwrap: open_ratio up to
                    // 0.93, mass != mesh != truth), so it stays declined (the cone
                    // analogue of the cyl/sphere NURBS-band machinery is a
                    // follow-up). Decline-safe via the mass==mesh + tight oracle.
                    Some(Surface3::Cone(c)) => matches!(
                        closed_curve_center_axis(curve),
                        Some((centre, ax))
                            if ax.cross(c.frame.z).norm() < 1e-6
                                && (centre - c.frame.origin).cross(c.frame.z).norm() < 1e-6
                    ),
                    _ => false,
                };
                // A NON-planar encircling NURBS seam (the cyl/cyl quartic wrap)
                // uses the dossier-64 periodic-domain band split (close each
                // band through the seam-slit sub-edge), not the antipode spur
                // that collapses it. Routed before the synthesize/spur path.
                let wrap_nurbs = wraps && matches!(curve, keel_geom::curve::Curve3::Nurbs(_));
                if wraps
                    && !wrap_nurbs
                    && !working.closed_curve_crosses_boundary(target, curve, tol)
                    && let Err(e) = working.synthesize_lateral_seam(target)
                {
                    faults.push(BoolFault::Topo(e));
                    continue;
                }
                let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_OPS_NS);
                crate::profile::count(&crate::profile::IMPRINT_OPS_CALLS);
                let res = if wrap_nurbs {
                    working.imprint_cylinder_wrap_bands(target, curve, tol)
                } else if working.closed_curve_crosses_boundary(target, curve, tol) {
                    working.imprint_closed_curve_crossing(target, curve, tol)
                } else {
                    working.imprint_closed_curve(target, curve, tol)
                };
                match res {
                    Ok(rep) => seam_edges.push(rep.edge),
                    Err(e) => faults.push(BoolFault::Topo(e)),
                }
                continue;
            }
            // Segments forming one closed loop interior to the face: ring.
            if let Some(nodes) = assemble_closed_loop(&eps, tol)
                && let Some(ring) = closed_polyline_nurbs(&nodes)
            {
                let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_OPS_NS);
                crate::profile::count(&crate::profile::IMPRINT_OPS_CALLS);
                let res = {
                    let _p2 = crate::profile::Scope::new(&crate::profile::CLOSED_IMPRINT_NS);
                    working.imprint_closed_curve(
                        target,
                        &keel_geom::curve::Curve3::Nurbs(ring),
                        tol,
                    )
                };
                match res {
                    Ok(rep) => {
                        // Match the OTHER operand's per-face open-edge seam
                        // subdivision (file 47): split this closed ring at its
                        // corners so the seam coedges can pair at stitch time.
                        let _p3 = crate::profile::Scope::new(&crate::profile::RING_SUBDIV_NS);
                        let subdiv = subdivide_seam_ring(&mut working, rep.edge, &nodes, tol);
                        seam_edges.extend(subdiv);
                    }
                    Err(e) => faults.push(BoolFault::Topo(e)),
                }
                continue;
            }
            // An open chain (single segment, or a corner-overlap L): split
            // the face boundary-to-boundary through any interior corners.
            if let Some(chain) = assemble_open_chain(&eps, etol) {
                let _prof = crate::profile::Scope::new(&crate::profile::IMPRINT_OPS_NS);
                crate::profile::count(&crate::profile::IMPRINT_OPS_CALLS);
                match working.imprint_open_chain(target, &chain, tol) {
                    Ok(es) => seam_edges.extend(es),
                    Err(e) => faults.push(BoolFault::Topo(e)),
                }
                continue;
            }
            faults.push(BoolFault::AssemblyFailed("unassembled face seams"));
        }
    }
    // Tolerant-edge contract (M7b): the imprinted seam edges carry the
    // SSI curve's certified error bound, propagated to their vertices.
    // The combinatorial topology stays exact; the GEOMETRY carries its
    // bound -- "exact topology decisions with tolerant geometry".
    let max_seam_tol = seams.iter().map(|s| s.tol).fold(0.0f64, f64::max);
    if max_seam_tol > 0.0 {
        for &edge in &seam_edges {
            working.set_seam_edge_tolerance(edge, max_seam_tol);
        }
    }
    ImprintedOperand {
        body: working,
        seam_edges,
    }
}

impl Body {
    /// Imprint one body onto another (item 74): split this body's faces
    /// along its intersection seams with `tool`, returning the imprinted
    /// copy. Topology gains the seam edges; the GEOMETRY is untouched
    /// (volume identical, mass == mesh preserved). The body-level form
    /// of the imprint-only boolean option (item 32), riding the same
    /// seam_curves + imprint_operand pipeline. Errs on any imprint fault
    /// (coincident/tangent contacts decline as in booleans; a fault is
    /// never silently dropped).
    pub fn imprint_body(&self, tool: &Body, tol: f64) -> Result<Body, BoolFault> {
        let (seams, mut faults) = seam_curves(self, tool, tol);
        let ia = imprint_operand(
            self,
            &seams,
            |s| (s.face_a, s.on_boundary_a),
            tol,
            &mut faults,
        );
        if let Some(f) = faults.into_iter().next() {
            return Err(f);
        }
        if ia.body.validate().is_err() {
            return Err(BoolFault::AssemblyFailed("imprinted body invalid"));
        }
        Ok(ia.body)
    }
}

/// Two-body imprint (M3 pipeline steps 1-3, per-operand form): localize
/// and intersect, then imprint the seams onto independent clones of
/// each operand. Each returned body is itself a valid solid (the imprint
/// only splits faces along on-surface curves); they are glued into one
/// result at stitch time, after selection discards the unwanted sides.
pub fn imprint_pair(
    a: &Body,
    b: &Body,
    tol: f64,
) -> (ImprintedOperand, ImprintedOperand, Vec<BoolFault>) {
    let (seams, mut faults) = seam_curves(a, b, tol);
    let ia = imprint_operand(a, &seams, |s| (s.face_a, s.on_boundary_a), tol, &mut faults);
    let ib = imprint_operand(b, &seams, |s| (s.face_b, s.on_boundary_b), tol, &mut faults);
    (ia, ib, faults)
}

/// Localize + intersect: every face of `a` against every face of `b`,
/// collecting transversal SSI curves. Coincident/tangent/failed pairs
/// become faults. (All-pairs for M6a's small analytic bodies; AABB/BVH
/// localization is a perf concern deferred until a fuzz/scale need.)
/// Canonical-recovery pre-pass (M8): for each NURBS face, recognize a
/// hidden analytic surface and certify it within `tol`. The recovered
/// analytic is used only for SSI DISPATCH, so a NURBS-sphere x
/// NURBS-sphere intersection routes to the exact tier-1 sphere-sphere
/// path (an exact circle) instead of grinding the tier-3 spline solver.
/// The face itself stays NURBS for imprint/tessellation (the imprint of
/// an exact circle onto a NURBS face is the proven analytic-x-NURBS
/// path). This is the affordability gradient: exact where a quadric is
/// hiding, certified-tolerant only for genuine free-form.
fn recovered_analytics(body: &Body, tol: f64) -> Vec<(FaceKey, Surface3)> {
    let mut out = Vec::new();
    for f in body.face_keys() {
        if let Some(SurfaceGeom::Nurbs(n)) = body.face_surface_geom(f)
            && let Some(rec) = keel_geom::recover::recover_surface(&n, tol)
        {
            out.push((f, rec.surface));
        }
    }
    out
}

/// Compute the surface-surface intersection ([`SeamCurve`]s) between the
/// faces of `a` and `b`, without imprinting or assembling anything.
///
/// This is the read-only first stage of a boolean: it returns the seam
/// curves the operation would imprint, plus a [`BoolFault`] for every
/// face pair whose intersection could not be computed (coincident,
/// tangent, or unsupported). Useful for previewing where two bodies
/// meet. `tol` is the linear modeling tolerance.
pub fn seam_curves(a: &Body, b: &Body, tol: f64) -> (Vec<SeamCurve>, Vec<BoolFault>) {
    let mut seams = Vec::new();
    let mut faults = Vec::new();
    let rec_a = recovered_analytics(a, tol);
    let rec_b = recovered_analytics(b, tol);
    for fa in a.face_keys() {
        let Some(ga) = a.face_surface_geom(fa) else {
            continue;
        };
        let ref_a = match rec_a.iter().find(|(k, _)| *k == fa) {
            Some((_, s)) => SurfaceRef::Analytic(s),
            None => match &ga {
                SurfaceGeom::Analytic(s) => SurfaceRef::Analytic(s),
                SurfaceGeom::Nurbs(n) => SurfaceRef::Nurbs(n),
            },
        };
        let id_a = a.faces.get(fa).map(|f| f.id.0).unwrap_or(0);
        for fb in b.face_keys() {
            let Some(gb) = b.face_surface_geom(fb) else {
                continue;
            };
            let ref_b = match rec_b.iter().find(|(k, _)| *k == fb) {
                Some((_, s)) => SurfaceRef::Analytic(s),
                None => match &gb {
                    SurfaceGeom::Analytic(s) => SurfaceRef::Analytic(s),
                    SurfaceGeom::Nurbs(n) => SurfaceRef::Nurbs(n),
                },
            };
            let id_b = b.faces.get(fb).map(|f| f.id.0).unwrap_or(0);
            // Cylinder-cylinder crossing pairs: the exact two-closed-
            // conic form assembles through imprint_crossing_pair
            // (task 29; the Steinmetz oracle pins 16/3). Inexact or
            // non-conic SSI residue on such pairs keeps declining
            // (DECLINE-never-WRONG; the old seamless path once read
            // 12.5 against the exact 16/3).
            let both_cyl = matches!(ref_a, SurfaceRef::Analytic(Surface3::Cylinder(_)))
                && matches!(ref_b, SurfaceRef::Analytic(Surface3::Cylinder(_)));
            // Non-coaxial cylinder/sphere: the lateral seam is a quartic the
            // SSI now resolves analytically (cyl_quadratic_branch_field) and
            // returns Empty when the sphere clears the lateral, but the
            // sphere-side imprint/classify/mass do not yet assemble that
            // family, and an off-axis sphere meeting only a cap exposes
            // downstream assembly gaps (a grazing cyl - sph difference read
            // ~1.7% over the cylinder's own volume). Decline the whole
            // non-coaxial cyl/sphere class -- EXACTLY the prior blanket-Err
            // behavior, same IntersectionFailed fault -- until that
            // downstream lands; the SSI gateway is exercised by its own unit
            // tests (ssi.rs). Coaxial cyl/sphere (exact circles) still
            // assembles. Matches the SSI's own coaxiality threshold (> tol).
            let cyl_sphere_wrap = match (&ref_a, &ref_b) {
                (
                    SurfaceRef::Analytic(Surface3::Cylinder(c)),
                    SurfaceRef::Analytic(Surface3::Sphere(s)),
                )
                | (
                    SurfaceRef::Analytic(Surface3::Sphere(s)),
                    SurfaceRef::Analytic(Surface3::Cylinder(c)),
                ) => {
                    let w = s.frame.origin - c.frame.origin;
                    let delta = (w - c.frame.z * w.dot(c.frame.z)).norm();
                    // WRAP (sphere swallows the cross-section, R >= delta+r_cyl
                    // -> two encircling loops -> a sphere band) still declines;
                    // the WINDOW (single grazing loop) flows to the imprint. The
                    // watertightness + op-bound gate backstops a malformed
                    // result; the canonical-seam work makes the good ones PASS.
                    delta > tol && s.radius + tol >= delta + c.radius
                }
                _ => false,
            };
            // WRAP un-gated (was KEEL_WRAP_FLOW): the sphere band assembles via the
            // dossier-64 periodic split on the cylinder lateral + the sphere
            // NURBS-cut faces. The TIGHT quadric_sphere_op_volume oracle (plus the
            // watertight + mass==mesh gate) backstops a malformed result -> it
            // DECLINES (WRONG-safe by construction): a correct wrap passes, a
            // mis-classified one fails the independent volume truth and declines.
            let _ = cyl_sphere_wrap;
            // COINCIDENT cylinders (coaxial, equal radius: the mated
            // pin-in-hole laterals, dossier 39 sec 5) are the curved
            // on-on class, not a crossing: no seam, an informational
            // Coincident note, and the carrier-based classification
            // resolves the pair (full mutual coverage needs no
            // pre-imprint).
            if let (
                SurfaceRef::Analytic(Surface3::Cylinder(ca)),
                SurfaceRef::Analytic(Surface3::Cylinder(cb)),
            ) = (&ref_a, &ref_b)
                && ca.frame.z.cross(cb.frame.z).norm() < 1e-9
                && (ca.frame.origin - cb.frame.origin).cross(ca.frame.z).norm() < 1e-9
                && (ca.radius - cb.radius).abs() < tol.max(1e-9)
            {
                faults.push(BoolFault::Coincident(id_a, id_b));
                continue;
            }
            // COINCIDENT cones (the mated countersink plug): coaxial,
            // equal taper in the shared axis sense, equal radius at a
            // common plane. Same on-on contract as the cylinder pair.
            if let (
                SurfaceRef::Analytic(Surface3::Cone(ca)),
                SurfaceRef::Analytic(Surface3::Cone(cb)),
            ) = (&ref_a, &ref_b)
                && ca.frame.z.cross(cb.frame.z).norm() < 1e-9
                && (ca.frame.origin - cb.frame.origin).cross(ca.frame.z).norm() < 1e-9
                && {
                    let sense = ca.frame.z.dot(cb.frame.z).signum();
                    let d = (cb.frame.origin - ca.frame.origin).dot(ca.frame.z);
                    (sense * cb.half_angle.tan() - ca.half_angle.tan()).abs() < 1e-9
                        && (cb.radius - d * ca.half_angle.tan() - ca.radius).abs() < tol.max(1e-9)
                }
            {
                faults.push(BoolFault::Coincident(id_a, id_b));
                continue;
            }
            // PLANE/CONE shared setup (tasks 38, 47): the analytic section
            // rung works on the INFINITE cone, so the only thing that tells a
            // real seam from a phantom one on the far nappe is the cone FACE's
            // finite axial band [zlo, zhi]. Sample it once for any plane/cone
            // pair; task 38 uses it for the parallel-axis disjoint skip, and
            // task 47 carries it to the seam arm to bound the tilted ellipse.
            let mut cone_band: Option<(keel_geom::surface::Cone3, f64, f64)> = None;
            {
                let plane_cone = match (&ref_a, &ref_b) {
                    (
                        SurfaceRef::Analytic(Surface3::Plane(p)),
                        SurfaceRef::Analytic(Surface3::Cone(c)),
                    ) => Some((p.clone(), c.clone(), fb, false)),
                    (
                        SurfaceRef::Analytic(Surface3::Cone(c)),
                        SurfaceRef::Analytic(Surface3::Plane(p)),
                    ) => Some((p.clone(), c.clone(), fa, true)),
                    _ => None,
                };
                if let Some((p, c, cone_face, cone_is_a)) = plane_cone {
                    let body = if cone_is_a { a } else { b };
                    let mut zlo = f64::INFINITY;
                    let mut zhi = f64::NEG_INFINITY;
                    let mut sampled = true;
                    'fins: for &lk in body
                        .faces
                        .get(cone_face)
                        .map(|f| &f.loops)
                        .into_iter()
                        .flatten()
                    {
                        let Some(entry) = body.loops.get(lk).and_then(|l| l.fin) else {
                            continue;
                        };
                        let mut cur = entry;
                        while let Some(fin) = body.fins.get(cur) {
                            match body.fin_curve_samples(cur, 8) {
                                Some(pts) => {
                                    for q in pts {
                                        let z = (q - c.frame.origin).dot(c.frame.z);
                                        zlo = zlo.min(z);
                                        zhi = zhi.max(z);
                                    }
                                }
                                None => {
                                    sampled = false;
                                    break 'fins;
                                }
                            }
                            cur = fin.next;
                            if cur == entry {
                                break;
                            }
                        }
                    }
                    if sampled && zlo.is_finite() {
                        // task 38: a plane PARALLEL to the cone axis meets the
                        // infinite cone in a hyperbola (no section rung); when
                        // the plane clears the trimmed lateral's largest radius
                        // the faces provably never meet, so skip without the
                        // spurious IntersectionFailed every countersink carried.
                        if p.frame.z.dot(c.frame.z).abs() < 1e-9 {
                            let r_at = |z: f64| (c.radius + z * c.half_angle.tan()).abs();
                            let rmax = r_at(zlo).max(r_at(zhi));
                            let dist = (c.frame.origin - p.frame.origin).dot(p.frame.z).abs();
                            if dist > rmax + tol.max(1e-9) + 1e-9 {
                                continue; // provably disjoint: no SSI, no fault
                            }
                        }
                        cone_band = Some((c.clone(), zlo, zhi));
                    }
                }
            }
            match intersect_surfaces(&ref_a, &ref_b, tol) {
                // Cylinder/cylinder crossing pairs now ASSEMBLE through the
                // imprint: exact two-closed-conic seams (Steinmetz, task 29) via
                // the crossing imprint, and NON-planar encircling NURBS wraps
                // (the unequal-radius quartic) via the dossier-64 periodic-domain
                // band split (imprint_cylinder_wrap_bands). The former blanket
                // decline is gone. Only a genuinely OPEN cyl/cyl seam (a
                // degenerate / tangent branch that does not close) has no wrap to
                // band-split and still declines; the mass==mesh gate + op-volume
                // bound backstop any residue (soak FAIL=0 both seeds, guard off).
                Ok(SsiResult::Curves(cs))
                    if both_cyl
                        && !cs.is_empty()
                        && !cs.iter().all(|c| c.closed)
                        && cs
                            .iter()
                            .any(|c| a.curve_on_cylinder_face(fa, &c.curve, tol)) =>
                {
                    faults.push(BoolFault::UnassemblableSeam(id_a, id_b));
                }
                Ok(SsiResult::Curves(cs)) => {
                    for c in cs {
                        if c.tangential {
                            faults.push(BoolFault::Tangent(id_a, id_b));
                            continue;
                        }
                        // task 47: a plane/cone analytic section is the ellipse
                        // of the plane with the INFINITE cone. A slice near-
                        // parallel to a ruling runs the major axis tens of units
                        // up the nappe, far past the finite face. Such an
                        // ellipse cannot be a real seam, and sampling it in the
                        // overlap test below is the perf cliff. Decide it here
                        // by axial extent vs the sampled face band. A genuine
                        // on-face section is never taller than its own cone
                        // face, so this never trips a case that would assemble.
                        if let Some((cone, zlo, zhi)) = &cone_band
                            && let keel_geom::curve::Curve3::Ellipse(e) = &c.curve
                        {
                            let ax = cone.frame.z;
                            let half = ((e.a * e.x_axis.dot(ax)).powi(2)
                                + (e.b * e.y_axis.dot(ax)).powi(2))
                            .sqrt();
                            let cz = (e.center - cone.frame.origin).dot(ax);
                            let (eh_lo, eh_hi) = (cz - half, cz + half);
                            let band = *zhi - *zlo;
                            let m = band.max(cone.radius.abs()).max(tol) + 1e-9;
                            if eh_hi - eh_lo > band + m {
                                if eh_lo >= *zhi - m || eh_hi <= *zlo + m {
                                    continue; // ellipse off the finite band: no seam
                                }
                                // straddles a rim: the true section is an open
                                // arc exiting the base/top, which the imprint
                                // cannot assemble -> decline (fast, not after a
                                // giant-ellipse sampling pass).
                                faults.push(BoolFault::UnassemblableSeam(id_a, id_b));
                                continue;
                            }
                        }
                        // Plane-plane SSI is an UNBOUNDED line; clip it
                        // to both trimmed faces to get the real seam
                        // segment. Self-bounding curves (e.g. the SSI
                        // circle of two spheres) pass through unclipped.
                        if let (
                            keel_geom::curve::Curve3::Line(line),
                            SurfaceGeom::Analytic(Surface3::Plane(pa)),
                            SurfaceGeom::Analytic(Surface3::Plane(pb)),
                        ) = (&c.curve, &ga, &gb)
                        {
                            let pts_a = a.face_outer_loop_points(fa);
                            let pts_b = b.face_outer_loop_points(fb);
                            // A pristine cap's boundary is ONE circle edge (1
                            // vertex) -> the polygon clip needs >= 3, so use the
                            // analytic line/circle clip for a circular-boundary
                            // face (exact endpoints, so the cap chord's corners
                            // coincide with the wall rulings and the tool's trim
                            // ring closes). Straight-bounded faces keep the polygon.
                            let ia = if pts_a.len() >= 3 {
                                clip_line_to_planar_face(line, pa, &pts_a)
                            } else {
                                a.circle_face_line_clip(fa, line)
                            };
                            let ib = if pts_b.len() >= 3 {
                                clip_line_to_planar_face(line, pb, &pts_b)
                            } else {
                                b.circle_face_line_clip(fb, line)
                            };
                            if let (Some((a0, a1)), Some((b0, b1))) = (ia, ib) {
                                let t0 = a0.max(b0);
                                let t1 = a1.min(b1);
                                if t1 - t0 > tol {
                                    let p0 = line.point(t0);
                                    let p1 = line.point(t1);
                                    if let Ok(seg) = keel_geom::nurbs_curve::NurbsCurve::new(
                                        1,
                                        vec![0., 0., 1., 1.],
                                        vec![p0, p1],
                                        None,
                                    ) {
                                        seams.push(SeamCurve {
                                            face_a: fa,
                                            face_b: fb,
                                            curve: keel_geom::curve::Curve3::Nurbs(seg),
                                            closed: false,
                                            tol: c.tol_achieved,
                                            on_boundary_a: false,
                                            on_boundary_b: false,
                                        });
                                    }
                                }
                            }
                            continue;
                        }
                        // Open STRAIGHT seam (a Line) between a PLANAR face and
                        // a CURVED (cylinder/cone) face: the flat/slot-mill
                        // ruling. Drilling a round hole imprints a CLOSED circle
                        // (plane PERP to axis) and already assembles; milling a
                        // flat/slot is the dual -- a plane PARALLEL to the axis
                        // cuts the wall in a straight ruling that lies ON the
                        // curved face. The plane-plane clip above needs BOTH
                        // planar; here clip the line to the planar face's
                        // half-planes AND to the curved face's axial extent (the
                        // line's parameter span over the curved face's boundary),
                        // then push the bounded segment so imprint_open_chain can
                        // split BOTH faces boundary-to-boundary. Previously this
                        // fell through to `UnassemblableSeam` -> DECLINE.
                        if let keel_geom::curve::Curve3::Line(line) = &c.curve {
                            let is_plane =
                                |g: &SurfaceGeom| matches!(g, SurfaceGeom::Analytic(Surface3::Plane(_)));
                            let is_curved = |g: &SurfaceGeom| matches!(
                                g,
                                SurfaceGeom::Analytic(Surface3::Cylinder(_))
                                    | SurfaceGeom::Analytic(Surface3::Cone(_))
                            );
                            let planar_curved = (is_plane(&ga) && is_curved(&gb))
                                || (is_plane(&gb) && is_curved(&ga));
                            // DORMANT milling-seam machinery (KEEL_MILL_FLOW) -- the
                            // dual of drilling. Drilling imprints a CLOSED circle
                            // (plane PERP to axis) and assembles; milling a flat/slot
                            // is the same operation with the plane PARALLEL/OBLIQUE
                            // to the axis, whose SSI is an OPEN ruling line / ellipse
                            // arc. This clips that open seam to both faces and pushes
                            // it. Default-OFF because the downstream open-chain
                            // imprint cannot yet land a chain endpoint MID-ARC on a
                            // CIRCULAR rim edge: edge_containing_point /
                            // edge_on_face_containing / loop_edge_containing are
                            // straight-segment only (linear a+ab*t), so a point on a
                            // cylinder/cap rim circle reads INTERIOR and
                            // imprint_open_chain declines ("open chain end not on
                            // boundary"). Landing this needs curve-aware edge
                            // containment + a circle/arc split_edge -- a core
                            // primitive used across the assembler. Gated so the
                            // default path is byte-identical (DECLINE-never-WRONG)
                            // until that primitive lands and the soak re-validates.
                            if planar_curved {
                                // (plane, plane-face-points, curved-face-points)
                                let (pl, pts_pl, pts_cv) = if is_plane(&ga) {
                                    let SurfaceGeom::Analytic(Surface3::Plane(p)) = &ga else {
                                        unreachable!()
                                    };
                                    (p.clone(), a.face_outer_loop_points(fa), b.face_outer_loop_points(fb))
                                } else {
                                    let SurfaceGeom::Analytic(Surface3::Plane(p)) = &gb else {
                                        unreachable!()
                                    };
                                    (p.clone(), b.face_outer_loop_points(fb), a.face_outer_loop_points(fa))
                                };
                                if let Some((lp0, lp1)) = clip_line_to_planar_face(line, &pl, &pts_pl) {
                                    // Axial clip: the line's parameter span over
                                    // the curved face's boundary (the line is a
                                    // ruling lying on that face, so its endpoints
                                    // on the trimmed face are at the rim extent).
                                    let dd = line.dir.dot(line.dir).max(1e-30);
                                    let proj = |p: keel_math::vec::Vec3| (p - line.origin).dot(line.dir) / dd;
                                    let (mut lc0, mut lc1) = (f64::INFINITY, f64::NEG_INFINITY);
                                    for p in &pts_cv {
                                        let t = proj(*p);
                                        lc0 = lc0.min(t);
                                        lc1 = lc1.max(t);
                                    }
                                    let t0 = lp0.max(lc0);
                                    let t1 = lp1.min(lc1);
                                    if lc1 > lc0 && t1 - t0 > tol {
                                        let p0 = line.point(t0);
                                        let p1 = line.point(t1);
                                        if let Ok(seg) = keel_geom::nurbs_curve::NurbsCurve::new(
                                            1,
                                            vec![0., 0., 1., 1.],
                                            vec![p0, p1],
                                            None,
                                        ) {
                                            seams.push(SeamCurve {
                                                face_a: fa,
                                                face_b: fb,
                                                curve: keel_geom::curve::Curve3::Nurbs(seg),
                                                closed: false,
                                                tol: c.tol_achieved,
                                                on_boundary_a: false,
                                                on_boundary_b: false,
                                            });
                                        }
                                    }
                                }
                                continue;
                            }
                        }
                        // Surface-surface SSI uses the UNBOUNDED surfaces;
                        // a curve OFF both faces' trimmed extents is no
                        // seam (skip), and a curve PARTIALLY on a
                        // trimmed extent crosses the face boundary: the
                        // imprint cannot yet assemble that trim, so the
                        // boolean DECLINES (the notch probe's quarter-
                        // band wrong-positive class).
                        // A seam lying ON an existing boundary edge of a
                        // face is already topology on that side (the
                        // mated pin's cap circle IS the hole lateral's
                        // rim): spurious per file 39 sec 3.2 on BOTH
                        // sides means nothing to do; on ONE side the
                        // other operand still needs the split (the
                        // countersink plug's lateral crossing the sunk
                        // block's existing rim).
                        let on_a = a.curve_on_face_boundary_edges(fa, &c.curve, tol);
                        let on_b = b.curve_on_face_boundary_edges(fb, &c.curve, tol);
                        if on_a && on_b {
                            continue;
                        }
                        let ova = if on_a {
                            CurveFaceOverlap::All
                        } else {
                            a.curve_face_overlap(fa, &c.curve, tol)
                        };
                        let ovb = if on_b {
                            CurveFaceOverlap::All
                        } else {
                            b.curve_face_overlap(fb, &c.curve, tol)
                        };
                        match (ova, ovb) {
                            (CurveFaceOverlap::All, CurveFaceOverlap::All) => {}
                            (CurveFaceOverlap::None, _) | (_, CurveFaceOverlap::None) => {
                                continue;
                            }
                            _ => {
                                faults.push(BoolFault::UnassemblableSeam(id_a, id_b));
                                continue;
                            }
                        }
                        seams.push(SeamCurve {
                            face_a: fa,
                            face_b: fb,
                            curve: c.curve,
                            closed: c.closed,
                            tol: c.tol_achieved,
                            on_boundary_a: on_a,
                            on_boundary_b: on_b,
                        });
                    }
                }
                Ok(SsiResult::Coincident) => faults.push(BoolFault::Coincident(id_a, id_b)),
                Ok(SsiResult::Points(_)) => faults.push(BoolFault::Tangent(id_a, id_b)),
                Ok(SsiResult::Empty) => {}
                Err(_) => {
                    // SSI runs on the UNBOUNDED surfaces, so a NON-overlapping
                    // face pair (a countersink cone vs a hole across the plate)
                    // can still reach a far/unfittable intersection and Err --
                    // the dominant cone-face residual (LOG Add 289). When the
                    // faces' actual (bulge-safe) AABBs are disjoint by more than
                    // the chord margin there is provably no seam within
                    // tolerance: skip, don't fault. RESTRICTED to CONE-involved
                    // pairs: that is the unbounded-surface class. A non-cone SSI
                    // Err (a hard cyl/sphere config) stays an honest
                    // IntersectionFailed -- skipping it would only UNMASK a
                    // separate downstream assembly issue (a disjoint cyl/sphere
                    // union whose tessellated volume passes the gate while its
                    // render mesh disagrees) that the soak rightly flags.
                    let cone_involved = matches!(ref_a, SurfaceRef::Analytic(Surface3::Cone(_)))
                        || matches!(ref_b, SurfaceRef::Analytic(Surface3::Cone(_)));
                    let disjoint = cone_involved
                        && match (a.face_aabb(fa), b.face_aabb(fb)) {
                            (Some((alo, ahi)), Some((blo, bhi))) => {
                                // Margin > the 1e-4 face_aabb chord error so a
                                // real (touching) seam is never falsely skipped;
                                // far features sit orders of magnitude beyond it.
                                let m = 1e-3;
                                ahi.x + m < blo.x
                                    || bhi.x + m < alo.x
                                    || ahi.y + m < blo.y
                                    || bhi.y + m < alo.y
                                    || ahi.z + m < blo.z
                                    || bhi.z + m < alo.z
                            }
                            _ => false,
                        };
                    if !disjoint {
                        faults.push(BoolFault::IntersectionFailed(id_a, id_b));
                    }
                }
            }
        }
    }
    (seams, faults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::curve::Curve3;
    use keel_geom::surface::Frame3;
    use keel_math::vec::Vec3;

    /// Sphere centered at `center`, seamed with its pole on the x-axis
    /// and meridian in the world plane z = center.z (the equatorial
    /// plane through the center). For two such spheres displaced along
    /// z, the SSI latitude circle (at the midplane) never crosses
    /// either seam meridian, so imprints are crossing-free (M6a).
    fn z_sphere(center: Vec3, r: f64) -> Body {
        let mut b = Body::new();
        let frame = Frame3 {
            origin: center,
            x: Vec3::new(0., 1., 0.),
            y: Vec3::new(0., 0., 1.),
            z: Vec3::new(1., 0., 0.),
        };
        b.sphere(frame, r).unwrap();
        b
    }

    /// Axis-aligned block [origin, origin+ext] as its own body.
    fn block(origin: Vec3, ext: Vec3) -> Body {
        let mut b = Body::new();
        b.block(origin, ext.x, ext.y, ext.z).unwrap();
        b
    }

    fn z_cylinder(base: Vec3, r: f64, h: f64) -> Body {
        let mut b = Body::new();
        b.cylinder(Frame3::from_z(base, Vec3::new(0., 0., 1.)).unwrap(), r, h)
            .unwrap();
        b
    }

    #[test]
    fn selective_boolean_matches_full_when_complete() {
        // Item 31: selecting exactly the intersecting face pairs
        // reproduces the full boolean; dropping a needed pair DECLINES
        // (the gates refuse an unbounded selection) instead of lying.
        let a = block(Vec3::ZERO, Vec3::new(2., 2., 2.));
        let b = block(Vec3::new(1.0, 0.5, 0.5), Vec3::new(2., 2., 2.));
        let (seams, _) = seam_curves(&a, &b, 1e-7);
        let mut pairs: Vec<_> = seams.iter().map(|s| (s.face_a, s.face_b)).collect();
        pairs.dedup();
        let full = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        let sel = boolean_selective(&a, &b, BoolOp::Union, &pairs, 1e-7).unwrap();
        let (vf, vs) = (
            full.body.mass_properties().unwrap().volume,
            sel.body.mass_properties().unwrap().volume,
        );
        assert!(
            (vf - vs).abs() < 1e-9,
            "selective(all pairs) must match full ({vf} vs {vs})"
        );
        assert!(sel.body.validate().is_ok());

        // Remove every pair touching one seam's target face: incomplete.
        let drop_face = pairs[0].0;
        let partial: Vec<_> = pairs.iter().copied().filter(|p| p.0 != drop_face).collect();
        assert!(
            partial.len() < pairs.len()
                && boolean_selective(&a, &b, BoolOp::Union, &partial, 1e-7).is_err(),
            "an insufficient selection must decline"
        );
    }

    #[test]
    fn sheet_solid_boolean_trims_and_punches() {
        // Item 28 (sheet-target MVP): a 4x4 sheet at z=0 against a 2x2x2
        // solid punching through its middle. Intersection = the interior
        // 2x2 patch; Difference = the holed ring sheet (area 12); Union
        // of mixed dimension declines.
        use crate::query::BodyClass;
        let sheet = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
        ])
        .unwrap();
        let tool = block(Vec3::new(1.0, 1.0, -1.0), Vec3::new(2.0, 2.0, 2.0));

        let inside = boolean_sheet_solid(&sheet, &tool, BoolOp::Intersection, 1e-7).unwrap();
        assert!(inside.validate().is_ok(), "inside sheet invalid");
        assert_eq!(inside.body_class(), BodyClass::Sheet);
        assert_eq!(inside.face_keys().len(), 1, "one interior patch");
        let a = inside.face_area(inside.face_keys()[0]);
        assert!((a - 4.0).abs() < 1e-9, "interior patch area {a} != 4");

        let outside = boolean_sheet_solid(&sheet, &tool, BoolOp::Difference, 1e-7).unwrap();
        assert!(outside.validate().is_ok(), "ring sheet invalid");
        assert_eq!(outside.body_class(), BodyClass::Sheet);
        assert_eq!(outside.face_keys().len(), 1, "one holed ring face");
        assert_eq!(
            outside.counts().inner_rings,
            1,
            "the punch is an inner ring"
        );
        // SIGNED area against the sheet normal: the ring fan is reversed,
        // so it subtracts: 16 - 4 = 12.
        let n = Vec3::new(0.0, 0.0, 1.0);
        let signed: f64 = outside
            .tessellate_face(outside.face_keys()[0])
            .iter()
            .map(|t| 0.5 * n.dot((t[1] - t[0]).cross(t[2] - t[0])))
            .sum();
        assert!(
            (signed.abs() - 12.0).abs() < 1e-9,
            "holed sheet area {signed} != 12"
        );

        assert!(
            boolean_sheet_solid(&sheet, &tool, BoolOp::Union, 1e-7).is_err(),
            "mixed-dimension union must decline"
        );
    }

    #[test]
    fn sheet_solid_guillotine_difference() {
        // A solid overlapping one side: Difference keeps the open-chain
        // trimmed remainder (no ring), Intersection the covered strip.
        use crate::query::BodyClass;
        let sheet = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(4.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ])
        .unwrap();
        // Covers x in [3,5]: strip 1x2 inside.
        let tool = block(Vec3::new(3.0, -1.0, -1.0), Vec3::new(2.0, 4.0, 2.0));
        let outside = boolean_sheet_solid(&sheet, &tool, BoolOp::Difference, 1e-7).unwrap();
        assert!(outside.validate().is_ok());
        assert_eq!(outside.body_class(), BodyClass::Sheet);
        let a: f64 = outside
            .face_keys()
            .iter()
            .map(|&f| outside.face_area(f))
            .sum();
        assert!((a - 6.0).abs() < 1e-9, "kept sheet area {a} != 6");
        let inside = boolean_sheet_solid(&sheet, &tool, BoolOp::Intersection, 1e-7).unwrap();
        let a: f64 = inside
            .face_keys()
            .iter()
            .map(|&f| inside.face_area(f))
            .sum();
        assert!((a - 2.0).abs() < 1e-9, "covered strip area {a} != 2");
    }

    #[test]
    fn imprint_body_splits_topology_not_geometry() {
        // Item 74: imprinting a corner-overlapping tool splits faces and
        // adds seam edges but leaves the geometry bit-identical: volume
        // unchanged, mass == mesh, valid.
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., 2., 2.), Vec3::new(4., 4., 4.));
        let imp = a.imprint_body(&b, 1e-7).unwrap();
        assert!(imp.validate().is_ok(), "imprinted body invalid");
        let c0 = a.counts();
        let c1 = imp.counts();
        assert!(
            c1.f > c0.f && c1.e > c0.e,
            "imprint must split faces/add edges (got {c0:?} -> {c1:?})"
        );
        let v = imp.mass_properties().unwrap().volume;
        let mv = imp.mesh_volume();
        assert!(
            (v - 64.0).abs() < 1e-9 && (mv - 64.0).abs() < 1e-9,
            "imprint must not change geometry (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn through_notch_difference_is_genus_one() {
        // Interior THROUGH-hole difference: a 2x2x0.5 slab minus a
        // 0.5x0.5 block piercing it completely. Exercises two fixes:
        // (1) multi-component imprint -- each tool wall is cut by the
        // slab's top AND bottom into two DISJOINT segments (neither one
        // closed loop nor one open chain), imprinted per component with
        // planar relocation; (2) component-genus stamping in
        // finalize_imported_assembly -- the result is ONE genus-1 shell
        // pair (V16 E24 F10 R2: 16-24+10-2 = 0 = 2(1-1)).
        let a = block(Vec3::new(0., 0., -0.25), Vec3::new(2., 2., 0.5));
        let b = block(Vec3::new(0.75, 0.75, -0.5), Vec3::new(0.5, 0.5, 1.0));
        let res = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "through-notch slab invalid: {:?}",
            res.body.validate()
        );
        assert!(res.faults.is_empty(), "faults: {:?}", res.faults);
        let c = res.body.counts();
        assert_eq!((c.v, c.e, c.f), (16, 24, 10), "through-hole counts");
        assert_eq!(c.inner_rings, 2, "ring on top and bottom");
        assert_eq!(c.genus, 1, "a through-hole is genus 1");
        let v = res.body.mass_properties().unwrap().volume;
        let mv = res.body.mesh_volume();
        let want = 2.0 - 0.5 * 0.5 * 0.5;
        assert!(
            (v - want).abs() < 1e-9 && (mv - want).abs() < 1e-9,
            "through-notch volume must be {want} with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn through_notch_intersection_is_core() {
        // Same operands, Intersection: the tool clipped to the slab's
        // z-range, an ordinary 0.5 x 0.5 x 0.5 box (genus 0). Checks the
        // multi-component imprint feeds the other select branches too.
        let a = block(Vec3::new(0., 0., -0.25), Vec3::new(2., 2., 0.5));
        let b = block(Vec3::new(0.75, 0.75, -0.5), Vec3::new(0.5, 0.5, 1.0));
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(res.body.validate().is_ok(), "core invalid");
        assert!(res.faults.is_empty(), "faults: {:?}", res.faults);
        let c = res.body.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "core is a box");
        let v = res.body.mass_properties().unwrap().volume;
        let mv = res.body.mesh_volume();
        assert!(
            (v - 0.125).abs() < 1e-9 && (mv - 0.125).abs() < 1e-9,
            "core volume must be 0.125 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn block_minus_cylinder_blind_hole() {
        // Block [0,4]^3 drilled from the top by a radius-1 cylinder on
        // the central axis, from z=2 (hole floor) up through z=4 and out
        // to z=6. A - B removes the cylinder segment z in [2,4]:
        // V = 4^3 - pi*1^2*2 = 64 - 2pi.
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = z_cylinder(Vec3::new(2., 2., 2.), 1.0, 4.0);
        let res = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "drilled block invalid: {:?}",
            res.faults
        );
        let v = res.body.tessellated_volume();
        let exp = 64.0 - 2.0 * core::f64::consts::PI;
        assert!((v - exp).abs() < 0.03 * exp, "drilled volume {v} vs {exp}");
        let w_solid = res
            .body
            .generalized_winding_number(Vec3::new(0.5, 0.5, 2.0));
        let w_hole = res
            .body
            .generalized_winding_number(Vec3::new(2.0, 2.0, 3.0));
        assert!(w_solid > 0.5, "material winding {w_solid} should be inside");
        assert!(w_hole < 0.5, "hole winding {w_hole} should be outside");
    }

    #[test]
    fn block_intersect_cylinder_is_a_plug() {
        // A ∩ B = the cylinder segment inside the block: radius 1,
        // z in [2,4] plug. V = pi*1^2*2.
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = z_cylinder(Vec3::new(2., 2., 2.), 1.0, 4.0);
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "plug invalid: {:?}",
            res.faults
        );
        let v = res.body.tessellated_volume();
        let exp = core::f64::consts::PI * 2.0;
        assert!((v - exp).abs() < 0.03 * exp, "plug volume {v} vs {exp}");
    }

    #[test]
    fn guillotine_seam_segments_are_four_clipped_edges() {
        // A = [0,4]^3. B overlaps A in a slab x in [2,4], spanning A
        // fully in y and z, so B's only cutting plane in A is x = 2.
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let (seams, faults) = seam_curves(&a, &b, 1e-9);
        // Four side faces of A (y=0,y=4,z=0,z=4) each cut once by x=2;
        // A's x-faces are parallel to the cut (coincident/empty); the
        // parallel pair may report a coincidence fault, which is fine.
        let segs: Vec<_> = seams.iter().collect();
        assert_eq!(
            segs.len(),
            4,
            "expected 4 clipped seam segments, got {} (faults {:?})",
            segs.len(),
            faults
        );
        // Every seam segment lies in the plane x = 2 and is length 4.
        for s in &segs {
            let p0 = sample_curve(&s.curve, 0.0);
            let p1 = sample_curve(&s.curve, 1.0);
            assert!(
                (p0.x - 2.0).abs() < 1e-9 && (p1.x - 2.0).abs() < 1e-9,
                "seam off x=2: {p0:?} {p1:?}"
            );
            assert!(
                (((p1 - p0).norm()) - 4.0).abs() < 1e-9,
                "seam length {} != 4",
                (p1 - p0).norm()
            );
        }
    }

    fn sample_curve(c: &Curve3, t: f64) -> Vec3 {
        match c {
            Curve3::Nurbs(n) => {
                let (a, b) = n.domain();
                n.point(a + t * (b - a))
            }
            Curve3::Line(l) => l.point(t),
            Curve3::Circle(ci) => ci.point(core::f64::consts::TAU * t),
            Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
        }
    }

    #[test]
    fn two_spheres_seam_is_one_circle() {
        // Unit spheres centered at z=0 and z=1.5 intersect in a single
        // latitude circle at z = 0.75 (by symmetry), radius
        // sqrt(1 - 0.75^2).
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let (seams, faults) = seam_curves(&a, &b, 1e-9);
        assert!(faults.is_empty(), "unexpected faults: {faults:?}");
        assert_eq!(seams.len(), 1, "expected one seam circle");
        let s = &seams[0];
        assert!(s.closed);
        match &s.curve {
            Curve3::Circle(c) => {
                assert!((c.center.z - 0.75).abs() < 1e-9, "center z {}", c.center.z);
                let expected_r = (1.0f64 - 0.75 * 0.75).sqrt();
                assert!((c.radius - expected_r).abs() < 1e-9, "radius {}", c.radius);
            }
            other => panic!("expected circle, got {other:?}"),
        }
    }

    #[test]
    fn guillotine_classify_fragments() {
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let (ia, ib, faults) = imprint_pair(&a, &b, 1e-7);
        assert!(faults.is_empty(), "faults: {faults:?}");
        // A's faces vs B: the x>2 fragments are inside B, x<2 outside.
        let ca = classify_faces(&ia.body, &b, 1e-7);
        let inside_a = ca
            .iter()
            .filter(|(_, c)| *c == FaceClass::InsideOther)
            .count();
        let outside_a = ca
            .iter()
            .filter(|(_, c)| *c == FaceClass::OutsideOther)
            .count();
        let bad_a = ca
            .iter()
            .filter(|(_, c)| matches!(c, FaceClass::Unknown | FaceClass::OnOther(_)))
            .count();
        assert_eq!(bad_a, 0, "A unclassified/coincident: {ca:?}");
        // 4 cut side-faces -> 4 inside fragments + 4 outside fragments;
        // x=4 face inside, x=0 face outside. => 5 inside, 5 outside.
        assert_eq!((inside_a, outside_a), (5, 5), "A class counts {ca:?}");
        // B's faces vs A: the inner rectangle on x=2 is inside A; its
        // outer remainder and the far faces are outside A.
        let cb = classify_faces(&ib.body, &a, 1e-7);
        let inside_b = cb
            .iter()
            .filter(|(_, c)| *c == FaceClass::InsideOther)
            .count();
        let bad_b = cb
            .iter()
            .filter(|(_, c)| matches!(c, FaceClass::Unknown | FaceClass::OnOther(_)))
            .count();
        assert_eq!(bad_b, 0, "B unclassified/coincident: {cb:?}");
        assert_eq!(
            inside_b, 1,
            "only B's inner x=2 rectangle is inside A: {cb:?}"
        );
    }

    #[test]
    fn guillotine_intersection_is_a_box() {
        // A = [0,4]^3, B overlaps in x in [2,4]; A ∩ B = [2,4]x[0,4]x[0,4],
        // volume 2*4*4 = 32.
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(res.body.validate().is_ok(), "result invalid");
        assert!(res.faults.is_empty(), "faults: {:?}", res.faults);
        let c = res.body.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "intersection is a box");
        let vol = res.body.mass_properties().unwrap().volume;
        assert!((vol - 32.0).abs() < 1e-6, "intersection volume {vol} != 32");
    }

    #[test]
    fn guillotine_difference_is_a_box() {
        // A - B = [0,2]x[0,4]x[0,4], volume 2*4*4 = 32.
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let res = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(res.body.validate().is_ok(), "result invalid");
        let c = res.body.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "difference is a box");
        let vol = res.body.mass_properties().unwrap().volume;
        assert!((vol - 32.0).abs() < 1e-6, "difference volume {vol} != 32");
    }

    #[test]
    fn corner_overlap_intersection_is_unit_cube() {
        // A = [0,2]^3, B = [1,3]^3 overlap at the octant [1,2]^3.
        // Every participating face is cut by an L (two segments meeting
        // at an interior corner). A ∩ B = unit cube, volume 1.
        let a = block(Vec3::ZERO, Vec3::new(2., 2., 2.));
        let b = block(Vec3::new(1., 1., 1.), Vec3::new(2., 2., 2.));
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(res.body.validate().is_ok(), "invalid: {:?}", res.faults);
        assert!(res.faults.is_empty(), "faults: {:?}", res.faults);
        let c = res.body.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "intersection is a unit cube");
        let vol = res.body.mass_properties().unwrap().volume;
        assert!((vol - 1.0).abs() < 1e-6, "corner-overlap volume {vol} != 1");
    }

    #[test]
    fn corner_overlap_difference_volume() {
        // A - B removes the [1,2]^3 octant from [0,2]^3: volume 8 - 1 = 7.
        let a = block(Vec3::ZERO, Vec3::new(2., 2., 2.));
        let b = block(Vec3::new(1., 1., 1.), Vec3::new(2., 2., 2.));
        let res = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(res.body.validate().is_ok(), "invalid: {:?}", res.faults);
        let vol = res.body.mass_properties().unwrap().volume;
        assert!((vol - 7.0).abs() < 1e-6, "A-B volume {vol} != 7");
    }

    #[test]
    fn tilted_guillotine_difference_volume() {
        // A NON-45-degree planar guillotine: remove the wedge z > 0.5x + 1
        // from [0,4]^3. The removed region has volume 32, so A - B = 32.
        // The cut face is tilted (slope atan(0.5)) and non-axis-aligned --
        // coverage for the file-46 sense-based planar integrand on a face
        // whose plane frame is oblique to the world axes. mass_properties
        // and the mesh oracle must agree.
        use keel_math::transform::Transform3;
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let theta = (0.5_f64).atan();
        let rot = Transform3::from_rotation(Vec3::new(0., 1., 0.), -theta).unwrap();
        let trans = Transform3::from_translation(Vec3::new(0., 0., 1.));
        // A big slab occupying its local +z half-space; rotated so its
        // bottom face becomes the plane z = 0.5x + 1, removed side up.
        let slab = block(Vec3::new(-20., -20., 0.), Vec3::new(40., 40., 40.));
        let b = slab.transformed(&rot.then(trans)).unwrap();
        let res = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "tilted cut invalid: {:?}",
            res.faults
        );
        let mp = res.body.mass_properties().unwrap().volume;
        let mv = res.body.mesh_volume();
        assert!(
            (mp - 32.0).abs() < 1e-6,
            "tilted-cut mass_properties {mp} != 32 (mesh {mv})"
        );
        assert!(
            (mp - mv).abs() < 0.1,
            "mass_properties {mp} vs mesh {mv} diverge on tilted cut"
        );
    }

    #[test]
    fn near_degenerate_intersection_no_wrong_answer() {
        // A thin sliver intersection at a large coordinate (scale
        // disparity vs tol) found by fuzzing. M6a may decline it, but
        // must NEVER return a wrong "valid" body: any Ok result has a
        // positive, finite volume; otherwise it is a clean Err.
        let mut a = Body::new();
        a.block(Vec3::new(0.0005, 0.0005, 0.0005), 0.5, 20.0, 0.5)
            .unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(0.0, 20.0, 0.0), 0.5, 0.5, 0.5).unwrap();
        // Declining a near-degenerate config is fine; an Ok result must
        // have positive finite volume (never a wrong "valid" body).
        if let Ok(res) = boolean(&a, &b, BoolOp::Intersection, 1e-7) {
            let v = res.body.mass_properties().map(|m| m.volume);
            assert!(
                matches!(v, Ok(vol) if vol.is_finite() && vol > 0.0),
                "Ok result must have positive finite volume, got {v:?}"
            );
        }
    }

    #[test]
    fn near_coincident_touch_declines() {
        // Tall thin box touched by a small box at a coincident face
        // (fuzz finding): never a wrong "valid" body. Historically this
        // declined; with the touch-only rule the regularized
        // intersection is the clean EMPTY body, which is the exact
        // answer (the contact is measure-zero).
        let a = block(Vec3::ZERO, Vec3::new(0.5, 0.5, 20.0));
        let b = block(Vec3::new(0.0, 0.0, 20.0), Vec3::new(0.5, 0.5, 0.5));
        if let Ok(res) = boolean(&a, &b, BoolOp::Intersection, 1e-7) {
            let v = res.body.mass_properties().map(|m| m.volume);
            let empty = res.body.mesh_volume().abs() <= 1e-9 && v.is_err();
            assert!(
                empty || matches!(v, Ok(vol) if vol.is_finite() && vol > 0.0),
                "got {v:?}"
            );
        }
    }

    #[test]
    fn boolean_is_deterministic() {
        // Same inputs -> identical result topology hash (determinism D9).
        let build = || {
            let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
            let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
            boolean(&a, &b, BoolOp::Intersection, 1e-7)
                .unwrap()
                .body
                .topology_hash()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn nested_boxes_no_seam() {
        // B entirely inside A: no surface intersection. A ∩ B = B,
        // A ∪ B = A. (A - B would enclose a void: 3 regions, beyond the
        // 2-region stitch; deferred.)
        let a = block(Vec3::ZERO, Vec3::new(3., 3., 3.));
        let b = block(Vec3::new(1., 1., 1.), Vec3::new(1., 1., 1.));
        let inter = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(inter.body.validate().is_ok());
        let vi = inter.body.mass_properties().unwrap().volume;
        assert!((vi - 1.0).abs() < 1e-6, "A∩B (nested) volume {vi} != 1");
        let uni = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(uni.body.validate().is_ok());
        let vu = uni.body.mass_properties().unwrap().volume;
        assert!((vu - 27.0).abs() < 1e-6, "A∪B (nested) volume {vu} != 27");
    }

    #[test]
    fn intersection_is_commutative_by_volume() {
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let ab = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        let ba = boolean(&b, &a, BoolOp::Intersection, 1e-7).unwrap();
        let va = ab.body.mass_properties().unwrap().volume;
        let vb = ba.body.mass_properties().unwrap().volume;
        assert!((va - vb).abs() < 1e-6, "A∩B vol {va} != B∩A vol {vb}");
        assert!((va - 32.0).abs() < 1e-6);
    }

    #[test]
    fn intersection_translation_invariant() {
        // Translating both operands translates the result; volume holds.
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let shift = Vec3::new(10.0, -3.0, 7.0);
        let a2 = block(shift, Vec3::new(4., 4., 4.));
        let b2 = block(Vec3::new(2., -1., -1.) + shift, Vec3::new(4., 6., 6.));
        let v1 = boolean(&a, &b, BoolOp::Intersection, 1e-7)
            .unwrap()
            .body
            .mass_properties()
            .unwrap()
            .volume;
        let v2 = boolean(&a2, &b2, BoolOp::Intersection, 1e-7)
            .unwrap()
            .body
            .mass_properties()
            .unwrap()
            .volume;
        assert!(
            (v1 - v2).abs() < 1e-6,
            "translation changed volume: {v1} vs {v2}"
        );
    }

    #[test]
    fn guillotine_selection_counts() {
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let (ia, ib, _) = imprint_pair(&a, &b, 1e-7);
        let ca = classify_faces(&ia.body, &b, 1e-7);
        let cb = classify_faces(&ib.body, &a, 1e-7);
        // Intersection [2,4]x[0,4]x[0,4] = a box: 6 faces.
        let inter = select_faces(BoolOp::Intersection, &ca, &cb);
        assert_eq!(inter.len(), 6, "intersection keeps 6 faces");
        // Difference A-B [0,2]x[0,4]x[0,4] = a box: 6 faces (1 reversed).
        let diff = select_faces(BoolOp::Difference, &ca, &cb);
        assert_eq!(diff.len(), 6, "difference keeps 6 faces");
        assert_eq!(
            diff.iter().filter(|k| k.reversed).count(),
            1,
            "one reversed B face"
        );
        // Union keeps A-outside(5) + B-outside(6) = 11 faces.
        let uni = select_faces(BoolOp::Union, &ca, &cb);
        assert_eq!(uni.len(), 11, "union keeps 11 faces");
    }

    #[test]
    fn guillotine_imprint_pair() {
        let a = block(Vec3::ZERO, Vec3::new(4., 4., 4.));
        let b = block(Vec3::new(2., -1., -1.), Vec3::new(4., 6., 6.));
        let (ia, ib, faults) = imprint_pair(&a, &b, 1e-7);
        assert!(faults.is_empty(), "faults: {faults:?}");
        assert!(ia.body.validate().is_ok());
        assert!(ib.body.validate().is_ok());
        // A: four side faces each split by one segment => 6 + 4 = 10
        // faces. (x-faces uncut, 4 side faces -> 8, +2 x-faces = 10.)
        assert_eq!(ia.body.counts().f, 10, "A faces after imprint");
        // B: the cutting face x=2 gains an interior rectangle ring,
        // splitting into ring + outer => 6 + 1 = 7 faces.
        assert_eq!(ib.body.counts().f, 7, "B faces after imprint");
        assert_eq!(ia.seam_edges.len(), 4);
        // B's interior seam ring is now SUBDIVIDED at its 4 corners (file
        // 47) so it matches A's 4 open seam edges -- 1 closed ring -> 4
        // open sides -- which is what lets the stitch pair the seam.
        assert_eq!(ib.seam_edges.len(), 4);
    }

    #[test]
    fn sphere_sphere_intersection_is_a_lens() {
        // Two unit spheres at z=0 and z=1.5. A ∩ B = a lens (two
        // spherical caps glued at the SSI circle). Cap height
        // h = R - d/2 = 0.25; V_lens = 2*(pi h^2/3)(3R - h) ~= 0.3601.
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "lens invalid: {:?}",
            res.faults
        );
        assert_eq!(res.body.counts().f, 2, "lens has two cap faces");
        // The lens midpoint is inside both spheres -> inside the result;
        // a point just outside the lens is out.
        let w_in = res
            .body
            .generalized_winding_number(Vec3::new(0.0, 0.0, 0.75));
        let w_out = res
            .body
            .generalized_winding_number(Vec3::new(0.0, 0.0, 2.0));
        assert!(w_in > 0.5, "lens midpoint winding {w_in} should be inside");
        assert!(
            w_out < 0.5,
            "point above lens winding {w_out} should be outside"
        );
        // Volume via the tessellated oracle (exact trimmed-cap mass
        // properties is the staged Task-5 item); coarse, so 5% tol.
        let h = 0.25;
        let v_lens = 2.0 * (core::f64::consts::PI * h * h / 3.0) * (3.0 - h);
        let v = res.body.tessellated_volume();
        assert!(
            (v - v_lens).abs() < 0.05 * v_lens,
            "lens volume {v} vs exact {v_lens}"
        );
    }

    fn z_nurbs_sphere(center: Vec3, r: f64) -> Body {
        let mut b = Body::new();
        let frame = Frame3 {
            origin: center,
            x: Vec3::new(0., 1., 0.),
            y: Vec3::new(0., 0., 1.),
            z: Vec3::new(1., 0., 0.),
        };
        b.nurbs_sphere(frame, r).unwrap();
        b
    }

    #[test]
    fn nurbs_sphere_intersect_analytic_sphere() {
        // THE FIRST NURBS BOOLEAN (M7b): a NURBS sphere intersected with
        // an analytic sphere, both equatorially seamed (crossing-free).
        // The SSI is the tier-2 analytic-vs-spline circle. Result = lens.
        let a = z_nurbs_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-6).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "nurbs lens invalid: {:?}",
            res.faults
        );
        assert_eq!(res.body.counts().f, 2, "lens has two cap faces");
        let h = 0.25;
        let v_lens = 2.0 * (core::f64::consts::PI * h * h / 3.0) * (3.0 - h);
        let v = res.body.tessellated_volume();
        assert!(
            (v - v_lens).abs() < 0.06 * v_lens,
            "nurbs lens volume {v} vs {v_lens}"
        );
        // Lens midpoint inside, point above outside.
        let w_in = res
            .body
            .generalized_winding_number(Vec3::new(0.0, 0.0, 0.75));
        assert!(w_in > 0.5, "nurbs lens midpoint winding {w_in}");
    }

    #[test]
    fn nurbs_sphere_boolean_recovers_to_exact() {
        // M8 affordability gradient: a NURBS sphere is a hidden quadric,
        // so canonical recovery recognizes it and the SSI routes to the
        // EXACT tier-1 sphere-sphere circle. The result is exact (achieved
        // tolerance at the floor), not tolerant -- the hidden quadric pays
        // quadric prices. (Before M8 this case used the tier-2 fit and
        // carried a ~4e-7 bound; recovery removes that approximation. The
        // other side of the gradient -- genuine free-form geometry that
        // recovery REJECTS, keeping the spline and its certified tolerant
        // bound -- is the M7b tolerant-edge contract plus the recovery
        // rejection gate `recover::tests::freeform_is_kept`.)
        let a = z_nurbs_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-6).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "lens invalid: {:?}",
            res.faults
        );
        assert_eq!(res.body.counts().f, 2, "lens has two cap faces");
        let eps = res.body.achieved_tolerance();
        assert!(
            eps < 1e-7,
            "recovered-exact seam should sit at the floor, got {eps:e}"
        );
        assert!(
            res.body.epsilon_solid(eps),
            "epsilon-solid at the floor {eps:e}"
        );
    }

    #[test]
    fn nurbs_nurbs_boolean_recovers_to_exact_lens() {
        // The retired M7c blocker: TWO NURBS spheres. Before M8 this
        // ground the tier-3 spline solver (~190s) and the fitted seam
        // failed to split either face (kept=0). With canonical recovery
        // both operands are recognized as exact spheres, the SSI is the
        // tier-1 circle, and the M6c crossing-imprint splits both into a
        // valid two-cap lens -- fast and exact.
        let a = z_nurbs_sphere(Vec3::ZERO, 1.0);
        let b = z_nurbs_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let res = boolean(&a, &b, BoolOp::Intersection, 1e-6).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "nurbs-nurbs lens invalid: {:?}",
            res.faults
        );
        assert_eq!(res.body.counts().f, 2, "lens has two cap faces");
        let h = 0.25;
        let v_lens = 2.0 * (core::f64::consts::PI * h * h / 3.0) * (3.0 - h);
        let v = res.body.tessellated_volume();
        // Coarse NURBS-cap tessellation (the curved volume oracle) is
        // good to a few percent, as for the M7b analytic-x-NURBS lens.
        assert!(
            (v - v_lens).abs() < 0.05 * v_lens,
            "nurbs-nurbs lens volume {v} vs {v_lens}"
        );
    }

    #[test]
    fn coincident_abutting_box_union() {
        // Two unit boxes sharing the x=1 face (coincident, opposite-
        // oriented). Union -> a 2x1x1 box (volume 2). Previously DECLINED
        // (Coincident); now handled (the shared faces classify OnOther and
        // are dropped as interior, the rest glue).
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let res = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "coincident union invalid: {:?}",
            res.faults
        );
        let v = res.body.mass_properties().unwrap().volume;
        assert!((v - 2.0).abs() < 1e-9, "coincident union volume {v} != 2");
    }

    #[test]
    fn cone_minus_slab_is_two_solids_with_correct_mass() {
        // The first overlapping-curved cone boolean: a cone cut by a
        // perpendicular slab (two circle seams) leaves a DISCONNECTED result,
        // a bottom frustum plus a top tip. Both pieces must assemble with the
        // tip's lateral kept (apex-aware classify) AND tessellating (apex-aware
        // tessellate_cone): mass == mesh == frustum + tip. (LOG Add. 255.)
        let mut cone = Body::new();
        cone.cone(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            2.0,
            3.0,
        )
        .unwrap();
        let mut slab = Body::new();
        slab.block(Vec3::new(-3.0, -3.0, 0.8), 6.0, 6.0, 1.0)
            .unwrap();
        let r = boolean(&cone, &slab, BoolOp::Difference, 1e-7).unwrap();
        assert!(r.faults.is_empty(), "faults {:?}", r.faults);
        assert!(r.body.validate().is_ok(), "invalid body");
        let mass = r.body.mass_properties().unwrap().volume;
        let mesh = r.body.mesh_volume();
        // Truth: frustum z[0,0.8] + tip z[1.8,3] of a cone r=2 h=3.
        let pi = core::f64::consts::PI;
        let r08 = 2.0 * (1.0 - 0.8 / 3.0);
        let frustum = pi / 3.0 * 0.8 * (4.0 + 2.0 * r08 + r08 * r08);
        let tip = pi / 3.0 * 0.8 * 0.8 * 1.2;
        let truth = frustum + tip;
        assert!(
            (mass - truth).abs() < 2e-2 * truth,
            "mass {mass} vs truth {truth}"
        );
        assert!(
            (mass - mesh).abs() < 3e-2 * (1.0 + mass),
            "mass {mass} mesh {mesh}"
        );
        // Two disconnected solid cells (frustum + tip).
        let solids = r.body.regions.iter().filter(|(_, rg)| rg.solid).count();
        assert_eq!(solids, 2, "expected 2 solid cells, got {solids}");
    }

    #[test]
    fn coaxial_cone_cylinder_seam_assembles() {
        // The exact cone x coaxial-cylinder SSI rung (ssi::cone_cylinder, LOG
        // Add. 259): a cone and a coaxial cylinder meet in the single circle at
        // the axial height where the cone radius equals the cylinder radius.
        // Before this rung EVERY cone-cylinder boolean declined
        // IntersectionFailed (the SSI returned Degenerate).
        //
        // Cone base r=2 at z=0, apex z=3 (slope -2/3). Coaxial cylinder r=1,
        // z in [0.3,2]: caps clear of the apex and the base plane, so the only
        // interaction is the lateral seam circle at z=1.5 (cone radius==1) plus
        // the cyl-top section at z=2. Union and (cyl - cone) assemble cleanly:
        // mass == mesh == analytic truth.
        let pi = core::f64::consts::PI;
        let mut cone = Body::new();
        cone.cone(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            2.0,
            3.0,
        )
        .unwrap();
        let mut cyl = Body::new();
        cyl.cylinder(
            Frame3::from_z(Vec3::new(0., 0., 0.3), Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
            1.7,
        )
        .unwrap();
        // inter = pi*1.2 (cyl part z[0.3,1.5]) + integral_{1.5}^{2} pi*(2-2z/3)^2 dz.
        let inter = (1.2 + 28.5 / 81.0) * pi;
        let cone_v = 4.0 * pi;
        let cyl_v = 1.7 * pi;

        let clean = |a: &Body, b: &Body, op: BoolOp, truth: f64| {
            let r = boolean(a, b, op, 1e-7).unwrap_or_else(|e| panic!("declined {e:?}"));
            assert!(r.faults.is_empty(), "faults {:?}", r.faults);
            assert!(r.body.validate().is_ok(), "invalid body");
            let mass = r.body.mass_properties().unwrap().volume;
            let mesh = r.body.mesh_volume();
            assert!(
                (mass - truth).abs() < 2e-2 * (1.0 + truth),
                "mass {mass} vs truth {truth}"
            );
            assert!(
                (mass - mesh).abs() < 3e-2 * (1.0 + mass),
                "mass {mass} mesh {mesh}"
            );
        };
        clean(&cone, &cyl, BoolOp::Union, cone_v + cyl_v - inter);
        clean(&cyl, &cone, BoolOp::Difference, cyl_v - inter);

        // The intersection and (cone - cyl) currently DECLINE on a separate
        // tessellation limitation (a cap floating inside the cone -> a blind
        // hole / pinch whose mesh disagrees with the exact mass). They must
        // stay DECLINE-safe: if either ever assembles it must be CORRECT, never
        // a wrong Ok. The exact mass equals truth in both, proving the seam
        // itself is right.
        for (a, b, op, truth) in [
            (&cone, &cyl, BoolOp::Intersection, inter),
            (&cone, &cyl, BoolOp::Difference, cone_v - inter),
        ] {
            if let Ok(r) = boolean(a, b, op, 1e-7) {
                let mass = r.body.mass_properties().unwrap().volume;
                assert!(
                    (mass - truth).abs() < 3e-2 * (1.0 + truth),
                    "wrong Ok mass {mass} vs truth {truth}"
                );
            }
        }
    }

    #[test]
    fn coaxial_cone_sphere_all_ops_pass() {
        // Coaxial cone (base r=2, apex z=3, m=-2/3) + sphere (centre z=1.5,
        // R=1, fully inside the cone z-range) -> two transversal seam circles.
        // Needs cone_sphere SSI + the sphere carve + band fallback + multi-rim
        // clip. All 4 ops mass==mesh==truth (truth by cross-section integral).
        let pi = core::f64::consts::PI;
        let cone_r = |z: f64| (2.0 - (2.0 / 3.0) * z).max(0.0);
        let sph_r = |z: f64| {
            let t = 1.0 - (z - 1.5) * (z - 1.5);
            if t > 0.0 { t.sqrt() } else { 0.0 }
        };
        let n = 200_000;
        let (zlo, zhi) = (0.5_f64, 2.5_f64);
        let dz = (zhi - zlo) / n as f64;
        let mut inter = 0.0;
        for i in 0..n {
            let z = zlo + (i as f64 + 0.5) * dz;
            let r = cone_r(z).min(sph_r(z));
            inter += pi * r * r * dz;
        }
        let cone_v = 4.0 * pi;
        let sphere_v = 4.0 / 3.0 * pi;
        let mut cn = Body::new();
        cn.cone(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            2.0,
            3.0,
        )
        .unwrap();
        let mut sp = Body::new();
        sp.sphere(
            Frame3::from_z(Vec3::new(0., 0., 1.5), Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
        )
        .unwrap();
        let check = |a: &Body, b: &Body, op: BoolOp, truth: f64| {
            let r = boolean(a, b, op, 1e-7).unwrap_or_else(|e| panic!("declined {e:?}"));
            assert!(r.faults.is_empty(), "faults {:?}", r.faults);
            assert!(r.body.validate().is_ok(), "invalid body");
            let mass = r.body.mass_properties().unwrap().volume;
            let mesh = r.body.mesh_volume();
            assert!(
                (mass - truth).abs() < 2e-2 * (1.0 + truth),
                "mass {mass} vs truth {truth}"
            );
            assert!(
                (mass - mesh).abs() < 3e-2 * (1.0 + mass),
                "mass {mass} mesh {mesh}"
            );
        };
        check(&cn, &sp, BoolOp::Intersection, inter);
        check(&cn, &sp, BoolOp::Union, cone_v + sphere_v - inter);
        check(&cn, &sp, BoolOp::Difference, cone_v - inter);
        check(&sp, &cn, BoolOp::Difference, sphere_v - inter);
    }

    #[test]
    fn coaxial_cyl_sphere_all_ops_pass() {
        // The dominant cyl/sph decline class, coaxial rung: a rod through a
        // ball (sphere R=2 at origin, cylinder r=1 z[-3,3], caps outside the
        // sphere; seam circles z=+-sqrt(3)). Needs cylinder_sphere SSI + the
        // sphere carve + the band mid-latitude fallback (seam+pole margin) +
        // the multi-rim tessellation clip. All 4 ops mass==mesh==truth.
        let pi = core::f64::consts::PI;
        let s3 = 3.0_f64.sqrt();
        let inter = pi * 2.0 * s3 + 2.0 * pi * ((16.0 / 3.0) - 3.0 * s3);
        let sphere_v = 32.0 * pi / 3.0;
        let cyl_v = 6.0 * pi;
        let mut sp = Body::new();
        sp.sphere(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            2.0,
        )
        .unwrap();
        let mut cl = Body::new();
        cl.cylinder(
            Frame3::from_z(Vec3::new(0., 0., -3.0), Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
            6.0,
        )
        .unwrap();
        let check = |a: &Body, b: &Body, op: BoolOp, truth: f64| {
            let r = boolean(a, b, op, 1e-7).unwrap_or_else(|e| panic!("declined {e:?}"));
            assert!(r.faults.is_empty(), "faults {:?}", r.faults);
            assert!(r.body.validate().is_ok(), "invalid body");
            let mass = r.body.mass_properties().unwrap().volume;
            let mesh = r.body.mesh_volume();
            assert!(
                (mass - truth).abs() < 2e-2 * (1.0 + truth),
                "mass {mass} vs truth {truth}"
            );
            assert!(
                (mass - mesh).abs() < 3e-2 * (1.0 + mass),
                "mass {mass} mesh {mesh}"
            );
        };
        check(&sp, &cl, BoolOp::Intersection, inter);
        check(&sp, &cl, BoolOp::Union, sphere_v + cyl_v - inter);
        check(&sp, &cl, BoolOp::Difference, sphere_v - inter);
        check(&cl, &sp, BoolOp::Difference, cyl_v - inter);
    }

    #[test]
    fn sphere_slab_carve_three_zones_pass() {
        // The sphere 3-zone carve: a sphere cut by a slab into two pole caps
        // plus a pole-free mid BAND. Difference keeps the two caps;
        // intersection keeps the band. Both PASS (mass == mesh == truth) only
        // with all three sphere fixes: the band-classify interior point
        // (Add.262), the sphere tessellation rim-clip (Add.265), and the
        // offset-robust mesh_volume (Add.264). Before, sph - slab read the
        // FULL-sphere mass (classify mis-kept the band).
        let mut sphere = Body::new();
        sphere
            .sphere(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                2.0,
            )
            .unwrap();
        let mut slab = Body::new();
        slab.block(Vec3::new(-5.0, -5.0, -0.5), 10.0, 10.0, 1.0)
            .unwrap();
        let pi = core::f64::consts::PI;
        let cap = pi * 1.5 * 1.5 * (3.0 * 2.0 - 1.5) / 3.0; // cap of height 1.5, R=2
        let full = 32.0 * pi / 3.0;
        let check = |op: BoolOp, truth: f64| {
            let r = boolean(&sphere, &slab, op, 1e-7).unwrap_or_else(|e| panic!("declined {e:?}"));
            assert!(r.faults.is_empty(), "faults {:?}", r.faults);
            assert!(r.body.validate().is_ok(), "invalid body");
            let mass = r.body.mass_properties().unwrap().volume;
            let mesh = r.body.mesh_volume();
            assert!(
                (mass - truth).abs() < 2e-2 * (1.0 + truth),
                "mass {mass} vs truth {truth}"
            );
            assert!(
                (mass - mesh).abs() < 3e-2 * (1.0 + mass),
                "mass {mass} mesh {mesh}"
            );
        };
        check(BoolOp::Difference, 2.0 * cap); // two caps
        check(BoolOp::Intersection, full - 2.0 * cap); // mid band
    }

    #[test]
    fn sphere_band_interior_point_is_on_the_band() {
        // A sphere split by a slab into THREE zones: two pole caps + a mid
        // BAND. The band has NO pole, so its classify interior point must lie
        // ON the band (between the two cuts), not at a pole. The old logic's
        // off-rim witness found the OTHER rim and returned a pole OUTSIDE the
        // band, so classify mis-kept it (sph - slab read FULL-sphere mass).
        // LOG Add. 262: a gated band branch (>=2 distinct parallel circle
        // rims, point_in_face_uv-verified) returns a mid-latitude point.
        let mut sphere = Body::new();
        sphere
            .sphere(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                2.0,
            )
            .unwrap();
        let mut slab = Body::new();
        slab.block(Vec3::new(-5.0, -5.0, -0.5), 10.0, 10.0, 1.0)
            .unwrap();
        let (ia, _ib, _f) = imprint_pair(&sphere, &slab, 1e-7);
        // The band is the sphere face whose tessellation stays inside the slab
        // (|z| < 0.9); the two caps reach the poles at z = +-2.
        let band = ia
            .body
            .face_keys()
            .into_iter()
            .find(|&f| {
                matches!(ia.body.face_surface3(f), Some(Surface3::Sphere(_))) && {
                    let tris = ia.body.tessellate_face(f);
                    !tris.is_empty() && tris.iter().flatten().all(|p| p.z.abs() < 0.9)
                }
            })
            .expect("no band face found among the imprinted sphere zones");
        let p = ia
            .body
            .sphere_face_interior_point(band)
            .expect("band face has no interior point");
        assert!(
            p.z.abs() < 0.5,
            "band interior point z={} is not on the band",
            p.z
        );
        assert!(
            (p.norm() - 2.0).abs() < 1e-6,
            "interior point off the sphere"
        );
    }

    #[test]
    fn overlapping_sphere_difference_never_returns_malformed_ok() {
        // KL5 / LOG Add.258: a heavy / equal-radius sphere-sphere difference
        // must NEVER return an Ok body whose mass declines (the malformed empty
        // body a mesh-only consumer would read as 0). It must PASS with a valid
        // mass or DECLINE honestly. Equal radii give the difference lower bound
        // lo == 0, so the op-volume guard alone cannot catch the wrong empty;
        // the A-not-contained-in-B surface probe does.
        let mk = |c: Vec3| {
            let mut b = Body::new();
            b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), 1.5)
                .unwrap();
            b
        };
        let a = mk(Vec3::ZERO);
        let b = mk(Vec3::new(1.5, 0., 0.));
        match boolean(&a, &b, BoolOp::Difference, 1e-7) {
            Err(_) => {} // honest decline is acceptable
            Ok(r) => assert!(
                r.body.mass_properties().is_ok(),
                "sphere-sphere difference returned a MALFORMED Ok (mass declines)"
            ),
        }
    }

    #[test]
    fn disjoint_operands_return_clean_answers() {
        // Dossier 29 (graceful degradation, the leg's first rung):
        // operands that do not touch are NOT an error class.
        // Intersection = the clean EMPTY result, difference = A
        // unchanged, union = the disconnected two-component body.
        // Exact oracles all three; no faults; the empty result is a
        // faceless body whose mesh volume is zero.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(3.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let i = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(i.faults.is_empty(), "intersection faults {:?}", i.faults);
        assert_eq!(i.body.face_keys().len(), 0, "empty body has no faces");
        assert!(i.body.mesh_volume().abs() < 1e-12, "empty volume");
        assert!(i.body.validate().is_ok(), "empty body invalid");
        let d = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(d.faults.is_empty(), "difference faults {:?}", d.faults);
        assert!(d.body.validate().is_ok(), "difference body invalid");
        let (vd, md) = (
            d.body.mass_properties().unwrap().volume,
            d.body.mesh_volume(),
        );
        assert!(
            (vd - 1.0).abs() < 1e-9 && (md - 1.0).abs() < 1e-9,
            "difference {vd}/{md} != A"
        );
        let d2 = boolean(&b, &a, BoolOp::Difference, 1e-7).unwrap();
        let v2 = d2.body.mass_properties().unwrap().volume;
        assert!((v2 - 1.0).abs() < 1e-9, "reversed difference {v2} != B");
        let u = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(u.faults.is_empty(), "union faults {:?}", u.faults);
        assert!(u.body.validate().is_ok(), "union body invalid");
        let (vu, mu) = (
            u.body.mass_properties().unwrap().volume,
            u.body.mesh_volume(),
        );
        assert!(
            (vu - 2.0).abs() < 1e-9 && (mu - 2.0).abs() < 1e-9,
            "union {vu}/{mu} != both components"
        );
        // Both components really are present, each as its own solid
        // cell (Euler-Poincare counts closed shells as regions - 1).
        let w1 = u.body.generalized_winding_number(Vec3::new(0.5, 0.5, 0.5));
        let w2 = u.body.generalized_winding_number(Vec3::new(3.5, 0.5, 0.5));
        assert!(
            (w1 - 1.0).abs() < 1e-3 && (w2 - 1.0).abs() < 1e-3,
            "components {w1} / {w2}"
        );
        let solids = u.body.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(solids, 2, "one solid cell per disconnected component");

        // NESTED operands (A strictly inside BIG): containment without
        // contact. Intersection = A, union = BIG, A - BIG = empty,
        // BIG - A = the cavity body (volume 26, with a void region).
        let mut big = Body::new();
        big.block(Vec3::new(-1.0, -1.0, -1.0), 3.0, 3.0, 3.0)
            .unwrap();
        let ni = boolean(&a, &big, BoolOp::Intersection, 1e-7).unwrap();
        let vni = ni.body.mass_properties().unwrap().volume;
        assert!((vni - 1.0).abs() < 1e-9, "nested intersection {vni} != A");
        let nu = boolean(&a, &big, BoolOp::Union, 1e-7).unwrap();
        let vnu = nu.body.mass_properties().unwrap().volume;
        assert!((vnu - 27.0).abs() < 1e-9, "nested union {vnu} != BIG");
        let nd = boolean(&a, &big, BoolOp::Difference, 1e-7).unwrap();
        assert_eq!(nd.body.face_keys().len(), 0, "swallowed difference empty");
        let nc = boolean(&big, &a, BoolOp::Difference, 1e-7).unwrap();
        assert!(nc.body.validate().is_ok(), "cavity body invalid");
        let (vc, mc) = (
            nc.body.mass_properties().unwrap().volume,
            nc.body.mesh_volume(),
        );
        assert!(
            (vc - 26.0).abs() < 1e-9 && (mc - 26.0).abs() < 1e-9,
            "cavity difference {vc}/{mc} != 26"
        );
        let voids = nc
            .body
            .regions
            .iter()
            .filter(|(_, r)| !r.solid && !r.infinite)
            .count();
        assert_eq!(voids, 1, "the cavity is a void region");
    }

    #[test]
    fn tolerant_boolean_snaps_near_coincident_contact() {
        // Dossier 29 M2 (Tier 2), the dossier-39 prepare phase: the
        // tolerant boolean SNAPS near-coincident planar contact to
        // exact coincidence within the caller-bounded fuzz (the OCCT
        // fuzzy-value semantics, ACIS's prepare), runs the strict
        // pipeline once, and reports honestly: clean tier 1 when
        // nothing moved, salvaged tier 2 with the ACHIEVED tolerance
        // when something did. Strict mode is untouched: the gapped
        // pair is two disjoint components to boolean(); design intent
        // (a flush mating face) is the tolerant caller's to declare.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        // Exact abutment: tier 1, nothing to snap.
        let mut b0 = Body::new();
        b0.block(Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let (r0, c0) = boolean_tolerant(&a, &b0, BoolOp::Union, 1e-7, 1e-4).unwrap();
        assert!(!c0.salvaged, "exact contact is clean");
        assert_eq!(c0.tier, 1);
        let v0 = r0.body.mass_properties().unwrap().volume;
        assert!((v0 - 2.0).abs() < 1e-9, "abutting union {v0}");
        // Near-touch with a 1e-5 GAP: strict sees two disjoint solids;
        // tolerant snaps the mating faces together: ONE component of
        // volume 2 (the snapped configuration, exactly), salvaged,
        // achieved tolerance >= the gap and <= the fuzz.
        let gap = 1e-5;
        let mut bg = Body::new();
        bg.block(Vec3::new(1.0 + gap, 0.0, 0.0), 1.0, 1.0, 1.0)
            .unwrap();
        let strict = boolean(&a, &bg, BoolOp::Union, 1e-7).unwrap();
        let strict_solids = strict.body.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(strict_solids, 2, "strict honors the literal gap");
        let (rg, cg) = boolean_tolerant(&a, &bg, BoolOp::Union, 1e-7, 1e-4).unwrap();
        assert!(cg.salvaged, "gap closure is salvage, never silent");
        assert_eq!(cg.tier, 2);
        assert!(
            cg.achieved_tolerance >= gap * 0.99 && cg.achieved_tolerance <= 1e-4,
            "achieved {}",
            cg.achieved_tolerance
        );
        // Face-snap semantics: the MATING face moves onto A's plane;
        // B's far side stays put (the prepare makes the PAIR
        // coincident, it does not rigidly translate the body). The
        // snapped configuration is exact: volume 2 + gap.
        let vg = rg.body.mass_properties().unwrap().volume;
        assert!((vg - (2.0 + gap)).abs() < 1e-9, "snapped union {vg}");
        let solids = rg.body.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(solids, 1, "the mated pair is one component");
        // Near-OVERLAP (1e-5 penetration): same salvage; the snapped
        // configuration's exact volume is 2 - gap.
        let mut bo = Body::new();
        bo.block(Vec3::new(1.0 - gap, 0.0, 0.0), 1.0, 1.0, 1.0)
            .unwrap();
        let (ro, co) = boolean_tolerant(&a, &bo, BoolOp::Union, 1e-7, 1e-4).unwrap();
        assert!(co.salvaged);
        let vo = ro.body.mass_properties().unwrap().volume;
        assert!(
            (vo - (2.0 - gap)).abs() < 1e-9,
            "snapped overlap union {vo}"
        );
        // Fuzz is a hard cap: a gap beyond it stays two components.
        let mut bf = Body::new();
        bf.block(Vec3::new(1.001, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let (rf, cf) = boolean_tolerant(&a, &bf, BoolOp::Union, 1e-7, 1e-4).unwrap();
        assert!(!cf.salvaged, "beyond-fuzz contact must not snap");
        let sf = rf.body.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(sf, 2);
    }

    #[test]
    fn partial_touch_booleans_are_clean() {
        // Dossier 29 M3 / dossier 39 sec 1: PARTIAL face-overlap
        // contact (B abuts A's wall offset by half in both cross
        // axes, so the overlap-boundary cuts CHAIN through a corner
        // interior to each wall). The spur-chain polyline imprint
        // splits both walls into uniform fragments and the Requicha
        // tables produce exact clean answers: empty intersection,
        // identity differences, the connected partial-abutment union.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 0.5, 0.5), 1.0, 1.0, 1.0).unwrap();
        let i = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert_eq!(
            i.body.face_keys().len(),
            0,
            "partial touch intersects empty"
        );
        let d = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        let vd = d.body.mass_properties().unwrap().volume;
        assert!((vd - 1.0).abs() < 1e-9, "A - B {vd} != A");
        let d2 = boolean(&b, &a, BoolOp::Difference, 1e-7).unwrap();
        let v2 = d2.body.mass_properties().unwrap().volume;
        assert!((v2 - 1.0).abs() < 1e-9, "B - A {v2} != B");
        let u = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(u.body.validate().is_ok(), "partial-abutment union invalid");
        let (vu, mu) = (
            u.body.mass_properties().unwrap().volume,
            u.body.mesh_volume(),
        );
        assert!(
            (vu - 2.0).abs() < 1e-9 && (mu - 2.0).abs() < 1e-9,
            "partial-abutment union {vu}/{mu} != 2"
        );
    }

    #[test]
    fn enclosed_pocket_touch_booleans_are_clean() {
        // The ENCLOSED-POCKET coincidence: a small box stands on a big
        // box's top face, its mating face strictly inside the host
        // face (the overlap boundary is a closed ring, never reaching
        // the host's edges). The ring imprint splits the host wall
        // into annulus + pocket; the Requicha tables then give exact
        // clean answers for all three ops.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 3.0, 3.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 1.0, 1.0), 1.0, 1.0, 1.0).unwrap();
        let i = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert_eq!(i.body.face_keys().len(), 0, "pocket touch intersects empty");
        let d = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        let vd = d.body.mass_properties().unwrap().volume;
        assert!((vd - 9.0).abs() < 1e-9, "A - B {vd} != A");
        let u = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(u.body.validate().is_ok(), "pocket union invalid");
        let (vu, mu) = (
            u.body.mass_properties().unwrap().volume,
            u.body.mesh_volume(),
        );
        assert!(
            (vu - 10.0).abs() < 1e-9 && (mu - 10.0).abs() < 1e-9,
            "pocket union {vu}/{mu} != 10"
        );
        // The host wall survives as an annulus: a face with an inner
        // ring around the pocket rim.
        let ringed = u
            .body
            .face_keys()
            .into_iter()
            .filter(|&f| u.body.faces.get(f).map(|x| x.loops.len()).unwrap_or(0) > 1)
            .count();
        assert_eq!(ringed, 1, "one annular host wall");
    }

    #[test]
    fn touching_intersection_and_difference_are_clean() {
        // Touch-only contact (exact abutment): the regularized
        // intersection is EMPTY and the difference is A unchanged,
        // both clean strict answers (Requicha on-on), not declines.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let i = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert_eq!(i.body.face_keys().len(), 0, "touching intersection empty");
        let d = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        let vd = d.body.mass_properties().unwrap().volume;
        assert!((vd - 1.0).abs() < 1e-9, "touching difference {vd} != A");
    }

    #[test]
    fn pin_in_hole_booleans_are_clean() {
        // The canonical CAD mating case (dossier 29 M4): a plate with
        // a through hole and the pin that EXACTLY fills it. The
        // laterals are coincident-opposite cylinders (dropped by the
        // on-on tables); the pin's cap discs TILE the plate's annular
        // faces in-plane (zero overlap), which requires REAL carrier
        // containment (an AABB guard false-ONs the disc against the
        // annulus) and the dossier-39 two-sided band test (the disc
        // centre sits at the hole mouth where the one-point winding
        // is ambiguous). Union = the solid box, exactly; difference =
        // the holed plate unchanged; intersection = empty.
        use keel_geom::surface::Frame3;
        let pi = core::f64::consts::PI;
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut drill = Body::new();
        drill.cylinder(dframe, 1.0, 2.0).unwrap();
        let holed = boolean(&plate, &drill, BoolOp::Difference, 1e-7)
            .unwrap()
            .body;
        let vh = holed.mass_properties().unwrap().volume;
        // The M5 chordal self-consistency witness: the drilled body's
        // chordal mesh must track its exact mass within the curved
        // gate's band (the seam-split lateral previously tessellated
        // tau - pi/8 of the ring, a 16 percent mesh deficit that the
        // old positive-volume-only gate let through).
        let mesh_h = holed.mesh_volume();
        assert!(
            (mesh_h - vh).abs() <= 2e-2 * (1.0 + vh.abs()),
            "holed plate mesh {mesh_h} vs mass {vh}: outside the chordal band"
        );
        assert!((vh - (16.0 - pi)).abs() < 1e-9, "holed plate {vh}");
        let pframe = Frame3::from_z(Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut pin = Body::new();
        pin.cylinder(pframe, 1.0, 1.0).unwrap();
        let u = boolean(&holed, &pin, BoolOp::Union, 1e-7).unwrap();
        assert!(u.body.validate().is_ok(), "pin union invalid");
        let (vu, mu) = (
            u.body.mass_properties().unwrap().volume,
            u.body.mesh_volume(),
        );
        assert!(
            (vu - 16.0).abs() < 1e-9 && (mu - 16.0).abs() < 1e-9,
            "pin union {vu}/{mu} != solid plate"
        );
        // No cylinder faces survive: the mated laterals are interior.
        let cyls = u
            .body
            .face_keys()
            .into_iter()
            .filter(|&f| matches!(u.body.face_surface3(f), Some(Surface3::Cylinder(_))))
            .count();
        assert_eq!(cyls, 0, "mated laterals must drop");
        let d2 = boolean(&holed, &pin, BoolOp::Difference, 1e-7).unwrap();
        let vd2 = d2.body.mass_properties().unwrap().volume;
        assert!(
            (vd2 - (16.0 - pi)).abs() < 1e-9,
            "pin difference {vd2} != holed plate"
        );
        let i2 = boolean(&holed, &pin, BoolOp::Intersection, 1e-7).unwrap();
        assert_eq!(i2.body.face_keys().len(), 0, "boundary-only intersection");
    }

    #[test]
    fn tolerant_radial_gap_pin_snaps_exact() {
        // The M6 oracle (dossier 29 tier 2, dossier 39 prepare): a pin
        // whose radius is 1e-5 UNDER the hole (the classic clearance
        // fit exported as-is) and a pin whose axis is offset 3e-6.
        // Strict sees parallel non-touching laterals; the tolerant
        // prepare snaps the pin's lateral onto the hole's EXACT
        // carrier (radius, axis, rims, ruling), after which the strict
        // pipeline produces the mated-pin exact results.
        use keel_geom::surface::Frame3;
        let pi = core::f64::consts::PI;
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut drill = Body::new();
        drill.cylinder(dframe, 1.0, 2.0).unwrap();
        let holed = boolean(&plate, &drill, BoolOp::Difference, 1e-7)
            .unwrap()
            .body;
        let pframe = Frame3::from_z(Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut pin = Body::new();
        pin.cylinder(pframe, 1.0 - 1e-5, 1.0).unwrap();
        let (u, conf) = boolean_tolerant(&holed, &pin, BoolOp::Union, 1e-7, 1e-4).unwrap();
        assert!(
            conf.salvaged && conf.tier == 2,
            "radial gap must salvage: {:?}",
            (conf.salvaged, conf.tier)
        );
        assert!(
            conf.achieved_tolerance >= 1e-6 && conf.achieved_tolerance <= 1e-4,
            "achieved {}",
            conf.achieved_tolerance
        );
        let (vu, mu) = (
            u.body.mass_properties().unwrap().volume,
            u.body.mesh_volume(),
        );
        assert!(
            (vu - 16.0).abs() < 1e-9 && (mu - 16.0).abs() < 1e-9,
            "snapped pin union {vu}/{mu} != solid plate"
        );
        let (d, _) = boolean_tolerant(&holed, &pin, BoolOp::Difference, 1e-7, 1e-4).unwrap();
        let vd = d.body.mass_properties().unwrap().volume;
        assert!((vd - (16.0 - pi)).abs() < 1e-9, "snapped difference {vd}");
        // The axis-offset same-radius pin snaps the same way.
        let oframe =
            Frame3::from_z(Vec3::new(2.0 + 3e-6, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut pin2 = Body::new();
        pin2.cylinder(oframe, 1.0, 1.0).unwrap();
        let (u2, c2) = boolean_tolerant(&holed, &pin2, BoolOp::Union, 1e-7, 1e-4).unwrap();
        assert!(c2.salvaged, "offset pin must salvage");
        let v2 = u2.body.mass_properties().unwrap().volume;
        assert!((v2 - 16.0).abs() < 1e-9, "offset pin union {v2}");
    }

    #[test]
    fn nonregularized_union_keeps_interface_wall() {
        // Item 29 Rung 1 (dossier 57): fuse two unit cubes sharing the
        // x=1 face WITHOUT regularizing. The shared face survives as a
        // DOUBLE-SIDED interior partition wall; the result is a CELLULAR
        // solid: TWO solid regions of mass 1 each inside one 2x1x1 outer
        // shell, mass == mesh == 2 over the outer boundary, and the
        // wall's boundary edges carry radial-3 cycles (wall + the two
        // outer walls). The regularized default is unchanged.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let res = boolean_with(
            &a,
            &b,
            BoolOp::Union,
            1e-7,
            BooleanOptions { regularize: false },
        )
        .unwrap();
        let body = &res.body;
        assert!(
            body.validate().is_ok(),
            "cellular union invalid: {:?}",
            res.faults
        );
        let solids = body.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(solids, 2, "two solid cells");
        let wall_count = body
            .face_keys()
            .iter()
            .filter(|&&f| body.is_interior_wall(f))
            .count();
        assert_eq!(wall_count, 1, "one interface wall");
        let radial3 = body
            .edges
            .iter()
            .filter(|(_, e)| e.radial.len() == 3)
            .count();
        assert_eq!(radial3, 4, "the wall's four boundary edges are radial-3");
        let v = body.mass_properties().unwrap().volume;
        let mv = body.mesh_volume();
        assert!(
            (v - 2.0).abs() < 1e-9 && (mv - 2.0).abs() < 1e-9,
            "outer-boundary mass {v} / mesh {mv} != 2"
        );
        // The winding classifier (outer boundary only) sees both cells.
        let w1 = body.generalized_winding_number(Vec3::new(0.5, 0.5, 0.5));
        let w2 = body.generalized_winding_number(Vec3::new(1.5, 0.5, 0.5));
        assert!(
            (w1 - 1.0).abs() < 1e-3 && (w2 - 1.0).abs() < 1e-3,
            "winding {w1} / {w2}"
        );
        // Regularized default: one box, one solid region (no regression).
        let reg = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        let solids_reg = reg.body.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(solids_reg, 1, "regularized union stays one cell");
    }

    #[test]
    fn crossing_cylinders_decline_pending_seam_assembly() {
        // The Steinmetz configuration (two perpendicular cylinders
        // crossing). The crossing-pair imprint (task 29) assembles the
        // INTERSECTION exactly: (16/3) r^3, no pi anywhere (the classic
        // bicylinder). The old wrong-positive (an Euler-valid body at
        // 12.5) stays dead: every op is EXACT-OR-DECLINE, an Ok result
        // must agree with the closed forms and its own mesh.
        let mut a = Body::new();
        a.cylinder(
            Frame3::from_z(Vec3::new(0., 0., -3.), Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
            6.0,
        )
        .unwrap();
        let mut b = Body::new();
        b.cylinder(
            Frame3::from_z(Vec3::new(-3., 0., 0.), Vec3::new(1., 0., 0.)).unwrap(),
            1.0,
            6.0,
        )
        .unwrap();
        let bicyl = 16.0 / 3.0;
        let cyl = core::f64::consts::PI * 6.0;
        for (op, exact) in [
            (BoolOp::Intersection, bicyl),
            (BoolOp::Union, 2.0 * cyl - bicyl),
            (BoolOp::Difference, cyl - bicyl),
        ] {
            match boolean(&a, &b, op, 1e-5) {
                Err(_) => {} // honest decline stays legal
                Ok(r) => {
                    assert!(r.body.validate().is_ok(), "{op:?}: invalid body");
                    let mv = r.body.mesh_volume();
                    match r.body.mass_properties() {
                        Ok(m) => {
                            assert!(
                                (m.volume - exact).abs() < 1e-9,
                                "{op:?}: volume {} vs exact {exact}",
                                m.volume
                            );
                            assert!(
                                (mv - m.volume).abs() <= 2e-2 * (1.0 + m.volume.abs()),
                                "{op:?}: mesh {mv} vs mass {}",
                                m.volume
                            );
                        }
                        // Mass may degrade gracefully on band faces;
                        // the mesh must still match the closed form
                        // within the chordal band.
                        Err(_) => assert!(
                            (mv - exact).abs() <= 2e-2 * (1.0 + exact.abs()),
                            "{op:?}: mesh {mv} vs exact {exact}"
                        ),
                    }
                }
            }
        }
        // Disjoint cylinder bodies do NOT trip the gate (their SSI is
        // empty on the trimmed faces; whatever the empty-intersection
        // outcome, it is not an UnassemblableSeam).
        let mut far = Body::new();
        far.cylinder(
            Frame3::from_z(Vec3::new(20., 0., 0.), Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
            2.0,
        )
        .unwrap();
        assert!(!matches!(
            boolean(&a, &far, BoolOp::Intersection, 1e-5),
            Err(BoolFault::UnassemblableSeam(..))
        ));
    }

    #[test]
    fn wire_solid_booleans_trim_by_containment() {
        // Dossier 57 Rung 4 oracle: a segment crossing a cube keeps
        // exactly its inside length (intersection) and the two outside
        // tails (difference); union of mixed dimension declines.
        let mut cube = Body::new();
        cube.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let mut w = Body::new();
        w.wire(Vec3::new(-1.0, 1.0, 1.0), Vec3::new(3.0, 1.0, 1.0))
            .unwrap();
        let len = |b: &Body| -> f64 {
            b.edges
                .iter()
                .map(|(_, e)| {
                    let (a, c) = e.bounds;
                    (b.vertices.get(c).unwrap().point - b.vertices.get(a).unwrap().point).norm()
                })
                .sum()
        };
        let i = boolean_wire_solid(&w, &cube, BoolOp::Intersection, 1e-7).unwrap();
        assert!(i.validate().is_ok());
        assert!((len(&i) - 2.0).abs() < 1e-9, "inside length {}", len(&i));
        let d = boolean_wire_solid(&w, &cube, BoolOp::Difference, 1e-7).unwrap();
        assert!((len(&d) - 2.0).abs() < 1e-9, "outside length {}", len(&d));
        assert_eq!(d.edges.iter().count(), 2, "two outside tails");
        assert!(boolean_wire_solid(&w, &cube, BoolOp::Union, 1e-7).is_err());
        let mut far = Body::new();
        far.wire(Vec3::new(10., 10., 10.), Vec3::new(11., 10., 10.))
            .unwrap();
        assert!(boolean_wire_solid(&far, &cube, BoolOp::Intersection, 1e-7).is_err());
    }

    #[test]
    fn sheet_sheet_booleans_are_the_2d_arrangement() {
        // Dossier 57 Rung 3 oracle: two overlapping 4x4 coplanar
        // sheets offset by 2: intersection area 8, difference area 8,
        // union area 24, all exact; disjoint union 32; crossing
        // sheets DECLINE.
        let z = Vec3::new(0., 0., 1.);
        let x = Vec3::new(1., 0., 0.);
        let a = Body::rectangular_sheet(Vec3::ZERO, z, x, 4.0, 4.0).unwrap();
        let b = Body::rectangular_sheet(Vec3::new(2.0, 0.0, 0.0), z, x, 4.0, 4.0).unwrap();
        let i = boolean_sheet_sheet(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!(i.validate().is_ok());
        assert!(
            (i.surface_area() - 8.0).abs() < 1e-9,
            "i {}",
            i.surface_area()
        );
        let d = boolean_sheet_sheet(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(d.validate().is_ok());
        assert!(
            (d.surface_area() - 8.0).abs() < 1e-9,
            "d {}",
            d.surface_area()
        );
        let u = boolean_sheet_sheet(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(u.validate().is_ok());
        assert!(
            (u.surface_area() - 24.0).abs() < 1e-9,
            "u {}",
            u.surface_area()
        );
        // Disjoint: union is both, intersection declines empty.
        let far = Body::rectangular_sheet(Vec3::new(10.0, 0.0, 0.0), z, x, 4.0, 4.0).unwrap();
        let u2 = boolean_sheet_sheet(&a, &far, BoolOp::Union, 1e-7).unwrap();
        assert!((u2.surface_area() - 32.0).abs() < 1e-9);
        assert!(boolean_sheet_sheet(&a, &far, BoolOp::Intersection, 1e-7).is_err());
        // Crossing sheets decline (the wire intersection is rung 4/5).
        let v = Body::rectangular_sheet(Vec3::new(2.0, 0.0, -2.0), x, z, 4.0, 4.0).unwrap();
        assert!(boolean_sheet_sheet(&a, &v, BoolOp::Intersection, 1e-7).is_err());
    }

    #[test]
    fn sheet_as_knife_partitions_cube_into_two_cells() {
        // Item 29 Rung 2 (dossier 57): a 4x4 planar sheet at z = 1 cuts
        // the [0,2]^3 cube into TWO solid cells separated by the
        // trimmed 2x2 interior wall. The outer boundary is unchanged:
        // mass == mesh == 8; the wall is double-sided with area 4 and
        // four radial-3 boundary edges; the winding classifier (outer
        // boundary only) sees both cells as inside.
        let mut cube = Body::new();
        cube.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let sheet = Body::rectangular_sheet(
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            4.0,
            4.0,
        )
        .unwrap();
        let body = partition_by_sheet(&cube, &sheet, 1e-7).unwrap();
        assert!(body.validate().is_ok(), "partitioned cube invalid");
        let solids = body.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(solids, 2, "two solid cells");
        let wall_faces: Vec<_> = body
            .face_keys()
            .into_iter()
            .filter(|&f| body.is_interior_wall(f))
            .collect();
        assert_eq!(wall_faces.len(), 1, "one interior wall face");
        let area = body.face_area(wall_faces[0]);
        assert!((area - 4.0).abs() < 1e-9, "wall area {area} != 4");
        let radial3 = body
            .edges
            .iter()
            .filter(|(_, e)| e.radial.len() == 3)
            .count();
        assert_eq!(radial3, 4, "the wall ring is radial-3");
        let v = body.mass_properties().unwrap().volume;
        let mv = body.mesh_volume();
        assert!(
            (v - 8.0).abs() < 1e-9 && (mv - 8.0).abs() < 1e-9,
            "outer-boundary mass {v} / mesh {mv} != 8"
        );
        let w_lo = body.generalized_winding_number(Vec3::new(1.0, 1.0, 0.5));
        let w_hi = body.generalized_winding_number(Vec3::new(1.0, 1.0, 1.5));
        assert!(
            (w_lo - 1.0).abs() < 1e-3 && (w_hi - 1.0).abs() < 1e-3,
            "winding {w_lo} / {w_hi}"
        );
        // A sheet that misses the solid declines.
        let far = Body::rectangular_sheet(
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
            1.0,
        )
        .unwrap();
        assert!(partition_by_sheet(&cube, &far, 1e-7).is_err());
    }

    #[test]
    fn partial_overlap_union_makes_l_solid() {
        // A = 2x2x1 base block (z in [0,1]); B = 1x2x1 block stacked on its
        // LEFT half (x in [0,1], z in [1,2]). The shared z=1 plane is a
        // PARTIAL coincidence: A's top [0,2]x[0,2] vs B's bottom [0,1]x[0,2].
        // The pre-imprint pass cuts A's top along x=1 so the left half
        // classifies on-/interior (dropped) and the right half stays a face.
        // Result: an L-shaped solid, volume 4 + 2 = 6.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 2.0, 2.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(0.0, 0.0, 1.0), 1.0, 2.0, 1.0).unwrap();
        // CONTRACT (research file 47, self-consistency gate): the
        // identity-preserving stitch is now the ONLY assembly (the
        // polygon soup is retired); the honest contract is
        // correct-or-decline,
        // NEVER a wrong-positive: if Ok, the L-solid must be the true 6 with
        // mass == mesh; if the seam can't yet assemble, Err. Making this
        // union actually build (mass == mesh == 6) is the dossier-47
        // identity-preserving stitch milestone (tasks #16 / #20).
        // If the seam can't yet assemble, the boolean declines (Err) and the
        // contract is satisfied vacuously; otherwise the body must be correct.
        let res = boolean(&a, &b, BoolOp::Union, 1e-7)
            .expect("L-union must now ASSEMBLE (boundary-coincident seam filter)");
        assert!(
            res.body.validate().is_ok(),
            "L-solid union invalid: {:?}",
            res.faults
        );
        let v = res.body.mass_properties().unwrap().volume;
        let mv = res.body.mesh_volume();
        assert!(
            (v - 6.0).abs() < 1e-9 && (v - mv).abs() < 1e-6,
            "L-solid union must be the true 6 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn coincident_identical_union_keeps_one_copy() {
        // Two identical unit boxes: every face pair is same-sense (on+).
        // Union keeps ONE copy of each -> a single box (volume 1, not 2).
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let res = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "identical union invalid: {:?}",
            res.faults
        );
        let v = res.body.mass_properties().unwrap().volume;
        assert!(
            (v - 1.0).abs() < 1e-9,
            "identical-box union volume {v} != 1"
        );
    }

    #[test]
    fn coincident_difference_abutting_keeps_wall() {
        // A=[0,1]^3, B abuts OUTSIDE at x=1. A - B removes nothing -> A
        // unchanged (volume 1). Exercises the on- difference rule: the
        // shared opposite-sense face survives on A.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let res = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "coincident diff invalid: {:?}",
            res.faults
        );
        let v = res.body.mass_properties().unwrap().volume;
        assert!(
            (v - 1.0).abs() < 1e-9,
            "coincident difference volume {v} != 1"
        );
    }

    #[test]
    fn boolean_multi_empty_single_and_two_tools() {
        // Empty tool list returns the target unchanged.
        let a = z_sphere(Vec3::ZERO, 1.0);
        let r0 = boolean_multi(&a, &[], BoolOp::Intersection, 1e-7).unwrap();
        assert!((r0.body.tessellated_volume() - a.tessellated_volume()).abs() < 1e-6);

        // A single tool equals the direct boolean.
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let r1 = boolean_multi(&a, &[&b], BoolOp::Intersection, 1e-7).unwrap();
        let direct = boolean(&a, &b, BoolOp::Intersection, 1e-7).unwrap();
        assert!((r1.body.tessellated_volume() - direct.body.tessellated_volume()).abs() < 1e-6);

        // Two tools: the proven sphere-difference config (unit spheres
        // at distance 1.5) applied at both poles -- a sphere with two
        // independent dimples. Spheres avoid the coplanar-face
        // coincidence two same-depth cylinder floors would trigger.
        let core = z_sphere(Vec3::ZERO, 1.0);
        let v_core = core.tessellated_volume();
        let s1 = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let s2 = z_sphere(Vec3::new(0.0, 0.0, -1.5), 1.0);
        let res = boolean_multi(&core, &[&s1, &s2], BoolOp::Difference, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "two-tool result invalid: {:?}",
            res.faults
        );
        let v = res.body.tessellated_volume();
        assert!(
            v.is_finite() && v < v_core && v > 0.0,
            "two-tool volume {v} vs core {v_core}"
        );
    }

    #[test]
    fn imprint_only_shares_intersection_edge() {
        // Imprint-only: the intersection circle splits each sphere's
        // single face into two, leaving both bodies valid and separate.
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let (ia, ib) = imprint(&a, &b, 1e-7).unwrap();
        assert_eq!(ia.counts().f, 2, "operand a not imprinted");
        assert_eq!(ib.counts().f, 2, "operand b not imprinted");
        assert!(ia.validate().is_ok() && ib.validate().is_ok());
    }

    #[test]
    fn sphere_sphere_union_and_difference() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let h = 0.25;
        let v_sphere = 4.0 / 3.0 * core::f64::consts::PI;
        let v_lens = 2.0 * (core::f64::consts::PI * h * h / 3.0) * (3.0 - h);
        // Union: two outer caps glued -> the peanut. V = 2*V_sphere - V_lens.
        let uni = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
        assert!(
            uni.body.validate().is_ok(),
            "union invalid: {:?}",
            uni.faults
        );
        let vu = uni.body.tessellated_volume();
        let exp_u = 2.0 * v_sphere - v_lens;
        assert!(
            (vu - exp_u).abs() < 0.05 * exp_u,
            "union vol {vu} vs {exp_u}"
        );
        // Difference A-B: sphere A with a spherical dimple. V = V_sphere - V_lens.
        let diff = boolean(&a, &b, BoolOp::Difference, 1e-7).unwrap();
        assert!(
            diff.body.validate().is_ok(),
            "difference invalid: {:?}",
            diff.faults
        );
        let vd = diff.body.tessellated_volume();
        let exp_d = v_sphere - v_lens;
        assert!(
            (vd - exp_d).abs() < 0.05 * exp_d,
            "difference vol {vd} vs {exp_d}"
        );
    }

    #[test]
    fn sphere_caps_classify_via_winding() {
        // Two unit spheres at z=0 and z=1.5 (equatorial seam, crossing-
        // free SSI circle at z=0.75). After imprint each sphere is two
        // caps. Each sphere has exactly one cap inside the other and one
        // outside, classified by the generalized winding number.
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let (ia, ib, faults) = imprint_pair(&a, &b, 1e-7);
        assert!(faults.is_empty(), "faults: {faults:?}");
        let ca = classify_faces(&ia.body, &b, 1e-7);
        let inside = ca
            .iter()
            .filter(|(_, c)| *c == FaceClass::InsideOther)
            .count();
        let outside = ca
            .iter()
            .filter(|(_, c)| *c == FaceClass::OutsideOther)
            .count();
        let bad = ca
            .iter()
            .filter(|(_, c)| matches!(c, FaceClass::Unknown | FaceClass::OnOther(_)))
            .count();
        assert_eq!(bad, 0, "sphere A caps unclassified: {ca:?}");
        assert_eq!((inside, outside), (1, 1), "sphere A cap classes {ca:?}");
        // Symmetric for B.
        let cb = classify_faces(&ib.body, &a, 1e-7);
        let inb = cb
            .iter()
            .filter(|(_, c)| *c == FaceClass::InsideOther)
            .count();
        assert_eq!(inb, 1, "sphere B cap inside A {cb:?}");
    }

    #[test]
    fn imprint_pair_splits_both_spheres() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let (ia, ib, faults) = imprint_pair(&a, &b, 1e-7);
        assert!(faults.is_empty(), "faults: {faults:?}");
        // Each sphere face (1) splits into a cap disc + the rest (2).
        assert!(ia.body.validate().is_ok());
        assert!(ib.body.validate().is_ok());
        assert_eq!(ia.body.counts().f, 2, "sphere A faces after imprint");
        assert_eq!(ib.body.counts().f, 2, "sphere B faces after imprint");
        assert_eq!(ia.seam_edges.len(), 1);
        assert_eq!(ib.seam_edges.len(), 1);
        // The seam edge is the imprinted circle, manifold (radial 2)
        // within its own operand body.
        let ra = ia.body.edge(ia.seam_edges[0]).map(|e| e.radial.len());
        assert_eq!(ra, Some(2));
    }

    #[test]
    fn disjoint_spheres_have_no_seam() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 5.0), 1.0);
        let (seams, faults) = seam_curves(&a, &b, 1e-9);
        assert!(seams.is_empty());
        assert!(faults.is_empty());
    }

    // ---- dossier 73b: compound-operand T-junction imprint -----------------

    /// Build the seed-715 topology class: a box with a rectangular BOSS on its
    /// +x face (a prior Union). The boss is offset from every box edge, so the
    /// box +x face carries an inner-loop HOLE and the boss top/bottom faces are
    /// interior features whose attach edges end MID the box +x face -- the
    /// configuration that T-junctions a later tool wall's seams.
    fn box_with_boss() -> Body {
        let base = block(Vec3::ZERO, Vec3::new(10., 10., 10.));
        let boss = block(Vec3::new(10., 3., 3.), Vec3::new(5., 4., 4.));
        let r = boolean(&base, &boss, BoolOp::Union, 1e-7).expect("boss union");
        assert!(r.faults.is_empty() && r.body.validate().is_ok());
        // exact: 1000 + 5*4*4
        let v = r.body.mass_properties().unwrap().volume;
        assert!((v - 1080.0).abs() < 1e-7, "boss body volume {v}");
        r.body
    }

    #[test]
    fn compound_operand_difference_assembles_at_exact_oracle() {
        // Dossier 73b: a Difference tool whose wall is first split by the
        // boss-attach plane (x=10) then crossed by the boss top/bottom seams
        // exercises the imprint open-chain T-junction repair + the straight
        // subdivision-alignment in the glue. The result is ALL-PLANAR, so its
        // mass and tessellated mesh must agree EXACTLY at the closed-form
        // oracle (no chordal slack). Before dossier 73b this declined with
        // `unmatched coedge: shell-closure invariant violated`.
        let body = box_with_boss();
        // tool [7,14] x [2,11] x [3,7], a Difference. Removes:
        //   base  slab x[7,10] y[2,10] z[3,7] = 3*8*4 = 96
        //   boss      x[10,14] y[3,7] z[3,7]  = 4*4*4 = 64
        // result = 1080 - 160 = 920.
        let tool = block(Vec3::new(7., 2., 3.), Vec3::new(7., 9., 4.));
        let r = boolean(&body, &tool, BoolOp::Difference, 1e-7)
            .expect("compound difference must assemble (dossier 73b)");
        assert!(r.faults.is_empty(), "unexpected faults: {:?}", r.faults);
        assert!(r.body.validate().is_ok(), "result not valid");
        let mass = r.body.mass_properties().unwrap().volume;
        let mesh = r.body.mesh_volume();
        assert!(
            (mass - 920.0).abs() < 1e-6,
            "mass {mass} != exact oracle 920"
        );
        assert!(
            (mass - mesh).abs() < 1e-6,
            "all-planar mass {mass} must equal mesh {mesh} exactly (watertight)"
        );
    }

    #[test]
    fn compound_operand_battery_is_decline_never_wrong() {
        // The floor for the whole compound-operand difference class: over a
        // battery of tool placements against the box+boss body, EVERY result
        // is either a clean Err (declined) OR a watertight body whose
        // (all-planar, exact) mass equals its mesh. A wrong-positive -- a body
        // that assembles with mass != mesh, or with faults, or that fails
        // validate -- must NEVER occur. This guards the dossier-73b imprint
        // changes against ever shipping a non-watertight / wrong body.
        let body = box_with_boss();
        let mut assembled = 0;
        for &(ox, oy, oz, dx, dy, dz) in &[
            (7., 2., 3., 7., 9., 4.),
            (8., 2., 2., 5., 9., 6.),
            (9., 2., 2., 4., 9., 6.),
            (7., 2., 2., 6., 9., 6.),
            (6., 4., 4., 6., 3., 2.),
            (8., 3., 3., 4., 4., 4.),
            (7., 5., 4., 5., 4., 2.),
            (9., 3., 5., 5., 4., 3.),
            (6., 2., 3., 8., 9., 4.),
            (8., 4., 4., 6., 4., 3.),
        ] {
            let tool = block(Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz));
            if let Ok(r) = boolean(&body, &tool, BoolOp::Difference, 1e-7) {
                if !r.faults.is_empty() {
                    continue; // a faulted partial result is a decline here
                }
                assert!(
                    r.body.validate().is_ok(),
                    "assembled body must be valid (tool o=({ox},{oy},{oz}))"
                );
                let mass = r.body.mass_properties().unwrap().volume;
                let mesh = r.body.mesh_volume();
                // all-planar: mass and mesh are both exact, so any gap is a
                // WRONG body (dropped/mis-stitched face). This is the sacred
                // DECLINE-never-WRONG assertion.
                assert!(
                    (mass - mesh).abs() <= 1e-6 * (1.0 + mass.abs()),
                    "WRONG: watertight-claimed body mass {mass} != mesh {mesh} \
                     (tool o=({ox},{oy},{oz}) d=({dx},{dy},{dz}))"
                );
                assembled += 1;
            }
        }
        assert!(
            assembled >= 1,
            "battery should assemble at least one compound difference"
        );
    }

    // ---- dossier 73c: seam-crosses-hole arrangement split -----------------

    #[test]
    fn seam_crosses_hole_assembles_at_exact_oracle() {
        // Dossier 73c: a Difference tool whose wall, on the box +x plane (which
        // carries the boss-footprint inner-loop HOLE), runs from the outer
        // boundary THROUGH the hole's void interior and back out -- the seam
        // crosses the hole. The plain boundary-to-boundary split routed the whole
        // hole to one side and stranded the hole-boundary fragments (the seed-715
        // `unmatched coedge` residual). The face-with-hole re-knit splits the
        // hole at the two crossings and bridges it into the outer loop, so both
        // result sub-faces carry the correct portion of the hole boundary. The
        // result is ALL-PLANAR: mass and mesh must agree EXACTLY at the oracle.
        let body = box_with_boss();
        // box [0,10]^3, boss x[10,15] y[3,7] z[3,7]; +x-face hole is y[3,7] z[3,7].
        // Tool [7,14] x [5,12] x [5,12] clips the hole corner (7,7): its cut on
        // x=10 enters the hole +y edge (y=7,z=5), dips to (5,5) inside the hole,
        // and exits the hole +z edge (y=5,z=7). Removes:
        //   base slab x[7,10] y[5,10] z[5,10] = 3*5*5 = 75
        //   boss      x[10,14] y[5,7] z[5,7]  = 4*2*2 = 16
        // result = 1080 - 91 = 989.
        let tool = block(Vec3::new(7., 5., 5.), Vec3::new(7., 7., 7.));
        let r = boolean(&body, &tool, BoolOp::Difference, 1e-7)
            .expect("seam-crosses-hole difference must assemble (dossier 73c)");
        assert!(r.faults.is_empty(), "unexpected faults: {:?}", r.faults);
        assert!(r.body.validate().is_ok(), "result not valid");
        let mass = r.body.mass_properties().unwrap().volume;
        let mesh = r.body.mesh_volume();
        assert!(
            (mass - 989.0).abs() < 1e-6,
            "mass {mass} != exact oracle 989"
        );
        assert!(
            (mass - mesh).abs() < 1e-6,
            "all-planar mass {mass} must equal mesh {mesh} exactly (watertight)"
        );

        // A second hole-crossing placement (a U through the hole in z): enters the
        // hole +y edge, both interior chain vertices inside the hole, exits +y
        // again. Removes base x[7,10] y[4,10] z[4,6] = 3*6*2 = 36 and boss
        // x[10,14] y[4,7] z[4,6] = 4*3*2 = 24; result 1080 - 60 = 1020.
        let tool2 = block(Vec3::new(7., 4., 4.), Vec3::new(7., 9., 2.));
        let r2 = boolean(&body, &tool2, BoolOp::Difference, 1e-7)
            .expect("second seam-crosses-hole difference must assemble");
        assert!(
            r2.faults.is_empty(),
            "unexpected faults (U): {:?}",
            r2.faults
        );
        assert!(r2.body.validate().is_ok(), "result (U) not valid");
        let mass2 = r2.body.mass_properties().unwrap().volume;
        let mesh2 = r2.body.mesh_volume();
        assert!((mass2 - 1020.0).abs() < 1e-6, "mass {mass2} != oracle 1020");
        assert!(
            (mass2 - mesh2).abs() < 1e-6,
            "all-planar mass {mass2} must equal mesh {mesh2} (U case)"
        );
    }

    #[test]
    fn seam_crosses_hole_battery_is_decline_never_wrong() {
        // The DECLINE-never-WRONG floor for the seam-crosses-hole class: a battery
        // of tools that variously cross, graze, or miss the +x-face hole. EVERY
        // result is either a clean decline (Err, or a faulted partial), or a
        // watertight all-planar body whose mass equals its mesh EXACTLY. The
        // arrangement re-knit must NEVER ship a body with mass != mesh, faults,
        // or an invalid topology. Guards the dossier-73c imprint change.
        let body = box_with_boss();
        let mut assembled = 0;
        for &(ox, oy, oz, dx, dy, dz) in &[
            (7., 5., 5., 7., 7., 7.), // corner-clip (assembles, 989)
            (7., 4., 4., 7., 9., 2.), // U through hole in z (assembles, 1020)
            (7., 4., 2., 7., 2., 6.), // span-z 4-crossing (harder: declines)
            (7., 2., 2., 6., 6., 6.), // big corner overlap
            (7., 6., 6., 6., 6., 6.), // far corner clip
            (8., 4., 4., 6., 3., 3.), // small dip into hole
            (7., 3., 3., 5., 4., 4.), // exactly the hole footprint
            (6., 5., 1., 7., 1., 8.), // thin slab across hole in z
            (7., 1., 5., 7., 8., 1.), // thin slab across hole in y
            (9., 4., 4., 5., 3., 3.), // dip from the boss-attach plane
        ] {
            let tool = block(Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz));
            if let Ok(r) = boolean(&body, &tool, BoolOp::Difference, 1e-7) {
                if !r.faults.is_empty() {
                    continue; // faulted partial -> a decline here
                }
                assert!(
                    r.body.validate().is_ok(),
                    "assembled body must be valid (tool o=({ox},{oy},{oz}))"
                );
                let mass = r.body.mass_properties().unwrap().volume;
                let mesh = r.body.mesh_volume();
                assert!(
                    (mass - mesh).abs() <= 1e-6 * (1.0 + mass.abs()),
                    "WRONG: watertight-claimed body mass {mass} != mesh {mesh} \
                     (tool o=({ox},{oy},{oz}) d=({dx},{dy},{dz}))"
                );
                assembled += 1;
            }
        }
        assert!(
            assembled >= 2,
            "battery should assemble at least the two clean hole-crossing cases"
        );
    }

    // ---- dossier 76: unified per-face planar overlay ----------------------

    /// Exact volume of the axis-aligned-box intersection a INTER b (empty -> 0).
    fn box_inter_vol(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> f64 {
        let lo = Vec3::new(a.0.x.max(b.0.x), a.0.y.max(b.0.y), a.0.z.max(b.0.z));
        let hi = Vec3::new(a.1.x.min(b.1.x), a.1.y.min(b.1.y), a.1.z.min(b.1.z));
        let d = hi - lo;
        if d.x <= 0.0 || d.y <= 0.0 || d.z <= 0.0 {
            0.0
        } else {
            d.x * d.y * d.z
        }
    }

    #[test]
    fn overlay_material_vertex_plus_hole_dip_assembles_at_exact_oracle() {
        // Dossier 76: the seed-715 STRUCTURE -- a chain on a compound face that
        // carries a MATERIAL interior vertex (a sibling-seam T-junction) AND dips
        // through the boss-footprint HOLE, the case dossier 73c could NOT resolve
        // (its single-dip routine excluded a material interior vertex). The
        // unified planar overlay spurs the material vertex, mekr-bridges the hole
        // dip, then split_faces. Here the body's +x-face hole is COUNTER-wound
        // (the conventional B-rep winding a clean Union produces), the certified
        // case. The seed-715 body+tool reproduced exactly; ALL-PLANAR, so mass ==
        // mesh and both equal the exact box-CSG closed form.
        let x0 = 9.551779708383604;
        let y0 = 11.665942029090884;
        let z0 = 15.281438357132698;
        let base_box = (Vec3::new(-x0, -y0, -z0), Vec3::new(x0, y0, z0));
        let boss_box = (
            Vec3::new(x0, -7.088981022063896, -9.647871036389681),
            Vec3::new(23.805674267424912, 6.118691584333003, 3.1315919809062702),
        );
        let base = block(base_box.0, base_box.1 - base_box.0);
        let boss = block(boss_box.0, boss_box.1 - boss_box.0);
        let body = boolean(&base, &boss, BoolOp::Union, 1e-7)
            .expect("boss union")
            .body;
        // The +x face (x = x0) carries the boss-footprint hole; the body is the
        // two-boolean compound seed-715 reaches. Tool: the seed-715 failing
        // Difference box, whose +x-face cut is the material-vertex + hole-dip U.
        let tool_box = (
            Vec3::new(0.8416865823442778, -3.4814833444455537, -9.166169799785951),
            Vec3::new(15.50577871691214, 27.197447619469333, 8.974436478810794),
        );
        let tool = block(tool_box.0, tool_box.1 - tool_box.0);

        // Exact oracle: vol(body) - vol(body INTER tool), body = base UNION boss.
        // vol(body INTER tool) = vol(base INTER tool) + vol(boss INTER tool)
        //   - vol(base INTER boss INTER tool); base INTER boss is empty (they
        //   meet only on the x=x0 plane), so the last term is 0.
        let v_body =
            (base_box.1 - base_box.0).x * (base_box.1 - base_box.0).y * (base_box.1 - base_box.0).z
                + (boss_box.1 - boss_box.0).x
                    * (boss_box.1 - boss_box.0).y
                    * (boss_box.1 - boss_box.0).z;
        let v_cut = box_inter_vol(base_box, tool_box) + box_inter_vol(boss_box, tool_box);
        let oracle = v_body - v_cut;

        let r = boolean(&body, &tool, BoolOp::Difference, 1e-7)
            .expect("seed-715-structure difference must assemble (dossier 76)");
        assert!(r.faults.is_empty(), "unexpected faults: {:?}", r.faults);
        assert!(r.body.validate().is_ok(), "result not valid");
        let mass = r.body.mass_properties().unwrap().volume;
        let mesh = r.body.mesh_volume();
        assert!(
            (mass - oracle).abs() <= 1e-6 * (1.0 + oracle),
            "mass {mass} != exact box-CSG oracle {oracle}"
        );
        assert!(
            (mass - mesh).abs() <= 1e-6 * (1.0 + mass),
            "all-planar mass {mass} must equal mesh {mesh} exactly (watertight)"
        );

        // The Intersection direction of the same pair (the seed-715 booleanI)
        // must also assemble exactly: vol = vol(body INTER tool).
        let oracle_i = v_cut;
        let ri = boolean(&body, &tool, BoolOp::Intersection, 1e-7)
            .expect("seed-715-structure intersection must assemble");
        assert!(
            ri.faults.is_empty(),
            "unexpected faults (I): {:?}",
            ri.faults
        );
        assert!(ri.body.validate().is_ok(), "result (I) not valid");
        let mass_i = ri.body.mass_properties().unwrap().volume;
        let mesh_i = ri.body.mesh_volume();
        assert!(
            (mass_i - oracle_i).abs() <= 1e-6 * (1.0 + oracle_i),
            "intersection mass {mass_i} != exact oracle {oracle_i}"
        );
        assert!(
            (mass_i - mesh_i).abs() <= 1e-6 * (1.0 + mass_i),
            "all-planar intersection mass {mass_i} must equal mesh {mesh_i}"
        );
    }

    /// The signed area of `face`'s first inner (hole) loop in the face plane,
    /// minus the sign of its outer loop: returns (cowound, inner_loop_key) where
    /// `cowound` is true iff the hole is stored with the SAME winding sense as
    /// the outer loop. None if the face has no inner loop. Test helper for the
    /// co-wound rung.
    fn first_hole_winding(body: &Body, face: FaceKey) -> Option<(bool, crate::entity::LoopKey)> {
        let Some(Surface3::Plane(pl)) = body.face_surface3(face) else {
            return None;
        };
        let (o, ex, ey) = (pl.frame.origin, pl.frame.x, pl.frame.y);
        let to2 = |q: Vec3| ((q - o).dot(ex), (q - o).dot(ey));
        let sa = |lk: crate::entity::LoopKey| -> f64 {
            let poly: Vec<(f64, f64)> = body.loop_polygon(lk).iter().map(|&q| to2(q)).collect();
            let n = poly.len();
            (0..n)
                .map(|k| {
                    let a = poly[k];
                    let b = poly[(k + 1) % n];
                    a.0 * b.1 - b.0 * a.1
                })
                .sum::<f64>()
                * 0.5
        };
        let loops = body.faces.get(face).map(|f| f.loops.clone())?;
        let outer = *loops.first()?;
        let inner = *loops.get(1)?;
        Some((sa(inner).signum() == sa(outer).signum(), inner))
    }

    /// The single PLANAR face of `body` that carries an inner hole loop (on the
    /// box+boss bodies this is the boss-attach +x face; there is exactly one).
    fn plus_x_holed_face(body: &Body) -> Option<FaceKey> {
        body.face_keys().into_iter().find(|&f| {
            let has_hole = body
                .faces
                .get(f)
                .map(|x| x.loops.len() > 1)
                .unwrap_or(false);
            let planar = matches!(body.face_surface3(f), Some(Surface3::Plane(_)));
            has_hole && planar
        })
    }

    #[test]
    fn overlay_cowound_hole_dip_assembles_at_exact_oracle() {
        // Dossier 76 sec 4 (the co-wound rung, RESOLVED): the SAME seed-715
        // structure (material vertex + hole dip) but with the +x-face hole stored
        // CO-WOUND (same winding sense as the outer loop), which is the storage
        // seed 715's own `extrude`-Union compound actually produces and which the
        // prior overlay DECLINED. Here we force the co-wound storage by reversing
        // the hole loop of the clean-Union body, then assert the overlay detects
        // the co-wound hole, canonicalizes it (reverse_inner_loop), and assembles
        // BOTH boolean directions at the EXACT same box-CSG oracle as the
        // counter-wound case -- mass == mesh, all-planar, no slack.
        let x0 = 9.551779708383604;
        let y0 = 11.665942029090884;
        let z0 = 15.281438357132698;
        let base_box = (Vec3::new(-x0, -y0, -z0), Vec3::new(x0, y0, z0));
        let boss_box = (
            Vec3::new(x0, -7.088981022063896, -9.647871036389681),
            Vec3::new(23.805674267424912, 6.118691584333003, 3.1315919809062702),
        );
        let base = block(base_box.0, base_box.1 - base_box.0);
        let boss = block(boss_box.0, boss_box.1 - boss_box.0);
        let mut body = boolean(&base, &boss, BoolOp::Union, 1e-7)
            .expect("boss union")
            .body;

        // Force the +x-face hole CO-wound (the seed-715 storage).
        let face = plus_x_holed_face(&body).expect("+x holed face");
        let (was_cowound, hole) = first_hole_winding(&body, face).expect("hole winding");
        if !was_cowound {
            body.reverse_inner_loop(hole)
                .expect("reverse hole to co-wound");
        }
        let (now_cowound, _) = first_hole_winding(&body, face).expect("hole winding 2");
        assert!(
            now_cowound,
            "test setup: +x-face hole must be CO-wound to exercise the rung"
        );
        assert!(body.validate().is_ok(), "co-wound body must still validate");

        let tool_box = (
            Vec3::new(0.8416865823442778, -3.4814833444455537, -9.166169799785951),
            Vec3::new(15.50577871691214, 27.197447619469333, 8.974436478810794),
        );
        let tool = block(tool_box.0, tool_box.1 - tool_box.0);

        let v_body =
            (base_box.1 - base_box.0).x * (base_box.1 - base_box.0).y * (base_box.1 - base_box.0).z
                + (boss_box.1 - boss_box.0).x
                    * (boss_box.1 - boss_box.0).y
                    * (boss_box.1 - boss_box.0).z;
        let v_cut = box_inter_vol(base_box, tool_box) + box_inter_vol(boss_box, tool_box);
        let oracle_d = v_body - v_cut;

        let r = boolean(&body, &tool, BoolOp::Difference, 1e-7)
            .expect("co-wound seed-715-structure difference must assemble (dossier 76 sec 4)");
        assert!(r.faults.is_empty(), "unexpected faults: {:?}", r.faults);
        assert!(r.body.validate().is_ok(), "result not valid");
        let mass = r.body.mass_properties().unwrap().volume;
        let mesh = r.body.mesh_volume();
        assert!(
            (mass - oracle_d).abs() <= 1e-6 * (1.0 + oracle_d),
            "co-wound difference mass {mass} != exact box-CSG oracle {oracle_d}"
        );
        assert!(
            (mass - mesh).abs() <= 1e-6 * (1.0 + mass),
            "all-planar mass {mass} must equal mesh {mesh} exactly (watertight)"
        );

        let oracle_i = v_cut;
        let ri = boolean(&body, &tool, BoolOp::Intersection, 1e-7)
            .expect("co-wound seed-715-structure intersection must assemble");
        assert!(
            ri.faults.is_empty(),
            "unexpected faults (I): {:?}",
            ri.faults
        );
        assert!(ri.body.validate().is_ok(), "result (I) not valid");
        let mass_i = ri.body.mass_properties().unwrap().volume;
        let mesh_i = ri.body.mesh_volume();
        assert!(
            (mass_i - oracle_i).abs() <= 1e-6 * (1.0 + oracle_i),
            "co-wound intersection mass {mass_i} != exact oracle {oracle_i}"
        );
        assert!(
            (mass_i - mesh_i).abs() <= 1e-6 * (1.0 + mass_i),
            "all-planar intersection mass {mass_i} must equal mesh {mesh_i}"
        );
    }

    #[test]
    fn overlay_battery_is_decline_never_wrong() {
        // The DECLINE-never-WRONG floor for the unified overlay: a battery of
        // tools against the box+boss body that produce material-vertex and/or
        // hole-dip chains in BOTH boolean directions. EVERY assembled result is a
        // watertight all-planar body whose mass equals its mesh EXACTLY; anything
        // the overlay cannot certify (e.g. a multi-dip chain) is a clean decline.
        // The overlay must NEVER ship a body with mass != mesh, faults, or
        // invalid topology, regardless of hole winding (co- or counter-wound).
        // Both hole windings: the conventional counter-wound body AND the same
        // body with its +x-face hole reversed to CO-wound (the seed-715 storage,
        // now canonicalized by the overlay). The floor must hold for both.
        let counter = box_with_boss();
        let cowound = {
            let mut b = box_with_boss();
            if let Some(face) = plus_x_holed_face(&b)
                && let Some((cw, hole)) = first_hole_winding(&b, face)
                && !cw
            {
                b.reverse_inner_loop(hole).expect("force co-wound");
            }
            b
        };
        assert!(cowound.validate().is_ok(), "co-wound battery body valid");
        let mut assembled = 0;
        for body in [&counter, &cowound] {
            for &(ox, oy, oz, dx, dy, dz) in &[
                (7., 1., 5., 7., 6., 7.), // U: outer edge -> material -> hole dip
                (7., 5., 1., 7., 7., 6.), // U dipping the hole from the other side
                (6., 2., 4., 8., 6., 4.), // material corner + hole dip
                (7., 4., 4., 7., 9., 2.), // U through hole in z (the 73c case)
                (7., 5., 5., 7., 7., 7.), // corner-clip (73c)
                (8., 1., 1., 5., 8., 8.), // large overlap with material corners
                (6., 6., 1., 8., 1., 9.), // thin slab grazing the hole
            ] {
                for op in [BoolOp::Difference, BoolOp::Intersection] {
                    let tool = block(Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz));
                    if let Ok(r) = boolean(body, &tool, op, 1e-7) {
                        if !r.faults.is_empty() {
                            continue; // faulted partial -> a decline here
                        }
                        assert!(
                            r.body.validate().is_ok(),
                            "assembled body must be valid ({op:?} o=({ox},{oy},{oz}))"
                        );
                        let mass = r.body.mass_properties().unwrap().volume;
                        let mesh = r.body.mesh_volume();
                        assert!(
                            (mass - mesh).abs() <= 1e-6 * (1.0 + mass.abs()),
                            "WRONG: watertight-claimed body mass {mass} != mesh {mesh} \
                             ({op:?} o=({ox},{oy},{oz}) d=({dx},{dy},{dz}))"
                        );
                        assembled += 1;
                    }
                }
            }
        }
        assert!(
            assembled >= 1,
            "overlay battery should assemble at least one compound case"
        );
    }

    #[test]
    fn blind_cut_interior_terminus_is_decline_never_wrong() {
        // CHARACTERIZATION GUARD (#76 multi-seam-crossing census, this round).
        //
        // The dominant PLANAR open-chain imprint decline in the 10k soak is NOT a
        // multi-seam crossing (those are curved-cylinder-wrap, the SSI frontier,
        // out of scope) but a BLIND CUT: a tool whose intersection with a body
        // face is a seam that starts on the face boundary and ENDS in the face
        // interior, with no sibling seam meeting it there (measured: the interior
        // terminus sits a MACROSCOPIC distance, 1 to 12 units, from any boundary
        // vertex, and coincides with no other member seam -- a true blind
        // terminus, not a junction and not tolerance starvation). Such a seam
        // cannot be resolved by a boundary-to-boundary `split_face`: there is no
        // second boundary point to cut to, and forcing a spur to the interior
        // point would leave a dangling radial-1 edge -- exactly the malformed
        // body the dual mass==mesh gate exists to reject. So this class is an
        // IRREDUCIBLE DECLINE: recovering it would violate DECLINE-never-WRONG.
        //
        // This test pins that floor. A tool box is positioned so that, after the
        // body is cut, its walls land partially across the body's faces (partial
        // overlaps that strand a seam mid-face). EVERY result must be either a
        // clean decline (Err, or a faulted/partial result) OR a watertight
        // all-planar body whose mass equals its mesh EXACTLY. A wrong-positive --
        // a body that claims to assemble with mass != mesh -- must NEVER occur.
        // A COMPOUND body (box + boss): the boss makes the box +x face holed and
        // the boss faces small, so a tool wall that ends partway across a boss
        // face strands a seam mid-face (the blind cut). A single convex box does
        // NOT reproduce this -- every box/box seam lands boundary-to-boundary --
        // which is itself part of the characterization (the blind cut is a
        // COMPOUND-operand phenomenon, matching the soak population).
        let body = box_with_boss(); // base [0,10]^3, boss x[10,15] y[3,7] z[3,7]
        let mut seen_decline = false;
        let mut seen_assembled = false;
        // Tools positioned to land a wall partway across a boss face (a partial
        // overlap that strands a seam mid-face), plus clean through-cuts.
        for &(ox, oy, oz, dx, dy, dz) in &[
            (12., 4., 5., 1.5, 2., 8.), // thin wall inside the boss top span
            (11., 4., 4., 2., 2., 2.),  // small cube buried in the boss corner
            (12., 2., 5., 2., 3.5, 8.), // wall ending mid-boss in y
            (11., 4., 2., 8., 2., 3.5), // wall ending mid-boss in z
            (13., 3., 3., 1.5, 4., 4.), // partial slab across boss far end
            (8., 4., 4., 4., 2., 2.),   // straddles base/boss attach, ends in boss
            (7., 2., 3., 7., 9., 4.),   // clean through-cut (assembles, oracle 920)
        ] {
            let tool = block(Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz));
            for op in [BoolOp::Difference, BoolOp::Intersection, BoolOp::Union] {
                match boolean(&body, &tool, op, 1e-7) {
                    Err(_) => seen_decline = true,
                    Ok(r) => {
                        if !r.faults.is_empty() {
                            seen_decline = true;
                            continue;
                        }
                        // A clean Ok must be a valid, watertight, exact body.
                        assert!(
                            r.body.validate().is_ok(),
                            "blind-cut assembled body must be valid \
                             ({op:?} o=({ox},{oy},{oz}) d=({dx},{dy},{dz}))"
                        );
                        let mass = r.body.mass_properties().unwrap().volume;
                        let mesh = r.body.mesh_volume();
                        // all-planar: any mass!=mesh gap is a WRONG (dropped or
                        // mis-stitched face) -- the sacred floor assertion.
                        assert!(
                            (mass - mesh).abs() <= 1e-6 * (1.0 + mass.abs()),
                            "WRONG: blind-cut body claims watertight but mass {mass} \
                             != mesh {mesh} ({op:?} o=({ox},{oy},{oz}) d=({dx},{dy},{dz}))"
                        );
                        seen_assembled = true;
                    }
                }
            }
        }
        // The battery must exercise BOTH outcomes: at least one clean watertight
        // assembly (the partial cuts that ARE resolvable, e.g. the oracle-920
        // through-cut) AND at least one decline (the true blind cut, e.g. the
        // base/boss-straddling tool ending inside the boss, which declines
        // `mass != mesh` / `unmatched coedge`). The load-bearing assertion is the
        // per-result mass==mesh check above: a blind cut is allowed to decline,
        // but a body that CLAIMS to assemble must be exactly watertight. This is
        // the sacred DECLINE-never-WRONG floor for the blind-cut class.
        assert!(
            seen_decline,
            "battery must exercise the blind-cut decline path"
        );
        assert!(seen_assembled, "battery must exercise a clean assembly too");
    }
}
