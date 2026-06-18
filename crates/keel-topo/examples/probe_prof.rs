//! Isolated stage breakdown for ONE op type, to localize a perf regression.
//! Run: KEEL_PROFILE=1 cargo run --release -p keel-topo --example probe_prof
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use std::time::Instant;

fn main() {
    let z = Vec3::new(0.0, 0.0, 1.0);
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
    let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), z).unwrap();
    let mut drill = Body::new();
    drill.cylinder(dframe, 1.0, 2.0).unwrap();

    // warm up
    let _ = boolean(&plate, &drill, BoolOp::Difference, 1e-7);
    keel_topo::profile::reset();
    let reps = 100;
    let t = Instant::now();
    for _ in 0..reps {
        let _ = boolean(&plate, &drill, BoolOp::Difference, 1e-7).unwrap();
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / reps as f64;
    eprintln!("drill difference: {ms:.3} ms/op ({reps} reps)");
    eprintln!("{}", keel_topo::profile::report());

    // Where does mass_properties time go? Box vs cylinder vs holed result.
    let result = boolean(&plate, &drill, BoolOp::Difference, 1e-7)
        .unwrap()
        .body;
    let mass_ms = |lbl: &str, b: &Body| {
        let _ = b.mass_properties();
        let t = Instant::now();
        let r = 200;
        for _ in 0..r {
            let _ = b.mass_properties();
        }
        eprintln!(
            "  mass {lbl}: {:.4} ms/call",
            t.elapsed().as_secs_f64() * 1000.0 / r as f64
        );
    };
    mass_ms("plate(box)", &plate);
    mass_ms("drill(cyl)", &drill);
    mass_ms("holed result", &result);
    let tess_ms = |lbl: &str, b: &Body| {
        let t = Instant::now();
        let r = 200;
        for _ in 0..r {
            let _ = b.facets(None);
        }
        eprintln!(
            "  facets {lbl}: {:.4} ms/call",
            t.elapsed().as_secs_f64() * 1000.0 / r as f64
        );
    };
    tess_ms("plate(box)", &plate);
    tess_ms("holed result", &result);
}
