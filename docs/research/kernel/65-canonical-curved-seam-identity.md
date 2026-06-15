# 65. Canonical curved-seam identity: the tolerant EdgeId and the shared cross-operand subdivision

## Title and scope

This is the implementation-grade literature dossier for **canonical curved-seam identity**: the deterministic rule and tolerant hash that make ONE shared seam curve subdivide at the SAME canonical nodes for BOTH boolean operands, so the two operands' seam fragments pair by IDENTITY (a shared `EdgeId`) and never by a coordinate compare at stitch time. Dossier 59 named this construct "the single most important new piece" of curved boolean assembly. Dossier 59 stated WHAT it must do (compute each seam once, canonicalize, share the `EdgeId`); this dossier specifies HOW: the canonical subdivision rule (the curved analog of the planar `subdivide_seam_ring`), the tolerant orientation key and node-snapping that make the `EdgeId` hash stable, the per-incident-face sense derivation, and the contract with the periodic-domain split of dossier 64.

It builds on, and does not repeat, dossier 47 (the `EdgeId` radial-splice stitcher, the shell-closure / drop-prevention invariant, identity-from-imprint), dossier 59 (the curved assembly conclusion: canonical identity plus seam-aware split feed the unchanged stitcher), dossier 52 (persistent naming, the canonical-key machinery Keel already reasons about), and dossier 46 (the `n_out = sense x natural` orientation convention). It is a **design recommendation** grounded in production-kernel practice (OCCT BOPDS / pave blocks / section edges, ACIS section-edge sharing) and the computational-geometry literature on canonical labeling, geometric hashing, and snap rounding, to be validated against Keel's curved-boolean regression suite and the explorer decline corpus.

**House style, machine-checked: no em-dashes anywhere; en-dashes are never used as separators; ranges are written with the word "to".**

---

## The confirmed failure mode, stated once (the thing this dossier removes)

> **Add.273, verbatim mechanism.** Keel ALREADY computes each seam once: `seam_curves` produces ONE shared `SeamCurve` carrying ONE `Curve3`. The divorce happens DOWNSTREAM. `imprint_operand(a)` and `imprint_operand(b)` each take that one shared curve and SUBDIVIDE it at THEIR OWN operand's seam-line crossing, which is a DIFFERENT point on the SAME curve. Operand A inserts a node at parameter t_A; operand B inserts a node at t_B != t_A. The two operands' seam sub-edges therefore have different endpoints, share no vertex, and the radial cycle around the seam cannot close. The stitcher reports an unmatched coedge: the Hoffmann divorce, surfacing as `UnassemblableSeam`. The PLANAR Steinmetz case escapes only by luck: the two ellipses cross each other at two points that BOTH operands necessarily split at, so a shared node exists by accident of the configuration. The general curved case (unequal radii, non-crossing seams) has no such accidental shared point. The planar analog `subdivide_seam_ring` exists and supplies the shared nodes for the planar path; the curved analog does NOT, which is the precise gap this dossier fills.

The cure is one sentence: **subdivide the one shared curve ONCE, at the canonical UNION of every node either operand could need, before either imprint runs, and tag each resulting sub-edge with a stable child `EdgeId` that both imprints reference.** There is then no second subdivision, no second point, and nothing to reconcile. The remainder of this dossier specifies that one subdivision and the hash that keeps its child ids stable under tolerant-f64 perturbation.

---

## Q1. The canonical subdivision: one curve, the union of all nodes, stable child ids

### The problem, precisely

A seam is a single `Curve3` C with a parameter domain t in [t0, t1] (closed-loop seams wrap, t1 identified with t0). Imprinting it onto each operand requires inserting nodes where C meets things that must become vertices. The set of required nodes differs per operand and per face:

- **Operand A's needs.** Where C crosses A's existing edges; where C crosses A's periodic seam-slit (u = 0 / u = 2*pi on a cone or cylinder lateral); where C reaches A's apex or pole; the endpoints of C if C is an open arc on A.
- **Operand B's needs.** The same list against B's topology. These nodes land at DIFFERENT parameters on C than A's, because A and B are different bodies.
- **The shared self-crossings.** Where C crosses ITSELF or another seam, both operands must split (the planar Steinmetz luck).

If each `imprint_operand` inserts only ITS operand's nodes, the two node sets differ and the sub-edges do not align. The fix is to take the UNION.

### The canonical subdivision rule (the deliverable, the curved `subdivide_seam_ring`)

> **Before either imprint runs, compute the COMPLETE node set N(C) = { all parameters t on C at which EITHER operand, OR the seam itself, requires a vertex }. Sort N(C) by the curve's canonical parameter (Q2). Snap clustered nodes ONCE under the tolerant rule (Q2) so coincident requirements collapse to one node. Subdivide C at the snapped, sorted N(C) into an ordered list of sub-edges e_0, e_1, ..., e_{k-1}. Assign each sub-edge a STABLE child `EdgeId` keyed by (the canonical seam id, the ordered index i, the two bounding node ids). BOTH `imprint_operand(a)` and `imprint_operand(b)` then consume this SAME pre-subdivided edge list: each operand attaches its fragment coedges to the child `EdgeId`s, never re-subdividing C. Cross-operand pairing at stitch is by child `EdgeId`, identical to the planar path.**

The node set N(C) is the union of four contributions, each computed from the ONE shared C, not from a re-intersection:

1. **Periodic seam-slit crossings of each incident periodic face.** For a cone or cylinder lateral, lift C to the (u, v) domain via its closed-form preimage (dossier 59 Q1 step 1) and find every t where the preimage crosses u = 0 / u = 2*pi. These are the curved analog of dossier 47's pave-block boundaries. They are face-specific, so the union runs over BOTH operands' incident periodic faces.
2. **Apex / pole nodes.** Any t where C's preimage reaches v = 1 (cone apex) or a sphere pole. A node here, off-degenerate, is legal (dossier 59 Q1 step 3); on-degenerate is DECLINE (dossier 59 Q5 class 2).
3. **Other-seam and self crossings.** Where C meets another seam curve, or crosses itself, on either face. Both operands split here.
4. **Pre-existing-edge crossings.** Where C crosses an original edge of either operand (the curved analog of imprinting onto an existing arrangement). The crossed original edge is ALSO subdivided at the shared node, inheriting its own parent identity (dossier 47: original-face boundary edges sub-divided into pave blocks with stable child ids).

The key invariant: **N(C) is computed against the SINGLE shared C, so a node that both operands need is computed ONCE and is one parameter value, not two that must later be matched.** The Steinmetz luck is generalized: every shared point is now shared by construction, not by configuration.

### How OCCT realises "one section edge shared to both faces with a consistent pcurve each"
- **Citation.** Open CASCADE Technology, "Boolean Operations" user guide (BOPAlgo_PaveFiller, BOPDS_DS, BOPDS_PaveBlock, MakeBlocks, BuildPCurves); BOPDS_DS and BOPAlgo_PaveFiller class references. https://dev.opencascade.org/doc/occt-7.6.0/refman/html/class_b_o_p_d_s___d_s.html ; https://dev.opencascade.org/doc/occt-7.3.0/refman/html/class_b_o_p_algo___pave_filler.html ; https://documentation.help/Open-Cascade/occt_user_guides__boolean_operations.html
- **Method / content (verified from the BOPDS model).** OCCT's intersection phase (`BOPAlgo_PaveFiller`) populates ONE shared data structure (`BOPDS_DS`). Intersection curves between a face pair become SECTION edges. Each edge (original or section) is represented by **pave blocks** (`BOPDS_PaveBlock`): a pave is a (parameter, vertex) pair on the edge, and a pave block is the segment of the edge between two consecutive paves. Splitting an edge at shared vertices means inserting paves; the pave block IS the canonical sub-edge, and it is held in the shared DS, so BOTH faces that the section edge bounds reference the SAME pave block. The doc states faces "are considered interfering in case they share the IN or SECTION edges," that is, identity is by shared section edge, not coordinate. `MakeBlocks` post-processes section edges, adding existing edges that interfere so that "all possible intersections" are resolved into the same pave structure. A dedicated **BuildPCurves** step then builds the 2D p-curve on EACH incident face for each section edge, and `AttachExistingPCurve` "takes the p-curve on the face from one edge and attaches this p-curve to another edge coinciding with the first one," that is, it shares the SAME pcurve to a coincident edge rather than recomputing it. Shape-healing `SameParameter` then enforces that the 3D curve and each pcurve agree to tolerance on periodic surfaces.
- **Limitations.** OCCT computes the pave structure with a tolerance-driven vertex/pave merge that can mis-cluster near-coincident intersections; it relies on post-hoc shape healing where the pave merge drifts. The wiki under-documents seam-straddling faces. `BOPAlgo_AlertBuildingPCurveFailed` fires when the pcurve build fails, which is OCCT's analog of Keel's `UnassemblableSeam` surfaced as an alert rather than a silent drop.
- **Kernel relevance.** This is the production confirmation of the recommended rule. The pave block IS the canonical sub-edge with a stable identity in a SHARED structure that both faces reference. Keel's N(C) subdivision = OCCT's paving of the section edge; Keel's child `EdgeId` = the pave block's identity in the shared DS; Keel's "both imprints consume the same edge list" = both faces referencing the same pave block. The one design correction Keel makes over OCCT: do the paving with the tolerant CANONICAL hash (Q2) so cross-operand pairs cannot mis-cluster, rather than a tolerance merge that OCCT then heals after the fact (dossier 47 argues against heal-after). **Verified** against the BOPDS pave-block / section-edge / BuildPCurves / AttachExistingPCurve model. Patent posture: OCCT is LGPL, methods published, no claim-reading risk.

### How ACIS realises the same
- **Citation.** Spatial Corp (Dassault), ACIS "Modeling Operations > Intersectors" and "Model Topology > COEDGE / Faces" (q-solid R17 mirror). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_mointrintr.htm ; http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_mtface.htm
- **Method / content.** ACIS computes the SSI once; the resulting intersection EDGE is a single topological entity shared by the two trimmed faces it bounds, and each face sees it through its own COEDGE with its own pcurve. A COEDGE "records the occurrence of an edge in a loop of a face," and an edge can occur in two or more faces' coedges (the radial cycle). The intersection edge is split at shared vertices and each segment is one edge shared to both faces. ACIS "correctly handles faces that span the seam(s) of a periodic surface," and the periodic seam is itself a named edge so loops stay in range.
- **Limitations.** Closed source; the once-and-share behavior is inferred from the documented intersector / COEDGE / pcurve model, not from source.
- **Kernel relevance.** Confirms the convention is universal: ONE edge entity, multiple coedges, multiple pcurves, computed once and shared. ACIS's COEDGE-per-face is exactly Keel's coedge referencing a child `EdgeId`; the radial cycle of coedges around the shared edge is the structure dossier 47's stitcher splices. **Verified** as documented behavior; SSI-once-and-share is **claimed** from the model.

### Requicha-Voelcker: subdivision creates the shared edges that merging reuses
- **Citation.** A. A. G. Requicha, H. B. Voelcker, "Boolean operations in solid modeling: boundary evaluation and merging algorithms," Proceedings of the IEEE 73(1), 1985. https://ieeexplore.ieee.org/abstract/document/1457376/
- **Method / content.** Boundary evaluation is subdivide, classify, merge. The SUBDIVISION stage splits each operand's boundary along its intersection with the other; the pieces produced ALREADY share the edges created by the intersection. MERGE connects over those shared boundaries; it does not re-derive them.
- **Limitations.** Pre-tolerant-kernel formulation; assumes exact subdivision. The principle (subdivision creates sharing, merge reuses it) transfers; the tolerant realization is Q2.
- **Kernel relevance.** This is the theoretical mandate for doing the subdivision ONCE and sharing it. Keel's bug is that subdivision happens TWICE (once per imprint), violating the Requicha-Voelcker design. The canonical-subdivision rule restores it: subdivide once, both imprints reuse. **Verified.** Pre-1995, no patent.

---

## Q2. The tolerant EdgeId hash: the stability condition

The subdivision of Q1 is only safe if the child `EdgeId`s are STABLE: two geometrically equal seam computations, or the same shared curve sampled in two traversal senses, must hash to the SAME id under tolerant-f64 perturbation, AND two geometrically distinct seams must NOT collide. This section gives the canonical orientation key, the once-only node snapping, and the soundness argument.

### Why a hash is needed even though Keel computes the seam once

Keel produces one `SeamCurve`, so why canonicalize at all? Three reasons. (1) The SAME curve is traversed in two senses by the two incident faces (dossier 46: each face's coedge runs the loop its own way), so the orientation key must collapse both senses to one id. (2) Node parameters from the four N(C) contributions are computed by different numeric paths (preimage crossing vs original-edge intersection) and may land sub-tolerance apart while meaning the same point; they must snap to one node. (3) For recovered-NURBS seams the fit is cached but re-evaluated, and re-evaluation perturbs the last bits; the id must survive that. The hash is the contract that makes the "compute once" guarantee robust against these three perturbations.

### The canonical orientation key (the deliverable)

> **Order the seam curve canonically by a deterministic key independent of operand order and traversal sense:**
> 1. **Primary axis.** Project the curve's two endpoints (or, for a closed loop, its two extremal points along a canonical frame axis derived from the two surfaces' axes) and orient the curve so it runs in increasing canonical-frame coordinate. For a closed seam, pick the start node as the one minimal under the tuple below.
> 2. **Ties broken by surface identity.** When the geometry is symmetric (a circle whose axis gives no preferred start), break ties by the lexicographically smaller tuple of the two incident surfaces' `EntityId`s. `EntityId` is stable within a session and deterministic, so the same surface pair always yields the same orientation.
> 3. **Curve-type canonical form.** For analytic seams, store the seam as a TYPED conic (circle / ellipse / parabola / hyperbola branch) with a canonical parametrization derived from the surfaces' axes (dossier 59 Q2), so the same surface pair always yields the identical canonical curve regardless of which operand was processed first. For recovered-NURBS, use the single cached certified fit.
> **The `EdgeId` of a sub-edge is then hash( canonical_seam_id, ordered_index_i, node_id_lo, node_id_hi ), where node ids come from the snapped node table below. Both operands compute the SAME tuple, so the SAME `EdgeId`.**

The key never consults raw coordinates at hash time. It consults the canonical curve identity (a function of the surface pair, computed once), an integer index, and node ids (integers into a snapped table). Hashing integers is exact; there is no float in the hash, which is what makes it collision-controlled.

### The once-only tolerant node snapping

> **At canonicalization, build a node table by snapping the raw N(C) parameters ONCE, with full geometric context, under a feature-size-relative tolerance (dossier 47: weld_eps_local = k * min(local_edge_lengths), k around 0.01 to 0.1). Two raw nodes within eps collapse to one table entry with one integer node id. This snap happens ONCE, before subdivision; it is NEVER repeated at stitch time. Stitch consumes integer node ids, not coordinates.**

This is the single allowed use of tolerance (dossier 47's discipline: tolerance is used once, at canonicalization, with full context; never as a coordinate compare at stitch). The snap is a clustering of parameter values along ONE curve, a 1D problem, far better-behaved than the 3D vertex weld dossier 47 forbids: the nodes lie on one curve, ordered by one parameter, so clustering is a sort-and-sweep with no cascade across dimensions.

### The soundness argument (the proof obligation)

**Claim A: geometrically equal seams cannot hash apart.** Two computations of the same seam (the two traversal senses, or a re-evaluated cached fit) produce the same canonical curve because the canonical curve is a function of the surface PAIR (axes plus `EntityId` tie-break), not of the traversal or the evaluation. Their node parameters differ only by sub-eps perturbation, so they snap to the same integer node ids. Same canonical curve plus same node ids plus same index gives the same tuple gives the same hash. The only way to hash apart is a perturbation larger than eps, which by the feature-size-relative choice of eps cannot occur between two computations of the SAME node (they differ by f64 rounding, orders of magnitude below k * feature_size).

**Claim B: geometrically distinct seams cannot collide.** Distinct seams come from distinct surface pairs (distinct `EntityId` tuples) or distinct curve types, so their canonical_seam_id differs, so the tuple differs. Within one seam, distinct nodes are separated by at least eps (the snap guarantees a minimum separation, the snap-rounding "minimum feature separation" property), so distinct sub-edges have distinct (node_id_lo, node_id_hi) pairs. A hash collision across distinct tuples is then only the cryptographic-hash birthday risk, controlled by a wide hash, not a geometric ambiguity. The geometric distinctness is preserved into the integer keys BEFORE hashing.

The soundness rests on the snap establishing a minimum separation (so distinct stays distinct) while collapsing sub-eps duplicates (so equal stays equal). This is exactly the snap-rounding soundness condition, imported below.

### Geometric hashing and canonical labeling (the technique)
- **Citation.** "Geometric hashing," Wikipedia (encode entities by an invariant key into a hash table for identity lookup). https://en.wikipedia.org/wiki/Geometric_hashing ; canonical-labeling background in V. Capoyleas, X. Chen, C. Hoffmann, "Generic naming in generative, constraint-based design," Computer-Aided Design 28(1), 1996; J. Kripac, "A mechanism for persistently naming topological entities in history-based parametric solid models," Computer-Aided Design 29(2), 1997. https://www.semanticscholar.org/paper/A-mechanism-for-persistently-naming-topological-in-Kripac/a5e40e5ac256c3d3c9beb7678ab9ffe6ac3d1e6f
- **Method / content.** Geometric hashing encodes a geometric entity by an invariant signature and stores / looks it up in a hash table, so recognition is by signature match, not by exhaustive coordinate compare. Canonical-labeling / persistent-naming methods (Kripac; Capoyleas-Chen) assign each topological entity a NAME derived from its generative provenance (the features / faces that created it) plus a deterministic disambiguation, so the same entity gets the same name across re-evaluations even as coordinates drift. Kripac names an edge by the ordered pair of faces that created it; ambiguity (several edges from the same face pair) is resolved by a secondary geometric key.
- **Limitations.** Persistent-naming methods target parametric re-evaluation (edit-and-replay), a harder problem than single-boolean cross-operand pairing; the full ambiguity machinery is more than Keel needs. Geometric hashing proper targets affine-invariant recognition, not seam identity; only the signature-not-coordinate principle transfers.
- **Kernel relevance.** This is the literature backing for the canonical key. Keel's canonical_seam_id is exactly Kripac's "edge named by its ordered face pair" (here the ordered surface `EntityId` tuple), and the ordered-index plus node-ids are the deterministic disambiguation. Keel already reasons about this machinery in dossier 52 (persistent naming); the curved-seam `EdgeId` is its application to boolean cross-operand pairing. **Verified** as the established naming principle; the specific tuple is **claimed** for Keel and matches Kripac's face-pair naming. Patent: Kripac's mechanism (1997) and related parametric-naming methods may have had patents (Autodesk lineage); they are now past the 20-year term (filed mid-1990s). The PRINCIPLE (name by provenance) is old prior art; design around any specific post-2006 naming-heuristic patent by using the plain ordered-`EntityId` tuple, which reads on Kripac / Capoyleas-Chen, not on later refinements.

### Snap rounding: the soundness conditions for the node snap
- **Citation.** J. Hobby, "Practical segment intersection with finite precision output," Computational Geometry 13(4), 1999; D. Greene, F. Yao, "Finite-resolution computational geometry," FOCS 1986; D. Halperin, E. Packer, "Iterated snap rounding," Computational Geometry 23(2), 2002; M. Hemmer, et al., "Stable snap rounding," Computational Geometry 46(4), 2013; "Snap rounding," Wikipedia. https://www.cgl.cs.tau.ac.il/projects/iterated-snap-rounding/ ; https://en.wikipedia.org/wiki/Snap_rounding ; https://dl.acm.org/doi/pdf/10.1145/1998196.1998226
- **Method / content.** Snap rounding maps arrangement vertices to representable points in a GLOBALLY topologically consistent way: it preserves the combinatorial structure (which point is on which segment, in what order) while collapsing sub-pixel features. Hobby and, independently, Greene gave the canonical method; the soundness property is that after snapping, every vertex is at the center of its pixel and no vertex lies in the interior of a non-incident edge's pixel neighborhood. Naive snapping CASCADES: snapping one vertex can push an edge onto another vertex, which must then also snap. Iterated snap rounding (Halperin-Packer) repeats until a minimum vertex-to-non-incident-edge separation holds, guaranteeing the separation at the cost of larger drift; stable snap rounding and snap-rounding-with-restore reduce the drift while keeping the separation.
- **Limitations.** Snap rounding is a 2D arrangement tool; applying the global-grid philosophy to 3D boolean assembly is the very thing dossier 47 forbids (it caused the thin-wedge collapse). The transferable part is the 1D-along-one-curve soundness condition, not the 2D grid.
- **Kernel relevance.** Keel's node snap is snap rounding restricted to ONE dimension (parameters along ONE seam curve), which sidesteps the cascade: there is no second segment for a snapped node to collide with, only earlier and later nodes on the same curve, so a single sort-and-sweep with a min-separation eps achieves the soundness condition directly (no iteration needed). The soundness condition Keel needs is exactly the snap-rounding guarantee: collapse sub-eps duplicates (equal stays equal) AND enforce a minimum separation (distinct stays distinct), which is Claims A and B. **Verified** as the soundness foundation; the 1D restriction is **claimed** as the reason Keel avoids the cascade. Hobby 1999 / Greene-Yao 1986 are old; no patent risk on the base method.

### Tolerant vertex/edge merging (Barequet-Sharir) as the contrastive anti-pattern
- **Citation.** G. Barequet, M. Sharir, "Filling gaps in the boundary of a polyhedron," Computer-Aided Geometric Design 12(2), 1995; G. Barequet, S. Kumar, "Repairing CAD models," IEEE Visualization 1997; the "Sealing Faceted Surfaces" watertight-CAD line. https://www.researchgate.net/publication/222495662_Filling_gaps_in_the_boundary_of_a_polyhedron ; https://fti.neep.wisc.edu/fti.neep.wisc.edu/pdf/fdm1389.pdf
- **Method / content.** Barequet-Sharir detect free edges (incident to one face only), split them for consistent length, then match groups by a distance-and-orientation heuristic and fill the gaps with new facets; vertices within a tolerance are moved together to seal gaps. This is heal-AFTER-the-fact merging by coordinate proximity.
- **Limitations.** It is a repair pass over an already-divorced boundary; it moves real geometry to close gaps a robust pipeline would never open. It is the anti-pattern dossier 47 names.
- **Kernel relevance.** Listed as the CONTRAST. Keel must NOT seal the curved seam by Barequet-Sharir coordinate vertex-merging at stitch (that is exactly the `UnassemblableSeam`-then-weld posture). The canonical subdivision makes the two operands' seam edges identical by construction, so there is no free edge to seal and no tolerance vertex-move at stitch. The free-edge / gap that Barequet-Sharir repairs is precisely the symptom the canonical `EdgeId` prevents. **Verified** as the documented repair method, cited to mark the boundary of what Keel does NOT do.

---

## Q3. Sense relative to each incident face

Once the seam is ONE canonical edge with multiple pcurves (one per incident face), each fragment's coedge sense must be derived from THAT face's natural normal, NOT from the traversal order in which the seam happened to be canonicalized. This is what makes the radial-2 pairing consistent across the two operands and leaves the Weiler region walk unchanged.

### The rule (tied to dossier 46)

> **The canonical orientation (Q2) fixes the seam edge's INTRINSIC direction once, for hashing. It does NOT dictate any coedge's sense. For each incident face F, the coedge's sense is derived from F's own outward normal by the dossier-46 convention: `effective_sense = face.sense XOR kept.reversed`, `n_out = effective_sense x natural_normal`, and the coedge runs the loop direction that keeps material on the left looking down `n_out`. Two faces sharing the canonical seam edge therefore traverse it in OPPOSITE senses (the half-edge consistency invariant), regardless of the seam's canonical direction, because each derives its sense from its OWN normal.**

This is the crux that makes canonicalization safe: the canonical direction is a hashing convenience that both operands agree on, but it is NEVER read as a coedge orientation. If a coedge inherited the canonical direction directly, then operand A's and operand B's coedges on the shared edge would run the SAME way (both follow the canonical direction), the radial cycle would not alternate, and the splice would fail. Deriving each coedge's sense from its face's natural normal restores the alternation: the two faces are on opposite sides of the seam (a manifold cut), their outward normals point oppositely across it, so their coedges run oppositely along it. The radial-2 pairing is then antiparallel by construction.

### Why the Weiler region walk is unchanged
- **Citation.** K. Weiler, "The Radial Edge Structure," Geometric Modeling for CAD Applications, North-Holland, 1988; Keel dossiers 46 (n_out = sense x natural), 47 (radial-splice stitcher), 59 (curved finalize unchanged).
- **Method / content.** The Weiler walk sorts coedges around an edge by dihedral angle into a radial cycle, then walks face-sides into regions. It needs, per coedge, a consistent outward normal to sort by. If every coedge's sense comes from the dossier-46 convention applied to its own face, the dihedral sort is well-defined and the walk is identical whether the edge is a planar line segment or a canonical conic seam. The seam being one entity with multiple pcurves changes the geometry the walk reads (a conic instead of a line) but NOT the combinatorics (group by `EdgeId`, sort by dihedral, walk), which is what dossier 47's stitcher and dossier 59's finalize already do.
- **Limitations.** At the apex / a tangency the natural normal is undefined, so the dihedral sort must use a sample perturbed off the singularity (dossier 59 Q3). That is a finalize-stage robustness fix, orthogonal to the identity work here.
- **Kernel relevance.** This closes the loop: canonical identity (Q1, Q2) plus per-face sense (Q3) make the curved seam's coedges group and alternate exactly as planar coedges do, so the EXISTING stitcher and finalize consume them with no change. The canonical direction is for hashing only; the sense is per-face; the walk is unchanged. **Verified** assembly of dossier 46 / 47 / 59 plus Weiler. Pre-1995 Weiler, no patent.

---

## Q4. Interaction with dossier 64: the periodic-domain split contract

Dossier 64 (the periodic-domain split) decides WHICH regions exist when a periodic / apex face's parameter domain is cut: it arranges the (u, v) domain, accounting for the seam-slit and apex, into bounded regions. This dossier (65) decides WHICH canonical sub-edges BOUND those regions. The two must compose without a second reconciliation pass, or the divorce reappears at a different stage.

### The contract (the deliverable, stated as an interface)

> **Dossier 65 OWNS the node set and the sub-edge identities; dossier 64 CONSUMES them. Concretely:**
> 1. **65 publishes N(C) and the child `EdgeId`s FIRST.** The canonical subdivision (Q1) runs before the periodic-domain arrangement. Its output is the ordered node table (snapped, with integer node ids) and the ordered sub-edge list with child `EdgeId`s, lifted into each incident periodic face's (u, v) domain as pcurve segments.
> 2. **64 arranges the domain USING those exact nodes.** The periodic-domain arrangement of dossier 64 takes the seam-slit (u = 0 / u = 2*pi), the apex / pole, AND the published seam pcurve segments as its INPUT edges. It does NOT compute its own intersection nodes; it uses N(C)'s nodes where the seam meets the slit and the apex, which 65 already snapped. The arrangement's vertices are exactly N(C)'s node ids plus the slit / apex vertices.
> 3. **64 returns regions as cycles of published sub-edge ids.** Each region dossier 64 reports is bounded by a cycle of (child seam `EdgeId`s, slit sub-edges, apex incidences, original-edge pave blocks). Every boundary element already has an identity from 65; 64 introduces no new edge geometry, only the region grouping.
> 4. **No second reconciliation.** Because 64's vertices and edges are 65's node ids and child `EdgeId`s, the regions compose with the cross-operand pairing automatically: operand A's region and operand B's region on the same seam reference the SAME child `EdgeId`s, so the stitch pairs them by identity with no re-matching of the domain arrangement against the seam.

The contract is a strict producer / consumer split: **65 is the single source of seam nodes and identities; 64 is a pure arranger over them.** If 64 were allowed to compute its own seam-slit crossing of the conic, that crossing would be a DIFFERENT point than 65's snapped node (the exact bug of Add.273, relocated into the domain arrangement), and a second reconciliation would be needed. Forbidding 64 from re-intersecting closes that door.

### Why this composition is sound
- **Citation.** C. M. Hoffmann, "Geometric and Solid Modeling," Morgan Kaufmann, 1989, and "The problems of accuracy and robustness in geometric computation," IEEE Computer 22(3), 1989; K. Sugihara, M. Iri, et al., "Topology-Oriented Implementation," Algorithmica 27, 2000. https://www.cs.purdue.edu/cgvlab/www/resources/papers/Hoffmann-IEEE-1989-The_problems_of_accuracy_and_robustness_in_geo_computation.pdf ; https://link.springer.com/article/10.1007/s004530010002
- **Method / content.** Hoffmann: robustness fails when the same geometric fact is computed by two independent numeric paths that disagree (the geometric-topological divorce); the cure is a single authoritative computation that all consumers reference. Sugihara-Iri: decide topology combinatorially ONCE, let geometry conform; never let two stages independently re-derive the same incidence. The 65 / 64 contract is exactly this: the seam incidence (where the conic meets the slit and apex) is computed ONCE by 65, and 64 conforms to it rather than recomputing.
- **Limitations.** Requires 64 and 65 to share the node table data structure; a clean module boundary must pass the snapped node ids, not coordinates, across the interface.
- **Kernel relevance.** This is the design that keeps the two dossiers from reintroducing the divorce at the seam-slit. The single-authoritative-computation principle (Hoffmann) and topology-once (Sugihara-Iri) mandate the strict producer / consumer split. **Verified** principle. Hoffmann 1989 and Sugihara-Iri foundational, no patent.

---

## Patent awareness

- **Requicha-Voelcker boundary evaluation, Weiler radial edge, Hoffmann consistency, Hobby / Greene-Yao snap rounding, geometric hashing (1985 to 1999, mostly pre-1995)** are old prior art, any patents expired. Foundation to cite and build on. Prefer them.
- **OCCT BOPDS / pave blocks / section edges / BuildPCurves / AttachExistingPCurve / SameParameter** is LGPL open source; the methods are published and freely usable, no claim-reading risk. Keel may follow the pave-block / section-edge / attach-existing-pcurve structure directly.
- **ACIS section-edge / COEDGE sharing** is documented public technical articles; the underlying SSI / shared-edge / pcurve methods are standard prior art, not novel claims.
- **Kripac persistent naming (1997) and Capoyleas-Chen generic naming (1996)** are past the 20-year term; the naming-by-ordered-face-pair principle is old prior art. Some parametric-naming refinements were patented in the Autodesk / PTC lineage; those filings are now expired or expiring. Keel's plain ordered-`EntityId`-tuple key reads on Kripac / Capoyleas-Chen, the design-around for any later refinement.
- **Post-2006 snap-rounding variants (iterated, stable, with-restore; Halperin-Packer 2002 borderline, Hemmer stable 2013)** are recent academic, no patent flagged for the base technique, but Keel uses only the 1D node-snap soundness condition (the old Hobby / Greene-Yao guarantee), not the 2D iterated machinery, so even if a specific variant were claimed, Keel does not practice it.
- **Post-2006 curved-boolean / pcurve-repair / topological-naming patents** may exist around specific healing and re-evaluation heuristics. Keel's posture (consistency by construction via canonical `EdgeId` at subdivision, NOT heal-after-the-fact merge, NOT parametric re-evaluation replay) is the deliberate design-around: Keel never produces a divorced seam to repair and never replays a feature tree, so it reads on Requicha / Hoffmann / Kripac prior art, not on later repair or naming patents. Verify any specific healing or naming heuristic against live patents before copying; the recommendation copies none.

Net: **build on the pre-1999 subdivision / naming / snap-rounding / consistency foundation and the LGPL OCCT pave-block structure; achieve identity by construction (canonical `EdgeId` at the single shared subdivision), the design-around of any post-2006 repair or naming patent; the path is clear.**

---

## Synthesis: ADOPT / AVOID / INVESTIGATE

**ADOPT.**
- **The canonical subdivision (Q1):** subdivide the ONE shared seam curve ONCE, at the canonical UNION N(C) of every node either operand or the seam needs, into an ordered sub-edge list with stable child `EdgeId`s; both imprints consume the SAME list. This is the curved `subdivide_seam_ring` and the single most important new piece. It removes `UnassemblableSeam` at its root (no second subdivision, nothing to reconcile).
- **The tolerant `EdgeId` hash (Q2):** canonical orientation key = order by a canonical frame axis, ties by the lexicographic ordered tuple of the incident surfaces' `EntityId`s, curve stored as typed canonical conic / cached NURBS fit; `EdgeId = hash(canonical_seam_id, ordered_index, node_id_lo, node_id_hi)` over INTEGER keys from a once-snapped node table. No float in the hash, no coordinate compare at stitch.
- **The once-only feature-size-relative node snap (Q2)** as the single licensed use of tolerance, a 1D sort-and-sweep along the curve with a minimum-separation guarantee (snap-rounding soundness, no cascade because it is 1D).
- **Per-face sense derivation (Q3):** each coedge's sense from its own face's `n_out = sense x natural` (dossier 46), NOT from the canonical direction; the canonical direction is for hashing only. This keeps the radial-2 pairing antiparallel and the Weiler walk unchanged.
- **The 65 / 64 producer-consumer contract (Q4):** 65 publishes nodes and child `EdgeId`s; 64 arranges over them and returns regions as cycles of those ids; 64 never re-intersects. One authoritative seam-incidence computation, no second reconciliation.

**AVOID.**
- **Subdividing the seam inside each `imprint_operand`** (the current bug): two operands split the same curve at two different points, no shared vertex, `UnassemblableSeam`. Subdivide once, before either imprint.
- **Any coordinate compare or tolerance vertex-merge at stitch time** to pair the two operands' seam edges (the Barequet-Sharir heal-after anti-pattern). Pair by integer `EdgeId`.
- **Letting a coedge inherit the canonical seam direction as its sense.** That makes both operands run the shared edge the same way and breaks the radial alternation. Derive sense per face.
- **Letting dossier 64 compute its own seam-slit / apex crossing of the conic.** That relocates the Add.273 divorce into the domain arrangement. 64 consumes 65's snapped nodes only.
- **A float in the hash key.** Hash integer node ids and indices; keep every tolerance decision in the one-time snap, never in the hash.

**INVESTIGATE.**
- The exact canonical frame and tie-break for each seam class (cone-plane circle / ellipse, cone-cylinder quartic, sphere-plane circle, torus-plane) so the orientation key is unambiguous for every analytic pair Keel supports.
- The minimum-separation eps as a function of LOCAL feature size along the curve (not global), and proof that it both collapses re-evaluation jitter and separates genuinely-distinct nodes for the recovered-NURBS case.
- The shared data structure across the 64 / 65 boundary: the node table and child-`EdgeId` map as the single passed artifact, so the producer / consumer split is enforced by the type, not by convention.

**RECOMMENDED FIRST MILESTONE (narrowest honest slice with an exact oracle).** **A single cylinder lateral cut by ONE seam that crosses the periodic seam-slit, assembled by canonical `EdgeId` with mass == mesh, AND a byte-equality oracle on the hash.** Take a cylinder cut by a tilted plane so the elliptical seam crosses the u = 0 / u = 2*pi slit at one point. The canonical subdivision must (a) snap that one slit-crossing node ONCE, (b) produce two sub-edges with stable child `EdgeId`s, (c) have BOTH operands reference those same two ids. **Exact oracle, two parts:** (1) geometric, the resulting two bands' areas and the enclosed volume are closed-form, mass == mesh == the closed-form value, fragment count out == KeptFace count in; (2) IDENTITY, compute the seam from operand A's direction and from operand B's direction and assert the child `EdgeId`s are byte-equal (the stability condition of Q2 made directly testable). This is the smallest case that exercises the slit-crossing node union (Q1 contribution 1) and the tolerant hash (Q2) together, with a hard exact oracle on BOTH the geometry and the identity, and it directly attacks the `UnassemblableSeam` decline. It needs no new stitch or finalize code.

**FOLLOW-UP LADDER.**
- Rung 1 (first milestone): one slit-crossing seam on a cylinder, mass == mesh AND byte-equal `EdgeId` oracle.
- Rung 2: cone lateral with a seam terminating AT the apex (off-degenerate), exercising the apex node (Q1 contribution 2) and the apex-perturbed dihedral sort.
- Rung 3: TWO non-mutually-intersecting seams on one periodic lateral (dossier 59's first milestone) now backed by this dossier's canonical subdivision, mass == mesh, both seams' `EdgeId`s byte-equal across operands.
- Rung 4: two seams that MUTUALLY cross on the face, exercising N(C) contribution 3 (shared self / other-seam crossing) and the 65 / 64 contract on a non-trivial domain arrangement.
- Rung 5: recovered-NURBS seam (torus-involving), the cached certified fit re-evaluated in two directions, asserting the hash survives re-evaluation jitter (Q2 reason 3).

**DECLINE FOREVER (unchanged from dossier 59 Q5, restated for identity).** A seam whose node set N(C) cannot be made well-separated even at the smallest feature-size eps (two genuinely-distinct required nodes closer than the tolerant-edge tube, dossier 42) is a sub-tolerance-sliver DECLINE: the canonical subdivision would have to either merge two distinct nodes (losing a real fragment) or keep a sub-tolerance sub-edge (no well-defined pcurve). DECLINE the named class, never emit the sliver. Self-intersecting seams and apex-degenerate conics remain DECLINE per dossier 59. Every residual is triaged against Q1 / Q2 FIRST: an `UnassemblableSeam` is presumed a missing-canonical-subdivision bug (FIXABLE) until the canonical `EdgeId` work is in place and proven by the byte-equal oracle.

**The one-line verdict.** Canonical curved-seam identity is delivered as **ONE tolerant subdivision of the ONE shared seam curve at the canonical union of all required nodes, producing stable integer-keyed child `EdgeId`s that both operands reference**, with per-face sense from dossier 46 and a strict producer / consumer contract to dossier 64, which dissolves the Add.273 divorce at its root and lets the unchanged dossier-47 stitcher pair every curved seam by identity, never by coordinate.

---

## References

1. A. A. G. Requicha, H. B. Voelcker. "Boolean operations in solid modeling: boundary evaluation and merging algorithms." Proceedings of the IEEE 73(1), 1985. https://ieeexplore.ieee.org/abstract/document/1457376/
2. Open CASCADE Technology. "Boolean Operations" user guide (BOPAlgo_PaveFiller, BOPDS_DS, BOPDS_PaveBlock, MakeBlocks, BuildPCurves). https://documentation.help/Open-Cascade/occt_user_guides__boolean_operations.html
3. Open CASCADE Technology. BOPDS_DS Class Reference (pave blocks, shared section edges). https://dev.opencascade.org/doc/occt-7.6.0/refman/html/class_b_o_p_d_s___d_s.html
4. Open CASCADE Technology. BOPAlgo_PaveFiller Class Reference (MakeBlocks, section-edge post-treatment, BuildPCurveForEdgeOnFace, AttachExistingPCurve). https://dev.opencascade.org/doc/occt-7.3.0/refman/html/class_b_o_p_algo___pave_filler.html
5. Spatial Corp (Dassault Systemes). ACIS "Modeling Operations > Intersectors" (CCI / CSI / SSI, shared intersection edge). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_mointrintr.htm
6. Spatial Corp (Dassault Systemes). ACIS "Model Topology > Faces / COEDGE" (coedge per face, edge shared to two or more faces, periodic seam edge). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_mtface.htm
7. K. Weiler. "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling." Geometric Modeling for CAD Applications, North-Holland, 1988.
8. C. M. Hoffmann. "Geometric and Solid Modeling: An Introduction." Morgan Kaufmann, 1989.
9. C. M. Hoffmann. "The problems of accuracy and robustness in geometric computation." IEEE Computer 22(3), 1989. https://www.cs.purdue.edu/cgvlab/www/resources/papers/Hoffmann-IEEE-1989-The_problems_of_accuracy_and_robustness_in_geo_computation.pdf
10. K. Sugihara, M. Iri, H. Inagaki, T. Imai. "Topology-Oriented Implementation: An Approach to Robust Geometric Algorithms." Algorithmica 27, 2000. https://link.springer.com/article/10.1007/s004530010002
11. J. Kripac. "A mechanism for persistently naming topological entities in history-based parametric solid models." Computer-Aided Design 29(2), 1997. https://www.semanticscholar.org/paper/A-mechanism-for-persistently-naming-topological-in-Kripac/a5e40e5ac256c3d3c9beb7678ab9ffe6ac3d1e6f
12. V. Capoyleas, X. Chen, C. M. Hoffmann. "Generic naming in generative, constraint-based design." Computer-Aided Design 28(1), 1996.
13. "Geometric hashing." Wikipedia (invariant signature into a hash table for identity lookup). https://en.wikipedia.org/wiki/Geometric_hashing
14. J. Hobby. "Practical segment intersection with finite precision output." Computational Geometry 13(4), 1999.
15. D. Greene, F. Yao. "Finite-resolution computational geometry." FOCS 1986.
16. D. Halperin, E. Packer. "Iterated snap rounding." Computational Geometry 23(2), 2002. https://www.cgl.cs.tau.ac.il/projects/iterated-snap-rounding/
17. M. Hemmer, et al. "Stable snap rounding." Computational Geometry 46(4), 2013. https://dl.acm.org/doi/pdf/10.1145/1998196.1998226
18. "Snap rounding." Wikipedia (globally topologically consistent finite-precision rounding). https://en.wikipedia.org/wiki/Snap_rounding
19. G. Barequet, M. Sharir. "Filling gaps in the boundary of a polyhedron." Computer-Aided Geometric Design 12(2), 1995. https://www.researchgate.net/publication/222495662_Filling_gaps_in_the_boundary_of_a_polyhedron
20. G. Barequet, S. Kumar. "Repairing CAD models." IEEE Visualization, 1997.
21. N. M. Patrikalakis, T. Maekawa. "Shape Interrogation for Computer Aided Design and Manufacturing." Springer, 2002 (MIT hyperbook edition). https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/

### Related Keel dossiers (build on, not repeated)
- `docs/research/kernel/47-boolean-result-assembly.md` (the `EdgeId` radial-splice stitcher, the shell-closure / drop-prevention invariant, identity-from-imprint, the count check, the feature-size-relative weld eps, retire-the-soup). The planar base whose `subdivide_seam_ring` this dossier is the curved analog of.
- `docs/research/kernel/59-curved-boolean-assembly.md` (the curved assembly conclusion: canonical curved-seam identity plus seam-aware periodic / apex split feed the UNCHANGED stitcher; this dossier specifies the canonical identity that dossier 59 named the single most important new piece).
- `docs/research/kernel/52-persistent-naming-keel.md` (the canonical-key / persistent-naming machinery, Kripac face-pair naming, the `EntityId` tuple key Keel already reasons about).
- `docs/research/kernel/46-face-orientation-sense-convention.md` (`n_out = sense x natural`, the per-face sense derivation Q3 uses so the canonical seam direction is for hashing only).
- `docs/research/kernel/64-periodic-domain-split.md` (the periodic-domain arrangement; the producer / consumer contract of Q4 makes 65 the source of seam nodes and ids and 64 the pure arranger over them).
- `docs/research/kernel/58-ssi-curved-matrix.md` (the analytic SSI matrix producing the one shared seam curve this dossier subdivides).
- `docs/research/kernel/39-coincident-tangent-face-booleans.md` (two-sided neighborhood classification feeding the kept-fragment outward normal Q3 derives sense from).
- `docs/research/kernel/42-russian-c3d-literature.md` (tolerant edge as a tube of radius t, the sub-tolerance-sliver decline boundary for the node-separation eps).
