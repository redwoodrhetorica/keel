//! The cone boolean slice (swap task 30 groundwork): countersink carve
//! and exact mated plug re-union, plus the sphere-socket decline rail.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::body::Body;
use keel_topo::boolean::{BoolOp, boolean};

#[test]
fn countersink_carve_and_mated_plug_are_exact() {
    let pi = core::f64::consts::PI;
    let mut block = Body::new();
    block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
    // Frustum tool: radius 0.5 at z=1 widening at 45 degrees to 2.5 at
    // z=3, cutting a countersink into the top face z=2.
    let f = Frame3::from_z(Vec3::new(2.0, 2.0, 1.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut tool = Body::new();
    tool.loft_circles(f.clone(), 0.5, 2.5, 2.0).unwrap();
    let sunk = boolean(&block, &tool, BoolOp::Difference, 1e-7)
        .unwrap()
        .body;
    assert!(sunk.validate().is_ok());
    let vs = sunk.mass_properties().unwrap().volume;
    // Removed material: frustum from z=1 (r=0.5) to z=2 (r=1.5):
    // pi*h/3*(r0^2 + r0 r1 + r1^2) = pi/3 * 3.25.
    let expect = 32.0 - pi / 3.0 * 3.25;
    assert!((vs - expect).abs() < 1e-9, "countersunk {vs} vs {expect}");
    let ms = sunk.mesh_volume();
    assert!(
        (ms - vs).abs() <= 2e-2 * (1.0 + vs.abs()),
        "countersunk mesh {ms} vs mass {vs}"
    );

    // The EXACT mated plug re-unioned: coincident cone laterals and the
    // coincident floor cap resolve via the on-on machinery; the plug's
    // lateral splits at the sunk block's EXISTING rim (the one-sided
    // on-boundary seam). Union = block + the frustum part above z=2.
    let mut plug = Body::new();
    plug.loft_circles(f, 0.5, 2.5, 2.0).unwrap();
    let r = boolean(&sunk, &plug, BoolOp::Union, 1e-7).unwrap();
    assert!(r.body.validate().is_ok());
    let vu = r.body.mass_properties().unwrap().volume;
    let expect_u = 32.0 + pi / 3.0 * 12.25;
    assert!(
        (vu - expect_u).abs() < 1e-9,
        "plug union {vu} vs {expect_u}"
    );
}

#[test]
fn tolerant_countersink_plug_snaps_exact() {
    // The task-30 oracle (the M6 radial-gap pin's cone twin): a plug
    // whose radii sit 1e-5 UNDER the countersink (same taper: the
    // clearance fit exported as-is) and a plug whose axis is offset
    // 3e-6. Strict sees separated laterals; the tolerant prepare snaps
    // the plug's lateral onto the countersink's EXACT carrier, after
    // which the strict pipeline produces the mated-plug exact result.
    use keel_topo::boolean::boolean_tolerant;
    let pi = core::f64::consts::PI;
    let mut block = Body::new();
    block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
    let f = Frame3::from_z(Vec3::new(2.0, 2.0, 1.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut tool = Body::new();
    tool.loft_circles(f.clone(), 0.5, 2.5, 2.0).unwrap();
    let sunk = boolean(&block, &tool, BoolOp::Difference, 1e-7)
        .unwrap()
        .body;
    let expect_u = 32.0 + pi / 3.0 * 12.25;

    // Radial clearance: same taper, both radii 1e-5 under.
    let mut plug = Body::new();
    plug.loft_circles(f.clone(), 0.5 - 1e-5, 2.5 - 1e-5, 2.0)
        .unwrap();
    let (u, conf) = boolean_tolerant(&sunk, &plug, BoolOp::Union, 1e-7, 1e-4).unwrap();
    assert!(
        conf.salvaged && conf.tier == 2,
        "radial-gap plug must salvage: {:?}",
        (conf.salvaged, conf.tier)
    );
    assert!(
        conf.achieved_tolerance >= 1e-6 && conf.achieved_tolerance <= 1e-4,
        "achieved {}",
        conf.achieved_tolerance
    );
    let vu = u.body.mass_properties().unwrap().volume;
    assert!(
        (vu - expect_u).abs() < 1e-9,
        "snapped plug union {vu} vs {expect_u}"
    );

    // Axis-offset clearance: exact radii, axis 3e-6 off.
    let f2 = Frame3::from_z(Vec3::new(2.0 + 3e-6, 2.0, 1.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut plug2 = Body::new();
    plug2.loft_circles(f2, 0.5, 2.5, 2.0).unwrap();
    let (u2, c2) = boolean_tolerant(&sunk, &plug2, BoolOp::Union, 1e-7, 1e-4).unwrap();
    assert!(c2.salvaged, "offset plug must salvage");
    let v2 = u2.body.mass_properties().unwrap().volume;
    assert!((v2 - expect_u).abs() < 1e-9, "offset plug union {v2}");

    // Rail: a fuzz bound SMALLER than the gap must not salvage (an
    // honest strict decline is also acceptable).
    let mut plug3 = Body::new();
    plug3.loft_circles(f, 0.5 - 1e-5, 2.5 - 1e-5, 2.0).unwrap();
    if let Ok((_, c3)) = boolean_tolerant(&sunk, &plug3, BoolOp::Union, 1e-7, 1e-7) {
        assert!(!c3.salvaged, "under-fuzz gap must not salvage");
    }
}

#[test]
fn sphere_socket_carve_declines_honestly() {
    // Block minus a ball poking through the top face: strict sphere
    // socket carving is not yet assembled+integrated end to end; the
    // honesty gates must DECLINE it, never return a wrong volume.
    let mut block = Body::new();
    block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
    let mut ball = Body::new();
    ball.sphere(
        Frame3::from_z(Vec3::new(2.0, 2.0, 1.5), Vec3::new(0., 0., 1.)).unwrap(),
        1.0,
    )
    .unwrap();
    match boolean(&block, &ball, BoolOp::Difference, 1e-7) {
        Err(_) => {}
        Ok(r) => {
            // If a future milestone makes this assemble, it must be RIGHT.
            let v = r.body.mass_properties().unwrap().volume;
            let pi = core::f64::consts::PI;
            let cap = pi * 0.25 * 2.5 / 3.0;
            let expect = 32.0 - (4.0 * pi / 3.0 - cap);
            assert!((v - expect).abs() < 1e-9, "socket {v} vs exact {expect}");
        }
    }
}
