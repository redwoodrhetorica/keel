//! NURBS curves on homogeneous 4D control points (spec D4 policy:
//! run non-rational algorithms on Vec4, divide last).

use crate::GeomError;
use crate::knots::KnotVector;
use keel_math::interval::Interval;
use keel_math::vec::{Vec3, Vec4};

/// In-place de Boor corner cutting (A3.1 inner loops): d[0..=p] holds
/// the affected control points for `span` (local index k maps to
/// global control index span - p + k); the result lands in d[p].
pub(crate) fn de_boor_in_place(knots: &[f64], p: usize, span: usize, u: f64, d: &mut [Vec4]) {
    for r in 1..=p {
        for i in (r..=p).rev() {
            let gi = span - p + i; // global control index
            let denom = knots[gi + p + 1 - r] - knots[gi];
            let a = if denom == 0.0 {
                0.0
            } else {
                (u - knots[gi]) / denom
            };
            d[i] = d[i - 1] * (1.0 - a) + d[i] * a;
        }
    }
}

/// Binomial coefficient as f64; orders here are small (<= MAX_DEGREE).
pub(crate) fn binom(n: usize, k: usize) -> f64 {
    let mut b = 1.0f64;
    for i in 1..=k {
        b = b * (n - i + 1) as f64 / i as f64;
    }
    b
}

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
        if points.iter().any(|p| !p.is_finite()) {
            return Err(GeomError::NonFinitePoint);
        }
        // Weights are projective: a common scale factor leaves the curve
        // unchanged. Canonicalize by an exact power of two so the largest
        // weight lands in (0.5, 1]; the homogeneous products w*x are then
        // bounded by |x| and cannot overflow (fuzz finding 5: coords
        // ~1.3e219 with weights ~1.3e219 made w*x = inf in storage).
        // The scaling is split into two power-of-two factors so the factor
        // itself stays finite even for subnormal or near-max weights;
        // power-of-two multiplies are exact wherever the result is normal.
        let wmax = ws.iter().fold(0.0f64, |m, w| m.max(*w));
        let e = wmax.log2().ceil() as i32;
        let h = e / 2;
        let (s1, s2) = (2.0f64.powi(-h), 2.0f64.powi(-(e - h)));
        let ws: Vec<f64> = ws.iter().map(|w| w * s1 * s2).collect();
        // A weight that leaves the normal range after canonicalization
        // means the max/min ratio is not representable; evaluation could
        // not divide by it safely. Reject rather than misbehave.
        if ws.iter().any(|w| *w < f64::MIN_POSITIVE) {
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
        // Working copy of the affected control points d[0..=p].
        let mut d: Vec<Vec4> = (0..=p).map(|i| self.ctrl[span - p + i]).collect();
        de_boor_in_place(self.kv.knots(), p, span, u, &mut d);
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
                // A zero-width window occurs only at a discontinuity of
                // an internally built hodograph (user knot vectors cap
                // multiplicity below this); 0/0 := 0 matches the
                // basis-derivative convention.
                if denom > 0.0 {
                    (self.ctrl[i + 1] - self.ctrl[i]) * (p as f64 / denom)
                } else {
                    Vec4::ZERO
                }
            })
            .collect();
        let dknots = knots[1..knots.len() - 1].to_vec();
        // Hodograph knots may carry interior multiplicity degree + 1
        // (derivative of a C0 corner), so skip the multiplicity caps.
        let kv = KnotVector::new_hodograph(p - 1, dknots).ok()?;
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
        if !radius.is_finite() || radius <= 0.0 || !(sweep > 0.0 && sweep <= core::f64::consts::TAU)
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

    /// Boehm single knot insertion (A5.1 on homogeneous points).
    /// Geometry is unchanged; representation gains one control point.
    pub fn insert_knot(&self, u: f64) -> Result<Self, GeomError> {
        let p = self.kv.degree();
        let (a, b) = self.domain();
        if !(u > a && u < b) {
            return Err(GeomError::OutOfDomain);
        }
        if self.kv.multiplicity(u) >= p {
            return Err(GeomError::MultiplicityExceeded);
        }
        let span = self.kv.find_span(u);
        let knots = self.kv.knots();
        let mut new_ctrl: Vec<Vec4> = Vec::with_capacity(self.ctrl.len() + 1);
        new_ctrl.extend_from_slice(&self.ctrl[..=span - p]);
        for i in (span - p + 1)..=span {
            let alpha = (u - knots[i]) / (knots[i + p] - knots[i]);
            new_ctrl.push(self.ctrl[i - 1] * (1.0 - alpha) + self.ctrl[i] * alpha);
        }
        new_ctrl.extend_from_slice(&self.ctrl[span..]);
        let mut new_knots = knots.to_vec();
        new_knots.insert(span + 1, u);
        let kv = KnotVector::new(p, new_knots)?;
        // Weights stay positive: each new w is a convex combination.
        Self::from_homogeneous(kv, new_ctrl)
    }

    /// Split at u into two independent clamped curves covering [a, u]
    /// and [u, b].
    pub fn split(&self, u: f64) -> Result<(Self, Self), GeomError> {
        let p = self.kv.degree();
        let (a, b) = self.domain();
        if !(u > a && u < b) {
            return Err(GeomError::OutOfDomain);
        }
        // Raise multiplicity of u to p.
        let mut cur = self.clone();
        while cur.kv.multiplicity(u) < p {
            cur = cur.insert_knot(u)?;
        }
        let span = cur.kv.find_span(u);
        let knots = cur.kv.knots();
        let i0 = span - p; // control point lying on the curve at u
        let mut left_knots = knots[..=span].to_vec();
        left_knots.push(u);
        let mut right_knots = vec![u; p + 1];
        right_knots.extend_from_slice(&knots[span + 1..]);
        let left =
            Self::from_homogeneous(KnotVector::new(p, left_knots)?, cur.ctrl[..=i0].to_vec())?;
        let right =
            Self::from_homogeneous(KnotVector::new(p, right_knots)?, cur.ctrl[i0..].to_vec())?;
        Ok((left, right))
    }

    /// Decompose into rational Bezier segments: per nonempty span, the
    /// p+1 homogeneous Bezier control points plus the span's parameter
    /// interval [u0, u1] in this curve's domain.
    pub fn to_beziers(&self) -> Vec<BezierSegment> {
        let p = self.kv.degree();
        // Raise every distinct interior knot to multiplicity p.
        let mut cur = self.clone();
        let interior: Vec<f64> = {
            let (a, b) = self.domain();
            let mut ks: Vec<f64> = self
                .kv
                .knots()
                .iter()
                .copied()
                .filter(|&k| k > a && k < b)
                .collect();
            ks.dedup();
            ks
        };
        for u in interior {
            while cur.kv.multiplicity(u) < p {
                match cur.insert_knot(u) {
                    Ok(next) => cur = next,
                    Err(_) => break,
                }
            }
        }
        // Nonempty spans each hold a Bezier segment.
        let knots = cur.kv.knots();
        let mut out = Vec::new();
        let mut start = p;
        while start + 1 < knots.len() - p {
            let (u0, u1) = (knots[start], knots[start + 1]);
            if u1 > u0 {
                let first_ctrl = start - p;
                out.push(BezierSegment {
                    ctrl: cur.ctrl[first_ctrl..=first_ctrl + p].to_vec(),
                    u0,
                    u1,
                });
            }
            start += 1;
        }
        out
    }
}

/// One rational Bezier span extracted from a NURBS curve, with its
/// parameter interval in the parent curve's domain.
#[derive(Clone, Debug)]
pub struct BezierSegment {
    /// Homogeneous Bezier control points, length = degree + 1.
    pub ctrl: Vec<Vec4>,
    pub u0: f64,
    pub u1: f64,
}

impl BezierSegment {
    /// De Casteljau at local t in [0,1]; homogeneous result.
    pub fn eval_homogeneous(&self, t: f64) -> Vec4 {
        let mut w = self.ctrl.clone();
        let mut len = w.len();
        while len > 1 {
            for i in 0..len - 1 {
                w[i] = w[i] * (1.0 - t) + w[i + 1] * t;
            }
            len -= 1;
        }
        w[0]
    }
    /// 3D point at local t.
    pub fn point(&self, t: f64) -> Vec3 {
        let h = self.eval_homogeneous(t);
        Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w)
    }
    /// Split at local t into left/right segments (de Casteljau edges).
    pub fn subdivide(&self, t: f64) -> (Self, Self) {
        let n = self.ctrl.len();
        let mut w = self.ctrl.clone();
        let mut left = Vec::with_capacity(n);
        let mut right = vec![Vec4::ZERO; n];
        left.push(w[0]);
        right[n - 1] = w[n - 1];
        for level in 1..n {
            for i in 0..n - level {
                w[i] = w[i] * (1.0 - t) + w[i + 1] * t;
            }
            left.push(w[0]);
            right[n - 1 - level] = w[n - 1 - level];
        }
        let um = self.u0 + t * (self.u1 - self.u0);
        (
            Self {
                ctrl: left,
                u0: self.u0,
                u1: um,
            },
            Self {
                ctrl: right,
                u0: um,
                u1: self.u1,
            },
        )
    }
    /// Projected 3D control points (hull bound, valid for w > 0).
    pub fn control_points(&self) -> Vec<Vec3> {
        self.ctrl
            .iter()
            .map(|c| Vec3::new(c.x / c.w, c.y / c.w, c.z / c.w))
            .collect()
    }

    /// Certified enclosure of the homogeneous point over a local
    /// parameter interval: interval de Casteljau. Over-wide (t and
    /// 1 - t are treated as independent) but always sound. The M5
    /// Krawczyk building block.
    pub fn eval_homogeneous_interval(&self, t: Interval) -> [Interval; 4] {
        let omt = Interval::point(1.0) - t;
        let mut w: Vec<[Interval; 4]> = self
            .ctrl
            .iter()
            .map(|c| {
                [
                    Interval::point(c.x),
                    Interval::point(c.y),
                    Interval::point(c.z),
                    Interval::point(c.w),
                ]
            })
            .collect();
        let mut len = w.len();
        while len > 1 {
            for i in 0..len - 1 {
                let next = w[i + 1];
                for (slot, nx) in w[i].iter_mut().zip(next) {
                    *slot = omt * *slot + t * nx;
                }
            }
            len -= 1;
        }
        w[0]
    }

    /// Certified 3D enclosure: the homogeneous enclosure divided by
    /// the weight interval. None when the weight enclosure straddles
    /// zero (the interval arithmetic must prove positivity, not
    /// assume it; for t inside [0,1] on a valid segment it does).
    pub fn point_enclosure(&self, t: Interval) -> Option<[Interval; 3]> {
        let h = self.eval_homogeneous_interval(t);
        Some([
            h[0].checked_div(h[3])?,
            h[1].checked_div(h[3])?,
            h[2].checked_div(h[3])?,
        ])
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
            NurbsCurve::new(1, vec![0., 0., 1., 1.], pts.clone(), Some(vec![1.0])).unwrap_err(),
            GeomError::CountMismatch
        );
        assert_eq!(
            NurbsCurve::new(1, vec![0., 0., 1., 1.], pts, Some(vec![1.0, -2.0])).unwrap_err(),
            GeomError::InvalidWeight
        );
    }

    /// Fuzz finding 5 (exact crash-artifact values): huge coords with
    /// huge weights overflowed the homogeneous lift w*x to inf, and the
    /// denormal trailing weight made the max/min ratio unrepresentable.
    /// The constructor must reject this input cleanly.
    #[test]
    fn fuzz_regression_extreme_weight_ratio_rejected() {
        let knots = vec![
            1.2217638442043777e161,
            1.2217638442043777e161,
            1.2217638442043777e161,
            1.2984845815252542e219,
            1.2984926927785823e219,
            1.2984926927785823e219,
            1.2984926927785823e219,
        ];
        let pts = vec![
            Vec3::new(
                1.4523965173143224e-176,
                1.2984926927785823e219,
                1.2984926927785823e219,
            ),
            Vec3::new(
                1.2984926927785823e219,
                1.2984926927785823e219,
                1.2984926927785823e219,
            ),
            Vec3::new(
                1.2984926927767764e219,
                1.2984926919361387e219,
                1.2984926927785823e219,
            ),
            Vec3::new(
                1.2984926927785823e219,
                1.2984926927785823e219,
                1.2984926927785823e219,
            ),
        ];
        let ws = vec![
            1.2984926927785837e219,
            1.2984926927785823e219,
            2008.3450982187308,
            3.5e-323,
        ];
        assert_eq!(
            NurbsCurve::new(2, knots, pts, Some(ws)).unwrap_err(),
            GeomError::InvalidWeight
        );
    }

    /// Companion to fuzz finding 5: the same huge coords with uniformly
    /// huge weights describe a perfectly finite curve; canonicalization
    /// must make the homogeneous lift survive and evaluation stay finite.
    #[test]
    fn huge_uniform_weights_canonicalize_to_finite_curve() {
        let big = 1.2984926927785823e219;
        let c = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![Vec3::new(big, 0.0, 0.0), Vec3::new(0.0, big, 0.0)],
            Some(vec![big, big]),
        )
        .unwrap();
        let p = c.point(0.5);
        assert!(p.is_finite());
        assert!((p - Vec3::new(0.5 * big, 0.5 * big, 0.0)).norm() < 1e-9 * big);
        // Canonicalized weights land in (0.5, 1].
        for h in c.homogeneous_control() {
            assert!(h.w > 0.5 && h.w <= 1.0);
        }
    }

    /// Weight canonicalization is projective: scaling all weights by a
    /// common factor must not move the curve.
    #[test]
    fn weight_scale_invariance() {
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(3.0, 0.0, 1.0),
        ];
        let knots = vec![0., 0., 0., 1., 1., 1.];
        let a = NurbsCurve::new(2, knots.clone(), pts.clone(), Some(vec![1.0, 2.0, 3.0])).unwrap();
        let b = NurbsCurve::new(2, knots, pts, Some(vec![1e200, 2e200, 3e200])).unwrap();
        for i in 0..=10 {
            let u = i as f64 / 10.0;
            assert!((a.point(u) - b.point(u)).norm() < 1e-12);
        }
    }

    /// Fuzz finding 6 (exact crash-artifact values): end knot with
    /// multiplicity p + 2 survived validation; derivative_curve then hit
    /// a zero knot denominator. The constructor must reject the knots.
    #[test]
    fn fuzz_regression_overfull_end_knot_rejected() {
        let a = 1.2217638442043777e161;
        let b = 1.2984926927785823e219;
        let knots = vec![a, a, a, b, b, b, b];
        let pts = vec![
            Vec3::new(1.4523965173143224e-176, b, b),
            Vec3::new(
                4.857875624558299e-33,
                4.8148343340696226e-33,
                4.848094946726139e-33,
            ),
            Vec3::new(5.1723916756679745e54, 7.440614902924675e219, b),
            Vec3::new(b, b, b),
        ];
        let ws = vec![1.2984926927785837e219, 1.2984522949348584e219, b, b];
        assert_eq!(
            NurbsCurve::new(2, knots, pts, Some(ws)).unwrap_err(),
            GeomError::MultiplicityExceeded
        );
    }

    #[test]
    fn nonfinite_points_rejected() {
        assert_eq!(
            NurbsCurve::new(
                1,
                vec![0., 0., 1., 1.],
                vec![Vec3::ZERO, Vec3::new(f64::NAN, 0.0, 0.0)],
                None,
            )
            .unwrap_err(),
            GeomError::NonFinitePoint
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

    /// M5a Task 0: enclosure soundness at the magnitude extremes the
    /// fuzz campaigns taught us to fear (huge, tiny, mixed-scale
    /// control points).
    #[test]
    fn interval_enclosure_sound_at_extreme_scales() {
        let cases: Vec<Vec<Vec3>> = vec![
            vec![
                Vec3::new(1e300, -1e300, 1e299),
                Vec3::new(-1e299, 1e300, -1e300),
                Vec3::new(1e300, 1e299, 1e300),
            ],
            vec![
                Vec3::new(1e-300, 1e-308, -1e-300),
                Vec3::new(-1e-299, 1e-300, 1e-308),
                Vec3::new(1e-300, -1e-300, 1e-299),
            ],
            vec![
                Vec3::new(1e300, 1e-300, 1.0),
                Vec3::new(-1.0, 1e300, 1e-300),
                Vec3::new(1e-300, -1.0, 1e300),
            ],
        ];
        for pts in cases {
            let c = NurbsCurve::new(2, vec![0., 0., 0., 1., 1., 1.], pts, None).unwrap();
            let segs = c.to_beziers();
            let seg = &segs[0];
            let ti = Interval::new(0.2, 0.7);
            let enc = seg.eval_homogeneous_interval(ti);
            for k in 0..=10 {
                let t = 0.2 + 0.5 * k as f64 / 10.0;
                let h = seg.eval_homogeneous(t);
                for (e, v) in enc.iter().zip([h.x, h.y, h.z, h.w]) {
                    assert!(!e.lo.is_nan() && !e.hi.is_nan());
                    assert!(e.contains(v), "{v} not in {e:?}");
                }
            }
        }
    }

    proptest! {
        // Interval enclosure soundness: any exact point at a parameter
        // inside the interval lies inside the enclosure.
        #[test]
        fn interval_eval_encloses_curve(
            c in arb_nurbs(),
            t0 in 0.0..1.0f64, dt in 0.0..0.3f64, fs in 0.0..1.0f64,
        ) {
            let segs = c.to_beziers();
            let seg = &segs[0];
            let t1 = (t0 + dt).min(1.0);
            let ti = Interval::new(t0.min(t1), t1.max(t0));
            let enc = seg.point_enclosure(ti).unwrap();
            let tx = ti.lo + fs * (ti.hi - ti.lo);
            let exact = seg.point(tx);
            prop_assert!(enc[0].contains(exact.x), "x {} not in {:?}", exact.x, enc[0]);
            prop_assert!(enc[1].contains(exact.y), "y {} not in {:?}", exact.y, enc[1]);
            prop_assert!(enc[2].contains(exact.z), "z {} not in {:?}", exact.z, enc[2]);
        }

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
            for (i, &ni) in n.iter().enumerate().take(p + 1) {
                h = h + c.homogeneous_control()[span - p + i] * ni;
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

        #[test]
        fn insertion_preserves_geometry(
            c in arb_nurbs(), uk in 0.1..0.9f64, t in 0.0..1.0f64,
        ) {
            if let Ok(c2) = c.insert_knot(uk) {
                prop_assert!((c.point(t) - c2.point(t)).norm() < 1e-9);
                prop_assert_eq!(
                    c2.homogeneous_control().len(),
                    c.homogeneous_control().len() + 1
                );
            }
        }

        #[test]
        fn split_halves_match(
            c in arb_nurbs(), us in 0.2..0.8f64, t in 0.0..1.0f64,
        ) {
            let (l, r) = c.split(us).unwrap();
            if t <= us {
                prop_assert!((l.point(t) - c.point(t)).norm() < 1e-8);
            }
            if t >= us {
                prop_assert!((r.point(t) - c.point(t)).norm() < 1e-8);
            }
        }

        #[test]
        fn bezier_segments_match(c in arb_nurbs(), t in 0.0..1.0f64) {
            let segs = c.to_beziers();
            let seg = segs
                .iter()
                .find(|s| t >= s.u0 && t <= s.u1)
                .expect("t inside some segment");
            let local = if seg.u1 > seg.u0 {
                (t - seg.u0) / (seg.u1 - seg.u0)
            } else {
                0.0
            };
            prop_assert!((seg.point(local) - c.point(t)).norm() < 1e-8);
        }
    }
}
