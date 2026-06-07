# M2b: NURBS Surfaces, Analytic Surfaces, Multivariate Solver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tensor-product NURBS surfaces with the full second-order derivative contract, first-class analytic surfaces, exact rational surfaces of revolution, certified-capable interval evaluation, global surface point projection, and the multivariate Bernstein subdivision (Projected Polyhedron) solver.

**Architecture:** Mirrors M2a one dimension up. Homogeneous 4D control nets, non-rational algorithms on Vec4 with division last (spec D4). The derivative path switches from hodograph curves to basis-function derivatives (A2.3 + A3.6 + A4.4) because mixed partials make hodograph chains expensive and the basis route is what fitting and IGA-style consumers need anyway. SurfaceLocalGeometry delivers E,F,G,L,M,N, K, H, principal curvatures and directions in one call (M5 Krawczyk tracing and kernel/24 canonical recovery both consume it). The PP solver lives in keel-math and is the global root backend for M4/M5.

**Tech Stack:** Rust workspace (keel-math, keel-geom), proptest, criterion, cargo-fuzz via WSL nightly.

**Research basis:** docs/research/nurbs/02 (A2.3, A3.5/A3.6, A4.3/A4.4, A5.3), nurbs/04 section 4 (Projected Polyhedron), kernel/06 (interrogation contract), kernel/24 (canonical recovery: exact fundamental forms + exact revolution surfaces), spec D4/D6.

**Carried lessons (M1/M2a fuzz campaign, findings 1-6):** power-of-two normalization for scale-pathological input (weights AND solver coefficients); reject unrepresentable weight ratios; finiteness checks on all constructor inputs; multiplicity caps at validation with `new_hodograph` for internal representations; overflow-safe midpoints `0.5*a + 0.5*b`; zero-fill derivatives beyond degree, never truncate.

**Branch:** `m2b-surfaces` off master. One commit per task minimum.

---

## File Structure

- Modify: `crates/keel-geom/src/basis.rs` (add `basis_ders`, A2.3)
- Modify: `crates/keel-geom/src/nurbs_curve.rs` (extract `de_boor_in_place`, share `binom`, BezierSegment interval eval)
- Create: `crates/keel-geom/src/nurbs_surface.rs` (NurbsSurface, BezierPatch, revolve_full)
- Create: `crates/keel-geom/src/surface.rs` (Frame3, Plane3, Cylinder3, Cone3, Sphere3, Torus3, Surface3, SurfaceLocalGeometry shared helper)
- Modify: `crates/keel-geom/src/project.rs` (surface projection; share `dist_to_aabb`)
- Modify: `crates/keel-geom/src/lib.rs` (module decls, error variants if needed)
- Modify: `crates/keel-math/src/interval.rs` (add checked `div`)
- Create: `crates/keel-math/src/multibernstein.rs` (MultiBernstein, solve_system)
- Modify: `crates/keel-math/src/lib.rs` (module decl)
- Create: `fuzz/fuzz_targets/fuzz_nurbs_surface.rs`
- Modify: `fuzz/Cargo.toml`, `crates/keel-geom/benches/geom.rs`

---

### Task 1: Basis function derivatives (A2.3)

**Files:**
- Modify: `crates/keel-geom/src/basis.rs`

- [ ] **Step 1: Write the failing tests** (append to basis.rs tests module)

```rust
#[test]
fn ders_row_zero_matches_basis_funs() {
    let kv = KnotVector::new(3, vec![0., 0., 0., 0., 0.4, 0.7, 1., 1., 1., 1.]).unwrap();
    for &u in &[0.0, 0.2, 0.4, 0.55, 0.9, 1.0] {
        let span = kv.find_span(u);
        let n = basis_funs(&kv, span, u);
        let d = basis_ders(&kv, span, u, 2);
        for j in 0..=3 {
            assert!((d[0][j] - n[j]).abs() < 1e-15, "u={u} j={j}");
        }
    }
}

#[test]
fn ders_sum_to_zero_for_k_ge_1() {
    // Partition of unity differentiates to zero.
    let kv = KnotVector::new(2, vec![0., 0., 0., 0.5, 1., 1., 1.]).unwrap();
    let u = 0.3;
    let span = kv.find_span(u);
    let d = basis_ders(&kv, span, u, 2);
    for k in 1..=2 {
        let s: f64 = (0..=2).map(|j| d[k][j]).sum();
        assert!(s.abs() < 1e-12, "k={k} sum={s}");
    }
}

#[test]
fn ders_beyond_degree_are_zero() {
    let kv = KnotVector::new(1, vec![0., 0., 1., 1.]).unwrap();
    let d = basis_ders(&kv, kv.find_span(0.5), 0.5, 3);
    for j in 0..=1 {
        assert_eq!(d[2][j], 0.0);
        assert_eq!(d[3][j], 0.0);
    }
}

proptest! {
    // Finite-difference oracle on first derivatives. h = 1e-5 with a
    // 1e-4-scaled tolerance: stay above the f64 cancellation floor
    // (M2a lesson: h too small makes FD noise, not signal).
    #[test]
    fn first_ders_match_finite_difference(u in 0.05..0.95f64) {
        let kv = KnotVector::new(3, vec![0., 0., 0., 0., 0.3, 0.6, 1., 1., 1., 1.]).unwrap();
        let h = 1e-5;
        let span = kv.find_span(u);
        // Same span for u-h and u+h or FD compares different functions.
        prop_assume!(kv.find_span(u - h) == span && kv.find_span(u + h) == span);
        let d = basis_ders(&kv, span, u, 1);
        let a = basis_funs(&kv, span, u - h);
        let b = basis_funs(&kv, span, u + h);
        for j in 0..=3 {
            let fd = (b[j] - a[j]) / (2.0 * h);
            prop_assert!((d[1][j] - fd).abs() < 1e-4 * (1.0 + fd.abs()));
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p keel-geom basis` -- expect compile error: `basis_ders` not found.

- [ ] **Step 3: Implement A2.3** (in basis.rs; transcription of The NURBS Book DersBasisFuns with the zero-fill-beyond-degree convention)

```rust
/// All nonzero basis functions and derivatives up to order `d` at `u`
/// (NURBS Book A2.3). Returns ders[k][j] = (d^k/du^k) N_{span-p+j, p}(u)
/// for k in 0..=d, j in 0..=p. Rows beyond the degree are zero (the
/// derivative of a degree-p polynomial vanishes; M2a finding: zero-fill,
/// never truncate the output array).
pub fn basis_ders(kv: &KnotVector, span: usize, u: f64, d: usize) -> Vec<[f64; MAX_ORDER]> {
    let p = kv.degree();
    let knots = kv.knots();
    let mut ndu = [[0.0f64; MAX_ORDER]; MAX_ORDER];
    let mut left = [0.0f64; MAX_ORDER];
    let mut right = [0.0f64; MAX_ORDER];
    ndu[0][0] = 1.0;
    for j in 1..=p {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            // Lower triangle: knot differences. Upper: basis values.
            ndu[j][r] = right[r + 1] + left[j - r];
            let temp = ndu[r][j - 1] / ndu[j][r];
            ndu[r][j] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        ndu[j][j] = saved;
    }
    let mut ders = vec![[0.0f64; MAX_ORDER]; d + 1];
    for j in 0..=p {
        ders[0][j] = ndu[j][p];
    }
    let dcap = d.min(p);
    let mut a = [[0.0f64; MAX_ORDER]; 2];
    for r in 0..=p {
        let (mut s1, mut s2) = (0usize, 1usize);
        a[0][0] = 1.0;
        a[1][0] = 0.0;
        for k in 1..=dcap {
            let mut dv = 0.0;
            let rk = r as i64 - k as i64;
            let pk = (p - k) as i64;
            if r >= k {
                a[s2][0] = a[s1][0] / ndu[(pk + 1) as usize][rk as usize];
                dv = a[s2][0] * ndu[rk as usize][pk as usize];
            }
            let j1 = if rk >= -1 { 1 } else { -rk };
            let j2 = if r as i64 - 1 <= pk { k as i64 - 1 } else { p as i64 - r as i64 };
            for j in j1..=j2 {
                let ju = j as usize;
                a[s2][ju] = (a[s1][ju] - a[s1][ju - 1]) / ndu[(pk + 1) as usize][(rk + j) as usize];
                dv += a[s2][ju] * ndu[(rk + j) as usize][pk as usize];
            }
            if r as i64 <= pk {
                a[s2][k] = -a[s1][k - 1] / ndu[(pk + 1) as usize][r];
                dv += a[s2][k] * ndu[r][pk as usize];
            }
            ders[k][r] = dv;
            std::mem::swap(&mut s1, &mut s2);
        }
    }
    // Multiply through by p! / (p - k)!.
    let mut factor = p as f64;
    for k in 1..=dcap {
        for j in 0..=p {
            ders[k][j] *= factor;
        }
        factor *= (p - k) as f64;
    }
    ders
}
```

- [ ] **Step 4: Run tests** -- `cargo test -p keel-geom basis` expect PASS.

- [ ] **Step 5: Commit** -- `git add -A && git commit -m "feat(geom): basis function derivatives (A2.3)"`

---

### Task 2: NurbsSurface core (struct, validation, evaluation)

**Files:**
- Modify: `crates/keel-geom/src/nurbs_curve.rs` (extract shared helpers)
- Create: `crates/keel-geom/src/nurbs_surface.rs`
- Modify: `crates/keel-geom/src/lib.rs` (add `pub mod nurbs_surface;`)

- [ ] **Step 1: Extract shared de Boor helper.** In nurbs_curve.rs, replace the body of `eval_homogeneous`'s corner-cutting loop with a call to a new pub(crate) free function, and make `binom` pub(crate):

```rust
/// In-place de Boor corner cutting (A3.1 inner loops): d[0..=p] holds
/// the affected control points for `span`; result lands in d[p].
pub(crate) fn de_boor_in_place(knots: &[f64], p: usize, span: usize, u: f64, d: &mut [Vec4]) {
    for r in 1..=p {
        for i in (r..=p).rev() {
            let gi = span - p + i;
            let denom = knots[gi + p + 1 - r] - knots[gi];
            let a = if denom == 0.0 { 0.0 } else { (u - knots[gi]) / denom };
            d[i] = d[i - 1] * (1.0 - a) + d[i] * a;
        }
    }
}
```

`eval_homogeneous` becomes: clamp, find span, copy the p+1 points, call helper, return `d[p]`. Run `cargo test -p keel-geom` to confirm no regression before proceeding.

- [ ] **Step 2: Write failing surface tests** (new file nurbs_surface.rs with tests module)

```rust
#[test]
fn construction_validates() {
    // 2x2 bilinear patch.
    let pts = vec![
        Vec3::ZERO, Vec3::new(0., 1., 0.),
        Vec3::new(1., 0., 0.), Vec3::new(1., 1., 1.),
    ];
    let k = vec![0., 0., 1., 1.];
    assert!(NurbsSurface::new(1, 1, k.clone(), k.clone(), pts.clone(), None).is_ok());
    assert_eq!(
        NurbsSurface::new(1, 1, k.clone(), k.clone(), pts.clone(), Some(vec![1.0; 3])).unwrap_err(),
        GeomError::CountMismatch
    );
    assert_eq!(
        NurbsSurface::new(1, 1, k.clone(), k.clone(), pts.clone(), Some(vec![1., 1., -1., 1.])).unwrap_err(),
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
        Vec3::ZERO, Vec3::new(0., 2., 0.),
        Vec3::new(2., 0., 0.), Vec3::new(2., 2., 2.),
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
        Vec3::new(big, 0., 0.), Vec3::new(0., big, 0.),
        Vec3::new(big, big, 0.), Vec3::new(0., 0., big),
    ];
    let k = vec![0., 0., 1., 1.];
    let s = NurbsSurface::new(1, 1, k.clone(), k, pts, Some(vec![big; 4])).unwrap();
    assert!(s.point(0.5, 0.5).is_finite());
    for h in s.homogeneous_control() {
        assert!(h.w > 0.5 && h.w <= 1.0);
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
                    + s.homogeneous_control()[(su - p + i) * s.count_v() + (sv - q + j)]
                        * (nu_[i] * nv_[j]);
            }
        }
        let got = s.eval_homogeneous(u, v);
        let scale = 1.0 + want.norm();
        prop_assert!((got - want).norm() < 1e-9 * scale);
    }

    // Convex hull property: surface point inside the control net AABB.
    #[test]
    fn surface_inside_control_aabb((s, u, v) in arb_surface_uv()) {
        let pt = s.point(u, v);
        let (mut lo, mut hi) = (Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
                                Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY));
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
```

With a `test_support` module providing `arb_surface_uv()` mirroring M2a's `arb_nurbs` (degrees 1..=3 per direction, 0..=2 extra interior knots with multiplicity 1, control coords in -10..10, weights in 0.2..5.0, u/v drawn in the domain). Mark it `pub(crate)` for reuse by later tasks.

- [ ] **Step 3: Run to verify failure** -- compile error, type not found.

- [ ] **Step 4: Implement NurbsSurface**

```rust
//! Tensor-product NURBS surfaces on homogeneous 4D control nets
//! (spec D4: non-rational algorithms on Vec4, divide last).

use crate::GeomError;
use crate::basis::basis_ders;
use crate::knots::KnotVector;
use crate::nurbs_curve::de_boor_in_place;
use crate::{MAX_ORDER};
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
        // finding 5): exact power-of-two scale so max weight is in
        // (0.5, 1]; split into two factors to stay finite; weights
        // leaving the normal range are an unrepresentable ratio.
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
        Ok(Self { kv_u, kv_v, ctrl, nu, nv })
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
        Ok(Self { kv_u, kv_v, ctrl, nu, nv })
    }

    #[inline]
    pub fn kv_u(&self) -> &KnotVector { &self.kv_u }
    #[inline]
    pub fn kv_v(&self) -> &KnotVector { &self.kv_v }
    #[inline]
    pub fn count_u(&self) -> usize { self.nu }
    #[inline]
    pub fn count_v(&self) -> usize { self.nv }
    #[inline]
    pub fn homogeneous_control(&self) -> &[Vec4] { &self.ctrl }
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
        for j in 0..=q {
            let gj = sv - q + j;
            for i in 0..=p {
                row[i] = self.ctrl[(su - p + i) * self.nv + gj];
            }
            de_boor_in_place(self.kv_u.knots(), p, su, u, &mut row[..=p]);
            col[j] = row[p];
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
```

Note: `de_boor_in_place` is called with a span computed against the FULL knot vector, but `row[..=p]` is a slice starting at local index 0. Reconcile by keeping the helper's indexing global (as extracted from the curve) and passing slices whose element k corresponds to global control index span - p + k. The helper indexes `d[i]` locally and `knots[gi]` globally, which is exactly the curve's existing arrangement.

- [ ] **Step 5: Run tests** -- `cargo test -p keel-geom nurbs_surface` expect PASS.

- [ ] **Step 6: Commit** -- `git commit -m "feat(geom): tensor-product NURBS surface core (A3.5)"`

---

### Task 3: Surface derivatives (A3.6 homogeneous + A4.4 rational)

**Files:**
- Modify: `crates/keel-geom/src/nurbs_surface.rs`
- Modify: `crates/keel-geom/src/nurbs_curve.rs` (make `binom` pub(crate) if not already)

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn derivatives_of_bilinear() {
    // S(u,v) = (2u, 2v, 2uv): S_u = (2,0,2v), S_v = (0,2,2u), S_uv = (0,0,2).
    let pts = vec![
        Vec3::ZERO, Vec3::new(0., 2., 0.),
        Vec3::new(2., 0., 0.), Vec3::new(2., 2., 2.),
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
}
```

Plus a rational sanity test: build a quarter-cylinder patch by extruding `NurbsCurve::circular_arc` along z (degree-1 v direction, two copies of the arc's homogeneous control offset by z, weights replicated), then check `derivatives(u, v, 1)[0][1]` equals the extrusion direction exactly and `d[1][0]` is tangent to the circle (perpendicular to the radial direction) at 1e-12.

- [ ] **Step 2: Run to verify failure** -- `derivatives` not found.

- [ ] **Step 3: Implement**

```rust
impl NurbsSurface {
    /// Homogeneous partial derivatives A^(k,l) for k+l-indexed orders
    /// up to d, via basis-function derivatives (A3.6 on Vec4). Entries
    /// beyond a direction's degree stay zero.
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
                for i in 0..=p {
                    let gi = su - p + i;
                    let mut inner = Vec4::ZERO;
                    for j in 0..=q {
                        inner = inner + self.ctrl[gi * self.nv + (sv - q + j)] * nv_d[l][j];
                    }
                    acc = acc + inner * nu_d[k][i];
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
                            - out[k - i][l - j]
                                * (binom(k, i) * binom(l, j) * a[i][j].w);
                    }
                }
                out[k][l] = acc * (1.0 / w0);
            }
        }
        out
    }
}
```

(`binom` already exists in nurbs_curve.rs for A4.2; export it pub(crate) and import here.)

- [ ] **Step 4: Run tests** -- expect PASS.

- [ ] **Step 5: Commit** -- `git commit -m "feat(geom): surface derivatives (A3.6 + A4.4)"`

---

### Task 4: SurfaceLocalGeometry (fundamental forms, curvatures, principal directions)

**Files:**
- Create: `crates/keel-geom/src/surface.rs` (the shared helper lives here; analytic types arrive in Task 6)
- Modify: `crates/keel-geom/src/nurbs_surface.rs` (the `local_geometry` method)
- Modify: `crates/keel-geom/src/lib.rs` (add `pub mod surface;`)

- [ ] **Step 1: Write failing tests** (in nurbs_surface.rs, using the extruded-arc cylinder patch from Task 3's test support and a planar patch)

```rust
#[test]
fn plane_patch_local_geometry() {
    let pts = vec![
        Vec3::ZERO, Vec3::new(0., 3., 0.),
        Vec3::new(3., 0., 0.), Vec3::new(3., 3., 0.),
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
    // Quarter cylinder radius 2 about z (extruded exact arc):
    // K = 0, |H| = 1/(2r) = 0.25, principal curvatures {0, +/-1/r}.
    let s = quarter_cylinder_r2(); // test-support builder from Task 3
    let lg = s.local_geometry(0.5, 0.5).unwrap();
    assert!(lg.gaussian.abs() < 1e-10);
    assert!((lg.mean.abs() - 0.25).abs() < 1e-10);
    let (kmax, kmin) = (lg.k1.abs().max(lg.k2.abs()), lg.k1.abs().min(lg.k2.abs()));
    assert!((kmax - 0.5).abs() < 1e-10 && kmin < 1e-10);
    // Principal direction for the zero curvature runs along the axis.
    let axis_dir = if lg.k1.abs() < lg.k2.abs() { lg.dir1 } else { lg.dir2 };
    assert!(axis_dir.cross(Vec3::new(0., 0., 1.)).norm() < 1e-8);
}

#[test]
fn degenerate_normal_is_error() {
    // All control points on one line: S_u x S_v = 0 everywhere.
    let pts = vec![Vec3::ZERO, Vec3::new(1., 0., 0.), Vec3::new(2., 0., 0.), Vec3::new(3., 0., 0.)];
    let k = vec![0., 0., 1., 1.];
    let s = NurbsSurface::new(1, 1, k.clone(), k, pts, None).unwrap();
    assert_eq!(s.local_geometry(0.5, 0.5).unwrap_err(), GeomError::Degenerate);
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** (surface.rs gets the struct and the shared computation; any surface kind that can produce point + first/second partials reuses it)

```rust
//! Analytic surfaces and shared local differential geometry.

use crate::GeomError;
use keel_math::vec::Vec3;

/// Second-order local geometry of a parametric surface at (u, v):
/// derivatives, unit normal, fundamental forms, curvatures, and
/// principal directions. The contract M5 (Krawczyk tracing) and the
/// kernel/24 canonical-recovery service consume.
#[derive(Clone, Debug)]
pub struct SurfaceLocalGeometry {
    pub point: Vec3,
    pub du: Vec3,
    pub dv: Vec3,
    pub duu: Vec3,
    pub duv: Vec3,
    pub dvv: Vec3,
    /// Unit normal, du x dv normalized.
    pub normal: Vec3,
    /// First fundamental form E, F, G.
    pub e: f64,
    pub f: f64,
    pub g: f64,
    /// Second fundamental form L, M, N.
    pub l: f64,
    pub m: f64,
    pub n: f64,
    pub gaussian: f64,
    pub mean: f64,
    /// Principal curvatures, k1 >= k2.
    pub k1: f64,
    pub k2: f64,
    /// Unit principal directions in 3D for k1 and k2. At an umbilic
    /// (k1 == k2 to working precision) any orthonormal tangent pair is
    /// principal; we return the normalized du and its in-plane
    /// perpendicular, deterministically.
    pub dir1: Vec3,
    pub dir2: Vec3,
}

/// Build local geometry from raw derivatives. Degenerate when the
/// normal vanishes (collapsed or singular parameterization).
pub(crate) fn local_geometry_from_ders(
    point: Vec3,
    du: Vec3,
    dv: Vec3,
    duu: Vec3,
    duv: Vec3,
    dvv: Vec3,
) -> Result<SurfaceLocalGeometry, GeomError> {
    let raw_n = du.cross(dv);
    let nn = raw_n.norm();
    let scale = du.norm().max(dv.norm());
    // Relative degeneracy test: |du x dv| tiny against the tangent
    // scale squared means the tangents are parallel or vanish.
    if !(nn > 1e-14 * scale * scale) || scale == 0.0 {
        return Err(GeomError::Degenerate);
    }
    let normal = raw_n * (1.0 / nn);
    let (e, f, g) = (du.dot(du), du.dot(dv), dv.dot(dv));
    let (l, m, n) = (duu.dot(normal), duv.dot(normal), dvv.dot(normal));
    let det1 = e * g - f * f; // == nn^2 > 0 here
    let gaussian = (l * n - m * m) / det1;
    let mean = (e * n - 2.0 * f * m + g * l) / (2.0 * det1);
    // Guard tiny negative discriminants from roundoff at umbilics.
    let disc = (mean * mean - gaussian).max(0.0).sqrt();
    let (k1, k2) = (mean + disc, mean - disc);
    let (dir1, dir2) = principal_dirs(k1, k2, e, f, g, l, m, n, du, dv, normal);
    Ok(SurfaceLocalGeometry {
        point, du, dv, duu, duv, dvv, normal,
        e, f, g, l, m, n, gaussian, mean, k1, k2, dir1, dir2,
    })
}

/// Principal directions: null vectors of (II - k I) in the {du, dv}
/// basis. Pick the larger row for stability; at an umbilic both rows
/// vanish and we fall back to the deterministic orthonormal pair.
#[allow(clippy::too_many_arguments)]
fn principal_dirs(
    k1: f64, k2: f64,
    e: f64, f: f64, g: f64,
    l: f64, m: f64, n: f64,
    du: Vec3, dv: Vec3, normal: Vec3,
) -> (Vec3, Vec3) {
    let tangent_dir = |k: f64| -> Option<Vec3> {
        let (r1a, r1b) = (l - k * e, m - k * f);
        let (r2a, r2b) = (m - k * f, n - k * g);
        let (a, b) = if r1a * r1a + r1b * r1b >= r2a * r2a + r2b * r2b {
            (r1b, -r1a)
        } else {
            (r2b, -r2a)
        };
        let d = du * a + dv * b;
        let dn = d.norm();
        // Reject when the row is numerically zero relative to the
        // form scale: umbilic.
        let row_scale = (e + g).abs() * (1.0 + k.abs());
        if dn > 1e-10 * row_scale { Some(d * (1.0 / dn)) } else { None }
    };
    match (tangent_dir(k1), tangent_dir(k2)) {
        (Some(d1), Some(d2)) => (d1, d2),
        (Some(d1), None) => (d1, normal.cross(d1)),
        (None, Some(d2)) => (d2.cross(normal), d2),
        (None, None) => {
            // Umbilic: deterministic orthonormal tangent pair.
            let d1 = du * (1.0 / du.norm());
            (d1, normal.cross(d1))
        }
    }
}
```

In nurbs_surface.rs:

```rust
impl NurbsSurface {
    /// Full second-order local geometry at (u, v).
    pub fn local_geometry(&self, u: f64, v: f64) -> Result<crate::surface::SurfaceLocalGeometry, GeomError> {
        let d = self.derivatives(u, v, 2);
        crate::surface::local_geometry_from_ders(d[0][0], d[1][0], d[0][1], d[2][0], d[1][1], d[0][2])
    }
}
```

- [ ] **Step 4: Run tests** -- expect PASS.

- [ ] **Step 5: Commit** -- `git commit -m "feat(geom): surface local geometry (fundamental forms, curvatures)"`

---

### Task 5: Directional knot insertion, splitting, Bezier patch decomposition

**Files:**
- Modify: `crates/keel-geom/src/nurbs_surface.rs`

- [ ] **Step 1: Write failing tests**

```rust
proptest! {
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
        // Locate the patch covering (u, v) and evaluate it there.
        let patch = patches.iter().find(|pc|
            pc.u0 <= u && u <= pc.u1 && pc.v0 <= v && v <= pc.v1).unwrap();
        let sloc = (u - patch.u0) / (patch.u1 - patch.u0);
        let tloc = (v - patch.v0) / (patch.v1 - patch.v0);
        prop_assert!((patch.eval(sloc, tloc) - p0).norm() < 1e-8 * scale);
    }
}

#[test]
fn insert_rejects_overfull_multiplicity() {
    let s = quarter_cylinder_r2();
    let ((u0, u1), _) = s.domain();
    let mid = 0.5 * u0 + 0.5 * u1;
    let s1 = s.insert_knot_u(mid).unwrap();
    let s2 = s1.insert_knot_u(mid).unwrap(); // multiplicity = degree 2: ok
    assert_eq!(s2.insert_knot_u(mid).unwrap_err(), GeomError::MultiplicityExceeded);
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement.** Boehm insertion applied across the net; the alphas depend only on the direction's knot vector, so compute once and sweep the other index. Mirror the curve's `insert_knot` checks (in-domain, multiplicity cap against the degree).

```rust
impl NurbsSurface {
    /// Insert `ubar` once into the u knot vector (Boehm A5.3 row form).
    pub fn insert_knot_u(&self, ubar: f64) -> Result<Self, GeomError> {
        let p = self.kv_u.degree();
        let (a, b) = self.kv_u.domain();
        if !(a < ubar && ubar < b) || !ubar.is_finite() {
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
                let alpha = if denom == 0.0 { 0.0 } else { (ubar - knots[i]) / denom };
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
        Ok(Self { kv_u, kv_v: self.kv_v.clone(), ctrl: new_ctrl, nu: self.nu + 1, nv: self.nv })
    }

    /// Insert `vbar` once into the v knot vector (column form).
    pub fn insert_knot_v(&self, vbar: f64) -> Result<Self, GeomError> {
        let q = self.kv_v.degree();
        let (a, b) = self.kv_v.domain();
        if !(a < vbar && vbar < b) || !vbar.is_finite() {
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
                let alpha = if denom == 0.0 { 0.0 } else { (vbar - knots[j]) / denom };
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
        Ok(Self { kv_u: self.kv_u.clone(), kv_v, ctrl: new_ctrl, nu: self.nu, nv: nv2 })
    }

    /// Decompose into rational Bezier patches by saturating every
    /// interior knot to full multiplicity in both directions.
    pub fn to_bezier_patches(&self) -> Result<Vec<BezierPatch>, GeomError> {
        let (p, q) = (self.kv_u.degree(), self.kv_v.degree());
        let mut s = self.clone();
        // Saturate u, then v. Collect distinct interior knots first;
        // re-inserting from the ORIGINAL multiplicities.
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
                        s = if dir_u { s.insert_knot_u(k)? } else { s.insert_knot_v(k)? };
                    }
                    None => break,
                }
            }
        }
        // Now each (span_u, span_v) block of (p+1) x (q+1) control
        // points is a Bezier patch. Distinct spans in order:
        let u_breaks = distinct_domain_knots(&s.kv_u);
        let v_breaks = distinct_domain_knots(&s.kv_v);
        let mut out = Vec::new();
        for (iu, uw) in u_breaks.windows(2).enumerate() {
            for (iv, vw) in v_breaks.windows(2).enumerate() {
                let i0 = iu * p; // first control row of this patch
                let j0 = iv * q;
                let mut ctrl = Vec::with_capacity((p + 1) * (q + 1));
                for i in 0..=p {
                    for j in 0..=q {
                        ctrl.push(s.ctrl[(i0 + i) * s.nv + (j0 + j)]);
                    }
                }
                out.push(BezierPatch {
                    p, q, ctrl,
                    u0: uw[0], u1: uw[1], v0: vw[0], v1: vw[1],
                });
            }
        }
        Ok(out)
    }
}

/// Distinct knot values across the domain, including both ends.
fn distinct_domain_knots(kv: &KnotVector) -> Vec<f64> {
    let mut out = Vec::new();
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
        for j in 0..=self.q {
            let mut lane = [Vec4::ZERO; MAX_ORDER];
            for i in 0..=self.p {
                lane[i] = self.ctrl[i * (self.q + 1) + j];
            }
            let mut len = self.p + 1;
            while len > 1 {
                for i in 0..len - 1 {
                    lane[i] = lane[i] * (1.0 - s) + lane[i + 1] * s;
                }
                len -= 1;
            }
            col[j] = lane[0];
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

    /// AABB of the projected control net (bounds the patch: convex
    /// hull property, positive weights).
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
```

CAUTION on `subdivide_u`/`subdivide_v` lane bookkeeping: `left` keeps row 0 unchanged and gains the de Casteljau left edge at each level; `right` keeps the last row and gains the right edge. The provided loops copy `self` first so the unchanged boundary rows are already correct; verify against the curve's `BezierSegment::subdivide` (same scheme one dimension down) with a test comparing a patch subdivision at s=0.5 to direct evaluation on both halves at a grid of parameters (add it to the test module).

- [ ] **Step 4: Run tests** -- expect PASS.

- [ ] **Step 5: Commit** -- `git commit -m "feat(geom): surface knot insertion and Bezier patch decomposition"`

---

### Task 6: Analytic surfaces (Plane3, Cylinder3, Cone3, Sphere3, Torus3)

**Files:**
- Modify: `crates/keel-geom/src/surface.rs`

Parameterizations (X, Y, Z the frame axes, o the origin):

- Plane: `S = o + u X + v Y`
- Cylinder (radius r): `S = o + r cos(u) X + r sin(u) Y + v Z`
- Cone (base radius r0 at v = 0, half-angle alpha, m = tan(alpha)): `S = o + (r0 + v m)(cos u X + sin u Y) + v Z`; apex where r0 + v m = 0
- Sphere (radius r, u azimuth, v latitude in (-pi/2, pi/2)): `S = c + r cos(v)(cos u X + sin u Y) + r sin(v) Z`; poles are excluded from local_geometry (parameterization is singular there, Degenerate)
- Torus (major R, minor r, ring: R > r > 0): `S = o + (R + r cos v)(cos u X + sin u Y) + r sin(v) Z`

Projection conventions (determinism doctrine, document on each): points on a symmetry axis project with `u = 0`; the sphere center and torus tube-center circle pick `v = 0` where ambiguous.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn frame_from_z_is_orthonormal() {
    let f = Frame3::from_z(Vec3::ZERO, Vec3::new(1., 2., 3.)).unwrap();
    assert!((f.x.norm() - 1.0).abs() < 1e-14);
    assert!(f.x.dot(f.z).abs() < 1e-14);
    assert!((f.x.cross(f.y) - f.z).norm() < 1e-14);
    assert!(Frame3::from_z(Vec3::ZERO, Vec3::ZERO).is_err());
}

#[test]
fn analytic_curvatures_match_closed_forms() {
    let f = Frame3::from_z(Vec3::new(1., -2., 0.5), Vec3::new(0., 0., 1.)).unwrap();
    // Cylinder r = 2: K = 0, |H| = 1/(2r).
    let cyl = Surface3::Cylinder(Cylinder3::new(f.clone(), 2.0).unwrap());
    let lg = cyl.local_geometry(0.7, 1.3).unwrap();
    assert!(lg.gaussian.abs() < 1e-12);
    assert!((lg.mean.abs() - 0.25).abs() < 1e-12);
    // Sphere r = 3: K = 1/r^2, |H| = 1/r, umbilic everywhere.
    let sph = Surface3::Sphere(Sphere3::new(f.clone(), 3.0).unwrap());
    let lg = sph.local_geometry(0.4, 0.2).unwrap();
    assert!((lg.gaussian - 1.0 / 9.0).abs() < 1e-12);
    assert!((lg.mean.abs() - 1.0 / 3.0).abs() < 1e-12);
    assert!((lg.k1 - lg.k2).abs() < 1e-9);
    // Torus R = 3, r = 1: K = cos(v) / (r (R + r cos v)).
    let tor = Surface3::Torus(Torus3::new(f, 3.0, 1.0).unwrap());
    let v = 0.9f64;
    let lg = tor.local_geometry(1.1, v).unwrap();
    let want_k = v.cos() / (1.0 * (3.0 + v.cos()));
    assert!((lg.gaussian - want_k).abs() < 1e-12);
}

#[test]
fn analytic_projections_are_exact() {
    let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    let cyl = Surface3::Cylinder(Cylinder3::new(f.clone(), 2.0).unwrap());
    let pr = cyl.project(Vec3::new(4.0, 0.0, 5.0)).unwrap();
    assert!((pr.distance - 2.0).abs() < 1e-14);
    assert!((pr.point - Vec3::new(2.0, 0.0, 5.0)).norm() < 1e-14);
    // Axis point: deterministic u = 0 convention.
    let pr = cyl.project(Vec3::new(0.0, 0.0, 1.0)).unwrap();
    assert_eq!(pr.u, 0.0);
    assert!((pr.distance - 2.0).abs() < 1e-14);
    let tor = Surface3::Torus(Torus3::new(f.clone(), 3.0, 1.0).unwrap());
    let pr = tor.project(Vec3::new(5.0, 0.0, 0.0)).unwrap();
    assert!((pr.distance - 1.0).abs() < 1e-14);
    assert!((pr.point - Vec3::new(4.0, 0.0, 0.0)).norm() < 1e-14);
    let con = Surface3::Cone(Cone3::new(f, 1.0, std::f64::consts::FRAC_PI_4).unwrap());
    // Point straight out from the cone wall at v = 0 ... wall direction
    // is (1, 0, 1)/sqrt(2) in the rz half-plane, normal (1, 0, -1)/sqrt(2).
    let d = std::f64::consts::SQRT_2;
    let pr = con.project(Vec3::new(1.0 + 1.0 / d, 0.0, -1.0 / d)).unwrap();
    assert!((pr.distance - 1.0).abs() < 1e-12);
}

proptest! {
    // Projection oracle: the projected point must beat a dense sample.
    #[test]
    fn analytic_projection_beats_dense_sampling(
        px in -6.0..6.0f64, py in -6.0..6.0f64, pz in -3.0..3.0f64,
    ) {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let tor = Surface3::Torus(Torus3::new(f, 3.0, 1.0).unwrap());
        let p = Vec3::new(px, py, pz);
        let pr = tor.project(p).unwrap();
        let tau = std::f64::consts::TAU;
        for i in 0..32 {
            for j in 0..32 {
                let q = tor.point(tau * i as f64 / 32.0, tau * j as f64 / 32.0);
                prop_assert!((q - p).norm() >= pr.distance - 1e-9);
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement.** Full code in surface.rs:

```rust
/// Right-handed orthonormal frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame3 {
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Frame3 {
    /// Build from origin and z axis; x is chosen deterministically as
    /// the normalized rejection of the smallest-component unit vector.
    pub fn from_z(origin: Vec3, z: Vec3) -> Result<Self, GeomError> {
        let zn = z.norm();
        if !(zn > 0.0) || !z.is_finite() {
            return Err(GeomError::Degenerate);
        }
        let z = z * (1.0 / zn);
        // Deterministic helper axis: the global axis least aligned with z.
        let ax = z.x.abs();
        let ay = z.y.abs();
        let az = z.z.abs();
        let h = if ax <= ay && ax <= az {
            Vec3::new(1., 0., 0.)
        } else if ay <= az {
            Vec3::new(0., 1., 0.)
        } else {
            Vec3::new(0., 0., 1.)
        };
        let x_raw = h - z * h.dot(z);
        let x = x_raw * (1.0 / x_raw.norm());
        let y = z.cross(x);
        Ok(Self { origin, x, y, z })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plane3 { pub frame: Frame3 }
#[derive(Clone, Debug, PartialEq)]
pub struct Cylinder3 { pub frame: Frame3, pub radius: f64 }
#[derive(Clone, Debug, PartialEq)]
pub struct Cone3 { pub frame: Frame3, pub radius: f64, pub half_angle: f64 }
#[derive(Clone, Debug, PartialEq)]
pub struct Sphere3 { pub frame: Frame3, pub radius: f64 }
#[derive(Clone, Debug, PartialEq)]
pub struct Torus3 { pub frame: Frame3, pub major: f64, pub minor: f64 }

impl Plane3 {
    pub fn new(frame: Frame3) -> Self { Self { frame } }
}
impl Cylinder3 {
    pub fn new(frame: Frame3, radius: f64) -> Result<Self, GeomError> {
        if !(radius > 0.0) || !radius.is_finite() {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { frame, radius })
    }
}
impl Cone3 {
    /// `radius` at v = 0; half_angle in (-pi/2, 0) u (0, pi/2).
    pub fn new(frame: Frame3, radius: f64, half_angle: f64) -> Result<Self, GeomError> {
        if !(radius > 0.0) || !radius.is_finite() || !half_angle.is_finite()
            || half_angle == 0.0 || half_angle.abs() >= std::f64::consts::FRAC_PI_2
        {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { frame, radius, half_angle })
    }
}
impl Sphere3 {
    pub fn new(frame: Frame3, radius: f64) -> Result<Self, GeomError> {
        if !(radius > 0.0) || !radius.is_finite() {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { frame, radius })
    }
}
impl Torus3 {
    /// Ring torus only: major > minor > 0.
    pub fn new(frame: Frame3, major: f64, minor: f64) -> Result<Self, GeomError> {
        if !(minor > 0.0 && major > minor) || !major.is_finite() {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { frame, major, minor })
    }
}

/// First-class analytic surfaces; never silently splinified (spec D4).
#[derive(Clone, Debug, PartialEq)]
pub enum Surface3 {
    Plane(Plane3),
    Cylinder(Cylinder3),
    Cone(Cone3),
    Sphere(Sphere3),
    Torus(Torus3),
}

/// Result of projecting a point onto a surface.
#[derive(Clone, Debug)]
pub struct SurfaceProjection {
    pub u: f64,
    pub v: f64,
    pub point: Vec3,
    pub distance: f64,
}

/// Raw derivatives bundle through second order.
pub(crate) struct Ders2 {
    pub s: Vec3,
    pub su: Vec3,
    pub sv: Vec3,
    pub suu: Vec3,
    pub suv: Vec3,
    pub svv: Vec3,
}

impl Surface3 {
    pub fn point(&self, u: f64, v: f64) -> Vec3 {
        self.ders2(u, v).s
    }

    pub fn local_geometry(&self, u: f64, v: f64) -> Result<SurfaceLocalGeometry, GeomError> {
        let d = self.ders2(u, v);
        local_geometry_from_ders(d.s, d.su, d.sv, d.suu, d.suv, d.svv)
    }

    pub(crate) fn ders2(&self, u: f64, v: f64) -> Ders2 {
        match self {
            Surface3::Plane(pl) => {
                let f = &pl.frame;
                Ders2 {
                    s: f.origin + f.x * u + f.y * v,
                    su: f.x,
                    sv: f.y,
                    suu: Vec3::ZERO, suv: Vec3::ZERO, svv: Vec3::ZERO,
                }
            }
            Surface3::Cylinder(c) => {
                let f = &c.frame;
                let (cu, su_) = (u.cos(), u.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                Ders2 {
                    s: f.origin + rad * c.radius + f.z * v,
                    su: tan * c.radius,
                    sv: f.z,
                    suu: rad * (-c.radius),
                    suv: Vec3::ZERO,
                    svv: Vec3::ZERO,
                }
            }
            Surface3::Cone(c) => {
                let f = &c.frame;
                let m = c.half_angle.tan();
                let (cu, su_) = (u.cos(), u.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                let r = c.radius + v * m;
                Ders2 {
                    s: f.origin + rad * r + f.z * v,
                    su: tan * r,
                    sv: rad * m + f.z,
                    suu: rad * (-r),
                    suv: tan * m,
                    svv: Vec3::ZERO,
                }
            }
            Surface3::Sphere(sp) => {
                let f = &sp.frame;
                let r = sp.radius;
                let (cu, su_) = (u.cos(), u.sin());
                let (cv, sv_) = (v.cos(), v.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                Ders2 {
                    s: f.origin + (rad * cv + f.z * sv_) * r,
                    su: tan * (r * cv),
                    sv: (f.z * cv - rad * sv_) * r,
                    suu: rad * (-r * cv),
                    suv: tan * (-r * sv_),
                    svv: (rad * cv + f.z * sv_) * (-r),
                }
            }
            Surface3::Torus(t) => {
                let f = &t.frame;
                let (cu, su_) = (u.cos(), u.sin());
                let (cv, sv_) = (v.cos(), v.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                let ring = t.major + t.minor * cv;
                Ders2 {
                    s: f.origin + rad * ring + f.z * (t.minor * sv_),
                    su: tan * ring,
                    sv: rad * (-t.minor * sv_) + f.z * (t.minor * cv),
                    suu: rad * (-ring),
                    suv: tan * (-t.minor * sv_),
                    svv: rad * (-t.minor * cv) + f.z * (-t.minor * sv_),
                }
            }
        }
    }

    /// Exact closest-point projection. Axis-ambiguous inputs resolve
    /// deterministically (u = 0; v = 0 at the sphere center and on the
    /// torus tube-center circle).
    pub fn project(&self, p: Vec3) -> Result<SurfaceProjection, GeomError> {
        match self {
            Surface3::Plane(pl) => {
                let f = &pl.frame;
                let w = p - f.origin;
                let (u, v) = (w.dot(f.x), w.dot(f.y));
                let q = f.origin + f.x * u + f.y * v;
                Ok(SurfaceProjection { u, v, point: q, distance: (p - q).norm() })
            }
            Surface3::Cylinder(c) => {
                let f = &c.frame;
                let w = p - f.origin;
                let h = w.dot(f.z);
                let (rx, ry) = (w.dot(f.x), w.dot(f.y));
                let rr = (rx * rx + ry * ry).sqrt();
                let u = if rr > 0.0 { ry.atan2(rx).rem_euclid(std::f64::consts::TAU) } else { 0.0 };
                let q = f.origin + (f.x * u.cos() + f.y * u.sin()) * c.radius + f.z * h;
                Ok(SurfaceProjection { u, v: h, point: q, distance: (p - q).norm() })
            }
            Surface3::Sphere(sp) => {
                let f = &sp.frame;
                let w = p - f.origin;
                let wn = w.norm();
                if wn == 0.0 {
                    let q = f.origin + f.x * sp.radius;
                    return Ok(SurfaceProjection { u: 0.0, v: 0.0, point: q, distance: sp.radius });
                }
                let dirn = w * (1.0 / wn);
                let (dx, dy, dz) = (dirn.dot(f.x), dirn.dot(f.y), dirn.dot(f.z));
                let u = if dx == 0.0 && dy == 0.0 { 0.0 } else { dy.atan2(dx).rem_euclid(std::f64::consts::TAU) };
                let v = dz.asin();
                let q = f.origin + dirn * sp.radius;
                Ok(SurfaceProjection { u, v, point: q, distance: (wn - sp.radius).abs() })
            }
            Surface3::Torus(t) => {
                let f = &t.frame;
                let w = p - f.origin;
                let h = w.dot(f.z);
                let (rx, ry) = (w.dot(f.x), w.dot(f.y));
                let rr = (rx * rx + ry * ry).sqrt();
                let u = if rr > 0.0 { ry.atan2(rx).rem_euclid(std::f64::consts::TAU) } else { 0.0 };
                let rad = f.x * u.cos() + f.y * u.sin();
                let center = f.origin + rad * t.major; // tube center at this u
                let d = p - center;
                let dn = d.norm();
                let (v, q) = if dn == 0.0 {
                    (0.0, center + rad * t.minor)
                } else {
                    // Meridian coordinates: along rad and along z.
                    let mr = d.dot(rad);
                    let mz = d.dot(f.z);
                    let v = mz.atan2(mr).rem_euclid(std::f64::consts::TAU);
                    (v, center + d * (t.minor / dn))
                };
                Ok(SurfaceProjection { u, v, point: q, distance: (p - q).norm() })
            }
            Surface3::Cone(c) => {
                let f = &c.frame;
                let m = c.half_angle.tan();
                let w = p - f.origin;
                let h = w.dot(f.z);
                let (rx, ry) = (w.dot(f.x), w.dot(f.y));
                let rr = (rx * rx + ry * ry).sqrt();
                let u = if rr > 0.0 { ry.atan2(rx).rem_euclid(std::f64::consts::TAU) } else { 0.0 };
                // Meridian half-plane: profile line (rho, h) = (r0 + t m, t).
                // Project (rr, h) onto it: direction (m, 1)/sqrt(1 + m^2).
                let t = ((rr - c.radius) * m + h) / (1.0 + m * m);
                let q = {
                    let rad = f.x * u.cos() + f.y * u.sin();
                    f.origin + rad * (c.radius + t * m) + f.z * t
                };
                Ok(SurfaceProjection { u, v: t, point: q, distance: (p - q).norm() })
            }
        }
    }
}
```

Note on the cone projection: the foot may land past the apex (negative radius). For M2b this returns the unclamped profile-line projection and documents that the apex-region case (where the true closest point is the apex itself) is refined when trimmed/bounded surfaces arrive in M3; the dense-sampling proptest for the cone must therefore avoid the apex region (sample v away from the apex). Add a `// M3: clamp at apex` marker comment.

- [ ] **Step 4: Run tests** -- expect PASS.

- [ ] **Step 5: Commit** -- `git commit -m "feat(geom): analytic surfaces with exact projection and curvature"`

---

### Task 7: Exact surfaces of revolution (revolve_full)

**Files:**
- Modify: `crates/keel-geom/src/nurbs_surface.rs`

The forward direction of kernel/24 canonical recovery and the M2b exactness oracle: revolving an exact rational profile yields an exactly rational quadric/torus. u is the revolution angle (the 9-column exact full circle, domain [0,1]), v is the profile parameter.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn revolved_sphere_is_exact() {
    // Half-circle profile in the xz plane (around z), revolved.
    let r = 2.0;
    // Exact half circle from circular_arc: center origin, xz plane.
    let profile = NurbsCurve::circular_arc(
        Vec3::ZERO, Vec3::new(1., 0., 0.), Vec3::new(0., 0., 1.),
        r, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2,
    ).unwrap();
    let s = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    for i in 0..=8 {
        for j in 0..=8 {
            let (du, dv) = (i as f64 / 8.0, j as f64 / 8.0);
            let ((u0, u1), (v0, v1)) = s.domain();
            let p = s.point(u0 + du * (u1 - u0), v0 + dv * (v1 - v0));
            assert!((p.norm() - r).abs() < 1e-12, "sphere residual at {du} {dv}");
        }
    }
}

#[test]
fn revolved_torus_is_exact() {
    let (major, minor) = (3.0, 1.0);
    let profile = NurbsCurve::full_circle(
        Vec3::new(major, 0., 0.), Vec3::new(1., 0., 0.), Vec3::new(0., 0., 1.), minor,
    ).unwrap();
    let s = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    for i in 0..=8 {
        for j in 0..=8 {
            let ((u0, u1), (v0, v1)) = s.domain();
            let p = s.point(u0 + (u1 - u0) * i as f64 / 8.0, v0 + (v1 - v0) * j as f64 / 8.0);
            let ring = (p.x * p.x + p.y * p.y).sqrt() - major;
            let res = ring * ring + p.z * p.z - minor * minor;
            assert!(res.abs() < 1e-11, "torus residual {res}");
        }
    }
}

#[test]
fn revolved_line_is_cylinder_and_cone() {
    // Line parallel to the axis at distance 2 -> cylinder radius 2.
    let cylp = NurbsCurve::new(
        1, vec![0., 0., 1., 1.],
        vec![Vec3::new(2., 0., 0.), Vec3::new(2., 0., 5.)], None,
    ).unwrap();
    let cyl = revolve_full(&cylp, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    for i in 0..=8 {
        let p = cyl.point(i as f64 / 8.0, 0.37);
        assert!(((p.x * p.x + p.y * p.y).sqrt() - 2.0).abs() < 1e-13);
    }
    // Slanted line -> cone: radius is linear in height.
    let conp = NurbsCurve::new(
        1, vec![0., 0., 1., 1.],
        vec![Vec3::new(1., 0., 0.), Vec3::new(3., 0., 4.)], None,
    ).unwrap();
    let con = revolve_full(&conp, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    for j in 0..=8 {
        let p = con.point(0.2, j as f64 / 8.0);
        let want_r = 1.0 + 2.0 * (p.z / 4.0);
        assert!(((p.x * p.x + p.y * p.y).sqrt() - want_r).abs() < 1e-12);
    }
}
```

(Adapt the `circular_arc`/`full_circle` calls to the actual M2a signatures; check nurbs_curve.rs before writing.)

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

```rust
/// Exact full surface of revolution: revolve `profile` about the axis
/// through `origin` along `axis` (NURBS Book 8.* construction). The u
/// direction is the exact 9-point rational full circle (degree 2,
/// double interior knots, shoulder weight sqrt(2)/2); v is the profile.
/// Weights multiply (tensor structure), so the result is exact.
pub fn revolve_full(
    profile: &NurbsCurve,
    origin: Vec3,
    axis: Vec3,
) -> Result<NurbsSurface, GeomError> {
    let an = axis.norm();
    if !(an > 0.0) || !axis.is_finite() || !origin.is_finite() {
        return Err(GeomError::Degenerate);
    }
    let az = axis * (1.0 / an);
    let prof = profile.homogeneous_control();
    let nv = prof.len();
    const SHOULDER: f64 = std::f64::consts::FRAC_1_SQRT_2;
    let wu = [1.0, SHOULDER, 1.0, SHOULDER, 1.0, SHOULDER, 1.0, SHOULDER, 1.0];
    let mut ctrl = vec![Vec4::ZERO; 9 * nv];
    for (j, pw) in prof.iter().enumerate() {
        let wj = pw.w;
        let pj = Vec3::new(pw.x / wj, pw.y / wj, pw.z / wj);
        let d = pj - origin;
        let o = origin + az * d.dot(az);
        let xv = pj - o;
        let r = xv.norm();
        let (xu, yu) = if r > 0.0 {
            let xu = xv * (1.0 / r);
            (xu, az.cross(xu))
        } else {
            (Vec3::ZERO, Vec3::ZERO) // on-axis: the ring degenerates to o
        };
        // 9 columns: quadrant points and tangent-intersection shoulders.
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
        for (i, &w_circ) in wu.iter().enumerate() {
            let w = w_circ * wj;
            ctrl[i * nv + j] = Vec4::new(pos[i].x * w, pos[i].y * w, pos[i].z * w, w);
        }
    }
    let kv_u = KnotVector::new(
        2,
        vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.],
    )?;
    let kv_v = profile.knot_vector().clone();
    NurbsSurface::from_homogeneous(kv_u, kv_v, ctrl)
}
```

- [ ] **Step 4: Run tests** -- expect PASS. The sphere/torus residual tolerances (1e-12, 1e-11) verify EXACTNESS up to evaluation roundoff; if they fail by orders of magnitude the construction is wrong, do not loosen them.

- [ ] **Step 5: Commit** -- `git commit -m "feat(geom): exact rational surfaces of revolution"`

---

### Task 8: Global surface point projection

**Files:**
- Modify: `crates/keel-geom/src/project.rs`

Same architecture as the M2a curve projection: Bezier decomposition, branch-and-bound on control AABBs, Newton polish. Share `dist_to_aabb` (make it take `(Vec3, Vec3)` bounds; refactor the curve path to match).

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn projects_onto_revolved_sphere_radially() {
    let profile = /* half circle as in Task 7 */;
    let s = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    let pr = project_point_surface(&s, Vec3::new(5.0, 0.0, 0.0)).unwrap();
    assert!((pr.distance - 3.0).abs() < 1e-7); // r = 2, point at 5
    assert!((pr.point - Vec3::new(2.0, 0.0, 0.0)).norm() < 1e-6);
}

proptest! {
    // Global optimality oracle: dense sampling cannot beat the result.
    #[test]
    fn surface_projection_is_global((s, _, _) in arb_surface_uv(),
                                    px in -12.0..12.0f64, py in -12.0..12.0f64, pz in -12.0..12.0f64) {
        let p = Vec3::new(px, py, pz);
        let pr = project_point_surface(&s, p).unwrap();
        let ((u0, u1), (v0, v1)) = s.domain();
        for i in 0..=24 {
            for j in 0..=24 {
                let q = s.point(u0 + (u1 - u0) * i as f64 / 24.0,
                                v0 + (v1 - v0) * j as f64 / 24.0);
                prop_assert!((q - p).norm() >= pr.distance - 1e-6,
                    "sample beats projection: {} < {}", (q - p).norm(), pr.distance);
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

```rust
use crate::nurbs_surface::{BezierPatch, NurbsSurface};
use crate::surface::SurfaceProjection;

/// Globally project a point onto a NURBS surface: Bezier patch
/// decomposition, best-first branch-and-bound on control AABBs
/// (lower bound: distance to AABB; upper bound: patch midpoint),
/// Newton polish on survivors. Mirrors the curve projector.
pub fn project_point_surface(
    srf: &NurbsSurface,
    p: Vec3,
) -> Result<SurfaceProjection, GeomError> {
    let patches = srf.to_bezier_patches()?;
    let mut best = SurfaceProjection { u: 0.0, v: 0.0, point: Vec3::ZERO, distance: f64::INFINITY };
    // Seed the upper bound from every patch midpoint.
    let mut work: Vec<BezierPatch> = patches;
    for pc in &work {
        consider(srf, pc, 0.5, 0.5, p, &mut best);
    }
    // Best-first via sort-by-bound stack (small N; a heap is overkill
    // until profiling says otherwise).
    const MIN_PARAM: f64 = 1e-4;
    let mut guard = 0usize;
    while let Some(pc) = work.pop() {
        guard += 1;
        if guard > 100_000 {
            break; // pathological subdivision; best-so-far is returned
        }
        let (lo, hi) = pc.control_aabb();
        if dist_to_aabb(p, lo, hi) >= best.distance {
            continue;
        }
        let du = pc.u1 - pc.u0;
        let dv = pc.v1 - pc.v0;
        if du <= MIN_PARAM && dv <= MIN_PARAM {
            consider(srf, &pc, 0.5, 0.5, p, &mut best);
            continue;
        }
        let (a, b) = if du >= dv { pc.subdivide_u(0.5) } else { pc.subdivide_v(0.5) };
        for half in [a, b] {
            consider(srf, &half, 0.5, 0.5, p, &mut best);
            let (lo, hi) = half.control_aabb();
            if dist_to_aabb(p, lo, hi) < best.distance {
                work.push(half);
            }
        }
        // Keep the stack loosely best-first.
        work.sort_by(|x, y| {
            let dx = { let (lo, hi) = x.control_aabb(); dist_to_aabb(p, lo, hi) };
            let dy = { let (lo, hi) = y.control_aabb(); dist_to_aabb(p, lo, hi) };
            dy.total_cmp(&dx)
        });
    }
    Ok(best)
}

/// Evaluate the patch at a local seed, Newton-polish on the full
/// surface clamped to the patch rectangle, and update the best.
fn consider(srf: &NurbsSurface, pc: &BezierPatch, s: f64, t: f64, p: Vec3, best: &mut SurfaceProjection) {
    let mut u = pc.u0 + s * (pc.u1 - pc.u0);
    let mut v = pc.v0 + t * (pc.v1 - pc.v0);
    for _ in 0..30 {
        let d = srf.derivatives(u, v, 2);
        let dl = d[0][0] - p;
        let (su, sv) = (d[1][0], d[0][1]);
        let g1 = dl.dot(su);
        let g2 = dl.dot(sv);
        let j11 = su.dot(su) + dl.dot(d[2][0]);
        let j12 = su.dot(sv) + dl.dot(d[1][1]);
        let j22 = sv.dot(sv) + dl.dot(d[0][2]);
        let det = j11 * j22 - j12 * j12;
        if det.abs() < 1e-300 {
            break;
        }
        // Solve J [du dv]^T = -[g1 g2]^T.
        let du = (-g1 * j22 + g2 * j12) / det;
        let dv = (-g2 * j11 + g1 * j12) / det;
        let nu_ = (u + du).clamp(pc.u0, pc.u1);
        let nv_ = (v + dv).clamp(pc.v0, pc.v1);
        let step = (nu_ - u).abs() + (nv_ - v).abs();
        u = nu_;
        v = nv_;
        if step < 1e-14 * (1.0 + u.abs() + v.abs()) {
            break;
        }
    }
    let q = srf.point(u, v);
    let dist = (q - p).norm();
    if dist < best.distance {
        *best = SurfaceProjection { u, v, point: q, distance: dist };
    }
}
```

Refactor note: `dist_to_aabb(p, lo, hi)` already exists for curves; unify the signature and reuse. Keep the curve projector's behavior identical (its tests stay green).

Performance note (LOG addendum 6): the curve projector's circle case is slow and Selimovic pruning is the known fix. The surface projector inherits the same worst case; record the bench number and defer optimization identically.

- [ ] **Step 4: Run tests** -- expect PASS. The 25x25 oracle is slow in debug; mark the proptest with `#![proptest_config(ProptestConfig { cases: 8, ..Default::default() })]` like the curve oracle.

- [ ] **Step 5: Commit** -- `git commit -m "feat(geom): global surface point projection"`

---

### Task 9: Multivariate Bernstein subdivision solver (Projected Polyhedron)

**Files:**
- Create: `crates/keel-math/src/multibernstein.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod multibernstein;`)

The global polynomial-system solver of nurbs/04 section 4.1 (Sherbrooke-Patrikalakis), spec D6. Consumers: CSI (3 vars), collinear normals and surface self-intersection (4 vars), starting points for M5 SSI. Coefficients carry the M1 power-of-two normalization (fuzz findings 1-3 medicine) since only signs and ratios matter for root location.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn eval_matches_tensor_construction() {
    // f(x, y) = (x^2 - 0.25) + (y^2 - 0.25): Bernstein coeffs are the
    // outer sum of the univariate coefficient vectors.
    let bx = [ -0.25, -0.25, 0.75 ]; // x^2 - 0.25 in Bernstein degree 2
    let by = [ -0.25, -0.25, 0.75 ];
    let mut coeffs = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            coeffs.push(bx[i] + by[j]);
        }
    }
    let f = MultiBernstein::new(vec![2, 2], coeffs).unwrap();
    for &(x, y) in &[(0.0, 0.0), (0.5, 0.5), (0.3, 0.8), (1.0, 0.2)] {
        let want = (x * x - 0.25) + (y * y - 0.25);
        assert!((f.eval(&[x, y]) - want).abs() < 1e-14, "at {x} {y}");
    }
}

#[test]
fn subdivision_preserves_values() {
    let f = MultiBernstein::new(vec![2, 1], vec![1., -2., 0.5, 3., -1., 0.25]).unwrap();
    let (l, r) = f.subdivide(0, 0.3);
    assert!((l.eval(&[0.5, 0.7]) - f.eval(&[0.15, 0.7])).abs() < 1e-13);
    assert!((r.eval(&[0.5, 0.7]) - f.eval(&[0.3 + 0.7 * 0.5, 0.7])).abs() < 1e-13);
}

#[test]
fn solves_circle_line_intersection() {
    // x^2 + y^2 = 0.5 and y = x on [0,1]^2: root at (0.5, 0.5).
    let bx = [ -0.5, -0.5, 0.5 ]; // x^2 - 0.5 ... wait: see construction below
    // f1 = x^2 + y^2 - 0.5: outer sum of (x^2 - 0.25) and (y^2 - 0.25).
    let u = [ -0.25, -0.25, 0.75 ];
    let mut c1 = Vec::new();
    for i in 0..3 { for j in 0..3 { c1.push(u[i] + u[j]); } }
    let f1 = MultiBernstein::new(vec![2, 2], c1).unwrap();
    // f2 = y - x, degree (1,1): coeffs[i][j] = j - i mapped to [0,1].
    let f2 = MultiBernstein::new(vec![1, 1], vec![0., 1., -1., 0.]).unwrap();
    let roots = solve_system(&[f1, f2], 1e-9, 100_000).unwrap();
    assert_eq!(roots.len(), 1, "{roots:?}");
    assert!((roots[0].lo[0] - 0.5).abs() < 1e-6 && (roots[0].lo[1] - 0.5).abs() < 1e-6);
}

#[test]
fn no_roots_for_positive_system() {
    let f = MultiBernstein::new(vec![2, 2], vec![1.; 9]).unwrap();
    assert!(solve_system(&[f], 1e-9, 10_000).unwrap().is_empty());
}

proptest! {
    // Univariate parity with the Bernstein root finder.
    #[test]
    fn univariate_matches_bernstein_roots(a in 0.1..0.9f64, b in 0.1..0.9f64) {
        prop_assume!((a - b).abs() > 1e-3);
        // (x - a)(x - b) in power form, then to Bernstein.
        let p = crate::bernstein::Bernstein::from_power(&[a * b, -(a + b), 1.0]).unwrap();
        let want = p.roots(1e-12);
        // Same polynomial as a 1-variable MultiBernstein. Bernstein
        // coefficients of degree-2 from power [c0, c1, c2]:
        // [c0, c0 + c1/2, c0 + c1 + c2].
        let c = [a * b, a * b - (a + b) / 2.0, a * b - (a + b) + 1.0];
        let f = MultiBernstein::new(vec![2], c.to_vec()).unwrap();
        let got = solve_system(&[f], 1e-9, 10_000).unwrap();
        prop_assert_eq!(got.len(), want.len());
        let mut centers: Vec<f64> = got.iter().map(|r| 0.5 * r.lo[0] + 0.5 * r.hi[0]).collect();
        centers.sort_by(f64::total_cmp);
        for (g, w) in centers.iter().zip(want.iter()) {
            prop_assert!((g - w).abs() < 1e-6);
        }
    }
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

```rust
//! Multivariate polynomials in tensor-product Bernstein form on
//! [0,1]^n and the Projected Polyhedron global root solver
//! (Sherbrooke and Patrikalakis 1993; spec D6). Every step is a
//! conservative convex-hull exclusion: no root is ever lost.

/// Tensor-product Bernstein polynomial. Coefficients row-major with
/// the LAST variable contiguous.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiBernstein {
    degrees: Vec<usize>,
    coeffs: Vec<f64>,
}

/// A certified root enclosure box in [0,1]^n.
#[derive(Clone, Debug)]
pub struct RootBox {
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
}

impl MultiBernstein {
    /// None for empty/mismatched/non-finite input. Coefficients are
    /// canonicalized by an exact power of two (roots depend only on
    /// signs and ratios; M1 fuzz lesson: scale pathologies overflow).
    pub fn new(degrees: Vec<usize>, mut coeffs: Vec<f64>) -> Option<Self> {
        if degrees.is_empty() {
            return None;
        }
        let n: usize = degrees.iter().map(|d| d + 1).product();
        if coeffs.len() != n || coeffs.iter().any(|c| !c.is_finite()) {
            return None;
        }
        let m = coeffs.iter().fold(0.0f64, |acc, c| acc.max(c.abs()));
        if m > 0.0 {
            let e = m.log2().ceil() as i32;
            let h = e / 2;
            let (s1, s2) = (2.0f64.powi(-h), 2.0f64.powi(-(e - h)));
            for c in coeffs.iter_mut() {
                *c = *c * s1 * s2;
            }
        }
        Some(Self { degrees, coeffs })
    }

    pub fn vars(&self) -> usize {
        self.degrees.len()
    }

    fn stride(&self, axis: usize) -> usize {
        self.degrees[axis + 1..].iter().map(|d| d + 1).product()
    }

    /// De Casteljau evaluation, collapsing the last (contiguous) axis
    /// first.
    pub fn eval(&self, x: &[f64]) -> f64 {
        debug_assert_eq!(x.len(), self.vars());
        let mut w = self.coeffs.clone();
        for ax in (0..self.vars()).rev() {
            let t = x[ax];
            let d = self.degrees[ax];
            let blocks = w.len() / (d + 1);
            let mut out = vec![0.0; blocks];
            for (b, slot) in out.iter_mut().enumerate() {
                let seg = &mut w[b * (d + 1)..(b + 1) * (d + 1)];
                let mut len = d + 1;
                while len > 1 {
                    for i in 0..len - 1 {
                        seg[i] = (1.0 - t) * seg[i] + t * seg[i + 1];
                    }
                    len -= 1;
                }
                *slot = seg[0];
            }
            w = out;
        }
        w[0]
    }

    /// Split along `axis` at local parameter t (de Casteljau lanes).
    pub fn subdivide(&self, axis: usize, t: f64) -> (Self, Self) {
        let d = self.degrees[axis];
        let stride = self.stride(axis);
        let lane_span = (d + 1) * stride;
        let outer = self.coeffs.len() / lane_span;
        let mut left = self.coeffs.clone();
        let mut right = self.coeffs.clone();
        let mut lane = vec![0.0; d + 1];
        for o in 0..outer {
            for s in 0..stride {
                let base = o * lane_span + s;
                for (i, l) in lane.iter_mut().enumerate() {
                    *l = self.coeffs[base + i * stride];
                }
                left[base] = lane[0];
                right[base + d * stride] = lane[d];
                for level in 1..=d {
                    for i in 0..=(d - level) {
                        lane[i] = (1.0 - t) * lane[i] + t * lane[i + 1];
                    }
                    left[base + level * stride] = lane[0];
                    right[base + (d - level) * stride] = lane[d - level];
                }
            }
        }
        (
            Self { degrees: self.degrees.clone(), coeffs: left },
            Self { degrees: self.degrees.clone(), coeffs: right },
        )
    }

    /// Per-level (min, max) coefficient envelope along `axis`.
    fn envelope(&self, axis: usize) -> Vec<(f64, f64)> {
        let d = self.degrees[axis];
        let stride = self.stride(axis);
        let mut env = vec![(f64::INFINITY, f64::NEG_INFINITY); d + 1];
        for (idx, &c) in self.coeffs.iter().enumerate() {
            let i = (idx / stride) % (d + 1);
            env[i].0 = env[i].0.min(c);
            env[i].1 = env[i].1.max(c);
        }
        env
    }
}

/// The x-interval of [0,1] where the convex hull of the projected
/// control points {(i/d, c)} can cross zero: where the lower convex
/// chain of the minima is <= 0 AND the upper concave chain of the
/// maxima is >= 0. None when the hull misses the axis (no root).
fn zero_interval(env: &[(f64, f64)]) -> Option<(f64, f64)> {
    let d = env.len() - 1;
    if d == 0 {
        return if env[0].0 <= 0.0 && env[0].1 >= 0.0 { Some((0.0, 1.0)) } else { None };
    }
    let xs: Vec<f64> = (0..=d).map(|i| i as f64 / d as f64).collect();
    let lower = chain(&xs, env, |e| e.0, true);
    let upper = chain(&xs, env, |e| e.1, false);
    let a = below_zero_span(&lower)?;       // lower convex <= 0
    let b = below_zero_span(&negate(&upper))?; // upper concave >= 0
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    if lo <= hi { Some((lo, hi)) } else { None }
}

/// Monotone-chain hull over points already sorted in x. `lower` keeps
/// right turns (convex from below); the flag flips for the upper chain.
fn chain(xs: &[f64], env: &[(f64, f64)], pick: impl Fn(&(f64, f64)) -> f64, lower: bool) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(xs.len());
    for (i, &x) in xs.iter().enumerate() {
        let y = pick(&env[i]);
        while out.len() >= 2 {
            let (x1, y1) = out[out.len() - 2];
            let (x2, y2) = out[out.len() - 1];
            let cross = (x2 - x1) * (y - y1) - (y2 - y1) * (x - x1);
            let keep = if lower { cross < 0.0 } else { cross > 0.0 };
            if keep {
                break;
            }
            out.pop();
        }
        out.push((x, y));
    }
    out
}

fn negate(c: &[(f64, f64)]) -> Vec<(f64, f64)> {
    c.iter().map(|&(x, y)| (x, -y)).collect()
}

/// For a CONVEX piecewise-linear chain, the x-span where y <= 0
/// (convexity makes it a single interval). None when always positive.
fn below_zero_span(chain: &[(f64, f64)]) -> Option<(f64, f64)> {
    let mut lo: Option<f64> = None;
    let mut hi: Option<f64> = None;
    for w in chain.windows(2) {
        let ((x1, y1), (x2, y2)) = (w[0], w[1]);
        if y1 <= 0.0 {
            lo.get_or_insert(x1);
            hi = Some(if y2 <= 0.0 { x2 } else { x1 + (x2 - x1) * (y1 / (y1 - y2)) });
        } else if y2 <= 0.0 {
            let xc = x1 + (x2 - x1) * (y1 / (y1 - y2));
            lo.get_or_insert(xc);
            hi = Some(x2);
        }
    }
    // Single-point chains.
    if chain.len() == 1 && chain[0].1 <= 0.0 {
        return Some((chain[0].0, chain[0].0));
    }
    match (lo, hi) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

/// Projected Polyhedron solve over [0,1]^n. Returns None when the
/// node budget is exhausted (degenerate/continuum solution sets).
pub fn solve_system(
    polys: &[MultiBernstein],
    tol: f64,
    max_nodes: usize,
) -> Option<Vec<RootBox>> {
    if polys.is_empty() {
        return Some(Vec::new());
    }
    let n = polys[0].vars();
    debug_assert!(polys.iter().all(|p| p.vars() == n));
    let mut out: Vec<RootBox> = Vec::new();
    let mut stack = vec![(polys.to_vec(), vec![(0.0f64, 1.0f64); n])];
    let mut nodes = 0usize;
    while let Some((mut ps, mut bx)) = stack.pop() {
        nodes += 1;
        if nodes > max_nodes {
            return None;
        }
        // Contract every axis by the hull projection of every poly.
        let mut alive = true;
        let mut worst_width = 0.0f64;
        for ax in 0..n {
            let mut lo = 0.0f64;
            let mut hi = 1.0f64;
            for p in &ps {
                match zero_interval(&p.envelope(ax)) {
                    None => {
                        alive = false;
                        break;
                    }
                    Some((a, b)) => {
                        lo = lo.max(a);
                        hi = hi.min(b);
                    }
                }
            }
            if !alive || lo > hi {
                alive = false;
                break;
            }
            worst_width = worst_width.max(hi - lo);
            // Crop the polynomials and the global box to [lo, hi].
            if lo > 0.0 || hi < 1.0 {
                let cropped: Vec<MultiBernstein> = ps
                    .iter()
                    .map(|p| {
                        let (_, r) = p.subdivide(ax, lo);
                        let t = if 1.0 - lo > 0.0 { (hi - lo) / (1.0 - lo) } else { 0.0 };
                        let (l, _) = r.subdivide(ax, t.clamp(0.0, 1.0));
                        l
                    })
                    .collect();
                ps = cropped;
                let w = bx[ax].1 - bx[ax].0;
                bx[ax] = (bx[ax].0 + lo * w, bx[ax].0 + hi * w);
            }
        }
        if !alive {
            continue;
        }
        if bx.iter().all(|(a, b)| b - a <= tol) {
            out.push(RootBox {
                lo: bx.iter().map(|x| x.0).collect(),
                hi: bx.iter().map(|x| x.1).collect(),
            });
            continue;
        }
        if worst_width < 0.8 {
            // Good contraction: iterate without splitting.
            stack.push((ps, bx));
            continue;
        }
        // Stalled: bisect the widest axis in ORIGINAL coordinates.
        let ax = (0..n)
            .max_by(|&i, &j| (bx[i].1 - bx[i].0).total_cmp(&(bx[j].1 - bx[j].0)))
            .unwrap();
        let halves: (Vec<MultiBernstein>, Vec<MultiBernstein>) = {
            let subs: Vec<(MultiBernstein, MultiBernstein)> =
                ps.iter().map(|p| p.subdivide(ax, 0.5)).collect();
            (
                subs.iter().map(|s| s.0.clone()).collect(),
                subs.iter().map(|s| s.1.clone()).collect(),
            )
        };
        let mid = 0.5 * bx[ax].0 + 0.5 * bx[ax].1; // overflow-safe midpoint
        let mut bl = bx.clone();
        bl[ax] = (bx[ax].0, mid);
        let mut br = bx;
        br[ax] = (mid, bl[ax].1.max(mid)); // (mid, original hi)
        br[ax].1 = bl[ax].1.max(br[ax].1);
        stack.push((halves.0, bl));
        stack.push((halves.1, br));
    }
    // Merge adjacent boxes (clusters split across bisection planes).
    merge_touching(&mut out, tol);
    Some(out)
}

/// Merge root boxes that touch within 2 * tol per axis (a cluster cut
/// by a bisection plane is one root, not two).
fn merge_touching(boxes: &mut Vec<RootBox>, tol: f64) {
    let mut i = 0;
    while i < boxes.len() {
        let mut j = i + 1;
        let mut merged = false;
        while j < boxes.len() {
            let touch = (0..boxes[i].lo.len()).all(|k| {
                boxes[i].lo[k] <= boxes[j].hi[k] + 2.0 * tol
                    && boxes[j].lo[k] <= boxes[i].hi[k] + 2.0 * tol
            });
            if touch {
                for k in 0..boxes[i].lo.len() {
                    boxes[i].lo[k] = boxes[i].lo[k].min(boxes[j].lo[k]);
                    boxes[i].hi[k] = boxes[i].hi[k].max(boxes[j].hi[k]);
                }
                boxes.remove(j);
                merged = true;
            } else {
                j += 1;
            }
        }
        if !merged {
            i += 1;
        }
    }
}
```

Fix the `br` bookkeeping (the snippet above garbles the right half's upper bound when transcribed; write it cleanly):

```rust
        let (lo_ax, hi_ax) = bx[ax];
        let mid = 0.5 * lo_ax + 0.5 * hi_ax;
        let mut bl = bx.clone();
        bl[ax] = (lo_ax, mid);
        let mut br = bx.clone();
        br[ax] = (mid, hi_ax);
```

Known limitation, document in the module docs: a polynomial that is identically zero on a sub-box (continuum solutions, tangential contact along a curve) cannot contract and will exhaust the node budget; `None` is the deliberate signal to the caller to switch to a dedicated coincidence handler (M5 work, per spec D6).

- [ ] **Step 4: Run tests** -- expect PASS.

- [ ] **Step 5: Commit** -- `git commit -m "feat(math): multivariate Bernstein Projected Polyhedron solver"`

---

### Task 10: Interval evaluation (the M5 Krawczyk building block)

**Files:**
- Modify: `crates/keel-math/src/interval.rs` (checked division)
- Modify: `crates/keel-geom/src/nurbs_curve.rs` (BezierSegment interval eval)
- Modify: `crates/keel-geom/src/nurbs_surface.rs` (BezierPatch interval eval)

Scope discipline: M2b ships certified POINT enclosures over parameter boxes via interval de Casteljau on Bezier leaves. M5 builds interval Jacobians and the Krawczyk operator on top of this plus the hodograph machinery; do not build Krawczyk here.

- [ ] **Step 1: Write failing tests**

```rust
// In interval.rs:
#[test]
fn div_requires_one_signed_divisor() {
    let a = Interval::new(1.0, 2.0);
    assert!(a.div(Interval::new(-1.0, 1.0)).is_none());
    let r = a.div(Interval::new(2.0, 4.0)).unwrap();
    assert!(r.lo <= 0.25 && r.hi >= 1.0);
}

proptest! {
    #[test]
    fn div_is_sound(a in -1e3..1e3f64, b in 0.5..1e3f64) {
        let r = Interval::point(a).div(Interval::point(b)).unwrap();
        prop_assert!(r.lo <= a / b && a / b <= r.hi);
    }
}

// In nurbs_curve.rs tests:
proptest! {
    // Enclosure soundness: any exact point in the parameter interval
    // lies inside the interval evaluation.
    #[test]
    fn interval_eval_encloses_curve((c, _) in arb_nurbs(),
                                    t0 in 0.0..1.0f64, dt in 0.0..0.3f64, s in 0.0..1.0f64) {
        let segs = c.to_beziers().unwrap();
        let seg = &segs[0];
        let t1 = (t0 + dt).min(1.0);
        let ti = Interval::new(t0.min(t1), t0.max(t1));
        let enc = seg.eval_homogeneous_interval(ti);
        let t = ti.lo + s * (ti.hi - ti.lo);
        let exact = seg.eval_homogeneous_at(t); // existing scalar eval path
        prop_assert!(enc[0].contains(exact.x) && enc[1].contains(exact.y)
            && enc[2].contains(exact.z) && enc[3].contains(exact.w));
    }
}
```

(Adapt names to the actual BezierSegment API from M2a; the scalar evaluation entry point exists, reuse it. Mirror the same test for BezierPatch with a (u box, v box) pair.)

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement.** Interval division in interval.rs:

```rust
impl Interval {
    /// Conservative division. None unless the divisor is strictly
    /// one-signed (an interval straddling zero has unbounded quotient).
    pub fn div(self, o: Self) -> Option<Self> {
        if !(o.lo > 0.0 || o.hi < 0.0) {
            return None;
        }
        let p = [self.lo / o.lo, self.lo / o.hi, self.hi / o.lo, self.hi / o.hi];
        let mut lo = p[0];
        let mut hi = p[0];
        for &v in &p[1..] {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        Some(Self { lo, hi }.widened())
    }
}
```

Interval de Casteljau on BezierSegment (and the BezierPatch analog, rows in u then a column in v):

```rust
impl BezierSegment {
    /// Certified enclosure of the homogeneous point over the parameter
    /// interval: interval de Casteljau. Over-wide (t and 1-t are
    /// treated as independent) but always sound.
    pub fn eval_homogeneous_interval(&self, t: Interval) -> [Interval; 4] {
        let one = Interval::point(1.0);
        let omt = one - t;
        let mut w: Vec<[Interval; 4]> = self
            .control()  // adapt to the actual accessor
            .iter()
            .map(|c| [
                Interval::point(c.x),
                Interval::point(c.y),
                Interval::point(c.z),
                Interval::point(c.w),
            ])
            .collect();
        let mut len = w.len();
        while len > 1 {
            for i in 0..len - 1 {
                for k in 0..4 {
                    w[i][k] = omt * w[i][k] + t * w[i + 1][k];
                }
            }
            len -= 1;
        }
        w[0]
    }

    /// Certified 3D enclosure: homogeneous enclosure divided by the
    /// weight interval. None if the weight enclosure straddles zero
    /// (cannot happen for valid positive-weight segments with t inside
    /// [0,1], but the interval arithmetic must prove it, not assume it).
    pub fn point_enclosure(&self, t: Interval) -> Option<[Interval; 3]> {
        let h = self.eval_homogeneous_interval(t);
        Some([h[0].div(h[3])?, h[1].div(h[3])?, h[2].div(h[3])?])
    }
}
```

- [ ] **Step 4: Run tests** -- expect PASS.

- [ ] **Step 5: Commit** -- `git commit -m "feat: interval de Casteljau enclosures (M5 groundwork)"`

---

### Task 11: Fuzz target, benches, final gate

**Files:**
- Create: `fuzz/fuzz_targets/fuzz_nurbs_surface.rs`
- Modify: `fuzz/Cargo.toml`
- Modify: `crates/keel-geom/benches/geom.rs`

- [ ] **Step 1: Fuzz target** (constructor contract identical in spirit to fuzz_nurbs_curve)

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

// The constructor must either reject cleanly or yield a surface whose
// evaluation never panics and is insertion-invariant.
fuzz_target!(|data: (u8, u8, Vec<f64>, Vec<f64>, Vec<(f64, f64, f64)>, Vec<f64>, f64, f64)| {
    let (du, dv, ku, kv, pts, ws, t, s) = data;
    let degree_u = 1 + (du as usize % 5);
    let degree_v = 1 + (dv as usize % 5);
    if ku.len() > 32 || kv.len() > 32 || pts.len() > 256 {
        return;
    }
    let points: Vec<_> = pts
        .iter()
        .map(|&(x, y, z)| keel_math::vec::Vec3::new(x, y, z))
        .collect();
    let result = keel_geom::nurbs_surface::NurbsSurface::new(
        degree_u, degree_v, ku, kv, points, Some(ws),
    );
    let Ok(srf) = result else { return };
    if !t.is_finite() || !s.is_finite() {
        return;
    }
    let ((u0, u1), (v0, v1)) = srf.domain();
    let u = u0 + t.rem_euclid(1.0).clamp(0.0, 1.0) * (u1 - u0);
    let v = v0 + s.rem_euclid(1.0).clamp(0.0, 1.0) * (v1 - v0);
    let p = srf.point(u, v);
    assert!(p.is_finite());
    let _ = srf.derivatives(u, v, 2);
    if let Ok(s2) = srf.insert_knot_u(0.5 * u0 + 0.5 * u1) {
        assert!((s2.point(u, v) - p).norm() <= 1e-6 * (1.0 + p.norm()));
    }
});
```

Add the `[[bin]]` block to fuzz/Cargo.toml mirroring the existing targets.

- [ ] **Step 2: Benches** (append to geom.rs)

```rust
fn bench_surface(c: &mut Criterion) {
    let s = /* revolved torus from Task 7 */;
    c.bench_function("nurbs_torus_point", |b| {
        b.iter(|| black_box(s.point(black_box(0.37), black_box(0.61))))
    });
    c.bench_function("nurbs_torus_local_geometry", |b| {
        b.iter(|| black_box(s.local_geometry(black_box(0.37), black_box(0.61))))
    });
    c.bench_function("surface_project", |b| {
        b.iter(|| black_box(project_point_surface(&s, black_box(Vec3::new(5., 1., 2.)))))
    });
}
```

- [ ] **Step 3: Final gate** (in order; all must pass before merge)

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace` (expect all green; record the count)
4. All M1/M2a fuzz artifacts still pass (WSL: run each crash file individually)
5. WSL: `cargo +nightly fuzz run fuzz_nurbs_surface` for 10 minutes via the established `timeout 700 ... -max_total_time=600` recipe; CLEAN required (findings follow the decode/fix/golden-test loop first)
6. Record bench baselines in the LOG
7. LOG.md addendum: M2b completion entry per the standing mandate

- [ ] **Step 4: Commit, merge to master** following superpowers:finishing-a-development-branch (run the gate, present options).

---

## Self-Review Notes

- **Spec coverage:** D4 (homogeneous policy, analytics first-class) Tasks 2-7; D6 (subdivision solver) Task 9; kernel/06 interrogation contract Task 4; kernel/24 recovery prerequisites Tasks 4, 6, 7; M5 groundwork Task 10. The M2b checklist from LOG addendum 5 (SurfaceLocalGeometry contract AND interval-capable evaluation) is Tasks 4 and 10.
- **Type consistency:** `SurfaceProjection` defined in surface.rs (Task 6), consumed by project.rs (Task 8). `BezierPatch` defined in nurbs_surface.rs (Task 5), consumed by Tasks 8 and 10. `basis_ders` (Task 1) consumed by Task 3. `de_boor_in_place` and `binom` extracted in Task 2/3 from nurbs_curve.rs.
- **Known adaptations for the executor:** exact M2a signatures for `circular_arc`, `full_circle`, `BezierSegment` accessors, and `dist_to_aabb` must be read from the source before Tasks 7, 8, 10; the plan marks each site. These are accessor-name adaptations, not design decisions.
- **Deliberate deferrals (documented in code comments):** cone apex clamping in projection (M3, bounded surfaces); Selimovic pruning for projection performance (when a consumer has a budget); PP solver coincidence handling (M5, spec D6); Krawczyk operator itself (M5).

