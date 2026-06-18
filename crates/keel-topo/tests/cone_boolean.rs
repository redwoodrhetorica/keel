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
fn tilted_cone_disjoint_block_is_fast_and_correct_task47() {
    // task 47: an axis-aligned block and a TILTED cone with disjoint AABBs.
    // The plane/cone section rung works on the INFINITE cone, so a block
    // plane near-parallel to a cone ruling yields an ellipse running tens of
    // units up the far nappe. Before the finite-band bound, imprinting that
    // phantom ellipse cost ~1.7 s and raised spurious faults (a false
    // decline); now it is short-circuited by axial extent vs the cone face
    // band. Intersection is the correct empty set; difference is the whole
    // cone. (Found by the evolutionary explorer; LOG Addendum 238.)
    let cone_frame = Frame3::from_z(
        Vec3::new(-1.57665870351692, 2.8749586010410453, 1.7976305535677664),
        Vec3::new(-0.5109344101770357, 0.4748539943223482, -0.4797740510998425),
    )
    .unwrap();
    let mut cone = Body::new();
    cone.cone(cone_frame, 1.7290277039162048, 2.761525492133425)
        .unwrap();
    let mut blk = Body::new();
    blk.block(
        Vec3::new(
            -0.15174651733833588,
            0.8677412594822895,
            -2.3397407903158065,
        ),
        2.7717690470589265,
        2.2579544930304674,
        1.576714175012244,
    )
    .unwrap();

    let t = std::time::Instant::now();
    let inter = boolean(&cone, &blk, BoolOp::Intersection, 1e-7).unwrap();
    let dt = t.elapsed();
    // Cliff guard: the phantom-ellipse path took ~1.7 s, the bound makes it
    // ~1 ms. One second is a generous separator (1000x headroom) that still
    // catches a regression back to the giant-ellipse sampling.
    assert!(
        dt.as_secs_f64() < 1.0,
        "task47 perf cliff regressed: {dt:?}"
    );
    assert!(inter.body.validate().is_ok());
    // Disjoint AABBs: the intersection is empty.
    assert_eq!(
        inter.body.counts().f,
        0,
        "disjoint cone^block must be empty"
    );
    assert!(inter.body.mesh_volume().abs() < 1e-9);

    // Difference removes nothing: cone minus a disjoint block is the cone.
    let diff = boolean(&cone, &blk, BoolOp::Difference, 1e-7).unwrap();
    assert!(diff.body.validate().is_ok());
    let v = diff.body.mass_properties().unwrap().volume;
    let cone_alone = cone.mass_properties().unwrap().volume;
    assert!(
        (v - cone_alone).abs() < 1e-6,
        "cone-minus-disjoint-block {v} vs cone {cone_alone}"
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
fn sphere_socket_carve_is_exact() {
    // The sphere strict-boolean milestone (task 36): a ball poking
    // through the block's top face. All three ops assemble and their
    // mass properties match the closed forms exactly; the re-unioned
    // ball (coincident sphere laterals, the on-on class) restores the
    // union. The old decline rail's wrong-positive (an Euler-valid
    // body whose bowl integrated to silent zero) stays dead.
    let pi = core::f64::consts::PI;
    let cap = pi * 0.25 * 2.5 / 3.0; // spherical cap above z = 2
    let ball_v = 4.0 * pi / 3.0;
    let mut block = Body::new();
    block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
    let mut ball = Body::new();
    ball.sphere(
        Frame3::from_z(Vec3::new(2.0, 2.0, 1.5), Vec3::new(0., 0., 1.)).unwrap(),
        1.0,
    )
    .unwrap();
    for (op, exact) in [
        (BoolOp::Difference, 32.0 - (ball_v - cap)),
        (BoolOp::Intersection, ball_v - cap),
        (BoolOp::Union, 32.0 + cap),
    ] {
        let r = boolean(&block, &ball, op, 1e-7).unwrap();
        assert!(r.body.validate().is_ok(), "{op:?}: invalid body");
        let v = r.body.mass_properties().unwrap().volume;
        assert!((v - exact).abs() < 1e-9, "{op:?}: {v} vs exact {exact}");
        let mv = r.body.mesh_volume();
        assert!(
            (mv - v).abs() <= 2e-2 * (1.0 + v.abs()),
            "{op:?}: mesh {mv} vs mass {v}"
        );
    }
    // Ball-in-socket: carve, then re-union the SAME ball. The
    // coincident sphere laterals resolve through the on-on machinery
    // (an informational Coincident fault, never a decline).
    let sunk = boolean(&block, &ball, BoolOp::Difference, 1e-7)
        .unwrap()
        .body;
    let r = boolean(&sunk, &ball, BoolOp::Union, 1e-7).unwrap();
    assert!(r.body.validate().is_ok(), "ball-in-socket: invalid body");
    let v = r.body.mass_properties().unwrap().volume;
    let expect = 32.0 + cap;
    assert!(
        (v - expect).abs() < 1e-9,
        "ball-in-socket {v} vs exact {expect}"
    );
}

#[test]
fn sphere_socket_carve_legacy_rail() {
    // The original decline-or-exact rail kept verbatim: an Err is no
    // longer acceptable (the milestone landed), but the exactness arm
    // is the same oracle.
    let mut block = Body::new();
    block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
    let mut ball = Body::new();
    ball.sphere(
        Frame3::from_z(Vec3::new(2.0, 2.0, 1.5), Vec3::new(0., 0., 1.)).unwrap(),
        1.0,
    )
    .unwrap();
    match boolean(&block, &ball, BoolOp::Difference, 1e-7) {
        Err(e) => panic!("socket carve must assemble (task 36): {e:?}"),
        Ok(r) => {
            let v = r.body.mass_properties().unwrap().volume;
            let pi = core::f64::consts::PI;
            let cap = pi * 0.25 * 2.5 / 3.0;
            let expect = 32.0 - (4.0 * pi / 3.0 - cap);
            assert!((v - expect).abs() < 1e-9, "socket {v} vs exact {expect}");
        }
    }
}
