//! Regression guard for the cyl/cyl periodic-domain band split (LOG Add. 275,
//! dossier 64). The unequal-radius perpendicular cylinder/cylinder boolean
//! (A: z-cyl r1, B: x-cyl r0.6) ASSEMBLES via the band split; three of four
//! ops pass with mass == the exact bicylinder volume and a valid shell. The
//! exact intersection volume V = 8 * integral_0^r0 sqrt(r1^2 - y^2) *
//! sqrt(r0^2 - y^2) dy (perpendicular bicylinder), pinned below.
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn cyl(origin: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(origin, axis).unwrap(), r, h).unwrap();
    b
}

/// A passing op: result is Ok, the shell validates, and the sense-exact mass
/// equals `want` (the analytic truth) to a tight tolerance. The boolean's own
/// curved gate already pins mass == mesh within its band, so a returned Ok
/// with the right mass is a genuine, self-consistent PASS.
fn assert_pass(a: &Body, b: &Body, op: BoolOp, want: f64) {
    let r = boolean(a, b, op, 1e-7)
        .unwrap_or_else(|e| panic!("{op:?} declined unexpectedly: {e:?}"));
    assert!(r.body.validate().is_ok(), "{op:?}: invalid shell");
    let m = r
        .body
        .mass_properties()
        .unwrap_or_else(|e| panic!("{op:?}: mass declined: {e:?}"))
        .volume;
    assert!(
        (m - want).abs() <= 1e-2 * (1.0 + want.abs()),
        "{op:?}: mass {m} != exact {want}"
    );
}

#[test]
fn unequal_perpendicular_cyl_cyl_three_ops_pass_exact() {
    // A z-cylinder radius 1 (z in [-2, 2]); B x-cylinder radius 0.6 (x in
    // [-2, 2]) boring through it.
    let az = cyl(Vec3::new(0., 0., -2.), Vec3::new(0., 0., 1.), 1.0, 4.0);
    let bx = cyl(Vec3::new(-2., 0., 0.), Vec3::new(1., 0., 0.), 0.6, 4.0);

    // Exact perpendicular-bicylinder volumes (r1 = 1, r0 = 0.6):
    let v_inter = 2.154_962_62; //  A and B
    let v_a = std::f64::consts::PI * 1.0 * 4.0; // pi r1^2 h = 12.566...
    let v_b = std::f64::consts::PI * 0.36 * 4.0; // pi r0^2 h = 4.5239...
    let v_a_minus_b = v_a - v_inter; // 10.4114...
    let v_b_minus_a = v_b - v_inter; // 2.3689...

    assert_pass(&az, &bx, BoolOp::Intersection, v_inter);
    assert_pass(&az, &bx, BoolOp::Difference, v_a_minus_b);
    assert_pass(&bx, &az, BoolOp::Difference, v_b_minus_a);
}
