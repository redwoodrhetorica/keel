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
