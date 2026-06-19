# 76 (FINDINGS). The unified per-face planar-overlay imprint: arrange {outer loop, hole loops, seam chain} on one compound face and re-form the sub-faces in one pass, with the exact hole-winding tractability boundary

## Status and posture

This is the deliverable for research request 0007 #76 (the per-face planar
overlay for the compound-operand boolean imprint), the follow-up to dossiers 73,
73b, 73c. The dossier-71/73 method was applied: instrument the failing imprint on
the exact repro (seed 11400715918834826715), build the construction from proven
Euler operators, verify against the exact all-planar oracle, and characterize the
remainder precisely.

Headline: the overlay is LANDED for the conventional (counter-wound hole) case
and SUBSUMES the two prior rungs. A single open seam chain on a planar compound
face -- carrying zero-or-more MATERIAL interior vertices (a sibling-seam
T-junction) AND dipping through one-or-more inner HOLE loops -- is resolved in one
pass: the planar arrangement is walked once, material interior vertices become
`mev` spurs, each hole dip becomes a `mekr` bridge that merges the hole into the
outer loop, and a final `split_face` divides the face into the two correct
sub-faces. This GENERALIZES dossier 73c (which resolved only a single hole dip on
the first-and-last chain segment with NO material interior vertex) and dossier 73b
(no hole crossed: pure spurs + a final split). The seed-715 STRUCTURE (a material
interior vertex stacked with a hole dip on one chain) now assembles WATERTIGHT at
the exact box-CSG oracle, mass == mesh, for a counter-wound hole.

The precise tractability boundary: the merge step (`mekr`) produces a SIMPLE
merged loop only when the inner hole loop is wound OPPOSITE the outer loop (the
B-rep convention). A boolean-built operand can store an inner loop in EITHER
winding, and seed 715's OWN +x-face hole is CO-WOUND (the failing op's body comes
from an `extrude`-built compound, not a clean Union). For a co-wound hole the
forward `mekr` traversal would enclose the void as MATERIAL (a self-touching
figure-8 face), and reversing a live inner loop in place is NOT a valid local
operation (it flips every shared edge's manifold pairing with the adjacent boss
faces). So the overlay DECLINES the co-wound dip cleanly (before any mutation,
falling through to the existing shell-closure decline), never shipping a
self-touching body. Seed 715 therefore still declines its specific op; it is the
co-wound-hole rung, characterized in section 5 and left for a dedicated round.

House style: no em-dashes; ranges written with "to".

Base: HEAD `3031fb7` (dossier 73c landed, LOG Add 297), branch
`swap-m39-close` worktree. The change is confined to
`crates/keel-topo/src/boolean.rs` (the imprint). `massprops.rs` and `blend.rs`
are untouched; every floor gate (dual mass-vs-tessellation, op-volume bound,
watertightness net) is unchanged. The 2.24B-fuzz correctness program stands;
soak WRONG = 0.

---

## 1. The load-bearing result, stated once

> **On a planar compound face (outer loop + K inner hole loops) an open seam
> chain that runs outer-boundary to outer-boundary can stack TWO features the
> per-chain `split_face` cannot compose: MATERIAL interior vertices (where the
> chain T-junctions a sibling seam on the same face) and HOLE DIPS (where the
> chain runs through a hole's void). The unified overlay walks the chain once,
> classifying each vertex material-or-inside-hole-h, and emits a SPUR event per
> material interior vertex and a DIP event per hole traversal (entry P_enter,
> exit P_exit). The re-knit, built entirely from proven Euler operators: pre-split
> the outer loop at the chain ends and each dipped hole at its crossings; then for
> each event in chain order, `mev`-spur a material vertex on the working loop, or
> `mekr`-bridge tip -> P_enter (the hole merges into the outer loop; the in-hole
> run is discarded void; resume at P_exit on the merged loop); finish with one
> `split_face` to the chain's far end. This subsumes 73c (a single DIP, no SPUR)
> and 73b (no DIP, pure SPURs). It is CORRECT (watertight, mass == mesh == exact
> box-CSG oracle) when each dipped hole is wound OPPOSITE the outer loop. mekr
> traverses the merged ring FORWARD, so a CO-WOUND hole would fold the void in as
> material (a figure-8 face whose tessellated area equals outer PLUS hole, the
> direct measured signature). Reversing a live inner loop is not a valid local
> op, so the overlay certifies only the counter-wound hole and declines the
> co-wound one before any mutation. DECLINE-never-WRONG: it performs provably
> valid Euler re-knits on a certified structure and never moves geometry (crossing
> points lie on the existing hole edges within tol; spur/bridge/cut endpoints are
> exact chain points).**

---

## 2. Method: instrument-then-attribute on seed 715

`KEEL_REPRO=11400715918834826715 realsoak` replays the seed; its
`booleanD`/`booleanI` (ops 2 and 3, body `g0 f11` after an `extrude` that built
the box+boss compound) decline `AssemblyFailed("unmatched coedge: shell-closure
invariant violated")`. A chain dump added to the open-chain dispatch
(`KEEL_CHAIN_DUMP`, reverted) showed the +x-face (plane x = 9.5518) chain on the
failing op:

```
chain = [(9.55, 11.67, 8.97), (9.55, -3.48, 8.97), (9.55, -3.48, -9.17), (9.55, 11.67, -9.17)]
outer = box +x face,  y in [-11.67, 11.67]  z in [-15.28, 15.28]
hole  = boss footprint, y in [-7.09, 6.12]  z in [-9.65, 3.13]  (6 vertices, two from this op's Phase-1)
```

In the face's 2D (y, z) frame the chain is A(11.67, 8.97) outer -> B(-3.48, 8.97)
MATERIAL -> C(-3.48, -9.17) INSIDE the hole -> D(11.67, -9.17) outer. So it
stacks a material interior vertex (B) and a hole dip (segment B->C enters the
hole top, C->D exits): EXACTLY the compound case dossier 73c (section 5)
characterized as needing the full overlay, and the case 73c's first-and-last-
segment trigger correctly excluded.

A standalone probe (`crates/keel-topo/examples/probe_sb3.rs`) reconstructs the
body (base box UNION boss, the same `g0 f11`, identical AABB to the soak body)
and the seed-715 Difference/Intersection tools from the dump, runs the op, and
adjudicates with an independent Monte-Carlo CSG truth (8M samples). An arrangement
trace (`KEEL_ARR_DEBUG`, retained, env-gated) reports the events, the dipped
holes, and the co-wound decline.

---

## 3. The construction (one localized routine, WRONG-safe)

All in `crates/keel-topo/src/boolean.rs`.

### 3.1 `try_imprint_chain_arrangement` (new), dispatched from `imprint_open_chain`

Replaces the dossier-73c `try_imprint_chain_through_hole` (removed; the new
routine subsumes it). Dispatched from `imprint_open_chain` BEFORE the boundary-
vertex precondition, only when the face has more than one loop. It:

1. PLANAR carrier only (a curved hole falls through to decline).
2. Requires both extreme chain endpoints on the OUTER loop.
3. Classifies every chain vertex material (outside every hole) or strictly inside
   exactly one hole (inside more than one hole -> decline; the endpoints must be
   material).
4. Walks the chain once, building an event list in chain order:
   - a MATERIAL interior vertex -> a `Spur` event;
   - a maximal in-hole run flanked by material -> a `Dip` event (P_enter on the
     entry segment's hole crossing, P_exit on the exit segment's), requiring every
     interior vertex of the run inside the SAME hole, the run's midpoint genuinely
     inside the hole (a traversal, not a double graze), and each hole dipped at
     most once. Any other transition (e.g. the chain starting inside a hole)
     declines.
5. Certifies, BEFORE any mutation, that each dipped hole is wound OPPOSITE the
   outer loop (signed-area sign comparison in the face frame). A co-wound hole
   declines here (section 4), so the fall-through plain path sees an untouched
   face.
6. Re-knit: pre-split outer at chain[0]/chain[last] and each dipped hole at its
   crossings; then for each event, `mev`-spur (tip -> material vertex) or
   `mekr`-bridge (tip -> P_enter, merge hole, resume at P_exit); finish with
   `split_face`(last tip -> chain[last]). The seam geometry is attached to each
   spur, bridge, and cut edge; the in-hole run carries no edge.

Returns `Ok(None)` (fall through to the unchanged split / decline) for any
structure it does not certify. It never moves geometry.

### 3.2 V-E-F bookkeeping (generalizing the 73b/73c special cases)

Per chain, with S material interior vertices and D hole dips: the re-knit adds S
spur edges (`mev`: +1 V, +1 E each), D bridge edges (`mekr`: +1 E each, and -1
inner loop each as the hole merges into the outer loop, so genus/inner-ring count
falls by D), and 1 split edge (`mef` via `split_face`: +1 E, +1 F). Euler-Poincare
is preserved by construction because every step is a single proven operator.
Dossier 73c is the case S = 0, D = 1; dossier 73b's open chain is D = 0 (the
arrangement falls through to the plain spur + split path, unchanged).

### 3.3 Effect on the repro (the instrumented walk)

- The seed-715 +x-face chain FIRES the overlay (`cls = [None, None, Some(0),
  None]`, one Spur + one Dip), but the hole is CO-WOUND (`fin_ring.next` from
  P_enter heads the wrong way around the hole; the figure-8 merged face's
  tessellated area measures outer PLUS hole instead of outer MINUS hole), so the
  overlay declines BEFORE mutating and the op falls through to the existing
  shell-closure decline (the dangler count returns to the clean baseline; no
  mangling). Seed 715 still declines, cleanly, WRONG-safe.
- The SAME seed-715 body+tool with a counter-wound hole (the clean-Union
  reconstruction in `probe_sb3` and in the regression test) ASSEMBLES WATERTIGHT:
  Difference mass == mesh == 12932.10 (exact box-CSG; MC truth 12937, within
  sampling noise), Intersection mass == mesh == 3096.32 (MC 3092). This is the
  seed-715 STRUCTURE (material vertex + hole dip) resolved exactly, the advance
  over 73c.

---

## 4. The tractability boundary: hole winding

mekr bridges an inner ring into the outer loop by splicing the merged ring to
traverse the hole FORWARD from the bridged fin. The result is a SIMPLE loop
(material on the correct side) only when the hole is wound OPPOSITE the outer loop
(the conventional B-rep inner-ring winding). The kernel does NOT enforce inner-
ring winding globally: a boolean-built operand stores whatever winding the stitch
produced, and consumers normalize by relative sign (e.g. `massprops.rs` reverses a
hole polygon for integration when `signed_area(inner).signum() ==
outer_sign`). Seed 715's +x-face hole, built by the `extrude` Union, is CO-WOUND;
the clean-Union box+boss (dossiers 73b/73c, `box_with_boss`) is counter-wound,
which is why those isolated cases assembled.

For a co-wound hole the forward mekr merge folds the void IN as material. The
measured signature on seed 715: the two split sub-faces' tessellated areas sum to
~882 = outer (713) PLUS hole (169), versus the correct ~544 = outer MINUS hole
(the counter-wound probe gives 544 exactly). Reversing the live inner loop to fix
this is NOT a valid local operation: the hole-loop edges are shared (radial-2)
with the adjacent boss faces, and flipping the hole-loop fins' direction breaks
those edges' manifold pairing (the kernel's boundary-chain-continuity invariant,
`validate::check_boundary_chains`, then fails, or the downstream orientation
consumers read the wrong side). Reversing the whole face flips outer AND hole
together, leaving the RELATIVE winding unchanged. So a co-wound dip cannot be
bridged into a simple loop by a local Euler sequence on the as-stored operand.

The overlay therefore CERTIFIES the counter-wound hole and DECLINES the co-wound
one. This is the honest boundary. Resolving the co-wound case needs one of:
(a) the boolean import/stitch producing inner loops in the canonical (counter-
wound) winding (a `merge_and_glue_imported` orientation pass, the natural place,
out of this round's imprint scope); or (b) a non-Euler arrangement re-extraction
that rebuilds both sub-faces' fin rings from the 2D arrangement directly (a
larger DCEL-to-B-rep import, rather than the in-place Euler re-knit landed here).
Both are the next rung; the all-planar exact oracle still applies.

---

## 5. Validation: zero regression, WRONG = 0, a real population gain

Apples-to-apples, SAME base (HEAD `3031fb7`), the per-attempt fault census is
deterministic, so the census deltas are real signal (PASS fluctuates +/-1 on STALL
timing, as in 73b/73c).

| metric | baseline (HEAD, 73c) | with overlay | delta |
|---|---:|---:|---:|
| WRONG (decline-never-wrong) | 0 | 0 | **0** |
| PASS (3000 projects) | 2358 | 2359 | +1 (STALL-timing noise) |
| failing attempts: unmatched-coedge | 5146 | 5167 | +21 |
| failing attempts: UnassemblableSeam | 7432 | 7349 | **-83** |
| failing attempts: degenerate(mass!=mesh) | 19231 | 19219 | -12 |
| failing attempts: other AssemblyFailed | 266 | 266 | 0 |
| total failing attempts | 32912 | 32856 | **-56** |

The overlay converts a net -56 failing boolean ATTEMPTs (most visibly -83
`UnassemblableSeam`: a now-completing material-vertex+dip imprint lets a chain
progress to ops that previously could not even seam, the same downstream shift
73b/73c saw; the small `unmatched-coedge` +21 is cases that now reach the stitch
and hit the co-wound-hole residual). The gain is bounded by the co-wound
limitation (section 4): the counter-wound material-vertex+dip chains convert,
the co-wound ones (including seed 715's own op) decline. WRONG = 0 is the decisive
signal: the certified counter-wound re-knit and the pre-mutation co-wound decline
never ship a wrong or non-watertight body.

Suite: `cargo test --workspace` GREEN, 0 failed (keel-topo lib 295 passed = 293
baseline + 2 new). `cargo fmt --all --check` exit 0. `cargo clippy --workspace
--all-targets -- -D warnings -A clippy::while_let_loop -A
clippy::doc_lazy_continuation` exit 0.

Regression tests (in `boolean.rs`):
- `overlay_material_vertex_plus_hole_dip_assembles_at_exact_oracle`: the seed-715
  STRUCTURE (a chain with a material interior vertex AND a hole dip) on the
  reconstructed seed-715 body with a COUNTER-wound hole; asserts WATERTIGHT
  Difference AND Intersection results at the EXACT box-CSG oracle with mass ==
  mesh (no slack). This is the advance over 73c (whose single-dip routine excluded
  a material interior vertex).
- `overlay_battery_is_decline_never_wrong`: a battery of material-vertex and/or
  hole-dip tools against the box+boss body in BOTH boolean directions; asserts
  EVERY result is a clean decline OR a watertight all-planar body whose mass
  equals its mesh EXACTLY, never a wrong-positive. The sacred-floor guard.
- The dossier-73b (`compound_operand_*`) and 73c (`seam_crosses_hole_*`) tests
  still pass unchanged (non-regression: the overlay subsumes both rungs).

---

## 6. Reproduction

```
cargo build --release --example realsoak

# Seed 715 still declines its co-wound-hole op, cleanly (the overlay fires,
# detects the co-wound hole, and declines before mutating):
KEEL_REPRO=11400715918834826715 KEEL_ARR_DEBUG=1 ./target/release/examples/realsoak.exe

# The reconstructed body+tool with a counter-wound hole ASSEMBLES exactly
# (mass == mesh == box-CSG, matches the independent Monte-Carlo truth):
cargo run --release -p keel-topo --example probe_sb3

# Population: WRONG = 0, total failing attempts 32856 (vs 32912 baseline):
KEEL_FAULT_CENSUS=1 ./target/release/examples/realsoak.exe 3000 1 out

# The landed slice (regression tests):
cargo test -p keel-topo --lib -- overlay_ seam_crosses_hole compound_operand
```

---

## 7. Related dossiers

- **73c** -- the single-hole-dip routine this overlay subsumes and generalizes
  (its first-and-last-segment trigger excluded a material interior vertex; the
  overlay handles the stacked material-vertex + dip). 73c section 5 named exactly
  this compound case as the next rung; landed here for the counter-wound hole and
  characterized for the co-wound one.
- **73b** -- the open-chain T-junction repair; its plain spur + split path is the
  D = 0 (no hole dipped) case of the overlay's walk, unchanged.
- **73** -- the attribution that isolated the 16% `unmatched-coedge` slice and
  named seed 715.
- **47** -- the import-and-glue / shell-closure invariant, confirmed CORRECT
  again: it fires on a genuinely incomplete face set (the co-wound dip the overlay
  declines), exactly as its "carry identity, assert closure, never silently drop"
  doctrine intends. The co-wound-hole fix belongs upstream in
  `merge_and_glue_imported`'s orientation handling (section 4 option a), 47's
  territory, not the imprint's.
