//! Canonical geometry recovery (M8): recognize the analytic surface or
//! curve hidden inside a NURBS, and replace it with the exact primitive
//! plus a certified max-deviation bound. This is the "simplify" phase of
//! a healing pipeline (ACIS HEAL, OCCT `ShapeAnalysis_CanonicalRecognition`)
//! and the gate that decides the affordability gradient: recover to exact
//! where a conic/quadric is hiding, KEEP the spline where the geometry is
//! genuinely free-form (research file 24).
//!
//! Pipeline: recognize (curvature signature) -> fit (analytic params) ->
//! certify (max-deviation upper estimate) -> caller substitutes. The
//! recover/keep decision is gated on the certified deviation versus the
//! active tolerance; the "keep" verdict is itself the certificate that a
//! surface is genuinely tolerant.

use crate::curve::{Circle3, Curve3, Line3};
use crate::nurbs_curve::NurbsCurve;
use crate::nurbs_surface::NurbsSurface;
use crate::surface::{Cone3, Cylinder3, Frame3, Plane3, Sphere3, Surface3, Torus3};
use keel_math::vec::Vec3;

/// Eigenvector of the smallest eigenvalue of a 3x3 symmetric matrix, by
/// cyclic Jacobi rotation. Used to recover a surface-of-revolution axis
/// from the point covariance (the axis is the symmetry direction).
#[allow(clippy::needless_range_loop)]
pub(crate) fn smallest_eigenvector_sym3(mut a: [[f64; 3]; 3]) -> Vec3 {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        // Largest off-diagonal.
        let (mut p, mut q, mut max) = (0, 1, 0.0);
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            if a[i][j].abs() > max {
                max = a[i][j].abs();
                p = i;
                q = j;
            }
        }
        if max < 1e-14 {
            break;
        }
        let theta = 0.5 * (a[q][q] - a[p][p]) / a[p][q];
        let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        // Apply rotation J^T A J and accumulate V J.
        for k in 0..3 {
            let (akp, akq) = (a[k][p], a[k][q]);
            a[k][p] = c * akp - s * akq;
            a[k][q] = s * akp + c * akq;
        }
        for k in 0..3 {
            let (apk, aqk) = (a[p][k], a[q][k]);
            a[p][k] = c * apk - s * aqk;
            a[q][k] = s * apk + c * aqk;
        }
        for k in 0..3 {
            let (vkp, vkq) = (v[k][p], v[k][q]);
            v[k][p] = c * vkp - s * vkq;
            v[k][q] = s * vkp + c * vkq;
        }
    }
    let diag = [a[0][0], a[1][1], a[2][2]];
    let mut imin = 0;
    for i in 1..3 {
        if diag[i] < diag[imin] {
            imin = i;
        }
    }
    Vec3::new(v[0][imin], v[1][imin], v[2][imin])
        .try_normalize()
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0))
}

/// Solve a 3x3 system by padding into the 4x4 solver (identity 4th
/// row/col), returning the leading 3 unknowns.
pub(crate) fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<Vec3> {
    let mut a4 = [[0.0f64; 4]; 4];
    let mut b4 = [0.0f64; 4];
    for r in 0..3 {
        a4[r][..3].copy_from_slice(&a[r]);
        b4[r] = b[r];
    }
    a4[3][3] = 1.0;
    let s = solve4(a4, b4)?;
    Some(Vec3::new(s[0], s[1], s[2]))
}

/// Solve a 4x4 linear system by Gaussian elimination with partial
/// pivoting. Returns None if singular.
pub(crate) fn solve4(mut a: [[f64; 4]; 4], mut b: [f64; 4]) -> Option<[f64; 4]> {
    for col in 0..4 {
        // Pivot.
        let mut piv = col;
        for r in (col + 1)..4 {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        for r in 0..4 {
            if r == col {
                continue;
            }
            let f = a[r][col] / a[col][col];
            #[allow(clippy::needless_range_loop)]
            for c in col..4 {
                a[r][c] -= f * a[col][c];
            }
            b[r] -= f * b[col];
        }
    }
    Some([
        b[0] / a[0][0],
        b[1] / a[1][1],
        b[2] / a[2][2],
        b[3] / a[3][3],
    ])
}

/// A recovered analytic surface and the certified upper bound on its max
/// deviation from the source spline.
#[derive(Clone, Debug)]
pub struct SurfaceRecovery {
    pub surface: Surface3,
    pub deviation: f64,
}

/// Sample `(u, v, point)` on an `n x n` interior grid of the surface
/// domain, nudged off the exact boundary so pole-degenerate
/// parameterizations (revolved spheres) stay evaluable.
fn sample_grid(nurbs: &NurbsSurface, n: usize) -> Vec<(f64, f64, Vec3)> {
    let ((u0, u1), (v0, v1)) = nurbs.domain();
    let mut out = Vec::with_capacity(n * n);
    let eps = 1e-6;
    for i in 0..n {
        let fu = (i as f64 + 0.5) / n as f64;
        let u = u0 + (u1 - u0) * (eps + fu * (1.0 - 2.0 * eps));
        for j in 0..n {
            let fv = (j as f64 + 0.5) / n as f64;
            let v = v0 + (v1 - v0) * (eps + fv * (1.0 - 2.0 * eps));
            out.push((u, v, nurbs.point(u, v)));
        }
    }
    out
}

/// A certified upper *estimate* of the max deviation from the spline to
/// the candidate analytic: dense sampling against the analytic's
/// closed-form distance, then local refinement around the worst sample
/// (research file 24, Theme 5: the cheap certifier the literature
/// endorses; point-to-quadric distance is closed form). NOTE: this is a
/// dense-sampled bound with adaptive local refinement, not yet a formal
/// interval certificate -- the interval-certified version is deferred
/// (see the plan ledger), and `fuzz_recover` cross-checks this estimate
/// against an independent denser sampling.
pub fn surface_deviation(nurbs: &NurbsSurface, cand: &Surface3) -> f64 {
    let dist = |p: Vec3| {
        cand.project(p)
            .map(|pr| pr.distance)
            .unwrap_or(f64::INFINITY)
    };
    // Coarse pass.
    let mut worst = (0.0f64, 0.0f64, 0.0f64); // (u, v, dist)
    for (u, v, p) in sample_grid(nurbs, 32) {
        let d = dist(p);
        if d > worst.2 {
            worst = (u, v, d);
        }
    }
    // Local refinement: shrink a window around the worst sample twice.
    let ((u0, u1), (v0, v1)) = nurbs.domain();
    let mut hw_u = (u1 - u0) / 32.0;
    let mut hw_v = (v1 - v0) / 32.0;
    let mut best = worst;
    for _ in 0..3 {
        let (cu, cv) = (best.0, best.1);
        for i in 0..9 {
            for j in 0..9 {
                let u = (cu - hw_u + 2.0 * hw_u * i as f64 / 8.0).clamp(u0, u1);
                let v = (cv - hw_v + 2.0 * hw_v * j as f64 / 8.0).clamp(v0, v1);
                let d = dist(nurbs.point(u, v));
                if d > best.2 {
                    best = (u, v, d);
                }
            }
        }
        hw_u *= 0.3;
        hw_v *= 0.3;
    }
    best.2
}

/// Characteristic size of the surface (sample bounding-box diagonal),
/// used to make curvature thresholds dimensionless.
fn extent(samples: &[(f64, f64, Vec3)]) -> f64 {
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(_, _, p) in samples {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    (hi - lo).norm()
}

/// Max `|k1|, |k2|` over interior samples (dimensionless when multiplied
/// by `extent`). Returns None if the surface is degenerate everywhere.
fn max_abs_curvature(nurbs: &NurbsSurface, n: usize) -> Option<(f64, f64)> {
    let ((u0, u1), (v0, v1)) = nurbs.domain();
    let eps = 1e-4;
    let mut kmax = 0.0f64;
    let mut kmin_of_max = 0.0f64; // max over samples of min(|k1|,|k2|)
    let mut any = false;
    for i in 0..n {
        let u = u0 + (u1 - u0) * (eps + (1.0 - 2.0 * eps) * (i as f64 + 0.5) / n as f64);
        for j in 0..n {
            let v = v0 + (v1 - v0) * (eps + (1.0 - 2.0 * eps) * (j as f64 + 0.5) / n as f64);
            if let Ok(lg) = nurbs.local_geometry(u, v) {
                any = true;
                let (a, b) = (lg.k1.abs(), lg.k2.abs());
                kmax = kmax.max(a.max(b));
                kmin_of_max = kmin_of_max.max(a.min(b));
            }
        }
    }
    if any { Some((kmax, kmin_of_max)) } else { None }
}

/// Fit a plane to the sampled points: centroid for position, the
/// surface normal at the domain centre for orientation (all normals are
/// equal on a true plane).
fn fit_plane(nurbs: &NurbsSurface, samples: &[(f64, f64, Vec3)]) -> Option<Surface3> {
    let n = samples.len() as f64;
    if n == 0.0 {
        return None;
    }
    let centroid = samples.iter().fold(Vec3::ZERO, |a, &(_, _, p)| a + p) * (1.0 / n);
    let ((u0, u1), (v0, v1)) = nurbs.domain();
    let lg = nurbs
        .local_geometry(0.5 * (u0 + u1), 0.5 * (v0 + v1))
        .ok()?;
    let frame = Frame3::from_z(centroid, lg.normal).ok()?;
    Some(Surface3::Plane(Plane3::new(frame)))
}

/// Is the surface umbilic (k1 ~ k2) and curved across its samples? True
/// for a sphere (k1 = k2 = 1/r everywhere), false for cylinder/cone
/// (one principal curvature ~0) and plane.
fn is_umbilic(nurbs: &NurbsSurface, size: f64) -> bool {
    let ((u0, u1), (v0, v1)) = nurbs.domain();
    let eps = 1e-4;
    let mut sum_ratio = 0.0;
    let mut count = 0;
    let mut curved = false;
    let n = 6;
    for i in 0..n {
        let u = u0 + (u1 - u0) * (eps + (1.0 - 2.0 * eps) * (i as f64 + 0.5) / n as f64);
        for j in 0..n {
            let v = v0 + (v1 - v0) * (eps + (1.0 - 2.0 * eps) * (j as f64 + 0.5) / n as f64);
            if let Ok(lg) = nurbs.local_geometry(u, v) {
                let kref = lg.k1.abs().max(lg.k2.abs());
                if kref * size > 1e-6 {
                    curved = true;
                    sum_ratio += (lg.k1 - lg.k2).abs() / kref;
                    count += 1;
                }
            }
        }
    }
    curved && count > 0 && (sum_ratio / count as f64) < 0.2
}

/// Algebraic sphere fit (linear in center and `r^2 - |c|^2`) over the
/// sampled points, the faithful-LS warm-up.
fn fit_sphere(samples: &[(f64, f64, Vec3)]) -> Option<Surface3> {
    if samples.len() < 4 {
        return None;
    }
    // Rows a_i = [2x, 2y, 2z, 1], rhs b_i = |p_i|^2.
    let mut ata = [[0.0f64; 4]; 4];
    let mut atb = [0.0f64; 4];
    for &(_, _, p) in samples {
        let row = [2.0 * p.x, 2.0 * p.y, 2.0 * p.z, 1.0];
        let b = p.dot(p);
        for r in 0..4 {
            for c in 0..4 {
                ata[r][c] += row[r] * row[c];
            }
            atb[r] += row[r] * b;
        }
    }
    let sol = solve4(ata, atb)?;
    let center = Vec3::new(sol[0], sol[1], sol[2]);
    let r2 = sol[3] + center.dot(center);
    if !r2.is_finite() || r2 <= 0.0 {
        return None;
    }
    let frame = Frame3::from_z(center, Vec3::new(0.0, 0.0, 1.0)).ok()?;
    Some(Surface3::Sphere(Sphere3 {
        frame,
        radius: r2.sqrt(),
    }))
}

/// An orthonormal pair spanning the plane perpendicular to `axis`.
fn ortho_basis(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.try_normalize().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
    let seed = if a.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = (seed - a * seed.dot(a)).try_normalize().unwrap_or(seed);
    let w = a.cross(u);
    (u, w)
}

/// Sample `(point, ruling_direction)`: the ruling is the principal
/// direction of the smaller-magnitude curvature (the ~0 direction on a
/// developable surface).
fn sample_rulings(nurbs: &NurbsSurface, n: usize) -> Vec<(Vec3, Vec3)> {
    let ((u0, u1), (v0, v1)) = nurbs.domain();
    let eps = 1e-4;
    let mut out = Vec::new();
    for i in 0..n {
        let u = u0 + (u1 - u0) * (eps + (1.0 - 2.0 * eps) * (i as f64 + 0.5) / n as f64);
        for j in 0..n {
            let v = v0 + (v1 - v0) * (eps + (1.0 - 2.0 * eps) * (j as f64 + 0.5) / n as f64);
            if let Ok(lg) = nurbs.local_geometry(u, v) {
                let ruling = if lg.k1.abs() < lg.k2.abs() {
                    lg.dir1
                } else {
                    lg.dir2
                };
                out.push((lg.point, ruling));
            }
        }
    }
    out
}

/// Fit a cylinder: axis = mean ruling direction; radius/center from a 2D
/// circle fit in the perpendicular plane.
fn fit_cylinder(rulings: &[(Vec3, Vec3)], samples: &[(f64, f64, Vec3)]) -> Option<Surface3> {
    if rulings.len() < 3 {
        return None;
    }
    let r0 = rulings[0].1;
    let mut axis = Vec3::ZERO;
    for &(_, t) in rulings {
        axis = axis + t * t.dot(r0).signum();
    }
    let axis = axis.try_normalize()?;
    let (uax, wax) = ortho_basis(axis);
    let origin =
        samples.iter().fold(Vec3::ZERO, |a, &(_, _, p)| a + p) * (1.0 / samples.len() as f64);
    // 2D circle fit in (uax, wax).
    let mut ata = [[0.0f64; 3]; 3];
    let mut atb = [0.0f64; 3];
    for &(_, _, p) in samples {
        let d = p - origin;
        let (x, y) = (d.dot(uax), d.dot(wax));
        let row = [2.0 * x, 2.0 * y, 1.0];
        let b = x * x + y * y;
        for r in 0..3 {
            for c in 0..3 {
                ata[r][c] += row[r] * row[c];
            }
            atb[r] += row[r] * b;
        }
    }
    let sol = solve3(ata, atb)?;
    let (cx, cy) = (sol.x, sol.y);
    let r2 = sol.z + cx * cx + cy * cy;
    if !r2.is_finite() || r2 <= 0.0 {
        return None;
    }
    let center = origin + uax * cx + wax * cy;
    let frame = Frame3::from_z(center, axis).ok()?;
    Some(Surface3::Cylinder(Cylinder3 {
        frame,
        radius: r2.sqrt(),
    }))
}

/// Fit a cone: apex = least-squares intersection of the ruling lines;
/// axis from apex to the point centroid; half-angle from the rulings.
fn fit_cone(rulings: &[(Vec3, Vec3)], samples: &[(f64, f64, Vec3)]) -> Option<Surface3> {
    if rulings.len() < 4 {
        return None;
    }
    // Minimize sum of squared distances from apex to each ruling line:
    // (Sum (I - t t^T)) a = Sum (I - t t^T) p.
    let mut m = [[0.0f64; 3]; 3];
    let mut rhs = Vec3::ZERO;
    for &(p, t) in rulings {
        let t = match t.try_normalize() {
            Some(t) => t,
            None => continue,
        };
        let proj = [
            [1.0 - t.x * t.x, -t.x * t.y, -t.x * t.z],
            [-t.y * t.x, 1.0 - t.y * t.y, -t.y * t.z],
            [-t.z * t.x, -t.z * t.y, 1.0 - t.z * t.z],
        ];
        for r in 0..3 {
            for c in 0..3 {
                m[r][c] += proj[r][c];
            }
        }
        let mp = Vec3::new(
            proj[0][0] * p.x + proj[0][1] * p.y + proj[0][2] * p.z,
            proj[1][0] * p.x + proj[1][1] * p.y + proj[1][2] * p.z,
            proj[2][0] * p.x + proj[2][1] * p.y + proj[2][2] * p.z,
        );
        rhs = rhs + mp;
    }
    let apex = solve3(m, [rhs.x, rhs.y, rhs.z])?;
    let centroid =
        samples.iter().fold(Vec3::ZERO, |a, &(_, _, p)| a + p) * (1.0 / samples.len() as f64);
    let axis = (centroid - apex).try_normalize()?;
    // Half-angle: mean angle between (p - apex) and axis.
    let mut ang = 0.0;
    let mut count = 0;
    for &(_, _, p) in samples {
        if let Some(d) = (p - apex).try_normalize() {
            ang += d.dot(axis).clamp(-1.0, 1.0).acos();
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let half_angle = ang / count as f64;
    if half_angle <= 1e-4 || half_angle >= core::f64::consts::FRAC_PI_2 - 1e-4 {
        return None;
    }
    let frame = Frame3::from_z(apex, axis).ok()?;
    Some(Surface3::Cone(Cone3 {
        frame,
        radius: 0.0,
        half_angle,
    }))
}

/// Fit a torus: axis = the surface-of-revolution symmetry direction
/// (smallest covariance eigenvector); centre = centroid; major/minor
/// radii from the radial extent in the axis frame. The most fragile fit
/// (research file 24); the certifier gates correctness.
fn fit_torus(samples: &[(f64, f64, Vec3)]) -> Option<Surface3> {
    if samples.len() < 8 {
        return None;
    }
    let n = samples.len() as f64;
    let centroid = samples.iter().fold(Vec3::ZERO, |a, &(_, _, p)| a + p) * (1.0 / n);
    let mut cov = [[0.0f64; 3]; 3];
    for &(_, _, p) in samples {
        let d = [p.x - centroid.x, p.y - centroid.y, p.z - centroid.z];
        for r in 0..3 {
            for c in 0..3 {
                cov[r][c] += d[r] * d[c];
            }
        }
    }
    let axis = smallest_eigenvector_sym3(cov);
    // Fit the tube circle in (rho, z) space, where rho is the radial
    // distance from the axis and z the axial coordinate: the tube is a
    // 2D circle centred (major, z0) with radius minor. A linear circle
    // fit (rows [2 rho, 2 z, 1] = rho^2 + z^2).
    let mut ata = [[0.0f64; 3]; 3];
    let mut atb = [0.0f64; 3];
    for &(_, _, p) in samples {
        let d = p - centroid;
        let z = d.dot(axis);
        let rho = (d - axis * z).norm();
        let row = [2.0 * rho, 2.0 * z, 1.0];
        let b = rho * rho + z * z;
        for r in 0..3 {
            for c in 0..3 {
                ata[r][c] += row[r] * row[c];
            }
            atb[r] += row[r] * b;
        }
    }
    let sol = solve3(ata, atb)?;
    let (major, z0) = (sol.x, sol.y);
    let minor2 = sol.z + major * major + z0 * z0;
    if !major.is_finite() || major <= 0.0 || minor2 <= 0.0 || !minor2.is_finite() {
        return None;
    }
    let minor = minor2.sqrt();
    // Shift the centre to the torus equatorial plane (axial offset z0).
    let center = centroid + axis * z0;
    let frame = Frame3::from_z(center, axis).ok()?;
    Some(Surface3::Torus(Torus3 {
        frame,
        major,
        minor,
    }))
}

/// Recognize -> fit -> certify a NURBS surface against the analytic
/// family. Returns `Some(recovery)` only when the certified deviation is
/// within `tol`; `None` means keep the spline (genuinely free-form, or
/// an analytic that does not certify within tolerance). The certifier is
/// the real discriminator: a wrong-type fit will not certify.
///
/// M8 covers plane, sphere, cylinder, cone, torus.
pub fn recover_surface(nurbs: &NurbsSurface, tol: f64) -> Option<SurfaceRecovery> {
    let samples = sample_grid(nurbs, 8);
    let size = extent(&samples).max(1e-12);
    let (kmax, kmin) = max_abs_curvature(nurbs, 6)?;
    let accept = |cand: Surface3| -> Option<SurfaceRecovery> {
        let dev = surface_deviation(nurbs, &cand);
        (dev <= tol).then_some(SurfaceRecovery {
            surface: cand,
            deviation: dev,
        })
    };

    // Plane: both principal curvatures vanish.
    if kmax * size < 1e-6 {
        return fit_plane(nurbs, &samples).and_then(accept);
    }

    // Developable: one principal curvature ~0 (Gaussian ~0). Try
    // cylinder then cone; the certifier selects.
    if kmin * size < 1e-5 {
        let rulings = sample_rulings(nurbs, 10);
        let dense = sample_grid(nurbs, 12);
        if let Some(rec) = fit_cylinder(&rulings, &dense).and_then(&accept) {
            return Some(rec);
        }
        if let Some(rec) = fit_cone(&rulings, &dense).and_then(&accept) {
            return Some(rec);
        }
        return None;
    }

    // Sphere: umbilic and curved.
    if is_umbilic(nurbs, size) {
        let dense = sample_grid(nurbs, 12);
        return fit_sphere(&dense).and_then(accept);
    }

    // Torus: doubly-curved, not umbilic, not developable.
    let dense = sample_grid(nurbs, 16);
    fit_torus(&dense).and_then(accept)
}

// ---------------------------------------------------------------------
// Curve recovery (NURBS curve -> line / circle). Ellipse recovery is
// noted for later (plane-cylinder seams); line and circle are what the
// analytic booleans actually produce (plane-plane -> line, sphere/plane
// quadric pairs -> circle).
// ---------------------------------------------------------------------

/// A recovered analytic curve and the certified upper bound on its max
/// deviation from the source spline.
#[derive(Clone, Debug)]
pub struct CurveRecovery {
    pub curve: Curve3,
    pub deviation: f64,
}

fn sample_curve(nurbs: &NurbsCurve, n: usize) -> Vec<Vec3> {
    let (t0, t1) = nurbs.domain();
    let eps = 1e-6;
    (0..n)
        .map(|i| {
            let f = (i as f64 + 0.5) / n as f64;
            nurbs.point(t0 + (t1 - t0) * (eps + f * (1.0 - 2.0 * eps)))
        })
        .collect()
}

fn dist_to_curve(c: &Curve3, p: Vec3) -> f64 {
    match c {
        Curve3::Line(l) => (p - l.point(l.project(p))).norm(),
        Curve3::Circle(ci) => (p - ci.point(ci.project(p))).norm(),
        Curve3::Ellipse(e) => (p - e.point(e.project(p))).norm(),
        Curve3::Nurbs(_) => f64::INFINITY,
    }
}

/// Dense-sampled max deviation of the spline curve from the candidate
/// analytic (the curve analogue of `surface_deviation`).
pub fn curve_deviation(nurbs: &NurbsCurve, cand: &Curve3) -> f64 {
    let (t0, t1) = nurbs.domain();
    let n = 200;
    let mut worst = 0.0f64;
    for i in 0..n {
        let t = t0 + (t1 - t0) * (0.001 + 0.998 * i as f64 / (n - 1) as f64);
        worst = worst.max(dist_to_curve(cand, nurbs.point(t)));
    }
    worst
}

/// Max curvature kappa = |r' x r''| / |r'|^3 and its variation over
/// interior samples; returns (max_kappa, kappa_range).
fn curve_curvature(nurbs: &NurbsCurve, n: usize) -> Option<(f64, f64, f64)> {
    let (t0, t1) = nurbs.domain();
    let eps = 1e-4;
    let mut ks = Vec::new();
    for i in 0..n {
        let t = t0 + (t1 - t0) * (eps + (1.0 - 2.0 * eps) * (i as f64 + 0.5) / n as f64);
        let d = nurbs.derivatives(t, 2);
        let (d1, d2) = (d[1], d[2]);
        let s = d1.norm();
        if s > 1e-12 {
            ks.push(d1.cross(d2).norm() / (s * s * s));
        }
    }
    if ks.is_empty() {
        return None;
    }
    let kmax = ks.iter().cloned().fold(0.0f64, f64::max);
    let kmin = ks.iter().cloned().fold(f64::INFINITY, f64::min);
    let mean = ks.iter().sum::<f64>() / ks.len() as f64;
    Some((kmax, kmax - kmin, mean))
}

fn fit_line(pts: &[Vec3]) -> Option<Curve3> {
    let p0 = *pts.first()?;
    let p1 = *pts.last()?;
    Line3::new(p0, p1 - p0).ok().map(Curve3::Line)
}

/// Fit a circle through 3D coplanar points: plane from three spread
/// points, then a 2D circle fit in that plane.
fn fit_circle(pts: &[Vec3]) -> Option<Curve3> {
    if pts.len() < 3 {
        return None;
    }
    let a = pts[0];
    let b = pts[pts.len() / 2];
    let c = pts[pts.len() - 1];
    let normal = (b - a).cross(c - a).try_normalize()?;
    let (ux, uy) = ortho_basis(normal);
    let origin = pts.iter().fold(Vec3::ZERO, |s, &p| s + p) * (1.0 / pts.len() as f64);
    let mut ata = [[0.0f64; 3]; 3];
    let mut atb = [0.0f64; 3];
    for &p in pts {
        let d = p - origin;
        let (x, y) = (d.dot(ux), d.dot(uy));
        let row = [2.0 * x, 2.0 * y, 1.0];
        let rhs = x * x + y * y;
        for r in 0..3 {
            for cc in 0..3 {
                ata[r][cc] += row[r] * row[cc];
            }
            atb[r] += row[r] * rhs;
        }
    }
    let sol = solve3(ata, atb)?;
    let (cx, cy) = (sol.x, sol.y);
    let r2 = sol.z + cx * cx + cy * cy;
    if !r2.is_finite() || r2 <= 0.0 {
        return None;
    }
    let center = origin + ux * cx + uy * cy;
    Circle3::new(center, ux, uy, r2.sqrt())
        .ok()
        .map(Curve3::Circle)
}

/// Recognize -> fit -> certify a NURBS curve as a line or circle. None =
/// keep the spline.
pub fn recover_curve(nurbs: &NurbsCurve, tol: f64) -> Option<CurveRecovery> {
    let pts = sample_curve(nurbs, 16);
    if pts.len() < 2 {
        return None;
    }
    let mut lo = pts[0];
    let mut hi = pts[0];
    for &p in &pts {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let size = (hi - lo).norm().max(1e-12);
    let (kmax, krange, _mean) = curve_curvature(nurbs, 12)?;
    let accept = |cand: Curve3| -> Option<CurveRecovery> {
        let dev = curve_deviation(nurbs, &cand);
        (dev <= tol).then_some(CurveRecovery {
            curve: cand,
            deviation: dev,
        })
    };

    // Line: curvature vanishes.
    if kmax * size < 1e-6 {
        return fit_line(&pts).and_then(accept);
    }
    // Circle: curvature roughly constant and nonzero.
    if krange * size < 1e-3 {
        return fit_circle(&sample_curve(nurbs, 24)).and_then(accept);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nurbs_surface::NurbsSurface;

    /// A flat bilinear NURBS patch (a tilted parallelogram) -- a plane in
    /// disguise.
    fn tilted_plane_patch() -> NurbsSurface {
        // z = 0.5*x + 0.25*y over [0,2]x[0,3]; corners of the bilinear net.
        let pts = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.75),
            Vec3::new(2.0, 0.0, 1.0),
            Vec3::new(2.0, 3.0, 1.75),
        ];
        NurbsSurface::new(1, 1, vec![0., 0., 1., 1.], vec![0., 0., 1., 1.], pts, None).unwrap()
    }

    /// A genuine exact NURBS sphere: revolve a rational quadratic
    /// semicircle meridian about z (the same construction the kernel's
    /// `nurbs_sphere` primitive uses).
    fn nurbs_sphere_surface(center: Vec3, r: f64) -> NurbsSurface {
        use crate::nurbs_curve::NurbsCurve;
        use crate::nurbs_surface::revolve_full;
        let s = core::f64::consts::FRAC_1_SQRT_2;
        let az = Vec3::new(0.0, 0.0, 1.0);
        let fx = Vec3::new(1.0, 0.0, 0.0);
        let ctrl = vec![
            center - az * r,
            center - az * r + fx * r,
            center + fx * r,
            center + az * r + fx * r,
            center + az * r,
        ];
        let profile = NurbsCurve::new(
            2,
            vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
            ctrl,
            Some(vec![1.0, s, 1.0, s, 1.0]),
        )
        .unwrap();
        revolve_full(&profile, center, az).unwrap()
    }

    /// A genuine NURBS cylinder: revolve a vertical line segment offset
    /// by the radius about z.
    fn nurbs_cylinder_surface(r: f64, half_h: f64) -> NurbsSurface {
        use crate::nurbs_curve::NurbsCurve;
        use crate::nurbs_surface::revolve_full;
        let az = Vec3::new(0.0, 0.0, 1.0);
        let ctrl = vec![Vec3::new(r, 0.0, -half_h), Vec3::new(r, 0.0, half_h)];
        let profile = NurbsCurve::new(1, vec![0., 0., 1., 1.], ctrl, None).unwrap();
        revolve_full(&profile, Vec3::ZERO, az).unwrap()
    }

    #[test]
    fn plane_recovers() {
        let s = tilted_plane_patch();
        let rec = recover_surface(&s, 1e-6).expect("tilted plane must recover");
        assert!(
            matches!(rec.surface, Surface3::Plane(_)),
            "expected a plane"
        );
        assert!(
            rec.deviation < 1e-9,
            "plane deviation {} too large",
            rec.deviation
        );
        let p = s.point(0.5, 0.5);
        let d = rec.surface.project(p).unwrap().distance;
        assert!(
            d <= rec.deviation + 1e-12,
            "sample off recovered plane: {d}"
        );
    }

    #[test]
    fn sphere_recovers_exactly() {
        let center = Vec3::new(0.3, -0.7, 1.1);
        let r = 1.4;
        let s = nurbs_sphere_surface(center, r);
        let rec = recover_surface(&s, 1e-6).expect("nurbs sphere must recover");
        match rec.surface {
            Surface3::Sphere(sph) => {
                assert!(
                    (sph.frame.origin - center).norm() < 1e-7,
                    "recovered center {:?} vs {:?}",
                    sph.frame.origin,
                    center
                );
                assert!(
                    (sph.radius - r).abs() < 1e-7,
                    "recovered radius {} vs {r}",
                    sph.radius
                );
            }
            other => panic!("expected a sphere, got {other:?}"),
        }
        assert!(
            rec.deviation < 1e-6,
            "sphere deviation {} too large",
            rec.deviation
        );
    }

    #[test]
    fn sphere_does_not_recover_as_plane() {
        let s = nurbs_sphere_surface(Vec3::ZERO, 1.0);
        let rec = recover_surface(&s, 1e-6).expect("sphere recovers");
        assert!(
            !matches!(rec.surface, Surface3::Plane(_)),
            "sphere wrongly recovered as a plane"
        );
    }

    /// A genuine NURBS cone: revolve a slanted line from the apex about z.
    fn nurbs_cone_surface(half_angle: f64, height: f64) -> NurbsSurface {
        use crate::nurbs_curve::NurbsCurve;
        use crate::nurbs_surface::revolve_full;
        let az = Vec3::new(0.0, 0.0, 1.0);
        let r_top = height * half_angle.tan();
        let ctrl = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(r_top, 0.0, height)];
        let profile = NurbsCurve::new(1, vec![0., 0., 1., 1.], ctrl, None).unwrap();
        revolve_full(&profile, Vec3::ZERO, az).unwrap()
    }

    /// A genuine NURBS torus: revolve a full rational-quadratic tube
    /// circle (centre (R,0,0), radius r, in the xz-plane) about z.
    fn nurbs_torus_surface(major: f64, minor: f64) -> NurbsSurface {
        use crate::nurbs_curve::NurbsCurve;
        use crate::nurbs_surface::revolve_full;
        let s = core::f64::consts::FRAC_1_SQRT_2;
        let (rr, r) = (major, minor);
        let ctrl = vec![
            Vec3::new(rr + r, 0., 0.),
            Vec3::new(rr + r, 0., r),
            Vec3::new(rr, 0., r),
            Vec3::new(rr - r, 0., r),
            Vec3::new(rr - r, 0., 0.),
            Vec3::new(rr - r, 0., -r),
            Vec3::new(rr, 0., -r),
            Vec3::new(rr + r, 0., -r),
            Vec3::new(rr + r, 0., 0.),
        ];
        let profile = NurbsCurve::new(
            2,
            vec![0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.],
            ctrl,
            Some(vec![1., s, 1., s, 1., s, 1., s, 1.]),
        )
        .unwrap();
        revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap()
    }

    #[test]
    fn torus_recovers() {
        let (rr, r) = (3.0, 0.8);
        let s = nurbs_torus_surface(rr, r);
        let rec = recover_surface(&s, 1e-5).expect("nurbs torus must recover");
        match rec.surface {
            Surface3::Torus(t) => {
                assert!(
                    (t.major - rr).abs() < 1e-3,
                    "torus major {} vs {rr}",
                    t.major
                );
                assert!((t.minor - r).abs() < 1e-3, "torus minor {} vs {r}", t.minor);
            }
            other => panic!("expected a torus, got {other:?}"),
        }
        assert!(rec.deviation < 1e-5, "torus deviation {}", rec.deviation);
    }

    #[test]
    fn cylinder_recovers() {
        let s = nurbs_cylinder_surface(1.3, 2.0);
        let rec = recover_surface(&s, 1e-6).expect("nurbs cylinder must recover");
        match rec.surface {
            Surface3::Cylinder(cy) => {
                assert!(
                    (cy.radius - 1.3).abs() < 1e-6,
                    "cyl radius {} vs 1.3",
                    cy.radius
                );
                let axis = cy.frame.z.try_normalize().unwrap();
                assert!(
                    axis.cross(Vec3::new(0.0, 0.0, 1.0)).norm() < 1e-6,
                    "cyl axis not z: {axis:?}"
                );
            }
            other => panic!("expected a cylinder, got {other:?}"),
        }
        assert!(rec.deviation < 1e-6, "cyl deviation {}", rec.deviation);
    }

    #[test]
    fn cone_recovers() {
        let ha = 0.4;
        let s = nurbs_cone_surface(ha, 2.0);
        // The pole-degenerate apex limits the dense-sampled certifier to
        // ~2e-6; recover at a realistic modeling tolerance. The fit
        // itself is essentially exact (half-angle to ~1e-12).
        let rec = recover_surface(&s, 1e-5).expect("nurbs cone must recover");
        match rec.surface {
            Surface3::Cone(co) => {
                assert!(
                    (co.half_angle - ha).abs() < 1e-4,
                    "cone half-angle {} vs {ha}",
                    co.half_angle
                );
                assert!(
                    (co.frame.origin).norm() < 1e-5,
                    "cone apex {:?} not origin",
                    co.frame.origin
                );
            }
            other => panic!("expected a cone, got {other:?}"),
        }
        assert!(rec.deviation < 1e-5, "cone deviation {}", rec.deviation);
    }

    /// A genuinely free-form bicubic NURBS patch (irregular bumps): no
    /// analytic primitive fits it within tolerance.
    fn freeform_patch() -> NurbsSurface {
        let mut pts = Vec::new();
        // 4x4 control net with irregular z bumps -> doubly-curved, not
        // any quadric.
        let z = [
            [0.0, 0.3, -0.2, 0.1],
            [0.25, 0.9, 0.4, -0.3],
            [-0.15, 0.5, 1.1, 0.2],
            [0.05, -0.4, 0.3, 0.8],
        ];
        for (i, zr) in z.iter().enumerate() {
            for (j, &zz) in zr.iter().enumerate() {
                pts.push(Vec3::new(i as f64, j as f64, zz));
            }
        }
        NurbsSurface::new(
            3,
            3,
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
            pts,
            None,
        )
        .unwrap()
    }

    fn nurbs_segment(p0: Vec3, p1: Vec3) -> NurbsCurve {
        NurbsCurve::new(1, vec![0., 0., 1., 1.], vec![p0, p1], None).unwrap()
    }

    fn nurbs_quarter_circle(r: f64) -> NurbsCurve {
        let s = core::f64::consts::FRAC_1_SQRT_2;
        NurbsCurve::new(
            2,
            vec![0., 0., 0., 1., 1., 1.],
            vec![
                Vec3::new(r, 0., 0.),
                Vec3::new(r, r, 0.),
                Vec3::new(0., r, 0.),
            ],
            Some(vec![1.0, s, 1.0]),
        )
        .unwrap()
    }

    #[test]
    fn line_recovers() {
        let p0 = Vec3::new(1.0, 2.0, -1.0);
        let p1 = Vec3::new(4.0, -2.0, 3.0);
        let rec = recover_curve(&nurbs_segment(p0, p1), 1e-6).expect("line recovers");
        match rec.curve {
            Curve3::Line(l) => {
                let want = (p1 - p0).try_normalize().unwrap();
                assert!(l.dir.cross(want).norm() < 1e-9, "line dir {:?}", l.dir);
            }
            other => panic!("expected a line, got {other:?}"),
        }
    }

    #[test]
    fn circle_recovers() {
        let r = 1.7;
        let rec = recover_curve(&nurbs_quarter_circle(r), 1e-6).expect("circle recovers");
        match rec.curve {
            Curve3::Circle(c) => {
                assert!(
                    c.center.norm() < 1e-7,
                    "circle center {:?} not origin",
                    c.center
                );
                assert!(
                    (c.radius - r).abs() < 1e-7,
                    "circle radius {} vs {r}",
                    c.radius
                );
            }
            other => panic!("expected a circle, got {other:?}"),
        }
        assert!(rec.deviation < 1e-6, "circle deviation {}", rec.deviation);
    }

    #[test]
    fn wavy_curve_is_kept() {
        // A degree-3 wiggly curve is no line or circle.
        let pts = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(2.0, -1.0, 0.5),
            Vec3::new(3.0, 0.8, -0.3),
            Vec3::new(4.0, -0.5, 0.2),
        ];
        let c = NurbsCurve::new(3, vec![0., 0., 0., 0., 0.5, 1., 1., 1., 1.], pts, None).unwrap();
        assert!(
            recover_curve(&c, 1e-6).is_none(),
            "wavy curve wrongly recovered"
        );
    }

    #[test]
    fn freeform_is_kept() {
        // The "keep the spline" verdict: a genuine free-form surface must
        // NOT recover to any analytic within a tight tolerance. This is
        // the certificate that the geometry is genuinely tolerant (the
        // other side of the affordability gradient).
        let s = freeform_patch();
        assert!(
            recover_surface(&s, 1e-6).is_none(),
            "free-form patch wrongly recovered as an analytic"
        );
    }

    #[test]
    fn cylinder_not_misrecovered_as_sphere() {
        let s = nurbs_cylinder_surface(1.0, 2.0);
        let rec = recover_surface(&s, 1e-6).expect("cylinder recovers");
        assert!(
            !matches!(rec.surface, Surface3::Sphere(_)),
            "cylinder wrongly recovered as a sphere"
        );
    }
}
