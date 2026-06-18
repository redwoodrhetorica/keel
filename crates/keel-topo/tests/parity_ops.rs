//! Per-operation parity coverage (docs/PARITY_OPS.md): one focused test per
//! kernel modeling operation, each checking a known invariant. Built out
//! incrementally toward "test each one".
#![allow(clippy::unwrap_used)]

use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{explode, knit};

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}

#[test]
fn explode_then_knit_round_trips_a_block() {
    // Unjoin Shells + Join Sheets, as inverses: a block explodes into one sheet
    // per face (6), each a valid lamina; knit sews them back into the SAME solid
    // (volume preserved). Exercises explode (new) and knit together.
    let block = blk(Vec3::ZERO, 3.0, 4.0, 5.0);
    let v0 = block.mass_properties().unwrap().volume;

    let sheets = explode(&block);
    assert_eq!(
        sheets.len(),
        6,
        "a block must explode into 6 face sheets, got {}",
        sheets.len()
    );
    for s in &sheets {
        assert!(s.validate().is_ok(), "an exploded face sheet is invalid");
    }

    let refs: Vec<&Body> = sheets.iter().collect();
    let rejoined =
        knit(&refs, 1e-7).unwrap_or_else(|e| panic!("knit of the 6 sheets declined: {e:?}"));
    assert!(rejoined.validate().is_ok(), "knit result invalid");
    let v1 = rejoined.mass_properties().unwrap().volume;
    assert!(
        (v1 - v0).abs() < 1e-6 * (1.0 + v0),
        "knit volume {v1} != block volume {v0}"
    );
}
