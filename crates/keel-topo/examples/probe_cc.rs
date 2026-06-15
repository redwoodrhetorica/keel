//! Curved-stitch diagnosis (dossier 59): canonical curved-curved crossings.
//! Two perpendicular cylinders (Steinmetz), plus sphere-sphere as a working
//! control. Reports which op assembles vs declines, and how.
//!
//! Usage: cargo run --release -p keel-topo --example probe_cc
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
fn sph(c: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r).unwrap();
    b
}

fn run(label: &str, a: &Body, b: &Body, op: BoolOp) {
    print!("{label}: ");
    match boolean(a, b, op, 1e-7) {
        Err(e) => println!("DECLINED {e:?}"),
        Ok(r) => {
            let mesh = r.body.mesh_volume();
            match r.body.mass_properties() {
                Ok(m) => println!(
                    "OK mass={:.4} mesh={mesh:.4} |d|={:.4} faults={:?}",
                    m.volume,
                    (m.volume - mesh).abs(),
                    r.faults
                ),
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4} faults={:?}", r.faults),
            }
        }
    }
}

fn main() {
    // Two unit cylinders crossing at the origin, axes +z and +x.
    let az = cyl(Vec3::new(0., 0., -2.), Vec3::new(0., 0., 1.), 1.0, 4.0);
    let ax = cyl(Vec3::new(-2., 0., 0.), Vec3::new(1., 0., 0.), 1.0, 4.0);
    println!("== perpendicular cylinders (Steinmetz) ==");
    run("cyl ∩ cyl (Steinmetz=16/3=5.333)", &az, &ax, BoolOp::Intersection);
    run("cyl ∪ cyl                       ", &az, &ax, BoolOp::Union);
    run("cyl - cyl                       ", &az, &ax, BoolOp::Difference);

    // Sphere-sphere control (known to work).
    let s1 = sph(Vec3::ZERO, 1.5);
    let s2 = sph(Vec3::new(1.5, 0., 0.), 1.5);
    println!("== overlapping spheres (control) ==");
    run("sph ∩ sph", &s1, &s2, BoolOp::Intersection);
    run("sph ∪ sph", &s1, &s2, BoolOp::Union);
    run("sph - sph", &s1, &s2, BoolOp::Difference);
}
