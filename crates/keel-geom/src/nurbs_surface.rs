//! Tensor-product NURBS surfaces on homogeneous 4D control nets
//! (spec D4: non-rational algorithms on Vec4, divide last).

use crate::GeomError;
use crate::MAX_ORDER;
use crate::basis::basis_ders;
use crate::knots::KnotVector;
use crate::nurbs_curve::{binom, de_boor_in_place};
use keel_math::interval::Interval;
use keel_math::vec::{Vec3, Vec4};

/// NURBS surface: clamped knot vectors in u and v plus a homogeneous
/// control net, row-major ctrl[i * nv + j] with i the u index (0..nu)
/// and j the v index (0..nv). All weights strictly positive and
/// canonicalized so the maximum lies in (0.5, 1].
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
                        acc = acc - out[k - i][l - j] * (binom(k, i) * binom(l, j) * a[i][j].w);
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
        crate::surface::local_geometry_from_ders(
            d[0][0], d[1][0], d[0][1], d[2][0], d[1][1], d[0][2],
        )
    }

    /// The four boundary iso-curves, with their surface edge and the
    /// curve-parameter direction: [u = u0 (param v), u = u1 (param v),
    /// v = v0 (param u), v = v1 (param u)]. For a clamped surface the
    /// boundary curve's control points are the boundary row/column of
    /// the net.
    pub fn boundary_curves(&self) -> [crate::nurbs_curve::NurbsCurve; 4] {
        use crate::nurbs_curve::NurbsCurve;
        let row =
            |i: usize| -> Vec<Vec4> { (0..self.nv).map(|j| self.ctrl[i * self.nv + j]).collect() };
        let col =
            |j: usize| -> Vec<Vec4> { (0..self.nu).map(|i| self.ctrl[i * self.nv + j]).collect() };
        #[allow(clippy::unwrap_used)]
        [
            NurbsCurve::from_homogeneous(self.kv_v.clone(), row(0)).unwrap(),
            NurbsCurve::from_homogeneous(self.kv_v.clone(), row(self.nu - 1)).unwrap(),
            NurbsCurve::from_homogeneous(self.kv_u.clone(), col(0)).unwrap(),
            NurbsCurve::from_homogeneous(self.kv_u.clone(), col(self.nv - 1)).unwrap(),
        ]
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

    /// Split at `u` into two independent clamped surfaces covering
    /// [u0, u] and [u, u1] in the u direction (parity item 142, the
    /// surface half of the NURBS split/edit toolkit; the curve half is
    /// `NurbsCurve::split`). Geometry is exactly preserved: knot
    /// insertion to full multiplicity, then a net partition.
    pub fn split_u(&self, u: f64) -> Result<(Self, Self), GeomError> {
        let p = self.kv_u.degree();
        let (a, b) = self.kv_u.domain();
        if !(u > a && u < b) {
            return Err(GeomError::OutOfDomain);
        }
        let mut cur = self.clone();
        while cur.kv_u.multiplicity(u) < p {
            cur = cur.insert_knot_u(u)?;
        }
        let span = cur.kv_u.find_span(u);
        let knots = cur.kv_u.knots();
        let i0 = span - p; // control ROW lying on the surface at u
        let mut left_knots = knots[..=span].to_vec();
        left_knots.push(u);
        let mut right_knots = vec![u; p + 1];
        right_knots.extend_from_slice(&knots[span + 1..]);
        let (ctrl, nv) = (&cur.ctrl, cur.nv);
        let row_block = |r0: usize, r1: usize| -> Vec<Vec4> {
            (r0..=r1)
                .flat_map(|i| (0..nv).map(move |j| ctrl[i * nv + j]))
                .collect()
        };
        let left = Self::from_homogeneous(
            KnotVector::new(p, left_knots)?,
            cur.kv_v.clone(),
            row_block(0, i0),
        )?;
        let right = Self::from_homogeneous(
            KnotVector::new(p, right_knots)?,
            cur.kv_v.clone(),
            row_block(i0, cur.nu - 1),
        )?;
        Ok((left, right))
    }

    /// Split at `v` in the v direction (see `split_u`).
    pub fn split_v(&self, v: f64) -> Result<(Self, Self), GeomError> {
        let q = self.kv_v.degree();
        let (a, b) = self.kv_v.domain();
        if !(v > a && v < b) {
            return Err(GeomError::OutOfDomain);
        }
        let mut cur = self.clone();
        while cur.kv_v.multiplicity(v) < q {
            cur = cur.insert_knot_v(v)?;
        }
        let span = cur.kv_v.find_span(v);
        let knots = cur.kv_v.knots();
        let j0 = span - q; // control COLUMN lying on the surface at v
        let mut left_knots = knots[..=span].to_vec();
        left_knots.push(v);
        let mut right_knots = vec![v; q + 1];
        right_knots.extend_from_slice(&knots[span + 1..]);
        let (ctrl, nu, nv) = (&cur.ctrl, cur.nu, cur.nv);
        let col_block = |c0: usize, c1: usize| -> Vec<Vec4> {
            (0..nu)
                .flat_map(|i| (c0..=c1).map(move |j| ctrl[i * nv + j]))
                .collect()
        };
        let left = Self::from_homogeneous(
            cur.kv_u.clone(),
            KnotVector::new(q, left_knots)?,
            col_block(0, j0),
        )?;
        let right = Self::from_homogeneous(
            cur.kv_u.clone(),
            KnotVector::new(q, right_knots)?,
            col_block(j0, cur.nv - 1),
        )?;
        Ok((left, right))
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

    /// Degree-elevate to `(target_u, target_v)` WITHOUT changing the
    /// surface geometry (same point set, more control points). Each
    /// direction is elevated independently; a target equal to the current
    /// degree is a no-op for that direction. Errs when a target is below
    /// the current degree, or above `MAX_DEGREE`.
    ///
    /// Algorithm (per direction, exact on the rational form): saturate
    /// every interior knot to full multiplicity so the net decomposes
    /// into C0-joined Bezier segments; elevate each segment's homogeneous
    /// control coordinates with the Bernstein elevation operator (exact,
    /// and applied to the homogeneous Vec4 it elevates the weights too --
    /// so the RATIONAL surface is preserved); reassemble, raising every
    /// interior knot's multiplicity by the elevation amount
    /// (geometry-identical, though not knot-minimal).
    pub fn elevated_to(&self, target_u: usize, target_v: usize) -> Result<Self, GeomError> {
        let s = self.elevate_u(target_u)?;
        s.elevate_v(target_v)
    }

    /// Degree-elevate the u direction to `target`.
    pub fn elevate_u(&self, target: usize) -> Result<Self, GeomError> {
        let p = self.kv_u.degree();
        if target == p {
            return Ok(self.clone());
        }
        if target < p || target > crate::MAX_DEGREE {
            return Err(GeomError::DegreeOutOfRange);
        }
        let raise = target - p;
        // Saturate interior u-knots -> Bezier segments along u.
        let mut s = self.clone();
        while let Some(k) = next_unsaturated(&s.kv_u) {
            s = s.insert_knot_u(k)?;
        }
        let breaks = distinct_domain_knots(&s.kv_u);
        let nseg = breaks.len() - 1;
        let new_nu = nseg * target + 1; // nseg*(target+1) - (nseg-1)
        let mut ctrl = vec![Vec4::ZERO; new_nu * s.nv];
        let mut lane = [Vec4::ZERO; MAX_ORDER];
        for seg in 0..nseg {
            let src0 = seg * p; // first source row of this segment
            let dst0 = seg * target; // first destination row
            for j in 0..s.nv {
                for (i, slot) in lane.iter_mut().enumerate().take(p + 1) {
                    *slot = s.ctrl[(src0 + i) * s.nv + j];
                }
                let elev = elevate_bezier_lane(&lane[..=p], raise);
                for (i, &pt) in elev.iter().enumerate() {
                    ctrl[(dst0 + i) * s.nv + j] = pt;
                }
            }
        }
        let new_kv = elevated_knots(&breaks, target)?;
        Self::from_homogeneous(new_kv, s.kv_v.clone(), ctrl)
    }

    /// Degree-elevate the v direction to `target`.
    pub fn elevate_v(&self, target: usize) -> Result<Self, GeomError> {
        let q = self.kv_v.degree();
        if target == q {
            return Ok(self.clone());
        }
        if target < q || target > crate::MAX_DEGREE {
            return Err(GeomError::DegreeOutOfRange);
        }
        let raise = target - q;
        let mut s = self.clone();
        while let Some(k) = next_unsaturated(&s.kv_v) {
            s = s.insert_knot_v(k)?;
        }
        let breaks = distinct_domain_knots(&s.kv_v);
        let nseg = breaks.len() - 1;
        let new_nv = nseg * target + 1;
        let mut ctrl = vec![Vec4::ZERO; s.nu * new_nv];
        let mut lane = [Vec4::ZERO; MAX_ORDER];
        for seg in 0..nseg {
            let src0 = seg * q;
            let dst0 = seg * target;
            for i in 0..s.nu {
                for (j, slot) in lane.iter_mut().enumerate().take(q + 1) {
                    *slot = s.ctrl[i * s.nv + (src0 + j)];
                }
                let elev = elevate_bezier_lane(&lane[..=q], raise);
                for (j, &pt) in elev.iter().enumerate() {
                    ctrl[i * new_nv + (dst0 + j)] = pt;
                }
            }
        }
        let new_kv = elevated_knots(&breaks, target)?;
        Self::from_homogeneous(s.kv_u.clone(), new_kv, ctrl)
    }
}

/// Exact full surface of revolution: revolve `profile` about the axis
/// through `origin` along `axis` (NURBS Book 8.* construction). The u
/// direction is the exact 9-point rational full circle (degree 2,
/// double interior knots, shoulder weight sqrt(2)/2, domain `[0,1]`);
/// v is the profile parameter. Weights multiply (tensor structure),
/// so the result is exact: revolving an exact arc yields an exact
/// quadric or torus (kernel/24 forward direction).
pub fn revolve_full(
    profile: &crate::nurbs_curve::NurbsCurve,
    origin: Vec3,
    axis: Vec3,
) -> Result<NurbsSurface, GeomError> {
    if !origin.is_finite() {
        return Err(GeomError::NonFinitePoint);
    }
    let az = axis.try_normalize().ok_or(GeomError::Degenerate)?;
    let prof = profile.homogeneous_control();
    let nv = prof.len();
    const SHOULDER: f64 = core::f64::consts::FRAC_1_SQRT_2;
    let wu = [
        1.0, SHOULDER, 1.0, SHOULDER, 1.0, SHOULDER, 1.0, SHOULDER, 1.0,
    ];
    let mut ctrl = vec![Vec4::ZERO; 9 * nv];
    for (j, pw) in prof.iter().enumerate() {
        let wj = pw.w;
        let pj = Vec3::new(pw.x / wj, pw.y / wj, pw.z / wj);
        let d = pj - origin;
        let o = origin + az * d.dot(az);
        let xv = pj - o;
        let r = xv.norm();
        // On-axis profile points revolve to themselves: a degenerate
        // ring of nine coincident control points (sphere poles).
        let (xu, yu) = if r > 0.0 {
            let xu = xv * (1.0 / r);
            (xu, az.cross(xu))
        } else {
            (Vec3::ZERO, Vec3::ZERO)
        };
        // Nine columns: quadrant points and the tangent-intersection
        // shoulders at o + r(+-X +- Y).
        let pos = [
            o + xu * r,
            o + (xu + yu) * r,
            o + yu * r,
            o + (yu - xu) * r,
            o - xu * r,
            o - (xu + yu) * r,
            o - yu * r,
            o + (xu - yu) * r,
            o + xu * r,
        ];
        for (i, (&w_circ, p)) in wu.iter().zip(&pos).enumerate() {
            let w = w_circ * wj;
            ctrl[i * nv + j] = Vec4::new(p.x * w, p.y * w, p.z * w, w);
        }
    }
    let kv_u = KnotVector::new(
        2,
        vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.],
    )?;
    let kv_v = profile.knot_vector().clone();
    NurbsSurface::from_homogeneous(kv_u, kv_v, ctrl)
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

/// First interior knot of `kv` whose multiplicity is below the degree,
/// or None when every interior knot is already saturated (Bezier form).
fn next_unsaturated(kv: &KnotVector) -> Option<f64> {
    let deg = kv.degree();
    let (a, b) = kv.domain();
    kv.knots()
        .iter()
        .copied()
        .find(|&k| a < k && k < b && kv.multiplicity(k) < deg)
}

/// Clamped knot vector for the elevated direction: ends at multiplicity
/// `target + 1`, each interior break at multiplicity `target`.
fn elevated_knots(breaks: &[f64], target: usize) -> Result<KnotVector, GeomError> {
    let mut knots: Vec<f64> = Vec::new();
    knots.extend(std::iter::repeat_n(breaks[0], target + 1));
    for &bk in &breaks[1..breaks.len() - 1] {
        knots.extend(std::iter::repeat_n(bk, target));
    }
    knots.extend(std::iter::repeat_n(breaks[breaks.len() - 1], target + 1));
    KnotVector::new(target, knots)
}

/// Degree-elevate one Bezier lane (`pts` are the p+1 homogeneous Bezier
/// control points) by `raise`, returning p+1+raise points. Each
/// homogeneous coordinate is an independent Bernstein polynomial; the
/// elevation operator is exact, so the rational lane is preserved.
fn elevate_bezier_lane(pts: &[Vec4], raise: usize) -> Vec<Vec4> {
    let p = pts.len() - 1;
    let target = p + raise;
    #[allow(clippy::unwrap_used)]
    let lift = |coord: &dyn Fn(&Vec4) -> f64| -> Vec<f64> {
        let b = keel_math::bernstein::Bernstein::new(pts.iter().map(coord).collect()).unwrap();
        b.elevated_to(target).coeffs().to_vec()
    };
    let xs = lift(&|v| v.x);
    let ys = lift(&|v| v.y);
    let zs = lift(&|v| v.z);
    let ws = lift(&|v| v.w);
    (0..=target)
        .map(|k| Vec4::new(xs[k], ys[k], zs[k], ws[k]))
        .collect()
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
    /// De Casteljau in s along each column, then in t (local `[0,1]^2`).
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

    /// Certified enclosure of the homogeneous point over a local
    /// (s, t) parameter box: interval de Casteljau, rows in u then a
    /// column in v. Over-wide but always sound (M5 Krawczyk block).
    pub fn eval_homogeneous_interval(&self, s: Interval, t: Interval) -> [Interval; 4] {
        let zero = [Interval::point(0.0); 4];
        let oms = Interval::point(1.0) - s;
        let omt = Interval::point(1.0) - t;
        let mut col: Vec<[Interval; 4]> = vec![zero; self.q + 1];
        for (j, slot) in col.iter_mut().enumerate() {
            let mut lane: Vec<[Interval; 4]> = (0..=self.p)
                .map(|i| {
                    let c = self.ctrl[i * (self.q + 1) + j];
                    [
                        Interval::point(c.x),
                        Interval::point(c.y),
                        Interval::point(c.z),
                        Interval::point(c.w),
                    ]
                })
                .collect();
            let mut len = self.p + 1;
            while len > 1 {
                for i in 0..len - 1 {
                    let next = lane[i + 1];
                    for (cell, nx) in lane[i].iter_mut().zip(next) {
                        *cell = oms * *cell + s * nx;
                    }
                }
                len -= 1;
            }
            *slot = lane[0];
        }
        let mut len = self.q + 1;
        while len > 1 {
            for j in 0..len - 1 {
                let next = col[j + 1];
                for (cell, nx) in col[j].iter_mut().zip(next) {
                    *cell = omt * *cell + t * nx;
                }
            }
            len -= 1;
        }
        col[0]
    }

    /// Certified 3D enclosure over a parameter box; None when the
    /// weight enclosure straddles zero.
    pub fn point_enclosure(&self, s: Interval, t: Interval) -> Option<[Interval; 3]> {
        let h = self.eval_homogeneous_interval(s, t);
        Some([
            h[0].checked_div(h[3])?,
            h[1].checked_div(h[3])?,
            h[2].checked_div(h[3])?,
        ])
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
    use crate::nurbs_curve::NurbsCurve;
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
                assert!(
                    (-1e-12..=3.0 + 1e-12).contains(&p.z),
                    "z {} at {u} {v}",
                    p.z
                );
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
    fn revolved_sphere_is_exact() {
        // Half circle in the xz half-plane (x >= 0): theta = 0 at the
        // south pole, sweeping through (r, 0, 0) to the north pole.
        let r = 2.0;
        let profile = NurbsCurve::circular_arc(
            Vec3::ZERO,
            Vec3::new(0., 0., -1.),
            Vec3::new(1., 0., 0.),
            r,
            core::f64::consts::PI,
        )
        .unwrap();
        let s = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let ((u0, u1), (v0, v1)) = s.domain();
        for i in 0..=8 {
            for j in 0..=8 {
                let u = u0 + (u1 - u0) * i as f64 / 8.0;
                let v = v0 + (v1 - v0) * j as f64 / 8.0;
                let p = s.point(u, v);
                assert!((p.norm() - r).abs() < 1e-12, "sphere residual at {u} {v}");
            }
        }
    }

    #[test]
    fn revolved_torus_is_exact() {
        let (major, minor) = (3.0, 1.0);
        let profile = NurbsCurve::full_circle(
            Vec3::new(major, 0., 0.),
            Vec3::new(1., 0., 0.),
            Vec3::new(0., 0., 1.),
            minor,
        )
        .unwrap();
        let s = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let ((u0, u1), (v0, v1)) = s.domain();
        for i in 0..=8 {
            for j in 0..=8 {
                let u = u0 + (u1 - u0) * i as f64 / 8.0;
                let v = v0 + (v1 - v0) * j as f64 / 8.0;
                let p = s.point(u, v);
                let ring = (p.x * p.x + p.y * p.y).sqrt() - major;
                let res = ring * ring + p.z * p.z - minor * minor;
                assert!(res.abs() < 1e-11, "torus residual {res} at {u} {v}");
            }
        }
    }

    #[test]
    fn revolved_line_is_cylinder_and_cone() {
        // Line parallel to the axis at distance 2 -> cylinder radius 2.
        let cylp = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![Vec3::new(2., 0., 0.), Vec3::new(2., 0., 5.)],
            None,
        )
        .unwrap();
        let cyl = revolve_full(&cylp, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        for i in 0..=8 {
            let p = cyl.point(i as f64 / 8.0, 0.37);
            assert!(((p.x * p.x + p.y * p.y).sqrt() - 2.0).abs() < 1e-13);
        }
        // Slanted line -> cone: radius linear in height.
        let conp = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![Vec3::new(1., 0., 0.), Vec3::new(3., 0., 4.)],
            None,
        )
        .unwrap();
        let con = revolve_full(&conp, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        for j in 0..=8 {
            let p = con.point(0.2, j as f64 / 8.0);
            let want_r = 1.0 + 2.0 * (p.z / 4.0);
            assert!(((p.x * p.x + p.y * p.y).sqrt() - want_r).abs() < 1e-12);
        }
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
        let (kmax, kmin) = (lg.k1.abs().max(lg.k2.abs()), lg.k1.abs().min(lg.k2.abs()));
        assert!((kmax - 0.5).abs() < 1e-10 && kmin < 1e-10);
        // The zero-curvature principal direction runs along the axis.
        let axis_dir = if lg.k1.abs() < lg.k2.abs() {
            lg.dir1
        } else {
            lg.dir2
        };
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
        assert_eq!(
            s.local_geometry(0.5, 0.5).unwrap_err(),
            GeomError::Degenerate
        );
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

        // Interval enclosure soundness: any exact point evaluated at a
        // parameter inside the box lies inside the interval enclosure.
        #[test]
        fn interval_eval_encloses_patch(
            (s, _, _) in arb_surface_uv(),
            s0 in 0.0..1.0f64, ds in 0.0..0.3f64,
            t0 in 0.0..1.0f64, dt in 0.0..0.3f64,
            fs in 0.0..1.0f64, ft in 0.0..1.0f64,
        ) {
            let patches = s.to_bezier_patches().unwrap();
            let pc = &patches[0];
            let s1 = (s0 + ds).min(1.0);
            let t1 = (t0 + dt).min(1.0);
            let si = Interval::new(s0.min(s1), s1.max(s0));
            let ti = Interval::new(t0.min(t1), t1.max(t0));
            let enc = pc.point_enclosure(si, ti).unwrap();
            let sx = si.lo + fs * (si.hi - si.lo);
            let tx = ti.lo + ft * (ti.hi - ti.lo);
            let exact = pc.eval(sx, tx);
            prop_assert!(enc[0].contains(exact.x), "x {} not in {:?}", exact.x, enc[0]);
            prop_assert!(enc[1].contains(exact.y), "y {} not in {:?}", exact.y, enc[1]);
            prop_assert!(enc[2].contains(exact.z), "z {} not in {:?}", exact.z, enc[2]);
        }

        // Splitting is representation-only: the half containing the
        // sample reproduces the surface (parity item 142).
        #[test]
        fn split_preserves_geometry((s, u, v) in arb_surface_uv()) {
            let ((u0, u1), (v0, v1)) = s.domain();
            let um = 0.5 * u0 + 0.5 * u1;
            let vm = 0.5 * v0 + 0.5 * v1;
            let p0 = s.point(u, v);
            let scale = 1.0 + p0.norm();
            let (lu, ru) = s.split_u(um).unwrap();
            let got_u = if u <= um { lu.point(u, v) } else { ru.point(u, v) };
            prop_assert!((got_u - p0).norm() < 1e-9 * scale);
            let (lv, rv) = s.split_v(vm).unwrap();
            let got_v = if v <= vm { lv.point(u, v) } else { rv.point(u, v) };
            prop_assert!((got_v - p0).norm() < 1e-9 * scale);
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
