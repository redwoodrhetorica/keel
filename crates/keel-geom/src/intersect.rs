//! Curve-curve and curve-surface intersection (M4; spec D5/D6/D7).
//!
//! Affordability gradient: analytic-analytic pairs take exact algebraic
//! paths; line-vs-spline uses the two-plane reduction (Nishita-
//! Sederberg-Kakimoto) onto univariate Bernstein root isolation;
//! spline-vs-spline pairs go through the multivariate Projected
//! Polyhedron solver on EXACTLY constructed Bernstein difference
//! systems (homogeneous cross-multiplied, never sampled-and-fit:
//! the Farouki-Rajan discipline), polished by Newton.
//!
//! Tangency is REPORTED (rank-deficient Jacobian at the hit), never
//! resolved: tangent-intersection semantics belong to the boolean
//! classifier (M6). Coincident-overlap pairs return
//! GeomError::Degenerate (the M5/M6 coincidence handlers own them).

use crate::GeomError;
use crate::curve::{Circle3, Curve3, Domain, Ellipse3, Line3};
use crate::nurbs_curve::{BezierSegment, NurbsCurve};
use keel_math::bernstein::Bernstein;
use keel_math::multibernstein::{MultiBernstein, solve_system};
use keel_math::vec::Vec3;

/// An intersection point between two curves.
#[derive(Clone, Debug)]
pub struct CurveHit {
    pub t1: f64,
    pub t2: f64,
    pub point: Vec3,
    pub tangential: bool,
}

/// Angular threshold for the tangency report: tangents whose cross
/// product is below this (relative) are flagged tangential.
const TANGENT_EPS: f64 = 1e-8;
/// PP solver settings for intersection systems.
const PP_TOL: f64 = 1e-10;
const PP_BUDGET: usize = 200_000;

/// Intersect two bounded curves. `tol` is the spatial coincidence
/// tolerance for accepting hits.
pub fn intersect_curves(
    a: &Curve3,
    da: Domain,
    b: &Curve3,
    db: Domain,
    tol: f64,
) -> Result<Vec<CurveHit>, GeomError> {
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    match (a, b) {
        (Curve3::Line(la), Curve3::Line(lb)) => line_line(la, da, lb, db, tol),
        (Curve3::Line(l), Curve3::Circle(c)) => line_conic(l, da, &conic_of_circle(c), db, tol),
        (Curve3::Circle(c), Curve3::Line(l)) => {
            swap_hits(line_conic(l, db, &conic_of_circle(c), da, tol))
        }
        (Curve3::Line(l), Curve3::Ellipse(e)) => line_conic(l, da, &conic_of_ellipse(e), db, tol),
        (Curve3::Ellipse(e), Curve3::Line(l)) => {
            swap_hits(line_conic(l, db, &conic_of_ellipse(e), da, tol))
        }
        (Curve3::Line(l), Curve3::Nurbs(n)) => line_nurbs(l, da, n, tol),
        (Curve3::Nurbs(n), Curve3::Line(l)) => swap_hits(line_nurbs(l, db, n, tol)),
        _ => {
            // General pair: exact NURBS forms, PP per segment pair.
            let (na, ra) = to_nurbs(a)?;
            let (nb, rb) = to_nurbs(b)?;
            let hits = nurbs_nurbs(&na, &nb, tol)?;
            Ok(hits
                .into_iter()
                .map(|mut h| {
                    h.t1 = ra.remap(h.t1);
                    h.t2 = rb.remap(h.t2);
                    h
                })
                .filter(|h| domain_contains(da, h.t1, tol) && domain_contains(db, h.t2, tol))
                .collect())
        }
    }
}

fn swap_hits(r: Result<Vec<CurveHit>, GeomError>) -> Result<Vec<CurveHit>, GeomError> {
    r.map(|hits| {
        hits.into_iter()
            .map(|h| CurveHit {
                t1: h.t2,
                t2: h.t1,
                point: h.point,
                tangential: h.tangential,
            })
            .collect()
    })
}

fn domain_contains(d: Domain, t: f64, _tol: f64) -> bool {
    match d {
        Domain::Finite { a, b } => t >= a - 1e-12 && t <= b + 1e-12,
        Domain::Periodic { .. } => true,
    }
}

// ---------------------------------------------------------------------
// line / line

fn line_line(
    la: &Line3,
    da: Domain,
    lb: &Line3,
    db: Domain,
    tol: f64,
) -> Result<Vec<CurveHit>, GeomError> {
    let cross = la.dir.cross(lb.dir);
    let denom = cross.dot(cross);
    let w = lb.origin - la.origin;
    if denom <= 1e-28 {
        // Parallel. Coincident within tolerance is a degenerate
        // overlap; otherwise no hits.
        let off = w - la.dir * w.dot(la.dir);
        if off.norm() <= tol {
            return Err(GeomError::Degenerate);
        }
        return Ok(Vec::new());
    }
    // Closest-point parameters on the infinite lines.
    let t1 = w.cross(lb.dir).dot(cross) / denom;
    let t2 = w.cross(la.dir).dot(cross) / denom;
    let p1 = la.point(t1);
    let p2 = lb.point(t2);
    if (p1 - p2).norm() > tol {
        return Ok(Vec::new());
    }
    if !(domain_contains(da, t1, tol) && domain_contains(db, t2, tol)) {
        return Ok(Vec::new());
    }
    Ok(vec![CurveHit {
        t1,
        t2,
        point: (p1 + p2) * 0.5,
        tangential: false,
    }])
}

// ---------------------------------------------------------------------
// line / conic (circle and ellipse share one in-plane quadratic path)

struct Conic {
    center: Vec3,
    x: Vec3, // unit
    y: Vec3, // unit
    a: f64,  // semi-axis along x
    b: f64,  // semi-axis along y
}

fn conic_of_circle(c: &Circle3) -> Conic {
    Conic {
        center: c.center,
        x: c.x_axis,
        y: c.y_axis,
        a: c.radius,
        b: c.radius,
    }
}

fn conic_of_ellipse(e: &Ellipse3) -> Conic {
    Conic {
        center: e.center,
        x: e.x_axis,
        y: e.y_axis,
        a: e.a,
        b: e.b,
    }
}

fn line_conic(
    l: &Line3,
    dl: Domain,
    c: &Conic,
    dc: Domain,
    tol: f64,
) -> Result<Vec<CurveHit>, GeomError> {
    let n = c.x.cross(c.y);
    let dn = l.dir.dot(n);
    let w = l.origin - c.center;
    let mut hits = Vec::new();
    if dn.abs() <= 1e-14 {
        // Line parallel to the conic plane.
        if w.dot(n).abs() > tol {
            return Ok(Vec::new());
        }
        // In-plane: substitute the line into the conic implicit form
        // (x/a)^2 + (y/b)^2 = 1 in conic coordinates.
        let (ox, oy) = (w.dot(c.x), w.dot(c.y));
        let (dx, dy) = (l.dir.dot(c.x), l.dir.dot(c.y));
        let qa = (dx / c.a) * (dx / c.a) + (dy / c.b) * (dy / c.b);
        let qb = 2.0 * ((ox * dx) / (c.a * c.a) + (oy * dy) / (c.b * c.b));
        let qc = (ox / c.a) * (ox / c.a) + (oy / c.b) * (oy / c.b) - 1.0;
        let roots = keel_math::poly::solve_quadratic(qa, qb, qc);
        let tangentish = roots.len() == 1
            || (roots.len() == 2 && (roots[0] - roots[1]).abs() < TANGENT_EPS.sqrt());
        for t in roots {
            let p = l.point(t);
            let theta = conic_angle(c, p);
            if domain_contains(dl, t, tol) && domain_contains(dc, theta, tol) {
                hits.push(CurveHit {
                    t1: t,
                    t2: theta,
                    point: p,
                    tangential: tangentish,
                });
            }
        }
    } else {
        // Transversal: single plane crossing; on the conic only if the
        // crossing satisfies the conic equation within tolerance.
        let t = -w.dot(n) / dn;
        let p = l.point(t);
        let wp = p - c.center;
        let (px, py) = (wp.dot(c.x), wp.dot(c.y));
        let val = (px / c.a) * (px / c.a) + (py / c.b) * (py / c.b) - 1.0;
        // Map the implicit residual to a distance scale.
        if val.abs() <= 2.0 * tol / c.a.min(c.b) {
            let theta = conic_angle(c, p);
            if domain_contains(dl, t, tol) && domain_contains(dc, theta, tol) {
                // The line pierces the conic's plane AT the conic: a
                // transversal point hit; tangential only if the line
                // direction is parallel to the conic tangent there.
                let tangent = c.y * (py).signum(); // placeholder magnitude
                let _ = tangent;
                let conic_tan = c.x * (-c.a * theta.sin()) + c.y * (c.b * theta.cos());
                let tangential = l.dir.cross(conic_tan).norm() <= TANGENT_EPS * conic_tan.norm();
                hits.push(CurveHit {
                    t1: t,
                    t2: theta,
                    point: p,
                    tangential,
                });
            }
        }
    }
    Ok(hits)
}

fn conic_angle(c: &Conic, p: Vec3) -> f64 {
    let w = p - c.center;
    (w.dot(c.y) / c.b)
        .atan2(w.dot(c.x) / c.a)
        .rem_euclid(core::f64::consts::TAU)
}

// ---------------------------------------------------------------------
// line / NURBS: two-plane reduction

/// Two planes through the line, mutually perpendicular; the line is
/// their intersection. The roots common to both reduced univariate
/// polynomials are the curve parameters of candidate hits.
fn line_nurbs(l: &Line3, dl: Domain, n: &NurbsCurve, tol: f64) -> Result<Vec<CurveHit>, GeomError> {
    // Build the perpendicular plane pair.
    let helper = if l.dir.x.abs() <= l.dir.y.abs() && l.dir.x.abs() <= l.dir.z.abs() {
        Vec3::new(1., 0., 0.)
    } else if l.dir.y.abs() <= l.dir.z.abs() {
        Vec3::new(0., 1., 0.)
    } else {
        Vec3::new(0., 0., 1.)
    };
    let n1 = l
        .dir
        .cross(helper)
        .try_normalize()
        .ok_or(GeomError::Degenerate)?;
    let n2 = l.dir.cross(n1);
    let mut hits = Vec::new();
    let (dom_a, dom_b) = n.domain();
    for seg in n.to_beziers() {
        // f_i(t) = n_i . C(t) - n_i . origin in Bernstein form over
        // the segment: for homogeneous control (X, w),
        // n . (X/w) - d = (n . X - d w) / w; numerator coefficients
        // are exact Bernstein coefficients.
        let coeffs1: Vec<f64> = seg
            .ctrl
            .iter()
            .map(|c| n1.dot(Vec3::new(c.x, c.y, c.z)) - n1.dot(l.origin) * c.w)
            .collect();
        let coeffs2: Vec<f64> = seg
            .ctrl
            .iter()
            .map(|c| n2.dot(Vec3::new(c.x, c.y, c.z)) - n2.dot(l.origin) * c.w)
            .collect();
        // The curve segment may lie entirely IN one of the planes
        // (e.g. a planar curve coplanar with the line): that
        // polynomial is identically zero and carries no information.
        // Take roots from the nondegenerate one; both zero means the
        // segment is coincident with the line.
        let scale = |cs: &[f64]| cs.iter().fold(0.0f64, |m, c| m.max(c.abs()));
        let (s1, s2) = (scale(&coeffs1), scale(&coeffs2));
        let seg_size = seg
            .control_points()
            .iter()
            .fold(0.0f64, |m, p| m.max((*p - l.origin).norm()));
        let zero_eps = 1e-13 * (1.0 + seg_size);
        let src = if s1 <= zero_eps && s2 <= zero_eps {
            return Err(GeomError::Degenerate); // coincident overlap
        } else if s1 >= s2 {
            Bernstein::new(coeffs1)
        } else {
            Bernstein::new(coeffs2)
        };
        let Some(bsrc) = src else { continue };
        // Verify the true 3D distance at each candidate root.
        for s in bsrc.roots(1e-12) {
            let p = seg.point(s);
            if dist_point_line(p, l) <= tol {
                let t_line = (p - l.origin).dot(l.dir);
                let t_curve = seg.u0 + s * (seg.u1 - seg.u0);
                if domain_contains(dl, t_line, tol) {
                    let tangent_c = curve_tangent(n, t_curve);
                    let tangential =
                        l.dir.cross(tangent_c).norm() <= TANGENT_EPS * tangent_c.norm();
                    hits.push(CurveHit {
                        t1: t_line,
                        t2: t_curve.clamp(dom_a, dom_b),
                        point: p,
                        tangential,
                    });
                }
            }
        }
    }
    dedup_hits(&mut hits, tol);
    Ok(hits)
}

fn dist_point_line(p: Vec3, l: &Line3) -> f64 {
    let w = p - l.origin;
    (w - l.dir * w.dot(l.dir)).norm()
}

fn curve_tangent(n: &NurbsCurve, t: f64) -> Vec3 {
    let d = n.derivatives(t, 1);
    d[1]
}

// ---------------------------------------------------------------------
// NURBS / NURBS via PP on exact difference systems

/// Affine remap from a sub-curve's [0,1] back to a conic's angle or a
/// curve's native parameter.
#[derive(Clone, Copy)]
struct Remap {
    scale: f64,
    offset: f64,
}

impl Remap {
    fn identity() -> Self {
        Remap {
            scale: 1.0,
            offset: 0.0,
        }
    }
    fn remap(&self, t: f64) -> f64 {
        self.offset + self.scale * t
    }
}

/// Exact NURBS forms for the generic path. Circles and ellipses map
/// angle linearly onto the NURBS domain (the exact rational circle's
/// parameter is NOT the angle; we return hits in NURBS parameters and
/// convert by computing the angle at the hit point instead).
fn to_nurbs(c: &Curve3) -> Result<(NurbsCurve, Remap), GeomError> {
    match c {
        Curve3::Nurbs(n) => Ok((n.clone(), Remap::identity())),
        Curve3::Circle(ci) => {
            let n = NurbsCurve::full_circle(ci.center, ci.x_axis, ci.y_axis, ci.radius)?;
            Ok((n, Remap::identity()))
        }
        Curve3::Ellipse(e) => {
            // Exact rational ellipse: anisotropic scaling of the unit
            // circle's homogeneous control in the ellipse frame.
            let n = NurbsCurve::full_circle(e.center, e.x_axis, e.y_axis, 1.0)?;
            let ctrl: Vec<keel_math::vec::Vec4> = n
                .homogeneous_control()
                .iter()
                .map(|h| {
                    let p = Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w);
                    let w = p - e.center;
                    let scaled = e.center
                        + e.x_axis * (w.dot(e.x_axis) * e.a)
                        + e.y_axis * (w.dot(e.y_axis) * e.b);
                    keel_math::vec::Vec4::new(scaled.x * h.w, scaled.y * h.w, scaled.z * h.w, h.w)
                })
                .collect();
            let n2 = NurbsCurve::from_homogeneous(n.knot_vector().clone(), ctrl)?;
            Ok((n2, Remap::identity()))
        }
        Curve3::Line(_) => Err(GeomError::Degenerate), // lines never take this path
    }
}

/// Per-Bezier-segment-pair PP solve of the homogeneous cross-
/// multiplied difference system:
/// X_a(s) w_b(t) - X_b(t) w_a(s) = 0 (three coordinates), an EXACT
/// bivariate Bernstein tensor: coeff[i][j] = xa_i * wb_j - xb_j * wa_i.
fn nurbs_nurbs(a: &NurbsCurve, b: &NurbsCurve, tol: f64) -> Result<Vec<CurveHit>, GeomError> {
    let mut hits: Vec<CurveHit> = Vec::new();
    for sa in a.to_beziers() {
        for sb in b.to_beziers() {
            // Quick AABB reject.
            if !aabb_overlap(&sa, &sb, tol) {
                continue;
            }
            let (pa, pb) = (sa.ctrl.len(), sb.ctrl.len());
            let mut systems = Vec::with_capacity(3);
            for axis in 0..3 {
                let mut coeffs = Vec::with_capacity(pa * pb);
                for ca in &sa.ctrl {
                    let xa = [ca.x, ca.y, ca.z][axis];
                    for cb in &sb.ctrl {
                        let xb = [cb.x, cb.y, cb.z][axis];
                        coeffs.push(xa * cb.w - xb * ca.w);
                    }
                }
                systems.push(
                    MultiBernstein::new(vec![pa - 1, pb - 1], coeffs)
                        .ok_or(GeomError::Degenerate)?,
                );
            }
            let Some(boxes) = solve_system(&systems, PP_TOL, PP_BUDGET) else {
                // Budget exhausted: overlap/coincidence suspected.
                return Err(GeomError::Degenerate);
            };
            for bx in boxes {
                let s = 0.5 * bx.lo[0] + 0.5 * bx.hi[0];
                let t = 0.5 * bx.lo[1] + 0.5 * bx.hi[1];
                let (s, t) = polish_pair(&sa, &sb, s, t);
                let p1 = sa.point(s);
                let p2 = sb.point(t);
                if (p1 - p2).norm() <= tol {
                    let g1 = a.derivatives(sa.u0 + s * (sa.u1 - sa.u0), 1)[1];
                    let g2 = b.derivatives(sb.u0 + t * (sb.u1 - sb.u0), 1)[1];
                    let tangential = g1.cross(g2).norm() <= TANGENT_EPS * g1.norm() * g2.norm();
                    hits.push(CurveHit {
                        t1: sa.u0 + s * (sa.u1 - sa.u0),
                        t2: sb.u0 + t * (sb.u1 - sb.u0),
                        point: (p1 + p2) * 0.5,
                        tangential,
                    });
                }
            }
        }
    }
    dedup_hits(&mut hits, tol);
    Ok(hits)
}

fn aabb_overlap(a: &BezierSegment, b: &BezierSegment, tol: f64) -> bool {
    let bounds = |s: &BezierSegment| {
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in s.control_points() {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        (lo, hi)
    };
    let (alo, ahi) = bounds(a);
    let (blo, bhi) = bounds(b);
    alo.x <= bhi.x + tol
        && blo.x <= ahi.x + tol
        && alo.y <= bhi.y + tol
        && blo.y <= ahi.y + tol
        && alo.z <= bhi.z + tol
        && blo.z <= ahi.z + tol
}

/// Damped 2-var Newton on the squared distance between two Bezier
/// segments, clamped to [0,1]^2.
fn polish_pair(sa: &BezierSegment, sb: &BezierSegment, mut s: f64, mut t: f64) -> (f64, f64) {
    for _ in 0..20 {
        let (p1, d1, dd1) = seg_ders2(sa, s);
        let (p2, d2, dd2) = seg_ders2(sb, t);
        let d = p1 - p2;
        let g1 = d.dot(d1);
        let g2 = -d.dot(d2);
        let h11 = d1.dot(d1) + d.dot(dd1);
        let h22 = d2.dot(d2) - d.dot(dd2);
        let h12 = -d1.dot(d2);
        let det = h11 * h22 - h12 * h12;
        if det.abs() < 1e-300 {
            break;
        }
        let ds = (-g1 * h22 + g2 * h12) / det;
        let dt = (-g2 * h11 + g1 * h12) / det;
        let ns = (s + ds).clamp(0.0, 1.0);
        let nt = (t + dt).clamp(0.0, 1.0);
        let step = (ns - s).abs() + (nt - t).abs();
        s = ns;
        t = nt;
        if step < 1e-15 {
            break;
        }
    }
    (s, t)
}

/// Point and first/second derivatives of a Bezier segment at local t,
/// via the segment's own NURBS form (single span).
fn seg_ders2(seg: &BezierSegment, t: f64) -> (Vec3, Vec3, Vec3) {
    let p = seg.ctrl.len() - 1;
    let mut knots = vec![0.0; p + 1];
    knots.extend(vec![1.0; p + 1]);
    let Ok(kv) = crate::knots::KnotVector::new(p.max(1), knots) else {
        return (seg.point(t), Vec3::ZERO, Vec3::ZERO);
    };
    let Ok(c) = NurbsCurve::from_homogeneous(kv, seg.ctrl.clone()) else {
        return (seg.point(t), Vec3::ZERO, Vec3::ZERO);
    };
    let d = c.derivatives(t, 2);
    (d[0], d[1], d[2])
}

// ---------------------------------------------------------------------
// curve / surface intersection (CSI)

use crate::nurbs_surface::NurbsSurface;
use crate::surface::Surface3;

/// An intersection point between a curve and a surface.
#[derive(Clone, Debug)]
pub struct SurfaceHit {
    /// Curve parameter.
    pub t: f64,
    /// Surface parameters at the hit.
    pub uv: (f64, f64),
    pub point: Vec3,
    pub tangential: bool,
}

/// Surface operand for CSI.
pub enum SurfaceRef<'a> {
    Analytic(&'a Surface3),
    Nurbs(&'a NurbsSurface),
}

/// Intersect a bounded curve with a surface.
pub fn intersect_curve_surface(
    c: &Curve3,
    dc: Domain,
    s: &SurfaceRef<'_>,
    tol: f64,
) -> Result<Vec<SurfaceHit>, GeomError> {
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    match s {
        SurfaceRef::Analytic(surf) => {
            let n = match c {
                Curve3::Line(l) => return line_analytic_surface(l, dc, surf, tol),
                _ => to_nurbs(c)?.0,
            };
            curve_analytic_surface(&n, surf, tol)
        }
        SurfaceRef::Nurbs(ns) => {
            let n = match c {
                Curve3::Line(_) => {
                    // Lines vs spline surfaces: bound the line to the
                    // surface's control AABB and route through the
                    // NURBS path as an exact degree-1 curve.
                    line_as_segment_curve(c, ns, tol)?
                }
                _ => to_nurbs(c)?.0,
            };
            nurbs_curve_nurbs_surface(&n, ns, tol)
        }
    }
}

/// Line restricted to a finite span covering the surface's control
/// AABB, as an exact degree-1 NURBS.
fn line_as_segment_curve(c: &Curve3, ns: &NurbsSurface, tol: f64) -> Result<NurbsCurve, GeomError> {
    let Curve3::Line(l) = c else {
        return Err(GeomError::Degenerate);
    };
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for h in ns.homogeneous_control() {
        let p = Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w);
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let center = (lo + hi) * 0.5;
    let radius = (hi - lo).norm() * 0.5 + 10.0 * tol;
    let mid = (center - l.origin).dot(l.dir);
    let (t0, t1) = (mid - radius, mid + radius);
    NurbsCurve::new(
        1,
        vec![t0, t0, t1, t1],
        vec![l.point(t0), l.point(t1)],
        None,
    )
}

/// Line vs analytic surface: exact low-degree polynomial roots of the
/// implicit form restricted to the line.
fn line_analytic_surface(
    l: &Line3,
    dl: Domain,
    s: &Surface3,
    tol: f64,
) -> Result<Vec<SurfaceHit>, GeomError> {
    // Power-basis coefficients of f(o + t d) by surface type; degree
    // <= 2 except the torus quartic.
    let o = l.origin;
    let d = l.dir;
    let roots: Vec<f64> = match s {
        Surface3::Plane(p) => {
            let denom = d.dot(p.frame.z);
            let num = (o - p.frame.origin).dot(p.frame.z);
            if denom.abs() <= 1e-14 {
                if num.abs() <= tol {
                    return Err(GeomError::Degenerate); // line in plane
                }
                Vec::new()
            } else {
                vec![-num / denom]
            }
        }
        Surface3::Sphere(sp) => {
            let w = o - sp.frame.origin;
            keel_math::poly::solve_quadratic(
                d.dot(d),
                2.0 * w.dot(d),
                w.dot(w) - sp.radius * sp.radius,
            )
        }
        Surface3::Cylinder(cy) => {
            let z = cy.frame.z;
            let w = o - cy.frame.origin;
            let wp = w - z * w.dot(z);
            let dp = d - z * d.dot(z);
            let qa = dp.dot(dp);
            if qa <= 1e-28 {
                // Line parallel to the axis: on the wall or not.
                if (wp.norm() - cy.radius).abs() <= tol {
                    return Err(GeomError::Degenerate);
                }
                Vec::new()
            } else {
                keel_math::poly::solve_quadratic(
                    qa,
                    2.0 * wp.dot(dp),
                    wp.dot(wp) - cy.radius * cy.radius,
                )
            }
        }
        Surface3::Cone(co) => {
            let m = co.half_angle.tan();
            let z = co.frame.z;
            let w = o - co.frame.origin;
            let (wh, dh) = (w.dot(z), d.dot(z));
            let wp = w - z * wh;
            let dp = d - z * dh;
            // |wp + t dp|^2 - (r0 + m (wh + t dh))^2 = 0.
            let qa = dp.dot(dp) - m * m * dh * dh;
            let qb = 2.0 * (wp.dot(dp) - m * dh * (co.radius + m * wh));
            let qc = wp.dot(wp) - (co.radius + m * wh) * (co.radius + m * wh);
            if qa.abs() <= 1e-28 && qb.abs() <= 1e-28 {
                if qc.abs() <= tol {
                    return Err(GeomError::Degenerate);
                }
                Vec::new()
            } else if qa.abs() <= 1e-28 {
                vec![-qc / qb]
            } else {
                keel_math::poly::solve_quadratic(qa, qb, qc)
            }
        }
        Surface3::Torus(t) => {
            // Quartic: ((|q|^2 + R^2 - r^2)^2 - 4 R^2 (|q|^2 - (q.z)^2))
            // with q(t) = w + t d. Build power-basis coefficient arrays
            // exactly and route through Bernstein on a bounded span.
            let z = t.frame.z;
            let w = o - t.frame.origin;
            // |q|^2 = at^2 + bt + c; (q.z)^2 = (e t + f)^2.
            let (qa, qb, qc) = (d.dot(d), 2.0 * w.dot(d), w.dot(w));
            let (e, f) = (d.dot(z), w.dot(z));
            let k = t.major * t.major - t.minor * t.minor;
            // A(t) = |q|^2 + k: degree 2.
            let a_poly = [qc + k, qb, qa];
            // B(t) = |q|^2 - (q.z)^2: degree 2.
            let b_poly = [qc - f * f, qb - 2.0 * e * f, qa - e * e];
            // f(t) = A^2 - 4 R^2 B: degree 4.
            let mut quart = [0.0f64; 5];
            for (i, &ai) in a_poly.iter().enumerate() {
                for (j, &aj) in a_poly.iter().enumerate() {
                    quart[i + j] += ai * aj;
                }
            }
            let four_r2 = 4.0 * t.major * t.major;
            for (i, &bi) in b_poly.iter().enumerate() {
                quart[i] -= four_r2 * bi;
            }
            // Bound the parameter span by the torus extent.
            let reach = t.major + t.minor + 10.0 * tol;
            let mid = (t.frame.origin - o).dot(d);
            let (t0, t1) = (mid - reach, mid + reach);
            // Affine substitution t = t0 + (t1 - t0) s into the power
            // poly, then Bernstein roots on s in [0,1].
            let subs = power_affine(&quart, t0, t1 - t0);
            match Bernstein::from_power(&subs) {
                Some(b) => b
                    .roots(1e-12)
                    .into_iter()
                    .map(|s| t0 + (t1 - t0) * s)
                    .collect(),
                None => Vec::new(),
            }
        }
    };
    let mut hits = Vec::new();
    for t in roots {
        if !domain_contains(dl, t, tol) {
            continue;
        }
        let p = l.point(t);
        let pr = s.project(p)?;
        if pr.distance > tol {
            continue;
        }
        let grad = s.implicit_gradient(p);
        let tangential = d.dot(grad).abs() <= TANGENT_EPS * d.norm() * grad.norm().max(1e-300);
        hits.push(SurfaceHit {
            t,
            uv: (pr.u, pr.v),
            point: pr.point,
            tangential,
        });
    }
    Ok(hits)
}

/// Power-basis affine substitution: p(t0 + s*scale) coefficients in s.
fn power_affine(p: &[f64], t0: f64, scale: f64) -> Vec<f64> {
    let n = p.len();
    let mut out = vec![0.0; n];
    // p(t) = sum a_i t^i; t = t0 + scale*s; expand binomially.
    for (i, &ai) in p.iter().enumerate() {
        // (t0 + scale*s)^i = sum_j C(i,j) t0^(i-j) scale^j s^j.
        let mut binom = 1.0f64;
        for (j, slot) in out.iter_mut().enumerate().take(i + 1) {
            if j > 0 {
                binom = binom * (i - j + 1) as f64 / j as f64;
            }
            *slot += ai * binom * t0.powi((i - j) as i32) * scale.powi(j as i32);
        }
    }
    out
}

/// General curve vs analytic surface: EXACT Bernstein composition of
/// the implicit form with each rational Bezier segment (never sample-
/// and-fit). For quadrics the composed numerator has degree 2p; the
/// torus gives 4p.
fn curve_analytic_surface(
    n: &NurbsCurve,
    s: &Surface3,
    tol: f64,
) -> Result<Vec<SurfaceHit>, GeomError> {
    let mut hits = Vec::new();
    for seg in n.to_beziers() {
        // Component Bernsteins of the homogeneous segment.
        let comp = |pick: fn(&keel_math::vec::Vec4) -> f64| -> Option<Bernstein> {
            Bernstein::new(seg.ctrl.iter().map(pick).collect())
        };
        let (Some(bx), Some(by), Some(bz), Some(bw)) =
            (comp(|c| c.x), comp(|c| c.y), comp(|c| c.z), comp(|c| c.w))
        else {
            continue;
        };
        let f = compose_implicit(s, &bx, &by, &bz, &bw);
        for t_local in f.roots(1e-12) {
            let p = seg.point(t_local);
            let pr = s.project(p)?;
            if pr.distance > tol {
                continue;
            }
            let t = seg.u0 + t_local * (seg.u1 - seg.u0);
            let tangent = curve_tangent(n, t);
            let grad = s.implicit_gradient(p);
            let tangential =
                tangent.dot(grad).abs() <= TANGENT_EPS * tangent.norm() * grad.norm().max(1e-300);
            hits.push(SurfaceHit {
                t,
                uv: (pr.u, pr.v),
                point: pr.point,
                tangential,
            });
        }
    }
    dedup_surface_hits(&mut hits, tol);
    Ok(hits)
}

/// The composed numerator polynomial f(C(t)) * w(t)^deg(f) in exact
/// Bernstein form, built from Bernstein products only.
fn compose_implicit(
    s: &Surface3,
    bx: &Bernstein,
    by: &Bernstein,
    bz: &Bernstein,
    bw: &Bernstein,
) -> Bernstein {
    // q = X - o*w in surface-local terms; helper dot products.
    let dotc = |a: (&Bernstein, &Bernstein, &Bernstein),
                b: (&Bernstein, &Bernstein, &Bernstein)|
     -> Bernstein { a.0.mul(b.0).add(&a.1.mul(b.1)).add(&a.2.mul(b.2)) };
    let origin = match s {
        Surface3::Plane(p) => p.frame.origin,
        Surface3::Sphere(sp) => sp.frame.origin,
        Surface3::Cylinder(c) => c.frame.origin,
        Surface3::Cone(c) => c.frame.origin,
        Surface3::Torus(t) => t.frame.origin,
    };
    let q = (
        bx.add(&bw.scale(-origin.x)),
        by.add(&bw.scale(-origin.y)),
        bz.add(&bw.scale(-origin.z)),
    );
    let qr = (&q.0, &q.1, &q.2);
    let zdir = match s {
        Surface3::Plane(p) => p.frame.z,
        Surface3::Sphere(_) => Vec3::new(0., 0., 1.), // unused
        Surface3::Cylinder(c) => c.frame.z,
        Surface3::Cone(c) => c.frame.z,
        Surface3::Torus(t) => t.frame.z,
    };
    let qz =
        q.0.scale(zdir.x)
            .add(&q.1.scale(zdir.y))
            .add(&q.2.scale(zdir.z));
    match s {
        Surface3::Plane(_) => qz,
        Surface3::Sphere(sp) => {
            // |q|^2 - r^2 w^2.
            dotc(qr, qr).add(&bw.mul(bw).scale(-(sp.radius * sp.radius)))
        }
        Surface3::Cylinder(c) => {
            // |q|^2 - qz^2 - r^2 w^2.
            dotc(qr, qr)
                .add(&qz.mul(&qz).scale(-1.0))
                .add(&bw.mul(bw).scale(-(c.radius * c.radius)))
        }
        Surface3::Cone(c) => {
            // |q|^2 - qz^2 - (r0 w + m qz)^2.
            let m = c.half_angle.tan();
            let rim = bw.scale(c.radius).add(&qz.scale(m));
            dotc(qr, qr)
                .add(&qz.mul(&qz).scale(-1.0))
                .add(&rim.mul(&rim).scale(-1.0))
        }
        Surface3::Torus(t) => {
            // A = |q|^2 + (R^2 - r^2) w^2; B = |q|^2 - qz^2;
            // f = A^2 - 4 R^2 w^2 B.
            let k = t.major * t.major - t.minor * t.minor;
            let q2 = dotc(qr, qr);
            let a = q2.add(&bw.mul(bw).scale(k));
            let b = q2.add(&qz.mul(&qz).scale(-1.0));
            a.mul(&a)
                .add(&bw.mul(bw).mul(&b).scale(-4.0 * t.major * t.major))
        }
    }
}

fn dedup_surface_hits(hits: &mut Vec<SurfaceHit>, tol: f64) {
    hits.sort_by(|a, b| a.t.total_cmp(&b.t));
    hits.dedup_by(|a, b| (a.point - b.point).norm() <= 2.0 * tol && (a.t - b.t).abs() <= 1e-6);
}

/// NURBS curve x NURBS surface: per patch/segment PP solve of the
/// exact homogeneous difference system in (u, v, t):
/// X_S(u,v) w_C(t) - X_C(t) w_S(u,v) = 0.
fn nurbs_curve_nurbs_surface(
    n: &NurbsCurve,
    ns: &NurbsSurface,
    tol: f64,
) -> Result<Vec<SurfaceHit>, GeomError> {
    let mut hits: Vec<SurfaceHit> = Vec::new();
    let patches = ns.to_bezier_patches()?;
    for seg in n.to_beziers() {
        for patch in &patches {
            let (lo, hi) = patch.control_aabb();
            if !aabb_overlap_box(&seg, lo, hi, tol) {
                continue;
            }
            let (pu, qv) = (patch.p, patch.q);
            let pc = seg.ctrl.len() - 1;
            let cols = qv + 1;
            let mut systems = Vec::with_capacity(3);
            for axis in 0..3 {
                let mut coeffs = Vec::with_capacity((pu + 1) * (qv + 1) * (pc + 1));
                for i in 0..=pu {
                    for j in 0..=qv {
                        let cs = patch.ctrl[i * cols + j];
                        let xs = [cs.x, cs.y, cs.z][axis];
                        for cc in &seg.ctrl {
                            let xc = [cc.x, cc.y, cc.z][axis];
                            coeffs.push(xs * cc.w - xc * cs.w);
                        }
                    }
                }
                systems.push(
                    MultiBernstein::new(vec![pu, qv, pc], coeffs).ok_or(GeomError::Degenerate)?,
                );
            }
            let Some(boxes) = solve_system(&systems, PP_TOL, PP_BUDGET) else {
                return Err(GeomError::Degenerate);
            };
            for bx in boxes {
                let mut u = 0.5 * bx.lo[0] + 0.5 * bx.hi[0];
                let mut v = 0.5 * bx.lo[1] + 0.5 * bx.hi[1];
                let mut t = 0.5 * bx.lo[2] + 0.5 * bx.hi[2];
                // 3x3 Newton polish on S(u,v) - C(t) = 0 in GLOBAL
                // parameters.
                let gu0 = patch.u0;
                let gu1 = patch.u1;
                let gv0 = patch.v0;
                let gv1 = patch.v1;
                let gt0 = seg.u0;
                let gt1 = seg.u1;
                let (mut gu, mut gv, mut gt) = (
                    gu0 + u * (gu1 - gu0),
                    gv0 + v * (gv1 - gv0),
                    gt0 + t * (gt1 - gt0),
                );
                for _ in 0..20 {
                    let sd = ns.derivatives(gu, gv, 1);
                    let cd = n.derivatives(gt, 1);
                    let r = sd[0][0] - cd[0];
                    let (su, sv, ct) = (sd[1][0], sd[0][1], cd[1]);
                    // Solve [su sv -ct] x = -r by Cramer.
                    let det = su.dot(sv.cross(ct * -1.0));
                    if det.abs() < 1e-300 {
                        break;
                    }
                    let rhs = r * -1.0;
                    let du = rhs.dot(sv.cross(ct * -1.0)) / det;
                    let dv = su.dot(rhs.cross(ct * -1.0)) / det;
                    let dt = su.dot(sv.cross(rhs)) / det;
                    gu = (gu + du).clamp(gu0, gu1);
                    gv = (gv + dv).clamp(gv0, gv1);
                    gt = (gt + dt).clamp(gt0, gt1);
                    if du.abs() + dv.abs() + dt.abs() < 1e-15 {
                        break;
                    }
                }
                u = gu;
                v = gv;
                t = gt;
                let p1 = ns.point(u, v);
                let p2 = n.point(t);
                if (p1 - p2).norm() <= tol {
                    let sd = ns.derivatives(u, v, 1);
                    let normal = sd[1][0].cross(sd[0][1]);
                    let tangent = n.derivatives(t, 1)[1];
                    let tangential = tangent.dot(normal).abs()
                        <= TANGENT_EPS * tangent.norm() * normal.norm().max(1e-300);
                    hits.push(SurfaceHit {
                        t,
                        uv: (u, v),
                        point: (p1 + p2) * 0.5,
                        tangential,
                    });
                }
            }
        }
    }
    dedup_surface_hits(&mut hits, tol);
    Ok(hits)
}

fn aabb_overlap_box(seg: &BezierSegment, lo: Vec3, hi: Vec3, tol: f64) -> bool {
    let mut slo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut shi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in seg.control_points() {
        slo = Vec3::new(slo.x.min(p.x), slo.y.min(p.y), slo.z.min(p.z));
        shi = Vec3::new(shi.x.max(p.x), shi.y.max(p.y), shi.z.max(p.z));
    }
    slo.x <= hi.x + tol
        && lo.x <= shi.x + tol
        && slo.y <= hi.y + tol
        && lo.y <= shi.y + tol
        && slo.z <= hi.z + tol
        && lo.z <= shi.z + tol
}

/// Merge hits that coincide in space and parameters (cluster artifacts
/// from segment boundaries and PP box splits).
fn dedup_hits(hits: &mut Vec<CurveHit>, tol: f64) {
    hits.sort_by(|a, b| a.t1.total_cmp(&b.t1).then(a.t2.total_cmp(&b.t2)));
    hits.dedup_by(|a, b| {
        (a.point - b.point).norm() <= 2.0 * tol
            && (a.t1 - b.t1).abs() <= 1e-6
            && (a.t2 - b.t2).abs() <= 1e-6
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Circle3, Line3};

    const TOL: f64 = 1e-9;

    fn full() -> Domain {
        Domain::Periodic {
            period: core::f64::consts::TAU,
        }
    }
    fn span(a: f64, b: f64) -> Domain {
        Domain::Finite { a, b }
    }

    #[test]
    fn line_line_crossing_and_parallel() {
        let a = Curve3::Line(Line3::new(Vec3::ZERO, Vec3::new(1., 0., 0.)).unwrap());
        let b = Curve3::Line(Line3::new(Vec3::new(2., -1., 0.), Vec3::new(0., 1., 0.)).unwrap());
        let hits = intersect_curves(&a, span(-10., 10.), &b, span(-10., 10.), TOL).unwrap();
        assert_eq!(hits.len(), 1);
        assert!((hits[0].point - Vec3::new(2., 0., 0.)).norm() < 1e-12);
        assert!((hits[0].t1 - 2.0).abs() < 1e-12);
        assert!((hits[0].t2 - 1.0).abs() < 1e-12);
        // Parallel distinct: no hits. Coincident: degenerate.
        let c = Curve3::Line(Line3::new(Vec3::new(0., 1., 0.), Vec3::new(1., 0., 0.)).unwrap());
        assert!(
            intersect_curves(&a, span(-10., 10.), &c, span(-10., 10.), TOL)
                .unwrap()
                .is_empty()
        );
        let d = Curve3::Line(Line3::new(Vec3::new(5., 0., 0.), Vec3::new(1., 0., 0.)).unwrap());
        assert_eq!(
            intersect_curves(&a, span(-10., 10.), &d, span(-10., 10.), TOL).unwrap_err(),
            GeomError::Degenerate
        );
    }

    #[test]
    fn line_circle_secant_tangent_miss() {
        let circ = Curve3::Circle(
            Circle3::new(
                Vec3::ZERO,
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                2.0,
            )
            .unwrap(),
        );
        // Secant through the center.
        let sec = Curve3::Line(Line3::new(Vec3::new(-5., 0., 0.), Vec3::new(1., 0., 0.)).unwrap());
        let hits = intersect_curves(&sec, span(-20., 20.), &circ, full(), TOL).unwrap();
        assert_eq!(hits.len(), 2);
        // Tangent at (0, 2).
        let tan = Curve3::Line(Line3::new(Vec3::new(-5., 2., 0.), Vec3::new(1., 0., 0.)).unwrap());
        let hits = intersect_curves(&tan, span(-20., 20.), &circ, full(), TOL).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].tangential, "{hits:?}");
        // Miss.
        let miss = Curve3::Line(Line3::new(Vec3::new(-5., 3., 0.), Vec3::new(1., 0., 0.)).unwrap());
        assert!(
            intersect_curves(&miss, span(-20., 20.), &circ, full(), TOL)
                .unwrap()
                .is_empty()
        );
        // Out-of-plane piercing line through the rim.
        let pierce =
            Curve3::Line(Line3::new(Vec3::new(2., 0., -3.), Vec3::new(0., 0., 1.)).unwrap());
        let hits = intersect_curves(&pierce, span(-20., 20.), &circ, full(), TOL).unwrap();
        assert_eq!(hits.len(), 1);
        assert!((hits[0].point - Vec3::new(2., 0., 0.)).norm() < 1e-9);
    }

    #[test]
    fn circle_circle_two_one_zero() {
        let mk = |cx: f64, r: f64| {
            Curve3::Circle(
                Circle3::new(
                    Vec3::new(cx, 0., 0.),
                    Vec3::new(1., 0., 0.),
                    Vec3::new(0., 1., 0.),
                    r,
                )
                .unwrap(),
            )
        };
        // Two crossings: centers 3 apart, radii 2 and 2.
        let hits = intersect_curves(&mk(0., 2.), full(), &mk(3., 2.), full(), 1e-7).unwrap();
        assert_eq!(hits.len(), 2, "{hits:?}");
        for h in &hits {
            assert!((h.point.x - 1.5).abs() < 1e-6);
            assert!((h.point.y.abs() - (4.0f64 - 2.25).sqrt()).abs() < 1e-6);
        }
        // Disjoint: no hits.
        let hits = intersect_curves(&mk(0., 1.), full(), &mk(5., 1.), full(), 1e-7).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn line_nurbs_arc_hits() {
        let arc = NurbsCurve::circular_arc(
            Vec3::ZERO,
            Vec3::new(1., 0., 0.),
            Vec3::new(0., 1., 0.),
            2.0,
            core::f64::consts::PI,
        )
        .unwrap();
        let n = Curve3::Nurbs(arc);
        let sec = Curve3::Line(Line3::new(Vec3::new(-5., 1., 0.), Vec3::new(1., 0., 0.)).unwrap());
        let hits = intersect_curves(&sec, span(-20., 20.), &n, span(0., 1.), 1e-9).unwrap();
        // y = 1 crosses the upper half-circle twice.
        assert_eq!(hits.len(), 2, "{hits:?}");
        for h in &hits {
            assert!((h.point.y - 1.0).abs() < 1e-9);
            assert!((h.point.norm() - 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn line_through_sphere_and_tangent_to_cylinder() {
        use crate::surface::{Cylinder3, Frame3, Sphere3, Surface3};
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let sph = Surface3::Sphere(Sphere3::new(f.clone(), 2.0).unwrap());
        let l = Curve3::Line(Line3::new(Vec3::new(-5., 0., 0.), Vec3::new(1., 0., 0.)).unwrap());
        let hits =
            intersect_curve_surface(&l, span(-20., 20.), &SurfaceRef::Analytic(&sph), TOL).unwrap();
        assert_eq!(hits.len(), 2, "{hits:?}");
        for h in &hits {
            assert!((h.point.norm() - 2.0).abs() < 1e-9);
            assert!(!h.tangential);
        }
        // Tangent line touching the cylinder wall at x = r.
        let cyl = Surface3::Cylinder(Cylinder3::new(f, 1.5).unwrap());
        let t = Curve3::Line(Line3::new(Vec3::new(1.5, -5., 1.0), Vec3::new(0., 1., 0.)).unwrap());
        let hits = intersect_curve_surface(&t, span(-20., 20.), &SurfaceRef::Analytic(&cyl), 1e-7)
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|h| h.tangential), "{hits:?}");
        // Line through the torus tube: 4 hits on the x axis.
        use crate::surface::Torus3;
        let f2 = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let tor = Surface3::Torus(Torus3::new(f2, 3.0, 1.0).unwrap());
        let lx = Curve3::Line(Line3::new(Vec3::new(-9., 0., 0.), Vec3::new(1., 0., 0.)).unwrap());
        let hits = intersect_curve_surface(&lx, span(-20., 20.), &SurfaceRef::Analytic(&tor), 1e-7)
            .unwrap();
        assert_eq!(hits.len(), 4, "{hits:?}");
        let mut xs: Vec<f64> = hits.iter().map(|h| h.point.x).collect();
        xs.sort_by(f64::total_cmp);
        for (got, want) in xs.iter().zip([-4.0, -2.0, 2.0, 4.0]) {
            assert!((got - want).abs() < 1e-7, "{xs:?}");
        }
    }

    #[test]
    fn nurbs_arc_vs_analytic_sphere() {
        use crate::surface::{Frame3, Sphere3, Surface3};
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let sph = Surface3::Sphere(Sphere3::new(f, 2.0).unwrap());
        // A circle of radius 3 in the z = 0 plane centered at (2,0,0)
        // crosses the sphere |p| = 2 where x^2+y^2 = 4: exact circle-
        // circle geometry gives two symmetric hits.
        let arc = Curve3::Nurbs(
            NurbsCurve::full_circle(
                Vec3::new(2., 0., 0.),
                Vec3::new(1., 0., 0.),
                Vec3::new(0., 1., 0.),
                3.0,
            )
            .unwrap(),
        );
        let hits =
            intersect_curve_surface(&arc, span(0., 1.), &SurfaceRef::Analytic(&sph), 1e-7).unwrap();
        assert_eq!(hits.len(), 2, "{hits:?}");
        for h in &hits {
            assert!((h.point.norm() - 2.0).abs() < 1e-7);
            assert!(((h.point - Vec3::new(2., 0., 0.)).norm() - 3.0).abs() < 1e-7);
        }
    }

    #[test]
    fn nurbs_curve_vs_nurbs_surface() {
        use crate::nurbs_surface::revolve_full;
        // Revolved exact sphere of radius 2; a straight NURBS segment
        // through it along x.
        let profile = NurbsCurve::circular_arc(
            Vec3::ZERO,
            Vec3::new(0., 0., -1.),
            Vec3::new(1., 0., 0.),
            2.0,
            core::f64::consts::PI,
        )
        .unwrap();
        let s = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let c = Curve3::Nurbs(
            NurbsCurve::new(
                1,
                vec![0., 0., 1., 1.],
                vec![Vec3::new(-5., 0.3, 0.2), Vec3::new(5., 0.3, 0.2)],
                None,
            )
            .unwrap(),
        );
        let hits = intersect_curve_surface(&c, span(0., 1.), &SurfaceRef::Nurbs(&s), 1e-7).unwrap();
        assert_eq!(hits.len(), 2, "{hits:?}");
        for h in &hits {
            assert!((h.point.norm() - 2.0).abs() < 1e-6, "{h:?}");
        }
    }

    #[test]
    fn nurbs_nurbs_circle_pair() {
        let mk = |cx: f64| {
            Curve3::Nurbs(
                NurbsCurve::full_circle(
                    Vec3::new(cx, 0., 0.),
                    Vec3::new(1., 0., 0.),
                    Vec3::new(0., 1., 0.),
                    2.0,
                )
                .unwrap(),
            )
        };
        let hits = intersect_curves(&mk(0.), span(0., 1.), &mk(3.), span(0., 1.), 1e-7).unwrap();
        assert_eq!(hits.len(), 2, "{hits:?}");
        for h in &hits {
            assert!((h.point.x - 1.5).abs() < 1e-6, "{h:?}");
        }
    }
}
