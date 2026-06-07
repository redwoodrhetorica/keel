# M1 Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the Keel workspace and build `keel-math`: the numeric foundation (vectors, matrices, transforms, bounding boxes, interval arithmetic, tolerance model, exact predicates, bracketed Newton, quadratic/cubic solvers, univariate Bernstein machinery) with property tests, benchmarks, fuzz scaffolding, and CI.

**Architecture:** Single Cargo workspace; `crates/keel-math` is the only member for M1. Exact predicates wrap the `robust` crate (Shewchuk adaptive, filter included) behind a sign-enum API so no caller ever branches on a raw f64. Interval arithmetic uses outward rounding via `next_up`/`next_down` (stable Rust). All solvers funnel through one bracketed Newton-bisection hybrid. The multivariate Bernstein subdivision solver (spec D6) is deliberately deferred to early M2 where its first consumers can test it.

**Tech Stack:** Rust stable (edition 2024), `robust` (predicates), dev-deps `proptest` + `criterion`, `cargo-fuzz` scaffolding (Linux/nightly execution), GitHub Actions CI.

**Conventions for every task:** run commands from the repo root `C:\Users\mcdon\Documents\Repo\Claude\parasolid`. No `unwrap`/`expect`/`panic!` in library code (tests may). No em-dashes in any file. Every public item gets a doc comment; keep them short.

---

### Task 1: Workspace scaffold

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `crates/keel-math/Cargo.toml`, `crates/keel-math/src/lib.rs`

- [ ] **Step 1: Verify toolchain**

Run: `cargo --version && rustc --version`
Expected: rustc 1.85+ (edition 2024 support). If cargo is missing, stop and ask the user to install via https://rustup.rs (suggest they run `! winget install Rustlang.Rustup` then restart the session).

- [ ] **Step 2: Write workspace files**

`Cargo.toml` (repo root):
```toml
[workspace]
resolver = "2"
members = ["crates/keel-math"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/keel-kernel/keel"

[workspace.lints.clippy]
unwrap_used = "warn"
expect_used = "warn"
dbg_macro = "warn"

[workspace.dependencies]
robust = "1"
proptest = "1"
criterion = "0.5"
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
```

`crates/keel-math/Cargo.toml`:
```toml
[package]
name = "keel-math"
description = "Numeric foundations for the Keel geometry kernel"
version.workspace = true
edition.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
robust.workspace = true

[dev-dependencies]
proptest.workspace = true
criterion.workspace = true

[[bench]]
name = "math"
harness = false
path = "benches/math.rs"
```

`crates/keel-math/src/lib.rs`:
```rust
//! Numeric foundations for the Keel geometry kernel.
//!
//! Policy: tolerant modeling on f64 with exact predicates at decision
//! points. No combinatorial branch may read a raw f64 sign; use the
//! `predicates` module. Tolerances come from `tolerance`, never inline.
```

Also create an empty placeholder bench so the manifest is valid, `crates/keel-math/benches/math.rs`:
```rust
fn main() {}
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build`
Expected: compiles with no warnings.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates
git commit -m "feat: scaffold Keel workspace with keel-math crate"
```

---

### Task 2: Vectors (Vec2, Vec3, Vec4)

**Files:**
- Create: `crates/keel-math/src/vec.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod vec;`)
- Test: inline `#[cfg(test)]` module in `vec.rs`

- [ ] **Step 1: Write the failing tests**

Append to a new `crates/keel-math/src/vec.rs` (tests first; the types do not exist yet):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn cross_is_orthogonal() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(-4.0, 0.5, 2.0);
        let c = a.cross(b);
        assert!(c.dot(a).abs() < 1e-12);
        assert!(c.dot(b).abs() < 1e-12);
    }

    #[test]
    fn normalize_zero_fails() {
        assert!(Vec3::ZERO.try_normalize().is_none());
    }

    #[test]
    fn vec2_perp_dot() {
        let a = Vec2::new(3.0, 4.0);
        assert_eq!(a.perp_dot(a), 0.0);
        assert_eq!(Vec2::new(1.0, 0.0).perp_dot(Vec2::new(0.0, 1.0)), 1.0);
    }

    fn finite_vec3() -> impl Strategy<Value = Vec3> {
        let f = -1.0e6..1.0e6f64;
        (f.clone(), f.clone(), f).prop_map(|(x, y, z)| Vec3::new(x, y, z))
    }

    proptest! {
        #[test]
        fn dot_commutes(a in finite_vec3(), b in finite_vec3()) {
            prop_assert_eq!(a.dot(b), b.dot(a));
        }
        #[test]
        fn cross_antisymmetric(a in finite_vec3(), b in finite_vec3()) {
            let lhs = a.cross(b);
            let rhs = b.cross(a) * -1.0;
            prop_assert_eq!(lhs, rhs);
        }
        #[test]
        fn normalized_has_unit_norm(a in finite_vec3()) {
            if let Some(n) = a.try_normalize() {
                prop_assert!((n.norm() - 1.0).abs() < 1e-12);
            }
        }
    }
}
```

Add `pub mod vec;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Vec3` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `vec.rs` (above the test module):
```rust
//! Fixed-size vector types. Plain f64 value types; no SIMD yet.

use core::ops::{Add, Div, Mul, Neg, Sub};

/// Generates component-wise operators and shared inherent methods.
macro_rules! impl_vec {
    ($V:ident { $($f:ident),+ }) => {
        impl $V {
            pub const ZERO: Self = Self { $($f: 0.0),+ };
            #[inline] pub const fn new($($f: f64),+) -> Self { Self { $($f),+ } }
            /// Dot product.
            #[inline] pub fn dot(self, o: Self) -> f64 { 0.0 $(+ self.$f * o.$f)+ }
            /// Squared Euclidean norm.
            #[inline] pub fn norm_sq(self) -> f64 { self.dot(self) }
            /// Euclidean norm.
            #[inline] pub fn norm(self) -> f64 { self.norm_sq().sqrt() }
            /// Unit vector, or None when the norm is zero, non-finite,
            /// or so small that 1/norm overflows.
            #[inline] pub fn try_normalize(self) -> Option<Self> {
                let inv = 1.0 / self.norm();
                if inv.is_finite() { Some(self * inv) } else { None }
            }
            /// True when every component is finite.
            #[inline] pub fn is_finite(self) -> bool { true $(&& self.$f.is_finite())+ }
        }
        impl Add for $V { type Output = Self;
            #[inline] fn add(self, o: Self) -> Self { Self { $($f: self.$f + o.$f),+ } } }
        impl Sub for $V { type Output = Self;
            #[inline] fn sub(self, o: Self) -> Self { Self { $($f: self.$f - o.$f),+ } } }
        impl Mul<f64> for $V { type Output = Self;
            #[inline] fn mul(self, s: f64) -> Self { Self { $($f: self.$f * s),+ } } }
        impl Div<f64> for $V { type Output = Self;
            #[inline] fn div(self, s: f64) -> Self { Self { $($f: self.$f / s),+ } } }
        impl Neg for $V { type Output = Self;
            #[inline] fn neg(self) -> Self { Self { $($f: -self.$f),+ } } }
    };
}

/// 2D vector.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec2 { pub x: f64, pub y: f64 }

/// 3D vector.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 { pub x: f64, pub y: f64, pub z: f64 }

/// 4D vector (homogeneous coordinates for rational geometry).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec4 { pub x: f64, pub y: f64, pub z: f64, pub w: f64 }

impl_vec!(Vec2 { x, y });
impl_vec!(Vec3 { x, y, z });
impl_vec!(Vec4 { x, y, z, w });

impl Vec2 {
    /// 2D cross product magnitude: self.x*o.y - self.y*o.x.
    #[inline] pub fn perp_dot(self, o: Self) -> f64 { self.x * o.y - self.y * o.x }
}

impl Vec3 {
    /// Cross product.
    #[inline]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
}

/// Points are vectors with positional meaning; alias for readability.
pub type Point2 = Vec2;
/// See [`Point2`].
pub type Point3 = Vec3;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): Vec2/Vec3/Vec4 with property tests"
```

---

### Task 3: Matrices (Mat2, Mat3)

**Files:**
- Create: `crates/keel-math/src/mat.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod mat;`)

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/mat.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::Vec3;
    use proptest::prelude::*;

    #[test]
    fn identity_inverse() {
        let inv = Mat3::IDENTITY.try_inverse().unwrap();
        assert_eq!(inv, Mat3::IDENTITY);
    }

    #[test]
    fn singular_has_no_inverse() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let m = Mat3::from_cols(a, a * 2.0, a * -1.0);
        assert!(m.try_inverse().is_none());
    }

    #[test]
    fn determinant_of_known_matrix() {
        let m = Mat3::from_cols(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::new(0.0, 0.0, 4.0),
        );
        assert_eq!(m.determinant(), 24.0);
    }

    fn well_conditioned() -> impl Strategy<Value = Mat3> {
        let f = -100.0..100.0f64;
        let v = move || (f.clone(), f.clone(), f.clone())
            .prop_map(|(x, y, z)| Vec3::new(x, y, z));
        (v(), v(), v())
            .prop_map(|(a, b, c)| Mat3::from_cols(a, b, c))
            .prop_filter("well conditioned", |m| m.determinant().abs() > 1.0)
    }

    proptest! {
        #[test]
        fn inverse_roundtrips(m in well_conditioned()) {
            let inv = m.try_inverse().unwrap();
            let p = m.mul(inv);
            for (a, b) in p.cols.iter().zip(Mat3::IDENTITY.cols.iter()) {
                prop_assert!((*a - *b).norm() < 1e-6);
            }
        }
    }
}
```

Add `pub mod mat;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Mat3` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `mat.rs`:
```rust
//! Small dense matrices, column-major.

use crate::vec::{Vec2, Vec3};

/// 2x2 matrix, column-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat2 { pub cols: [Vec2; 2] }

impl Mat2 {
    pub const IDENTITY: Self =
        Self { cols: [Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)] };
    #[inline] pub const fn from_cols(a: Vec2, b: Vec2) -> Self { Self { cols: [a, b] } }
    #[inline] pub fn determinant(self) -> f64 { self.cols[0].perp_dot(self.cols[1]) }
    #[inline] pub fn mul_vec(self, v: Vec2) -> Vec2 {
        self.cols[0] * v.x + self.cols[1] * v.y
    }
    /// Inverse, or None when the determinant's reciprocal is not finite.
    #[inline]
    pub fn try_inverse(self) -> Option<Self> {
        let inv_det = 1.0 / self.determinant();
        if !inv_det.is_finite() { return None; }
        let [a, b] = self.cols;
        Some(Self::from_cols(
            Vec2::new(b.y, -a.y) * inv_det,
            Vec2::new(-b.x, a.x) * inv_det,
        ))
    }
}

/// 3x3 matrix, column-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 { pub cols: [Vec3; 3] }

impl Mat3 {
    pub const IDENTITY: Self = Self { cols: [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ]};
    #[inline] pub const fn from_cols(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { cols: [a, b, c] }
    }
    /// Scalar triple product of the columns.
    #[inline] pub fn determinant(self) -> f64 {
        self.cols[0].dot(self.cols[1].cross(self.cols[2]))
    }
    #[inline] pub fn transpose(self) -> Self {
        let [a, b, c] = self.cols;
        Self::from_cols(
            Vec3::new(a.x, b.x, c.x),
            Vec3::new(a.y, b.y, c.y),
            Vec3::new(a.z, b.z, c.z),
        )
    }
    #[inline] pub fn mul_vec(self, v: Vec3) -> Vec3 {
        self.cols[0] * v.x + self.cols[1] * v.y + self.cols[2] * v.z
    }
    #[inline] pub fn mul(self, o: Self) -> Self {
        Self { cols: [
            self.mul_vec(o.cols[0]),
            self.mul_vec(o.cols[1]),
            self.mul_vec(o.cols[2]),
        ]}
    }
    /// Inverse via the adjugate, or None when 1/det is not finite.
    pub fn try_inverse(self) -> Option<Self> {
        let [a, b, c] = self.cols;
        let r0 = b.cross(c);
        let r1 = c.cross(a);
        let r2 = a.cross(b);
        let inv_det = 1.0 / a.dot(r0);
        if !inv_det.is_finite() { return None; }
        Some(Self::from_cols(
            Vec3::new(r0.x, r1.x, r2.x) * inv_det,
            Vec3::new(r0.y, r1.y, r2.y) * inv_det,
            Vec3::new(r0.z, r1.z, r2.z) * inv_det,
        ))
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): Mat2/Mat3 with inverse roundtrip property test"
```

---

### Task 4: Rigid transforms (Transform3)

**Files:**
- Create: `crates/keel-math/src/transform.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod transform;`)

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/transform.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::Vec3;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn rotation_quarter_turn_about_z() {
        let t = Transform3::from_rotation(Vec3::new(0.0, 0.0, 1.0), FRAC_PI_2).unwrap();
        let p = t.apply_point(Vec3::new(1.0, 0.0, 0.0));
        assert!((p - Vec3::new(0.0, 1.0, 0.0)).norm() < 1e-15);
    }

    #[test]
    fn rotation_about_zero_axis_fails() {
        assert!(Transform3::from_rotation(Vec3::ZERO, 1.0).is_none());
    }

    #[test]
    fn compose_then_inverse_roundtrips() {
        let t = Transform3::from_rotation(Vec3::new(1.0, 2.0, 0.5), 0.7).unwrap()
            .then(Transform3::from_translation(Vec3::new(3.0, -1.0, 2.0)));
        let inv = t.try_inverse().unwrap();
        let p = Vec3::new(0.3, -2.0, 5.0);
        assert!((inv.apply_point(t.apply_point(p)) - p).norm() < 1e-12);
    }

    #[test]
    fn vectors_ignore_translation() {
        let t = Transform3::from_translation(Vec3::new(10.0, 10.0, 10.0));
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(t.apply_vector(v), v);
    }
}
```

Add `pub mod transform;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Transform3` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `transform.rs`:
```rust
//! Affine transforms (rotation, translation, uniform scale).

use crate::mat::Mat3;
use crate::vec::Vec3;

/// Affine transform: p -> linear * p + translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform3 {
    pub linear: Mat3,
    pub translation: Vec3,
}

impl Transform3 {
    pub const IDENTITY: Self =
        Self { linear: Mat3::IDENTITY, translation: Vec3::ZERO };

    #[inline]
    pub const fn from_translation(t: Vec3) -> Self {
        Self { linear: Mat3::IDENTITY, translation: t }
    }

    #[inline]
    pub fn from_uniform_scale(s: f64) -> Self {
        Self {
            linear: Mat3::from_cols(
                Vec3::new(s, 0.0, 0.0),
                Vec3::new(0.0, s, 0.0),
                Vec3::new(0.0, 0.0, s),
            ),
            translation: Vec3::ZERO,
        }
    }

    /// Rotation by `angle` radians about `axis` (Rodrigues' formula).
    /// None when the axis cannot be normalized.
    pub fn from_rotation(axis: Vec3, angle: f64) -> Option<Self> {
        let k = axis.try_normalize()?;
        let (s, c) = angle.sin_cos();
        let v = 1.0 - c;
        let linear = Mat3::from_cols(
            Vec3::new(k.x * k.x * v + c, k.y * k.x * v + k.z * s, k.z * k.x * v - k.y * s),
            Vec3::new(k.x * k.y * v - k.z * s, k.y * k.y * v + c, k.z * k.y * v + k.x * s),
            Vec3::new(k.x * k.z * v + k.y * s, k.y * k.z * v - k.x * s, k.z * k.z * v + c),
        );
        Some(Self { linear, translation: Vec3::ZERO })
    }

    /// Transform a position (translation applies).
    #[inline]
    pub fn apply_point(self, p: Vec3) -> Vec3 {
        self.linear.mul_vec(p) + self.translation
    }

    /// Transform a direction (translation ignored).
    #[inline]
    pub fn apply_vector(self, v: Vec3) -> Vec3 {
        self.linear.mul_vec(v)
    }

    /// `self` first, then `next`: (self.then(next))(p) == next(self(p)).
    #[inline]
    pub fn then(self, next: Self) -> Self {
        Self {
            linear: next.linear.mul(self.linear),
            translation: next.linear.mul_vec(self.translation) + next.translation,
        }
    }

    /// Inverse transform, or None when the linear part is singular.
    pub fn try_inverse(self) -> Option<Self> {
        let li = self.linear.try_inverse()?;
        Some(Self { linear: li, translation: -li.mul_vec(self.translation) })
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): Transform3 with Rodrigues rotation and composition"
```

---

### Task 5: Bounding boxes (Aabb2, Aabb3)

**Files:**
- Create: `crates/keel-math/src/bbox.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod bbox;`)

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/bbox.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::Vec3;

    #[test]
    fn empty_contains_nothing_and_unions_as_identity() {
        let e = Aabb3::EMPTY;
        assert!(!e.contains(Vec3::ZERO));
        let b = Aabb3::from_points([Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0)]);
        assert_eq!(e.union(b), b);
    }

    #[test]
    fn from_points_bounds_all_inputs() {
        let pts = [
            Vec3::new(1.0, -2.0, 3.0),
            Vec3::new(-1.0, 5.0, 0.0),
            Vec3::new(0.0, 0.0, -7.0),
        ];
        let b = Aabb3::from_points(pts);
        for p in pts { assert!(b.contains(p)); }
        assert_eq!(b.min, Vec3::new(-1.0, -2.0, -7.0));
        assert_eq!(b.max, Vec3::new(1.0, 5.0, 3.0));
    }

    #[test]
    fn disjoint_boxes_do_not_intersect() {
        let a = Aabb3::from_points([Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0)]);
        let b = Aabb3::from_points([Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 1.0, 1.0)]);
        assert!(!a.intersects(b));
        assert!(a.intersects(a));
        // Touching at a face counts as intersecting (closed boxes).
        let c = Aabb3::from_points([Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 1.0)]);
        assert!(a.intersects(c));
    }

    #[test]
    fn expanded_grows_symmetrically() {
        let b = Aabb3::from_points([Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0)]).expanded(0.5);
        assert_eq!(b.min, Vec3::new(-0.5, -0.5, -0.5));
        assert_eq!(b.max, Vec3::new(1.5, 1.5, 1.5));
    }
}
```

Add `pub mod bbox;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Aabb3` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `bbox.rs`:
```rust
//! Axis-aligned bounding boxes (closed intervals per axis).

use crate::vec::{Vec2, Vec3};

macro_rules! impl_aabb {
    ($B:ident, $V:ident { $($f:ident),+ }) => {
        /// Axis-aligned box; `EMPTY` has inverted bounds.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $B { pub min: $V, pub max: $V }

        impl $B {
            pub const EMPTY: Self = Self {
                min: $V { $($f: f64::INFINITY),+ },
                max: $V { $($f: f64::NEG_INFINITY),+ },
            };
            /// Smallest box containing all points (EMPTY for no points).
            pub fn from_points(points: impl IntoIterator<Item = $V>) -> Self {
                let mut b = Self::EMPTY;
                for p in points { b = b.grown_to(p); }
                b
            }
            /// This box grown to include point `p`.
            #[inline]
            pub fn grown_to(self, p: $V) -> Self {
                Self {
                    min: $V { $($f: self.min.$f.min(p.$f)),+ },
                    max: $V { $($f: self.max.$f.max(p.$f)),+ },
                }
            }
            /// Closed-interval point containment.
            #[inline]
            pub fn contains(self, p: $V) -> bool {
                true $(&& self.min.$f <= p.$f && p.$f <= self.max.$f)+
            }
            /// Smallest box containing both.
            #[inline]
            pub fn union(self, o: Self) -> Self {
                Self {
                    min: $V { $($f: self.min.$f.min(o.min.$f)),+ },
                    max: $V { $($f: self.max.$f.max(o.max.$f)),+ },
                }
            }
            /// Closed-interval overlap test (shared boundary counts).
            #[inline]
            pub fn intersects(self, o: Self) -> bool {
                true $(&& self.min.$f <= o.max.$f && o.min.$f <= self.max.$f)+
            }
            /// Box expanded by `r` on every side (negative shrinks).
            #[inline]
            pub fn expanded(self, r: f64) -> Self {
                Self {
                    min: $V { $($f: self.min.$f - r),+ },
                    max: $V { $($f: self.max.$f + r),+ },
                }
            }
        }
    };
}

impl_aabb!(Aabb2, Vec2 { x, y });
impl_aabb!(Aabb3, Vec3 { x, y, z });
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): Aabb2/Aabb3 closed-interval bounding boxes"
```

---

### Task 6: Interval arithmetic

**Files:**
- Create: `crates/keel-math/src/interval.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod interval;`)

Design note for the engineer: Rust gives no portable access to FPU rounding modes, so we get conservative (outward-rounded) intervals by computing in round-to-nearest and then widening each bound by one ulp via `f64::next_down`/`next_up`. This over-widens slightly; that is safe. Soundness invariant: for the true real result r of an operation on reals inside the operand intervals, r is inside the result interval. All inputs are assumed finite; constructors debug_assert this.

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/interval.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn point_interval_contains_its_value() {
        let i = Interval::point(0.1);
        assert!(i.contains(0.1));
    }

    #[test]
    fn sign_detection() {
        assert_eq!(Interval::new(1.0, 2.0).sign(), Some(1));
        assert_eq!(Interval::new(-2.0, -1.0).sign(), Some(-1));
        assert_eq!(Interval::new(-1.0, 1.0).sign(), None);
        assert_eq!(Interval::point(0.0).sign(), Some(0));
    }

    #[test]
    fn sqrt_of_negative_is_none() {
        assert!(Interval::new(-2.0, -1.0).sqrt().is_none());
        // Straddling zero clamps the low bound to 0.
        let s = Interval::new(-1.0, 4.0).sqrt().unwrap();
        assert!(s.lo <= 0.0 && s.hi >= 2.0);
    }

    fn small() -> impl Strategy<Value = f64> { -1.0e3..1.0e3f64 }

    proptest! {
        // Soundness: real arithmetic on members stays inside the result.
        #[test]
        fn add_is_sound(a in small(), b in small()) {
            let r = Interval::point(a) + Interval::point(b);
            prop_assert!(r.lo <= a + b && a + b <= r.hi);
        }
        #[test]
        fn mul_is_sound(a in small(), b in small()) {
            let r = Interval::point(a) * Interval::point(b);
            prop_assert!(r.lo <= a * b && a * b <= r.hi);
        }
        #[test]
        fn mul_sound_for_wide_operands(
            a in small(), b in small(), c in small(), d in small(),
            t in 0.0..1.0f64, u in 0.0..1.0f64,
        ) {
            let i = Interval::new(a.min(b), a.max(b));
            let j = Interval::new(c.min(d), c.max(d));
            let x = i.lo + t * (i.hi - i.lo);
            let y = j.lo + u * (j.hi - j.lo);
            let r = i * j;
            prop_assert!(r.lo <= x * y && x * y <= r.hi);
        }
    }
}
```

Add `pub mod interval;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Interval` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `interval.rs`:
```rust
//! Conservative interval arithmetic via one-ulp outward widening.

use core::ops::{Add, Mul, Neg, Sub};

/// Closed interval [lo, hi]; finite bounds; lo <= hi.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval { pub lo: f64, pub hi: f64 }

impl Interval {
    /// New interval; debug_asserts finite, ordered bounds.
    #[inline]
    pub fn new(lo: f64, hi: f64) -> Self {
        debug_assert!(lo.is_finite() && hi.is_finite() && lo <= hi);
        Self { lo, hi }
    }
    /// Degenerate interval [x, x].
    #[inline]
    pub fn point(x: f64) -> Self { Self::new(x, x) }
    /// Widen both bounds by one ulp (the outward-rounding step).
    #[inline]
    fn widened(self) -> Self {
        Self { lo: self.lo.next_down(), hi: self.hi.next_up() }
    }
    #[inline]
    pub fn contains(self, x: f64) -> bool { self.lo <= x && x <= self.hi }
    #[inline]
    pub fn width(self) -> f64 { self.hi - self.lo }
    /// Certified sign: Some(1)/Some(-1) when strictly one-signed,
    /// Some(0) for the exact zero point interval, None when ambiguous.
    #[inline]
    pub fn sign(self) -> Option<i8> {
        if self.lo > 0.0 { Some(1) }
        else if self.hi < 0.0 { Some(-1) }
        else if self.lo == 0.0 && self.hi == 0.0 { Some(0) }
        else { None }
    }
    /// Conservative square root; None when entirely negative.
    /// A straddling interval is clamped to [0, hi] first.
    pub fn sqrt(self) -> Option<Self> {
        if self.hi < 0.0 { return None; }
        let lo = self.lo.max(0.0).sqrt();
        let hi = self.hi.sqrt();
        Some(Self { lo, hi }.widened())
    }
}

impl Add for Interval {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        Self { lo: self.lo + o.lo, hi: self.hi + o.hi }.widened()
    }
}

impl Sub for Interval {
    type Output = Self;
    #[inline]
    fn sub(self, o: Self) -> Self {
        Self { lo: self.lo - o.hi, hi: self.hi - o.lo }.widened()
    }
}

impl Neg for Interval {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self { Self { lo: -self.hi, hi: -self.lo } }
}

impl Mul for Interval {
    type Output = Self;
    #[inline]
    fn mul(self, o: Self) -> Self {
        let p = [self.lo * o.lo, self.lo * o.hi, self.hi * o.lo, self.hi * o.hi];
        let mut lo = p[0];
        let mut hi = p[0];
        for &v in &p[1..] {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        Self { lo, hi }.widened()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): conservative interval arithmetic with soundness property tests"
```

---

### Task 7: Tolerance model

**Files:**
- Create: `crates/keel-math/src/tolerance.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod tolerance;`)

Design note: this module is the single home of tolerance policy (spec D2). Session defaults follow Parasolid's published figures. Per-entity tolerance values appear on topology entities in M3; they will always be >= the session linear resolution. No other module may define an epsilon.

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/tolerance.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let t = Tolerances::default();
        assert_eq!(t.linear, 1.0e-8);
        assert_eq!(t.angular, 1.0e-11);
    }

    #[test]
    fn linear_equality_is_tolerant() {
        let t = Tolerances::default();
        assert!(t.eq_linear(1.0, 1.0 + 0.5e-8));
        assert!(!t.eq_linear(1.0, 1.0 + 2.0e-8));
        assert!(t.is_zero_linear(-0.9e-8));
    }

    #[test]
    fn angular_equality_is_tolerant() {
        let t = Tolerances::default();
        assert!(t.eq_angular(0.5, 0.5 + 0.5e-11));
        assert!(!t.eq_angular(0.5, 0.5 + 2.0e-11));
    }

    #[test]
    fn construction_rejects_nonpositive() {
        assert!(Tolerances::new(0.0, 1e-11).is_none());
        assert!(Tolerances::new(1e-8, -1.0).is_none());
        assert!(Tolerances::new(1e-6, 1e-9).is_some());
    }
}
```

Add `pub mod tolerance;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Tolerances` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `tolerance.rs`:
```rust
//! Session tolerance policy: the only module allowed to define epsilons.
//!
//! Model (spec D2): per-entity local tolerances are the primary model;
//! the session values here are the floor/default. Parasolid's published
//! figures are adopted: linear resolution 1e-8 in a normalized model
//! box, angular resolution 1e-11 radians.

/// Session-wide tolerance floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerances {
    /// Linear resolution: distances below this are coincident.
    pub linear: f64,
    /// Angular resolution in radians.
    pub angular: f64,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self { linear: 1.0e-8, angular: 1.0e-11 }
    }
}

impl Tolerances {
    /// Custom tolerances; None unless both are finite and positive.
    pub fn new(linear: f64, angular: f64) -> Option<Self> {
        let ok = |x: f64| x.is_finite() && x > 0.0;
        if ok(linear) && ok(angular) { Some(Self { linear, angular }) } else { None }
    }
    /// |a - b| <= linear resolution.
    #[inline]
    pub fn eq_linear(self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.linear
    }
    /// |x| <= linear resolution.
    #[inline]
    pub fn is_zero_linear(self, x: f64) -> bool {
        x.abs() <= self.linear
    }
    /// |a - b| <= angular resolution (radians).
    #[inline]
    pub fn eq_angular(self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.angular
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): centralized session tolerance policy"
```

---

### Task 8: Exact predicates

**Files:**
- Create: `crates/keel-math/src/predicates.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod predicates;`)

Design note: this wraps the `robust` crate (Rust port of Shewchuk's adaptive predicates, internal filtering included) behind sign enums so kernel code never touches the raw f64 result. Convention documented and tested: `orient_2d(a, b, c)` is Positive when c lies to the left of directed line a->b (counterclockwise triangle); `orient_3d(a, b, c, d)` is Positive when d lies on the side of plane a,b,c from which the triangle a,b,c appears counterclockwise. If the `robust` crate's API names differ from those used below, adapt only inside this module; check https://docs.rs/robust.

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/predicates.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::{Vec2, Vec3};
    use proptest::prelude::*;

    #[test]
    fn orient2d_basic_convention() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 0.0);
        assert_eq!(orient_2d(a, b, Vec2::new(0.0, 1.0)), Sign::Positive);
        assert_eq!(orient_2d(a, b, Vec2::new(0.0, -1.0)), Sign::Negative);
        assert_eq!(orient_2d(a, b, Vec2::new(2.0, 0.0)), Sign::Zero);
    }

    #[test]
    fn orient3d_basic_convention() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(orient_3d(a, b, c, Vec3::new(0.0, 0.0, 1.0)), Sign::Positive);
        assert_eq!(orient_3d(a, b, c, Vec3::new(0.0, 0.0, -1.0)), Sign::Negative);
        assert_eq!(orient_3d(a, b, c, Vec3::new(0.3, 0.3, 0.0)), Sign::Zero);
    }

    #[test]
    fn exactness_on_adversarial_near_degenerate_grid() {
        // Classic Kettner-style stress: points on a line built from
        // floats that do not sum exactly. The exact predicate must be
        // perfectly antisymmetric and transitive-consistent where a
        // naive determinant flips signs unpredictably.
        let base = Vec2::new(0.5, 0.5);
        let dir = Vec2::new(12.0, 12.0);
        for i in 0..64 {
            for j in 0..64 {
                let p = base + dir * (i as f64 * 1.0e-16);
                let q = base + dir * (0.5 + j as f64 * 1.0e-16);
                let r = base + dir * 1.0;
                let s1 = orient_2d(p, q, r);
                let s2 = orient_2d(q, p, r);
                // Exact antisymmetry must hold for every perturbation.
                assert_eq!(s1, s2.flipped(), "i={i} j={j}");
            }
        }
    }

    fn finite_vec2() -> impl Strategy<Value = Vec2> {
        let f = -1.0e6..1.0e6f64;
        (f.clone(), f).prop_map(|(x, y)| Vec2::new(x, y))
    }

    proptest! {
        #[test]
        fn orient2d_antisymmetric(a in finite_vec2(), b in finite_vec2(), c in finite_vec2()) {
            prop_assert_eq!(orient_2d(a, b, c), orient_2d(b, a, c).flipped());
        }
        #[test]
        fn orient2d_cyclic(a in finite_vec2(), b in finite_vec2(), c in finite_vec2()) {
            prop_assert_eq!(orient_2d(a, b, c), orient_2d(b, c, a));
        }
    }
}
```

Add `pub mod predicates;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Sign`/`orient_2d` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `predicates.rs`:
```rust
//! Exact geometric predicates (EPICK discipline, spec D2).
//!
//! Wraps the `robust` crate (Shewchuk adaptive predicates with
//! internal floating-point filters). Kernel code must use these sign
//! enums for every combinatorial decision; branching on a raw f64
//! sign anywhere else in the codebase is a defect.

use crate::vec::{Vec2, Vec3};

/// Certified sign of an exact predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign { Negative, Zero, Positive }

impl Sign {
    /// From an exactly-signed f64 (the robust crate's return contract).
    #[inline]
    fn from_f64(v: f64) -> Self {
        if v > 0.0 { Self::Positive }
        else if v < 0.0 { Self::Negative }
        else { Self::Zero }
    }
    /// Negated sign.
    #[inline]
    pub fn flipped(self) -> Self {
        match self {
            Self::Negative => Self::Positive,
            Self::Zero => Self::Zero,
            Self::Positive => Self::Negative,
        }
    }
}

/// Positive when c is left of directed line a->b (a,b,c counterclockwise).
#[inline]
pub fn orient_2d(a: Vec2, b: Vec2, c: Vec2) -> Sign {
    let v = robust::orient2d(
        robust::Coord { x: a.x, y: a.y },
        robust::Coord { x: b.x, y: b.y },
        robust::Coord { x: c.x, y: c.y },
    );
    Sign::from_f64(v)
}

/// Positive when d is on the side of plane a,b,c from which a,b,c
/// appears counterclockwise.
#[inline]
pub fn orient_3d(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Sign {
    let v = robust::orient3d(
        robust::Coord3D { x: a.x, y: a.y, z: a.z },
        robust::Coord3D { x: b.x, y: b.y, z: b.z },
        robust::Coord3D { x: c.x, y: c.y, z: c.z },
        robust::Coord3D { x: d.x, y: d.y, z: d.z },
    );
    // robust::orient3d returns positive when d is BELOW the plane
    // (Shewchuk's convention); flip to our left-handed-free convention
    // verified by the basic-convention unit test. If the unit test
    // fails on sign, remove this flip; the test is the arbiter.
    Sign::from_f64(v).flipped()
}

/// True when a, b, c are exactly collinear.
#[inline]
pub fn collinear_2d(a: Vec2, b: Vec2, c: Vec2) -> bool {
    orient_2d(a, b, c) == Sign::Zero
}

/// True when a, b, c, d are exactly coplanar.
#[inline]
pub fn coplanar_3d(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    orient_3d(a, b, c, d) == Sign::Zero
}
```

Note to the engineer: the `orient_3d` sign convention against `robust` must be settled empirically by the `orient3d_basic_convention` unit test; Shewchuk's raw convention is "positive = below plane". Keep or remove the `.flipped()` so the unit test passes, and leave the comment explaining whichever direction was needed.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass (adjust the flip per the note if the convention test fails; everything else must pass untouched).

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): exact orientation predicates behind sign-enum API"
```

---

### Task 9: Bracketed Newton solver

**Files:**
- Create: `crates/keel-math/src/newton.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod newton;`)

Design note: the single 1D root-polishing engine for the whole kernel (poly and Bernstein both call it). Newton-bisection hybrid: a maintained bracket guarantees convergence; Newton inside the bracket gives quadratic speed. Requires f(lo) and f(hi) with opposite signs.

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/newton.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_sqrt2() {
        let f = |x: f64| (x * x - 2.0, 2.0 * x);
        let r = solve_bracketed(f, 0.0, 2.0, 1e-14, 100).unwrap();
        assert!((r - 2.0_f64.sqrt()).abs() < 1e-14);
    }

    #[test]
    fn endpoint_roots_returned_immediately() {
        let f = |x: f64| (x, 1.0);
        assert_eq!(solve_bracketed(f, 0.0, 1.0, 1e-14, 100), Some(0.0));
    }

    #[test]
    fn no_sign_change_is_none() {
        let f = |x: f64| (x * x + 1.0, 2.0 * x);
        assert_eq!(solve_bracketed(f, -1.0, 1.0, 1e-14, 100), None);
    }

    #[test]
    fn survives_flat_derivative() {
        // f(x) = x^3 has zero derivative at the root; bisection
        // fallback must still converge.
        let f = |x: f64| (x * x * x, 3.0 * x * x);
        let r = solve_bracketed(f, -1.0, 2.0, 1e-12, 200).unwrap();
        assert!(r.abs() < 1e-10);
    }
}
```

Add `pub mod newton;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `solve_bracketed` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `newton.rs`:
```rust
//! Bracketed Newton-bisection hybrid: the kernel's 1D root engine.

/// Find x in [lo, hi] with f(x) = 0, where `f` returns (value,
/// derivative) and f(lo), f(hi) have opposite signs (or either
/// endpoint is already a root). Newton steps are taken when they stay
/// inside the current bracket and shrink it; otherwise bisect.
/// Returns None when there is no sign change over the input bracket.
pub fn solve_bracketed<F>(
    f: F,
    mut lo: f64,
    mut hi: f64,
    x_tol: f64,
    max_iter: u32,
) -> Option<f64>
where
    F: Fn(f64) -> (f64, f64),
{
    let (mut flo, _) = f(lo);
    let (fhi, _) = f(hi);
    if flo == 0.0 { return Some(lo); }
    if fhi == 0.0 { return Some(hi); }
    if (flo > 0.0) == (fhi > 0.0) { return None; }

    let mut x = 0.5 * (lo + hi);
    for _ in 0..max_iter {
        let (fx, dfx) = f(x);
        if fx == 0.0 { return Some(x); }
        // Shrink the bracket around the sign change.
        if (fx > 0.0) == (flo > 0.0) {
            lo = x;
            flo = fx;
        } else {
            hi = x;
        }
        if hi - lo <= x_tol { return Some(0.5 * (lo + hi)); }
        // Newton proposal; fall back to bisection when it exits the
        // bracket or the derivative is unusable.
        let newton = x - fx / dfx;
        x = if newton.is_finite() && newton > lo && newton < hi {
            newton
        } else {
            0.5 * (lo + hi)
        };
    }
    Some(0.5 * (lo + hi))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): bracketed Newton-bisection 1D root engine"
```

---

### Task 10: Polynomial solvers (quadratic, cubic)

**Files:**
- Create: `crates/keel-math/src/poly.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod poly;`)

Design note (research track B): quadratics use the Blinn/Press stable form (compute q, derive both roots from it, never subtract nearly equal quantities). Cubics use the Yuksel HPG 2022 pattern: locate the derivative's roots to split the domain into monotonic intervals, then run the bracketed Newton engine on each interval with a sign change. Cauchy's bound caps the search domain. Naive Cardano is banned. Quartics are deferred until torus intersections need them (M4).

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/poly.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn assert_roots_eq(mut got: Vec<f64>, mut want: Vec<f64>, tol: f64) {
        got.sort_by(f64::total_cmp);
        want.sort_by(f64::total_cmp);
        assert_eq!(got.len(), want.len(), "got {got:?} want {want:?}");
        for (g, w) in got.iter().zip(want.iter()) {
            assert!((g - w).abs() < tol, "got {got:?} want {want:?}");
        }
    }

    #[test]
    fn quadratic_simple() {
        // (x - 1)(x - 3) = x^2 - 4x + 3
        assert_roots_eq(solve_quadratic(1.0, -4.0, 3.0), vec![1.0, 3.0], 1e-14);
    }

    #[test]
    fn quadratic_catastrophic_cancellation() {
        // Roots 1e-8 and 1e8: the naive formula destroys the small root.
        let roots = solve_quadratic(1.0, -(1.0e8 + 1.0e-8), 1.0);
        assert_roots_eq(roots, vec![1.0e-8, 1.0e8], 1e-6);
        // Tight relative check on the small root.
        let small = solve_quadratic(1.0, -(1.0e8 + 1.0e-8), 1.0)
            .into_iter().fold(f64::MAX, f64::min);
        assert!((small - 1.0e-8).abs() / 1.0e-8 < 1e-9);
    }

    #[test]
    fn quadratic_degenerates_to_linear() {
        assert_roots_eq(solve_quadratic(0.0, 2.0, -4.0), vec![2.0], 1e-14);
        assert!(solve_quadratic(0.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn quadratic_no_real_roots() {
        assert!(solve_quadratic(1.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn cubic_three_roots() {
        // (x + 2)(x - 1)(x - 5) = x^3 - 4x^2 - 7x + 10
        assert_roots_eq(
            solve_cubic(1.0, -4.0, -7.0, 10.0),
            vec![-2.0, 1.0, 5.0],
            1e-10,
        );
    }

    #[test]
    fn cubic_double_root() {
        // (x - 1)^2 (x + 2) = x^3 - 3x + 2; root 1 has multiplicity 2.
        let roots = solve_cubic(1.0, 0.0, -3.0, 2.0);
        assert!(roots.iter().any(|r| (r - 1.0).abs() < 1e-6));
        assert!(roots.iter().any(|r| (r + 2.0).abs() < 1e-10));
    }

    #[test]
    fn cubic_single_real_root() {
        // x^3 + x + 1 has exactly one real root near -0.6823.
        let roots = solve_cubic(1.0, 0.0, 1.0, 1.0);
        assert_eq!(roots.len(), 1);
        assert!((roots[0] + 0.682_327_803_828_019_3).abs() < 1e-12);
    }

    proptest! {
        // Oracle: build the cubic from chosen roots, recover them.
        #[test]
        fn cubic_recovers_constructed_roots(
            r1 in -100.0..100.0f64,
            r2 in -100.0..100.0f64,
            r3 in -100.0..100.0f64,
        ) {
            // Skip clustered roots; multiplicity accuracy is tested
            // separately and deterministically.
            prop_assume!((r1 - r2).abs() > 1e-3);
            prop_assume!((r2 - r3).abs() > 1e-3);
            prop_assume!((r1 - r3).abs() > 1e-3);
            let b = -(r1 + r2 + r3);
            let c = r1 * r2 + r1 * r3 + r2 * r3;
            let d = -(r1 * r2 * r3);
            let got = solve_cubic(1.0, b, c, d);
            prop_assert_eq!(got.len(), 3);
            for want in [r1, r2, r3] {
                prop_assert!(
                    got.iter().any(|g| (g - want).abs() < 1e-6),
                    "missing root {} in {:?}", want, got
                );
            }
        }
    }
}
```

Add `pub mod poly;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `solve_quadratic` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `poly.rs`:
```rust
//! Closed-degree polynomial solvers (research track B: Blinn-stable
//! quadratic, Yuksel-style monotonic-interval cubic). Power-basis
//! evaluation of higher degrees is banned; use the bernstein module.

use crate::newton::solve_bracketed;

/// Real roots of a x^2 + b x + c, ascending. Stable form: no
/// subtraction of nearly equal quantities (Blinn / Numerical Recipes).
pub fn solve_quadratic(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a == 0.0 {
        if b == 0.0 { return Vec::new(); }
        return vec![-c / b];
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 { return Vec::new(); }
    let sqrt_disc = disc.sqrt();
    // q has the sign chosen so b + sign(b)*sqrt_disc never cancels.
    let q = -0.5 * (b + b.signum() * sqrt_disc);
    let mut roots = if q == 0.0 {
        vec![0.0]
    } else {
        let r1 = q / a;
        let r2 = c / q;
        if r1 == r2 { vec![r1] } else { vec![r1, r2] }
    };
    roots.sort_by(f64::total_cmp);
    roots
}

/// Real roots of a x^3 + b x^2 + c x + d, ascending, deduplicated.
/// Monotonic-interval strategy (Yuksel HPG 2022): split the domain at
/// the derivative's roots, run bracketed Newton on each sign change.
pub fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    if a == 0.0 { return solve_quadratic(b, c, d); }

    // Cauchy bound: all real roots lie in [-bound, bound].
    let bound = 1.0 + (b.abs().max(c.abs()).max(d.abs())) / a.abs();

    let eval = |x: f64| {
        let f = ((a * x + b) * x + c) * x + d;
        let df = (3.0 * a * x + 2.0 * b) * x + c;
        (f, df)
    };

    // Domain split points: derivative roots inside the bound.
    let mut cuts = vec![-bound];
    for r in solve_quadratic(3.0 * a, 2.0 * b, c) {
        if r > -bound && r < bound { cuts.push(r); }
    }
    cuts.push(bound);

    let mut roots = Vec::with_capacity(3);
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0], w[1]);
        // A critical point that IS a root (multiple root) is caught
        // here, since solve_bracketed returns endpoint roots.
        if let Some(r) = solve_bracketed(eval, lo, hi, 1e-14 * bound, 128) {
            roots.push(r);
        }
    }
    roots.sort_by(f64::total_cmp);
    // Deduplicate multiple roots found from adjacent intervals.
    roots.dedup_by(|x, y| (*x - *y).abs() <= 1e-12 * bound);
    roots
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass. The double-root test exercises the endpoint-root path in `solve_bracketed`; if it fails, check that critical points evaluating exactly or nearly to zero are returned by the interval whose endpoint they are.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): stable quadratic and monotonic-interval cubic solvers"
```

---

### Task 11: Univariate Bernstein machinery

**Files:**
- Create: `crates/keel-math/src/bernstein.rs`
- Modify: `crates/keel-math/src/lib.rs` (add `pub mod bernstein;`)

Design note (research track B): Bernstein coefficients on [0,1], de Casteljau evaluation (backward stable), subdivision, derivative, and root isolation by sign-variation subdivision (variation-diminishing property: zero variations means zero roots; one variation with an endpoint sign change brackets exactly one root, polished by the Newton engine). This is the seed of the multivariate Projected Polyhedron solver built in M2.

- [ ] **Step 1: Write the failing tests**

`crates/keel-math/src/bernstein.rs`, tests first:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn evaluates_linear() {
        // Bernstein coeffs [0, 1] is the identity on [0,1].
        let p = Bernstein::new(vec![0.0, 1.0]).unwrap();
        assert_eq!(p.eval(0.25), 0.25);
    }

    #[test]
    fn rejects_empty() {
        assert!(Bernstein::new(vec![]).is_none());
    }

    #[test]
    fn matches_power_basis() {
        // p(x) = 2 - 3x + x^2 in power basis.
        let p = Bernstein::from_power(&[2.0, -3.0, 1.0]).unwrap();
        for &t in &[0.0, 0.1, 0.5, 0.9, 1.0] {
            let want = 2.0 - 3.0 * t + t * t;
            assert!((p.eval(t) - want).abs() < 1e-14, "t={t}");
        }
    }

    #[test]
    fn subdivision_preserves_values() {
        let p = Bernstein::from_power(&[1.0, -2.0, 0.5, 3.0]).unwrap();
        let (l, r) = p.subdivide(0.3);
        // Left piece reparameterized: l(s) = p(0.3 s).
        assert!((l.eval(0.5) - p.eval(0.15)).abs() < 1e-13);
        // Right piece: r(s) = p(0.3 + 0.7 s).
        assert!((r.eval(0.5) - p.eval(0.65)).abs() < 1e-13);
    }

    #[test]
    fn derivative_of_quadratic() {
        // p(x) = x^2, p'(x) = 2x.
        let p = Bernstein::from_power(&[0.0, 0.0, 1.0]).unwrap();
        let d = p.derivative();
        assert!((d.eval(0.7) - 1.4).abs() < 1e-14);
    }

    #[test]
    fn roots_of_known_cubic() {
        // p(x) = (x - 0.2)(x - 0.5)(x - 0.9), roots all inside (0,1).
        let p = Bernstein::from_power(&[-0.09, 0.73, -1.6, 1.0]).unwrap();
        let roots = p.roots(1e-12);
        assert_eq!(roots.len(), 3);
        for (got, want) in roots.iter().zip([0.2, 0.5, 0.9]) {
            assert!((got - want).abs() < 1e-9, "got {roots:?}");
        }
    }

    #[test]
    fn no_roots_when_strictly_positive() {
        let p = Bernstein::new(vec![1.0, 2.0, 0.5]).unwrap();
        assert!(p.roots(1e-12).is_empty());
    }

    proptest! {
        // Oracle: cross-check against the cubic solver on [0,1].
        #[test]
        fn roots_match_cubic_solver(
            b in -5.0..5.0f64, c in -5.0..5.0f64, d in -5.0..5.0f64,
        ) {
            let p = Bernstein::from_power(&[d, c, b, 1.0]).unwrap();
            let got = p.roots(1e-12);
            let want: Vec<f64> = crate::poly::solve_cubic(1.0, b, c, d)
                .into_iter()
                .filter(|r| (0.0..=1.0).contains(r))
                // Drop roots so close to another that dedup policies
                // differ between the two solvers.
                .collect();
            // Compare as sets within tolerance, skipping clustered cases.
            let clustered = want.windows(2).any(|w| (w[1] - w[0]).abs() < 1e-4)
                || want.iter().any(|r| *r < 1e-4 || *r > 1.0 - 1e-4);
            prop_assume!(!clustered);
            prop_assert_eq!(got.len(), want.len(),
                "got {:?} want {:?}", &got, &want);
            for (g, w) in got.iter().zip(want.iter()) {
                prop_assert!((g - w).abs() < 1e-7,
                    "got {:?} want {:?}", &got, &want);
            }
        }
    }
}
```

Add `pub mod bernstein;` to `lib.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p keel-math`
Expected: compile error, `Bernstein` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `bernstein.rs`:
```rust
//! Univariate polynomials in Bernstein form on [0,1]: backward-stable
//! evaluation, subdivision, and variation-diminishing root isolation.
//! Seed of the M2 multivariate subdivision solver (spec D6).

use crate::newton::solve_bracketed;

/// Polynomial in Bernstein basis on [0,1]; degree = coeffs.len() - 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Bernstein {
    coeffs: Vec<f64>,
}

impl Bernstein {
    /// None when `coeffs` is empty.
    pub fn new(coeffs: Vec<f64>) -> Option<Self> {
        if coeffs.is_empty() { None } else { Some(Self { coeffs }) }
    }

    pub fn degree(&self) -> usize { self.coeffs.len() - 1 }

    /// Convert from power basis (`power[i]` multiplies x^i), for tests
    /// and for analytic-geometry callers that derive power-form
    /// polynomials. b_i = sum_{j<=i} C(i,j)/C(n,j) * power[j].
    pub fn from_power(power: &[f64]) -> Option<Self> {
        if power.is_empty() { return None; }
        let n = power.len() - 1;
        let mut coeffs = vec![0.0; n + 1];
        for (i, c) in coeffs.iter_mut().enumerate() {
            let mut acc = 0.0;
            // ratio = C(i,j) / C(n,j), built incrementally.
            let mut ratio = 1.0;
            for (j, &p) in power.iter().enumerate().take(i + 1) {
                if j > 0 {
                    ratio *= (i - j + 1) as f64 / (n - j + 1) as f64;
                }
                acc += ratio * p;
            }
            *c = acc;
        }
        Self::new(coeffs)
    }

    /// De Casteljau evaluation (backward stable).
    pub fn eval(&self, t: f64) -> f64 {
        let mut w = self.coeffs.clone();
        let mut len = w.len();
        while len > 1 {
            for i in 0..len - 1 {
                w[i] = (1.0 - t) * w[i] + t * w[i + 1];
            }
            len -= 1;
        }
        w[0]
    }

    /// Derivative, degree n-1 (constant zero for degree 0 input).
    pub fn derivative(&self) -> Self {
        let n = self.degree();
        if n == 0 {
            return Self { coeffs: vec![0.0] };
        }
        let coeffs = self
            .coeffs
            .windows(2)
            .map(|w| n as f64 * (w[1] - w[0]))
            .collect();
        Self { coeffs }
    }

    /// Split at parameter t into left ([0,t] reparameterized to [0,1])
    /// and right ([t,1] reparameterized to [0,1]) via de Casteljau.
    pub fn subdivide(&self, t: f64) -> (Self, Self) {
        let mut w = self.coeffs.clone();
        let n = w.len();
        let mut left = Vec::with_capacity(n);
        let mut right = vec![0.0; n];
        right[n - 1] = w[n - 1];
        left.push(w[0]);
        for level in 1..n {
            for i in 0..n - level {
                w[i] = (1.0 - t) * w[i] + t * w[i + 1];
            }
            left.push(w[0]);
            right[n - 1 - level] = w[n - 1 - level];
        }
        (Self { coeffs: left }, Self { coeffs: right })
    }

    /// Count strict sign changes in the coefficient sequence, skipping
    /// zeros. Variation-diminishing: an upper bound on root count.
    fn sign_variations(&self) -> usize {
        let mut count = 0;
        let mut prev = 0.0f64;
        for &c in &self.coeffs {
            if c == 0.0 { continue; }
            if prev != 0.0 && (c > 0.0) != (prev > 0.0) {
                count += 1;
            }
            prev = c;
        }
        count
    }

    /// All roots in [0,1], ascending. Subdivide until each piece has
    /// zero variations (discard) or one variation with an endpoint
    /// sign change (bracketed Newton polish); pieces narrower than
    /// `t_tol` yield their midpoint (root clusters).
    pub fn roots(&self, t_tol: f64) -> Vec<f64> {
        let mut out = Vec::new();
        let deriv = self.derivative();
        self.roots_rec(0.0, 1.0, &deriv, t_tol, &mut out);
        out.sort_by(f64::total_cmp);
        out.dedup_by(|a, b| (*a - *b).abs() <= t_tol.max(1e-12));
        out
    }

    fn roots_rec(
        &self,
        t0: f64,
        t1: f64,
        deriv: &Self,
        t_tol: f64,
        out: &mut Vec<f64>,
    ) {
        match self.sign_variations() {
            0 => {
                // No roots, unless an endpoint coefficient is exactly 0.
                if self.coeffs[0] == 0.0 { out.push(t0); }
                if *self.coeffs.last().unwrap_or(&1.0) == 0.0 { out.push(t1); }
            }
            1 if (self.coeffs[0] > 0.0)
                != (*self.coeffs.last().unwrap_or(&0.0) > 0.0) =>
            {
                // Exactly one bracketed root: polish in global coords.
                let f = |t: f64| {
                    // Map global t back into this piece's local coords.
                    let s = (t - t0) / (t1 - t0);
                    (self.eval(s), deriv.eval(t) * 1.0)
                };
                // Use a derivative-free fallback: pass value with the
                // GLOBAL derivative evaluated on the original poly.
                if let Some(r) = solve_bracketed(f, t0, t1, t_tol, 128) {
                    out.push(r);
                }
            }
            _ => {
                if t1 - t0 <= t_tol {
                    out.push(0.5 * (t0 + t1));
                    return;
                }
                let (l, r) = self.subdivide(0.5);
                let tm = 0.5 * (t0 + t1);
                l.roots_rec(t0, tm, deriv, t_tol, out);
                r.roots_rec(tm, t1, deriv, t_tol, out);
            }
        }
    }
}
```

Implementation caution for the engineer: in `roots_rec`'s bracketed branch, `deriv` is the derivative of the ORIGINAL polynomial on global [0,1] coordinates while `self` is a subdivided piece on local coordinates; the closure maps global t to local s for the value and evaluates the derivative at global t. The chain-rule scale factor between local and global derivatives is a positive constant and does not affect root finding through `solve_bracketed` (it only perturbs Newton step length, and the bracket guarantees convergence), but verify the `roots_of_known_cubic` and proptest oracle pass; if Newton stalls, replace the derivative component with `(self.eval(s + h) - self.eval(s)) / h`-free central differences or simply pass the piece's own derivative scaled by `1.0 / (t1 - t0)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p keel-math`
Expected: all pass, including the proptest oracle against `solve_cubic`.

- [ ] **Step 5: Commit**

```bash
git add crates/keel-math
git commit -m "feat(math): Bernstein-basis evaluation, subdivision, and root isolation"
```

---

### Task 12: Benchmarks

**Files:**
- Modify: `crates/keel-math/benches/math.rs` (replace placeholder)

- [ ] **Step 1: Write the benchmarks**

Replace `crates/keel-math/benches/math.rs` entirely:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use keel_math::bernstein::Bernstein;
use keel_math::poly::solve_cubic;
use keel_math::predicates::orient_3d;
use keel_math::vec::Vec3;

fn bench_orient3d(c: &mut Criterion) {
    let a = Vec3::new(0.1, 0.2, 0.3);
    let b = Vec3::new(1.0, 0.1, -0.2);
    let p = Vec3::new(-0.3, 0.9, 0.5);
    let d = Vec3::new(0.4, 0.4, 0.40000001);
    c.bench_function("orient3d_generic", |bch| {
        bch.iter(|| orient_3d(black_box(a), black_box(b), black_box(p), black_box(d)))
    });
}

fn bench_cubic(c: &mut Criterion) {
    c.bench_function("solve_cubic_three_roots", |bch| {
        bch.iter(|| solve_cubic(black_box(1.0), black_box(-4.0), black_box(-7.0), black_box(10.0)))
    });
}

fn bench_bernstein(c: &mut Criterion) {
    let p = Bernstein::from_power(&[-0.09, 0.73, -1.6, 1.0]).unwrap();
    c.bench_function("bernstein_eval_deg3", |bch| {
        bch.iter(|| black_box(&p).eval(black_box(0.37)))
    });
    c.bench_function("bernstein_roots_deg3", |bch| {
        bch.iter(|| black_box(&p).roots(1e-12))
    });
}

criterion_group!(benches, bench_orient3d, bench_cubic, bench_bernstein);
criterion_main!(benches);
```

- [ ] **Step 2: Run the benchmarks briefly to verify they work**

Run: `cargo bench -p keel-math -- --quick`
Expected: all four benchmarks compile, run, and report times. (Numbers are baselines, not gates, for M1.)

- [ ] **Step 3: Commit**

```bash
git add crates/keel-math/benches
git commit -m "feat(math): criterion baselines for predicates and solvers"
```

---

### Task 13: Fuzz scaffolding and CI

**Files:**
- Create: `fuzz/Cargo.toml`, `fuzz/fuzz_targets/fuzz_solve_cubic.rs`, `fuzz/fuzz_targets/fuzz_bernstein_roots.rs`, `.github/workflows/ci.yml`, `README.md`

Design note: cargo-fuzz needs nightly + libFuzzer and does not run reliably on Windows; targets are written now (they compile-check on any platform via `cargo check`) and execute on Linux CI or any Linux box. The invariants asserted are: no panic, finite outputs, roots within the Cauchy bound.

- [ ] **Step 1: Write the fuzz harnesses**

`fuzz/Cargo.toml`:
```toml
[package]
name = "keel-fuzz"
version = "0.0.0"
publish = false
edition = "2024"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
arbitrary = { version = "1", features = ["derive"] }
keel-math = { path = "../crates/keel-math" }

[[bin]]
name = "fuzz_solve_cubic"
path = "fuzz_targets/fuzz_solve_cubic.rs"
test = false
doc = false

[[bin]]
name = "fuzz_bernstein_roots"
path = "fuzz_targets/fuzz_bernstein_roots.rs"
test = false
doc = false
```

`fuzz/fuzz_targets/fuzz_solve_cubic.rs`:
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (f64, f64, f64, f64)| {
    let (a, b, c, d) = data;
    if ![a, b, c, d].iter().all(|v| v.is_finite()) { return; }
    let roots = keel_math::poly::solve_cubic(a, b, c, d);
    // Invariants: finite roots, count <= 3, inside the Cauchy bound.
    assert!(roots.len() <= 3);
    if a != 0.0 {
        let bound = 1.0 + (b.abs().max(c.abs()).max(d.abs())) / a.abs();
        for r in roots {
            assert!(r.is_finite());
            assert!(r.abs() <= bound * (1.0 + 1e-9));
        }
    }
});
```

`fuzz/fuzz_targets/fuzz_bernstein_roots.rs`:
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|coeffs: Vec<f64>| {
    if coeffs.is_empty() || coeffs.len() > 32 { return; }
    if !coeffs.iter().all(|v| v.is_finite() && v.abs() < 1e12) { return; }
    if let Some(p) = keel_math::bernstein::Bernstein::new(coeffs) {
        let roots = p.roots(1e-10);
        for r in roots {
            // Invariants: roots live in [0,1] and are finite.
            assert!(r.is_finite());
            assert!((-1e-9..=1.0 + 1e-9).contains(&r));
        }
    }
});
```

Note: the workspace `Cargo.toml` must exclude the fuzz crate. Add to the root `Cargo.toml` `[workspace]` table:
```toml
exclude = ["fuzz"]
```

- [ ] **Step 2: Verify the fuzz crate compiles (build check only on Windows)**

Run: `cargo check --manifest-path fuzz/Cargo.toml`
Expected: compiles. (Execution happens on Linux CI / a Linux box via `cargo +nightly fuzz run fuzz_solve_cubic`.)

- [ ] **Step 3: Write CI workflow and README**

`.github/workflows/ci.yml`:
```yaml
name: CI
on:
  push: { branches: [master, main] }
  pull_request: {}
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: "rustfmt, clippy" }
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
  fuzz-build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz
      - run: cargo +nightly fuzz build
```

`README.md`:
```markdown
# Keel

An open-source B-rep solid modeling kernel in Rust. NURBS and analytic
geometry, tolerant modeling on f64 with exact predicates at decision
points, and a roadmap to robust boolean operations.

Status: M1 (numeric foundations). Pre-alpha; APIs change without notice.

- Architecture spec: `docs/superpowers/specs/2026-06-07-keel-kernel-architecture-design.md`
- Research basis: `docs/research/00-synthesis.md`

## Layout

- `crates/keel-math`: vectors, matrices, transforms, intervals,
  tolerance policy, exact predicates, root finding, Bernstein machinery.
- `fuzz`: libFuzzer harnesses (run on Linux: `cargo +nightly fuzz run <target>`).

## Development

cargo test --workspace
cargo bench -p keel-math
cargo clippy --workspace --all-targets

License: MIT OR Apache-2.0.
```

- [ ] **Step 4: Run the full validation suite**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: fmt clean, zero clippy warnings, all tests pass. Fix anything that surfaces before committing.

- [ ] **Step 5: Commit**

```bash
git add fuzz .github README.md Cargo.toml
git commit -m "feat: fuzz harnesses, CI workflow, and README"
```

---

## Completion criteria (M1 exit gate)

- `cargo test --workspace` green on Windows (and CI matrix once a remote exists).
- `cargo clippy -- -D warnings` clean; no `unwrap`/`expect` in library code.
- `cargo bench` baselines recorded for orient3d, cubic, Bernstein.
- Fuzz targets compile; at least one 10-minute Linux run of each before M2 starts (or documented as pending Linux access).
- Spec D2/D6 items delivered in `keel-math`: tolerance policy module, sign-enum predicates, interval arithmetic, Newton engine, closed-degree solvers, Bernstein machinery.
- Deferred (recorded): multivariate subdivision solver and 2x2 Newton (to M2 with first consumers), quartic solver (to M4 with torus intersections), expansion-arithmetic / indirect predicates (to M5/M6 with the boolean substrate).
