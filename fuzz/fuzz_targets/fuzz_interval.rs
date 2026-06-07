#![no_main]
use keel_math::interval::Interval;
use libfuzzer_sys::fuzz_target;

// Interval arithmetic soundness under arbitrary op programs over
// arbitrary f64s (M5a Task 0 gate): every node keeps the pointwise
// member enclosed, never produces NaN bounds, never inverts lo > hi.
fuzz_target!(|data: (Vec<f64>, Vec<u8>)| {
    let (seeds, ops) = data;
    let seeds: Vec<f64> = seeds
        .into_iter()
        .filter(|x| x.is_finite())
        .take(6)
        .collect();
    if seeds.len() < 2 || ops.len() > 64 {
        return;
    }
    let mut vals: Vec<f64> = seeds.clone();
    let mut ivs: Vec<Interval> = seeds.iter().map(|&x| Interval::point(x)).collect();
    for (k, op) in ops.iter().enumerate() {
        let i = k % vals.len();
        let j = (k + 1) % vals.len();
        let (v, iv) = match op % 5 {
            0 => (vals[i] + vals[j], ivs[i] + ivs[j]),
            1 => (vals[i] - vals[j], ivs[i] - ivs[j]),
            2 => (vals[i] * vals[j], ivs[i] * ivs[j]),
            3 => (-vals[i], -ivs[i]),
            _ => {
                let d = vals[j];
                if d == 0.0 {
                    continue;
                }
                match ivs[i].checked_div(ivs[j]) {
                    Some(r) => (vals[i] / d, r),
                    None => continue,
                }
            }
        };
        if v.is_nan() {
            break; // pointwise computation left the domain
        }
        assert!(
            !iv.lo.is_nan() && !iv.hi.is_nan() && iv.lo <= iv.hi,
            "invariant broken: {iv:?}"
        );
        if v.is_finite() {
            assert!(iv.contains(v), "{v} escaped {iv:?}");
        } else {
            // Pointwise overflow: the interval must be unbounded on
            // that side (the honest answer the audit demanded).
            if v > 0.0 {
                assert!(iv.hi.is_infinite(), "+inf member but bounded hi: {iv:?}");
            } else {
                assert!(iv.lo.is_infinite(), "-inf member but bounded lo: {iv:?}");
            }
            break;
        }
        vals[i] = v;
        ivs[i] = iv;
    }
});
