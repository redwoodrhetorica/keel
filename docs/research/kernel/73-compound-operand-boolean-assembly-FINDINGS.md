# 73 (FINDINGS). Compound-operand boolean assembly: instrumented attribution of the dominant `AssemblyFailed` / `UnassemblableSeam` decline class, and the proven tractability boundary

## Status and posture

This is the **instrumented-attribution deliverable** for research request 0006 #73
(the compound-operand boolean assembler), produced by the dossier-71 method:
instrument the failing path on the real repro seeds, find the true failing stage
and root cause, correct the prior assumption, and either fix precisely or
characterize precisely. The headline result is a **correction of the framing**:
the dominant compound-operand decline is NOT the Weiler region walk failing on a
compound face set (the prompt's lead hypothesis). It is, by a wide margin, the
**curved `mass == mesh` post-condition gate declining bodies the assembler stitched
WATERTIGHT** -- the DECLINE-never-WRONG floor doing its job on a body whose
analytic mass and/or tessellation disagree. A localized fix to the gate was
attempted, validated against the full suite (553/0) AND a 3000-project soak, and
**produced WRONG = 1** (a body the dual-reference self-consistency check was
correctly declining). The fix was reverted; baseline WRONG = 0 restored. That
WRONG-on-attempt is itself the key finding: it PROVES the dual mass-vs-tessellation
self-consistency is load-bearing for WRONG-prevention, and pins the tractability
boundary exactly.

House style: no em-dashes; ranges written with "to".

Base: HEAD `2212c916939e5e193aabf8d11fd36cee125186d4`, branch
`worktree-agent-ad02b33d7e54e01a6`. Assembler (`crates/keel-topo/src/boolean.rs`)
is UNCHANGED from HEAD. The only landed change is env-gated diagnostics in
`crates/keel-topo/examples/realsoak.rs` (a per-attempt fault census and a verbose
per-attempt fault print under `KEEL_REPRO`), which reproduce every number below.

---

## 1. The load-bearing correction, stated once

> **For a simple tool (block/cylinder/cone/sphere peg) booleaned into an
> already-compound body, the dominant decline is the curved post-condition gate in
> `assemble_boolean` (`boolean.rs:5512`, `AssemblyFailed("degenerate or
> self-inconsistent result (mass != mesh)")`), NOT a seam-incompleteness
> (`UnassemblableSeam`) and NOT the Weiler region-walk / stitch failure
> (`AssemblyFailed("unmatched coedge ...")`). The seam stage and the import-glue
> stitch SUCCEED on most compound operands; the body comes out WATERTIGHT
> (`mesh_open_ratio` ~ 0). It is then declined because one of the gate's
> self-consistency measures disagrees. The gate compares the analytic
> `mass_properties` against THREE references -- `tessellated_volume` (single-
> reference divergence sum), `mesh_volume` (per-component-recentered divergence
> sum), and an independent op-volume bound -- and any disagreement above the band
> declines. The dominant decline therefore decomposes into (a) a genuine analytic
> MASS-INTEGRATION discrepancy on a watertight body (the dossier-71 green-slab
> class, mass disagreeing with BOTH tessellations, `massprops.rs` territory, OUT
> of the assembler's scope), (b) a genuinely NON-WATERTIGHT stitch (correctly
> declined), and (c) a small numerical tail where `tessellated_volume` alone blows
> up. The naive recovery for (c) -- accept self-consistency when mass agrees with
> the robust `mesh_volume` even if `tessellated_volume` disagrees -- ADMITS A
> WRONG (soak seed 11400715918834829047, +1 WRONG), because a watertight
> `mass == mesh` body can still be geometrically wrong and `tessellated_volume`
> was independently catching it. DECLINE-never-WRONG requires the dual reference.
> The honest tractable slice within `boolean.rs` is therefore very narrow; the
> bulk of the class is either the dossier-71 mass-integration frontier (a
> different file) or a genuine non-watertight-stitch frontier that needs a real
> assembler advance, not a gate tweak.**

---

## 2. Method: how the failing stage was attributed

The four prompt seeds replay via `KEEL_REPRO=<seed>
./target/release/examples/realsoak.exe`. Under the current harness these seeds run
to a PASS by SKIPPING the failing boolean op (the realsoak chain continues past a
declined op), so the verbose repro alone shows only the SUCCESSFUL ops and hides
the decline. Two env-gated diagnostics were added to surface the truth:

1. **Per-attempt fault print** (`realsoak.rs`, in `do_boolean`, gated by the
   existing `KEEL_REPRO` flag): prints the exact `BoolFault` and the body/tool
   face counts for every FAILING boolean attempt (the `Err` path and the
   `faulted` returned-result path), not just the surviving decline signature.
2. **Population fault census** (`realsoak.rs`, gated by `KEEL_FAULT_CENSUS`): a
   thread-safe tally of every failing boolean ATTEMPT by coarse fault category,
   printed at the end of a soak.
3. **Gate-decline classifier** (a throwaway diagnostic that was reverted with the
   fix attempt, gated by `KEEL_GATE_CENSUS`): at the gate's failing return,
   printed `open_ratio`, `mesh_volume`, `tessellated_volume`, `mass`, face count,
   and curved-face count, so the dominant gate-decline could be split by
   watertightness and mass-vs-mesh agreement.

This is the dossier-71 instrument-then-attribute method applied to the assembler
post-condition rather than the green slab.

---

## 3. Per-seed failing stage and root cause (the four prompt seeds)

All four seeds, replayed with the per-attempt diagnostic, show the SAME multi-mode
picture. The failing boolean ATTEMPTs (each seed's chain retries up to 3 tool
placements per boolean op, so each op yields several attempts):

### Seed 11400715918834826719 (`sketch->extrude : extrudeU`)
Seeded `cyl`. The `extrudeU` (union of a tool into the cylinder body, `f3`) and a
later `booleanD` all fail with
`AssemblyFailed("degenerate or self-inconsistent result (mass != mesh)")`. With
`KEEL_BOOL_DEBUG=1` the gate values are decisive:
```
curved gate: tess 4286.86 mass 5480.68 mesh 3647.92 open_ratio 0.00000
curved gate: tess 6647.56 mass 5480.68 mesh 6647.56 open_ratio 0.00000
```
The body is **watertight** (`open_ratio` = 0; the stitch succeeded). In the second
attempt `tess == mesh == 6647.56` agree perfectly, yet `mass = 5480.68` (an 18%
UNDER-read). **Root cause: the analytic `mass_properties` mis-integrates a curved
trimmed face** (the dossier-71 green-slab class), surfacing here through the
boolean gate rather than the fillet gate. The assembler is NOT at fault; the
stitch is watertight and the two tessellations agree. This is `massprops.rs`
territory, explicitly OUT of this scope.

### Seed 11400715918834826709 (`extrude->extrude : extrudeU`)
Seeded `block`. A rich mix across the chain's many compound-operand booleans:
- `UnassemblableSeam(176, 5)`, `UnassemblableSeam(170, 5)`, `UnassemblableSeam(194, 5)`,
  `UnassemblableSeam(250, 5)`: a face pair genuinely crosses but the imprint cannot
  assemble the seam where the tool crosses the compound body's prior edges. The
  recurring face id `5` (a tool face) crossing several distinct target faces is
  the "tool crosses a prior seam / blend on a compound body" signature.
- `AssemblyFailed("degenerate or self-inconsistent result (mass != mesh)")`: the
  dominant mode (watertight body, gate self-consistency fails).
- `AssemblyFailed("unmatched coedge: shell-closure invariant violated")`: a genuine
  STITCH failure (the all-planar shell-closure invariant in
  `finalize_imported_assembly`, `boolean.rs:3292`, an edge left with
  `radial.len() < 2`).
- a returned result carrying `Topo(Precondition("open chain end not on boundary"))`
  faults (the imprint left an open chain; the result is returned but the harness
  rejects the non-Coincident fault).

### Seed 11400715918834826706 (`fillet->extrude : extrudeU`)
Seeded `block`, fillet first (so the body carries a curved blend face from op 1).
The compound-operand booleans fail with the same spread:
`degenerate (mass != mesh)` (dominant), `UnassemblableSeam(241, 5)` /
`UnassemblableSeam(241, 19)`, and returned `open chain` preconditions. With
`KEEL_BOOL_DEBUG=1` the gate shows the watertight-but-mass-discrepant pattern,
e.g. `tess 14247 mass 14963 mesh 14888 open_ratio 0.019` (mass approx mesh, but
`tessellated_volume` 4.5% low trips self-consistency-vs-tess) and
`tess 17385 mass 18821 mesh 15718 open_ratio 0.035` (all three disagree, a busier
near-degenerate body).

### Seed 11400715918834826715 (`extrude->boolean : booleanI`)
Seeded `block`, with two prior booleans + a fillet (a genuinely compound,
two-boolean body, `g0 f11`). EVERY failing attempt is
`AssemblyFailed("unmatched coedge: shell-closure invariant violated")` -- a pure
STITCH failure. With `KEEL_BOOL_DEBUG=1` the unmatched edges cluster on the body's
existing face plane (`x = 9.5518`) and the tool's far face (`x = 15.506`), forming
open chains around the tool's protrusion where its side faces cross the body's
PRIOR-boolean faces. **Root cause: the import-glue stitch (`merge_and_glue_imported`
-> shell-closure check) could not pair the seam coedges into radial-2 where the
tool's seam meets the compound body's existing edges.** This IS the assembler
frontier, all-planar.

**Per-seed summary:** seed 19 is pure dossier-71 (mass integration, out of scope);
seeds 09 and 06 are a mix of mass-integration (dominant), genuine
unmatched-coedge stitch failures, and `UnassemblableSeam`; seed 15 is a pure
all-planar stitch (`unmatched coedge`) failure on a two-boolean body.

---

## 4. The population census (3000 projects, seed 1): the dominant class is the gate

Failing boolean ATTEMPTs over a 3000-project soak (`KEEL_FAULT_CENSUS=1`), the
`Err`-path faults by category:

| Category | Count | Share | Stage |
|---|---:|---:|---|
| `degenerate (mass != mesh)` gate | 19,252 | 59% | post-condition gate (body stitched) |
| `UnassemblableSeam` | 7,636 | 23% | seam / imprint (genuine crossing not assembled) |
| `unmatched-coedge` (stitch / Weiler) | 5,134 | 16% | import-glue shell-closure (all-planar) |
| other `AssemblyFailed` | 266 | 1% | misc gate / import |
| `Topo` / `Coincident` / `Tangent` (Err path) | 0 | 0% | (these arrive on the `faulted` returned-result path, counted separately) |

So the prompt's hypothesised dominant mode (the Weiler region walk failing on the
compound face set) is real but is only the 16% `unmatched-coedge` slice; the true
dominant mode (59%) is the curved gate declining a stitched body. WRONG = 0 over
the run (the floor holds).

### 4.1 Splitting the dominant 59% by watertightness (3717 sampled gate-declines)

With the gate-decline classifier (`KEEL_GATE_CENSUS`), the
`degenerate (mass != mesh)` declines split as:

| Sub-mode | Count | Share of gate-declines | Interpretation |
|---|---:|---:|---|
| NON-watertight (`open_ratio > 0.05`) | 1,983 | 53.3% | genuinely cracked / degenerate stitch -- correctly declined |
| watertight, `mass != mesh` (> 2%) | 1,351 | 36.3% | analytic mass-integration discrepancy (dossier-71), OR coarse curved mesh -- mostly OUT of assembler scope |
| watertight, `mass == mesh` (<= 2%) | 330 | 8.9% | body's two robust measures agree; declined because `tessellated_volume` disagrees |
| of which pure `tessellated_volume` blow-up (`open <= 0.01`, `mass == mesh`, `tess` off > 25%) | 8 | 0.2% | single-reference f64 cancellation false-decline |

The standout pure-blow-up example: `open = 0.00000, mesh = 459.84,
tess = 2618.08, mass = 461.32` -- a watertight body whose analytic mass and
per-component mesh AGREE on ~460, declined because `tessellated_volume` read 5.7x
high. `tessellated_volume` (`winding.rs:90`) uses a SINGLE local reference (the
first triangle vertex); the comment admits world-origin cancellation but a single
reference cannot keep coordinates small across a large or multi-component body.
`mesh_volume` (`interrogate.rs:808`) was given PER-COMPONENT recentering for
exactly this reason (LOG Add 264); `tessellated_volume` was not.

---

## 5. The fix attempt and the WRONG it produced (the proven tractability boundary)

The only sub-mode tractable WITHIN `boolean.rs` is 4.1(d): the watertight,
`mass == mesh`, `tessellated_volume`-blew-up false-decline. The attempted fix
(in `assemble_boolean`'s curved `self_consistent`): accept self-consistency when
mass agrees with EITHER `tessellated_volume` OR the robust `mesh_volume` (the
second arm guarded by `validate()`, exactly the kernel's own advisory-fault clear
criterion at `boolean.rs:5545`).

- **Validation passed:** full suite 553 passed / 0 failed; the targeted seeds
  behaved correctly (seed 19 stayed DECLINED -- its `mass != mesh` means the second
  arm does not fire; the pure-blow-up cases recovered, watertight `mass == mesh`
  gate-declines 330 -> 48 over 500 projects).
- **But the 3000-project soak produced WRONG = 1:** seed 11400715918834829047
  (`cyl` seed, a 68-op chain). Pre-fix this seed STALLs at realized=3
  (`mass = None, mesh = 44336`, in-bound, no violation). Post-fix the fix let an
  earlier compound-operand union PASS that the dual-reference gate had been
  declining; the differently-grown body then drove a downstream `fillet` to a mesh
  out of the independent volume bound (`mesh = 116378 > 110369 + slack`) -- a
  `malformed:fillet` WRONG.

**Why the fix is unsafe (the load-bearing lesson).** A watertight body with
`mass == mesh` can still be GEOMETRICALLY WRONG: the analytic mass and the
per-component mesh can agree on an impossible value (the #48 self-consistent-WRONG
phenomenon the gate's own comments cite for sphere/sphere lenses). In those cases
`tessellated_volume` -- precisely because it is a DIFFERENT divergence sum with a
DIFFERENT reference -- disagrees and independently catches the wrong body. The
dual mass-vs-tessellation reference is therefore LOAD-BEARING for WRONG-prevention,
not redundant. Add-288 (LOG, 2026-06-17) already documented the mirror-image
hazard (clearing a warning on a `mass == tess` body whose `mesh` disagreed unmasked
a defective body) and deliberately kept BOTH references. This finding completes the
symmetry: neither `tessellated_volume` nor `mesh_volume` alone is a safe
self-consistency partner for mass; the gate needs both, and tightening to either
one admits a WRONG in one direction or the other. The fix was reverted; baseline
WRONG = 0 and PASS 2361 / 3000 restored.

---

## 6. The honest tractable-vs-intractable boundary

**Tractable within `boolean.rs` (the assembler), but small and risky:**
- The 16% `unmatched-coedge` all-planar stitch failures (seed 15 is the clean
  repro) are a GENUINE assembler frontier: the import-glue
  (`merge_and_glue_imported`) cannot pair the seam coedges into radial-2 where a
  tool's seam meets a compound body's existing prior-boolean edges. This is the
  dossier-47 silent-dropped-face backstop firing correctly (it is NOT a wrong body
  being shipped; it is a real stitch that did not close). Making these ASSEMBLE
  requires junctioning the new seam loop with the target's existing edges at the
  crossing vertices (the prompt's question 2) -- a real seam-junction advance in
  the imprint/glue, not a gate change. High value (4,935 attempts) but high risk
  on a 2.24B-fuzz-validated stitch; needs its own focused round with a watertight
  oracle per fragment.
- The 0.2% pure `tessellated_volume` blow-up is tractable ONLY if paired with a
  WRONG-safe guard that the recovery cannot admit a self-consistent wrong. The
  attempted guard (validate + mass == mesh) is NOT sufficient (it admitted the
  WRONG above). A sufficient guard would need an INDEPENDENT correctness witness
  (e.g. a per-component gwn containment re-check, or an op-volume bound tight
  enough to bound the specific result), which is more than a gate tweak. Until
  such a witness exists, this sub-mode must stay declined.

**Intractable within this scope (a different file or a research advance):**
- The 36% watertight `mass != mesh` sub-mode is the dossier-71 green-slab
  mass-integration class (seed 19 is the clean repro: watertight, `tess == mesh`,
  `mass` off 18%). The analytic `mass_properties` mis-integrates a curved trimmed
  face produced by the boolean. Fixing it lives in `massprops.rs` (the green-slab),
  explicitly forbidden by this scope and correctly so -- it is not an assembler
  bug. This is the largest genuinely-recoverable slice and the highest-value
  follow-up, but it is dossier 71's target, not 73's.
- The 53% non-watertight sub-mode is a genuine assembler residual (the stitch
  produced a cracked body, `open_ratio` up to 0.38) and is CORRECTLY declined
  today. Making these watertight is the same deep seam-junction / region-walk
  advance as the `unmatched-coedge` slice, on busier curved-compound bodies.

**The DECLINE-never-WRONG verdict.** Every decline measured here is a body the
floor SHOULD decline (it is non-watertight, or its mass and tessellation disagree
beyond the chordal band, or a seam genuinely did not assemble) OR a correct body
the gate over-declines for a reason that cannot be relaxed without admitting a
WRONG. There is no "free" recovery: the 59% dominant class is either a different
file's bug (mass integration) or a genuine assembler advance (seam junction /
watertight stitch), and the one gate-level recovery that looked free is provably
not (WRONG = 1). The floor is doing exactly its job.

---

## 7. What a real fix would require (grounding the next round)

For research 0006 #73 to convert these declines to PASSES without a WRONG, in
priority order:

1. **The dossier-71 green-slab mass integration on boolean-produced trimmed curved
   faces (36% of the dominant class).** This is the single biggest recoverable
   slice. The boolean produces watertight bodies whose curved trimmed faces the
   analytic integrator reads wrong (seed 19: 18% under, `tess == mesh` agreeing on
   the truth). Land dossier 71's per-face witness + the trimmed-cylinder/cone
   integration there, and these gate-declines become PASSES automatically (the
   assembler already builds them watertight). NO assembler change needed.

2. **Seam junction at crossing vertices for the all-planar stitch (16%
   `unmatched-coedge`).** When a tool's seam crosses a compound body's existing
   prior-boolean edge, the imprint must split BOTH the new seam and the existing
   edge at the crossing vertex and glue all four coedges, so
   `merge_and_glue_imported` leaves no `radial < 2` edge. Seed 15 is the minimal
   repro (block, two booleans, then a box-tool Difference whose side faces cross
   the prior cut faces). Oracle: per-fragment watertight + mass == mesh on the
   all-planar result (exact, no chordal slack). This is a real imprint/glue advance
   with a clean planar oracle -- the best-isolated assembler slice for a dedicated
   round.

3. **A WRONG-safe independent witness for the gate (enables the 0.2% tail and
   guards #2).** A per-component containment re-check (gwn over the result at
   recentered interior points) or a per-fragment op-volume bound would give the
   gate a correctness signal independent of BOTH tessellations, so a watertight
   `mass == mesh` body could be admitted ONLY when the witness confirms it -- the
   guard the attempted fix lacked. This is the prerequisite that makes any
   gate-level recovery safe.

4. **`UnassemblableSeam` on compound operands (23%).** A genuine crossing where the
   tool meets a prior blend / seam and the imprint cannot assemble the seam loop
   (seed 09 face-5-crosses-many signature). This is the curved seam-junction
   analog of #2 and the hardest; it overlaps the curved-SSI matrix that rounds
   58 to 69 hardened for primitive pairs, now on a compound target.

---

## 8. Reproduction

```
cargo build --release --example realsoak

# Per-seed failing-stage attribution (verbose, the four prompt seeds):
KEEL_REPRO=11400715918834826719 KEEL_BOOL_DEBUG=1 ./target/release/examples/realsoak.exe   # seed 19: mass-integration (dossier 71)
KEEL_REPRO=11400715918834826709 KEEL_BOOL_DEBUG=1 ./target/release/examples/realsoak.exe   # seed 09: mixed
KEEL_REPRO=11400715918834826706 KEEL_BOOL_DEBUG=1 ./target/release/examples/realsoak.exe   # seed 06: mixed
KEEL_REPRO=11400715918834826715 KEEL_BOOL_DEBUG=1 ./target/release/examples/realsoak.exe   # seed 15: pure unmatched-coedge stitch

# Population fault census (3000 projects):
KEEL_FAULT_CENSUS=1 ./target/release/examples/realsoak.exe 3000 1 census-out

# Baseline correctness invariant (WRONG must be 0, assembler unchanged):
./target/release/examples/realsoak.exe 3000 1 baseline-out
```

Verified: full suite 553 passed / 0 failed; `cargo fmt --all --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings -A clippy::while_let_loop
-A clippy::doc_lazy_continuation` exit 0; baseline soak WRONG = 0, PASS 2361 / 3000.

---

## 9. Related Keel dossiers

- **71. Post-fillet curved-face mass** -- the green-slab mass-integration class that
  is 36% of the dominant decline here (a boolean-produced watertight body whose
  curved trimmed face the analytic mass reads wrong). The highest-value #73
  follow-up lives in 71's target file, not the assembler.
- **47. Curved boolean assembly / import-and-glue** -- the `stitch_by_import` /
  `merge_and_glue_imported` shell-closure invariant whose `unmatched coedge`
  failure is the 16% genuine-stitch slice; the seam-junction advance #2 needs lives
  here.
- **59. Curved boolean assembly (BuildPCurves gap)** -- the `UnassemblableSeam`
  class (23%) is the compound-operand surfacing of the curved seam-junction gap.
- **57 / 39. Non-regularized union, on-on tables, touch contact** -- the
  classify/select consistency the gate's op-volume bound and the cellular finalize
  rest on; not the failing stage here (classify/select were never the dominant
  fault).
- **LOG Add 288 (2026-06-17)** -- the mirror-image hazard (clearing an advisory
  fault on a `mass == tess` body whose `mesh` disagreed unmasked a defective body),
  which deliberately kept BOTH tessellation references. This finding completes the
  symmetry and proves the dual reference is load-bearing in both directions.

---

## 10. ADDENDUM (instrumented, GWN witness): seed 719 is NOT a mass bug -- the mesh OVER-reads a malformed body, the analytic mass is correct. The watertight `mass != mesh` slice is a genuine MIX, not uniformly dossier-71.

Section 3 (and section 6/7) attributed seed 11400715918834826719 to the
dossier-71 green-slab mass-integration class ("watertight, `tess == mesh`,
`mass` off 18% ... the analytic `mass_properties` mis-integrates a curved trimmed
face"). A follow-up round instrumented that body with the dossier-71 method (a
per-face flux witness PLUS an INDEPENDENT witness) and **refutes that
attribution**. The 18% gap is the MESH over-reading a geometrically malformed
body; the analytic mass is correct.

### 10.1 The independent witness that settles it

The premise "tess == mesh == 6647 agree, so the mesh is the truth and the
analytic mass (5480) under-reads" has a hidden flaw: `tessellated_volume`
(`winding.rs`) and `mesh_volume` (`interrogate.rs`) are the SAME signed-tetra
divergence sum over the SAME `tessellate_face` triangles, so their agreement is
not two independent measurements -- it is one measurement reported twice. The
analytic mass is a DIFFERENT integrator (a per-face boundary x-flux), so the
real question is which of the two integrators is right, and that needs a THIRD,
independent integrator. The generalized winding number is exactly that: it sums
signed solid angles (Van Oosterom-Strackee, `winding.rs::gwn_over`), depending on
NO divergence/flux reduction. A Monte-Carlo GWN-membership volume over the
assembled body's AABB (400k samples) reads:

```
GWN_WITNESS  analytic_mass 5480.68   gwn_vol 5474.17   (NOT 6647.56)
```

The GWN volume matches the ANALYTIC mass to 0.12%, not the mesh. Three further
instrumented facts agree, each independently decisive:

1. **Per-face flux self-consistency.** Each of the three cylinder lateral bands
   the body carries (axial `v` bands of a full-`2*pi` cylinder, radius 8.502,
   z in [-12.07, 12.07]) has an analytic flux that equals its OWN tessellated
   x-flux to < 0.2% (band dv 3045.66 vs face-local tess flux 3040.77; 1618.56 vs
   1615.96; 816.46 vs 815.15). The green-slab / iso-rectangle integrator reads
   every band CORRECTLY. There is no under-integrated face.

2. **GWN is ~0 inside the box protrusion.** The `extrudeU` tool is a slab that
   crosses the cylinder and protrudes far in +y (to y = 22.58, well past the
   cylinder's y = 8.5 rim). Probing GWN at points inside that protrusion
   (`(0, 15, 0)`, `(0, 20, 0)`) gives `-0.0003` and `-0.0002` -- the body encloses
   NO solid there. The protrusion's planar walls are a dangling open sheet.

3. **Nine radial-1 edges.** The protrusion's walls (faces 5..8) are bounded by
   NINE edges with `radial == 1` (a single incident face) -- the topological
   signature of an open shell. The body is genuinely NON-watertight; the gate's
   `mesh_open_ratio` reading 0.00 is a coincidental net-area-vector cancellation
   (the tube-like sheet's opposite walls have near-cancelling area vectors), a
   FALSE NEGATIVE of the `open_ratio <= 5e-2` watertightness net.

So the analytic mass (5480) integrates the only genuinely-enclosed region (the
closed cylinder, `pi * 8.502^2 * 24.135 = 5481`), the GWN confirms it (5474),
and the signed-tetra `mesh_volume` (6647) over-reads because the dangling sheet's
triangles contribute spurious tetra volume that GWN correctly ignores. This is
the SAME shape as dossier 71's OWN resolution (`tests/post_fillet_mass.rs`): the
integrator was correct and the SURGERY handed it a non-watertight body. Here the
ASSEMBLER (boolean.rs, the union of a crossing slab into a cylinder) handed the
integrator a non-watertight body. **It is NOT a `massprops` integration bug.**
Forcing the analytic mass up to 6647 would report a WRONG volume for a WRONG body
and break DECLINE-never-WRONG. The gate's decline is correct.

### 10.2 The slice is a genuine MIX (refines section 4.1's 36%)

A population census (`KEEL_GWN_CENSUS`, ~1180 watertight `mass != mesh` events
over a 500-project soak) compared, per event, whether the independent GWN tracks
the ANALYTIC mass (=> mesh over-reads, NOT a mass bug, seed-719-like) or the MESH
(=> analytic genuinely mis-integrates, the real dossier-71 class). It splits
roughly evenly, and the genuine-mis-integration cases run in BOTH directions,
GWN-confirmed (e.g. analytic 985.7 vs mesh/gwn ~1438, a 31% UNDER; analytic 816.8
vs mesh/gwn ~223, a 266% OVER). So the section-4.1 "36% watertight `mass != mesh`
= dossier-71 green-slab" bucket is really two classes:
- a large seed-719-like sub-class where the body is malformed (open sheet that
  passes the `open_ratio` net) and the MESH over-reads -- an ASSEMBLER /
  watertightness-net frontier, NOT massprops;
- a genuine, heterogeneous green-slab over/under-integration remainder (the
  dossier-71 target proper), which is NOT a single tractable fix -- the cases span
  many distinct trimmed-face configurations and both error directions.

### 10.3 Consequence for the #73 follow-up priority

Section 7 ranked "the dossier-71 green-slab mass integration (36%)" as the single
biggest recoverable slice, to be fixed in `massprops.rs` with "NO assembler change
needed." This addendum corrects that: a large part of that 36% is the
assembler/watertightness-net frontier (seed 719 is the clean repro), where the
fix is NOT in `massprops.rs` (the integrator is already exact and GWN-consistent
on the cylinder bands -- see `tests/cyl_union_mass_witness.rs`). The genuine
green-slab remainder is real but smaller and heterogeneous. Two concrete leads
the GWN witness surfaces for the next round:
1. **A watertightness net that catches radial-1 sheets.** The `open_ratio`
   net-area-vector test false-negatived seed 719's open protrusion. A
   radial-count check (every edge `radial >= 2`) or a GWN-based interior probe
   would decline these malformed bodies on a CORRECT signal rather than letting
   the `mass != mesh` gate decline them for the wrong stated reason. (Still a
   DECLINE -- the body IS wrong -- but attributed to the assembler, where the
   real fix lives.) `boolean.rs` / watertightness territory, not massprops.
2. **GWN as the gate's independent correctness witness** (section 7 item 3): the
   Monte-Carlo GWN volume IS the WRONG-safe witness the gate lacks. It already
   discriminates the seed-719 malformed class (GWN tracks analytic) from a genuine
   integrator error (GWN tracks mesh); a (faster, e.g. importance-sampled or
   deterministic-quadrature) GWN volume could let the gate ACCEPT a watertight
   `mass == gwn` body even when the chord-mesh disagrees, and DECLINE a malformed
   one -- closing the dossier-73 tractability gap that a pure `mass == mesh`
   relaxation could not (it admitted WRONG = 1).

Reproduce: `KEEL_REPRO=11400715918834826719 KEEL_BOOL_DEBUG=1` (the gate values),
plus the env-gated witnesses used here (a per-face flux witness and a Monte-Carlo
GWN-membership volume; reverted after the attribution, the dossier-71 method).
Regression lock: `crates/keel-topo/tests/cyl_union_mass_witness.rs` asserts the
curved-band integrator is exact and GWN-consistent on the multi-band cylinder
seed 719 exercises (a tube and a stepped shaft), the positive invariant behind
this finding.
