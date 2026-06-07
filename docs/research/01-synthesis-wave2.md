# Research Synthesis, Wave 2: Kernel-Layer and NURBS Deep Dives

**Date:** 2026-06-07
**Inputs:** `docs/research/kernel/` (10 dossiers + index, ~250 sources) and `docs/research/nurbs/` (6 dossiers + index, ~200 sources). This document records what the deep dives CHANGE relative to wave 1 (`00-synthesis.md`) and the spec, what they confirm, and what they add. Binding deltas are applied to the architecture spec.

## Verdict changes (spec decisions overturned)

### V1. Non-manifold, cellular, region-first topology from day one (was: manifold-first)

Wave 1 recommended a manifold-first coedge core with a radial cycle generalizable later. The kernel/02 dossier reverses this with unusually consistent evidence:

- The retrofit is the documented worst option. OCCT chose manifold-with-escape-hatches (compounds) and the consequences are in its own docs: the boolean engine refuses SHELL-SOLID fuse, demands homogeneous compounds, cannot guarantee booleans on open solids, and has no space-partitioning region concept. Retrofitting non-manifoldness means rewriting the data structure, the operator set, and the boolean classifier, the three hardest-tested parts of a kernel.
- Parasolid's decisive design choice is that **regions are first-class and partition all of 3D space** (every body has an infinite void region; manifold bodies are restricted cases of the general body). ACIS bolts cellular topology on as a secondary attribute layer and pays a consistency-maintenance burden, the cautionary middle ground.
- The commercial demand is concrete: CAE midsurface-in-solid idealization and CFD/CHT zone modeling are inherently non-manifold and require shared interface faces between regions. A manifold-only kernel cannot host these models at all.
- The recommended structure is **PES-class** (Lee & Lee partial entity structure): radial-edge expressiveness at roughly half the storage, with a half-edge lineage that keeps the manifold common case fast and materializes partial entities only at genuine non-manifold junctions.
- Costs accepted knowingly: the tidy single-scalar Euler-Poincare invariant does not survive; validity moves to a proven operator algebra (Masuda complex-based operators; Lee & Lee generalized operators for PES) plus structural checks, with the LAR chain-complex identity (boundary-of-boundary equals zero) as a cheap rigorous validation oracle.

**Spec change:** D3 rewritten. Hierarchy gains first-class `Region`; data structure is PES-class non-manifold from day one; manifold semantics remain the first implementation target but the representation never assumes them.

### V2. Lineage, determinism, and sessions are day-one structural mandates (was: unaddressed/deferred)

Kernel/07 establishes that persistent naming cannot be bolted on: every scheme from Kripac (1995) to Cascaval's PLDI 2023 lineage DSL reduces to the same kernel hook, and FreeCAD's decade-long topological-naming saga is the cost of retrofitting it onto a kernel (OCCT) that at least already had Modified/Generated/Deleted reporting. The non-negotiables, all structural:

1. Every operation emits **per-output-entity lineage records**: creating operation id, operand entity ids, derivation kind {created, modified, generated, split-child, merge-result, deleted}. No operation may produce anonymous topology.
2. **First-class split/merge events** with ordinals linking children to ancestors.
3. **Object-space anchor points** per entity as disambiguators (more robust than (u,v) anchors under reparameterization).
4. **Deterministic re-evaluation** as an API guarantee: stable iteration order, deterministic id assignment, deterministic tolerance decisions (OnShape's hard rule; without it no naming or query scheme is sound).
5. **Copy-on-write persistent topology** as the substrate: yields undo/redo, named checkpoints (Parasolid pmarks/deltas, ACIS bulletin boards are both approximations of it), automatic transactional atomicity (a failed boolean drops its work-in-progress version, the Parasolid known-state invariant for free), and cheap journaling. Idiomatic in Rust via Arc + structural sharing; encodes Parasolid's partition-locking concurrency rules in the type system (`&mut Partition` exclusive, `&Partition` shared).
6. **Call journaling (record/replay)** from day one: the single highest-leverage debugging and regression tool, dependent on mandate 4.

**Spec change:** new D9 added with these mandates; the pre-M3 gate now covers them.

## Major additions (new binding decisions)

### A1. Homogeneous 4D representation policy (M2-binding)

Both nurbs collections converge on the single most leverage-giving NURBS decision: **store control points in homogeneous 4D (wx, wy, wz, w), run every algorithm (de Boor, knot insertion, Oslo refinement, degree elevation, splitting, derivatives) as the NON-rational algorithm on 4D points, divide only at the API boundary.** Weights strictly positive, canonicalized. Basis conditioning degrades with degree (kappa < p*2^p), not knot non-uniformity: cap working degree, prefer per-span Bezier extraction (Borden et al. operators, precomputable per span) for delicate numerics. Power basis is banned outright (Farouki-Rajan). The corner-cutting schemes are backward stable with error linear in degree (Mainar-Pena); a compensated de Casteljau (FMA error-free transformations, K-fold precision) is the targeted escalation tool for near-tangent inner loops.

### A2. Representation policy for constructed geometry: the approximation is a cache, not the truth

Nurbs/05: offsets of freeform geometry are provably non-rational (the unit-normal square root); fillets are trimmed canal surfaces; sweeps are procedures. Policy: store constructed surfaces in their most exact natural form (procedural: spine + radius law + supports + trimlines for fillets; trajectory + profile + RMF frame rule for sweeps; exact rational offsets for analytic/PN/canal faces), and bake tolerance-controlled NURBS only at consumption boundaries, always carrying {procedural source, achieved deviation, requested tolerance} so geometry can be re-baked finer. Exact escape hatches: Pythagorean-hodograph curves (polynomial speed, exact rational offsets, exact arc length), canal surfaces (Peternell-Pottmann rational parameterization), PN surfaces. Rotation-minimizing frames via Wang et al. double reflection.

### A3. The kernel's missing 2D subsystem

Kernel/04 draws the in/above-kernel boundary cleanly: **geometric constraint solving lives ABOVE the kernel** (the Siemens D-Cubed-beside-Parasolid architecture; decompose-then-solve, witness-configuration diagnosis), but **2D profile computational geometry lives INSIDE**: one arrangement engine (CGAL-style traits-parameterized sweep over a DCEL) from which region extraction (Jiang-Bunke), curved planar booleans, and offset cleanup (Chen-McMains winding-number trimming) all derive. Exact line+arc offsetting, nonzero-winding convention aligned with B-rep loop orientation. This engine also serves HLR, section hatching, and sketch-region extraction (kernel/06). Roadmap: lands with M4 (extrude/revolve consume profiles).

### A4. Tessellation contract (M3-binding)

Kernel/05, unanimous across Parasolid/ACIS/OCCT: **watertightness is a by-construction property: discretize every model edge once; both adjacent faces consume the identical polyline.** Output is an attributed indexed mesh (per-triangle face id, per-boundary-vertex edge id + curve parameter, optional fin adjacency, persistent ids for incremental re-faceting). Tolerance criteria: chord sag (L <= sqrt(8 R delta)), normal deviation, max edge length, MinSize floor. Metric-aware CDT (first fundamental form) for quality mode; pole/seam/degenerate-edge special cases are mandatory robustness engineering. Deterministic, per-face parallel.

### A5. Universal inside/outside oracle: generalized winding numbers

Kernel/09 + nurbs/06: the generalized winding number (Jacobson et al. 2013, Barnes-Hut accelerated per Barill et al. 2018) is the representation-agnostic in/out oracle that degrades gracefully on leaky geometry, and Spainhour & Weiss (TOG 2025) extend it to **robust containment queries over trimmed-NURBS collections, with a BSD implementation in LLNL Axom**. This hardens the wave-1 winding-number boolean classification decision and gives M6 a reference implementation for curved B-rep PMC.

### A6. Convergent-modeling headroom

Kernel/09: Parasolid's Convergent Modeling lesson is architectural: facet/mesh faces as a first-class face type in the SAME topology so booleans/blends/offsets dispatch on surface kind. Keel's surface enum must leave room for a `FacetSurface` variant; mesh booleans get wrapped (Cherchi/Attene line) before being reimplemented (EMBER's plane-based representation is the architecturally sympathetic long-term native model). Lattices/TPMS stay implicit (OpenVDB interchange); neural implicits are import-only.

### A7. Shared-engine consolidation map

The wave-2 literature repeatedly collapses feature families onto a few engines. Recorded as the build-once list:

1. **Multivariate Bernstein subdivision solver** (already D6): now additionally backs silhouettes, isoclines/draft, isophotes, curvature extrema, umbilics, collinear-normal/loop seeds, self-intersection (with diagonal exclusion). One solver, a dozen interrogation features (kernel/06, nurbs/04).
2. **2D arrangement + winding engine** (A3): profiles, planar booleans, offset cleanup, HLR visibility, hatching.
3. **Envelope/distance-field core** (kernel/10): the grazing condition v.n = 0 unifies swept volumes, Minkowski sums, offsets, and (dually) the medial axis. One pruned MAT (lambda-medial-axis pruning with the weak-feature-size guarantee, Q-MAT compaction) feeds midsurface, wall thickness, shelling validity (r >= t), mesh sizing, and defeaturing candidates.
4. **Offset + self-intersection-trim core** (nurbs/05): 2D offset, surface offset, shelling, fillet spines.
5. **Tweak/reintersect core** (kernel/03): Parasolid and ACIS both state tweak (change-face-surfaces + extend-and-reintersect the 1-ring) is THE foundation local op; offset/taper/move/delete-heal/blend-removal are clients. OCCT lacks it entirely: the clearest open-source differentiator. Tiered extension: analytic exact first, minimal bounded de Boor extension, conditioning-aware trimmed extension, mesh-boolean fallback. Delete-face is a dedicated primitive (cannot be decomposed, Zou et al. 2023). Blend removal exploits blend structure (Venkataraman-Sohoni chain classification), never blind face deletion.
6. **Cotangent-Laplacian linear-algebra module**: heat-method geodesics, ARAP flattening, fairing.

### A8. Engineering doctrine deltas (kernel/08)

- **FMA hazard, immediate:** compiler fusion of a*b+c can break predicate antisymmetry. Rust does not contract by default; policy recorded: no fast-math-style flags ever, `mul_add` only where intended, predicates isolated; consider FLiT-style cross-platform CI checks.
- **Differential testing vs OCCT** over the ABC corpus (1M STEP B-reps) as the approximate oracle, with a tiered shape comparator (mass properties fast-reject, then symmetric sampled Hausdorff). Metamorphic relations (transform invariance, scale laws, boolean round-trips, idempotence) as proptest properties.
- **Importers are an adversarial-input boundary**: validate every reference, bound every allocation, fuzz continuously.
- **Partial-success error model** for booleans: result + fault tags per failed face pair, severity-classified, never all-or-nothing.
- Tiered BVH (assembly over body over face) shared by selection/booleans/clash; refit locally, rebuild on topology change, lazy build.

## Confirmations (wave-1 decisions that survived contact with the deep dives)

- Arena/handle storage, geometry/topology split, Euler-operator-mediated mutation (kernel/01 calls the hierarchy "a solved consensus"; fin = coedge = half-edge = edge-use).
- EPICK predicate discipline, Bernstein-basis-everywhere, interval certification on correctness-critical paths (nurbs/04 section 9 is effectively our D2 restated).
- Three-tier SSI (Hohmeyer separability gate -> certified topology for residual pairs (Yang-Jia-Yan / IATA / Krishnan-Manocha) -> fast marching generically, validated interval-ODE (Mukundan 2004) or 2025 monotone-segment tracing near criticality -> Hausdorff-bounded NURBS output). Nurbs/04 section 10 is the M5 implementation spec.
- Mesh-boolean renaissance transfer limits, coincidence-first, corpus-as-product (fTetWild pass-rate methodology), regions where Requicha r-set/regularized-boolean theory grounds PMC (Tilove SMC).
- Refinements: cubic hybrid clipping (Lou-Liu 2022, order-4) joins the root-engine portfolio above plain Bezier clipping; Hu-Wallner second-order geometric iteration replaces bare Newton as the projection polisher; Selimovic + circular/torus clipping as the global projection pruner.

## Reading-list updates per milestone

- **Before M2:** The NURBS Book algorithm taxonomy (A2.x/A3.x/A4.x/A5.x) as the implementation map; Farouki-Rajan/Goodman conditioning; Lyche-Morken knot removal; Borden Bezier extraction.
- **Before M3 (with the paper-design gate, now expanded):** Weiler thesis; Lee & Lee PES papers; Masuda operators; kernel/07 in full (lineage + sessions); kernel/05 faceter contract; Mantyla.
- **Before M4:** kernel/04 Part 2 (arrangement engine), nurbs/04 sections 1-2, 5-6.
- **Before M5:** nurbs/04 section 10 (the assembled SSI architecture), Mukundan validated marching, Hohmeyer TR.
- **Before M6:** Spainhour-Weiss Axom winding numbers; Tilove; kernel/09 booleans.
- **Long tail:** kernel/03 (tweak/local ops, the OCCT-gap differentiator), kernel/10 (MAT/sweep stack), kernel/06 (interrogation/HLR), nurbs/03 (fitting tiers), nurbs/05 (fillet engine).
