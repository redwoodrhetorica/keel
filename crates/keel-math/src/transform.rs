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
    pub const IDENTITY: Self = Self {
        linear: Mat3::IDENTITY,
        translation: Vec3::ZERO,
    };

    #[inline]
    pub const fn from_translation(t: Vec3) -> Self {
        Self {
            linear: Mat3::IDENTITY,
            translation: t,
        }
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
            Vec3::new(
                k.x * k.x * v + c,
                k.y * k.x * v + k.z * s,
                k.z * k.x * v - k.y * s,
            ),
            Vec3::new(
                k.x * k.y * v - k.z * s,
                k.y * k.y * v + c,
                k.z * k.y * v + k.x * s,
            ),
            Vec3::new(
                k.x * k.z * v + k.y * s,
                k.y * k.z * v - k.x * s,
                k.z * k.z * v + c,
            ),
        );
        Some(Self {
            linear,
            translation: Vec3::ZERO,
        })
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
        Some(Self {
            linear: li,
            translation: -li.mul_vec(self.translation),
        })
    }
}

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
        let t = Transform3::from_rotation(Vec3::new(1.0, 2.0, 0.5), 0.7)
            .unwrap()
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

    #[test]
    fn uniform_scale_scales() {
        let t = Transform3::from_uniform_scale(2.5);
        assert_eq!(
            t.apply_point(Vec3::new(1.0, -2.0, 4.0)),
            Vec3::new(2.5, -5.0, 10.0)
        );
    }
}
