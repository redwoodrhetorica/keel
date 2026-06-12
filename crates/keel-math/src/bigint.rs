//! Minimal arbitrary-precision signed integer for the exact predicate
//! layer (task 32). Only what sign batteries need: add, sub, mul,
//! compare, sign. No division anywhere (one-root comparisons clear
//! denominators by cross-multiplication), no parsing, no display
//! beyond Debug. Little-endian u64 limbs, no trailing zero limbs,
//! canonical zero = empty limbs with neg = false.

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BigInt {
    neg: bool,
    mag: Vec<u64>,
}

impl BigInt {
    pub fn zero() -> Self {
        BigInt {
            neg: false,
            mag: Vec::new(),
        }
    }

    pub fn from_i64(v: i64) -> Self {
        if v == 0 {
            return Self::zero();
        }
        let neg = v < 0;
        let m = v.unsigned_abs();
        BigInt { neg, mag: vec![m] }
    }

    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    /// -1, 0, +1.
    pub fn signum(&self) -> i32 {
        if self.is_zero() {
            0
        } else if self.neg {
            -1
        } else {
            1
        }
    }

    pub fn neg(&self) -> Self {
        if self.is_zero() {
            Self::zero()
        } else {
            BigInt {
                neg: !self.neg,
                mag: self.mag.clone(),
            }
        }
    }

    fn trim(mut mag: Vec<u64>) -> Vec<u64> {
        while mag.last() == Some(&0) {
            mag.pop();
        }
        mag
    }

    fn cmp_mag(a: &[u64], b: &[u64]) -> core::cmp::Ordering {
        use core::cmp::Ordering;
        if a.len() != b.len() {
            return a.len().cmp(&b.len());
        }
        for i in (0..a.len()).rev() {
            match a[i].cmp(&b[i]) {
                Ordering::Equal => {}
                o => return o,
            }
        }
        Ordering::Equal
    }

    fn add_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
        let n = a.len().max(b.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry = 0u64;
        for i in 0..n {
            let x = *a.get(i).unwrap_or(&0) as u128;
            let y = *b.get(i).unwrap_or(&0) as u128;
            let s = x + y + carry as u128;
            out.push(s as u64);
            carry = (s >> 64) as u64;
        }
        if carry != 0 {
            out.push(carry);
        }
        out
    }

    /// a - b, REQUIRES |a| >= |b|.
    fn sub_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
        let mut out = Vec::with_capacity(a.len());
        let mut borrow = 0i128;
        for (i, &xa) in a.iter().enumerate() {
            let x = xa as i128;
            let y = *b.get(i).unwrap_or(&0) as i128;
            let mut d = x - y - borrow;
            if d < 0 {
                d += 1i128 << 64;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(d as u64);
        }
        debug_assert_eq!(borrow, 0, "sub_mag requires |a| >= |b|");
        Self::trim(out)
    }

    pub fn add(&self, other: &Self) -> Self {
        use core::cmp::Ordering;
        if self.is_zero() {
            return other.clone();
        }
        if other.is_zero() {
            return self.clone();
        }
        if self.neg == other.neg {
            return BigInt {
                neg: self.neg,
                mag: Self::add_mag(&self.mag, &other.mag),
            };
        }
        match Self::cmp_mag(&self.mag, &other.mag) {
            Ordering::Equal => Self::zero(),
            Ordering::Greater => BigInt {
                neg: self.neg,
                mag: Self::sub_mag(&self.mag, &other.mag),
            },
            Ordering::Less => BigInt {
                neg: other.neg,
                mag: Self::sub_mag(&other.mag, &self.mag),
            },
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut out = vec![0u64; self.mag.len() + other.mag.len()];
        for (i, &x) in self.mag.iter().enumerate() {
            let mut carry = 0u128;
            for (j, &y) in other.mag.iter().enumerate() {
                let cur = out[i + j] as u128 + (x as u128) * (y as u128) + carry;
                out[i + j] = cur as u64;
                carry = cur >> 64;
            }
            let mut k = i + other.mag.len();
            while carry != 0 {
                let cur = out[k] as u128 + carry;
                out[k] = cur as u64;
                carry = cur >> 64;
                k += 1;
            }
        }
        BigInt {
            neg: self.neg != other.neg,
            mag: Self::trim(out),
        }
    }

    pub fn square(&self) -> Self {
        self.mul(self)
    }

    pub fn shl(&self, bits: u32) -> Self {
        if self.is_zero() || bits == 0 {
            return self.clone();
        }
        let limbs = (bits / 64) as usize;
        let rem = bits % 64;
        let mut mag = vec![0u64; limbs];
        if rem == 0 {
            mag.extend_from_slice(&self.mag);
        } else {
            let mut carry = 0u64;
            for &x in &self.mag {
                mag.push((x << rem) | carry);
                carry = x >> (64 - rem);
            }
            if carry != 0 {
                mag.push(carry);
            }
        }
        BigInt {
            neg: self.neg,
            mag: Self::trim(mag),
        }
    }

    /// Approximate f64 value (top 128 bits of the magnitude, scaled).
    /// Saturates to +-inf far beyond f64 range; the filter treats any
    /// non-finite as inconclusive.
    pub fn to_f64(&self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        let n = self.mag.len();
        let top = self.mag[n - 1] as f64;
        let next = if n >= 2 { self.mag[n - 2] as f64 } else { 0.0 };
        let v = (top + next / 18446744073709551616.0) * 18446744073709551616f64.powi(n as i32 - 1);
        if self.neg { -v } else { v }
    }
}

impl Ord for BigInt {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;
        match (self.signum(), other.signum()) {
            (a, b) if a != b => a.cmp(&b),
            (0, _) => Ordering::Equal,
            (s, _) => {
                let m = Self::cmp_mag(&self.mag, &other.mag);
                if s > 0 { m } else { m.reverse() }
            }
        }
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(v: i128) -> BigInt {
        // Build via two i64 halves to exercise carries.
        let lo = (v.unsigned_abs() & 0xFFFF_FFFF_FFFF_FFFF) as u64;
        let hi = (v.unsigned_abs() >> 64) as u64;
        let mag = BigInt::trim(vec![lo, hi]);
        if v == 0 {
            BigInt::zero()
        } else {
            BigInt { neg: v < 0, mag }
        }
    }

    #[test]
    fn arithmetic_matches_i128() {
        // Deterministic pseudo-random small operands whose products
        // stay inside i128.
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 16) as i64 % 1_000_000_007) as i128 - 500_000_000
        };
        for _ in 0..2000 {
            let (x, y) = (next(), next());
            assert_eq!(big(x).add(&big(y)), big(x + y), "{x}+{y}");
            assert_eq!(big(x).sub(&big(y)), big(x - y), "{x}-{y}");
            assert_eq!(big(x).mul(&big(y)), big(x * y), "{x}*{y}");
            assert_eq!(big(x).cmp(&big(y)), (x).cmp(&y), "{x} cmp {y}");
        }
    }

    #[test]
    fn multilimb_carry_chain() {
        // (2^64 - 1)^2 = 2^128 - 2^65 + 1 crosses limbs both ways
        // (the value itself exceeds i128: construct limbs directly).
        let m = big(u64::MAX as i128);
        let sq = m.square();
        let expect = BigInt {
            neg: false,
            mag: vec![1, u64::MAX - 1],
        };
        assert_eq!(sq, expect);
        assert_eq!(
            m.shl(64),
            BigInt {
                neg: false,
                mag: vec![0, u64::MAX],
            }
        );
        // Triple-limb products compare correctly.
        let a = m.mul(&m).mul(&m);
        let b = m.mul(&m).mul(&m).add(&big(1));
        assert_eq!(a.cmp(&b), core::cmp::Ordering::Less);
    }

    #[test]
    fn to_f64_tracks_magnitude() {
        let x = big(1_000_000_007).mul(&big(998_244_353));
        let approx = x.to_f64();
        let exact = 1_000_000_007f64 * 998_244_353f64;
        assert!((approx - exact).abs() <= 1e-6 * exact);
    }
}
