# B-Splines and NURBS: Mathematical Foundations and Theory

**Scope.** This document is a depth-first literature investigation into the mathematical theory underpinning B-splines and NURBS, written to support the design of *Keel*, an open-source B-rep solid modeling kernel. It covers the seminal theory (Schoenberg splines; the Cox and de Boor recurrence; the Curry-Schoenberg and Schoenberg-Whitney theorems), the structural properties of the B-spline basis, knot-vector theory, the rational (NURBS) extension and its projective interpretation, the Bezier/Bernstein substrate, parametric versus geometric continuity, derivative formulas for polynomial and rational forms, the canonical textbooks and surveys, and the theoretically important post-2000 results (basis conditioning and optimality, total positivity, blossoming). Each significant source has a structured entry: Citation, What it establishes, Key mathematics (stated explicitly), Significance, and Kernel relevance. A closing "Theoretical toolkit for a kernel implementer" synthesizes what a kernel author must internalize.

A note on notation. Throughout, $p$ is the polynomial degree (order $= p+1$), $U = \{u_0, u_1, \dots, u_m\}$ is the knot vector with $m+1$ knots, $N_{i,p}(u)$ is the $i$-th B-spline basis function of degree $p$, $P_i$ are control points, $w_i$ are weights, and $C(u)$ / $S(u,v)$ denote curves / surfaces.

---

## 1. Origins and Seminal Theory

### 1.1 Schoenberg and the birth of splines

**Citation.** Schoenberg, I. J. (1946). Contributions to the problem of approximation of equidistant data by analytic functions. *Quarterly of Applied Mathematics*, 4, Part A 45-99, Part B 112-141.

**What it establishes.** This two-part paper introduces the modern mathematical theory of splines: piecewise-polynomial functions joined with prescribed smoothness at the joints (knots). Schoenberg analyzes smoothing and interpolation of equidistant data, introduces the *cardinal* (integer-knot) splines, and gives the analytic machinery (the fundamental functions, exponential Euler splines, generating functions) that turns a piecewise-polynomial construction into a clean approximation theory. The very word "spline" enters mathematics here, borrowed from the draftsman's flexible strip.

**Key mathematics.** A spline of degree $p$ on a knot sequence is a function $s$ that is a polynomial of degree $\le p$ on each interval $[u_i, u_{i+1})$ and is $C^{p-1}$ at simple knots. Schoenberg builds the basic B-spline by repeated convolution of the box function: with $M_0 = \mathbf{1}_{[-1/2,1/2]}$ (the unit box), the cardinal B-spline of order $p+1$ is
$$
M_{p}(x) = (M_0 * M_0 * \cdots * M_0)(x) \quad (p+1 \text{ factors}),
$$
a nonnegative, bell-shaped, compactly supported, $C^{p-1}$ function. Convolution makes its smoothness and positivity manifest, and the Fourier transform is $\widehat{M_p}(\xi) = \left( \frac{\sin(\xi/2)}{\xi/2} \right)^{p+1}$.

**Significance.** Schoenberg gave splines a rigorous home in approximation theory. The convolution/box-spline viewpoint is the cleanest proof that B-splines are smooth, nonnegative, and local, and it directly seeds later total-positivity results.

**Kernel relevance.** A kernel author rarely uses cardinal splines directly (CAD needs non-uniform knots), but Schoenberg's convolution picture is the conceptual root of every property a kernel relies on (smoothness, locality, nonnegativity). It is also the cleanest mental model for why higher degree means smoother but wider-support basis functions, which governs the locality/stiffness tradeoff in kernel data structures.

### 1.2 The Cox-de Boor recurrence

**Citations.**
- de Boor, C. (1972). On calculating with B-splines. *Journal of Approximation Theory*, 6(1), 50-62.
- Cox, M. G. (1972). The numerical evaluation of B-splines. *Journal of the Institute of Mathematics and its Applications*, 10(2), 134-149. (Often dated 1971 from the NPL technical report.)

**What it establishes.** Independently, Cox and de Boor derived the stable *recurrence relation* that defines B-spline basis functions of any degree from those of one lower degree. This converted B-splines from an awkward divided-difference definition into a numerically robust, easily programmed object. It is the single most important computational formula in the field.

**Key mathematics.** Given a knot vector $U = \{u_0, \dots, u_m\}$, the degree-zero basis functions are indicator functions of knot spans:
$$
N_{i,0}(u) = \begin{cases} 1 & u_i \le u < u_{i+1} \\ 0 & \text{otherwise.} \end{cases}
$$
For $p \ge 1$,
$$
N_{i,p}(u) = \frac{u - u_i}{u_{i+p} - u_i}\, N_{i,p-1}(u) \;+\; \frac{u_{i+p+1} - u}{u_{i+p+1} - u_{i+1}}\, N_{i+1,p-1}(u).
$$
The convention $0/0 := 0$ handles repeated knots (zero-length spans). There are $n+1 = m - p$ basis functions of degree $p$. The companion *de Boor algorithm* evaluates a curve $C(u) = \sum_i P_i N_{i,p}(u)$ by repeated affine interpolation of the control points using the same coefficients, without forming the basis functions explicitly:
$$
P_i^{[r]} = (1 - \alpha_{i,r})\, P_{i-1}^{[r-1]} + \alpha_{i,r}\, P_i^{[r-1]}, \qquad \alpha_{i,r} = \frac{u - u_i}{u_{i+p-r+1} - u_i},
$$
returning $C(u) = P_k^{[p]}$ for $u \in [u_k, u_{k+1})$.

**Significance.** The recurrence is provably stable: it is a convex combination of nonnegative quantities, so no catastrophic cancellation occurs. This is why it superseded the earlier divided-difference definition of B-splines for computation.

**Kernel relevance.** This is the workhorse of curve/surface evaluation. A kernel implements two variants: (a) compute the nonzero basis values $N_{i,p}(u)$ (there are exactly $p+1$ of them) for assembly and least-squares fitting; (b) the de Boor algorithm for direct point evaluation, which is more stable and avoids dividing by basis values. Both must be coded with the $0/0$ convention and span-finding (binary search for $k$ with $u \in [u_k, u_{k+1})$) handled carefully.

### 1.3 The Curry-Schoenberg theorem (B-splines are a basis)

**Citation.** Curry, H. B., & Schoenberg, I. J. (1966). On Polya frequency functions IV: The fundamental spline functions and their limits. *Journal d'Analyse Mathematique*, 17, 71-107.

**What it establishes.** This theorem justifies the letter "B" in B-spline: it proves that, for a given knot vector, the B-splines form a *basis* of the full spline space (all piecewise polynomials of degree $p$ with the continuity dictated by knot multiplicities). It also identifies the B-splines as having minimal support among splines, and characterizes them via divided differences of the truncated power function.

**Key mathematics.** The spline space is
$$
\mathbb{S}_{p}(U) = \{ s : s|_{[u_i, u_{i+1})} \in \Pi_p,\; s \in C^{p - \mu_j} \text{ at interior knot of multiplicity } \mu_j \},
$$
and Curry-Schoenberg states $\dim \mathbb{S}_p(U) = n+1$ with $\{N_{i,p}\}_{i=0}^{n}$ a basis. The classical (pre-recurrence) definition of the normalized B-spline is via the divided difference of the truncated power $(t - u)_+^p$:
$$
N_{i,p}(u) = (u_{i+p+1} - u_i)\, [u_i, \dots, u_{i+p+1}]\, (\cdot - u)_+^{p},
$$
where $[\cdot]$ is the divided difference in the knot variable. Minimal support: $N_{i,p}$ is supported exactly on $[u_i, u_{i+p+1}]$, $p+1$ spans.

**Significance.** Without this theorem, working in the B-spline basis would be a representational choice of unknown completeness. Curry-Schoenberg guarantees that *every* spline of the given smoothness can be written uniquely in the B-spline basis, so the representation loses nothing.

**Kernel relevance.** This underwrites every conversion algorithm a kernel performs: Bezier-to-B-spline, knot insertion, degree elevation, and spline arithmetic all rely on the target space being spanned by the B-spline basis. It is the theoretical license for "any NURBS the kernel needs can be expressed in this representation."

---

## 2. B-Spline Basis Properties

### 2.1 The fundamental property set

**Citation.** de Boor, C. (1986/various). B(asic)-spline basics. (Technical exposition; widely circulated, e.g., ftp.cs.wisc.edu/Approx/bsplbasic.pdf.) See also de Boor (2001), *A Practical Guide to Splines* (rev. ed.), Springer.

**What it establishes.** A compact, rigorous catalogue of the properties that make the B-spline basis the right basis for computation. These properties are the daily working facts of any kernel.

**Key mathematics.** For degree-$p$ B-splines on $U$:

1. **Local support.** $N_{i,p}(u) = 0$ for $u \notin [u_i, u_{i+p+1}]$. Consequently at most $p+1$ basis functions are nonzero on any span $[u_k, u_{k+1})$, namely $N_{k-p,p}, \dots, N_{k,p}$.
2. **Nonnegativity.** $N_{i,p}(u) \ge 0$ for all $u$.
3. **Partition of unity.** $\sum_{i} N_{i,p}(u) = 1$ for all $u$ in $[u_p, u_{m-p}]$. This makes curve evaluation a convex combination of control points.
4. **Linear independence.** The $\{N_{i,p}\}$ are linearly independent (Curry-Schoenberg), hence a basis.
5. **Continuity.** $N_{i,p}$ is $C^{p - \mu}$ at a knot of multiplicity $\mu$, and $C^\infty$ in the interior of spans.
6. **Variation diminishing.** A B-spline curve crosses any line (2D) or plane (3D) no more times than its control polygon does; the curve is "no wigglier than its control polygon."

**Marsden's identity.** A central tool linking monomials/Bezier to B-splines: for any $y$,
$$
(u - y)^p = \sum_i \psi_{i,p}(y)\, N_{i,p}(u), \qquad \psi_{i,p}(y) = \prod_{j=1}^{p} (u_{i+j} - y).
$$
Differentiating Marsden's identity yields partition of unity (constant term) and the *Greville abscissae* (the node averages)
$$
\xi_i = \frac{u_{i+1} + u_{i+2} + \cdots + u_{i+p}}{p},
$$
which are the natural parameter values associated to each control point.

**Significance.** Local support gives sparsity (banded matrices); nonnegativity plus partition of unity give the convex-hull property and affine invariance; variation diminishing gives geometric predictability. Marsden's identity is the bridge for degree/format conversions and for computing dual functionals.

**Kernel relevance.** Local support determines the bandwidth of fitting matrices and the locality of edits (moving one control point perturbs only $p+1$ spans). Convex hull (from nonnegativity + partition of unity) is the basis of fast culling in intersection and clipping. The Greville abscissae are the standard parameter samples for control-point-space algorithms (e.g., approximate offsetting, control-polygon-based bounds, T-spline-style fitting).

### 2.2 Total positivity and variation diminishing

**Citations.**
- Karlin, S. (1968). *Total Positivity, Vol. I.* Stanford University Press.
- Floater, M. S. (2022). The Schoenberg-Whitney theorem and total positivity. Lecture notes, University of Oslo (MAT4170).

**What it establishes.** The B-spline collocation matrix is *totally nonnegative* (all minors $\ge 0$), which is the structural reason for the variation-diminishing property and for the excellent shape behavior of B-spline curves. Total positivity (TP) is the deep property from which many "nice shape" facts follow as corollaries.

**Key mathematics.** The collocation matrix $A = [N_{j,p}(x_i)]$ evaluated at increasing sites $x_0 < x_1 < \cdots$ is totally nonnegative: every minor is $\ge 0$. A linear map with a TP matrix is *variation diminishing*: the number of sign changes in the output sequence is at most that of the input. Applied to control points, $S^-(C - L) \le S^-(\text{control polygon} - L)$ for any line/plane $L$, where $S^-$ counts sign changes. The Bernstein and B-spline bases are the canonical *totally positive bases*.

**Significance.** TP is the rigorous, basis-independent explanation of why B-spline curves do not introduce spurious oscillations. It also underlies corner-cutting interpretations of subdivision and the monotonicity/convexity-preservation results used in shape-preserving interpolation.

**Kernel relevance.** Shape predictability matters for robustness: a kernel that fits or fairs curves benefits from knowing the result will not oscillate beyond the data. TP also underwrites the stability of de Casteljau/de Boor (convex combinations) and motivates choosing the B-spline basis over, say, a monomial basis for any numerical assembly.

---

## 3. Knot Vector Theory

### 3.1 Multiplicity, continuity, and knot-vector types

**Citation.** Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer. (Chapters 2-3.)

**What it establishes.** The knot vector is the combinatorial backbone of a B-spline: its spacing controls parameterization, its multiplicities control continuity, and its end structure controls whether the curve interpolates its endpoints.

**Key mathematics.**
- **Continuity at a knot.** At an interior knot of multiplicity $\mu$, a degree-$p$ B-spline is $C^{p-\mu}$. Simple knot ($\mu=1$): $C^{p-1}$. A knot with $\mu = p$ gives $C^0$ (a corner is possible); $\mu = p+1$ splits the curve (a break). A control point can be interpolated by raising a knot to multiplicity $p$.
- **Clamped (open) knot vector.** End knots have multiplicity $p+1$:
$$
U = \{\underbrace{a, \dots, a}_{p+1}, u_{p+1}, \dots, u_{m-p-1}, \underbrace{b, \dots, b}_{p+1}\}.
$$
The curve then interpolates $P_0$ at $u=a$ and $P_n$ at $u=b$, with the end tangents along $P_1 - P_0$ and $P_n - P_{n-1}$. This is the dominant CAD convention.
- **Uniform vs non-uniform.** Uniform: equally spaced interior knots (basis functions are shifts of one another in the interior). Non-uniform: arbitrary spacing, needed to fit data and to clamp ends.
- **Periodic / unclamped.** Knots wrap so the curve and its first $p-1$ derivatives match at the seam; used for closed curves (closed surfaces of revolution, tubes).

**Significance.** Knot multiplicity is the single lever a designer/kernel has for local smoothness control. Clamping is what makes B-splines behave like Bezier curves at the ends, giving intuitive endpoint control.

**Kernel relevance.** The kernel must canonicalize knot vectors (clamped vs periodic representation), enforce validity (nondecreasing, end multiplicities), and reason about continuity at joins for blending, filleting, and trimming. Multiplicity arithmetic is central to knot insertion/removal and to merging two curves into one (which requires a common knot vector and degree).

### 3.2 The Schoenberg-Whitney theorem (interpolation well-posedness)

**Citations.**
- Schoenberg, I. J., & Whitney, A. (1953). On Polya frequency functions III: The positivity of translation determinants with an application to the interpolation problem by spline curves. *Transactions of the AMS*, 74, 246-259.
- Floater, M. S. (2022), op. cit.

**What it establishes.** A complete characterization of which sets of interpolation sites admit a unique spline interpolant in a given B-spline space. It is the well-posedness theorem for spline interpolation and least-squares fitting.

**Key mathematics.** Given $n+1$ B-splines $N_{0,p}, \dots, N_{n,p}$ and $n+1$ strictly increasing sites $x_0 < x_1 < \cdots < x_n$, the interpolation matrix $A = [N_{j,p}(x_i)]$ is nonsingular **if and only if** its diagonal is positive:
$$
N_{i,p}(x_i) > 0 \quad \text{for all } i,
$$
equivalently (for the standard clamped case)
$$
u_i < x_i < u_{i+p+1}, \qquad i = 0, \dots, n.
$$
The deeper fact is that $A$ is totally nonnegative, so nonsingularity reduces to positivity of the diagonal (a Schoenberg-Whitney/total-positivity pairing).

**Significance.** This is the precise rule for placing interpolation parameters relative to knots so the fit is solvable and well-conditioned. Violating it produces a singular or near-singular system.

**Kernel relevance.** Any "fit a NURBS through these points" feature (the bread and butter of curve/surface fitting in a kernel) must place parameters and knots so Schoenberg-Whitney holds. The standard recipe (chord-length or centripetal parameterization plus averaging knots, $u_{j} = \frac{1}{p}\sum_{i=j}^{j+p-1}\bar{t}_i$) is engineered precisely to satisfy this condition and yield a banded, totally-nonnegative, solvable system.

---

## 4. The Bezier / Bernstein Substrate

### 4.1 Bernstein polynomials and the de Casteljau algorithm

**Citations.**
- de Casteljau, P. (1959, 1963). *Outillages methodes calcul* and *Courbes et surfaces a poles.* Internal Citroen technical reports.
- Bezier, P. (1966, 1967). Definition numerique des courbes et surfaces. *Automatisme.*
- Farin, G. (2002). *Curves and Surfaces for CAGD: A Practical Guide* (5th ed.). Morgan Kaufmann.

**What it establishes.** Bezier curves, built on the Bernstein basis and evaluated by de Casteljau's recursive affine interpolation, are the degree-$p$ special case of B-splines (a single span with clamped knots). They are the conceptual and numerical seed of the whole field; de Casteljau is the prototype of de Boor.

**Key mathematics.** The Bernstein basis of degree $p$ on $[0,1]$:
$$
B_{i,p}(t) = \binom{p}{i} t^i (1-t)^{p-i}, \qquad i = 0, \dots, p,
$$
with $\sum_i B_{i,p}(t) = 1$ and $B_{i,p}(t) \ge 0$. A Bezier curve is $C(t) = \sum_{i=0}^{p} P_i B_{i,p}(t)$. The de Casteljau recurrence:
$$
P_i^{[0]} = P_i, \qquad P_i^{[r]}(t) = (1-t)\, P_i^{[r-1]}(t) + t\, P_{i+1}^{[r-1]}(t), \qquad C(t) = P_0^{[p]}(t).
$$
Bernstein recurrence: $B_{i,p}(t) = (1-t) B_{i,p-1}(t) + t\, B_{i-1,p-1}(t)$. Derivative: $C'(t) = p \sum_{i=0}^{p-1} (P_{i+1} - P_i) B_{i,p-1}(t)$, so the *hodograph* (derivative curve) is itself a Bezier curve on the forward differences $\Delta P_i = P_{i+1}-P_i$.

**Convex hull.** Because the basis is nonnegative and sums to one, $C(t)$ lies in the convex hull of $\{P_i\}$ for $t \in [0,1]$.

**Significance.** Bezier/Bernstein is where convex hull, variation diminishing, and the recursive corner-cutting evaluation first appear in their simplest, most analyzable form. The de Casteljau algorithm also gives subdivision (splitting a curve at $t$) for free as a byproduct.

**Kernel relevance.** A kernel almost always converts NURBS to piecewise rational Bezier internally for intersection, tessellation, and clipping, because per-span Bezier form gives tight convex hulls, trivial subdivision, and degree-local algorithms. The hodograph being a Bezier curve enables fast tangent/curvature bounds and self-intersection tests via the derivative's control polygon.

### 4.2 Degree elevation theory

**Citation.** Farin, G. (2002), op. cit.; Prautzsch, H., Boehm, W., & Paluszny, M. (2002). *Bezier and B-Spline Techniques.* Springer.

**What it establishes.** Any degree-$p$ Bezier (or B-spline) curve can be written exactly in degree $p+1$ by recomputing control points; the curve is unchanged. This is essential for making two curves/surfaces compatible (common degree) before merging, lofting, or Boolean-combining them.

**Key mathematics.** For a Bezier curve, degree elevation from $p$ to $p+1$ gives new control points
$$
P_i^{*} = \frac{i}{p+1} P_{i-1} + \left(1 - \frac{i}{p+1}\right) P_i, \qquad i = 0, \dots, p+1,
$$
with $P_{-1}, P_{p+1}$ ignored at the ends ($P_0^* = P_0$, $P_{p+1}^* = P_p$). Repeated elevation makes the control polygon converge to the curve. Degree elevation is variation diminishing and corner-cutting. For B-splines the operation is done per Bezier segment (after knot insertion to Bezier form) or via Prautzsch's direct B-spline degree-elevation formulas, raising each interior knot multiplicity by one.

**Significance.** Degree compatibility is a precondition for nearly all multi-curve/multi-surface operations. Degree elevation is the safe direction (exact, no information lost), unlike degree reduction (approximate).

**Kernel relevance.** Lofting, skinning, Coons/Gordon surface construction, and surface-surface merging all require operands of equal degree and compatible knots. The kernel routinely degree-elevates the lower-degree operand. The convergence of the control polygon under repeated elevation is also a (slow) bound-generation technique.

---

## 5. Continuity Theory: Parametric vs Geometric

### 5.1 Geometric continuity and the beta-constraints

**Citations.**
- Barsky, B. A. (1981). *The Beta-spline: A Local Representation Based on Shape Parameters and Fundamental Geometric Measures.* PhD thesis, University of Utah.
- Barsky, B. A., & DeRose, T. D. (1989). Geometric continuity of parametric curves: three equivalent characterizations. *IEEE Computer Graphics and Applications*, 9(6), 60-68; and (1990) *Deriving the beta-constraints for geometric continuity of parametric curves.*

**What it establishes.** A rigorous distinction between *parametric* continuity $C^k$ (equality of derivative vectors at a join) and *geometric* continuity $G^k$ (equality of geometric attributes, tangent direction and curvature, independent of parameterization). $G^k$ is the weaker, design-relevant notion: two segments can meet smoothly to the eye while having different-magnitude derivative vectors.

**Key mathematics.** Two curve segments $f$ (ending at parameter $0^-$) and $g$ (starting at $0^+$) meet with:
- **$G^0$:** $f(0) = g(0)$ (positional).
- **$G^1$:** tangent directions agree, i.e., $g'(0^+) = \beta_1 f'(0^-)$ for some $\beta_1 > 0$ (reparameterization scale).
- **$G^2$:** curvature continuous, the beta-2 constraint
$$
g''(0^+) = \beta_1^2\, f''(0^-) + \beta_2\, f'(0^-), \qquad \beta_1 > 0,\; \beta_2 \in \mathbb{R}.
$$
In general $G^k$ is expressed by a lower-triangular *connection matrix* (the beta-constraints) relating the one-sided derivatives via the chain rule of an unknown reparameterization $\phi$ with $\phi'(0) = \beta_1 > 0$. Equivalently, $G^k$ at a join holds iff there is a reparameterization making the join $C^k$. The Frenet frame and arc-length-derivative characterizations are the other two of Barsky-DeRose's "three equivalent characterizations."

**Significance.** $G^k$ captures what designers actually want (visually smooth surfaces, no curvature jumps causing light-reflection lines to break) while permitting more freedom than $C^k$. The shape parameters $\beta_1$ (bias) and $\beta_2$ (tension) become user-facing controls in beta-splines.

**Kernel relevance.** A B-rep kernel must reason about continuity *across edges* where two surfaces meet, and this is intrinsically geometric: the two surfaces have independent parameterizations, so only $G^k$ (tangent-plane continuity $G^1$, curvature continuity $G^2$) is meaningful. Fillet/blend surfaces, class-A surfacing, and reflection-line quality are all $G^2$ problems. The kernel needs $G^1$/$G^2$ checks and constructors, and must convert designer-level "tangent" or "curvature" continuity into the algebraic beta-constraints between control points of adjoining patches.

### 5.2 Reparameterization

**Citation.** Farin, G. (2002), op. cit., chapters on reparameterization and arc length.

**What it establishes.** A curve's geometry is invariant under an orientation-preserving reparameterization $u = \phi(t)$, $\phi' > 0$; only its parameterization (speed) changes. This is the formal substrate of geometric continuity and of why "arc-length" is a canonical (but generally non-polynomial) parameterization.

**Key mathematics.** For $C(u)$ and $\tilde C(t) = C(\phi(t))$: $\tilde C' = \phi' \, C'$, $\tilde C'' = (\phi')^2 C'' + \phi'' C'$ (exactly the beta-1/beta-2 structure). Curvature $\kappa = \frac{\|C' \times C''\|}{\|C'\|^3}$ is reparameterization-invariant; arc length $s(u) = \int \|C'(\tau)\| \, d\tau$ generally has no closed form for polynomial/rational $C$, hence numerical quadrature is required.

**Significance.** Explains why arc-length parameterization cannot in general be represented exactly as a NURBS, and why kernels carry a separate numerical arc-length machinery.

**Kernel relevance.** Arc-length evaluation, equal-arc-length point distribution (for meshing and toolpaths), and reparameterization for compatibility all require numerical integration (Gauss-Legendre) plus root finding (Newton) against $s(u)$. The invariance results tell the kernel which quantities (curvature, torsion, tangent direction) are safe to compare across differently-parameterized curves.

---

## 6. The Rational Extension: NURBS

### 6.1 Versprille's rational B-splines

**Citation.** Versprille, K. J. (1975). *Computer-Aided Design Applications of the Rational B-Spline Approximation Form.* PhD thesis, Syracuse University. (Advisor: S. A. Coons.)

**What it establishes.** The first systematic treatment of *non-uniform rational B-splines*: Versprille generalized Riesenfeld's B-spline curves to the rational form by attaching weights, unifying conics and free-form curves in one representation and demonstrating their use in interactive CAD. This thesis is the direct ancestor of the NURBS that became the geometry standard (IGES, STEP, and Parasolid/ACIS-class kernels).

**Key mathematics.** A NURBS curve of degree $p$ with control points $P_i$ and positive weights $w_i$:
$$
C(u) = \frac{\sum_{i=0}^{n} w_i P_i N_{i,p}(u)}{\sum_{i=0}^{n} w_i N_{i,p}(u)} = \sum_{i=0}^{n} P_i R_{i,p}(u), \qquad R_{i,p}(u) = \frac{w_i N_{i,p}(u)}{\sum_{j=0}^{n} w_j N_{j,p}(u)}.
$$
The rational basis functions $R_{i,p}$ are nonnegative and form a partition of unity, so the convex-hull and variation-diminishing properties survive the rational extension. With all $w_i$ equal, $R_{i,p} = N_{i,p}$ and the polynomial B-spline is recovered.

**Significance.** Rational form is what lets one representation hold both exact conics/quadrics (which polynomials cannot) and sculptured surfaces, which is exactly the requirement of mechanical CAD.

**Kernel relevance.** NURBS is the canonical geometry type of a Parasolid-class kernel. Versprille's formulation is *the* internal curve/surface representation Keel should target: one type covering lines, arcs, conics, free-form, and (via tensor product) planes, cylinders, spheres, tori, and sculptured surfaces.

### 6.2 Projective interpretation and homogeneous coordinates

**Citations.**
- Patrikalakis, N. M., Maekawa, T., & Cho, W. (2009). *Shape Interrogation for Computer Aided Design and Manufacturing* (MIT hyperbook), section 1.5.
- Farin, G. (1999). *NURBS: From Projective Geometry to Practical Use* (2nd ed.). A K Peters.

**What it establishes.** A NURBS curve in $\mathbb{R}^d$ is the central projection of a *non-rational* B-spline curve living in $\mathbb{R}^{d+1}$. Weights are the extra homogeneous coordinate. This gives the cleanest theory: every NURBS algorithm is a polynomial B-spline algorithm performed in homogeneous space, followed by a perspective divide.

**Key mathematics.** Define weighted homogeneous control points $P_i^w = (w_i x_i, w_i y_i, w_i z_i, w_i) \in \mathbb{R}^{4}$ (the "4D" point). The non-rational B-spline curve in homogeneous space is
$$
C^w(u) = \sum_{i=0}^{n} P_i^w N_{i,p}(u) = \big( W(u)\,X(u),\, W(u)\,Y(u),\, W(u)\,Z(u),\, W(u) \big).
$$
The map $H: (X, Y, Z, W) \mapsto (X/W, Y/W, Z/W)$ (perspective divide, $W \neq 0$) yields the rational curve: $C(u) = H(C^w(u))$. Thus the rational curve is the image under perspective projection of a polynomial curve one dimension up.

**Significance.** This is the unifying abstraction: project once, and knot insertion, degree elevation, splitting, derivatives, and refinement are all done by the *polynomial* algorithms in homogeneous space. It also makes projective (perspective) invariance transparent.

**Kernel relevance.** Implementation strategy for Keel: store curves/surfaces in homogeneous coordinates $P_i^w$ and run the ordinary (non-rational) de Boor/knot-insertion/degree-elevation routines on the 4D points, dividing only at the final point query. This avoids special-casing rational algorithms and is the standard approach in The NURBS Book. Caution: the perspective divide breaks affine-but-not-projective operations; the kernel must keep weights positive to stay in the convex-hull-safe regime.

### 6.3 Geometric meaning of weights

**Citations.** Piegl, L. (1991). On NURBS: a survey. *IEEE CG&A*, 11(1), 55-71. Piegl & Tiller (1997), op. cit.

**What it establishes.** A weight $w_i$ is a projective "pull" toward control point $P_i$. Increasing $w_i$ draws the curve toward $P_i$; decreasing it pushes the curve away. The effect is *local* (within the support of $N_{i,p}$) and the curve always stays in the convex hull as long as weights are positive.

**Key mathematics.** Holding all but $w_i$ fixed, the point $C(u; w_i)$ moves along a straight line through $P_i$ as $w_i$ varies (a projective pencil); the locus is governed by
$$
C(u; w_i) = C(u; 0) + \frac{w_i N_{i,p}(u)}{\sum_j w_j N_{j,p}(u)}\big(P_i - C(u; 0)\big),
$$
so the fractional displacement toward $P_i$ is the *rational basis value* $R_{i,p}(u)$. As $w_i \to \infty$, $C(\xi_i) \to P_i$ (the curve is pulled onto the control point at its Greville parameter); as $w_i \to 0^+$, $P_i$ loses influence. Negative weights can make the denominator vanish (a pole / singularity) and are generally disallowed.

**Significance.** Weights add a continuous shape degree of freedom orthogonal to control-point position, and they are exactly the parameter needed to make a quadratic NURBS trace a conic.

**Kernel relevance.** A kernel must (a) keep weights strictly positive for robustness (a zero in the denominator is a fatal evaluation failure), (b) normalize weights when comparing/merging curves (a NURBS is invariant under uniform weight scaling, so canonicalize, e.g., $w_0 = 1$ or $\max w_i = 1$), and (c) handle the numerical ill-conditioning that arises when weights span many orders of magnitude.

### 6.4 Exact representation of conics and quadrics

**Citations.** Piegl & Tiller (1997), op. cit., chapter 7; Lee, E. (1987). The rational Bezier representation for conics. In *Geometric Modeling: Algorithms and New Trends* (SIAM).

**What it establishes.** Every conic arc is a quadratic ($p=2$) rational Bezier curve, and the weight of the middle control point alone selects ellipse/parabola/hyperbola. Full circles, cylinders, spheres, cones, and tori then follow as multi-segment NURBS curves and tensor-product NURBS surfaces.

**Key mathematics.** A conic arc with endpoints $P_0, P_2$ (weights 1) and shoulder point $P_1$ (weight $w_1$):
$$
C(u) = \frac{(1-u)^2 P_0 + 2u(1-u) w_1 P_1 + u^2 P_2}{(1-u)^2 + 2u(1-u) w_1 + u^2}, \qquad u \in [0,1].
$$
The classification by the conic shape factor:
- $w_1 < 1$: ellipse (arc),
- $w_1 = 1$: parabola (the rational form degenerates to ordinary quadratic Bezier),
- $w_1 > 1$: hyperbola.
For a circular arc of half-angle $\theta$ (with $P_1$ at the tangent intersection), $w_1 = \cos\theta$. A full circle is commonly built as a degree-2 NURBS with 9 control points (four 90 degree arcs, $w_1 = \cos 45^\circ = \tfrac{\sqrt2}{2}$ on the corner points) or 7 control points (three 120 degree arcs, $w_1 = \cos 60^\circ = \tfrac12$). Tensor-product extension gives exact spheres (surface of revolution of a semicircle), cylinders, cones, and tori.

**Significance.** This is the entire commercial justification for "R" in NURBS: a single representation that is *exact* for the analytic surfaces (planes, quadrics, torus) that dominate mechanical parts, and also handles free-form. No polynomial scheme can do this (a circle has no polynomial parameterization).

**Kernel relevance.** Keel can store *all* its analytic geometry (lines, arcs, conics, planes, cylinders, spheres, cones, tori) as NURBS and still be exact, OR keep dedicated analytic types and convert to NURBS on demand. The conic weight formulas above are the exact converters. Critically, the kernel must preserve exactness end-to-end: a circle that becomes a slightly-off NURBS will accumulate error through Booleans. Many production kernels keep a hybrid (analytic primitives plus NURBS for free-form) precisely to avoid this drift.

---

## 7. Derivative Formulas

### 7.1 B-spline derivatives and hodographs

**Citation.** de Boor, C. (2001), op. cit.; Piegl & Tiller (1997), op. cit., chapter 3.

**What it establishes.** The derivative of a B-spline curve is again a B-spline curve, of degree $p-1$, on (almost) the same knot vector, with control points that are scaled forward differences. Higher derivatives iterate this.

**Key mathematics.** Basis-function derivative:
$$
N_{i,p}'(u) = \frac{p}{u_{i+p} - u_i} N_{i,p-1}(u) - \frac{p}{u_{i+p+1} - u_{i+1}} N_{i+1,p-1}(u).
$$
Curve derivative (hodograph) of $C(u) = \sum_i P_i N_{i,p}(u)$:
$$
C'(u) = \sum_{i=0}^{n-1} Q_i\, N_{i+1,p-1}(u), \qquad Q_i = \frac{p}{u_{i+p+1} - u_{i+1}}\,(P_{i+1} - P_i),
$$
on the knot vector with the first and last knots dropped. The $k$-th derivative iterates this with falling-factorial scaling.

**Significance.** Closed-form, exact derivatives of all orders, again in the same family, mean tangents, normals, and curvature are computed without numerical differentiation.

**Kernel relevance.** Tangent vectors, surface normals ($S_u \times S_v$), and curvature feed nearly every kernel operation: offsetting, shelling, draft analysis, silhouette/silhouette-edge computation, and Newton iteration for intersections (which needs the Jacobian, i.e., derivatives). The hodograph control polygon also bounds the derivative, giving a guaranteed tangent cone used in robust intersection and self-intersection tests.

### 7.2 Rational (NURBS) derivatives: the quotient-rule structure

**Citations.** Piegl & Tiller (1997), op. cit., section 4.5; Floater, M. S. (1992). Derivatives of rational Bezier curves. *CAGD*, 9(3), 161-181.

**What it establishes.** Because a NURBS curve is a quotient $C(u) = A(u)/w(u)$ of a vector numerator and a scalar denominator (both polynomial B-splines), its derivatives follow the quotient rule but are best computed via a recursion that avoids dividing by high powers of $w$.

**Key mathematics.** Write the homogeneous curve as $C^w(u) = (A(u), w(u))$ where $A(u) = \sum_i w_i P_i N_{i,p}(u)$ and $w(u) = \sum_i w_i N_{i,p}(u)$. Then $C(u) = A(u)/w(u)$ and the first derivative is
$$
C'(u) = \frac{A'(u) - w'(u)\, C(u)}{w(u)},
$$
which is the quotient rule rearranged to reuse $C(u)$ and avoid $w^2$. The general Leibniz-style recursion (The NURBS Book) gives the $k$-th derivative:
$$
C^{(k)}(u) = \frac{A^{(k)}(u) - \sum_{j=1}^{k} \binom{k}{j} w^{(j)}(u)\, C^{(k-j)}(u)}{w(u)},
$$
where $A^{(j)}$ and $w^{(j)}$ are computed by the polynomial B-spline derivative formulas above. Floater (1992) further showed the rational Bezier hodograph can be written as a rational Bezier curve of bounded degree, with explicit control points and weights.

**Significance.** The quotient structure means a NURBS derivative is *not* a NURBS of one lower degree in general (the denominator persists), but the recursion keeps evaluation stable and division-light. This is the practical formula every kernel uses for rational tangents/normals.

**Kernel relevance.** Surface normals on a rational surface, curvature for analytic surfaces stored as NURBS, and Newton Jacobians for surface-surface intersection all run through this recursion. The kernel must guard $w(u) > 0$ (else division blows up) and should compute derivatives in homogeneous space then apply this recursion, mirroring the projective strategy of section 6.2.

---

## 8. Canonical Textbooks and Surveys

### 8.1 Piegl and Tiller, *The NURBS Book*

**Citation.** Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Monographs in Visual Communication, Springer.

**What it establishes.** The definitive engineering reference for NURBS: it collects, with pseudocode, every algorithm a kernel needs (basis evaluation, de Boor, knot insertion/removal, refinement, degree elevation/reduction, point/derivative evaluation, conic and surface construction, fitting/interpolation, and surface algorithms). Notation here ($N_{i,p}$, $R_{i,p}$, $C^w$) is the de facto standard.

**Key mathematics.** Consolidates all of sections 2-7 above into implementable algorithms: `FindSpan`, `BasisFuns`, `DersBasisFuns`, `CurvePoint`, `CurveDerivsAlg`, `RatCurveDerivs`, `CurveKnotIns`, `RefineKnotVectCurve`, `DegreeElevateCurve`, plus the surface analogues and global/local interpolation/approximation.

**Significance.** It is the single most cited practical NURBS reference and the closest thing to a specification of how production kernels implement NURBS.

**Kernel relevance.** This should be Keel's primary implementation reference for the NURBS layer. The algorithm catalogue maps almost one-to-one onto the methods a kernel's curve/surface classes must expose.

### 8.2 de Boor, *A Practical Guide to Splines*

**Citation.** de Boor, C. (2001). *A Practical Guide to Splines* (rev. ed.). Applied Mathematical Sciences 27, Springer. (1st ed. 1978.)

**What it establishes.** The rigorous numerical-analysis treatment of splines: the recurrence, stability, dual functionals, the de Boor-Fix functionals, knot insertion, condition numbers, and best-approximation/quasi-interpolation. The companion to the engineering-oriented NURBS Book.

**Key mathematics.** The de Boor-Fix dual functionals $\lambda_i$ satisfy $\lambda_i(N_{j,p}) = \delta_{ij}$, giving the coefficient-extraction formula
$$
\lambda_i(f) = \sum_{k=0}^{p} (-1)^k \psi_{i,p}^{(p-k)}(\tau)\, f^{(k)}(\tau) / p!,
$$
for any $\tau$ in the support, where $\psi_{i,p}(y) = \prod_{j=1}^{p}(u_{i+j} - y)$ (the Marsden polynomial). These functionals are the tool for quasi-interpolation and for bounding $\|$coefficients$\|$ by $\|f\|$ (stability).

**Significance.** Provides the theoretical guarantees (stability constants, approximation orders) that the practical algorithms rely on, and the dual-functional machinery for provably good local fitting.

**Kernel relevance.** Quasi-interpolation (local, matrix-free fitting via dual functionals) is valuable for fast surface fitting and for robust conversion. The stability theory tells the kernel how much error a coefficient perturbation can cause, which matters for tolerant modeling.

### 8.3 Farin, *Curves and Surfaces for CAGD*

**Citation.** Farin, G. (2002). *Curves and Surfaces for Computer-Aided Geometric Design: A Practical Guide* (5th ed.). Morgan Kaufmann.

**What it establishes.** The standard pedagogical text bridging the geometry and the algorithms: Bezier/de Casteljau, B-splines, rational forms, geometric continuity, blossoming, interpolation, and a strong emphasis on geometric intuition (convex hull, variation diminishing, affine invariance) and on the history of the field.

**Significance.** Best source for *why* the algorithms work geometrically; introduces blossoming pedagogically and treats geometric continuity and shape thoroughly.

**Kernel relevance.** The conceptual foundation for design decisions: which invariants to preserve (affine invariance, convex hull), how to think about continuity at edges, and how to reason about shape quality.

### 8.4 Prautzsch, Boehm, and Paluszny, *Bezier and B-Spline Techniques*

**Citation.** Prautzsch, H., Boehm, W., & Paluszny, M. (2002). *Bezier and B-Spline Techniques.* Springer.

**What it establishes.** A blossoming-centric, mathematically polished development of all the core algorithms (Boehm is the originator of the standard knot-insertion algorithm). Especially clean treatments of knot insertion, degree elevation, and the polar-form (blossom) derivations.

**Key mathematics.** Boehm's knot insertion: to insert a knot $\bar u \in [u_k, u_{k+1})$ into a degree-$p$ curve, the new control points are
$$
Q_i = (1 - \alpha_i) P_{i-1} + \alpha_i P_i, \quad \alpha_i = \begin{cases} 1 & i \le k-p \\ \dfrac{\bar u - u_i}{u_{i+p} - u_i} & k-p+1 \le i \le k \\ 0 & i \ge k+1. \end{cases}
$$
Only $p$ control points change; the curve is unchanged.

**Significance.** Knot insertion (Boehm) and the Oslo algorithm (Cohen-Lyche-Riesenfeld, refinement of many knots at once) are the foundation of subdivision-into-Bezier, curve splitting, and refinement.

**Kernel relevance.** Knot insertion is one of the most-used kernel primitives: it splits curves/surfaces at parameters (for trimming and intersection), converts to per-span Bezier (for tessellation and clipping), and refines control nets for fitting. Boehm's formula and the Oslo algorithm are both must-haves.

### 8.5 Cohen, Riesenfeld, and Elber, *Geometric Modeling with Splines*

**Citation.** Cohen, E., Riesenfeld, R. F., & Elber, G. (2001). *Geometric Modeling with Splines: An Introduction.* A K Peters.

**What it establishes.** A modeling-oriented treatment from the group that produced the Oslo algorithm and much of practical spline modeling (Riesenfeld introduced B-spline curves to CAGD in 1973; Cohen-Lyche-Riesenfeld gave discrete B-splines / the Oslo algorithm in 1980). Strong on refinement, subdivision, and symbolic/algebraic operations on splines (Elber's specialty).

**Key mathematics.** Discrete B-splines and the Oslo algorithm express refinement from a knot vector $U$ to a refinement $\bar U \supset U$ via a banded *knot-insertion matrix* $\alpha$:
$$
N_{j,p}^{U}(u) = \sum_i \alpha_{j,i}^{(p)}\, N_{i,p}^{\bar U}(u),
$$
computed by a Cox-de Boor-like recurrence on $\alpha$.

**Significance.** Refinement-as-a-matrix is the basis of multiresolution modeling, subdivision surfaces, and of symbolic spline arithmetic (sum, product, composition of splines), which Elber's IRIT system pioneered.

**Kernel relevance.** Symbolic spline operations (product of two B-splines is a B-spline of summed degree on a merged knot vector) power exact computation of dot products, distance fields, and curvature as splines, useful for robust intersection and analysis. The Oslo refinement matrix is the tool for compatible refinement before Boolean/loft operations.

### 8.6 Schumaker, *Spline Functions: Basic Theory*

**Citation.** Schumaker, L. L. (2007). *Spline Functions: Basic Theory* (3rd ed.). Cambridge University Press. (1st ed. 1981, Wiley.)

**What it establishes.** The most complete pure-mathematics reference on splines: existence/uniqueness, dimension counts, B-spline theory, Schoenberg-Whitney, total positivity, approximation orders, and Tchebycheffian (generalized) splines. The rigorous backbone behind the practical texts.

**Significance.** Where one goes for proofs and for the general theory (e.g., splines over arbitrary extended complete Chebyshev systems, multivariate splines).

**Kernel relevance.** Reference of last resort for correctness questions (dimension of a spline space, well-posedness of a fit, exact continuity counts), and the source for advanced generalizations (Tchebycheffian splines) if Keel ever needs exponential/trigonometric exact forms beyond NURBS.

---

## 9. Important Theoretical Results Post-2000 Relevant to Practice

### 9.1 Optimality and conditioning of the B-spline basis

**Citations.**
- Lyche, T. (1978). A note on the condition numbers of the B-spline bases. *Journal of Approximation Theory*, 22(3), 202-205.
- Lyche, T., & Peña, J. M. (2004). Optimally stable multivariate bases. *Advances in Computational Mathematics*, 20(1-3), 149-159.
- Lyche, T., Manni, C., & Speleers, H. (2018). Foundations of spline theory: B-splines, spline approximation, and hierarchical refinement. In *Splines and PDEs*, Lecture Notes in Mathematics 2219, Springer, 1-76.

**What it establishes.** Among all *nonnegative* bases (in particular, all nonnegative partition-of-unity bases) for a given spline space, the B-spline basis is *optimally conditioned*: no normalized nonnegative basis has a smaller condition number (up to scaling/permutation). Lyche-Peña extend this to the tensor-product B-spline and triangular Bernstein bases. The condition number itself is bounded by a constant depending only on the degree, not on the knots.

**Key mathematics.** The $L_\infty$ condition number of the B-spline basis,
$$
\kappa_\infty = \sup_{c \neq 0} \frac{\|c\|_\infty}{\|\sum_i c_i N_{i,p}\|_\infty} \cdot \sup_{c \neq 0} \frac{\|\sum_i c_i N_{i,p}\|_\infty}{\|c\|_\infty},
$$
satisfies a knot-independent bound; classical estimates give $\kappa < p\,2^{p}$ (refined) and the cruder $\kappa < p\,9^{p}$. de Boor's conjecture (1972), later substantiated, is that interpolation at knot averages is bounded by a function of $p$ alone, independent of the knot spacing.

**Significance.** This is the rigorous justification for using the B-spline basis (rather than monomials, which have condition numbers growing exponentially worse and knot-dependent) in *all* numerical work: the representation is as stable as any nonnegative basis can be.

**Kernel relevance.** It tells the kernel author that the B-spline form is the numerically safe internal representation, and that conditioning degrades with *degree*, not knot non-uniformity. Practical takeaway: cap working degree (most CAD uses $p \le 5$ for curves, $\le 3$ commonly), and prefer per-span Bezier ($p$ small, single span) for the most numerically delicate operations.

### 9.2 Blossoming / polar forms (Ramshaw and de Casteljau)

**Citations.**
- Ramshaw, L. (1987). *Blossoming: A Connect-the-Dots Approach to Splines.* DEC Systems Research Center, Report 19.
- Ramshaw, L. (1989). Blossoms are polar forms. *Computer Aided Geometric Design*, 6(4), 323-358.

**What it establishes.** Every degree-$p$ polynomial $F$ has a unique symmetric, multi-affine "blossom" (polar form) $f(t_1, \dots, t_p)$ with $f(t, \dots, t) = F(t)$. The blossom unifies and trivializes the derivations of de Casteljau, the de Boor algorithm, knot insertion, and degree elevation: control points are simply blossom values at consecutive knot arguments.

**Key mathematics.** The blossom $f$ of a degree-$p$ polynomial curve $F$ is the unique function $f: \mathbb{R}^p \to \mathbb{R}^d$ that is
1. **Symmetric:** $f(t_{\sigma(1)}, \dots, t_{\sigma(p)}) = f(t_1, \dots, t_p)$ for any permutation $\sigma$;
2. **Multi-affine:** affine in each argument separately;
3. **Diagonal:** $f(\underbrace{t, \dots, t}_{p}) = F(t)$.

Then the **B-spline control points are blossom values at knot windows**:
$$
P_i = f(u_{i+1}, u_{i+2}, \dots, u_{i+p}),
$$
and Bezier control points are $P_i = f(\underbrace{a, \dots, a}_{p-i}, \underbrace{b, \dots, b}_{i})$ on $[a,b]$. The de Boor/de Casteljau recurrence is just the multi-affine blossom evaluated by replacing one repeated argument at a time:
$$
f(\dots, u, \dots) = (1-\alpha)\, f(\dots, u_a, \dots) + \alpha\, f(\dots, u_b, \dots).
$$

**Significance.** Blossoming is the most elegant modern foundation of the whole subject: knot insertion, the conversion between Bezier and B-spline, and degree elevation become one-line consequences of "evaluate the symmetric multi-affine form at the right arguments." de Casteljau had the idea (his "poles") earlier; Ramshaw gave it the clean theory and the name.

**Kernel relevance.** Even if Keel implements the classical recurrences directly, the blossom is the best *mental model* for verifying their correctness and for deriving new conversions (e.g., extracting Bezier segments, computing the control points of a derivative or product curve, or implementing degree elevation). It is the cleanest way to reason about and test the kernel's knot-manipulation code.

### 9.3 Total positivity as the unifying shape theory

**Citations.** Carnicer, J. M., & Peña, J. M. (1994). Totally positive bases for shape preserving curve design and optimality of B-splines. *CAGD*, 11(6), 633-654. Goodman, T. N. T. (2002). Shape preserving representations. (Survey.)

**What it establishes.** Among all bases of a space of functions, the *normalized totally positive* basis (the B-spline basis in the spline case, the Bernstein basis in the polynomial case) is the one that is most shape-preserving and best-conditioned; it is the unique (up to scaling) "optimal" totally positive basis. This ties together variation diminishing, convex hull, and conditioning under one theorem.

**Key mathematics.** A basis is *totally positive* if its collocation matrices are totally nonnegative. Carnicer-Peña prove the existence of a *normalized B-basis* (the unique TP basis with optimal shape-preserving and stability properties) for any space possessing a TP basis; for splines this normalized B-basis is exactly the B-spline basis.

**Significance.** It is the theoretical capstone explaining why B-splines/Bernstein are not just *a* good choice but *the* optimal choice for geometric design among all bases of their spaces.

**Kernel relevance.** Confirms that the kernel's representation choices (B-spline basis internally, per-span Bernstein for delicate ops) are provably optimal, not merely conventional. It also guides any future extension (Tchebycheffian, exponential splines): seek the normalized B-basis of the target space to retain shape and stability guarantees.

---

## 10. Theoretical Toolkit for a Kernel Implementer

A kernel author building Keel's NURBS layer must have the following internalized, in roughly priority order.

1. **The Cox-de Boor recurrence and the de Boor evaluation algorithm.** Both the "compute the $p+1$ nonzero basis values" form (for fitting/assembly) and the "evaluate by repeated affine interpolation" form (for stable point evaluation). With the $0/0 := 0$ convention and robust span finding.

2. **The homogeneous-coordinate projective model.** Store control points as $P_i^w = (w_i P_i, w_i)$ and run all algorithms (evaluation, knot insertion, degree elevation, splitting, refinement, derivatives) as *non-rational* B-spline algorithms on the 4D points, dividing only at the final query. Keep all weights strictly positive.

3. **Knot-vector discipline.** Nondecreasing knots; clamped end multiplicity $p+1$ as the canonical form; continuity $= C^{p-\mu}$ at a knot of multiplicity $\mu$; canonicalization of weights (a NURBS is invariant under uniform weight scaling) and of representation (clamped vs periodic).

4. **The core conversion primitives.** Knot insertion (Boehm), knot refinement (Oslo / Cohen-Lyche-Riesenfeld), degree elevation, conversion to per-span rational Bezier, and curve/surface splitting. Each is exact and is used pervasively for trimming, intersection prep, tessellation, and operand compatibility.

5. **Derivative machinery.** B-spline derivative (hodograph) formula; the rational quotient-rule recursion $C^{(k)} = [A^{(k)} - \sum_j \binom{k}{j} w^{(j)} C^{(k-j)}]/w$. Needed for tangents, surface normals $S_u \times S_v$, curvature, and Newton Jacobians in intersection.

6. **Exact conic/quadric construction.** The quadratic rational Bezier conic formula and shape factor ($w_1 = \cos\theta$ for circular arcs; $w_1 \lessgtr 1$ for ellipse/parabola/hyperbola), and the standard 7- or 9-point full-circle and tensor-product sphere/cylinder/torus constructions. Decide the hybrid analytic-plus-NURBS strategy and preserve exactness across operations.

7. **Well-posedness of fitting (Schoenberg-Whitney).** Parameterization (chord-length/centripetal) plus averaging knots so $u_i < x_i < u_{i+p+1}$ holds, giving a banded, totally-nonnegative, solvable interpolation system.

8. **Continuity reasoning at edges (geometric continuity).** $G^1$ tangent-plane and $G^2$ curvature continuity across surface boundaries (independent parameterizations), via the beta-constraints. Essential for blends, fillets, and class-A quality. Plus the reparameterization facts and the need for numerical arc length.

9. **The shape/stability guarantees and their limits.** Convex hull and variation diminishing (from nonnegativity, partition of unity, total positivity); the B-spline basis is optimally conditioned among nonnegative bases, with conditioning bounded by degree alone ($\kappa < p\,2^p$). Hence: keep degree modest, prefer per-span Bezier for the touchiest numerics, and never trust a monomial basis.

10. **Blossoming as the verification lens.** The symmetric multi-affine polar form, $P_i = f(u_{i+1}, \dots, u_{i+p})$, as the cleanest way to derive and test every knot-manipulation and degree-manipulation routine.

The throughline: a Parasolid-class kernel can represent essentially all of its geometry (analytic and free-form, curves and surfaces) as NURBS, run nearly every algorithm as a non-rational B-spline computation in homogeneous space, and lean on total positivity and the B-spline basis's optimality for both shape predictability and numerical safety. The mathematics above is exactly the layer on which the harder kernel problems (intersection, Booleans, tolerant modeling) are subsequently built.

---

## References

1. Barsky, B. A. (1981). *The Beta-spline: A Local Representation Based on Shape Parameters and Fundamental Geometric Measures.* PhD thesis, University of Utah.
2. Barsky, B. A., & DeRose, T. D. (1989). Geometric continuity of parametric curves: three equivalent characterizations. *IEEE Computer Graphics and Applications*, 9(6), 60-68.
3. Barsky, B. A., & DeRose, T. D. (1990). Deriving the beta-constraints for geometric continuity of parametric curves. *Milan Journal of Mathematics* (formerly Rendiconti).
4. Bezier, P. (1966/1967). Definition numerique des courbes et surfaces. *Automatisme.*
5. Boehm, W. (1980). Inserting new knots into B-spline curves. *Computer-Aided Design*, 12(4), 199-201.
6. Carnicer, J. M., & Peña, J. M. (1994). Totally positive bases for shape preserving curve design and optimality of B-splines. *Computer Aided Geometric Design*, 11(6), 633-654.
7. Cohen, E., Lyche, T., & Riesenfeld, R. F. (1980). Discrete B-splines and subdivision techniques in computer-aided geometric design and computer graphics (the Oslo algorithm). *Computer Graphics and Image Processing*, 14(2), 87-111.
8. Cohen, E., Riesenfeld, R. F., & Elber, G. (2001). *Geometric Modeling with Splines: An Introduction.* A K Peters.
9. Cox, M. G. (1972). The numerical evaluation of B-splines. *Journal of the Institute of Mathematics and its Applications*, 10(2), 134-149.
10. Curry, H. B., & Schoenberg, I. J. (1966). On Polya frequency functions IV: The fundamental spline functions and their limits. *Journal d'Analyse Mathematique*, 17, 71-107.
11. de Boor, C. (1972). On calculating with B-splines. *Journal of Approximation Theory*, 6(1), 50-62.
12. de Boor, C. (2001). *A Practical Guide to Splines* (rev. ed.). Applied Mathematical Sciences 27, Springer. (1st ed. 1978.)
13. de Casteljau, P. (1959, 1963). *Courbes et surfaces a poles.* Internal Citroen technical reports.
14. Farin, G. (1999). *NURBS: From Projective Geometry to Practical Use* (2nd ed.). A K Peters.
15. Farin, G. (2002). *Curves and Surfaces for Computer-Aided Geometric Design: A Practical Guide* (5th ed.). Morgan Kaufmann.
16. Floater, M. S. (1992). Derivatives of rational Bezier curves. *Computer Aided Geometric Design*, 9(3), 161-181.
17. Floater, M. S. (2022). The Schoenberg-Whitney theorem and total positivity. Lecture notes, University of Oslo, MAT4170.
18. Karlin, S. (1968). *Total Positivity, Vol. I.* Stanford University Press.
19. Lee, E. T. Y. (1987). The rational Bezier representation for conics. In *Geometric Modeling: Algorithms and New Trends*, SIAM, 3-19.
20. Lyche, T. (1978). A note on the condition numbers of the B-spline bases. *Journal of Approximation Theory*, 22(3), 202-205.
21. Lyche, T., & Peña, J. M. (2004). Optimally stable multivariate bases. *Advances in Computational Mathematics*, 20(1-3), 149-159.
22. Lyche, T., Manni, C., & Speleers, H. (2018). Foundations of spline theory: B-splines, spline approximation, and hierarchical refinement. In *Splines and PDEs: From Approximation Theory to Numerical Linear Algebra*, Lecture Notes in Mathematics 2219, Springer, 1-76.
23. Patrikalakis, N. M., Maekawa, T., & Cho, W. (2009). *Shape Interrogation for Computer Aided Design and Manufacturing.* MIT online hyperbook edition.
24. Piegl, L. (1991). On NURBS: a survey. *IEEE Computer Graphics and Applications*, 11(1), 55-71.
25. Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer.
26. Prautzsch, H., Boehm, W., & Paluszny, M. (2002). *Bezier and B-Spline Techniques.* Springer.
27. Ramshaw, L. (1987). *Blossoming: A Connect-the-Dots Approach to Splines.* DEC Systems Research Center, Report 19.
28. Ramshaw, L. (1989). Blossoms are polar forms. *Computer Aided Geometric Design*, 6(4), 323-358.
29. Riesenfeld, R. F. (1973). *Applications of B-spline Approximation to Geometric Problems of Computer-Aided Design.* PhD thesis, Syracuse University.
30. Schoenberg, I. J. (1946). Contributions to the problem of approximation of equidistant data by analytic functions. *Quarterly of Applied Mathematics*, 4, 45-99 and 112-141.
31. Schoenberg, I. J., & Whitney, A. (1953). On Polya frequency functions III. *Transactions of the American Mathematical Society*, 74, 246-259.
32. Schumaker, L. L. (2007). *Spline Functions: Basic Theory* (3rd ed.). Cambridge University Press. (1st ed. 1981.)
33. Versprille, K. J. (1975). *Computer-Aided Design Applications of the Rational B-Spline Approximation Form.* PhD thesis, Syracuse University.
