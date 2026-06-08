# M5b: Imprint, Trimmed Faces, Surface Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn M5a's `SsiCurve`s into topology: pcurves on faces, single-face imprinting (split a face along a curve lying on it), two-body imprinting (the boolean precursor producing the shared non-manifold seam with correctly dihedral-sorted radial cycles), the bounded/validated surface extension service, and trimmed-face mass properties. After M5b the topology is trim-ready and M6 booleans become classify + stitch + select over an already-imprinted pair.

**Architecture:** Imprint lives in keel-topo (it mutates bodies through the M3 operators); pcurve computation and extension live where their geometry does (keel-geom). The dihedral radial sort deferred from M3's `glue_edges` comes due here, its first real consumer. Everything routes toleranced coincidence decisions through the caller per the M3 contract; M5b is that caller, so the toleranced judgement (does this SSI curve actually lie on the face? do these two edges coincide?) is made here against the session tolerance.

**Research basis:** kernel/13 Part 2 (extension service requirements, re-read 2026-06-07), kernel/01 (boundary evaluation: intersect-imprint-classify-stitch), nurbs/04 section 3 (SSI topology), M3 gate section 4 (boolean pipeline; M5b implements imprint, M6 implements classify/stitch). Spec D3 (PES non-manifold), D5 (intersection), D9 (lineage through every op).

**Carried lessons:** every operator atomic + lineage-reporting; debug-build validate after each mutation; oracles before code; `if ($LASTEXITCODE -eq 0)` commit guards; honest deferral ledger.

**Branch:** `m5b-imprint` off master.

---

## Scope boundary (binding)

1. **M5b imprints; M6 classifies/stitches/selects.** M5b produces an imprinted body or body-pair with the intersection curves present as real edges and pcurves, the topology valid (every fin a pcurve, every face winding-correct). M5b does NOT decide which fragments survive a union/intersection/difference: that is M6.
2. **Analytic faces first; NURBS faces ride the same interface.** M5b's faces are M2/M4-built (analytic surfaces, with the revolved-NURBS primitives as the spline case). Pcurve computation is exact inversion for analytics, projection for NURBS. General trimmed-NURBS healing is M-healing-era work.
3. **Extension is bounded and may fail cleanly.** Per kernel/13: safe-reach cap, fold/weight validation, analytic-exact preferred, clean `Err` when neighbors cannot meet. No open-ended extrapolation.
4. **Coincidence judgement is explicit and toleranced** (the M3 deferral resolves here): `imprint` decides a curve lies on a face within tolerance; `glue_edges` callers assert geometric coincidence; the dihedral sort orders the merged radial cycle.

---

## File structure

- Modify: `crates/keel-geom/src/ssi.rs` (surface SsiCurve to carry `pcurve_a`, `pcurve_b`; compute them)
- Create: `crates/keel-geom/src/extend.rs` (extension service)
- Modify: `crates/keel-geom/src/surface.rs`, `nurbs_surface.rs` (inversion helpers for pcurve computation)
- Create: `crates/keel-topo/src/imprint.rs` (single-face + two-body imprint)
- Modify: `crates/keel-topo/src/ops.rs` (dihedral radial sort in glue_edges)
- Modify: `crates/keel-topo/src/massprops.rs` (Green-theorem trimmed-face integration)
- Modify: `crates/keel-topo/src/validate.rs` (pcurve-endpoint watertightness already exists; extend to imprinted faces)
- Create: `fuzz/fuzz_targets/fuzz_imprint.rs`

---

### Task 1: pcurves on SSI output
Extend `SsiCurve` with `pcurve_a: Curve3` and `pcurve_b: Curve3` (in each surface's (u,v,0) domain). Tier 1: exact (plane inversion = dot products; sphere/cylinder/cone/torus = closed-form angle/height inversion of the circle/ellipse result). Tier 2/3: project each fitted 3D sample into each surface's parameter domain (analytic exact `project`; NURBS `project_point_surface`-derived (u,v)), fit a 2D pcurve. Tests: pcurve endpoints map (via the surface) back to the 3D curve endpoints within tol; pcurve interior samples on-surface.

### Task 2: surface extension service (keel-geom/extend.rs)
```rust
pub enum ExtendDir { UMin, UMax, VMin, VMax }
pub enum ExtendMode { Natural, Tangential }
pub fn extend_surface(s: &SurfaceKind, dir: ExtendDir, amount: f64, mode: ExtendMode)
    -> Result<SurfaceKind, GeomError>
```
- **Analytic: exact, unbounded.** Planes/cylinders/cones/spheres/tori are domain-unbounded already; "extension" just widens the parameter window the consumer marches over. Return the same surface with an annotated wider domain (a thin `ExtendedDomain` wrapper or simply documented: analytics carry no domain, so extension is a no-op that always succeeds). This is the common M5b case and is exact.
- **NURBS: bounded natural extension by knot unclamping / de Boor recurrence** (Shi et al.; Lu-Wang). Compute safe reach from degree + boundary curvature + weight positivity (kernel/13 section 2.2). Validate the extended control net for folding (no sign flip of `S_u x S_v` along the new strip) and weight positivity. `Err(GeomError::Degenerate)` when reach exceeds the safe cap or validation fails: NEVER a folded surface.
Tests: analytic extension is identity-exact; NURBS short natural extension reproduces the surface on the original domain and stays fold-free; over-long extension fails cleanly; negative-weight-inducing extension refused.

### Task 3: dihedral radial sort (keel-topo, the M3 deferral)
In `glue_edges`, order the merged radial cycle by dihedral angle about the edge tangent. For each fin, the reference direction is its face's surface normal projected perpendicular to the edge tangent (using M2 `local_geometry` at the fin's pcurve midpoint); sort fins by `atan2` of that direction in the plane perpendicular to the tangent. Manifold (2-fin) cycles are unaffected; the non-manifold (4+) cycles get the correct angular order, which is what neighborhood classification (M6) reads. Test: two blocks glued along a shared edge produce radial-4 in correct dihedral order (the four face-sides alternate around the edge as geometry dictates, verified against hand-computed angles).

### Task 4: single-face imprint (keel-topo/imprint.rs)
```rust
pub fn imprint_curve(body: &mut Body, face: FaceKey, curve: &Curve3, pcurve: &Curve3, tol: f64)
    -> Result<ImprintReport, TopoError>
```
Cases by how the pcurve meets the face's loops (in UV):
- **Crossing (endpoints on two boundary points/edges):** `split_edge` at the crossings, `split_face` along the curve. New edge carries the 3D curve + both pcurves.
- **Closed interior loop:** create an inner ring (the M3 `kemr`/ring machinery via a bridge, or directly: new loop on the face + new face inside if the loop bounds area). Winding/region bookkeeping per M3.
- **Endpoint on a boundary, other interior (open spur):** `split_edge` + `mev`-style spur to the interior terminus.
Coincidence is judged here: the curve must lie on the face surface within tol (sampled check) or `Err`. Tests: imprint a circle onto a cube's top face (inner ring, validate, winding now excludes the disc); imprint a line crossing a face (two faces); lineage on all new entities; topology hash golden.

### Task 5: two-body imprint (REASSIGNED TO M6)
**Scope decision (2026-06-07, during execution):** two-body imprint
(localize face pairs, SSI each, imprint both sides, glue) is reassigned
to the opening of M6. Rationale discovered in execution: even the
simplest transversal case (cylinder through block) requires periodic-
wrap face splitting (a full cross-section circle on the cylinder's
lateral face is not an interior loop) and coplanar-face coincidence
handling, both of which are M6's classify/coincidence concerns. M5b
delivers the single-face imprint PRIMITIVES (the hard, reusable core);
M6 composes them with the classify/select logic only it owns. The
M3-gate boolean pipeline is unchanged; only the M5b/M6 task boundary
moved by one step. Original Task 5 sketch retained below for M6.


```rust
pub fn imprint_bodies(a: &mut Body, b: &mut Body, tol: f64) -> Result<ImprintReport, TopoError>
```
For every face pair (BVH/AABB localize), SSI (M5a) the surfaces; for each resulting SsiCurve, `imprint_curve` on A's face and B's face; `glue_edges` to identify the coincident imprinted edges (dihedral sort fires). Result: both bodies share identical edge geometry along the intersection; shared edges carry 4 fins. This is steps 1-3 of the M3 boolean pipeline; M6 adds classify + select. Tests: two overlapping blocks imprint to a shared rectangular seam (4 new coincident edges, radial-4, both bodies valid); a block and a cylinder imprint along the cylinder's circular intersection; validate + d-of-d hold on the non-manifold result.

### Task 6: trimmed-face mass properties (Green's theorem)
Lift the M4 parameter-rectangle limitation: integrate a trimmed face by Green's theorem over its UV region bounded by the pcurve loops (the divergence-theorem surface integrand pushed to a boundary integral around the trim loops). Reuses the pcurve polylines the winding test already builds. Test: a cube face with a circular hole imprinted has mass-property contribution = full-face minus disc (exact-ish); a block-with-a-through-hole body (two imprinted faces + cylinder wall) has the right volume.

### Task 7: fuzz, gate
`fuzz_imprint`: random analytic primitive pairs imprinted; the imprinted bodies must validate (every fin a pcurve, winding-consistent, d-of-d holds); no panics; glue produces consistent radial cycles. Exit gate: fmt, clippy, workspace tests, all prior artifacts replay, 10-min fuzz_imprint soak CLEAN, LOG, merge. (No 2-hour soak unless the user requests; the M5a interval foundation is already billion-execution-verified.)

## Deferred ledger (honest)
- General trimmed-NURBS faces (non-primitive splines) and their healing: M-healing era.
- Reflection-based G2 and energy-min extension (Shetty-White, Mo-Zhao): only natural + analytic in M5b; the others land when a gap-fill consumer needs controlled reach.
- The boolean classify/select/stitch: M6, built on M5b's imprinted pairs.
- Krawczyk-verified imprint-curve fitting: inherits M5a's staged upgrade.
