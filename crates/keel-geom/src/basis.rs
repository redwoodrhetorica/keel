//! B-spline basis evaluation (NURBS Book A2.2), stack-allocated.

use crate::MAX_ORDER;
use crate::knots::KnotVector;

/// All degree+1 nonzero basis values N_{span-p..=span, p}(u), in
/// `out[0..=p]` (NURBS Book A2.2). Convex corner-cutting: every output
/// is in [0,1] and they sum to 1 (partition of unity).
pub fn basis_funs(kv: &KnotVector, span: usize, u: f64) -> [f64; MAX_ORDER] {
    let p = kv.degree();
    let knots = kv.knots();
    let mut n = [0.0; MAX_ORDER];
    let mut left = [0.0; MAX_ORDER];
    let mut right = [0.0; MAX_ORDER];
    n[0] = 1.0;
    for j in 1..=p {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let temp = n[r] / (right[r + 1] + left[j - r]);
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knots::KnotVector;
    use proptest::prelude::*;

    #[test]
    fn degree_one_is_hat_function() {
        let kv = KnotVector::new(1, vec![0., 0., 1., 2., 2.]).unwrap();
        let n = basis_funs(&kv, kv.find_span(0.25), 0.25);
        assert!((n[0] - 0.75).abs() < 1e-15);
        assert!((n[1] - 0.25).abs() < 1e-15);
    }

    #[test]
    fn matches_bernstein_on_single_span() {
        // Clamped single-span degree 3 = cubic Bernstein basis.
        let kv = KnotVector::new(3, vec![0., 0., 0., 0., 1., 1., 1., 1.]).unwrap();
        let t = 0.3_f64;
        let n = basis_funs(&kv, kv.find_span(t), t);
        let b = [
            (1.0 - t).powi(3),
            3.0 * t * (1.0 - t).powi(2),
            3.0 * t * t * (1.0 - t),
            t.powi(3),
        ];
        for i in 0..4 {
            assert!((n[i] - b[i]).abs() < 1e-15, "i={i}");
        }
    }

    proptest! {
        #[test]
        fn partition_of_unity(u in 0.0..2.0f64) {
            let kv = KnotVector::new(
                3,
                vec![0., 0., 0., 0., 0.7, 1.3, 2., 2., 2., 2.],
            ).unwrap();
            let n = basis_funs(&kv, kv.find_span(u), u);
            let sum: f64 = n[..4].iter().sum();
            prop_assert!((sum - 1.0).abs() < 1e-12);
            prop_assert!(n[..4].iter().all(|&v| (-1e-15..=1.0 + 1e-12).contains(&v)));
        }
    }
}
