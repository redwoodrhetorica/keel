#![no_main]
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_math::vec::Vec3;
use libfuzzer_sys::fuzz_target;

// Two axis-aligned boxes with random origin/extents and a random op.
// Invariants (partial-success boolean model): the call never panics;
// a successful CLEAN result is a valid solid whose volume is finite and
// within the loose set bounds (intersection <= min, union in
// [max, sum], difference <= |A|). Coplanar/touching/degenerate configs
// (deferred to M6b) come back as faults or Err, which is fine.
fuzz_target!(|data: ([f64; 12], u8)| {
    let (vals, sel) = data;
    let [ax, ay, az, adx, ady, adz, bx, by, bz, bdx, bdy, bdz] = vals;
    if vals.iter().any(|x| !x.is_finite()) {
        return;
    }
    let ext = |x: f64| x.abs().clamp(0.5, 20.0);
    let org = |x: f64| x.clamp(-20.0, 20.0);

    let mut a = Body::new();
    let Ok(_) = a.block(
        Vec3::new(org(ax), org(ay), org(az)),
        ext(adx),
        ext(ady),
        ext(adz),
    ) else {
        return;
    };
    let mut b = Body::new();
    let Ok(_) = b.block(
        Vec3::new(org(bx), org(by), org(bz)),
        ext(bdx),
        ext(bdy),
        ext(bdz),
    ) else {
        return;
    };

    let va = a.mass_properties().map(|m| m.volume).unwrap_or(0.0);
    let vb = b.mass_properties().map(|m| m.volume).unwrap_or(0.0);

    let op = match sel % 3 {
        0 => BoolOp::Union,
        1 => BoolOp::Intersection,
        _ => BoolOp::Difference,
    };

    if let Ok(res) = boolean(&a, &b, op, 1e-7) {
        assert!(res.body.validate().is_ok(), "boolean produced invalid body");
        // A clean EMPTY result (disjoint intersection / swallowed
        // difference, dossier 29 rung 1) has no faces, so analytic mass
        // properties decline; its volume is zero by definition. Mass
        // declining on a NON-empty body is still a failure.
        let v = match res.body.mass_properties() {
            Ok(m) => m.volume,
            Err(_) => {
                let mv = res.body.mesh_volume();
                assert!(
                    mv.abs() <= 1e-9,
                    "mass declined on a non-empty body (mesh {mv})"
                );
                0.0
            }
        };
        assert!(v.is_finite(), "non-finite result volume");
        assert!(v >= -1e-6, "negative result volume {v}");
        // Loose set bounds only when the result is clean (no faults).
        if res.faults.is_empty() {
            let slack = 1e-4 * (va + vb + 1.0);
            match op {
                BoolOp::Intersection => assert!(v <= va.min(vb) + slack, "A∩B {v} > min"),
                BoolOp::Union => {
                    assert!(v <= va + vb + slack, "A∪B {v} > sum");
                    assert!(v >= va.max(vb) - slack, "A∪B {v} < max");
                }
                BoolOp::Difference => assert!(v <= va + slack, "A-B {v} > |A|"),
            }
        }
    }
});
