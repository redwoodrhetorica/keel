# Geometric Constraint Solving and 2D Profile Computational Geometry

Research dossier 04 for the Keel kernel. This file investigates two adjacent but separable bodies of literature. Part 1 covers geometric constraint solving (GCS): the machinery behind a parametric sketcher, where a user draws rough geometry, applies dimensions and relations, and the solver moves the geometry to satisfy them. Part 2 covers 2D profile computational geometry: turning the solved (or even un-constrained) curve soup of a sketch into clean planar regions, performing planar booleans, and offsetting curves. The kernel itself (extrude, revolve, sweep) consumes the Part 2 output; the Part 1 solver may live above the kernel as a peer component, mirroring how Siemens ships D-Cubed DCM beside Parasolid. A central deliverable of this dossier is a recommendation on that boundary.

Scope note on terminology: a "constraint graph" has geometric entities (points, lines, circles) as vertices and constraints (distance, angle, tangency) as edges. "Degrees of freedom" (DOF) counts the free parameters of the entities minus the equations the constraints impose. A system is "well-constrained" when DOF nets to the rigid-motion residue (3 in 2D: two translations plus one rotation), "under-constrained" when more, "over-constrained" when constraints are redundant or conflicting.

---

# Part 1: Geometric Constraint Solving

## 1.1 Graph-based decomposition

The dominant industrial strategy is to avoid throwing the entire nonlinear system at a numerical solver. Instead, analyze the constraint graph, decompose it into small rigid pieces that can be solved by closed-form construction (ruler-and-compass style), then reassemble. Decomposition makes the solver fast, robust, and able to report exactly which subpart is over- or under-constrained.

### Owen (1991): top-down triconnected decomposition

**Citation.** Owen, J. C. (1991). Algebraic solution for geometry from dimensional constraints. In *Proceedings of the First ACM Symposium on Solid Modeling Foundations and CAD/CAM Applications* (pp. 397-407). ACM.

**Method.** Owen pioneered top-down recursive decomposition of the 2D constraint graph. The graph (points and lines linked by distance and angle constraints) is recursively split at its articulation pairs, the separating pairs of vertices that, when removed, disconnect the graph. This is precisely the triconnected-component decomposition of Hopcroft and Tarjan (1973), and Owen's algorithm runs in quadratic time using their linear triconnectivity routine inside the recursion. Each split produces subproblems that share the separating pair; subproblems are solved recursively and then merged by aligning shared geometry. Owen's key theoretical insight ties solvability to field theory: a configuration is constructible by ruler and compass (solvable through nested quadratic field extensions, hence in closed form) exactly when the decomposition terminates in primitive triangle-like clusters. A graph that remains triconnected after decomposition represents a configuration with no quadratic-field solution and so cannot be solved by construction alone.

**Limitations.** Restricted to points and lines with distance/angle constraints in its original form; circles, tangencies, and higher entities need extensions. Triconnected cores (for example the classic "double-banana" analog or a pentagon of distances) fall outside the constructible class and must drop to numerical methods. It is a structural method, so it cannot see non-structural (theorem-induced) dependencies.

**Kernel relevance.** Owen is the canonical reference for the "decompose then construct" philosophy that any serious sketcher should adopt before invoking Newton. For Keel's above-kernel sketcher, top-down triconnected decomposition is a proven, patent-expired baseline that gives O(n^2) analysis and closed-form solutions for the overwhelming majority of real sketches.

### Fudos & Hoffmann (1997): bottom-up cluster rewriting

**Citation.** Fudos, I., & Hoffmann, C. M. (1997). A graph-constructive approach to solving systems of geometric constraints. *ACM Transactions on Graphics, 16*(2), 179-216.

**Method.** Where Owen splits from the top, Fudos and Hoffmann build from the bottom. The solver runs in two phases. The analysis phase examines the constraint graph and derives a sequence of elementary construction steps; the construction phase executes them numerically. The atom of the method is the *cluster*: a rigid body with three DOF in the plane. Initially a maximal set of minimal clusters is formed, each cluster being two geometric elements joined by one constraint. The core rewrite rule is the "merge three clusters that pairwise share one geometric element into one cluster," which corresponds to solving a triangle. The process iterates, recombining clusters until either a single cluster remains (well-constrained) or no rule applies (under-constrained, or a non-tractable core). The authors formalize this as a rewrite system over sets of clusters and prove its correctness and confluence properties. The framework explicitly handles well-, over-, and under-constrained inputs and reports which is which.

**Limitations.** The basic cluster merge captures only "tree-decomposable" or "triangle-decomposable" problems; it cannot resolve graphs whose minimal rigid core needs more than three clusters merged simultaneously (these need generalized cluster merges or numerical fallback). Like Owen it is structural and blind to geometric-theorem dependencies.

**Kernel relevance.** The cluster abstraction (a rigid frame with a known transform) maps cleanly to a Rust data model: a `Cluster { entities, local_coords, placement }`. The two-phase split (analyze once, then replay construction cheaply on parameter change) is exactly what a parametric history rebuild wants. This is the most implementation-friendly decomposition paper and a strong candidate to anchor Keel's sketcher.

### Hoffmann, Lomonosov & Sitharam: bottom-up DR-planning via network flow

**Citation.** Hoffmann, C. M., Lomonosov, A., & Sitharam, M. (2001). Decomposition plans for geometric constraint systems, Part I: Performance measures for CAD. *Journal of Symbolic Computation, 31*(4), 367-408. Companion: Part II: New algorithms, *Journal of Symbolic Computation, 31*(4), 409-427. See also Hoffmann, Lomonosov & Sitharam, "Finding solvable subsets of constraint graphs," in *Principles and Practice of Constraint Programming (CP 1997)*, Springer LNCS 1330.

**Method.** This line generalizes both Owen and Fudos-Hoffmann into a unified theory of decomposition-recombination (DR) planning. A DR-plan is a sequence of graph transformations, each isolating a solvable (generically rigid) subgraph, replacing it with a simplified abstraction, and recursing. The engine is a network-flow degree-of-freedom analysis: the constraint graph is turned into a flow network where vertex weights encode entity DOF and edge weights encode constraint DOF, and a max-flow computation isolates a "dense" subgraph (one whose constraint count exceeds the rigidity threshold). The flagship algorithm isolates such a subgraph in O(n(m+n)) time. Part I is the more cited contribution: it defines formal performance measures by which any DR-planner can be judged, including generality, validity, completeness, the Church-Rosser (confluence) property, complexity, best/worst-choice approximation factors, solvability preservation, ability to handle under-constrained systems, and ability to honor designer-specified decompositions.

**Limitations.** Flow-based DOF analysis is still a structural/generic test; it can be fooled by special-position degeneracies and theorem-induced dependencies (the standard caveat that "generic rigidity is not actual rigidity"). The optimal DR-planning problem is in general hard, so practical planners are approximations.

**Kernel relevance.** Provides the rigorous yardstick for evaluating whatever decomposition Keel ships, and the max-flow DOF isolator is the recommended tool for diagnosing the offending subgraph when a sketch is reported over-constrained. The performance-measure checklist is a ready-made acceptance-test rubric for Keel's solver.

## 1.2 Degrees-of-freedom analysis and rigidity

### Laman's theorem (combinatorial 2D rigidity)

**Citation.** Laman, G. (1970). On graphs and rigidity of plane skeletal structures. *Journal of Engineering Mathematics, 4*(4), 331-340. Modern treatment: Graver, Servatius & Servatius, *Combinatorial Rigidity* (AMS, 1993); and the rigidity chapters of the Sitharam-St.John-Sidman handbook below.

**Method.** Laman gives the exact combinatorial criterion for when a generic bar-joint framework in the plane is minimally rigid. A graph with n vertices is generically minimally rigid (isostatic) iff it has exactly m = 2n - 3 edges and every subset of n' vertices spans at most m' <= 2n' - 3 edges. The intuition: 2n coordinates minus 3 rigid-motion DOF (two translations, one rotation) gives 2n - 3 internal DOF that the edges must pin. The subgraph inequality forbids any region from being over-braced while another stays floppy. Laman counts are checked efficiently by the "pebble game" algorithm (Jacobs & Hendrickson, 1997), which runs in O(n^2) and simultaneously identifies rigid components and redundant edges.

**Limitations.** Laman holds only in 2D. There is no known purely combinatorial characterization of generic rigidity in 3D (the open Maxwell count 3n - 6 is necessary but not sufficient; the double-banana graph is the famous counterexample). And generic rigidity ignores special positions: a Laman-rigid graph can still be flexible at a degenerate placement.

**Kernel relevance.** Laman plus the pebble game is the theoretically clean foundation for Keel's 2D DOF accounting and over/under-constraint diagnosis. For 3D assembly mating, the absence of a Laman analog is a warning: 3D constraint diagnosis must lean on numerical rank tests (witness method, QR) rather than pure graph counting.

### Latham & Middleditch: flow-based connectivity analysis

**Citation.** Latham, R. S., & Middleditch, A. E. (1996). Connectivity analysis: a tool for processing geometric constraints. *Computer-Aided Design, 28*(11), 917-928.

**Method.** Latham and Middleditch recast DOF analysis as a connectivity/matching problem on a bipartite graph of constraints versus the entity-parameters they consume. They form a generalized maximum matching (sometimes labeled the "MM" method) and run a connectivity (max-flow) analysis to isolate solvable subgraphs, then read off a construction sequence. Their algorithm both finds rigid structure and pinpoints under-constrained (deficient matching) and over-constrained (surplus) portions. This is the conceptual ancestor of the Hoffmann-Lomonosov-Sitharam flow approach.

**Limitations.** Structural only; same genericity caveats. The construction sequence it yields is not guaranteed minimal, and ambiguous matchings can pick a poor decomposition.

**Kernel relevance.** A compact, directly implementable flow algorithm for DOF bookkeeping in the sketcher; useful as the diagnostic layer that highlights the specific dragging handle a user still has free, or the redundant dimension to delete.

### Over/under-constrained diagnosis (Sitharam, Joan-Arinyo)

**Citation.** Joan-Arinyo, R., Soto-Riera, A., Vila-Marta, S., & Vilaplana-Pasto, J. (2003). Transforming an under-constrained geometric constraint problem into a well-constrained one. In *Proceedings of the Eighth ACM Symposium on Solid Modeling and Applications* (pp. 33-44). ACM. See also Sitharam et al. on tree-decomposable and under-constrained problems (arXiv:1608.05205).

**Method.** This body of work formalizes the three constraint states (under-, well-, over-) and the operations that move between them. For under-constrained sketches, Joan-Arinyo et al. give a procedure to *complete* the constraint set by adding constraints that make the system well-constrained without introducing over-constraint, with the guarantee that if every well-constrained subproblem was solvable, the completed problem is solvable. The broader research program targets optimal decomposition: split a non-well-constrained system into minimal over-constrained parts and maximal well-constrained parts, and split a well-constrained system into minimal well-constrained parts.

**Limitations.** Completion is not unique; choosing which constraints to add to match design intent is heuristic. Structural diagnosis again misses theorem-induced redundancy.

**Kernel relevance.** Directly informs the sketcher UX: how to color geometry by constraint state, how to auto-suggest dimensions to fully constrain a sketch, and how to localize a conflict. These are table-stakes features for a Parasolid-class product.

## 1.3 Numerical methods

When decomposition bottoms out at a non-constructible rigid core, or when the user wants a single variational solve, the system reduces to F(x) = 0, a square (or rectangular) nonlinear system, and numerical iteration takes over.

### Newton-Raphson and quasi-Newton (BFGS), trust-region (Dogleg, Levenberg-Marquardt)

**Citation.** Standard numerical optimization; canonical reference Nocedal, J., & Wright, S. (2006). *Numerical Optimization* (2nd ed.). Springer. As applied to GCS see the FreeCAD GCS solver (Section 1.7) and the review by Cao, Li & Liu (2022), arXiv:2202.13795.

**Method.** Constraints become residual equations; the Jacobian is the matrix of partial derivatives of residuals with respect to entity parameters. Newton-Raphson takes the step x <- x - J^{-1} F(x) and converges quadratically near a root, but only with a good initial guess (the user's rough sketch provides exactly that). For robustness, production solvers wrap Newton in globalization: Levenberg-Marquardt damps the normal equations (J^T J + lambda I) to interpolate between Gauss-Newton and gradient descent and survive rank-deficient Jacobians; the Dogleg trust-region method blends the Cauchy (steepest-descent) step with the Newton step inside a trust radius; BFGS is a quasi-Newton fallback that builds an approximate inverse Hessian from gradients alone, avoiding explicit second derivatives. Over-determined systems are solved in the least-squares sense.

**Limitations.** Local methods find one root and may converge to an unintended branch (the chirality problem), stall in local minima, or diverge if the start point is poor. They give no global picture of the solution variety.

**Kernel relevance.** A hybrid is mandatory: decompose structurally first, then solve each irreducible cluster numerically. Keel should expose a small numerical core (Newton with Levenberg-Marquardt or Dogleg globalization plus a BFGS fallback), exactly the algorithm menu FreeCAD's GCS exposes.

### Homotopy / continuation methods

**Citation.** For the GCS application see Lamure, H., & Michelucci, D. (1996). Solving geometric constraints by homotopy. *IEEE Transactions on Visualization and Computer Graphics, 2*(1), 28-34. General theory: Allgower & Georg, *Numerical Continuation Methods* (Springer, 1990); Sommese & Wampler, *The Numerical Solution of Systems of Polynomials* (World Scientific, 2005).

**Method.** Continuation deforms an easy start system G(x)=0, whose solutions are known, into the target F(x)=0 along a parameterized homotopy H(x,t) = (1-t)G(x) + t F(x), tracking solution paths from t=0 to t=1 with predictor-corrector steps. Unlike Newton, it can in principle enumerate all roots of a polynomial system, giving access to multiple solution branches. Parameter and "cheater" homotopies reuse solutions from a generic parameter set to re-solve a structurally identical system cheaply, which is attractive for parametric rebuilds where only dimension values change.

**Limitations.** Cost grows with the number of paths (the Bezout or BKK bound), which can be exponential; many paths diverge or are spurious. Heavyweight relative to Newton for the single-solution case that interactive sketching usually wants.

**Kernel relevance.** Niche but valuable. Continuation is the principled way to recover from chirality flips during parametric replay: track the path from the previous solved state to the new parameter values so the solver lands on the same branch the user originally chose, rather than jumping to a mirror configuration.

### Witness configuration method (Michelucci & Foufou)

**Citation.** Michelucci, D., & Foufou, S. (2006). Geometric constraint solving: the witness configuration method. *Computer-Aided Design, 38*(4), 284-299. Follow-ups: Michelucci & Foufou, "Detecting all dependences in systems of geometric constraints using the witness method," *ADG 2008*, Springer LNCS 6301; Thierry, Schreck, Michelucci, Funfzig & Genevaux, "Extensions of the witness method to characterize under-, over- and well-constrained geometric constraint systems," *Computer-Aided Design, 43*(10), 2011.

**Method.** The witness method sidesteps the central weakness of graph methods: their blindness to dependencies caused by geometric theorems (for example, the three altitudes of a triangle are concurrent, so a constraint asserting concurrency is redundant although the graph looks fine). The idea is to compute a *witness*: any easy-to-find configuration that satisfies the same incidence structure as the target, even if it does not satisfy the actual dimension values. At the witness one evaluates the constraint Jacobian and analyzes it with linear algebra. The rank of the Jacobian, and the basis of its null space (the free infinitesimal motions), reveal the true DOF and expose every dependence, structural and non-structural alike, because a generic witness shares the target's Jacobian rank structure. Comparing the Jacobian rank against constraint and parameter counts classifies the system as under-, well-, or over-constrained.

**Limitations.** Requires constructing a valid witness, which itself can be nontrivial for incidence-heavy systems; later work (arXiv:1904.00526) catalogs cases where witness construction or genericity assumptions break down. It detects dependence at the witness, which is generically but not universally the same as at the target (special positions can still differ).

**Kernel relevance.** This is the recommended over-constraint detector for Keel, far superior to pure graph counting. A single numerical rank computation at a witness gives a trustworthy well/over/under verdict and identifies the redundant constraints to highlight, including the subtle theorem-induced ones that frustrate users of weaker sketchers.

## 1.4 Symbolic and algebraic approaches

**Citation.** Kondo, K. (1992) and the symbolic-numeric surveys; representative: Durand, C., & Hoffmann, C. M. (2000). A systematic framework for solving geometric constraints analytically. *Journal of Symbolic Computation, 30*(5), 493-519.

**Method.** Pure algebraic methods translate constraints into polynomial systems and attack them with Grobner bases, resultants, or Wu-Ritt characteristic sets, yielding exact symbolic solutions and the ability to reason about solvability and number of solutions. Durand and Hoffmann give a systematic framework for solving the small irreducible clusters left by decomposition analytically.

**Limitations.** Grobner-basis computation has doubly-exponential worst-case complexity; usable only on the tiny clusters that decomposition isolates, never on a whole sketch. Symbolic output can be unwieldy.

**Kernel relevance.** Confirms the dominant architecture is hybrid symbolic-numeric: use graph decomposition plus (optionally) analytic solutions for the standard small clusters (triangle, point-on-line, line-tangent-to-two-circles), and numerical iteration only for the residual hard cores.

## 1.5 3D constraint solving for assemblies

### Kramer: degrees-of-freedom analysis (TLA-style)

**Citation.** Kramer, G. A. (1992). *Solving Geometric Constraint Systems: A Case Study in Kinematics*. MIT Press. Companion paper: Kramer, G. A. (1991). Using degrees of freedom analysis to solve geometric constraint systems. In *Proceedings of the First ACM Symposium on Solid Modeling Foundations and CAD/CAM Applications* (pp. 371-378).

**Method.** Kramer solves 3D mechanisms and assemblies by reasoning symbolically about DOF rather than crunching algebra. Each rigid body starts with six DOF; each mate constraint (coincidence, in-plane, parallel-axis, distance) removes specific DOF and is satisfied by an explicit incremental construction (a "plan fragment") that places one body relative to another. The solver maintains a "locus" of allowed positions for each body and intersects loci as constraints are added (the metaphor: a body whose center must lie on a sphere and on a plane has its locus reduced to a circle). This degrees-of-freedom-analysis (DOF) approach produces a procedure, not just a solution, and naturally handles articulated kinematic loops. It is sometimes grouped with "TLA" (the Locus/Analysis) reasoning style.

**Limitations.** The library of canned plan fragments must cover every constraint combination; novel combinations or genuinely coupled loops fall back to numerics. Works best for the well-constrained assembly-mating regime, less so for free variational 3D sketching.

**Kernel relevance.** This is the assembly-mating playbook. If Keel grows an assembly layer (bodies positioned by mates), Kramer's DOF reasoning is the proven, interactive-speed approach, and it sits clearly above the B-rep kernel: it manipulates rigid-body transforms, not topology. It validates treating assembly constraint solving as a distinct module that calls the kernel only for the bodies it positions.

## 1.6 Variational vs procedural semantics; solution selection and chirality

**Citation.** Bettig, B., & Hoffmann, C. M. (2011). Geometric constraint solving in parametric computer-aided design. *Journal of Computing and Information Science in Engineering, 11*(2), 021001. Historical lineage from Sutherland, I. (1963). *Sketchpad: A Man-Machine Graphical Communication System* (MIT PhD thesis).

**Method.** Two CAD modeling paradigms are contrasted. *Procedural* (history-based, generative) modeling records an ordered list of operations; geometry is whatever replaying the script produces. *Variational* (constraint-based) modeling states relations declaratively and lets a solver find any configuration satisfying them simultaneously; this is the Sketchpad lineage. Most commercial CAD is hybrid: a procedural feature history in which each sketch feature is solved variationally. A core subtlety is *solution selection*: a well-constrained system typically has multiple real solutions (a distance-plus-distance intersection has two; a tangency configuration can have four). The solver must pick the branch matching user intent, the *chirality* or *root selection* problem. Heuristics include "stay nearest to the current sketch positions" (the user's rough drawing disambiguates), preserving orientation/handedness across rebuilds, and recording the chosen branch as extra selector data in the feature. Constructive (decomposition) solvers select per-construction-step (which intersection point of two circles), which is more controllable than numerical solvers, where the start point implicitly selects the branch.

**Limitations.** No selection heuristic is foolproof; large parameter changes can cross branch boundaries and flip the result (the dreaded "sketch turned inside out on rebuild"). Recording selectors couples the sketch to its solve history.

**Kernel relevance.** Drives two design rules for Keel: (1) the sketcher must persist branch-selection data so parametric rebuilds are deterministic, and (2) prefer a constructive solver where each step's root choice is explicit and storable, falling back to "nearest-to-current" for the numerical core. Continuation (Section 1.3) is the rigorous backstop for branch continuity.

## 1.7 Commercial and open-source solver landscape

### D-Cubed DCM (Siemens)

**Citation.** Siemens Digital Industries Software. *D-Cubed 2D DCM* and *D-Cubed 3D DCM* product documentation. https://www.siemens.com/en-us/products/plm-components/d-cubed/

**Method (as published).** D-Cubed 2D DCM (Dimensional Constraint Manager) is the most widely licensed commercial 2D geometric constraint solver, embedded in numerous CAD/CAM/CAE products as the sketcher engine. Published capability: it operates on points, lines, circles, ellipses, conics, splines, and general parametric curves, with constraints including distance, angle, radius, parallel, perpendicular, tangent, concentric, symmetric, normal, and equal distance/radius. Spline-specific constraints cover spline length, tangent direction and length, second derivative, equal direction, and equal curvature. Variable dimensions can be linked by equations solved simultaneously. It supports symmetry and linear/circular patterns of repeated elements. Critically, it handles well-defined, over-defined, and under-defined models and always reports sketch status (which geometry is under- or over-constrained). The D-Cubed family also includes 3D DCM (the 3D assembly/variational solver), AEM (Assembly Engineering Manager, collision/clearance and contact-based positioning), CDM (Collision Detection Manager), PGM (Profile Geometry Manager), and HLM (Hidden Line Manager). D-Cubed sits beside, not inside, the geometric kernel (Parasolid).

**Limitations (inferred).** Closed-source and commercial; internal algorithms are not published, so it serves as a capability target rather than an implementation blueprint.

**Kernel relevance.** D-Cubed is the explicit architectural model for Keel's intended boundary: a separately licensed constraint solver beside the kernel. Its published feature list defines the bar for a "Parasolid-class" sketcher. Notably, the existence of PGM (Profile Geometry Manager) as a separate D-Cubed component validates treating 2D profile geometry (Part 2) as its own module distinct from both the solver and the 3D kernel.

### SolveSpace solver (SLVS)

**Citation.** Westhues, J., and contributors. *SolveSpace* technical documentation. https://solvespace.github.io/solvespace-web/tech.html (solver also distributed standalone as the `slvs` library).

**Method.** SolveSpace represents every constraint as an equation in a small symbolic-algebra system and solves the assembled system numerically by a *modified Newton's method*. Special cases that can be solved by forward substitution, or that are soluble alone, are handled directly rather than iterated. The distinctive feature is its handling of under-constrained sketches: rather than adding artificial "weak dimensions," it solves the Jacobian in a least-squares sense, with each equation written so that the minimized quantity is a meaningful penalty metric. This yields "less surprising" dragging behavior. When a sketch is genuinely incompatible the solver fails to converge (correctly flagging the conflict), and when multiple solutions exist the initial sketch position selects the branch. Constraints are 3D by default with optional projection into a workplane. The solver is fully 3D, which is unusual; most sketchers are strictly 2D.

**Limitations.** Pure numerical (no graph decomposition), so diagnosis of *which* constraints conflict is weaker than decomposition-based or witness-based solvers; large systems lean entirely on Newton convergence.

**Kernel relevance.** SLVS is the most directly studyable open-source solver and proves a small symbolic-plus-Newton core can drive a real 3D parametric sketcher. Its least-squares treatment of under-constrained drag is a UX pattern Keel should copy. Its permissive licensing and clean C API make it a candidate reference (or even an interim dependency) for Keel's sketcher.

### FreeCAD GCS / PlanGCS

**Citation.** FreeCAD contributors. *Generic Constraint Solver (GCS)*, Sketcher workbench. Source `planegcs`; WASM wrapper at github.com/Salusoft89/planegcs; architecture summary at deepwiki.com/FreeCAD/FreeCAD constraint-system-and-gcs-solver.

**Method.** FreeCAD's GCS is a numerical 2D solver offering a menu of algorithms: DogLeg (default trust-region), Levenberg-Marquardt (damped least squares, robust to local minima), BFGS (quasi-Newton with approximated Hessian), and SQP for temporary constraints during dragging. Each constraint implements `calcGrad()` to supply partial derivatives, populating the Jacobian. Diagnosis is rank-based: the solver computes the rank of the Jacobian (via QR-style factorization); if rank is less than the constraint count, redundancies or conflicts exist. It distinguishes return codes for *redundant* constraints (gradient linearly dependent on others but residual zero), *conflicting* constraints (targets unsatisfiable together), and *malformed* constraints (invalid parameters, for example a zero-length line for a tangent). It solves partial subsystems and reports remaining DOF.

**Limitations.** Primarily numerical with limited structural decomposition, so very large or pathological sketches can be slow or converge to unintended branches; documented cases exist where one algorithm succeeds where another fails (hence the user-selectable solver menu).

**Kernel relevance.** GCS is the best open-source template for Keel's numerical core specifically: the algorithm trio (DogLeg, Levenberg-Marquardt, BFGS), the per-constraint gradient interface, and the rank-based redundant/conflicting/malformed taxonomy are all directly portable to a clean Rust design.

## 1.8 Surveys and handbooks

### Hoffmann & Joan-Arinyo: handbook chapter / brief

**Citation.** Hoffmann, C. M., & Joan-Arinyo, R. (2005). A brief on constraint solving. *Computer-Aided Design and Applications, 2*(5), 655-663. Their longer treatment appears as the geometric-constraint-solving chapter in the *Handbook of Computer Aided Geometric Design* (Farin, Hoschek & Kim, eds., Elsevier, 2002).

**Method.** A compact, authoritative survey organizing the field into graph-based constructive solvers (Owen, Fudos-Hoffmann, DR-planning), algebraic/symbolic solvers, and numerical solvers, with discussion of 2D versus 3D and the constraint-state (well/over/under) taxonomy. It introduces the s-tree formalism for top-down decomposition and contrasts the constructive and numerical philosophies.

**Limitations.** A survey, so light on implementation detail; predates the maturation of the witness method's later extensions.

**Kernel relevance.** The single best orientation document; its taxonomy should structure Keel's solver design spec.

### Sitharam, St. John & Sidman: Handbook of Geometric Constraint Systems Principles

**Citation.** Sitharam, M., St. John, A., & Sidman, J. (Eds.). (2018). *Handbook of Geometric Constraint Systems Principles*. Discrete Mathematics and Its Applications. Chapman and Hall/CRC.

**Method.** The definitive reference compendium. Part one (chapters 2-7) covers geometric reasoning and automated theorem proving via algebraic methods; later parts (chapters 13-17) treat the rigidity and flexibility theory of constraint systems with emphasis on bar-joint frameworks (Laman, the pebble game, generic rigidity, matroid structure of rigidity). It unifies the CAD-engineering and the discrete-mathematics communities' views of the same problem.

**Limitations.** Mathematically deep, oriented to theory rather than to building a shipping solver; CAD-specific algorithmic detail (chirality, parametric replay) is comparatively thin.

**Kernel relevance.** The authoritative source for the rigidity-theory foundations behind DOF analysis and over/under-constraint diagnosis. Keel's solver-correctness reasoning should cite this for the Laman/pebble-game machinery.

### A review on geometric constraint solving (recent survey)

**Citation.** Cao, C., Li, H., & Zhang, J. (2022). A review on geometric constraint solving. arXiv:2202.13795.

**Method.** An up-to-date taxonomy spanning graph decomposition (top-down Owen, bottom-up clusters, DR-planning), DOF/rigidity analysis, numerical methods (Newton, homotopy, optimization-based), the witness method, and learning-based approaches, with a comparison of strengths and failure modes.

**Limitations.** Survey-level; broad rather than deep on any one method.

**Kernel relevance.** The most current single-stop literature map; useful to confirm Keel's chosen architecture is consistent with the present state of the art and to scout emerging (for example, neural) directions.

---

# Part 2: 2D Profile Computational Geometry

Once a sketch's geometry exists (whether constraint-solved or not), the kernel needs clean planar regions to extrude, revolve, or sweep, and it needs robust planar booleans and offsets to support fillets, shells, draft, and pocket operations on profiles. This is a separate computational-geometry problem from constraint solving and arguably belongs *inside* the kernel.

## 2.1 Arrangements of curves in the plane

### CGAL 2D Arrangements

**Citation.** Wein, R., Fogel, E., Zukerman, B., & Halperin, D., with Berberich, E., Hemmer, M., & Salzman, O. (2007 onward). 2D Arrangements. In *CGAL User and Reference Manual*. https://doc.cgal.org/latest/Arrangement_on_surface_2/. Foundational paper: Wein, Fogel, Zukerman & Halperin, "Advanced programming techniques applied to CGAL's arrangement package," *Computational Geometry: Theory and Applications, 38*(1-2), 2007. Sweep on surfaces: Berberich, Fogel, Halperin, Mehlhorn & Wein, "Sweeping and maintaining two-dimensional arrangements on surfaces," *ESA 2007*, Springer LNCS 4698.

**Method.** A 2D arrangement is the subdivision of the plane induced by a set of curves into vertices, edges, and faces, stored in a doubly-connected edge list (DCEL). CGAL's package constructs and maintains arrangements via two algorithmic frameworks: a Bentley-Ottmann style *sweep-line* (for batch construction) and *zone computation* (for incremental insertion). The package is generic over a *geometry-traits* concept that supplies the curve-specific predicates and constructions: comparing points, splitting curves into x-monotone subcurves, and intersecting subcurves. Concrete traits exist for line segments, polylines, conic arcs (including circular arcs and ellipse arcs), arcs of rational functions, algebraic curves, and Bezier curves. Robustness and exactness come from instantiating the traits with exact number types so all predicates are evaluated without round-off error, guaranteeing topologically consistent output.

**Limitations.** Exact arithmetic is slow relative to floating point; Bezier and algebraic traits are computationally heavy. C++/template-heavy; not directly reusable from Rust without an FFI bridge or reimplementation. Output complexity can blow up with many intersections.

**Kernel relevance.** The gold-standard model of what a robust planar-arrangement layer must do, and the proof that a *traits-parameterized* design (one sweep engine, pluggable curve predicates) is the right abstraction. Keel's 2D profile layer should mirror this: a generic arrangement/sweep core with per-curve-type predicate traits for line, arc, ellipse, and (later) NURBS segments. CGAL also defines the exact-predicate bar Keel's profiles must meet for line/arc inputs.

### CGAL 2D Regularized Boolean Set-Operations (curved generalization)

**Citation.** Fogel, E., Wein, R., & Halperin, D. 2D Regularized Boolean Set-Operations. In *CGAL User and Reference Manual*. https://doc.cgal.org/latest/Boolean_set_operations_2/.

**Method.** Built directly on the arrangement package, this package performs regularized intersection, union, difference, and symmetric difference on *general polygons* whose boundaries are x-monotone subcurves of arbitrary curve types, not just straight segments. The `General_polygon_set_2<Traits, Dcel>` class internally maintains an arrangement; supplying the conic-arc traits gives booleans on circular/elliptic-arc regions, and supplying `Arr_Bezier_curve_traits_2` (via the `Gps_traits_2` adapter) gives booleans on Bezier-bounded regions. Bezier and conic boundaries are subdivided into x-monotone pieces as the adapter requires. "Regularized" means the result is the closure of the interior, discarding lower-dimensional dangling features.

**Limitations.** Inherits the exact-arithmetic cost and C++ template complexity of the arrangement layer.

**Kernel relevance.** This is the curved-boolean capability Keel needs for profiles bounded by line+arc (and eventually spline) loops, and it demonstrates that the *same* arrangement engine yields both region extraction and planar booleans. Strong argument for building one robust arrangement core in Keel and deriving booleans, region-finding, and offset-cleanup from it.

## 2.2 Region/loop detection from sketch curves

### Face extraction from a planar graph (Jiang-Bunke, Eberly)

**Citation.** Jiang, X. Y., & Bunke, H. (1993). An optimal algorithm for extracting the regions of a plane graph. *Pattern Recognition Letters, 14*(7), 553-558. Eberly, D. Constructing a cycle basis for a planar graph (Geometric Tools technical note). Practitioner writeup: Arcol engineering blog, "Extracting faces from graphs," https://blog.arcol.io/pen-tool.

**Method.** Turning a "curve soup" (a set of sketch curves with arbitrary crossings) into faces is a planar-subdivision problem. The pipeline: (1) insert vertices at all pairwise intersections (Bentley-Ottmann for segments, subdivision-then-intersection for arcs); (2) build a planar straight-line graph (PSLG), approximating arcs by polylines for consistent angular ordering; (3) extract minimal cycles by the Jiang-Bunke method, which radially sorts the edges around each vertex and, at every vertex, pairs incoming and outgoing half-edges into "wedges," then walks the always-turn-most-clockwise (or counter-clockwise) next-edge to trace each minimal face boundary in O(n log n); (4) split non-simple cycles at repeated vertices into simple loops; (5) build the containment hierarchy (which loops are holes of which faces) using signed area (the shoelace formula gives winding/orientation) plus point-in-polygon tests in a DFS; (6) restore the original arc primitives from the subdivision mapping. The half-edge / DCEL structure and the "sort by angle, follow twin/next" traversal are the core ideas.

**Limitations.** Arc-to-polyline subdivision is a tradeoff: too coarse creates spurious intersections, too fine slows everything. Robustness hinges on correct, consistent intersection computation; floating-point angle sorting at near-coincident vertices is fragile and usually needs exact or snapped predicates.

**Kernel relevance.** This is the exact algorithm Keel needs to turn sketch loops into extrudable regions, including correct hole/island nesting. The half-edge traversal with angular sorting, signed-area orientation, and containment tree is directly implementable in Rust over a DCEL. The arc-handling caveat motivates a native arc-aware sweep (per CGAL) rather than naive polyline approximation.

### Winding rules for region interior

**Citation.** Standard fill-rule theory (nonzero winding vs even-odd); see Foley, van Dam et al., *Computer Graphics: Principles and Practice*, and the OpenGL/GLU tessellator documentation.

**Method.** When sketch loops self-intersect or nest ambiguously, a fill rule decides which subregions are "solid." The *nonzero winding* rule counts signed crossings of a ray from a test point; nonzero means inside. The *even-odd* rule counts parity. CAD profiles typically use nonzero winding with consistently oriented loops (outer CCW, holes CW), matching B-rep face-loop conventions (one CCW outer loop, inner loops CW).

**Kernel relevance.** Keel must adopt a definite, documented winding convention for profiles and align it with the kernel's face-loop orientation so that region extraction, booleans, and the eventual B-rep face all agree on what is material.

## 2.3 Planar booleans on curved regions

**Citation.** Greiner, G., & Hormann, K. (1998). Efficient clipping of arbitrary polygons. *ACM Transactions on Graphics, 17*(2), 71-83. Vatti, B. R. (1992). A generic solution to polygon clipping. *Communications of the ACM, 35*(7), 56-63. Weiler, K. (1980). Polygon comparison using a graph representation. *SIGGRAPH '80*. Degeneracy-robust extension: Foster, Hormann & Popa (2019), and Erich & Hormann, "Clipping of arbitrary polygons with degeneracies" (arXiv:1211.3376).

**Method.** Three classic lineages compute polygon booleans (union, intersection, difference, XOR). *Greiner-Hormann* walks both polygons, inserts intersection vertices into linked lists of both, marks each as entry/exit by an inside test, then traces the result by hopping between the two lists at intersections; it is compact and handles non-convex polygons. *Vatti* is a sweep-line/scanline algorithm tracking active edges and "bound" pairs, robust to self-intersection and the basis of the Clipper library. *Weiler-Atherton* uses a graph of polygon boundaries and is the ancestor of B-rep boolean traversal. The crucial point for CAD: all three generalize to *curved* segments provided you supply a pairwise curve-curve intersection routine and a curve-aware point-in-region test. Greiner-Hormann in particular needs only its collinearity check and inside test swapped for curve versions (for example, insideness via Bezier subdivision), so arcs and Beziers can be clipped directly.

**Limitations.** Greiner-Hormann's original form mishandles degeneracies (vertices exactly on edges, overlapping edges), needing the later perturbation/labeling extensions. Curve-curve intersection can produce doubled or spurious intersections (especially Bezier-Bezier near-tangencies), demanding careful tolerancing. Vatti/Clipper natively handle only linear edges, so arcs are approximated unless extended.

**Kernel relevance.** Keel's profile booleans (needed for combined sketch regions, pockets, and trimming) should follow the arrangement-based approach (Section 2.1) for full curve support and robustness, but the Greiner-Hormann curved generalization is a lighter-weight alternative for the common line+arc case. The degeneracy literature is required reading: real sketches are full of coincident endpoints and tangent arcs.

## 2.4 2D curve offsetting

### Clipper / Clipper2 (Vatti-based polygon offsetting and clipping)

**Citation.** Johnson, A. *Clipper2: Polygon Clipping, Offsetting and Triangulation*. https://angusj.com/clipper2/ and github.com/AngusJohnson/Clipper2. Built on Vatti (1992); offsetting informed by Chen & McMains (2005, below).

**Method.** Clipper2 performs boolean clipping (intersection/union/difference/XOR) and polygon offsetting (inflate/deflate) on integer-coordinate polygons, with support for simple and complex (self-intersecting, holed) inputs and Delaunay triangulation. Clipping uses an extended Vatti sweep. Offsetting builds a "raw" offset by displacing each edge by the offset distance and joining with the chosen join type (miter, round, square), which generally self-intersects, then cleans the raw curve. It uses integer coordinates internally for exact, deterministic results, requiring callers to scale floating-point geometry to a fixed grid.

**Limitations.** Polygon-only: arcs must be flattened to line segments (round joins are approximated by polylines), so it is not an exact arc-line offsetter. Integer coordinates impose a fixed precision/scale choice. No native curved-segment support.

**Kernel relevance.** Clipper2 is a battle-tested, permissively licensed (Boost license) reference and a viable interim dependency for polygonal profile offsetting (shells, thin-wall, tool-path-like operations). Its integer-grid robustness strategy is instructive. But because CAD profiles carry true arcs, Keel ultimately needs an exact arc-line offsetter beyond what Clipper provides.

### Polygon offsetting by computing winding numbers (Chen & McMains)

**Citation.** Chen, X., & McMains, S. (2005). Polygon offsetting by computing winding numbers. In *Proceedings of ASME IDETC/CIE 2005* (Paper DETC2005-85513, pp. 565-575).

**Method.** This is the principled way to clean a self-intersecting raw offset. Offset every edge outward (or inward) to form a "raw offset curve," which contains invalid loops where the offset overshoots concave corners or thin regions. Instead of geometric loop-pruning, compute the *winding number* of each connected region of the raw curve's arrangement (the authors feed it to the GLU tessellator with the positive-winding rule). By construction, the invalid self-overlap loops bound regions of non-positive winding and are automatically discarded; the valid offset is exactly the positive-winding region. Runs in O((n+k) log n) time and O(n+k) space, where k is the number of self-intersections. The method is simple and reliably correct.

**Limitations.** As presented, operates on polygonal input (arcs flattened); extending the winding-number cleanup to exact arc segments requires an arc-aware arrangement. Relies on a correct arrangement/tessellation of the raw curve.

**Kernel relevance.** This is the recommended offset-cleanup algorithm for Keel: it is robust, theoretically clean, and reuses the same arrangement-plus-winding machinery already needed for region extraction (Section 2.2) and booleans (Section 2.3). The unifying theme is that arrangement plus winding number solves region-finding, booleans, and offset-trimming with one engine.

### Self-intersection trimming and exact arc-line offsetting

**Citation.** Synthesis of the offsetting literature; see Chen & McMains (2005) above and the offset surveys in the CAD-geometry literature (for example, Held's work on Voronoi-based offsetting).

**Method.** The general 2D offset pipeline: (1) offset each curve primitive (a line offsets to a parallel line; a circular arc offsets to a concentric arc with the radius adjusted by the offset distance, with sign by convexity); (2) fill gaps at convex vertices with arc/round, miter, or bevel joins; (3) the resulting raw offset self-intersects near concave features and thin necks; (4) remove invalid loops via winding-number filtering or explicit self-intersection detection and trimming. Keeping arcs exact (rather than flattening) preserves G1 continuity and keeps file sizes and downstream tessellation clean.

**Kernel relevance.** Keel should implement exact line+arc offsetting (the common CAD profile case) with winding-number trimming, only flattening to polylines as a last resort. This is a kernel-internal capability (fillets, shells, dradt, constant-distance profiles) and belongs inside the kernel, not in the above-kernel sketcher.

## 2.5 Robustness in 2D

### Snap rounding and finite-precision arrangements

**Citation.** Hobby, J. D. (1999, journal; 1993 tech report). Practical segment intersection with finite precision output. *Computational Geometry: Theory and Applications, 13*(4), 199-214. Halperin, D., & Packer, E. (2002). Iterated snap rounding. *Computational Geometry: Theory and Applications, 23*(2), 209-225. Related: Goodrich, Guibas, Hershberger & Tanenbaum on snap rounding; de Berg, Halperin & Overmars on intersection-sensitive snap rounding.

**Method.** Snap rounding converts an arbitrary-precision arrangement of segments into a fixed-precision one that stays topologically consistent. The plane is overlaid with a unit grid of "pixels"; every segment endpoint and every intersection point is snapped to its pixel center, and any segment passing through a "hot" pixel (one containing a vertex) is bent to pass through that pixel's center. This bounds coordinate bit-length while guaranteeing no new crossings appear inconsistently. *Iterated* snap rounding (Halperin & Packer) repeats the process until a guaranteed minimum separation between every vertex and every non-incident edge is achieved, eliminating the residual near-degeneracies that single-pass snap rounding can leave.

**Limitations.** Snapping perturbs geometry (vertices move by up to half a pixel), which can collapse thin features; iterated snap rounding moves geometry more. A precision/fidelity tradeoff that must be tuned to the sketch's tolerance.

**Kernel relevance.** Directly applicable to ingesting messy real-world sketches and imported 2D data, where endpoints almost-but-not-quite coincide. Snap rounding is the pragmatic alternative to full exact arithmetic for Keel's profile cleanup, producing a topologically sound arrangement at controlled precision.

### Exact arithmetic and tolerance-driven merging

**Citation.** The exact-geometric-computation (EGC) paradigm; see CGAL's exact predicate kernels (Section 2.1), and Shewchuk, J. R. (1997), Adaptive precision floating-point arithmetic and fast robust geometric predicates, *Discrete & Computational Geometry, 18*(3).

**Method.** Two complementary robustness strategies. *Exact arithmetic* (rational or adaptive-precision filtered predicates, as in CGAL and Shewchuk's orientation/incircle tests) evaluates the sign-deciding predicates exactly so the combinatorial structure of the arrangement is always correct; floating-point filters keep the common case fast and fall back to exact only near degeneracies. *Tolerance-driven merging* pre-processes the sketch by snapping near-coincident points, collapsing tiny edges, and treating geometry within a model tolerance as identical, which is the approach mainstream CAD kernels (and Parasolid) take. The two can be combined: merge within tolerance first, then run exact predicates on the cleaned input.

**Limitations.** Exact arithmetic costs performance and complicates the number type throughout the geometry stack; tolerance merging is heuristic and can produce inconsistent results if the tolerance is mis-set (the classic source of "kernel could not heal geometry" failures).

**Kernel relevance.** Keel must pick a robustness regime explicitly. A practical recommendation: filtered exact predicates (Shewchuk-style) for the orientation/intersection sign decisions in the arrangement core, plus an explicit model-tolerance for merging near-coincident sketch geometry, matching the Parasolid tolerance-based model. This is a kernel-internal concern that the profile layer must own.

---

# Sketch and profile strategy for Keel

## The in-kernel vs above-kernel boundary (primary recommendation)

The two parts of this dossier fall on opposite sides of the kernel boundary, and the literature supports drawing the line cleanly between them.

**Constraint solving (Part 1) should live ABOVE the kernel, as a peer component.** This mirrors the proven Siemens architecture, where D-Cubed 2D DCM and 3D DCM are licensed and shipped beside Parasolid, not inside it. The constraint solver manipulates entity *parameters* and rigid-body *transforms*; it does not touch B-rep topology. Keeping it separate means (1) the kernel has no dependency on a nonlinear solver, (2) the sketcher can evolve (or be swapped) independently, (3) Keel can ship a usable kernel before the sketcher is mature, and (4) third parties can pair Keel with their own constraint engine. Concretely, Keel should define a thin interface where the sketcher emits *resolved geometry* (a set of curves with final coordinates plus branch-selection metadata) and the kernel consumes it. Kramer's assembly DOF work reinforces this: 3D mating is also a transform-level concern that sits above the topology kernel.

**2D profile geometry (Part 2) should live INSIDE the kernel.** Region extraction, planar booleans, and offsetting are prerequisites for extrude/revolve/sweep and for fillet/shell/draft; they operate on the same robust-geometry substrate (exact/filtered predicates, model tolerance) as the 3D kernel and must share its tolerance regime and number types. CGAL demonstrates that arrangements, booleans, and offset cleanup all derive from one arrangement engine, so this belongs as a kernel-internal module. Notably, D-Cubed's own family separates the Profile Geometry Manager (PGM) from the DCM solver, validating that profile geometry is distinct from constraint solving even within the commercial-component world.

## Recommended solver architecture (Part 1)

1. **Hybrid decompose-then-solve.** Adopt a constructive decomposition front end (Fudos-Hoffmann bottom-up cluster rewriting is the most implementation-friendly; Owen's top-down triconnected decomposition is the alternative) to break the sketch into small rigid clusters, solving the standard clusters by closed-form construction. This gives interactive speed and per-step root control.
2. **Numerical core for residual cores.** For irreducible clusters, use Newton-Raphson with trust-region globalization (Dogleg or Levenberg-Marquardt) and a BFGS fallback, exactly the FreeCAD GCS algorithm set. Solve under-constrained drags in least-squares with a meaningful penalty metric, per SolveSpace, for natural dragging.
3. **Witness-based diagnosis.** Use the Michelucci-Foufou witness configuration method (Jacobian rank at a generic witness) as the authoritative well/over/under-constrained classifier and redundant-constraint detector, backed by Laman/pebble-game structural counts for the fast common case. This beats pure graph counting because it catches theorem-induced redundancy.
4. **Deterministic replay.** Persist branch-selection data per construction step; use homotopy/continuation as the backstop to track the chosen solution branch across parameter changes, avoiding chirality flips on rebuild.
5. **Reference implementations.** SolveSpace (SLVS, permissive) and FreeCAD GCS (LGPL) are the two open codebases to study or interim-adopt; D-Cubed defines the commercial capability bar.

## Recommended profile architecture (Part 2)

1. **One arrangement engine.** Build a single robust planar-arrangement core (sweep-line over a DCEL/half-edge structure, parameterized by per-curve-type predicate *traits* for line, arc, ellipse, later NURBS), following CGAL's design. Derive region extraction, planar booleans, and offset cleanup from this one engine.
2. **Region extraction.** Use Jiang-Bunke face extraction (radial half-edge sort, wedge traversal, signed-area orientation, containment tree) to turn sketch loops into oriented faces with correct hole nesting, native arc handling, and a fixed nonzero-winding fill convention aligned with B-rep loop orientation.
3. **Booleans.** Prefer arrangement-based curved booleans (CGAL Boolean-set-operations model) for full line+arc+spline support; the Greiner-Hormann curved generalization is the lighter fallback for line+arc, with attention to the degeneracy-robust extensions.
4. **Offsetting.** Implement exact line+arc offsetting with winding-number-based self-intersection trimming (Chen-McMains), reusing the arrangement+winding machinery. Clipper2 is a viable interim dependency for purely polygonal offsets and a robustness reference.
5. **Robustness regime.** Use Shewchuk-style filtered exact predicates for the sign decisions in the arrangement core, plus an explicit model tolerance for merging near-coincident sketch geometry (the Parasolid approach), with snap rounding / iterated snap rounding as the cleanup tool for messy imported 2D data.

The unifying insight: a single robust arrangement-plus-winding-number engine inside the kernel powers region finding, booleans, and offsetting, while a separate hybrid decomposition-plus-numerical constraint solver lives above the kernel and feeds it resolved profiles.

---

# References

1. Owen, J. C. (1991). Algebraic solution for geometry from dimensional constraints. *Proc. First ACM Symp. on Solid Modeling Foundations and CAD/CAM Applications*, 397-407.
2. Fudos, I., & Hoffmann, C. M. (1997). A graph-constructive approach to solving systems of geometric constraints. *ACM Transactions on Graphics, 16*(2), 179-216.
3. Hoffmann, C. M., Lomonosov, A., & Sitharam, M. (2001). Decomposition plans for geometric constraint systems, Part I and Part II. *Journal of Symbolic Computation, 31*(4), 367-427.
4. Hoffmann, Lomonosov & Sitharam (1997). Finding solvable subsets of constraint graphs. *CP 1997*, Springer LNCS 1330.
5. Laman, G. (1970). On graphs and rigidity of plane skeletal structures. *Journal of Engineering Mathematics, 4*(4), 331-340.
6. Jacobs, D. J., & Hendrickson, B. (1997). An algorithm for two-dimensional rigidity percolation: the pebble game. *Journal of Computational Physics, 137*, 346-365.
7. Latham, R. S., & Middleditch, A. E. (1996). Connectivity analysis: a tool for processing geometric constraints. *Computer-Aided Design, 28*(11), 917-928.
8. Joan-Arinyo, R., Soto-Riera, A., Vila-Marta, S., & Vilaplana-Pasto, J. (2003). Transforming an under-constrained geometric constraint problem into a well-constrained one. *Proc. Eighth ACM Symp. on Solid Modeling and Applications*, 33-44.
9. Nocedal, J., & Wright, S. (2006). *Numerical Optimization* (2nd ed.). Springer.
10. Lamure, H., & Michelucci, D. (1996). Solving geometric constraints by homotopy. *IEEE TVCG, 2*(1), 28-34.
11. Michelucci, D., & Foufou, S. (2006). Geometric constraint solving: the witness configuration method. *Computer-Aided Design, 38*(4), 284-299.
12. Michelucci, D., & Foufou, S. (2008). Detecting all dependences in systems of geometric constraints using the witness method. *ADG 2008*, Springer LNCS 6301.
13. Thierry, S. E. B., Schreck, P., Michelucci, D., Funfzig, C., & Genevaux, J.-D. (2011). Extensions of the witness method to characterize under-, over- and well-constrained geometric constraint systems. *Computer-Aided Design, 43*(10).
14. Durand, C., & Hoffmann, C. M. (2000). A systematic framework for solving geometric constraints analytically. *Journal of Symbolic Computation, 30*(5), 493-519.
15. Kramer, G. A. (1992). *Solving Geometric Constraint Systems: A Case Study in Kinematics*. MIT Press.
16. Kramer, G. A. (1991). Using degrees of freedom analysis to solve geometric constraint systems. *Proc. First ACM Symp. on Solid Modeling*, 371-378.
17. Bettig, B., & Hoffmann, C. M. (2011). Geometric constraint solving in parametric computer-aided design. *Journal of Computing and Information Science in Engineering, 11*(2), 021001.
18. Sutherland, I. E. (1963). *Sketchpad: A Man-Machine Graphical Communication System*. MIT PhD thesis.
19. Siemens Digital Industries Software. D-Cubed 2D DCM / 3D DCM product documentation. https://www.siemens.com/en-us/products/plm-components/d-cubed/
20. SolveSpace technical documentation. https://solvespace.github.io/solvespace-web/tech.html
21. FreeCAD GCS (PlanGCS) solver. https://github.com/Salusoft89/planegcs and FreeCAD Sketcher docs.
22. Hoffmann, C. M., & Joan-Arinyo, R. (2005). A brief on constraint solving. *Computer-Aided Design and Applications, 2*(5), 655-663.
23. Sitharam, M., St. John, A., & Sidman, J. (Eds.). (2018). *Handbook of Geometric Constraint Systems Principles*. Chapman and Hall/CRC.
24. Cao, C., Li, H., & Zhang, J. (2022). A review on geometric constraint solving. arXiv:2202.13795.
25. Wein, R., Fogel, E., Zukerman, B., & Halperin, D. (2007). Advanced programming techniques applied to CGAL's arrangement package. *Computational Geometry: Theory and Applications, 38*(1-2). CGAL 2D Arrangements manual: https://doc.cgal.org/latest/Arrangement_on_surface_2/
26. Berberich, E., Fogel, E., Halperin, D., Mehlhorn, K., & Wein, R. (2007). Sweeping and maintaining two-dimensional arrangements on surfaces. *ESA 2007*, Springer LNCS 4698.
27. Fogel, E., Wein, R., & Halperin, D. CGAL 2D Regularized Boolean Set-Operations manual. https://doc.cgal.org/latest/Boolean_set_operations_2/
28. Jiang, X. Y., & Bunke, H. (1993). An optimal algorithm for extracting the regions of a plane graph. *Pattern Recognition Letters, 14*(7), 553-558.
29. Eberly, D. Constructing a cycle basis for a planar graph. Geometric Tools technical note. (Practitioner writeup: Arcol, "Extracting faces from graphs," https://blog.arcol.io/pen-tool.)
30. Greiner, G., & Hormann, K. (1998). Efficient clipping of arbitrary polygons. *ACM Transactions on Graphics, 17*(2), 71-83.
31. Vatti, B. R. (1992). A generic solution to polygon clipping. *Communications of the ACM, 35*(7), 56-63.
32. Weiler, K. (1980). Polygon comparison using a graph representation. *SIGGRAPH '80*.
33. Foster, E. L., Hormann, K., & Popa, R. T. (2019 / arXiv:1211.3376). Clipping of arbitrary polygons with degeneracies.
34. Johnson, A. Clipper2: Polygon Clipping, Offsetting and Triangulation. https://angusj.com/clipper2/
35. Chen, X., & McMains, S. (2005). Polygon offsetting by computing winding numbers. *Proc. ASME IDETC/CIE 2005*, DETC2005-85513, 565-575.
36. Hobby, J. D. (1999). Practical segment intersection with finite precision output. *Computational Geometry: Theory and Applications, 13*(4), 199-214.
37. Halperin, D., & Packer, E. (2002). Iterated snap rounding. *Computational Geometry: Theory and Applications, 23*(2), 209-225.
38. Shewchuk, J. R. (1997). Adaptive precision floating-point arithmetic and fast robust geometric predicates. *Discrete & Computational Geometry, 18*(3).
