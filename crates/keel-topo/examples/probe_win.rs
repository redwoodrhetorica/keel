//! Minimal cyl/sphere WINDOW trace (gate narrowed to allow windows).
//! Run with KEEL_BOOL_DEBUG=1 to see the classify dump.
//! Usage: KEEL_BOOL_DEBUG=1 cargo run --release -p keel-topo --example probe_win
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn main() {
    let mut a = Body::new();
    a.cylinder(Frame3::from_z(Vec3::new(0., 0., -3.), Vec3::new(0., 0., 1.)).unwrap(), 1.0, 6.0)
        .unwrap();
    let mut b = Body::new();
    b.sphere(Frame3::from_z(Vec3::new(0., 1.5, 0.), Vec3::new(0., 0., 1.)).unwrap(), 1.2)
        .unwrap();
    for (lbl, x, y, op) in [
        ("cyl - sph", &a, &b, BoolOp::Difference),
        ("cyl n sph", &a, &b, BoolOp::Intersection),
    ] {
        println!("==== {lbl} ====");
        match boolean(x, y, op, 1e-7) {
            Err(e) => println!("{lbl}: DECLINED {e:?}"),
            Ok(r) => println!(
                "{lbl}: faults={:?} mass={:?} mesh={:.4}",
                r.faults,
                r.body.mass_properties().map(|m| m.volume),
                r.body.mesh_volume()
            ),
        }
    }
}
