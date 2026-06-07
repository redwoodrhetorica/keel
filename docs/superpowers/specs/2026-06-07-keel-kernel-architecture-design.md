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
- **Indirect predicates** (Attene 2020) for decisions on constructed points (line-plane, three-plane intersections): exactness extends through linear constructions at near-float cost.
- **Predicate strategy tiered by algebraic degree** *(amended per synthesis v2, docs/research/00-synthesis-v2.md, delta 13)*: linear decisions exact (above); circle/conic decisions exact via degree-2 algebraic number comparison (affordable); quadric intersection decisions exact-when-cheap (QI: the curve is rational whenever singular, one square root in the smooth case; escalate only when the data demands it); spline decisions are certified-numeric topology (interval subdivision, Krawczyk) over tolerant geometry, with no exactness pretense. Rationale: CAD models are degeneracy-dense by design, exactly where filters fail; see docs/research/kernel/11-curved-exact-predicates.md.
- **Per-entity local tolerances with propagation are the primary tolerance model from day one**, not an add-on. The session-wide linear resolution (1e-8 in a normalized model box; angular 1e-11, Parasolid's published figures) is the floor/default only. A single global epsilon is explicitly rejected; every robustness track of the literature review converged on this. Tolerance policy is centralized in `keel-math`; no inline epsilons.
- Topology enforces watertightness; geometry is never assumed exact. (Trimmed-NURBS patches are intrinsically gappy; the literature is unanimous that watertightness is a topological contract, not a geometric fact.)

### D3. Topology: arena storage, generational handles, Euler operators

*(Amended per synthesis v2, docs/research/00-synthesis-v2.md, deltas 9, 11, 12.)*

- Entity hierarchy: `Body -> Region -> Shell -> Face -> Loop -> Coedge -> Edge -> Vertex`, with coedges pairing across adjacent faces (radial/partner links). Regions are first-class from day one (Parasolid's model) even while semantics stay manifold; the partial entity structure (Lee & Lee) is the designated non-manifold core if/when semantics extend.
- All entities live in slot-map arenas keyed by typed generational handles (`FaceId`, `EdgeId`, ...). No interior pointers anywhere. Handle reuse is detected by generation mismatch.
- All topology mutation flows through Euler operators (MEV, MEF, KEMR, etc.), each of which preserves the Euler-Poincare invariant. Higher layers (booleans) compose Euler operators; they do not perform freestyle graph edits.
- Manifold semantics first. The coedge/radial structure is chosen so non-manifold extension is additive, not a rewrite.
- **Lineage events are a kernel primitive:** every mutating operation emits per-output-entity records (operation id, operand ids, derivation kind: created/modified/split/merged/deleted). Every persistent-naming scheme in the literature consumes exactly this; it cannot be retrofitted after the topology layer freezes. Attributes get split/merge/delete callbacks on the same mechanism. No naming layer ships before post-M7, but the emission contract is binding at M3.
- **Copy-on-write persistent arenas** (promoted from investigation to commitment): structural sharing provides undo, atomic rollback of failed operations (D5 requires that a failed boolean never corrupts the model), journaling snapshots, and cacheable intermediate bodies.
- Debug builds run a full validator (Euler-Poincare formula, shell closure, coedge pairing, loop orientation consistency, geometry-topology consistency) after every mutating operation.

### D4. Geometry: analytics first-class, NURBS general, exhaustive enum dispatch

- Curve types: line, circle, ellipse, NURBS curve. Surface types: plane, cylinder, cone, sphere, torus, NURBS surface. (Parabola/hyperbola added when a consumer needs them.)
- Analytic types are never converted to NURBS for computation. Closed-form paths (plane-cylinder, line-sphere, ...) are exact, fast, and robust; discarding them is the canonical amateur-kernel mistake.
- Dispatch via Rust enums and exhaustive `match`: adding a geometry type produces compile errors at every site that fails to handle it.
- Geometry and topology are separate stores linked by handles. Faces reference surfaces; edges reference 3D curves; every coedge carries a pcurve (the edge's image in its face's surface parameter space). Pcurves are mandatory, not optional: trimming, classification, and booleans depend on them.
- Curves and surfaces expose: evaluation, derivatives (at least 2nd order), parameter bounds and periodicity, closest-point projection, and bounding boxes.

### D5. Error handling

- Every fallible operation returns `Result<T, KernelError>` with structured error variants (tolerance unsatisfiable, intersection non-convergence, degenerate input, invalid topology, ...).
- No `unwrap`/`expect`/`panic!` in library code paths. Fuzzing enforces this continuously.

### D6. One solver behind every query

*(Added per research synthesis; pattern from Elber/Kim's IRIT solver line.)*

- A single central **Bernstein-basis subdivision multivariate solver** (Projected Polyhedron with Mourrain-Pavone reduction) lives in the math layer and backs every polynomial-system query in the kernel: curve-curve and curve-surface intersection, surface-surface start points, closest-point projection, interrogation. Interrogation queries (silhouettes, draft isoclines, curvature extrema, umbilics) are zero-set queries against this same solver *(synthesis v2, delta 22)*.
- **Cubic hybrid clipping** (order-4 convergence at simple roots) is the default univariate root engine, with Bezier clipping (Sederberg-Nishita) as the simpler fallback; **Yuksel (HPG 2022) and Blinn-style solvers** handle fixed low degrees (the analytic-primitive hot path). Naive Cardano/Ferrari and the power basis are banned. *(Amended per synthesis v2, delta 14.)*
- Evaluation follows Piegl-Tiller (de Boor / de Casteljau) with per-span Bezier extraction for stability at high degree; the IGA-hardened refinement toolkit (knot insertion, degree elevation, Bezier decomposition) is core, with the invariant that refinement preserves geometry exactly. Control points are stored in homogeneous 4D, dividing last; the supported working degree is capped and documented (basis conditioning grows like p times 2^p, knot-independent). *(Synthesis v2, delta 14.)*
- Rationale: ad-hoc per-query numerics is where robustness investment goes to die; concentrating it in one solver makes every hardening improvement global.

### D7. Machine learning policy

*(Added per research synthesis, track F.)*

- **Nothing learned sits on the correctness path.** Published accuracy ceilings for neural geometry (about 1e-3 of bounding box) are six orders of magnitude short of kernel decisions. The binding contract is propose-then-certify: ML may seed, rank, tune, and generate tests; classical machinery certifies every output.
- Sanctioned uses, in roadmap order: ML-guided fuzzing with learned mutators against kernel invariants (M3+), Newton seed prediction for projection/tracing (M2+), degeneracy prediction for preemptive precision escalation (M5+), SSI algorithm-portfolio selection (M5+), Bayesian optimization of thresholds against the regression corpus (M6+).
- Experiment designs (hypothesis, data source, model class, metric, certification route) live in docs/research/f-ml-for-geometry.md.

### D8. Dependencies: minimal and deliberate

- Own small linear algebra in `keel-math` (Vec2/3/4, Mat2/3/4, transforms): kernels need full control of numerics and the types are public API surface.
- Exact predicates: `robust` crate or vetted internal port.
- Dev-dependencies only: `proptest`, `criterion`, `cargo-fuzz` targets.
- Everything else is written in-house; tolerance control is the entire game and third-party numerics hide their epsilons.

### D9. Determinism contract

*(Added per synthesis v2, docs/research/00-synthesis-v2.md, delta 10.)*

Identical inputs produce identical outputs across platforms, sessions, and thread counts. This is an API promise (naming, caching, collaboration, and replay debugging above the kernel all depend on it), enforced by:

- FP contraction pinned off in predicate code (FMA fusion can break orientation-predicate antisymmetry and silently corrupt topology); no fast-math anywhere.
- Correctly-rounded transcendentals where results feed combinatorial decisions; the system libm is the largest cross-platform reproducibility hole.
- No iteration over address- or hash-ordered containers in any result-affecting path (Rust `HashMap` is randomized by design; use ordered containers keyed on stable IDs).
- Deterministic parallel reduction orders (fixed-shape trees, not work-stealing order).
- Exact double round-tripping in serialization (shortest round-trip or hex floats; lossy decimal silently breaks reproducibility on reload).
- Cross-compiler/cross-platform differential CI on the regression corpus.

### D10. License and patent policy

*(Added per synthesis v2, docs/research/00-synthesis-v2.md, delta 21. Informational survey, not legal advice; see docs/research/kernel/18-patent-ip-landscape.md.)*

- Dual license **MIT OR Apache-2.0** (the Apache patent grant matters for a geometry kernel).
- The classical core (NURBS, Euler operators, booleans, classic blends including variable-radius and setback, tessellation) rests on expired or never-patented art. The original T-spline patent expired March 2024.
- Three active no-fly zones, each with a documented safe alternative: automatic constraint inference during direct edits (Siemens, to ~2034; explicit user-applied constraints are fine), mixed facet/B-rep faces within a single body (convergent modeling, ~2035+; separate bodies with boundary conversion are fine), and U-spline-class refinement (~2037; THB-splines are the safe local-refinement basis).

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
| `keel-tess` | Watertight tessellation: each model edge discretized once and shared by both faces (watertight by construction); metric-driven (u,v) meshing under the first fundamental form; facet-to-face/edge attribution in the output |
| `keel-io` | STL/OBJ export (early); STEP (post-M7). Versioned schema with skip-unknown forward compatibility; exact double round-tripping (D9); hostile-input hardening (count-vs-filesize validation, fuzzed importers) |
| `keel` | Public facade: stable, documented API over opaque handles |

## 4. Testing strategy (first-class subsystem)

- **Property tests** (`proptest`) in every crate from its first commit. Examples: NURBS evaluation against de Boor reference for random knot vectors; closest-point results verified by perturbation; extruded solids pass the validator for random profiles.
- **Invariant validator** in `keel-topo` run after every mutating op in debug builds and after every public op in the test suite.
- **Fuzzing** (`cargo fuzz`) for every intersector and for the boolean engine: random and adversarial inputs must never panic, hang, or return invalid topology.
- **Mass-property oracles** for booleans: vol(A union B) = vol(A) + vol(B) - vol(A intersect B) must hold (within tolerance) for arbitrary random solid pairs. Volume/centroid/inertia via divergence-theorem integration in `keel-construct` with smoothness-aware quadrature. This provides a correctness oracle requiring no hand-authored expected values.
- **Comparison cascade as the shape oracle** *(synthesis v2, delta 20)*: entity counts, then mass properties, then rigid-invariant fingerprints (inertia eigenvalues, D2 distributions), then registration plus sampled deviation, then entity correspondence. CAx-IF/LOTAR validation properties (volume/area/centroid/cloud-of-points) implemented natively: they are both the oracle's cheap stages and the interop handshake with commercial validators.
- **Pass-rate robustness reporting** (fTetWild methodology): run operations over the full ABC corpus and report pass rates; every release publishes the numbers.
- **Golden regression suite**: every bug ever found becomes a permanent named test case.
- **Benchmarks** (`criterion`) on evaluation, intersection, and boolean hot paths to catch performance regressions.

## 5. Roadmap

- **M1. Foundations:** `keel-math` complete (types, tolerances, predicates, solvers), CI with tests/fuzz/bench wired, workspace scaffolded.
- **M2. Geometry:** all curve/surface types with evaluation, derivatives, closest-point; property-tested against references.
- **M3. Topology and first solids:** arenas, Euler ops, validator; primitive construction; tessellation; STL export. First visible output. The lineage-emission contract and Region entity (D3) freeze here; read Mantyla, Lee & Lee (PES), and the naming dossier (docs/research/kernel/07) before this milestone.
- **M4. Sweeps and simple intersection:** extrude/revolve; curve-curve and curve-surface intersection with fuzzing.
- **M5. Surface-surface intersection:** topology-first architecture per the research synthesis. Closed forms for analytic pairs, with quadric-quadric via the exact QI parameterization (Dupont et al.), never Levin's pencil method. For freeform: normal-cone loop-detection gate (Sederberg-Meyers, Hohmeyer) before any marching, Interval Projected Polyhedron start points, predictor-corrector tracing with Ye-Maekawa differential geometry and per-step Krawczyk certification. Overlap/coincidence extraction is a first-class result kind, and tangential cases are designed for at the start, not deferred: the literature identifies them as the dominant boolean failure mode. Track the Yang-Jia-Yan papers (2023-2026), the current state of the art. The hardest single component; expect the longest schedule.
- **M6. Booleans on analytic solids:** pipeline per the research synthesis: BVH broad phase, local imprint along intersection curves only (no global arrangement, the EMBER lesson), winding-number-vector classification (Zhou/Jacobson, more robust than neighborhood walking and gives n-ary booleans free), tolerant stitch with gap covering. Symbolic perturbation for degeneracies; coincidence-first mindset (exact touching is CAD's common case); caller-tunable fuzzy mode planned. Mass-property oracles online.
- **M7. Booleans on NURBS-bounded solids:** the proof milestone.
- **Post-M7 long tail** *(expanded per synthesis v2)*: fillets/blends (represented procedurally as trimmed canal surfaces with NURBS caches), shelling, local operations built on the tweak primitive (change-face-surfaces + extend-and-reintersect; delete-face stays a dedicated primitive; M5/M6 imprint machinery is designed for reuse here), surface extension as a bounded validated service, STEP import with the healing pipeline (stitch, simplify via canonical recovery, geombuild) and the one-engine fitting core (L2/min-zone/MIC/MCC modes), canonical geometry recovery (recognize-fit-certify-substitute), deeper tolerant modeling, C ABI layer, PyO3 bindings, WASM demos. Boundaries stay firm: constraint solving, motion, parametric regeneration, and PMI authoring live above the kernel; the kernel ships their enumerated obligations (synthesis v2, delta 22).

Each milestone gets its own brainstorm/spec/plan cycle before implementation; this document governs the architecture they plug into.

## 6. Risks and mitigations

- **Surface-surface intersection complexity (M5)** is the schedule's dominant risk. Mitigation: exhaustive analytic special-casing first (most CAD geometry is analytic), marching only for the genuinely freeform remainder, and fuzzing from the first week of M5.
- **Tolerance-model mistakes** are unfixable late. Mitigation: centralized tolerance policy in `keel-math` (D2), per-entity tolerance fields from day one, no inline epsilons (enforced by review/lint).
- **Scope creep** toward Parasolid's full surface (thousands of API functions). Mitigation: this spec's non-goals list and the single proof milestone; nothing ships before M7 except via the roadmap.
- **Performance regressions** from safety abstractions. Mitigation: criterion benchmarks from M1; arena/handle design is already the known-fast layout (struct-of-arrays friendly, cache-coherent).
- **Robustness without an industrial regression base.** Production kernels lean on decades of customer models (C3D cites 500k+). Mitigation: the corpus is a first-class artifact from M1: generated adversarial cases, ABC-dataset imports as they become readable, ML-guided fuzzing findings, and every bug ever found. No public NURBS-boolean robustness benchmark exists; publishing ours doubles as a community contribution.
- **Patent exposure** in three mapped zones (D10). Mitigation: the no-fly list with safe alternatives is part of design review for direct modeling, hybrid mesh work, and local refinement; everything currently on the roadmap rests on expired or never-patented art.
- **Determinism erosion** under optimization pressure. Mitigation: D9 is enforced by cross-platform differential CI from M1; any nondeterminism is a release blocker, not a known issue.

## 8. Research basis

Six literature-review tracks with full citations live in `docs/research/` (robust numerics, NURBS/freeform, surface intersection, booleans/tolerant modeling, kernel architecture lessons, ML for geometry), consolidated in `docs/research/00-synthesis.md` (deltas 1-8). A subsequent deep-dive corpus of 32 files and roughly 800 sources lives in `docs/research/nurbs/` (7 files: foundations, core algorithms, fitting, intersection, constructed surfaces, applications) and `docs/research/kernel/` (25 files: topology through sheet metal), consolidated in `docs/research/00-synthesis-v2.md` (deltas 9-22, applied to this spec as the marked amendments). Each collection has a `00-index.md` with headline findings and per-file pointers. Reading lists per milestone live in both syntheses (Golovanov and Mantyla before M3, Patrikalakis-Maekawa-Cho before M4/M5, Yang-Jia-Yan before M5, the mesh-arrangement corpus before M6, the determinism dossier before M1). Topology future-proofing note from track E, confirmed by kernel/02: the radial cycle around each edge holds 3+ coedges so partial-entity non-manifold support remains an extension, not a rewrite.

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
