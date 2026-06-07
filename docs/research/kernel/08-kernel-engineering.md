# Kernel Engineering Practice: Spatial Acceleration, Testing Methodology, and API Design

Research review supporting the design of Keel, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition. Kernels do not die from missing features; they die from robustness failures on real-world models. The three engineering disciplines covered here, spatial acceleration, testing methodology, and API design, are the load-bearing structure that determines whether a kernel survives contact with millions of imported parts. This document presents full findings per source: citation, content, limitations, and kernel relevance.

Scope: roughly two dozen sources across three parts. Part 1 covers spatial acceleration structures and their use in B-rep query, intersection, and clash pipelines. Part 2 covers test corpora, differential and metamorphic testing, fuzzing, robustness benchmarks, and numerical reproducibility. Part 3 covers comparative kernel API anatomy (Parasolid PK, ACIS, OCCT), error and attribute models, foreign geometry, and Rust-specific design considerations.

---

## PART 1: Spatial acceleration in kernels

A B-rep kernel does almost nothing without spatial acceleration. Boolean operations need to find which face pairs can possibly intersect before doing exact surface/surface intersection. Selection, ray casting, point classification, minimum distance, and assembly clash all reduce to "find the small set of geometric entities near a query region" out of potentially millions. The structures below are the candidate-generation layer that makes the exact (expensive) geometry layer tractable.

### 1.1 Bounding volume hierarchy variants and choice criteria

#### Source: Gottschalk, Lin, Manocha, "OBBTree: A Hierarchical Structure for Rapid Interference Detection" (SIGGRAPH 1996)

Citation: S. Gottschalk, M. C. Lin, D. Manocha. OBBTree: A Hierarchical Structure for Rapid Interference Detection. Proc. SIGGRAPH 96, pp. 171-180. UNC Technical Report TR96-013. https://www.cs.unc.edu/techreports/96-013.pdf

Content: The foundational paper on oriented bounding box (OBB) trees, the RAPID library. OBBs are fitted by computing the covariance matrix of the geometry (over the convex hull triangles, area-weighted, to avoid bias from dense sampling) and taking its eigenvectors as the box axes; extents come from projecting points onto each axis. Overlap between two OBBs is tested with the separating axis theorem (SAT): two convex polytopes are disjoint iff a separating axis exists, and for two boxes only 15 axes need testing (3 face normals of box A, 3 of box B, and 9 pairwise edge cross products). Each SAT test is under ~200 operations. The key theoretical contribution is that OBBs converge to the underlying surface quadratically with tree depth, whereas AABBs and spheres converge only linearly, so for curved or thin geometry an OBB tree reaches a yes/no answer far higher in the tree. The paper demonstrates exact contact detection on models of hundreds of thousands of polygons at interactive rates.

Limitations: OBB fitting and SAT are heavier per node than AABB tests; the structure is built for rigid polygonal models, so it does not natively address curved B-rep faces or deforming geometry. Refitting an OBB tree after deformation is expensive because the optimal orientation changes. Memory per node is large (orientation matrix plus extents).

Kernel relevance: For Keel, OBB trees are the right tool for narrow-phase rigid clash and minimum-distance between faceted bodies, and the convergence argument justifies using tighter-than-AABB volumes for elongated or curved faces. The covariance-PCA fitting recipe and the 15-axis SAT are directly implementable. For the broad phase and for dynamic edits, however, AABB structures (below) are usually preferred because they refit cheaply.

#### Source: CGAL AABB Tree package (user and reference manual)

Citation: Pierre Alliez, Stephane Tayeb, Camille Wormser et al. CGAL 6.x, "3D Fast Intersection and Distance Computation (AABB Tree)." https://doc.cgal.org/latest/AABB_tree/index.html

Content: CGAL's AABB_tree is a static hierarchy of axis-aligned boxes built over a set of "primitives," where each primitive exposes a datum (the actual geometric object, e.g. a triangle or segment) and an id (a back-reference, e.g. a face handle into a polyhedral surface). This datum/id split is the important design idea: the tree returns both the geometric intersection and the topological identity of what was hit. The tree answers do-intersect, all-intersections, count, first-intersection, closest-point, and squared-distance queries. Distance queries use a hint (a seed point) and KD-tree-accelerated point sampling to prime the search. The tree is built lazily: insertion is cheap and the actual build is deferred until the first query or an explicit build() call.

Limitations: Static structure, no incremental refit; any change to the primitive set forces a full rebuild. AABBs are looser than OBBs for non-axis-aligned geometry. Optimized for meshes/polyhedra rather than trimmed parametric faces directly.

Kernel relevance: The primitive = (datum, id) pattern is exactly what a B-rep kernel needs: store boxes over tessellated faces or curve segments, carry the face/edge handle as the id, and recover topology on a hit. CGAL's lazy-build and hint-seeded distance query are good defaults for Keel's face/edge spatial index. The static limitation motivates the refit-vs-rebuild discussion below for editing workflows.

#### Source: OCCT BVH package and redesigned selection

Citation: Open CASCADE Technology, BVH_Tree class template and "Redesigned selection in OCCT 6.9.0." https://dev.opencascade.org/doc/occt-7.4.0/refman/html/class_b_v_h___tree_base.html and https://dev.opencascade.org/index.php?q=node/1084

Content: OCCT ships a general BVH package (BVH_Box, BVH_Builder, BVH_BinnedBuilder, BVH_SpatialMedianBuilder, BVH_SweepPlaneBuilder, BVH_LinearBuilder) used across ray tracing, collision, frustum culling, and nearest-object search. The notable engineering detail is the selection subsystem: OCCT uses a 3-level BVH (a BVH over objects, plus per-object structures) specifically chosen to minimize rebuilding when only part of the scene changes, an explicit acknowledgement that rebuild cost dominates in interactive editing. Builders are pluggable: binned SAH for quality, spatial-median for speed, sweep-plane for full SAH, linear (Morton-code) for fast parallel construction. Historically OCCT also used a UB-tree (a space-filling-curve-ordered B-tree) for spatial indexing of bounding boxes in some search contexts.

Limitations: Documentation is sparse on quantitative tradeoffs between builders. The multi-level design is shaped by OCCT's specific handle/transient architecture and may not transfer cleanly.

Kernel relevance: The strongest takeaway is the multi-level BVH chosen to localize rebuilds. Keel will face the same interactive-edit pressure, and a tiered structure (assembly-level over body-level over face-level) bounds the rebuild blast radius to the edited body. The pluggable-builder pattern (fast linear build during editing, high-quality SAH build for a finalized model that will be queried heavily) is a clean policy.

#### Source: Wald, Woop, Benthin, Johnson, Ernst, "Embree: A Kernel Framework for Efficient CPU Ray Tracing" (ACM TOG / SIGGRAPH 2014)

Citation: I. Wald, S. Woop, C. Benthin, G. S. Johnson, M. Ernst. Embree: A Kernel Framework for Efficient CPU Ray Tracing. ACM Trans. Graph. 33(4), 2014. https://www.sci.utah.edu/~wald/Publications/2014/embree/embree.pdf

Content: Embree is the reference design for high-performance CPU BVH traversal. Lessons transferable to a geometry kernel: (1) use wide BVHs (BVH4/BVH8, multi-branching nodes) so a single SIMD instruction tests a ray against 4 or 8 child boxes, replacing ray-packet tricks with efficient single-ray traversal; (2) provide multiple specialized builders (high-quality spatial-split SAH for static geometry, fast binned-SAH or Morton/LBVH for dynamic) behind one API; (3) data layout and memory-bandwidth discipline matter more than raw FLOPs, so quantized/compressed node layouts pay off; (4) separate the build policy from the traversal kernel so the same tree serves rays, occlusion, and point queries.

Limitations: Embree targets ray-triangle workloads with coherent-ish streams; B-rep queries (face/face candidate generation, exact surface intersection) have different access patterns. The SIMD-width assumptions are x86-specific. It is a renderer-oriented framework, not a modeling kernel.

Kernel relevance: For Keel, the wide-BVH + SIMD-box-test pattern accelerates ray casting (point-in-solid via ray classification) and face culling directly. The build-policy/traversal separation is a clean architectural seam. Rust SIMD (std::simd / portable_simd) can replicate the 4/8-wide node test. Embree itself can be a vendored dependency for tessellation-level ray queries while Keel owns the exact B-rep layer.

#### Source: R-trees / R*-trees and sphere trees (background, with Quinlan sphere-tree distance)

Citation: A. Guttman, R-Trees: A Dynamic Index Structure for Spatial Searching (SIGMOD 1984); N. Beckmann et al., The R*-tree (SIGMOD 1990); S. Quinlan, Efficient Distance Computation between Non-Convex Objects (ICRA 1994). Summarized via comparative collision/proximity literature.

Content: R-trees and R*-trees are dynamic, disk-oriented bounding-box hierarchies designed for insertion/deletion with balanced fanout; R*-trees improve query performance by forced reinsertion and overlap-minimizing splits. They support incremental update natively, unlike static BVHs. Sphere trees (Quinlan) wrap geometry in nested spheres; sphere overlap and sphere-to-sphere distance are trivially cheap and rotation-invariant, making sphere trees attractive for minimum-distance and deformable self-collision even though spheres are loose fitters.

Limitations: R-trees were designed for database workloads (high fanout, page-aligned) and are not cache-optimal for in-memory geometric traversal. Sphere trees fit poorly to flat or thin geometry, inflating false positives.

Kernel relevance: R*-tree-style dynamic balancing is a model for Keel's persistent, frequently-edited assembly index where bodies are added and removed. Sphere trees' rotation invariance is useful for minimum-distance under rigid motion (the leaf bound does not need refitting when a body only rotates), complementing OBB/AABB narrow phase.

### 1.2 Box filtering and bounding NURBS in intersection pipelines

#### Source: NURBS convex-hull bounds and recentered Bernstein bounds for surface intersection

Citation: Synthesis of NURBS convex-hull property literature: M. E. Mortenson, Geometric Modeling; T. W. Sederberg surface intersection notes; and recentered Bernstein bound results (e.g. P. S. V. Nataraj and others on Bernstein range enclosure). Property reference: https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/spline/NURBS/NURBS-property.html

Content: Every NURBS patch lies inside the convex hull of its (weighted) control points, so a bounding box of the control net is a valid, evaluation-free bound on the surface. This convex-hull property is the cheapest bound but is loose, especially for high-degree patches or skewed weights. Tighter bounds come from converting to Bernstein/Bezier form and using the recentered (or "recentered-and-scaled") Bernstein enclosure, which subtracts the affine part before bounding the residual, dramatically shrinking the range estimate and reducing it quadratically under subdivision. Subdividing a patch (de Casteljau / knot insertion) and re-bounding each piece gives progressively tighter boxes; this drives the classic subdivision-based surface/surface intersection: recursively box-test patch pairs, discard pairs whose boxes are disjoint, subdivide overlapping pairs until flat, then solve.

Limitations: Convex-hull boxes over-estimate badly for surfaces that curve away from their control net; naive interval arithmetic accumulates conservative error. Recentered Bernstein bounds need conversion to Bezier and are more expensive to compute per node. Subdivision can fan out combinatorially on near-tangent surfaces.

Kernel relevance: This is the heart of Keel's intersection candidate generation. The doctrine: cheap control-hull AABB for the first cull, then recentered Bernstein (or refined subdivision) bounds for the surviving pairs, feeding only the genuinely overlapping patch pairs into the exact (Newton/marching) SSI solver. Carrying these bounds in the same BVH used for selection unifies the spatial layer.

#### Source: Box filtering for face-pair candidate generation in boolean pipelines

Citation: Synthesis from OCCT Boolean Operations algorithm documentation and clash literature (see PK_TOPOL_clash and BIM clash sources below).

Content: Boolean and general intersection algorithms begin with a "box filter": compute an AABB per face (and per edge), build/query a BVH, and emit only the face-pair (and edge-face) candidates whose boxes overlap. This converts a naive O(n*m) all-pairs surface intersection into roughly O((n+m) log + k) where k is the small number of real candidates. Tolerance must be baked into the boxes (inflate by the modeling tolerance) so that near-coincident faces are not culled. The same filter feeds imprint, section, and clash.

Limitations: Box overlap is necessary but not sufficient; many candidate pairs survive the filter and still do not actually intersect, so the exact layer must be robust to false candidates. Tolerance inflation widens the candidate set.

Kernel relevance: Keel's boolean engine should treat box filtering as a mandatory first stage with tolerance-inflated AABBs, sharing the per-face BVH with selection and clash. The candidate set, not the raw surfaces, is the unit of work handed to exact SSI.

### 1.3 Incremental updates: refit vs rebuild and lazy build

#### Source: Kopta et al. / Bittner et al. line of work on BVH refit, tree rotations, and incremental update

Citation: D. Kopta et al., Fast, Effective BVH Updates for Animated Scenes (I3D 2012); J. Bittner et al., Fast Insertion-Based Optimization of BVHs; survey via "Fast, Effective BVH Updates" and dynamic-BVH literature. https://www.researchgate.net/publication/254007711_Fast_Effective_BVH_Updates_for_Animated_Scenes

Content: Refitting updates leaf boxes and propagates enlarged boxes up to the root in O(n) without changing topology; it is fast but quality degrades rapidly when geometry moves or deforms significantly, because the original split planes become poor. Rebuilding restores quality but costs a full build each time. The middle path is incremental restructuring: tree rotations during refit locally swap subtrees to undo SAH degradation, and insertion-based optimization re-inserts the worst nodes. The practical pattern for dynamic systems is fast incremental insertion (place a new leaf where it minimizes parent surface-area growth) plus periodic full rebuild when accumulated SAH cost crosses a threshold.

Limitations: Refit alone is unsafe for large topology changes (booleans that delete/create many faces). Rotation/reinsertion heuristics add code complexity and have workload-dependent thresholds.

Kernel relevance: Keel edits are local most of the time (fillet one edge, move one face), so refit + rotations on the edited body, with the assembly-level index untouched, matches the OCCT tiered design. After a topology-changing boolean, rebuild only the affected body's face BVH. A monitored SAH-degradation metric triggers full rebuild. Lazy build (CGAL-style: defer construction to first query) avoids building indices for bodies that are created and immediately deleted in a feature script.

### 1.4 Clash, interference, and minimum-distance at assembly scale

#### Source: Parasolid PK_TOPOL_clash interference detection

Citation: Siemens Parasolid, PK_TOPOL_clash function reference. http://www.q-solid.com/Parasolid_Docs_V35/headers/pk_topol_clash.html

Content: PK_TOPOL_clash takes sets of topologies (bodies, faces, edges) and returns the pairs that clash. Parasolid defines interference precisely: two entities interfere if a point exists that lies within (not merely touching) the bounding topologies of both; two solids interfere if they share common volume, two faces if they share common area, and so on. Crucially it distinguishes touching/coincidence from true overlap, which is the semantically correct definition for a tolerant kernel. The function is the production reference for an industrial-strength clash API: it classifies the kind of clash, not just a boolean yes/no.

Limitations: Public docs do not expose the internal acceleration structure or scaling behavior; it is the API contract, not the implementation. Behavior at exact tangency depends on tolerance settings.

Kernel relevance: This is the API target for Keel's clash function: take topology sets, return classified clashing pairs (volume overlap vs area overlap vs touching) with the topological identity of each pair. The within-not-touching definition is the correct default semantics and must be tolerance-aware.

#### Source: Large-assembly clash culling (BIM/clash-detection practice)

Citation: Spatial, "What is clash detection"; Designing Buildings Wiki, "Clash Detection in 3D BIM Models." https://www.spatial.com/glossary/what-is-clash-detection and https://www.designingbuildings.co.uk/wiki/Clash_Detection_in_3D_BIM_Models

Content: At assembly/BIM scale, checking every object pair precisely is infeasible, so the universal first step is a bounding-box broad phase: if two elements' AABBs do not overlap, reject the pair immediately; reserve detailed geometric intersection and clearance-zone analysis for the surviving candidates. Clearance/soft clashes (within a tolerance distance but not touching) are handled by inflating boxes by the clearance threshold. This is the same box-filter principle as booleans, applied at the assembly graph level.

Limitations: This is industry/practitioner literature, light on algorithmic specifics. Broad-phase box overlap still admits many false positives for interleaved complex parts.

Kernel relevance: Keel's assembly clash should run a two-tier broad phase (assembly BVH of body boxes, then per-body face BVH) before any exact test, with clearance handled by box inflation. This dovetails with the tiered spatial index recommended above.

#### Source: PQP, sphere trees (Quinlan), and GJK for minimum-distance narrow phase

Citation: E. Larsen, S. Gottschalk, M. C. Lin, D. Manocha, PQP (Proximity Query Package) and Fast Proximity Queries with Swept Sphere Volumes; S. Quinlan, Efficient Distance Computation (ICRA 1994); E. Gilbert, D. Johnson, S. Keerthi, GJK distance algorithm (1988). https://gamma.web.unc.edu/research/collision/packages/ and https://github.com/GammaUNC/PQP

Content: PQP performs three query types: collision (do they overlap), distance (shortest translation to contact), and tolerance (closer/farther than a threshold). It uses OBB trees for collision but rectangle-swept-sphere (RSS) bounding volumes for distance and tolerance, because RSS gives tight, cheaply-computed distance lower bounds that prune the BVH descent effectively. Quinlan's sphere trees give the same idea with spheres: sphere-to-sphere distance is a single subtraction, ideal for pruning and rotation-invariant. GJK computes exact minimum distance between two convex pieces by iterating on the Minkowski difference simplex; it is the standard narrow-phase distance primitive once geometry is decomposed into convex pieces.

Limitations: PQP/RSS operate on triangle meshes, not curved B-rep faces; results are tessellation-accurate, not exact. GJK requires convex inputs, so non-convex B-rep faces need convex decomposition or per-patch treatment. Penetration depth (vs separation distance) needs EPA or other extensions.

Kernel relevance: For Keel's minimum-distance and tolerance queries, the doctrine is: BVH with distance-tight leaf bounds (RSS or spheres) for pruning, then GJK on convex tessellated pieces for the narrow-phase number, optionally refined to exact distance by Newton iteration on the underlying surfaces for the final answer. This layered design (mesh prune, convex GJK, exact refine) gives both speed and exactness.

---

## PART 2: Testing methodology for geometry kernels

A geometry kernel has no closed-form oracle: there is rarely a "correct answer" to compare against, and floating-point makes bitwise reproducibility nontrivial. Testing therefore relies on large real-world corpora, differential comparison against reference kernels, metamorphic invariants, fuzzing, and disciplined numerical reproducibility. The literature below provides both the corpora and the methodology.

### 2.1 Test corpora

#### Source: Koch et al., "ABC: A Big CAD Model Dataset for Geometric Deep Learning" (CVPR 2019)

Citation: S. Koch, A. Matveev, Z. Jiang, F. Williams, A. Artemov, E. Burnaev, M. Alexa, D. Zorin, D. Panozzo. ABC: A Big CAD Model Dataset for Geometric Deep Learning. CVPR 2019. arXiv:1812.06216. https://deep-geometry.github.io/abc-dataset/

Content: ABC is over one million CAD models collected from Onshape over four months, each as a STEP file with explicit parametric curves and surfaces (B-rep). It provides ground-truth differential quantities, patch segmentation, and feature labels. Most models are mechanical parts with sharp edges and well-defined surfaces. The authors processed all models through Open CASCADE, which is itself an implicit large-scale stress test of a kernel's STEP reader and topology builder. The dataset is downloadable in chunks and ships with tooling.

Limitations: License varies per model (Onshape user uploads); not all models are clean (some have invalid topology, self-intersections, tiny faces). It is curated for deep learning, not regression testing, so there are no golden outputs for boolean or fillet operations. Heavy skew toward mechanical parts.

Kernel relevance: ABC is the single most valuable corpus for Keel's STEP import and B-rep validity regression suite: a million real parts will exercise edge cases no hand-written test ever will. Run import + validity-check + round-trip export across the corpus and track the pass rate as the primary robustness metric. The "OCCT successfully processed N of 1M" framing is the model for a kernel acceptance benchmark.

#### Source: Willis et al., "Fusion 360 Gallery Dataset" and MFCAD/MFCAD++

Citation: K. D. D. Willis et al., Fusion 360 Gallery: A Dataset and Environment for Programmatic CAD Construction from Human Design Sequences (ACM TOG 2021). https://github.com/AutodeskAILab/Fusion360GalleryDataset . MFCAD: Cao et al.; MFCAD++ extensions for machining-feature recognition.

Content: The Fusion 360 Gallery dataset provides ~8.6K human-authored CAD construction sequences (sketch+extrude reconstruction subset) and a segmentation subset of 35,858 B-rep bodies (~390K faces), each face labeled by the modeling operation that created it (extrude, cut, fillet, chamfer, revolve). MFCAD and MFCAD++ supply B-rep models with per-face machining-feature labels for supervised recognition. The construction sequences are uniquely valuable because they give the operation history, not just the final solid.

Limitations: Smaller than ABC; reconstruction subset is deliberately limited to sketch+extrude to bound complexity. Labels target ML feature recognition, not kernel correctness. Autodesk research license terms apply.

Kernel relevance: The construction sequences let Keel replay real feature scripts (sketch, extrude, fillet, boolean) as integration tests, exercising the modeling API end-to-end rather than just import. Per-face operation labels are a ready-made oracle for testing persistent naming and attribute propagation: after a boolean, faces should retain consistent identity/labels.

#### Source: Zhou and Jacobson, "Thingi10K: A Dataset of 10,000 3D-Printing Models" (2016)

Citation: Q. Zhou, A. Jacobson. Thingi10K: A Dataset of 10,000 3D-Printing Models. arXiv:1605.04797. https://arxiv.org/abs/1605.04797

Content: Thingi10K is 10,000 real triangle-mesh models from Thingiverse, deliberately curated to capture in-the-wild pathologies: self-intersections, non-manifold edges, degenerate triangles, open boundaries, duplicated faces, and wildly varying complexity. The thesis is that algorithms must be validated on data "representative of inputs encountered in the wild," not on clean synthetic meshes. Distributed via Python package and several mirrors.

Limitations: Mesh side only (triangle soup), not B-rep; no parametric surfaces. Tests the meshing/repair/tessellation parts of a kernel, not exact surface intersection. Models are 3D-printing oriented (toys, mechanisms), not strictly mechanical CAD.

Kernel relevance: Thingi10K is the corpus for Keel's mesh-import, mesh-repair, and tessellation robustness, and for any mesh-Boolean or mesh-to-B-rep path. More importantly it is the methodological template (see fTetWild below): a 10K-model "does it crash / does it succeed" survey is the gold standard for reporting kernel robustness.

#### Source: STEP file zoos and interoperability corpora (CAx-IF, NIST)

Citation: CAx Interoperability Forum (CAx-IF) test suites; NIST STEP File Analyzer test models. Summarized via STEP-format interoperability literature.

Content: Standards bodies (CAx-IF, NIST) publish curated STEP files specifically to test reader/writer conformance and interoperability across vendors, including tricky representation items, tolerances, assemblies, and validation properties (geometric volume/area/centroid stored in the file for cross-check).

Limitations: Smaller, conformance-focused; not a stress corpus. Coverage of exotic geometry is uneven.

Kernel relevance: STEP validation properties (stored volume, area, centroid) are a built-in oracle: import a part, compute mass properties, and compare against the values the authoring system wrote into the file. This is a cheap, vendor-neutral correctness check for Keel's importer and mass-property code.

### 2.2 Differential testing and the oracle problem

#### Source: Differential testing methodology and shape-equality comparison

Citation: Synthesis of differential-testing literature (e.g. McKeeman, Differential Testing for Software, 1998) applied to geometry, plus shape-comparison metrics below. Cross-backend discrepancy thresholding described in fuzzing/differential-testing surveys.

Content: Because geometry has no analytic oracle, the dominant correctness strategy is differential testing: run the same operation through two independent implementations (e.g. Keel vs OCCT) and flag disagreements above a threshold. Discrepancies are measured numerically (e.g. Euclidean/Hausdorff distance between results) and only differences exceeding a tolerance count as bugs. This sidesteps the oracle problem by treating a mature reference kernel as an approximate oracle.

Limitations: A reference kernel is not ground truth; it has its own bugs and tolerance behavior, so a disagreement may indicate either kernel is wrong, and both can be wrong identically. Triage is labor-intensive. Tolerance choice for "same result" is delicate.

Kernel relevance: Keel should run booleans, offsets, fillets, and intersections differentially against OCCT (open source, scriptable) over the ABC corpus, comparing results with the shape-distance metrics below. Disagreements become triage tickets. This is the most practical correctness net for an early-stage kernel.

#### Source: Shape-distance metrics: Metro/Hausdorff and mass-property comparison

Citation: P. Cignoni, C. Rocchini, R. Scopigno, Metro: Measuring Error on Simplified Surfaces (Computer Graphics Forum 1998); Hausdorff-distance mesh literature (e.g. precise Hausdorff between polygonal meshes). https://www.researchgate.net/publication/222396707_Precise_Hausdorff_distance_computation_between_polygonal_meshes

Content: To decide whether two B-reps are "the same shape" despite different topology/parameterization, the practical metrics are: (1) symmetric Hausdorff distance between tessellations (max of forward and backward nearest-point distances), computed by surface sampling as in Metro, giving max and mean error; (2) mass-property comparison (volume, surface area, centroid, inertia), which is cheap, rotation/parameterization-invariant, and catches gross errors immediately; (3) sampling-based mean distance for tolerance-aware "close enough" judgments. Hausdorff is sensitive to outliers (a single spike dominates), so mean distance and mass properties complement it.

Limitations: Tessellation-based, so accuracy is bounded by sampling density; can miss small features. Mass properties can coincide for genuinely different shapes (false equality). Hausdorff outlier sensitivity cuts both ways.

Kernel relevance: Keel's "same shape" oracle should be a tiered comparator: first compare mass properties (fast reject), then symmetric sampled Hausdorff (max + mean), with explicit tolerances. This comparator is the backbone of both differential testing and metamorphic round-trip checks.

### 2.3 Metamorphic and property-based testing for geometry

#### Source: Metamorphic testing (Chen et al.) and property-based testing for invariants

Citation: T. Y. Chen, S. C. Cheung, S. M. Yiu, Metamorphic Testing: A New Approach for Generating Next Test Cases (1998); Wikipedia and surveys on metamorphic testing applied to graphics/scientific software. https://en.wikipedia.org/wiki/Metamorphic_testing . Property-based testing via QuickCheck (Claessen and Hughes 2000) and Rust's proptest.

Content: Metamorphic testing solves the oracle problem by checking metamorphic relations (MRs): properties that must hold between the outputs of related inputs, without knowing the correct output of either. For a geometry kernel the natural MRs are: rigid-transform invariance (rotate/translate inputs, results transform identically and mass properties are unchanged); scaling invariance (scale by s, volume scales by s^3, area by s^2); boolean round-trips (A union B then subtract B should recover A within tolerance; A minus B union (A intersect B) recovers A); idempotence (A union A = A, A intersect A = A); commutativity (A union B = B union A as shapes); and De Morgan-style identities. Property-based frameworks (QuickCheck, Rust proptest) generate randomized inputs and shrink failing cases to minimal reproducers, which pairs naturally with these MRs and with geometric predicates (orientation, in-circle) whose properties (antisymmetry under point swap) are checkable.

Limitations: MRs are necessary but not sufficient; passing all known MRs does not prove correctness. Round-trips can hide compensating errors. Tolerance in the comparison can mask real drift. Generating valid random solids that are interesting (not degenerate) is itself hard.

Kernel relevance: This is arguably the highest-leverage testing technique for Keel. Encode transform/scale invariance, boolean round-trips, and idempotence as proptest properties over randomly generated and corpus-sourced solids, using the tiered shape comparator as the relation check. Proptest shrinking turns a sprawling failure into a minimal failing solid, which is invaluable for debugging a kernel. Geometric predicates get their own property tests (sign flips under argument permutation).

### 2.4 Fuzzing geometric and CAD code

#### Source: libFuzzer / AFL coverage-guided fuzzing of importers and predicates

Citation: LLVM libFuzzer documentation; AFL/AFL++; OSS-Fuzz methodology. https://llvm.org/docs/LibFuzzer.html

Content: Coverage-guided in-process fuzzers (libFuzzer, AFL++) link against the target, feed mutated inputs through one entry point, track edge coverage, and evolve inputs to maximize coverage, catching crashes, asserts, sanitizer violations (ASan/UBSan), and FP exceptions. The highest-value fuzz targets for a kernel are the file importers (STEP, IGES, STL, OBJ), which parse untrusted external data and are the classic source of memory-safety and crash bugs. Structured/grammar-aware fuzzing (or starting from a seed corpus of real STEP files plus a dictionary of STEP tokens) reaches deeper than blind byte mutation.

Limitations: Fuzzing finds crashes and memory errors, not semantic geometric wrongness (a boolean that returns a subtly wrong but valid solid will not crash). Coverage-guided fuzzing struggles to satisfy strict format checksums/grammar without structure awareness. Requires sanitizer builds and CI compute.

Kernel relevance: Rust gives Keel memory safety in safe code, but importers often use unsafe parsing, integer arithmetic, and any C/C++ dependencies (OCCT, Embree) remain attack surface. cargo-fuzz (libFuzzer) over the STEP/STL importers, seeded with the ABC and Thingi10K corpora, is a cheap continuous crash net. Pair with UBSan for any FFI.

#### Source: OpenCASCADE fuzzing and importer robustness reports

Citation: Public OCCT issue tracker and security-advisory discussions on STEP/IGES importer crashes; differential-testing-of-importers methodology (general). Summarized from fuzzing and differential-testing surveys.

Content: OCCT's data-exchange importers have historically been a source of crashes and out-of-bounds reads when fed malformed STEP/IGES, a well-known pattern: industrial CAD readers are large, old, C++ and were not written defensively against adversarial input. The general lesson reported across importer-fuzzing efforts is that the parser layer (tokenization, entity-reference resolution) fails long before the geometry layer.

Limitations: Specific public OCCT fuzzing writeups are scattered and version-specific; details vary by release.

Kernel relevance: Confirms that Keel must treat importers as a hardened, untrusted-input boundary, distinct from the trusted internal geometry layer. Validate all entity references, bound all allocations from file-declared counts, and never trust a length field. Fuzz this boundary continuously.

### 2.5 Robustness benchmarks as a methodology model

#### Source: Hu et al., "Fast Tetrahedral Meshing in the Wild" (fTetWild) (ACM TOG / SIGGRAPH 2020)

Citation: Y. Hu, T. Schneider, B. Wang, D. Zorin, D. Panozzo. Fast Tetrahedral Meshing in the Wild. ACM Trans. Graph. 39(4), 2020. arXiv:1908.03581. https://arxiv.org/abs/1908.03581

Content: fTetWild is the methodological exemplar for kernel robustness reporting. The authors run their meshing algorithm over the entire Thingi10K dataset (10,000 in-the-wild, pathology-laden meshes) and report concrete robustness numbers: 98.7% of inputs meshed in under 2 minutes, all models processed within 32 minutes, zero crashes. The key engineering idea is maintaining a valid floating-point mesh at every algorithmic stage (interleaving triangle insertion with optimization) instead of TetWild's expensive exact-rational construction, trading the guarantee of preserving every input triangle for full floating-point robustness and speed. The "report a pass rate over a 10K wild corpus" framing is now the de facto standard for claiming a geometric algorithm is robust.

Limitations: Meshing, not B-rep modeling; the specific algorithm does not transfer. "98.7%" still means ~130 failures, and the relaxation (not all input triangles preserved) is a real semantic concession.

Kernel relevance: This is the benchmark methodology Keel should adopt: define a wild corpus (ABC for B-rep, Thingi10K for mesh), run the operation under test across all of it, and report a single headline robustness number (e.g. "boolean succeeds and produces a valid solid on X% of N pairs") that goes up over time. The "always keep a valid representation" design discipline (never enter an unrepresentable intermediate state) is directly applicable to Keel's tolerant boolean engine.

### 2.6 Numerical regression and reproducibility

#### Source: Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates"

Citation: J. R. Shewchuk. Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates. Discrete & Computational Geometry 18:305-363, 1997. https://people.eecs.berkeley.edu/~jrs/papers/robust-predicates.pdf

Content: Shewchuk's predicates (orient2d/3d, incircle/insphere) compute the exact sign of geometric determinants using adaptive multi-stage evaluation: a fast floating-point estimate with an error bound; if the estimate is safely far from zero, return immediately; otherwise progressively add precision (expansion arithmetic) until the sign is certain. This yields exact, deterministic predicate signs at near-floating-point speed in the common case, eliminating the inconsistent-orientation bugs that corrupt mesh/Boolean topology. Companion ports exist (georust/robust in Rust).

Limitations: Covers specific polynomial predicates (orientation, in-circle/sphere), not arbitrary curved-surface intersections. Expansion arithmetic is verbose and must be generated carefully. Does not by itself give exact NURBS intersection.

Kernel relevance: Keel's topological decisions (point-side-of-plane, orientation, in-circle for triangulation) should use exact adaptive predicates rather than tolerance-compared floats, because a single inconsistent sign can produce non-manifold output. The Rust georust/robust crate is a ready port. This is the bedrock of robust topology.

#### Source: FMA/compiler-induced non-reproducibility and FLiT cross-platform testing

Citation: Bartels and Fortune, Fast Floating-Point Filters for Robust Predicates (arXiv:2208.00497); G. Sawaya et al., FLiT: Cross-Platform Floating-Point Result-Consistency Tester (IISWC 2017). https://pruners.github.io/pdf/iiswc2017-final43.pdf

Content: Documents a concrete, dangerous failure mode: compilers may fuse a*b + c into a single FMA instruction, which has different rounding than separate multiply-then-add. For an orientation predicate this can break anticommutativity, so swapping two input points fails to flip the sign, corrupting robustness. The same source code thus gives different results across compilers, flags (-ffast-math, -mfma), and CPUs. FLiT is a tool to detect such cross-platform/cross-compiler floating-point divergence systematically by running a workload across configurations and diffing results.

Limitations: Suppressing FMA can cost performance; full bitwise cross-platform reproducibility may require disabling beneficial optimizations. FLiT is a testing tool, not a fix.

Kernel relevance: For deterministic geometry tests and reproducible regression baselines, Keel must control FP contraction. In Rust this means avoiding fast-math-style transformations, being explicit with mul_add only where intended, and being aware that LLVM codegen differs across targets. Predicate code in particular must be compiled so FMA does not silently break sign properties; consider a FLiT-style cross-platform consistency check in CI to catch divergence before it becomes a "works on my machine" topology bug.

#### Source: CI strategy and performance-regression tracking for long-running geometry suites

Citation: Synthesis of CI practice (sharded test execution, nightly full-corpus runs, performance baselines) drawn from the dataset/benchmark sources above and standard continuous-integration methodology.

Content: Geometry suites split into tiers: a fast per-commit unit/property suite (seconds to minutes), and nightly/weekly full-corpus differential and fuzz runs (hours over ABC/Thingi10K) that gate releases rather than commits. Performance regressions are caught by recording timing and memory baselines per operation and alerting on statistically significant deltas, since a kernel that gets correct answers 3x slower is a regression. Crash corpora found by fuzzing are minimized and added to the fast suite as permanent regression tests.

Limitations: Full-corpus runs are compute-heavy and slow to give feedback; flaky FP comparisons can produce noisy failures if tolerances are mis-set.

Kernel relevance: Keel should adopt tiered CI: fast proptest + unit + minimized-crash corpus per commit; nightly differential-vs-OCCT and cargo-fuzz over ABC/Thingi10K; per-operation performance baselines with regression alerts. Every fuzz/triage find becomes a permanent fast-suite regression test.

---

## PART 3: Kernel API design study

API design decides whether a kernel is usable, embeddable, and survivable across decades of binary-compatibility and save/load evolution. The three reference kernels embody three different philosophies: Parasolid's tag-based C API, ACIS's C++ ENTITY object model, and OCCT's handle-based C++ split. Keel, in Rust, must choose deliberately among these for ownership, references, error handling, and extensibility.

### 3.1 Comparative anatomy

#### Source: Parasolid PK architecture (introduction and concepts)

Citation: Siemens Parasolid documentation, "Introduction to Parasolid" and "Parasolid Concepts." http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.002.html and .../fd_chap.003.html

Content: Parasolid exposes a C API (the PK, "partitioned kernel") over an opaque kernel. Objects (body, region, shell, face, loop, edge, vertex, plus geometry: surfaces, curves, points) are referenced by integer tags, not pointers; the application never holds a raw pointer into kernel memory, which is what makes save/load and rollback safe. Every object has one or more classes (tokens defined in parasolid_kernel.h). The application provides three callback interfaces called from inside the kernel: the frustrum (memory allocation, file I/O, rollback storage), graphical output (GO) for rendering, and foreign geometry for user-defined surfaces/curves. Parasolid supports a typed attribute system for attaching user data to objects, and session/partitioned rollback (delta states) for undo and error recovery. Error handling is via returned error codes plus a severity model.

Limitations: C API loses type safety and RAII; tags require a lookup indirection on every access. The frustrum callback model pushes memory/IO responsibility onto the host. Public docs describe the contract, not internals.

Kernel relevance: The tag/handle model is the single most important lesson for Keel: stable integer/handle identifiers, never raw pointers, across save/load and rollback. The frustrum pattern (host provides allocation, IO, and undo storage) maps naturally to Rust traits the host implements. The typed attribute system and delta-state rollback are direct design targets. Foreign geometry (below) is a distinguishing capability worth replicating.

#### Source: 3D ACIS Modeler ENTITY hierarchy, api_ functions, bulletin board

Citation: Spatial Corp, 3D ACIS Modeler documentation; ACIS Kernel reference (R10 function chapters); "3D Modelling with the ACIS Kernel and Toolkit." https://www.spatial.com/solutions/3d-modeling/3d-acis-modeler and http://www-isl.ece.arizona.edu/ACIS-docs/

Content: ACIS is a C++ object model: the core topological entities (BODY, LUMP, SHELL, FACE, LOOP, WIRE, EDGE, COEDGE, VERTEX) all derive from a common ENTITY base, with geometry entities (SURFACE, CURVE, etc.) alongside. The application calls api_ functions (e.g. api_solid_block, api_boolean) that operate on these entities. ACIS attaches user data via derived ATTRIB classes hung off entities. The defining mechanism is the bulletin board / history stream / delta state: every modeling operation records bulletins of created/modified/deleted entities, enabling roll (undo/redo) by replaying or reverting deltas; api_start_bulletin_board / api_note_state / roll bracket operations. Save/load serializes the entity graph.

Limitations: Exposing C++ objects across the API ties clients to ABI/compiler compatibility and the C++ object layout, making binary compatibility fragile across versions and compilers. Pointer-based entity references complicate save/load and concurrency. The ENTITY hierarchy is heavyweight.

Kernel relevance: The bulletin-board/delta-state history model is the canonical undo/redo architecture and a strong template for Keel's edit history. But ACIS also illustrates the cost of exposing the C++ object graph directly: Keel should prefer Parasolid-style handles at the API boundary to preserve binary compatibility, even if it uses a rich type model internally. The ATTRIB-on-entity pattern informs the attribute system.

#### Source: OCCT TopoDS/Geom split, handles, BRepBuilderAPI

Citation: Open CASCADE Technology, "Modeling Data" overview, Standard_Transient and Handle documentation, BRepBuilderAPI. https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html and https://dev.opencascade.org/doc/refman/html/class_standard___transient.html

Content: OCCT splits topology (TopoDS_Shape and subclasses: TopoDS_Vertex/Edge/Wire/Face/Shell/Solid) from geometry (Geom_Surface, Geom_Curve, etc.). TopoDS_Shape is a lightweight handle holding a reference-counted pointer (myTShape, a Handle to a TopoDS_TShape subclass of Standard_Transient); shapes are cheap to copy and share underlying data. Reference-counted Handles (Handle(Standard_Transient)) provide automatic memory management for geometry and many other classes. Construction goes through BRepBuilderAPI_* algorithm classes (e.g. BRepBuilderAPI_MakeEdge, MakeFace, Transform), separating data structures (TopoDS) from construction algorithms (BRepBuilderAPI). Documented caveats note that the Handle reference-count increment/decrement is real overhead in tight loops over many objects.

Limitations: Handle ref-counting overhead is measurable in hot loops; the topology/geometry split adds indirection; the Handle macro machinery is idiosyncratic and pre-dates modern smart pointers. C++ ABI constraints affect binary compatibility.

Kernel relevance: The topology/geometry separation is a clean design Keel should mirror: topological entities carry adjacency and reference shared, reference-counted geometry. In Rust this maps to lightweight topology handles indexing into geometry stored in an arena/object-store (Rc/Arc or generational-index handles), getting the sharing benefit without C++ Handle overhead. The MakeX builder pattern (algorithm objects that report success/failure and yield results) is a good API ergonomic.

### 3.2 References, ownership, and binary compatibility

#### Source: Handle/ID vs pointer references and versioning practice (synthesis)

Citation: Synthesis of Parasolid tag model, ACIS pointer/entity model, OCCT Handle model, and binary-compatibility practice from the three kernels' documentation.

Content: The cross-kernel lesson on references: pointer-based references (ACIS) are fast but fragile across save/load, concurrency, and ABI changes; handle/tag-based references (Parasolid) add an indirection but are stable, serializable, and decouple the client from internal memory layout, which is essential for forward/backward binary compatibility and for persistent files that must reopen in future versions. A C API (Parasolid) maximizes binary compatibility and language interop at the cost of type safety and RAII; a C++ object API (ACIS, OCCT) gives ergonomics and type safety but ties clients to compiler/ABI and complicates versioning.

Limitations: This is a synthesized comparison; exact versioning policies of commercial kernels are not fully public.

Kernel relevance: Keel should expose stable handles (generational indices into an object store) at its public API, keep raw references internal, and define a serialization format whose persistent IDs survive across versions. A C-ABI (or stable Rust + C header) boundary maximizes embeddability. Internally, Rust's ownership lets the object store own all entities while handles act as safe, checkable references (the Fornjot validation lesson below).

### 3.3 Attribute and user-data systems, persistent naming

#### Source: Persistent naming and attribute callbacks on split/merge/delete

Citation: Persistent-naming literature: J. Kripac, A Mechanism for Persistently Naming Topological Entities in History-Based Parametric Solid Models (1997); Y. Wu et al., Mechanisms of Persistent Identification of Topological Entities in CAD Systems: A Review (Alexandria Eng. J. 2018); name-matching with merge/split history. https://www.sciencedirect.com/science/article/pii/S1110016818300814

Content: Parametric/feature-based modeling requires that an entity referenced by a feature (e.g. "fillet this edge") survive regeneration even though the underlying B-rep is rebuilt. The persistent-naming problem splits into naming (assign a stable, regeneration-invariant name based on generating features and entity properties) and name matching (re-find the named entity after edits). The core difficulty is topology splitting and merging: a face split in two by a later feature, or two faces merged, creates ambiguity about which child inherits the name. Solutions track the split/merge history and use local neighborhood matching. The kernel-level hook is attribute callbacks: when the kernel splits, merges, or deletes an entity during an operation, it must notify attached attributes so they can propagate, duplicate, or invalidate.

Limitations: No fully general solution exists; all approaches have ambiguous cases. History-tracking adds bookkeeping overhead. Cross-system name exchange (neutral formats) remains unsolved in general.

Kernel relevance: This is a make-or-break design decision that must be designed in from the start, not retrofitted. Keel's attribute system needs split/merge/delete callbacks (as Parasolid and ACIS provide) so that user attributes and persistent names propagate correctly through booleans and feature regeneration. The attribute system and the spatial/topology operations are coupled: every operation that creates/destroys topology must drive attribute propagation.

#### Source: Typed attribute systems and schema evolution (synthesis from Parasolid/ACIS)

Citation: Parasolid attribute documentation and ACIS ATTRIB class system (as cited above).

Content: Both Parasolid and ACIS provide typed, user-extensible attributes attached to any entity, with the kernel managing their lifecycle through modeling operations and serialization. Schema evolution (changing attribute definitions across software versions) is handled by versioned attribute definitions so old files still load.

Limitations: Typed attribute schemas must be registered/declared; evolving them without breaking old files needs explicit versioning discipline.

Kernel relevance: Keel should offer typed attributes (Rust enums/traits for attribute kinds) with explicit version tags for forward/backward file compatibility, and define the propagation policy (copy, split, merge, delete) per attribute type.

### 3.4 Foreign geometry

#### Source: Parasolid foreign geometry interface

Citation: Siemens Parasolid, foreign geometry interface (described in the Introduction/Concepts chapters cited above).

Content: Parasolid's foreign geometry interface lets an application supply user-defined surfaces and curves: the kernel calls back into application-provided evaluators (point, derivatives, bounds) when it needs to evaluate geometry it does not natively represent. This allows a body to contain non-Parasolid geometry (e.g. a proprietary or legacy surface type) while still participating in modeling. The docs note few applications need it, but it is essential for ingesting legacy data with surface types the kernel does not natively implement.

Limitations: Foreign geometry is slower (callback per evaluation) and limits which exact algorithms can run on it; robustness depends on the quality of the user evaluator. Rarely needed, so it is an advanced feature.

Kernel relevance: A foreign-geometry/evaluator-callback trait future-proofs Keel for legacy and proprietary surfaces: define a Surface/Curve trait the host can implement, and let the kernel evaluate via that trait when it encounters non-native geometry. This is the migration path for importing data whose exact surface math Keel does not yet implement, and it cleanly decouples geometry representation from the topology engine.

### 3.5 Error handling models

#### Source: Error codes, partial success, and severity (synthesis from Parasolid PK and OCCT)

Citation: Parasolid error/severity model (Concepts chapter); OCCT algorithm status reporting (BRepBuilderAPI IsDone / error status); PK_TOPOL_clash classified results.

Content: Industrial kernels favor explicit, returned status over exceptions: Parasolid returns error codes with a severity classification; OCCT algorithm objects report IsDone() plus detailed error/status enums rather than throwing. Critically, robust modeling operations support partial success: a boolean over many faces can return a result flagged with the faces/regions where it failed, rather than aborting wholesale, so the caller gets a usable partial result plus fault tags. Severity classification (warning vs recoverable error vs fatal) lets callers decide whether to proceed.

Limitations: Error-code APIs are verbose and easy to ignore; partial-result semantics complicate the contract (the caller must check fault tags). Mixing partial success with strict validity is subtle.

Kernel relevance: Rust's Result and rich error enums are an ideal fit: Keel should return structured errors with severity, and for booleans/operations support a partial-result type carrying both the produced geometry and the set of fault tags (which face pairs failed, why). This is more honest than all-or-nothing and matches how real CAD operations on dirty data behave. Never panic across the API boundary; map every internal failure to a typed error.

### 3.6 Rust-specific API considerations

#### Source: Fornjot object store, handles, and reference validation (PR #2144, design docs)

Citation: H. Braun et al., Fornjot, early-stage b-rep CAD kernel in Rust; PR #2144 "Object Reference Validation." https://github.com/hannobraun/fornjot and https://github.com/hannobraun/fornjot/pull/2144

Content: Fornjot uses a handle-based object store rather than raw pointers/Rc graphs: geometric objects (HalfEdge, Cycle, Shell) live in a store and are referred to by Handles, which enables declarative validation of ownership (e.g. each HalfEdge belongs to exactly one Cycle) by counting references, something pointer graphs cannot easily check. The project deliberately rejects early-aborting validation: experience showed early aborts leave too little context to diagnose failures, so it runs comprehensive validation and includes both the referenced and referencing objects in error messages. Design philosophy: favor reliability over features, every operation should either work or give a clear, actionable error. Modular crate structure (fj-core, fj-math, fj-interop, fj-export).

Limitations: Early-stage and not feature-complete; the handle-store approach has performance/ergonomics tradeoffs still being worked out; lessons are from a young codebase, not a battle-tested kernel.

Kernel relevance: Directly applicable to Keel. The object-store + generational-handle model is the idiomatic Rust answer to Parasolid tags: it gives stable serializable references, enables reference-count-based structural validation, and sidesteps the borrow-checker pain of cyclic topology graphs (which Rc/RefCell or arena+indices both address, with arena+indices being the cleaner choice). The comprehensive-validation-with-rich-errors stance and the modular crate split are good defaults for Keel.

#### Source: truck (Rust CAD kernel) memory-safety design

Citation: RICOS, truck: a Rust CAD kernel. https://github.com/ricosjp/truck

Content: truck is a Rust CAD kernel whose explicit selling point is using Rust's safety to eliminate the core dumps that plague C++ kernels: no segfaults from CPU-side geometry processing because safe Rust prevents the memory errors that cause them. It is organized as modular crates (truck-geometry, truck-topology, truck-modeling, truck-meshalgo) mirroring the topology/geometry separation.

Limitations: Smaller scope than commercial kernels; less mature boolean/feature support; safety eliminates memory crashes but not logical/robustness bugs in geometry algorithms.

Kernel relevance: Validates the core thesis behind Keel: a Rust kernel converts a large class of C++ kernel crashes into either compile errors or typed Results, raising the robustness floor for free. The crate decomposition (geometry / topology / modeling / mesh) is a proven module boundary to adopt. Safety is necessary but not sufficient: Keel still needs the predicates, tolerance discipline, and testing methodology from Parts 1-2 to be robust in the geometric sense.

---

## Engineering doctrine for Keel

Synthesizing the three parts into actionable design commitments:

1. Tiered spatial index, shared across subsystems. Build one BVH layer used by selection, boolean candidate generation, clash, and ray classification. Tier it: assembly-level (body AABBs) over body-level over face-level, so local edits refit only the affected subtree (the OCCT multi-level lesson). Use AABBs by default (cheap refit), OBB/RSS leaves where distance tightness pays (the OBBTree/PQP lesson), and wide SIMD nodes for ray traversal (the Embree lesson).

2. Box filtering is mandatory before exact geometry. Every boolean, imprint, section, and clash starts with a tolerance-inflated box filter that emits only candidate face/edge pairs. NURBS get a cheap control-hull AABB first, then recentered Bernstein / subdivision bounds for survivors, feeding only genuinely overlapping patch pairs to the exact SSI solver.

3. Refit locally, rebuild on topology change, lazy-build always. Local geometric edits trigger refit plus tree rotations on the edited body; topology-changing booleans rebuild that body's face BVH; an SAH-degradation metric triggers full rebuild; index construction is deferred to first query.

4. Robustness is reported as a corpus pass rate. Adopt the fTetWild methodology: run each operation across ABC (B-rep) and Thingi10K (mesh) and report a headline success-and-validity percentage that must monotonically increase. Always maintain a valid representation at every algorithmic stage.

5. Three-layer correctness net. (a) Exact adaptive predicates (Shewchuk / georust-robust) for all topological sign decisions, with controlled FP contraction so FMA never breaks predicate sign properties. (b) Metamorphic / property tests (proptest) for transform invariance, scaling laws, boolean round-trips, and idempotence, judged by a tiered shape comparator (mass properties, then symmetric sampled Hausdorff). (c) Differential testing against OCCT over the corpus for operations lacking an analytic oracle.

6. Harden importers as an untrusted boundary. Treat STEP/IGES/STL readers as adversarial-input parsers: validate every reference, bound every allocation, never trust a length field, and fuzz continuously with cargo-fuzz seeded from ABC/Thingi10K. This is where crashes live.

7. Handle-based API, never raw pointers. Expose stable generational-index handles (Parasolid tags, Fornjot object store), keep raw references internal, and define a serialization format whose persistent IDs survive version changes. Mirror OCCT's topology/geometry split with lightweight topology handles indexing shared geometry in an arena.

8. Attributes and persistent naming designed in from day one. Provide typed, versioned attributes with split/merge/delete propagation callbacks; build the persistent-naming machinery (history-tracked, neighborhood-matched) into the topology engine, because it cannot be bolted on later.

9. Foreign geometry via evaluator traits. Define Surface/Curve traits the host can implement so legacy/proprietary geometry can be evaluated by callback, decoupling representation from the topology engine and providing a migration path for unimplemented surface types.

10. Honest, structured error handling. Use Rust Result with rich severity-tagged error enums; support partial-success results for booleans (produced geometry plus fault tags per failed face pair); never panic across the API boundary. Lean on Rust's memory safety (truck's thesis) to eliminate the crash class, then add the geometric-robustness discipline above to handle the rest.

11. Tiered CI. Fast per-commit suite (unit + proptest + minimized crash corpus); nightly differential-vs-OCCT and fuzzing over the full corpora; per-operation performance baselines with regression alerts; every triaged failure becomes a permanent fast-suite test.

---

## References

Part 1, spatial acceleration:
- Gottschalk, Lin, Manocha. OBBTree: A Hierarchical Structure for Rapid Interference Detection. SIGGRAPH 1996. UNC TR96-013. https://www.cs.unc.edu/techreports/96-013.pdf
- CGAL. 3D Fast Intersection and Distance Computation (AABB Tree). https://doc.cgal.org/latest/AABB_tree/index.html
- Open CASCADE Technology. BVH_Tree class and Redesigned selection in OCCT 6.9.0. https://dev.opencascade.org/doc/occt-7.4.0/refman/html/class_b_v_h___tree_base.html ; https://dev.opencascade.org/index.php?q=node/1084
- Wald, Woop, Benthin, Johnson, Ernst. Embree: A Kernel Framework for Efficient CPU Ray Tracing. ACM TOG 33(4), 2014. https://www.sci.utah.edu/~wald/Publications/2014/embree/embree.pdf
- Guttman (R-trees, 1984); Beckmann et al. (R*-tree, 1990); Quinlan (sphere trees, ICRA 1994), via collision/proximity surveys.
- NURBS convex-hull property and recentered Bernstein bounds. https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/spline/NURBS/NURBS-property.html
- Kopta et al., Fast Effective BVH Updates for Animated Scenes (I3D 2012); Bittner et al., insertion-based BVH optimization. https://www.researchgate.net/publication/254007711_Fast_Effective_BVH_Updates_for_Animated_Scenes
- Siemens Parasolid. PK_TOPOL_clash. http://www.q-solid.com/Parasolid_Docs_V35/headers/pk_topol_clash.html
- Spatial / Designing Buildings. Clash detection in BIM. https://www.spatial.com/glossary/what-is-clash-detection ; https://www.designingbuildings.co.uk/wiki/Clash_Detection_in_3D_BIM_Models
- Larsen, Gottschalk, Lin, Manocha. PQP / Swept Sphere Volumes. https://gamma.web.unc.edu/research/collision/packages/ ; Gilbert, Johnson, Keerthi. GJK distance (1988).

Part 2, testing methodology:
- Koch et al. ABC: A Big CAD Model Dataset for Geometric Deep Learning. CVPR 2019. arXiv:1812.06216. https://deep-geometry.github.io/abc-dataset/
- Willis et al. Fusion 360 Gallery Dataset. ACM TOG 2021. https://github.com/AutodeskAILab/Fusion360GalleryDataset ; MFCAD / MFCAD++.
- Zhou, Jacobson. Thingi10K: A Dataset of 10,000 3D-Printing Models. arXiv:1605.04797. https://arxiv.org/abs/1605.04797
- CAx-IF / NIST STEP test suites and validation properties.
- McKeeman. Differential Testing for Software (1998), applied to geometry.
- Cignoni, Rocchini, Scopigno. Metro: Measuring Error on Simplified Surfaces (CGF 1998); precise Hausdorff between meshes. https://www.researchgate.net/publication/222396707_Precise_Hausdorff_distance_computation_between_polygonal_meshes
- Chen, Cheung, Yiu. Metamorphic Testing (1998). https://en.wikipedia.org/wiki/Metamorphic_testing ; Claessen, Hughes. QuickCheck (2000); Rust proptest.
- LLVM libFuzzer; AFL++; OSS-Fuzz. https://llvm.org/docs/LibFuzzer.html ; OpenCASCADE importer-robustness reports.
- Hu, Schneider, Wang, Zorin, Panozzo. Fast Tetrahedral Meshing in the Wild (fTetWild). ACM TOG 2020. arXiv:1908.03581. https://arxiv.org/abs/1908.03581
- Shewchuk. Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates. DCG 1997. https://people.eecs.berkeley.edu/~jrs/papers/robust-predicates.pdf
- Bartels, Fortune. Fast Floating-Point Filters for Robust Predicates. arXiv:2208.00497 ; Sawaya et al. FLiT. IISWC 2017. https://pruners.github.io/pdf/iiswc2017-final43.pdf

Part 3, API design:
- Siemens Parasolid. Introduction to Parasolid / Concepts. http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.002.html
- Spatial. 3D ACIS Modeler documentation; ACIS Kernel reference. https://www.spatial.com/solutions/3d-modeling/3d-acis-modeler ; http://www-isl.ece.arizona.edu/ACIS-docs/
- Open CASCADE Technology. Modeling Data; Standard_Transient; BRepBuilderAPI. https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html
- Kripac. A Mechanism for Persistently Naming Topological Entities (1997); Wu et al. Mechanisms of Persistent Identification of Topological Entities in CAD Systems: A Review. Alexandria Eng. J. 2018. https://www.sciencedirect.com/science/article/pii/S1110016818300814
- Braun et al. Fornjot. https://github.com/hannobraun/fornjot ; PR #2144 Object Reference Validation. https://github.com/hannobraun/fornjot/pull/2144
- RICOS. truck: a Rust CAD kernel. https://github.com/ricosjp/truck
