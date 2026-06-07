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

/// All nonzero basis functions and derivatives up to order `d` at `u`
/// (NURBS Book A2.3). Returns ders[k][j] = (d^k/du^k) N_{span-p+j, p}(u)
/// for k in 0..=d, j in 0..=p. Rows beyond the degree are zero (the
/// derivative of a degree-p polynomial vanishes; M2a lesson: zero-fill,
/// never truncate the output array).
pub fn basis_ders(kv: &KnotVector, span: usize, u: f64, d: usize) -> Vec<[f64; MAX_ORDER]> {
    let p = kv.degree();
    let knots = kv.knots();
    // ndu: upper triangle holds basis values, lower the knot
    // differences (A2.3 shared table).
    let mut ndu = [[0.0f64; MAX_ORDER]; MAX_ORDER];
    let mut left = [0.0f64; MAX_ORDER];
    let mut right = [0.0f64; MAX_ORDER];
    ndu[0][0] = 1.0;
    for j in 1..=p {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            ndu[j][r] = right[r + 1] + left[j - r];
            let temp = ndu[r][j - 1] / ndu[j][r];
            ndu[r][j] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        ndu[j][j] = saved;
    }
    let mut ders = vec![[0.0f64; MAX_ORDER]; d + 1];
    for j in 0..=p {
        ders[0][j] = ndu[j][p];
    }
    let dcap = d.min(p);
    // a: two alternating rows of the derivative coefficient table.
    let mut a = [[0.0f64; MAX_ORDER]; 2];
    for r in 0..=p {
        let (mut s1, mut s2) = (0usize, 1usize);
        a[0][0] = 1.0;
        a[1][0] = 0.0;
        for k in 1..=dcap {
            let mut dv = 0.0;
            let rk = r as i64 - k as i64;
            let pk = (p - k) as i64;
            if r >= k {
                a[s2][0] = a[s1][0] / ndu[(pk + 1) as usize][rk as usize];
                dv = a[s2][0] * ndu[rk as usize][pk as usize];
            }
            let j1 = if rk >= -1 { 1 } else { -rk };
            let j2 = if r as i64 - 1 <= pk {
                k as i64 - 1
            } else {
                p as i64 - r as i64
            };
            for j in j1..=j2 {
                let ju = j as usize;
                a[s2][ju] = (a[s1][ju] - a[s1][ju - 1]) / ndu[(pk + 1) as usize][(rk + j) as usize];
                dv += a[s2][ju] * ndu[(rk + j) as usize][pk as usize];
            }
            if r as i64 <= pk {
                a[s2][k] = -a[s1][k - 1] / ndu[(pk + 1) as usize][r];
                dv += a[s2][k] * ndu[r][pk as usize];
            }
            ders[k][r] = dv;
            std::mem::swap(&mut s1, &mut s2);
        }
    }
    // Multiply through by p! / (p - k)!.
    let mut factor = p as f64;
    for (k, row) in ders.iter_mut().enumerate().skip(1).take(dcap) {
        for v in row[..=p].iter_mut() {
            *v *= factor;
        }
        factor *= (p - k) as f64;
    }
    ders
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

    #[test]
    fn ders_row_zero_matches_basis_funs() {
        let kv = KnotVector::new(3, vec![0., 0., 0., 0., 0.4, 0.7, 1., 1., 1., 1.]).unwrap();
        for &u in &[0.0, 0.2, 0.4, 0.55, 0.9, 1.0] {
            let span = kv.find_span(u);
            let n = basis_funs(&kv, span, u);
            let d = basis_ders(&kv, span, u, 2);
            for j in 0..=3 {
                assert!((d[0][j] - n[j]).abs() < 1e-15, "u={u} j={j}");
            }
        }
    }

    #[test]
    fn ders_sum_to_zero_for_k_ge_1() {
        // Partition of unity differentiates to zero.
        let kv = KnotVector::new(2, vec![0., 0., 0., 0.5, 1., 1., 1.]).unwrap();
        let u = 0.3;
        let span = kv.find_span(u);
        let d = basis_ders(&kv, span, u, 2);
        for (k, row) in d.iter().enumerate().skip(1) {
            let s: f64 = row[..=2].iter().sum();
            assert!(s.abs() < 1e-12, "k={k} sum={s}");
        }
    }

    #[test]
    fn ders_beyond_degree_are_zero() {
        let kv = KnotVector::new(1, vec![0., 0., 1., 1.]).unwrap();
        let d = basis_ders(&kv, kv.find_span(0.5), 0.5, 3);
        for (a, b) in d[2][..=1].iter().zip(&d[3][..=1]) {
            assert_eq!(*a, 0.0);
            assert_eq!(*b, 0.0);
        }
    }

    #[test]
    fn ders_match_bernstein_closed_form() {
        // Single-span degree 2: N = [(1-t)^2, 2t(1-t), t^2].
        // N' = [-2(1-t), 2-4t, 2t]; N'' = [2, -4, 2].
        let kv = KnotVector::new(2, vec![0., 0., 0., 1., 1., 1.]).unwrap();
        let t = 0.3_f64;
        let d = basis_ders(&kv, kv.find_span(t), t, 2);
        let want1 = [-2.0 * (1.0 - t), 2.0 - 4.0 * t, 2.0 * t];
        let want2 = [2.0, -4.0, 2.0];
        for j in 0..=2 {
            assert!((d[1][j] - want1[j]).abs() < 1e-14, "k=1 j={j}");
            assert!((d[2][j] - want2[j]).abs() < 1e-14, "k=2 j={j}");
        }
    }

    proptest! {
        // Finite-difference oracle on first derivatives. h = 1e-5 with
        // 1e-4-scaled tolerance: stay above the f64 cancellation floor
        // (M2a lesson: h too small makes FD noise, not signal).
        #[test]
        fn first_ders_match_finite_difference(u in 0.05..0.95f64) {
            let kv = KnotVector::new(
                3,
                vec![0., 0., 0., 0., 0.3, 0.6, 1., 1., 1., 1.],
            ).unwrap();
            let h = 1e-5;
            let span = kv.find_span(u);
            // Same span on both sides or FD compares different spans.
            prop_assume!(kv.find_span(u - h) == span && kv.find_span(u + h) == span);
            let d = basis_ders(&kv, span, u, 1);
            let a = basis_funs(&kv, span, u - h);
            let b = basis_funs(&kv, span, u + h);
            for j in 0..=3 {
                let fd = (b[j] - a[j]) / (2.0 * h);
                prop_assert!((d[1][j] - fd).abs() < 1e-4 * (1.0 + fd.abs()), "j={j}");
            }
        }

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
