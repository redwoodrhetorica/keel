# M6a: Boolean Engine (Clean Transversal Cases) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Regularized boolean operations (union, intersection, difference) on solid bodies, working end-to-end on clean transversal configurations: two-body imprint (SSI + imprint both + glue), fragment classification (in/out via PMC + neighborhood), regularized selection, and shell/region stitch into a valid result body. THIS IS THE PROOF MILESTONE's core: the kernel computes A ∪ B, A ∩ B, A − B and the results are valid solids with correct volume.

**Architecture (M3-gate boolean pipeline, now built):** localize → intersect (M5a) → imprint+glue (M5b primitives, composed here) → classify → select → stitch → report. Regularized r-set semantics (Requicha): `A op* B = cl(int(A op B))`. Partial-success fault model: a result plus per-face-pair fault tags, never all-or-nothing (synthesis A: booleans report faults, never panic). Coplanar faces, tangencies, and degenerate coincidence are DEFERRED to M6b/M7 and returned as fault tags here.

**Proof cases (the milestone bar):**
- **Sphere ∪/∩/− Sphere**: the textbook boolean; SSI is one exact circle (tier 1); each sphere splits into two caps; select and stitch. Volume checked against the exact lens/union formula.
- **Block − Cylinder** (drill a through-hole): transversal, no coplanar faces; result volume = block − cylinder-segment.
- **Block ∩ Block (offset on all axes)**: transversal box intersection; result = the overlap box.

**Research basis:** kernel/01 Part 6 (Requicha-Voelcker boundary evaluation, Tilove SMC/PMC + neighborhood, re-read for M3 gate), wave-2 synthesis A (partial-success model, metamorphic testing, winding PMC), M3 gate section 4. Spec D3/D5/D7.

**Carried lessons:** every mutation atomic + lineage; debug-validate after each op; oracles before code; `if ($LASTEXITCODE -eq 0)` commit guards; honest fault tags over silent wrong answers; metamorphic proptests (idempotence, transform-invariance, round-trip).

**Branch:** `m6a-boolean` off master.

---

## Scope boundary (binding)

1. **Transversal only in M6a.** Faces meet transversally (curves cross surfaces, not lie in them). Coplanar/coincident faces, tangential contact, and curves-through-vertices return `Fault::Coincident`/`Fault::Tangent` and the operation reports partial success or declines. M6b/M7 handle them.
2. **Regularized semantics always** (Requicha r-sets): results are the closure of the interior; lower-dimensional debris (sheets, dangling edges from on-boundary fragments) is discarded.
3. **Both operands copied into one working body.** Glue and classification need both topologies in one Body. The result is a new Body; operands are unchanged (functional booleans).
4. **Classification is the M4 PMC** plus a neighborhood test for the imprinted-edge fins; no new PMC engine.
5. **Two-body imprint completes here** (the M5b-reassigned Task 5), specialized to transversal pairs.

---

## Tasks

### Task 1: two-body imprint (transversal)
`crates/keel-topo/src/boolean.rs` (new): `imprint_pair(a: &Body, b: &Body, tol) -> Result<(Body, ImprintMap), BoolFault>`. Copy both into one working Body (a `Body::merge_disjoint(a, b)` helper preserving lineage with operand tags). Localize face pairs by AABB. For each pair, SSI (M5a `intersect_surfaces` over the faces' surfaces); for each transversal SsiCurve, imprint on A's face and B's face (M5b `imprint_open_curve`/`imprint_closed_curve` by curve closedness), then `glue_edges` the coincident pair (dihedral sort fires). Coincident/tangent SSI results → fault tag, skip. Output: the imprinted working body + a map from result faces to (operand, original face). Tests: sphere-sphere imprint → both spheres split into caps sharing one circle (radial-2 after glue... radial-4 only if both bodies' faces meet, which for transversal solids they do → the shared circle has 4 fins); block-cylinder imprint → cylinder hole circles on the block faces + the block-plane circles on the cylinder; validate + d-of-d hold.

### Task 2: fragment classification
For each face of the imprinted working body, classify it as INSIDE, OUTSIDE, or ON the OTHER operand: sample the face interior (a point strictly inside its trim loops, found via the winding test + an interior UV point), `classify_point` against the other operand's regions. Faces whose interior point is In(other) are inside; Out are outside; On (within tol of the other's surface) are coincident → `Fault::Coincident` (M6b). Store a per-face classification tag. Tests: sphere-sphere caps classify correctly (the cap of A inside B vs outside B); block-cylinder face classifications.

### Task 3: regularized selection tables
```rust
pub enum BoolOp { Union, Intersection, Difference }
```
Per operation, the keep-set of faces by (owner, classification):
- **Union**: keep A-faces OUTSIDE B + B-faces OUTSIDE A. (The outer hull.)
- **Intersection**: keep A-faces INSIDE B + B-faces INSIDE A.
- **Difference A−B**: keep A-faces OUTSIDE B + B-faces INSIDE A, with B's kept faces REVERSED (the cavity wall faces inward).
Reversed faces flip their (front_region, back_region) and fin senses. Discard all non-kept faces (and their now-orphaned topology) regularized: drop lower-dimensional debris. Tests: the keep-sets for each op on sphere-sphere match the hand-computed cap sets.

### Task 4: stitch + region rebuild
From the kept face-side set, reassemble shells (walk the glued seam: at each shared edge the kept fins from both operands now form a manifold 2-cycle again after the unkept sides are removed), and recompute the region partition (the result has its own infinite region + solid region(s)). Delete unkept entities through the operators where possible, or build the result shell set directly and re-run region inference. Validate the result body (Euler-Poincare for manifold results, d-of-d, every face a region pair). Tests: each boolean result validates; the shared seam edges are radial-2 in the result (the non-manifold transient is resolved by selection).

### Task 5: the boolean API + volume proofs
```rust
pub struct BoolResult { pub body: Body, pub faults: Vec<BoolFault>, pub report: OpReport }
pub fn boolean(a: &Body, b: &Body, op: BoolOp, tol: f64) -> Result<BoolResult, BoolFault>
```
Compose Tasks 1-4. Partial-success: faults accumulate; a fatal fault (no valid result) is `Err`, a recoverable one (a skipped coincident pair) rides in `BoolResult::faults`. THE PROOF TESTS:
- two unit spheres distance 1.5 apart: union volume = 2·V_sphere − V_lens, intersection = V_lens, A−B = V_sphere − V_lens, each to 1e-6 relative (exact lens formula).
- block 4³ minus a radius-1 cylinder drilled through: volume = 64 − π·1²·4.
- two offset blocks intersection = overlap box volume exact.
- A ∪ B and A ∩ B classify a probe point consistently with set membership.

### Task 6: metamorphic proptests + fuzz + gate
Metamorphic relations as proptests (synthesis A): **idempotence** (A ∪ A ≈ A by volume, A ∩ A ≈ A); **commutativity** (A ∪ B vol = B ∪ A vol); **transform-invariance** (boolean then translate = translate then boolean, by volume); **absorption** (A ∪ (A ∩ B) ≈ A). `fuzz_boolean`: random sphere/block pairs + random ops; results validate, volumes are finite and bounded by the operand volumes (V(A∩B) ≤ min, V(A∪B) ≤ sum, V(A∪B) ≥ max), never panic, coincident configs fault-tag cleanly. Exit gate: fmt, clippy, workspace tests, prior artifacts replay, 10-min fuzz_boolean soak CLEAN, LOG, merge.

## Deferred to M6b / M7 (the robustness milestone)
- Coplanar/coincident faces (the union-of-aligned-boxes case): neighborhood-merge classification.
- Tangential contact and curves through vertices.
- NURBS-bounded solids end-to-end (the M7 proof bar): tier-2/3 SSI feeding imprint, fit-hardened trim curves, Krawczyk-verified throughout.
- Differential testing vs OCCT over the ABC corpus (synthesis A approximate oracle).
- Self-intersection / non-manifold-result booleans (sheet ∪ solid).
