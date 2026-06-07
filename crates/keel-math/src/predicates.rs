//! Exact geometric predicates (EPICK discipline, spec D2).
//!
//! Wraps the `robust` crate (Shewchuk adaptive predicates with
//! internal floating-point filters). Kernel code must use these sign
//! enums for every combinatorial decision; branching on a raw f64
//! sign anywhere else in the codebase is a defect.

use crate::vec::{Vec2, Vec3};

/// Certified sign of an exact predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Negative,
    Zero,
    Positive,
}

impl Sign {
    /// From an exactly-signed f64 (the robust crate's return contract).
    #[inline]
    fn from_f64(v: f64) -> Self {
        if v > 0.0 {
            Self::Positive
        } else if v < 0.0 {
            Self::Negative
        } else {
            Self::Zero
        }
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
        robust::Coord3D {
            x: a.x,
            y: a.y,
            z: a.z,
        },
        robust::Coord3D {
            x: b.x,
            y: b.y,
            z: b.z,
        },
        robust::Coord3D {
            x: c.x,
            y: c.y,
            z: c.z,
        },
        robust::Coord3D {
            x: d.x,
            y: d.y,
            z: d.z,
        },
    );
    // robust::orient3d follows Shewchuk's convention (positive when d
    // is below the plane); flip to our above-the-ccw-plane convention.
    // The orient3d_basic_convention unit test is the arbiter.
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
        // Kettner-style stress: points on a line built from floats
        // that do not sum exactly. The exact predicate must be
        // perfectly antisymmetric for every perturbation, where a
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
