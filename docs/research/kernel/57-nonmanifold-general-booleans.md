# 57. Booleans on general / non-manifold bodies (Parasolid capability item 29)

## Title and scope

This is the implementation-grade literature dossier for **booleans on general and non-manifold bodies**: mixed-dimension and non-manifold boolean algebra, Parasolid capability item 29. It is the deep dossier of its batch because its central question gates whether item 29 is **one milestone or three**: can Keel deliver general-body booleans as a thin **cell-complex layer over the existing solid-solid plus sheet-solid pipeline**, by classifying every fragment against both operands and keeping cells per a **selection table generalized to dimension pairs**, or does mixed-dimension non-manifoldness force a **rebuild of the dossier-47 radial-splice stitcher**? This file answers that with a defensible verdict, a generalized dimension-pair-keyed selection table, and an honest assessment of the stitcher.

It **builds on and does not repeat**:

- **Dossier 02** for the cellular / radial-edge representation survey (Weiler RES, Lee-Lee partial entity structure, Rossignac-O'Connor SGC, Gursoz-Choi-Prinz NOODLES, Masuda complex-based operators, ACIS Cellular Topology, Parasolid general bodies and first-class regions). Keel already has a PES topology with first-class regions, free-edge sheets, wires, and acorns representable, edited through GWB Euler operators. This dossier consumes that representation and asks only about **boolean semantics** over it.
- **Dossier 47** for the assembly pipeline: the `EdgeId` radial-splice stitcher, the shell-closure (drop-prevention) invariant, identity-from-imprint, and the recommendation to retire the polygon-soup `build_result_solid` in favor of a single identity-preserving `stitch_by_import`.
- **Dossier 39** for on-on coincident / tangent face classification via the two-sided winding-number neighborhood test.
- **Dossier 51** for sheet-body PES validity, the radial-cycle-length-1 free-edge discriminator, double-sided faces, and the open-shell Euler bookkeeping.

What Keel already has, stated once so this dossier does not re-litigate it: winding-number-classified booleans on solids AND sheet-solid (imprint, classify, select, import-and-glue stitch with mass==mesh and shell-closure gates); the solid-solid and sheet-solid pipeline **already shares** imprint / classify / glue; multi-component and interior-ring imprint; identity-preserving assembly carrying `EdgeId` from imprint with a radial-splice and a closure invariant; on-on classification via the two-sided neighborhood test; blend surgery; certified evaluator-to-NURBS fit; canonical recovery; tolerant edges with per-entity bounds.

This is a **design recommendation** grounded in production-kernel and research practice, to be validated against Keel's region, boolean, sheet, and mass-property test suites. House style: **no em-dashes anywhere**, machine-checked.

---

## The load-bearing conclusion, stated once

Before the per-question detail, here is the verdict the whole dossier defends.

> **Item 29 is ONE milestone plus a follow-up ladder, not three, BECAUSE the dossier-47 stitcher does not need a rebuild. What item 29 needs is a CELL-COMPLEX SELECTION LAYER inserted between `classify` and `assemble`, plus three localized RELAXATIONS of validity assumptions the stitcher already almost satisfies.**

The defensible reason, in one paragraph. Keel's pipeline is already the right shape: imprint produces a complete local arrangement of fragments that share edges by `EdgeId`; classify labels each fragment; select keeps a subset; the radial-splice stitcher rebuilds the shell by grouping coedges per `EdgeId` and forming a **radial cycle** (Weiler) around each edge, asserting a closure invariant. Every one of those stages is **already dimension-agnostic in its core mechanism**. The radial cycle is the exact structure a non-manifold edge needs (three or more faces around one edge); dossier 47 explicitly built it as a radial cycle, not a manifold pair, precisely so this generalization would be free. What is **manifold-specific today** is not the stitcher's machinery but three **assumptions layered on top of it**: (1) that selection produces a 2-manifold closed shell, so the closure invariant demands exactly two coedges per edge; (2) that every kept fragment is a 2-cell (face) bounding a 3-cell (solid region), so wires and lamina have nowhere to go; (3) that the result is **regularized** (lower-dimensional and interior cells discarded). Item 29 is the work of replacing those three assumptions with a **dimension-aware selection table** and a **per-cell keep predicate** over the arrangement, which is additive, not a rewrite. The literature is unanimous that this is the correct factoring: SGC (subdivide / select / simplify), ACIS Cellular Topology (the ct-graph: keep vertices to keep cells), and the Pereira multi-region boolean (a face-removal table keyed on region count and dimension) are all **selection layers over a shared arrangement**, never bespoke stitchers per dimension pair. Keel is one such selection layer short of item 29, and it already owns the arrangement and the radial-splice that the layer feeds.

The one honest caveat: this holds **only because dossier 47's recommendation is adopted first**. The polygon-soup `build_result_solid` cannot represent a radial cycle of more than two faces and cannot carry a kept wire or lamina; on the soup path item 29 genuinely would be a rebuild. So the dependency is: **finish dossier 47 (single identity-preserving stitcher), then item 29 is a selection layer.** That sequencing is what collapses three milestones into one plus a ladder.

---

## Q1. What do union / intersection / difference MEAN when operands mix solids, sheets, wires, and lamina with radial-3+ edges?

The manifold solid-solid boolean has a clean point-set definition: the result is an r-set (regular closed set), and `A op B` is `closure(interior(A op_set B))` evaluated on the 3D point sets. Mixed dimension breaks the single-point-set picture because **a sheet is a 2D point set of measure zero in 3D** and a wire is 1D. The literature resolves this not by a single point-set formula but by a **cellular / selective complex**: subdivide all operands into a common arrangement of cells of every dimension (0-cells vertices, 1-cells edges, 2-cells faces, 3-cells regions), then **select which cells are active in the result** by a per-cell predicate, then **simplify** (merge adjacent same-classification cells). This is the Rossignac-O'Connor SGC algorithm and it is the semantic backbone for item 29.

### Rossignac and O'Connor, Selective Geometric Complexes (SGC)
- **Citation.** J. R. Rossignac, M. A. O'Connor, "SGC: A dimension-independent model for pointsets with internal structures and incomplete boundaries," in *Geometric Modeling for Product Engineering* (Wozny, Turner, Preiss, eds.), North-Holland, 1990. Companion theory: J. Rossignac, M. O'Connor, "Representing geometric structures in d dimensions: Topology and order," *Discrete & Computational Geometry*, 1991. https://link.springer.com/article/10.1007/BF02189330
- **Method / content.** An object is a finite set of mutually disjoint **cells** of any dimension 0 to n, connected by an **incidence graph** (which cell bounds which). Each cell carries a Boolean **selection label** (active / inactive): a cell is *in* the represented pointset iff its label is true. The boolean algorithm is three stages, and this is the exact template for item 29: **(1) subdivide** all operands into one common complex where every cell has uniform classification against every operand (this is imprint generalized to all dimensions); **(2) select** by evaluating the boolean expression's predicate on each cell's classification vector and setting its active bit; **(3) simplify**, which Rossignac's *Merge* operator does by combining a k-cell with two incident (k+1)-cells when they share the same active / inactive classification and the k-cell bounds nothing else, removing now-redundant internal boundaries. Activeness lets one structure represent open sets, closed sets, dangling lower-dimensional features, and internal partitions uniformly, which is precisely the mixed-dimension general body.
- **Limitations.** SGC is a unifying scaffold, not a tuned production B-rep; the incidence graph is large and the geometry per cell is carried separately. It does not by itself prescribe the numerics of the subdivide step (the hard part), and maintaining activeness consistency through edits requires discipline.
- **Kernel relevance.** **This is the meaning of union / intersection / difference at mixed dimension for Keel.** The result of a general-body boolean is the SGC obtained by subdividing both operands into a shared arrangement, setting each cell's active bit by the operation's per-cell predicate over the classification, and simplifying. Keel's imprint IS the subdivide step (already producing a shared-edge arrangement, dossier 47); classify IS the per-cell classification; the missing piece is a **dimension-aware select** that sets active bits on cells of all dimensions, not just faces of a closed solid, plus an optional **simplify** for the regularized case. The active / inactive bit is the single mechanism that makes regularized and non-regularized (Q4) the same code with a different keep predicate. **Verified** against the SGC primary description and Rossignac's topology handout summary of subdivide / select / Merge.

### Weiler radial edge: the boolean meaning of a radial-3+ edge
- **Citation.** K. Weiler, "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling," in *Geometric Modeling for CAD Applications*, North-Holland, 1988 (and the 1986 RPI thesis). Surveyed in dossier 02.
- **Method / content.** A non-manifold edge owns an **ordered radial cycle of edge-uses**, one per face-side meeting along that edge, sorted by dihedral angle around the edge axis. Two-manifold edge: radial cycle length 2. Free sheet edge: length 1 (dossier 51). Three faces meeting along one edge (the canonical non-manifold junction a union of two solids sharing a face wall produces): length 3, or length 4 / 6 when counting both sides. The boolean meaning of such an edge is local: each **angular sector between consecutive faces in the radial cycle is a piece of a region**, and the region's solid / void status is what classification assigns. A radial-3+ edge is exactly where a shared interface face meets the outer walls of two adjacent regions.
- **Limitations.** Storage-heavy (dossier 02); pointer / handle chasing for adjacency.
- **Kernel relevance.** Keel's PES already stores this radial cycle, and dossier-47's stitcher already **builds the result's radial cycles by sorting coedge groups by dihedral angle**. So the data structure and the assembly primitive for radial-3+ edges already exist. What is new for item 29 is only that **selection may keep more than two coedges per `EdgeId`** (an interior interface face contributes a third), so the stitcher's closure invariant must accept a complete radial cycle of any length, not insist on length 2. That is the relaxation, not a rebuild. **Verified.**

### Gursoz, Choi, Prinz NOODLES: non-regular booleans across dimensions are tractable
- **Citation.** E. L. Gursoz, Y. Choi, F. B. Prinz, "Vertex-based representation of non-manifold boundaries" (NOODLES), in *Geometric Modeling for Product Engineering*, North-Holland, 1990. Related: Y. Choi, "Non-regularized Boolean set operations on non-manifold b-rep objects," Carnegie Mellon, https://kilthub.cmu.edu/articles/journal_contribution/Non-regularlized_Boolean_set_operations_on_non-manifold_b-rep_objects/6469088/1 ; "Boolean set operations on non-manifold boundary representation objects," *Computer-Aided Design*, 1991, https://www.sciencedirect.com/science/article/abs/pii/001044859190097G
- **Method / content.** NOODLES models non-homogeneous entities of mixed dimension (vertices, linear edges, planar faces, volumetric regions) and performs **non-regular boolean operations between objects of different dimensionality**. The intersection algorithm proceeds **dimension by dimension** (vertex against vertex / edge / face, then edge, then face), building the merged non-manifold complex, in contrast to the manifold face-face-only intersection. Singular intersections are handled by a tolerance-region containment test. This is the existence proof that union / intersection / difference are well-defined and computable when operands differ in dimension, including the cases regularized solid booleans forbid (a face touching a solid, a wire piercing a sheet).
- **Limitations.** Vertex-centric structure makes edge-radial queries less direct than Keel's PES; NOODLES is a research system, influential conceptually rather than as shipping code.
- **Kernel relevance.** Confirms the three-bucket catalog below has a non-empty bucket (c) and that mixed-dimension booleans are not pathological: a dimension-ordered intersection plus per-cell classification yields a valid result. Keel's imprint already handles the 2D-2D and 2D-3D intersection; the new intersections item 29 adds are **1D-against-2D / 3D (wire imprinting)** and **0D incidence**, which NOODLES shows reduce to lower-dimensional containment tests. **Verified** as a tractability proof; the specific NOODLES data structure is **not** recommended for Keel (PES is the chosen runtime, dossier 02).

---

## Q2. Result classification when the winding number is UNDEFINED or HALF-INTEGER

The winding number `w_B(p)` of a body B about a point p is **undefined on B's own boundary**: the integral has a jump discontinuity there, and the on-surface value is conventionally the **average of the two side limits**, a half-integer (1/2 for a single closed orientable shell). This is exactly the on-on situation dossier 39 resolves for coincident faces. Item 29 makes it the *common* case, because at every shared interface face and every kept lamina, representative points of kept fragments lie on a boundary.

### The precise rule
> **Never classify a cell by evaluating the winding number AT a point on a boundary. Classify by the two-sided limit across the cell, and let the cell's own dimension and the operation's keep predicate decide.** For a 2-cell fragment F with natural normal n at representative point p, evaluate the classification vector at `p_plus = p + eps*n` and `p_minus = p - eps*n` (eps a small multiple of local tolerance, not crossing another feature). These two points are off the boundary, so `w_i` is integer and well-defined for every operand i. The pair `(class(p_minus), class(p_plus))` is the **transition** the fragment witnesses; the keep predicate (Q3 table) is a function of that transition pair, the fragment's dimension, and the operation. The half-integer on-surface value is never used as a classification; it is only a **flag that the two-sided test is required here**, which is dossier 39's resolution generalized from coincident faces to every kept interface.

### Jacobson generalized winding number and its half-integer behavior
- **Citation.** A. Jacobson, L. Kavan, O. Sorkine-Hornung, "Robust Inside-Outside Segmentation Using Generalized Winding Numbers," ACM TOG 32(4), 2013, https://igl.ethz.ch/projects/winding-number/ ; A. Jacobson, "Boolean Operations using Generalized Winding Numbers," arXiv:1601.07953, 2016.
- **Method / content.** The GWN is well-defined and smooth for arbitrary meshes (open, non-manifold, self-intersecting) away from the surface, and on the surface it averages across the jump, yielding a half-integer. For open / non-watertight input the field degrades smoothly to fractional values rather than misclassifying. Practical classification rounds the field to the nearest integer away from boundaries.
- **Limitations.** The smooth field is exact only in the integral sense; on the boundary the value is definitionally ambiguous and must be resolved by the two-sided test, not by rounding (rounding 1/2 is a coin flip).
- **Kernel relevance.** Confirms Keel's existing classifier (winding-number with a 0.25 band around 0.5) is the right primitive and that the **0.5 band is precisely the flag for "use the two-sided neighborhood test here."** Item 29 changes nothing in the classifier; it changes only how many fragments fall in the band (many more, because interface faces are common) and therefore how often the two-sided test runs. **Verified** against the GWN half-integer definition.

### Spainhour and Weiss, generalized winding numbers over trimmed NURBS
- **Citation.** J. Spainhour (CU Boulder), K. Weiss (LLNL), "Robust Containment Queries over Collections of Trimmed NURBS Surfaces via Generalized Winding Numbers," arXiv:2504.11435, 2025, https://arxiv.org/abs/2504.11435 (companion: rational parametric curves, arXiv:2403.17371).
- **Method / content.** Computes the GWN for **trimmed NURBS** B-reps by reformulating the surface integral via Stokes' theorem into a **boundary line integral** along the trim curves, evaluated by adaptive Gaussian quadrature. The method is "robust, and indeed indifferent, to watertightness": for a collection of disconnected or gapped surfaces the GWN sums signed solid angles and degrades smoothly to fractional values rather than failing. Points coincident with a surface get the half-integer average; classification rounds toward 0 (outside) or 1 (inside) away from the surface.
- **Limitations.** Curve / surface quadrature is the cost; the half-integer on-surface case still needs the two-sided resolution for a definite keep decision.
- **Kernel relevance.** This is the **curved-surface evaluator** for Keel's classify step when operands are trimmed-NURBS general bodies. Its watertightness-indifference is exactly what a non-manifold / open general body needs: a sheet operand is not watertight, yet the GWN classification of a solid operand against it remains well-defined. Recommend it as the analytic-then-NURBS classification rung for item 29's curved cases. **Verified.**

---

## Q3. How Parasolid general-body booleans actually behave at mixed dimensions, and the GENERALIZED SELECTION TABLE (centerpiece)

### Parasolid general bodies and regions
- **Citation.** Siemens, *Parasolid Functional Description*, ch. 8 "General Bodies" and ch. 10 "Boolean Operations" (q-solid.com mirror; mirror offline at fetch time, content reconstructed from dossier 02 and secondary summaries). Surveyed in dossier 02.
- **Method / content (claimed, reconstructed from secondary sources; primary mirror was unreachable).** With **generalized topology enabled**, the result of a boolean is a **single general body which may be disconnected, non-manifold, or of mixed dimension**; only **global** booleans may be attempted in that mode. Regions are first-class and partition all of space (solid or void; every body has an infinite void region). Imprinting can add vertices to edges, faces, and regions. The classical limitation: sheets and solids can be Booleaned, but the result of a fuse that would create a **non-manifold vertex** is restricted (some non-manifold-vertex configurations are declined). Regularization, when applied, classifies topological elements and removes internal or lower-dimensional structure; the non-regularized / generalized mode **retains** those as interior double-sided faces and dangling parts.
- **Limitations / labeling.** This is **claimed**, not verified from the primary doc this session (the q-solid mirror returned connection-refused). The region-first-class and general-body-as-superset facts are **verified** via dossier 02's primary read; the specific non-manifold-vertex restriction is **claimed** from secondary forum / summary sources and should be confirmed against the live Parasolid docs before being treated as a hard target.
- **Kernel relevance.** Parasolid is Keel's north star and confirms the target semantics: general-body boolean = one valid mixed-dimension body, regions partition space, interior interface faces survive in the non-regularized mode. It also licenses Keel to **decline** a small set of pathological non-manifold-vertex configurations (Q5, the DECLINE list) without being "wrong," since Parasolid itself restricts them.

### ACIS Cellular Topology: the ct-graph, the cleanest implementable model for item 29
- **Citation.** Spatial Corp. (Dassault), *ACIS Cellular Topology Component* reference (R10), ch. 1; CFACE / CELL / CSHELL / ATTRIB_CELL class refs (www-isl.ece.arizona.edu/ACIS-docs mirror). Patents embodying the mechanism: US 7,031,790 "Operator for sculpting solids with sheet bodies," US 7,366,581 "Replace face operator," US 6,906,711 "Graph-based method for multi-bodied sweep terminations," US 9,817,924 (non-regularized union with cellular topology). https://patents.google.com/patent/US7031790
- **Method / content.** CT is a **secondary representation** layered on ordinary ACIS topology via attributes, partitioning a model into solid cells (CELL3D) and sheet cells (CELL2D), with CFACE = a face seen from one cell (a face-use). A boolean is performed by building a **ct-graph** for the intersecting bodies in which **vertices represent cells and edges represent cell adjacency**; the boolean is then **deciding which ct-graph vertices to keep** ("if a vertex is kept, its associated cell is present in the resulting solid"). A **non-regularized (selective) union** keeps cells that a regularized union would discard, attaching the intersection-and-union cellular graph to the result so interior cells and interface faces persist. For two **sheet** bodies the ct-graph's cells "are not bounded regions of space as in the solid selective Boolean, but rather bounded subsets of the original sheets," and the patents describe a **web body** (the collection of mutually split faces) that is then simplified by culling.
- **Limitations.** CT is a layered add-on, so consistency between base topology and the cell layer must be maintained (CT invalidates and recomputes cells by watching the bulletin board after each API call). Dossier 02 flags this layering as a cautionary tale: Keel should make regions primary rather than an attribute layer (and Keel already did, dossier 02).
- **Kernel relevance.** **This is the single most decisive source for the gating question.** ACIS proves that a mature kernel implements mixed-dimension and non-regularized booleans as **a selection over a cell-adjacency graph (keep cells = keep ct-graph vertices)**, not as a bespoke per-dimension stitcher. Keel already has first-class regions (= CELL3D), the sheet case (= CELL2D), and CFACE-equivalent fins. Item 29 is Keel building the ct-graph analogue (cells and their adjacencies) from the imprinted arrangement and applying a keep predicate, which is exactly the cell-complex selection layer the verdict recommends. **Verified** from the CT chapter summary and corroborated by four embodying patents. Patent note below.

### Pereira, Arruda, Miranda, Lira, Martha: the dimension-and-region-keyed removal table
- **Citation.** A. M. B. Pereira, M. C. de Arruda, A. C. de O. Miranda, W. W. M. Lira, L. F. Martha, "Boolean operations on multi-region solids for mesh generation," *Engineering with Computers*, 27, 2011. https://link.springer.com/article/10.1007/s00366-011-0228-8 ; PDF https://www.academia.edu/14496557
- **Method / content.** A non-manifold boolean for solids with **multiple internal regions and degenerate (sheet / wire) portions**. Entities are classified into **AinB** (inside any region of B), **AoutB** (outside all regions of B), **INTERS** (on both), computed **hierarchically**: vertices by point location, edges inherit from vertices (interior point test if both endpoints INTERS), faces inherit from outer-loop vertices (interior point test if all INTERS). The keep / remove decision is a **table (their Figure 3) keyed on the number of regions the face bounds** (`numRegA`: 0 = dangling sheet, 1 = external face of one region, 2 = internal interface between two regions), the **outward normal orientation**, and the face's location relative to the other group (in / out / intersecting). Union keeps faces on the outer boundary; intersection keeps shared space; difference applies a dominance factor. A specialized boundary definition distinguishes closed interface shells (boundary entities, kept) from isolated loose faces (kept only when functionally relevant, the non-regularized choice).
- **Limitations.** Aimed at mesh-generation multi-region solids; the table is presented for the cases that paper needed, not as an exhaustive dimension-pair matrix; curved-primitive support is asserted but the worked cases are polyhedral.
- **Kernel relevance.** **This is the concrete prior art for Keel's generalized selection table.** It demonstrates that the manifold Requicha keep / drop table generalizes by **adding two key columns: the cell's dimension (via `numRegA`) and the region-adjacency count.** Keel's `select_faces` already emits `KeptFace { operand, face, reversed }` from a Requicha table; item 29 widens that table to the Pereira keying. **Verified.**

### The generalized selection table (the deliverable)

The manifold solid-solid table is a function `(transition_pair, op) -> keep / drop / reverse`. The generalized table adds two inputs: the **dimension pair** of the operands and the **cell's region-adjacency class**. Notation: operands have body kinds in {3 = solid, 2 = sheet, 1 = wire}; a fragment is classified by its **two-sided transition** `(s_minus, s_plus)` where each side is `in` / `out` / `same-cell` (sheet borders same void both sides, dossier 51); `numReg` is how many distinct solid regions a face bounds (0, 1, 2). `R` = regularized result, `N` = non-regularized result. "keep+" / "keep-" denote keep with outward sense as-is / reversed (the dossier-47 `reversed` flag, dossier 46 convention).

| Operand dim pair | Op | Fragment kind | Transition / locality | R (regularized) | N (non-regularized) |
|---|---|---|---|---|---|
| solid / solid (3/3) | union | face, numReg=1 | inside other -> drop; outside -> keep+ | keep+ if out | keep+ if out; interior coincident face becomes double-sided interior, kept |
| solid / solid (3/3) | intersection | face, numReg=1 | inside other -> keep+; outside -> drop | keep+ if in | same as R; plus retained on-shell coincident faces |
| solid / solid (3/3) | difference A-B | face A, numReg=1 | outside B -> keep+; inside B -> drop | keep+ if out(B) | keep+; B-shell-inside-A faces kept reversed as cavity wall |
| solid / solid (3/3) | difference A-B | face B, numReg=1 | inside A -> keep- (reversed); outside -> drop | keep- if in(A) | keep- if in(A) |
| solid / solid (3/3) | any | interface face, numReg=2 | borders two kept regions | drop (regularized: interior wall removed) | **keep, double-sided**: this is the non-regularized delta |
| sheet / solid (2/3) | union | sheet face | sheet portion outside solid | drop sheet-inside, keep+ sheet-outside (Keel has this) | keep both; sheet-inside-solid becomes interior membrane |
| sheet / solid (2/3) | intersection (trim sheet by solid) | sheet face | portion inside solid | keep sheet-inside (the trim result) | keep all, tagged |
| sheet / solid (2/3) | difference (cut solid by sheet, or imprint) | solid face + sheet | sheet imprints solid face | regularized: sheet vanishes unless splitting | keep sheet as interior partition wall (the key non-regular use) |
| sheet / sheet (2/2) | union | sheet faces | non-coincident | merge sheets, shared seam edge radial-cycle-2 | keep all; coincident sheet overlap -> one face or double-layer per N |
| sheet / sheet (2/2) | intersection | sheet faces | overlap region only | keep overlap sub-sheet (a 2D-in-2D boolean, dossier 39 sec 1.2) | keep overlap + tag |
| sheet / sheet (2/2) | difference | sheet faces | A minus B-overlap | keep A-not-overlap sub-sheet | keep with imprint seam retained |
| wire / solid (1/3) | union | wire edge | edge inside / outside solid | regularized: wire inside solid is absorbed / dropped | **keep wire** (inside as embedded 1-cell, outside as dangling) |
| wire / solid (1/3) | intersection | wire edge | edge portion inside solid | keep wire-inside (trim wire to solid) | keep wire-inside, tagged |
| wire / sheet (1/2) | any | wire edge | edge on / off sheet | regularized: drop unless on a kept boundary | keep wire (NOODLES dimension-by-dimension) |
| any / any | any | isolated vertex (0-cell) | acorn / touch point | drop (regularized removes isolated vertices) | keep if N requests all incidences |

Reading the table for the gating question: **every row is the SAME machinery** (classify the fragment by the two-sided transition, consult a keep predicate, emit keep / drop / reverse), differing only in the keep predicate's arguments. The R column is the **regularized** keep predicate (drop interior and lower-dimensional cells); the N column flips a few cells from drop to keep. **Nothing in the table requires a different stitcher per row.** The stitcher's only new obligation is to accept the kept lower-dimensional cells (wires, lamina, double-sided interface faces) into the radial-edge structure, which is the Q1 / Q5 relaxation, not a new algorithm. The table is the SGC select stage (Q1) made concrete with Pereira's dimension-and-region keying.

---

## Q4. Regularized versus non-regularized, and which item 29 implies

### Requicha regularization and the r-set
- **Citation.** A. A. G. Requicha, "Representations for rigid solids: theory, methods, and systems," *ACM Computing Surveys* 12(4), 1980; A. A. G. Requicha, H. B. Voelcker, "Boolean operations in solid modeling," Proc. IEEE 73(1), 1985 (dossier 47); R. B. Tilove, "Closure of Boolean operations on geometric entities," *CAD*, 1980, https://www.sciencedirect.com/science/article/abs/pii/0010448580900251
- **Method / content.** A set is **regular** iff it equals the closure of its interior. Manifold solids are **not closed** under ordinary set booleans (an intersection can leave a dangling face or antenna), so Requicha defines the **regularized** operation `A op* B = closure(interior(A op B))`, which **discards lower-dimensional dangling and interior features** and keeps the result an r-set. The **non-regularized** operation retains them: external faces that fall inside the result become **double-sided interior faces**, and dangling sheets / wires persist.
- **Limitations.** Regularization is defined for homogeneous-dimension r-sets; the whole point of a general body is to *not* regularize, so the r-set formalism is the thing item 29 must transcend, not obey.
- **Kernel relevance.** **Item 29 implies the NON-REGULARIZED result is available, but as an OPTION, with regularized as the default.** This is exactly Parasolid (generalized topology is opt-in; default booleans are regularized solids) and ACIS (regularized union by default, *selective* non-regularized union on request). The single-bit SGC activeness (Q1) makes this one code path: regularized = the keep predicate drops numReg=2 interior faces and all sub-solid-dimensional cells; non-regularized = the keep predicate retains them. **Verified.** The design consequence: Keel should expose a `regularize: bool` (or `BooleanOptions { keep_interior, keep_lower_dim }`) on the boolean API, defaulting to regularized, and route both through the same select-then-stitch with different keep predicates.

### What non-regularized retains, concretely (from the selective-boolean prior art)
- **Citation.** ACIS selective-boolean patents (US 7,031,790; US 7,366,581) and the CMU non-regularized boolean work (Choi / Gursoz, dossier-39 / Q1 lineage).
- **Method / content.** Non-regularized unite produces a **web body** (all faces mutually split), retaining external faces that end up interior as **double-sided interior faces**, then optionally simplifies by culling redundant extension faces. The retained interior faces are exactly the **numReg=2 interface walls** of the generalized table.
- **Kernel relevance.** Tells Keel precisely which cells the non-regularized keep predicate flips to keep: interior interface faces (double-sided, dossier 46/51), embedded sheets, embedded wires, touch vertices. All are representable in Keel's PES today (dossier 02). **Verified.**

---

## Q5. The staged implementation path, and the stitcher question answered

### Does the dossier-47 radial-splice stitcher generalize, or must it be rebuilt? (the gating question, answered in full)

**It generalizes. It is not rebuilt.** Here is the element-by-element argument, mapped to the stitcher's actual responsibilities (dossier 47, the recommended single `stitch_by_import` path, NOT the soup path).

1. **Coedge grouping by `EdgeId`** is dimension-agnostic already. A wire edge, a sheet free edge, and a solid face edge all carry an `EdgeId` from imprint; grouping coedges by `EdgeId` is unchanged. **No change.**
2. **Radial cycle construction** (sort the coedge group by dihedral angle around the edge axis) already handles length 1 (free edge, dossier 51), length 2 (manifold), and length 3+ (non-manifold). Dossier 47 built it as a cycle precisely so 3+ is free. **No change; the relaxation is only that the closure invariant must accept any complete cycle length, not assert exactly 2.** This is a one-line predicate change: "every coedge lands in exactly one consistent radial cycle" (already the dossier-47 wording) instead of "every edge has exactly two opposite coedges."
3. **Orientation carry** (`effective_sense = face.sense XOR reversed`, `n_out = effective_sense x natural`, dossier 46) is per-fragment and unaffected by dimension. Double-sided interface faces (numReg=2) get tagged DOUBLE_SIDED (dossier 51) and skip the single-material-side query; the stitcher already must handle double-sided faces for sheets. **Reuse of the dossier-51 path.**
4. **Region / cell partition** (partition solid / void by carried `n_out`) generalizes to **partition into all regions** by walking the radial cycles. Keel already has first-class regions (dossier 02); the new work is keeping **kept lower-dimensional cells** (wires, lamina) as cells not bounding a 3-region, which the PES represents natively. **Additive: cells of dimension < 3 are attached, not stitched into a shell.**
5. **Closure invariant / drop-prevention** (dossier 47) is unchanged in spirit: every kept fragment's coedges must find their radial partners or the build errors loudly. For a general body the invariant is restated per Weiler / dossier 51: **every edge has a complete consistent radial cycle of its kept coedges (length >= 1), and every kept cell is reachable.** Same assertion, relaxed arity.

The three relaxations, listed once: **(R1)** closure invariant accepts radial-cycle length 1 and >= 3, not only 2; **(R2)** selection / stitch accept kept cells of dimension 0, 1, 2 that do not bound a 3-region (wires, lamina, double-sided interface faces); **(R3)** the keep predicate is the dimension-and-region-keyed table (Q3), with a `regularize` switch. None touches the stitcher's core grouping / radial-splice algorithm. **This is why item 29 is one milestone (the selection layer + three relaxations) plus a follow-up ladder, not a rebuild.**

The **hard dependency**, stated plainly: this verdict is true **only if the soup `build_result_solid` is already retired** (dossier 47's recommendation). The soup model cannot hold a radial-3 cycle or a kept wire and would force a rebuild. So **sequence dossier 47's single-stitcher consolidation before item 29.** With that done, item 29 is layer + relaxations.

### Staged implementation path (the follow-up ladder)

A path from Keel's current solid-solid + sheet-solid pipeline to full cellular results, each rung with an exact oracle.

- **Rung 0 (prerequisite, dossier 47).** Single identity-preserving `stitch_by_import`; retire the soup. Oracle: existing boolean suite stays green; asymmetric-chamfer repro passes (dossier 47).
- **Rung 1 (FIRST MILESTONE, the narrowest honest slice).** **Non-regularized solid-solid union that retains the shared interface wall as a double-sided interior face**, producing a two-region (cellular) solid. This reuses 100% of the existing solid-solid pipeline; the only new code is (a) the `regularize: bool` option, (b) the keep predicate retaining numReg=2 faces, (c) relaxation R1 (accept the radial-3 cycle where the interface wall meets the outer shells). **Exact oracle:** fuse two unit cubes sharing a common face; regularized result = one 1x2x1 box (mass 2, the existing answer, must not regress); non-regularized result = the same outer shell PLUS the interior wall, partitioned into **two solid regions** each of mass 1, summing to 2, with the interior face present and double-sided. mass == mesh == 2, region_count == 2, interior_face_count == 1. This is fully analytic, combinatorial, and bucket-(a) exact.
- **Rung 2.** **Sheet-as-knife: non-regularized difference that imprints a sheet into a solid as an interior partition wall** (split one region into two). Reuses sheet-solid imprint (Keel has it). Oracle: a planar sheet spanning a cube's cross-section splits it into two regions of known volume; sum and interface area exact.
- **Rung 3.** **Sheet-sheet booleans** (union / intersection / difference of two sheets), solved as a 2D arrangement in the shared / overlapping parameter domain (dossier 39 sec 1.2 for coincident, ordinary SSI for crossing). Oracle: two overlapping planar rectangles; result area and seam edges exact.
- **Rung 4.** **Wire imprinting** (wire-against-solid / sheet), the NOODLES dimension-by-dimension lowest rung: classify wire edges by point / segment containment, keep per table. Oracle: a polyline crossing a cube; kept-inside length exact.
- **Rung 5.** **Full cellular general body**: arbitrary mixed-dimension operands, the SGC subdivide / select / simplify in full, with the `regularize` switch. Oracle: the three-bucket billion-boolean oracle (MEMORY: PASS strict mass==mesh==ref / DECLINE / WRONG), extended to count regions and interface faces.

### Configurations to DECLINE forever (with justification)
- **Booleans that would create an isolated non-manifold vertex with no incident kept cell of dimension >= 1** (a bare touch point that bounds nothing). Parasolid restricts non-manifold-vertex results (Q3, claimed); the cell carries no metric and no oracle can distinguish kept from dropped, so it is pure ambiguity. **DECLINE** (drop the 0-cell; this is the regularized behavior even in non-regularized mode).
- **Non-orientable result sheets requested with a material side** (Mobius-band thicken): dossier 51 accept-and-tags the sheet but the operation needing an orientation must DECLINE. Carry the tag, fail the dependent op, not the boolean.
- **Coincident-overlap booleans on uncertified curved coincidence** where the shared-carrier recognition (dossier 39 sec 5) returns "unknown": DECLINE rather than guess, per the three-bucket DECLINE bucket (MEMORY).

---

## Cross-cutting three-bucket catalog (required deliverable)

Which mixed-dimension boolean configurations admit (a) exact analytic / combinatorial results, (b) a procedural-evaluator formulation, (c) genuinely require full general machinery.

**(a) EXACT ANALYTIC / COMBINATORIAL.** These need no marching and no general arrangement; the answer is a combinatorial selection over cells whose geometry is exact.
- Non-regularized solid-solid union / intersection / difference where all seams are **planar or shared-analytic** (the interface wall is a face of a known plane / cylinder / sphere): the cell partition and interface faces are combinatorial once imprint produces the seams. **Rung 1, the first milestone, is here.**
- Sheet-against-solid where the sheet lies on a solid face's carrier (coincident): the imprint is a 2D arrangement in the shared parameter domain (dossier 39 sec 1.2), exact.
- Wire-against-solid / sheet containment classification by exact point / segment-in-region tests when carriers are analytic.
- The regularized-vs-non-regularized *choice* itself: a single active-bit flip per cell (SGC), purely combinatorial.

**(b) PROCEDURAL-EVALUATOR.** These need a certified evaluator (winding number, containment) but not a full general arrangement rebuild.
- Classification of any fragment against a trimmed-NURBS general body: the Spainhour-Weiss boundary-integral GWN evaluator (Q2), watertightness-indifferent, gives the per-cell label as a procedural query.
- The two-sided neighborhood test on interface / coincident faces (dossier 39): a procedural perturb-and-evaluate, used wherever the 0.5 band fires.
- Sheet-sheet and wire booleans on curved carriers where the overlap region is found by an evaluator (SSI marching + containment) rather than closed form.

**(c) GENUINELY REQUIRES FULL GENERAL MACHINERY.** These need the complete SGC subdivide / select / simplify over a non-manifold arrangement with cells of all dimensions, because the result is irreducibly mixed-dimension and no analytic shortcut or single evaluator closes it.
- Arbitrary general-body op general-body with simultaneous solids, sheets, and wires sharing topology, producing a result with kept cells of every dimension and radial-3+ edges throughout (Rung 5).
- Non-regularized results where simplification (Rossignac Merge) must decide which interior boundaries collapse, requiring the full incidence graph.
- Multi-region (3+ region) solids where interface faces form a non-trivial 2-complex (CFD zone bodies, dossier 02): the ct-graph / SGC incidence structure is load-bearing.

The catalog's payoff for the verdict: buckets (a) and (b) cover Rungs 1 to 4, and they reuse Keel's existing imprint / classify / stitch with the selection layer and relaxations. Only bucket (c) (Rung 5) exercises the full SGC machinery, and even there the machinery is the **selection layer plus simplify**, still over the same stitcher. So the deep, expensive case is the *last* rung, and the first milestone (Rung 1) is squarely in bucket (a), exact and cheap.

---

## Patent awareness

- **SGC, NOODLES, Masuda complex-based operators, Weiler RES (1986-1993)** are old enough that any patents have **expired**; they are safe prior art and are the foundation to cite and build on. Prefer them.
- **ACIS Cellular Topology ct-graph and selective (non-regularized) booleans** are embodied in Spatial / Dassault patents filed in the **early-to-mid 2000s** (US 6,906,711 priority ~2000; US 7,031,790 and US 7,366,581 priority ~2000-2003; US 9,817,924 later). US utility patents expire 20 years from the earliest non-provisional priority, so the **core ct-graph / selective-union patents from ~2000-2003 are expired or expiring now (2020-2023)**. **Verify each patent's exact priority and expiry before relying on a specific claim.** Even where live, Keel's design reads on the **SGC subdivide / select / simplify** prior art (1990) and Pereira's region-keyed table (2011, a journal paper, not a patent), which **predate and are independent of** the ACIS patents. Claim-element design-around: the ACIS claims recite a *ct-graph as a secondary attribute layer* and *keeping ct-graph vertices*; Keel's regions are **first-class (primary), not an attribute layer** (dossier 02), and Keel selects cells via the **per-fragment two-sided transition table**, not a separately-materialized ct-graph vertex-keep. Building the selection over Keel's existing primary PES regions, citing SGC / Pereira as the method, is the clean design-around and is independently justified by dossier 02's "regions primary" decision.
- **Mesh-arrangement labeling (Zhou 2016, Cherchi 2020, EMBER 2022)** are academic with open implementations; the *idea* of per-cell winding-vector labeling is prior art (dossier 09 / 47). No blocking patent on the labeling concept; avoid copying any specific patented exact-predicate construction verbatim, which Keel does not need anyway (curved carriers, dossier 47).
- **GWN over trimmed NURBS (Spainhour-Weiss 2025)** is recent academic work (LLNL); no patent flagged, but being 2025 it is worth a patent check before shipping a verbatim implementation. The underlying GWN (Jacobson 2013) is clear prior art.

Net: **build on the expired 1990-1993 SGC / NOODLES / Weiler / Masuda foundation and the 2011 Pereira table (no patent), keep regions primary (design-around of the ACIS attribute-layer claims), and the path is clear.**

---

## Synthesis: ADOPT / AVOID / INVESTIGATE

**ADOPT.**
- The **SGC subdivide / select / simplify** semantics (Rossignac-O'Connor) as the definition of mixed-dimension union / intersection / difference, with the **active / inactive cell bit** as the single mechanism unifying regularized and non-regularized results.
- A **cell-complex SELECTION LAYER** between `classify` and `assemble`, applying the **dimension-and-region-keyed selection table** (Q3, after Pereira et al. 2011 and ACIS ct-graph), with a `regularize: bool` option defaulting to regularized.
- The **dossier-47 radial-splice stitcher, generalized by three relaxations** (R1 radial-cycle arity, R2 kept sub-3D cells, R3 dimension-keyed keep predicate). Do NOT rebuild it.
- The **two-sided neighborhood test** (dossier 39) wherever the 0.5 band fires, and the **Spainhour-Weiss boundary-integral GWN** as the curved / trimmed-NURBS classification evaluator (watertightness-indifferent, ideal for open general bodies).

**AVOID.**
- Building item 29 on the polygon-soup `build_result_solid`. It cannot hold a radial-3 cycle or a kept wire / lamina; on the soup path item 29 genuinely IS a rebuild. Retire the soup first (dossier 47).
- A separate per-dimension-pair stitcher. The literature (SGC, ACIS, Pereira) is unanimous that the stitcher is shared and only the **selection** varies by dimension. A per-pair stitcher is the rebuild this dossier shows is unnecessary.
- Copying the ACIS ct-graph as a **secondary attribute layer**. Keel's regions are primary (dossier 02); selecting over primary regions is both simpler and the patent design-around.
- Classifying any cell by the **on-surface half-integer** value. Always use the two-sided transition.

**INVESTIGATE.**
- The exact Parasolid non-manifold-vertex restriction (Q3 is **claimed**, mirror was offline): confirm against live Parasolid docs which configurations Parasolid declines, to align Keel's DECLINE list with the north star.
- Rossignac's **Merge / simplification** operator for the non-regularized-to-regularized collapse: the precise predicate for when an interior boundary is removed (needed only at Rung 5 / bucket c).
- The cost / accuracy of the Spainhour-Weiss boundary-integral GWN at Keel's tolerances for curved general-body classification, versus Keel's existing analytic classifier.

**RECOMMENDED FIRST MILESTONE (narrowest honest slice with an exact oracle).** **Non-regularized solid-solid union retaining the shared interface wall, producing a two-region cellular solid.** Reuses the entire existing solid-solid pipeline; new code is the `regularize` option, the keep-numReg=2 predicate, and relaxation R1. **Oracle:** fuse two unit cubes sharing a face. Regularized = one box, mass 2 (no regression). Non-regularized = same shell + interior double-sided wall, **two solid regions of mass 1 each summing to 2**, interface_face_count == 1, mass == mesh == 2. Fully analytic and combinatorial (bucket a).

**FOLLOW-UP LADDER.** Rung 2 sheet-as-interior-partition; Rung 3 sheet-sheet; Rung 4 wire imprinting; Rung 5 full cellular general body with SGC simplify. Each rung has its exact oracle (Q5).

**DECLINE FOREVER.** Isolated non-manifold-vertex results that bound no kept cell of dimension >= 1 (pure ambiguity, Parasolid-restricted); non-orientable sheets where a dependent op demands a material side (tag and fail the dependent op, not the boolean); coincident curved booleans on uncertified carriers (DECLINE bucket, MEMORY three-bucket oracle).

**The one-line verdict.** Item 29 is delivered as a **cell-complex selection layer over the existing pipeline plus three stitcher relaxations**, NOT a stitcher rebuild, which makes it **one milestone (Rung 1, exact) plus a follow-up ladder**, contingent only on first retiring the polygon soup per dossier 47.

---

## References

1. J. R. Rossignac, M. A. O'Connor. "SGC: A dimension-independent model for pointsets with internal structures and incomplete boundaries." Geometric Modeling for Product Engineering, North-Holland, 1990.
2. J. Rossignac, M. O'Connor. "Representing geometric structures in d dimensions: Topology and order." Discrete & Computational Geometry, 1991. https://link.springer.com/article/10.1007/BF02189330
3. J. Rossignac. "Topology for Computer Graphics" (course handout, subdivide / select / Merge). Georgia Tech, 2006. https://faculty.cc.gatech.edu/~jarek/graphics/reading/topology.pdf
4. K. Weiler. "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling." Geometric Modeling for CAD Applications, North-Holland, 1988.
5. E. L. Gursoz, Y. Choi, F. B. Prinz. "Vertex-based representation of non-manifold boundaries" (NOODLES). Geometric Modeling for Product Engineering, North-Holland, 1990.
6. Y. Choi, et al. "Boolean set operations on non-manifold boundary representation objects." Computer-Aided Design, 1991. https://www.sciencedirect.com/science/article/abs/pii/001044859190097G
7. Y. Choi (CMU). "Non-regularized Boolean set operations on non-manifold b-rep objects." https://kilthub.cmu.edu/articles/journal_contribution/Non-regularlized_Boolean_set_operations_on_non-manifold_b-rep_objects/6469088/1
8. H. Masuda. "Topological operators and Boolean operations for complex-based non-manifold geometric models." Computer-Aided Design, 25(2), 1993. https://www.sciencedirect.com/science/article/abs/pii/0010448593900978
9. A. M. B. Pereira, M. C. de Arruda, A. C. de O. Miranda, W. W. M. Lira, L. F. Martha. "Boolean operations on multi-region solids for mesh generation." Engineering with Computers, 27, 2011. https://link.springer.com/article/10.1007/s00366-011-0228-8
10. A. A. G. Requicha. "Representations for rigid solids: theory, methods, and systems." ACM Computing Surveys, 12(4), 1980.
11. A. A. G. Requicha, H. B. Voelcker. "Boolean operations in solid modeling: boundary evaluation and merging algorithms." Proc. IEEE, 73(1), 1985. https://scispace.com/pdf/boolean-operations-in-solid-modeling-boundary-evaluation-and-9zz6lpq7h1.pdf
12. R. B. Tilove. "Closure of Boolean operations on geometric entities." Computer-Aided Design, 1980. https://www.sciencedirect.com/science/article/abs/pii/0010448580900251
13. A. Jacobson, L. Kavan, O. Sorkine-Hornung. "Robust Inside-Outside Segmentation Using Generalized Winding Numbers." ACM TOG 32(4), 2013. https://igl.ethz.ch/projects/winding-number/
14. A. Jacobson. "Boolean Operations using Generalized Winding Numbers." arXiv:1601.07953, 2016.
15. J. Spainhour, K. Weiss. "Robust Containment Queries over Collections of Trimmed NURBS Surfaces via Generalized Winding Numbers." arXiv:2504.11435, 2025. https://arxiv.org/abs/2504.11435
16. J. Spainhour, et al. "Robust Containment Queries over Collections of Rational Parametric Curves via Generalized Winding Numbers." arXiv:2403.17371, 2024.
17. Q. Zhou, E. Grinspun, D. Zorin, A. Jacobson. "Mesh Arrangements for Solid Geometry." ACM TOG 35(4), 2016. https://www.cs.columbia.edu/cg/mesh-arrangements/
18. G. Cherchi, M. Livesu, R. Scateni, M. Attene. "Fast and Robust Mesh Arrangements using Floating-point Arithmetic." ACM TOG 39(6), 2020.
19. P. Hachenberger, L. Kettner, K. Mehlhorn. "Boolean Operations on 3D Selective Nef Complexes: Data Structure, Algorithms, and Implementation." ESA 2005 / CGAL. https://link.springer.com/chapter/10.1007/978-3-540-39658-1_59 ; thesis https://publikationen.sulb.uni-saarland.de/bitstream/20.500.11880/25961/1/Dissertation_1778_Hach_Pete_2006.pdf
20. Spatial Corporation (Dassault Systemes). ACIS Cellular Topology Component reference (R10), ch. 1; CELL / CSHELL / CFACE / ATTRIB_CELL class refs. http://www-isl.ece.arizona.edu/ACIS-docs/PDF/CT/01CMP.PDF
21. US Patent 7,031,790 B2. "Operator for sculpting solids with sheet bodies" (ACIS selective boolean, ct-graph). https://patents.google.com/patent/US7031790
22. US Patent 7,366,581 B2. "Replace face operator for solid body modeling" (web body, non-regularized unite).
23. US Patent 6,906,711 B2. "Graph-based method for multi-bodied sweep terminations" (ct-graph).
24. US Patent 9,817,924 B2. "Sweeping a profile along small circular arcs of a path" (non-regularized union with cellular topology).
25. Siemens Digital Industries Software. Parasolid Functional Description, ch. 8 "General Bodies," ch. 10 "Boolean Operations"; Parasolid XT Format Reference. (q-solid.com mirror; primary mirror offline at fetch time, claims reconstructed from dossier 02 and secondary summaries.)
26. R. Bidarra, et al. "Boundary Evaluation for a Cellular Model" / cellular representation for feature modeling. (Cellular-model boundary evaluation, regions maintained through edits.)

### Related Keel dossiers (build on, not repeated)
- `docs/research/kernel/02-nonmanifold-cellular.md` (the representation survey: SGC, Weiler RES, Lee-Lee PES, NOODLES, Masuda operators, ACIS Cellular Topology, Parasolid general bodies, first-class regions). This dossier consumes that and adds only boolean semantics.
- `docs/research/kernel/47-boolean-result-assembly.md` (the `EdgeId` radial-splice stitcher, closure invariant, identity-from-imprint, retire-the-soup recommendation). The Rung-0 prerequisite.
- `docs/research/kernel/39-coincident-tangent-face-booleans.md` (on-on two-sided neighborhood test, 2D-in-parameter-domain coincident imprint).
- `docs/research/kernel/51-sheet-bodies-ops.md` (sheet PES validity, radial-cycle-length-1 free edge, double-sided faces, open-shell Euler bookkeeping).
- `docs/research/kernel/09-mesh-implicit-hybrid.md` (mesh-arrangement booleans, winding-number-vector per-cell labeling).
- `docs/research/d-booleans-tolerant.md` (boolean pipeline, local imprint, EMBER no-global-arrangement lesson).
