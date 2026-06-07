# Local Operations and Direct Editing in B-rep Solid Modeling

Research dossier supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition. Compiled 2026-06-07.

## Scope and framing

Local operations are the booleans' awkward sibling. Where a boolean takes two complete bodies and computes a new one through global surface-surface intersection, a local operation takes an *existing* body and edits a small set of faces in place: it replaces a face's surface, offsets or tapers it, moves it, deletes it and heals the wound, or removes a blend chain so an edge becomes sharp again. The defining technical move is **extend-and-reintersect**: when a face's surface changes or vanishes, the geometry of adjacent faces no longer meets it, so their surfaces must be extended (extrapolated) and re-intersected to recompute the bounding edges and vertices, while the topology (the face/edge/vertex graph) is kept as stable as possible and changed only where forced.

This capability is the foundation of the entire **direct modeling** wave: Siemens Synchronous Technology, SpaceClaim, PTC Creo Flexible Modeling, Autodesk Fusion direct edit, and the OnShape "Delete Face / Move Face / Replace Face" tools. It is also a major Parasolid differentiator: Parasolid's PK local-ops surface is broad and robust, ACIS's Local Operations Component (LOP) is comparable, and OpenCASCADE's equivalents are conspicuously thin. For Keel to be competitive it must treat local operations as a first-class subsystem, not an afterthought layered on booleans.

This file covers eight themes: (1) local-operation fundamentals and kernel API anatomy; (2) face deletion and wound healing / defeaturing; (3) direct modeling and synchronous technology; (4) blend recognition and removal; (5) feature recognition adjacent to editing; (6) parametric re-evaluation vs direct edits; (7) surface extension/extrapolation for healing; (8) midsurface and thin-wall-aware editing, plus robustness throughout.

A short style note: this document avoids the em-dash character entirely, per project convention.

---

## Theme 1: Local-operation fundamentals and kernel API anatomy

### 1.1 Parasolid PK local operations (tweaking, offsetting, tapering)

**Citation.** Siemens Digital Industries Software. (various versions, e.g. v12.0 through v35). *Parasolid Functional Description, Chapter 18: Local Operations* and *Parasolid PK Interface Programming Reference*. Siemens PLM Components documentation. Mirrored at q-solid.com/Parasolid_Docs.

**Method.** Parasolid groups under "local operations" a family that all share the tweak-and-reintersect skeleton. The canonical operation is surface replacement: `PK_FACE_change_surfs` / `PK_FACE_replace_surfs_3` swap the geometric surface(s) attached to one or more faces while keeping the surrounding topology. Parasolid then walks the edges bounding the tweaked faces, extends the surfaces of the *adjacent* faces as needed, and recomputes each shared edge as the intersection curve of the new surface pair, recomputing vertices as curve-curve or curve-surface intersections. The documentation explicitly demonstrates multi-face tweaks in one call (for example changing a cylinder's top planar face to a sphere and its side cylindrical face to a truncated cone simultaneously). Related members of the family: dependent face offsetting (`PK_FACE_offset` / offset with reintersection so neighbors follow), tapering faces about a draft axis (`PK_FACE_taper`), transforming/moving a face set (`PK_BODY_transform_faces`-style move), creating isocline taper surfaces from polylines, and hollow/shell as a local-ops client. Face deletion is `PK_FACE_delete_2` with options controlling whether the wound is healed by growing neighbors or simply left as a sheet hole.

**Robustness.** Parasolid's documented value is precisely its handling of the hard cases: it supports a wide range of topology changes during a tweak (edges appearing, disappearing, faces splitting). The docs enumerate failure modes candidly: a tweak fails when the new surface no longer intersects a neighbor at all (so the bounding edge cannot be recomputed: think pushing a hole's wall outward until it no longer reaches the top/bottom caps), or when a tweak splits a planar face into two, which would create two disjoint solids. In those cases the documentation directs users to fall back to boolean operations.

**Limitations.** Surface replacement "cannot handle" cases that change connectivity globally (splitting a body, merging separate shells); these are out of scope by design and pushed to booleans. The local-ops promise is *local*: bounded face neighborhood, bounded recompute.

**Kernel relevance.** This is the reference target. Keel should expose a `change_face_surfaces(faces, new_surfaces)` primitive that drives a generic edge/vertex regeneration pass over the 1-ring of adjacent faces, with offset/taper/move/delete all implemented as thin wrappers that supply the new surfaces and a topology-change policy. The explicit failure taxonomy (no-intersection, body-split, face-split-into-solids) should be Keel's error enum from day one.

### 1.2 ACIS Local Operations Component (LOP) and the tweak algorithm

**Citation.** Spatial Corp. (Dassault Systemes). *ACIS 3D Modeler: Local Operations Component (LOP) and Remove Faces (REM) Reference*, R10 and later. Spatial / Arizona ISL mirror (www-isl.ece.arizona.edu/ACIS-docs).

**Method.** ACIS states the design principle directly: "The tweak algorithm is the foundation for all other local operations." `api_tweak_faces` changes the surfaces of any number of faces to any other user-supplied surfaces, "providing that the surfaces intersect appropriately and that necessary topology changes are supported." A companion `lop:tweak-faces-extend` (`api_tweak_faces` with extension) extrapolates surfaces as necessary so neighbors that would otherwise miss can still be re-intersected. On top of tweak, ACIS layers `api_offset_faces`, `api_taper_faces`, `api_move_faces`, and the Remove Faces component `api_remove_faces`, which "removes faces by growing adjacent faces to fill the gap." Blends get special treatment: when LOP encounters faces flagged as blends, it preserves the blend property and "recalculates the blend geometry so that it remains tangent to its supporting surfaces," but with the constraint that "the topology of the blend cannot change."

**Robustness.** ACIS's tweak shares Parasolid's failure surface: appropriate intersection must exist; topology change must be of a supported kind. The extend variant widens the success envelope at the cost of extrapolation risk (see Theme 7).

**Limitations.** Blend recalculation cannot change blend topology; complex chains still require the dedicated blend remove path. Tweak success is gated on surface intersectability, which fails silently on near-tangent or grazing configurations.

**Kernel relevance.** ACIS confirms the architecture: build one robust tweak/reintersect core, express everything else as a surface-supplier plus a topology policy. The blend-aware tweak (keep tangency, freeze topology) is a specific behavior Keel should replicate for "edit a body that still has its fillets" workflows.

### 1.3 OpenCASCADE local-ops equivalents (the gap to beat)

**Citation.** Open Cascade SAS. *Open CASCADE Technology Documentation: BRepAlgoAPI_Defeaturing and BOPAlgo_RemoveFeatures*, OCCT 7.3+. dev.opencascade.org.

**Method.** OCCT's nearest equivalent to delete-face-with-healing is `BRepAlgoAPI_Defeaturing`, a thin wrapper over the low-level `BOPAlgo_RemoveFeatures`. It removes unwanted parts (holes, protrusions, gaps, chamfers, fillets) by deleting the selected faces and rebuilding a *new* shape (the input is never mutated). Only `SOLID`, `COMPSOLID`, and `COMPOUND`-of-solids inputs are accepted, and "only FACEs can be removed." The reconstruction reuses OCCT's general boolean/builder machinery to extend and reglue neighbors.

**Robustness.** Because it is built on the boolean builder rather than a dedicated local-ops engine, it inherits boolean fragility and is documented as having "the same options, input data requirements, and limitations" as the low-level tool. It has no true tweak/replace-surface primitive comparable to Parasolid or ACIS.

**Limitations.** No general face-surface replacement, no offset-face-with-reintersect, no taper-as-local-op, no blend-chain removal that exploits blend structure. OCCT users routinely report defeaturing failures on real imported geometry.

**Kernel relevance.** This is the bar Keel must clear to be more than "another OCCT." A purpose-built tweak core (not a boolean wrapper) is the single biggest local-ops differentiator available to an open-source kernel.

---

## Theme 2: Face deletion, wound healing, and defeaturing

### 2.1 Thakur, Banerjee & Gupta: the defeaturing taxonomy

**Citation.** Thakur, A., Banerjee, A. G., & Gupta, S. K. (2009). A survey of CAD model simplification techniques for physics-based simulation applications. *Computer-Aided Design, 41*(2), 64-80.

**Method.** The canonical survey. It classifies simplification/defeaturing into four operator families: (1) **surface-entity-based operators** (delete faces and heal by extending neighbors, suppress blends, collapse small faces); (2) **volume-entity-based operators** (identify a feature's volume and add/subtract it via boolean, for example fill a hole with its negative cylinder); (3) **explicit-feature-based operators** (recognize a parametric/semantic feature and suppress it through its construction history); and (4) **dimension-reduction operators** (midsurface/beam idealization). For surface-based delete-and-heal, the survey describes the dominant pattern: remove the target face set, then extend the surrounding "external" faces until they re-intersect and stitch the hole closed; where extension cannot close the wound, fall back to a cap surface or a volumetric boolean.

**Robustness.** The survey's lasting contribution is taxonomizing *when* each family works. Surface operators are cheapest and most local but fail when neighbors cannot be extended to meet. Volume operators are more robust (booleans close any wound) but heavier and can perturb more topology. The right engine picks per-feature.

**Limitations.** A 2009 survey, so it predates ML feature recognition; its strength is the operator taxonomy, not specific algorithms.

**Kernel relevance.** Keel's defeaturing/local-ops layer should implement *both* the surface (extend-reintersect) and volume (boolean-fill) paths and choose adaptively, exactly as the survey recommends. The four-way taxonomy is a clean module decomposition.

### 2.2 Venkataraman, Sohoni & Rajadhyaksha: removal of blends from B-rep models

**Citation.** Venkataraman, S., Sohoni, M., & Rajadhyaksha, R. (2002). Removal of blends from boundary representation models. In *Proceedings of the 7th ACM Symposium on Solid Modeling and Applications (SM '02)* (pp. 83-94). ACM. (Related: Venkataraman, Sohoni & Elber, 2001, *6th ACM SM*, 99-108, blend recognition.)

**Method.** The seminal blend-deletion paper, and the intellectual core of healing. Rather than blindly deleting blend faces and hoping neighbors reintersect, it *uses the blend's own structure to predict the post-removal topology*. It first recognizes all blend faces, then classifies the blend region into **edge-blend features** (a fillet running along one edge, typically cylindrical/swept), **vertex-blend features** (a spherical/triangular patch where three or more edge-blends meet), and **mixed-blend regions**, which it further decomposes into pure edge-blend and pure vertex-blend parts. Each recognized blend feature is then suppressed *as a whole* using **Euler operators** that delete the blend faces and reconnect the support faces, after which the support surfaces are extended and re-intersected to recover the original sharp edge or vertex. Because the target topology is predicted from blend structure, the reintersection produces fewer spurious intersections and is far more predictable than naive face-deletion.

**Robustness.** Predicting topology from blend structure is what makes the failure rate tractable. The paper handles awkward configurations: blends-on-blends, "cliff" blends, and degenerate vertex regions, with explicit degeneracy handling. The face-deletion fallback exists for cases where the structural prediction does not apply.

**Limitations.** Requires reliable upstream blend recognition; struggles when blends were modeled by unusual constructions; very large/variable-radius blends and setback vertex blends remain hard.

**Kernel relevance.** This is the algorithm Keel's `remove_blends` should be modeled on: recognize, classify (edge/vertex/mixed), suppress whole features via Euler operators, then extend-and-reintersect supports. The structural-prediction insight (do not reintersect blindly) directly improves robustness over a generic delete-face.

### 2.3 US Patent 7,236,168: Method for removing blends in B-rep models

**Citation.** Sequin et al. / assignee (UGS / Siemens lineage). (2007). *Method for removing blends in B-rep models* (U.S. Patent No. 7,236,168). U.S. Patent and Trademark Office. (Patent text is a scanned image; analysis from claims abstract and corroborating literature.)

**Method.** A productized blend-removal pipeline of the Venkataraman-Sohoni family: detect blend faces and assemble them into chains, remove the chain's faces, regrow/extend the adjacent support faces, and recompute the intersection edges/vertices to restore unblended geometry, with handling for chains that terminate at vertex blends. Patents are cited here because they are often the only public record of the exact production heuristics inside commercial kernels.

**Robustness / Limitations.** As a patent it documents the method's existence and broad claim scope rather than measured failure rates; the corroborating academic work (2.2) supplies the robustness picture.

**Kernel relevance.** Confirms that the commercial state of the art for blend removal is chain-aware structural suppression plus extend-reintersect. Useful for prior-art awareness when Keel implements its own (clean-room) version.

### 2.4 Small-feature suppression / unsuppression for analysis

**Citation.** (Authors per ACM record.) (2005). A small feature suppression/unsuppression system for preparing B-rep models for analysis. In *Proceedings of the 2005 ACM Symposium on Solid and Physical Modeling (SPM '05)* (pp. 113-123). ACM.

**Method.** Targets the CAE-preparation use case: detect features below a size threshold (small holes, slots, blends, sliver faces), suppress them with the appropriate local op (delete-and-heal or boolean-fill), and critically support **unsuppression** (reversible restoration), so the analysis-idealized model can be brought back to the design model. Suppression order matters (suppress small blends before the faces they sit on).

**Robustness.** Reversibility forces clean bookkeeping of what was removed and how, which also aids recovery when a later suppression fails.

**Limitations.** Threshold-driven detection misses context-dependent "small but important" features.

**Kernel relevance.** Argues that Keel's local ops should record an undo/replay log (a removed-feature journal) so direct edits and defeaturing are reversible, which dovetails with the history-free-but-recoverable goal of direct modeling.

### 2.5 Removing small features with real CAD operations

**Citation.** (Authors per the ResearchGate record.) Removing small features with real CAD operations. (CAD/CAE simplification literature, late 2000s.)

**Method.** Argues that defeaturing should be performed with *the kernel's own local operations* (delete-face, blend removal, tweak) rather than mesh-level edits, so the simplified result is a valid B-rep usable downstream. Reinforces the surface-operator path: delete the feature's faces, extend neighbors, reintersect.

**Robustness / Limitations.** Real-CAD-op defeaturing is only as robust as the kernel's local ops; on dumb imported geometry it inherits all the extend-reintersect failure modes.

**Kernel relevance.** Validates that Keel's defeaturing should be a *client* of its local-ops core, not a separate mesh pipeline, keeping one robust code path.

---

## Theme 3: Direct modeling and synchronous technology

### 3.1 Siemens Synchronous Technology white paper

**Citation.** Siemens PLM Software / CPDA. (2008). *Synchronous Technology* (white paper). Siemens PLM. Plus Siemens community articles "Synchronous Technology and Live Rules" and the Solid Edge hybrid-modeling blog.

**Method.** Synchronous Technology (ST) is direct modeling built directly on the Parasolid B-rep: a *synchronous feature is a collection of faces that defines a shape, with no history of how it was created*, and the user edits those faces. The push-pull edit moves/offsets a selected face set; ST then drives Parasolid local operations (tweak, offset, move-faces) to regenerate the neighboring geometry. The intelligence layer is **Live Rules**: at edit time ST *infers* geometric relationships from the current B-rep (coplanarity, parallelism, perpendicularity, concentricity, symmetry, tangency, equal radius) and applies them as live constraints so that, for example, faces that started perpendicular to the base planes stay perpendicular, and a hole concentric with a boss stays concentric. These relations are solved by the **D-Cubed 3D Dimensional Constraint Manager (3D DCM)**, Siemens' constraint solver, operating on the B-rep faces *at the moment of the edit* rather than from stored history. The user can toggle individual Live Rules on/off to steer intent, and "face relate" commands add explicit 3D relationships akin to 2D sketch relations.

**Robustness.** The on-the-fly inference is what makes history-free editing feel intelligent: instead of breaking distant geometry, the solver propagates the edit through inferred relations. When inference is wrong, the user toggles rules.

**Limitations.** Inference is heuristic; ambiguous models produce surprising propagation. ST also does not retain feature semantics (a "through hole" is just faces), so semantic intent must be re-inferred each edit.

**Kernel relevance.** This is the blueprint for Keel's direct-edit story: (a) a robust local-ops core (tweak/offset/move), plus (b) a *runtime constraint inference + solver* layer that detects relations on the live B-rep and re-imposes them after the geometric edit. A 3D constraint solver (the D-Cubed analog) is a required companion subsystem, not optional.

### 3.2 SpaceClaim and push-pull semantics

**Citation.** SpaceClaim Corp. / Ansys. *SpaceClaim direct modeling* (product documentation and practitioner writeups); referenced in the Variational Direct Modeling comparison (Zou et al., 2023).

**Method.** SpaceClaim popularized the four-tool direct-edit gesture vocabulary: **Pull** (push-pull a face to offset/extrude/round), **Move** (translate/rotate a face set with the body following), **Fill** (the delete-face-and-heal operation, the everyday name for wound healing), and **Combine** (boolean). Each gesture maps onto kernel local ops; "Fill" is literally delete-face plus extend-reintersect. SpaceClaim infers a minimal set of live relations (it is lighter-touch than ST's Live Rules) and emphasizes immediate, gesture-driven editing of dumb imported geometry.

**Robustness.** In the Zou et al. benchmark of topology-changing edits, SpaceClaim succeeded on a smaller fraction of hard cases than the research prototype, reflecting that even mature direct modelers fail when an edit forces topology changes the engine cannot decompose.

**Limitations.** Lighter constraint inference means less automatic intent preservation than ST; complex topology-changing pulls still fail.

**Kernel relevance.** The Pull/Move/Fill/Combine vocabulary is a clean public API surface for Keel's direct-edit layer, each backed by the same tweak/heal/boolean primitives.

### 3.3 Synchronous vs ordered (hybrid) and partial B-rep conversion

**Citation.** Siemens. *Hybrid modeling in Solid Edge with synchronous and ordered modes* (blog) and *Examining Solid Edge Synchronous Technology from a Parametric Point of View* (Digital Engineering 24/7).

**Method.** Production CAD does not abandon history; it runs hybrid. When switching from parametric (ordered) to direct (synchronous) editing, Solid Edge does a *partial* conversion: only direct-edit-targeted features are baked to B-rep faces, while other features retain history. Edits to synchronous geometry propagate to dependent ordered features without recreation.

**Robustness / Limitations.** Hybrid avoids the all-or-nothing cost of fully flattening history, but the boundary between baked and history-retained features must be tracked carefully (persistent naming, Theme 6).

**Kernel relevance.** Suggests Keel should not force a binary history/no-history choice: support direct edits on a sub-body while preserving a feature tree elsewhere, requiring robust persistent face/edge identifiers across local ops.

---

## Theme 4: Blend recognition and removal

(Venkataraman-Sohoni and the patent in Theme 2 are the removal core; this theme adds recognition.)

### 4.1 Zhu & Menq: B-rep simplification by automatic fillet/round suppression

**Citation.** Zhu, H., & Menq, C. H. (2002). B-rep model simplification by automatic fillet/round suppressing for efficient automatic feature recognition. *Computer-Aided Design, 34*(2), 109-123.

**Method.** A foundational recognition+suppression method. It recognizes blend candidates by surface type (cylindrical for edge-fillets, toroidal for variable/edge-meeting fillets, spherical for vertex blends) plus tangency to two support faces. Recognized blends are removed and the supports extended to form the original intersection edge ("extending adjacent faces to form an intersection edge representing the original sharp edge before filleting"). Suppression is ordered so that smaller/dependent blends go first.

**Robustness.** Surface-type plus tangency tests are cheap and reliable for *standard* constant-radius blends; the method explicitly targets making downstream feature recognition tractable by stripping blends first.

**Limitations.** Surface-type heuristics misfire on blends modeled as general NURBS, on large-radius blends that look like primary faces, and on setback/variable blends.

**Kernel relevance.** Provides Keel's default rule-based blend detector: classify by surface type + dual-tangency, order the chain, hand to the structural suppressor of 2.2.

### 4.2 Cui & Gao: efficient recognition and suppression of blend features

**Citation.** Cui, X., Gao, S., & colleagues. (c. 2010s). An efficient algorithm for recognizing and suppressing blend features. (CAD simplification literature; Semantic Scholar / ResearchGate.)

**Method.** Improves global efficiency: recognizes blend faces, groups *all* entities of a blend feature, and suppresses the whole feature in a single global step rather than face-by-face, "to improve suppression efficiency," and claims to handle blend faces generated by various methods (not just primitive surface types).

**Robustness.** Global whole-feature suppression reduces the intermediate-state intersections that cause face-by-face methods to fail.

**Limitations.** Still recognition-dependent; broad-method coverage is claimed more than exhaustively proven.

**Kernel relevance.** Reinforces whole-feature (not face-at-a-time) suppression as the robust default.

### 4.3 Automatic recognition and suppression of blend chains

**Citation.** (Authors per Springer record.) (2020). Simplification of CAD models by automatic recognition and suppression of blend chains. *Programming and Computer Software, 46*(3). Springer.

**Method.** Focuses on **chain detection and ordering**: a fillet rarely lives alone; it forms chains and networks meeting at vertex blends. The method traces connected blend faces into chains, determines a safe suppression order (leaves before junctions, vertex blends after their incident edge blends), and suppresses with extend-reintersect healing.

**Robustness.** Correct ordering is the difference between clean removal and cascading reintersection failures; this paper's contribution is the ordering logic.

**Limitations.** Network topologies with cycles or interdependent blends remain edge cases.

**Kernel relevance.** Keel's blend remover needs an explicit chain-graph builder and topological-sort suppression order, not a flat face list.

### 4.4 FilletRec: lightweight GNN for fillet recognition

**Citation.** Gao, J., Liu, T., Ye, H., & Chen, J. (2025). *FilletRec: A lightweight graph neural network with intrinsic features for automated fillet recognition* (arXiv:2511.05561). Zhejiang University.

**Method.** Learning-based recognition. Converts the B-rep to an undirected graph (nodes = NURBS surfaces, edges = adjacency), attaches *intrinsic, rigid-motion-invariant* node features sampled on a 5x5 parametric grid: Gaussian and mean curvature (from the fundamental forms), face width (distance between midpoints of the two longest boundary edges), and dihedral angle to neighbors. A tiny 3-layer GCN (only 0.02M parameters) performs per-face binary fillet/non-fillet classification with step-wise fusion of curvature/width/angle. Trained on 4,486 STEP files in three fillet classes (uniform-radius, variable-radius, irregular). Reports 99.9% on its dataset and 100% on six complex ABC models, beating AAGNet and Hierarchical CADNet. It pairs recognition with an **Extend-Intersect-Clean** removal pipeline: extend the boundary mesh of the holes outward to overlap, resolve self-intersections with robust mesh booleans to recover the sharp edge, then clean non-manifold/isolated geometry.

**Robustness.** Data-driven recognition handles fillet chains and irregular blends that defeat hand-tuned thresholds; intrinsic features give transformation invariance. The mesh-boolean Extend-Intersect-Clean is robust to the near-tangent cases that break exact-surface reintersection.

**Limitations.** Cannot reliably recognize large-radius fillets, fails on fillets between parallel planes, and requires clean B-rep input. Mesh-boolean healing yields a tessellated, not exact, result.

**Kernel relevance.** Suggests a two-tier blend detector for Keel: fast rule-based (Zhu-Menq) by default, with an optional learned recognizer (exportable to ONNX, callable from Rust via `ort`/`tract`) for dumb/irregular geometry. The mesh-based Extend-Intersect-Clean is a valuable *fallback healer* when exact reintersection fails.

---

## Theme 5: Feature recognition adjacent to editing

Direct editing needs to find *editable face sets* (the faces of "a hole", "a slot", "a boss") before it can offer a sensible push-pull. That is feature recognition.

### 5.1 Joshi & Chang: the Attributed Adjacency Graph (AAG)

**Citation.** Joshi, S., & Chang, T. C. (1988). Graph-based heuristics for recognition of machined features from a 3D solid model. *Computer-Aided Design, 20*(2), 58-66.

**Method.** The origin of graph-based feature recognition. Build the AAG: one node per face, one arc per shared edge; label nodes with face attributes (type, etc.) and label each arc **convex or concave** based on the dihedral angle across the edge. Machining features (pockets, slots, steps, blind steps/slots, polyhedral holes) appear as characteristic subgraphs, recognized by deleting all convex-edge arcs (concave-edge connected components isolate cavity features) and matching the residual subgraph patterns with heuristics.

**Robustness.** Convex/concave edge labeling is a remarkably durable signal that still underlies modern methods; the delete-convex-arcs heuristic cleanly separates protrusions from depressions.

**Limitations.** Original scope is polyhedral features only; intersecting features (where one feature's faces are partly consumed by another) break the clean subgraph patterns, the classic open problem.

**Kernel relevance.** The AAG (face graph + convex/concave edge labels) is the right base data structure for Keel's editable-set finder. Convexity labeling on every edge is cheap to compute and reusable across feature recognition, blend recognition, and chamfer detection.

### 5.2 Vandenbrande & Requicha; Gao & Shah: hint-based recognition

**Citation.** Vandenbrande, J. H., & Requicha, A. A. G. (1993). Spatial reasoning for the automatic recognition of machinable features in solid models. *IEEE TPAMI, 15*(12), 1269-1285. And Gao, S., & Shah, J. J. (1998). Automatic recognition of interacting machining features based on minimal condition subgraph. *Computer-Aided Design, 30*(9), 727-739.

**Method.** Addresses AAG's Achilles heel (intersecting features). Hint-based recognition looks for *traces* a feature must leave even when partially destroyed by interaction (a "hint", e.g. a pair of opposing parallel faces hints a slot), then completes the feature volume by geometric reasoning. Gao & Shah's minimal-condition-subgraph (MCSG) formalizes the minimal graph evidence sufficient to assert a feature, recovering features whose AAG signature is incomplete.

**Robustness.** Far more robust to feature interaction than pure subgraph isomorphism; degrades gracefully (a partial hint still localizes the editable region).

**Limitations.** More complex, more geometric reasoning, slower; hint libraries are feature-vocabulary-specific.

**Kernel relevance.** For direct editing on messy imported parts, hints (opposing-face pairs, concave loops) are exactly what lets Keel offer "edit this slot" even when the slot intersects other features.

### 5.3 Modern ML feature recognition (FeatureNet, BRepNet, UV-Net)

**Citation.** Zhang, Z., et al. (2018). FeatureNet (3D-CNN, 24 machining features). Lambourne, J. G., et al. (2021). BRepNet: A topological message passing system for solid models. *CVPR*. Jayaraman, P. K., et al. (2021). UV-Net: Learning from boundary representations. *CVPR*. Plus Hierarchical CADNet, BRepGAT, BrepMFR (2022-2024).

**Method.** FeatureNet voxelizes and applies a 3D-CNN. UV-Net encodes each curve/surface by sampling its U/V parameter domain into an image fed to a CNN, then runs a GCN over the face-adjacency graph. BRepNet convolves directly on the B-rep, centering convolution on **coedges** and walking topological permutations (next/mate/previous) so it learns from native topology without meshing. These produce per-face feature labels.

**Robustness.** Learn from data, so they tolerate geometric variation and (BRepNet/UV-Net) operate without lossy meshing. State of the art exceeds 90-99% face-segmentation accuracy on benchmark sets.

**Limitations.** Static labels with no design context; need training data; opaque failure modes; generalization across CAD styles is imperfect (the motivation for domain adaptation in BrepMFR).

**Kernel relevance.** Keel can stay kernel-pure (rule-based recognition in core) while exposing the B-rep in a form (coedge graph + UV samples) that downstream ML recognizers consume. The coedge-centric convolution of BRepNet maps onto a winged/half-edge topology Keel will already have.

---

## Theme 6: Parametric re-evaluation vs direct edits

### 6.1 Zou, Feng & Gao: Variational Direct Modeling (VDM)

**Citation.** Zou, Q., Feng, H.-Y., & Gao, S. (2023). Variational direct modeling: A framework towards integration of parametric modeling and direct modeling in CAD. *Computer-Aided Design, 157*, 103465 (arXiv:2301.02999).

**Method.** The most rigorous public account of *what breaks* when you direct-edit a constrained model and how to fix it. A CAD model has three information layers: geometry, topology, and the geometric constraint system (GCS). A direct edit (push-pull) changes geometry, leaving two inconsistencies: **Geometry-Topology Inconsistency (GTI)**, where the new geometry no longer matches the old topology (faces that should meet now miss, or faces that should not now collide), and **Shape-Associativity Inconsistency (SAI)**, where the new geometry makes the constraints over- or under-determined. VDM is a detect-then-resolve framework. For GTI it *decomposes the user edit into a sequence of small sub-edits* and, at each **GTI point** (where a degenerate configuration arises: surfaces become tangent, intersect, or collide), it repairs topology by constructing auxiliary volumes bounded by the neighbor faces and the swept boundary, then applies a boolean to keep the body valid and the shape varying continuously (Algorithm 1 exhaustively generates and filters degenerate-configuration candidates to find the next GTI point). For SAI it updates the GCS, then extracts the **minimal over-constrained** and **maximal well-constrained** parts using a witness-configuration method and an L0 sparse-recovery formulation (min ||x||_0 s.t. J^T x = 0, J the constraint Jacobian), and resolves by adding/removing constraints, ranked first by constraint-type precedence (e.g. parallelism over angle) then by a sensitivity (model-shape-change-rate) metric.

**Robustness.** Decomposing edits into sub-edits and repairing at each degenerate point is what lets it survive topology-changing pulls. In the authors' benchmark the prototype solved 8/8 hard edits where Siemens NX solved 3/8 and SpaceClaim 2/8, and 93% of 100 topology-changing edits vs 76% (NX) and 84% (SpaceClaim).

**Limitations.** Face-delete cannot be decomposed into sub-edits, so the framework fails for abrupt face removal (delete-face still needs the dedicated heal of Theme 2). Limited to linear/quadratic surfaces (no freeform yet). Boolean-heavy resolution is compute-intensive. Full automation of SAI prioritization "seems impossible"; some edits need user choice. Robust **persistent/topological naming** of renamed/deleted entities remains unsolved.

**Kernel relevance.** This is the design spec for Keel's direct-edit-on-constrained-bodies engine. Concrete takeaways: (1) decompose edits and repair topology incrementally at degenerate configurations; (2) carry a GCS alongside the B-rep and run over/under-constraint detection (sparse recovery) after each edit; (3) rank constraint repairs by type precedence then shape sensitivity; (4) treat delete-face as a *separate* primitive outside the decomposition framework; (5) invest early in persistent naming, because every higher-level feature depends on it.

### 6.2 Persistent naming / topological naming (the cross-cutting problem)

**Citation.** Capoyleas, V., Chen, X., & Hoffmann, C. M. (1996). Generic naming in generative, constraint-based design. *Computer-Aided Design, 28*(1), 17-26. (And the persistent-naming literature it seeded; the problem is restated as open in Zou et al., 2023.)

**Method.** Defines stable, regeneration-invariant identifiers for B-rep entities so that when the model is re-evaluated (parametric) or directly edited (faces split/merge/vanish), references like "the fillet on edge E" still resolve. Names are generated from invariant generative/topological context rather than transient index order.

**Robustness / Limitations.** No scheme is fully robust under arbitrary topology change (face splits into two: which is the original?); remains an active research problem.

**Kernel relevance.** Keel must assign persistent IDs to faces/edges/vertices and update them through every local op (split, merge, delete, heal). This underpins direct-edit selection, hybrid history, and Live-Rules re-application. It is foundational plumbing, not a feature.

---

## Theme 7: Surface extension / extrapolation for healing

Extend-and-reintersect is only as good as the surface extension it relies on. Extrapolating a NURBS surface to meet a neighbor is the riskiest numerical step in local ops.

### 7.1 Shetty & White / Hartwig: extrapolation methods for CAD

**Citation.** Shetty, S., & White, P. R. (1991). Curvature-continuous extensions for rational B-spline curves and surfaces. *Computer-Aided Design, 23*(7), 484-491. And the survey "Extensions: Extrapolation methods for CAD."

**Method.** Catalogs how to extend a B-spline/NURBS curve or surface past its parameter domain. The simplest is **natural extension**: extend via the de Boor recurrence, where the new control points are the intermediate de Boor points evaluated at the extended parameter; this yields maximal (Ck) continuity with the original at the join. Curvature-continuous variants add control points to maintain G2/C2. Curve unclamping is an alternative that opens the end knot multiplicity to expose extendable control points.

**Robustness.** Natural extension is exact and continuity-preserving for *short* extensions. The literature is blunt: "extrapolation can in general result in unpredictable results," so extension "should be used for relatively short extensions." Far extrapolation amplifies curvature and can fold the surface back on itself or oscillate.

**Limitations.** No bound guarantees a long extension stays well-behaved; trimmed/degenerate surfaces extend poorly; rational weights can blow up.

**Kernel relevance.** Keel's reintersect step must (a) extend neighbor surfaces by the *minimum* amount needed to reach the new intersection, (b) bound the extension and bail to a fallback (cap surface or boolean fill) when the required extension is large, and (c) prefer analytic extension for analytic surfaces (a plane/cylinder/cone/sphere extends trivially and exactly: the dominant, safe case in mechanical parts).

### 7.2 Stable extension operators for trimmed spline spaces

**Citation.** (Authors per arXiv.) (2022). *Extension operators for trimmed spline spaces* (arXiv:2205.06543).

**Method.** From isogeometric analysis: defines extension operators that eliminate ill-conditioned "small" degrees of freedom near a trim boundary by expressing them in terms of well-conditioned "large" DOFs, with proven approximation and stability and optimal-order accuracy.

**Robustness.** Provides *provable* stability where ad-hoc extrapolation only has heuristics, by controlling the conditioning of the basis near trims.

**Limitations.** Developed for analysis (stable bases for IGA) rather than for producing a watertight B-rep edge; adapting to modeling reintersection is non-trivial.

**Kernel relevance.** Points to a principled, conditioning-aware extension for Keel's trimmed-surface healing, an upgrade path beyond naive de Boor extrapolation when robustness on freeform geometry matters.

---

## Theme 8: Midsurface and thin-wall-aware editing

### 8.1 Midsurface abstraction for thin-walled parts

**Citation.** Robinson, T. T., Armstrong, C. G., et al. (mid-surface and dimensional-reduction literature); plus Chong, C. S., et al.; and recent work: *MidSurfer: Efficient mid-surface abstraction from variable thin-walled models* (2025, *Computer-Aided Design*) and divide-and-conquer / chordal-axis-transform approaches.

**Method.** For thin-walled plastic/sheet-metal parts, the analysis-relevant idealization is the **midsurface**: a non-manifold sheet body lying midway between opposing wall faces, meshed with shell elements. Methods include medial axis transform (MAT) / chordal axis, face-pairing (identify opposing face pairs, generate the surface midway and trim/extend to stitch the sheet at junctions and ribs), and rib-decomposition hierarchies. The hard part is junctions: where walls of different thickness meet, the midsurfaces must be extended and reintersected to close the sheet, the same extend-reintersect machinery used in local ops, now applied to derived surfaces.

**Robustness.** Face-pairing is reliable on constant-thickness regions; T-junctions, ribs, and variable thickness force extend-and-trim that frequently leaves gaps requiring repair (the recurring wound-healing problem).

**Limitations.** Fully automatic midsurfacing of complex variable-thickness parts is still unsolved; commercial tools leave gaps for manual cleanup.

**Kernel relevance.** Midsurfacing is "local ops on a derived sheet body". A kernel with a strong extend/trim/reintersect core gets midsurface extraction (and shell/offset operations) largely for free. Keel should expose non-manifold sheet bodies and the same healing primitives so that thin-wall idealization and shell-aware edits reuse the local-ops engine rather than a separate pipeline.

---

## Robustness of local operations: cross-cutting failure analysis

Synthesizing the failure modes documented across Parasolid, ACIS, OCCT, Venkataraman-Sohoni, and Zou et al.:

1. **No-intersection failure.** After a tweak/offset, an extended neighbor surface no longer meets the edited surface, so the bounding edge is undefined. Parasolid and ACIS report this and direct the user to booleans. Mitigation: bound extension length and fall back to cap/boolean.
2. **Topology-change failure.** The edit forces a topology change the engine cannot perform: a face splits into pieces that would create two solids, a loop must split, an edge must appear/vanish. Parasolid supports many such changes; the open frontier (Zou et al.) is doing them *automatically and continuously* by decomposing the edit and repairing at each degenerate point.
3. **Near-tangent / grazing intersection.** Surfaces meet at a shallow angle, so the intersection curve is numerically ill-conditioned and tolerance-sensitive. Mesh-boolean fallback (FilletRec's Extend-Intersect-Clean) trades exactness for robustness here.
4. **Extrapolation instability.** Long NURBS extensions fold or oscillate (Theme 7). Mitigation: analytic extension where possible, minimal extension length, conditioning-aware operators.
5. **Tolerance accumulation.** Each reintersection introduces tolerance; chains of local ops drift. Mitigation: persistent tolerant geometry and periodic re-snap.
6. **Delete-face is special.** It cannot be decomposed into incremental sub-edits (Zou et al.), so it needs the dedicated structural heal of Theme 2, not the general direct-edit engine.

---

## Local operations architecture for Keel

Pulling the threads together into concrete guidance for an open-source Rust kernel:

**1. Build one tweak/reintersect core, layer everything on it.** Both Parasolid and ACIS state explicitly that tweak (change-face-surfaces) is the foundation and offset, taper, move, delete-heal, and blend-removal are clients. Keel's core primitive is `change_face_surfaces(faces, new_surfaces, topology_policy)` driving a generic 1-ring edge/vertex regeneration. This is the single biggest differentiator versus OCCT, whose defeaturing is a boolean wrapper with no real tweak.

**2. Make the extend-reintersect step tiered.** (a) analytic-on-analytic intersection first (planes/cylinders/cones/spheres: the common, exact, safe case); (b) minimal NURBS extension via de Boor natural extension, bounded; (c) conditioning-aware trimmed extension for freeform; (d) mesh-boolean Extend-Intersect-Clean as the robust last resort. Bound every extension and bail to boolean-fill rather than producing garbage.

**3. Delete-face / heal is a dedicated primitive.** Implement the surface path (extend neighbors, reintersect, stitch) and the volume path (boolean-fill with the feature's negative volume), choosing adaptively per Thakur et al. Treat it as separate from the constraint-decomposition engine, because (Zou et al.) it cannot be decomposed.

**4. Blend removal exploits blend structure.** Recognize (rule-based Zhu-Menq by surface type + dual-tangency, optional learned FilletRec for dumb geometry), build the chain graph, classify edge/vertex/mixed (Venkataraman-Sohoni), suppress whole features via Euler operators in topological-sort order, then extend-reintersect supports. Do not delete blend faces blindly.

**5. Direct-edit layer = local ops + a 3D constraint solver.** Match Synchronous Technology: a Pull/Move/Fill/Combine gesture vocabulary (SpaceClaim) over the tweak core, plus a Live-Rules-style runtime inference of geometric relations (coplanar, parallel, perpendicular, concentric, tangent, equal-radius) re-imposed by a 3D constraint solver after each geometric edit. Keel needs a D-Cubed-class solver as a companion subsystem.

**6. Constrained edits use the VDM detect-then-resolve loop.** Decompose edits into sub-edits, detect degenerate (GTI) configurations, repair topology incrementally with auxiliary-volume booleans, and run over/under-constraint detection (sparse-recovery) on the GCS after each edit, ranking repairs by constraint-type precedence then shape sensitivity.

**7. Persistent naming is foundational plumbing.** Assign stable IDs to every face/edge/vertex and maintain them through split/merge/delete/heal. Everything above (direct-edit selection, hybrid history, Live-Rules re-application, undo/replay) depends on it. Start it on day one; retrofitting is painful.

**8. Recognition feeds editing.** Compute the AAG (face graph + convex/concave edge labels) once and reuse it for editable-set finding, blend recognition, and chamfer detection. Add hint-based reasoning (opposing-face pairs, concave loops) for messy imported parts, and expose a coedge+UV-sample view so external ML recognizers (BRepNet/UV-Net style) can plug in without polluting the core.

**9. Reuse the healing core for derived geometry.** Shell, offset-body, and midsurface extraction are all extend/trim/reintersect on derived surfaces. Support non-manifold sheet bodies so thin-wall idealization reuses the local-ops engine.

**10. Adopt the published failure taxonomy as the error model.** No-intersection, topology-change-unsupported, near-tangent, extrapolation-out-of-bounds, body-would-split: make these explicit, recoverable errors with boolean-fallback paths, exactly as Parasolid and ACIS document.

---

## References

1. Capoyleas, V., Chen, X., & Hoffmann, C. M. (1996). Generic naming in generative, constraint-based design. *Computer-Aided Design, 28*(1), 17-26.
2. Cui, X., Gao, S., et al. (c. 2010s). An efficient algorithm for recognizing and suppressing blend features. (CAD simplification literature.)
3. Gao, J., Liu, T., Ye, H., & Chen, J. (2025). *FilletRec: A lightweight graph neural network with intrinsic features for automated fillet recognition* (arXiv:2511.05561).
4. Gao, S., & Shah, J. J. (1998). Automatic recognition of interacting machining features based on minimal condition subgraph. *Computer-Aided Design, 30*(9), 727-739.
5. Jayaraman, P. K., et al. (2021). UV-Net: Learning from boundary representations. *CVPR 2021*.
6. Joshi, S., & Chang, T. C. (1988). Graph-based heuristics for recognition of machined features from a 3D solid model. *Computer-Aided Design, 20*(2), 58-66.
7. Lambourne, J. G., et al. (2021). BRepNet: A topological message passing system for solid models. *CVPR 2021*.
8. *MidSurfer: Efficient mid-surface abstraction from variable thin-walled models.* (2025). *Computer-Aided Design.*
9. Open Cascade SAS. *OCCT Documentation: BRepAlgoAPI_Defeaturing and BOPAlgo_RemoveFeatures* (7.3+). dev.opencascade.org.
10. Robinson, T. T., Armstrong, C. G., et al. Mid-surface abstraction and dimensional-reduction literature for thin-walled FEA.
11. Sequin et al. / assignee (UGS/Siemens). (2007). *Method for removing blends in B-rep models* (U.S. Patent No. 7,236,168).
12. Shetty, S., & White, P. R. (1991). Curvature-continuous extensions for rational B-spline curves and surfaces. *Computer-Aided Design, 23*(7), 484-491.
13. Siemens PLM Software / CPDA. (2008). *Synchronous Technology* (white paper).
14. Siemens Digital Industries Software. *Parasolid Functional Description Ch. 18: Local Operations* and *PK Interface Programming Reference* (v12-v35).
15. *A small feature suppression/unsuppression system for preparing B-rep models for analysis.* (2005). *ACM SPM '05*, 113-123.
16. Spatial Corp. (Dassault Systemes). *ACIS Local Operations Component (LOP) and Remove Faces (REM) Reference* (R10+).
17. *Extension operators for trimmed spline spaces.* (2022). arXiv:2205.06543.
18. Thakur, A., Banerjee, A. G., & Gupta, S. K. (2009). A survey of CAD model simplification techniques for physics-based simulation applications. *Computer-Aided Design, 41*(2), 64-80.
19. Vandenbrande, J. H., & Requicha, A. A. G. (1993). Spatial reasoning for the automatic recognition of machinable features in solid models. *IEEE TPAMI, 15*(12), 1269-1285.
20. Venkataraman, S., Sohoni, M., & Elber, G. (2001). Blend recognition algorithm and applications. *6th ACM Symposium on Solid Modeling and Applications*, 99-108.
21. Venkataraman, S., Sohoni, M., & Rajadhyaksha, R. (2002). Removal of blends from boundary representation models. *7th ACM Symposium on Solid Modeling and Applications (SM '02)*, 83-94.
22. Zhang, Z., et al. (2018). FeatureNet: Machining feature recognition based on 3D convolutional neural networks. *Computer-Aided Design.*
23. Zhu, H., & Menq, C. H. (2002). B-rep model simplification by automatic fillet/round suppressing for efficient automatic feature recognition. *Computer-Aided Design, 34*(2), 109-123.
24. *Simplification of CAD models by automatic recognition and suppression of blend chains.* (2020). *Programming and Computer Software, 46*(3).
25. Zou, Q., Feng, H.-Y., & Gao, S. (2023). Variational direct modeling: A framework towards integration of parametric modeling and direct modeling in CAD. *Computer-Aided Design, 157*, 103465 (arXiv:2301.02999).
