# Mesh Booleans, Implicit/SDF Modeling, and Hybrid B-rep/Mesh/Implicit Modeling

Research review supporting the design of Keel, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition. The driving question is where the conversion boundary between B-rep, mesh, and implicit representations should sit, and for each adjacent technology whether Keel should reimplement it, wrap an existing library, or treat it as an import/export format. Modern kernels increasingly ingest meshes (scan data, topology-optimization output) and interoperate with implicit/field-driven modeling (nTopology-style lattices and TPMS). This document surveys the state of the art across two parts and closes with an interop strategy.

Scope note on style: this document avoids em-dashes by deliberate house rule. All citations are APA-style. Each significant source carries Citation, Method, Results, Limitations, and Kernel relevance.

---

## PART 1: Mesh booleans and robust mesh processing

The central difficulty in mesh booleans is that triangle meshes carry no algebraic guarantee of being closed, manifold, non-self-intersecting, or consistently oriented. Real input (scans, STL exports, generative-design output) routinely violates all of these. The field has bifurcated into two philosophies: exact arrangement methods that compute the true intersection geometry with provable correctness, and tolerant/indirect methods that sidestep exactness by re-meshing, snapping, or replacing inside/outside topology propagation with field queries.

### 1.1 Exact mesh arrangements and exact predicates

#### Zhou, Grinspun, Zorin, and Jacobson (2016): Mesh Arrangements for Solid Geometry

**Citation.** Zhou, Q., Grinspun, E., Zorin, D., and Jacobson, A. (2016). Mesh arrangements for solid geometry. ACM Transactions on Graphics, 35(4), Article 39 (SIGGRAPH 2016).

**Method.** The paper reframes booleans as a two-stage problem. Stage one computes the exact arrangement of an arbitrary set of input triangles: every pairwise triangle intersection is found and the triangles are subdivided so the result is a valid simplicial complex with no interpenetration. This stage uses exact predicates (orientation and intersection tests) accelerated by floating-point filters, so the common case runs in hardware floating point and only ambiguous cases escalate to exact rational arithmetic. Stage two assigns an integer winding number to each cell of the arrangement by propagating across patches, then evaluates a per-operation predicate on the winding-number vector to decide which patches survive. Because any boolean (union, intersection, difference, symmetric difference, and arbitrary n-ary CSG) is just a function of winding numbers, the same machinery handles all of them, plus self-union and "outer hull" extraction.

**Results.** Robust on the full Thingi10K corpus including meshes with self-intersections, multiple components, and non-manifold features. It is the reference implementation in libigl, gptoolbox, and PyMesh and became the de facto correctness baseline against which later fast methods are measured.

**Limitations.** Exact rational fallback makes it slow on large or heavily intersecting inputs (seconds to minutes), and it materializes explicit exact intersection coordinates, which is memory-heavy. Not interactive.

**Kernel relevance.** This is the canonical algorithm to understand before building any mesh-boolean path. The winding-number-as-CSG-classifier idea is the cleanest known formulation and maps directly onto how Keel could classify mesh regions during convergent (mesh + B-rep) operations. Use it as the correctness oracle in Keel's test suite even if production uses a faster method.

#### Cherchi, Livesu, Scateni, and Attene (2020): Fast and Robust Mesh Arrangements using Floating-point Arithmetic

**Citation.** Cherchi, G., Livesu, M., Scateni, R., and Attene, M. (2020). Fast and robust mesh arrangements using floating-point arithmetic. ACM Transactions on Graphics, 39(6), Article 250 (SIGGRAPH Asia 2020).

**Method.** Builds the same exact arrangement as Zhou et al. but avoids ever constructing explicit exact coordinates for intersection points. It uses indirect predicates (Attene 2020): an intersection point is represented implicitly by the geometric primitives that define it (for example a line-plane intersection LPI), and every downstream geometric test is rewritten as a sign computation on the defining primitives' coordinates. These predicates are exact by construction yet evaluate almost entirely in filtered floating point. Triangle intersections are resolved locally and triangulated.

**Results.** Robust on 100 percent of a 4500-model stress set, while running roughly an order of magnitude faster than libigl/CGAL exact paths and close to the speed of non-robust floating-point implementations. Header-only C++ reference code is public (gcherchi/FastAndRobustMeshArrangements).

**Limitations.** Computes the arrangement; boolean classification (inside/outside) was originally a separate, slower topological propagation step (addressed in the 2022 follow-up). Single-threaded in the original release. The implicit-point representation complicates exporting exact coordinates if a downstream consumer wants them.

**Kernel relevance.** Indirect predicates are the single most important idea in this document for a Rust kernel. They let Keel get exactness without a bignum-everywhere architecture: store points implicitly, evaluate signs in filtered f64, escalate only on filter failure. This is portable to Rust cleanly and is the recommended foundation for a from-scratch robust mesh-boolean module.

#### Cherchi, Pellacini, Attene, and Livesu (2022): Interactive and Robust Mesh Booleans

**Citation.** Cherchi, G., Pellacini, F., Attene, M., and Livesu, M. (2022). Interactive and robust mesh booleans. ACM Transactions on Graphics, 41(6), Article 247 (SIGGRAPH Asia 2022).

**Method.** Extends the 2020 arrangement to a full, fast boolean. Two key changes. First, the arrangement stage is sped up roughly 5x by caching plane coefficients so each orient3D becomes a 4D scalar product, replacing O(n^2) earcut with deterministic O(n) segment-insertion triangulation, and parallelizing with Swiss-table hash maps, arena allocators, and cache-coherent arrays. Second, inside/outside classification is reformulated as exact ray casting rather than topological flooding: one ray per surface patch is shot to infinity, intersections are counted using LPI implicit points and indirect predicates, with a cascaded floating-point-then-rational fallback. This is up to 100x faster than flood-based classification.

**Results.** Interactive frame rates on meshes up to about 200K triangles on a laptop. Around 18x faster than libigl on interactive 50K-triangle rotation, 6.3x overall on a 3814-boolean Thingi10K batch, about 25x on million-triangle meshes. Public code (gcherchi/InteractiveAndRobustMeshBooleans).

**Limitations.** Cannot sustain interactive rates much above 200K triangles without temporal caching. Chaining operations across frames requires snap-rounding between frames, which breaks the robustness guarantee for cascaded booleans. Still requires inputs free of self-intersection per patch.

**Kernel relevance.** This is the current best-in-class open-source exact mesh boolean and the most likely candidate for Keel to either wrap (via FFI) or port. The per-patch ray-cast classifier is independently valuable: Keel can reuse it to classify mesh regions against an implicit or B-rep field. If Keel wants a native Rust mesh-boolean, this is the algorithm to reimplement.

#### Cherchi et al. (2024): Exact and Efficient Intersection Resolution for Mesh Arrangements

**Citation.** Cherchi, G., et al. (2024). Exact and efficient intersection resolution for mesh arrangements. ACM Transactions on Graphics, 43(6) (SIGGRAPH Asia 2024).

**Method.** Refines the intersection-resolution core of the arrangement pipeline, improving how coplanar and near-coplanar triangle clusters and complex intersection patterns are handled exactly, while keeping the indirect-predicate, filter-first design. It tightens the simplicial-complex construction so fewer cases fall back to expensive paths.

**Results.** Further speed and robustness improvements over the 2020/2022 line on pathological coplanar-heavy inputs, which are exactly the cases that defeat naive arrangement code.

**Limitations.** Incremental over the prior work; the architectural story is unchanged, so the same caveats about cascaded operations and per-patch self-intersection apply.

**Kernel relevance.** Confirms the Cherchi/Attene line is actively maintained and improving, lowering the risk of depending on it. Coplanar robustness is precisely where naive in-house implementations fail, so this is a strong argument for wrapping rather than reimplementing in Keel's first release.

#### Trettner, Nehring-Wirxel, and Kobbelt (2022): EMBER

**Citation.** Trettner, P., Nehring-Wirxel, J., and Kobbelt, L. (2022). EMBER: Exact mesh booleans via efficient and robust local arrangements. ACM Transactions on Graphics, 41(4), Article 163 (SIGGRAPH 2022).

**Method.** A fundamentally different exactness strategy: a plane-based mesh representation. Each face is stored as a plane with homogeneous integer coordinates, and vertices are defined implicitly as the intersection of three planes. All predicates then reduce to integer determinant sign tests, which are exact with bounded bit-width. Rather than build a global acceleration structure, EMBER does an adaptive recursive subdivision of the bounding box (an implicit octree), generating and tracking only the data needed in each cell on the fly, and computing local arrangements per cell. Boolean membership is decided through generalized winding numbers. A work-stealing multithreaded scheduler drives the recursion.

**Results.** On Thingi10K it outperforms state-of-the-art exact methods and even inexact ones such as QuickCSG by orders of magnitude, computing booleans between million-triangle meshes at interactive rates. Exactness is guaranteed by the integer plane arithmetic.

**Limitations.** The plane-based representation is a different data model than most kernels use, so input must be converted to planes and output converted back to explicit vertices, which involves controlled rounding. Integer coordinate growth must be managed. Less mature/public tooling than the Cherchi line.

**Kernel relevance.** The plane-based, vertices-as-plane-triples idea is conceptually close to how a B-rep already thinks (faces carry surfaces; vertices are intersections). For Keel this is the most architecturally sympathetic mesh-boolean method, because a B-rep kernel already stores faces with carrier geometry. Worth deep study as a long-term native approach, even if the first release wraps Cherchi.

#### Hachenberger, Kettner, and Mehlhorn (2007): Boolean Operations on 3D Selective Nef Complexes (CGAL Nef_3)

**Citation.** Hachenberger, P., Kettner, L., and Mehlhorn, K. (2007). Boolean operations on 3D selective Nef complexes: Data structure, algorithms, optimized implementation, and experiments. Computational Geometry: Theory and Applications, 38(1-2), 64-99. (See also Granados et al., 2003, ESA.)

**Method.** Nef polyhedra are the point sets obtained from a finite number of complement and intersection operations on half-spaces, so they are closed under all boolean operations and can exactly represent unbounded, mixed-dimensional, and non-manifold sets (isolated vertices, dangling faces). The implementation pairs a local pyramid (sphere map) at each vertex describing its neighborhood with a global incidence structure of edges, faces, and volumes. Booleans are computed by overlaying the two structures using exact rational arithmetic.

**Results.** Fully general and provably exact, handling cases (open vs closed sets, lower-dimensional features) that triangle-mesh methods cannot represent at all. Shipped and maintained in CGAL as Nef_3.

**Limitations.** Heavy: exact rational coordinates plus the sphere-map machinery make it slow and memory-intensive, and conversion to/from ordinary triangle meshes is lossy at the boundary unless an exact-construction kernel is used throughout. Overkill for closed-manifold-to-closed-manifold booleans.

**Kernel relevance.** A reference for the most general possible semantics (non-manifold, mixed-dimensional, open/closed). Keel will not adopt Nef_3 as a runtime path (too slow), but its representation theory informs how to model degenerate boolean results. Treat as a correctness reference and a possible offline fallback via CGAL, not a core.

#### Loriot et al.: CGAL Polygon Mesh Processing Corefinement and Boolean Operations

**Citation.** Loriot, S., Mesmoudi, M., Rouxel-Labbe, M., et al. CGAL Polygon Mesh Processing: Corefinement and Boolean operations. In CGAL User and Reference Manual (current: CGAL 6.x). The CGAL Project.

**Method.** Corefinement refines two triangle meshes so their intersection polylines become shared edges in both, after which the union, intersection, and difference of the bounded volumes are extracted by selecting and stitching the appropriate connected components. Correct topology requires a kernel with exact predicates (Exact_predicates_inexact_constructions_kernel). Robust geometry across chained operations additionally requires exact constructions (Exact_predicates_exact_constructions_kernel), otherwise output may carry self-intersections from rounding.

**Results.** Production-quality, widely deployed, far lighter than Nef_3 for the common closed-manifold case. Operates directly on a halfedge surface mesh, so no conversion to a special representation is needed.

**Limitations.** Slower than the Cherchi/EMBER state of the art. With only exact predicates (not constructions) the embedding can self-intersect; with exact constructions it slows down and accumulates exact coordinates. Inputs must be valid closed surfaces for clean volume semantics.

**Kernel relevance.** The pragmatic, well-documented baseline. If Keel needs mesh booleans on day one with minimal effort and is willing to take a C++ dependency, CGAL corefinement is the lowest-risk wrap. The corefinement primitive (refine both meshes along shared intersection curves) is itself reusable for imprinting one B-rep's edges onto another.

### 1.2 Indirect, tolerant, and field-based approaches

#### Jacobson, Kavan, and Sorkine-Hornung (2013): Robust Inside-Outside Segmentation using Generalized Winding Numbers

**Citation.** Jacobson, A., Kavan, L., and Sorkine-Hornung, O. (2013). Robust inside-outside segmentation using generalized winding numbers. ACM Transactions on Graphics, 32(4), Article 33 (SIGGRAPH 2013). SIGGRAPH Test-of-Time Award, 2024.

**Method.** Generalizes the 2D winding number to 3D as the sum of signed solid angles subtended by all triangles at a query point, divided by 4 pi. For a clean closed surface this is exactly 1 inside and 0 outside; for defective input (holes, self-intersections, non-manifold edges, multiple components) it degrades gracefully to a smooth real-valued field whose value still indicates "how enclosed" a point is. The paper drives a graph-cut over a constrained Delaunay tessellation using this field to produce a clean inside/outside volumetric segmentation that meets the input boundary exactly.

**Results.** Succeeds on inputs with thousands of intersecting triangle pairs and dozens of disconnected components (the "Big SigCat" example) that defeat conventional fill/repair. Requires only reasonably consistent triangle orientation, not watertightness.

**Limitations.** Naive evaluation is O(n) per query (every triangle contributes), so dense voxelization is expensive without acceleration. Needs orientation to be roughly consistent; wildly inconsistent normals degrade the field.

**Kernel relevance.** This is the key to ingesting dirty meshes. Keel can use generalized winding number as a representation-agnostic inside/outside oracle: classify mesh points, B-rep points, or arrangement cells uniformly. It is the bridge that lets a defective scan participate in a boolean without first being perfectly repaired.

#### Barill, Dickson, Schmidt, Levin, and Jacobson (2018): Fast Winding Numbers for Soups and Clouds

**Citation.** Barill, G., Dickson, N. G., Schmidt, R., Levin, D. I. W., and Jacobson, A. (2018). Fast winding numbers for soups and clouds. ACM Transactions on Graphics, 37(4), Article 43 (SIGGRAPH 2018).

**Method.** Accelerates generalized winding number evaluation with a Barnes-Hut-style hierarchical tree. Far-away clusters of triangles (or oriented points) are approximated by a Taylor expansion of the solid-angle kernel about the cluster center, so each query touches O(log n) clusters instead of all n primitives. An accuracy parameter beta trades exactness for speed by setting when a cluster is "far enough" to approximate. It extends the concept to oriented point clouds, not just triangle soups.

**Results.** Reduces winding-number evaluation from linear to roughly logarithmic per query, making dense voxelization, signing of distance fields, and defect-tolerant booleans practical at scale. Public reference code; integrated into Houdini (SideFX) and gpytoolbox.

**Limitations.** Approximate (controlled by beta); exact near-surface values still need direct evaluation. Tree build cost amortizes only over many queries. Point-cloud mode needs reliable oriented normals.

**Kernel relevance.** This is what makes the 2013 winding-number oracle fast enough for production. Keel should treat fast winding number as the standard acceleration for any inside/outside query against meshes or point sets, including conversion of scans to solids. Straightforward to reimplement in Rust.

#### Hu, Zhou, Gao, Jacobson, Zorin, and Panozzo (2018): Tetrahedral Meshing in the Wild (TetWild)

**Citation.** Hu, Y., Zhou, Q., Gao, X., Jacobson, A., Zorin, D., and Panozzo, D. (2018). Tetrahedral meshing in the wild. ACM Transactions on Graphics, 37(4), Article 60 (SIGGRAPH 2018).

**Method.** Converts an arbitrary triangle soup into a valid tetrahedral mesh whose surface approximates the input within a user-set epsilon-envelope. It builds a background Delaunay tetrahedralization of the input points plus grid samples, then performs a BSP subdivision in which each input triangle becomes a cutting plane against the background tets, all in exact rational arithmetic. A final mesh-improvement pass optimizes element quality, and the surface is recovered as the tet faces nearest the envelope. Inside/outside is resolved with winding numbers.

**Results.** Produces inversion-free, non-degenerate tet meshes from genuinely broken input (self-intersections, gaps, non-manifold), succeeding where constrained tetrahedralizers fail. The tet mesh doubles as a robust mesh-repair and boolean substrate: booleans become tagging operations on tets.

**Limitations.** Slow due to pervasive rational arithmetic (minutes per model). Output volume-meshes, not surface-meshes, so recovering a clean boundary surface costs an extraction step. Envelope approximation means the output surface is not bit-identical to the input.

**Kernel relevance.** Establishes the "re-mesh to a tolerant volume, then operate" paradigm. For Keel, TetWild-style tolerant tetrahedralization is the heavy-duty fallback for hopeless input: when exact arrangement refuses, convert to a tet mesh, do the operation as cell tagging, extract the surface. More relevant as a repair/simulation bridge than as a primary boolean engine.

#### Hu, Schneider, Wang, Zorin, and Panozzo (2020): Fast Tetrahedral Meshing in the Wild (fTetWild)

**Citation.** Hu, Y., Schneider, T., Wang, B., Zorin, D., and Panozzo, D. (2020). Fast tetrahedral meshing in the wild. ACM Transactions on Graphics, 39(4), Article 117 (SIGGRAPH 2020).

**Method.** Same guarantees as TetWild but replaces the exact BSP with floating-point incremental triangle insertion interleaved with mesh optimization. Input triangles are inserted one at a time; vertices are snapped within a delta tolerance during plane-tet intersection; precomputed subdivision tables keep refinement consistent. A hybrid kernel falls back to rational arithmetic only locally, when the AMIPS distortion energy exceeds about 10^8, which is rare. Inside/outside again via winding numbers.

**Results.** About 7x faster than TetWild (around 50s vs 360s average on Thingi10K) with a 99.97 percent success rate on the 10,000-model corpus. Always emits a valid floating-point mesh. In practice all input triangles were inserted across the corpus despite the algorithm not guaranteeing it.

**Limitations.** Trades the formal input-preservation guarantee for speed (empirically preserved, not provably). Still produces an approximating surface within the envelope, not the exact input. Volume output.

**Kernel relevance.** The practical version of the tolerant-tet paradigm and the one to actually use. For Keel, fTetWild is the recommended wrap for the "repair an arbitrary scan/topology-opt mesh into a clean solid" use case, and as a robustness safety net for booleans that the exact path cannot handle. Its hybrid floating-point-with-local-rational-fallback design mirrors the indirect-predicate philosophy and validates it.

#### Mandoline-style cut-cell methods

**Citation.** Tao, M., Batty, C., Ben-Chen, M., Fiume, E., and Levin, D. I. W. (2019). Mandoline: Robust cut-cell generation for arbitrary triangle meshes. ACM Transactions on Graphics, 38(6) (SIGGRAPH Asia 2019).

**Method.** Embeds an arbitrary (possibly non-watertight, self-intersecting) triangle mesh into a background grid and robustly computes the cut cells where the mesh slices grid voxels, producing a consistent cell complex with inside/outside labels even for imperfect input. It combines exact predicates for the cutting with tolerant handling of grid-mesh degeneracies.

**Results.** Produces robust volumetric cut-cell decompositions usable for simulation and signed fields from input too dirty for exact booleans.

**Limitations.** Grid-based, so feature fidelity is bounded by grid resolution; output is a cell complex, not a B-rep. Aimed at simulation rather than CAD-quality surfaces.

**Kernel relevance.** Of niche interest to Keel: a tolerant route to a signed/labelled volume from bad meshes, relevant if Keel ever needs voxelized simulation export. Not a boolean engine for CAD-quality output. Lower priority than fTetWild for the same problem space.

### 1.3 Mesh repair

#### Attene (2010): A Lightweight Approach to Repairing Digitized Polygon Meshes (MeshFix)

**Citation.** Attene, M. (2010). A lightweight approach to repairing digitized polygon meshes. The Visual Computer, 26(11), 1393-1406.

**Method.** Assumes the input is meant to be a single closed solid and produces a single watertight, manifold, intersection-free triangle mesh. Rather than globally resampling, it works locally: it iteratively detects undesired configurations (degenerate triangles, non-manifold edges/vertices, self-intersections, holes), removes growing neighborhoods around them, and patches the resulting gaps, leaving clean regions untouched. The result bounds a well-defined polyhedron.

**Results.** Effective on a wide range of digitized/scan meshes, preserving good geometry while surgically fixing defects. Public C++ (MeshFix-V2.1) and Python (pymeshfix) implementations are widely used.

**Limitations.** Assumes a single solid component, so it can merge or discard parts of multi-component models. Local patching can alter geometry near defects and does not guarantee feature preservation. Not exact-arithmetic by default.

**Kernel relevance.** A ready, license-permissive repair stage to run on meshes before they enter Keel's boolean or convergent paths. Cheaper and more surface-faithful than tet-meshing for moderately defective input. Strong candidate to wrap as a preprocessing utility.

#### Attene, Campen, and Kobbelt (2013): Polygon Mesh Repairing: An Application Perspective (survey)

**Citation.** Attene, M., Campen, M., and Kobbelt, L. (2013). Polygon mesh repairing: An application perspective. ACM Computing Surveys, 45(2), Article 15.

**Method.** A taxonomy of mesh defects (holes, gaps, self-intersections, non-manifold elements, inconsistent orientation, degeneracies, noise) and of repair algorithms, classified as surface-oriented (operate directly on the mesh, preserve detail, local) versus volumetric (voxelize/resample, guarantee watertightness, lose detail). It maps which repair method fits which downstream application (rendering, printing, simulation, CAD).

**Results.** The definitive map of the repair landscape; clarifies the fundamental tradeoff that surface methods preserve fidelity but cannot guarantee global validity, while volumetric methods guarantee validity but resample everything.

**Limitations.** A 2013 survey, so it predates winding-number-driven repair, fTetWild, and indirect-predicate booleans; still conceptually current on the taxonomy.

**Kernel relevance.** The design reference for Keel's repair subsystem. It tells Keel to offer both a surface-preserving path (MeshFix-like) and a volumetric guaranteed path (fTetWild-like) and to choose by downstream need. Directly informs the API surface of an import/repair module.

### 1.4 Hybrid B-rep + mesh modeling in shipping products

#### Siemens Parasolid Convergent Modeling

**Citation.** Siemens Digital Industries Software. Parasolid with Convergent Modeling. PLM Components technical material and product announcements (Convergent Modeling introduced across Parasolid v28-v29, expanded through v33, 2016-2021).

**Method.** Parasolid represents faceted (mesh) geometry as facet B-rep, a first-class face type living in the same topology model (body/shell/face/edge/vertex) as classic analytic and NURBS B-rep. A single body can mix facet faces and classic faces. Because both are just faces in one topology graph, Parasolid's existing operations (booleans, blending, offsetting, sectioning, mass properties) run across the mix without translating facets into NURBS, which is the step that was slow and error-prone historically.

**Results.** Enables direct CAD workflows on scan data, topology-optimization output, and 3D-print meshes (the "currency of scanning, 3D printing, and generative design") inside one modeling session, marketed across medical implants, generative engineering, and reverse engineering. Shipped and maturing over many releases.

**Limitations.** Public material is light on internal algorithms and on which operations are restricted on facet faces; facet faces lack analytic parameterization, so some operations are necessarily approximate or unavailable. Facet quality still bounds downstream operation quality.

**Kernel relevance.** This is the strategic blueprint for Keel. The decisive lesson is architectural: make a facet/mesh face a first-class face type in the same B-rep topology rather than a separate world, so that booleans, blends, and offsets are written once and dispatch on face type. Keel should design its topology and geometry traits from day one to admit a FacetSurface alongside analytic and NURBS surfaces.

#### Autodesk Fusion 360 Mesh Environment

**Citation.** Autodesk. Fusion 360 Mesh workspace documentation. Autodesk Help.

**Method.** Fusion provides a dedicated mesh workspace where imported/scan/topology-opt meshes are edited (remesh, reduce, repair, plane-cut) and then, where the geometry suits it, converted to solid/B-rep bodies for parametric editing. It exposes both prismatic and organic (faceted/quad) conversion paths and supports limited direct mesh operations alongside B-rep ones.

**Results.** A widely deployed example of the mesh-as-staging-area workflow: meshes are first-class for import and cleanup but are usually converted to B-rep before serious parametric modeling.

**Limitations.** Mesh-to-B-rep conversion quality varies and large meshes are costly; not a fully unified single-topology model the way Parasolid convergent modeling is.

**Kernel relevance.** Represents the alternative to convergent modeling: keep mesh and B-rep mostly separate and convert at a boundary. Useful contrast for Keel's decision of whether to unify (Parasolid path) or to convert (Fusion path). Suggests Keel should at minimum ship strong mesh-to-B-rep conversion even if it also pursues unification.

#### Rhino SubD + NURBS + Mesh Coexistence

**Citation.** Robert McNeel and Associates. Rhinoceros (Rhino) documentation on SubD, NURBS, and Mesh object types and conversions.

**Method.** Rhino maintains three distinct geometry object types (NURBS surfaces/breps, SubD surfaces, and meshes) that coexist in one document with explicit conversion commands between them (ToNURBS, ToSubD, mesh from brep, etc.). SubD provides an editable intermediate that converts cleanly to NURBS, while meshes serve import/export and analysis.

**Results.** A pragmatic multi-representation design where each type is authoritative for its strengths and conversions are explicit user actions rather than automatic unification.

**Limitations.** Conversions are lossy and user-driven; there is no single unified topology in which all three participate in one boolean.

**Kernel relevance.** Validates that a clean trait-based geometry abstraction with explicit, well-characterized conversions is a viable shipping architecture. For Keel it supports designing geometry as a small set of surface kinds behind a common trait, with conversions as named, tested operations.

---

## PART 2: Implicit and SDF modeling

Implicit representations define a solid by a scalar field f over space, with the solid as the sublevel set f <= 0 (or the surface as f = 0). Booleans become field arithmetic (min/max or smooth variants), offsets become adding a constant, and shells become differences of offsets, all trivially and robustly, with no topology bookkeeping. The cost is that surfaces must be extracted (meshed) to interoperate with B-rep, and exact sharp features and analytic surfaces are hard to recover. This is the world of nTopology-style field-driven design and additive-manufacturing lattices.

### 2.1 Foundations of implicit and function representation

#### Bloomenthal et al.: Introduction to Implicit Surfaces

**Citation.** Bloomenthal, J. (Ed.). (1997). Introduction to Implicit Surfaces. Morgan Kaufmann. (With Bajaj, Blinn, Cani, Rockwood, Wyvill, and Wyvill.)

**Method.** The foundational text. Defines implicit surfaces as level sets of scalar fields, covers skeletal/blobby primitives and their summation blends, polygonization (continuation and grid methods for extracting a mesh from f = 0), conversion between implicit and parametric forms, and the mathematics of blending and offset. Establishes the vocabulary (field, potential, iso-surface, polygonization) used by all later work.

**Results.** The reference framing for why implicits make blends, offsets, and booleans easy and why meshing them is the central practical problem.

**Limitations.** Pre-dates GPU sphere tracing, OpenVDB, R-function rigor, and neural implicits; conceptual rather than algorithmically current.

**Kernel relevance.** Required grounding for Keel's implicit module design and terminology. It frames the core tradeoff Keel must manage: implicits give cheap blends/offsets/booleans but owe a meshing/conversion step to rejoin the B-rep world.

#### Shapiro (2007): Semi-Analytic Geometry with R-Functions (Rvachev theory)

**Citation.** Shapiro, V. (2007). Semi-analytic geometry with R-functions. Acta Numerica, 16, 239-303. (Foundational: Rvachev, V. L. (1982). Theory of R-functions and Some Applications.)

**Method.** An R-function (Rvachev function) is a real function whose sign depends only on the signs of its arguments, so it encodes a Boolean operation in continuous real analysis. R-conjunction and R-disjunction give exact intersection and union of implicitly defined solids while remaining smooth (Ck) except where required, so CSG over implicits becomes closed-form field composition with controllable continuity. This lets one build a single analytic function that is exactly negative inside a CSG solid and positive outside, with tunable smoothness of blends.

**Results.** A rigorous, exact theory of implicit CSG with provable sign and smoothness properties, underpinning meshfree analysis (the R-function method, RFM) and field-driven modeling. Decades of use in modeling, computational physics, and engineering.

**Limitations.** R-function CSG yields a field whose magnitude is not a true Euclidean distance (only sign is exact), which complicates sphere tracing and offsetting unless normalized. Building and evaluating deep CSG trees can be costly and numerically delicate.

**Kernel relevance.** This is the exact-implicit-boolean theory Keel needs if it offers an implicit modeling path. R-functions give Keel mathematically exact CSG with controllable blend smoothness, the implicit counterpart to B-rep blends. Key caveat for Keel: distinguish sign-exact R-function fields from true SDFs, because offset/shell/sphere-trace correctness depends on the distance property.

#### Pasko et al.: F-rep and HyperFun

**Citation.** Pasko, A., Adzhiev, V., Sourin, A., and Savchenko, V. (1995). Function representation in geometric modeling: concepts, implementation and applications. The Visual Computer, 11(8), 429-446. (Language: Adzhiev et al. (1999), HyperFun.)

**Method.** Function Representation (F-rep) generalizes CSG: a solid is a single real function f(x,y,z) >= 0, built from primitives and operations (including R-function booleans, blends, offsets, sweeps, and deformations) in a closed algebra over functions. HyperFun is a minimal high-level language (assignments, conditionals, loops, arithmetic/relational operators) for authoring such functions, including higher-dimensional and heterogeneous (multi-material) objects.

**Results.** A general, extensible implicit-modeling framework supporting far more primitives and operators than classic CSG, with open tooling used in research and education.

**Limitations.** Like all implicits, owes a polygonization step to produce boundary meshes; evaluation cost grows with the function tree; sharp-feature recovery on meshing is not automatic.

**Kernel relevance.** The conceptual ancestor of nTopology-style field modeling and a model for how Keel could expose a composable implicit/field language as an authoring layer above the B-rep core. Confirms that booleans, blends, offsets, and lattices are uniform field operations, which is the value proposition Keel must interoperate with.

### 2.2 Discrete SDFs and iso-surfacing

#### Frisken, Perry, Rockwood, and Jones (2000): Adaptively Sampled Distance Fields

**Citation.** Frisken, S. F., Perry, R. N., Rockwood, A. P., and Jones, T. R. (2000). Adaptively sampled distance fields: a general representation of shape for computer graphics. Proceedings of SIGGRAPH 2000, 249-254.

**Method.** Store the signed distance field in an adaptive spatial hierarchy (octree in 3D; quadtree in 2D), sampling densely only where the field has high curvature/detail and sparsely where it varies smoothly. Distance and gradient inside a cell are reconstructed by trilinear interpolation of corner samples. ADFs unify many operations: rendering, sculpting, offsetting, level-of-detail, and collision all become field queries.

**Results.** High geometric accuracy at far lower memory than a uniform grid, with a single representation serving rendering, editing, and analysis.

**Limitations.** Trilinear reconstruction blurs sharp features unless special cells are used; building an accurate ADF from arbitrary geometry requires reliable distance queries; not exact at edges/corners.

**Kernel relevance.** The classic adaptive-SDF data structure and the natural format for Keel to cache distance fields of B-rep or implicit bodies for fast queries (proximity, offset, collision). Directly relevant to converting a B-rep into a sampled field and back.

#### Museth (2013): VDB / OpenVDB

**Citation.** Museth, K. (2013). VDB: High-resolution sparse volumes with dynamic topology. ACM Transactions on Graphics, 32(3), Article 27. Open-source as OpenVDB (Academy Software Foundation).

**Method.** A hierarchical, B+tree-like sparse volume structure over a virtually infinite 3D index space, with a shallow fixed-depth tree of progressively branching nodes ending in dense leaf tiles. It gives average O(1) random access, cache-coherent sequential traversal, and unrestricted (dynamic) sparse topology, ideal for narrow-band level sets where only a thin shell around the surface stores data. Includes tools for converting meshes to narrow-band SDFs, level-set CSG, filtering, and re-meshing.

**Results.** The industry-standard sparse-volume engine (VFX and increasingly engineering), enabling high-resolution narrow-band level sets and SDF operations at production scale. Mature, permissively licensed, widely deployed.

**Limitations.** Narrow-band level sets carry distance only near the surface; CSG via level-set min/max is resolution-limited and approximate at sharp features; voxel resolution bounds fidelity.

**Kernel relevance.** The obvious thing to wrap for sparse-SDF storage and level-set operations rather than reinvent. For Keel, OpenVDB is the recommended backend for voxel/SDF interchange, mesh-to-SDF conversion, and large narrow-band fields. It is the de facto interchange substrate when interoperating with implicit/field tools.

#### Ju, Losasso, Schaefer, and Warren (2002): Dual Contouring of Hermite Data

**Citation.** Ju, T., Losasso, F., Schaefer, S., and Warren, J. (2002). Dual contouring of hermite data. ACM Transactions on Graphics, 21(3), 339-346 (SIGGRAPH 2002).

**Method.** Contours a signed grid whose edges carry Hermite data (exact zero-crossing positions and surface normals). For each grid cell containing the surface, it places one vertex at the minimizer of a quadratic error function (QEF) built from the cell's Hermite samples, then connects vertices across shared edges to form quads. The QEF minimizer naturally lands on sharp edges and corners, so features are reproduced without explicit feature detection, and the same machinery drives octree-based adaptive simplification with no crack-patching.

**Results.** Reproduces sharp edges and corners that marching cubes rounds off, on adaptive octrees, with a stable QEF formulation. Became the standard sharp-feature iso-surfacing method.

**Limitations.** Can generate non-manifold vertices where multiple surface sheets pass through one cell (addressed by Manifold Dual Contouring, Schaefer et al. 2007). QEF needs reliable normals; noisy Hermite data degrades features. Output is quads needing triangulation for many consumers.

**Kernel relevance.** The method Keel should use to extract a feature-preserving mesh from an implicit/SDF body, which is the implicit-to-mesh half of the conversion boundary. Because B-rep edges are exactly the sharp features Keel cares about, dual contouring's feature recovery is the right primitive when surfacing fields that originated from CAD.

#### Kobbelt, Botsch, Schwanecke, and Seidel (2001): Extended Marching Cubes

**Citation.** Kobbelt, L. P., Botsch, M., Schwanecke, U., and Seidel, H.-P. (2001). Feature sensitive surface extraction from volume data. Proceedings of SIGGRAPH 2001, 57-66.

**Method.** Augments the signed grid with directional (Hermite) information and, in cells flagged as containing a sharp feature, inserts an extra vertex at the intersection of the tangent planes sampled at edge crossings (rather than averaging), then re-triangulates to preserve the crease. A predecessor and complement to dual contouring for sharp-feature iso-surfacing.

**Results.** Removes the aliasing/rounding of sharp features that plain marching cubes produces, at modest extra cost, given an enhanced distance-field representation.

**Limitations.** Relies on detecting feature cells and on accurate normals; primal (marching-cubes-topology) rather than dual, so it inherits some MC ambiguity handling. Largely superseded by dual contouring for octree adaptivity.

**Kernel relevance.** The other half of the sharp-feature iso-surfacing literature Keel must know. Together with dual contouring it defines the menu for implicit-to-B-rep surfacing; Keel will likely favor (manifold) dual contouring but should understand EMC's tangent-plane intersection as the feature-recovery idea.

### 2.3 Smooth blends, offsets, and additive-manufacturing fields

#### nTopology / nTop: Field-Driven Implicit Modeling

**Citation.** nTopology, Inc. (now nTop). Implicit modeling for engineering design; Field-driven design (technical blog and whitepaper material).

**Method.** nTop represents solids as signed distance fields evaluated lazily and in parallel (CPU/GPU). Booleans, offsets, shells, fillets, and lattice/TPMS infills are all field operations: union/intersection via min/max (smooth variants for fillets), offset via adding a scalar, shell via difference of offsets, lattice by combining a periodic unit-cell field with a domain field. "Fields" (scalars over space, possibly from simulation, test data, or formulas) drive parameters spatially, so wall thickness or lattice density can vary point-by-point. Meshing to triangles happens only at export.

**Results.** Robustly produces geometry (graded lattices, TPMS, conformal infills) that is essentially impossible to model in classic B-rep, at scale and without boolean failures, because field operations never fail topologically. Widely adopted in aerospace/medical additive manufacturing.

**Limitations.** Output is ultimately a (often huge) mesh; recovering editable analytic B-rep from the field is the hard reverse direction. Field magnitude is not always a true SDF after smooth blends, affecting offset accuracy. Proprietary engine.

**Kernel relevance.** This is the primary external implicit ecosystem Keel must interoperate with, not reimplement wholesale. The clear strategy: let Keel export B-rep bodies as SDFs/meshes into field tools, and import their mesh/field output back through Keel's mesh-to-B-rep and convergent paths. Understanding that lattices/TPMS are cheap in field-land and expensive in B-rep-land tells Keel where the boundary belongs: do graded lattices implicitly, not as B-rep.

#### Carbon and TPMS/Lattice Generation for Additive Manufacturing

**Citation.** Industry and academic material on triply periodic minimal surfaces (gyroid, Schwarz P/D) and lattice generation for additive manufacturing (e.g., Carbon Design Engine; survey: review of geometric modeling methods in microstructure design, arXiv:2411.15833, 2024).

**Method.** TPMS are defined directly as implicit iso-surfaces of simple trigonometric fields (for example the gyroid sin x cos y + sin y cos z + sin z cos x = c), so a lattice is just a field thresholded and intersected with a part's SDF, with the iso-value controlling volume fraction and a spatially varying field grading density. Generation is therefore an implicit operation, meshed only at the end. Carbon's design engine parameterizes lattices for printability and mechanical targets.

**Results.** Compact, robust generation of mechanically tuned cellular structures (energy absorption, lightweighting, heat exchange) that scale to billions of effective surface elements because they are never explicitly stored until meshing.

**Limitations.** Explicit B-rep or even explicit mesh of a dense TPMS lattice is enormous; printability and self-support constraints must be enforced; the field-to-printable-mesh step dominates cost.

**Kernel relevance.** Concrete evidence that lattice/TPMS work belongs on the implicit side of Keel's boundary. Keel should provide implicit primitives (TPMS fields, unit-cell fields) and intersect them with B-rep-derived SDFs, exporting the result as mesh, rather than attempting B-rep lattices.

### 2.4 Conversion across the implicit / B-rep boundary

#### Exact and Computed Distance Fields of NURBS B-reps

**Citation.** Distance-field-computation literature for parametric and NURBS surfaces (e.g., point-to-NURBS closest-point and signed-distance methods; trimmed-surface containment work such as arXiv:2510.25159, 2025), with surveys of SDF computation for parametric models.

**Method.** Computing the signed distance from a query point to a trimmed NURBS B-rep requires, per face, a closest-point projection onto the surface (Newton iteration in parameter space, robust initialization by control-net sampling), trimming-curve containment tests to reject projections outside the trimmed region, and aggregation (minimum unsigned distance over faces, sign from inside/outside via winding number or ray casting). Exact distance to NURBS has no closed form, so practical pipelines compute it to tolerance and cache it in an ADF/VDB structure. Trimmed-surface point containment is itself a robustness challenge (the cited 2025 work targets fast, robust containment).

**Results.** Enables turning a B-rep into a queryable SDF so it can enter field-driven workflows (offset, lattice intersection, blending) on equal footing with native implicits.

**Limitations.** Closest-point on NURBS is iterative and can be slow or non-robust near trimming boundaries and high-curvature regions; signing requires a reliable inside/outside oracle; the field is sampled/toleranced, not analytic.

**Kernel relevance.** This is the B-rep-to-implicit direction Keel needs to feed nTop-style tools. Concretely: implement robust closest-point-on-NURBS plus trimming containment, sign with generalized winding number, cache in an adaptive field (ADF/OpenVDB). This, plus dual contouring for the reverse direction, is the conversion machinery at the heart of Keel's implicit interop.

#### Surfacing of Level Sets / Feature-Preserving Iso-Surfacing for Reverse Engineering

**Citation.** Synthesis of the iso-surfacing literature above (Ju et al. 2002; Kobbelt et al. 2001; Manifold Dual Contouring, Schaefer et al. 2007) applied to reverse engineering, plus OpenVDB level-set-to-mesh tools (Museth 2013).

**Method.** The implicit-to-B-rep pipeline: extract a feature-preserving triangle mesh from the field with (manifold) dual contouring or extended marching cubes, then optionally segment that mesh into regions and fit analytic/NURBS surfaces per region to recover an editable B-rep (the reverse-engineering step). Sharp edges recovered by the iso-surfacer become candidate B-rep edges.

**Results.** Bridges field/scan data to editable CAD when fidelity allows; quality hinges on faithful sharp-feature recovery during iso-surfacing.

**Limitations.** Mesh-to-analytic fitting is heuristic and error-prone for complex shapes; perfect editable B-rep recovery from arbitrary fields is unsolved in general; tolerance accumulates across the chain.

**Kernel relevance.** Defines the realistic ceiling of implicit-to-B-rep conversion for Keel: faithful feature-preserving meshing is achievable and worth building; full automatic re-NURBS-ing is a best-effort, not a guarantee. Keel should ship the meshing/feature-recovery step and treat surface fitting as assisted, not automatic.

### 2.5 Neural implicits (conversion-boundary relevance only)

#### Park, Florence, Straub, Newcombe, and Lovegrove (2019): DeepSDF

**Citation.** Park, J. J., Florence, P., Straub, J., Newcombe, R., and Lovegrove, S. (2019). DeepSDF: Learning continuous signed distance functions for shape representation. CVPR 2019, 165-174.

**Method.** Trains an auto-decoder MLP that maps a latent code plus a 3D point to a signed distance value, so a whole class of shapes is encoded as a continuous learned SDF. The surface is the network's zero level set; a new shape is recovered by optimizing its latent code to fit partial/noisy observations, enabling completion and interpolation.

**Results.** State-of-the-art learned shape representation and completion in 2019 at an order-of-magnitude smaller model size than voxel/point alternatives, demonstrating that neural fields can represent and reconstruct shape families.

**Limitations.** The learned field is only approximately a true SDF (Eikonal property not enforced exactly), accuracy is bounded by training distribution, and sharp CAD features and exactness are not guaranteed. Not directly usable as exact CAD geometry.

**Kernel relevance.** Relevant only at the conversion boundary: a neural SDF is just another field Keel could iso-surface (via dual contouring) and then attempt to reverse-engineer. Keel should treat neural implicits as a possible import source of approximate geometry, never as an internal exact representation. No core dependency warranted.

### 2.6 Guarantees: Lipschitz evaluation and topologically correct meshing

#### Hart (1996): Sphere Tracing and Lipschitz Bounds

**Citation.** Hart, J. C. (1996). Sphere tracing: a geometric method for the antialiased ray tracing of implicit surfaces. The Visual Computer, 12(10), 527-545.

**Method.** To intersect a ray with an implicit surface, march along the ray in steps equal to a lower bound on the distance to the surface, guaranteeing no step overshoots. For a true SDF the field value is exactly that safe step; more generally a Lipschitz bound on the field gives a conservative step (field value divided by the Lipschitz constant). This is correct even for surfaces whose gradient vanishes or jumps, and it supports cone-traced antialiasing.

**Results.** The standard, provably non-overshooting method for rendering SDFs/implicits, foundational to all GPU SDF rendering and to nTop-style real-time field visualization.

**Limitations.** Correctness and efficiency depend on having a valid (and tight) Lipschitz bound; loose bounds make tracing slow, and fields that are not true SDFs (for example after smooth R-function blends) need a known Lipschitz constant or normalization to stay correct.

**Kernel relevance.** If Keel renders or queries fields, sphere tracing is the method, and the practical takeaway is that Keel must track Lipschitz bounds (or maintain true distance) through field operations to keep ray queries correct. This reinforces the earlier caution that R-function/smooth-blend fields are sign-exact but not distance-exact.

#### Plantinga and Vegter (2004/2007): Isotopic Approximation of Implicit Surfaces

**Citation.** Plantinga, S., and Vegter, G. (2004). Isotopic approximation of implicit curves and surfaces. Eurographics/ACM SIGGRAPH Symposium on Geometry Processing, 245-254. (Extended: The Visual Computer, 2007.)

**Method.** Uses interval arithmetic to certify global properties of the implicit function over octree cells: it adaptively subdivides until each cell is small enough that interval evaluation of f and of grad f guarantees the surface inside is simple, then extracts a mesh proven to be isotopic (topologically equivalent under ambient deformation) to the true surface, not merely close to it.

**Results.** Among the first practical algorithms to guarantee the extracted mesh has the correct topology (no spurious handles, no missed components), driven entirely by interval-arithmetic certificates.

**Limitations.** Requires an interval extension of the field (and its gradient), which not all fields admit cheaply; can over-refine near features; guarantees topology but not sharp-feature fidelity by itself.

**Kernel relevance.** This is how Keel can mesh an implicit body with a topology guarantee, which matters for a kernel that must not silently produce a wrong-genus solid. Interval/Lipschitz certification is the rigorous backbone for trustworthy implicit-to-mesh conversion, complementing dual contouring's feature handling.

#### Boissonnat and Oudot (2005): Provably Good Sampling and Meshing of Surfaces

**Citation.** Boissonnat, J.-D., and Oudot, S. (2005). Provably good sampling and meshing of surfaces. Graphical Models, 67(5), 405-451. (Related: Boissonnat and Oudot, 2006, meshing of Lipschitz surfaces, SoCG.)

**Method.** Delaunay-refinement sampling: incrementally insert sample points and maintain the restricted Delaunay triangulation (the part of the 3D Delaunay triangulation that lies on the surface). With a sampling density tied to the local feature size (epsilon-sample, and the easier-to-verify loose epsilon-sample), the restricted Delaunay triangulation is provably a good topological and geometric approximation of a smooth surface, with bounded element quality.

**Results.** Rigorous guarantees on both topology and geometric accuracy plus controllable triangle quality, the theoretical basis for CGAL's surface and 3D mesh generators.

**Limitations.** Assumes a smooth (C2 or Lipschitz) surface and access to closest-point/intersection queries; feature size must be estimable; not designed for sharp CAD creases without modification.

**Kernel relevance.** The quality-and-correctness counterpart to Plantinga-Vegter, and the algorithm family behind CGAL's mesher. For Keel, this is the route to well-shaped, provably faithful meshes of smooth implicit or B-rep surfaces (for example for analysis), available off the shelf via CGAL. Pair it with dual contouring (features) and Plantinga-Vegter (topology certificate) to cover the meshing design space.

---

## Interop boundary strategy for Keel

The literature points to a clear division of labor. The recurring, validated pattern (Parasolid convergent modeling, nTop field-driven design, Cherchi/Attene indirect predicates, fTetWild tolerant fallback) is: keep an exact B-rep core, admit meshes as a first-class face type, push lattice/offset/blend-heavy work to implicit fields, and own the conversion machinery in both directions.

What to BUILD natively in Rust:
1. Indirect predicates (Attene/Cherchi style): implicit point representation plus filtered-float-with-rational-fallback sign tests. This is the robustness foundation for everything geometric and ports cleanly to Rust. Highest priority.
2. Generalized winding number plus the fast (Barnes-Hut) accelerator as Keel's universal inside/outside oracle across meshes, point clouds, and arrangements. Cheap to implement, high leverage.
3. A facet/mesh surface as a first-class face type in the B-rep topology (the convergent-modeling lesson), so booleans/blends/offsets dispatch on surface kind and are written once.
4. The B-rep-to-implicit bridge: robust closest-point-on-NURBS plus trimming containment, signed by winding number, cached in an adaptive field. This is what feeds external field tools and has no good off-the-shelf substitute tailored to Keel's geometry.
5. Feature-preserving iso-surfacing via (manifold) dual contouring for the implicit-to-mesh direction, since B-rep edges are exactly the sharp features to recover.

What to WRAP rather than reimplement (at least initially):
1. Exact mesh booleans: the Cherchi/Attene "Interactive and Robust Mesh Booleans" line (actively maintained, 2020-2024) for the native mesh-on-mesh boolean, with CGAL corefinement as a conservative fallback. Reimplement natively (likely following EMBER's plane-based model, which suits a B-rep kernel) only once the wrap is proven and a Rust port is justified.
2. OpenVDB for sparse-SDF/narrow-band storage, mesh-to-SDF conversion, and level-set CSG interchange. It is the de facto substrate for talking to implicit/VFX/engineering tools.
3. fTetWild for the tolerant "repair an arbitrary scan or topology-opt mesh into a clean solid" path and as a robustness safety net when exact booleans refuse. MeshFix for lighter, surface-preserving repair.
4. CGAL surface/3D meshers (Delaunay refinement, Boissonnat-Oudot) for provably good smooth-surface meshing when analysis-quality meshes are needed.

What to CONVERT at the boundary (do not try to unify):
1. Dense lattices and TPMS stay implicit; intersect TPMS/unit-cell fields with B-rep-derived SDFs and export as mesh. Never materialize them as B-rep. This is where nTop/Carbon prove the implicit side wins decisively.
2. Neural implicits (DeepSDF and kin) are an approximate import source only: iso-surface them, then assisted-fit; never an internal exact representation.
3. Full implicit-to-editable-B-rep (re-NURBS-ing) is best-effort/assisted, not a guarantee. Ship faithful feature-preserving meshing; treat analytic surface fitting as a human-in-the-loop reverse-engineering aid.

Cross-cutting guarantees to honor: track Lipschitz bounds or maintain true distance through field operations (Hart) so sphere-traced and offset queries stay correct, and prefer topology-certified meshing (Plantinga-Vegter interval certificates, Boissonnat-Oudot sampling guarantees) wherever Keel emits a solid from a field, so the kernel never silently changes a solid's genus. Distinguish sign-exact fields (R-functions, smooth blends) from true SDFs in the type system, because offset/shell/sphere-trace correctness depends on the distinction.

---

## References

Attene, M. (2010). A lightweight approach to repairing digitized polygon meshes. The Visual Computer, 26(11), 1393-1406.

Attene, M., Campen, M., and Kobbelt, L. (2013). Polygon mesh repairing: An application perspective. ACM Computing Surveys, 45(2), Article 15.

Barill, G., Dickson, N. G., Schmidt, R., Levin, D. I. W., and Jacobson, A. (2018). Fast winding numbers for soups and clouds. ACM Transactions on Graphics, 37(4), Article 43.

Bloomenthal, J. (Ed.). (1997). Introduction to Implicit Surfaces. Morgan Kaufmann.

Boissonnat, J.-D., and Oudot, S. (2005). Provably good sampling and meshing of surfaces. Graphical Models, 67(5), 405-451.

Cherchi, G., Livesu, M., Scateni, R., and Attene, M. (2020). Fast and robust mesh arrangements using floating-point arithmetic. ACM Transactions on Graphics, 39(6), Article 250.

Cherchi, G., Pellacini, F., Attene, M., and Livesu, M. (2022). Interactive and robust mesh booleans. ACM Transactions on Graphics, 41(6), Article 247.

Cherchi, G., et al. (2024). Exact and efficient intersection resolution for mesh arrangements. ACM Transactions on Graphics, 43(6).

Frisken, S. F., Perry, R. N., Rockwood, A. P., and Jones, T. R. (2000). Adaptively sampled distance fields: a general representation of shape for computer graphics. Proceedings of SIGGRAPH 2000, 249-254.

Hachenberger, P., Kettner, L., and Mehlhorn, K. (2007). Boolean operations on 3D selective Nef complexes. Computational Geometry: Theory and Applications, 38(1-2), 64-99.

Hart, J. C. (1996). Sphere tracing: a geometric method for the antialiased ray tracing of implicit surfaces. The Visual Computer, 12(10), 527-545.

Hu, Y., Zhou, Q., Gao, X., Jacobson, A., Zorin, D., and Panozzo, D. (2018). Tetrahedral meshing in the wild. ACM Transactions on Graphics, 37(4), Article 60.

Hu, Y., Schneider, T., Wang, B., Zorin, D., and Panozzo, D. (2020). Fast tetrahedral meshing in the wild. ACM Transactions on Graphics, 39(4), Article 117.

Jacobson, A., Kavan, L., and Sorkine-Hornung, O. (2013). Robust inside-outside segmentation using generalized winding numbers. ACM Transactions on Graphics, 32(4), Article 33.

Ju, T., Losasso, F., Schaefer, S., and Warren, J. (2002). Dual contouring of hermite data. ACM Transactions on Graphics, 21(3), 339-346.

Kobbelt, L. P., Botsch, M., Schwanecke, U., and Seidel, H.-P. (2001). Feature sensitive surface extraction from volume data. Proceedings of SIGGRAPH 2001, 57-66.

Loriot, S., Mesmoudi, M., Rouxel-Labbe, M., et al. CGAL Polygon Mesh Processing: Corefinement and Boolean operations. CGAL User and Reference Manual.

Museth, K. (2013). VDB: High-resolution sparse volumes with dynamic topology. ACM Transactions on Graphics, 32(3), Article 27.

nTopology, Inc. (nTop). Implicit modeling for engineering design; Field-driven design (technical material).

Park, J. J., Florence, P., Straub, J., Newcombe, R., and Lovegrove, S. (2019). DeepSDF: Learning continuous signed distance functions for shape representation. CVPR 2019, 165-174.

Pasko, A., Adzhiev, V., Sourin, A., and Savchenko, V. (1995). Function representation in geometric modeling. The Visual Computer, 11(8), 429-446.

Plantinga, S., and Vegter, G. (2004). Isotopic approximation of implicit curves and surfaces. Eurographics/ACM SIGGRAPH Symposium on Geometry Processing, 245-254.

Rvachev, V. L. (1982). Theory of R-functions and Some Applications. Naukova Dumka (in Russian).

Shapiro, V. (2007). Semi-analytic geometry with R-functions. Acta Numerica, 16, 239-303.

Siemens Digital Industries Software. Parasolid with Convergent Modeling. PLM Components technical material.

Tao, M., Batty, C., Ben-Chen, M., Fiume, E., and Levin, D. I. W. (2019). Mandoline: Robust cut-cell generation for arbitrary triangle meshes. ACM Transactions on Graphics, 38(6).

Trettner, P., Nehring-Wirxel, J., and Kobbelt, L. (2022). EMBER: Exact mesh booleans via efficient and robust local arrangements. ACM Transactions on Graphics, 41(4), Article 163.

Zhou, Q., Grinspun, E., Zorin, D., and Jacobson, A. (2016). Mesh arrangements for solid geometry. ACM Transactions on Graphics, 35(4), Article 39.

Distance-field and trimmed-surface containment computation literature for NURBS B-reps (e.g., arXiv:2510.25159, 2025; survey arXiv:2411.15833, 2024).
