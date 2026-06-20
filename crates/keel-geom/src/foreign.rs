//! Foreign (host-defined) geometry: evaluator traits plus certified
//! fit-to-NURBS conversion (parity items 114/116 geometry core;
//! research/kernel/25 sec 17, research/kernel/08 sec 3.4 + rec 9).
//!
//! The host supplies a black-box evaluator (the TRUTH); the kernel
//! samples it and fits a certified NURBS (the CACHE), which then flows
//! through every existing NURBS path (tessellation, recover/simplify,
//! serde) with no new geometry variant. A black-box evaluator has no
//! exact predicates, so the NURBS-tolerant representation is the honest
//! one; when the fit cannot meet the requested tolerance the caller
//! sees that in `tol_achieved` and DECLINES rather than building a
//! wrong body.
//!
//! Fit method: tensor-product least squares by two-stage curve fitting
//! (NURBS Book 9.4.2) on a uniform parameter grid affinely mapped from
//! the foreign domain to `[0,1]` (so the cache reproduces the foreign
//! parameterization up to that affine map). Certification is against
//! FRESH evaluator samples on a denser offset grid, inflated by the
//! same `SAFETY` factor as `fit::fit_cubic`.

use crate::GeomError;
use crate::fit::lsq_fit;
use crate::nurbs_curve::NurbsCurve;
use crate::nurbs_surface::NurbsSurface;
use keel_math::vec::Vec3;

/// A host-defined parametric surface evaluated by callback. `eval`
/// must be defined and finite over the closed `domain` rectangle.
pub trait ForeignSurface {
    /// Parameter rectangle ((u0, u1), (v0, v1)), u0 < u1, v0 < v1.
    fn domain(&self) -> ((f64, f64), (f64, f64));
    /// Position at (u, v).
    fn eval(&self, u: f64, v: f64) -> Vec3;
}

/// A host-defined parametric curve evaluated by callback.
pub trait ForeignCurve {
    /// Parameter interval (t0, t1), t0 < t1.
    fn domain(&self) -> (f64, f64);
    /// Position at t.
    fn eval(&self, t: f64) -> Vec3;
}

/// A certified surface fit: the NURBS cache plus the certified upper
/// estimate of its max deviation from the evaluator.
#[derive(Clone, Debug)]
pub struct ForeignSurfaceFit {
    pub surface: NurbsSurface,
    /// Max deviation against fresh evaluator samples, times the
    /// safety factor.
    pub tol_achieved: f64,
}

/// A certified curve fit.
#[derive(Clone, Debug)]
pub struct ForeignCurveFit {
    pub curve: NurbsCurve,
    pub tol_achieved: f64,
}

/// Same between-sample inflation as `fit::fit_cubic`.
const SAFETY: f64 = 2.0;

/// Control counts tried per direction, in escalation order.
const ESCALATION: [usize; 4] = [4, 8, 16, 32];

/// Fit a foreign surface to a bicubic NURBS within `tol` if possible.
/// Control count escalates per `ESCALATION`; the best fit is returned
/// even when `tol` is not met (callers read `tol_achieved` and decide;
/// `Body::foreign_sheet` declines). The fit parameter domain is
/// `[0,1]^2`, affinely mapped from the foreign domain.
pub fn fit_foreign_surface(
    f: &dyn ForeignSurface,
    tol: f64,
) -> Result<ForeignSurfaceFit, GeomError> {
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    let ((u0, u1), (v0, v1)) = f.domain();
    if !(u0.is_finite() && u1.is_finite() && v0.is_finite() && v1.is_finite())
        || u0 >= u1
        || v0 >= v1
    {
        return Err(GeomError::OutOfDomain);
    }
    let mut best: Option<ForeignSurfaceFit> = None;
    for &n_ctrl in &ESCALATION {
        let m = 2 * n_ctrl + 1;
        // Uniform samples over the closed domain (ends included so the
        // end-interpolating fit pins the boundary to real data).
        let su: Vec<f64> = (0..m).map(|i| i as f64 / (m - 1) as f64).collect();
        let mut grid = vec![Vec3::ZERO; m * m];
        for (i, &si) in su.iter().enumerate() {
            for (j, &sj) in su.iter().enumerate() {
                let p = f.eval(u0 + si * (u1 - u0), v0 + sj * (v1 - v0));
                if !p.is_finite() {
                    return Err(GeomError::NonFinitePoint);
                }
                grid[i * m + j] = p;
            }
        }
        let Ok(surface) = two_stage_fit(&grid, m, &su, n_ctrl) else {
            continue;
        };
        // Certify against FRESH evaluator samples: a denser grid offset
        // half a step from the fit samples (plus the closed boundary).
        let dev = surface_eval_deviation(f, &surface, 2 * m + 1);
        let tol_achieved = dev * SAFETY;
        if best.as_ref().is_none_or(|b| tol_achieved < b.tol_achieved) {
            best = Some(ForeignSurfaceFit {
                surface,
                tol_achieved,
            });
        }
        if best.as_ref().is_some_and(|b| b.tol_achieved <= tol) {
            break;
        }
    }
    best.ok_or(GeomError::Degenerate)
}

/// Fit a foreign curve to a cubic NURBS within `tol` if possible.
/// Same contract as `fit_foreign_surface`, one direction.
pub fn fit_foreign_curve(f: &dyn ForeignCurve, tol: f64) -> Result<ForeignCurveFit, GeomError> {
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    let (t0, t1) = f.domain();
    if !(t0.is_finite() && t1.is_finite()) || t0 >= t1 {
        return Err(GeomError::OutOfDomain);
    }
    let mut best: Option<ForeignCurveFit> = None;
    for &n_ctrl in &ESCALATION {
        let m = 2 * n_ctrl + 1;
        let params: Vec<f64> = (0..m).map(|i| i as f64 / (m - 1) as f64).collect();
        let pts: Vec<Vec3> = params.iter().map(|&s| f.eval(t0 + s * (t1 - t0))).collect();
        if pts.iter().any(|p| !p.is_finite()) {
            return Err(GeomError::NonFinitePoint);
        }
        let Ok(curve) = lsq_fit(&pts, &params, n_ctrl) else {
            continue;
        };
        let dev = curve_eval_deviation(f, &curve, 2 * m + 1);
        let tol_achieved = dev * SAFETY;
        if best.as_ref().is_none_or(|b| tol_achieved < b.tol_achieved) {
            best = Some(ForeignCurveFit {
                curve,
                tol_achieved,
            });
        }
        if best.as_ref().is_some_and(|b| b.tol_achieved <= tol) {
            break;
        }
    }
    best.ok_or(GeomError::Degenerate)
}

/// A certified NURBS helix (capability-map item 15): axis through
/// `origin` along `axis`, `radius`, `pitch` advance per full turn,
/// `turns` total (may be fractional), fit within `tol` by the
/// foreign-curve machinery. A helix is transcendental (no exact NURBS
/// form exists), so the certified fit IS the honest representation;
/// callers read `tol_achieved` and decline when it misses.
pub fn fit_helix(
    origin: Vec3,
    axis: Vec3,
    radius: f64,
    pitch: f64,
    turns: f64,
    tol: f64,
) -> Result<ForeignCurveFit, GeomError> {
    let ok = origin.is_finite()
        && radius.is_finite()
        && radius > 0.0
        && pitch.is_finite()
        && turns.is_finite()
        && turns > 0.0;
    if !ok {
        return Err(GeomError::Degenerate);
    }
    let az = axis.try_normalize().ok_or(GeomError::Degenerate)?;
    // Frame perpendicular to the axis.
    let seed = if az.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let x = (seed - az * seed.dot(az))
        .try_normalize()
        .ok_or(GeomError::Degenerate)?;
    let y = az.cross(x);
    struct Helix {
        origin: Vec3,
        x: Vec3,
        y: Vec3,
        z: Vec3,
        radius: f64,
        pitch: f64,
        turns: f64,
    }
    impl ForeignCurve for Helix {
        fn domain(&self) -> (f64, f64) {
            (0.0, self.turns)
        }
        fn eval(&self, t: f64) -> Vec3 {
            let a = t * core::f64::consts::TAU;
            self.origin
                + self.x * (self.radius * a.cos())
                + self.y * (self.radius * a.sin())
                + self.z * (self.pitch * t)
        }
    }
    fit_foreign_curve(
        &Helix {
            origin,
            x,
            y,
            z: az,
            radius,
            pitch,
            turns,
        },
        tol,
    )
}

/// Tensor-product LSQ by two-stage curve fitting (NURBS Book 9.4.2):
/// fit each v-row of samples over u to get intermediate control
/// columns, then fit those columns over v. All row fits share the
/// same parameters and control count, hence the same derived knot
/// vector, so the columns assemble into one tensor-product net.
/// `grid` is row-major `grid[i * m + j]` with i the u sample index.
fn two_stage_fit(
    grid: &[Vec3],
    m: usize,
    params: &[f64],
    n_ctrl: usize,
) -> Result<NurbsSurface, GeomError> {
    // Stage 1: for each fixed j, fit over i (the u direction).
    let mut kv_u = None;
    let mut q = vec![Vec3::ZERO; n_ctrl * m]; // q[k * m + j]
    for j in 0..m {
        let row: Vec<Vec3> = (0..m).map(|i| grid[i * m + j]).collect();
        let c = lsq_fit(&row, params, n_ctrl)?;
        let ctrl = c.control_points();
        if ctrl.len() != n_ctrl {
            return Err(GeomError::CountMismatch);
        }
        for (k, p) in ctrl.into_iter().enumerate() {
            q[k * m + j] = p;
        }
        kv_u = Some(c.knot_vector().clone());
    }
    // Stage 2: for each control row k, fit over j (the v direction).
    let mut kv_v = None;
    let mut net = vec![Vec3::ZERO; n_ctrl * n_ctrl]; // net[k * n_ctrl + l]
    for k in 0..n_ctrl {
        let col: Vec<Vec3> = (0..m).map(|j| q[k * m + j]).collect();
        let c = lsq_fit(&col, params, n_ctrl)?;
        let ctrl = c.control_points();
        if ctrl.len() != n_ctrl {
            return Err(GeomError::CountMismatch);
        }
        for (l, p) in ctrl.into_iter().enumerate() {
            net[k * n_ctrl + l] = p;
        }
        kv_v = Some(c.knot_vector().clone());
    }
    let (kv_u, kv_v) = (
        kv_u.ok_or(GeomError::Degenerate)?,
        kv_v.ok_or(GeomError::Degenerate)?,
    );
    NurbsSurface::new(
        kv_u.degree(),
        kv_v.degree(),
        kv_u.knots().to_vec(),
        kv_v.knots().to_vec(),
        net,
        None,
    )
}

/// Max |fit - evaluator| over an (n x n) grid that includes the closed
/// boundary and lands between the fit samples (fresh evidence).
fn surface_eval_deviation(f: &dyn ForeignSurface, s: &NurbsSurface, n: usize) -> f64 {
    let ((u0, u1), (v0, v1)) = f.domain();
    let mut dev = 0.0f64;
    for i in 0..n {
        let si = i as f64 / (n - 1) as f64;
        for j in 0..n {
            let sj = j as f64 / (n - 1) as f64;
            let truth = f.eval(u0 + si * (u1 - u0), v0 + sj * (v1 - v0));
            dev = dev.max((s.point(si, sj) - truth).norm());
        }
    }
    dev
}

/// Max |fit - evaluator| over n fresh parameters including the ends.
fn curve_eval_deviation(f: &dyn ForeignCurve, c: &NurbsCurve, n: usize) -> f64 {
    let (t0, t1) = f.domain();
    let mut dev = 0.0f64;
    for i in 0..n {
        let s = i as f64 / (n - 1) as f64;
        let truth = f.eval(t0 + s * (t1 - t0));
        dev = dev.max((c.point(s) - truth).norm());
    }
    dev
}

#[cfg(test)]
mod tests {
    use super::*;

    /// z = a sin(x) cos(y) over [0,2]^2: smooth, genuinely bicubic-hard.
    struct Wavy {
        a: f64,
    }
    impl ForeignSurface for Wavy {
        fn domain(&self) -> ((f64, f64), (f64, f64)) {
            ((0.0, 2.0), (0.0, 2.0))
        }
        fn eval(&self, u: f64, v: f64) -> Vec3 {
            Vec3::new(u, v, self.a * u.sin() * v.cos())
        }
    }

    /// Quarter cylinder, radius 2 about z, height 3.
    struct QuarterCyl;
    impl ForeignSurface for QuarterCyl {
        fn domain(&self) -> ((f64, f64), (f64, f64)) {
            ((0.0, core::f64::consts::FRAC_PI_2), (0.0, 1.0))
        }
        fn eval(&self, u: f64, v: f64) -> Vec3 {
            Vec3::new(2.0 * u.cos(), 2.0 * u.sin(), 3.0 * v)
        }
    }

    /// C0 crease z = |u - 0.5|: a cubic spline cannot meet a tight tol.
    struct Crease;
    impl ForeignSurface for Crease {
        fn domain(&self) -> ((f64, f64), (f64, f64)) {
            ((0.0, 1.0), (0.0, 1.0))
        }
        fn eval(&self, u: f64, v: f64) -> Vec3 {
            Vec3::new(u, v, (u - 0.5).abs())
        }
    }

    #[test]
    fn wavy_surface_fits_within_tol() {
        let f = Wavy { a: 0.3 };
        let fit = fit_foreign_surface(&f, 1e-4).unwrap();
        assert!(
            fit.tol_achieved <= 1e-4,
            "wavy fit must certify 1e-4 (got {})",
            fit.tol_achieved
        );
        // Independent spot checks at parameters unrelated to either the
        // fit or the certification grid.
        for &(su, sv) in &[(0.137, 0.713), (0.591, 0.244), (0.93, 0.07)] {
            let truth = f.eval(2.0 * su, 2.0 * sv);
            let got = fit.surface.point(su, sv);
            assert!(
                (got - truth).norm() <= fit.tol_achieved,
                "spot deviation {} exceeds certificate {}",
                (got - truth).norm(),
                fit.tol_achieved
            );
        }
    }

    #[test]
    fn corners_are_interpolated_exactly() {
        // End-pinned LSQ in both stages: the four corners are data.
        let f = Wavy { a: 0.3 };
        let fit = fit_foreign_surface(&f, 1e-4).unwrap();
        for &(su, sv) in &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            let truth = f.eval(2.0 * su, 2.0 * sv);
            assert!(
                (fit.surface.point(su, sv) - truth).norm() < 1e-12,
                "corner ({su},{sv}) not interpolated"
            );
        }
    }

    #[test]
    fn cylinder_patch_fits_tightly() {
        let fit = fit_foreign_surface(&QuarterCyl, 1e-5).unwrap();
        assert!(
            fit.tol_achieved <= 1e-5,
            "quarter cylinder must certify 1e-5 (got {})",
            fit.tol_achieved
        );
    }

    #[test]
    fn crease_declines_at_tight_tol() {
        // The best cubic fit of a C0 crease is far from 1e-9; the
        // certificate must say so (the caller then DECLINES).
        let fit = fit_foreign_surface(&Crease, 1e-9).unwrap();
        assert!(
            fit.tol_achieved > 1e-9,
            "a C0 crease must not certify at 1e-9 (got {})",
            fit.tol_achieved
        );
    }

    struct HelixArc;
    impl ForeignCurve for HelixArc {
        fn domain(&self) -> (f64, f64) {
            (0.0, core::f64::consts::PI)
        }
        fn eval(&self, t: f64) -> Vec3 {
            Vec3::new(t.cos(), t.sin(), 0.25 * t)
        }
    }

    #[test]
    fn helix_curve_fits_within_tol() {
        let fit = fit_foreign_curve(&HelixArc, 1e-5).unwrap();
        assert!(
            fit.tol_achieved <= 1e-5,
            "helix arc must certify 1e-5 (got {})",
            fit.tol_achieved
        );
        for &s in &[0.171, 0.503, 0.887] {
            let truth = HelixArc.eval(core::f64::consts::PI * s);
            assert!((fit.curve.point(s) - truth).norm() <= fit.tol_achieved);
        }
    }

    #[test]
    fn helix_fit_certifies_and_lands_on_the_helix() {
        // Two full turns, radius 2, pitch 0.5, about +z through the
        // origin. Spot-check fresh parameters against the analytic
        // helix; certificate must hold there.
        let fit = fit_helix(Vec3::ZERO, Vec3::new(0., 0., 1.), 2.0, 0.5, 2.0, 1e-3).unwrap();
        assert!(
            fit.tol_achieved <= 1e-3,
            "helix must certify 1e-3 (got {})",
            fit.tol_achieved
        );
        let (t0, t1) = fit.curve.domain();
        for &s in &[0.083, 0.291, 0.566, 0.940] {
            let p = fit.curve.point(t0 + s * (t1 - t0));
            // The fit parameter is turns scaled to the domain.
            let t = s * 2.0;
            let a = t * core::f64::consts::TAU;
            let truth = Vec3::new(2.0 * a.cos(), 2.0 * a.sin(), 0.5 * t);
            assert!(
                (p - truth).norm() <= fit.tol_achieved,
                "helix point off by {} at s={s}",
                (p - truth).norm()
            );
        }
    }

    #[test]
    fn bad_inputs_are_rejected() {
        struct BadDomain;
        impl ForeignSurface for BadDomain {
            fn domain(&self) -> ((f64, f64), (f64, f64)) {
                ((1.0, 1.0), (0.0, 1.0))
            }
            fn eval(&self, _u: f64, _v: f64) -> Vec3 {
                Vec3::ZERO
            }
        }
        assert_eq!(
            fit_foreign_surface(&BadDomain, 1e-6).unwrap_err(),
            GeomError::OutOfDomain
        );
        assert_eq!(
            fit_foreign_surface(&Wavy { a: 0.3 }, 0.0).unwrap_err(),
            GeomError::Degenerate
        );
        struct NanEval;
        impl ForeignSurface for NanEval {
            fn domain(&self) -> ((f64, f64), (f64, f64)) {
                ((0.0, 1.0), (0.0, 1.0))
            }
            fn eval(&self, _u: f64, _v: f64) -> Vec3 {
                Vec3::new(f64::NAN, 0.0, 0.0)
            }
        }
        assert_eq!(
            fit_foreign_surface(&NanEval, 1e-6).unwrap_err(),
            GeomError::NonFinitePoint
        );
    }
}
