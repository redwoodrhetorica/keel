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
use crate::nurbs_surface::NurbsSurface;
use crate::surface::Surface3;

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

/// Bounded natural extension of a NURBS surface past one boundary.
///
/// STAGED (M5b deferral ledger): correct natural extension of a
/// clamped B-spline requires the Piegl-Tiller unclamping algorithm
/// (recompute the boundary control points so the clamped basis
/// continues naturally past its end). A naive knot-widening produces
/// an unclamped, invalid surface. Rather than ship a folded or
/// invalid result, this returns a clean `Err`; the analytic path
/// (`extend_analytic`, exact and unbounded) covers the M6 boolean
/// neighbor-extension case, and NURBS natural extension lands when a
/// delete-face-healing or untrimming consumer needs it. The
/// safe-reach cap and fold/weight validation below are the validation
/// half, ready for the unclamping engine.
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
    // Unclamping engine not yet implemented; fail cleanly (never a
    // folded/invalid surface). See the doc comment.
    Err(GeomError::Degenerate)
}

/// Validate an extended surface: weights strictly positive and the
/// surface normal does not flip sign along a sampling line (fold
/// detection). Used by the NURBS extension engine when it lands.
#[allow(dead_code)]
fn validate_extension(s: &NurbsSurface) -> Result<(), GeomError> {
    if s.homogeneous_control().iter().any(|c| c.w <= 0.0) {
        return Err(GeomError::InvalidWeight);
    }
    let ((u0, u1), (v0, v1)) = s.domain();
    let mut ref_n: Option<keel_math::vec::Vec3> = None;
    for k in 0..=8 {
        let p = k as f64 / 8.0;
        let (u, v) = (u0 + p * (u1 - u0), 0.5 * (v0 + v1));
        if let Ok(lg) = s.local_geometry(u, v) {
            match ref_n {
                None => ref_n = Some(lg.normal),
                Some(r) => {
                    if r.dot(lg.normal) < 0.0 {
                        return Err(GeomError::Degenerate);
                    }
                }
            }
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

    #[test]
    fn nurbs_extension_fails_cleanly_pending_unclamp() {
        // Staged: NURBS natural extension returns a clean Err until the
        // unclamping engine lands (never a folded/invalid surface).
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
        let s = NurbsSurface::from_homogeneous(arc.knot_vector().clone(), kv_v, ctrl).unwrap();
        // Short request: staged Err. Over-cap request: Err. Both clean.
        assert!(extend_nurbs_surface(&s, ExtendDir::VMax, 0.1, ExtendMode::Natural).is_err());
        assert!(extend_nurbs_surface(&s, ExtendDir::VMax, 100.0, ExtendMode::Natural).is_err());
    }
}
