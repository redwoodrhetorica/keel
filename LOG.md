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
