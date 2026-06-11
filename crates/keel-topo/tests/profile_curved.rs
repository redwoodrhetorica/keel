//! OPT-M4 curved-workload benchmark: the canonical curved bodies and
//! booleans, timed per operation with the KEEL_PROFILE stage breakdown.
//! The box oracle hit 0.9 ms/trial; this harness decides whether the
//! curved path needs its own optimization milestones before any
//! Parasolid-yardstick claim. Run:
//!   KEEL_PROFILE=1 cargo test --release -p keel-topo
//!     --test profile_curved -- --ignored --nocapture

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use std::time::Instant;

fn timed<R>(label: &str, reps: usize, mut f: impl FnMut() -> R) -> R {
    // One warm-up, then the timed reps.
    let mut out = f();
    let t = Instant::now();
    for _ in 0..reps {
        out = f();
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / reps as f64;
    eprintln!("CURVED {label}: {ms:.2} ms/op ({reps} reps)");
    out
}

#[test]
#[ignore = "profiling instrument; run with KEEL_PROFILE=1 --release --nocapture"]
fn profile_curved_workload() {
    keel_topo::profile::reset();
    let z = Vec3::new(0.0, 0.0, 1.0);

    // The canonical CAD scenario bodies.
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
    let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), z).unwrap();
    let mut drill = Body::new();
    drill.cylinder(dframe, 1.0, 2.0).unwrap();
    let pframe = Frame3::from_z(Vec3::new(2.0, 2.0, 0.0), z).unwrap();
    let mut pin = Body::new();
    pin.cylinder(pframe, 1.0, 1.0).unwrap();

    // 1. The drill difference (seam synthesis + crossing imprint +
    //    curved relocation + chordal gate).
    let holed = timed("drill difference (plate - cylinder)", 10, || {
        boolean(&plate, &drill, BoolOp::Difference, 1e-7)
            .unwrap()
            .body
    });

    // 2. The mated-pin union (coincident cylinders, on-on tables, cap
    //    tiling).
    timed("pin union (holed + pin)", 10, || {
        boolean(&holed, &pin, BoolOp::Union, 1e-7).unwrap()
    });

    // 3. Curved mass properties + chordal mesh (the judge pair).
    timed("holed mass_properties", 20, || {
        holed.mass_properties().unwrap().volume
    });
    timed("holed mesh_volume", 20, || holed.mesh_volume());

    // 4. Sphere boolean (Green-slab + sphere SSI path).
    let sframe = Frame3::from_z(Vec3::new(1.0, 1.0, 1.0), z).unwrap();
    let mut ball = Body::new();
    ball.sphere(sframe, 0.8).unwrap();
    let _ = timed("sphere difference (plate - ball)", 10, || {
        boolean(&plate, &ball, BoolOp::Difference, 1e-7).map(|r| r.body.mesh_volume())
    });

    // 5. GWN probe cost on a curved body (the classification kernel).
    timed("1000 GWN probes on holed", 5, || {
        let mut acc = 0.0;
        for i in 0..1000 {
            let t = i as f64 / 1000.0;
            acc += holed.generalized_winding_number(Vec3::new(4.0 * t, 2.0, 0.5));
        }
        acc
    });

    eprintln!("{}", keel_topo::profile::report());
}
