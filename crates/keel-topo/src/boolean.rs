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

/// The three regularized boolean operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolOp {
    Union,
    Intersection,
    /// `a` minus `b`.
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
}

impl Body {
    /// Live face keys in deterministic EntityId order.
    pub(crate) fn face_keys(&self) -> Vec<FaceKey> {
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

    /// True unless `face` is a cylindrical lateral face and the closed
    /// `curve`'s axial height falls OUTSIDE the face's actual height band
    /// (the surface-surface SSI uses the unbounded cylinder, so it can
    /// produce a circle outside the trimmed lateral). Non-cylinder faces
    /// and non-planar curves are unconstrained here.
    pub(crate) fn curve_on_cylinder_face(
        &self,
        face: FaceKey,
        curve: &keel_geom::curve::Curve3,
        tol: f64,
    ) -> bool {
        use keel_geom::curve::Curve3;
        let Some(Surface3::Cylinder(c)) = self.face_surface3(face) else {
            return true;
        };
        let (origin, ez) = (c.frame.origin, c.frame.z);
        let curve_h = match curve {
            Curve3::Circle(ci) => (ci.center - origin).dot(ez),
            Curve3::Ellipse(e) => (e.center - origin).dot(ez),
            _ => return true,
        };
        // Band from the face's circle/arc edges.
        let heights = self.cyl_circle_heights(face, origin, ez);
        if heights.len() < 2 {
            return true; // cannot determine a band; do not reject
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let t = tol.max(1e-7);
        curve_h >= hlo - t && curve_h <= hhi + t
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
        let n = match self.surfaces.get(sk)? {
            SurfaceGeom::Analytic(s) => {
                let pr = s.project(p).ok()?;
                s.local_geometry(pr.u, pr.v).ok()?.normal
            }
            SurfaceGeom::Nurbs(nb) => {
                let pr = keel_geom::project::project_point_surface_fast(nb, p);
                nb.local_geometry(pr.u, pr.v).ok()?.normal
            }
        };
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
    ) -> OnSense {
        for f in self.face_keys() {
            // The face's surface must pass through p AND its normal be
            // parallel to n_other (a coincident carrier, not a transversal
            // face that merely contains p).
            let on_surface = match self.face_surface_geom(f) {
                Some(SurfaceGeom::Analytic(s)) => {
                    s.project(p).map(|pr| pr.distance < 1e-6).unwrap_or(false)
                }
                Some(SurfaceGeom::Nurbs(nb)) => {
                    let pr = keel_geom::project::project_point_surface_fast(&nb, p);
                    (pr.point - p).norm() < 1e-6
                }
                None => false,
            };
            if !on_surface {
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
        if denom.abs() < 1e-300 {
            if num < 0.0 {
                return None; // parallel and outside this edge
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
    /// Corner points of a face's outer-loop polygon. Uses the loop's
    /// vertices when there are 3+; for a loop that is a single closed
    /// edge (the rectangle ring from imprint_closed_curve) it falls back
    /// to that edge's degree-1 NURBS control points.
    pub(crate) fn face_ring_points(&self, face: FaceKey) -> Vec<keel_math::vec::Vec3> {
        let pts = self.face_outer_loop_points(face);
        if pts.len() >= 3 {
            return pts;
        }
        // Single closed-edge loop: read corners from the curve.
        let Some(lp) = self.faces.get(face).and_then(|f| f.loops.first().copied()) else {
            return pts;
        };
        let Some(fin) = self.loops.get(lp).and_then(|l| l.fin) else {
            return pts;
        };
        let Some((ck, _)) = self
            .fins
            .get(fin)
            .map(|f| f.edge)
            .and_then(|e| self.edges.get(e))
            .and_then(|e| e.curve)
        else {
            return pts;
        };
        if let Some(keel_geom::curve::Curve3::Nurbs(n)) = self.curves.get(ck) {
            let mut corners: Vec<keel_math::vec::Vec3> = Vec::new();
            for h in n.homogeneous_control() {
                if h.w.abs() < 1e-300 {
                    continue;
                }
                let p = keel_math::vec::Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w);
                if corners
                    .last()
                    .map(|q| (*q - p).norm() > 1e-9)
                    .unwrap_or(true)
                {
                    corners.push(p);
                }
            }
            // Drop the closing duplicate of the first corner.
            if let Some(&last) = corners.last()
                && corners.len() >= 2
                && (corners[0] - last).norm() <= 1e-9
            {
                corners.pop();
            }
            return corners;
        }
        pts
    }

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
        let (origin, ex, ez, r) = (cyl.frame.origin, cyl.frame.x, cyl.frame.z, cyl.radius);
        let heights = self.cyl_circle_heights(face, origin, ez);
        if heights.len() < 2 {
            return None;
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let hmid = 0.5 * (hlo + hhi);
        // Angle pi (opposite the seam at angle 0): origin - ex*r + ez*hmid.
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
                // Only the CLOSED SSI seam circle bounds a cap; skip the
                // sphere's open pole-to-pole meridian.
                let closed = self.edges.get(fin.edge).map(|e| e.is_closed()) == Some(true);
                if closed
                    && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                    && let Some(cv) = self.curves.get(ck)
                    && let Some((_center_c, ax)) = closed_curve_center_axis(cv)
                    && let Some((m, t)) = closed_curve_point_tangent(cv, 0.25)
                {
                    let n = (m - center).try_normalize()?; // sphere outward at m
                    // Into-face = surface-tangent to the LEFT of the
                    // traversal for an outer loop; flipped for an inner
                    // ring (whose interior is on the other side).
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
        None
    }

    pub(crate) fn face_interior_point(&self, face: FaceKey) -> Option<keel_math::vec::Vec3> {
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
                if let Some(samples) = self.fin_curve_samples(cur, 16) {
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
            out.push((face, FaceClass::Unknown));
            continue;
        }
        let class = match working.face_interior_point(face) {
            Some(p) => {
                let w = other.generalized_winding_number(p);
                if (w - 0.5).abs() < COINCIDENCE_BAND {
                    // On the other body's boundary: resolve the
                    // coincident-pair orientation (file 39 §1.4/§2.1) by
                    // the sign of the two faces' outward normals.
                    let sense = match working.face_outward_normal(face) {
                        Some(n) => other.coincident_sense_at(p, n),
                        None => OnSense::Unknown,
                    };
                    FaceClass::OnOther(sense)
                } else if w > 0.5 {
                    FaceClass::InsideOther
                } else {
                    FaceClass::OutsideOther
                }
            }
            None => FaceClass::Unknown,
        };
        out.push((face, class));
    }
    out
}

/// A kept face reduced to geometry for stitching: its outer-loop ring
/// (3D points, in any order) and the outward normal the result needs.
struct ResultPoly {
    ring: Vec<keel_math::vec::Vec3>,
    outward: keel_math::vec::Vec3,
    surface: SurfaceGeom,
}

/// Extract each kept face as an oriented polygon. Faces with inner
/// loops (holes) are deferred (M6a proof cases produce simple faces).
fn kept_to_polys(
    ia: &Body,
    ib: &Body,
    kept: &[KeptFace],
    faults: &mut Vec<BoolFault>,
) -> Vec<ResultPoly> {
    let mut out = Vec::new();
    for k in kept {
        let body = match k.operand {
            Operand::A => ia,
            Operand::B => ib,
        };
        let Some(face) = body.faces.get(k.face) else {
            continue;
        };
        if face.loops.len() != 1 {
            faults.push(BoolFault::AssemblyFailed("kept face has holes (M6b)"));
            continue;
        }
        let ring = body.face_ring_points(k.face);
        if ring.len() < 3 {
            faults.push(BoolFault::AssemblyFailed("kept face ring too small"));
            continue;
        }
        // Operand outward normal: front faces outside, so the operand's
        // outward is sense ? frame.z : -frame.z. Reversal flips it.
        let Some((sk, opsense)) = face.surface else {
            continue;
        };
        let Some(sg) = body.surfaces.get(sk).cloned() else {
            continue;
        };
        let nz = match &sg {
            SurfaceGeom::Analytic(Surface3::Plane(p)) => p.frame.z,
            _ => {
                faults.push(BoolFault::AssemblyFailed("non-planar kept face (M6b)"));
                continue;
            }
        };
        let mut outward = if opsense { nz } else { nz * -1.0 };
        if k.reversed {
            outward = outward * -1.0;
        }
        out.push(ResultPoly {
            ring,
            outward,
            surface: sg,
        });
    }
    out
}

/// Newell normal of a 3D polygon ring.
fn newell_normal(ring: &[keel_math::vec::Vec3]) -> keel_math::vec::Vec3 {
    use keel_math::vec::Vec3;
    let mut n = Vec3::ZERO;
    let m = ring.len();
    for i in 0..m {
        let a = ring[i];
        let b = ring[(i + 1) % m];
        n = n + Vec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    n
}

/// Stitch oriented polygons into one valid manifold solid: dedup
/// vertices, share edges, build fins/loops/faces, and a two-region
/// partition (solid interior + the infinite void) with its two shells.
/// Built directly in the arenas (the Euler operators forbid the
/// intermediate non-solid states a soup passes through), then validated.
fn build_result_solid(polys: &[ResultPoly], tol: f64) -> Result<Body, BoolFault> {
    use crate::entity::{EdgeKey, LoopKind, Side, VertexKey};
    use crate::lineage::Derivation;
    use keel_math::vec::Vec3;
    use std::collections::BTreeMap;

    let vtol = tol.max(1e-7);
    let mut b = Body::new();
    let inf = b.infinite_region();
    let mut rec = b.begin_op();
    let solid = b.new_region(&mut rec, true, Derivation::Created);

    // Global vertex dedup.
    let mut vpts: Vec<Vec3> = Vec::new();
    let mut vkeys: Vec<VertexKey> = Vec::new();
    let mut vindex = |p: Vec3, b: &mut Body, rec: &mut crate::body::OpRecorder| -> usize {
        for (i, q) in vpts.iter().enumerate() {
            if (*q - p).norm() <= vtol {
                return i;
            }
        }
        let vk = b.new_vertex(rec, p);
        vpts.push(p);
        vkeys.push(vk);
        vpts.len() - 1
    };

    // Orient each ring CCW about its outward normal, map to vertex ids.
    let mut faces_vi: Vec<Vec<usize>> = Vec::new();
    for poly in polys {
        let mut ring = poly.ring.clone();
        if newell_normal(&ring).dot(poly.outward) < 0.0 {
            ring.reverse();
        }
        let vis: Vec<usize> = ring
            .into_iter()
            .map(|p| vindex(p, &mut b, &mut rec))
            .collect();
        faces_vi.push(vis);
    }

    // Edge sharing: undirected (min,max) vertex id -> edge + bounds.
    let mut edge_of: BTreeMap<(usize, usize), (EdgeKey, (usize, usize))> = BTreeMap::new();
    let mut get_edge = |vi: usize,
                        vj: usize,
                        b: &mut Body,
                        rec: &mut crate::body::OpRecorder|
     -> (EdgeKey, bool) {
        let key = (vi.min(vj), vi.max(vj));
        if let Some(&(ek, (a, _))) = edge_of.get(&key) {
            return (ek, a == vi); // forward iff bounds.0 == vi
        }
        let ek = b.new_edge(rec, (vkeys[vi], vkeys[vj]), Derivation::Created);
        edge_of.insert(key, (ek, (vi, vj)));
        (ek, true)
    };

    let mut face_keys = Vec::new();
    for (fi, vis) in faces_vi.iter().enumerate() {
        let face = b.new_face(&mut rec, inf, solid, Derivation::Created);
        let lp = b.new_loop(&mut rec, face, LoopKind::Outer, Derivation::Created);
        if let Some(f) = b.faces.get_mut(face) {
            f.loops = vec![lp];
        }
        let n = vis.len();
        let mut fins = Vec::with_capacity(n);
        for k in 0..n {
            let (vi, vj) = (vis[k], vis[(k + 1) % n]);
            let (ek, forward) = get_edge(vi, vj, &mut b, &mut rec);
            let fin = b.new_fin(&mut rec, ek, forward, lp, Derivation::Created);
            if let Some(e) = b.edges.get_mut(ek) {
                e.radial.push(fin);
            }
            fins.push(fin);
        }
        for k in 0..n {
            let (nx, pv) = (fins[(k + 1) % n], fins[(k + n - 1) % n]);
            if let Some(f) = b.fins.get_mut(fins[k]) {
                f.next = nx;
                f.prev = pv;
            }
        }
        if let Some(l) = b.loops.get_mut(lp) {
            l.fin = Some(fins[0]);
        }
        for &fin in &fins {
            if let Some(sv) = b.fin_start_vertex(fin)
                && let Some(v) = b.vertices.get_mut(sv)
                && v.fin.is_none()
            {
                v.fin = Some(fin);
            }
        }
        // Surface: attach so the front normal points outward.
        let nz = match &polys[fi].surface {
            SurfaceGeom::Analytic(Surface3::Plane(p)) => p.frame.z,
            _ => Vec3::new(0., 0., 1.),
        };
        let sense = nz.dot(polys[fi].outward) > 0.0;
        b.attach_face_surface(face, polys[fi].surface.clone(), sense);
        face_keys.push(face);
    }

    // Shells: the solid region owns every (face, Back); the infinite
    // region owns every (face, Front). (Front normal points outward.)
    let solid_shell = b.new_shell(&mut rec, solid, Derivation::Created);
    let inf_shell = b.new_shell(&mut rec, inf, Derivation::Created);
    if let Some(s) = b.shells.get_mut(solid_shell) {
        s.faces = face_keys.iter().map(|&f| (f, Side::Back)).collect();
    }
    if let Some(s) = b.shells.get_mut(inf_shell) {
        s.faces = face_keys.iter().map(|&f| (f, Side::Front)).collect();
    }
    if let Some(r) = b.regions.get_mut(solid) {
        r.shells.push(solid_shell);
    }
    if let Some(r) = b.regions.get_mut(inf) {
        r.shells.push(inf_shell);
    }
    let _ = rec.finish();

    // Planar pcurves so trimmed-face mass properties can integrate.
    for &face in &face_keys {
        let _ = b.attach_plane_pcurves(face);
    }

    match b.validate() {
        Ok(()) => Ok(b),
        Err(_) => Err(BoolFault::AssemblyFailed("stitched body invalid")),
    }
}

/// The result of a boolean: the body, accumulated faults, and the
/// operation tag.
#[derive(Clone, Debug)]
pub struct BoolResult {
    pub body: Body,
    pub faults: Vec<BoolFault>,
    pub op: BoolOp,
}

/// Regularized boolean of two solids (M6a: clean transversal cases).
/// Pipeline: imprint both operands along the SSI seams, classify each
/// fragment against the other solid, select per the r-set tables, and
/// stitch the kept faces into a new solid. Partial-success: recoverable
/// faults ride along in `BoolResult::faults`; an unbuildable result is
/// `Err`.
/// Import one entity-subtree (vertices, edges, fins, loops, the face,
/// and its geometry) from `src` into `dst`, deduping by source EntityId
/// within the same operand. `reversed` flips the face's orientation
/// (fin senses, loop traversal order, surface sense) for difference.
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
    let (b0, b1, curve, etol) = {
        let se = src.edges.get(edge)?;
        (se.bounds.0, se.bounds.1, se.curve, se.tolerance)
    };
    let dv0 = import_vertex(dst, src, b0, op, rec, vmap)?;
    let dv1 = import_vertex(dst, src, b1, op, rec, vmap)?;
    let de = dst.new_edge(rec, (dv0, dv1), Derivation::Created);
    if let Some(e) = dst.edges.get_mut(de) {
        e.tolerance = e.tolerance.max(etol);
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
    let ia = imprint_operand(sheet, &seams, |s| s.face_a, tol, &mut faults);
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
    let ia = imprint_operand(solid, &seams, |s| s.face_a, tol, &mut faults);
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
        let target = dst.edges.iter().find_map(|(k, e)| {
            if k == j {
                return None;
            }
            if e.bounds == bj {
                Some((k, false))
            } else if e.bounds == (bj.1, bj.0) {
                Some((k, true))
            } else {
                None
            }
        });
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

/// The shared back half of import-and-glue assembly (used by the boolean
/// stitch AND by `knit`): merge coincident vertices, glue coincident free /
/// dangling edges into radial pairs, assert the planar shell-closure
/// invariant, partition into solid / void / infinite regions (enclosed-void
/// aware), then validate. Takes the body with its kept faces already
/// imported (front -> `inf`, back -> `solid`) and the live op recorder.
pub(crate) fn finalize_imported_assembly(
    mut dst: Body,
    mut rec: crate::body::OpRecorder,
    faces: Vec<FaceKey>,
    walls: Vec<FaceKey>,
    inf: crate::entity::RegionKey,
    solid: crate::entity::RegionKey,
    vtol: f64,
) -> Result<Body, BoolFault> {
    use crate::entity::Side;
    use crate::lineage::Derivation;
    merge_and_glue_imported(&mut dst, &mut rec, vtol);

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
                face.back_region = solid;
                face.front_region = front_region;
            }
        }
        let back_shell = dst.new_shell(&mut rec, solid, Derivation::Created);
        if let Some(s) = dst.shells.get_mut(back_shell) {
            s.faces = comp.iter().map(|&f| (f, Side::Back)).collect();
            s.genus = comp_genus;
        }
        if let Some(r) = dst.regions.get_mut(solid) {
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
    let _ = rec.finish();

    match dst.validate() {
        Ok(()) => Ok(dst),
        Err(_) => Err(BoolFault::AssemblyFailed("stitched (curved) body invalid")),
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

/// Partition `faces` (kept fragments of the stitched body) into connected
/// boundary components: faces sharing an edge are in the same component.
/// Each component is one closed boundary shell (e.g. the outer box surface
/// and the inner void surface of a hollow body are two separate components).
fn connected_face_components(dst: &Body, faces: &[FaceKey]) -> Vec<Vec<FaceKey>> {
    use std::collections::BTreeMap;
    let idx: BTreeMap<FaceKey, usize> = faces.iter().enumerate().map(|(i, &f)| (f, i)).collect();
    let mut parent: Vec<usize> = (0..faces.len()).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        if p[i] != i {
            let r = find(p, p[i]);
            p[i] = r;
        }
        p[i]
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
fn coincident_face_pairs(a: &Body, b: &Body) -> Vec<(FaceKey, FaceKey, keel_math::vec::Vec3)> {
    let is_planar = |body: &Body, f: FaceKey| {
        matches!(
            body.face_surface_geom(f),
            Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
        )
    };
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
        for fb in b.face_keys() {
            if !is_planar(b, fb) {
                continue;
            }
            let nb = match b.face_outward_normal(fb) {
                Some(n) => n,
                None => continue,
            };
            let pb = match b.face_outer_loop_points(fb).first().copied() {
                Some(p) => p,
                None => continue,
            };
            // Same plane: parallel normals and a-point lying in b's plane.
            if na.cross(nb).norm() < 1e-7 && (pb - pa).dot(na).abs() < 1e-7 {
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
    let pairs = coincident_face_pairs(a, b);
    if pairs.is_empty() {
        return None;
    }
    let mut a = a.clone();
    let mut b = b.clone();
    let imprint_cuts = |body: &mut Body,
                        face: FaceKey,
                        subj: &[keel_math::vec::Vec3],
                        other: &[keel_math::vec::Vec3],
                        n: keel_math::vec::Vec3| {
        for (s, e) in crate::coincident::overlap_interior_segments(subj, other, n) {
            if let Ok(line) = keel_geom::curve::Line3::new(s, e - s) {
                let _ = body.imprint_open_curve(face, &keel_geom::curve::Curve3::Line(line), tol);
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
pub struct BooleanOptions {
    pub regularize: bool,
}

impl Default for BooleanOptions {
    fn default() -> Self {
        Self { regularize: true }
    }
}

pub fn boolean(a: &Body, b: &Body, op: BoolOp, tol: f64) -> Result<BoolResult, BoolFault> {
    boolean_with(a, b, op, tol, BooleanOptions::default())
}

pub fn boolean_with(
    a: &Body,
    b: &Body,
    op: BoolOp,
    tol: f64,
    opts: BooleanOptions,
) -> Result<BoolResult, BoolFault> {
    // Pre-pass (research file 39 §1): where two coplanar faces partially
    // overlap, imprint the overlap-boundary cuts onto the operands so each
    // resulting fragment is uniformly inside/outside/on the other body --
    // the on-on tables in select_faces then classify them correctly. With
    // no coincident faces this is a no-op and the originals flow through.
    let pre = preimprint_coincident_overlaps(a, b, tol);
    let (a, b): (&Body, &Body) = match &pre {
        Some((pa, pb)) => (pa, pb),
        None => (a, b),
    };
    let (seams, faults) = seam_curves(a, b, tol);
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
    assemble_boolean(a, b, op, tol, &seams, faults, opts)
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
    let ia = imprint_operand(a, seams, |s| s.face_a, tol, &mut faults);
    let ib = imprint_operand(b, seams, |s| s.face_b, tol, &mut faults);
    let class_a = classify_faces(&ia.body, b, tol);
    let class_b = classify_faces(&ib.body, a, tol);
    let kept = select_faces(op, &class_a, &class_b);
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
    // PRIMARY assembly: the identity-preserving import-and-glue (research
    // file 47). It imports each kept fragment carrying its operand's edge
    // identity and glues only the genuinely-coincident cross-operand seam;
    // with the matching seam subdivision (subdivide_seam_ring) the planar
    // seam coedges now pair 1:1, so it correctly assembles the thin/oblique
    // cuts the old polygon-soup stitcher mis-built (the asymmetric-chamfer
    // class). FALLBACK: partial-coincidence unions whose coincident seam
    // the identity glue does not yet assemble (e.g. the L-union) still use
    // the legacy soup builder -- a tracked file-47 follow-on, not a wrong
    // answer (the final volume post-condition guards both paths).
    let body = match stitch_by_import(&ia, &ib, &kept, &walls, tol) {
        Ok(b) => b,
        Err(e) if walls.is_empty() => {
            let _ = e;
            let polys = kept_to_polys(&ia.body, &ib.body, &kept, &mut faults);
            build_result_solid(&polys, tol)?
        }
        // The cellular (non-regularized) result has no soup fallback:
        // the soup cannot hold a radial-3 cycle (dossier 57 Rung 0).
        Err(e) => return Err(e),
    };
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
        let v = body.tessellated_volume();
        v.is_finite() && v > 1e-9 * (1.0 + v.abs())
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
    if !ok {
        return Err(BoolFault::AssemblyFailed(
            "degenerate or self-inconsistent result (mass != mesh)",
        ));
    }
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
    let ia = imprint_operand(a, &seams, |s| s.face_a, tol, &mut faults);
    let ib = imprint_operand(b, &seams, |s| s.face_b, tol, &mut faults);
    Ok((ia.body, ib.body))
}

/// Endpoints of a seam curve (sample at the parameter ends; closed
/// curves return the seam point twice).
fn curve_point(c: &keel_geom::curve::Curve3, t: f64) -> keel_math::vec::Vec3 {
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

/// Distance from point `p` to the segment `a`-`b` in 3D.
fn seg_dist3(p: keel_math::vec::Vec3, a: keel_math::vec::Vec3, b: keel_math::vec::Vec3) -> f64 {
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
        // Endpoints must be on the boundary.
        if self
            .loop_fin_ending_at_point(face, chain[0], etol)
            .is_none()
            || self
                .loop_fin_ending_at_point(face, chain[last], etol)
                .is_none()
        {
            return Err(TopoError::Precondition("open chain end not on boundary"));
        }
        let mut edges = Vec::new();
        let mut tip = chain[0];
        // Build spurs through the interior vertices.
        for &c in &chain[1..last] {
            let fin = self
                .loop_fin_ending_at_point(face, tip, etol)
                .ok_or(TopoError::Precondition("spur start vertex lost"))?;
            let m = self.mev(MevSite::AfterFin(fin), c)?;
            if let Some(curve) = seg_curve(tip, c) {
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
        let out = self.split_face(fa, fb, None)?;
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
    fn attach_seam_geometry(
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
    pick: impl Fn(&SeamCurve) -> FaceKey,
    tol: f64,
    faults: &mut Vec<BoolFault>,
) -> ImprintedOperand {
    use keel_math::vec::Vec3;
    use std::collections::BTreeMap;
    let mut working = body.clone();
    let mut seam_edges = Vec::new();
    let etol = tol.max(1e-7);

    // Group seam indices by their face on this operand.
    let mut groups: BTreeMap<u64, (FaceKey, Vec<usize>)> = BTreeMap::new();
    for (i, s) in seams.iter().enumerate() {
        let face = pick(s);
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
    for (face, members) in groups.values_mut() {
        let bnd = working.face_outer_loop_points(*face);
        if bnd.len() < 3 {
            continue;
        }
        // Scope to the spurious-RING case only. If these seams assemble into
        // a CLOSED loop, boundary-coincident sides make it a phantom inner
        // ring that punches a hole instead of splitting the face (L-union).
        // Open chains / lone cuts (chamfer) must be left intact -- dropping
        // their boundary-touching parts would un-trim a kept tool face.
        let eps: Vec<(Vec3, Vec3)> = members
            .iter()
            .map(|&i| curve_endpoints(&seams[i].curve))
            .collect();
        if assemble_closed_loop(&eps, tol).is_none() {
            continue;
        }
        let on_boundary = |p: Vec3| -> bool {
            let m = bnd.len();
            (0..m)
                .map(|i| seg_dist3(p, bnd[i], bnd[(i + 1) % m]))
                .fold(f64::INFINITY, f64::min)
                <= etol
        };
        let is_on_boundary_seg = |i: usize| {
            [0.0, 0.25, 0.5, 0.75, 1.0]
                .iter()
                .all(|&t| on_boundary(curve_point(&seams[i].curve, t)))
        };
        // A genuine interior ring (a hole, e.g. the chamfer cutter face's
        // footprint) has >=2 interior segments. The spurious case is a loop
        // that closes only BECAUSE boundary-coincident segments complete it
        // around a single real interior cut (<=1 interior segment): then the
        // "ring" is really that one chord SPLITTING the face, and imprinting
        // it as a closed hole punches a phantom inner loop (the L-union: 3 of
        // 4 sides on A-top's boundary). Drop the boundary segments only then.
        let interior = members.iter().filter(|&&i| !is_on_boundary_seg(i)).count();
        if interior <= 1 {
            members.retain(|&i| !is_on_boundary_seg(i));
        }
    }

    // Phase 1: pre-split boundary edges at unique OPEN-seam endpoints
    // (corners of open chains). Closed curves are skipped: their two
    // "endpoints" are the same degenerate point, which for a cylinder
    // SSI circle is exactly the seam crossing -- pre-splitting there
    // would defeat the crossing imprint.
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
            let _ = working.split_edge(edge, p);
        }
    }

    // Phase 2: imprint per face.
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
                match planar_face_containing(&working, probe, etol) {
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
                let res = if working.closed_curve_crosses_boundary(target, curve, tol) {
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
                match working.imprint_closed_curve(
                    target,
                    &keel_geom::curve::Curve3::Nurbs(ring),
                    tol,
                ) {
                    Ok(rep) => {
                        // Match the OTHER operand's per-face open-edge seam
                        // subdivision (file 47): split this closed ring at its
                        // corners so the seam coedges can pair at stitch time.
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
        let ia = imprint_operand(self, &seams, |s| s.face_a, tol, &mut faults);
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
    let ia = imprint_operand(a, &seams, |s| s.face_a, tol, &mut faults);
    let ib = imprint_operand(b, &seams, |s| s.face_b, tol, &mut faults);
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
            // Cylinder-cylinder SSI curves exist (the certified
            // evaluator rung), but the IMPRINT cannot yet assemble two
            // closed seams crossing each other and the periodic seam on
            // one lateral face: proceeding produced an Euler-valid but
            // geometrically WRONG body (the Steinmetz probe read 12.5
            // against the exact 16/3). DECLINE-never-WRONG: these
            // configurations keep declining at the boolean layer until
            // the crossing-seam imprint lands; the geometry rung serves
            // direct SSI consumers and the unequal-radius mitre.
            let both_cyl = matches!(ref_a, SurfaceRef::Analytic(Surface3::Cylinder(_)))
                && matches!(ref_b, SurfaceRef::Analytic(Surface3::Cylinder(_)));
            match intersect_surfaces(&ref_a, &ref_b, tol) {
                Ok(SsiResult::Curves(cs)) if both_cyl => {
                    if cs
                        .iter()
                        .any(|c| a.curve_on_cylinder_face(fa, &c.curve, tol))
                    {
                        faults.push(BoolFault::UnassemblableSeam(id_a, id_b));
                    }
                }
                Ok(SsiResult::Curves(cs)) => {
                    for c in cs {
                        if c.tangential {
                            faults.push(BoolFault::Tangent(id_a, id_b));
                            continue;
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
                            let ia = clip_line_to_planar_face(line, pa, &pts_a);
                            let ib = clip_line_to_planar_face(line, pb, &pts_b);
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
                                        });
                                    }
                                }
                            }
                            continue;
                        }
                        // Surface-surface SSI uses the UNBOUNDED surfaces;
                        // reject a closed curve that does not lie on both
                        // faces' trimmed extents (an infinite cylinder
                        // meets a plane outside the actual cylinder face).
                        if !a.curve_on_cylinder_face(fa, &c.curve, tol)
                            || !b.curve_on_cylinder_face(fb, &c.curve, tol)
                        {
                            continue;
                        }
                        seams.push(SeamCurve {
                            face_a: fa,
                            face_b: fb,
                            curve: c.curve,
                            closed: c.closed,
                            tol: c.tol_achieved,
                        });
                    }
                }
                Ok(SsiResult::Coincident) => faults.push(BoolFault::Coincident(id_a, id_b)),
                Ok(SsiResult::Points(_)) => faults.push(BoolFault::Tangent(id_a, id_b)),
                Ok(SsiResult::Empty) => {}
                Err(_) => faults.push(BoolFault::IntersectionFailed(id_a, id_b)),
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
        // Tall thin box touched by a small box at a near-coincident face
        // (fuzz finding): the planar mass-properties post-condition must
        // reject the zero/sliver-volume result rather than return it.
        let a = block(Vec3::ZERO, Vec3::new(0.5, 0.5, 20.0));
        let b = block(Vec3::new(0.0, 0.0, 20.0), Vec3::new(0.5, 0.5, 0.5));
        if let Ok(res) = boolean(&a, &b, BoolOp::Intersection, 1e-7) {
            // If anything comes back, mass properties must be computable
            // and positive (never a wrong "valid" body).
            let v = res.body.mass_properties().map(|m| m.volume);
            assert!(
                matches!(v, Ok(vol) if vol.is_finite() && vol > 0.0),
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
        // crossing) now has CERTIFIED SSI curves (the cylinder-cylinder
        // evaluator rung), but the imprint cannot yet assemble two
        // closed seams crossing each other and the periodic seam on one
        // lateral face. Proceeding SEAMLESS used to return an Euler-
        // valid body with volume 12.5 against the exact 16/3: a wrong
        // positive that predates the rung. DECLINE-never-WRONG: the
        // pair is a hard UnassemblableSeam fault until the crossing-
        // seam imprint lands.
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
        for op in [BoolOp::Intersection, BoolOp::Union, BoolOp::Difference] {
            assert!(
                matches!(
                    boolean(&a, &b, op, 1e-5),
                    Err(BoolFault::UnassemblableSeam(..))
                ),
                "crossing cylinders must DECLINE ({op:?})"
            );
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
        // CONTRACT (research file 47, self-consistency gate): the coincident
        // (partial-overlap) union seam is not yet assembled identity-clean,
        // so build_result_solid drops a face -> a body whose sense-exact mass
        // (6) disagrees with its tessellated mesh (5.333). The gate now
        // catches that disagreement and DECLINES rather than return the
        // malformed body. So the honest contract is correct-or-decline,
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
}
