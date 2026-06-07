#![no_main]
use libfuzzer_sys::fuzz_target;

// Arbitrary degree/knots/points/weights: the constructor must either
// reject cleanly or yield a curve whose evaluation never panics and
// is insertion-invariant.
fuzz_target!(|data: (u8, Vec<f64>, Vec<(f64, f64, f64)>, Vec<f64>, f64)| {
    let (deg, knots, pts, ws, t) = data;
    let degree = 1 + (deg as usize % 11);
    if knots.len() > 64 || pts.len() > 32 {
        return;
    }
    let points: Vec<_> = pts
        .iter()
        .map(|&(x, y, z)| keel_math::vec::Vec3::new(x, y, z))
        .collect();
    let result = keel_geom::nurbs_curve::NurbsCurve::new(degree, knots, points, Some(ws));
    let Ok(c) = result else { return };
    // Constructor accepted: inputs were validated finite.
    let (a, b) = c.domain();
    if !t.is_finite() {
        return;
    }
    let u = a + t.rem_euclid(1.0).clamp(0.0, 1.0) * (b - a);
    let p = c.point(u);
    assert!(p.is_finite());
    let _ = c.derivatives(u, 2);
    if let Ok(c2) = c.insert_knot(0.5 * a + 0.5 * b) {
        assert!((c2.point(u) - p).norm() <= 1e-6 * (1.0 + p.norm()));
    }
});
