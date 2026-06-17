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
        if coeffs.is_empty() {
            None
        } else {
            Some(Self { coeffs })
        }
    }

    pub fn degree(&self) -> usize {
        self.coeffs.len() - 1
    }

    /// Bernstein coefficients (control values), low index first.
    pub fn coeffs(&self) -> &[f64] {
        &self.coeffs
    }

    /// Convert from power basis (`power[i]` multiplies x^i), for tests
    /// and for analytic-geometry callers that derive power-form
    /// polynomials. b_i = sum_{j<=i} C(i,j)/C(n,j) * power[j].
    pub fn from_power(power: &[f64]) -> Option<Self> {
        if power.is_empty() {
            return None;
        }
        let n = power.len() - 1;
        let mut coeffs = vec![0.0; n + 1];
        for (i, c) in coeffs.iter_mut().enumerate() {
            let mut acc = 0.0;
            // ratio = C(i,j) / C(n,j), built incrementally over j.
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

    /// Exact Bernstein product (Farouki-Rajan 1988): degree p + q,
    /// c_k = sum_i C(p,i) C(q,k-i) / C(p+q,k) * a_i * b_{k-i}.
    /// Stays in the well-conditioned basis; never converts to power.
    pub fn mul(&self, other: &Bernstein) -> Bernstein {
        let p = self.degree();
        let q = other.degree();
        let n = p + q;
        let binom = |n: usize, k: usize| -> f64 {
            let mut b = 1.0f64;
            for i in 1..=k {
                b = b * (n - i + 1) as f64 / i as f64;
            }
            b
        };
        let mut coeffs = vec![0.0; n + 1];
        for (k, c) in coeffs.iter_mut().enumerate() {
            let lo = k.saturating_sub(q);
            let hi = k.min(p);
            let mut acc = 0.0;
            for i in lo..=hi {
                acc += binom(p, i) * binom(q, k - i) * self.coeffs[i] * other.coeffs[k - i];
            }
            *c = acc / binom(n, k);
        }
        Bernstein { coeffs }
    }

    /// Sum after degree-raising both operands to the max degree.
    pub fn add(&self, other: &Bernstein) -> Bernstein {
        let n = self.degree().max(other.degree());
        let a = self.elevated_to(n);
        let b = other.elevated_to(n);
        Bernstein {
            coeffs: a.coeffs.iter().zip(&b.coeffs).map(|(x, y)| x + y).collect(),
        }
    }

    /// Scalar multiple.
    pub fn scale(&self, s: f64) -> Bernstein {
        Bernstein {
            coeffs: self.coeffs.iter().map(|c| c * s).collect(),
        }
    }

    /// Exact degree elevation to degree n >= self.degree().
    pub fn elevated_to(&self, n: usize) -> Bernstein {
        let p = self.degree();
        if n == p {
            return self.clone();
        }
        debug_assert!(n > p);
        let r = n - p;
        let binom = |n: usize, k: usize| -> f64 {
            let mut b = 1.0f64;
            for i in 1..=k {
                b = b * (n - i + 1) as f64 / i as f64;
            }
            b
        };
        let mut coeffs = vec![0.0; n + 1];
        for (k, c) in coeffs.iter_mut().enumerate() {
            let lo = k.saturating_sub(r);
            let hi = k.min(p);
            let mut acc = 0.0;
            for i in lo..=hi {
                acc += binom(p, i) * binom(r, k - i) * self.coeffs[i];
            }
            *c = acc / binom(n, k);
        }
        Bernstein { coeffs }
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
            if c == 0.0 {
                continue;
            }
            if prev != 0.0 && (c > 0.0) != (prev > 0.0) {
                count += 1;
            }
            prev = c;
        }
        count
    }

    /// All roots in [0,1], ascending. Subdivide until each piece has
    /// zero variations (discard, modulo endpoint zeros) or one
    /// variation with an endpoint sign change (bracketed Newton
    /// polish); pieces narrower than `t_tol` yield their midpoint
    /// (root clusters).
    pub fn roots(&self, t_tol: f64) -> Vec<f64> {
        let mut out = Vec::new();
        let deriv = self.derivative();
        self.roots_rec(0.0, 1.0, &deriv, t_tol, &mut out);
        out.sort_by(f64::total_cmp);
        out.dedup_by(|a, b| (*a - *b).abs() <= t_tol.max(1e-12));
        out
    }

    fn roots_rec(&self, t0: f64, t1: f64, deriv: &Self, t_tol: f64, out: &mut Vec<f64>) {
        let last = *self.coeffs.last().unwrap_or(&0.0);
        match self.sign_variations() {
            0 => {
                // No interior roots; endpoint coefficients ARE values.
                if self.coeffs[0] == 0.0 {
                    out.push(t0);
                }
                if last == 0.0 {
                    out.push(t1);
                }
            }
            1 if (self.coeffs[0] > 0.0) != (last > 0.0) && self.coeffs[0] != 0.0 && last != 0.0 => {
                // Exactly one bracketed root: polish in GLOBAL
                // coordinates. self.eval at local s equals the
                // original polynomial at global t, and deriv is the
                // original's derivative at global t, so the (value,
                // derivative) pair is exactly Newton-consistent.
                let f = |t: f64| {
                    let s = (t - t0) / (t1 - t0);
                    (self.eval(s), deriv.eval(t))
                };
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
    fn product_and_sum_match_power_arithmetic() {
        // (1 + 2x)(3 - x) = 3 + 5x - 2x^2.
        let a = Bernstein::from_power(&[1.0, 2.0]).unwrap();
        let b = Bernstein::from_power(&[3.0, -1.0]).unwrap();
        let prod = a.mul(&b);
        for &t in &[0.0, 0.3, 0.7, 1.0] {
            let want = 3.0 + 5.0 * t - 2.0 * t * t;
            assert!((prod.eval(t) - want).abs() < 1e-14);
        }
        let sum = a.add(&b.scale(2.0));
        for &t in &[0.0, 0.5, 1.0] {
            let want = (1.0 + 2.0 * t) + 2.0 * (3.0 - t);
            assert!((sum.eval(t) - want).abs() < 1e-14);
        }
    }

    #[test]
    fn elevation_preserves_values() {
        let p = Bernstein::from_power(&[1.0, -2.0, 0.5]).unwrap();
        let e = p.elevated_to(5);
        assert_eq!(e.degree(), 5);
        for &t in &[0.0, 0.25, 0.6, 1.0] {
            assert!((e.eval(t) - p.eval(t)).abs() < 1e-14);
        }
    }

    #[test]
    fn roots_of_known_cubic() {
        // p(x) = (x - 0.2)(x - 0.5)(x - 0.9), roots all inside (0,1).
        let p = Bernstein::from_power(&[-0.09, 0.73, -1.6, 1.0]).unwrap();
        let roots = p.roots(1e-12);
        assert_eq!(roots.len(), 3, "{roots:?}");
        for (got, want) in roots.iter().zip([0.2, 0.5, 0.9]) {
            assert!((got - want).abs() < 1e-9, "got {roots:?}");
        }
    }

    #[test]
    fn no_roots_when_strictly_positive() {
        let p = Bernstein::new(vec![1.0, 2.0, 0.5]).unwrap();
        assert!(p.roots(1e-12).is_empty());
    }

    #[test]
    fn endpoint_root_found() {
        // p(x) = x(x - 0.5): roots at 0.0 and 0.5.
        let p = Bernstein::from_power(&[0.0, -0.5, 1.0]).unwrap();
        let roots = p.roots(1e-12);
        assert_eq!(roots.len(), 2, "{roots:?}");
        assert!(roots[0].abs() < 1e-9, "{roots:?}");
        assert!((roots[1] - 0.5).abs() < 1e-9, "{roots:?}");
    }

    proptest! {
        // Oracle: cross-check against the cubic solver on [0,1].
        #[test]
        fn roots_match_cubic_solver(
            b in -5.0..5.0f64, c in -5.0..5.0f64, d in -5.0..5.0f64,
        ) {
            let p = Bernstein::from_power(&[d, c, b, 1.0]).unwrap();
            let want: Vec<f64> = crate::poly::solve_cubic(1.0, b, c, d)
                .into_iter()
                .filter(|r| (0.0..=1.0).contains(r))
                .collect();
            // Skip clustered or near-endpoint cases where dedup
            // policies legitimately differ between the two solvers.
            let clustered = want.windows(2).any(|w| (w[1] - w[0]).abs() < 1e-4)
                || want.iter().any(|r| *r < 1e-4 || *r > 1.0 - 1e-4);
            prop_assume!(!clustered);
            let got = p.roots(1e-12);
            prop_assert_eq!(got.len(), want.len(),
                "got {:?} want {:?}", &got, &want);
            for (g, w) in got.iter().zip(want.iter()) {
                prop_assert!((g - w).abs() < 1e-7,
                    "got {:?} want {:?}", &got, &want);
            }
        }
    }
}
