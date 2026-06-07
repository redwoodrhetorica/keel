#![no_main]
use libfuzzer_sys::fuzz_target;

// The constructor must either reject cleanly or yield a surface whose
// evaluation never panics and is insertion-invariant.
fuzz_target!(|data: (
    u8,
    u8,
    Vec<f64>,
    Vec<f64>,
    Vec<(f64, f64, f64)>,
    Vec<f64>,
    f64,
    f64
)| {
    let (du, dv, ku, kv, pts, ws, t, s) = data;
    let degree_u = 1 + (du as usize % 5);
    let degree_v = 1 + (dv as usize % 5);
    if ku.len() > 32 || kv.len() > 32 || pts.len() > 256 {
        return;
    }
    let points: Vec<_> = pts
        .iter()
        .map(|&(x, y, z)| keel_math::vec::Vec3::new(x, y, z))
        .collect();
    let result =
        keel_geom::nurbs_surface::NurbsSurface::new(degree_u, degree_v, ku, kv, points, Some(ws));
    let Ok(srf) = result else { return };
    if !t.is_finite() || !s.is_finite() {
        return;
    }
    let ((u0, u1), (v0, v1)) = srf.domain();
    let u = u0 + t.rem_euclid(1.0).clamp(0.0, 1.0) * (u1 - u0);
    let v = v0 + s.rem_euclid(1.0).clamp(0.0, 1.0) * (v1 - v0);
    let p = srf.point(u, v);
    assert!(p.is_finite());
    let _ = srf.derivatives(u, v, 2);
    if let Ok(s2) = srf.insert_knot_u(0.5 * u0 + 0.5 * u1) {
        assert!((s2.point(u, v) - p).norm() <= 1e-6 * (1.0 + p.norm()));
    }
});
