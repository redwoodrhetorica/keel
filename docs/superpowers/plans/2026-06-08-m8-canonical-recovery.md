# M8: Canonical Geometry Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Recognize the analytic geometry hidden inside NURBS surfaces and curves, and replace it with the exact primitive plus a *certified* max-deviation bound. This is Parasolid/ACIS "HEAL simplify" / OCCT `ShapeAnalysis_CanonicalRecognition`, and it is the missing piece that makes the affordability gradient real: exact for recoverable conics/quadrics, certified-tolerant only for genuine freeform. Its first internal client is the boolean front-end, where it dissolves the M7c blockers (NURBS-sphere x NURBS-sphere was grinding the 190s tier-3 spline solver to re-derive an exact circle).

**Architecture:** A new `recover` module in `keel-geom` implements the recognize -> fit -> certify cascade on a `NurbsSurface` / `NurbsCurve`, returning `Option<Recovered{ surface|curve, deviation_bound }>` (None = keep the spline). A `simplify` pass in `keel-topo` performs the topology-preserving substitution (surface swap, pcurve refit, seam/merge, topology-coordinated). The boolean front-end runs recovery on operand faces before SSI dispatch so analytic-vs-analytic fast paths fire. Recognition uses CLOSED-FORM differential invariants from the spline (k1, k2, normals via existing `local_geometry`), certification uses the Bezier convex-hull bound (existing `to_bezier_patches` / `point_enclosure`) against the analytic's closed-form distance. No new analytic types are needed: `Surface3::{Plane,Cylinder,Cone,Sphere,Torus}` and `Curve3::{Line,Circle,Ellipse}` all exist.

**Research basis (re-read 2026-06-08):** file 24 (canonical-geometry-recovery) -- the recognize/fit/certify/substitute pipeline, the curvature-signature classifier (Benko-Varady), the Gauss-map + linear-line-complex axis recognizer (Pottmann/Randrup), the faithful least-squares fitter (Lukacs-Marshall-Martin), the certified max-deviation gate (Kim/Elber; OCCT max-deviation criterion), topology-coordinated substitution. file 11 / headline #16 (the exactness affordability gradient: spheres/quadrics are exactness-affordable; using tier-3 spline machinery on a disguised quadric is the wrong tier). file 23 (the single orthogonal-distance fitting core). file 25 (Parasolid capability audit lists simplify/canonical conversion).

**Carried lessons:** Task 0 soundness-gate any new winding/classification input; the certified deviation bound is MANDATORY, never advisory (file 24: "recovery must never silently exceed tolerance"); recover-or-KEEP is an explicit decision (the "keep the spline" verdict is itself the certificate that a surface is genuinely tolerant); run the EXACT CI triplet before merge; fuzz findings -> golden regressions; honest deferral ledger; decline-don't-lie.

**Branch:** `m8-canonical-recovery` off master.

**STATUS (2026-06-08):** COMPLETE except the free-form-solid capstone.
Core slice shipped in Addendum 34 (Tasks 1-3 plane/sphere/cylinder/cone +
certifier; Task 7 boolean integration retiring the M7c blocker, NURBS x
NURBS 190s -> 0.68s; Task 8 rejection gate; Task 9 fuzz_recover). M8
completion in Addendum 35: Task 4 (torus -- full quadric family now
recovers), Task 5 (curve recovery line/circle), Task 6 (`Body::simplify`
public HEAL pass, orientation-preserving). STILL DEFERRED: ellipse curve
recovery; the full free-form-faced-SOLID boolean capstone (needs a
free-form solid constructor that does not exist yet -- a separate
milestone); face-merge; constraint-aware re-fit; interval-certified
tighter bound.

---

## Part A: the recovery cascade (keel-geom `recover` module)

### Task 1: module scaffold + plane recovery + the certified-deviation gate
**Files:** Create `crates/keel-geom/src/recover.rs`; modify `crates/keel-geom/src/lib.rs` (add `pub mod recover`).

- `pub struct SurfaceRecovery { pub surface: Surface3, pub deviation: f64 }` -- the recovered analytic plus its CERTIFIED upper-bound deviation from the source spline.
- `pub fn recover_surface(nurbs: &NurbsSurface, tol: f64) -> Option<SurfaceRecovery>` -- the recognize/fit/certify cascade; returns `Some` only when the certified deviation bound `<= tol`, else `None` (keep the spline).
- `pub fn certified_surface_deviation(nurbs: &NurbsSurface, cand: &Surface3) -> f64` -- a GUARANTEED upper bound on max deviation: decompose with `to_bezier_patches`, and for each patch bound the distance from its control-point convex hull to `cand` using the analytic's closed-form signed distance (point-to-plane/sphere/cylinder/cone are closed-form); the per-patch bound is `max over control points of |dist(cp, cand)|` (convex-hull property: the patch lies within the hull, and signed distance to a plane is linear, to a sphere/cylinder is 1-Lipschitz, so the control-point max bounds the patch max). Return the max over patches.
- Plane recognition + fit: sample k1,k2 on a UV grid via `local_geometry`; if all `|k1|,|k2| < curv_eps` it is a plane candidate; fit by SVD/centroid+normal of sampled points (normal = average cross of grid tangents, refined). Build `Surface3::Plane`.
- **Test:** a degree-1 / planar NURBS patch recovers to `Plane` with `deviation < 1e-9`; a `nurbs_sphere` surface returns `None` for the plane path (and is not mis-accepted). A plane tilted/translated recovers the correct frame (normal and offset within 1e-9).

### Task 2: sphere recovery (the M7c-unblocking case)
**Files:** modify `recover.rs`.

- Recognition: curvature signature k1 = k2 = const != 0 (Benko-Varady); the common sign gives inside/outside (radius `r = 1/|k|`).
- Fit: algebraic sphere fit (solve for center/radius minimizing `||p-c||^2 - r^2` over sampled points, linear in (c, r^2-|c|^2)), one Gauss-Newton refine on true geometric distance for faithfulness (Lukacs-Marshall-Martin).
- Certify with `certified_surface_deviation` (point-to-sphere closed form), gate at tol.
- **Test:** `nurbs_sphere(frame, r)`'s surface recovers to `Surface3::Sphere` with center/radius within 1e-9 and certified deviation `< 1e-6`; a cylinder surface returns `None` from the sphere path. **Task 0 soundness:** the recovered analytic sphere's parameters match the constructed sphere exactly (this is the geometry booleans will classify on).

### Task 3: cylinder + cone recovery
**Files:** modify `recover.rs`.

- Recognition: one principal curvature ~0 (developable) via curvature signature + Gauss-map dimension (normals lie on a great circle => cylinder; non-great circle => cone). Axis via the zero-curvature principal direction (closed-form from `local_geometry`'s principal frame) averaged over samples; for robustness, the Pottmann linear-line-complex 6x6 eigenproblem on sampled normal lines is the principled recognizer (p=inf => cylinder, finite p with apex => cone) -- implement the eigenproblem; the recovered complex gives axis (and rejects freeform via large residual = recognition confidence).
- Fit: faithful LS for cylinder (axis + radius) and cone (axis + apex + half-angle).
- Certify + gate (point-to-cylinder / point-to-cone closed form).
- **Test:** an extruded/revolved NURBS cylinder recovers to `Cylinder3` (axis/radius within 1e-9); a NURBS cone recovers to `Cone3` (apex/half-angle within tol); sphere and plane do not mis-recover as cylinder/cone (cross-discrimination matrix).

### Task 4: torus recovery
**Files:** modify `recover.rs`.

- Recognition: both principal curvatures nonzero with the torus functional relation; axis via the line-complex (revolution, pitch 0); major radius from axis-to-tube-center, minor from tube curvature.
- Certify (point-to-torus distance via nearest tube circle) + gate.
- **Test:** a revolved NURBS torus recovers to `Torus3` (major/minor within tol). HONEST LEDGER: if torus certification proves fragile (file 24 flags torus/cone as the least robust), gate it tightly (only accept on a confident signature) and document the conservative threshold rather than over-accept.

### Task 5: curve recovery (line / circle / ellipse)
**Files:** modify `recover.rs`.

- `pub fn recover_curve(nurbs: &NurbsCurve, tol: f64) -> Option<CurveRecovery>`: line (zero curvature), circle (constant curvature + planar), ellipse (planar, conic) via sampled curvature + planarity; fit; certified deviation (point-to-line/circle/ellipse, closed or near-closed form) + gate.
- **Test:** a NURBS curve fitted to a circular arc recovers to `Circle3` within tol; a genuine wavy spline returns `None`.

## Part B: substitution + integration (keel-topo)

### Task 6: topology-preserving surface substitution -- `Body::simplify`
**Files:** Create `crates/keel-topo/src/simplify.rs`; modify `crates/keel-topo/src/lib.rs`.

- `pub fn simplify(&mut self, tol: f64) -> SimplifyReport`: for each NURBS face, run `recover_surface`; on success swap `SurfaceGeom::Nurbs -> Analytic`, refit the bounding edges' pcurves onto the new analytic (existing pcurve machinery), preserve sense/orientation. Topology-coordinated: when two faces recover to the SAME analytic (a sphere arriving as two cap faces sharing a seam), keep them consistent; full face-MERGE may defer (note in report). Recover edge curves to analytic with `recover_curve`. The report lists what was recovered and the max deviation carried.
- Re-validate after substitution (topology unchanged; geometry tightened).
- **Test:** `nurbs_sphere` body `.simplify(1e-6)` yields a body whose face surface is `Surface3::Sphere`, still valid, same mass-properties volume within the carried deviation; a body of a genuinely freeform face is unchanged (`simplify` is a no-op, report says kept).

### Task 7: boolean integration -- exact tier restored, M7c retired
**Files:** modify `crates/keel-topo/src/boolean.rs`.

- In `boolean()`, before SSI dispatch, run a lightweight recovery pass on the operands (or accept already-simplified bodies): when both relevant faces are (recovered) analytics, the existing `seam_curves` tier-1/tier-2 fast paths fire. NURBS-sphere x NURBS-sphere now recognizes two spheres -> exact `Circle3` SSI -> the M6c crossing-imprint (proven on clean circles) splits both -> exact lens, sub-second.
- **Test (retires the M7c deferral):** `nurbs_sphere INTERSECT nurbs_sphere` after recovery -> valid two-cap lens, volume within 1% of the exact formula, runs in << 1s. Update `nurbs_boolean_is_epsilon_solid`: with recovery, the NURBS-sphere x analytic-sphere case becomes EXACT (deviation ~0), so that test's `eps > 0` expectation moves to the freeform capstone (Task 8); here assert the lens is exact (achieved tolerance at the floor) and valid. Keep the analytic-vs-analytic sphere lens green.

### Task 8: the genuinely-tolerant capstone (exact topology + tolerant geometry on a TRUE spline)
**Files:** modify `boolean.rs` tests; maybe a freeform constructor in `construct.rs`.

- Build a genuine freeform surface that recovery correctly REJECTS (a bumped/perturbed NURBS surface, or a non-quadric lofted patch) -- `recover_surface` returns `None` with a certified deviation above tol, which is the certificate that this surface is genuinely tolerant.
- Boolean it against a sphere/plane so the SSI MUST use tier-3 (or tier-2 analytic-vs-genuine-spline), producing a fitted seam with genuinely nonzero `tol_achieved`. The tolerant-edge contract (M7b, already shipped) carries that bound onto `Edge.tolerance`; assert the result is epsilon-solid at the achieved (nonzero) tolerance, combinatorics exact.
- This is the principled home of the "hybrid nobody shipped" demonstration: recovery decides exact-vs-tolerant per surface, and HERE the verdict is honestly "tolerant".

### Task 9: fuzz + gate
**Files:** Create `fuzz/fuzz_targets/fuzz_recover.rs`; modify `fuzz/Cargo.toml`.

- `fuzz_recover`: random near-analytic NURBS surfaces (analytic + bounded perturbation) -> `recover_surface` never panics; INVARIANT (the soundness property): when it returns `Some`, the returned `deviation` is a true upper bound (spot-check: dense-sample distance <= reported deviation + slack) AND `deviation <= tol`; when it returns `None`, no false "keep" that should have recovered (sample a known-analytic-within-tol always recovers). Extend `fuzz_nurbs_boolean` to optionally pre-`simplify`.
- Exit gate: EXACT CI triplet (fmt --all --check; clippy --workspace --all-targets -- -D warnings; cargo test --workspace); prior artifacts replay; 10-min soaks CLEAN; LOG addendum; merge; verify CI green.

## Deferred to M-later (honest ledger)
- Face-MERGE of split analytics (two half-cylinder faces -> one) wired into persistent naming (file 24 priority #4; needs the naming layer).
- Constraint-aware re-fit / beautification (coaxiality/parallelism snapping, file 24 priority #5; Benko-Kos-Varady + Langbein) -- a separate milestone tied to the constraint solver.
- Helical / developable / translational surface recovery and learned (UV-Net) type proposals (file 24 priority #6).
- General trimmed-NURBS faces with multiple/curved trim loops; exact trimmed-NURBS mass properties (still the coarse tessellated oracle on genuinely-freeform faces).
