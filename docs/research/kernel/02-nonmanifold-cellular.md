# Non-Manifold and Cellular Topology for Solid Modeling

Research dossier supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition.

## Scope and motivation

Parasolid's "general body" model supports mixed-dimension, non-manifold bodies: wires, sheets, solids, and arbitrary combinations sharing topology in a single valid body. Retrofitting non-manifold support into a kernel originally architected for 2-manifold solids is famously expensive, because the core data structure, the Euler operator set, the Boolean engine, and every traversal API bake in manifold assumptions (every edge borders exactly two faces, every face has two sides, every vertex has a single fan). This dossier surveys the literature on non-manifold boundary representations, cellular topology, generalized maps, chain-complex algebra, the Euler operator theory that underpins them, and how the three reference kernels (Parasolid, ACIS, OCCT) expose or restrict non-manifoldness. It closes with a concrete day-one strategy recommendation for Keel.

The central design question: should Keel adopt a radial-edge-class non-manifold structure from day one, adopt the more compact partial-entity structure, or ship a manifold core with escape hatches (compounds, attributes) and pay the retrofit cost later? The literature is unusually consistent that the retrofit is the worst option, so the real choice is between the first two, plus the question of cellular region tracking.

---

## Theme 1: The radial edge structure and the founding work

### Weiler 1986 thesis and 1988 radial-edge paper

**Citation.** Weiler, K. J. (1986). *Topological structures for geometric modeling* (Doctoral dissertation). Rensselaer Polytechnic Institute, Troy, NY. And: Weiler, K. (1988). The radial edge structure: A topological representation for non-manifold geometric boundary modeling. In M. J. Wozny, H. W. McLaughlin, & J. L. Encarnacao (Eds.), *Geometric Modeling for CAD Applications* (pp. 3-36). Amsterdam: North-Holland.

**Content.** Weiler's thesis is the foundational document of non-manifold B-rep. The winged-edge structure (Baumgart) assumes every edge is shared by exactly two faces, which Boolean operations on solids can violate (an edge where four faces meet, a dangling edge, a face-internal wire, a vertex touching another solid). The radial edge structure (RES) resolves this by introducing a hierarchy of "uses": each edge owns an ordered radial cycle of *edge-uses*, one per face that meets along that edge. Each face has two *face-uses* (one per side). Each edge-use belongs to a *loop-use* of a face-use, and points to the next/previous edge-use radially around the edge and along the loop. *Vertex-uses* localize a vertex's participation in each incident edge-use. The entity tower is: model, region, shell, face, face-use, loop, loop-use, edge, edge-use, vertex, vertex-use. Regions are explicit 3D cells of space (solid or void), making RES inherently cellular. The radial ordering around an edge is the key innovation: it captures the cyclic angular order of all faces sharing an edge, supporting any number of faces (non-manifold) rather than exactly two.

**Limitations.** RES is storage-heavy. The proliferation of use-entities (two face-uses per face, one edge-use per face-edge incidence, vertex-uses per incidence) inflates memory well beyond a manifold half-edge representation, roughly proportional to the sum of incidences rather than the count of cells. Pointer-chasing for adjacency queries is the cost of generality. The 1988 companion paper "Boundary Graph Operators for Non-Manifold Geometric Modeling" (same volume, pp. 37-66) defines the manipulation operators, but the full operator algebra is intricate.

**Kernel relevance.** RES is the canonical answer and the de facto template that ACIS and Parasolid descend from conceptually. If Keel wants Parasolid-class generality, the use-based radial concept is the proven baseline. The thesis should be read in full before fixing Keel's entity tower, because the use-entity factoring is exactly what makes mixed-dimension and many-faces-per-edge work cleanly.

---

## Theme 2: Alternative and compact non-manifold representations

### Lee & Lee 2001 partial entity structure

**Citation.** Lee, S. H., & Lee, K. (2001). Partial entity structure: A compact boundary representation for non-manifold geometric modeling. *Journal of Computing and Information Science in Engineering, 1*(4), 356-365. Also: Lee, S. H., & Lee, K. (2001). Partial entity structure: A compact non-manifold boundary representation based on partial topological entities. *Proceedings of the Sixth ACM Symposium on Solid Modeling and Applications (SMA '01)*, 159-170.

**Content.** The partial entity structure (PES) is the most important compact alternative to RES. It extends the manifold half-edge structure rather than the winged-edge, and introduces *partial topological entities*: partial-vertex, partial-edge, and partial-face. Where RES eagerly materializes use-entities for every incidence, PES creates partial entities only where non-manifold conditions actually occur, and keeps an ordinary half-edge-like representation elsewhere. The headline claim, repeated across the literature, is that PES reduces storage to roughly **half that of the radial edge structure** while still deriving all topological adjacency relationships without loss of query efficiency. The entity hierarchy is hierarchical (region, shell, face, loop, edge, vertex) augmented by the three partial entities that splice in non-manifold fans and radial cycles on demand.

**Limitations.** Cleverness has a cost: the conditional presence of partial entities makes traversal code branch-heavy (is this edge manifold or not?), and the operator set is more subtle to implement correctly than RES's uniform-but-fat scheme. "Half the storage" is a representative figure, not a guarantee across all models; pathological non-manifold density narrows the gap.

**Kernel relevance.** PES is arguably the best-engineered point on the storage/generality curve and is highly relevant to a memory-conscious Rust kernel. If Keel chooses full non-manifold day one, PES is the leading candidate to implement instead of raw RES, because it gives the same expressive power at half the footprint, and its half-edge lineage maps cleanly onto a fast manifold path.

### Gursoz, Choi & Prinz NOODLES

**Citation.** Gursoz, E. L., Choi, Y., & Prinz, F. B. (1990). Vertex-based representation of non-manifold boundaries. In M. J. Wozny, J. U. Turner, & K. Preiss (Eds.), *Geometric Modeling for Product Engineering* (pp. 107-130). Amsterdam: North-Holland. Implemented as the NOODLES system at Carnegie Mellon.

**Content.** NOODLES is a vertex-based non-manifold representation. Rather than centering the structure on the edge (as RES does), it centers it on the vertex and the local neighborhood around each vertex. It models non-homogeneous entities of mixed dimension (vertices, linear edges, planar faces, volumetric regions) and supports non-regular Boolean operations between objects of different dimensions, which regularized solid Booleans forbid. The intersection algorithm proceeds dimension by dimension: vertex against vertex/edge/face, then edge, then face, building the merged non-manifold complex.

**Limitations.** Vertex-centric structures make edge-radial queries less direct than RES. NOODLES is a research system; its influence is conceptual (proving non-regular mixed-dimension Booleans) more than as a shipping data structure.

**Kernel relevance.** Important as proof that non-regularized Booleans across dimensions are tractable, which is exactly what CAE idealization and zone modeling need. The vertex-neighborhood viewpoint is a useful cross-check on any edge-centric design Keel adopts.

### Yamaguchi & Kimura coupling-entity model

**Citation.** Yamaguchi, Y., & Kimura, F. (1995). Nonmanifold topology based on coupling entities. *IEEE Computer Graphics and Applications, 15*(1), 42-50.

**Content.** This model represents cell decompositions of 3D Euclidean space with a data structure built around *coupling entities* that record neighborhood relationships and boundary information. It provides two classes of operations: ones that change the topology and ones that change the cell complex. The coupling concept abstracts how lower-dimensional cells "couple" into the neighborhoods of higher-dimensional ones, giving a uniform handle on non-manifold junctions.

**Limitations.** Less widely implemented than RES or PES; the coupling abstraction is elegant but adds a conceptual layer that few production kernels adopted directly.

**Kernel relevance.** A clean, citable formalization of non-manifold neighborhoods. Useful reading when defining Keel's invariants for what a valid non-manifold junction is.

### Q-Complex (inclusion topology, modern compaction)

**Citation.** Lee, Y.-J., et al. (2012). Q-Complex: Efficient non-manifold boundary representation with inclusion topology. *Computer-Aided Design* (Tsinghua CG group). Retrieved from https://cg.cs.tsinghua.edu.cn/papers/lyj-CAD12.pdf

**Content.** Q-Complex is a later non-manifold B-rep that adds *inclusion topology*, explicitly recording when one shell/region is contained inside another (e.g., a void cavity inside a solid, or a sheet floating inside a region). It positions itself against RES, PES, Yamaguchi-Kimura coupling entities, and NOODLES, surveying their entity sets and storage. Its contribution is making containment/nesting a first-class queryable relation rather than something recomputed.

**Limitations.** The fetched PDF was binary-corrupt to the tool, so detailed tables were not extracted; treat specific numbers as needing confirmation from the primary PDF. Inclusion topology adds bookkeeping that must be maintained through every Boolean.

**Kernel relevance.** Inclusion/containment queries (which region am I inside, what nests in what) are exactly the kind of thing CAE multi-zone modeling asks constantly. Worth studying for Keel's region-classification layer even if the full Q-Complex is not adopted.

---

## Theme 3: Selective Geometric Complexes and dimension-independent / cellular models

### Rossignac & O'Connor Selective Geometric Complexes

**Citation.** Rossignac, J. R., & O'Connor, M. A. (1990). SGC: A dimension-independent model for pointsets with internal structures and incomplete boundaries. In M. J. Wozny, J. U. Turner, & K. Preiss (Eds.), *Geometric Modeling for Product Engineering* (pp. 145-180). Amsterdam: North-Holland.

**Content.** The Selective Geometric Complex (SGC) is a dimension-independent cellular model. An object is a finite collection of mutually disjoint cells of any dimension (0 to n), connected by an incidence graph that records which cells bound which. The defining innovation is per-cell *activeness*: each cell carries a flag stating whether it is "active" (contributes to the represented pointset) or "inactive" (present structurally but not part of the set). This single mechanism lets one structure represent open sets, closed sets, sets with dangling lower-dimensional features, sets with internal partitions, and incomplete boundaries, all uniformly. Cells need not be manifold, need not tile a manifold, and may be of mixed dimension in one complex. Subclasses (faces, polygons, simplicial complexes, triangle meshes) drop out by restriction.

**Limitations.** Generality at the cost of compactness and of geometric specificity: SGC is a topological/combinatorial scaffold; the geometry on each cell is carried separately. The incidence graph can be large, and maintaining activeness consistency through operations requires discipline. Not optimized as a production B-rep; more a unifying framework.

**Kernel relevance.** The active/inactive idea is a powerful unifier and directly informs how Keel might represent partial or in-process models (manufacturing intermediates with incomplete boundaries) and selective Booleans. Even if Keel's runtime structure is RES/PES-like, SGC is the right mental model for what mixed-dimension generality means.

### Rossignac & O'Connor incidence-graph / dimension-and-order

**Citation.** Rossignac, J., & O'Connor, M. (1989/1991). Representing geometric structures in d dimensions: Topology and order. *Discrete & Computational Geometry*. (Companion theory to SGC.)

**Content.** Formalizes how to encode cell complexes in arbitrary dimension via incidence plus an ordering relation that disambiguates the cyclic/radial arrangement of cells around shared boundaries, the higher-dimensional analogue of RES's radial edge ordering. This is the theoretical bridge between "incidence graph" (who bounds whom) and "ordered B-rep" (in what rotational order).

**Limitations.** Abstract; implementation guidance is light.

**Kernel relevance.** Establishes that radial ordering generalizes to all dimensions, reassuring for any forward-compatible cellular design.

### Linear Algebraic Representation (LAR) and chain complexes

**Citation.** DiCarlo, A., Paoluzzi, A., & Shapiro, V. (2014). Linear algebraic representation for topological structures. *Computer-Aided Design, 46*, 269-274. And: DiCarlo, A., Milicchio, F., Paoluzzi, A., & Shapiro, V. (2009). Chain-based representations for solid and physical modeling. *IEEE Transactions on Automation Science and Engineering*. And: Paoluzzi, A., et al. (2017). Geometric computing with chain complexes: Design and features of a Julia package. arXiv:1710.07819. Implemented as LinearAlgebraicRepresentation.jl.

**Content.** LAR models a cellular complex as a (co)chain complex: a sequence of vector spaces of chains (formal sums of cells, over the field GF(2) for mod-2) connected by boundary operators. Every topological relation, boundary, coboundary, incidence, adjacency, becomes a sparse matrix; topological computation becomes sparse matrix algebra (chiefly sparse matrix-vector and matrix-matrix products). A 3D complex needs only two binary sparse matrices for topology plus a coordinate array for geometry. The boundary operator d satisfies d-of-d equals zero, which is the algebraic statement of "the boundary of a boundary is empty," and this single identity validates the complex. Booleans and arrangements reduce to chain operations.

**Limitations.** Elegant for batch/arrangement computation but less natural for incremental local edits (the daily work of a CAD kernel), where pointer-based B-reps update in place and matrices want rebuilding. GF(2) loses orientation unless extended. Numerical/robustness of the geometric arrangement step remains the hard part, same as everyone's.

**Kernel relevance.** LAR is the most mathematically clean dimension-independent model and an excellent way to *validate* a complex (check d-of-d equals zero) and to *implement Booleans/arrangements* as a batch backend. A pragmatic hybrid: keep a pointer-based editable B-rep as the live model, drop to a chain-complex/LAR view for arrangement and validation. Worth prototyping in Rust with a sparse-matrix crate.

### Dimension-independent simplicial structures (IA, IS, Stellar)

**Citation.** De Floriani, L., Hui, A., Panozzo, D., & Canino, D. (2010). A dimension-independent data structure for simplicial complexes. *Proceedings of the 19th International Meshing Roundtable*, 403-420. (Incidence-Simplicial; related: the IA* structure, and Canino's work on non-manifold simplicial d-complexes.)

**Content.** A line of compact, dimension-independent data structures for non-manifold *simplicial* complexes, storing top simplices and partial incidence/adjacency, reconstructing the rest on demand. Designed to scale to large non-manifold meshes in arbitrary dimension.

**Limitations.** Simplicial, not general-cell; a CAD kernel's faces are trimmed NURBS, not triangles, so these inform meshing/analysis layers more than the B-rep core.

**Kernel relevance.** Directly relevant to Keel's eventual meshing/CAE-handoff layer where the model is tessellated and may be non-manifold (midsurfaces in solids). Good reference for that downstream structure.

---

## Theme 4: Generalized maps and combinatorial maps

### Lienhardt generalized maps (G-maps)

**Citation.** Lienhardt, P. (1994). N-dimensional generalized combinatorial maps and cellular quasi-manifolds. *International Journal of Computational Geometry & Applications, 4*(3), 275-324. (Foundational earlier: Lienhardt, P. (1991). Topological models for boundary representation: A comparison with n-dimensional generalized maps. *Computer-Aided Design, 23*(1), 59-82.)

**Content.** Generalized maps (G-maps) are a combinatorial model of the topology of subdivided spaces. The atom is the *dart*; an n-G-map carries n+1 involutions (alpha-0 through alpha-n) that link darts. Cells are not stored explicitly: an i-cell is recovered as an orbit of darts under the involutions excluding alpha-i. G-maps represent orientable *and* non-orientable quasi-manifolds, with or without boundary, in any dimension, with a uniform algebra. The 1991 CAD paper explicitly compares G-maps against the established B-rep structures (winged-edge, radial-edge, etc.), arguing the combinatorial maps are more rigorous and uniform across dimension. Combinatorial maps (the orientable variant) use a single permutation per dimension and are equivalent, in 2D, to the half-edge structure.

**Limitations.** G-maps natively represent *quasi-manifolds* (cellular complexes that are "almost" manifold). True non-manifold configurations (an edge shared by three faces, mixed-dimension dangling features) are not directly a single G-map; they require either a collection of maps, "chunks," or extensions (multi-incidence, opening/sewing along orbits). Dart count is high (a 3-cell quad needs many darts), so raw memory is large, comparable to RES in spirit. The orbit-traversal mental model is unfamiliar to most CAD engineers.

**Kernel relevance.** G-maps are the mathematically cleanest topology engine and scale to any dimension, but the quasi-manifold restriction is exactly the wrong fit for a kernel whose headline feature is arbitrary non-manifoldness in 3D. They are most attractive if Keel wanted provable correctness and dimension independence above all; they are less attractive when many-faces-per-edge and mixed dimension must be the common, cheap case. Strong as inspiration for invariant design and validation, weaker as the literal day-one runtime structure.

### CGAL combinatorial maps and Linear Cell Complex

**Citation.** Damiand, G., & Teillaud, M. (2014). A generic implementation of dD combinatorial maps in CGAL. *Procedia Engineering, 82*, 46-58 (23rd International Meshing Roundtable). CGAL packages: *Combinatorial Maps*, *Generalized Maps*, *Linear Cell Complex*.

**Content.** CGAL ships the only widely available generic, arbitrary-dimension implementation of combinatorial maps and G-maps in C++. Darts are stored in a compact container; cells are attributed via the Linear Cell Complex, which attaches a geometric point to each vertex orbit. The design is fully generic over dimension via C++ templates, with optional per-cell attributes and direct access to all incidence/adjacency in optimal time.

**Limitations.** Combinatorial-map packages inherit the quasi-manifold limitation; representing genuinely non-manifold CAD bodies is not the package's sweet spot. C++ template-heaviness is a porting consideration, not a blocker for a Rust reimplementation of the ideas.

**Kernel relevance.** A reference implementation to study for how dart-and-attribute storage is laid out compactly. Confirms combinatorial maps are production-grade for mesh-like quasi-manifold data; reinforces that they are not the natural choice for arbitrary non-manifold solids.

### CGoGN

**Citation.** Kraemer, P., Untereiner, L., Jund, T., Thery, S., & Cazier, D. (2014). CGoGN: N-dimensional meshes with combinatorial maps. In *Proceedings of the 22nd International Meshing Roundtable* (pp. 485-503). Springer.

**Content.** CGoGN is an open-source C++ combinatorial-map library tuned for performance: an index-based (not pointer-based) dart implementation, cell markers, and quick-traversal accelerators for common incidence/adjacency queries. Benchmarks place it close to the fastest mesh libraries, with quick traversals pushing it ahead on specific queries, while keeping the single-model dimension-independent API.

**Limitations.** Same quasi-manifold scope as combinatorial maps generally. Optimized for meshes and multiresolution, not for trimmed-NURBS B-rep with arbitrary non-manifold junctions.

**Kernel relevance.** The index-based dart storage and quick-traversal design are excellent engineering precedents for a Rust kernel that wants cache-friendly, arena-allocated topology (indices/handles over raw pointers fits Rust's ownership model beautifully). Borrow the storage strategy even if not the combinatorial-map model.

---

## Theme 5: Non-manifold Euler operators and the extended Euler-Poincare formula

### Masuda complex-based non-manifold operators

**Citation.** Masuda, H. (1993). Topological operators and Boolean operations for complex-based non-manifold geometric models. *Computer-Aided Design, 25*(2), 119-129.

**Content.** Masuda generalizes Euler operators to non-manifold models built on a *complex-based* topology (cells of mixed dimension). He defines a set of low-level topological operators that always preserve a valid non-manifold complex, and shows Boolean operations can be expressed in terms of them. The work ties the operator set to the generalized Euler-Poincare relation for non-manifold complexes.

**Limitations.** The non-manifold operator set is larger and less canonical than Mantyla's manifold five; there is no single agreed minimal complete set across the literature.

**Kernel relevance.** This is the operator-theory backbone for a non-manifold kernel. Keel's internal mutation primitives should be a Masuda-style set proven to preserve complex validity, with Booleans implemented on top, mirroring how Parasolid/ACIS layer Booleans over low-level make/kill operators.

### Manifold Euler operators (Mantyla) and the extended Euler-Poincare theorem

**Citation.** Mantyla, M. (1988). *An Introduction to Solid Modeling*. Computer Science Press. (Completeness of Euler operators: Mantyla, 1984.) And the extended theorem context: the non-manifold generalization of V - E + F = 2(s - h) + r and the algebraic-topology Euler-Poincare relation chi = sum of (-1)^i b_i.

**Content.** For 2-manifold solids the Euler-Poincare formula is V - E + F - H = 2(S - G), where H counts inner loops (holes in faces), S shells, G genus. Mantyla proved the manifold Euler operators (Make/Kill Vertex-Face, Edge-Vertex, Edge-Face, etc.) form a *complete* set: any valid 2-manifold polyhedron is reachable by a finite sequence from a seed. For non-manifold complexes the simple formula no longer holds; the correct invariant is the topological Euler characteristic as the alternating sum of Betti numbers (the extended Euler-Poincare relation), and the operator set must be enlarged to cover non-manifold incidences.

**Limitations.** The manifold formula's elegance does not survive into the non-manifold world; validity checking becomes per-complex (d-of-d = 0, as LAR frames it) rather than a single scalar identity.

**Kernel relevance.** Tells Keel exactly what it gives up by going non-manifold: the tidy single-equation invariant. Validity must instead be enforced by operator design plus structural checks. This is a strong argument for adopting a proven operator algebra (Masuda) rather than inventing one.

### Lee & Lee generalized Euler operators for PES

**Citation.** Lee, S. H., & Lee, K. (in the PES line of work, and related Korean CDE literature, e.g., "Compact boundary representation and generalized Euler operators for non-manifold geometric modeling"). Korean Journal of CAD/CAM / SMA 2001.

**Content.** Defines a generalized Euler/Make-Kill operator set specifically for the partial entity structure, so that PES non-manifold models are built and edited only through validity-preserving primitives, the PES analogue of Mantyla's manifold set.

**Limitations.** Tied to PES specifics; not a universal operator standard.

**Kernel relevance.** If Keel adopts PES, this is the matching operator layer to implement; it closes the loop from data structure to safe mutation API.

---

## Theme 6: Cellular topology and mixed-dimension in commercial kernels

### ACIS Cellular Topology component

**Citation.** Spatial Corporation (Dassault Systemes). *ACIS Cellular Topology Component* (CT) reference documentation. And: Bidarra, R., et al. work on cellular topology in feature modeling. Retrieved from ACIS R10 CT documentation.

**Content.** ACIS's base topology is the entity tower body, lump, shell, face, loop, coedge, edge, vertex, supporting both manifold and non-manifold configurations. The *Cellular Topology* component sits on top as a secondary representation attached via attributes. It partitions a model of mixed dimensionality into the solid and sheet *cells* it is composed of. Classes include CELL (with subclasses CELL2D for sheet cells and CELL3D for solid/volume cells), CSHELL, CFACE (a face-use, a face as seen from one cell), and ATTRIB_CELL. A single lump can contain many solid and sheet regions; CT computes, maintains, and edits which cells exist and which face-uses (CFACEs) bound them. This makes ACIS able to answer "which volumetric regions does this model partition space into" and to track them through Booleans, the basis for multi-volume/zone modeling.

**Limitations.** CT is a *secondary*, attribute-attached layer, not the primary topology, meaning it is computed and maintained alongside rather than being the native model; consistency between base topology and CT must be managed. This is itself evidence that bolting cellular structure onto a non-cellular core is doable but adds machinery.

**Kernel relevance.** Strong precedent: ACIS proves a use-based non-manifold core (CFACE = face-use, mirrors RES) plus an explicit cell/region layer delivers commercial mixed-dimension modeling. The fact that CT is a layered add-on is a cautionary tale: Keel should consider making regions/cells primary from day one rather than a later attribute layer.

### Parasolid general bodies and regions

**Citation.** Siemens Digital Industries Software. *Parasolid Functional Description*, chapters on Model Structure, Body Types, and General Bodies; *Parasolid XT Format Reference*. Retrieved from q-solid.com Parasolid documentation mirror.

**Content.** Parasolid's topology tower is body, region, shell, face, loop, fin (its name for a coedge/edge-use), edge, vertex. Crucially, **regions are first-class**: a region is an open connected subset of 3D space bounded by vertices, edges, and oriented faces; regions are solid or void; every body has an infinite void region, and the union of all regions is all of 3D space. Body types form a lattice by what topology they contain: minimum/acorn (a single vertex), wire (connected edges), sheet (connected faces), solid (one or more solid regions), and **general** bodies. A general body may contain "any combination of non-manifold, cellular, or mixed-dimension parts in a single, valid body," subsuming all the others. The fin (edge-use) and two-sided face structure give the radial/use machinery non-manifoldness needs.

**Limitations.** General-body generality is opt-in; many Parasolid algorithms historically expect the manifold body classes, and not every operation is defined on arbitrary general bodies. The XT data model's richness is a high bar to match.

**Kernel relevance.** This is Keel's north star. The lesson is decisive: Parasolid made *regions first-class and space-partitioning from the ground up*, and treats manifold bodies as restricted cases of the general body. That is the architecture to emulate, regions as primary, manifold as a constraint, not regions as an afterthought.

### OCCT (Open CASCADE) non-manifold support and compounds

**Citation.** Open CASCADE Technology documentation: *Modeling Data*, *Boolean Operations*; TopoDS class references (TopoDS_Shape, TopoDS_TShape, TopoDS_Compound). Retrieved from dev.opencascade.org.

**Content.** OCCT's topology is TopoDS: vertex, edge, wire, face, shell, solid, compsolid, compound. TopoDS_TShape holds the actual topology (shared by reference); TopoDS_Shape is a lightweight handle adding orientation and location (shared by value). OCCT advertises a built-in non-manifold/mixed-dimensional capability: a single object can mix bodies with embedded curves and surfaces connected or not to an outer boundary. However, the practical pathway for heterogeneous results is the *compound*, a loose grouping of shapes. The Boolean engine restricts inputs: a compound argument should be homogeneous (only edges/wires, or only faces/shells, or only solids), contained solids must not touch, and the FUSE of a SHELL and a SOLID is disallowed because the result would be non-manifold. Booleans on open solids "cannot always be guaranteed" precisely because classification of a face can depend on the chosen sample point, a non-manifold ambiguity.

**Limitations.** OCCT's non-manifoldness is real but second-class: compounds are bags, not validity-checked non-manifold complexes, and the Boolean engine declines the genuinely non-manifold cases. There is no first-class space-partitioning region concept comparable to Parasolid's. This is the "manifold-with-escape-hatches" approach, and its limits (restricted Booleans, classification ambiguity, no region cells) are exactly the costs the synthesis below warns about.

**Kernel relevance.** OCCT is the most accessible open-source kernel and the clearest cautionary example. Keel's stated ambition (Parasolid-class general bodies) is precisely the thing OCCT does *not* fully deliver. Studying where OCCT's Boolean engine refuses non-manifold results is a checklist of capabilities Keel must do natively.

---

## Theme 7: Applications driving the non-manifold requirement

### CAE idealization: midsurfaces embedded in solids

**Citation.** Robinson, T. T., Armstrong, C. G., et al. (multiple), e.g., midsurface/dimensional-reduction and mixed-dimensional coupling literature; Abaqus/CAE midsurface modeling documentation; Sheen, D.-P., et al. (graph-based midsurface extraction for FEA). And: a mesh-geometry-based solution to mixed-dimensional coupling (*Computer-Aided Design*, 2010).

**Content.** For analysis, thin regions of a solid are idealized to midsurface shells while thick regions stay solid, producing a *mixed-dimensional* model where 2D shell faces connect to 3D solid faces along shared edges, an inherently non-manifold configuration. Tools extract a non-manifold model for CAE directly from the solid master model; where a part cannot be fully reduced, the result is a non-manifold mix of solids and midsurfaces. Abaqus/CAE auto-generates shell-to-solid coupling constraints at these junctions. The motivation is dramatic: mixed-dimensional analysis slashes element count, meshing time, and solve time versus all-solid meshes.

**Limitations.** Coupling shells to solids introduces DOF-compatibility and mesh-discontinuity problems at junctions, an analysis concern, but the *geometry* must support the non-manifold junction in the first place.

**Kernel relevance.** This is the single most concrete commercial reason a kernel must be non-manifold: serious CAE/CAD-integration customers expect midsurface-in-solid idealization, and that needs sheet faces sharing edges with solid faces in one body. A manifold-only Keel cannot host this model at all.

### Zone / multi-volume modeling (fluid + solid regions)

**Citation.** Multi-region / zone modeling literature, e.g., Boolean operations on multi-region solids for mesh generation (*Engineering with Computers*, 2012); CFD/CHT pre-processing practice.

**Content.** CFD and conjugate-heat-transfer setups need space partitioned into adjacent, non-overlapping volumetric zones (fluid region, solid region, multiple material regions) that *share* internal interface faces, with each interface face bounding two regions. This is exactly Parasolid's "regions partition all of space, each face is shared by the regions on its two sides." Imprinting and non-manifold Booleans create and maintain these shared interfaces.

**Limitations.** Maintaining watertight shared interfaces through edits is demanding; tiny gaps break the zone partition.

**Kernel relevance.** Reinforces that *regions as first-class space-partitioning cells* (Parasolid/ACIS-CT style) is not optional for a CAE-adjacent kernel. Keel should track regions and shared interface faces natively.

### Manufacturing in-process models and incomplete boundaries

**Citation.** In-process / intermediate-state modeling in machining and additive; conceptually aligned with SGC's incomplete-boundary representation (Rossignac & O'Connor, 1990).

**Content.** Manufacturing process models represent partially-formed parts: stock with cuts in progress, additive layers, intermediate states that may have dangling faces, internal partitions, or boundaries that are not yet closed solids. SGC's active/inactive cells and incomplete-boundary support are the theoretical fit; non-manifold mixed-dimension representation lets one model carry tool paths (wires), machined faces (sheets), and remaining stock (solid) together.

**Limitations.** Niche relative to mainstream MCAD, but growing with digital-manufacturing pipelines.

**Kernel relevance.** Another vote for non-manifold + cellular + selective (active/inactive) capability, and for SGC-style thinking in Keel's data model even if the runtime structure is RES/PES-based.

---

## Non-manifold strategy for Keel

**Verdict: adopt a first-class non-manifold, cellular core on day one. Make regions primary and space-partitioning. Do not ship manifold-with-escape-hatches.**

The literature and the kernel comparison point one direction with unusual consistency.

**1. The retrofit is the worst option, and OCCT proves it.** OCCT chose a manifold-leaning core with compounds as the escape hatch, and the consequences are visible in its own docs: the Boolean engine refuses genuinely non-manifold results (no SHELL-SOLID fuse), demands homogeneous compounds, cannot guarantee Booleans on open solids, and offers no first-class space-partitioning region. Every one of those is a capability Keel's Parasolid-class ambition requires. Bolting non-manifoldness on later means rewriting the data structure, the Euler/Make-Kill operator set, and the Boolean classifier, the three hardest, most-tested parts of a kernel. Mantyla's tidy V-E+F invariant does not survive the transition, so even validity checking gets rewritten. Pay this cost on day one when the operator set and Boolean engine are still small.

**2. Make regions first-class, like Parasolid, not a secondary layer like ACIS CT.** Parasolid's decisive design choice is that regions partition all of 3D space, are solid or void, and are present from the simplest body up; manifold bodies are restricted cases of the general body. ACIS instead computes cellular topology as an attribute-attached secondary representation, which works but adds a consistency-maintenance burden between base topology and the cell layer. Keel should make region cells and shared two-sided faces the native model, so CAE zone modeling, multi-volume, and midsurface-in-solid idealization are first-class rather than reconstructed.

**3. Choose the data structure: PES-class, not raw RES, with G-maps/LAR as influences not the runtime.** Three families were surveyed:
   - *Radial-edge (Weiler):* the proven baseline, use-based, edge-radial, regions explicit. Correct but storage-heavy.
   - *Partial entity structure (Lee & Lee):* same expressive power, roughly half the storage, half-edge lineage that gives a fast manifold path and a non-manifold path that only materializes partial entities where needed. Best storage/generality trade-off in the literature.
   - *Generalized/combinatorial maps (Lienhardt, CGAL, CGoGN):* mathematically cleanest and dimension-independent, but natively quasi-manifold; arbitrary 3D non-manifoldness (many faces per edge, mixed-dimension dangling features) is awkward, exactly Keel's common case. Wrong fit as the runtime, valuable as inspiration.

   **Recommendation:** implement a PES-class non-manifold B-rep as Keel's runtime topology, with explicit first-class regions (the one place to go beyond stock PES). Borrow CGoGN's *index-based / handle-based, arena-allocated dart-style storage* (indices not raw pointers), which suits Rust's ownership and gives cache-friendly, ABA-safe topology. This gets RES generality at PES memory cost with a Rust-idiomatic layout.

**4. Operators: adopt a proven validity-preserving set, do not invent one.** Build mutation on a Masuda-style complex-based non-manifold Make/Kill operator set (and the Lee & Lee generalized Euler operators matched to PES), so every internal edit preserves a valid non-manifold complex. Implement Booleans, imprint, and idealization on top of these primitives, mirroring Parasolid/ACIS layering. Because the single-scalar Euler invariant is gone, add structural validity checks; the LAR "boundary-of-boundary equals zero" (d-of-d = 0) identity is a cheap, rigorous validation oracle worth implementing even if LAR is not the runtime.

**5. Keep two batch escape valves from the dimension-independent camp.** (a) An SGC-style *active/inactive* cell concept for partial and in-process models with incomplete boundaries. (b) A *chain-complex/LAR* view, generated on demand from the live B-rep, as the backend for arrangements, Booleans, and validation, with a sparse-matrix Rust crate. This pairs an editable pointer/handle B-rep (good for incremental CAD edits) with an algebraic batch engine (good for robust arrangement and verification), the best of both literatures.

**In one line:** Keel should ship a partial-entity-class non-manifold B-rep with first-class space-partitioning regions, Masuda/Lee validity-preserving operators, CGoGN-style index-based storage, and an SGC/LAR batch layer for selective and algebraic operations, all from day one, because the survey shows every kernel that deferred non-manifoldness paid more later and still delivered less.

---

## References

1. Weiler, K. J. (1986). *Topological structures for geometric modeling* (Doctoral dissertation). Rensselaer Polytechnic Institute, Troy, NY.
2. Weiler, K. (1988). The radial edge structure: A topological representation for non-manifold geometric boundary modeling. In M. J. Wozny, H. W. McLaughlin, & J. L. Encarnacao (Eds.), *Geometric Modeling for CAD Applications* (pp. 3-36). North-Holland.
3. Weiler, K. (1988). Boundary graph operators for non-manifold geometric modeling representations. In *Geometric Modeling for CAD Applications* (pp. 37-66). North-Holland.
4. Lee, S. H., & Lee, K. (2001). Partial entity structure: A compact boundary representation for non-manifold geometric modeling. *Journal of Computing and Information Science in Engineering, 1*(4), 356-365.
5. Lee, S. H., & Lee, K. (2001). Partial entity structure: A compact non-manifold boundary representation based on partial topological entities. *Proc. 6th ACM Symposium on Solid Modeling and Applications*, 159-170.
6. Gursoz, E. L., Choi, Y., & Prinz, F. B. (1990). Vertex-based representation of non-manifold boundaries (NOODLES). In *Geometric Modeling for Product Engineering* (pp. 107-130). North-Holland.
7. Yamaguchi, Y., & Kimura, F. (1995). Nonmanifold topology based on coupling entities. *IEEE Computer Graphics and Applications, 15*(1), 42-50.
8. Lee, Y.-J., et al. (2012). Q-Complex: Efficient non-manifold boundary representation with inclusion topology. *Computer-Aided Design*. https://cg.cs.tsinghua.edu.cn/papers/lyj-CAD12.pdf
9. Rossignac, J. R., & O'Connor, M. A. (1990). SGC: A dimension-independent model for pointsets with internal structures and incomplete boundaries. In *Geometric Modeling for Product Engineering* (pp. 145-180). North-Holland.
10. Rossignac, J., & O'Connor, M. (1991). Representing geometric structures in d dimensions: Topology and order. *Discrete & Computational Geometry*.
11. DiCarlo, A., Paoluzzi, A., & Shapiro, V. (2014). Linear algebraic representation for topological structures. *Computer-Aided Design, 46*, 269-274.
12. DiCarlo, A., Milicchio, F., Paoluzzi, A., & Shapiro, V. (2009). Chain-based representations for solid and physical modeling. *IEEE Transactions on Automation Science and Engineering*. arXiv:0812.3249
13. Paoluzzi, A., et al. (2017). Geometric computing with chain complexes: Design and features of a Julia package. arXiv:1710.07819.
14. De Floriani, L., Hui, A., Panozzo, D., & Canino, D. (2010). A dimension-independent data structure for simplicial complexes. *Proc. 19th International Meshing Roundtable*, 403-420.
15. Lienhardt, P. (1991). Topological models for boundary representation: A comparison with n-dimensional generalized maps. *Computer-Aided Design, 23*(1), 59-82.
16. Lienhardt, P. (1994). N-dimensional generalized combinatorial maps and cellular quasi-manifolds. *International Journal of Computational Geometry & Applications, 4*(3), 275-324.
17. Damiand, G., & Teillaud, M. (2014). A generic implementation of dD combinatorial maps in CGAL. *Procedia Engineering, 82*, 46-58.
18. Kraemer, P., Untereiner, L., Jund, T., Thery, S., & Cazier, D. (2014). CGoGN: N-dimensional meshes with combinatorial maps. *Proc. 22nd International Meshing Roundtable* (pp. 485-503). Springer.
19. Masuda, H. (1993). Topological operators and Boolean operations for complex-based non-manifold geometric models. *Computer-Aided Design, 25*(2), 119-129.
20. Mantyla, M. (1988). *An Introduction to Solid Modeling*. Computer Science Press.
21. Spatial Corporation. *ACIS Cellular Topology Component* reference documentation (R10). Dassault Systemes.
22. Siemens Digital Industries Software. *Parasolid Functional Description* (Model Structure, Body Types, General Bodies) and *Parasolid XT Format Reference*.
23. Open CASCADE Technology. *Modeling Data* and *Boolean Operations* user guides; TopoDS class references. dev.opencascade.org.
24. Robinson, T. T., Armstrong, C. G., et al., and Abaqus/CAE midsurface modeling documentation; mixed-dimensional coupling literature (*Computer-Aided Design*, 2010).
