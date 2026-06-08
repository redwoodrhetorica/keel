# M6b: Winding-Number Classification + Curved Booleans Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace M6a's fragile interior-point + ray-cast PMC fragment classifier with **generalized winding-number classification** (Jacobson/Barill; the `d-booleans-tolerant.md` design mandate), then use it to unlock the **curved booleans M6a deferred** (sphere ∪/∩/− sphere). Winding numbers degrade gracefully at on-boundary/tangential/near-coincident contacts and are surface-type-agnostic (they work on periodic surfaces with no pcurve dependency), so they fix the exact pain points that pushed curved booleans out of M6a.

**Research basis (re-read 2026-06-07):** `docs/research/d-booleans-tolerant.md` §4 Design Impact — ADOPT "arrangement-then-classify with winding-number classification (Zhou/Jacobson)... the most robust and most composable classification strategy known"; AVOID "neighborhood-walk / ray-cast PMC as the sole classifier... fragile at on-boundary and tangential contacts." Generalized winding number: Jacobson et al. 2013 (robust inside-outside via solid angle), Barill et al. 2018 (fast hierarchical GWN), Spainhour & Weiss TOG 2025 (trimmed-NURBS containment, LLNL Axom) for the eventual M7 NURBS case. Solid angle of a triangle: Van Oosterom–Strackee. Spec D3/D5/D7.

**The core idea.** For a point `p` and an oriented closed boundary `S`, the generalized winding number `w(p) = (1/4π) ∮_S dΩ` (sum of signed solid angles) is ≈1 inside the solid, ≈0 outside, and varies smoothly across `S`. Tessellate each operand's boundary into oriented triangles; `w(p)` is the sum of the triangles' signed solid angles (Van Oosterom–Strackee). To classify a fragment of operand A against operand B, evaluate `w_B(q)` at a point `q` on the fragment: `w_B ≈ 1` → inside B, `≈ 0` → outside B. This replaces `other.classify_point(q)` and is robust where PMC was not.

**Carried lessons:** verify the winding number is SOUND before building classification on it (the M5a interval-soundness mandate, self-imposed here as Task 0); every fuzz finding becomes a golden regression test; honest deferral ledger; `if ($LASTEXITCODE -eq 0)` commit guards; debug-validate after each mutation.

**Branch:** `m6b-winding` off master.

---

## Scope boundary (binding)

1. **M6b delivers the winding-number classifier and sphere-sphere curved booleans.** Block−cylinder (mixed planar+periodic seams crossing the cylinder lateral seam), coplanar/coincident neighborhood handling, holed-face stitch, enclosed-void (3-region) stitch, tolerant edges, and NURBS-bounded solids are M6c/M7. The partial-success model already declines those cleanly.
2. **Winding number is the PRIMARY classifier; PMC stays only as a corroborating cross-check** (research AVOID note). The boolean's `classify_faces` switches to GWN.
3. **Tessellation is for classification only** (coarse, internal), not a user-facing facet product. It needs only enough fidelity for robust inside/outside of points well off the other solid's boundary; near-seam points are handled by sampling the fragment interior away from its seams.
4. **Curved trimmed mass properties** (spherical-cap volume) land here as the sphere-boolean volume oracle; if the exact integral proves heavy, a certified grid/Monte-Carlo volume with a stated bound is the honest fallback, with the exact integral staged.

---

## File structure

- Create: `crates/keel-topo/src/tessellate.rs` — coarse boundary tessellation of analytic faces into oriented triangles (for GWN).
- Create: `crates/keel-topo/src/winding.rs` — `generalized_winding_number(&Body, Vec3) -> f64` via summed triangle solid angles; the classification predicate.
- Modify: `crates/keel-topo/src/boolean.rs` — `classify_faces` uses GWN; curved fragment interior point; route sphere seams (already crossing-free) through the existing closed-curve imprint.
- Modify: `crates/keel-topo/src/massprops.rs` — trimmed spherical-cap contribution (divergence theorem over a trimmed curved face), or the staged fallback.
- Create: `fuzz/fuzz_targets/fuzz_winding.rs` — GWN never NaN; inside>0.5, outside<0.5 for primitive solids.
- Modify: `crates/keel-topo/src/lib.rs` — module declarations.

---

## Tasks

### Task 0 (MANDATE): winding-number soundness before building on it
Before any classification depends on it, prove the GWN is sound. `generalized_winding_number(body, p)`:
- For a watertight primitive solid (box, sphere, cylinder, cone, torus) tessellated into oriented outward triangles, `w(p)` must be within 1e-3 of **1.0** for points deep inside and within 1e-3 of **0.0** for points well outside, across a magnitude ladder of positions and sizes.
- Orientation audit: a consistently outward-oriented boundary gives `w≈+1` inside (not −1); this cross-checks M3's face-side conventions the way M4 mass properties did.
- Graceful degradation: as `p` approaches the boundary, `w` varies continuously through ≈0.5 (no jumps/NaN), sampled along a segment crossing a face.
- Determinism: same inputs → identical `w` bits (tessellation is deterministic; solid-angle sum order is stable).
Tests assert all four. THIS GATES the rest of M6b: if the winding number is not sound, classification built on it is worthless (the M5a lesson).

### Task 1: triangle solid angle + boundary tessellation (`tessellate.rs`, `winding.rs`)
- `tri_solid_angle(p, a, b, c) -> f64`: Van Oosterom–Strackee signed solid angle of triangle (a,b,c) seen from p: `2·atan2(det[a-p,b-p,c-p], |x||y||z| + (x·y)|z| + (y·z)|x| + (z·x)|y|)` with x=a−p etc. Returns 0 when p is on the triangle plane within the triangle.
- `tessellate_face(&Body, FaceKey) -> Vec<[Vec3;3]>`: oriented (outward, via face front-normal) triangles covering the face's trimmed region. Planar faces: fan/ear-trim over the loop polygon (reuse the UV polygon machinery). Analytic curved faces (sphere/cylinder/cone/torus): sample the surface on a UV grid clipped to the trim loops, emit two triangles per cell whose all-corners-inside the trim; boundary cells clipped coarsely (coarse is fine for GWN of off-surface points). Orientation from the face's `(surface, sense)`.
- `generalized_winding_number(&Body, Vec3) -> f64`: `(1/4π)·Σ tri_solid_angle` over every face's tessellation.
Tests: a single tessellated sphere gives w≈1 at center, w≈0 outside; sum is orientation-correct.

### Task 2: curved fragment interior point
`face_interior_point` currently needs planar pcurves. Add `face_interior_point_3d(&Body, FaceKey) -> Option<Vec3>`: tessellate the fragment face (Task 1), return the centroid of the largest triangle (guaranteed on the trimmed face, away from the boundary). This removes the pcurve/periodicity dependency that blocked sphere-cap classification. Planar faces keep the existing path; curved faces use this. Test: interior point of a sphere cap lies on the sphere within the cap.

### Task 3: switch the boolean classifier to GWN
In `classify_faces`, replace `other.classify_point(q)` with: `let w = generalized_winding_number(other, q); if w > 0.5 { InsideOther } else { OutsideOther }`, with a coincidence band `|w−0.5| < band` (and `q` within tol of an `other` face) → `OnOther` (deferred). Keep `classify_point` available as a corroborating cross-check in debug asserts only. RE-VERIFY: every M6a box test (guillotine, corner-overlap, nested, commutativity, translation, determinism, near-degenerate) stays green — the classifier swap must not regress the planar proofs.

### Task 4: sphere-sphere boolean (the curved proof)
With robust classification, the sphere SSI + crossing-free imprint (already M6a unit tests) compose into a full boolean. Two unit spheres at distance 1.5 (seamed equatorially so the SSI circle is crossing-free, per M6a):
- `imprint_pair` splits each sphere into two caps along the SSI circle (works today).
- `classify_faces` (GWN) labels each cap inside/outside the other sphere (now robust).
- `select_faces` + a curved-capable `build_result_solid` (the stitch must accept curved faces: it already attaches arbitrary `SurfaceGeom`; the seam edge is the shared SSI circle, glued radial-2).
- Result validates; PMC/GWN of a probe point is consistent with set membership.

### Task 5: trimmed spherical-cap mass properties + volume proof
Extend `massprops` to integrate a trimmed spherical cap (the divergence-theorem surface integrand over the cap's trimmed region; reuse the Task-1 tessellation as the integration mesh with a stated discretization bound, or the exact cap formula for the analytic case). PROOF: sphere∪sphere = `2·V_sphere − V_lens`, sphere∩sphere = `V_lens`, sphere−sphere = `V_sphere − V_lens`, each to a stated tolerance against the exact lens formula `V_lens = (π h²/3)(3r−h)` summed for both caps. State the achieved tolerance honestly (exact if the analytic cap integral lands; ~1e-3 relative if the tessellated oracle is used, with the exact integral staged).

### Task 6: fuzz + gate
- `fuzz_winding`: random primitive solids + random probe points; GWN is finite, in [−ε, 1+ε], and inside-points > 0.5, far-outside-points < 0.5; never NaN.
- Extend `fuzz_boolean` with sphere pairs (random radii/offsets); results validate or decline cleanly, volume bounds hold, never panic.
- Exit gate: fmt, clippy, workspace tests, all prior artifacts replay, 10-min fuzz_winding + fuzz_boolean soaks CLEAN, LOG addendum, merge.

## Deferred to M6c / M7 (honest ledger)
- Block−cylinder and general analytic mixed-surface booleans (cylinder-lateral seam crossings).
- Coplanar/coincident faces via the full winding-number-VECTOR model + neighborhood for exact coincidence (M6b handles single-operand classification; n-ary coincidence is M6c).
- Holed-face stitch (union of overlapping boxes) and enclosed-void 3-region stitch.
- Tolerant edges/vertices with propagated per-entity tolerance (Jackson) — required for M7 NURBS booleans, where SSI curves are inexact.
- NURBS-bounded-solid booleans (M7 proof bar): tier-2/3 SSI seams + tolerant stitch + Barill hierarchical GWN for speed.
- Hierarchical/BVH-accelerated GWN and AABB face-pair localization (perf).
