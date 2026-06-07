#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (f64, f64, f64, f64)| {
    let (a, b, c, d) = data;
    if ![a, b, c, d].iter().all(|v| v.is_finite()) {
        return;
    }
    let roots = keel_math::poly::solve_cubic(a, b, c, d);
    // Invariants: finite roots, count <= 3, inside the Cauchy bound.
    assert!(roots.len() <= 3);
    if a != 0.0 {
        let bound = 1.0 + (b.abs().max(c.abs()).max(d.abs())) / a.abs();
        for r in roots {
            assert!(r.is_finite());
            assert!(r.abs() <= bound * (1.0 + 1e-9));
        }
    }
});
