# Keel: Capabilities and Limitations

This document is the honest, grounded account of what the Keel kernel does, what it
declines, and the guarantee that backs both. It complements the capability matrix in
`README.md` by going deeper on the **limitations frontier**: where the kernel refuses,
why, and how that refusal is kept safe.

Every quantitative claim here is reproducible from the test suite (commands at the end).
Numbers are reported conservatively; a DECLINE is a documented outcome, not a defect.

---

## 1. The contract, restated

Keel operations return one of three outcomes:

| Outcome | Meaning |
|---|---|
| **PASS** | A topologically certified body whose analytic mass and tessellated mesh volume agree within tolerance (exact for all-planar bodies, within the adaptive chordal band for curved bodies). |
| **DECLINE** | The operation was refused or could not be certified. The input body is returned unchanged. The caller gets an `Err`, never a malformed body. |
| **WRONG** | A body that is returned but is malformed or has inconsistent mass. **This outcome is not permitted.** Every test lane enforces zero occurrences. |

The design priority is strict: **a DECLINE is always acceptable; a WRONG never is.** When a
case cannot be made to PASS safely, the kernel is built to DECLINE it cleanly rather than
risk emitting a wrong body. Driving the DECLINE set toward zero is the ongoing program;
keeping WRONG at zero is the non-negotiable floor.

---

## 2. The WRONG=0 evidence chain

The "never wrong" claim rests on independent, overlapping checks, not a single test:

- **Randomized oracle (1,000,000 trials/lane).** PASS only when the output matches an
  independent reference (an exact closed form, or a separate code path); WRONG=0 across
  strict, tolerant, and cone lanes (LOG Add 193; see `README.md` for the per-lane split).
- **Fuzz soak (>2.4 billion executions).** 16 sectors, zero crashes (LOG Add 193).
- **Realistic-workflow soak (10,000 projects).** Grammar-driven long op-chains distilled
  from 113 Plasticity tutorial transcripts. Most recent run: **PASS 8039 / WRONG 0**
  (LOG Add 303). A three-bucket oracle classifies every operation; WRONG is a hard fail.
- **Faithful workflow census (10,000 distinct workflows).** A separate, harder bar:
  10,000 hand-specified realistic CAD workflows (flanges, pulleys, gears, shafts,
  housings, brackets, manifolds), each a full multi-step recipe built from its own stated
  dimensions and run through the same three-bucket oracle. Complete, with **WRONG 0 across
  all 10,000** (LOG Add 315). The per-workflow PASS rate is intentionally not the headline
  here: the corpus is curved-compound-heavy and one declined step (typically a curved
  boolean or a fillet) declines the whole workflow, so PASS is low while the never-wrong
  floor is what the census measures, and it held.
- **The dual mass-vs-mesh gate.** Every solid result must have its analytic mass agree with
  its independently tessellated mesh volume, paired with a coedge-pairing (shell-closure)
  check so a dropped face cannot pass under symmetric volume cancellation. This gate is
  load-bearing: relaxing it has been shown to produce a WRONG, so it is never relaxed.
- **An independent winding-number witness.** A generalized-winding-number (solid-angle)
  Monte-Carlo volume can adjudicate mass-vs-mesh disagreements as a third, independent
  integrator.

### What faithful testing added (and why it matters)

The billion-execution fuzz milestone certified WRONG=0 *for the fuzzer's input
distribution*. A separate, faithful test harness — real tutorial workflow **structures**
run with sensible fixed parameters, rather than random primitive dimensions — surfaced a
real decline-never-WRONG violation that the random fuzzer had not hit: a
`shell -> mirror -> fillet -> union` chain that returned a body with analytic mass
~1.9x its mesh volume (a malformed multi-component result). The oracle **detected** it
(the contract's detection promise held); the kernel's gate had a hole, which is now fixed
(LOG Add 300). The lesson, now standing practice: realistic workflows exercise the kernel
the way real use does and find gate holes a random distribution misses. Faithful tutorial
testing is the primary correctness instrument going forward.

---

## 3. Faithful tutorial coverage (the live scoreboard)

`crates/keel-topo/tests/tutorial_workflows.rs` encodes the dominant CAD workflow classes
distilled from the tutorial corpus, each run with realistic fixed parameters and asserted
against the full oracle (validate + watertight + mass==mesh). It is the project's honest
measure of real-world readiness. Each declining workflow is marked `#[ignore]` with its
reason and un-ignored when fixed, so the scoreboard is self-reporting.

**Current: 10 of 12 passing** (LOG Add 302-303).

| Workflow | Status |
|---|---|
| Union two blocks, fillet the seam | PASS |
| Difference, then fillet the concave inner edge | PASS |
| Chamfer a single edge | PASS |
| Loft between two rectangular profiles | PASS |
| Loft between two circular profiles | PASS |
| Tapered loft (square frustum) | PASS |
| Extrude a sketch, fillet the top edges, then shell (hollow) | PASS |
| Mirror an asymmetric solid and union the halves | PASS |
| Chamfer all four top edges (adjacent) | PASS |
| Union a cylinder boss onto a plate, fillet the circular seam | PASS |
| Fillet all 12 edges of a box | DECLINE (see 4.1) |
| G2 (curvature-continuous) fillet | DECLINE (see 4.2) |

This is a strong automated baseline. The two remaining items are not decline-fixes (one is
deep coupled surgery, one is a missing feature); they are the natural target of a manual
tutorial pass.

---

## 4. Known limitations (the honest frontier)

These are the cases the kernel currently DECLINES. Each is a clean refusal that returns the
input unchanged, never a wrong body.

### 4.1 Fillet all edges of a box (3-edge corner)

Filleting every edge of a box succeeds through the 2-edge corners (where two filleted edges
meet) but DECLINES at the first 3-edge corner (where three filleted edges meet at a box
vertex). The geometrically correct corner there is a sphere octant; that surgery is proven
exact for the isolated case but the box's interleaved fillet ordering leaves a later edge
landing mid-arc on a prior fillet's far cap, which needs a generic-cap trim follow-up. A
guard DECLINES the unhandled corner rather than emit the ~1-2% wrong geometry an
approximate cap would produce. Adjacent 2-edge fillets are exact and hollow-compatible
(LOG Add 301-302).

### 4.2 G2 (curvature-continuous) fillets

The fillet API is radius-only (circular / G1-continuous). A G2 fillet ("make that a G2
fillet") requires a new curvature-continuous blend surface family (conic or NURBS); this is
a missing **feature**, not a robustness gap, and is out of scope for the decline-reduction
program (LOG Add 302).

### 4.3 The compound-operand boolean assembler

On the realistic-workflow soak, the dominant remaining real DECLINE class is the
compound-operand boolean: a tool unioned or cut into an already-multi-feature body whose
intersection curves cross each other and the body's holes in stacked configurations. Many
single-shape and simple-compound cases now assemble (T-junction imprint, seam-crosses-hole,
the per-face planar overlay, co-wound holes); the residual is curved-carrier arrangements
and harder stacked planar cases. These DECLINE cleanly (LOG Add 296-301).

A recent fix in this class: a cylindrical tool placed at a rotated orientation (a
transform-placed feature, e.g. the holes of a bolt circle or a ring of radial set-screws)
was declining a *valid* result. The hole-wall face was being threaded to cover only half
its azimuth, so the tessellation disagreed with the analytic mass and the self-consistency
gate declined. Recording the swept angle on the wrap-circle's two halves makes the wall
cover the full revolution; the class now assembles exactly (LOG Add 314).

### 4.4 Curved surface-surface intersection (SSI) edge cases

Some booleans on curved carriers (cylinder/cone/sphere/NURBS) where the intersection is
tangential, grazing, or near-degenerate DECLINE rather than risk an unstable seam. Equal-
radius crossing cylinders, countersinks, and ball-in-socket assemble exactly; an off-axis
rod swallowed by a sphere (the cyl/sphere wrap band) now assembles for all three ops (LOG
Add 312). The frontier is the harder curved crossings (LOG Add 290-295). Where a curved
crossing has no closed-form trimmed volume, an independent op-volume oracle backs the gate
so a misclassified result declines rather than ships: cylinder-vs-sphere and cone-vs-sphere
via a 1D-integral truth, and cylinder-vs-cylinder via a deterministic voxel integral (LOG
Add 313). Tilted cylinder-vs-cylinder still declines on render-mesh watertightness at the
shared seam, kept safe by that oracle.

### 4.5 General NURBS booleans

SSI is certified and analytic-surface booleans (plane/cylinder/cone/sphere) are exact, but
general free-form NURBS-NURBS booleans are not yet certified and may DECLINE.

### 4.6 Scope-by-design (not failures)

Loft, sweep, fill, array, and trim have no defined realization on an arbitrary existing
solid in the current scope and DECLINE by design when requested in that position. T-splines,
auto-inferred constraints, and mesh-plus-B-rep convergent modeling are off the roadmap by
design (see `README.md` roadmap and the architecture spec's patent posture).

---

## 5. The decline worklist, measured honestly

A naive count of the fuzzer's declines overstates the real worklist: the random-parameter
generator produces a large fraction of geometrically impossible or ill-posed inputs (a tool
that merely grazes the body, a shell thicker than the wall, a fillet on a body with no
blendable edge, an operation on an already-open body). A decline-provenance classifier
(`KEEL_FAITHFUL=1` in the realsoak harness) separates these:

- A decline is a **real kernel gap** only if the input was sound and watertight, the
  operation was a real modeling op, its parameters were sensible, and it was well-posed.
- On a 10,000-project run, roughly **30% of declines were real** gaps; the rest were
  artifacts of impossible/ill-posed random input (LOG Add 300).

The real worklist is dominated by the compound-operand boolean assembler (4.3), with
secondary contributions from fillet/chamfer/mirror preconditions on complex grown bodies
and the curved-crossing-seam frontier (4.4). This classifier makes the minimize-declines
program target genuine gaps rather than chase noise.

---

## 6. Robustness posture

- **No crashes on deep histories.** The two unbounded recursions on the boolean path (a
  recursive union-find and a recursive assembly-DAG walk) were converted to iterative,
  byte-identical implementations, so deep realistic op-chains cannot overflow the stack into
  an uncatchable abort. A crash is treated as strictly worse than a decline because it
  bypasses the contract entirely (LOG Add 303).
- **Mass integrator is locked.** The Green's-theorem ("green-slab") mass integrator is
  proven correct and locked by a regression test; it is not modified to make a malformed
  body pass — instead the malformed body is declined.
- **Graceful degeneracy.** Fillet overflow, sub-tolerance contact, and tangential cases
  degrade to a documented DECLINE rather than an unstable result.

---

## 7. How to verify these claims locally

```sh
# The faithful tutorial scoreboard (10/12 pass; 2 documented declines are #[ignore]d):
cd crates/keel-topo
cargo test --release --test tutorial_workflows
cargo test --release --test tutorial_workflows -- --ignored   # see the 2 declines + reasons

# The realistic-workflow soak (three-bucket oracle; WRONG must be 0):
cargo run --release --example realsoak                          # adversarial generator
KEEL_FAITHFUL=1 cargo run --release --example realsoak          # tutorial-plausible + provenance

# The full test suite, including the WRONG-lock regressions:
cargo test --release
```

The WRONG-lock tests (`scan_wrong`, `union_wrong_repro`, `cyl_union_mass_witness`,
`three_bucket`, `post_fillet_mass`, `fillet_surgery_robustness`) pin specific
previously-dangerous cases to DECLINE-or-correct so a regression that reopens a WRONG
fails the suite.

---

## 8. Reading further

- `README.md` — the capability matrix (shipped/partial/declined), the honesty contract, the
  demo gallery, quickstart.
- `docs/ARCHITECTURE.md` — how the kernel is built (data structures, pipelines, algorithms).
- `docs/research/kernel/` — the dossiers behind each frontier fix.
- `docs/research/kernel/tutorial-workflows-FINDINGS.md` — the per-workflow tutorial findings.
