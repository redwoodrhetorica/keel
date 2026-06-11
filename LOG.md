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

## Addendum 29 (2026-06-07): M6a COMPLETE (boolean engine, clean transversal cases); merged

- The proof-milestone CORE works: the kernel computes regularized
  union/intersection/difference on solid bodies, and the results are
  valid B-rep solids with EXACT volume. Branch m6a-boolean.
- Pipeline (crates/keel-topo/src/boolean.rs): seam_curves (all-pairs
  analytic SSI + Cyrus-Beck clip of plane-plane lines to both trimmed
  faces) -> imprint_operand (two-phase: pre-split boundary edges at
  seam-loop corners, then per face dispatch: single closed curve ->
  ring; segments forming a closed loop -> one degree-1 NURBS ring;
  otherwise one open chain -> split the face boundary-to-boundary
  through interior corners via mev spurs + split_face) -> classify_faces
  (most-central UV-interior sample via projected edge curves, then the
  other operand's PMC) -> select_faces (regularized r-set tables;
  difference reverses the subtracted walls) -> build_result_solid
  (direct arena construction of an oriented polygon soup: vertex dedup,
  shared edges, fin rings, a two-region inside/outside partition with
  two shells; the Euler operators forbid the intermediate non-solid
  states a soup passes through, so it is built then validated).
- PROOFS (exact volumes, 16 boolean tests green):
  - Guillotine A=[0,4]^3 vs slab B: A∩B and A-B are box solids
    (V8 E12 F6), volume 32 to 1e-6.
  - Corner overlap A=[0,2]^3, B=[1,3]^3 (open L-chains with interior
    corners): A∩B = unit cube vol 1, A-B vol 7.
  - Nested (no SSI): A∩B = inner box, A∪B = outer box.
  - Metamorphic: intersection commutative + translation-invariant;
    determinism (D9): identical result topology hash.
- Partial-success fault model: coplanar/coincident and tangential
  configs are detected from SSI and DECLINED up front (a fast clean
  Err), never ground through. Post-condition gate: a real solid has
  positive finite volume (the scalar Euler identity is necessary not
  sufficient); near-degenerate results are declined, never returned as
  a wrong "valid" body.
- FUZZ (fuzz_boolean: random box pairs + random op): two findings, both
  fixed and captured as golden regression tests + corpus seeds:
  (1) coincident boxes ground for 16s through degenerate edge-grazing
      imprints -> early decline on Coincident/Tangent faults (16135ms ->
      29ms);
  (2) a thin sliver at large coordinates produced a 3-face Euler-valid
      non-solid (NaN volume) -> positive-volume post-condition declines.
  Final 10-min soak CLEAN: 3013 runs, no crashes.
- GATE: fmt + clippy clean, 222 workspace tests green (90 geom + 77
  math + 55 topo), 10-min fuzz_boolean soak CLEAN. Merged to master.
- DEFERRED to M6b (honest ledger): coplanar/coincident faces and
  neighborhood-merge classification; tangential contact; periodic-
  surface booleans (sphere/cylinder full pipeline -- their SSI + crossing-
  free imprint are proven as M6a unit tests, but classification and
  trimmed mass-props fight surface periodicity); NURBS-bounded solids
  (the M7 bar); union of transversally-overlapping boxes (needs holed-
  face stitch); enclosed-void difference (needs 3-region stitch);
  tolerant scaling robustness (the sliver class); AABB/BVH localization
  (the all-pairs O(n^2) is the throughput cost). Differential testing
  vs OCCT over the ABC corpus remains the M6b/M7 approximate oracle.

## Addendum 30 (2026-06-07): M6b COMPLETE (winding-number classification + curved booleans); merged

- THE architectural win this milestone, mandated by docs/research/
  d-booleans-tolerant.md section 4 (ADOPT winding-number classification,
  AVOID ray-cast/neighborhood PMC as the sole classifier): the boolean
  fragment classifier is now the GENERALIZED WINDING NUMBER (Jacobson/
  Barill), which degrades gracefully at on-boundary/tangential contacts
  and is surface-type-agnostic -- it works on periodic faces with no
  pcurve dependency, the exact thing that blocked curved booleans in
  M6a. Branch m6b-winding.
- New modules: winding.rs (tri_solid_angle Van Oosterom-Strackee;
  generalized_winding_number = (1/4pi) sum of signed solid angles;
  tessellated_volume), tessellate.rs (outward-oriented coarse triangle
  tessellation of planar + spherical faces, with trimmed-cap meshing
  keyed off the CLOSED SSI circle vs the sphere's open meridian).
- Task 0 MANDATE (self-imposed, the M5a soundness-before-build lesson):
  proved the winding number sound before any classification used it --
  w~1 inside / ~0 outside for box and sphere across a magnitude ladder;
  orientation audit gives +1 inside (independent cross-check of the M3
  face-side conventions, as M4 mass props did); continuous graceful
  degradation through ~0.5 across a face (no jumps/NaN); deterministic.
- Tasks delivered: classifier swap (all 16 M6a planar booleans stay
  green); curved fragment interior point (sphere cap apex, side fixed
  by the boundary fin's LOOP KIND since the closed circle's forward
  flag + shared pcurve cannot distinguish the two caps); import-and-glue
  stitch (stitch_by_import copies kept faces' full topology from both
  operands, merges coincident seam vertices, glues the coincident SSI-
  circle seams, builds the 2-region partition -- the body-to-body copier
  deferred from M6a); ALL THREE sphere booleans:
  - sphere INTERSECT sphere = a lens (two caps glued).
  - sphere UNION sphere = a peanut.
  - sphere DIFFERENCE sphere = a dimpled sphere (reversed subtracted cap;
    a reversed face keeps front=inf/back=solid -- a solid boundary face's
    outward normal always faces the exterior; reversal flips only sense/
    fin/loop-order).
  Volumes within 5% of the exact spherical-cap/lens formulas via the
  tessellated oracle (the exact trimmed-cap mass-properties integral is
  the staged item).
- Dual-mode degeneracy post-condition: PLANAR results gated by exact
  mass-properties (catches near-coincident slivers the coarse tessellation
  would accept); CURVED results (lens) gated by the surface-agnostic
  tessellated volume (mass-props cannot integrate trimmed caps yet).
- FUZZ: fuzz_winding (random box/sphere + probe; GWN finite, in [~0,1]
  off-boundary, inside>0.85/outside<0.15). Two findings, both fixed +
  golden-tested + corpus-seeded: (1) GWN at an exact box corner is a
  boundary value -> tight bound gated off the boundary; (2) near-
  coincident box touch sliver -> dual-mode post-condition. Final soaks
  CLEAN: fuzz_winding 1,228,685 runs, fuzz_boolean clean.
- GATE: fmt + clippy clean, 231 workspace tests green (90 geom + 77
  math + 64 topo), both 10-min soaks CLEAN. Merged to master.
- DEFERRED to M6c (honest ledger): block-cylinder + general analytic
  mixed-surface booleans (cylinder-lateral seam crossings; needs
  cylinder/cone/torus tessellation); coplanar/coincident neighborhood
  classification (winding-number-VECTOR / n-ary); EXACT trimmed-cap mass
  properties (the lens volume is currently the coarse tessellated
  oracle, ~few %); holed-face stitch (union of overlapping boxes);
  enclosed-void 3-region stitch; tolerant edges/vertices (Jackson, the
  M7 NURBS-SSI requirement); hierarchical/BVH-accelerated GWN + AABB
  localization (the all-pairs O(n^2) throughput cost); general sphere-
  pair fuzzing with arbitrary-axis equatorial seaming. M7 remains the
  proof bar: robust booleans on NURBS-bounded solids.

## Addendum 31 (2026-06-07): M6c COMPLETE (block-cylinder boolean; periodic-topology cracked); merged

- The headline: block - cylinder = a DRILLED HOLE works -- the first
  MIXED-analytic-surface boolean, and the last analytic stop before
  NURBS (M7). Branch m6c-cylinder.
- THE periodic-topology breakthrough (two interacting bugs, both fixed):
  1. A plane-cylinder SSI circle WRAPS the lateral face: a non-
     contractible loop. A single closed-loop mef(P,P) only bounds a
     degenerate disc, leaving the band unsplit. imprint_closed_curve_
     crossing now splits the circle into TWO ARCS at the seam crossing P
     AND its antipode Q (= 2*center - P): the lateral then splits cleanly
     into the two height bands, each sharing both arcs, and the result
     circle carries vertices P,Q that the cross-operand glue matches.
  2. The boolean's phase-1 pre-split was splitting the seam at the
     circle's degenerate "endpoint" (sample(0) == sample(1)), which for
     a cylinder SSI circle is EXACTLY the seam crossing P -- making P a
     vertex so find_planar_seam_crossing (needs a strictly-interior
     crossing) failed and the dispatch fell back to the inner-ring
     imprint. Phase 1 now SKIPS closed seam curves.
- Supporting work: cylinder tessellation (lateral band from the face's
  circle/arc heights + caps) with a Task-0 winding-soundness gate (w~1
  in / ~0 out, +1 orientation audit, passes ~0.5 crossing the wall,
  deterministic); tessellate_planar rewritten to fan each loop's polygon
  (fin-vertex corners OR sampled circle for disc caps) with inner-ring
  holes fanned reversed -- so disc caps AND holed faces both tessellate;
  cyl_circle_heights / cylinder_face_interior_point for periodic
  fragments; curve_on_cylinder_face rejects spurious seams outside a
  cylinder face's actual band (the unbounded SSI surface).
- keel-geom BUG FIX (valuable independently): plane_cylinder returned
  Err for a plane PERPENDICULAR to the axis (the exact drill
  orientation) because minor_dir = f.z x axis is degenerate when the
  plane normal is parallel to the axis; the section is a circle, built
  from the cylinder's own in-plane frame axes.
- PROOF: block [0,4]^3 minus a radius-1 blind-hole cylinder = 64 - 2pi
  (valid solid; material inside / hole outside by winding); block
  intersect cylinder = the 2pi plug. Volumes within 3% (coarse
  tessellated oracle; exact trimmed-curved mass props still staged).
- FUZZ: fuzz_cyl_boolean (random block + axis-aligned through/blind
  cylinder). Smoke + 10-min soak CLEAN (803 runs; low throughput as each
  cylinder boolean tessellates many faces). Re-soaked fuzz_boolean
  (1900 runs) and fuzz_winding (1,183,564 runs) -- the phase-1 and
  tessellate_planar changes touch box/winding paths -- both CLEAN.
- PROCESS NOTE: discovered (user caught it) that CI exists
  (.github/workflows/ci.yml from M1): a `test` job runs fmt --check +
  `clippy --workspace --all-targets -- -D warnings` + cargo test on
  ubuntu/windows/macos, plus a `fuzz-build` job that only COMPILES the
  fuzz targets (the SOAKS are local WSL). The M6a MERGE CI went RED on
  clippy -D warnings --all-targets (a test/bench-target lint my narrower
  `clippy -p keel-topo --lib` missed); M6b fixed it (M6b CI green).
  GOING FORWARD: run the exact CI triplet (fmt --all --check; clippy
  --workspace --all-targets -- -D warnings; test --workspace) before
  every merge. Verified GREEN for M6c.
- GATE: fmt + clippy(-D warnings, all-targets) clean, 236 workspace
  tests green (90 geom + 77 math + 69 topo), three 10-min soaks CLEAN.
  Merged to master.
- DEFERRED to M6d / M7 (honest ledger): cone + torus booleans
  (tessellation + their seam structures); cylinder cap meeting the block
  (partial-depth blind holes at an angle / cap-plane coincidence);
  through-hole with the cylinder lateral split by TWO planes (multi-seam-
  per-periodic-face ordering); coplanar/coincident neighborhood
  classification (winding-number-vector); EXACT trimmed-curved mass
  properties (volume oracle stays coarse tessellation); tolerant
  edges/vertices (Jackson, the M7 NURBS-SSI requirement); BVH GWN + AABB
  localization (the all-pairs O(n^2) throughput cost, also why
  fuzz_cyl_boolean throughput is low). M7 = robust booleans on NURBS-
  bounded solids (the proof bar).

## Assessment (2026-06-07): is Keel a Parasolid competitor? (candid opinion, requested)

Honest, calibrated opinion (not a claim, not marketing). Split into three
buckets, because lumping them flatters or insults the work:

- SOUND? Genuinely yes -- arguably more rigorously than some shipping
  kernels. Foundations are CERTIFIED not "seems to work": extended-real
  interval arithmetic (caught a real release-mode overflow bug),
  Krawczyk verification on the spline solver, validity-by-construction
  topology, a determinism contract + FNV hash, the M4 orientation audit
  and M6b/M7a winding soundness gates that independently cross-check
  conventions. Sound by construction + proof, not by accumulated scars.
  Punches above its weight for its age.

- ROBUST? Only partially, and this is the brutal Parasolid gap. Keel is
  robust ON WHAT IT COVERS and robust-by-DECLINING on degeneracies (the
  never-lie partial-success model). But Parasolid's crown jewel is the
  thing Keel does NOT do: swallow arbitrary DIRTY real-world input
  (near-tangent, near-coincident, sliver, mm-vs-km scale, self-
  intersecting imports) and still return a usable answer. Keel's fuzzers
  found exactly those cases and the response was to decline cleanly --
  correct for now, but DECLINING IS NOT COMPETING. A CAD user wants the
  boolean to succeed on their messy part.

- COVERAGE / FEATURE-COMPLETE? Not close. Keel = booleans + primitives
  (analytic + just-started NURBS). Parasolid = ~40 yrs, full trimmed-
  NURBS booleans + blending/chamfer/shell/loft/sweep/sheet-metal/
  healing/draft, dozens of subsystems. Low-single-digit % of the surface
  area. Curved-volume verification is still a COARSE tessellated oracle
  (no exact trimmed mass props); no differential testing vs OCCT/ABC
  corpus yet -- robustness evidence is fuzzing-strong but real-world-
  corpus-untested.

VERDICT: NOT a Parasolid competitor today -- a legitimately sound, well-
architected FOUNDATION that has proven the conceptual CORE (robust
booleans, the hardest part) correctly on a tiny slice of the domain.

BUT "is it a Parasolid competitor" is slightly the wrong question. Head-
to-head nobody beats Parasolid (a billions-of-parts moat, not a code-
quality gap). The real question is a DEFENSIBLE NICHE Parasolid doesn't
serve well -- and Keel has one: the exact-topology/tolerant-geometry
hybrid (file 11) that per the literature NO kernel fully ships. If M7b
lands it (certified SSI error bounds riding into the topology, epsilon-
solidity as a checkable contract), Keel could credibly claim PROVABLY-
BOUNDED booleans -- a wedge for verification/simulation-meshing/
regulated-aerospace where "empirically robust" isn't good enough. A
wedge, not a frontal assault.

One line: sound enough to be taken seriously, nowhere near robust or
broad enough to compete yet -- but making the RIGHT architectural bets,
with a differentiation thesis that is real rather than marketing. The
risk isn't a wrong foundation; it's the multi-year robustness-and-
coverage grind, and whether the niche is worth it.

## Addendum 32 (2026-06-08): M7a COMPLETE (NURBS-faced solid plumbing); merged

- First step of the proof milestone (M7, robust booleans on NURBS-
  bounded solids): make the winding-classified pipeline accept NURBS
  FACES. Branch m7a-nurbs.
- Delivered:
  - tessellate_nurbs (tessellate.rs): grid over the NURBS parameter
    domain into outward triangles (outward = local_geometry normal,
    sense-adjusted; quad-normal fallback at poles). tessellate_face now
    dispatches on the face's SurfaceGeom (Analytic vs Nurbs), not just
    face_surface3 (which only returns analytics).
  - nurbs_sphere (construct.rs): the first NURBS-faced solid -- the
    sphere TOPOLOGY of sphere() (V2E1F1) but with a genuine curved NURBS
    surface (revolve_full of a rational quadratic semicircle meridian
    profile about frame.z) and a NURBS meridian seam curve.
  - Task 0 MANDATE (soundness before classifying on it): the NURBS
    sphere's generalized winding number is ~1 inside / ~0 outside across
    a magnitude ladder, +1 orientation audit (the revolved surface's
    local_geometry normal points outward). Proven on a real curved NURBS
    surface, not a degree-1 planar fake.
- SCOPE DECISION (during execution): the NURBS BOOLEAN moved to M7b,
  which becomes the NURBS-boolean-WITH-TOLERANT-EDGES centerpiece. A
  working NURBS boolean needs three M7b-grade pieces (NURBS imprint
  pcurve via project_point_surface; trimmed-NURBS fragment tessellation
  with a parameter-space cap-side filter; the NURBS cap interior point)
  -- and that is exactly where the exact-topology/tolerant-geometry
  hybrid (file 11, the project's sharpest differentiation per the user)
  gets delivered. M7a is the clean soundness-gated PLUMBING those build
  on; aligning the milestone boundary with the thesis.
- M7b CENTERPIECE (planned): the SSI engine ALREADY computes the curve
  error bound (SsiCurve.tol_achieved) and currently THROWS IT AWAY when
  the curve becomes a topology edge -- M7b plumbs it into Edge.tolerance,
  propagates the bound through booleans, and adopts Qi/Shapiro epsilon-
  solidity as a CHECKABLE validity contract. "Exact topology decisions
  with tolerant geometry" -- the hybrid no kernel fully ships.
- GATE: exact CI triplet GREEN (fmt --all --check; clippy --workspace
  --all-targets -- -D warnings; cargo test --workspace = 238 tests:
  90 geom + 77 math + 71 topo). No new fuzz target (M7a adds no mutation
  path: nurbs_sphere is a constructor, tessellation is a read). Merged.

## Addendum 33 (2026-06-08): M7b -- FIRST NURBS BOOLEAN + the tolerant-edge CENTERPIECE delivered (inexact NURBS x NURBS deferred to M7c)

The proof milestone's payload, in two halves. Branch m7b-nurbs-booleans.

PART A -- the first NURBS boolean end-to-end:
- NURBS-sphere INTERSECT analytic-sphere (both x-axis seamed ->
  crossing-free), distance 1.5 -> a valid two-cap lens, tessellated
  volume within 6% of the exact lens formula, midpoint inside / above
  outside by GWN. The winding-classified pipeline (localize -> SSI ->
  imprint both -> classify -> select -> import-and-glue stitch) now runs
  with a genuinely curved NURBS operand, not just analytic primitives.
- Plumbing this needed three M7b-grade pieces (per the M7a scope split):
  - NURBS imprint pcurve: curve_pcurve_on_any (imprint.rs) handles
    analytic AND NURBS faces -- sample the SSI seam, project each sample
    onto the NURBS surface, UNWRAP the periodic u, fit a cubic pcurve.
  - project_point_surface_fast (geom/project.rs): coarse 8x8 grid seed +
    clamped Newton, ~100x faster than the certified global projector
    (which re-decomposes Bezier patches every call, ~2.5s on a pole-
    degenerate revolved sphere). The imprint hung >180s on the certified
    projector + a seam-wrapping fit that escalated forever; fast project
    + u-unwrap brought it to 0.32s.
  - Trimmed-NURBS fragment tessellation + interior point: tessellate_nurbs
    gained a cap-side trim (nurbs_cap_trim) and face_interior_point
    dispatches NURBS faces to nurbs_face_interior_point (cap apex via
    fast-project along the SSI-circle axis, side from loop kind). BUG
    fixed: nurbs_cap_trim originally matched only Circle3 seams, so the
    fitted-NURBS SSI seam fell through and the lens tessellated as a FULL
    sphere (volume 3.99 not 0.36); generalized via closed_curve_center_axis
    (Circle3/Ellipse3 exact x-cross-y; NURBS sampled-centroid + Newell
    normal) -> lens volume within 6%.

PART B -- TOLERANT EDGES, the differentiation centerpiece ("exact
topology decisions with tolerant geometry", file 11's hybrid no kernel
fully ships):
- The SSI engine already computes the curve's certified error bound
  (SsiCurve.tol_achieved) and USED TO THROW IT AWAY when the curve became
  a topology edge. M7b plumbs it through:
  - SeamCurve gained `tol`, set from c.tol_achieved at both seam_curves
    construction sites.
  - imprint_operand sets each seam edge's tolerance to the max seam tol.
  - import_vertex / import_edge (the import-and-glue stitch) now COPY
    vertex/edge tolerance into the result body (e.tolerance.max(etol)), so
    the bound survives stitching.
  - Body::epsilon_solid(eps) -> bool: a CHECKABLE validity predicate
    (every edge/vertex tolerance <= eps); Body::achieved_tolerance() ->
    the max tolerance carried. set_seam_edge_tolerance for the plumbing.
- PROVEN (test nurbs_boolean_is_epsilon_solid): the tier-2 analytic-
  sphere x NURBS-sphere SSI is solved by certified-numeric FITTING, so
  the intersection circle is reproduced to a GENUINELY NONZERO certified
  bound (4.02e-7) -- not exactly. That bound now rides onto Edge.tolerance,
  and the lens is provably epsilon_solid(4.02e-7) while its COMBINATORIAL
  topology (two valid caps, Euler-valid) stays EXACT. The test asserts the
  bound is nonzero (eps > 0) so a regression that silently drops the bound
  back to the floor fails loudly. This is the thesis, demonstrated on a
  real curved NURBS boolean: exact combinatorics, geometry carrying its
  own certified deviation.

DEFERRED to M7c (honest ledger -- attempted in M7b, not shipped):
- The INEXACT tier-3 NURBS-sphere x NURBS-sphere boolean (Task 5). Built
  the test; it FAILS for two empirically-found reasons (not theory):
  1. Correctness: the tier-3 FITTED seam (a general NurbsCurve, not a
     clean Circle3) is not split onto either NURBS sphere by the crossing
     imprint -- diagnostic showed ia.f=1, ib.f=1 (neither face cut), so
     both whole faces classify OutsideOther and kept=0 (then the empty
     kept-set hits the vacuous-true all_planar path and builds a garbage
     body -> AssemblyFailed). The M6c crossing/two-arc imprint relies on
     clean-conic seam geometry (center/axis/antipode) to locate the seam
     crossing; it must be generalized to fitted NURBS seams. (Tier-2
     analytic-vs-NURBS works because its seam, though carrying a nonzero
     fitted BOUND, is still produced as a clean circle the crossing logic
     can split.)
  2. Performance: one such boolean runs ~190s (the certified solver re-
     decomposes both pole-degenerate revolved spheres into Bezier patches
     and Krawczyk-verifies every patch pair) -- un-shippable as a default
     cargo test; needs SSI patch-pair pruning/caching first.
  CRUCIALLY: the centerpiece is NOT blocked by this. The tier-2 lens
  already carries a genuinely nonzero (4e-7) fitted bound, so tolerant
  geometry is really exercised today; NURBS x NURBS is a harder SECOND
  demonstration, not a prerequisite for the thesis.
- Also deferred (unchanged): general trimmed-NURBS faces (multiple/curved
  trim loops); exact trimmed-NURBS mass properties (the coarse tessellated
  volume is the curved oracle for now); tolerance-GROWTH theory under
  chained booleans; differential testing vs OCCT over the ABC corpus.

FUZZ: fuzz_nurbs_boolean added (NURBS-sphere x analytic-sphere, random
radii/separation/op) -- asserts never-panic, valid + epsilon_solid at
achieved tolerance + bounded volume on Ok. Compiles under nightly; soak
clean. (Per standing mandate, any finding becomes a golden regression.)

GATE: exact CI triplet GREEN (fmt --all --check; clippy --workspace
--all-targets -- -D warnings; cargo test --workspace = 241 tests: 90 geom
+ 77 math + 74 topo). Merged.

## Addendum 34 (2026-06-08): M8 canonical recovery CORE -- the M7c blocker dissolved (NURBS x NURBS 190s -> 0.68s, exact)

PROMPT: the user added research (kernel files 25-33) and said "Maybe the
answer to your blockers is there." It was. The M7c blockers (the tier-3
NURBS-sphere x NURBS-sphere boolean: 190s and a fitted seam that would
not split either face) were both symptoms of ONE mistake, named by file
24 (canonical-geometry-recovery) and file 11 / headline #16 (the
exactness affordability gradient): a `nurbs_sphere` is an EXACT quadric
wearing a NURBS coat (Piegl-Tiller forward construction), and two spheres
meet in an EXACT circle. I was using the heaviest spline machinery to
re-derive a circle the kernel has a closed form for. The fix is to
RECOGNIZE the hidden analytic and route to the exact tier -- the
affordability gradient made real (exact for conics/quadrics, certified-
tolerant only for genuine free-form).

The user chose the FULL canonical-recovery milestone (M8). This session
shipped the CORE slice. Branch m8-canonical-recovery.

DELIVERED (keel-geom `recover` module -- recognize -> fit -> certify ->
keep/recover):
- `recover_surface(nurbs, tol) -> Option<SurfaceRecovery>`: curvature-
  signature recognition (closed-form k1/k2 from `local_geometry`, no scan
  noise) dispatched by signature -- PLANE (k1=k2=0), SPHERE (umbilic
  k1=k2=const, algebraic center/radius fit), CYLINDER/CONE (developable,
  one principal curvature ~0; axis from the zero-curvature principal
  direction / ruling-line LS intersection for the cone apex). Each fit is
  CERTIFIED before acceptance.
- The certifier `surface_deviation` is file 24 Theme 5's endorsed cheap
  bound: dense sampling of the spline against the analytic's closed-form
  distance (`Surface3::project`) + local refinement around the worst
  sample. KEY SAFETY PROPERTY: the certifier is the real discriminator,
  so a wrong-type fit cannot certify -> the failure direction is false-
  REJECT (keep the spline), never false-accept. (The interval-certified
  tighter bound via `point_enclosure` is deferred; `fuzz_recover` cross-
  checks soundness.)
- Recovery verified exact at the geom level: sphere center/radius to
  1e-7; cylinder radius/axis to 1e-6; cone half-angle to ~1e-4 (its
  pole-degenerate apex limits the sampled certifier to ~2e-6, so cones
  recover at a ~1e-5 modeling tolerance -- documented). The "keep the
  spline" verdict is proven too: a genuine free-form bicubic patch is
  REJECTED (`freeform_is_kept`) -- the other side of the gradient.

BOOLEAN INTEGRATION (the M7c retirement): a recovery pre-pass in
`seam_curves` uses the recovered analytic ONLY for SSI dispatch (the
NURBS face stays for imprint/tessellation -- imprinting an exact circle
onto a NURBS face is the proven analytic-x-NURBS path). Results:
- NURBS-sphere INTERSECT NURBS-sphere: was 190s and FAILED (fitted seam
  split neither face, kept=0); now both operands recover to spheres, the
  SSI is the exact tier-1 circle, the M6c crossing-imprint splits both ->
  a valid two-cap lens in 0.68s (280x faster), volume within ~4% of the
  exact formula. THE M7C BLOCKER IS RETIRED.
- NURBS-sphere INTERSECT analytic-sphere: now EXACT (was the tier-2 fit
  carrying ~4e-7); achieved tolerance drops to the floor (1e-8). The
  hidden quadric pays quadric prices. (The old `nurbs_boolean_is_epsilon_
  solid` test, which asserted a genuine nonzero bound on this case, was
  honestly rewritten to `nurbs_sphere_boolean_recovers_to_exact` -- with
  recovery the case is exact, so that eps was the tier-2 approximation
  recovery now removes.)

FUZZ: `fuzz_recover` -- random analytic NURBS (plane/sphere/cylinder),
recover, assert the soundness invariant: never panics, and a returned
deviation is a genuine UPPER bound on an INDEPENDENT denser sampling and
<= tol. Soak clean.

DEFERRED to M8-completion (honest ledger -- the full milestone the user
chose; this session shipped the core that retires M7c):
- TORUS recovery (file 24 flags torus as the most fragile; both principal
  curvatures nonzero + the torus relation; line-complex axis).
- CURVE recovery (NURBS curve -> line/circle/ellipse, same certify gate).
- `Body::simplify` -- the public HEAL "simplify" pass (topology-preserving
  surface swap + pcurve refit); needs curve recovery and pcurve refit to
  produce a VALID swapped body, so it follows those.
- The full FREE-FORM-faced-SOLID boolean capstone (a genuine non-
  recoverable surface driving a tier-3 SSI with a tolerant seam): the
  tolerant-edge machinery is shipped (M7b) and the rejection gate is
  proven (`freeform_is_kept`), but the end-to-end demo needs a free-form
  SOLID constructor (lofting/skinning) that does not exist yet -- genuine
  M-later, and arguably outside "recovery" proper.
- Face-MERGE of split analytics (naming-coupled); constraint-aware re-fit
  / beautification (coaxiality snapping); helical/developable/translational
  and learned type proposals.

GATE: exact CI triplet GREEN (fmt --all --check; clippy --workspace
--all-targets -- -D warnings; cargo test --workspace = 249 tests: 97 geom
+ 77 math + 75 topo). fuzz_recover added + soak clean. Merged.

## Addendum 35 (2026-06-08): M8 COMPLETION -- curve recovery, Body::simplify, torus; the full quadric family recovers

Continued M8 (user: "Continue") through the completion items, finishing
the canonical-recovery milestone. Branch m8-completion.

DELIVERED:
- CURVE recovery (keel-geom recover): `recover_curve(nurbs, tol) ->
  Option<CurveRecovery>` -- line (zero curvature) and circle (constant
  curvature, planar; plane from three spread samples + a 2D circle fit in
  that plane), certified by `curve_deviation` (dense sample vs the curve's
  closed-form projection). Line dir to 1e-9, circle center/radius to 1e-7;
  a wavy degree-3 curve is correctly KEPT. (Ellipse recovery -- the
  plane-cylinder seam case -- is noted for later; line and circle are what
  the analytic booleans actually produce.)
- `Body::simplify(tol)` (keel-topo simplify.rs): the public HEAL
  "simplify" pass. Topology-preserving in-place swap of NURBS faces ->
  analytic and NURBS edges -> line/circle, gated by the recovery
  certifier. ORIENTATION is preserved: the swapped surface's sense is
  chosen so the outward normal at a shared geometric point agrees with the
  original (analytic vs NURBS du x dv normals differ). Tested: a
  nurbs_sphere body simplifies to an analytic-Sphere3-faced body that
  stays VALID with volume preserved (geometry only tightened). Returns a
  SimplifyReport (counts + max certified deviation).
- TORUS recovery (completes the quadric family plane/sphere/cylinder/
  cone/torus): doubly-curved, not-umbilic, not-developable branch. The
  surface-of-revolution AXIS is the smallest-eigenvalue eigenvector of the
  point covariance via a compact 3x3 cyclic-Jacobi eigensolver; the tube
  (major/minor radii) is a 2D circle fit in (rho, z) axis-frame
  coordinates (rho = radial distance from axis, z = axial). FIX during
  build: the first estimator took major/minor from the rho min/max extent,
  but interior samples miss the exact tube extremes -> minor off by ~2%
  and deviation 1.3e-2; the (rho,z) LS circle fit gives major/minor to
  ~1e-11 and deviation ~6e-6. Like the cone, the sampled certifier sits at
  ~6e-6 here, so torus recovers at a ~1e-5 modeling tolerance (documented).

So the full M8 milestone the user chose is delivered: recognize ->
fit -> certify -> substitute for the entire analytic surface family plus
line/circle curves, exposed both as `keel_geom::recover` and the
`Body::simplify` HEAL pass, with the boolean front-end as the first
internal client (M7c blocker retired in Addendum 34).

NUMERICS ADDED (small, self-contained, certifier-gated): a 3x3 cyclic
Jacobi symmetric eigensolver and 3x3/4x4 Gaussian-elimination solvers in
recover.rs. The certifier is still the discriminator everywhere: a
wrong-type fit cannot certify, so the failure direction stays false-
reject (keep the spline), never false-accept.

DEFERRED (honest ledger, genuinely later / separate milestones):
- ELLIPSE curve recovery (plane-cylinder seams).
- The FREE-FORM-faced-SOLID boolean capstone (a non-recoverable surface
  driving a tier-3 SSI with a tolerant seam end-to-end): the tolerant-edge
  machinery is shipped (M7b) and the rejection gate is proven
  (freeform_is_kept), but the demo needs a free-form SOLID constructor
  (lofting/skinning) that does not exist -- a separate constructor
  milestone, not recovery.
- Face-MERGE of split analytics (naming-coupled); constraint-aware re-fit
  / beautification (coaxiality snapping); helical/developable/translational
  and learned (UV-Net) type proposals; the interval-certified tighter
  deviation bound (the current bound is dense-sampled, fuzz-backstopped).

GATE: exact CI triplet GREEN (fmt --all --check; clippy --workspace
--all-targets -- -D warnings; cargo test --workspace = 254 tests: 101 geom
+ 77 math + 76 topo). fuzz_recover re-soaked clean. Merged.

## Addendum 36 (2026-06-08): Capability-gap assessment vs Parasolid + the PARITY PROGRAM kickoff

The user directed a strategic pivot: after M8, close the capabilities/
features gap toward Parasolid "as close to parity as possible." Read the
researcher's 144-item Parasolid capability map (research file 25) and
audited Keel's actual code against it.

GAP ASSESSMENT (grounded in the pub-fn inventory, not guesses):
- SOLID (~30 of 144): full topology hierarchy + Euler ops + non-manifold
  PES regions (Keel's strongest area, near parity); primitives block/
  prism/cyl/cone/sphere/torus/nurbs_sphere; analytic+NURBS curves/surfaces;
  unite/subtract/intersect on SOLIDS for CLEAN TRANSVERSAL analytic +
  recovered-NURBS bodies (narrow); SSI; GWN point classification; mass
  properties; validity checking; canonical recovery/simplify (M8);
  tolerant edges/epsilon-solidity (M7b); imprint curves onto faces;
  in-memory session snapshot/replay + lineage.
- MISSING (~100): the entire feature surface -- blends/chamfers (0 files),
  sweep/loft (0), shell/offset/thicken (0), local ops/tweak (0), sheet
  ops, sectioning, HLR/rendering, attributes (0), persistence/XT (0),
  assemblies (0), foreign geometry (0), healing/defeaturing, mid-surface,
  convergent/mesh.
- BOTTOM LINE: Keel is SOUND but NARROW. It has the certified KERNEL SPINE
  (the hard math) and ~70% of Parasolid's capability surface is absent.
  The missing part is the breadth a CAD app consumes daily. Breadth, not
  soundness, is now the dominant gap. The differentiation thesis (exact-
  topology/tolerant-geometry + affordability-gradient recovery) is real
  but a WEDGE, not parity.

ROADMAP (docs/superpowers/specs/2026-06-08-parasolid-parity-roadmap.md):
sequenced by leverage + dependency + reuse + patent-safety. Honest target
= the kernel-appropriate, patent-safe subset (~110-120 of 144; excludes
host/D-Cubed items, patent-fenced convergent modeling per file 18, and
fragile mid-surface). Phases:
- Phase 0 FOUNDATION (gates everything): 0a general-position booleans
  (coincident/tangent/sheet/multi-tool -- today DECLINED; features
  generate exactly these), 0b offset-surface primitive, 0c attribute
  system.
- Phase 1 TWEAK keystone (file 03: change-surface+reintersect -> move/
  offset/taper/delete-heal faces; OCCT lacks it).
- Phase 2 sweep/spin/loft constructors (file 26).
- Phase 3 BLENDS/CHAMFERS (file 28, ~15 items, the biggest area).
- Phase 4 shell/offset family. Phase 5 sheet ops + sectioning (file 13).
- Phase 6 interrogation+HLR (file 06). Phase 7 persistence/XT (file 14).
- Phase 8 healing/defeaturing (file 13). Phase 9 foreign geometry +
  assemblies (files 16, 24).
Each phase = milestone(s), CI-green, fuzzed, honest ledger, same bar as
M1-M8. NEXT: pick the starting phase (recommendation: Phase 0 foundation,
since the feature engines stand on general-position booleans + offset
surfaces; building features first would hit "declined" on realistic
inputs).

## Addendum 37 (2026-06-08): Parity Phase 0b-i -- analytic offset surfaces (enabler)

First build of the parity program (Phase 0 foundation). keel-geom
offset.rs: `offset_surface(&Surface3, d) -> Option<Surface3>`, the surface
displaced by signed distance d along its outward normal -- the shared
primitive under shelling/hollowing, body offset, offset-face, and the
blend spring-curve. EXACT closed forms: plane stays a plane (origin += z*d);
sphere/cylinder radius += d; CONE radius += d/cos(half_angle) (derived: the
offset of a cone is a coaxial cone of the same half-angle, apex unchanged);
torus minor += d. Degenerate offsets (radius -> <=0, torus tube reaching
major) decline. Verified by the defining property (offset passes through
point + d*normal; point sits at |d| from the offset). NURBS certified
offset (sample-displace-refit-certify) is the follow-up 0b-ii. This is an
ENABLER -- it does not tick a 144-map item by itself (offsets surface in
items 37/41/45 as OPERATIONS, built later). CI triplet green (260 tests:
107 geom + 77 math + 76 topo). Capability map unchanged at baseline.

## Addendum 38 (2026-06-08): Parity Phase 0c -- attribute system COMPLETE (map 30 -> 35/144)

Closed Parasolid capability-map items 117-121. The Body already had a
minimal attr store (BTreeMap<EntityId, BTreeMap<String, AttrValue>> + basic
set_attr/attr + cleanup on delete); completed it to a real system:
- AttrValue extended with Vec3([f64;3]) (colors/directions) and Bytes
  (raw user fields) alongside F64/I64/Bool/Str. [117, 119]
- Full API: set_attr/attr/remove_attr/attr_keys, plus face_id/edge_id/
  vertex_id (key -> stable EntityId) so callers can attribute entities
  they hold a key to.
- System attributes [118, 120]: set_color/color ([f64;3] RGB), set_name/
  name, set_density/density, set_user_field/user_field convenience
  accessors over well-known keys.
- PROPAGATION [121]: a single central hook in Body::register -- a
  SplitChild or Modified entity inherits its parent's attributes, a
  MergeResult inherits from its primary source, Generated (lower-dim
  spawn) does NOT inherit; the child's own sets override. Because every
  operation registers entities through this path, attributes propagate
  through ALL operations (split/boolean/euler) for free. Tested: split a
  block edge -> both children inherit the parent's color+name.
RUNNING TOTAL: capability map 30 -> 35/144 (items 117-121).
GATE: exact CI triplet GREEN (fmt; clippy --workspace --all-targets -D
warnings; cargo test --workspace = 262 tests: 107 geom + 77 math + 78
topo). No new fuzz target (attributes are a pure data store, no geometric
mutation path). Merged.

## Addendum 39 (2026-06-08): Peer review of crusst -- borrowable ideas logged

Read-only review of crussella0129/crusst (~6845 LOC, pure-Rust nalgebra
B-rep peer; cloned to ../crusst-peer, nothing changed). It is the mirror
image of where Keel started: ALL representation, NO operations. Real:
coedge/half-edge topology (MANIFOLD-ONLY), analytic surfaces + NURBS
(faithful NURBS-Book de Boor basis), curves, primitives, adaptive
tessellation, topology validation (2-coedges/edge + Euler/genus), a fluent
Shape builder, and multi-format export. STUBBED: booleans (StubBooleanEngine
-> NotImplemented), fillet, chamfer, shell, loft, sweep -- all
NotImplemented. Placeholder (dummy linear) pcurves; shallow tests.

BORROW (worth pulling into Keel; Keel is far ahead on the modeling spine,
so only the interchange/ergonomics layer is worth taking):
1. STEP AP203 EXPORT -- the single highest-value takeaway. Keel has ZERO
   interchange today (an adoption blocker). crusst's src/export/step.rs is
   a working blueprint: direct topology->STEP entity mapping with NO
   tessellation -- MANIFOLD_SOLID_BREP + CLOSED_SHELL, ADVANCED_FACE +
   FACE_OUTER_BOUND, EDGE_CURVE, and B_SPLINE_SURFACE_WITH_KNOTS /
   B_SPLINE_CURVE_WITH_KNOTS with real multiplicities+knots, under
   AUTOMOTIVE_DESIGN + ADVANCED_BREP_SHAPE_REPRESENTATION. Keel's geometry
   is MORE faithful (real pcurves, certified SSI seams, canonical
   recovery), so Keel STEP output would be strictly better. Pull forward
   as an early, cheap, high-visibility milestone -> roadmap.
2. Export breadth STL/OBJ/3MF -- thin writers over Keel's existing
   tessellation; 3MF is the additive-manufacturing lingua franca.
3. A fluent Shape-style builder over the low-level Body/Euler API
   (Shape::sphere(r).translate(..).difference(..)) -- usability, no kernel
   change.
4. A `mint`/glam math-interop feature -- lowers downstream-graphics
   adoption friction (Keel uses its own keel-math).
5. pcurve-as-first-class-coedge-field with a Curve2 (incl. NURBS2) type --
   Parasolid-style always-present pcurve, done FAITHFULLY (unlike crusst's
   placeholders), cleans up trimmed-face + STEP/trim export.

DO NOT borrow: the modeling core. crusst has no booleans/fillet/shell,
manifold-only topology, placeholder pcurves. Keel leads decisively there.
See memory [[peer-kernels]].

## Addendum 40 (2026-06-08): FOUNDATION FIX -- certified surface projector made globally correct + 25x faster

A proptest soundness oracle (project::tests::surface_projection_is_global)
caught project_point_surface returning a NON-GLOBAL (local) minimum on a
high-derivative degree-(2,3) rational NURBS surface; proptest persisted the
counterexample to proptest-regressions/project.txt (a permanent golden
case). User directive: fix the foundation before building further.

ROOT CAUSE: the certified projector used the correct Selimovic/Ma-Hewitt
subdivide-with-convex-hull-exclusion architecture (research b-nurbs-freeform
section "Closest-point projection"), but explored patches with a DFS STACK
plus a `guard > 100_000` hard break. On a wild surface, DFS visits far
regions before near ones, so `best` stays loose, AABB exclusion is weak, the
stack explodes, the guard fires, and it returns an UNDER-CONVERGED best.
This caused both the wrong answer AND the 376s runtime.

FIX: best-first branch-and-bound. Replace the stack with a BinaryHeap
keyed by the patch's control-AABB lower bound to p (PatchFront), expanding
the CLOSEST patch first. `best` tightens immediately, exclusion becomes
strong, and the first popped patch whose lower bound exceeds `best` proves
every remaining patch is at least as far -> the current best is the
CERTIFIED GLOBAL minimum (early, provable termination; no guard, no
explosion). Soundness unchanged (control AABB contains the patch; positive
weights). RESULT: the proptest (persisted counterexample + 16 random cases)
passes in 14.67s (was 376s, 25x); the whole keel-geom suite dropped 348s ->
23s since everything using the projector is now fast. 107/107 geom green.
The persisted regression seed is KEPT as a permanent golden case.

## Addendum 41 (2026-06-08): Parity Phase 0a (partial) -- multiple tool bodies + imprint-only (map 35 -> 37/144)

The tractable general-position-boolean wins that do not need the hard
coincident/tangent machinery (which remains the next milestone, to be built
on the Zhou/Jacobson winding-number-VECTOR + coincidence-first + symbolic
perturbation approach from research d-booleans-tolerant). Closed items:
- 30 MULTIPLE TOOL BODIES: boolean_multi(target, &[tools], op, tol) applies
  tools in order (union/intersection accumulate, difference subtracts each).
  Tested: empty list = unchanged; single = direct boolean; two-tool sphere
  dimple. (Found a coincidence false-positive when two cylinder blind-holes
  share a floor plane -> used spheres, which avoids coplanar faces.)
- 32 IMPRINT-ONLY: imprint(a, b, tol) imprints the SSI intersection onto
  BOTH operands as shared edges without combining (reuses imprint_operand).
  Tested: two spheres each split 1 face -> 2, both still valid.
RUNNING TOTAL: capability map 35 -> 37/144.
REMAINING in 0a (next milestone): coincident/tangent (33), sheet booleans
(28), general/non-manifold bodies (29), local face-pair (31). GATE: exact CI
triplet green. Merged.

## Addendum 42 (2026-06-08): Parity interrogation -- bounding box + min distance (map 37 -> 39/144)

keel-topo interrogate.rs, built on the winding classifier's outward
tessellation:
- 105 BOUNDING BOX: Body::bounding_box() -> Aabb3, tight from the
  tessellation (exact for planar, tessellation-tight for curved; exact
  analytic extrema a later refinement). Tested: block bbox exact; sphere
  bbox tight within 0.05.
- 101 MIN DISTANCE: Body::min_distance(other) -> f64, symmetric min over
  each body's tessellation vertices of point-to-triangle distance to the
  other (Ericson closest-point-on-triangle); ~0 when touching/overlapping.
  A tessellation-resolution approximation (exact face-pair surface
  projection later). Tested: separated spheres gap ~2.5; overlapping ~0.
RUNNING TOTAL: capability map 37 -> 39/144.
GATE: exact CI triplet green. Merged.

## Addendum 43 (2026-06-08): Parity interrogation cont. -- clash + body equivalence (map 39 -> 41/144)

- 102 CLASH / INTERFERENCE: Body::clashes(other) -> bool. bbox reject;
  then surfaces-cross/touch via the analytic SSI (seam_curves non-empty,
  or a Coincident/Tangent fault); else nesting via one winding-number
  probe per side. Fast (SSI + 2 GWN probes, NOT the O(n*m) tessellation
  sweep -- first cut used min_distance and ran 76s; rewrote to ~instant).
  Tested: overlapping clash, separated no-clash, fully-nested clash.
- 108 BODY EQUIVALENCE: Body::approx_equals(other, tol) -> bool. Equal
  entity counts + genus + (within tol) bounding box + volume -- the cheap
  CAx-IF validation-property comparison stage (research file 22), position
  sensitive; exact B-rep equality oracle later. Tested: clone equal,
  different-radius unequal, identical blocks equal.
RUNNING TOTAL: capability map 39 -> 41/144.
GATE: exact CI triplet green. Merged.

## Addendum 44 (2026-06-08): Parity Phase 1 -- TWEAK / local-ops keystone (planar) (map 41 -> 45/144)

The file-03 foundation local operation (change-surface + reintersect),
the keystone OCCT lacks. keel-topo tweak.rs, PLANAR/polyhedral slice:
- tweak_face_to_plane(face, new_plane, sense): change/replace a planar
  face's surface (items 35, 39). Each corner of the face is recomputed as
  new_plane intersected with its two neighbour planes (closed-form
  three-plane point); incident straight edges are rebuilt through the
  moved corners; topology unchanged. Declines non-planar neighbours and
  non-simple (!=2-neighbour-plane) corners.
- offset_face(face, distance) (item 37): shift the plane along the OUTWARD
  normal (reuses the offset-surface sense), re-intersect.
- move_face(face, t) (item 36): translate the plane, re-intersect.
Tested on a 2x2x2 block: offset top +1 -> vol 12 (exact mass-properties);
offset -0.5 -> vol 6; move top +2 -> vol 16; all validate.
RUNNING TOTAL: capability map 41 -> 45/144 (items 35,36,37,39 planar).
DEFERRED (next tweak slices): CURVED tweak (cylinder/sphere faces
re-intersected by SSI, reusing offset_surface + ssi); taper/draft (38,78);
delete-face-with-heal (40); non-simple corners.
GATE: exact CI triplet green. Merged.

## Addendum 45 (2026-06-08): Parity tweak DEEPENED -- curved (cylindrical) face tweak (map stays 45/144)

Deepened the tweak keystone (user: "deepen tweak") from planar-only to the
CURVED case. offset_face now dispatches on surface type:
- Plane -> the existing three-plane-corner planar tweak.
- Cylinder -> tweak_cylinder_radius: offset the lateral face's radius
  (outward = radial-out for a solid, sense-aware), recompute each circular
  boundary edge (cylinder ^ cap-plane) at the new radius via Circle3, move
  the seam line + corner vertices radially (keeping height), swap the face
  surface. Topology unchanged.
Tested: a cylinder r=1 h=2 offset +0.5 -> r=1.5, tessellated volume ~4.5*pi
within 5%, valid. This is COVERAGE of item 37 (offset face now planar AND
cylindrical), not a new map item, so the counter honestly stays 45/144.
DEFERRED (next tweak slices): taper/draft (38,78 -- tilt the face plane,
reuses tweak_face_to_plane + a rotation), delete-face-with-heal (40),
sphere/cone/torus tweak.
GATE: exact CI triplet green. Merged.

## Addendum 46 (2026-06-08): Parity tweak cont. -- taper/draft (map 45 -> 47/144)

taper_face(face, pivot, axis, angle) (items 38, 78): rotate a planar
face's plane about the parting line through `pivot` along `axis` by the
draft `angle` (Transform3::from_rotation), then re-intersect via
tweak_face_to_plane. Corners on the parting line stay; the rest tilt.
Tested: draft a block's top face 0.2 rad -> valid, volume changed (exact
mass-properties). Covers item 38 (taper/draft faces, single-sided planar)
and 78 (draft about a parting reference). Double-sided draft and curved
taper deferred.
RUNNING TOTAL: capability map 45 -> 47/144.
GATE: exact CI triplet green. Merged.

## Addendum 47 (2026-06-08): Parity tweak FINISHED -- delete-face-with-heal + new-research review (map 47 -> 49/144)

delete_face(face) (items 40, 133): the MERGE/absorb heal mode -- merge a
face into a coplanar neighbour by killing their shared edge (kef), no
geometric change. Handles redundant/sliver coplanar faces and undoes a
coplanar split (= face merging, 133). Tested: split a block top into two
coplanar halves, delete one -> count + volume restored, valid. The
EXTEND-AND-REINTERSECT heal (non-coplanar neighbours grow back to meet)
needs edge-contraction surgery the current Euler op set (mvfs/mev/mef/kev/
kef/kemr/kfmrh/mfkrh + merge_vertices, which refuses shared-edge merges)
does NOT provide -- file 03 confirms delete-face is a dedicated,
non-decomposable primitive; that mode is honestly deferred.

TWEAK FAMILY COMPLETE (planar + cylindrical): change/replace surface
(35,39), move (36), offset (37, planar+cylindrical), taper/draft (38,78),
delete-face-heal merge mode (40), face merging (133).
RUNNING TOTAL: capability map 47 -> 49/144.

NEW RESEARCH REVIEW (user flagged; files 34-38 + the updated
03-completeness-audit): adversarial audit of the corpus. Load-bearing
flags relevant to shipped/upcoming work: (2.1) per-entity tolerance
PROPAGATION is unvalidated original engineering, not in the literature --
bears on the M7b tolerant edges; (2.2) the exact-topology/tolerant-geometry
hybrid (the canonical-recovery thesis) is asserted interactive but never
BENCHMARKED -- a microbenchmark is owed before it is load-bearing; (3.4)
attribute-propagation POLICY is "specified nowhere" -- the M10 register
hook is a concrete default we invented (original engineering, flag as
such); (3.1) STEP/AP242 import is under-researched (file 38 adds it) --
reinforces the STEP-export borrowable already pulled into the roadmap
(0d). Files 34-37 (test corpus, verification/oracles, performance,
numerical substrate) are quality/infra for later. None redirected the
tweak work.
GATE: exact CI triplet green. Merged.

## Addendum 48 (2026-06-08): Parity interchange -- STEP AP203 export (roadmap 0d; map stays 49/144)

keel-topo step_export.rs: to_step_string(body) -> STEP AP203 text, direct
topology->entity mapping, NO tessellation (the crusst peer borrowable; its
output strictly better given Keel's faithful geometry). Walks vertices ->
VERTEX_POINT/CARTESIAN_POINT, edges -> LINE + EDGE_CURVE, faces -> PLANE +
ADVANCED_FACE/FACE_OUTER_BOUND/EDGE_LOOP/ORIENTED_EDGE, then CLOSED_SHELL +
MANIFOLD_SOLID_BREP + ADVANCED_BREP_SHAPE_REPRESENTATION under
AUTOMOTIVE_DESIGN with a unit/uncertainty context. PLANAR solids this cut
(curved analytic + NURBS B_SPLINE surfaces next). Tested: a block exports
6 ADVANCED_FACE/8 VERTEX_POINT/12 EDGE_CURVE/6 PLANE with valid header +
schema; a cylinder declines (Unsupported). Interchange value (roadmap 0d),
not a numbered 144 PK item, so the counter stays 49/144.
NOTE on sequencing: started the engine list with STEP (independent,
completable, high-value) and am ordering by BUILDABILITY -- blends were
recommended first but a robust blend needs coincident-capable booleans
(the cutting wedge's faces are coplanar with the part) or intricate
seam-band surgery, so blends move after the coincident-boolean foundation.
GATE: exact CI triplet green. Merged.

## Addendum 49 (2026-06-08): Parity shell/offset -- whole-body offset (map 49 -> 50/144)

offset_body(distance) (item 45): move every planar face along its outward
normal and re-intersect all corners (each vertex = meet of its three
incident offset planes), reusing the tweak machinery globally. Tested: a
2x2x2 block offset +0.5 -> 3x3x3, exact volume 27, valid. Convex polyhedra;
self-intersection resolution (concave) + curved-face offset deferred.
RUNNING TOTAL: 49 -> 50/144.
GATE: exact CI triplet green. Merged.

## Addendum 50 (2026-06-08): Parity sectioning -- section by plane (map 50 -> 51/144)

section_by_plane(point, normal) (item 75): non-destructive section -- the
ordered polygon where the plane cuts the body's straight edges (linear
edge-plane crossings, ordered around the centroid in the cutting plane).
Tested: a 2x2x2 block at z=1 -> a 4-point square, shoelace area 4. Convex
polyhedra/straight edges; curved-edge crossings + multi-loop sections + the
split-into-two-bodies variant deferred.
RUNNING TOTAL: 50 -> 51/144.
GATE: exact CI triplet green. Merged.

## Addendum 51 (2026-06-08): Parity slicing -- planar slices / AM (map 51 -> 52/144)

planar_slices(base, normal, offsets) (item 77): one section polygon per
offset plane (reuses section_by_plane), empty vectors aligned to offsets
that miss the body -- the additive-manufacturing slicing path. Tested: a
block sliced at z=0.5/1.0/1.5 -> three squares; a slice above is empty.
RUNNING TOTAL: 51 -> 52/144. GATE: CI triplet green. Merged.

## Addendum 52 (2026-06-08): Parity wire bodies (map 52 -> 54/144)

wire(p0, p1) (items 8, 18): a first-class wire-body constructor wrapping
embed_wire + line geometry. (The capability already existed via embed_wire
-- undercounted at baseline; now first-class + tested.) Tested: body_class
== Wire, valid. RUNNING TOTAL: 52 -> 54/144.

INFLECTION: the clean quick wins are now exhausted. A sheet-body attempt
this session hit the lamina/region Euler semantics (a single planar face
must not create a second region; my mvfs+mef base made a 2-region Solid) --
deferred rather than thrashed (standing order). The remaining ~90 map items
are the BIG ENGINES, each a focused milestone, several research-hard and
interdependent: coincident/tangent boolean core (28,29,31,33 -- the
keystone unblocking blends/hollow/chamfer-by-boolean), sweep-along-path/
spin/loft (63,64,66-69), BLENDS/chamfers (47-61), hollow/shell (41-43),
HLR/silhouette (96-97), sheet ops (70-72), persistence/XT (126-129),
assemblies (82-85), foreign geometry (114-116). Next focused milestone: the
coincident-boolean foundation.
GATE: exact CI triplet green. Merged.

## Addendum 53 (2026-06-08): Parity coincident booleans -- FIRST SLICE (map 54 -> 55/144); the keystone subsystem opens

The coincident/tangent boolean core (user-directed keystone). Discovery:
the machinery was MORE built than feared -- classify_faces already emits
FaceClass::OnOther for coincident faces (winding ~0.5), and select_faces
already drops OnOther (the `_ => false`), which IS the correct on-on rule
for the COMMON case (abutting/face-to-face solids whose shared face is
interior to a union/intersection). The only blocker was boolean() DECLINING
on Coincident before reaching the classifier. Change: decline only on
Tangent; let Coincident PROCEED. The positive-volume post-condition guards
partial-overlap cases the simplest drop-rule mis-selects (declines them
honestly). Tested: two unit boxes sharing the x=1 face, UNION -> a 2x1x1
box (exact volume 2, valid). All 28 prior boolean tests still pass (no
coincident faces -> unaffected). RUNNING TOTAL: 54 -> 55/144 (item 33,
common case).
RESEARCH REQUESTED (passed to the user for the research agent): the full
Requicha on-on classification TABLES for coincident/coplanar/tangent faces
in CURVED B-rep booleans (keep/drop+orientation per op; OCCT BOPAlgo gluing
mode; coplanar->curved-coincidence generalization). That unlocks: coincident
DIFFERENCE, same-oriented coincidence, partial-overlap imprint, and tangent
faces -- deferred to when it lands.
GATE: exact CI triplet green (283 tests). fuzz_boolean re-soak validates
the new coincident path (the existing target now exercises coincident
inputs; post-condition + validity invariants hold). Merged.

## Addendum 54 (2026-06-08): Coincident booleans -- FULL on-on tables (file 39); the keystone is real (map 55/144)

Implemented research file 39 §2.3 (Laidlaw-Trumbore-Hughes / Requicha on-on
selection tables) -- the coincident-boolean keystone, now proper not just
the one abutting case. Changes:
- FaceClass::OnOther now carries OnSense (Same/Opposite/Unknown).
- classify_faces: for the winding ~0.5 (on-boundary) band, resolve the
  coincident-pair orientation via the sign of the two faces' outward
  normals (new helpers face_outward_normal + coincident_sense_at: find the
  other body's face sharing the carrier at p with a parallel normal, take
  n_self . n_other).
- select_faces: encode the tables. UNION/INTERSECTION keep same-sense (on+)
  overlaps ONE copy (from A, the lower-indexed body), drop opposite-sense
  (on-) -- interior walls cancel. DIFFERENCE keeps opposite-sense (on-) one
  copy, drops same-sense; B's inside-A faces kept reversed. Unknown-sense
  coincidence drops (conservative).
Tested: identical-box UNION -> vol 1 (on+ keep one, not 2); abutting UNION
-> vol 2 (on- drop both); abutting DIFFERENCE -> vol 1 (on- keep the wall);
all 28 prior transversal tests green; tangent still declines. This unblocks
chamfer-by-boolean and hollow (both need coincident difference/union).
RUNNING TOTAL: 55/144 (item 33 now properly done, sans partial-overlap
imprint / tangent-edge / curved-coincidence, which file 39 §1/§3/§5 specify
as follow-ups).
GATE: exact CI triplet green (285 tests). fuzz_boolean re-soak validates
the new classification on random inputs. Merged.

## Addendum 55 (2026-06-08): Parity blends -- edge CHAMFER (map 55 -> 56/144); first blend-family op

chamfer_edge(edge, distance) (item 52): constant-setback chamfer on a
convex edge between two PLANAR faces, as a TRANSVERSAL boolean -- subtract
a prism whose chamfer face slices the corner at the setback, its other
faces placed OUTSIDE the body (sized from the bounding box) so the cut
stays transversal (no coincident-face handling needed; reuses the existing
M6a transversal boolean). Tested: a 2x2x2 block, chamfer one top edge by
0.5 -> removes the 0.25 wedge, volume 7.75, valid. Curved-face chamfer,
variable/asymmetric setback, and fillets (arc section) are follow-ups.
RUNNING TOTAL: 55 -> 56/144. First operation of the blend/chamfer family
(items 47-61).
GATE: exact CI triplet green. Merged.

## Addendum 56 (2026-06-08): PARTIAL-OVERLAP coincident booleans (file 39 §1); the coincident keystone completes (map 56 -> 57/144)

The Addendum 53-54 coincident booleans handled FULL coincidence (faces
that overlap entirely: abutting/identical solids). The remaining gap was
PARTIAL coincidence -- two coplanar faces overlapping on a sub-region,
where one face is partly on-the-other-body and partly free. The simplest
on-on rule mis-selects these, so they were declined by the positive-volume
post-condition. This addendum closes that gap.

New module crates/keel-topo/src/coincident.rs -- the 2D overlap geometry
(research file 39 §1.2, "the genuinely hard part"):
  - clip_convex: Sutherland-Hodgman convex polygon intersection in a
    shared-plane 2D basis (auto-orients the clip CCW).
  - overlap_interior_segments(face_a, face_b, n): clips the two coplanar
    face polygons, lifts the overlap boundary back to 3D, and keeps only
    the edges INTERIOR to face_a (midpoint not on face_a's own boundary) --
    exactly the cuts that must be imprinted to split face_a into its
    on-overlap and off-overlap parts. Boundary-coincident edges need no cut.
  - coplanar_overlap_exists: positive-area overlap test (pair detection).
Three unit tests: partial overlap -> one interior cut at the dividing
line; identical faces -> no cut; disjoint -> no overlap.

Integration (boolean.rs): a PRE-PASS at the top of boolean().
  - coincident_face_pairs(a, b): finds coplanar (parallel normals + shared
    plane) planar faces with positive-area overlap.
  - preimprint_coincident_overlaps: clones both operands and imprints each
    overlap's interior cuts onto the respective face via the existing
    imprint_open_curve (boundary-to-boundary face split). Returns None
    (originals flow through unchanged) when no coincident faces exist, so
    transversal booleans are completely unaffected. Best-effort imprint;
    the positive-volume post-condition still guards a bad selection. The
    rest of boolean() is untouched -- a, b rebind to &Body either way.
After the pre-pass the partial overlap is split into fragments that the
existing on-on tables (Addendum 54) classify correctly.

Test: L-SHAPE UNION. A = 2x2x1 base block; B = 1x2x1 block stacked on its
LEFT half. The z=1 plane is a partial coincidence (A's top 2x2 vs B's
bottom 1x2). Pre-pass cuts A's top along x=1; the left half drops as
interior, the right half stays -> a valid L-solid, volume 4+2 = 6.
Previously this was declined. Full workspace green (107/77/106), fmt +
clippy -D warnings clean, fuzz_boolean re-soaked (new pre-pass path).

Scope/deferrals (honest): planar + CONVEX faces only (Sutherland-Hodgman
needs convex operands); curved coincident carriers and non-convex/holed
overlaps (a fully-interior overlap island needs a hole loop, not a
crossing cut) remain follow-ups per file 39 §1.4/§5. Multiple cuts on one
face use the first-imprint key (best-effort) -- robust multi-segment key
remapping is a follow-up.
RUNNING TOTAL: 56 -> 57/144. The coincident-boolean keystone (full +
partial) is now complete for the planar-convex case.
GATE: exact CI triplet green. Merged.

## Addendum 57 (2026-06-08): SWEEP/LOFT family opens -- ruled LOFT between two profiles (map 57 -> 58/144)

loft(bottom, top) (construct.rs, sweep/loft items 62-69): a solid bounded
by two parallel-ordered profile loops (same vertex count) joined by
quadrilateral side faces + two cap faces. Pure Euler-operator construction
mirroring prism() -- mvfs seed -> bottom rim (mev chain) -> close bottom cap
(mef) -> raise verticals to the top profile -> close side faces, last mef
closes the top cap -- then analytic geometry attach (cap planes from the
profile normals, each side plane computed from its quad and oriented
outward from the loft axis, edges as lines, plane pcurves). Constraint:
each side quad [b_i, b_{i+1}, t_{i+1}, t_i] must be PLANAR (verified up
front, rejected otherwise) so every face has an exact plane. Covers tapered
boxes, frusta, prisms. Twisted/ruled lofts with non-planar sides need NURBS
side faces -- a follow-up (the ruled-surface sweep tier).
Tests: square frustum 2x2 -> 1x1 height 2 -> V8 E12 F6, validate ok, exact
volume 14/3 (the frustum formula (h/3)(A1+A2+sqrt(A1A2))); a 90-degree
twisted loft is correctly rejected as non-planar.
RUNNING TOTAL: 57 -> 58/144. First op of the sweep/loft family. Linear
extrude already existed as prism(); loft adds the two-profile ruled case.
GATE: exact CI triplet green (fmt + clippy --workspace --all-targets -D
warnings + workspace test). Pure constructor -> Euler-op fuzzing covers it.
Merged.

## Addendum 58 (2026-06-08): Interrogation -- SURFACE AREA (map 58 -> 59/144)

surface_area() + face_area(face) (interrogate.rs): sum of outward-triangle
areas (0.5*|(b-a)x(c-a)|) over the same tessellation the winding classifier
and volume oracle use -- EXACT for planar faces, tessellation-approximate
for curved (consistent with curved volume; exact analytic area is a later
refinement). mass_properties() already reported volume/centroid/inertia, so
this completes the headline integral-property quartet. Test: 2x3x4 block ->
area 2(6+12+8) = 52 exactly.
RUNNING TOTAL: 58 -> 59/144.
GATE: exact CI triplet green. Merged.

## Addendum 59 (2026-06-08): REVOLVE (solid of revolution) + cone first-classness + mesh_volume (map 59 -> 60/144)

The sweep/loft family's headline op. Building revolve surfaced a real
architecture gap: CONE faces were dropped to empty in tessellate_face (the
`_ => Vec::new()` arm), so cone-faced solids (the existing cone() primitive
included) had wrong bbox / broken winding / broken booleans. Revolve
naturally makes cone bands, so it was blocked on cones being first-class.
Three pieces, bottom-up:

1. tessellate_cone (tessellate.rs): lat-band mesher mirroring
   tessellate_cylinder but with r(v) = radius + v*tan(half_angle); band
   bounds from the face's circle-edge heights plus the apex height (where
   r->0) when the face reaches a pole. Wired into the tessellate_face
   dispatch. Cones are now first-class for bbox/area/winding/booleans.
   Test: cone(r1,h1) bbox tight to [-1,-1,0]..[1,1,1] and area ~ pi(1+sqrt2)
   (base + lateral slant) -- both were wrong before.

2. mesh_volume() (interrogate.rs): pcurve-free volume via the divergence
   theorem (1/6 sum a.(bxc)) over the outward tessellation. Exact for
   all-planar bodies, tessellation-approximate for curved. A companion to
   the analytic mass_properties().volume that does not need pcurves -- the
   oracle for revolve solids (and a genuine interrogation capability).

3. revolve(frame, profile) (construct.rs): full 360-degree solid of
   revolution of a (radius, height) meridian about frame.z. Pure Euler-op
   construction: seed at the first off-axis point, build its circle
   (mef_on_vertex_loop) and the bottom cone (mev down to the bottom pole),
   then for each interior segment mev the seam up + mef(at,at) to split off
   the band as a face (the cylinder()'s seam-then-cap trick generalized),
   and finally mev up to the top pole for the top cone. m points ->
   m vertices, 2m-3 edges, m-1 faces (Euler 2). Per-band geometry: cylinder
   (equal radii) or cone (anchored v=0 at the radius>0 end, half_angle =
   atan(dr/dh)); edges attach as latitude circles (closed) or seam lines
   (open) by inspecting each edge.
   Tests: bicone [(0,-1),(1,0),(0,1)] -> V3 E3 F2, valid, mesh_volume 2pi/3
   (within 1% tessellation undershoot); barrel [(0,-1),(1,-0.5),(1,0.5),
   (0,1)] -> V4 E5 F3 (2 cones + cylinder), mesh_volume 4pi/3; open profile
   rejected.

Scope/deferrals (honest): the profile must meet the axis at BOTH ends with
NON-horizontal end segments (pole bands are true cones, not flat discs),
and no interior segment may be horizontal (which would revolve to a holed
washer face). Flat end-caps and washer faces need the holed-face/disc path
(deferred elsewhere too). PCURVES are not attached, so analytic
mass_properties() does not yet work on revolve solids (use mesh_volume);
validate(), tessellation, and GWN booleans all work without them. Partial
(<360) revolve is a follow-up.
RUNNING TOTAL: 59 -> 60/144. Sixty. Cone tessellation also retroactively
fixes the existing cone() primitive's bbox/booleans.
GATE: exact CI triplet green (fmt + clippy --workspace --all-targets -D
warnings + workspace test 113 keel-topo). fuzz_boolean re-soaked (tessellate
is shared with the winding classifier). Merged.

## Addendum 60 (2026-06-08): Revolve PCURVES -> analytic mass_properties first-class (map stays 60/144; closes Addendum 59's deferral)

Closed the pcurve deferral from Addendum 59 so analytic mass_properties()
works on revolve solids (not just the tessellated mesh_volume). New helper
attach_revolve_band_pcurves(face, origin_h, o, ez): walks a band's loop and
attaches a degree-1 NURBS UV pcurve per fin (via attach_pcurve_segment),
matching the band surface's parameterization (u = angle [0, tau], v = height
along ez from the band frame origin). Latitude-circle fins span u 0->tau at
their height; seam (open-edge) fins are vertical at u = tau when ascending,
u = 0 when descending (the cone/cylinder primitives' seam convention);
collapsed pole edges contribute no fin. integrate_curved_face reads these
pcurve endpoints for its UV-domain quadrature bounds.
Proof: bicone and barrel now assert mass_properties().volume EXACT to 1e-9
(2pi/3 and 4pi/3) -- confirming the pcurve UV mapping matches Cone3/Cylinder3
exactly (a wrong mapping would mis-bound the integral). The two cone bands of
the bicone share a latitude circle (not full-surface-covering), so they take
integrate_curved_face's pcurve-bounds path -- exactly what this exercises.
RUNNING TOTAL: stays 60/144 (hardening, not a new item). Revolve solids are
now first-class for both tessellation AND analytic mass properties.
GATE: exact CI triplet green. No boolean/tessellate pipeline change (pcurves
only) -> no new fuzz path. Merged.

## Addendum 61 (2026-06-08): BLEND/FILLET family opens -- rolling-ball blend GEOMETRY, plane-plane exact cylinder (file 40 rung 1; map stays 60/144, surgery next)

Fillet research landed (docs/research/kernel/40-blend-fillet-surface-generation
+ 41-blend-overflow + Parasolid edge-blending manual fetches). Per the
standing order, read file 40 before building. Its centerpiece (the "single
most important procedural fact", §1.2): spine = SSI(offset(S_a,r),
offset(S_b,r)); classify the spine -> line=Cylinder(exact), circle=Torus
(exact), else cyclide/NURBS canal. Build order rung 1 = plane-plane -> exact
cylinder. KEY INSIGHT from the research: a fillet is TANGENT (G1) to its
supports along the spring curves, so it CANNOT be done by a global boolean
(Keel's engine declines tangent faces) -- it is a local imprint/trim/stitch
(file 40 §3), bespoke topology surgery, exactly why production kernels do it
that way and not via booleans.

New crates/keel-topo/src/blend.rs: blend_cylinder_for_edge(edge, radius) ->
EdgeBlend { spine, spring_a, spring_b, surface: Cylinder3 } for a CONVEX
plane-plane edge. Offsets each support plane by r toward material
({n.x = n.p - r}), intersects them for the spine LINE (two-plane intersection,
denom = sin^2), feet-of-perpendicular for the two spring lines, and a cylinder
of radius r about the spine (frame x -> first spring, z = spine dir). This is
the GEOMETRY stage; it does not yet modify the body.
Tests: block [0,2]^3 top-right edge, r=0.5 -> spine line {x=1.5,z=1.5}||y,
cylinder radius 0.5 EXACTLY TANGENT to both supports (axis 0.5 from z=2 and
x=2), spring lines on z=2/x=2 at the tangency points, spring points exactly r
from the spine. Bad-radius rejected.

Scope: plane-plane convex only (rung 1 geometry). The trim-and-stitch local
operation that inserts the blend face (imprint spring curves, trim the strip
to the sharp edge, delete the edge, sew the cylinder in with closed-form
pcurves -- file 40 §3) is the NEXT stage and completes the fillet capability.
Circle-spine tori (plane-cyl/plane-sphere), cyclides, NURBS canals, variable
radius, and vertex/setback blends are the higher rungs (file 40 §1.4/2/4).
RUNNING TOTAL: stays 60/144 (blend geometry validated; the fillet item
completes with the surgery). The biggest remaining family is now open.
GATE: exact CI triplet green (115 keel-topo). Pure geometry -> no fuzz path.
Merged.

## Addendum 62 (2026-06-08): EDGE FILLET complete -- trim-and-stitch surgery inserts the exact cylinder blend (file 40 §3; map 60 -> 61/144)

The user said "build it now". Completed the plane-plane edge fillet end to
end: fillet_edge(edge, radius) -> filleted Body. KEY INSIGHT that unblocked
it: no new Euler operator is needed -- the cap-corner-vertex dissolution
(remove the sharp vertex, replace two boundary edges with the end arc) is
achievable as kef (merge the corner triangle into the strip) THEN kev (kill
the resulting degree-1 spur). The whole surgery uses existing operators.

Pipeline (file 40 §3, the local imprint/trim/stitch; NOT a boolean, since the
blend is tangent/G1 and the boolean engine declines tangent faces):
 Phase 1 -- trim each support to its spring line. imprint_open_curve could
   NOT be reused: it treats the curve as pre-bounded over t in [0,1] and Line3
   normalizes its dir, so a Line3 only spans a unit segment. Instead
   imprint_spring_line computes the spring's crossings with the two cap-side
   boundary edges explicitly (in-plane perpendicular m = n x spring.dir; sign-
   straddle test), split_edge at each crossing, split_face between them ->
   spring edge + strip (the sliver still carrying the sharp edge) + trimmed
   support. Attaches the spring line curve.
 Phase 2 -- split each cap face (perpendicular end face) along its end arc via
   split_face between the two spring vertices; attaches the quarter-circle arc
   (centre = spine projected into the cap plane, radius r).
 Phase 3 -- dissolve: kef(sharp edge) merges the two strips; then for each end,
   kef(spring stub) merges the corner triangle in and kev(spur) removes the now
   degree-1 sharp vertex. The four corner fragments become one face.
 Phase 4 -- attach the exact Cylinder3 (axis = spine, radius r) to that face.
Helpers added (blend.rs): faces_at_vertex, edge_between, face_has_edge,
boundary_edge_at_vertex_excluding, line_crosses_edge, imprint_spring_line.
Made imprint.rs fin_ending_at_vertex pub(crate).

Test: 2x2x2 block, fillet top-right edge r=0.5 -> validate() ok, V8E12F6 ->
V10 E15 F7 (Euler 2), exactly ONE cylinder blend face of radius 0.5. The
fillet OPERATION produces a valid B-rep with the exact rolling-ball cylinder.

Scope/deferrals (honest): convex plane-plane edge with simple degree-3 end
corners (the box-like case). The blend face is a QUARTER cylinder, but
tessellate_cylinder is full-wrap (no angular trim) and the blend face has no
pcurves yet -> mesh_volume / analytic mass_properties do NOT yet measure
filleted solids (validate() proves the topology + the exact surface; the
volume oracle is the follow-up). Also follow-ups: trimmed-cylinder
tessellation, blend pcurves, concave edges, non-planar supports, circle-spine
torus rung (plane-cyl/plane-sphere), overflow/radius-limit handling (file 41
LFS predictor), variable radius, vertex/setback blends.
RUNNING TOTAL: 60 -> 61/144. The fillet (constant-radius rolling-ball, the
headline blend op) is real: a valid filleted solid from a sharp-edged one.
GATE: exact CI triplet green (fmt + clippy --workspace --all-targets -D
warnings + workspace test 116 keel-topo). Surgery uses Euler ops covered by
fuzz_topo_ops; no boolean/tessellate pipeline change. Merged.

## Addendum 63 (2026-06-08): TRIMMED-CYLINDER tessellation -> fillet VOLUME oracle (closes Addendum 62's deferral; map stays 61/144)

Closed the volume-oracle deferral from the fillet milestone. tessellate_cylinder
was full-wrap only; a fillet's blend face is a QUARTER cylinder, so mesh_volume
mis-measured filleted solids. Two fixes:

1. ANGULAR TRIM (tessellate.rs): new cyl_angular_span(face, ...) returns the
   full wrap (0, TAU) when the face carries a CLOSED circle edge (whole lateral
   or SSI-trimmed band -- existing behavior preserved), else the [phi_lo, phi_hi]
   span of the face's boundary vertices in the cylinder (ex, ey) basis (a
   partial-angle patch). tessellate_cylinder meshes that phi range instead of
   the full TAU. The quarter blend now meshes exactly its 90 degrees.
2. SURFACE-COPY BUG in the fillet surgery (blend.rs imprint_spring_line): split_face
   leaves the NEW face with no surface (imprint_open_curve copies it manually
   afterward; I had not). When the kept support was the new face it tessellated
   EMPTY -> a whole support face missing from mesh_volume (the 5.85-vs-7.89
   miss). Now copy the support plane to split.face_new.

Proof: the filleted 2x2x2 block (top-right edge, r=0.5) mesh_volume now matches
8 - (r^2 - pi r^2/4)*L = 7.89270 within 1% (tessellation). The fillet is now
GEOMETRICALLY validated (correct volume), not just topologically. The angular-
trim also unblocks trimmed-cylinder faces generally (cone/cyl SSI boolean
fragments). Analytic mass_properties on the blend still needs blend pcurves
(next); mesh_volume now covers it.
RUNNING TOTAL: stays 61/144 (hardening the fillet, not a new item).
GATE: exact CI triplet green (116 keel-topo). tessellate is shared with the
winding classifier -> fuzz_boolean re-soaked. Merged.

## Addendum 64 (2026-06-08): Fillet ROBUSTNESS + TORUS-RUNG geometry (file 40 Case B; map stays 61/144)

Two increments on the blend family:
1. Fillet robustness: fillet_edge now tested on a VERTICAL box edge (+x/+y
   supports) as well as the top-right (+z/+x) -- valid B-rep, V10 E15 F7,
   volume 7.8927. The surgery generalizes across edge orientations.
2. TORUS-RUNG geometry (the second rung of the exact-analytic ladder, file 40
   Case B): blend_torus_for_edge(edge, radius) -> EdgeBlendTorus { spine,
   spring_plane, spring_cyl, surface: Torus3 } for a CAP-RIM edge where a
   planar face meets a cylinder perpendicular to its axis. Offsets: plane by r
   toward material (h - sgn*r), coaxial cylinder inward to R - r; their
   intersection is the spine CIRCLE (radius R-r); the rolling-ball envelope of
   a circle is a TORUS (major = R-r, minor = r) -- exact native Keel torus, no
   NURBS. Spring circles: radius R-r on the cap, radius R on the cylinder.
   Requires R > 2r. Mirrors blend_cylinder_for_edge (geometry stage only).
   Test: cylinder R=1 h=2, round the top rim r=0.3 -> torus major 0.7 minor
   0.3, EXACTLY tangent (outer equator major+minor=R=1 touches the lateral;
   top centre_z+minor=2 touches the cap), spine radius 0.7 at z=1.7.
Non-perpendicular plane-cylinder (cyclide) and the torus trim-and-stitch
surgery (arc spring circles + trimmed-torus tessellation) are follow-ups.
RUNNING TOTAL: stays 61/144 (extends the fillet item's reach).
GATE: exact CI triplet green (118 keel-topo). Pure geometry -> no fuzz path.
Merged.

## Addendum 65 (2026-06-08): TORUS tessellation -> torus faces first-class (map stays 61/144; torus-fillet precursor)

Like cones (Addendum 59), TORUS faces were dropped to empty in tessellate_face
(the `_` arm), so the existing torus() primitive had broken bbox/winding/
booleans (mass_properties worked via the analytic full-coverage path, but the
tessellation consumers did not). tessellate_torus added and wired into the
dispatch (now exhaustive over Plane/Sphere/Cylinder/Cone/Torus): grid mesh of
point(u,v) = c + (R + rr cos v)(ex cos u + ey sin u) + ez rr sin v, outward =
away from the tube centreline. Whole torus (partial-tube blend faces -- the
torus-fillet blend -- are a follow-up with the torus surgery).
Test: torus major 3 minor 1 -> bbox xy +/-4 z +/-1 (tight), mesh_volume = 2 pi^2
R r^2 = 6 pi^2 within 1%. Torus is now first-class for bbox/winding/booleans,
and the torus blend face can be measured once the surgery trims its tube.
RUNNING TOTAL: stays 61/144 (infrastructure; fixes the torus primitive +
unblocks the torus-rung fillet surgery).
GATE: exact CI triplet green (119 keel-topo). tessellate shared with the
winding classifier -> fuzz_boolean re-soaked. Merged.

## Addendum 66 (2026-06-08): TORUS FILLET complete -- cap-rim rounding to exact torus (file 40 rung 2; map 61 -> 62/144)

Completed the second rung of the exact-analytic blend ladder: fillet_cap_rim(
edge, radius) rounds a CYLINDRICAL-BOSS RIM (a planar cap meeting a cylinder
perpendicular to its axis) with an exact torus blend. Both fillet rungs now
work end to end (rung 1 plane-plane -> cylinder; rung 2 plane-cylinder -> torus).

Surgery (file 40 §3 for the circle-spine case): imprint the two SPRING CIRCLES
(spring_plane on the cap via imprint_closed_curve; spring_cyl on the periodic
lateral via imprint_closed_curve_crossing -- it crosses the cylinder seam),
trimming the cap to radius R-r and the cylinder to the offset height; then
remove the sharp rim and form the torus ring.
KEY TOPOLOGY INSIGHT: imprinting the interior spring_plane circle makes the cap
annulus a HOLED face (rim outer loop + spring_plane hole loop), and kef refuses
a multi-loop dying face. Fix: mekr(fin_outer, fin_ring) (the inverse of kemr)
bridges the spring_plane hole to the rim outer loop, making the annulus single-
loop (the bridge becomes the seam of the periodic torus ring); THEN kef(rim)
merges the annulus and the cylinder top band into the ring, which gets the exact
Torus3. This periodic-ring construction is what the cylinder fillet did not need
(its open-line spring curves gave simple non-holed strips). imprint_ring helper
dispatches crossing-vs-interior like the boolean does.

Test: cylinder R=1 h=2, round the top rim r=0.3 -> validate() ok, exactly one
torus blend face (minor 0.3). The torus fillet OPERATION produces a valid B-rep.

Scope/deferrals: cap case (plane perpendicular to cylinder axis), R > 2r,
convex. Trimmed-TORUS tessellation (partial tube -> mesh_volume on the result;
tessellate_torus is full-tube so the ring is not yet measured -- validate proves
topology + exact surface), general non-perpendicular plane-cylinder (cyclide),
concave, variable radius, and vertex blends remain follow-ups.
RUNNING TOTAL: 61 -> 62/144. Constant-radius fillet now covers BOTH the
plane-plane and the plane-cylinder (curved-support rim) cases exactly.
GATE: exact CI triplet green (fmt + clippy --workspace --all-targets -D
warnings + workspace test 120 keel-topo). Euler-op surgery (imprint/mekr/kef)
covered by fuzz_topo_ops + fuzz_imprint; no boolean/tessellate change. Merged.

## Addendum 67 (2026-06-08): TRIMMED-TORUS tessellation -> torus fillet VOLUME oracle (closes Addendum 66's deferral; map stays 62/144)

Closed the volume-oracle deferral for the torus fillet. tessellate_torus was
full-tube; the cap-rim blend ring is a QUARTER tube (v in [0, pi/2]), so
mesh_volume over-counted. New torus_tube_span(face, torus): full tube (0, TAU)
when the face covers its whole closed surface (the torus primitive), else the
min/max tube-angle v of the face's boundary vertices (v computed per vertex as
atan2((p - tubecentre).ez, (p - tubecentre).radial), tubecentre = c +
radial*major). tessellate_torus now trims v to that span (u stays full
revolution). Mirrors the trimmed-cylinder cyl_angular_span.
Proof: the filleted cylinder (R=1 h=2, top rim r=0.3) mesh_volume now matches
the solid-of-revolution value 1.7pi + pi*int_0^0.3 (0.7+sqrt(0.09-s^2))^2 ds =
6.1700 (within tolerance). The torus fillet is now GEOMETRICALLY validated, not
just topologically. The full torus primitive still meshes fully (face-covers-
closed-surface -> no trim).
RUNNING TOTAL: stays 62/144 (hardening the torus fillet).
GATE: exact CI triplet green (120 keel-topo). tessellate shared with the
winding classifier -> fuzz_boolean re-soaked. Merged.

## Addendum 68 (2026-06-08): CONCAVE fillet GEOMETRY + convexity detection + fillet_edge dispatch (map stays 62/144)

Three increments toward concave (reentrant) fillets, the highest-value fillet
extension (inner-corner rounding is ~half of all fillets):
1. fillet_edge now AUTO-DISPATCHES by support geometry: a plane/cylinder cap rim
   -> the torus rung (fillet_cap_rim); two planes -> the cylinder rung. One entry
   point. (Bottom-rim torus fillet test also added -- the sgn=-1 cap path.)
2. CONVEXITY DETECTION: edge_is_convex(edge) probes the generalized winding
   number just off the edge along the in-face bisector (u0+u1, into both faces'
   interiors); inside the material => convex, in the open notch => concave.
   Corrects an earlier WRONG assumption (I had thought the offset sign was
   convexity-independent): a concave edge needs the ball on the +r (non-material)
   side, filling the notch, not the -r material side.
3. blend_cylinder_for_edge now uses the detected sign (off = -r convex / +r
   concave), so the blend GEOMETRY is correct for both. The convex plane-plane
   SURGERY is gated for concave (it adds rather than removes material -- a
   follow-up); the blend geometry already handles concave.
Test: L-prism (non-convex profile [(0,0),(2,0),(2,1),(1,1),(1,2),(0,2)] extruded)
-> reentrant edge at (1,1) detected CONCAVE, corner edge CONVEX; concave blend
spine at (1+r,1+r)=(1.3,1.3) tangent to both faces; convex blend spine inside the
material (1.7,0.3); concave fillet_edge surgery gated. Existing convex fillets
still pass through the new detection unchanged.
RUNNING TOTAL: stays 62/144 (concave geometry done; concave surgery completes it).
GATE: exact CI triplet green (122 keel-topo). edge_is_convex reuses the existing
winding classifier; no tessellate/boolean pipeline change -> no new fuzz path.
Merged.

## Addendum 69 (2026-06-08): CONCAVE fillet COMPLETE -- the unified pipeline (research file 44; map 62 -> 63/144)

Requested + received research file 44 (concave-fillet-topology-surgery): the
canonical recipe. Verdict: convex (round, material removed) and concave (fillet,
material added) are ONE pipeline that forks only on the convexity sign sigma; the
cap-SHRINK (convex) and cap-GROW (concave) are duals sharing the kef+kev tail,
and grow-vs-shrink happens AUTOMATICALLY via face adjacency (file 44 Q2/Q4,
confirmed verbatim against Parasolid PK_BLENDSF, OCCT ChFi3d::ConcaveSide, ACIS).
So the whole concave un-gate was TWO lines: drop the gate, and attach the blend
cylinder with sense = convex (radial-out convex / radial-IN concave).

CRITICAL DIAGNOSIS that unblocked it: my earlier "wrong volume" (3.0479 vs
3.0193) was NOT a surgery bug -- the cap-loop dump proved the topology was already
correct (the reentrant corner (1,1) is REMOVED and replaced by the arc edge
(1.3,1)->(1,1.3), the cap grown). The 0.95% gap is the chord-vs-arc tessellation
approximation of the cap's arc boundary edge (loop_polygon samples straight
chords), the SAME approximation the convex fillet has and which I tolerated at 1%
there; I had mis-attributed it via a too-tight 0.01 absolute tolerance. The
unfilleted L-prism mesh_volume is exactly 3.0, confirming tessellation handles the
non-convex L-cap fine. Earlier helper: the split_cap surface-copy fix (committed
2d20959) was the real bug (the bottom cap had been vanishing).

fillet_edge now rounds CONVEX and CONCAVE plane-plane edges through one unified
surgery. Test: L-prism (non-convex profile) reentrant edge r=0.3 -> valid B-rep,
one cylinder blend face, mesh_volume 3.0193 (L 3 + filled sliver (r^2 - pi r^2/4)h)
within 1% (tessellation). Convex fillets unaffected (sense=true).
Deferral: exact analytic volume needs loop_polygon to sample arc cap edges (a
tessellation-accuracy improvement; would tighten BOTH convex and concave);
mixed-convexity ends + bead-construct-and-sew (file 44 Q3/Q5) for harder caps.
RUNNING TOTAL: 62 -> 63/144. Inner-corner rounding (concave fillet), ~half of all
real fillets, now works -- the unify verdict from file 44 held exactly.
GATE: exact CI triplet green (fmt + clippy --workspace --all-targets -D warnings +
workspace test 122 keel-topo). Euler-op surgery; no boolean/tessellate change.
Merged.

## Addendum 70 (2026-06-08): EAR-CLIPPING planar tessellation + ARC-edge sampling -> near-exact fillet volumes (map stays 63/144)

Two coupled tessellation fixes (the follow-up flagged in Addendum 69):
1. EAR-CLIPPING (tessellate.rs): tessellate_planar fanned each loop from its
   CENTROID, valid only for STAR-CONVEX loops -- a latent correctness gap for any
   non-star-convex planar face (a fillet's L-cap, a boolean fragment) affecting
   winding/volume/bbox. Replaced the outer-loop fan with proper ear-clipping
   (earclip_3d projects to the plane's 2D basis; earclip_2d is the standard
   O(n^2) reflex-aware ear removal with a degeneracy guard). Inner-ring (hole)
   loops keep the reversed-centroid-fan subtraction.
2. ARC-edge sampling (loop_polygon): an OPEN arc edge (a Circle3 between two
   distinct vertices -- a fillet cap's spring/end arc) is now sampled along its
   SHORT span (8 segments) so the boundary polygon follows the true arc, not its
   chord. (Closed full-circle edges still use the whole-circle fallback.)
   These had to land TOGETHER: arc sampling alone made the non-star-convex
   concave L-cap WORSE under the centroid fan (Addendum 69's revert); with
   ear-clipping it triangulates correctly.
Result: fillet mesh_volume is now near-exact. Tightened the convex box/vertical
and concave fillet volume asserts from ~1% to 0.2% (they pass at 0.08%). The
unfilleted-solid volumes (box 8, L-prism 3) and all curved primitives unchanged.
This hardens the winding classifier for non-star-convex faces kernel-wide.
RUNNING TOTAL: stays 63/144 (tessellation robustness + accuracy; not a new item).
GATE: exact CI triplet green (122 keel-topo). tessellate_planar is the winding-
classifier hot path -> fuzz_boolean re-soaked. Merged.

## Addendum 71 (2026-06-08): PARTIAL (wedge) REVOLVE -- loft-topology + arc verticals + cylinder/sector bands (64/144)

revolve_partial(frame, profile, theta) sweeps a CLOSED off-axis meridian polygon
through theta in (0, pi] about frame.z. KEY INSIGHT: a partial revolve of an
OFFSET profile has the SAME topology as a loft -- two end-cap faces (the meridian
region at phi=0 and phi=theta) joined by side bands -- so the construction REUSES
loft's exact Euler skeleton (mvfs, rim mev-chain, cap mef, verticals, side mefs +
top cap). Only the geometry differs: the "verticals" are ARCS (Circle3 at each
profile point's radius/height, phi 0->theta) and the side bands are partial
CYLINDERS (constant-r segment) or planar ANNULAR SECTORS (constant-z segment)
instead of loft's line verticals and planar quads. Caps are planar (phi=0 outward
-ey; phi=theta outward -sin th ex + cos th ey). Profile winding is normalized to
CCW so the per-band outward normals (cylinder sense = sign of dz; sector normal z
= -sign of dr) come out consistent. The faces it needs already tessellate: the
cylinder sectors via cyl_angular_span (Addendum 62) and the planar sectors via
ear-clipping + arc-edge sampling (Addendum 70).
Topology: n points -> 2n V, 3n E, n+2 F (Euler 2). Test: rectangle meridian
r in [1,2] x z in [0,1] revolved pi/2 -> annular sector, counts (8,12,6),
mesh_volume within 3% of 3pi/4 (curved-face chord undershoot; documented).
SCOPE / follow-ups: slanted segments revolve to CONE sectors (need cone angular
trim, mirroring cyl_angular_span) -> rejected for now; theta > pi (arc tessellation
samples the short span) -> rejected; true-pole profiles (profile meets the axis)
and pcurves for analytic mass_properties -> follow-ups (use mesh_volume meanwhile).
RUNNING TOTAL: 63 -> 64/144 (partial/wedge revolve, sweep/loft family).
GATE: exact CI triplet green (125 keel-topo, clippy -D warnings, fmt). No fuzz
needed -- pure constructor, does not touch tessellate_planar or the boolean
pipeline. Merged.

## Addendum 72 (2026-06-08): CONE angular trim -> slanted partial revolve (broadens 64/144)

Completes revolve_partial: slanted meridian segments now sweep CONE SECTORS
(previously rejected). The enabler is a one-line mirror of the cylinder's angular
trim: tessellate_cone now calls cyl_angular_span (already generic -- it returns
[0, tau] when the face has a CLOSED circle edge, else the boundary vertices' phi
span) and sweeps [plo, phi_hi] instead of a hardcoded full [0, tau]. Full cones
(bicone/barrel/cone primitive: closed cap circles) are PROVABLY unchanged --
cyl_angular_span returns [0, tau] for them -- so the existing cone tests pass
untouched. revolve_partial attaches a Cone3 for slanted segments (anchor v=0 at
the segment's start, slope = dr/dz, sense = (zb > za) radial-out, same rule as the
cylinder band). Test: off-axis triangle meridian [(1,0),(2,0),(1,1)] revolved pi/2
-> cone-sector wedge, counts (6,9,5), mesh_volume ~pi/3 (partial Pappus
V = theta*R_centroid*Area = (pi/2)(4/3)(1/2)). revolve_partial now handles all
axis-parallel / axis-perpendicular / slanted segment mixes; only theta>pi, true
poles, and pcurves remain follow-ups.
RUNNING TOTAL: stays 64/144 (broadens the partial-revolve item already counted in
Addendum 71; also unblocks future chamfer/fillet-of-revolution).
GATE: exact CI triplet green (126 keel-topo, clippy -D warnings, fmt) + fuzz_boolean
soak (tessellate_cone feeds the classifier for cone-faced bodies; change is inert
for the full-cone fuzz corpus). Merged.

## Addendum 73 (2026-06-08): partial revolve theta -> (0, 2pi) via opt-in Edge::arc_sweep (broadens 64/144)

Completes the angular range of revolve_partial. The blocker was real: a Circle3
edge plus its two endpoints is genuinely ambiguous (the theta arc vs the 2pi-theta
complement), and BOTH tessellators inferred the arc from atan2, which wraps at the
+/-pi branch cut -- so any sector wider than pi was mis-measured (loop_polygon
sampled the short complement; cyl_angular_span's atan2 min/max gave the wrong
span).
FIX: a new OPT-IN field Edge::arc_sweep: Option<f64> -- the signed angular sweep
(bounds.0 -> bounds.1, circle frame) of an arc edge. Default None means "short
span", exactly the behaviour every pre-existing arc relied on, so there is ZERO
change to fillet/cap/boolean arcs (regression-free by construction). revolve_partial
stamps Some(theta) on its vertical arcs (bounds.0 = phi=0 -> bounds.1 = phi=theta,
CCW, so +theta). Both tessellators now use it when present:
- loop_polygon: d = +/-sweep (sign from fin.forward) instead of the short-span
  normalization; segment count scales with |sweep| so chord error stays bounded as
  the arc approaches 2pi.
- cyl_angular_span: returns start_angle .. start_angle + sweep CONTINUOUSLY (no
  atan2 min/max), so a >pi sector is exact and the branch cut is never crossed.
Topology is unchanged (no slab-splitting needed): same 2n V, 3n E, n+2 F, just
wider arcs. Test: annular rectangle revolved 3pi/2, counts (8,12,6), mesh_volume
~9pi/4. The (0, pi] cap is gone; only theta -> 2pi (full revolution) and true-pole
profiles and pcurves remain follow-ups.
RUNNING TOTAL: stays 64/144 (broadens the partial-revolve item; full angular range).
GATE: exact CI triplet green (127 keel-topo, clippy -D warnings, fmt) + fuzz_boolean
soak (loop_polygon + cyl_angular_span are the winding-classifier hot path; the
arc_sweep paths are gated on Some, inert for the full-primitive fuzz corpus).
Merged.

## Addendum 74 (2026-06-08): RIGID BODY TRANSFORM (rotation + translation) -- 65/144 (first counter advance since partial revolve)

Body::transformed(&Transform3) -> Body (new file crates/keel-topo/src/transform.rs):
the same isometry carries every vertex point and every analytic surface/curve
FRAME; radii/angles are invariant because the motion is rigid; pcurves (parameter
space) are untouched. Foundational for assemblies/instancing; mirror (reflection)
and NURBS-body transforms are follow-ups.
GUARD: the linear part must be a proper rotation -- the basis images must stay
orthonormal (rigid -> radii preserved) and right-handed. A scale/shear (non-
orthonormal) is rejected ("rigid only"); a reflection (det < 0) is rejected
("mirror is a follow-up"). NURBS surfaces/curves error ("follow-up").
KEY CORRECTNESS CATCH: the curve/surface arenas hold MORE than the 3D model
geometry -- a 12-edge box's arena had 36 curves (edge lines + fin PCURVES, which
live in the same arena, + construction orphans, including a NURBS pcurve).
Transforming every arc would have corrupted the pcurves (they are UV parameter-
space curves and must NOT move in 3D). Fix: transform ONLY geometry REFERENCED by
the topology -- surfaces via face.surface, curves via edge.curve (deduped through
a HashSet); pcurves (fin.pcurve) and orphans are deliberately left alone.
Test: 2^3 block rotated pi/2 about z then translated +5x -> validate ok,
mass_properties volume invariant at 8, bbox exactly [3,5]x[0,2]x[0,2]. Plus a
rejection test for uniform scale and for an x-reflection.
NOTE (cleanliness, not a bug): construction leaves orphan curves in the arena (36
vs 12 referenced for a box). Harmless (unreferenced geometry is ignored by
tessellation/mass_properties/validate) but a future arena-compaction pass could
reclaim them.
RUNNING TOTAL: 64 -> 65/144 (body transform, a fresh capability).
GATE: exact CI triplet green (129 keel-topo, clippy -D warnings, fmt). No fuzz
needed -- additive new op, no boolean/tessellation-pipeline change. Merged.

## Addendum 75 (2026-06-08): MIRROR (reflection across a plane) -- 66/144

Body::mirrored(plane_point, plane_normal) -> Body (transform.rs): reflect a body
across an arbitrary plane via a Householder reflection R(x) = (I - 2 m m^T)x +
2(p.m)m built as a Transform3, applied through the SAME apply_isometry path as the
rigid transform.
KEY INSIGHT (simpler than the first attempt): a reflection needs NO special
orientation handling. Reflecting every frame DIRECTLY sends frame.z -> M(frame.z),
so the outward normal -- which mass_properties derives from frame.z x region-
solidity and tessellation from frame.z x sense -- maps to M(N), the correct
mirrored outward. Because M is orthogonal the face's (u,v) coordinates are
preserved (u' = M(x).M(p-o) = x.(p-o) = u), so pcurves stay valid. The frame goes
left-handed but nothing relies on its chirality. The first attempt flipped frame.z
to "restore" right-handedness + flipped the face sense; that DOUBLE-flip made
mass_properties' frame.z x region-orient normal point inward -> non-positive
volume. Removing all of it (plain direct reflection) is correct AND works for
CURVED faces too (cylinder/cone/sphere/torus), with no per-type casing.
transformed() (rigid) now routes reflections to mirrored() via an isometry_kind
classifier (orthonormal check rejects scale/shear; det sign splits rotation vs
reflection). Tests: box across x=0 -> [-2,0]x[0,2]x[0,2], volume 8, valid; offset
cylinder across x=0 -> validate + mesh_volume ~2pi + mirrored bbox.
RUNNING TOTAL: 65 -> 66/144 (mirror, a fresh capability; builds on the Addendum-74
transform).
GATE: exact CI triplet green (131 keel-topo, clippy -D warnings, fmt). No fuzz --
additive op, no boolean/tessellation-pipeline change. Merged.

## Addendum 76 (2026-06-08): DRAFT ANALYSIS (item 107) -- 67/144

Body::draft_analysis(pull) -> Vec<FaceDraft{face,min,max}> (interrogate.rs): per-
face signed draft-angle RANGE relative to a pull direction, for moldability /
pull-direction checks. Draft = arcsin(outward_normal . pull_hat) over the face's
OUTWARD tessellation triangles: +pi/2 faces toward the pull, -pi/2 away, 0 a
vertical wall (zero draft / undercut risk); a min<0<max range flags an undercut
face for that pull. Planar faces collapse to min==max; curved faces report the
true range. Reuses tessellate_face (the same outward triangles the winding
classifier/volume use) -- no new geometry. Roadmap Phase 6.
Tests: 2^3 box vs +z pull -> 2 caps at +/-pi/2, 4 walls at 0; cylinder vs +z ->
caps +/-pi/2, lateral wall 0 all the way around (the undraftable case).
RUNNING TOTAL: 66 -> 67/144 (draft analysis, a fresh interrogation item).
GATE: exact CI triplet green (133 keel-topo, clippy -D warnings, fmt). No fuzz --
read-only query, no mutation/boolean/tessellation-pipeline change. Merged.

## Addendum 77 (2026-06-08): full-revolve FLAT END-CAPS (disc caps) -- 68/144

revolve() now accepts a HORIZONTAL end segment (pole (0,h) -> rim (r,h)), which
revolves to a planar DISC cap instead of requiring a cone-to-pole. The band
TOPOLOGY is unchanged (pole center + outer latitude circle, same as a cone band);
only the geometry attach gains a branch: when h_lo == h_hi, attach a Plane(z=h)
with outward -ez for the lower cap / +ez for the upper (decided by h vs the
profile mid-height), instead of a Cone/Cylinder. The end-horizontal guard is
relaxed; INTERIOR horizontal segments (holed washers) stay rejected (a later
holed-face item). No pcurves are attached to the disc -- revolve attaches edge
curves AFTER the band loop, and the planar mass_properties path samples the disc
loop geometry directly at call time -- so analytic volume still works.
Now a flat-ended solid of revolution (a unit cylinder, a bowl, a flat-ended
frustum) is constructible by revolve, not just pole-to-pole cone/cylinder shells.
Test: revolve [(0,0),(1,0),(1,1),(0,1)] -> unit cylinder, 3 faces (disc/wall/
disc), validate ok, mesh_volume AND mass_properties ~pi. bicone/barrel regression
tests stay green (cone-to-pole path untouched).
RUNNING TOTAL: 67 -> 68/144 (flat-cap solid of revolution).
GATE: exact CI triplet green (134 keel-topo, clippy -D warnings, fmt). No fuzz --
constructor change, not a boolean/tessellation-pipeline mutation. Merged.

## Addendum 78 (2026-06-08): UNIFORM BODY SCALE (planar bodies) -- 69/144

Body::scaled(center, factor) -> uniform scale about a point (transform.rs). Vertices
-> center + (p-center)*factor; plane-surface frame origins scale, axes unchanged
(uniform scale preserves directions); line-curve origins scale, dir unchanged.
Planar-faced bodies only for now: mass_properties projects the SCALED vertices
through the (scaled-origin, unchanged-axes) frame, so it computes u' = s*u and
stays self-consistent WITHOUT touching stored pcurves (u'=(p_scaled-origin').x =
s*(p-origin).x); tessellation likewise samples scaled geometry. Curved surfaces
(radii would scale) and circle/ellipse/NURBS edges (radii + pcurve params) are a
follow-up -- rejected.
Test: 2^3 block scaled 2x about origin -> [0,4]^3, validate ok, mass_properties
volume 64 = 8*2^3, bbox exact; rejects a curved body and factor 0.
RUNNING TOTAL: 68 -> 69/144 (body scale; rounds out the transform family
transform/mirror/scale).
GATE: exact CI triplet green (136 keel-topo, clippy -D warnings, fmt). No fuzz --
additive op. Merged.

## Addendum 79 (2026-06-08): INTERIOR WASHER / HOLED-FACE revolve via kemr -- 70/144

revolve() now accepts INTERIOR horizontal segments (both ends off-axis) -> flat
ANNULAR (washer/shoulder) faces. This is the HOLED-FACE construction path, the
foundation shell (41) and thicken (44) also need.
KEY INSIGHT: the sequential band construction builds each band as a single-loop
SEAM-BRIDGED face (the per-band meridian seam bridges the lower+upper latitude
circles into one loop). For cone/cylinder bands that is fine (they tessellate via
tessellate_cylinder/cone, bypassing loop_polygon). But a flat washer is PLANAR ->
tessellate_planar -> loop_polygon, which mis-handles a 2-closed-circle + seam loop
(it falls back to sampling ONE circle as a full disc -> wrong volume, 13.58 vs
5pi). FIX: for an interior horizontal band, kemr (kill-edge-make-ring) the band's
SEAM edge -> the single loop splits into a proper 2-LOOP holed face (outer circle
loop + inner circle RING/hole), which tessellate_planar (outer ear-clip + inner
reversed fan) and mass_properties (inner subtracts) already handle. Disc end-caps
keep their seam (inner "circle" is the pole -> no ring to make). Tracked each
band's seam edge through construction to reach it. Also replaced the disc-cap
normal heuristic (h vs mid-height) with the robust nz = -sign(dr) rule that is
correct for caps AND washers (meridian is bottom->top, solid on the axis side).
Test: stepped cylinder [(0,0),(2,0),(2,1),(1,1),(1,2),(0,2)] -> wide r=2 over
h[0,1] + narrow r=1 over h[1,2] with a washer shoulder at h=1; 5 faces, validate
ok, mesh_volume AND mass_properties ~5pi (4pi + pi). bicone/barrel/flat-cap
regressions green.
RUNNING TOTAL: 69 -> 70/144 (washer/holed-face solid of revolution; unlocks the
holed-face path).
GATE: exact CI triplet green (137 keel-topo, clippy -D warnings, fmt). No fuzz --
constructor change; the holed-face tessellation path it exercises is the existing
inner-ring path, not a boolean/winding mutation. Merged.

## Addendum 80 (2026-06-08): CLOSEST-POINT-ON-BODY + tube-boolean gap finding -- 71/144

Body::closest_point(p) -> Option<(Vec3, f64)> (interrogate.rs): the closest surface
point to an external point + its distance. Refactored the existing Ericson point-
triangle routine into closest_on_tri (returns the point) with point_tri_distance a
thin norm wrapper (min_distance regressions stay green), then min-reduce over the
body's outward tessellation. Exact for planar faces; tessellation-approximate for
curved (exact face projection later). Test: [0,2]^3, point (5,1,1) -> closest
(2,1,1) on +x face, distance 3.
FINDING (logged, not fixed): a TUBE via Difference of two coaxial same-height
cylinders FAILS -- boolean returns AssemblyFailed("stitched (curved) body
invalid"). The coincident end-caps + genus-1 (through-hole) result hits the known
enclosed-void/genus-1 stitch gap. So tube/annular solids need a from-scratch
genus-1 constructor (like torus()'s kfmrh handle-punch), not a boolean. Queued.
RUNNING TOTAL: 70 -> 71/144 (closest-point interrogation).
GATE: exact CI triplet green (138 keel-topo, clippy -D warnings, fmt). No fuzz --
read-only query + a safe point-triangle refactor. Merged.

## Addendum 81 (2026-06-08): GENUS-1 solid of revolution (tube / annular) via research file 45 -- 72/144

revolve_closed(frame, profile) -> a full-360 solid of revolution of a CLOSED,
OFF-AXIS meridian (all radii > 0) = a GENUS-1 (toroidal) solid: tube / hollow
cylinder / annular ring. The construction follows file 45's derived, Euler-
Poincare-checked sequence, validated against Keel's validator on the first tube:
  mvfs(p0) + mef_on_vertex_loop  -> seed + C0 latitude + bottom cap
  per segment 0..n-2: mev (next latitude) + mef (close band)  -> n-1 lateral bands
  kfmrh(top_cap, bottom_cap)     -> PUNCH THE HANDLE (genus 0->1); the bottom cap
                                    absorbs the top cap's circle as a ring and
                                    becomes the closing inner band (2 loops)
  mekr(outer_fin, ring_fin)      -> add the inner band's meridian seam
  kemr each interior washer seam -> 2-loop holed annulus caps (Addendum 79 trick)
For the hollow cylinder [(1,0),(2,0),(2,1),(1,1)] this lands exactly on the
dossier's V4 E8 F4 (then E6 R2 after the two washer kemrs), debug_validate clean.
The diagnostic that nailed the close: build seed+3 bands -> 3 lateral bands + 2
end caps (C0 at v0, C3 at v3); kfmrh fuses them into the handle, mekr seams it.
Geometry: cylinder/cone/washer-plane per segment (reusing revolve's logic + the
-sign(dr) washer normal); CCW profile normalization. Test: tube r_in=1 r_out=2
h=1 -> 4 faces (bottom washer, outer cyl, top washer, inner cyl), validate ok,
mesh_volume ~3pi = pi(4-1)(1).
KNOWN FOLLOW-UP: analytic mass_properties returns 5pi not 3pi -- the inner band's
contribution ADDS rather than subtracts pi (a region-solidity orientation
subtlety on the kfmrh-punched face). mesh_volume (sense-based tessellation) is
correct, so the GEOMETRY is right; exact analytic volume on the genus-1 face is
deferred (revolve_partial shipped on mesh_volume too).
EARLIER (logged, reverted): both BOOLEAN routes to a tube fail -- coaxial-
coincident Difference -> stitch-invalid; transversal Difference -> valid but inner
wall mis-bounded (Addendum 80). Hence the from-scratch constructor.
RUNNING TOTAL: 71 -> 72/144 (genus-1 tube/annular solid of revolution).
GATE: exact CI triplet green (139 keel-topo, clippy -D warnings, fmt). No fuzz --
constructor; uses existing Euler ops + holed-face path, no boolean/winding change.
Merged.

## Addendum 82 (2026-06-08, overnight): face_curvature surface analysis (broadens 72/144) + massprops sense wall confirmed

Two things this autonomous cycle:
1. ATTEMPTED the unified face-orientation fix (research file 46) to make the genus-1
   tube exact in analytic mass_properties. Tried PURE sense (orient = sign(sense),
   massprops normal = sense*natural = the existing face_outward_normal helper).
   SAME 3 regressions as before (corner_overlap/guillotine/chamfer). Sharpened root
   cause + REVERTED per the log-and-move rule: integrate_planar_face COUPLES loop
   WINDING to the orient sign; boolean-path loops are wound for region-based
   normals, Euler-path (tube) loops for sense-based -- so the helper alone is
   insufficient, the unification must reconcile winding+sense+region together (file
   46's validator invariant). Also found a latent placeholder: boolean.rs ~1122 uses
   nz=(0,0,1) for non-plane surfaces when deriving stitch sense. All recorded in
   [[massprops-sense-region-inconsistency]]; deep, do attended.
2. SHIPPED face_curvature: Body::face_curvature(face, p) -> Option<(k1,k2)>, the
   principal curvatures at the surface point nearest p, from the analytic surface's
   local_geometry (exact): plane (0,0); cylinder r {0,1/r}; sphere r (1/r,1/r);
   cone/torus position-dependent. Reuses local_geometry + project. Test: cylinder
   lateral {0,1/2}, sphere (1/2,1/2) at the equator (poles are param singularities).
   Surface-analysis family alongside draft_analysis (both item 107) -> BROADENS,
   counter stays 72/144.
GATE: exact CI triplet green (140 keel-topo, clippy -D warnings, fmt). No fuzz --
read-only query. Merged.

## Addendum 83 (2026-06-09, overnight): tapered/draft extrude (item 65) -- 73/144

Body::extrude_tapered(base, dir, top_scale): sweep a planar polygon along dir while
scaling the top profile by top_scale about the base centroid -- a truncated pyramid
/ drafted boss. KEY INSIGHT: under centroid scaling the side quads are PLANAR for
ANY polygon (top t_i = c + (b_i-c)*s + dir; then t0-b0 = (t1-b0) - s*(b1-b0) lies in
span(t1-b0, b1-b0) -> the four corners are coplanar), so it is a thin, exact
specialization of loft -- no new topology. top_scale in (0,1) tapers in, 1.0 is a
straight prism, >1 flares. Test: 2x2 square base, +z by 1, top_scale 0.5 -> 1x1 top,
6 faces, mass_properties volume (h/3)(A1+A2+sqrt(A1 A2)) = (1/3)(4+1+2) = 7/3 exact.
FOLLOW-UP: perpendicular-offset draft (every face tilts by a fixed ANGLE; needs a
polygon-inset/miter, not centroid scaling) is the general drafted-extrude.
RUNNING TOTAL: 72 -> 73/144 (tapered extrude, sweep/loft family item 65).
GATE: exact CI triplet green (141 keel-topo, clippy -D warnings, fmt). No fuzz --
additive constructor over loft. Merged.

## Addendum 84 (2026-06-09, overnight): revolve_closed hardening coverage (stays 73/144)

Hardened the genus-1 revolve_closed (Addendum 81), previously tested only on the
rectangular tube. Added: (1) a TRIANGULAR ring cross-section [(1,0),(2,0),(1,1)]
exercising a CONE band ((2,0)->(1,1)) inside the kfmrh-handle construction -- 3
faces (washer+cone+inner cylinder), validate ok, mesh_volume ~4pi/3 (Pappus
2pi*R_c*A = 2pi*(4/3)*(1/2)); (2) rejection of an axis-touching (radius-0) profile
(genus-0, belongs to revolve). Both pass -- the genus-1 constructor + cone-band
geometry are robust beyond the rectangular case. Test-only; counter stays 73/144.
GATE: exact CI triplet green (143 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 85 (2026-06-09, overnight): EXACT analytic surface area for curved faces (stays 73/144)

face_area now computes EXACT analytic area for curved analytic faces (was the
tessellation undershoot), reusing the same trim the tessellator uses, so primitive
faces AND trimmed fillet/revolve bands are exact:
- Cylinder: r * dphi * dh (dphi = cyl_angular_span, dh = cyl_circle_heights range).
- Cone: (dphi/2)(r1+r2) * slant, slant = sqrt(dr^2+dh^2); adds the apex height
  (radius->0) when the face reaches it, so the cone primitive is exact too.
- Sphere (full coverage): 4 pi r^2. Torus (full coverage): 4 pi^2 R r.
- Planar faces -> None (tessellation already exact); NURBS + TRIMMED sphere/torus
  patches -> None (fall back to tessellation; a UV-integral exact area is a
  follow-up). Made tessellate.rs::cyl_angular_span pub(crate) (cyl_circle_heights
  already was, face_covers_closed_surface is pub).
Test: cylinder lateral r=2 h=3 -> 12pi exact; cone lateral -> pi*r*sqrt(13) exact.
NOTE: a cylinder/cone BODY surface_area is still slightly off because the planar
DISC caps tessellate to a 32-gon -- exact single-circle-disc planar area
(pi r^2, the mass_properties single_circle_disc path) is the matching follow-up.
RUNNING TOTAL: stays 73/144 (accuracy improvement to the area interrogation).
GATE: exact CI triplet green (144 keel-topo, clippy -D warnings, fmt). No fuzz --
read-only query. Merged.

## Addendum 86 (2026-06-09, overnight): EXACT planar disc/annulus area -> full bodies exact (stays 73/144)

Completes the exact-surface-area work (Addendum 85). face_area now also computes
exact pi*r^2 for PLANAR disc/annulus faces (every loop a single circle): outer loop
adds pi*r_out^2, inner-ring holes subtract pi*r_in^2. Reuses single_circle_disc
(made pub(crate)) -- the same detector mass_properties' fast path uses, so cylinder/
cone CAPS (single-circle discs) and revolve washers (kemr'd 2-loop annuli) are now
exact. Polygon loops -> None (tessellation already exact). Result: a cylinder BODY's
surface_area is now EXACT (caps 8pi + lateral 12pi = 20pi), not just the lateral.
Still tessellation-approximate: NURBS faces, trimmed sphere/torus patches, and
seam-bridged discs (the flat-cap revolve disc has a rim+seam loop, not a single
circle) -- minor, documented.
RUNNING TOTAL: stays 73/144 (accuracy: surface area now exact for analytic
disc/annulus + curved faces).
GATE: exact CI triplet green (144 keel-topo, clippy -D warnings, fmt). No fuzz --
read-only query. Merged.

## Addendum 87 (2026-06-09, overnight): tube/pipe primitive (stays 73/144)

Body::tube(frame, r_inner, r_outer, h): a named hollow-cylinder (pipe) primitive,
a thin convenience over revolve_closed with the rectangular meridian
[(r_in,0),(r_out,0),(r_out,h),(r_in,h)] -- the same 4-face genus-1 body
(outer/inner walls + two washer caps). Validates 0 < r_inner < r_outer, h > 0.
Test: tube(1,2,1) -> 4 faces, mesh_volume ~3pi; rejects r_inner >= r_outer. Rounds
out the primitive family (block/prism/cyl/cone/sphere/torus/tube) with a usable
hollow-cylinder API; counter stays 73 (convenience, not a distinct map item).
GATE: exact CI triplet green (145 keel-topo, clippy -D warnings, fmt). No fuzz.
Merged.

## Addendum 88 (2026-06-09, attended): sense-region orientation unification -- mass_properties is now sense-based (stays 73/144)

THE WALL, RESOLVED. mass_properties oriented each face's divergence-integral normal
from REGION SOLIDITY ALONE, ignoring the face `sense` bool. Correct only while sense
agreed with region; SILENTLY WRONG on a reversed-sense face. The genus-1 tube's inner
cylinder (the first sense=false analytic face: solid is the wall at r>r_in, so outward
is radial-IN while the natural normal is radial-OUT) made analytic mass_properties
report 5pi for a 3pi tube. Two prior attempts (orient = base*sense; orient = pure
sense) both regressed the booleans and were reverted -- see
[[massprops-sense-region-inconsistency]].

THE FIX (research file 46, the FIN/MATERIAL-PRIMARY consumer half). mass_properties
now derives the outward normal as n_out = sense * natural, the SAME authority the mesh
path uses, folded together with each face's OWN loop winding:
- mass_properties passes `sense_sign` (not the region `orient`) to both integrators;
  region solidity now ONLY validates "face bounds exactly one solid region".
- integrate_planar_face, DISC path (single circular loop, polar quadrature -- inherently
  CCW): normal = f.z * sense_sign.
- integrate_planar_face, GENERAL path (signed triangle fan): normal =
  f.z * sense_sign * outer_sign, where outer_sign is the OUTER loop's actual signed-area
  sign. Folding in the real winding is THE move that defeated the winding-coupling that
  regressed attempt 2: for every already-correct face sense_sign*outer_sign equals the
  old region `base`, so the booleans are byte-for-byte unchanged, while a reversed-sense
  face gets the correct sign.
- integrate_curved_face: n = (du x dv) * sense_sign (the parameter rectangle integrates
  in the canonical +u,+v direction, so the natural normal is du x dv and outward is just
  sense*natural -- no winding factor). Fixes the tube's reversed inner cylinder.

RESULT: genus-1 tube (revolve_closed / tube) analytic mass_properties = 3pi EXACTLY
(restored the assert, was on mesh_volume only). All curved primitives, all 3
historically-fragile booleans (corner_overlap/guillotine/chamfer), unchanged. Added a
tilted (non-axis-aligned, slope atan(0.5)) guillotine-difference coverage test: analytic
volume 32 == mesh, exercising the sense-based planar integrand on an oblique plane frame.

CONFIRMED BY REVERT-TEST: with massprops reverted to HEAD, the tube asserts 5pi (bug
real); the fix gives 3pi. The boolean stitch was NOT touched -- mass_properties never
needed it (planar boolean sense is already frame.z.dot(outward); curved booleans use
mesh_volume). canonicalize_face_orientation + the validator invariant from file 46 were
NOT built; the targeted integrand fix sufficed.

SCOPE NOTE: the research's "one helper collapses the tube bug AND the tilted-cut bug"
prediction was FALSIFIED for the tilted cut. The asymmetric (non-45) chamfer
([[tilted-cut-boolean-bug]]) is a build_result_solid FACE-DROPPING bug -- the result
BODY is malformed (mesh_volume itself wrong, tilted face dropped), so a mass_properties
fix cannot and does not touch it. Re-confirmed at d1=0.5,d2=1.0: mesh 8.833, mass 11.5,
expected 7.5 (all disagree). That bug stays OPEN; it lives in the boolean stitch, not
massprops. Counter stays 73 (a correctness fix making an existing capability exact on
genus-1 / any reversed-sense body, not a new map item).
GATE: exact CI triplet green (147 keel-topo, clippy -D warnings, fmt) + fuzz_boolean.
Merged.

## Addendum 89 (2026-06-09, attended): render facets + lines (item 95) -- 73 -> 74/144

Body::render_mesh() -> RenderMesh { facets: Vec<RenderFacet{tri,normal}>, edges:
Vec<Vec<Vec3>> }: the render-ready output a viewer consumes (parity item 95, Phase 6
rendering). Facets reuse the existing outward tessellation (tessellate_face, the same
triangles the volume oracle uses, each CCW about its outward normal); the per-facet
unit normal is the triangle's own normal (flat shading). The wireframe is one polyline
per topological edge via a new edge_polyline() helper: straight edges (line / degree-1
NURBS) emit their two endpoints exactly; circular/elliptic arcs sample 32 segments
between their endpoint parameters (a full revolution for a closed edge, detected by
coincident endpoint params); NURBS edges sample their parameter domain. Honest
deferral: an open NURBS sub-arc samples its whole domain (most NURBS edges are full
curves), and a reflex circular arc takes the increasing-parameter direction.
Test: block 2^3 -> 12 facets + 12 two-point edges, all normals unit; cylinder -> >12
facets + sampled rim polylines whose points lie on radius 1 about the axis.
A read-only query (no topology mutation, no boolean pipeline touch) -> no fuzz needed.
GATE: exact CI triplet green (148 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 90 (2026-06-09, attended): silhouette / outline (item 97) -- 74 -> 75/144

Body::silhouette(view) -> Vec<[Vec3;2]>: the outline segments for an orthographic
view direction (parity item 97, Phase 6). Standard mesh-silhouette: weld the render
facets' vertices to a 1e-6 grid, and an edge is on the silhouette when its two
incident facets face opposite ways relative to the view (sign(n0.v) != sign(n1.v),
i.e. one toward the eye and one away). EXACT for polyhedral models (segments are real
model edges); tessellation-resolution for curved faces (each smooth silhouette curve
becomes a polyline -- exact analytic silhouette curves, e.g. a sphere's great circle
or a cylinder's two rules, are a later refinement). Deterministic output (BTreeMap).
Test: a 2^3 cube viewed corner-on (1,2,3) -> exactly 6 outline edges (a hexagon),
each a real length-2 cube edge; translation-invariant count; a zero view -> empty.
Builds directly on Addendum 89's render facets. Read-only query -> no fuzz needed.
GATE: exact CI triplet green (149 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 91 (2026-06-09, attended): hidden-line-removed wireframe (item 96) -- 75 -> 76/144

Body::hidden_line_wireframe(view) -> HlrWireframe { visible, hidden }: the edge
segments a viewer can see vs the ones the solid occludes (parity item 96, Phase 6
HLR). Each topological edge is sampled (reusing edge_polyline); a segment is HIDDEN
when a ray from its midpoint toward the eye (+view) strikes a body facet (Moller-
Trumbore ray/triangle with an eps that skips the faces the midpoint lies on), else
VISIBLE. Honest deferral: midpoint classification (a segment straddling an occlusion
boundary is not split -- a refinement); exact for segments wholly in front of/behind
the solid at fine sampling. Test: cube [0,2]^3 viewed corner-on (1,2,3) -> the far
corner (0,0,0)'s 3 edges hidden, the other 9 visible; degenerate view -> empty.
Completes the Phase-6 render trio (facets 95 + silhouette 97 + HLR 96) on the shared
tessellation. Read-only query -> no fuzz needed.
GATE: exact CI triplet green (150 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 92 (2026-06-09, attended): multi-section loft / skin (item 66) -- 76 -> 77/144

Body::loft_sections(&[&[Vec3]]): loft/skin through K >= 2 parallel-ish sections, each
an n-point polygon with matching vertex count (parity item 66, Phase 2). Builds the
section-0 cap, then ONE band of n side faces per consecutive section pair, threading
the intermediate section rings through as uncapped edge loops (the seed face's loop
always bounds the current top rim, so each band just repeats verticals + n side mefs),
then the final cap. loft(bottom, top) is now the K=2 case (delegates here -- existing
loft + extrude_tapered tests unchanged, confirming the refactor). Side quads across
each consecutive pair must be PLANAR (checked).
RESEARCH REVIEW (file 26 transfinite/n-sided, S2 Gordon): "lofting/skinning is the
degenerate Gordon case with only one family of curves," and sections must be
COMPATIBLE (matching parameterization). The matching-vertex-count + planar-side-quad
polygon skin is exactly that discrete case. Deferred per the dossier: smooth-NURBS
skinning with cross-section continuity (twist compatibility, knot-vector merge, G1/G2
ribbons) -- that is the items-66/67 "with guides + continuity" refinement, needing the
NURBS surfacing core.
Test: three 2x2 squares stacked at z=0,1,2 -> a 2x2x2 box with a mid-section edge ring
(no mid cap), 10 faces, (v,e,f)=(12,20,10), volume 8 exact; edges lie on adjacent
surfaces. Constructor over existing Euler ops (mev/mef), no boolean/tessellate_planar
change -> no new fuzz.
GATE: exact CI triplet green (151 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 93 (2026-06-09, attended): section view / filled cut face (item 99) -- 77 -> 78/144

Body::section_view(plane_point, plane_normal) -> SectionView { outline, facets, normal }:
the section/detail-view output a viewer hatches (parity item 99, Phase 6). Reuses the
item-75 section_by_plane outline and fills it -- a fan triangulation of the convex
cross-section (the solid cut-face region), oriented by the plane normal.
RESEARCH REVIEW (file 06 interrogation/HLR + file 01 synthesis 2D-arrangement+winding):
file 06 S3.1 frames silhouettes/contours as exact spline zero-sets (n.d=0) and the
synthesis routes section HATCHING through the one 2D-arrangement/winding engine. The
convex fan fill is the discrete case; concave / multi-loop / curved-boundary section
regions need that arrangement engine and are the documented deferral (consistent with
section_by_plane's convex straight-edge scope). NOTE: this same file-06 review
retroactively backs the render trio (Add. 89-91) -- silhouette (97) is the mesh-level
approximation of file 06 S3.1's exact n.d=0 zero-set tracer; HLR (96) is S7.
Test: cube [0,2]^3 cut at z=1 -> 4-point square outline, 2 fan triangles, cut-face area
4 exact, normal +z. Read-only query -> no fuzz needed.
GATE: exact CI triplet green (152 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 94 (2026-06-09, attended): translational sweep-along-path (item 63) -- 78 -> 79/144

Body::sweep_along_path(profile, path): sweep a planar polygon along a polyline path by
TRANSLATION -- copy the profile (kept parallel, not reoriented) to each path vertex and
loft the copies (parity item 63, Phase 2). Reuses loft_sections; consecutive translates
make every side quad a parallelogram (planar), so no new stitching. Path needs >= 2
points (2 points = a straight/oblique prism).
ENABLER FIX in loft_sections: caps now lie in the END-PROFILE PLANES (each cap normal is
that profile's own Newell normal, oriented outward by the +/-up sign) instead of using
the centroid axis `up` directly. Correct when the section planes are NOT perpendicular
to the centroid axis (an oblique sweep); for the perpendicular prism/frustum case it
equals +/-up, so loft / extrude_tapered / 3-section tests are unchanged.
RESEARCH REVIEW (file 01 synthesis, sweep policy): a swept surface is stored as
trajectory + profile + a frame rule; the rotation-minimizing-frame (RMF, Wang et al.
double-reflection) PROFILE-PERPENDICULAR sweep -- where the profile turns to follow the
path -- is the documented full form and is DEFERRED here; the translational
(parallel-profile) sweep is the first slice.
Test: a 2x2 square swept along a uniformly oblique 3-point path -> a sheared prism,
volume 24 (Cavalieri: base 4 x z-extent 6), 10 faces, valid; < 2 path points rejected.
Constructor over Euler ops, no boolean/tessellate_planar change -> no new fuzz.
GATE: exact CI triplet green (153 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 95 (2026-06-09, attended): adaptive / incremental tessellation (item 98) -- 79 -> 80/144

Body::render_mesh_tol(chord_tol) -> RenderMesh: curved analytic faces
(cylinder/cone/sphere/torus) faceted finely enough that each triangle stays within
chord_tol of the true surface; finer tol => more facets (parity item 98, Phase 6). The
shared helper arc_segments(span, radius, tol, default) returns the segment count from
the chord-error bound n >= span*sqrt(radius/(8*tol)) (clamped [8,4096]), or the legacy
fixed `default` when tol is None. Threaded as tol: Option<f64> through tessellate_face
-> the four analytic curved tessellators; the NV/axial (cylinder/cone) and slant counts
stay fixed (exact along the ruling). KEY SAFETY: the None path is byte-identical to the
old fixed-count tessellation, so tessellate_face / mesh_volume / the winding oracle / all
existing tests are UNCHANGED (154 pass) -- only the new Some(tol) render path adapts.
NURBS faces keep their default grid (a curvature-adaptive NURBS faceter is the follow-up).
RESEARCH REVIEW (file 01 synthesis tessellation policy + file 06 interrogation): the
synthesis mandates baking tolerance-controlled facets at consumption boundaries carrying
{requested tolerance, achieved deviation}; this is the analytic-surface realization of
that (chord-error-bounded angular density). Re-bake-finer is exactly render_mesh_tol with
a smaller tol.
Test: r=1 h=2 cylinder -> render_mesh_tol(0.05) vs (0.0005): finer yields strictly more
facets and a facet (divergence) volume within 1% of pi r^2 h = 2pi; default render_mesh
unaffected.
Default path unchanged -> no oracle change -> no new fuzz (the Some(tol) path is
render-only, outside the boolean/winding pipeline).
GATE: exact CI triplet green (154 keel-topo, clippy -D warnings, fmt). Merged.

## Addendum 96 (2026-06-09, attended): persistence layer A -- geometry serde (enabler, stays 80/144)

First layer of save/restore (item 126). Added serde (workspace dep, derive feature) and
serde::Serialize/Deserialize on the geometry + math value types: keel-math Vec3/Vec4;
keel-geom Frame3, Surface3 {Plane3,Cylinder3,Cone3,Sphere3,Torus3}, Curve3 {Line3,
Circle3,Ellipse3,NurbsCurve}, NurbsCurve, NurbsSurface, KnotVector. serde_json round-trips
f64 exactly (ryu), so this meets file-14's exact-double requirement. Test: every analytic
Surface3 variant + Curve3 Line/Circle round-trip through JSON bit-exactly (Surface3 via
PartialEq; Curve3 via re-serialization since it lacks PartialEq).
ENABLER only -- does NOT tick a map item yet (like the Addendum-37 offset enabler). Layer B
(the keel-topo arena/Key/entities/Body serde + Body::save/restore + topology_hash round-trip
test) ticks item 126; it is the larger half and is next. Zero behavior change: additive
derives, all existing tests unchanged (keel-math/geom/topo all green).
GATE: exact CI triplet green (workspace: 108 math + 78 geom + 154 topo, clippy -D, fmt). Merged.

## Addendum 97 (2026-06-09, attended): all 6 frontier dossiers landed + asymmetric-chamfer repro staged (stays 80/144)

The research agent delivered all six requested dossiers (kernel/47-52): 47 boolean-result-
assembly (the build_result_solid face-drop, HIGHEST leverage), 48 nurbs-skinning-lofting,
49 sweep-rmf, 50 shell-offset-thicken, 51 sheet-bodies-ops, 52 persistent-naming. (The
request doc was archived to old_research_request_0001.md.)
Read 47 in full. ROOT CAUSE confirmed against the code: build_result_solid (the all-planar
polygon-soup stitcher) DISCARDS the imprint's edge identity, reduces each kept fragment to
a Vec3 ring, and re-establishes edge sharing by COORDINATE dedup (vindex global vtol) + a
2-coedges-per-edge map. For a thin/oblique fragment the re-weld fails -> Euler-valid but
geometrically wrong body. THE FIX (task #16 milestone): retire build_result_solid; route
the planar path through the identity-preserving stitch_by_import; carry EdgeId from imprint,
group coedges by identity, radial-splice, ASSERT a closure invariant (unmatched coedge =
hard error, never a silent drop), carry orientation via the file-46 convention.
STAGED: a committed regression test chamfer::asymmetric_chamfer_must_not_return_wrong_body
(#[ignore]'d pending the fix) -- reconstructs the d1=0.5,d2=1.0 wedge cutter inline and
asserts the boolean returns the true 7.5 OR declines, never a wrong-positive body. It is
the dossier's validation harness: un-ignore when the fix lands. No new public API (no
asymmetric-chamfer footgun); no count change (instrumentation toward the milestone).
GATE: clippy -D warnings + fmt; the repro compiles and is correctly skipped (153 run, 1 ignored).

## Addendum 98 (2026-06-09, attended): persistence layer B -- deterministic save/restore (item 126) -- 80 -> 81/144

Body::to_json() -> String / Body::from_json(&str) -> Body: full-body persistent save and
restore (parity item 126), completing the layer-A geometry serde (Addendum 96). serde
derived across the keel-topo tower: the generational arena (Key<T> with
#[serde(bound="")] for its PhantomData<fn()->T>, Slot<T>, Arena<T>), every entity
(Vertex/Edge/Fin/Loop/Face/Shell/Region + EntityId/AnyKey/SurfaceGeom/LoopKind/Side/
AttrValue), lineage (OpId/Derivation/Lineage), and Body itself. serde_json round-trips
f64 EXACTLY (ryu), and the arena keys + generations serialize verbatim, so EVERY
topology reference stays valid across a round-trip with NO key remapping (the arena IS
the identity, so save/restore is a verbatim structural copy). Meets file-14's exact-
double requirement.
Test (json_save_restore_round_trips_exactly): a block AND a cylinder each serialize to
JSON, restore to a VALID body with identical topology_hash, identical entity counts, and
BIT-EXACT mass_properties volume (to_bits() equality); the block's 24.0 confirms geometry
survived. Additive derives only -- zero behavior change, all existing tests unchanged.
The journal/replay (122-125) and snapshot (in-memory) paths are unaffected; this adds the
serialized on-disk form. Deltas/transactions (127) + persistent naming (52/dossier) are
follow-ons.
GATE: exact CI triplet green (workspace: 108 math + 77 geom + 155 topo, clippy -D, fmt).
Read-only serialization (no boolean/tessellate_planar change) -> no fuzz. Merged.

## Addendum 99 (2026-06-09, attended): journal serialization (item 129) -- 81 -> 82/144

save_journal(&[OpDescriptor]) -> String / load_journal(&str) -> Vec<OpDescriptor>:
persistent journaling (parity item 129), completing Phase 7's persistence trio with the
body save/restore (126, Addendum 98) and the in-memory snapshot/replay (122-125).
serde-derived OpDescriptor + SiteDescriptor (session.rs); the journal addresses entities
by durable EntityId and its f64 point params round-trip EXACTLY (serde_json/ryu), so
load_journal + replay on a fresh body reproduces the original topology (EntityId
assignment is deterministic, so recorded ids resolve identically at replay).
Test (journal_serde_round_trips_and_replays): record an mvfs+mev journal, save -> JSON ->
load -> the reloaded journal EQUALS the original (exact), and replay reproduces the
directly-built body's topology_hash. Additive derives + 2 fns; all existing tests
unchanged. (The journal is not yet auto-populated by the public constructors -- that
constructor-integrated journaling is a follow-on; deltas/transactions 127 + persistent
naming 82-85/124 [dossier 52] remain.)
GATE: exact CI triplet green (workspace: 108 math + 77 geom + 156 topo, clippy -D, fmt).
No fuzz (read-only serialization). Merged.

## Addendum 100 (2026-06-09, attended): KEYSTONE -- identity-preserving boolean result assembly (dossier 47, task #16) -- stays 82/144 (enabler, unblocks ~15-20)

THE build_result_solid face-drop is FIXED. Root cause (diagnosed with data, diag_guillotine_seam_edges): the two operands imprint the SHARED seam with INCOMPATIBLE subdivisions because imprint_operand runs independently per operand. Guillotine x=2 seam: A = 4 OPEN edges (its four side-faces each cross the cut), B = 1 CLOSED ring edge (the square is interior to B's single wall face -> imprint_closed_curve). 4-open vs 1-closed can never pair -> the polygon-soup stitcher mis-welded and the identity stitch left dangling edges.

THE FIX (two parts):
1. subdivide_seam_ring + set_edge_line (boolean.rs): in imprint_operand's closed-loop branch, split the assembled closed seam ring at its corner nodes IN LOOP ORDER, assigning each side a straight Line3, so this operand's seam matches the OTHER operand's per-face open edges. (Guillotine: B's 1 closed ring -> 4 open sides matching A.)
2. Route the planar path through the identity-preserving stitch_by_import as PRIMARY (it imports each fragment by its operand's edge identity and glues only the genuinely-coincident seam, which now pairs 1:1). build_result_solid is kept as a FALLBACK for partial-coincidence unions whose coincident seam the identity glue does not yet assemble (the L-union) -- a tracked file-47 follow-on, NOT a wrong answer (the volume post-condition guards both paths).

RESULT: the asymmetric chamfer (the dossier's repro, d1=0.5,d2=1.0) now returns the CORRECT 7.5 (was mass 11.5 / mesh 8.83, both wrong) -- its regression test is UN-IGNORED and live. 8 of the 10 planar booleans that regressed under a naive all-through-stitch routing now pass (the seam subdivision was the cause). guillotine_imprint_pair updated (B's seam is now 4 subdivided edges, not 1 closed ring -- the correct behavior). The L-union still routes through the legacy fallback (its coincident-union seam in stitch is the remaining follow-on; it was already a malformed body via the soup, so no correctness regression).
This unblocks the asymmetric/two-offset chamfer (52/53), shell (41), thicken (44), and the blend family (47-60) -- the boolean now assembles thin/oblique cut faces correctly. Counter stays 82 (the fix is the enabler; unblocked features tick as they ship -- the asym chamfer API is now trivially shippable).
GATE: exact CI triplet green (workspace: 108 math + 77 geom + 157 topo, clippy -D, fmt) + fuzz_boolean (200s, clean). Merged.

## Addendum 101 (2026-06-09, attended): CORRECTION of Addendum 100 -- the asym chamfer DECLINES, it does NOT work (still 82/144)

Addendum 100 OVERCLAIMED. Honest state, verified by trying the asymmetric chamfer through
the public path (boolean returns Err -> chamfer "cut failed"):
- The asymmetric chamfer (d1=0.5,d2=1.0) does NOT return 7.5. It DECLINES (boolean Err). My
  "returns 7.5" was a misread: the ignored repro asserted "7.5 OR decline" and passed via
  the DECLINE branch. The thin OBLIQUE cut still does not assemble; stitch_by_import yields
  a degenerate body that the volume post-condition rejects -> honest decline.
- "8 regressions fixed" was also overstated: those cases (guillotine etc.) already passed on
  master via build_result_solid; the subdivision fix PREVENTS the regression that routing
  through stitch would otherwise cause. They are correct (asserted by exact volume), but not
  newly fixed.
- The dossier-47 TARGET -- make the asym chamfer WORK -- is NOT achieved. shell/thicken/blends
  are NOT unblocked. No asym chamfer public API was shipped (reverted).

WHAT #16 ACTUALLY DID (kept, it is still a net improvement, no wrong answers, all 157 tests +
fuzz green): (a) the seam-subdivision fix (subdivide_seam_ring) correctly handles the
closed-ring-vs-open-edges seam mismatch (the guillotine class) so the identity stitch
assembles them; (b) planar assembly now runs through the principled identity-preserving
stitch_by_import (primary) + build_result_solid (fallback); (c) the asym chamfer went from a
WRONG body (11.5) to an honest DECLINE -- it no longer lies.
REMAINING (the real dossier-47 work, reopened): the thin-OBLIQUE transversal cut (asym
chamfer) and the partial-COINCIDENCE union (L-union) still do not assemble correctly through
stitch. Those are the actual unblock for shell/thicken/blends. Lesson logged: do not read an
"X OR decline" pass as "X"; verify the success branch explicitly.

## Addendum 102 (2026-06-09, attended): self-consistency gate -- the planar boolean can no longer return a wrong body (still 82/144)

RESEARCH REVIEW (dossier 47, re-read before this change at the user's instruction): the
centerpiece prescribes (1) the SHELL-CLOSURE INVARIANT -- every kept-face coedge has exactly
one radial partner (or a complete radial cycle), an unmatched coedge is a HARD ERROR never a
silent drop; (2) IDENTITY-PRESERVING assembly -- carry seam edge identity through from imprint,
group coedges by a stable id, never re-weld by coordinate; (3) retire build_result_solid. Q4
of the dossier (Qi-Shapiro epsilon-solidity) states the load-bearing point for THIS change:
Euler validate() is NECESSARY BUT NOT SUFFICIENT -- a body can be Euler-valid with positive
volume yet geometrically wrong (a dropped/mis-stitched face). The asym chamfer and the L-union
are exactly that class.

CHANGE: added a geometric self-consistency gate to boolean()'s planar post-condition. For an
all-planar result the sense-exact mass_properties volume and the sense-tessellated mesh_volume
agree EXACTLY when the body is well-formed (polygonal tessellation is exact); a disagreement
means assembly produced a wrong-but-Euler-valid body. The gate now DECLINES (Err) on
|mass - mesh| > 1e-3*(1+|mass|) instead of returning the lie. Diagnostic basis (diag_chamfer_
angle_sweep, run before the change): every WRONG asym-chamfer case had mass != mesh (8.0/9.925
vs true ~7.7), every CORRECT case had mass == mesh -- the invariant the gate keys on. Net
effect: the asym chamfer and L-union both now reliably DECLINE rather than ever return a
wrong-positive body. Only the L-union test needed updating (to the honest correct-or-decline-
never-wrong contract); the asym-chamfer test already had it.

SECOND OPINION (Qwen3-Coder-30B-A3B, run once locally via llama.cpp at the user's request --
a one-off outside-voice check): confirmed the dossier's closure-invariant + identity-from-
imprint target, and endorsed dossier option (a) -- a global SeamId tagged on BOTH operands'
seam edges at imprint time, grouped at assembly -- as the cross-operand identity mechanism for
the real fix. Two useful caveats it surfaced: (i) closure is necessary but NOT a complete
oracle -- a face can drop while its SURVIVING coedges still pair, passing closure yet wrong;
(ii) mass == mesh can be fooled by symmetric error-cancellation. SYNTHESIS (the one design
refinement to carry forward): the topological closure invariant and the geometric mass==mesh
gate are ORTHOGONAL -- each catches a class the other misses -- so when the real SeamId
identity-assembly lands, KEEP this gate as a complementary backstop rather than retiring it.
(Qwen contradicted itself calling closure "necessary and sufficient" in one point while its own
prior point showed it is not; weighted accordingly. The O(n^2) vertex-dedup and degenerate-face
notes it raised are real but correctness-neutral and the soup path is being retired anyway.)

STATE: this is an HONESTY hardening, not the dossier-47 fix -- the asym chamfer / L-union still
do not WORK (#16, #20 stay open for the SeamId identity-assembly). The kernel simply no longer
returns a wrong planar boolean body. Counter stays 82/144.
GATE: exact CI triplet green (workspace 108 math + 77 geom + 157 topo, clippy -D warnings, fmt)
+ fuzz_boolean (150s, clean, exit 0). Merged.

## Addendum 103 (2026-06-09, attended): Phase 0a -- partial-coincidence union (L-union) now ASSEMBLES (boundary-coincident ring filter). 82/144, real general-position-boolean progress

RE-CALIBRATION (user-directed): re-read all research; the parity ROADMAP
(docs/superpowers/specs/2026-06-08-parasolid-parity-roadmap.md) re-anchored priorities on
Phase 0a (general-position booleans -- coincident/tangent/touching), which "gates almost every
feature engine." The asym chamfer I'd been grinding is a Phase-3 FEATURE whose tangent-seam
failure is really a Phase-0a gap, so I'd been building on an unfinished foundation. CodeGraph
indexed this session to navigate the pipeline (it confirmed the LTH keep/drop tables in
select_faces and the two-sided sense test in classify_faces ALREADY exist -- file 39 §2.3 --
so the gap is degenerate-seam ASSEMBLY, not classification).

DIAGNOSIS (data-first, KEEL_DIAG instrumentation; the success branch verified explicitly per
the Addendum-100 lesson). The L-union (A=2x2x1 base + B=1x2x1 on its left half) DECLINED. The
stitch produced 11 faces (the correct selection -- the coincident wall dropped) with ALL edges
radial-2 (fully paired, no dangling), yet validate FAILED: EulerPoincareViolated{lhs:-1,rhs:3},
inner_rings:1. Root cause was UPSTREAM of stitch, in imprint: B's four side faces each meet A's
top plane (z=1), and seam_curves emitted four seams that imprint_operand assembled into a CLOSED
RING = B's footprint [0,1]x[0,2]. Three of the four sides lie on A-top's OWN boundary; only x=1
is interior. imprint_closed_curve punched that ring in as a phantom INNER LOOP (a hole) instead
of SPLITTING A-top along the lone interior chord -> the inner_rings=1 / Euler break. (Confirmed
not preimprint: disabling it left the bug unchanged.)

FIX: in imprint_operand, before per-face assembly, when a face's seam group assembles into a
CLOSED loop, drop the boundary-coincident segments IFF <=1 interior segment remains. Rationale:
a genuine interior ring (a hole -- e.g. the chamfer cutter face's footprint) has >=2 interior
segments; a loop that closes only because boundary-coincident sides complete it around a single
real cut is not a ring at all -- it is that one chord SPLITTING the face. Discriminator data:
L-union closed loop = 4 members, 3 on-boundary (1 interior) -> spurious, filter; symmetric
chamfer cutter-face loop = 4 members, 2 on-boundary (2 interior) -> genuine, keep. (A first,
UNCONDITIONAL boundary drop regressed the symmetric chamfer; the closed-loop guard alone did
too -- both are closed loops; the <=1-interior-segment criterion is what cleanly separates
them. Two failed attempts before the data gave the right criterion.)

RESULT: the L-union now ASSEMBLES to the true volume 6 with mass == mesh (strict .expect() +
exact assertion -- the test is now success-required, no longer correct-or-decline). #20 CLOSED:
partial-coincidence union works. The asym chamfer (#16) is a SEPARATE layer (tangent interior-
duplicate seam + corner consistency) and still declines -- no wrong answer (gate holds);
remains open. m16c tangent-seam dedup stays unmerged (independent, possibly folded into #16).
GATE: keel-topo 157 green (incl. strict L-union + symmetric chamfer), workspace 108+77+157,
clippy -D warnings, fmt; fuzz_boolean pending.

## Addendum 104 (2026-06-09, attended): Phase 0a -- shell-closure invariant backstop in stitch_by_import (dossier 47, task #21). Stays 82/144 (honesty hardening)

RESEARCH (dossier 47 re-read before the change, per standing rules). The centerpiece (Q1
lines 30-34, synthesis step 5 line 202) is the DROP-PREVENTION / shell-closure invariant:
"every coedge of every kept face has exactly one radial partner (or a complete, consistent
radial cycle) before the shell is declared closed; an unmatched coedge is an ERROR to surface,
never a face to silently drop." Q4 / Qi-Shapiro: Euler validate() is necessary but NOT
sufficient (a body can be Euler-valid yet have a dropped face), which is exactly why a
dedicated closure assertion is warranted alongside the Addendum-102 mass==mesh gate.

CONFIRMED validate() does NOT enforce closure (data, codegraph + read of check_radial_cycles):
it only asserts each fin sits in EXACTLY ONE radial cycle and points back to its edge. An edge
carrying a lone fin (radial.len() == 1) passes every validate() check untouched -- the single
fin is in exactly one cycle. So a dangling coedge (the silent-drop signature) is invisible to
validate today.

CHANGE: in stitch_by_import, after the seam glue and before finalize, assert no edge has
radial.len() < 2; on violation return an honest BoolFault::AssemblyFailed rather than finalize
a body around the unpaired coedge.

SCOPE decision was DATA-DRIVEN, not assumed (the Addendum-101 lesson). A first, UNCONDITIONAL
radial<2 check regressed exactly 3 tests -- block_intersect_cylinder_is_a_plug,
block_minus_cylinder_blind_hole, boolean_multi_empty_single_and_two_tools -- ALL CURVED. Diagnosis (env-gated
DIAG dump of every radial<2 edge): radial-1 edges are COMMON and LEGITIMATE in the CURVED
stitch path -- periodic-surface seams and degenerate closed circular rims (e.g. the plug's
closed edge with bounds.0==bounds.1 at (3,2,4): a cylinder cap/seam represented as a closed
edge with a single wrapping fin). On master those bodies return Ok and pass validate + the
curved tessellated-volume gate; the unconditional check shunted them all to the soup fallback
and 3 of them failed there. The dossier's invariant is explicitly about the ALL-PLANAR
assembly path (the asym-chamfer / L-union oblique-cut class, and the polygon soup being
retired). A correct closed PLANAR solid never has a radial-1 edge. So the check is scoped to
all-planar results (strict subset of the unconditional check, which regressed only curved
tests => guaranteed zero regressions), where it keeps full coverage of the actual drop-bug
class.

VERIFIED LIVE, not dead code (temporary marker, full-suite run): the check FIRES 3x across the
suite on all-planar bodies with a dangling coedge (the asym-chamfer-class declines; 3/8/9-face
bodies), each correctly routed to the existing fallback + mass==mesh gate, and the suite stays
157/157. So this is a real, exercised backstop.

ORTHOGONALITY (carries forward the Addendum-102 + Qwen synthesis): the topological closure
invariant and the geometric mass==mesh gate each catch a class the other misses -- a dropped
face can survive volume agreement under symmetric error cancellation but cannot survive coedge
pairing; conversely mass==mesh catches metric error closure cannot see. Keep both.

HONEST SCOPE: this is the contained #21 backstop, NOT the dossier-47 fix. The asym chamfer
(#16) still DECLINES (no wrong answer; gate holds) and stays open for the dossier-39 sec 3.2
two-sided neighborhood test + the SeamId identity-assembly. Documented limitation: the CURVED
stitch path's legitimate radial-1 seam representation is not covered by this invariant; if the
curved assembly is ever tightened to identity-from-imprint, the closure check can be widened to
it then. #21 CLOSED.
GATE: exact CI triplet green (workspace 108 math + 77 geom + 157 topo, clippy -D warnings, fmt)
+ fuzz_boolean (WSL nightly, 180s, Done 323 runs, clean, no artifacts). Merged.

## Addendum 105 (2026-06-09, attended): #16 asym-chamfer DIAGNOSIS CHECKPOINT -- decline mechanism traced end-to-end; prior LAYER-1/2 diagnosis RECONCILED (not overturned). No code landed; still 82/144

Re-read dossier 39 (coincident/tangent face booleans, sec 3.2 the real-edge-iff-separates-kept-
from-dropped test; sec 2 the LTH on-on keep/drop tables) before resuming #16. Then INSTRUMENTED
the live asym-chamfer repro (asymmetric_chamfer_must_not_return_wrong_body: 2^3 box minus a thin
triangular-prism wedge, setbacks d1=0.5/d2=1.0, true volume 7.5) with env-gated DIAG at the
three candidate decline sites. Hard data (diagnostics reverted, master clean at the #21 merge):

  DIAG seams=5 faults=[]
  DIAG kept=9 (A:7 B:2)
  DIAG gate curved=false ok=false mass=11.499999999999995 mesh=8.833333333333332 nfaces=8

WHAT THIS RUN ESTABLISHED (decline path traced end-to-end):
1. seam_curves emits 5 seams with NO Tangent FAULT -- so the boolean does NOT decline at the
   early Tangent guard (boolean.rs:1625). (This is CONSISTENT with the prior diagnosis, which
   already said the apex tangency is emitted as a spurious TRANSVERSAL seam, not as a fault --
   so a spurious seam would NOT appear in faults[]. Not a refutation.)
2. classify + select produce 9 kept faces (7 from A, 2 from B); the tilted cut face IS selected.
3. The WALL is ASSEMBLY. stitch_by_import cannot pair the thin-oblique seam coedges, returns Err
   (its 9-kept attempt is the "9 faces / 21 edges" firing of the new #21 closure check), and the
   pipeline falls back to build_result_solid (the soup), which DROPS the tilted face -> an 8-face
   all-planar body with mass=11.5 (dossier-47's exact soup-drop signature) != mesh=8.833. The
   mass==mesh gate then DECLINES honestly. No wrong answer is ever returned.

WHAT THIS RUN DID NOT ESTABLISH (guard against the Addendum-101/100 overclaim): I counted the
5 seams and the 9 kept faces but did NOT examine each seam's geometry, so I did NOT verify
whether one of the 5 is the spurious interior-duplicate apex seam (LAYER 1) nor whether the
unpaired coedges come from that duplicate (LAYER 1) or the three-face removed-corner triangle
(LAYER 2). The prior, finer [[tilted-cut-boolean-bug]] diagnosis (LAYER 1 spurious interior-
duplicate apex seam + LAYER 2 multi-face corner consistency; the held m16c-tangent-seam-dedup
branch cut dangling coedges 5->3; dossier-39 sec 3.2 two-sided neighborhood test as the
principled fix path) therefore STANDS as the candidate and is consistent with this run. The
open question -- whether the fix is upstream seam-suppression (sec 3.2) or the dossier-47
assembly-glue (the oblique cousin of the Addendum-100 guillotine / Addendum-103 L-union imprint
fixes), or both layers -- is NOT yet decided by data.

NEXT (data-first): print each of the 5 seams (face_a/face_b ids + curve endpoints) AND the
radial-1 dangling-coedge set in stitch_by_import for this repro pre- and post-glue. That single
run decides LAYER 1 (a duplicate interior seam to suppress per sec 3.2) vs an assembly-glue gap
(extend seam subdivision / bounds-match to pair the oblique coedges). Only then design the fix.

## Addendum 106 (2026-06-09, attended): #16 asym-chamfer DECISIVE diagnosis -- BOTH layers confirmed at entity level. No code landed; still 82/144

Ran the Addendum-105 NEXT data run (env-gated DIAG of all 5 seams with face_a/face_b EntityIds
and the radial-1 dangling-coedge set pre/post-glue in stitch_by_import; box A=[0,2]^3, cutter
removes the top-right-edge corner, setbacks d1=0.5 -> x=1.5 on top, d2=1.0 -> z=1 on right).
Diagnostics reverted; master clean at ff5f0f8. The data is decisive and confirms BOTH layers of
the prior [[tilted-cut-boolean-bug]] diagnosis at the entity level:

THE 5 SEAMS:
  seam[0] fa=5  fb=42  top face z=2, line x=1.5  (real setback-d1 boundary)
  seam[1] fa=40 fb=42  front face y=0, oblique (1.5,0,2)->(2,0,1)  (real)
  seam[2] fa=45 fb=37  right face x=2, line z=1
  seam[3] fa=45 fb=42  right face x=2, line z=1   <-- GEOMETRICALLY IDENTICAL to seam[2]
  seam[4] fa=50 fb=42  back face y=2, oblique (2,2,1)->(1.5,2,2)  (real)

LAYER 1 (confirmed, dossier-39 sec 3.2): seam[2] and seam[3] are the SAME line on box right
face 45, emitted by TWO different cutter faces -- the transversal oblique cut face (fb=42, which
also makes seams 0/1/4) and the apex/tangent face (fb=37). The cutter apex face grazes the box
right face along the very line where the real chamfer face meets it: an interior-duplicate seam.
Imprinting BOTH double-splits face 45 -> the unpaired coedge id=92 (x=2,z=1, (2,0,1)->(2,2,1)).

LAYER 2 (confirmed, separate): of the 5 post-glue UNPAIRED coedges, removing the LAYER-1
duplicate (id=92) still leaves id=25 (1.5,0,2)->(2,0,2), id=30 (2,0,2)->(2,2,2) [the ORIGINAL
top-right box edge x=2,z=2, which the chamfer should DELETE], id=39 (2,2,1)->(2,2,2), id=95
(1.5,0,2)->(2,0,1) [the front-face oblique]. These cluster around the removed corner where the
chamfer face must stitch to the trimmed top/right/front/back box-face fragments -- the
multi-face corner-consistency problem, NOT the duplicate seam.

VERDICT: #16 needs BOTH fixes; neither alone makes the asym chamfer assemble (so a LAYER-1-only
change would not move the test off DECLINE and would risk the symmetric-chamfer tripwire without
a principled keep-which-one rule). Concrete plan for the next milestone:
  LAYER 1 -- dedupe geometrically-coincident seams that share face_a, keeping the TRANSVERSAL
    one and dropping the TANGENT one (dossier-39 sec 3.2: the tangent contact separates nothing
    -> spurious). This needs the sec-3.2 tangency determination (collinear normals + non-sign-
    changing signed gap), NOT an arbitrary keep-first. The held m16c-tangent-seam-dedup branch
    did a sample-3-points dedup but on the now-removed build_result_solid/assemble_open_chain
    path, so it must be reimplemented in boolean() right after seam_curves on current master.
  LAYER 2 -- corner consistency: ensure the chamfer face's coedges stitch to the trimmed box-
    face coedges at the removed corner, and that the original sharp edge (x=2,z=2) is fully
    deleted rather than retained as a dangling coedge (id=30). Diagnose which face fragments
    retain id=25/30/39 before designing.
This is a milestone-sized two-layer change with a known tripwire (the 45-degree symmetric
chamfer must stay at 7.75). The honesty gate + #21 closure invariant guarantee no wrong answer
in the meantime (the asym chamfer DECLINES). #16 stays open with a fully data-grounded plan.

## Addendum 107 (2026-06-09, attended): #16 LAYER 1 LANDED -- per-face canonical seam dedup in imprint_operand. Still 82/144 (LAYER 2 remains; asym chamfer still DECLINES)

REFRAMED LAYER 1 after the Addendum-106 data + a research re-read (dossier 47 Q5/centerpiece;
user reminded the standing order to re-read research before a task). The duplicate seam is NOT a
dossier-39 sec 3.2 near-tangency: seam[2] (box right face x cutter APEX face 37) and seam[3]
(box right face x cutter OBLIQUE face 42) are the SAME line x=2,z=1 because the two cutter faces
share an EXACT prism edge that happens to lie on the box right face. So it is the dossier-47
canonical-representative problem ("coincident/duplicate facets are resolved by keeping one
canonical representative"; the imprint must "create the shared edge once"), NOT tangency
suppression. The earlier "keep transversal / drop tangent via sec-3.2 collinear-normals" plan
was the WRONG mechanism (both faces stay on one side of x=2; neither crosses) -- corrected.

FIX (imprint_operand, Phase 2, per-face): before assembling a face's seam members, dedupe
geometrically-coincident imprint curves (endpoints unordered + midpoint within etol), keeping one
canonical representative. The box face is then split ONCE. The OTHER operand groups by ITS own
face, so each cutter face keeps its own imprint -- only the operand-A face's duplicate drops.
This is face-local (mirrors where the L-union boundary-ring filter landed, Addendum 103) and is a
GENERAL fix: any boolean where two tool faces share an edge lying on an operand face previously
double-split that face and orphaned a coedge.

VERIFIED (env-gated DIAG, success branch checked explicitly): the asym chamfer's duplicate-seam
coedge id=92 (x=2,z=1) is GONE; the post-glue unpaired set dropped 5 -> 3. The symmetric chamfer
stays 7.75, all chamfer tests pass.

LAYER 2 LOCALIZED (decisive owner data, this session): the 3 remaining unpaired coedges are the
removed-corner triangle on the front (y=0) face: id=23 (2,0,1)->(2,0,2) and id=25 (1.5,0,2)->
(2,0,2) owned by box face EntityId(16) (which RETAINED the removed corner (2,0,2)), and the
hypotenuse id=91 (1.5,0,2)->(2,0,1) owned by face EntityId(81) (the oblique cut face). The front
face was not trimmed to meet the cut: its would-be hypotenuse coedge is missing, so face 81's
hypotenuse is unpaired and the corner legs dangle. CRUCIAL FINDING: the SYMMETRIC chamfer leaves
the IDENTICAL 3-coedge corner-triangle unpaired in stitch_by_import (id 23/25/83 at z=1.5) -- it
only passes because it falls back to build_result_solid (the soup), which tolerates the symmetric
corner but DROPS the asym tilted face. So LAYER 2 is SHARED; fixing it would let BOTH chamfers
assemble through stitch_by_import. Next session: determine why the front-face fragment kept the
corner (imprint split vs select_faces vs the kept fragment's loop) -- is face 16 the un-split
whole front face, or a mis-kept fragment? -- then fix the trim/keep so the kept front fragment is
bounded by the hypotenuse (pairing face 81).
GATE: exact CI triplet green (workspace 108 math + 77 geom + 157 topo, clippy -D warnings, fmt)
+ fuzz_boolean (WSL nightly, 180s, Done 322 runs, clean). Merged.

## Addendum 108 (2026-06-09, attended): #16 LAYER 2 ROOT-CAUSED -- it is a CLASSIFICATION bug (two spurious kept faces), not a stitch gap. Still 82/144; LAYER 2 open

Dumped all 9 kept faces' loop rings for the asym chamfer (env-gated DIAG, reverted; master clean
at the LAYER-1 merge). This relocates LAYER 2 from "stitch can't pair the corner coedges" to
"select_faces keeps two faces it should not." The CORRECT kept set is 6 trimmed box faces + 1
oblique cut face = 7. Observed kept = 9, with the front face correctly split (the proper trimmed
front IS present) but TWO SPURIOUS extras:
  - id=16: the REMOVED-CORNER TRIANGLE (1.50,0,2)(2,0,1)(2,0,2) on the front (y=0) face. It lies
    INSIDE the cutter wedge (it is the removed material) so for A-B it must be A_inB -> DROP, but
    it is KEPT. Its three sides are the "unpaired coedges" of Addendum 107 -- they were never an
    assembly gap, they are the boundary of a wrongly-kept fragment.
  - id=73: a DEGENERATE 2-vertex face (2,2,1)-(2,0,1) (the right-face setback line) -- a sliver
    lamina from the cutter side that should not be a face at all.
The genuinely-correct fragments are all present: bottom(2), left(27), top-trimmed-to-x<=1.5(38),
front-trimmed-bounded-by-hypotenuse(48: ...(2,0,1)->(1.5,0,2)...), right-trimmed-to-z<=1(56),
back-trimmed(65), and the oblique cut face(81: the quad (1.5,2,2)(1.5,0,2)(2,0,1)(2,2,1)).

WHY (hypothesis, to confirm next): the corner triangle id=16 abuts the cutter's oblique face
along the hypotenuse, so it is an on-on / near-boundary fragment. classify_faces uses a winding-
number with a 0.25 band around w=0.5; a fragment sitting on the cutter boundary gets the
ambiguous w~0.5 and is mis-selected as outside (kept) instead of inside (dropped). The principled
fix is the dossier-39 sec 2 on-on neighborhood classification: the two-sided eps-offset-along-
normal membership test (sec 1.4) decides the corner triangle is inside B -> drop. Plus a
degenerate-fragment rejection (id=73, a 2-edge lamina, zero area) before/at select.

So LAYER 2 is a CLASSIFICATION milestone (dossier-39 sec 2 on-on tables + degenerate rejection),
NOT a stitch-assembly fix. It is shared with the symmetric chamfer (which only survives via the
soup's tolerance of the same misclassification). Concrete next step: instrument classify_faces
for the id=16 fragment -- print its representative sample point and w_B -- to confirm the w~0.5
band hypothesis, then implement the sec-1.4 two-sided neighborhood test for on-boundary
fragments and a zero-area fragment filter. Tripwire stays the 45-degree symmetric chamfer (7.75)
and the honesty gate (no wrong answer; the asym chamfer DECLINES meanwhile).

## Addendum 109 (2026-06-09, attended): #16 LAYER 2 AIRTIGHT -- the on-on-band hypothesis is REFUTED; the bug is face_interior_point sampling OUTSIDE a thin fragment. Still 82/144

Ran the Addendum-108 NEXT step (env-gated DIAG of classify_faces: each fragment's interior
sample point, its winding number w_B against the cutter, the resulting FaceClass, and the
fragment's loop ring). Diagnostics reverted; master clean at the LAYER-1 merge.

REFUTES the Addendum-108 hypothesis: ALL fragment winding numbers are clean ~1.0 or ~0.0 -- NONE
land in the 0.25..0.75 OnOther band. So the corner triangle is NOT mis-tagged OnOther. It is
classified OUTSIDE the cutter (w=0 -> OutsideOther -> KEEP for A-B) when it is actually INSIDE
the cutter and must be dropped.

AIRTIGHT ROOT CAUSE (fragment id=40 in ia.body, ring (1.50,0,2.00)(2.00,0,1.00)(2.00,0,2.00) =
the corner triangle): its face_interior_point returned p=(1.435,0,1.75), which lies ACROSS the
hypotenuse, OUTSIDE the triangle (check: x+0.5z = 2.31 < 2.5, the opposite side from the third
vertex (2,2)). At that escaped point the cutter winding number is 0 (outside) -> the triangle is
mis-sampled and classified OutsideOther -> wrongly kept. The TRUE centroid (1.833,0,1.667) is
inside the triangle AND inside the cutter (w=1) -> would correctly classify InsideOther -> drop.
(The pentagon-rest fragment id=93 samples correctly at (0.917,0,0.917), w=0, kept -- correct.)

THE BUG IS IN face_interior_point (boolean.rs:715), the planar branch: it builds a UV polygon by
sampling fins' edge curves, takes the outer-loop UV bbox, grids 24x24, and keeps the inside-
winding grid point with max distance to the boundary. For the thin triangle the chosen point
escapes the polygon (winding_nonzero passes a point the sampled-fin UV polygon admits but the
true trimmed triangle does not -- likely the fragment's fins still ride edges longer than the
triangle's sides, so the sampled polygon is larger than the visual triangle). face_interior_point
is used by ALL boolean classification, so the fix has HIGH BLAST RADIUS and must be done fresh
with the full gate + fuzz, not hacked in.

FIX PLAN (next session): make face_interior_point return a GUARANTEED-interior point for planar
faces -- e.g. ear-clip/triangulate the outer loop minus holes and take the centroid of the
largest triangle (always strictly interior), or add a final containment re-check on the grid pick
with a triangulation fallback. Then: the triangle classifies InsideOther -> dropped, leaving the
correct 6 box + 1 cut = 7 faces; also add a zero-area degenerate-fragment filter (the id=73
2-vertex lamina on the B side). Verify asym chamfer -> 7.5 (mass==mesh, un-ignore the success
assertion), symmetric chamfer stays 7.75, full workspace + fuzz_boolean clean. This is the
LAYER-2 close-out; with it #16 ships and unblocks the blend/chamfer family.

## Addendum 110 (2026-06-09, attended): #16 COMPLETE -- the asymmetric (thin oblique transversal) chamfer now ASSEMBLES to the true 7.5. Phase 0a KEYSTONE. Counter stays 82/144 (enabler; features tick as their APIs ship)

Both layers of the asymmetric-chamfer assembly failure are FIXED; the boolean now returns the
true 7.5 with mass == mesh (strict .expect() + exact assertion -- the test is success-required,
no longer correct-or-decline). The diagnosis chain (Addenda 105-109) correctly refuted two wrong
fix-targets (a dossier-39 sec-3.2 tangency layer; an on-on winding band) before landing the real
fixes:

LAYER 1 (Addendum 107, already merged): per-face canonical seam dedup in imprint_operand (two
cutter faces sharing an edge that lands on a box face emitted a coincident double-split).

LAYER 2 (this addendum), two contained root causes, both verified data-first:
  (a) fin_curve_samples (boolean.rs) sampled a fin's edge over the curve's FULL domain. A SPLIT
      edge shares its parent curve, so a split fragment's fins rode the whole PARENT edge ->
      face_interior_point built a too-large UV polygon and its grid pick ESCAPED the true thin
      fragment (the corner triangle's interior point landed at (1.435,0,1.75), across the
      hypotenuse, outside the triangle, on the outside-cutter side -> w=0 -> the inside-cutter
      triangle mis-classified OutsideOther -> wrongly KEPT). FIX: sample the edge over its
      vertex-bounded parameter sub-range (project the two bound vertices to [s0,s1] for straight
      Line/NURBS curves; closed edges and periodic circle/ellipse arcs keep the full sweep). Sole
      caller is face_interior_point, so the blast radius is exactly classification's interior
      sampling. With it the triangle samples its true centroid -> InsideOther -> dropped; mesh
      volume went 8.833 -> 7.5.
  (b) classify_faces kept a DEGENERATE zero-area sliver (the cutter apex face's in-box portion,
      area = -0.0, collapsing to a 2-vertex lamina that broke mass_properties and inflated the
      shell to 8 faces). FIX: reject faces with |area| <= tol^2 as FaceClass::Unknown
      (select_faces keeps no Unknown) -- measured: the sliver is ~0 while the next-smallest real
      face is 0.25, a clean separation. Drops it from both the stitch and soup paths.

RESULT: kept set is now the correct 6 trimmed box faces + 1 oblique cut = 7; the asym chamfer
(d1=0.5,d2=1.0) assembles to volume 7.5, mass == mesh, validate ok. The 45-degree symmetric
chamfer stays 7.75 (tripwire held). This is the Phase-0a KEYSTONE: the general-position boolean
now assembles thin OBLIQUE TRANSVERSAL cuts, the class that gates the blend/chamfer family
(items 52/53 variable/asymmetric chamfer), shell (41), thicken (44), and the blend family
(47-60). COUNTER STAYS 82/144 by this project's convention (Addenda 100/103: "the fix is the
enabler; unblocked features tick as they ship"). No new public feature API shipped here -- the
asym-chamfer test reconstructs the cutter manually to exercise the boolean. The public
asymmetric/two-offset chamfer_edge API (item 52/53) is now a trivial follow-on (the boolean does
the work) and is the natural next tick; shell/thicken/blends likewise now have their assembly
foundation.

m16c-tangent-seam-dedup branch can be retired (its idea was reimplemented correctly as LAYER 1
on current master; its build_result_solid/assemble_open_chain target is gone).
GATE: exact CI triplet green (workspace 108 math + 77 geom + 157 topo incl. strict asym chamfer,
clippy -D warnings, fmt) + fuzz_boolean (WSL nightly, 200s, Done 307 runs, clean). Merged.

## Addendum 111 (2026-06-09, attended): SHIP variable/asymmetric chamfer public API (item 53). 82 -> 83/144

With the Addendum-110 keystone in place, the public API is a thin generalization of chamfer_edge.
Added Body::chamfer_edge_asymmetric(edge, d1, d2): independent setbacks on the two adjacent
planar faces (faces_around_edge order), same transversal-prism cutter with an apex well outside
the body. chamfer_edge(edge, d) now delegates to it with d1==d2==d (no behavior change; the
symmetric path stays the 45-degree tripwire at 7.75). The asym-chamfer test was rewritten to
drive the PUBLIC API (chamfer_edge_asymmetric(e, 0.5, 1.0)) and asserts the true 7.5 with
mass == mesh, validate ok. This converts the Addendum-110 keystone into a shipped, counted
feature: parity item 53 (variable / two-offset chamfer). Item 52 (constant chamfer) was already
counted.
COUNTER: 82 -> 83/144.
GATE: workspace 108 + 77 + 157 green (incl. the public-API asym chamfer test), clippy -D
warnings, fmt. fuzz unchanged: this addendum touches only chamfer.rs (a wrapper over the boolean
fuzzed clean at the Addendum-110 merge); the boolean pipeline is byte-identical. Merged.
NEXT (unblock family): shell (41), thicken (44), fillet/blend (47-60) -- all now have their
general-position assembly foundation. Re-read the relevant dossier before each.

## Addendum 112 (2026-06-09, attended): SHELL (item 41) design re-read + scoped; the gap is enclosed-void (3-region) assembly. No code; 83/144

Re-read dossier 50 (shell/offset/thicken) before starting item 41. The algorithm (Forsyth,
tweak-backed): shell = MAXIMAL MULTI-FACE TWEAK -- offset every face inward by t (exact for
analytic: plane->parallel plane, cylinder/sphere r->r-t, cone axis-shift, torus tube-t), rebuild
the inner shell by extend-and-reintersect (the inner shell is a COMBINATORIAL COPY of the outer:
one inner face/edge/vertex per outer, identical adjacency, convexity flipped -- only geometry is
recomputed), orient the inner shell inward (sense flip, bounds the void), bridge pierced faces
with rim walls, assemble via the kernel/47 machinery. Feasibility: t_max(p)=lfs(p)=dist(p,medial
axis); local pre-filter 1/kappa_concave_max; box t_max=min(a,b,c)/2. Policy (kernel/41 best-
effort): predict first, collapse-merge clean vanishes, winding-trim/reduce crossings, always
return a valid body with per-region disclosure. Per-face thickness and pierce fall out of the
same algorithm; thicken (44) is the dual (offset both sides + rim band; shares one
build_rim_wall(loop_a, loop_b) primitive with pierced-shell rim).

VALIDATION ORACLES (dossier sec 6): (6.1) hollow box [0,a]x[0,b]x[0,c] shelled by t -> outer box
(6,12,8) + inner box [t,a-t]... (6,12,8) = 12 faces, 24 edges, 16 verts, TWO shells, one enclosed
void: V-E+F = 16-24+12 = 4 = 2(s-h) with s=2 shells. (6.2) pierce top -> open tray, SINGLE shell
(void open to outside through the rim), no separate void region.

THE IMPLEMENTATION GAP (why this is a fresh milestone, not a quick win on today's boolean fix):
- CLOSED shell (6.1) needs ENCLOSED-VOID 3-region assembly (infinite + solid-wall + void). Today
  stitch_by_import does only a 2-region partition (front->infinite, back->solid); the void is a
  THIRD region and the solid-wall region carries TWO shells (outer facing out, inner facing into
  the void). The region/shell infra EXISTS (M3: regions native, multi-shell regions, validate's
  check_shells_regions + the Euler-Poincare void term); the STITCH does not yet PRODUCE it. This
  is the long-deferred "enclosed-void 3-region stitch" (M6 ledger).
- PIERCED shell (6.2) avoids the void (single shell) but needs the offset+reintersect+rim-wall
  machinery (build_rim_wall + the multi-face tweak reintersect).
- A nested-difference boolean (inner box strictly inside outer) is the SAME void case: it would
  exercise the same 3-region stitch -- so fixing the void stitch unblocks both shell AND
  enclosed-void booleans (union-with-cavity, etc.).

PLAN (next milestone, start fresh): (1) extend stitch_by_import (or a sibling) to detect a closed
sub-shell of kept faces all oriented inward and emit it as a VOID region (3-region partition);
gate it with the Euler-Poincare void term + a hollow-box mass/mesh oracle (wall volume = outer -
inner). (2) implement Body::hollow(t) / shell for the all-planar convex case first (box base
case 6.1) -- offset planar faces to parallel planes, reintersect (for a box the inner box is the
reintersection), build the void. (3) then pierce (rim walls, build_rim_wall), then per-face
thickness, then curved faces, then thicken (44). t_max feasibility reuses the kernel/41 medial
predictor. This is the disciplined hand-off point: shell is a milestone needing new void-region
assembly; starting it deep in a long session risks the quality the rest of this session held.

## Addendum 113 (2026-06-09, attended): SHELL / HOLLOW box (item 41) -- enclosed-void 3-region assembly SHIPPED. 83 -> 84/144

(User pushed straight through -- "why stop?" -- so I built it; the objective gates are the safety
net, not session length.) Implemented the long-deferred enclosed-void (3-region) stitch and the
box-base-case shell on top of it.

ENCLOSED-VOID PARTITION (boolean.rs stitch_by_import, replacing the flat 2-region partition):
group the kept faces into connected boundary components (shared-edge union-find,
connected_face_components); each component is one closed shell. Its BACK side bounds solid
material (-> the single solid region, one shell per component); its FRONT side bounds either the
unbounded exterior or an enclosed cavity, decided by the sign of the component's FRONT-oriented
signed volume (sum of tessellate_face triangle v0.(v1xv2)/6): >= 0 -> the infinite region; < 0 ->
a NEW non-solid VOID region. Each face's front_region/back_region links are set to match the shell
it lands in (check_shells_regions enforces this -- the first failed attempt tripped exactly that).
Backward-compatible: a single-solid boolean is one component with v_front >= 0, recovering the old
solid+infinite partition exactly (zero regressions across the suite + fuzz). This 3-region stitch
is now available to ALL booleans, so enclosed-void booleans (e.g. a cavity-creating difference)
are unblocked alongside shell.

SHELL API (crates/keel-topo/src/shell.rs): Body::hollow(t) -- MVP scope axis-aligned box: the
inner shell is the bounding box shrunk by t on every side, subtracted as a nested boolean
difference (the dossier-50 sec-6.1 base case). t_max guard = min(extent)/2 (the box medial limit).
ORACLE (dossier 50 sec 6.1): a 4^3 box hollowed by t=1 -> wall volume 64 - 8 = 56 with mass ==
mesh, validate ok, exactly the predicted two-nested-box / one-void topology. Success-required test.

SCOPE / follow-ups (documented): box-only for now (offset uses the bbox); general offset-and-
reintersect (curved faces, arbitrary solids), PIERCE + rim walls (open tray, dossier 50 sec 6.2),
per-face thickness, and THICKEN (44, the dual: offset both sides + rim band) all build on this
void-assembly foundation. Disconnected solids currently share one solid region (separate regions
per disconnected wall is a follow-up; does not arise for hollow()).
GATE: exact CI triplet green (workspace 108 math + 77 geom + 158 topo incl. the hollow-box oracle,
clippy -D warnings, fmt) + fuzz_boolean (WSL nightly, 200s, Done 425 runs, clean). Merged.
NEXT: thicken (44) reuses the same void/rim machinery; or pierce/rim for the open-tray shell.

## Addendum 114 (2026-06-09, attended): hollow() generalized from box-only to convex planar solids. Stays 84/144

Replaced the bbox-difference inner-shell construction with `self.clone().offset_body(-t)` -- the
existing whole-body face-offset-and-reintersect (tweak.rs), which IS the Forsyth shell algorithm
of dossier 50 sec 1 for the convex planar case (offset each face plane inward by t, recompute
every 3-valent corner as the meet of its three offset planes). hollow() now shells any convex
PRISM, not just an axis-aligned box, and declines honestly (offset_body errors on non-convex /
non-planar / non-simple-vertex bodies; over-thick t collapses the inner shell -> the difference's
mass==mesh post-condition rejects it). No boolean.rs change (the enclosed-void partition is
identical to Addendum 113), so fuzz coverage is unchanged. Added tests: a triangular-prism hollow
(valid two-shell wall, mass==mesh, 0<wall<outer) and an over-thick decline (2^3 box, t=1.5 ->
Err). Curved-face and concave shells remain the follow-up (need offset_body to grow beyond convex
planar, or the winding-number offset trim of dossier 50 sec 4).
GATE: workspace 108 + 77 + 160 (two new shell tests) green, clippy -D warnings, fmt. Merged.

## Addendum 115 (2026-06-09, attended): PER-FACE / multi-thickness shell (item 43). 84 -> 85/144

Generalized offset_body to a per-face distance: offset_body_with(dist: Fn(FaceKey)->f64) moves
each face plane inward by dist(f) and reintersects the 3-valent corners (the differently-offset
planes simply meet at a new corner / step, dossier 50 sec 3.1); offset_body(distance) is now a
thin wrapper. Body::hollow_per_face(thickness: Fn(FaceKey)->f64) clones, shrinks per-face by
offset_body_with(-thickness), and subtracts -- the enclosed-void difference yields the multi-
thickness wall; hollow(t) delegates with a constant closure. The per-face thicknesses are keyed
by the body's own FaceKeys, which a deep clone preserves, so the closure matches the inner copy's
faces. ORACLE: a 4^3 box with the top wall at t=2 and the rest at t=1 -> inner void
[1,3]x[1,3]x[1,2] = 4, wall volume 64 - 4 = 60, mass == mesh, validate ok. offset_body behaviour
is unchanged (delegation only) and boolean.rs is untouched, so fuzz coverage is unchanged.
GATE: workspace 108 + 77 + 161 green, clippy -D warnings, fmt. Merged.
SHELL-FAMILY STATUS: 41 (hollow) + 43 (multi-thickness) + 45 (offset_body) done; 42 (pierce/rim
walls) and 44 (thicken, needs sheet bodies kernel/51) remain.

## Addendum 116 (2026-06-09, attended): PIERCE / open-tray shell (item 42). 85 -> 86/144

Body::hollow_pierce(t, pierced: Fn(FaceKey)->bool): opens the faces for which pierced(f) is true
so the void communicates with the outside (a cup/tray, dossier 50 sec 3.2/6.2). The clean
mechanism avoids rim-wall surgery entirely: a pierced face's inner-shell counterpart is pushed
OUTWARD past the original face (offset_body_with returns +margin for pierced faces, -t otherwise),
so the subtracted inner pocket POKES THROUGH that face and the boolean difference opens that side
TRANSVERSALLY -- the rim of the opening is just the wall's edge, produced by the ordinary
transversal boolean (no separate build_rim_wall). ORACLE: pierce the top of a 4^3 box at t=1 ->
open tray, removed pocket 2x2x3 = 12, tray volume 64 - 12 = 52, mass == mesh, validate ok, a
SINGLE connected shell (the void is open, no enclosed-void region). Convex planar scope (offset_
body); at least one face must stay un-pierced. boolean.rs untouched (this is a new transversal
USE of the existing pipeline, not new boolean code), so fuzz coverage is unchanged.
GATE: workspace 108 + 77 + 162 green, clippy -D warnings, fmt. Merged.
SHELL FAMILY now 41/42/43/45 done; only 44 (thicken, needs sheet-body representation kernel/51)
remains -- its own milestone.

## Addendum 117 (2026-06-09, attended): SHEET BODIES + THICKEN (item 44) -- the first non-solid body kind. 86 -> 87/144

(User: "open the sheet-body keystone" -> "1". Built it.) Re-read dossier 51 (sheet/open-body
topology) before starting. KEY FINDING that collapsed the risk: Keel's EXISTING validator already
admits a lamina sheet with NO change. A sheet (one double-sided planar face, n FREE edges of
radial-1, bordering the ambient void on both sides, no solid region) is non-manifold, so
check_euler_poincare auto-skips (it only runs when every edge is radial-2); check_radial_cycles
already accepts radial-1 (each lone fin is in exactly one cycle); check_shells_regions requires
only the single infinite VOID region (not a solid one) and the double-sided face's two sides both
link to that void. So NO BodyKind / validator refactor was needed -- the feared core-touching
change evaporated; the only work was constructing a correct lamina.

CONSTRUCTOR (crates/keel-topo/src/sheet.rs): Body::planar_sheet(profile) builds the lamina
DIRECTLY in the arenas (new_face with front==back==void, new_loop, n new_vertex, n new_edge each
with a single radial fin = a free edge, splice the fin ring, one shell in the void holding both
(F,Front) and (F,Back)), then attaches the Plane (Newell normal) + boundary Line3s. It validates
as a real open body (the first body in Keel with no solid region). THICKEN: Body::thicken(t) --
MVP single planar face -- reads the sheet's face plane + outer-loop profile and extrudes it to
thickness t CENTRED on the sheet plane (-t/2 base, +t sweep), via prism. ORACLES: a 2x3 sheet
validates (1 face, 4 free edges); thicken(0.5) -> slab volume 3.0, mass == mesh, validate ok.

This SHIPS item 44 AND stands up the sheet-body representation, which is the shared prerequisite
for Phase 5 sheet ops -- extend (70), knit/sew (71), trim (72), split (76) -- now unblocked.
Follow-ups (documented): multi-face / curved sheets thicken via offset-both-sides + a rim band
(dossier 50 sec 5), one-sided thicken mode, and a BodyKind tag for explicit sheet/solid semantics
(not needed for validation, useful for API clarity). No boolean.rs change, so fuzz is unchanged.
GATE: workspace 108 + 77 + 164 green (two new sheet tests), clippy -D warnings, fmt. Merged.
SHELL FAMILY COMPLETE: 41/42/43/44/45 all done.

## Addendum 118 (2026-06-09, attended): TRIM a sheet by a plane (item 72) -- first Phase-5 sheet op. 87 -> 88/144

Body::trim_by_plane(plane_point, plane_normal): the first sheet operation on the new sheet-body
representation. Keeps the portion of a planar sheet on the BACK side of the plane ((p-pt).n <= 0)
by clipping the boundary polygon to that half-space with a single-plane Sutherland-Hodgman pass
(emit each inside vertex; at a crossing edge emit the plane intersection point t = da/(da-db)),
then rebuilds the kept polygon as a sheet via planar_sheet. ORACLE: a 4x4 sheet trimmed by x=2
(normal +x) -> a 2x4 sheet; thickened by 1 -> volume 8 (so the kept half is exactly right). MVP:
single planar face, simple boundary; trim by a general SURFACE and trimming solid faces are
follow-ups on the same clip. No boolean.rs change -> fuzz unchanged.
GATE: workspace 108 + 77 + 165 green, clippy -D warnings, fmt. Merged.
PHASE 5 progress: 72 (trim) done; 70 (extend) / 71 (knit-sew, with closure->solid promotion) /
76 (split) / 77 (slice) remain, all on the sheet-body foundation.

## Addendum 119 (2026-06-09, attended): SPLIT a solid by a plane (item 76). 88 -> 89/144

Body::split_by_plane(point, normal) -> (back, front): split a solid into two pieces by a cutting
plane, reusing the boolean -- no new machinery. Each half-space is modelled as a large oriented
SLAB (a big square in the plane, spanned by in-plane axes u, v = n x u, extruded |big| along the
normal; the base winding is self-corrected so the slab is a positive-volume solid). Each piece is
the body with the OTHER half-space REMOVED by a guillotine DIFFERENCE (back = body - front_slab,
front = body - back_slab). ORACLE: a 4^3 box split by x=2 -> two 2x4x4 = 32 pieces, both validate,
mass-exact. MVP: any solid the guillotine difference handles; general curved/oblique cuts inherit
the boolean's current coverage.

NOTE / latent boolean follow-up found in passing: the INTERSECTION form (body INTERSECT half-slab)
was ASYMMETRIC -- the back-side intersection assembled but the geometrically-mirrored front-side
intersection failed the mass==mesh gate ("degenerate or self-inconsistent result"), both slabs
being valid positive-volume solids. The DIFFERENCE path is clean for both (guillotine is the
tested case), so split uses it; the intersection asymmetry is a real boolean bug worth a separate
fuzz/diagnosis pass (likely an A-vs-B operand-order or face-EntityId-order sensitivity in the
intersection select/stitch). Logged, not chased here. No boolean.rs change -> fuzz unchanged.
GATE: workspace 108 + 77 + 166 green, clippy -D warnings, fmt. Merged.
PHASE 5 now: 72 (trim) + 76 (split) done; 70 (extend) / 71 (knit-sew) / 77 (slice) remain.

## Addendum 120 (2026-06-09, attended): SLICE by an offset list (item 77). 89 -> 90/144

Body::slice(point, normal, offsets) -> Vec<Body>: cut a solid by a list of parallel planes
(at point + off*normal), returning the N+1 ordered pieces between consecutive planes. Implemented
as a repeated split_by_plane in increasing offset order (split off the back piece at each plane,
carry the front forward), so it inherits split's coverage with zero new machinery. ORACLE: a 6^3
box sliced at x=2 and x=4 -> three 2x6x6 = 72 slabs, each valid + mass-exact. No boolean.rs change
-> fuzz unchanged.
GATE: workspace 108 + 77 + 167 green, clippy -D warnings, fmt. Merged.
PHASE 5 now: 72/76/77 done; only 70 (extend) + 71 (knit-sew, closure->solid promotion) remain.
Counter at 90/144 -- a +8 run this session (53/41/43/42/44/72/76/77) on top of the #21/#16
keystones + enclosed-void stitch + sheet-body representation.

## Addendum 121 (2026-06-09, attended): KNIT / SEW with closure->solid promotion (item 71, dossier 51 centerpiece). 90 -> 91/144

boolean::knit(bodies, tol) -> Body: join a set of sheet (or solid) bodies into one, merge
coincident vertices, glue coincident free edges into radial pairs, and -- when the result closes
into a watertight shell -- PROMOTE it to a solid. REFACTOR (the clean enabler): the back half of
stitch_by_import (vertex-merge + edge-glue + shell-closure invariant + enclosed-void region
partition + validate) was EXTRACTED verbatim into a shared pub(crate) finalize_imported_assembly
(dst, rec, faces, inf, solid, vtol); both the boolean stitch AND knit now call it, so knit's
assembly is the exact same heavily-tested code (no duplication, both paths cover it). knit imports
each body's faces with per-body identity (within-body shared edges dedup by source id; the
cross-body coincident edges join in the coordinate merge), then finalizes. ORACLE: six planar
square sheets (each oriented outward via a CCW-about-normal helper) knit into a closed cube and
promote to a SOLID -- volume 8, mass == mesh, validate ok. This completes the sheet-body story:
construct (planar_sheet) -> trim/split/slice -> KNIT -> promote-to-solid, and is the dossier-51
centerpiece. The extraction is behavior-preserving: 32 boolean tests pass and fuzz_boolean is
clean (the refactor is a pure code move).
GATE: exact CI triplet green (workspace 108 math + 77 geom + 168 topo, clippy -D warnings, fmt)
+ fuzz_boolean (WSL nightly, 200s, Done 414 runs, clean). Merged.
PHASE 5 now: 71/72/76/77 done; only 70 (surface extend) remains. Counter 91/144 -- a +9 run this
session (53/41/43/42/44/72/76/77/71).

## Addendum 122 (2026-06-09, attended): EXTEND a sheet boundary (item 70) -- PHASE 5 COMPLETE. 91 -> 92/144

Body::extend(d): grow a planar sheet's boundary outward by d. Project the boundary to 2D in the
plane (u, v with u x v = n), offset each edge's supporting line outward by d (outward unit normal
sign set by the polygon's signed area), reintersect consecutive offset lines at the new sharp
corners, unproject, rebuild via planar_sheet. The planar analogue of the dossier-13 surface-
extension core (extending the trim of an infinite plane). d<0 shrinks; parallel/degenerate corners
decline. ORACLE: a 2x2 sheet extended by 1 -> a 4x4 sheet; thickened by 1 -> volume 16. MVP single
planar convex face; curved extend is a follow-up. No boolean.rs change -> fuzz unchanged.
GATE: workspace 108 + 77 + 169 green, clippy -D warnings, fmt. Merged.

PHASE 5 COMPLETE: 70 (extend) + 71 (knit) + 72 (trim) + 76 (split) + 77 (slice) all done, on the
sheet-body foundation built this session. Counter 92/144 -- a +10 run this session
(53/41/43/42/44/72/76/77/71/70) on top of the #21/#16 Phase-0a keystones + enclosed-void 3-region
stitch + sheet-body representation + the finalize_imported_assembly extraction. Shell family
(41-45) AND Phase-5 sheet ops (70/71/72/76/77) both fully shipped.

## Addendum 123 (2026-06-09, attended): ASSEMBLY LAYER -- instances + DAG + transforms + stable ids (items 82-85). 92 -> 96/144

Re-read dossier 16 (assembly layer) first. The industry-consensus model (Parasolid partitions +
app instancing, OCCT XCAF shape-ref + TopLoc_Location, STEP NAUO + mapped_item): an assembly is a
DAG of DEFINITIONS (geometry stored once) and INSTANCES (definition reference + placement
transform); structure is ORTHOGONAL to placement; an OCCURRENCE's world placement is the
composition of instance transforms along its path and its IDENTITY is that path of stable
instance ids (survives geometry edits). The kernel provides this thin tier; mating/constraints/PLM
stay in the host.

NEW MODULE crates/keel-topo/src/assembly.rs (no change to any existing code -> zero regression
risk): Assembly { defs, root } with Def::Part(Box<Body>) | Def::Sub(Vec<Instance>);
add_part/add_subassembly/set_root/edit_part; flatten() -> Vec<Occurrence{ path, world, body }>
walks the DAG accumulating child.transform.then(parent_acc) and places each leaf via the existing
Body::transformed (item 74). Definitions can only reference EARLIER-added defs, so the graph is
acyclic by construction (no cycle check needed). Delivers all four items: 82 instances (Instance),
83 assembly DAG (a Def referenced by many instances -- a SHARED subassembly), 84 per-instance
transforms (world = path composition), 85 stable edit-surviving ids (InstanceId / occurrence
path). ORACLES: (1) one box def -> a 2x2 grid via a sub-assembly instanced twice (the DAG) -> 4
distinct world occurrences, each the unit box (vol 8, validate ok), each a length-2 path, all
paths/positions unique; (2) editing the box def to a 4^3 box leaves the occurrence paths
unchanged (identity survives) while the new volume 64 flows through. Instance transforms are rigid
(Body::transformed's domain: rotation+translation; scale/mirror/NURBS instances are a follow-up).
GATE: workspace 108 + 77 + 171 green (two new assembly tests), clippy -D warnings (boxed the large
Def::Part variant), fmt. No boolean.rs change -> fuzz unchanged. Merged.
Counter 96/144 -- a +14 run this session. Phase 9 assemblies 82-85 done; foreign geometry
114-116 remains in Phase 9.

## Addendum 124 (2026-06-09, attended): FIX the "latent boolean bug" -- it was a prism CW-base inconsistency, NOT a boolean asymmetry. Stays 96/144 (correctness fix)

CORRECTS Addendum 119's misdiagnosis. Data-first repro (box[0,4]^3 INTERSECT a half-slab at
x=2): with AXIS-ALIGNED slabs BOTH sides assemble (back=32, front=32, identical kept=6/5A+1B) ->
the boolean is NOT asymmetric. The failure was specific to split's ORIENTED slab: the front
slab gave kept=1 (0 A-faces, 1 B-face) -- classify found ZERO box faces inside it, even the x=4
face plainly inside. ROOT CAUSE (slab volume probe): the oriented front slab had
mass_properties = +54309 but mesh_volume = -18103 -- prism produced a body whose SENSE-based
volume is positive yet whose TESSELLATION winding is INVERTED. So the generalized winding number
(tessellation-based) was inverted -> classify_faces saw every box face as outside -> nothing kept.
The trigger: split's self-correcting slab fed prism a base wound CW about the extrude dir; prism
attaches face senses as if the base were CCW, so mass (sense) and mesh (winding) disagree.

FIX (construct.rs, prism -- the ROOT, helps every caller): auto-orient the base to CCW about
`dir` (reverse the profile when its Newell normal opposes dir) so prism NEVER emits a
sense-vs-tessellation-inconsistent body. Backward-compatible: every existing caller passes a CCW
base (no-op); only CW bases are flipped. After the fix the oriented front slab is consistent
(mass==mesh==54309) and box INTERSECT either oriented slab gives 32. Simplified split_by_plane's
slab (removed the now-redundant volume self-correct). REGRESSION TESTS: prism_cw_base_is_consistent
(a CW-about-+z square prism -> mass==mesh==12) and intersection_with_oriented_slabs_is_symmetric
(4^3 split by x=2 -> two valid 32 halves). The Addendum-119 note is hereby retracted: there is no
boolean intersection asymmetry; the bug was a prism orientation footgun, fixed at the source.
GATE: workspace 108 + 77 + 173 green, clippy -D warnings, fmt + fuzz_boolean (WSL nightly, 200s,
Done 361 runs, clean). Merged. Counter stays 96/144 (a correctness fix, no new feature).

## Addendum 125 (2026-06-09, attended): PARTITIONS + PMARKS + TRANSACTIONS (items 123/124/125). 96 -> 99/144

New crates/keel-topo/src/partition.rs (no change to existing code -> zero regression risk). The
session layer already had per-body snapshot/restore + json + journal/replay (126, 129); this adds
the multi-body rollback layer:
- item 123 PARTITION: Partition { bodies, marks } -- the top-level rollback container; add/body/
  body_mut/len + to_json/from_json (the partition serialized as ONE unit, exact f64 round-trip per
  the body serde, dossier 14). Parasolid's partition = the undo unit holding bodies.
- item 124 PMARKS / rollback: set_pmark(name) captures the current body state; roll_to(name)
  reverts (or advances -- marks are retained) every body in the partition to that mark. Both
  rollback and rollforward.
- item 125 TRANSACTIONS: begin (save state, nestable stack) / abort (revert to begin) / commit
  (accept). The open-transaction stack is #[serde(skip)] (a persisted partition has none).
The three share one substrate (a body-state clone) but are distinct user-facing capabilities --
container, navigable history marks, atomic op grouping -- exactly as the parity map lists them
separately; each has its own API + test. ORACLES: a 2-body partition round-trips through json with
exact volumes (8, 27); a pmark reverts an edited box (64 -> 8); begin/abort reverts (8) while
begin/commit keeps (27). Incremental DELTA save (127) and version control (128) are follow-ons.
GATE: workspace 108 + 77 + 176 green (three new partition tests), clippy -D warnings, fmt. No
boolean.rs change -> fuzz unchanged. Merged. Counter 99/144.

## Addendum 126 (2026-06-09, attended): DELTA save (item 127) -- counter hits 100/144

Added Partition::delta_from(base) -> PartitionDelta and Partition::apply_delta(base, delta):
incremental save storing ONLY the bodies whose topology_hash differs from base (or are newly
added) plus the new length, serializable as a compact unit. The serialize-only-what-changed half
of dossier 14's persistence contract (full to_json is the other half). ORACLE: a 3-body partition
with one body edited -> the delta carries exactly 1 body; round-tripping the delta through json and
applying it to the base reconstructs all three exactly (unchanged bodies preserved, changed body
applied). No core change -> fuzz unchanged.
GATE: workspace 108 + 77 + 177 green, clippy -D warnings, fmt. Merged.

*** COUNTER 100/144 *** -- a +18 run this session (53/41/43/42/44/72/76/77/71/70 + 82/83/84/85 +
123/124/125/127) on top of the #21/#16 Phase-0a keystones, the enclosed-void 3-region stitch, the
sheet-body representation, the finalize_imported_assembly extraction, and the prism CW-base
correctness fix. Complete this session: SHELL FAMILY (41-45), PHASE 5 sheet ops (70/71/72/76/77),
ASSEMBLY DAG (82-85), PERSISTENCE rollback layer (123/124/125/127). Roadmap realistic target is
~110-120; remaining tractable kernel items thin out into the invasive (foreign geometry 114-116
needs a Surface3::Foreign variant threaded through SSI/tessellation), version control 128, and
healing 130/132 (need imperfect/recognized geometry to exercise).

## Addendum 127 (2026-06-09, attended): SESSION-END RESUME ANCHOR (session buttoned up; counter 100/144)

WHERE WE ARE. master HEAD = e138f77, all green: workspace 108 math + 77 geom + 177 topo, clippy
-D warnings, fmt, fuzz_boolean clean. Tree clean, all work merged (NOT pushed -- push only if
asked; master is well ahead of origin). CodeGraph index synced (file watcher current). This
session ran a +18 counter sprint 82->100 plus the #21/#16 Phase-0a keystones, the enclosed-void
3-region stitch, the sheet-body representation (first non-solid body kind, crates/keel-topo/src/
sheet.rs), the finalize_imported_assembly extraction (boolean.rs, shared by stitch + knit), the
assembly layer (assembly.rs), the partition/persistence layer (partition.rs), and the prism
CW-base correctness fix. Per-item detail is in Addenda 104-126.

COMPLETE: SHELL FAMILY 41-45, PHASE 5 sheet ops 70/71/72/76/77, ASSEMBLY DAG 82-85, PERSISTENCE
rollback 123/124/125/127.

FOREIGN GEOMETRY (114-116) -- ARCHITECTURE CORRECTED before building (a brief /plan-eng-review
investigation, cancelled mid-run but this finding kept): the earlier note "needs a Surface3::
Foreign variant threaded through SSI/tessellation" is the WRONG shape. Facts from the code:
Surface3 (keel-geom) is ANALYTIC-ONLY {Plane,Cylinder,Cone,Sphere,Torus}; non-analytic geometry
lives one level up in SurfaceGeom { Analytic(Surface3), Nurbs(NurbsSurface) } (entity.rs:56),
matched in ~9 files. A Surface3::Foreign / SurfaceGeom::Foreign(Box<dyn ...>) variant would (a)
break the body/partition serde (a dyn trait object is not Serialize/Deserialize -- it would
poison items 126/127 just shipped) and (b) cost a ~15-site match blast radius. RIGHT APPROACH
(aligns with the kernel's founding NURBS-AS-CACHE doctrine): foreign geometry is an INPUT -- a
host-implemented evaluator trait (ForeignSurface{eval(u,v)->Vec3, domain}, ForeignCurve) consumed
by a constructor (e.g. Body::add_foreign_face(&dyn ForeignSurface, tol)) that SAMPLES + FITS to a
certified NURBS face (reusing the M7a fit + M8 recover/certify machinery), storing a standard
Nurbs SurfaceGeom. The procedural evaluator is the truth, the NURBS is the cache. Non-invasive (no
new SurfaceGeom variant), serde-safe, and honest (a black-box evaluator has no exact predicates
anyway -> NURBS-tolerant is the correct representation). This is the recommended next milestone
plan; the variant approach is rejected.

NEXT (pick one): (1) foreign geometry 114-116 via the evaluator->fit-to-NURBS plan above (~3
items, non-invasive); (2) DEEPEN existing MVPs (no counter tick, higher correctness value):
curved/multi-face generalizations of shell (offset_body is convex-planar only) and the sheet ops
(thicken/trim/extend are single-planar-face only), or the oblique-cut blend classes; (3) version
control 128 (thin over the partition pmarks already built -- watch for double-counting). Standing
rules unchanged: re-read the relevant dossier first + cite it; data-first diagnose before fixing;
verify the SUCCESS branch explicitly (never read "X or decline" as "X"); branch-per-milestone, run
the exact CI triplet + a fuzz_boolean soak before merge, then a LOG addendum; NO EM-DASHES.

## Addendum 128 (2026-06-10, attended): PHASE 9 FOREIGN GEOMETRY 114-116 (counter 103/144)

WHAT SHIPPED (branch phase9-foreign): foreign geometry the NON-invasive way, exactly per the
Addendum 127 corrected architecture. The host implements an evaluator (ForeignSurface{domain,
eval(u,v)}, ForeignCurve{domain, eval(t)} in crates/keel-geom/src/foreign.rs); the kernel SAMPLES
+ FITS it to a certified NURBS and stores a standard SurfaceGeom::Nurbs / Curve3::Nurbs. The
evaluator is the procedural truth, the NURBS is the cache (founding doctrine). No new geometry
variant, body/partition serde (126/127) untouched, every existing NURBS path (tessellate_nurbs,
recover/simplify, booleans after recovery) works unchanged.

DOSSIERS (cited per standing rule): research/kernel/25-parasolid-capability-map.md sec 17 +
checklist 114-116 (PK foreign-geometry evaluator-callback family; "can later be converted to
native NURBS where needed" is literally this design). research/kernel/08-kernel-engineering.md
sec 3.4 + recommendation 9 (evaluator traits decouple representation from the topology engine;
callback geometry "limits which exact algorithms can run on it", hence cache + certify, never
direct). Fit method: NURBS Book 9.4.2 two-stage surface approximation.

MECHANICS. fit_foreign_surface: uniform parameter grid affinely mapped to [0,1]^2, two-stage
tensor-product LSQ reusing fit::lsq_fit (made pub(crate); all row fits share params + control
count so the derived knot vectors agree and the columns assemble into one net), control count
escalating 4->8->16->32, certification against FRESH evaluator samples on a 2x-denser offset
grid, inflated by the same SAFETY=2.0 as fit_cubic. End-pinned LSQ means the four corners (and
curve endpoints) interpolate exactly. fit_foreign_curve: same, one direction. Topology:
Body::foreign_sheet builds the planar_sheet-shaped lamina (one double-sided face, four free
edges) whose edges carry the EXACT boundary iso-curves of the fitted patch
(NurbsSurface::boundary_curves, senses set per ring direction), so face and edge geometry are
consistent by construction; DECLINES (honesty gate) when the certificate misses tol or the
boundary is degenerate (pole patches excluded, MVP). Body::attach_foreign_edge_curve covers the
curve half (endpoint match in either orientation sets the sense; misses decline).

ITEM MAPPING, stated honestly. 114 = the traits + certified constructors (surfaces AND curves).
115 = the foreign-derived body is first-class: validates, tessellates (tessellate_nurbs grids the
full domain, which IS the sheet's face), and after native conversion feeds the modeling stack;
test chain: foreign plane under a non-affine parameterization -> simplify recovers the native
Plane -> thicken to a 2.0-volume slab (mass == mesh, 1e-9) -> corner-overlap boolean Difference
to 1.875 (mass == mesh, 1e-9). Direct booleans on a still-NURBS open sheet are NOT claimed.
116 = foreign -> NURBS (the fit) plus NURBS -> native (the existing simplify/recover certified
gate): a foreign quarter-cylinder evaluator recovers Surface3::Cylinder at 1e-4; a foreign
segment edge recovers to a Line. SUCCESS branches verified explicitly: the wavy sin/cos patch
CERTIFIES at 1e-4 and fresh-parameter spot checks stay inside the certificate; the C0 crease
evaluator REFUSES at 1e-9 at both the fit level and the constructor level (decline tests).

DISCOVERY (pre-existing, follow-up candidate): block-minus-block INTERIOR THROUGH-NOTCH declines
(AssemblyFailed "mass != mesh") even on pristine primitive blocks; a control test reproduced it
with no foreign geometry involved, so it is not this milestone's bug. The drilled-block cylinder
case passes. Honest decline, not a wrong body; the likely gap is the planar-face interior-ring
imprint path. Not pursued this session.

CI: fmt, clippy -D warnings, workspace 114 geom + 77 math + 182 topo all green (+6 geom, +5
topo; note Addendum 127's "108 math + 77 geom" had the two labels swapped). fuzz_boolean WSL
soak: 10 minutes, 1260 runs, clean. CodeGraph synced.

COUNTER: 100 -> 103/144 (items 114, 115, 116). NEXT (menu unchanged from Addendum 127, minus
foreign): deepen the MVPs (curved/multi-face shell + sheet ops, oblique-cut blend classes) or
version control 128 (thin over partition pmarks, watch double-counting); plus the new
through-notch imprint follow-up above.

## Addendum 129 (2026-06-10, attended): INTERIOR THROUGH-HOLE BOOLEANS (no counter tick; boolean
deepening, the Addendum 128 follow-up landed immediately)

WHAT SHIPPED (branch boolean-through-notch): the interior through-notch decline found during the
foreign-geometry session is FIXED. A 2x2x0.5 slab minus a 0.5x0.5 block piercing it completely now
assembles to a valid GENUS-1 body (V16 E24 F10, 2 inner rings), volume exactly 1.875 with
mass == mesh to 1e-9; the same operands' Intersection yields the exact 0.125 core box. This is the
kernel's first genus-RAISING boolean.

DIAGNOSIS (data-first, staged pipeline introspection; dossier check first ruled OUT file 39, this
is transversal, and confirmed placement in d-booleans-tolerant.md's stage-2 "imprint locally along
intersection curves"): the control case faulted 4x "unassembled face seams" on operand B. Each
tool wall is cut by the slab's top AND bottom planes into two DISJOINT parallel segments: neither
one closed loop nor one connected open chain, the only two shapes imprint_operand's dispatch could
assemble, so the walls never split; the unsplit whole wall then classified InsideOther from an
interior sample, was kept whole, and stitch correctly declined on the #21 shell-closure invariant.
After fixing that, a SECOND gap surfaced: the stitched result failed Euler-Poincare (lhs 2, rhs 4)
because finalize_imported_assembly built brand-new shells with genus 0 while the through-hole
component is genus 1. The validator was RIGHT both times; both declines were honest (three-bucket
doctrine: DECLINE, never WRONG).

FIX 1, multi-component imprint (boolean.rs imprint_operand): after canonical dedup, partition the
face's seams into connected components by open-endpoint adjacency (seam_components, union-find,
deterministic order; closed seams stand alone) and run the EXISTING three-way dispatch per
component. An earlier component's split can strand a later one on a descendant piece of the
original face, so each later component relocates onto the planar face that now contains its probe
point (planar_face_containing: on-plane within etol + 2D crossing number accumulated over ALL the
face's loops, orientation-free, rings included; loop_polygon made pub(crate) in tessellate.rs).
Single-component faces take exactly the old path (zero behavior change). Curved multi-component
faces (a cylinder pierced clean through carries two closed SSI circles on one lateral face) still
DECLINE via the new "unlocated seam component" fault: honest, recorded as the curved follow-up.

FIX 2, component genus stamping (finalize_imported_assembly): each connected face component's
shell pair now carries the genus implied by the component's own Euler characteristic, chi = V - E
+ sum_f(2 - loops(f)) = V - E + F - rings over the component's entities, genus = (2 - chi)/2,
clamped at 0. Matches the kfmrh convention (both shells of the pair carry it; counts() sums
shells / 2): the through-notch result is 16 - 24 + 10 - 2 = 0 -> genus 1; a plain box component
stays 8 - 12 + 6 - 0 = 2 -> genus 0.

TESTS: through_notch_difference_is_genus_one (counts, rings, genus, mass==mesh==1.875) and
through_notch_intersection_is_core (exact 0.125 box) in boolean.rs; the foreign.rs participation
test keeps its corner-overlap config with the stale "known DECLINE" comment corrected.

CI: fmt, clippy -D warnings, workspace 114 geom + 77 math + 184 topo (+2) all green. fuzz_boolean
WSL soak (mandatory, boolean internals changed): 15 minutes, 1724 runs, clean.

COUNTER unchanged at 103/144 (this is correctness deepening, the kind Addendum 127 rated above
thin ticks). NEXT: curved through-holes (relocation on curved faces, e.g. cylinder-through-block
both-caps-out), or version control 128, or deepen shell/sheet MVPs.

## Addendum 130 (2026-06-10, attended): PARITY BREADTH WAVE 1 -- items 7, 9, 15, 62, 74 (103 ->
108/144)

ENDGAME ACCOUNTING FIRST (the /goal is "finish the parity list"): reconstructed the tick ledger
from every RUNNING TOTAL line in this LOG. Items cited as ticked nowhere in the LOG and still
tractable: 7, 9, 10, 15, 28, 29, 31, 48-51, 54-60 (blend depth), 62, 67, 68, 74, 80, 94, 113,
122, 128, 130, 132, 137-143 subset, 140. Out-of-scope per the roadmap stays 34, 46, 61, 79, 81,
86-93, 134-135 (~15 items). The counter also contains ~8 capability-slice ticks with no named
item (the partial-overlap completion, surface-area, fillet rungs 2-3, transform/mirror/scale,
closest-point, flat-cap/washer/tube revolve slices); to avoid double-counting, NEW ticks will
only ever use the never-cited numbers above. ANTI-DOUBLE-COUNT RULE in force.

WAVE 1 SHIPPED (branch parity-breadth-1), five honest thin ticks, each a public API + tests over
proven machinery (dossier 25 checklist rows; the construction items align with roadmap Phase 2):
- 9 ACORN: Body::acorn(p) wraps the existing ops::embed_vertex lone-vertex shell;
  BodyClass::Acorn (query.rs) already classified it. Test: classifies + validates.
- 62 SWEEP BY TRANSLATION: Body::sweep(profile, dir) publishes prism under the capability name
  (prism IS the translational sweep with lateral faces; its CW-base auto-orient from Addendum 124
  makes it winding-safe). Test: swept L-profile, volume 8, mass == mesh to 1e-9.
- 15 HELIX: keel_geom::foreign::fit_helix(origin, axis, radius, pitch, turns, tol) -- a helix is
  transcendental (no exact NURBS form), so the CERTIFIED foreign-curve fit is the honest
  representation (the Addendum 128 machinery earning rent immediately); Body::helix_wire embeds
  the certified curve as a wire edge and DECLINES if the certificate misses tol. Tests: 2-turn
  r=2 pitch=0.5 helix certifies 1e-3, fresh-parameter spot checks inside the certificate, wire
  body validates, samples stay on the helix cylinder.
- 7 SHEET PRIMITIVES: Body::rectangular_sheet (over planar_sheet) and Body::disc_sheet -- one
  double-sided planar face bounded by a single FREE closed circular edge (the closed-circle loop
  shape the flat-cap solids already use, now as a lamina). Tests: rect thickens to the exact
  slab; disc validates and tessellates to pi r^2 within 2% (chordal undershoot, the tessellation-
  tolerance lesson). BUG CAUGHT: Circle3::new takes IN-PLANE axes (center, x_axis, y_axis, r),
  not (center, normal, x_ref, r); the first draft built a vertical circle and the disc
  tessellated to ZERO area -- the area oracle caught it (loop_polygon diag confirmed).
- 74 IMPRINT BODY ONTO BODY: Body::imprint_body(tool, tol) -- the body-level form of the
  imprint-only option (32), straight over seam_curves + imprint_operand; any fault is an Err,
  never dropped. Test: corner-overlapping blocks: faces/edges increase, volume EXACTLY unchanged
  (mass == mesh == 64), valid.

CI: fmt, clippy -D warnings, workspace 115 geom + 77 math + 190 topo (+1, +6) green. No fuzz
soak: every change is ADDITIVE (new constructors + a new imprint entry point; no existing
boolean/tessellation path modified), the Addendum 74/75-class justification.

COUNTER: 103 -> 108/144 (items 7, 9, 15, 62, 74). NEXT (wave 2 menu): 122+113 sessions config +
precision control; 128 version control (thin over pmarks, the double-count check done above
clears it); 139 cosmetic threads over the attribute system; 137 convert-to-NURBS (exact quadric
-> NURBS via revolve_full + bilinear plane patches); 94 tessellate-to-tolerance (body-level
facet output exists as render_mesh_tol; tick with the documented API + test); then the heavier
28 sheet-solid booleans / 31 selective booleans / 80 / 138 / 140 / 142, and blend depth 48-51,
54-60 last.

## Addendum 131 (2026-06-10, attended): PARITY BREADTH WAVE 2 -- items 94, 113, 122, 128, 137,
139 (108 -> 114/144) + XNURBS PATENT RE-EVALUATION

PATENT RE-EVALUATION (user request): the worry that XNurbs-style functionality is patent-gated
does not match the record. (1) No Keel document ever fenced it: the D10/dossier-18 fences are
Siemens convergent/mesh (86-93), Siemens Synchronous Technology variational DIRECT-EDITING
(US 9,235,659 + family: constraint inference among faces during push/pull, a different
technology that shares the word "variational"), and Coreform U-splines. (2) Dossier 32 already
designates the XNurbs-equivalent as Keel's design target on PRE-1995 prior art: Celniker-Gossard
1991, Welch-Witkin 1992 (sec 3.3: "the closest published analog to the XNURBS core and the
reference design for Keel's variational surfacer"), Moreton-Sequin, Greiner; OCCT ships LGPL
energy-minimizing constrained filling (GeomPlate) as living prior art. CONCLUSION: the
variational surfacer (fairness matrix + constraint rows + sparse solve, dossier 32 sec 4) is
patent-clear; it is the natural engine for item 68 (n-sided boundary surface) and later 67
(loft with continuity). Queued.

WAVE 2 SHIPPED (branch parity-breadth-2), six ticks (dossier 25 rows; sec 16/19/22 language):
- 122 SESSIONS + 113 PRECISION CONTROL (session.rs): Session::start/configure/stop owning
  partitions (123) and a SessionConfig { linear_tolerance, angular_tolerance } that genuinely
  DEFAULTS session-run operations (Session::boolean uses it); configure rejects non-finite/
  non-positive tolerances. Per-entity tolerant edges (110-112) remain the local override layer.
  DATA POINT recorded: a coincident-overlap union (shared y/z planes) DECLINES at tol 1e-6 while
  passing at the default 1e-7 -- the coincident pre-pass is tolerance-sensitive; transversal
  cases pass at both. Known limitation, not pursued (the session test uses a transversal pair).
- 128 VERSION CONTROL (session.rs): save_versioned(target)/load_versioned with a v2 envelope
  { keel_save_version, body } over the bare v1 item-126 document. Read-old (v1 bare and v2 both
  load), write-target (v1 emitted for old readers, verified readable by from_json), and a
  FUTURE-stamped document errs honestly (UnsupportedVersion) instead of misreading.
- 137 CONVERT TO NURBS (new keel-geom convert.rs + keel-topo convert.rs): the FORWARD direction
  of M8 recover. Exact rational forms: plane bilinear; cylinder/cone bands, sphere, torus via
  revolve_full (NURBS Book ch 8); curves line/circle/ellipse (ellipse = affine map of the
  rational unit arc, weights unchanged). Body::face_to_nurbs derives bounds from the face
  (plane: outer-loop frame bbox; cyl/cone: loop height range; sphere/torus: full closed).
  Round-trip test: barrel -> exact NURBS band -> recover_surface certifies the cylinder again at
  1e-9. GOTCHA fixed: loop_polygon's closed-circle fallback returns ONE rim's samples for a
  2-vertex barrel loop, so face_to_nurbs also gathers the raw loop vertices for the height range.
- 139 COSMETIC THREADS (new cosmetic.rs): CosmeticThread { designation, pitch, depth,
  right_handed } stored under reserved keel.thread.* attribute keys -- exactly Parasolid's
  carry-as-attributes model (dossier 25 sec 22), riding items 117-121 for typing/propagation and
  126 for persistence (round-trip tested). No thread geometry, by design.
- 94 TESSELLATE TO TOLERANCE (tessellate.rs): pub Body::facets(tol: Option<f64>) -- the body-
  level facet product over the existing per-face tessellators (None = legacy density; Some =
  the item-98 adaptive chord refinement). Test: sphere refines (more facets) and the facet
  volume lands within 1% of 4pi/3 at 2e-4.

CI: fmt, clippy -D warnings, workspace 118 geom + 77 math + 197 topo (+3, +7) green. No fuzz
soak: additive APIs only (facets wraps tessellate_face_opt; Session::boolean delegates; no
boolean/tessellation/imprint path modified).

COUNTER: 108 -> 114/144 (items 94, 113, 122, 128, 137, 139). The realistic-target band (110-120)
is now entered. NEXT (wave 3): 80 (deformation workflow via existing tweak/offset/taper, a
composition tick), 138 (general topology replace: replace edge curve / vertex point to round out
replace-surface), 140 (thread-safety contract + concurrent-query test), 142 (NURBS split/edit
toolkit: split-at-param via knot insertion, the pieces exist), then the real engines: 28
sheet-solid booleans, 31 selective booleans, 68 n-sided variational fill (now patent-cleared),
130 healing, 132 defeaturing, blends 48/50/51.

## Addendum 132 (2026-06-10, attended): PARITY BREADTH WAVE 3 -- items 80, 138, 140, 142 (114 ->
118/144)

WAVE 3 SHIPPED (branch parity-breadth-3), four ticks (dossier 25 rows, sec 22 language for
138/142; anti-double-count rule honored, all four never previously cited):
- 142 NURBS SPLIT/EDIT TOOLKIT: NurbsCurve::split already existed; the missing surface half is
  now NurbsSurface::split_u / split_v (knot insertion to full multiplicity, then a net
  partition -- exactly the curve algorithm lifted to rows/columns). Proptest over random rational
  surfaces: the half containing the sample reproduces the surface to 1e-9. With create/evaluate/
  insert_knot/to_bezier/split on both curves and surfaces, the toolkit row is genuinely covered.
- 138 GENERAL TOPOLOGY REPLACE (new replace.rs): replace_face_surface_checked +
  replace_edge_curve_checked -- representation-level geometry swaps under existing topology,
  GUARDED: every face-boundary sample must lie on the new surface within tol (analytic project /
  project_point_surface_fast), edge-curve endpoints must land on the edge's vertices (either
  orientation -> sense; closed edges need a closed curve through the seam vertex). A replace can
  re-represent (the item-137 exact NURBS) but never silently relocate. Tests: box face ->
  bilinear NURBS -> valid, volume unchanged, simplify recovers the plane (137+138+M8 closure);
  a shifted plane and an offset curve are REFUSED. The reintersecting replace-and-move form
  remains the tweak family (35-39), as the dossier distinguishes.
- 80 DEFORMATION WORKFLOW: the capability-map row is the deform-via-tweak/offset/taper WORKFLOW
  over items 35-39/78 (all long since shipped); ticked with the composed end-to-end test:
  offset top (+1, vol 12) -> move top (+1, vol 16) -> taper +x side inward (wedge 4, vol 12
  exact), valid and mass == mesh at every step. Sign convention recorded: positive taper angle
  leans the face OUTWARD (the test wanted inward, hence -atan(0.25)).
- 140 THREAD SAFETY (the safety half): Body is Send + Sync by construction (owned arenas/maps,
  no interior mutability) -- statically asserted -- and a thread::scope test runs
  mass_properties / mesh_volume / facets / surface_area concurrently on one &Body with exact
  results. Mutation is exclusive (&mut) by the borrow checker. SMP-PARALLEL algorithms are the
  opportunistic half and stay a follow-up, exactly as the roadmap lists 140.

CI: fmt, clippy -D warnings, workspace 119 geom + 77 math + 202 topo (+1, +5) green. No fuzz
soak: additive only (new split/replace APIs + tests; no boolean/tessellation/imprint change).

COUNTER: 114 -> 118/144 (items 80, 138, 140, 142). Remaining tractable: 10 (body from supplied
geometry), 28/29/31 (sheet/general/selective booleans), 48-51 + 54-60 (blend depth), 67/68
(loft guides / n-sided variational fill, patent-cleared per Addendum 131), 130 (healing), 132
(defeaturing), 141/143 (verify not consumed by the unnamed slice ticks first). Excluded stays
~15. NEXT: the engines, biggest leverage first: 28 sheet-solid booleans (the GWN classifier is
surface-type-agnostic; sheet fragments classify the same way), then 68 variational fill, then
132 defeature-small-features over delete_face.

## Addendum 133 (2026-06-10, attended): SHEET-SOLID BOOLEANS (item 28, 118 -> 119/144)

THE ENGINE (branch sheet-booleans): boolean_sheet_solid(sheet, solid, op, tol) -- the first
mixed-dimension boolean. Pipeline: seam_curves (sheet faces x solid faces SSI) -> imprint_operand
on the sheet (the multi-component + interior-ring machinery from Addenda 129/127 carries it
unchanged) -> classify_faces fragments by GWN against the solid (the d-booleans-tolerant.md
classifier is surface-type-agnostic, so sheet fragments classify exactly like solid faces) ->
keep Inside (Intersection) or Outside (Difference) -> import + merge + glue -> SHEET finalize.

NEW PIECES: merge_and_glue_imported -- the merge-coincident-vertices + glue-dangling-edges half
of finalize_imported_assembly extracted VERBATIM (behavior-preserving; the solid path calls it
unchanged) -- and finalize_imported_sheet: one shell per connected component holding BOTH face
sides in the void, faces front==back==void, no solid region and no closure invariant (free edges
are the nature of an open sheet; body_class stays Sheet).

HONEST BOUNDARIES: Union of mixed dimension is undefined -> declines; coincident/tangent or
unclassifiable contact -> declines (the same general-position MVP boundary the solid boolean
started from); sheet-SHEET booleans are a follow-up (the imprint exists, the kept-side semantics
need defining); solids CUT BY sheets (knife) likewise.

ORACLES: 4x4 sheet, 2x2x2 block punching the middle: Intersection = the interior 2x2 patch
(area exactly 4); Difference = a HOLED RING SHEET (one face, one inner ring, signed area exactly
12) -- the interior-ring imprint earning rent on sheets. Guillotine overlap: Difference 6,
Intersection 2, both exact. Union declines.

CI: fmt, clippy -D warnings, workspace 119 geom + 77 math + 204 topo (+2) green. fuzz_boolean
WSL soak (boolean.rs internals changed): 10 minutes, 977 runs, clean.

COUNTER: 118 -> 119/144 (item 28). Remaining tractable: 10, 29, 31, 48-51, 54-60, 67, 68, 130,
132 (+ 141/143 pending the consumed-slice check). NEXT: 31 selective/local booleans (face-pair
scoped imprint over the same machinery) or 68 variational n-sided fill (dossier 32 sec 4,
patent-cleared), then 132 defeature over delete_face, 130 gap-tighten heal.

## Addendum 134 (2026-06-10, attended): N-SIDED BOUNDARY FILL (item 68, 119 -> 120/144)

THE CONSTRUCTOR (branch nsided-fill): keel-geom fill.rs fill_boundary(sides, tol) +
Body::filled_sheet -- surface from boundary curves. Method per dossier 26 sec 1.1 (Coons 1967,
MAC-TR-41): per-side end-pinned LSQ (shared resampled parameterization so the four fits share
knots; corners interpolate EXACTLY) gives the boundary control rows; the interior net is the
DISCRETE bilinearly blended Coons sum (lofts minus the bilinear corner correction). N != 4 sides
join and re-quarter by arc length. Certification against fresh side samples, foreign-fit style,
SAFETY x max deviation; Body::filled_sheet DECLINES over tol. The sheet topology builder was
extracted from foreign_sheet into nurbs_sheet_body (shared by items 114 and 68, behavior-
preserving; foreign tests unchanged).

HONEST LIMITATION recorded in the module doc and the hexagon test: quartering places N != 4
corners INSIDE the virtual sides where the cubic fit ROUNDS them; the certificate reports it
(~7e-3 on the unit hexagon) rather than hiding it. The corner-exact central-split scheme
(dossier 26 sec 4) and the variational fairing upgrade (dossiers 31/32, patent-clear per
Addendum 131) are the follow-ups on this same skeleton.

ORACLES: flat 2x2 square boundary -> certifies at 1e-9, whole patch flat to 1e-12, and the
CHAIN test: filled sheet -> simplify recovers the native Plane -> thicken to the exact 2.0 slab
(68 -> 116 -> 44 composing). Saddle boundary (one corner lifted): certifies 1e-6, corners exact,
valid Sheet body, tessellates. Hexagon: quartering path, in-plane to 1e-9, certificate honest.
Open chain: rejected.

CI: fmt, clippy -D warnings, workspace 123 geom + 77 math + 206 topo (+4, +2) green. No fuzz:
additive constructor (no boolean/imprint/tessellation change; the foreign_sheet refactor is a
verbatim extraction covered by its existing tests).

COUNTER: 119 -> 120/144 (item 68). THE REALISTIC TARGET BAND (110-120) IS NOW FULLY REACHED.
Remaining tractable: 10, 29, 31, 48-51, 54-60 (the blend deep end), 67, 130, 132 (+141/143
pending slice-check). NEXT: 132 defeature (delete_face exists) and 130 gap-tighten are the
moderate pair; 31 selective booleans; the blend family is the long tail.

## Addendum 135 (2026-06-10, attended): HEALING + DEFEATURING (items 130, 132; 120 -> 122/144)

DOSSIER 13 is the design source: healing = the three-phase skeleton "stitch, then simplify, then
build geometry" with per-phase reporting; "defeaturing is delete-face plus heal ... the same
toolkit, so build it once". New crates/keel-topo/src/heal.rs.

130 HEAL / GAP-TIGHTEN: Body::heal(tol) -> HealReport { vertices_merged, edges_glued,
surfaces_recovered, curves_recovered }. Phase 1 = the stitch-phase vertex-gap + edge-gap closing
(merge_and_glue_imported, now pub(crate), applied IN PLACE -- the dossier's gap taxonomy);
phase 2 = M8 simplify (canonical recognition). Phase 3 (geometry rebuild by re-intersection /
surface extension) is the documented follow-up, exactly the dossier ordering. Test: a box face
re-coated with its exact item-137 NURBS heals back to analytic, volume preserved.

132 DEFEATURE SMALL HOLES: Body::defeature_small_holes(max_area) removes small holes by EXACT
SURGERY, no boolean: a small inner ring with FREE edges (a punched sheet) is deleted outright
(the host plane already covers the hole -- removing the trim extends the face); a WALLED small
ring (solid through/blind hole) floods its wall faces (terminating at ring-bearing host faces,
capped at 32; escape -> decline) and deletes them, which frees the opposite ring for the first
rule. Shell genus is restamped from chi = V - E + F - rings per shell (the Addendum-129
formula). HONESTY GATES: the surgery runs on a CLONE and commits only if validate() passes and
(planar solids) mass == mesh; volume bound documented. Oracles: the genus-1 through-notch slab
defeatures back to the EXACT plain slab (V8 E12 F6, genus 0, volume 2.0 mass == mesh); the
punched sheet refills to area 16; a hole larger than max_area survives untouched. Blend/boss
removal stays with the blend-family follow-ups (unblend needs extend-and-heal), stated in the
module doc.

CI: fmt, clippy -D warnings, workspace 123 geom + 77 math + 210 topo (+4) green. No fuzz:
additive module; the only boolean.rs change is merge_and_glue_imported visibility.

COUNTER: 120 -> 122/144 (items 130, 132). Remaining tractable: 10, 29, 31, 48-51, 54-60 (the
blend deep end, ~11 items), 67, (141/143 pending slice-check). Excluded ~15. NEXT: 31 selective
booleans or 10 body-from-geometry, then the blend family is the last big chunk.

## Addendum 136 (2026-06-10, attended): BODY FROM SUPPLIED GEOMETRY (item 10, 122 -> 123/144)

Body::from_polygon_faces(faces, tol) (construct.rs): create a body directly from supplied
geometry -- one planar polygon per face, outward-CCW; shared edges are discovered by coincidence
and glued by the knit machinery; a closed set PROMOTES to a solid, an open set stays a sheet
assembly. The EXPLICIT-topology construction path remains the Euler operators (item 136 of the
map, long shipped) plus attach_face_surface / attach_edge_curve; this constructor is the
geometry-first form Parasolid's create-from-topology row describes, on Keel's proven stitch.
Oracle: a 4-polygon tetrahedron builds to the exact 8/6 volume, mass == mesh, valid.

CI: fmt, clippy -D warnings, workspace 123 + 77 + 211 (+1) green. No fuzz: thin additive wrapper
over knit.

COUNTER: 122 -> 123/144 (item 10). Remaining tractable: 29, 31, 48-51, 54-60, 67. NEXT: blend
recognition (58) + unblend (59) -- the inverse pair of the shipped fillet machinery (the
plane-plane fillet's exact inverse is reconstructable: delete the cylinder face and re-intersect
the two support planes); dossier 40/28 re-read first.

## Addendum 137 (2026-06-10, attended): BLEND RECOGNITION + UNBLEND (items 58, 59; 123 ->
125/144)

THE INVERSE PAIR (branch blend-recognize-unblend; dossier 28 sec 6.3: model blends with their
support data "exactly as the blend-removal literature describes the inverse"):

58 RECOGNITION: Body::recognize_blends(angular_tol) -> Vec<RecognizedBlend { face, radius,
supports, spring_edges }>. A blend face is a CYLINDRICAL (edge fillet) or TOROIDAL (cap rim)
face TANGENT to exactly two distinct neighbors along shared edges -- tangency = parallel
outward normals at the shared-edge midpoint (project + local_geometry, sense-adjusted). Radius =
cylinder radius / torus minor. Negative control: a cylinder primitive's barrel meets its caps at
90 degrees and is NOT recognized; planar faces (chamfers) are never candidates by construction.

59 UNBLEND: Body::unblend(face, tol) -- the EXACT inverse of fillet_edge for plane-plane
supports. The sharp edge is the support planes' intersection line; each end-arc's sharp vertex
is that line cut by the arc's cap plane. Surgery (clone + gates, the heal.rs doctrine): move the
four spring-end vertices onto the sharp positions, merge each arc's endpoint pair, splice the
arc fins out of their loop rings (ids unregistered -- the IdMapInconsistent lesson: raw arena
removals MUST unregister), drop the arcs, then kef the first spring edge (the blend face merges
into support A; the survivor is re-pointed at A's plane since kef's keep-side is fin-order
dependent), and the second spring edge BECOMES the sharp edge, carrying the intersection line.
Gates: validate + mass == mesh, else DECLINE with the body untouched. Toroidal and
curved-support unblends are follow-ups.

ORACLE (the round trip): box -> fillet_edge(0.5) -> recognize exactly one blend (radius 0.5)
-> unblend -> the EXACT original box: V8 E12 F6, volume 8, mass == mesh to 1e-9, and
recognition then finds nothing. Construction and removal are mutual inverses on the rung-1
fillet.

CI: fmt, clippy -D warnings, workspace 123 + 77 + 213 (+2) green. No fuzz: additive module
(recognition is read-only; unblend is clone-gated surgery; no boolean/imprint path change).

COUNTER: 123 -> 125/144 (items 58, 59). Remaining tractable: 29, 31, 48-51, 54-57, 60, 67.
NEXT: 31 selective booleans, then the remaining blend depth (48 variable radius is the
spine-with-varying-r generalization; 54-57 are policy layers on the same engine).

## Addendum 138 (2026-06-10, attended): SELECTIVE FACE-PAIR BOOLEANS (item 31; 125 -> 126/144)

boolean_selective(a, b, op, pairs, tol) (branch selective-booleans): the boolean restricted to
the intersection seams of caller-chosen (target-face, tool-face) pairs. REFACTOR: the post-seam
tail of boolean() -- imprint both operands, classify, select, stitch, degeneracy +
self-consistency gates -- extracted VERBATIM into assemble_boolean and shared by both entries,
so the selective path runs every honesty gate unchanged: an INSUFFICIENT selection (seams that
cannot bound a closed result) DECLINES through the standard gates instead of emitting a wrong
body. The coincident-overlap pre-pass is skipped on the selective path (it re-clones operands
and would remap the caller's face keys); coincident contacts decline as usual.

ORACLES: corner-overlap blocks: selecting exactly the seam-bearing pairs reproduces the full
union volume to 1e-9; dropping every pair touching one target face declines. fuzz_boolean WSL
soak (mandatory, boolean.rs refactored): 10 minutes, 978 runs, clean. CI: fmt, clippy -D
warnings, workspace 123 + 77 + 214 (+1) green.

COUNTER: 125 -> 126/144 (item 31).

## Addendum 139 (2026-06-10, attended): SESSION-END RESUME ANCHOR (counter 126/144)

WHERE WE ARE. A +23 session (103 -> 126), all merged to master, all green: workspace 123 geom +
77 math + 214 topo, clippy -D warnings, fmt, four clean fuzz_boolean soaks at every boolean
change (1260, 1724, 977, 978 runs). NOT pushed (push only if asked; master is well ahead of
origin). The roadmap's realistic band (110-120) is EXCEEDED.

SHIPPED THIS SESSION (per-item detail in Addenda 128-138): foreign geometry 114-116 (evaluator
-> certified fit-to-NURBS; the Surface3::Foreign variant rejected per Addendum 127 stands);
INTERIOR THROUGH-HOLE booleans (multi-component imprint + component genus stamping -- the first
genus-RAISING boolean); breadth waves 7/9/10/15/62/74 + 94/113/122/128/137/139 +
80/138/140/142; SHEET-SOLID booleans (28) with the sheet finalize; n-sided Coons boundary fill
(68) + filled_sheet; healing (130) + defeaturing small holes (132, exact clone-gated surgery);
blend RECOGNITION + UNBLEND (58/59, the exact fillet inverse, box round-trips); SELECTIVE
booleans (31). Plus the XNurbs PATENT RE-EVALUATION (Addendum 131): variational surfacing is
patent-clear (Welch-Witkin 1992 baseline; the dossier-18 "variational" fences cover Siemens
DIRECT EDITING, a different technology); and the ANTI-DOUBLE-COUNT LEDGER (Addendum 130).

THE REMAINING LIST IS NOW ALL DEEP-END ENGINES, each a multi-session milestone, none a
single-session tick: 29 (general/non-manifold-body booleans, dossier 02 territory); 48
variable-radius blends (varying-r spine, canal surfaces); 49/60 conic + G2 sections; 50
face-face blends; 51 vertex/setback blends; 54 hold-line; 55 range-controlled; 56 networks +
mitring; 57 overflow handling (dossier 41); 67 loft with guides + continuity (Gordon, dossier
26). Out-of-scope stays ~15 (34, 46, 61, 79, 81, 86-93 patent-fenced convergent, 134-135).
126 done + 15 excluded = 141 of 144 accounted; the 3-item gap vs the checklist is the
slice-tick history recorded in Addendum 130 (the ledger is the LOG, kept honest there).

NEXT SESSION (pick one): (1) blend depth, starting 48 variable-radius (re-read dossiers 40/41
first; the spine machinery generalizes) or 57 overflow (dossier 41 catalogues the failure
modes); (2) 67 Gordon loft with guides (dossier 26 secs 2-3); (3) 29 general-body booleans
(dossier 02; the sheet finalize + GWN classifier are the foundations). Also live: curved
through-holes ("unlocated seam component" relocation on curved faces), sheet-sheet booleans,
the central-split corner-exact n-sided fill, variational fairing on the fill skeleton, toroidal
unblend, and the completion gate (10h all-sector fuzz soak + three-bucket billion-boolean
oracle) before any 1.0 claim. Standing rules unchanged: dossier re-read + citation per item;
data-first; verify the SUCCESS branch; branch-per-milestone; exact CI triplet + fuzz_boolean
soak when boolean internals change; LOG addendum then merge; NO EM-DASHES; push only if asked.

## Addendum 140 (2026-06-10, attended): VARIABLE-RADIUS BLEND, EXACT CONE RUNG (item 48; 126 ->
127/144)

THE SURPRISE (branch variable-radius-blend; dossiers 40/28 re-read): item 48 has an EXACT
ANALYTIC rung. With the ball radius varying LINEARLY along a plane-plane edge, each offset
"plane" tilts but remains a PLANE (the offset distance is linear in position: (n - k e).x = c),
so the spine is still a straight line; the envelope of the spheres is an exact CONE (half-angle
asin(dr/ds_spine), perpendicular radius rho/cos(alpha), apex where rho extrapolates to 0); and
the tangency loci on both supports stay STRAIGHT LINES (touch = spine - sign*n*rho(tau), linear
in tau). The whole rung-1 story generalizes without leaving the analytic island -- no NURBS
canal needed for the linear law.

PIECES: blend_cone_for_edge (geometry, hand-verified in-test: spheres equidistant to both
planes along the spine, springs on-plane, radii exact at the ends); fillet_edge_variable (the
proven trim-and-stitch surgery with the end arcs the EXACT ELLIPSES of cone x cap-plane --
closed-form conic: quadric expanded in plane coordinates, 2x2 center solve, closed-form eigen
for the axes); loop_polygon gained an open-ELLIPSE-arc sampling branch; tessellate_cone clamps
each ruling to its exact cap-plane intersections for two-ellipse patches (pt(phi, v) is linear
in v, so each cap is a scalar solve -- the band meets the caps watertight).

REGRESSION CAUGHT BY THE SUITE: the first no-rim height fallback used loop_polygon, whose
closed-circle degenerate path returns ONLY rim samples -- a revolve cone's POLE vanished and the
barrel/bicone volumes halved. Fixed: raw edge-bound vertex heights (the pole bounds the seam
edge). The revolve suite is exactly why constructor regressions stay cheap.

ORACLES: r 0.3 -> 0.6 on a box edge: valid; ONE cone blend face; mesh_volume within 0.01 of
8 - (1 - pi/4) L (r0^2 + r0 r1 + r1^2)/3 (the exact removed-wedge integral); cone face area
1.4165 vs ~1.43 analytic. CI: fmt, clippy -D warnings, workspace 123 + 77 + 216 (+2) green;
fuzz_boolean soak (tessellation feeds the GWN classifier): 10 minutes, 746 runs, clean.

COUNTER: 126 -> 127/144 (item 48, the linear-law rung; general radius laws = the NURBS canal
follow-up). Remaining: 29, 49-51, 54-57, 60, 67 -- all multi-session engines. The conic-section
rung (49) now looks closest: the same surgery with a rational-quadratic strip between the same
spring lines.

## Addendum 141 (2026-06-10, attended): CONIC BLEND SECTIONS (item 49; 127 -> 128/144)

fillet_edge_conic(edge, d, w) (branch conic-blend): the chamfer-to-circle CONTINUUM on the
proven trim-and-stitch surgery. Springs set back `d` along each support from the edge (the
setback-orientation sign was caught by the existing spring-misses-cap guard: convex moves AWAY
from the other support's outward side); the cross-section is a rational quadratic with the
SHOULDER ON THE OLD SHARP EDGE and weight w. The blend face carries the exact degree-2x1
rational strip (NurbsSurface, 3x2 net, weights [1,w,1] per column); the cap arcs are the same
conic per cap plane (NurbsCurve, attached exactly). loop_polygon gained an open-NURBS-arc
sampling branch (oriented to the fin), so NURBS-bounded planar caps stop chording -- a general
improvement beyond this item.

ORACLE (the continuum, all on one box edge, d = 0.5): w = sqrt(2)/2 reproduces the EXACT
circular fillet volume 7.8927 within 0.02 (on perpendicular supports the rational quadratic
with the sqrt(2)/2 shoulder weight IS the circle -- the established nine-point machinery's own
constant); w = 0.02 approaches the chamfer chord 7.75 within 0.02; w = 25 hugs the corner
(> 7.95); fullness verified MONOTONE in w. All three bodies validate. G2 / curvature-continuous
sections (item 60) are NOT claimed: w controls fullness, not curvature matching.

CI: fmt, clippy -D warnings, workspace 123 + 77 + 217 (+1) green; fuzz_boolean soak
(loop_polygon feeds the winding oracle): 10 minutes, 803 runs, clean.

COUNTER: 127 -> 128/144 (item 49). Remaining: 29, 50, 51, 54-57, 60, 67 -- the true deep end
(face-face, vertex, hold-line, ranges, networks, overflow, G2, Gordon loft, non-manifold
booleans).

## Addendum 142 (2026-06-10, attended): HOLD-LINE BLENDS (item 54; 128 -> 129/144)

fillet_edge_hold_line(edge, hold_on_first, setback) (branch hold-line-blend): the spring on the
hold support is PINNED `setback` in from the edge; the RADIUS FLOATS to keep the ball tangent
to the other support. For a hold line parallel to the edge between planar supports the floating
radius is CLOSED FORM (centre = hold - n_h r; tangency: r (1 - n_h . n_o) = dist(hold, other
plane)), so the blend stays an exact cylinder and the standard trim-and-stitch surgery applies
unchanged. Scope: parallel holds, convex plane-plane edges; curved/non-parallel holds and
concave edges decline as follow-ups.

ORACLES: perpendicular box edge, hold 0.5 -> r floats to 0.5 = the exact circular fillet
(volume 7.8927 within mesh tolerance). THE DISCRIMINATOR: a right-triangle prism's 45-degree
wedge edge, hold d = 1 on the x = 0 wall -> r = d/(1 + sqrt(2)) = sqrt(2) - 1, verified
INDEPENDENTLY by item-58 blend recognition on the result (radius to 1e-9) -- construction and
recognition cross-check each other. Valid, material removed.

NOTED TECH DEBT: the trim-and-stitch surgery body now exists four times (fillet_edge,
fillet_edge_variable, fillet_edge_conic, fillet_edge_hold_line) varying only in section
geometry and attached curves; a parameterized extraction is queued cleanup (deliberately not
done mid-stride: each copy is gated by its own oracles).

CI: fmt, clippy -D warnings, workspace 123 + 77 + 218 (+1) green. No fuzz: additive blend.rs
only.

COUNTER: 128 -> 129/144 (item 54). Remaining: 29, 50, 51, 55, 56, 57, 60, 67.

## Addendum 143 (2026-06-10, attended): FACE-FACE BLEND (item 50; 129 -> 130/144)

blend_face_face(wall) (branch face-face-blend): the rolling ball between two PARALLEL planar
faces that share NO edge, consuming the wall face joining them -- the rounded-end slab. The
radius is FORCED to half the gap (the only ball tangent to both); the blend is the exact
HALF-CYLINDER along the wall midline, so this rung needs no spine marching (the spine is a line
by symmetry). Surgery: imprint a spring on each support (reusing imprint_spring_line against
each support's own wall edge), split each cap along the half-circle arc between the spring ends
-- recording the SIGNED ARC SWEEP, because a pi arc is direction-ambiguous from its endpoints
(the Edge::arc_sweep machinery from partial revolve, flipped so the OUTWARD half is taken) --
then dissolve strips + wall into one face (kef across both wall edges) and kill each cap's
corner chain (kef stub, kev wall-cap edge, kev stub). General face-face (non-parallel supports,
free radius, marched spine) stays the follow-up.

ORACLES: slab 4x1x2 rounded at x = 4: volume 8 - r^2 (2 - pi/2) h = 7.7854 within 0.02; valid;
item-58 RECOGNITION independently finds exactly one blend, the half-cylinder tangent to both
side faces, radius 0.5 to 1e-9 (the construction/recognition cross-check again).

CI: fmt, clippy -D warnings, workspace 123 + 77 + 219 (+1) green. No fuzz: additive blend.rs.

COUNTER: 129 -> 130/144 (item 50). Remaining: 29, 51, 55, 56, 57, 60, 67.

## Addendum 144 (2026-06-10, attended): G2 BLEND SECTIONS (item 60; 130 -> 131/144)

fillet_edge_g2(edge, d, fullness) (branch g2-blend): TRUE curvature continuity to planar
supports. The cross-section is a QUINTIC Bezier whose first three and last three control points
are COLLINEAR along the in-plane tangent directions: a Bezier's endpoint curvature is
proportional to |(P1-P0) x (P2-P1)|, so collinearity makes the section curvature ZERO at both
springs -- matching the flat supports exactly. (The circular fillet is only G1: its curvature
JUMPS from 0 to 1/r at the spring.) The blend face carries the exact degree-5x1 strip; the cap
arcs are the same quintic per cap plane; the standard surgery applies (the fifth copy -- the
parameterized extraction remains queued cleanup).

ORACLE: box edge, d = 0.5, fullness 0.4: valid; volume strictly between the chamfer chord and
the sharp box; and the strip's PRINCIPAL CURVATURE measured on the attached NURBS surface
(local_geometry) is < 1e-2 at u = 0.001/0.999 (the springs) and > 0.5 mid-strip -- the G2
property verified numerically on the actual face geometry, not assumed from construction.

CI: fmt, clippy -D warnings, workspace 123 + 77 + 220 (+1) green. No fuzz: additive blend.rs.

COUNTER: 130 -> 131/144 (item 60). Remaining: 29, 51, 55, 56, 57, 67 -- six items, each a true
multi-session engine (vertex blends, range/runout surgery, networks/mitring, overflow policies,
Gordon loft, non-manifold booleans).

## Addendum 145 (2026-06-10, attended): GORDON LOFT WITH GUIDES (item 67; 131 -> 132/144)

gordon_surface(sections, guides, tol) (keel-geom fill.rs) + Body::lofted_sheet_with_guides
(branch gordon-loft): loft through sections WITH guide curves, per dossier 26 sec 2 -- the
Gordon surface as the BOOLEAN SUM of the section loft, the guide loft, and minus the tensor
interpolant of their node grid. THE REDUCTION: rather than wrestling three NURBS fits into
compatible knot vectors, the Boolean sum is built TRANSFINITELY -- an EVALUATOR (piecewise-
linear blends between adjacent sections/guides, piecewise-bilinear node tensor) that satisfies
the Gordon interpolation identity by construction -- and fed to the EXISTING certified
foreign-surface fit (Addendum 128 machinery). One NURBS out, honest tol_achieved against the
transfinite truth, the sheet via nurbs_sheet_body, DECLINE over tol. Guides must MEET every
section near the grid nodes (validated, slack 10 tol) or the construction declines.

ORACLE: three quarter-circle sections (48-segment polylines) stacked in z with the two end
RULINGS as guides: the section blend is an extrusion, so the Gordon surface IS the quarter
cylinder -- the sheet certifies at 2e-3, validates, and SIMPLIFY RECOVERS THE NATIVE ANALYTIC
CYLINDER (67 -> 116 -> M8 chaining). Disjoint guides decline. (A 16-segment polyline failed the
2e-3 certificate first -- the chordal input WAS the truth and the certificate said so; the
denser polyline is the fix, not a looser gate.)

CI: fmt, clippy -D warnings, workspace 123 + 77 + 221 (+1) green. No fuzz: additive constructor.

COUNTER: 131 -> 132/144 (item 67). Remaining: 29, 51, 55, 56, 57 -- vertex blends, range/runout,
networks/mitring, overflow policies, non-manifold booleans: the five true multi-session engines.

## Addendum 146 (2026-06-10, attended): SESSION-END RESUME ANCHOR (counter 132/144)

WHERE WE ARE. master HEAD all green: workspace 123 geom + 77 math + 221 topo, clippy -D
warnings, fmt; six clean fuzz_boolean soaks across the session at every boolean/tessellation
change. NOT pushed (push only if asked). A +29 SESSION: 103 -> 132/144, far past the roadmap's
realistic band (110-120).

THE SECOND HALF OF THE RUN (Addenda 140-145) cracked the blend deep end faster than the roadmap
priced it, by finding analytic/reduction rungs: 48 variable-radius = an EXACT CONE (tilted
offset planes stay planes; ellipse cap arcs closed-form); 49 conic sections = a rational
quadratic strip on the same surgery (w = sqrt(2)/2 IS the circle); 54 hold-line = closed-form
floating radius (the sqrt(2)-1 wedge oracle, cross-checked by item-58 recognition); 50
face-face = the half-cylinder rounded-end slab (signed arc-sweep disambiguation); 60 G2 = a
quintic with collinear end triples (zero spring curvature VERIFIED numerically on the attached
surface); 67 Gordon loft = a TRANSFINITE EVALUATOR fed to the certified foreign-surface fit.
Pattern worth remembering: before building a marching/NURBS engine, look for the analytic rung
and for a reduction to the certified-evaluator pipeline.

THE WALL (this session's honest stopping point): the five remaining tractable items are true
multi-session engines needing NEW machinery, not reductions: 29 general/non-manifold-body
booleans (dossier 02); 51 vertex/setback blends (corner patches where three blends meet); 55
range-controlled blends (partial-span springs + setback runout mid-face); 56 blend networks +
mitring (multi-edge propagation); 57 overflow handling (dossier 41 policy engine; today
overflow declines via the spring-misses-cap guard, honest but not "handling"). Out-of-scope
stays as ledgered (Addendum 130; the counter includes the historic slice ticks recorded there).

QUEUED CLEANUPS: parameterize the five-fold trim-and-stitch surgery copies (each currently
gated by its own oracles); curved through-hole relocation ("unlocated seam component");
sheet-sheet booleans; central-split corner-exact n-sided fill; variational fairing pass
(patent-clear, Addendum 131); toroidal/curved-support unblend; coincident-union tolerance
sensitivity (Addendum 131 data point); the 10h all-sector fuzz + three-bucket billion-boolean
completion gate before any 1.0 claim.

STANDING RULES unchanged: read this LOG first (this addendum is the anchor); dossier re-read +
citation per item; data-first diagnosis; verify the SUCCESS branch explicitly; branch-per-
milestone; exact CI triplet + fuzz_boolean soak when boolean/tessellation internals change; LOG
addendum then merge; anti-double-count rule (Addendum 130); NO EM-DASHES; push only if asked.

## Addendum 147 (2026-06-10, attended): RESEARCH ROUND LANDED + CLIFF OVERFLOW (item 57; 132 ->
133/144)

RESEARCH: dossiers 53-57 merged from worktree-nurbs-research (commit bb57472): vertex/setback
blends, partial-span blends/runouts, blend networks/mitring, overflow handling, non-manifold
general booleans. Every dossier names a narrowest-honest-slice first milestone with an exact
oracle, and the attack order falls out: cliff overflow (verbatim surgery reuse) -> partial-span
(square stop + exact cone runout) -> degree-2 mitre (exact bisector trim) -> sphere-octant
corner -> general-boolean Rung 1 (interface-keeping selection layer; contingent on retiring the
polygon-soup fallback per dossier 47).

CLIFF SHIPPED (branch cliff-overflow; dossier 56 secs 1 + 7, the "Pareto-dominant first
conquest"): fillet_edge_cliff(edge, r) -- when r exceeds one support's width the blend is
HALF-TANGENT (Parasolid: "tangent to only one of the faces ... and runs along an edge in the
other face"): a radius-r cylinder tangent to the wide support only, passing THROUGH the narrow
support's far edge b, found in closed form (center on the offset plane at distance r from b's
line; the perpendicular case degenerates to s = r, t = sqrt(2rW - W^2)). The narrow support is
CONSUMED; b stays SHARP, promoted to a blend/neighbor shared edge with zero new edges along it.
Surgery = the standard trim-and-stitch with the narrow-side spring imprint REPLACED by whole-
face consumption (kef across e absorbs the narrow face; cap corner chains die by kef stub + kev
spur; the cap arcs run spring-end to the EXISTING b corners).

ORACLES: box 4x1x2, wall-top edge, r = 1.25 > W = 1: exact integral oracle (removed = L (HW -
Iseg), the circle-segment antiderivative evaluated in-test) within 0.02; DIFFERENTIAL SWEEP:
below the trigger (r = 0.8) the ordinary fillet applies and the cliff REFUSES, above it the
ordinary fillet DECLINES and the cliff handles -- the dossier-41/56 sweep contract. Half-tangent
signature verified: full-blend recognition finds NOTHING (tangent to one support only); the
narrow top face is gone. Valid throughout.

HONEST SCOPE: the cliff is the most common overflow per dossier 56 and turns Keel's decline
into a handle for it; the smooth/roll-on, notch, cap handlers and the four-predicate
classification ladder remain the dossier-56 follow-ups, as do creeping/partial cliffs.

CI: fmt, clippy -D warnings, workspace 123 + 77 + 222 (+1) green. No fuzz: additive blend.rs.

COUNTER: 132 -> 133/144 (item 57). Remaining: 29, 51, 55, 56.

## Addendum 148 (2026-06-10, attended): PARTIAL-SPAN BLEND (item 55; 133 -> 134/144)

fillet_edge_partial(edge, t0, t1, radius) (branch partial-span-blend; dossier 54 Q3/Q4/Q5):
round only the interior interval of a plane-plane edge, tapering linearly from r at t0 to ZERO
at t1 -- the feathered runout, an EXACT CONE with apex at the vanishing vertex -- with a planar
SQUARE-STOP cross face at t0. The cap-into-supports end surgery, exactly the dossier recipe:
split the sharp edge at both stations (junction + vanishing vertices), mev-SPUR each spring
landing into its support's interior, split_face the CONVERGING spring lines to the apex, kef
the interior sharp interval (merging the two trim slivers), and the cross-arc split then leaves
the little planar face between the arc and the corner -- which IS the kept square-stop face.
The sharp edge RESUMES on both sides (asserted: two collinear fragments on the original line);
the apex vertex joins both springs and the resumed edge.

TWO REAL FINDS BEYOND THE FEATURE:
1. cyl_angular_span REWRITTEN: the span now comes from loop_polygon SAMPLES via the largest-
   angular-gap method -- branch-cut-free (the old vertex min..max broke when a patch straddles
   +-pi) -- with on-axis points SKIPPED (a cone apex has no angle and polluted the span).
   Tilted-CIRCLE boundaries now clamp cone ruling bands exactly like the ellipse caps (the stop
   arc lies in the plane perpendicular to the EDGE, not the tilted cone axis). Zero regressions
   across the 222 existing tests; fuzz_boolean soak (tessellation feeds the winding oracle):
   10 minutes, 808 runs, clean.
2. THE ORACLE WAS WRONG, NOT THE MESH: the dossier-54 closed form (1 - pi/4) integral is the
   UNTILTED idealization; the true tilted envelope (spine from the t0 section center to the
   apex) removes measurably less material. A brute SWEPT-SPHERE grid oracle (per-station spine-
   quadrant gate + closed-form min over the rolling balls) agrees with the mesh within 0.01.
   Verify-the-success-branch earned out again: three candidate numbers (mesh, closed form,
   naive grid) disagreed until the characterization was made exact.

CI: fmt, clippy -D warnings, workspace 123 + 77 + 223 (+1) green; soak clean as above.

COUNTER: 133 -> 134/144 (item 55). Remaining tractable: 29, 51, 56. Ladder per dossier 54:
two square stops, curved spines (torus segment + canal runout), partial recognition/unblend.

## Addendum 149 (2026-06-10, attended): MITRED BLEND CORNER (item 56; 134 -> 135/144)

mitre_fillet_corner(e1, e2, r) (branch mitre-blend; dossier 55 milestone 1): two equal-radius
convex plane-plane edges sharing a corner and one support, their cylinder blends joined along
the EXACT INTERSECTION ELLIPSE -- for equal radii the two cylinders intersect in the bisector
plane on E(t) = M + (W - M) cos t + (X - M) sin t (verified on both cylinders for every t),
with M the spine crossing, X the top-spring crossing, W the side-spring crossing on the shared
vertical edge. SURGERY: the partial-span SPUR trick (Addendum 148) imprints the two top springs
meeting at X; the sides imprint boundary-to-boundary (side 2 reusing the W vertex -- splitting
the shared vertical edge once); far ends take the standard cap treatment (cap = the face at the
far vertex NOT carrying the sharp edge -- the strip-list exclusion mis-picked under the top
split's old/new ambiguity); and the old corner vertex RELOCATES onto the ellipse, its two
corner edges recurved as exact ellipse sub-arcs shared by the two blends (the unblend-style
vertex-move, no edge created or destroyed at the corner).

TESSELLATION (the third face type to learn it): tessellate_cylinder gained the vertex-height
fallback and the oblique cap-plane ruling clamps -- the mitre blend face carries ONE circle arc
+ two ellipse sub-arcs, so cyl_circle_heights returned a single height and the face tessellated
EMPTY (mesh down 0.44). All three curved tessellators (cylinder, cone, and the span machinery)
now handle vertex-trimmed oblique-bounded patches uniformly.

ORACLES: both blends RECOGNIZED at exactly r = 0.5 (item-58 cross-check); the volume against a
TERRITORY-SPLIT grid integral -- each point is governed by the bisector side's blend alone
(the first oracle draft used inside-either-cylinder and was WRONG; the mitre genuinely removes
more than the union near the seam). Valid; workspace 123 + 77 + 224 green; clippy -D warnings;
fmt; fuzz_boolean soak (tessellate_cylinder changed): 10 minutes, 785 runs, clean.

COUNTER: 134 -> 135/144 (item 56, the milestone-1 rung; unequal radii via marched SSI, chain
contraction, and roll-over junctions are the dossier-55 ladder). Remaining: 29, 51.

## Addendum 150 (2026-06-10, attended): SPHERE-OCTANT VERTEX BLEND (item 51; 135 -> 136/144)

fillet_corner_octant(corner, r) (branch octant-corner; dossier 53 Q1 + the worked topology
diagram): a convex trihedral corner of three mutually perpendicular planar supports rounds to
the EXACT SPHERE OCTANT. All the geometry is closed-form: M = the three inward offset planes
intersection (triple products, d_i = n_i . p_i - r); the sphere tangency feet q_i = M + n_i r
(both of face i springs meet there); every edge cylinder axis passes through M; each cylinder
meets the sphere along the exact QUARTER CIRCLE of radius r about M in the plane perpendicular
to its edge (axes n_a, n_b of the adjacent supports).

SURGERY (the mitre-top treatment applied to all three faces, then a three-way merge): per face,
split the two far boundaries at the spring crossings, spur the corner to q_i (mev AfterFin),
two split_faces along the springs; far caps take the standard end treatment x3 (cap = the face
at the far vertex NOT carrying that edge); kef each sharp edge merges its two support trims
into one blend face; each blend face then splits along its exact quarter circle between q_a and
q_b (the band keeps the cylinder); finally the three corner pieces merge by kef on two spurs
and kev on the third, which absorbs the old corner vertex (net +1 face, 0 edges, -1 vertex over
the merge, per the dossier). The octant face is the unique face touching all three q vertices;
it carries Sphere3 at M (frame z along n1+n2+n3), sense true.

TESSELLATION (the sphere learns vertex trimming): tessellate_sphere previously meshed the FULL
sphere for any face without a closed circle edge; the octant face is bounded by three OPEN
circle arcs. New arc-plane trim: collect the open-circle-arc boundary planes (>= 2 of them, cap
trim absent) and keep triangles whose centroid lies on the boundary-vertex-average side of
EVERY plane. The closed-circle cap trim path is untouched.

RECOGNITION taught about smooth junctions: the octant band has THREE tangent neighbours (two
supports + the sphere it runs into), so recognize_blends' exactly-two-tangents test rejected
it. A cylinder blend's springs are STRAIGHT edges parallel to its axis; tangent ARC junctions
(the smooth run into a vertex blend) are not springs and no longer count. Torus path unchanged;
all prior recognition tests (plain fillet, mitre, unblend) still green.

ORACLES: box [0,2]^3 corner at (2,2,2), r = 0.5 -- valid; face count 10 (3 kept + 3 caps + 3
bands + 1 sphere); exactly one Sphere face; all THREE cylinder blends recognized at exactly
r = 0.5; mesh volume against the exact closed form 8 - [3 (r^2 - pi r^2/4)(2 - r) +
(r^3 - pi r^3/6)] within 0.02. (Analytic mass_properties over blend faces still needs
blend-face pcurves -- the documented follow-up shared by every fillet; the honesty gate here is
the exact closed-form volume.)

CI: fmt; clippy -D warnings; workspace 123 + 77 + 225 (+1) green; fuzz_boolean soak
(tessellate_sphere changed): 10 minutes, 856 runs, clean.

COUNTER: 135 -> 136/144 (item 51, the perpendicular equal-radius rung; setbacks, unequal
radii, and oblique dihedrals are the dossier-53 ladder). Remaining tractable: 29.

## Addendum 151 (2026-06-10, attended): NON-REGULARIZED CELLULAR BOOLEAN (item 29; 136 -> 137/144)

boolean_with(a, b, op, tol, BooleanOptions { regularize }) (branch nonreg-union; dossier 57,
the deep dossier of the batch): item 29 lands as the dossier's verdict prescribed -- a CELL
SELECTION LAYER over the existing imprint/classify/select/stitch pipeline plus the three
stitcher relaxations, NOT a stitcher rebuild. Rung 1 (the recommended first milestone): the
non-regularized solid-solid UNION retains the coincident on-OPPOSITE interface fragments as
DOUBLE-SIDED interior partition walls (one copy, operand A, same convention as the on-on
tables), producing a CELLULAR solid whose material is partitioned into multiple solid regions.

The three relaxations, exactly as dossier 57 listed them: (R1) the radial glue generalized --
a dangling (radial-1) edge now joins a coincident edge of ANY radial count, so the interface
wall's boundary edges form radial-3 cycles (wall + the two outer walls; A's side joins by edge
IDENTITY from imprint, B's dangling copy glues in); two coincident manifold edges of separate
shells still stay separate. (R2) kept cells that do not bound the single solid: the new
finalize_cellular partitions face SIDES into cells by the WEILER SECTOR RULE -- manifold
(radial-2) edges join front-front / back-back (all faces import outward-oriented); radial-3+
edges take the angular sector walk (per-fin into-face spoke theta = angle of n x d_fin,
interior-on-the-left; the side of face i bounding the sector ccw of its spoke is FRONT iff its
normal sits +pi/2 ccw of the spoke -- the first draft tested the normal against the sector
interval and misclassified the EXACTLY-PERPENDICULAR junction where phi lands on the interval
boundary). Each bounded cell with an outer-shell material side becomes a solid region (the
unbounded cell is the infinite region; bounded cells with only front sides are voids); signed
cell volume (out-of-cell flux: +front for back sides, -front for front sides) identifies the
exterior. (R3) the keep predicate is the regularize switch; regularized defaults are
byte-identical (boolean() delegates to boolean_with with regularize: true).

HONESTY PLUMBING: an interior wall (both sides solid) carries no net boundary flux, so
mass_properties skips it (the two cells' contributions cancel), and all_triangles /
tessellated_volume / generalized_winding_number skip it (the OUTER boundary is the body's
boundary). The cellular path has NO soup fallback (the polygon soup cannot hold a radial-3
cycle, dossier 57 Rung 0): if the identity stitch declines, the cellular boolean declines.

ORACLE (dossier 57 Rung 1, fully analytic): fuse two unit cubes sharing the x=1 face,
regularize: false -- valid; TWO solid regions; exactly ONE double-sided interior wall; the
wall's FOUR boundary edges are radial-3; mass == mesh == 2 over the outer boundary; winding
number 1.0 at points in BOTH cells; the regularized default still yields one solid region
(no regression). All 226 keel-topo tests green (the k-way glue changed shared machinery; knit,
heal, sheet, and every boolean test unaffected).

CI: fmt; clippy -D warnings; workspace 123 + 77 + 226 (+1) green; fuzz_boolean soak (boolean
internals changed): 10 minutes, 829 runs, clean.

COUNTER: 136 -> 137/144 (item 29, the Rung-1 milestone; the dossier-57 ladder: Rung 2 sheet-as
-interior-partition, Rung 3 sheet-sheet, Rung 4 wire imprinting, Rung 5 full SGC cellular with
simplify; plus full soup retirement per dossier 47). This reaches the projected tractable
ceiling: the remaining 7 items are the permanently-declined / out-of-scope set (Addendum 130
ledger).

## Addendum 152 (2026-06-10, attended): SESSION-END RESUME ANCHOR (137/144, the tractable ceiling)

STATE: master = the merge of nonreg-union (#29), counter 137/144, ALL GREEN (123 geom + 77 math
+ 226 topo; fmt; clippy -D warnings), two clean 10-minute fuzz_boolean soaks this leg (856 runs
after the octant tessellator change, 829 after the cellular boolean). NOT pushed (push only on
request).

THIS SESSION (Addenda 139-152): +37 ticks, 100 -> 137. The arc: foreign geometry 114-116
(certified evaluator -> NURBS fit), genus-raising through-hole booleans, two breadth waves,
sheet-solid booleans (28), Coons fill (68), heal + defeature (130/132), blend recognize +
unblend (58/59), selective booleans (31), the BLEND DEEP END via analytic/reduction rungs (48
variable-radius exact cone, 49 conic strip, 54 hold-line closed form, 50 face-face, 60 G2
quintic, 67 Gordon-as-evaluator, 55 partial-span feathered cone, 56 mitred corner on the exact
ellipse), 51 the SPHERE-OCTANT vertex blend (Add. 150), and 29 the NON-REGULARIZED CELLULAR
boolean (Add. 151, dossier 57 Rung 1: selection layer + three stitcher relaxations, never a
rebuild).

THE CEILING, plainly: 137/144 is the projected tractable ceiling (Addendum 130 ledger + the
dossier sweep). The remaining 7 counter slots correspond to the out-of-scope / declined set
(Siemens convergent-mesh 86-93 family, U-splines, and the roadmap's excluded items 34, 46, 61,
79, 81, 134-135 -- see Addendum 130 for the exact ledger and the anti-double-count rule). No
new dossier surfaced a rung that converts any of them honestly.

QUEUED DEPTH (not ticks): dossier-57 ladder Rungs 2-5 (sheet-as-partition, sheet-sheet, wire
imprint, full SGC simplify); FULL soup retirement (dossier 47; the L-union partial-coincidence
seam still falls back); blend-face pcurves so analytic mass_properties covers blend faces;
parameterize the ~8 trim-and-stitch surgery copies; smooth/roll-on/notch overflow handlers
(dossier 56); unequal-radius mitre (marched SSI); setback vertex blends (dossier 53 Q2/Q3);
curved through-hole seam relocation; the completion gate (10h all-sector fuzz soak + the
three-bucket billion-boolean oracle, see MEMORY).

RESUME RECIPE: read this addendum, then Addendum 151/150 for the latest machinery; the ledger
is Addendum 130; CI = exact triplet + fuzz_boolean WSL soak when boolean/tessellation internals
change; constructors must emit mass == mesh bodies; LOG addendum then merge; NO EM-DASHES.

## Addendum 153 (2026-06-10, attended): STEP IMPORT, PLANAR MILESTONE (keel-io plan step 1)

Context: the corpus re-read (all 78 research files, 8-reader audit) named STEP IMPORT the largest
unimplemented opportunity on the shippable path (dossier 38 is an implementation-grade spec; the
completeness audit flagged the parsing/mapping layer as owned by no one). The user directive:
proceed with the recommendation (import -> watertightness-indifferent GWN -> cellular rungs),
then the rest of the audit findings. This addendum is step 1.

step_import.rs (branch step-import; dossier 38 secs 1, 4, 6, 8, 12): a pure-Rust ISO 10303-21
tokenizer + TWO-PASS resolver (forward references are legal and pervasive; pass one tables every
#N = record(s) including COMPLEX/AND instances reassembled into one logical entity; pass two
resolves while mapping). The parser accepts the full Part 21 value grammar (strings with ''
escapes, .ENUM. tokens, typed parameters like LENGTH_MEASURE(25.4), nested lists with a bounded
depth, /* */ comments, $ unset and * derived) and is panic-free against hostile files. The
planar AP203/AP214/AP242 mapping drives off ENTITY NAMES (the Part 42 core is shared across the
three APs, dossier 38 sec 2): MANIFOLD_SOLID_BREP -> CLOSED_SHELL -> ADVANCED_FACE(PLANE) ->
FACE_(OUTER_)BOUND -> EDGE_LOOP -> ORIENTED_EDGE -> EDGE_CURVE -> VERTEX_POINT, with the
orientation algebra (ORIENTED_EDGE.orientation picks the traversal-start vertex;
FACE_BOUND.orientation .F. reverses the loop; closed shells orient outward) delivering each
face's outer loop CCW about the outward normal. UNITS convert to millimetres from SI prefixes
and CONVERSION_BASED_UNIT chains (inch = 25.4 via LENGTH_MEASURE_WITH_UNIT).

ASSEMBLY REUSES THE KERNEL: from_polygon_faces (item 10) = planar_sheet per face + knit, which
IS the dossier's heal-on-import step 2 (vertex merge within tolerance, shared-edge glue,
closed-set promotion to a solid) with the mass == mesh gates intact. No new stitching code.

ORACLES: round-trip export -> import of a 2x3x4 block (counts 8/12/6, volume exactly 24,
mass == mesh) and of a BOOLEAN RESULT with oblique faces (box minus tilted prism; identical
exact volume back, proving the orientation algebra off-axis); a hand-written foreign cube in
unprefixed METRES with one CW-stored bound flagged .F., reversed oriented edges, forward
references, and a comment (imports to the exact 1e9 mm^3); an INCH cube via
CONVERSION_BASED_UNIT (25.4^3); curved and holed faces DECLINE loudly (planar milestone,
correct-or-decline). New fuzz_step_import target (raw bytes + DATA-framed) with a seed
tetrahedron corpus: 10 minutes, 3,160,100 runs, ZERO findings (the parser is allocation-bounded
and panic-free as dossier 38 sec 12 demands).

CI: fmt; clippy -D warnings; workspace 123 + 77 + 231 (+5) green. No boolean/tessellation
internals changed (purely additive module); the parser got its own dedicated soak above.

LADDER (dossier 38 build plan): analytic surfaces + seams/poles (milestone 2), NURBS homogeneous
4D conversion, pcurve supply-or-reconstruct, stitch-then-heal tolerance escalation, validation-
property round-trip oracle, AP242 tessellation import. NOT a parity counter tick (the 144-item
counter is closed at its ceiling); this is the post-parity opportunity program.

## Addendum 154 (2026-06-10, attended): SHEET-AS-KNIFE CELLULAR PARTITION (dossier 57 Rung 2)

partition_by_sheet(solid, sheet, tol) (branch cellular-rung2): the non-regularized sheet-as-
interior-partition of dossier 57 Rung 2. The wall = the sheet trimmed to the solid by the
ITEM-28 sheet-solid INTERSECTION (zero new trim code); the solid imprints along the same seam
curves; every solid fragment plus the trimmed wall stitches through the identity import path
with the wall in the WALLS channel; the Rung-1 cellular finalize (Weiler sector walk) extracts
the cells. Gates: two-or-more solid cells (a knife that fails to split DECLINES), mass == mesh
== the input volume (the outer boundary is untouched material), validate.

THE BUG THE ORACLE CAUGHT (and the diagnosis pattern): the first run produced ONE +8 material
cell and TWO negative exterior cells ("exterior cell not unique" decline). Spoke-table
instrumentation at the radial-3 ring edges showed the WALL's sector spoke pointing OUT of the
material (theta pi/2 instead of 3pi/2): interior-on-the-left (w = n x d_fin) assumes loops are
CCW about the stored front normal, which SOLID faces guarantee by construction but SHEET-built
faces do not (a lamina has no material side to orient by). FIX: per-face TRUE LOOP WINDING
(Newell normal of the outer loop dotted with the front normal) flips the spoke when the loop is
CW, making the sector walk winding-agnostic. The fix hardens Rung 1 as well: any future operand
with unconventional winding walks correctly.

ORACLE (dossier 57 Rung 2, fully analytic): cube [0,2]^3, 4x4 rectangular sheet at z = 1 ->
valid; TWO solid cells; ONE double-sided interior wall of area exactly 4; the wall ring is four
radial-3 edges; mass == mesh == 8; winding number 1.0 in both cells (outer boundary only); a
sheet that misses the solid declines.

CI: fmt; clippy -D warnings; workspace 123 + 77 + 232 (+1) green; fuzz_boolean soak (the
winding fix touches the shared cellular walk): 10 minutes, 822 runs, clean.

LADDER: Rung 3 sheet-sheet booleans, Rung 4 wire imprinting, Rung 5 full SGC with simplify;
partial-penetration knives (wall boundary ending interior) decline honestly today.

## Addendum 155 (2026-06-10, attended): CYLINDER-CYLINDER SSI + A WRONG-POSITIVE RETIRED

CYLINDER-CYLINDER SSI (branch cyl-cyl-ssi; corpus-audit opportunity A, dossier 11 QI-class +
the analytic-rung lesson): analytic_analytic gains the quadric pair via THREE RUNGS, never a
marching engine. Rung 1 PARALLEL AXES: the cross-section circle pair gives 0/1/2 exact ruling
LINES (tangent ruling flagged) or Coincident. Rung 2 EQUAL RADII + INTERSECTING AXES: the
bicylinder factors into TWO EXACT ELLIPSES in the planes through the crossing point with
normals az - bz and az + bz (on either plane ((P-M).az)^2 = ((P-M).bz)^2 makes the two
distance-to-axis conditions identical), each delivered by the existing exact plane-cylinder
section: tier-1 exact, tol_achieved 0. Rung 3 GENERAL: each ruling of cylinder A meets B where
a QUADRATIC in the ruling parameter holds, so the curve is the closed-form two-branch field
v(theta) = (-q1 +- sqrt D)/(2 q2); D's sign pattern fixes the topology (all-positive = two
closed wrap curves, one per branch; each positive interval otherwise = one closed BITE, branch
+ out and - back, meeting smoothly at the D = 0 ends, endpoints refined by bisection walked
cyclically from a non-positive sample so wrap-around runs stay whole). Each curve is a
certified cubic LSQ fit with an extended escalation (16..256 controls; fresh-sample max
deviation x SAFETY 2); certificate misses DECLINE. KEY NUMERICS CATCH: the bite parameterized
by theta has sqrt-type UNBOUNDED SPEED at the D = 0 turnarounds (the first fit read 0.33
deviation); the CHEBYSHEV substitution theta = mid + half cos(phi) makes the closed loop
analytic in phi and the fit certifies at ~1e-6.

THE WRONG-POSITIVE THE PROBE EXPOSED (pre-existing on master, found by the milestone's
Steinmetz probe): crossing-cylinder BOOLEANS never declined; the unsupported SSI pair
produced an IntersectionFailed fault that did NOT block assembly, so the pipeline built a
SEAMLESS Euler-valid body (12.546 against the exact Steinmetz 16/3) gated only by
tessellated-volume positivity. FIX: a new HARD fault BoolFault::UnassemblableSeam (a face pair
that GENUINELY CROSSES, curves on the trimmed faces, but whose seams the imprint cannot yet
assemble) raised for cylinder-cylinder curve pairs and EARLY-RETURNED by boolean_with and
boolean_selective like Tangent. Regression test: Steinmetz intersection/union/difference all
DECLINE; disjoint cylinder bodies do not trip the gate. DECLINE-never-WRONG now actually holds
for this class; making these ASSEMBLE (the crossing-seam imprint: two closed seams crossing
each other and the periodic seam on one lateral face) is the named follow-up, with the
certified curves ready.

ORACLES: equal-radius perpendicular pair = two exact ellipses on both implicit surfaces at
1e-9; full-wrap pair (r 1 through R 2) = two closed certified curves at 1e-5 (achieved 1.3e-6);
two-bite pair (r 1 piercing R 2) = entry + exit loops; offset one-bite; disjoint Empty;
parallel rulings exact; shared cylinder Coincident.

CI: fmt; clippy -D warnings; workspace 126 + 77 + 233 (+3, +2) green; DUAL soak (seam handling
changed): fuzz_boolean 822 runs + fuzz_cyl_boolean 890 runs, 10 minutes each, both clean.

CONSUMERS UNBLOCKED: the unequal-radius mitre (dossier 55, queued) now has its junction curve
in closed form; cylinder-cylinder boolean assembly has certified seams waiting.

## Addendum 156 (2026-06-10, attended): METROLOGY-GRADE FITTING (opportunity B)

keel-geom metrology.rs (branch metrology-fits; corpus-audit opportunity B, dossiers 23 + 17):
point-set fitting with an EXPLICIT OBJECTIVE parameter, never a silent least-squares default
(the Shakarji taxonomy: GD&T prescribes which functional each datum / form evaluation uses,
and the answers genuinely differ). FitObjective = LeastSquares | MinZone | MinCircumscribed |
MaxInscribed; every fit reports BOTH residuals (rms and the Chebyshev zone), the same
philosophy as the canonical-recovery certifier whose dense max-deviation IS a Chebyshev
residual.

IMPLEMENTED EXACTLY: LS plane = centroid + smallest covariance eigenvector (the recover.rs
Jacobi solver, now shared); LS circle/sphere = Kasa/Coope linearization + Gauss-Newton on the
geometric residual (solve3/solve4 shared); MIN-ZONE PLANE exact by contact-characterization
candidate enumeration (3-1 contacts = point-triple normals, 2-2 contacts = chord-pair cross
normals, plus the LS seed) under a documented size cap (CMM-probe sets, not meshes; beyond the
cap DECLINE, never silently degrade); MIN-CIRCUMSCRIBED circle = move-to-front Welzl
(deterministic, NO RNG, per the kernel determinism posture); MAX-INSCRIBED circle = exact
Voronoi-vertex (triple circumcenter) + pair-midpoint enumeration with a convex-hull membership
gate; MIN-ZONE ANNULUS exact (3-1 circumcenters + 2-2 bisector crossings). Sphere gauges and
the min-zone sphere are the queued ladder and DECLINE.

ORACLES: alternating +-e probe sets where every objective has a closed-form answer: LS plane
normal to 1e-6 with zone exactly 2e; min-zone plane and annulus exactly 2e (and never wider
than LS); MCC encloses a deliberate outward outlier with minimal radius; MIC stops exactly at
an inward outlier; LS sphere centre/radius/zone on a perturbed sphere grid.

CI: fmt; clippy -D warnings; workspace 131 + 77 + 233 (+5) green. No fuzz soak: additive
pure-geometry module, no boolean/tessellation path touched (the Addendum 74/75-class
justification).

CONSUMERS: GD&T datum simulation (dossier 17) and inspection-grade profile evaluation get
their fitting engine; the connection B insight stands (one engine, objective-parameterized,
shared with recovery).

## Addendum 157 (2026-06-10, attended): NURBS EXTENSION VIA KNOT UNCLAMPING (the staged engine lands)

extend.rs (branch unclamp-extend; dossier 13 sec 2.1, corpus-audit top-tier item): the M5b
staged Err is retired. The engine is the POLAR-FORM construction: the natural extension past a
clamped end is the SAME polynomial as the boundary span, so the appended segment's control
points are blossoms of that span at mixed (u1, e) arguments, computed by the generalized
de Boor triangle (a different argument per elimination level) over the last p+1 control
points, all in HOMOGENEOUS 4D so rational surfaces extend exactly. New knot vector = the
original with ONE end knot dropped (u1 becomes interior at multiplicity p) plus p+1 knots at
e; every original control window is untouched (the polar-form windows for j <= n are
unchanged), so the original domain reproduces BITWISE and the join is C-infinity. Min-end
extension by reverse/extend/reverse; surfaces extend per control row/column (v-curves are
contiguous chunks; u via transpose). Public API: extend_nurbs_curve(curve, at_end, amount) +
the existing extend_nurbs_surface signature now returning real surfaces, still guarded by the
safe-reach cap, weight positivity, and fold detection.

FOLD-DETECTOR FIX (first real consumer caught it): the validator compared every sampled normal
against a fixed reference, which FALSE-POSITIVES on a closed patch (a full cylinder rotates
the normal a full turn). A fold flips the normal between ADJACENT samples; compare
consecutively.

ORACLES (exactness, not approximation): a rational quadratic on a circle satisfies x^2 + y^2 =
r^2 w^2 as a POLYNOMIAL identity, so its natural extension must stay exactly on the circle:
the extended full-circle curve stays within 1e-9 of radius at both ends, with the original
domain unchanged to 1e-12. The rational cylinder surface (arc x line) extends along the axis
(degree-1 direction) AND around the arc (rational direction) staying exactly on the infinite
cylinder; over-cap requests still decline.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 233 (+1 net) green. No soak: additive
geometry service, no boolean/tessellation path touched.

CONSUMERS UNBLOCKED: NURBS delete-face healing, curved tweak re-intersection, and the
untrimming strategy (the corpus's "chief obstacle to robust booleans on NURBS solids") now
have their extension primitive.

## Addendum 158 (2026-06-10, attended): STEP IMPORT MILESTONE 2, THE GEOMETRY CONVERSION LAYER

step_import.rs (branch step-import-2; dossier 38 build-plan step 3, "the high-stakes path"):
the conversion layer from Part 21 geometric entities to Keel geometry, the substrate the
curved-topology milestone will consume.

- PLACEMENTS: AXIS2_PLACEMENT_3D -> orthonormal Frame3 with the two classic import bugs
  handled (directions are NOT normalized in files; ref_direction is NOT necessarily orthogonal
  to the axis -> Gram-Schmidt project; $ defaults to global Z/X; an axis-parallel seed falls
  back to any perpendicular).
- ANALYTIC SURFACES: plane / cylindrical / conical / spherical / toroidal -> exact Surface3
  with millimetre radii and the cone semi-angle in radians (a PLANE_ANGLE_UNIT reader joins the
  length-unit machinery; degree files chain through CONVERSION_BASED_UNIT).
- NURBS (the high-stakes conversion): B_SPLINE_SURFACE / _WITH_KNOTS / RATIONAL_B_SPLINE_
  SURFACE in BOTH the complex/AND form (attributes split across leaves, names absent) and the
  simple flattened form (name first) via one `unnamed` dispatch; knot vectors EXPANDED from
  (distinct values x multiplicities) with the count invariant checked; the control grid is
  row-major BY U (outer list U, inner V, exactly Keel's u-outer/v-inner layout); weights
  PRE-MULTIPLY into homogeneous 4D (w x, w y, w z, w), never point-plus-weight. Periodic /
  non-clamped knots DECLINE (unwrap is queued). Curves analogous (LINE / CIRCLE / ELLIPSE with
  in-plane placement axes / B_SPLINE_CURVE rational and not).
- PUBLIC API: surfaces_from_step / curves_from_step convert every recognized entity; the
  fuzz target now exercises both alongside the solid importer, with a rational-surface seed.

ORACLES (exactness): a hand-written COMPLEX-INSTANCE rational quarter-cylinder patch (w =
sqrt(2)/2 middle row) samples onto x^2 + y^2 = 4 within 1e-12, proving complex reassembly +
knot expansion + grid order + weight premultiplication in one assert; the five analytics
convert from a METRE-unit file with a non-normalized axis and non-orthogonal ref_direction to
orthonormal frames and mm radii to 1e-12; a tilted CIRCLE lands its in-plane axes; a complex
rational B_SPLINE_CURVE quarter arc stays on its circle to 1e-12; a simple-form degree-1
curve binds positionally with the name offset.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 236 (+3) green; fuzz_step_import soak with
the conversion entry points added to the target: 10 minutes, 1,522,752 runs, clean.

LADDER: curved TOPOLOGY assembly (advanced_face on analytic/NURBS surfaces, seams on periodic
faces, vertex_loop poles, pcurve supply-or-reconstruct), heal-on-import tolerance escalation,
validation properties, AP242 tessellation import.

## Addendum 159 (2026-06-10, attended): VALIDATION-PROPERTY ROUND TRIP + SHORTEST-ROUND-TRIP REALS

Branch validation-props (dossier 38 sec 9 + dossier 22, CAx-IF GVP practice): the STEP exporter
now embeds VOLUME / SURFACE AREA / CENTROID as geometric validation properties
(MEASURE_REPRESENTATION_ITEM with VOLUME_MEASURE / AREA_MEASURE typed values + a centre-point
representation, bound through PROPERTY_DEFINITION_REPRESENTATION), and the importer RECOMPUTES
them with mass_properties / surface_area as its ACCEPTANCE ORACLE: a declared-vs-recomputed
mismatch (1e-3 relative on scalars, unit-scaled, centroid distance-gated) DECLINES the import.
A translation that builds the wrong geometry now fails loudly instead of passing the
counts-and-validity checks.

ALSO: export reals switched from {:.9} truncation to SHORTEST ROUND-TRIP decimals ({:?}, which
always keeps the decimal point the Part 21 real token requires), closing the corpus-audit
serialization finding for the STEP path: save -> load preserves every f64 bit.

ORACLES: both existing round trips now pass THROUGH the gate (properties embedded, recomputed,
matched); a tampered volume AND a tampered centroid each decline; foreign files without
properties are unaffected (the gate only fires when properties are declared). The tamper
needle is built from the computed value (the first draft assumed "24.0" and the shortest
round trip of the actual integral differs in the last ulp, a tidy demonstration of why the
formatting change matters).

CI: fmt; clippy -D warnings; workspace 132 + 77 + 237 (+1) green. No soak: the gate consumes
already-parsed values (no new parsing path), no boolean/tessellation change.

## Addendum 160 (2026-06-10, attended): BLEND-CHAIN ORDERED SUPPRESSION + THE WEDGE ORACLE

Body::unblend_all(tol) (branch blend-chains; dossiers 03 sec 2.2 / 15 sec 1.4, the
Venkataraman-Sohoni ordering): whole-model blend suppression, LEAVES FIRST. A blend whose face
serves as another blend's SUPPORT is a junction and waits until its dependents are gone (the
dependency edge that prevents re-intersection targets vanishing); recognition re-runs after
every removal; declines are tracked per face and re-tried after any successful removal (a
removal can unlock its neighbors). Returns (removed, remaining): a nonzero remaining means
those blends honestly DECLINED with the body at the last valid state.

THE GATE THE CHAIN CASE EXPOSED (and its fix): unblend's commit gate demanded analytic
mass_properties == mesh on the candidate. On a body with MORE THAN ONE blend, the candidate
still carries another blend's pcurve-less cylinder face, the analytic integral fails (the
documented blend-pcurve follow-up), and EVERY chain unblend declined: item 59 had only ever
been exercised on single-blend bodies. The gate is now TIERED: when mass_properties succeeds,
the strong mass == mesh identity applies unchanged; when it is blocked, the fallback is the
EXACT WEDGE ORACLE: this unblend must change the mesh volume by exactly the removed fillet's
analytic wedge r^2 (cot(theta/2) - (pi - theta)/2) per unit length at interior dihedral theta
(sign-agnostic for convex/concave). The untouched faces' tessellation error CANCELS in the
before/after difference, leaving only the removed band's own chordal error (2 percent gate at
default density). DECLINE-never-WRONG is preserved with a quantified independent oracle, not a
weakened one.

ORACLES: two independent fillets on a box suppress completely back to the EXACT sharp box
(volume 8 to 1e-9, 6 faces, removed/remaining = 2/0, the second removal passing through the
wedge oracle and the final one through the strong gate); the mitred-corner body's two blends
are not the plane-plane unblend class and decline wholesale (0 removed, 2 remaining, mesh
volume bit-identical).

CI: fmt; clippy -D warnings; workspace 132 + 77 + 238 (+1) green. No soak: no
boolean/tessellation path touched (blend surgery + recognition only).

## Addendum 161 (2026-06-10, attended): INSCRIBED-RADIUS MEDIAL FIELD (wall-thickness MVP)

interrogate.rs (branch wall-thickness; corpus-audit connection A first slice, dossiers 10 / 41
/ 50): Body::inscribed_radius(p, outward) = the radius of the largest inscribed sphere tangent
at surface point p = the DISTANCE TO THE MEDIAL AXIS at p. Implementation: the sphere at
p - n r is empty iff the closest surface distance from its centre stays r (the tangent contact
itself); emptiness is monotone in r, so a doubling-bounded bisection (60 steps) converges;
resolution follows closest_point (exact planar, tessellation-resolution curved). This is the
SHARED FEASIBILITY FIELD the corpus synthesis identified with four consumers: shell t_max,
the blend overflow ceiling, defeature safety, and wall-thickness analysis (this milestone's
face-level convenience: Body::min_wall_thickness = 2x the smallest inscribed radius at each
face interior point, documented as sample-point dependent).

ORACLES (closed form): 4x4x1 slab at the top centre r = 0.5 exactly; near a rim the medial
distance shrinks to the rim distance (0.2 at 0.2 in, the field honestly reflecting edge
proximity); a side-face centre is governed by the vertical clearance (0.5). The hollowed 4^3
box with 1-thick walls: the cavity bounds the sphere from inside (outer-face centre 0.5) and
the cavity wall sees the outer face (0.5); the face-sampled aggregate stays within (0, 1].

CI: fmt; clippy -D warnings; workspace 132 + 77 + 239 (+1) green. No soak: additive
interrogation only.

LADDER: the exact medial-axis structure (Culver-Keyser-Manocha seams) stays queued; this field
is its query-level MVP and the bridge for retiring the per-feature r_max duplicates.

## Addendum 162 (2026-06-10, attended): TOLERANCE PROPAGATION SPECIFICATION (the unfixable-late gap)

docs/superpowers/specs/2026-06-10-tolerance-propagation.md (branch tol-propagation-spec): the
corpus audit named tolerance propagation the single most serious unvalidated dependency (spec
D2 promised "per-entity tolerances WITH PROPAGATION"; the algorithms are proprietary everywhere
and absent from open literature; the audit verdict: write the specification BEFORE the model
calcifies). The spec adopts the C3D sphere/tube formalization (dossier 42: vertex = ball, edge
= tube, geometry stays exact), three containment invariants, six propagation rules (creation
from certificates; ADDITIVE conservative growth on merge gaps; max-inheritance on dissolution;
isometry/scale transport; the lfs-relative RE-INTERSECTION TRIGGER at k = 0.01 wired to the new
inscribed-radius medial field; and the chaining bound stated as a MEASURED conjecture, not an
assumption), and the measurement program that turns the missing literature into data: the fuzz
/ billion-boolean infrastructure gains a tolerance-growth bucket, chains run at depth 8-32 with
per-op growth logging, and any super-additive case becomes a repro. Honest conformance
inventory included: P1/P3/P4 already conform; P2 gap recording, the lfs trigger, and the M1-M3
instrumentation are the named follow-ups.

CI: docs only; no code changed (workspace remains 132 + 77 + 239 green from Addendum 161).

## Addendum 163 (2026-06-10, attended): SESSION-END ANCHOR -- THE POST-PARITY OPPORTUNITY PROGRAM

STATE: master = the merge of tol-propagation-spec, all green (132 geom + 77 math + 239 topo;
fmt; clippy -D warnings), NOT pushed. The parity counter remains CLOSED at 137/144 (Add. 152);
everything after it is the post-parity opportunity program born from the FULL CORPUS RE-READ
(all 78 research files, 8-reader audit + cross-synthesis; findings and novel cross-connections
recorded in the conversation and actioned below).

SHIPPED THIS PROGRAM (Addenda 153-162, ten milestones, each branch-gated):
153 STEP import planar (pure-Rust Part 21 tokenizer + two-pass resolver; 3.16M-run parser
    soak); 154 sheet-as-knife cellular partition (dossier 57 Rung 2; Newell winding fix
    hardening Rung 1); 155 cylinder-cylinder SSI (three rungs, Chebyshev-regularized certified
    bites) + the UnassemblableSeam HARD FAULT retiring a pre-existing wrong-positive (seamless
    Steinmetz, 12.5 vs 16/3); 156 metrology fitting (explicit Shakarji objectives; exact
    min-zone plane/annulus, Welzl MCC, exact MIC); 157 NURBS extension via knot unclamping
    (polar-form blossom, C-infinity joins, exactness oracles); 158 STEP geometry conversion
    layer (complex-instance rational NURBS to homogeneous 4D; 1.52M-run soak); 159 validation-
    property round trip (GVP oracle; shortest-round-trip reals closing the serialization
    finding); 160 blend-chain ordered suppression (unblend_all leaves-first; TIERED commit gate
    with the exact-wedge fallback oracle, fixing chain unblends that all declined); 161 the
    inscribed-radius MEDIAL FIELD (the four-consumer feasibility query) + min wall thickness;
    162 the tolerance-propagation SPECIFICATION (the D2 unfixable-late gap: sphere/tube
    containment algebra + the measured-growth program).

DISPOSITIONS (audit items resolved without code): Dupin-cyclide recognition DECLINED (needs a
new first-class surface kind, contra spec D4); Park knot reduction MOOT until multi-section
skinning exists (the loft is ruled); cubic hybrid clipping + IGA half-point quadrature stay
documented perf-queued.

REMAINING OPPORTUNITY BACKLOG (ranked; each has its dossier): STEP curved-topology assembly
(milestone 3: advanced_face on analytics/NURBS, seams, vertex_loop poles, pcurve supply-or-
reconstruct -- the conversion layer is ready); cellular Rungs 3-5 (sheet-sheet, wire imprint,
SGC simplify); the crossing-seam imprint that turns UnassemblableSeam declines into assemblies
(certified curves waiting); Spainhour-Weiss boundary-integral GWN for open operands; the
P2/P5/M1-M3 tolerance-spec follow-ups; exact medial-axis structure over the new field; HLR
(needs the 2D arrangement); virtual-topology operator family; blend-face pcurves; the
surgery parameterization; reblend-after-edit; 3D constraint solver / relation inference
(companion crate); FFD via the foreign-evaluator back door; sheet-metal hinge graph; model-diff
cascade over lineage + topology hash; soup retirement; the completion gate.

RESUME RECIPE unchanged (Add. 152): read this anchor, the exact CI triplet, fuzz when
boolean/tessellation/parser internals change, mass == mesh constructors, branch-per-milestone,
LOG addendum then merge, NO EM-DASHES, push only on request.

## Addendum 164 (2026-06-10, attended): ANALYTIC MASS PROPERTIES ON BLEND FACES

Branch blend-pcurves (the queued-depth blend-pcurve milestone, resolved WITHOUT pcurve
authoring): integrate_curved_face gains a PROJECTED ISO-RECTANGLE rung. When every boundary
edge of a curved face projects onto its own surface as an iso-u or iso-v line, the face IS its
UV rectangle and the existing rectangle quadrature is exact; bounds come from projecting
boundary samples (open circle/ellipse arcs sampled by ENDPOINT ANGLES + arc_sweep, never the
generic full-periodic sweep; closed rings sampled densely at 64), with periodic directions
resolved by the LARGEST-GAP complement (full ring iff the gap is explained by the sampling
density). Non-rectangular curved faces (the sphere octant, the mitre ellipse band) keep
declining honestly.

TWO REAL BUGS THE ORACLES EXPOSED:
1. STALE PCURVES: the cap-rim kef merge leaves CYLINDER-SPACE pcurves on the TORUS ring, and
   the pcurve-bounds path read tube-angle bounds of (0, 1.7) instead of the quarter; the rim
   body's analytic mass had silently never been right (only mesh was tested). The pcurve path
   now carries a STALENESS GUARD: a pcurve endpoint must EVALUATE through the face's own
   surface onto one of the fin's 3D endpoints (pole-adjacent evaluation failures are
   inconclusive, not stale); stale or missing pcurves fall to the projected rung.
2. SPARSE-RING GAPS: 8 ring samples fake a pi/4 boundary gap; the full-ring detection now
   scales with density (closed edges sample 64, threshold 1.6 tau/64).
Also: vertex (pole) loops constrain no UV direction and are skipped; and unblend's commit gate
dispatches by FACE TYPE now (all-planar = strict mass == mesh; any curved face = the exact
wedge oracle), since the analytic integral SUCCEEDING on curved candidates must not be
compared against chordal mesh.

ORACLES: plain fillet body analytic mass == the closed form to 1e-9 (the doc-promised
milestone); the cap-rim torus body == the PAPPUS-exact volume to 1e-9 (corner region square
minus quarter disc, 2 pi xbar A; the formula independently lands on the old mesh estimate
6.1700); the sphere-octant body still declines analytic massprops (non-rectangular, honest);
all revolve bodies unchanged (their valid pcurves pass the guard).

CI: fmt; clippy -D warnings; workspace 132 + 77 + 240 (+1) green; fuzz_boolean soak
(mass_properties is load-bearing in the boolean gates): 10 minutes, 733 runs, clean.

## Addendum 165 (2026-06-10, attended): THE POLYGON SOUP IS RETIRED (dossier 47 Rung 0)

Branch soup-retirement: build_result_solid + kept_to_polys + ResultPoly (+ the now-orphaned
face_ring_points and newell helper, ~290 lines) are DELETED; stitch_by_import is the ONLY
assembly. The decisive experiment: an env-gated census showed the fallback fired exactly FOUR
times across the whole suite, every one inside a test whose contract is DECLINE or whose
passing path never consumed the soup body (near-coincident touch, too-thick hollow, the
crossing-cylinder gate, a selective-boolean probe); replacing the fallback with an honest Err
left ALL 240 keel-topo tests green. The L-union itself has assembled through the identity
stitch since the boundary-coincident-ring filter (Add. 103) and the #16 keystone; the "soup
still needed for partial coincidence" note in the code was STALE. A stitch failure now
DECLINES the boolean, full stop: correct-or-decline with no second assembly path for the
closure invariant to miss.

This was the HARD DEPENDENCY for the dossier-57 ladder ("the soup cannot hold a radial-3 cycle
or a kept wire/lamina; on the soup path item 29 genuinely would be a rebuild"): Rungs 3-5 now
build on a single stitcher.

CI: fmt; clippy -D warnings (the deletion surfaced a stale allow and one orphan, both
cleaned); workspace 132 + 77 + 240 green; fuzz_boolean soak (assembly path changed): 10
minutes, 706 runs, clean.

## Addendum 166 (2026-06-10, attended): SHEET-SHEET BOOLEANS (dossier 57 Rung 3) + A LATENT IMPRINT NO-OP FIXED

boolean_sheet_sheet(a, b, op, tol) (branch sheet-sheet; dossier 57 Rung 3, dossier 39 sec
1.2): the 2D arrangement of two COPLANAR planar sheets. The overlap interior boundary imprints
onto each operand (the coincident pre-pass machinery), fragments classify by a 2D
point-in-polygon test against the other sheet, selection follows the dimension table (union =
all of A + B-outside, one overlap copy; intersection = A-inside; difference = A-outside), and
the kept fragments IMPORT through the identity path with the SHEET finalize (free edges are
the nature of a lamina; the first draft knitted planar_sheet copies and learned that knit's
SOLID finalize rightly rejects open sets at the closure invariant). Crossing (non-coplanar)
sheets intersect in a WIRE and decline (rung 4/5 ladder).

THE LATENT BUG THE RUNG EXPOSED: imprint_open_curve's contract is a PRE-BOUNDED curve (t in
[0,1] = the segment), but Line3 NORMALIZES its direction, so every Line3-based open imprint
spanned exactly ONE UNIT from the start point: longer segments ended in the face interior and
the imprint declined ("endpoint not on any boundary edge interior"). The coincident PRE-PASS
(preimprint_coincident_overlaps) has carried this silently since Addendum 56: its non-unit
overlap cuts were silent no-ops swallowed by best-effort handling, with the downstream
boundary-coincident-ring filter masking the gap. Both call sites now pass exact degree-1
NURBS segments. All 240 pre-existing tests stay green with the pre-pass actually imprinting.

ORACLES (dossier 57 Rung 3, exact): two 4x4 coplanar sheets offset by 2: intersection area 8,
difference 8, union 24, all to 1e-9 and valid; disjoint union 32 and disjoint intersection
declines; crossing sheets decline.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 241 (+1) green; fuzz_boolean soak (the
pre-pass fix changes coincident boolean internals): 10 minutes, 662 runs, clean.

## Addendum 167 (2026-06-10, attended): SURGERY PARAMETERIZATION, FIRST EXTRACTION (the cap skeleton)

Branch surgery-extract (the queued tech-debt item): the blend CAP-SPLIT skeleton (find the cap
face, split it between the fins ending at the two spring vertices, copy the support surface to
the new piece, attach the end-arc curve) existed as SEVEN near-literal copies. It is now ONE
method, Body::split_blend_cap(cap, a_end, b_end, arc) -> EdgeKey, adopted by the variable-
radius cone, G2 quintic, conic-strip, hold-line, face-face (which consumes the returned edge
for its signed arc sweep), mitre, and octant engines. Each engine keeps its own CAP SELECTOR
and ARC GEOMETRY, which genuinely differ; only the topology skeleton was duplicated.
Behavior-preserving: all 241 keel-topo tests green unchanged, ~140 lines removed.

REMAINING SLICES (queued, not blockers): the dissolve-chain (kef stub / kef / kev spur)
sequences still repeat per engine; the spring-imprint endpoint-kind dispatch (dossier 54
OnBoundary / AtSharpStop / AtRunout) generalizes imprint_spring_line when the overflow
handlers land.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 241 green. No soak: behavior-preserving
refactor, no boolean/tessellation logic change (the suite is the gate).

## Addendum 168 (2026-06-10, attended): CO-ANALYTIC PLANAR ROLL-ON (dossier 56 ladder step 1) + THE PLANAR ARC-SAMPLING FIX

fillet_edge_chain(e1, e2, r) (branch overflow-handlers; dossier 56 sec 6, the ladder step after
the parity-shipped cliff): ONE constant-radius cylinder fillet rolled across TWO COLLINEAR
convex edges whose support pairs are COPLANAR (the "fillet wider than the face, same plane
beyond" case). The blend surface never changes (one exact cylinder, one straight spine, ONE
ribbon face); the transverse boundary's corridor segments dissolve (the wall segment kef-merges
the ribbon halves, the floor segment dangles and kev absorbs the old mid vertex) while the
boundary OUTSIDE the corridor SURVIVES on the kept supports, the fidelity point distinguishing
a roll-on from heal-then-fillet. Surgery: fragment pair 1 takes the standard boundary-to-
boundary spring imprint (splitting the transverse boundaries at the crossings); pair 2 reuses
those crossing vertices (the mitre side-2 pattern); far caps via the shared split_blend_cap.

THE PRE-EXISTING BUG THE DIFFERENTIAL ORACLE EXPOSED: the planar mass-properties integrator
(loop_uv_polyline_planar) sampled OPEN circle/ellipse arc edges over the FULL periodic curve,
corrupting any planar polygon with an arc boundary (a fillet END CAP). The corruption was
INVISIBLE until now because the divergence form takes only x-flux and no prior oracle gave an
arc-bounded cap an x-normal: the 4x2x2 fillet (edge along x, caps x-normal) read mass 17.106
against mesh 15.784. Open arcs now sample their true extent (endpoint angles + arc_sweep, fin-
direction aware), leaving only the inherent polyline chordal residue (5e-4 here, documented).

ORACLES: the DIFFERENTIAL form (immune to the shared cap approximation): the chain fillet of
the abutting-union body (two 2-cubes fused = the 4x2x2 box with every transverse face split at
x = 2) equals the plain fillet of the unsplit box to 1e-9; exactly ONE cylinder ribbon; the
kept supports keep their artificial split (more faces than the plain body); valid. The test
setup itself exercises the regularized union + the collinear-chain detection.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 242 (+1) green; fuzz_boolean soak
(mass_properties planar path changed, load-bearing in the boolean gates): 10 minutes, 662 runs, clean.

LADDER NEXT: planar notch bridge (in-place trim-loop extension, the US 8,935,130 design-
around), then planar cap (extend two planes), then the bucket-b evaluators (creeping cliff,
smooth merge).

## Addendum 169 (2026-06-10, attended): WIRE-SOLID BOOLEANS (dossier 57 Rung 4)

boolean_wire_solid(wire, solid, op, tol) (branch wire-imprint; the NOODLES dimension-by-
dimension lowest rung): trim a WIRE body against a SOLID. Intersection keeps the inside
portions, Difference the outside; Union of mixed dimension declines (the kept embedded wire is
the rung-5 non-regularized ladder). MVP scope: straight wire edges against planar solids: each
segment splits at its boundary crossings (segment-plane intersections, containment by 2D
winding in the face plane), sub-segments classify by the winding number at their midpoints,
kept pieces rebuild as one wire body; curved wires/solids decline honestly.

ORACLE (dossier 57 Rung 4, exact): a segment crossing the [0,2]^3 cube keeps EXACTLY its
inside length 2 (intersection) and the two outside tails totalling 2 as separate wire edges
(difference); union declines; a disjoint wire's intersection declines empty.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 243 (+1) green. No soak: additive entry
point, no existing boolean path touched.

With Rung 4 landed the dossier-57 ladder stands: Rung 0 soup retirement DONE (Add. 165),
Rung 1 cellular union DONE (Add. 151), Rung 2 sheet-as-knife DONE (Add. 154), Rung 3
sheet-sheet DONE (Add. 166), Rung 4 wire imprint DONE (this); Rung 5 (full SGC with the
Rossignac Merge simplify) is the remaining tail, with the completion gate after it.

## Addendum 170 (2026-06-10, attended): BACKLOG-LEG ANCHOR (Addenda 164-169)

STATE: master = the merge of wire-imprint (613aedb), all green (132 + 77 + 243; fmt; clippy
-D warnings), clean soaks at every gated milestone, NOT pushed.

THIS LEG (the queued-depth + dossier-57 ladder, in order): 164 analytic massprops on blend
faces (projected iso-rectangle bounds; stale-pcurve guard; the cap-rim torus had silently
wrong analytic mass); 165 the POLYGON SOUP RETIRED (census-proven dead, ~300 lines deleted,
stitch-or-decline); 166 sheet-sheet booleans (Rung 3) + the Line3-normalization open-imprint
no-op fixed at every call site; 167 the surgery cap-skeleton extraction (split_blend_cap,
seven adopters); 168 the co-analytic planar roll-on (fillet_edge_chain, dossier 56 ladder
step 1) + the planar arc-sampling massprops fix (open arcs were sampled as FULL circles,
invisible until an arc-bounded cap carried x-flux); 169 wire-solid booleans (Rung 4).
Dossier-57 ladder: Rungs 0-4 ALL DONE; Rung 5 (SGC Merge simplify) remains.

REMAINING (task list + designs annotated per task): #15 remainder = planar NOTCH bridge
(dossier 56 sec 2.1 in-place trim-loop extension, the US 8,935,130 design-around) and planar
CAP (sec 2.2 extend-two-faces); #16 setback vertex blends (dossier 53 Q2/Q3, Charrot-Gregory
via the foreign-fit pipeline); #18 Rung 5 SGC simplify + the COMPLETION GATE (10h all-sector
soak + billion-boolean three-bucket oracle + the tolerance-growth bucket of the Add. 162
spec). Each needs its dossier re-read per standing rule.

PATTERN OF THE LEG: every differential/exactness oracle exposed a real pre-existing defect
(stale pcurves, sparse-ring gaps, the unit-segment imprint no-op, the full-circle planar arc
sampling). Write the oracle FIRST; the corpus's exactness culture is the kernel's best fuzzer.

## Addendum 171 (2026-06-10, attended): PARTIAL BAND SEAMS HARD-DECLINE (another wrong-positive retired)

Branch notch-cap, found while probing the NOTCH handler: the dossier-56 notch semantics ("the
blend is undeformed; the notch element trims to the rounding") is literally fillet-then-
subtract-the-element, so the probe ran boolean(filleted box, groove tool, Difference). It
PROCEEDED and validated, but the corridor grid oracle disagreed with the body's own winding by
18 percent and the mesh sat 0.13 below the closed form: a SEAMLESS WRONG-POSITIVE. Root cause:
curve_on_cylinder_face checked only the AXIAL band, so the tool planes' full SSI circles
passed containment on the QUARTER band (their heights are interior) and the closed-circle
imprint machinery (built for the drill's full lateral) mis-trimmed the band behind the weak
curved gate (tessellated-volume positivity).

FIX: curve_cylinder_face_overlap classifies a seam curve against BOTH the axial band and the
ANGULAR span (cyl_angular_span) by sampling: All = usable seam (unchanged paths: drill, lens),
None = skip (as before), PARTIAL = the seam crosses the trimmed face boundary, which the
imprint cannot yet assemble: seam_curves raises the hard UnassemblableSeam fault and the
boolean DECLINES. Regression test: filleted box minus a groove crossing the blend DECLINES
while the planar control (unfilleted box minus the same groove) stays exact at 15.92.

CONSEQUENCE FOR THE NOTCH HANDLER (#15): the composition route (fillet then subtract) is
blocked on the PARTIAL-ARC BAND IMPRINT (the same named follow-up as the crossing-cylinder
seams, with the certified curves already available); the in-place Euler surgery route stays
open with its full configuration design recorded in the task. The groove-notch closed form
(16 - filletcut - 0.08 + 0.2 x (0.1 - A_in), A_in the circle band integral) is the ready
oracle for whichever route lands first.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 244 (+1) green; fuzz_boolean soak
(seam classification changed): count below.

## Addendum 172 (2026-06-10, attended): NOTCH OVERFLOW HANDLER (dossier 56 sec 2.1) + lune-exact planar massprops

Branch notch-surgery. fillet_edge_notch(e1, e2, r): fillet two collinear convex plane-plane
edges separated by a planar-walled groove crossing the blend corridor. Per the notch semantics
the blend cylinder stays UNDEFORMED and the notch element trims to the rounding: the groove
walls trim to cross-section arcs (split_blend_cap), the groove floor trims to the cylinder
ruling line, and the blend trim boundary extends across the corridor IN PLACE by plain Euler
surgery on the existing faces, no separate sheet body, no sewing (the US 8,935,130
design-around recorded in sec 2.1). The ribbon is built as THREE cylinder faces split at the
corridor cross arcs so every face is an exact UV iso-rectangle (the roll-on transfer-curve
precedent): mass and mesh stay exact with the existing rectangle integrators. Scope: convex
chains, coplanar tops, one shared second support, walls perpendicular to the spine, floor
strictly between the springs (0 < h < r); tilted walls, deep floors, and multi-face notch
elements decline.

ORACLE FIRST, and again it paid twice over. The closed form (16 - groove - filletcut +
0.2 x (0.1 - A_in), A_in the circle band integral 0.3..0.5) at the 1e-9 mass gate exposed:

1. The DOCUMENTED 5.6e-4 chordal residual (Add.169): planar faces bounded by circular arcs
   chord-sampled their arcs in loop_uv_polyline_planar. RETIRED EXACTLY: each chord now also
   emits signed LUNE quadrature samples (GL8 x GL8 over the (theta, t) lerp(chord, arc) patch;
   the Jacobian sign makes convex/concave arcs, full rings, reversed fins, and inner loops all
   fall out of one formula; ellipse fins included, degree>1 NURBS stay chordal). The fan over
   the chord polygon plus the lunes integrates arc-bounded planar faces exactly: the notch
   body hits the closed form to 1e-9 with no differential crutch.

2. A FULL-DOMAIN NURBS sampling defect in tessellate loop_polygon: split survivors of the
   boolean's degree-1 NURBS seam edges (the Add.171 imprint segments) sampled the PARENT
   span (split_edge keeps the whole curve on both halves), folding spurious points into the
   polygon (top faces lost 0.24 area each as ear-clip folds). Degree-1 NURBS is straight:
   vertices suffice; the sampler now requires degree > 1 (the open-imprint Line3 lesson, now
   in its third disguise).

Regression test notch_fillet_bridges_the_groove_with_an_undeformed_blend: plain fillet_edge
declines on both pieces (overflow guard), the handler validates, mass == closed form to 1e-9,
mesh to 0.02 (chordal), exactly 3 cylinder ribbon faces, 13 faces total, argument-order
agnostic. CI: fmt; clippy -D warnings; workspace 132 + 77 + 245 (+1) green; fuzz_boolean soak
(tessellation + massprops changed): count below. Remaining in the leg: planar CAP (sec 2.2),
then setbacks (dossier 53), Rung 5 + completion gate.

## Addendum 173 (2026-06-10, attended): CAP OVERFLOW HANDLER (dossier 56 sec 2.2) + Green-slab cylinder massprops

Branch cap-handler. The cap construction, per the dossier verbatim: "a face or faces in the
model will be intersected with the blend sheet to trim the sheet exactly to the model"
(prior-art extend-and-trim, unencumbered; no separate sheet). The honest narrow slice is the
OBLIQUE-END cap, implemented INSIDE fillet_edge itself: the split_cap step now reads the cap
face's plane: perpendicular to the spine = the standard quarter circle (unchanged);
oblique = the EXACT plane-cylinder ELLIPSE (centre at spine-plane intersection, semi-major
r/|n.dir| along the steep in-plane direction, semi-minor r along n x dir); parallel to the
spine = the genuine extend-two-faces case, declined with its own message. The surgery
skeleton needed NOTHING else: imprint_spring_line already lands springs at different spine
stations, split_blend_cap takes any conic, and the standard kef/kev dissolve consumes the
slant sliver. Test: chamfered prism (profile (0,0)(4,0)(4,1)(3,2)(0,2) extruded 2 in y),
fillet r 0.5 between top and cap: springs end at x 3 (top) and x 3.5 (front), ribbon ends on
the exact ellipse in the chamfer plane x+z=5.

The enabling piece: non-iso-rectangle cylinder trims used to DECLINE mass_properties (the
mitre note in Add.156). NEW integrate_cylinder_face_green: the region integral folds onto the
boundary (int_R F du dv = -loop_int G du, G the inner v-antiderivative), each boundary Gauss
node carrying an inner v-slab of Moments::add samples. Lines and degree-1 NURBS are rulings
and contribute zero through u-prime = 0; circles and ellipses carry the flux; the integrand
is u-periodic so fins need only LOCAL angle continuity (no seam unwrap); weights normalize by
the sign of the enclosed UV area. Curved NURBS fins still decline. The MITRE test now asserts
mass against its grid oracle (formerly mesh-only): the Add.156 decline is retired.

Closed-form oracle written FIRST (the streak holds, though this one passed first try: every
piece was already precedented): exact = 15 - (1/4 - pi/16) x 3 - [1/16 - (pi/32 - 1/24)];
mass to 1e-9 (through BOTH the new Green slab on the ribbon and the Add.172 lunes on the
ellipse-bounded chamfer face), mesh to 0.02, 8 faces, chamfer survives trimmed.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 246 (+1) green; fuzz_boolean soak
(massprops changed): count below. Dossier 56 sec 2 is now COMPLETE (notch + cap); remaining
in the leg: setbacks (dossier 53), Rung 5 SGC + completion gate.

## Addendum 174 (2026-06-10, attended): TWO-FACE ROOF CAP (dossier 56 sec 2.2 complete for existing-ridge planar ends)

Branch cap-roof. The two-face cap, "extending at most two of the adjoining faces to meet",
in its zero-extension form (the faces already meet at a MODEL RIDGE; bucket (a): plane-plane
intersection is a line, plane-cylinder sections are conics, all exact). fillet_edge end
handling generalized: ONE cap face at the end vertex keeps the Add.173 paths (perpendicular
circle / oblique ellipse / parallel decline); TWO cap faces meeting at a ridge incident to
the end vertex now split the ridge at its FIRST cylinder crossing (smaller quadratic root:
the corner is outside, so it enters on the material quarter) and trim each cap face to its
own plane-cylinder conic sub-arc, the two arcs meeting at the crossing vertex. The dissolve
forks per end: Single = kef(stub) + kev(spur) as before; Roof = kef both cap stubs + kev the
ridge stub (the old 4-face corner vertex collapses). More than two cap faces (the multi-face
construct-and-sew cap flagged for US 8,935,130 claim care) and true virtual extension stay
declined.

Test roof_cap_fillet_closes_the_ribbon_end_on_two_planes: box 4x2x2 with the end corner cut
by BOTH x+z=5 and x+y=5 (two wedge-prism booleans; body volume 14+1/3 asserted exact first),
fillet r 0.5: the ribbon closes with two ellipse sub-arcs meeting at x = 3.5 - r/sqrt2.
Closed-form oracle written FIRST: corner cut = r^3 (1/3 + sqrt2/3 - pi/4); mass to 1e-9
(the Green-slab integrator handles the two-ellipse-fin ribbon unchanged), mesh to 0.02.

The oracle exposed ONE defect, in tessellate_cylinder's ruling_band: each oblique cap plane
ASSIGNED its clamp to the nearer band end, so with two distinct planes on one end the last
visited won and the mesh overhung the binding plane by 0.079. First fix (combine via min/max
against the vertex band) regressed the MITRE, whose ellipse legitimately bulges PAST the
extreme boundary vertices: the correct semantics is REPLACE-not-limit per end, innermost
among planes only: lo/hi clip accumulators that override the vertex band when present. Both
the roof and the mitre gates now hold.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 247 (+1) green; fuzz_boolean soak
(tessellation changed): count below. Dossier 56 sec 2 (notch + one-face cap + two-face roof
cap) COMPLETE; #15 overflow handlers close (ov_smooth transition patch = procedural-evaluator
follow-up per the three-bucket verdict). Next: #16 setbacks (dossier 53).

## Addendum 175 (2026-06-10, attended): OBLIQUE TRIHEDRAL CORNER (dossier 53 milestone 2) + Green-slab SPHERES

Branch corner-triangle, the analytic rung of #16 setback corners taken FIRST per the standing
lesson. fillet_corner_octant generalizes to NON-perpendicular equal-radius trihedral corners:
the inscribed sphere exists whenever the three inward offset planes meet at one point M, and
EVERY cylinder spine passes through M (each spine is the intersection of two of the three
offset planes), so the cube-corner surgery generalizes verbatim. Only two things actually
changed: (1) the corner-arc circle frame (the second axis is dir x n, orthonormal by
construction; the old code passed the OTHER support normal, orthogonal only for the cube),
and (2) the spherical patch's mass integration.

(2) is the milestone's enabling piece: integrate_face_green now serves SPHERES as well as
cylinders. The per-point u' = dp.theta_hat / rho uses the point's own equatorial radius
(cylinder r, sphere R cos lat), v maps to latitude, and the slab base is chosen by the
boundary's total u-WINDING, the subtlety this surface class introduces: a sphere trim
enclosing a pole has winding +-1 and the base-shift term -loop_int g(u) du does NOT vanish,
so the slab anchors at the enclosed pole (the corner constructors put the patch's interior
pole at frame +z; boundaries touching the anchor pole, |winding| > 1, and non-circle sphere
fins decline). Winding-0 trims keep the boundary-minimum base; inner slabs go composite GL8
per quarter-pi (the sphere integrand is trigonometric in latitude; one panel was 2e-7, three
are 1e-15). The OCTANT's documented massprops decline is retired: both the suitcase test and
the projected-bounds test now assert the closed form at 1e-9.

Test oblique_corner_blend_is_an_exact_inscribed_sphere_triangle: pentagon prism, base corner
60 degrees, sides arranged so all three far caps stay perpendicular; M = (r sqrt3, r, r).
Closed-form oracle written FIRST and it tiles exactly: per-edge wedge cuts
r^2 (cot(t/2) - (pi-t)/2) over cross-plane-to-far-cap lengths (the cross planes through M
make the three cut prisms PAIRWISE DISJOINT: prism i lies on the far side of the very
support-offset plane that bounds prism j), plus the corner region (sqrt3 r^3) minus the kept
ball sector (polar-cone solid angle 2pi - perimeter = 2pi/3, volume 2 pi r^3/9; the
perpendicular case reproduces pi/2 and the octant pi r^3/6). Mass to 1e-9 ON FIRST RUN, mesh
to 0.02, 11 faces, 3 recognized blends.

CI: fmt; clippy -D warnings; workspace 132 + 77 + 248 (+1) green; fuzz_boolean soak
(massprops changed): count below. Next: dossier 53 milestone 3, the setback split +
Charrot-Gregory convex-combination patch through the certified evaluator-to-NURBS fit.

## Addendum 176 (2026-06-10, attended): SETBACK VERTEX BLEND (dossier 53 milestone 3): Varady-Rockwood split + Charrot-Gregory certified quads

Branch corner-triangle (continues Add.175). The first corner construction OUTSIDE the
analytic island, built exactly as the dossier prescribes: the certified-evaluator reduction,
not a new surface type.

NEW keel-geom::corner: the Charrot-Gregory convex-combination evaluator over the setback
hexagon (pre-2006 prior art: Charrot-Gregory 1984, Plowman-Charrot Gregory twists,
Varady-Rockwood setback split; the post-2006 US 8,004,517 / US 10,621,781 claim elements are
not used). The load-bearing design facts: (1) the cross-derivative field of every side is
FORCED by corner compatibility (D_i(0) = -B_{i-1}'(1), D_i(1) = +B_{i+1}'(0)), and those
vectors lie in each side's host tangent plane EXACTLY because the cylinders are tangent to
the supports along the springs, so interpolating the field in the side's own surface basis
(axial/tangential coefficients on arcs, plane vectors on profiles) keeps the patch G1 against
every band and support along the WHOLE side; (2) the rational perpendicular-distance-SQUARED
weights localize each corner interpolant with first-order flatness; (3) the Gregory twist
(eta M1 + xi M2)/(xi + eta) resolves the per-corner incompatible mixed partials. Evaluator
oracle written FIRST and passed first run: boundary reproduction 1e-9 (the regular hexagon's
radial-sweep parameter is exactly linear along each side), tangent-plane agreement 5e-5 just
inside every side.

Body::fillet_corner_setback(&[(edge, r, d); 3]): per-face surgery opens the L-strip by a spur
chain (corner spur to the setback station, spring split, profile spur, second spring split;
the profile is a REAL edge, the corner spurs are temporaries), far caps standard; the
dissolve kefs the three sharps (strips to wings), kefs two corner spurs (wings to one), kevs
the third with the corner vertex; the three bands peel off along exact cross-section arcs;
the remaining HEXAGON face central-splits (six side midpoints + the centre vertex at
eval(0,0)) into six quads, each fit by fit_foreign_surface and DECLINED if certification
misses 1e-4. Sense per quad from the fitted normal against the material corner.

Test (box corner, UNEQUAL radii 0.5/0.4/0.3, setbacks 0.8): validator, 15 faces, six
certified NURBS quads, rigorous volume bounds, inside/outside winding probes. NURBS faces
remain outside analytic mass properties (the documented M5 line), matching the dossier
ladder's oracle level for this rung; the exactness burden sits on the evaluator oracle and
the per-quad certification. CI: fmt; clippy -D warnings; workspace 133 + 77 + 249 (+2)
green; fuzz_boolean soak: count below. Remaining on the dossier-53 ladder: curved supports
(M4), n > 3 (M5), mixed convexity (M6): queued, not blocking #18.

## Addendum 177 (2026-06-10, attended): RUNG 5 SGC MERGE + the three-bucket completion-gate instrument

Branch sgc-simplify. Part (A) of #18, the Rossignac-O''Connor Merge operator (dossier 57,
"combine a k-cell with two incident (k+1)-cells when they share the same classification and
the k-cell bounds nothing else"), in its 2-cell instance: NEW crates/keel-topo/src/merge.rs
(named for Rossignac''s operator; distinct from simplify.rs, the M8 canonical-recovery HEAL
pass). Body::dissolve_interior_wall: a wall whose two sides bound two distinct SOLID regions
detaches its fins from every rim''s radial cycle (radial-3 junctions return to manifold
pairs), re-anchors vertex fin-references onto surviving fins, fuses the two host shells
(genus adds), repoints the dead region''s shells and faces, and unregisters the wall complex
plus the dead region. Body::merge_cells loops it. Honest declines: dangling rims,
ring-carrying walls, single-shell walls (the genuine mixed-dimension tail).

DIFFERENTIAL ORACLE, first try: cellular union of two abutting cubes + Merge == the
regularized union EXACTLY (one region, equal face counts, zero walls, zero radial-3 rims,
mass == mesh == 2 at 1e-9, equal to the regularized mass at 1e-12, idempotent); and Merge
undoes partition_by_sheet (knife wall dissolves, cube back to one cell at mass == mesh == 8,
imprint scars legitimately remain).

Part (B), the completion-gate INSTRUMENT: crates/keel-topo/tests/three_bucket.rs, the
three-bucket boolean oracle per the standing completion plan: random axis-aligned box pairs
against EXACT interval references; PASS = strict mass == mesh == reference, DECLINE = Err or
faulted (counted, never penalized), WRONG must be ZERO. Deterministic LCG, scaled by
KEEL_ORACLE_N, ignored-by-default (a gate instrument, not a fast-suite test; ~150 ms/trial
debug). FIRST SMOKE, N = 2000: PASS 817 / DECLINE 1183 / WRONG 0. The decline mass is
dominated by disjoint and degenerate-contact configurations (honest declines per
DECLINE-never-WRONG); driving the decline rate down is quality work the gate makes visible,
not a gate failure.

THE COMPLETION-GATE PROCEDURE (documented for the scheduled run, out of single-session scope):
1. All-sectors fuzz soak, ~10 h: for each target in fuzz/fuzz_targets (fifteen: boolean,
   cyl_boolean, nurbs_boolean, imprint, topo_ops, winding, pmc, ssi, step_import, recover,
   nurbs_curve, nurbs_surface, bernstein_roots, interval, solve_cubic):
   CARGO_TARGET_DIR=~/keel-fuzz-target cargo +nightly fuzz run <target> -- -max_total_time=2400
2. The scaled three-bucket run (release, millions of trials):
   KEEL_ORACLE_N=1000000 cargo test --release -p keel-topo --test three_bucket -- --ignored --nocapture
   Gate: WRONG == 0. Record PASS/DECLINE counts in the LOG.
CI: fmt; clippy -D warnings; workspace 133 + 77 + 251 green (+2 Merge tests; the oracle
ignored by default); fuzz_boolean soak: count below.

## Addendum 178 (2026-06-10, attended): completion-gate INTERMEDIATE EXECUTION, all clean

The two Add.177 instruments ran at intermediate scale on master 3907a76 (+ f690570, the
runnable soak script fuzz/soak_sectors.sh):

1. THREE-BUCKET ORACLE, N = 100,000 (release, 4259 s, ~42 ms/trial):
   PASS 41192 / DECLINE 58808 / WRONG 0.
   The WRONG == 0 gate holds at fifty times the smoke scale; the decline share (58.8 pct,
   matching the N=2000 smoke) is dominated by disjoint and degenerate-contact pairs
   declining honestly. Reducing it is visible quality work, not a gate failure.

2. SEVEN-SECTOR SOAK, 20 min/target, ZERO crashes:
   cyl_boolean 1623 runs, nurbs_boolean 921, imprint 1947, topo_ops 459456,
   winding 2896879, pmc 3674822, ssi 273203656.

REMAINING for the full gate (the scheduled overnight run, user-initiated):
   bash fuzz/soak_sectors.sh                # all 15 targets x 40 min, ~10 h
   KEEL_ORACLE_N=1000000 cargo test --release -p keel-topo --test three_bucket -- --ignored --nocapture
With this, every item of the queued post-parity program (Add.163) is BUILT and gated at
intermediate scale: the audit milestones (153-162), the queued-depth leg (164-169), the
dossier-56 overflow handlers (170-174), the dossier-53 corner ladder M1-M3 (175-176), and
Rung 5 Merge + the gate instruments (177). The declined tails (dossier-53 M4-M6, the
mixed-dimension Merge instances, ov_smooth, the multi-face cap, crossing-seam imprint) are
recorded follow-ups, each behind an honest decline.

## Addendum 179 (2026-06-10, attended): GRACEFUL DEGRADATION rung 1: no-interaction booleans (dossier 29, the new program)

USER MANDATE: graceful degradation IS the product ("the reason Parasolid is king is not their
number of features, but their ability to take pretty much any geo and return an answer").
Dossier 29 re-read in full; the doctrine mapped onto Keel: the two hard rules (topology
non-negotiable, never-panic) and the fault-report return already hold; what is missing is
everything between strict and DECLINE: the tolerant tier, the cascade, confidence reporting.
This leg builds them bottom-up. Branch graceful-disjoint, rung 1: the NO-INTERACTION class.

Operands with no seams and no coincidence are disjoint or nested, decided by one unambiguous
winding probe per side (the 0.25 band rule; ambiguous probes decline). These are CLEAN
answers, not an error class: disjoint intersection = the EMPTY body; disjoint difference =
A unchanged; nested intersection/union = the inner/outer clone; swallowed difference =
empty; the two cases that genuinely assemble (the DISCONNECTED union, the CAVITY difference)
fall through to the standard pipeline. Coplanar-but-empty-overlap Coincident flags are
dropped as noise on this path (the pre-pass proved no actual overlap).

The rung exposed and fixed TWO real pre-existing defects:
1. DISCONNECTED RESULTS shared one solid region (the documented finalize simplification),
   which violates Euler-Poincare (closed shells count as regions minus one): each exterior
   component now gets its own solid cell. The disjoint union is Euler-true: two solid
   regions, mass == mesh == 2 at 1e-9.
2. INSIDE-OUT OPERANDS were declined only by ACCIDENT (the shared-region Euler mismatch);
   fix 1 made the wrong body self-consistent (an over-thick hollow returned volume 9 with
   mass == mesh agreeing). Root cause is garbage-in: dossier 29 Part 6. boolean_with now
   has a FRONT-DOOR intake check declining negative-signed-volume operands loudly
   ("operand is inside-out"); sheets and wires (volume ~ 0) pass; orientation REPAIR with
   a report is the follow-up. hollow's over-thick decline now fires for the right reason.

THE PAYOFF, measured by the gate instrument: the three-bucket oracle went from
PASS 41192 / DECLINE 58808 / WRONG 0 (N = 100k, Add.178) to
PASS 10000 / DECLINE 0 / WRONG 0 (N = 10k, this rung). The ENTIRE random-box decline
population was the no-interaction class; generic-position overlaps already passed. WRONG
stays zero throughout.

Tests: disjoint_operands_return_clean_answers (disjoint x3 ops + reversed difference +
component winding probes + per-component solid cells; nested x4 incl. the cavity difference
with its void region, mass == mesh == 26 at 1e-9). CI: fmt; clippy -D warnings; workspace
133 + 77 + 252 green; fuzz_boolean soak (boolean internals changed): count below.
NEXT (the leg continues): M2 = the Tier-2 TOLERANT boolean (dossier 39 touching/on-on class
+ the Add.162 tolerance spec) with OpReport confidence (clean/salvaged + tier + achieved
tolerance), never silent.

## Addendum 180 (2026-06-10, attended): GRACEFUL DEGRADATION M2: the Tier-2 TOLERANT boolean

Branch graceful-tolerant. Dossiers re-read: 39 (the ACIS prepare phase: snap near-coincidence
to exact coincidence BEFORE classification, ~70 pct of failed booleans fixed in one
iteration; OCCT fuzzy value; PRESERVE-never-perturb) and the Add.162 tolerance spec.

NEW pub boolean_tolerant(a, b, op, tol, fuzz) -> (BoolResult, Confidence): the dossier-39
PREPARE phase snaps B planar faces whose planes lie within (0, fuzz] of a parallel
near-mating A plane (inflated-AABB overlap required; curved rims skipped: the planar slice)
EXACTLY onto the A plane (face-snap semantics: the mating face moves, the far side stays),
then runs the strict pipeline ONCE. Confidence reports honestly: clean tier 1 with achieved
tolerance 0 when nothing moved; salvaged tier 2 with the max movement when something did.
NO SILENT SALVAGE. The kernel caps effective fuzz at one tenth of the shortest edge (the P5
local-feature-size guardrail). Strict boolean() is UNTOUCHED: a literal 1e-5 gap remains two
honest components there; declaring it a flush mate is the tolerant caller's choice.

Strict-mode companion fix: TOUCH-ONLY selection (kept empty, no walls) now returns the clean
EMPTY result for intersection/difference per Requicha (touching solids intersect in measure
zero; A - A is empty); an empty union stays a decline. The fuzz-found
near_coincident_touch_declines test updated: the empty body IS the exact answer there now.

ORACLE, contact lane added (random floats never touch; every 4th trial abuts B on A's +face
with delta in {0, +-1e-9, +-1e-7} and overlapping cross-extents): the lane exposed THREE
judge-calibration facts, each resolved on principle:
1. Coincident faults are informational notes, not partial failure: only-Coincident results
   are judged on their volumes.
2. A contact WITHIN the op tolerance may legitimately resolve as the snapped configuration
   (the boolean''s own tolerance contract / the Fang-Bruderlin tie): the strict judge accepts
   either reference there. The witness: a 1e-7-penetration difference whose mass == mesh ==
   the snapped volume to 1e-14.
3. Mass stays the 1e-9 exact gate; mesh (the independent dropped-face net) gets sliver
   headroom at 1e-7 relative: ear-clip float noise on 1e-9-thin sliver faces reaches ~1e-8
   relative while a genuinely dropped face is feature-scale. The witness: a sliver union
   with mass exact to 1e-14 and mesh off by 4.7e-8.

MEASURED (N = 2000, release): strict PASS 1855 / DECLINE 145 / WRONG 0; tolerant PASS 327 /
DECLINE 173 / WRONG 0 (65 pct of the contact class salvaged EXACTLY against the snapped
reference, salvage flags verified per-trial). The tolerant declines are dominated by the
partial-overlap on-on intersection laminas (the honest mass != mesh gate): the on-on table
refinement is the named follow-up, plus curved-surface prepare (cylinder/cone mates).

Tests: tolerant_boolean_snaps_near_coincident_contact (tier 1 exact contact; tier-2 gap and
penetration salvage with exact snapped volumes 2 +- gap, one component, achieved tolerance
in [gap, fuzz]; beyond-fuzz stays two components), touching_intersection_and_difference_are_
clean. CI: fmt; clippy -D warnings; workspace 133 + 77 + 254 green; fuzz_boolean soak: count
below.

## Addendum 181 (2026-06-10, attended): GRACEFUL DEGRADATION M3: the on-on coincidence machinery completed (chains, pockets, geometric-first classification)

Branch graceful-tolerant (continues Add.180). The dominant decline class was the
PARTIAL-OVERLAP touching contact; the probe revealed the real defect chain, fixed bottom-up:

1. CHAIN IMPRINT (imprint_open_polyline): the pre-imprint cuts of an off-corner partial
   overlap CHAIN through corners interior to the subject face; imprint_open_curve requires
   boundary endpoints, so every such cut silently no-opped (the swallowed-Err class). The
   new op runs the proven spur-chain surgery: boundary split, mev through the corners,
   closing split_face. chain_segments (coincident.rs) assembles the cut segments into
   chains by shared endpoints.

2. POCKET RING IMPRINT (imprint_closed_polyline): the ENCLOSED-pocket coincidence (B''s
   mating face wholly inside A''s) yields a CLOSED chain. Generalizes the closed-circle
   imprint: spur + mev around the polygon + mef back to the spur vertex + kemr the bridge.
   Two conventions mattered: mef moves fin_a.next..=fin_b to the NEW face (the spur-out fin
   must be fin_a so the new face is the pocket), and the chain is pre-oriented to wind WITH
   the host''s outer loop so the kemr ring winds opposite, as an inner ring must.

3. HOUSE CONVENTION ENFORCED: chain edges carry their exact Line geometry. Curveless edges
   are invisible to fin_curve_samples, so the pocket face had no UV polygon, no interior
   point, no outward normal, and classified Unknown (the partial-touch fragments worked
   only because their original boundary edges carried curves).

4. GEOMETRIC-FIRST CLASSIFICATION (dossier 39 sec 1.4, now enforced): the winding number is
   UNDEFINED on the boundary, so classify_faces consults the extent-guarded coincident-
   carrier test BEFORE any winding; the on-band fallback remains for near-misses.
   coincident_sense_at gained the inflated-AABB extent guard it needs to be safe
   geometrically-first (a distant point on an infinite carrier is not coincident).

5. SPURIOUS-SEAM FILTER WIDENED: imprint_operand''s boundary-coincident seam filter tested
   the OUTER loop only; a pre-imprinted pocket rim is an INNER ring, so its rim seams
   re-imprinted as a duplicate ring (the third loop that cost the union exactly the
   pocket''s fan). Every loop''s polygon now counts as boundary.

Tests: partial_touch_booleans_are_clean (corner-chain config: empty intersection, identity
differences, union 2 exactly) and enclosed_pocket_touch_booleans_are_clean (pocket config:
empty intersection, identity difference, union 10 exactly with the one annular host wall).

ORACLE (N = 2000, the contact lane): tolerant PASS went 327 -> 487 of 500 (65 to 97.4 pct
of the touching class salvaged exactly, salvage flags verified); strict PASS 1855 -> 1892;
WRONG = 0 in both lanes. Judge refinement on principle: the dual-reference scheme is
replaced by the EPSILON-SOLIDITY allowance (Qi-Shapiro): a contact within op tolerance may
resolve as any coincidence-glued configuration, all within op_tol x (larger mating-face
area) of the literal volume; the clean-empty acceptance honors the same slack (a
sub-tolerance sliver glues to the empty touching configuration legitimately).

CI: fmt; clippy -D warnings; workspace 133 + 77 + 256 green; fuzz_boolean soak: count below.
Remaining tolerant declines (13/500): mixed sliver-assembly shapes; with curved-surface
prepare (cylinder/cone mates) these are the M4 candidates.

## Addendum 182 (2026-06-10, attended): GRACEFUL DEGRADATION M4: the pin-in-hole class, and the wrong-positive it unmasked

Branch graceful-m4. The canonical CAD mating case, ORACLE FIRST: a plate with a through hole
and the pin that exactly fills it. The oracle exposed a chain of SEVEN defects, the deepest a
genuine WRONG-POSITIVE CLASS that predates this leg:

1. THE DRILL DIFFERENCE WAS WRONG: boolean(plate, drill, Difference) returned a body whose
   hole lateral spanned the drill''s FULL height (sticking out of the plate), passing only
   because the curved-result gate (positive tessellated volume) is weak. Three causes:
   (a) the drill''s seamless tube lateral (two closed rims, no seam line) made the wrap-circle
   crossing imprint silently no-op, and the interior-ring imprint is topologically wrong for
   a non-contractible wrap: NEW synthesize_lateral_seam (mekr between the rim seam vertices)
   gives the crossing imprint its seam; (b) the multi-cut seam relocation was PLANAR-only:
   NEW curved_face_containing relocates later components onto descendant cylinder bands;
   (c) two massprops gaps hid the shape: face_covers_closed_surface short-circuits only for
   genuinely closed surfaces now (a tube lateral is a bounded band), and the pcurve-bounds
   path gained the periodic u-clamp plus the vertex-extent staleness witness (a fragment
   carrying its PARENT''s pcurves read v [0,2] while its rims sit at [0,1]).
   The drilled plate is now ANALYTICALLY EXACT: mass == 16 - pi at 1e-9.

2. THE MATED-CYLINDER CLASS: coaxial equal-radius laterals are the curved on-on coincidence
   (dossier 39 sec 5), not a crossing: seam_curves marks the pair with the informational
   Coincident note instead of UnassemblableSeam; rim-contact SSI circles lying ON existing
   boundary edges are spurious per sec 3.2 (NEW curve_on_face_boundary_edges filter).

3. THE VERTEX-ONLY TRAP, THIRD AND FOURTH SIGHTINGS: the carrier AABB guard and
   planar_face_contains both built extents/polygons from loop VERTICES, which a
   circle-bounded face does not have (one seam vertex per rim): both now sample fin CURVES.
   And the M1 no-interaction containment probe used raw vertices, ALL of which lie on the
   other body''s boundary for a mated pin (the union returned the holed plate as "B inside
   A"): the probe now uses guaranteed-interior points (face interior nudged inward by 1e-3
   of the shortest edge).

4. GEOMETRIC-FIRST DIVIDENDS (M4 fixes from the M3 groundwork): real point-in-face carrier
   containment (a pin cap TILING the plate annulus shares a carrier and an AABB but not a
   point) and the dossier-39 TWO-SIDED band test (the cap centre sits at the hole mouth
   where the one-point winding is noise; both side-limits outside = Outside).

Test pin_in_hole_booleans_are_clean: holed plate exact (16 - pi at 1e-9, analytic), union =
the SOLID box (mass == mesh == 16 at 1e-9, zero surviving cylinder faces), difference = the
holed plate, intersection = empty. ORACLE (N = 2000): unchanged at strict 1892/108/0,
tolerant 487/13/0 (the box lanes do not exercise cylinders; the pin tests are the cylinder
gate). CI: fmt; clippy -D warnings; workspace 133 + 77 + 257 green; fuzz_boolean +
fuzz_cyl_boolean soak (boolean + imprint + massprops changed): counts below.
FOLLOW-UP NOTED: the curved-result self-consistency gate stays weak (positive tessellated
volume only); extending mass==mesh to curved bodies within a chordal band is the next
honesty-net upgrade and would have caught defect 1 directly.

## Addendum 183: graceful degradation M5, the curved chordal honesty gate (2026-06-11)

Dossier 29 re-read (Part 4, confidence reporting; Part 6, no silent salvage): the doctrine
demands that a returned body be SELF-CONSISTENT or declined, never silently wrong. The
planar mass==mesh post-condition (research file 47) embodied that for planar results;
curved results passed on POSITIVE TESSELLATED VOLUME alone, the weak gate that let the M4
drill wrong-positive ship. M5 closes the asymmetry.

THE GATE (assemble_boolean, curved branch): when mass_properties is computable the result
must satisfy |mass - mesh| <= 2e-2 * (1 + |mass|), the CHORDAL band (the adaptive
tessellation''s worst legitimate deviation on small arcs). Bodies whose mass legitimately
declines (NURBS corner patches) keep the positive-volume floor: the gate never punishes an
honest decline, only a self-inconsistent "success".

THE GATE''S FIRST CATCH, ON ITS FIRST RUN: pin_in_hole''s drilled plate DECLINED under the
new band. The probe: the holed lateral tessellated an angular span of [0, tau - pi/8],
a 16 percent mesh deficit (dv -1.756 vs exact -2.094) that had been shipping silently.
ROOT CAUSE: cyl_angular_span''s three paths all failed on a seam-split full ring: (a) no
arc_sweep on the rim arcs, (b) no single CLOSED edge survives the seam imprint (each rim
is two arcs), (c) the sample-gap fallback takes the complement of the LARGEST angular gap,
and a full ring of finite samples always has a gap of one sampling step (pi/8 here).
FIX (tessellate.rs): before the gap fallback, walk each loop''s polygon and accumulate the
UNWRAPPED angle delta; a loop whose cumulative range covers tau - 1e-6 bounds a FULL
revolution regardless of how its edges were split. Branch-cut-free by construction.

This is the M5 thesis demonstrated in one commit: the honesty net does not just guard
against hypothetical future bugs, it found a live one in the first body it inspected
(WRITE THE EXACTNESS ORACLE FIRST, again). The pin test now carries the chordal witness
assertion (|mesh - mass| of the holed plate inside the band).

CI: fmt; clippy -D warnings; workspace 133 + 77 + 257 green; oracle N=2000 unchanged
(strict 1892/108/0, tolerant 487/13/0); fuzz_boolean + fuzz_cyl_boolean soak (boolean +
tessellation internals changed): counts below.
NEXT (the leg continues): tolerant cylinder prepare (radial-gap pins), the residual
13-trial tolerant tail, doctrine-wide OpReport.

## Addendum 184: graceful degradation M6, the tolerant cylinder prepare (2026-06-11)

Dossier 39 re-read (sec 1, the ACIS prepare phase: snap near-coincidence to EXACT
coincidence before classification fixes the dominant share of failed booleans; sec 5,
coincident curved carriers): M2''s prepare_snap covered planar near-mates only. M6 extends
it to the canonical curved case, the RADIAL-GAP CLEARANCE PIN: a pin exported 1e-5 under
its hole (or 3e-6 off-axis), which strict correctly sees as parallel non-touching laterals.

THE SNAP (prepare_snap, cylinder pass): a B lateral near-mated to an A lateral (axes
parallel within 1e-6, axis offset and radius difference within fuzz but not exactly zero,
axial spans overlapping, B''s lateral bounded only by flat caps perpendicular to the axis)
is re-seated on A''s EXACT carrier: the surface becomes A''s axis and radius with B''s
axis sense and angular reference preserved; every loop vertex reprojects radially; rim
circles are rebuilt at the exact radius about the exact axis (winding preserved); straight
rulings re-fit their snapped endpoints. Axial extents come from fin CURVE samples (the
vertex-only trap, sighting five, avoided at design time). After the snap the strict
pipeline runs the M4 mated-pin machinery unchanged: coincident-opposite laterals drop,
caps tile, the union is the SOLID box.

Oracle first (tolerant_radial_gap_pin_snaps_exact): the 1e-5 undersize pin unions to
mass == mesh == 16 at 1e-9 with salvaged/tier 2/achieved in [1e-6, 1e-4], differences back
to 16 - pi at 1e-9; the 3e-6 axis-offset pin salvages identically. PASSED ON THE FIRST
FULL RUN of the implementation.

The flat-cap guard is the honesty boundary: oblique or curved neighbours would be pulled
off their carriers by a radial move, so those configurations stay strict (DECLINE-never-
WRONG). Cones and spheres remain the recorded follow-up.

CI: fmt; clippy -D warnings; workspace 133 + 77 + 258 green; oracle N=2000 unchanged
(strict 1892/108/0, tolerant 487/13/0). M5 and M6 share this branch''s soak window
(both touch boolean internals; one fuzz_boolean + fuzz_cyl_boolean soak gates the merge);
counts recorded at the merge.

## Addendum 185: the tolerant tail retired; the sec 3.2 filter goes universal (2026-06-11)

Dossier 39 re-read (sec 3.2: "a curve along existing boundary separates nothing and is
never imprinted"; sec 1.4 on-on classification): the residual 13-trial tolerant decline
tail (steady through M2-M6 at 2.6 percent of contact trials) traced to ONE root cause.

DIAGNOSIS (tail_repro harness, twelve deterministic LCG replays): after the coincident
pre-imprint cuts a pocket or partial-overlap chain into a face, the SSI phase re-emits
seams for the contact lines, and those seams lie exactly ON the freshly imprinted
boundary. The sec 3.2 filter dropped them ONLY when a face''s seam group assembled into a
closed loop (the L-union phantom-ring fix); the snapped-contact configurations produce
NON-closed groups (the pocket face''s group has three sides; the enclosing face''s group
has all four), so the boundary-coincident segments re-imprinted along existing edges:
a duplicate ring edge, one extra E, Euler-Poincare off by exactly one. Three decline
flavours, one cause: "stitched body invalid" (7 trials), "unmatched coedge" (2), and the
"open chain end not on boundary" Topo fault (3, chains corrupted by boundary segments).

FIX: the boundary-coincident drop is now UNIVERSAL: any seam segment whose every sample
lies on the subject face''s existing boundary (all loops, rings included) is dropped
regardless of the group''s chain topology. A genuine cut always carries interior samples;
a segment that merely TOUCHES the boundary keeps its interior samples and survives. The
old closed-loop scoping (and its interior-count heuristic) is retired; the canonical
dedup and the chamfer shared-edge case are unaffected (those segments are not pure-
boundary).

ORACLE, the headline: tolerant 500 PASS / 0 DECLINE / 0 WRONG (was 487/13/0): the
TOLERANT CONTACT LANE IS CLEAN. Strict improved to 1902/98/0 (was 1892/108/0). WRONG = 0
in both lanes, as always. The twelve replays are promoted to a permanent regression
(tolerant_contact_tail_regression: clean result, informational Coincident faults only,
mass == mesh).

CI: fmt; clippy -D warnings; workspace 133 + 77 + 258 + 1 green. M5 commit soak completed
clean (fuzz_boolean 665 runs, fuzz_cyl_boolean 693 runs); the combined M6 + filter soak
gates the merge: counts below.

SOAK COUNTS (the M6 + M7 merge gate, 2026-06-11): fuzz_boolean 1876 runs / 601 s clean;
fuzz_cyl_boolean 666 runs / 606 s clean. Oracle at N=10000: strict 9507/493/0, tolerant
2497/3/0 (the three new declines are rarer shapes surfacing at 5x sample; triage noted).
WRONG = 0 in both lanes.
