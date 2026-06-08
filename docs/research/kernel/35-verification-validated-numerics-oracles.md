# 35. Verification, Validated Numerics, and Correctness Oracles

Research dossier supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition. Milestone context: M7b.

## Scope and framing

The companion file (kernel/34) covers the *input* half of the testing problem: how to generate breaking inputs and curate a corpus that exercises the kernel's failure modes. This file covers the *output* half, which is harder and less obvious: **given a result the kernel produced, how do you decide it is correct, and how do you certify it?**

This is the **oracle problem**. In most software you can write down the expected output for a test input and compare. In a geometry kernel you usually cannot. There is no closed-form "correct boolean of these two NURBS solids" to compare against. The intersection curve of two trimmed splines is an algebraic object no human will hand-author, the volume of a filleted casting has no reference value, and even when a reference exists (say, OCCT's answer) that reference is itself a buggy program. The kernel must therefore be verified largely **without an oracle**, by exploiting properties the answer must satisfy regardless of what the answer is.

This dossier surveys the strategies that make that possible, organized as an **oracle stack** from cheapest to most expensive:

1. **Invariant / self-consistency oracles** (kernel/01 topology validator run as a continuous check): the result must be a valid B-rep, full stop, whatever shape it is.
2. **Metamorphic oracles**: the result must relate correctly to *other* results (boolean algebra laws, transform invariance, refinement invariance, mass-property identities). No ground truth needed, only relations between runs.
3. **Differential oracles**: the result must agree (within tolerance) with one or more independent reference kernels, used as a voting jury.
4. **Exact oracles for the easy cases**: for inputs cheap enough to compute exactly (linear, low-degree), an exact-arithmetic engine produces trusted ground truth to validate the fast f64 path.
5. **Validated numerics**: rather than hoping the f64 answer is right, carry a **rigorous error enclosure** (interval/affine arithmetic, interval Newton/Krawczyk, validated ODE marching, Taylor models) so the answer arrives with a proof of its own accuracy.
6. **Formal verification**: machine-checked proofs of the small, load-bearing pieces (predicates, hull/arrangement kernels), honest about how far this reaches.

Cross-cutting: **reproducibility as a correctness property** (kernel/14 determinism), and **best-effort certification** (the Qi-Shapiro epsilon-solidity contract from kernel/29, turning "we salvaged this" into a reported, checkable quality measure).

This file deliberately does **not** re-derive the predicate machinery (kernel/11 exact and filtered predicates) or the determinism/serialization mechanics (kernel/14). It builds on both: predicates are the *units* that validated numerics and formal proof certify; determinism is the *precondition* that makes differential and metamorphic comparison meaningful.

Each significant source gets a Citation / Method / Limitations / Kernel relevance entry.

---

# PART 1: The oracle problem in geometry

## 1.1 Why geometry is "untestable" in the classical sense

A test oracle is a mechanism that decides whether a program's output for a given input is correct. Geometry kernels are a textbook example of software where the oracle is expensive or nonexistent: the correct output of a NURBS-NURBS boolean is not something a tester can write down. This is exactly the class of problem that motivated metamorphic testing, and the framing below comes from that literature.

### Source: Segura, Fraser, Sanchez, Ruiz-Cortes, "A Survey on Metamorphic Testing"

**Citation.** Segura, S., Fraser, G., Sanchez, A. B., & Ruiz-Cortes, A. (2016). A Survey on Metamorphic Testing. *IEEE Transactions on Software Engineering, 42*(9), 805-824. https://eprints.whiterose.ac.uk/id/eprint/110335/1/segura16-tse.pdf

**Method.** The survey is the canonical reference for the field. Metamorphic testing (MT), introduced by T. Y. Chen in a 1998 technical report, sidesteps the oracle problem by checking **metamorphic relations (MRs)**: necessary properties relating the inputs and outputs of *multiple* executions of the program, rather than the absolute correctness of any single output. The classic illustration: a program computing sin(x) has no easy oracle, but it must satisfy sin(x) = sin(pi - x); run it twice on x and pi - x, and if the two outputs disagree, the program is wrong, with no need to know the true value of either. The survey systematizes the methodology (source test case, transformation, follow-up test case, relation check), reviews MR identification approaches, and catalogs application domains including computer graphics, simulation, compilers, web services, and numerical programs. It analyzes empirical practice across the literature and lays out open challenges: systematic MR discovery, MR prioritization, and integration into existing test processes.

**Limitations.** MT detects *violations* of relations; a passing MR does not prove correctness, only the absence of that specific inconsistency. The power of an MT campaign is entirely a function of the quality and diversity of the MRs chosen, which the survey identifies as the central open problem. A program can satisfy every chosen MR and still be wrong in a way no MR captures (for instance, a consistent-but-wrong constant offset that cancels in every relation).

**Kernel relevance.** This is the theoretical license for everything in Part 2. Keel cannot hand-author correct boolean results, but it can assert and continuously check the boolean algebra laws, transform invariance, refinement invariance, and mass-property identities that any correct kernel must satisfy. The survey's discipline (source case to follow-up case to relation check) is exactly the shape of the metamorphic test harness Keel should build, and its warning about MR coverage tells Keel to treat the MR checklist as a living, expanding artifact, never "done."

### Source: Kanewala & Bieman, metamorphic relations for scientific software; Lin et al. hierarchical MRs

**Citation.** Kanewala, U., & Bieman, J. M. (2016). Predicting metamorphic relations for testing scientific software: a machine learning approach using graph kernels. *Software Testing, Verification and Reliability, 26*(3), 245-269. https://onlinelibrary.wiley.com/doi/10.1002/stvr.1594 ; Lin, X., Simon, M., & Niu, N. (2018). Hierarchical Metamorphic Relations for Testing Scientific Software. *Proc. SE4Science*. https://homepages.uc.edu/~niunn/papers/SE4Science18.pdf ; Kanewala, U., et al. (2021). Discovering Metamorphic Relations for Scientific Software From User Forums. https://pmc.ncbi.nlm.nih.gov/articles/PMC8129917/

**Method.** This thread specializes MT for *scientific and numerical* software, which is the closest published analogue to a geometry kernel. Kanewala and Bieman taxonomize the MRs that recur in numerical code: **additive** (add a constant to inputs), **multiplicative / scaling** (multiply inputs by a constant), **permutative** (reorder inputs), **invertive** (invert/negate), **inclusive** (add an element), and **compositional** relations, and they train a graph-kernel classifier on the control-flow/data-flow graph to *predict* which MRs a given function should satisfy. Lin et al.'s hierarchical MRs compose simple relations into stronger ones and order them by detection strength so that cheap relations run first and expensive ones run only when needed. The user-forum work mines naturally occurring MRs from how practitioners describe expected behavior.

**Limitations.** The scaling and permutation MRs assume the computation is genuinely scale- or order-invariant, which for a *tolerance-based* kernel is only approximately true: uniform scale changes the relationship between coordinates and the fixed absolute tolerance, so a scaled rerun is not bit-identical, only equal within a scaled tolerance. MR prediction by ML is a research aid, not a guarantee.

**Kernel relevance.** Keel's MR catalog maps almost one-to-one onto this taxonomy. Scaling MR = uniform-scale invariance (with tolerance scaled accordingly). Permutative MR = operand-order invariance of commutative booleans. Additive MR = translation invariance. Invertive MR = the unextrude / undo round-trips. The hierarchical-MR idea directly informs CI tiering (Part 10): run cheap permutation/translation checks on every commit, expensive refinement-convergence checks nightly.

---

# PART 2: Metamorphic relations a kernel must satisfy

This part enumerates the concrete MRs for a B-rep kernel and how to check each. These are *necessary* properties: a correct kernel satisfies all of them; a violation is a definite bug (modulo the tolerance caveats noted).

## 2.1 Boolean algebra laws

The regularized set operations of a solid modeler form a Boolean algebra (more precisely, the regular closed sets under regularized union, intersection, difference form a Boolean lattice). That algebra's laws are free, high-value MRs:

- **Commutativity**: A ∪ B = B ∪ A, A ∩ B = B ∩ A. Run the boolean with operands swapped; the results must be the same shape (within tolerance, per kernel/22 model comparison). This catches order-dependent code paths, a classic robustness bug.
- **Associativity**: (A ∪ B) ∪ C = A ∪ (B ∪ C). Catches accumulation/round-off asymmetries.
- **Idempotence**: A ∪ A = A, A ∩ A = A. Self-union must return A unchanged. A surprisingly effective stress test because coincident faces are a degeneracy.
- **De Morgan**: complement(A ∪ B) = complement(A) ∩ complement(B), realized via difference against a large bounding solid. Couples union and intersection code paths so a bug in one shows as a relation violation.
- **Annihilation / identity**: A ∖ A = ∅, A ∖ ∅ = A, A ∩ ∅ = ∅. The empty-result and unchanged-result cases test the kernel's handling of total cancellation, a frequent crash site.

**Checking** uses the model-comparison machinery (kernel/22): two B-reps are "the same shape" when there is a tolerance-aware correspondence of faces/edges/vertices and the surfaces agree within epsilon. The comparison is itself nontrivial and is the tolerance-aware-comparison problem discussed in Part 3.

## 2.2 Round-trip (invertive) relations

- **Boolean undo**: (A ∪ B) ∖ B should approximate A on the part of A disjoint from B; (A ∩ B) within B equals A within B. Exact recovery is not expected (the boolean introduces new edges along the seam), so the relation is checked as "agrees with A away from the interaction zone, within tolerance."
- **Extrude then unextrude / imprint then unimprint**: a modeling operation followed by its inverse should return the original body within tolerance. This is the geometric analogue of the encode/decode round-trip and catches non-invertible bookkeeping (persistent naming, kernel/07; serialization, kernel/14).
- **Transform round-trip**: apply T then T-inverse; result must be bitwise or near-bitwise identical to input.

## 2.3 Invariance under rigid transform and uniform scale

- **Rigid-transform invariance**: the kernel applied to a rotated/translated copy of the input must produce the rotated/translated copy of the output. Topologically identical; geometrically related by the transform. This is the single most powerful MR for a kernel because almost every robustness bug is position-dependent: an algorithm that works at the origin but fails at coordinates of 10^6 (the "far from origin" failure) is caught immediately. Run every boolean and feature operation at the origin and at a large offset and require the same topology.
- **Uniform-scale invariance**: scaling the input by s scales lengths by s, areas by s^2, volumes by s^3, and the *topology* must be identical. Caveat: because the kernel uses a fixed absolute tolerance, scaling does change which degeneracies are within tolerance, so the relation holds only when the scaled tolerance is also applied. This MR doubles as a tolerance-sensitivity probe.

## 2.4 Invariance under representational refinement (geometry must not change)

NURBS carry redundant representational freedom. The *geometry* is invariant under:

- **Knot refinement / insertion**: inserting knots (subdividing the parameter domain) changes the control net and count but not the curve/surface point set. A boolean computed on a knot-refined operand must yield the same shape as on the original.
- **Degree elevation**: raising the degree with compensating control points leaves the geometry unchanged. Same MR.

These are exceptionally good MRs because they perturb the *internal representation* while holding the *modeled object* fixed, so any representation-dependent bug (e.g., an intersection routine that keys off control points rather than the surface) is exposed. They are cheap to generate (knot insertion and degree elevation are standard, exact spline operations) and the expected relation is exact-shape-equality, not just tolerance-equality, on the operands, with tolerance-equality on the results.

## 2.5 Mass-property identities

Integral quantities give continuous, numeric MRs that do not require shape comparison at all:

- **Inclusion-exclusion for volume**: vol(A ∪ B) = vol(A) + vol(B) - vol(A ∩ B). Compute all four bodies and check the identity numerically. A violation beyond the integration tolerance flags a boolean error even when the result B-rep is topologically plausible.
- **Additivity over a partition**: if a body is split by an imprint into pieces, the volumes (and surface areas, and inertia tensors about a common frame) of the pieces must sum to the whole. Surface area is additive minus the shared seam; volume is cleanly additive.
- **Surface-area and centroid identities** under union/intersection give independent checks at lower cost than full shape comparison.

These identities are checked by the mass-property integrator (which itself should be validated, ideally with a validated-numerics enclosure per Part 5 so the comparison tolerance is rigorous rather than guessed).

## 2.6 Tessellation-refinement convergence

For tessellation/faceting (kernel/05), the MR is **convergence under refinement**: as the chord/angular tolerance shrinks, the tessellation's measured volume, area, and Hausdorff distance to the exact surface must converge monotonically (or near-monotonically) toward the analytic values. A non-converging or oscillating sequence indicates a faceting bug or a surface-evaluation error. This is a metamorphic relation across a *family* of runs at decreasing tolerance, and it is the natural place to also assert the validated error bound (the facet should lie within the claimed deviation band, checked rigorously via interval surface evaluation).

---

# PART 3: Differential testing against reference kernels

## 3.1 The methodology and its founder

### Source: McKeeman, "Differential Testing for Software"

**Citation.** McKeeman, W. M. (1998). Differential Testing for Software. *Digital Technical Journal, 10*(1), 100-107.

**Method.** McKeeman coined and formalized differential testing: feed the same input to two or more independent implementations of the same specification and flag any divergence in output as a bug in at least one of them. The technique needs no oracle because the *implementations check each other*. McKeeman developed it for compilers, generating large volumes of programs and comparing compiled behavior. The method scales because input generation and comparison are both automatable; the human cost is only in triaging divergences to find which implementation is at fault.

**Limitations.** Differential testing only finds bugs where the implementations *disagree*; a fault common to all references is invisible (correlated failure). It also requires a meaningful equality test on outputs, which for geometry is the hard tolerance-aware comparison problem below. And it presumes the implementations target the *same* specification: subtle tolerance-model or convention differences (orientation, parameterization) produce false-positive divergences that must be normalized away.

**Kernel relevance.** This is the backbone of Keel's external verification. Keel will compare its boolean, fillet, offset, and tessellation outputs against OCCT (open, scriptable) on a large generated corpus, treating any divergence as a triage item. The McKeeman framing tells Keel to invest in *automatic divergence triage* (which kernel is wrong?) as much as in running the comparison.

### Source: Yang, Chen, Eide, Regehr, Csmith ("Finding and Understanding Bugs in C Compilers")

**Citation.** Yang, X., Chen, Y., Eide, E., & Regehr, J. (2011). Finding and Understanding Bugs in C Compilers. *Proc. PLDI 2011*, 283-294. https://www.cs.utah.edu/~regehr/papers/pldi11-preprint.pdf

**Method.** Csmith generates random, well-defined C programs (carefully avoiding undefined behavior so that all conforming compilers must agree) and runs them through multiple compilers, using **majority vote** as the oracle: when compilers disagree, the minority is the likely bug. This randomized differential testing with a voting oracle found and got fixed *hundreds* of bugs across GCC, LLVM, and commercial compilers. The two innovations Keel should steal: (1) generate inputs that are *guaranteed valid* so any divergence is a real bug not an input-definition dispute, and (2) use *three or more* references so a vote localizes the fault.

**Limitations.** The voting oracle fails when a majority of implementations share a bug (common for spec ambiguities). Generating "definitely valid, definitely unambiguous" inputs is itself hard, and the generator's own biases shape which bugs are found. For geometry, exact bit-equality of outputs is never available, so "agreement" must be tolerance-defined, weakening the vote.

**Kernel relevance.** The voting-oracle pattern upgrades Keel's differential testing from two-kernel ("they disagree, someone is wrong, but who?") to N-kernel ("Keel is the outlier, Keel is probably wrong"). With OCCT plus any accessible second reference plus Keel's own exact-path oracle (Part 4) on the easy cases, Keel can form a three-way vote on linear and low-degree inputs. The "generate only valid inputs" discipline ties directly to the corpus-generation companion (kernel/34).

## 3.2 The tolerance-aware comparison problem

Differential testing needs an equality test on B-reps, and "equal" is not bit-equality. Two kernels producing the same shape will differ in: vertex coordinates (different intersection solvers), edge parameterizations, face splitting choices (one seam vs two), and tolerance assignment. The comparison must therefore answer **"are these the same shape within epsilon?"** rather than "are these identical data structures?". This is precisely the model-comparison / model-diff problem (kernel/22): establish a correspondence between topological entities, then check geometric agreement (Hausdorff distance between corresponding faces below epsilon, matching adjacency, matching genus and Euler characteristic). The robust comparison runs structural invariants first (genus, shell count, face count after canonicalization) as a cheap reject, then geometric distance on the survivors.

**Pitfall: reference kernels are buggy.** OCCT's boolean operations are documented as having a robustness gap (kernel/29 cites the OCCT "in search of a robust process" threads). A divergence between Keel and OCCT is therefore *ambiguous*: it may be Keel's bug or OCCT's. This is why the exact oracle (Part 4) matters: on the subset of inputs where Keel can compute an exact trusted answer, the exact result breaks the tie and *labels* the divergence, simultaneously testing Keel and characterizing the reference's reliability.

---

# PART 4: Invariant and self-consistency oracles

The cheapest oracle requires no second run and no reference: the result must satisfy the kernel's own validity invariants. A correct B-rep is a heavily constrained object, and most bugs produce an *invalid* object before they produce a subtly-wrong-but-valid one.

### Source: B-rep validity checking via Euler-Poincare and topological integrity

**Citation.** Euler-Poincare formula for B-rep validity, standard solid-modeling references: Mantyla, M. (1988). *An Introduction to Solid Modeling*. Computer Science Press; course notes https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler.html ; Analysis Situs "check shape" tooling https://www.analysissitus.org/features/features_check-shape.html ; "Boundary Representation Models: Validity and Rectification," Springer.

**Method.** A valid manifold B-rep must satisfy the **generalized Euler-Poincare relation** V - E + F = 2(S - H) + R (vertices, edges, faces, shells S, holes/genus H, ring-loops R), plus a battery of combinatorial and geometric integrity checks: every edge is shared by exactly two coedges with opposite orientation (**coedge pairing / orientation consistency**), every shell is closed (no boundary edges, **watertightness**), face normals are consistently outward, loops are correctly nested and oriented, no edge is shorter than tolerance, no face self-intersects, and geometry-topology consistency (the curve of an edge actually lies on the surfaces of its two adjacent faces within tolerance). The Euler-Poincare test is necessary but not sufficient (a count can balance while the model is invalid), so it is the cheapest gate, backed by the stronger structural checks. Production kernels (ACIS, Parasolid) and the OCCT-based Analysis Situs ship exactly such a checker.

**Limitations.** Euler-Poincare balance is necessary, not sufficient: it catches gross topological corruption but not a valid-but-geometrically-wrong result. Full geometry-topology consistency checking (does the edge curve truly lie on both faces?) is itself a tolerance computation and can be expensive. The checker validates *structure*, not *intent*: a perfectly valid cube returned where a cylinder was expected passes every invariant.

**Kernel relevance.** This is Keel's continuously-on, always-affordable oracle and it is non-negotiable (the kernel/29 doctrine: a returned body must be topologically valid even when geometry was salvaged). Keel should run the full validator as a **post-condition assertion on every operation in debug/CI builds**: any operation that emits an invalid B-rep is a bug regardless of whether the shape "looks right." Because it needs neither a reference nor a second run, it is the first line of the oracle stack and the one that runs on literally every test. It pairs with the kernel/01 topology layer (Euler operators preserve the invariant by construction) so that the validator is checking the rare path where an operator or boolean violates an invariant it should have maintained.

### The self-consistency principle generalized

Beyond topology, many results can be checked against internal invariants without a known answer: a fillet surface must be tangent to both supporting faces within tolerance (G1 continuity check); an offset surface must be at the requested distance from its base everywhere (sample and measure); a projection must land on the target; a parameterization must be monotone. Each is an *oracle derived from the operation's own specification*, computable from the single output. Keel should accumulate a library of these per-operation post-condition checks; they are the geometric analogue of an assertion and the highest-value-per-line verification a kernel can have.

---

# PART 5: Validated and certified numerics

The oracles above check the *answer*. Validated numerics changes the *answer itself* so that it arrives with a rigorous, machine-proved error bound rather than a hoped-for one. Instead of "the intersection point is approximately (x,y,z)," the kernel returns "the intersection point is provably inside this box," and a predicate returns not "the sign is probably +" but "the sign is + with certainty, or I cannot decide."

## 5.1 The validated-numerics tradition

### Source: Moore, interval analysis (the foundation)

**Citation.** Moore, R. E. (1966). *Interval Analysis*. Prentice-Hall. Moore, R. E., Kearfott, R. B., & Cloud, M. J. (2009). *Introduction to Interval Analysis*. SIAM.

**Method.** Interval arithmetic replaces each real number x by an interval [x_lo, x_hi] guaranteed to contain it, and defines arithmetic so the result interval is guaranteed to contain the true result: [a,b] + [c,d] = [a+c, b+d], with **outward rounding** (round the lower bound down, the upper bound up) so floating-point round-off is absorbed into the enclosure. Every operation is *self-validating*: the width of the output interval is a rigorous, automatically-propagated error bound. A sign test that returns an interval not containing zero proves the sign.

**Limitations.** The **dependency problem**: interval arithmetic treats each occurrence of a variable as independent, so x - x yields [-w, w] not 0, and chained computations suffer **error explosion** where enclosures blow up far beyond the true range. This makes naive interval arithmetic too pessimistic for long computations (exactly the curved-intersection chains Keel faces).

**Kernel relevance.** Interval arithmetic is the substrate of every certified geometric test. Keel already needs interval filters for predicates (kernel/11); the same machinery, applied to *constructions*, lets the kernel attach a rigorous bound to every computed point and curve. The dependency problem is why Keel needs the refinements below (affine arithmetic, Krawczyk, Taylor models) rather than naive intervals for anything nontrivial.

### Source: Rump, INTLAB

**Citation.** Rump, S. M. (1999). INTLAB - INTerval LABoratory. In T. Csendes (Ed.), *Developments in Reliable Computing* (pp. 77-104). Kluwer. https://www.tuhh.de/ti3/rump/intlab/

**Method.** INTLAB is the de facto standard interval/validated-numerics library (MATLAB/Octave), engineered by Rump to be *fast* by exploiting BLAS-level operations with directed rounding and Rump's own verified-linear-algebra and verified-root-finding algorithms. It provides verified solutions of linear systems, verified eigenvalue enclosures, verified nonlinear root finding, and supports real/complex intervals over scalars, vectors, and sparse matrices. Its significance is the demonstration that validated numerics can run at a small constant factor over plain floating point when implemented carefully, and that verified solvers for the operations a kernel needs (linear systems, roots) exist as a coherent toolkit.

**Limitations.** MATLAB-based, not directly usable in a Rust kernel, but the *algorithms* (Rump's verified-solver recipes, the directed-rounding-via-BLAS trick) transfer. Performance still degrades on ill-conditioned inputs.

**Kernel relevance.** INTLAB is the reference design for the validated-numerics layer Keel should build in Rust: an interval/affine type, directed-rounding primitives, and verified solvers for the small linear systems and root problems inside surface intersection and fitting (kernel/23). It is also a development-time oracle: prototype a tricky enclosure in INTLAB, then port.

### Source: Tucker, "Validated Numerics"

**Citation.** Tucker, W. (2011). *Validated Numerics: A Short Introduction to Rigorous Computations*. Princeton University Press.

**Method.** Tucker's book is the modern, accessible treatment of how to build computations whose every step carries a rigorous error bound: directed rounding, interval/affine arithmetic, automatic differentiation with enclosures, validated quadrature, and validated ODE integration. Tucker is famous for using exactly these techniques to give a computer-assisted proof resolving Smale's 14th problem (the Lorenz attractor), demonstrating that validated numerics can establish *mathematical theorems*, not merely careful estimates.

**Limitations.** A pedagogical and methodological text, not a kernel; the techniques require deliberate engineering and impose a constant-factor (sometimes order-of-magnitude) cost, so they are applied selectively, not everywhere.

**Kernel relevance.** Tucker is Keel's playbook for *where validated numerics pays off*: the high-consequence, hard-to-otherwise-verify computations (surface-surface intersection topology, singular/tangential intersections, certified marching) where a rigorous enclosure is worth the cost. The Lorenz precedent is the existence proof that "rigorously correct numeric geometry" is achievable, not aspirational.

## 5.2 Affine arithmetic (taming the dependency problem)

### Source: Comba, Stolfi, de Figueiredo, affine arithmetic

**Citation.** Comba, J. L. D., & Stolfi, J. (1993). Affine arithmetic and its applications to computer graphics. *Proc. SIBGRAPI*. de Figueiredo, L. H., & Stolfi, J. (2004). Affine Arithmetic: Concepts and Applications. *Numerical Algorithms, 37*(1-4), 147-158. https://link.springer.com/article/10.1023/B:NUMA.0000049462.70970.b6

**Method.** Affine arithmetic (AA) represents a quantity as an affine form x = x0 + x1*e1 + ... + xn*en, where each ei is a noise symbol in [-1, 1] shared across the computation. Because correlated quantities share noise symbols, **cancellation is tracked**: x - x is exactly 0, not [-w, w]. AA produces guaranteed enclosures like interval arithmetic but far tighter for chained, correlated computations, at the cost of carrying a vector of coefficients per value. It is the standard tool for reliable ray-surface intersection, implicit-surface rendering, and range analysis in graphics, precisely the operations a kernel performs.

**Limitations.** Nonlinear operations (multiply, sqrt) introduce a new noise symbol approximating the nonlinear part, so the form grows and the bound, while much tighter than intervals, is not exact. Per-value cost is higher than an interval (a coefficient vector vs two endpoints).

**Kernel relevance.** AA is the right enclosure model for Keel's curved range analysis: certifying that a Bezier/NURBS patch's coordinate range, or a ray-patch intersection's parameter range, lies within a box, without the interval-arithmetic explosion. It directly supports the tessellation error-bound MR (Part 2.6) and robust subdivision-based intersection (deciding "these two patches provably do not overlap" to prune the search), which is the affine-arithmetic sweet spot.

## 5.3 Certified root finding: interval Newton and Krawczyk

### Source: Krawczyk operator; Moore's existence/uniqueness theorem

**Citation.** Krawczyk, R. (1969). Newton-Algorithmen zur Bestimmung von Nullstellen mit Fehlerschranken. *Computing, 4*, 187-201. Moore, R. E. (1977). A test for existence of solutions to nonlinear systems. *SIAM J. Numerical Analysis, 14*(4), 611-615. Practical references: Zgliczynski lecture notes https://ww2.ii.uj.edu.pl/~zgliczyn/cap07/krawczyk.pdf ; IntervalRootFinding.jl https://juliaintervals.github.io/IntervalRootFinding.jl/

**Method.** The **interval Newton operator** N(X) = m - F(m)/F'(X) (with F'(X) an interval enclosure of the derivative over box X) has two certifying properties: if N(X) ⊂ X, there is a **unique** root of F in X (existence and uniqueness, proved, not estimated); if N(X) ∩ X = ∅, there is **no** root in X. Iterating N contracts the box to the root with rigorous bounds. The **Krawczyk operator** K(X) = m - Y*F(m) + (I - Y*F'(X))(X - m) (Y a numeric approximation of the inverse Jacobian) achieves the same certification *without interval division/matrix inversion*, making it numerically robust for systems. Krawczyk (1969), and Moore (1977) recognizing its existence/uniqueness power, are the foundation of all certified solving; modern certified path-tracking (homotopy continuation) and certified polynomial-system solving rest on it.

**Limitations.** Requires a derivative enclosure (interval Jacobian), so the function must be differentiable and the Jacobian computable. The test is *conclusive only when the containment holds*; near singular or multiple roots the operator may neither confirm nor exclude, and the box must be subdivided, which can be slow. Tangential intersections (Jacobian near-singular) are the hard case, exactly where geometry is hardest.

**Kernel relevance.** This is the certified core of Keel's curved intersection and fitting. Every "is there exactly one intersection point of these two curves in this box?" is a Krawczyk test; a positive result *certifies the topology* of the intersection (right number of points/branches), which is the decision that robustness ultimately hinges on. It deepens the kernel/11 predicate story: predicates certify *signs*, Krawczyk certifies *root existence/uniqueness*, the next level up. Keel should implement a Krawczyk-based certified solver for the 2x2 and 3x3 systems in curve-curve and curve-surface intersection.

## 5.4 Validated ODE solving for certified marching, and Taylor models

### Source: Mukundan, Ko, Maekawa, Sakkalis, Patrikalakis, validated-ODE surface intersection

**Citation.** Mukundan, H., Ko, K. H., Maekawa, T., Sakkalis, T., & Patrikalakis, N. M. (2004). Tracing surface intersections with a validated ODE system solver. *Proc. ACM Symposium on Solid Modeling and Applications*, 249-254. https://diglib.eg.org/server/api/core/bitstreams/760c6104-646e-40c8-be52-f6e7437e5952/content ; related: Maekawa & Patrikalakis offset-curve singularities; Patrikalakis & Maekawa (2002), *Shape Interrogation for Computer Aided Design and Manufacturing*, Springer.

**Method.** Surface-surface intersection (SSI) is the kernel's hardest robustness problem: the intersection is a space curve traced by marching, and naive marching can **stray** (jump to a wrong branch) or **loop** (cycle near singularities). Mukundan et al. formulate the intersection curve as the solution of an ODE system and integrate it with a **validated ODE solver** that produces, at each step, a rigorous interval enclosure of the true curve. This yields a gap-free boundary with a **numerically verified upper bound on the intersection-curve error in parameter space**, eliminating straying and looping by construction: the enclosure tells the marcher exactly where the true curve can be.

**Limitations.** Validated ODE integration is expensive (orders of magnitude over a plain marcher) and intricate to implement; near singular intersections (tangency, self-intersection) the enclosures widen and steps shrink. It addresses the *transversal* curve well; singular points still need special handling.

**Kernel relevance.** SSI is the single computation where Keel most needs a certified result, because a wrong intersection branch silently corrupts every downstream boolean. Keel should adopt validated-ODE (or equivalently Krawczyk-corrected) marching for the high-assurance SSI path, with a fast marcher as the default and the validated marcher as the certifying oracle (and the fallback when the fast path fails consistency checks). The "verified error bound in parameter space" is exactly the quantity the epsilon-solidity certificate (Part 9) wants to report.

### Source: CAPD library and Taylor models

**Citation.** CAPD::DynSys, Computer Assisted Proofs in Dynamics library, Jagiellonian University. http://capd.ii.uj.edu.pl/html/odes_rigorous.html ; Taylor models: Makino, K., & Berz, M. (2003). Taylor models and other validated functional inclusion methods. *Int. J. Pure and Applied Mathematics*.

**Method.** **Taylor models** represent a function over a domain as a Taylor polynomial plus a rigorous interval **remainder bound**: f(x) ∈ P(x) + I for all x in the domain. They dramatically reduce the dependency-problem blowup of interval arithmetic for smooth functions by capturing the function's shape in the polynomial part and bounding only the higher-order error in I. CAPD is the mature, open-source C++ library for **rigorous ODE integration** and Poincare maps using Taylor-model-style enclosures; it is the engine behind many computer-assisted proofs in dynamical systems and provides validated integration of the kind SSI marching needs.

**Limitations.** Taylor models are heavyweight (polynomial arithmetic with remainder tracking); CAPD targets dynamical-systems proofs, not CAD, so adapting it is non-trivial. The remainder bound still grows over long integrations.

**Kernel relevance.** Taylor models are the tightest practical enclosure for the smooth NURBS evaluations Keel does, and CAPD is the proof-of-existence that a production-grade validated integrator is buildable and open-source. For Keel, the takeaway is the *technique* (polynomial-plus-remainder enclosures for certified surface evaluation and marching), realized in Rust at the precision tier the kernel needs, rather than a direct dependency.

---

# PART 6: Exact computation as ground truth for the easy cases

Validated numerics certifies *bounds*. Exact computation certifies the *answer* outright, and for the inputs cheap enough to afford it, an exact engine is the gold-standard oracle that validates the fast f64 path.

### Source: CGAL exact-predicates-exact-constructions kernel and lazy exact evaluation

**Citation.** CGAL Exact_predicates_exact_constructions_kernel, https://doc.cgal.org/latest/Kernel_23/classCGAL_1_1Exact__predicates__exact__constructions__kernel.html ; Pion, S., & Fabri, A. (2011). A generic lazy evaluation scheme for exact geometric computation. *Science of Computer Programming, 76*(4), 307-323. https://arxiv.org/pdf/cs/0608063

**Method.** CGAL's exact kernels guarantee that both **predicates** (sign decisions) and **constructions** (computed points/curves) are mathematically exact, using exact number types (rationals, algebraic numbers) under a **lazy evaluation** scheme: every value carries a fast interval approximation, and the exact (expensive) representation is computed only when the interval fails to decide a sign. For linear and low-degree geometry this delivers exactness at near-floating-point speed on non-degenerate inputs (the interval decides almost always), escalating to exact arithmetic only at degeneracies. EPECK is the standard "trusted answer" engine in computational geometry.

**Limitations.** Exact constructions on chained operations cause the algebraic representation to grow without bound (the lazy DAG accumulates), so it is affordable for *bounded-depth, low-degree* computations, not arbitrary NURBS chains. High-degree algebraic numbers (curved-curved intersections, kernel/11) are exact but slow.

**Kernel relevance.** This is the "**exact oracle for the easy cases**" pattern, and it is one of the most cost-effective verification investments Keel can make. For the linear and low-degree subset of the test corpus (planar/polyhedral booleans, line-plane intersections, the linear predicates already exact in Keel per kernel/11), an EPECK-style exact engine (CGAL itself as an external oracle, or a Rust exact path) computes the *true* answer, against which Keel's fast f64 result is checked exactly. This labels differential-testing divergences (Part 3.2): on the easy inputs, the exact engine is the tiebreaker that says whether Keel or OCCT is wrong. The strategy is "exact where affordable, validated where not, invariant-checked always."

### Source: Shewchuk adaptive exact predicates (the certified predicate kernel)

**Citation.** Shewchuk, J. R. (1997). Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates. *Discrete & Computational Geometry, 18*(3), 305-363. https://www.cs.cmu.edu/~quake/robust.html

**Method.** Shewchuk's orientation and incircle predicates compute the *exact sign* of a determinant using adaptive-precision floating-point expansions: a fast f64 estimate with an error bound is computed first, and precision is increased incrementally only when the estimate is too close to zero to be trusted, terminating as soon as the sign is certain. The result is a predicate that is *exact* (never wrong) yet runs at near-f64 speed on non-degenerate inputs. This is the gold standard for linear predicate correctness and the canonical example of a numeric routine that is its own oracle: the sign it returns is provably the true sign.

**Limitations.** Covers the specific linear predicates (orientation, incircle in 2D/3D); does not extend to curved or constructed geometry without the heavier machinery of kernel/11. The expansions grow at exact degeneracies.

**Kernel relevance.** Keel's linear predicate layer (kernel/11) should be Shewchuk-based and is, by construction, self-certifying: it needs no external oracle because its output is provably exact. In the verification stack, Shewchuk predicates are simultaneously a *building block* (used inside every algorithm) and a *trusted oracle* (the exact sign against which any filtered/approximate predicate is validated). They are the floor of the exact-ground-truth tier.

---

# PART 7: Formal verification reach

How far can machine-checked proof realistically go for a geometry kernel? The honest answer: far enough to fully verify the *predicates and small algorithms*, not the whole kernel. The literature shows convex hull, arrangement, and cell-decomposition algorithms have been formally verified, and that large-scale geometric proof (Flyspeck) is achievable, but a fully verified industrial boolean engine remains out of reach.

### Source: Pichardie & Bertot, formal convex hull in Coq

**Citation.** Pichardie, D., & Bertot, Y. (2001). Formalizing Convex Hull Algorithms. *Proc. TPHOLs 2001*, LNCS 2152, 346-361. http://www-sop.inria.fr/members/Yves.Bertot/hulls.pdf

**Method.** Pichardie and Bertot give machine-checked Coq proofs of correctness for two convex-hull algorithms (incremental and Jarvis' march), including the **degenerate cases** usually hand-waved in textbook presentations. A key insight for kernels: the **orientation predicate** can be formalized over a *ring* rather than a complete field, so the proofs avoid floating-point entirely; the algorithm's correctness is proved in terms of the exact sign predicate, and a correct exact-sign implementation (Shewchuk, Part 6) then discharges the predicate obligation. They extract running programs from the proofs. This establishes that the combinatorial core of a geometric algorithm, *including degeneracy handling*, is formally verifiable.

**Limitations.** Convex hull is a comparatively simple algorithm over points; the effort to formalize was substantial. Scaling to booleans on curved B-reps, with their tolerance models and floating-point constructions, is far beyond what has been done. The proof assumes an exact predicate oracle, so it verifies the *combinatorics given correct predicates*, not the numerics.

**Kernel relevance.** This defines the realistic *reach* of formal methods for Keel: the combinatorial/topological algorithms (Euler operators kernel/01, hull, arrangement, the coedge-pairing invariants) are formalizable, and the predicate-over-a-ring framing matches Keel's exact predicate layer exactly. A pragmatic target: formally verify the topology-validator invariants and the Euler-operator correctness, the pieces whose bugs are most catastrophic and whose logic is most amenable to proof.

### Source: Bertot, "Formal Verification of a Geometry Algorithm" (abstract views and symmetry)

**Citation.** Bertot, Y. (2018). Formal Verification of a Geometry Algorithm: A Quest for Abstract Views and Symmetry in Coq Proofs. *Proc. ICTAC 2018*, LNCS. https://arxiv.org/pdf/1809.00559 ; related: "Formally Verifying a Vertical Cell Decomposition Algorithm," *Proc. ITP 2025*, https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ITP.2025.24

**Method.** This later work tackles the *engineering* of geometric proofs: managing the combinatorial explosion of degenerate cases via abstract views and symmetry arguments, and formally verifying a vertical cell decomposition (a building block of motion planning and arrangement computation). It shows the field progressing from points-only algorithms toward the planar-subdivision/arrangement algorithms that are closer to what a kernel does.

**Limitations.** Still planar, still combinatorial, still assuming exact predicates; the proof-engineering cost remains high and grows with algorithm complexity.

**Kernel relevance.** Confirms the trajectory and the ceiling: arrangements and cell decompositions (relevant to imprint and 2D boolean substeps) are reaching formal verification, so Keel could in principle verify its planar-arrangement substep. But it reinforces that whole-kernel verification is not on the table; the proof budget should target the highest-consequence invariants.

### Source: Hales et al., Flyspeck (formal proof of the Kepler conjecture)

**Citation.** Hales, T., Adams, M., Bauer, G., et al. (2017). A Formal Proof of the Kepler Conjecture. *Forum of Mathematics, Pi, 5*, e2. https://www.cambridge.org/core/journals/forum-of-mathematics-pi/article/formal-proof-of-the-kepler-conjecture/ ; https://github.com/flyspeck/flyspeck

**Method.** Flyspeck (Formal Proof of Kepler) machine-checked the entire proof of the Kepler sphere-packing conjecture in **HOL Light and Isabelle/HOL**, a proof that crucially involves thousands of nonlinear arithmetic inequalities over geometric configurations. These inequalities were discharged by **verified interval-arithmetic computation** reflected into the proof assistant: the rigorous numeric bounds (Part 5) become formal theorems. Completed in 2014 after an 11-year collaboration, it is the landmark demonstration that large-scale geometric reasoning, combining combinatorics and rigorous numerics, can be fully formalized.

**Limitations.** A heroic, decade-long, special-purpose effort, not a repeatable engineering process. It proves a fixed theorem, not a reusable kernel; the techniques (reflected interval arithmetic) are transferable but the scale of effort is not.

**Kernel relevance.** Flyspeck is the existence proof and the *bridge* between Parts 5 and 7: it shows validated numerics and formal proof compose, that rigorous interval bounds can become machine-checked theorems. For Keel this is aspirational evidence, not a plan: it justifies the claim that the validated-numerics layer is "proof-grade," and suggests that specific high-value geometric inequalities (a fillet's tangency condition, an offset's distance bound) could in principle be discharged with reflected interval arithmetic if a guarantee were ever required for a safety-critical deployment.

---

# PART 8: Reproducibility as a correctness component

Differential and metamorphic oracles presuppose that a rerun produces a comparable result. If the kernel is non-deterministic across platforms or compilers, "Keel disagrees with itself" becomes indistinguishable from "Keel has a bug," and every comparison is contaminated. Reproducibility is therefore a *testability property*, and tools that detect numerical instability double as oracles for "this code path is dangerously sensitive." (Determinism mechanics are kernel/14; here the focus is its role in verification.)

### Source: FLiT (cross-platform floating-point result-consistency tester)

**Citation.** Sawaya, G., Bentley, M., Briggs, I., Gopalakrishnan, G., & Ahn, D. H. (2017). FLiT: Cross-platform floating-point result-consistency tester and workload. *Proc. IISWC 2017*, 229-238. https://pruners.github.io/flit/ ; https://github.com/PRUNERS/FLiT

**Method.** FLiT (originally "Floating-point Litmus Tests") compiles the same code under many compilers and a wide range of optimization flags, runs it, and compares results against an unoptimized "ground truth" compilation, reporting which compiler/flag combinations change the answer. It pinpoints **compiler-induced variability**: fused multiply-add contraction, reassociation, vectorization, and fast-math transforms that silently alter results. It then helps select the *fastest compilation that is still bit-reproducible*.

**Limitations.** Detects *that* results vary across builds, not *whether* the variation is harmful (a variation below tolerance is harmless). Requires running the suite across many toolchains. It addresses build-induced nondeterminism, not algorithmic instability.

**Kernel relevance.** Keel needs exactly this to defend its determinism guarantee (kernel/14): a CI job that compiles Keel under the supported Rust toolchains/targets and flags any operation whose output diverges. Any divergence is either a determinism bug (forbidden by Keel's contract) or a numerically unstable code path that should be hardened. FLiT is the off-the-shelf design; Keel can replicate it as a "build-matrix consistency" CI tier.

### Source: CADNA / CESTAC discrete stochastic arithmetic; Verificarlo; Verrou

**Citation.** Vignes, J. (2004). Discrete Stochastic Arithmetic for Validating Results of Numerical Software. *Numerical Algorithms, 37*(1-4), 377-390. (CESTAC method, Vignes & La Porte, 1970s.) Jezequel, F., & Chesneaux, J.-M. (2008). CADNA: a library for estimating round-off error propagation. *Computer Physics Communications, 178*(12), 933-955. Denis, C., de Oliveira Castro, P., & Petit, E. (2016). Verificarlo: Checking Floating Point Accuracy through Monte Carlo Arithmetic. *Proc. ARITH 2016*. https://arxiv.org/pdf/1509.01347 ; Verrou (EDF), Valgrind-based.

**Method.** Stochastic arithmetic estimates round-off error by **perturbing the computation randomly and measuring the spread of results**. CESTAC/CADNA (Discrete Stochastic Arithmetic) runs each operation several times with **randomly switched rounding modes**, then uses a Student's t-test on the ensemble to estimate the **number of significant digits** that survived, flagging operations where the result is numerically meaningless ("computational zero"). **Monte Carlo Arithmetic** (Verificarlo, an LLVM tool, and Verrou, a Valgrind tool) generalizes this: it injects controlled noise at each floating-point operation and re-runs the whole program many times; if the output distribution is wide, the code is numerically unstable. Crucially, neither tool requires source annotation in the Verificarlo/Verrou case (they instrument at compile/binary level), making them low-friction.

**Limitations.** Stochastic methods give a *statistical* estimate of instability, not a rigorous bound (unlike interval arithmetic); a stable-looking sample does not prove stability. CADNA requires source-type changes; Verificarlo/Verrou are sampling-based and add runtime cost. They find unstable *spots*, they do not fix them.

**Kernel relevance.** These are Keel's **instability detectors**: run the kernel's hot numeric paths (intersection solvers, fitting, mass-property integration) under Verrou/Verificarlo on the corpus and flag any computation whose result loses most of its significant digits under perturbation. Those flagged paths are exactly where a near-degenerate input will flip a decision, the bugs that differential testing finds only occasionally but stochastic arithmetic finds systematically. It is the nightly "where is this kernel numerically fragile?" oracle, complementary to the rigorous-but-narrow validated-numerics layer.

---

# PART 9: Certifying best-effort results (the tolerant-kernel contract)

The kernel/29 doctrine says Keel returns a *usable answer* on dirty input rather than refusing. That promise is only honest if the returned answer carries a **checkable quality certificate**: how far was it salvaged, and against what guarantee? This part connects the verification machinery to the best-effort contract.

### Source: Qi & Shapiro, epsilon-solidity and epsilon-regularity

**Citation.** Qi, J., & Shapiro, V. (2006). Geometric Interoperability With Epsilon Solidity. *J. Computing and Information Science in Engineering, 6*(3), 213-222. https://turbomachinery.asmedigitalcollection.asme.org/computingengineering/article-abstract/6/3/213/ ; Qi, J., & Shapiro, V. (2004). Epsilon-Solidity in Geometric Data Translation. (Tech report / related.) https://spatial.engr.wisc.edu/wp-content/uploads/sites/715/2014/04/2004-2p2.pdf

**Method.** Qi and Shapiro replace the classical, exact notion of a "solid" (a regular closed point set) with **epsilon-solidity**: a representation is an epsilon-solid if there exists a true solid within Hausdorff distance epsilon of it, i.e. the imperfect B-rep is *certifiably close to some valid solid* for a reported epsilon. Their **epsilon-regularity** generalizes open/closed-regular sets to a tolerance band (a "set interval"), with classical solids as the epsilon to 0 limit. The key results: classical B-rep validity checks are *neither necessary nor sufficient* for maintaining epsilon-solidity under numerical inaccuracy, and most data-translation problems can preserve epsilon-solidity *without* expensive geometric healing, provided epsilon is tracked and reported. The framework turns "is this valid?" (binary, often unanswerable on dirty data) into "for what epsilon is this an epsilon-solid?" (a measurable quantity).

**Limitations.** Epsilon-solidity certifies *closeness to some solid*, not closeness to the *intended* solid; a small epsilon means the representation is internally near-consistent, not that it models what the user meant. Computing or bounding the true epsilon rigorously is itself work (this is where validated numerics, Part 5, supplies the rigorous bound).

**Kernel relevance.** This is the formal backbone of Keel's best-effort certificate. Every operation that widens tolerance or salvages geometry should return, alongside the body, a **reported epsilon** (the achieved deviation) and the claim "this is a valid epsilon-solid for this epsilon," ideally with the epsilon *rigorously bounded* by the validated-numerics layer rather than estimated. This is the contract a tolerant kernel owes the caller (kernel/29 Part 7): not "trust me," but "here is the body, here is how far it was salvaged, here is the validity class it provably belongs to." The verification stack supplies the proof: the topology validator (Part 4) certifies the *combinatorial* validity flag, and interval/affine enclosures (Part 5) certify the *epsilon* number.

**The certify-the-result contract, stated.** Every Keel operation returns: (1) the body; (2) a validity flag from the always-on topology validator (provably a valid B-rep, or explicitly flagged invalid); (3) for tolerant/salvaged results, a reported epsilon with the epsilon-solidity claim, rigorously bounded where the validated path was used; (4) per-entity fault annotations (kernel/29) for anything carried forward imperfect. This turns "we salvaged this" into a structured, machine-checkable quality measure the caller can act on.

---

# PART 10: Wiring the oracles into CI

The oracle stack is only useful if it runs continuously and affordably. The governing principle (from the hierarchical-MR work, Part 1.2): order checks by cost, run cheap ones always, expensive ones nightly, and gate merges on the cheap-but-conclusive tier.

**Always-on (every commit, must pass to merge), cheap and conclusive:**
- **Topology validator** as a post-condition on every operation (Part 4). A single operation emitting an invalid B-rep blocks the merge. Cheapest, highest-value, no oracle needed.
- **Self-consistency post-conditions** (fillet tangency, offset distance, projection-on-target). Per-operation, single-run, cheap.
- **Cheap metamorphic relations**: commutativity, operand-order, translation invariance, idempotence (A∪A=A), small-offset rigid-transform invariance. Each is one extra run plus a tolerance-aware compare.
- **Exact-oracle check on the linear/low-degree corpus subset** (Part 6): fast for the easy cases, conclusive (labels right/wrong unambiguously).
- **Determinism check**: same input, same platform, bit-identical output (kernel/14).

**Nightly / weekly (expensive, broad), the heavy oracles:**
- **Differential testing against OCCT** (and any second reference) on the full generated corpus (kernel/34), with automatic divergence triage and the exact oracle as tiebreaker where it applies.
- **Refinement-convergence MRs**: tessellation convergence, knot-refinement/degree-elevation invariance across a family of resolutions.
- **Mass-property identity sweep**: inclusion-exclusion volume checks across the boolean corpus.
- **Build-matrix consistency (FLiT-style)**: compile across toolchains/targets, flag any output divergence.
- **Stochastic-arithmetic instability scan (Verrou/Verificarlo)**: flag numerically fragile code paths on hot numeric routines.
- **Validated-numerics spot checks**: on a curated hard subset (tangential/singular intersections), run the certified Krawczyk/validated-ODE path and confirm the fast path's result lies inside the rigorous enclosure.

**Release-gated / on-demand (heaviest):**
- **Formal-proof regression**: re-check the machine-verified invariants (Euler-operator correctness, validator soundness) if their code changed.
- **Full epsilon-solidity audit**: on the dirty-geometry corpus, confirm every salvaged result's reported epsilon is rigorously sound.

**Gating policy.** Merge is gated only on the always-on tier (validity + cheap MRs + exact-easy + determinism), keeping per-commit CI fast. The nightly tier produces a triage queue, not a merge block, because differential divergences need human classification (which kernel is wrong). A regression in the always-on tier is a hard stop; a new nightly divergence is a ticket.

---

# Verification doctrine for Keel (synthesis)

**The oracle stack, cheapest to most expensive.** Keel verifies without a ground-truth oracle by layering checks of increasing cost and decreasing frequency:

1. **Invariant oracle (always, free):** every result is a provably valid B-rep (Euler-Poincare, shell closure, coedge pairing, orientation, geometry-topology consistency). Runs as a post-condition on every operation. This is the non-negotiable floor and the one check that needs no second run and no reference.
2. **Self-consistency oracle (always, cheap):** each operation's own specification yields a single-output check (tangency, offset distance, monotone parameterization).
3. **Metamorphic oracle (cheap always, expensive nightly):** the result relates correctly to other runs. The checklist below.
4. **Exact oracle for easy cases (cheap on the subset):** EPECK-style exact arithmetic gives true ground truth on linear/low-degree inputs, validating the fast path and labeling differential divergences. Shewchuk predicates are self-certifying at the predicate level.
5. **Differential oracle (nightly):** agreement with OCCT and other references, as a voting jury, with the exact oracle as tiebreaker, McKeeman/Csmith methodology.
6. **Validated-numerics oracle (nightly, hard subset):** rigorous enclosures (interval, affine, Krawczyk-certified roots, validated-ODE marching, Taylor models) confirm the fast result lies in a proved error band, the answer carries its own proof.
7. **Formal proof (release-gated, narrow):** machine-checked correctness of the load-bearing combinatorial pieces.

**The metamorphic-relation checklist Keel must satisfy:**
- Boolean algebra: commutativity, associativity, idempotence (A∪A=A), De Morgan, annihilation (A∖A=∅), identity (A∖∅=A).
- Round-trips: (A∪B)∖B ≈ A; extrude/unextrude; transform/inverse-transform.
- Invariance: rigid transform (especially far-from-origin), uniform scale (with scaled tolerance).
- Representational invariance: knot refinement, degree elevation leave geometry unchanged.
- Mass-property identities: vol(A∪B) = vol(A)+vol(B)-vol(A∩B); additivity over partitions; area/centroid identities.
- Convergence: tessellation refinement converges to analytic volume/area/Hausdorff distance.

**The certify-the-result contract.** Every operation returns the body plus: a provable validity flag (always), a rigorously-bounded reported epsilon with an epsilon-solidity claim (for tolerant/salvaged results, per Qi-Shapiro, with the bound supplied by validated numerics), and per-entity fault annotations. The tolerant kernel owes the caller not just an answer but a *characterized* answer.

**What formal methods can realistically cover.** Honestly: the predicates (Shewchuk-style exact signs, self-certifying) and the small combinatorial algorithms (Euler operators, convex hull, arrangements, cell decomposition, the validator invariants), following Pichardie-Bertot and the ITP cell-decomposition line. Flyspeck proves that rigorous numeric geometry can be machine-checked at scale, but a fully verified industrial boolean engine on curved B-reps is not achievable with current methods. The pragmatic formal-verification target is the highest-consequence, most-combinatorial core: prove the validator sound and the Euler operators invariant-preserving, lean on exact predicates for the numeric obligations, and rely on validated numerics (proof-grade but not proof) for the rest. Formal methods cover the *spine*; validated numerics covers the *flesh*; metamorphic, differential, and invariant oracles cover the *whole body* continuously.

**The throughline.** A geometry kernel has no oracle for most of what it computes, so it must be built to check itself: every answer validated against invariants for free, related to other answers metamorphically, compared to independent references differentially, pinned to exact ground truth where affordable, and, on the decisions that matter most, certified by rigorous enclosure. Correctness is not a single test; it is a stack of mutually reinforcing oracles, none sufficient alone, jointly making a kernel trustworthy on inputs no human will ever check by hand.

---

# References

1. Segura, S., Fraser, G., Sanchez, A. B., & Ruiz-Cortes, A. (2016). A Survey on Metamorphic Testing. *IEEE TSE, 42*(9), 805-824. https://eprints.whiterose.ac.uk/id/eprint/110335/1/segura16-tse.pdf
2. Kanewala, U., & Bieman, J. M. (2016). Predicting metamorphic relations for testing scientific software: a machine learning approach using graph kernels. *Software Testing, Verification and Reliability, 26*(3), 245-269. https://onlinelibrary.wiley.com/doi/10.1002/stvr.1594
3. Lin, X., Simon, M., & Niu, N. (2018). Hierarchical Metamorphic Relations for Testing Scientific Software. *Proc. SE4Science*. https://homepages.uc.edu/~niunn/papers/SE4Science18.pdf
4. Kanewala, U., et al. (2021). Discovering Metamorphic Relations for Scientific Software From User Forums. https://pmc.ncbi.nlm.nih.gov/articles/PMC8129917/
5. McKeeman, W. M. (1998). Differential Testing for Software. *Digital Technical Journal, 10*(1), 100-107.
6. Yang, X., Chen, Y., Eide, E., & Regehr, J. (2011). Finding and Understanding Bugs in C Compilers. *Proc. PLDI 2011*, 283-294. https://www.cs.utah.edu/~regehr/papers/pldi11-preprint.pdf
7. Mantyla, M. (1988). *An Introduction to Solid Modeling*. Computer Science Press. (Euler-Poincare B-rep validity.) https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler.html
8. Analysis Situs, Check shape validity. https://www.analysissitus.org/features/features_check-shape.html
9. Moore, R. E. (1966). *Interval Analysis*. Prentice-Hall. Moore, Kearfott, & Cloud (2009). *Introduction to Interval Analysis*. SIAM.
10. Rump, S. M. (1999). INTLAB - INTerval LABoratory. In *Developments in Reliable Computing* (pp. 77-104). Kluwer. https://www.tuhh.de/ti3/rump/intlab/
11. Tucker, W. (2011). *Validated Numerics: A Short Introduction to Rigorous Computations*. Princeton University Press.
12. Comba, J. L. D., & Stolfi, J. (1993). Affine arithmetic and its applications to computer graphics. *Proc. SIBGRAPI*. de Figueiredo, L. H., & Stolfi, J. (2004). Affine Arithmetic: Concepts and Applications. *Numerical Algorithms, 37*(1-4), 147-158. https://link.springer.com/article/10.1023/B:NUMA.0000049462.70970.b6
13. Krawczyk, R. (1969). Newton-Algorithmen zur Bestimmung von Nullstellen mit Fehlerschranken. *Computing, 4*, 187-201. Moore, R. E. (1977). A test for existence of solutions to nonlinear systems. *SIAM J. Numer. Anal., 14*(4), 611-615. https://ww2.ii.uj.edu.pl/~zgliczyn/cap07/krawczyk.pdf
14. Mukundan, H., Ko, K. H., Maekawa, T., Sakkalis, T., & Patrikalakis, N. M. (2004). Tracing surface intersections with a validated ODE system solver. *Proc. ACM SM 2004*, 249-254. https://diglib.eg.org/server/api/core/bitstreams/760c6104-646e-40c8-be52-f6e7437e5952/content
15. Patrikalakis, N. M., & Maekawa, T. (2002). *Shape Interrogation for Computer Aided Design and Manufacturing*. Springer.
16. CAPD::DynSys library, rigorous ODE methods, Jagiellonian University. http://capd.ii.uj.edu.pl/html/odes_rigorous.html
17. Makino, K., & Berz, M. (2003). Taylor models and other validated functional inclusion methods. *Int. J. Pure and Applied Mathematics*.
18. CGAL Exact_predicates_exact_constructions_kernel. https://doc.cgal.org/latest/Kernel_23/classCGAL_1_1Exact__predicates__exact__constructions__kernel.html ; Pion, S., & Fabri, A. (2011). A generic lazy evaluation scheme for exact geometric computation. *Sci. Comput. Program., 76*(4), 307-323. https://arxiv.org/pdf/cs/0608063
19. Shewchuk, J. R. (1997). Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates. *Discrete & Comput. Geom., 18*(3), 305-363. https://www.cs.cmu.edu/~quake/robust.html
20. Pichardie, D., & Bertot, Y. (2001). Formalizing Convex Hull Algorithms. *Proc. TPHOLs 2001*, LNCS 2152, 346-361. http://www-sop.inria.fr/members/Yves.Bertot/hulls.pdf
21. Bertot, Y. (2018). Formal Verification of a Geometry Algorithm: A Quest for Abstract Views and Symmetry in Coq Proofs. *Proc. ICTAC 2018*. https://arxiv.org/pdf/1809.00559 ; Formally Verifying a Vertical Cell Decomposition Algorithm. *Proc. ITP 2025*. https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ITP.2025.24
22. Hales, T., Adams, M., Bauer, G., et al. (2017). A Formal Proof of the Kepler Conjecture. *Forum of Mathematics, Pi, 5*, e2. https://www.cambridge.org/core/journals/forum-of-mathematics-pi/article/formal-proof-of-the-kepler-conjecture/ ; https://github.com/flyspeck/flyspeck
23. Sawaya, G., Bentley, M., Briggs, I., Gopalakrishnan, G., & Ahn, D. H. (2017). FLiT: Cross-platform floating-point result-consistency tester and workload. *Proc. IISWC 2017*, 229-238. https://pruners.github.io/flit/ ; https://github.com/PRUNERS/FLiT
24. Vignes, J. (2004). Discrete Stochastic Arithmetic for Validating Results of Numerical Software. *Numerical Algorithms, 37*(1-4), 377-390. Jezequel, F., & Chesneaux, J.-M. (2008). CADNA: a library for estimating round-off error propagation. *Comput. Phys. Commun., 178*(12), 933-955.
25. Denis, C., de Oliveira Castro, P., & Petit, E. (2016). Verificarlo: Checking Floating Point Accuracy through Monte Carlo Arithmetic. *Proc. ARITH 2016*. https://arxiv.org/pdf/1509.01347 ; Verrou (EDF), Valgrind-based MCA tool.
26. Qi, J., & Shapiro, V. (2006). Geometric Interoperability With Epsilon Solidity. *J. Comput. Inf. Sci. Eng., 6*(3), 213-222. https://turbomachinery.asmedigitalcollection.asme.org/computingengineering/article-abstract/6/3/213/ ; Qi, J., & Shapiro, V. (2004). Epsilon-Solidity in Geometric Data Translation. https://spatial.engr.wisc.edu/wp-content/uploads/sites/715/2014/04/2004-2p2.pdf

---

*Cross-references: kernel/01 (topology and Euler operators, the validator substrate), kernel/05 (tessellation, the convergence MR), kernel/07 (persistent naming, round-trip MRs), kernel/11 (exact and filtered predicates, certified by validated numerics and formal proof), kernel/14 (determinism and serialization, the reproducibility precondition), kernel/22 (model comparison, the tolerance-aware-equality engine behind differential and metamorphic checks), kernel/23 (metrology and fitting, validated-numerics consumer), kernel/29 (graceful degradation and the best-effort contract, certified here via epsilon-solidity), kernel/34 (breaking-input generation and corpus, the input half of this file's output half).*
