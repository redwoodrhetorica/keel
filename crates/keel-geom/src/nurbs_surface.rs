//! Tensor-product NURBS surfaces on homogeneous 4D control nets
//! (spec D4: non-rational algorithms on Vec4, divide last).

use crate::GeomError;
use crate::MAX_ORDER;
use crate::knots::KnotVector;
use crate::nurbs_curve::de_boor_in_place;
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

    proptest! {
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
            for i in 0..=p {
                for j in 0..=q {
                    want = want
                        + s.homogeneous_control()
                            [(su - p + i) * s.count_v() + (sv - q + j)]
                            * (nu_[i] * nv_[j]);
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
