//! Conservative interval arithmetic via one-ulp outward widening.
//!
//! Rust gives no portable access to FPU rounding modes, so operations
//! compute in round-to-nearest and then widen each bound by one ulp
//! (`next_down`/`next_up`). Slightly over-wide, always sound: the true
//! real result of an operation on members of the operand intervals is
//! contained in the result interval.
//!
//! BOUNDS ARE EXTENDED REALS (M5a Task 0 soundness audit): an
//! operation whose true range is unbounded yields an infinite bound
//! rather than silently violating a finiteness invariant (the
//! pre-audit implementation overflowed near-MAX products to inf and
//! broke its own contract in release builds). Invariants: lo <= hi,
//! never NaN, and the interval is never the empty point at infinity
//! (lo == hi == +-inf is forbidden). NaN production (inf - inf,
//! 0 * inf) is prevented by case analysis: 0 * anything := 0 for
//! corner candidates (exact: the member is literally zero), and
//! indeterminate division corners widen conservatively to +-inf.

use core::ops::{Add, Mul, Neg, Sub};

/// Closed interval [lo, hi] over the extended reals; lo <= hi; no NaN;
/// not a point at infinity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    /// New interval; debug_asserts the invariants.
    #[inline]
    pub fn new(lo: f64, hi: f64) -> Self {
        debug_assert!(!lo.is_nan() && !hi.is_nan() && lo <= hi);
        debug_assert!(!(lo == hi && lo.is_infinite()));
        Self { lo, hi }
    }

    /// Degenerate interval [x, x] (x finite).
    #[inline]
    pub fn point(x: f64) -> Self {
        debug_assert!(x.is_finite());
        Self::new(x, x)
    }

    /// The whole extended real line.
    #[inline]
    pub fn everything() -> Self {
        Self {
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
        }
    }

    /// Widen both bounds by one ulp (the outward-rounding step).
    /// Infinite bounds stay infinite (next_down(-inf) == -inf).
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
    pub fn is_finite(self) -> bool {
        self.lo.is_finite() && self.hi.is_finite()
    }

    /// Width; infinite when either bound is infinite.
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
    /// one-signed. Indeterminate corners (inf/inf) widen to the
    /// appropriate infinity: over-wide, never unsound.
    pub fn checked_div(self, o: Self) -> Option<Self> {
        if !(o.lo > 0.0 || o.hi < 0.0) {
            return None;
        }
        let cands = [
            div_corner(self.lo, o.lo),
            div_corner(self.lo, o.hi),
            div_corner(self.hi, o.lo),
            div_corner(self.hi, o.hi),
        ];
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &(clo, chi) in &cands {
            lo = lo.min(clo);
            hi = hi.max(chi);
        }
        Some(Self { lo, hi }.widened().depoint())
    }

    /// Conservative square root; None when entirely negative.
    /// A straddling interval is clamped to [0, hi] first.
    pub fn sqrt(self) -> Option<Self> {
        if self.hi < 0.0 {
            return None;
        }
        let lo = self.lo.max(0.0).sqrt();
        let hi = self.hi.sqrt();
        Some(Self { lo, hi }.widened().depoint())
    }

    /// Collapse any accidental point-at-infinity to a one-ulp band
    /// next to it (cannot occur via public ops; defensive).
    #[inline]
    fn depoint(self) -> Self {
        if self.lo == self.hi && self.lo.is_infinite() {
            if self.lo > 0.0 {
                Self {
                    lo: f64::MAX,
                    hi: f64::INFINITY,
                }
            } else {
                Self {
                    lo: f64::NEG_INFINITY,
                    hi: f64::MIN,
                }
            }
        } else {
            self
        }
    }
}

/// A division corner as a (lo, hi) contribution: finite quotients
/// contribute themselves; the indeterminate inf/inf corner widens to
/// the full line (sound; only reachable with an unbounded numerator).
#[inline]
fn div_corner(a: f64, b: f64) -> (f64, f64) {
    let q = a / b;
    if q.is_nan() {
        (f64::NEG_INFINITY, f64::INFINITY)
    } else {
        (q, q)
    }
}

/// A multiplication corner: zero times anything is exactly zero (the
/// member IS zero), preventing 0 * inf = NaN.
#[inline]
fn mul_corner(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 { 0.0 } else { a * b }
}

impl Add for Interval {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        // lo bounds are never +inf and hi bounds never -inf (the
        // point-at-infinity is forbidden), so inf - inf cannot arise.
        Self {
            lo: self.lo + o.lo,
            hi: self.hi + o.hi,
        }
        .widened()
        .depoint()
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
        .depoint()
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
            mul_corner(self.lo, o.lo),
            mul_corner(self.lo, o.hi),
            mul_corner(self.hi, o.lo),
            mul_corner(self.hi, o.hi),
        ];
        let mut lo = p[0];
        let mut hi = p[0];
        for &v in &p[1..] {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        Self { lo, hi }.widened().depoint()
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
    fn overflow_yields_unbounded_interval_not_lies() {
        // The pre-audit soundness hole: near-MAX products must yield
        // an interval honestly unbounded above, never a broken one.
        let big = Interval::new(1e308, 1.5e308);
        let r = big * big;
        assert!(r.hi.is_infinite() && r.hi > 0.0);
        assert!(!r.lo.is_nan() && r.lo <= f64::MAX);
        let s = big + big;
        assert!(s.hi.is_infinite());
        // Subtraction of unbounded intervals must not manufacture NaN.
        let t = r - s;
        assert!(!t.lo.is_nan() && !t.hi.is_nan());
        assert!(t.lo <= t.hi);
    }

    #[test]
    fn zero_times_unbounded_is_zero() {
        let zero = Interval::point(0.0);
        let unb = Interval::new(2.0, f64::INFINITY);
        let r = zero * unb;
        assert!(!r.lo.is_nan() && !r.hi.is_nan());
        assert!(r.contains(0.0));
        assert!(r.lo >= -1e-300 && r.hi <= 1e-300, "{r:?}");
    }

    #[test]
    fn div_with_unbounded_operands_is_sound_not_nan() {
        let num = Interval::new(1.0, f64::INFINITY);
        let den = Interval::new(2.0, f64::INFINITY);
        let r = num.checked_div(den).unwrap();
        assert!(!r.lo.is_nan() && !r.hi.is_nan());
        // True set is (0, inf); enclosure must cover representatives.
        assert!(r.contains(0.5) && r.contains(1e10));
    }

    #[test]
    fn div_requires_one_signed_divisor() {
        let a = Interval::new(1.0, 2.0);
        assert!(a.checked_div(Interval::new(-1.0, 1.0)).is_none());
        let r = a.checked_div(Interval::new(2.0, 4.0)).unwrap();
        assert!(r.lo <= 0.25 && r.hi >= 1.0);
        let r = a.checked_div(Interval::new(-4.0, -2.0)).unwrap();
        assert!(r.lo <= -1.0 && r.hi >= -0.25);
    }

    #[test]
    fn sqrt_of_negative_is_none() {
        assert!(Interval::new(-2.0, -1.0).sqrt().is_none());
        let s = Interval::new(-1.0, 4.0).sqrt().unwrap();
        assert!(s.lo <= 0.0 && s.hi >= 2.0);
    }

    /// Magnitude ladder covering every f64 regime the fuzz campaigns
    /// have taught us to fear.
    fn ladder() -> impl Strategy<Value = f64> {
        prop_oneof![
            -1.0e3..1.0e3f64,
            (-1.0e308..1.0e308f64),
            (-1.0e-300..1.0e-300f64),
            Just(0.0),
            Just(-0.0),
            Just(f64::MIN_POSITIVE),
            Just(-f64::MIN_POSITIVE),
            Just(5e-324),
            Just(f64::MAX),
            Just(f64::MIN),
        ]
    }

    /// Soundness oracle: the true real result x of an op on exact f64
    /// inputs satisfies |x - fl(x)| <= ulp/2 for the correctly rounded
    /// fl(x), so x is guaranteed inside [next_down(fl), next_up(fl)].
    /// The interval result must contain that band.
    fn band_contained(r: Interval, fl: f64) -> bool {
        if fl.is_nan() {
            return true; // pointwise op left the f64 domain; skip
        }
        r.lo <= fl.next_down() && fl.next_up() <= r.hi
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]
        #[test]
        fn add_sound_on_ladder(a in ladder(), b in ladder()) {
            let r = Interval::point(a) + Interval::point(b);
            prop_assert!(band_contained(r, a + b));
            prop_assert!(!r.lo.is_nan() && !r.hi.is_nan() && r.lo <= r.hi);
        }
        #[test]
        fn sub_sound_on_ladder(a in ladder(), b in ladder()) {
            let r = Interval::point(a) - Interval::point(b);
            prop_assert!(band_contained(r, a - b));
            prop_assert!(!r.lo.is_nan() && !r.hi.is_nan() && r.lo <= r.hi);
        }
        #[test]
        fn mul_sound_on_ladder(a in ladder(), b in ladder()) {
            let r = Interval::point(a) * Interval::point(b);
            prop_assert!(band_contained(r, a * b));
            prop_assert!(!r.lo.is_nan() && !r.hi.is_nan() && r.lo <= r.hi);
        }
        #[test]
        fn div_sound_on_ladder(a in ladder(), b in ladder()) {
            prop_assume!(b != 0.0);
            let denom = Interval::point(b);
            if let Some(r) = Interval::point(a).checked_div(denom) {
                prop_assert!(band_contained(r, a / b));
                prop_assert!(!r.lo.is_nan() && !r.hi.is_nan() && r.lo <= r.hi);
            }
        }
        #[test]
        fn sqrt_sound_on_ladder(a in ladder()) {
            prop_assume!(a >= 0.0);
            if let Some(r) = Interval::point(a).sqrt() {
                prop_assert!(band_contained(r, a.sqrt()));
            }
        }

        // Chained-op soundness: random expression programs evaluated
        // as intervals alongside the pointwise f64 member.
        #[test]
        fn chained_ops_keep_members_enclosed(
            seeds in proptest::collection::vec(ladder(), 2..6),
            ops in proptest::collection::vec(0u8..4, 1..8),
        ) {
            let mut vals: Vec<f64> = seeds.clone();
            let mut ivs: Vec<Interval> = seeds.iter().map(|&x| Interval::point(x)).collect();
            for (k, op) in ops.iter().enumerate() {
                let i = k % vals.len();
                let j = (k + 1) % vals.len();
                let (v, iv) = match op {
                    0 => (vals[i] + vals[j], ivs[i] + ivs[j]),
                    1 => (vals[i] - vals[j], ivs[i] - ivs[j]),
                    2 => (vals[i] * vals[j], ivs[i] * ivs[j]),
                    _ => (-vals[i], -ivs[i]),
                };
                if v.is_nan() {
                    break;
                }
                prop_assert!(!iv.lo.is_nan() && !iv.hi.is_nan() && iv.lo <= iv.hi);
                if v.is_finite() {
                    prop_assert!(iv.contains(v), "{v} not in {iv:?}");
                }
                vals[i] = if v.is_finite() { v } else { vals[i] };
                ivs[i] = if v.is_finite() { iv } else { ivs[i] };
            }
        }

        #[test]
        fn mul_sound_for_wide_operands(
            a in -1.0e3..1.0e3f64, b in -1.0e3..1.0e3f64,
            c in -1.0e3..1.0e3f64, d in -1.0e3..1.0e3f64,
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
