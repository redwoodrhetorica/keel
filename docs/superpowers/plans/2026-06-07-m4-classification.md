# M4: Intersection Primitives, PMC, Mass Properties Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Curve-curve and curve-surface intersection, parameter-space face containment (pcurves for the primitives), ray-cast point membership classification over regions, and divergence-theorem mass properties: the first consumers that can FALSIFY the M3 constructor orientation conventions.

**Architecture:** Intersections live in keel-geom (pure geometry); PMC and mass properties live in keel-topo (they need regions and loops). The affordability gradient governs: analytic-analytic pairs take exact algebraic paths through keel-math's solvers; anything involving splines routes through Bernstein machinery (univariate `Bernstein::roots` after implicit substitution; the multivariate PP solver for parametric-parametric) with Newton polish. Combinatorial decisions (crossing parity, on-boundary verdicts) never read raw f64 signs outside the predicate/tolerance discipline.

**Research basis:** nurbs/04 sections 1, 2, 6 (CCI, CSI, two-plane reduction, Toth interval-Newton context), kernel/01 (Tilove SMC/PMC, neighborhood problem), kernel/11 (affordability gradient), kernel/06 (interrogation foundations). Spec D5/D6/D7.

**Carried lessons:** power-of-two normalization before algebraic solves; overflow-safe midpoints; constructor-grade input validation on every public entry; oracles planned before code; `if ($LASTEXITCODE -eq 0)` commit guards.

**Branch:** `m4-classification` off master.

---

## Scope decisions (binding)

1. **Pcurves arrive now, for primitives only.** PMC needs point-in-face, which needs parameter-space loop containment. M3's primitive faces have analytic pcurves (iso-segments and full-period spans in UV); Task 3 attaches them to fins. General trimmed pcurves (from SSI) remain M5 work; everything in M4 is written against the pcurve interface so M5 trims slot in.
2. **PMC verdicts are tolerant-In/Out/On** (spec D2): `On` when distance to the nearest face is within the body tolerance; In/Out decided by crossing parity of a deterministically chosen ray, with a deterministic retry ladder on degenerate hits (hit near an edge/vertex/tangency = retry with the next direction, never perturb randomly: determinism contract).
3. **Mass properties integrate parameter rectangles.** Primitive faces cover full parameter rectangles (seam edges bound them), so Gauss-Legendre over the rectangle is exact-enough (machine precision for polynomial integrands, ~1e-12 for trig). The volume SIGN test against the region structure is the orientation audit. Green-theorem trimmed-region integration lands with M5 trims.
4. **Curve-curve intersection ships analytic-analytic exact and spline-pairs via PP**; tangency is REPORTED (multiplicity flag from clustered roots), not resolved: tangent intersection semantics belong to the boolean classifier (M6).

---

## File structure

- Create: `crates/keel-geom/src/intersect.rs` (CCI + CSI)
- Modify: `crates/keel-geom/src/surface.rs` (implicit forms: `Surface3::implicit_eval(p) -> f64` signed, plus gradient)
- Modify: `crates/keel-topo/src/construct.rs` (pcurve attachment)
- Create: `crates/keel-topo/src/pmc.rs` (UV containment + ray-cast PMC)
- Create: `crates/keel-topo/src/massprops.rs`
- Create: `fuzz/fuzz_targets/fuzz_pmc.rs`
- Benches in `crates/keel-geom/benches/geom.rs` + gate

---

### Task 1: implicit forms for analytic surfaces

`Surface3::implicit(p: Vec3) -> f64`: signed scalar field, negative inside (plane: (p-o).z_axis; cylinder: |radial|^2 - r^2 normalized; sphere: |p-c|^2 - r^2; cone: radial^2 - (r(h))^2 with axis-distance form; torus: (|radial| - R)^2 + z^2 - r^2). Plus `implicit_gradient(p)`. Tests: zero on `point(u,v)` samples (1e-12 scaled), gradient matches FD, sign convention checked against `project` distance for off-surface points. These power CSI fast paths AND the PMC fuzz oracle.

### Task 2: curve-curve intersection (keel-geom intersect.rs)

```rust
pub struct CurveHit { pub t1: f64, pub t2: f64, pub point: Vec3, pub tangential: bool }
pub fn intersect_curves(a: &Curve3, da: Domain, b: &Curve3, db: Domain, tol: f64)
    -> Result<Vec<CurveHit>, GeomError>
```
- line/line: exact 2x2 solve with parallel/coincident classification through predicates.
- line/circle, line/ellipse: substitute into the conic's plane + quadratic (keel-math `solve_quadratic` after plane projection); out-of-plane lines: distance check first.
- circle/circle (coplanar + skew rejection), conic/conic in a shared plane: via Bernstein root isolation on the angle parameterization (bounded, robust) rather than resultant gymnastics.
- Any pair involving NURBS: difference system into MultiBernstein over [0,1]^2 (3 equations, 2 unknowns; PP handles overdetermined), root boxes polished by 2-var Newton on the squared distance, `tangential` = Jacobian rank deficiency at the hit (threshold documented).
Oracles: constructed intersections (two circles with known crossing points; NURBS circle vs its own exact line secant), dense-sample no-miss proptest (any sample pair closer than tol must have a reported hit nearby).

### Task 3: curve-surface intersection

```rust
pub struct SurfaceHit { pub t: f64, pub uv: (f64, f64), pub point: Vec3, pub tangential: bool }
pub fn intersect_curve_surface(c: &Curve3, dc: Domain, s: &SurfaceRef, tol: f64)
    -> Result<Vec<SurfaceHit>, GeomError>
```
- Curve x Surface3: compose `implicit(C(t))`: line/plane exact linear; line/quadric exact quadratic; line/torus quartic via `Bernstein::roots` on a normalized interval; general curve (incl. NURBS curve) vs analytic surface: f(C(t)) sampled into Bernstein form per Bezier segment of the curve (composition by interpolation is FORBIDDEN; build the composition exactly: for polynomial implicit forms of degree d and curve segments of degree p, f(C(t)) is degree d*p polynomial obtained by exact Bernstein arithmetic: implement `compose_implicit` via de Casteljau-based polynomial arithmetic on the segment, the Farouki-Rajan discipline).
- NURBS curve x NurbsSurface: 3-var PP system (S(u,v) - C(t) = 0) + 3x3 Newton polish (Jacobian [S_u, S_v, -C']), uv recovered from the root box.
- Hits report `uv` ON the surface always (inversion via projection when the algebraic path loses it).
Oracles: line through sphere center (2 hits at exact distance), tangent line to cylinder (tangential flag), helix-free sanity: NURBS arc vs revolved torus hits match analytic torus implicit roots.

### Task 4: pcurves for primitives + UV containment (keel-topo)

- Extend `construct.rs`: every fin of every primitive face gets `pcurve = Some((curve_key, sense))` where the pcurve is a `Curve3::Line` in UV-as-3D (z = 0) parameter space (iso segments; seam fins get the two rectangle sides; closed-edge fins get full-period horizontal spans). Document the UV embedding convention (pcurves live in the face's (u, v, 0) space).
- `pmc.rs`: `point_in_face_uv(body, face, uv) -> InOutOn`: WINDING NUMBER of the loops' pcurves about the query point (not crossing parity: winding is robust to noise and open boundaries and is the interface M5's curved trims need; for M4's exact segment pcurves the two coincide and winding costs the same). Periodic faces (cylinder u, torus u and v) evaluate in the universal cover (lift the query and the loop representatives by the period) per Liu et al. 2025 (arXiv:2510.25159), which makes seam-crossing loops consistent. When curved trims arrive (M5), adopt that paper's recursive evaluation with ellipse bounds on Bezier segments (linear-time, no subdivision). `On` within parameter tolerance mapped from the face tolerance via first-fundamental-form scaling.
- Validation extension: every fin of a face with a surface must carry a pcurve whose endpoints map (via the surface) to its edge's endpoints within tolerance (watertightness in parameter space). Run on all primitives.

### Task 5: ray-cast PMC

```rust
pub enum Containment { In(RegionKey), On(FaceKey), Out } // Out = infinite region
pub fn classify_point(body: &Body, p: Vec3) -> Result<Containment, TopoError>
```
Architecture per the 2026-06-07 performance research (user-prompted): the
PRIMARY classifier is nearest-face signed classification, not ray casting.

1. Nearest face: project p onto every face (linear scan now; the Task 5b
   BVH makes it O(log n)), UV-contain the foot via the winding test.
   Distance <= tolerance: report On.
2. Signed verdict from the nearest face with an interior foot: the sign
   of (p - foot) . outward_normal (outward derived from front/back
   region solidity) names the region directly via the face's region
   links. O(1) local information, no ray.
3. Foot on an edge/vertex (within parameter tolerance of a loop):
   angle-weighted pseudonormal (Baerentzen-Aanaes 2005) over the
   incident faces decides; if still degenerate (non-manifold incidence),
   FALL BACK to the deterministic ray ladder: directions +x, +y, +z,
   then the documented mixed set; degenerate hits (near-loop in UV, or
   tangential) advance the ladder; crossings walk regions from infinity
   via front/back links.
4. Debug builds cross-check nearest-face verdicts against the ray
   parity on a sampled basis; disagreement = TopoError (bug, not
   tolerance).

5b. **Face-AABB BVH** (shared infrastructure; booleans and M5 reuse it):
   static BVH over per-face control/geometry AABBs with ray traversal
   and nearest-primitive search. Lives in keel-topo (or keel-geom if
   face-agnostic), built per body on demand, invalidated by mutation.

Tests: all five primitives (center In(solid), far Out, surface-point On),
block with a void (mvfs inside): nested classification, the two-glued-
cubes body, proptest vs `Surface3::implicit` sign oracle for the
analytic primitives, near-edge and near-vertex queries exercising the
pseudonormal path, and a documented case that forces the ray fallback.

Scaling notes for the record: generalized winding numbers (Jacobson et
al. 2013; Barill et al. 2018 fast winding numbers; 2024 one-shot GWN,
arXiv:2408.04466) are the import/healing-era fallback for UNTRUSTED
geometry (gaps, soups), where local methods lie; not needed while all
bodies are operator-built and validated.

### Task 6: mass properties

```rust
pub struct MassProps { pub volume: f64, pub centroid: Vec3, pub inertia: [[f64; 3]; 3] }
pub fn mass_properties(body: &Body) -> Result<MassProps, TopoError>
```
Divergence theorem per face over its parameter rectangle (from pcurve bounds): V = (1/3) sum_f orient_f * integral S . (S_u x S_v) du dv with Gauss-Legendre (order 8 per axis, subdivided per knot span for NURBS faces), orient_f = +1 when the face's FRONT faces the infinite-side region along the solid's outward direction (derived from front/back region solidity), else -1. Centroid and inertia by the standard higher moments.
**Golden falsification tests:** block dx*dy*dz exact; sphere 4/3 pi r^3 to 1e-9 relative; cylinder pi r^2 h; cone pi r^2 h / 3; torus 2 pi^2 R r^2; VOLUME MUST COME OUT POSITIVE from the orientation conventions alone, with no sign fudge: a negative volume here means the M3 conventions are wrong and the fix goes in M3's code, not in a `abs()`.
Centroids at the analytic centers; sphere inertia 2/5 m r^2 diagonal.

### Task 7: fuzz, benches, gate

- `fuzz_pmc`: random points + random primitive parameters; classification must agree with the implicit-form sign oracle (away from the tolerance band) and never panic; degenerate ray retries must terminate (ladder exhaustion = clean error).
- Benches: classify_point on the torus, mass_properties on the torus.
- Exit gate: fmt, clippy, workspace tests, all prior artifacts replay, 10-minute fuzz_pmc soak CLEAN, LOG addendum, merge.

## Self-review

- PMC's neighborhood problem (Tilove) is dodged honestly: degenerate hits advance a deterministic ray ladder instead of being classified; the full neighborhood classifier belongs to the boolean engine (M6) where it is unavoidable.
- The composition discipline in Task 3 (exact Bernstein composition, never sample-and-fit) is the load-bearing numerical decision; it keeps certified-no-miss semantics.
- Mass properties is deliberately the LAST sign-sensitive consumer wired to the M3 conventions with no compensating sign: that is the entire point of the milestone.
