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

## Deferred — Phase 2/3 keel-wasm work (NOT done here)

These remain for the consumer/WASM phase, as scoped:

- `worker_mesh` `edge_groups`: per-edge polylines in the tessellation for
  UI edge highlighting. (`closest_point_on_edge` / `nearest_edge` already
  give the building block for ray-based picking in keel-wasm.)
- `worker_mesh_tol(chord)`: tolerance-parameterized tessellation for draft
  vs final.
