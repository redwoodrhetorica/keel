#![no_main]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use libfuzzer_sys::fuzz_target;

// Generalized winding number soundness under fuzzing: for a random
// primitive solid and a random probe point, the GWN is finite and in
// [~0, ~1]; a point clearly inside reads > 0.85 and a point clearly
// outside reads < 0.15 (the band between is graceful degradation near
// the boundary, asserted nowhere). Never NaN, never panics.
fuzz_target!(|data: ([f64; 7], u8)| {
    let [cx, cy, cz, r, px, py, pz] = data.0;
    if data.0.iter().any(|x| !x.is_finite()) {
        return;
    }
    let r = r.abs().clamp(0.3, 20.0);
    let c = Vec3::new(cx.clamp(-30.0, 30.0), cy.clamp(-30.0, 30.0), cz.clamp(-30.0, 30.0));
    let p = Vec3::new(px.clamp(-80.0, 80.0), py.clamp(-80.0, 80.0), pz.clamp(-80.0, 80.0));

    // Box or sphere, selected by the low bit.
    let (body, signed_dist): (Body, f64) = if data.1 & 1 == 0 {
        let mut b = Body::new();
        if b.block(c, r, r, r).is_err() {
            return;
        }
        // Signed distance to an axis-aligned box [c, c+r]^3 (negative
        // inside).
        let q = p - (c + Vec3::new(r, r, r) * 0.5);
        let half = r * 0.5;
        let d = Vec3::new(q.x.abs() - half, q.y.abs() - half, q.z.abs() - half);
        let outside = Vec3::new(d.x.max(0.0), d.y.max(0.0), d.z.max(0.0)).norm();
        let inside = d.x.max(d.y).max(d.z).min(0.0);
        (b, outside + inside)
    } else {
        let Ok(frame) = Frame3::from_z(c, Vec3::new(0., 0., 1.)) else {
            return;
        };
        let mut b = Body::new();
        if b.sphere(frame, r).is_err() {
            return;
        }
        (b, (p - c).norm() - r)
    };

    let w = body.generalized_winding_number(p);
    let scale = r.max(1.0);
    // Always finite and never blowing up (catches NaN / sign-flip bugs).
    assert!(w.is_finite(), "winding NaN");
    assert!((-1.5..=2.5).contains(&w), "winding {w} blew up");
    // The tight [0,1] bound holds off the boundary; exactly on the
    // boundary the winding is an ill-defined transition value.
    if signed_dist.abs() > 0.05 * scale {
        assert!((-0.05..=1.05).contains(&w), "winding {w} out of [0,1] (d={signed_dist})");
    }
    if signed_dist < -0.3 * scale {
        assert!(w > 0.85, "clearly-inside point has winding {w} (d={signed_dist})");
    }
    if signed_dist > 0.6 * scale + 0.5 {
        assert!(w < 0.15, "clearly-outside point has winding {w} (d={signed_dist})");
    }
});
