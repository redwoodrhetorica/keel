#![no_main]
use keel_geom::curve::{Circle3, Curve3};
use keel_math::vec::Vec3;
use keel_topo::Body;
use libfuzzer_sys::fuzz_target;

// Imprint a random circle onto a block's top face. Whether or not the
// circle lies on the face (the imprint may reject it), the body must
// stay valid and never panic; a successful imprint must leave a valid,
// winding-consistent body with the new edge carrying pcurves.
fuzz_target!(|data: (f64, f64, f64, f64, f64)| {
    let (sx, sy, sz, cx, cy) = data;
    if [sx, sy, sz, cx, cy].iter().any(|x| !x.is_finite()) {
        return;
    }
    let clamp = |x: f64, lo: f64, hi: f64| x.abs().clamp(lo, hi);
    let (dx, dy, dz) = (
        clamp(sx, 0.5, 50.0),
        clamp(sy, 0.5, 50.0),
        clamp(sz, 0.5, 50.0),
    );
    let mut b = Body::new();
    let Ok(out) = b.block(Vec3::ZERO, dx, dy, dz) else {
        return;
    };
    let top = match out.faces.last() {
        Some(&t) => t,
        None => return,
    };
    // Circle on the top plane z = dz, with a clamped center/radius.
    let r = clamp(cx, 0.05, dx.min(dy) * 0.5);
    let (ccx, ccy) = (
        cx.abs().clamp(r, dx - r).max(r),
        cy.abs().clamp(r, dy - r).max(r),
    );
    let Ok(circle) = Circle3::new(
        Vec3::new(ccx, ccy, dz),
        Vec3::new(1., 0., 0.),
        Vec3::new(0., 1., 0.),
        r,
    ) else {
        return;
    };
    let before = b.topology_hash();
    match b.imprint_closed_curve(top, &Curve3::Circle(circle), 1e-9) {
        Ok(rep) => {
            assert!(b.validate().is_ok(), "imprint produced invalid body");
            // The new edge is manifold with pcurves on both fins.
            let radial = b.edge(rep.edge).map(|e| e.radial.clone()).unwrap_or_default();
            assert_eq!(radial.len(), 2);
            for fk in radial {
                assert!(b.fin(fk).and_then(|f| f.pcurve).is_some());
            }
        }
        Err(_) => {
            // Atomic rejection: body unchanged.
            assert_eq!(b.topology_hash(), before, "rejected imprint mutated body");
            assert!(b.validate().is_ok());
        }
    }
});
