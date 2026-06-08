# M6c: Block-Cylinder Boolean (Mixed Analytic Surfaces) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the winding-classified boolean pipeline to **mixed analytic surfaces**, proven by the canonical CAD operation: **block − cylinder = a drilled hole**. This is the last analytic stop before NURBS (M7). The new capability over M6b is the **periodic imprint across a seam**: the plane-cylinder SSI is a circle that wraps the cylinder lateral face and unavoidably crosses its vertical seam (unlike the sphere, which could be re-seamed to dodge the crossing). That crossing-aware closed-curve imprint, plus cylinder tessellation for the winding number, are the milestone's substance.

**Research basis (re-read 2026-06-07):** c-surface-intersection.md section 4 (SSI is exact on the linear/rational sub-layer; plane-cylinder is a closed-form circle/ellipse), d-booleans-tolerant.md (winding-number classification, already adopted in M6b, is surface-agnostic so it extends to cylinder faces for free), kernel/01 Part 6 (boundary evaluation: imprint across an existing boundary edge = split the edge at the crossing, then close the loop there). Spec D3/D5/D7.

**The key insight (why this needs new imprint machinery).** A sphere's SSI circle could be made crossing-free by seaming the sphere equatorially (M6b). A cylinder's lateral face has a vertical seam line from the bottom cap circle to the top cap circle; ANY full-wrap circle around the cylinder crosses that seam exactly once. So the SSI circle cannot be imprinted as a crossing-free interior ring. Instead: split the seam line at the crossing point P, then base the closed circle edge at P (a `mef` closing a loop at P), which splits the lateral face into the upper and lower portions — exactly the drilled topology. The result's holed block faces (a square face with a circular hole) are handled by the M6b import-and-glue stitch, which already imports inner-ring loops.

**Carried lessons:** soundness-gate new winding inputs before classifying on them (cylinder tessellation orientation audit); every fuzz finding becomes a golden regression; honest deferral ledger; `if ($LASTEXITCODE -eq 0)` commit guards; debug-validate after each mutation; partial-success faults.

**Branch:** `m6c-cylinder` off master.

---

## Scope boundary (binding)

1. **M6c delivers block − cylinder (and block ∩ cylinder) for a cylinder whose caps clear the block** (a through-hole / clean transversal: the cylinder lateral cuts the block, the caps do not). Cone/torus, cylinder-cap-meets-block, and cylinder-axis-not-perpendicular-to-a-block-face are noted but only implemented as far as the proof needs.
2. **Winding number stays the classifier.** Cylinder tessellation feeds it; the classifier code is unchanged.
3. **The crossing-aware imprint is a NEW primitive** (`imprint_closed_curve_crossing`) alongside the M5b interior-ring imprint; the boolean's per-face imprint dispatch picks it when the closed seam curve crosses a face boundary edge.
4. **Coplanar/coincident, cone/torus booleans, exact trimmed mass-properties, and tolerant edges remain deferred** (M6d/M7); the partial-success model declines what is unimplemented.

---

## File structure

- Modify: `crates/keel-topo/src/tessellate.rs` — cylinder lateral + cap tessellation (height from the cap circle edges; trimmed by the SSI circle's height for fragments).
- Modify: `crates/keel-topo/src/boolean.rs` — `imprint_closed_curve_crossing`; per-face dispatch detects a seam-crossing closed curve and routes to it; cylinder fragment interior point.
- Create: `fuzz/fuzz_targets/fuzz_cyl_boolean.rs` — random block + axis-aligned through-cylinder; result validates or declines, volume bounded, never panics.
- Modify: `crates/keel-topo/src/lib.rs` if needed.

---

## Tasks

### Task 0 (MANDATE): cylinder winding soundness before classifying on it
Add cylinder tessellation, then prove (like M6b Task 0) the winding number is sound for cylinders BEFORE any classification uses it: a tessellated solid cylinder gives w within 2e-3 of 1.0 deep inside and 0.0 well outside across a magnitude ladder; orientation audit gives +1 inside (outward = radial-from-axis on the lateral, ±axis on the caps); graceful continuous degradation crossing the lateral wall and a cap; deterministic. THIS GATES the rest.

### Task 1: cylinder tessellation (`tessellate.rs`)
- `tessellate_face` routes `Surface3::Cylinder` to `tessellate_cylinder`.
- Lateral face: height `h` from the two cap-circle edges (distance between their centers along `frame.z`); mesh `u in [0,2pi] x v in [0,h]` into outward (radial) quads. A fragment trimmed by a horizontal SSI circle keeps only the band on its side (axial-height filter, the cylinder analogue of the sphere cap-side filter — keyed off the CLOSED SSI circle, not the open vertical seam).
- The cylinder's planar cap faces already tessellate via `tessellate_planar`.
Tests: a solid cylinder's GWN is ~1 inside / ~0 outside (Task 0 uses this).

### Task 2: crossing-aware periodic imprint (`boolean.rs`)
```rust
fn imprint_closed_curve_crossing(&mut self, face, curve, tol) -> Result<ImprintReport, TopoError>
```
- Find the single point P where the closed `curve` crosses one of the face's boundary edges (the cylinder's vertical seam line), within tol (sample the curve, test proximity to each boundary edge segment; refine).
- `split_edge` the crossed boundary edge at P.
- `mef(fin_ending_at_P, fin_ending_at_P, surface)` to base the closed curve edge at P, splitting the face into the two portions; attach the 3D curve + pcurves to both fins; the new face inherits the surface.
- The per-face imprint dispatch in `imprint_operand`: for a single closed seam curve, test whether it crosses a face boundary edge; if yes -> `imprint_closed_curve_crossing`, else the existing interior-ring `imprint_closed_curve`.
Tests: imprint the plane-cylinder SSI circle onto a cylinder lateral face -> the lateral splits into upper + lower fragments, both valid, the circle edge manifold (radial 2) with pcurves; the seam line is split at P.

### Task 3: cylinder fragment interior point (`boolean.rs`)
Dispatch `face_interior_point` for `Surface3::Cylinder` to a cylinder version: a point on the lateral fragment, on its side of the bounding SSI circle, robust to periodicity. Mirror the sphere approach: find the CLOSED SSI circle boundary edge, take its axis (= cylinder axis), step to the fragment's mid-height on the cylinder surface (a point at the lateral fragment's central height and a representative angle away from the seam). For a cap fragment (planar) the existing planar path applies.
Tests: interior point of an upper/lower cylinder-lateral fragment lies on the cylinder within that fragment's height band.

### Task 4: block - cylinder through-hole (the proof)
Config: block `[0,4]^3`, cylinder radius 1 on the vertical axis through the block centre, height 6 (caps at z=-1 and z=5, clearing the block). Pipeline:
- seam_curves: cylinder lateral meets block top (z=4) and bottom (z=0) in circles (crossing-free, on planar faces -> existing ring imprint); block top/bottom planes meet cylinder lateral in circles (crossing the cylinder seam -> Task 2). Cylinder caps clear the block (no seams).
- classify (GWN): the block's drilled-out interior wall pieces, the cylinder lateral band inside the block.
- select (Difference): block faces outside the cylinder + cylinder lateral band inside the block, REVERSED (the hole wall faces into the hole).
- stitch: route to `stitch_by_import` (cylinder lateral is curved). The block top/bottom become holed faces (square + inner circle ring) -> imported with their inner rings; the cylinder lateral band reversed; glue the four circle seams.
- PROOF: result validates; volume = `4^3 - pi*1^2*4` (block minus the cylinder segment inside the block) within the tessellated oracle's ~few %. GWN of a point in the remaining material is inside; a point in the drilled hole is outside.
Also `block intersect cylinder` = the cylinder segment inside the block (a radius-1 height-4 cylinder), volume `pi*1^2*4`.

### Task 5: fuzz + gate
- `fuzz_cyl_boolean`: random block + a random axis-aligned through-cylinder (radius/centre clamped so it stays a clean transversal through-hole); result validates or declines cleanly, volume finite and bounded (`V(block - cyl) <= V(block)`, `V(block ∩ cyl) <= min`), never panics.
- Exit gate: fmt, clippy, workspace tests, all prior artifacts replay, 10-min fuzz_cyl_boolean + fuzz_boolean + fuzz_winding soaks CLEAN, LOG addendum, merge.

## Deferred to M6d / M7 (honest ledger)
- Cone and torus booleans (tessellation + their seam structures).
- Cylinder cap meeting the block (cap-plane coincidence / partial-depth blind holes at an angle).
- Coplanar/coincident neighborhood classification (winding-number-vector, n-ary).
- EXACT trimmed mass properties for curved caps/bands (the volume oracle stays the coarse tessellation until then).
- Tolerant edges/vertices (Jackson) — the M7 NURBS-SSI requirement.
- BVH-accelerated GWN + AABB face-pair localization (the all-pairs O(n^2) throughput cost).
- NURBS-bounded-solid booleans (the M7 proof bar).
