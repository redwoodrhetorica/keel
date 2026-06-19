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

UPDATE (this round, the co-wound rung CLOSED): the co-wound hole now ASSEMBLES.
The merge step (`mekr`) produces a SIMPLE merged loop only when the inner hole
loop is wound OPPOSITE the outer loop (the B-rep convention). A boolean-built
operand can store an inner loop in EITHER winding, and seed 715's OWN +x-face
hole is CO-WOUND (the failing op's body comes from an `extrude`-built compound).
The prior round believed reversing the live inner loop was UNSAFE (that flipping
the hole fins' `forward` would break the manifold pairing with the adjacent boss
walls). Instrumenting the operand (`KEEL_COWOUND_PROBE`, retained env gate)
REFUTED that: each shared hole edge carries exactly two fins (the +x-face hole
fin and the wall-face fin), and the kernel's radial invariant
(`check_radial_cycles`) requires only that each fin sit in exactly ONE radial
cycle -- it does NOT constrain the two fins' relative `forward`. A wall face
reads its OWN fin's direction for its material side, never the hole loop's fin.
So reversing the hole loop in place (swap each loop fin's `next`<->`prev`, flip
each `forward`) is a VALID local operation: it touches only the hole loop's own
fins, leaves every wall loop and every edge's radial set byte-identical, and
preserves fin-ring continuity, boundary-chain continuity, and the radial cycles
(verified by `debug_validate` on the real co-wound seed-715 body). After the
subsequent `mekr` the hole ceases to be an inner loop at all (it merges into the
+x outer loop), so no co-wound inner loop survives to mislead a downstream
consumer. The overlay therefore CANONICALIZES a co-wound dipped hole to
counter-wound (`reverse_inner_loop`) BEFORE the re-knit and then runs the proven
counter-wound path unchanged. Seed 715's own failing Difference now assembles
WATERTIGHT at the exact box-CSG oracle (mass == mesh == 12932.10); the
Intersection direction is exact too (3096.32). DECLINE-never-WRONG is preserved:
the reversal is purely combinatorial (no geometry moves), and if it ever produced
a malformed body the all-planar mass==mesh gate would still reject it (the floor
is unchanged). See section 4 (rewritten) and section 5b (this round's validation).

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

## 4. The hole-winding boundary: now RESOLVED by local loop reversal

mekr bridges an inner ring into the outer loop by splicing the merged ring to
traverse the hole FORWARD from the bridged fin. The result is a SIMPLE loop
(material on the correct side) only when the hole is wound OPPOSITE the outer loop
(the conventional B-rep inner-ring winding). The kernel does NOT enforce inner-
ring winding globally: a boolean-built operand stores whatever winding the stitch
produced, and consumers normalize by relative sign (e.g. `massprops.rs` reverses a
hole polygon for integration when `signed_area(inner).signum() ==
outer_sign`). Seed 715's +x-face hole, built by the `extrude` Union, is CO-WOUND;
the clean-Union box+boss (dossiers 73b/73c, `box_with_boss`) is counter-wound,
which is why those isolated cases assembled out of the box.

For a co-wound hole the forward mekr merge folds the void IN as material. The
measured signature on seed 715: the two split sub-faces' tessellated areas sum to
outer PLUS hole, versus the correct outer MINUS hole.

The prior round believed this was unfixable by a local operation, on the premise
that reversing the live inner loop would break the hole edges' manifold pairing
with the boss walls. **That premise was wrong, and this round refuted it with
direct instrumentation.** `KEEL_COWOUND_PROBE` (retained env gate) dumps each
co-wound dipped hole's fin ring on the real seed-715 operand. The data
(reproduced below) shows each shared hole edge carries exactly TWO fins -- the
+x-face hole fin and the wall-face fin -- and BOTH are stored `forward = true`
(same direction), yet the body validates. The reason: the kernel's radial
invariant `validate::check_radial_cycles` requires only that each fin sit in
exactly ONE radial cycle; it imposes NO constraint on the two fins' relative
`forward`. And `check_boundary_chains` is a PER-LOOP continuity check (each fin's
end vertex equals the next fin's start vertex WITHIN its own loop); it never
relates a hole-loop fin to a wall-loop fin. A wall face reads its OWN fin's
direction for its material side. So reversing the hole loop touches nothing the
wall faces depend on.

```
COWOUND: hole 0 loop ... cowound=true
  fin .. fwd=true radial=2 (..,6.12,3.13)->(..,-3.48,3.13) others=[(.., fwd=true, wall face 6)]
  fin .. fwd=true radial=2 (..,-3.48,3.13)->(..,-7.09,3.13) others=[(.., fwd=true, wall face 6)]
  ... (every hole edge: radial-2, the OTHER fin on a boss wall, same fwd)
```

`reverse_inner_loop(lp)` performs the reversal as a purely combinatorial local
op: for every fin in the inner loop, swap `next` <-> `prev` (reverse the cycle)
and flip `forward` (so the fin traverses its edge the other way; a reversed
fin's start/end vertices swap, keeping the now-reversed chain end-to-start
continuous). It touches ONLY this loop's own fins; each shared edge keeps its
exact radial set (the same fin keys), so `check_radial_cycles` is preserved, and
the adjacent wall loops are untouched. After the reversal the hole is
counter-wound and the proven counter-wound re-knit (mev spurs + mekr bridge +
split_face) runs unchanged; the mekr then MERGES the hole into the +x outer
loop, so no co-wound inner loop survives. `debug_validate` runs through the
reversal and the full re-knit on the real co-wound seed-715 body with no panic
(every invariant holds), and the result is mass == mesh == the exact oracle.

DECLINE-never-WRONG is preserved with margin: the reversal moves NO geometry
(crossing/spur/bridge/cut points are exact chain or existing-edge points), and
the result still passes the load-bearing dual mass-vs-tessellation gate. If the
reversal ever produced a malformed body (it does not), that gate plus the
shell-closure net would DECLINE it, never ship it. This is dossier 76 sec 4
option (a) -- inner-loop winding canonicalization -- realized at the cheapest
correct site (the imprint's working operand, exactly where the dip is detected),
rather than a global stitch pass that would have to reverse loops it cannot prove
need reversing. The decline path remains for any hole the reversal helper cannot
form a well-formed loop from (returns None -> fall through to the existing
shell-closure decline).

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

## 5b. This round (co-wound rung): validation, WRONG = 0, the seed-715 op CLOSED

Apples-to-apples, SAME base (this worktree's fork commit, the prior co-wound-
HARDENING HEAD), deterministic seed 1, the FULL 10000-project soak (not 3000):

| metric | baseline (decline co-wound) | with reversal | delta |
|---|---:|---:|---:|
| WRONG (decline-never-wrong) | 0 | 0 | **0** (invariant HELD) |
| PASS (10000 projects) | 7970 | 7984 | **+14** |
| KERNEL-FRONTIER declines | 65945 | 65677 | **-268** |
| failing attempts: unmatched-coedge | 16253 | 15776 | **-477** |
| failing attempts: UnassemblableSeam | 24378 | 24459 | +81 |
| failing attempts: degenerate(mass!=mesh) | 62848 | 62912 | +64 |
| total declines | 108538 | 108348 | **-190** |

The co-wound canonicalization converts the dominant compound-operand residual:
the `unmatched-coedge` class (the shell-closure decline the seed-715 op hit)
drops by 477 attempts, the kernel-frontier decline count by 268, and PASS rises
by 14 with WRONG = 0. The small `UnassemblableSeam +81` / `degenerate +64` are the
same downstream shift 73b/73c/76 documented: a now-completing co-wound imprint
lets a chain progress to LATER ops that decline on their own merits (a different
tool, a genuine mass!=mesh), not a regression. WRONG = 0 across all 10000
projects (both lanes) is the decisive floor signal.

Seed 715's OWN failing op is now CLOSED. `KEEL_REPRO=11400715918834826715`
replays it; its compound-operand Difference now ASSEMBLES at mass == mesh ==
12932.10 (`ARR: co-wound dipped hole 0 -> reversing to counter-wound` ->
`ARR: DONE counter-wound arrangement re-knit`), where it previously declined
`unmatched coedge`. (The seed's later ops, run on the now-evolved body, still
decline on their own merits; the realized-op count is unchanged at 3, the
compound Difference being the one this round converts.)

New regression test (in `boolean.rs`):
- `overlay_cowound_hole_dip_assembles_at_exact_oracle`: builds the seed-715
  box+boss body, FORCES the +x-face hole CO-wound (via `reverse_inner_loop`, the
  seed-715 storage), asserts it is co-wound and still validates, then asserts the
  overlay canonicalizes it and assembles BOTH Difference and Intersection at the
  EXACT box-CSG oracle with mass == mesh (no slack). The `debug` build runs the
  reversal and the full re-knit under `debug_validate` with no panic.
- `overlay_battery_is_decline_never_wrong` (extended): the same battery now runs
  against BOTH a counter-wound and a forced-co-wound body; every assembled result
  is a watertight all-planar body whose mass equals its mesh exactly, in both
  windings. The sacred-floor guard, now covering the co-wound path.

Suite: `cargo test --release` GREEN (keel-topo lib 296 passed = 295 prior + 1
new co-wound test; all binaries 0 failed). `cargo fmt --all --check` exit 0.
`cargo clippy --release --all-targets -- -D warnings -A clippy::while_let_loop -A
clippy::doc_lazy_continuation` exit 0. The change is confined to
`crates/keel-topo/src/boolean.rs` (`reverse_inner_loop` + the dispatch in
`try_imprint_chain_arrangement`); `massprops.rs` and `blend.rs` untouched; every
floor gate unchanged.

---

## 6. Reproduction

```
cargo build --release --example realsoak

# Seed 715's co-wound-hole op now ASSEMBLES: the overlay fires, detects the
# co-wound hole, REVERSES it to counter-wound, and the Difference completes at
# mass == mesh == 12932.10 (look for "reversing to counter-wound" then "DONE"):
KEEL_REPRO=11400715918834826715 KEEL_ARR_DEBUG=1 ./target/release/examples/realsoak.exe

# Optional: dump a co-wound dipped hole's fin ring (the data that refuted the
# prior "reversal is unsafe" obstruction -- each shared edge is radial-2, the
# other fin on a wall face, and the radial invariant ignores relative `forward`):
KEEL_REPRO=11400715918834826715 KEEL_ARR_DEBUG=1 KEEL_COWOUND_PROBE=1 \
  ./target/release/examples/realsoak.exe

# The reconstructed clean-Union body+tool (counter-wound hole) assembles exactly
# (mass == mesh == box-CSG, matches the independent Monte-Carlo truth):
cargo run --release -p keel-topo --example probe_sb3

# Population: WRONG = 0; PASS 7984 (vs 7970 baseline), kernel-frontier 65677
# (vs 65945), unmatched-coedge attempts 15776 (vs 16253):
KEEL_FAULT_CENSUS=1 ./target/release/examples/realsoak.exe 10000 1 out

# The landed slice (regression tests, incl. the new co-wound oracle):
cargo test -p keel-topo --lib -- overlay_ seam_crosses_hole compound_operand
```

---

## 7. Related dossiers

- **73c** -- the single-hole-dip routine this overlay subsumes and generalizes
  (its first-and-last-segment trigger excluded a material interior vertex; the
  overlay handles the stacked material-vertex + dip). 73c section 5 named exactly
  this compound case as the next rung; landed for the counter-wound hole, and now
  CLOSED for the co-wound hole too (section 4, this round).
- **73b** -- the open-chain T-junction repair; its plain spur + split path is the
  D = 0 (no hole dipped) case of the overlay's walk, unchanged.
- **73** -- the attribution that isolated the 16% `unmatched-coedge` slice and
  named seed 715.
- **47** -- the import-and-glue / shell-closure invariant, confirmed CORRECT
  throughout: it correctly declined the co-wound dip before this round (a then-
  genuinely-unmergeable face set), and the fix is the dossier-76 sec-4 option (a)
  inner-loop winding canonicalization. This round realizes that canonicalization
  at the cheapest correct site -- the imprint's working operand, exactly where the
  co-wound dip is detected (`reverse_inner_loop`) -- rather than a global
  `merge_and_glue_imported` orientation pass that would have to reverse loops it
  cannot prove need reversing. 47's "carry identity, assert closure, never
  silently drop" doctrine is intact; the imprint now hands the stitch a mergeable
  face set, so the closure check passes instead of declining.
