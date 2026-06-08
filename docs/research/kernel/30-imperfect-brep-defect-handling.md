# 30. Robust Operations on Imperfect B-Reps: Defect Taxonomy and Per-Defect Repair/Tolerate Techniques

Research dossier supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition.

## Scope and motivation

Handling dirty geometry is, in this project's framing, the single most valuable kernel capability. A kernel that only operates on geometry it authored itself is a demo. The real world arrives broken: translated bodies, scan-derived surfaces, legacy archives, and the output of the kernel's own near-degenerate operations all carry gaps, slivers, self-intersections, inverted normals, and tiny junk features. The kernel's job is not to refuse them. It is to repair what can be repaired cheaply, tolerate what cannot, and fall through to a robustness floor that always yields a valid result.

This file is the concrete companion to **file 29** (graceful-degradation philosophy: the heal-vs-tolerate-vs-fail policy, confidence levels, the degradation ladder). File 29 sets the philosophy. This file is the per-defect catalog and the per-defect algorithms. It deliberately does **not** re-cover the import pipeline (stitch / simplify / geombuild) or surface extension: that is **file 13** (import healing and surface extension). Where file 13 organizes defects by the pipeline that consumes them, this file organizes them by **defect class** and gives, for each, the detection criterion and the menu of repair-or-tolerate techniques, with emphasis on the hard cases: virtual topology, self-intersection, slivers and degeneracies, snapping pitfalls, tolerant reasoning, orientation repair, and mesh rescue.

Organization:

- **Part A: The defect taxonomy** (a per-defect handling catalog, building on the morphologic taxonomy of file 13 but reorganized for handling).
- **Part B: Per-technique sections** (virtual topology, self-intersection, slivers/short edges, degeneracies, snapping/merging, tolerant reasoning, orientation repair, mesh rescue, validation), each with per-source Citation / Technique / Limitations / Kernel relevance entries.
- **Part C: The defect-handling toolkit for Keel** (synthesis: the per-defect playbook, virtual topology as a first-class tool, mesh rescue as the robustness floor, the re-validation contract).
- **References.**

Style note for this file: no em-dashes anywhere; commas, colons, and periods only.

---

# PART A: THE DEFECT TAXONOMY

The morphologic taxonomy from file 13 (Gonzalez-Lluch et al. 2017, and the PDQ/SASIG and VDA model-quality criteria) gives the diagnostic vocabulary. Here those classes are reorganized as a **handling catalog**: each defect gets a definition, a detection criterion, and the candidate responses (repair, tolerate, or rescue). The per-technique sections in Part B then develop the algorithms.

### A.1 Gaps and overlaps between faces

Two faces that should share an edge are separated by empty space (a **gap**) or their material regions overlap (an **overlap**). Detected by: boundary edges (edges incident to exactly one face) that lie within a proximity tolerance of another boundary edge, with consistent or anti-consistent orientation. Gaps below vertex/edge tolerance are absorbed by tolerant entities (B.6) or closed by snapping/zippering (B.5); gaps above tolerance need stitching (file 13) or face creation. Overlaps are the dangerous twin: snapping them naively creates self-intersections (B.5).

### A.2 Cracks at edges

A **crack** is a gap localized at a shared edge: the two coedges that should bind one edge are two distinct curves a sliver apart, so the shell is a face soup rather than a sewn shell. This is the single most common translation defect. Handling: merge the two curves into one tolerant edge whose tolerance covers the deviation (B.6), or geometrically re-intersect the two surfaces to compute a shared exact edge (file 13 geombuild).

### A.3 Self-intersecting faces and self-intersecting solids

A **self-intersecting face** has a boundary loop that crosses itself in parameter space, so its interior is ill-defined. A **self-intersecting solid** has two distinct portions of its boundary passing through each other in 3D. Detection is a surface-surface (or face pairwise) intersection test, restricted to non-adjacent faces, plus a per-face loop-self-intersection test in (u,v). Resolution: trim away the inverted/overlapping region, or fall to mesh rescue where generalized winding numbers (B.7) define inside/outside even through the tangle. See B.2.

### A.4 Degenerate faces: zero-area, knife-edge, needle, sliver

- **Zero-area / collapsed face**: a face whose surface area is below an area tolerance, often collapsed to a line or point.
- **Knife-edge face**: a face whose two long boundaries meet at a near-zero dihedral, from near-tangent trims.
- **Needle**: a triangle/face with one very small angle and two long edges.
- **Sliver**: high aspect ratio, small area, two near-parallel long edges and two short ends.

Detection: area below tolerance, or aspect ratio above a threshold, or minimum inscribed-circle radius below tolerance. Handling: collapse to an edge or vertex (B.3), or suppress via virtual topology by absorbing into a neighbor (B.1).

### A.5 Short edges and sliver faces

A **short edge** has length below a length tolerance (often the same family as the sliver: a sliver face is bounded by short ends). Handling: collapse the edge, fusing its two vertices (B.3), subject to a shape-fidelity check; or absorb both endpoints into a tolerant vertex; or hide via a virtual (composite) edge (B.1).

### A.6 Tiny features below tolerance

Small fillets, blends, bosses, holes, or chamfers whose size is below the downstream operation's or mesh's resolution. Not "errors" in the validity sense, but they break meshing and inflate cost. Handling is defeaturing/suppression: remove with real CAD operations (geometric), or hide with virtual topology (topological only), or decimate on a discrete proxy (B.1, B.3).

### A.7 Non-manifold junk and stray entities

Edges shared by more than two faces where two were intended; isolated (acorn) vertices; wire edges dangling into space; lamina faces with no volume; bodies that mix a solid region with stray sheets. Note: intended non-manifold geometry is legitimate (file 02). The defect is **unintended** non-manifoldness. Detection: edge-use count not equal to 2 in a solid shell, vertices with no incident face, etc. Handling: delete stray entities, split intended non-manifold apart, or flag for the operation to treat locally.

### A.8 Duplicate / coincident vertices, edges, faces

Two entities occupying the same location that should be one. The classic symptom of "exploded" translation where every face is independent. Detection: spatial hashing / proximity within tolerance. Handling: merge (the heart of stitching). The merge must be transitive and tolerance-consistent to avoid the cascading-snap pathology (B.5).

### A.9 Inconsistent or inverted orientation/normals

Adjacent faces disagree on which side is solid, so the shell is not coherently orientable; or the whole solid is inside-out. Detection: across a shared edge, the two coedges should traverse it in opposite senses; a violation means one face is flipped. Globally, the outward normal should point away from the interior. Handling: flood-fill orientation propagation plus a global ray-cast/winding test to fix the absolute sense (B.8).

### A.10 Invalid or non-closed loops

A face's boundary wire does not close, traverses an edge twice, or has the wrong nesting of outer vs inner loops. Detection: loop traversal fails to return to start; inner-loop containment test fails. Handling: re-order/re-close coedges, recompute pcurves, or rebuild the loop from the 3D edges.

### A.11 T-junctions and hanging edges

A **T-junction** is a vertex of one face landing mid-edge of a neighbor, so the two faces share only part of an edge: the meshes/topology do not match across the boundary. A **hanging (dangling) edge** is a free edge protruding from a vertex into a face interior. Handling: imprint the T-vertex onto the neighbor to split its edge (making the junction conformal), or, for meshing, hide the T with a virtual edge so the composite boundary is matched (B.1).

### A.12 Near-tangencies and near-degeneracies

Two surfaces that meet at a near-zero angle, or an intersection curve that grazes a face boundary, or a fillet whose radius approaches the local feature size. These are not defects in the body, they are **defects in the operation's conditioning**: the intersector produces an unstable or topologically wrong result. Handling: snap-to-exact-tangent when within tolerance, widen tolerance locally for the operation, or perturb symbolically. This is where tolerant reasoning (B.6) earns its keep.

### A.13 Free / dangling edges and open shells

Boundary edges that bound a hole in what should be a closed solid. Detection: any edge with a single coedge in a body declared solid. Handling: stitch to a partner if one exists within tolerance, build a missing face to cap the hole, or accept the body as a sheet body if open was intended.

This taxonomy is the diagnostic spine. Every entry maps to one or more techniques below.

---

# PART B: PER-TECHNIQUE SECTIONS

## B.1 Virtual topology: tolerate-by-hiding, the central technique

Virtual topology is the most important idea in this file. Rather than repairing bad geometry, it changes only the **topology** the downstream operation sees: a cluster of small or ugly faces is presented as one logical **composite face**, a chain of short edges as one **virtual edge**, without touching the underlying surfaces. The geometry is left exactly as imported (so nothing is approximated away), but meshers, booleans, and queries see clean, large, well-conditioned entities. For meshing and many operations this is strictly better than geometric repair, because it is reversible, introduces zero shape error, and cannot create new self-intersections.

### Source: Sheffer, Bidarra, Quadros and the virtual-topology operator family

**Citation.** Sheffer, A., Blacker, T., Clements, J., & Bercovier, M. (1997). Virtual topology operators for meshing. *International Journal of Computational Geometry & Applications*. See also Sheffer, A. (2001). Model simplification for meshing using face clustering. *Computer-Aided Design, 33*(13), 925-934. https://doi.org/10.1016/S0010-4485(00)00116-0

**Technique.** Virtual topology defines a secondary topological layer over the real B-rep. Two core operators: **merge** replaces a set of adjacent faces by a single virtual face whose "surface" is the union of the originals (a composite/parametric-free face the mesher walks via its real sub-faces but treats as one region), removing the shared internal edges from view; **collapse** removes a small edge or face by absorbing it into a neighbor, fusing its bounding entities. Virtual vertices and virtual edges arise as the boundaries of merged regions. Sheffer's 1997 operators are paired with **face clustering** (2001): planar facets generated per face are clustered into regions of restricted curvature and bounded distance deviation, and each cluster becomes one virtual face suitable for projection-based meshing. The model is simplified topologically while the geometry is preserved exactly. Small features below mesh size are suppressed: a short edge is collapsed in the virtual layer, a sliver face is merged into its larger neighbor, a T-junction is hidden by merging across it. Crucially, the operation is recorded so it can be undone and so analysis results can be mapped back to the real faces.

**Limitations.** A composite face has no single parametric domain, so any operation that needs a global (u,v) on the face (e.g. classic projection meshing) must either remesh per sub-face or refit a proxy surface; merge can produce composite faces that are non-developable and awkward to parameterize. Virtual topology hides geometry, it does not fix it: a self-intersecting underlying surface remains self-intersecting, so virtual topology is a tolerate, not a cure, for that class. The bookkeeping (mapping virtual to real) is the implementation cost.

**Kernel relevance.** This is a first-class subsystem for Keel, not an afterthought. Model a `VirtualTopology` overlay: composite faces and virtual edges referencing the real entities beneath, with merge/collapse/split operators and a forward/inverse map. It lets meshing, hidden-line, and selection see clean topology while booleans and exact queries can still drop to the real geometry. It is the cheapest, safest response to slivers, short edges, tiny features, and T-junctions, and it should be tried before any geometric repair.

### Source: Inoue, Itoh, Yamada, Furuhata, Shimada: face clustering for large-scale CAD

**Citation.** Inoue, K., Itoh, T., Yamada, A., Furuhata, T., & Shimada, K. (2001). Face clustering of a large-scale CAD model for surface mesh generation. *Computer-Aided Design, 33*(3), 251-261. https://doi.org/10.1016/S0010-4485(00)00124-X

**Technique.** A greedy face-clustering pre-process for meshing models with thousands of faces, many smaller than the target element size. Faces are merged into clusters such that each cluster projects cleanly onto a simple parametric surface (a plane or simple primitive). Clustering is driven by geometric indices scoring "preferable cluster shape": planarity/curvature deviation within the cluster, compactness, and boundary smoothness. The final mesh is the aggregate of per-cluster sub-meshes, eliminating manual clean-up. This is virtual topology specialized to projection meshing: the cluster is the composite face.

**Limitations.** Tailored to projection-based surface meshing; the assumption that each cluster maps to a simple surface limits cluster shapes. Greedy clustering is order-dependent and not globally optimal.

**Kernel relevance.** The clustering objective (regions of bounded curvature/distance deviation) is the criterion Keel's merge operator should use to decide which faces can safely become one composite face. Reuse this as the auto-virtual-topology policy: cluster, then expose clusters as composite faces to the mesher.

### Source: Quadros & Owen: defeaturing on a discrete proxy

**Citation.** Quadros, W. R., & Owen, S. J. (2009/2012). Defeaturing CAD models using a geometry-based size field and facet-based reduction operators. *Proc. 18th International Meshing Roundtable / Engineering with Computers*. https://doi.org/10.1007/s00366-011-0252-8

**Technique.** A defeaturing method that never touches the CAD B-rep. The model is faceted into a **discrete B-rep**; a geometry-based **size field** estimates the locally desired mesh size from curvature and proximity; facets in regions where features are smaller than the size field are marked irrelevant and removed by **facet-based reduction operators** (edge collapses and equivalent local ops). Validity of the discrete model's geometry and topology is maintained throughout, and the original CAD model is left unaltered. This sits between virtual topology (pure topological hiding) and geometric repair: it removes tiny features but only on a separate proxy.

**Limitations.** The defeatured result is a mesh/discrete model, not an editable B-rep: fidelity is bounded by facet resolution, and you do not get a clean exact CAD body back. Size-field thresholds need tuning.

**Kernel relevance.** This is the template for Keel's "defeature for analysis" path: build a discrete proxy, suppress sub-size-field features there, mesh the proxy, and keep the exact body untouched for downstream exact operations. It pairs naturally with mesh rescue (B.7) as the discrete representation.

---

## B.2 Self-intersection detection and resolution

Self-intersections arise from offsets (file 05 has the offset context), from sweeps along high-curvature paths, from booleans on near-tangent inputs, and from imported garbage. They are the defect most likely to make a downstream operation produce a topologically wrong result silently.

### Source: Self-intersection removal in mesh offsetting and the offset connection

**Citation.** Jung, W., Shin, H., & Choi, B. K. (2004). Self-intersection removal in triangular mesh offsetting. *Computer-Aided Design and Applications, 1*(1-4), 477-484. https://doi.org/10.1080/16864360.2004.10738290

**Technique.** When a surface is offset by distance d, regions of concave curvature with radius below d fold over and the offset self-intersects. The method offsets every facet, then detects the self-intersection loops where folded regions cross, and **removes the invalid (interior, wrongly-oriented) sheets**, keeping only the outer envelope. Detection is pairwise triangle-triangle intersection among non-adjacent facets; the kept region is the one consistent with the offset direction. This is the canonical local-fold case: the connection to general B-rep self-intersection is that any operation producing a degenerate fold (offset, thick-shell, large fillet) must trim the fold the same way.

**Limitations.** Mesh-resolution dependent; the trimming is a meshing answer, not a smooth retrim of the exact surface. Tangle topology can be complex with many spurious loops.

**Kernel relevance.** Keel's offset/shell/large-fillet operators must run a self-intersection pass after construction: detect fold loops, trim to the valid envelope. For exact surfaces, retrim the NURBS; for the fallback, do it on the tessellation.

### Source: Edge swapping, edge hammering, face lifting

**Citation.** Removing self-intersections of a triangular mesh by edge swapping, edge hammering, and face lifting. (2009). In *Proc. International Meshing Roundtable*, Springer. https://doi.org/10.1007/978-3-642-04319-2_2

**Technique.** Three local repair operators applied where two mesh elements intersect: **edge swapping** reconnects a crossing pair to remove the intersection; **edge hammering** nudges vertices to eliminate a near-coincidence that caused the crossing; **face lifting** displaces a face off the intersecting plane by the minimum needed. The algorithm avoids traversing deep into highly tangled invalid regions (which carry many meaningless intersections) and instead repairs at the boundary of the tangle, keeping it efficient and robust. The intent is to remove self-intersections without changing the original shape beyond tolerance.

**Limitations.** Local operators can fail to resolve globally inconsistent tangles; the three ops are heuristics with no global guarantee. Resolution-bound, like all mesh methods.

**Kernel relevance.** A library of local untangling operators for Keel's mesh-domain repair. When exact retrimming is too fragile, drop to the tessellation and apply swap/hammer/lift, then refit if an exact result is required.

### The general principle for Keel

Detection of self-intersection in the exact B-rep is a restricted face-pairwise surface-surface intersection (skip topologically adjacent faces, since they legitimately meet). Resolution has three tiers: (1) **retrim** the offending region of the exact surface if the fold is local and well-conditioned; (2) **virtual-suppress** the bad region if it is tiny; (3) **mesh-rescue** the whole body if the tangle is global, letting generalized winding numbers (B.7) define inside/outside through the intersection. Self-intersecting solids are the strongest argument for the mesh-rescue floor.

---

## B.3 Sliver and short-edge remediation

The recurring tradeoff: removal simplifies topology and conditioning, but every removal moves the boundary, so it must be bounded by a shape-fidelity tolerance.

### Source: COMSOL / commercial defeaturing operators (collapse, delete sliver faces, short edges)

**Citation.** COMSOL CAD Import Module documentation: "Delete Sliver Faces", "Repair" (short edges/spikes/small faces), and "Using Virtual Operations to Simplify Your Geometry". https://www.comsol.com/blogs/using-virtual-operations-simplify-geometry/ and https://doc.comsol.com/

**Technique.** A representative production catalog of sliver/short-edge operators. **Short-edge collapse**: any edge shorter than a tolerance is collapsed, fusing its two vertices into one, and incident faces are healed to the new vertex. **Delete sliver faces**: a high-aspect-ratio small-area face is collapsed into one of its longer bounding edges (the sliver "zips" into the neighbor). **Repair tolerance** governs the threshold: faces and edges smaller than it are removed. Critically, COMSOL distinguishes **geometric defeaturing** (changes the B-rep, irreversible) from **virtual operations** (topological only, preserves surface curvature, reversible). The guidance is to prefer virtual operations when surface curvature matters and the change must be undoable.

**Limitations.** Collapsing a short edge shared by larger faces can pull those faces out of shape if the tolerance is loose; sliver removal can leave a tiny gap that re-triggers stitching. Aggressive collapse cascades (collapsing one short edge creates another).

**Kernel relevance.** Direct spec for Keel's `collapse_short_edge` and `remove_sliver_face` operators, each gated by a fidelity tolerance and a check that the post-collapse Hausdorff deviation stays under budget. Offer both a geometric mode (alters body) and a virtual mode (B.1).

### Source: Topological beautification of reverse-engineered models

**Citation.** Local topological beautification of reverse engineered models. (Cardiff/Langbein, Martin et al.). https://orca.cardiff.ac.uk/id/eprint/31747/1/topological_beautification.pdf

**Technique.** Reverse-engineered B-reps from scans carry spurious tiny faces and edges produced by the segmentation. Beautification applies local topological operations (merge near-coplanar faces, remove sliver faces, collapse short edges, regularize loops) chosen to restore the **intended** topology, using consistency of the surrounding geometry (e.g. two faces that are within tolerance of coplanar should be one). The operations are local and validity-preserving.

**Limitations.** Inferring intended topology is heuristic and can over-merge; needs good thresholds. Aimed at scan-derived models, where defects cluster.

**Kernel relevance.** The "beautify" pass for Keel's reverse-engineering and mesh-to-BRep paths: after fitting surfaces, run local topological cleanup to remove the segmentation litter before the body is certified.

### The fidelity contract

Every sliver/short-edge removal must answer: how far did the boundary move? Keel should compute, per removal, a bound on the introduced deviation (collapse a short edge -> deviation is at most its length; remove a sliver -> deviation is at most its width), accumulate it into the body's tolerance budget, and refuse the removal if it would push the body past a user shape-fidelity limit. Tolerance-driven decimation of topology, not unconditional removal.

---

## B.4 Degenerate surface and face handling

Poles and knife edges are not bugs to remove, they are legitimate geometry that breaks naive code. The kernel must cope, not "fix."

### Source: Degenerate NURBS poles/singularities in robust tessellation

**Citation.** Robust conforming NURBS tessellation for industrial applications based on a mesh generation approach. (2015). *Computer-Aided Design, 60*. https://doi.org/10.1016/j.cad.2015.01.002 (and the broader CAGD literature on pole handling).

**Technique.** A pole (singular point) is where a whole iso-parametric edge of a surface collapses to one 3D point: the apex of a cone, the two poles of a sphere, the apex of a degenerate Bezier patch. At a pole the surface normal is undefined (the first partial derivatives are parallel or zero), so any algorithm that divides by the normal length, or steps in (u,v) assuming a regular metric, fails. Robust tessellation handles this by detecting degenerate edges (zero-length iso-curve segments), collapsing the corresponding mesh row to the single pole vertex, and computing the normal at the pole from a limit (the cross product of the two non-degenerate tangents, or an averaged fan of adjacent normals) rather than from the degenerate partials. Quadrature/sampling near the pole is refined or switched to a singularity-aware scheme (e.g. Duffy-style) because uniform sampling clusters infinitely there.

**Limitations.** Limit-normal computation is itself fragile if the surface is also nearly flat near the pole; over-refinement near poles wastes elements. Self-overlapping parameterizations (where the surface folds in (u,v)) are harder than simple poles and may need re-parameterization.

**Kernel relevance.** Keel's surface evaluator must carry a **degeneracy map** per surface: which boundary edges are poles, where the normal must be taken as a limit, and where (u,v) stepping must avoid the singular row. Evaluation, tessellation, point-inversion, and offset all consult it. This is "cope" infrastructure: poles are never removed, they are flagged and handled.

### Knife edges, zero-width regions, self-overlapping parameterization

A **knife edge** (two boundaries meeting at near-zero dihedral) and a **zero-width region** (a face that pinches to zero width somewhere in its interior) are the face-level analog. Detection: the face's medial width drops below tolerance somewhere. Handling: if the zero-width region is at the boundary, treat it as a pole/degenerate vertex; if interior, the face is effectively two faces touching at a point and should be split, or the pinch suppressed via virtual topology. A **self-overlapping parameterization** (the surface covers part of 3D twice as (u,v) ranges) is a more severe defect: point-inversion becomes multi-valued. The robust response is to restrict the trimmed domain so only one sheet is live, or to reparameterize/refit a clean surface.

---

## B.5 Snapping, zippering, and merging under tolerance

Snapping is how gaps close and how stitching merges coincident entities. It is also the single richest source of new defects, because moving one vertex to close one gap can open or invert another.

### Source: Turk & Levoy, mesh zippering

**Citation.** Turk, G., & Levoy, M. (1994). Zippered polygon meshes from range images. *Proc. SIGGRAPH '94*. https://graphics.stanford.edu/papers/zipper/

**Technique.** The seminal zippering algorithm: overlapping scanned mesh patches are merged by **eroding** the redundant overlap region back to a clean boundary, then **clipping and re-triangulating** along the seam so the two meshes share a consistent border, then **consensus geometry** averages vertex positions in the overlap to reduce scanner noise. Zippering closes the seam between two surfaces by stitching their boundaries with new triangles rather than by snapping every vertex, which limits the topological disturbance to the seam.

**Limitations.** Sensitive to overlap quality; the erosion/clip step can produce slivers at the seam that then need cleanup. The follow-on literature (mesh-zippering robustness) notes that when boundary loops have very different shapes or arbitrary orientation, the zipper can produce self-intersections or non-manifold correspondences. This is the cascading hazard in concrete form.

**Kernel relevance.** Zippering is the right model for Keel's sheet-to-sheet sewing and for closing a gap between two trimmed faces: build the seam explicitly rather than snapping vertices globally. Adopt the consensus-averaging idea for tolerant edges (the merged edge sits between its two parents).

### Source: Snap rounding / geometry snapping pitfalls (NetTopologySuite, GIS snapping)

**Citation.** NetTopologySuite `GeometrySnapper` and the snap-rounding literature; the JTS/GEOS snapping-for-robustness approach. https://nettopologysuite.github.io/NetTopologySuite/

**Technique.** Snapping one geometry to another within a snap tolerance eliminates near-coincident vertices/edges and improves robustness of overlay (boolean) operations by removing the near-degenerate configurations that break exact predicates. The key engineering lesson: **too much snapping produces invalid topology**, so production snappers use heuristics to decide when a snap is safe, and deliberately omit some snaps rather than risk a collapse or inversion. Snap rounding rounds all vertices to a grid so that intersections become exact, trading a bounded position error for guaranteed topological consistency.

**Limitations.** Grid rounding introduces a bounded but nonzero shape error and can collapse very thin features; choosing which snaps to skip is itself heuristic.

**Kernel relevance.** This is the direct warning for Keel: snapping must be **conservative and validated**. After any snap/merge, re-check local validity (no inverted face, no new self-intersection) before committing; if the check fails, back off the snap or convert the gap to a tolerant entity instead (B.6). Prefer tolerant absorption over physical vertex movement whenever both are available.

### Robustness pitfalls and how to avoid them

The cascading pathology: vertex A snaps to B, which now coincides with C, forcing another snap, which inverts a sliver face, which the next pass tries to fix by another snap. Avoid it by: (1) computing **all** merge clusters first via a single transitive proximity grouping (union-find over a spatial hash), then performing each cluster merge once; (2) validating each merge locally and rejecting any that inverts orientation or creates an intersection; (3) preferring tolerant entities to physical motion; (4) never iterating snap passes unbounded. Stitching is a one-shot clustered merge, not a relaxation loop.

---

## B.6 Tolerant entity reasoning during operations

Tolerant modeling lets an operation proceed over imperfect geometry by attaching a per-entity imprecision that absorbs the error, instead of demanding exact coincidence.

### Source: ACIS tolerant modeling (tolerant edges and vertices)

**Citation.** Spatial Corp. ACIS Kernel documentation, Chapter 6: Tolerant Modeling. http://www-isl.ece.arizona.edu/ACIS-docs/ (Tolerant Edges, Tolerance Variables).

**Technique.** ACIS attaches a **geometric tolerance** to individual EDGE and VERTEX entities (via tolerant subclasses derived from EDGE/COEDGE/VERTEX). The mental model: edges are tubes and vertices are spheres, and the tolerance is the radius. Normal ("exact") edges must match within the global resolution `SPAresabs`; when two faces meet with a gap larger than `SPAresabs` but the model should still be one body, the edge is promoted to a **tolerant edge** whose tolerance equals the actual gap, so the two faces are considered to meet along that fat edge. Tolerant entities are created automatically during healing and booleans when exact geometry cannot be made coincident. Tolerances are **maintained by the system and updated after each operation**: the user can query but not set them. This lets flawed models from other CAD systems be operated on without forcing exact re-intersection everywhere.

**Limitations.** Tolerant entities are a controlled lie: downstream code must respect the per-entity tolerance everywhere (a query that uses the global tolerance near a fat edge gets the wrong answer). Widening tolerance to absorb a gap can swallow a genuine small feature. Tolerances ratchet upward through operations and can accumulate.

**Kernel relevance.** Keel should adopt per-entity tolerance from day one rather than a single global epsilon: `Edge` and `Vertex` carry a tolerance, exact entities default to the global resolution, tolerant ones carry the gap they absorb. Every predicate (point-on-edge, edge-edge coincidence, face-side) must read the local tolerance. This is the mechanism that lets a boolean run over a slightly-gappy import without first healing it geometrically.

### Source: Parasolid tolerant modeling and checking

**Citation.** Siemens Parasolid documentation: tolerant modeling overview and `PK_BODY_check` / face-repair, edge-repair APIs. http://www.q-solid.com/Parasolid_Docs/

**Technique.** Parasolid similarly gives each boundary element (vertex, edge, face) its own imprecision value, lowering required precision so edges that do not intersect within the nominal resolution are still treated as coincident. Healing widens tolerances and removes discontinuities/self-intersections to reduce `PK_BODY_check` faults. Repair APIs target faults found on specific faces or edges.

**Limitations.** As with ACIS: per-entity tolerance is a maintenance burden and can mask real defects; the standard is to keep tolerances as tight as possible and only widen where necessary.

**Kernel relevance.** Confirms the design: tolerant entities plus a checker that reports per-entity faults, plus localized repair APIs. Keel's repair operators should be face/edge-scoped, not whole-body, mirroring this.

### When widening tolerance is safe vs dangerous

Safe: the gap being absorbed is smaller than the smallest real feature you intend to keep, and the absorption is local (one edge, not the whole body). Dangerous: widening global tolerance to swallow a large gap, which then merges genuinely distinct features and corrupts downstream selection/queries. Rule for Keel: prefer the **narrowest local** tolerant entity that closes the specific defect; never raise the global resolution to paper over a local problem; record every tolerant entity so re-validation knows where precision was traded away.

---

## B.7 Mesh-based rescue and remediation: the robustness floor

When exact repair is too fragile (global self-intersection, leaky shells, garbage scans), convert to a mesh, repair the mesh with methods that **cannot fail to produce a manifold**, and optionally refit surfaces. This is the path that always yields a valid result, at a fidelity cost.

### Source: Attene, Campen & Kobbelt: the mesh-repair survey

**Citation.** Attene, M., Campen, M., & Kobbelt, L. (2013). Polygon mesh repairing: An application perspective. *ACM Computing Surveys, 45*(2), Article 15, 1-33. https://doi.org/10.1145/2431211.2431214

**Technique.** The definitive taxonomy and survey. It classifies mesh defects (singular/non-manifold edges and vertices, gaps and holes, degenerate elements, self-intersections, inconsistent orientation, noise) and divides repair algorithms along two axes. **Surface-oriented** methods operate directly on the input mesh, fixing defects locally and preserving most original triangles (high fidelity, but no global guarantee: they can leave residual defects). **Volumetric** methods resample the model onto an intermediate volume (a signed/unsigned distance field, an octree, or a space partition) and extract a new surface, which is **guaranteed clean** (manifold, watertight, intersection-free) but discards the original tessellation and loses sharp features and fidelity. A second axis is **local vs global**. The survey's central message: there is no universal repair; you choose surface-oriented when you must preserve the input and can accept residual risk, volumetric when you need a guaranteed valid output and can pay the fidelity tax.

**Limitations.** A survey, not an algorithm. Volumetric guarantees come at resolution-bound fidelity loss; surface-oriented fidelity comes at no guarantee.

**Kernel relevance.** This is the decision framework for Keel's rescue subsystem. Keep a surface-oriented repairer (MeshFix-style, below) for high-fidelity local fixes and a volumetric repairer (winding-number/tetrahedralization, below) as the absolute floor. Expose the axis to the caller: "preserve my geometry, best effort" vs "give me something guaranteed valid."

### Source: Attene, MeshFix (surface-oriented local repair)

**Citation.** Attene, M. (2010). A lightweight approach to repairing digitized polygon meshes. *The Visual Computer, 26*(11), 1393-1406. https://doi.org/10.1007/s00371-010-0416-3 (code: MeshFix).

**Technique.** Converts a raw digitized mesh into a single manifold, watertight, intersection-free, degeneracy-free triangle mesh by working **only locally** around defects rather than globally resampling. The pipeline: first remove all topological singularities and isolated/redundant components to get a single oriented manifold; then iteratively detect undesired configurations (self-intersections, degenerate triangles, holes), **remove a growing neighborhood** around each, and **patch the resulting gap** with a triangulation that is checked to be intersection-free. By growing the removed neighborhood until a clean patch can be inserted, it guarantees convergence to a valid mesh while keeping the vast majority of the original triangles untouched. High fidelity precisely because it does not touch good regions.

**Limitations.** Local patching can, in pathological cases, eat large neighborhoods (if defects are dense the "local" region grows huge). It produces triangles, not surfaces; refitting is a separate step. It assumes the input is mostly good.

**Kernel relevance.** The default mesh-rescue engine for Keel when the input is "mostly fine, a few bad spots": preserves shape, fixes the bad bits, guarantees a manifold. Port or wrap MeshFix's grow-and-patch loop. Use it before resorting to volumetric.

### Source: Jacobson, Kavan & Sorkine-Hornung: generalized winding numbers

**Citation.** Jacobson, A., Kavan, L., & Sorkine-Hornung, O. (2013). Robust inside-outside segmentation using generalized winding numbers. *ACM Transactions on Graphics (SIGGRAPH), 32*(4), Article 33. https://doi.org/10.1145/2461912.2461916

**Technique.** Defines the **generalized winding number** at a point as the sum of signed solid angles subtended by every triangle of an oriented mesh. For a clean closed mesh this is exactly 1 inside and 0 outside; the generalization is that it remains a smooth, meaningful real-valued function even when the mesh has **self-intersections, open boundaries, non-manifold pieces, and holes**. To segment inside from outside on such leaky input, they build a constrained Delaunay tessellation of space and run a graph-cut that labels each cell inside/outside using the winding number as the data term, snapping the fuzzy field to a crisp watertight result. This is the key that makes inside/outside well-defined on dirty geometry without first repairing it.

**Limitations.** Needs reasonably consistent triangle orientation as input (it is a signed quantity); wildly inconsistent normals degrade it. The graph-cut adds cost; the naive winding sum is O(points x triangles) though fast hierarchical evaluations exist.

**Kernel relevance.** This is the inside/outside oracle for Keel's robustness floor and for booleans on dirty meshes. When a body is leaky or self-intersecting and exact classification fails, classify points by generalized winding number. It underpins mesh-rescue (define the solid, then extract a clean boundary) and dirty-boolean fallback.

### Source: Hu et al., TetWild / fTetWild (volumetric tolerant tetrahedralization)

**Citation.** Hu, Y., Zhou, Q., Gao, X., Jacobson, A., Zorin, D., & Panozzo, D. (2018). Tetrahedral meshing in the wild. *ACM TOG, 37*(4). Hu, Y., Schneider, T., Wang, B., Zorin, D., & Panozzo, D. (2020). Fast tetrahedral meshing in the wild. *ACM TOG, 39*(4), Article 117. https://doi.org/10.1145/3386569.3392385 (code: fTetWild).

**Technique.** The strongest robustness floor in the literature: it tetrahedralizes a **triangle soup** (no manifold, watertight, or intersection-free assumption) into a valid tetrahedral mesh. It combines exact rational predicates with a user **geometric envelope tolerance**: the output surface is allowed to deviate from the input by up to that tolerance, which is exactly what lets it absorb gaps, self-intersections, and overlaps (it snaps them within the envelope). TetWild builds a background mesh, inserts the input triangles with exact arithmetic, and optimizes; fTetWild replaces rational triangle insertion with an incremental floating-point construction that keeps a valid float mesh at every stage (so the result is valid after float conversion, which TetWild's rational output is not guaranteed to be). The boundary of the tet mesh is a guaranteed-valid surface within the envelope of the dirty input.

**Limitations.** Output is a (tet) mesh, not an exact B-rep: refitting to NURBS is lossy and a separate problem. The envelope tolerance trades fidelity for robustness; sharp features within the envelope are rounded. Volumetric cost is higher than surface methods.

**Kernel relevance.** This is Keel's **last-resort path that always yields a valid manifold result**. Feed the dirty B-rep's tessellation (a soup) plus an envelope tolerance, get back a guaranteed-valid solid, take its boundary, and optionally refit canonical surfaces (file 24) where the geometry is recognizably planar/cylindrical/etc. The fidelity cost is explicit and bounded by the envelope, which makes it honest: the caller chooses the tolerance.

### The refit-back step

Mesh rescue yields a valid mesh; many downstream uses want an exact B-rep again. The path is mesh-to-BRep: segment the repaired mesh into regions, fit canonical or NURBS surfaces (file 23 metrology/fitting, file 24 canonical recovery), and rebuild topology. Fidelity is bounded by the rescue envelope plus the fit error. Keel should treat this as optional: the mesh result is itself a valid deliverable for analysis/printing, and refitting is only run when exact downstream operations are required.

---

## B.8 Orientation and normal repair

A shell can be geometrically perfect yet topologically inside-out or inconsistently oriented. This is cheap to detect and cheap to fix, and must be done before any inside/outside-dependent operation.

### Source: Consistent normal orientation (propagation + global sense)

**Citation.** Consistent normal orientation for polygonal meshes / point clouds (Borodin, Novotni, Klein and the broader literature); ray-casting visibility-based orientation. https://www.researchgate.net/publication/4079416

**Technique.** Two stages. **Relative consistency** via flood fill: pick a seed face, fix its orientation, then propagate across shared edges, flipping each neighbor so that the shared edge is traversed in opposite senses by the two coedges (the manifold consistency condition). A breadth-first walk over the face adjacency graph orients every face consistently relative to the seed, in one pass per connected shell. **Absolute sense** (is the consistent normal pointing out or in): cast rays from a face outward, or compute the signed volume of the shell (negative volume means inverted), or use the generalized winding number at a probe point; if the test says inward, flip the whole shell. For non-orientable or self-intersecting input the flood fill can hit a contradiction (a face reachable by two paths demanding opposite orientations), which is itself a diagnostic of a non-manifold/defective edge.

**Limitations.** Flood fill assumes manifold adjacency; it stalls or contradicts at non-manifold edges (those must be resolved first). Ray-casting the absolute sense is fragile on thin/leaky shells; winding-number or signed-volume is more robust there. Multiple shells (a solid with voids) need per-shell sense plus containment logic to know which shells are voids.

**Kernel relevance.** Keel's `repair_orientation`: (1) per shell, flood-fill consistent coedge senses; flag any contradiction edge as a non-manifold defect to fix first; (2) determine absolute sense by signed volume (cheap, exact for closed shells) or winding number (robust for leaky); (3) for multi-shell solids, orient the outer shell outward and void shells inward by containment. Run it as a validity gate before booleans and mass-properties.

---

## B.9 Validation and re-certification after repair/operation

Repair is meaningless without a contract that says the result is valid. Every repair or operation ends with re-validation, and the kernel must define what "valid" means.

### Source: Parasolid body checking (the re-validation contract)

**Citation.** Siemens Parasolid documentation, Checking chapter: `PK_BODY_check_b` and related, plus the PDQ/SASIG and VDA model-quality criteria referenced in file 13. http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.09.html

**Technique.** `PK_BODY_check` runs a battery of geometric and topological tests and returns a list of **specific faults** keyed to specific faces/edges/vertices, rather than a single pass/fail. The checks include: topological integrity (correct entity nesting, loops close, shells closed for solids), geometric validity (no self-intersecting faces, pcurves consistent with edges, faces not degenerate), gap/tolerance consistency (edges meet within tolerance, no gaps exceeding kernel tolerance), face-normal consistency (orientation coherent and outward), and manifoldness where required. Healing/repair then targets the reported faults, and the body is re-checked. The contract is: a body is **certified valid** only when the checker returns no faults at the current tolerances.

**Limitations.** Checking is expensive (pairwise face tests for self-intersection are the costly part); production kernels offer staged checks (cheap topology first, expensive geometry on demand). A body can pass at a loose tolerance and fail at a tight one, so the certificate is tolerance-relative.

**Kernel relevance.** Keel needs exactly this: a `check_body` that returns a structured fault list (reusing the Part A defect codes), used both as the diagnosis stage feeding repair and as the **re-validation gate** after every repair/operation. The re-validation contract for Keel: after repair or any operation that could introduce defects, run `check_body`; the result is certified valid (manifold or intended-non-manifold per file 02, watertight if solid, consistently oriented, tolerance-coherent) only if the fault list is empty at the declared tolerance. Carry the certificate (validity level + tolerance) on the body so downstream operations know what they are standing on. This dovetails with file 29's confidence levels.

---

## B.10 Making the core operations robust to these defects

Beyond repair, the operations themselves must degrade gracefully when fed dirty input.

- **Intersection (surface-surface, face-face)** must handle near-tangency without dropping or duplicating the intersection curve: snap to exact tangent within tolerance, use tolerant edges to absorb the residual, and use a marching scheme that does not stall at grazing contact. When two faces are within tolerance over an area (near-coincident), report a coincidence, not a curve.
- **Imprint** of one body's edges onto another's faces must imprint **tolerantly**: a near-miss edge that lies within tolerance of the face should imprint (creating a tolerant edge), and a T-junction created by imprinting should be recorded so it can be hidden by virtual topology for meshing. Imprint must not create slivers; if it would, snap the imprint to the existing vertex/edge.
- **Stitching** (covered in file 13 for the pipeline) must, at the operation level, cluster all coincident boundary edges first (union-find, B.5), merge each cluster once, create tolerant edges where the gap exceeds resolution, and validate locally per merge. Fed slivers, it should collapse them rather than sew around them.
- **Booleans** on near-tangent or slightly-gappy inputs should run over tolerant entities (B.6) rather than demanding exact coincidence, and fall to the mesh/winding-number classifier (B.7) only when exact classification is ambiguous. The boolean must never silently produce a topologically wrong result at a near-tangency: detect the ill-conditioning and either snap-to-exact or escalate to rescue.

The unifying principle: every core operation has a **tolerant mode** (proceed over small defects using per-entity tolerance) and an **escalation path** (when conditioning is too poor, drop to virtual suppression or mesh rescue) rather than a single brittle exact path.

---

# PART C: THE DEFECT-HANDLING TOOLKIT FOR KEEL

### The per-defect playbook

For each defect class in Part A, the default response, in priority order (cheapest/highest-fidelity first):

| Defect | First response | Fallback | Floor |
|---|---|---|---|
| Gap / crack at edge (A.1, A.2) | tolerant edge (B.6) | snap/zipper seam (B.5) | mesh rescue (B.7) |
| Self-intersecting face/solid (A.3) | retrim exact region (B.2) | virtual-suppress if tiny (B.1) | winding-number rescue (B.7) |
| Degenerate/sliver face (A.4) | collapse to edge/vertex (B.3) | virtual merge into neighbor (B.1) | mesh rescue |
| Short edge (A.5) | collapse, fuse vertices (B.3) | tolerant vertex (B.6) / virtual edge (B.1) | mesh rescue |
| Tiny feature (A.6) | virtual suppression (B.1) | discrete-proxy defeature (B.1 Quadros) | mesh rescue |
| Non-manifold junk (A.7) | delete stray / split (A.7) | flag local for operation | mesh rescue |
| Duplicate/coincident (A.8) | clustered merge (B.5) | tolerant entity (B.6) | mesh rescue |
| Inverted/inconsistent orientation (A.9) | flood-fill + signed volume (B.8) | winding-number sense (B.8) | mesh rescue |
| Invalid loop (A.10) | re-close / recompute pcurve (A.10) | rebuild loop from 3D edges | mesh rescue |
| T-junction / hanging edge (A.11) | imprint to conform (B.10) | hide with virtual edge (B.1) | mesh rescue |
| Near-tangency (A.12) | snap-to-exact in tolerance (B.6) | local tolerance widen (B.6) | mesh rescue |
| Free/dangling edge, open shell (A.13) | stitch to partner / cap face (file 13) | accept as sheet body | mesh rescue |

The column structure encodes the philosophy: try exact/local repair, then tolerate via virtual topology or tolerant entities, then fall to the mesh floor that cannot fail.

### Virtual topology as a first-class tool

The single highest-leverage decision in this file: build the **virtual topology overlay** as a core subsystem (B.1), not a meshing afterthought. Composite faces and virtual edges/vertices that reference the real geometry beneath, with merge/collapse/split operators, a forward/inverse map, and an auto-clustering policy (Inoue/Sheffer criteria: bounded curvature and distance deviation). It is the cheapest, safest, fully-reversible, zero-fidelity-loss response to slivers, short edges, tiny features, and T-junctions, and it should be tried before any geometric repair for the meshing and query paths. It is "tolerate by hiding," and for those paths it is strictly better than "repair by altering."

### Mesh rescue as the robustness floor

Beneath everything sits the path that always succeeds (B.7): tessellate the dirty body to a soup, repair with MeshFix (surface-oriented, high fidelity) for mostly-good input or fTetWild + generalized winding numbers (volumetric, guaranteed) for badly-broken input, classify inside/outside by generalized winding number even through self-intersections and leaks, extract a guaranteed-valid manifold within a chosen geometric envelope, and optionally refit canonical/NURBS surfaces. The fidelity cost is explicit and bounded by the envelope tolerance the caller sets. This floor is what lets Keel promise that **no input ever produces a hard failure**: worst case, you get a valid mesh-derived solid with a stated, bounded deviation. That promise is the project's core value proposition made concrete.

### The re-validation contract

Repair and operations are only trustworthy if every output is re-certified (B.9). After any repair or any operation that could introduce a defect, run `check_body`, which returns a structured fault list keyed to the Part A defect codes. A body is certified valid only when the list is empty at its declared tolerance: manifold (or intended-non-manifold per file 02), watertight if solid, consistently and outwardly oriented, and tolerance-coherent. The validity certificate (level plus tolerance plus list of tolerant entities where precision was traded) rides on the body, so every downstream operation knows the conditioning of its input and so file 29's confidence levels are grounded in an actual check rather than an assumption. Repair never silently lowers quality: every tolerance traded and every entity collapsed is recorded in the certificate.

### Tolerance discipline, summarized

Three rules that recur across every technique:

1. **Prefer the narrowest local tolerance** that closes a specific defect (a tolerant edge sized to one gap), never a global tolerance widened to paper over local problems.
2. **Bound and budget every shape change**: each collapse/snap/merge contributes a bounded boundary deviation; accumulate it and refuse changes that exceed the caller's fidelity budget.
3. **Validate after every change**: clustered merges, snaps, and collapses are validated locally (no inversion, no new self-intersection) before commit, and the whole body is re-certified at the end. Snapping is one-shot and clustered, never an unbounded relaxation loop.

---

# REFERENCES

1. Sheffer, A., Blacker, T., Clements, J., & Bercovier, M. (1997). Virtual topology operators for meshing. *International Journal of Computational Geometry & Applications*.
2. Sheffer, A. (2001). Model simplification for meshing using face clustering. *Computer-Aided Design, 33*(13), 925-934. https://doi.org/10.1016/S0010-4485(00)00116-0
3. Inoue, K., Itoh, T., Yamada, A., Furuhata, T., & Shimada, K. (2001). Face clustering of a large-scale CAD model for surface mesh generation. *Computer-Aided Design, 33*(3), 251-261. https://doi.org/10.1016/S0010-4485(00)00124-X
4. Quadros, W. R., & Owen, S. J. (2011/2012). Defeaturing CAD models using a geometry-based size field and facet-based reduction operators. *Engineering with Computers / Proc. 18th International Meshing Roundtable*. https://doi.org/10.1007/s00366-011-0252-8
5. Jung, W., Shin, H., & Choi, B. K. (2004). Self-intersection removal in triangular mesh offsetting. *Computer-Aided Design and Applications, 1*(1-4), 477-484. https://doi.org/10.1080/16864360.2004.10738290
6. Removing self-intersections of a triangular mesh by edge swapping, edge hammering, and face lifting. (2009). *Proc. International Meshing Roundtable*, Springer. https://doi.org/10.1007/978-3-642-04319-2_2
7. COMSOL CAD Import Module: Delete Sliver Faces, Repair, and virtual operations. https://www.comsol.com/blogs/using-virtual-operations-simplify-geometry/ ; https://doc.comsol.com/
8. Langbein, F. C., Marshall, A. D., & Martin, R. R. Local topological beautification of reverse engineered models. *Computer-Aided Design*. https://orca.cardiff.ac.uk/id/eprint/31747/
9. Robust conforming NURBS tessellation for industrial applications based on a mesh generation approach. (2015). *Computer-Aided Design, 60*. https://doi.org/10.1016/j.cad.2015.01.002
10. Turk, G., & Levoy, M. (1994). Zippered polygon meshes from range images. *Proc. SIGGRAPH '94*. https://graphics.stanford.edu/papers/zipper/
11. NetTopologySuite GeometrySnapper and snap-rounding for robust overlay. https://nettopologysuite.github.io/NetTopologySuite/
12. Spatial Corp. ACIS Kernel Documentation, Chapter 6: Tolerant Modeling (Tolerant Edges, Tolerance Variables). http://www-isl.ece.arizona.edu/ACIS-docs/
13. Siemens Parasolid Documentation: Tolerant Modeling overview; Checking chapter (PK_BODY_check), face/edge repair. http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.09.html
14. Attene, M., Campen, M., & Kobbelt, L. (2013). Polygon mesh repairing: An application perspective. *ACM Computing Surveys, 45*(2), Article 15. https://doi.org/10.1145/2431211.2431214
15. Attene, M. (2010). A lightweight approach to repairing digitized polygon meshes. *The Visual Computer, 26*(11), 1393-1406. https://doi.org/10.1007/s00371-010-0416-3 (MeshFix)
16. Jacobson, A., Kavan, L., & Sorkine-Hornung, O. (2013). Robust inside-outside segmentation using generalized winding numbers. *ACM TOG, 32*(4), Article 33. https://doi.org/10.1145/2461912.2461916
17. Hu, Y., Zhou, Q., Gao, X., Jacobson, A., Zorin, D., & Panozzo, D. (2018). Tetrahedral meshing in the wild. *ACM TOG, 37*(4). (TetWild)
18. Hu, Y., Schneider, T., Wang, B., Zorin, D., & Panozzo, D. (2020). Fast tetrahedral meshing in the wild. *ACM TOG, 39*(4), Article 117. https://doi.org/10.1145/3386569.3392385 (fTetWild)
19. Borodin, P., Novotni, M., & Klein, R. Consistent normal orientation for polygonal meshes. https://www.researchgate.net/publication/4079416
20. Gonzalez-Lluch, C., Company, P., Contero, M., Camba, J. D., & Plumed, R. (2017). A survey on 3D CAD model quality assurance and testing tools. *Computer-Aided Design, 83*, 64-79. https://doi.org/10.1016/j.cad.2016.10.003 (taxonomy basis; detailed in file 13)
21. ANSYS SpaceClaim / Discovery: Repair (gaps, missing faces, stitch, fix) documentation. https://help.spaceclaim.com/ ; https://innovationspace.ansys.com/
22. CADfix (ITI/EnginSoft): CAD repair, healing, defeaturing, simplification. https://www.enginsoft.com/solutions/cadfix.html
23. Elysium CADdoctor: PDQ verification, healing, CAE simplification, mid-surfacing. https://altair.com/caddoctor/
24. Barequet, G., & Sharir, M. (1995). Filling gaps in the boundary of a polyhedron. *Computer Aided Geometric Design, 12*(2), 207-229. (stitching foundation; detailed in file 13)

---

*Companion files: file 29 (graceful-degradation philosophy, heal-vs-tolerate policy, confidence levels); file 13 (import healing pipeline: stitch/simplify/geombuild, surface extension); file 02 (non-manifold and cellular topology); file 05 (tessellation/faceting and the offset context); file 23 (metrology/fitting); file 24 (canonical geometry recovery). This file is the per-defect taxonomy and per-defect handling catalog.*
