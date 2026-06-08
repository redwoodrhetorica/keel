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

### Task 4 (re-scoped to M7b): the first NURBS boolean
**Scope decision (2026-06-07, during execution):** the NURBS boolean is
moved to M7b, which becomes the NURBS-boolean-WITH-tolerant-edges
centerpiece (the differentiation thesis). Rationale discovered in
execution: a working NURBS boolean needs three substantial new pieces
that are each M7b-grade and are exactly where the exact-topology/
tolerant-geometry hybrid gets delivered:
  - the imprint pcurve on a NURBS face (project_point_surface +
    NURBS pcurve fit, generalizing the M5b analytic-only branch);
  - trimmed-NURBS FRAGMENT tessellation (the result lens has NURBS cap
    faces trimmed by the SSI circle -- needs the cap-side filter in
    parameter space, the NURBS analogue of tessellate_sphere's trim);
  - the NURBS cap fragment interior point (the apex via
    project_point_surface along the SSI circle axis, side from the loop
    kind).
M7a delivers the PLUMBING those three build on (tessellation of whole
NURBS faces, a NURBS-faced solid, soundness). M7b composes them with the
tolerant-edge core. This keeps M7a a clean soundness-gated foundation
and aligns the milestone boundary with the differentiation.

### Task 5 (re-scoped to M7b): fuzz the NURBS boolean.

## M7a exit gate
- The EXACT CI triplet (fmt --all --check; clippy --workspace
  --all-targets -- -D warnings; cargo test --workspace); LOG addendum;
  merge; verify CI green after push. (No new fuzz target: M7a adds no
  new mutation path -- nurbs_sphere is a constructor, tessellation is a
  read. The NURBS-boolean fuzz lands in M7b.)

## M7b (the centerpiece): NURBS booleans + tolerant edges
- The first NURBS boolean (NURBS sphere INTERSECT analytic sphere): the
  three pieces above + import-and-glue stitch; volume vs the lens formula.
- TOLERANT EDGES (the differentiation): plumb the SSI curve's
  `tol_achieved` (already computed by the M5a engine, currently thrown
  away) into `Edge.tolerance`; propagate the bound through the boolean;
  adopt Qi/Shapiro epsilon-solidity as a CHECKABLE validity contract.
  This is "exact topology decisions with tolerant geometry" -- the
  hybrid no kernel fully ships (file 11). Treat as the project's sharpest
  differentiation, not a checkbox.
- NURBS x NURBS booleans where the SSI is INEXACT (tier-3 marching).
- General trimmed-NURBS faces (multiple/curved trim loops); exact
  trimmed-NURBS mass properties; differential testing vs OCCT/ABC.
