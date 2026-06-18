//! Trace where overlapping cone booleans decline: SSI (no seam), stitch
//! (UnassemblableSeam / invalid), or mass (mass != mesh). Grounds the next
//! milestone step. Cases: cone minus a perpendicular slab (circle cuts,
//! should be tractable), cone minus a side-poking block (hyperbola cuts),
//! cone minus an oblique cut.
//!
//! Usage: cargo run --release -p keel-topo --example probe_oc
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn cone(r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(
        Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
        r,
        h,
    )
    .unwrap();
    b
}
fn block(p: [f64; 3], d: [f64; 3]) -> Body {
    let mut b = Body::new();
    b.block(Vec3::new(p[0], p[1], p[2]), d[0], d[1], d[2])
        .unwrap();
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
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4}"),
            }
        }
    }
}

fn main() {
    let cn = cone(2.0, 3.0);
    // (1) perpendicular slab cut: two circle seams (z=0.8 and z=1.8 planes).
    let slab = block([-3.0, -3.0, 0.8], [6.0, 6.0, 1.0]);
    run(
        "cone - perp slab (circle cuts)",
        &cn,
        &slab,
        BoolOp::Difference,
    );
    // (2) side-poking block: axis-parallel faces -> hyperbola cuts.
    let side = block([0.8, -0.7, 0.6], [3.0, 1.4, 1.6]);
    run(
        "cone - side block (hyperbola)",
        &cn,
        &side,
        BoolOp::Difference,
    );
    run("cone I side block       ", &cn, &side, BoolOp::Intersection);
    run("cone U side block       ", &cn, &side, BoolOp::Union);
    // (3) corner block clipping the tip region.
    let corner = block([-0.5, -0.5, 1.5], [3.0, 3.0, 3.0]);
    run("cone - corner block     ", &cn, &corner, BoolOp::Difference);
}
