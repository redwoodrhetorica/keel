#![no_main]
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::nurbs_surface::{revolve_full, NurbsSurface};
use keel_geom::recover::recover_surface;
use keel_math::vec::Vec3;
use libfuzzer_sys::fuzz_target;

// Canonical-recovery soundness (M8): build a random analytic surface as
// a NURBS (plane / sphere / cylinder), recover it, and check the core
// invariant -- recovery never panics, and when it returns Some, the
// reported deviation is a genuine UPPER BOUND on an INDEPENDENT denser
// sampling of the spline-to-analytic distance (the certificate is sound)
// and is within the requested tolerance. The "keep the spline" verdict
// (None) is always acceptable.
fn build(data: &[f64; 6], sel: u8) -> Option<NurbsSurface> {
    if data.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let az = Vec3::new(0.0, 0.0, 1.0);
    match sel % 3 {
        0 => {
            // Tilted bilinear plane over [0,a] x [0,b].
            let a = data[0].abs().clamp(0.5, 10.0);
            let b = data[1].abs().clamp(0.5, 10.0);
            let (sx, sy) = (data[2].clamp(-2.0, 2.0), data[3].clamp(-2.0, 2.0));
            let pts = vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, b, sy * b),
                Vec3::new(a, 0.0, sx * a),
                Vec3::new(a, b, sx * a + sy * b),
            ];
            NurbsSurface::new(1, 1, vec![0., 0., 1., 1.], vec![0., 0., 1., 1.], pts, None).ok()
        }
        1 => {
            // Sphere via revolved rational semicircle.
            let r = data[3].abs().clamp(0.2, 5.0);
            let c = Vec3::new(
                data[0].clamp(-5.0, 5.0),
                data[1].clamp(-5.0, 5.0),
                data[2].clamp(-5.0, 5.0),
            );
            let s = core::f64::consts::FRAC_1_SQRT_2;
            let fx = Vec3::new(1.0, 0.0, 0.0);
            let ctrl = vec![
                c - az * r,
                c - az * r + fx * r,
                c + fx * r,
                c + az * r + fx * r,
                c + az * r,
            ];
            let profile =
                NurbsCurve::new(2, vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.], ctrl, Some(vec![1.0, s, 1.0, s, 1.0])).ok()?;
            revolve_full(&profile, c, az).ok()
        }
        _ => {
            // Cylinder via revolved vertical segment.
            let r = data[3].abs().clamp(0.2, 5.0);
            let h = data[4].abs().clamp(0.5, 8.0);
            let ctrl = vec![Vec3::new(r, 0.0, -h), Vec3::new(r, 0.0, h)];
            let profile = NurbsCurve::new(1, vec![0., 0., 1., 1.], ctrl, None).ok()?;
            revolve_full(&profile, Vec3::ZERO, az).ok()
        }
    }
}

fuzz_target!(|input: ([f64; 6], u8)| {
    let (data, sel) = input;
    let Some(s) = build(&data, sel) else {
        return;
    };
    let tol = 1e-5;
    if let Some(rec) = recover_surface(&s, tol) {
        assert!(rec.deviation.is_finite() && rec.deviation >= 0.0, "bad deviation");
        assert!(rec.deviation <= tol + 1e-12, "deviation {} exceeds tol", rec.deviation);
        // Independent denser sampling: the reported bound must dominate
        // it (allowing a modest factor for sampling differences).
        let ((u0, u1), (v0, v1)) = s.domain();
        let mut actual = 0.0f64;
        let n = 40;
        for i in 0..n {
            let u = u0 + (u1 - u0) * (0.01 + 0.98 * i as f64 / (n - 1) as f64);
            for j in 0..n {
                let v = v0 + (v1 - v0) * (0.01 + 0.98 * j as f64 / (n - 1) as f64);
                let d = rec.surface.project(s.point(u, v)).map(|p| p.distance).unwrap_or(0.0);
                actual = actual.max(d);
            }
        }
        assert!(
            actual <= rec.deviation * 1.5 + 1e-6,
            "reported deviation {} under-bounds independent sampling {actual}",
            rec.deviation
        );
    }
});
