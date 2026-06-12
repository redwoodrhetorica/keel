//! The WASM spike (task 21): minimal extern-C exports proving the
//! kernel runs in a WebAssembly host and measuring the wasm-vs-native
//! performance ratio on the canonical cases. No wasm-bindgen: the
//! host calls these directly and times them with performance.now().
//! The real worker-protocol surface (task 28) builds on this.

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

/// The canonical curved scenario: plate drilled, pin unioned back.
/// Returns the union volume (expected exactly 16.0); negative values
/// encode failure stages for host-side assertion.
#[unsafe(no_mangle)]
pub extern "C" fn pin_in_hole() -> f64 {
    let z = Vec3::new(0.0, 0.0, 1.0);
    let mut plate = Body::new();
    if plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).is_err() {
        return -1.0;
    }
    let Ok(dframe) = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), z) else {
        return -2.0;
    };
    let mut drill = Body::new();
    if drill.cylinder(dframe, 1.0, 2.0).is_err() {
        return -3.0;
    }
    let Ok(holed) = boolean(&plate, &drill, BoolOp::Difference, 1e-7) else {
        return -4.0;
    };
    let Ok(pframe) = Frame3::from_z(Vec3::new(2.0, 2.0, 0.0), z) else {
        return -5.0;
    };
    let mut pin = Body::new();
    if pin.cylinder(pframe, 1.0, 1.0).is_err() {
        return -6.0;
    }
    let Ok(u) = boolean(&holed.body, &pin, BoolOp::Union, 1e-7) else {
        return -7.0;
    };
    u.body.mass_properties().map(|m| m.volume).unwrap_or(-8.0)
}

/// One box-pair boolean (the oracle's bread and butter). Returns the
/// union volume of two unit-ish boxes (expected 1.0 + 1.0 - 0.125).
#[unsafe(no_mangle)]
pub extern "C" fn box_union() -> f64 {
    let mut a = Body::new();
    if a.block(Vec3::ZERO, 1.0, 1.0, 1.0).is_err() {
        return -1.0;
    }
    let mut b = Body::new();
    if b.block(Vec3::new(0.5, 0.5, 0.5), 1.0, 1.0, 1.0).is_err() {
        return -2.0;
    }
    match boolean(&a, &b, BoolOp::Union, 1e-7) {
        Ok(r) => r.body.mass_properties().map(|m| m.volume).unwrap_or(-4.0),
        Err(_) => -3.0,
    }
}
