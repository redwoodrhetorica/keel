//! Tensor-product NURBS surfaces on homogeneous 4D control nets
//! (spec D4: non-rational algorithms on Vec4, divide last).

use crate::GeomError;
use crate::MAX_ORDER;
use crate::basis::basis_ders;
use crate::knots::KnotVector;
use crate::nurbs_curve::{binom, de_boor_in_place};
use keel_math::vec::{Vec3, Vec4};

/// NURBS surface: clamped knot vectors in u and v plus a homogeneous
/// control net, row-major ctrl[i * nv + j] with i the u index (0..nu)
/// and j the v index (0..nv). All weights strictly positive and
/// canonicalized so the maximum lies in (0.5, 1].
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    kv_u: KnotVector,
    kv_v: KnotVector,
    ctrl: Vec<Vec4>,
    nu: usize,
    nv: usize,
}

impl NurbsSurface {
    /// Construct from 3D control points (row-major nu x nv) and
    /// optional weights (None = all 1).
    pub fn new(
        degree_u: usize,
        degree_v: usize,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        points: Vec<Vec3>,
        weights: Option<Vec<f64>>,
    ) -> Result<Self, GeomError> {
        let kv_u = KnotVector::new(degree_u, knots_u)?;
        let kv_v = KnotVector::new(degree_v, knots_v)?;
        let (nu, nv) = (kv_u.control_count(), kv_v.control_count());
        if points.len() != nu * nv {
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
        // Same projective canonicalization as NurbsCurve::new (fuzz
        // finding 5): exact power-of-two scale so the max weight lands
        // in (0.5, 1]; split into two factors so the factor itself
        // stays finite; weights leaving the normal range mean the
        // max/min ratio is not representable.
        let wmax = ws.iter().fold(0.0f64, |m, w| m.max(*w));
        let e = wmax.log2().ceil() as i32;
        let h = e / 2;
        let (s1, s2) = (2.0f64.powi(-h), 2.0f64.powi(-(e - h)));
        let ws: Vec<f64> = ws.iter().map(|w| w * s1 * s2).collect();
        if ws.iter().any(|w| *w < f64::MIN_POSITIVE) {
            return Err(GeomError::InvalidWeight);
        }
        let ctrl = points
            .iter()
            .zip(&ws)
            .map(|(p, &w)| Vec4::new(p.x * w, p.y * w, p.z * w, w))
            .collect();
        Ok(Self {
            kv_u,
            kv_v,
            ctrl,
            nu,
            nv,
        })
    }

    /// Construct directly from a homogeneous net (row-major nu x nv).
    pub fn from_homogeneous(
        kv_u: KnotVector,
        kv_v: KnotVector,
        ctrl: Vec<Vec4>,
    ) -> Result<Self, GeomError> {
        let (nu, nv) = (kv_u.control_count(), kv_v.control_count());
        if ctrl.len() != nu * nv {
            return Err(GeomError::CountMismatch);
        }
        if ctrl.iter().any(|c| !c.is_finite() || c.w <= 0.0) {
            return Err(GeomError::InvalidWeight);
        }
        Ok(Self {
            kv_u,
            kv_v,
            ctrl,
            nu,
            nv,
        })
    }

    #[inline]
    pub fn kv_u(&self) -> &KnotVector {
        &self.kv_u
    }
    #[inline]
    pub fn kv_v(&self) -> &KnotVector {
        &self.kv_v
    }
    #[inline]
    pub fn count_u(&self) -> usize {
        self.nu
    }
    #[inline]
    pub fn count_v(&self) -> usize {
        self.nv
    }
    #[inline]
    pub fn homogeneous_control(&self) -> &[Vec4] {
        &self.ctrl
    }
    /// Parameter rectangle ((u0, u1), (v0, v1)).
    #[inline]
    pub fn domain(&self) -> ((f64, f64), (f64, f64)) {
        (self.kv_u.domain(), self.kv_v.domain())
    }

    /// Homogeneous point via tensor-product de Boor (A3.5): corner-cut
    /// in u along each affected v column, then once in v.
    pub fn eval_homogeneous(&self, u: f64, v: f64) -> Vec4 {
        let (p, q) = (self.kv_u.degree(), self.kv_v.degree());
        let u = self.kv_u.clamp(u);
        let v = self.kv_v.clamp(v);
        let su = self.kv_u.find_span(u);
        let sv = self.kv_v.find_span(v);
        let mut col = [Vec4::ZERO; MAX_ORDER];
        let mut row = [Vec4::ZERO; MAX_ORDER];
        for (j, slot) in col.iter_mut().enumerate().take(q + 1) {
            let gj = sv - q + j;
            for (i, r) in row.iter_mut().enumerate().take(p + 1) {
                *r = self.ctrl[(su - p + i) * self.nv + gj];
            }
            de_boor_in_place(self.kv_u.knots(), p, su, u, &mut row[..=p]);
            *slot = row[p];
        }
        de_boor_in_place(self.kv_v.knots(), q, sv, v, &mut col[..=q]);
        col[q]
    }

    /// 3D point on the surface.
    #[inline]
    pub fn point(&self, u: f64, v: f64) -> Vec3 {
        let h = self.eval_homogeneous(u, v);
        Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w)
    }

    /// Homogeneous partial derivatives A^(k,l) up to total order d via
    /// basis-function derivatives (A3.6 on Vec4). Entries beyond a
    /// direction's degree stay zero (M2a lesson: zero-fill, never
    /// truncate).
    fn ders_homogeneous(&self, u: f64, v: f64, d: usize) -> Vec<Vec<Vec4>> {
        let (p, q) = (self.kv_u.degree(), self.kv_v.degree());
        let u = self.kv_u.clamp(u);
        let v = self.kv_v.clamp(v);
        let su = self.kv_u.find_span(u);
        let sv = self.kv_v.find_span(v);
        let nu_d = basis_ders(&self.kv_u, su, u, d);
        let nv_d = basis_ders(&self.kv_v, sv, v, d);
        let mut out = vec![vec![Vec4::ZERO; d + 1]; d + 1];
        for (k, row_k) in out.iter_mut().enumerate().take(d.min(p) + 1) {
            for (l, slot) in row_k.iter_mut().enumerate().take(d.min(q) + 1) {
                let mut acc = Vec4::ZERO;
                for (i, &nui) in nu_d[k][..=p].iter().enumerate() {
                    let gi = su - p + i;
                    let mut inner = Vec4::ZERO;
                    for (j, &nvj) in nv_d[l][..=q].iter().enumerate() {
                        inner = inner + self.ctrl[gi * self.nv + (sv - q + j)] * nvj;
                    }
                    acc = acc + inner * nui;
                }
                *slot = acc;
            }
        }
        out
    }

    /// 3D partial derivatives S^(k,l) for k + l <= d via the bivariate
    /// rational quotient rule (NURBS Book A4.4):
    /// S_{k,l} = (A_{k,l} - sum_i B(k,i) w_{i,0} S_{k-i,l}
    ///                    - sum_j B(l,j) w_{0,j} S_{k,l-j}
    ///                    - sum_{i,j} B(k,i) B(l,j) w_{i,j} S_{k-i,l-j}) / w.
    /// Entries with k + l > d are zero.
    pub fn derivatives(&self, u: f64, v: f64, d: usize) -> Vec<Vec<Vec3>> {
        let a = self.ders_homogeneous(u, v, d);
        let w0 = a[0][0].w;
        let mut out = vec![vec![Vec3::ZERO; d + 1]; d + 1];
        for k in 0..=d {
            for l in 0..=(d - k) {
                let mut acc = Vec3::new(a[k][l].x, a[k][l].y, a[k][l].z);
                for i in 1..=k {
                    acc = acc - out[k - i][l] * (binom(k, i) * a[i][0].w);
                }
                for j in 1..=l {
                    acc = acc - out[k][l - j] * (binom(l, j) * a[0][j].w);
                }
                for i in 1..=k {
                    for j in 1..=l {
                        acc = acc
                            - out[k - i][l - j] * (binom(k, i) * binom(l, j) * a[i][j].w);
                    }
                }
                out[k][l] = acc * (1.0 / w0);
            }
        }
        out
    }

    /// Full second-order local geometry at (u, v).
    pub fn local_geometry(
        &self,
        u: f64,
        v: f64,
    ) -> Result<crate::surface::SurfaceLocalGeometry, GeomError> {
        let d = self.derivatives(u, v, 2);
        crate::surface::local_geometry_from_ders(d[0][0], d[1][0], d[0][1], d[2][0], d[1][1], d[0][2])
    }

    /// Insert `ubar` once into the u knot vector (Boehm A5.3 row form).
    /// Geometry unchanged; representation refined.
    pub fn insert_knot_u(&self, ubar: f64) -> Result<Self, GeomError> {
        let p = self.kv_u.degree();
        let (a, b) = self.kv_u.domain();
        if !ubar.is_finite() || ubar <= a || ubar >= b {
            return Err(GeomError::OutOfDomain);
        }
        if self.kv_u.multiplicity(ubar) >= p {
            return Err(GeomError::MultiplicityExceeded);
        }
        let knots = self.kv_u.knots();
        let k = self.kv_u.find_span(ubar);
        let mut new_ctrl = vec![Vec4::ZERO; (self.nu + 1) * self.nv];
        for j in 0..self.nv {
            for i in 0..=(k - p) {
                new_ctrl[i * self.nv + j] = self.ctrl[i * self.nv + j];
            }
            for i in (k - p + 1)..=k {
                let denom = knots[i + p] - knots[i];
                let alpha = if denom == 0.0 {
                    0.0
                } else {
                    (ubar - knots[i]) / denom
                };
                new_ctrl[i * self.nv + j] = self.ctrl[(i - 1) * self.nv + j] * (1.0 - alpha)
                    + self.ctrl[i * self.nv + j] * alpha;
            }
            for i in k..self.nu {
                new_ctrl[(i + 1) * self.nv + j] = self.ctrl[i * self.nv + j];
            }
        }
        let mut new_knots = knots.to_vec();
        new_knots.insert(k + 1, ubar);
        let kv_u = KnotVector::new(p, new_knots)?;
        Ok(Self {
            kv_u,
            kv_v: self.kv_v.clone(),
            ctrl: new_ctrl,
            nu: self.nu + 1,
            nv: self.nv,
        })
    }

    /// Insert `vbar` once into the v knot vector (column form).
    pub fn insert_knot_v(&self, vbar: f64) -> Result<Self, GeomError> {
        let q = self.kv_v.degree();
        let (a, b) = self.kv_v.domain();
        if !vbar.is_finite() || vbar <= a || vbar >= b {
            return Err(GeomError::OutOfDomain);
        }
        if self.kv_v.multiplicity(vbar) >= q {
            return Err(GeomError::MultiplicityExceeded);
        }
        let knots = self.kv_v.knots();
        let k = self.kv_v.find_span(vbar);
        let nv2 = self.nv + 1;
        let mut new_ctrl = vec![Vec4::ZERO; self.nu * nv2];
        for i in 0..self.nu {
            for j in 0..=(k - q) {
                new_ctrl[i * nv2 + j] = self.ctrl[i * self.nv + j];
            }
            for j in (k - q + 1)..=k {
                let denom = knots[j + q] - knots[j];
                let alpha = if denom == 0.0 {
                    0.0
                } else {
                    (vbar - knots[j]) / denom
                };
                new_ctrl[i * nv2 + j] = self.ctrl[i * self.nv + (j - 1)] * (1.0 - alpha)
                    + self.ctrl[i * self.nv + j] * alpha;
            }
            for j in k..self.nv {
                new_ctrl[i * nv2 + (j + 1)] = self.ctrl[i * self.nv + j];
            }
        }
        let mut new_knots = knots.to_vec();
        new_knots.insert(k + 1, vbar);
        let kv_v = KnotVector::new(q, new_knots)?;
        Ok(Self {
            kv_u: self.kv_u.clone(),
            kv_v,
            ctrl: new_ctrl,
            nu: self.nu,
            nv: nv2,
        })
    }

    /// Decompose into rational Bezier patches by saturating every
    /// interior knot to full multiplicity in both directions.
    pub fn to_bezier_patches(&self) -> Result<Vec<BezierPatch>, GeomError> {
        let (p, q) = (self.kv_u.degree(), self.kv_v.degree());
        let mut s = self.clone();
        for dir_u in [true, false] {
            loop {
                let kv = if dir_u { &s.kv_u } else { &s.kv_v };
                let deg = kv.degree();
                let (a, b) = kv.domain();
                let next = kv
                    .knots()
                    .iter()
                    .copied()
                    .find(|&k| a < k && k < b && kv.multiplicity(k) < deg);
                match next {
                    Some(k) => {
                        s = if dir_u {
                            s.insert_knot_u(k)?
                        } else {
                            s.insert_knot_v(k)?
                        };
                    }
                    None => break,
                }
            }
        }
        // Each (span_u, span_v) block of (p+1) x (q+1) control points
        // is now a Bezier patch.
        let u_breaks = distinct_domain_knots(&s.kv_u);
        let v_breaks = distinct_domain_knots(&s.kv_v);
        let mut out = Vec::new();
        for (iu, uw) in u_breaks.windows(2).enumerate() {
            for (iv, vw) in v_breaks.windows(2).enumerate() {
                let i0 = iu * p;
                let j0 = iv * q;
                let mut ctrl = Vec::with_capacity((p + 1) * (q + 1));
                for i in 0..=p {
                    for j in 0..=q {
                        ctrl.push(s.ctrl[(i0 + i) * s.nv + (j0 + j)]);
                    }
                }
                out.push(BezierPatch {
                    p,
                    q,
                    ctrl,
                    u0: uw[0],
                    u1: uw[1],
                    v0: vw[0],
                    v1: vw[1],
                });
            }
        }
        Ok(out)
    }
}

/// Distinct knot values in order, including both ends.
fn distinct_domain_knots(kv: &KnotVector) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for &k in kv.knots() {
        if out.last() != Some(&k) {
            out.push(k);
        }
    }
    out
}

/// Rational Bezier patch (one span pair of a NurbsSurface) with its
/// originating parameter rectangle. Control row-major i * (q+1) + j.
#[derive(Clone, Debug)]
pub struct BezierPatch {
    pub p: usize,
    pub q: usize,
    pub ctrl: Vec<Vec4>,
    pub u0: f64,
    pub u1: f64,
    pub v0: f64,
    pub v1: f64,
}

impl BezierPatch {
    /// De Casteljau in s along each column, then in t (local [0,1]^2).
    pub fn eval_homogeneous(&self, s: f64, t: f64) -> Vec4 {
        let mut col = [Vec4::ZERO; MAX_ORDER];
        let mut lane = [Vec4::ZERO; MAX_ORDER];
        for (j, slot) in col.iter_mut().enumerate().take(self.q + 1) {
            for (i, l) in lane.iter_mut().enumerate().take(self.p + 1) {
                *l = self.ctrl[i * (self.q + 1) + j];
            }
            let mut len = self.p + 1;
            while len > 1 {
                for i in 0..len - 1 {
                    lane[i] = lane[i] * (1.0 - s) + lane[i + 1] * s;
                }
                len -= 1;
            }
            *slot = lane[0];
        }
        let mut len = self.q + 1;
        while len > 1 {
            for j in 0..len - 1 {
                col[j] = col[j] * (1.0 - t) + col[j + 1] * t;
            }
            len -= 1;
        }
        col[0]
    }

    pub fn eval(&self, s: f64, t: f64) -> Vec3 {
        let h = self.eval_homogeneous(s, t);
        Vec3::new(h.x / h.w, h.y / h.w, h.z / h.w)
    }

    /// Split along local u at s, propagating the parameter rectangle.
    pub fn subdivide_u(&self, s: f64) -> (Self, Self) {
        let cols = self.q + 1;
        let mut left = self.clone();
        let mut right = self.clone();
        for j in 0..cols {
            let mut lane: Vec<Vec4> = (0..=self.p).map(|i| self.ctrl[i * cols + j]).collect();
            for level in 1..=self.p {
                for i in 0..=(self.p - level) {
                    lane[i] = lane[i] * (1.0 - s) + lane[i + 1] * s;
                }
                left.ctrl[level * cols + j] = lane[0];
                right.ctrl[(self.p - level) * cols + j] = lane[self.p - level];
            }
        }
        let um = self.u0 + s * (self.u1 - self.u0);
        left.u1 = um;
        right.u0 = um;
        (left, right)
    }

    /// Split along local v at t.
    pub fn subdivide_v(&self, t: f64) -> (Self, Self) {
        let cols = self.q + 1;
        let mut left = self.clone();
        let mut right = self.clone();
        for i in 0..=self.p {
            let mut lane: Vec<Vec4> = (0..=self.q).map(|j| self.ctrl[i * cols + j]).collect();
            for level in 1..=self.q {
                for j in 0..=(self.q - level) {
                    lane[j] = lane[j] * (1.0 - t) + lane[j + 1] * t;
                }
                left.ctrl[i * cols + level] = lane[0];
                right.ctrl[i * cols + (self.q - level)] = lane[self.q - level];
            }
        }
        let vm = self.v0 + t * (self.v1 - self.v0);
        left.v1 = vm;
        right.v0 = vm;
        (left, right)
    }

    /// AABB of the projected control net (bounds the patch by the
    /// convex hull property; weights are positive).
    pub fn control_aabb(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for c in &self.ctrl {
            let p3 = Vec3::new(c.x / c.w, c.y / c.w, c.z / c.w);
            lo = Vec3::new(lo.x.min(p3.x), lo.y.min(p3.y), lo.z.min(p3.z));
            hi = Vec3::new(hi.x.max(p3.x), hi.y.max(p3.y), hi.z.max(p3.z));
        }
        (lo, hi)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::nurbs_curve::NurbsCurve;
    use proptest::prelude::*;

    /// Random valid NURBS surfaces: degrees 1..=3 per direction, up to
    /// 2 extra control rows/columns, sorted random interior knots,
    /// weights in [0.2, 5], coordinates in [-10, 10], domain [0, 1]^2,
    /// plus a (u, v) sample drawn inside the domain.
    pub(crate) fn arb_surface_uv() -> impl Strategy<Value = (NurbsSurface, f64, f64)> {
        (1usize..=3, 1usize..=3, 0usize..=2, 0usize..=2).prop_flat_map(|(p, q, eu, ev)| {
            let nu = p + 1 + eu;
            let nv = q + 1 + ev;
            let int_u = proptest::collection::vec(0.05..0.95f64, nu - p - 1);
            let int_v = proptest::collection::vec(0.05..0.95f64, nv - q - 1);
            let coords = proptest::collection::vec(
                (-10.0..10.0f64, -10.0..10.0f64, -10.0..10.0f64),
                nu * nv,
            );
            let weights = proptest::collection::vec(0.2..5.0f64, nu * nv);
            (int_u, int_v, coords, weights, 0.0..1.0f64, 0.0..1.0f64).prop_map(
                move |(mut iu, mut iv, pts, ws, su, sv)| {
                    iu.sort_by(f64::total_cmp);
                    iv.sort_by(f64::total_cmp);
                    let mut ku = vec![0.0; p + 1];
                    ku.extend(iu);
                    ku.extend(vec![1.0; p + 1]);
                    let mut kv = vec![0.0; q + 1];
                    kv.extend(iv);
                    kv.extend(vec![1.0; q + 1]);
                    let points = pts.iter().map(|&(x, y, z)| Vec3::new(x, y, z)).collect();
                    #[allow(clippy::unwrap_used)]
                    let s = NurbsSurface::new(p, q, ku, kv, points, Some(ws)).unwrap();
                    (s, su, sv)
                },
            )
        })
    }

    /// Quarter cylinder of radius 2 about the z axis, height 0..3:
    /// exact circular arc extruded linearly (weights replicated).
    pub(crate) fn quarter_cylinder_r2() -> NurbsSurface {
        #[allow(clippy::unwrap_used)]
        let arc = NurbsCurve::circular_arc(
            Vec3::ZERO,
            Vec3::new(1., 0., 0.),
            Vec3::new(0., 1., 0.),
            2.0,
            core::f64::consts::FRAC_PI_2,
        )
        .unwrap();
        let prof = arc.homogeneous_control().to_vec();
        let n = prof.len();
        let mut ctrl = Vec::with_capacity(n * 2);
        for c in &prof {
            // Row-major with v contiguous: v index j in {0 (z=0), 1 (z=3)}.
            ctrl.push(*c);
            ctrl.push(Vec4::new(c.x, c.y, c.z + 3.0 * c.w, c.w));
        }
        #[allow(clippy::unwrap_used)]
        let kv_v = KnotVector::new(1, vec![0., 0., 1., 1.]).unwrap();
        #[allow(clippy::unwrap_used)]
        NurbsSurface::from_homogeneous(arc.knot_vector().clone(), kv_v, ctrl).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{arb_surface_uv, quarter_cylinder_r2};
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn construction_validates() {
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(0., 1., 0.),
            Vec3::new(1., 0., 0.),
            Vec3::new(1., 1., 1.),
        ];
        let k = vec![0., 0., 1., 1.];
        assert!(NurbsSurface::new(1, 1, k.clone(), k.clone(), pts.clone(), None).is_ok());
        assert_eq!(
            NurbsSurface::new(1, 1, k.clone(), k.clone(), pts.clone(), Some(vec![1.0; 3]))
                .unwrap_err(),
            GeomError::CountMismatch
        );
        assert_eq!(
            NurbsSurface::new(
                1,
                1,
                k.clone(),
                k.clone(),
                pts.clone(),
                Some(vec![1., 1., -1., 1.])
            )
            .unwrap_err(),
            GeomError::InvalidWeight
        );
        let mut bad = pts.clone();
        bad[2].x = f64::NAN;
        assert_eq!(
            NurbsSurface::new(1, 1, k.clone(), k, bad, None).unwrap_err(),
            GeomError::NonFinitePoint
        );
    }

    #[test]
    fn bilinear_patch_interpolates() {
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(0., 2., 0.),
            Vec3::new(2., 0., 0.),
            Vec3::new(2., 2., 2.),
        ];
        let k = vec![0., 0., 1., 1.];
        let s = NurbsSurface::new(1, 1, k.clone(), k, pts, None).unwrap();
        assert!((s.point(0.0, 0.0) - Vec3::ZERO).norm() < 1e-15);
        assert!((s.point(1.0, 1.0) - Vec3::new(2., 2., 2.)).norm() < 1e-15);
        assert!((s.point(0.5, 0.5) - Vec3::new(1., 1., 0.5)).norm() < 1e-15);
    }

    /// Carried from fuzz finding 5: huge uniform weights canonicalize.
    #[test]
    fn huge_uniform_weights_canonicalize() {
        let big = 1.2984926927785823e219;
        let pts = vec![
            Vec3::new(big, 0., 0.),
            Vec3::new(0., big, 0.),
            Vec3::new(big, big, 0.),
            Vec3::new(0., 0., big),
        ];
        let k = vec![0., 0., 1., 1.];
        let s = NurbsSurface::new(1, 1, k.clone(), k, pts, Some(vec![big; 4])).unwrap();
        assert!(s.point(0.5, 0.5).is_finite());
        for h in s.homogeneous_control() {
            assert!(h.w > 0.5 && h.w <= 1.0);
        }
    }

    #[test]
    fn cylinder_points_lie_on_cylinder() {
        let s = quarter_cylinder_r2();
        let ((u0, u1), (v0, v1)) = s.domain();
        for i in 0..=6 {
            for j in 0..=6 {
                let u = u0 + (u1 - u0) * i as f64 / 6.0;
                let v = v0 + (v1 - v0) * j as f64 / 6.0;
                let p = s.point(u, v);
                let r = (p.x * p.x + p.y * p.y).sqrt();
                assert!((r - 2.0).abs() < 1e-12, "radius {r} at {u} {v}");
                assert!((-1e-12..=3.0 + 1e-12).contains(&p.z), "z {} at {u} {v}", p.z);
            }
        }
    }

    #[test]
    fn derivatives_of_bilinear() {
        // S(u,v) = (2u, 2v, 2uv): S_u = (2,0,2v), S_v = (0,2,2u), S_uv = (0,0,2).
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(0., 2., 0.),
            Vec3::new(2., 0., 0.),
            Vec3::new(2., 2., 2.),
        ];
        let k = vec![0., 0., 1., 1.];
        let s = NurbsSurface::new(1, 1, k.clone(), k, pts, None).unwrap();
        let d = s.derivatives(0.3, 0.7, 2);
        assert!((d[1][0] - Vec3::new(2., 0., 1.4)).norm() < 1e-13);
        assert!((d[0][1] - Vec3::new(0., 2., 0.6)).norm() < 1e-13);
        assert!((d[1][1] - Vec3::new(0., 0., 2.)).norm() < 1e-13);
        // Beyond degree: zero-filled, never truncated (M2a lesson).
        assert_eq!(d[2][0], Vec3::ZERO);
        assert_eq!(d[0][2], Vec3::ZERO);
    }

    #[test]
    fn cylinder_derivatives_are_tangent_and_axial() {
        let s = quarter_cylinder_r2();
        let ((u0, u1), _) = s.domain();
        let u = u0 + 0.37 * (u1 - u0);
        let d = s.derivatives(u, 0.42, 1);
        // v direction is the exact extrusion direction (0, 0, 3).
        assert!((d[0][1] - Vec3::new(0., 0., 3.)).norm() < 1e-12);
        // u derivative is tangent to the circle: perpendicular to the
        // radial direction and to z.
        let p = s.point(u, 0.42);
        let radial = Vec3::new(p.x, p.y, 0.0);
        assert!(d[1][0].dot(radial).abs() < 1e-10 * d[1][0].norm() * radial.norm());
        assert!(d[1][0].z.abs() < 1e-12);
    }

    #[test]
    fn insert_rejects_overfull_multiplicity() {
        let s = quarter_cylinder_r2();
        let ((u0, u1), _) = s.domain();
        let mid = 0.5 * u0 + 0.5 * u1;
        // The arc already has interior knots; pick a fresh value.
        let fresh = u0 + 0.37 * (u1 - u0);
        let m0 = s.kv_u().multiplicity(fresh);
        let mut cur = s;
        for _ in m0..2 {
            cur = cur.insert_knot_u(fresh).unwrap();
        }
        assert_eq!(
            cur.insert_knot_u(fresh).unwrap_err(),
            GeomError::MultiplicityExceeded
        );
        let _ = mid;
    }

    #[test]
    fn patch_subdivision_matches_eval() {
        let s = quarter_cylinder_r2();
        let patches = s.to_bezier_patches().unwrap();
        let pc = &patches[0];
        let (l, r) = pc.subdivide_u(0.5);
        for i in 0..=4 {
            for j in 0..=4 {
                let (a, b) = (i as f64 / 4.0, j as f64 / 4.0);
                let scale = 1.0 + pc.eval(a, b).norm();
                assert!((l.eval(a, b) - pc.eval(0.5 * a, b)).norm() < 1e-12 * scale);
                assert!((r.eval(a, b) - pc.eval(0.5 + 0.5 * a, b)).norm() < 1e-12 * scale);
            }
        }
        let (bl, br) = pc.subdivide_v(0.3);
        for i in 0..=4 {
            let a = i as f64 / 4.0;
            let scale = 1.0 + pc.eval(a, 0.5).norm();
            assert!((bl.eval(a, 0.5) - pc.eval(a, 0.15)).norm() < 1e-12 * scale);
            assert!((br.eval(a, 0.5) - pc.eval(a, 0.3 + 0.7 * 0.5)).norm() < 1e-12 * scale);
        }
    }

    #[test]
    fn plane_patch_local_geometry() {
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(0., 3., 0.),
            Vec3::new(3., 0., 0.),
            Vec3::new(3., 3., 0.),
        ];
        let k = vec![0., 0., 1., 1.];
        let s = NurbsSurface::new(1, 1, k.clone(), k, pts, None).unwrap();
        let lg = s.local_geometry(0.4, 0.6).unwrap();
        assert!((lg.normal - Vec3::new(0., 0., 1.)).norm() < 1e-14);
        assert!(lg.gaussian.abs() < 1e-14);
        assert!(lg.mean.abs() < 1e-14);
        assert!(lg.k1.abs() < 1e-14 && lg.k2.abs() < 1e-14);
    }

    #[test]
    fn cylinder_patch_curvatures() {
        // Quarter cylinder radius 2 about z: K = 0, |H| = 1/(2r) = 0.25,
        // principal curvatures {0, +/-1/r}.
        let s = quarter_cylinder_r2();
        let lg = s.local_geometry(0.5, 0.5).unwrap();
        assert!(lg.gaussian.abs() < 1e-10);
        assert!((lg.mean.abs() - 0.25).abs() < 1e-10);
        let (kmax, kmin) = (
            lg.k1.abs().max(lg.k2.abs()),
            lg.k1.abs().min(lg.k2.abs()),
        );
        assert!((kmax - 0.5).abs() < 1e-10 && kmin < 1e-10);
        // The zero-curvature principal direction runs along the axis.
        let axis_dir = if lg.k1.abs() < lg.k2.abs() { lg.dir1 } else { lg.dir2 };
        assert!(axis_dir.cross(Vec3::new(0., 0., 1.)).norm() < 1e-8);
    }

    #[test]
    fn degenerate_normal_is_error() {
        // All control points on one line: S_u x S_v = 0 everywhere.
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(1., 0., 0.),
            Vec3::new(2., 0., 0.),
            Vec3::new(3., 0., 0.),
        ];
        let k = vec![0., 0., 1., 1.];
        let s = NurbsSurface::new(1, 1, k.clone(), k, pts, None).unwrap();
        assert_eq!(s.local_geometry(0.5, 0.5).unwrap_err(), GeomError::Degenerate);
    }

    proptest! {
        // FD oracle for first partials. h = 1e-5, tolerance 1e-4-scaled
        // (M2a lesson on the cancellation floor), same-span guard.
        #[test]
        fn surface_ders_match_finite_difference((s, u, v) in arb_surface_uv()) {
            let h = 1e-5;
            let ((u0, u1), (v0, v1)) = s.domain();
            prop_assume!(u - h > u0 && u + h < u1 && v - h > v0 && v + h < v1);
            prop_assume!(s.kv_u().find_span(u - h) == s.kv_u().find_span(u + h));
            prop_assume!(s.kv_v().find_span(v - h) == s.kv_v().find_span(v + h));
            let d = s.derivatives(u, v, 1);
            let fdu = (s.point(u + h, v) - s.point(u - h, v)) * (1.0 / (2.0 * h));
            let fdv = (s.point(u, v + h) - s.point(u, v - h)) * (1.0 / (2.0 * h));
            let scale = 1.0 + fdu.norm() + fdv.norm();
            prop_assert!((d[1][0] - fdu).norm() < 1e-4 * scale);
            prop_assert!((d[0][1] - fdv).norm() < 1e-4 * scale);
        }

        // Insertion is representation-only: geometry unchanged.
        #[test]
        fn insertion_preserves_geometry((s, u, v) in arb_surface_uv()) {
            let ((u0, u1), (v0, v1)) = s.domain();
            let su = s.insert_knot_u(0.5 * u0 + 0.5 * u1).unwrap();
            let sv = s.insert_knot_v(0.5 * v0 + 0.5 * v1).unwrap();
            let p0 = s.point(u, v);
            let scale = 1.0 + p0.norm();
            prop_assert!((su.point(u, v) - p0).norm() < 1e-9 * scale);
            prop_assert!((sv.point(u, v) - p0).norm() < 1e-9 * scale);
        }

        // Bezier decomposition reproduces the surface.
        #[test]
        fn bezier_patches_match((s, u, v) in arb_surface_uv()) {
            let p0 = s.point(u, v);
            let scale = 1.0 + p0.norm();
            let patches = s.to_bezier_patches().unwrap();
            let patch = patches.iter().find(|pc|
                pc.u0 <= u && u <= pc.u1 && pc.v0 <= v && v <= pc.v1).unwrap();
            let sloc = (u - patch.u0) / (patch.u1 - patch.u0);
            let tloc = (v - patch.v0) / (patch.v1 - patch.v0);
            prop_assert!((patch.eval(sloc, tloc) - p0).norm() < 1e-8 * scale);
        }

        // De Boor against the basis-dot-net oracle.
        #[test]
        fn de_boor_matches_basis_dot((s, u, v) in arb_surface_uv()) {
            let p = s.kv_u().degree();
            let q = s.kv_v().degree();
            let su = s.kv_u().find_span(u);
            let sv = s.kv_v().find_span(v);
            let nu_ = crate::basis::basis_funs(s.kv_u(), su, u);
            let nv_ = crate::basis::basis_funs(s.kv_v(), sv, v);
            let mut want = Vec4::ZERO;
            for (i, nui) in nu_.iter().enumerate().take(p + 1) {
                for (j, nvj) in nv_.iter().enumerate().take(q + 1) {
                    want = want
                        + s.homogeneous_control()
                            [(su - p + i) * s.count_v() + (sv - q + j)]
                            * (nui * nvj);
                }
            }
            let got = s.eval_homogeneous(u, v);
            let scale = 1.0 + want.norm();
            prop_assert!((got - want).norm() < 1e-9 * scale);
        }

        // Convex hull property: surface point inside the net AABB.
        #[test]
        fn surface_inside_control_aabb((s, u, v) in arb_surface_uv()) {
            let pt = s.point(u, v);
            let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for c in s.homogeneous_control() {
                let p3 = Vec3::new(c.x / c.w, c.y / c.w, c.z / c.w);
                lo = Vec3::new(lo.x.min(p3.x), lo.y.min(p3.y), lo.z.min(p3.z));
                hi = Vec3::new(hi.x.max(p3.x), hi.y.max(p3.y), hi.z.max(p3.z));
            }
            let tol = 1e-9 * (1.0 + hi.norm() + lo.norm());
            prop_assert!(pt.x >= lo.x - tol && pt.x <= hi.x + tol);
            prop_assert!(pt.y >= lo.y - tol && pt.y <= hi.y + tol);
            prop_assert!(pt.z >= lo.z - tol && pt.z <= hi.z + tol);
        }
    }
}
