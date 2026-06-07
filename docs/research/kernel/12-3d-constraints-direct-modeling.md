# 3D Constraint Solving on Live B-reps and Geometric Relation Inference for Direct Modeling

Research dossier 12 for the Keel kernel. This file is the three-dimensional, on-the-solid half of the direct-modeling story. Dossier 04 already covered the 2D sketch solver (Owen, Fudos-Hoffmann, DR-planning, Laman, the pebble game) and the variational sketch machinery; Kramer's assembly degree-of-freedom analysis is treated alongside the assembly work elsewhere. This dossier addresses the engine that makes Siemens Synchronous Technology feel intelligent: a 3D geometric constraint solver running on the *live* boundary representation, fed by a *relation-inference* engine that detects coaxial, coplanar, tangent, symmetric, and parallel face sets on the fly and turns them into a temporary, well-ranked constraint scheme. The prior six-track review established the concrete finding that Synchronous Technology is, mechanically, D-Cubed 3D DCM solving inferred Live Rules relations on the model. This file works out the theory, the published systems, and the solver-to-kernel contract that Keel needs to reproduce that behavior.

Scope and terminology. By "live B-rep" we mean the actual solid the user is editing, not an abstract sketch graph: the entities the solver moves are whole faces (their underlying surface placements), and the constraints are relations among those surfaces. By "relation inference" we mean automatically discovering geometric regularities that hold (exactly or within tolerance) in the current model, because direct-modeling models are typically "dumb" imported solids with no recorded constraints. By "minimal motion" or "least change" we mean the central direct-modeling requirement: when the user drags one face, the under-constrained system must resolve to the configuration *closest to the current one*, not to some arbitrary far solution, so the edit looks local and predictable.

The dossier is organized thematically: (1) 3D constraint-solving theory and its limits; (2) numerical 3D solving and minimal-motion under-constrained resolution; (3) D-Cubed 3D DCM and Synchronous Technology; (4) variational direct modeling and push-pull formalizations; (5) relation and symmetry inference from B-rep; (6) auto-constraining and consistent-subset selection; (7) constraint-driven B-rep update and the solver-to-topology contract; (8) feature preservation and solution continuity. A closing synthesis specifies the Keel direct-modeling solver architecture and the solver-to-kernel contract.

---

# Part 1: 3D geometric constraint solving theory and its limits

## 1.1 Why 2D theory does not lift to 3D

### Hoffmann & Vermeer (1994, 1995): extending graph reduction to space

**Citation.** Hoffmann, C. M., & Vermeer, P. J. (1995). Geometric constraint solving in R^2 and R^3. In D.-Z. Du & F. Hwang (Eds.), *Computing in Euclidean Geometry* (2nd ed., pp. 266-298). World Scientific. See also Hoffmann, C. M., & Vermeer, P. J. (1994). A spatial constraint problem. In *Computational Kinematics '95 / Workshop on Computational Kinematics*, Kluwer.

**Method.** This pair of papers is the canonical attempt to carry the 2D "decompose into rigid clusters, then recombine" strategy into three dimensions. In 2D a cluster is a rigid body with three degrees of freedom and the workhorse recombination rule is "merge three clusters that pairwise share a geometric element" (the triangle rule, equivalent to solving a triangle by ruler and compass). Hoffmann and Vermeer generalize the cluster to a rigid body with six DOF in space and seek the analogous recombination rules. The natural 3D analog is to merge clusters that share enough geometry to fix all six relative DOF: pairs of clusters sharing a line plus a point, triples sharing points pairwise, and so on. They enumerate the small "wrench" configurations (three rigid bodies pairwise sharing geometry) whose solution is the 3D counterpart of triangle solving, and show these reduce to solving systems of low-degree polynomials (often the intersection of spheres, planes, and cylinders). The construction sequence, when one exists, again gives a step-by-step ruler-compass-protractor style build of the solid.

**Limitations.** The papers themselves document why the lift is incomplete. There is no clean 3D analog of the triangle rule that covers all minimal rigid bodies, because the combinatorics of 3D rigidity are not captured by a simple counting rule (see Laman below). Many 3D minimal rigid clusters require simultaneously solving more than three sub-clusters, producing high-degree polynomial systems with no closed form. The decomposition therefore terminates with irreducible cores far more often than in 2D, dropping the solver into numerical iteration.

**Kernel relevance.** This is the foundational warning for Keel: the elegant 2D sketcher architecture (dossier 04) does not transplant onto the live B-rep. A 3D constraint engine must be primarily numerical with structural decomposition as an accelerator, not the reverse. Hoffmann and Vermeer's wrench-configuration catalog is still useful as a library of closed-form fast paths for the common mate types (coaxial cylinders, coplanar planes, point-on-plane) that dominate real edits.

### Laman's theorem and the absence of a 3D analog

**Citation.** Laman, G. (1970). On graphs and rigidity of plane skeletal structures. *Journal of Engineering Mathematics, 4*(4), 331-340. For the 3D obstruction: Graver, J., Servatius, B., & Servatius, H. (1993). *Combinatorial Rigidity*. American Mathematical Society. Maxwell count: Maxwell, J. C. (1864).

**Method.** Laman gives the exact combinatorial test for generic minimal rigidity of a bar-joint framework in the plane: m = 2n - 3 edges with every n'-vertex subgraph spanning at most 2n' - 3 edges. The plane has 2n coordinates and 3 rigid-motion DOF. The naive 3D extension uses the Maxwell count: a spatial framework has 3n coordinates and 6 rigid-motion DOF, suggesting m = 3n - 6 with subsets bounded by 3m' <= 3n' - 6. This counting condition is necessary but not sufficient.

**Limitations.** The Maxwell count fails, and the canonical counterexample is the double banana (next entry). Decades of rigidity-theory research have not produced a purely combinatorial characterization of generic 3D rigidity; it remains a celebrated open problem. Consequently no graph-only algorithm can correctly classify 3D constraint systems as well/over/under-constrained in all cases.

**Kernel relevance.** Keel must not trust degree-of-freedom *counting* for 3D constraint diagnosis. Where a 2D sketcher can report over/under-constraint from the graph alone, the 3D engine has to fall back on numerical rank analysis of the constraint Jacobian (the witness method, Part 2) to be correct. This is a hard design constraint, not an optimization choice.

### The double banana: the canonical 3D counterexample

**Citation.** Reviewed in: Hendrickson, B. (1992). Conditions for unique graph realizations. *SIAM Journal on Computing, 21*(1), 65-84. Loop-based treatment: Simroth, A., & Ding, H. (2016). Solving the double-banana rigidity problem: a loop-based approach. *Mechanical Sciences, 7*, 107-114.

**Method.** The double banana is two rigid octahedral "bananas" glued at two shared vertices. Counting edges and vertices, it exactly satisfies the Maxwell 3n - 6 condition and every subgraph satisfies the corresponding inequality, so a graph-counting test declares it minimally rigid. Geometrically it is not rigid: the two bananas can rotate freely about the axis through the two shared points, an "implied hinge." Simultaneously, that hinge axis length is determined twice (once by each banana), so the system is locally redundant *and* flexible at the same time, a combination 2D never produces. The loop-based approach of Simroth and Ding reformulates the system in terms of closed kinematic loops, exposing the rotational freedom that body-by-body decomposition hides.

**Limitations.** The double banana is only the smallest member of an infinite family of counting-correct but flexible 3D frameworks; detecting all of them combinatorially is the open problem above. Loop-based methods solve specific cases but are not a general characterization.

**Kernel relevance.** A direct concrete test case for Keel's 3D diagnosis: any classifier that reports the double banana as well-constrained is using counting and will mis-handle real coaxial-and-shared-edge configurations. The lesson reinforces a numerical-rank diagnosis pipeline.

### Sitharam et al.: module rigidity and the FRONTIER 3D solver

**Citation.** Sitharam, M., & Zhou, Y. (2004). A tractable, approximate, combinatorial 3D rigidity characterization. *Proceedings of the Automated Deduction in Geometry (ADG) workshop*. Related: Hoffmann, C. M., Lomonosov, A., & Sitharam, M. (2001). Decomposition plans for geometric constraint systems, Parts I and II. *Journal of Symbolic Computation, 31*(4). Software: the open-source FRONTIER solver (Geoplexity).

**Method.** Accepting that exact 3D combinatorial rigidity is intractable, Sitharam and Zhou define *module rigidity*, an approximate but efficiently computable characterization that captures a class of 3D systems "adequate for most applications." The core algorithmic engine is the Frontier Vertex Algorithm, a degrees-of-freedom-graph decomposition-recombination method that works in both 2D and 3D. It computes a DR-plan: a hierarchy of small generically rigid subsystems, each solved by the algebraic-numeric solver and abstracted to a simplified node before recursion, so the exponential cost of solving the whole sparse polynomial system is avoided. The frontier-vertex bookkeeping tracks which shared entities ("frontier" vertices) must be preserved when a subsystem is collapsed, which is what lets the same machinery serve over-constrained detection.

**Limitations.** Module rigidity is deliberately approximate: it accepts some systems that are not truly rigid and is conservative on others, trading exactness for tractability. It still rests on a generic-position assumption and so can be fooled by special-position degeneracies common in mechanical parts (exactly axis-aligned faces, repeated dimensions).

**Kernel relevance.** FRONTIER is the most directly relevant open-source precedent for a 3D feature-based and assembly constraint solver, and module rigidity is the pragmatic stance Keel should adopt: do not chase exact 3D rigidity, use an approximate decomposition to shrink the numerical subproblems, and lean on a numerical rank test for the final verdict.

### Schreck, Mathis, Jermann and the witness configuration method

**Citation.** Michelucci, D., & Foufou, S. (2006). Geometric constraint solving: the witness configuration method. *Computer-Aided Design, 38*(4), 284-299. Extensions: Thierry, S. E. B., Schreck, P., Michelucci, D., Fünfzig, C., & Genevaux, J.-D. (2011). Extensions of the witness method to characterize under-, over- and well-constrained geometric constraint systems. *Computer-Aided Design, 43*(10), 1234-1249. Dependence detection: Michelucci, D., & Foufou, S. (2009). Detecting all dependences in systems of geometric constraints using the witness method. In *Automated Deduction in Geometry (ADG 2008)*, LNCS 4869, Springer. Critique: Hu, H., Kleiner, M., & Pernot, J.-P. (2019). On limitations of the witness configuration method for geometric constraint solving in CAD modeling. arXiv:1904.00526.

**Method.** The witness method sidesteps the combinatorial intractability of 3D rigidity by testing rigidity *numerically but symbolically-faithfully*. A witness is any one known configuration that satisfies the same incidence and combinatorial structure as the target problem (typically the current model itself, which is a perfect witness). The method forms the Jacobian of the constraint equations and evaluates it at the witness. The dimension of the kernel (null space) of this Jacobian equals the number of remaining degrees of freedom; subtracting the six (or three in 2D) rigid-body motions tells whether the system is well-, under-, or over-constrained. Crucially, because the witness is a real configuration, the rank reflects *actual* (not merely generic) dependencies, so it catches both structural dependencies (double banana) and non-structural ones (theorem-induced redundancies such as the three medians of a triangle being concurrent). The 2011 extension shows how to read off, from the Jacobian's row and column dependencies, the maximal well-constrained subsystems and the minimal over-constrained subsystems, giving precise diagnosis. The 2009 paper uses repeated witness perturbation to enumerate all dependences.

**Limitations.** Hu, Kleiner and Pernot (2019) show the method is sensitive to the algebraic *representation* of the same geometry: different parameterizations of identical constraints can yield different Jacobian ranks, producing contradictory verdicts. Obtaining a valid witness for a not-yet-solved problem can itself require solving a related problem. Rank determination near singular configurations is numerically delicate.

**Kernel relevance.** This is the method Keel should use for 3D constraint diagnosis, and the live B-rep gives it for free: the current solid *is* the witness, so a single Jacobian rank computation at the present configuration tells Keel exactly how many DOF remain and which face groups are redundant. The 2011 row/column-dependency reading is the algorithm Keel needs to highlight the offending faces when an inferred relation set is over-constrained.

### Michelucci's broader numerical-geometric program

**Citation.** Michelucci, D., Foufou, S., Lamarque, L., & Schreck, P. (2006). Geometric constraints solving: some tracks. In *Proceedings of the ACM Symposium on Solid and Physical Modeling (SPM 2006)*, pp. 185-196. ACM.

**Method.** Michelucci's "tracks" paper surveys and argues for numerical and interval methods over purely combinatorial ones for hard (especially 3D) constraint systems: homotopy/continuation for finding all solutions, interval Newton and subdivision for certified enclosure of solutions, the witness method for diagnosis, and Gröbner-basis or resultant methods for small symbolic kernels. The unifying theme is that 3D constraint solving is fundamentally an algebraic-geometry problem and the practical route is robust numerics guided by lightweight structural pre-analysis.

**Limitations.** The certified methods (interval Newton, full homotopy) are expensive and do not scale to the hundreds of faces a large direct edit can touch; they are tools for small kernels, not whole-model solving.

**Kernel relevance.** Sets Keel's intellectual posture: a sparse numerical Newton solver as the core, the witness method for diagnosis, homotopy reserved for continuity tracking during drag (Part 8), and decomposition only to shrink subproblems.

---

# Part 2: Numerical 3D solving and minimal-motion resolution

## 2.1 Sparse Newton on transform variables

### Newton-Raphson and Levenberg-Marquardt on rigid-body parameters

**Citation.** Standard formulation surveyed in: Cao, C., et al., and in Gao, X.-S., Hoffmann, C. M., & Yang, W. (2004). Solving spatial basic geometric constraint configurations with locus intersection. *Computer-Aided Design, 36*(2), 111-122. General optimization treatment: Ge, J.-X., Chou, S.-C., & Gao, X.-S. (1999). Geometric constraint satisfaction using optimization methods. *Computer-Aided Design, 31*(14), 867-879.

**Method.** The bread-and-butter 3D solver assigns each rigid face group a transform (a rigid motion in SE(3)) as its variables and writes each constraint as one or more scalar residual equations: coincidence of points, perpendicularity or parallelism of directions (dot products), distances (norms), tangency (distance equals radius). Stacking residuals gives a vector function F(x) = 0 whose Jacobian J is large but very sparse, because each constraint touches only the two face groups it relates. Newton-Raphson iterates x <- x - J^+ F using a sparse linear solve (sparse LU or QR) of the Jacobian at each step; when the system is non-square or ill-conditioned, Levenberg-Marquardt damping (J^T J + lambda I) provides a robust least-squares step that degrades gracefully on over- and under-constrained systems. Gao et al. (2004) give closed-form "locus intersection" solutions for the small basic 3D configurations (point/line/plane/sphere/cylinder mates) used as fast paths and as the leaves of a decomposition.

**Limitations.** Plain least squares minimizes residual, not motion, so it does not by itself give the "least change from the current shape" behavior direct modeling needs (see 2.3). Newton can diverge or jump to a far solution from a poor start; rotation parameterization matters (next entry). Convergence basins shrink near singular configurations.

**Kernel relevance.** This is the literal core of Keel's 3D engine. The sparse Jacobian per-constraint locality maps cleanly onto a Rust sparse-matrix assembly, and the closed-form basic-mate solutions become a `fast_path` table. The choice between solving for minimal *residual* versus minimal *motion* is the central design decision, resolved in 2.3.

### Quaternion and dual-quaternion parameterization of rigid motion

**Citation.** Dual-quaternion kinematics: Kavan, L., Collins, S., Zara, J., & O'Sullivan, C. (2008). Geometric skinning with approximate dual quaternion blending. *ACM Transactions on Graphics, 27*(4), article 105. Foundational: Clifford, W. K. (1873). Preliminary sketch of biquaternions. Application to constraint/assembly motion: see D-Cubed and kinematics literature in Part 3.

**Method.** Representing each rigid face group's orientation as a unit quaternion (four parameters, one normalization constraint) avoids the gimbal-lock singularities and range wrapping of Euler angles and gives a well-conditioned Jacobian for rotational constraints. The full rigid motion (rotation plus translation) is captured by a *dual quaternion*, an eight-parameter object q = q_r + epsilon q_d (with epsilon^2 = 0) that represents screw motions compactly; its two unit-norm conditions encode a valid SE(3) element. Dual quaternions interpolate rigid motions smoothly (useful for drag), compose by multiplication, and yield clean derivatives for the Jacobian. In a constraint solver each face group carries a (dual) quaternion plus a position, and constraints are differentiated through the quaternion algebra.

**Limitations.** The unit-norm side constraints add equations and require either projection back to the manifold each iteration or a Lagrange-multiplier formulation, slightly complicating the linear algebra. Over-parameterization (4 numbers for 3 rotational DOF) means the Jacobian has a built-in rank deficiency that must be handled.

**Kernel relevance.** Keel should parameterize face-group motion with quaternions for rotation and either a translation vector or a full dual quaternion, manifold-projecting each Newton step. This gives numerically robust drag and smooth motion interpolation, both required for live editing. Rust crates exist for quaternion and dual-quaternion algebra, or a small bespoke module suffices.

### Null-space motion analysis: finding the remaining DOF

**Citation.** Sitharam, M., Arbree, A., Zhou, Y., & Kohareswaran, N. (2006). Solution space navigation for geometric constraint systems. *ACM Transactions on Graphics, 25*(2), 194-213. Witness-based DOF reading: Thierry et al. (2011), cited above.

**Method.** After (or instead of) driving residual to zero, the kernel (null space) of the constraint Jacobian at the current configuration is precisely the space of infinitesimal motions that keep all constraints satisfied. Computing an orthonormal basis of ker(J) (via SVD or rank-revealing QR) yields the remaining degrees of freedom as concrete motion vectors: each basis vector is a combined infinitesimal translation/rotation of the face groups that the constraints permit. Sitharam et al.'s solution-space navigation lets the user (or the system) move along these basis motions to explore the realization space of an under-constrained system in a geometrically meaningful way.

**Limitations.** The null space is only the *tangent* space of motions at the current point; following it for a finite distance requires re-projection onto the constraint manifold (a Newton correction step), because the manifold curves. For highly under-constrained systems the basis is high-dimensional and not all directions are physically meaningful.

**Kernel relevance.** This is how Keel knows what a drag is *allowed* to do. When the user grabs a face, the permitted motion is exactly the projection of the drag onto ker(J); the solver moves along that subspace and corrects back. Null-space analysis also drives the minimal-motion resolution below.

## 2.2 Under-constrained solving with minimal motion: the direct-modeling requirement

### Least-change / minimal-perturbation resolution

**Citation.** Ge, J.-X., Chou, S.-C., & Gao, X.-S. (1999). Geometric constraint satisfaction using optimization methods. *Computer-Aided Design, 31*(14), 867-879. Related dragging formulation: Hsu, C.-Y., & Brüderlin, B. (1997), and the continuation literature in Part 8.

**Method.** Direct modeling almost always presents an under-constrained system: a few inferred relations plus one user drag, far short of fully fixing every face. The defining requirement is that the resolution be the configuration *closest to the current one*. This is posed as a constrained minimization: minimize the change ||x - x_0||^2 from the current configuration x_0 (sum of squared face-transform changes, or better a motion metric weighting rotation and translation) subject to satisfying the constraints F(x) = 0 and the user's drag. The solution lies in the constraint manifold at minimal motion-distance from x_0, which is exactly the metric projection of the dragged configuration onto the constraint set. In the linearized step this reduces to a minimum-norm least-squares solve: among all Newton steps that satisfy the (linearized) constraints, take the one of smallest norm, i.e. the component lying in the row space of J, equivalently the pseudo-inverse step J^+ applied to the drag, leaving the null-space DOF unmoved.

**Limitations.** Minimal Euclidean change in transform parameters is not the same as minimal *perceived* change; a good motion metric must weight rotational and translational components and ideally per-face extent so a large face and a tiny chamfer move comparably. The minimum lies on a curved manifold, so a single linear step is approximate and must be iterated with re-projection. Local minima exist; the closest solution can flip discontinuously as the drag crosses a singularity (Part 8).

**Kernel relevance.** This is the single most important behavior for Keel's direct-modeling feel, and it pins the solver formulation: not "solve F(x)=0" but "find the nearest point of {F(x)=0} to x_0 that honors the drag." Concretely, every Newton iteration uses the *minimum-norm* solution of the linearized system (move only in row(J), freeze ker(J)), with a weighted motion metric. This makes edits look local and predictable, which is the entire value proposition over history-replay parametric modeling.

---

# Part 3: D-Cubed 3D DCM and Synchronous Technology

### D-Cubed 3D DCM: the industrial 3D constraint engine

**Citation.** Siemens Digital Industries Software. *D-Cubed 3D DCM (3D Dimensional Constraint Manager)* product documentation and release notes (versions 56.0, 57.0, 58.0). Siemens PLM Components. Historical lineage: D-Cubed Ltd. (Cambridge, UK), founded by John Owen, acquired by UGS/Siemens 2004.

**Method.** 3D DCM is the commercial component that supplies the actual 3D solving behind NX, Solid Edge Synchronous Technology, and many third-party CADs. The published capability description: it solves 3D geometric constraints applied to points, lines, planes, circles, ellipses, cylinders, cones, tori, spheres, and general parametric curves and surfaces. Supported constraints include coincidence, concentricity, parallelism, perpendicularity, angle, distance/dimension, tangency, symmetry, and equality constraints (equal distance, equal radius), plus equal-direction and equal-curvature constraints for splines. Geometry can be grouped into *rigid sets* that move as one body, which is the mechanism by which a 3D part is positioned by constraints and by which assembly mating works. The same engine drives three distinct use cases the docs call out explicitly: 3D parametric sketching, part shape control via direct modeling (this dossier's subject), and assembly positioning plus kinematic motion simulation. For direct modeling, the application hands DCM the faces to move and the relations to maintain, and DCM returns updated face placements. The engine handles well-, under-, and over-constrained inputs and reports the diagnosis.

**Limitations.** Closed proprietary source; the published material describes capabilities, not algorithms. The relation *inference* (deciding which relations to feed DCM) lives in the host CAD's Live Rules layer, not in DCM itself; DCM is the solver, not the intent-detector. Performance characteristics on very large face sets are not published.

**Kernel relevance.** 3D DCM is the exact functional specification Keel's 3D solver must match: the same entity types, the same constraint vocabulary, the same rigid-set grouping, the same three use cases from one engine, and the same well/under/over diagnosis. It also confirms the architecture boundary: the solver is a separable component (DCM sits beside Parasolid, not inside it), so Keel should ship its 3D constraint solver as a peer crate to the kernel, mirroring the prior finding from dossier 04 about the 2D DCM boundary.

### Synchronous Technology and Live Rules

**Citation.** Siemens Solid Edge and NX documentation, "Synchronous Technology" and "Live Rules"; Siemens PLM blog posts ("ST8: Design Intent and Live Rules"; "Synchronous Technology and Live Rules"); soliddna.com Live Rules options reference. Industry analysis: Digital Engineering, "Examining Solid Edge Synchronous Technology from a Parametric Point of View."

**Method.** Synchronous Technology is the user-facing direct-modeling system built on 3D DCM. Its intelligence layer is *Live Rules*: a geometry-recognition pass that, the instant the user selects a face to move, scans the model for faces standing in strong geometric relations to the moving set, specifically coplanar, tangent, concentric/coaxial, parallel, perpendicular, and symmetric relationships. Detected relations become temporary constraints that the solver maintains during the drag, so moving one face of a slot keeps the opposite wall parallel, keeps coplanar faces coplanar, keeps coaxial holes coaxial, and so on, automatically. The Live Rules panel exposes each detected rule and lets the user toggle it: green means active and effective, red means disabled, orange means active but currently having no effect. Rules detected at drag time are transient, but the user can promote any of them to a *persistent relationship* that is re-enforced on every future edit. There is an implicit ranking and locking scheme: planar faces coplanar to a base reference plane lock to that plane, cylindrical faces coaxial to a global axis lock to that axis, establishing a priority of "stronger" datums.

**Limitations.** The exact ranking and conflict-resolution heuristics are proprietary and only partly documented through the green/orange/red UI semantics. Over-aggressive inference produces surprising rigidity (too many faces move together); under-aggressive inference breaks design intent. Tuning this is the hard, unpublished part.

**Kernel relevance.** This is the precise behavior Keel must reproduce and the published evidence for the dossier's central thesis: the "magic" is inference plus 3D solving, not a smarter kernel. Keel needs (a) an inference pass producing candidate relations (Part 5), (b) a ranking/consistency stage producing a non-over-constrained scheme (Part 6), (c) the 3D solver maintaining them during drag (Parts 1-2), and (d) a UI contract exposing and letting the user toggle/persist each inferred rule. The green/orange/red model is a ready-made interaction spec.

---

# Part 4: Variational direct modeling and push-pull formalizations

### Zou & Feng: Variational Direct Modeling

**Citation.** Zou, Q., & Feng, H.-Y. (2023). Variational direct modeling: a framework towards integration of parametric modeling and direct modeling in CAD. *Computer-Aided Design, 157*, 103465. (Preprint arXiv:2301.02999.)

**Method.** This is the central academic articulation of the problem Keel is solving. Zou and Feng identify two inconsistencies a direct edit creates. *Geometry-Topology Inconsistency* (GTI): moving a surface can make some topological connections impossible to form and force extra ones, producing invalid faces, self-intersections, or non-manifold structure. *Shape-Associativity Inconsistency* (SAI): after the geometry changes, the geometric constraint system no longer matches the shape, producing over- or under-constrained models (SAI-A after a direct edit, SAI-B after a parametric edit). The constraint state is characterized with the Witness Configuration Method via the Jacobian: over-constraint shows as linearly dependent rows, under-constraint as a null space larger than the six rigid-body DOF, well-constraint as exactly six. Minimal over-constraint detection is posed as a sparse-recovery (L0-minimization) problem, min ||x||_0 subject to J^T x = 0, solved by compressive-sensing relaxation. The headline contribution is *decomposed edits*: rather than resolve inconsistency after a big move, the edit is split at *Critical GTI Points* (GTIPs) into a sequence of small modifications. At each GTIP the method detects the next degenerate configuration (surface-surface intersection, tangency, face-collision) along the edit trajectory, resolves it with a Boolean operation between the original model and an auxiliary swept volume bounded by the extended neighboring faces, then continues. SAI is then resolved by updating the constraint system to the new shape, extracting minimal over-constrained cycles and maximal well-constrained parts from the Jacobian, and choosing which constraints to drop via a two-level ranking: a rough type-precedence (e.g. parallelism over angle, by empirical design frequency) and a fine sensitivity-based ranking that prefers the option producing the smallest shape-variation rate.

**Limitations.** The authors flag abrupt face-delete operations as a "serious limitation" the incremental decomposition cannot handle; freeform surfaces are out of scope (only linear, quadratic, and holed faces); global (distant) penetrations are handled weakly; the constraint-prioritization heuristics may not generalize and need partial user guidance; feature semantics (blind vs through hole) are not preserved automatically; Boolean operations are compute-intensive.

**Kernel relevance.** This paper is effectively Keel's requirements document for the direct-modeling subsystem. The GTI/SAI split tells Keel the two failure modes to guard. The witness/Jacobian diagnosis and L0 over-constraint detection are directly implementable. The decomposed-edit-at-GTIP strategy is a concrete algorithm for the hardest part, when re-intersection changes topology mid-edit (Part 7). The two-level constraint ranking is a starting heuristic for Keel's auto-constrainer (Part 6). The named limitations are Keel's known-hard backlog.

### Zou & Feng earlier: geometry-topology inconsistency tracking and decision support

**Citation.** Zou, Q., & Feng, H.-Y. (2022). A robust direct modeling method for quadric B-rep models based on geometry-topology inconsistency tracking. *Engineering with Computers, 38*, 3815-3830. (Preprint arXiv:1906.08455.) Companion: Zou, Q., & Feng, H.-Y. (2020). A decision-support method for information inconsistency resolution in direct modeling of CAD models. *Advanced Engineering Informatics* (preprint arXiv:1906.08433). Earlier analysis: Zou, Q., & Feng, H.-Y. (2019). Variational B-rep model analysis for direct modeling using geometric perturbation. arXiv:1903.07770.

**Method.** The 2022 quadric paper focuses on push-pulling models made of planes and quadrics, where smooth (tangent) face-face connections make naive face moves produce geometry-topology inconsistency. It tracks, as faces move, exactly where intersection curves appear or vanish and where smooth connections break, and uses that tracking to regenerate correct topology. The 2020 decision-support paper frames inconsistency resolution as choosing among many valid options and supplies a structured method (criteria and ranking) to pick one, anticipating the auto-constraining problem. The 2019 perturbation paper introduces the geometric-perturbation analysis (a witness-style infinitesimal test) used to classify the constraint state of a B-rep for direct modeling.

**Limitations.** Quadric-only in the 2022 work; the decision-support criteria are heuristic; perturbation analysis inherits the witness method's representation sensitivity.

**Kernel relevance.** The inconsistency-tracking algorithm is the practical recipe for Keel's "when does re-intersection change topology" question (Part 7). The perturbation analysis is a concrete, code-ready version of the witness method specialized to B-rep direct modeling.

### Zou & Liu: Semantic Direct Modeling

**Citation.** Zou, Q., & Liu, S. (2025). Semantic direct modeling. State Key Laboratory of CAD&CG, Zhejiang University. arXiv:2504.13893.

**Method.** SDM lifts direct modeling from low-level face manipulation to high-level semantic intent expressed in natural language ("move the slot 3 mm forward"). A speech/LLM front end (Whisper plus a fine-tuned Llama with chain-of-thought) parses commands into structured feature type, operation type, and parameters. The technically novel core for Keel is the *dynamic feature generation* module: the first application of Transformers to B-rep tokenization. A B-rep is converted to a triangulated mesh and encoded hierarchically: segments (start vertex plus direction, in R^6), polygons, triangles (center, three corner directions, normal, neighbor indices), and faces (aggregated triangle tokens with convolution), with sum-pooling over neighboring faces to capture topology. A cross-attention fusion aligns a textual feature embedding with this geometric embedding, and an autoregressive Transformer decoder emits the set of face IDs that constitute the named feature, *context-sensitively* (the same face can belong to different features depending on intent). The result is a learned feature recognizer that turns "the slot" into the exact face set to operate on.

**Limitations.** LLMs cannot reason about spatial relations ("upper-left corner") without visual state; no explicit symmetry or pattern inference; needs RAG for broad domain coverage; network latency between CAD and AI modules; design-intent only (no analysis or manufacturing intent).

**Kernel relevance.** SDM is mostly an interaction layer above Keel, but the geometric-embedding feature recognizer is relevant to Keel's *feature-set detection* (which faces constitute "the slot" or "the boss" that should move together), an alternative or complement to the rule-based inference of Part 5. For a Rust kernel the learned approach is a longer-term option; the immediate takeaway is the framing that direct edits operate on *features* (face sets), not individual faces.

### Lipp, Wonka & Müller: PushPull++

**Citation.** Lipp, M., Wonka, P., & Müller, P. (2014). PushPull++. *ACM Transactions on Graphics, 33*(4), article 130 (Proceedings of SIGGRAPH 2014). DOI 10.1145/2601097.2601197.

**Method.** PushPull++ is the graphics-community formalization of push-pull editing on polygonal meshes, and it solves precisely the "which faces adapt and how" problem at the mesh level. Prior push-pull tools either required adjacent faces to be orthogonal or supported slanted faces but never created new geometry. PushPull++ contributes four mechanisms. *Adaptive face insertion*: when a face is pushed, new side faces are inserted automatically and adaptively, only where needed, so a push along a non-orthogonal neighbor produces correct slanted connecting faces. *Adjacent face update*: neighboring faces are re-fitted to remain consistent with the moved face, with the rule set deciding whether a neighbor stretches, tilts, or spawns a new face. *Edge collapse handling*: when a push drives an edge to zero length the topology is collapsed cleanly rather than degenerating. *Drag-direction inference*: the UI suggests the natural drag direction from the local geometry, cutting clicks for common operations by up to an order of magnitude. The face-set that moves together is computed adaptively from the push geometry rather than fixed in advance.

**Limitations.** It is a polygonal-mesh method, not a B-rep solver; it has no exact curved surfaces, no constraint inference (the adaptation rules are geometric, not relation-driven), and no notion of design intent beyond local adjacency. It does not solve the variational/least-change problem; it is deterministic local mesh surgery.

**Kernel relevance.** PushPull++ is the cleanest published formalization of the *adaptive face set* idea: when a face moves, decide locally which neighbors stretch, which tilt, which spawn new faces, and which collapse. Keel's B-rep direct-edit local-update logic (Part 7) needs exactly this taxonomy of neighbor responses, but driven by inferred relations rather than mesh adjacency, and producing exact B-rep faces rather than polygons. It is the graphics-side complement to Zou's CAD-side GTI handling.

---

# Part 5: Relation and symmetry inference from B-rep

### Detecting face relations: coaxiality, coplanarity, tangency, parallelism

**Citation.** Synthesized from D-Cubed/Live Rules behavior (Part 3) and the beautification literature (next entries). Tolerant detection foundations: Langbein, F. C., Mills, B. I., Marshall, A. D., & Martin, R. R. (2001). Approximate geometric regularities. *International Journal of Shape Modeling, 7*(2), 129-162.

**Method.** Relation inference on a dumb B-rep scans pairs and groups of faces (restricted to the analytic surfaces that dominate mechanical parts: planes, cylinders, cones, spheres, tori) and tests for regularities both *exactly* and *within tolerance*. Coplanarity: two planar faces share a plane if their normals are (anti)parallel and their signed distances to the origin agree within tolerance. Parallelism/perpendicularity: angle between face normals or axes within tolerance of 0/90 degrees. Coaxiality/concentricity: two cylinders/cones/tori share an axis if axis directions are parallel and axis lines coincide within tolerance. Tangency: two faces are tangent along a shared edge if their surface normals agree along that edge (G1 continuity). Equal-radius: cylinder/sphere radii agree within tolerance. The detector typically uses spatial hashing or an axis/normal accumulator (a discretized direction space) so that all faces sharing an axis or normal fall in the same bucket, turning pairwise testing into near-linear group detection.

**Limitations.** Tolerant detection is inherently ambiguous: with loose tolerance, far more relations are detected than truly hold, most mutually inconsistent (the over-detection problem that Part 6 resolves). Freeform (NURBS) faces lack a canonical axis/normal, so they fall outside analytic relation tests. Tolerance choice is model-dependent.

**Kernel relevance.** This is Keel's Live Rules engine. The accumulator-based grouping (bucket faces by quantized normal/axis) is the practical near-linear algorithm. Output is a candidate-relation list with a residual (how well each holds), which feeds the ranking/consistency stage. Restricting to analytic surfaces is the right initial scope, matching both Live Rules and the beautification literature.

### Langbein et al.: approximate geometric regularities and beautification

**Citation.** Langbein, F. C., Mills, B. I., Marshall, A. D., & Martin, R. R. (2001). Approximate geometric regularities. *International Journal of Shape Modeling, 7*(2), 129-162. Beautification: Langbein, F. C., Marshall, A. D., & Martin, R. R. (2004). Choosing consistent constraints for beautification of reverse engineered geometric models. *Computer-Aided Design, 36*(3), 261-278. Topological/geometric: Langbein, F. C., et al. (2004). Topological and geometric beautification of reverse engineered geometric models. In *ACM Symposium on Solid Modeling*. Thesis: Langbein, F. C. (2003). *Beautification of Reverse Engineered Geometric Models*. PhD thesis, Cardiff University.

**Method.** Beautification is the inference problem in its purest form: a B-rep reverse-engineered from a 3D scan exhibits its intended regularities only approximately (noise), and the goal is to recover and *exactly* enforce the designer's intent. The pipeline first detects, within tolerance, a large catalog of approximate regularities: equal lengths/angles/radii, parallel/perpendicular/coaxial/coplanar/symmetric arrangements, both among individual faces/edges and among groups. Each regularity is expressed as a geometric constraint. Because many detected regularities are mutually inconsistent, the key contribution is *selection*: regularities are added one at a time, in priority order, to a growing constraint system, and a regularity is accepted only if the expanded system remains *solvable*. Solvability is tested by a degrees-of-freedom analysis that interprets each constraint in its topological context (an efficient DOF-based solvability test). The accepted, consistent subset is then imposed exactly by solving the constraint system, snapping the noisy model to its ideal form. Scope is limited to planar, spherical, cylindrical, conical, and toroidal faces with sharp edges or fixed-radius rolling-ball blends.

**Limitations.** Analytic-surface only; greedy priority-order selection is not guaranteed to find the globally best consistent subset; the DOF solvability test inherits generic-position assumptions; tolerance and priority tuning are heuristic.

**Kernel relevance.** This is the most directly transferable algorithm in the dossier for Keel's auto-constrainer: detect many approximate relations, then *incrementally add in priority order, accepting only if still solvable*, which is exactly the recipe for turning an over-detected relation set into a well-constrained scheme without over-constraint. The DOF-in-topological-context solvability test is the consistency gate Keel needs. It also doubles as Keel's import-cleanup ("beautify a dumb STEP solid") feature.

### Li, Langbein & Martin: detecting design intent using symmetry

**Citation.** Li, M., Langbein, F. C., & Martin, R. R. (2010). Detecting design intent in approximate CAD models using symmetry. *Computer-Aided Design, 42*(3), 183-201. Related: Li, M., Langbein, F. C., & Martin, R. R. (2008). Constructing regularity feature trees for solid models. In *Geometric Modeling and Processing (GMP 2006)*, LNCS 4077, Springer.

**Method.** This work recovers design intent as high-level geometric relations between a model's sub-parts via *hierarchical symmetry decomposition*. The model is recursively decomposed into simpler, more symmetric sub-parts; design intent is then detected as congruences, symmetries, and symmetric arrangements among the leaf parts of this decomposition. The method handles *approximate* and *incomplete local* symmetries: a feature that is almost symmetric, or a sub-part that is symmetric only in part, is still detected. The output is a regularity feature tree capturing the nested symmetric structure (e.g. a flange with a symmetric bolt-circle of holes, each hole locally cylindrically symmetric).

**Limitations.** Hierarchical decomposition is sensitive to the chosen decomposition rules; approximate-symmetry detection is tolerance-dependent; computationally heavier than pairwise relation tests; aimed at analysis/recovery rather than real-time drag-time inference.

**Kernel relevance.** Symmetry and symmetric-arrangement detection is the relation type that pairwise tests in the previous entry miss (a bolt circle is a rotational pattern, not a set of pairwise relations). Keel needs this to make patterns and symmetric features move coherently under direct edits, and the regularity feature tree is a good intermediate representation linking inferred relations to feature-preserving edit propagation (Part 8).

### Mitra, Guibas & Pauly: partial/approximate symmetry detection and symmetrization

**Citation.** Mitra, N. J., Guibas, L. J., & Pauly, M. (2006). Partial and approximate symmetry detection for 3D geometry. *ACM Transactions on Graphics, 25*(3), 560-568 (SIGGRAPH 2006). Mitra, N. J., Guibas, L. J., & Pauly, M. (2007). Symmetrization. *ACM Transactions on Graphics, 26*(3), article 63 (SIGGRAPH 2007). Survey: Mitra, N. J., Pauly, M., Wand, M., & Ceylan, D. (2013). Symmetry in 3D geometry: extraction and applications. *Computer Graphics Forum, 32*(6), 1-23.

**Method.** The 2006 paper detects Euclidean symmetries (reflections, rotations, translations, and their partial/approximate variants) in general 3D geometry by a transformation-space voting scheme: sample points, compute local shape signatures (curvatures), match pairs with similar signatures, and for each match compute the transformation that maps one to the other. These transformations are accumulated as points in a transformation space; a mean-shift clustering stage finds dense clusters, each a candidate symmetry, followed by a verification step. The 2007 Symmetrization paper takes detected approximate symmetries and *enforces* them: it optimizes the geometry so the symmetry becomes exact while minimizing deformation, jointly adjusting the shape and the symmetry transform. The extracted symmetry graph is a high-level structural descriptor enabling consistent editing.

**Limitations.** Designed for meshes/point clouds, not B-rep, so it works on sampled geometry rather than exact analytic surfaces; the voting/clustering pipeline is heavier than analytic relation tests and is offline-scale, not drag-time; verification can miss subtle partial symmetries.

**Kernel relevance.** This is the graphics-side gold standard for *global* symmetry detection and for the symmetrization (snap-to-exact) operation. For Keel, the transformation-space voting idea generalizes the analytic accumulator of the relation detector to discover non-obvious symmetries, and the symmetrization optimization is conceptually the same "enforce detected regularity exactly while minimizing change" objective as both Langbein beautification and the minimal-motion solver, unifying inference and solving under one least-deformation principle.

---

# Part 6: Auto-constraining and consistent-subset selection

### Generating a well-constrained scheme from over-detected relations

**Citation.** Core algorithm from Langbein, Marshall & Martin (2004), cited above. Over-constraint detection: Zou & Feng (2023) L0 formulation, and Thierry et al. (2011) witness extension, both cited above. Transforming under- to well-constrained: Joan-Arinyo, R., Soto-Riera, A., Vila-Marta, S., & Vilaplana-Pasto, J. (2003). Transforming an under-constrained geometric constraint problem into a well-constrained one. In *ACM Symposium on Solid Modeling and Applications*, pp. 33-44.

**Method.** The auto-constraining problem: relation inference (Part 5) typically returns far more relations than the model has free DOF, many redundant or conflicting, so feeding them all to the solver over-constrains it. The solution is to select a *maximal consistent subset*. Two complementary techniques appear. (1) Incremental greedy selection with a solvability gate (Langbein): sort relations by a priority/confidence score, add them one at a time, and after each addition test whether the system is still solvable (no over-constraint); reject any relation that would over-constrain. This yields a maximal consistent set in priority order. (2) Rank-based redundancy removal (Zou/witness): assemble all relations, compute the constraint Jacobian, detect the linearly dependent rows (the minimal over-constrained cycles) via the L0/null-space analysis, and drop the lowest-priority constraint in each redundant cycle until the rank is full. Joan-Arinyo et al. handle the opposite direction, completing an under-constrained system to well-constrained by adding constraints. Priority heuristics combine relation *type* precedence (Zou's empirical frequency: parallelism over angle, etc.) with *confidence* (how exactly the relation holds, the residual from Part 5) and Live Rules-style datum strength (coplanar-to-base-plane and coaxial-to-global-axis rank highest).

**Limitations.** Greedy selection is order-dependent and not globally optimal; the "right" subset is genuinely ambiguous (it is a guess at design intent); conflicting high-priority relations still require a tie-break or user decision; rank tests near singular configurations are fragile.

**Kernel relevance.** This is Keel's bridge from raw inference to a solvable scheme, and it is the unpublished heart of Live Rules made explicit. Keel should implement the incremental-with-solvability-gate algorithm (cheap, robust, matches beautification) as the default, with witness/Jacobian rank detection to catch non-structural redundancies the greedy pass misses, and a priority function combining type precedence, residual confidence, and datum strength. Expose the resulting accepted/rejected relations to the user with the green/orange/red toggle semantics from Part 3.

---

# Part 7: Constraint-driven B-rep update and the solver-to-topology contract

### What the solver outputs and how it propagates to topology

**Citation.** Synthesized from Zou & Feng (2022, 2023) GTI tracking, cited above; classical tweak/local-operation literature (dossier 03); and the B-rep boolean/re-intersection literature: Zou & Feng (2022) and general kernel practice.

**Method.** The solver-to-kernel contract is the crux of "live" editing. The constraint solver works on a *reduced* model: each movable rigid face group is a transform (a SE(3) placement of its underlying surface), and the solver's output is a set of new surface placements (and possibly new surface parameters, e.g. a changed cylinder radius). The kernel must then rebuild a valid B-rep from these moved surfaces. The propagation is hierarchical and follows the B-rep dependency order. (1) *Faces*: apply the new surface placement to each moved face. (2) *Edges*: every edge is the intersection curve of its two adjacent faces; when one or both faces move, the edge curve is recomputed by re-intersecting the two (possibly new) surfaces. (3) *Vertices*: every vertex is the intersection of its incident edges/faces; recompute by intersecting the new edge curves. As long as the *topology* (which faces are adjacent) is unchanged, this is a pure geometric re-evaluation: same face/edge/vertex graph, new geometry, a classic "tweak." The hard case is when re-intersection changes topology: a face shrinks to nothing (edge collapse), two faces that did not touch begin to intersect (new edge), or a smooth/tangent connection forms or breaks. Zou's GTI tracking detects exactly these events along the edit trajectory; the decomposed-edit strategy stops at each such Critical GTI Point, applies the topology change (via local Euler operators or a local Boolean with a swept auxiliary volume), and resumes. The contract is therefore: solver returns surface placements; kernel re-evaluates edges/vertices by re-intersection; a GTI monitor watches for topology-changing events and triggers local topology surgery when they occur.

**Limitations.** Re-intersection of moved analytic surfaces is robust; re-intersection involving NURBS is the usual numerical hazard. Detecting topology-change events precisely (not too early, not too late) along a continuous drag is delicate. Concurrent multiple events at one configuration are hard. Persistent naming of the regenerated faces/edges (so downstream features still refer to them) is a separate hard problem (dossier 07).

**Kernel relevance.** This defines the API between Keel's solver crate and Keel's kernel: the solver consumes the current B-rep plus a drag plus a constraint scheme and emits *new surface placements per face group*; the kernel exposes a `retweak(face -> new_surface)` operation that re-intersects to update edges/vertices, plus a `topology_event` callback when re-intersection would change the face graph, at which point the direct-modeling layer invokes local Euler operators or local Boolean. Keeping the solver on placements (not on the full B-rep) keeps the contract clean and matches how DCM sits beside the kernel.

---

# Part 8: Feature preservation and solution continuity

### Feature-preserving editing: blends, holes, and patterns

**Citation.** Pattern/feature recovery from Li, Langbein & Martin (2010) regularity feature trees, cited above; blend handling from the local-operations literature (dossier 03); Synchronous Technology pattern behavior (Part 3 sources).

**Method.** A direct edit must preserve recognized features rather than dumbly moving raw faces. *Blends/fillets*: a rolling-ball blend is recognized (constant or variable radius surface tangent to two neighbors) and treated as a dependent feature: when its supporting faces move, the blend is re-created tangent to the moved faces at its recorded radius, not moved as an independent face. *Holes*: a cylindrical hole is a feature; moving the host face must keep the hole through/blind as intended (the very semantics Zou flags as unpreserved). *Patterns*: a linear or circular array of features (detected as a symmetric arrangement per Li et al.) must propagate edits to all instances, so editing one pocket of a pattern edits all of them and keeps the pattern's spacing/symmetry. The mechanism is to detect the feature/pattern relations during inference (Part 5), encode them as constraints (pattern = a set of symmetry/equal constraints binding the instances), and let the solver maintain them, so feature preservation is just another relation in the constraint scheme.

**Limitations.** Recognizing features robustly on a dumb solid is itself hard (the SDM learned recognizer or rule-based recognition); variable-radius blends and intersecting patterns are difficult; recreating a blend after large moves can fail geometrically.

**Kernel relevance.** Confirms that feature preservation is not a separate subsystem but a *consequence* of good inference plus solving: detect blends/holes/patterns as relations, add them to the scheme, and the minimal-motion solver preserves them automatically. The exception is blends, which often must be *suppressed* before the move and *re-applied* after (the classic "unblend, edit, reblend" pattern), so Keel's blend recognizer must support round-trip removal and recreation.

### Solution continuity: chirality, nearest-solution tracking, homotopy

**Citation.** Hoffmann, C. M., & Joan-Arinyo, R. (1997). Continuous solving in geometric constraint solving. Continuation/homotopy: Lamure, H., & Michelucci, D. (1996). Solving geometric constraints by homotopy. *IEEE Transactions on Visualization and Computer Graphics, 2*(1), 28-34. Tracking under reparameterization: Imbach, R., Schreck, P., & Mathis, P. (2011/2012). Tracking method for reparametrized geometrical constraint systems. Chirality: Schreck, P., & Mathis, P., and the order-type/chirality-constraint literature. Recent: (2026) Approximating continuous motions of geometric constraint systems (Riemannian optimization plus homotopy continuation).

**Method.** During a drag, the solver is called repeatedly with slightly changed parameters, and it must return the *same branch* of the solution each time, not jump to a different (equally valid) root. Three ideas address this. *Nearest-solution / least-change* (Part 2.2): each solve starts from the previous configuration and takes the minimum-motion step, so it stays on the current branch by construction. *Chirality preservation*: chirality (a.k.a. order-type or relative-orientation) constraints fix the sign of orientation determinants (which side of a plane a point is on, the handedness of a frame), preventing the solver from flipping to a mirror solution; these signs are read from the current configuration and held fixed during the drag. *Homotopy/continuation tracking*: model the parameter change as a continuous path and follow the solution along it with predictor-corrector continuation, which by certification avoids path-jumping; the 2026 Riemannian-plus-homotopy framework follows certified solution curves on the constraint manifold, giving genuine, artifact-free motions. Imbach-Schreck-Mathis handle the case where the parameterization itself changes during tracking.

**Limitations.** At genuine singularities (where solution branches actually merge or the configuration becomes degenerate) continuity cannot be maintained and the motion must stop or the user must choose; full homotopy is expensive for interactive rates on large systems; chirality sign assignment can be ambiguous at near-coplanar configurations.

**Kernel relevance.** This is what makes Keel's drag feel continuous instead of popping. The cheap and sufficient default is least-change-from-previous (already required for direct modeling) plus held chirality signs read from the start configuration. Reserve homotopy continuation for the hard cases where least-change alone jumps branches. Detect singularities (Jacobian rank drop, via the witness machinery already in place) and halt the drag gracefully there.

---

# Direct modeling solver architecture for Keel

The literature converges on a clear architecture. Keel's direct-modeling capability is *not* a kernel feature; it is a constraint solver plus an inference engine that sits beside the kernel and drives it, exactly as D-Cubed 3D DCM sits beside Parasolid and as Synchronous Technology layers Live Rules over DCM. The recommended decomposition into crates and stages:

1. **Relation-inference engine (Live Rules analog).** On selection/drag start, scan the live B-rep for approximate relations among analytic faces using direction/axis accumulators (near-linear): coplanar, parallel, perpendicular, coaxial/concentric, tangent, equal-radius, plus symmetry and pattern detection (Li/Mitra-style) for arrangements. Output: a candidate-relation list, each with a residual (confidence) and a datum-strength rank. This is the inference half the prior research identified as the missing piece.

2. **Auto-constrainer (consistent-subset selection).** Turn the over-detected candidate list into a solvable, non-over-constrained scheme. Default algorithm: incremental greedy addition in priority order (type precedence + confidence + datum strength) with a solvability gate after each addition (Langbein beautification recipe). Catch non-structural redundancies with a witness/Jacobian rank test (Thierry et al., Zou L0). Expose accepted/rejected/ineffective relations to the user with green/orange/red toggle and persist-on-demand semantics.

3. **3D numerical solver core.** Variables: a SE(3) transform per rigid face group, orientation as a quaternion (or full motion as a dual quaternion), manifold-projected each step. Constraints: scalar residuals per relation, assembled into a large sparse Jacobian (per-constraint locality). Solve by sparse Newton / Levenberg-Marquardt. The governing objective is *minimal motion*: each step is the minimum-norm (pseudo-inverse) solution honoring the drag, moving only in row(J) and freezing ker(J), under a weighted rotation/translation metric. This is the direct-modeling requirement, not optional. Use the closed-form basic-mate solutions (Hoffmann-Vermeer wrenches, Gao locus intersection) as fast paths, and module-rigidity decomposition (FRONTIER style) only to shrink subproblems, never as the diagnosis of record.

4. **Diagnosis (witness method).** The current model is a free witness: one Jacobian rank computation gives remaining DOF (kernel dimension minus six) and, via row/column dependencies, the over-constrained cycles and well-constrained parts. This is the only correct 3D diagnosis (no Laman analog; double-banana caution).

5. **Continuity layer.** During drag, least-change-from-previous keeps the branch; chirality signs read at drag start are held to prevent mirror flips; homotopy continuation is the fallback for branch-jump-prone moves; singularities (rank drop) halt the drag gracefully.

6. **Feature preservation.** Recognized blends, holes, and patterns become relations in the scheme so the solver preserves them automatically; blends additionally support unblend/edit/reblend round-tripping.

**The solver-to-kernel contract.** The solver consumes (current B-rep, drag input, constraint scheme) and emits *new surface placements (and surface parameters) per face group* only. The kernel exposes:

- `retweak(face -> new_surface)`: re-evaluate the B-rep by applying new face surfaces and re-intersecting to update edges (face-face intersection curves) and vertices (edge intersections), with topology held fixed. This is the common, fast path (a classic tweak).
- `topology_event` monitoring (Zou GTI tracking): along the drag, detect re-intersection events that change the face graph (face collapse, new face-face intersection, tangency form/break). At each Critical GTI Point, pause and invoke local Euler operators or a local Boolean with a swept auxiliary volume to apply the topology change, then resume (decomposed-edit strategy).
- Persistent naming hooks so regenerated faces/edges retain stable identities for downstream feature references (dossier 07).

Keeping the solver on placements rather than on the full B-rep keeps this contract narrow and is what makes the solver a clean, separable peer of the kernel. The two failure modes to engineer against, per Zou & Feng, are Geometry-Topology Inconsistency (handled by the GTI monitor + local surgery) and Shape-Associativity Inconsistency (handled by the auto-constrainer + witness diagnosis updating the scheme after each edit).

In one sentence: Keel reproduces Synchronous Technology by pairing an accumulator-based relation-inference engine and a beautification-style auto-constrainer (the Live Rules half) with a sparse, quaternion-parameterized, minimal-motion Newton solver diagnosed by the witness method (the 3D DCM half), driving the kernel through a narrow placements-in / re-intersect-and-watch-for-topology-events contract.

---

# References

- Clifford, W. K. (1873). Preliminary sketch of biquaternions. *Proceedings of the London Mathematical Society*.
- Gao, X.-S., Hoffmann, C. M., & Yang, W. (2004). Solving spatial basic geometric constraint configurations with locus intersection. *Computer-Aided Design, 36*(2), 111-122.
- Ge, J.-X., Chou, S.-C., & Gao, X.-S. (1999). Geometric constraint satisfaction using optimization methods. *Computer-Aided Design, 31*(14), 867-879.
- Graver, J., Servatius, B., & Servatius, H. (1993). *Combinatorial Rigidity*. American Mathematical Society.
- Hendrickson, B. (1992). Conditions for unique graph realizations. *SIAM Journal on Computing, 21*(1), 65-84.
- Hoffmann, C. M., & Joan-Arinyo, R. (1997). Continuous solving in geometric constraint solving (continuity/dragging). 
- Hoffmann, C. M., Lomonosov, A., & Sitharam, M. (2001). Decomposition plans for geometric constraint systems, Parts I and II. *Journal of Symbolic Computation, 31*(4), 367-427.
- Hoffmann, C. M., & Vermeer, P. J. (1994). A spatial constraint problem. *Workshop on Computational Kinematics*, Kluwer.
- Hoffmann, C. M., & Vermeer, P. J. (1995). Geometric constraint solving in R^2 and R^3. In D.-Z. Du & F. Hwang (Eds.), *Computing in Euclidean Geometry* (2nd ed., pp. 266-298). World Scientific.
- Hu, H., Kleiner, M., & Pernot, J.-P. (2019). On limitations of the witness configuration method for geometric constraint solving in CAD modeling. arXiv:1904.00526.
- Imbach, R., Schreck, P., & Mathis, P. (2011). Tracking method for reparametrized geometrical constraint systems.
- Joan-Arinyo, R., Soto-Riera, A., Vila-Marta, S., & Vilaplana-Pasto, J. (2003). Transforming an under-constrained geometric constraint problem into a well-constrained one. *ACM Symposium on Solid Modeling and Applications*, 33-44.
- Kavan, L., Collins, S., Zara, J., & O'Sullivan, C. (2008). Geometric skinning with approximate dual quaternion blending. *ACM Transactions on Graphics, 27*(4), article 105.
- Laman, G. (1970). On graphs and rigidity of plane skeletal structures. *Journal of Engineering Mathematics, 4*(4), 331-340.
- Lamure, H., & Michelucci, D. (1996). Solving geometric constraints by homotopy. *IEEE Transactions on Visualization and Computer Graphics, 2*(1), 28-34.
- Langbein, F. C., Mills, B. I., Marshall, A. D., & Martin, R. R. (2001). Approximate geometric regularities. *International Journal of Shape Modeling, 7*(2), 129-162.
- Langbein, F. C. (2003). *Beautification of Reverse Engineered Geometric Models*. PhD thesis, Cardiff University.
- Langbein, F. C., Marshall, A. D., & Martin, R. R. (2004). Choosing consistent constraints for beautification of reverse engineered geometric models. *Computer-Aided Design, 36*(3), 261-278.
- Li, M., Langbein, F. C., & Martin, R. R. (2010). Detecting design intent in approximate CAD models using symmetry. *Computer-Aided Design, 42*(3), 183-201.
- Lipp, M., Wonka, P., & Müller, P. (2014). PushPull++. *ACM Transactions on Graphics, 33*(4), article 130 (SIGGRAPH 2014).
- Michelucci, D., & Foufou, S. (2006). Geometric constraint solving: the witness configuration method. *Computer-Aided Design, 38*(4), 284-299.
- Michelucci, D., Foufou, S., Lamarque, L., & Schreck, P. (2006). Geometric constraints solving: some tracks. *ACM Symposium on Solid and Physical Modeling*, 185-196.
- Michelucci, D., & Foufou, S. (2009). Detecting all dependences in systems of geometric constraints using the witness method. *Automated Deduction in Geometry (ADG 2008)*, LNCS 4869, Springer.
- Mitra, N. J., Guibas, L. J., & Pauly, M. (2006). Partial and approximate symmetry detection for 3D geometry. *ACM Transactions on Graphics, 25*(3), 560-568.
- Mitra, N. J., Guibas, L. J., & Pauly, M. (2007). Symmetrization. *ACM Transactions on Graphics, 26*(3), article 63.
- Mitra, N. J., Pauly, M., Wand, M., & Ceylan, D. (2013). Symmetry in 3D geometry: extraction and applications. *Computer Graphics Forum, 32*(6), 1-23.
- Siemens Digital Industries Software. *D-Cubed 3D DCM* product documentation and release notes (v56-58). PLM Components.
- Siemens Digital Industries Software. *Solid Edge / NX Synchronous Technology and Live Rules* documentation and PLM blog.
- Simroth, A., & Ding, H. (2016). Solving the double-banana rigidity problem: a loop-based approach. *Mechanical Sciences, 7*, 107-114.
- Sitharam, M., Arbree, A., Zhou, Y., & Kohareswaran, N. (2006). Solution space navigation for geometric constraint systems. *ACM Transactions on Graphics, 25*(2), 194-213.
- Sitharam, M., & Zhou, Y. (2004). A tractable, approximate, combinatorial 3D rigidity characterization. *Automated Deduction in Geometry workshop*.
- Thierry, S. E. B., Schreck, P., Michelucci, D., Fünfzig, C., & Genevaux, J.-D. (2011). Extensions of the witness method to characterize under-, over- and well-constrained geometric constraint systems. *Computer-Aided Design, 43*(10), 1234-1249.
- A review on geometric constraint solving (2022). arXiv:2202.13795.
- Approximating continuous motions of geometric constraint systems (2026). (Riemannian optimization plus homotopy continuation.)
- Zou, Q., & Feng, H.-Y. (2019). Variational B-rep model analysis for direct modeling using geometric perturbation. arXiv:1903.07770.
- Zou, Q., & Feng, H.-Y. (2020). A decision-support method for information inconsistency resolution in direct modeling of CAD models. *Advanced Engineering Informatics* (arXiv:1906.08433).
- Zou, Q., & Feng, H.-Y. (2022). A robust direct modeling method for quadric B-rep models based on geometry-topology inconsistency tracking. *Engineering with Computers, 38*, 3815-3830 (arXiv:1906.08455).
- Zou, Q., & Feng, H.-Y. (2023). Variational direct modeling: a framework towards integration of parametric modeling and direct modeling in CAD. *Computer-Aided Design, 157*, 103465 (arXiv:2301.02999).
- Zou, Q., & Liu, S. (2025). Semantic direct modeling. arXiv:2504.13893.
