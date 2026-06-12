#![no_main]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use libfuzzer_sys::fuzz_target;

// A block countersunk by an axis-aligned cone frustum (task 38: gate
// coverage for the cone vocabulary task 30 added). Random extents,
// radii, centre, base depth and height, clamped to a transversal
// countersink configuration: the frustum base strictly inside the
// block, the frustum top above the block top, the footprint inside the
// walls at every crossed height. Invariants: never panics; a successful
// result is a valid solid with finite volume bounded by the operands;
// degenerate configs decline cleanly.
fuzz_target!(|data: ([f64; 9], u8)| {
    let [ex, ey, ez, cx, cy, r0, r1, cbase, ch] = data.0;
    if data.0.iter().any(|x| !x.is_finite()) {
        return;
    }
    let ext = Vec3::new(
        ex.abs().clamp(2.0, 20.0),
        ey.abs().clamp(2.0, 20.0),
        ez.abs().clamp(2.0, 20.0),
    );
    // Frustum base strictly inside; top pokes out of the block top.
    let base = cbase.abs().clamp(0.3, ext.z - 0.3).min(ext.z - 0.3);
    let height = ch.abs().clamp(ext.z - base + 0.3, ext.z - base + 50.0);
    // Radii: bounded so the WIDEST crossed section (at the block top)
    // stays inside the footprint with margin.
    let rmax = (ext.x.min(ext.y) * 0.5 - 0.3).max(0.3);
    let r0 = r0.abs().clamp(0.2, rmax);
    let r1 = r1.abs().clamp(0.2, rmax * 2.0);
    let r_top = r0 + (r1 - r0) * (ext.z - base) / height;
    let rmaxz = r0.max(r_top);
    if rmaxz >= rmax {
        return;
    }
    let cx = cx.abs().clamp(rmaxz + 0.1, ext.x - rmaxz - 0.1).max(rmaxz + 0.1);
    let cy = cy.abs().clamp(rmaxz + 0.1, ext.y - rmaxz - 0.1).max(rmaxz + 0.1);
    if cx + rmaxz >= ext.x || cy + rmaxz >= ext.y {
        return;
    }
    // A genuine cone, not a cylinder in disguise.
    if (r1 - r0).abs() < 1e-3 {
        return;
    }

    let mut a = Body::new();
    if a.block(Vec3::ZERO, ext.x, ext.y, ext.z).is_err() {
        return;
    }
    let Ok(frame) = Frame3::from_z(Vec3::new(cx, cy, base), Vec3::new(0., 0., 1.)) else {
        return;
    };
    let mut b = Body::new();
    if b.loft_circles(frame, r0, r1, height).is_err() {
        return;
    }

    let v_block = ext.x * ext.y * ext.z;
    let op = if data.1 & 1 == 0 {
        BoolOp::Difference
    } else {
        BoolOp::Intersection
    };

    if let Ok(res) = boolean(&a, &b, op, 1e-7) {
        assert!(
            res.body.validate().is_ok(),
            "cone boolean produced invalid body"
        );
        let v = res.body.tessellated_volume();
        assert!(v.is_finite(), "non-finite cone result volume");
        assert!(v >= -1e-4, "negative cone result volume {v}");
        if res.faults.is_empty() {
            let slack = 1e-3 * (v_block + 1.0);
            assert!(v <= v_block + slack, "result {v} exceeds block {v_block}");
        }
    }
});
