//! Least-squares B-spline fitting with verified deviation (M5a shared
//! fitter: SSI branch curves and pcurves in every tier flow through
//! this). End-interpolating cubic fits, knots by averaged parameters
//! (NURBS Book 9.2 style), refinement by control-count escalation
//! until the sampled max deviation (inflated by a safety factor for
//! between-sample excursions) meets tolerance.

use crate::GeomError;
use crate::knots::KnotVector;
use crate::nurbs_curve::NurbsCurve;
use keel_math::vec::Vec3;

/// Result of a verified fit.
pub struct FitResult {
    pub curve: NurbsCurve,
    /// Sampled max deviation times the safety factor.
    pub tol_achieved: f64,
}

/// Fit a cubic B-spline through `points` (>= 4, ordered along the
/// branch), end-interpolating, within `tol` if possible. Control
/// count escalates from 4 to points.len().min(64); the best fit is
/// returned even when `tol` is not met (callers read tol_achieved).
pub fn fit_cubic(points: &[Vec3], tol: f64) -> Result<FitResult, GeomError> {
    if points.len() < 2 || points.iter().any(|p| !p.is_finite()) {
        return Err(GeomError::Degenerate);
    }
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    // Degenerate-short inputs: a line segment.
    if points.len() < 4 {
        let c = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![points[0], points[points.len() - 1]],
            None,
        )?;
        let dev = sampled_deviation(&c, points);
        return Ok(FitResult {
            curve: c,
            tol_achieved: dev * SAFETY,
        });
    }
    // Chord-length parameterization.
    let params = chord_params(points);
    let mut best: Option<FitResult> = None;
    let mut n_ctrl = 4usize;
    let cap = points.len().min(64);
    loop {
        if let Ok(curve) = lsq_fit(points, &params, n_ctrl) {
            let dev = sampled_deviation(&curve, points) * SAFETY;
            let better = best.as_ref().is_none_or(|b| dev < b.tol_achieved);
            if better {
                best = Some(FitResult {
                    curve,
                    tol_achieved: dev,
                });
            }
            if best.as_ref().is_some_and(|b| b.tol_achieved <= tol) {
                break;
            }
        }
        if n_ctrl >= cap {
            break;
        }
        n_ctrl = (n_ctrl * 2).min(cap);
    }
    best.ok_or(GeomError::Degenerate)
}

/// Inflation for between-sample excursions; honest-but-not-formal
/// (the formal Hausdorff enclosure is a staged upgrade, see plan).
const SAFETY: f64 = 2.0;

fn chord_params(points: &[Vec3]) -> Vec<f64> {
    let mut t = vec![0.0; points.len()];
    let mut acc = 0.0;
    for i in 1..points.len() {
        acc += (points[i] - points[i - 1]).norm();
        t[i] = acc;
    }
    if acc <= 0.0 {
        // Coincident samples: spread uniformly.
        for (i, ti) in t.iter_mut().enumerate() {
            *ti = i as f64 / (points.len() - 1) as f64;
        }
    } else {
        for ti in t.iter_mut() {
            *ti /= acc;
        }
    }
    t
}

/// End-interpolating least-squares cubic fit with n_ctrl controls.
/// (Also the building block of the foreign-geometry surface fit.)
pub(crate) fn lsq_fit(
    points: &[Vec3],
    params: &[f64],
    n_ctrl: usize,
) -> Result<NurbsCurve, GeomError> {
    let p = 3usize.min(n_ctrl - 1);
    // Averaged interior knots from the parameters (NURBS Book 9.68).
    let n_interior = n_ctrl - p - 1;
    let mut knots = vec![0.0; p + 1];
    for j in 1..=n_interior {
        // Average of parameter window.
        let d = points.len() as f64 / (n_interior + 1) as f64;
        let pos = j as f64 * d;
        let i = pos.floor() as usize;
        let alpha = pos - i as f64;
        let idx = i.clamp(1, points.len() - 1);
        let val = (1.0 - alpha) * params[idx - 1] + alpha * params[idx.min(params.len() - 1)];
        knots.push(val.clamp(0.0, 1.0));
    }
    knots.extend(vec![1.0; p + 1]);
    // Enforce strictly nondecreasing interior knots.
    for i in (p + 1)..(knots.len() - p - 1) {
        if knots[i] < knots[i - 1] {
            knots[i] = knots[i - 1];
        }
    }
    let kv = KnotVector::new(p, knots.clone())?;
    // Basis matrix rows.
    let n = points.len();
    let m = n_ctrl;
    let mut a = vec![vec![0.0f64; m]; n];
    for (row, &t) in params.iter().enumerate() {
        let span = kv.find_span(t);
        let funs = crate::basis::basis_funs(&kv, span, t);
        for (k, &f) in funs.iter().enumerate().take(p + 1) {
            let col = span - p + k;
            if col < m {
                a[row][col] = f;
            }
        }
    }
    // Normal equations with end-point interpolation: pin first/last
    // controls to the end samples and solve for the interior.
    let interior = m - 2;
    if interior == 0 {
        return NurbsCurve::new(p, knots, vec![points[0], points[n - 1]], None);
    }
    let mut ata = vec![vec![0.0f64; interior]; interior];
    let mut atb = vec![[0.0f64; 3]; interior];
    for row in 0..n {
        for i in 0..interior {
            let ai = a[row][i + 1];
            if ai == 0.0 {
                continue;
            }
            for j in 0..interior {
                ata[i][j] += ai * a[row][j + 1];
            }
            let rhs = points[row] - points[0] * a[row][0] - points[n - 1] * a[row][m - 1];
            atb[i][0] += ai * rhs.x;
            atb[i][1] += ai * rhs.y;
            atb[i][2] += ai * rhs.z;
        }
    }
    let sol = gauss_solve(&mut ata, &mut atb).ok_or(GeomError::Degenerate)?;
    let mut ctrl = Vec::with_capacity(m);
    ctrl.push(points[0]);
    for s in sol {
        ctrl.push(Vec3::new(s[0], s[1], s[2]));
    }
    ctrl.push(points[n - 1]);
    NurbsCurve::new(p, knots, ctrl, None)
}

/// Gaussian elimination with partial pivoting; rhs has 3 columns.
fn gauss_solve(a: &mut [Vec<f64>], b: &mut [[f64; 3]]) -> Option<Vec<[f64; 3]>> {
    let n = a.len();
    for col in 0..n {
        // Pivot.
        let mut piv = col;
        for r in (col + 1)..n {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col].abs() < 1e-300 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let d = a[col][col];
        let pivot_row = a[col].clone();
        let pivot_b = b[col];
        for r in (col + 1)..n {
            let f = a[r][col] / d;
            if f == 0.0 {
                continue;
            }
            for (val, pv) in a[r].iter_mut().zip(&pivot_row).skip(col) {
                *val -= f * pv;
            }
            for (val, pv) in b[r].iter_mut().zip(pivot_b) {
                *val -= f * pv;
            }
        }
    }
    let mut out = vec![[0.0f64; 3]; n];
    for col in (0..n).rev() {
        for k in 0..3 {
            let mut acc = b[col][k];
            for c in (col + 1)..n {
                acc -= a[col][c] * out[c][k];
            }
            out[col][k] = acc / a[col][col];
        }
    }
    Some(out)
}

/// Max distance from each input sample to the fitted curve (global
/// projection per sample).
fn sampled_deviation(curve: &NurbsCurve, points: &[Vec3]) -> f64 {
    points
        .iter()
        .map(|&p| crate::project::project_point(curve, p, 1e-5).distance)
        .fold(0.0f64, f64::max)
}

/// Compute a pcurve: invert a 3D curve (known to lie on the analytic
/// surface) into the surface's (u, v) domain and fit a 2D pcurve in
/// (u, v, 0) space. `period` lets callers request seam-consistent
/// unwrapping for periodic directions (None = no unwrap).
pub fn pcurve_on_analytic(
    curve: &crate::curve::Curve3,
    surface: &crate::surface::Surface3,
    n: usize,
    tol: f64,
) -> Result<FitResult, GeomError> {
    use crate::curve::Curve3;
    let sample3 = |t: f64| -> Vec3 {
        match curve {
            Curve3::Line(l) => l.point(t),
            Curve3::Circle(c) => c.point(core::f64::consts::TAU * t),
            Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
            Curve3::Nurbs(nb) => {
                let (a, b) = nb.domain();
                nb.point(a + t * (b - a))
            }
        }
    };
    let mut uvs: Vec<Vec3> = Vec::with_capacity(n + 1);
    let mut prev_u: Option<f64> = None;
    for k in 0..=n {
        let p = sample3(k as f64 / n as f64);
        let pr = surface.project(p)?;
        // Unwrap u across the periodic seam so the pcurve is continuous.
        let mut u = pr.u;
        if let Some(pu) = prev_u {
            let tau = core::f64::consts::TAU;
            while u - pu > tau * 0.5 {
                u -= tau;
            }
            while pu - u > tau * 0.5 {
                u += tau;
            }
        }
        prev_u = Some(u);
        uvs.push(Vec3::new(u, pr.v, 0.0));
    }
    fit_cubic(&uvs, tol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_circle_samples_within_tolerance() {
        let pts: Vec<Vec3> = (0..=40)
            .map(|i| {
                let a = core::f64::consts::PI * i as f64 / 40.0;
                Vec3::new(2.0 * a.cos(), 2.0 * a.sin(), 0.5)
            })
            .collect();
        let fit = fit_cubic(&pts, 1e-6).unwrap();
        assert!(fit.tol_achieved <= 1e-6, "tol {}", fit.tol_achieved);
        // Fitted curve stays near the true circle between samples too.
        for k in 0..=100 {
            let (a, b) = fit.curve.domain();
            let t = a + (b - a) * k as f64 / 100.0;
            let p = fit.curve.point(t);
            assert!(
                ((p.x * p.x + p.y * p.y).sqrt() - 2.0).abs() < 1e-4,
                "off-circle at t={t}"
            );
        }
    }

    #[test]
    fn short_input_yields_segment() {
        let fit = fit_cubic(&[Vec3::ZERO, Vec3::new(1., 0., 0.)], 1e-9).unwrap();
        assert!(fit.tol_achieved < 1e-12);
    }

    #[test]
    fn line_samples_fit_exactly() {
        let pts: Vec<Vec3> = (0..=10)
            .map(|i| Vec3::new(i as f64, 2.0 * i as f64, -0.5 * i as f64))
            .collect();
        let fit = fit_cubic(&pts, 1e-9).unwrap();
        assert!(fit.tol_achieved <= 1e-9, "tol {}", fit.tol_achieved);
    }
}
