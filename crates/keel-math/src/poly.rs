//! Closed-degree polynomial solvers (research track B: Blinn-stable
//! quadratic, Yuksel-style monotonic-interval cubic). Power-basis
//! evaluation of higher degrees is banned; use the bernstein module.

use crate::newton::solve_bracketed;

/// Scale every coefficient by the power of two that brings the largest
/// magnitude near [0.5, 1). Power-of-two scaling is exact in binary
/// floating point and leaves the roots unchanged, while preventing the
/// overflow of discriminants and Cauchy bounds that extreme coefficient
/// ratios otherwise cause (found by fuzzing: finite inputs produced
/// non-finite roots). Coefficients whose scaled value underflows to
/// zero were negligible relative to the largest and are treated as
/// exact zeros.
fn normalize(coeffs: &mut [f64]) {
    let m = coeffs.iter().fold(0.0f64, |acc, c| acc.max(c.abs()));
    if m == 0.0 || !m.is_finite() {
        return;
    }
    let e = m.log2().ceil() as i32;
    let scale = 2.0f64.powi(-e);
    if scale.is_finite() && scale > 0.0 {
        for c in coeffs.iter_mut() {
            *c *= scale;
        }
    }
}

/// Real roots of a x^2 + b x + c, ascending. Stable form: no
/// subtraction of nearly equal quantities (Blinn / Numerical Recipes).
pub fn solve_quadratic(a: f64, b: f64, c: f64) -> Vec<f64> {
    let mut k = [a, b, c];
    normalize(&mut k);
    let (a, b, c) = (k[0], k[1], k[2]);
    if a == 0.0 {
        if b == 0.0 {
            return Vec::new();
        }
        // A denormal b can push the root beyond f64 range even after
        // normalization (fuzz finding 3); unrepresentable means no root.
        let r = -c / b;
        return if r.is_finite() { vec![r] } else { Vec::new() };
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return Vec::new();
    }
    let sqrt_disc = disc.sqrt();
    // q has the sign chosen so b + sign(b)*sqrt_disc never cancels.
    let q = -0.5 * (b + b.signum() * sqrt_disc);
    let mut roots = if q == 0.0 {
        vec![0.0]
    } else {
        let r1 = q / a;
        let r2 = c / q;
        if r1 == r2 { vec![r1] } else { vec![r1, r2] }
    };
    // Denormal coefficients can push a true root beyond f64 range
    // even after normalization; such roots are not representable and
    // are dropped (fuzz finding 2, 2026-06-07).
    roots.retain(|r| r.is_finite());
    roots.sort_by(f64::total_cmp);
    roots
}

/// Real roots of a x^3 + b x^2 + c x + d, ascending, deduplicated.
/// Monotonic-interval strategy (Yuksel HPG 2022): split the domain at
/// the derivative's roots, run bracketed Newton on each sign change.
pub fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    let mut k = [a, b, c, d];
    normalize(&mut k);
    let (a, b, c, d) = (k[0], k[1], k[2], k[3]);
    if a == 0.0 {
        return solve_quadratic(b, c, d);
    }

    // Cauchy bound: all real roots lie in [-bound, bound]. If the
    // leading coefficient is denormal-tiny relative to the rest, the
    // bound (and the third root) exceed f64 range: the representable
    // roots are those of the quadratic part (fuzz finding 2).
    let bound = 1.0 + (b.abs().max(c.abs()).max(d.abs())) / a.abs();
    if !bound.is_finite() {
        return solve_quadratic(b, c, d);
    }

    let eval = |x: f64| {
        let f = ((a * x + b) * x + c) * x + d;
        let df = (3.0 * a * x + 2.0 * b) * x + c;
        (f, df)
    };

    // Domain split points: derivative roots inside the bound.
    let mut cuts = vec![-bound];
    for r in solve_quadratic(3.0 * a, 2.0 * b, c) {
        if r > -bound && r < bound {
            cuts.push(r);
        }
    }
    cuts.push(bound);

    let mut roots = Vec::with_capacity(3);
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0], w[1]);
        // A critical point that IS a root (multiple root) is caught
        // here, since solve_bracketed returns endpoint roots.
        if let Some(r) = solve_bracketed(eval, lo, hi, 1e-14 * bound, 128) {
            roots.push(r);
        }
    }
    roots.sort_by(f64::total_cmp);
    // Deduplicate multiple roots found from adjacent intervals.
    roots.dedup_by(|x, y| (*x - *y).abs() <= 1e-12 * bound);
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn assert_roots_eq(mut got: Vec<f64>, mut want: Vec<f64>, tol: f64) {
        got.sort_by(f64::total_cmp);
        want.sort_by(f64::total_cmp);
        assert_eq!(got.len(), want.len(), "got {got:?} want {want:?}");
        for (g, w) in got.iter().zip(want.iter()) {
            assert!((g - w).abs() < tol, "got {got:?} want {want:?}");
        }
    }

    #[test]
    fn quadratic_simple() {
        // (x - 1)(x - 3) = x^2 - 4x + 3
        assert_roots_eq(solve_quadratic(1.0, -4.0, 3.0), vec![1.0, 3.0], 1e-14);
    }

    #[test]
    fn quadratic_catastrophic_cancellation() {
        // Roots 1e-8 and 1e8: the naive formula destroys the small root.
        let roots = solve_quadratic(1.0, -(1.0e8 + 1.0e-8), 1.0);
        assert_roots_eq(roots, vec![1.0e-8, 1.0e8], 1e-6);
        // Tight relative check on the small root.
        let small = solve_quadratic(1.0, -(1.0e8 + 1.0e-8), 1.0)
            .into_iter()
            .fold(f64::MAX, f64::min);
        assert!((small - 1.0e-8).abs() / 1.0e-8 < 1e-9);
    }

    #[test]
    fn quadratic_degenerates_to_linear() {
        assert_roots_eq(solve_quadratic(0.0, 2.0, -4.0), vec![2.0], 1e-14);
        assert!(solve_quadratic(0.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn quadratic_no_real_roots() {
        assert!(solve_quadratic(1.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn cubic_three_roots() {
        // (x + 2)(x - 1)(x - 5) = x^3 - 4x^2 - 7x + 10
        assert_roots_eq(
            solve_cubic(1.0, -4.0, -7.0, 10.0),
            vec![-2.0, 1.0, 5.0],
            1e-10,
        );
    }

    #[test]
    fn cubic_double_root() {
        // (x - 1)^2 (x + 2) = x^3 - 3x + 2; root 1 has multiplicity 2.
        let roots = solve_cubic(1.0, 0.0, -3.0, 2.0);
        assert!(roots.iter().any(|r| (r - 1.0).abs() < 1e-6), "{roots:?}");
        assert!(roots.iter().any(|r| (r + 2.0).abs() < 1e-10), "{roots:?}");
    }

    // Golden regression tests from the first fuzzing campaign
    // (2026-06-07): extreme coefficient ratios overflowed the
    // discriminant / Cauchy bound and returned non-finite roots.
    #[test]
    #[allow(clippy::excessive_precision)] // exact crash-artifact bits
    fn fuzz_regression_extreme_ratios_yield_finite_roots() {
        let cases = [
            (1e-308, 1e308, 1.0, 0.0),
            (1e-308, 1e300, 0.0, 1.0),
            (0.0, 1e-308, 1e308, 1.0),
            (5e-324, -1e308, 1e308, -1e300),
            (1e308, 1e-308, -1e308, 1e-300),
            // Finding 2: denormals survive normalization.
            (5e-324, 1.0, 1.0, 0.5),
            (1.0, 5e-324, 1.0, 5e-324),
            (5e-324, 5e-324, 1.0, 1.0),
            // Finding 3 (decoded from crash artifact 51a74872): the
            // linear early-return path was unfiltered.
            (1.33e-315, 0.0, 1.74e-310, 2.0),
            (0.0, 1e-310, 1.0, 0.0),
            // Finding 4 (artifact 2c7c25c4): midpoint overflow in
            // solve_bracketed when lo + hi exceeds f64::MAX.
            (
                2.2250738588309922e-308,
                -2.0000000037252903,
                6.7399159074193948e-306,
                1.6189543082925967e-319,
            ),
        ];
        for (a, b, c, d) in cases {
            for r in solve_cubic(a, b, c, d) {
                assert!(r.is_finite(), "cubic({a},{b},{c},{d}) gave {r}");
            }
            for r in solve_quadratic(a, b, c) {
                assert!(r.is_finite(), "quadratic({a},{b},{c}) gave {r}");
            }
        }
    }

    #[test]
    fn normalization_preserves_roots() {
        // Same polynomial at two scales must give the same roots.
        let r1 = solve_cubic(2.0, -8.0, -14.0, 20.0); // 2x the known cubic
        let r2 = solve_cubic(1.0, -4.0, -7.0, 10.0);
        assert_eq!(r1.len(), r2.len());
        for (x, y) in r1.iter().zip(r2.iter()) {
            assert!((x - y).abs() < 1e-10);
        }
    }

    #[test]
    fn cubic_single_real_root() {
        // x^3 + x + 1 has exactly one real root near -0.6823.
        let roots = solve_cubic(1.0, 0.0, 1.0, 1.0);
        assert_eq!(roots.len(), 1);
        assert!((roots[0] + 0.682_327_803_828_019_3).abs() < 1e-12);
    }

    proptest! {
        // Oracle: build the cubic from chosen roots, recover them.
        #[test]
        fn cubic_recovers_constructed_roots(
            r1 in -100.0..100.0f64,
            r2 in -100.0..100.0f64,
            r3 in -100.0..100.0f64,
        ) {
            // Skip clustered roots; multiplicity accuracy is tested
            // separately and deterministically.
            prop_assume!((r1 - r2).abs() > 1e-3);
            prop_assume!((r2 - r3).abs() > 1e-3);
            prop_assume!((r1 - r3).abs() > 1e-3);
            let b = -(r1 + r2 + r3);
            let c = r1 * r2 + r1 * r3 + r2 * r3;
            let d = -(r1 * r2 * r3);
            let got = solve_cubic(1.0, b, c, d);
            prop_assert_eq!(got.len(), 3, "got {:?}", &got);
            for want in [r1, r2, r3] {
                prop_assert!(
                    got.iter().any(|g| (g - want).abs() < 1e-6),
                    "missing root {} in {:?}", want, &got
                );
            }
        }
    }
}
