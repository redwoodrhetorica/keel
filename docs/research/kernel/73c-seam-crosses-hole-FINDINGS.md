# 73c (FINDINGS). Seam-crosses-hole: a landed face-with-hole arrangement split for the isolated case, and the precise boundary where seed 715 still needs the compound (hole + sibling-T-junction) arrangement

## Status and posture

This is the follow-up to dossier 73b, attacking the RESIDUAL it characterized
(73b section 5): a seam chain that crosses a PRE-EXISTING INNER-LOOP HOLE on a
compound-operand planar face. The dossier-71/73/73b method was applied: instrument
the failing split on the real repro, find the TRUE failing stage and root cause,
and fix the tractable structure precisely while characterizing the harder
remainder exactly.

Headline: the seam-crosses-hole split is **NOT** a `split_face` (mef) bug and
**NOT** a stitch bug. The true root is that `imprint_open_chain` resolves a chain
that dips through a hole's void interior with a single boundary-to-boundary
`split_face`, which routes the WHOLE inner hole loop to one side and strands the
hole-boundary fragments on the other (the shell-closure check then correctly
declines the incomplete face set). The fix is a **face-with-hole arrangement
re-knit** built from the existing proven Euler operators (`mekr` to bridge the
hole into the outer loop, then `split_face` for the second cut), gated to the
TRACTABLE structure (one hole, two crossings, the in-between run inside the hole).
It lands the isolated case (corner-clip + through-hole tools assemble watertight
at the EXACT all-planar oracle) with zero regression and WRONG = 0. Seed 715's
own failing op is a HARDER COMPOUND case (the same wall ALSO T-junctions sibling
boss-top/bottom seams on that face, so the chain carries a MATERIAL interior
vertex), which the conservative trigger declines cleanly rather than attempting a
partial re-knit; it is characterized precisely and left for a compound-arrangement
round.

House style: no em-dashes; ranges written with "to".

Base: HEAD `416a421` (dossier 73b imprint T-junction repair), branch
`swap-m39-close` worktree. The change is confined to
`crates/keel-topo/src/boolean.rs` (the imprint). `massprops.rs` and `blend.rs`
are untouched; every floor gate (dual mass-vs-tessellation, op-volume bound,
watertightness net) is unchanged. The 2.24B-fuzz correctness program stands.

---

## 1. The load-bearing correction, stated once

> **When an open seam chain on a planar compound face whose boundary carries an
> inner HOLE loop runs from the outer boundary, THROUGH the hole's void interior,
> and back to the outer boundary, `imprint_open_chain` splits the face with a
> single `split_face` between the two outer-boundary endpoints. `mef` (the engine
> under `split_face`) moves the fins of ONE loop between the two cut fins and
> leaves every OTHER loop of the face with the old face. So the inner hole loop
> goes WHOLESALE to one side; the cut chain's middle leg, which lies inside the
> hole's void, is imprinted as real material edges that bound nothing; and the
> hole-boundary fragments on the other side of the cut are left radial-1. The
> stitch's shell-closure check then declines the genuinely incomplete face set
> (the seed-715 `unmatched coedge`). The correct resolution is a planar
> face-with-hole ARRANGEMENT: the chain crosses the hole boundary at two points
> P1, P2; the portion between them (inside the hole) is discarded; the two
> material sub-chains are outer-end -> P1 and P2 -> outer-end. Bridging the first
> (`mekr`) MERGES the hole into the outer loop (the face becomes simply
> connected), and the second cut (`split_face`, now both endpoints on the single
> merged loop) divides the face into the two correct sub-faces, each carrying its
> portion of the (now-merged) hole boundary. Both sub-faces are watertight within
> the operand. This is built ENTIRELY from proven Euler operators, so validity
> and lineage hold by construction. It is gated to the structure where the chain
> ENTERS the hole on its first segment and EXITS on its last (every interior
> chain vertex inside the hole): the seed-715/corner-clip "L or U dips into the
> hole" shape. A chain with a MATERIAL interior vertex (one that also
> T-junctions a sibling seam on the same compound face) is a harder compound
> arrangement and is DECLINED (fall through to the existing shell-closure
> decline), never partially re-knit.**

---

## 2. Method: instrument-then-attribute on the minimal repro

The seed-715 topology is reproduced exactly by `box_with_boss()` (the dossier-73b
test helper): a box `[0,10]^3` Unioned with a boss `x[10,15] y[3,7] z[3,7]`, so
the box +x face (plane x=10) carries an inner-loop HOLE = the boss footprint
`y[3,7] z[3,7]`. A throwaway env-gated probe (`KEEL_SCH_DEBUG`, reverted) over a
battery of Difference tools, with the per-attempt loop/chain dump added to
`imprint_open_chain`, isolated the mechanism:

1. **Which loops does the chain touch?** For the cleanest repro (tool
   `[7,14] x [5,12] x [5,12]`, a corner clip of the hole), the chain on the +x
   face is `[(10,10,5), (10,5,5), (10,5,10)]`. BOTH extreme endpoints are on the
   OUTER loop; the MIDDLE vertex `(10,5,5)` is strictly INSIDE the hole
   `y[3,7] z[3,7]`. So the cut dips through the hole's void.
2. **What does `split_face` do with it?** It splits outer-to-outer and leaves the
   hole loop wholesale on the old face. The middle leg's two segments cross the
   hole boundary at `(10,7,5)` (hole +y edge) and `(10,5,7)` (hole +z edge); the
   hole-boundary fragments around the enclosed hole corner `(10,7,7)` dangle
   radial-1. Confirmed against the seed-715 `KEEL_BOOL_DEBUG=1` dump: the
   danglers cluster on `x=9.5518` around one hole corner, exactly two per attempt.
3. **The crossings are already hole VERTICES.** The imprint's Phase-1 pre-split
   turned each tool corner that lands on the hole into a hole vertex, so the chain
   enters/exits the hole AT hole vertices, not edge interiors. (An early
   transversal-crossing detector that required strict edge-interior hits found
   nothing here; the working detector classifies each chain vertex inside/outside
   the hole and takes the boundary crossing on each inside<->outside transition
   segment, which catches a crossing at a hole vertex.)

This is the dossier-71/73b instrument-then-attribute method applied to the
imprint arrangement.

---

## 3. The fix (one localized routine, WRONG-safe)

All in `crates/keel-topo/src/boolean.rs`.

### 3.1 `try_imprint_chain_through_hole` (new)

Dispatched from `imprint_open_chain` BEFORE the boundary-vertex precondition,
only when the face has more than one loop. It:

1. Projects to the face plane (planar carrier only; a curved hole is out of
   scope and falls through).
2. Requires both extreme chain endpoints on the OUTER loop.
3. For each inner loop, detects boundary crossings as inside<->outside
   transitions of the chain (robust to a crossing landing on a hole vertex).
   Requires exactly ONE inner loop crossed at exactly TWO points P1, P2.
4. Requires the chain run BETWEEN the crossings to lie inside the hole
   (midpoint point-in-polygon), and the crossings to be on the FIRST and LAST
   chain segments (so every interior chain vertex is inside the hole and the two
   material sub-chains are single segments). This is the tractable trigger.
5. Re-knits: split the hole loop at P1, P2 (usually a no-op, they are already
   vertices); pre-split the outer loop at the chain ends; `mekr(outer-end, P1)`
   to bridge the hole into the outer loop; `split_face(P2, outer-end)` for the
   second cut. Attaches the seam geometry to the two new edges.

Returns `Ok(None)` (fall through to the unchanged decline) for any shape it does
not recognize. It never moves geometry (P1, P2 lie on the existing hole edges
within tol); a missing topological precondition declines rather than corrupts.

### 3.2 Two small loop-scoped helpers

`loop_fin_on_loop_ending_at` and `loop_edge_containing`: loop-scoped companions
to the existing face-scoped `loop_fin_ending_at_point` / `edge_on_face_containing`
(which only ever scan a face's FIRST/outer loop). The re-knit needs to query and
split the INNER hole loop specifically.

### 3.3 Effect on the repro battery

- Tool `[7,14]^... [5,12]^2` corner-clip: was `unmatched coedge`; now assembles
  WATERTIGHT, mass == mesh == 989 EXACTLY (1080 minus base-slab 75 minus boss-cut
  16). All-planar, no chordal slack.
- Tool `[7,14] y[4,13] z[4,6]` U-through-hole: now assembles, mass == mesh ==
  1020 EXACTLY.
- Tool `[7,14] y[4,6] z[2,8]` span-z (a strip ENTIRELY within the hole's y-band,
  4 hole crossings): correctly DECLINES (not the two-crossing structure).
- The dossier-73b control (no hole crossing) still assembles at 920.

---

## 4. Validation: zero regression, WRONG = 0

Apples-to-apples, SAME base, the per-attempt fault census is deterministic, so the
census deltas are real signal (PASS fluctuates +/-2 on STALL timing, as in 73b).

| metric | baseline (416a421) | with fix | delta |
|---|---:|---:|---:|
| WRONG (decline-never-wrong) | 0 | 0 | 0 |
| PASS (3000 projects) | 2359 | 2358 | -1 (STALL-timing noise) |
| failing attempts: unmatched-coedge | 5161 | 5146 | **-15** (seam-crosses-hole conversions) |
| failing attempts: UnassemblableSeam | 7383 | 7432 | +49 (now-completing imprints reach a later seam stage) |
| failing attempts: degenerate(mass!=mesh) | 19230 | 19231 | +1 (noise) |
| failing attempts: other AssemblyFailed | 266 | 266 | 0 |

The deterministic `unmatched-coedge` drop (-15) is the isolated seam-crosses-hole
slice converting. The `UnassemblableSeam` rise is the expected downstream shift
the 73b validation also saw: an imprint that now completes lets a chain progress
to ops that previously could not even seam, where some hit a different (curved
seam-junction) frontier. WRONG = 0 is the decisive signal: the conservative
trigger never ships a wrong or non-watertight body.

Suite: `cargo test --workspace` 293 passed / 0 failed in keel-topo lib (291
baseline + 2 new regression tests) and 0 failed across all other crates.
`cargo fmt --all --check` exit 0. `cargo clippy --workspace --all-targets -- -D
warnings -A clippy::while_let_loop -A clippy::doc_lazy_continuation` exit 0.

Regression tests (in `boolean.rs`):
- `seam_crosses_hole_assembles_at_exact_oracle`: the corner-clip AND the
  U-through-hole tools on the box+boss body; asserts WATERTIGHT results at the
  EXACT all-planar oracles (989, 1020) with mass == mesh (no slack).
- `seam_crosses_hole_battery_is_decline_never_wrong`: a battery of 10 tools that
  variously cross, graze, or miss the hole; asserts EVERY result is a clean
  decline OR a watertight body with mass == mesh EXACTLY, never a wrong-positive.
  The sacred-floor guard for the class.

---

## 5. The boundary: why seed 715's own op still declines (the compound case)

Seed 11400715918834826715 itself still declines its specific failing
Difference/Intersection. Its chain on the +x face is a U:
`[(9.55, 11.67, 8.97), (9.55, -3.48, 8.97), (9.55, -3.48, -9.17), (9.55, 11.67, -9.17)]`.
Its interior vertices `(9.55, -3.48, *)` lie at the boss-attach x-line, OUTSIDE
the hole: they are MATERIAL vertices where the tool wall also T-junctions the
sibling boss-top and boss-bottom seams on the SAME face. So the chain is not a
clean "dip into the hole and back"; it is a hole crossing COMPOSED with a
sibling-seam T-junction on the compound face. The first/last-segment trigger in
3.1 correctly EXCLUDES it (the crossings are on interior segments, not the first
and last), so the routine falls through and the op declines exactly as before
(verified: no mangling, the dangler count returns to the clean baseline, not the
inflated set a partial re-knit produced in an earlier permissive draft).

This is the honest tractability boundary. Resolving seed 715 needs the
arrangement to handle a chain that crosses a hole AND T-junctions sibling seam
components on the same face simultaneously: a full per-face planar OVERLAY of
{outer loop, hole loop(s), all seam components}, re-extracting faces from the
arrangement, rather than the two-Euler-op special case landed here. That is a
larger imprint advance and the next rung; the all-planar exact oracle still
applies, so it has a clean test target.

A permissive draft of 3.1 (firing on any two-crossing chain, including the
compound U) was measured and produced WRONG = 0 over the soak, but it MANGLED
seed 715 (4 danglers to 12+) by re-knitting a structure it could not complete.
The conservative first/last-segment trigger is the correct, WRONG-safe scope: it
converts the genuinely isolated cases and declines the compound ones, never
attempting a partial arrangement.

---

## 6. Reproduction

```
cargo build --release --example realsoak

# Seed 715 still declines its compound op (hole + sibling T-junction), cleanly:
KEEL_REPRO=11400715918834826715 KEEL_BOOL_DEBUG=1 ./target/release/examples/realsoak.exe

# Population: WRONG = 0, unmatched-coedge 5146 (vs 5161 baseline):
KEEL_FAULT_CENSUS=1 ./target/release/examples/realsoak.exe 3000 1 out

# The landed slice (regression tests):
cargo test -p keel-topo --lib seam_crosses_hole
```

The minimal repro of the LANDED case: box `[0,10]^3` + boss `x[10,15] y[3,7]
z[3,7]` (Union), then Difference by `block((7,5,5),(7,7,7))` (corner-clip) or
`block((7,4,4),(7,9,2))` (U through the hole). Both assemble at the exact
all-planar oracle.

---

## 7. Related dossiers

- **73b** -- the T-junction imprint repair that named this residual (its section
  5); this doc lands the isolated slice of it. The compound case (section 5 here)
  is the union of 73b's T-junction mechanism with the hole crossing.
- **73** -- the attribution that isolated the 16% `unmatched-coedge` slice and
  named seed 715.
- **47** -- the import-and-glue / shell-closure invariant, confirmed CORRECT
  again: it fired on a genuinely incomplete face set (the wholesale-routed hole),
  exactly as its "carry identity, assert closure, never silently drop" doctrine
  intends. The bug was upstream in the imprint arrangement, and the assert
  surfaced it instead of shipping a dropped face.
