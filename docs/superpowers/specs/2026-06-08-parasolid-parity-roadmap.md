# Keel -> Parasolid Capability Parity Roadmap

**Goal:** close the capabilities/features gap between Keel and Parasolid as far as a single-maintainer, patent-safe, OSS kernel realistically can. Measured against the researcher's 144-item Parasolid capability map (`docs/research/kernel/25-parasolid-capability-map.md`).

## Honest framing of "parity"

Full 144/144 parity is a multi-decade, large-team effort and is not the target. The achievable target is the **kernel-appropriate, patent-safe subset**, sequenced so each engine reuses the certified spine Keel already has. Three groups of items are explicitly OUT of scope and are not counted against parity:

- **Host / D-Cubed concerns the map itself excludes** (item 81 free-form deformation; constraint solving; assembly mating logic; feature/history trees; parametric sketching). The kernel provides the primitives these drive; it does not implement them.
- **Patent-fenced** (items 86-93 convergent/mesh modeling -- Siemens, ~2035 per `18-patent-ip-landscape.md`). Approached only via a clean-room design-around (indirect-predicate mesh arrangements, file 09) or deferred. Not on the critical path.
- **Commercially-unsolved / fragile** (items 134-135 mid-surface): best-effort service with explicit failure reporting, not a guaranteed operation.

Net realistic target: roughly **110-120 of 144** items at "works on the common cases, honest deferral ledger for the rest" -- the same quality bar as M1-M8.

## Current state (baseline, after M8)

Solid (~30 items): full topology hierarchy + Euler ops + non-manifold PES regions (16-23, 136); primitives block/prism/cyl/cone/sphere/torus/nurbs_sphere (1-6); analytic+NURBS curves/surfaces (11-14); unite/subtract/intersect on solids for clean transversal analytic + recovered-NURBS bodies (24-27, narrow); SSI (104); GWN point classification (103); mass properties (100); validity checking (109); canonical recovery / simplify (131); tolerant edges / epsilon-solidity (110-112); imprint curves onto faces (73); in-memory session snapshot/replay + lineage events (partial 122-125).

Missing (~100 items): the entire feature surface -- blends/chamfers, sweep/loft, shell/offset, local ops/tweak, sheet ops, sectioning, HLR/rendering, attributes, persistence (XT), assemblies, foreign geometry, healing/defeaturing, mid-surface, convergent/mesh.

## Sequencing principles

1. **Foundation before features.** Several engines stand on shared machinery (general-position booleans, offset surfaces, surface extension). Build those first or they become per-engine reinvention.
2. **Keystone engines first.** Some single engines unlock many checklist items (tweak -> 6; blend engine -> 15). Prioritize leverage.
3. **Reuse the certified spine.** Prefer engines that compose Keel's existing SSI + classified booleans + imprint + recovery, so each milestone is mostly assembly, not new foundations.
4. **Patent-safe always** (D10 / file 18): no convergent-modeling single-body mesh+brep, no auto-inferred-constraint direct edits, no U-spline refinement.
5. **Same quality bar as M1-M8:** per-milestone research re-read; TDD; fuzz each new mutation path; honest deferral ledger; exact CI triplet before merge; LOG addendum.

---

## Phase 0 -- Foundation hardening (the cross-cutting prerequisites)

These gate almost every feature engine; features produce the exact configurations Keel currently declines.

- **0a. General-position booleans.** Today booleans DECLINE coincident/tangent/touching faces. Feature ops generate these constantly (a blend is tangent to its neighbors; a shell offsets to coincident faces; a swept solid united back is coplanar). Extend the winding-classified pipeline to coincident/tangent neighborhoods (the n-ary winding-number-VECTOR classify already noted in the M6 ledger), sheet-solid and sheet-sheet booleans, multiple tool bodies, imprint-only option. Closes 28-34; unblocks everything downstream.
- **0b. Offset-surface geometry primitive.** Exact offsets of plane/cyl/cone/sphere/torus (closed form) + certified NURBS offset (file: nurbs offsets / Piegl-Tiller). The shared primitive under blends, shell, offset-face, thicken.
- **0c. Attribute system** (117-121). Cheap infrastructure, broadly enabling (color, names, density, feature data, and the substrate persistent naming and feature recognition need). Typed attributes + user fields + propagation-through-ops rules.

## Phase 1 -- The local-operations keystone (tweak) [file 03]

`change-face-surface + reintersect` is, per file 03, THE foundation local operation (Parasolid and ACIS both implement the rest as its clients; OCCT lacks it -- a differentiator). One engine closes: tweak/change-surface (35, 39), move faces (36), offset face (37), taper/draft (38, 78), delete-face-with-heal (40). Reuses SSI (reintersection) + classified booleans + imprint. **~6 items, high reuse.**

## Phase 2 -- Construction breadth (sweep / spin / loft) [file 26]

Body constructors: sweep-by-translation with lateral faces (62), sweep-along-path (63), spin/revolve-solid (64), extrude-general (65), loft/skin through sections with guides+continuity (66-67), surface-from-boundary / n-sided (68), ruled/tabulated (69). Reuses revolve_full, Euler ops, the transfinite/Gordon surfacing in file 26 (central-split for n!=4). **~8 items.**

## Phase 3 -- Blends and chamfers [file 28] (the single biggest area)

One spine + spring-curve + polymorphic-section engine: constant-radius rolling-ball (47), variable-radius (48), conic/G2 sections (49, 60), face-face blends (50), vertex/setback (51), chamfers equal/two-offset/offset-angle (52), apex-range (53), hold-line (54), range-controlled (55), networks+mitring (56), overflow (57), recognition (58), removal/unblend (59). Reuses Phase-0 offset surfaces + SSI + imprint + stitch. **~15 items.** The hardest, highest-payoff chunk; depends on Phase 0b.

## Phase 4 -- Shell / offset family

Hollow/shell to wall thickness (41), pierce-open (42), per-face thickness (43), thicken sheet (44), whole-body offset with self-intersection resolution (45). Reuses Phase-0 offsets + Phase-1 tweak reintersection + booleans. **~5 items.**

## Phase 5 -- Sheet/surface ops + sectioning [file 13]

Extend sheet (70), knit/sew (71), trim (72), section by plane/surface (75), split (76), slice with offset list (77). Surface extension is the shared core of delete-face-heal/tweak/untrim (file 13), so it compounds with Phase 1. **~6 items.**

## Phase 6 -- Interrogation + rendering completion [files 06, 22]

HLR + wireframe (96), exact silhouette/outline (97), draft analysis (107 surface analysis), precise bounding boxes (105), entity/body equivalence (108), min/max distance + clash (101-102), render facets+lines (95), incremental tessellation (98), section views (99). Silhouettes/isoclines are zero-set queries against the existing subdivision solver (file 06). **~8 items.**

## Phase 7 -- Persistence + sessions hardening [file 14]

Deterministic XT-class save/restore with EXACT double round-trip (126), partitions (123), pmarks/rollback already partial -> full (124), transactions/deltas (125, 127), version control (128), journaling (129). File 14: FP determinism + exact serialization is load-bearing for persistent naming. **~5 items.**

## Phase 8 -- Healing / defeaturing [file 13]

Stitch (part of knit, Phase 5) -> simplify (M8, done) -> gap-tighten (130), defeature small blends/holes/bosses (132, needs Phase 3 blend-removal), redundant-entity removal + face merging (133). Reuses M8 recovery + Phase-3 unblend + Phase-5 stitch. **~5 items.**

## Phase 9 -- Extensibility + assemblies [files 16, 24]

Foreign geometry via an evaluator trait (114-116); instances + assembly DAG with per-instance transforms and stable edit-surviving IDs (82-85), geometry in local frames. **~6 items.**

## Deferred / out-of-scope (counted honestly)

- Convergent/mesh (86-93): patent-fenced; clean-room mesh-arrangement design-around (file 09) only if pursued, else explicitly excluded.
- Free-form deformation/emboss/wrap (80-81), constraint solving, assembly mating, feature trees, sketching: host/D-Cubed, not kernel.
- Mid-surface (134-135): fragile best-effort service.
- Helix (15), foreign-geometry full coverage, SMP/threading (140): opportunistic.

## Definition of parity (the finish line for this program)

Each phase = one or more milestones, each merged CI-green with its own fuzz target and honest ledger. The program is "as close to parity as possible" when Phases 0-9 land: the kernel-appropriate, patent-safe subset of the 144 map (~110-120 items) works on common cases with documented deferrals -- a kernel that can actually back a CAD application, with the exact-topology/tolerant-geometry + canonical-recovery differentiation intact throughout.
