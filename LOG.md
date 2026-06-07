# Keel Project Log

A running diary of project state, decisions, and next steps. Newest entry last.
Convention: every working session appends an entry (date, what happened, decisions
made, open questions, exact next step). This file is the authoritative "where are we"
record; the spec holds the architecture, docs/research/ holds the evidence, git holds
the code history. Style rule: no em-dashes anywhere in this repo's prose.

---

## 2026-06-07 (project founding through M1 + wave-2 research)

### What this project is

Keel: an open-source B-rep solid modeling kernel in Rust, Parasolid-class ambition.
Purpose: OSS alternative (the gap is precise: OCCT is the only full open kernel and
its booleans/local-ops are the weak points; everything else is geometry libraries
without the solid-modeling tier). Possibly production if quality permits.
Proof milestone (M7): robust booleans on NURBS-bounded solids surviving mass
randomized testing. The developer is Claude (AI, working across sessions); the user
directs strategy and supplies research but does not code or micro-manage.

### Founding decisions (debated, settled)

- **Rust** (stable, no nightly), chosen over C/C++/C# after extended debate. Deciding
  argument: the developer is an AI; the compiler is the only memory that never resets
  between sessions, memory-corruption-at-a-distance is the worst bug class for a
  run/read/edit debug loop, and cargo's test economics enable the generated-test
  volume that substitutes for decades of industrial regression data.
- Working name "Keel" (ship's backbone). Crate name availability check deferred to
  first publish. "Parasolid" is a Siemens trademark, benchmark reference only.
- Architecture spec: docs/superpowers/specs/2026-06-07-keel-kernel-architecture-design.md
  (decisions D1-D9, roadmap M1-M7). M1 plan: docs/superpowers/plans/2026-06-07-m1-foundations.md.

### Research program (two waves so far, user keeps feeding more)

- Wave 1: six agent-researched tracks in docs/research/ (a-f: robust numerics, NURBS,
  SSI, booleans/tolerant modeling, kernel architecture, ML-for-geometry), synthesized
  in 00-synthesis.md. Produced the binding deltas: EPICK predicate cascade, per-entity
  tolerances primary (no global epsilon), one Bernstein subdivision solver behind every
  query, topology-first SSI, winding-number boolean classification, ML strictly
  offline/propose-then-certify, corpus-as-product.
- Wave 2 (user-supplied, ~450 sources): docs/research/kernel/ (10 dossiers) and
  docs/research/nurbs/ (6 dossiers), synthesized in 01-synthesis-wave2.md. TWO VERDICT
  CHANGES applied to the spec:
  1. D3 rewritten: non-manifold PES-class topology with FIRST-CLASS REGIONS from day
     one (was manifold-first). OCCT is the cautionary proof that the retrofit fails.
  2. New D9: lineage records on every operation, determinism as API guarantee,
     copy-on-write persistent topology, journaling. Cannot be retrofitted (FreeCAD).
  Plus: homogeneous 4D NURBS policy, procedural-exact constructed geometry with
  NURBS-as-cache, in-kernel 2D profile/arrangement subsystem (M4), edge-first
  watertight faceting contract (M3), generalized winding numbers as the universal
  in/out oracle (Spainhour-Weiss/Axom), FacetSurface headroom, tweak/reintersect core
  as the eventual OCCT-gap differentiator, FP-contraction (FMA) policy.

### Code state

- M1 COMPLETE, merged to master. Workspace + crates/keel-math: vec (Vec2/3/4), mat
  (Mat2/3), transform (Rodrigues), bbox, interval (outward-rounded, soundness
  proptested), tolerance (the only epsilon home: linear 1e-8, angular 1e-11),
  predicates (robust crate wrapped behind Sign enum; orient3d flips Shewchuk's
  below-plane convention, unit test is the arbiter), newton (bracketed hybrid; bug
  found and fixed: one-sided convergence returned bracket midpoint on exhaustion, now
  returns iterate), poly (Blinn quadratic, Yuksel-style monotonic-interval cubic),
  bernstein (de Casteljau eval, subdivision, sign-variation root isolation,
  cross-validated against the cubic solver by proptest oracle).
- 56 tests green, clippy -D warnings clean (clippy.toml allows unwrap in tests),
  criterion baselines: orient3d 23ns, cubic 523ns, bernstein eval 89ns, roots 9.7us.
- Fuzz harnesses (solve_cubic, bernstein_roots) compile; EXECUTION PENDING on Linux
  (cargo-fuzz does not run on Windows; do a 10-minute run of each before M2 ends).
- CI workflow written (.github/workflows/ci.yml), inactive until a remote exists.

### Standing rules

1. Re-read everything new in docs/research/ before writing each milestone plan.
2. Pre-M3 gate (expanded): boolean/SSI pipeline, D9 lineage/session hooks, and the
   PES operator set designed on paper BEFORE the topology API freezes.
3. ML is an offline weapon (fuzzing mutators, threshold search, corpus mining);
   nothing learned on the correctness path; runtime models only propose-then-certify.
4. No single global epsilon; per-entity tolerances; all epsilons live in keel-math::tolerance.
5. Never branch on a raw f64 sign for combinatorial decisions; use keel-math::predicates.
6. No fast-math, no implicit FMA in predicate paths; mul_add only where intended.
7. Deterministic everything (iteration order, ids, tolerance decisions); it is an API
   promise, not an implementation detail.
8. Append a LOG.md entry every working session (this rule added today at user request).
9. User communication style: engage deeply with answers, no decision-menu funnels,
   no em-dashes ever.

### Open questions / risks

- Whether the PES non-manifold core meaningfully slows M3 relative to the old
  manifold-first plan (accepted cost; the literature says pay it now).
- Multivariate Bernstein subdivision solver design (PP/IPP + Mourrain-Pavone
  reduction): first consumer is M2 projection/intersection; design lands in M2 plan.
- Fuzz runs need Linux access (or CI remote). Pending.
- Crate name "keel" availability on crates.io unchecked.

### Next step

Write the M2 plan (curves/surfaces: analytics + NURBS on homogeneous 4D points per
The NURBS Book algorithm taxonomy, derivatives, closest-point with Hu-Wallner polish,
multivariate subdivision solver, knot insertion/refinement (Boehm/Oslo), Bezier
extraction), starting with a fresh read of any new research that has landed by then.
