# Core NURBS / B-Spline Algorithms: Evaluation, Refinement, Conversion

Research dossier for the **Keel** B-rep kernel. This document goes one level deeper than the
high-level survey (`docs/research/b-nurbs-freeform.md` and the synthesis): it records the full
algorithmic recurrences, the numerical-stability literature behind them, complexity, limitations,
and concrete implementation guidance for a Parasolid-class kernel.

Scope: basis and point/derivative evaluation (Cox de Boor, de Boor, de Casteljau, rational and
derivative families); numerical stability of corner-cutting schemes (Farouki and Rajan; Mainar and
Pena; compensated algorithms); knot insertion (Boehm, Oslo); knot removal (Lyche and Morken);
degree elevation (Prautzsch; Piegl and Tiller; Huang, Hu and Martin) and degree reduction (Eck;
Watkins and Worsey); Bezier decomposition and extraction (Borden et al.); subdivision; basis
conversion conditioning; blossoming and Marsden's identity; library and GPU implementation studies.

Notation used throughout: degree `p`, order `k = p + 1`, control points `P_i`, weights `w_i`,
knot vector `U = {u_0, ..., u_m}` with `m = n + p + 1`, parameter `u`. Homogeneous (4D) control
points are written `P^w_i = (w_i x_i, w_i y_i, w_i z_i, w_i)`.

---

## 1. Foundations: basis evaluation and the recurrence backbone

### 1.1 Cox de Boor recursion (basis function evaluation)

**Citation.** de Boor, C. (1972). On calculating with B-splines. *Journal of Approximation
Theory*, 6(1), 50-62. Cox, M. G. (1972). The numerical evaluation of B-splines. *IMA Journal of
Applied Mathematics*, 10(2), 134-149. Textbook treatment: Piegl, L., & Tiller, W. (1997).
*The NURBS Book* (2nd ed.). Springer (Algorithm A2.2).

**Algorithm / result.** The B-spline basis functions are defined by the two-term recurrence

```
N_{i,0}(u) = 1 if u_i <= u < u_{i+1}, else 0
N_{i,p}(u) = (u - u_i)/(u_{i+p} - u_i)       * N_{i,p-1}(u)
           + (u_{i+p+1} - u)/(u_{i+p+1} - u_{i+1}) * N_{i+1,p-1}(u)
```

with the 0/0 convention treated as 0. Naive recursion is exponential and recomputes shared
subexpressions. Practical kernels never evaluate one basis function at a time; they use the
**triangular table** form (NURBS Book A2.2) that, given the knot span index, computes all `p + 1`
nonzero functions `N_{i-p..i, p}(u)` at once via a left/right array:

```
N[0] = 1
for j in 1..p:
    left[j]  = u - U[span+1-j]
    right[j] = U[span+j] - u
    saved = 0
    for r in 0..j-1:
        temp   = N[r] / (right[r+1] + left[j-r])
        N[r]   = saved + right[r+1]*temp
        saved  = left[j-r]*temp
    N[j] = saved
```

Finding the span (A2.1) is a binary search in `O(log n)`.

**Complexity / numerics.** All nonzero basis functions: `O(p^2)`. Only divisions by knot
differences appear, all of which are positive on a valid (non-degenerate) span; the scheme is a
convex corner-cutting process so it is numerically benign. Each output lies in `[0,1]` and the
`p + 1` values sum to exactly 1 in exact arithmetic (partition of unity); in floating point the sum
is `1 + O(p * eps)`, which is a useful internal consistency check.

**Kernel relevance.** This is the single most-called numeric routine in a NURBS kernel. Keel should
implement the A2.1/A2.2 pair with stack-allocated `left/right` arrays (no heap traffic in the hot
path), and offer a fused "basis + derivatives" variant (A2.3) so that point and tangent evaluation
share the triangular table.

### 1.2 de Boor's algorithm (curve point evaluation)

**Citation.** de Boor, C. (1972), as above; NURBS Book A3.1; MIT hyperbook (Patrikalakis, Maekawa,
Cho, *Shape Interrogation for Computer Aided Design and Manufacturing*, node 18).

**Algorithm / result.** Rather than forming basis functions and dotting them against control points,
de Boor's algorithm directly interpolates control points in a triangular corner-cutting scheme. With
`span = k` such that `u in [u_k, u_{k+1})`, set `d^0_i = P_i` for `i = k-p .. k`, then for levels
`r = 1 .. p`:

```
d^r_i = (1 - a) * d^{r-1}_{i-1} + a * d^{r-1}_i
a     = (u - u_i) / (u_{i+p+1-r} - u_i)
```

and `C(u) = d^p_k`. This is exactly the generalization of de Casteljau to nonuniform knots: every
step is a convex combination (`a in [0,1]` for `u` inside the span).

**Complexity / numerics.** `O(p^2)` per point for the corner cutting, plus `O(log n)` span search.
Because every operation is a convex combination of points already on or near the curve, the scheme
is backward stable in the corner-cutting sense (see Section 2): no cancellation, bounded error
growth. It is generally preferred over "basis dot control points" when only the point is needed,
because it avoids forming and storing the basis array and keeps all intermediate quantities on the
geometry scale.

**Kernel relevance.** Use de Boor (A3.1) for bare point evaluation; use the basis-function route
(A2.2 + dot) when basis values are needed anyway (e.g. for fitting, IGA assembly, or simultaneous
multi-control-point sensitivity). Surface evaluation (A3.5) is the tensor product: run de Boor in
`u` on each of the `q + 1` affected rows, then once in `v`, cost `O(p^2 q + q^2)`.

### 1.3 de Casteljau's algorithm (Bezier evaluation, the Bernstein special case)

**Citation.** de Casteljau, P. (1959/1963), internal Citroen reports; standard treatment in
Farin, G. (2002). *Curves and Surfaces for CAGD* (5th ed.), Morgan Kaufmann.

**Algorithm / result.** For a degree-`p` Bezier curve with control points `b^0_i = P_i`:

```
b^r_i(t) = (1 - t) * b^{r-1}_i + t * b^{r-1}_{i+1},  r = 1..p, i = 0..p-r
C(t) = b^p_0
```

It is de Boor specialized to the clamped knot vector `{0,...,0,1,...,1}` and is the per-span engine
behind Bezier extraction (Section 6). The same triangle also yields subdivision: the left and right
edges of the triangle (`b^r_0` and `b^{p-r}_r`) are the control points of the two halves at `t`.

**Complexity / numerics.** `O(p^2)`. Each `b^r_i(t)` is a convex combination for `t in [0,1]`, so
all intermediate points stay inside the convex hull of the input. This convexity is the source of
its excellent stability and is exactly what Farouki and Rajan quantify (Section 2). Horner
evaluation of the same polynomial in the monomial basis can be cheaper (`O(p)`) but is far less
stable; de Casteljau trades a factor of `p` in work for robustness.

**Kernel relevance.** Bezier is the canonical "leaf" representation. A kernel that decomposes NURBS
into Bezier segments (per-span) can run de Casteljau on each, which is the most stable evaluation
path and the natural unit for intersection, tessellation, and clipping (Bezier clipping).

---

## 2. Numerical stability of evaluation and corner-cutting schemes

### 2.1 Conditioning of the Bernstein basis (the foundational result)

**Citation.** Farouki, R. T., & Rajan, V. T. (1987). On the numerical condition of polynomials in
Bernstein form. *Computer Aided Geometric Design*, 4(3), 191-216. Farouki, R. T., & Rajan, V. T.
(1988). Algorithms for polynomials in Bernstein form. *Computer Aided Geometric Design*, 5(1),
1-26.

**Result.** These two papers established why CAGD uses Bernstein/Bezier rather than the power
(monomial) basis. The 1987 paper defines condition numbers for a polynomial's roots and values under
coefficient perturbation and proves that for any polynomial that is positive on `[0,1]`, the
Bernstein-form condition number is never worse, and is frequently many orders of magnitude better,
than the power-basis condition number. The basis-transformation matrix from power to Bernstein has
entries that grow combinatorially, so converting a well-conditioned Bernstein polynomial into
monomial form can inflate the condition number exponentially in the degree. The 1988 paper supplies
the companion arithmetic (multiplication, division, degree raising, GCD, root finding) directly in
Bernstein form so that one never has to leave the well-conditioned basis.

**Significance / kernel relevance.** This is the theoretical license for representing all polynomial
geometry in Bernstein/B-spline form inside a kernel. The practical mandate for Keel: **never convert
spline geometry to monomial/power basis for evaluation, intersection, or root finding.** Keep
everything in B-spline or per-span Bezier form; if a power-form is unavoidable (e.g. interfacing a
legacy solver) restrict it to low degree and treat results as suspect.

### 2.2 Bernstein form is optimally stable, and a centennial survey

**Citation.** Farouki, R. T., & Goodman, T. N. T. (1996). On the optimal stability of the Bernstein
basis. *Mathematics of Computation*, 65(216), 1553-1566. Farouki, R. T. (2012). The Bernstein
polynomial basis: a centennial retrospective. *Computer Aided Geometric Design*, 29(6), 379-419.

**Result.** Farouki and Goodman proved that among all nonnegative bases of the degree-`p` polynomial
space on `[0,1]`, the Bernstein basis is *optimally stable*: no other nonnegative basis has a
uniformly smaller condition number for the value map. The 2012 retrospective is the best single
reference for the whole topic, collecting conditioning, the de Casteljau stability analysis, basis
conversions, intersection and root isolation, and the multivariate (triangular and tensor) cases.

**Kernel relevance.** Settles the basis question definitively. Use the centennial paper as the
canonical citation in Keel's design notes when justifying the representation choice.

### 2.3 Error analysis of corner-cutting algorithms

**Citation.** Mainar, E., & Pena, J. M. (1999). Error analysis of corner cutting algorithms.
*Numerical Algorithms*, 22(1), 41-52. Related: Delgado, J., & Pena, J. M. (2003). Running error
analysis of evaluation algorithms for bivariate polynomials in barycentric Bernstein form.
*Computing*. Mainar, E., & Pena, J. M. (2007). A corner cutting algorithm for evaluating rational
Bezier surfaces and the optimal stability of the basis. *SIAM Journal on Scientific Computing*,
30(3).

**Result.** Mainar and Pena gave a unified backward and forward error analysis for the whole family
of corner-cutting (convex-combination) algorithms, which includes de Casteljau, de Boor, and the
rational variants. The central conclusion: corner-cutting schemes are backward stable, with relative
error bounds that grow only linearly in `p` (proportional to `p * eps` times a modest constant)
because each step is a convex combination and therefore introduces no subtractive cancellation. They
also derive *running error bounds*: the algorithm can compute a certified a posteriori error bound
alongside the value at negligible extra cost. The 2007 paper extends this to rational Bezier
surfaces and proves optimal stability of the rational Bernstein basis.

**Kernel relevance.** Justifies preferring de Boor/de Casteljau over basis-then-dot or monomial
Horner whenever robustness matters (intersection seeds, boundary evaluation near tolerances). The
running-error idea is worth adopting: Keel can return a conservative point-evaluation error bar that
downstream tolerant algorithms (intersection, healing) consume directly.

### 2.4 Modern synthesis of de Casteljau accuracy

**Citation.** Delgado, J., Mainar, E., & Pena, J. M. (2023). On the accuracy of de Casteljau-type
algorithms and Bernstein representations. *Computer Aided Geometric Design*, 104, 102211.

**Result.** A 2023 survey that consolidates two decades of results: de Casteljau-type algorithms are
numerically stable under perturbed data and rounding; Bernstein-type bases are optimally stable for
evaluation among nonnegative bases; and there now exist algorithms adapted to the *totally positive*
matrix structure of Bernstein collocation that solve interpolation and least-squares to high
*relative* accuracy (not just high absolute accuracy). It links conditioning of the basis to the
backward stability of the evaluation algorithm in one coherent picture.

**Kernel relevance.** The most current single citation for "why our evaluator is trustworthy." The
total-positivity-aware solvers are relevant to Keel's fitting/approximation subsystem, where
accurate interpolation at high degree otherwise loses relative precision.

### 2.5 Compensated de Casteljau (when ordinary stability is not enough)

**Citation.** Jiang, H., Barrio, R., Li, H., Liao, X., Cheng, L., & Su, F. (2010). Accurate
evaluation of a polynomial in Chebyshev/Bernstein form. Hefferon-style follow-ups; and Jiang, H.,
Graillat, S., et al. (2019). Compensated de Casteljau algorithm in K times the working precision.
*Applied Mathematics and Computation*, 357, 57-74 (arXiv:1808.10387).

**Result.** Even though plain de Casteljau is backward stable, the *forward* error still scales with
the condition number of the evaluation, which blows up near multiple roots or for high-degree curves
sampled near zeros. The compensated algorithm applies error-free transformations (TwoSum,
TwoProduct via FMA) to capture the rounding error of every elementary operation and accumulate a
correction term. The result is as accurate as if the whole de Casteljau triangle had been computed
in `K` times the working precision and rounded once, for tunable `K = 2, 3, ...`. Cost is a small
constant multiple (roughly 10x to 20x for `K = 2`) of the standard algorithm, with no change in
asymptotic complexity and no extended-precision data types.

**Kernel relevance.** A targeted tool, not a default. Keel should keep standard de Casteljau/de Boor
in the hot path and reach for the compensated variant only in accuracy-critical inner loops:
root isolation near tangencies, self-intersection at near-singular points, and verification passes
in tolerant Booleans. Because it needs only FMA, it is portable to any modern CPU/GPU.

---

## 3. Rational (NURBS) evaluation and derivatives

### 3.1 Rational point evaluation via homogeneous coordinates

**Citation.** Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer (Algorithms
A4.1 curve, A4.3 surface; Chapter 4).

**Algorithm / result.** A NURBS curve is the projection of a B-spline curve in 4D homogeneous space.
Evaluate the homogeneous curve `C^w(u) = sum N_{i,p}(u) P^w_i` with ordinary de Boor / basis
machinery on the 4-vectors, giving `C^w(u) = (X, Y, Z, W)`, then project: `C(u) = (X/W, Y/W, Z/W)`.
This is the standard, fast, stable route: it requires no special rational recurrence, only one extra
coordinate and a single division at the end.

**Complexity / numerics.** Same `O(p^2)` as the nonrational case plus one division. Stable provided
weights are positive (then `W > 0` strictly inside the domain and the projection is well
conditioned). Mixed-sign or near-zero weights destroy the convex-combination property and must be
rejected at construction.

**Kernel relevance.** Keel should store and process geometry in homogeneous 4D form internally and
project only at the boundary of the API. This unifies rational and nonrational code paths: a
nonrational curve is just one with all weights equal to 1, and every refinement/elevation operation
below acts on the 4D points unchanged.

### 3.2 Rational derivatives (the quotient-rule family)

**Citation.** Piegl & Tiller (1997), Algorithm A4.2 (curve derivatives), A4.4 (surface
derivatives), Section 4.3. Foundational identity attributed there and to Floater.

**Algorithm / result.** Differentiating `C(u) = A(u)/w(u)`, where `A(u)` is the vector numerator
(the first three homogeneous components) and `w(u)` the weight denominator, gives a recursive
quotient rule. Let `A^(k)` and `w^(k)` be the derivatives of numerator and denominator (obtained
from the *nonrational* B-spline derivative algorithm A3.2 applied to homogeneous components). Then

```
C^(k)(u) = ( A^(k)(u) - sum_{i=1..k} C(k,i) * w^(i)(u) * C^(k-i)(u) ) / w(u)
```

where `C(k,i)` are binomial coefficients. So one computes the homogeneous derivatives once, then
unwinds the rational derivatives recursively. The surface version applies the bivariate analogue
(Leibniz in both parameters).

**Complexity / numerics.** Computing derivatives up to order `d`: dominated by the homogeneous
B-spline derivative evaluation, `O(p^2 + d^2)` for curves. The recursion only adds/subtracts already
computed quantities and divides by `w(u) > 0`, so it inherits the stability of the underlying
nonrational derivatives. The chief hazard is requesting high-order derivatives near reduced
continuity (at knots of high multiplicity), where the true derivative is large or discontinuous.

**Kernel relevance.** First and second derivatives drive curvature, surface normals, offset
construction, and Newton iterations in intersection. Keel needs A4.2/A4.4 (or the SISL/OpenNURBS
equivalents) as a core service, ideally returning the full `0..d` derivative array in one pass so
that point, tangent, and curvature come from a single triangular evaluation.

### 3.3 Marsden's identity, blossoming, and Greville abscissae

**Citation.** Marsden, M. J. (1970). An identity for spline functions with applications to
variation-diminishing spline approximation. *Journal of Approximation Theory*, 3, 7-49. Ramshaw, L.
(1989). Blossoms are polar forms. *Computer Aided Geometric Design*, 6(4), 323-358. Seidel, H.-P.
(1991). Blossoming and knot insertion algorithms for B-spline curves. *Computer Aided Geometric
Design*. Lee, E. T. Y. (1996) and others on the blossoming form of Marsden's identity.

**Result.** Marsden's identity expresses the monomial `(u - x)^p` (for fixed `x`) as a B-spline
expansion whose coefficients are products of shifted knots, `prod_{j=1..p}(u_{i+j} - x)`. It is the
algebraic root from which the recurrence, knot insertion, and the dual (de Boor-Fix) functionals all
follow, and it is equivalent to the blossoming (polar form) theorem of Ramshaw. **Blossoming** gives
the cleanest derivation of every refinement algorithm in this document: a degree-`p` polynomial has a
unique symmetric multiaffine `p`-argument blossom `b(.,...,.)`, the control points are blossom values
at consecutive knots `P_i = b(u_{i+1}, ..., u_{i+p})`, knot insertion is "evaluate the blossom with
the new knot substituted into one slot," and de Boor's algorithm is repeated blossom evaluation.

The **Greville abscissae** (knot averages) `xi_i = (u_{i+1} + ... + u_{i+p}) / p` are the natural
parameter values associated with each control point. They are where the control polygon is, in an
averaged sense, "anchored" to the curve.

**Kernel relevance.** Two concrete uses. (1) Greville abscissae are the correct sample parameters for
control-point editing, for building the collocation matrix in interpolation/fitting (sampling at
Grevilles gives a banded, totally positive, well-conditioned system), and for IGA collocation. (2)
Blossoming is the cleanest mental and implementation model for a kernel author: implementing
insertion, splitting, and elevation as blossom operations reduces the chance of index errors and
makes the rational case fall out automatically (blossom the homogeneous points).

---

## 4. Knot insertion and refinement

### 4.1 Boehm's single knot insertion

**Citation.** Boehm, W. (1980). Inserting new knots into B-spline curves. *Computer-Aided Design*,
12(4), 199-201. Boehm, W. (1985). On the efficiency of knot insertion algorithms. *Computer Aided
Geometric Design*. NURBS Book Algorithm A5.1.

**Algorithm / result.** To insert a single new knot `ubar` into span `[u_j, u_{j+1})`, the new
control polygon has one extra point. Only `p` control points change; the rest are copied. The new
points are convex combinations of consecutive old points:

```
Q_i = (1 - a_i) * P_{i-1} + a_i * P_i,   i = j-p+1 .. j
a_i = (ubar - u_i) / (u_{i+p} - u_i)
```

The knot vector gains `ubar` in sorted position. Geometry is unchanged; only the representation is
refined (an instance of the general principle that knot insertion is a corner-cut that adds a
description of the same curve). Inserting the same knot `p` times reduces continuity to `C^0` at that
parameter and is exactly the splitting/Bezier-decomposition operation.

**Complexity / numerics.** `O(p)` per single insertion (only `p` convex combinations), plus the
knot-array shift. Convex combinations, so backward stable; identical stability story to de Boor. For
inserting one knot to multiplicity `s`, repeat `s` times in `O(s*p)`.

**Kernel relevance.** The workhorse refinement primitive. Used for: splitting a curve/surface at a
parameter (insert to multiplicity `p`), isolating a Bezier span, adding local editing freedom, and
as the construction route for the Bezier extraction operator (Section 6). Keel needs A5.1 (curve),
A5.3 (surface, applied row/column-wise), and the rational version acting on homogeneous points.

### 4.2 The Oslo algorithm (simultaneous many-knot refinement)

**Citation.** Cohen, E., Lyche, T., & Riesenfeld, R. F. (1980). Discrete B-splines and subdivision
techniques in computer-aided geometric design and computer graphics. *Computer Graphics and Image
Processing*, 14(2), 87-111. Efficiency improvement: Lyche, T., & Morken, K. (1986). Making the Oslo
algorithm more efficient. *SIAM Journal on Numerical Analysis*, 23(3), 663-675.

**Algorithm / result.** Given an original knot vector `U` and a refined superset `Ubar` (`U` is a
subset of `Ubar`), the Oslo algorithm computes the refined control points in one shot. The new
control points `Q_j` are weighted sums of old `P_i` using **discrete B-splines** `alpha_{i,p}(j)`:

```
Q_j = sum_i  alpha_{i,p}(j) * P_i
```

The discrete B-splines `alpha` satisfy their own Cox de Boor-like recurrence in the refined knots
and act as the entries of the (banded) refinement matrix mapping the old polygon to the new. Boehm's
algorithm is the special case "refine by one knot"; Oslo handles an arbitrary refinement, including
high multiplicities, at once. The 1986 Lyche and Morken improvement reorganizes the computation so
Oslo is as cheap per knot as repeated Boehm while retaining the batch formulation.

**Complexity / numerics.** Building the banded refinement matrix and applying it: roughly
`O((number of new knots) * p)` with the efficient formulation, comparable to repeated Boehm but
with better data locality and a reusable matrix. The `alpha` are nonnegative and each row sums to 1
(discrete partition of unity), so application is again a stable convex-combination process.

**Significance / kernel relevance.** Oslo is the right tool when many knots are inserted together:
merging two curves onto a common (union) knot vector before adding/blending, refining a whole surface
for tessellation or IGA `h`-refinement, or down-stream from approximation. The reusable refinement
matrix is valuable: in IGA and in surface-surface operations the same refinement is applied to many
coordinate channels (x, y, z, w), so amortizing the `alpha` computation pays off. Keel should expose
both "insert one knot" (Boehm) and "refine to superset knot vector" (Oslo) as first-class ops.

### 4.3 Knot vector merging and common-refinement

A recurring kernel need (addition of splines, blending, lofting, trimming bookkeeping) is to bring
two B-splines of the same degree onto a *common* knot vector. The procedure: form the union of the
two knot multisets (taking the max multiplicity at each value), then apply Oslo refinement to each
operand. After merging, the operands share basis functions and coefficient-wise operations are
exact. This is a direct corollary of Section 4.2 and is worth a dedicated, well-tested routine.

---

## 5. Knot removal and degree reduction (data reduction)

### 5.1 Knot removal with tolerance (Lyche and Morken)

**Citation.** Lyche, T., & Morken, K. (1987). Knot removal for parametric B-spline curves and
surfaces. *Computer Aided Geometric Design*, 4(3), 217-230. Companion: Lyche, T., & Morken, K.
(1988). A data-reduction strategy for splines with applications to the approximation of functions
and data. *IMA Journal of Numerical Analysis*, 8(2), 185-208. NURBS Book Algorithms A5.8 (remove one
knot), A5.9/A5.10 (remove to tolerance).

**Algorithm / result.** Knot removal is the inverse of insertion: it attempts to drop a knot and
recompute fewer control points so the curve is unchanged (if the knot is genuinely removable, i.e.
the continuity it enforces is not actually used) or perturbed within a user tolerance. Removability
of a knot of multiplicity `s` is decided by checking whether the over-determined system that "undoes"
the insertion is consistent to within tolerance; the NURBS Book A5.8 gives the explicit
back-substitution. The Lyche and Morken global strategy ranks all interior knots by an inexpensive
**weighted-norm error estimate** (using the dual functionals / divided differences), then removes
knots greedily in increasing order of estimated error, accumulating the bound and stopping before the
tolerance is exceeded. The algorithm automatically chooses both how many knots and which knots to
remove. The 1988 paper frames this as a general data-reduction strategy: fit a dense spline, then
remove knots to the tolerance to obtain a parsimonious representation.

**Complexity / numerics.** Each single removal is `O(p)` (mirror of insertion). The global strategy
adds the error-estimate sort. The error bounds are based on `L_infinity`-type discrete norms and are
provably conservative, so the output is guaranteed within tolerance. The delicate part is the
parametric/vector case: removal must respect all coordinate channels and the weight channel jointly.

**Significance / kernel relevance.** Essential for keeping kernel data clean. After Booleans,
offsets, intersections, and especially after format import, splines accumulate redundant knots and
spurious control points. Knot removal to tolerance is the canonical *simplification* pass: it shrinks
data, restores higher continuity where it was artificially lost, and improves downstream robustness
and performance. Keel should run tolerance-based removal as part of a "normalize/heal spline" service
and after every operation that uses knot insertion as scaffolding (e.g. addition via common
refinement).

### 5.2 Degree reduction with error bounds

**Citation.** Eck, M. (1993). Degree reduction of Bezier curves. *Computer Aided Geometric Design*,
10(3-4), 237-251. Eck, M. (1995). Least squares degree reduction of Bezier curves. *Computer-Aided
Design*. Watkins, M. A., & Worsey, A. J. (1988). Degree reduction of Bezier curves.
*Computer-Aided Design*, 20(7), 398-405. Lachance, M. A. (1988). Chebyshev economization for
parametric surfaces. *Computer Aided Geometric Design*, 5(3), 195-208. Modern: Sunwoo, H. (multi
degree reduction matrix forms); Chebyshev-Bernstein basis-transformation methods.

**Algorithm / result.** Degree reduction approximates a degree-`p` polynomial curve by a degree
`p - 1` (or `p - r`) curve, minimizing the deviation subject to endpoint continuity constraints. The
key insight (Watkins and Worsey; Lachance; Eck) is that the *best uniform* (minimax) constrained
reduction is governed by **constrained Chebyshev polynomials**: the optimal error is proportional to
the leading Bezier coefficient combination times a known Chebyshev factor, giving an explicit a
priori error bound *before* doing the work. Eck made Lachance's construction concrete with closed-form
control-point formulas; the `L_2` (least-squares) variant reduces to a weighted Euclidean best
approximation of the Bernstein coefficients, with constraints handled by projecting out the
endpoint-continuity subspace. For curves of varying degree (B-splines), reduction is done per Bezier
segment and re-merged.

**Complexity / numerics.** Closed-form reductions are `O(p)` to `O(p^2)` per segment; the
attractive feature is the *certified error bound*, so the kernel can decide reduction is safe before
committing. Degree reduction is inherently lossy (unlike knot removal, which can be exact), so the
tolerance check is mandatory.

**Kernel relevance.** Used for interoperability (downconverting to a target system's max degree),
simplification, and producing lower-degree proxies for fast preview/tessellation. The a priori
Chebyshev bound is exactly what a kernel wants: query "can I drop a degree within tol?" cheaply, then
do it. Keel should pair degree reduction with knot removal in the simplification pipeline.

---

## 6. Bezier decomposition and extraction

### 6.1 Per-span Bezier decomposition

**Result.** Any B-spline is a piecewise Bezier curve. Inserting every interior knot up to
multiplicity `p` (via Boehm or Oslo) yields a knot vector in which each span is a clamped Bezier
segment; the control points then partition into per-span Bezier control nets. This is the basis for
treating a NURBS as a collection of rational Bezier patches for intersection, tessellation, and
display. Cost: dominated by the insertions, `O(n * p^2)` for a curve of `n` spans.

### 6.2 Bezier extraction operator (Borden, Scott, Evans, Hughes)

**Citation.** Borden, M. J., Scott, M. A., Evans, J. A., & Hughes, T. J. R. (2011). Isogeometric
finite element data structures based on Bezier extraction of NURBS. *International Journal for
Numerical Methods in Engineering*, 87(1-5), 15-47. Companion for T-splines: Scott, M. A., et al.
(2011), same journal, 88.

**Algorithm / result.** Bezier extraction factors the decomposition of Section 6.1 into a fixed,
reusable linear operator. Repeated knot insertion to full multiplicity is a sequence of refinement
matrices; their product, restricted to element `e`, is the **element extraction operator** `C^e`, a
`(p+1) x (p+1)` matrix that maps the element's *Bezier* control points to the *spline* control
points (or, transposed, maps the global spline basis to the local Bernstein basis on the element):

```
P^e_Bezier = (C^e)^T * P^e_spline      (control-point form)
N^e(u)     = C^e * B(u)                (basis form: spline basis = C^e times Bernstein)
```

The `C^e` depend only on the knot vector, not on the geometry, so they are precomputed once and
reused for every coordinate and every analysis. For the **rational** (NURBS) case the operator acts
on the homogeneous 4D control points exactly as in the polynomial case; weights are carried through
and the per-element rational weight is recovered, with the rational Bezier element using a diagonal
weight scaling `W^e` derived from the extracted homogeneous points.

**Complexity / numerics.** Building all `C^e` is essentially the cost of decomposition, done once.
Thereafter evaluation on an element is plain Bernstein evaluation (stable de Casteljau) composed with
a fixed matrix multiply, `O(p^2)`. Because `C^e` is built from convex-combination insertions, its
entries are bounded and the operator is well conditioned.

**Significance / kernel relevance.** This is the cleanest bridge between spline geometry and any
element-based or patch-based consumer. Even outside IGA, the pattern is valuable for Keel: precompute
per-span extraction operators so that tessellation, intersection, and visualization all run on the
optimally stable Bernstein form via fixed matrices, while the editable representation stays in
compact B-spline form. It also localizes work: changing one element's data touches only its operator.

---

## 7. Subdivision, splitting, and control-polygon convergence

**Citation.** Cohen, Lyche, & Riesenfeld (1980) (subdivision via discrete B-splines, as in 4.2);
Lane, J. M., & Riesenfeld, R. F. (1980). A theoretical development for the computer generation and
display of piecewise polynomial surfaces. *IEEE PAMI*, 2(1), 35-46. Prautzsch, H., Boehm, W., &
Paluszny, M. (2002). *Bezier and B-Spline Techniques*, Springer (convergence proofs).

**Result.** Splitting a Bezier curve at `t` is free from the de Casteljau triangle (Section 1.3):
the left/right triangle edges are the two sub-curves' control points. Splitting a B-spline at `u`
means inserting `u` to multiplicity `p`. Repeated subdivision generates a sequence of control
polygons that converge to the curve; the key quantitative result (Lane and Riesenfeld; Dahmen;
Cohen-Schumaker) is that the distance between the control polygon and the curve after `j` uniform
subdivisions decreases like `O(2^(-2j))` (quadratically per level), bounded by a constant times the
second differences of the control points. This is the theoretical backing for "subdivide until the
polygon is within tolerance, then treat it as the curve."

**Kernel relevance.** Subdivision-to-tolerance is the backbone of tessellation, the bounding-volume
hierarchies used in intersection, and Bezier clipping. The quadratic convergence bound lets Keel
choose subdivision depth a priori from a chord/deviation tolerance instead of guessing. Adaptive
(non-uniform) subdivision driven by local second differences gives near-minimal triangle counts.

---

## 8. Degree elevation

### 8.1 Prautzsch's B-spline degree elevation

**Citation.** Prautzsch, H. (1984). Degree elevation of B-spline curves. *Computer Aided Geometric
Design*, 1(2), 193-198. Prautzsch, H., & Piper, B. (1991). A fast algorithm to raise the degree of
spline curves. *Computer Aided Geometric Design*, 8(4), 253-265.

**Algorithm / result.** Degree elevation raises a degree-`p` spline to degree `p + 1` representing
the *same* curve. Prautzsch's approach: (1) every knot's multiplicity must increase by 1 (since
raising degree by 1 raises the multiplicity needed for a given continuity), and (2) express the
original B-splines of order `k` as combinations of order-`k + 1` B-splines on the augmented knot
vector. The identity Prautzsch discovered writes each lower-order basis function exactly in the
higher-order basis, yielding the new control points. Prautzsch-Piper (1991) made this fast with a
recurrence.

**Complexity / numerics.** Knot multiplicities all increase, so the control count grows by the number
of distinct interior knots plus one; the computation is a structured linear map, stable as it is
built from positive combinations. The output has more control points than strictly necessary for the
geometry (an unavoidable consequence of uniform multiplicity bump), which is why an elevation pass is
often followed by knot removal (Section 5.1) to clean up.

### 8.2 Piegl and Tiller degree elevation

**Citation.** Piegl, L., & Tiller, W. (1997). *The NURBS Book*, Algorithm A5.9 (degree elevate
curve); originally Piegl & Tiller (1994), Software-engineering style writeup in *Computer-Aided
Design*.

**Algorithm / result.** The NURBS Book A5.9 decomposes the spline into Bezier segments, elevates each
Bezier segment by the classic Bernstein degree-elevation formula

```
P*_i = (i/(p+1)) * P_{i-1} + (1 - i/(p+1)) * P_i,  i = 0..p+1   (per Bezier segment)
```

then removes the now-redundant knots between segments to reassemble a single higher-degree B-spline.
It handles elevation by an arbitrary number of degrees `t` at once and works on homogeneous points
for the rational case.

**Complexity / numerics.** Practical and robust; cost is decomposition plus per-segment elevation
plus removal, roughly `O(n * p^2)` for elevating by one degree across `n` segments. The Bezier
elevation step is a convex combination (stable). The reassembly via knot removal is exact here
(the redundant knots are genuinely removable).

### 8.3 Fast degree elevation via derivatives (Huang, Hu, Martin)

**Citation.** Huang, Q.-X., Hu, S.-M., & Martin, R. R. (2005). Fast degree elevation and knot
insertion for B-spline curves. *Computer Aided Geometric Design*, 22(2), 183-197. (Tsinghua / Cardiff
groups; see also their CAD-journal 2004 short paper "Efficient degree elevation and knot insertion
using derivatives.")

**Algorithm / result.** Their method elevates in three conceptually simple steps that avoid explicit
Bezier decomposition: (1) compute the spline's derivatives (the control points of the derivative
splines) from the original control points; (2) resample / build the target (elevated) knot vector;
(3) reconstruct the elevated control points from the derivative information by integrating in the
higher-degree basis. The same derivative-based machinery also yields a fast multiple knot insertion.
The authors report it is both faster than Prautzsch-Piper and the Bezier-decompose approach and much
easier to implement and understand, because it works directly on the global representation.

**Complexity / numerics.** Linear in the number of control points for fixed degree (and substantially
fewer operations than decompose-elevate-merge for elevating by several degrees at once); stable as it
is composed of the standard derivative and corner-cutting operations.

**Kernel relevance for Section 8.** Degree elevation is needed to bring two splines to a common degree
(prerequisite, with common-knot merging, for addition, blending, and surface lofting across patches
of different degrees) and for interoperability with systems that mandate a specific degree. Keel
should implement the Piegl-Tiller A5.9 as the robust, well-tested baseline and consider the
Huang-Hu-Martin route as a fast path for multi-degree elevation. Always follow elevation with a
knot-removal cleanup pass.

---

## 9. Basis conversion and why monomial form is harmful

**Citation.** Farouki & Rajan (1987, 1988) as in Section 2.1; MIT hyperbook node 11 (Numerical
condition of polynomials in Bernstein form); Farin (2002) on Bernstein-to-monomial transforms.

**Result.** Converting between Bernstein/B-spline and power (monomial `1, u, u^2, ...`) basis uses a
transformation matrix whose entries grow combinatorially and alternate in sign, so the conversion is
ill conditioned: a perfectly well-conditioned Bezier curve can become a wildly sensitive monomial
polynomial, and evaluating that monomial form (even with Horner) suffers catastrophic cancellation
for high degree or for arguments away from where the coefficients were balanced. The literature is
unanimous: stay in Bernstein/B-spline form. Conversions that *are* safe and routinely used inside a
kernel are the corner-cutting ones, B-spline to per-span Bezier (knot insertion) and back (knot
removal), and Bernstein-to-Bernstein degree change (elevation/reduction), all of which are convex and
well conditioned.

**Kernel relevance.** A hard rule for Keel: the internal canonical form is (homogeneous) B-spline,
with per-span Bezier as the only alternate representation for leaf-level numeric kernels. Power-basis
conversion is permitted only as a last-resort export at low degree, flagged as lossy/risky. This also
informs root-finding: isolate roots in Bernstein form (subdivision / Bezier clipping), not by forming
the companion matrix of a monomial polynomial.

---

## 10. Implementation studies: libraries and GPU

### 10.1 Open-source kernels and library design

**Citation.** SINTEF SISL (SINTEF Spline Library), C, the most feature-complete open NURBS library;
openNURBS (Robert McNeel & Associates), C++, the de facto interchange/SDK reference, basis of the
3DM format; Open CASCADE Technology (OCCT), C++, the most widely used open B-rep kernel (FreeCAD,
etc.); Verb (Peter Boyer), JS/multi-language, a clean modern reimplementation; libnurbs; the
geomdl/NURBS-Python package (pedagogical, faithful to the NURBS Book algorithm numbering).

**Findings.** Implementations cluster around the NURBS Book algorithm numbering (A2.x evaluation,
A5.x refinement), which is effectively the lingua franca; openNURBS documents its knot insertion as
"algorithm A5.1 from The NURBS Book," and Verb mirrors the same structure. Practitioner commentary
notes the older large kernels (OCCT, openNURBS) carry substantial accreted complexity, while leaner
reimplementations (Verb) trade breadth for clarity. The consistent lesson: a kernel's NURBS core is a
fairly small, well-specified set of routines (span search, basis/derivative eval, de Boor, Boehm/Oslo
insertion, knot removal, degree elevate/reduce, decomposition); the engineering difficulty is in
robustness, tolerancing, and the surrounding topology, not in the spline formulas themselves.

**Kernel relevance.** Keel should (a) follow the NURBS Book algorithm taxonomy so the code is auditable
against the canonical reference and against openNURBS/SISL behavior; (b) study SISL for breadth and
openNURBS for the data model and interchange; (c) keep the numeric core small, pure, and heavily
tested (it is the foundation everything else stands on); and (d) treat homogeneous-coordinate
processing as the default so rational and polynomial paths unify.

### 10.2 GPU evaluation and matrix-form B-splines

**Citation.** Krishnamurthy, A., Khardekar, R., McMains, S., Haller, K., & Elber, G. (2007). Direct
evaluation of NURBS curves and surfaces on the GPU. *ACM Symposium on Solid and Physical Modeling
(SPM)*. Krishnamurthy, A., McMains, S., et al. (2009). Performing efficient NURBS modeling operations
on the GPU. *IEEE TVCG*, 15(4). Recent: matrix-representation and GPU-optimized parallel B-spline
computing (arXiv:2504.11498, 2025).

**Result.** Krishnamurthy et al. showed that NURBS surfaces can be evaluated *directly* on the GPU
from control points and knot vectors stored as textures, in a fragment program, without first
approximating by lower-degree Bezier patches as earlier GPU methods required; they add dynamic
level-of-detail for real-time interaction and later extend to modeling operations (surface-surface
intersection, etc.) on the GPU. The 2025 matrix-representation work recasts B-spline evaluation as
dense small-matrix products (per-span basis matrices times control points), which map efficiently to
GPU/SIMD and tensor hardware and parallelize across sample points trivially.

**Complexity / numerics.** Per-sample evaluation parallelizes embarrassingly; the matrix form trades a
slightly higher flop count for regular, branch-free, cache-friendly kernels, often a net win on
wide hardware. Care is needed near knots (span selection branches) and with rational weights
(division), but the underlying arithmetic is the same stable corner-cutting / Bernstein evaluation.

**Kernel relevance.** For Keel's display and tessellation tiers, a precomputed per-span basis-matrix
(closely related to the Bezier extraction operator of Section 6.2) plus batched GPU evaluation gives
real-time surface sampling. The same matrices serve CPU SIMD. This keeps the *editable* model in
B-spline form while the *render/sample* path uses the fast matrix form, with no loss of stability.

---

## 11. Implementation guidance for Keel (synthesis)

1. **Canonical representation.** Store all curves/surfaces as homogeneous (4D) B-splines. Nonrational
   is the all-weights-equal-1 special case. Every refinement/elevation operation acts on the 4D
   points unchanged; project to 3D only at the API boundary. This unifies the rational and polynomial
   code paths and is mandated by the conditioning results (Sections 2, 9).

2. **Evaluation core (the hot path).** Implement, against the NURBS Book numbering:
   span search (A2.1, binary search), all-nonzero basis + derivatives (A2.2/A2.3) with
   stack-allocated `left/right` arrays, de Boor point eval (A3.1), and the rational point/derivative
   family (A4.1/A4.2 and surface A4.3/A4.4) returning the full `0..d` derivative array in one pass.
   Prefer de Boor / de Casteljau corner cutting over basis-then-dot when only points are needed, and
   never over a monomial Horner form. These corner-cutting schemes are provably backward stable with
   error growing only linearly in degree (Mainar and Pena).

3. **Refinement primitives.** Boehm single insertion (A5.1, `O(p)`) and Oslo superset refinement
   (with the Lyche-Morken efficiency reorganization) as first-class ops, both acting on homogeneous
   points. Build a robust common-knot-merge (union of multiplicities + Oslo) since addition, blending,
   and lofting all depend on it.

4. **Simplification pipeline.** Tolerance-based knot removal (Lyche and Morken; A5.8-A5.10) plus
   constrained degree reduction with Chebyshev a priori error bounds (Eck; Watkins and Worsey). Run
   this after every operation that uses knot insertion as scaffolding and on every import, to shrink
   data and restore continuity. Knot removal can be exact; degree reduction is lossy and must be
   tolerance-gated.

5. **Degree elevation.** Piegl-Tiller A5.9 (decompose, elevate Bezier segments, remove redundant
   knots) as the trusted baseline; Huang-Hu-Martin derivative-based method as a fast path for
   multi-degree elevation. Always follow with knot-removal cleanup. Needed to reach common degree
   before binary spline operations.

6. **Leaf numeric form.** Per-span Bezier via decomposition (Section 6.1) and a precomputed Bezier
   *extraction operator* per span (Borden et al., Section 6.2). Run intersection, tessellation, and
   root isolation on the optimally stable Bernstein form via fixed matrices. Subdivide-to-tolerance
   using the quadratic-convergence bound (Section 7) to size tessellation and BVHs a priori.

7. **Accuracy escalation.** Standard de Casteljau/de Boor everywhere by default; reach for the
   *compensated* de Casteljau (FMA-based EFT, accuracy as if in K times working precision) only in
   accuracy-critical inner loops: near-tangent intersections, near-singular self-intersection,
   verification passes in tolerant Booleans. Optionally surface a running-error bound from the
   evaluator for downstream tolerant algorithms (Mainar and Pena).

8. **Never convert to power basis** for evaluation, intersection, or root finding. Conversion is
   combinatorially ill conditioned (Farouki and Rajan; Section 9). Permit monomial export only at low
   degree, flagged as lossy.

9. **Parameter conventions.** Use Greville abscissae (knot averages) as the canonical control-point
   parameters for fitting collocation (well-conditioned, totally positive systems), control editing,
   and any collocation sampling. Consider blossoming as the implementation model for insertion,
   splitting, and elevation: it minimizes index errors and makes the rational case automatic.

10. **Performance tiers.** Keep the editable model in B-spline form; for display/tessellation and
    batch sampling, use per-span basis matrices (kin to the extraction operator) for CPU SIMD and GPU
    evaluation (Krishnamurthy et al.; matrix-form B-splines), with no loss of stability.

11. **Provenance.** Track the NURBS Book algorithm taxonomy in the code so behavior can be audited
    against openNURBS and SISL. The numeric core is small and well specified; invest the engineering
    effort in robustness, tolerancing, and test coverage, not in reinventing the recurrences.

---

## References

1. de Boor, C. (1972). On calculating with B-splines. *Journal of Approximation Theory*, 6(1), 50-62.
2. Cox, M. G. (1972). The numerical evaluation of B-splines. *IMA Journal of Applied Mathematics*, 10(2), 134-149.
3. Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer-Verlag.
4. Patrikalakis, N. M., Maekawa, T., & Cho, W. *Shape Interrogation for Computer Aided Design and Manufacturing* (MIT hyperbook). https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/
5. Farin, G. (2002). *Curves and Surfaces for CAGD: A Practical Guide* (5th ed.). Morgan Kaufmann.
6. Farouki, R. T., & Rajan, V. T. (1987). On the numerical condition of polynomials in Bernstein form. *Computer Aided Geometric Design*, 4(3), 191-216.
7. Farouki, R. T., & Rajan, V. T. (1988). Algorithms for polynomials in Bernstein form. *Computer Aided Geometric Design*, 5(1), 1-26.
8. Farouki, R. T., & Goodman, T. N. T. (1996). On the optimal stability of the Bernstein basis. *Mathematics of Computation*, 65(216), 1553-1566.
9. Farouki, R. T. (2012). The Bernstein polynomial basis: a centennial retrospective. *Computer Aided Geometric Design*, 29(6), 379-419.
10. Mainar, E., & Pena, J. M. (1999). Error analysis of corner cutting algorithms. *Numerical Algorithms*, 22(1), 41-52.
11. Mainar, E., & Pena, J. M. (2007). A corner cutting algorithm for evaluating rational Bezier surfaces and the optimal stability of the basis. *SIAM Journal on Scientific Computing*, 30(3).
12. Delgado, J., Mainar, E., & Pena, J. M. (2023). On the accuracy of de Casteljau-type algorithms and Bernstein representations. *Computer Aided Geometric Design*, 104, 102211.
13. Jiang, H., Graillat, S., et al. (2019). Compensated de Casteljau algorithm in K times the working precision. *Applied Mathematics and Computation*, 357, 57-74. (arXiv:1808.10387)
14. Marsden, M. J. (1970). An identity for spline functions with applications to variation-diminishing spline approximation. *Journal of Approximation Theory*, 3, 7-49.
15. Ramshaw, L. (1989). Blossoms are polar forms. *Computer Aided Geometric Design*, 6(4), 323-358.
16. Seidel, H.-P. (1991). Blossoming and knot insertion algorithms for B-spline curves. *Computer Aided Geometric Design*.
17. Boehm, W. (1980). Inserting new knots into B-spline curves. *Computer-Aided Design*, 12(4), 199-201.
18. Cohen, E., Lyche, T., & Riesenfeld, R. F. (1980). Discrete B-splines and subdivision techniques in computer-aided geometric design and computer graphics. *Computer Graphics and Image Processing*, 14(2), 87-111.
19. Lyche, T., & Morken, K. (1986). Making the Oslo algorithm more efficient. *SIAM Journal on Numerical Analysis*, 23(3), 663-675.
20. Lyche, T., & Morken, K. (1987). Knot removal for parametric B-spline curves and surfaces. *Computer Aided Geometric Design*, 4(3), 217-230.
21. Lyche, T., & Morken, K. (1988). A data-reduction strategy for splines. *IMA Journal of Numerical Analysis*, 8(2), 185-208.
22. Prautzsch, H. (1984). Degree elevation of B-spline curves. *Computer Aided Geometric Design*, 1(2), 193-198.
23. Prautzsch, H., & Piper, B. (1991). A fast algorithm to raise the degree of spline curves. *Computer Aided Geometric Design*, 8(4), 253-265.
24. Huang, Q.-X., Hu, S.-M., & Martin, R. R. (2005). Fast degree elevation and knot insertion for B-spline curves. *Computer Aided Geometric Design*, 22(2), 183-197.
25. Eck, M. (1993). Degree reduction of Bezier curves. *Computer Aided Geometric Design*, 10(3-4), 237-251.
26. Watkins, M. A., & Worsey, A. J. (1988). Degree reduction of Bezier curves. *Computer-Aided Design*, 20(7), 398-405.
27. Lachance, M. A. (1988). Chebyshev economization for parametric surfaces. *Computer Aided Geometric Design*, 5(3), 195-208.
28. Borden, M. J., Scott, M. A., Evans, J. A., & Hughes, T. J. R. (2011). Isogeometric finite element data structures based on Bezier extraction of NURBS. *International Journal for Numerical Methods in Engineering*, 87(1-5), 15-47.
29. Lane, J. M., & Riesenfeld, R. F. (1980). A theoretical development for the computer generation and display of piecewise polynomial surfaces. *IEEE Transactions on Pattern Analysis and Machine Intelligence*, 2(1), 35-46.
30. Prautzsch, H., Boehm, W., & Paluszny, M. (2002). *Bezier and B-Spline Techniques*. Springer.
31. Krishnamurthy, A., Khardekar, R., McMains, S., Haller, K., & Elber, G. (2007). Direct evaluation of NURBS curves and surfaces on the GPU. *ACM Symposium on Solid and Physical Modeling*.
32. Krishnamurthy, A., McMains, S., et al. (2009). Performing efficient NURBS modeling operations on the GPU. *IEEE Transactions on Visualization and Computer Graphics*, 15(4).
33. Matrix representation and GPU-optimized parallel B-spline computing (2025). arXiv:2504.11498.
34. SINTEF SISL, openNURBS (Robert McNeel & Associates), Open CASCADE Technology, Verb (P. Boyer), geomdl/NURBS-Python: open-source library documentation and source.
