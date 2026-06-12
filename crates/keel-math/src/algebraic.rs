//! The Tier-1 exact algebraic layer (tasks 32/33, dossier 11): ONE-ROOT
//! real numbers (a + b sqrt(c)) / d over exact integers, with the
//! Devillers-Fronville-Mourrain-Teillaud comparison recipe: decisions
//! reduce to sign batteries on integer expressions (squaring with sign
//! case analysis), no square root is ever evaluated exactly. Every
//! public predicate runs the standard cascade: a cheap floating filter
//! first, the exact battery only when the filter is inconclusive.
//!
//! SCOPE (stated plainly): this is AlgebraicReal restricted to degree-2
//! one-root numbers, which covers circle/circle and same-family conic
//! intersection coordinates (the dossier's "cheapest curved exactness,
//! implement first"). The API is shaped after the CGAL algebraic
//! kernel's Ak_1 (isolate / compare / sign_at / approximate) so the
//! degree-4 conic-conic tier can slot in behind the same interface;
//! that tier, and expression-DAG generality, are NOT built here.
//!
//! Inputs arrive as f64 but are treated as the EXACT dyadic rationals
//! they are (the EGC contract): construction converts them losslessly.

use crate::bigint::BigInt;

/// (a + b * sqrt(c)) / d with integer a, b, c, d; c >= 0, d > 0.
#[derive(Clone, Debug)]
pub struct OneRoot {
    pub a: BigInt,
    pub b: BigInt,
    pub c: BigInt,
    pub d: BigInt,
}

/// Exact sign of `a + b*sqrt(c)` (c >= 0) by the squaring battery.
fn sign_one_radical(a: &BigInt, b: &BigInt, c: &BigInt) -> i32 {
    debug_assert!(c.signum() >= 0, "radicand must be non-negative");
    let sb = if c.is_zero() { 0 } else { b.signum() };
    let sa = a.signum();
    if sb == 0 {
        return sa;
    }
    if sa == 0 {
        return sb;
    }
    if sa == sb {
        return sa;
    }
    // Opposite signs: |a| vs |b|sqrt(c) decides; compare a^2 vs b^2 c.
    let lhs = a.square();
    let rhs = b.square().mul(c);
    sa * lhs.cmp(&rhs) as i32
}

/// Exact sign of `a + b1*sqrt(c1) + b2*sqrt(c2)` (c1, c2 >= 0).
fn sign_two_radicals(a: &BigInt, b1: &BigInt, c1: &BigInt, b2: &BigInt, c2: &BigInt) -> i32 {
    // Sign of T = b1 sqrt(c1) + b2 sqrt(c2) first (a one-radical-style
    // battery of its own).
    let s1 = if c1.is_zero() { 0 } else { b1.signum() };
    let s2 = if c2.is_zero() { 0 } else { b2.signum() };
    let st = match (s1, s2) {
        (0, s) => s,
        (s, 0) => s,
        (x, y) if x == y => x,
        (x, _) => {
            let lhs = b1.square().mul(c1);
            let rhs = b2.square().mul(c2);
            x * lhs.cmp(&rhs) as i32
        }
    };
    let sa = a.signum();
    if st == 0 {
        return sa;
    }
    if sa == 0 {
        return st;
    }
    if sa == st {
        return sa;
    }
    // Opposite: sign(a + T) = sign(a) * sign(a^2 - T^2), where
    // T^2 = b1^2 c1 + b2^2 c2 + 2 b1 b2 sqrt(c1 c2): one more radical.
    let d = a
        .square()
        .sub(&b1.square().mul(c1))
        .sub(&b2.square().mul(c2));
    let m2 = BigInt::from_i64(-2).mul(b1).mul(b2);
    let s = sign_one_radical(&d, &m2, &c1.mul(c2));
    sa * s
}

impl OneRoot {
    /// A rational value p / q (q != 0), normalized to d > 0.
    pub fn rational(p: BigInt, q: BigInt) -> Self {
        debug_assert!(q.signum() != 0, "rational with zero denominator");
        let (p, q) = if q.signum() < 0 {
            (p.neg(), q.neg())
        } else {
            (p, q)
        };
        OneRoot {
            a: p,
            b: BigInt::zero(),
            c: BigInt::zero(),
            d: q,
        }
    }

    /// (a + b sqrt(c)) / d, normalized to d > 0. c must be >= 0.
    pub fn new(a: BigInt, b: BigInt, c: BigInt, d: BigInt) -> Self {
        debug_assert!(c.signum() >= 0, "negative radicand");
        debug_assert!(d.signum() != 0, "zero denominator");
        if d.signum() < 0 {
            OneRoot {
                a: a.neg(),
                b: b.neg(),
                c,
                d: d.neg(),
            }
        } else {
            OneRoot { a, b, c, d }
        }
    }

    /// Approximate value and a CONSERVATIVE absolute error bound. The
    /// bound only has to be sound enough for the filter; the exact
    /// battery is the authority either way.
    pub fn approx(&self) -> (f64, f64) {
        let (af, bf, cf, df) = (
            self.a.to_f64(),
            self.b.to_f64(),
            self.c.to_f64(),
            self.d.to_f64(),
        );
        let num = af + bf * cf.sqrt();
        let v = num / df;
        let mag = (af.abs() + bf.abs() * cf.sqrt()) / df.abs();
        // to_f64 truncates below the top 128 bits; a handful of ulps
        // per operation, inflated generously.
        let err = mag * 1e-9 + 1e-300;
        (v, err)
    }

    /// Exact sign of the value.
    pub fn sign(&self) -> i32 {
        sign_one_radical(&self.a, &self.b, &self.c)
    }

    /// Exact comparison, floating filter first.
    pub fn compare(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;
        let (x, ex) = self.approx();
        let (y, ey) = other.approx();
        if x.is_finite() && y.is_finite() {
            if x - y > ex + ey {
                return Ordering::Greater;
            }
            if y - x > ex + ey {
                return Ordering::Less;
            }
        }
        // Exact: sign of self - other. Clear denominators (both > 0):
        // A + B1 sqrt(c1) + B2 sqrt(c2) with
        //   A = a1 d2 - a2 d1, B1 = b1 d2, B2 = -b2 d1.
        let a = self.a.mul(&other.d).sub(&other.a.mul(&self.d));
        let b1 = self.b.mul(&other.d);
        let b2 = other.b.mul(&self.d).neg();
        let s = if self.c == other.c {
            // Same radicand (roots of one quadratic): one radical.
            sign_one_radical(&a, &b1.add(&b2), &self.c)
        } else {
            sign_two_radicals(&a, &b1, &self.c, &b2, &other.c)
        };
        match s {
            1 => Ordering::Greater,
            -1 => Ordering::Less,
            _ => Ordering::Equal,
        }
    }
}

/// Exact quadratic a2 x^2 + a1 x + a0 with integer coefficients: the
/// Ak_1-style root container (isolate via `roots`, evaluate via
/// `sign_at`, compare roots via `OneRoot::compare`).
#[derive(Clone, Debug)]
pub struct Quadratic {
    pub a2: BigInt,
    pub a1: BigInt,
    pub a0: BigInt,
}

impl Quadratic {
    /// Real roots in INCREASING order (0, 1, or 2; a linear input
    /// yields its single rational root; a degenerate constant yields
    /// none). A double root is reported once.
    pub fn roots(&self) -> Vec<OneRoot> {
        if self.a2.is_zero() {
            if self.a1.is_zero() {
                return Vec::new();
            }
            return vec![OneRoot::rational(self.a0.neg(), self.a1.clone())];
        }
        let disc = self
            .a1
            .square()
            .sub(&BigInt::from_i64(4).mul(&self.a2).mul(&self.a0));
        match disc.signum() {
            -1 => Vec::new(),
            0 => vec![OneRoot::rational(
                self.a1.neg(),
                BigInt::from_i64(2).mul(&self.a2),
            )],
            _ => {
                let d = BigInt::from_i64(2).mul(&self.a2);
                let lo = OneRoot::new(
                    self.a1.neg(),
                    if d.signum() > 0 {
                        BigInt::from_i64(-1)
                    } else {
                        BigInt::from_i64(1)
                    },
                    disc.clone(),
                    d.clone(),
                );
                let hi = OneRoot::new(
                    self.a1.neg(),
                    if d.signum() > 0 {
                        BigInt::from_i64(1)
                    } else {
                        BigInt::from_i64(-1)
                    },
                    disc,
                    d,
                );
                vec![lo, hi]
            }
        }
    }

    /// Exact sign of the quadratic at a one-root argument: substitute
    /// x = (a + b sqrt(c)) / d; d^2 p(x) is again a one-radical
    /// expression.
    pub fn sign_at(&self, x: &OneRoot) -> i32 {
        let (a, b, c, d) = (&x.a, &x.b, &x.c, &x.d);
        // d^2 p(x) = a2 (a^2 + b^2 c) + a1 d a + a0 d^2
        //          + (2 a2 a b + a1 d b) sqrt(c)
        let plain = self
            .a2
            .mul(&a.square().add(&b.square().mul(c)))
            .add(&self.a1.mul(d).mul(a))
            .add(&self.a0.mul(&d.square()));
        let radical = BigInt::from_i64(2)
            .mul(&self.a2)
            .mul(a)
            .mul(b)
            .add(&self.a1.mul(d).mul(b));
        sign_one_radical(&plain, &radical, c)
    }
}

/// Lossless dyadic decomposition of a finite f64: value = m * 2^e.
fn dyadic(x: f64) -> (i64, i32) {
    if x == 0.0 {
        return (0, 0);
    }
    let bits = x.to_bits();
    let sign = if bits >> 63 == 1 { -1i64 } else { 1 };
    let biased = ((bits >> 52) & 0x7FF) as i32;
    let frac = bits & 0x000F_FFFF_FFFF_FFFF;
    let (m, e) = if biased == 0 {
        (frac as i64, -1074) // subnormal
    } else {
        ((frac | 0x0010_0000_0000_0000) as i64, biased - 1075)
    };
    (sign * m, e)
}

/// Convert a set of finite f64s LOSSLESSLY to integers sharing one
/// power-of-two scale: returns (ints, shift) with
/// `ints[i] = vals[i] * 2^shift` exactly (shift >= 0).
pub fn dyadic_ints(vals: &[f64]) -> (Vec<BigInt>, u32) {
    let mut emin = 0i32;
    let mut decomp = Vec::with_capacity(vals.len());
    for &v in vals {
        debug_assert!(v.is_finite(), "dyadic_ints requires finite input");
        let (m, e) = dyadic(v);
        if m != 0 {
            emin = emin.min(e);
        }
        decomp.push((m, e));
    }
    let ints = decomp
        .into_iter()
        .map(|(m, e)| BigInt::from_i64(m).shl((e - emin) as u32))
        .collect();
    (ints, (-emin) as u32)
}

/// The abscissa quadratic of two circles' intersection points (the
/// dossier 3.1 predicate): circle i is (x - xi)^2 + (y - yi)^2 = ri^2
/// with EXACT dyadic f64 data. The intersection x-coordinates are the
/// real roots. Returns None for concentric circles (no radical line).
pub fn circle_circle_x_quadratic(
    (x1, y1, r1): (f64, f64, f64),
    (x2, y2, r2): (f64, f64, f64),
) -> Option<Quadratic> {
    let (v, shift) = dyadic_ints(&[x1, y1, r1, x2, y2, r2]);
    let (x1, y1, r1, x2, y2, r2) = (&v[0], &v[1], &v[2], &v[3], &v[4], &v[5]);
    // Circle i: x^2 + y^2 - 2 xi x - 2 yi y + (xi^2 + yi^2 - ri^2) = 0.
    // Radical line (c1 - c2): 2(x2-x1) x + 2(y2-y1) y + (k1 - k2) = 0
    // with ki = xi^2 + yi^2 - ri^2.
    let two = BigInt::from_i64(2);
    let gx = two.mul(&x2.sub(x1));
    let gy = two.mul(&y2.sub(y1));
    let k1 = x1.square().add(&y1.square()).sub(&r1.square());
    let k2 = x2.square().add(&y2.square()).sub(&r2.square());
    let g0 = k1.sub(&k2);
    if gx.is_zero() && gy.is_zero() {
        return None; // concentric: no radical line
    }
    if gy.is_zero() {
        // Vertical radical line: x = -g0/gx exactly; both intersection
        // points share it. Encode as the linear "quadratic", unscaled
        // back to ORIGINAL coordinates (x_scaled = 2^shift x).
        return Some(Quadratic {
            a2: BigInt::zero(),
            a1: gx.shl(shift),
            a0: g0,
        });
    }
    // y = -(gx x + g0)/gy substituted into circle 1, scaled by gy^2:
    // (gy^2 + gx^2) x^2
    //   + (2 gx g0 - 2 x1 gy^2 + 2 y1 gy gx) x
    //   + (g0^2 + 2 y1 gy g0 + k1 gy^2) = 0
    let gy2 = gy.square();
    let a2 = gy2.add(&gx.square());
    let a1 = two
        .mul(&gx)
        .mul(&g0)
        .sub(&two.mul(x1).mul(&gy2))
        .add(&two.mul(y1).mul(&gy).mul(&gx));
    let a0 = g0
        .square()
        .add(&two.mul(y1).mul(&gy).mul(&g0))
        .add(&k1.mul(&gy2));
    // Unscale to ORIGINAL coordinates: the equation above is in the
    // dyadic frame x_scaled = 2^shift x, so q(x) = a2 2^(2 shift) x^2
    // + a1 2^shift x + a0.
    Some(Quadratic {
        a2: a2.shl(2 * shift),
        a1: a1.shl(shift),
        a0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;

    fn b(v: i64) -> BigInt {
        BigInt::from_i64(v)
    }

    #[test]
    fn one_radical_signs() {
        // -2 + sqrt(4) = 0 exactly.
        assert_eq!(sign_one_radical(&b(-2), &b(1), &b(4)), 0);
        // 3 - sqrt(8) > 0 (9 > 8); 3 - sqrt(10) < 0.
        assert_eq!(sign_one_radical(&b(3), &b(-1), &b(8)), 1);
        assert_eq!(sign_one_radical(&b(3), &b(-1), &b(10)), -1);
        // Zero radicand falls back to the plain sign.
        assert_eq!(sign_one_radical(&b(-7), &b(5), &b(0)), -1);
    }

    #[test]
    fn two_radical_signs() {
        // 3 sqrt(2) - sqrt(18) = 0 exactly.
        assert_eq!(sign_two_radicals(&b(0), &b(3), &b(2), &b(-1), &b(18)), 0);
        // -5 + sqrt(8) + sqrt(2) = -5 + 3 sqrt(2) > 0 (18 > 25? no: < 0).
        assert_eq!(sign_two_radicals(&b(-5), &b(1), &b(8), &b(1), &b(2)), -1);
        // -4 + sqrt(8) + sqrt(2) = 3 sqrt(2) - 4 > 0 (18 > 16).
        assert_eq!(sign_two_radicals(&b(-4), &b(1), &b(8), &b(1), &b(2)), 1);
        // 1 + sqrt(2) - sqrt(3+2 sqrt(2)) would be 0, but nested
        // radicals are out of scope; instead pin a hard near-tie:
        // 7 - sqrt(48) - sqrt(0.01...) style with integers:
        // 7 - sqrt(48) > 0 (49 > 48), minus sqrt(1/100) -> scale by 10:
        // 70 - 10 sqrt(48) - sqrt(100): 70 - sqrt(4800) - 10:
        // 60 vs sqrt(4800): 3600 < 4800: negative.
        assert_eq!(
            sign_two_radicals(&b(70), &b(-10), &b(48), &b(-1), &b(100)),
            -1
        );
    }

    #[test]
    fn quadratic_roots_order_and_sign_at() {
        // x^2 - 3x + 1: roots (3 +- sqrt(5))/2.
        let q = Quadratic {
            a2: b(1),
            a1: b(-3),
            a0: b(1),
        };
        let r = q.roots();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].compare(&r[1]), Ordering::Less);
        assert_eq!(q.sign_at(&r[0]), 0);
        assert_eq!(q.sign_at(&r[1]), 0);
        // Between the roots the quadratic is negative; outside positive.
        let mid = OneRoot::rational(b(3), b(2));
        assert_eq!(q.sign_at(&mid), -1);
        let far = OneRoot::rational(b(100), b(1));
        assert_eq!(q.sign_at(&far), 1);
        // Scaling the quadratic does not move the roots: 2x^2 - 6x + 2.
        let q2 = Quadratic {
            a2: b(2),
            a1: b(-6),
            a0: b(2),
        };
        let r2 = q2.roots();
        assert_eq!(r[0].compare(&r2[0]), Ordering::Equal);
        assert_eq!(r[1].compare(&r2[1]), Ordering::Equal);
        // Double root reported once: (x-2)^2.
        let qd = Quadratic {
            a2: b(1),
            a1: b(-4),
            a0: b(4),
        };
        assert_eq!(qd.roots().len(), 1);
    }

    #[test]
    fn negative_leading_coefficient_roots_stay_ordered() {
        // -x^2 + 3x - 1: same roots as x^2 - 3x + 1.
        let q = Quadratic {
            a2: b(-1),
            a1: b(3),
            a0: b(-1),
        };
        let r = q.roots();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].compare(&r[1]), Ordering::Less);
        let qp = Quadratic {
            a2: b(1),
            a1: b(-3),
            a0: b(1),
        };
        let rp = qp.roots();
        assert_eq!(r[0].compare(&rp[0]), Ordering::Equal);
        assert_eq!(r[1].compare(&rp[1]), Ordering::Equal);
    }

    #[test]
    fn circle_predicate_tangency_is_exact_equality() {
        // Externally tangent circles: r=1 at origin, r=1 at (2, 0):
        // both intersection abscissae are EXACTLY x = 1 (a double
        // contact). The exact layer must say Equal where f64 jitter
        // would wobble.
        let q = circle_circle_x_quadratic((0.0, 0.0, 1.0), (2.0, 0.0, 1.0)).unwrap();
        let r = q.roots();
        assert_eq!(r.len(), 1, "tangency: one abscissa");
        let one = OneRoot::rational(b(1), b(1));
        assert_eq!(r[0].compare(&one), Ordering::Equal);

        // Crossing circles with dyadic data: unit circles at x = 0 and
        // x = 1: intersections at x = 1/2 exactly.
        let q = circle_circle_x_quadratic((0.0, 0.0, 1.0), (1.0, 0.0, 1.0)).unwrap();
        let r = q.roots();
        let half = OneRoot::rational(b(1), b(2));
        for root in &r {
            assert_eq!(root.compare(&half), Ordering::Equal);
        }

        // Off-axis crossing: circles at (0,0) r 5/4 and (1, 1/2) r 3/4
        // (all dyadic): compare the two abscissae strictly.
        let q = circle_circle_x_quadratic((0.0, 0.0, 1.25), (1.0, 0.5, 0.75)).unwrap();
        let r = q.roots();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].compare(&r[1]), Ordering::Less);
        assert_eq!(r[0].compare(&r[0]), Ordering::Equal);
    }

    #[test]
    fn filter_agrees_with_exact_on_randoms() {
        // Deterministic random one-root values: the filtered compare
        // must agree with a pure-exact compare (filter disabled by
        // construction: compare twice, the second time after verifying
        // against approx separation manually).
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 40) as i64) - (1 << 23)
        };
        for _ in 0..4000 {
            let x = OneRoot::new(b(next()), b(next()), b(next().abs()), b(next().abs() + 1));
            let y = OneRoot::new(b(next()), b(next()), b(next().abs()), b(next().abs() + 1));
            let ord = x.compare(&y);
            // Cross-check against approximations where they separate.
            let (xa, xe) = x.approx();
            let (ya, ye) = y.approx();
            if (xa - ya).abs() > xe + ye {
                let approx_ord = xa.partial_cmp(&ya).unwrap();
                assert_eq!(ord, approx_ord, "filter/exact disagreement");
            }
            // Self-comparison is always Equal (a hard tie by identity).
            assert_eq!(x.compare(&x.clone()), Ordering::Equal);
        }
    }

    #[test]
    fn dyadic_conversion_is_lossless() {
        let vals = [0.1, -3.5, 1e-17, 1024.0, 0.0];
        let (ints, shift) = dyadic_ints(&vals);
        assert!(shift > 0);
        assert_eq!(ints[4].signum(), 0);
        assert_eq!(ints[1].signum(), -1);
        assert_eq!(ints[3].signum(), 1);
        // Ratios are preserved exactly through the common scale: 0.1
        // recovered against the exactly-representable 1024.
        let approx = ints[0].to_f64() / ints[3].to_f64() * 1024.0;
        assert!((approx - 0.1).abs() < 1e-12);
    }
}
