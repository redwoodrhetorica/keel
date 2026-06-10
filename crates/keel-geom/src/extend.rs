//! Surface extension service (M5b; kernel/13 Part 2). Extension exists
//! to let local operations and healing re-intersect a surface beyond
//! its current patch. The requirements (kernel/13 section 2.2-2.4):
//! bounded safe-reach, fold + weight-positivity validation, ANALYTIC
//! continuation preferred (planes/cylinders/... extend to infinity
//! exactly), and CLEAN FAILURE rather than a folded surface.
//!
//! M5b scope: analytic extension is exact and unbounded (the common
//! boolean-neighbor case); NURBS extension is bounded natural
//! continuation by boundary-Bezier extrapolation with validation.
//! Reflection-G2 (Shetty-White) and energy-min (Mo-Zhao) extensions
//! are staged for a controlled-reach gap-fill consumer.

use crate::GeomError;
use crate::knots::KnotVector;
use crate::nurbs_surface::NurbsSurface;
use crate::surface::Surface3;
use keel_math::vec::Vec4;

/// Which parametric boundary to extend past.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtendDir {
    UMin,
    UMax,
    VMin,
    VMax,
}

/// Extension shape mode (M5b ships Natural; Tangential is documented
/// as the staged fallback for fold-prone high-curvature boundaries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtendMode {
    Natural,
}

/// Analytic surfaces are domain-unbounded: extension is exact and a
/// no-op. The caller marches the wider parameter window directly. This
/// is the kernel/13 "prefer analytic continuation" path, and the
/// common case when a boolean must extend an analytic neighbor.
pub fn extend_analytic(s: &Surface3) -> Surface3 {
    s.clone()
}

/// Blossom (polar form) of the END-span polynomial of a clamped spline
/// over homogeneous control points: the generalized de Boor triangle
/// on the last `p + 1` control points, eliminating with a DIFFERENT
/// argument per level. With arguments beyond the domain this is exact
/// polynomial extrapolation of the last span.
fn blossom_end(knots: &[f64], ctrl: &[Vec4], p: usize, args: &[f64]) -> Vec4 {
    let n = ctrl.len() - 1;
    let mut d: Vec<Vec4> = ctrl[n - p..].to_vec();
    for (r, &t) in args.iter().enumerate() {
        let r = r + 1;
        for i in (r..=p).rev() {
            let j = n - p + i;
            let lo = knots[j];
            let hi = knots[j + p - r + 1];
            let alpha = (t - lo) / (hi - lo);
            d[i] = d[i - 1] * (1.0 - alpha) + d[i] * alpha;
        }
    }
    d[p]
}

/// Natural extension of a clamped homogeneous spline past its MAX end
/// by `amount` (the knot-unclamping construction, dossier 13 sec 2.1):
/// the appended segment is the SAME polynomial as the last span (polar
/// form evaluated at mixed `u1`/`e` arguments), so the result is
/// C-infinity at the join and the original domain is bitwise the same
/// curve. New knots = the original with ONE end knot dropped (interior
/// multiplicity p at `u1`) plus `p + 1` knots at `e`; new control =
/// the original plus the blossom points.
fn extend_homogeneous_end(
    kv: &KnotVector,
    ctrl: &[Vec4],
    amount: f64,
) -> Result<(KnotVector, Vec<Vec4>), GeomError> {
    let p = kv.degree();
    let knots = kv.knots();
    let (_, u1) = kv.domain();
    let e = u1 + amount;
    let mut q = Vec::with_capacity(p);
    for k in 1..=p {
        let mut args = vec![u1; p - k];
        args.extend(core::iter::repeat_n(e, k));
        q.push(blossom_end(knots, ctrl, p, &args));
    }
    if q.iter().any(|c| !c.is_finite() || c.w <= 0.0) {
        return Err(GeomError::InvalidWeight);
    }
    let mut nk: Vec<f64> = knots.to_vec();
    nk.truncate(nk.len() - 1);
    nk.extend(core::iter::repeat_n(e, p + 1));
    let mut nc = ctrl.to_vec();
    nc.extend(q);
    Ok((KnotVector::new(p, nk)?, nc))
}

/// Reverse a homogeneous spline (control reversed, knots reflected).
fn reverse_homogeneous(
    kv: &KnotVector,
    ctrl: &[Vec4],
) -> Result<(KnotVector, Vec<Vec4>), GeomError> {
    let (a, b) = kv.domain();
    let nk: Vec<f64> = kv.knots().iter().rev().map(|&k| a + b - k).collect();
    let nc: Vec<Vec4> = ctrl.iter().rev().copied().collect();
    Ok((KnotVector::new(kv.degree(), nk)?, nc))
}

fn extend_homogeneous(
    kv: &KnotVector,
    ctrl: &[Vec4],
    at_end: bool,
    amount: f64,
) -> Result<(KnotVector, Vec<Vec4>), GeomError> {
    if at_end {
        extend_homogeneous_end(kv, ctrl, amount)
    } else {
        let (rk, rc) = reverse_homogeneous(kv, ctrl)?;
        let (ek, ec) = extend_homogeneous_end(&rk, &rc, amount)?;
        reverse_homogeneous(&ek, &ec)
    }
}

/// Natural extension of a NURBS curve past one end (the curve-level
/// unclamping service; healing and untrimming consume it directly).
pub fn extend_nurbs_curve(
    c: &crate::nurbs_curve::NurbsCurve,
    at_end: bool,
    amount: f64,
) -> Result<crate::nurbs_curve::NurbsCurve, GeomError> {
    if amount <= 0.0 || !amount.is_finite() {
        return Err(GeomError::Degenerate);
    }
    let (kv, ctrl) = extend_homogeneous(c.knot_vector(), c.homogeneous_control(), at_end, amount)?;
    crate::nurbs_curve::NurbsCurve::from_homogeneous(kv, ctrl)
}

/// Bounded natural extension of a NURBS surface past one boundary by
/// knot unclamping (dossier 13 sec 2.1, the exact natural prolongation
/// of the boundary span), applied per control row/column in
/// HOMOGENEOUS space so rational surfaces extend correctly. Guarded by
/// the safe-reach cap and fold/weight validation; the original domain
/// is reproduced exactly (the polar-form construction leaves every
/// original control window untouched).
pub fn extend_nurbs_surface(
    s: &NurbsSurface,
    dir: ExtendDir,
    amount: f64,
    _mode: ExtendMode,
) -> Result<NurbsSurface, GeomError> {
    if amount <= 0.0 || !amount.is_finite() {
        return Err(GeomError::Degenerate);
    }
    let (deg_u, deg_v) = (s.kv_u().degree(), s.kv_v().degree());
    let ((u0, u1), (v0, v1)) = s.domain();
    let (span, deg) = match dir {
        ExtendDir::UMin | ExtendDir::UMax => (u1 - u0, deg_u),
        ExtendDir::VMin | ExtendDir::VMax => (v1 - v0, deg_v),
    };
    let cap = span * 0.5 / deg as f64;
    if amount > cap {
        // Exceeds safe reach: the correct answer regardless of engine.
        return Err(GeomError::Degenerate);
    }
    let (nu, nv) = (s.count_u(), s.count_v());
    let ctrl = s.homogeneous_control();
    // The grid is u-outer / v-inner: v-curves are contiguous chunks.
    let extended = match dir {
        ExtendDir::VMin | ExtendDir::VMax => {
            let at_end = dir == ExtendDir::VMax;
            let mut new_kv: Option<KnotVector> = None;
            let mut grid = Vec::new();
            for iu in 0..nu {
                let chunk = &ctrl[iu * nv..(iu + 1) * nv];
                let (kv, nc) = extend_homogeneous(s.kv_v(), chunk, at_end, amount)?;
                grid.extend(nc);
                new_kv = Some(kv);
            }
            let kv_v = new_kv.ok_or(GeomError::Degenerate)?;
            NurbsSurface::from_homogeneous(s.kv_u().clone(), kv_v, grid)?
        }
        ExtendDir::UMin | ExtendDir::UMax => {
            let at_end = dir == ExtendDir::UMax;
            // Transpose to v-inner along u, extend, transpose back.
            let mut new_kv: Option<KnotVector> = None;
            let mut cols: Vec<Vec<Vec4>> = Vec::with_capacity(nv);
            for iv in 0..nv {
                let chunk: Vec<Vec4> = (0..nu).map(|iu| ctrl[iu * nv + iv]).collect();
                let (kv, nc) = extend_homogeneous(s.kv_u(), &chunk, at_end, amount)?;
                cols.push(nc);
                new_kv = Some(kv);
            }
            let kv_u = new_kv.ok_or(GeomError::Degenerate)?;
            let nu2 = kv_u.control_count();
            let mut grid = Vec::with_capacity(nu2 * nv);
            for iu in 0..nu2 {
                for col in cols.iter() {
                    grid.push(col[iu]);
                }
            }
            NurbsSurface::from_homogeneous(kv_u, s.kv_v().clone(), grid)?
        }
    };
    validate_extension(&extended)?;
    Ok(extended)
}

/// Validate an extended surface: weights strictly positive and the
/// surface normal does not flip sign along a sampling line (fold
/// detection).
fn validate_extension(s: &NurbsSurface) -> Result<(), GeomError> {
    if s.homogeneous_control().iter().any(|c| c.w <= 0.0) {
        return Err(GeomError::InvalidWeight);
    }
    let ((u0, u1), (v0, v1)) = s.domain();
    // A fold flips the normal between ADJACENT samples; a closed patch
    // (full cylinder) legitimately rotates the normal a full turn, so
    // compare consecutive samples, not a fixed reference.
    let mut prev: Option<keel_math::vec::Vec3> = None;
    for k in 0..=16 {
        let p = k as f64 / 16.0;
        let (u, v) = (u0 + p * (u1 - u0), 0.5 * (v0 + v1));
        if let Ok(lg) = s.local_geometry(u, v) {
            if let Some(r) = prev
                && r.dot(lg.normal) < 0.0
            {
                return Err(GeomError::Degenerate);
            }
            prev = Some(lg.normal);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nurbs_curve::NurbsCurve;
    use crate::surface::{Frame3, Sphere3};
    use keel_math::vec::Vec3;

    #[test]
    fn analytic_extension_is_identity() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let s = Surface3::Sphere(Sphere3::new(f, 2.0).unwrap());
        let e = extend_analytic(&s);
        assert_eq!(e, s);
    }

    /// The rational cylinder test surface: full-circle arc swept along
    /// z by 3 (degree-1 v), radius 2 about the origin.
    fn cylinder_surface() -> NurbsSurface {
        let arc = NurbsCurve::full_circle(
            Vec3::ZERO,
            Vec3::new(1., 0., 0.),
            Vec3::new(0., 1., 0.),
            2.0,
        )
        .unwrap();
        let prof = arc.homogeneous_control().to_vec();
        let mut ctrl = Vec::new();
        for c in &prof {
            ctrl.push(*c);
            ctrl.push(keel_math::vec::Vec4::new(c.x, c.y, c.z + 3.0 * c.w, c.w));
        }
        let kv_v = crate::knots::KnotVector::new(1, vec![0., 0., 1., 1.]).unwrap();
        NurbsSurface::from_homogeneous(arc.knot_vector().clone(), kv_v, ctrl).unwrap()
    }

    #[test]
    fn nurbs_surface_extension_continues_the_exact_cylinder() {
        // Unclamping extension along the axis: every extended point
        // stays EXACTLY on the infinite cylinder (the v-curves are the
        // natural polynomial continuation), and the original domain is
        // untouched.
        let s = cylinder_surface();
        let e = extend_nurbs_surface(&s, ExtendDir::VMax, 0.1, ExtendMode::Natural).unwrap();
        let ((u0, u1), (v0, v1_old)) = s.domain();
        let ((eu0, eu1), (ev0, ev1)) = e.domain();
        assert!((eu0 - u0).abs() < 1e-12 && (eu1 - u1).abs() < 1e-12);
        assert!((ev0 - v0).abs() < 1e-12 && (ev1 - (v1_old + 0.1)).abs() < 1e-12);
        for i in 0..=8 {
            let u = u0 + (u1 - u0) * i as f64 / 8.0;
            // Original domain reproduced exactly.
            for j in 0..=4 {
                let v = v0 + (v1_old - v0) * j as f64 / 4.0;
                let d = (e.point(u, v) - s.point(u, v)).norm();
                assert!(d < 1e-12, "original domain changed by {d}");
            }
            // Extension stays on the cylinder.
            for j in 1..=4 {
                let v = v1_old + 0.1 * j as f64 / 4.0;
                let p = e.point(u, v);
                let r = (p.x * p.x + p.y * p.y).sqrt();
                assert!((r - 2.0).abs() < 1e-9, "off cylinder by {}", (r - 2.0));
                assert!(p.z > 3.0 - 1e-12, "extension must be past the old rim");
            }
        }
        // Over-cap request still declines.
        assert!(extend_nurbs_surface(&s, ExtendDir::VMax, 100.0, ExtendMode::Natural).is_err());
        // UMin extension (the rational direction) also validates.
        let eu = extend_nurbs_surface(&s, ExtendDir::UMin, 0.05, ExtendMode::Natural).unwrap();
        let ((nu0, _), _) = eu.domain();
        let p = eu.point(nu0 + 0.01, 0.5);
        let r = (p.x * p.x + p.y * p.y).sqrt();
        assert!((r - 2.0).abs() < 1e-9, "rational u-extension off cylinder");
    }

    #[test]
    fn nurbs_curve_extension_stays_on_the_circle() {
        // A rational quadratic on a circle satisfies x^2 + y^2 = r^2
        // w^2 as a POLYNOMIAL identity, so its natural extension stays
        // exactly on the circle.
        let c = NurbsCurve::full_circle(
            Vec3::new(1.0, -2.0, 0.5),
            Vec3::new(1., 0., 0.),
            Vec3::new(0., 1., 0.),
            1.5,
        )
        .unwrap();
        let (t0, t1) = c.domain();
        let e = extend_nurbs_curve(&c, true, 0.07 * (t1 - t0)).unwrap();
        let (_, te) = e.domain();
        for i in 0..=6 {
            let t = t0 + (t1 - t0) * i as f64 / 6.0;
            assert!((e.point(t) - c.point(t)).norm() < 1e-12, "domain changed");
        }
        for i in 1..=5 {
            let t = t1 + (te - t1) * i as f64 / 5.0;
            let p = e.point(t) - Vec3::new(1.0, -2.0, 0.5);
            assert!((p.norm() - 1.5).abs() < 1e-9, "extension off circle");
        }
        // The start end extends too.
        let e2 = extend_nurbs_curve(&c, false, 0.05 * (t1 - t0)).unwrap();
        let (ts, _) = e2.domain();
        let p = e2.point(ts + 1e-3) - Vec3::new(1.0, -2.0, 0.5);
        assert!((p.norm() - 1.5).abs() < 1e-9, "start extension off circle");
    }
}
