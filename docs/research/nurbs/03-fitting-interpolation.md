# NURBS / B-Spline Fitting, Interpolation, Approximation, and Parameterization

Research dossier for the Keel B-rep kernel. This file collects the algorithmic literature behind turning point data into B-spline curves and surfaces, the parameterization and knot-placement decisions that govern fit quality, the approximation machinery (least squares, smoothing penalties, error-bounded refinement, PIA), the surface-construction operators a kernel actually ships (lofting, skinning, scattered-data fitting), reverse-engineering pipelines, fairing, analytic-curve approximation, degree reduction, and the recent ML-adjacent work. Each entry gives a full citation, the method in concrete detail (formulas where they are load-bearing), reported results, limitations, and direct kernel relevance. A closing synthesis says what Keel needs to build first.

Notation used throughout: data points `Q_0 ... Q_m`, parameters `u_0 ... u_m`, B-spline of degree `p` with control points `P_0 ... P_n`, knot vector `U = {u_0, ..., u_{n+p+1}}`, B-spline basis functions `N_{i,p}(u)`. A point on the curve is `C(u) = sum_i N_{i,p}(u) P_i`. For NURBS add weights `w_i` and use the rational basis `R_{i,p}(u) = w_i N_{i,p}(u) / sum_j w_j N_{j,p}(u)`.

---

## 1. Foundations: the interpolation problem and solvability

### 1.1 Global curve interpolation by a banded linear system

**Citation.** Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.), Chapter 9 ("Curve and Surface Fitting"), Sections 9.2.1 ("Global Interpolation"). Berlin: Springer.

**Method.** Given `m+1` points `Q_k` and assigned parameters `u_k`, choose a degree `p` and a knot vector `U` with `n = m` (so the number of unknown control points equals the number of points). Interpolation requires `C(u_k) = Q_k` for every `k`, which expands to

```
Q_k = sum_{i=0}^{n} N_{i,p}(u_k) P_i ,   k = 0..m
```

This is a single linear system `A P = Q`, one scalar system per coordinate (x, y, z share the same coefficient matrix `A`, with `A_{k,i} = N_{i,p}(u_k)`). Because each row of `A` evaluates the basis at one parameter, and a degree-`p` basis function is nonzero over only `p+1` knot spans, `A` is **banded** with bandwidth `2p+1`. The matrix is totally positive and (under the solvability condition below) nonsingular, so it is solved by LU decomposition without pivoting in `O(n (2p+1))` time and `O(n p)` storage. For NURBS interpolation with prescribed weights, the weights enter `Q` and the rational basis, but the typical fitting workflow interpolates with integral (non-rational) splines and treats weights as a separate design freedom.

**Results.** Exact interpolation, `C^{p-1}` continuity internally, stable for moderate `n`. The banded solve is the workhorse of every CAD kernel's "interpolate through points" command.

**Limitations.** Interpolation reproduces every wiggle and every noise spike in the data; it is the wrong tool for measured/scanned data. Parameter and knot choices (Sections 2 and 3) dominate the visual quality of the result far more than the solver does. With `n = m` the curve can have as many control points as data points, which is heavy for downstream operations.

**Kernel relevance.** This is the canonical primitive Keel must implement first for any "fit curve through these points" feature and as the inner loop for surface interpolation. The banded LU solver should be a shared numerics component reused by lofting, skinning, and the iso-curve constructions. Keel should expose end-derivative variants (next entry) because real CAD workflows constrain tangents.

### 1.2 End conditions: clamped tangents, Bessel, natural, not-a-knot

**Citation.** Piegl & Tiller (1997), Section 9.2.2; Farin, G. (2002). *Curves and Surfaces for CAGD: A Practical Guide* (5th ed.), Chapters 9 and 10. San Francisco: Morgan Kaufmann.

**Method.** The bare interpolation system has `m+1` equations and `m+1` unknowns. To control endpoints or interior derivatives, add two extra equations (and two extra control points, `n = m+2`) prescribing `C'(u_0) = D_0` and `C'(u_m) = D_m`. The derivative of a B-spline is itself a B-spline of degree `p-1`, so the constraint rows still fit the banded structure. Common end-condition choices: **clamped** (user supplies tangent vectors), **Bessel** (estimate end tangents from a parabola through the first/last three points), **natural** (second derivative zero at the ends, minimizing energy but flattening the curve), and **not-a-knot** (force `C^p` continuity across the first and last interior knots, the default for many cubic-spline routines). Farin emphasizes that for cubic interpolation these conditions differ only near the ends but can change the entire curve through the global coupling.

**Results.** Clamped/Bessel give the most predictable shapes for engineering; natural is smoothest in the bending-energy sense but tends to "pull in" near the boundary.

**Limitations.** Wrong end conditions are a frequent source of unwanted overshoot at the first and last spans; users rarely have good tangent data.

**Kernel relevance.** Keel needs all four as named options on its interpolation API, because IGES/STEP import and surface lofting both rely on consistent end-tangent handling, and surface skinning needs matching end conditions across section curves.

### 1.3 Schoenberg-Whitney solvability condition

**Citation.** Schoenberg, I. J., & Whitney, A. (1953). On Polya frequency functions III: The positivity of translation determinants with an application to the interpolation problem by spline curves. *Transactions of the American Mathematical Society*, 74(2), 246-259. (Stated in modern form in de Boor, C. (2001). *A Practical Guide to Splines* (rev. ed.), Springer, Chapter XIII.)

**Method.** The interpolation matrix `A` with `A_{k,i} = N_{i,p}(u_k)` is nonsingular **iff** `N_{i,p}(u_i) != 0` for all `i`, equivalently iff each parameter lies in the support of its corresponding basis function:

```
u_i < t_{i+p+1}  and  t_i < u_i   (with appropriate clamped-end handling),
```

stated compactly as `t_i < u_i < t_{i+p+1}` for every `i` (the diagonal of `A` must be strictly positive). Practically: between any two consecutive knots there must not be more or fewer data parameters than the basis can resolve, and parameters must be "spread" so that each basis function "sees" its own data point. If knots are placed by the averaging rule (Section 3.1), Schoenberg-Whitney is automatically satisfied; arbitrary or clustered knots can violate it and make `A` singular.

**Results.** This is the exact, necessary-and-sufficient condition; total positivity of the spline collocation matrix then guarantees a well-conditioned banded solve.

**Limitations.** The condition guarantees nonsingularity, not good conditioning. Near-violations (parameters almost outside support) produce ill-conditioned systems and oscillatory fits even though `A` is technically invertible.

**Kernel relevance.** Keel's interpolation and approximation routines must validate Schoenberg-Whitney (or use a knot rule that guarantees it) and fall back gracefully (reparameterize, merge near-duplicate points, or switch to least-squares) when the condition is marginal. This is a robustness requirement, not optional: import healing routinely throws degenerate parameter distributions at the fitter.

### 1.4 Local interpolation methods (Catmull-Rom, local cubic, Bessel-tangent)

**Citation.** Piegl & Tiller (1997), Section 9.3 ("Local Interpolation"); Farin (2002), Chapter 9.

**Method.** Instead of one global system, build each curve segment from local data only: estimate a tangent `T_k` at each `Q_k` (Bessel, Akima, or chord-based heuristics) and construct a Hermite/Bezier segment per interval, then represent the whole as a B-spline (or rational B-spline) by knot insertion at the joints. Catmull-Rom is the special case `T_k = (Q_{k+1} - Q_{k-1}) / (u_{k+1} - u_{k-1})`. Piegl-Tiller's local methods produce `C^1` (or `G^1`) curves and can locally choose weights to control fullness.

**Results.** Local methods are `O(m)`, embarrassingly parallel, and changing one point affects only nearby segments. They avoid the global oscillation that a single bad point induces in global interpolation.

**Limitations.** Lower continuity (`C^1`/`G^1`, not `C^2`); tangent heuristics can misbehave at sharp data; more control points than the global cubic for the same data.

**Kernel relevance.** Useful for interactive editing (local updates), for sketch tools, and as a robust fallback when global interpolation is ill-conditioned. Keel should offer a local-interpolation path for responsiveness and for data with corners.

---

## 2. Parameterization of the data points

The assignment `Q_k -> u_k` is made before any solve, and it controls fit quality more than the solver. All methods below normalize to `[0,1]`.

### 2.1 Uniform, chord-length, centripetal

**Citation.** Lee, E. T. Y. (1989). Choosing nodes in parametric curve interpolation. *Computer-Aided Design*, 21(6), 363-370. (Foundational comparison; the centripetal exponent `1/2` is Lee's recommendation. Also Piegl & Tiller (1997), Section 9.2.1.)

**Method.** Let `d_k = |Q_k - Q_{k-1}|`.
- **Uniform:** `u_k = k/m`. Ignores geometry.
- **Chord length:** `u_k = u_{k-1} + d_k / L`, `L = sum d_j`. Distributes parameter proportional to distance.
- **Centripetal (Lee):** `u_k = u_{k-1} + sqrt(d_k) / sum_j sqrt(d_j)`. Uses the square root of chord length. Lee derived the exponent `1/2` as a balance that suppresses overshoot when chord lengths vary sharply.

A general family uses `d_k^a`: `a=0` uniform, `a=1` chord, `a=1/2` centripetal.

**Results.** Lee and many later studies (and the SideFX/CAD-practice literature) report: uniform produces loops and overshoot when point spacing is uneven; chord length is good for evenly spaced smooth data but overshoots at sharp turns; centripetal gives "obviously less oscillation than chord length" near abrupt direction changes and is the best general default for fit quality versus computational cost. Catmull-Rom with centripetal parameterization provably avoids cusps and self-intersections (Yuksel et al., later result).

**Limitations.** None is optimal for all data. Centripetal can be slightly too "tight" on smooth uniform data where chord length is ideal.

**Kernel relevance.** Keel should default to **centripetal**, expose chord-length and uniform, and use the same parameterization consistently across a surface's u and v directions during lofting. Parameterization choice must be a documented, switchable option because import-healing and reverse-engineering need to match the source CAD system's convention.

### 2.2 Foley-Nielson (curvature/angle-corrected)

**Citation.** Foley, T. A., & Nielson, G. M. (1989). Knot selection for parametric spline interpolation. In *Mathematical Methods in Computer Aided Geometric Design* (pp. 261-272). Academic Press.

**Method.** Correct chord-length spacing by a factor that accounts for the exterior angle `theta_k` between successive chords, so that parameter intervals grow where the data turns sharply. The Foley-Nielson interval is

```
u_k - u_{k-1} = d_k * (1 + 3/2 * (theta_{k-1} d_{k-1} / (d_{k-1}+d_k)
                                + theta_k d_{k+1} / (d_k+d_{k+1})) )
```

with the angles clamped to `pi/2`. Variants substitute the chord metric with the centripetal or Nielson metric (the F-CEN and F-CHD families seen in comparison papers).

**Results.** Better than plain chord length on data with significant curvature variation; one of the standard comparison baselines.

**Limitations.** More parameters/heuristics; angle estimates are noisy on scanned data; rarely beats centripetal enough to justify the complexity in practice.

**Kernel relevance.** Worth implementing as an option for high-quality interpolation of clean design data; lower priority than centripetal.

### 2.3 Universal parameterization (Lim)

**Citation.** Lim, C.-G. (1999). A universal parametrization in B-spline curve and surface interpolation. *Computer Aided Geometric Design*, 16(5), 407-422. See also Lim's follow-up on smoothly-connected B-spline surfaces.

**Method.** Decouple parameterization from chord length entirely: set `u_k` to the parameter value where the `k`-th B-spline basis function attains its maximum (the Greville/maximum abscissa of `N_{k,p}`). Because basis maxima depend only on the knot vector and degree, the resulting parameterization is **affine/transformation invariant**: scaling or shearing the data does not change `u_k`. Lim shows this yields "more natural looking" interpolants and, crucially for surfaces, guarantees `G^0` continuity when independently constructed patches are assembled without extra constraints.

**Results.** Transformation invariance and patch-assembly compatibility are the headline benefits; competitive interpolation quality.

**Limitations.** Ignores data geometry, so for a single curve it can be worse than centripetal on highly non-uniform data; mainly attractive for multi-patch surface consistency.

**Kernel relevance.** Directly relevant to **skinning and patch assembly**: universal parameterization is a tool for making independently fit section curves compatible without forcing knot merges. Keep on the radar for surface construction, not as the default curve fitter.

### 2.4 Modern parameterization improvements and the open problem

**Citation.** Balta, C., Ozturk, S., & Kuncan, M. (2020) and related: see also "An improved parameterization method for B-spline curve and surface interpolation" (*Computer-Aided Design*, 2013) and supervised-regression parameterization (*Computers & Graphics*, 2025).

**Method.** These hybridize chord/centripetal with local-feature corrections or learn the mapping (Section 9). The 2013 method blends exponents adaptively per segment based on local point distribution.

**Results.** Small but consistent error reductions over centripetal on benchmark sets.

**Limitations.** Marginal gains; added complexity.

**Kernel relevance.** Evidence that parameterization is still an open optimization problem; supports Keel keeping the parameterizer pluggable so ML methods (Section 9) can slot in later.

---

## 3. Knot placement

### 3.1 Averaging (Piegl-Tiller) and de Boor's recommendation

**Citation.** Piegl & Tiller (1997), Eq. 9.8 (averaging); de Boor, C. (2001). *A Practical Guide to Splines* (rev. ed.). Springer.

**Method.** Given parameters `u_k` and degree `p` for **interpolation** (`n = m`), place the `n+p+1` knots by clamping the ends with multiplicity `p+1` and setting each interior knot to a moving average of `p` consecutive parameters:

```
u_0 = ... = u_p = 0,    u_{n+1} = ... = u_{n+p+1} = 1,
u_{j+p} = (1/p) * sum_{i=j}^{j+p-1} u_i ,   j = 1..n-p
```

This **guarantees the Schoenberg-Whitney condition** (every basis function's support contains its data parameter), gives a banded, well-conditioned `A`, and reflects the data distribution. de Boor's broader recommendation (the "not-a-knot" plus averaging philosophy) is to make knots track the parameter density so no basis function is starved of data. For **approximation** with `n < m`, Piegl-Tiller give a separate rule (Eq. 9.68/9.69) that selects `n+p+1` knots so each knot span contains at least one parameter, using `d = (m+1)/(n-p+1)` and `i = int(j*d)`, `alpha = j*d - i`, `u_{p+j} = (1-alpha) u_{i-1} + alpha u_i`.

**Results.** The averaging rule is the de facto default; robust, cheap, parameterization-coupled.

**Limitations.** Non-geometric ("NKTP" in recent literature): it ignores curvature, so it can over-resolve flat regions and under-resolve high-curvature features. For fixed-budget approximation it is not error-optimal.

**Kernel relevance.** This is the **default knot rule Keel must ship** for interpolation, and the safe default for least-squares approximation. It pairs with the banded solver and the Schoenberg-Whitney check.

### 3.2 Dominant-point and feature-based placement (Park & Lee)

**Citation.** Park, H., & Lee, J.-H. (2007). B-spline curve fitting based on adaptive curve refinement using dominant points. *Computer-Aided Design*, 39(6), 439-451. Earlier: Park, H. (2004). An error-bounded approximate method for representing planar curves in B-splines. *Computer Aided Geometric Design*, 21(5), 479-497; Park, H., & Lee, J.-H. (2006/2007), error-bounded approximation based on dominant point selection.

**Method.** Reframe knot placement as **dominant-point selection**. A subset of the data points are chosen as "dominant" (where the curve must be well-resolved), few in flat regions and many in complex/high-curvature regions. Knots are then obtained by **averaging the parameter values of the dominant points** (same averaging machinery as 3.1 but applied to the selected subset, not all points), and a final least-squares solve fits all data. The selection is **adaptive**: start coarse, measure the deviation, and insert new dominant points where the error exceeds the bound, exploiting the **local-modification property** (a new knot/dominant point changes the curve only locally). The four pipeline steps are: parameterization, dominant-point selection, knot placement by averaging, least-squares minimization.

**Results.** Reported to reach a target shape fidelity with **far fewer control points** than uniform or pure-averaging placement, with the error concentrated and controlled by the refinement loop.

**Limitations.** Selection heuristics (curvature/deviation thresholds) need tuning; dominant points are chosen from the input set, so very sparse input limits resolution.

**Kernel relevance.** This is the recommended approach for **error-bounded import healing and reverse engineering**: it gives compact curves (cheap downstream booleans/intersections) while meeting a tolerance. Keel should implement an adaptive dominant-point fitter as its primary "fit to tolerance" curve routine.

### 3.3 Optimization-based knot placement: free knots, metaheuristics, sparse optimization

**Citations.**
- Yoshimoto, F., Harada, T., & Yoshimoto, Y. (2003). Data fitting with a spline using a real-coded genetic algorithm. *Computer-Aided Design*, 35(8), 751-760.
- Galvez, A., & Iglesias, A. (2011). Efficient particle swarm optimization approach for data fitting with free knot B-splines. *Computer-Aided Design*, 43(12), 1683-1692. (Also Galvez et al., elitist clonal selection.)
- Yuan, Y., Chen, N., & Zhou, S. (2013). Adaptive knot placement using a GMM-based continuous optimization. *Computer-Aided Design*, 45(8).
- Kang, H., Chen, F., Li, Y., Deng, J., & Yang, Z. (2015). Knot calculation for spline fitting via sparse optimization. *Computer-Aided Design*, 58, 179-188.

**Method.** Free-knot fitting treats the knot positions as continuous optimization variables, minimizing fitting error (a famously non-convex, multimodal landscape). **Metaheuristics** (genetic algorithms, PSO, clonal selection) search the knot vector directly: each individual encodes a candidate knot set, fitness is the least-squares residual after solving for control points, and the population evolves toward low-error knot layouts. **Sparse optimization** (Kang-Deng et al.) instead places a dense set of candidate knots and selects a sparse subset by an `l1`/group-sparsity penalty: minimize fitting error plus `lambda * (sparsity of knot activations)`, recast as a convex `l1`-minimization, which automatically drops knots whose removal does not hurt the fit. A related family uses the jump in the third derivative (or a Lasso on knot indicators) to pick initial knots.

**Results.** Metaheuristics achieve lower error for a fixed knot count than averaging, at high compute cost. Sparse-optimization methods give near-optimal compact knot vectors with a convex, deterministic solve, a major robustness advantage over stochastic search.

**Limitations.** Metaheuristics are slow, non-deterministic, and hard to reproduce (bad for a kernel's regression tests). Free-knot problems can have many near-equal optima. Sparse methods need a candidate-knot discretization and a `lambda` tuning.

**Kernel relevance.** Avoid stochastic metaheuristics in the core kernel (nondeterminism breaks reproducibility). The **sparse-optimization / convex `l1` knot selection is the attractive modern option** when a high-quality compact fit matters (data reduction, file-size-sensitive export), because it is deterministic and convex. Treat as an advanced fitter, not the default.

---

## 4. Least-squares approximation

### 4.1 Fixed-knot least squares

**Citation.** Piegl & Tiller (1997), Section 9.4.1.

**Method.** Choose `n < m` (fewer control points than data), fix the knot vector (averaging rule 3.1), and minimize `sum_k |Q_k - C(u_k)|^2`. With the first and last points interpolated exactly, the interior control points solve the normal equations

```
(N^T N) P = N^T Q ,    N_{k,i} = N_{i,p}(u_k)
```

`N^T N` is `(n-1) x (n-1)`, symmetric positive definite, and **banded** (bandwidth `2p+1`), solved by banded Cholesky in `O(n p^2)`. The residual gives the fit error.

**Results.** Smooths noise, compact representation, the standard tool for measured data when a tolerance is not strictly required.

**Limitations.** No guarantee the max error meets a bound; `n` is a guess. Forming `N^T N` squares the condition number (mitigate by solving the rectangular system directly via QR when conditioning is poor).

**Kernel relevance.** Core approximation primitive. Keel should prefer a QR/least-squares solve on the rectangular `N` for numerical robustness, with banded Cholesky as a fast path.

### 4.2 Error-bounded approximation with knot-refinement loop

**Citation.** Piegl & Tiller (1997), Section 9.4.5 (approximation to within a tolerance); Park (2004), error-bounded planar B-spline representation; Razdan, A. (1999) and the adaptive-refinement literature (Li, Xu, Zhang, "Adaptive knot placement in B-spline curve approximation", *Computer-Aided Design* 37, 2005).

**Method.** Start with a coarse knot vector, least-squares fit, compute the per-point deviation `e_k = |Q_k - C(u_k)|`. While `max_k e_k > tol`: insert a knot (by knot insertion, preserving the curve) in the span with the largest error (or split the span at the worst point's parameter), refit, repeat. Because B-spline knot insertion is local, refitting after one insertion mostly perturbs the region near the new knot, so the loop converges with the minimum extra DOF needed for the tolerance. Variants insert several knots per pass guided by an error-distribution heuristic.

**Results.** Produces a curve guaranteed within `tol` using a near-minimal control-point count; the standard "fit to tolerance" algorithm in CAD.

**Limitations.** Greedy span-by-span insertion is not globally optimal in knot count (sparse optimization 3.3 does better); naive implementations refit globally each pass (`O(iterations * solve)`).

**Kernel relevance.** This refinement loop, combined with dominant-point selection (3.2), is exactly what import healing and IGES/STEP tolerant-fitting need. Keel must implement it as the primary tolerance-driven fitter and reuse it inside surface fitting (fit each iso-row to tolerance, then merge).

### 4.3 Weighted and constrained least squares

**Citation.** Piegl & Tiller (1997), Section 9.4.4 ("Approximation with constraints"); Farin (2002).

**Method.** Weighted LS minimizes `sum_k w_k |Q_k - C(u_k)|^2` (down-weight noisy points, up-weight trusted/feature points): the normal equations become `(N^T W N) P = N^T W Q` with diagonal `W`. **Constrained** LS enforces exact interpolation of selected points and/or derivatives (endpoints, feature points, tangents) as hard constraints while least-squares-fitting the rest, solved by Lagrange multipliers / KKT system or by partitioning the unknowns. Piegl-Tiller give a partition scheme: split points into `Q^l` (interpolated) and `Q^r` (approximated), move the interpolation contribution to the right-hand side, and least-squares solve the remainder.

**Results.** Lets the fitter honor known-exact geometry (a shared edge, a datum point) while smoothing the rest.

**Limitations.** Constraints reduce available DOF; over-constraining forces oscillation.

**Kernel relevance.** Essential for **continuity across patches** in reverse engineering and for honoring exact CAD constraints during healing. Keel's surface fitter must support pinning boundary curves while approximating the interior.

### 4.4 Smoothing penalties and the P-spline connection (Eilers & Marx)

**Citation.** Eilers, P. H. C., & Marx, B. D. (1996). Flexible smoothing with B-splines and penalties. *Statistical Science*, 11(2), 89-121. Follow-up: Eilers & Marx (2010/2021), *Splines, knots, and penalties* (WIREs Comput. Stat.).

**Method.** **P-splines** decouple the smoothness from knot placement. Use a generous, fixed B-spline basis (e.g., 20-50 equally spaced knots, many more than needed), and instead of choosing knots, add a **discrete difference penalty on adjacent control points** to the least-squares objective:

```
minimize  sum_k (Q_k - sum_i N_{i,p}(u_k) P_i)^2  +  lambda * sum_i (Delta^d P_i)^2
```

where `Delta^d` is the `d`-th finite difference (`d=2` is standard, penalizing curvature of the control polygon). In matrix form `(N^T N + lambda D_d^T D_d) P = N^T Q`, where `D_d` is the difference matrix. The single scalar `lambda` tunes smoothness continuously (chosen by cross-validation or AIC); `lambda -> 0` interpolates, `lambda -> inf` drives toward a degree-`d-1` polynomial fit. Eilers-Marx prove the difference penalty is a discrete analogue of the classical `integral (C''(u))^2 du` smoothing-spline penalty, so P-splines approximate smoothing splines without solving for optimal knots and with a banded, fixed system.

**Results.** Extremely robust to noise, no knot-placement decisions, banded constant-size system regardless of `lambda`, conserves moments of the data. Dominant in statistics; increasingly used in geometry.

**Limitations.** Many control points (the dense basis) even when the shape is simple, so the output is not compact unless followed by knot removal/degree reduction. `lambda` is global (locally adaptive `lambda` is an extension).

**Kernel relevance.** The right tool for **noisy scanned data and fairing** when compactness is a secondary concern: fit dense P-spline, then knot-remove (Section 7) to compact. The penalty term is also exactly the energy-fairing operator (Section 7), unifying smoothing and fairing in one formulation. Keel should implement the `(N^T N + lambda D^T D)` smoother as a shared component used by both the approximation and fairing subsystems.

---

## 5. Surface fitting

### 5.1 Tensor-product interpolation and approximation

**Citation.** Piegl & Tiller (1997), Sections 9.2.5 (global surface interpolation) and 9.4.5 (surface approximation).

**Method.** A tensor-product B-spline surface is `S(u,v) = sum_i sum_j N_{i,p}(u) N_{j,q}(v) P_{ij}`. For a **grid** of data `Q_{k,l}` (`(m+1) x (s+1)`), interpolation **separates** into 1-D solves by the tensor structure: first parameterize and fit each row (constant-`l`) in `u` to get intermediate control points, then fit each resulting column in `v`. Equivalently, solve `A_u X = Q` then `A_v P^T = X^T`, two sets of banded systems. Parameters are usually assigned by averaging the per-row and per-column chord/centripetal values so the whole grid shares one `u`-knot vector and one `v`-knot vector. Surface least-squares approximation generalizes the curve case to two banded factors.

**Results.** Same `O` cost and conditioning advantages as curve interpolation, applied twice; exact through a grid, `C^{p-1,q-1}` smooth.

**Limitations.** Requires **gridded** data. Scattered data needs Section 5.3. A single bad row parameterization corrupts a whole iso-direction.

**Kernel relevance.** The foundational surface constructor. Keel builds lofting, skinning, and Gordon/Coons-type surfaces on this tensor-product solve.

### 5.2 Lofting / skinning and the knot-compatibility problem

**Citations.**
- Piegl & Tiller (1997), Section 10.3 ("Skinned Surfaces").
- Woodward, C. D. (1988). Skinning techniques for interactive B-spline surface interpolation. *Computer-Aided Design*, 20(8).
- Park, H. (2003). Lofted B-spline surface interpolation by linearly constrained energy minimization. *Computer-Aided Design*, 35(14), 1331-1337.
- Engleitner, N., & Juttler, B. (2017). Lofting with patchwork B-splines.

**Method.** Skinning/lofting builds a surface that interpolates a sequence of **section curves** `C_0 ... C_K`. Step 1: make the sections **compatible**, meaning same degree and **identical knot vector**. Sections generally have different degrees and independent knots, so the classic approach **degree-elevates** all to the maximum degree and **merges all knot vectors** (the union of all section knots, with knot insertion bringing each curve onto the common vector). Step 2: with all sections sharing `n+1` control points over a common `u`-knot vector, parameterize across the sections (`v`-direction) and **interpolate the corresponding control points** column by column with a 1-D B-spline solve. The result is a tensor-product surface through all sections. The classic problem: union-merging knot vectors causes a **knot/control-point explosion** (the surface accumulates every section's knots). Mitigations: Piegl-Tiller's approximate skinning reduces data volume; Park (2003) finds a **common knot vector with fewer knots** and computes compatible section curves by **linearly constrained energy minimization** (fit each section to its data subject to lying on the chosen common knot vector while minimizing bending energy), then lofts. T-spline skinning (Section 5.5 of the surface literature) avoids the merge entirely by allowing T-junctions.

**Results.** Exact loft through sections; energy-minimization common-knot methods cut control-point counts dramatically versus naive union-merge while keeping fairness.

**Limitations.** Naive union-merge is heavy; reducing the common knot vector is itself an approximation (sections no longer interpolated exactly unless constrained). Section parameterization mismatch causes twist/waviness.

**Kernel relevance.** **This is a top-priority kernel operator.** Keel needs: (a) robust degree-elevation and knot-insertion to bring curves to a common knot vector, (b) a **knot-merge utility** (union with tolerance to coalesce near-equal knots and avoid near-zero spans), and (c) ideally Park-style common-knot reduction so lofted/skinned surfaces are not bloated. Knot merging with tolerance is a reusable primitive shared by lofting, Boolean edge-curve reconciliation, and import healing.

### 5.3 Scattered-data fitting: multilevel B-splines (Lee, Wolberg, Shin)

**Citation.** Lee, S., Wolberg, G., & Shin, S. Y. (1997). Scattered data interpolation with multilevel B-splines. *IEEE Transactions on Visualization and Computer Graphics*, 3(3), 228-244.

**Method.** Fit a **functional** bicubic B-spline `f(x,y) = sum sum phi_{ij} B_k(s) B_l(t)` to irregularly scattered samples `(x_c, y_c, z_c)` over a regular control lattice `Phi`. The **BA (Basic Approximation) algorithm** sets each control point by a weighted least-squares of the nearby data: for each data point spread its value to the `4x4` surrounding lattice cells weighted by the bicubic basis, then for each control point combine the contributions:

```
phi_{ij} = ( sum_c w_c^2 phi_c ) / ( sum_c w_c^2 ) ,
w_c = B_k(s_c) B_l(t_c),  phi_c = w_c z_c / (sum_{a,b} w_{ab}^2)
```

A single lattice cannot both fit sparse and dense regions, so use a **multilevel coarse-to-fine hierarchy**: lattice `Phi_0` (coarse) approximates the data, the residuals `Delta z = z - f_0` are fit by a finer lattice `Phi_1`, and so on; the final surface is the sum `f = sum_h f_h`. **B-spline refinement** then collapses the sum of all levels into **one equivalent finest-level B-spline**, so the output is a single tensor-product surface, not a sum. Complexity is `O(data + lattice)` per level.

**Results.** `C^2` surface, high-fidelity reconstruction from sparse/irregular samples, very fast (no global linear solve), interpolation in the limit and approximation at coarser truncation. Widely used (open-source `mba` library exists).

**Limitations.** **Functional** (height-field `z = f(x,y)`), not a general parametric surface, so it fits graphs/terrains/displacement, not closed or folded geometry directly. Needs a base parameterization/projection to a plane. Multilevel sum can overfit noise if carried too fine.

**Kernel relevance.** Excellent for **scattered scan data over a base surface** (displacement fitting, defeaturing residuals) and as a fast surface-from-points where a parametric domain exists. For full freeform B-rep faces Keel needs a parametric (not purely functional) fitter, but multilevel B-splines are the right engine for the height-field sub-problem and for fairing residual fields.

### 5.4 Surface adaptive knot placement (dominant columns)

**Citation.** Park, H., & Lee, J.-H. (2010). B-spline surface fitting based on adaptive knot placement using dominant columns. *Computer-Aided Design*, 42(... ). Extends the dominant-point idea to surfaces.

**Method.** Generalize dominant-point selection to a grid: choose **dominant rows and columns** that drive `u`- and `v`-knot placement adaptively, refining where surface deviation is high. Knots in each parametric direction are averaged from the selected dominant lines.

**Results.** Compact surfaces meeting a tolerance with far fewer control points than uniform tensor-product approximation.

**Limitations.** Tensor-product structure forces a knot inserted for one local feature to run across the entire surface (the global-knot-line limitation that T-splines/hierarchical B-splines remove).

**Kernel relevance.** The surface analogue of 3.2 for error-bounded face fitting. The global-knot-line limitation is the strongest practical argument for Keel to evaluate hierarchical/T-spline representations for adaptive surface fitting (covered in the freeform-surface dossier).

### 5.5 Progressive-iterative approximation (PIA / LSPIA) for surfaces

**Citation.** Lin, H., Wang, G., & Dong, C. (2004). Constructing iterative non-uniform B-spline curve and surface to fit data points. *Science in China Series F*, 47. Foundational PIA: Lin, H., Bao, H., & Wang, G. (2005). Totally positive bases and progressive iteration approximation. *Computers & Mathematics with Applications*. LSPIA: Deng, C., & Lin, H. (2014). Progressive and iterative approximation for least squares B-spline curve and surface fitting. *Computer-Aided Design*, 47, 32-44. Convergence/equivalence: Rios, D., & Juttler, B. (2022) (LSPIA = gradient descent); Lin et al., survey of PIA.

**Method.** **PIA** builds an interpolant by iteration instead of a linear solve. Initialize control points `P_i^0 = Q_i`. At iteration `k`, compute the difference at each data point `delta_i^k = Q_i - C^k(u_i)` and update the control point by that difference: `P_i^{k+1} = P_i^k + delta_i^k`. The key theorem: if the collocation matrix is built from a **normalized totally positive basis** (B-splines qualify), the iteration converges to the interpolant, with convergence rate governed by the eigenvalues of the collocation matrix (in `[0, 2)` after suitable scaling). **LSPIA** (Deng-Lin) generalizes to **least squares** with fewer control points than data: the update for control point `i` is a **weighted sum of the difference vectors of all data points in its support**,

```
P_i^{k+1} = P_i^k + mu * sum_{j in support(i)} N_{i,p}(u_j) (Q_j - C^k(u_j))
```

with step size `mu = 2 / (max row sum or spectral bound of N^T N)`. The limit is exactly the least-squares fit. Rios & Juttler showed LSPIA is gradient descent on the LS objective, explaining convergence even when `N^T N` is singular.

**Results.** No large linear solve; each iteration is a cheap weighted sum, fully local and **parallelizable** (GPU-friendly). Handles **very large data sets** robustly and supports **incremental fitting** (warm-start from the previous result when data is added). Geometrically intuitive (each step pulls the curve toward the data). Convergent for singular systems where normal equations fail.

**Limitations.** Iterative (many cheap steps vs one expensive solve); convergence rate depends on conditioning; choosing `mu` and stopping criterion matters. For small problems a direct banded solve is faster.

**Kernel relevance.** **Strongly relevant.** LSPIA is the method of choice for **massive scan-data surface fitting** and for **interactive/incremental** workflows (adding points without resolving from scratch). Its locality and GPU-friendliness fit a modern kernel; its gradient-descent interpretation makes it a natural bridge to the ML-adjacent layer (Section 9). Keel should implement LSPIA as the large-data and incremental fitter alongside the direct least-squares solver.

---

## 6. Reverse-engineering pipelines (point cloud to B-rep)

### 6.1 Classic segment-then-fit with continuity constraints

**Citation.** Varady, T., Martin, R. R., & Cox, J. (1997). Reverse engineering of geometric models: an introduction. *Computer-Aided Design*, 29(4), 255-268. Benko, P., Martin, R. R., & Varady, T. (2001). Algorithms for reverse engineering boundary representation models. *Computer-Aided Design*, 33(11).

**Method.** The canonical pipeline: (1) acquire and clean the point cloud (denoise, register, outlier removal); (2) **segment** into regions corresponding to single surfaces, by region growing on estimated normals/curvature; (3) **classify and fit** each region (plane, cylinder, cone, sphere, torus via primitive fitting; freeform via B-spline least-squares approximation, Sections 4-5); (4) reconstruct **topology** (intersect adjacent fitted surfaces to recover edges and vertices); (5) **enforce continuity/constraints** across patches: blend boundaries, impose `G^0/G^1/G^2` between adjacent B-spline patches by constraining shared boundary control rows and their tangent rows, and detect/restore design intent (parallelism, concentricity, symmetry) by constrained re-fitting. Varady-Martin-Cox formalized this as the reference workflow.

**Results.** Produces a watertight B-rep with engineering constraints honored; the basis of commercial reverse-engineering tools (Geomagic, etc.).

**Limitations.** Segmentation is the hard, error-prone step; continuity enforcement across many patches is a coupled constrained-optimization problem; sharp-edge recovery is fragile.

**Kernel relevance.** Defines what Keel's import/reverse-engineering subsystem must do end to end. The **cross-patch continuity-constrained fitting** (constrained LS from 4.3, shared boundary curves from lofting 5.2) is the kernel-side core; Keel must let the fitter pin a boundary curve and match cross-boundary tangents while approximating interior data.

### 6.2 Learning-based full-pipeline reconstruction

**Citations.**
- Sharma, G., et al. (2020). ParSeNet: A parametric surface fitting network for 3D point clouds. *ECCV 2020*.
- Guo, H., et al. (2022). ComplexGen: CAD reconstruction by B-rep chain complex generation. *ACM TOG (SIGGRAPH 2022)*.
- Liu, Y., et al. (2024). Point2CAD: Reverse engineering CAD models from 3D point clouds. *CVPR 2024* (arXiv:2312.04962).

**Method.** Neural networks replace or augment segmentation and fitting. **ParSeNet** jointly segments a point cloud into patches and predicts open/closed B-spline patch parameters with a differentiable spline-fitting module (so reconstruction loss backpropagates into segmentation). **Point2CAD** segments points into surface clusters, fits each with primitive fitting **plus a neural freeform-surface fitter**, then **intersects adjacent surfaces** analytically to recover edges and corners, yielding a full B-rep with topology, addressing the long-standing weakness that learned methods got geometry but not topology. **ComplexGen** generates the B-rep **chain complex** (vertices, edges, faces and their incidence) jointly, treating reconstruction as structured prediction.

**Results.** State-of-the-art segmentation and increasingly reliable topology; Point2CAD reports full B-rep recovery on the ABC dataset with edges/corners, a step beyond patch-only methods.

**Limitations.** Still struggles with exact constraints, sharp features, and guaranteeing watertight valid topology; need large training data; outputs need kernel-side validation/healing.

**Kernel relevance.** These are **front ends that feed Keel**: the kernel must consume their patch+topology output and **validate, intersect, and heal** it into a valid B-rep. The differentiable fitting modules (ParSeNet, NURBS-Diff in 9.1) argue for Keel exposing a differentiable evaluation path so learned front ends can be trained against the kernel's own geometry.

---

## 7. Curve and surface fairing

### 7.1 Energy-functional fairing

**Citation.** Hagen, H., & Schulze, G. (1987); Sapidis, N. (Ed.) (1994). *Designing Fair Curves and Surfaces*. SIAM. Veltkamp, R. C., & Wesselink, W. (1995). Modeling 3D curves of minimal energy. Greiner, G. (1994), variational design. "Fairing spline curves and surfaces by minimizing energy" (*Computer-Aided Design*, 2001).

**Method.** Improve "fairness" (smoothness of curvature) by minimizing a bending/stretching energy. The classic linearized bending energy is `E = integral |C''(u)|^2 du` (curve) or the thin-plate `E = integral (S_uu^2 + 2 S_uv^2 + S_vv^2) du dv` (surface). Fitting becomes minimize `data-fidelity + lambda * E`, which is **exactly the smoothing-spline / P-spline objective** (Section 4.4): the discretized energy is `P^T K P` with a banded stiffness matrix `K`, so the solve is `(N^T N + lambda K) P = N^T Q`. True (nonlinear) fairness uses the geometric curvature `kappa`, giving a nonlinear functional solved by iteration. Local energy fairing perturbs one control point at a time to minimize the energy increment, giving an `O(1)` local update per step.

**Results.** Removes curvature wiggles, produces aesthetically and aerodynamically "fair" curves/surfaces; `lambda` trades fidelity for smoothness.

**Limitations.** Linearized energy is only valid for nearly arc-length parameterization; over-smoothing flattens features; global solves are expensive for large surfaces (local fairing mitigates).

**Kernel relevance.** Fairing is a required post-process for lofted/skinned surfaces and for healed import geometry. Because the operator is the same `(N^T N + lambda K)` system as the P-spline smoother, Keel should implement **one penalized-least-squares engine** serving smoothing, approximation, and fairing.

### 7.2 Knot-removal-based fairing (Farin-Sapidis, Lyche-Morken, Eck-Hadenfeld)

**Citations.**
- Farin, G., & Sapidis, N. (1989). Curvature and the fairness of curves and surfaces. *IEEE CG&A*, 9(2).
- Lyche, T., & Morken, K. (1987). Knot removal for parametric B-spline curves and surfaces. *Computer Aided Geometric Design*, 4(3), 217-230.
- Eck, M., & Hadenfeld, J. (1995). Knot removal for B-spline curves. *Computer Aided Geometric Design*, 12(3), 259-282.

**Method.** Fairing by reducing complexity: **knot removal** is the inverse of knot insertion. To remove an interior knot, test whether the curve can be re-expressed on the reduced knot vector within a tolerance (solve the over-determined system that drops one basis function and measure the deviation `||removed||`). Lyche-Morken give the ranked, tolerance-bounded removal of the **maximum number of knots** while staying within a global error band, with explicit error bounds per removal. Eck-Hadenfeld extend with a least-squares knot-removal that minimizes the perturbation. Farin-Sapidis fair by **removing then reinserting** a knot: the difference between the original and reinserted control point flags a "bad" point; replacing it with the faired value smooths the curve locally while keeping the knot structure.

**Results.** Compacts data and improves fairness simultaneously; Lyche-Morken's bounded removal is a standard data-reduction tool; Farin-Sapidis localizes fairing to flagged points.

**Limitations.** Knot removal is constrained by tolerance (cannot remove more than the data allows); reinsertion fairing handles one defect at a time.

**Kernel relevance.** Knot removal is a **must-have kernel utility**: it compacts P-spline/multilevel/over-fit results, reduces lofted-surface bloat (pairs with the merge problem in 5.2), and shrinks export file size. Implement Lyche-Morken bounded removal as the standard data-reduction pass and Farin-Sapidis reinsertion as a targeted fairing tool.

### 7.3 Curvature-based metrics and automatic fairing

**Citation.** "Automatic fairing algorithm for B-spline curves" (*Computer-Aided Design*, 22, 1990); target-curvature-driven fairing (*Computer Aided Geometric Design*, 21, 2004).

**Method.** Detect unfair regions by inspecting the **curvature plot** (curvature comb): isolated spikes or sign changes in `kappa(u)` mark defects. Automatic algorithms iterate: find the worst curvature-plot feature, fair it locally (energy or knot-reinsertion), recheck, until the curvature plot is monotone-piecewise/clean.

**Results.** Objective, automatable fairness criterion tied to the curvature comb designers already use.

**Limitations.** Curvature estimation is noise-sensitive; "fair" is partly subjective.

**Kernel relevance.** Keel should expose curvature-comb evaluation (cheap from the analytic B-spline derivatives) as both a diagnostic and the driver of automatic fairing.

---

## 8. Approximation of analytic curves and surfaces by NURBS

### 8.1 Exact conics vs integral-spline approximation

**Citation.** Piegl & Tiller (1997), Sections 1.4 and 7.5 (exact rational conics/circles); Lee, E. T. Y. (1987). The rational Bezier representation for conics. In *Geometric Modeling*. SIAM.

**Method.** A circle/arc is **exactly** a rational (NURBS) curve. A full circle is representable as a degree-2 rational B-spline with 9 control points (4 arcs of 90 deg) or 7 control points (3 arcs of 120 deg), with the well-known weights `w = cos(theta/2)` at the off-curve control points. Conics generally are degree-2 NURBS with one interior weight encoding eccentricity (`w<1` ellipse, `w=1` parabola, `w>1` hyperbola). This is exact, compact, and the standard CAD representation of analytic edges.

**Results.** Exact, minimal data, the reason NURBS unified analytic and freeform geometry.

**Limitations.** Rationality complicates downstream math: rational derivatives, offsets, and intersections are heavier; weights must be carried and merged consistently; a full circle needs interior knots (cannot be a single Bezier).

**Kernel relevance.** Keel must store analytic edges as exact NURBS conics for fidelity, but be aware that rationality costs propagate into every algorithm.

### 8.2 Circle/arc approximation by integral (non-rational) B-splines

**Citation.** Piegl, L., & Tiller, W. (2003). Circle approximation using integral B-splines. *Computer-Aided Design*, 35(6), 601-607.

**Method.** Approximate a circular arc of arbitrary sweep with an **integral** (non-rational) B-spline of **any chosen degree and continuity**, by interpolating end derivatives plus internal points sampled uniformly from the trigonometric circle equation `(R cos t, R sin t)`. Because the lowest-degree exact rational circle is degree 5 (with no internal knots) and the popular degree-2/3 circles are only approximate anyway when one wants nonrational form, this method gives a controllable-accuracy degree-`p`, `C^k` integral approximation. The user trades degree/continuity/segment count against error.

**Results.** Arbitrary accuracy at chosen degree; produces non-rational circles that avoid weight bookkeeping.

**Limitations.** Approximate (never exact, unlike the rational form); more control points than the rational conic for the same arc.

**Kernel relevance.** Important because **many algorithms (offsetting, tessellation, some intersections) are far simpler on integral splines** than rational ones. Keel can keep exact rational conics for storage/exchange but convert to integral-spline approximations within a tolerance for algorithm-internal use, dropping the "R" from NURBS where rationality is not needed. This is a concrete performance lever.

---

## 9. Degree reduction and merging as approximation problems

### 9.1 Degree reduction

**Citations.**
- Eck, M. (1995). Least squares degree reduction of Bezier curves. *Computer-Aided Design*, 27(11), 845-851.
- Piegl, L., & Tiller, W. (1995). Algorithm for degree reduction of B-spline curves. *Computer-Aided Design*, 27(2), 101-110.
- Ahn, Y. J., et al. (constrained multi-degree reduction, `G^{k,l}` continuity).

**Method.** Lowering degree from `p` to `p-1` (or `m<p`) cannot generally be exact, so it is an approximation problem. The B-spline approach **decomposes into Bezier segments** (knot insertion to full multiplicity), degree-reduces each Bezier piece, then **removes the now-unnecessary knots** (Section 7.2) to recover a B-spline. Bezier degree reduction in the `L2` norm is equivalent to the best **weighted Euclidean approximation of the Bernstein-Bezier coefficients** (Eck), with endpoint and derivative continuity imposed as constraints; the constrained problem reduces to a constrained least-squares on coefficients.

**Results.** Near-optimal `L2` reduced curve with controlled boundary continuity; error bounds available per segment.

**Limitations.** Error grows with the degree gap; continuity constraints consume DOF; the decompose-reduce-remove pipeline can be fiddly at high multiplicity.

**Kernel relevance.** Needed for **data exchange** (down-convert high-degree imports), for normalizing to a kernel-preferred degree, and as a building block. Pairs with knot removal as a shared simplification toolkit.

### 9.2 Curve merging

**Citation.** Hu, S.-M., et al. (2001/2003). Approximate merging of B-spline curves via knot adjustment and constrained optimization. *Computer-Aided Design*, 35(...). Approximate merging of a pair of Bezier curves (Tsinghua group).

**Method.** Merge two adjacent curves into one. Find the conditions for **exact** merge (matching degree, knot vectors, and continuity at the join), then when exact merge is impossible **perturb control points by constrained optimization** to minimize the deviation from both originals subject to continuity at the seam, choosing a merged knot vector by knot adjustment. The objective is a least-squares deviation; constraints fix endpoints and seam continuity (`G^1`/`G^2`).

**Results.** Single compact curve approximating two, with bounded error and prescribed continuity; reduces segment count after Boolean/trim operations.

**Limitations.** Approximate; error depends on how dissimilar the seam tangents are; choosing the merged knot vector is a heuristic.

**Kernel relevance.** Directly useful after **Boolean and trimming operations** that fragment edges into many small pieces: merging recombines them into compact edges, lowering downstream cost. A practical post-Boolean simplification pass for Keel.

---

## 10. ML-adjacent fitting

### 10.1 NURBS-Diff: a differentiable NURBS layer

**Citation.** Prasad, A. R. V., Balu, A., Shah, H., Sarkar, S., Hegde, C., & Krishnamurthy, A. (2022). NURBS-Diff: A differentiable programming module for NURBS. *Computer-Aided Design*, 146, 103199 (arXiv:2104.14547, 2021).

**Method.** Define the derivatives of a NURBS curve/surface point with respect to the inputs (control points, weights, knots, evaluation parameter), and assemble an **approximate Jacobian** so that evaluation `(control points, weights, knots) -> sampled points` is **differentiable end to end**. Implemented as a GPU-accelerated **PyTorch layer**: forward pass evaluates the NURBS surface, backward pass propagates gradients of a loss (e.g., Chamfer distance to a point cloud) back to the NURBS parameters. This lets gradient descent **fit** a NURBS surface to data and lets the NURBS evaluator sit **inside a neural network** so reconstruction networks output valid NURBS.

**Results.** Demonstrated on curve/surface fitting, surface offsetting, and **unsupervised point-cloud reconstruction**; the differentiable layer fits CAD-standard NURBS directly rather than meshes/implicits.

**Limitations.** Jacobian is approximate (knot derivatives are tricky); gradient-based fitting inherits local-minima and parameterization issues; not faster than classical solvers for clean small problems.

**Kernel relevance.** Strong fit for Keel's ML-adjacency goal: exposing a **differentiable NURBS evaluation API** lets learned reverse-engineering front ends (Section 6.2) train against Keel's own geometry and output kernel-native NURBS. LSPIA (5.5) being gradient descent means a differentiable layer and the classical iterative fitter share math, an opportunity for one unified engine.

### 10.2 Deep learning for parameterization and knot prediction

**Citations.**
- Laube, P., Franz, M. O., & Umlauf, G. (2018). Deep learning parametrization for B-spline curve approximation. *International Conference on 3D Vision (3DV)* (arXiv:1807.08304).
- Laube et al. (2018). Learnt knot placement in B-spline curve approximation using support vector machines. *Computer Aided Geometric Design*.
- Scholz, F., & Juttler, B. (2021). Parameterization for polynomial curve approximation via residual deep neural networks. *Computer Aided Geometric Design*, 85.
- Luo, J., et al. (2022). A deep neural network for knot placement in B-spline approximation (arXiv:2205.02978).
- SplineGen: a generative model for B-spline approximation of unorganized points (arXiv:2406.09692, 2024).

**Method.** Learn the two classically heuristic decisions, parameterization and knot placement, from data. Laube's 2018 method uses two coupled networks: a **Point Parametrization Network (PPN)** assigning `u_k` to each point, and a **Knot Selection Network (KSN)** predicting refined knots; trained to minimize the downstream approximation error. The SVM variant classifies where knots should go from local geometric features. Scholz-Juttler use a residual DNN to output parameters directly. **SplineGen** casts parameterization and knot placement jointly as a **sequence-to-sequence translation** so parameters and knots are generated in alignment, improving accuracy over treating them separately.

**Results.** Consistently smaller approximation error than centripetal/averaging baselines on benchmark curve sets; SplineGen handles unorganized points end to end.

**Limitations.** Need training data and generalize imperfectly to out-of-distribution shapes; nondeterministic relative to a closed-form rule (a concern for kernel reproducibility); benefits are modest on clean engineering data where classical rules already work.

**Kernel relevance.** These target exactly the two decisions Keel keeps pluggable (Sections 2-3). The practical takeaway: keep the **parameterizer and knot-placer as swappable strategy interfaces** so a learned model can replace the centripetal/averaging defaults later, while the deterministic classical methods remain the verifiable baseline the kernel ships and tests against.

---

## 11. What Keel needs from fitting (synthesis)

Priorities, in build order, derived from the literature above.

**Tier 1, foundational primitives (ship first).**
- Banded LU / Cholseky and rectangular QR least-squares solvers as a shared numerics component (1.1, 4.1). One engine, reused everywhere.
- Global curve interpolation with all four end conditions (1.1, 1.2) and a hard Schoenberg-Whitney validity check with graceful fallback (1.3).
- Parameterization as a swappable strategy with **centripetal default**, plus chord-length and uniform (2.1); keep the interface open for Foley-Nielson, universal, and learned methods (2.2-2.4, 10.2).
- Knot placement by **averaging** as default, with the Schoenberg-Whitney guarantee it provides (3.1).
- Tensor-product surface interpolation/approximation via separated 1-D solves (5.1).

**Tier 2, the approximation and tolerance machinery (the real workhorses).**
- Fixed-knot least squares plus the **error-bounded knot-refinement loop** (4.1-4.2) as the primary "fit to tolerance" routine.
- **Dominant-point / dominant-column adaptive fitting** (3.2, 5.4) for compact tolerance-bounded curves and faces, the core of import healing and reverse engineering.
- **Weighted and constrained least squares** (4.3) to pin exact points/edges and enforce cross-patch continuity.
- A single **penalized-least-squares engine** `(N^T N + lambda(D^T D or K)) P = N^T Q` serving P-spline smoothing, energy fairing, and noisy-data approximation (4.4, 7.1). This unification is a real architectural simplification.
- **LSPIA** (5.5) for massive scan data and incremental/interactive fitting; its gradient-descent nature also seeds the ML-adjacent path.

**Tier 3, surface construction operators (what a kernel is judged on).**
- Robust **degree elevation, knot insertion, and tolerant knot merging** (5.2) as shared primitives.
- **Lofting / skinning** with section-curve compatibility and Park-style common-knot reduction to avoid control-point explosion (5.2). Top-priority surface operator.
- **Multilevel B-splines** (5.3) as the height-field / scattered-residual engine.

**Tier 4, simplification, fairing, and exchange.**
- **Knot removal** (Lyche-Morken bounded, Eck-Hadenfeld least-squares) as the universal compaction pass after over-fitting, lofting, and Booleans (7.2).
- **Energy / curvature-comb fairing** sharing the penalized-LS engine (7.1, 7.3).
- **Degree reduction and curve merging** for data exchange and post-Boolean simplification (9.1, 9.2).
- **Exact rational conics** for storage with **integral-spline approximation** for algorithm-internal use where rationality is a tax (8.1, 8.2).

**Tier 5, ML adjacency (design for it now, build later).**
- Expose a **differentiable NURBS evaluation API** (NURBS-Diff style, 10.1) so learned reverse-engineering front ends (ParSeNet, Point2CAD, ComplexGen, 6.2) can train against Keel's geometry and emit kernel-native NURBS, and so learned parameterizers/knot-placers (10.2) can slot into the Tier-1 strategy interfaces.

**Cross-cutting robustness requirements.** Validate Schoenberg-Whitney everywhere; coalesce near-duplicate points and near-zero knot spans with tolerance; prefer QR over normal equations when conditioning is marginal; keep all heuristic decisions (parameterization, knots, end conditions, weights) as inspectable, switchable, deterministic strategies so the kernel stays reproducible and testable, with learned methods as opt-in alternatives rather than the default.

---

## References

1. Schoenberg, I. J., & Whitney, A. (1953). On Polya frequency functions III. *Transactions of the American Mathematical Society*, 74(2), 246-259.
2. Lyche, T., & Morken, K. (1987). Knot removal for parametric B-spline curves and surfaces. *Computer Aided Geometric Design*, 4(3), 217-230.
3. Farin, G., & Sapidis, N. (1989). Curvature and the fairness of curves and surfaces. *IEEE Computer Graphics and Applications*, 9(2), 52-57.
4. Foley, T. A., & Nielson, G. M. (1989). Knot selection for parametric spline interpolation. In *Mathematical Methods in Computer Aided Geometric Design* (pp. 261-272). Academic Press.
5. Lee, E. T. Y. (1989). Choosing nodes in parametric curve interpolation. *Computer-Aided Design*, 21(6), 363-370.
6. Sapidis, N. (Ed.) (1994). *Designing Fair Curves and Surfaces*. SIAM.
7. Eck, M. (1995). Least squares degree reduction of Bezier curves. *Computer-Aided Design*, 27(11), 845-851.
8. Eck, M., & Hadenfeld, J. (1995). Knot removal for B-spline curves. *Computer Aided Geometric Design*, 12(3), 259-282.
9. Piegl, L., & Tiller, W. (1995). Algorithm for degree reduction of B-spline curves. *Computer-Aided Design*, 27(2), 101-110.
10. Eilers, P. H. C., & Marx, B. D. (1996). Flexible smoothing with B-splines and penalties. *Statistical Science*, 11(2), 89-121.
11. Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer.
12. Lee, S., Wolberg, G., & Shin, S. Y. (1997). Scattered data interpolation with multilevel B-splines. *IEEE Transactions on Visualization and Computer Graphics*, 3(3), 228-244.
13. Varady, T., Martin, R. R., & Cox, J. (1997). Reverse engineering of geometric models: an introduction. *Computer-Aided Design*, 29(4), 255-268.
14. Lim, C.-G. (1999). A universal parametrization in B-spline curve and surface interpolation. *Computer Aided Geometric Design*, 16(5), 407-422.
15. Hu, S.-M., et al. (2003). Approximate merging of B-spline curves via knot adjustment and constrained optimization. *Computer-Aided Design*.
16. de Boor, C. (2001). *A Practical Guide to Splines* (rev. ed.). Springer.
17. Benko, P., Martin, R. R., & Varady, T. (2001). Algorithms for reverse engineering boundary representation models. *Computer-Aided Design*, 33(11), 839-851.
18. Farin, G. (2002). *Curves and Surfaces for CAGD: A Practical Guide* (5th ed.). Morgan Kaufmann.
19. Piegl, L., & Tiller, W. (2003). Circle approximation using integral B-splines. *Computer-Aided Design*, 35(6), 601-607.
20. Yoshimoto, F., Harada, T., & Yoshimoto, Y. (2003). Data fitting with a spline using a real-coded genetic algorithm. *Computer-Aided Design*, 35(8), 751-760.
21. Park, H. (2003). Lofted B-spline surface interpolation by linearly constrained energy minimization. *Computer-Aided Design*, 35(14), 1331-1337.
22. Lin, H., Wang, G., & Dong, C. (2004). Constructing iterative non-uniform B-spline curve and surface to fit data points. *Science in China Series F*, 47.
23. Li, W., Xu, S., Zhao, G., & Goh, L. P. (2005). Adaptive knot placement in B-spline curve approximation. *Computer-Aided Design*, 37(8), 791-797.
24. Park, H., & Lee, J.-H. (2007). B-spline curve fitting based on adaptive curve refinement using dominant points. *Computer-Aided Design*, 39(6), 439-451.
25. Galvez, A., & Iglesias, A. (2011). Efficient particle swarm optimization approach for data fitting with free knot B-splines. *Computer-Aided Design*, 43(12), 1683-1692.
26. Deng, C., & Lin, H. (2014). Progressive and iterative approximation for least squares B-spline curve and surface fitting. *Computer-Aided Design*, 47, 32-44.
27. Kang, H., Chen, F., Li, Y., Deng, J., & Yang, Z. (2015). Knot calculation for spline fitting via sparse optimization. *Computer-Aided Design*, 58, 179-188.
28. Laube, P., Franz, M. O., & Umlauf, G. (2018). Deep learning parametrization for B-spline curve approximation. *International Conference on 3D Vision (3DV)*. arXiv:1807.08304.
29. Sharma, G., et al. (2020). ParSeNet: A parametric surface fitting network for 3D point clouds. *ECCV 2020*.
30. Scholz, F., & Juttler, B. (2021). Parameterization for polynomial curve approximation via residual deep neural networks. *Computer Aided Geometric Design*, 85.
31. Prasad, A. R. V., Balu, A., Shah, H., Sarkar, S., Hegde, C., & Krishnamurthy, A. (2022). NURBS-Diff: A differentiable programming module for NURBS. *Computer-Aided Design*, 146, 103199. arXiv:2104.14547.
32. Guo, H., et al. (2022). ComplexGen: CAD reconstruction by B-rep chain complex generation. *ACM Transactions on Graphics (SIGGRAPH 2022)*. arXiv:2205.14573.
33. Luo, J., et al. (2022). A deep neural network for knot placement in B-spline approximation. arXiv:2205.02978.
34. Rios, D., & Juttler, B. (2022). LSPIA, (stochastic) gradient descent, and parameter correction. (LSPIA equivalence to gradient descent.)
35. Liu, Y., et al. (2024). Point2CAD: Reverse engineering CAD models from 3D point clouds. *CVPR 2024*. arXiv:2312.04962.
36. SplineGen: a generative model for B-spline approximation of unorganized points (2024). arXiv:2406.09692.
