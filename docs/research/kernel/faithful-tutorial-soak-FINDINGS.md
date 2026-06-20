# Faithful tutorial soak: the REAL decline worklist

Headline (seed 1, clean base kernel `2bd0be9`, 9,197 projects tallied):
**of 100,821 declines, only 30,092 (29.8%) are REAL tutorial gaps; 70,729 (70.2%)
are artifacts of impossible / ill-posed random input.** The old all-in
"kernel-frontier" count put 60,129 declines on the worklist; the provenance
classifier shows HALF of those are noise. PASS 7,681 / STALL 1,515 / WRONG 1 /
TIMEOUT 0. The single WRONG is a pre-existing base-kernel boolean gap the
faithful generation surfaced (the random fuzzer masked it); the oracle caught it
(never silently passed), and a sibling agent has since fixed it in the boolean
gate.

## Why this exists

`crates/keel-topo/examples/realsoak.rs` samples the LEARNED grammar of real
Plasticity (CAD) modeling sessions and runs long op-chains against the kernel,
classifying every result by the three-bucket oracle (PASS / DECLINE / WRONG=0).
It is a faithful op-SEQUENCE model, but in its original form it fed those
sequences RANDOM primitive dimensions and placements. The consequence: its
"kernel-frontier" decline count was dominated by **geometrically impossible or
ill-posed random input** -- a sliver primitive, a boolean tool that merely
grazes the body, a shell thickness larger than the wall, a fillet on a body
with no blendable edge -- NOT by real tutorial-workflow failures.

The operator does not care about those random-primitive declines. The real
question is: **when a VALID, realistic tutorial operation is requested, does the
kernel do it or decline?** That decline set is the worklist. This document
reports it.

## What changed (this branch)

Two non-destructive additions to `realsoak.rs`, both gated behind
`KEEL_FAITHFUL=1` (the original adversarial fuzzer is untouched and still runs
by default):

### Part 1 -- decline PROVENANCE classifier

Every declined op is now classified REAL vs ARTIFACT. A decline is a **REAL
tutorial gap** only when ALL of these hold:

- **(a) sound input**: the input body is `validate()`-Ok AND watertight. Watertight
  is measured directly by counting radial-1 open boundary edges
  (`edge.radial.len() == 1 && edge.bounds.0 != edge.bounds.1`, the same predicate
  the kernel's interrogation uses); zero == closed. This matters because
  `validate()` *skips* the Euler-Poincare/closure check on non-manifold bodies
  (`validate.rs::check_euler_poincare` returns early when any edge has
  `radial.len() != 2`), so an OPEN body can be `validate()`-Ok. Declining
  against an already-open body is the body's fault, not the op's.
- **(b) real tutorial op**: fillet / chamfer / boolean(union|diff|intersect) /
  mirror+union / shell / extrude(=union-with-tool). Free-form ops with no
  realization on an arbitrary solid (loft / sweep / fill / array / trim) fail
  this and are artifacts by definition (`op-not-on-solid`).
- **(c) sane parameters**: fillet/chamfer radius is a small fraction (2..8%) of
  the edge length by construction (always fits); shell thickness must be < half
  the body's `min_wall_thickness()` (else `thickness-exceeds-wall`).
- **(d) well-posed**: the target edge exists (else `no-target-edge`); the boolean
  tool genuinely + meaningfully overlaps the body -- AABB overlap fraction >=
  10% of the smaller body (else `glancing-overlap`), and a tool could be built
  at all (else `no-tool-buildable`).

If all hold and the kernel still declines -> REAL gap, recorded with a
`prevOp->op:fault` class key and a minimal repro seed. Otherwise -> ARTIFACT,
counted separately by reason, never on the worklist.

The classifier alone converts the existing run into a real-vs-artifact
breakdown without changing generation.

### Part 2 -- tighter, tutorial-plausible generation

The op SEQUENCES (the grammar's real value) are kept verbatim. Only the
geometry is constrained toward what a real modeler would request, so artifacts
are generated less often:

- **primitive aspect ratio** tightened to near-cubic (0.75..1.35, max ~1.8x
  anisotropy; fuzzer kept 0.6..1.6);
- **seed mix** leaned harder into prismatic (block 0.80 / cyl 0.16 / cone 0.02 /
  sphere 0.02) so a curved-first-boolean decline rarely kills a chain at op 1;
- **boolean tool** cross-section solidified to 0.4..0.65 (a deliberate transversal
  cut/join that clears the meaningful-overlap bar, not a thin grazing peg), and
  tool kind leaned to block/cyl (0.72 / 0.24) with the curved cone/sphere tools
  trimmed to the 0.02/0.02 frontier minority.

## Results -- faithful run, seed 1, clean base kernel (commit 2bd0be9)

Measured on a pristine checkout of the fork base `2bd0be9` (the kernel state this
task assumed), so the numbers attribute only to the harness work -- no in-flight
kernel edits from sibling agents are mixed in. `KEEL_FAITHFUL=1`, seed 1.

The dispatcher requested 10,000 projects; **9,197 completed and are tallied
below**. The run aborted near the end on a single pathological late project that
overflowed even a 256 MB worker stack -- a SECOND real finding (see "Crash"
below). 9,197 is a robust sample and the proportions are stable across the run
(the per-1000 progress line held WRONG=1 / TIMEOUT=0 from project 1000 onward).

| metric | value |
|---|---|
| projects tallied | 9,197 |
| PASS | 7,681 |
| STALL (abandoned after 12 consecutive declines) | 1,515 |
| WRONG (decline-never-wrong violations) | **1** |
| TIMEOUT (perf cliff) | 0 |
| total declines | 100,821 |
| -- REAL tutorial declines | **30,092 (29.8%)** |
| -- ARTIFACT declines | 70,729 (70.2%) |

**WRONG = 1**, and it is a genuine PRE-EXISTING base-kernel boolean gap, not an
artifact of the harness and not introduced by these changes (no kernel code was
touched). It reproduces byte-identical via the public API alone, and the oracle
caught it as WRONG -- it was never silently accepted as a PASS, so the core
decline-never-wrong promise held. Documented as REAL finding #0 below.

## The REAL decline worklist (what to solve)

### REAL finding #0 -- the one WRONG (highest priority)

`shell -> mirror -> fillet -> boolean(Union)` on a curved hollow body. The kernel
returns `Ok` from the union, but the result is self-inconsistent: analytic
`mass = 10009.65` vs tessellated `mesh = 5242.54` (ratio 1.91) -- and the mesh is
even LOWER than the pre-union body (5458.97), which a union can never do. The body
is malformed (geometrically-overlapping components left topologically disjoint,
with a spurious genus-1 handle); the mass is correct, the mesh collapsed. The
oracle's curved tier-2 net (>25% mass!=mesh) caught it as WRONG; it was never
passed. Repro: `KEEL_FAITHFUL=1 KEEL_REPRO=11400715918834827198`. Root cause (per
the sibling fix): the post-assembly self-consistency gate's CURVED branch checked
`mass` against a global-reference signed-tetra sum but not against the per-
component `mesh_volume()` the planar branch already gates on. Class: boolean
assembly self-consistency (a decline-never-wrong gate hole, now fixed in the
boolean gate by a sibling task).

### Top REAL tutorial decline classes (the build-this-without-going-wrong list)

`prevOp -> op : fault`, count over the 9,197-project run, and a minimal repro seed
(replay with `KEEL_FAITHFUL=1 KEEL_REPRO=<seed>`):

| count | class (prevOp->op:fault) | repro seed |
|---:|---|---|
| 2197 | sketch->extrude : extrudeU:AssemblyFailed | 11400715918834826711 |
| 1556 | extrude->extrude : extrudeU:AssemblyFailed | 11400715918834826705 |
| 1544 | fillet->extrude : extrudeU:AssemblyFailed | 11400715918834826702 |
| 1264 | fillet->chamfer : chamfer:Precondition | 11400715918834826727 |
| 1142 | boolean->extrude : extrudeU:AssemblyFailed | 11400715918834826701 |
| 1134 | extrude->boolean : booleanI:AssemblyFailed | 11400715918834826705 |
| 1025 | extrude->boolean : booleanD:AssemblyFailed | 11400715918834826782 |
|  933 | fillet->boolean : booleanI:AssemblyFailed | 11400715918834826702 |
|  911 | fillet->fillet : fillet:Precondition | 11400715918834826739 |
|  853 | extrude->boolean : booleanU:AssemblyFailed | 11400715918834826708 |
|  838 | boolean->mirror : mirror:Precondition | 11400715918834826701 |
|  780 | fillet->boolean : booleanD:AssemblyFailed | 11400715918834826727 |
|  709 | sketch->mirror : mirror-union:AssemblyFailed | 11400715918834826711 |
|  674 | extrude->mirror : mirror:Precondition | 11400715918834826733 |
|  660 | fillet->mirror : mirror:Precondition | 11400715918834826722 |
|  624 | fillet->boolean : booleanU:AssemblyFailed | 11400715918834826715 |
|  595 | fillet->extrude : extrudeU:UnassemblableSeam | 11400715918834826727 |
|  546 | sketch->boolean : booleanU:AssemblyFailed | 11400715918834826743 |
|  506 | boolean->boolean : booleanI:AssemblyFailed | 11400715918834826710 |
|  494 | sketch->boolean : booleanD:AssemblyFailed | 11400715918834826701 |
|  493 | sketch->boolean : booleanI:AssemblyFailed | 11400715918834826711 |
|  488 | boolean->chamfer : chamfer:Precondition | 11400715918834826737 |
|  464 | sketch->sketch : extrudeU:AssemblyFailed | 11400715918834826751 |
|  455 | boolean->boolean : booleanD:AssemblyFailed | 11400715918834826710 |
|  422 | extrude->chamfer : chamfer:Precondition | 11400715918834826705 |

Aggregated by KERNEL MECHANISM (the actual work items), REAL declines are:

- **boolean assembly (`*:AssemblyFailed`)** -- by far the largest share: union /
  difference / intersection (incl. extrude=union and mirror+union) that cross
  cleanly but cannot be re-stitched into a watertight shell. The dominant
  worklist item.
- **fillet / chamfer Precondition** -- the blend/chamfer engine refusing a
  sound, comfortably-sized edge on a complex grown body.
- **mirror Precondition / mirror-union** -- the mirror op + its union on a sound
  body.
- **`*:UnassemblableSeam`** -- the hard curved-crossing-seam decline (e.g. the
  cylinder-cylinder crossing pair) the kernel takes on purpose rather than emit a
  wrong body.

### Crash (SECOND real finding -- perf/robustness)

At least one realistic chain at seed 1 drives the kernel's recursive boolean /
tessellation routines deep enough to overflow the default 2 MB worker thread
stack (a stack overflow is NOT catch_unwind-able -- it aborted the whole soak
process at ~4.8k projects on the first attempt). Raising the worker stack to
256 MB (a harness-only change in `spawn_pool`) let the run reach ~9.2k before a
SINGLE remaining pathological project overflowed even 256 MB. That residual is a
real signal of extreme- or unbounded-depth recursion on some grown body -- a
robustness item for the kernel (the per-op recursion should be iterative or
depth-bounded). It is a perf/robustness concern, not a correctness WRONG.

### Class attribution (which kernel op/class)

The REAL declines map onto a small number of kernel mechanisms, all of them
honest decline-never-wrong guards (the kernel refuses rather than emit a wrong
body):

- **`*:AssemblyFailed`** (the dominant REAL class) -- `BoolFault::AssemblyFailed`
  (`boolean.rs:48`): the two operands genuinely cross (SSI curves exist) and the
  seams imprint, but the result cannot be re-stitched into a valid watertight
  shell. Sub-reasons (via `KEEL_FAULT_CENSUS`): "degenerate or self-inconsistent
  result (mass != mesh)" and "unmatched coedge: shell-closure invariant
  violated". This is the boolean assembler / Weiler-stitch frontier. Hit by
  Extrude(=union), Boolean(U/D/I), and mirror+union.
- **`*:UnassemblableSeam`** -- `BoolFault::UnassemblableSeam` (`boolean.rs:44`):
  a HARD decline. A face pair crosses (the SSI curves exist) but the imprint
  cannot assemble those seams (e.g. the cylinder-cylinder crossing pair).
  Proceeding seamless would yield an Euler-valid but geometrically WRONG body,
  so the kernel declines on purpose. The curved-crossing-seam frontier.
- **`fillet:Precondition` / `chamfer:Precondition`** -- the blend/chamfer engine
  refuses a precondition on the chosen edge (support geometry / convexity /
  local overflow) even though the radius comfortably fits the edge length and
  the input is sound. The fillet/chamfer-engine frontier on complex grown
  bodies.
- **`mirror:Precondition` / `mirror-union:*`** -- the mirror op or its follow-up
  union declines on a sound body (the mirror+union assembly frontier; the union
  half lands on `AssemblyFailed`/`UnassemblableSeam` like any other union).

All four are honest declines, not bugs. Solving them (expanding the assembler /
seam imprinter / blend preconditions so these cases BUILD without ever going
wrong) is the minimize-declines worklist -- WRONG stays 0 throughout.

## Contrast: how much was noise

The original soak split declines into "by-design unsupported-on-solid" vs
"KERNEL-FRONTIER (the worklist)". On this run that split is:

| bucket | count |
|---|---:|
| total declines | 100,821 |
| by-design unsupported-on-solid (loft/sweep/fill/array/trim) | 40,692 |
| **old "kernel-frontier" worklist** (everything else) | **60,129** |

The provenance classifier then shows that of those **60,129** the original soak
called "the worklist", only **30,092 are REAL** -- the other **30,037 are
artifacts** of impossible / ill-posed input that no real tutorial would request:

| artifact reason | count | what it is |
|---|---:|---|
| op-not-on-solid | 40,692 | free-form ops with no realization on a solid (the by-design bucket) |
| input-not-watertight | 21,921 | the op was declined against a body that was already OPEN (validate-Ok but not closed) -- the body's fault, not the op's |
| no-target-edge | 4,222 | fillet/chamfer requested on a body with no blendable edge |
| thickness-exceeds-wall | 3,043 | shell thickness >= half the body's min wall |
| no-tool-buildable | 661 | boolean tool could not be constructed for the body |
| empty-body | 190 | mirror/shell on a degenerate/empty body |

So the operator's real worklist is **HALF** the size the old all-in count
implied (30,092 vs 60,129), and **less than a third** of all declines. The big
hidden contributor is `input-not-watertight` (21,921): the original soak counted
every decline against an already-open body as a "kernel frontier" item, but those
declines are the kernel correctly refusing to operate on a malformed input -- not
a gap in what it can do on a VALID body.

## Caveat on attribution / environment

This worktree branch was concurrently written by sibling agents (it carries an
auto-committed WIP of this work plus an unrelated `#77` watertightness-witness
commit that edits the boolean gate). To keep the measurement honest, the numbers
above were produced from a SEPARATE pristine checkout of the fork base `2bd0be9`
with only `realsoak.rs` swapped in -- so no sibling kernel edits influence them.

## Reproduce

```
cd crates/keel-topo
cargo build --release --example realsoak
KEEL_FAITHFUL=1 cargo run --release --example realsoak -- 10000 1 realsoak-faithful-10k

# replay one project (seed from the worklist table) with per-op mass/mesh/bound:
KEEL_FAITHFUL=1 KEEL_REPRO=<seed> cargo run --release --example realsoak
```

Outputs land in `realsoak-faithful-10k/`:
`declines.jsonl` now carries `"real":bool` and `"artifact":"<reason>"` per line,
so the worklist can be filtered with `jq 'select(.real)'`.
