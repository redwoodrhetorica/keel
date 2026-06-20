# Deep-chain stack-overflow (recursion) hardening — FINDINGS

**Date:** 2026-06-20
**Branch:** swap-m39-close (worktree)
**Contract upheld:** DECLINE-never-WRONG. A stack overflow is a non-`catch_unwind`-able
process ABORT, which is strictly worse than a decline — it bypasses the entire
oracle/harness safety net. The kernel must never abort on deep input; it must
complete or return `Err`.

## Symptom (reported evidence)

The faithful realistic-workflow soak (`examples/realsoak.rs`) found that some deep
op-chains on a "heavy grown body" overflowed even a **256 MB** worker stack and
aborted the process. The harness worked around it by raising the *worker* stack
(256 MB hardcoded) — a band-aid. The brief: the KERNEL itself must not abort; find
and bound the unbounded recursion in the boolean / tessellation paths.

## Investigation

### 1. Static survey of recursion in the hot paths

Read every recursive-capable function in the boolean (`boolean.rs`, `imprint.rs`),
tessellation (`tessellate.rs`), validation (`validate.rs`), mass (`massprops.rs`,
read-only/locked) and supporting geometry (`ssi.rs`, `bernstein.rs`, `curve.rs`)
paths. Findings:

- **Tessellation** (`earclip_2d`, `earclip_2d_eps`, `earclip_3d`, `clip_half_space`,
  `subtract_cone`) — fully ITERATIVE, with `guard > n*n+16` loop caps. Not recursive.
- **Validation** (`check_*`) — iterative loops over arenas.
- **Mass properties** (`integrate_planar_face` / `integrate_curved_face` /
  `integrate_face_green`) — per-face, single delegation, no recursion. (Locked; not
  touched.)
- **SSI marchers** (`walk`, `walk_ssi`, `march_branch`, `trace_implicit_2d`,
  `spline_spline`, `intersect_surfaces`) — bounded `for _ in 0..N` continuation
  loops; `roots_rec` (Bernstein) recurses but has a `t1-t0 <= t_tol` floor →
  depth `≈ log2(1/tol) ≈ 30`. Bounded-shallow.
- **Seam assembly** (`assemble_open_chain`, `assemble_closed_loop`,
  `seam_components`, `uf_find`/`uf_union` in `finalize_cellular`,
  `try_imprint_chain_arrangement`, `imprint_operand`) — all iterative; the
  `seam_components` and cellular union-finds are already iterative two-pass.

Two genuine UNBOUNDED recursions remained:

1. **`connected_face_components::find`** (`boolean.rs`, was ~line 4044) — a
   RECURSIVE path-compressing union-find. It is on the boolean ASSEMBLY hot path
   (called from `finalize_imported_sheet` and `finalize_imported_assembly`). Its
   recursion depth equals the union-tree height; because the union step was
   `parent[a] = b` (arbitrary, NOT by rank), a pathological component (a long union
   chain) makes the tree degenerate to height O(n), so the first `find` recurses
   O(component-size) deep — unbounded in body complexity.
2. **`Assembly::walk`** (`assembly.rs`) — recursive DFS over the assembly DAG;
   depth = subassembly NESTING depth (unbounded for a deeply nested DAG). Not on
   the realsoak path, but a real abort vector for the assembly API.

### 2. Empirical isolation (`examples/probe_deepstack.rs`)

Built a direct repro that grows a heavy body by a long chain of clean boolean
features, then runs each per-op kernel routine inside a deliberately tiny worker
stack.

- A body grown to **806 faces / 1508 vertices / genus 100** runs `clone`, `validate`,
  `mass_properties`, `mesh_volume`, `boolean`, and `topology_hash` to completion on a
  **256 KiB** stack. A linear-in-faces recursion would have overflowed 256 KiB long
  before 806 faces.

**Conclusion:** there is no body-complexity-driven recursion in those paths that
overflows a normal stack for a *clean* heavy body. The recursive union-find stays
shallow in practice because a real grown body's faces share edges in a bushy
(not chain) pattern, and path compression flattens the tree after the first walk.

### 3. Direct soak reproduction attempt (`KEEL_CRASH_HUNT`)

Added a crash-hunt mode to `realsoak.rs` that runs projects SEQUENTIALLY on a
small (1 MB) stack, flushing each seed to a log BEFORE running it, so a true
overflow (which aborts) leaves the crashing seed as the last log line. Also made
the worker stack env-configurable (`KEEL_WORKER_STACK_MB`).

Ran 8 parallel hunts (faithful + fuzzer modes, distinct seed ranges), ~1,600+
projects, generated chains up to **150 ops**, realized chains up to ~94 ops — all
survived a **1 MB** stack with **zero** overflows and zero caught panics.

**Conclusion:** the reported 256 MB overflow could NOT be reproduced via a direct
API path in the budget. It is a RARE, degenerate-configuration-specific runaway
(one pathological project in many thousands), not a generic deep-chain recursion —
consistent with most projects stalling early (median realized depth was low) so
genuinely huge bodies are rare in the sampled grammar.

## Fix (WRONG-safe; iterative, byte-identical)

Both genuine unbounded recursions converted to ITERATIVE form (no behavior change
for any valid input — verified by the full suite):

1. **`connected_face_components::find` → iterative two-pass union-find**
   (find-root loop, then path-compress loop). Same root, same full path
   compression, O(1) native stack regardless of component size. This is the only
   unbounded recursion proven to be on the boolean assembly hot path.

2. **`Assembly::flatten` / `walk` → explicit-stack DFS (`walk_iter`)**. Children
   pushed in reverse so they pop in forward order, reproducing the recursive
   pre-order EXACTLY; each stack frame carries its instance-id path + composed
   world transform, so occurrence identity and world placement are unchanged.
   O(1) native stack at any DAG nesting depth.

Why iterative and not a depth-guard: both are trivially convertible, and the
genuinely-unbounded case (a long union chain / a deep DAG) should COMPLETE
correctly, not decline. The brief prefers iterative for the genuinely-unbounded
case; a depth guard is the fallback when iterative is intractable, which it was
not here.

A depth-guard precedent already exists in the codebase (`step_import.rs`
`MAX_DEPTH = 64`) for the recursive STEP parser; that remains the pattern if a
future degenerate-specific runaway is pinned to a genuinely hard-to-iterate site.

## Depth limit

N/A — both fixes are iterative (no recursion remains), so there is no depth limit
to set. Native stack usage is now O(1) in body complexity and DAG depth for these
routines.

## Verification

- `cargo build --release` — clean.
- New regression `tests/deep_chain_no_abort.rs`: builds a heavy body (≥80 faces,
  high genus) and runs `boolean` + `validate` + `mass_properties` + `mesh_volume`
  on a **256 KiB** stack inside a worker thread; asserts the thread JOINS (a stack
  overflow would abort the process and fail the test by termination). PASS.
- Full suite `cargo test --release`: **351 passed, 0 failed, 10 ignored** (the
  ignored are the heavy oracle/scan shards the orchestrator runs). WRONG-locks all
  green: `scan_wrong`, `union_wrong_repro`, `cyl_union_mass_witness`, `three_bucket`,
  `post_fillet_mass`, `fillet_surgery_robustness`. `tutorial_workflows`: 10/10.
- `cargo clippy --release --tests --examples`: no NEW warnings from the changed
  files (pre-existing warnings elsewhere untouched).

## Residual risk / follow-up

The exact rare 256 MB runaway was not reproduced, so a depth-specific runaway on
some degenerate geometry could in principle still exist on a path I could not
trigger. Mitigations in place:

- The two genuine unbounded recursions (the only ones found on/near the boolean
  path) are eliminated.
- `realsoak.rs` keeps a generous default worker stack (256 MB) as belt-and-
  suspenders, now overridable via `KEEL_WORKER_STACK_MB` so the orchestrator can
  dial it down to re-verify the kernel no longer aborts, and `KEEL_CRASH_HUNT` is
  available to pin any future overflow to a precise seed.

If the consolidated soak ever does pin a crashing seed, `KEEL_REPRO=<seed>` replays
it and `KEEL_CRASH_HUNT` localizes it; the established `MAX_DEPTH` guard pattern is
then the WRONG-safe fallback for that specific site.
