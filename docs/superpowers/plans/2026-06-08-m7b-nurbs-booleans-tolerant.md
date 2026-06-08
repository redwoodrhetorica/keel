# M7b: NURBS Booleans + Tolerant Edges Implementation Plan (THE CENTERPIECE)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** The proof milestone and the project's sharpest differentiation: **robust booleans on NURBS-bounded solids**, made on the **exact-topology / tolerant-geometry hybrid** that (per file 11) no kernel fully ships. Two halves, built in order: (A) the first NURBS boolean working end-to-end on a curved NURBS solid; (B) **tolerant edges** -- the SSI curve's certified error bound (already computed as `SsiCurve.tol_achieved`, currently discarded) plumbed into `Edge.tolerance`, propagated through the boolean, with Qi/Shapiro epsilon-solidity as a *checkable* validity contract.

**Research basis (re-read 2026-06-08):** d-booleans-tolerant.md section 4 "Position: tolerant-modeling machinery required on day one" (tolerance field on vertices/edges; tolerant-edge geometry whose curve is approximate and whose tolerance bounds the deviation; propagation; tolerant stitch); c-surface-intersection.md (tier-2 analytic-spline gives a curve + a rigorous bound); b-nurbs-freeform.md (project for pcurves, surface tessellation). Existing: M5a SSI (tier-2/3, `tol_achieved`); M6b winding classifier + import-and-glue stitch; M6c two-arc periodic imprint; M7a NURBS tessellation + nurbs_sphere.

**Carried lessons:** soundness-gate any new winding input; run the EXACT CI triplet before merge; fuzz findings -> golden regressions; honest deferral ledger; decline-don't-lie.

**Branch:** `m7b-nurbs-booleans` off master.

---

## Part A: the first NURBS boolean

### Task 1: NURBS imprint pcurve
Generalize the M5b imprint (currently `face_analytic_surface` -> `Err` on NURBS). Add a NURBS path to `curve_pcurve_on` / the imprint preconditions: sample the SSI curve, `project_point_surface(nurbs, p, tol)` each sample to (u, v), verify on-surface (distance < tol), fit a pcurve through the (u, v, 0) points (the M5b sample-invert-fit, generalized). The crossing / interior-ring topology (M6b/M6c) is surface-agnostic; only the surface check + pcurve differ. Test: imprint the analytic-x-NURBS SSI circle onto a `nurbs_sphere` face -> valid split, the circle edge carries a pcurve.

### Task 2: trimmed-NURBS fragment tessellation + interior point
- `tessellate_nurbs` gains a trim filter: for a NURBS sphere cap bounded by a CLOSED SSI circle, keep grid triangles whose centre is on the cap side of that circle's plane (the NURBS analogue of `tessellate_sphere`'s cap-side filter; side from the fragment interior point / loop kind).
- `face_interior_point` dispatches NURBS faces to a NURBS version: the cap apex = `project_point_surface(nurbs, circle.center + axis * big * side)`, side from the boundary fin's loop kind (mirror the analytic sphere method).

### Task 3: the proof
**NURBS sphere INTERSECT analytic sphere** (both equatorially seamed -> crossing-free), distance 1.5: the SSI is the tier-2 analytic-vs-spline circle (exact for this pair); imprint both; classify (NURBS-aware GWN); select; stitch (import-and-glue); result validates; tessellated volume within ~5% of the exact lens formula. Analytic-vs-analytic sphere lens stays green. (This boolean's SSI is still EXACT -- it proves the NURBS plumbing end-to-end; the INEXACT case is Part B.)

## Part B: tolerant edges (the differentiation)

### Task 4: the tolerant-edge contract
- Carry the SSI bound onto topology: when an imprinted seam edge is created from an `SsiCurve`, set `Edge.tolerance = max(session_tol, ssi.tol_achieved)` (today it is the default floor). The edge's curve is the approximation; its tolerance bounds the deviation from the true intersection.
- Propagation: a vertex where tolerant edges meet takes `Vertex.tolerance = max` of incident edge tolerances + the gap closed at stitch. Document the propagation rule.
- `epsilon_solid(&self, eps) -> bool`: a checkable validity predicate -- every edge/vertex tolerance <= eps AND the boundary is gap-closed within eps (samples on each edge lie within the incident faces' tolerances). The boolean's post-condition asserts epsilon-solidity at the achieved tolerance.

### Task 5: an INEXACT NURBS x NURBS boolean -- DEFERRED to M7c (honest ledger)
Two `nurbs_sphere`s: the SSI is tier-3 spline-x-spline (marched + FIT, genuinely inexact, `tol_achieved` ~ 5e-4). **Attempted in M7b; deferred.** Two blockers found empirically (not theory):
1. **Correctness:** the tier-3 FITTED seam (a general NurbsCurve, not a clean Circle3) is not split onto either NURBS sphere by the crossing imprint -- the diagnostic showed `ia.f=1, ib.f=1` (neither face cut), so both whole faces classify `OutsideOther` and `kept=0`. The crossing/two-arc imprint (M6c) currently relies on clean-conic seam geometry (center/axis, antipode) to locate the seam crossing; it must be generalized to fitted NURBS seams. The tier-2 analytic-vs-NURBS case works because its seam, though carrying a nonzero fitted *bound*, is still produced as a clean circle the crossing logic can split.
2. **Performance:** one tier-3 NURBS-sphere x NURBS-sphere boolean runs ~190s (the certified solver re-decomposes both pole-degenerate revolved spheres into Bezier patches and Krawczyk-verifies every patch pair). Un-shippable as a default `cargo test` case; needs the SSI patch-pair pruning / caching perf work first.

**The centerpiece is NOT blocked by this.** The tolerant-edge contract ("exact topology decisions with tolerant geometry") is delivered and proven by Task 4 on the tier-2 analytic-sphere x NURBS-sphere lens, whose SSI is solved by certified-numeric **fitting** and so carries a GENUINELY nonzero certified bound (~4e-7) that now rides onto `Edge.tolerance` and gates `epsilon_solid`. Inexactness of the *geometry* is real there; M7c's NURBS x NURBS is a harder *second* demonstration (fitted-seam crossing imprint + SSI perf), not a prerequisite for the thesis.

### Task 6: fuzz + gate
- `fuzz_nurbs_boolean`: random NURBS-sphere x (analytic|NURBS)-sphere pairs; result validates/declines cleanly, is epsilon-solid when Ok, volume bounded, never panics.
- Exit gate: EXACT CI triplet; prior artifacts replay; 10-min soaks CLEAN; LOG addendum; merge; verify CI green.

## Deferred to M-later (honest ledger)
- General trimmed-NURBS faces (multiple/curved trim loops, non-sphere); exact trimmed-NURBS mass properties (replace the coarse tessellated oracle); tolerance GROWTH theory under chained booleans (open in the literature); differential testing vs OCCT over the ABC corpus; non-manifold / sheet booleans; healing of dirty imported NURBS.
