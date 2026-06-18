//! Root-cause A minimal repro: drill a hole in a plate, then cut a pocket
//! elsewhere on the same plate -> the second feature's imprint faults
//! ("open chain end not on boundary" / "unlocated seam component
//! (non-planar multi-cut face)"). Non-interacting features; the gap is the
//! imprint onto an already-featured body.
//! Run: KEEL_BOOL_DEBUG=1 cargo run --release -p keel-topo --example probe_multicut
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}
fn cyl(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, axis).unwrap(), r, h)
        .unwrap();
    b
}

fn main() {
    let z = Vec3::new(0., 0., 1.);
    let plate = blk(Vec3::ZERO, 13.5, 22.5, 2.226);
    let hole = cyl(Vec3::new(2.509, 1.95, -0.5), z, 1.119, 3.226);
    let r1 = boolean(&plate, &hole, BoolOp::Difference, 1e-7).unwrap();
    println!(
        "after hole: faults={:?} valid={} mass={:?}",
        r1.faults,
        r1.body.validate().is_ok(),
        r1.body.mass_properties().map(|m| m.volume)
    );
    let b2 = r1.body;
    let pocket = blk(Vec3::new(1.269, 6.363, 1.112), 1.767, 0.626, 1.713);
    println!("--- now cut a pocket elsewhere (non-interacting) ---");
    match boolean(&b2, &pocket, BoolOp::Difference, 1e-7) {
        Ok(r) => println!(
            "pocket: OK faults={:?} valid={} mass={:?}",
            r.faults,
            r.body.validate().is_ok(),
            r.body.mass_properties().map(|m| m.volume)
        ),
        Err(e) => println!("pocket: DECLINED {e:?}"),
    }

    // Control: cut the SAME pocket into a PRISTINE plate (no prior hole).
    println!("--- control: same pocket into a pristine plate ---");
    match boolean(&plate, &pocket, BoolOp::Difference, 1e-7) {
        Ok(r) => println!("control pocket: OK faults={:?}", r.faults),
        Err(e) => println!("control pocket: DECLINED {e:?}"),
    }
}
