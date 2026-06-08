#![no_main]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use libfuzzer_sys::fuzz_target;

// A block drilled by an axis-aligned cylinder (a blind hole that clears
// the block at the top). Random extents/radius/centre/depth, clamped to
// a clean transversal blind-hole configuration. Invariants: never
// panics; a successful result is a valid solid with finite volume
// bounded by the operands (difference <= block, intersection <= min);
// degenerate configs decline cleanly.
fuzz_target!(|data: ([f64; 8], u8)| {
    let [ex, ey, ez, cx, cy, cr, cbase, ch] = data.0;
    if data.0.iter().any(|x| !x.is_finite()) {
        return;
    }
    let ext = Vec3::new(
        ex.abs().clamp(2.0, 20.0),
        ey.abs().clamp(2.0, 20.0),
        ez.abs().clamp(2.0, 20.0),
    );
    // Radius small enough to stay inside the footprint with margin.
    let rmax = (ext.x.min(ext.y) * 0.5 - 0.3).max(0.3);
    let r = cr.abs().clamp(0.2, rmax);
    // Centre within the footprint, radius+margin from the walls.
    let cx = cx.abs().clamp(r + 0.1, ext.x - r - 0.1).max(r + 0.1);
    let cy = cy.abs().clamp(r + 0.1, ext.y - r - 0.1).max(r + 0.1);
    if cx + r >= ext.x || cy + r >= ext.y {
        return;
    }
    // Hole floor strictly inside the block; cylinder pokes out the top.
    let base = cbase.abs().clamp(0.3, ext.z - 0.3).min(ext.z - 0.3);
    let height = ch.abs().clamp(ext.z - base + 0.3, ext.z - base + 50.0);

    let mut a = Body::new();
    if a.block(Vec3::ZERO, ext.x, ext.y, ext.z).is_err() {
        return;
    }
    let Ok(frame) = Frame3::from_z(Vec3::new(cx, cy, base), Vec3::new(0., 0., 1.)) else {
        return;
    };
    let mut b = Body::new();
    if b.cylinder(frame, r, height).is_err() {
        return;
    }

    let v_block = ext.x * ext.y * ext.z;
    let op = if data.1 & 1 == 0 {
        BoolOp::Difference
    } else {
        BoolOp::Intersection
    };

    if let Ok(res) = boolean(&a, &b, op, 1e-7) {
        assert!(res.body.validate().is_ok(), "cyl boolean produced invalid body");
        let v = res.body.tessellated_volume();
        assert!(v.is_finite(), "non-finite cyl result volume");
        assert!(v >= -1e-4, "negative cyl result volume {v}");
        if res.faults.is_empty() {
            let slack = 1e-3 * (v_block + 1.0);
            assert!(v <= v_block + slack, "result {v} exceeds block {v_block}");
        }
    }
});
