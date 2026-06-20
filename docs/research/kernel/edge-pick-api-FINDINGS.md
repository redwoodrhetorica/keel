# Edge-pick public API (keel-topo) — findings

Additive, read-only accessors that let a consumer enumerate and pick the
edges/faces of an ARBITRARY body (e.g. a boolean result), unblocking
fillet/chamfer-by-pick in the fieldforge integration. Zero geometry or
behavior change; no existing signature touched; cannot affect WRONG.

## What was added (all `pub` on `Body`)

| Method | Signature | File |
|---|---|---|
| `edge_keys` | `fn edge_keys(&self) -> Vec<EdgeKey>` | `crates/keel-topo/src/body.rs` |
| `face_keys` | `fn face_keys(&self) -> Vec<FaceKey>` | `crates/keel-topo/src/boolean.rs` (visibility widened `pub(crate)` -> `pub`, rustdoc added) |
| `closest_point_on_edge` | `fn closest_point_on_edge(&self, e: EdgeKey, p: Vec3) -> Option<(Vec3, f64)>` | `crates/keel-topo/src/interrogate.rs` |
| `nearest_edge` | `fn nearest_edge(&self, p: Vec3) -> Option<EdgeKey>` | `crates/keel-topo/src/interrogate.rs` |

- `edge_keys` / `face_keys`: all LIVE edges/faces in deterministic
  `EntityId` order. `edge_keys` wraps the existing `entity_ids()` +
  `lookup()` enumeration (the exact idiom several tests already
  open-coded). `face_keys` was already the canonical implementation,
  only its visibility changed — kept as ONE clean public method, no
  wrapper.
- `closest_point_on_edge`: closest point on edge `e` to `p` and the
  distance (model length units). `None` only if `e` is stale/foreign or
  its endpoint vertices are missing.
- `nearest_edge`: the picked edge (smallest `closest_point_on_edge`
  distance). Deterministic tie-break: lowest `EntityId` among
  equidistant edges, so repeated picks reproduce. Built on
  `edge_keys` + `closest_point_on_edge`.

`EdgeKey` / `FaceKey` were already public (they appear in the
`fillet_edge` / `chamfer_edge` signatures), so these methods need no new
type exports. Keys are transient (valid for one body value, invalidated
by the next mutation); the order is stable across identical inputs.

## Curved-edge closest-point approach

A boolean seam is frequently a circle / ellipse / NURBS, not a line, so
`closest_point_on_edge` is curve-aware and respects the edge's BOUNDED
span (an edge is a segment of its carrier, not the whole infinite curve):

- **Straight** (`Line`, degree-1 NURBS, or no carrier): exact closed-form
  segment projection between the two endpoint vertices, clamped to
  `[0,1]`.
- **Circle / Ellipse**: project `p` onto the infinite conic in closed
  form (`Circle3::project` / `Ellipse3::project`), then clamp the
  parameter to the edge's TRUE angular span. The span is rebuilt with the
  same rules `interrogate.rs::edge_polyline` uses — endpoint projection,
  the recorded `arc_sweep` identity, `true_arc_span` disambiguation, and
  full-revolution for a closed edge — via a small shared helper
  (`conic_span_param` / `conic_endpoint_params`), so pick and wireframe
  stay consistent. If the projected parameter is outside the span, the
  nearer endpoint wins.
- **NURBS** (degree >= 2): keel-geom's certified-global projector
  `keel_geom::project::project_point` (Bezier decomposition + control-AABB
  branch-and-bound + bracketed Newton). No new solver written — the
  existing global projection is reused as specified. `width_tol` follows
  the edge tolerance.
- **Belt-and-suspenders**: for any curved edge the bounded
  `edge_polyline` samples are also fed in as candidates (taking the min of
  distances). The polyline lies ON the edge, so it is a safe fallback when
  the closed-form conic parameter is out of span or the carrier is
  degenerate (the corner-blend "spring edge" coincident-projection case);
  the analytic candidate always wins when valid, so accuracy is exact in
  the normal case and never worse than wireframe resolution in the
  pathological one.

The endpoints are always candidates, so the result is correct whether the
nearest point is interior to the arc or at a vertex.

## Tests

`crates/keel-topo/tests/edge_pick_api.rs` (3 tests, all green):

1. `boolean_union_enumerate_pick_fillet` — the fieldforge scenario:
   union two overlapping blocks (a stepped solid), `edge_keys()` exposes
   the result's edges, `nearest_edge(seam_pt)` picks the z=10 step seam
   edge, `nearest_edge(top_pt)` picks a convex top edge, and
   `fillet_edge(picked, 1.5)` yields a watertight body with analytic
   `mass == mesh` (`validate` + the floor-gate self-consistency check).
2. `closest_point_on_straight_edge` — exact segment projection on a known
   block edge, including the clamp past an endpoint.
3. `closest_point_on_curved_edge` — a cylinder's top cap circle (r=5):
   in-plane, on-axis-above (distance `sqrt(r^2 + h^2)`, proving the curve
   is sampled rather than chorded), and on-curve points are all exact to
   1e-7.

Note: filleting the stepped-vertex CORNER edge of the union DECLINES
(`kev: fins not adjacent`) — a legitimate fillet-surgery limitation at
that vertex, never WRONG — so the watertight-fillet assertion uses a
clean convex top edge (the pick itself works on every edge).

## Verification

- `cargo build --release` (keel-topo): clean.
- `cargo test --release` (keel-topo): all green, incl. the WRONG-locks
  (`scan_wrong`, `union_wrong_repro`, `cyl_union_mass_witness`,
  `three_bucket`, `post_fillet_mass`, `tutorial_workflows` 10 pass).
- `cargo clippy --release --all-targets`: zero new warnings (the new code
  is clippy-clean; only pre-existing warnings remain in unrelated files).
- `cargo doc --workspace --no-deps`: clean (rustdoc on every new method).

## Phase 2 — worker-mesh edge_groups + chord-tolerance variant (DONE)

The two deferred items below are now implemented (additive; the default
worker_mesh / mass / oracle path is byte-identical — see Verification).

### Feature 1 — `edge_groups` (parallel to `face_groups`)

`WorkerMesh` (`crates/keel-topo/src/render.rs`) gains one field:

```rust
pub edge_groups: Vec<(u64, u32, u32)>,   // (id, start, count)
```

- **`id`** = the source edge's STABLE `EntityId.0` (NOT an arena slot;
  unlike `face_groups`, which carry `FaceKey::index`). The host maps a
  rendered/picked edge back to its `EdgeKey` with
  `Body::lookup(EntityId(id)) -> Some(AnyKey::Edge(k))`.
- **`start` / `count`** are in **f32 ELEMENTS** of the existing `lines`
  buffer: the edge's segments are `lines[start as usize .. (start+count)
  as usize]`. `count` is always a multiple of 6 (two xyz endpoints per
  LineSegments segment; `count/6` = segment count for that edge).
- Ranges are contiguous, non-overlapping, and tile the WHOLE `lines`
  buffer in build order. `lines` contents/order are UNCHANGED — this is
  pure metadata over the existing buffer (the existing `mesh_lines_ptr/len`
  consumers are unaffected).
- One group per topological edge that tessellates (polyline >= 2 points);
  the group set's ids equal `edge_keys()` ids exactly.

### Feature 2 — `Body::worker_mesh_tol(chord)`

```rust
pub fn worker_mesh(&self) -> WorkerMesh;             // DEFAULT density (unchanged)
pub fn worker_mesh_tol(&self, chord: f64) -> WorkerMesh;   // NEW: chord-tolerance
```

Both delegate to a private `worker_mesh_opt(tol: Option<f64>)`.
`worker_mesh()` = `worker_mesh_opt(None)` (default density, the mass/oracle
tessellation). `worker_mesh_tol(chord)` = `worker_mesh_opt(Some(chord))`:
curved analytic FACES are faceted within `chord` (existing
`tessellate_face_tol`), and curved EDGES are arc-sampled to the same chord
via a new `Body::edge_polyline_opt(edge, Option<f64>)` (interrogate.rs)
that routes the conic segment count through `tessellate::arc_segments`
(now `pub(crate)`). `edge_polyline` now = `edge_polyline_opt(edge, None)`;
since `arc_segments(.., None, 32)` returns exactly 32, every default
consumer (incl. `render_mesh`) is byte-identical. NURBS edges keep the
32-sample grid (no analytic radius — matches `render_mesh_tol`'s NURBS
face policy).

### keel-wasm exports (`crates/keel-wasm/src/lib.rs`) — host binds these

| Export | Signature | Purpose |
|---|---|---|
| `mesh_edge_groups_ptr` | `() -> *const u32` | ptr to flattened `(id,start,count)` u32 triples |
| `mesh_edge_groups_len` | `() -> usize` | element count (triples * 3) |
| `demo_mesh_build_tol` | `(chord: f64) -> i32` | build the demo scene at chord tol; 0 ok / negative stage code |

The edge-groups buffer mirrors the existing `mesh_groups_ptr/len` (faces):
`(id, start, count)` u32 triples, `id` = edge `EntityId` cast to u32 (ids
in one body fit u32, same as `face_groups`), `start`/`count` = f32-element
offsets into the `lines` buffer read via `mesh_lines_ptr/len`. Staging is
shared by `demo_mesh_build` and `demo_mesh_build_tol` (a new `EDGE_GROUPS`
thread-local + `stage_mesh` helper). All existing exports are unchanged.

### Tests

`crates/keel-topo/tests/worker_mesh_edge_groups.rs` (3 tests, all green):

1. `edge_groups_partition_lines_and_resolve` — on a drilled-plate boolean:
   edge_groups are contiguous/non-overlapping, tile the `lines` buffer,
   each `count` a multiple of 6, every `id` resolves to a live `EdgeKey`
   via `lookup`, and the group-id set == live `edge_keys()` ids.
2. `worker_mesh_tol_refines_curved_edges_and_faces` — on a cylinder,
   `worker_mesh_tol(1e-4)` has strictly more line floats AND more
   triangles than `worker_mesh_tol(0.1)`; both still partition `lines`.
3. `worker_mesh_default_unchanged_snapshot` — `worker_mesh()` is
   deterministic and its `lines` are byte-identical to the legacy
   `render_mesh().edges` flattening (proves edge_groups is pure additive
   metadata and the default density is unchanged).

`crates/keel-wasm/run_spike.mjs` extended: verifies `mesh_edge_groups_*`
partition the lines buffer across the WASM boundary, and that
`demo_mesh_build_tol(0.2)` (coarse) yields fewer tris/segments than
`demo_mesh_build_tol(1e-4)` (fine). Live run: `edge groups: 17 edges
tiling 846 line floats -> PARTITION OK`; `chord tol: coarse 336 tris / 45
segs, fine 7216 tris / 461 segs -> REFINES OK`.

### Phase-2 verification

- `cargo build --release` + `cargo test --release` (keel-topo): all green
  (298 lib + new 3); WRONG-locks green (`three_bucket` [ignored, compiles],
  `cyl_union_mass_witness` 3, `post_fillet_mass` 2, `scan_wrong`,
  `union_wrong_repro` 2, `tutorial_workflows` 10).
- **Default path byte-identical**: `worker_mesh()` positions/normals/
  indices/lines and `mesh_volume()` unchanged — confirmed by the snapshot
  test (`lines == render_mesh().edges`) and the unchanged WRONG-locks. No
  10k soak needed (additive-only). `src/massprops.rs` untouched.
- `cargo build -p keel-wasm` (native) + `--target wasm32-unknown-unknown
  --release`: both clean. `node run_spike.mjs`: all contracts OK, pin/box
  volumes EXACT.
- `cargo clippy --release`: zero new warnings from the new code (only
  pre-existing keel-topo lib warnings in unrelated files remain).
