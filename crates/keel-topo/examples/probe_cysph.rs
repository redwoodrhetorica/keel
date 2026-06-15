//! Coaxial cylinder + sphere booleans (the dominant cyl/sph class), with
//! cylinder_sphere SSI + sphere carve + band fallback + multi-rim clip.
//! Sphere R=2 at origin; cylinder r=1, z in [-3,3] (rod through the ball).
//! Seam circles z = +-sqrt(3).
//!
//! Usage: cargo run --release -p keel-topo --example probe_cysph
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn sphere(r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(), r).unwrap();
    b
}
fn cyl(z0: f64, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(Vec3::new(0., 0., z0), Vec3::new(0., 0., 1.)).unwrap(), r, h)
        .unwrap();
    b
}

fn run(label: &str, a: &Body, b: &Body, op: BoolOp, truth: f64) {
    print!("{label}: truth={truth:.4}  ");
    match boolean(a, b, op, 1e-7) {
        Err(e) => println!("DECLINED {e:?}"),
        Ok(r) => {
            let mesh = r.body.mesh_volume();
            let valid = r.body.validate().is_ok();
            match r.body.mass_properties() {
                Ok(m) => {
                    let pass = r.faults.is_empty()
                        && valid
                        && (m.volume - mesh).abs() < 3e-2 * (1.0 + m.volume)
                        && (m.volume - truth).abs() < 3e-2 * (1.0 + truth);
                    println!(
                        "mass={:.4} mesh={mesh:.4} valid={valid} faults={:?} -> {}",
                        m.volume,
                        r.faults,
                        if pass { "PASS" } else { "OFF" }
                    );
                }
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4} valid={valid}"),
            }
        }
    }
}

fn main() {
    let pi = std::f64::consts::PI;
    let s3 = 3.0_f64.sqrt();
    let inter = pi * 2.0 * s3 + 2.0 * pi * ((16.0 / 3.0) - 3.0 * s3);
    let sphere_v = 32.0 * pi / 3.0;
    let cyl_v = 6.0 * pi;
    let sp = sphere(2.0);
    let cl = cyl(-3.0, 1.0, 6.0);
    println!("== coaxial sphere(R2) + cylinder(r1, z[-3,3]) ==  inter={inter:.4}");
    run("sph n cyl", &sp, &cl, BoolOp::Intersection, inter);
    run("sph u cyl", &sp, &cl, BoolOp::Union, sphere_v + cyl_v - inter);
    run("sph - cyl", &sp, &cl, BoolOp::Difference, sphere_v - inter);
    run("cyl - sph", &cl, &sp, BoolOp::Difference, cyl_v - inter);
}
