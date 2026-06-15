//! Isolate the flat-cone tessellation under-count (KL6): a very flat cone
//! meshes ~13% under its exact mass. Test flat vs normal aspect, axis-
//! aligned vs tilted, to localize the cause.
//!
//! Usage: cargo run --release -p keel-topo --example probe_flatcone
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;

fn cone(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap();
    b
}

fn report(label: &str, b: &Body, r: f64, h: f64) {
    let pi = std::f64::consts::PI;
    let exact = pi / 3.0 * r * r * h;
    let mesh = b.mesh_volume();
    let mass = b.mass_properties().map(|m| m.volume);
    println!(
        "{label:32} exact={exact:8.4} mass={mass:?} mesh={mesh:8.4}  mesh-err={:6.2}%",
        (mesh - exact) / exact * 100.0
    );
}

fn main() {
    let z = Vec3::new(0., 0., 1.);
    let tilt = Vec3::new(-1.320315027903836, -0.11413404865637267, 1.4472367325482571);
    report("normal cone r2 h3 +z", &cone(Vec3::ZERO, z, 2.0, 3.0), 2.0, 3.0);
    report("flat cone r3.13 h0.2 +z", &cone(Vec3::ZERO, z, 3.1299, 0.2), 3.1299, 0.2);
    report("flat cone r3.13 h0.2 tilt", &cone(Vec3::ZERO, tilt, 3.1299, 0.2), 3.1299, 0.2);
    report("tall cone r0.2 h3 +z", &cone(Vec3::ZERO, z, 0.2, 3.0), 0.2, 3.0);
    report("flat cone r3.13 h1.0 +z", &cone(Vec3::ZERO, z, 3.1299, 1.0), 3.1299, 1.0);
    report("flat cone r3.13 h0.5 +z", &cone(Vec3::ZERO, z, 3.1299, 0.5), 3.1299, 0.5);
    report("flat cone r3.13 h0.05 +z", &cone(Vec3::ZERO, z, 3.1299, 0.05), 3.1299, 0.05);
    // The exact failing genome position (far from world origin):
    let far = Vec3::new(1.3078230871926368, 1.5941096321500363, -5.881691043236107);
    report("flat tilt cone @far", &cone(far, tilt, 3.1299, 0.2), 3.1299, 0.2);
    report("flat tilt cone @z-5.88", &cone(Vec3::new(0., 0., -5.88), z, 3.1299, 0.2), 3.1299, 0.2);
    report("normal cone r2 h3 @far", &cone(far, z, 2.0, 3.0), 2.0, 3.0);
}
