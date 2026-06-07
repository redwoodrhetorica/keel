# Keel: B-rep Geometry Kernel Architecture Design

**Date:** 2026-06-07
**Status:** Approved pending user review
**Working name:** Keel (the structural backbone a ship is built on). Final crate name confirmed against crates.io availability before first publish. "Parasolid" is a Siemens trademark and is used in this document only as a benchmark reference.

## 1. Mission

Build an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambitions: NURBS and analytic geometry, full boundary-representation topology, robust boolean operations, and tolerant modeling. Primary goal is a credible open-source alternative; production use is the stretch goal if quality permits.

### Success criteria

- **Proof milestone (M7):** boolean operations (union, intersection, subtraction) on NURBS-bounded solids that survive large-scale randomized property testing and fuzzing without crashes or invariant violations.
- The kernel is adoptable with `cargo add keel`, builds on stable Rust with no nightly features, and runs identically on Windows, Linux, and macOS.
- Every public operation returns `Result`; library code never panics on any input (panics are bugs, hunted by fuzzing).

### Non-goals (initial)

- Non-manifold modeling semantics (the data structure permits it; semantics deferred).
- Fillets/blends, shelling, sheet-metal, healing, history/parametrics (post-M7 long tail).
- STEP/IGES import (post-M7; STL/OBJ export ships early for visibility).
- GPU evaluation, exact-arithmetic modeling mode.

## 2. Core architectural decisions

### D1. Language: Rust (stable, no nightly)

Decided after extended debate against C, C++, and C#. Deciding factors, given that the developer is an AI working across many sessions:

- Ownership and the type system act as enforced institutional memory: invariants established early cannot be silently violated by later sessions.
- The dominant kernel-killing bug class (memory corruption during boolean topology surgery, manifesting far from its cause) is unrepresentable in safe Rust, and is the bug class least suited to a run/read/edit debugging loop.
- Test economics: `proptest`, `cargo fuzz`, and `criterion` make massive generated testing nearly free, and generated test volume is this project's only viable substitute for decades of industrial regression data.
- Strategy: no incumbent Rust B-rep kernel exists; `cargo add` adoption, WASM demos, and PyO3 bindings are structural advantages for an OSS project. A C++ kernel competes with OCCT's gravity forever.
- A C ABI export layer is deferred until the public API stabilizes (post-M7).

### D2. Numerics: tolerant modeling on f64, exact predicates at decision points

*(Amended per research synthesis, docs/research/00-synthesis.md.)*

- All geometry arithmetic in `f64`. No exact-arithmetic modeling mode (CGAL-style exactness is incompatible with freeform NURBS performance and with imported real-world data; no exact paradigm exists for NURBS intersection curves at all).
- **EPICK discipline codebase-wide:** no combinatorial decision ever branches on an unfiltered f64 sign. Every sign-critical predicate (orientation, incidence, point-vs-plane) runs a three-tier cascade: semi-static f64 filter, interval-arithmetic fallback, exact floating-point-expansion evaluation (Shewchuk-style; `robust` crate or vetted port).
- **Indirect predicates** (Attene 2020) for decisions on constructed points (line-plane, three-plane intersections): exactness extends through linear constructions at near-float cost. Extending the exact island to quadric constructions is a flagged investigation.
- **Per-entity local tolerances with propagation are the primary tolerance model from day one**, not an add-on. The session-wide linear resolution (1e-8 in a normalized model box; angular 1e-11, Parasolid's published figures) is the floor/default only. A single global epsilon is explicitly rejected; every robustness track of the literature review converged on this. Tolerance policy is centralized in `keel-math`; no inline epsilons.
- Topology enforces watertightness; geometry is never assumed exact. (Trimmed-NURBS patches are intrinsically gappy; the literature is unanimous that watertightness is a topological contract, not a geometric fact.)

### D3. Topology: non-manifold cellular B-rep, regions first-class, arena storage

*(Rewritten per wave-2 research synthesis, docs/research/01-synthesis-wave2.md, verdict V1. Wave 1 said manifold-first; the kernel-layer literature reversed it: every kernel that deferred non-manifoldness paid more later and delivered less, with OCCT as the documented cautionary case.)*

- Entity hierarchy: `Body -> Region -> Shell -> Face -> Loop -> Coedge(fin) -> Edge -> Vertex`. **Regions are first-class and partition all of 3D space** (every body has one infinite void region; the union of regions is all of space), the Parasolid model. Manifold bodies are restricted cases of the general body, not the native assumption.
- Data structure: **PES-class** (Lee & Lee partial entity structure): half-edge lineage keeps the manifold common case fast; partial entities materialize only at genuine non-manifold junctions (3+ faces per edge, mixed dimension, sheet-in-solid). Radial-edge expressiveness at roughly half the storage.
- All entities live in slot-map arenas keyed by typed generational handles (`FaceId`, `EdgeId`, ...). No interior pointers anywhere. Handle reuse is detected by generation mismatch.
- All topology mutation flows through a proven validity-preserving operator set: the Mantyla manifold five on the manifold path, extended per Masuda's complex-based non-manifold operators and Lee & Lee's PES operator set. Higher layers compose operators; they never perform freestyle graph edits.
- Validity checking: the single-scalar Euler-Poincare identity holds only on manifold shells; it is asserted there, and supplemented globally by structural checks (radial-cycle consistency, region closure, orientation) and the LAR chain-complex oracle (boundary-of-boundary equals zero). Debug builds run the full validator after every mutating operation.
- Implementation order is still manifold-semantics-first (M3 builds manifold solids), but the representation, operator vocabulary, and validator are non-manifold-capable from the first commit.

### D4. Geometry: analytics first-class, NURBS general, exhaustive enum dispatch

- Curve types: line, circle, ellipse, NURBS curve. Surface types: plane, cylinder, cone, sphere, torus, NURBS surface. (Parabola/hyperbola added when a consumer needs them.)
- Analytic types are never converted to NURBS for computation. Closed-form paths (plane-cylinder, line-sphere, ...) are exact, fast, and robust; discarding them is the canonical amateur-kernel mistake.
- Dispatch via Rust enums and exhaustive `match`: adding a geometry type produces compile errors at every site that fails to handle it.
- Geometry and topology are separate stores linked by handles. Faces reference surfaces; edges reference 3D curves; every coedge carries a pcurve (the edge's image in its face's surface parameter space). Pcurves are mandatory, not optional: trimming, classification, and booleans depend on them.
- Curves and surfaces expose: evaluation, derivatives (at least 2nd order), parameter bounds and periodicity, closest-point projection, and bounding boxes.
- **Homogeneous 4D policy (wave-2, A1):** NURBS store control points as (wx, wy, wz, w); every algorithm (evaluation, knot insertion, Oslo refinement, degree elevation, splitting, derivatives) runs as the non-rational algorithm on 4D points, dividing only at the API boundary. Weights strictly positive and canonicalized. Working degree capped; per-span Bezier extraction operators precomputed for delicate numerics. Power basis is banned.
- **Constructed geometry policy (wave-2, A2):** offsets of freeform geometry, fillets (trimmed canal surfaces), and sweeps are stored in their most exact natural form (procedural definitions; exact rational forms where they exist: PH curves, canal/PN surfaces), with tolerance-controlled NURBS baked at consumption boundaries. Every baked approximation carries {procedural source, achieved deviation, requested tolerance}. The approximation is a cache, not the truth.
- The surface enum leaves headroom for a `FacetSurface` variant (Parasolid Convergent Modeling lesson): mesh faces as first-class topology citizens later, without rearchitecting dispatch.

### D5. Error handling

- Every fallible operation returns `Result<T, KernelError>` with structured error variants (tolerance unsatisfiable, intersection non-convergence, degenerate input, invalid topology, ...).
- No `unwrap`/`expect`/`panic!` in library code paths. Fuzzing enforces this continuously.

### D6. One solver behind every query

*(Added per research synthesis; pattern from Elber/Kim's IRIT solver line.)*

- A single central **Bernstein-basis subdivision multivariate solver** (Projected Polyhedron with Mourrain-Pavone reduction) lives in the math layer and backs every polynomial-system query in the kernel: curve-curve and curve-surface intersection, surface-surface start points, closest-point projection, interrogation.
- **Bezier clipping** (Sederberg-Nishita) is the quadratically convergent local refiner; **Yuksel (HPG 2022) and Blinn-style solvers** handle fixed low degrees (the analytic-primitive hot path). Naive Cardano/Ferrari and the power basis are banned.
- Evaluation follows Piegl-Tiller (de Boor / de Casteljau) with per-span Bezier extraction for stability at high degree; the IGA-hardened refinement toolkit (knot insertion, degree elevation, Bezier decomposition) is core, with the invariant that refinement preserves geometry exactly.
- Rationale: ad-hoc per-query numerics is where robustness investment goes to die; concentrating it in one solver makes every hardening improvement global.

### D7. Machine learning policy

*(Added per research synthesis, track F; sharpened per user direction.)*

- **Primary mode: ML as an offline development weapon, not a runtime component.** Brute-force search, learned mutators, and black-box optimization attack problems we cannot theory our way out of: mining adversarial configurations that break intersectors and booleans, discovering tolerance and filter thresholds no published analysis derives, finding counterexamples to design assumptions, and growing the regression corpus. Offline results ship as ordinary constants, test cases, and corpus entries; the kernel binary contains no model. Embedding a model at runtime (e.g., Newton seed prediction) is permitted but optional, and only ever under the propose-then-certify contract below.
- **Nothing learned sits on the correctness path.** Published accuracy ceilings for neural geometry (about 1e-3 of bounding box) are six orders of magnitude short of kernel decisions. The binding contract is propose-then-certify: ML may seed, rank, tune, and generate tests; classical machinery certifies every output.
- Sanctioned uses, in roadmap order: ML-guided fuzzing with learned mutators against kernel invariants (M3+), Newton seed prediction for projection/tracing (M2+), degeneracy prediction for preemptive precision escalation (M5+), SSI algorithm-portfolio selection (M5+), Bayesian optimization of thresholds against the regression corpus (M6+).
- Experiment designs (hypothesis, data source, model class, metric, certification route) live in docs/research/f-ml-for-geometry.md.

### D8. Dependencies: minimal and deliberate

- Own small linear algebra in `keel-math` (Vec2/3/4, Mat2/3/4, transforms): kernels need full control of numerics and the types are public API surface.
- Exact predicates: `robust` crate or vetted internal port.
- Dev-dependencies only: `proptest`, `criterion`, `cargo-fuzz` targets.
- Everything else is written in-house; tolerance control is the entire game and third-party numerics hide their epsilons.

### D9. Identity, determinism, and sessions (day-one structural mandates)

*(Added per wave-2 research synthesis, verdict V2: persistent naming and session machinery cannot be retrofitted; FreeCAD's topological-naming saga is the cost of trying.)*

- **Lineage on every operation:** every face/edge/vertex an operation produces carries the creating operation id, the operand entity ids it descends from, and a derivation kind in {created, modified, generated, split-child, merge-result, deleted}. Split/merge are first-class events with ordinals. No operation may produce anonymous topology. References from layers above the kernel are queries over lineage (Cascaval/OnShape model), not stored indices.
- **Object-space anchor** (a representative 3D point) stored per named entity as the disambiguator that survives reparameterization.
- **Determinism is an API guarantee:** stable iteration order over topology, deterministic entity-id assignment, deterministic tolerance and numerical decisions. No naming, query, or replay scheme is sound without it.
- **Copy-on-write persistent topology** is the substrate for undo/redo, named checkpoints (pmark equivalents), and automatic transactional atomicity: a failed operation drops its work-in-progress version and the model is untouched (Parasolid's known-state invariant, free under structural sharing). Partition-style roll units map to Rust ownership (`&mut Partition` exclusive, `&Partition` shared reads).
- **Call journaling (record/replay)** of kernel operations from the start: the primary regression-capture, fuzzing-corpus, and bug-reproduction tool.

## 3. Workspace structure

Cargo workspace, dependency-ordered (each crate depends only on those above it):

| Crate | Responsibility |
|---|---|
| `keel-math` | Vec/Mat types, transforms, intervals, tolerance model, exact predicates, polynomial root finders, Newton/quasi-Newton solvers, quadrature |
| `keel-geom` | Curve and surface types, evaluation, derivatives, closest-point, bounding boxes, fitting/interpolation |
| `keel-topo` | B-rep arenas and handles, Euler operators, traversal iterators, validator |
| `keel-construct` | Primitive solids (block, cylinder, sphere, cone, torus), extrude, revolve, sweep |
| `keel-isect` | Curve-curve, curve-surface, surface-surface intersection |
| `keel-bool` | Boolean engine: imprint, classification, stitching |
| `keel-tess` | Watertight tessellation of faces/bodies for visualization and export |
| `keel-io` | STL/OBJ export (early); STEP (post-M7) |
| `keel` | Public facade: stable, documented API over opaque handles |

## 4. Testing strategy (first-class subsystem)

- **Property tests** (`proptest`) in every crate from its first commit. Examples: NURBS evaluation against de Boor reference for random knot vectors; closest-point results verified by perturbation; extruded solids pass the validator for random profiles.
- **Invariant validator** in `keel-topo` run after every mutating op in debug builds and after every public op in the test suite.
- **Fuzzing** (`cargo fuzz`) for every intersector and for the boolean engine: random and adversarial inputs must never panic, hang, or return invalid topology.
- **Mass-property oracles** for booleans: vol(A union B) = vol(A) + vol(B) - vol(A intersect B) must hold (within tolerance) for arbitrary random solid pairs. Volume/centroid/inertia via divergence-theorem integration in `keel-construct`. This provides a correctness oracle requiring no hand-authored expected values.
- **Golden regression suite**: every bug ever found becomes a permanent named test case.
- **Benchmarks** (`criterion`) on evaluation, intersection, and boolean hot paths to catch performance regressions.

## 5. Roadmap

- **M1. Foundations:** `keel-math` complete (types, tolerances, predicates, solvers), CI with tests/fuzz/bench wired, workspace scaffolded.
- **M2. Geometry:** all curve/surface types with evaluation, derivatives, closest-point; property-tested against references.
- **M3. Topology and first solids:** arenas, Euler ops, validator; primitive construction; tessellation; STL export. First visible output. Includes the day-one tolerant data model (research track D position): per-entity tolerance fields AND the tolerant-edge concept (approximate curve geometry plus a tolerance bounding its deviation from the true intersection), with propagation rules sketched even before booleans consume them. **Gate before the topology API freezes:** the boolean/SSI pipeline, the D9 lineage/determinism/session hooks, and the PES non-manifold operator set are all designed on paper first, because coedge splitting, tolerant merging, and lineage emission shape the topology API (Fornjot/OCCT/FreeCAD lessons); retrofitting any of them is the documented dead end. Tessellation follows the wave-2 faceter contract: edge-first discretization (each model edge discretized once, both faces consume the identical polyline, watertight by construction) emitting an attributed indexed mesh (per-triangle face id, per-boundary-vertex edge id and parameter, persistent ids for incremental re-faceting).
- **M4. Sweeps and simple intersection:** extrude/revolve; curve-curve and curve-surface intersection with fuzzing. Includes the in-kernel **2D profile subsystem** (wave-2, A3): one traits-parameterized planar arrangement engine over a DCEL, from which region extraction with hole nesting, curved planar booleans, and winding-number offset cleanup all derive; exact line+arc offsetting; nonzero-winding convention aligned with B-rep loop orientation. (Constraint solving stays above the kernel, the D-Cubed-beside-Parasolid boundary.) Root-engine portfolio: Bezier clipping first pass, cubic hybrid clipping (order-4) at tight tolerance; Hu-Wallner second-order geometric iteration as the projection polisher behind Selimovic/circular-clipping global pruning.
- **M5. Surface-surface intersection:** topology-first architecture per the research synthesis. Closed forms for analytic pairs, with quadric-quadric via the exact QI parameterization (Dupont et al.), never Levin's pencil method. For freeform: normal-cone loop-detection gate (Sederberg-Meyers, Hohmeyer) before any marching, Interval Projected Polyhedron start points, predictor-corrector tracing with Ye-Maekawa differential geometry and per-step Krawczyk certification. Overlap/coincidence extraction is a first-class result kind, and tangential cases are designed for at the start, not deferred: the literature identifies them as the dominant boolean failure mode. Track the Yang-Jia-Yan papers (2023-2026), the current state of the art. **Output contract:** every SSI result is a tolerance-bounded NURBS approximation carried in both surfaces' parameter domains and in model space, whose certified error bound becomes the consuming tolerant edge's tolerance (the 2025 CAGD tracing contract); type dispatch is a single chokepoint so analytic pairs never silently fall through to marching. Residual critical cases in topology resolution get a controlled-perturbation fallback (Mukundan et al. 2008). The hardest single component; expect the longest schedule.
- **M6. Booleans on analytic solids:** pipeline per the research synthesis: BVH broad phase, local imprint along intersection curves only (no global arrangement, the EMBER lesson), winding-number-vector classification (Zhou/Jacobson, more robust than neighborhood walking and gives n-ary booleans free; for curved B-rep containment, generalized winding numbers per Spainhour-Weiss TOG 2025, BSD reference implementation in LLNL Axom), tolerant stitch with gap covering. Symbolic perturbation for degeneracies; coincidence-first mindset (exact touching is CAD's common case); caller-tunable fuzzy mode planned. Mass-property oracles online.
- **M7. Booleans on NURBS-bounded solids:** the proof milestone.
- **Post-M7 long tail:** fillets/blends, shelling, STEP import and healing, deeper tolerant modeling, C ABI layer, PyO3 bindings, WASM demos.

Each milestone gets its own brainstorm/spec/plan cycle before implementation; this document governs the architecture they plug into.

## 6. Risks and mitigations

- **Surface-surface intersection complexity (M5)** is the schedule's dominant risk. Mitigation: exhaustive analytic special-casing first (most CAD geometry is analytic), marching only for the genuinely freeform remainder, and fuzzing from the first week of M5.
- **Tolerance-model mistakes** are unfixable late. Mitigation: centralized tolerance policy in `keel-math` (D2), per-entity tolerance fields from day one, no inline epsilons (enforced by review/lint).
- **Scope creep** toward Parasolid's full surface (thousands of API functions). Mitigation: this spec's non-goals list and the single proof milestone; nothing ships before M7 except via the roadmap.
- **Performance regressions** from safety abstractions. Mitigation: criterion benchmarks from M1; arena/handle design is already the known-fast layout (struct-of-arrays friendly, cache-coherent).
- **Robustness without an industrial regression base.** Production kernels lean on decades of customer models (C3D cites 500k+). Mitigation: the corpus is a first-class artifact from M1: generated adversarial cases, ABC-dataset imports as they become readable, ML-guided fuzzing findings, and every bug ever found. No public NURBS-boolean robustness benchmark exists; publishing ours doubles as a community contribution.

## 8. Research basis

Six literature-review tracks with full citations live in `docs/research/` (robust numerics, NURBS/freeform, surface intersection, booleans/tolerant modeling, kernel architecture lessons, ML for geometry), consolidated in `docs/research/00-synthesis.md`. **Wave 2** added two deep-dive collections: `docs/research/kernel/` (10 dossiers: topology/Euler, non-manifold/cellular, local ops, constraints/2D profiles, tessellation, interrogation/HLR, persistent naming/sessions, kernel engineering, mesh/implicit hybrid, Minkowski/medial/midsurface) and `docs/research/nurbs/` (6 dossiers: foundations, core algorithms, fitting, intersection/projection, offsets/blends/sweeps, applications/extensions), consolidated in `docs/research/01-synthesis-wave2.md`, which records two verdict changes applied to this spec (D3 non-manifold/regions-first; D9 lineage/determinism/sessions) and the shared-engine consolidation map. FP-contraction policy (wave-2, A8): no fast-math flags ever; `mul_add` only where intended; predicate code isolated from FMA contraction. The synthesis lists the binding design deltas applied to this spec and a flagged reading list per milestone (Golovanov before M3, Patrikalakis-Maekawa-Cho before M4/M5, Yang-Jia-Yan before M5, the mesh-arrangement corpus before M6). Topology future-proofing note from track E: the radial cycle around each edge is structured to hold 3+ coedges so partial-entity non-manifold support remains an extension, not a rewrite; persistent/immutable arenas are flagged as a candidate for Parasolid-style session rollback.

## 7. Public API sketch (facade crate)

Parasolid-inspired: a session owning all entities, opaque typed handles, every operation fallible.

```rust
let mut s = keel::Session::new();
let block = s.create_block(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0))?;
let hole  = s.create_cylinder(Vec3::new(5.0, 5.0, -1.0), Axis::Z, 2.0, 12.0)?;
let part  = s.subtract(block, hole)?;          // consumes inputs, returns BodyId
let mesh  = s.tessellate(part, TessOptions::default())?;
keel_io::write_stl("part.stl", &mesh)?;
```

Internals are idiomatic Rust; the facade keeps the surface small, stable, and bindable (future C ABI maps 1:1 onto facade functions).
