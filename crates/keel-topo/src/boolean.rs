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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    let mut keep = Vec::new();
    let want = |operand, want_inside: bool, reversed: bool, list: &[(FaceKey, FaceClass)], keep: &mut Vec<KeptFace>| {
        for &(face, c) in list {
            let take = match c {
                FaceClass::InsideOther => want_inside,
                FaceClass::OutsideOther => !want_inside,
                _ => false,
            };
            if take {
                keep.push(KeptFace { operand, face, reversed });
            }
        }
    };
    match op {
        BoolOp::Union => {
            want(Operand::A, false, false, class_a, &mut keep);
            want(Operand::B, false, false, class_b, &mut keep);
        }
        BoolOp::Intersection => {
            want(Operand::A, true, false, class_a, &mut keep);
            want(Operand::B, true, false, class_b, &mut keep);
        }
        BoolOp::Difference => {
            want(Operand::A, false, false, class_a, &mut keep);
            want(Operand::B, true, true, class_b, &mut keep);
        }
    }
    keep
}

/// A face fragment's position relative to the OTHER operand solid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaceClass {
    InsideOther,
    OutsideOther,
    /// On the other operand's boundary (coincident) -> M6b.
    OnOther,
    /// Could not find an interior sample / classify.
    Unknown,
}

impl Body {
    /// Sample a fin's edge 3D curve in the fin's traversal direction
    /// (`m` points, start inclusive). Returns None if the edge has no
    /// curve.
    fn fin_curve_samples(&self, fin: FinKey, m: usize) -> Option<Vec<keel_math::vec::Vec3>> {
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
        let mut out = Vec::with_capacity(m);
        for i in 0..m {
            let s = i as f64 / m as f64;
            out.push(eval(if fwd { s } else { 1.0 - s }));
        }
        Some(out)
    }

    /// A 3D point on `face`'s surface, strictly interior to its trim
    /// loops. Builds each loop's UV polygon by projecting its 3D
    /// vertices onto the surface (exact for planes, no dependency on
    /// stored pcurve completeness), then grid-samples the outer loop's
    /// UV box and winding-tests against every loop. Analytic faces only.
    pub(crate) fn face_interior_point(&self, face: FaceKey) -> Option<keel_math::vec::Vec3> {
        let surf = self.face_surface3(face)?;
        let loops: Vec<crate::entity::LoopKey> =
            self.faces.get(face).map(|f| f.loops.clone()).unwrap_or_default();
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
        let (mut umin, mut umax, mut vmin, mut vmax) =
            (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
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
                if best.is_none() || d > best.unwrap().1 {
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
pub(crate) fn classify_faces(
    working: &Body,
    other: &Body,
    _tol: f64,
) -> Vec<(FaceKey, FaceClass)> {
    use crate::pmc::Containment;
    let mut out = Vec::new();
    for face in working.face_keys() {
        let class = match working.face_interior_point(face) {
            Some(p) => match other.classify_point(p) {
                Ok(Containment::In(_)) => FaceClass::InsideOther,
                Ok(Containment::Out) => FaceClass::OutsideOther,
                Ok(Containment::On(_)) => FaceClass::OnOther,
                Err(_) => FaceClass::Unknown,
            },
            None => FaceClass::Unknown,
        };
        out.push((face, class));
    }
    out
}

/// Endpoints of a seam curve (sample at the parameter ends; closed
/// curves return the seam point twice).
fn curve_endpoints(c: &keel_geom::curve::Curve3) -> (keel_math::vec::Vec3, keel_math::vec::Vec3) {
    use keel_geom::curve::Curve3;
    let s = |t: f64| match c {
        Curve3::Nurbs(n) => {
            let (a, b) = n.domain();
            n.point(a + t * (b - a))
        }
        Curve3::Line(l) => l.point(t),
        Curve3::Circle(ci) => ci.point(core::f64::consts::TAU * t),
        Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
    };
    (s(0.0), s(1.0))
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
            let Some(e) = self.edges.get(ek) else { continue };
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
        let lp = self.faces.get(face).and_then(|f| f.loops.first().copied())?;
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
fn imprint_operand(
    body: &Body,
    seams: &[SeamCurve],
    pick: impl Fn(&SeamCurve) -> FaceKey,
    tol: f64,
    faults: &mut Vec<BoolFault>,
) -> ImprintedOperand {
    use std::collections::BTreeMap;
    use keel_math::vec::Vec3;
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

    // Phase 1: pre-split boundary edges at unique seam endpoints that
    // lie on them.
    let mut corners: Vec<Vec3> = Vec::new();
    for s in seams {
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
    for (_, (face, members)) in groups {
        let eps: Vec<(Vec3, Vec3)> = members
            .iter()
            .map(|&i| curve_endpoints(&seams[i].curve))
            .collect();
        // Are all endpoints now boundary vertices on this face?
        let all_on_boundary = eps.iter().all(|(p0, p1)| {
            working.loop_fin_ending_at_point(face, *p0, etol).is_some()
                && working.loop_fin_ending_at_point(face, *p1, etol).is_some()
        });
        if all_on_boundary {
            if members.len() != 1 {
                // Multi-segment per face (corner overlap) is deferred.
                faults.push(BoolFault::AssemblyFailed("multi-segment open face"));
                continue;
            }
            let (p0, p1) = eps[0];
            let (Some(fa), Some(fb)) = (
                working.loop_fin_ending_at_point(face, p0, etol),
                working.loop_fin_ending_at_point(face, p1, etol),
            ) else {
                faults.push(BoolFault::AssemblyFailed("seam endpoint vertex lost"));
                continue;
            };
            match working.split_face(fa, fb, None) {
                Ok(out) => {
                    // The new fragment inherits the parent's surface.
                    if let Some(surf) = working.faces.get(face).and_then(|f| f.surface)
                        && let Some(nf) = working.faces.get_mut(out.face_new)
                    {
                        nf.surface = Some(surf);
                    }
                    working.attach_seam_geometry(out.edge, face, &seams[members[0]].curve, tol);
                    working.debug_validate();
                    seam_edges.push(out.edge);
                }
                Err(e) => faults.push(BoolFault::Topo(e)),
            }
            continue;
        }
        // Otherwise: a single closed curve, or segments forming one
        // closed loop interior to the face -> a single interior ring.
        if members.len() == 1 && seams[members[0]].closed {
            match working.imprint_closed_curve(face, &seams[members[0]].curve, tol) {
                Ok(rep) => seam_edges.push(rep.edge),
                Err(e) => faults.push(BoolFault::Topo(e)),
            }
            continue;
        }
        if let Some(nodes) = assemble_closed_loop(&eps, tol)
            && let Some(ring) = closed_polyline_nurbs(&nodes)
        {
            match working.imprint_closed_curve(face, &keel_geom::curve::Curve3::Nurbs(ring), tol) {
                Ok(rep) => seam_edges.push(rep.edge),
                Err(e) => faults.push(BoolFault::Topo(e)),
            }
        } else {
            faults.push(BoolFault::AssemblyFailed("unassembled face seams"));
        }
    }
    ImprintedOperand {
        body: working,
        seam_edges,
    }
}

/// Two-body imprint (M3 pipeline steps 1-3, per-operand form): localize
/// + intersect, then imprint the seams onto independent clones of each
/// operand. Each returned body is itself a valid solid (the imprint
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
pub fn seam_curves(
    a: &Body,
    b: &Body,
    tol: f64,
) -> (Vec<SeamCurve>, Vec<BoolFault>) {
    let mut seams = Vec::new();
    let mut faults = Vec::new();
    for fa in a.face_keys() {
        let Some(sa) = a.face_surface3(fa) else {
            continue;
        };
        let id_a = a.faces.get(fa).map(|f| f.id.0).unwrap_or(0);
        for fb in b.face_keys() {
            let Some(sb) = b.face_surface3(fb) else {
                continue;
            };
            let id_b = b.faces.get(fb).map(|f| f.id.0).unwrap_or(0);
            match intersect_surfaces(&SurfaceRef::Analytic(&sa), &SurfaceRef::Analytic(&sb), tol) {
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
                        if let (keel_geom::curve::Curve3::Line(line), Surface3::Plane(pa), Surface3::Plane(pb)) =
                            (&c.curve, &sa, &sb)
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
                                        });
                                    }
                                }
                            }
                            continue;
                        }
                        seams.push(SeamCurve {
                            face_a: fa,
                            face_b: fb,
                            curve: c.curve,
                            closed: c.closed,
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
        assert_eq!(segs.len(), 4, "expected 4 clipped seam segments, got {} (faults {:?})", segs.len(), faults);
        // Every seam segment lies in the plane x = 2 and is length 4.
        for s in &segs {
            let p0 = sample_curve(&s.curve, 0.0);
            let p1 = sample_curve(&s.curve, 1.0);
            assert!((p0.x - 2.0).abs() < 1e-9 && (p1.x - 2.0).abs() < 1e-9, "seam off x=2: {p0:?} {p1:?}");
            assert!((((p1 - p0).norm()) - 4.0).abs() < 1e-9, "seam length {} != 4", (p1 - p0).norm());
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
        let inside_a = ca.iter().filter(|(_, c)| *c == FaceClass::InsideOther).count();
        let outside_a = ca.iter().filter(|(_, c)| *c == FaceClass::OutsideOther).count();
        let bad_a = ca.iter().filter(|(_, c)| matches!(c, FaceClass::Unknown | FaceClass::OnOther)).count();
        assert_eq!(bad_a, 0, "A unclassified/coincident: {ca:?}");
        // 4 cut side-faces -> 4 inside fragments + 4 outside fragments;
        // x=4 face inside, x=0 face outside. => 5 inside, 5 outside.
        assert_eq!((inside_a, outside_a), (5, 5), "A class counts {ca:?}");
        // B's faces vs A: the inner rectangle on x=2 is inside A; its
        // outer remainder and the far faces are outside A.
        let cb = classify_faces(&ib.body, &a, 1e-7);
        let inside_b = cb.iter().filter(|(_, c)| *c == FaceClass::InsideOther).count();
        let bad_b = cb.iter().filter(|(_, c)| matches!(c, FaceClass::Unknown | FaceClass::OnOther)).count();
        assert_eq!(bad_b, 0, "B unclassified/coincident: {cb:?}");
        assert_eq!(inside_b, 1, "only B's inner x=2 rectangle is inside A: {cb:?}");
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
        assert_eq!(diff.iter().filter(|k| k.reversed).count(), 1, "one reversed B face");
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
        assert_eq!(ib.seam_edges.len(), 1);
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
