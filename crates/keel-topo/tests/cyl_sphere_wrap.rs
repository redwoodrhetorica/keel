//! cyl/sphere WRAP: an off-axis rod SWALLOWED by a sphere (delta + r_cyl <= R)
//! so the SSI is two encircling NURBS loops and the sphere is cut into a band.
//! The non-coaxial NURBS-rim analogue of the coaxial rod-through-ball (Add 267),
//! and the dominant remaining curved-boolean decline class. Now PASSES all three
//! ops: mass == mesh == truth, backstopped by the independent
//! `quadric_sphere_op_volume` 1D-integral oracle (a wrong assembly fails it and
//! DECLINES, so the class is safe to assemble un-gated).

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

/// Rod radius 1 along +z, offset x=1, z in [-4,4]; ball radius 3 at the origin
/// with a NON-z sphere frame (so the rod poke-out lands near the sphere equator,
/// not the parametric pole). delta = 1, delta + r = 2 <= R = 3 -> WRAP.
fn rod_and_ball() -> (Body, Body) {
    let mut rod = Body::new();
    rod.cylinder(
        Frame3::from_z(Vec3::new(1., 0., -4.), Vec3::new(0., 0., 1.)).unwrap(),
        1.0,
        8.0,
    )
    .unwrap();
    let mut ball = Body::new();
    ball.sphere(Frame3::from_z(Vec3::ZERO, Vec3::new(1., 0., 0.)).unwrap(), 3.0)
        .unwrap();
    (rod, ball)
}

fn check(op: BoolOp, want: f64, label: &str) {
    let (a, b) = rod_and_ball();
    let r = boolean(&a, &b, op, 1e-7).unwrap_or_else(|e| panic!("{label} declined: {e:?}"));
    assert!(r.faults.is_empty(), "{label} faults: {:?}", r.faults);
    assert!(r.body.validate().is_ok(), "{label} invalid shell");
    let m = r.body.mass_properties().expect("mass declined").volume;
    let mesh = r.body.mesh_volume();
    // mass within 2% of the closed-form truth (the gate already requires it via
    // the tight oracle); mass == mesh within the curved-tessellation band.
    assert!((m - want).abs() < 0.02 * want, "{label}: mass {m} != truth ~{want}");
    assert!(
        (m - mesh).abs() < 0.025 * (1.0 + m),
        "{label}: mass {m} vs mesh {mesh} not watertight/consistent"
    );
}

#[test]
fn wrap_intersection_passes() {
    check(BoolOp::Intersection, 17.17, "I cyl/sph");
}

#[test]
fn wrap_difference_passes() {
    check(BoolOp::Difference, 7.95, "D cyl/sph");
}

#[test]
fn wrap_union_passes() {
    check(BoolOp::Union, 121.0, "U cyl/sph");
}
