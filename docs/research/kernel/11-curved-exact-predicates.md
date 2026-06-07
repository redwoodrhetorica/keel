# Exact and Filtered Geometric Predicates for Curved Geometry

Research dossier for the Keel kernel (open-source Rust B-rep, Parasolid-class ambition).

## Scope and framing

Prior Keel research (files 01 through 10, and the linear-predicate threads) established a solid floor for *linear* exact geometry: Shewchuk adaptive predicates, Cherchi/Attene indirect predicates, and EMBER's plane-based exact booleans. All of those operate on points defined by linear forms (intersections of planes), where the underlying decisions are signs of determinants over rationals. They are fast, well understood, and shippable.

The moment Keel touches a circle, an ellipse, a cylinder, a sphere, a torus, a quadric blend, or a NURBS surface, that floor disappears. The coordinates of an intersection point between two conics are *algebraic numbers of degree up to 4*; between two quadric surfaces the intersection is a degree-4 space curve whose points are degree-8 (or higher) algebraic numbers. Every robustness-critical decision in booleans, arrangements, hidden-line removal, and medial-axis computation reduces to *comparing two algebraic numbers* or *determining the sign of a polynomial evaluated at an algebraic number*. This is the wall. The literature here is an order of magnitude thinner than the linear case, concentrated in a small community (MPI Saarbrücken, INRIA Sophia/Nancy, NYU, Tel Aviv, Groningen) over roughly 1995 to 2015, with EXACUS, QI, and the CGAL Algebraic Kernel as the three landmark engineering efforts.

This file maps that frontier so Keel can make a deliberate, layered choice rather than drifting into either naive floating point (unsound) or full symbolic algebra (unshippable). The synthesis at the end is a concrete per-decision strategy.

Note on terminology: a *one-root number* is an algebraic number specified as a chosen root of a degree-2 (or higher) polynomial with rational coefficients plus an isolating interval. A *separation bound* is a positive lower bound on the absolute value of a nonzero algebraic expression, so that an interval evaluation narrower than the bound proves the sign. These two ideas underpin nearly everything below.

---

## Theme 1: Foundations of exact geometric computation (EGC)

### 1.1 Yap, the EGC paradigm

**Citation.** Yap, C. K. (1997). Towards exact geometric computation. *Computational Geometry: Theory and Applications, 7*(1-2), 3-23. See also Yap, C. K., & Dube, T. (1995). The exact computation paradigm. In D.-Z. Du & F. Hwang (Eds.), *Computing in Euclidean Geometry* (2nd ed., pp. 452-492). World Scientific. And Sharma, V., & Yap, C. K. (2017). Robust geometric computation. In *Handbook of Discrete and Computational Geometry* (3rd ed., Ch. 45). CRC Press.

**Method.** The central thesis of EGC: an algorithm is robust if every *branching decision* (every predicate) it makes is the one the exact-real algorithm would make. This does *not* require exact arithmetic everywhere. It requires that whenever a predicate evaluates a sign, the implementation either (a) certifies the sign via a filter, or (b) falls back to enough precision to guarantee correctness. The decisive technical insight is that a predicate is a *sign computation* on an algebraic expression, and sign computation on a nonzero quantity is decidable in finite precision *given a separation bound*: compute an approximation with error below the bound, and the sign of the approximation is the true sign. The hard residual case is the *zero test* (the expression really is zero), which separation bounds resolve by guaranteeing that an approximation tighter than the bound, yet still bracketing zero, proves equality.

**Performance.** EGC is a correctness paradigm, not an algorithm, so it has no intrinsic cost; cost is dominated by how often filters fail and how expensive the exact fallback is. Yap's surveys report that for well-conditioned inputs, filtered EGC runs within a small constant factor of pure floating point; degenerate or near-degenerate inputs pay the full algebraic cost.

**Limitations.** Separation bounds for high-degree expressions can be astronomically pessimistic (hundreds to thousands of bits), making the exact fallback very slow. EGC says nothing about *which* representation to use; it only defines the contract.

**Kernel relevance.** This is Keel's north star for the curved layer: define every curved predicate as a sign computation, route it through a filter, and have a certified fallback. Adopting EGC as the *contract* (not necessarily the *implementation*) lets Keel reason about correctness uniformly across linear and curved decisions.

### 1.2 Real root isolation and comparison of algebraic numbers

**Citation.** Collins, G. E., & Akritas, A. G. (1976). Polynomial real root isolation using Descartes' rule of signs. In *Proc. SYMSAC* (pp. 272-275). ACM. Plus: Eigenwillig, A., Sharma, V., & Yap, C. K. (2006). Almost tight recursion tree bounds for the Descartes method. In *Proc. ISSAC* (pp. 71-78). And Akritas/continued-fraction methods (Tsigaridas & Emiris).

**Method.** Three classical families isolate the real roots of a univariate polynomial with rational coefficients, each yielding a set of disjoint intervals each holding exactly one root: (1) *Sturm sequences*, which count real roots in an interval via sign-change counts of a polynomial remainder sequence, exact but slow because of coefficient growth in the remainders; (2) the *Descartes method* in the Bernstein/monomial basis, which uses Descartes' rule of signs under interval subdivision, generally the fastest in practice; (3) *continued-fraction* (CF) methods, which compute partial quotients and tend to have excellent bit-complexity. Comparing two algebraic numbers alpha (root of f) and beta (root of g) reduces to isolating the roots of f, g, and a derived polynomial (often gcd or resultant based) and refining the isolating intervals until they separate, at which point endpoint comparison decides the order; if intervals refuse to separate down to the separation bound, the numbers are equal.

**Performance.** Descartes and CF dominate Sturm by large factors in measured ISSAC/SoCG benchmarks; for degree-2 and degree-4 polynomials (the conic regime) the differences are small and all are sub-microsecond on isolated cases, but Sturm's remainder-coefficient blowup hurts when chained.

**Limitations.** Coefficient bit-length grows under the algebraic operations that produce these polynomials (resultants square the degree and roughly multiply bit-length by degree), so naive chaining explodes. Root *refinement* to the separation bound is the cost driver, not isolation.

**Kernel relevance.** Keel needs exactly one robust univariate isolator for the conic/quadric layer. The pragmatic choice is a Bernstein-basis Descartes isolator over exact rationals (it composes naturally with Bezier/B-spline geometry, which is already in Bernstein form), with Sturm available as a slow but simple cross-check oracle during development.

### 1.3 Separation bounds: Canny, BFMS, BFMSS, Li-Yap

**Citation.** Canny, J. (1987). *The Complexity of Robot Motion Planning*. MIT Press (the gap theorem). Burnikel, C., Funke, S., Mehlhorn, K., Schirra, S., & Schmitt, S. (2009). A separation bound for real algebraic expressions. *Algorithmica, 55*(1), 14-28 (the BFMSS bound; conference version ESA 2001). Li, C., & Yap, C. K. (2001). A new constructive root bound for algebraic expressions. In *Proc. SODA* (pp. 496-505). Foundational: Mignotte root bounds.

**Method.** A separation bound certifies that a nonzero radical expression E is bounded away from zero by sep(E) > 0, so an approximation with error < sep(E) settles the sign or proves E = 0. The *constructive* bounds propagate, for each node of the expression DAG, a small tuple of integers and reason about an algebraic conjugate-product (the *measure*) and a degree. BFMSS tracks for each subexpression an upper bound u(E) and a lower bound l(E) on the absolute values of E and its conjugates, plus a degree D = product of the degrees of the radicals (the degree of the extension field). The recursion rules are: for E1 +/- E2, u = u1 l2 + u2 l1 and l = l1 l2; for E1 * E2, u = u1 u2 and l = l1 l2; for E1 / E2, u = u1 l2 and l = l1 u2; for the k-th root, u = u1^(1/k) and l = l1^(1/k) (the radical splits cleanly). The final bound is roughly sep(E) >= 1 / (u(E)^(D-1) * l(E)) for the leading-coefficient-normalized form. Canny's gap theorem is the non-constructive ancestor (a bound from the resultant of the system). Li-Yap improves BFMS in the presence of division by a tighter accounting of conjugates; for division-free expressions BFMSS and Li-Yap coincide, and with division they are incomparable, with BFMSS usually best.

**Performance.** The bounds are cheap to compute (one pass over the DAG). Their *quality* is the issue: for an expression of degree D over n bits, the certified precision can be on the order of D times n bits, which for a degree-8 quadric-intersection comparison over 53-bit inputs means a fallback at hundreds of bits.

**Limitations.** Pessimism. The bound is a worst case over all conjugates and is frequently orders of magnitude smaller than the true separation, so the exact fallback runs at far higher precision than actually needed. Bounds also degrade rapidly with nesting depth of radicals.

**Kernel relevance.** These are the engine of any expression-DAG number type Keel might adopt (see CORE/LEDA below). For the conic layer (degree <= 4, few radicals) the bounds are tolerable; for general quadric and spline algebra they become the reason a pure-EGC approach is too slow, motivating the layered strategy.

### 1.4 Expression-DAG number types: LEDA real, CORE Expr, Yap and Dube

**Citation.** Burnikel, C., Mehlhorn, K., & Schirra, S. (1996). The LEDA class real number (Tech. Rep. MPI-I-96-1-001). MPI Saarbrücken. Karamcheti, V., Li, C., Pechtchanski, I., & Yap, C. (1999). A core library for robust numeric and geometric computation. In *Proc. SoCG* (pp. 351-359) (the CORE library and Expr). Du, Z., & Yap, C. (2006). Uniform complexity of approximating expressions. (CORE 2.) Schmitt, S. (2005). The diamond operator: implementation of exact real algebraic numbers. In *Proc. CASC*.

**Method.** These number types represent a real value as a *directed acyclic graph* of operations over rational (or integer) leaves, with nodes for +, -, *, /, sqrt, and k-th root, and (in LEDA real and the diamond operator) a node for "the i-th real root of a polynomial whose coefficients are themselves expressions". Each value carries a fast floating-point or interval approximation. The sign() and comparison operations first try the interval; if it brackets zero, the type *escalates precision* using a multiprecision approximation, repeatedly doubling precision, until either the sign is clear or the precision exceeds the separation bound (BFMSS/Li-Yap computed over the same DAG), at which point the value is certified zero. The diamond operator extends this to roots of polynomials with algebraic coefficients, which is exactly what conic/quadric intersection coordinates need.

**Performance.** LEDA real and CORE Expr are the de facto reference implementations of EGC for radical expressions. Reported overheads over double are typically 5x to 50x on non-degenerate inputs (dominated by the interval bookkeeping), spiking by 100x to 1000x when separation-bound fallbacks fire. The diamond operator's polynomial-root nodes are markedly slower because each sign query may trigger root isolation and refinement.

**Limitations.** Memory and time grow with DAG size; long geometric pipelines build deep DAGs unless explicitly collapsed. The separation-bound fallback is the worst-case tail. Neither library is natively Rust, and both are GPL/research-licensed.

**Kernel relevance.** This is the single most directly reusable design for Keel's curved number layer. A Rust `AlgebraicReal` as an expression DAG with interval approximation + BFMSS escalation is a known-good architecture. The diamond-operator capability (roots of polynomials with algebraic coefficients) is the precise feature needed for quadric-quadric and curve-surface intersections, and it is also the most expensive, which is why Keel should restrict it to topology decisions, not bulk geometry.

---

## Theme 2: Filtered arithmetic

### 2.1 Fortune and Van Wyk: static filters and the LN compiler

**Citation.** Fortune, S., & Van Wyk, C. J. (1993). Efficient exact arithmetic for computational geometry. In *Proc. SoCG* (pp. 163-172). And: Fortune, S., & Van Wyk, C. J. (1996). Static analysis yields efficient exact integer arithmetic for computational geometry. *ACM Transactions on Graphics, 15*(3), 223-248.

**Method.** LN is an *expression compiler*: given a polynomial predicate expression with integer inputs of bounded bit-length, it emits straight-line C++ that (a) evaluates a floating-point approximation, (b) computes a *static* a-priori error bound from the known input bit-lengths, and (c) compares the approximation against the bound; only if the magnitude is below the bound does it fall back to extended-precision integer arithmetic (radix 2^23 digits stored in doubles). Because the expression and operand sizes are fixed at compile time, all loops, allocations, and function calls are eliminated, and the filter threshold is a compile-time constant.

**Performance.** Roughly an order-of-magnitude speedup over multiprecision libraries at equal bit complexity, plus a further large gain from the floating-point filter on non-degenerate inputs. This paper is the origin of the "filter first, fall back rarely" discipline that every later kernel follows.

**Limitations.** Integer inputs only; the user must bound bit-lengths in advance; the bounds are *static* (worst case over all inputs of that bit-length), so they trigger fallback more often than dynamic filters on benign-looking but large-coefficient inputs. No support for radicals, hence no native curved geometry.

**Kernel relevance.** The architectural lesson transfers directly even though curved predicates are not integer-polynomial: compile each predicate into a filtered fast path with a precomputed error bound and a rare exact fallback. For Keel, a Rust proc-macro or const-generic predicate generator could play LN's role for the conic-coefficient predicates.

### 2.2 Filter theory: Devillers and Pion, Melquiond and Pion, error analysis

**Citation.** Devillers, O., & Pion, S. (2003). Efficient exact geometric predicates for Delaunay triangulations. In *Proc. ALENEX* (pp. 37-44). Melquiond, G., & Pion, S. (2007). Formally certified floating-point filters for homogeneous geometric predicates. *Theoretical Informatics and Applications (RAIRO-ITA), 41*(1), 57-69. Nanevski, Blelloch, & Harper (semi-static filter generation). Background: Brönnimann, Burnikel, & Pion on interval filters.

**Method.** Filters are classified by when their error bound is computed. *Static* filters precompute the bound from input bit/magnitude bounds (fastest, least adaptive). *Semi-static* filters precompute the *form* of the bound symbolically but plug in the actual magnitudes of the inputs at runtime (one cheap pass, much tighter than static). *Dynamic* filters (interval arithmetic) propagate a running error interval through the whole computation (most adaptive, most expensive). The semi-static derivation tracks, per expression, a magnitude bound and an error term in units of the last place (ulp), accumulating epsilon * (sum of magnitude products) so the final comparison is value vs. a runtime-computed epsilon-scaled bound. Melquiond and Pion produced *machine-checked* (Coq) proofs of the floating-point error bounds for the homogeneous Delaunay/orientation filters, eliminating hand-analysis errors.

**Performance.** Semi-static filters succeed on the overwhelming majority of CAD-typical inputs with overhead of roughly 1.5x to 3x over raw double, while catching all the cases a static filter would have to defer. Dynamic interval filters cost more (each operation becomes two rounded ops) but rescue inputs that overflow the semi-static magnitude assumptions.

**Limitations.** All these filters are derived for *polynomial* predicates over floating leaves. Extending the error analysis to expressions containing square roots (conic coordinates) requires bounding the sqrt's relative error, which is doable (sqrt is correctly rounded in IEEE 754) but the bound propagation through subsequent operations is more involved and less studied.

**Kernel relevance.** Keel should generate semi-static filters as the default fast path for every curved predicate, with a dynamic interval filter as the intermediate tier before the exact algebraic fallback. The Coq-certified bounds are a model for how Keel could, at minimum, property-test or ideally machine-check its own filter constants.

### 2.3 Lazy exact evaluation: Benouamer-Michelucci-Peroche, CGAL Lazy_exact_nt and the filtered kernel

**Citation.** Benouamer, M., Jaillon, P., Michelucci, D., & Peroche, B. (1993). A lazy exact arithmetic. In *Proc. 11th IEEE Symposium on Computer Arithmetic* (pp. 242-249). Pion, S., & Fabri, A. (2011). A generic lazy evaluation scheme for exact geometric computations. *Science of Computer Programming, 76*(4), 307-323. CGAL: Brönnimann, Fabri, Giezeman, Hert, Hoffmann, Kettner, Pion, & Schirra, *2D and 3D Linear Geometry Kernel* and *Number Types* manuals (Lazy_exact_nt, Filtered_kernel, EPECK).

**Method.** Lazy exact evaluation defers exact computation until a predicate actually needs it. `Lazy_exact_nt<ET>` is a handle to a DAG node (`Lazy_exact_nt_rep`) with two methods: `.approx()` returns a cached interval, `.exact()` computes the exact value (type ET, e.g. a multiprecision rational or algebraic) *on demand* and memoizes it, collapsing the DAG. Arithmetic only builds DAG nodes and updates intervals; the exact tree is materialized only when an interval-based sign test is inconclusive. CGAL's `Filtered_kernel` and the `Lazy_kernel` (Fabri and Pion) lift this from numbers to whole geometric objects: an exact point is a lazy pair of an interval-approximated point and a deferred exact construction, and predicates are evaluated on the intervals first. EPECK (`Exact_predicates_exact_constructions_kernel`) is the productized combination.

**Performance.** Pion and Fabri report that the lazy scheme keeps exact-construction kernels within a small factor of pure-double for typical inputs because `.exact()` is rarely forced, while remaining fully robust. The memoization is what makes repeated predicates on shared constructed points affordable.

**Limitations.** DAG memory grows with construction depth; long boolean cascades retain large lazy trees until forced or freed. The default CGAL exact types are tuned for linear (rational) geometry; using Lazy_exact_nt over an *algebraic* exact type for conics works but is heavier, and CGAL routes curved geometry through the separate Algebraic Kernel rather than plain Lazy_exact_nt.

**Kernel relevance.** This is the production blueprint Keel should copy almost wholesale: a Rust lazy-exact wrapper carrying `(Interval, OnceCell<Exact>)`, predicates that consult the interval first and force the exact value only on ambiguity, and a kernel-wide policy of exact predicates with lazily-materialized exact constructions. The hard part for Keel is supplying the `Exact` type for curved coordinates (Theme 5).

---

## Theme 3: Predicates on conics and circular arcs

### 3.1 Devillers, Fronville, Mourrain, Teillaud: exact predicates on circle arcs

**Citation.** Devillers, O., Fronville, A., Mourrain, B., & Teillaud, M. (2002). Algebraic methods and arithmetic filtering for exact predicates on circle arcs. *Computational Geometry: Theory and Applications, 22*(1-3), 119-142 (conference version SoCG 2000).

**Method.** The paper isolates the hardest predicate in arrangements of circular arcs: *comparing the x-coordinates (abscissae) of two intersection points*, where each intersection point is a root of a degree-2 equation (the two circles' radical axis substituted back). The coordinates are therefore one-root numbers of the form (a + b sqrt(c)) / d with rational a, b, c, d. The authors derive the comparison as a *sign sequence of the coefficients of a derived polynomial* obtained by a resultant computation, so the decision becomes a small fixed battery of sign tests on rational expressions in the input coefficients rather than an explicit sqrt evaluation. Critically, they pair this with an *arithmetic filter*: evaluate the sign tests in interval/floating arithmetic first, fall back to exact rational arithmetic only when a test is inconclusive.

**Performance.** With the filter, the predicates run at near-floating speed on non-degenerate input; the exact fallback (rational arithmetic on degree-2 derived quantities) is invoked rarely and is cheap because the algebraic degree is only 2. This was one of the first demonstrations that *curved* exact predicates can be made practical.

**Limitations.** Specific to circles (degree-2, and the special structure that the radical axis is linear). The general-conic case (ellipses, hyperbolas in arbitrary position) raises the algebraic degree and loses the linear radical-axis simplification.

**Kernel relevance.** Circles and circular arcs are pervasive in CAD (fillets, holes, cylindrical sections). This paper gives Keel a directly implementable recipe for the circle layer: represent intersection coordinates as one-root numbers, decide comparisons via resultant-derived sign sequences, and filter. It is the cheapest curved exactness Keel can buy, and it should be implemented first.

### 3.2 Wein: high-level filtering for arrangements of conic arcs

**Citation.** Wein, R. (2002). High-level filtering for arrangements of conic arcs. In *Proc. ESA* (LNCS 2461, pp. 884-895). Springer. Realized in CGAL as `Arr_conic_traits_2`.

**Method.** "High-level" filtering operates above the number-type level. Instead of (or in addition to) filtering each arithmetic operation, the traits class *records the construction history and defining equations* of each arrangement vertex (which two conics produced it, and which root). Most vertices are first computed only approximately. When a predicate (compare two vertices, locate a point) is ambiguous on the approximations, the system *refines on demand* using the stored exact equations, and only then invokes the exact algebraic number type. This caches and shares exact work across the many predicate calls a sweep makes on the same vertices.

**Performance.** Wein reports large speedups over a naive exact-number-type implementation precisely because the expensive exact refinement is amortized and triggered only at genuine ambiguities; the bulk of a sweep runs on approximations.

**Limitations.** Requires bookkeeping infrastructure (every geometric object carries provenance), which complicates the kernel. The CGAL conic traits require *rational* conic coefficients, so general algebraic conics must be rationalized or approximated first.

**Kernel relevance.** This is the conic-level analogue of lazy evaluation and is the standard against which Keel's conic arrangement layer should be measured. The "carry the defining equations, refine on demand" pattern integrates cleanly with the lazy-exact wrapper from Theme 2.3.

### 3.3 Berberich, Eigenwillig, Hemmer, Hert, Mehlhorn, Schömer: computational basis for conic arcs and conic-polygon booleans

**Citation.** Berberich, E., Eigenwillig, A., Hemmer, M., Hert, S., Mehlhorn, K., & Schömer, E. (2002). A computational basis for conic arcs and Boolean operations on conic polygons. In *Proc. ESA* (LNCS 2461, pp. 174-186). Springer. (The CONIX library, later folded into EXACUS; built on LEDA.)

**Method.** This defines a complete, exact, efficient geometry kernel for conic arcs and uses it to implement *regularized boolean operations on conic polygons* (regions bounded by linear and conic edges). Points are one-root / two-root algebraic numbers of degree up to 4 (a conic-conic intersection has up to 4 points). The kernel provides the full predicate set: compare two points lexicographically, locate a point relative to a conic arc, compare the y-order of two arcs at a given x, and the sweep events. Conic-conic intersection is handled by reducing to a univariate polynomial (via resultant / pencil analysis), isolating its real roots, and back-substituting; comparisons of the resulting algebraic numbers use isolating-interval refinement and (for equality) Sturm-sequence / gcd reasoning. Booleans are then a Bentley-Ottmann sweep over the conic-arc arrangement with consistent, exact predicates, yielding the overlay from which the regularized boolean is read off.

**Performance.** Reported as complete (handles all degeneracies), exact (mathematically correct), and efficient (practical on real conic-polygon inputs), benchmarked within the LEDA/EXACUS ecosystem. It demonstrated for the first time that full boolean operations on curved (conic) regions could be done exactly and robustly end to end.

**Limitations.** Degree-2 surfaces only in the plane; LEDA-dependent and GPL-era licensed; coefficients effectively rational. The algebraic-number machinery (degree-4 root isolation and comparison) is heavier than the circle-only case of 3.1.

**Kernel relevance.** This is the closest existing artifact to "exact 2D booleans on curved profiles", which Keel needs for sketch geometry, trimming, and 2D arrangement of UV-space curves. It is a direct architectural template for Keel's exact conic boolean layer and a proof that the approach is feasible.

---

## Theme 4: Quadric surfaces, intersections, and 3D arrangements

### 4.1 Dupont, Lazard, Lazard, Petitjean: near-optimal parameterization of quadric intersections (QI)

**Citation.** Dupont, L., Lazard, D., Lazard, S., & Petitjean, S. (2003). Near-optimal parameterization of the intersection of quadrics. In *Proc. SoCG* (pp. 246-255). ACM. Three-part journal version: (2008). *Journal of Symbolic Computation, 43*(3), 168-191 (I: theory), 192-215 (II: classification of pencils), 216-232 (III: parameterizing singular intersections). Implementation: Lazard, S., Peñaranda, L., & Petitjean, S. (2006). Intersecting quadrics: an efficient and exact implementation. *Computational Geometry: Theory and Applications, 35*(1-2), 74-99 (the QI library).

**Method.** Given two quadrics with *rational* coefficients, QI computes an *exact parametric* representation of their intersection curve (in general a degree-4 space curve). It analyzes the *pencil* lambda*Q1 + mu*Q2, finds a quadric in the pencil of minimal rank (which controls the intersection type), and from it derives a parameterization. The headline result: the parameterization is *near-optimal in the number and depth of radicals* introduced, and is *rational whenever the intersection admits a rational parameterization* (i.e. whenever the quartic is singular: a nodal/cuspidal quartic, a cubic-plus-line, two conics, conic-plus-double-line, etc.). When the intersection is a smooth quartic (no rational parameterization exists) it uses a single square root of a polynomial of minimal degree. All connected components and their topology are identified correctly in every case.

**Performance.** The C++ QI library handles arbitrary-size integer coefficients exactly; the SoCG/journal/CGTA reports show it correctly classifying and parameterizing all pencil cases, with the near-optimal radical depth keeping subsequent algebraic-number computations as low-degree as the geometry allows. It is the reference exact quadric-intersection engine.

**Limitations.** Rational (or integer) input coefficients required; irrational quadric placements must be rationalized. The output for the smooth-quartic case still carries a square root, so downstream predicates on the curve are degree-elevated. QI gives the *curve*, not a full 3D arrangement; assembling arrangements still needs predicates on these parameterized curves.

**Kernel relevance.** Quadrics (planes, cylinders, cones, spheres, the natural quadric set plus the torus's quadric pieces) cover a large fraction of CAD faces. QI is the canonical method for getting their intersection curves *exactly* and with minimal algebraic degree, which directly controls the cost of every downstream predicate. If Keel implements exact quadric booleans, the QI pencil-classification approach is the method to port. The "rational whenever possible, single sqrt otherwise" property is a gift: it tells Keel exactly when a quadric intersection stays cheap.

### 4.2 Berberich, Hemmer, Kettner, Schömer, Wolpert: arrangements and cells of quadrics

**Citation.** Berberich, E., Hemmer, M., Kettner, L., Schömer, E., & Wolpert, N. (2005). An exact, complete and efficient implementation for computing planar maps of quadric intersection curves. In *Proc. SoCG* (pp. 99-106). ACM. Related: Mourrain, Técourt, & Teillaud (2005), On the computation of an arrangement of quadrics in 3D, *CGTA*; and Schömer & Wolpert (2006), An exact and efficient approach for computing a cell in an arrangement of quadrics, *CGTA, 33*(1-2), 65-97.

**Method.** To build a 3D arrangement of quadrics, the approach projects: it computes, for one reference quadric, the *planar map* induced on its surface by the intersection curves with all other quadrics. Each such curve is a (generally degree-4) algebraic curve on the reference quadric's parameter plane; the planar map is the 2D arrangement of these projected curves. The 2D curve-arrangement machinery (Theme 5) then provides the exact topology, and stitching the per-quadric maps yields the 3D adjacency graph / cell structure. Predicates reduce to comparing algebraic numbers arising from these projected curves, handled with resultants, subresultants, and exact root isolation.

**Performance.** Reported as exact, complete, and efficient on quadric inputs; these papers established that even 3D arrangements of degree-2 surfaces are tractable exactly, at substantially higher cost than the 2D conic case because the projected curves can reach degree 4 and the algebraic numbers degree 8.

**Limitations.** Heavy machinery; degree-2 surfaces only; rational coefficients; the full 3D arrangement (as opposed to a single cell or adjacency graph) is at the edge of what was demonstrated. Performance is acceptable for analysis but far from interactive-modeling speed.

**Kernel relevance.** This is the state of the art for *exact* 3D solid arrangements of curved surfaces, and it tops out at quadrics. The clear message for Keel: exact 3D booleans on curved surfaces are *possible but expensive* even for quadrics, and the projection-to-planar-map technique is the only demonstrated route. It strongly suggests Keel reserve exact 3D curved arrangements for offline/verification use and rely on certified-tolerant methods for the interactive path.

---

## Theme 5: Algebraic kernels and topology of algebraic curves

### 5.1 EXACUS

**Citation.** Berberich, E., Eigenwillig, A., Hemmer, M., Hert, S., Kettner, L., Mehlhorn, K., Reichel, J., Schmitt, S., Schömer, E., & Wolpert, N. (2005). EXACUS: efficient and exact algorithms for curves and surfaces. In *Proc. ESA* (LNCS 3669, pp. 155-166). Springer.

**Method.** EXACUS is the umbrella C++ library suite consolidating the conic/cubic/algebraic-curve work from MPI Saarbrücken: a number-type layer (NumeriX: integers, rationals, polynomials, algebraic numbers, root isolation, resultants), a curve-analysis layer (SweepX with libraries CONIX for conics, CubiX for cubics, AlciX for arbitrary-degree algebraic curves), and arrangement/boolean layers on top. Its design goals were efficiency, correctness, completeness, clarity, modularity, flexibility, and ease of use. Much of EXACUS was subsequently migrated into CGAL as the Algebraic Kernel and the curved-arrangement traits.

**Performance.** EXACUS demonstrated end-to-end exact arrangements and booleans for conics and cubics at practical speeds, and arbitrary-degree algebraic curves at higher but still feasible cost. Its measured lesson: the cost is dominated by algebraic-number comparison, and aggressive filtering plus shared/cached exact computation (high-level filtering) is what makes it usable.

**Limitations.** Research-grade, C++/LEDA-lineage, not maintained as a standalone product (absorbed into CGAL). Degrees beyond cubic are markedly slower.

**Kernel relevance.** EXACUS is the single richest source of *engineering* lessons for Keel's curved layer: the layering (number type, curve analysis, arrangement), the centrality of resultants/subresultants and root isolation, and the dominance of high-level filtering. Keel should treat EXACUS as its reference architecture for any exact curved subsystem, even while choosing a leaner scope.

### 5.2 Eigenwillig and Kerber: 2D arrangements of arbitrary algebraic curves

**Citation.** Eigenwillig, A., & Kerber, M. (2008). Exact and efficient 2D-arrangements of arbitrary algebraic curves. In *Proc. SODA* (pp. 122-131). Earlier: Eigenwillig, Kerber, & Wolpert (2007), Fast and exact geometric analysis of real algebraic plane curves, *Proc. ISSAC*. And Wolpert et al. on Jacobi curves for arrangement topology.

**Method.** Computes the exact arrangement of segments of arbitrary-degree real algebraic plane curves via a Bentley-Ottmann sweep. The geometric primitives reduce to *cylindrical algebraic decomposition (CAD)* of the plane for one curve (its critical points and vertical-line topology) and for two curves (their intersection structure). The core operations are: project via resultant to get critical x-values, isolate and refine those roots, and at each critical x determine the *vertical arrangement* (the order and multiplicity of points on the fiber) using subresultant sequences and careful handling of vertical asymptotes and singular points. The 2007 ISSAC paper ("analysis") is the per-curve and per-curve-pair engine; the 2008 SODA paper assembles arrangements.

**Performance.** Reported as exact and efficient relative to prior approaches, handling singular and tangential cases completely. Cost scales steeply with curve degree (resultant degree is quadratic in input degree), so conics/cubics are fast and high-degree curves are slow.

**Limitations.** Fully general but heavy; the resultant/subresultant computations are the bottleneck; designed for analysis rather than real-time modeling.

**Kernel relevance.** This is the general-curve fallback Keel would need if it ever wanted exact arrangements of *trimming curves in UV space* of arbitrary algebraic degree. It is over-powered (and too slow) for the common conic/quadric case but is the principled answer for the rare high-degree case, and it defines the CAD-based primitive set Keel would implement.

### 5.3 Bisolve and the CGAL Algebraic Kernel (Ak_2)

**Citation.** Berberich, E., Emeliyanenko, P., & Sagraloff, M. (2011). An elimination method for solving bivariate polynomial systems: eliminating the usual drawbacks. In *Proc. ALENEX* (pp. 35-47) (Bisolve). Emeliyanenko, P., & Sagraloff, M. (2012). On the complexity of solving a bivariate polynomial system. In *Proc. ISSAC* (pp. 154-161). CGAL: Berberich, Hemmer, Kerber, Lazard, Mehlhorn, Teillaud et al., *Algebraic Kernel* package (Algebraic_kernel_d_1, Algebraic_kernel_d_2). Root refinement: Kerber & Sagraloff (2011).

**Method.** Bisolve isolates *all real solutions of a zero-dimensional bivariate system* f = g = 0 (f, g in Z[x,y]) by elimination: it projects solutions onto the x- and y-axes via resultants, isolates the projected roots, forms the candidate grid of boxes, and then *certifies* which boxes contain a true common solution (using sign/multiplicity tests), with no genericity assumption and no coordinate change. For f, g of total degree <= n and coefficient bit-length <= tau, it runs in O~(n^8 tau^2) bit operations, improving the prior bound by at least n^2. The CGAL bivariate Algebraic Kernel (Ak_2) exposes exactly the operations curved geometry needs: isolate real roots of a bivariate system, compare and refine algebraic coordinates, sign-evaluate a polynomial at an algebraic point, and analyze a curve's topology, all built on a univariate kernel (Ak_1) providing root isolation, comparison, and sign-at.

**Performance.** The CGAL implementation of Bisolve outperformed prior bivariate solvers by a large factor in the ALENEX experiments. The Algebraic Kernel is the productized, maintained descendant of EXACUS and is the recommended foundation for curved arrangements in current CGAL.

**Limitations.** Integer/rational input; bit-complexity, while improved, is still high-degree polynomial, so high curve degrees are slow; building and refining resultants dominates. The kernel is C++ and GPL/commercial-dual licensed.

**Kernel relevance.** The CGAL Algebraic Kernel's *interface* (Ak_1: isolate, compare, refine, sign_at; Ak_2: solve, x/y-compare, topology) is the exact API Keel should specify for its own curved algebraic layer in Rust, whether implemented natively or initially via FFI. Bisolve is the algorithm to implement behind `solve_bivariate`. This is the most concrete, modern, reusable specification in the entire dossier.

---

## Theme 6: Splines and NURBS predicates

### 6.1 The certified-numeric vs exact-symbolic gap for splines

**Summary (synthesizing the literature, no single canonical paper).** For B-spline and NURBS geometry there is *no* widely used exact-symbolic predicate stack analogous to EXACUS for conics. The reasons are structural: (a) NURBS surfaces are piecewise *rational* of arbitrary degree, so an intersection point's coordinates are algebraic of very high degree; (b) the defining data (knots, control points, weights) are floating-point in every real CAD file, so "exact" would require first rationalizing inputs that were never rational; (c) the resultant/subresultant cost that is merely heavy for conics becomes prohibitive at NURBS degrees. Consequently the field uses *certified numerics*: interval/subdivision methods that produce results with *proven* topological correctness (isotopy) or *proven* error bounds, rather than exact algebraic decisions.

### 6.2 Root isolation in the Bernstein basis with exact rational arithmetic

**Citation.** Mourrain, B., & Pavone, J.-P. (2009). Subdivision methods for solving polynomial equations. *Journal of Symbolic Computation, 44*(3), 292-306. Background: Lane & Riesenfeld (1981) on Bernstein subdivision; Spencer/Rockwood on Bezier clipping.

**Method.** Because Bezier and B-spline curves are already in the Bernstein basis, real root isolation can be done directly via the variation-diminishing property: the number of real roots of a Bernstein-form polynomial in [0,1] is bounded by the sign changes of its coefficients, and de Casteljau subdivision refines until each subinterval has at most one root. Done over *exact rational* control coefficients (when the spline data is rational), this is an exact Descartes-style isolator native to the spline representation; done over intervals, it is a certified numeric isolator. This is the cheapest, most spline-friendly exactness available, and it underlies Bezier-clipping intersection.

**Performance.** Subdivision isolators are fast and numerically friendly; exact-rational variants pay the usual rational-arithmetic growth but compose naturally with spline refinement.

**Limitations.** Exactness requires rational input; surface-surface intersection is a *bivariate* problem where simple subdivision gives boxes, not exact algebraic points, so isolated equality/ordering decisions are not resolved exactly.

**Kernel relevance.** Keel should use Bernstein-basis subdivision as its spline univariate isolator. For 1D decisions on rational splines it can be exact; for the general case it provides certified enclosures that feed the tolerant layer.

### 6.3 Plantinga-Vegter isotopic certified subdivision

**Citation.** Plantinga, S., & Vegter, G. (2004). Isotopic approximation of implicit curves and surfaces. In *Proc. Symposium on Geometry Processing* (pp. 245-254). And: Plantinga, S., & Vegter, G. (2007). Isotopic meshing of implicit surfaces. *The Visual Computer, 23*(1), 45-58. Extensions: Burr, Choi, Galehouse, & Yap (2008) on adaptive isotopic curves; Lin & Yap on the "PV" complexity.

**Method.** Given a smooth implicit curve f = 0 in R^2 (or surface in R^3), the algorithm builds a balanced quadtree/octree and, in each cell, uses *interval arithmetic* on f and its gradient to certify two predicates: C0 (f does not vanish, so the cell is empty) and C1 (the gradient does not vanish in any single component direction, so the curve crosses the cell simply). Cells failing either test are subdivided. Once every non-empty cell passes C1, a piecewise-linear mesh built from the cell crossings is *guaranteed isotopic* to the true curve/surface (topologically equivalent under ambient deformation), even though the vertex *positions* are only approximate.

**Performance.** Practical and widely cited; cost depends on feature size (cells must shrink below the local feature size near tight features and curvature). It is the standard certified-topology mesher for implicit and (via implicitization or evaluation) parametric curved geometry.

**Limitations.** Requires *smoothness* and bounded geometry; the basic version fails at singular points (where the gradient vanishes), exactly the degenerate cases booleans care about; near-tangencies force deep subdivision. It certifies *topology*, not exact coordinates, so it does not by itself answer ordering/equality predicates between two features.

**Kernel relevance.** Plantinga-Vegter (and its singular-aware successors, e.g. the Cxy/marching-cube-with-interval-tests family) is the realistic certified method for *NURBS-era* geometry where exactness is unaffordable. Keel can use it to get provably correct *topology* of intersection curves and trimmed regions, then attach tolerances for the metric data. It is the bridge between "certified" and "tolerant".

### 6.4 Validated continuation and certified intersection curves

**Citation.** Representative: Martin, Shou, Voiculescu, Bowyer, & Wang (2002). Comparison of interval methods for plotting algebraic curves. *Computer Aided Geometric Design, 19*(7), 553-587. Plus interval-Newton / Krawczyk continuation (Kearfott; Neumaier) applied to surface-surface intersection (SSI).

**Method.** Surface-surface intersection curves are traced by *validated continuation*: from a certified starting point, an interval-Newton or Krawczyk operator proves existence and uniqueness of the curve point in a box (the Krawczyk/interval-Newton contraction certifies a solution exists and is unique), and the tracer steps along the curve with each step certified, detecting and resolving turning points and bifurcations via the interval Jacobian. This yields a polyline with *guaranteed* per-vertex error bounds and guaranteed no missed components within the searched region.

**Performance.** Robust and reasonably fast for well-separated curves; cost spikes near tangential or singular intersections where the Jacobian is ill-conditioned and boxes must shrink.

**Limitations.** Does not give exact algebraic points; near-tangencies (the robustness-critical cases) are exactly where the certificates become expensive or require special handling; global completeness (finding all components) needs a separate certified search.

**Kernel relevance.** This is the practical engine for Keel's NURBS SSI: certified enclosures and certified topology rather than exact decisions, with tolerances mediating the final snap. It pairs naturally with Plantinga-Vegter for topology and with the tolerant layer for metric resolution.

---

## Theme 7: Controlled and perturbation-based approaches

### 7.1 Simulation of Simplicity (SoS)

**Citation.** Edelsbrunner, H., & Mücke, E. P. (1990). Simulation of simplicity: a technique to cope with degenerate cases in geometric algorithms. *ACM Transactions on Graphics, 9*(1), 66-104.

**Method.** SoS removes *degeneracies* (not numeric error) by symbolically perturbing each input coordinate by distinct infinitesimals epsilon^(i*2^j), so that no predicate ever returns exactly zero. The sign of a perturbed predicate is read off the first nonvanishing term of its expansion in epsilon, which is a determinant minor of the original data. The algorithm then runs as if the input were in general position.

**Performance.** Cheap in the linear case (the perturbation terms are existing minors). It is purely a degeneracy-handling layer, orthogonal to the exact/filtered question.

**Limitations.** For *curved* predicates the expansion of a perturbed algebraic expression in the infinitesimals is far less clean than for determinants; deriving the leading nonzero term of a perturbed conic/quadric sign test is not routine and is sparsely studied. SoS also changes the answer to a *nearby* degenerate-free problem, which may be undesirable when the degeneracy is intentional (coincident faces in CAD).

**Kernel relevance.** SoS is attractive for Keel's linear degeneracies and possibly for the conic layer if the leading-term expansions are worked out, but it is not a ready solution for general curved degeneracies. Treat it as a tool for the linear/conic tiers, not the spline tier.

### 7.2 Controlled perturbation

**Citation.** Halperin, D., & Shelton, C. R. (1998). A perturbation scheme for spherical arrangements with application to molecular modeling. *Computational Geometry: Theory and Applications, 10*(4), 273-287. Halperin & Leiserowitz (2004). Controlled perturbation for arrangements of circles. *International Journal of Computational Geometry & Applications, 14*(4-5), 277-310. Mehlhorn, Osbild, & Sagraloff (2006). Reliable and efficient computational geometry via controlled perturbation. In *Proc. ICALP* (LNCS 4051, pp. 299-310). General analysis: Mehlhorn, Osbild, & Sagraloff (2011).

**Method.** Controlled perturbation solves the problem on a *nearby* input that is provably non-degenerate *with respect to the fixed precision in use*. Each input is randomly perturbed within a radius delta; an analysis of the predicate's *guard* (how far from zero a non-degenerate predicate must be so that fixed-precision floating evaluation is certainly correct) determines the perturbation magnitude needed so that, with high probability, every predicate's value exceeds its guard. The algorithm then runs entirely in fast fixed-precision floating point, with a certificate that all decisions were correct on the perturbed input. Halperin et al. worked out the guards and perturbation analyses specifically for arrangements of *spheres* and *circles*.

**Performance.** Near-floating-point speed (the whole point), with a modest precision/perturbation overhead. Mehlhorn-Osbild-Sagraloff give a general framework to derive the required perturbation and precision for a broad class of predicates.

**Limitations.** It changes the input by delta, so the output is correct for a *perturbed* model, not the exact one; for CAD this means metric error on the order of delta and possible loss of intended exact coincidences. Deriving guards for high-degree spline predicates is hard. It trades exactness of *the given problem* for robustness and speed.

**Kernel relevance.** Controlled perturbation is the best-analyzed *principled* alternative to exactness for curved arrangements (and it is the only one with worked-out guards for circles and spheres, both core CAD primitives). For Keel it is a credible strategy for spherical/cylindrical arrangement subproblems where a tiny, certified perturbation is acceptable, and it degrades gracefully to the spline case as a heuristic.

---

## Theme 8: Snap rounding and finite-precision geometry for curves

**Citation.** Goodrich, Guibas, Hershberger, & Tanenbaum (1997). Snap rounding line segments efficiently in two and three dimensions. In *Proc. SoCG* (pp. 284-293). Halperin & Packer (2002). Iterated snap rounding. *Computational Geometry: Theory and Applications, 23*(2), 209-225. Hershberger (2013). Stable snap rounding. *Computational Geometry: Theory and Applications, 46*(4), 403-416. Packer (2008). Iterated snap rounding with bounded drift.

**Method.** Snap rounding converts an arbitrary-precision arrangement of segments into a fixed-precision one: place a grid of pixels centered at representable points, mark "hot" pixels (those containing a vertex/intersection), and reroute every edge through the centers of the hot pixels it passes through. *Iterated* snap rounding repeats so that each vertex is at least half a pixel from any non-incident edge (removing the degenerate near-coincidences plain snap rounding leaves). *Stable* snap rounding (Hershberger) adds idempotence: re-snapping a snapped arrangement leaves it unchanged.

**Performance.** Efficient and practical for *line-segment* arrangements; it is the standard way to land an exact 2D overlay into fixed precision for storage/interchange.

**Limitations.** The theory and implementations target *straight* segments. Curved snap rounding is essentially open: snapping a curve's vertices changes the curve, and there is no settled definition of a fixed-precision rounded arrangement of conics/splines that preserves topology and stays idempotent. Attempts exist only in fragments. This is a genuine gap.

**Kernel relevance.** Keel will face the same problem any kernel does: exact internal computation must eventually be serialized to finite-precision geometry for interchange (STEP, Parasolid XT). Snap rounding is the right *concept* for the linear parts, but for curved geometry Keel will likely have to round to tolerant geometry (Theme 9) rather than to a clean curved snap-rounded arrangement, because the curved theory does not exist. This argues for a tolerant *boundary* representation even if the *internal* decisions are exact.

---

## Theme 9: Industry compromises and academic critiques

### 9.1 Parasolid and ACIS: tolerant modeling

**Citation.** Spatial Corp. *ACIS R10 Kernel Documentation, Ch. 6: Tolerant Modeling.* Siemens / Spatial product documentation on Parasolid adaptive tolerance. Background and history: novedge.com, "ROMULUS to Parasolid and ACIS"; Spatial, "What is Parasolid"; Wikipedia, "Geometric modeling kernel".

**Method.** Commercial kernels chose *tolerance-based* geometry, not exact arithmetic. Each entity has an associated tolerance; ACIS attaches per-entity tolerance values to *tolerant edges* and *tolerant vertices* when ordinary "precise" geometry cannot represent a join within system precision. Two faces whose surfaces do not actually intersect within machine precision are still stitched into valid topology by widening the shared edge's tolerance until the geometries are "close enough". Parasolid uses kernel-managed adaptive tolerance to similar effect. The *topology* is treated as authoritative; the *geometry* is allowed to be approximate within the recorded tolerance band.

**Performance.** This is why commercial kernels are fast and ship: every predicate is a tolerant floating comparison, with no algebraic-number machinery. The cost is paid in correctness edge cases, not in per-operation time. Spatial's own documentation frames it as "precision at the cost of speed, or speed at the cost of precision".

**Limitations.** Tolerant modeling is *not robust* in the EGC sense: tolerance choices can cascade (an operation's output tolerance feeds the next operation), near-tangent and near-coincident configurations can flip decisions, and "sliver" and "leaky-solid" failures are the well-known result. The topology can become inconsistent with any single geometric interpretation. Decades of CAD bug reports trace to exactly this.

**Kernel relevance.** This is the incumbent Keel is positioning against. The lesson is double-edged: tolerances are why the incumbents are fast and complete, *and* they are the source of the robustness failures Keel wants to fix. Keel cannot simply copy tolerant modeling (that abandons its differentiator) nor go fully exact for splines (unshippable). The resolution is the hybrid below.

### 9.2 Hybrid proposals and critiques: exact topology with tolerant geometry

**Citation.** Synthesis of: Fortune, S. (1997). Polyhedral modelling with exact arithmetic / with multiprecision arithmetic (various). Hoffmann, C. M. (1989). *Geometric and Solid Modeling*. Morgan Kaufmann (the classic statement of the robustness problem). Sugihara & Iri on topology-oriented robust methods. Shewchuk's robustness notes. Plus the EGC-vs-tolerance debate threaded through Yap's surveys and CGAL's design papers.

**Method.** The recurring hybrid proposal: make the *combinatorial structure* (the topology: which faces meet which edges) decided by *exact/certified* predicates so it is always globally consistent, while letting the *metric geometry* (the surface equations, vertex positions) be approximate but tagged with rigorous error bounds. Topology-oriented methods (Sugihara) go further: enforce topological consistency as the invariant and reject any numeric result that would violate it. The academic critique of pure tolerance is that it has no consistency guarantee; the critique of pure exactness is unaffordable separation bounds at spline degree.

**Performance.** No single shipped system fully realizes exact-topology-plus-tolerant-geometry for NURBS, which is itself the finding: the hybrid is the consensus *direction* but remains largely unrealized in production, which is the opportunity Keel is chasing.

**Limitations.** The hard interface is keeping exact topology and tolerant geometry mutually consistent: an exactly-decided edge must correspond to a geometry that actually (within tolerance) exists, or downstream evaluation diverges from the topology. Reconciling the two is the open engineering problem.

**Kernel relevance.** This is the intellectual core of Keel's positioning and the synthesis below builds directly on it.

### 9.3 Cost analyses and where filters fail

**Citation.** Synthesis of measured results across Devillers-Pion (linear), Wein (conic), Pion-Fabri (lazy kernel), and the EXACUS/QI/CGAL benchmarks cited above.

**Findings.** (1) On *non-degenerate* inputs, well-built filters (semi-static, then interval, then lazy-exact) keep curved exact predicates within a small constant factor (often 2x to 10x) of floating point, because the exact fallback almost never fires. (2) The cost is concentrated in *degeneracies and near-degeneracies*, where filters fail and the algebraic fallback (root isolation, refinement to a separation bound, degree-4 to degree-8 algebraic-number comparison) costs 100x to 1000x. (3) Crucially, *CAD models are degeneracy-dense*: deliberate tangencies (fillets meeting faces tangentially), coincident faces, axis-aligned features, and exact symmetries are pervasive and intentional, so filters fail far more often on real CAD geometry than on random inputs. (4) Higher algebraic degree multiplies both the filter-failure rate and the fallback cost, which is why the cost wall rises sharply from circles (deg 2) to general conics (deg 4) to quadric intersections (deg 8) to NURBS (very high).

**Kernel relevance.** This is the decisive cost reality for Keel: the curved exact path is cheap on average and ruinous exactly in the configurations CAD users create on purpose. That asymmetry, more than any single algorithm, dictates the layered strategy.

---

## Predicate layer strategy for Keel

The literature converges on a clear, defensible architecture. Keel should *not* pick one paradigm; it should layer four tiers and route each decision to the cheapest tier that is sound for that decision's algebraic degree and role.

**Tier 0: Linear, exact (already chosen).** Keep the Shewchuk/indirect/EMBER linear exact predicates for all decisions reducible to signs of determinants over rational/plane data. This is settled and fast.

**Tier 1: Low-degree curved, exact via one-root numbers + filters (build this next).** For *circles and circular arcs* (deg 2) and *general conics* (deg up to 4), implement the EXACUS/CONIX + Devillers-Fronville-Mourrain-Teillaud recipe: represent intersection coordinates as one-root algebraic numbers, decide comparisons via resultant-derived sign sequences, and wrap everything in a lazy-exact number type (CGAL `Lazy_exact_nt`-style: interval first, exact-on-demand, memoized) with semi-static filters as the fast path. Exactness here is genuinely affordable, and circles/conics are extremely common (holes, fillets, sketch arcs, cylindrical and conical faces in section). This tier buys Keel real, differentiating robustness at acceptable cost. Add Wein-style high-level filtering (carry defining equations, refine on demand) for conic *arrangements*.

**Tier 2: Quadrics, exact-when-cheap / certified-otherwise.** For *quadric surfaces* (planes, spheres, cylinders, cones, and torus quadric pieces), use QI-style pencil analysis for intersection curves. Exploit QI's key property: the intersection is *rational whenever it is singular* and needs only a *single square root* when it is a smooth quartic. So make the decision data-driven: when QI yields a rational or low-radical parameterization, run exact predicates (the algebraic degree stays manageable); when it yields the smooth-quartic single-sqrt case and a predicate is degeneracy-critical, escalate to the algebraic kernel; otherwise use certified intervals. Full *exact 3D arrangements* of quadrics (Berberich et al.) are too slow for the interactive path: reserve them for an offline verification/validation mode, not the live modeler.

**Tier 3: NURBS and high-degree, certified-numeric topology + tolerant geometry (the only practical answer).** For splines and any high-degree algebraic geometry, exactness is unaffordable (no separation-bound budget, and inputs are floating to begin with). Use the certified-numeric stack: Bernstein-basis subdivision isolation (exact when the spline is rational, certified-interval otherwise), Plantinga-Vegter-style certified subdivision for *topology* of intersection curves and trimmed regions, and validated (interval-Newton/Krawczyk) continuation for SSI with per-vertex error bounds. Decide the *combinatorial topology* with these certificates so it is globally consistent; represent the *metric geometry* tolerantly with rigorous, propagated error bands. This is the exact-topology-plus-tolerant-geometry hybrid the field endorses but no shipped kernel fully delivers, which is Keel's opening.

**Cross-cutting mechanisms.**
- *Number type.* Implement one Rust `AlgebraicReal` as an expression DAG with interval approximation and BFMSS/Li-Yap separation-bound escalation (LEDA-real / CORE-Expr architecture, including a diamond-operator capability for roots of polynomials with algebraic coefficients, restricted to Tier 1-2 topology decisions). This single type serves Tiers 1 and 2.
- *Algebraic kernel API.* Specify Keel's curved algebraic layer to the CGAL Algebraic Kernel interface (Ak_1: isolate / compare / refine / sign_at; Ak_2: solve_bivariate via Bisolve, x/y-compare, curve topology). Implement natively in Rust over time; FFI to CGAL is an acceptable bootstrap for validation.
- *Filtering discipline.* Every curved predicate gets the same cascade: semi-static filter, then dynamic interval filter, then lazy-exact / algebraic fallback. Generate the filters (a Rust proc-macro in the spirit of Fortune-Van Wyk's LN) and property-test (ideally machine-check) the error constants.
- *Degeneracy handling.* Use Simulation of Simplicity for Tier 0 and (where leading-term expansions are tractable) Tier 1 degeneracies. Keep controlled perturbation (Halperin's circle/sphere guards) as the principled fixed-precision option for spherical/cylindrical arrangement subproblems where a certified delta-perturbation is acceptable. Do *not* rely on uncontrolled tolerances for *topological* decisions anywhere.
- *Serialization.* Accept that curved snap rounding is an open problem; round to a tolerant boundary representation (with recorded tolerances) for interchange, while keeping internal topology exact/certified. This isolates the unsolved curved-snap-rounding gap to the I/O boundary.

**One-line summary.** Exactness is affordable and worth it for circles, conics, and the cheap (rational / single-sqrt) quadric cases; certified intervals (subdivision + validated continuation + Plantinga-Vegter topology) are the right tool for the expensive quadric cases and all of NURBS; tolerances remain unavoidable only for the *metric* geometry of high-degree surfaces and for finite-precision serialization, never for the combinatorial topology, which Keel should always decide exactly or with a certificate.

---

## References

1. Yap, C. K. (1997). Towards exact geometric computation. *Computational Geometry: Theory and Applications, 7*(1-2), 3-23.
2. Yap, C. K., & Dube, T. (1995). The exact computation paradigm. In *Computing in Euclidean Geometry* (2nd ed.). World Scientific.
3. Sharma, V., & Yap, C. K. (2017). Robust geometric computation. In *Handbook of Discrete and Computational Geometry* (3rd ed., Ch. 45). CRC Press.
4. Collins, G. E., & Akritas, A. G. (1976). Polynomial real root isolation using Descartes' rule of signs. In *Proc. SYMSAC* (pp. 272-275). ACM.
5. Eigenwillig, A., Sharma, V., & Yap, C. K. (2006). Almost tight recursion tree bounds for the Descartes method. In *Proc. ISSAC* (pp. 71-78). ACM.
6. Canny, J. (1987). *The Complexity of Robot Motion Planning*. MIT Press.
7. Burnikel, C., Funke, S., Mehlhorn, K., Schirra, S., & Schmitt, S. (2009). A separation bound for real algebraic expressions. *Algorithmica, 55*(1), 14-28. (Conf. version ESA 2001.)
8. Li, C., & Yap, C. K. (2001). A new constructive root bound for algebraic expressions. In *Proc. SODA* (pp. 496-505).
9. Burnikel, C., Mehlhorn, K., & Schirra, S. (1996). The LEDA class real number (Tech. Rep. MPI-I-96-1-001). MPI Saarbrücken.
10. Karamcheti, V., Li, C., Pechtchanski, I., & Yap, C. (1999). A core library for robust numeric and geometric computation. In *Proc. SoCG* (pp. 351-359). ACM.
11. Schmitt, S. (2005). The diamond operator: implementation of exact real algebraic numbers. In *Proc. CASC* (LNCS 3718). Springer.
12. Fortune, S., & Van Wyk, C. J. (1996). Static analysis yields efficient exact integer arithmetic for computational geometry. *ACM Transactions on Graphics, 15*(3), 223-248. (Conf. version SoCG 1993.)
13. Devillers, O., & Pion, S. (2003). Efficient exact geometric predicates for Delaunay triangulations. In *Proc. ALENEX* (pp. 37-44).
14. Melquiond, G., & Pion, S. (2007). Formally certified floating-point filters for homogeneous geometric predicates. *RAIRO-ITA, 41*(1), 57-69.
15. Benouamer, M., Jaillon, P., Michelucci, D., & Peroche, B. (1993). A lazy exact arithmetic. In *Proc. 11th IEEE Symposium on Computer Arithmetic* (pp. 242-249).
16. Pion, S., & Fabri, A. (2011). A generic lazy evaluation scheme for exact geometric computations. *Science of Computer Programming, 76*(4), 307-323.
17. Devillers, O., Fronville, A., Mourrain, B., & Teillaud, M. (2002). Algebraic methods and arithmetic filtering for exact predicates on circle arcs. *Computational Geometry: Theory and Applications, 22*(1-3), 119-142. (Conf. version SoCG 2000.)
18. Wein, R. (2002). High-level filtering for arrangements of conic arcs. In *Proc. ESA* (LNCS 2461, pp. 884-895). Springer.
19. Berberich, E., Eigenwillig, A., Hemmer, M., Hert, S., Mehlhorn, K., & Schömer, E. (2002). A computational basis for conic arcs and Boolean operations on conic polygons. In *Proc. ESA* (LNCS 2461, pp. 174-186). Springer.
20. Dupont, L., Lazard, D., Lazard, S., & Petitjean, S. (2008). Near-optimal parameterization of the intersection of quadrics (I, II, III). *Journal of Symbolic Computation, 43*(3), 168-232. (Conf. version SoCG 2003, pp. 246-255.)
21. Lazard, S., Peñaranda, L., & Petitjean, S. (2006). Intersecting quadrics: an efficient and exact implementation. *Computational Geometry: Theory and Applications, 35*(1-2), 74-99. (The QI library.)
22. Berberich, E., Hemmer, M., Kettner, L., Schömer, E., & Wolpert, N. (2005). An exact, complete and efficient implementation for computing planar maps of quadric intersection curves. In *Proc. SoCG* (pp. 99-106). ACM.
23. Schömer, E., & Wolpert, N. (2006). An exact and efficient approach for computing a cell in an arrangement of quadrics. *Computational Geometry: Theory and Applications, 33*(1-2), 65-97.
24. Berberich, E., Eigenwillig, A., Hemmer, M., Hert, S., Kettner, L., Mehlhorn, K., Reichel, J., Schmitt, S., Schömer, E., & Wolpert, N. (2005). EXACUS: efficient and exact algorithms for curves and surfaces. In *Proc. ESA* (LNCS 3669, pp. 155-166). Springer.
25. Eigenwillig, A., Kerber, M., & Wolpert, N. (2007). Fast and exact geometric analysis of real algebraic plane curves. In *Proc. ISSAC* (pp. 151-158). ACM.
26. Eigenwillig, A., & Kerber, M. (2008). Exact and efficient 2D-arrangements of arbitrary algebraic curves. In *Proc. SODA* (pp. 122-131).
27. Berberich, E., Emeliyanenko, P., & Sagraloff, M. (2011). An elimination method for solving bivariate polynomial systems (Bisolve). In *Proc. ALENEX* (pp. 35-47). SIAM.
28. Emeliyanenko, P., & Sagraloff, M. (2012). On the complexity of solving a bivariate polynomial system. In *Proc. ISSAC* (pp. 154-161). ACM.
29. Mourrain, B., & Pavone, J.-P. (2009). Subdivision methods for solving polynomial equations. *Journal of Symbolic Computation, 44*(3), 292-306.
30. Plantinga, S., & Vegter, G. (2004). Isotopic approximation of implicit curves and surfaces. In *Proc. Symposium on Geometry Processing* (pp. 245-254).
31. Plantinga, S., & Vegter, G. (2007). Isotopic meshing of implicit surfaces. *The Visual Computer, 23*(1), 45-58.
32. Martin, R., Shou, H., Voiculescu, I., Bowyer, A., & Wang, G. (2002). Comparison of interval methods for plotting algebraic curves. *Computer Aided Geometric Design, 19*(7), 553-587.
33. Edelsbrunner, H., & Mücke, E. P. (1990). Simulation of simplicity. *ACM Transactions on Graphics, 9*(1), 66-104.
34. Halperin, D., & Shelton, C. R. (1998). A perturbation scheme for spherical arrangements with application to molecular modeling. *Computational Geometry: Theory and Applications, 10*(4), 273-287.
35. Halperin, D., & Leiserowitz, E. (2004). Controlled perturbation for arrangements of circles. *International Journal of Computational Geometry & Applications, 14*(4-5), 277-310.
36. Mehlhorn, K., Osbild, R., & Sagraloff, M. (2006). Reliable and efficient computational geometry via controlled perturbation. In *Proc. ICALP* (LNCS 4051, pp. 299-310). Springer.
37. Goodrich, M. T., Guibas, L. J., Hershberger, J., & Tanenbaum, P. J. (1997). Snap rounding line segments efficiently in two and three dimensions. In *Proc. SoCG* (pp. 284-293). ACM.
38. Halperin, D., & Packer, E. (2002). Iterated snap rounding. *Computational Geometry: Theory and Applications, 23*(2), 209-225.
39. Hershberger, J. (2013). Stable snap rounding. *Computational Geometry: Theory and Applications, 46*(4), 403-416.
40. Hoffmann, C. M. (1989). *Geometric and Solid Modeling: An Introduction*. Morgan Kaufmann.
41. Spatial Corp. ACIS R10 Kernel Documentation, Ch. 6: Tolerant Modeling. (And Siemens/Spatial Parasolid adaptive-tolerance documentation.)
42. CGAL Editorial Board. *CGAL User and Reference Manual*: Number Types (Lazy_exact_nt), 2D/3D Linear Geometry Kernel (Filtered_kernel, EPECK), Algebraic Kernel, 2D Arrangements (Arr_conic_traits_2), 2D Circular Geometry Kernel.
