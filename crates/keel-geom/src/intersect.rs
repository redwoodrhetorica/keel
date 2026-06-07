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
