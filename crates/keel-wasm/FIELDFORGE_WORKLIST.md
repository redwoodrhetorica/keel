# Keel worklist from fieldforge (real-usage decline log)

Gaps fieldforge has hit driving real workflows. Each is op + repro + expected
behavior. Prioritized by how common the workflow is in practice. The app handles
all of these decline-safe (geometry left unchanged + a warning); these are
kernel/binding items for the Keel session, not app bugs.

## P0 - worker_mesh: T-junction CRACKS at curved face-face boundaries. [RESOLVED: edge-conforming worker mesh]
`worker_mesh` tessellates each FACE independently, so a shared topological edge
(e.g. a bored hole's rim, where the cylindrical wall meets the planar cap) is
discretized TWICE -- once by each adjacent face -- at DIFFERENT vertex angles.
The two faces therefore do not share rim vertices, leaving a ring of open
(unpaired) edges = T-junction cracks all around every curved face boundary. At a
grazing view angle through a thin wall the far-rim crack foreshortens into a wide
dark wedge (looks like a missing segment). Confirmed NOT a normal/winding bug.

- Repro: `block(2.4 x 1.6 x 0.4)` - `cylinder` r=0.3 bore -> Difference. Mesh
  (`kw_mesh`, any chord). Weld the triangle soup by rounded position and check
  edge pairing:
  - measured: at chord 1.0e-4, 3952 tris, **306 open edges, ALL with both
    endpoints on a rim plane (|z|=0.2), zero on the wall interior**; winding
    100% consistent (0 flipped); no degenerate/zero-area tris. Coarsening the
    weld tolerance to 5e-4 does NOT pair them -> the wall and the cap genuinely
    sample the rim circle at different points (independent per-face meshing),
    not a float-precision mismatch. At chord 3.4e-5 it's 484 open edges (finer =
    more, smaller cracks). So this is chord-independent in KIND, present at every
    tessellation density.
- Expected (OCCT BRepMesh / Parasolid): discretize each topological EDGE ONCE
  (edge polyline), then mesh each face CONFORMING to its boundary edges' shared
  polylines, so adjacent faces share identical edge vertices -> watertight render
  mesh, zero open edges on a closed solid, no rim cracks.
- Fix: edge-first meshing. Tessellate every Edge to a shared polyline (honoring
  the chord), cache per-edge; when meshing a face, its boundary loops MUST use
  the already-tessellated shared-edge vertices (not re-sample the curve). Both
  faces on an edge then index the same rim points. (This also makes seam handling
  and the P1 rim-merge cleaner since rim topology is explicit.)
- Note: independent of the LOD/chord work on the fieldforge side; app cannot fix
  this without re-stitching the soup (insert T-junction verts + re-triangulate),
  which we are NOT doing -- the kernel should emit a watertight render mesh.
- **STATUS — RESOLVED** (`tessellate.rs` + `render.rs::worker_mesh_opt`). `worker_mesh`
  now tessellates EDGE-FIRST: a new `tessellate_face_watertight` path derives every
  face's boundary from the shared per-edge polyline (`edge_polyline_opt`, a pure
  deterministic function of the edge), so two faces meeting along a curved rim emit
  IDENTICAL vertices and the welded soup has zero boundary edges. Planar faces reuse
  the ear-clip / holed-bridge triangulation on the shared rims; cylinder/cone laterals
  and full tori connect their shared rim polylines with an ANGLE-MERGED ribbon (handles
  unequal vertex counts AND unequal start phases between a rim and an interior tube row);
  spheres / NURBS / complex-trim analytic faces fall back to the proven grid (still
  watertight where they share only circular rims). This is a SEPARATE path from the
  GWN/boolean classifier (`tessellate_face`) and the mass==mesh oracle (`mesh_volume`),
  both left byte-identical -- so no boolean-correctness or mass==mesh risk. The P1 edge
  picking metadata (seam-skip + co-circular-arc merge) is unchanged. Verified by
  `tests/worker_mesh_watertight.rs`: box, drilled plate, AND drilled+filleted plate all
  yield ZERO unpaired edges (was 192 / 352 before); full keel-topo suite green (298 lib
  + all integration incl. the mass==mesh witnesses), smoke test passes. Flat per-face
  normals preserved (the worker recomputes them from the triangle winding). Rebuilt
  keel-wasm (1.3 MB); revendor the .wasm to ship.

## P0 - fillet/chamfer a CIRCULAR edge (hole rim). VERY common. [CONVEX RIM: RESOLVED in source]
`fillet_edge` / `chamfer_edge` on a closed circular edge where a cylindrical face
meets a planar face (the rim of a bored hole).

- Repro: `block(origin,2.4,1.6,0.4)`; `cylinder` radius 0.3 through it; `boolean
  Difference` -> a plate with a bore. Circular top-rim edge (plane ∩ cylinder),
  `fillet_edge(rim, 0.05)` / `chamfer_edge(rim, 0.05)`.

- **API ANSWER (the consumer's question): call `fillet_edge(edge, radius)` /
  `chamfer_edge(edge, distance)` directly. NO routing needed in the binding.**
  `fillet_edge` auto-detects a closed circular plane-cylinder rim and dispatches
  internally to `fillet_cap_rim` (`blend.rs:1742`); `chamfer_edge` -> `chamfer_cap_rim`.
  The rim is bisected into TWO arcs at the cylinder seam; pass EITHER arc (the
  consumer's blind `nearest_edge` pick is fine) -- the dispatch rounds the FULL
  360 deg loop (verified: removes the full toroidal wedge, not a half).

- **STATUS (this session):**
  - CONVEX rim (bored-hole lip / mouth, the exact repro above): **WORKS** -- valid
    solid, watertight, validate() Ok, mass == mesh within the curved band, full
    360 deg. Verified by `tests/circular_rim_fillet.rs` (fillet + chamfer, 4/4 green).
    The decline fieldforge sees is a STALE BUILD: this fix lives in the working
    tree (blend.rs, ~554 lines vs HEAD) and is not yet committed/released. Rebuild
    keel-wasm from current source and the convex case passes.
  - Bug fixed this session: the annulus-vs-disc dispatch used a pure GWN probe that
    mis-classified a blind-hole FLOOR disc (solid lies beyond the disc radially, so
    GWN says "annulus", but the disc's rim is its OUTER loop) -> routed to annulus
    surgery which rejected it ("not an inner ring"). Now refined by the rim's actual
    loop role (`cap_rim_geometry`, blend.rs).
  - CONCAVE blind-hole FLOOR rim: still **DECLINES** (the disc-cap surgery's loop
    selection is not yet correct for the reentrant orientation; it kept the wrong
    region -> a body that validate()'d but filled the bore, +128 vol). The fillet
    disc path now SELF-CHECKS (mass-declined / mass!=mesh / volume-direction) and
    declines it honestly instead of shipping the wrong body (DECLINE-never-WRONG
    restored). Regression-locked by `concave_blind_floor_fillet_never_wrong`.
    FOLLOW-UP: fix the disc-cap surgery loop selection for the concave case so the
    reentrant floor fillet produces the correct small wedge.
- Rounding/chamfering hole edges shows up in a large fraction of tutorials; the
  common convex mouth rim is the bulk of that and is now handled.

## P1 - cylinder SEAM: bisected rim edge + visible seam line. [RESOLVED via worker_mesh, option (b)]
A cylindrical face is represented WITH A SEAM, so a bored hole's rim comes back as
TWO arc edges (semicircles) instead of one closed circular edge, and the seam shows
as a straight edge line down the bore wall. fieldforge renders this topology
faithfully -> the user sees two half-rim edges + a seam line, and a rim pick selects
only half. Fillet still works (the cap-rim dispatch rounds the full loop from one
arc), so this is a CLEANLINESS/topology issue, not broken geometry.
- Repro: `block` - `cylinder` bore; `worker_mesh().edge_groups` shows 4 curved arcs
  for a through-hole (2 rims x 2 arcs of ~24 segs each) + a 2-vertex seam edge.
- Expected (OCCT/Parasolid): the cylinder as one periodic face with ONE closed
  circular rim edge per end, and no exposed seam edge.
- Fix options: (a) represent cylinder/cone/sphere/torus as periodic faces with
  closed rim edges; or (b) at minimum, don't emit the seam as a pickable edge and
  merge the two rim arcs into a single edge_group so the consumer can
  select/highlight the whole rim as one edge.
- **STATUS — RESOLVED via option (b)** (`render.rs::worker_mesh_opt`). The worker
  mesh's edge picking metadata (`lines` + `edge_groups`) now: (1) SKIPS a periodic
  SEAM edge (a manifold edge whose BOTH coedges lie on the SAME face), so the seam
  neither renders as a line nor is pickable; and (2) MERGES co-circular arcs --
  keyed by the circle's geometry (center, normal, radius) so it works across
  distinct curve instances -- into ONE edge_group, so a bored rim is a single
  pickable edge spanning the full loop. The TRIANGLE mesh (the mass==mesh oracle
  path) is byte-identical -- only picking metadata changed. Verified: drilled plate
  17 raw edges -> 14 edge_groups (2 rim-merges of 4 arcs -> 2, + 1 lateral seam
  dropped); box unchanged (12 groups); 298 lib + 3 edge_group tests green
  (`tests/worker_mesh_edge_groups.rs` updated to the new contract). To ship:
  rebuild + revendor keel-wasm; `mesh_edge_groups` then exposes the clean topology.

## P1 - closed-profile revolve (washer/tube). Common.
`revolve` is lathe-only (meridian must touch the axis at both ends). Revolving a
CLOSED 2D cross-section that does NOT touch the axis (to make a washer/tube/ring)
is unsupported, so fieldforge declines closed-profile revolves.
- Repro: revolve a rectangle whose nearest edge is at radius 1 (not on axis) 360°
  -> expect an annular tube; Keel requires meridian ends at radius 0.
- Expected: solid of revolution of a closed off-axis profile (replicad/Parasolid behavior).

## P1 - fillet all edges of a box (3-edge corner). Common.
Filleting the three edges meeting at a box corner: the corner cap is the known
decline (sphere-octant / generic-cap trim follow-up). fieldforge applies picked
edges sequentially, so independent edges round fine; the shared 3-edge vertex is
where it declines.
- Repro: `block` + fillet all 12 edges at a modest radius.

## P2 - bindings fieldforge will call once they exist (per FIELDFORGE_BINDING.md) [DONE]
- [DONE] `kw_step_export(h) -> i32` + `step_buffer_ptr()/len()` (wraps `step_export::to_step_string`).
- [DONE] `kw_offset_body(h, distance) -> i32` (wraps `Body::offset_body`).
- The COMPLETE binding (the whole FIELDFORGE_BINDING.md ABI, not the task-21 spike) is now
  implemented in `crates/keel-wasm/src/lib.rs`, builds clean to wasm32, and passes the end-to-end
  smoke test (`node crates/keel-wasm/phase2_smoke.mjs`). Rebuild + revendor the .wasm to ship it.

## P3 - robust edge identity for fillet/chamfer re-blend (direct-modeling polish)
fieldforge re-applies every edge op (fillet/chamfer) on each rebuild by re-finding the edge
via `nearest_edge(point)` + `closest_point_on_edge` from a stored 3D point. This already
gives Shapr3D-style "auto-reblend" for moderate base edits. The gap is ROBUSTNESS: when the
base geometry changes enough that the stored point is closer to a DIFFERENT edge, the op
silently re-finds the wrong edge (or the edge vanishes and the op drops). A stable
`EntityId`-based mapping that survives edits — so an edge op tracks the SAME topological edge
across rebuilds, with an explicit decline when that edge truly no longer exists — would make
the re-blend trustworthy. Not blocking (point-based works for now); a precision/robustness
enhancement surfaced by the interaction research. fieldforge-side, the op stores a point; if
Keel exposed a re-resolvable stable edge key (or a "track this edge through this op" handle),
the consumer would use it instead of nearest-point.

## P3 - enhancement (not a defect): smooth normals in worker_mesh
`worker_mesh` emits FLAT per-face normals, so consumers must reconstruct smooth
shading on curved faces (fieldforge does crease-angle smoothing app-side). If
`worker_mesh` emitted true per-vertex surface normals on curved faces (radial on
cylinder/cone/sphere/torus, exact on fillets), consumers get correct shading for
free. Optional polish.
