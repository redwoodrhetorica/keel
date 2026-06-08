# M7a: NURBS-Faced Solids in the Boolean Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** First step of the proof milestone (M7, robust booleans on NURBS-bounded solids): make the winding-classified boolean pipeline accept **NURBS faces**. Build a NURBS-faced solid (a sphere revolved from a NURBS half-circle profile), extend every pipeline stage that currently bails on `SurfaceGeom::Nurbs` (tessellation for the GWN, the fragment interior point, the tessellated volume oracle), and prove a first NURBS-involved boolean. The hard NURBS-NURBS inexact-SSI + tolerant-edge core is M7b; M7a proves the NURBS *plumbing* on a curved NURBS solid whose SSI with an analytic operand is already supported (tier-2 analytic x spline, M5a).

**Research basis (re-read 2026-06-07):** d-booleans-tolerant.md section 4 (winding-number classification is surface-agnostic -> it extends to NURBS faces by tessellating them; tolerant edges required for inexact NURBS-NURBS SSI -> M7b), b-nurbs-freeform.md (evaluation via de Boor, surface tessellation, closest-point projection for pcurves), c-surface-intersection.md (tier-2 analytic-spline SSI gives a fitted curve + error bound). Existing: NurbsSurface::{point, local_geometry, project_point_surface, revolve_full, domain}; SSI tier-2/3 (M5a); the M6b winding classifier + import-and-glue stitch; the M6c periodic two-arc imprint.

**Carried lessons:** soundness-gate the NURBS winding before classifying on it (the M5a/M6b/M6c Task-0 mandate); run the EXACT CI triplet (fmt --all --check; clippy --workspace --all-targets -- -D warnings; cargo test --workspace) before merge; every fuzz finding -> golden regression; honest deferral ledger.

**Branch:** `m7a-nurbs` off master.

---

## Scope boundary (binding)

1. **M7a delivers NURBS-face PLUMBING + one NURBS-involved boolean** (NURBS sphere with an analytic operand, where the SSI is the supported tier-2 analytic x spline circle). Full NURBS x NURBS booleans, inexact-SSI tolerant edges, and tolerance propagation are M7b.
2. **Winding number stays the classifier**; NURBS tessellation feeds it. No new classifier.
3. **The NURBS sphere is a genuine curved NURBS surface** (revolved rational), not a degree-1 planar fake -- the point is to exercise real NURBS evaluation in the pipeline.
4. **Volume is the coarse tessellated oracle** (exact trimmed-NURBS mass properties is far future).

---

## Tasks

### Task 0 (MANDATE): NURBS winding soundness before classifying on it
Tessellate a NURBS face (Task 1) and prove the GWN is sound on a NURBS-faced solid BEFORE any classification uses it: a NURBS sphere solid gives w within 3e-3 of 1.0 deep inside and 0.0 well outside across a magnitude ladder; orientation audit gives +1 inside (the revolved surface's `local_geometry` normal must point outward); deterministic. THIS GATES the rest.

### Task 1: NURBS surface tessellation (`tessellate.rs`)
- `tessellate_face` routes `SurfaceGeom::Nurbs` to `tessellate_nurbs`.
- Sample `point(u, v)` over the face's `domain()` on a grid (finer than the analytic primitives, since NURBS curvature varies), emit two outward-oriented triangles per cell. Outward from `local_geometry(u,v).normal` adjusted by the face `sense`; fall back to the cell-centroid-minus-axis only if local_geometry is degenerate (sphere poles).
- Trimming: a whole-surface NURBS face (the primitive) meshes the full domain; trimmed NURBS fragments mesh the sub-domain inside their pcurve loops (reuse the planar UV-winding idea in parameter space). M7a's proof faces are whole or simply-trimmed.
Tests: a tessellated NURBS sphere has the right surface area / closed-ness (the GWN soundness in Task 0 is the real check).

### Task 2: NURBS sphere solid (`construct.rs`)
```rust
pub fn nurbs_sphere(&mut self, frame: Frame3, radius: f64) -> Result<PrimitiveOut, TopoError>
```
Build the sphere TOPOLOGY exactly as `sphere()` (V2 E1 F1: two poles, one seam meridian) but attach a NURBS surface (from `revolve_full` of a NURBS half-circle meridian profile about the frame axis) instead of `Surface3::Sphere`, and a NURBS meridian curve on the seam edge. Reuse the same crossing-free equatorial seam framing the M6b sphere booleans used. Validate.

### Task 3: NURBS fragment interior point + volume (`boolean.rs`)
- `face_interior_point` dispatches `SurfaceGeom::Nurbs` to a NURBS version: for a NURBS sphere cap fragment, the cap apex on the fragment's side of the bounding SSI circle (mirror the analytic-sphere method, but evaluate the NURBS surface). For a whole NURBS sphere, any surface point.
- `tessellated_volume` already sums over `tessellate_face`, so NURBS faces contribute once Task 1 lands.
- The boolean's post-condition already routes curved results to the tessellated volume.

### Task 4: NURBS pcurve for imprint + the first NURBS boolean (proof)
- The imprint pcurve on a NURBS face: replace the M5b `face_analytic_surface` precondition with a NURBS path -- sample the SSI curve, `project_point_surface` each sample to (u, v), fit a pcurve (the M5b sample-invert-fit, generalized off the analytic-only branch). The crossing/interior-ring topology (M6b/M6c) is surface-agnostic; only the pcurve computation differs.
- PROOF: a **NURBS sphere INTERSECT an analytic sphere** (equatorially seamed, crossing-free), distance 1.5: the SSI is the tier-2 analytic-vs-spline circle; imprint splits each sphere into caps; classify (NURBS-aware GWN); select; stitch (import-and-glue); result validates; tessellated volume within ~5% of the exact lens formula. Also the analytic-vs-analytic sphere lens must stay green (no regression).

### Task 5: fuzz + gate
- Extend a fuzz target (or add `fuzz_nurbs_boolean`) with NURBS-sphere x analytic-sphere pairs at random offsets/radii: result validates or declines cleanly, volume bounded, never panics.
- Exit gate: the EXACT CI triplet, all prior artifacts replay, 10-min soaks CLEAN on the new/changed targets, LOG addendum, merge (verify CI green after push).

## Deferred to M7b (honest ledger)
- NURBS x NURBS booleans where the SSI is INEXACT (tier-3 marching): tolerant edges carrying the SSI error bound (Jackson local tolerance), tolerance propagation through the boolean, epsilon-solidity (Qi/Shapiro) as the validity contract.
- General trimmed-NURBS faces with multiple/curved trim loops and their tessellation.
- Exact trimmed-NURBS mass properties.
- Differential testing vs OCCT over the ABC corpus.
