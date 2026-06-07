# NURBS Applications and Modern Extensions: A Deep Literature Review for Keel

## Title and Scope

This document is the applications-and-extensions deep dive supporting the design of **Keel**, an open-source B-rep solid modeling kernel with Parasolid-class ambition. An earlier survey touched on isogeometric analysis (IGA), T-splines, THB-splines, subdivision evaluation, and watertight trimmed NURBS at a one-paragraph level. This file goes for depth on what *uses* NURBS and B-reps, and on the spline technologies that have grown out of and beyond classical NURBS.

The intent is not abstract mathematics (covered elsewhere) but the working question: **given that people build ships, cars, wings, blades, buildings, and ML pipelines on top of NURBS B-reps, what does each application domain actually demand from the kernel underneath it?** Every entry closes with a "Kernel implication" because the deliverable is design pressure on Keel, not a bibliography.

Themes covered:

1. Isogeometric analysis in depth (foundations, Bezier extraction, trimmed/immersed IGA, analysis-suitable geometry)
2. Spline extensions beyond NURBS (T-splines, LR B-splines, THB-splines, U-splines, comparative assessments)
3. Trimmed surface tessellation and rendering (adaptive/watertight tessellation, GPU pipelines, ray tracing, containment queries)
4. CAD data exchange (STEP AP203/214/242, IGES, tolerance and healing)
5. Volumetric/trivariate NURBS and V-reps (additive manufacturing, lattices)
6. Industrial application domains (ship hulls, automotive Class-A, aerospace, turbomachinery, architecture)
7. NURBS in graphics and ML (differentiable CAD, B-rep learning, point-cloud reconstruction)
8. Open-source ecosystem survey

A style note: this document uses no em-dashes anywhere, by deliberate constraint.

---

## 1. Isogeometric Analysis in Depth

IGA is the single most important reason a modern geometry kernel cannot treat its spline representation as a passive display artifact. IGA says: the same NURBS that defines the part is also the analysis basis. That couples kernel design decisions (knot structure, refinement, trimming) directly to whether downstream simulation is even possible.

### 1.1 Hughes, Cottrell, and Bazilevs (2005): the founding paper

**Citation.** Hughes, T. J. R., Cottrell, J. A., & Bazilevs, Y. (2005). Isogeometric analysis: CAD, finite elements, NURBS, exact geometry and mesh refinement. *Computer Methods in Applied Mechanics and Engineering, 194*(39-41), 4135-4195.

**Content.** This paper proposed using the NURBS basis that already describes CAD geometry directly as the basis for finite element analysis, eliminating the lossy mesh-generation step that traditionally sits between design and simulation and that consumes the majority of analysis engineer time. The geometry is *exact* at every level of refinement: there is no polygonal approximation that converges to the true shape, because the true shape is the basis. The authors carried the standard FEA refinement vocabulary over to splines: h-refinement (knot insertion, adds elements), p-refinement (order elevation), and a genuinely new scheme they named **k-refinement**, in which order elevation is performed *before* knot insertion so that the inserted knots inherit the elevated, higher-continuity basis. k-refinement produces fields that are C^(p-1) across element boundaries rather than the C^0 of classical FEA, with dramatically fewer degrees of freedom for the same accuracy. They demonstrated superior per-DOF accuracy and notably better spectral (eigenvalue) behavior, important for structural dynamics and wave problems. Crucially, refinement is performed *without returning to the CAD system*, because the refined representation still describes the identical geometry and parameterization.

**Limitations.** Tensor-product NURBS refinement is global: inserting a knot to refine one region propagates a full row or column across the patch. Multi-patch models need C^0 or weakly enforced coupling at patch interfaces. Trimmed geometry, which is the overwhelming majority of real CAD, is not addressed by the basic theory at all.

**Kernel implication.** If Keel wants to be analysis-relevant rather than display-only, its spline representation must support *exact-geometry refinement*: knot insertion and order elevation that change the basis without perturbing the surface. The kernel should store knot vectors and control nets as first-class, queryable, refineable data, not bake everything into evaluated meshes. The global-propagation problem of tensor-product refinement is precisely what motivates the spline extensions in Section 2, so Keel's surface abstraction should be designed to admit local-refinement bases later rather than hard-coding tensor-product assumptions.

### 1.2 Cottrell, Hughes, and Bazilevs (2009): the consolidating book

**Citation.** Cottrell, J. A., Hughes, T. J. R., & Bazilevs, Y. (2009). *Isogeometric Analysis: Toward Integration of CAD and FEA*. Wiley.

**Content.** The book systematized the field: NURBS as analysis basis, k-refinement, structural vibration and shell formulations, fluid-structure interaction, and the foundational treatment of T-splines and local refinement for analysis. It established the conceptual frame that geometry and analysis should share one model, and it catalogued the open problems (trimming, local refinement, robust quadrature) that defined the next fifteen years of research.

**Limitations.** As a 2009 snapshot it predates the maturation of analysis-suitable T-splines, LR B-splines, THB-splines, and immersed methods for trimmed CAD, all of which it gestures at but does not resolve.

**Kernel implication.** The "one model" thesis is the strongest possible argument that Keel should not maintain a separate "analysis mesh" data path. The same B-rep that the modeler edits should carry enough structure (continuity, parameterization, refineability) to be consumed by an analysis client. This is an architectural commitment, not a feature: it argues for clean separation between geometry definition and any evaluated/tessellated caches.

### 1.3 Borden, Scott, Evans, and Hughes (2011): Bezier extraction

**Citation.** Borden, M. J., Scott, M. A., Evans, J. A., & Hughes, T. J. R. (2011). Isogeometric finite element data structures based on Bezier extraction of NURBS. *International Journal for Numerical Methods in Engineering, 87*(1-5), 15-47.

**Content.** This is the paper that made IGA *implementable inside existing FEA codes*. The smooth, globally-coupled NURBS basis is awkward for traditional element-by-element finite element machinery. Bezier extraction defines, per element, a constant **extraction operator** C (a sparse matrix) such that the NURBS basis restricted to that element equals C times the local Bernstein (Bezier) basis. The Bezier basis is C^0, element-local, and identical on every element up to the operator, so it slots directly into the standard FEA loop: element formation, Gauss quadrature on Bernstein polynomials, and assembly proceed unchanged; only the element extraction operator differs. The smoothness of the original NURBS is recovered exactly through assembly. A companion paper (Scott et al., 2011, *IJNME* 88, 126-156) extended extraction to T-splines, which is what made unstructured analysis-suitable splines practical.

**Limitations.** Extraction is a representation change for *evaluation and assembly*; it does not by itself solve local refinement or trimming. The number of nonzeros in extraction operators grows with degree and smoothness.

**Kernel implication.** This is arguably the most directly actionable IGA result for Keel. If the kernel can emit per-element Bezier extraction operators alongside its NURBS surfaces, any FEA or rendering client gets a uniform, C^0, element-local interface to smooth geometry without understanding global knot vectors. Bezier extraction is also a natural *internal* representation for the kernel's own evaluators and tessellators (everything reduces to Bernstein evaluation, which is numerically stable via de Casteljau). Keel should consider Bezier elements as a canonical evaluation form and store/compute extraction operators as part of its surface caching layer.

### 1.4 Trimmed-domain IGA and the immersed / finite cell methods

**Citation.** Schillinger, D., & Ruess, M. (2015). The Finite Cell Method: A review in the context of higher-order structural analysis of CAD and image-based geometric models. *Archives of Computational Methods in Engineering, 22*(3), 391-455.

**Content.** Real CAD geometry is *trimmed*: a face is a NURBS surface plus a set of trimming curves in parameter space that cut away part of the domain. Trimmed regions break tensor-product IGA because the basis no longer aligns with the actual material domain. The Finite Cell Method (FCM) and related immersed/embedded approaches sidestep mesh generation entirely: the geometry is embedded in a simple structured background grid (the "cells"), and the trimming/boundary is enforced through (a) special quadrature that integrates only the material portion of each cut cell, typically via adaptive octree/quadtree subdivision or by reconstructing the cut boundary, and (b) weak enforcement of boundary conditions (Nitsche's method) on surfaces not aligned with the grid. The review surveys FCM applied to higher-order and IGA bases for solids, image-based (CT/voxel) models, fluid-structure interaction, and contact. A recurring research thread, confirmed across the immersed-IGA literature, is high-order B-spline *re-parameterization* of trimmed (cut) elements so that classical Gauss quadrature on the reparameterized cut element achieves optimal energy-norm convergence with far fewer quadrature points, with reported savings exceeding 90% of quadrature points on full knot spans versus naive subdivision.

**Limitations.** Cut cells with tiny material fractions cause ill-conditioning (the "small cut" problem) and require stabilization or preconditioning. Robust, watertight quadrature on trimmed domains remains delicate. Weak boundary enforcement adds parameters (penalty/Nitsche constants) that must be chosen carefully.

**Kernel implication.** Trimming is not a rendering detail Keel can paper over; it is the central data-structure decision that determines whether the kernel's output can be analyzed at all. Keel must represent trimmed faces *precisely*: the underlying (untrimmed) NURBS surface, the parameter-space trim loops, and the 3D trim curves, kept consistent. Critically, downstream IGA needs the kernel to answer **"is this parameter point inside the trimmed domain?"** robustly and to supply quadrature-friendly descriptions of trimmed regions (e.g. boundary reparameterizations). This argues that point-in-trimmed-face classification and trim-loop reparameterization should be core, well-tested kernel services, not afterthoughts. See Section 3.5 (generalized winding numbers) for the modern robust answer.

### 1.5 Why IGA matters to kernel design: the invariants

Synthesizing the above, IGA imposes three invariants on any kernel that wants to feed it. **Refinement invariance:** the geometry and its parameterization must be preserved exactly under h-, p-, and k-refinement. **Quadrature-readiness:** the kernel must expose enough structure (Bezier elements, trim domains) that integration over faces and trimmed faces is well-defined and efficient. **Analysis-suitable geometry:** the basis should ideally be locally refineable, linearly independent, nonnegative, and form a partition of unity (the properties that classical tensor NURBS have globally but lose under naive local refinement). These three invariants are the bridge from "Keel as a CAD kernel" to "Keel as an analysis-grade kernel," and they are why the spline extensions below are not academic curiosities but candidate core representations.

---

## 2. Spline Extensions Beyond NURBS

Tensor-product NURBS has one structural flaw that everything in this section attacks: you cannot refine it locally. The remedies trade away the clean tensor structure for local adaptivity, and each remedy has a different position on the spectrum of "easy to implement" versus "provably analysis-suitable."

### 2.1 T-splines (Sederberg et al., 2003): T-junctions in the control grid

**Citation.** Sederberg, T. W., Zheng, J., Bakenov, A., & Nasri, A. (2003). T-splines and T-NURCCs. *ACM Transactions on Graphics (SIGGRAPH), 22*(3), 477-484.

**Content.** A T-spline is a NURBS generalization whose control grid is allowed to contain **T-junctions**: a row or column of control points need not run all the way across the grid. This means local detail can be added without propagating a full row/column of control points globally, which is exactly what tensor NURBS forbids. Each blending function is inferred from local knot intervals read off the T-mesh. The paper showed two headline operations: (1) *local refinement*, adding control points only where needed, and (2) **merging multiple NURBS patches with different knot vectors into a single gap-free surface**, which directly attacks the watertightness problem of multi-patch B-reps. T-NURCCs (Non-Uniform Rational Catmull-Clark surfaces with T-junctions) extend the construction to arbitrary topology, unifying T-splines with subdivision. A 2004 SIGGRAPH companion (Sederberg et al., 2004) detailed T-spline simplification.

**Limitations.** General T-splines are *not guaranteed* to have linearly independent blending functions (Section 2.3), and they do not necessarily form a partition of unity, so arbitrary T-splines are unsafe as an analysis basis. The original construction is geometry-and-design oriented, not analysis-proven.

**Kernel implication.** T-splines deliver the two things B-rep kernels most want: local refinement and watertight patch merging. But the linear-independence failure means Keel cannot adopt *arbitrary* T-splines as a trusted internal representation. The lesson is to adopt the *restricted* analysis-suitable subclass (2.2) if T-splines are used at all, and to treat "merge patches into gap-free surface" as a capability worth having even if implemented by other means.

### 2.2 Analysis-suitable T-splines (Li, Scott, et al.)

**Citation.** Li, X., Zheng, J., Sederberg, T. W., Hughes, T. J. R., & Scott, M. A. (2012). On linear independence of T-spline blending functions. *Computer Aided Geometric Design, 29*(1), 63-76. See also Scott, M. A., Li, X., Sederberg, T. W., & Hughes, T. J. R. (2012). Local refinement of analysis-suitable T-splines. *CMAME, 213-216*, 206-222.

**Content.** Analysis-suitable T-splines (AST) are the restricted subclass of T-splines that *provably* retain the properties analysis needs: linear independence of the blending functions and partition of unity. The restriction is topological, expressed as conditions on the configuration of T-junction extensions in the T-mesh (no overlapping horizontal/vertical T-junction extensions). Within this class the authors gave a *local* refinement algorithm that stays inside the class, and showed AST behave essentially like NURBS for analysis while permitting local refinement. Combined with Bezier extraction of T-splines (Scott et al., 2011), this made AST a usable industrial analysis basis.

**Limitations.** The admissibility conditions constrain mesh design and complicate refinement bookkeeping. Extension to arbitrary unstructured topology and to high degree required further work (Veiga, Buffa, Cho, Sangalli and others). Watertight handling of extraordinary points remains an issue.

**Kernel implication.** AST is the cautionary template for Keel: *a representation can be visually fine yet mathematically unsafe.* If Keel exposes any locally-refineable spline, it should enforce (and be able to *verify*) the admissibility invariants that guarantee linear independence and partition of unity, and its refinement operators must preserve those invariants by construction.

### 2.3 Linear-independence failures (Buffa, Cho, Sangalli, 2010)

**Citation.** Buffa, A., Cho, D., & Sangalli, G. (2010). Linear independence of the T-spline blending functions associated with some particular T-meshes. *Computer Methods in Applied Mechanics and Engineering, 199*(23-24), 1437-1445.

**Content.** This paper supplied the rigorous warning shot: it exhibited T-meshes whose blending functions are *linearly dependent*. Linear dependence is fatal for analysis (the stiffness matrix is singular) and undesirable for geometry (redundant control points). The work characterized conditions under which dependence arises, motivating the precise definition of the analysis-suitable subclass.

**Limitations.** It is an impossibility/characterization result, not a constructive remedy; the remedy is the AST restriction of 2.2.

**Kernel implication.** This is the single clearest argument that Keel must treat "is this basis linearly independent and a partition of unity?" as a *checkable property*, ideally enforced by the refinement API so that no sequence of legal kernel operations can produce a degenerate basis. Robustness reputations of kernels are built or destroyed exactly here.

### 2.4 LR B-splines (Dokken, Lyche, Pettersen, 2013)

**Citation.** Dokken, T., Lyche, T., & Pettersen, K. F. (2013). Polynomial splines over locally refined box-partitions. *Computer Aided Geometric Design, 30*(3), 331-356.

**Content.** LR B-splines (Locally Refined B-splines) take a different route to local refinement. Instead of a T-mesh of control points, they define refinement on a **box-partition** of the parameter domain: refinement proceeds by inserting individual *meshline segments*, and each insertion splits the B-splines that the new line crosses, via the classical knot-insertion relation, into hierarchically scaled B-splines. The result is a collection of LR B-splines forming a nonnegative partition of unity over an "LR-mesh." Refinement is genuinely local and dimension-independent (works in any number of variables, which matters for trivariate volumetric models). LR B-splines were rapidly applied to IGA (Johannessen, Kvamsdal, Dokken, 2014).

**Limitations.** Linear independence is *not automatic*: depending on the refinement sequence, LR B-splines can become linearly dependent, and the literature ("Some properties of LR-splines," Bressan 2013) studies when this happens and how to detect/avoid it. The set of functions and their overlaps must be tracked carefully; bookkeeping is non-trivial.

**Kernel implication.** LR B-splines are attractive for Keel because the meshline-insertion model is conceptually simple and extends cleanly to trivariates (Section 5). But the same linear-independence caveat as T-splines applies, and even more sharply because it is refinement-history-dependent. If Keel adopts LR B-splines, the refinement engine must either restrict to provably-independent insertion strategies or carry a dependence-detection check. The dimension-independence is a strong point for a kernel that wants both surface and volumetric representations from one mechanism.

### 2.5 THB-splines (Giannelli, Juttler, Speleers, 2012)

**Citation.** Giannelli, C., Juttler, B., & Speleers, H. (2012). THB-splines: The truncated basis for hierarchical splines. *Computer Aided Geometric Design, 29*(7), 485-498. See also Giannelli, C., Juttler, B., Kleiss, S. K., Mantzaflaris, A., Simeon, B., & Speh, J. (2016). THB-splines: An effective mathematical technology for adaptive refinement in geometric design and isogeometric analysis. *CMAME, 299*, 337-365.

**Content.** Hierarchical B-splines build a basis from a nested sequence of tensor-product spline spaces, activating coarse functions away from refined regions and fine functions inside them. Classical hierarchical B-splines (HB-splines) lose the partition-of-unity property and have large overlaps. The **truncation** mechanism of THB-splines fixes this: when a fine level is introduced, the coarse functions are *truncated* by subtracting their representation in terms of the finer functions, restoring partition of unity, reducing the supports' overlaps, and improving sparsity and numerical conditioning. THB-splines are nonnegative, form a partition of unity, are linearly independent by construction, and preserve coefficients well under refinement. The 2016 paper gave efficient quadtree/octree-based data structures for construction and evaluation and demonstrated geometric design plus IGA. THB-splines are the basis adopted by major IGA software (e.g. the G+Smo library).

**Limitations.** The hierarchy is still built on tensor-product levels, so the "footprint" of refinement is coarser-grained than fully unstructured T-splines or LR B-splines; refinement is by nested rectangular regions. Strongly graded refinement can require many levels. Extraordinary points / unstructured topology need extra machinery.

**Kernel implication.** THB-splines are the *safest* of the local-refinement options for Keel because linear independence and partition of unity are guaranteed *by construction*, not by an admissibility side-condition that could be violated. The quadtree/octree level structure maps naturally onto a kernel's spatial acceleration structures. If Keel wants one adaptive-refinement scheme it can trust without writing a linear-dependence checker, THB-splines are the leading candidate; the cost is the coarser tensor-level granularity of refinement.

### 2.6 U-splines (Thomas et al., Coreform)

**Citation.** Thomas, D. C., Engvall, L., Schmidt, S. K., Tew, K., & Scott, M. A. (2022). U-splines: Splines over unstructured meshes. *Computer Methods in Applied Mechanics and Engineering, 401*, 115515. (Earlier patent filing 2018; commercialized by Coreform.)

**Content.** U-splines ("U" for unstructured) aim to be the most general analysis-suitable construction: a spline basis over an arbitrary, mixed-element, unstructured Bezier mesh that supports *simultaneous and local* variation of h (element size), p (degree), and k (smoothness) over any topology. The construction starts from a Bezier mesh and solves for a basis that is smooth across element boundaries to the prescribed continuity while remaining a positive partition of unity and (the central claim) linearly independent. A key compatibility property is *pointwise exactness*: when the input Bezier mesh coincides with a single/multi-patch NURBS or an analysis-suitable T-spline, the U-spline algorithm reproduces exactly that spline space and basis. Coreform built its commercial IGA product (Coreform Flex, with LS-DYNA integration) on U-splines.

**Limitations.** The construction is proprietary and patent-encumbered, which directly conflicts with an *open-source* kernel's licensing goals. The general-topology smoothness-solve is algorithmically heavier than tensor or hierarchical schemes. Published independent reproductions are limited compared to THB/LR literature.

**Kernel implication.** U-splines define the aspirational target (h, p, k adaptivity on any topology with guaranteed analysis-suitability) but the patent situation means Keel almost certainly cannot implement them directly. The pointwise-exactness idea is the useful takeaway: a kernel's adaptive basis should *degrade gracefully* to standard NURBS/T-splines on inputs that are already structured, so that interoperability with classical CAD is lossless. Keel should pursue an unencumbered scheme (THB or LR) and treat U-splines as a feature-parity benchmark, not a blueprint.

### 2.7 Comparative assessment

The practical ranking for a kernel:

- **Tensor NURBS:** universal interchange, no local refinement. Mandatory to support for I/O.
- **THB-splines:** guaranteed partition of unity and linear independence by construction; refinement granularity is tensor-rectangular. Lowest robustness risk. Open.
- **LR B-splines:** finest-grained meshline refinement, dimension-independent (good for trivariates); linear independence is refinement-history-dependent and must be checked. Open.
- **Analysis-suitable T-splines:** local refinement plus watertight patch merging; safe only within admissibility conditions that the kernel must enforce. Largely open (concepts), though some toolchains are commercial.
- **U-splines:** most general (any topology, simultaneous hpk); proprietary, unsuitable for an OSS kernel to implement.

A 2016 computational comparison of adaptive refinement strategies in IGA (Hennig, Kastner, Morgenstern, Peterseim; arXiv:1605.00825) found THB and AST-style schemes broadly competitive, with trade-offs in implementation complexity versus refinement locality, reinforcing that no single scheme dominates and that the *robustness guarantees* often matter more than marginal DOF efficiency.

---

## 3. Trimmed Surface Tessellation and Rendering

A trimmed NURBS B-rep is not directly displayable: GPUs rasterize triangles, and ray tracers need intersection tests. The gap between "exact trimmed NURBS" and "pixels" is where an enormous amount of kernel-adjacent engineering lives, and where the watertightness problem becomes visible to users.

### 3.1 The core problem: trim intersections are not NURBS curves

The foundational difficulty, stated cleanly in the watertight-tessellation literature: the 3D intersection curve of two trimmed NURBS surfaces is *in general not itself a NURBS curve*. It can only be approximated. Therefore adjacent trimmed faces that should share an edge actually carry two different approximations of that edge, leaving **gaps and cracks**. Every tessellation and rendering technique below is, at bottom, a strategy for making those gaps invisible (rendering) or provably closed (watertight tessellation).

### 3.2 Watertight tessellation via connectivity (Stoger/Kurka and related)

**Citation.** Representative: "Watertight tessellation of B-rep NURBS CAD-models using connectivity information" (Stoger & Kurka); and Hahmann et al. (2015), Watertight conversion of trimmed CAD surfaces to Clough-Tocher splines, *Computer Aided Geometric Design, 35-36*, 161-185.

**Content.** The connectivity approach exploits the B-rep *topology*: because the kernel knows which faces share which edges, it can tessellate each face's trim boundary and then force adjacent faces to use the *same* discretized edge polyline, so the triangulations meet exactly with no cracks. The Hahmann et al. work takes a more radical route: it *converts* the trimmed CAD surfaces into a single watertight piecewise-polynomial (Clough-Tocher macro-element) representation that is gap-free by construction, trading exactness of the original surface for guaranteed watertightness and smoothness.

**Limitations.** Connectivity-based stitching is only as good as the B-rep topology it trusts; if the imported model has bad topology (a common situation after data exchange), the stitching has nothing to lock onto. Conversion approaches change the surface representation and may not round-trip to the original CAD.

**Kernel implication.** Watertight tessellation is fundamentally a *topology* operation, not a *geometry* operation. This is a strong argument that Keel must maintain rigorous, trustworthy adjacency information (which edge bounds which faces, with consistent orientation) as a first-class invariant, because the tessellator, the mesher, and the analysis client all depend on shared edges being literally shared. The kernel should provide a "tessellate this B-rep watertight" service that consumes topology, not just per-face geometry.

### 3.3 GPU trimming and tessellation (Guthe, Balazs, Klein, 2005)

**Citation.** Guthe, M., Balazs, A., & Klein, R. (2005). GPU-based trimming and tessellation of NURBS and T-spline surfaces. *ACM Transactions on Graphics (SIGGRAPH), 24*(3), 1016-1023.

**Content.** This work pushed trimmed-NURBS display onto the GPU. Surfaces are approximated by rational bicubic Bezier patches and tessellated on the GPU to a prescribed screen-space error. Trimming is handled by a **trim texture**: the parameter-space trim region is rasterized into a texture whose resolution adapts to need, and a fragment that maps to a trimmed-away region is discarded by texture lookup. They reported speedups up to ~1000x over CPU techniques, enabling real-time visualization of real trimmed-NURBS and T-spline models on consumer hardware.

**Limitations.** Trim textures are a *raster* approximation of the trim boundary, so trim edges can show aliasing/staircasing unless the texture is very high resolution, and the approximation reintroduces the gap problem at face boundaries. Memory cost of trim textures scales with trim complexity.

**Kernel implication.** Even if Keel does not ship a renderer, this defines the *export contract* a kernel owes a GPU client: per-face rational Bezier patches (consistent with Bezier extraction from Section 1.3) plus a parameter-space description of the trim region. Providing Bezier-patch output and clean trim loops makes Keel directly consumable by modern GPU pipelines.

### 3.4 Direct GPU trimming via classification (Schollmeyer & Froehlich, 2009/2019)

**Citation.** Schollmeyer, A., & Froehlich, B. (2009). Direct trimming of NURBS surfaces on the GPU. *ACM Transactions on Graphics (SIGGRAPH), 28*(3), 1-9. And Schollmeyer, A., & Froehlich, B. (2019). Efficient and anti-aliased trimming for rendering large NURBS models. *IEEE Transactions on Visualization and Computer Graphics, 25*(3).

**Content.** Rather than rasterizing trim curves into a texture, this approach classifies points *directly* against the analytic trim curves. Trim curves are split into segments that are monotonic in each parameter direction; a point is classified inside/outside by an optimized bisection against these monotonic segments, with no explicit ray-curve intersection. The whole trimmed-NURBS ray-casting pipeline (surface evaluation and trimming) fits in a single GLSL shader and handles arbitrary-degree surfaces and trim curves using the *original CAD data without approximation*. The 2019 follow-up added anti-aliasing and scaled to models with hundreds of thousands of trim curves at interactive rates.

**Limitations.** More shader-complex than trim textures; performance depends on trim-curve segmentation quality. Still a rendering technique, so cross-face watertightness at intersection edges is a separate concern.

**Kernel implication.** The "split trim curves into monotonic segments and classify by bisection" idea is exactly the kind of robust point-in-trim primitive Keel needs *internally* (for point classification, Section 1.4, and Booleans), not just for rendering. Keel should expose trim curves in a form (or with utilities) that supports monotone-segment decomposition, because the same machinery serves trimming, rendering, and inside/outside queries.

### 3.5 Robust containment via generalized winding numbers (Spainhour & Weiss, 2025)

**Citation.** Spainhour, J., & Weiss, K. (2025). Robust containment queries over collections of trimmed NURBS surfaces via generalized winding numbers. *ACM Transactions on Graphics, 44*. (arXiv:2504.11435; implemented in the open-source Axom library, BSD-licensed, LLNL.) See also the related 2024 paper on rational parametric *curves* (TOG, 3658228) and the 2025 follow-on "Fast and robust point containment queries on trimmed surfaces" (arXiv:2510.25159).

**Content.** This is the state-of-the-art answer to the kernel's most basic question, "is this point inside the solid?", made *robust to watertightness defects*. The generalized winding number (GWN) measures how much solid angle a (possibly leaky) boundary subtends at a query point; it is well-defined even when the boundary has gaps, and it varies continuously, so classification does not catastrophically fail near cracks. The method computes the GWN for trimmed NURBS *without first discretizing the surfaces*: it uses the solid-angle structure and Stokes' theorem to reduce the surface integral to a 1D line integral over each trimmed patch boundary, evaluated by rapidly converging adaptive quadrature, with special handling (line-surface intersection, parametric-disk extraction) when the query point is near the surface where Stokes' theorem is invalid.

**Limitations.** Adaptive quadrature near the surface is the expensive case. The method classifies containment; it is not a full Boolean engine. It is recent, with implementation maturity still growing.

**Kernel implication.** This is directly load-bearing for Keel. Point-in-solid is the primitive under Booleans, mass properties, selection, and IGA domain queries. The GWN approach means Keel can offer *robust* containment **even on imperfect, leaky imported B-reps**, which is the realistic input condition (Section 4). That an open-source (BSD) implementation exists in Axom is a concrete reuse opportunity. Keel should treat GWN-style containment as the reference algorithm for point classification rather than relying on ray-casting parity counting, which is fragile at exactly the gaps that imported CAD always has.

### 3.6 Ray tracing trimmed NURBS (Sloup & Havran, 2021)

**Citation.** Sloup, J., & Havran, V. (2021). Optimizing ray tracing of trimmed NURBS surfaces on the GPU. *Computer Graphics Forum, 40*(7), 161-172.

**Content.** Ray tracing trimmed NURBS has historically been too slow for industry because each ray-surface hit requires a Newton iteration to find the parametric intersection and then a trimming classification. This paper accelerates the trimming-side point location with a 2D kd-tree over the trim curves that is faster, smaller, and cheaper to preprocess than prior structures, reaching tens to hundreds of millions of rays per second on the GPU for scenes with hundreds of thousands of surfaces and trim curves.

**Limitations.** Still requires per-ray Newton iteration on the surface (initialization and convergence robustness are perennial issues). GPU-memory layout of acceleration structures is intricate.

**Kernel implication.** If Keel ever supports exact (non-tessellated) visualization or exact ray-based queries (e.g. for measurement, selection, or rendering integration), it needs robust ray-surface intersection (Newton with good seeding) plus a fast trim acceleration structure. The reusable kernel lesson is to *cache* per-face acceleration structures (kd-tree over trim curves, bounding hierarchies over Bezier patches) so both rendering and geometric queries share them.

---

## 4. CAD Data Exchange

A Parasolid-class kernel lives or dies by interoperability. The data-exchange formats are where NURBS geometry crosses between kernels, and where tolerance and topology mismatches turn clean geometry into leaky garbage that downstream code (and Keel) must heal.

### 4.1 STEP AP203 / AP214 / AP242

**Citation.** ISO 10303 series: AP203 (configuration-controlled 3D design, ISO 10303-203, originally aerospace/defense), AP214 (core data for automotive mechanical design, ISO 10303-214), AP242 (managed model-based 3D engineering, ISO 10303-242, 2014 onward).

**Content.** STEP (ISO 10303) is the dominant neutral B-rep exchange format. It carries full topological B-rep (solids, sheet/open shells, wireframe) with explicit connectivity, where face geometry is either elementary analytic surfaces (plane, cylinder, cone, sphere, torus) or NURBS (B_SPLINE_SURFACE_WITH_KNOTS and rational variants), and edge geometry is analytic or NURBS curves. AP203 originated in aerospace; AP214 added automotive needs (colors, layers, kinematics, assembly). **AP242 unifies AP203 and AP214** and adds semantic PMI (product manufacturing information), tessellated geometry alongside exact B-rep, and richer assembly/composite support. STEP's explicit topology means a well-formed STEP solid can in principle be meshed or analyzed directly without geometry healing.

**Limitations.** "In principle" is doing heavy lifting: in practice STEP files frequently arrive with gaps, sliver faces, inconsistent normals, and tolerance mismatches because the *exporting* kernel's tolerances differ from the importer's. AP242's breadth means implementations vary in coverage; PMI semantics are unevenly supported.

**Kernel implication.** Keel must read and write STEP, and must store exactly what STEP stores: analytic surfaces *as analytic surfaces* (not silently converted to NURBS, which loses intent and inflates data), NURBS with explicit knots/weights, and full topology with orientation. A subtle but critical design choice: keep elementary surfaces as their own types so round-tripping through STEP is lossless. The healing burden (4.3) is unavoidable on import.

### 4.2 IGES entities 126 and 128

**Citation.** IGES (Initial Graphics Exchange Specification), ANSI/USPRO. Entity 126 = Rational B-Spline Curve; Entity 128 = Rational B-Spline Surface; Entity 144 = Trimmed (Parametric) Surface; Entity 142 = Curve on a Parametric Surface.

**Content.** IGES is the older neutral format and remains relevant precisely because it carries *untrimmed* NURBS surface patches cleanly. Entity 128 is a rational B-spline surface (knots, weights, control points, degree, with flags for closed/periodic/polynomial-vs-rational). Entity 126 is its curve analogue. Trimming is layered on top via entity 144 (trimmed parametric surface), which references a base 128 surface plus entity 142 boundary curves. IGES surfaces are widely produced when a clean NURBS hand-off is wanted without the topological machinery of STEP.

**Limitations.** IGES is essentially a *surface/wireframe* exchange; its solid/topology support is weak and inconsistent, so a multi-surface IGES model often imports as a disconnected "surface soup" with no reliable adjacency, which is the worst case for watertightness. It is largely legacy; new development favors STEP.

**Kernel implication.** Keel will encounter IGES "surface soup" and must be able to *sew* free surfaces into a topological B-rep using tolerance-based edge matching. Supporting IGES import means Keel needs robust sewing/stitching as a core operation, which in turn depends on the tolerance model (4.3). Entities 126/128/144/142 map naturally onto Keel's NURBS curve, NURBS surface, trimmed face, and trim curve abstractions, so a clean internal model makes IGES I/O mostly a serialization concern.

### 4.3 Tolerance, accuracy, and model healing

**Citation.** Representative literature: model-healing and "dirty geometry" repair surveys in CAD interoperability; commercial reality documented across STEP/IGES translation tooling. (See also Sederberg et al. 2008 watertight trimmed NURBS, Section 3.1, as a representation-level remedy.)

**Content.** The central interoperability pathology: every kernel has its own *modeling tolerance* (the distance below which two points are "the same"). When geometry moves between kernels with different tolerances, edges that were coincident become gaps, and faces that met now overlap or separate. Trimmed-surface intersection approximations (Section 3.1) compound this. Model **healing** is the repair pipeline: detect gaps and slivers, snap near-coincident vertices/edges, rebuild missing or inconsistent topology, fix face orientations, and re-establish watertightness within a target tolerance. Healing is a large fraction of real-world CAD/CAE engineering effort.

**Limitations.** Healing is heuristic and lossy: aggressive snapping can distort geometry, conservative snapping can leave gaps. There is no universal correct tolerance; the right value depends on model scale and downstream use.

**Kernel implication.** This is one of the most important design decisions for Keel. The kernel needs an explicit, *consistent* tolerance model attached to bodies (not hard-coded global constants), tolerant-edge/tolerant-vertex representation so that imperfect coincidences can be represented honestly rather than forced, and a healing toolkit (sew, snap, orient, gap-fill) as a first-class subsystem. The GWN containment approach (3.5) is valuable precisely because it tolerates the residual leakiness that healing cannot fully remove. Robustness reputation, the thing that distinguishes Parasolid from hobby kernels, is largely *this*: graceful behavior on dirty imported geometry.

---

## 5. Volumetric / Trivariate NURBS and V-reps

B-rep models the *boundary* of a solid. A growing class of applications (IGA on solids, additive manufacturing of graded/porous materials, lattice structures) needs the *interior* parameterized too. That is trivariate (volumetric) NURBS, and Elber's V-rep is the most developed framework.

### 5.1 V-reps (Massarwi & Elber, 2016)

**Citation.** Massarwi, F., & Elber, G. (2016). A B-spline based framework for volumetric object modeling. *Computer-Aided Design, 78* (SPM 2016), 36-47. See also Massarwi, Antolin, Elber (2019) on volumetric untrimming (arXiv:1903.08907), and Elber et al. (2023), A review of a B-spline based volumetric representation, *Computer-Aided Design, 163*, 103587.

**Content.** V-rep generalizes B-rep from boundary surfaces to *volumes*. A V-rep cell (V-cell) is a 3-manifold defined over one or more (possibly intersecting) trimmed B-spline tensor-product **trivariates**, with the cell sub-domain delineated by trimming surfaces (the volumetric analogue of trimming curves). The framework lifts the standard B-rep modeling toolkit, primitive constructors, surface/volume constructors, and Boolean operations, to trimmed trivariate V-reps. Because the interior is parameterized, V-rep gives a tight geometry-analysis link (the model *is* an IGA volume discretization) and natively supports additive manufacturing of porous, functionally-graded, heterogeneous, and anisotropic material distributions, including lattice and microstructure synthesis driven by the trivariate parameterization. The 2019 untrimming work decomposes trimmed trivariates back into tensor-product pieces (important because analysis prefers untrimmed tensor patches).

**Limitations.** Trivariate modeling is heavy: trivariate Booleans and intersections are harder and slower than their surface counterparts, and constructing well-parameterized trivariates from a boundary B-rep (the "volumetric parameterization" problem) is itself an open research area. Trimmed trivariates inherit and amplify the trimming robustness issues of surfaces.

**Kernel implication.** Whether Keel supports trivariates is a scoping decision with large consequences. If additive manufacturing, lattices, or solid IGA are target markets, the kernel's geometry layer should be designed so that the *same* spline machinery (evaluation, knot insertion, trimming, Bezier extraction) generalizes from bivariate surfaces to trivariate volumes, rather than treating surfaces as a special case with no upgrade path. LR B-splines' dimension-independence (2.4) is attractive here. Even if Keel ships surfaces first, leaving room for trivariates in the type system is cheap; retrofitting them is not.

---

## 6. Industrial Application Domains

Each domain stresses a different kernel capability. Reading them together reveals which kernel properties are genuinely cross-cutting (continuity, robust trimming, parameterization) versus domain-specific.

### 6.1 Ship hull design and fairing

**Citation.** Representative: Perez-Arribas, Suarez-Suarez, Fernandez-Jambrina (and related), NURBS-based parametric hull form work; Mancuso, "An optimization approach for fairing of ship hull forms"; FFD-based hull modification studies (e.g. *MDPI J. Marine Sci. Eng.* 10(5):686, 2022).

**Content.** Hull surfaces are represented as NURBS and *faired*: control points are adjusted to minimize a strain-energy / fairness functional so the hull is smooth and free of unwanted curvature wiggles that would worsen drag. Modern workflows are *parametric*: a small set of naval-architecture parameters (length, beam, draft, section shapes) drives NURBS control-point placement, and the resulting surface feeds hydrodynamic (CFD) optimization. **Free-form deformation (FFD)** is widely used to modify the hull during optimization: an FFD lattice envelopes the NURBS surface, and moving lattice points smoothly deforms the whole hull, giving low-dimensional shape control (often <50 variables) suitable for expensive CFD-in-the-loop optimization.

**Limitations.** Fairing functionals trade fidelity against smoothness; over-fairing erases intended features. FFD couples regions that the designer might want independent. Maintaining feature constraints (flat-of-side, transom) under deformation needs care.

**Kernel implication.** Hull design wants two kernel services: a **fairing/energy-minimization** operator over control points, and robust **free-form deformation** of NURBS bodies (deform-while-preserving-parameterization). Both argue that Keel should expose control nets for editing and provide higher-level shape operators (fair, smooth, FFD) rather than only low-level geometry. Curvature evaluation (for fairness diagnostics, "porcupine"/curvature-comb plots) should be a kernel query.

### 6.2 Automotive Class-A surfacing

**Citation.** Industry practice (Autodesk Alias, ICEM Surf documentation); academic overviews of Class-A surface modeling and curvature-continuity requirements.

**Content.** "Class-A" is the automotive standard for visible exterior surfaces (body panels). The defining requirement is *aesthetic curvature quality*: surfaces must meet at least **G2 (curvature) continuity**, and premium work demands **G3 (curvature-rate) continuity**, so that reflected highlight lines flow without kinks or jumps. The geometric criteria are evaluated with reflection-line and curvature-comb analysis, zebra striping, and Gaussian/mean curvature maps. Class-A modeling is painstaking, often using single-span low-degree Bezier-like patches with carefully matched cross-boundary derivatives, because adding control points to fix a defect can introduce subtle curvature ripples.

**Limitations.** G2/G3 continuity across trimmed boundaries is hard to guarantee and hard to preserve under editing; many CAD operations (offset, blend, fillet) degrade continuity. Tooling for *verifying* high-order continuity is as important as tooling for creating it.

**Kernel implication.** Class-A is the most demanding *continuity* customer. Keel must support and *measure* G1/G2/G3 continuity across edges, provide curvature and curvature-derivative evaluation, and offer continuity-preserving operations (matched blends, G2 fillets, order-elevation that maintains continuity). Reflection/zebra analysis depends on accurate normal and curvature fields, so the evaluator must be accurate to high order, not just positionally. This is also where Bezier-patch internal representation (1.3) pays off: cross-boundary derivative matching is naturally expressed on Bezier control points.

### 6.3 Aerospace: CST vs NURBS for wings

**Citation.** Kulfan, B. M. (2008). Universal parametric geometry representation method (CST). *Journal of Aircraft, 45*(1), 142-158 (and Kulfan & Bussoletti 2006). Comparative studies: Masters et al. (2017), "Geometric comparison of aerofoil shape parameterization methods," *AIAA Journal*; NASA piecewise-CST-of-wings reports.

**Content.** Aerospace shape optimization needs *low-dimensional, smooth* parameterizations of airfoils and wings. The **Class-Shape Transformation (CST)** of Kulfan represents an airfoil as a class function (encoding the round-nose, sharp-tail topology) times a shape function (a Bernstein-polynomial expansion), plus a trailing-edge term. CST reaches good airfoil accuracy with very few parameters (often 6-12 per side) because its class function already matches typical airfoil shape, which is attractive for optimization where every design variable multiplies cost. NURBS are the alternative: more parameters but more local control and direct CAD compatibility. Comparative studies find CST excellent for *families of conventional airfoils* with few DOF, while NURBS (and B-splines) give finer local control and integrate directly with the CAD/B-rep pipeline. CST extends to 3D wings by lofting parameterized sections along the span.

**Limitations.** CST is specialized to airfoil-like topology; it does not generalize to arbitrary CAD geometry, and high-order CST can be ill-conditioned. NURBS need more variables and care to keep the optimization space smooth.

**Kernel implication.** Aerospace tells Keel something subtle: the *design* parameterization and the *kernel* representation need not be the same. CST lives above the kernel as a domain parameterization that *emits* NURBS. So Keel should make it easy to *construct* NURBS curves/surfaces from external parameterizations (fit, interpolate, loft from sections) and to *extract* derivatives with respect to control points for optimization (connecting to differentiable CAD, Section 7). A robust loft/skinning operator over section curves is a concrete requirement.

### 6.4 Turbomachinery blade design

**Citation.** Representative: parametric blade-design tools using NURBS for camber and thickness (e.g. *Advances in Engineering Software* blade-design papers; ANSYS BladeGen/BladeModeler practice; DLR differentiated blade parameterization, *Computers & Fluids* 2025).

**Content.** Compressor and turbine blades are built from stacked 2D profile sections, each typically defined by a NURBS/B-spline **camber line** plus a **thickness distribution**, with additional B-spline control over leading-edge shape, sweep, and lean both chordwise and spanwise. The 2D sections are transformed and stacked into 3D blade surfaces and solids. The parameterization mixes engineering scalars (inlet/outlet metal angles, chord, stagger) with spline control points, and feeds CFD-plus-optimizer loops (genetic algorithms, adjoint methods) to minimize pressure loss and widen operating range. Compressor airfoils need fewer NURBS points than turbine blades because turbine suction sides are more highly curved.

**Limitations.** Leading-edge geometry is delicate (small radius, high curvature) and a frequent source of meshing/CFD trouble; blends between blade and hub/shroud (fillets) are robustness-critical. Watertight blade-plus-platform solids stress the trimming/Boolean machinery.

**Kernel implication.** Turbomachinery reinforces the loft-from-sections requirement (6.3) and adds emphasis on *robust filleting/blending* between surfaces of very different curvature, and on accurate leading-edge representation (high-curvature, possibly small-radius arcs as exact analytic geometry rather than coarse NURBS). It also wants *differentiable* geometry for adjoint optimization (Section 7).

### 6.5 Architectural geometry (Pottmann)

**Citation.** Pottmann, H., Asperl, A., Hofer, M., & Kilian, A. (2007). *Architectural Geometry*. Bentley Institute Press. Plus Liu, Pottmann, Wallner, et al. (2006), Geometric modeling with conical meshes and developable surfaces, *ACM TOG*; Pottmann et al. (2007), Geometry of multi-layer freeform structures, *ACM TOG*; Jiang et al. (2015) interactive freeform-structure modeling.

**Content.** Architectural geometry rationalizes freeform building skins into *buildable* pieces. The key constraint is *fabrication*: glass and panel facades want **planar quadrilateral (PQ) meshes** because flat panels are far cheaper than doubly-curved ones, and **conical meshes** (a PQ subclass) additionally admit consistent offset surfaces with torsion-free supporting beams, which matters for multi-layer structures. The field applies discrete differential geometry to approximate a designed NURBS freeform surface by a PQ/conical mesh that respects panel planarity, beam offsets, and statics, often via constrained optimization. Pottmann's program connects smooth NURBS design surfaces to discrete fabrication-aware meshes.

**Limitations.** PQ/conical remeshing is a constrained nonlinear optimization that can fail to converge or distort the intended shape; the smooth-to-discrete gap means the built structure approximates the design surface.

**Kernel implication.** Architecture mostly consumes Keel's *output* (a smooth NURBS surface) and then re-discretizes it under fabrication constraints, so the direct kernel requirement is high-quality surface evaluation, curvature fields (to drive panelization), and offset-surface generation (for multi-layer structures). Offsetting NURBS surfaces *robustly* (the offset of a NURBS is not a NURBS and must be approximated) is the concrete kernel ask, and it recurs across domains.

---

## 7. NURBS in Graphics and Machine Learning

ML on CAD is now a real consumer of kernel design, because the data structures Keel chooses determine whether learning systems can ingest its output, and differentiable-CAD pipelines want gradients *through* geometry operations.

### 7.1 Differentiable NURBS: NURBS-Diff (Prasad et al., 2021/2022)

**Citation.** Prasad, A. D., Balu, A., Shah, H., Sarkar, S., Hegde, C., & Krishnamurthy, A. (2022). NURBS-Diff: A differentiable programming module for NURBS. *Computer-Aided Design, 146*, 103199. (arXiv:2104.14547, 2021.)

**Content.** NURBS-Diff makes the NURBS map differentiable end-to-end. It derives the analytic derivatives of a NURBS curve/surface with respect to *all* its inputs (control points, weights, and knot vector) and implements forward and backward passes as a GPU-accelerated PyTorch module, so a NURBS evaluation can sit inside a neural network and receive gradients by backpropagation. Demonstrated uses: curve/surface *fitting* by gradient descent, surface *offsetting*, unsupervised point-cloud-to-NURBS *reconstruction*, and enforcing analysis constraints during learning.

**Limitations.** Differentiating through the knot vector is delicate (knots define a non-smooth partition); robustness of gradient-based fitting depends on initialization and parameterization, the same Newton-seeding issues as elsewhere. It is a layer, not a full kernel.

**Kernel implication.** Differentiable CAD is a forcing function on the kernel's *evaluation* code: if Keel's NURBS evaluator is written so that derivatives with respect to control points and weights are available (analytically or via clean factoring through Bezier/Bernstein form), it becomes usable in optimization and ML pipelines (shape optimization in 6.3/6.4, learning in 7.2). Designing the evaluator to expose parametric derivatives (and ideally control-point Jacobians) is a low-cost, high-leverage choice.

### 7.2 Learning on B-reps: UV-Net and BRepNet (2021)

**Citation.** Jayaraman, P. K., Sanghi, A., Lambourne, J. G., Willis, K. D. D., Davies, T., Shayani, H., & Morris, N. (2021). UV-Net: Learning from boundary representations. *CVPR 2021*. And Lambourne, J. G., et al. (2021). BRepNet: A topological message passing system for solid models. *CVPR 2021*.

**Content.** These tackle the problem that B-reps are awkward for ML: they mix *continuous, non-Euclidean* geometry (NURBS faces/edges) with *discrete* topology (the face-edge-vertex graph). **UV-Net** samples each face and edge on a regular grid in its U/V parameter domain to get structured 2D/1D "UV-grids" of points (and normals), processes those with image/1D CNNs for geometry, and processes the face-adjacency graph with a graph CNN for topology, coupling the two efficiently. **BRepNet** defines convolution *directly over the topological hierarchy*, using the ordered coedge/edge/face incidences as a message-passing structure, so the network's receptive field follows the B-rep's actual topology. Both enable classification, segmentation, and downstream generative tasks on solid models.

**Limitations.** UV-grid sampling discards exactness (it is a discretization) and struggles with highly non-uniform parameterizations and trimmed faces. BRepNet's hand-defined topological kernels are tied to a particular incidence convention. Both depend on *clean* B-reps, which (Section 4) imports rarely are.

**Kernel implication.** The relevant lesson for Keel is about *data-structure exposure*, not about implementing ML. Learning systems consume exactly two things: (1) sampled UV-grids of geometry with normals, which is a sampling/evaluation service Keel already needs for tessellation, and (2) a clean, ordered topological incidence graph (coedges, edges, faces with consistent orientation). Keel should make both first-class outputs: a robust UV-sampling query and a well-specified, ordered B-rep topology (a coedge/half-edge style structure). A kernel whose topology is messy or whose parameterizations are wildly non-uniform is hostile to this entire ecosystem.

### 7.3 Point cloud to CAD reconstruction (commercial pipelines)

**Citation.** Commercial practice (Geomagic Design X, Geomagic Wrap) and research benchmarks (Fit4CAD, arXiv:2105.06858; Point2CAD; clustering/approximate-implicitization reverse engineering, arXiv:1810.07451).

**Content.** Reverse engineering converts scanned point clouds / meshes into CAD. The commercial pipeline (Geomagic being the de facto standard) is: import millions of scan points, register and clean, build a watertight triangle mesh, **segment** the mesh into regions corresponding to CAD faces, then *fit* each region with the appropriate representation, analytic primitives (plane, cylinder, cone, sphere, torus) for prismatic shapes via either as-built or design-intent fitting, and **NURBS surface patches** for organic/freeform regions (Geomagic's "Auto Surface" lays down a network of NURBS patches automatically). The output is a parametric or NURBS B-rep ready for CAD/CAM/inspection.

**Limitations.** Segmentation is the hard part and is error-prone at feature boundaries; auto-fitted NURBS patch networks are often not watertight and need stitching; recovering true design intent (exact symmetry, round numbers, constraints) from noisy scans is only partially automatable.

**Kernel implication.** Reverse engineering is a major *producer* of geometry that Keel would ingest, and it produces exactly the dirty, near-watertight, patch-network B-reps that stress Section 4's healing and Section 3.5's robust containment. The kernel ask is twofold: excellent **surface fitting** (least-squares NURBS fitting to point sets, with knot placement, the inverse of evaluation) and excellent **sewing/healing** of the resulting patch networks. Both are core operations, and both connect back to the differentiable-fitting machinery of 7.1.

---

## 8. Open-Source Ecosystem Survey

Keel does not start from zero; it starts in a field of partial solutions with well-known strengths and reputations. This survey is about *what each implements* and *how robust it is reputed to be*, because reuse-versus-rewrite is a live decision.

### 8.1 OpenCASCADE (OCCT)

**Content.** The only mature, full open-source B-rep *kernel* (LGPL with linking exception). Implements analytic and NURBS surfaces/curves, full B-rep topology, Boolean operations, filleting/chamfering, offsets, and STEP/IGES translation. It is the kernel under FreeCAD, KiCad's 3D, and many others. Its Boolean operations and B-rep machinery are its defining capability; it is faster and far more battle-tested than naive NURBS libraries.

**Reputation.** Powerful but notoriously *heavy and quirky*: a large, idiosyncratic C++ API, a steep learning curve, historically uneven robustness on dirty Booleans, and a codebase many find hard to extend. It is the benchmark for *capability* and the cautionary tale for *API design*.

**Kernel implication.** OCCT defines the feature bar Keel must approach to be "Parasolid-class" (Booleans, fillets, STEP I/O, healing) and simultaneously demonstrates the *design failure mode* (opaque API, robustness gaps) Keel should avoid. Keel's differentiator is most plausibly *robustness on dirty geometry* and a clean, modern API, not raw feature count on day one.

### 8.2 SISL (SINTEF Spline Library)

**Content.** The most mature, feature-complete open-source *NURBS* library (GPL, from SINTEF). Rich curve/surface creation, evaluation, intersection, and approximation routines. Strong on the spline mathematics (the same group produced LR B-splines, Section 2.4).

**Reputation.** Solid, well-tested spline mathematics; a C API showing its age. It is a *geometry* library, not a topological B-rep kernel, so it lacks the solid-modeling layer (topology, Booleans).

**Kernel implication.** SISL is a reference for *what robust NURBS algorithms look like* (intersection, approximation) and a potential algorithmic source, but its GPL license and C API limit direct reuse depending on Keel's licensing goals. The SINTEF lineage (SISL plus LR B-splines) is the most coherent open spline ecosystem to learn from.

### 8.3 OpenNURBS

**Content.** McNeel's open-source (permissive) library defining the 3DM file format and the NURBS geometry classes used by Rhino. Full-featured NURBS *geometry* and a robust, widely-used interchange format.

**Reputation.** Well-engineered and clean, but, as practitioners stress, it is primarily a *geometry and I/O* library, *not* a solid-modeling kernel: it has NURBS objects and 3DM serialization but lacks Boolean/B-rep solid operations (Rhino's solid operations live above it).

**Kernel implication.** OpenNURBS is the model for *clean NURBS classes and a good interchange format* under a permissive license, exactly the layer Keel needs at its base. It is the most reusable-in-spirit reference for Keel's geometry tier, with the explicit understanding that the solid-modeling kernel is the part OpenNURBS does *not* provide and Keel *must*.

### 8.4 verb

**Content.** An open-source NURBS library (originally Haxe, with JavaScript output) focused on curve/surface evaluation, tessellation, and intersection, designed for cross-platform/web use.

**Reputation.** Useful and approachable for evaluation and adaptive tessellation; not a solid kernel and not aimed at industrial robustness.

**Kernel implication.** verb is a reference for *adaptive tessellation and a clean evaluation API*, especially for the rendering/web-facing side, rather than a robustness benchmark.

### 8.5 geomdl (NURBS-Python)

**Content.** A pure-Python NURBS library: curve/surface evaluation, knot operations, fitting, tessellation, visualization. Widely used in research and teaching.

**Reputation.** Convenient and readable, but, by repeated practitioner accounts, *slow* compared to native implementations and dependency-heavy; it is a research/prototyping tool, not a production kernel.

**Kernel implication.** geomdl is the readable specification of NURBS algorithms (good for cross-checking Keel's math) and a clear demonstration that *language and performance* matter for a kernel: pure Python is fine for prototyping and wrong for a production core. It validates a compiled-core design for Keel.

### 8.6 tinynurbs

**Content.** A small, header-only C++ library for NURBS curves and surfaces with a clean free-function API (curve*/surface* on Curve/RationalCurve/Surface/RationalSurface types).

**Reputation.** Minimal, clean, easy to read and embed; intentionally *not* a full kernel (no topology, no Booleans, limited robustness scope).

**Kernel implication.** tinynurbs is the model for *a clean, minimal C++ geometry core API*. Its scope is a useful lower bound: it shows the shape of the geometry tier without any of the solid-modeling, trimming, or healing machinery that constitutes the actual hard part of Keel.

### 8.7 Ayam, and the Axom reference

**Content.** **Ayam** is an open-source 3D modeling environment built around NURBS (curves, surfaces, and NURBS-based primitives) on top of a Tcl/Tk and RenderMan-oriented stack; it is a *modeler/application*, useful as a reference for interactive NURBS modeling UX rather than as a kernel library. **Axom** (LLNL, BSD) is not a NURBS modeler but is notable here because it now hosts the open-source generalized-winding-number containment implementation (Section 3.5), making it a concrete, permissively-licensed source for robust trimmed-NURBS containment.

**Kernel implication.** Ayam shows what an interactive NURBS modeling layer looks like above a geometry core. Axom is a directly reusable (BSD) reference for the single hardest robustness primitive Keel needs: point-in-trimmed-solid that survives leaky geometry.

### 8.8 Ecosystem synthesis

The landscape splits cleanly: **OpenCASCADE** is the only full open *kernel* (heavy, quirky, the capability bar); **SISL / OpenNURBS / verb / geomdl / tinynurbs** are *geometry libraries* of varying maturity and language, none providing the solid-modeling tier; **Ayam** is a modeler; **Axom** supplies a key robustness primitive. The gap Keel fills is precisely *a clean, modern, robust, permissively-licensed B-rep kernel*, the solid-modeling tier that almost nothing open does well, built on a geometry core that the existing libraries collectively show how to write.

---

## What Applications Demand from Keel

Pulling every "Kernel implication" together, the application and extension literature converges on a small set of non-negotiable demands. These should be read as design constraints, not feature wishes.

1. **Exact, refineable spline representation.** IGA (1.1-1.5) requires that knot vectors, control nets, and weights are first-class, queryable, and refineable *without changing the geometry*. Bake-to-mesh-only architectures are disqualified. Bezier extraction (1.3) should be a canonical internal/evaluation form.

2. **Trimming as a core, precise, robust subsystem.** Trimmed faces (underlying surface + parameter-space loops + 3D curves, kept consistent) are the central data structure (1.4, 3.1). Point-in-trimmed-face/solid classification must be *robust to leaky geometry*, for which generalized winding numbers (3.5, open in Axom) are the reference algorithm. Naive ray-parity classification is too fragile for real imported data.

3. **Trustworthy topology with consistent orientation.** Watertight tessellation (3.2), B-rep learning (7.2), and analysis (1.2) all depend on adjacency being literally shared and correctly oriented. A clean coedge/half-edge topology is load-bearing for the whole stack.

4. **An explicit, consistent tolerance model plus a healing toolkit.** Data exchange (4.1-4.3) guarantees dirty input. Tolerant edges/vertices, sewing/stitching, gap-fill, and orientation repair must be first-class, because *robustness on dirty geometry is the reputation that separates a real kernel from a toy.*

5. **High-order continuity creation and measurement.** Automotive Class-A (6.2), turbomachinery blends (6.4), and architecture offsets (6.5) demand G2/G3 continuity, curvature and curvature-derivative fields, continuity-preserving blends/fillets, and reflection-line-grade evaluation accuracy.

6. **Construction and fitting operators, not just storage.** Aerospace (6.3), turbomachinery (6.4), reverse engineering (7.3), and hull design (6.1) need loft/skin from sections, least-squares NURBS fitting with knot placement, fairing/energy minimization, free-form deformation, and robust offsetting. The kernel must *build* and *fit* geometry, not merely hold it.

7. **Differentiable, derivative-exposing evaluation.** Shape optimization (6.3/6.4) and ML/differentiable CAD (7.1, 7.2) want gradients of geometry with respect to control points and weights. An evaluator designed to expose parametric and control-point derivatives unlocks an entire optimization and learning ecosystem at low marginal cost.

8. **A locally-refineable, analysis-suitable basis path, chosen for guaranteed safety.** The extensions (Section 2) warn that local refinement can silently produce linearly dependent, non-partition-of-unity bases (2.3). Keel should favor a scheme whose safety is guaranteed *by construction* (THB-splines, 2.5) or enforce admissibility invariants in the refinement API (AST, LR). U-splines (2.6) are the patent-blocked aspirational ceiling; design for graceful degradation to plain NURBS on structured inputs.

9. **A generalization path to trivariates.** If additive manufacturing, lattices, or solid IGA are in scope (5.1), the spline machinery should generalize from surfaces to volumes rather than special-casing surfaces. LR B-splines' dimension-independence makes this cheaper. Leaving type-system room now avoids a painful retrofit.

10. **Clean, permissive, modern engineering as the differentiator.** The OSS survey (Section 8) shows the open field is either a heavy quirky kernel (OCCT) or geometry libraries without a solid tier. Keel's opportunity is the missing piece: a robust, permissively-licensed, well-architected B-rep *kernel*, where robustness on imperfect geometry and API cleanliness, not day-one feature count, are the competitive ground.

---

## References

1. Hughes, T. J. R., Cottrell, J. A., & Bazilevs, Y. (2005). Isogeometric analysis: CAD, finite elements, NURBS, exact geometry and mesh refinement. *Computer Methods in Applied Mechanics and Engineering, 194*(39-41), 4135-4195.
2. Cottrell, J. A., Hughes, T. J. R., & Bazilevs, Y. (2009). *Isogeometric Analysis: Toward Integration of CAD and FEA*. Wiley.
3. Borden, M. J., Scott, M. A., Evans, J. A., & Hughes, T. J. R. (2011). Isogeometric finite element data structures based on Bezier extraction of NURBS. *International Journal for Numerical Methods in Engineering, 87*(1-5), 15-47.
4. Scott, M. A., Borden, M. J., Verhoosel, C. V., Sederberg, T. W., & Hughes, T. J. R. (2011). Isogeometric finite element data structures based on Bezier extraction of T-splines. *International Journal for Numerical Methods in Engineering, 88*(2), 126-156.
5. Schillinger, D., & Ruess, M. (2015). The Finite Cell Method: A review in the context of higher-order structural analysis of CAD and image-based geometric models. *Archives of Computational Methods in Engineering, 22*(3), 391-455.
6. Sederberg, T. W., Zheng, J., Bakenov, A., & Nasri, A. (2003). T-splines and T-NURCCs. *ACM Transactions on Graphics (SIGGRAPH), 22*(3), 477-484.
7. Sederberg, T. W., Cardon, D. L., Finnigan, G. T., North, N. S., Zheng, J., & Lyche, T. (2004). T-spline simplification and local refinement. *ACM Transactions on Graphics (SIGGRAPH), 23*(3), 276-283.
8. Li, X., Zheng, J., Sederberg, T. W., Hughes, T. J. R., & Scott, M. A. (2012). On linear independence of T-spline blending functions. *Computer Aided Geometric Design, 29*(1), 63-76.
9. Scott, M. A., Li, X., Sederberg, T. W., & Hughes, T. J. R. (2012). Local refinement of analysis-suitable T-splines. *Computer Methods in Applied Mechanics and Engineering, 213-216*, 206-222.
10. Buffa, A., Cho, D., & Sangalli, G. (2010). Linear independence of the T-spline blending functions associated with some particular T-meshes. *Computer Methods in Applied Mechanics and Engineering, 199*(23-24), 1437-1445.
11. Dokken, T., Lyche, T., & Pettersen, K. F. (2013). Polynomial splines over locally refined box-partitions. *Computer Aided Geometric Design, 30*(3), 331-356.
12. Bressan, A. (2013). Some properties of LR-splines. *Computer Aided Geometric Design, 30*(8), 778-794.
13. Giannelli, C., Juttler, B., & Speleers, H. (2012). THB-splines: The truncated basis for hierarchical splines. *Computer Aided Geometric Design, 29*(7), 485-498.
14. Giannelli, C., Juttler, B., Kleiss, S. K., Mantzaflaris, A., Simeon, B., & Speh, J. (2016). THB-splines: An effective mathematical technology for adaptive refinement in geometric design and isogeometric analysis. *Computer Methods in Applied Mechanics and Engineering, 299*, 337-365.
15. Thomas, D. C., Engvall, L., Schmidt, S. K., Tew, K., & Scott, M. A. (2022). U-splines: Splines over unstructured meshes. *Computer Methods in Applied Mechanics and Engineering, 401*, 115515.
16. Hennig, P., Kastner, M., Morgenstern, P., & Peterseim, D. (2016). Adaptive mesh refinement strategies in isogeometric analysis: A computational comparison. arXiv:1605.00825.
17. Hahmann, S., Bonneau, G.-P., Yvart, A., et al. (2015). Watertight conversion of trimmed CAD surfaces to Clough-Tocher splines. *Computer Aided Geometric Design, 35-36*, 161-185.
18. Guthe, M., Balazs, A., & Klein, R. (2005). GPU-based trimming and tessellation of NURBS and T-spline surfaces. *ACM Transactions on Graphics (SIGGRAPH), 24*(3), 1016-1023.
19. Schollmeyer, A., & Froehlich, B. (2009). Direct trimming of NURBS surfaces on the GPU. *ACM Transactions on Graphics (SIGGRAPH), 28*(3), 1-9.
20. Schollmeyer, A., & Froehlich, B. (2019). Efficient and anti-aliased trimming for rendering large NURBS models. *IEEE Transactions on Visualization and Computer Graphics, 25*(3).
21. Spainhour, J., & Weiss, K. (2025). Robust containment queries over collections of trimmed NURBS surfaces via generalized winding numbers. *ACM Transactions on Graphics, 44*. (arXiv:2504.11435; Axom implementation, LLNL.)
22. Sloup, J., & Havran, V. (2021). Optimizing ray tracing of trimmed NURBS surfaces on the GPU. *Computer Graphics Forum, 40*(7), 161-172.
23. Sederberg, T. W., Finnigan, G. T., Li, X., Lin, H., & Ipson, H. (2008). Watertight trimmed NURBS. *ACM Transactions on Graphics (SIGGRAPH), 27*(3).
24. ISO 10303 (STEP): AP203 (ISO 10303-203), AP214 (ISO 10303-214), AP242 (ISO 10303-242, managed model-based 3D engineering).
25. IGES (Initial Graphics Exchange Specification): entities 126 (rational B-spline curve), 128 (rational B-spline surface), 142 (curve on parametric surface), 144 (trimmed parametric surface).
26. Massarwi, F., & Elber, G. (2016). A B-spline based framework for volumetric object modeling. *Computer-Aided Design, 78* (SPM 2016), 36-47.
27. Massarwi, F., Antolin, P., & Elber, G. (2019). Volumetric untrimming: Precise decomposition of trimmed trivariates into tensor products. arXiv:1903.08907.
28. Elber, G., et al. (2023). A review of a B-spline based volumetric representation: Design, analysis and fabrication of porous and/or heterogeneous geometries. *Computer-Aided Design, 163*, 103587.
29. Perez-Arribas, F., et al. (2022). NURBS-based parametric design for ship hull form. *Journal of Marine Science and Engineering, 10*(5), 686. (And Mancuso, A., An optimization approach for fairing of ship hull forms.)
30. Kulfan, B. M. (2008). Universal parametric geometry representation method (Class-Shape Transformation). *Journal of Aircraft, 45*(1), 142-158.
31. Masters, D. A., Taylor, N. J., Rendall, T. C. S., Allen, C. B., & Poole, D. J. (2017). Geometric comparison of aerofoil shape parameterization methods. *AIAA Journal, 55*(5).
32. Pottmann, H., Asperl, A., Hofer, M., & Kilian, A. (2007). *Architectural Geometry*. Bentley Institute Press.
33. Liu, Y., Pottmann, H., Wallner, J., Yang, Y.-L., & Wang, W. (2006). Geometric modeling with conical meshes and developable surfaces. *ACM Transactions on Graphics (SIGGRAPH), 25*(3).
34. Prasad, A. D., Balu, A., Shah, H., Sarkar, S., Hegde, C., & Krishnamurthy, A. (2022). NURBS-Diff: A differentiable programming module for NURBS. *Computer-Aided Design, 146*, 103199. (arXiv:2104.14547.)
35. Jayaraman, P. K., Sanghi, A., Lambourne, J. G., Willis, K. D. D., Davies, T., Shayani, H., & Morris, N. (2021). UV-Net: Learning from boundary representations. *CVPR 2021*.
36. Lambourne, J. G., Willis, K. D. D., Jayaraman, P. K., Sanghi, A., Meltzer, P., & Shayani, H. (2021). BRepNet: A topological message passing system for solid models. *CVPR 2021*.
37. Romanengo, C., et al. (2022). Fit4CAD: A point cloud benchmark for fitting simple geometric primitives in CAD objects. arXiv:2105.06858. (And Geomagic Design X / Wrap commercial reverse-engineering pipeline.)
38. SINTEF SISL; McNeel OpenNURBS; OpenCASCADE Technology (OCCT); verb; geomdl (NURBS-Python); tinynurbs; Ayam; LLNL Axom. (Open-source ecosystem references.)
