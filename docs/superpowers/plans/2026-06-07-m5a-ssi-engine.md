# M5a: Surface-Surface Intersection Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The SSI engine in keel-geom: given two surfaces, produce the complete set of intersection curves with certified branch enumeration, traced and fitted within a stated tolerance, with pcurves in both parameter domains. M5b (separate plan) wires the output into topology as trims and imprints.

**Architecture (nurbs/04 section 10, the three-tier strategy):**
1. **Exact tier:** analytic-analytic pairs with analytic-curve results take closed-form paths (plane/plane line; plane/sphere, sphere/sphere circles; plane/cylinder conics; coaxial families). Everything else falls through.
2. **Lower-dimensional tier (analytic x spline):** substitute the spline surface into the analytic implicit form via EXACT bivariate Bernstein composition (the surface generalization of M4's compose_implicit): the intersection becomes the zero set of one bivariate Bernstein field on ONE parameter domain. 2D implicit-curve topology is certified by PP on the critical-point systems (f = f_u = f_v singular points; f = f_v = 0 u-turning points), then traced in 2D. This is Krishnan-Manocha's insight executed with our own machinery, and it covers every primitive-vs-spline case.
3. **General tier (spline x spline):** Bezier patch pair subdivision pruned by AABBs and Hohmeyer GAUSS-MAP SEPARABILITY (normal-cone separation proves loop-freedom and projective monotonicity per leaf pair); collinear-normal seeds (4-var PP system, built exactly from hodograph cross products) for any pair that will not separate; boundary seeds from M4 CSI; predictor-corrector marching with curvature-derived steps and a KRAWCZYK-VERIFIED corrector (the M2b interval enclosures cash in: each accepted step carries an existence/uniqueness certificate for the corrected point in its box); turning-point scaffolding from the collinear-normal set.

**Output contract:**
```rust
pub struct SsiCurve {
    pub curve: NurbsCurve,          // 3D fit within `tol_achieved`
    pub pcurve_a: NurbsCurve,       // (u,v,0) in surface A's domain
    pub pcurve_b: NurbsCurve,
    pub closed: bool,
    pub tangential: bool,            // anywhere along the branch
    pub tol_achieved: f64,           // verified Hausdorff-style bound
}
pub enum SsiResult {
    Empty,
    Points(Vec<SsiPoint>),           // isolated touch points
    Curves(Vec<SsiCurve>),
    Coincident,                      // overlap region: M5b/M6 handlers
}
pub fn intersect_surfaces(a: &SurfaceRef, b: &SurfaceRef, tol: f64) -> Result<SsiResult, GeomError>
```
Solution-dimensionality classification happens FIRST (empty/points/curves/coincident) per Patrikalakis-Maekawa; coincident is detected and returned, never mis-traced.

**Honesty ledger (what is certified in v1):** branch enumeration is certified-conservative (separability + collinear-normal + boundary seeds enumerate every branch; the proofs are the cited ones); the traced polyline is Krawczyk-verified pointwise; the FIT is verified by dense sampled distance plus a derivative-bound inflation, which is a strong check but not a formal enclosure. The full validated-ODE enclosure and the Yang-Jia-Yan algebraic-topology escalation are explicitly staged as upgrades behind the same API.

**Research basis:** nurbs/04 sections 3-4, 8-10 (full re-read 2026-06-07); kernel/11 four-tier predicate strategy (spec); kernel/13 NOTE: the surface EXTENSION service must be co-designed here: the engine accepts surfaces with domains slightly LARGER than their natural bounds (analytics are naturally unbounded; NURBS extension is M5b work), so trims never starve the marcher at face boundaries.

**Branch:** `m5a-ssi` off master.

---

## Tasks

### Task 0: interval soundness audit (USER MANDATE, blocks Task 5)
Certification is only as good as the interval layer under it. Before
any Krawczyk machinery is built:
1. **Fix the overflow hole.** `Interval::mul`/`add` of near-MAX
   operands overflow bounds to +-inf, silently violating the finite-
   bounds invariant in release builds. Decision: bounds become
   EXTENDED reals (+-inf legal, meaning unbounded on that side);
   `widened()`, `contains`, `sign`, `sqrt`, `checked_div` and the
   de Casteljau consumers handle infinite bounds; NaN construction
   (inf - inf) is made impossible by case analysis, tested.
2. **Half-ulp containment property.** For every op: the correctly
   rounded f64 result fl(x) of the true real value x satisfies
   |x - fl(x)| <= ulp/2, so soundness REQUIRES
   r.lo <= next_down(fl) and next_up(fl) <= r.hi. Property-test this
   across a magnitude ladder: normals, denormals, near-MAX, mixed
   signs, zero (the M1/M2 fuzz-findings magnitude classes).
3. **Chained-op soundness.** Random expression trees (depth <= 8) of
   interval ops evaluated alongside pointwise f64 at random members:
   the point result must stay enclosed at every node.
4. **Enclosure re-audit at scale.** The M2b interval de Casteljau
   proptests re-run with extreme-magnitude control points (1e-300,
   1e300, denormals) added to the strategies.
5. **fuzz_interval target**: arbitrary op programs over arbitrary
   f64s; containment + no-NaN invariants; 10-MINUTE SOAK CLEAN IS THE
   GATE for starting Task 5.

### Task 1: solution classification + exact analytic-analytic tier
`crates/keel-geom/src/ssi.rs`. Pair dispatch with up-front coincidence detection (same-type parameter comparison within tolerance: coplanar planes, concentric equal spheres, coaxial equal cylinders...). Exact results:
- plane/plane: Line3 (predicate-routed parallel/coincident classification).
- plane/sphere: Circle3 or point or empty (center distance vs r).
- sphere/sphere: Circle3/point/empty (radical plane).
- plane/cylinder: ellipse (Ellipse3)/line pair/single line (tangent)/empty by axis-angle cases; exact parameter pcurves for both operands.
- plane/cone: circle/ellipse cases only (axis-perpendicular and tilted-within-cone planes); parabola/hyperbola sections route to tier 2 (Curve3 lacks those types; DOCUMENTED).
- cylinder/cylinder, cylinder/sphere, cone/sphere... coaxial cases exact (circles); general cases route to tier 2 via implicitization of one side.
- torus pairs: axis-perpendicular plane circles + coaxial cases exact; else tier 2.
Tests: every exact case against implicit residual (points on result curve satisfy BOTH implicit forms to 1e-12) + pcurve coincidence.

### Task 2: bivariate Bernstein field machinery
`compose_implicit_surface(analytic, patch) -> MultiBernstein(2 vars)`: exact composition of the analytic implicit polynomial with a rational Bezier patch (numerator form, products via tensor Bernstein multiplication; quadrics give degree (2p, 2q), torus (4p, 4q)). Needs bivariate Bernstein multiply: implement `MultiBernstein::mul` (tensor product convolution per axis, the Farouki-Rajan formula per dimension). Oracle tests: field value vs direct implicit(S(u,v)) at random parameters, 1e-12 scaled.

### Task 3: certified 2D implicit-curve tracer (tier 2 core)
Given a bivariate Bernstein field f on [0,1]^2:
1. Critical points by PP: singular (f = f_u = f_v), u-turning (f = f_v = 0), v-turning (f = f_u = 0); f_u, f_v by Bernstein derivative along each axis (implement `MultiBernstein::derivative(axis)`).
2. Border crossings: univariate Bernstein roots of f restricted to the four edges.
3. Segment scaffold: critical points + border crossings split every branch into monotone arcs (Bajaj significant-point completeness); march each arc with parameter continuation (step in the dominant axis, Newton-correct the other; curvature-adaptive steps), endpoints snapped to scaffold points.
4. Closed loops with no scaffold point: by the turning-point theory any interior loop contains u- and v-turning points, which step 1 found: seed there. (No scaffold point at all + nonempty interior sign variation = escalate to subdivision.)
5. Output: 2D polylines per branch tagged with singular/tangential flags.
Tests: circles (f = implicit circle in UV), figure-ground sign tests, two-branch hyperbola-like fields, a tangential (double-root) field flagged not crossed, dense-sample no-miss oracle (any sign change along a scan line lies near a traced branch).

### Task 4: tier-2 SSI (analytic x spline)
Per Bezier patch of the spline operand: compose (Task 2), trace (Task 3) in the patch domain, lift 2D branches to 3D points via the patch, compute the analytic side's pcurve by exact projection (M4 `project`), merge branch pieces across patch boundaries (endpoint matching within tol), fit (Task 6). Tangential branches flagged from the tracer.
Oracles: revolved-exact sphere vs analytic plane (result must be the exact circle: compare against tier-1 plane/sphere of the analytic twin); revolved torus vs plane (Villarceau-adjacent configurations sampled-residual checked); cylinder-as-NURBS (extruded arc) vs analytic sphere.

### Task 5: tier-3 SSI (spline x spline)
1. Normal cones per Bezier patch: bounding cone of the hodograph cross-product control vectors (Daniel-Daniel construction, conservative); Hohmeyer separability test = linear separation of the two cones (small LP or the dual test on cone axes/half-angles).
2. Recursive patch-pair subdivision until separable or size floor; non-separable floor pairs: collinear-normal PP system (n_a x n_b = 0 + coincidence, 4 vars, built exactly from homogeneous control products) for loop seeds + tangential classification.
3. Seeds: boundary crossings via M4 CSI (patch border curves vs other surface) + loop seeds from step 2.
4. March: predictor along t = n_a x n_b normalized, curvature-adaptive step (kernel/06 fundamental forms), corrector = 3-var Newton on (S_a - S_b) with the two-plane closure, KRAWCZYK-VERIFIED on the M2b interval eval of both patches (reject + halve step when verification fails: certified-step marching); stop at borders/seeds/loop closure.
5. Dedup branches across seeds; classify tangential (normal alignment along branch).
Oracles: two revolved-exact spheres (intersection = exact circle, compare to tier 1); revolved torus vs revolved cylinder configurations with known counts; near-tangent sphere pairs (the certified-step machinery must not jump branches: assert single branch with correct topology).

### Task 6: branch fitting + verification
Fit polylines (3D + both pcurves simultaneously) with cubic least-squares NURBS, knots by arc-length; verify: dense parameter sampling of fitted vs traced (and fitted point's distance to BOTH surfaces via projection), inflate by a sampled-derivative bound; refine knots and refit until tol met; record `tol_achieved`. Closed branches fit periodically (wrap by knot repetition... clamped with matched ends + tangents for M5a, documented).

### Task 7: fuzz, benches, gate
`fuzz_ssi`: random analytic pairs (+ random rigid transforms): every reported curve point must satisfy both implicit forms within tol; no panics; Coincident never mis-traced (coaxial constructions). Benches: plane/sphere exact, sphere-as-NURBS vs plane (tier 2), two NURBS spheres (tier 3). Exit gate: standard + 10-min soak + LOG + merge.

## Deferred to M5b (the topology half)
Trim loops from SSI output (real pcurves on faces); imprint (split_face along SsiCurves, glue_edges with the dihedral radial sort it now needs); NURBS surface extension service (kernel/13 co-design); PMC/massprops upgrades for trimmed faces (winding handles curved pcurves already by construction); validated-ODE and algebraic-topology escalations as needed by M6 robustness data.
