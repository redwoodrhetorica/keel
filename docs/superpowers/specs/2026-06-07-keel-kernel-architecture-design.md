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

- All geometry arithmetic in `f64`. No exact-arithmetic modeling mode (CGAL-style exactness is incompatible with freeform NURBS performance and with imported real-world data).
- Session-wide linear resolution of 1e-8 in a normalized model box (Parasolid's published figures: 1000-unit box, 1e-8 resolution, angular resolution 1e-11). Tolerances are centralized in `keel-math`; no magic epsilons scattered through code.
- Per-entity tolerances (tolerant vertices/edges) are part of the data model from day one, even though tolerant-modeling algorithms deepen later. Retrofitting per-entity tolerance into a kernel is a known disaster; carrying the fields early is cheap.
- Sign-critical decisions (orientation, incidence, point-vs-plane) use adaptive-precision exact predicates (Shewchuk-style; the `robust` crate or a vetted port). Float for arithmetic, exact for decisions.

### D3. Topology: arena storage, generational handles, Euler operators

- Entity hierarchy: `Body -> Shell -> Face -> Loop -> Coedge -> Edge -> Vertex`, with coedges pairing across adjacent faces (radial/partner links).
- All entities live in slot-map arenas keyed by typed generational handles (`FaceId`, `EdgeId`, ...). No interior pointers anywhere. Handle reuse is detected by generation mismatch.
- All topology mutation flows through Euler operators (MEV, MEF, KEMR, etc.), each of which preserves the Euler-Poincare invariant. Higher layers (booleans) compose Euler operators; they do not perform freestyle graph edits.
- Manifold semantics first. The coedge/radial structure is chosen so non-manifold extension is additive, not a rewrite.
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

### D6. Dependencies: minimal and deliberate

- Own small linear algebra in `keel-math` (Vec2/3/4, Mat2/3/4, transforms): kernels need full control of numerics and the types are public API surface.
- Exact predicates: `robust` crate or vetted internal port.
- Dev-dependencies only: `proptest`, `criterion`, `cargo-fuzz` targets.
- Everything else is written in-house; tolerance control is the entire game and third-party numerics hide their epsilons.

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
- **M3. Topology and first solids:** arenas, Euler ops, validator; primitive construction; tessellation; STL export. First visible output.
- **M4. Sweeps and simple intersection:** extrude/revolve; curve-curve and curve-surface intersection with fuzzing.
- **M5. Surface-surface intersection:** analytic-analytic closed forms; general marching with start-point detection, loop detection, and singularity handling. The hardest single component; expect the longest schedule.
- **M6. Booleans on analytic solids:** imprint/classify/stitch pipeline, mass-property oracles online.
- **M7. Booleans on NURBS-bounded solids:** the proof milestone.
- **Post-M7 long tail:** fillets/blends, shelling, STEP import and healing, deeper tolerant modeling, C ABI layer, PyO3 bindings, WASM demos.

Each milestone gets its own brainstorm/spec/plan cycle before implementation; this document governs the architecture they plug into.

## 6. Risks and mitigations

- **Surface-surface intersection complexity (M5)** is the schedule's dominant risk. Mitigation: exhaustive analytic special-casing first (most CAD geometry is analytic), marching only for the genuinely freeform remainder, and fuzzing from the first week of M5.
- **Tolerance-model mistakes** are unfixable late. Mitigation: centralized tolerance policy in `keel-math` (D2), per-entity tolerance fields from day one, no inline epsilons (enforced by review/lint).
- **Scope creep** toward Parasolid's full surface (thousands of API functions). Mitigation: this spec's non-goals list and the single proof milestone; nothing ships before M7 except via the roadmap.
- **Performance regressions** from safety abstractions. Mitigation: criterion benchmarks from M1; arena/handle design is already the known-fast layout (struct-of-arrays friendly, cache-coherent).

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
