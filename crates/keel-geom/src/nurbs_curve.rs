//! NURBS curves on homogeneous 4D control points (spec D4 policy:
//! run non-rational algorithms on Vec4, divide last).

use crate::GeomError;
use crate::knots::KnotVector;
use keel_math::vec::{Vec3, Vec4};

/// NURBS curve: clamped knot vector + homogeneous control points
/// (w*x, w*y, w*z, w), all weights strictly positive.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve {
    kv: KnotVector,
    /// Homogeneous control points, length == kv.control_count().
    ctrl: Vec<Vec4>,
}

impl NurbsCurve {
    /// Construct from 3D control points and optional weights
    /// (None = all 1, a polynomial B-spline).
    pub fn new(
        degree: usize,
        knots: Vec<f64>,
        points: Vec<Vec3>,
        weights: Option<Vec<f64>>,
    ) -> Result<Self, GeomError> {
        let kv = KnotVector::new(degree, knots)?;
        if points.len() != kv.control_count() {
            return Err(GeomError::CountMismatch);
        }
        let ws = match &weights {
            Some(w) if w.len() != points.len() => return Err(GeomError::CountMismatch),
            Some(w) => w.clone(),
            None => vec![1.0; points.len()],
        };
        if ws.iter().any(|w| !w.is_finite() || *w <= 0.0) {
            return Err(GeomError::InvalidWeight);
        }
        let ctrl = points
            .iter()
            .zip(&ws)
            .map(|(p, &w)| Vec4::new(p.x * w, p.y * w, p.z * w, w))
            .collect();
        Ok(Self { kv, ctrl })
    }

    /// Construct directly from homogeneous control points.
    pub fn from_homogeneous(kv: KnotVector, ctrl: Vec<Vec4>) -> Result<Self, GeomError> {
        if ctrl.len() != kv.control_count() {
            return Err(GeomError::CountMismatch);
        }
        if ctrl.iter().any(|c| !c.is_finite() || c.w <= 0.0) {
            return Err(GeomError::InvalidWeight);
        }
        Ok(Self { kv, ctrl })
    }

    #[inline]
    pub fn degree(&self) -> usize {
        self.kv.degree()
    }
    #[inline]
    pub fn knot_vector(&self) -> &KnotVector {
        &self.kv
    }
    #[inline]
    pub fn homogeneous_control(&self) -> &[Vec4] {
        &self.ctrl
    }
    #[inline]
    pub fn domain(&self) -> (f64, f64) {
        self.kv.domain()
    }
    /// True when any weight differs from the first (rational curve).
    pub fn is_rational(&self) -> bool {
        let w0 = self.ctrl[0].w;
        self.ctrl.iter().any(|c| (c.w - w0).abs() > 1e-14 * w0)
    }
    /// Projected (3D) control points; convex hull bounds the curve.
    pub fn control_points(&self) -> Vec<Vec3> {
        self.ctrl
            .iter()
            .map(|c| Vec3::new(c.x / c.w, c.y / c.w, c.z / c.w))
            .collect()
    }

    /// Homogeneous point via de Boor corner cutting (A3.1 on Vec4).
    pub fn eval_homogeneous(&self, u: f64) -> Vec4 {
        let p = self.kv.degree();
        let u = self.kv.clamp(u);
        let span = self.kv.find_span(u);
        let knots = self.kv.knots();
        // Working copy of the affected control points d[0..=p].
        let mut d: Vec<Vec4> = (0..=p).map(|i| self.ctrl[span - p + i]).collect();
        for r in 1..=p {
            for i in (r..=p).rev() {
                let gi = span - p + i; // global control index
                let denom = knots[gi + p + 1 - r] - knots[gi];
                let a = if denom == 0.0 { 0.0 } else { (u - knots[gi]) / denom };
                d[i] = d[i - 1] * (1.0 - a) + d[i] * a;
            }
        }
        d[p]
    }

    /// 3D point on the curve.
    #[inline]
    pub fn point(&self, u: f64) -> Vec3 {
        let h = self.eval_homogeneous(u);
        Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w)
    }

    /// Control points and knots of the homogeneous derivative curve
    /// (degree p-1): Q_i = p * (P_{i+1} - P_i) / (u_{i+p+1} - u_{i+1}).
    /// None when degree is already 1 (handled by the caller).
    fn derivative_curve(&self) -> Option<Self> {
        let p = self.kv.degree();
        if p == 1 {
            return None;
        }
        let knots = self.kv.knots();
        let ctrl: Vec<Vec4> = (0..self.ctrl.len() - 1)
            .map(|i| {
                let denom = knots[i + p + 1] - knots[i + 1];
                debug_assert!(denom > 0.0);
                (self.ctrl[i + 1] - self.ctrl[i]) * (p as f64 / denom)
            })
            .collect();
        let dknots = knots[1..knots.len() - 1].to_vec();
        let kv = KnotVector::new(p - 1, dknots).ok()?;
        // Derivative curves are vector-valued: w components may be
        // any sign, so skip the positive-weight check.
        if ctrl.len() != kv.control_count() {
            return None;
        }
        Some(Self { kv, ctrl })
    }

    /// Homogeneous derivatives A^(k) for k = 0..=d via successive
    /// derivative curves.
    fn ders_homogeneous(&self, u: f64, d: usize) -> Vec<Vec4> {
        let mut out = Vec::with_capacity(d + 1);
        out.push(self.eval_homogeneous(u));
        let mut cur = self.clone();
        let mut k = 1;
        while k <= d {
            if cur.kv.degree() == 1 {
                // Derivative of a degree-1 curve is the difference
                // quotient on the active span; higher ders are zero.
                let span = cur.kv.find_span(u);
                let knots = cur.kv.knots();
                let denom = knots[span + 1] - knots[span];
                let q = if denom > 0.0 {
                    (cur.ctrl[span] - cur.ctrl[span - 1]) * (1.0 / denom)
                } else {
                    Vec4::ZERO
                };
                out.push(q);
                for _ in (k + 1)..=d {
                    out.push(Vec4::ZERO);
                }
                return out;
            }
            match cur.derivative_curve() {
                Some(dc) => {
                    out.push(dc.eval_homogeneous(u));
                    cur = dc;
                }
                None => out.push(Vec4::ZERO),
            }
            k += 1;
        }
        out
    }

    /// 3D derivatives C^(0..=d)(u) via the rational quotient-rule
    /// recursion (NURBS Book A4.2):
    /// C^(k) = (A^(k) - sum_{i=1..k} binom(k,i) w^(i) C^(k-i)) / w.
    pub fn derivatives(&self, u: f64, d: usize) -> Vec<Vec3> {
        let h = self.ders_homogeneous(u, d);
        let d = h.len() - 1;
        let mut c: Vec<Vec3> = Vec::with_capacity(d + 1);
        let w0 = h[0].w;
        for k in 0..=d {
            let ak = Vec3::new(h[k].x, h[k].y, h[k].z);
            let mut v = ak;
            let mut binom = 1.0_f64;
            for i in 1..=k {
                binom = binom * (k - i + 1) as f64 / i as f64;
                v = v - c[k - i] * (binom * h[i].w);
            }
            c.push(v / w0);
        }
        c
    }

    /// Exact circular arc: center, two unit orthogonal in-plane axes,
    /// radius, sweep angle in (0, 2*pi]. Built from
    /// ceil(sweep / (pi/2)) rational quadratic segments with interior
    /// weights cos(d_theta / 2) (NURBS Book A7.1 pattern).
    pub fn circular_arc(
        center: Vec3,
        x_axis: Vec3,
        y_axis: Vec3,
        radius: f64,
        sweep: f64,
    ) -> Result<Self, GeomError> {
        if !(radius.is_finite() && radius > 0.0)
            || !(sweep > 0.0 && sweep <= core::f64::consts::TAU)
        {
            return Err(GeomError::Degenerate);
        }
        let x = x_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        let y = y_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        if x.dot(y).abs() > 1e-12 {
            return Err(GeomError::Degenerate);
        }
        let narcs = (sweep / core::f64::consts::FRAC_PI_2).ceil() as usize;
        let dtheta = sweep / narcs as f64;
        let w1 = (dtheta / 2.0).cos();
        let on_circle =
            |theta: f64| center + x * (radius * theta.cos()) + y * (radius * theta.sin());
        let tangent = |theta: f64| x * (-theta.sin()) + y * theta.cos();

        let mut ctrl: Vec<Vec4> = Vec::with_capacity(2 * narcs + 1);
        let p0 = on_circle(0.0);
        ctrl.push(Vec4::new(p0.x, p0.y, p0.z, 1.0));
        for i in 0..narcs {
            let t0 = i as f64 * dtheta;
            let t2 = t0 + dtheta;
            let q0 = on_circle(t0);
            let q2 = on_circle(t2);
            // Shoulder point: the end tangents intersect at distance
            // r * tan(dtheta/2) along the tangent from each endpoint.
            let s = radius * (dtheta / 2.0).tan();
            let q1 = q0 + tangent(t0) * s;
            debug_assert!((q1 - (q2 - tangent(t2) * s)).norm() < 1e-9 * radius.max(1.0));
            ctrl.push(Vec4::new(q1.x * w1, q1.y * w1, q1.z * w1, w1));
            ctrl.push(Vec4::new(q2.x, q2.y, q2.z, 1.0));
        }
        // Clamped degree 2 with double interior knots at segment
        // boundaries; domain [0, 1].
        let mut knots = vec![0.0, 0.0, 0.0];
        for i in 1..narcs {
            let v = i as f64 / narcs as f64;
            knots.push(v);
            knots.push(v);
        }
        knots.extend_from_slice(&[1.0, 1.0, 1.0]);
        let kv = KnotVector::new(2, knots)?;
        Self::from_homogeneous(kv, ctrl)
    }

    /// Exact full circle in the plane spanned by x_axis/y_axis.
    pub fn full_circle(
        center: Vec3,
        x_axis: Vec3,
        y_axis: Vec3,
        radius: f64,
    ) -> Result<Self, GeomError> {
        Self::circular_arc(center, x_axis, y_axis, radius, core::f64::consts::TAU)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use proptest::prelude::*;

    /// Random valid NURBS curves: degree 1..=4, up to 3 extra control
    /// points, sorted random interior knots, weights in [0.2, 5],
    /// coordinates in [-100, 100], domain [0, 1].
    pub(crate) fn arb_nurbs() -> impl Strategy<Value = NurbsCurve> {
        (1usize..=4, 0usize..=3).prop_flat_map(|(p, extra)| {
            let n_ctrl = p + 1 + extra;
            let n_interior = n_ctrl - p - 1;
            let interior = proptest::collection::vec(0.05..0.95f64, n_interior);
            let coords = proptest::collection::vec(
                (-100.0..100.0f64, -100.0..100.0f64, -100.0..100.0f64),
                n_ctrl,
            );
            let weights = proptest::collection::vec(0.2..5.0f64, n_ctrl);
            (interior, coords, weights).prop_map(move |(mut int, pts, ws)| {
                int.sort_by(f64::total_cmp);
                let mut knots = vec![0.0; p + 1];
                knots.extend(int);
                knots.extend(vec![1.0; p + 1]);
                let points = pts.iter().map(|&(x, y, z)| Vec3::new(x, y, z)).collect();
                #[allow(clippy::unwrap_used)]
                NurbsCurve::new(p, knots, points, Some(ws)).unwrap()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::arb_nurbs;
    use super::*;
    use crate::basis::basis_funs;
    use proptest::prelude::*;

    #[test]
    fn construction_validates() {
        let pts = vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)];
        assert!(NurbsCurve::new(1, vec![0., 0., 1., 1.], pts.clone(), None).is_ok());
        assert_eq!(
            NurbsCurve::new(1, vec![0., 0., 1., 1.], pts.clone(), Some(vec![1.0]))
                .unwrap_err(),
            GeomError::CountMismatch
        );
        assert_eq!(
            NurbsCurve::new(1, vec![0., 0., 1., 1.], pts, Some(vec![1.0, -2.0]))
                .unwrap_err(),
            GeomError::InvalidWeight
        );
    }

    #[test]
    fn degree_one_interpolates_linearly() {
        let c = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![Vec3::ZERO, Vec3::new(2.0, 4.0, 6.0)],
            None,
        )
        .unwrap();
        assert!((c.point(0.5) - Vec3::new(1.0, 2.0, 3.0)).norm() < 1e-15);
    }

    #[test]
    fn full_circle_is_exact() {
        let c = NurbsCurve::full_circle(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            2.5,
        )
        .unwrap();
        for i in 0..=200 {
            let u = i as f64 / 200.0;
            let p = c.point(u);
            let r = (p - Vec3::new(1.0, 2.0, 3.0)).norm();
            assert!((r - 2.5).abs() < 1e-12, "u={u} r={r}");
            assert!((p.z - 3.0).abs() < 1e-12);
        }
    }

    #[test]
    fn arc_endpoints_and_sweep() {
        let c = NurbsCurve::circular_arc(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            1.0,
            core::f64::consts::FRAC_PI_3, // 60 degrees, single segment
        )
        .unwrap();
        assert!((c.point(0.0) - Vec3::new(1.0, 0.0, 0.0)).norm() < 1e-15);
        let end = Vec3::new(0.5, 3.0_f64.sqrt() / 2.0, 0.0);
        assert!((c.point(1.0) - end).norm() < 1e-14);
    }

    #[test]
    fn line_derivative_is_direction() {
        let c = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![Vec3::ZERO, Vec3::new(2.0, 4.0, 6.0)],
            None,
        )
        .unwrap();
        let d = c.derivatives(0.3, 2);
        assert!((d[1] - Vec3::new(2.0, 4.0, 6.0)).norm() < 1e-13);
        assert!(d[2].norm() < 1e-13);
    }

    #[test]
    fn circle_derivative_is_tangent() {
        let c = NurbsCurve::full_circle(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            3.0,
        )
        .unwrap();
        for i in 1..20 {
            let u = i as f64 / 20.0;
            let d = c.derivatives(u, 1);
            // Tangent is orthogonal to the radius vector.
            assert!(
                d[1].dot(d[0]).abs() < 1e-9 * d[1].norm() * d[0].norm().max(1.0),
                "u={u}"
            );
        }
    }

    proptest! {
        // Finite-difference cross-check of the first derivative.
        #[test]
        fn derivative_matches_finite_difference(
            c in arb_nurbs(), t in 0.05..0.95f64,
        ) {
            let h = 1e-6;
            let d = c.derivatives(t, 1);
            let fd = (c.point(t + h) - c.point(t - h)) / (2.0 * h);
            // FD error scales with curve magnitude; generous bound,
            // loosened further near reduced-continuity interior knots.
            let scale = d[1].norm().max(1.0);
            prop_assert!(
                (d[1] - fd).norm() < 1e-2 * scale,
                "analytic {:?} fd {:?}", d[1], fd
            );
        }

        // de Boor must agree with basis-functions-dot-control-points.
        #[test]
        fn de_boor_matches_basis_dot(c in arb_nurbs(), t in 0.0..1.0f64) {
            let p = c.degree();
            let span = c.knot_vector().find_span(t);
            let n = basis_funs(c.knot_vector(), span, t);
            let mut h = Vec4::ZERO;
            for i in 0..=p {
                h = h + c.homogeneous_control()[span - p + i] * n[i];
            }
            let via_basis = Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w);
            prop_assert!((c.point(t) - via_basis).norm() < 1e-9);
        }

        // The curve stays inside the convex hull of projected control
        // points; cheap necessary check via AABB.
        #[test]
        fn curve_inside_control_aabb(c in arb_nurbs(), t in 0.0..1.0f64) {
            use keel_math::bbox::Aabb3;
            let bb = Aabb3::from_points(c.control_points()).expanded(1e-9);
            prop_assert!(bb.contains(c.point(t)));
        }
    }
}
