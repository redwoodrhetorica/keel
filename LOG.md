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

---

## 2026-06-07 (addendum: M2a planned, PAUSED before execution)

- M2 split into M2a (spline core + curves) and M2b (surfaces + multivariate
  subdivision solver). M2a plan written, self-reviewed, committed:
  docs/superpowers/plans/2026-06-07-m2a-spline-curves.md (9 tasks: scaffold
  keel-geom, knots, basis A2.2, NURBS eval + exact arcs, hodograph derivatives,
  insertion/split/Bezier decomposition, analytic curves + Curve3 enum, global
  closest-point, benches/fuzz/wrap-up).
- Branch `m2a-spline-curves` created from master. NO M2a CODE WRITTEN YET.
  Resume point: execute Task 1 (scaffold keel-geom crate) per the plan.
- WSL Ubuntu-24.04 set up with Rust nightly + cargo-fuzz for fuzz runs (libFuzzer
  needs Linux; native dev stays on Windows stable). First run failed: repo
  rust-toolchain.toml pins stable, overriding WSL nightly; fix is
  `cargo +nightly fuzz run`. Rerun of both M1 targets (10 min each) was running
  in the background at pause time; check results before closing the M1 gate item.
- USER PAUSED the session here.

---

## 2026-06-07 (addendum 2: kernel/11 curved-predicates dossier absorbed, still paused)

- User's research agent flagged docs/research/kernel/11-curved-exact-predicates.md
  for immediate reading. Read in full and absorbed. It completes the D2 doctrine:
  four predicate tiers (linear exact / conics exact via one-root numbers + lazy
  exact + filters / quadrics exact-when-cheap via QI pencil analysis / NURBS
  certified-numeric topology over tolerant metric geometry), with the sharpened
  rule that TOLERANCES NEVER DECIDE COMBINATORIAL TOPOLOGY, only metric geometry
  and serialization. Spec D2 amended; synthesis wave-2 addendum written.
- Implementation consequences (future, none affect M2a): a Rust `AlgebraicReal`
  expression-DAG number type (interval + BFMSS separation bounds) before M4's
  conic arrangement work; the 2D profile subsystem gets Tier-1 exact conic
  predicates; QI port consideration at M5; curved snap rounding is open, so
  serialization targets tolerant boundary rep.
- Still PAUSED. Resume point unchanged: execute M2a Task 1 (scaffold keel-geom).
- Background: WSL fuzz rerun (cargo +nightly) of the two M1 targets still in
  flight at time of writing; check task output before logging the M1 gate item
  closed.

---

## 2026-06-07 (addendum 3: wave-3 research absorbed, dossiers 12-19)

- Eight more dossiers landed and were reviewed (12 direct modeling, 13 healing,
  14 determinism/serialization, 15 feature recognition, 16 assembly, 17 GD&T/PMI,
  18 patents, 19 regeneration). Synthesis: docs/research/02-synthesis-wave3.md.
- PROJECT-LEVEL: (1) D10 patent posture added to spec. Three HIGH zones with safe
  alternatives adopted: no auto-inferred Live-Rules constraints (explicit user
  constraints instead, ~2034); no single-body mesh+B-rep convergent ops (separate
  bodies + boundary conversion, ~2035+); no U-splines (THB-splines, or classic
  T-splines whose patent EXPIRED March 2024). Classical core all clear. License
  MIT OR Apache-2.0 validated. New standing rule: defensive publication of novel
  Keel algorithms. (2) Differentiation thesis promoted into the spec mission:
  exact/certified topology over tolerant metric geometry, by affordability
  gradient; the literature's consensus hybrid that nobody ships.
- D9 now carries the binding file-14 determinism contract + file-format doctrine
  (pure-Rust libm for critical transcendentals, reproducible reductions, exact
  f64 round-trip, append-only schema + down-save, AP242 interchange, fuzzed
  importers) and the wave-3 kernel-boundary obligations (snapshot/restore,
  fail-soft operators, datums + geometric selectors, structural-sharing
  instancing + location primitive, local coords + f64 placement, typed PMI-ready
  attributes).
- Cross-wave note: persistent naming/lineage now demanded independently by SIX
  consumers (regen, direct modeling, assembly, PMI, FR, collaboration). D9 is
  the most load-bearing non-geometric decision in the kernel.
- Still PAUSED. Resume point: M2a Task 1. None of wave 3 affects the M2a plan.

---

## 2026-06-07 (addendum 4: UNPAUSED; fuzz find fixed; merges reconciled; T-splines dropped)

- USER DECISIONS: (1) T-splines dropped entirely (added to spec non-goals): no
  kernel ships them, no format exchanges them, THB covers refinement. (2) Patent
  posture discussed and affirmed: design-arounds are evaluated against CLAIM
  ELEMENTS, not outcomes; when implementing near a fenced zone, record which
  claim element is omitted + prior-art citation (composes with the defensive
  publication rule). (3) UNPAUSED: proceed where safe.
- FIRST FUZZING CAMPAIGN RESULTS (M1 gate item closed): fuzz_bernstein_roots
  clean over 8.6M runs/10min. fuzz_solve_cubic FOUND A REAL BUG: finite inputs
  with extreme coefficient ratios (a~1e-308, b~1e308) overflowed the
  discriminant/Cauchy bound to inf and returned non-finite roots. Fix: exact
  power-of-two coefficient normalization (root-preserving) in solve_quadratic
  and solve_cubic + golden regression tests (fuzz_regression_extreme_ratios...).
  58 tests green. FINDING 2 (caught by the 5-min rerun): denormal coefficients
  survive normalization; q/a, -c/b, and the Cauchy bound can still hit inf.
  Fix: non-finite roots are dropped (not representable in f64) and an infinite
  Cauchy bound falls back to the quadratic part's roots. Both findings have
  golden regression tests. All-artifact repro + 7-min rerun in background.
- REPO TOPOLOGY NOTE: the research agent works on branch worktree-nurbs-research
  and merges into our branch. A conflicted merge of its synthesis-v2 spec deltas
  was reconciled BY THE AGENT (commit 807e32c): resolution verified good: our
  stronger D3/D9/D10 govern (its synthesis v2 explicitly defers to wave-spec
  text), its unique additions merged (D6 cubic-hybrid-clipping default,
  keel-tess/keel-io crate contracts, comparison-cascade testing oracle, risk
  entries, D9 CI clause). docs/research/00-synthesis-v2.md is the agent's
  delta record (9-22); our wave records are 01-synthesis-wave2.md and
  02-synthesis-wave3.md. Spec remains the single binding document.
- Round-3 research (kernel/20-24: sheet metal, kinematics, model diff,
  metrology fitting, canonical recovery) is bound into the spec via synthesis-v2
  deltas 16/17/20/22; full reads deferred to their milestones (all long-tail,
  none affect M2).
- RESUMING: M2a execution from Task 1 (scaffold keel-geom).
- FUZZ FINDING 4 (15 minutes after finding 3): midpoint overflow in
  solve_bracketed: 0.5*(lo+hi) hits inf for brackets near f64::MAX, poisoning
  the bracket (returned Some(inf)). Fixed as 0.5*lo + 0.5*hi everywhere; exact
  artifact bits kept as a golden test. Four real findings from under an hour
  of fuzzing: the corpus-as-product doctrine is earning out. Known limitation
  noted, not yet fixed: x_tol = 1e-14*bound gives poor RELATIVE accuracy for
  small roots of mixed-scale cubics (kernel consumers operate in normalized
  boxes; revisit if a consumer needs it).
- M2b PLAN CHECKLIST (from a peer-session review, adopted): (1) the surface
  evaluator contract is the full local-geometry record per kernel/06
  (E,F,G,L,M,N,K,H, principal curvatures + directions), not point+normal;
  (2) the evaluation core must be INTERVAL-CAPABLE by design (Krawczyk
  per-step certification in M5 consumes interval evaluation of surfaces and
  derivatives; retrofitting generic-over-scalar evaluation is expensive).
  Curve derivatives in M2a are already first-class (hodograph + rational
  recursion + FD oracle tests; projection consumes 2nd derivatives).
