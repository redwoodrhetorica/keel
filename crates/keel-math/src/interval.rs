//! Conservative interval arithmetic via one-ulp outward widening.
//!
//! Rust gives no portable access to FPU rounding modes, so operations
//! compute in round-to-nearest and then widen each bound by one ulp
//! (`next_down`/`next_up`). Slightly over-wide, always sound: the true
//! real result of an operation on members of the operand intervals is
//! contained in the result interval. Inputs are assumed finite.

use core::ops::{Add, Mul, Neg, Sub};

/// Closed interval [lo, hi]; finite bounds; lo <= hi.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    /// New interval; debug_asserts finite, ordered bounds.
    #[inline]
    pub fn new(lo: f64, hi: f64) -> Self {
        debug_assert!(lo.is_finite() && hi.is_finite() && lo <= hi);
        Self { lo, hi }
    }
    /// Degenerate interval [x, x].
    #[inline]
    pub fn point(x: f64) -> Self {
        Self::new(x, x)
    }
    /// Widen both bounds by one ulp (the outward-rounding step).
    #[inline]
    fn widened(self) -> Self {
        Self {
            lo: self.lo.next_down(),
            hi: self.hi.next_up(),
        }
    }
    #[inline]
    pub fn contains(self, x: f64) -> bool {
        self.lo <= x && x <= self.hi
    }
    #[inline]
    pub fn width(self) -> f64 {
        self.hi - self.lo
    }
    /// Certified sign: Some(1)/Some(-1) when strictly one-signed,
    /// Some(0) for the exact zero point interval, None when ambiguous.
    #[inline]
    pub fn sign(self) -> Option<i8> {
        if self.lo > 0.0 {
            Some(1)
        } else if self.hi < 0.0 {
            Some(-1)
        } else if self.lo == 0.0 && self.hi == 0.0 {
            Some(0)
        } else {
            None
        }
    }
    /// Conservative division. None unless the divisor is strictly
    /// one-signed (an interval straddling zero has unbounded quotient).
    pub fn checked_div(self, o: Self) -> Option<Self> {
        if !(o.lo > 0.0 || o.hi < 0.0) {
            return None;
        }
        let p = [
            self.lo / o.lo,
            self.lo / o.hi,
            self.hi / o.lo,
            self.hi / o.hi,
        ];
        let mut lo = p[0];
        let mut hi = p[0];
        for &v in &p[1..] {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        Some(Self { lo, hi }.widened())
    }

    /// Conservative square root; None when entirely negative.
    /// A straddling interval is clamped to [0, hi] first.
    pub fn sqrt(self) -> Option<Self> {
        if self.hi < 0.0 {
            return None;
        }
        let lo = self.lo.max(0.0).sqrt();
        let hi = self.hi.sqrt();
        Some(Self { lo, hi }.widened())
    }
}

impl Add for Interval {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        Self {
            lo: self.lo + o.lo,
            hi: self.hi + o.hi,
        }
        .widened()
    }
}

impl Sub for Interval {
    type Output = Self;
    #[inline]
    fn sub(self, o: Self) -> Self {
        Self {
            lo: self.lo - o.hi,
            hi: self.hi - o.lo,
        }
        .widened()
    }
}

impl Neg for Interval {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            lo: -self.hi,
            hi: -self.lo,
        }
    }
}

impl Mul for Interval {
    type Output = Self;
    #[inline]
    fn mul(self, o: Self) -> Self {
        let p = [
            self.lo * o.lo,
            self.lo * o.hi,
            self.hi * o.lo,
            self.hi * o.hi,
        ];
        let mut lo = p[0];
        let mut hi = p[0];
        for &v in &p[1..] {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        Self { lo, hi }.widened()
    }
}

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
    fn div_requires_one_signed_divisor() {
        let a = Interval::new(1.0, 2.0);
        assert!(a.checked_div(Interval::new(-1.0, 1.0)).is_none());
        let r = a.checked_div(Interval::new(2.0, 4.0)).unwrap();
        assert!(r.lo <= 0.25 && r.hi >= 1.0);
        // Negative divisor flips the quotient sign.
        let r = a.checked_div(Interval::new(-4.0, -2.0)).unwrap();
        assert!(r.lo <= -1.0 && r.hi >= -0.25);
    }

    #[test]
    fn sqrt_of_negative_is_none() {
        assert!(Interval::new(-2.0, -1.0).sqrt().is_none());
        // Straddling zero clamps the low bound to 0.
        let s = Interval::new(-1.0, 4.0).sqrt().unwrap();
        assert!(s.lo <= 0.0 && s.hi >= 2.0);
    }

    fn small() -> impl Strategy<Value = f64> {
        -1.0e3..1.0e3f64
    }

    proptest! {
        #[test]
        fn div_is_sound(a in small(), b in 0.5..1.0e3f64) {
            let r = Interval::point(a).checked_div(Interval::point(b)).unwrap();
            prop_assert!(r.lo <= a / b && a / b <= r.hi);
        }

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
