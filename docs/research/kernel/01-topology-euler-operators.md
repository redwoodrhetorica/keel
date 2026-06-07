# B-rep Topology Data Structures, Euler Operators, and Solid Modeling Theory

Research dossier for the Keel kernel (open-source B-rep solid modeling kernel in Rust, Parasolid-class ambition).

This file covers the topology layer: the data structure that stores the boundary of a solid, the Euler operators that mutate it while preserving validity, the Euler-Poincare invariant used as a runtime check, the formal representation-scheme theory (Requicha, Tilove, Shapiro) that tells us what "valid" and "complete" mean, the industrial topology hierarchies (Parasolid, OCCT, STEP, ACIS, BRL-CAD), the boundary-evaluation / merging literature that drives booleans, and the topology models chosen by modern open kernels (truck, Fornjot, CGAL, OpenMesh).

## Scope and how to read this

Every boolean and local operation in a B-rep kernel ultimately reduces to a sequence of Euler operators applied to a topological data structure. The design choices here (which data structure, manifold vs non-manifold, which entity hierarchy, which validity invariant) are foundational and very expensive to change later. The sources below are grouped thematically. Each entry gives a citation, detailed content (with explicit data-structure layouts and operator definitions where useful), limitations, and Keel relevance.

A note on terminology used throughout:
- V, E, F = counts of vertices, edges, faces.
- S = number of shells (connected boundary components; for a simple solid S = 1).
- G or H = genus (number of through-holes / handles).
- L = total face loops; R = number of "rings" (inner loops, also called holes-in-faces or peninsular boundaries). The two formula families (L-based and R-based) are equivalent; the conversion is L = F + R.
- A "fin" (Parasolid) = "coedge" (ACIS) = "halfedge" (academic) = "oriented edge use". The same concept under four names.

---

## Part 1: Topology data structures

### 1.1 Baumgart, winged-edge (the origin of B-rep + Euler operators)

**Citation.** Baumgart, B. G. (1972). *Winged edge polyhedron representation* (Technical Report STAN-CS-72-320 / AIM-179). Stanford University, Computer Science Department. Also Baumgart, B. G. (1975). A polyhedron representation for computer vision. In *Proceedings of the May 19-22, 1975, National Computer Conference and Exposition (AFIPS '75)* (pp. 589-596). ACM Press.

**Content.** Baumgart introduced both the first edge-centric B-rep data structure and the first use of the term "Euler operators", inside his "GEOMED" / "Euclid" modeler. Three record types:
- **Vertex record:** a name, point coordinates, and one reference to an incident edge.
- **Face record:** a name and one reference to a bounding edge.
- **Edge record:** the heart of the structure. It stores the two endpoint vertices (start, end), the two adjacent faces (left, right), and four "wing" pointers: the predecessor and successor edge when traversing the left face, and the predecessor and successor edge when traversing the right face. With start/end, left/right, and the four wings, an edge effectively names eight neighbouring entities, giving constant-time adjacency queries.

Traversal: to walk a face boundary, start from the face's edge and repeatedly follow the appropriate wing (left-successor if the face is the edge's left face, right-successor otherwise) until returning to the start. To walk the edges around a vertex (the vertex "umbrella"), follow wings while tracking which side the vertex is on. The traversal needs a conditional at each step (which face is "left" for this edge?), which is the structure's main ergonomic weakness; the half-edge structure removes that conditional.

Baumgart's primitives preserve Euler's relation V - E + F = 2 at every step, which is the seed idea of Euler operators.

**Limitations.** Strictly orientable 2-manifold. Cannot represent non-manifold conditions (an edge shared by more than two faces, a face dangling from an edge, a vertex where two cones touch). Booleans on solids routinely create transient non-manifold states, so a pure winged-edge kernel needs special handling or perturbation. The left/right conditional makes traversal code branchy.

**Kernel relevance.** Winged-edge is the historical baseline and the conceptual parent of everything below. Keel should understand it but almost certainly should not use it directly: the half-edge variant is strictly easier to traverse, and the radial-edge / use-based hierarchies are required for non-manifold robustness. Worth implementing once as a reference to validate against.

### 1.2 Muller and Preparata, the doubly-connected edge list (DCEL) / half-edge

**Citation.** Muller, D. E., & Preparata, F. P. (1978). Finding the intersection of two convex polyhedra. *Theoretical Computer Science, 7*(2), 217-236.

**Content.** Introduced the DCEL to represent planar subdivisions and convex-polyhedron intersection. The modern "half-edge" data structure is the DCEL with each undirected edge split into two oppositely directed half-edges. Each half-edge stores:
- `origin` (or `target`) vertex,
- `twin` / `opposite` (the other half-edge of the same geometric edge, pointing the other way),
- `next` (next half-edge around the same face, consistent orientation),
- `prev` (optional, derivable from next but usually cached),
- `incident_face` (the face to the half-edge's left).

A vertex stores one outgoing half-edge; a face stores one boundary half-edge. Crucially, all connectivity records are fixed size, so vertices, half-edges and faces are uniform structs (unlike face-vertex lists, where a face holds a variable-length vertex array).

Traversal becomes branch-free: face boundary = follow `next`; vertex one-ring = alternate `twin` then `next` (or `prev` then `twin`). No "which side is this edge on?" test.

**Limitations.** Native half-edge is a 2-manifold structure: an edge has exactly two half-edges, so an edge cannot be shared by three faces. Boundary edges need a sentinel (a "hole" half-edge with a null face) or an explicit border convention. Non-manifold vertices (two surface patches meeting at a single point) cannot be represented without extension.

**Kernel relevance.** The half-edge is the default modern choice for 2-manifold surface work and is what OpenMesh, CGAL Polyhedron_3, and Fornjot use. For Keel, half-edge is the right mental model for the common case, but the kernel must decide up front whether to (a) restrict to manifold and detect/repair non-manifold output, or (b) adopt a use-based / radial structure (Section 1.4) that subsumes half-edge.

### 1.3 Guibas and Stolfi, the quad-edge

**Citation.** Guibas, L., & Stolfi, J. (1985). Primitives for the manipulation of general subdivisions and the computation of Voronoi diagrams. *ACM Transactions on Graphics, 4*(2), 74-123.

**Content.** The quad-edge simultaneously represents a subdivision of an orientable manifold and its dual. The central object is an edge record holding four directed edges: the edge in both directions plus its dual edge in both directions. Two algebraic operators generate all topology: `Rot` (rotate to the dual / rotate by 90 degrees in the quad), and `Onext` (next edge counterclockwise about the origin). From these, derived operators (`Sym`, `Lnext`, `Oprev`, etc.) are defined algebraically rather than by pointer chasing. The single topological mutator is `Splice`, which both connects and disconnects edge rings depending on context, and is its own careful inverse. The construction is elegant because primal and dual share one structure (Delaunay and Voronoi at once).

**Limitations.** Restricted to 2-manifolds (it represents subdivisions of a surface, possibly non-orientable with the variant, but not solid non-manifold configurations). The dual machinery is overkill for a solid modeler that does not need Voronoi duals. Geometry attachment is awkward compared with half-edge.

**Kernel relevance.** Mostly relevant to Keel's meshing / triangulation subsystems (Delaunay, constrained triangulation for tessellation and for face triangulation in booleans) rather than to the B-rep topology proper. The `Splice`-as-its-own-inverse design is a useful lesson: a small set of invertible primitives is easier to test and to undo than a large operator zoo.

### 1.4 Weiler, the radial-edge (non-manifold) structure and the "use" concept

**Citation.** Weiler, K. J. (1986). *Topological structures for geometric modeling* (Doctoral dissertation). Rensselaer Polytechnic Institute. See also Weiler, K. (1988). The radial edge structure: A topological representation for non-manifold geometric boundary modeling. In M. J. Wozny, H. W. McLaughlin, & J. L. Encarnacao (Eds.), *Geometric Modeling for CAD Applications* (pp. 3-36). North-Holland.

**Content.** Weiler diagnosed exactly the winged-edge limitation: it can only pair two faces per edge, so it cannot represent the non-manifold conditions that booleans produce. The radial-edge structure links *all* faces sharing an edge in a radial cycle around that edge. The key abstraction is the *use*:
- a **vertex-use** is an application of a vertex by an edge,
- an **edge-use** is an application of an edge by a loop (one oriented traversal of the edge by one face side),
- a **loop-use** is an application of a loop by a face side,
- a **face-use** is one of the two sides of a face.

The data hierarchy is: model -> region -> shell -> face (with two face-uses) -> loop (loop-uses) -> edge (edge-uses) -> vertex (vertex-uses). The radial ordering around an edge stores the cyclic sequence of face-uses incident to it, which is what makes "an edge shared by 3+ faces" representable and orderable.

**Limitations.** Heavyweight: the use entities multiply the object count (an edge in a manifold context carries two edge-uses, two faces carry two face-uses each, etc.), increasing memory and pointer-chasing. Implementation complexity is high. For purely manifold models it is more machinery than necessary.

**Kernel relevance.** This is the most important data-structure decision for Keel. Parasolid (fin), ACIS (coedge), and BRL-CAD (NMG) are all variants of the use-based / radial idea. If Keel wants Parasolid-class robustness (booleans, local ops, sheet bodies, mixed-dimensional models), it should adopt a use-based topology rather than a bare half-edge. The "fin/coedge = oriented edge use" concept (Section 4) is the practical core to implement; full radial cycles are needed only where non-manifold edges are allowed.

### 1.5 OpenMesh (practical half-edge library)

**Citation.** Botsch, M., Steinberg, S., Bischoff, S., & Kobbelt, L. (2002). OpenMesh: A generic and efficient polygon mesh data structure. In *Proceedings of OpenSG Symposium 2002*. RWTH Aachen.

**Content.** A widely used C++ half-edge library. Layout: an Edge owns two Halfedges; a Halfedge stores a FaceHandle, a (target) VertexHandle, and next/prev halfedge handles; a Vertex stores one outgoing HalfedgeHandle; a Face stores one HalfedgeHandle. Because connectivity lives on the halfedges and is fixed-size, all four element types are constant-size records (contrast face-based meshes with variable-length face arrays). Entities are referenced by integer *handles* (indices) rather than raw pointers, enabling array storage, cache locality, and stable references across reallocation. Circulators (vertex-, face-, halfedge-circulators) encapsulate one-ring traversal. `prev` is derivable from `next` but stored by default for speed. Arbitrary per-element properties can be attached dynamically.

**Limitations.** 2-manifold only; non-manifold input must be split/repaired. It is a *mesh* library (faces are typically polygons with no attached parametric surface), not a B-rep kernel with curved geometry.

**Kernel relevance.** OpenMesh's handle-based storage (indices into typed arrays, not pointers) is the right pattern for a Rust kernel: it sidesteps the borrow-checker pain of cyclic pointer graphs, gives O(1) lookup, cache locality, and trivially serializable topology. Keel should store topology in arena/slotmap-style typed arrays keyed by typed indices, exactly as OpenMesh, truck, and Fornjot do.

### 1.6 CGAL HalfedgeDS and Polyhedron_3

**Citation.** Kettner, L. (1999). Using generic programming for designing a data structure for polyhedral surfaces. *Computational Geometry, 13*(1), 65-90. CGAL Project. *CGAL HalfedgeDS and Polyhedron_3 reference manuals.* https://doc.cgal.org/

**Content.** `HalfedgeDS` is a configurable half-edge container with three item types (vertex, halfedge, face). Required incidences: `opposite()` (paired halfedge, mandatory), `next()` (around a facet), and optionally `prev()`, `vertex()` (target), and `facet()`. Connectivity is selectable from minimal (only next + opposite, modeling an undirected graph) to maximal (all incidences plus data). Storage is list-based or vector-based (the latter pre-reserves capacity and uses indices). `Polyhedron_3` layers geometric points/normals plus high-level *Euler operations* on top of `HalfedgeDS` while guaranteeing the surface stays a valid combinatorial 2-manifold. Its Euler operations include: `split_facet` / `join_facet` (inverse pair, add/remove a diagonal edge inside a facet), `split_vertex` / `join_vertex` (inverse pair, the half-edge realization of make-edge-vertex and its kill), `split_loop` / `join_loop`, and border-edge operations `make_hole` / `fill_hole` (and `add_facet_to_border`, `add_vertex_and_facet_to_border`). The documented invariant `join_facet(split_facet(h,g)) == h` is exactly Euler-operator invertibility.

**Limitations.** 2-manifold orientable surfaces only. Polyhedron_3 is older; CGAL's `Surface_mesh` (an indexed half-edge) is the more modern container but offers the same topological model.

**Kernel relevance.** CGAL is the closest open, well-specified mapping from classic Euler operators (Section 2) onto a half-edge implementation. `split_facet`/`split_vertex` are literally MEF and MEV. Keel can borrow this naming and the invariant-pair testing discipline directly. CGAL also shows the generic-programming trick of making the connectivity configurable, but Keel can keep it simpler with one fixed, well-chosen layout.

---

## Part 2: Euler operators

### 2.1 Mantyla and Sulonen, GWB (the canonical Euler-operator modeler)

**Citation.** Mantyla, M., & Sulonen, R. (1982). GWB: A solid modeler with Euler operators. *IEEE Computer Graphics and Applications, 2*(7), 17-31.

**Content.** GWB (Geometric WorkBench), built in Helsinki, demonstrated that a small, general set of boundary-model manipulation operators suffices to construct a comprehensive solid modeler. The operators add or remove vertices, edges, faces, shells, holes (handles) and rings (inner loops) so that the Euler-Poincare relation holds at *every* intermediate step. The operators are partitioned into:
- **Skeleton operators** that create or destroy an elementary minimal body: `MVFS` (make vertex, face, shell: the seed of a body, one vertex + one face + one shell) and its inverse `KVFS`.
- **Local operators** that change connectivity without changing global topological type: `MEV` (make edge + vertex), `KEV`; `MEF` (make edge + face), `KEF`; `KEMR` (kill edge, make ring: removing an edge that splits a loop creates an inner ring), `MEKR` (make edge, kill ring: the inverse, joining a ring to the outer loop with a bridge edge).
- **Global / handle operators** that change genus or shell count: `KFMRH` (kill face, make ring, make hole/handle: punching a handle through the body, raising genus) and `MFKRH` (its inverse).

**Limitations.** GWB is strictly 2-manifold. Euler operators are purely *topological*: they enforce combinatorial validity but say nothing about geometric validity (no self-intersection check, no orientation-consistency of geometry). They are low-level; a usable kernel layers higher operations (sweeps, booleans) on top.

**Kernel relevance.** This is the operator API Keel's topology layer should expose internally. Booleans, sweeps, fillets, and local ops should *all* be expressed as sequences of these operators so that validity is preserved by construction. Implement them as invertible pairs to get undo for free.

### 2.2 Mantyla, "An Introduction to Solid Modeling" (the textbook formalization)

**Citation.** Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press.

**Content.** The definitive textbook treatment of GWB-style modeling. It develops the half-edge data structure, the full Euler-operator set, and the proof that the operators are *complete*: any valid 2-manifold B-rep can be constructed from the empty model by some finite sequence of Euler operators, and conversely any sequence keeps the model on the manifold of valid B-reps (soundness). It presents a frequently used "five operator" generating set (each operator and its inverse), commonly listed as MVFS, MEV, MEF, KEMR (the loop-splitting operator), and KFMRH (the handle operator), which together span all degrees of freedom of the Euler-Poincare relation (vertices, edges, faces, rings, shells, genus). The book also covers the half-edge realization, set operations via the boundary, and a worked modeler architecture.

**Limitations.** 2-manifold focus; non-manifold modeling is out of scope. Pre-dates the NURBS-centric and tolerance-driven concerns of modern kernels; the geometry side is simpler than Parasolid/ACIS.

**Kernel relevance.** This is the single best implementation reference for Keel's topology + Euler layer. The completeness/soundness result is the theoretical license to build *every* higher operation out of Euler operators. Keel should treat the five-operator generating set as the minimal trusted core and define everything else in terms of it.

### 2.3 Mantyla and Tamminen, localized set operations (Euler operators driving booleans)

**Citation.** Mantyla, M., & Tamminen, M. (1983). Localized set operations for solid modeling. *Computer Graphics (SIGGRAPH '83), 17*(3), 279-288.

**Content.** Shows how to evaluate boolean set operations on B-reps efficiently by localizing the intersection search to the region of overlap, using a 3D spatial directory (the EXCELL extendible cell structure). The actual stitching of the result boundary is carried out with Euler operators, so the boolean result is guaranteed manifold-valid by construction. This is the bridge from "Euler operators are nice primitives" to "Euler operators are how real booleans are implemented".

**Limitations.** The spatial-directory acceleration is dated relative to modern BVH / AABB-tree approaches; robustness under floating point is not the focus.

**Kernel relevance.** Confirms Keel's intended architecture: booleans = (1) find intersections (accelerated with a spatial index), (2) split edges/faces and stitch with Euler operators. The localization idea (only touch topology near the intersection) is also the foundation of *local* operations and of incremental/lazy rebuild.

### 2.4 Analysis Situs (a modern open OCCT-based Euler-operator toolkit)

**Citation.** Analysis Situs. *Euler operators.* https://analysissitus.org/features/features_euler-ops.html (open-source CAD prototyping platform built on OpenCASCADE).

**Content.** A contemporary, freely documented implementation that groups operators into a "make" group (MEV, MEF, ...) and a "kill" group (KEV, KEF, ...), built on OCCT's TopoDS. States explicitly that Euler operators "form a sufficient set of solid definition and manipulation operations" and uses the practical form of the Euler-Poincare invariant `v - e + f = 2(s - h) + r`.

**Limitations.** Documentation is partial (does not enumerate the full operator set in one place). Tied to OCCT's shape model.

**Kernel relevance.** Useful as a living, readable example of Euler operators implemented over an industrial B-rep (OCCT) rather than a toy half-edge, and a confirmation of the `v - e + f = 2(s - h) + r` invariant form that Keel should assert after each operator.

---

## Part 3: Euler-Poincare formula as a validity invariant

### 3.1 The generalized Euler-Poincare relation

**Citation.** Shene, C.-K. *The Euler-Poincare formula* (course notes, CS3621 Introduction to Computing with Geometry). Michigan Technological University. https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler.html . See also Zhang, H., & Feng, J. (2007). *Introduction to Solid Modeling* (course notes), Zhejiang University.

**Content.** The generalized relation for a solid with handles, voids, and faces-with-holes is stated as:

`V - E + F - (L - F) - 2(S - G) = 0`

equivalently `V - E + F = 2(S - G) + (L - F)`, and in the ring form `V - E + F = 2(S - G) + R` where R = L - F is the number of inner rings (holes in faces). Symbols: V vertices, E edges, F faces, L total loops (outer + inner of all faces), S shells (the solid itself counts as one; internal voids add more), G genus (through-holes / handles), R rings.

Worked checks from the source:
- Cube: V=8, E=12, F=6, G=0, S=1, L=6 -> 8 - 12 + 6 - (6 - 6) - 2(1 - 0) = 0. Valid.
- Solid with an internal chamber (a void shell): V=24, E=36, F=16, G=1, S=2, L=18 -> 24 - 36 + 16 - (18 - 16) - 2(2 - 1) = 0. Valid.

The formula is a *one-sided* test: a non-zero value proves invalidity, but a zero value does not prove validity (geometry could still self-intersect, orientation could be wrong, etc.).

**Limitations.** Necessary, not sufficient. Purely combinatorial: it cannot catch geometric defects (self-intersecting faces, inconsistent normals, gaps within tolerance).

**Kernel relevance.** Keel should compute the Euler characteristic incrementally (each Euler operator changes the counts in a known fixed way) and assert the invariant after every operation in debug/validation builds. It is the cheapest, most reliable structural sanity check available and catches operator-sequencing bugs immediately.

### 3.2 Sequin, generalized Euler-Poincare theorem

**Citation.** Sequin, C. H. *Generalized Euler-Poincare theorem.* University of California, Berkeley. https://people.eecs.berkeley.edu/~sequin/PAPERS/EulerRel.pdf

**Content.** A pedagogical derivation and generalization of the relation that carefully accounts for shells, genus per shell, and ring/loop bookkeeping, and relates the modeling-engineering form to the topological Euler characteristic chi = 2 - 2g for an orientable closed surface (per connected boundary component). It clarifies how each Euler operator perturbs the relation by a fixed signed amount, which is what makes incremental invariant maintenance possible.

**Limitations.** Theoretical/expository; not an implementation.

**Kernel relevance.** Gives the precise per-operator deltas Keel needs to maintain the invariant incrementally rather than recomputing from scratch. Good source for the validation module's correctness argument.

---

## Part 4: Industrial topology hierarchies

### 4.1 Parasolid model structure (the target reference)

**Citation.** Siemens Digital Industries Software. *Parasolid documentation: Model Structure* and *Overview of Parasolid.* (V12 and V35 reference manuals.) e.g. http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.04.html

**Content.** Parasolid's entity hierarchy, from coarse to fine:
- **Assembly:** a grouping of bodies and sub-assemblies (with transforms).
- **Body:** the fundamental modeling object; one or more connected components. Body types include solid, sheet, wire, and general (mixed/non-manifold).
- **Region:** an open connected subset of 3D space whose boundary is vertices, edges, and oriented faces. Every body has at least one region; exactly one region is infinite (the outside). A solid body's interior is the union of its "solid" regions; voids are also regions. Regions are what make Parasolid's point-membership and boolean semantics clean.
- **Shell:** a connected collection of oriented faces (each face used by the shell on one or both sides) and edges; bounds a region. A region can have multiple shells (an outer shell plus void shells).
- **Face:** a bounded subset of a surface, bounded by zero or more loops, with an attached surface and a sense.
- **Loop:** a connected component of a face's boundary (outer loop or inner ring).
- **Fin:** the *oriented use of an edge by a loop*. This is Parasolid's half-edge/coedge. Two fins per manifold edge, more for non-manifold edges.
- **Edge:** a bounded piece of a single curve, bounded by zero/one/two vertices.
- **Vertex:** a point in space.

Parasolid explicitly supports manifold and non-manifold bodies, sheet (open) bodies, wire bodies, and general bodies that mix dimensions; the region+shell+fin machinery is precisely what enables this.

**Limitations.** Closed-source; the documentation describes the model but not internal storage. The full generality (general bodies, non-manifold) is a large surface area to match.

**Kernel relevance.** This is the entity model Keel is explicitly aiming at. The recommended Keel hierarchy mirrors it: body -> region -> shell -> face -> loop -> fin -> edge -> vertex, with fin as the oriented edge-use carrying the next/previous-around-loop and the edge/loop back-references. Adopting region as a first-class entity (not just "inside vs outside") is what makes booleans and point classification principled.

### 4.2 OpenCASCADE TopoDS (open-source industrial B-rep)

**Citation.** OpenCASCADE Technology. *Modeling Data user guide* and TopoDS/TopAbs reference. https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html

**Content.** OCCT's shape hierarchy (TopAbs_ShapeEnum), coarse to fine: COMPOUND (any group), COMPSOLID (solids joined at faces), SOLID (region of space bounded by shells), SHELL (faces joined at edges; open or closed), FACE (part of a surface), WIRE (edges joined at vertices), EDGE (a restrained curve), VERTEX (a point).

The defining design idea is the **TShape / Shape split**:
- `TopoDS_TShape` is the shared, reference-counted underlying topology in its own coordinate system. It holds the list of child TShapes plus optional geometry.
- `TopoDS_Shape` is a lightweight value = a handle to a TShape *plus* an Orientation *plus* a Location (a TopLoc_Location coordinate transform). Shapes are passed by value and cheap to copy; assignment does not copy the TShape, it shares it.

Orientation enum: FORWARD, REVERSED, INTERNAL, EXTERNAL (interior is default region / complement / both / neither). Location: composed rigid transforms (TopLoc_Datum3D), cached to avoid recomputation. Geometry attaches via the BRep layer: curves to edges (an edge can carry a 3D curve plus per-face pcurves), surfaces to faces, points to vertices.

**Limitations.** OCCT's robustness and tolerance handling are weaker than Parasolid's; the API is large and idiosyncratic. The orientation/location-on-handle model is powerful but a frequent source of subtle bugs (forgetting to compose the location).

**Kernel relevance.** The TShape/Shape sharing model is directly applicable to Keel: store canonical topology once (the TShape analogue), and let lightweight references carry orientation + transform. This gives instancing for free (the same gear referenced 20 times at 20 locations stores topology once) and makes orientation reversal an O(1) reference flip rather than a data mutation. Strongly recommended pattern.

### 4.3 ACIS topology model (the other major commercial kernel)

**Citation.** Spatial Corp / Dassault Systemes. *ACIS Format Component Guide: Model Topology* (3D ACIS Modeler documentation). e.g. http://www-isl.ece.arizona.edu/ACIS-docs/PDF/FCG/06TOPO.PDF

**Content.** ACIS uses a closely parallel hierarchy: body -> lump (= Parasolid region/connected chunk) -> shell -> face -> loop -> **coedge** -> edge -> vertex. The **coedge** is ACIS's name for the oriented edge-use (= Parasolid fin = half-edge); coedges form the doubly-linked ring around each loop and pair up across an edge. ACIS also carries a separate geometry hierarchy (surface, curve, point) referenced by the topology, mirroring OCCT's geometry/topology separation.

**Limitations.** Closed source. The lump-vs-region terminology differs subtly from Parasolid's (lump is a connected solid chunk; Parasolid's region is a point set of space), which matters when porting concepts.

**Kernel relevance.** Confirms that the two dominant commercial kernels independently converged on the same skeleton (body/region(lump)/shell/face/loop/coedge(fin)/edge/vertex with a coedge/fin half-edge). That convergence is strong evidence Keel should adopt the same hierarchy rather than invent a new one.

### 4.4 BRL-CAD NMG (open-source non-manifold radial-edge)

**Citation.** Muuss, M. J., & Butler, L. A. (1991). Combinatorial solid geometry, boundary representations, and non-manifold geometry. In D. F. Rogers & R. A. Earnshaw (Eds.), *State of the Art in Computer Graphics: Visualization and Modeling.* Springer. BRL-CAD NMG topology API docs: https://brlcad.org/docs/api/d2/d9d/group__nmg__topology.html

**Content.** BRL-CAD's NMG (N-Manifold Geometry) subsystem is an open implementation of Weiler's radial-edge structure. Hierarchy: model -> nmgregion -> shell -> faceuse/face -> loopuse/loop -> edgeuse/edge -> vertexuse/vertex. The "use" entities are explicit and first-class: a face has two faceuses (its two sides); a loopuse carries orientation; an edgeuse references the edge in a particular loop traversal; a vertexuse references a vertex in a particular context. Non-manifold edges are handled by the radial ordering of edgeuses around the edge (the radial-edge cycle), so an edge shared by N faces stores all N face sides in cyclic order.

**Limitations.** NMG is notoriously heavyweight and intricate; many object types and back-pointers. BRL-CAD's primary representation is actually CSG with NMG used for boundary evaluation, so NMG is a means to an end there.

**Kernel relevance.** The one open, studyable, full non-manifold radial-edge implementation. If Keel commits to non-manifold support, BRL-CAD NMG is the reference for how radial edge cycles and use-entities are actually wired together. Even if Keel starts manifold-only, designing the fin/coedge as a "use" from day one leaves a clean upgrade path to radial.

### 4.5 STEP ISO 10303-42 topology (the interchange standardization)

**Citation.** ISO. (2021/2024). *ISO 10303-42: Industrial automation systems and integration -- Product data representation and exchange -- Part 42: Integrated generic resource: Geometric and topological representation.* See the topology_schema reference: https://ap238.org/SMRL_v8_final/data/resources/topology_schema/topology_schema.htm

**Content.** STEP's `topology_schema` standardizes the same hierarchy as an exchange format. Entities (increasing complexity), each a `topological_representation_item`:
- **vertex** (dim 0) and its geometric subtype **vertex_point** (vertex + a cartesian point).
- **edge** (bounded by edge_start, edge_end vertices); subtypes **edge_curve** (edge + a geometric curve + same_sense flag), **oriented_edge** (an edge_element + orientation boolean), subedge.
- **path** (sequence of connected oriented edges); subtypes open_path, oriented_path, and **edge_loop**.
- **loop**: subtypes **vertex_loop** (a single isolated vertex), **edge_loop** (a closed path), **poly_loop** (polygon from cartesian points).
- **face_bound** (a loop + orientation); subtype **face_outer_bound** (designates the outer boundary).
- **face** (one+ face_bounds); subtypes **face_surface** (face + geometric surface + same_sense), **oriented_face**, subface.
- **connected_face_set** (set of topologically connected faces); subtypes **open_shell** and **closed_shell**.
- **vertex_shell** (a shell that is a single vertex_loop) and **wire_shell** (loops only, no faces) for lower-dimensional bodies.

**Limitations.** A *data exchange* schema, not a runtime kernel structure: it has no Euler operators, no fast adjacency pointers, and no mutation operations. Loops are unordered sets of oriented edges; reconstructing traversal order is the importer's job.

**Kernel relevance.** STEP is what Keel must import and export, so Keel's internal topology should map cleanly onto these entities (note the close match to Parasolid/OCCT). The same_sense and orientation booleans on edge_curve/face_surface are exactly the orientation bits Keel needs on its topology-to-geometry references. Treat ISO 10303-42 as the canonical "what entities must round-trip" checklist.

---

## Part 5: Solid modeling theory (validity, completeness, uniqueness)

### 5.1 Requicha, representations for rigid solids (the foundational theory)

**Citation.** Requicha, A. A. G. (1980). Representations for rigid solids: Theory, methods, and systems. *ACM Computing Surveys, 12*(4), 437-464.

**Content.** The paper that put solid modeling on a rigorous footing. Key ideas:
- **r-sets (regular sets):** the mathematical model of a "solid". A set is regular if it equals the closure of its interior, `r(X) = cl(int(X))`. This excludes degenerate features (dangling faces/edges, isolated points, "hairs") that ordinary point sets allow. Solids are bounded, regular, semi-analytic r-sets.
- **Regularized boolean operators:** ordinary intersection of two solids can produce non-solid results (e.g. two cubes touching only on a face yield a 2D sheet). Regularized operators fix this: `A *intersect* B = cl(int(A intersect B))`, and similarly for union and difference. The result of a regularized boolean on r-sets is always an r-set. This is the precise reason kernels implement *regularized* booleans, not naive set operations.
- **Properties of a representation scheme** (the formal yardstick for any representation): a scheme is a relation between a *modeling space* of abstract solids and a *representation space* of data structures. The desirable formal properties are **domain** (which solids it can represent), **validity** (which data structures are valid representations), **completeness / unambiguousness** (a valid representation determines a unique solid: no ambiguity), **uniqueness** (a solid has only one representation: usually *not* achievable, hence non-unique schemes), plus the pragmatic properties **conciseness**, **ease of creation**, and **efficacy** (support for downstream algorithms).
- **Taxonomy of schemes:** pure primitive instancing, spatial occupancy enumeration (voxels), cell decomposition, constructive solid geometry (CSG), sweeping, and boundary representation (B-rep). Each is rated against the properties above; CSG is unambiguous and always valid but non-unique and not directly renderable; B-rep is unambiguous, directly usable, but validity is hard to guarantee.

**Limitations.** The framework is descriptive; it does not by itself give algorithms. "Validity is hard for B-rep" is precisely the gap Euler operators and the Euler-Poincare invariant fill.

**Kernel relevance.** This is the conceptual constitution for Keel. Three direct mandates: (1) Keel's solids are regularized r-sets, so all booleans are regularized; (2) Keel's B-rep must be a *complete/unambiguous* scheme, which means the topology + geometry together must unambiguously determine a point set (orientation and sense bits matter); (3) Keel should explicitly track which representation properties it guarantees (validity via Euler invariants + geometric checks; completeness by construction; uniqueness is deliberately not guaranteed).

### 5.2 Tilove, set membership classification and point membership

**Citation.** Tilove, R. B. (1980). Set membership classification: A unified approach to geometric intersection problems. *IEEE Transactions on Computers, C-29*(10), 874-883.

**Content.** Defines the **set membership classification (SMC)** function `M[X, S]` that partitions a candidate set X against a reference solid S into the three parts X-inside-S, X-outside-S, and X-on-the-boundary-of-S. Point membership classification (PMC) is the special case where X is a single point: classify it as in / out / on. Tilove shows that a large family of geometric problems (clipping, intersection detection, boolean evaluation) are instances of SMC, and that boundary evaluation for booleans is fundamentally repeated classification of the boundary of one operand against the other, combined according to the boolean. The hard part is the "on" / boundary case, which requires **neighborhood** information (what does the solid look like in an infinitesimal ball around the point) to decide how on-boundary pieces survive a union vs an intersection vs a difference.

**Limitations.** The neighborhood machinery is delicate near edges and vertices (multiple faces meeting). Numerical robustness of the on-boundary decision is the classic failure mode of booleans.

**Kernel relevance.** PMC is a core Keel primitive (used for booleans, for "is this point inside the body", for selection, for trimming). Keel needs a robust PMC built on the region structure (Parasolid-style regions make in/out trivial once you know which region a point is in) plus a careful neighborhood classifier for on-boundary cases during booleans. Budget significant robustness effort here.

### 5.3 Shapiro, "Solid Modeling" survey

**Citation.** Shapiro, V. (2002). Solid modeling. In G. Farin, J. Hoschek, & M.-S. Kim (Eds.), *Handbook of Computer Aided Geometric Design* (pp. 473-518). North-Holland.

**Content.** The authoritative modern survey. Consolidates and updates Requicha's representation-scheme theory, covers CSG vs B-rep vs cell/decomposition representations, the informational-completeness question, dual (CSG + B-rep) representations and how to maintain them consistently, point-set topology underpinnings, and the move toward unevaluated / parametric / feature-based and heterogeneous models. Discusses the validity problem for B-reps in depth and the role of constructive (operator-based) approaches in guaranteeing validity. Also frames non-manifold and mixed-dimensional modeling within the same theory.

**Limitations.** A survey: broad rather than implementation-deep. (For history-to-present context, complement with Shapiro's later "A note on solid modeling: history, state of the art, future", arXiv 2302.14373.)

**Kernel relevance.** The best single orientation document for Keel's whole design: it justifies the dual-representation strategy (keep a constructive/feature history *and* an evaluated B-rep), the validity-by-construction philosophy (build B-reps with operators that preserve validity, i.e. Euler operators), and situates non-manifold support in theory. Read first; it cross-references almost every other source here.

---

## Part 6: Boundary evaluation and CSG-to-B-rep merging

### 6.1 Requicha and Voelcker, boundary evaluation

**Citation.** Requicha, A. A. G., & Voelcker, H. B. (1985). Boolean operations in solid modeling: Boundary evaluation and merging algorithms. *Proceedings of the IEEE, 73*(1), 30-44.

**Content.** Defines **boundary evaluation**: computing the explicit B-rep boundary of a solid defined constructively (e.g. a CSG tree of booleans). The two-phase pattern: (1) **generate-and-test / boundary generation**, intersect the candidate boundary faces of the operands to produce candidate boundary pieces; (2) **merging / classification**, classify each candidate piece (face fragment, edge fragment) against the operands using set membership classification, keep the pieces that belong to the result per the boolean, and stitch them into a consistent B-rep. This paper crystallizes the "intersect, classify, merge" pipeline that essentially every B-rep boolean still follows.

**Limitations.** Robustness under floating point is the perennial problem (coincident faces, near-tangent intersections). The classical formulation assumes exact predicates.

**Kernel relevance.** This is the algorithmic blueprint for Keel's booleans and for any CSG-to-B-rep conversion (useful if Keel keeps a CSG/feature tree per Shapiro and evaluates it to a B-rep). The output stitching should be done with Euler operators (Section 2) so the evaluated boundary is valid by construction.

### 6.2 Benouamer, Michelucci, and Peroche, exact/lazy boundary evaluation

**Citation.** Benouamer, M. O., Michelucci, D., & Peroche, B. (1994). Error-free boundary evaluation based on a lazy rational arithmetic: A detailed implementation. *Computer-Aided Design, 26*(6), 403-416. (See also their "Bridging the gap between CSG and B-rep via a triple ray representation.")

**Content.** Attacks the robustness failure of boundary evaluation head-on with a **lazy exact arithmetic**: every numeric value carries both a fast floating-point interval and a symbolic/rational definition; an exact (rational) evaluation is performed *only when* a sign decision cannot be resolved from the interval. This postpones (and usually avoids) expensive exact computation while still guaranteeing correct boolean/geometric *decisions*. With error-free predicates, their intersection algorithm "always runs to completion, producing consistent solids from consistent operands", handling all geometric special cases uniformly via one general data structure for solid boundaries. Numerical error is handled at an algorithm-independent layer (the arithmetic), decoupling robustness from the geometric algorithms.

**Limitations.** Rational/lazy arithmetic has overhead and complexity; restricted historically to low-degree (polyhedral / low-degree algebraic) geometry. Exact arithmetic on full NURBS is far harder.

**Kernel relevance.** Directly relevant to Keel's robustness strategy. The lazy-evaluation pattern (float fast path, exact fallback only at indeterminate sign tests) is a proven way to get robust predicates, and the principle of isolating *all* numerical robustness into one arithmetic/predicate layer (rather than scattering epsilon hacks through geometry code) is exactly the discipline Keel should adopt. Compare with Shewchuk's adaptive predicates and CGAL's exact-predicates-inexact-constructions kernel for the predicate layer.

---

## Part 7: Modern Rust open kernels

### 7.1 truck (Rust CAD kernel)

**Citation.** RICOS Co., Ltd. *truck: A CAD kernel written in Rust.* https://github.com/ricosjp/truck (crates: truck-topology, truck-geometry, truck-modeling, truck-meshalgo, ...).

**Content.** truck splits topology and geometry into separate crates ("Ship of Theseus" modular philosophy: small replaceable crates). `truck-topology` defines the classic abstract entities Vertex, Edge, Wire, Face, Shell, Solid as generic containers parameterized over the geometry types they hold (a Vertex holds a point, an Edge holds a curve and two vertices, a Face holds a surface and boundary wires). Entities are reference-counted and carry stable IDs; orientation is handled by the inversion of edges/faces (a face has a sense, edges in a wire are oriented). It emphasizes memory safety: no segfaults from CPU-side processing, leveraging Rust's ownership and Cargo tooling. truck-modeling builds higher operations (extrude/sweep, booleans-in-progress) on the topology.

**Limitations.** Younger and less battle-tested than commercial kernels; boolean robustness and non-manifold support are limited. The model is closer to a classic manifold B-rep than to Parasolid's full region/fin generality.

**Kernel relevance.** The most directly comparable prior art for Keel: a Rust B-rep kernel with a clean topology/geometry crate split and generic topology entities. Lessons: the modular crate boundary (topology must not depend on concrete geometry) is good; the generic-over-geometry entity design is worth evaluating; but Keel likely wants a heavier use-based (fin/coedge) topology and explicit regions to reach Parasolid-class capability, which truck does not fully have.

### 7.2 Fornjot (Rust B-rep kernel)

**Citation.** Braun, H. *Fornjot: An early-stage B-rep CAD kernel written in Rust.* https://github.com/hannobraun/fornjot and https://www.fornjot.app/

**Content.** An explicitly B-rep, code-first CAD kernel for mechanical CAD (3D printing, machining, woodworking), defining models directly in Rust. Its object model is half-edge-based and organized as a hierarchy of object types (vertex, half-edge, cycle = loop, region, face, shell, sketch, solid), with objects stored centrally and referenced by handles (an object store / arena), so the topology graph is index/handle-based rather than raw-pointer-based, matching the OpenMesh/truck pattern. The project has gone through several iterations of its topological core and is candid that it is experimental and not yet production-ready.

**Limitations.** Early-stage; feature-incomplete; booleans and curved-surface robustness are works in progress. Manifold-oriented; non-manifold generality is not a goal.

**Kernel relevance.** Confirms the practical Rust patterns Keel should use: a central object/handle store (arena) for topology, half-edge connectivity, and a code-first kernel API. Fornjot's repeated rewrites of its topology core are a cautionary tale: get the topology model right early (use-based, region-aware) because it is the most disruptive thing to change later. Worth tracking as the closest open Rust effort to Keel's goals.

---

## Topology layer design guidance for Keel

Synthesizing the sources into concrete decisions:

1. **Entity hierarchy: copy the commercial consensus.** Parasolid (body/region/shell/face/loop/fin/edge/vertex) and ACIS (body/lump/shell/face/loop/coedge/edge/vertex) independently converged, and STEP and OCCT match. Keel should use: body -> region -> shell -> face -> loop -> fin -> edge -> vertex. Make **region** first-class (with one infinite region) so point membership and boolean semantics are principled (Requicha r-sets, Tilove PMC). Make **fin** (= coedge = half-edge = oriented edge-use) the workhorse for loop traversal.

2. **Data structure: use-based half-edge, with a non-manifold upgrade path.** Implement the fin as a Weiler-style *edge-use*. For the manifold common case this behaves exactly like a half-edge (two fins per edge, `next`/`prev` around the loop, twin across the edge). Design the edge-to-fin link as a *radial cycle* from day one so non-manifold edges (3+ faces) become representable later without a rewrite, per Weiler/BRL-CAD. Do not ship a bare winged-edge (branchy traversal, no non-manifold).

3. **Storage: handles into typed arenas, not pointers.** Follow OpenMesh/truck/Fornjot: store each entity type in its own arena (slotmap/generational-index), reference entities by typed indices. This dodges Rust's borrow-checker pain with cyclic graphs, gives cache locality, stable references across growth, trivial serialization, and cheap impact analysis. 

4. **Separate topology from geometry, with shared canonical topology.** Per OCCT's TShape/Shape split and truck's crate split: store canonical topology once; let lightweight references carry *orientation* + *transform* (location). This yields free instancing and O(1) orientation reversal. Geometry (curves on edges, pcurves per fin, surfaces on faces, sense flags) attaches by reference, matching STEP's same_sense/orientation booleans.

5. **Mutate only through Euler operators.** Expose the GWB/Mantyla operator set internally: MVFS/KVFS (seed/destroy a body), MEV/KEV, MEF/KEF, KEMR/MEKR (ring split/merge), KFMRH/MFKRH (handle/genus). Implement them as invertible pairs so undo is free (CGAL's `join(split(x)) == x` discipline). By Mantyla's completeness/soundness result, every higher operation (sweeps, booleans, fillets, local ops) can and should be expressed as Euler-operator sequences, guaranteeing topological validity by construction.

6. **Maintain the Euler-Poincare invariant incrementally.** Track V, E, F, L/R, S, G and assert `V - E + F = 2(S - G) + (L - F)` after every operator in validation builds (each operator perturbs the counts by a known fixed delta, per Sequin). It is a cheap necessary check that catches operator-sequencing bugs instantly. Remember it is necessary, not sufficient: pair it with geometric checks (orientation consistency, no self-intersection within tolerance, 2-manifold/edge-sharing checks).

7. **Booleans: intersect-classify-merge, stitched with Euler operators, on robust predicates.** Follow Requicha-Voelcker boundary evaluation and Mantyla-Tamminen localized set operations: localize intersection with a spatial index (BVH/AABB tree), classify candidate boundary pieces via Tilove SMC/PMC (with neighborhood handling for on-boundary cases), and stitch the result with Euler operators. Isolate *all* numerical robustness into one predicate/arithmetic layer (Benouamer-Michelucci-Peroche lazy-exact pattern, or Shewchuk adaptive predicates / CGAL EPIC kernel), never scatter epsilons through geometry code.

8. **Represent the model as a dual: constructive history + evaluated B-rep.** Per Shapiro, keep an unevaluated feature/CSG-style history that boundary-evaluates to the B-rep. This supports parametric edit/rebuild and gives a second, independently valid representation to cross-check against.

9. **Map cleanly to STEP and learn from prior Rust kernels.** Ensure every internal entity round-trips to ISO 10303-42 topology entities (import/export is non-negotiable for a real kernel). Track truck and Fornjot: borrow their handle-arena and crate-separation patterns, but go heavier (use-based topology, explicit regions, full Euler-operator core) to reach the Parasolid-class robustness they currently lack. Heed Fornjot's lesson that the topology core is the costliest thing to get wrong, so commit to the use-based, region-aware design before building higher layers.

---

## References

1. Baumgart, B. G. (1972). *Winged edge polyhedron representation* (STAN-CS-72-320 / AIM-179). Stanford University. https://www.cs.jhu.edu/~misha/Spring25/Readings/Baumgart75.pdf
2. Baumgart, B. G. (1975). A polyhedron representation for computer vision. *AFIPS '75*, 589-596. ACM.
3. Muller, D. E., & Preparata, F. P. (1978). Finding the intersection of two convex polyhedra. *Theoretical Computer Science, 7*(2), 217-236.
4. Guibas, L., & Stolfi, J. (1985). Primitives for the manipulation of general subdivisions and the computation of Voronoi diagrams. *ACM TOG, 4*(2), 74-123.
5. Weiler, K. J. (1986). *Topological structures for geometric modeling* (PhD dissertation). Rensselaer Polytechnic Institute.
6. Weiler, K. (1988). The radial edge structure: A topological representation for non-manifold geometric boundary modeling. In *Geometric Modeling for CAD Applications* (pp. 3-36). North-Holland.
7. Botsch, M., Steinberg, S., Bischoff, S., & Kobbelt, L. (2002). OpenMesh: A generic and efficient polygon mesh data structure. *OpenSG Symposium 2002.* https://www.graphics.rwth-aachen.de/media/papers/openmesh1.pdf
8. Kettner, L. (1999). Using generic programming for designing a data structure for polyhedral surfaces. *Computational Geometry, 13*(1), 65-90. CGAL HalfedgeDS / Polyhedron_3 reference: https://doc.cgal.org/
9. Mantyla, M., & Sulonen, R. (1982). GWB: A solid modeler with Euler operators. *IEEE CG&A, 2*(7), 17-31. https://www.researchgate.net/publication/3209518
10. Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press. https://archive.org/details/introductiontoso0000mant
11. Mantyla, M., & Tamminen, M. (1983). Localized set operations for solid modeling. *Computer Graphics (SIGGRAPH '83), 17*(3), 279-288.
12. Analysis Situs. *Euler operators.* https://analysissitus.org/features/features_euler-ops.html
13. Shene, C.-K. *The Euler-Poincare formula* (CS3621 notes). Michigan Tech. https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler.html
14. Zhang, H., & Feng, J. (2007). *Introduction to Solid Modeling* (course notes). Zhejiang University. http://www.cad.zju.edu.cn/home/zhx/GM/015/00-ism.pdf
15. Sequin, C. H. *Generalized Euler-Poincare theorem.* UC Berkeley. https://people.eecs.berkeley.edu/~sequin/PAPERS/EulerRel.pdf
16. Siemens. *Parasolid documentation: Model Structure / Overview.* http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.04.html
17. OpenCASCADE Technology. *Modeling Data user guide* (TopoDS / TShape / TopAbs). https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html
18. Spatial / Dassault Systemes. *ACIS Format Component Guide: Model Topology.* http://www-isl.ece.arizona.edu/ACIS-docs/PDF/FCG/06TOPO.PDF
19. Muuss, M. J., & Butler, L. A. (1991). Combinatorial solid geometry, boundary representations, and non-manifold geometry. In *State of the Art in Computer Graphics.* Springer. BRL-CAD NMG topology: https://brlcad.org/docs/api/d2/d9d/group__nmg__topology.html
20. ISO. (2021/2024). *ISO 10303-42: Geometric and topological representation.* topology_schema: https://ap238.org/SMRL_v8_final/data/resources/topology_schema/topology_schema.htm
21. Requicha, A. A. G. (1980). Representations for rigid solids: Theory, methods, and systems. *ACM Computing Surveys, 12*(4), 437-464. https://lvelho.impa.br/i3d14/modtec/p437-requicha.pdf
22. Tilove, R. B. (1980). Set membership classification: A unified approach to geometric intersection problems. *IEEE Transactions on Computers, C-29*(10), 874-883.
23. Shapiro, V. (2002). Solid modeling. In *Handbook of Computer Aided Geometric Design* (pp. 473-518). North-Holland. (See also arXiv:2302.14373.)
24. Requicha, A. A. G., & Voelcker, H. B. (1985). Boolean operations in solid modeling: Boundary evaluation and merging algorithms. *Proceedings of the IEEE, 73*(1), 30-44.
25. Benouamer, M. O., Michelucci, D., & Peroche, B. (1994). Error-free boundary evaluation based on a lazy rational arithmetic: A detailed implementation. *Computer-Aided Design, 26*(6), 403-416.
26. RICOS Co. *truck: A CAD kernel in Rust.* https://github.com/ricosjp/truck
27. Braun, H. *Fornjot: An early-stage B-rep CAD kernel in Rust.* https://github.com/hannobraun/fornjot ; https://www.fornjot.app/
