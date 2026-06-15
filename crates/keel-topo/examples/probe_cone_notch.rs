//! Minimal DISJOINT unions to isolate the "unmatched coedge:
//! shell-closure invariant violated" decline that dominates the
//! block/cone union frontier. Axis-aligned, far-separated operands.
//! Controls: block-cyl and block-sph (is it cone-specific?).
//!
//! Usage: cargo run --release -p keel-topo --example probe_cone_notch
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn z() -> Vec3 {
    Vec3::new(0.0, 0.0, 1.0)
}

fn cone(at: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(Frame3::from_z(at, z()).unwrap(), r, h).unwrap();
    b
}
fn cyl(at: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(at, z()).unwrap(), r, h).unwrap();
    b
}
fn sph(at: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(at, z()).unwrap(), r).unwrap();
    b
}
fn block(at: Vec3, d: f64) -> Body {
    let mut b = Body::new();
    b.block(at, d, d, d).unwrap();
    b
}

fn run(label: &str, a: &Body, b: &Body) {
    print!("{label}: ");
    match boolean(a, b, BoolOp::Union, 1e-7) {
        Err(e) => println!("DECLINED {e:?}"),
        Ok(r) => {
            let mesh = r.body.mesh_volume();
            let c = r.body.counts();
            let valid = r.body.validate().is_ok();
            match r.body.mass_properties() {
                Ok(m) => println!(
                    "OK mass={:.4} mesh={mesh:.4} valid={valid} regions={} shells={} genus={} faults={:?}",
                    m.volume, c.regions, c.shells, c.genus, r.faults
                ),
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4} valid={valid}"),
            }
        }
    }
}

fn main() {
    let far = Vec3::new(8.0, 0.0, 0.0);
    let cn = cone(Vec3::ZERO, 1.0, 2.0);
    let cy = cyl(Vec3::ZERO, 1.0, 2.0);
    let sp = sph(Vec3::ZERO, 1.0);
    let bl = block(far, 1.0);

    run("disjoint block U cone", &bl, &cn);
    run("disjoint cone U block", &cn, &bl);
    run("disjoint block U cyl ", &bl, &cy);
    run("disjoint block U sph ", &bl, &sp);
    run("disjoint cone U cone ", &cn, &cone(far, 1.0, 2.0));

    // The two explorer FAILs (op=Difference, sph/sph): characterize them
    // directly to confirm they are the sphere-split trap, not the union fix.
    let s = |p: [f64; 3], r: f64, ax: [f64; 3]| {
        let mut b = Body::new();
        b.sphere(
            Frame3::from_z(Vec3::new(p[0], p[1], p[2]), Vec3::new(ax[0], ax[1], ax[2])).unwrap(),
            r,
        )
        .unwrap();
        b
    };
    for (lbl, a, b) in [
        (
            "sph - sph #1",
            s([2.41, 0.06, 2.61], 3.65, [0.42, 0.65, -0.17]),
            s([-0.36, -1.92, 0.14], 0.7, [1.07, -1.27, 1.28]),
        ),
        (
            "sph - sph #2",
            s([-2.13, 4.38, -2.81], 3.46, [0.6, 0.86, -0.15]),
            s([-1.87, 0.78, 1.46], 2.27, [1.27, 0.44, -2.17]),
        ),
    ] {
        print!("{lbl} (Difference): ");
        match boolean(&a, &b, BoolOp::Difference, 1e-7) {
            Err(e) => println!("DECLINED {e:?}"),
            Ok(r) => {
                let mesh = r.body.mesh_volume();
                match r.body.mass_properties() {
                    Ok(m) => println!("OK mass={:.4} mesh={mesh:.4}", m.volume),
                    Err(e) => println!("mass DECLINED {e:?}  mesh={mesh:.4} (MALFORMED: mass declines, mesh lies)"),
                }
            }
        }
    }
}
