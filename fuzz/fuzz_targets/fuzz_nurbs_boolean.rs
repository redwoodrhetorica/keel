#![no_main]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use libfuzzer_sys::fuzz_target;

// A NURBS sphere booleaned with an analytic sphere (the shipped M7b
// path: NURBS-faced operand, certified-numeric fitted SSI seam). Both
// are seamed about the world-X axis so the horizontal intersection
// circle stays crossing-free; centres are separated along world-Z into
// a clean transversal lens. Invariants: never panics; a successful
// result is a valid solid that is EPSILON-SOLID at its achieved
// tolerance (the tolerant-edge contract), with finite volume bounded by
// the operands; degenerate/declined configs return cleanly.
fn x_pole_frame(center: Vec3) -> Frame3 {
    Frame3 {
        origin: center,
        x: Vec3::new(0., 1., 0.),
        y: Vec3::new(0., 0., 1.),
        z: Vec3::new(1., 0., 0.),
    }
}

fuzz_target!(|data: ([f64; 3], u8)| {
    let [ra, rb, dz] = data.0;
    if data.0.iter().any(|x| !x.is_finite()) {
        return;
    }
    let ra = ra.abs().clamp(0.5, 3.0);
    let rb = rb.abs().clamp(0.5, 3.0);
    // Transversal lens: the centres are far enough apart for two distinct
    // caps but close enough to actually intersect (no tangency, no
    // containment).
    let lo = (ra - rb).abs() + 0.2;
    let hi = ra + rb - 0.2;
    if hi <= lo {
        return;
    }
    let dz = dz.abs().clamp(lo, hi).clamp(lo, hi);

    let mut a = Body::new();
    if a.nurbs_sphere(x_pole_frame(Vec3::ZERO), ra).is_err() {
        return;
    }
    let mut b = Body::new();
    if b.sphere(x_pole_frame(Vec3::new(0., 0., dz)), rb).is_err() {
        return;
    }

    let vol = |r: f64| 4.0 / 3.0 * core::f64::consts::PI * r * r * r;
    let v_sum = vol(ra) + vol(rb);
    let op = match data.1 % 3 {
        0 => BoolOp::Intersection,
        1 => BoolOp::Union,
        _ => BoolOp::Difference,
    };

    if let Ok(res) = boolean(&a, &b, op, 1e-6) {
        assert!(res.body.validate().is_ok(), "nurbs boolean produced invalid body");
        // The tolerant-edge contract: the result is epsilon-solid at the
        // certified bound the SSI achieved (a real nonzero value for the
        // fitted seam).
        let eps = res.body.achieved_tolerance();
        assert!(eps.is_finite() && eps >= 0.0, "non-finite achieved tolerance {eps}");
        assert!(
            res.body.epsilon_solid(eps),
            "nurbs result not epsilon-solid at achieved {eps}"
        );
        let v = res.body.tessellated_volume();
        assert!(v.is_finite(), "non-finite nurbs result volume");
        assert!(v >= -1e-4, "negative nurbs result volume {v}");
        if res.faults.is_empty() {
            let slack = 1e-2 * (v_sum + 1.0);
            assert!(v <= v_sum + slack, "result {v} exceeds operand sum {v_sum}");
        }
    }
});
