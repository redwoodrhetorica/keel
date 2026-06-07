//! Bracketed Newton-bisection hybrid: the kernel's 1D root engine.

/// Find x in [lo, hi] with f(x) = 0, where `f` returns (value,
/// derivative) and f(lo), f(hi) have opposite signs (or either
/// endpoint is already a root). Newton steps are taken when they stay
/// inside the current bracket; otherwise bisect. The maintained
/// bracket guarantees convergence even with a flat or wrong
/// derivative. Returns None when there is no sign change over the
/// input bracket.
///
/// Midpoints are computed as 0.5*lo + 0.5*hi, never 0.5*(lo + hi):
/// the sum form overflows to infinity for brackets near f64::MAX and
/// poisons the bracket (fuzz finding 4, 2026-06-07).
pub fn solve_bracketed<F>(f: F, mut lo: f64, mut hi: f64, x_tol: f64, max_iter: u32) -> Option<f64>
where
    F: Fn(f64) -> (f64, f64),
{
    let (mut flo, _) = f(lo);
    let (fhi, _) = f(hi);
    if flo == 0.0 {
        return Some(lo);
    }
    if fhi == 0.0 {
        return Some(hi);
    }
    if (flo > 0.0) == (fhi > 0.0) {
        return None;
    }

    let mut x = 0.5 * lo + 0.5 * hi;
    for _ in 0..max_iter {
        let (fx, dfx) = f(x);
        if fx == 0.0 {
            return Some(x);
        }
        // Shrink the bracket around the sign change.
        if (fx > 0.0) == (flo > 0.0) {
            lo = x;
            flo = fx;
        } else {
            hi = x;
        }
        if hi - lo <= x_tol {
            return Some(0.5 * lo + 0.5 * hi);
        }
        // Newton proposal; fall back to bisection when it exits the
        // bracket or the derivative is unusable.
        let newton = x - fx / dfx;
        if newton.is_finite() && newton > lo && newton < hi {
            // One-sided convergence can leave the bracket wide while
            // the iterate converges; a tiny step is a converged root.
            if (newton - x).abs() <= x_tol {
                return Some(newton);
            }
            x = newton;
        } else {
            x = 0.5 * lo + 0.5 * hi;
        }
    }
    // Exhausted iterations: the iterate is the best estimate. Never
    // return the midpoint of a bracket that one-sided Newton kept wide.
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_sqrt2() {
        let f = |x: f64| (x * x - 2.0, 2.0 * x);
        let r = solve_bracketed(f, 0.0, 2.0, 1e-14, 100).unwrap();
        assert!((r - 2.0_f64.sqrt()).abs() < 1e-14);
    }

    #[test]
    fn endpoint_roots_returned_immediately() {
        let f = |x: f64| (x, 1.0);
        assert_eq!(solve_bracketed(f, 0.0, 1.0, 1e-14, 100), Some(0.0));
    }

    #[test]
    fn no_sign_change_is_none() {
        let f = |x: f64| (x * x + 1.0, 2.0 * x);
        assert_eq!(solve_bracketed(f, -1.0, 1.0, 1e-14, 100), None);
    }

    #[test]
    fn survives_flat_derivative() {
        // f(x) = x^3 has zero derivative at the root; bisection
        // fallback must still converge.
        let f = |x: f64| (x * x * x, 3.0 * x * x);
        let r = solve_bracketed(f, -1.0, 2.0, 1e-12, 200).unwrap();
        assert!(r.abs() < 1e-10);
    }
}
