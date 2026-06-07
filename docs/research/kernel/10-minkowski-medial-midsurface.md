# Minkowski Sums, Swept Volumes, Medial Axis, and Midsurface Extraction

Research dossier for the Keel B-rep kernel (Rust, Parasolid-class ambition).

## Scope

This file goes deep on four operations that the kernel must eventually expose as deliverable services: Minkowski sums, swept volumes, the medial axis transform (MAT), and midsurface extraction. The first two underpin collision-aware sweeps, clearance and offset features, machining simulation, and packaging analysis. The latter two underpin CAE idealization (dimensional reduction of thin-walled parts), wall-thickness and moldability analysis, and defeaturing guidance.

A prior research file (offset and shelling) covered the connection between offset self-intersection and the medial axis. This file treats extraction as a first-class operation rather than as a side effect: how to compute the MAT exactly and approximately, how to prune it to stability, how to derive midsurfaces from it, and how those results feed back into meshing, shelling, and manufacturability checks.

The dossier is organized in two parts. Part 1 covers sweep-family operations (Minkowski, swept volumes). Part 2 covers skeleton-family operations (medial axis, midsurface, thickness). Each significant source gets a Citation / Method / Results / Limitations / Kernel relevance entry. A closing synthesis ("Sweep and skeleton services for Keel") ties the two families together into a coherent service plan.

A unifying mathematical thread runs through the whole file: the **grazing condition**. A swept-volume boundary is the envelope where the moving body's surface velocity is tangent to its surface, i.e. the normal-velocity dot product vanishes, v · n = 0. A Minkowski-sum boundary is a special case where the motion is a pure translation set. The medial axis is the locus where the distance-to-boundary function is non-smooth, which is dual to the envelope of the family of maximal inscribed balls. Offsetting, sweeping, Minkowski summing, and skeletonizing are therefore four views of one envelope/distance-field machinery. Building that machinery once, robustly, pays off across all four operations.

---

# Part 1: Minkowski Sums and Swept Volumes

The Minkowski sum of two sets A and B is A ⊕ B = { a + b : a ∈ A, b ∈ B }. Geometrically it is the volume swept by B as its origin translates over every point of A (a pure-translation sweep). It is the workhorse of configuration-space (C-space) reasoning: the set of placements where a translating robot B collides with obstacle A is exactly A ⊕ (−B), so collision-free planning reduces to point-in-C-obstacle queries. Rounding and filleting with a ball are Minkowski operations (dilation, erosion, opening, closing). Swept volumes generalize Minkowski sums to arbitrary rigid or deforming motions, where the boundary is an envelope governed by the grazing condition.

## 1.1 Minkowski sums for polyhedra: exact computation

### Hachenberger, exact 3D Minkowski sums via Nef polyhedra and convex decomposition

**Citation.** Hachenberger, P. (2009). Exact Minkowski sums of polyhedra and exact and efficient decomposition of polyhedra into convex pieces. *Algorithmica*, 55(2), 329-345. (Conference version: ESA 2007, LNCS 4698, 669-680.) Implemented as the CGAL package `Minkowski_sum_3`.

**Method.** The algorithm is the canonical decompose-sum-union pipeline made exact. (1) Decompose each input polyhedron into convex pieces. Hachenberger contributes an exact and efficient convex decomposition built on Chazelle's notch-resolution idea but realized on CGAL 3D Nef polyhedra, which represent arbitrary subdivisions of R^3 (vertices, edges, facets, volumes) with each cell flagged selected or unselected. Nef representation is what makes the whole thing robust to degeneracies (coplanar facets, lower-dimensional features, open vs closed solids). (2) For each pair of convex pieces (P_i, Q_j), compute the convex-convex Minkowski sum. For convex polytopes this is straightforward: the sum is the convex hull of the pairwise vertex sums, and its facets come from merging the facet normals of both operands. (3) Union all pairwise sums with the exact Nef Boolean union. Exact arithmetic (CGAL kernels with rational or algebraic number types) guarantees the result is topologically and numerically correct.

**Results.** First provably exact, fully general 3D Minkowski-sum implementation handling all degeneracies and non-convex, even non-manifold and open, inputs. Shipped in CGAL, so it is a reference oracle. Handles the "tight passage" motion-planning case where the free space pinches to a lower-dimensional set, which approximate methods silently lose.

**Limitations.** Combinatorially heavy. A non-convex polyhedron can require a quadratic number of convex pieces, so up to n^2 m^2 pairwise sums are merged, giving O(n^3 m^3) worst case. Exact arithmetic adds a large constant factor. Practical only for modest models; not interactive. The union step over many overlapping pieces is the bottleneck.

**Kernel relevance.** This is the gold-standard correctness reference for any Minkowski feature Keel ships. Even if production uses a faster approximate or GPU path, a CGAL-backed exact mode (or its algorithmic design) is invaluable for test oracles and for the rare cases where exactness is contractually required (clearance certification, interference proofs). The Nef-based exact convex decomposition is itself a reusable primitive: convex decomposition supports collision detection, exact Booleans, and integration over solids.

### Li and McMains, GPU voxelized Minkowski sums with robust culling

**Citation.** Li, W., & McMains, S. (2011). Voxelized Minkowski sum computation on the GPU with robust culling. *Computer-Aided Design*, 43(10), 1270-1283. (Related: Li & McMains, A GPU-based voxelization approach to 3D Minkowski sum computation, ACM SPM 2010.)

**Method.** Rather than build a B-rep, the algorithm produces both a solid and a surface voxelization of A ⊕ B directly on the GPU. The naive Minkowski sum of two surface meshes is the union of pairwise primitive sums (vertex-face, edge-edge contributions), which is enormous. The key contribution is a set of **robust culling** rules that discard pairs of primitives that provably cannot contribute to the outer boundary of the sum, dramatically shrinking the work before rasterization. Remaining contributions are rasterized into a volumetric grid using the GPU's depth/stencil hardware and parallel scan. Culling is made numerically robust so no boundary voxel is wrongly dropped.

**Results.** Order-of-magnitude speedups over CPU exact methods for comparable resolution, fast enough for interactive C-space and clearance previews. Output is a voxel field, which is exactly what downstream distance-field, collision, and clearance queries want.

**Limitations.** Resolution-limited: thin features and tight passages below voxel size are lost, and the result is an approximation with no exact B-rep. Memory scales with grid resolution cubed. GPU dependency complicates a portable Rust kernel.

**Kernel relevance.** This is the template for an interactive, approximate Minkowski/clearance service: voxelize, cull aggressively, rasterize. The culling theory transfers directly to swept-volume voxelization (Part 1.3). For Keel, a wgpu-based compute path could implement the same culling-then-rasterize pattern, with the CGAL exact path as the fallback oracle.

### Convolution methods: the kinetic framework

**Citation.** Guibas, L., Ramshaw, L., & Stolfi, J. (1983). A kinetic framework for computational geometry. *Proc. 24th IEEE FOCS*, 100-111. Practical 2D realization: Wein, R. (2006). Exact and efficient construction of planar Minkowski sums using the convolution method. *ESA 2006*, LNCS 4168, 829-840 (the CGAL 2D Minkowski-sum convolution backend).

**Method.** Instead of decomposing into convex pieces, convolution traverses the two boundaries simultaneously. In the kinetic framework, a polygon is a "tracing" in a position-plus-orientation state space. The convolution of two tracings B and R is formed by summing edges whose outward-normal directions are compatible (each edge of one polygon is paired with the vertex/edge of the other whose normal interval contains the edge's normal). The convolution is a planar curve (generally self-intersecting); the Minkowski-sum boundary is the subset of the convolution with the correct winding number, extracted by an arrangement and winding-number filter. Guibas et al. proved this characterizes the sum boundary; Wein made it exact and practical.

**Results.** For polygons, convolution beats decomposition by roughly 2x to 5x in runtime because it avoids the quadratic blow-up of convex pieces and produces far fewer candidate edges. The CGAL 2D Minkowski-sum package uses convolution as its primary engine.

**Limitations.** Cleanest in 2D. The winding-number extraction from a self-intersecting convolution arrangement is delicate and the natural home of robustness bugs. 3D convolution (surface convolution) is far more complex: the analog pairs faces with vertices, edges with edges, vertices with faces by normal compatibility on the Gauss sphere, and the resulting convolution surface must be trimmed by a 3D winding/orientation test. This is an active research area, not a settled algorithm.

**Kernel relevance.** For 2D clearance, profile offsetting, nesting, and packing, convolution is the method of choice and Keel should implement it natively (it doubles as the engine for variable offsets and for 2D MAT via the bisector view). In 3D it informs the design but the exact (Hachenberger) or voxel (Li-McMains) paths are more dependable near-term.

## 1.2 Minkowski sums for curved and B-rep objects, and the morphology view

### Peternell, Pottmann, Steiner, Zhao, swept volumes and Minkowski boundaries of curved objects

**Citation.** Peternell, M., Pottmann, H., Steiner, T., & Zhao, H. (2005). Swept volumes. *Computer-Aided Design and Applications*, 2(5), 599-608. (Companion: Peternell & Steiner, Minkowski sum boundary surfaces of 3D objects.)

**Method.** For curved (B-rep / freeform) objects, exact Boolean union of pairwise sums is intractable, so the approach is envelope-plus-distance-field. The Minkowski-sum or swept-volume boundary is characterized as the envelope of a moving surface, defined by the grazing condition: a surface point contributes to the boundary only where the instantaneous velocity v of the motion is tangent to the surface, i.e. v · n = 0 with n the surface normal. (For a Minkowski sum with translate set B, the "motion" is translation by each point of B, and the condition selects, for each translation direction, the silhouette of A.) They sample the family of surfaces, collect candidate boundary points satisfying the grazing condition, build a signed distance field on a grid, and reconstruct the boundary as the zero level set (marching cubes plus cleanup). Self-intersections and global trimming are handled implicitly by the distance field's sign.

**Results.** A practical, robust pipeline for curved Minkowski sums and general swept volumes where exact methods fail. The distance-field detour sidesteps the hardest part (global trimming of a self-intersecting envelope) by letting the level set do the trimming.

**Limitations.** Approximate and resolution-bound; the reconstructed boundary is a mesh, not a trimmed-NURBS B-rep, so reintegrating it into an exact kernel needs a fitting step. Sharp edges of the true boundary are rounded at grid resolution.

**Kernel relevance.** This is the realistic near-term path for Keel's curved Minkowski and swept features: characterize the envelope by the grazing condition, accumulate a distance field, extract a level set, optionally fit surfaces back. The same code serves Minkowski sums, swept volumes, and offsets, which is exactly the consolidation Keel should aim for.

### Mathematical morphology: rounding and filleting as ball Minkowski operations

**Citation.** Williams, J., & Rossignac, J. (2005). Mason: morphological simplification. *Graphical Models*, 67(4), 285-303. Background: Serra, J. (1982). *Image Analysis and Mathematical Morphology*. Academic Press.

**Method.** With a ball B_r of radius r as structuring element, the four morphological operators are: dilation A ⊕ B_r (grow), erosion A ⊖ B_r (shrink), opening (A ⊖ B_r) ⊕ B_r (open: removes outward features and convex corners narrower than 2r, i.e. rounds convex edges from the inside), and closing (A ⊕ B_r) ⊖ B_r (close: fills concavities narrower than 2r, i.e. fillets concave edges from the outside). Opening then rounds convex edges to radius r; closing then fillets concave edges to radius r. Mason combines opening and closing (a "mortar"/"mason" filter) to simplify a shape by removing all features below a size threshold while preserving topology where possible, noting that naive rounding/filleting filters are biased (they shift the surface) and proposing a less-biased combined operator.

**Results.** Gives a clean, kernel-friendly definition of constant-radius rounding and filleting as Minkowski operations, and of feature-size-based simplification (defeaturing) as morphological opening/closing. Connects directly to the medial axis: opening by B_r removes exactly the parts of A not reachable by a ball of radius r, which is the union of medial balls of radius >= r.

**Limitations.** Pure morphology gives constant-radius results only; variable-radius blends need more. The erosion step is offset-inward, which self-intersects exactly where the medial axis has radius < r, so robust morphology requires the same self-intersection handling as offsetting. Exact ball Minkowski on curved B-reps is itself hard (canal/pipe surfaces, rolling-ball blends).

**Kernel relevance.** This reframes three Keel features as one operation family: constant fillet/round = ball closing/opening; defeaturing by size = morphological opening; clearance = dilation. Implementing a robust ball-Minkowski (rolling-ball) primitive, with medial-axis-aware self-intersection trimming, would unify rounding, filleting, shelling, and defeaturing under one engine. The bias warning from Mason is a concrete correctness requirement.

### Configuration-space obstacles for toolpaths and clearance

**Citation.** Lozano-Perez, T. (1983). Spatial planning: a configuration space approach. *IEEE Transactions on Computers*, C-32(2), 108-120. (Foundational C-space/Minkowski result, applied throughout CAM and robotics; see also Li & McMains 2011 above for the computational side.)

**Method.** For a translating object B among obstacles A, the C-space obstacle is A ⊕ (−B); collision-free placements are the complement. For toolpath clearance, B is the tool (or tool plus holder) and A is the part plus fixtures; the Minkowski sum gives the forbidden region for the tool reference point, so gouge/collision checks become point-in-region tests against a precomputed sum. Adding rotation lifts C-space to higher dimension (the swept-volume generalization).

**Results.** Turns continuous collision checking along a path into static set-membership against one precomputed Minkowski set, the standard formulation behind clearance analysis, accessibility, and reachability in CAM and packaging.

**Limitations.** Pure translation only for the simple sum; full rigid motion needs swept volumes. The C-obstacle inherits all the Minkowski-sum computational costs.

**Kernel relevance.** Defines the "clearance offset" and "collision-aware sweep" features in kernel terms: they are Minkowski/swept-volume queries. Keel's clearance API should expose both the set (for repeated queries) and a boolean path-clear test.

## 1.3 Swept volume computation: envelope, SDE, and Jacobian rank-deficiency

The swept volume of a solid S under a one-parameter motion M(t) is the union over t of M(t)·S. Its boundary is contained in the envelope of the moving boundary surface. Three classical theoretical frameworks characterize that envelope, and a fourth (voxel/distance-field) computes it numerically.

### Abdel-Malek and Yeh, Jacobian rank-deficiency formulation

**Citation.** Abdel-Malek, K., & Yeh, H.-J. (1997). Geometric representation of the swept volume using Jacobian rank-deficiency conditions. *Computer-Aided Design*, 29(6), 457-468. Survey: Abdel-Malek, K., Yang, J., Blackmore, D., & Joy, K. (2006). Swept volumes: foundations, perspectives, and applications. *International Journal of Shape Modeling*, 12(1), 87-127.

**Method.** Parameterize the moving surface by intrinsic surface parameters u (a vector, e.g. (u,v) on a patch) and motion parameter t, giving a sweep map x(u, t) into R^3. The boundary of the swept volume lies where this map is **rank-deficient**, i.e. where the Jacobian J = [ ∂x/∂u , ∂x/∂t ] loses full column rank. The singular set is { (u,t) : rank J < dim }, found by setting the appropriate Jacobian determinants (or the Gram determinant) to zero. This is exactly the grazing condition in disguise: rank deficiency means the velocity ∂x/∂t lies in the tangent plane spanned by ∂x/∂u, i.e. v · n = 0. Stratifying the Jacobian (examining which minors vanish) yields exact boundary surface patches in closed form, plus the "perturbation/restriction" analysis identifies which strata are actually on the outer boundary versus interior. The formulation is dimension-agnostic, so it extends to multi-parameter sweeps and to implicit surfaces.

**Results.** A unified, exact, closed-form characterization of swept-volume boundaries for parametric and implicit inputs, with a principled way to enumerate boundary strata and to handle voids and self-intersections via the rank/perturbation analysis. The 2006 survey taxonomizes all swept-volume methods into envelope, SDE/SEDE, Jacobian rank-deficiency, voxel, and freeform-sweeping categories.

**Limitations.** Solving the rank-deficiency equations symbolically is only tractable for simple geometry and motions; for general NURBS-under-screw-motion it becomes a hard system of nonlinear equations. Identifying the outer boundary among many singular strata (trimming) remains the hard part. Closed form rarely survives realistic CAD inputs.

**Kernel relevance.** This is the rigorous backbone: it tells Keel exactly what equation defines a swept boundary (rank deficiency = grazing). Even when solved numerically rather than symbolically, the formulation guides where to sample, how to detect boundary points, and how to classify them. The dimension-agnostic framing covers rotation, screw motion, and multi-axis sweeps uniformly.

### Blackmore, the sweep differential equation (SDE/SEDE)

**Citation.** Blackmore, D., Leu, M.-C., & Wang, L. P. (1997). The sweep-envelope differential equation algorithm and its application to NC machining verification. *Computer-Aided Design*, 29(9), 629-637. Foundational: Blackmore, D., & Leu, M.-C. (1992). Analysis of swept volume via Lie groups and differential equations. *Int. J. Robotics Research*, 11(6), 516-537. Deformed case: Blackmore, D., Leu, M.-C., Wang, L., & Jiang, H. (1997). Sweep-envelope differential equation algorithm for general deformed swept volumes. *Computer Aided Geometric Design*.

**Method.** Every smooth Euclidean motion is generated by a first-order linear ODE on the configuration (a curve in the Lie group SE(3)); this is the **sweep differential equation (SDE)**, ẋ = (Ω × (x − c)) + ċ in body-frame terms, where Ω is angular velocity and ċ the translational velocity. The swept-volume boundary is then characterized and traced by the **sweep-envelope differential equation (SEDE)**: starting from the initial grazing points (points on the t=0 surface satisfying v · n = 0), one integrates an ODE whose trajectories sweep out the envelope (boundary) of the moving solid. The grazing set evolves continuously, and the SEDE is the flow that transports it. The method extends with "surprisingly similar" structure to deforming solids and was specialized to 5-axis NC cutter envelopes.

**Results.** Turns boundary computation into ODE integration from grazing seeds, which is numerically stable and gives ordered boundary curves (good for surfacing). Handles general rigid motions and deformation; validated on NC machining verification.

**Limitations.** Needs reliable grazing seed points and breaks at topological events (when the grazing set splits/merges, or the envelope self-intersects); switching strata mid-integration is the hard part. Global trimming of the traced envelope into the true outer boundary still required.

**Kernel relevance.** The SEDE is the most "kernel-shaped" exact-ish method because it produces ordered boundary curves rather than a voxel cloud, which a B-rep kernel can surface and stitch. For Keel, a hybrid is attractive: use the distance-field method to get topology and seeds robustly, then refine boundary curves with SEDE integration where exactness matters.

### Kim, Varadhan, Lin, Manocha, fast swept-volume approximation of polyhedra

**Citation.** Kim, Y. J., Varadhan, G., Lin, M. C., & Manocha, D. (2004). Fast swept volume approximation of complex polyhedral models. *Computer-Aided Design*, 36(11), 1013-1027. (See also Zhang, Kim et al., Reliable sweeps, SPM 2009.)

**Method.** Given a polyhedron and a parametric motion path, enumerate a **superset** of the swept boundary as ruled and developable surface primitives (each edge sweeps a ruled surface, each face a more complex patch). The true swept volume is the outer boundary of the arrangement of all these primitives. To extract it robustly, compute a bounded-error approximation of each primitive, sample an unsigned distance field on a uniform grid, classify grid points by fast-marching front propagation from the primitives, reconstruct the iso-surface, and topologically refine it. The distance-field/fast-marching step performs the global trimming that the superset enumeration cannot do directly.

**Results.** Fast, robust approximate swept volumes for complex polyhedral models, with guaranteed error bounds from the grid resolution and primitive approximation. Practical on real CAD-scale meshes; widely cited as the reference approximate method.

**Limitations.** Approximate and resolution-bound; output is a mesh, not exact B-rep. Polyhedral input only (curved models must be tessellated first, losing exactness). Memory/time scale with grid resolution.

**Kernel relevance.** This is the proven recipe for Keel's approximate swept-volume service: enumerate ruled/developable boundary primitives from the moving B-rep, accumulate a distance field, fast-march to classify, extract and refine the iso-surface. It composes cleanly with the Peternell distance-field view and the Li-McMains GPU culling.

### Schroeder, Lorensen, Linthicum, implicit modeling of swept volumes; and modern GPU SV

**Citation.** Schroeder, W. J., Lorensen, W. E., & Linthicum, S. (1994). Implicit modeling of swept surfaces and volumes. *Proc. IEEE Visualization 1994*, 40-45. Modern: Sellan, S., Aigerman, N., & Jacobson, A. (2021). Swept volumes via spacetime numerical continuation. *ACM TOG (SIGGRAPH)*, 40(4). Recent: (2025) Swept Volume Computation with Enhanced Geometric Detail Preservation, arXiv:2509.09325.

**Method.** Convert the moving object to a distance field on a 3D grid and, as the object moves, progressively update each voxel with the minimum distance seen across all time samples; the swept volume is the union (zero-or-negative region) accumulated over time. Degeneracies (self-intersections, trajectory singularities) are handled gracefully because the distance field never needs explicit boundary topology. Modern work (Sellan et al.) reformulates SV boundary extraction as numerical continuation in space-time: the boundary is the grazing/envelope set { (x,t) : v · n = 0 } projected to space, traced by continuation rather than dense grid sampling, which preserves sharp detail and is provably more accurate. The 2025 detail-preserving work pushes resolution and feature fidelity further on the GPU.

**Results.** Schroeder gave the original robust implicit-SV idea (1994), still the conceptual basis for voxel SV. Sellan et al. (2021) made it accurate and detail-preserving by continuation on the v · n = 0 set, unifying the envelope theory with a practical numerical method, and is the current state of the art reference. The 2025 work improves geometric detail on GPUs.

**Limitations.** Grid-based variants are resolution-bound and memory-hungry; continuation needs good seeds and careful handling of topological transitions. Output is still a mesh/level set, not exact B-rep.

**Kernel relevance.** Sellan's space-time continuation is the most modern and most directly useful: it operationalizes the exact grazing condition (v · n = 0) as a traceable manifold, bridging the theoretical (Abdel-Malek, Blackmore) and the practical (distance field). For Keel, this is the recommended algorithmic spine of a high-fidelity swept-volume service, with voxel accumulation as the robust fallback.

## 1.4 Five-axis tool swept envelopes

### Chiou and Lee, analytic swept profiles for five-axis tool motions

**Citation.** Chiou, C.-J., & Lee, Y.-S. (2002). A machining potential field approach to tool path generation for multi-axis sculptured surface machining. *Computer-Aided Design*, 34(5), 357-371; and Chiou & Lee, Swept tool envelope and machining potential field for 5-axis sculptured surface machining, *CAD&A*, 3(6), 2006, 751-760. Related: Chiou & Lee, Formulating swept profiles for five-axis tool motions.

**Method.** For a generalized APT cutter (flat, ball, fillet/torus, cone) undergoing instantaneous 5-axis motion, derive the **instantaneous swept profile** analytically as the set of grazing points: on each cross-section of the tool, the grazing points are where the instantaneous velocity is perpendicular to the surface normal (v · n = 0). Because the tool is a surface of revolution, this condition reduces to a closed-form expression per section depending on the machine kinematics (tool axis tilt and lead/lag, feed). Sweeping the profile along the path gives the swept envelope surface; comparing it to the design surface yields machining error, which drives a "machining potential field" for toolpath optimization.

**Results.** Closed-form instantaneous swept profiles for generalized cutters in 5-axis motion, enabling accurate gouge/overcut detection and error-driven path optimization without full SV computation. Foundational for envelope-based 5-axis CAM.

**Limitations.** Per-instant profile, so the full envelope still requires integrating profiles along the path and trimming self-overlaps. Assumes idealized cutter geometry and rigid kinematics (no runout, deflection).

**Kernel relevance.** Shows that for surfaces of revolution (the common tool case) the grazing condition collapses to closed form, which Keel can exploit: a specialized fast path for revolved-profile sweeps (tools, but also turned parts and revolve features) inside the general SV engine.

### Aras, cutter swept envelopes by two-parameter families of spheres

**Citation.** Aras, E. (2009). Generating cutter swept envelopes in five-axis milling by two-parameter families of spheres. *Computer-Aided Design*, 41(2), 95-105.

**Method.** Model the moving cutter as a **canal surface**: the envelope of a one-parameter family of spheres along the tool axis, and the moving tool as a **two-parameter family of spheres** (one parameter along the axis, one along the path). The envelope of a two-parameter sphere family is found via the classical relation between the **characteristic circles** of each sphere and the great circles, giving the grazing points on each section circle analytically. These grazing points split each section circle into two arcs; the feasible contact arc (FCA) is the boundary-contributing arc, which is then intersected with the workpiece for engagement/material-removal computation.

**Results.** Closed-form envelope surfaces for general cutters represented as canal surfaces, with clean analytic grazing-point extraction, well suited to 5-axis simulation and cutter-workpiece engagement (CWE) calculation. Analytic FCA extraction is efficient and robust.

**Limitations.** Requires the tool to be representable as a canal surface (revolved or sphere-swept), so exotic cutters need approximation. Two-parameter envelope algebra is intricate; handling self-intersection of the envelope along curved paths needs care.

**Kernel relevance.** Canal-surface/sphere-family modeling is broadly useful in Keel: it is the same machinery as rolling-ball fillets (Part 1.2), pipe surfaces, and MAT (a solid is a union of medial balls). A robust "envelope of a sphere family" primitive would serve fillets, tool sweeps, and MAT-to-boundary reconstruction alike. Strong argument for investing in sphere-family/canal-surface code as a shared kernel capability.

---

# Part 2: Medial Axis and Midsurface

The medial axis (MA) of a solid is the set of interior points having two or more nearest points on the boundary, equivalently the locus of centers of maximal inscribed balls. The medial axis transform (MAT) adds the radius function r(p) = distance to boundary, so the solid is exactly the union of balls B(p, r(p)) over the MA. The MAT is a complete, reversible, dimension-reduced descriptor: a 3D solid maps to a 2D sheet structure (sheets meeting at seams meeting at junctions) plus a radius field. This is precisely what CAE idealization wants (turn a thin solid into a midsurface) and what thickness analysis wants (r is half the local thickness on a thin region).

## 2.1 Medial axis theory and stability

### Blum, the medial axis transform

**Citation.** Blum, H. (1967). A transformation for extracting new descriptors of shape. In W. Wathen-Dunn (Ed.), *Models for the Perception of Speech and Visual Form* (pp. 362-380). MIT Press.

**Method.** Blum introduced the MAT via the **grassfire analogy**: set fire to the boundary and let the fire front propagate inward at unit speed; the medial axis is the set of quench points where fronts from different boundary parts meet. Equivalently the MA is the ridge (non-smooth locus) of the distance-to-boundary function, and each MA point carries the arrival time = inscribed-ball radius. Motivated by biological shape perception.

**Results.** Established the skeleton as a compact, reversible shape descriptor uniting topology (the skeleton's graph) and geometry (the radius field). The conceptual foundation for all subsequent skeletonization, idealization, and thickness work.

**Limitations.** As originally defined the MAT is famously **unstable**: an arbitrarily small boundary perturbation can create long spurious branches, so the Hausdorff distance between the MATs of a shape and a slightly noisy version can be large. This instability is the central practical problem of the whole field.

**Kernel relevance.** Defines the target object and the radius field that every Keel skeleton/idealization/thickness service produces. The instability is the reason Keel must always pair MAT computation with a principled pruning/stabilization stage (next entries), never expose a raw MAT.

### Chazal and Lieutier, the lambda-medial axis and pruning theory

**Citation.** Chazal, F., & Lieutier, A. (2005). The "lambda-medial axis." *Graphical Models*, 67(4), 304-331. (Companion: Chazal & Lieutier, Stability and homotopy of a subset of the medial axis.)

**Method.** Define, for each medial point, the radius of the smallest ball enclosing its set of nearest boundary points (the "generators"), call it the local separation. The **lambda-medial axis** M_lambda keeps only medial points whose separation is at least lambda. Equivalently, it filters by how "spread out" the nearest-boundary contacts are: a spurious branch from a tiny bump has nearly-coincident generators (small separation) and is removed; a genuine branch from two well-separated walls survives. They define the **weak feature size (wfs)** as the smallest distance from the boundary's complement to the critical points of the distance function, and prove that for lambda < wfs, M_lambda is **homotopy equivalent** to the shape and is **stable** under small (Hausdorff) perturbations of the boundary.

**Results.** The first single-parameter pruning with rigorous topological and stability guarantees. lambda directly controls the feature scale retained. This is the theoretical license to prune: below wfs you lose nothing topologically, above it you simplify predictably.

**Limitations.** Choosing lambda relative to the (often unknown) wfs needs care; one global lambda may over-prune some regions while under-pruning others. Computing exact separation needs the full nearest-generator set. Stability is in Hausdorff distance, not necessarily in the combinatorial branch structure.

**Kernel relevance.** This is the recommended **default pruning law** for Keel's MAT service: parameterize the skeleton by a single feature-scale lambda with a topological guarantee. Expose lambda as the user-facing "smallest feature to keep" control for idealization and defeaturing, and tie it to local feature size for adaptive variants.

### Giesen, Miklos, et al., the scale axis transform

**Citation.** Giesen, J., Miklos, B., Pauly, M., & Wormser, C. (2009). The scale axis transform. *Proc. 25th ACM Symposium on Computational Geometry (SoCG)*, 106-115. 3D realization: Miklos, B., Giesen, J., & Pauly, M. (2010). Discrete scale axis representations for 3D geometry. *ACM TOG (SIGGRAPH)*, 29(4), Article 101.

**Method.** Work in the union-of-balls picture. Scale every medial ball's radius by a factor s > 1, take the medial axis of that enlarged union (the s-scaled shape), then scale the radii back by 1/s. Small/spurious medial balls get absorbed by their larger neighbors under scaling and disappear, while large structural balls persist, so the **s-scale axis** is a hierarchy of progressively simplified skeletons indexed by s. Discrete 3D version operates directly on a union of polar balls from a point sample, classifies features adaptively, and converts between unions of balls and meshes/images/point clouds.

**Results.** A provably topology-preserving simplification for s below a shape-dependent bound, producing a clean multi-scale skeleton hierarchy. Robust on meshes, images, implicit surfaces, and point clouds. A leading practical 3D skeleton simplifier.

**Limitations.** Aggressive scaling can merge distinct features or change topology beyond the safe bound. The multiplicative scaling can bias radii. Discrete version's quality depends on sampling density.

**Kernel relevance.** Complements lambda-pruning: where lambda thresholds by separation, scale-axis simplifies by ball absorption, which is often more intuitive for "remove features below size X" and gives a hierarchy for level-of-detail. Both should be available; scale-axis pairs especially well with a union-of-balls internal representation that Keel could share with sphere-family sweep code (Part 1.4).

## 2.2 Exact and algebraic MAT for CAD solids

### Culver, Keyser, Manocha, exact medial axis of a polyhedron

**Citation.** Culver, T., Keyser, J., & Manocha, D. (1999/2004). Accurate computation of the medial axis of a polyhedron. *Proc. 5th ACM Symposium on Solid Modeling and Applications*, 179-190; journal version *Computer-Aided Design*, 36(13), 2004 (the gamma.cs.unc.edu MAT project).

**Method.** Compute the internal Voronoi diagram of the polyhedron's boundary features (faces, edges, vertices), whose internal portion is the medial axis. The MA's elements are: **sheets** = bisectors of two features (trimmed quadric surfaces, since face-face bisectors are planar but edge-face and edge-edge bisectors are quadrics), **seams** = curves where three features are equidistant (algebraic space curves), and **junctions** = points equidistant from four features (algebraic points). The algorithm seeds at junctions and **traces** seam curves between neighboring junctions, recovering sheets by traversing closed loops of seams. Everything is done in **exact arithmetic** with specialized accelerations: lazy evaluation via multivariate Sturm sequences, fast resultants, geometric culling, and floating-point filters to avoid expensive exact tests when a fast test suffices.

**Results.** The reference for accurate, topologically correct MAT of polyhedra, representing the skeleton exactly as trimmed quadrics, algebraic curves, and algebraic points. Robust to the degeneracies (cospherical features) that defeat naive Voronoi code.

**Limitations.** Polyhedral input only. Exact arithmetic is slow and the algebraic seam-tracing is intricate; scaling to large meshes is limited. Curved B-reps must be faceted, losing exactness and inflating feature count.

**Kernel relevance.** The exact MAT oracle for polyhedral input and the structural blueprint (sheets = quadrics, seams = algebraic curves, junctions = algebraic points) for how Keel should represent a MAT internally as a non-manifold cell complex with a radius field. The float-filter-then-exact strategy is the standard robustness pattern Keel should adopt across the kernel.

### Sherbrooke, Patrikalakis, Brisson, MAT of 3D polyhedral solids by tracing

**Citation.** Sherbrooke, E. C., Patrikalakis, N. M., & Brisson, E. (1996). An algorithm for the medial axis transform of 3D polyhedral solids. *IEEE Transactions on Visualization and Computer Graphics*, 2(1), 44-61. (Earlier: SMA 1995.)

**Method.** A tracing/marching algorithm that builds the MAT as a connected structure: locate medial **vertices** (junctions) first, then connect them by **tracing along adjacent medial edges** (seams), and finally recover medial **faces** (sheets) by traversing closed loops of the traced edges. The radius function is carried along. Handles the full non-manifold structure of the 3D MAT.

**Results.** One of the first complete, structurally correct 3D polyhedral MAT algorithms producing a proper cell complex (vertices, edges, faces with radii), foundational for engineering applications of the MAT (meshing, idealization).

**Limitations.** Polyhedral input; numerical robustness of the tracing near near-degenerate junctions is delicate (motivated the later exact Culver-Keyser-Manocha work). Tracing can lose connectivity at thin or sharp features.

**Kernel relevance.** Defines the trace-junctions-then-edges-then-faces pipeline that Keel can mirror, and the cell-complex output schema (the natural input to midsurface extraction in 2.4). Read together with Culver et al., these two give Keel both the algorithm shape and the exact-arithmetic hardening.

### Elber and Kim, bisector surfaces for curved-boundary MAT

**Citation.** Elber, G., & Kim, M.-S. (1998). The bisector surface of rational space curves. *ACM Transactions on Graphics*, 17(1), 32-49. Related: Elber & Kim, Bisector curves of planar rational curves, *Computer-Aided Design*, 30(14), 1998; Elber & Kim, A computational model for nonrational bisector surfaces (curve-surface and surface-surface bisectors); Precise Voronoi cell extraction of freeform rational planar closed curves, SPM 2005.

**Method.** The MAT of a curved object is built from **bisectors** of its boundary features, since a medial point is equidistant from two boundary entities. For rational boundary curves/surfaces, derive the bisector exactly: the bisector of two rational space curves is itself **rational** (except the degenerate coplanar case), and curve-surface and surface-surface bisectors are characterized algebraically (generally non-rational, requiring algebraic representation). The MA is then the trimmed arrangement of bisectors that are genuinely closest (the lower envelope of the distance functions), extracted via precise Voronoi-cell construction for freeform rational curves.

**Results.** Exact-as-possible bisector formulations for rational CAD boundaries, the curved-geometry counterpart to the polyhedral quadric-bisector approach, enabling MAT/Voronoi of freeform planar and (partially) spatial domains without faceting.

**Limitations.** Surface-surface bisectors are generally high-degree non-rational, so exact 3D freeform MAT is only partially solved; trimming bisectors to the true medial subset (the closest-pair condition) is expensive. Mostly demonstrated in 2D and curve cases; full 3D freeform-solid MAT remains open.

**Kernel relevance.** This is the path toward MAT directly on Keel's native NURBS B-rep without faceting, at least in 2D (profiles, sketches) and for curve-based features. The bisector machinery doubles as the engine for variable-radius blends and for offset self-intersection prediction (the prior offset research file), reinforcing that bisectors are a shared core capability.

## 2.3 Discrete and approximate MAT, and learned skeletons

### Amenta, Bern, Choi, Kolluri, crust and power crust

**Citation.** Amenta, N., & Bern, M. (1999). Surface reconstruction by Voronoi filtering. *Discrete & Computational Geometry*, 22(4), 481-504. Amenta, N., Choi, S., & Kolluri, R. (2001). The power crust, unions of balls, and the medial axis transform. *Computational Geometry: Theory and Applications*, 19(2-3), 127-153.

**Method.** From a dense surface point sample, build the 3D Voronoi diagram. For a well-sampled surface, each sample's Voronoi cell is long and thin, perpendicular to the surface, with its far ends near the medial axis; the two farthest Voronoi vertices of a cell (the **poles**) approximate medial-axis points (one inside, one outside). The **crust** selects Delaunay triangles whose circumcenters avoid poles to reconstruct the surface; the **power crust** instead takes the **polar balls** (balls centered at poles, radius = distance to the sample) as an approximate union-of-balls / MAT, then separates inner from outer polar balls via a power (weighted Voronoi) diagram, and reconstructs the surface as the boundary between them. Comes with provable geometric-error and topological-correctness guarantees under a sampling condition tied to local feature size.

**Results.** A clean, guaranteed bridge between point samples, the MAT (as inner polar balls), and surface reconstruction, with topology correctness for sufficiently dense samples. The inner polar balls are a directly usable approximate MAT.

**Limitations.** Needs dense, clean, noise-free samples meeting the local-feature-size condition; degrades with noise/undersampling (poles drift). Produces many balls (needs simplification, e.g. Q-MAT). Pure-geometric, no semantics.

**Kernel relevance.** The standard recipe for approximate MAT from tessellation or scan data when an exact path is unavailable: sample the B-rep boundary, take inner polar balls. Pairs naturally with Q-MAT simplification (next) and with the union-of-balls representation Keel can share with scale-axis pruning and sphere-family sweeps.

### Dey and Zhao, medial axis approximation with theoretical guarantees

**Citation.** Dey, T. K., & Zhao, W. (2004). Approximating the medial axis from the Voronoi diagram with a convergence guarantee. *Algorithmica*, 38(1), 179-200. (SoCG/ESA 2002 origins.)

**Method.** Refine the pole-based approximation with two scale- and angle-based filters that select Voronoi facets approximating the medial axis: keep a Voronoi facet if the angle subtended by its two defining samples at the medial point exceeds a threshold (angle condition) and/or the ratio of medial distance to local feature is large enough (ratio condition). Prove that as sampling density increases, the filtered set **converges** to the true medial axis in both geometry and topology.

**Results.** Turned pole heuristics into a method with a convergence guarantee, with explicit, tunable filter parameters, a principled discrete MAT extractor that prunes noise while provably retaining the true axis in the limit.

**Limitations.** Guarantees are asymptotic in sample density; finite samples still need parameter tuning. Noise sensitivity remains; thresholds trade completeness against spuriousness.

**Kernel relevance.** Provides the **filtering criteria** (angle and ratio tests) to convert a raw polar-ball MAT into a clean one, complementary to lambda and scale-axis pruning. Together with Amenta and Chazal-Lieutier, gives Keel a layered approximate-MAT stack: sample, polar balls, angle/ratio filter, lambda/scale prune.

### Li, Wang, Sun, Guo, Zhang, Wang, Q-MAT (quadratic-error MAT simplification)

**Citation.** Li, P., Wang, B., Sun, F., Guo, X., Zhang, C., & Wang, W. (2015). Q-MAT: computing medial axis transform by quadratic error minimization. *ACM Transactions on Graphics*, 35(1), Article 8. Extension: Q-MAT+ (CAGD, 2019).

**Method.** Adapt the quadric error metric (QEM) of mesh decimation to the **medial mesh** (vertices are medial spheres = center + radius, a point in R^4). Define a quadratic error that measures, when two medial spheres are collapsed into one, how much the reconstructed boundary (the envelope of the spheres) moves. Iteratively contract medial edges in order of increasing error, choosing the merged sphere (in R^4) that minimizes the quadratic. Add a quantitative test that flags and removes **unstable branches** (spurious spikes) before they distort the metric.

**Results.** Produces a structurally simple, compact, geometrically accurate MAT from a dense/noisy input MAT, with controllable boundary-approximation error. Open-source (GitHub). The de facto standard for MAT simplification and a strong companion to power-crust-style extraction.

**Limitations.** Needs an initial MAT (from polar balls etc.), so it inherits that stage's noise. The R^4 QEM is a boundary-error proxy, not a topological guarantee (Q-MAT+ improves error control and feature sensitivity). Greedy contraction can make locally suboptimal merges.

**Kernel relevance.** This is Keel's recommended **MAT post-processor**: take any raw MAT (exact polyhedral, or approximate polar-ball) and reduce it to a clean, compact medial mesh with bounded boundary error, the form best suited to midsurface extraction and thickness queries. Its R^4 sphere representation aligns with the union-of-balls theme.

### Lin et al., Point2Skeleton (learned skeletal/MAT representations)

**Citation.** Lin, C., Li, C., Liu, Y., Chen, N., Choi, Y.-K., & Wang, W. (2021). Point2Skeleton: learning skeletal representations from point clouds. *Proc. IEEE/CVF CVPR 2021*, 4277-4286. Related: P2MAT-NET (learning MAT from sparse point clouds, 2020).

**Method.** Unsupervised deep network that predicts skeletal spheres (center + radius, i.e. MAT points) directly from a point cloud by learning a geometric transformation, then learns the **connectivity** among skeletal points to form a skeletal mesh. Grounds the learning in the MAT's mathematical definition (so it generalizes beyond tubular shapes and tolerates non-watertight, noisy input), and supervises only via reconstruction/coverage losses (no ground-truth skeleton needed).

**Results.** Generalized, robust skeletons from raw, possibly incomplete point clouds where classical Voronoi methods fail (noise, non-watertight). Benefits downstream reconstruction and segmentation. Represents the current learning-based direction for skeletonization.

**Limitations.** Learned, so no formal geometric/topological guarantees and possible domain shift on CAD-specific geometry; output sphere count and connectivity are network-dependent; needs training data distribution to match targets. Not exact.

**Kernel relevance.** Of limited immediate use for an exact kernel, but relevant for two scenarios: MAT from scanned/reverse-engineered point data (reverse engineering import), and fast approximate skeletons for interactive feedback. A pragmatic role is initialization: a learned skeleton seeds a classical exact/Q-MAT refinement. Keel should treat learned MAT as an optional front-end, never as the authoritative result.

### Tagliasacchi et al., 3D skeletons state-of-the-art survey

**Citation.** Tagliasacchi, A., Delame, T., Spagnuolo, M., Amenta, N., & Telea, A. (2016). 3D skeletons: a state-of-the-art report. *Computer Graphics Forum (Eurographics STAR)*, 35(2), 573-597.

**Method.** Survey. Defines and contrasts the skeleton families (medial surfaces / MAT, curve skeletons, and others), proposes a **taxonomy** by properties (thinness, centeredness, homotopy/topology preservation, reconstruction ability, stability under noise, invariances), and reviews computation methods (Voronoi/Delaunay, distance-field/thinning, general-field, geometric/contraction) with assumptions, strengths, and limitations, then surveys applications.

**Results.** The single best orientation map of the field. Makes explicit the property trade-offs every method navigates (e.g. exact MAT is reconstruction-complete but unstable; curve skeletons are stable and simple but lossy).

**Limitations.** A 2016 snapshot, so it predates the recent learning-based and restricted-power-diagram MAT work; a survey, not an algorithm.

**Kernel relevance.** The checklist for specifying Keel's skeleton service: decide, per use case, which properties are required (idealization wants reconstruction + topology + stability; thickness wants radius accuracy; defeaturing wants stable feature-scale control) and pick the method/pruning accordingly. Use it to structure the API around guaranteed properties.

## 2.4 Midsurface extraction for CAE

### Rezayat, midsurface abstraction by face pairing

**Citation.** Rezayat, M. (1996). Midsurface abstraction from 3D solid models: general theory and applications. *Computer-Aided Design*, 28(11), 905-915.

**Method.** Build a **face-adjacency graph** of the solid. Identify **face-pairs**: pairs of (roughly) opposing faces whose separation is below a thickness threshold (the two sides of a thin wall). For each pair, generate a midsurface patch halfway between them. Then **extend and stitch** the patches across the part, trimming and joining at junctions to produce a connected midsurface, and abstract the wall thickness as patch attributes for the downstream shell FE model.

**Results.** The first general midsurface-abstraction theory and the basis of most commercial midsurface tools. Direct, intuitive, and fast for clean, locally prismatic thin-walled parts.

**Limitations.** Brittle: face-pair identification fails on complex geometry (non-planar faces, tapers, varying thickness, T/L junctions, ribs meeting walls), and patch extension/stitching is heuristic and frequently produces gaps or wrong connectivity at junctions. This is the well-documented failure mode of commercial face-pairing midsurfacers, which require heavy manual cleanup.

**Kernel relevance.** Defines the baseline midsurface feature Keel should ship (fast, B-rep-native, good on simple parts) and, crucially, defines its known failure cases. The fix for those cases is the MAT-based approach (next), so Keel should offer face-pairing as the fast path and MAT-based as the robust fallback for complex regions.

### Armstrong, Robinson, Donaghy et al., MAT-based dimensional reduction and idealization

**Citation.** Donaghy, R. J., Armstrong, C. G., & Price, M. A. (2000). Dimensional reduction of surface models for analysis. *Engineering with Computers*, 16(1), 24-35. Robinson, T. T., Armstrong, C. G., et al. (2006-2013) on automatic dimensional reduction and meshing of stiffened thin-wall structures (e.g. *Engineering with Computers*, 30, 2014). Foundational group work: Armstrong, C. G. (1994). Modelling requirements for finite-element analysis. *Computer-Aided Design*, 26(7).

**Method.** Use the MAT as the rigorous engine for idealization. Compute the medial axis/surface of the part; the **radius field** is half the local thickness and the medial **sheets** are candidate midsurfaces. Classify medial topology to recognize idealizable regions: thin sheet-like regions (collapse the solid to a shell/midsurface), slender beam-like regions (collapse to a beam/centerline), and transitions. Define **geometric measures** from the MAT (aspect ratios of medial radius to extent) to decide where dimensional reduction is valid versus where full 3D must be retained, and to drive the idealization automatically. Robinson et al. extend this to **stiffened thin-wall structures** (sheet plus rib/stiffener decomposition) with automatic dimensional reduction and compatible meshing across the shell/beam interfaces.

**Results.** A principled, automatable replacement for ad hoc face-pairing: the MAT both finds the midsurface (medial sheets) and certifies where reduction is valid (radius-vs-extent measures), handling varying thickness, ribs, and stiffeners that defeat face-pairing. The basis of robust idealization research and several commercial/academic tools.

**Limitations.** Inherits MAT instability (needs the pruning of 2.1) and the cost/robustness limits of MAT computation. Stitching medial sheets into a clean, connected, analysis-ready midsurface with correct beam/shell transitions is still involved. Mostly demonstrated on prismatic-ish and stiffened parts.

**Kernel relevance.** This is the **target architecture for Keel's idealization service**: MAT (pruned) -> classify medial regions (sheet/beam/3D) -> emit midsurfaces from medial sheets with a thickness attribute from the radius field -> handle rib/stiffener transitions. It directly consumes the MAT representation from 2.2/2.3 and the pruning from 2.1, closing the loop from skeleton to deliverable CAE idealization.

### Recent automatic midsurface work (chordal axis, solid deflation, divide-and-conquer, learning)

**Citation.** Representative recent works: Quadros et al. / chordal-axis-transform non-manifold midsurfaces (*Engineering with Computers*); Sheen et al. (2010), Transformation of a thin-walled solid model into a surface model via solid deflation, *Computer-Aided Design*, 42(8); divide-and-conquer midsurface abstraction (*Computer-Aided Design*, 2013); virtual-topology mid-surface mesh abstraction (*Computer-Aided Design*, 2025); MidSurfer (*Computer-Aided Design*, 2025).

**Method.** A spectrum of strategies to make midsurfacing robust on real parts: (1) **chordal axis transform** (a Delaunay-based discrete medial-surface variant) to produce non-manifold midsurfaces directly from a tessellation; (2) **solid deflation**, which iteratively shrinks the solid (offset inward) until it collapses to a surface, a morphological/erosion view that sidesteps explicit face-pairing; (3) **divide-and-conquer**, segmenting the part into simpler thin regions, midsurfacing each, then stitching; (4) **virtual-topology** mesh abstraction that builds the midsurface mesh on a virtual-topology overlay; (5) variable-thickness handling and (increasingly) learning-assisted region recognition.

**Results.** Steadily improving robustness and automation on complex, variable-thickness, multi-junction parts where Rezayat-style pairing fails, with non-manifold output (needed where three or more walls meet, e.g. ribs) handled natively by the chordal/medial methods. Active area (multiple 2025 papers), indicating the problem is still not fully solved commercially.

**Limitations.** No single method is robust on all parts; each trades generality for cleanliness. Solid deflation inherits offset self-intersection issues (same medial-axis-driven failure as shelling). Output often a mesh midsurface, not exact trimmed surfaces, needing surface fitting for a clean B-rep deliverable.

**Kernel relevance.** Confirms midsurfacing is an open, valuable problem where a robust kernel can differentiate. The pragmatic Keel plan: face-pairing fast path (Rezayat) + MAT/medial-sheet robust path (Armstrong) + solid-deflation/erosion fallback for awkward regions, all sharing one offset/medial engine, with non-manifold output support from the start (rib junctions are non-manifold). Note the explicit link: solid deflation = inward offset to collapse = exactly the offset/medial self-intersection machinery the prior research file covered, so these features share code.

## 2.5 Thickness analysis and applications tying back

### Wall-thickness analysis: rolling-ball (inscribed sphere) vs ray methods

**Citation.** Industrial practice and DfM literature; representative tool documentation: 3DViewStation (Kisters) wall-thickness via rolling ball; Glovius wall-thickness analysis for casting and injection molding. Conceptual basis is Blum's MAT radius field (1967) and the inscribed-sphere thickness measure.

**Method.** Two dominant local-thickness definitions. (1) **Ray / opposing-wall method**: from a surface point, cast a ray (often along the inward normal) and measure distance to the opposite wall. Fast, intuitive, good for nominally uniform sheet-metal-like parts, but direction-dependent and wrong at corners, curved zones, and oblique walls. (2) **Rolling-ball / inscribed-sphere method (MAT-based)**: thickness at a point is twice the radius of the largest inscribed sphere touching that surface point, i.e. 2 r where r is the MAT radius of the nearest medial point. Direction-independent and correct in curved zones and inner corners; this is the MAT radius field evaluated at the boundary. Results are visualized as thickness heatmaps; thin regions flag moldability/structural risks, thick regions flag sink-mark and long-cooling risks for molding.

**Results.** The rolling-ball/MAT method is the accurate industrial standard for complex and molded geometry; the ray method survives as a fast approximation for uniform parts. Both are routine in DfM/moldability checks.

**Limitations.** Ray method mis-measures at corners and non-parallel walls. Rolling-ball needs a MAT or a robust largest-inscribed-sphere query (a distance-field/optimization problem) and is more expensive. Neither alone captures process physics (just geometry).

**Kernel relevance.** Wall-thickness analysis is a **direct, high-value consumer of the MAT radius field**: once Keel computes a (pruned) MAT, thickness = 2r is essentially free, and thin-region detection for moldability is a threshold on r. This is one of the clearest ROI arguments for building the MAT service. Provide both modes (fast ray, accurate inscribed-sphere) sharing the kernel's distance/medial machinery.

### Tying back: shelling safety, mesh sizing, defeaturing guidance

**Citation.** Synthesis across the above plus the prior offset/shelling research file; mesh-sizing-from-MAT lineage: Quadros, W. R., Owen, S. J., et al. (medial-axis-based sizing functions for meshing); local-feature-size meshing from Amenta-style sampling theory.

**Method.** The MAT radius field is the unifying quantity. (1) **Shelling safety**: an inward shell of wall t is valid only where the local MAT radius r >= t; regions with r < t are exactly where the inward offset self-intersects and the shell fails, so the MAT predicts shell feasibility before attempting the Boolean. (2) **Mesh sizing fields**: element size should track **local feature size** (distance to the MAT), which the MAT provides directly, so a sizing field h(x) proportional to local feature size yields well-graded meshes that resolve thin regions. (3) **Defeaturing guidance**: features whose MAT separation (lambda) or scale-axis level is below a threshold are exactly the small features safe to remove, so the same pruning that stabilizes the MAT enumerates defeaturing candidates with topological guarantees.

**Results.** One computed-and-pruned MAT serves shelling validity, mesh sizing, defeaturing, idealization, and thickness analysis, a single skeleton service feeding five downstream capabilities.

**Limitations.** All inherit MAT computation cost and the need for correct pruning; tying CAE decisions to a geometric skeleton still requires engineering judgment thresholds.

**Kernel relevance.** This is the strategic payoff: the MAT is not one feature but the **shared substrate** for an entire manufacturability/idealization/meshing subsystem in Keel. Build it once, robustly, with pluggable pruning, and expose the radius and local-feature-size fields as first-class queries.

---

# Sweep and skeleton services for Keel

The two parts of this dossier are one machine viewed from two sides. The sweep side (Minkowski, swept volumes) computes envelopes of moving boundaries via the grazing condition v · n = 0. The skeleton side (MAT, midsurface, thickness) computes the dual structure: the ridge of the distance field, equivalently the envelope of the family of maximal inscribed balls. Both reduce to robust handling of distance fields, envelopes, and self-intersection trimming. Keel should therefore build a **shared envelope/distance-field core** and layer both service families on top, rather than implementing sweeps and skeletons as unrelated subsystems.

**Recommended sweep stack.**
- 2D Minkowski (clearance, nesting, profile offset): native **convolution** engine (Guibas-Ramshaw-Stolfi / Wein), which also serves 2D MAT via bisectors. Highest near-term ROI and lowest risk.
- 3D Minkowski exact mode: **CGAL-style Nef + convex-decomposition** (Hachenberger) as a correctness oracle and for certification-grade results; accept the cubic cost for small models.
- 3D Minkowski / swept-volume interactive mode: **distance-field accumulation** (Peternell/Schroeder) with **robust culling** (Li-McMains) and **superset-of-ruled-primitives** enumeration (Kim et al.); GPU via wgpu where available, CPU fallback always.
- High-fidelity swept volume: **space-time numerical continuation on the v · n = 0 set** (Sellan et al. 2021, with 2025 detail-preserving refinements), seeded from the distance-field topology; refine boundary curves with **SEDE** (Blackmore) where ordered curves help surfacing. The **Jacobian rank-deficiency** formulation (Abdel-Malek-Yeh) is the spec these numerics implement.
- Specialized fast paths: **canal-surface / sphere-family** envelopes (Aras) and **revolved-profile grazing in closed form** (Chiou-Lee) for tools, fillets, and turned/revolved features. Rounding and filleting are **ball Minkowski opening/closing** (Mason morphology), sharing the sphere-family code.

**Recommended skeleton stack.**
- Internal representation: a **non-manifold medial cell complex** (sheets = trimmed quadrics, seams = algebraic curves, junctions = algebraic points, per Culver-Keyser-Manocha and Sherbrooke et al.) plus a **radius field**, equivalently a **union-of-balls** (shared with sphere-family sweep code and scale-axis pruning).
- Exact MAT: **tracing + exact arithmetic** for polyhedral input (Sherbrooke/Culver), **bisectors of rational boundaries** (Elber-Kim) toward faceting-free 2D and curve MAT.
- Approximate MAT: **polar balls from boundary samples** (Amenta power crust) with **angle/ratio convergence filters** (Dey-Zhao); **learned skeletons** (Point2Skeleton) only as an optional front-end for scan/reverse-engineering data.
- Stabilization (mandatory, never expose raw MAT): **lambda-medial-axis** pruning with its wfs topological guarantee (Chazal-Lieutier) as the default single-knob "smallest feature to keep", **scale-axis** for multi-scale LOD (Giesen-Miklos), and **Q-MAT** quadratic-error simplification (Li et al.) as the compacting post-process. The Tagliasacchi survey is the property checklist for choosing among them per use case.

**Downstream services from one MAT.** Midsurface/idealization (face-pairing fast path per Rezayat, MAT medial-sheet robust path per Armstrong/Robinson/Donaghy, solid-deflation fallback), wall-thickness (inscribed-sphere = 2r accurate mode, ray fast mode), shelling-validity prediction (r >= t), mesh sizing fields (local feature size), and defeaturing candidate enumeration (sub-lambda features). These five capabilities all read the same pruned MAT radius and local-feature-size fields.

**Cross-cutting requirements.** Every operation here is a robustness problem first and an algorithm second: float-filter-then-exact predicates (Culver), distance-field level-set trimming to dodge explicit self-intersection (Kim, Peternell, Sellan), and mandatory MAT pruning (Chazal-Lieutier). Self-intersection trimming, the rolling-ball/canal-surface primitive, and bisector computation each appear in both the sweep and skeleton stacks, so they are the three core investments that unlock the most features per unit of engineering.

---

# References

Abdel-Malek, K., & Yeh, H.-J. (1997). Geometric representation of the swept volume using Jacobian rank-deficiency conditions. *Computer-Aided Design*, 29(6), 457-468.

Abdel-Malek, K., Yang, J., Blackmore, D., & Joy, K. (2006). Swept volumes: foundations, perspectives, and applications. *International Journal of Shape Modeling*, 12(1), 87-127.

Amenta, N., & Bern, M. (1999). Surface reconstruction by Voronoi filtering. *Discrete & Computational Geometry*, 22(4), 481-504.

Amenta, N., Choi, S., & Kolluri, R. (2001). The power crust, unions of balls, and the medial axis transform. *Computational Geometry: Theory and Applications*, 19(2-3), 127-153.

Aras, E. (2009). Generating cutter swept envelopes in five-axis milling by two-parameter families of spheres. *Computer-Aided Design*, 41(2), 95-105.

Armstrong, C. G. (1994). Modelling requirements for finite-element analysis. *Computer-Aided Design*, 26(7), 573-578.

Blackmore, D., & Leu, M.-C. (1992). Analysis of swept volume via Lie groups and differential equations. *International Journal of Robotics Research*, 11(6), 516-537.

Blackmore, D., Leu, M.-C., & Wang, L. P. (1997). The sweep-envelope differential equation algorithm and its application to NC machining verification. *Computer-Aided Design*, 29(9), 629-637.

Blum, H. (1967). A transformation for extracting new descriptors of shape. In W. Wathen-Dunn (Ed.), *Models for the Perception of Speech and Visual Form* (pp. 362-380). MIT Press.

Chazal, F., & Lieutier, A. (2005). The "lambda-medial axis." *Graphical Models*, 67(4), 304-331.

Chiou, C.-J., & Lee, Y.-S. (2002). A machining potential field approach to tool path generation for multi-axis sculptured surface machining. *Computer-Aided Design*, 34(5), 357-371.

Culver, T., Keyser, J., & Manocha, D. (2004). Exact computation of the medial axis of a polyhedron. *Computer Aided Geometric Design* / Proc. ACM Solid Modeling 1999.

Dey, T. K., & Zhao, W. (2004). Approximating the medial axis from the Voronoi diagram with a convergence guarantee. *Algorithmica*, 38(1), 179-200.

Donaghy, R. J., Armstrong, C. G., & Price, M. A. (2000). Dimensional reduction of surface models for analysis. *Engineering with Computers*, 16(1), 24-35.

Elber, G., & Kim, M.-S. (1998). The bisector surface of rational space curves. *ACM Transactions on Graphics*, 17(1), 32-49.

Giesen, J., Miklos, B., Pauly, M., & Wormser, C. (2009). The scale axis transform. *Proc. 25th ACM Symposium on Computational Geometry (SoCG)*, 106-115.

Guibas, L., Ramshaw, L., & Stolfi, J. (1983). A kinetic framework for computational geometry. *Proc. 24th IEEE FOCS*, 100-111.

Hachenberger, P. (2009). Exact Minkowski sums of polyhedra and exact and efficient decomposition of polyhedra into convex pieces. *Algorithmica*, 55(2), 329-345.

Kim, Y. J., Varadhan, G., Lin, M. C., & Manocha, D. (2004). Fast swept volume approximation of complex polyhedral models. *Computer-Aided Design*, 36(11), 1013-1027.

Li, P., Wang, B., Sun, F., Guo, X., Zhang, C., & Wang, W. (2015). Q-MAT: computing medial axis transform by quadratic error minimization. *ACM Transactions on Graphics*, 35(1), Article 8.

Li, W., & McMains, S. (2011). Voxelized Minkowski sum computation on the GPU with robust culling. *Computer-Aided Design*, 43(10), 1270-1283.

Lin, C., Li, C., Liu, Y., Chen, N., Choi, Y.-K., & Wang, W. (2021). Point2Skeleton: learning skeletal representations from point clouds. *Proc. IEEE/CVF CVPR 2021*, 4277-4286.

Lozano-Perez, T. (1983). Spatial planning: a configuration space approach. *IEEE Transactions on Computers*, C-32(2), 108-120.

Miklos, B., Giesen, J., & Pauly, M. (2010). Discrete scale axis representations for 3D geometry. *ACM Transactions on Graphics (SIGGRAPH)*, 29(4), Article 101.

Peternell, M., Pottmann, H., Steiner, T., & Zhao, H. (2005). Swept volumes. *Computer-Aided Design and Applications*, 2(5), 599-608.

Rezayat, M. (1996). Midsurface abstraction from 3D solid models: general theory and applications. *Computer-Aided Design*, 28(11), 905-915.

Robinson, T. T., Armstrong, C. G., et al. (2014). Automatic dimensional reduction and meshing of stiffened thin-wall structures. *Engineering with Computers*, 30.

Schroeder, W. J., Lorensen, W. E., & Linthicum, S. (1994). Implicit modeling of swept surfaces and volumes. *Proc. IEEE Visualization 1994*, 40-45.

Sellan, S., Aigerman, N., & Jacobson, A. (2021). Swept volumes via spacetime numerical continuation. *ACM Transactions on Graphics (SIGGRAPH)*, 40(4).

Sheen, D.-P., et al. (2010). Transformation of a thin-walled solid model into a surface model via solid deflation. *Computer-Aided Design*, 42(8), 720-730.

Sherbrooke, E. C., Patrikalakis, N. M., & Brisson, E. (1996). An algorithm for the medial axis transform of 3D polyhedral solids. *IEEE Transactions on Visualization and Computer Graphics*, 2(1), 44-61.

Tagliasacchi, A., Delame, T., Spagnuolo, M., Amenta, N., & Telea, A. (2016). 3D skeletons: a state-of-the-art report. *Computer Graphics Forum (Eurographics STAR)*, 35(2), 573-597.

Wein, R. (2006). Exact and efficient construction of planar Minkowski sums using the convolution method. *Proc. ESA 2006*, LNCS 4168, 829-840.

Williams, J., & Rossignac, J. (2005). Mason: morphological simplification. *Graphical Models*, 67(4), 285-303.
