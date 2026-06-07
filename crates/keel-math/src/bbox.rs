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
        for p in pts {
            assert!(b.contains(p));
        }
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
