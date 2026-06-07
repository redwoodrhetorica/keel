# Feature Recognition in B-rep Models: Classical and Machine Learning

Research file 15 for the Keel kernel. This is the broad treatment of feature
recognition (FR) from boundary representation (B-rep) solid models. Prior
research (the blend recognition file) covered blend detection narrowly; here we
situate that work inside the full FR landscape and add the classical graph,
hint, and volumetric traditions plus the machine learning (ML) transition of
2018 to 2025.

## Scope and why Keel needs this

Feature recognition is the inverse of feature-based design: given a "dumb"
B-rep (a STEP import, a translated body, or the output of direct editing), it
recovers higher-level semantic shapes (holes, pockets, slots, ribs, blends,
patterns) and the faces that constitute them. Keel needs FR as a service layer
feeding five workflows:

1. Defeaturing for CAE: detect and suppress features smaller than the target
   mesh size before meshing, removing dense-mesh hotspots that add cost without
   accuracy.
2. Blend removal before editing: recognize fillet/round chains so they can be
   suppressed, the underlying sharp model edited, then reblended.
3. Direct-edit face-set selection: when a user grabs "the pocket," the kernel
   must map that intent to a coherent face set.
4. Machining feature extraction for CAM: holes, slots, pockets, steps with
   canonical parameters for process planning.
5. Design-intent reconstruction from imports: recover patterns, parametric
   holes, and editable feature trees from geometry alone.

FR is not one algorithm. It is a family with sharply different tradeoffs, and
the persistent hard problem across all of them, classical and ML alike, is
intersecting features, where two features share or destroy each other's faces
so neither appears as a clean pattern.

---

## Part 1: Classical feature recognition

### 1.1 Graph-based recognition

#### Joshi and Chang (1988): the Attributed Adjacency Graph (AAG)

**Citation.** Joshi, S., & Chang, T.-C. (1988). Graph-based heuristics for
recognition of machined features from a 3D solid model. *Computer-Aided
Design*, 20(2), 58-66.

**Method.** The founding graph-based paper. Build the AAG: a node for each face
of the B-rep, an arc between nodes whose faces share an edge, and a binary
attribute on each arc marking the shared edge as convex (attribute 1) or
concave (attribute 0). Feature recognition becomes subgraph search: a feature
type corresponds to a characteristic subgraph pattern of nodes connected by
concave arcs. The recognizer deletes all faces incident only to convex edges
(these tend to be the stock/base faces), leaving connected components of
concave-adjacent faces. Each component is matched against a small library of
feature subgraphs using heuristics on the number of faces and the arc-attribute
distribution to classify it as a pocket, slot, step, blind step, blind slot, or
polyhedral hole.

**Results.** Clean recognition of isolated polyhedral depression features
directly from topology, with no need to compute volumes. Fast and elegant for
the cases it handles.

**Limitations.** Restricted to planar polyhedral features. The fatal weakness
is intersecting features: a T-slot, or a hole breaking into a slot, destroys
the canonical subgraph (faces are split or merged, arcs change attribute), so
the exact pattern no longer matches and recognition silently fails. Curved
faces (cylinders for holes) need the AAG extended with face-type attributes.

**Kernel relevance.** The AAG is the canonical in-kernel data structure for FR
and Keel should expose it directly: a face-adjacency graph with per-arc
convex/concave/smooth/tangent classification and per-face surface-type tags.
Nearly every later method, classical and ML, is built on this graph. Computing
the convexity attribute robustly (a signed test using face normals and the edge
tangent) is a core interrogation primitive Keel must provide. This ties FR
performance directly to adjacency-traversal speed in the topology layer.

#### Kyprianou (1980): the precursor

**Citation.** Kyprianou, L. K. (1980). *Shape classification in computer-aided
design* (Doctoral dissertation). University of Cambridge.

**Method.** The earliest systematic FR work. Classified faces into primary and
secondary groups based on their loop structure and edge convexity, then
inspected the concave-edge graph to identify protrusions and depressions. It
introduced the convex/concave edge distinction that the AAG later formalized as
an arc attribute.

**Limitations.** Heuristic and tied to the GPM/BUILD modeller of the era;
limited feature vocabulary; superseded by the cleaner AAG formulation.

**Kernel relevance.** Historical anchor. Confirms that edge-convexity
classification is the irreducible primitive of topological FR.

#### Marefat and Kashyap (1990): cavity graphs and hypothesis reasoning

**Citation.** Marefat, M., & Kashyap, R. L. (1990). Geometric reasoning for
recognition of three-dimensional object features. *IEEE Transactions on Pattern
Analysis and Machine Intelligence*, 12(10), 949-965.

**Method.** Extended graph recognition toward intersecting features. Builds a
graph of the cavity (the depression volumes) and uses the edge-face topology to
generate hypotheses about features, then applies geometric reasoning and a
Dempster-Shafer evidential scheme to resolve competing interpretations when
features interact and the topology is ambiguous. This was an early
acknowledgement that intersecting features force the recognizer from pattern
matching into hypothesis-and-verify reasoning.

**Limitations.** Computationally heavy; the evidential machinery is complex and
the feature library still bounded. Polyhedral focus.

**Kernel relevance.** Foreshadows the modern neuro-symbolic stance: propose
multiple interpretations, then score and verify. Keel's FR layer should be
designed to return ranked hypotheses, not a single hard answer, exactly because
intersecting cases are genuinely ambiguous.

#### Graph grammar approaches

**Citation.** Representative: Fu, M.-W., et al., and the AI EDAM grammar
literature; see also Pinilla, Finger, & Prinz grammar work cited in surveys.

**Method.** Treat the feature library as a graph grammar: production rules
rewrite subgraphs of the face-adjacency graph into feature symbols. Recognition
is parsing. Grammars give a compact, extensible way to define families of
features and their compositions rather than enumerating subgraphs.

**Limitations.** Grammar design is hard, ambiguity (multiple parses) explodes
with intersection, and parsing cost grows with model size.

**Kernel relevance.** Of interest for a user-extensible feature definition
language in a layer above the kernel rather than in-kernel. Keel should make
the underlying graph queryable enough that a grammar engine can sit on top.

### 1.2 Hint-based and trace-based recognition

#### Vandenbrande and Requicha (1993): OOFF and the hint paradigm

**Citation.** Vandenbrande, J. H., & Requicha, A. A. G. (1993). Spatial
reasoning for the automatic recognition of machinable features in solid models.
*IEEE Transactions on Pattern Analysis and Machine Intelligence*, 15(12),
1269-1285.

**Method.** The Object-Oriented Feature Finder (OOFF) abandoned exact pattern
matching. The insight: when features intersect, most of a feature's boundary is
destroyed, but a minimal indispensable portion always survives. That surviving
evidence is a hint. For example, two opposing planar faces of opposite normal
are a hint for a slot; a cylindrical face is a hint for a hole. The recognizer
collects hints from geometry (and, importantly, from tolerances and design
attributes, which survive even when geometry does not), then for each hint runs
a generate-and-test cycle: hypothesize the largest feature consistent with the
hint, grow its volume, and verify it is producible (accessible by a tool,
non-intersecting with material that should remain). Hints are promoted to
features only after geometric verification.

**Results.** Recognized intersecting machinable features that defeated graph
subgraph matching, the first method to do so robustly.

**Limitations.** Computationally expensive: many hints generate traces that
never become valid features, and the generate-test-verify loop is costly.
Completeness depends on the hint catalogue.

**Kernel relevance.** This is the architecture Keel should adopt for robust FR.
The kernel provides cheap hint primitives (opposing-face queries, cylinder
detection, concave-edge loops) and an exact volumetric verification engine
(Booleans against the stock or the part); a layer above orchestrates the
generate-and-verify loop. Crucially, OOFF establishes that FR needs exact
geometry, not approximations, for the verification step, which is precisely
where an exact kernel beats a mesh/ML-only pipeline.

#### Han and Requicha (1998): IF2, the Integrated Incremental Feature Finder

**Citation.** Han, J., & Requicha, A. A. G. (1998). Integration of feature
based design and feature recognition. *Computer-Aided Design*, 30(5), 393-405.
See also Han, J., Regli, W. C., & colleagues, Hint-based reasoning for feature
recognition: status report. *Computer-Aided Design*, 30(13), 1003-1007 (1998).

**Method.** IF2 matured the hint paradigm into a working, incremental system.
It handles arbitrary spatial intersections between features (holes through
slots, slots crossing pockets) and integrates feature-based design with
recognition: as a design evolves, IF2 incrementally updates the recognized
feature set rather than re-recognizing from scratch. The status report
catalogues the hint-based family (IF2, F-Rex, FBMach) and argues that
hint-based reasoning is the only classical approach that scales to complex
intersections.

**Results.** Demonstrated incremental, intersection-robust recognition tightly
coupled to a feature-based modeller.

**Limitations.** Still costly; depends on rich attribute hints; system
complexity is high.

**Kernel relevance.** The incremental angle matters for Keel. If FR results are
attached to persistent face names (see file 07), the kernel can invalidate and
recompute only the features touched by an edit, supporting interactive
defeaturing and direct editing without full re-recognition.

#### Gao and Shah (1998): extended AAG with hints and minimal condition subgraphs

**Citation.** Gao, S., & Shah, J. J. (1998). Automatic recognition of
interacting machining features based on minimal condition subgraphs.
*Computer-Aided Design*, 30(9), 727-739.

**Method.** A landmark hybrid. Recognize isolated features with conventional
graph matching on a manufacturing face-adjacency graph, but for interacting
features use the Extended AAG (EAAG): when interactions destroy edges and faces,
add virtual links to the graph to restore the entities lost to intersection.
Each feature is characterized by its Minimal Condition Subgraph (MCSG), the
smallest subgraph that must survive for the feature to still be present, which
plays the role of a graph-encoded hint. Recognition decomposes the EAAG and
searches for MCSGs.

**Results.** Recognized many interacting feature configurations that pure AAG
matching missed, while keeping the efficiency of graph search for the easy
isolated cases.

**Limitations.** Virtual-link generation is heuristic and can be ambiguous;
combinatorics still grow with interaction density.

**Kernel relevance.** The MCSG concept directly informs Keel's API: a feature
definition should carry both its full subgraph and its minimal-condition
subgraph, so the recognizer can fall back to the robust minimal pattern under
intersection. This unifies the graph and hint traditions in one data structure.

#### Rahmani and Arezoo (2006): hybrid hint-and-graph framework

**Citation.** Rahmani, K., & Arezoo, B. (2006). Boundary analysis and geometric
completion for recognition of interacting machining features. *Computer-Aided
Design*, 38(8), 845-856. (See also their 2007 hybrid framework in *Computers in
Industry*.)

**Method.** Use cheap graph analysis to localize promising regions, then apply
the more expensive hint-based reasoning only there, with boundary analysis and
geometric completion to reconstruct destroyed feature boundaries. This is the
efficiency answer to OOFF's cost problem.

**Limitations.** Still bounded feature library; completion heuristics can
misfire.

**Kernel relevance.** Confirms the two-tier architecture: fast topological
triage in-kernel, expensive verification only where needed.

### 1.3 Volumetric decomposition

#### Woo (1982): Alternating Sum of Volumes (ASV)

**Citation.** Woo, T. C. (1982). Feature extraction by volume decomposition.
*Proceedings of the Conference on CAD/CAM Technology in Mechanical
Engineering*, MIT, 76-94.

**Method.** Decompose the part by repeatedly taking convex hulls. Compute the
convex hull of the solid, subtract the solid to get the deficiency (the
material missing from the hull, i.e. the cavities), take the convex hull of
each deficiency component, subtract again, and so on. The result is an
alternating sum of convex volumes: solid = hull1 - (hull2 - (hull3 - ...)).
Positive and negative volumes in the series correspond to protrusions and
depressions, a clean volumetric notion of features independent of surface
topology.

**Limitations.** ASV does not always converge: for certain non-convex shapes
the deficiency hulls oscillate and the recursion never terminates. This
non-convergence severely restricts the geometry it can handle.

**Kernel relevance.** Establishes the volumetric definition of a feature (a
maximal simple volume), complementary to the topological definition. Requires
robust convex-hull and exact Boolean difference, both core kernel services.

#### Kim (1992): ASVP, the convergence fix

**Citation.** Kim, Y. S. (1992). Recognition of form features using convex
decomposition. *Computer-Aided Design*, 24(9), 461-476. See also Kim, Y. S., &
Wilde, D. J. (1992). A convergent convex decomposition of polyhedral objects.
*ASME Journal of Mechanical Design*, 114(3), 468-476.

**Method.** Alternating Sum of Volumes with Partitioning (ASVP). Diagnose where
ASV fails to converge and insert a remedial partitioning step that cuts the
problematic component before the next hull, guaranteeing termination. ASVP was
formally proven convergent. The output is a hierarchical, convexity-based
volumetric decomposition of the part from its boundary, and form features are
recognized from this hierarchy. Kim also addressed incremental and localized
update of the decomposition so edits do not force a full recompute.

**Results.** A convergent, provably terminating convex decomposition usable for
real polyhedral parts; recognition of form features including some intersecting
cases via the volume hierarchy.

**Limitations.** Polyhedral emphasis; partitioning choices affect the feature
interpretation; curved geometry is hard.

**Kernel relevance.** ASVP is the reference algorithm if Keel offers volumetric
FR. It demands exact convex hull, Boolean difference, and a notion of
decomposition hierarchy, and Kim's localized-update work maps onto Keel's
incremental-recompute goal.

#### Sakurai and Dave (1996): maximal volume decomposition

**Citation.** Sakurai, H., & Dave, P. (1996). Volume decomposition and feature
recognition, part II: curved objects. *Computer-Aided Design*, 28(6/7),
519-537. (Part I, polyhedral: Sakurai, H. (1995). *Computer-Aided Design*,
27(11), 833-843.)

**Method.** Decompose the delta volume (the difference between stock/convex
hull and the part, i.e. all the removed material) into minimal cells by
extending each face through the part across its concave edges, then merge cells
into maximal volumes. A maximal volume is a large, simple volume with no concave
edge that is not contained in any other such volume. Many maximal volumes are
directly recognizable as features by graph matching; compound and intersecting
features are found by subtracting maximal volumes from each other in different
orders and re-matching, which generates multiple valid feature interpretations.
Machining rules can then sequence the volumes into a near-optimal process plan.
Part II crucially extends this to curved objects, not just polyhedra.

**Results.** Multiple-interpretation recognition of intersecting features
grounded in volume, with curved-surface support, a major advance over
polyhedral-only methods.

**Limitations.** Cell decomposition is expensive and explodes for large
face counts; the number of interpretations can be combinatorial.

**Kernel relevance.** The delta-volume / maximal-volume framing is exactly the
CAM machining-feature model and the right basis for a defeaturing volume
estimate. It places heavy demands on the kernel: robust face extension, cell
Booleans, and convex-edge analysis. The "multiple interpretations" output again
argues for a ranked-hypothesis API.

#### Cell-based decomposition: fast variants

**Citation.** Woo, Y., & Sakurai, H. (2002). Recognition of maximal features by
volume decomposition. *Computer-Aided Design*, 34(3), 195-207. See also fast
cell-based decomposition, *Computer-Aided Design*, 35 (2003).

**Method.** Refines maximal-volume recognition with faster cell generation and
merging, reducing the combinatorial cost of the original Sakurai approach and
improving handling of intersecting maximal features.

**Kernel relevance.** Performance template: volumetric FR is only viable in
Keel if the cell Booleans are fast and robust, reinforcing the priority on
Boolean engine quality.

### 1.4 Blend and fillet chain recognition

(The dedicated blend file covers Venkataraman and Sohoni. Here we extend with
the wider literature, since blend removal is the first step of almost every FR
pipeline.)

#### Zhu and Menq (2002): fillet/round suppression for FR

**Citation.** Zhu, H., & Menq, C. H. (2002). B-rep model simplification by
automatic fillet/round suppressing for efficient automatic feature recognition.
*Computer-Aided Design*, 34(2), 109-123.

**Method.** Detect fillet/round faces by surface type (constant-radius
cylindrical/toroidal faces tangent to two neighbours) and by their spring-edge
signature (tangent edges where the blend meets the adjacent faces). Group
tangent-connected blend faces into chains, then suppress them: remove the blend
faces and extend the adjacent faces to re-intersect, recovering the sharp edge.
This simplification runs before volumetric FR so blends do not corrupt the
feature graph.

**Limitations.** Variable-radius and complex setback (vertex) blends are hard;
extending faces to a clean intersection can fail when neighbours are non-planar.

**Kernel relevance.** Direct: Keel's blend-suppression service needs reliable
surface classification (is this face a constant-radius blend), spring-edge
detection, and the face-extend-and-reintersect local operation. This couples FR
to the local-operations and entity-simplification layers.

#### Cui, Gao, and others; Li et al.: blend taxonomy and chains

**Citation.** Cui, X., Gao, S., & Zhou, G. (2004). An efficient algorithm for
recognizing and suppressing blend features. *Computer-Aided Design & Applications*,
1(1-4), 421-428. Plus the blend-chain simplification line, e.g.
*Programming and Computer Software* (2020), Simplification of CAD models by
automatic recognition and suppression of blend chains.

**Method.** Classify blends into face-face, face-edge, and vertex (setback)
types and handle mixed blends. Recognize spring-edge groups, order them into
chains and networks, and suppress in dependency order so that a vertex blend at
the meeting of three edge blends is removed only after its constituents. This
chain/network ordering is the practical heart of robust blend removal.

**Limitations.** Setback vertex blends and mutually dependent blend networks
remain the failure cases; suppression can leave invalid geometry needing
healing.

**Kernel relevance.** Keel needs a blend dependency graph and an ordered
suppression engine, plus geometry healing after removal. This is the most
demanded FR service for the edit and CAE workflows.

#### FilletRec (2025): learned fillet recognition

**Citation.** FilletRec: A lightweight graph neural network with intrinsic
features for automated fillet recognition (2025). arXiv:2511.05561.

**Method.** A small GNN that classifies fillet faces using intrinsic geometric
features (curvature, tangency, radius consistency) on the face graph, targeting
the specific subproblem of fillet detection where classical surface-type tests
are brittle on imported or noisy geometry.

**Kernel relevance.** Suggests a hybrid: classical spring-edge logic for clean
native geometry, an ML classifier as a fallback for ambiguous imported blends,
with exact geometric verification gating either.

### 1.5 Hole, pocket, slot, step, and pattern recognition

#### Hole and compound-hole recognition

**Method.** Holes are recognized from cylindrical (and conical for
countersinks, toroidal for chamfers) faces with the right concavity. Compound
holes (counterbored, countersunk, stepped) are coaxial chains of cylinder/cone
faces sharing an axis, recognized by axis collinearity plus radius ordering.
Commercial systems (Solid Edge, NX) recognize hole types and recover procedural
hole parameters (diameter, depth, thread, countersink angle) so the hole becomes
an editable parametric feature regardless of how it was originally modelled
(extruded cut, revolved cut, or hole command).

**Kernel relevance.** Hole recognition is the highest-value, most-used FR
service. It needs exact cylinder/cone/torus classification, axis extraction, and
coaxiality testing, all of which require canonical surface recovery (see 1.7).
Keel should ship hole recognition as a first-class, kernel-adjacent service.

#### Pocket, slot, step recognition

**Method.** These are the canonical AAG depression features: a slot is two
opposing walls plus a floor open at both ends; a pocket is a closed loop of
walls plus a floor; a step is one wall plus a floor open on one side. Recognized
by concave-edge loop topology plus open/closed boundary analysis. Hint-based
methods recognize them under intersection via opposing-face hints.

**Kernel relevance.** Maps directly onto the AAG service and the
opposing-face-query hint primitive.

#### Pattern recognition: linear and circular arrays

**Method.** After individual features are found, detect repetition: cluster
features of identical type and parameters, then fit a transform (translation
for linear arrays, rotation about an axis for circular/bolt-circle patterns) by
testing whether feature positions lie on a regular lattice or circle. Recovering
the pattern collapses N holes into one seed plus a pattern rule, the strongest
form of design-intent recovery. Solid Edge, for example, recognizes parallel
hole series and bolt circles and redefines them as a single pattern feature.

**Limitations.** Tolerance on "regular" spacing; partial/broken patterns;
features modelled by different methods must first be normalized to a comparable
parametric form.

**Kernel relevance.** Pattern detection is a layer above core FR but depends on
the kernel delivering normalized feature parameters and exact axis/position
data. High value for the design-intent reconstruction workflow.

#### Thin-wall and rib recognition

**Method.** Ribs and thin walls are protrusion features recognized by detecting
pairs of nearly-parallel offset faces within a small wall-thickness threshold
(a local distance/medial query) rather than by concave-edge topology alone.
This ties to medial-axis and thickness analysis (see file 10).

**Kernel relevance.** Needs fast face-pair proximity / local-thickness queries,
linking FR to the medial/thickness services. Important for CAE (shell meshing)
and for moldability checks.

---

## Part 2: Machine learning feature recognition

The 2018 to 2025 shift moved FR from hand-coded rules to learned models. The
representations split into voxel, point cloud, multi-view, and (most relevant
to a kernel) native B-rep graph methods.

### 2.1 Voxel and image representations

#### Zhang et al. (2018): FeatureNet

**Citation.** Zhang, Z., Jaiswal, P., & Rai, R. (2018). FeatureNet: Machining
feature recognition based on 3D convolution neural network. *Computer-Aided
Design*, 101, 12-22.

**Method.** Voxelize the CAD model into a 3D occupancy grid and feed it to a
deep 3D CNN (about 34M parameters) that learns to classify the machining
feature. Trained on an automatically generated dataset of 144,000 models, each
containing a single feature, across 24 feature classes, split 70/15/15.

**Results.** 96.7% accuracy on single-feature recognition. The pioneering deep
FR paper and the proof that learned FR is viable.

**Limitations.** Single-feature only: it classifies one feature per model and
cannot segment or distinguish multiple or intersecting features. Voxelization
loses exact geometry and is resolution-limited (curved faces, small features
blur). No face-level output, so it cannot tell CAM which faces to machine.

**Kernel relevance.** Cautionary: the voxel route discards exactly the exact
geometry a kernel exists to provide. Useful only as a coarse classifier; the
kernel-native graph methods below are the right direction.

#### Multi-sectional-view and point-cloud recognizers (MsvNet, MFPointNet)

**Citation.** Shi, P., et al. (2020). A novel learning-based feature
recognition method using multiple sectional view representation. *Journal of
Intelligent Manufacturing*. MFPointNet (2022). *Machines*, 10(12), 1165.

**Method.** MsvNet renders multiple 2D sectional views of the solid and applies
2D CNNs plus view pooling, transferring 2D vision pretraining and improving
multi-feature handling over FeatureNet. MFPointNet samples the surface to a
point cloud and applies a PointNet variant with selective downsampling to
classify 24 machining feature types.

**Results.** MsvNet improved multi-feature performance; point-cloud methods
report around 95% on synthetic data.

**Limitations.** All conversion routes (views, points) lose resolution and
exact surface identity; small and intersecting features degrade. Point clouds
discard topology entirely.

**Kernel relevance.** Confirms that any representation that throws away the
B-rep underperforms on the cases (small features, intersections, exact
parameters) that matter most for a kernel. Reinforces the native-B-rep choice.

### 2.2 Native B-rep learning

#### Cao et al. (2020): the MFCAD dataset and graph node classification

**Citation.** Cao, W., Robinson, T., Hua, Y., Boussuge, F., Colligan, A. R., &
Pan, W. (2020). Graph representation of 3D CAD models for machining feature
recognition with deep learning. *ASME IDETC/CIE*. (Dataset: MFCAD.)

**Method.** Frame FR as node classification on the face-adjacency graph: each
B-rep face is a graph node with geometric features, classified into one of 16
machining feature classes by a GNN. Introduced MFCAD, the first synthetic B-rep
segmentation dataset: 15,488 models with planar faces, each face labelled with
its feature class.

**Results.** Established face-level FR (segmentation, not just whole-model
classification) on native B-rep graphs and the first reusable benchmark.

**Limitations.** Planar faces only; synthetic; features mostly isolated.

**Kernel relevance.** This is the template Keel's FR-for-ML hook should serve:
emit the face-adjacency graph with per-face and per-edge attributes in a stable
form a GNN can consume.

#### Jayaraman et al. (2021): UV-Net

**Citation.** Jayaraman, P. K., Sanghi, A., Lambourne, J. G., Willis, K. D. D.,
Davies, T., Shayani, H., & Morris, N. (2021). UV-Net: Learning from boundary
representations. *CVPR 2021*.

**Method.** A unified B-rep representation: sample each face over its UV
parameter domain into a regular 2D grid of 3D points, normals, and tangents (a
"UV-grid"), and each edge over its 1D parameter into a UV-grid. Build the
face-adjacency graph with face UV-grids as node features and edge UV-grids as
edge features. A 2D CNN encodes each face grid into a node embedding (image
convolution captures surface shape), then a graph CNN propagates over topology.
This couples image and graph convolution compute-and-memory efficiently.

**Results.** Outperformed point cloud, voxel, and mesh representations across
five supervised and unsupervised tasks; released the SolidLetters dataset.
Generalizes across classification, segmentation, and retrieval.

**Limitations.** UV-grid sampling still discretizes geometry (though it
preserves the parametric domain better than voxels); trimmed-face boundaries are
only implicitly handled by sampling inside trims.

**Kernel relevance.** UV-grid sampling is a clean kernel export: it is exactly
surface and curve evaluation over the parameter domain, which Keel's evaluator
already does. Keel should expose batched UV-grid sampling per face/edge as the
ML feature-extraction hook, since it is representation-agnostic and lossless up
to sampling density.

#### Lambourne et al. (2021): BRepNet

**Citation.** Lambourne, J. G., Willis, K. D. D., Jayaraman, P. K., Sanghi, A.,
Meltzer, P., & Shayani, H. (2021). BRepNet: A topological message passing
system for solid models. *CVPR 2021*.

**Method.** Convolve directly on the B-rep's oriented topology. The atomic unit
is the coedge (a use of an edge by one face, with orientation). Around each
coedge a fixed small neighbourhood is defined by topological walks: next and
previous coedge around the face, the mating coedge on the adjacent face, the
parent face, and the parent edge. Learnable kernels combine feature vectors
gathered along these walks, an exact analogue of image convolution but defined
on B-rep topology rather than a pixel grid. Released the Fusion 360 Gallery
segmentation dataset.

**Results.** Higher segmentation accuracy than mesh and point-cloud methods,
operating with no geometric approximation of the topology.

**Limitations.** The coedge kernel walks are fixed-template; performance depends
on input face/edge feature engineering; trained largely on sketch-extrude
geometry.

**Kernel relevance.** BRepNet's coedge neighbourhood is precisely the
half-edge / coedge adjacency Keel's topology layer must expose. The lesson:
a kernel with clean, fast coedge traversal can feed a topological GNN with zero
conversion. This is the tightest possible kernel-ML coupling.

#### Colligan et al. (2022): Hierarchical CADNet and MFCAD++

**Citation.** Colligan, A. R., Robinson, T. T., Nolan, D. C., Hua, Y., & Cao,
W. (2022). Hierarchical CADNet: Learning from B-reps for machining feature
recognition. *Computer-Aided Design*, 147, 103226.

**Method.** A two-level hierarchical GNN. The lower level is a mesh discretizing
each face's surface (capturing fine geometry); the upper level is the B-rep
face-adjacency graph (capturing topology). Hierarchical edges link mesh nodes to
their parent B-rep face, so the network reasons jointly over fine geometry and
coarse topology. Released MFCAD++: 59,655 models, 3 to 10 features each, planar
and non-planar faces, including intersecting features.

**Results.** Outperformed prior architectures on feature identification
including intersecting machining features, the first native-B-rep ML method to
explicitly target intersections, with a dataset built to contain them.

**Limitations.** The mesh level reintroduces a discretization; still synthetic
training data.

**Kernel relevance.** The hierarchy (exact topology over a tessellation)
mirrors Keel's own faceting service feeding the topology graph. MFCAD++'s
intersecting-feature focus makes it the benchmark to target if Keel ships a
learned recognizer.

#### Wu et al. (2024): AAGNet, multi-task on the geometric AAG

**Citation.** Wu, H., Lei, R., Peng, Y., & Gao, L. (2024). AAGNet: A graph
neural network towards multi-task machining feature recognition. *Robotics and
Computer-Integrated Manufacturing*, 86, 102661.

**Method.** Define the geometric AAG (gAAG): the classical AAG enriched with
geometric and extended attributes per face and edge from the neutral B-rep. A
GNN with an input encoder, graph encoder, and multi-task head performs three
tasks at once: semantic segmentation (feature type per face), instance
segmentation (group faces into individual feature instances), and bottom-face
identification (find each feature's supporting floor face). Released MFInstSeg
(over 60,000 STEP files with instance labels).

**Results.** Strong multi-task results; instance segmentation and bottom-face
output are directly actionable for CAM (the bottom face anchors tool approach).

**Limitations.** Synthetic data; intersection-heavy real parts still stress it.

**Kernel relevance.** AAGNet closes the loop: the modern ML method consumes
exactly the enriched AAG that classical methods used and that Keel must produce
anyway. The instance and bottom-face outputs are the shape Keel's FR API should
return for CAM. The gAAG attribute list is a concrete spec for Keel's
graph-export hook.

#### Lee et al. (2023): BRepGAT

**Citation.** Lee, J., et al. (2023). BRepGAT: Graph neural network to segment
machining feature faces in a B-rep model. *Journal of Computational Design and
Engineering*, 10(6), 2384-2400.

**Method.** A graph attention network over a homogeneous graph built from
B-rep face and edge descriptors designed specifically for FR. Attention weights
let the model focus on the most informative neighbours when segmenting feature
faces.

**Results.** 99.1% accuracy on MFCAD18++, state of the art at publication, with
reasonable cross-dataset robustness.

**Limitations.** High accuracy is on synthetic benchmarks; real-world and
intersecting performance is the open question (the recurring caveat).

**Kernel relevance.** Confirms attention-based graph models as the current
front of native-B-rep FR; the descriptor set is another concrete attribute
spec for Keel's export.

#### Dai et al. (2025): BRepFormer, transformer on B-rep

**Citation.** Dai, et al. (2025). BRepFormer: Transformer-based B-rep geometric
feature recognition. *ACM International Conference on Multimedia Retrieval
(ICMR) 2025*. arXiv:2504.07378.

**Method.** A transformer with four stages (feature extraction, encoding,
transformer blocks, recognition head). Topology is encoded as attention biases:
face shortest-path distance (Dijkstra on the adjacency graph), face angular
distance (dihedral angles), centroid distance, and shortest edge path, forming
bias matrices that constrain attention to respect geometry. Geometry per face is
UV-sampled (points, normals, tangents) plus attributes (surface type, area,
centroid, edge type/length/convexity), encoded by CNNs/MLPs to 256-d vectors. A
global virtual face token enables whole-model interactions, aimed at
intersecting features.

**Results.** 99.62% accuracy and 98.74% mIoU on MFInstSeg; 93.16% on MFTRCAD;
beats AAGNet on the proposed CBF dataset (94.66% vs 93.41%). Competitive with or
ahead of UV-Net, BRepNet, Hierarchical CADNet.

**Limitations.** No explicit neuro-symbolic verification; gains are on synthetic
benchmarks; transformer cost scales with face count.

**Kernel relevance.** The attention-bias inputs (shortest path, dihedral angle,
convexity) are all kernel graph queries. Reinforces that the kernel's job is to
serve a rich, exact attributed graph; the network is downstream.

#### Zhang et al. (2024): BrepMFR, domain adaptation for the synthetic-to-real gap

**Citation.** Zhang, S., et al. (2024). BrepMFR: Enhancing machining feature
recognition in B-rep models through deep learning and domain adaptation.
*Computer Aided Geometric Design*, 111, 102318.

**Method.** A transformer-based B-rep recognizer trained on abundant synthetic
data, with a domain-adaptation stage that aligns the feature distributions of
synthetic and real CAD so the model transfers to real parts it never saw in
training. This directly attacks the field's deepest limitation: every benchmark
(MFCAD, MFCAD++, Fusion 360 Gallery) is synthetic and biased toward clean,
often isolated features, so models that score 99% on benchmarks degrade on real
imported geometry with messy intersections and translation artifacts.

**Results.** Improved transfer to real-world B-rep models versus
synthetic-only training; explicit treatment of intersecting features as the
hard case.

**Limitations.** Domain adaptation narrows but does not close the gap; labelled
real data remains scarce.

**Kernel relevance.** The synthetic-bias problem is why Keel should not bet FR
correctness on ML alone. The robust design is neuro-symbolic: ML proposes
candidate feature faces and types (cheap, handles fuzzy cases), and the kernel
verifies each proposal exactly (Boolean reconstruction, accessibility,
parameter fit) before accepting it. The kernel's exactness is the antidote to
ML's distribution shift.

### 2.3 The datasets and their limitations

**MFCAD** (Cao 2020): 15,488 planar-face models, 16 features, one label per
face. **MFCAD++** (Colligan 2022): 59,655 models, 3 to 10 features, planar and
non-planar, with intersections. **Fusion 360 Gallery Segmentation** (Lambourne
2021): about 35,858 real human-designed B-rep bodies (~390,000 faces) labelled
by the modelling operation responsible for each face, plus 8,625 CAD construction
sequences. **MFInstSeg / MFTRCAD / CBF** (2024-2025): 60k+ models with instance
and intersecting-feature labels.

**Shared limitations.** (1) Synthetic bias: most are procedurally generated, so
they under-represent real-world messiness (translation noise, sliver faces,
non-canonical surfaces). Only Fusion 360 Gallery is human-authored, and it is
labelled by modelling operation, not by machining feature. (2) Single-feature
or isolated-feature bias in the early sets: real parts have dense intersections.
(3) Class imbalance and a fixed, manufacturing-centric feature vocabulary.
(4) Labelling-by-operation conflates design history with semantic features.

**Kernel relevance.** Keel can contribute the missing piece: a faithful,
exact-geometry corpus generator. Because Keel produces real B-reps with exact
surfaces, it can synthesize training data with controlled intersections and
canonical-vs-spline surface variation, and (via persistent naming) auto-label
faces, closing some of the synthetic-bias gap.

### 2.4 Neuro-symbolic and uncertainty handling

The convergent direction across the recent literature is neuro-symbolic: ML for
proposal under ambiguity, exact symbolic geometry for verification. This echoes
the classical hint-based generate-and-verify loop (OOFF, IF2) with an ML hint
generator replacing hand-coded hint rules. Uncertainty is handled by returning
per-face confidence and multiple ranked interpretations (the Marefat-Kashyap and
Sakurai multiple-interpretation idea, now with learned scores). The persistent
ML weakness remains intersecting features: every survey and the
domain-adaptation work flag that benchmark accuracy collapses on dense
real-world intersections, and that exact verification is the only reliable
guard.

---

## Part 3: Applications in detail

### 3.1 Defeaturing for CAE

Before meshing, suppress features smaller than the mesh size: small holes,
fillets, chamfers, logos, and beads that force the mesher into dense local
refinement, inflating element count and solve time without improving accuracy.
FR identifies candidate suppressible features; a size/criticality filter
(volume or characteristic length vs mesh target, plus stress-region heuristics)
decides which to remove; local operations remove them with real CAD edits
(blend suppression, hole filling) rather than mesh-only virtual topology so the
simplified body stays a valid B-rep. There is no standard objective criterion
for "suppressible," so the decision is partly heuristic and a candidate for ML
ranking. Keel relevance: FR feeds the suppression list; the local-operations
and healing layers execute it; the tessellation layer consumes the result.

### 3.2 CAM feature extraction in practice

Commercial CAM (Siemens NX CAM, Autodesk FeatureCAM) ships production FR:
recognize holes (with type, thread, depth), pockets, slots, faces, and chamfers,
map each to a machining operation with tool and strategy, and detect hole
patterns to generate efficient drilling cycles. NX CAM applies feature-based
machining where recognized features drive operation templates; FeatureCAM
auto-recognizes prismatic and turned features and assigns feature-based
operations. The pipeline is: recognize feature -> recover canonical parameters
-> select tool/operation -> order operations by machining rules (echoing
Sakurai's volume sequencing). Keel relevance: FR must output instance-level
features with bottom faces (AAGNet-style) and exact canonical parameters; this
is a kernel-adjacent service, with the operation mapping in a CAM layer above.

### 3.3 Design retrieval by feature signature

Encode a part by its feature set (a feature histogram or graph signature) and
retrieve similar parts from a library, for design reuse and cost estimation.
UV-Net and the B-rep GNNs were evaluated on exactly this retrieval task using
learned embeddings. Keel relevance: a content-based retrieval layer can consume
either the symbolic feature list or a learned B-rep embedding produced from
Keel's graph export.

---

## Recognition services in Keel

**In-kernel (the substrate every method needs):**

- **Attributed adjacency graph as a first-class queryable structure.** Faces as
  nodes; edges as arcs with exact convex/concave/smooth/tangent classification;
  per-face surface-type tags. This single structure serves classical graph
  matching, hint reasoning, volumetric setup, and every B-rep GNN. It must be
  fast: FR cost is dominated by adjacency traversal on parts with hundreds to
  thousands of faces, so the topology layer's half-edge/coedge traversal and
  the convexity test are hot paths.
- **Exact surface-type classification and canonical recovery.** "Is this spline
  actually a cylinder/cone/torus/plane," with axis, radius, and apex recovery.
  This is shared with entity simplification (file on simplification) and is the
  precondition for hole recognition, blend detection, and pattern fitting. An
  imported spline that is really a cylinder must be recognized as such or all
  downstream FR fails. This is the single highest-leverage kernel service for FR.
- **Hint and verification primitives.** Cheap queries: opposing-face pairs,
  coaxial cylinder chains, concave-edge loops, spring-edge groups,
  near-parallel offset face pairs (ribs/walls). And the exact verifier:
  Boolean reconstruction of a hypothesized feature volume against the part or
  stock, plus accessibility tests. The verifier is what makes neuro-symbolic FR
  trustworthy.
- **UV-grid and coedge-neighbourhood export for ML.** Batched UV-domain
  sampling of faces and edges (UV-Net style) and coedge adjacency walks
  (BRepNet style), so any learned recognizer trains and runs with zero lossy
  conversion. This is the kernel's ML hook and a differentiator over
  mesh/voxel-bound pipelines.
- **Blend recognition and ordered suppression.** Spring-edge detection, blend
  dependency graph, ordered chain/network suppression, face-extend-reintersect,
  and post-suppression healing. Used by both the edit and CAE workflows.

**As a layer above the kernel:**

- **Feature library and matching/hint orchestration.** The catalogue of feature
  types, their full subgraphs and minimal-condition subgraphs (Gao-Shah), the
  generate-and-verify loop (OOFF/IF2), and grammar-based user-extensible
  definitions all live in an FR service that calls kernel primitives.
- **Learned recognizers.** The GNNs/transformers themselves (AAGNet, BRepGAT,
  Hierarchical CADNet, BRepFormer) are a plug-in proposal stage, not kernel
  code, consuming the kernel's graph/UV-grid export and feeding the kernel's
  verifier.
- **Application policies.** Defeaturing size/criticality rules, CAM
  operation mapping and sequencing, pattern detection, and retrieval signatures
  are domain policy layers.

**Design stances for Keel.**

1. **Hypotheses, not verdicts.** FR returns ranked interpretations with
   confidence (intersecting cases are genuinely ambiguous: Marefat-Kashyap,
   Sakurai, and modern ML all agree). The API surfaces multiplicity instead of
   forcing a wrong single answer.
2. **Neuro-symbolic by construction.** ML (or classical hints) propose; the
   exact kernel verifies. The kernel's exactness is the structural answer to
   ML's synthetic-bias and intersecting-feature weakness.
3. **Incremental recompute.** Tie recognized features to persistent face names
   (file 07) so an edit invalidates and recomputes only affected features
   (Kim's localized update, Han's IF2 incrementality), enabling interactive FR.
4. **Be the data source.** Keel's exact-geometry corpus generation plus
   auto-labelling via persistent naming can help close the synthetic-bias gap
   that limits every public dataset.

---

## References

1. Joshi, S., & Chang, T.-C. (1988). Graph-based heuristics for recognition of
   machined features from a 3D solid model. *Computer-Aided Design*, 20(2),
   58-66.
2. Kyprianou, L. K. (1980). *Shape classification in computer-aided design*
   (Doctoral dissertation). University of Cambridge.
3. Marefat, M., & Kashyap, R. L. (1990). Geometric reasoning for recognition of
   three-dimensional object features. *IEEE TPAMI*, 12(10), 949-965.
4. Vandenbrande, J. H., & Requicha, A. A. G. (1993). Spatial reasoning for the
   automatic recognition of machinable features in solid models. *IEEE TPAMI*,
   15(12), 1269-1285.
5. Han, J., & Requicha, A. A. G. (1998). Integration of feature based design and
   feature recognition. *Computer-Aided Design*, 30(5), 393-405.
6. Han, J., Regli, W. C., et al. (1998). Hint-based reasoning for feature
   recognition: status report. *Computer-Aided Design*, 30(13), 1003-1007.
7. Gao, S., & Shah, J. J. (1998). Automatic recognition of interacting machining
   features based on minimal condition subgraphs. *Computer-Aided Design*,
   30(9), 727-739.
8. Rahmani, K., & Arezoo, B. (2006). Boundary analysis and geometric completion
   for recognition of interacting machining features. *Computer-Aided Design*,
   38(8), 845-856.
9. Woo, T. C. (1982). Feature extraction by volume decomposition. *Proc.
   CAD/CAM Technology in Mechanical Engineering*, MIT.
10. Kim, Y. S. (1992). Recognition of form features using convex decomposition.
    *Computer-Aided Design*, 24(9), 461-476.
11. Kim, Y. S., & Wilde, D. J. (1992). A convergent convex decomposition of
    polyhedral objects. *ASME J. Mechanical Design*, 114(3), 468-476.
12. Sakurai, H. (1995). Volume decomposition and feature recognition, part I:
    polyhedral objects. *Computer-Aided Design*, 27(11), 833-843.
13. Sakurai, H., & Dave, P. (1996). Volume decomposition and feature
    recognition, part II: curved objects. *Computer-Aided Design*, 28(6/7),
    519-537.
14. Woo, Y., & Sakurai, H. (2002). Recognition of maximal features by volume
    decomposition. *Computer-Aided Design*, 34(3), 195-207.
15. Han, J., Pratt, M., & Regli, W. C. (2000). Manufacturing feature recognition
    from solid models: a status report. *IEEE Transactions on Robotics and
    Automation*, 16(6), 782-796.
16. Shah, J. J., Anderson, D., Kim, Y. S., & Joshi, S. (2001). A discourse on
    geometric feature recognition from CAD models. *ASME J. Computing and
    Information Science in Engineering*, 1(1), 41-51.
17. Babic, B., Nesic, N., & Miljkovic, Z. (2008). A review of automated feature
    recognition with rule-based pattern recognition. *Computers in Industry*,
    59(4), 321-337.
18. Zhu, H., & Menq, C. H. (2002). B-rep model simplification by automatic
    fillet/round suppressing for efficient automatic feature recognition.
    *Computer-Aided Design*, 34(2), 109-123.
19. Cui, X., Gao, S., & Zhou, G. (2004). An efficient algorithm for recognizing
    and suppressing blend features. *Computer-Aided Design & Applications*,
    1(1-4), 421-428.
20. Zhang, Z., Jaiswal, P., & Rai, R. (2018). FeatureNet: Machining feature
    recognition based on 3D convolution neural network. *Computer-Aided
    Design*, 101, 12-22.
21. Shi, P., et al. (2020). A learning-based feature recognition method using
    multiple sectional view (MsvNet) representation. *J. Intelligent
    Manufacturing*.
22. Cao, W., Robinson, T., Hua, Y., Boussuge, F., Colligan, A. R., & Pan, W.
    (2020). Graph representation of 3D CAD models for machining feature
    recognition with deep learning (MFCAD dataset). *ASME IDETC/CIE*.
23. Jayaraman, P. K., et al. (2021). UV-Net: Learning from boundary
    representations. *CVPR 2021*.
24. Lambourne, J. G., Willis, K. D. D., Jayaraman, P. K., Sanghi, A., Meltzer,
    P., & Shayani, H. (2021). BRepNet: A topological message passing system for
    solid models. *CVPR 2021*.
25. Willis, K. D. D., et al. (2021). Fusion 360 Gallery: A dataset and
    environment for programmatic CAD construction from human design sequences.
    *ACM TOG / arXiv:2010.02392*.
26. Colligan, A. R., Robinson, T. T., Nolan, D. C., Hua, Y., & Cao, W. (2022).
    Hierarchical CADNet: Learning from B-reps for machining feature recognition
    (MFCAD++ dataset). *Computer-Aided Design*, 147, 103226.
27. Lee, J., et al. (2023). BRepGAT: Graph neural network to segment machining
    feature faces in a B-rep model. *J. Computational Design and Engineering*,
    10(6), 2384-2400.
28. Wu, H., Lei, R., Peng, Y., & Gao, L. (2024). AAGNet: A graph neural network
    towards multi-task machining feature recognition. *Robotics and
    Computer-Integrated Manufacturing*, 86, 102661.
29. Zhang, S., et al. (2024). BrepMFR: Enhancing machining feature recognition
    in B-rep models through deep learning and domain adaptation. *Computer Aided
    Geometric Design*, 111, 102318.
30. Dai, et al. (2025). BRepFormer: Transformer-based B-rep geometric feature
    recognition. *ACM ICMR 2025 / arXiv:2504.07378*.
31. FilletRec: A lightweight graph neural network with intrinsic features for
    automated fillet recognition (2025). *arXiv:2511.05561*.
