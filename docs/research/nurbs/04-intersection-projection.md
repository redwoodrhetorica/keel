# NURBS / Spline Intersection and Projection Algorithms

Research dossier 04 for the Keel B-rep kernel. This file goes deep on the numerical and algebraic machinery behind curve-curve, curve-surface, and surface-surface intersection (SSI), point projection and inversion, ray-surface intersection, distance and Hausdorff computation, and the robustness theory that ties them together. It both expands the summary-level survey already done (Projected Polyhedron, Bezier clipping, Mourrain-Pavone, Yuksel, the 2023 TOG paper, Selimovic, Ma and Hewitt) and adds coverage the survey missed (validated interval ODE marching, Krishnan-Manocha lower-dimensional SSI, Hohmeyer Gauss-map loop detection, mu-bases, Manocha-Canny eigenvalue resultants, Toth interval ray tracing, second-order geometric projection, Hausdorff distance algorithms, the 2025 error-controlled tracing paper).

A standing note on style: no em-dashes appear anywhere in this document by deliberate choice.

---

## 0. Why intersection is the hard core of a B-rep kernel

In a boundary-representation kernel, almost every Boolean operation, fillet, shell, offset, draft, and imprint resolves to one or more of three primitives: intersect two surfaces and get a curve (SSI), intersect a curve with a surface and get points (CSI), and project or invert a point onto a curve or surface. The defining requirement that separates a toy modeler from a Parasolid-class kernel is not raw speed but topological correctness under near-degenerate configurations: tangential contact, near-tangency, tiny intersection loops, coincident regions, and grazing trims. A wrong branch count or a missed small loop corrupts the B-rep topology and the error propagates silently into every downstream operation. Consequently the literature splits into two cultures that Keel must blend:

1. The numeric-geometric culture (subdivision, clipping, marching, Newton). Fast, general, but historically unable to certify that it found every branch with the correct topology.
2. The algebraic culture (resultants, implicitization, eigenvalue solvers, mu-bases, Groebner). Exact in principle, gives global guarantees, but expensive and numerically delicate at high degree.

The modern frontier (2023-2025) is the certified hybrid: subdivision plus interval arithmetic plus algebraic topology tests that deliver both speed and a provable topology guarantee. Keel should be architected around that frontier.

---

## 1. Curve-curve intersection

### 1.1 Bezier clipping (Sederberg and Nishita 1990)

**Citation.** Sederberg, T. W., and Nishita, T. (1990). Curve intersection using Bezier clipping. Computer-Aided Design, 22(9), 538-549.

**Algorithm.** Given two planar Bezier curves P(t) (degree m) and Q(s) (degree n), the goal is the set of (t, s) with P(t) = Q(s). Bezier clipping exploits the convex hull property to throw away large parameter ranges that provably contain no intersection. The construction:

- Pick a "fat line" L for curve Q. The natural choice is the line through Q's two endpoints, written in normalized implicit form L(x, y) = a x + b y + c = 0 with a^2 + b^2 = 1, so that L evaluated at a point equals the signed distance to the line. A fat line is the strip d_min <= L <= d_max where d_min and d_max are the minimum and maximum of L over Q's control points (which bound L over all of Q by convexity).
- Form the signed distance of the other curve P from L: d(t) = a x_P(t) + b y_P(t) + c. Because P is Bezier and L is affine, d(t) is itself a degree-m polynomial in Bernstein form whose control points are D_i = (i/m, L(P_i)) for the i-th control point P_i of P. The "explicit" Bezier curve (i/m, D_i) bounds d(t) by its convex hull.
- Intersect the convex hull of the points (i/m, L(P_i)) with the horizontal strip [d_min, d_max]. Any t for which P can intersect Q must satisfy d_min <= d(t) <= d_max, so the hull intersected with the strip yields a sub-interval [t_lo, t_hi] of [0, 1]. Everything outside is discarded.
- Clip P to [t_lo, t_hi] with de Casteljau, swap roles (clip Q against a fat line of the new P), and iterate. When an interval stops shrinking by a heuristic factor (typically a clip removes less than about 20 percent), the algorithm subdivides at the midpoint and recurses on both halves, which is how multiple intersections are separated.

For ray tracing and other applications the second curve degenerates to a line, so only one curve is clipped against the fixed line. Two fat lines, one parallel and one perpendicular to the chord, are used to clip in both parameters.

**Convergence and robustness.** Empirically quadratic at transversal (simple) intersections. The formal proof that Bezier clipping is quadratically convergent (and that the convergence is independent of which fat-line family is used) came later, from Schulz (2009), Bezier clipping is quadratically convergent, CAGD 26(1), 61-74. At tangential intersections, where d(t) has a double root, convergence degrades to linear, exactly the behavior of Newton at a multiple root. Robustness is good because every step is a guaranteed convex-hull exclusion: the algorithm never discards a real root, so it is conservative and complete down to the subdivision tolerance.

**Failure modes.** Tangential and high-multiplicity intersections cause slow linear convergence and clusters of nearly identical roots that are hard to disambiguate at finite tolerance. Overlapping (coincident) curve segments are not handled by clipping at all and need a separate coincidence test. High curve degree slows each de Casteljau clip (O(m^2)).

**Kernel relevance.** Bezier clipping is the workhorse for 2D curve-curve intersection in the parameter domain (for example when intersecting two trimming curves on a shared surface, or computing curve self-intersections). Keel should implement it with the perpendicular-plus-parallel fat-line pair and fall back to subdivision on stall. It is also the kernel of ray-trim-curve tests in trimmed surface ray casting.

### 1.2 Quadratic and cubic clipping; hybrid clipping (Barton and Juttler 2007; Liu et al. 2009; Lou and Liu 2022)

**Citations.**
- Barton, M., and Juttler, B. (2007). Computing roots of polynomials by quadratic clipping. Computer Aided Geometric Design, 24(3), 125-141.
- Liu, L., Zhang, L., Lin, B., and Wang, G. (2009). Fast approach for computing roots of polynomials using cubic clipping. Computer Aided Geometric Design, 26(5), 547-559.
- Lou, Q., and Liu, L. (2022). Curve intersection based on cubic hybrid clipping. Visual Computing for Industry, Biomedicine, and Art, 5, 16.

**Algorithm.** Where Bezier clipping bounds the curve by a line (degree 1), quadratic and cubic clipping bound the difference or distance function by a low-degree polynomial that hugs it more tightly. Quadratic clipping fits a degree-2 polynomial q(t) to the target function f(t) by least squares in the Bernstein basis, then bounds the approximation error |f(t) - q(t)| <= delta by a constant computed from the Bernstein coefficients of the remainder. The two parabolas q(t) +/- delta are intersected with the t-axis (solving quadratics directly), and the union of those root intervals is the new clipped domain.

Cubic hybrid clipping (Lou and Liu 2022) is the state of the art for curve-curve intersection. It writes the distance function in a "cubic hybrid" Bernstein form

  d_hat(t) = (1-t)^3 d_hat_0 + 3t(1-t)^2 d_hat_1(t) + 3t^2(1-t) d_hat_2(t) + t^3 d_hat_3,

where d_hat_0 and d_hat_3 are scalars (the endpoint values) and d_hat_1(t), d_hat_2(t) are themselves Bezier curves of degree n-3 (the "moving control points"). Tight scalar bounds d_{i,min} and d_{i,max} on the moving coefficients give cubic upper and lower bounding polynomials

  d_min(t) = B0 d_hat_0 + B1 d_{1,min} + B2 d_{2,min} + B3 d_hat_3,
  d_max(t) = B0 d_hat_0 + B1 d_{1,max} + B2 d_{2,max} + B3 d_hat_3,

with Bi the cubic Bernstein basis. A blend parameter lambda in [0,1] mixes two representations and is chosen by linear optimization to minimize the gap d_max - d_min. Clipping solves the four cubics d_min(t) = d_min/d_max and d_max(t) = d_min/d_max via the closed-form cubic formula to get the new interval [alpha_{k+1}, beta_{k+1}].

**Convergence and robustness.** Cubic bounds approximate the distance function to O(h^4) (Lemma 1 of Lou and Liu). This gives order-4 convergence at a single root (gamma = 4) and order-2 at a double root (gamma = 2). For transversal curve-curve intersection the f-interval converges at order 4 and the g-interval at order 2. Quadratic clipping gives order-3 at simple roots, order-2 at doubles. On 40,000 random curve pairs of degree 4 to 10, cubic hybrid clipping ran about 30 percent faster than 2-clipping and about 60 percent faster than Bezier clipping at the same accuracy.

**Failure modes.** Same degeneracies as Bezier clipping (multiple roots drop to quadratic order). The error-bound and lambda optimization add per-iteration overhead that only pays off when high accuracy is demanded; at coarse tolerance plain Bezier clipping can win.

**Kernel relevance.** For the precision a CAD kernel needs (typically 1e-7 to 1e-12 model units), the higher convergence order of cubic clipping translates into materially fewer iterations and de Casteljau subdivisions. Keel's 1D root engine for curve-curve and ray-curve work should default to cubic clipping with Bezier clipping as a cheap first pass.

### 1.3 Implicitization and resultants for curve intersection (Sederberg; Manocha and Canny)

**Citations.**
- Sederberg, T. W., Anderson, D. C., and Goldman, R. N. (1984). Implicit representation of parametric curves and surfaces. Computer Vision, Graphics, and Image Processing, 28(1), 72-84.
- Manocha, D., and Canny, J. F. (1992). Multipolynomial resultants and linear algebra. Proc. ISSAC 1992, ACM, 96-102.
- Manocha, D., and Canny, J. F. (1992). Algorithms for implicitizing rational parametric surfaces. Computer Aided Geometric Design, 9(1), 25-50.

**Algorithm.** To intersect a parametric curve C1(t) = (x(t), y(t)) with a parametric curve C2(s) algebraically, implicitize C2 into f(x, y) = 0 by eliminating s from x = X(s), y = Y(s) using the Sylvester resultant: f(x, y) = Res_s(X(s) - x, Y(s) - y). Substituting C1 gives a single univariate polynomial f(C1(t)) = 0 whose roots are the t-parameters of intersection. The Sylvester resultant of two polynomials of degree d is the determinant of a 2d-by-2d matrix; its vanishing is exactly the condition that the two polynomials share a root.

Manocha and Canny's contribution is to avoid expanding that symbolic determinant (which is enormous and numerically unstable) and instead pose root finding as an eigenvalue problem. A resultant matrix M(x) whose entries are polynomials in the eliminated variable can be written as a matrix polynomial M(x) = M_0 + x M_1 + ... + x^k M_k. Finding the x where det M(x) = 0 is a polynomial eigenvalue problem, which is linearized into a generalized eigenvalue problem A v = x B v of a "companion-like" pencil. The intersection parameters are then the eigenvalues of a single matrix, computed by standard, backward-stable numerical linear algebra (QZ algorithm), and the corresponding eigenvectors recover the other coordinates. For sparse systems they use the sparse (Newton polytope) resultant of Gelfand-Kapranov-Zelevinsky rather than the dense Macaulay resultant.

**Convergence and robustness.** No iteration to converge: the method is direct. Accuracy is governed by the conditioning of the eigenvalue problem, which is far better than expanding determinants symbolically. It finds all intersections (including complex ones, which are discarded) in one shot, giving a global guarantee on the count that subdivision methods only get asymptotically.

**Failure modes.** Base points and degenerate parametrizations make the resultant vanish identically or introduce extraneous factors. The matrix size grows as the product of degrees, so cost scales poorly for high-degree NURBS (a bicubic times bicubic surface intersection produces large pencils). Numerical conditioning still degrades for nearly-tangent configurations where eigenvalues cluster.

**Kernel relevance.** Implicitization is the right tool when one operand is low-degree (planes, quadrics, tori, which dominate real mechanical parts) and exactness or a certified root count matters. Keel should keep an algebraic curve-curve and curve-surface path for the low-degree analytic surfaces and use the eigenvalue formulation rather than symbolic resultant expansion. For general high-degree NURBS, fall back to clipping or subdivision.

### 1.4 mu-bases and moving lines / moving surfaces (Cox, Sederberg, Chen)

**Citations.**
- Cox, D., Sederberg, T. W., and Chen, F. (1998). The moving line ideal basis of planar rational curves. Computer Aided Geometric Design, 15(8), 803-827.
- Sederberg, T. W., and Chen, F. (1995). Implicitization using moving curves and surfaces. Proc. SIGGRAPH 1995, 301-308.
- Chen, F., Cox, D., and Liu, Y. (2005). The mu-basis and implicitization of a rational parametric surface. Journal of Symbolic Computation, 39(6), 689-706.

**Algorithm.** A moving line for a planar rational curve (x(t):y(t):w(t)) in homogeneous form is a family of lines L(t): A(t) x + B(t) y + C(t) w = 0 that "follows" the curve, meaning the curve point lies on the line for every t. The set of such moving lines forms a module; its mu-basis is a special minimal generating pair {p, q} of polynomials, linear in the spatial variables, of degrees mu and d - mu in t (with mu the smaller). Two key facts make this powerful:

1. The implicit equation of the curve is the resultant Res_t(p, q), which is exactly degree d and base-point-free, far more compact than the Sylvester resultant of the original parametrization.
2. The curve is recovered as the intersection of its two moving lines: at a given t the two lines meet at the curve point.

For surfaces the analog is moving planes and moving quadrics; the mu-basis of a rational surface gives its implicit equation as a resultant of the basis elements with respect to the surface parameter. Computing a mu-basis reduces to Gaussian elimination on the coefficient matrix of the moving-line conditions, so it is cheap.

**Convergence and robustness.** Direct and exact. mu-bases give the lowest-degree, base-point-aware implicitization available, which improves the conditioning of any downstream resultant or eigenvalue solve. They also handle improper (non-injective) parametrizations gracefully when the theory is applied with multiplicity care.

**Failure modes.** Surface mu-basis theory is more intricate than the curve case; for general rational surfaces a clean two-element basis does not always exist and moving quadrics may be needed. Floating-point computation of the basis can lose the exact module structure.

**Kernel relevance.** mu-bases are the modern, robust route to implicitizing the analytic surfaces (cylinders, cones, tori, spheres, low-degree NURBS patches) that Keel will intersect most often. Storing a mu-basis or implicit form alongside each analytic surface lets Keel turn parametric-parametric SSI into the much easier parametric-implicit case (substitute one parametrization into the other's implicit form, get an algebraic curve in a 2D domain).

---

## 2. Curve-surface intersection (CSI)

**Citation.** Patrikalakis, N. M., Maekawa, T., and Cho, W. (2009). Shape Interrogation for Computer Aided Design and Manufacturing (hyperbook edition). Chapter 5.7, Curve/surface intersection. web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho.

**Algorithm.** A rational parametric curve C(t) intersecting a rational parametric surface S(u, v) is three nonlinear equations in three unknowns:

  X_S(u, v) - X_C(t) = 0,  Y_S(u, v) - Y_C(t) = 0,  Z_S(u, v) - Z_C(t) = 0.

The hyperbook lists three practical routes:

1. Implicitization. Implicitize S into f(x, y, z) = 0, substitute C(t) to get a single univariate f(C(t)) = 0, solve for t, then invert to recover (u, v). Cheapest when S is a low-degree analytic surface.
2. Bounding box plus subdivision plus minimization. Recursively subdivide curve and surface, prune pairs whose axis-aligned (or oriented) bounding boxes are disjoint, and on surviving small pairs minimize the squared distance ||S(u,v) - C(t)||^2 to a root. A preprocessing bounding-box overlap test cheaply rejects the common no-intersection case.
3. Interval Projected Polyhedron (IPP) solver. Treat the three equations as a polynomial system and feed it to the IPP global solver (Section 4), which returns certified boxes around all roots.

Once a box isolates a root, Newton on the 3-by-3 system polishes it quadratically. The Newton update solves J [du dv dt]^T = -(S - C) with J = [S_u, S_v, -C_t], a 3-by-3 Jacobian.

**Convergence and robustness.** Subdivision plus IPP is globally convergent and certified (finds every root within tolerance, no spurious ones). Newton polish is locally quadratic at transversal hits. Implicitization is direct but inherits resultant conditioning issues. Tangential curve-surface contact (the curve grazes the surface) makes the Jacobian rank-deficient and slows Newton to linear.

**Failure modes.** Curve lying in or near the surface (infinitely many or ill-conditioned intersections). Grazing tangency. High degree explodes the subdivision tree.

**Kernel relevance.** CSI is needed for imprinting wires onto faces, projecting edges, and trimming. Keel's CSI should preprocess with a fat bounding-volume hierarchy on both operands, use implicitization when the surface is analytic, otherwise IPP, and always finish with Newton polish plus a tangency check that flags ill-conditioned hits for special handling.

---

## 3. Surface-surface intersection (SSI)

SSI is the deepest and most failure-prone primitive. The literature organizes it into three method families (lattice, subdivision, marching), plus the algebraic and certified-hybrid approaches that now dominate. Three sub-problems recur for every method: find significant points (border crossings, turning points, singular points, loop witnesses), trace each branch, and sort/connect branches into the correct topology.

### 3.1 The three classical families (hyperbook Chapter 5.8 overview)

**Citation.** Patrikalakis, N. M., Maekawa, T., and Cho, W. (2009). Shape Interrogation for CAD/Manufacturing, Sections 5.8 to 5.8.2 (Case F1, rational parametric / rational parametric surfaces).

**Lattice methods.** Reduce SSI to a family of simpler problems by fixing iso-parameter lines: for each iso-curve u = u_i of surface A, intersect that curve with surface B (a CSI), collecting points, then connect them into intersection curves. Simple and parallelizable but the connection step is fragile and small loops between sampled iso-lines are easily missed.

**Subdivision methods.** Recursively subdivide both surfaces, prune patch pairs whose bounding volumes are disjoint, and when surviving pairs are small and nearly flat, intersect them as planes (or quadrics) and stitch the segments. Robust against missing branches if the pruning is conservative, but produces many short segments and can over-subdivide near tangency.

**Marching methods.** The fast default. Once a starting point on the intersection curve is known, step along the curve numerically. The intersection curve tangent at a transversal point is the cross product of the two surface normals,

  T = n_A x n_B,  where n_A = S_A,u x S_A,v and n_B = S_B,u x S_B,v.

Marching advances by an ODE/predictor-corrector: predict along T by step h (Euler or higher-order), then correct back onto the intersection by Newton on the 4 equations S_A(u_A, v_A) = S_B(u_B, v_B) in the 4 unknowns (u_A, v_A, u_B, v_B), which is one degree of freedom (a curve). The corrector adds a hyperplane constraint perpendicular to T to make the system square.

**Convergence and robustness.** Marching is the fastest per point and produces smooth, well-parametrized curves, but it has three classic failure modes that the rest of this section addresses: starting-point determination, step-size control (component jumping and straying), and loop detection (closed branches with no boundary point are invisible to a pure border-march).

### 3.2 Step-size control, straying, looping

Step size must balance speed against two errors. Too large a step and the corrector converges to a different branch (component jumping) or wanders off (straying); too small and the march is slow. Adaptive schemes set h from the local curvature of the intersection curve (kappa estimated from the two surfaces' second fundamental forms) so that the predictor error stays below a fraction of the corrector's basin of attraction. Most production implementations historically used very conservative fixed or curvature-bounded steps, accepting slowness for safety. The deficiency of all heuristic step control is that no finite step guarantees you stayed on the same branch; this is precisely what validated marching (Section 3.6) fixes.

### 3.3 Loop detection: collinear-normal and Gauss-map criteria

**Citations.**
- Sederberg, T. W., and Meyers, R. J. (1988). Loop detection in surface patch intersections. Computer Aided Geometric Design, 5(2), 161-171.
- Sederberg, T. W., et al. (1989). Surface intersection algorithm based on loop detection (collinear normal). Proc. Symposium on Solid Modeling; Int. J. Comput. Geom. Appl. 1(3), 1991.
- Hohmeyer, M. E. (1991). A surface intersection algorithm based on loop detection. Proc. ACM Symposium on Solid Modeling, 197-207; and Robust and Efficient Surface Intersection for Solid Modeling, UC Berkeley TR UCB/CSD-92-681 (1992).

**The problem.** A border-walking marcher finds every branch that touches a patch boundary, because it starts marches at all boundary crossings. A closed loop entirely interior to both patches has no boundary point, so it is silently missed. A correct kernel must prove either that no loop exists in a patch pair (then border-walking is complete) or locate a point on each loop to seed a march.

**Collinear-normal criterion (Sederberg and Meyers 1988).** If two surfaces intersect in a closed loop, then there exists a line normal to both surfaces simultaneously (a collinear-normal point pair). Intuitively, going around a loop the intersection-curve tangent T = n_A x n_B must reverse, which forces n_A parallel to n_B somewhere. So: compute all collinear-normal point pairs (solve n_A x n_B = 0 with the surface-coincidence constraints), and if there are none, there is no loop. Collinear-normal points also serve as turning points that split the parameter domain so each sub-domain has monotone marching.

**Gauss-map / bounding-pyramid criterion (Hohmeyer 1991/1992).** A cleaner, sufficient no-loop test. Bound the Gauss map (the set of unit normals) of each patch by a convex region on the sphere, approximated by a convex bounding pyramid or cone of normals (Daniel and Daniel 1996 give the tight convex-pyramid construction). If the two normal cones can be strictly separated, equivalently if there is a vector that has positive dot product with all normals of A and all normals of B (tested by a small linear program), then n_A and n_B are never antiparallel/parallel in a way that permits a loop, and additionally the intersection projects injectively onto a plane (it is a function graph over that plane's coordinates), so there can be no loop and the march cannot turn back. Hohmeyer combines this with subdivision: subdivide until each patch pair passes the separability LP, then border-walk each pair safely.

**Convergence and robustness.** These criteria convert "did I miss a loop?" from a hope into a theorem. The Gauss-map test is conservative (it may subdivide more than strictly necessary) but never wrong. The collinear-normal approach needs a global polynomial solve (use IPP or eigenvalue resultants), which is the expensive part.

**Failure modes.** Tangential intersections make normals collinear along an entire curve, so the collinear-normal solve becomes degenerate and the Gauss-map cones touch rather than separate, defeating the LP. These cases require the algebraic/topological treatment of Sections 3.5 and 3.7.

**Kernel relevance.** Loop detection is non-negotiable for a B-rep kernel. Keel should adopt the Hohmeyer subdivide-until-separable strategy as the structural backbone of SSI (it gives both loop-freedom and a guaranteed-injective march per leaf), and use collinear-normal points to seed loops in the residual patches that cannot be separated.

### 3.4 Marching on surface boundaries; significant-point ordering (Bajaj et al. 1988; Barnhill and Kersey 1990)

**Citations.**
- Bajaj, C. L., Hoffmann, C. M., Lynch, R. E., and Hopcroft, J. E. (1988). Tracing surface intersections. Computer Aided Geometric Design, 5(4), 285-307.
- Barnhill, R. E., and Kersey, S. N. (1990). A marching method for parametric surface/surface intersection. Computer Aided Geometric Design, 7(1-4), 257-280.

**Algorithm.** Bajaj et al. give the foundational tracing framework for algebraic surfaces: characterize significant points (singular points, turning points where the curve is tangent to an iso-direction, and boundary points) using the implicit algebraic structure, then trace monotone arcs between consecutive significant points with a predictor-corrector and connect them. The singular-point analysis uses the Taylor expansion of the implicit curve: at a point where the gradient vanishes, the quadratic form (the Hessian) classifies the local geometry into transversal self-intersection (two real tangent directions), cusp (one double direction), or isolated point (complex directions). Barnhill and Kersey formalize the parametric-surface marching version: compute the tangent T = n_A x n_B, take an Euler or Runge-Kutta predictor step, correct with Newton onto the 4-equation system, adapt the step to curvature, and stop at boundaries. They emphasize starting-point determination by intersecting each patch's four boundary curves with the other surface (a CSI) to seed all boundary-touching branches.

**Convergence and robustness.** Predictor-corrector marching converges quadratically per corrector step; the global correctness depends entirely on having all significant points and a step small enough to avoid jumping, which these papers handle heuristically. Bajaj's significant-point machinery is what makes the trace provably complete for algebraic curves (every branch is bounded by significant points).

**Failure modes.** Same trio: missed loops (Bajaj addresses via singular points but not interior loops without singularities), component jumping, and tangential degeneracy where the Hessian classification is ill-conditioned.

**Kernel relevance.** This is the algorithmic skeleton every kernel's marcher descends from. Keel's tracer should follow Barnhill-Kersey's predictor-corrector with Bajaj-style significant-point boundaries, layered under the certified controls of Sections 3.6 and 3.7.

### 3.5 Lower-dimensional / algebraic SSI (Krishnan and Manocha 1997)

**Citation.** Krishnan, S., and Manocha, D. (1997). An efficient surface intersection algorithm based on lower-dimensional formulation. ACM Transactions on Graphics, 16(1), 74-106.

**Algorithm.** Instead of tracing a curve in the 4D parameter space (u_A, v_A, u_B, v_B), reformulate SSI as an algebraic plane curve in the 2D parameter domain of one surface. Substitute the parametrization of surface B into a matrix representation derived from surface A (via resultants/implicitization expressed as a matrix whose determinant vanishes on A), producing a single bivariate algebraic equation F(u_B, v_B) = 0 whose zero set is the preimage of the intersection on B. The matrix representation lets singular points and topology be analyzed with eigenvalue/numerical-linear-algebra tools rather than symbolic determinant expansion: at each (u_B, v_B), F is the determinant of a matrix M(u_B, v_B), and points where M drops rank are singular/critical points. The algebraic plane curve is then traced in 2D with guaranteed topology (the loop and singularity structure is read off from the matrix), and lifted back to 3D.

**Convergence and robustness.** Far more robust than 4D marching because the topology (loops, singular points, branch count) is determined algebraically up front in 2D, removing the loop-detection and component-jumping uncertainties. Numerical linear algebra (SVD/eigenvalues) on the matrix representation is backward stable.

**Failure modes.** Building the matrix representation requires implicitizing one surface, so it is most practical when that surface is low-to-moderate degree; high-degree NURBS produce large matrices. Base points and improper parametrizations need care.

**Kernel relevance.** The lower-dimensional idea is the bridge between the numeric and algebraic cultures and is essentially the ancestor of the 2023 TOG approach. Keel should use it for the analytic and low-degree cases where the matrix stays small, getting certified 2D topology nearly for free.

### 3.6 Validated interval-ODE marching (Mukundan, Maekawa, Patrikalakis 2004)

**Citation.** Mukundan, H., Ko, K. H., Maekawa, T., Sakkalis, T., and Patrikalakis, N. M. (2004). Tracing surface intersections with a validated ODE system solver. Proc. 9th ACM Symposium on Solid Modeling and Applications, 249-254. (See also Surface to surface intersections, Computer-Aided Design and Applications 1(1-4), 2004, 449-457.)

**Algorithm.** Express the intersection curve as the solution of an ODE initial value problem in the 4 parameters. Differentiating the coincidence constraints S_A(u_A, v_A) = S_B(u_B, v_B) along arc length gives a linear system for the parameter velocities; symbolically the parameter-space tangent components come from projecting the 3D tangent T = n_A x n_B onto each surface's tangent basis:

  [du_A/ds, dv_A/ds] from solving S_A,u du_A + S_A,v dv_A = T (in the tangent plane of A), similarly for B.

This is an ODE dy/ds = g(y) with y = (u_A, v_A, u_B, v_B). The key move is to solve it with a validated (interval) ODE solver using rounded interval arithmetic: instead of producing a point, each step produces an interval box guaranteed to contain the true curve, with a numerically verified upper bound on the error in parameter space, which is then mapped to a guaranteed 3D error bound. Because every step's enclosure is verified, the solver cannot stray off the curve or loop onto a wrong branch: straying and looping are provably eliminated. It handles both transversal and tangential intersections (tangential needs higher-order interval Taylor terms because the first-order tangent degenerates).

**Convergence and robustness.** Produces a continuous, gap-free, error-bounded enclosure of the intersection branch. This is the certified upgrade of heuristic marching: the output comes with a rigorous tolerance, exactly what a kernel needs to build watertight topology.

**Failure modes.** Interval arithmetic is slower than floating point (boxes, not points) and can over-conservatively shrink steps near tangency, where the enclosures balloon. It still needs an external mechanism (loop detection, Section 3.3) to find a starting point on each branch; it certifies the trace, not the branch enumeration.

**Kernel relevance.** This is the gold standard for the tracing sub-problem in Keel. Pair it with Hohmeyer loop detection for branch enumeration and you get a marcher that is both complete (all branches found) and certified (each branch enclosed to tolerance). The error bound directly feeds Keel's tolerance bookkeeping.

### 3.7 Topology-guaranteed B-spline SSI (Yang, Jia, Yan 2023) and topology-driven interval analysis (Jia et al. 2023)

**Citations.**
- Yang, J., Jia, X., and Yan, D.-M. (2023). Topology guaranteed B-spline surface/surface intersection. ACM Transactions on Graphics, 42(6), Article 211. DOI 10.1145/3618349.
- (Related) Topology driven approximation to rational surface-surface intersection via interval algebraic topology analysis. ACM Transactions on Graphics, 42(4), 2023. DOI 10.1145/3592452.

**Algorithm.** The 2023 TOG method is the current reference for certified SSI. Its goal is to guarantee the correct intersection topology even in near-critical relative positions where ACIS and OCCT (the commercial benchmarks) sometimes fail. The strategy:

1. Subdivide both B-spline surfaces (into Bezier patches) and prune disjoint pairs by bounding volumes.
2. For each surviving patch pair, apply algebraic-topology tests on bounded quantities (positions, tangent/normal cones, and the sign structure of the intersection equations) to certify how many intersection branches cross the pair and how they connect, distinguishing the topology types: transversal crossings, isolated contacts (point tangencies), intersections running along a boundary, contact spanning multiple branches, high-order contact along an entire curve, and multiple isolated contacts.
3. Where a pair cannot yet be certified to a single simple topology, subdivide further; the process terminates because near-critical regions shrink until the bounds resolve the ambiguity.
4. Connect the certified per-patch pieces into global intersection curves with the proven topology, then approximate each branch within an error bound.

The companion 2023 paper (Jia et al., DOI 10.1145/3592452) does the rational-surface case via interval algebraic topology analysis: it computes the topology of the intersection in a parameter domain using interval arithmetic to certify critical-point counts and branch connectivity before any tracing, the same philosophy as Krishnan-Manocha but with interval certification.

**Convergence and robustness.** Demonstrated correct topology across all the hard cases (cross intersections, isolated contacts, boundary-coincident branches, high-order tangential contact, multiple contacts) where the commercial kernels produce wrong branch counts or missed loops. This is the strongest published robustness result for SSI.

**Failure modes.** Cost: heavy subdivision plus interval/algebraic tests in near-critical regions is slower than a plain marcher in the generic transversal case. It is a B-spline method; integration with analytic surfaces and the full trimmed-NURBS data model is engineering work.

**Kernel relevance.** This is the architecture Keel's SSI should target for correctness-critical Booleans. Use it (or its interval-algebraic-topology sibling) to certify branch count and connectivity, then hand each certified branch to the validated interval-ODE marcher (Section 3.6) for an error-bounded sampled curve. Generic transversal pairs can shortcut to fast heuristic marching, falling back to the certified path only in near-critical regions detected by the Gauss-map separability test.

### 3.8 Error-controlled, topology-guaranteed tracing (2025)

**Citation.** Topology guaranteed and error controlled curve tracing for parametric surface-surface intersection. Computer Aided Geometric Design, 2025 (ScienceDirect S0167839625000214).

**Algorithm.** Decomposes the 4D intersection curve in parameter space into strongly monotonic segments such that the corresponding 3D segments are also strongly monotonic. Strong monotonicity per segment guarantees that within a segment the march cannot turn back or jump (no straying, no looping, correct 3D topology between segments). By controlling the decomposition density in model space, the approximation error of each segment is bounded directly. This unifies the topology guarantee (from the monotone decomposition, which is the modern restatement of significant-point splitting at turning/collinear-normal points) with the error guarantee (from density control, the modern restatement of validated enclosure) in a single tracing pass.

**Convergence and robustness.** Provides both correct topology and a prescribed error bound at lower cost than full interval-ODE enclosure, because monotone segments admit ordinary (fast) predictor-corrector marching with a posteriori error control rather than interval boxes at every step.

**Kernel relevance.** A pragmatic middle path for Keel: cheaper than full validated-ODE marching, still topology-safe. A strong candidate for Keel's default tracer once branch enumeration is certified.

---

## 4. The Projected Polyhedron family (global polynomial solving underpinning everything)

### 4.1 Projected Polyhedron (Sherbrooke and Patrikalakis 1993)

**Citations.**
- Sherbrooke, E. C., and Patrikalakis, N. M. (1993). Computation of the solutions of nonlinear polynomial systems. Computer Aided Geometric Design, 10(5), 379-405.
- Patrikalakis, N. M., Maekawa, T., and Cho, W. (2009). Shape Interrogation for CAD/Manufacturing, Section 4.4.

**Algorithm.** A global, iterative root finder for a system of m polynomials p_1, ..., p_m in n variables sought in a box B = [a_1, b_1] x ... x [a_n, b_n]. Steps:

1. Affine-transform each [a_i, b_i] to [0, 1] so the system lives on the unit hypercube.
2. Convert each p_j to multivariate Bernstein form (better numerical stability under perturbation than the power basis and the convex-hull property holds).
3. For each p_j build the graph hypersurface x_{n+1} = p_j(x); roots are where all graphs cross x_{n+1} = 0.
4. Project the control points of all p_j onto n coordinate planes (the x_1-x_{n+1} plane, ..., x_n-x_{n+1} plane). In each 2D plane, take the convex hull of each projected control-point set and intersect it with the horizontal axis to get a parameter interval (or empty). Intersect across all m polynomials to get one interval per coordinate. Their Cartesian product is a sub-box guaranteed to contain all roots.
5. If a sub-box dimension is not sufficiently smaller than 1 (poor contraction), subdivide (de Casteljau) along the offending dimensions and recurse on each piece, tracking the map back to the original box.
6. Terminate when the sub-box is below tolerance; report it as a root box (optionally confirm via the convex hulls crossing all axes).

**Convergence and robustness.** Quadratic convergence in one dimension; at best linear in higher dimensions. Per-iteration cost is O(product of d_i). It needs only two primitives (multivariate Bernstein subdivision and 2D convex hull), so it is simple and very robust: every step is a conservative convex-hull exclusion, so no root is ever lost. Handles overconstrained systems (m > n) well; for underconstrained systems (m < n, the SSI/marching case) it is slow and parametric tracing is preferred.

**Failure modes.** Curse of dimensionality: cost grows with n and with degree. Clustered/tangential roots subdivide heavily. Floating-point hull construction needs care for robustness.

**Kernel relevance.** PP is the global solver behind CSI, point projection, collinear-normal solving, and starting-point determination. Keel should implement multivariate Bernstein subdivision and a robust 2D hull once and reuse them throughout.

### 4.2 Interval Projected Polyhedron (IPP)

**Citation.** Patrikalakis, N. M., Maekawa, T., and Cho, W. (2009). Shape Interrogation for CAD/Manufacturing, Section 4.x (IPP) and 5.6.5.3.

**Algorithm.** Same structure as PP but the Bernstein coefficients are intervals computed with rounded interval arithmetic, so the projected control points become small boxes and the convex hulls are hulls of boxes. The resulting root boxes are then certified to contain the true roots despite floating-point error: IPP delivers verified enclosures, not just approximations.

**Convergence and robustness.** Inherits PP's convergence with a rigorous, floating-point-proof guarantee. This is the certified solver to use when correctness (not just accuracy) is required, for example seeding SSI starting points or guaranteeing a root count.

**Kernel relevance.** IPP should be Keel's certified backend for any polynomial system where a missed or spurious root would corrupt topology (significant points, collinear normals, transversality witnesses).

### 4.3 Subdivision solvers and complexity (Mourrain and Pavone 2009)

**Citation.** Mourrain, B., and Pavone, J.-P. (2009). Subdivision methods for solving polynomial equations. Journal of Symbolic Computation, 44(3), 292-306.

**Algorithm.** Generalizes Bernstein-subdivision solving with sharper exclusion (preconditioning by a partial linear solve, Descartes-rule sign tests on Bernstein coefficients, and reduction operators that contract domains faster than naive bisection). Provides complexity bounds and a univariate-to-multivariate framework that subsumes Bezier clipping, quadratic clipping, and PP as instances.

**Convergence and robustness.** Faster contraction than plain PP via preconditioning; rigorous when paired with interval arithmetic. Establishes the theoretical backbone (sign-variation/Descartes tests on Bernstein coefficients) that all clipping methods rely on.

**Kernel relevance.** The conceptual umbrella for Keel's 1D and nD solvers. Adopt its preconditioning and reduction tricks to speed up PP/IPP without sacrificing certification.

---

## 5. Point projection and inversion (foot-point computation)

The projection problem: given a test point R, find the parameter(s) where a curve C(t) or surface S(u, v) is closest to R, equivalently where (C(t) - R) is orthogonal to C'(t), i.e. f(t) = (C(t) - R) . C'(t) = 0. Inversion is the special case where R is known to lie on the entity.

### 5.1 Newton formulation

**Algorithm.** For a curve, Newton on f(t) = (C(t) - R) . C'(t):

  t_{k+1} = t_k - [(C - R) . C'] / [C' . C' + (C - R) . C''].

For a surface, solve the 2-by-2 system f_1 = (S - R) . S_u = 0, f_2 = (S - R) . S_v = 0 with the Jacobian containing S_uu, S_uv, S_vv terms. Newton is quadratically convergent near a simple foot point but diverges or jumps to a wrong stationary point from a poor initial guess, and the closest-point problem can have several local minima.

### 5.2 Second-order geometric iteration (Hu and Wallner 2005)

**Citation.** Hu, S.-M., and Wallner, J. (2005). A second order algorithm for orthogonal projection onto curves and surfaces. Computer Aided Geometric Design, 22(3), 251-260.

**Algorithm.** A geometric (parametrization-independent) iteration that uses only intrinsic data (the point, tangent, and curvature/osculating circle), so it behaves the same under any reparametrization. The idea per step: project R orthogonally onto the local geometric model of the curve (its osculating circle, which captures second-order shape) rather than onto the tangent line (first-order). Concretely, foot-point on the tangent gives the first-order estimate; correcting with the curvature radius yields the next parameter. For surfaces the analog uses the normal curvature in the relevant direction.

**Convergence and robustness.** Second-order convergence for projection that is essentially independent of the initial value (a major advantage over Newton, which needs a good seed), and third-order for the inversion problem. Low sensitivity to starting guess makes it far more reliable than raw Newton from arbitrary seeds.

**Failure modes.** Still local: with multiple candidate foot points it converges to one basin and needs a global stage (subdivision/exclusion, Section 5.4) to enumerate all candidates. Degenerates at inflection points where curvature vanishes.

**Kernel relevance.** This should be Keel's local polisher for projection, replacing bare Newton, because its initial-value robustness reduces the number of seeds the global stage must supply.

### 5.3 Control-polygon (clipping) approach (Ma and Hewitt 2003)

**Citation.** Ma, Y. L., and Hewitt, W. T. (2003). Point inversion and projection for NURBS curve and surface: control polygon approach. Computer Aided Geometric Design, 20(2), 79-99.

**Algorithm.** Decompose the NURBS into Bezier sub-curves/patches and use the control polygon to bound where the closest point can lie: project R onto the control polygon, identify the nearest control points/edges, and discard sub-segments whose control polygon is provably farther than the current best candidate. Surviving segments are refined by subdivision and finished with Newton. The control polygon both supplies a good initial guess and prunes the search globally.

**Convergence and robustness.** Global (finds the true closest point, not just a local one) with the speed of clipping. Robust to multiple candidates because pruning is conservative.

**Kernel relevance.** The pruning idea is the basis for Keel's global projection stage; combine its control-polygon bounds with the exclusion criteria below and Hu-Wallner polish.

### 5.4 Subdivision-exclusion criteria (Selimovic 2006; Chen et al. circular clipping; torus/sphere clipping)

**Citations.**
- Selimovic, I. (2006). Improved algorithms for the projection of points on NURBS curves and surfaces. Computer Aided Geometric Design, 23(5), 439-445.
- Chen, X.-D., Yong, J.-H., Wang, G., Paul, J.-C., and Xu, G. (2008). Computing the minimum distance between a point and a NURBS curve. Computer-Aided Design, 40(10-11), 1051-1054 (circular/elimination clipping).
- Chen, X.-D., et al. (2009-2010). Efficient point projection to freeform curves and surfaces (tangent-cone / circular-cone exclusion).

**Algorithm.** Selimovic gives two exclusion criteria that prune a Bezier sub-patch when it cannot contain the closest point:

1. Orthogonality/angle criterion: if for every control point P_i of the sub-patch the vector (P_i - R) makes an angle with the patch's tangent/normal cone that excludes the possibility of orthogonality, the patch cannot contain a foot point and is discarded.
2. Distance criterion: if the entire control polygon of the sub-patch is farther from R than the closest endpoint (corner) distance already achieved, it cannot contain the global minimum and is discarded.

Chen et al.'s circular clipping bounds the curve by a circle (or, for surfaces, the foot-point locus by a tangent cone) centered such that the no-orthogonal-projection region is excluded directly: the test reduces to whether R lies inside/outside a clipping circle derived from the curve's control points, eliminating sub-curves in one cheap geometric test. More recent torus/sphere clipping methods bound a surface patch by an enclosing torus or sphere and exclude the patch when R's distance to that primitive precludes a closer foot point, giving tighter pruning than planar/circular tests for curved patches.

**Convergence and robustness.** These criteria make subdivision-based projection both global (no foot point missed) and fast (most of the domain pruned early). Combined with a Newton or Hu-Wallner polish on survivors, they give certified-global closest-point computation. Selimovic's criteria are simple to evaluate (dot products and distances on control points).

**Failure modes.** Conservative cones can under-prune near high curvature; very flat patches relative to R can leave many survivors. Tangential/equidistant configurations (R on a symmetry axis) produce many tied foot points.

**Kernel relevance.** This is the recommended global stage for Keel's point projection: subdivide to Bezier, prune with Selimovic plus circular/torus/sphere clipping, then polish survivors with Hu-Wallner, keep the global minimum. It is the same exclusion philosophy as Bezier clipping, reused for distance.

---

## 6. Ray-NURBS intersection

### 6.1 Toth interval Newton (1985)

**Citation.** Toth, D. L. (1985). On ray tracing parametric surfaces. Computer Graphics (Proc. SIGGRAPH 1985), 19(3), 171-179.

**Algorithm.** Solve the ray-surface intersection directly by multivariate Newton on the 3 equations ray(s) = S(u, v) (3 equations, 3 unknowns s, u, v). The classic obstacle is finding a starting region where Newton converges. Toth uses interval analysis: apply the interval Newton / Krawczyk operator to a parameter box B. If the Krawczyk operator K(B) is contained in B, a unique root exists in B and Newton converges to it; if K(B) and B are disjoint, no root exists and B is discarded; otherwise subdivide B and recurse. This both isolates the convergence region and supplies a good initial guess.

**Convergence and robustness.** Provides existence and uniqueness guarantees per box (a rare property in 1985) and guaranteed Newton convergence inside verified boxes. Finds all ray hits, not just the nearest.

**Failure modes.** Interval boxes are conservative and the subdivision can be slow for grazing rays or silhouettes (near-tangent hits where the Jacobian is near-singular and boxes refuse to contract).

**Kernel relevance.** The interval-Newton existence test is reusable wherever Keel needs certified isolation of a Newton root (ray casting for selection/visibility, certified projection). The Krawczyk test is a building block, not just a ray-tracing trick.

### 6.2 Bezier clipping for ray tracing (Nishita, Sederberg, Kakimoto 1990)

**Citation.** Nishita, T., Sederberg, T. W., and Kakimoto, M. (1990). Ray tracing trimmed rational surface patches. Computer Graphics (Proc. SIGGRAPH 1990), 24(4), 337-345.

**Algorithm.** Reduce the 3D ray-patch problem to 2D by writing the ray as the intersection of two non-parallel planes P_1 . X + d_1 = 0 and P_2 . X + d_2 = 0 (choose P_1, P_2 perpendicular to the ray direction and to each other). Substituting the rational patch S(u, v) into both planes gives two scalar equations in (u, v):

  f(u, v) = P_1 . S(u, v) + d_1 = 0,  g(u, v) = P_2 . S(u, v) + d_2 = 0,

which discards the depth-along-ray coordinate. Each equation's left side is a rational Bezier surface in (u, v); apply Bezier clipping in 2D (fat lines / fat planes in the (u, v) domain) to iteratively clip away regions where f or g cannot vanish, isolating all (u, v) roots to a user precision, then recover depth and shade. Trimming is handled by testing each candidate (u, v) against the trim curves, where ray-trim-curve tests themselves use iterative Bezier clipping.

**Convergence and robustness.** Quadratic at transversal hits (inherited from clipping), finds all hits, and the convex-hull exclusion makes it robust. The two-plane reduction is exact and numerically clean.

**Failure modes.** Silhouette/grazing hits (tangential) slow to linear; very thin or high-curvature patches need deep clipping. Trimmed boundaries add cost.

**Kernel relevance.** The two-plane reduction is the canonical way to turn any ray-surface query into a 2D root problem, directly reusable in Keel for picking, ray-based containment tests, and visualization. Pair with cubic clipping (Section 1.2) for faster convergence than the original Bezier clipping.

### 6.3 Robust/stable Bezier clipping and modern GPU ray tracing of trimmed NURBS

**Citations.**
- Efremov, A., Havran, V., and Seidel, H.-P. (2005). Robust and numerically stable Bezier clipping method for ray tracing NURBS surfaces. Proc. SCCG.
- Pabst, H.-F., Springer, J. P., Schollmeyer, A., Lenhardt, R., Lessig, C., and Froehlich, B. (2006/2010). Ray casting of trimmed NURBS surfaces on the GPU. Proc. IEEE Symposium on Interactive Ray Tracing.
- Subsequent OptiX/RTX-era work (NVIDIA, KIT Karlsruhe) on direct GPU ray tracing of trimmed NURBS and Bezier patches.

**Algorithm.** The robust Bezier clipping variant fixes numerical instabilities in the original (degenerate fat lines, near-zero pivots) and adds an iterative Newton refinement accelerated by polynomial extrapolation once the clip has localized a root. GPU approaches partition each trimmed NURBS into rational bi-cubic Bezier patches, build a BVH over patches, and on the GPU per ray do the two-plane reduction plus Newton iteration (with interval or clipping fallback for starting points), then test trim curves with iterative Bezier clipping; T-spline and arbitrary-degree support follows the same recipe with on-the-fly degree handling. Modern RTX hardware accelerates the BVH traversal while the patch intersection runs in an intersection shader.

**Convergence and robustness.** Newton with extrapolation converges fast once seeded; the BVH plus per-patch clipping is the standard high-performance pipeline. Robust-clipping variants survive the degenerate fat-line cases that broke the 1990 formulation.

**Kernel relevance.** Keel will want GPU-accelerated ray casting for interactive picking, section views, and visibility on large trimmed NURBS assemblies. The patch-BVH plus two-plane-plus-Newton pipeline is the proven design; the kernel's surface evaluator and trim model should expose Bezier extraction to feed it.

---

## 7. Self-intersection of curves and surfaces (offset ties)

**Citations.**
- Patrikalakis, N. M., Maekawa, T., and Cho, W. (2009). Shape Interrogation for CAD/Manufacturing, Chapter 11 (Offset curves and surfaces), Sections 11.3.x self-intersection of offsets.
- Pekerman, D., Elber, G., and Kim, M.-S. (2008). Self-intersection detection and elimination in freeform curves and surfaces. Computer-Aided Design, 40(2), 150-159.

**Algorithm.** A curve self-intersects where C(t_1) = C(t_2) with t_1 != t_2; cast as a polynomial system and solved with subdivision/PP, taking care to exclude the trivial diagonal t_1 = t_2 (factor it out, e.g. by dividing the difference C(t_1) - C(t_2) by (t_1 - t_2) using the symmetric/antisymmetric structure, leaving a system whose roots are genuine self-intersections). Surface self-intersection S(u_1, v_1) = S(u_2, v_2) is the 4D analog with the diagonal excluded similarly. Offsets are the practical driver: the offset of a curve/surface self-intersects exactly where the offset distance exceeds the local radius of curvature, so the self-intersection parameters are tied to curvature extrema and to the collinear-normal condition between the progenitor and its offset. For polynomial offset approximations the system becomes trivariate equations involving square roots of polynomials (from the unit-normal denominator), handled by squaring or auxiliary variables.

**Convergence and robustness.** Subdivision/PP gives global enumeration; the diagonal-exclusion is the subtle part (a naive solve returns the entire diagonal as spurious roots). Pekerman et al. give a clean elimination scheme and connect self-intersection loops to trimming.

**Kernel relevance.** Offsetting, shelling, and thickening are core kernel operations and all require detecting and trimming offset self-intersections to stay watertight. Keel needs a robust self-intersection solver with diagonal exclusion as a first-class primitive, sharing the PP/IPP backend and the collinear-normal machinery from SSI.

---

## 8. Distance and Hausdorff computation between curves/surfaces

**Citations.**
- Chen, X.-D., Chen, L., Wang, Y., Xu, G., Yong, J.-H., and Paul, J.-C. (2010). Computing the Hausdorff distance between two B-spline curves. Computer-Aided Design, 42(12), 1197-1206.
- Kim, Y.-J., Oh, Y.-T., Yoon, S.-H., Kim, M.-S., and Elber, G. (2013). Precise Hausdorff distance computation for planar freeform curves using biarcs and depth buffer. The Visual Computer / earlier polyline-approach variants.
- Krishnamurthy, A., McMains, S., et al. (2011). Computing the Hausdorff distance between NURBS surfaces using numerical iteration on the GPU. Computers and Graphics.

**Algorithm.** Minimum distance between two curves/surfaces is a stationary-point problem: at the closest pair, the connecting segment is orthogonal to both entities, giving a polynomial system solved by subdivision plus exclusion (the same pruning as point projection, generalized to two parametric domains). The Hausdorff distance h(A, B) = max over a in A of min over b in B of ||a - b|| is harder because of the nested max-min. The standard reduction: the directed Hausdorff distance is the maximum over A of the point-to-curve minimum distance function, so it is a one-sided projection composed with a maximization. Algorithms subdivide A, and for each piece bound the point-to-B distance using control-polygon and elimination criteria, pruning pieces that cannot achieve the running maximum and pieces of B that cannot be the nearest. Candidate maximizers occur at: endpoints, points where the foot-point on B is at a B-endpoint, and points where the distance function is stationary; these are enumerated and the largest taken. Upper-bound theorems on the Hausdorff distance between two Bezier curves (used for curve fitting/reduction within an error threshold) come from bounding the difference of control polygons. GPU methods sample one surface densely and compute per-sample nearest distance to the other via numerical iteration, then take the max with a refinement pass.

**Convergence and robustness.** Subdivision-plus-exclusion gives certified bounds on min distance and (with care on the candidate enumeration) on Hausdorff distance. The error bound is what makes these usable for tolerance-driven simplification.

**Failure modes.** Hausdorff is sensitive to missing a sharp local maximum; conservative bounding is essential. Equidistant regions create plateaus of maximizers.

**Kernel relevance.** Minimum distance feeds clearance/interference checks, fitting, and snapping. Hausdorff distance is the right metric for certifying that an approximated intersection curve, a simplified surface, or a tessellation stays within a stated tolerance of the exact geometry, which Keel needs to keep its tolerant B-rep provably within band.

---

## 9. Tolerance and robustness theory specific to intersection

**Citation.** Patrikalakis, N. M., and Maekawa, T. (2002 print; 2009 hyperbook). Shape Interrogation for Computer Aided Design and Manufacturing. Springer. Chapters 4 (solvers), 5 (intersections), 6 (differential geometry of intersection curves), 7 (distance functions), 11 (offsets).

**Content.** The Patrikalakis-Maekawa book and its free hyperbook are the single most complete robustness reference for kernel intersection. Key transferable lessons:

- Bernstein basis everywhere. Convert to Bernstein/Bezier form before any subdivision or clipping because of its superior conditioning under perturbation and its convex-hull property; the power basis is numerically unsafe at the degrees CAD uses.
- Rounded interval arithmetic to convert approximate algorithms into certified ones (IPP, validated ODE marching). The kernel should carry verified error bounds, not just floating-point estimates, on every intersection result.
- Differential geometry of the intersection curve (Chapter 6): the curvature and torsion of an SSI curve are computed from the two surfaces' first and second fundamental forms; these drive step-size control and tangential-case detection (tangential intersection is exactly where the two surfaces share a tangent plane, det of the relevant form vanishes).
- Classification of solution dimensionality up front (empty, points, curve with several branches, a patch, isolated point) so the kernel dispatches to the right algorithm and never mis-handles a coincident region as a transversal crossing.
- Significant points (boundary crossings, turning/collinear-normal points, singular points) as the scaffold that makes tracing complete.

**Kernel relevance.** Keel should treat this book as its intersection specification: Bernstein-basis solvers, interval-certified results, fundamental-form-driven step control, and explicit solution-type classification before tracing.

---

## 10. Intersection architecture for Keel

Synthesizing the above into a concrete design.

**Shared numerical backbone.** Implement once and reuse: (a) multivariate Bernstein/Bezier conversion and de Casteljau subdivision; (b) a robust 2D convex-hull routine; (c) rounded interval arithmetic; (d) PP and its certified sibling IPP as the global polynomial solver (Mourrain-Pavone preconditioning for speed); (e) interval Newton / Krawczyk for existence-and-uniqueness isolation; (f) ordinary Newton plus Hu-Wallner second-order geometric iteration as local polishers. Every higher operation is built from these.

**Curve-curve intersection.** Convert to Bezier; first pass with Bezier clipping (perpendicular plus parallel fat lines), upgrade to cubic hybrid clipping for the kernel's tight tolerances; subdivide on stall to separate multiple roots; coincidence test for overlaps. For low-degree analytic curves, an algebraic path via mu-basis implicitization and Manocha-Canny eigenvalue root finding gives a certified root count.

**Curve-surface intersection.** Bounding-volume prune; implicitize the surface when analytic (substitute, solve univariate), otherwise IPP on the 3-equation system; Newton polish with a tangency guard.

**Surface-surface intersection (the centerpiece).** A three-tier strategy by difficulty:
1. Branch enumeration with a topology guarantee. Subdivide both surfaces to Bezier and apply Hohmeyer Gauss-map separability (the normal-cone LP) to split into leaf pairs that are provably loop-free and inject-onto-a-plane. For residual near-critical pairs that will not separate, escalate to the certified algebraic-topology test (Yang-Jia-Yan 2023 / interval algebraic topology 2023) or the lower-dimensional matrix formulation (Krishnan-Manocha) to certify branch count, loops, singular points, and connectivity. Use collinear-normal points to seed any interior loop.
2. Tracing each certified branch. For generic transversal branches, fast predictor-corrector marching with curvature-based step control and 2025-style strongly-monotone segmentation for a built-in topology and error guarantee. For near-critical or tangential branches, the validated interval-ODE marcher (Mukundan-Patrikalakis) for a gap-free, error-bounded enclosure.
3. Curve approximation. Fit each traced branch to a NURBS curve within a Hausdorff-distance tolerance (Section 8), so the stored intersection edge is provably within the kernel's tolerance band of the exact curve.

This layering buys the generic-case speed of marching while never losing the topology guarantee that distinguishes a serious kernel; it is essentially the architecture the 2023-2025 papers point to, assembled from parts.

**Point projection and inversion.** Global stage: subdivide to Bezier, prune with Selimovic exclusion plus circular/torus/sphere clipping (and Ma-Hewitt control-polygon bounds), then polish survivors with Hu-Wallner second-order geometric iteration; keep the global minimum. Robust to bad initial guesses and certified global.

**Ray-NURBS.** Two-plane reduction to a 2D system, isolate starting points with interval Newton (Toth) or 2D clipping, polish with Newton-plus-extrapolation; GPU pipeline of patch-BVH plus per-patch two-plane-plus-Newton with iterative Bezier clipping for trims when interactive performance is needed.

**Self-intersection / offsets.** PP/IPP solve of the self-intersection system with explicit diagonal exclusion, tied to collinear-normal and curvature-extremum analysis; required to keep offset, shell, and thicken operations watertight.

**Robustness posture.** Bernstein basis throughout; interval-certified results (IPP, validated ODE) on the correctness-critical paths; fundamental-form-driven step control; explicit up-front classification of solution dimensionality (empty / points / curve / patch / isolated point) so coincident and tangential cases are never silently treated as transversal. Carry a verified error bound on every intersection and projection result and feed it into the tolerant B-rep bookkeeping.

---

## References

1. Bajaj, C. L., Hoffmann, C. M., Lynch, R. E., and Hopcroft, J. E. (1988). Tracing surface intersections. Computer Aided Geometric Design, 5(4), 285-307.
2. Barnhill, R. E., and Kersey, S. N. (1990). A marching method for parametric surface/surface intersection. Computer Aided Geometric Design, 7(1-4), 257-280.
3. Barton, M., and Juttler, B. (2007). Computing roots of polynomials by quadratic clipping. Computer Aided Geometric Design, 24(3), 125-141.
4. Chen, F., Cox, D., and Liu, Y. (2005). The mu-basis and implicitization of a rational parametric surface. Journal of Symbolic Computation, 39(6), 689-706.
5. Chen, X.-D., Chen, L., Wang, Y., Xu, G., Yong, J.-H., and Paul, J.-C. (2010). Computing the Hausdorff distance between two B-spline curves. Computer-Aided Design, 42(12), 1197-1206.
6. Chen, X.-D., Yong, J.-H., Wang, G., Paul, J.-C., and Xu, G. (2008). Computing the minimum distance between a point and a NURBS curve. Computer-Aided Design, 40(10-11), 1051-1054.
7. Cox, D., Sederberg, T. W., and Chen, F. (1998). The moving line ideal basis of planar rational curves. Computer Aided Geometric Design, 15(8), 803-827.
8. Daniel, M., and Daniel, P. (1996). Using a convex pyramid to bound surface normal vectors. Computer Graphics Forum, 15(3).
9. Efremov, A., Havran, V., and Seidel, H.-P. (2005). Robust and numerically stable Bezier clipping method for ray tracing NURBS surfaces. Proc. Spring Conference on Computer Graphics (SCCG).
10. Hohmeyer, M. E. (1991). A surface intersection algorithm based on loop detection. Proc. ACM Symposium on Solid Modeling Foundations and CAD/CAM Applications, 197-207. (Also UC Berkeley TR UCB/CSD-92-681, 1992.)
11. Hu, S.-M., and Wallner, J. (2005). A second order algorithm for orthogonal projection onto curves and surfaces. Computer Aided Geometric Design, 22(3), 251-260.
12. Krishnan, S., and Manocha, D. (1997). An efficient surface intersection algorithm based on lower-dimensional formulation. ACM Transactions on Graphics, 16(1), 74-106.
13. Lou, Q., and Liu, L. (2022). Curve intersection based on cubic hybrid clipping. Visual Computing for Industry, Biomedicine, and Art, 5, 16.
14. Liu, L., Zhang, L., Lin, B., and Wang, G. (2009). Fast approach for computing roots of polynomials using cubic clipping. Computer Aided Geometric Design, 26(5), 547-559.
15. Ma, Y. L., and Hewitt, W. T. (2003). Point inversion and projection for NURBS curve and surface: control polygon approach. Computer Aided Geometric Design, 20(2), 79-99.
16. Manocha, D., and Canny, J. F. (1992). Multipolynomial resultants and linear algebra. Proc. ISSAC 1992, 96-102.
17. Manocha, D., and Canny, J. F. (1992). Algorithms for implicitizing rational parametric surfaces. Computer Aided Geometric Design, 9(1), 25-50.
18. Mourrain, B., and Pavone, J.-P. (2009). Subdivision methods for solving polynomial equations. Journal of Symbolic Computation, 44(3), 292-306.
19. Mukundan, H., Ko, K. H., Maekawa, T., Sakkalis, T., and Patrikalakis, N. M. (2004). Tracing surface intersections with a validated ODE system solver. Proc. 9th ACM Symposium on Solid Modeling and Applications, 249-254.
20. Nishita, T., Sederberg, T. W., and Kakimoto, M. (1990). Ray tracing trimmed rational surface patches. Computer Graphics (Proc. SIGGRAPH 1990), 24(4), 337-345.
21. Pabst, H.-F., Springer, J. P., Schollmeyer, A., Lenhardt, R., Lessig, C., and Froehlich, B. (2006). Ray casting of trimmed NURBS surfaces on the GPU. Proc. IEEE Symposium on Interactive Ray Tracing.
22. Patrikalakis, N. M., and Maekawa, T. (2002). Shape Interrogation for Computer Aided Design and Manufacturing. Springer. (Hyperbook edition with Cho, W., 2009: web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho.)
23. Pekerman, D., Elber, G., and Kim, M.-S. (2008). Self-intersection detection and elimination in freeform curves and surfaces. Computer-Aided Design, 40(2), 150-159.
24. Schulz, C. (2009). Bezier clipping is quadratically convergent. Computer Aided Geometric Design, 26(1), 61-74.
25. Sederberg, T. W., Anderson, D. C., and Goldman, R. N. (1984). Implicit representation of parametric curves and surfaces. Computer Vision, Graphics, and Image Processing, 28(1), 72-84.
26. Sederberg, T. W., and Chen, F. (1995). Implicitization using moving curves and surfaces. Proc. SIGGRAPH 1995, 301-308.
27. Sederberg, T. W., and Meyers, R. J. (1988). Loop detection in surface patch intersections. Computer Aided Geometric Design, 5(2), 161-171.
28. Sederberg, T. W., and Nishita, T. (1990). Curve intersection using Bezier clipping. Computer-Aided Design, 22(9), 538-549.
29. Selimovic, I. (2006). Improved algorithms for the projection of points on NURBS curves and surfaces. Computer Aided Geometric Design, 23(5), 439-445.
30. Sherbrooke, E. C., and Patrikalakis, N. M. (1993). Computation of the solutions of nonlinear polynomial systems. Computer Aided Geometric Design, 10(5), 379-405.
31. Toth, D. L. (1985). On ray tracing parametric surfaces. Computer Graphics (Proc. SIGGRAPH 1985), 19(3), 171-179.
32. Yang, J., Jia, X., and Yan, D.-M. (2023). Topology guaranteed B-spline surface/surface intersection. ACM Transactions on Graphics, 42(6), Article 211. DOI 10.1145/3618349.
33. Jia, X., et al. (2023). Topology driven approximation to rational surface-surface intersection via interval algebraic topology analysis. ACM Transactions on Graphics, 42(4). DOI 10.1145/3592452.
34. (2025). Topology guaranteed and error controlled curve tracing for parametric surface-surface intersection. Computer Aided Geometric Design (S0167839625000214).
