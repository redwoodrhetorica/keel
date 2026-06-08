# Constrained NURBS Surface Fitting and Inter-Patch Continuity

Research dossier for the Keel kernel on the **constraint and continuity machinery** behind XNURBS-style surfacing: how to express boundary, point, curve, and per-edge G0/G1/G2/G3 continuity as algebraic conditions on a NURBS control net, how to make a new surface meet an existing neighbour face to a prescribed continuity level, and how to fold all of this into a single constrained-fitting solve that always returns an answer even on dirty, gapped, or over-constrained input.

**Relationship to sibling dossiers.** This file is the *algebraic constraint/optimization* companion to three others. Dossier 31 (XNURBS capability map) is the behavioural target: an engine that accepts an arbitrary mix of boundary curves, internal curves, points, and per-edge continuity (to curves or to adjacent existing surfaces) and returns one NURBS surface, reporting conflicts on screen. Dossier 32 covers the **energy-minimization objective** (the fairness functional that picks the smoothest feasible surface); here we treat that functional as a given quadratic term `x^T K x` and concentrate on the **constraints** that sit on top of it. Dossier 26 (transfinite/n-sided) covers the **continuity-as-cross-derivative-fields** view (Coons ribbons, Gregory twists, vertex enclosure); here we take the **control-point-row** view: continuity as explicit linear or bilinear equations on the near-boundary rows of the control net. The two views are dual; this dossier states the algebra and the solver.

The central engineering question for Keel: given a tensor-product NURBS surface with unknown control points `P_{ij}` (and possibly weights `w_{ij}`), and a heterogeneous constraint set (interpolate these points, approximate these curves, be G2 to that existing face along that edge, be G1 to this free curve along this edge), assemble one system whose solution is the fairest surface satisfying the constraints to tolerance, detect and report infeasibility, and refine the surface's degrees of freedom (knot insertion, degree elevation) when the constraints cannot be met with the current control net.

---

## 1. Continuity definitions: parametric C^k versus geometric G^k

### 1.1 The precise hierarchy

For two surface pieces meeting along a shared boundary curve, with parametrizations `S(u,v)` (left) and `T(s,t)` (right) sharing the curve `S(u,0) = T(u,0) = c(u)`:

- **G0 (positional).** The two pieces share the boundary curve: `S(u,0) = T(u,0)` for all `u`. No derivative condition. On control nets this is simply *shared boundary control points* (the last row of one patch equals the first row of the other).
- **G1 (tangent-plane).** The two surface pieces share a common tangent plane all along the boundary: the surface normal is continuous across the seam. Equivalently the cross-boundary tangent `T_t` of the right piece lies in the plane spanned by the boundary tangent `c'(u)` and the cross-boundary tangent `S_v` of the left piece. There is no requirement that the cross-boundary derivatives be *equal*, only *coplanar* (this is the relaxation from C1).
- **G2 (curvature / tangent-plane + curvature).** Beyond G1, the two pieces share the same surface curvature along the boundary: the normal curvature in every direction, equivalently the second fundamental form, agrees, so the **Dupin indicatrix** is common across the seam. Operationally: the second cross-boundary derivative of one piece is a prescribed linear combination of `c''`, `c'`, `S_v`, and `S_vv` of the other.
- **G3 (curvature-rate).** Beyond G2, the rate of change of curvature (the third-order cross-boundary structure, the derivative of the curvature tensor along the boundary) agrees. Used for the highest-grade reflective "Class-A" automotive surfaces where a curvature discontinuity would show as a visible band under a zebra map.

### 1.2 Parametric versus geometric continuity

Parametric continuity `C^k` requires the partial derivatives up to order k to be *equal*: `S_v(u,0) = T_t(u,0)` for C1, and so on. Geometric continuity `G^k` requires only that there *exist* a reparametrization (a change of variables) under which the pieces meet C^k; equivalently the derivatives must satisfy the chain-rule relations with some positive reparametrization function rather than being identical. DeRose's key observation (Section 2) is that **parametric continuity is sufficient but not necessary** for visual/geometric smoothness: G^k is the genuinely shape-relevant notion, and C^k is the special case where the reparametrization is the identity. This matters for kernels because two adjacent NURBS faces almost never share a parametrization (different knot vectors, different parameter ranges), so the kernel must work in G^k, not C^k.

### 1.3 The beta-constraint formulation (surface join form)

The beta-constraints (introduced for curves in nurbs/01) come from applying the chain rule to a reparametrization. For curves joining at a point, with `Q(t)` the second segment reparametrized from the first `P(u)`:

- **G1 (beta1 only):** `Q'(0) = beta1 * P'(1)`, with `beta1 > 0`. beta1 is the *bias*/velocity ratio.
- **G2 (beta1, beta2):** `Q''(0) = beta1^2 * P''(1) + beta2 * P'(1)`, with beta2 free (the *tension*).
- **G3 (beta1, beta2, beta3):** `Q'''(0) = beta1^3 P''' + 3 beta1 beta2 P'' + beta3 P'`.

These are exactly the derivatives of the composite `P(phi(t))` where `phi` has `phi'(0)=beta1`, `phi''(0)=beta2`, `phi'''(0)=beta3`. For **surfaces** the same chain rule is applied bivariately along the boundary: the cross-boundary derivative of the right patch is expressed through a reparametrization that may *vary along the boundary*, so beta1 and beta2 become **scalar functions** `beta1(u)`, `beta2(u)` of the boundary parameter, plus a **twist/skew** term mixing the boundary-tangent direction. This variable-scalar-function structure is what makes surface G1/G2 conditions richer than the curve case and is the heart of the Liu-Hoschek and Che-Liang-Li conditions below.

---

## 2. DeRose: beta-constraints and the chain-rule derivation of G^n

**Citation.** DeRose, T. D. (1985). *Geometric continuity: A parametrization independent measure of continuity for computer aided geometric design* (Ph.D. thesis / Technical Report UCB/CSD-86-255). University of California, Berkeley, EECS. See also Barsky, B. A., & DeRose, T. D. (1989). Geometric continuity of parametric curves: three equivalent characterizations. *IEEE Computer Graphics and Applications*, 9(6), 60-68; and DeRose, T. D. (1990). Necessary and sufficient conditions for tangent plane continuity of Bezier surfaces. *Computer Aided Geometric Design*, 7(1-4), 165-179.

**Method / conditions.** DeRose extends geometric continuity to arbitrary order `G^n` for both curves and surfaces and derives constraint equations that are **necessary and sufficient**, obtained by direct application of the univariate chain rule (curves) and the bivariate chain rule (surfaces). The resulting beta-constraints introduce freely selectable **shape parameters** (the betas). Two patches are assumed only to share common boundary control points (and weights, in the rational case); no assumption is made about equal degree or triangular-versus-rectangular type. For tangent-plane (G1) continuity of two Bezier surfaces the condition is that, all along the shared boundary, the two cross-boundary tangent vectors and the boundary tangent are **coplanar**, expressible as a scalar-function linear combination

  `nu(u) * S_v(u,0) + lambda(u) * T_t(u,0) = mu(u) * c'(u)`

for scalar functions nu, lambda, mu (polynomials in u), which in turn becomes a set of **bilinear equations on the two near-boundary control-point rows** when the surfaces are written in Bernstein/B-spline form. DeRose also shows the rational case and that the conditions are coordinate/parametrization independent.

**Limitations.** The conditions are bilinear (products of the unknown betas/scalar functions with the unknown control points), so a *simultaneous* solve for both an unknown surface and unknown betas is nonlinear; they linearize only when one side (the neighbour, or the scalar functions) is fixed. The shape-parameter freedom is a design feature, not automatically resolved.

**Kernel relevance.** This is the foundational result Keel cites for why it works in G^n not C^n, and the source of the "fix the neighbour, solve for control points linearly" strategy (Section 5): once the neighbour patch and the scalar functions are fixed, the beta-constraints collapse to **linear** equations on the new surface's control rows. Keel's continuity-constraint assembler is, in effect, a DeRose beta-constraint generator specialized to "one side known."

---

## 3. Liu & Hoschek: GC1 conditions between adjacent patches

**Citation.** Liu, D., & Hoschek, J. (1989). GC1 continuity conditions between adjacent rectangular and triangular Bezier surface patches. *Computer-Aided Design*, 21(4), 194-200. (Companion: Liu, D. (1990). GC1 continuity conditions between two adjacent rational Bezier surface patches. *Computer Aided Geometric Design*, 7(1-4), 151-163.)

**Conditions.** Liu and Hoschek give **necessary and sufficient** conditions for geometric C1 (tangent-plane) continuity covering all four combinations of rectangular and triangular Bezier patches, then derive simpler **sufficient** conditions practical for CAD. For two rectangular patches sharing a boundary, writing the boundary control points `b_{i,0}` (shared) and the two adjacent interior rows `b_{i,1}` (left) and `b'_{i,1}` (right), the GC1 condition takes the cross-boundary-tangent-coplanarity form: there exist scalar functions (low-degree polynomials in the boundary parameter) `alpha(u)`, `beta(u)`, `gamma(u)` such that, all along the boundary,

  `D_right(u) = alpha(u) * D_left(u) + beta(u) * c'(u)`,

where `D_left`, `D_right` are the cross-boundary first derivatives and `c'` the boundary tangent. Equating Bernstein coefficients turns this into a finite set of **vector equations relating `b_{i,1}`, `b_{i,0}`, `b'_{i,1}`**, i.e. each right-side near-boundary control point must be an affine combination of the corresponding left-side control point, the boundary point, and its neighbours, with the alpha/beta coefficients carrying the variable scalar functions. The simplest sufficient case takes alpha, beta constant (the "constant-scalar" join), which forces the three control points `b_{i,1}, b_{i,0}, b'_{i,1}` to be **collinear with a fixed ratio** (the classic "tangent control points colinear across the seam" rule).

**Limitations.** The general (variable-scalar) conditions raise the degree of the matching constraints and couple neighbouring rows; the easy collinear/constant case over-restricts the shape (it is sufficient, not necessary, so it can refuse otherwise-valid G1 input). Triangular/rectangular mixing needs case analysis.

**Kernel relevance.** Liu-Hoschek is the canonical "G1 as conditions on control-point rows" reference. For Keel the constant-scalar collinearity rule is the cheap fast path; the variable-scalar version is the fallback when collinearity cannot be met. The structure "right interior row = affine function of left interior row + boundary row" is exactly the linear constraint block Keel emits when matching a fixed neighbour.

---

## 4. Che, Liang & Li: G1 conditions for general NURBS surfaces

**Citation.** Che, X., Liang, X., & Li, Q. (2005). G1 continuity conditions of adjacent NURBS surfaces. *Computer Aided Geometric Design*, 22(4), 285-298.

**Conditions.** Earlier results (DeRose, Liu-Hoschek) were for Bezier or special (bicubic/biquartic, single-interior-knot) B-spline surfaces. Che-Liang-Li give **necessary and sufficient** G1 conditions for two adjacent **NURBS** surfaces of **arbitrary degree and general knot structure**, plus two practical **sufficient** conditions. The derivation handles the rational weights and the non-Bezier knot structure by working segment-by-segment along the shared boundary, requiring the coplanarity/scalar-function relation to hold on each knot span. The upshot is again a set of linear relations among the shared boundary control points, the left near-boundary row, and the right near-boundary row (now weighted by the NURBS weights), with the scalar functions expressed in the boundary's B-spline basis.

**Limitations.** General-knot NURBS conditions are heavier (per-span equations, weight coupling); the rational weights make the conditions bilinear in (control point, weight) unless weights are fixed. The paper's sufficient conditions trade generality for a smaller linear system.

**Kernel relevance.** This is the *directly applicable* result for Keel because Keel's surface store is general-knot NURBS, not Bezier. The practical policy: fix the NURBS weights (work polynomially in the control points), use the per-span linear G1 relations as constraint rows. Weight optimization, if needed, is a separate outer loop.

---

## 5. Matching a FIXED neighbour surface (the kernel's core operation)

This is the operation XNURBS markets as "G2 to surrounding faces" and the one Keel most needs: the existing face is **given and immovable**; the new surface must meet it to G1/G2/G3 along a shared edge. When one side is fixed, the bilinear DeRose/Liu-Hoschek conditions become **linear constraints fixing the new surface's near-boundary control rows.**

### 5.1 Ribbon extraction from the neighbour

From the fixed neighbour face `N(s,t)` evaluated along the shared edge `c(u)`, extract:

- **Position ribbon (G0):** the boundary curve `c(u) = N(s(u), 0)` itself, reparametrized to the new surface's boundary parameter.
- **Tangent ribbon (G1):** the cross-boundary first-derivative field `N_t(u)` along the edge, plus the boundary tangent `c'(u)` and the unit normal field `n(u) = (c' x N_t)/|...|`. The new surface must have its cross-boundary tangent `S_v` lie in the tangent plane spanned by `c'(u)` and `N_t(u)`.
- **Curvature ribbon (G2):** the second-order cross-boundary field `N_tt(u)` (and the boundary curvature `c''(u)`), enough to reconstruct the neighbour's second fundamental form / normal curvature along the edge. The new surface's `S_vv` is then constrained so the normal curvatures match.
- **Curvature-rate ribbon (G3):** the third-order field `N_ttt(u)` for matching the rate of change of curvature.

### 5.2 The linear constraints on the new control rows

Write the new tensor-product surface boundary-direction rows as row 0 (boundary), row 1 (first interior), row 2 (second interior), etc., each a vector of control points `P_{i,0}, P_{i,1}, P_{i,2}`. Because the j-th cross-boundary derivative at `v=0` of a B-spline surface is a fixed linear combination of rows 0..j (via the de Boor derivative formula, with coefficients fixed by the knot vector), prescribing the derivative ribbons fixes the rows successively:

- **G0:** `P_{i,0} = ` (control points of the fitted boundary curve `c(u)`). One linear equation per control point per coordinate. The new boundary row is *pinned* to the shared curve.
- **G1:** the first cross-boundary derivative `S_v(u,0)`, which is `(p/ (knot span)) * (P_{i,1} - P_{i,0})` for each i (degree p), must equal the prescribed tangent ribbon (the chosen vector in the neighbour's tangent plane). With row 0 already pinned, this is a **linear equation pinning row 1**: `P_{i,1} = P_{i,0} + (span/p) * D1_i`, where `D1_i` is the sampled/fitted tangent-ribbon coefficient. Geometrically: `P_{i,0}, P_{i,1}` and the neighbour's adjacent control point are collinear (the colinear-tangent-CV rule from Alias Class-A practice).
- **G2:** the second cross-boundary derivative fixes a linear combination of rows 0,1,2; with rows 0,1 pinned this **pins row 2**. So G2 to a fixed neighbour consumes **three control-point rows** (rows 0,1,2) of the new surface (confirmed by the patent/practice note: "at least 3 rows of CVs each side for curvature continuity").
- **G3:** pins a fourth row (rows 0..3).

This is why degree and span count matter (Section 10): to satisfy G2 on **both** opposite edges of a 4-sided fill you need rows 0,1,2 pinned at v=0 **and** rows n,n-1,n-2 pinned at v=1; with a degree-(p) by degree-(q) surface this requires enough interior rows that the two pinned blocks do not overlap and contradict. Alias's rule (degree 5 for G2, degree 7 for G3, with extra spans inserted as needed) is exactly this DOF accounting.

### 5.3 Reparametrization matching (the variable scalars)

When the neighbour's edge parametrization differs from the new surface's boundary parametrization (the usual case), the ribbons must be **reparametrized** before sampling, or equivalently the beta scalar functions `beta1(u), beta2(u)` absorb the mismatch. In practice Keel will fit the new boundary curve `c(u)` to the neighbour edge (a curve-fit), establish the `u <-> s` correspondence, sample the neighbour ribbons at the new surface's Greville/boundary parameters, and emit the pinned-row equations. The variable scalar functions are realized implicitly by this resampling.

**Kernel relevance.** This section *is* the "fill with G2 to surrounding faces" kernel operation. It composes cleanly: each shared edge contributes an independent block of pinned-row equations; the interior control points (rows 3..n-3) remain free and are fixed by the fairness term plus any interior point/curve constraints.

---

## 6. Constrained fitting as a solve: equality-constrained least squares (KKT)

**Citation (formulation).** Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.), Ch. 9 (Curve and Surface Fitting), Springer. See also Lawson, C. L., & Hanson, R. J. (1974). *Solving Least Squares Problems*. Prentice-Hall (LSE / equality-constrained least squares); and Bjorck, A. (1996). *Numerical Methods for Least Squares Problems*. SIAM.

**Method.** Let the unknowns be the stacked control points `x` (one block per coordinate, or all three stacked). Three ingredients combine:

1. **Data term** (least squares to points/curves): `min_x || A x - d ||^2`, where each row of `A` is a tensor-product basis evaluation `R_{ij}(u_k, v_k)` at a data parameter and `d_k` the target point coordinate.
2. **Fairness term** (from dossier 32): a quadratic `x^T K x`, with `K` the discretized thin-plate/bending energy (symmetric positive semidefinite). Added with weight `lambda_f`.
3. **Continuity / interpolation constraints** as **hard equalities** `C x = e` (the pinned-row equations of Section 5, plus exact point interpolations).

The combined hard-constrained problem is

  `min_x  (A x - d)^T W (A x - d) + lambda_f * x^T K x   subject to   C x = e`,

with `W` a diagonal data-weight matrix. This is a quadratic program with linear equality constraints; its optimum satisfies the **KKT (Karush-Kuhn-Tucker) system**:

  `[ H   C^T ] [ x      ]   = [ A^T W d ]`
  `[ C    0  ] [ lambda ]     [ e       ]`

where `H = A^T W A + lambda_f K` is the (symmetric, positive semidefinite) Hessian and `lambda` the Lagrange multipliers. Solving this saddle-point system yields the fairest surface that **exactly** satisfies the continuity and interpolation constraints. Equivalent direct methods: the **nullspace method** (parametrize `x = x_p + Z y` with `Z` a basis for `null(C)`, reducing to an unconstrained solve in `y`), or **weighting/elimination** of the constraints.

**Hard versus soft.** Hard constraints (the KKT route) guarantee exact satisfaction but can make the system **infeasible or rank-deficient** when constraints conflict. The alternative is to move constraints into the objective as a **penalty**: `min_x (Ax-d)^T W (Ax-d) + lambda_f x^T K x + mu || C x - e ||^2`, with a large weight `mu`. As `mu -> infinity` the soft solution approaches the hard one, but a *finite* mu yields a system that is **always solvable** and trades a tiny constraint violation for guaranteed feasibility and better conditioning. The kernel policy (Section 12) blends both.

**Limitations.** The KKT matrix is indefinite (saddle point), so needs symmetric-indefinite factorization (LDL^T) or the nullspace/Schur approach; ill-conditioning when constraints are nearly dependent; exact hard constraints can over-determine the surface (Section 9).

**Kernel relevance.** This is the master template for Keel's solve. The fairness `K` comes from dossier 32; the `C x = e` blocks come from Sections 5 (continuity) and 7 (points/curves); the policy for hard vs soft comes from Section 8.

---

## 7. Point and curve constraints as rows of the system

**Citation.** Piegl & Tiller (1997), *The NURBS Book*, Ch. 9 (global interpolation/approximation, constrained least squares, end-derivative specification). Hoschek, J., & Lasser, D. (1993). *Fundamentals of Computer Aided Geometric Design* (Ch. on interpolation/approximation), A K Peters.

**Methods (explicit constraint rows).**

- **Point interpolation (pass exactly through `Q` at parameter `(u*, v*)`):** one linear equation per coordinate, `sum_{ij} R_{ij}(u*,v*) P_{ij} = Q`. This is a single row of the constraint matrix `C` (a hard equality), or a single heavily weighted row of `A` (soft). The basis functions `R_{ij}` evaluated at the parameter are the coefficients.
- **Point approximation (pass *near* many points):** each point is a row of the data matrix `A` in the least-squares term, weight `w_k` proportional to confidence. No exact satisfaction; the fairness term smooths between points.
- **Tangent / normal constraint at a point:** prescribe a derivative. `S_u(u*,v*) = t` gives `sum_{ij} (dR_{ij}/du)(u*,v*) P_{ij} = t`, a linear row in the basis *derivatives*. A **normal** constraint `n . S_u = 0` and `n . S_v = 0` gives two linear rows (the surface tangents are orthogonal to the prescribed normal). Curvature constraints use second-derivative basis rows.
- **Internal curve constraint (XNURBS "internal curves inside the surface"):** an internal curve `g(r)` that the surface must contain/approximate is **sampled** to a sequence of points `g(r_k)` at fitted parameters `(u_k, v_k)`, each becoming a point row (interpolation -> hard rows, approximation -> weighted data rows). If the curve must be *tangent* to a direction field along it, add derivative rows. This is exactly how "internal curves and points inside the surface" become **constraint rows**: every transfinite curve constraint is discretized to a finite set of linear point/derivative equations.
- **Boundary curve constraint:** the boundary row of control points is fitted to the boundary curve (Section 5.2, G0), either by interpolation (pin row 0) or by approximation (weighted boundary data rows).

**Parameterization of the constraint points.** Each point/curve sample needs a surface parameter `(u_k, v_k)`. For boundary/edge data this is the boundary parameter; for interior data a **parameter-correction / closest-point** iteration is run (project each data point to the current surface to get `(u_k,v_k)`, re-solve, repeat). This is the classic alternating fit (footpoint update + linear solve).

**Limitations.** Discretizing curves to points introduces a sampling-density tradeoff (too few points -> the curve is not really followed between samples; too many -> over-constraint and heavier system). Parameter correction is nonlinear and can be slow to converge on twisted data.

**Kernel relevance.** This is the dictionary that turns XNURBS's "mixed curves + points + edges" input into matrix rows. Every constraint type reduces to *basis-function (or basis-derivative) evaluations as a linear row*, hard (into `C`) or soft (into `A`), which is what makes one unified solve possible.

---

## 8. Gap-tolerant constraints: soft constraints, slack, and dirty-input absorption

**Citation.** Welch, W., & Witkin, A. (1992). Variational surface modeling. *Computer Graphics (SIGGRAPH '92 Proceedings)*, 26(2), 157-166. Kallay, M. (1993). Constrained optimization in surface design. In *Modeling in Computer Graphics* (pp. 85-93), Springer; and Kallay, M. (2000). Fairing spline curves and surfaces by minimizing energy. *Computer-Aided Design*. (See also dossiers 29/30 on dirty geometry.)

**Method.** Welch and Witkin present the surface to the user as an "infinitely malleable" sheet with **no fixed control mesh**: the user applies point and curve controls that act as **handles**, and within those constraints the shape is fixed by a **fairness criterion** (a thin-plate-style energy). The model is a **constrained variational optimization** over a nonuniformly subdivided B-spline surface. Constraints can be **hard** (interpolated exactly, enforced via Lagrange multipliers) or **soft** (added as penalty/energy terms with weights). This hard/soft duality is precisely the mechanism that absorbs dirty input.

The **gap/overlap absorption** works as follows. Inconsistent input (a boundary loop that does not close, curves that overlap, points that contradict a continuity request) is expressed as **soft (weighted) constraints with slack** rather than hard equalities. Each soft constraint contributes `w_k || c_k(x) - e_k ||^2` to the objective; the slack `s_k = c_k(x) - e_k` is whatever residual the optimum leaves. Because the objective is a sum of squares plus the positive-definite fairness term, it **always has a finite minimizer** regardless of whether the constraints are mutually satisfiable. There is no feasibility test to fail: a gapped boundary just yields nonzero slack at the gap, and the fairness term bridges it smoothly. This is the algebraic form of XNURBS's "solves across gaps and disjoint curves" claim and ties directly to the dirty-geometry dossiers: **messy input is non-fatal because every constraint is, at worst, a penalty term, never a hard precondition.**

**Weighting and regularization.** Conflicting constraints are balanced by their weights `w_k`: high-confidence data (a clean shared edge) gets large weight; uncertain data (a gappy scanned curve) gets small weight. The fairness weight `lambda_f` regularizes: raising it smooths over noisy/contradictory data at the cost of fidelity. The relative weights are the user-facing "sliders" of XNURBS (continuity precision vs surface simplicity).

**Limitations.** Soft constraints are only *approximately* satisfied, so G2-to-neighbour realized softly can leave a small curvature mismatch (XNURBS's reported G1 < 0.05 deg tolerance is exactly such a residual). Weight tuning is delicate: badly scaled weights produce either constraint violation or ill-conditioning. Penalty-only methods need large weights for tight constraints, which worsens conditioning (the motivation for the hybrid in Section 12).

**Kernel relevance.** This is the dossier's load-bearing idea for robustness. Keel should make **every** constraint expressible as either hard or soft, default boundary/continuity-to-clean-neighbour to hard-or-tight-soft, and default dirty/uncertain input to loose soft with slack, so the solve **always returns a surface**. The slack values are also the raw material for conflict reporting (Section 9).

---

## 9. Conflict detection, feasibility, and choosing among feasible solutions

**Citation.** Lawson & Hanson (1974), *Solving Least Squares Problems* (rank-deficient LS, the LSE problem); Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.), Johns Hopkins (rank detection via SVD/QR). Welch & Witkin (1992) and Kallay (1993) for the variational over/under-constrained handling.

**Methods.**

- **Over-constrained / conflicting detection.** With hard constraints `C x = e`, conflict shows up as **inconsistency of the linear system** (no exact solution) or, in the KKT matrix, as rank deficiency in `C`. Concretely: factor `C` (rank-revealing QR or SVD); if `rank([C | e]) > rank(C)`, the equality constraints are contradictory. The offending rows (the ones that, when dropped, restore consistency) are the **conflicting constraints to report** (XNURBS displays these on screen). With soft constraints there is never literal infeasibility, so conflict is detected by **large residual slack**: any soft constraint whose optimal slack `|s_k|` exceeds its tolerance is flagged as conflicting/unsatisfiable.
- **Rank-deficiency / under-constrained handling.** If the combined system leaves directions unconstrained (the data + constraints do not pin every control point), `H` is singular along those directions. The **fairness term** `lambda_f K` regularizes most of them (it penalizes non-smooth free modes), and any remaining nullspace is resolved by minimum-norm selection (pseudoinverse) or by the fairness energy itself. This is "choosing among feasible solutions via the fairness term": when many surfaces satisfy the constraints, return the smoothest, exactly the XNURBS/batten principle.
- **Reporting.** Map flagged constraint rows back to user input (this edge's G2 request conflicts with that point's interpolation; this gap cannot be closed at G1). XNURBS V7.0's on-screen conflict display is this mapping.

**Limitations.** Identifying a *minimal* conflicting subset is combinatorial in general; the practical approach reports the largest-residual constraints, which is a heuristic, not a guaranteed minimal explanation. Near-conflicts (consistent but ill-conditioned) are easy to misclassify.

**Kernel relevance.** Keel needs (a) a rank-revealing factorization of the hard-constraint block to detect contradiction, (b) slack thresholds on soft constraints to flag near-conflicts, and (c) the fairness term as the universal tie-breaker for under-determination. The output is a diagnosable surface plus a conflict report, matching XNURBS behaviour.

---

## 10. Corner / vertex compatibility in the constrained setting

**Citation.** Hahn, J. (1989). Filling polygonal holes with rectangular patches. In W. Strasser & H.-P. Seidel (Eds.), *Theory and Practice of Geometric Modelling* (pp. 81-91), Springer. Peters, J. (1992). Joining smooth patches around a vertex to form a C^k surface. *Computer Aided Geometric Design*, 9(5), 387-411. Gregory, J. A., & Hahn, J. M. (1989). A C2 polygonal surface patch. *Computer Aided Geometric Design*, 6(1), 69-75. (Cross-reference dossier 26, vertex enclosure / twist compatibility.)

**The problem.** When a new surface must match **several** neighbours that meet at a shared **corner**, the continuity constraints from the different edges all bear on the corner control points (and their twists). Peters showed that around a vertex the per-edge G^k conditions form a **cyclic (circular) system**: going around the vertex and composing the edge-to-edge reparametrizations must return to the identity (the **vertex-enclosure / compatibility condition**). If it does not, **no single tensor-product patch can meet all neighbours exactly** at the prescribed continuity, the corner is over-constrained. This is the algebraic twin of the twist-incompatibility problem (dossier 26): the mixed partials `S_uv` implied by the two edges at a corner must agree, and independently chosen neighbour ribbons generally make them disagree (the "suitcase corner" of Hahn's polygonal fill).

**How constrained optimization copes.** When exact match is impossible, the constrained solve **relaxes the corner conditions to least-squares**: the corner control points are not hard-pinned by all edges simultaneously (which would be infeasible) but are placed to **minimize the total weighted continuity residual** across the meeting edges, with the fairness term smoothing the result. Practically: demote the corner-coincident continuity rows from hard to soft, let the solver split the difference, and accept a small curvature mismatch at the corner rather than a failed solve. Hahn's and Gregory-Hahn's constructions instead *change the representation* (rational Gregory twists, or n rectangular subpatches meeting at a central point) to gain the freedom to satisfy the cyclic condition exactly; Keel's constrained-LS route trades that for a stored tensor-product surface with a relaxed corner.

**Limitations.** Relaxation means the corner is only approximately G2/G3; for true Class-A a representation change (Gregory/subdivision, dossier 26) may be required. The cyclic compatibility around an odd-valence vertex can be impossible to satisfy with even parity (parity obstruction).

**Kernel relevance.** Keel must (a) detect the cyclic vertex-enclosure obstruction (compose the edge reparametrizations, check closure), (b) when violated, demote corner continuity to soft and report the residual, and (c) for high-grade demands, fall back to the dossier-26 subdivision/Gregory construction. This is where dossier 33 (algebra) and dossier 26 (representation) explicitly meet.

---

## 11. Variational/constrained-optimization references for the objective + constraints

**Citation.** Welch, W., & Witkin, A. (1992), *Variational surface modeling* (full entry in Section 8). Kallay, M. (1993). Constrained optimization in surface design. *Modeling in Computer Graphics*, Springer. Greiner, G. (1994). Variational design and fairing of spline surfaces. *Computer Graphics Forum (Eurographics '94)*, 13(3), 143-154. Celniker, G., & Gossard, D. (1991). Deformable curve and surface finite-elements for free-form shape design. *Computer Graphics (SIGGRAPH '91)*, 25(4), 257-266.

**Method.** This cluster establishes the **variational constrained-design** paradigm Keel inherits: a surface is the minimizer of a fairness energy subject to user constraints. Celniker-Gossard cast it as **finite elements** (the surface energy assembled as element stiffness matrices `K`, constraints as boundary conditions), solved as a linear (or linearized) system, the same `H x = ...` structure as Section 6 with `K` from FE assembly. Greiner formulates surface fairing as minimizing a quadratic fairness functional over the control points subject to interpolation constraints, solved as a sparse linear system (or KKT system) per coordinate. Kallay's "constrained optimization in surface design" and "fairing by minimizing energy" give the canonical statement: **reshape/build a surface by choosing control points that minimize change-in-energy subject to the specified constraints**, with the energy a quadratic form so the solve is linear when constraints are linear. Kallay-Ravani (1990, optimal twist vectors) supplies the twist-as-free-DOF-chosen-by-energy idea relevant to corners (Section 10).

**Limitations.** Linear/quadratic only after fixing weights and (for NURBS) weights/parametrization; true G2 matching to an arbitrary neighbour is nonlinear and needs an outer iteration. FE discretization adds mesh-design overhead.

**Kernel relevance.** Confirms the architecture: assemble `K` (fairness, dossier 32) once, append linear constraint rows (Sections 5/7), solve the sparse symmetric (KKT or normal-equation) system. Keel's solver is a direct descendant of this 1990s variational-design lineage, specialized to NURBS control nets.

---

## 12. Solver structure, conditioning, and the refine-until-feasible loop

**Citation.** Piegl & Tiller (1997), Ch. 9 (knot insertion / degree elevation to add DOF); Boehm, W. (1980). Inserting new knots into B-spline curves. *Computer-Aided Design*, 12(4), 199-201; Golub & Van Loan (2013) (conditioning, sparse symmetric-indefinite solves); Autodesk Alias surfacing documentation (Curvature Continuity in Surface Modeling) for the degree/span DOF rules in practice.

**Method.**

- **System structure and sparsity.** The Hessian `H = A^T W A + lambda_f K` is **sparse and banded**: a control point couples only to data/constraints within its basis support, and `K` couples only neighbouring control points. The constraint block `C` is sparse (each continuity row touches a few near-boundary control points; each point row touches the `(p+1)(q+1)` basis functions in one knot box). The KKT matrix is therefore sparse symmetric indefinite, solved by sparse LDL^T or by the nullspace/Schur-complement method. Per-coordinate decoupling (x, y, z solved with the same `H`, different right sides) lets one factorization serve three solves.
- **Conditioning.** Conditioning degrades with near-dependent constraints, very uneven weights (penalty `mu` too large), and inadequate parameterization. Mitigations: scale/normalize the basis and data, prefer the nullspace method or moderate penalty weights over huge ones, regularize with `lambda_f K`, and use rank-revealing factorization to drop redundant constraint rows.
- **Degree/knot adequacy (DOF accounting).** The constraints are satisfiable only if the control net has **enough degrees of freedom**. G2-to-neighbour on an edge consumes 3 control rows (Section 5.2); G2 on opposite edges consumes 6 rows, so the surface needs degree/span enough that 6 rows fit without the two pinned blocks colliding. Alias's practical rule: **degree 5 for G2, degree 7 for G3, add extra spans when tangent/curvature targets cannot otherwise be met.**
- **The refine-until-feasible loop.** When the current control net cannot satisfy the constraints to tolerance (large residual slack, or hard infeasibility from too few DOF), **add freedom and re-solve**: insert knots (Boehm knot insertion adds control rows without changing the current surface) and/or elevate degree, then reassemble and solve again. Iterate: solve -> measure max constraint residual (G0 distance, G1 angle, G2 curvature deviation) -> if over tolerance, refine (knot insertion where the residual is largest) -> re-solve, until all tolerances are met or a DOF/iteration cap is hit. This is exactly XNURBS's "the optimizer chooses the control-point count / knotting needed to hit the precision," and the V6.1 "control-point reduction" is the reverse pass (knot removal where the surface is over-resolved).

**Limitations.** Unbounded refinement yields heavy, control-point-dense surfaces (XNURBS's documented "dense grid / wavy interior" failure mode): more DOF satisfies constraints but can hurt fairness. A refine cap plus a knot-removal cleanup pass is needed. Knot insertion at the wrong location wastes DOF.

**Kernel relevance.** This defines Keel's solver loop end to end: sparse KKT/normal-equation solve, residual measurement in geometric units, knot-insertion refinement targeted at the worst residual, knot-removal cleanup, all bounded. It directly implements the XNURBS "solver-chosen knots to meet precision" behaviour with an explicit, inspectable loop.

---

## 13. Practical surface-matching operations in kernels (published accounts)

**Citation.** Autodesk Alias, *Curvature Continuity in Surface Modeling* (tutorial/reference, multiple versions); Pottmann, H., & Wallner, J. (2001). *Computational Line Geometry* and the Class-A / reflection-line literature; Farin, G. (2002). *Curves and Surfaces for CAGD* (5th ed.), Morgan Kaufmann (Ch. on surface continuity and the Farin-Boehm / Boehm conditions); Hoschek & Lasser (1993), *Fundamentals of CAGD* (surface continuity chapters).

**Method / operations.** The CAD-surfacing literature names the concrete operations Keel must provide:

- **Match-surface (G0/G1/G2/G3 to a target face along an edge):** pin 1/2/3/4 near-boundary control rows of the surface being matched to the target's ribbons (Section 5). Alias exposes exactly G0 position, G1 tangent, G2 curvature, G3 curvature-rate as per-edge options, and notes the **colinear tangent CVs** rule for G1 and the **3-CV-row** requirement for G2.
- **Tangent/curvature-continuous fill ("fill with G2 to surrounding faces"):** the n-edge version, one pinned-row block per surrounding edge, interior free, faired. This is the headline XNURBS operation and the n-sided fill of dossier 26, here solved as constrained-LS.
- **Farin-Boehm G2 condition (control-point form):** for a composite spline the classic condition that the second differences across an interior knot be consistent. Farin gives the **Farin-Boehm** construction (a specific collinearity/ratio condition on the control points straddling a knot for C2/G2), the polygon-level statement of curvature continuity Keel uses to verify or enforce G2 internally.
- **Reflection/zebra acceptance.** Class-A acceptance is judged by reflection-line continuity: G2 removes curvature-band artifacts, G3 removes the subtler acceleration band. The kernel exposes curvature combs / zebra to verify, since meeting the algebraic condition to tolerance is necessary but the visual check is the acceptance test.

**Limitations.** Published kernel internals are sparse (the strong engines are proprietary); the Alias/Farin accounts give the conditions and DOF rules but not a full robust solver. True Class-A often still needs manual CV cleanup, the gap between "meets G2 to tolerance" and "fair under reflection."

**Kernel relevance.** These are the user-facing operations Keel must ship on top of the Sections 5-12 machinery: per-edge match-surface, multi-edge G2 fill, and curvature verification, all reducing to pinned-row constraint blocks plus the constrained-LS solve.

---

## 14. Constrained surfacing for Keel (synthesis)

**The unified solve.** Keel builds every surfacing operation (blend, fill, loft, network, point-fit, match-surface) as **one constrained least-squares problem**:

  `min_x  (A x - d)^T W (A x - d)  +  lambda_f * x^T K x   subject to (and/or penalizing)  C x = e`

where `x` is the control net (weights fixed in the inner solve), `A/d/W` the **data term** (points and sampled curves, Section 7), `K` the **fairness energy** (dossier 32), and `C x = e` the **constraint block** (G0/G1/G2/G3 pinned rows from fixed neighbours and free curves, Section 5; exact point/derivative interpolations, Section 7). Solve via the **KKT saddle-point system** (Section 6), exploiting sparsity and per-coordinate decoupling.

**Hard vs soft policy.** Default **clean** constraints (G0 to a clean shared edge, exact point interpolation, G1/G2 to a watertight neighbour) to **hard** (into `C`) or tight-soft. Default **dirty/uncertain** constraints (gapped boundary, overlapping curves, scanned points, contradictory requests) to **loose weighted soft** with slack (Section 8). Because soft constraints are penalty terms, **the solve never fails on dirty input**: gaps become slack, the fairness term bridges them. This is the algebraic realization of XNURBS's gap tolerance and ties directly to dossiers 29/30 (dirty geometry, graceful degradation).

**Conflict and feasibility.** Detect contradiction by rank-revealing factorization of the hard block (`rank([C|e]) > rank(C)`) and by **large residual slack** on soft constraints; map flagged rows back to user input and **report conflicts** (XNURBS V7.0 behaviour, Section 9). Resolve under-determination with the fairness term as universal tie-breaker (smoothest feasible surface, the batten principle).

**The refine-until-feasible loop.** If residuals exceed geometric tolerance (G0 distance / G1 angle / G2 curvature deviation), **add DOF where the residual is worst** via Boehm knot insertion and/or degree elevation (degree 5 for G2, 7 for G3), re-assemble, re-solve; iterate under a cap; finish with a **knot-removal cleanup** pass to shed excess control points (XNURBS V6.1 reduction). This implements "solver-chosen knots to meet precision" explicitly and inspectably (Section 12).

**Composition with siblings.**
- With **dossier 32 (energy):** dossier 32 supplies `K`; this dossier supplies `A/d`, `C/e`, and the solver around it. The fairness term is both the smoother and the tie-breaker for under-determination.
- With **dossier 26 (transfinite/n-sided):** dossier 26's cross-derivative ribbons are the *source data* for the pinned-row constraints here (sample the ribbon, emit linear rows); dossier 26's vertex-enclosure/twist-compatibility is the corner obstruction (Section 10) that this dossier handles by **demoting corner continuity to least-squares** when exact match is infeasible, falling back to dossier-26 Gregory/subdivision representation for true Class-A corners.

**The G2-fill-to-neighbours operation (the deliverable).** Given a region bounded by existing faces and free curves with per-edge continuity requests: (1) fit boundary rows to edges (G0); (2) extract tangent/curvature ribbons from each fixed neighbour (Section 5.1); (3) emit pinned-row constraint blocks per edge (G1 -> row 1, G2 -> rows 1-2); (4) discretize internal curves/points to constraint rows (Section 7); (5) assemble `H = A^T W A + lambda_f K`, solve the KKT system; (6) measure residuals, refine knots where worst, re-solve until tolerance; (7) clean up knots, report any conflicts. The output is **one tensor-product NURBS surface, watertight to its neighbours at the requested G-level, fairest among feasible solutions** , the XNURBS headline capability, expressed entirely as constrained least squares over the control net.

---

## References

- Barsky, B. A., & DeRose, T. D. (1989). Geometric continuity of parametric curves: three equivalent characterizations. *IEEE Computer Graphics and Applications*, 9(6), 60-68. https://www.semanticscholar.org/paper/621845e04ba0ec63f7c518e7357ea848cfbf3e25
- Bjorck, A. (1996). *Numerical Methods for Least Squares Problems*. SIAM.
- Boehm, W. (1980). Inserting new knots into B-spline curves. *Computer-Aided Design*, 12(4), 199-201.
- Celniker, G., & Gossard, D. (1991). Deformable curve and surface finite-elements for free-form shape design. *Computer Graphics (SIGGRAPH '91)*, 25(4), 257-266.
- Che, X., Liang, X., & Li, Q. (2005). G1 continuity conditions of adjacent NURBS surfaces. *Computer Aided Geometric Design*, 22(4), 285-298. https://www.sciencedirect.com/science/article/abs/pii/S0167839605000117
- DeRose, T. D. (1985/1986). *Geometric continuity: A parametrization independent measure of continuity for computer aided geometric design* (Ph.D. thesis; Tech. Report UCB/CSD-86-255). UC Berkeley. https://www2.eecs.berkeley.edu/Pubs/TechRpts/1986/6081.html
- DeRose, T. D. (1990). Necessary and sufficient conditions for tangent plane continuity of Bezier surfaces. *Computer Aided Geometric Design*, 7(1-4), 165-179. https://www.sciencedirect.com/science/article/abs/pii/016783969090028P
- Farin, G. (2002). *Curves and Surfaces for CAGD: A Practical Guide* (5th ed.). Morgan Kaufmann.
- Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.). Johns Hopkins University Press.
- Greiner, G. (1994). Variational design and fairing of spline surfaces. *Computer Graphics Forum (Eurographics '94)*, 13(3), 143-154.
- Gregory, J. A., & Hahn, J. M. (1989). A C2 polygonal surface patch. *Computer Aided Geometric Design*, 6(1), 69-75. https://www.sciencedirect.com/science/article/abs/pii/0167839689900071
- Hahn, J. (1989). Filling polygonal holes with rectangular patches. In W. Strasser & H.-P. Seidel (Eds.), *Theory and Practice of Geometric Modelling* (pp. 81-91). Springer. https://link.springer.com/chapter/10.1007/978-3-642-61542-9_6
- Hoschek, J., & Lasser, D. (1993). *Fundamentals of Computer Aided Geometric Design*. A K Peters.
- Kallay, M. (1993). Constrained optimization in surface design. In *Modeling in Computer Graphics* (pp. 85-93). Springer. https://link.springer.com/chapter/10.1007/978-3-642-78114-8_5
- Kallay, M. (2000). Fairing spline curves and surfaces by minimizing energy. *Computer-Aided Design*, 32(3). https://www.sciencedirect.com/science/article/abs/pii/S0010448500001147
- Lawson, C. L., & Hanson, R. J. (1974). *Solving Least Squares Problems*. Prentice-Hall.
- Liu, D., & Hoschek, J. (1989). GC1 continuity conditions between adjacent rectangular and triangular Bezier surface patches. *Computer-Aided Design*, 21(4), 194-200. https://www.sciencedirect.com/science/article/abs/pii/0010448589900444
- Liu, D. (1990). GC1 continuity conditions between two adjacent rational Bezier surface patches. *Computer Aided Geometric Design*, 7(1-4), 151-163. https://www.sciencedirect.com/science/article/abs/pii/016783969090027O
- Peters, J. (1992). Joining smooth patches around a vertex to form a C^k surface. *Computer Aided Geometric Design*, 9(5), 387-411. https://www.sciencedirect.com/science/article/abs/pii/016783969290032K
- Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer.
- Welch, W., & Witkin, A. (1992). Variational surface modeling. *Computer Graphics (SIGGRAPH '92)*, 26(2), 157-166. https://www.ri.cmu.edu/publications/variational-surface-modeling/
- Autodesk. *Curvature Continuity in Surface Modeling* (Alias documentation). https://help.autodesk.com/cloudhelp/2026/ENU/Alias-Video-Tutorials/files/essential-concepts/continuity-g0-g1-g2-g3.html
- G2 continuity algorithms between adjacent NURBS patches along a common cubic boundary curve. *Procedia Engineering / ScienceDirect*. https://www.sciencedirect.com/science/article/pii/S100093611160191X
