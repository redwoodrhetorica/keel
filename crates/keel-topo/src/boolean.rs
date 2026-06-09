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
pub(crate) fn classify_faces(working: &Body, other: &Body, _tol: f64) -> Vec<(FaceKey, FaceClass)> {
    // Generalized winding number is the PRIMARY classifier (the
    // d-booleans-tolerant.md mandate): robust at on-boundary/tangential
    // contacts and surface-type-agnostic (no pcurve/periodicity
    // dependency), where ray-cast PMC was fragile. w ~ 1 inside other,
    // ~ 0 outside; the band around 0.5 means the sample sits on/near
    // other's boundary (coincident -> M6c).
    const COINCIDENCE_BAND: f64 = 0.25;
    let mut out = Vec::new();
    for face in working.face_keys() {
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
    tol: f64,
) -> Result<Body, BoolFault> {
    use crate::entity::{EdgeKey, Side, VertexKey};
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
                    dst.unregister(&mut rec, id);
                }
                dst.vertices.remove(vkeys[j]);
                alive[j] = false;
            }
        }
    }

    // Glue coincident seam edges: edges that are radial-1 (dangling
    // after import) and share bounds (post-merge) pair up.
    let dangling: Vec<EdgeKey> = dst
        .edges
        .iter()
        .filter(|(_, e)| e.radial.len() == 1)
        .map(|(k, _)| k)
        .collect();
    let mut used = vec![false; dangling.len()];
    for i in 0..dangling.len() {
        if used[i] {
            continue;
        }
        let (bi, _) = match dst.edges.get(dangling[i]) {
            Some(e) => (e.bounds, ()),
            None => continue,
        };
        for j in (i + 1)..dangling.len() {
            if used[j] {
                continue;
            }
            let bj = match dst.edges.get(dangling[j]) {
                Some(e) => e.bounds,
                None => continue,
            };
            let aligned = bi == bj;
            let reversed = bi == (bj.1, bj.0);
            if aligned || reversed {
                // Move edge j's fin onto edge i; drop edge j.
                let moved: Vec<_> = dst
                    .edges
                    .get(dangling[j])
                    .map(|e| e.radial.clone())
                    .unwrap_or_default();
                for fk in &moved {
                    if let Some(f) = dst.fins.get_mut(*fk) {
                        f.edge = dangling[i];
                        if reversed {
                            f.forward = !f.forward;
                        }
                    }
                }
                if let Some(e) = dst.edges.get_mut(dangling[i]) {
                    e.radial.extend(moved);
                }
                if let Some(id) = dst.edges.get(dangling[j]).map(|e| e.id) {
                    dst.unregister(&mut rec, id);
                }
                dst.edges.remove(dangling[j]);
                used[i] = true;
                used[j] = true;
                break;
            }
        }
    }

    // Two-region partition: front (outward) -> infinite, back -> solid.
    let solid_shell = dst.new_shell(&mut rec, solid, Derivation::Created);
    let inf_shell = dst.new_shell(&mut rec, inf, Derivation::Created);
    if let Some(s) = dst.shells.get_mut(solid_shell) {
        s.faces = faces.iter().map(|&f| (f, Side::Back)).collect();
    }
    if let Some(s) = dst.shells.get_mut(inf_shell) {
        s.faces = faces.iter().map(|&f| (f, Side::Front)).collect();
    }
    if let Some(r) = dst.regions.get_mut(solid) {
        r.shells.push(solid_shell);
    }
    if let Some(r) = dst.regions.get_mut(inf) {
        r.shells.push(inf_shell);
    }
    let _ = rec.finish();

    match dst.validate() {
        Ok(()) => Ok(dst),
        Err(_) => Err(BoolFault::AssemblyFailed("stitched (curved) body invalid")),
    }
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

pub fn boolean(a: &Body, b: &Body, op: BoolOp, tol: f64) -> Result<BoolResult, BoolFault> {
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
    let (seams, mut faults) = seam_curves(a, b, tol);
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
        .find(|f| matches!(f, BoolFault::Tangent(..)))
        .cloned()
    {
        return Err(f);
    }
    let ia = imprint_operand(a, &seams, |s| s.face_a, tol, &mut faults);
    let ib = imprint_operand(b, &seams, |s| s.face_b, tol, &mut faults);
    let class_a = classify_faces(&ia.body, b, tol);
    let class_b = classify_faces(&ib.body, a, tol);
    let kept = select_faces(op, &class_a, &class_b);
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
    let body = match stitch_by_import(&ia, &ib, &kept, tol) {
        Ok(b) => b,
        Err(_) => {
            let polys = kept_to_polys(&ia.body, &ib.body, &kept, &mut faults);
            build_result_solid(&polys, tol)?
        }
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
    for (_, (face, members)) in groups {
        let eps: Vec<(Vec3, Vec3)> = members
            .iter()
            .map(|&i| curve_endpoints(&seams[i].curve))
            .collect();
        // A single, already-closed curve. A sphere/planar SSI circle is
        // interior to its face (ring imprint); a cylinder SSI circle
        // wraps the lateral face and crosses its seam line (crossing
        // imprint).
        if members.len() == 1 && seams[members[0]].closed {
            let curve = &seams[members[0]].curve;
            let res = if working.closed_curve_crosses_boundary(face, curve, tol) {
                working.imprint_closed_curve_crossing(face, curve, tol)
            } else {
                working.imprint_closed_curve(face, curve, tol)
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
            match working.imprint_closed_curve(face, &keel_geom::curve::Curve3::Nurbs(ring), tol) {
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
            match working.imprint_open_chain(face, &chain, tol) {
                Ok(es) => seam_edges.extend(es),
                Err(e) => faults.push(BoolFault::Topo(e)),
            }
            continue;
        }
        faults.push(BoolFault::AssemblyFailed("unassembled face seams"));
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
            match intersect_surfaces(&ref_a, &ref_b, tol) {
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
