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
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

/// 3D vector.
#[derive(Clone, Copy, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// 4D vector (homogeneous coordinates for rational geometry).
#[derive(Clone, Copy, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Vec4 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl_vec!(Vec2 { x, y });
impl_vec!(Vec3 { x, y, z });
impl_vec!(Vec4 { x, y, z, w });

impl Vec2 {
    /// 2D cross product magnitude: self.x*o.y - self.y*o.x.
    #[inline]
    pub fn perp_dot(self, o: Self) -> f64 {
        self.x * o.y - self.y * o.x
    }
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
