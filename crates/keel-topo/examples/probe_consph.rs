//! Coaxial cone + sphere booleans, exercising the cone_sphere SSI rung +
//! the sphere carve + band fallback + multi-rim clip. Cone base r=2 at
//! z=0, apex z=3 (m=-2/3). Coaxial sphere centre z=1.5, R=1 (fully inside
//! the cone z-range; no base/apex cut). Truth by numerical cross-section
//! integration.
//!
//! Usage: cargo run --release -p keel-topo --example probe_consph
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn cone(r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(), r, h).unwrap();
    b
}
fn sphere(zc: f64, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(Vec3::new(0., 0., zc), Vec3::new(0., 0., 1.)).unwrap(), r).unwrap();
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
    // cone base r=2 at z=0, apex z=3 -> r(z)=2-(2/3)z. sphere centre z=1.5, R=1.
    let cone_r = |z: f64| (2.0 - (2.0 / 3.0) * z).max(0.0);
    let sph_r = |z: f64| {
        let t = 1.0 - (z - 1.5) * (z - 1.5);
        if t > 0.0 { t.sqrt() } else { 0.0 }
    };
    // numerical inter = integral over z of pi*min(cone_r, sph_r)^2.
    let n = 200_000;
    let (zlo, zhi) = (0.5_f64, 2.5_f64);
    let dz = (zhi - zlo) / n as f64;
    let mut inter = 0.0;
    for i in 0..n {
        let z = zlo + (i as f64 + 0.5) * dz;
        let r = cone_r(z).min(sph_r(z));
        inter += pi * r * r * dz;
    }
    let cone_v = pi / 3.0 * 4.0 * 3.0; // 4pi
    let sphere_v = 4.0 / 3.0 * pi; // R=1
    let cn = cone(2.0, 3.0);
    let sp = sphere(1.5, 1.0);
    println!("== coaxial cone(r2,h3) + sphere(zc1.5,R1) ==  inter={inter:.4}");
    run("cone n sph", &cn, &sp, BoolOp::Intersection, inter);
    run("cone u sph", &cn, &sp, BoolOp::Union, cone_v + sphere_v - inter);
    run("cone - sph", &cn, &sp, BoolOp::Difference, cone_v - inter);
    run("sph - cone", &sp, &cn, BoolOp::Difference, sphere_v - inter);
}
