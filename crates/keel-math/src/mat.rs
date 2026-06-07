//! Small dense matrices, column-major.

use crate::vec::{Vec2, Vec3};

/// 2x2 matrix, column-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat2 {
    pub cols: [Vec2; 2],
}

impl Mat2 {
    pub const IDENTITY: Self = Self {
        cols: [Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
    };
    #[inline]
    pub const fn from_cols(a: Vec2, b: Vec2) -> Self {
        Self { cols: [a, b] }
    }
    #[inline]
    pub fn determinant(self) -> f64 {
        self.cols[0].perp_dot(self.cols[1])
    }
    #[inline]
    pub fn mul_vec(self, v: Vec2) -> Vec2 {
        self.cols[0] * v.x + self.cols[1] * v.y
    }
    /// Inverse, or None when the determinant's reciprocal is not finite.
    #[inline]
    pub fn try_inverse(self) -> Option<Self> {
        let inv_det = 1.0 / self.determinant();
        if !inv_det.is_finite() {
            return None;
        }
        let [a, b] = self.cols;
        Some(Self::from_cols(
            Vec2::new(b.y, -a.y) * inv_det,
            Vec2::new(-b.x, a.x) * inv_det,
        ))
    }
}

/// 3x3 matrix, column-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub cols: [Vec3; 3],
}

impl Mat3 {
    pub const IDENTITY: Self = Self {
        cols: [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
    };
    #[inline]
    pub const fn from_cols(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { cols: [a, b, c] }
    }
    /// Scalar triple product of the columns.
    #[inline]
    pub fn determinant(self) -> f64 {
        self.cols[0].dot(self.cols[1].cross(self.cols[2]))
    }
    #[inline]
    pub fn transpose(self) -> Self {
        let [a, b, c] = self.cols;
        Self::from_cols(
            Vec3::new(a.x, b.x, c.x),
            Vec3::new(a.y, b.y, c.y),
            Vec3::new(a.z, b.z, c.z),
        )
    }
    #[inline]
    pub fn mul_vec(self, v: Vec3) -> Vec3 {
        self.cols[0] * v.x + self.cols[1] * v.y + self.cols[2] * v.z
    }
    #[inline]
    pub fn mul(self, o: Self) -> Self {
        Self {
            cols: [
                self.mul_vec(o.cols[0]),
                self.mul_vec(o.cols[1]),
                self.mul_vec(o.cols[2]),
            ],
        }
    }
    /// Inverse via the adjugate, or None when 1/det is not finite.
    pub fn try_inverse(self) -> Option<Self> {
        let [a, b, c] = self.cols;
        let r0 = b.cross(c);
        let r1 = c.cross(a);
        let r2 = a.cross(b);
        let inv_det = 1.0 / a.dot(r0);
        if !inv_det.is_finite() {
            return None;
        }
        Some(Self::from_cols(
            Vec3::new(r0.x, r1.x, r2.x) * inv_det,
            Vec3::new(r0.y, r1.y, r2.y) * inv_det,
            Vec3::new(r0.z, r1.z, r2.z) * inv_det,
        ))
    }
}

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

    #[test]
    fn mat2_inverse_roundtrips() {
        let m = Mat2::from_cols(Vec2::new(3.0, 1.0), Vec2::new(-2.0, 4.0));
        let inv = m.try_inverse().unwrap();
        let p = inv.mul_vec(m.mul_vec(Vec2::new(0.7, -1.3)));
        assert!((p - Vec2::new(0.7, -1.3)).norm() < 1e-14);
    }

    fn well_conditioned() -> impl Strategy<Value = Mat3> {
        let f = -100.0..100.0f64;
        let v = move || {
            (f.clone(), f.clone(), f.clone()).prop_map(|(x, y, z)| Vec3::new(x, y, z))
        };
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
