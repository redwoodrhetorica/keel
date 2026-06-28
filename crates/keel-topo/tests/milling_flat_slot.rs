//! Milling a flat/slot on a round shaft: `cylinder - box` where the cut plane
//! is PARALLEL to the cylinder axis. The SSI is two open axial rulings, so the
//! cylinder lateral must split into two sector bands -- the dossier-64
//! periodic-domain split (strategy A: build each band's (theta,z) loop directly
//! rather than an incremental `mef`, which doubles a periodic loop). This is the
//! dominant milling-seam decline in the workflow census (D-cut shaft, keyway,
//! slot, flat-on-boss); it now PASSES mass == mesh == closed-form truth.

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn shaft() -> Body {
    // Cylinder radius 10, height 20 (z in [-10, 10]); lateral area 2*pi*10*20.
    let mut b = Body::new();
    b.cylinder(
        Frame3::from_z(Vec3::new(0., 0., -10.), Vec3::new(0., 0., 1.)).unwrap(),
        10.0,
        20.0,
    )
    .unwrap();
    b
}

fn block(corner: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(corner, dx, dy, dz).unwrap();
    b
}

/// True volume of the shaft after a flat cut at |x| = 6: full cylinder minus the
/// circular-segment slab beyond the chord. segment_area = r^2 (a - sin a cos a),
/// a = acos(6/10); slab = segment_area * height.
fn d_shaft_volume() -> f64 {
    let r = 10.0_f64;
    let a = (6.0_f64 / r).acos();
    let seg = r * r * (a - a.sin() * a.cos());
    (std::f64::consts::PI * r * r - seg) * 20.0
}

fn assert_d_shaft(r: keel_topo::boolean::BoolResult, label: &str) {
    assert!(r.faults.is_empty(), "{label}: faults {:?}", r.faults);
    assert!(r.body.validate().is_ok(), "{label}: invalid shell");
    let m = r.body.mass_properties().expect("mass declined").volume;
    let mesh = r.body.mesh_volume();
    let truth = d_shaft_volume();
    assert!(
        (m - truth).abs() < 0.01 * truth,
        "{label}: mass {m} != truth {truth}"
    );
    // Watertight + consistent: mass == mesh within the curved-tessellation band.
    assert!(
        (m - mesh).abs() < 0.02 * (1.0 + m),
        "{label}: mass {m} vs mesh {mesh} not watertight/consistent"
    );
}

#[test]
fn front_flat_on_shaft_passes() {
    // The seam azimuth (theta = 0, +x generator) is INSIDE the removed sector.
    let tool = block(Vec3::new(6., -15., -12.), 20., 30., 24.); // keeps x <= 6
    assert_d_shaft(
        boolean(&shaft(), &tool, BoolOp::Difference, 1e-7).expect("front flat declined"),
        "front flat",
    );
}

#[test]
fn back_flat_on_shaft_passes() {
    // The seam azimuth is in the KEPT (major) sector -- the band that wraps
    // through the old seam vertex.
    let tool = block(Vec3::new(-26., -15., -12.), 20., 30., 24.); // keeps x >= -6
    assert_d_shaft(
        boolean(&shaft(), &tool, BoolOp::Difference, 1e-7).expect("back flat declined"),
        "back flat",
    );
}
