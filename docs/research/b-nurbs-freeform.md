# Track B: NURBS and Freeform Geometry Algorithms

Literature review supporting the design of Keel, an open-source B-rep solid modeling kernel in Rust. Scope: evaluation stability, root finding and intersection, point projection, isogeometric analysis spillover, advanced spline representations, and trimmed-surface watertightness. Emphasis on work after Piegl and Tiller (1997), especially 2010 to 2025.

## 1. Executive Summary

The classical numerical foundations of a NURBS kernel are settled and reliable. De Boor for B-spline/NURBS evaluation and de Casteljau for Bezier evaluation remain the correct default workhorses: both are backward stable, run in low-degree-per-span arithmetic, and avoid the conditioning blowups of monomial or power-basis forms. The frontier work since 2010 is not about replacing these algorithms but about three things: making evaluation robust for unusual inputs (very high degree, multi-degree splines, pathological knot vectors), making polynomial root finding both fast and certified, and making trimmed-surface boundaries watertight so that downstream booleans and meshing do not fail at gaps.

For root finding and intersection, two families dominate and are complementary. Subdivision solvers in the Bernstein basis (Projected Polyhedron / Interval Projected Polyhedron from MIT, and the Mourrain-Pavone reduction methods that improve them) are the robust, globally convergent workhorses for systems of polynomial equations, which is what most curve-curve, curve-surface, and surface-surface intersection problems reduce to. Bezier clipping (Sederberg-Nishita) and its quadratic/cubic clipping successors give fast, geometrically guided convergence for the univariate and bivariate sub-problems. For the small fixed-degree closed-form solvers that appear everywhere in a kernel (quadratics from circle/line intersections, cubics and quartics from torus and offset problems), the modern recommendation is Yuksel's HPG 2022 monotonic-interval Newton method and Blinn's numerically careful closed forms, not naive Cardano/Ferrari.

Isogeometric analysis (Hughes et al., 2005 onward) has been a major funder of NURBS research and has hardened exactly the primitives a kernel reuses: stable knot insertion, order elevation, refinement, Bezier extraction, and efficient quadrature. THB-splines, T-splines, hierarchical B-splines, and subdivision surfaces are valuable but are best treated as application-layer or interchange representations, not as the kernel's core surface type. The most actionable gap is trimmed-NURBS watertightness: the literature confirms gaps are intrinsic to independent trimmed patches and offers untrimming and reconstruction strategies, but no fully robust, tolerance-driven, automatic solution exists. Keel should design its topology layer to carry and enforce tolerances explicitly rather than hope geometry is exact.

## 2. Annotated Key References

### Evaluation stability

**Piegl, L. and Tiller, W. (1997). The NURBS Book, 2nd ed. Springer.** The canonical baseline. Establishes de Boor (Cox-de Boor recurrence) for B-spline basis and curve/surface evaluation, knot insertion (Boehm), the Oslo algorithm, degree elevation, and decomposition into Bezier segments. Everything below is measured against this. Takeaway: adopt its algorithms as the default implementation target, then layer the post-1997 stability refinements on top.

**Beccari, C. V. and Casciola, G. (2021). Stable numerical evaluation of multi-degree B-splines. Journal of Computational and Applied Mathematics. arXiv:2102.03252, https://arxiv.org/abs/2102.03252.** Provides a numerically stable algorithm for evaluating multi-degree B-splines (splines whose pieces have different polynomial degrees) by constructing the mapping between bases using only stable operations: knot insertion and degree elevation. Takeaway: if Keel ever supports multi-degree splines, this is the reference implementation pattern; the broader lesson is that stable operations compose into stable evaluators.

**Recent differential-recurrence and Bernstein-Bezier extraction work (2021 to 2024).** A thread of papers builds B-spline basis values by extracting per-span Bernstein-Bezier coefficients via stable recurrences, reporting more preserved significant digits than direct Cox-de Boor at high degree. Takeaway: for the rare high-degree path, evaluate by extracting Bezier coefficients per knot span and running de Casteljau, which keeps arithmetic in [0,1] and is the most stable practical route.

### Root finding and polynomial system solvers

**Sherbrooke, E. C. and Patrikalakis, N. M. (1993). Computation of the solutions of nonlinear polynomial systems. Computer Aided Geometric Design 10(5):379-405.** Introduces the Projected Polyhedron (PP) algorithm and its interval arithmetic variant IPP. An n-dimensional global root finder for polynomial systems in Bernstein form: it subdivides multivariate Bernstein polynomials and uses 2D convex hulls of control points projected onto coordinate planes to clip away root-free regions. Documented in detail in Patrikalakis and Maekawa, Shape Interrogation for Computer Aided Design and Manufacturing (Springer, 2002), online at https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/. Takeaway: this is the robust global backbone for intersection starting points and constraint solving; adopt the Bernstein-subdivision + convex-hull-clip structure.

**Mourrain, B. and Pavone, J.-P. (2009). Subdivision methods for solving polynomial equations. Journal of Symbolic Computation 44(3):292-306. Inria report: https://inria.hal.science/inria-00070350.** Improves IPP with a stronger reduction strategy: a univariate Bernstein-basis root finder using Descartes' rule plus a preconditioning step, achieving local quadratic convergence for simple roots and provable complexity bounds. Demonstrated on implicit curve intersection, rational curve self-intersection, and the parallel-robot benchmark. Takeaway: this is the modern, faster successor to plain IPP; adopt its reduction-before-subdivision idea to cut subdivision steps dramatically.

**Elber, G. and Kim, M.-S. (2001). Geometric constraint solver using multivariate rational spline functions. ACM Symposium on Solid Modeling and Applications, http://3map.snu.ac.kr/publications/docs-intl-conf/sm01.pdf.** The IRIT multivariate solver line: expresses geometric constraints (intersections, extrema, bisectors, sweeps) as systems of multivariate rational spline equations and solves by subdivision plus root isolation, with later work on subdivision termination criteria (Elber et al., 2006). IRIT is a long-lived, battle-tested research kernel. Takeaway: the "reduce every geometric query to a multivariate spline constraint system, then hand it to one robust solver" architecture is worth emulating; it consolidates intersection, projection, and interrogation behind a single solver.

**Sederberg, T. W. and Nishita, T. (1990). Curve intersection using Bezier clipping. Computer-Aided Design 22(9):538-549. https://www.sciencedirect.com/science/article/abs/pii/001044859090039F.** Introduces the "fat line" and Bezier clipping: an interval-Newton-like method that uses the convex hull and a bounding fat line to discard parameter ranges that cannot contain an intersection, converging quadratically. Faster than implicitization for degree five and above. Takeaway: adopt as the univariate/bivariate intersection refiner; it is simple, robust, and quadratically convergent, and pairs naturally with subdivision globalization.

**Barton, M. and Juttler, B. (2007) and successors: quadratic clipping; plus cubic hybrid clipping (2022, Visual Computing for Industry, Biomedicine, and Art, https://vciba.springeropen.com/articles/10.1186/s42492-022-00114-3).** Quadratic clipping bounds the polynomial between two quadratics for cubic (order 3) convergence on roots; hybrid and cubic-hybrid clipping push to higher convergence order by fitting low-degree enclosing polynomials. Takeaway: a worthwhile INVESTIGATE for the hot univariate root-finding inner loop where higher-order clipping can beat plain Bezier clipping on iteration count.

### Closed-form and small-degree solvers

**Yuksel, C. (2022). High-Performance Polynomial Root Finding for Graphics. Proceedings of the ACM on Computer Graphics and Interactive Techniques 5(3), HPG 2022. PDF: https://www.cemyuksel.com/research/polynomials/polynomial_roots_hpg2022.pdf. DOI: 10.1145/3543865.** Finds real roots by first isolating intervals on which the polynomial is monotonic (via roots of the derivative, recursively), then running a robust bracketed Newton iteration in each interval with guaranteed convergence. For cubics it beats both the analytical Cardano solution and naive Newton in accuracy and speed, and it scales cleanly to degree 20+. Takeaway: ADOPT as Keel's general fixed-degree real-root finder; it gives bracketing safety (no Cardano cancellation, no Newton divergence) with excellent performance and a clean recursive structure.

**Blinn, J. F. (2005-2007). How to Solve a Quadratic Equation; How to Solve a Cubic Equation (Parts 1-4), IEEE Computer Graphics and Applications.** Homogeneous quadratic formulation that avoids division by zero and catastrophic cancellation; cubic strategy that computes the largest-magnitude root first and uses the reciprocal-polynomial trick to keep every branch numerically stable. See the practitioner write-up at https://momentsingraphics.de/CubicRoots.html. Takeaway: adopt Blinn's careful quadratic as the quadratic primitive (used constantly in analytic-geometry intersections), and use his cubic approach where a closed form is preferred; both avoid the well-known failure modes of textbook formulas.

**Companion-matrix eigenvalue root finding (LAPACK-style).** General real/complex root finding via the eigenvalues of the companion matrix is robust and widely used but heavier than specialized small-degree solvers. Takeaway: keep as a fallback for arbitrary high degree where Yuksel-style methods are not configured, not as the default for degree 2 to 4.

### Curve-surface and surface-surface intersection

**Topology Guaranteed B-Spline Surface/Surface Intersection (2023). ACM Transactions on Graphics 42(6), DOI: 10.1145/3618349. https://dl.acm.org/doi/10.1145/3618349.** A practical algorithm computing intersection loci of two B-spline surfaces with guaranteed correct topology even in near-critical (near-tangent) configurations, combining algebraic topology analysis, iterative subdivision robustness, and forward marching efficiency. Related: "Topology driven approximation to rational surface-surface intersection via interval algebraic topology analysis," ACM TOG, DOI: 10.1145/3592452. Takeaway: this is the current state of the art for SSI and the model to target; the three-way combination (algebra for topology, subdivision for robustness, marching for speed) is the right architecture.

**Topology guaranteed and error controlled curve tracing for parametric surface-surface intersection (2025). Computer Aided Geometric Design, https://www.sciencedirect.com/science/article/abs/pii/S0167839625000214.** Adds explicit error control to the marched intersection curve and approximates it as a NURBS curve within a bound. Takeaway: SSI output must be a tolerance-bounded NURBS curve in both parameter domains plus model space; design the intersection result type to carry that error bound.

### Closest-point projection

**Selimovic, I. (2006). Improved algorithms for the projection of points on NURBS curves and surfaces. Computer Aided Geometric Design 23(5):439-445. https://www.sciencedirect.com/science/article/abs/pii/S0167839606000185.** Uses control-polygon/control-net exclusion criteria within a subdivision strategy to discard regions that cannot contain the closest point, improving robustness and cutting computation versus naive global Newton. Takeaway: ADOPT the subdivide-to-locate-then-Newton-on-a-flat-Bezier-patch pattern; do not run Newton on the whole surface.

**Control-polygon and geometric-iteration projection methods (Ma and Hewitt 2003; later second-order geometric iteration).** Newton-Raphson on a full NURBS surface can converge to wrong answers even from good seeds, so robust pipelines first subdivide into Bezier pieces, pick candidate pieces by point-to-control-net relationship, and only then refine. Geometric iteration methods report global second-order convergence and insensitivity to the initial guess. Takeaway: globalize projection by subdivision-based candidate selection and treat parameter correction as bracketed, not as unguarded Newton.

### Isogeometric analysis spillover

**Hughes, T. J. R., Cottrell, J. A., and Bazilevs, Y. (2005). Isogeometric analysis: CAD, finite elements, NURBS, exact geometry and mesh refinement. Computer Methods in Applied Mechanics and Engineering 194(39-41):4135-4195. PDF: https://www.ljll.fr/~frey/papers/meshing/.** Founding IGA paper. Drove a decade of hardening of knot insertion (h-refinement), order elevation (p-refinement), the combined k-refinement, and nestedness guarantees. Takeaway: reuse IGA's refinement machinery and its insistence that refinement must not change geometry or parameterization; that invariant is exactly what a kernel needs for robust subdivision.

**Hughes, J. A. Cottrell, Y. Bazilevs et al. (2010). Efficient quadrature for NURBS-based isogeometric analysis. CMAME 199:301-313. https://www.sciencedirect.com/science/article/abs/pii/S0045782508004295.** Establishes the "half-point rule" and smoothness-aware quadrature for spline bases. Takeaway: if Keel computes areas, volumes, moments, or mass properties on NURBS, use smoothness-aware quadrature rather than naive Gauss-per-Bezier, which over-integrates.

### Advanced spline representations

**Sederberg, T. W. et al. (2003/2004). T-splines and T-NURCCs. ACM TOG.** Local refinement without propagating knot lines across the whole tensor grid; the basis for converting trimmed NURBS to watertight T-spline models. Takeaway: relevant as an interchange/untrimming target, not as the kernel core.

**Giannelli, C., Juttler, B., and Speleers, H. (2012). THB-splines: the truncated basis for hierarchical splines. CAGD 29(7):485-498. https://www.sciencedirect.com/science/article/abs/pii/S0167839612000519.** Truncation restores partition of unity and reduces support overlap in hierarchical B-splines, giving clean local adaptive refinement. Takeaway: excellent for adaptive analysis and fitting, an application-layer tool, not a B-rep core surface.

**Stam, J. (1998). Exact Evaluation of Catmull-Clark Subdivision Surfaces at Arbitrary Parameter Values. SIGGRAPH. https://www.dgp.toronto.edu/public_user/stam/reality/Research/pdf/sig98.pdf.** Makes subdivision surfaces directly evaluable like a parametric patch. The literature explicitly notes subdivision is widely used in graphics but "largely ignored in solid modeling." Takeaway: keep subdivision surfaces out of the B-rep core; support import/approximation to NURBS if needed.

### Trimmed-NURBS watertightness

**Sederberg, T. W. et al. (2008). Watertight trimmed NURBS. ACM TOG 27(3), DOI: 10.1145/1360612.1360678.** Converts each trimmed NURBS into an untrimmed T-spline and merges them into a single watertight model. Takeaway: a concrete route to gap-free models, but it changes the representation; useful for export, heavyweight for the core.

**Untrimming: precise conversion of trimmed surfaces to tensor-product surfaces (2017) and Reconstruction of Trimmed NURBS Surfaces for Gap-Free Intersections (2020, ASME JCISE 20(5):051008).** Reparameterize or reconstruct trimmed regions into untrimmed tensor-product patches that meet gap-free along shared edges. Recent: "Scalable Field-Aligned Reparameterization for Trimmed NURBS," arXiv:2410.14318 (2024). Takeaway: untrimming is an active, unsolved-in-general problem; treat it as an optional post-process, and make the native B-rep tolerant so watertightness is enforced at the topology level.

## 3. State-of-the-Art Assessment

Evaluation is a solved problem for the common case. De Boor and de Casteljau are backward stable and remain the standard; the only open refinements concern multi-degree splines and extreme-degree evaluation, both handled by extracting per-span Bezier coefficients and evaluating in [0,1]. No serious kernel should evaluate NURBS in the power basis.

Root finding has bifurcated cleanly. For systems (the intersection/interrogation core), Bernstein-basis subdivision solvers (IPP, Mourrain-Pavone, IRIT) are the robust global standard, with reduction-augmented subdivision now clearly preferred over plain subdivision. For small fixed degrees, Yuksel 2022 and Blinn give numerically safe, fast solvers that beat textbook closed forms. Bezier and quadratic/cubic clipping provide the high-convergence local refiners. There is broad consensus and mature, citable algorithms across this whole stack.

Intersection topology is the active research front. As of 2023 to 2025, topology-guaranteed SSI with error control exists and works in near-critical configurations by fusing algebraic topology, subdivision robustness, and marching efficiency. This is newer, more complex, and less commoditized than evaluation or basic root finding, and it is where a new kernel will spend the most engineering effort to match commercial robustness.

IGA has been a strong tailwind: it hardened refinement, Bezier extraction, and quadrature, all directly reusable. Advanced representations (T-splines, THB, subdivision) are mature in their niches but remain application-layer.

Trimmed-surface watertightness is the most consequential unsolved practical problem. Gaps are intrinsic to independently trimmed patches; untrimming and reconstruction help but are not universally robust or automatic. This is the single biggest risk area for a NURBS B-rep kernel aiming at robust booleans.

## 4. Design Impact for Keel

### ADOPT

- De Boor for NURBS evaluation, de Casteljau for Bezier, both implemented per Piegl-Tiller, with per-knot-span Bezier extraction as the high-degree and stability path. Never evaluate in the power/monomial basis.
- Knot insertion (Boehm/Oslo), degree elevation, and Bezier decomposition as core stable operations, reusing the IGA-hardened guarantee that refinement does not change geometry or parameterization.
- A single Bernstein-basis subdivision solver as the backbone for all polynomial systems (curve-curve, curve-surface, projection seeds, interrogation), built on the Projected Polyhedron structure and upgraded with Mourrain-Pavone style reduction/preconditioning to cut subdivision steps.
- Bezier clipping (Sederberg-Nishita) as the quadratically convergent local refiner for univariate/bivariate intersection sub-problems.
- Yuksel (HPG 2022) monotonic-interval bracketed Newton as the default real-root finder for fixed-degree polynomials, and Blinn's numerically careful quadratic (homogeneous form) and cubic strategies for the constant analytic-geometry primitives.
- Selimovic-style subdivision-with-exclusion for point projection: locate a candidate Bezier patch first, then run bracketed Newton on the flat patch; never run Newton globally on a full NURBS surface.
- A tolerant B-rep model from day one: every geometric entity carries explicit tolerance, and the topology layer enforces watertightness independently of geometric exactness. This is the Parasolid-class design choice.
- Smoothness-aware quadrature (IGA half-point rule) for mass properties and area/volume integration over splines.

### AVOID

- Power-basis or single high-degree polynomial evaluation of NURBS; it is the classic conditioning trap.
- Naive Cardano/Ferrari closed forms for cubics/quartics without Blinn/Yuksel safeguards; they suffer cancellation and lose roots.
- Unbracketed global Newton for intersection or projection; it converges to wrong answers from reasonable seeds on curved surfaces.
- Making T-splines, THB-splines, hierarchical B-splines, or subdivision surfaces the kernel's core surface type. They are application-layer or interchange representations; keep the core tensor-product NURBS plus analytics.
- Assuming exact geometric watertightness from independently trimmed patches; the literature is clear that gaps are intrinsic.

### INVESTIGATE

- Higher-order clipping (quadratic clipping, cubic hybrid clipping) for the hot univariate root-finding inner loop, where order-3+ convergence can beat plain Bezier clipping on iteration count.
- A topology-guaranteed, error-controlled SSI pipeline (algebraic topology for correctness, subdivision for robustness, marching for speed, NURBS approximation with a tolerance bound for output) modeled on ACM TOG 2023 (DOI 10.1145/3618349) and the 2025 CAGD curve-tracing work. This is the highest-effort, highest-value module.
- Untrimming/reconstruction (Sederberg 2008 watertight T-splines; 2020 ASME reconstruction; arXiv:2410.14318 reparameterization) as an optional export/interop path for analysis and manufacturing, decoupled from the native trimmed core.
- The IRIT "reduce every query to one multivariate spline constraint solver" architecture as a way to consolidate intersection, projection, extrema, and interrogation behind a single robust engine, reducing surface area for bugs.

## 5. Gaps and Open Problems

1. Robust, automatic, tolerance-driven trimmed-surface watertightness. No method universally and automatically converts arbitrary trimmed NURBS assemblies into provably gap-free models at a user tolerance. This is the chief obstacle to robust booleans on NURBS-bounded solids and the area most likely to require original engineering in Keel.
2. Certified topology for surface-surface intersection in fully general, near-degenerate configurations. Recent work guarantees topology for B-spline SSI, but coverage of all tangency/grazing/self-intersection cases at production robustness is still maturing.
3. End-to-end certified intersection pipelines (interval/exact arithmetic from seed finding through marching to NURBS approximation) that remain fast enough for interactive modeling. Certified variants exist in pieces; a unified, performant, open implementation is rare.
4. Stable evaluation and operations for very high degree and pathological knot vectors (near-coincident knots, extreme multiplicity) lack a single agreed best practice beyond Bezier extraction; worth a small targeted study before committing to a degree cap.
5. Open-source maturity gap. The robust algorithms are published, but freely available, production-grade, well-tested Rust implementations of the full stack (subdivision solver, topology-guaranteed SSI, tolerant booleans) do not yet exist. This is precisely Keel's opportunity and its main build risk.
