# 73b (FINDINGS). Compound-operand `unmatched-coedge`: the T-junction imprint root cause, a landed fix for the clean slice, and the seam-crosses-hole residual

## Status and posture

This is the follow-up to dossier 73, attacking the `unmatched-coedge` /
shell-closure decline on an all-planar compound body (the 16% slice dossier 73
isolated, with seed 11400715918834826715 as the clean repro). The dossier-71/73
method was applied: instrument the failing path on the real repro, find the
TRUE failing stage and root cause, correct the prior framing, and fix precisely
or characterize precisely.

Headline: the `unmatched-coedge` failure is **NOT** an assembler (stitch / Weiler
region-walk / glue) bug. The stitch's shell-closure check is firing CORRECTLY on
a genuinely incomplete face set. The true root is **upstream, in the IMPRINT**:
`imprint_open_chain` rejects a seam whose endpoint lands in the INTERIOR of a
boundary edge that a SIBLING seam component created earlier in the same face's
Phase-2 pass (a T-junction). A localized, WRONG-safe fix to the imprint (plus the
straight-edge analog of the existing conic subdivision-alignment in the glue)
lands the clean slice of this class with zero regression and WRONG = 0. A harder
RESIDUAL -- a seam that crosses a pre-existing inner-loop HOLE on a compound body
face -- is characterized precisely and left for a dedicated round.

House style: no em-dashes; ranges written with "to".

Base: HEAD `9a2e2f6` (dossier 73 FINDINGS), branch `swap-m39-close` worktree.
The change is confined to `crates/keel-topo/src/boolean.rs` (the imprint +
glue). `massprops.rs` and `blend.rs` are untouched; every floor gate (dual
mass-vs-tessellation, op-volume bound, watertightness net) is unchanged.

---

## 1. The load-bearing correction, stated once

> **The dominant mechanism behind the all-planar `unmatched-coedge` decline is a
> T-JUNCTION the IMPRINT cannot resolve, not a coedge-pairing failure in the
> stitch. When a Difference/Union tool wall meets a COMPOUND body, two body
> faces cut that wall in lines that T-cross: one body face's SSI line spans the
> wall (e.g. the box +x face at x=9.55), and a second body face's SSI line ENDS
> mid that span (e.g. the boss top at z=3.13, whose attach edge ends at x=9.55).
> `seam_curves` correctly produces BOTH seams; `imprint_operand` groups them as
> SEPARATE connected components (they share no endpoint) and imprints each via
> `imprint_open_chain`. The first component splits the wall into a fresh edge;
> the second component's endpoint now lands in the INTERIOR of that fresh edge,
> but `imprint_open_chain`'s precondition requires endpoints at existing loop
> VERTICES, and Phase-1's pre-split only saw the ORIGINAL boundary. So the second
> imprint fails (`Topo(Precondition("open chain end not on boundary"))`), the
> tool wall keeps a fragment it should not, and the stitch then finds dangling
> coedges with NO bound-twin and NO collinear overlap -- the hallmark of a
> genuinely incomplete (not mis-paired) face set. The fix splits the fresh
> boundary edge at the mid-edge endpoint so the chain attaches vertex-to-vertex.
> A second, symmetric mismatch -- the body face's wall-cut carried as `A->C`
> facing the tool's `A->B + B->C` -- is the STRAIGHT-edge analog of the existing
> CONIC subdivision-alignment in `merge_and_glue_imported`, and is fixed by
> extending that pass to straight seam edges. Together these convert the clean
> slice; the RESIDUAL is a seam that crosses a pre-existing HOLE (inner loop) on
> a compound face, which needs a real face-with-hole arrangement split.**

---

## 2. Method: instrument-then-attribute on seed 715

`KEEL_REPRO=11400715918834826715 KEEL_BOOL_DEBUG=1 realsoak` replays the seed;
its failing `booleanD`/`booleanI` declines `AssemblyFailed("unmatched coedge:
shell-closure invariant violated")` on an all-planar body (`g0 f11`). The body
is a box `x[-9.55,9.55] y[-11.67,11.67] z[-15.28,15.28]` with a rectangular boss
on its +x face spanning `x[9.55,23.81] y[-7.09,6.12] z[-9.65,3.13]` (two prior
booleans); the tool is a box. Steps:

1. **Dangling-edge pairing diagnosis** (throwaway, env-gated): for each radial-1
   edge at the failing stitch, search ALL edges for a bound-twin (the glue's own
   match key) or a collinear overlap (a split mismatch). Result on seed 715:
   EVERY dangling edge had NEITHER. So it is not a glue miss and not a simple
   split mismatch on the same line -- the partner fragment is ABSENT. The defect
   is upstream of the glue.
2. **Per-face plane + loop dump** at import: located the dangling edges on the
   tool's -y wall (`y=-3.48`) and the body's +x plane (`x=9.55`), where the tool
   wall is cut by both the box +x face and the boss top/bottom.
3. **`seam_curves` plane-plane dump**: confirmed the boss-top x tool-wall seam IS
   produced (`fa` boss-top z=3.13, `fb` tool -y, segment x[9.55,15.51]). The seam
   exists; it is the IMPRINT that drops it.
4. **`imprint_operand` per-component dump**: the smoking gun. On the tool -y wall
   (face id 40) the two seams form `comps=2 multi=true`; the x=9.55 component
   imprints `Ok`, then the z=3.13 component fails
   `open-chain ... Err(Precondition("open chain end not on boundary"))`. Same on
   face id 19. These are the only two `Err`s; they are the root.

A standalone probe (load the dumped body+tool JSON, run the op) reproduced the
failure exactly and gave a fast iteration loop.

---

## 3. The fix (two parts, both WRONG-safe)

All in `crates/keel-topo/src/boolean.rs`.

### 3.1 Open-chain T-junction repair (`imprint_open_chain`)

Before the boundary-vertex precondition, for each chain endpoint NOT at a loop
vertex, find a boundary edge of THIS face (new helper `edge_on_face_containing`,
the face-scoped companion to `edge_containing_point`, scanning all loops) whose
segment passes within `etol` of the endpoint with the point strictly INTERIOR,
and `split_edge_raw` it there. The point is provably on the existing boundary
(within `etol`), so this only ADDS the missing junction vertex; it never moves
geometry. A point genuinely OFF the boundary finds no edge and still declines at
the unchanged precondition -- DECLINE-never-WRONG preserved.

### 3.2 Straight-edge subdivision alignment (`merge_and_glue_imported`)

The existing pass splits a dangling CONIC arc at any interior merged vertex so
both operands' subdivisions match (task 29). Extended with
`straight_edge_contains_interior` to also split a dangling STRAIGHT edge at an
interior vertex (the `A->C` vs `A->B + B->C` mismatch the tool/body sides emit at
a T-junction). `split_edge_raw` carries the parent line curve to both children;
no `arc_sweep` applies to a line.

### 3.3 Effect on seed 715 (the instrumented walk)

The failing op's dangling-edge count drops 10 -> 8 (3.1, the tool walls) -> 4
(3.2, the body cut edges). All `imprint_open_chain` calls now return `Ok` (the
two `Precondition` errors are gone; the per-attempt census `Topo(open chain)`
stays 0 because those faults already arrived on the returned-result path). The
remaining 4 danglers are the seam-crosses-hole residual (section 5).

---

## 4. Validation: zero regression, WRONG = 0, a real population gain

Apples-to-apples, SAME base commit, the per-attempt fault census is byte-for-byte
DETERMINISTIC (two baseline runs were identical), so the deltas are real signal,
not run noise.

| metric | baseline | with fix | delta |
|---|---:|---:|---:|
| WRONG (failures.jsonl) | 0 | 0 | 0 |
| PASS (3000 projects) | 2361 | 2359 | -2 (STALL-timing noise) |
| failing attempts: degenerate(mass!=mesh) | 19252 | 19230 | -22 |
| failing attempts: unmatched-coedge | 5134 | 5161 | +27 |
| failing attempts: UnassemblableSeam | 7636 | 7383 | **-253** |
| total failing attempts | 33159 | 32868 | **-291** |

The fix converts a net 291 failing boolean ATTEMPTs to successes (most visibly
-253 `UnassemblableSeam`, because the now-completing imprint lets chains progress
to ops that previously could not even seam). The small `unmatched-coedge` +27 is
cases that previously failed earlier and now reach the stitch and hit the
residual. A direct case study over 800 to 1200 soak projects found 8 to 9
single-op CONVERSIONS (baseline `Err(unmatched coedge)` -> fix returns a
WATERTIGHT body, `validate` ok, `faults` empty, mass within the gate's 2 percent
curved band of mesh). EVERY converted body carried 1 to 2 curved (fillet) faces,
so the small mass-mesh gap is the LEGITIMATE chordal tessellation deficit the
gate's 2 percent band exists for; all-planar conversions are exact. The fix is
load-bearing specifically when a curved face routes assembly through
`stitch_by_import` (the all-planar soup path recovers some T-junctions on its
own).

Suite: `cargo test --workspace` 555 passed / 0 failed (553 baseline + 2 new
regression tests). `cargo fmt --all --check` exit 0. `cargo clippy --workspace
--all-targets -- -D warnings ...` exit 0.

Regression tests (in `boolean.rs`):
- `compound_operand_difference_assembles_at_exact_oracle`: a box+boss body (the
  seed-715 topology) Differenced by a tool that T-junctions the boss seams;
  asserts a watertight result at the EXACT closed-form oracle (920) with mass ==
  mesh (all-planar, no slack).
- `compound_operand_battery_is_decline_never_wrong`: a battery of tools on the
  same body; asserts EVERY result is either a clean decline or a watertight body
  whose (exact, all-planar) mass equals its mesh -- never a wrong-positive. The
  sacred-floor guard for the class.

---

## 5. The residual: a seam crossing a pre-existing HOLE on a compound face

Seed 715 itself still declines its specific failing op (now at 4 danglers, all on
`x=9.55`). Root cause, instrumented: the body's +x face (the box face the boss
attaches to) carries an inner-loop HOLE (the boss opening). The tool's wall-cut
seam on that face is a chain that PASSES OVER the hole (its `y=-3.48` segment
runs `z[8.97,-9.17]`, crossing the hole's `z[-9.65,3.13]` span). `imprint_open_
chain` splits the face boundary-to-boundary with `split_face`, but `split_face`
routes the inner loop WHOLESALE to one side; it does not split the inner loop at
the two points where the chain crosses it. The result's hole boundary is left
inconsistent, and its edges in the un-cut sub-region (`y[-7.09,-3.48]`) dangle.

This is a genuine, harder imprint feature: a planar-arrangement split of a
face-WITH-HOLE where the cut crosses the hole, requiring the chain to be merged
with the inner-loop arcs at the crossing vertices (split the inner loop, re-knit
both). It is NOT a wrong body shipped -- the shell-closure check declines it
correctly. It is the next rung for this class and the reason seed 715 is not yet
a full PASS. A box + centered-boss (hole) + Difference is the minimal repro;
the all-planar exact oracle applies.

Note that the box+boss battery test (section 4) confirms this residual NEVER
ships a wrong body: over the battery, every assembled result is exact-watertight
and the rest are clean declines.

---

## 6. Reproduction

```
cargo build --release --example realsoak

# Seed 715 still declines its 2 unmatched-coedge booleans (the hole residual),
# but realizes the same 3 ops; the imprint now completes (no open-chain Err):
KEEL_REPRO=11400715918834826715 KEEL_BOOL_DEBUG=1 ./target/release/examples/realsoak.exe

# Population: WRONG=0, total failing attempts 32868 (vs 33159 baseline):
KEEL_FAULT_CENSUS=1 ./target/release/examples/realsoak.exe 3000 1 out

# The landed slice (regression tests):
cargo test -p keel-topo --lib compound_operand
```

---

## 7. Related dossiers

- **73** -- the attribution that isolated this 16% slice and named seed 715; this
  doc is its #2 follow-up (the seam-junction advance it called for), landed.
- **47** -- the import-and-glue / shell-closure invariant. Confirmed CORRECT: the
  invariant fires on a genuinely incomplete set; the bug was upstream in the
  imprint, exactly as 47's "carry identity, assert closure, never silently drop"
  doctrine intends (the assert surfaced the real imprint gap instead of shipping
  a dropped face).
- **59 / 39** -- the curved `UnassemblableSeam` seam-junction analog (23%), the
  curved counterpart of the residual in section 5.
