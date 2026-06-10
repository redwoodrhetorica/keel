//! Metrology-grade fitting (corpus-audit opportunity B; dossiers 23 +
//! 17): fit analytic primitives to measured POINT SETS with an EXPLICIT
//! OBJECTIVE, never a silent least-squares default (the Shakarji
//! taxonomy: the GD&T standards prescribe which objective each datum /
//! form evaluation uses, and the answers genuinely differ).
//!
//! Objectives:
//! - LEAST SQUARES (Gaussian): the L2 fit. Plane = centroid + smallest
//!   covariance eigenvector (exact); circle/sphere = linearized Kasa
//!   seed + Gauss-Newton refinement on the geometric residual.
//! - MIN ZONE (Chebyshev): minimize the max deviation band. EXACT for
//!   the discrete point set by candidate enumeration from the contact
//!   characterization (a min-zone optimum is held by 3-1 or 2-2
//!   contacts for the plane; by a 3-1 circumcenter or a 2-2 bisector
//!   crossing for the annulus), with documented size caps (metrology
//!   point sets are CMM-probe sized, not meshes).
//! - MIN CIRCUMSCRIBED (shaft gauge): the minimum enclosing circle,
//!   exact by move-to-front Welzl (deterministic, no RNG).
//! - MAX INSCRIBED (hole gauge): the largest empty circle centred
//!   inside the convex hull, exact by Voronoi-vertex (triple
//!   circumcenter) + pair-midpoint candidate enumeration.
//!
//! Every fit reports BOTH residuals (rms and the Chebyshev zone), so a
//! least-squares datum can still be checked against a zone budget. The
//! canonical-recovery certifier (kernel/24) shares this philosophy: its
//! dense max-deviation certificate IS the Chebyshev residual.

use crate::GeomError;
use crate::recover::{smallest_eigenvector_sym3, solve3, solve4};
use keel_math::vec::Vec3;

/// Which functional the fit minimizes (Shakarji / ASME Y14.5 fits).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FitObjective {
    LeastSquares,
    MinZone,
    MinCircumscribed,
    MaxInscribed,
}

/// A fitted plane with both residuals.
#[derive(Clone, Debug)]
pub struct PlaneFit {
    pub origin: Vec3,
    /// Unit normal.
    pub normal: Vec3,
    pub rms: f64,
    /// Chebyshev band: max minus min signed distance.
    pub zone: f64,
}

/// A fitted circle in the XY plane (2D metrology profile).
#[derive(Clone, Debug)]
pub struct CircleFit2 {
    pub center: (f64, f64),
    pub radius: f64,
    pub rms: f64,
    /// Radial band about the center: r_max minus r_min.
    pub zone: f64,
}

/// A fitted sphere.
#[derive(Clone, Debug)]
pub struct SphereFit {
    pub center: Vec3,
    pub radius: f64,
    pub rms: f64,
    pub zone: f64,
}

/// Size caps for the exact enumeration objectives (documented honest
/// bounds; beyond them the fit DECLINES rather than degrade silently).
const MINZONE_CAP: usize = 48;
const INSCRIBED_CAP: usize = 96;

fn plane_residuals(pts: &[Vec3], origin: Vec3, n: Vec3) -> (f64, f64) {
    let mut sum2 = 0.0;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &p in pts {
        let d = (p - origin).dot(n);
        sum2 += d * d;
        lo = lo.min(d);
        hi = hi.max(d);
    }
    ((sum2 / pts.len() as f64).sqrt(), hi - lo)
}

/// Fit a plane to `pts` under `objective` (LeastSquares or MinZone;
/// the circumscribed/inscribed gauges are circle/cylinder concepts and
/// decline here).
pub fn fit_plane(pts: &[Vec3], objective: FitObjective) -> Result<PlaneFit, GeomError> {
    if pts.len() < 3 || pts.iter().any(|p| !p.is_finite()) {
        return Err(GeomError::Degenerate);
    }
    let centroid = pts.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / pts.len() as f64);
    let ls_normal = {
        let mut cov = [[0.0f64; 3]; 3];
        for &p in pts {
            let d = p - centroid;
            let v = [d.x, d.y, d.z];
            for (i, vi) in v.iter().enumerate() {
                for (j, vj) in v.iter().enumerate() {
                    cov[i][j] += vi * vj;
                }
            }
        }
        smallest_eigenvector_sym3(cov)
    };
    match objective {
        FitObjective::LeastSquares => {
            let (rms, zone) = plane_residuals(pts, centroid, ls_normal);
            Ok(PlaneFit {
                origin: centroid,
                normal: ls_normal,
                rms,
                zone,
            })
        }
        FitObjective::MinZone => {
            if pts.len() > MINZONE_CAP {
                return Err(GeomError::Degenerate);
            }
            // Exact: the optimal band direction is held by a 3-1
            // contact (normal of a point triple) or a 2-2 contact
            // (normal perpendicular to two chords). Enumerate both
            // candidate families plus the LS direction, keep the
            // narrowest band.
            let mut best: Option<(Vec3, f64)> = None;
            let mut consider = |n: Vec3| {
                let Some(n) = n.try_normalize() else { return };
                let (_, zone) = plane_residuals(pts, centroid, n);
                if best.as_ref().is_none_or(|(_, bz)| zone < *bz) {
                    best = Some((n, zone));
                }
            };
            consider(ls_normal);
            let n_pts = pts.len();
            for i in 0..n_pts {
                for j in (i + 1)..n_pts {
                    for k in (j + 1)..n_pts {
                        consider((pts[j] - pts[i]).cross(pts[k] - pts[i]));
                    }
                }
            }
            let mut chords = Vec::new();
            for i in 0..n_pts {
                for j in (i + 1)..n_pts {
                    chords.push(pts[j] - pts[i]);
                }
            }
            for i in 0..chords.len() {
                for j in (i + 1)..chords.len() {
                    consider(chords[i].cross(chords[j]));
                }
            }
            let (normal, zone) = best.ok_or(GeomError::Degenerate)?;
            let (rms, _) = plane_residuals(pts, centroid, normal);
            // Centre the band: origin at the mid-offset plane.
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &p in pts {
                let d = (p - centroid).dot(normal);
                lo = lo.min(d);
                hi = hi.max(d);
            }
            Ok(PlaneFit {
                origin: centroid + normal * (0.5 * (lo + hi)),
                normal,
                rms,
                zone,
            })
        }
        _ => Err(GeomError::Degenerate),
    }
}

fn circle_residuals(pts: &[(f64, f64)], c: (f64, f64), r: f64) -> (f64, f64) {
    let mut sum2 = 0.0;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in pts {
        let ri = ((x - c.0).powi(2) + (y - c.1).powi(2)).sqrt();
        let d = ri - r;
        sum2 += d * d;
        lo = lo.min(ri);
        hi = hi.max(ri);
    }
    ((sum2 / pts.len() as f64).sqrt(), hi - lo)
}

fn radial_extremes(pts: &[(f64, f64)], c: (f64, f64)) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in pts {
        let r = ((x - c.0).powi(2) + (y - c.1).powi(2)).sqrt();
        lo = lo.min(r);
        hi = hi.max(r);
    }
    (lo, hi)
}

/// Circumcenter of three 2D points; None if collinear.
fn circumcenter2(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64)> {
    let d = 2.0 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if d.abs() < 1e-14 {
        return None;
    }
    let a2 = a.0 * a.0 + a.1 * a.1;
    let b2 = b.0 * b.0 + b.1 * b.1;
    let c2 = c.0 * c.0 + c.1 * c.1;
    Some((
        (a2 * (b.1 - c.1) + b2 * (c.1 - a.1) + c2 * (a.1 - b.1)) / d,
        (a2 * (c.0 - b.0) + b2 * (a.0 - c.0) + c2 * (b.0 - a.0)) / d,
    ))
}

/// Smallest circle through 1/2/3 support points.
fn circle_of(support: &[(f64, f64)]) -> ((f64, f64), f64) {
    match support {
        [] => ((0.0, 0.0), 0.0),
        [a] => (*a, 0.0),
        [a, b] => {
            let c = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
            (c, 0.5 * ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt())
        }
        [a, b, c] => match circumcenter2(*a, *b, *c) {
            Some(cc) => {
                let r = ((a.0 - cc.0).powi(2) + (a.1 - cc.1).powi(2)).sqrt();
                (cc, r)
            }
            // Collinear support: span by the farthest pair.
            None => {
                let pairs = [(*a, *b), (*a, *c), (*b, *c)];
                let (p, q) = pairs
                    .iter()
                    .max_by(|(p1, q1), (p2, q2)| {
                        let d1 = (p1.0 - q1.0).powi(2) + (p1.1 - q1.1).powi(2);
                        let d2 = (p2.0 - q2.0).powi(2) + (p2.1 - q2.1).powi(2);
                        d1.total_cmp(&d2)
                    })
                    .copied()
                    .unwrap_or((*a, *b));
                circle_of(&[p, q])
            }
        },
        _ => unreachable!("welzl support is at most 3"),
    }
}

fn in_circle(p: (f64, f64), c: (f64, f64), r: f64) -> bool {
    (p.0 - c.0).powi(2) + (p.1 - c.1).powi(2) <= r * r * (1.0 + 1e-12) + 1e-24
}

/// Minimum enclosing circle: move-to-front Welzl (deterministic, no
/// RNG; metrology-sized inputs).
fn welzl_mec(pts: &[(f64, f64)]) -> ((f64, f64), f64) {
    let mut order: Vec<(f64, f64)> = pts.to_vec();
    let (mut c, mut r) = circle_of(&[]);
    let mut i = 0;
    while i < order.len() {
        if in_circle(order[i], c, r) {
            i += 1;
            continue;
        }
        // order[i] is on the boundary of the new circle.
        let (c1, r1) = mec_with_one(&order[..i], order[i]);
        c = c1;
        r = r1;
        let p = order.remove(i);
        order.insert(0, p);
        i = 1;
    }
    (c, r)
}

fn mec_with_one(pts: &[(f64, f64)], q: (f64, f64)) -> ((f64, f64), f64) {
    let (mut c, mut r) = circle_of(&[q]);
    for i in 0..pts.len() {
        if in_circle(pts[i], c, r) {
            continue;
        }
        let (c2, r2) = mec_with_two(&pts[..i], pts[i], q);
        c = c2;
        r = r2;
    }
    (c, r)
}

fn mec_with_two(pts: &[(f64, f64)], p: (f64, f64), q: (f64, f64)) -> ((f64, f64), f64) {
    let (mut c, mut r) = circle_of(&[p, q]);
    for &s in pts {
        if in_circle(s, c, r) {
            continue;
        }
        let (c3, r3) = circle_of(&[s, p, q]);
        c = c3;
        r = r3;
    }
    (c, r)
}

/// Convex hull (Andrew monotone chain), counterclockwise.
fn hull2(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut s: Vec<(f64, f64)> = pts.to_vec();
    s.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    s.dedup();
    if s.len() < 3 {
        return s;
    }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| -> f64 {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for &p in &s {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for &p in s.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn inside_hull(hull: &[(f64, f64)], p: (f64, f64)) -> bool {
    if hull.len() < 3 {
        return false;
    }
    for i in 0..hull.len() {
        let a = hull[i];
        let b = hull[(i + 1) % hull.len()];
        if (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0) < -1e-12 {
            return false;
        }
    }
    true
}

/// Fit a circle to 2D points under `objective`.
pub fn fit_circle_2d(pts: &[(f64, f64)], objective: FitObjective) -> Result<CircleFit2, GeomError> {
    if pts.len() < 3 || pts.iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
        return Err(GeomError::Degenerate);
    }
    match objective {
        FitObjective::LeastSquares => {
            // Kasa linearization: x^2 + y^2 + a x + b y + c = 0 in LS,
            // then Gauss-Newton on the geometric residual.
            let mut m = [[0.0f64; 3]; 3];
            let mut rhs = [0.0f64; 3];
            for &(x, y) in pts {
                let row = [x, y, 1.0];
                let t = -(x * x + y * y);
                for (i, ri) in row.iter().enumerate() {
                    for (j, rj) in row.iter().enumerate() {
                        m[i][j] += ri * rj;
                    }
                    rhs[i] += ri * t;
                }
            }
            let s = solve3(m, rhs).ok_or(GeomError::Degenerate)?;
            let mut c = (-0.5 * s.x, -0.5 * s.y);
            let mut r = (c.0 * c.0 + c.1 * c.1 - s.z).max(0.0).sqrt();
            for _ in 0..4 {
                // Gauss-Newton on sum (|p - c| - r)^2.
                let mut jtj = [[0.0f64; 3]; 3];
                let mut jtr = [0.0f64; 3];
                for &(x, y) in pts {
                    let (dx, dy) = (x - c.0, y - c.1);
                    let d = (dx * dx + dy * dy).sqrt().max(1e-12);
                    let res = d - r;
                    let row = [-dx / d, -dy / d, -1.0];
                    for (i, ri) in row.iter().enumerate() {
                        for (j, rj) in row.iter().enumerate() {
                            jtj[i][j] += ri * rj;
                        }
                        jtr[i] += ri * res;
                    }
                }
                let Some(step) = solve3(jtj, [-jtr[0], -jtr[1], -jtr[2]]) else {
                    break;
                };
                c.0 += step.x;
                c.1 += step.y;
                r += step.z;
            }
            let (rms, zone) = circle_residuals(pts, c, r);
            Ok(CircleFit2 {
                center: c,
                radius: r,
                rms,
                zone,
            })
        }
        FitObjective::MinCircumscribed => {
            let (c, r) = welzl_mec(pts);
            let (rms, zone) = circle_residuals(pts, c, r);
            Ok(CircleFit2 {
                center: c,
                radius: r,
                rms,
                zone,
            })
        }
        FitObjective::MaxInscribed => {
            if pts.len() > INSCRIBED_CAP {
                return Err(GeomError::Degenerate);
            }
            // Largest empty circle centred inside the hull: the optimum
            // is a Voronoi vertex (a triple circumcenter) or a pair
            // midpoint; enumerate both.
            let hull = hull2(pts);
            let n = pts.len();
            let mut best: Option<((f64, f64), f64)> = None;
            let mut consider = |c: (f64, f64)| {
                if !inside_hull(&hull, c) {
                    return;
                }
                let (rmin, _) = radial_extremes(pts, c);
                if best.as_ref().is_none_or(|(_, br)| rmin > *br) {
                    best = Some((c, rmin));
                }
            };
            for i in 0..n {
                for j in (i + 1)..n {
                    consider((0.5 * (pts[i].0 + pts[j].0), 0.5 * (pts[i].1 + pts[j].1)));
                    for k in (j + 1)..n {
                        if let Some(c) = circumcenter2(pts[i], pts[j], pts[k]) {
                            consider(c);
                        }
                    }
                }
            }
            let (c, r) = best.ok_or(GeomError::Degenerate)?;
            let (rms, zone) = circle_residuals(pts, c, r);
            Ok(CircleFit2 {
                center: c,
                radius: r,
                rms,
                zone,
            })
        }
        FitObjective::MinZone => {
            if pts.len() > MINZONE_CAP {
                return Err(GeomError::Degenerate);
            }
            // Exact annulus: the optimal centre is a 3-1 contact (a
            // triple circumcenter) or a 2-2 contact (the crossing of
            // two point-pair bisectors). Enumerate both families plus
            // the LS centre.
            let ls = fit_circle_2d(pts, FitObjective::LeastSquares)?;
            let n = pts.len();
            let mut best: Option<((f64, f64), f64)> = None;
            let mut consider = |c: (f64, f64)| {
                if !c.0.is_finite() || !c.1.is_finite() {
                    return;
                }
                let (rmin, rmax) = radial_extremes(pts, c);
                let zone = rmax - rmin;
                if best.as_ref().is_none_or(|(_, bz)| zone < *bz) {
                    best = Some((c, zone));
                }
            };
            consider(ls.center);
            for i in 0..n {
                for j in (i + 1)..n {
                    for k in (j + 1)..n {
                        if let Some(c) = circumcenter2(pts[i], pts[j], pts[k]) {
                            consider(c);
                        }
                    }
                }
            }
            // Bisector lines of all pairs: (midpoint, direction).
            let mut bis: Vec<((f64, f64), (f64, f64))> = Vec::new();
            for i in 0..n {
                for j in (i + 1)..n {
                    let m = (0.5 * (pts[i].0 + pts[j].0), 0.5 * (pts[i].1 + pts[j].1));
                    let d = (-(pts[j].1 - pts[i].1), pts[j].0 - pts[i].0);
                    bis.push((m, d));
                }
            }
            for i in 0..bis.len() {
                for j in (i + 1)..bis.len() {
                    let ((m1, d1), (m2, d2)) = (bis[i], bis[j]);
                    let det = d1.0 * (-d2.1) - (-d2.0) * d1.1;
                    if det.abs() < 1e-14 {
                        continue;
                    }
                    let (rx, ry) = (m2.0 - m1.0, m2.1 - m1.1);
                    let t = (rx * (-d2.1) - (-d2.0) * ry) / det;
                    consider((m1.0 + t * d1.0, m1.1 + t * d1.1));
                }
            }
            let (c, zone) = best.ok_or(GeomError::Degenerate)?;
            let (rmin, rmax) = radial_extremes(pts, c);
            let r = 0.5 * (rmin + rmax);
            let (rms, _) = circle_residuals(pts, c, r);
            Ok(CircleFit2 {
                center: c,
                radius: r,
                rms,
                zone,
            })
        }
    }
}

/// Fit a sphere (LeastSquares only in this milestone; the gauges and
/// min-zone sphere are the queued ladder).
pub fn fit_sphere(pts: &[Vec3], objective: FitObjective) -> Result<SphereFit, GeomError> {
    if objective != FitObjective::LeastSquares {
        return Err(GeomError::Degenerate);
    }
    if pts.len() < 4 || pts.iter().any(|p| !p.is_finite()) {
        return Err(GeomError::Degenerate);
    }
    // Linearized: |p|^2 + a x + b y + c z + d = 0, then Gauss-Newton.
    let mut m = [[0.0f64; 4]; 4];
    let mut rhs = [0.0f64; 4];
    for &p in pts {
        let row = [p.x, p.y, p.z, 1.0];
        let t = -(p.dot(p));
        for (i, ri) in row.iter().enumerate() {
            for (j, rj) in row.iter().enumerate() {
                m[i][j] += ri * rj;
            }
            rhs[i] += ri * t;
        }
    }
    let s = solve4(m, rhs).ok_or(GeomError::Degenerate)?;
    let mut c = Vec3::new(-0.5 * s[0], -0.5 * s[1], -0.5 * s[2]);
    let mut r = (c.dot(c) - s[3]).max(0.0).sqrt();
    for _ in 0..4 {
        let mut jtj = [[0.0f64; 4]; 4];
        let mut jtr = [0.0f64; 4];
        for &p in pts {
            let d = p - c;
            let dist = d.norm().max(1e-12);
            let res = dist - r;
            let row = [-d.x / dist, -d.y / dist, -d.z / dist, -1.0];
            for (i, ri) in row.iter().enumerate() {
                for (j, rj) in row.iter().enumerate() {
                    jtj[i][j] += ri * rj;
                }
                jtr[i] += ri * res;
            }
        }
        let Some(step) = solve4(jtj, [-jtr[0], -jtr[1], -jtr[2], -jtr[3]]) else {
            break;
        };
        c = c + Vec3::new(step[0], step[1], step[2]);
        r += step[3];
    }
    let mut sum2 = 0.0;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &p in pts {
        let ri = (p - c).norm();
        sum2 += (ri - r).powi(2);
        lo = lo.min(ri);
        hi = hi.max(ri);
    }
    Ok(SphereFit {
        center: c,
        radius: r,
        rms: (sum2 / pts.len() as f64).sqrt(),
        zone: hi - lo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_fits_ls_and_minzone() {
        // Points alternating +-e about a known tilted plane: LS
        // recovers the plane; the min-zone band is exactly 2e and is
        // NARROWER than a deliberately skewed band.
        let n = Vec3::new(0.3, 0.1, 1.0).try_normalize().unwrap();
        let o = Vec3::new(1.0, 2.0, 5.0);
        let (u, v) = {
            let u = n.cross(Vec3::new(1.0, 0.0, 0.0)).try_normalize().unwrap();
            (u, n.cross(u))
        };
        let e = 0.01;
        let mut pts = Vec::new();
        for i in 0..16 {
            let a = core::f64::consts::TAU * i as f64 / 16.0;
            let s = if i % 2 == 0 { e } else { -e };
            pts.push(o + u * (3.0 * a.cos()) + v * (2.0 * a.sin()) + n * s);
        }
        let ls = fit_plane(&pts, FitObjective::LeastSquares).unwrap();
        assert!(ls.normal.dot(n).abs() > 0.999_999, "LS normal off");
        assert!((ls.zone - 2.0 * e).abs() < 1e-9, "LS zone {}", ls.zone);
        let mz = fit_plane(&pts, FitObjective::MinZone).unwrap();
        assert!(mz.zone <= ls.zone + 1e-12, "min-zone must not exceed LS");
        assert!((mz.zone - 2.0 * e).abs() < 1e-9, "MZ zone {}", mz.zone);
    }

    #[test]
    fn circle_ls_recovers_and_zone_reports() {
        let c = (3.0, -2.0);
        let r = 5.0;
        let e = 0.02;
        let pts: Vec<(f64, f64)> = (0..24)
            .map(|i| {
                let a = core::f64::consts::TAU * i as f64 / 24.0;
                let ri = r + if i % 2 == 0 { e } else { -e };
                (c.0 + ri * a.cos(), c.1 + ri * a.sin())
            })
            .collect();
        let f = fit_circle_2d(&pts, FitObjective::LeastSquares).unwrap();
        assert!((f.center.0 - c.0).abs() < 1e-6 && (f.center.1 - c.1).abs() < 1e-6);
        assert!((f.radius - r).abs() < 1e-3, "r {}", f.radius);
        assert!((f.zone - 2.0 * e).abs() < 1e-9, "zone {}", f.zone);
    }

    #[test]
    fn circle_gauges_min_circumscribed_and_max_inscribed() {
        // A ring of points with one outlier OUT and one IN: the shaft
        // gauge (MCC) must cover the outer outlier; the hole gauge
        // (MIC) must stop at the inner one.
        let c = (1.0, 1.0);
        let mut pts: Vec<(f64, f64)> = (0..20)
            .map(|i| {
                let a = core::f64::consts::TAU * i as f64 / 20.0;
                (c.0 + 4.0 * a.cos(), c.1 + 4.0 * a.sin())
            })
            .collect();
        pts.push((c.0 + 4.3, c.1)); // outward outlier
        pts.push((c.0 - 3.6, c.1)); // inward outlier
        let mcc = fit_circle_2d(&pts, FitObjective::MinCircumscribed).unwrap();
        let (_, rmax) = radial_extremes(&pts, mcc.center);
        assert!(
            rmax <= mcc.radius * (1.0 + 1e-9),
            "MCC must enclose all points"
        );
        assert!(mcc.radius < 4.31, "MCC radius {} too big", mcc.radius);
        let mic = fit_circle_2d(&pts, FitObjective::MaxInscribed).unwrap();
        let (rmin, _) = radial_extremes(&pts, mic.center);
        assert!(
            mic.radius <= rmin * (1.0 + 1e-9),
            "MIC must stay empty of points"
        );
        assert!(mic.radius > 3.0, "MIC radius {} too small", mic.radius);
        assert!(mic.radius < 3.95, "MIC must respect the inward outlier");
    }

    #[test]
    fn circle_minzone_beats_ls_on_skewed_profiles() {
        // Points on two concentric arcs r +- e, alternating: the exact
        // annulus is 2e wide. An asymmetric distortion makes LS choose
        // a wider band, min-zone must still find the true one.
        let c = (0.5, -0.25);
        let r = 3.0;
        let e = 0.05;
        let pts: Vec<(f64, f64)> = (0..20)
            .map(|i| {
                let a = core::f64::consts::TAU * i as f64 / 20.0;
                let ri = r + if i % 2 == 0 { e } else { -e };
                (c.0 + ri * a.cos(), c.1 + ri * a.sin())
            })
            .collect();
        let mz = fit_circle_2d(&pts, FitObjective::MinZone).unwrap();
        assert!((mz.zone - 2.0 * e).abs() < 1e-9, "MZ zone {}", mz.zone);
        assert!(
            (mz.center.0 - c.0).abs() < 1e-9 && (mz.center.1 - c.1).abs() < 1e-9,
            "MZ centre {:?}",
            mz.center
        );
        let ls = fit_circle_2d(&pts, FitObjective::LeastSquares).unwrap();
        assert!(mz.zone <= ls.zone + 1e-12);
    }

    #[test]
    fn sphere_ls_recovers() {
        let c = Vec3::new(0.5, -1.0, 2.0);
        let r = 2.0;
        let mut pts = Vec::new();
        for i in 0..8 {
            for j in 1..8 {
                let phi = core::f64::consts::TAU * i as f64 / 8.0;
                let theta = core::f64::consts::PI * j as f64 / 8.0;
                let e = if (i + j) % 2 == 0 { 0.01 } else { -0.01 };
                let ri = r + e;
                pts.push(
                    c + Vec3::new(
                        ri * theta.sin() * phi.cos(),
                        ri * theta.sin() * phi.sin(),
                        ri * theta.cos(),
                    ),
                );
            }
        }
        let f = fit_sphere(&pts, FitObjective::LeastSquares).unwrap();
        assert!((f.center - c).norm() < 1e-3, "centre {:?}", f.center);
        assert!((f.radius - r).abs() < 1e-3, "r {}", f.radius);
        assert!((f.zone - 0.02).abs() < 1e-6, "zone {}", f.zone);
        // The gauges decline for spheres in this milestone.
        assert!(fit_sphere(&pts, FitObjective::MinZone).is_err());
    }
}
