//! The WASM spike (task 21): minimal extern-C exports proving the
//! kernel runs in a WebAssembly host and measuring the wasm-vs-native
//! performance ratio on the canonical cases. No wasm-bindgen: the
//! host calls these directly and times them with performance.now().
//! The real worker-protocol surface (task 28) builds on this.

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::render::WorkerMesh;
use std::cell::RefCell;

thread_local! {
    /// The last-built worker mesh; buffer pointers below view into it.
    /// WASM is single-threaded, and the host reads the buffers before
    /// the next build call, so the borrow discipline is structural.
    static MESH: RefCell<Option<WorkerMesh>> = const { RefCell::new(None) };
    /// face_groups flattened to (id, start, count) u32 triples.
    static GROUPS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// The last op's report: newline-joined warnings + the confidence
    /// triple (the BuildReport mapping, task 22).
    static REPORT_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static REPORT_NUMS: RefCell<(i32, i32, f64)> = const { RefCell::new((0, 1, 0.0)) };
}

fn stage_report(rep: &keel_topo::report::OpReport) {
    REPORT_TEXT.with(|t| *t.borrow_mut() = rep.warnings.join("\n"));
    REPORT_NUMS
        .with(|n| *n.borrow_mut() = (rep.salvaged as i32, rep.tier as i32, rep.achieved_tolerance));
}

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

/// Build the demo scene (drilled plate + mated pin union) and stage
/// its worker mesh for the buffer accessors below. Returns 0 on
/// success, negative stage codes on failure: the wire protocol the
/// real op-tree build will follow (task 28).
#[unsafe(no_mangle)]
pub extern "C" fn demo_mesh_build() -> i32 {
    let z = Vec3::new(0.0, 0.0, 1.0);
    let mut plate = Body::new();
    if plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).is_err() {
        return -1;
    }
    let Ok(dframe) = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), z) else {
        return -2;
    };
    let mut drill = Body::new();
    if drill.cylinder(dframe, 1.0, 2.0).is_err() {
        return -3;
    }
    let Ok(holed) = boolean(&plate, &drill, BoolOp::Difference, 1e-7) else {
        return -4;
    };
    let mesh = holed.body.worker_mesh();
    GROUPS.with(|g| {
        let mut g = g.borrow_mut();
        g.clear();
        for &(id, start, count) in &mesh.face_groups {
            g.extend_from_slice(&[id as u32, start, count]);
        }
    });
    MESH.with(|m| *m.borrow_mut() = Some(mesh));
    0
}

macro_rules! mesh_accessor {
    ($ptr:ident, $len:ident, $field:ident, $ty:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $ptr() -> *const $ty {
            MESH.with(|m| {
                m.borrow()
                    .as_ref()
                    .map(|x| x.$field.as_ptr())
                    .unwrap_or(core::ptr::null())
            })
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $len() -> usize {
            MESH.with(|m| m.borrow().as_ref().map(|x| x.$field.len()).unwrap_or(0))
        }
    };
}
mesh_accessor!(mesh_positions_ptr, mesh_positions_len, positions, f32);
mesh_accessor!(mesh_normals_ptr, mesh_normals_len, normals, f32);
mesh_accessor!(mesh_indices_ptr, mesh_indices_len, indices, u32);
mesh_accessor!(mesh_lines_ptr, mesh_lines_len, lines, f32);

#[unsafe(no_mangle)]
pub extern "C" fn mesh_groups_ptr() -> *const u32 {
    GROUPS.with(|g| g.borrow().as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn mesh_groups_len() -> usize {
    GROUPS.with(|g| g.borrow().len())
}

/// Tolerant demo (task 22): the clearance pin (1e-5 radial gap) is
/// salvaged by the tolerant tier; the OpReport crosses the worker
/// boundary via the accessors below. Returns the union volume
/// (expected exactly 16.0) or negative stage codes.
#[unsafe(no_mangle)]
pub extern "C" fn demo_tolerant_build() -> f64 {
    use keel_topo::boolean::boolean_tolerant;
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
    if pin.cylinder(pframe, 1.0 - 1e-5, 1.0).is_err() {
        return -6.0;
    }
    let Ok((res, conf)) = boolean_tolerant(&holed.body, &pin, BoolOp::Union, 1e-7, 1e-4) else {
        return -7.0;
    };
    stage_report(&keel_topo::report::OpReport::tolerant(&res, &conf));
    res.body.mass_properties().map(|m| m.volume).unwrap_or(-8.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn report_warnings_ptr() -> *const u8 {
    REPORT_TEXT.with(|t| t.borrow().as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn report_warnings_len() -> usize {
    REPORT_TEXT.with(|t| t.borrow().len())
}
#[unsafe(no_mangle)]
pub extern "C" fn report_salvaged() -> i32 {
    REPORT_NUMS.with(|n| n.borrow().0)
}
#[unsafe(no_mangle)]
pub extern "C" fn report_tier() -> i32 {
    REPORT_NUMS.with(|n| n.borrow().1)
}
#[unsafe(no_mangle)]
pub extern "C" fn report_achieved_tolerance() -> f64 {
    REPORT_NUMS.with(|n| n.borrow().2)
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
