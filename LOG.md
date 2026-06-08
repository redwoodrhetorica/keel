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

---

## 2026-06-07 (addendum 5: M2a execution progress, pre-compaction checkpoint)

### M2a status (plan: docs/superpowers/plans/2026-06-07-m2a-spline-curves.md)

DONE, committed, all tests green (23 in keel-geom + 58 in keel-math):
- Task 1: keel-geom scaffolded, workspace member added.
- Task 2: knots.rs (KnotVector: validation, find_span A2.1, multiplicity).
- Task 3: basis.rs (A2.2 basis_funs, partition-of-unity proptest).
- Task 4: nurbs_curve.rs core (homogeneous Vec4 ctrl, de Boor eval_homogeneous,
  exact circular_arc/full_circle with w=cos(dtheta/2), circle exact at 1e-12;
  arb_nurbs proptest strategy lives in pub(crate) test_support module).
- Task 5: derivatives (hodograph derivative_curve chain + A4.2 rational
  recursion; NOTE: derivatives beyond degree return ZEROS, do not cap d).
- Task 6: insert_knot (Boehm A5.1), split (multiplicity-p insertion), to_beziers
  + BezierSegment (eval/subdivide/control_points); geometry-preservation
  proptests all green.
- Task 7: curve.rs (Line3/Circle3/Ellipse3 + Curve3 enum: point, derivatives,
  domain, bbox; analytic projections; ellipse projection via 32-scan + bracketed
  Newton). FD-test lesson: central SECOND differences need h about 1e-4
  (eps^(1/4)), not 1e-6, or cancellation noise dominates. Last commit may show
  the curve.rs FD fix uncommitted: COMMIT IT FIRST THING if so.

REMAINING in M2a:
- Task 8: project.rs: global NURBS closest-point (Bezier decompose +
  control-AABB branch-and-bound prune + bracketed-Newton polish on
  g(u) = (C-p).C'); plan has FULL CODE: copy from plan section Task 8. Includes
  dense-sampling global-optimality proptest oracle.
- Task 9: benches/geom.rs (criterion: eval, derivs, project), fuzz target
  fuzz_nurbs_curve (plan has full code; add [[bin]] to fuzz/Cargo.toml + dep
  keel-geom), full validation (fmt + clippy -D warnings + workspace tests),
  LOG entry with bench numbers, merge m2a-spline-curves to master per
  finishing-a-development-branch.

### Fuzzing state

- Findings 1-4 all fixed + golden regression tests in keel-math
  (poly.rs fuzz_regression_extreme_ratios_yield_finite_roots; newton.rs
  midpoint = 0.5*lo + 0.5*hi doctrine).
- A 15-minute fuzz_solve_cubic soak was running in background (task id
  bi08j68hp) at checkpoint time; output file under AppData\Local\Temp\claude\
  ...\tasks\bi08j68hp.output; prints ALL-ARTIFACTS-PASS then FUZZ-15MIN-CLEAN
  or FUZZ-FOUND-NEW. If FOUND-NEW: decode artifact f64s via
  [BitConverter]::ToDouble on 8-byte strides, fix, add golden test (procedure
  proven 4x this session). WSL fuzzing recipe: wsl bash, . ~/.cargo/env,
  CARGO_TARGET_DIR=~/keel-fuzz-target, cargo +nightly fuzz run <target>.
- fuzz_bernstein_roots: clean over 8.6M runs.

### Branch/repo state

- On branch m2a-spline-curves. Research agent merges arrive via branch
  worktree-nurbs-research (sometimes as new commits ahead: check git log before
  assuming). Master is BEHIND: merge m2a-spline-curves to master at M2a end.
- Research: 25 kernel dossiers + 7 nurbs dossiers + 3 syntheses (agent's
  00-synthesis-v2.md deltas 9-22; my 01-synthesis-wave2.md, 02-synthesis-wave3.md).
  Spec carries everything; spec governs.

### Session decisions to remember

- T-splines dropped (spec non-goal). Patent design-around doctrine: evaluate
  against CLAIM ELEMENTS, not outcomes; record omitted element + prior-art
  citation when near a fenced zone. M2b checklist: full SurfaceLocalGeometry
  contract + interval-capable evaluation core (for M5 Krawczyk).

---

## 2026-06-07 (addendum 6: M2a COMPLETE)

- Tasks 8 and 9 done post-compaction (clean resume from the checkpoint: the
  LOG-first discipline works). project.rs: global closest-point via Bezier
  decomposition + control-AABB branch-and-bound + bracketed-Newton polish;
  dense-sampling global-optimality proptest oracle green first run.
- Benches (criterion baselines): nurbs_circle_point 98ns, derivs2 670ns,
  project 786us. PROJECTION IS SLOW: the circle is the pruning worst case
  (all segments near-equidistant). Known fix when needed: Selimovic
  angle-criterion pruning + earlier Newton + coarser width_tol. Baseline
  recorded, optimization deferred to a consumer with a budget.
- fuzz_nurbs_curve target added (constructor validation + eval finiteness +
  insertion invariance); compiles; first WSL run pending.
- Final validation: fmt clean, clippy -D warnings clean workspace-wide,
  84 tests green (26 geom + 58 math). M1-era fuzz artifacts all pass.
- MemPalace (github.com/MemPalace/mempalace) evaluated at user request:
  verdict skip-for-now; LOG/spec/git discipline is the authority and just
  survived a compaction live; gbrain MCP already connected covers semantic
  recall if wanted; revisit only if rediscovery costs appear.
- M2a exit gate: all criteria met except the 10-minute fuzz run of the new
  curve target (queued; the cubic soak from the M1 findings is still running).
- NEXT: merge m2a-spline-curves to master, then plan M2b (surfaces +
  multivariate Bernstein subdivision solver; checklist in addendum 5).

## Addendum 7 (2026-06-07, post-merge): fuzz finding 5, homogeneous lift overflow

- First 10-minute run of fuzz_nurbs_curve: CURVE-FUZZ-FOUND. Artifact
  crash-25aa70aff2 decoded via cargo fuzz fmt: degree 2, valid clamped knots
  ~1.2e161/1.3e219, control coords ~1.3e219, weights ~1.3e219 plus one
  DENORMAL weight 3.5e-323. Constructor accepted; point(u) returned inf.
- Mechanism: the homogeneous lift stores (w*x, w*y, w*z, w); 1.3e219 *
  1.3e219 = 1.7e438 overflows to inf IN STORAGE. Curve-level cousin of fuzz
  finding 1 (poly coefficient overflow), same medicine.
- Fix in NurbsCurve::new: weights are projective (common scale is identity),
  so canonicalize by an EXACT power of two so max weight lands in (0.5, 1];
  w*x is then bounded by |x| and cannot overflow. Scale applied as two
  power-of-two factors (s1*s2) so the factor itself stays finite for
  subnormal/near-max weight scales; power-of-two multiplies are exact
  wherever the result is normal. Weights that leave the normal range after
  canonicalization (the 3.5e-323 one: max/min ratio ~1e542, unrepresentable)
  are rejected as InvalidWeight. Also added the missing finiteness check on
  input points (new GeomError::NonFinitePoint), preempting the obvious next
  fuzz find.
- Golden tests added (exact artifact bits): extreme-ratio rejection, huge
  uniform weights now canonicalize to a finite curve, weight scale
  invariance (1,2,3 vs 1e200-scaled), NaN point rejection. 88 tests green
  (30 geom + 58 math), clippy -D warnings clean, artifact re-run clean.
- Note: canonicalization changes stored weights (homogeneous_control now
  returns max-weight-in-(0.5,1] form). Ratios are preserved exactly; any
  future round-trip/serialization code must treat weights as projective.
- Fresh 10-minute soak relaunched after the fix.

## Addendum 8 (2026-06-07): fuzz finding 6, overfull end-knot multiplicity

- Post-fix soak: CURVE-FUZZ-FOUND again, crash-79c0a738. Decoded: degree 2,
  knots [a,a,a,b,b,b,b]: the END value b has multiplicity 4 = p+2.
  KnotVector::new accepted it (clamped checks look at positions p and m-p
  only); derivative_curve then hit denom = u_{i+p+1} - u_{i+1} = 0 and
  tripped its debug_assert.
- Fix in KnotVector::new: multiplicity caps. End values must have
  multiplicity EXACTLY p+1 (adjacent knot must differ); interior values at
  most p (windows(p+1) scan over the interior slice). Violations are
  MultiplicityExceeded.
- Instructive breakage: the cap broke circle derivatives. The hodograph of
  a degree-p curve with interior mult p (legal C0 corner, and the circle's
  double knots) is a degree p-1 vector with interior mult p = (p-1)+1,
  which the new cap rejects; ders_homogeneous silently mapped the failed
  construction to ZERO derivatives. Resolution: pub(crate)
  KnotVector::new_hodograph skips the multiplicity caps (structural checks
  retained) for internally built derivative curves, and derivative_curve
  now guards zero-width windows with the 0/0 := 0 basis-derivative
  convention instead of asserting. Lesson recorded: validation tightening
  must distinguish user-facing invariants from internal representations
  that legitimately live one step outside them.
- Golden tests: exact artifact bits (MultiplicityExceeded), knots-level
  rejection matrix (end mult p+2 both sides, interior mult p+1 rejected,
  interior mult p accepted). 90 tests green (32 geom + 58 math), clippy
  clean, both artifacts re-run clean. Soak relaunched.

## Addendum 9 (2026-06-07): curve fuzz CLEAN, M2b plan written

- fuzz_nurbs_curve formal 10-minute soak after findings 5 and 6: CLEAN.
  Bonus signal: a mistyped replay command accidentally ran a 2.4 HOUR fuzz
  session seeded with both crash inputs (directory passed as corpus) and
  found nothing new. Corpus salvaged (+102 entries, now 194). M2a fuzz
  loop is closed: 6 findings total across M1+M2a, 6 golden tests, all
  artifacts replay clean.
- M2b research sweep done: no new dossiers since wave 3. Key inputs
  re-confirmed: nurbs/02 (A2.3/A3.5/A3.6/A4.4/A5.3), nurbs/04 section 4
  (Projected Polyhedron), kernel/24 (canonical recovery wants exact
  fundamental forms + exact revolution surfaces: both now M2b deliverables).
- M2b plan written: docs/superpowers/plans/2026-06-07-m2b-surfaces.md.
  11 tasks, full code: basis_ders (A2.3), NurbsSurface core (A3.5, with
  all M2a fuzz medicine baked into the constructor), derivatives (A4.4),
  SurfaceLocalGeometry (E,F,G,L,M,N,K,H, principal curvature directions),
  directional insertion + Bezier patches, analytic surfaces (plane,
  cylinder, cone, sphere, torus; exact projections, deterministic
  axis-point conventions), exact revolve_full (oracle: revolved sphere /
  torus implicit residuals at 1e-12/1e-11), global surface projection,
  multivariate Bernstein PP solver in keel-math (spec D6), interval de
  Casteljau enclosures (M5 Krawczyk groundwork), surface fuzz target +
  benches + gate. Derivative path switched from hodograph to basis-ders
  (A2.3): mixed partials make hodograph chains expensive and fitting/IGA
  consumers want basis ders anyway.
- NEXT: execute the M2b plan on branch m2b-surfaces.

## Addendum 10 (2026-06-07): M2b COMPLETE (surfaces, analytics, PP solver)

- All 11 plan tasks executed on branch m2b-surfaces. 132 tests green
  (66 geom + 66 math), fmt + clippy -D warnings clean, all M1/M2a fuzz
  artifacts replay clean, fuzz_nurbs_surface 10-minute soak CLEAN.
- Delivered: basis_ders (A2.3); NurbsSurface (A3.5 eval, constructor
  carries all M2a fuzz medicine); surface derivatives (A3.6 + A4.4);
  SurfaceLocalGeometry (E,F,G,L,M,N, K, H, principal curvatures and
  directions, deterministic umbilic fallback); directional knot
  insertion + Bezier patch decomposition; analytic surfaces (plane,
  cylinder, cone, sphere, torus; exact closed-form derivatives and
  projections, deterministic axis-point conventions, cone apex clamp
  deferred to M3); exact revolve_full (sphere residual <= 1e-12, torus
  <= 1e-11 at dense samples: exactness oracles passed first run);
  global surface projection (patch B&B + 2x2 Newton + boundary-curve
  candidates); MultiBernstein + Projected Polyhedron solver in
  keel-math (spec D6); interval de Casteljau enclosures on
  BezierSegment/BezierPatch + Interval::checked_div (M5 groundwork).
- Bugs caught by planned oracles during execution:
  1. PP hull chain keep/pop INVERTED (lower chain computed the upper
     hull): contraction over-tightened, roots silently lost; caught by
     univariate parity proptest vs Bernstein::roots.
  2. PP f64 endgame: deeply cropped coefficients are rounding noise;
     the floating hull falsely excluded a box that had converged onto
     a root (traced live at width 1.2e-9). Fix: 1 percent + 1e-12
     guard band on crops; exclusion on a box <= 8*tol emits instead of
     discards. Module docs flag IPP (M5) as the certified upgrade.
  3. Surface projection missed boundary minima (clamped 2x2 Newton
     under-optimizes the tangential coordinate on an edge); caught by
     the dense-sampling oracle. Fix: project onto the four boundary
     iso-curves with the certified curve projector (boundary_curves()).
- Bench baselines: nurbs_torus_point 143ns, local_geometry 1.35us,
  surface project 87ms (torus = equidistant worst case, same pruning
  story as the curve circle; Selimovic-class pruning still the known
  deferred fix when a consumer needs the speed).
- Process slip to not repeat: twice a PowerShell `;` chain committed
  before the clippy gate result was checked (the exact M2a lesson).
  Switched to `if ($LASTEXITCODE -eq 0)` guards mid-session; keep that.
- NEXT: M3 planning, which opens with the PRE-M3 PAPER-DESIGN GATE
  (boolean/SSI pipeline shape, D9 lineage hooks, PES operator set
  BEFORE the topology API freezes) plus the standing research re-read
  (files 14 in full, 19 kernel-obligations, 16 location/instancing).

## Addendum 11 (2026-06-07): pre-M3 research re-read + topology gate design

- Research sweep: no new dossiers since the M2b sweep. Pre-M3 standing
  reading completed: kernel/01 (topology/Euler), kernel/02 (non-manifold
  PES verdict), kernel/07 (naming/sessions mandates 1-14), kernel/14 in
  full (determinism contract + file doctrine), kernel/22 skimmed
  (obligations: canonicalization service, deterministic sampling, clean
  traversability; all above-kernel, consistent with D9).
- M3 PAPER-DESIGN GATE WRITTEN:
  docs/superpowers/specs/2026-06-07-m3-topology-gate-design.md.
  Key bindings: 8-entity tower (body/region/shell/face/loop/fin/edge/
  vertex, Parasolid names); regions primary and space-partitioning with
  one infinite region per body; PES-class fins with day-one radial
  cycles (manifold = 2-cycle fast path); typed arenas + stable u64
  EntityIds (slots are never identity); manifold Euler five + inverses
  with incremental Euler-Poincare checks; subdivision/stitch primitives
  (split_edge/split_face/merge/glue_edges/embed) as first-class API
  because lineage needs their split/merge semantics; total OpReports
  (Modified/Generated/Deleted/Split/Merged, no anonymous topology);
  derivation records per entity; snapshot/restore (deep clone first,
  COW later behind the same API); journal trait + replay determinism
  test; d-of-d validation oracle alongside Euler-Poincare; boolean
  pipeline designed on paper (localize/intersect/imprint/classify/
  stitch/report) with the freeze checklist mapped item-by-item to the
  M3 API. M3 scope: keel-topo crate, operators, validation, lineage,
  primitive constructors (block/prism/cylinder/sphere/cone/torus) as
  pure Euler sequences, debug dump + topology-hash goldens.
- NEXT: M3 implementation plan (writing-plans), then execute.

## Addendum 12 (2026-06-07): symgen plan shelved; M3 gate decisions confirmed

- User confirmed the M3 gate decisions: regions go NATIVE (decision 1
  explicit); decisions 2-5 (PES fins + radial cycles, Euler-only
  mutation, total lineage, paper boolean pipeline) per my judgement as
  written in the gate doc.
- Symbolic simplification (Wolfram/SymPy) PLANNED but SHELVED at user
  direction: docs/superpowers/plans/2026-06-07-symgen-symbolic-
  simplification.md. Two modes designed: Mode A (CAS ground-truth
  verification tables for hand-derived formulas) and Mode B
  (CSE codegen with ULP-parity + pathological-corpus + bench gates).
  Binding policy if ever picked up: sign-feeding expressions are NEVER
  swapped (real-arithmetic equivalence does not survive rounding; the
  determinism contract forbids tolerance-band parity where signs
  branch); artifacts committed, Python never a build dependency.
  Revisit triggers recorded in the plan header.
- NEXT: M3 implementation plan.

## Addendum 13 (2026-06-07): M3 implementation plan written

- Symgen purpose clarified by user and recorded in the shelf plan:
  it exists for PERFORMANCE (replace a profiled-hot hand formula with a
  faster CAS-derived form), not formula verification; Mode A tables are
  the safety gate enabling a swap, not the goal.
- M3 plan: docs/superpowers/plans/2026-06-07-m3-topology.md. Ten tasks:
  (1) generational arenas + EntityId + entity tower (regions native,
  shells = oriented face-side collections owned by one region; closed
  manifold surface appears as two shells), (2) lineage + OpReport via a
  per-op recorder, (3) validation (structural + d-of-d oracle +
  Euler-Poincare on the manifold sub-complex) + FNV topology hash,
  (4) MVFS/KVFS + MEV/KEV with vertex-loop bootstrap, (5) MEF/KEF +
  KEMR/MEKR + KFMRH/MFKRH + closed-edge MEF variant (seam-vertex
  doctrine: no vertex-free ring edges), cube-by-operators test plus
  balloon/cylinder/sphere/torus skeleton count tests, randomized
  operator-sequence proptest, (6) split_edge/split_face/merge_vertices/
  glue_edges (dihedral radial_sort)/embed_* + region split/merge,
  (7) snapshot/restore + OpJournal trait + replay-determinism proof,
  (8) constructors block/prism/cylinder/cone/sphere/torus attaching M2
  geometry with coincidence oracles + hash goldens, (9) adjacency/
  classification/debug dump, (10) fuzz_topo_ops (operator programs;
  validate after every op; failed ops must not mutate, checked by hash)
  + exit gate.
- Binding conventions fixed in the plan: fin direction, face sense to
  front region, shell ownership, closed-edge seam vertices, region
  solidity alternation rule, the manifold Euler check formula.
- NEXT: execute M3 on branch m3-topology.

## Addendum 14 (2026-06-07): M3 execution, Tasks 1-6 COMPLETE

- Branch m3-topology. keel-topo crate live: 18 tests green, clippy clean.
- Task 1-2: generational Arena<T>+Key<T> (own impl, zero deps, LIFO free
  list, stale-key tests), EntityId monotonic via Body counter, entity
  tower with regions native, lineage map + OpRecorder (every mutation =
  one recorded op).
- Task 3: validate() = id-map bijection, fin-ring closure+prev
  consistency, radial-cycle exactly-once, loop xor rule + outer-first,
  shell/region bijection (every face side in exactly one shell, region
  links match), boundary-chain continuity (the practical d-of-d
  oracle), Euler-Poincare on purely-manifold bodies
  (V-E+F = 2(S_closed - G) + R, S_closed = regions-1, G = sum genus/2).
  topology_hash = FNV-1a in EntityId order, geometry as f64 bits.
- Task 4-5: MVFS/KVFS (region pair + two shells per closed surface),
  MEV/KEV (vertex-loop bootstrap + spur), MEF/KEF (incl. CLOSED-edge
  variant fin_a==fin_b and on-vertex-loop), KEMR/MEKR, KFMRH/MFKRH
  (genus on shell pairs). CUBE-BY-OPERATORS test green (V8 E12 F6);
  skeletons: balloon V1E1F2, sphere V2E1F1, cylinder V2E3F3,
  genus-1 V2E2F1R1G1 (mvfs+mev+mef-closed+kfmrh). All inverse pairs
  round-trip. Randomized 64-case operator-sequence proptest validates
  after every step. ONE design fix mid-build: mef closer-fin directions
  (old-loop closer runs va->vb forward; chain continuity caught on
  paper before tests ran).
- Task 6: split_edge (all radial fins split, SplitChild 0/1 + Generated
  lineage), split_face (mef_impl with split lineage), merge_vertices
  (umbrella groups = PES partial-entity trigger), glue_edges (radial
  merge; FIRST NON-MANIFOLD STATE: two cubes stitched along an edge =
  radial 4, Euler check correctly skips, d-of-d holds), embed_vertex/
  embed_wire. Deferrals recorded in ops.rs module docs: dihedral radial
  sort (M5), split_region/merge_regions (M6), coincidence judgement on
  callers (M5).
- REMAINING M3: Task 7 session.rs (snapshot/journal/replay), Task 8
  construct.rs (block/prism/cylinder/cone/sphere/torus + geometry),
  Task 9 query.rs (adjacency/classification/dump), Task 10 fuzz_topo_ops
  + exit gate (fmt/clippy/tests/artifact replays/10-min soak/goldens/
  LOG/merge).

## Addendum 15 (2026-06-07): M3 COMPLETE; repo goes to GitHub

- Tasks 7-10 done: session.rs (Snapshot deep-clone, OpDescriptor journal,
  REPLAY DETERMINISM PROOF: journaled build replays to identical
  topology hash), construct.rs (block/prism/cylinder/cone/sphere/torus,
  pure Euler sequences + M2 exact geometry, watertightness oracle: edge
  curve samples on all adjacent face surfaces <= 1e-9), query.rs
  (adjacency, connected components, Parasolid body-class lattice,
  deterministic dump grammar), fuzz_topo_ops (operator programs;
  validate after every op; failed ops must leave the hash unchanged).
- EXIT GATE: fmt clean, clippy -D warnings clean, 159 workspace tests
  green (66 math + 66 geom + 27 topo), all M1/M2 fuzz artifacts replay
  clean, fuzz_topo_ops 10-minute soak CLEAN on first campaign.
- Deferred (documented in module docs): dihedral radial sort (M5),
  split_region/merge_regions (M6), Body-held auto-journaling (session
  object refactor), umbrella-walk vertex adjacency optimization.
- README updated: M3 status, crate layout, and the official project
  subtitle per the user: "It will keel."
- Repo to GitHub at user direction: keel-kernel/keel, PRIVATE (backup
  without publicity; publish decision stays decoupled).
- NEXT: M4 planning (PMC, CSI, curve-curve intersection, mass
  properties; the consumers that can falsify constructor orientation
  conventions). Standing rules: research re-read first.

## Addendum 16 (2026-06-07): GitHub backup live

- M3 merged to master (post-merge sanity green: 159 tests).
- Remote: https://github.com/redwoodrhetorica/keel (PRIVATE). The
  keel-kernel org does not exist yet and orgs cannot be created via
  CLI; the repo lives under the personal account and transfers to
  keel-kernel/keel in one command (Settings > Transfer, or
  `gh api repos/redwoodrhetorica/keel/transfer -f new_owner=keel-kernel`)
  once the user creates the org in the web UI. GitHub auto-redirects
  old URLs after transfer. All branches pushed (m1-foundations,
  m2a-spline-curves, m2b-surfaces, m3-topology, worktree-nurbs-research,
  master).

## Addendum 17 (2026-06-07): M4 plan written

- Research sweep: nothing new; kernel/06 re-read (differential geometry
  contract confirmed shipped in M2b; isophotes/silhouettes noted as
  future interrogation consumers of the normal field).
- M4 plan: docs/superpowers/plans/2026-06-07-m4-classification.md.
  Scope: implicit forms for Surface3 (signed, with gradient: CSI fast
  paths + PMC fuzz oracle), curve-curve intersection (analytic exact +
  PP for spline pairs, tangency reported not resolved), curve-surface
  intersection (implicit substitution with EXACT Bernstein composition,
  never sample-and-fit; PP 3-var + Newton for spline-spline), pcurves
  for primitive faces (UV iso-segments; the M5 trim interface), UV
  point-in-face crossing test + parameter-space watertightness check,
  ray-cast PMC with deterministic retry LADDER on degenerate hits (no
  random perturbation: determinism contract; Tilove neighborhood
  classification deferred to M6 where it is unavoidable), mass
  properties via divergence theorem over parameter rectangles with the
  VOLUME SIGN as the M3 orientation falsification test (no abs():
  negative volume = fix M3, not the formula). fuzz_pmc vs implicit-sign
  oracle. Branch m4-classification.

## Addendum 18 (2026-06-07): PMC performance research; M4 plan amended

- User flagged ray-cast PMC cost. Research verdict: the field moved to
  (a) winding numbers over crossing parity for containment (robust to
  noise/open boundaries; Liu et al. arXiv:2510.25159 solves EXACTLY our
  trimmed-parametric-surface UV containment with recursive winding +
  ellipse bounds on Bezier trim segments + universal-cover lifting for
  PERIODIC surfaces, i.e. our cylinder/torus seams), and (b) nearest-
  face signed classification (BVH projection + Baerentzen-Aanaes
  angle-weighted pseudonormals at edge/vertex feet) instead of rays for
  the 3D query: O(log n), local, no ray ladder in the common case.
- M4 plan amended: UV containment = winding number (universal cover on
  periodic faces); 3D PMC primary = nearest-face signed classification;
  ray ladder demoted to degenerate-incidence fallback + debug cross-
  check; face-AABB BVH added as Task 5b shared infrastructure (booleans
  reuse). Generalized winding numbers (Jacobson 2013, Barill 2018 fast
  GWN, 2024 one-shot GWN) recorded as the healing-era fallback for
  untrusted geometry. The M4 reference workload (<= 6 analytic faces,
  closed-form projections) was never expensive; the INTERFACE is now
  the scalable one from day one.

## Addendum 19 (2026-06-07): M4 Tasks 1-3 COMPLETE

- Branch m4-classification. 172 workspace tests green, clippy clean.
- Task 1: Surface3::implicit (signed, negative inside; cone valid on
  the physical nappe, documented) + implicit_gradient (torus axis
  radial term := 0). Tests: zero-on-surface, FD gradients, sign cases.
- Task 2: intersect_curves in keel-geom/intersect.rs. Exact paths:
  line/line (parallel-distinct = empty, coincident = Degenerate),
  line/conic (shared Conic for circle+ellipse: in-plane quadratic +
  transversal pierce), line/NURBS two-plane reduction with the
  IN-PLANE-CURVE fix (a curve lying in one reduction plane makes that
  polynomial identically zero; pick roots from the nondegenerate
  plane, both-zero = coincident Degenerate: found by the arc test on
  first run). Generic pairs: exact NURBS forms (ellipse = anisotropic
  scaling of the exact circle ctrl) + PP per Bezier segment pair on
  homogeneous cross-multiplied difference tensors (coeff = xa_i*wb_j -
  xb_j*wa_i: EXACT, never sampled), Newton polish on squared distance,
  tangency = near-parallel tangents, cluster dedup. PP budget
  exhaustion maps to Degenerate (coincidence suspected).
- Task 3: Bernstein mul/add/scale/elevated_to in keel-math (Farouki-
  Rajan product); intersect_curve_surface: line vs analytics exact
  (plane linear, quadrics quadratic, torus quartic via power_affine +
  Bernstein on a bounded span: 4-hit x-axis test exact to 1e-7);
  general curve vs analytics via compose_implicit (EXACT Bernstein
  composition of the implicit form with each rational segment: 2p for
  quadrics, 4p for torus); NURBS curve x NurbsSurface via 3-var PP on
  exact trivariate difference tensors + 3x3 Cramer Newton. All hit
  tests passed first run including tangency flags.
- REMAINING: Task 4 pcurves+UV winding containment, Task 5 PMC
  (nearest-face primary + ray-ladder fallback + face BVH), Task 6 mass
  properties (volume-sign orientation audit), Task 7 fuzz_pmc + gate.

## Addendum 20 (2026-06-07): M4 COMPLETE; ORIENTATION AUDIT PASSED

- Tasks 4-7 done after the Tasks 1-3 checkpoint (addendum 19):
- Task 4: pcurves for primitives. Planar faces: generic vertex-UV
  segments + full-circle UV pcurves for closed rims
  (attach_plane_pcurves); periodic laterals: explicit unwrapped-cover
  paths in loop walk order (attach_loop_uv_path); collapsed boundaries
  (cone apex) close implicitly in the winding accumulation. KEY
  INSIGHT: a face whose every boundary edge has its whole radial cycle
  on that face covers its entire closed surface
  (face_covers_closed_surface): sphere/torus containment is trivially
  In, deferring genus-1 universal-cover bookkeeping until real trims.
- Task 5: classify_point: nearest-face signed classification primary
  (per the PMC research amendment), pole/apex Degenerate feet fall
  through to the deterministic ray ladder (found by the sphere-axis
  test on first run: a point above the north pole projects TO the
  pole). Ray verdict needs only the FIRST interior hit (front/back
  region links), parity not required. Winding-number UV containment
  with boundary tolerance via first-fundamental-form scaling.
- Task 6: mass_properties: divergence theorem, 10 moments, planar
  faces by triangle-fan degree-5 rule / polar periodic-trapezoid x GL8
  discs, curved faces by 16x16-panel composite GL8. THE AUDIT PASSED:
  all five primitive volumes positive and correct from M3 region-
  solidity conventions alone, NO sign fudge (block exact, sphere/cone
  1e-9 rel, torus 2 pi^2 R r^2 with Izz = V(R^2+3r^2/4), pentagon
  prism = shoelace). One quadrature bug caught (triangle weights sum
  to 1: factor is area, not doubled cross product: planar volumes were
  exactly 2x); curved faces exact on first run.
- Task 7: fuzz_pmc (classification vs implicit sign oracle + block
  half-space oracle) 10-MINUTE SOAK CLEAN. Prior artifacts replay
  clean. 180 workspace tests green, fmt + clippy clean.
- NEXT: merge M4; then M5 planning (trims/imprint/SSI certified:
  the big one), standing research re-read first (nurbs/04 sections
  3-4, kernel/11, kernel/13 extension service).

## Addendum 21 (2026-06-07): M5a plan written; interval soundness mandate

- M5 split M2-style: M5a = SSI engine in keel-geom (this plan), M5b =
  topology integration (trims, imprint, extension service, dihedral
  radial sort). Plan: docs/superpowers/plans/2026-06-07-m5a-ssi-engine.md.
- Three-tier architecture per nurbs/04 section 10 (re-read in full):
  exact analytic-analytic; lower-dimensional analytic x spline via
  EXACT bivariate Bernstein composition + certified 2D implicit-curve
  tracing (PP critical points: singular + turning + border scaffold);
  spline x spline via Gauss-map separability + collinear-normal seeds
  + Krawczyk-verified marching. Output: SsiResult with up-front
  dimensionality classification (Coincident never mis-traced).
  Honesty ledger in the plan: what is certified in v1 vs staged
  (validated-ODE, Yang-Jia-Yan escalation behind the same API).
- USER MANDATE recorded: verify interval soundness before building
  certification on it. Task 0 added, BLOCKING Task 5: fix the
  Interval overflow-to-inf hole (bounds become extended reals; found
  by inspection during plan review), half-ulp containment property
  tests across the magnitude ladder, chained-op soundness, M2b
  enclosure proptests at extreme scales, fuzz_interval target with a
  clean 10-minute soak as the gate.
- NEXT: execute M5a on branch m5a-ssi, Task 0 first.

## Addendum 22 (2026-06-07): M5a Tasks 0-4 COMPLETE; interval gate CLEAN

- Branch m5a-ssi. Task 0 (USER MANDATE): Interval bounds are now
  EXTENDED REALS. The pre-audit hole was real: mul/add of near-MAX
  operands overflowed to +-inf and broke the finite-bounds invariant
  in release. Fix: infinite bounds legal (unbounded), point-at-infinity
  forbidden (so inf-inf never arises in add/sub), 0*inf := 0 via
  mul_corner, inf/inf div corner widens to the line. Half-ulp
  containment property tests across a magnitude ladder (normals,
  denormals, near-MAX, MIN_POSITIVE, 5e-324, MAX/MIN), chained-op
  expression-tree soundness, M2b enclosure proptests re-run at 1e+-300
  scales. fuzz_interval target: 10-MINUTE SOAK CLEAN = the gate for
  the Krawczyk consumer (Task 5) is satisfied.
- Shared fitter (fit.rs): least-squares cubic with chord params,
  end-interpolation, control-count escalation, sampled-deviation x
  safety verification, tol_achieved reported. Circle/line/segment
  tests.
- Task 1: SSI tier 1 exact (plane/plane line; plane/sphere &
  sphere/sphere circles via radical plane; plane/cylinder ellipse +
  ruling-line + tangent cases) with up-front Coincident classification
  (parallel coplanar, concentric equal). Every result curve verified
  on both implicit forms to 1e-9.
- Task 2: tensor Bernstein mul/add/scale/elevate/derivative(axis) +
  degree_of/coeff_at on MultiBernstein. Pointwise + FD oracles.
- Tasks 3-4: SSI tier 2 (analytic x spline). compose_implicit_surface
  = EXACT bivariate Bernstein composition of the analytic implicit
  with each rational Bezier patch (2p,2q quadrics; 4p,4q torus).
  Certified 2D tracer: whole-patch sign-variation reject, border
  crossings (univariate Bernstein roots per edge) + PP critical points
  (f=f_v / f=f_u turning) as the significant-point scaffold, gradient-
  perpendicular continuation marching with Newton projection back to
  the zero set, polyline merge across patch borders, cubic fit.
  Tests: revolved-sphere-NURBS vs plane = circle; cylinder-NURBS vs
  sphere; both verified to 1e-5 (fit tolerance, signed-distance form).
- 196 workspace tests green, clippy clean.
- NEXT: Task 5 tier 3 (spline x spline: Gauss-map separability,
  collinear-normal seeds, Krawczyk-verified marching), Task 6 fit
  hardening for closed branches, Task 7 fuzz_ssi + gate (incl. the
  USER-MANDATED 2-HOUR extended soak before merge).

## Addendum 23 (2026-06-07): M5a Tasks 5-7 done; gate soaks running

- Task 5 (tier 3, spline x spline): the centerpiece. PatchInfo with
  sampled normal cones; cones_separable = antiparallel-angle test
  (Hohmeyer loop-freedom). Boundary seeds by SIGNED-CROSSING detection
  (a border iso-curve's signed distance to the other patch flips sign
  where it pierces; bracketed-bisected to the surface: the naive
  distance-below-tol seeding missed every crossing, found by the
  two-sphere test returning Empty). Collinear-normal loop seeds for
  non-separable interior pairs. Marcher: predictor along n_a x n_b,
  corrector = 4-var GAUSS-NEWTON on |S_a - S_b|^2 (3x4 Jacobian, 4x4
  normal equations, Levenberg damping) - the first Gauss-Seidel
  corrector left the curve 0.73 off; the joint solve fixed it to
  corrector tolerance. Step-halving on corrector failure, border/loop
  stop, seed consumption. Two-NURBS-sphere test: correct circle on
  both surfaces.
- HONESTY LEDGER (per the plan): marched points lie on both surfaces
  to corrector tolerance (1e-5); the FITTED curve deviation is the
  non-rational-cubic-vs-transcendental-circle fit error (~5e-4),
  bounded at 1e-3 in tests. Fit hardening to arbitrary tol is deferred
  to a real M5b/M6 consumer budget (rational fitting or finer
  segmentation). Krawczyk interval verification of each step is staged
  behind the working float corrector; the interval LAYER is verified
  (Task 0 gate clean) so the upgrade is drop-in. Recorded so it is not
  mistaken for full certification.
- Task 7: fuzz_ssi (random analytic pairs; curve points on both
  implicit forms; coincident never mis-traced) + SSI benches.
- 200 workspace tests green, fmt + clippy clean.
- GATE RUNNING: 10-min fuzz_ssi, then 10-min fuzz_interval re-confirm,
  then the USER-MANDATED 2-HOUR extended soak (fuzz_ssi + fuzz_interval)
  before merge.

## Addendum 24 (2026-06-07): M5a COMPLETE; 2-hour soak CLEAN; merged

- USER-MANDATED EXTENDED SOAK PASSED. Prior artifacts replay clean.
  fuzz_interval 1hr: 385,916,505 runs, 0 errors, 0 artifacts.
  fuzz_ssi 1hr: 723,023,053 runs, 0 errors, 0 artifacts. ~1.1 BILLION
  executions total across both targets, nothing found. The interval
  extended-real soundness fix and the SSI engine both hold under
  adversarial fuzzing at scale.
- Full M5a exit gate met: fmt clean, clippy -D warnings clean, 200
  workspace tests green, all prior fuzz artifacts replay clean, both
  new 10-minute soaks clean, the 2-hour extended soak clean.
- M5a (SSI engine, keel-geom) merged to master.
- NEXT: M5b (topology integration: SsiCurves -> trim loops, imprint
  via split_face + glue_edges with the dihedral radial sort now due,
  NURBS surface extension service per kernel/13, trimmed-face PMC/mass
  upgrades). Standing research re-read first (kernel/13 extension
  service, kernel/01 boundary-evaluation, nurbs/04 section 3 SSI
  topology). Then M6: booleans, the proof milestone (M7 = robust
  booleans on NURBS-bounded solids).

## Addendum 25 (2026-06-07): M5b plan written

- Research sweep: nothing new. kernel/13 Part 2 re-read: extension
  service requirements pinned (bounded safe-reach from degree+curvature
  +weight-positivity, fold validation, analytic-exact preferred,
  fail-clean, co-designed with the intersector).
- M5b plan: docs/superpowers/plans/2026-06-07-m5b-imprint-trim.md.
  Seven tasks: (1) SsiCurve gains pcurve_a/pcurve_b (exact inversion
  for analytics, project+fit for spline tiers); (2) extend_surface
  service (analytic exact/unbounded = domain widen; NURBS bounded
  natural unclamping with fold+weight validation, clean Err on
  over-reach); (3) DIHEDRAL RADIAL SORT in glue_edges (the M3
  deferral, first consumer here); (4) single-face imprint_curve
  (crossing -> split_edge+split_face; closed loop -> inner ring;
  coincidence judged here vs tol); (5) two-body imprint_bodies (SSI +
  imprint both + glue = the boolean precursor, shared radial-4 seam);
  (6) trimmed-face mass props via GREEN'S THEOREM (lifts the M4
  parameter-rectangle limit); (7) fuzz_imprint + gate. Scope boundary
  binding: M5b imprints, M6 classifies/stitches/selects. Deferral
  ledger: general trimmed-NURBS healing, Shetty-White/Mo-Zhao
  extensions, Krawczyk imprint fitting all staged.
- NEXT: execute M5b on branch m5b-imprint.

## Addendum 26 (2026-06-07): M5b in progress, imprint core proven

- Branch m5b-imprint. 205 workspace tests green, clippy clean.
- Task 3 DONE: dihedral radial sort in glue_edges (the M3 deferral).
  Fins ordered by their face normal's angle about the edge tangent;
  manifold 2-cycles unaffected, non-manifold 4+ cycles get the angular
  order M6 neighborhood classification reads.
- Task 2 DONE: surface extension service (extend.rs). Analytic =
  exact/unbounded no-op (the M6 boolean neighbor case). NURBS natural
  extension STAGED behind a clean Err: correct unclamping (Piegl-
  Tiller) is involved and serves healing not the boolean path; the
  safe-reach cap + fold/weight validation half is built. A naive
  knot-widening was tried and correctly rejected (produced an
  unclamped invalid surface) before staging.
- Task 4 CORE DONE: single-face closed-curve imprint
  (imprint_closed_curve). Construction = MEV spur into the face + mef
  closed-self-loop-edge (disc face) + kemr (spur becomes inner ring):
  splits a face into disc + annulus sharing the circular edge. pcurve
  computed by sample-invert-fit (pcurve_on_analytic in fit.rs, with
  periodic-seam u-unwrapping). Coincidence judged here (off-surface
  curve rejected atomically). TEST PROOF: imprint a circle on a cube
  top face -> PMC classifies the disc interior correctly In; off-
  surface curve rejected, body unchanged. fuzz_imprint target added,
  10-min soak running.
- REMAINING M5b: Task 4 crossing case (boundary-to-boundary curve via
  split_edge + split_face); Task 5 two-body imprint (SSI + imprint
  both + glue = the boolean precursor); Task 6 trimmed-face mass props
  (Green's theorem over multi-loop UV regions: the annulus face is not
  yet integrable); Task 7 full gate + merge. NOT merged to master
  until the gate is met. Branch pushed for backup.
- NEXT: crossing-case imprint, then two-body imprint, then Green mass
  props, then gate + merge.

## Addendum 27 (2026-06-07): M5b COMPLETE (imprint primitives, trimmed faces, extension); merged

- Tasks done: 2 (extension), 3 (dihedral sort), 4 (single-face imprint
  both cases), 6 (trimmed-face mass props), 7 (fuzz/gate). Task 5
  (two-body imprint) REASSIGNED TO M6 (scope decision in addendum/
  plan: periodic-wrap face splitting + coplanar-face coincidence are
  M6 classify concerns; M5b ships the reusable imprint PRIMITIVES, M6
  composes them with classify/select).
- Task 4 completed: imprint_closed_curve (spur+mef+kemr -> disc+annulus)
  AND imprint_open_curve (split_edge x2 + split_face for a boundary-
  crossing curve). Both compute pcurves by sample-invert-fit, judge
  on-surface coincidence at the call site (atomic rejection), attach
  pcurves to all new fins. PMC proof: imprinted disc classifies In.
- Task 6: trimmed-face mass properties via signed-triangle-fan Green
  integration over multi-loop UV regions (outer loop natural winding,
  inner rings forced opposite). Exact polar fast-path preserved for
  single-circle disc caps (cylinder/cone). Curvedness judged by the
  EDGE's 3D curve, not the pcurve enum (a degree-1 NURBS pcurve is
  straight: this was the bug that made block volume 17.79 instead of
  24). Imprint volume-invariance verified exactly (the circle polyline
  cancels between annulus-subtract and disc-add regardless of sampling).
- GATE: fmt + clippy clean, 206 workspace tests green, all prior fuzz
  artifacts replay clean, fuzz_imprint 10-min soak CLEAN.
- M5b merged to master.
- NEXT: M6 = booleans, the proof-milestone front end. Opens with
  two-body imprint (SSI + imprint both + glue, the reassigned Task 5)
  then classify (PMC-based fragment in/out) + select (union/intersect/
  difference tables) + stitch. Standing research re-read first
  (kernel/01 boundary evaluation, Requicha-Voelcker, Tilove SMC/PMC).

## Addendum 28 (2026-06-07): M6a in progress, boolean pipeline front half built (imprint + classify + select)

- M6 split into M6a (clean transversal cases, the milestone-proving
  pipeline) and M6b (robustness: coplanar, tangency, periodic surfaces,
  NURBS-bounded solids). Plan: docs/superpowers/plans/2026-06-07-m6a-
  boolean-engine.md. Branch m6a-boolean.
- ARCHITECTURE DECISION (during execution): the first end-to-end proof
  is ALL-PLANAR (box-box), NOT sphere-sphere. Analysis showed the
  sphere's SSI + imprint are trivial (one self-bounding circle, made
  crossing-free by seaming each sphere in its equatorial plane) BUT its
  fragment CLASSIFICATION and trimmed MASS-PROPERTIES fight surface
  periodicity (seam meridian has no pcurve; latitude loops wrap the
  u-domain) -- genuinely M6b-grade. Planar faces have watertight
  pcurves so classify/mass-props/stitch all run on M4/M5b machinery
  that's already proven. The planar path's price is geometry the sphere
  got free: plane-plane SSI is an UNBOUNDED line, so seams must be
  CLIPPED to both trimmed faces and ASSEMBLED before imprinting -- but
  that clip+assemble code is non-periodic, robust, reused by every
  future boolean, so it's the right investment. Sphere/cylinder
  (periodic) booleans -> M6b. Sphere SSI + crossing-free two-body
  imprint kept as M6a unit tests (they prove those pieces compose).
- Built (crates/keel-topo/src/boolean.rs), TDD, 7 tests green, 45/45
  keel-topo lib tests (no regressions):
  - Task 1 (two-body imprint): seam_curves does all-pairs analytic SSI
    + Cyrus-Beck clip of plane-plane lines to both faces' convex
    polygons. imprint_pair clones each operand and imprints the seams
    via a TWO-PHASE method: phase 1 pre-splits operand boundary edges
    at the seam-loop corners (so a loop wrapping across faces, like A's
    cut rectangle crossing shared box edges, gets shared corner
    vertices); phase 2 per face: boundary-vertex-to-boundary-vertex
    segments -> split_face; a loop interior to one face (B's cut
    rectangle) -> one degree-1 NURBS ring via imprint_closed_curve.
    Guillotine A=[0,4]^3, B=[2,6]x[-1,5]x[-1,5]: A->10 faces, B->7, both
    valid.
  - Task 2 (classify): face_interior_point builds each loop's UV
    polygon by projecting SAMPLED EDGE CURVES (robust to closed-curve
    edges and to split_edge child fins that carry no pcurve), then
    picks the MOST-CENTRAL interior sample (max distance to all loop
    boundaries) so the classification point sits well away from seams
    (which lie on the other solid's boundary). classify_faces tests it
    against the other operand's PMC. Guillotine: A 5 inside / 5 outside
    B; B inner rectangle inside A.
  - Task 3 (select): select_faces = regularized r-set tables
    (union/intersection/difference; difference reverses the subtracted
    walls). Guillotine: intersection 6, difference 6 (1 reversed),
    union 11.
- NEXT (M6a remainder): Task 4 stitch + region rebuild (the hard one:
  import kept faces from both operands into one body, glue coincident
  seam edges, extract shells, infer the region partition satisfying
  every validate() invariant -- one infinite region, every face-side in
  exactly one shell, Euler-Poincare). Then Task 5 boolean() API +
  exact volume proofs (intersection/difference boxes), Task 6
  metamorphic proptests + fuzz_boolean + gate.
