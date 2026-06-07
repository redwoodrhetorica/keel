#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|coeffs: Vec<f64>| {
    if coeffs.is_empty() || coeffs.len() > 32 {
        return;
    }
    if !coeffs.iter().all(|v| v.is_finite() && v.abs() < 1e12) {
        return;
    }
    if let Some(p) = keel_math::bernstein::Bernstein::new(coeffs) {
        let roots = p.roots(1e-10);
        for r in roots {
            // Invariants: roots live in [0,1] and are finite.
            assert!(r.is_finite());
            assert!((-1e-9..=1.0 + 1e-9).contains(&r));
        }
    }
});
