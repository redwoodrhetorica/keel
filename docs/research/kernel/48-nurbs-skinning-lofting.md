# NURBS Skinning / Lofting with Cross-Section Continuity

**Scope.** This dossier is the dedicated, implementation-grade treatment of **smooth NURBS skinning / lofting** for the Keel B-rep kernel: building one tensor-product NURBS surface `S(u,v)` that interpolates (or tolerance-approximates) a sequence of `K+1` section curves `C_0(u) ... C_K(u)`, with controlled continuity at the end sections and against guide rails, with proper handling of incompatible sections, closed/periodic sections, and the degree-1 (ruled) special case. It directly serves roadmap items **66 (smooth loft), 67 (lofting with continuity/rails), 69 (ruled surface)**. It fills the gap explicitly deferred by `kernel/26-transfinite-nsided-surfacing.md` ("lofting/skinning deferred to separate lofting research").

This is a **design recommendation to validate against Keel's test suite**, not a description of shipped code. Keel currently has `loft_sections` (planar polygon sections, faceted side quads, not smooth) and translational `sweep_along_path`; the smooth, continuity-controlled skin needs real NURBS surfaces, not facets. The recommendation is validated at the end against a 3-section circle-to-square skin with tangent ends.

**Build on, do not repeat, the sibling dossiers.** This file assumes and cites:
- `nurbs/03-fitting-interpolation.md` Sec 5.2: the lofting/skinning knot-merge explosion and **Park-style common-knot reduction**; the banded global-interpolation solver (Sec 1.1) and end conditions (Sec 1.2) are reused verbatim here as the v-direction solver.
- `nurbs/05-offsets-blends-sweeps.md` Sec 7: the skinning survey, approximate skinning, and the RMF (double-reflection) frame engine reused for rails/sweeps.
- `kernel/26-transfinite-nsided-surfacing.md` Sec 2: Gordon surfaces (lofting is the degenerate single-family Gordon case) and Coons Boolean-sum skeleton.
- `kernel/32-variational-surface-design.md`: the thin-plate fairness functional `x^T K x` used to stabilize the common-knot refit.
- `kernel/33-constrained-surfacing-continuity.md` Sec 5: **continuity as pinned control-point rows** (G0 pins row 0, G1 pins row 1, G2 pins rows 0,1,2). The end-condition and rail constraints in this dossier are emitted as exactly those pinned-row blocks.

**Notation.** Section curves `C_k(u) = sum_i N_{i,p}(u) P_{i,k}`, `k = 0..K`, degree `p`, common u-knot vector `U`, `n+1` control points each after compatibility. Section parameters in `v`: `v_0 ... v_K`. Final surface `S(u,v) = sum_{i=0}^{n} sum_{l=0}^{K} N_{i,p}(u) N_{l,q}(v) Q_{i,l}` of degree `(p,q)` over knot vectors `(U, V)`. Rational case carries weights `w`; the inner solves fix weights and work on homogeneous coordinates `(w x, w y, w z, w)`.

---

## 1. The B-spline skinning algorithm (item 66)

The classic Piegl-Tiller global skinning pipeline (The NURBS Book, Sec 10.3) has four stages: **(A) compatibility**, **(B) v-parameterization**, **(C) v-knots**, **(D) the per-column lofting solve**. Stages A and the knot-merge blowup are the hard part; the rest is the tensor-product interpolation of `nurbs/03` Sec 5.1 applied once across sections.

### 1.1 Stage A: make the K+1 sections compatible (common degree, common knot vector)

A tensor-product surface requires every section to be a B-spline of the **same degree** over the **same knot vector**. Input sections generally differ in both. The exact procedure:

1. **Common degree by elevation.** Let `p = max_k degree(C_k)`. Degree-elevate every lower-degree section to `p` (Piegl-Tiller The NURBS Book Sec 5.5, algorithm A5.9 `DegreeElevateCurve`). Degree elevation is exact: the curve is unchanged, only its representation grows (a degree-`d` Bezier segment with `s` interior knots of multiplicity gains control points predictably). Keel already has degree elevation in its NURBS curve layer; this stage reuses it.

2. **Common knot vector by union merge.** Form `U = union_k U_k` (the merged knot vector: every distinct knot from every section, each at the **maximum multiplicity** it attains in any section). Then for each section, insert the knots it is missing via Boehm knot insertion (A5.4 `RefineKnotVectCurve`) so it is re-expressed over `U`. Knot insertion is exact: `C_k` is unchanged, its control-point count rises to `n+1 = |U| - p - 1`.

After stage A all sections share `(p, U)` and have `n+1` control points `P_{i,k}`, `i = 0..n`. The control-point **rows** are now in correspondence: column `i` across all sections is one v-direction data sequence.

**The union-merge blowup.** The control-point count `n+1` after merging is the union of all sections' knots. For `K+1` sections with mostly-distinct knot layouts this **grows additively**: a loft of many sections, each fit independently to tolerance, can produce a surface with hundreds of u-control-points even when each section needed only a handful. This is the central practical defect of naive skinning (`nurbs/03` Sec 5.2, `nurbs/05` Sec 7.1-7.2). The mitigation is Stage A', below.

### 1.2 Stage A' (mitigation): Park common-knot reduction instead of union merge

Rather than the exact union, choose a **single common knot vector `W` with as few knots as possible** that still admits an interpolant of every section's data, then **refit** each section onto `W` (approximate, not exact). This is Park's linearly-constrained energy minimization (Park 2003; Park-Lee 2008 common-knot determination), recommended in `nurbs/03` Sec 5.2 and reproduced here as the Keel default:

1. **Determine `W`.** Take the section with the most data points as the anchor; build its averaged knot vector (`nurbs/03` Sec 3.1). For each remaining section, extract a compatible knot vector by the `ConstructKnot` procedure over flexible intervals `[a_i, b_i] = [u_i^p - alpha_i (u_i^p - u_{i-1}^p), u_i^p + beta_i (u_{i+1}^p - u_i^p)]` (a `per in [0,1]` flexibility parameter widens the window), and merge into `W`. The merge keeps `|W|` small by coalescing near-equal knots within tolerance.

2. **Existence guarantee.** `W` is admissible iff a knot subset satisfies Schoenberg-Whitney `u_i < t_i < u_{i+p+1}` for every section's parameters (Park-Lee Theorem 1). This is the same solvability check `nurbs/03` Sec 1.3 mandates everywhere; it guarantees each section's interpolation matrix on `W` is full rank.

3. **Refit each section by constrained energy minimization.** Fit `C_k` on `W` to interpolate its data while minimizing the curve bending+stretching energy

   ```
   E(C) = integral ( alpha |C'(t)|^2 + beta |C''(t)|^2 ) dt ,   alpha = 1.0, beta = 0.2
   ```

   Because `E` is quadratic in the control points, the constrained minimization is a **single linear (KKT) system**: minimize `P^T K P` subject to the interpolation equations `A P = Q_k`, where `K` is the energy stiffness matrix (the same `K` as `kernel/32` thin-plate, here in 1D) and `A` is the banded collocation matrix. This stabilizes an otherwise under-determined refit (more control points than constraints) and produces fair sections.

**Result.** Reported control-point reduction of up to **99%** versus naive union merge at a chosen tolerance (Piegl-Tiller approximate skinning; Park common-knot determination), while staying within `eps` of every section. **Keel must default to Park common-knot reduction, not union merge**, or every loft bloats.

### 1.3 Stage B: section reparameterization so isocurves align

The surface's u-isocurves at the section parameters are exactly the sections; but at intermediate `v` the isocurve is the v-interpolation of corresponding control points. For this to be geometrically sensible, **corresponding parameters across sections must denote corresponding features**: `C_0(u*)` and `C_1(u*)` should be points the loft ought to connect. If the sections were fit with independent parameterizations (one chord-length on a long section, another on a short one), the same `u*` lands at non-corresponding places and the loft **twists/wrinkles**.

Fix: reparameterize all sections to a **consistent parameterization** (centripetal default, `nurbs/03` Sec 2.1), or, when sections are non-parallel/dissimilar (a circle vs a square), establish an explicit feature correspondence (align the seam/start, distribute parameters by matched arc-fraction or by **radial alignment** of feature points; ship-hull skinning literature, Shamsuddin-Ahmed 2005). Universal parameterization (`nurbs/03` Sec 2.3, Lim) is the tool when independently-fit sections must be made compatible without forcing a feature match. This is the single biggest source of ugly lofts and must be exposed as an inspectable, switchable option.

### 1.4 Stage C: v-parameterization and v-knot vector

Assign each section a v-parameter `v_k`. Per control-point column `i`, the v-data are `P_{i,0} ... P_{i,K}`. Piegl-Tiller assign `v_k` by **averaging the chord-length (or centripetal) parameterizations of all columns** so the whole surface shares one `V`:

```
v_0 = 0,  v_K = 1,
v_k = v_{k-1} + ( 1/(n+1) ) * sum_{i=0}^{n} |P_{i,k} - P_{i,k-1}| / L_i ,   L_i = sum_j |P_{i,j} - P_{i,j-1}|
```

(the averaged per-column chord lengths). Then build `V` by the averaging knot rule (`nurbs/03` Sec 3.1, Eq 9.8) from the `v_k`, with v-degree `q` (typically `q = min(3, K)`; `q = 1` gives ruled, Sec 4). This guarantees Schoenberg-Whitney for the v-solve.

### 1.5 Stage D: the v-direction lofting linear solve

With `(p, U)` fixed and `(q, V)` chosen, the surface control points `Q_{i,l}` are found by **interpolating each control-point column across the sections in v**. For a fixed column `i`, solve the global interpolation system (`nurbs/03` Sec 1.1):

```
P_{i,k} = sum_{l=0}^{K} N_{l,q}(v_k) Q_{i,l} ,   k = 0..K     (one banded system per column i)
```

i.e. `A_v Q_i = P_i`, where `(A_v)_{k,l} = N_{l,q}(v_k)` is the **same banded matrix for every column** (bandwidth `2q+1`, totally positive, nonsingular by Schoenberg-Whitney). Factor `A_v` **once** (banded LU, no pivoting), then back-substitute for all `n+1` columns and all 3 (or 4 homogeneous) coordinates. Cost: one `O(K (2q+1))` factorization plus `(n+1)*3` cheap solves. The result `Q_{i,l}` is the control net of

```
S(u,v) = sum_{i=0}^{n} sum_{l=0}^{K} N_{i,p}(u) N_{l,q}(v) Q_{i,l}
```

which **interpolates every section**: `S(u, v_k) = C_k(u)` exactly. Continuity `C^{p-1}` in u, `C^{q-1}` in v internally.

### 1.6 Full pipeline summary (K+1 sections -> one NURBS surface)

```
1. degree-elevate all C_k to p = max degree                         (exact)
2. choose common u-knots W (Park reduction, default) OR U = union   (A' vs A)
3. refit/insert each C_k onto W/U                                    (approx / exact)
   -> all sections share (p, W), n+1 control points P_{i,k}
4. reparameterize sections for feature correspondence (Stage B)
5. assign v_k (averaged chord/centripetal), build V, choose q        (Stage C)
6. per column i: solve A_v Q_i = P_i  (one banded factor, reused)    (Stage D)
   -> surface S(u,v) of degree (p,q) over (W,V), control net Q_{i,l}
```

Stages 1, 3, 6 reuse existing Keel primitives (degree elevation, knot insertion, banded interpolation). Stages 2 and 4 are the new code: common-knot reduction and feature-correspondence parameterization.

---

## 2. Continuity: G1/G2 ends, guide rails, and pinned control rows (item 67)

### 2.1 Tangent (G1) and curvature (G2) end conditions at the first/last section

To make the skin leave `C_0` and arrive at `C_K` with a prescribed cross-boundary tangent (or curvature), add v-derivative constraints to the v-solve. This is the surface analogue of the curve end conditions in `nurbs/03` Sec 1.2, applied per column.

**G1 ends.** Prescribe `S_v(u, 0) = D_0(u)` and `S_v(u, 1) = D_K(u)`, where `D_0, D_K` are cross-boundary tangent fields (e.g. perpendicular to the section plane for a "natural exit", or the tangent ribbon of an adjacent face). Per column `i`, augment the v-interpolation with the two derivative rows (degree elevates the v-solve to add two control rows, `n = K+2`):

```
A_v Q_i = [ C'(v_0) end-deriv ; P_{i,0} ; ... ; P_{i,K} ; C'(v_K) end-deriv ]
```

In **pinned-row terms** (`kernel/33` Sec 5): the first cross-boundary derivative is `S_v(u,0) = (q / v_{q+1}) (Q_{i,1} - Q_{i,0})`, so prescribing `D_0` **pins row 1**:

```
Q_{i,1} = Q_{i,0} + (v_{q+1} / q) * D0_i ,   Q_{i,0} = P_{i,0}  (row 0 pinned to section C_0)
```

Geometrically `Q_{i,0}, Q_{i,1}` and the target are collinear: the Class-A "collinear tangent CV" rule.

**G2 ends.** Prescribe also `S_vv(u,0)`. The second cross-boundary derivative fixes a linear combination of rows 0,1,2; with rows 0,1 pinned this **pins row 2** (`kernel/33` Sec 5.2). So a G2 end consumes **three v-control-rows** at that end; G2 at both ends consumes rows `0,1,2` at `v=0` and `K,K-1,K-2` at `v=1`, requiring `q` and the section count large enough that the two pinned blocks do not overlap. This is the Alias DOF rule: **degree 5 (here `q`) for G2, degree 7 for G3, insert extra v-spans if blocks collide** (`kernel/33` Sec 10). Each pinned row is one linear block fed to the constrained-LS solve of `kernel/33` Sec 6; the interior v-rows remain free and are set by the section interpolation plus the fairness term.

### 2.2 Guide rails: the Gordon-surface direction

A **rail** (guide curve) is a curve the surface must follow in the **v-direction**, threading the sections. Skinning interpolates a 1-parameter family of sections; adding rails that interpolate the **cross (v) family** turns the construction into a **Gordon surface** (`kernel/26` Sec 2): a surface interpolating both a family of u-curves (the sections) and a family of v-curves (the rails). Lofting is the degenerate Gordon case with **no** v-family; adding rails restores the second family.

**Gordon Boolean-sum (curve-network interpolation).** Given sections `C_k(u)` at `v_k` and rails `R_j(v)` at `u_j`:

```
S(u,v) =  L_u(u,v)  +  L_v(u,v)  -  T(u,v)
L_u = sum_k C_k(u) A_k(v)         (loft through the sections, v-blends A_k cardinal at v_k)
L_v = sum_j R_j(v) B_j(u)         (loft through the rails, u-blends B_j cardinal at u_j)
T   = sum_k sum_j G_{jk} A_k(v) B_j(u)   (tensor interpolant of the grid points G_{jk}=C_k(u_j)=R_j(v_k))
```

The correction `T` removes the doubly-counted network-intersection points. **Compatibility requirement** (the hard part, `kernel/26` Sec 2): every section must intersect every rail, and section `k` must meet rail `j` at the consistent parameter pair `(u_j, v_k)` for both, i.e. `C_k(u_j) = R_j(v_k) = G_{jk}`. If the input rails do not actually pass through the sections at common parameters they must be reparameterized/refit first (radial/feature alignment as in Sec 1.3). The result converts cleanly to a single NURBS surface (Lin-Hewitt, `kernel/26` Sec 7.1): degree-elevate and knot-merge the three terms onto common `(U,V)` and add the control nets.

**How rails constrain the v-family.** Without rails, `S(u_j, v)` (a v-isocurve) is whatever the v-interpolation of column `j` produces. A rail **pins that isocurve to `R_j(v)`**: the rail's control points become hard interpolation constraints on the v-solve along column `j`. In `kernel/33` constrained-LS terms, each rail contributes a block of exact interpolation equations `S(u_j, v_l) = R_j(...)` (or, sampled at Greville v-parameters, pinned-row equations on the columns the rail passes through). One-rail and two-rail (birail) sweeps are the low-count special cases: a **birail** sweeps a profile so its two ends ride two rails, equivalent to a 2-section Gordon-ish surface where the rails are the v-family and the profile(s) are the u-family, oriented by a rotation-minimizing frame (`nurbs/05` Sec 5.1, double-reflection RMF) to avoid spurious twist.

### 2.3 Tie to the pinned-control-row formulation (kernel/33)

Every continuity request in this dossier reduces to a **pinned control-row block** in the master constrained-LS solve of `kernel/33` Sec 6 (`minimize x^T H x s.t. C x = e`, `H = A^T W A + lambda_f K`):
- **Section interpolation** -> the data term `A x = d` (or hard rows).
- **G0 to an adjacent face at an end** -> pin row 0 (= the shared boundary curve).
- **G1 end / tangent to neighbour** -> pin row 1 (collinear-CV).
- **G2 end / curvature to neighbour** -> pin rows 0,1,2.
- **Each rail** -> a block of exact v-isocurve interpolation equations.
- **Fairness `K`** (thin-plate, `kernel/32`) sets the free interior rows and bridges any gap/slack.

So Keel does **not** need a separate "loft solver" and "fill solver": skinning with continuity is the constrained-LS engine of `kernel/33` with section data + end-pinned-rows + optional rail blocks. The pure-interpolation banded solve of Sec 1.5 is the fast path when there are no continuity constraints and the sections are clean.

---

## 3. Sections with differing vertex/knot counts: tolerance-bounded refit (item 67)

Real sections (imported, sketched, or output of prior features) have **different control-point counts and knot vectors**. Two routes:

**Exact compatibility (union).** Degree-elevate to common `p`, union-merge knots (Sec 1.1). Exact (every section interpolated) but explodes `n`. Use only when sections are few and already near-compatible.

**Tolerance-bounded refit (default).** Choose the small common `W` (Park, Sec 1.2) and **reapproximate** each section onto `W` within tolerance `eps`:
1. Knot-refine `W` only where some section needs resolution (the flexible-interval `ConstructKnot` of Sec 1.2 adds knots only where a section's data demands it, by Schoenberg-Whitney).
2. Least-squares (or constrained-energy) refit each section on `W`; measure max deviation `e_k = max_u |C_k(u) - C_k^W(u)|`.
3. While `e_k > eps`: insert one knot into `W` in the worst span (knot insertion is local, `nurbs/03` Sec 4.2 refinement loop) and refit; this adds DOF to **all** sections (tensor-product knot lines are global) but only where truly needed.
4. Stop when every section is within `eps`. The shared `W` is now as small as the tolerance allows.

This is the surface analogue of the error-bounded knot-refinement loop (`nurbs/03` Sec 4.2) and dominant-column fitting (`nurbs/03` Sec 5.4). The key discipline (`nurbs/05` Sec 12): **the approximation is a cache, keep the procedural source** (the original sections + tolerance) so the loft can be re-baked finer for downstream intersection/export. After lofting, a Lyche-Morken bounded **knot-removal** pass (`nurbs/03` Sec 7.2) compacts any residual over-resolution.

---

## 4. Ruled surfaces (item 69), closed/periodic sections, and the seam problem

### 4.1 Ruled surface as the degree-1 linear-v special case

A ruled surface between two curves is **exactly the skinning pipeline with `q = 1` and `K = 1`** (two sections, linear v). Make `C_0, C_1` compatible (Sec 1.1: common `p`, common `U`), then

```
S(u,v) = (1 - v) C_0(u) + v C_1(u) = sum_i N_{i,p}(u) [ (1-v) P_{i,0} + v P_{i,1} ]
```

which is `S(u,v) = sum_i sum_{l in {0,1}} N_{i,p}(u) N_{l,1}(v) Q_{i,l}` with `Q_{i,0} = P_{i,0}`, `Q_{i,1} = P_{i,1}`, `V = {0,0,1,1}` (degree-1, no interior v-knots). **Exact NURBS, no tolerance** (`nurbs/05` Sec 8). The only required work is curve compatibility; the v-solve is trivial (identity). Item 69 is therefore the smallest member of the same engine, not a separate feature. A multi-section "ruled-between-each-pair" loft is `q = 1, K > 1` (piecewise-linear in v, `C^0` across sections), the un-smoothed analogue of `loft_sections`.

### 4.2 Closed / periodic sections and the seam alignment problem

A **closed** section (start = end control point, `C^0` at the seam) or a **periodic** section (the knot vector wraps so the curve is `C^{p-1}` across the seam) lofts into a surface that is **closed/periodic in u**. Keel must:

1. **Use periodic u-knots and wrap the control net.** A periodic B-spline repeats the first `p` control points at the end (`P_{n+1} = P_0`, etc.) and uses a periodic knot vector; the surface inherits `C^{p-1}` across the u-seam automatically. Convert a merely-closed (`C^0`) section to periodic if smoothness across the seam is wanted (e.g. a smooth tube), or keep it `C^0` (a hard edge along the seam) if a true corner is intended.

2. **Solve the seam-alignment / twist problem.** When lofting **dissimilar** closed sections (the canonical **circle to square**), the parameterizations and seam locations generally do not correspond. If the circle's `u = 0` seam maps to a corner of the square but the user wants it to map to a mid-edge, the loft **twists** by up to a quarter turn and pinches. The fix is the Sec 1.3 feature correspondence specialized to closed curves:
   - **Align seams**: rotate one section's parameter origin so `C_0(0)` and `C_1(0)` are corresponding features (e.g. both at "12 o'clock").
   - **Minimize twist**: choose the relative parameter offset `theta` that minimizes total swept distance `sum_i |P_{i,0} - P_{i,1}(theta)|^2` (radial alignment, ship-hull literature). This is a 1D search over the seam offset.
   - **Match feature counts**: a circle (smooth) and a square (4 corners) have different natural feature parameters. Refit both onto a common periodic `W` with knots placed at the union of both feature sets (the square's 4 corners become 4 knots of raised multiplicity so the loft develops the corners as it approaches `C_K`), so corresponding `u` denotes corresponding fraction-of-perimeter.

3. **Hohmeyer-Barsky smoothness caveat (rational sections).** When the sections are **rational** B-splines (a circle is rational degree-2), naive skinning can introduce **unwanted discontinuities** in the surface even though each section is smooth, because the weights do not interpolate consistently across v. Hohmeyer-Barsky (1991) derive the conditions under which rational-curve skinning stays smooth and the modifications needed; the practical rule is to **loft in homogeneous (wx, wy, wz, w) space** and keep the weight function smooth across sections (or convert the circle to an **integral** spline approximation first, `nurbs/03` Sec 8.2, dropping rationality where it is a tax).

---

## 5. Degree and knot growth control; tolerance-bounded approximation

The two growth axes and their controls:

**Degree growth.** Common degree `p = max_k degree(C_k)` (u) and chosen `q` (v). Keep `q` as low as the continuity demands: `q = 1` ruled, `q = 3` for G1-capable smooth loft, `q = 5` only if **G2 at both ends** is required (the two 3-row pinned blocks need `q >= 5` to not collide, Sec 2.1 / `kernel/33` Sec 10). Do **not** raise `p` unnecessarily: a single high-degree section forces all sections up; if one section is degree-5 only because of a local feature, consider degree-reducing it first (`nurbs/03` Sec 9.1) within tolerance.

**Knot growth.** The union merge is the worst case; **Park common-knot reduction (Sec 1.2) is the primary control**, trading exact interpolation for a tolerance-bounded refit on a small shared `W`. Secondary controls:
- **Knot coalescing**: merge knots within `tol` during the union to avoid near-zero spans (a reusable tolerant-merge primitive shared with Boolean edge reconciliation, `nurbs/03` Sec 5.2).
- **Post-loft knot removal** (Lyche-Morken bounded, `nurbs/03` Sec 7.2) in both u and v, removing every knot whose removal keeps the surface within `eps` of the sections.
- **Tolerance budget**: split the global `eps` between the section-refit error and the v-interpolation error so the two stages do not each consume the whole budget.

**When exact compatibility would explode:** always prefer tolerance-bounded approximation. The recurring lesson (`nurbs/05` Sec 12, Piegl-Tiller approximate skinning): up to **99% of control points** can be eliminated versus union merge with a deviation tolerance the user controls, and the procedural source is retained so fidelity is recoverable.

---

## Per-source entries

**S1. Piegl, L., & Tiller, W. (1997). The NURBS Book (2nd ed.), Sec 10.3 "Skinned Surfaces"; Ch. 5 (degree elevation A5.9, knot insertion A5.4); Sec 9.2 (global interpolation).** Springer. . *Content.* The canonical global skinning algorithm: make sections compatible by degree elevation to `p = max degree` and knot-vector **union merge**, then interpolate corresponding control points across sections with a v-direction banded solve `A_v Q_i = P_i`, yielding `S(u,v) = sum sum N_{i,p}(u) N_{l,q}(v) Q_{i,l}` interpolating every section. Defines the averaging v-parameterization and the union-merge blowup. . *Kernel relevance.* The skeleton of Keel's loft (Sec 1 here). The banded v-solve and degree-elevation/knot-insertion primitives already exist in Keel; the algorithm is their composition. Union merge is the baseline that Park (S4) replaces as default. [The NURBS Book](https://books.google.com/books/about/The_NURBS_Book.html?id=7dqY5dyAwWkC)

**S2. Piegl, L. A., & Tiller, W. (2002). Surface skinning revisited. The Visual Computer, 18(4), 273-283.** . *Content.* Reworks skinning to reduce control points and improve fairness, treating section compatibility and the v-knot choice as the levers; open lofted interpolation gives the v-knots freedom so each section's row is interpolated with as few new knots as possible. . *Kernel relevance.* Confirms the "don't union-merge naively" stance and motivates the open/flexible v-knot strategy used in Stage C. [Semantic Scholar](https://www.semanticscholar.org/paper/Surface-skinning-revisited-Piegl-Tiller/def7d28fc718a7757b5fae4434ba5ef8f8730064)

**S3. Piegl, L., & Tiller, W. (1995). Algorithm for approximate NURBS skinning. Computer-Aided Design, 27(... ), S0010448595000844.** . *Content.* Approximate skinning: make a set of NURBS curves of various types compatible by **simultaneous refitting on a common knot vector** with parametrically regular transition across curves, reducing the interior-knot count for a compact result; then approximately fit a smooth surface to any tolerance. Reports up to **99% control-point elimination** depending on tolerance. . *Kernel relevance.* The direct justification for Keel defaulting to tolerance-bounded refit over exact union merge (Sec 3, Sec 5). [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/0010448595000844)

**S4. Park, H. (2003). Lofted B-spline surface interpolation by linearly constrained energy minimization. Computer-Aided Design, 35(14), 1331-1337.** . *Content.* Finds a common knot vector with **fewer** knots than the union, then computes compatible section curves by **linearly constrained energy minimization** (interpolate the data subject to lying on the common knot vector while minimizing `integral(alpha|C'|^2 + beta|C''|^2)`), a quadratic objective giving one linear KKT system; then lofts. . *Kernel relevance.* The recommended Stage A' (Sec 1.2). The energy term is the 1D version of the `kernel/32` thin-plate fairness, unifying the loft-refit with Keel's variational engine. [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S001044850300040X)

**S5. Park, H., & Lee, J.-H. (2008/2009). Reducing control points in lofted B-spline surface interpolation using common knot vector determination. Computer-Aided Design, S0010448508001607.** . *Content.* Two-step common-knot determination: anchor on the densest section's averaged knots, then per section extract a compatible knot vector over flexible intervals `[a_i,b_i] = [u_i^p - alpha_i(...), u_i^p + beta_i(...)]` with flexibility `per in [0,1]`, merging into a small `W`. Theorem 1: full-rank interpolation iff a Schoenberg-Whitney-admissible knot subset exists. Refit by constrained energy minimization (`E = integral(1.0|C'|^2 + 0.2|C''|^2)`). . *Kernel relevance.* The concrete `W`-determination algorithm and existence guarantee for Sec 1.2 / Sec 3. Handles sections with **differing point counts** directly (item 67). [Academia PDF](https://www.academia.edu/15919352/Reducing_control_points_in_lofted_B_spline_surface_interpolation_using_common_knot_vector_determination)

**S6. Woodward, C. D. (1988). Skinning techniques for interactive B-spline surface interpolation. Computer-Aided Design, 20(8), 441-451.** . *Content.* Interactive skinning with **projection/spine curve** control of the longitudinal (v) shape, using a **Boolean-sum** representation so the skinned surface's v-shape resembles a designer-supplied spine. . *Kernel relevance.* The foundational interactive-skinning paper; the spine-curve idea is a precursor to guide rails (Sec 2.2) and the Boolean-sum tie to Gordon (S9). Establishes longitudinal shape control beyond plain chord-length v-parameterization. [Semantic Scholar](https://www.semanticscholar.org/paper/Skinning-techniques-for-interactive-B-spline-Woodward/b32b80612a096a7bd1d0d5cb67d4e47d737f7a62)

**S7. Hohmeyer, M. E., & Barsky, B. A. (1991). Skinning rational B-spline curves to construct an interpolatory surface. CVGIP: Graphical Models and Image Processing, 53(6), 511-521.** . *Content.* Shows how skinning **rational** generator curves can introduce surface **discontinuities** even when each section is smooth, derives the smoothness conditions, and gives modifications to the skinning algorithm to avoid them. . *Kernel relevance.* The caveat for circle-to-square and any rational sections (Sec 4.2): loft in homogeneous coordinates and keep the weight function smooth across v, or convert rationals to integral approximations. Directly load-bearing for the validation case. [Semantic Scholar](https://www.semanticscholar.org/paper/Skinning-rational-B-spline-curves-to-construct-an-Hohmeyer-Barsky/7a269aef500e468fa53aa41c8bc0c75e656aa329)

**S8. Gordon, W. J. (1969). Spline-blended surface interpolation through curve networks. J. Math. Mech., 18(10), 931-952; (1971) SIAM J. Numer. Anal., 8(1), 158-177.** . *Content.* Boolean-sum interpolation of a **network** of u-curves and v-curves: `S = L_u + L_v - T` with cardinal blends; lofting is the degenerate single-family case (drop `L_v` and `T`). Requires a compatible grid: every u-curve meets every v-curve at consistent parameters. . *Kernel relevance.* The formal basis for **guide rails** (Sec 2.2): adding rails to a loft is restoring Gordon's v-family. Lofting and rail-lofting are one engine. [via kernel/26 Sec 2] [Sverchok Gordon](https://nortikin.github.io/sverchok/docs/nodes/surface/gordon_surface.html)

**S9. Lin, F., & Hewitt, W. T. (1994). Expressing Coons-Gordon surfaces as NURBS. Computer-Aided Design, 26(2), 145-155.** . *Content.* Converts the Coons-Gordon Boolean sum into a single tensor-product NURBS by degree-elevating and knot-merging the three terms and adding control nets. . *Kernel relevance.* The mechanism that makes the rail/Gordon construction (Sec 2.2) produce a storable NURBS face rather than a procedural blend. [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/0010448594900353)

**S10. Tai, C.-L., et al. / Piegl-Tiller (1999). A method for approximate NURBS curve compatibility based on multiple curve refitting. Computer-Aided Design, S0010448599000883.** . *Content.* Makes a set of NURBS curves compatible by simultaneously refitting them on a common knot vector (approximate), the compatibility step that precedes skinning, avoiding the exact union's blowup. . *Kernel relevance.* The curve-compatibility primitive underlying Stage A'/Sec 3; shared with any operation needing co-defined curves. [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0010448599000883)

**S11. "Optimal lofted B-spline surface interpolation" (2022). arXiv:2202.06330.** . *Content.* Modern optimal-loft formulation: open lofted interpolation giving the knots freedom so each row interpolates with minimal added knots; common-knot strategy with energy stabilization; efficient data reduction with visually pleasing surfaces, usable for skinning and surface-from-mesh-of-curves. . *Kernel relevance.* Up-to-date corroboration of the Park-style default and the flexible v-knot strategy (Sec 1.4, Sec 5). [arXiv PDF](https://arxiv.org/pdf/2202.06330)

**S12. Shamsuddin, S. M., Ahmed, M. A. (2005). NURBS skinning surface for ship hull design based on new parameterization method. Int. J. Adv. Manuf. Technol.** . *Content.* Skinning **non-parallel, dissimilar** sections (ship hull) with a parameterization that minimizes control points and uses **radial vector alignment** across sections to fix correspondence and reduce twist; minimum-strain-energy smoothing. . *Kernel relevance.* The feature-correspondence/seam-alignment method for Sec 1.3 and Sec 4.2 (the twist fix), exactly the circle-to-square dissimilar-section problem. [Springer](https://link.springer.com/article/10.1007/s00170-004-2454-3)

**S13. Wang, W., Juttler, B., Zheng, D., & Liu, Y. (2008). Computation of rotation minimizing frames. ACM TOG, 27(1), Art. 2.** . *Content.* Double-reflection RMF, O(h^4), the minimally-twisted frame along a curve (full treatment in `nurbs/05` Sec 5.1). . *Kernel relevance.* Orients the profile along rails in birail/one-rail sweeps (Sec 2.2) so the swept v-family has no spurious roll; the frame engine is shared with `sweep_along_path`. [ACM TOG](https://doi.org/10.1145/1330511.1330513)

**S14. Autodesk Maya / MayaLT (2015). Sweep one or more profile curves along two path curves (birail).** . *Content.* Birail: sweep profile(s) so the ends ride two rail curves; multiple profiles are interpolated (skinned) between specified rail-parameter stations; optional auto-rotation of the profile. . *Kernel relevance.* The user-facing shape of one-rail/two-rail lofting (Sec 2.2): rails as the v-family, profiles as the u-family, RMF-oriented. Confirms item-67 rail semantics. [Autodesk birail](https://help.autodesk.com/cloudhelp/2015/ENU/MayaLT/files/Creating_NURBS_surfaces_Sweep_one_or_more_profile_curves_along_two_path_curves_birail.htm)

**S15. Sverchok (Nortikin). NURBS Loft / Gordon Surface / NURBS Birail nodes.** . *Content.* Concrete open-source NURBS loft: unify curves (insert each curve's missing knots into the others, or average if counts match), v-parameterize by Euclidean/centripetal/etc. distance, interpolate control points across sections; Gordon node requires an exactly-intersecting NxM grid with even parameter spacing and non-rational curves, with reparameterization fallbacks. . *Kernel relevance.* A working reference implementation matching this dossier's pipeline (Sec 1.1, 1.4) and Gordon constraints (Sec 2.2); a cross-check oracle for Keel's loft tests. [Sverchok Loft](https://nortikin.github.io/sverchok/docs/nodes/surface/nurbs_loft.html), [Sverchok Birail](https://nortikin.github.io/sverchok/docs/nodes/surface/nurbs_birail.html)

**S16. Maya NURBS overview: Periodic, closed, and open geometry; Grasshopper "Lofting periodic and closed curves".** . *Content.* Periodic curve = looped with overlapping wrap-spans giving `C^2` across the seam; closed curve = coincident start/end edit points (`C^0` seam). Lofting periodic/closed curves needs consistent seam direction and parameter origin across sections or the loft twists. . *Kernel relevance.* The periodic-vs-closed distinction and the seam-origin requirement for Sec 4.2 (circle-to-square periodic u). [Maya periodic/closed](https://download.autodesk.com/global/docs/maya2014/en_us/files/NURBS_overview_Periodic_closed_and_open_geometry.htm)

**S17. Park, H., & Lee, J.-H. (2007). B-spline curve fitting based on adaptive curve refinement using dominant points. Computer-Aided Design, 39(6), 439-451.** (and Surface dominant-columns, CAD 2010.) . *Content.* Error-bounded adaptive knot placement by dominant-point/column selection (full treatment `nurbs/03` Sec 3.2, 5.4). . *Kernel relevance.* The refit-to-tolerance loop for Sec 3 (sections with differing counts) and the v-direction adaptive knot insertion in Sec 5. [via nurbs/03 Sec 3.2]

---

## Skinning for Keel (synthesis)

**The pipeline.** Build smooth loft (item 66), continuity loft (item 67), and ruled (item 69) as **one parameterized operator** on top of the `kernel/33` constrained-LS engine:

1. **Compatibility (default = Park reduction, not union).** Degree-elevate sections to `p = max degree`; choose a **small common u-knot vector `W`** by Park-Lee common-knot determination (flexible-interval `ConstructKnot`, Schoenberg-Whitney existence check); **refit** each section onto `W` to tolerance by constrained energy minimization (`E = integral(alpha|C'|^2 + beta|C''|^2)`, a single linear KKT solve reusing the `kernel/32` stiffness `K`). Fall back to exact union merge only for few, near-compatible sections. This is the **knot-growth control verdict**: never union-merge by default; Park reduction plus post-loft Lyche-Morken knot removal keeps the surface compact (up to ~99% fewer control points), and the procedural sections are retained as the cache's truth for re-baking.
2. **Feature-correspondence parameterization (Stage B).** Reparameterize sections (centripetal default; radial/seam alignment for dissimilar or closed sections) so corresponding `u` denotes corresponding features. This is the anti-twist step.
3. **v-parameterization and v-knots (Stage C).** Averaged chord/centripetal `v_k`; v-degree `q` chosen by continuity demand (`q=1` ruled, `q=3` smooth/G1, `q=5` for G2-both-ends).
4. **The solve (Stage D).** Per control-point column, the v-direction interpolation `A_v Q_i = P_i` (one banded factor reused across all columns and coordinates) when unconstrained; otherwise the full `kernel/33` constrained-LS `minimize x^T H x s.t. C x = e` with the continuity and rail blocks.

**The knot-merge mitigation.** Park common-knot reduction (S4, S5) + tolerance-bounded refit (S3) + tolerant knot coalescing + post-loft Lyche-Morken knot removal. The union merge (S1) is the explicit worst case to avoid.

**The G1/G2 and rail handling.** End tangents/curvatures become **pinned v-control-rows** (`kernel/33` Sec 5): G1 pins row 1 (collinear CV), G2 pins rows 0,1,2 (so `q>=5` for G2 both ends, the Alias DOF rule). Guide rails turn the loft into a **Gordon surface** (S8): rails are the v-family, pinned as exact v-isocurve interpolation blocks; the Boolean sum converts to one NURBS via Lin-Hewitt (S9). Birail/one-rail sweeps orient the profile by double-reflection RMF (S13, S14). All of this is one constraint assembler feeding one constrained-LS solve, shared with n-sided fill (`kernel/26`, `kernel/33`).

**The periodic-section seam rule.** For closed/periodic sections: use **periodic u-knots and a wrapped control net** (inherits `C^{p-1}` across the u-seam); **align seam origins** across sections and **minimize twist** by a 1D search over the relative seam offset (radial alignment, S12, S16); place common knots at the **union of both sections' feature parameters** so a circle's smooth perimeter and a square's four corners correspond by perimeter fraction. For **rational** sections, loft in homogeneous coordinates with a smooth weight function across v (Hohmeyer-Barsky, S7) or convert to integral approximations.

**Validation: the 3-section circle-to-square skin with tangent ends.**
- Sections: `C_0` = circle (rational degree-2, periodic, ~9 control points), `C_1` = rounded intermediate, `C_2` = square (degree-2 with 4 raised-multiplicity corner knots), all **closed/periodic in u**.
- **Compatibility.** Common degree `p=2`. Because the square has corner knots the circle lacks, Park reduction places the common `W` knots at the **union of perimeter-fraction feature parameters** (4 corner stations + circle's smooth stations), refit all three onto periodic `W` to tolerance. Loft in **homogeneous coordinates** (S7) so the circle's weights interpolate smoothly to the square's; the surface develops the four corners only as `v -> 1`.
- **Seam/twist.** Align all three seam origins to one feature ("12 o'clock"); minimize `sum_i|P_{i,0}-P_{i,k}(theta)|^2` over the seam offset so the circle's seam maps to the square's corresponding mid-edge, not a corner: **no quarter-turn twist, no pinch**.
- **Tangent ends.** Prescribe `S_v(u,0)` perpendicular to the circle's plane (a "flat exit" off the circle) and `S_v(u,1)` perpendicular to the square's plane: each **pins v-row 1** at both ends (collinear CV), `q=3` sufficient for G1. Surface interpolates `C_0, C_1, C_2` exactly, leaves and arrives tangent as specified.
- **Expected failure mode if done naively.** Union-merge -> control-point explosion; independent section parameterization -> twist/pinch at the corners; rational loft in Cartesian -> weight discontinuity bands (Hohmeyer-Barsky). The recommended pipeline avoids all three.

**Caveat / flag.** This is a **design recommendation to validate against Keel's tests**. The specific tolerances (`eps`, the energy `alpha=1.0, beta=0.2`, the `per` flexibility), the choice of `q`, and the twist-minimization search must be tuned against Keel's tolerant-f64 substrate and regression corpus. The pure-interpolation banded path (Sec 1.5) and the constrained-LS path (`kernel/33`) should produce identical surfaces on clean unconstrained input: that equivalence is the first regression test. The circle-to-square case above is the recommended acceptance test for items 66/67; a two-curve `q=1` ruled surface is the acceptance test for item 69.

---

## References

1. Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.), Sec 10.3 (skinning), Ch. 5 (degree elevation, knot insertion), Sec 9.2 (global interpolation). Springer.
2. Piegl, L. A., & Tiller, W. (2002). Surface skinning revisited. *The Visual Computer*, 18(4), 273-283.
3. Piegl, L., & Tiller, W. (1995). Algorithm for approximate NURBS skinning. *Computer-Aided Design*, 27.
4. Park, H. (2003). Lofted B-spline surface interpolation by linearly constrained energy minimization. *Computer-Aided Design*, 35(14), 1331-1337.
5. Park, H., & Lee, J.-H. (2008). Reducing control points in lofted B-spline surface interpolation using common knot vector determination. *Computer-Aided Design*, 41(.).
6. Woodward, C. D. (1988). Skinning techniques for interactive B-spline surface interpolation. *Computer-Aided Design*, 20(8), 441-451.
7. Hohmeyer, M. E., & Barsky, B. A. (1991). Skinning rational B-spline curves to construct an interpolatory surface. *CVGIP: Graphical Models and Image Processing*, 53(6), 511-521.
8. Gordon, W. J. (1969). Spline-blended surface interpolation through curve networks. *Journal of Mathematics and Mechanics*, 18(10), 931-952; (1971) *SIAM J. Numer. Anal.*, 8(1), 158-177.
9. Lin, F., & Hewitt, W. T. (1994). Expressing Coons-Gordon surfaces as NURBS. *Computer-Aided Design*, 26(2), 145-155.
10. Tai, C.-L., et al. / Piegl-Tiller (1999). A method for approximate NURBS curve compatibility based on multiple curve refitting. *Computer-Aided Design*, 31(.).
11. "Optimal lofted B-spline surface interpolation" (2022). arXiv:2202.06330.
12. Shamsuddin, S. M., & Ahmed, M. A. (2005). NURBS skinning surface for ship hull design based on new parameterization method. *Int. J. Adv. Manuf. Technol.*
13. Wang, W., Juttler, B., Zheng, D., & Liu, Y. (2008). Computation of rotation minimizing frames. *ACM Transactions on Graphics*, 27(1), Art. 2.
14. Autodesk Maya / MayaLT (2015). Sweep one or more profile curves along two path curves (birail).
15. Sverchok (Nortikin). NURBS Loft / Gordon Surface / NURBS Birail node documentation.
16. Autodesk Maya. NURBS overview: Periodic, closed, and open geometry.
17. Park, H., & Lee, J.-H. (2007). B-spline curve fitting based on adaptive curve refinement using dominant points. *Computer-Aided Design*, 39(6), 439-451.

*Sibling dossiers cited (not repeated): `nurbs/03` (fitting/interpolation, knot-merge, Park reduction, banded solver, end conditions), `nurbs/05` (skinning survey, RMF), `kernel/26` (Gordon/Coons, transfinite), `kernel/32` (thin-plate fairness `x^T K x`), `kernel/33` (continuity as pinned control rows, constrained-LS KKT solve).*
