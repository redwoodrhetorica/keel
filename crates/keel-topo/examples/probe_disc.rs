//! Disconnected / banded curved booleans (cone/cyl/sphere cut by slabs),
//! to see what the apex-fallback fixes now enable. Pass = Ok + faults
//! empty + validate + mass==mesh==analytic truth.
//!
//! Usage: cargo run --release -p keel-topo --example probe_disc
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn zf() -> Frame3 {
    Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap()
}
fn cone(r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(zf(), r, h).unwrap();
    b
}
fn cyl(r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(zf(), r, h).unwrap();
    b
}
fn sph(r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(zf(), r).unwrap();
    b
}
fn slab(zlo: f64, zhi: f64) -> Body {
    let mut b = Body::new();
    b.block(Vec3::new(-5.0, -5.0, zlo), 10.0, 10.0, zhi - zlo)
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
                        if pass { "PASS" } else { "off" }
                    );
                }
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4}"),
            }
        }
    }
}

fn main() {
    let pi = std::f64::consts::PI;
    // cone(r2,h3) - mid slab -> frustum + tip (disconnected). [locked test]
    run(
        "cone - slab (frustum+tip)",
        &cone(2.0, 3.0),
        &slab(0.8, 1.8),
        BoolOp::Difference,
        8.413,
    );
    // cone(r2,h3) ∩ mid slab -> middle frustum band (connected).
    run(
        "cone ∩ slab (mid band)   ",
        &cone(2.0, 3.0),
        &slab(0.8, 1.8),
        BoolOp::Intersection,
        12.566 - 7.609 - 0.804,
    );
    // cylinder(r1,h4) - mid slab -> two cylinders (disconnected).
    run(
        "cyl  - slab (two cyls)   ",
        &cyl(1.0, 4.0),
        &slab(1.0, 2.0),
        BoolOp::Difference,
        3.0 * pi,
    );
    // cylinder ∩ mid slab -> middle cylinder (connected).
    run(
        "cyl  ∩ slab (mid cyl)    ",
        &cyl(1.0, 4.0),
        &slab(1.0, 2.0),
        BoolOp::Intersection,
        pi,
    );
    // sphere(r2) - mid slab -> two caps (disconnected).
    let cap = pi * 1.5 * 1.5 * (3.0 * 2.0 - 1.5) / 3.0;
    run(
        "sph  - slab (two caps)   ",
        &sph(2.0),
        &slab(-0.5, 0.5),
        BoolOp::Difference,
        2.0 * cap,
    );
    // cone(r2,h3) - two slabs -> three pieces.
    let mut twoslabs = Body::new();
    twoslabs
        .block(Vec3::new(-5.0, -5.0, 0.6), 10.0, 10.0, 0.4)
        .unwrap();
    let cone_one_slab = boolean(&cone(2.0, 3.0), &twoslabs, BoolOp::Difference, 1e-7);
    println!(
        "cone - 1 thin slab: {:?}",
        cone_one_slab.map(|r| (
            r.body
                .mass_properties()
                .map(|m| (m.volume * 1e3).round() / 1e3),
            r.faults.len()
        ))
    );
}
