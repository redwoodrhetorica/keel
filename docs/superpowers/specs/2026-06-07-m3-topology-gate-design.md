# M3 Paper-Design Gate: Topology Layer, Operator Set, Lineage Hooks, Boolean Pipeline Shape

**Date:** 2026-06-07
**Status:** Gate document required by the kernel architecture spec before the M3 topology API freezes. Refines and operationalizes spec decisions D3 (PES-class non-manifold topology with first-class regions) and D9 (lineage/determinism/sessions as day-one mandates). Inputs: research dossiers kernel/01, 02, 07, 14 (re-read in full 2026-06-07), wave-3 synthesis items D-regen/D-asm, kernel/22 obligations.

**Why this gate exists:** the topology data structure, the operator set, and the lineage reporting contract are the three most expensive things in a kernel to change after the fact (Fornjot's repeated topology rewrites; OCCT's manifold-with-escape-hatches ceiling; FreeCAD's decade of toponaming pain). The boolean/SSI pipeline is designed ON PAPER here so the topology API provably carries what M6 will need, before a line of M3 code exists.

---

## 1. Entity model

### 1.1 The tower

Body -> Region -> Shell -> Face -> Loop -> Fin -> Edge -> Vertex.

This is the Parasolid/ACIS/STEP/OCCT consensus (kernel/01 part 4). Names follow Parasolid (fin, not coedge or halfedge). All eight are first-class arena-stored entities.

### 1.2 Regions are primary and space-partitioning

- Every body partitions all of 3D space: the union of its regions is R^3.
- Exactly one region per body is the infinite (exterior) region; it is created by the body seed operator and can never be deleted while the body lives.
- Regions are solid or void (a flag). A solid body's point set is the union of its solid regions.
- Every face has exactly two sides; each side bounds exactly one region (possibly the same region on both sides for an embedded sheet). The face-side-to-region link is stored on the face (front_region, back_region), oriented by the face normal sense.
- A shell is a connected set of oriented face-sides (plus wire edges and acorn vertices for lower-dimensional content) bounding a region; a region holds one or more shells (outer plus voids).

Rationale (kernel/02 verdict): Parasolid made regions native and treats manifold bodies as restricted general bodies; ACIS bolted cellular topology on as a secondary attribute layer and pays a consistency tax; OCCT skipped it and its boolean engine visibly hits the ceiling. CAE zone modeling, midsurface-in-solid, and principled PMC all require the partition. We make it native.

### 1.3 PES-class non-manifold structure

- **Fin** = oriented use of an edge by a loop. Stores: edge ref + sense, loop ref, next/prev fin around the loop, and its position in the edge's radial cycle.
- **Edge** stores the radial cycle: an ordered cyclic sequence of all fins using it. Manifold edge = exactly 2 fins; the cycle order is the angular order of face-sides around the edge. Designed-in from day one (the kernel/02 mandate); the manifold fast path simply sees a 2-cycle.
- **Loop** = doubly-linked fin ring + owning face. Outer loop and inner rings distinguished by a flag on the loop (not by ordering).
- **Vertex** non-manifold handling, PES-style: partial entities materialized only where needed. A manifold vertex stores one incident fin; a non-manifold vertex (two cones touching, wire meeting sheet) stores an explicit list of incidence groups (one per local "umbrella"). M3 implements the manifold representation plus the structural slot for the group list; the splitting/merging of umbrellas arrives with the imprint work.
- **Mixed dimension:** a region may directly contain wire edges (no fins, no faces) and acorn vertices; a shell may consist of faces, wires, or a single vertex (STEP vertex_shell/wire_shell equivalents). Body "types" (minimum, wire, sheet, solid, general) are classification queries, not distinct data models.

### 1.4 Storage and identity

- Typed arenas per entity kind (slotmap-style generational indices) inside a `Body`. Arena slots are NEVER identity: they are transient memory addresses.
- Every entity carries a stable `EntityId` (u64), assigned from a per-body monotonic counter at creation, never reused, persisted across save/load (D14 doctrine). Deterministic because operator execution order is deterministic.
- All iteration that can influence output is in EntityId order or in explicitly documented structural order (loop traversal, radial cycle). `BTreeMap`/`IndexMap` only; no `std::HashMap` anywhere in keel-topo (kernel/14 section 1.7).
- Geometry attaches by reference into a per-body geometry arena: Vertex -> point (+ tolerance), Edge -> curve ref + sense (+ tolerance), Face -> surface ref + sense, Fin -> reserved pcurve slot (filled when trimming lands in M4/M5). Multiple topology entities may share one geometry entity (OCCT TShape lesson: canonical geometry stored once).
- Per-entity tolerances default to the session floor (1e-8 linear) and are stored, not implied (tolerant-modeling doctrine; tolerances never decide combinatorial topology, they only gate geometric coincidence tests whose DECISIONS route through the predicate layer).
- Attributes: a minimal typed attribute store keyed by EntityId ships in M3 (PMI, cellular flags, healing reports, and FR hints all attach here later).

---

## 2. Operator set (the only mutation API)

All topology mutation goes through validity-preserving operators (Mantyla completeness licenses building everything above from them). No public API may mutate arenas directly.

### 2.1 Manifold Euler core (M3 implements fully)

| Operator | Inverse | Effect (V, E, F, L, S, G, R deltas) |
|---|---|---|
| MVFS | KVFS | seed body: +V +F +L +S, plus one shell, one new region pair bookkeeping |
| MEV | KEV | +E +V (extend a loop) |
| MEF | KEF | +E +F +L (split a loop into two; new face) |
| KEMR | MEKR | -E +R (delete bridge edge, loop becomes inner ring) |
| KFMRH | MFKRH | -F +R +G (kill face, make ring, raise genus) |

- Each operator asserts the Euler-Poincare relation incrementally in debug builds: `V - E + F = 2(S - G) + R` per manifold shell complex (kernel/01 part 3), via known per-operator deltas (Sequin).
- Implemented as strict inverse pairs with `inverse(op(x)) == x` tests (CGAL discipline).
- Region bookkeeping is part of the operator contracts: MEF that closes a shell creates a region split; KFMRH merges regions through the punched handle. The exact region effects are specified per-operator in the M3 plan.

### 2.2 Subdivision and stitching primitives (M3 implements; booleans consume)

These are the operators the boolean pipeline (section 4) needs; each is expressible as Euler sequences plus radial-cycle edits, but they are first-class API because lineage reporting needs their semantics (split/merge events):

- `split_edge(edge, vertex_geometry) -> (edge_a, edge_b, vertex)`: MEV variant; both child edges recorded as split-children of the parent.
- `split_face(face, fin_a, fin_b, curve) -> (face_a, face_b, edge)`: MEF variant along an imprinted curve; split-children recorded.
- `merge_edges`, `merge_faces`: KEV/KEF variants with merge-result lineage.
- `merge_vertices(v_a, v_b)`: the glue primitive; may create a non-manifold vertex (umbrella groups).
- `glue_edges(edge_a, edge_b)`: identifies two geometrically coincident edges, merging radial cycles; THE non-manifold maker (an edge acquires more than 2 fins). Used by stitch/imprint/boolean.
- `embed_wire(region, edge)`, `embed_vertex(region, vertex)`, `embed_sheet(region, face)`: mixed-dimension content placement.
- `split_region(shell) -> region`, `merge_regions(face)`: region bookkeeping when closed shells form or interface faces dissolve.

### 2.3 Validation (always-on structural, debug-build exhaustive)

- Incremental Euler-Poincare per manifold shell (necessary, not sufficient).
- Structural checks: every fin's loop/edge back-references consistent; loop rings closed; radial cycles consistent (every fin appears exactly once in its edge's cycle); every face side maps to a region; every region reachable from its shells; exactly one infinite region.
- Boundary-of-boundary check (LAR d-of-d = 0 over the incidence structure) as the non-manifold validity oracle where the scalar Euler formula does not apply (kernel/02 theme 5).
- `Body::validate()` public; debug builds run it after every operator; release builds on demand.

### 2.4 Atomicity

Every public operation is transactional: on failure the body is byte-identical to before (Parasolid known-state invariant, kernel/07 mandate 12). M3 mechanism: operations either (a) pre-validate all preconditions then perform infallible mutations, or (b) for compound operations, build against a body clone and swap on commit. Full COW structural sharing is a later optimization; the API contract (snapshot/restore + atomic ops) is fixed now.

---

## 3. Lineage and session hooks (D9, day one)

### 3.1 Per-entity lineage record (kernel/07 mandates 1-4)

Every entity carries, at creation: `{ op_id, derivation }` where derivation is one of
`Created`, `Modified { from }`, `Generated { from }`, `SplitChild { from, ordinal }`, `MergeResult { from: Vec<EntityId> }`.
Deleted entities are recorded in the op report (the entity itself is gone). Ordinals make split children distinguishable; merge results carry all parents.

### 3.2 Operation reports

Every public operation (Euler op, subdivision primitive, and every future compound op: boolean, fillet, sweep) returns an `OpReport`:
- `op_id` (monotonic per body history)
- `created`, `deleted`: Vec<EntityId>
- `modified`, `generated`: Vec<(EntityId, EntityId)> (input -> output)
- `split`: Vec<(EntityId, Vec<EntityId>)>, `merged`: Vec<(Vec<EntityId>, EntityId)>

This is the OCCT Modified/Generated/Deleted contract that FreeCAD's toponaming fix consumes, made native and total: no operation may mint anonymous topology.

### 3.3 Anchors and queries

- Every face/edge/vertex can produce an object-space representative point on demand (Mun-Han OSI disambiguator); not stored, computed deterministically from geometry.
- The reference model upward is lineage queries (`created_by(op)`, `derived_from_any/all(ids)`), resolved against the current body (OnShape/Cascaval). M3 ships the lineage storage and the basic query functions; the full query language grows with consumers.

### 3.4 Sessions

- `Body::snapshot() -> Snapshot` and `Body::restore(Snapshot)`: M3 implementation is a deep clone (correct first); the API admits a COW/persistent-structure upgrade later (kernel/07 section 2.3) without signature change.
- Journal hook: a `trait OpJournal { fn record(&mut self, op: &OpDescriptor); }` the body accepts at construction; M3 ships the trait plus a replay test proving record/replay determinism on the primitive constructors. Full journaling/partitions follow Parasolid's model later.
- Determinism contract applies in full (kernel/14): stable IDs, ordered containers, no address leaks, golden topology-hash regression tests (deterministic hash over the entity tower in ID order).

---

## 4. Boolean/SSI pipeline shape (paper design; implemented M5-M6)

Stated here so the M3 API freeze provably carries it. The pipeline for `boolean(body_a, body_b, op)`:

1. **Localize.** AABB/BVH over faces of both bodies; candidate face pairs only (Mantyla-Tamminen localization).
2. **Intersect (SSI/CSI, M5).** Per face pair: analytic-analytic pairs take exact special-case paths (plane/plane, plane/quadric, quadric/quadric via QI where cheap); spline pairs take the certified path: Gauss-map separability to bound branch topology, collinear-normal solves (PP solver, shipped in M2b) for loop seeds, traced with monotone-segment or validated marching, all start/end points polished and toleranced. Output: intersection curves with (u,v) pcurves on both faces plus vertex events.
3. **Imprint.** Split topology along intersection curves using section 2.2 primitives: `split_edge` at curve/edge crossings, `split_face` along each curve segment, `glue_edges` to identify the coincident imprinted edges of A and B. After imprinting, the two bodies share identical edge geometry along the intersection, and shared edges carry 4 fins in their radial cycle (2 from each body): the non-manifold transient state that motivated D3.
4. **Classify.** Each face fragment of A is classified against B (and vice versa) as in/out/on via PMC; "on" fragments use neighborhood classification (Tilove SMC) routed through the exact predicate layer per the affordability gradient. Region structure makes in/out a region lookup once one sample point is classified.
5. **Select and stitch.** Regularized semantics (Requicha r-sets): keep fragments per the boolean's selection table, reassemble shells by walking radial cycles, recompute region partition (`split_region`/`merge_regions`), discard or keep "on" fragments per regularization.
6. **Report.** The OpReport composes the lineage of every step: every result face knows its ancestor face and operation; imprinted edges know both parent faces. This falls out of sections 2.2 and 3.2 rather than being bolted on.

**Freeze checklist derived from the pipeline:** the M3 API must (and, per this document, does) provide: radial cycles capable of 4+ fins; split/glue/embed primitives with split/merge lineage; region split/merge bookkeeping; face-side-to-region links for classification; per-entity tolerances; fin pcurve slots; atomic compound operations; total OpReports. Each item maps to a section above.

---

## 5. M3 scope (what gets built now)

New crate `keel-topo` (depends on keel-math, keel-geom):

1. Arenas, EntityId, the eight-entity tower with regions and radial cycles, geometry attachment, attribute store.
2. Manifold Euler operator set (2.1) with incremental invariant checking and inverse-pair tests.
3. Subdivision/stitch primitives (2.2) at least: split_edge, split_face, merge_vertices, glue_edges, embed_* (the rest as needed by constructors).
4. Lineage records, OpReport, lineage queries; snapshot/restore; journal trait + replay determinism test.
5. Validation suite (2.3) including the d-of-d oracle.
6. Primitive constructors proving the operator set end-to-end: block, extrude-of-profile (prism), revolve-based cylinder/sphere/cone/torus bodies wiring M2 geometry to topology. Each constructor is pure Euler-operator sequence and emits full lineage.
7. Topology debug text dump (documented grammar seed) + golden topology-hash regression tests.
8. Adjacency/interrogation queries: loops of face, fins of edge, faces around vertex, shells of region, body classification (wire/sheet/solid/general/mixed), connected components.

Deferred (explicitly): general PMC (needs CSI, M4), imprint against curves (M5), booleans (M6), COW snapshots, partitions/full journaling, file format.

## 6. Non-goals and risks

- No attempt at G-maps/combinatorial maps (quasi-manifold mismatch, kernel/02 theme 4).
- The scalar Euler invariant does not extend to non-manifold complexes; we accept per-structure validation plus d-of-d instead (kernel/02 theme 5). Risk: validation cost; mitigated by debug-build gating.
- Region bookkeeping through operators is the subtlest part of the design (it is where OCCT never went). Mitigation: region effects specified per-operator in the plan, property-tested with randomized operator sequences checked against full revalidation.
- PES partial-entity vertex machinery is sketched, not fully specified; M3 implements the manifold umbrella plus the group-list slot, and the first real consumer (imprint, M5) drives the completion. This is a deliberate two-phase commitment matching how PES itself materializes partial entities on demand.
