# M2a Spline Core and Curves Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `keel-geom`'s curve layer: knot-vector machinery, homogeneous-4D NURBS curves (evaluation, derivatives, knot insertion, splitting, Bezier decomposition, exact circular arcs), analytic curves (line, circle, ellipse), the `Curve3` dispatch enum, and certified-global closest-point projection.

**Architecture:** Spec D4 amendments (wave 2): control points stored as homogeneous `Vec4` (wx, wy, wz, w); every algorithm runs as the NON-rational algorithm on 4D points (de Boor evaluation, Boehm insertion, hodograph derivatives), dividing only at the API boundary. Weights strictly positive. Derivatives via derivative-curve (hodograph) construction rather than the A2.3 a-table (simpler to get right; A2.3 lands with fitting in a later milestone). Closest-point: analytic closed forms where they exist; for NURBS, Bezier-decompose + convex-hull branch-and-bound pruning + bracketed-Newton polish (Ma-Hewitt/Selimovic pattern, research nurbs/04 section 5).

**Tech Stack:** Rust stable, `keel-math` (Vec3/Vec4, Aabb3, solve_bracketed, tolerance), dev-deps proptest/criterion.

**Conventions:** repo root working directory; no `unwrap`/`expect`/`panic!` in library code; every public item documented; no em-dashes anywhere; commit per task; `cargo test -p keel-geom` green before each commit. M2a exit also requires the M1 pending fuzz runs (now executing in WSL) logged in LOG.md.

---

### Task 1: Scaffold keel-geom

**Files:**
- Create: `crates/keel-geom/Cargo.toml`, `crates/keel-geom/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Write the crate**

`crates/keel-geom/Cargo.toml`:
```toml
[package]
name = "keel-geom"
description = "Curves and surfaces for the Keel geometry kernel"
version.workspace = true
edition.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
keel-math = { path = "../keel-math" }

[dev-dependencies]
proptest.workspace = true
criterion.workspace = true

[[bench]]
name = "geom"
harness = false
path = "benches/geom.rs"
```

`crates/keel-geom/src/lib.rs`:
```rust
//! Curves and surfaces for the Keel geometry kernel.
//!
//! Policy (spec D4): NURBS geometry lives in homogeneous 4D; all
//! algorithms are the non-rational ones run on `Vec4` control points,
//! with projection to 3D only at the API boundary. Analytic curve and
//! surface types are first-class and never silently converted to
//! NURBS for computation.

pub mod knots;

/// Maximum supported spline degree (spec D4: cap working degree;
/// Bernstein conditioning grows as p * 2^p, knot-independent).
pub const MAX_DEGREE: usize = 11;
/// MAX_DEGREE + 1: array capacity for one span's basis values.
pub const MAX_ORDER: usize = MAX_DEGREE + 1;

/// Errors from geometry construction and queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeomError {
    /// Degree outside 1..=MAX_DEGREE.
    DegreeOutOfRange,
    /// Knot vector too short for the degree and control count.
    TooFewKnots,
    /// Knots must be nondecreasing and finite.
    InvalidKnots,
    /// End knots must have multiplicity degree + 1 (clamped form).
    NotClamped,
    /// Control-point / weight counts inconsistent with knots.
    CountMismatch,
    /// Weights must be strictly positive and finite.
    InvalidWeight,
    /// Geometry is degenerate (zero direction, zero radius, ...).
    Degenerate,
    /// Parameter outside the curve domain.
    OutOfDomain,
    /// Knot multiplicity would exceed the degree.
    MultiplicityExceeded,
}
```

Add `"crates/keel-geom"` to the workspace `members` array in the root `Cargo.toml`. Create `crates/keel-geom/benches/geom.rs` containing `fn main() {}` as the placeholder.

- [ ] **Step 2: Build and commit**

Run: `cargo build -p keel-geom` (expect success; `knots` module added next task, so temporarily comment the `pub mod knots;` line OR create an empty `knots.rs`. Create the empty file: `crates/keel-geom/src/knots.rs` containing only `//! Knot vectors.`)

```bash
git add Cargo.toml crates/keel-geom
git commit -m "feat(geom): scaffold keel-geom crate"
```

---

### Task 2: Knot vectors

**Files:**
- Modify: `crates/keel-geom/src/knots.rs`

- [ ] **Step 1: Implement with tests**

Replace `knots.rs`:
```rust
//! Clamped knot vectors: validation, span search, multiplicity.

use crate::{GeomError, MAX_DEGREE};

/// A validated, clamped (end multiplicity = degree + 1), nondecreasing
/// knot vector. Immutable after construction.
#[derive(Clone, Debug, PartialEq)]
pub struct KnotVector {
    degree: usize,
    knots: Vec<f64>,
}

impl KnotVector {
    /// Validate and construct. Requirements: 1 <= degree <= MAX_DEGREE,
    /// len >= 2 * (degree + 1), finite nondecreasing knots, clamped
    /// ends, and a nonempty domain.
    pub fn new(degree: usize, knots: Vec<f64>) -> Result<Self, GeomError> {
        if degree < 1 || degree > MAX_DEGREE {
            return Err(GeomError::DegreeOutOfRange);
        }
        if knots.len() < 2 * (degree + 1) {
            return Err(GeomError::TooFewKnots);
        }
        if knots.iter().any(|k| !k.is_finite()) {
            return Err(GeomError::InvalidKnots);
        }
        if knots.windows(2).any(|w| w[1] < w[0]) {
            return Err(GeomError::InvalidKnots);
        }
        let p = degree;
        let m = knots.len() - 1;
        let (a, b) = (knots[0], knots[m]);
        if knots[p] != a || knots[m - p] != b || a >= b {
            return Err(GeomError::NotClamped);
        }
        Ok(Self { degree, knots })
    }

    #[inline]
    pub fn degree(&self) -> usize {
        self.degree
    }
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }
    /// Number of control points this knot vector pairs with.
    #[inline]
    pub fn control_count(&self) -> usize {
        self.knots.len() - self.degree - 1
    }
    /// Parameter domain [a, b].
    #[inline]
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[0], self.knots[self.knots.len() - 1])
    }
    /// Clamp a parameter into the domain.
    #[inline]
    pub fn clamp(&self, u: f64) -> f64 {
        let (a, b) = self.domain();
        u.clamp(a, b)
    }

    /// Knot span index k with knots[k] <= u < knots[k+1] (NURBS Book
    /// A2.1). The end parameter returns the last nonempty span.
    pub fn find_span(&self, u: f64) -> usize {
        let p = self.degree;
        let n = self.control_count() - 1;
        let u = self.clamp(u);
        if u >= self.knots[n + 1] {
            return n;
        }
        let mut lo = p;
        let mut hi = n + 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if u < self.knots[mid] { hi = mid } else { lo = mid }
        }
        lo
    }

    /// Multiplicity of value `u` among the knots (exact comparison).
    pub fn multiplicity(&self, u: f64) -> usize {
        self.knots.iter().filter(|&&k| k == u).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(p: usize, k: &[f64]) -> KnotVector {
        KnotVector::new(p, k.to_vec()).unwrap()
    }

    #[test]
    fn validation_rejects_bad_input() {
        assert_eq!(
            KnotVector::new(0, vec![0., 0., 1., 1.]).unwrap_err(),
            GeomError::DegreeOutOfRange
        );
        assert_eq!(
            KnotVector::new(1, vec![0., 0., 1.]).unwrap_err(),
            GeomError::TooFewKnots
        );
        assert_eq!(
            KnotVector::new(1, vec![0., 0., 1., 0.5]).unwrap_err(),
            GeomError::InvalidKnots
        );
        assert_eq!(
            KnotVector::new(2, vec![0., 0., 0.5, 1., 1., 1.]).unwrap_err(),
            GeomError::NotClamped
        );
        assert_eq!(
            KnotVector::new(1, vec![0., 0., 0., 0.]).unwrap_err(),
            GeomError::NotClamped
        );
    }

    #[test]
    fn span_search_basics() {
        // Degree 2, knots {0,0,0, 1, 2,2,2}: 4 control points.
        let k = kv(2, &[0., 0., 0., 1., 2., 2., 2.]);
        assert_eq!(k.control_count(), 4);
        assert_eq!(k.domain(), (0.0, 2.0));
        assert_eq!(k.find_span(0.0), 2);
        assert_eq!(k.find_span(0.5), 2);
        assert_eq!(k.find_span(1.0), 3);
        assert_eq!(k.find_span(1.5), 3);
        // End parameter maps into the last nonempty span.
        assert_eq!(k.find_span(2.0), 3);
        // Out-of-domain input is clamped.
        assert_eq!(k.find_span(99.0), 3);
        assert_eq!(k.find_span(-1.0), 2);
    }

    #[test]
    fn multiplicity_counts() {
        let k = kv(2, &[0., 0., 0., 1., 1., 2., 2., 2.]);
        assert_eq!(k.multiplicity(0.0), 3);
        assert_eq!(k.multiplicity(1.0), 2);
        assert_eq!(k.multiplicity(0.3), 0);
    }
}
```

- [ ] **Step 2: Test and commit**

Run: `cargo test -p keel-geom`
Expected: all pass.

```bash
git add crates/keel-geom
git commit -m "feat(geom): validated clamped knot vectors with span search"
```

---

### Task 3: Basis functions (A2.2)

**Files:**
- Create: `crates/keel-geom/src/basis.rs`
- Modify: `lib.rs` (add `pub mod basis;`)

- [ ] **Step 1: Implement with tests**

`basis.rs`:
```rust
//! B-spline basis evaluation (NURBS Book A2.2), stack-allocated.

use crate::MAX_ORDER;
use crate::knots::KnotVector;

/// All degree+1 nonzero basis values N_{span-p..=span, p}(u), in
/// `out[0..=p]` (NURBS Book A2.2). Convex corner-cutting: every output
/// is in [0,1] and they sum to 1 (partition of unity).
pub fn basis_funs(kv: &KnotVector, span: usize, u: f64) -> [f64; MAX_ORDER] {
    let p = kv.degree();
    let knots = kv.knots();
    let mut n = [0.0; MAX_ORDER];
    let mut left = [0.0; MAX_ORDER];
    let mut right = [0.0; MAX_ORDER];
    n[0] = 1.0;
    for j in 1..=p {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let temp = n[r] / (right[r + 1] + left[j - r]);
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knots::KnotVector;
    use proptest::prelude::*;

    #[test]
    fn degree_one_is_hat_function() {
        let kv = KnotVector::new(1, vec![0., 0., 1., 2., 2.]).unwrap();
        let n = basis_funs(&kv, kv.find_span(0.25), 0.25);
        assert!((n[0] - 0.75).abs() < 1e-15);
        assert!((n[1] - 0.25).abs() < 1e-15);
    }

    #[test]
    fn matches_bernstein_on_single_span() {
        // Clamped single-span degree 3 = cubic Bernstein basis.
        let kv = KnotVector::new(3, vec![0., 0., 0., 0., 1., 1., 1., 1.]).unwrap();
        let t = 0.3_f64;
        let n = basis_funs(&kv, kv.find_span(t), t);
        let b = [
            (1.0 - t).powi(3),
            3.0 * t * (1.0 - t).powi(2),
            3.0 * t * t * (1.0 - t),
            t.powi(3),
        ];
        for i in 0..4 {
            assert!((n[i] - b[i]).abs() < 1e-15, "i={i}");
        }
    }

    proptest! {
        #[test]
        fn partition_of_unity(u in 0.0..2.0f64) {
            let kv = KnotVector::new(
                3,
                vec![0., 0., 0., 0., 0.7, 1.3, 2., 2., 2., 2.],
            ).unwrap();
            let n = basis_funs(&kv, kv.find_span(u), u);
            let sum: f64 = n[..4].iter().sum();
            prop_assert!((sum - 1.0).abs() < 1e-12);
            prop_assert!(n[..4].iter().all(|&v| (-1e-15..=1.0 + 1e-12).contains(&v)));
        }
    }
}
```

- [ ] **Step 2: Test and commit**

Run: `cargo test -p keel-geom`. Expected: pass.

```bash
git add crates/keel-geom
git commit -m "feat(geom): B-spline basis evaluation with partition-of-unity property test"
```

---

### Task 4: NURBS curve core and evaluation

**Files:**
- Create: `crates/keel-geom/src/nurbs_curve.rs`
- Modify: `lib.rs` (add `pub mod nurbs_curve;`)

- [ ] **Step 1: Implement with tests**

`nurbs_curve.rs` (core + evaluation + exact arcs):
```rust
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

    /// Exact circular arc (NURBS Book A7.1 pattern): center, two unit
    /// orthogonal in-plane axes, radius, sweep angle in (0, 2*pi].
    /// Built from ceil(sweep / (pi/2)) rational quadratic segments
    /// with interior weights cos(d_theta / 2).
    pub fn circular_arc(
        center: Vec3,
        x_axis: Vec3,
        y_axis: Vec3,
        radius: f64,
        sweep: f64,
    ) -> Result<Self, GeomError> {
        if !(radius.is_finite() && radius > 0.0) || !(sweep > 0.0 && sweep <= core::f64::consts::TAU) {
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
        let on_circle = |theta: f64| center + x * (radius * theta.cos()) + y * (radius * theta.sin());
        let tangent = |theta: f64| x * (-theta.sin()) + y * theta.cos();

        let mut ctrl: Vec<Vec4> = Vec::with_capacity(2 * narcs + 1);
        let p0 = on_circle(0.0);
        ctrl.push(Vec4::new(p0.x, p0.y, p0.z, 1.0));
        for i in 0..narcs {
            let t0 = i as f64 * dtheta;
            let t2 = t0 + dtheta;
            let q0 = on_circle(t0);
            let q2 = on_circle(t2);
            // Shoulder point: intersection of the end tangents.
            // q1 = q0 + s * tan(t0), with s = chordal construction:
            // for a circular arc, q1 lies at distance r*tan(dtheta/2)
            // along the tangent from each endpoint.
            let s = radius * (dtheta / 2.0).tan();
            let q1 = q0 + tangent(t0) * s;
            debug_assert!((q1 - (q2 - tangent(t2) * s)).norm() < 1e-9 * radius.max(1.0));
            ctrl.push(Vec4::new(q1.x * w1, q1.y * w1, q1.z * w1, w1));
            ctrl.push(Vec4::new(q2.x, q2.y, q2.z, 1.0));
        }
        // Knot vector: clamped degree 2 with double interior knots at
        // segment boundaries, domain [0, 1].
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
mod tests {
    use super::*;
    use crate::basis::basis_funs;
    use proptest::prelude::*;

    pub(crate) fn arb_nurbs() -> impl Strategy<Value = NurbsCurve> {
        // degree 1..=4, 1..=4 extra control points, random interior
        // knots, weights in [0.2, 5], coordinates in [-100, 100].
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
                NurbsCurve::new(p, knots, points, Some(ws)).unwrap()
            })
        })
    }

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
            assert!(p.z - 3.0 == 0.0 || (p.z - 3.0).abs() < 1e-12);
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

    proptest! {
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
```

Note for the executor: `Vec4` needs `ZERO`, `is_finite`, `Add`, and `Mul<f64>`, all already provided by the keel-math macro. If `is_finite` is missing on Vec4, it is in the macro and available.

- [ ] **Step 2: Test and commit**

Run: `cargo test -p keel-geom`. Expected: pass, including circle exactness at 1e-12.

```bash
git add crates/keel-geom
git commit -m "feat(geom): homogeneous NURBS curves with de Boor evaluation and exact arcs"
```

---

### Task 5: Derivatives (hodograph + rational recursion)

**Files:**
- Modify: `crates/keel-geom/src/nurbs_curve.rs`

- [ ] **Step 1: Implement with tests**

Add to `impl NurbsCurve`:
```rust
    /// Control points and knot vector of the homogeneous derivative
    /// curve (degree p-1): Q_i = p * (P_{i+1} - P_i) / (u_{i+p+1} - u_{i+1}).
    fn derivative_curve(&self) -> Option<Self> {
        let p = self.kv.degree();
        if p == 1 {
            // Degree-0 result not representable as KnotVector (degree
            // floor is 1); handled by the caller via constant tangent.
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
        Self::from_homogeneous_unchecked(kv, ctrl)
    }

    /// Internal constructor that skips the positive-w check: derivative
    /// curves are vector-valued and may have any w component.
    fn from_homogeneous_unchecked(kv: KnotVector, ctrl: Vec<Vec4>) -> Option<Self> {
        if ctrl.len() != kv.control_count() {
            return None;
        }
        Some(Self { kv, ctrl })
    }

    /// Homogeneous derivatives A^(k) = d^k/du^k of the 4D curve for
    /// k = 0..=d, via successive derivative curves.
    fn ders_homogeneous(&self, u: f64, d: usize) -> Vec<Vec4> {
        let p = self.kv.degree();
        let d = d.min(p);
        let mut out = Vec::with_capacity(d + 1);
        out.push(self.eval_homogeneous(u));
        let mut cur = self.clone();
        for k in 1..=d {
            if cur.kv.degree() == 1 {
                // Derivative of a degree-1 curve is the constant
                // difference quotient on the active span.
                let span = cur.kv.find_span(u);
                let knots = cur.kv.knots();
                let denom = knots[span + 1] - knots[span];
                let q = (cur.ctrl[span] - cur.ctrl[span - 1]) * (1.0 / denom);
                out.push(q);
                // All higher derivatives are zero.
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
                None => {
                    out.push(Vec4::ZERO);
                }
            }
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
            let mut binom = 1.0_f64; // binom(k, i) built incrementally
            for i in 1..=k {
                binom = binom * (k - i + 1) as f64 / i as f64;
                v = v - c[k - i] * (binom * h[i].w);
            }
            c.push(v / w0);
        }
        c
    }
```

Add tests to the test module:
```rust
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
            // FD error scales with curve magnitude; generous bound.
            let scale = d[1].norm().max(1.0);
            prop_assert!(
                (d[1] - fd).norm() < 1e-3 * scale,
                "analytic {:?} fd {:?}", d[1], fd
            );
        }
    }
```

- [ ] **Step 2: Test and commit**

Run: `cargo test -p keel-geom`. Expected: pass. If `derivative_matches_finite_difference` fails near interior knots with reduced continuity, restrict the strategy's evaluation parameter away from knots or loosen to 1e-2 * scale; record whichever was needed.

```bash
git add crates/keel-geom
git commit -m "feat(geom): NURBS derivatives via hodograph and rational recursion"
```

---

### Task 6: Knot insertion, splitting, Bezier decomposition

**Files:**
- Modify: `crates/keel-geom/src/nurbs_curve.rs`

- [ ] **Step 1: Implement with tests**

Add to `impl NurbsCurve`:
```rust
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

    /// Split at u into two independent clamped curves covering
    /// [a, u] and [u, b].
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
        let left = Self::from_homogeneous(
            KnotVector::new(p, left_knots)?,
            cur.ctrl[..=i0].to_vec(),
        )?;
        let right = Self::from_homogeneous(
            KnotVector::new(p, right_knots)?,
            cur.ctrl[i0..].to_vec(),
        )?;
        Ok((left, right))
    }

    /// Decompose into rational Bezier segments: per nonempty span, the
    /// p+1 homogeneous Bezier control points plus the span's parameter
    /// interval [u0, u1] in the original domain.
    pub fn to_beziers(&self) -> Vec<BezierSegment> {
        let p = self.kv.degree();
        // Insert every distinct interior knot to multiplicity p.
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
                // Insertion failure is impossible here: u is interior
                // with multiplicity < p by construction.
                match cur.insert_knot(u) {
                    Ok(next) => cur = next,
                    Err(_) => break,
                }
            }
        }
        // Now spans of nonzero length each hold a Bezier segment.
        let knots = cur.kv.knots();
        let mut out = Vec::new();
        let mut start = p; // first domain knot index
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
```

Add after the `NurbsCurve` impl:
```rust
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
            Self { ctrl: left, u0: self.u0, u1: um },
            Self { ctrl: right, u0: um, u1: self.u1 },
        )
    }
    /// Projected 3D control points (hull bound, valid for w > 0).
    pub fn control_points(&self) -> Vec<Vec3> {
        self.ctrl
            .iter()
            .map(|c| Vec3::new(c.x / c.w, c.y / c.w, c.z / c.w))
            .collect()
    }
}
```

Tests:
```rust
    proptest! {
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
        fn split_halves_match(c in arb_nurbs(), us in 0.2..0.8f64, t in 0.0..1.0f64) {
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
            // Locate the segment containing t.
            let seg = segs.iter()
                .find(|s| t >= s.u0 && t <= s.u1)
                .expect("t inside some segment");
            let local = if seg.u1 > seg.u0 { (t - seg.u0) / (seg.u1 - seg.u0) } else { 0.0 };
            prop_assert!((seg.point(local) - c.point(t)).norm() < 1e-8);
        }
    }
```

- [ ] **Step 2: Test and commit**

Run: `cargo test -p keel-geom`. Expected: pass.

```bash
git add crates/keel-geom
git commit -m "feat(geom): knot insertion, splitting, and Bezier decomposition"
```

---

### Task 7: Analytic curves and the Curve3 enum

**Files:**
- Create: `crates/keel-geom/src/curve.rs`
- Modify: `lib.rs` (add `pub mod curve;`)

- [ ] **Step 1: Implement with tests**

`curve.rs`:
```rust
//! Analytic curve types and the exhaustive curve dispatch enum
//! (spec D4: analytics are first-class, never silently NURBS).

use crate::GeomError;
use crate::nurbs_curve::NurbsCurve;
use keel_math::bbox::Aabb3;
use keel_math::vec::Vec3;

/// Parameter domain of a curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Domain {
    /// Closed interval [a, b].
    Finite { a: f64, b: f64 },
    /// Periodic with the given period, canonical range [0, period).
    Periodic { period: f64 },
}

/// Infinite straight line: point(t) = origin + t * dir (dir unit, so
/// t is arc length). Carried unbounded; edges bound it in topology.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line3 {
    pub origin: Vec3,
    /// Unit direction.
    pub dir: Vec3,
}

impl Line3 {
    pub fn new(origin: Vec3, dir: Vec3) -> Result<Self, GeomError> {
        let dir = dir.try_normalize().ok_or(GeomError::Degenerate)?;
        Ok(Self { origin, dir })
    }
    #[inline]
    pub fn point(&self, t: f64) -> Vec3 {
        self.origin + self.dir * t
    }
    /// Exact closest parameter to p.
    #[inline]
    pub fn project(&self, p: Vec3) -> f64 {
        (p - self.origin).dot(self.dir)
    }
}

/// Circle: point(theta) = center + r cos(theta) x + r sin(theta) y,
/// with x, y unit and orthogonal. theta in [0, 2*pi).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle3 {
    pub center: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub radius: f64,
}

impl Circle3 {
    pub fn new(center: Vec3, x_axis: Vec3, y_axis: Vec3, radius: f64) -> Result<Self, GeomError> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(GeomError::Degenerate);
        }
        let x = x_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        let y = y_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        if x.dot(y).abs() > 1e-12 {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { center, x_axis: x, y_axis: y, radius })
    }
    #[inline]
    pub fn point(&self, theta: f64) -> Vec3 {
        self.center + self.x_axis * (self.radius * theta.cos())
            + self.y_axis * (self.radius * theta.sin())
    }
    /// Closest parameter to p. Degenerate case (p on the circle axis):
    /// every point is equidistant; returns theta = 0 by convention.
    pub fn project(&self, p: Vec3) -> f64 {
        let d = p - self.center;
        let (cx, cy) = (d.dot(self.x_axis), d.dot(self.y_axis));
        if cx == 0.0 && cy == 0.0 {
            return 0.0;
        }
        let theta = cy.atan2(cx);
        if theta < 0.0 { theta + core::f64::consts::TAU } else { theta }
    }
}

/// Ellipse: point(theta) = center + a cos(theta) x + b sin(theta) y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse3 {
    pub center: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub a: f64,
    pub b: f64,
}

impl Ellipse3 {
    pub fn new(
        center: Vec3,
        x_axis: Vec3,
        y_axis: Vec3,
        a: f64,
        b: f64,
    ) -> Result<Self, GeomError> {
        if !(a.is_finite() && b.is_finite() && a > 0.0 && b > 0.0) {
            return Err(GeomError::Degenerate);
        }
        let x = x_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        let y = y_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        if x.dot(y).abs() > 1e-12 {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { center, x_axis: x, y_axis: y, a, b })
    }
    #[inline]
    pub fn point(&self, theta: f64) -> Vec3 {
        self.center + self.x_axis * (self.a * theta.cos())
            + self.y_axis * (self.b * theta.sin())
    }
    #[inline]
    fn deriv(&self, theta: f64) -> Vec3 {
        self.x_axis * (-self.a * theta.sin()) + self.y_axis * (self.b * theta.cos())
    }
    /// Closest parameter to p: stationary points of the squared
    /// distance, found by bracketing sign changes of
    /// g(theta) = (E - p) . E' over a uniform scan, polished with the
    /// bracketed Newton engine, best candidate kept.
    pub fn project(&self, p: Vec3) -> f64 {
        use keel_math::newton::solve_bracketed;
        const SCAN: usize = 32;
        let tau = core::f64::consts::TAU;
        let g = |th: f64| {
            let e = self.point(th) - p;
            let d1 = self.deriv(th);
            let d2 = self.x_axis * (-self.a * th.cos()) + self.y_axis * (-self.b * th.sin());
            (e.dot(d1), d1.dot(d1) + e.dot(d2))
        };
        let mut best = (0.0_f64, (self.point(0.0) - p).norm_sq());
        let mut prev_th = 0.0;
        let mut prev_g = g(0.0).0;
        for i in 1..=SCAN {
            let th = i as f64 * tau / SCAN as f64;
            let gv = g(th).0;
            if prev_g == 0.0 || (prev_g > 0.0) != (gv > 0.0) {
                if let Some(root) = solve_bracketed(g, prev_th, th, 1e-14, 64) {
                    let d = (self.point(root) - p).norm_sq();
                    if d < best.1 {
                        best = (root.rem_euclid(tau), d);
                    }
                }
            }
            prev_th = th;
            prev_g = gv;
        }
        best.0
    }
}

/// Exhaustive curve dispatch (compile error at every non-exhaustive
/// match when a curve type is added).
#[derive(Clone, Debug)]
pub enum Curve3 {
    Line(Line3),
    Circle(Circle3),
    Ellipse(Ellipse3),
    Nurbs(NurbsCurve),
}

impl Curve3 {
    pub fn point(&self, t: f64) -> Vec3 {
        match self {
            Curve3::Line(c) => c.point(t),
            Curve3::Circle(c) => c.point(t),
            Curve3::Ellipse(c) => c.point(t),
            Curve3::Nurbs(c) => c.point(t),
        }
    }

    /// Derivatives 0..=d at t (d capped per type; analytic types
    /// support any d).
    pub fn derivatives(&self, t: f64, d: usize) -> Vec<Vec3> {
        match self {
            Curve3::Line(c) => {
                let mut out = vec![c.point(t), c.dir];
                out.extend(std::iter::repeat(Vec3::ZERO).take(d.saturating_sub(1)));
                out.truncate(d + 1);
                out
            }
            Curve3::Circle(c) => {
                // Successive derivatives rotate by 90 degrees.
                let r = c.radius;
                (0..=d)
                    .map(|k| {
                        let th = t + k as f64 * core::f64::consts::FRAC_PI_2;
                        if k == 0 {
                            c.point(t)
                        } else {
                            c.x_axis * (r * th.cos()) + c.y_axis * (r * th.sin())
                        }
                    })
                    .collect()
            }
            Curve3::Ellipse(c) => (0..=d)
                .map(|k| match k % 4 {
                    0 if k == 0 => c.point(t),
                    0 => c.x_axis * (c.a * t.cos()) + c.y_axis * (c.b * t.sin()),
                    1 => c.x_axis * (-c.a * t.sin()) + c.y_axis * (c.b * t.cos()),
                    2 => c.x_axis * (-c.a * t.cos()) + c.y_axis * (-c.b * t.sin()),
                    _ => c.x_axis * (c.a * t.sin()) + c.y_axis * (-c.b * t.cos()),
                })
                .collect(),
            Curve3::Nurbs(c) => c.derivatives(t, d),
        }
    }

    pub fn domain(&self) -> Domain {
        match self {
            Curve3::Line(_) => Domain::Finite {
                a: f64::NEG_INFINITY,
                b: f64::INFINITY,
            },
            Curve3::Circle(_) | Curve3::Ellipse(_) => Domain::Periodic {
                period: core::f64::consts::TAU,
            },
            Curve3::Nurbs(c) => {
                let (a, b) = c.domain();
                Domain::Finite { a, b }
            }
        }
    }

    /// Bounding box. Lines are unbounded: callers bound them via edges;
    /// here the box of the origin is returned as a placeholder and
    /// topology-level code must never call bbox on an unbounded line.
    pub fn bbox(&self) -> Aabb3 {
        match self {
            Curve3::Line(c) => Aabb3::from_points([c.origin]),
            Curve3::Circle(c) => {
                // Per-axis extent of r*(cos*x + sin*y) is
                // r * sqrt(x_i^2 + y_i^2).
                let r = c.radius;
                let e = Vec3::new(
                    (c.x_axis.x.powi(2) + c.y_axis.x.powi(2)).sqrt(),
                    (c.x_axis.y.powi(2) + c.y_axis.y.powi(2)).sqrt(),
                    (c.x_axis.z.powi(2) + c.y_axis.z.powi(2)).sqrt(),
                ) * r;
                Aabb3 { min: c.center - e, max: c.center + e }
            }
            Curve3::Ellipse(c) => {
                let e = Vec3::new(
                    ((c.a * c.x_axis.x).powi(2) + (c.b * c.y_axis.x).powi(2)).sqrt(),
                    ((c.a * c.x_axis.y).powi(2) + (c.b * c.y_axis.y).powi(2)).sqrt(),
                    ((c.a * c.x_axis.z).powi(2) + (c.b * c.y_axis.z).powi(2)).sqrt(),
                );
                Aabb3 { min: c.center - e, max: c.center + e }
            }
            Curve3::Nurbs(c) => Aabb3::from_points(c.control_points()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_project_roundtrip() {
        let l = Line3::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 2.0, 0.0)).unwrap();
        let p = l.point(3.5) + Vec3::new(5.0, 0.0, 0.0); // offset normal to dir
        assert!((l.project(p) - 3.5).abs() < 1e-14);
    }

    #[test]
    fn circle_project_roundtrip() {
        let c = Circle3::new(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            2.0,
        )
        .unwrap();
        let theta = 2.3;
        // Offset radially and along the axis: projection unchanged.
        let p = c.point(theta) + (c.point(theta) - c.center) * 0.4 + Vec3::new(0.0, 0.0, 9.0);
        assert!((c.project(p) - theta).abs() < 1e-12);
    }

    #[test]
    fn ellipse_project_known_cases() {
        let e = Ellipse3::new(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            3.0,
            1.0,
        )
        .unwrap();
        // On-axis exterior point projects to theta = 0.
        assert!(e.project(Vec3::new(10.0, 0.0, 0.0)).abs() < 1e-9);
        // Roundtrip from a normal offset at a generic angle.
        let th = 0.9_f64;
        let d1 = e.deriv(th);
        let n = Vec3::new(0.0, 0.0, 1.0).cross(d1).try_normalize().unwrap();
        let p = e.point(th) + n * 0.2;
        let got = e.project(p);
        assert!(
            (e.point(got) - p).norm() <= (e.point(th) - p).norm() + 1e-12,
            "projection not closer: got {got} want {th}"
        );
    }

    #[test]
    fn circle_bbox_contains_samples() {
        let c = Circle3::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 1.0).cross(Vec3::new(1.0, 1.0, 0.0)),
            1.7,
        );
        // Use orthogonalized axes; build via cross products.
        let x = Vec3::new(1.0, 1.0, 0.0).try_normalize().unwrap();
        let helper = Vec3::new(0.0, 0.0, 1.0);
        let y = helper.cross(x).try_normalize().unwrap();
        let c = Circle3::new(Vec3::new(1.0, 2.0, 3.0), x, y, 1.7).unwrap();
        let bb = Curve3::Circle(c).bbox().expanded(1e-12);
        for i in 0..64 {
            let th = i as f64 * core::f64::consts::TAU / 64.0;
            assert!(bb.contains(c.point(th)), "theta={th}");
        }
        let _ = c;
    }
}
```

Note: the first `circle_bbox_contains_samples` construction is shadowed deliberately by the orthogonalized rebuild; the executor should clean that to a single valid construction (build x, then y = helper x x normalized) rather than keep the dead first binding.

- [ ] **Step 2: Test and commit**

Run: `cargo test -p keel-geom`. Expected: pass.

```bash
git add crates/keel-geom
git commit -m "feat(geom): analytic curves and exhaustive Curve3 dispatch"
```

---

### Task 8: NURBS closest-point projection

**Files:**
- Create: `crates/keel-geom/src/project.rs`
- Modify: `lib.rs` (add `pub mod project;`)

- [ ] **Step 1: Implement with tests**

`project.rs`:
```rust
//! Certified-global closest-point projection onto NURBS curves:
//! Bezier decomposition, convex-hull branch-and-bound pruning, and
//! bracketed-Newton polish (Ma-Hewitt / Selimovic pattern).

use crate::nurbs_curve::{BezierSegment, NurbsCurve};
use keel_math::bbox::Aabb3;
use keel_math::vec::Vec3;

/// Result of a closest-point query.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    /// Parameter of the closest point in the curve's domain.
    pub u: f64,
    /// The closest point.
    pub point: Vec3,
    /// Euclidean distance.
    pub distance: f64,
}

/// Distance from p to an AABB (zero inside): the pruning lower bound,
/// valid because the curve lies in the hull of its control points.
fn dist_to_aabb(p: Vec3, bb: &Aabb3) -> f64 {
    let dx = (bb.min.x - p.x).max(0.0).max(p.x - bb.max.x);
    let dy = (bb.min.y - p.y).max(0.0).max(p.y - bb.max.y);
    let dz = (bb.min.z - p.z).max(0.0).max(p.z - bb.max.z);
    Vec3::new(dx, dy, dz).norm()
}

/// Globally closest point on `curve` to `p`.
///
/// Algorithm: decompose to Bezier segments; branch-and-bound with the
/// control-AABB lower bound against the best endpoint-sample upper
/// bound, subdividing surviving segments until their parameter width
/// is below `width_tol`; polish each survivor with bracketed Newton on
/// g(u) = (C(u) - p) . C'(u) and keep the global best.
pub fn project_point(curve: &NurbsCurve, p: Vec3, width_tol: f64) -> Projection {
    let (dom_a, dom_b) = curve.domain();
    // Seed best with the domain endpoints.
    let mut best = Projection {
        u: dom_a,
        point: curve.point(dom_a),
        distance: (curve.point(dom_a) - p).norm(),
    };
    let end = Projection {
        u: dom_b,
        point: curve.point(dom_b),
        distance: (curve.point(dom_b) - p).norm(),
    };
    if end.distance < best.distance {
        best = end;
    }

    let mut stack: Vec<BezierSegment> = curve.to_beziers();
    let mut survivors: Vec<BezierSegment> = Vec::new();
    while let Some(seg) = stack.pop() {
        let bb = Aabb3::from_points(seg.control_points());
        if dist_to_aabb(p, &bb) > best.distance + 1e-12 {
            continue; // provably no closer point in this segment
        }
        // Tighten the upper bound from the segment midpoint.
        let mid = seg.point(0.5);
        let dm = (mid - p).norm();
        if dm < best.distance {
            best = Projection {
                u: 0.5 * (seg.u0 + seg.u1),
                point: mid,
                distance: dm,
            };
        }
        if seg.u1 - seg.u0 <= width_tol {
            survivors.push(seg);
        } else {
            let (l, r) = seg.subdivide(0.5);
            stack.push(l);
            stack.push(r);
        }
    }

    // Newton polish on each survivor in the parent parameter.
    for seg in survivors {
        let g = |u: f64| {
            let d = curve.derivatives(u, 2);
            let e = d[0] - p;
            (e.dot(d[1]), d[1].dot(d[1]) + e.dot(d[2]))
        };
        let (g0, _) = g(seg.u0);
        let (g1, _) = g(seg.u1);
        let candidate = if g0 == 0.0 {
            Some(seg.u0)
        } else if g1 == 0.0 {
            Some(seg.u1)
        } else if (g0 > 0.0) != (g1 > 0.0) {
            keel_math::newton::solve_bracketed(g, seg.u0, seg.u1, 1e-14, 64)
        } else {
            None
        };
        if let Some(u) = candidate {
            let q = curve.point(u);
            let d = (q - p).norm();
            if d < best.distance {
                best = Projection { u, point: q, distance: d };
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nurbs_curve::NurbsCurve;
    use proptest::prelude::*;

    fn circle() -> NurbsCurve {
        NurbsCurve::full_circle(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            2.0,
        )
        .unwrap()
    }

    #[test]
    fn projects_onto_circle_radially() {
        let c = circle();
        // A point at radius 5 in the plane projects to radius 2 along
        // the same ray.
        let p = Vec3::new(3.0, 4.0, 0.0); // norm 5, direction (0.6, 0.8)
        let pr = project_point(&c, p, 1e-4);
        let expected = Vec3::new(1.2, 1.6, 0.0);
        assert!((pr.point - expected).norm() < 1e-8, "{:?}", pr);
        assert!((pr.distance - 3.0).abs() < 1e-8);
    }

    #[test]
    fn on_curve_point_projects_to_itself() {
        let c = circle();
        let q = c.point(0.37);
        let pr = project_point(&c, q, 1e-4);
        assert!(pr.distance < 1e-9);
        assert!((pr.point - q).norm() < 1e-9);
    }

    proptest! {
        // Sample the curve densely; the projection distance must not
        // exceed the distance to any sample (global optimality check).
        #[test]
        fn projection_beats_dense_sampling(
            c in crate::nurbs_curve::tests::arb_nurbs(),
            px in -150.0..150.0f64, py in -150.0..150.0f64, pz in -150.0..150.0f64,
        ) {
            let p = Vec3::new(px, py, pz);
            let pr = project_point(&c, p, 1e-5);
            for i in 0..=200 {
                let t = i as f64 / 200.0;
                let d = (c.point(t) - p).norm();
                prop_assert!(
                    pr.distance <= d + 1e-6,
                    "projection {} beaten at t={t} with {}", pr.distance, d
                );
            }
        }
    }
}
```

Note for the executor: `arb_nurbs` must be visible: change its definition in `nurbs_curve.rs` tests from a private fn to `#[cfg(test)] pub(crate)` at module scope (move it out of the `mod tests` block into a `#[cfg(test)] pub(crate) mod test_support` module if needed, and fix both use sites).

- [ ] **Step 2: Test and commit**

Run: `cargo test -p keel-geom`. Expected: pass; the dense-sampling oracle is the key check.

```bash
git add crates/keel-geom
git commit -m "feat(geom): global NURBS closest-point via branch-and-bound plus Newton"
```

---

### Task 9: Benches, fuzz target, validation, log

**Files:**
- Modify: `crates/keel-geom/benches/geom.rs`, `fuzz/Cargo.toml`, `LOG.md`
- Create: `fuzz/fuzz_targets/fuzz_nurbs_curve.rs`

- [ ] **Step 1: Benchmarks**

`benches/geom.rs`:
```rust
#![allow(clippy::unwrap_used)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::project::project_point;
use keel_math::vec::Vec3;

fn circle() -> NurbsCurve {
    NurbsCurve::full_circle(
        Vec3::ZERO,
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        2.0,
    )
    .unwrap()
}

fn bench_eval(c: &mut Criterion) {
    let curve = circle();
    c.bench_function("nurbs_circle_point", |b| {
        b.iter(|| black_box(&curve).point(black_box(0.37)))
    });
    c.bench_function("nurbs_circle_derivs2", |b| {
        b.iter(|| black_box(&curve).derivatives(black_box(0.37), 2))
    });
}

fn bench_project(c: &mut Criterion) {
    let curve = circle();
    let p = Vec3::new(3.0, 4.0, 1.0);
    c.bench_function("nurbs_circle_project", |b| {
        b.iter(|| project_point(black_box(&curve), black_box(p), 1e-5))
    });
}

criterion_group!(benches, bench_eval, bench_project);
criterion_main!(benches);
```

Run: `cargo bench -p keel-geom --bench geom -- --quick` and record numbers in the LOG entry.

- [ ] **Step 2: Fuzz target**

Append to `fuzz/Cargo.toml`:
```toml
[[bin]]
name = "fuzz_nurbs_curve"
path = "fuzz_targets/fuzz_nurbs_curve.rs"
test = false
doc = false
```
and add `keel-geom = { path = "../crates/keel-geom" }` to its `[dependencies]`.

`fuzz/fuzz_targets/fuzz_nurbs_curve.rs`:
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

// Arbitrary degree/knots/points/weights: the constructor must either
// reject cleanly or yield a curve whose evaluation never panics and
// stays inside the control AABB.
fuzz_target!(|data: (u8, Vec<f64>, Vec<(f64, f64, f64)>, Vec<f64>, f64)| {
    let (deg, knots, pts, ws, t) = data;
    let degree = 1 + (deg as usize % 11);
    if knots.len() > 64 || pts.len() > 32 {
        return;
    }
    if knots.iter().any(|k| !k.is_finite()) {
        // Constructor must reject; just exercise it.
        let _ = keel_geom::nurbs_curve::NurbsCurve::new(
            degree,
            knots,
            pts.iter().map(|&(x, y, z)| keel_math::vec::Vec3::new(x, y, z)).collect(),
            Some(ws),
        );
        return;
    }
    let points: Vec<_> = pts
        .iter()
        .map(|&(x, y, z)| keel_math::vec::Vec3::new(x, y, z))
        .collect();
    if points.iter().any(|p| !p.is_finite()) {
        return;
    }
    if let Ok(c) = keel_geom::nurbs_curve::NurbsCurve::new(degree, knots, points, Some(ws)) {
        let (a, b) = c.domain();
        if !t.is_finite() {
            return;
        }
        let u = a + t.rem_euclid(1.0).clamp(0.0, 1.0) * (b - a);
        let p = c.point(u);
        assert!(p.is_finite());
        let _ = c.derivatives(u, 2);
        if let Ok(c2) = c.insert_knot(0.5 * (a + b)) {
            assert!((c2.point(u) - p).norm() <= 1e-6 * (1.0 + p.norm()));
        }
    }
});
```

Run: `cargo check --manifest-path fuzz/Cargo.toml`. Expected: compiles.

- [ ] **Step 3: Full validation, fuzz results, log, merge**

Run: `cargo fmt --all; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace`
Expected: all clean and green.

Check the WSL fuzz-run results (background task from M1 gate); record outcomes. Optionally run the new curve fuzz target 10 minutes in WSL.

Append a LOG.md entry: M2a complete, modules shipped, bench baselines, fuzz results, deviations from plan, next step (M2b).

```bash
git add -A
git commit -m "feat(geom): benches, curve fuzz target, M2a wrap-up"
```

Merge the working branch to master per the finishing-a-development-branch flow.

---

## Completion criteria (M2a exit gate)

- `cargo test --workspace` green; clippy clean with `-D warnings`.
- Circle exactness at 1e-12 over 200 samples (the rational-conic contract).
- Knot insertion / split / Bezier decomposition geometry-preservation proptests green.
- Projection beats a 200-sample dense scan on random curves (global-optimality oracle).
- Bench baselines recorded for eval, derivatives, projection.
- M1 fuzz-run results (WSL) recorded in LOG.md; new curve fuzz target compiles.
- Deferred and recorded: A2.3 basis derivatives (with fitting), Oslo batch refinement (with lofting), knot removal (with simplification pipeline), periodic knot vectors (with closed-surface work), multivariate solver + surfaces (M2b).
