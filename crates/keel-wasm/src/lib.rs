//! keel-wasm: the COMPLETE fieldforge binding (task 28), not the spike.
//!
//! Implements the exact extern-C ABI in `FIELDFORGE_BINDING.md`. No
//! wasm-bindgen: the host reads linear memory directly via the `*_ptr()`/
//! `*_len()` accessors. Bodies never cross the boundary -- they live in a
//! thread-local slab and the host holds an `i32` HANDLE. Every body-producing
//! call returns the handle (>= 0) or a NEGATIVE decline code; on decline the
//! inputs are untouched and no body is created (DECLINE-never-WRONG). All
//! coordinates/lengths are f64; rotations are DEGREES; booleans use tol 1e-7.

use keel_geom::surface::{Frame3, Plane3};
use keel_math::transform::Transform3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::AnyKey;
use keel_topo::render::WorkerMesh;
use std::cell::RefCell;

const TOL: f64 = 1e-7;

thread_local! {
    /// Body slab. A HANDLE is an index; a freed slot holds None and is reused.
    static BODIES: RefCell<Vec<Option<Body>>> = const { RefCell::new(Vec::new()) };
    /// The last meshed body; the buffer accessors view into it. WASM is
    /// single-threaded and the host reads the buffers before the next call,
    /// so the borrow discipline is structural.
    static MESH: RefCell<Option<WorkerMesh>> = const { RefCell::new(None) };
    /// FACE groups flattened to (id, start, count) u32 triples.
    static GROUPS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// EDGE groups flattened to (id, start, count) u32 triples.
    static EDGE_GROUPS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// The last STEP-export text; `step_buffer_ptr/len` view into it.
    static STEP_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
}

// ---- handle helpers -------------------------------------------------------

/// Insert a body, returning its handle (reusing a freed slot when possible).
fn put(body: Body) -> i32 {
    BODIES.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(i) = s.iter().position(|x| x.is_none()) {
            s[i] = Some(body);
            i as i32
        } else {
            s.push(Some(body));
            (s.len() - 1) as i32
        }
    })
}

/// Read a body by handle (immutable). Returns None for a stale/negative handle.
/// The borrow is released before the caller `put`s any produced body.
fn read<R>(h: i32, f: impl FnOnce(&Body) -> R) -> Option<R> {
    if h < 0 {
        return None;
    }
    BODIES.with(|s| s.borrow().get(h as usize).and_then(|o| o.as_ref()).map(f))
}

/// Standard "produced a new body or declined" reducer.
#[inline]
fn finish(r: Option<Result<Body, keel_topo::TopoError>>) -> i32 {
    match r {
        Some(Ok(b)) => put(b),
        _ => -1,
    }
}

// ---- memory (host writes variable-length inputs here) ---------------------

/// 8-byte-aligned allocation (backed by `Vec<f64>`) so the host can lay down
/// `Float64Array` point data. Returns a pointer into linear memory.
#[unsafe(no_mangle)]
pub extern "C" fn kw_alloc(len: usize) -> *mut u8 {
    let n = len.div_ceil(8).max(1);
    let mut v = Vec::<f64>::with_capacity(n);
    let ptr = v.as_mut_ptr() as *mut u8;
    core::mem::forget(v);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    let n = len.div_ceil(8).max(1);
    unsafe {
        drop(Vec::<f64>::from_raw_parts(ptr as *mut f64, 0, n));
    }
}

// ---- primitives (built at LOCAL origin; host applies placement) -----------

#[unsafe(no_mangle)]
pub extern "C" fn kw_block(ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64) -> i32 {
    let mut b = Body::new();
    match b.block(Vec3::new(ox, oy, oz), dx, dy, dz) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_cylinder(
    ox: f64, oy: f64, oz: f64, ax: f64, ay: f64, az: f64, radius: f64, h: f64,
) -> i32 {
    let Ok(frame) = Frame3::from_z(Vec3::new(ox, oy, oz), Vec3::new(ax, ay, az)) else {
        return -2;
    };
    let mut b = Body::new();
    match b.cylinder(frame, radius, h) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_cone(
    ox: f64, oy: f64, oz: f64, ax: f64, ay: f64, az: f64, radius: f64, h: f64,
) -> i32 {
    let Ok(frame) = Frame3::from_z(Vec3::new(ox, oy, oz), Vec3::new(ax, ay, az)) else {
        return -2;
    };
    let mut b = Body::new();
    match b.cone(frame, radius, h) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> i32 {
    let Ok(frame) = Frame3::from_z(Vec3::new(cx, cy, cz), Vec3::new(0.0, 0.0, 1.0)) else {
        return -2;
    };
    let mut b = Body::new();
    match b.sphere(frame, radius) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_torus(
    cx: f64, cy: f64, cz: f64, ax: f64, ay: f64, az: f64, major: f64, minor: f64,
) -> i32 {
    let Ok(frame) = Frame3::from_z(Vec3::new(cx, cy, cz), Vec3::new(ax, ay, az)) else {
        return -2;
    };
    let mut b = Body::new();
    match b.torus(frame, major, minor) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

// ---- placement / transform ------------------------------------------------

/// Scale about the origin (NURBS-exact `Body::scaled`), then rigid rotate
/// X,Y,Z (degrees, about origin) + translate via `Body::transformed`.
#[unsafe(no_mangle)]
pub extern "C" fn kw_place(
    h: i32, scale: f64, rx_deg: f64, ry_deg: f64, rz_deg: f64, px: f64, py: f64, pz: f64,
) -> i32 {
    finish(read(h, |b| -> Result<Body, keel_topo::TopoError> {
        let scaled = if (scale - 1.0).abs() > 1e-15 {
            b.scaled(Vec3::ZERO, scale)?
        } else {
            b.clone()
        };
        let rx = Transform3::from_rotation(Vec3::new(1.0, 0.0, 0.0), rx_deg.to_radians())
            .unwrap_or(Transform3::IDENTITY);
        let ry = Transform3::from_rotation(Vec3::new(0.0, 1.0, 0.0), ry_deg.to_radians())
            .unwrap_or(Transform3::IDENTITY);
        let rz = Transform3::from_rotation(Vec3::new(0.0, 0.0, 1.0), rz_deg.to_radians())
            .unwrap_or(Transform3::IDENTITY);
        let t = rx
            .then(ry)
            .then(rz)
            .then(Transform3::from_translation(Vec3::new(px, py, pz)));
        scaled.transformed(&t)
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_mirror(h: i32, px: f64, py: f64, pz: f64, nx: f64, ny: f64, nz: f64) -> i32 {
    finish(read(h, |b| {
        b.mirrored(Vec3::new(px, py, pz), Vec3::new(nx, ny, nz))
    }))
}

// ---- profile ops (host writes a flat f64 point buffer) --------------------

/// Extrude a closed world loop (n_pts * 3 f64) along `dir`.
#[unsafe(no_mangle)]
pub extern "C" fn kw_prism(ptr: *const f64, n_pts: usize, dx: f64, dy: f64, dz: f64) -> i32 {
    if ptr.is_null() || n_pts < 3 {
        return -2;
    }
    let raw = unsafe { core::slice::from_raw_parts(ptr, n_pts * 3) };
    let profile: Vec<Vec3> = raw.chunks_exact(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    let mut b = Body::new();
    match b.prism(&profile, Vec3::new(dx, dy, dz)) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

/// Lathe a (radius,height) meridian (n_pairs * 2 f64) about the axis through
/// origin. angle_deg >= 360 -> full `revolve`, else `revolve_partial`.
#[unsafe(no_mangle)]
pub extern "C" fn kw_revolve(
    ptr: *const f64, n_pairs: usize, ax: f64, ay: f64, az: f64, ox: f64, oy: f64, oz: f64,
    angle_deg: f64,
) -> i32 {
    if ptr.is_null() || n_pairs < 3 {
        return -2;
    }
    let raw = unsafe { core::slice::from_raw_parts(ptr, n_pairs * 2) };
    let profile: Vec<(f64, f64)> = raw.chunks_exact(2).map(|c| (c[0], c[1])).collect();
    let Ok(frame) = Frame3::from_z(Vec3::new(ox, oy, oz), Vec3::new(ax, ay, az)) else {
        return -2;
    };
    let mut b = Body::new();
    let res = if angle_deg >= 360.0 {
        b.revolve(frame, &profile)
    } else {
        b.revolve_partial(frame, &profile, angle_deg.to_radians())
    };
    match res {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

/// Loft two equal-count world loops (each point 3 f64).
#[unsafe(no_mangle)]
pub extern "C" fn kw_loft(bot: *const f64, n_bot: usize, top: *const f64, n_top: usize) -> i32 {
    if bot.is_null() || top.is_null() || n_bot < 3 || n_top != n_bot {
        return -2;
    }
    let br = unsafe { core::slice::from_raw_parts(bot, n_bot * 3) };
    let tr = unsafe { core::slice::from_raw_parts(top, n_top * 3) };
    let bottom: Vec<Vec3> = br.chunks_exact(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    let topl: Vec<Vec3> = tr.chunks_exact(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    let mut b = Body::new();
    match b.loft(&bottom, &topl) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

/// Exact frustum between two coaxial circles.
#[unsafe(no_mangle)]
pub extern "C" fn kw_loft_circles(
    ax: f64, ay: f64, az: f64, ox: f64, oy: f64, oz: f64, r0: f64, r1: f64, h: f64,
) -> i32 {
    let Ok(frame) = Frame3::from_z(Vec3::new(ox, oy, oz), Vec3::new(ax, ay, az)) else {
        return -2;
    };
    let mut b = Body::new();
    match b.loft_circles(frame, r0, r1, h) {
        Ok(_) => put(b),
        Err(_) => -1,
    }
}

// ---- booleans -------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn kw_boolean(ha: i32, hb: i32, op: i32) -> i32 {
    let op = match op {
        0 => BoolOp::Union,
        1 => BoolOp::Difference,
        2 => BoolOp::Intersection,
        _ => return -2,
    };
    if ha < 0 || hb < 0 {
        return -2;
    }
    let result = BODIES.with(|s| {
        let s = s.borrow();
        let a = s.get(ha as usize).and_then(|o| o.as_ref());
        let b = s.get(hb as usize).and_then(|o| o.as_ref());
        match (a, b) {
            (Some(a), Some(b)) => Some(boolean(a, b, op, TOL)),
            _ => None,
        }
    });
    match result {
        Some(Ok(r)) => put(r.body),
        Some(Err(_)) => -1,
        None => -2,
    }
}

// ---- edge ops by 3D point (pick crux) -------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn kw_fillet_nearest(h: i32, x: f64, y: f64, z: f64, radius: f64, tol: f64) -> i32 {
    let p = Vec3::new(x, y, z);
    finish(
        read(h, |b| -> Option<Result<Body, keel_topo::TopoError>> {
            let e = b.nearest_edge(p)?;
            let (_, dist) = b.closest_point_on_edge(e, p)?;
            if dist > tol {
                return None; // pick miss -> decline, original untouched
            }
            Some(b.fillet_edge(e, radius))
        })
        .flatten(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_chamfer_nearest(
    h: i32, x: f64, y: f64, z: f64, distance: f64, tol: f64,
) -> i32 {
    let p = Vec3::new(x, y, z);
    finish(
        read(h, |b| -> Option<Result<Body, keel_topo::TopoError>> {
            let e = b.nearest_edge(p)?;
            let (_, dist) = b.closest_point_on_edge(e, p)?;
            if dist > tol {
                return None;
            }
            Some(b.chamfer_edge(e, distance))
        })
        .flatten(),
    )
}

// ---- face ops by 3D point (clone-then-mutate; a decline keeps the original) ----

#[unsafe(no_mangle)]
pub extern "C" fn kw_push_face(h: i32, x: f64, y: f64, z: f64, d: f64, tol: f64) -> i32 {
    let p = Vec3::new(x, y, z);
    finish(
        read(h, |b| -> Option<Result<Body, keel_topo::TopoError>> {
            let fk = b.pick_face(p, tol)?;
            let mut c = b.clone();
            Some(c.push_face(fk, d).map(|_| c))
        })
        .flatten(),
    )
}

/// Draft a picked face about the neutral plane at the body base (min Z, +Z normal).
#[unsafe(no_mangle)]
pub extern "C" fn kw_draft_face(h: i32, x: f64, y: f64, z: f64, angle_deg: f64, tol: f64) -> i32 {
    let p = Vec3::new(x, y, z);
    finish(
        read(h, |b| -> Option<Result<Body, keel_topo::TopoError>> {
            let fk = b.pick_face(p, tol)?;
            let minz = b
                .worker_mesh()
                .positions
                .iter()
                .skip(2)
                .step_by(3)
                .copied()
                .fold(f64::INFINITY, |a, z| a.min(z as f64));
            if !minz.is_finite() {
                return None;
            }
            let frame = Frame3::from_z(Vec3::new(0.0, 0.0, minz), Vec3::new(0.0, 0.0, 1.0)).ok()?;
            let neutral = Plane3::new(frame);
            let mut c = b.clone();
            Some(c.draft_face(fk, &neutral, angle_deg.to_radians()).map(|_| c))
        })
        .flatten(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_hollow(h: i32, t: f64) -> i32 {
    finish(read(h, |b| b.hollow(t)))
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_hollow_pierce(h: i32, x: f64, y: f64, z: f64, t: f64, tol: f64) -> i32 {
    let p = Vec3::new(x, y, z);
    finish(
        read(h, |b| -> Option<Result<Body, keel_topo::TopoError>> {
            let fk = b.pick_face(p, tol)?;
            Some(b.hollow_pierce(t, |f| f == fk))
        })
        .flatten(),
    )
}

// ---- meshing + interrogation ----------------------------------------------

/// Stage `worker_mesh_tol(chord)` into the thread-locals. 0 ok, <0 fail. The
/// host calls the buffer accessors below immediately after (read before the
/// next call).
#[unsafe(no_mangle)]
pub extern "C" fn kw_mesh(h: i32, chord: f64) -> i32 {
    let mesh = read(h, |b| b.worker_mesh_tol(chord));
    let Some(mesh) = mesh else {
        return -1;
    };
    GROUPS.with(|g| {
        let mut g = g.borrow_mut();
        g.clear();
        for &(id, start, count) in &mesh.face_groups {
            g.extend_from_slice(&[id as u32, start, count]);
        }
    });
    EDGE_GROUPS.with(|e| {
        let mut e = e.borrow_mut();
        e.clear();
        for &(id, start, count) in &mesh.edge_groups {
            e.extend_from_slice(&[id as u32, start, count]);
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
#[unsafe(no_mangle)]
pub extern "C" fn mesh_edge_groups_ptr() -> *const u32 {
    EDGE_GROUPS.with(|e| e.borrow().as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn mesh_edge_groups_len() -> usize {
    EDGE_GROUPS.with(|e| e.borrow().len())
}

/// Exact analytic volume, or a negative sentinel when mass declines / handle stale.
#[unsafe(no_mangle)]
pub extern "C" fn kw_volume(h: i32) -> f64 {
    read(h, |b| b.mass_properties().map(|m| m.volume).unwrap_or(-1.0)).unwrap_or(-1.0)
}

/// Topological edge count (one group per edge in the worker mesh).
#[unsafe(no_mangle)]
pub extern "C" fn kw_edge_count(h: i32) -> i32 {
    read(h, |b| {
        b.entity_ids()
            .filter(|id| matches!(b.lookup(*id), Some(AnyKey::Edge(_))))
            .count() as i32
    })
    .unwrap_or(-1)
}

// ---- lifecycle ------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn kw_release(h: i32) {
    if h < 0 {
        return;
    }
    BODIES.with(|s| {
        if let Some(slot) = s.borrow_mut().get_mut(h as usize) {
            *slot = None;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kw_clear() {
    BODIES.with(|s| s.borrow_mut().clear());
}

// ---- STEP export ----------------------------------------------------------

/// Serialize a body to STEP (AP242). 0 ok (read `step_buffer_ptr/len`), <0 fail.
#[unsafe(no_mangle)]
pub extern "C" fn kw_step_export(h: i32) -> i32 {
    match read(h, |b| keel_topo::step_export::to_step_string(b)) {
        Some(Ok(text)) => {
            STEP_TEXT.with(|t| *t.borrow_mut() = text);
            0
        }
        _ => -1,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn step_buffer_ptr() -> *const u8 {
    STEP_TEXT.with(|t| t.borrow().as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn step_buffer_len() -> usize {
    STEP_TEXT.with(|t| t.borrow().len())
}

// ---- offset ---------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn kw_offset_body(h: i32, distance: f64) -> i32 {
    finish(read(h, |b| {
        let mut c = b.clone();
        c.offset_body(distance).map(|_| c)
    }))
}
