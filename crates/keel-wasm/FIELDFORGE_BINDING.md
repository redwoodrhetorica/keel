# keel-wasm binding contract (for fieldforge)

This is the exact extern-C ABI fieldforge's WASM bridge (`fieldforge/src/geometry/keel/bridge.ts`)
calls. The Keel session owns keel-wasm now; implement the canonical binding to match these
signatures and semantics so the fieldforge bridge needs no changes. (A complete working build
of this ABI is currently vendored as `fieldforge/src/geometry/keel/keel_wasm.wasm`; treat it as
the reference behavior.)

## Conventions
- No wasm-bindgen. Raw `#[unsafe(no_mangle)] pub extern "C"` exports; the host reads linear
  memory directly via `instance.exports.memory.buffer` + the `*_ptr()`/`*_len()` accessors.
- **Bodies never cross the boundary.** They live in a thread-local slab; the host holds an
  `i32` HANDLE. Every body-producing call returns the handle (>= 0) or a NEGATIVE decline code
  (DECLINE-never-WRONG: on decline the inputs are untouched, no body is created).
- All coordinates/lengths are `f64`. Rotations are DEGREES. Booleans use tol 1e-7 internally.

## Memory (host writes variable-length inputs here)
- `kw_alloc(len: usize) -> *mut u8` — **8-byte aligned** (back it with `Vec<f64>`), so the host
  can lay down `Float64Array` point data.
- `kw_free(ptr: *mut u8, len: usize)`

## Primitives (build at LOCAL origin; the host applies placement separately)
- `kw_block(ox,oy,oz, dx,dy,dz) -> i32`  (origin + extents)
- `kw_cylinder(ox,oy,oz, ax,ay,az, radius, h) -> i32`  (base origin, axis dir)
- `kw_cone(ox,oy,oz, ax,ay,az, radius, h) -> i32`
- `kw_sphere(cx,cy,cz, radius) -> i32`
- `kw_torus(cx,cy,cz, ax,ay,az, major, minor) -> i32`

## Placement / transform
- `kw_place(h, scale, rx_deg, ry_deg, rz_deg, px,py,pz) -> i32` — scale about origin via
  `Body::scaled` (NURBS-exact; `transformed` is rigid-only and must NOT take a scaling matrix),
  then rigid rotate X,Y,Z (deg, about origin) + translate, via `Body::transformed(&Transform3)`.
- `kw_mirror(h, px,py,pz, nx,ny,nz) -> i32` — `Body::mirrored(point, normal)` (host unions for symmetry).

## Profile ops (host writes a flat point buffer, passes ptr + count)
- `kw_prism(ptr: *const f64, n_pts, dx,dy,dz) -> i32` — extrude a closed world loop (n_pts*3 f64) along dir. `Body::prism`.
- `kw_revolve(ptr: *const f64, n_pairs, ax,ay,az, ox,oy,oz, angle_deg) -> i32` — lathe a
  (radius,height) meridian (n_pairs*2 f64) about the axis through origin; >=360 -> `revolve`,
  else `revolve_partial`. (Meridian ends must be on the axis; closed-profile revolve is a known gap.)
- `kw_loft(bot: *const f64, n_bot, top: *const f64, n_top) -> i32` — `Body::loft` of two equal-count world loops.
- `kw_loft_circles(ax,ay,az, ox,oy,oz, r0, r1, h) -> i32` — exact `Body::loft_circles` frustum.

## Booleans
- `kw_boolean(ha, hb, op: i32) -> i32` — op 0=Union, 1=Difference, 2=Intersection. `boolean(&a,&b,op,1e-7)`.

## Edge ops by 3D point (pick crux)
- `kw_fillet_nearest(h, x,y,z, radius, tol) -> i32` — `nearest_edge(p)`; reject if
  `closest_point_on_edge` distance > tol; else `fillet_edge`.
- `kw_chamfer_nearest(h, x,y,z, distance, tol) -> i32` — same, `chamfer_edge`.

## Face ops by 3D point (clone-then-mutate so a decline keeps the original)
- `kw_push_face(h, x,y,z, d, tol) -> i32` — `pick_face(p,tol)` -> `push_face(fk, d)` on a clone.
- `kw_draft_face(h, x,y,z, angle_deg, tol) -> i32` — neutral plane at body base (min Z, +Z normal); `draft_face`.
- `kw_hollow(h, t) -> i32` — closed `Body::hollow(t)`.
- `kw_hollow_pierce(h, x,y,z, t, tol) -> i32` — `hollow_pierce(t, |fk| fk == pick_face(p,tol))`.

## Meshing + interrogation
- `kw_mesh(h, chord: f64) -> i32` — stage `Body::worker_mesh_tol(chord)` into thread-locals; 0 ok, <0 fail.
  Host calls these accessors immediately after (read before the next call):
  - `mesh_positions_ptr()/len()` (f32), `mesh_normals_ptr()/len()` (f32), `mesh_indices_ptr()/len()` (u32), `mesh_lines_ptr()/len()` (f32)
  - `mesh_groups_ptr()/len()` (u32) — FACE groups, flat (id, start, count) triples; start/count index into `indices`.
  - `mesh_edge_groups_ptr()/len()` (u32) — EDGE groups, flat (id, start, count) triples; **start/count are
    f32-element offsets into `lines`** (count a multiple of 6). The host divides by 3 for vertex units.
- `kw_volume(h) -> f64`, `kw_edge_count(h) -> i32`
- `kw_release(h)`, `kw_clear()` — free one / all bodies (host calls `kw_clear` at the start of each build).

## IMPLEMENTED (these now exist in `src/lib.rs`)
- `kw_step_export(h) -> i32` (0 ok) + `step_buffer_ptr()/len()` — `keel_topo::step_export::to_step_string`.
- `kw_offset_body(h, distance) -> i32` — `Body::offset_body`.

## Build (the COMPLETE binding now lives in `src/lib.rs`, not just the spike)
- `cargo build --release --target wasm32-unknown-unknown -p keel-wasm`
- Artifact: `target/wasm32-unknown-unknown/release/keel_wasm.wasm` (~1.3 MB). Copy this to
  `fieldforge/src/geometry/keel/keel_wasm.wasm`.
- Smoke test (loads the wasm, checks all 42 contract exports, runs box - bore -> rim fillet ->
  mesh/edge-groups/STEP end-to-end): `node crates/keel-wasm/phase2_smoke.mjs` -> "ALL SMOKE CHECKS PASSED".
  (The old `run_spike.mjs`/`phase1_smoke.mjs` targeted the removed task-21 demo exports and are superseded.)
