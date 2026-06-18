//! Sphere-as-TARGET carve diagnosis (the dominant cyl/sph decline linchpin).
//! Sphere R=2 at origin cut by half-spaces / slabs. Localizes where the
//! sphere carve breaks: one cut (hemisphere, pole-containing cap) -> two
//! cuts (slab: two caps / mid band).
//!
//! Usage: cargo run --release -p keel-topo --example probe_sphcut
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn sphere(r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(
        Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
        r,
    )
    .unwrap();
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
    let half = 16.0 * pi / 3.0; // hemisphere of R=2
    let full = 32.0 * pi / 3.0;
    // ONE cut: upper half-space removes z>0 -> lower hemisphere.
    println!("== one cut (half-space) ==");
    run(
        "sph - upper   (lower hemi)",
        &sphere(2.0),
        &slab(0.0, 5.0),
        BoolOp::Difference,
        half,
    );
    run(
        "sph n upper   (upper hemi)",
        &sphere(2.0),
        &slab(0.0, 5.0),
        BoolOp::Intersection,
        half,
    );
    run(
        "upper - sph   (block-bite)",
        &slab(0.0, 5.0),
        &sphere(2.0),
        BoolOp::Difference,
        10.0 * 10.0 * 5.0 - half,
    );
    // TWO cuts: a thin mid slab -> two caps (diff) / mid band (inter).
    let cap = pi * 1.5 * 1.5 * (3.0 * 2.0 - 1.5) / 3.0; // spherical cap height 1.5
    println!("== two cuts (mid slab z[-0.5,0.5]) ==");
    run(
        "sph - slab    (two caps)  ",
        &sphere(2.0),
        &slab(-0.5, 0.5),
        BoolOp::Difference,
        2.0 * cap,
    );
    run(
        "sph n slab    (mid band)  ",
        &sphere(2.0),
        &slab(-0.5, 0.5),
        BoolOp::Intersection,
        full - 2.0 * cap,
    );
}
