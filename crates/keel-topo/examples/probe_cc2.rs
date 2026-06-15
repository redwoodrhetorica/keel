//! Hard cyl-cyl configs to reproduce the UnassemblableSeam decline class.
//! cyl-cyl SSI exists (per-ruling branch field); Steinmetz (equal-radius
//! perpendicular) passes. Probe unequal-radius / skew / tangent.
//!
//! Usage: cargo run --release -p keel-topo --example probe_cc2
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

fn run(label: &str, a: &Body, b: &Body, op: BoolOp) {
    print!("{label}: ");
    match boolean(a, b, op, 1e-7) {
        Err(e) => println!("DECLINED {e:?}"),
        Ok(r) => {
            let mesh = r.body.mesh_volume();
            match r.body.mass_properties() {
                Ok(m) => println!(
                    "mass={:.4} mesh={mesh:.4} |d|={:.4} valid={} faults={:?}",
                    m.volume,
                    (m.volume - mesh).abs(),
                    r.body.validate().is_ok(),
                    r.faults
                ),
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4} faults={:?}", r.faults),
            }
        }
    }
}

fn main() {
    // 1. Perpendicular UNEQUAL radii: small cyl B (r=0.6, x-axis) bores
    //    through large cyl A (r=1, z-axis).
    let az = cyl(Vec3::new(0., 0., -2.), Vec3::new(0., 0., 1.), 1.0, 4.0);
    let bx = cyl(Vec3::new(-2., 0., 0.), Vec3::new(1., 0., 0.), 0.6, 4.0);
    println!("== perpendicular UNEQUAL (r1 x r0.6) ==");
    run("A n B", &az, &bx, BoolOp::Intersection);
    run("A u B", &az, &bx, BoolOp::Union);
    run("A - B", &az, &bx, BoolOp::Difference);
    run("B - A", &bx, &az, BoolOp::Difference);

    // 2. SKEW axes (B offset in z so axes do not meet), equal radii.
    let bsk = cyl(Vec3::new(-2., 0., 0.4), Vec3::new(1., 0., 0.), 0.6, 4.0);
    println!("== skew (B offset z=0.4) ==");
    run("A n B", &az, &bsk, BoolOp::Intersection);
    run("A - B", &az, &bsk, BoolOp::Difference);

    // 3. Tilted axis (B at 45 deg in the xz-plane).
    let bti = cyl(Vec3::new(-2., 0., -2.), Vec3::new(1., 0., 1.), 0.6, 6.0);
    println!("== tilted (B axis 45deg xz) ==");
    run("A n B", &az, &bti, BoolOp::Intersection);
    run("A - B", &az, &bti, BoolOp::Difference);
}
