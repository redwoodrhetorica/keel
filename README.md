# Keel

**An open-source B-rep solid modeling kernel in Rust.**

Exact topology decisions over tolerant geometry. A "decline, never wrong" contract on every operation.
Pre-alpha, APIs change without notice.

<!-- BADGES: replace the placeholder hrefs once a remote and CI are live -->
[![CI](https://img.shields.io/github/actions/workflow/status/keel-kernel/keel/ci.yml?label=CI)](https://github.com/keel-kernel/keel/actions)
[![License](https://img.shields.io/badge/license-TBD-lightgrey)](#license)
[![Crates.io](https://img.shields.io/badge/crates.io-not%20yet%20published-lightgrey)](https://crates.io)

---

## What and Why

Most open B-rep kernels treat topology as a downstream consequence of floating-point geometry.
Keel inverts that: combinatorial topology decisions (which edge bounds which face, which region is inside or outside) are certified
exact using Shewchuk-class predicates and algebraic real arithmetic at branch points,
while metric geometry (coordinates, parameter values) uses `f64` with explicit per-entity tolerances.

The practical consequence is a defined error contract: an operation either produces
a topologically correct result or it explicitly declines. It does not silently produce a wrong answer.
The three-bucket oracle used in randomized testing enforces this: PASS (correct), DECLINE (refused),
WRONG (zero tolerance). WRONG must be zero.

This approach is the consensus recommendation of the research literature on robust computational geometry.
It is not yet widely shipped in open kernels.

---

## Demo Gallery

All animations are rendered directly from Keel's own tessellation output.
Resolution and shading fidelity reflect the kernel's current state, not a rendering budget.

| | | |
|---|---|---|
| ![boolean-trio](docs/media/boolean-trio.webp) | ![steinmetz](docs/media/steinmetz.webp) | ![clearance-pin](docs/media/clearance-pin.webp) |
| Boolean trio (union / intersect / subtract) | Steinmetz solid (bicylinder intersection) | Clearance-pin assembly boolean |
| ![drill](docs/media/drill.webp) | ![countersink](docs/media/countersink.webp) | ![socket](docs/media/socket.webp) |
| Drill pocket | Countersink | Socket boolean |
| ![fillet-overflow](docs/media/fillet-overflow.webp) | ![corner-blend](docs/media/corner-blend.webp) | ![shell-cutaway](docs/media/shell-cutaway.webp) |
| Fillet overflow (graceful degeneracy) | Corner blend | Shell cutaway |
| ![imprint-push](docs/media/imprint-push.webp) | ![sgc-merge](docs/media/sgc-merge.webp) | ![honest-decline](docs/media/honest-decline.webp) |
| Imprint and push | SGC merge | Honest decline (operation refused, no result) |

<details>
<summary>Full media corpus (remaining demos)</summary>

| File | Caption |
|---|---|
| `blendzoo.webp` | Blend zoo, multiple fillet families side by side |
| `cellular-union.webp` | Cellular union, non-manifold result retained |
| `corners2.webp` | Corner blend variant, second configuration |
| `defeature.webp` | Defeature: round removal |
| `deleteface.webp` | Delete face, heal boundary |
| `draft.webp` | Draft angle applied to a pocket |
| `extrude-revolve.webp` | Extrude and revolve primitives |
| `helix.webp` | Helix sweep |
| `hlr.webp` | Hidden-line removal wireframe |
| `knit.webp` | Sheet knit (open shell closure) |
| `loft.webp` | Loft between profiles |
| `mirror.webp` | Mirror operation |
| `moveface.webp` | Move-face direct edit |
| `multitool.webp` | Multi-tool boolean (several cutters) |
| `nonuniform.webp` | Non-uniform scaling / non-uniform surface |
| `offset.webp` | Offset face |
| `partition.webp` | Partition body by surface |
| `pierce.webp` | Pierce (body-through-body) |
| `primitives.webp` | Primitive constructors: box, cylinder, sphere, cone, torus |
| `revolve_partial.webp` | Partial revolve (sector) |
| `sheets.webp` | Sheet bodies and open shells |
| `slicestack.webp` | Slice-stack cross-section sweep |
| `taperface.webp` | Taper face |
| `unblend.webp` | Unblend (fillet removal) |
| `vfillet.webp` | Variable-radius fillet |
| `winding-cloud.webp` | Generalized winding-number point cloud classification |
| `wiretrim.webp` | Wire trim on a face |

</details>

---

## The Honesty Contract

### Decline, never wrong

Every boolean, blend, and local-edit operation returns one of three outcomes:

| Outcome | Meaning |
|---|---|
| **PASS** | A topologically verified result. Mass properties and mesh volume agree to within tessellation error. |
| **DECLINE** | The operation was not attempted or could not be certified. The input body is unchanged. |
| **WRONG** | This outcome is not permitted. The oracle enforces zero occurrences. |

A DECLINE is a first-class, documented result, not a crash. The `honest-decline` demo above shows this in action.

### Randomized oracle testing

Three-bucket correctness is enforced by a large randomized oracle suite.
Each trial generates a random input, runs the operation, and classifies the result:
PASS if output matches an independently computed reference (exact closed form or a separate code path),
DECLINE if the kernel refused, WRONG if the output disagrees with the reference and was not declined.

<!-- FILL: total trial count and WRONG=0 record, source LOG Addendum XXX (current: see LOG.md Add.193) -->
Across <!-- FILL: N --> randomized trials, the WRONG bucket has been <!-- FILL: 0 (confirm from LOG Addendum XXX) -->.
The DECLINE rate varies by operation class and is documented per-operation in the engineering log.

<!-- FILL: cite strict vs. tolerant lane counts from LOG Addendum XXX, e.g. strict N / tolerant M -->

### Mass-mesh self-consistency gate

For every solid result, the kernel computes mass properties from the B-rep (analytic integrals over faces)
and independently from the tessellation (mesh volume via the divergence theorem).
Agreement within tessellation error is a necessary gate before a result is classified PASS.

<!-- FILL: cite gate threshold and the specific addendum where it was tuned, e.g. LOG Addendum XXX -->

### Fuzz soak

Fuzz harnesses exercise the kernel's parser, solver, and boolean pipeline continuously.
Findings from fuzzing have historically caught real bugs (overflow in polynomial solvers,
bracket midpoint overflow in Newton iteration, denormal-coefficient edge cases).
<!-- FILL: cite total fuzz execution count and soak duration from LOG Addendum XXX, e.g. "N billion executions, M hours" -->

---

## Capabilities

Legend: **S** = shipped and tested, **P** = partial or in progress, **D** = declined by design (not in scope), **?** = placeholder to fill

<!-- FILL: audit this table against the current milestone and mark each row honestly before publishing -->

### Boolean operations

| Operation | Status | Notes |
|---|---|---|
| Union, intersect, subtract (planar faces) | **S** | |
| Union, intersect, subtract (cylinder faces) | **S** | |
| Union, intersect, subtract (cone faces) | **?** | <!-- FILL: confirm from milestone record --> |
| Union, intersect, subtract (sphere faces) | **?** | <!-- FILL: confirm from milestone record --> |
| Union, intersect, subtract (NURBS faces) | **P** | Planned, not yet certified |
| Multi-body (cellular) boolean | **?** | <!-- FILL --> |
| Non-manifold boolean result retention | **S** | First-class regions in topology |

### Blends and fillets

| Operation | Status | Notes |
|---|---|---|
| Constant-radius edge fillet | **S** | |
| Variable-radius fillet | **S** | |
| Corner blend (vertex fillet) | **S** | |
| Fillet overflow / graceful degeneracy | **S** | Documented in `fillet-overflow` demo |
| Chamfer | **?** | <!-- FILL --> |
| Unblend (fillet removal) | **S** | |

### Local direct-edit operations

| Operation | Status | Notes |
|---|---|---|
| Push / pull face | **S** | |
| Draft face | **S** | |
| Taper face | **S** | |
| Move face | **S** | |
| Offset face | **S** | |
| Shell / hollow | **S** | |
| Delete face (heal) | **S** | |
| Defeature (round removal) | **S** | |
| Mirror | **S** | |

### Sheets, knit, partition

| Operation | Status | Notes |
|---|---|---|
| Sheet bodies and open shells | **S** | |
| Knit (close open shell) | **S** | |
| Partition body by surface | **S** | |

### Sweeps, lofts, revolves

| Operation | Status | Notes |
|---|---|---|
| Extrude | **S** | |
| Revolve (full and partial) | **S** | |
| Loft between profiles | **S** | |
| Helix sweep | **S** | |
| Wire trim | **S** | |

### Interrogation

| Operation | Status | Notes |
|---|---|---|
| Mass properties (volume, area, centroid, inertia) | **S** | Analytic integrals per face |
| Cross-section slices | **S** | |
| Hidden-line removal wireframe | **S** | |
| Winding-number point-in-solid | **S** | Generalized winding numbers (Spainhour-Weiss) |

### Import / export

| Format | Status | Notes |
|---|---|---|
| STEP export | **?** | <!-- FILL: confirm from milestone record --> |
| STEP import | **?** | <!-- FILL --> |
| WASM build | **P** | `keel-wasm` crate in workspace; spike in progress |

---

## Verification

### Oracle methodology

For each operation class, Keel maintains an independent oracle: a separately derived closed-form
or a second code path that produces a known-good answer for randomly generated inputs.
The boolean oracle uses generalized winding numbers to classify points in the result body.
The mass-properties oracle uses analytic closed forms (for example, the bicylinder Steinmetz solid
has volume exactly <!-- FILL: e.g. 16/3 r^3 per closed form, cite LOG Addendum XXX -->) against which
the kernel's numeric integral is compared.

Exact closed-form references remove the need to trust the test itself, a known weakness of
differential-testing oracles.

### Fuzz sectors

Fuzzing is organized into sectors, one harness per subsystem:
- `fuzz_boolean`: random boolean inputs, checks WRONG=0 and structural validity
- `fuzz_cyl_boolean`: cylinder-specific boolean geometry
- `fuzz_imprint`: edge imprint pipeline
- Additional sectors covering the polynomial solver, Bernstein root isolation, and NURBS curve evaluation

<!-- FILL: cite current run counts per sector from LOG Addendum XXX -->
<!-- FILL: cite any findings resolved (with addendum refs) and any known open items -->

### Demo corpus and op gym

The demo corpus (the `.webp` files in `docs/media/`) doubles as an integration instrument:
each demo is a fixed input/output pair that must survive every refactor.
The op gym is a set of parameterized operation exercises run as part of the test suite.
Together they provide broad coverage of the kernel's surface area at the integration level,
complementing the unit-level oracle tests.

---

## Architecture

### `keel-math`

Numeric foundations: vectors (`Vec2`, `Vec3`, `Vec4`), matrices, transforms (Rodrigues rotation),
axis-aligned bounding boxes, outward-rounded interval arithmetic, tolerance policy
(one home for every epsilon: linear `1e-8`, angular `1e-11`), exact predicates
(Shewchuk `robust` crate behind a `Sign` enum, with the orient3d below-plane convention
documented in unit tests), bracketed hybrid Newton solver, polynomial arithmetic
(Blinn quadratic, Yuksel monotonic-interval cubic), and Bernstein machinery including
the projected-polyhedron multivariate subdivision solver.

The one-root algebraic layer (`AlgebraicReal`, an expression DAG with interval enclosures
and BFMSS separation bounds) supports exact conic predicates at the topology tier.

### `keel-geom`

Curves and surfaces: homogeneous 4D NURBS (de Boor evaluation, hodograph derivatives,
rational recursion, exact circular arcs and full circles, knot insertion / Bezier decomposition),
analytic curves (`Line3`, `Circle3`, `Ellipse3`), analytic surfaces (planes, cylinders, cones,
spheres, tori, surfaces of revolution), local differential geometry (first and second fundamental
forms, principal curvatures, Gaussian and mean curvature), global closest-point projection
(Bezier decompose, AABB branch-and-bound prune, bracketed-Newton polish), and interval enclosures
for certified containment tests.

All evaluators are generic over scalar type to admit interval arithmetic in the certification
pipeline.

### `keel-topo`

PES-class non-manifold B-rep topology with first-class space-partitioning regions, Euler operators
(MEV, MEF, MEKR, KEV, KEF, KEKR and their inverses), operation lineage records on every entity,
session / pmark / rollback support (transactional editing), boolean pipeline (imprint, classify,
stitch), blend and fillet families, local direct-edit operators, tessellation (edge-first watertight
contract), cross-section interrogation, mass-properties integrator, and hidden-line removal.

### `keel-wasm`

Thin WebAssembly binding layer over `keel-topo`. Exposes a subset of the API for browser
and Node.js consumers. Spike in progress; the API surface is not yet stable.

---

## Quickstart

### Build and test

```sh
# Requires Rust stable (see rust-toolchain.toml for the pinned version).
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Fuzz harnesses require Linux and Rust nightly:

```sh
# In WSL or a Linux environment:
cargo +nightly fuzz run fuzz_boolean
cargo +nightly fuzz run fuzz_cyl_boolean
cargo +nightly fuzz run fuzz_imprint
```

### Minimal example

```rust
// FILL: replace this block with a real worked example once the API stabilizes.
// Suggested content: construct two boxes, boolean-subtract one from the other,
// query the result volume, and print it. Show the DECLINE path if the operation
// is ambiguous.
//
// use keel_topo::{Body, BooleanOp};
// ...
```

---

## Roadmap

The milestones below follow the sequence established in the architecture spec
(`docs/superpowers/specs/2026-06-07-keel-kernel-architecture-design.md`).
Status is approximate; read `LOG.md` for the current anchor.

<!-- FILL: audit each row against LOG.md before publishing; mark done/in-progress/planned honestly -->

| Milestone | Description | Status |
|---|---|---|
| M1 | Numeric foundations (`keel-math`) | <!-- FILL: e.g. complete, merged --> |
| M2 | Curves and surfaces (`keel-geom`) | <!-- FILL --> |
| M3 | Topology layer, Euler ops, primitives | <!-- FILL --> |
| M4 | Boolean pipeline, classify, stitch | <!-- FILL --> |
| M5 | Certified SSI (surface-surface intersection), NURBS booleans | <!-- FILL --> |
| M6 | Blends, fillets, local direct-edit operators | <!-- FILL --> |
| M7 | STEP import/export, WASM build, first external consumer | <!-- FILL --> |

Items not on the current roadmap (by design): T-splines (patent landscape, THB-splines cover
the refinement use case), auto-inferred live-rules constraints, single-body mesh-plus-B-rep
convergent operations. See `docs/superpowers/specs/2026-06-07-keel-kernel-architecture-design.md`
section D10 for the patent posture.

---

## License

The license for this project has not yet been finalized. A dual-license arrangement
(permissive open-source plus a possible commercial tier) is under consideration.

**Contributions are not yet accepted.** A contributor license agreement (CLA) decision
is pending. Please open an issue to discuss before submitting any pull request.

<!-- FILL: once the license is decided, replace this section with the SPDX identifier,
a badge, and the contributing policy. Update the Cargo.toml workspace.package.license
field to match. -->

---

## A note on the demos

Every animation in this README was rendered from the kernel's own tessellation output,
processed by the `docs/media/render_gif.py` script in this repository.
The geometry you see is exactly what the kernel produces. No external renderer, no cleanup.
