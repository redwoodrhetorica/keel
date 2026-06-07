# Track A: Robust Geometric Computation — Literature Review for Keel

Research date: 2026-06-07. Scope: foundational work (post-1990) and recent work (2010-2025) on robust numerics for a NURBS-plus-analytic B-rep kernel built on tolerant f64 with exact predicates. Style note: this document avoids em-dashes by design.

## 1. Executive Summary

The robustness problem in solid modeling stems from a mismatch: geometric algorithms are designed for the Real-RAM model assuming exact real arithmetic and inputs in general position, but they execute on IEEE 754 f64 where rounding makes predicate evaluations (orientation, in-circle, in-sphere, sidedness) occasionally return the wrong sign. A single wrong sign can produce inconsistent combinatorial decisions, leading to crashes, infinite loops, or invalid topology. Two broad traditions address this.

The first is the **Exact Geometric Computation (EGC)** paradigm (Yap, and as embodied in CGAL and CORE). EGC does not compute exact real numbers; it guarantees that every *predicate* (sign decision) is correct, computing only to the precision needed to resolve the sign. Modern EGC is made affordable by **floating-point filters**: a fast f64 evaluation with a certified error bound runs first, and exact arithmetic (expansions or arbitrary-precision rationals, often via lazy interval evaluation) runs only when the filter cannot certify the sign. Shewchuk's 1997 adaptive-precision predicates are the canonical fast-path-plus-exact-fallback implementation and remain the backbone of the field; the Rust `robust` crate is a direct port.

The second tradition is **tolerant / epsilon modeling**, the commercial-kernel approach (Parasolid, ACIS). Geometry is approximate but topology carries explicit tolerances (tolerant edges and vertices) so that surfaces that do not intersect cleanly within machine resolution are still glued into valid topology. Tolerant modeling is essential for handling imported, imperfect, and curved geometry that EGC alone cannot make exact, but it is harder to implement reliably and shifts the burden from arithmetic to topological bookkeeping.

For Keel, the two are complementary, not competing. The state of the art for a NURBS kernel is a layered design: exact filtered predicates for the *linear/analytic* substrate and combinatorial decisions (where EGC is cheap and bulletproof), interval and affine arithmetic for *certified curved evaluation* (surface-surface intersection, where exactness is generally impossible because intersection points are algebraic numbers of high degree), and explicit tolerance propagation in the topology layer to absorb the residual uncertainty of curved constructions. The most important recent development for Keel specifically is **Attene's indirect predicates** (2020), which keep predicates exact even when their inputs are *constructed* points (line-plane and three-plane intersections), exactly the situation booleans create. The catch is that indirect predicates are defined only for linear primitives, so they cover a polyhedral or linearized substrate but do not by themselves solve exact predicates on NURBS intersections.

## 2. Annotated Key References

### 2.1 Foundational exact predicates

**Shewchuk, J. R. (1997). "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates." Discrete & Computational Geometry 18(3): 305-363.** DOI: 10.1007/PL00009321. PDF: https://people.eecs.berkeley.edu/~jrs/papers/robustr.pdf . Code: https://www.cs.cmu.edu/~quake/robust.html .
The foundational paper. Provides (a) algorithms for exact addition and multiplication of f64 values using nonoverlapping *floating-point expansions* (sequences of f64 whose exact sum is the value), building on Dekker/Knuth two-sum and Priest's work; (b) *adaptive* evaluation where successive correction terms are computed only until the sign is certain, so running time scales with the difficulty of the input rather than worst-case precision; (c) concrete `orient2d`, `orient3d`, `incircle`, `insphere` predicates. Requires radix-2, exactly-rounded arithmetic (IEEE 754). This remains the single most-cited and most-deployed robustness technique and should be considered the baseline Keel must match or wrap.

**Edelsbrunner, H. and Mücke, E. P. (1990). "Simulation of Simplicity: A technique to cope with degenerate cases in geometric algorithms." ACM Transactions on Graphics 9(1): 66-104.** DOI: 10.1145/77635.77639. PDF: https://arxiv.org/abs/math/9410209 (and Sandia mirror).
Introduces *symbolic perturbation*: each input is perturbed by a distinct infinitesimal (a power of an indeterminate epsilon) that is never numerically applied but is resolved symbolically in the predicate sign computation, guaranteeing no degenerate (zero) predicate result ever occurs. This removes the need to write special-case code for collinear, cocircular, or coplanar configurations. Powerful for triangulations and arrangements of *linear* objects; the cost is that the perturbed answer must then be interpreted, and the technique is intricate to extend to curved geometry.

### 2.2 Floating-point filters and certified error bounds

**Meyer, A. and Pion, S. (2008). "FPG: A code generator for fast and certified geometric predicates." Real Numbers and Computers (RNC8), pp. 47-60.** PDF: https://inria.hal.science/inria-00344297/document .
Formalizes the filter taxonomy Keel should reason in: *static* filters (global, input-independent error bound), *semi-static* (bound has a constant part plus a part scaled by input magnitudes), *dynamic* (interval-arithmetic-style, error recomputed every step), and the FPG contribution *almost-static* (begins from initial input bounds and refreshes only when inputs exceed them). FPG auto-generates near-zero-overhead certified filters from a predicate's algebraic expression. The conceptual takeaway: the right default for a predicate is a semi-static filter, falling back to interval then exact.

**Brönnimann, Burnikel, Pion (and related, "Interval arithmetic yields efficient dynamic filters for computational geometry," 2001).** Plus **Ozaki et al. / "Simple floating-point filters for the two-dimensional orientation problem," BIT Numerical Mathematics 56 (2016).** DOI: 10.1007/s10543-015-0574-9.
These give the explicit forward-error analysis for the 2D orientation test and tight, simple constant factors (small multiples of unit roundoff times the magnitude of the largest term). Useful as a direct, copyable error-bound derivation when Keel hand-writes its `orient2d`/`orient3d` filters rather than depending on a generator.

**Bartels, T. (2022). "Fast Floating-Point Filters for Robust Predicates."** arXiv:2208.00497. PDF: https://arxiv.org/pdf/2208.00497 .
Recent and directly Rust-relevant: the author is associated with the Rust geometry ecosystem and the work studies how to generate and tune fast filters in a modern systems language. Important because it tracks the practical question of how to get filter overhead close to naive f64 in code Keel could actually adopt or mirror.

### 2.3 Indirect predicates and constructions (most relevant to booleans)

**Attene, M. (2020). "Indirect Predicates for Geometric Constructions." Computer-Aided Design 126: 102856.** DOI: 10.1016/j.cad.2020.102856. Preprint: https://arxiv.org/abs/2105.09772 . Code: https://github.com/MarcoAttene/Indirect_Predicates .
The key insight for boolean operations. When a predicate's input is itself a *constructed* point (for example the intersection of two segments, or a line-plane intersection), evaluating the construction in f64 first and then the predicate corrupts the result no matter how clever the predicate is. Attene represents constructed points *implicitly* as unevaluated rational expressions lambda/d over the original explicit inputs, defines point types LPI (line-plane intersection) and TPI (three-plane intersection), and composes the construction's expression *into* the predicate so the whole thing is evaluated exactly with a cascaded filter (semi-static f64, then interval, then expansions). Result: predicates on constructed points that are exact yet nearly as fast as non-robust f64. Limitation, stated plainly in the paper: it is defined for *linear* primitives only (lines, planes). It does not extend to NURBS intersection points, which are algebraic of high degree.

**Cherchi, Livesu, Scateni, Attene (2020). "Fast and Robust Mesh Arrangements using Floating-point Arithmetic." ACM TOG 39(6) (SIGGRAPH Asia).** DOI: 10.1145/3414685.3417818. And **Cherchi et al. "Interactive and Robust Mesh Booleans" (2022).** PDFs at https://www.gianmarcocherchi.com .
These show indirect predicates in action: exact, robust triangle-mesh arrangements and booleans built entirely on indirect predicates plus floating-point filtering, outperforming CGAL's exact-construction kernel on mixed/constructed inputs and avoiding its memory blowups. This is the strongest evidence that the indirect-predicate approach is production-grade for the *linearized* boolean problem.

### 2.4 The exact computation paradigm and its libraries

**Yap, C. K. "Towards Exact Geometric Computation." (and Yap & Dubé, "The exact computation paradigm," 1995.)** CGAL summary: https://www.cgal.org/exact.html .
Defines EGC precisely: robustness requires only that predicate (branching) decisions are never wrong; we compute to sufficient precision to decide each sign and no further. This reframes "exact" from "exact reals" (impossible for algebraic numbers in finite time in general) to "exact decisions," which is achievable and affordable with filters. This is the philosophical core Keel should adopt for its *combinatorial* layer.

**Fabri, Pion, et al. "A generic lazy evaluation scheme for exact geometric computations." Science of Computer Programming (2011).** https://www.sciencedirect.com/science/article/pii/S016764231000167X . Plus CGAL's `Exact_predicates_inexact_constructions_kernel` (EPICK) and `Exact_predicates_exact_constructions_kernel` (EPECK).
CGAL's lazy exact kernel builds a DAG of the construction history; it evaluates with interval arithmetic first and only re-evaluates the relevant subexpression exactly (via rationals/algebraics in CORE or LEDA-style number types) when an interval cannot decide a sign. EPICK (exact predicates, inexact constructions) is the workhorse default and is the closest existing analog to Keel's intended stance. The cost of EPECK (fully exact constructions) is the memory/perf blowup Attene's experiments highlight, which is precisely why a NURBS kernel cannot go fully exact.

### 2.5 Interval and affine arithmetic for curved geometry

**Hu, Patrikalakis, Ye. "Robust interval algorithm for surface intersections." Computer-Aided Design 28(6-7) (1996).** https://www.sciencedirect.com/science/article/abs/pii/S0010448596000991 . Foundational text: **Patrikalakis & Maekawa, "Shape Interpolation and Robust Computation of Geometric Operations / Shape Interrogation for Computer Aided Design and Manufacturing" (Springer, 2002).**
Establishes interval NURBS and the rounded-interval-arithmetic approach to surface-surface intersection (SSI): represent control points as intervals, subdivide, and use interval Newton to isolate and certify intersection branches, handling transversal and (harder) tangential/non-transversal cases. This is the canonical robust SSI methodology and directly informs Keel's curved-intersection layer.

**Lin, Yang, et al. "Affine Arithmetic-Based B-Spline Surface Intersection with GPU Acceleration." IEEE TVCG (2014).** DOI: 10.1109/TVCG.2013.237.
Affine arithmetic (AA) tracks linear correlations between error terms, giving far tighter enclosures than naive interval arithmetic and so far fewer spurious subdivisions; AA is shown to find every intersection branch and to handle tangency/singular cases. The paper extends AA to locally-supported B-spline basis functions and accelerates it on GPU. Takeaway: AA, not plain IA, is closer to state of the art for *tight certified enclosures* of curved evaluations.

**Recent (2023-2025): "Topology driven approximation to rational surface-surface intersection via interval algebraic topology analysis" (ACM TOG, 2023, DOI 10.1145/3592452); and a 2025 ACM TOG NURBS intersection paper handling small loops and tangent intersections (DOI 10.1145/3807948).**
These represent the current frontier in robust SSI, combining algebraic topology guarantees, iterative subdivision robustness, and forward marching efficiency, and benchmark favorably against IRIT, SISL, and Parasolid on hard topological cases. They are the papers to track for Keel's most ambitious milestone (booleans on NURBS-bounded solids).

### 2.6 Perturbation, snap rounding, and reliability without exactness

**Halperin, D., Leiserowitz (Raab), Mehlhorn, et al. "Controlled Perturbation."** Survey/chapter: "Controlled Perturbation for Certified Geometric Computing with Fixed-Precision Arithmetic" (2010), https://link.springer.com/chapter/10.1007/978-3-642-15582-6_19 . Also Mehlhorn et al., "Reliable and Efficient Computational Geometry via Controlled Perturbation" (ICALP 2006).
Replaces an input with a nearby perturbed input that is guaranteed (via a precomputed, precision-dependent perturbation bound derived from differential calculus) to be non-degenerate and correctly evaluable in plain fixed-precision f64. Demonstrated for arrangements of polyhedral surfaces, spheres, and circles. Attractive because it keeps fast f64 arithmetic and needs no exact fallback, but it *moves the output*, which is unacceptable when exact incidence with the input must be preserved.

**Snap rounding family: Halperin & Packer, "Iterated Snap Rounding" (CGTA 2002); Hershberger, "Stable Snap Rounding" (SoCG 2011) and "Improved Output-Sensitive Snap Rounding" (DCG 2008); Belussi et al., "Snap Rounding with Restore" (ACM TSAS 2016).** And **"Geometric Rounding and Feature Separation in Meshes," arXiv:1805.03252.**
Snap rounding takes an arrangement of segments and rounds vertices to a grid while keeping topology consistent and vertices/edges well-separated; the variants fix drift and stability problems. This is the mechanism for producing a *robust, finite-precision representable output* after an exact computation, relevant to Keel when emitting results to downstream consumers. Curve-aware snap rounding exists but is far less mature than the segment case.

### 2.7 Formal verification and GPU

**Boldo, Melquiond, and collaborators: "Formally Certified Floating-Point Filters For Homogeneous Geometric Predicates" (INRIA, RR), https://inria.hal.science/inria-00071232 ; and Coq-derived C++ predicate generation (https://inria.hal.science/hal-00916880 ).** Background: Boldo & Melquiond, "Computer Arithmetic and Formal Proofs" (2017).
Machine-checked proofs (Coq, using the Flocq floating-point formalization) that specific predicate filters return the correct sign or correctly report uncertainty, and code extraction of verified predicates usable in real kernels. This is the credibility frontier: it gives confidence that a hand-tuned filter's error bound is actually sound.

**Yan, K., et al. "GPredicates: GPU Implementation of Robust and Adaptive Floating-Point Predicates for Computational Geometry." IEEE Access (2019).** DOI 10.1109/ACCESS.2019.2910500. And Menezes et al., "Accelerating the Exact Evaluation of Geometric Predicates with GPUs" (IMR 2019) / "Fast Parallel Evaluation of Exact Geometric Predicates on GPUs" (CAD 2022).
GPU ports split Shewchuk's predicate into a fast-check kernel and an exact-check kernel, compacting the few threads that need exact evaluation; or run interval filters on GPU and re-check failures on CPU with rationals, reporting up to ~289x speedups. Relevant only if Keel later parallelizes predicate-heavy stages; not needed for a correct serial kernel.

## 3. State-of-the-Art Assessment

For *linear and analytic* geometry and all combinatorial decisions, the field is essentially solved and converged: filtered, adaptive, exact predicates (Shewchuk-style expansions behind a semi-static filter, with interval arithmetic as a middle tier) give correct signs at near-f64 speed, and indirect predicates extend this to constructed points without explicit exact constructions. CGAL's EPICK kernel and Attene's indirect-predicate libraries are the two production references, and both are validated at scale (mesh booleans on millions of primitives). Formal verification of the filter error bounds is now available and removes the last "did we get the constant right" doubt.

For *curved* geometry (NURBS surface-surface intersection), there is no exact paradigm, because intersection points are algebraic numbers of high degree and the SSI curve has no rational parameterization in general. The state of the art is *certified enclosure*: interval/affine arithmetic plus interval Newton and subdivision to isolate and verify branches, now augmented (2023-2025) with algebraic-topology guarantees on the branch structure. These methods are robust and benchmark against Parasolid, but they are slower, and tangential/near-tangential intersections remain the hard, actively-researched case. The residual uncertainty here is what *forces* a tolerant topology layer in any NURBS kernel: you cannot make the curved intersection exact, so you must represent its uncertainty explicitly. This is exactly what Parasolid and ACIS do with tolerant edges and vertices.

The clear consensus design for a Parasolid-class kernel in 2026: exact filtered predicates underneath, certified interval/affine evaluation for curved interrogation, and explicit tolerance propagation in topology to carry the irreducible curved uncertainty. Snap rounding and controlled/symbolic perturbation are supporting tools, not the spine.

## 4. Design Impact for Keel

Keel's stated stance (tolerant f64 plus exact predicates) is well-aligned with the EPICK + tolerant-topology consensus. Concrete recommendations:

### ADOPT
- **Shewchuk-style filtered adaptive exact predicates as the predicate spine.** Wrap or reimplement `orient2d/3d`, `incircle`, `insphere`, and sidedness. Start by depending on the Rust `robust` crate (direct Shewchuk port, MIT/Apache-2.0, v1.0 since 2023) for `orient2d`/`incircle`, and extend for 3D. This is the cheapest path to correctness.
- **The EGC discipline of "exact predicates, inexact constructions" (EPICK model) for the combinatorial layer.** Never branch on a raw f64 sign; always route sign decisions through a filtered exact predicate. Keep constructions in f64.
- **A three-tier filter cascade per predicate: semi-static f64 filter, then interval arithmetic, then exact expansion.** This is the universally-validated structure (FPG, CGAL, Attene). Use semi-static, not purely static, bounds.
- **Indirect predicates (Attene LPI/TPI representation) for the linearized boolean substrate.** When booleans create intersection points between planar/linear faces or linearized patches, carry them as unevaluated constructions and evaluate predicates exactly over them. This is the single most impactful adoption for robust booleans on the linear parts.
- **Explicit tolerance propagation in topology (tolerant edges/vertices), Parasolid-style,** to absorb the unavoidable uncertainty of curved intersections and imported geometry.

### AVOID
- **Fully exact constructions (CGAL EPECK style) as the default.** The DAG/rational-blowup memory and performance cost is real (Attene shows CGAL exhausting 1.6GB+ and crashing on cases his method handles), and it is impossible for genuine NURBS intersections anyway. Reserve exact constructions, if used at all, for tiny linear sub-problems.
- **Controlled perturbation and symbolic perturbation as the primary robustness mechanism.** Controlled perturbation moves the output, which conflicts with a kernel that must preserve exact incidence with user input and with reproducible B-rep topology. Simulation of Simplicity is elegant for tie-breaking but is intricate for curved geometry and can be deferred or replaced by explicit, deterministic degeneracy handling.
- **A single global epsilon for all comparisons.** This is the classic tolerant-modeling failure mode. Tolerances must be local (per edge/vertex) and derived from the construction, not a fixed constant.
- **Premature GPU predicate work.** It is a speed optimization, not a correctness requirement; it adds substantial complexity. Defer until profiling shows predicate evaluation dominates.

### INVESTIGATE
- **Affine arithmetic (not just interval arithmetic) for certified NURBS evaluation and SSI**, for tighter enclosures and fewer subdivisions, including the locally-supported-basis extension. Benchmark against plain rounded interval arithmetic on Keel's own SSI test cases.
- **The 2023-2025 topology-driven / algebraic-topology SSI methods** (ACM TOG 2023 DOI 10.1145/3592452; 2025 DOI 10.1145/3807948) for the hard tangency and small-loop cases. These are the current frontier and benchmark against Parasolid.
- **Formally-verified filter error bounds (Boldo/Melquiond, Flocq/Coq).** Even if Keel does not extract verified code, using their proven bounds for hand-written filters removes a class of subtle soundness bugs. Worth a focused read before finalizing the filter constants.
- **Curve-aware snap rounding** for emitting finite-precision, topologically-consistent output to downstream consumers. Mature for segments, immature for curves; assess whether Keel needs it at the export boundary.
- **Whether indirect predicates can be extended to a bounded class of curved constructions** (for example intersections involving low-degree analytic surfaces: planes, cylinders, spheres, cones, tori). The analytic primitives are where indirect-style exactness might still be reachable, and they cover a large fraction of real CAD models.

## 5. Gaps and Open Problems

1. **No exact paradigm for general NURBS intersections.** Intersection points are high-degree algebraic numbers; certified enclosure is the best available, so irreducible uncertainty must be carried in topology. This is fundamental, not an implementation gap.
2. **Tangential and near-tangential SSI remains hard.** It is the explicit subject of 2023-2025 papers; robustness here is not yet a solved, drop-in capability.
3. **Indirect predicates stop at linear primitives.** Extending the implicit-point + filtered-cascade idea to analytic (degree-2) surfaces is, as far as this review found, not fully developed and is a concrete research opportunity for Keel.
4. **Curve-aware snap rounding is immature** compared to the segment case, leaving the "robust finite-precision output of curved arrangements" problem partly open.
5. **End-to-end formal verification covers predicates/filters, not whole constructions or topology operations.** The correctness of the tolerant-topology gluing logic is still validated empirically, not proven.
6. **Tolerance-propagation theory is underspecified in the open literature.** Commercial kernels (Parasolid, ACIS) implement local tolerance propagation but the algorithms are proprietary; the public literature gives the concept but not a complete, citable recipe, so Keel will be doing original engineering here.

## Primary Sources

- Shewchuk 1997, DCG 18(3): https://link.springer.com/article/10.1007/PL00009321 , https://people.eecs.berkeley.edu/~jrs/papers/robustr.pdf
- Rust `robust` crate: https://github.com/georust/robust
- Attene 2020, CAD 126:102856: https://arxiv.org/abs/2105.09772 , https://github.com/MarcoAttene/Indirect_Predicates
- Cherchi et al., mesh arrangements/booleans: https://www.gianmarcocherchi.com
- Edelsbrunner & Mücke 1990, ACM TOG 9(1): https://dl.acm.org/doi/10.1145/77635.77639
- Meyer & Pion, FPG 2008: https://inria.hal.science/inria-00344297/document
- Bartels 2022, Fast FP filters: https://arxiv.org/pdf/2208.00497
- Yap, EGC paradigm (CGAL summary): https://www.cgal.org/exact.html
- Fabri/Pion lazy exact scheme: https://www.sciencedirect.com/science/article/pii/S016764231000167X
- Hu/Patrikalakis interval SSI: https://www.sciencedirect.com/science/article/abs/pii/S0010448596000991
- Affine arithmetic B-spline SSI (IEEE TVCG 2014): https://dl.acm.org/doi/10.1109/TVCG.2013.237
- Topology-driven SSI (ACM TOG 2023): https://dl.acm.org/doi/abs/10.1145/3592452
- NURBS intersection small loops/tangency (ACM TOG 2025): https://dl.acm.org/doi/10.1145/3807948
- Controlled perturbation (Springer 2010): https://link.springer.com/chapter/10.1007/978-3-642-15582-6_19
- Iterated/Stable/Restore snap rounding: https://dl.acm.org/doi/10.1145/2811256 , https://dl.acm.org/doi/pdf/10.1145/1998196.1998226
- Formally certified FP filters (Boldo/Melquiond/INRIA): https://inria.hal.science/inria-00071232v2/document
- GPredicates (IEEE Access 2019): https://ieeexplore.ieee.org/document/8692354
