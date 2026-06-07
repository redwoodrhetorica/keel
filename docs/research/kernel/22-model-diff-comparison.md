# CAD Model Comparison, Differencing, and Shape Similarity

Research review supporting the design of Keel, an open-source B-rep solid modeling kernel in Rust.

## Scope and motivation

This topic surfaced from three independent consumers inside and above the kernel:

1. **A testing oracle.** Differential testing (running an operation through Keel and through a reference kernel) and regression testing (running an operation across two Keel versions) both need a "same shape?" predicate. The predicate must be tolerant (floating point and tessellation differ), cheap in the common pass case, and decisive in the fail case (localize where the shapes diverge).
2. **Version control of models.** OnShape-style branch/merge, PDM revision tracking, and engineering change orders all need to detect and localize differences between two versions of a model, ideally with semantic meaning, not just "the red pixels here changed."
3. **Duplicate-part retrieval and reuse.** PDM dedup, standard-part libraries (fasteners), and "have we modeled this before?" search all need shape similarity over large catalogs.

These three look different but share a backbone: a cascade of comparison primitives from cheap global fingerprints to expensive entity-by-entity correspondence. Most of the diff and retrieval intelligence lives *above* the kernel. What the kernel must provide is a small set of deterministic, stable primitives: mass properties to tight tolerance, stable face and edge sampling, canonical orientation services, and entity correspondence hooks. This review surveys the published methods that consume those primitives, so we know exactly which primitives to expose and to what tolerance.

The review is organized thematically: (1) exact and tolerant B-rep equality, (2) validation properties standards, (3) model differencing for version control, (4) shape retrieval and similarity descriptors, (5) graph and feature based matching, (6) geometric hashing and fingerprinting, (7) registration as a subproblem, (8) partial and assembly level comparison, (9) learning based B-rep comparison. A closing synthesis proposes the comparison cascade for Keel.

---

## 1. Exact and tolerant B-rep equality

### 1.1 B-rep matching for cross-system correspondence (Autodesk)

**Citation.** Hu, B., Lambourne, J. G., Willis, K. D. D., Lou, K., & Morris, J. (2023). *B-rep Matching for Collaborating Across CAD Systems.* ACM Transactions on Graphics, 42(4). (arXiv:2306.03169.)

**Method.** A learning-based algorithm finds correspondences between two B-reps expressed in the neutral interchange representation, so that references to a specific face, edge, or vertex survive geometric and topological edits and round trips through different kernels. The pipeline is hierarchical: first match whole bodies by coarse signals (size, complexity, vertex/edge/face counts and their spatial layout), then match exactly coincident vertices, edges, and faces within tolerance, then use a learned model to resolve the remaining ambiguous entities by local geometric and topological context. The output is a partial bijection between the two entity sets, with confidence scores.

**Limitations.** Learned; needs training data and may mismatch on shapes outside its distribution. Aimed at near-identical models that diverged through interoperation, not at arbitrary similar parts. Correspondence is per-entity and does not by itself produce a tolerance-quantified "equal/not equal" verdict.

**Kernel relevance.** This is the canonical statement of the entity-correspondence problem Keel's diff layer must solve. The hierarchical cascade (coarse count and size filter, then exact coincidence within tolerance, then ambiguous-case resolution) is exactly the structure of the testing oracle. The kernel should expose stable per-entity geometric signatures (surface type and parameters, bounding box, area, centroid) so a matcher above the kernel can run the coarse and coincidence stages without learning, reserving learned or expensive matching for residual ambiguity.

### 1.2 Canonical surface forms and tolerance semantics

**Citation.** Drawn from PTC's B-rep matching patent family and general kernel practice: PTC Inc. (2022). *B-rep matching for maintaining associativity across CAD interoperation.* US Patent 11,288,411 B2.

**Method.** Canonical analytic surfaces (plane, cylinder, cone, sphere, torus) are compared by canonicalizing their defining coefficients: a cylinder by axis line and radius, a quadric by its coefficient vector reduced to a canonical frame, and so on. Two surfaces are "equal" when their canonical parameters agree within tolerance and their trimmed extents overlap. Tolerance is an explicit length scale (commonly 1e-6 m or tighter) below which two points, curves, or surfaces are deemed coincident. For free-form (NURBS) geometry, equality cannot rely on control-point identity (knot insertion and reparameterization leave the curve unchanged), so comparison falls back to sampling.

**Limitations.** Canonical forms exist only for analytic surfaces; NURBS comparison is intrinsically sampling-based and tolerance-sensitive. Trimming makes two surfaces with identical underlying geometry but different trim loops "different" by face, which is correct for diff but must be handled deliberately.

**Kernel relevance.** Keel should provide a canonicalization service for analytic geometry (axis-aligned, normalized parameter forms) so equality and hashing operate on stable representatives, and a sampling service for NURBS so the comparison layer is not forced to reach into control points. The tolerance must be a single, documented, kernel-wide length scale that the oracle can reason about.

### 1.3 Sampling-based Hausdorff distance between B-reps

**Citation.** Standard practice synthesized from the validation-properties literature (Section 2) and the distance-computation research feeding Keel; representative: ASME / mesh-comparison Hausdorff usage as in commercial validators.

**Method.** When analytic equality does not apply, geometric agreement is quantified by sampling each face on its parameter grid (and each edge along its curve) into dense point sets, then measuring the directed and symmetric Hausdorff distance (and often the mean/RMS distance) between the two point sets after alignment. A small max-distance certifies geometric coincidence; a localized spike localizes a difference to a region. This is the engine behind "highlight the changed region in red" overlays.

**Limitations.** Hausdorff is sensitive to outliers and to sampling density; sparse sampling can miss thin features, dense sampling is costly. It needs prior alignment (registration) when the two bodies are not already in the same frame. It measures point-set proximity, not topological equivalence: two shapes can be Hausdorff-close yet topologically different.

**Kernel relevance.** Keel must offer deterministic, density-controllable face and edge sampling that returns identical points for identical inputs, so Hausdorff distance is reproducible across runs and machines. This is the bridge between Keel's existing distance research and the comparison oracle: the same robust point-on-surface sampling serves both.

---

## 2. Validation properties standards (CAx-IF / LOTAR)

### 2.1 Geometric and assembly validation properties

**Citation.** CAx Interoperability Forum / MBx-IF. (2024). *Recommended Practices for Geometric and Assembly Validation Properties* (v4.5). mbx-if.org. Related standard: ISO 10303-59 (data quality). LOTAR International long-term-archiving workflows build on this.

**Method.** A producing CAD system computes a set of validation properties from its native model and writes them into the STEP file alongside the geometry. The consuming system, after rebuilding geometry from the same STEP file, recomputes the same properties from its own kernel and compares. Properties are split into:
- **Geometric validation properties (per solid/shell/surface):** volume (solids only), surface area, and centroid (center of gravity). For shells and surfaces, only area applies.
- **Cloud of Points (COPS):** a deterministic sample of points attached to faces and edges, serving as a geometric fingerprint that catches local deviations a single scalar (volume) would miss. The producer records the points; the consumer verifies the points lie on its rebuilt faces and edges within tolerance.
- **Assembly validation properties:** counts of children, and per-occurrence placement (transform) checks, so an assembly's structure and component positions can be verified independently of the leaf geometry.

Deviations between stated and recomputed values give a quantitative measure of exchange (translation) quality. LOTAR treats the bundle (mass properties plus geometric centers plus surface point samples) as the digital fingerprint that proves model integrity decades later.

**Limitations.** Validation properties certify agreement, they do not localize or explain a difference beyond "this face's COPS failed." Volume and area agreement is necessary but not sufficient (different shapes can share volume). COPS sampling density and placement must be agreed or the comparison is ambiguous. The standard assumes both sides share an entity correspondence (same STEP entity ids), which holds for translation validation but not for independently authored models.

**Kernel relevance.** This is the single most directly reusable standard for Keel. It defines, with industry consensus, exactly which kernel primitives the comparison layer needs: deterministic volume, area, centroid (to tight tolerance, tying to Keel's quadrature research), plus a deterministic cloud-of-points sampler on faces and edges. Keel should implement a native validation-properties producer and consumer. The producer doubles as the cheap fingerprint stage of the testing oracle; the consumer is the regression check. Adopting COPS verbatim gives us free interoperability with every STEP-AP242 tool and with LOTAR archives.

### 2.2 Commercial translation validators (Kubotek K-Compare Validate)

**Citation.** Kubotek Kosmos. (2018-2024). *K-Compare Validate* product documentation and release notes (v4.0-5.0). kubotekkosmos.com; DEVELOP3D review.

**Method.** An independent, CAD-neutral framework reads the major native formats (CATIA, NX, Creo, SolidWorks) plus STEP/IGES without host CAD licenses and acts as a neutral auditor comparing a translated model against its authority model. It performs geometric comparison (volume, area, point/face deviation) and, from v4.0, full semantic comparison of STEP AP242 PMI (annotations and tolerances tagged to faces), confirming GD&T survived translation. v5.0 adds comparison of point-cloud and mesh derivative models against the original CAD authority.

**Limitations.** Proprietary; the matching and tolerance internals are not published. Designed for translation validation (where correspondence is implied by a shared source), less so for independently authored similar parts.

**Kernel relevance.** Validates the architecture: a neutral, kernel-level comparator that consumes geometry plus validation properties plus tagged PMI. Confirms that for an OSS kernel, supporting AP242 validation properties and face-tagged PMI comparison is the table-stakes feature set for the version-control and translation consumers.

---

## 3. Model differencing for version control

### 3.1 Version control in modern CAD: challenges and tool survey

**Citation.** Deng, X., Zhang, et al., Olechowski, A., & Zhou, et al. (2026). *Untangling the Timeline: Challenges and Opportunities in Supporting Version Control in Modern Computer-Aided Design.* Proceedings of the 2026 CHI Conference on Human Factors in Computing Systems (CHI '26), Barcelona.

**Method.** An interview-and-tool study mapping how engineers actually do version control in cloud and parametric CAD. It identifies three diff dimensions: **geometric** (boundary changes, broadly supported), **parametric** (modified feature-command parameters, partially supported), and **semantic/intent** (the *why* of a change, essentially unsupported). It tabulates how tools compare models: Onshape overlays two versions and highlights geometric differences in red while listing the changed modeling commands in a side panel; SolidWorks shows side-by-side with parameter comparison and add/delete highlighting; Fusion shows only a command list. The central thesis: CAD VCS is not software VCS in another domain, because geometry and parameters are tightly coupled, so Git-style fine-grained selective merge does not transfer.

**Limitations.** A human-factors study, not an algorithm paper; it characterizes the gap rather than solving it. Tool capabilities are as-documented and evolve.

**Kernel relevance.** Defines the requirements ceiling for Keel's VC consumer. The geometric-diff layer (overlay plus changed-region highlight) is exactly the sampling-plus-Hausdorff cascade Keel can power. The parametric and semantic layers live above the kernel in the feature/history graph, but Keel must supply stable entity lineage (naming research) so that "this face is the same face, edited" is distinguishable from "this face was deleted and a new one created." Cross-session diff (two independently opened models) cannot rely on lineage and must fall back to the geometric matcher of Section 1.

### 3.2 Feature-based model difference identification

**Citation.** Representative: *Feature-Based Model Difference Identification for Aerospace Sheet Metal Parts* (2020), and the lineage from Bhandarkar, M. P., & Nagi, R. (2000). *STEP-based feature extraction from STEP geometry for Agile Manufacturing.* Computers in Industry, 41(1), 3-24.

**Method.** Bhandarkar and Nagi established rule-based extraction of manufacturing features (holes, pockets, slots) from low-level STEP AP203 geometry, mapping them into a neutral feature representation (AP224). Model-difference-identification (MDI) work builds on this: rather than diffing raw geometry, it recognizes features in both versions and then diffs the *feature sets* and their parameters, so a change is reported in engineering terms ("hole diameter 8 to 10," "pocket added") rather than as a cloud of moved points. Sheet-metal MDI specializes the feature vocabulary (bends, flanges, cutouts).

**Limitations.** Feature recognition is itself fragile under feature interaction and free-form geometry; the diff quality is bounded by the recognizer. Vocabulary is domain-specific. Two models authored with different feature trees that produce identical geometry will diff as "completely different" at the feature level even though geometrically identical.

**Kernel relevance.** Shows the value of a feature-aware diff layer above the kernel, and that it depends on (a) the kernel exposing clean topology and curvature for a recognizer to run on, and (b) a separate purely-geometric fallback for the "same geometry, different tree" case. Keel should not bake feature recognition into the kernel but must make its B-rep cleanly traversable for one.

---

## 4. Shape retrieval and similarity descriptors

### 4.1 Shape distributions (D2)

**Citation.** Osada, R., Funkhouser, T., Chazelle, B., & Dobkin, D. (2002). *Shape Distributions.* ACM Transactions on Graphics, 21(4), 807-832.

**Method.** Represent a shape's signature as a probability distribution sampled from a shape function over the model. The flagship function D2 is the distribution of Euclidean distances between pairs of random surface points; others measure angles (A3), areas (D3), volumes (D4), or single-point distances (D1). Shape similarity reduces to comparing 1D distributions (L_p, chi-squared, or earth-mover distance). The descriptor is independent of representation (mesh, CSG, B-rep, voxels, polygon soup) as long as random surface points can be drawn, and is invariant to rigid motion because it uses only pairwise distances.

**Limitations.** Coarse; good at gross discrimination (cup vs car), weak at fine distinctions (two similar brackets). No localization, no correspondence. Sensitive to sampling and to scale unless normalized.

**Kernel relevance.** A near-ideal cheap retrieval and pre-filter descriptor, and it consumes exactly the primitive Keel already needs for Hausdorff and validation properties: uniform random surface point sampling. D2 over a body is a few-hundred-float fingerprint usable for catalog pre-filtering and as a cheap "obviously different" rejecter in the oracle cascade. Rigid-invariance means no alignment is required.

### 4.2 Rotation-invariant spherical harmonic descriptors

**Citation.** Kazhdan, M., Funkhouser, T., & Rusinkiewicz, S. (2003). *Rotation Invariant Spherical Harmonic Representation of 3D Shape Descriptors.* Symposium on Geometry Processing (SGP), 156-164.

**Method.** Voxelize the model on concentric spheres; on each sphere decompose the function into spherical harmonics; the energy per frequency band on each sphere is rotation-invariant (rotations mix harmonics within a band but preserve band energy). The descriptor is the matrix of band energies across radii and frequencies. This converts any non-rotation-invariant spherical descriptor into a rotation-invariant one without needing pose normalization, sidestepping the instability of PCA alignment.

**Limitations.** Discards phase, so it cannot distinguish shapes that differ only in the angular arrangement within bands (some loss of discrimination). Voxelization resolution trades accuracy for cost. Still a global descriptor: no localization or partial matching.

**Kernel relevance.** Important as the principled alternative to PCA pose-normalization (Section 6.2). For Keel's retrieval consumer, this argues for invariant descriptors over fragile canonical-alignment pipelines. The kernel primitive needed is a deterministic surface-to-volume or surface-sampling rasterizer; otherwise the descriptor lives entirely above the kernel.

### 4.3 Topology matching with multiresolutional Reeb graphs

**Citation.** Hilaga, M., Shinagawa, Y., Kohmura, T., & Kunii, T. L. (2001). *Topology Matching for Fully Automatic Similarity Estimation of 3D Shapes.* SIGGRAPH 2001, 203-212.

**Method.** Build a Reeb graph using integral geodesic distance as the Morse function (translation/rotation/scale invariant and robust to mesh resimplification). Compute it at multiple resolutions to form a Multiresolutional Reeb Graph (MRG). Similarity is the sum of similarity scores over topologically consistent node pairs, found by a coarse-to-fine heuristic graph matcher. Captures part structure and connectivity (a chair has a back, seat, legs) rather than just bulk distance statistics.

**Limitations.** Geodesic-distance functions assume a single connected manifold mesh; multi-shell solids, sheets, and non-manifold B-reps need care. More expensive than distribution descriptors. Best for organic/articulated shapes; mechanical parts with many flat faces give degenerate Reeb structure.

**Kernel relevance.** Less directly applicable to mechanical B-reps than D2 or graph methods, but it establishes the principle that topological/structural descriptors discriminate where global statistics fail, motivating the AAG-based methods (Section 5) that are the mechanical-CAD analog.

### 4.4 Survey: three-dimensional shape searching

**Citation.** Iyer, N., Jayanti, S., Lou, K., Kalyanaraman, Y., & Ramani, K. (2005). *Three-dimensional shape searching: state-of-the-art review and future trends.* Computer-Aided Design, 37(5), 509-530. (Companion: Iyer et al. (2005), *Shape-based searching for product lifecycle applications*, CAD.)

**Method.** A taxonomy of 3D shape search by representation: global feature based (moments, mass properties, D2), histogram/distribution based, graph based (skeletal, Reeb, B-rep adjacency), view/image based, and topology based. Compares them on discrimination, invariance, partial-match capability, and cost, with an engineering-domain lens (the authors built the Purdue 3D engineering search system).

**Limitations.** Predates deep learning; the descriptor landscape has shifted. Evaluation predates large standardized benchmarks beyond their own.

**Kernel relevance.** The reference map for choosing Keel's retrieval descriptors. Its core lesson: no single descriptor wins; production systems combine a cheap global filter (mass properties, D2) with an expensive structural matcher (graph). That two-tier design is precisely the cascade we adopt.

### 4.5 The Engineering Shape Benchmark (ESB)

**Citation.** Jayanti, S., Kalyanaraman, Y., Iyer, N., & Ramani, K. (2006). *Developing an engineering shape benchmark for CAD models.* Computer-Aided Design, 38(9), 939-953.

**Method.** A public benchmark of 867 mechanical CAD models in 44 human-defined functional classes (flat-thin, rectangular-cubic, solid-of-revolution families), distributed as STL/OBJ, with a defined retrieval-evaluation protocol (precision-recall, nearest-neighbor, tier metrics). Built specifically because generic graphics benchmarks (e.g., Princeton Shape Benchmark) do not reflect engineering parts.

**Limitations.** Mesh-only (no B-rep), modest size by modern ML standards, classes are functional not geometric so some geometrically-similar parts sit in different classes. No partial-match ground truth.

**Kernel relevance.** The standard yardstick for any retrieval descriptor Keel's ecosystem ships. If Keel exposes descriptor primitives, ESB is how a downstream retrieval tool proves them. Useful as a regression corpus for the sampling and mass-property primitives themselves.

---

## 5. Graph and feature-based matching

### 5.1 Attributed adjacency graph (AAG) and machining-feature recognition

**Citation.** Joshi, S., & Chang, T. C. (1988). *Graph-based heuristics for recognition of machined features from a 3D solid model.* Computer-Aided Design, 20(2), 58-66.

**Method.** Represent the B-rep as an Attributed Adjacency Graph: nodes are faces, arcs are edges connecting adjacent faces, and each arc carries an attribute for the convexity of the shared edge (0 = concave, 1 = convex). Machining features are predefined subgraph patterns (a pocket is a set of mutually-concave-connected faces); recognition is subgraph matching against the model's AAG. Foundational to all graph-based feature recognition.

**Limitations.** Brittle under feature interaction (intersecting features corrupt the expected subgraph); exact subgraph isomorphism is NP-hard in general; pure convexity attributes lose geometric detail. Later work (minimal-condition subgraphs, hint-based methods) addresses interaction.

**Kernel relevance.** The AAG is the structural fingerprint format for mechanical B-reps and underlies both feature-based diff and graph-based similarity. Keel must make it trivial to emit an AAG: face adjacency, edge convexity (sign of dihedral/curvature), and per-face/edge attributes. This single export serves feature recognition, structural diff, and graph-based retrieval simultaneously.

### 5.2 Attributed graph matching for component similarity (El-Mehalawi & Miller)

**Citation.** El-Mehalawi, M., & Miller, R. A. (2003). *A database system of mechanical components based on geometric and topological similarity. Part I: representation; Part II: indexing, retrieval, matching and similarity assessment.* Computer-Aided Design, 35(1), 83-94 and 95-105.

**Method.** Each component is an attributed graph derived from its STEP file: nodes are faces (attributed by surface type, area), arcs are edges (attributed by type, convexity, length). Retrieval indexes the database by coarse graph invariants (node/arc counts, surface-type histograms) to shortlist candidates, then computes a similarity coefficient between query and candidate via optimal bipartite graph matching (matching faces to faces to maximize attribute agreement). Produces a graded similarity score, not a binary verdict.

**Limitations.** Bipartite/graph matching is expensive, hence the necessity of the coarse index; sensitive to the chosen attributes and weights; whole-model (not partial) by default; tessellation/translation noise perturbs attributes.

**Kernel relevance.** The clearest blueprint for a B-rep-native similarity service: cheap invariant index, then expensive correspondence-based score. The invariants it indexes on (counts, surface-type histograms, areas) are kernel-cheap and overlap with validation properties. This is the retrieval analog of the testing oracle's matching stage, confirming a shared correspondence engine can serve both consumers.

---

## 6. Geometric hashing and fingerprinting

### 6.1 Geometric hashing

**Citation.** Lamdan, Y., & Wolfson, H. J. (1988). *Geometric Hashing: A General and Efficient Model-Based Recognition Scheme.* Proceedings of the International Conference on Computer Vision (ICCV), Tampa, 238-249.

**Method.** Off-line, for each model, pick bases from triples of interest points; express all other points in each basis's invariant coordinate frame; quantize and store (model, basis) votes in a hash table keyed by the invariant coordinates. On-line, pick a basis from the scene, look up its points, and accumulate votes; a model-basis pair with many votes is a match (and yields the aligning transform). Achieves recognition under rigid (and affine) transforms via an indexing scheme, trading heavy preprocessing for fast query.

**Limitations.** Hash-table size and collision behavior grow badly with point count; quantization is tolerance-sensitive (the perennial bin-boundary problem); designed for point features, so it needs robust, repeatable interest points, which smooth CAD surfaces lack. Noise and missing points degrade voting.

**Kernel relevance.** The intellectual root of rigid-invariant indexing and content-addressable geometry. For Keel, the lesson is cautionary: geometric hashing of raw points is fragile for CAD, but the underlying idea (invariant keys indexing a content store) maps onto PDM dedup. A robust CAD fingerprint should hash invariant *aggregate* quantities (volume, area, inertia eigenvalues, D2 buckets) rather than individual points.

### 6.2 PCA pose normalization and its instabilities; inertia-tensor fingerprints

**Citation.** Synthesized from the descriptor literature (Kazhdan 2003, above; Vranic and Saupe pose-estimation work) and standard kernel mass-property practice.

**Method.** A common canonical-alignment recipe translates the centroid to the origin and rotates so the principal axes of the inertia tensor align with the coordinate axes, optionally flipping by moment signs. This yields a pose-normalized model for direct descriptor comparison and a rigid-invariant fingerprint from the inertia eigenvalues (principal moments). Volume, surface area, centroid, and the three principal moments together form a compact, rigid-motion-invariant signature.

**Limitations.** PCA alignment is unstable when principal moments are near-equal (symmetric or near-symmetric parts), where small perturbations flip axes, breaking the canonical frame and producing false mismatches. Axis sign ambiguity needs heuristics. This is the central reason invariant descriptors (Kazhdan) are preferred over canonical alignment.

**Kernel relevance.** Two takeaways. First, the inertia-eigenvalue + volume + area + centroid vector is the cheapest meaningful rigid-invariant fingerprint and Keel must compute it deterministically to tight tolerance (ties directly to the quadrature/mass-property research). Second, Keel's canonical-orientation service must declare and detect the near-symmetric degenerate cases and refuse to over-commit to an unstable frame, exposing the symmetry group instead so the comparison layer can handle ambiguity explicitly rather than silently mismatching.

---

## 7. Registration as a subproblem

### 7.1 Iterative Closest Point (ICP)

**Citation.** Besl, P. J., & McKay, N. D. (1992). *A Method for Registration of 3-D Shapes.* IEEE Transactions on Pattern Analysis and Machine Intelligence, 14(2), 239-256.

**Method.** Given a rough initial alignment, iterate: find each source point's nearest target point, solve for the rigid transform minimizing squared correspondence distance (closed form via SVD), apply, repeat to convergence. The workhorse for fine alignment of two point sets or sampled surfaces before comparison.

**Limitations.** Converges only to a local minimum; needs a good initial guess; sensitive to outliers, partial overlap, and sampling. Well known; many robust variants (point-to-plane, trimmed ICP).

**Kernel relevance.** The fine-alignment step that precedes Hausdorff comparison when two bodies are not already co-framed. Keel's sampling primitive feeds it; ICP itself lives above the kernel.

### 7.2 Globally optimal registration (Go-ICP)

**Citation.** Yang, J., Li, H., Campbell, D., & Jia, Y. (2016). *Go-ICP: A Globally Optimal Solution to 3D ICP Point-Set Registration.* IEEE Transactions on Pattern Analysis and Machine Intelligence, 38(11), 2241-2254. (arXiv:1605.03344.)

**Method.** Branch-and-bound over the full rigid-motion space SE(3) with derived upper/lower bounds on the ICP L2 error, interleaved with local ICP to tighten bounds. Guarantees the global optimum regardless of initialization, removing ICP's local-minimum failure.

**Limitations.** Slower than local ICP; cost grows with point count and required precision; still L2-based, so outlier-sensitive without trimming.

**Kernel relevance.** The fallback when the cheap fingerprints disagree on pose but the shapes may still be equal (the oracle must not declare "different" merely because two correct results came out in different frames). Go-ICP provides an initialization-free alignment before the final Hausdorff verdict. Above the kernel, fed by Keel's sampler.

---

## 8. Partial similarity and assembly-level comparison

### 8.1 Partial and subpart matching on AAG / partial shape retrieval

**Citation.** Representative: *Partial Matching of 3D CAD Models with Attributed Graph* (Applied Mechanics and Materials, 528, 302); and minimal-condition-subgraph interacting-feature recognition (Computer-Aided Design, 1998).

**Method.** Finding a feature or region of one model inside another reduces to subgraph matching of the query's AAG against the target's AAG, with attribute-tolerant node/edge matching. Minimal-condition-subgraph methods make this robust to feature interaction by matching the smallest distinguishing pattern rather than the full ideal feature graph. Application: recognizing standard parts (a fastener's thread/head subgraph) embedded in a larger model.

**Limitations.** Subgraph isomorphism is NP-hard; needs strong pruning by attribute filters. Tolerance on attributes trades false positives against misses. Geometric (not just topological) verification of a candidate subgraph match is still required.

**Kernel relevance.** Partial retrieval is the standard-part-recognition and fastener-library consumer. It rides on the same AAG export (Section 5.1) plus a geometric verification step using the sampler. Confirms one structural export serves whole-model and partial matching alike.

### 8.2 Assembly-structure and BOM diff

**Citation.** Synthesized from the validation-properties assembly section (Section 2.1), Onshape/SolidWorks compare documentation (Section 3.1), and PDM BOM-compare practice.

**Method.** Assembly comparison is a tree diff over the occurrence hierarchy: match occurrences across two versions by (a) persistent occurrence ids / lineage when available, (b) referenced-part identity, and (c) placement transform, with rename/move detection analogous to file-tree diff. BOM comparison flattens to a multiset of (part, quantity) and diffs the multisets, then reconciles structural moves. Component position is verified via the assembly validation properties (per-occurrence transforms).

**Limitations.** Occurrence matching is ambiguous when many identical components exist (which of the eight identical bolts moved?); pure transform matching is fragile under global repositioning; rename detection is heuristic. Cross-session diff without persistent ids degrades to geometric leaf matching plus structural alignment.

**Kernel relevance.** Assembly diff is almost entirely above the kernel (it is graph/tree logic over occurrences), but it consumes two kernel-level primitives: per-occurrence transforms with stable precision, and a leaf-geometry equality test (Sections 1-2) to confirm "same part." Keel should expose stable occurrence transforms and a leaf-body fingerprint so the assembly differ can match occurrences geometrically when ids are absent.

---

## 9. Learning-based B-rep comparison and retrieval

### 9.1 UV-Net

**Citation.** Jayaraman, P. K., Sanghi, A., Lambourne, J. G., Willis, K. D. D., Davies, T., Shayani, H., & Morris, N. (2021). *UV-Net: Learning from Boundary Representations.* CVPR 2021.

**Method.** Encode a B-rep directly: sample each face as a 2D UV-grid and each edge as a 1D UV-grid (uniform steps in parameter space, carrying point position and surface normal/derivatives), apply 2D/1D CNNs to encode local geometry, then a graph network over the face-adjacency graph to fuse them into a whole-model embedding. The embedding supports classification and, by nearest-neighbor in embedding space, retrieval. Showed structured UV sampling beats unordered point clouds for B-rep tasks.

**Limitations.** Learned, data-hungry, distribution-bound; embeddings give similarity scores, not tolerance-quantified equality or localized diffs. Requires a faithful UV sampler and the face-adjacency graph as input.

**Kernel relevance.** Its input requirements are a precise inventory of the kernel primitives needed for modern retrieval: deterministic UV-grid sampling of faces and edges (position + normal + derivatives) and the face-adjacency graph. These are the *same* primitives the validation-properties COPS and the AAG export need. Confirming this convergence is the key design payoff: one sampling-and-adjacency layer serves classical fingerprints, classical graph matching, and learned embeddings.

### 9.2 BRepNet and self-supervised B-rep retrieval

**Citation.** Lambourne, J. G., Willis, K. D. D., Jayaraman, P. K., Sanghi, A., Meltzer, P., & Shayani, H. (2021). *BRepNet: A Topological Message Passing System for Solid Models.* CVPR 2021. Plus: *Self-Supervised Representation Learning for CAD* (2022, arXiv:2210.10807).

**Method.** BRepNet defines convolution-like message passing directly on B-rep topology using ordered face-edge-coedge walks (kernels defined over the winged-edge-style local topology), learning per-entity features (e.g., machining-feature segmentation). The self-supervised line trains a UV-Net/B-rep encoder with graph contrastive learning (augment the face-adjacency graph to form positive pairs) so retrieval embeddings can be learned without labels, addressing the absence of labeled CAD retrieval corpora.

**Limitations.** Tied to a specific topological data structure (coedges/halfedges); learned and distribution-bound; per-entity learned features are not a substitute for exact correspondence in a verification oracle.

**Kernel relevance.** BRepNet's dependence on ordered coedge/halfedge traversal is a direct requirement on Keel's topology API: it must expose the winged-edge/coedge structure with deterministic ordering. Self-supervised retrieval matters for the dedup/reuse consumer because real PDM vaults are unlabeled. Both ride entirely above the kernel on the topology-traversal and UV-sampling primitives.

---

## Comparison services for Keel

The three consumers (testing oracle, version-control diff, duplicate retrieval) all reduce to one architectural pattern: a **cheap-to-expensive comparison cascade** built on a small set of kernel primitives, with diff and retrieval intelligence layered above. The recurring finding across every source is that no single comparison is right: production systems chain a cheap global filter to an expensive correspondence step, and short-circuit as early as possible.

### Kernel primitives Keel must provide

These are the union of what every method above consumes, and they collapse to a short list:

1. **Deterministic mass properties to tight tolerance.** Volume, surface area, centroid, and full inertia tensor (principal moments). Required by validation properties (Section 2), inertia fingerprints (6.2), and global-feature retrieval (4.4). Must be reproducible bit-for-bit given identical input, tying directly to the quadrature research. This is the cheapest fingerprint and the first cascade stage.
2. **Deterministic, density-controllable surface and edge sampling.** Three flavors from one engine: uniform random surface points (for D2 / shape distributions, 4.1), UV-grid sampling with normals and derivatives (for COPS validation 2.1, Hausdorff 1.3, and learned encoders 9.1), and edge-curve sampling. Identical inputs must yield identical samples across machines.
3. **Canonical geometry forms.** Normalized analytic surface/curve parameters (axis-aligned cylinders, reduced quadrics) for exact tolerant equality and stable hashing (1.2), plus a NURBS sampling fallback.
4. **Structural export (AAG / face-adjacency graph) with attributes.** Face nodes (surface type, area), edge arcs (type, convexity sign from dihedral/curvature). One export feeds feature-based diff (3.2), graph similarity (5.1, 5.2), partial matching (8.1), and learned graph encoders (9.1, 9.2).
5. **Ordered topology traversal (coedge/halfedge).** Deterministic ordering, required by BRepNet-style consumers (9.2) and by any robust correspondence walk.
6. **Canonical orientation and symmetry service.** Inertia-based pose with explicit detection and reporting of near-symmetric degenerate cases (6.2), so the comparison layer handles pose ambiguity rather than silently mismatching. Prefer exposing the symmetry group over forcing an unstable canonical frame.
7. **Entity correspondence hooks (lineage).** Persistent entity ids within a session (from the naming research) so intra-session diff is exact; cross-session diff falls back to the geometric matcher.

### The cascade (testing oracle)

For "are these two B-reps the same shape within tolerance?", run stages in increasing cost, rejecting or accepting as early as possible:

1. **Topology counts and type histograms.** Compare vertex/edge/face counts and surface-type histograms. Mismatch on a comparison that should be exact rejects immediately. (From 1.1, 5.2.) Near-free.
2. **Scalar mass-property fingerprint.** Compare volume, area, centroid magnitude, and inertia eigenvalues within tolerance. A volume mismatch beyond tolerance rejects. (From 2.1, 6.2.) Cheap; rigid-invariant, so no alignment needed.
3. **Distribution fingerprint (D2).** Compare shape-distribution histograms. Catches gross shape differences that share volume. (From 4.1.) Cheap; rigid-invariant.
4. **Cloud-of-points / Hausdorff after alignment.** If the fingerprints pass, sample both bodies and compute symmetric Hausdorff (and RMS). Where bodies are not co-framed, align first with ICP, escalating to Go-ICP when pose is uncertain or near-symmetry was flagged. A small max-distance accepts; a localized spike both rejects and *localizes* the difference for the diff consumer. (From 1.3, 2.1, 7.1, 7.2.)
5. **Entity correspondence verification.** For a decisive verdict and for diff localization, run hierarchical entity matching (coarse signals, then coincidence within tolerance, then ambiguous-case resolution) and verify matched entities agree geometrically. (From 1.1, 5.2.)

For differential testing against a reference kernel, stages 1-4 with tight tolerances are the workhorse; for regression testing across Keel versions, intra-session lineage (primitive 7) lets stage 5 be exact and fast.

### What stays above the kernel

The diff and retrieval *intelligence* is not kernel code:

- **Version-control diff** (overlay, changed-region highlight, parametric and feature-level diff, branch/merge) lives in the application/PDM layer. It consumes the cascade's geometric verdict and localization (stage 4) plus the structural export (primitive 4) and lineage (primitive 7). Semantic/intent diff (3.1) is out of scope for the kernel entirely.
- **Retrieval and dedup** (descriptor indexing, embedding training, nearest-neighbor search, ESB-style evaluation) is a service over the kernel's fingerprints (primitives 1, 2, 4). Classical (D2, inertia, AAG matching) and learned (UV-Net/BRepNet) approaches both ride the same primitives; the choice is the downstream tool's, not the kernel's.
- **Assembly and BOM diff** (8.2) is tree/graph logic over occurrences, consuming only stable occurrence transforms and the leaf-body equality test.
- **Feature recognition** (5.1, 3.2) runs on the AAG export; the kernel supplies clean topology, not the recognizer.

### Standards posture

Keel should implement the **CAx-IF / LOTAR geometric and assembly validation properties (AP242)** natively, as both producer and consumer (Section 2). This single decision: (a) gives the testing oracle its cheap fingerprint stage for free, (b) makes Keel interoperable with every commercial validator (Kubotek, NX, SolidWorks compare) and LOTAR archive, and (c) reuses exactly the mass-property and cloud-of-points primitives the rest of the cascade already requires. It is the highest-leverage standards investment for the comparison surface.

---

## References

1. Besl, P. J., & McKay, N. D. (1992). A Method for Registration of 3-D Shapes. *IEEE TPAMI*, 14(2), 239-256.
2. Bhandarkar, M. P., & Nagi, R. (2000). STEP-based feature extraction from STEP geometry for Agile Manufacturing. *Computers in Industry*, 41(1), 3-24.
3. CAx Interoperability Forum / MBx-IF. (2024). *Recommended Practices for Geometric and Assembly Validation Properties* (v4.5). https://www.mbx-if.org/
4. Deng, X., Zhang, Olechowski, A., & Zhou (2026). Untangling the Timeline: Challenges and Opportunities in Supporting Version Control in Modern Computer-Aided Design. *CHI '26*. https://arxiv.org/abs/2602.09236
5. El-Mehalawi, M., & Miller, R. A. (2003). A database system of mechanical components based on geometric and topological similarity, Parts I & II. *Computer-Aided Design*, 35(1), 83-105.
6. Hilaga, M., Shinagawa, Y., Kohmura, T., & Kunii, T. L. (2001). Topology Matching for Fully Automatic Similarity Estimation of 3D Shapes. *SIGGRAPH 2001*, 203-212.
7. Hu, B., Lambourne, J. G., Willis, K. D. D., Lou, K., & Morris, J. (2023). B-rep Matching for Collaborating Across CAD Systems. *ACM TOG*, 42(4). https://arxiv.org/abs/2306.03169
8. Iyer, N., Jayanti, S., Lou, K., Kalyanaraman, Y., & Ramani, K. (2005). Three-dimensional shape searching: state-of-the-art review and future trends. *Computer-Aided Design*, 37(5), 509-530.
9. Jayanti, S., Kalyanaraman, Y., Iyer, N., & Ramani, K. (2006). Developing an engineering shape benchmark for CAD models. *Computer-Aided Design*, 38(9), 939-953.
10. Jayaraman, P. K., et al. (2021). UV-Net: Learning from Boundary Representations. *CVPR 2021*. https://github.com/AutodeskAILab/UV-Net
11. Joshi, S., & Chang, T. C. (1988). Graph-based heuristics for recognition of machined features from a 3D solid model. *Computer-Aided Design*, 20(2), 58-66.
12. Kazhdan, M., Funkhouser, T., & Rusinkiewicz, S. (2003). Rotation Invariant Spherical Harmonic Representation of 3D Shape Descriptors. *SGP 2003*, 156-164.
13. Kubotek Kosmos. (2018-2024). K-Compare Validate product documentation (v4.0-5.0). https://www.kubotekkosmos.com/
14. Lambourne, J. G., et al. (2021). BRepNet: A Topological Message Passing System for Solid Models. *CVPR 2021*.
15. Lamdan, Y., & Wolfson, H. J. (1988). Geometric Hashing: A General and Efficient Model-Based Recognition Scheme. *ICCV 1988*, 238-249.
16. Osada, R., Funkhouser, T., Chazelle, B., & Dobkin, D. (2002). Shape Distributions. *ACM TOG*, 21(4), 807-832.
17. PTC Inc. (2022). B-rep matching for maintaining associativity across CAD interoperation. US Patent 11,288,411 B2.
18. *Self-Supervised Representation Learning for CAD* (2022). https://arxiv.org/abs/2210.10807
19. *Partial Matching of 3D CAD Models with Attributed Graph* (2014). *Applied Mechanics and Materials*, 528, 302.
20. *A universal method to compare parts from STEP files* (2022). *Journal of Intelligent Manufacturing*. https://link.springer.com/article/10.1007/s10845-022-01984-3
21. *Feature-Based Model Difference Identification for Aerospace Sheet Metal Parts* (2020).
22. Yang, J., Li, H., Campbell, D., & Jia, Y. (2016). Go-ICP: A Globally Optimal Solution to 3D ICP Point-Set Registration. *IEEE TPAMI*, 38(11), 2241-2254. https://arxiv.org/abs/1605.03344
