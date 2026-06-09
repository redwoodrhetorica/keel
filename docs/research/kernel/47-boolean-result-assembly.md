# 47. Robust Boolean Result Assembly: Not Dropping the Tilted Cut Face

## Title and scope

This dossier is an implementation-grade investigation into **robust boolean result assembly**: the final stage of the boolean pipeline that builds the output B-rep shell from classified, selected face fragments without ever silently dropping a kept fragment. The target is a specific, reproducible bug in the Keel kernel.

**The pipeline (Keel).** Keel is a tolerant-f64, non-manifold (PES / radial-edge) B-rep kernel in Rust. Its boolean pipeline is:

1. `preimprint` coincident overlaps,
2. `seam_curves` (analytic surface-surface intersection),
3. `imprint_operand(a)` and `imprint_operand(b)` (split faces along the seams),
4. `classify_faces` (generalized winding number: inside / outside / on, with a 0.25 band around w = 0.5),
5. `select_faces` (Requicha set-membership tables, producing `KeptFace { operand, face, reversed }`),
6. **assemble**, which forks by geometry:
   - all-planar result: `build_result_solid`, a **polygon-soup stitcher** over `ResultPoly { ring: Vec<Vec3>, outward: Vec3, surface }`. It dedups vertices by coordinate, shares edges, builds fins / loops / faces directly in the arenas, then partitions into solid and void.
   - curved result: `stitch_by_import`, an Euler import-and-glue path on shared topology.

**The bug (reproducible, the recommendation is validated against it).** A non-transversal / thin-tilted-feature Difference drops a kept face. Repro: subtract a thin tilted wedge prism from a box corner (an asymmetric chamfer: setbacks d1 != d2, cut plane at a non-45-degree angle). The kept B-fragment is the single tilted cut face. The result **validates** (Euler-consistent) but is geometrically **wrong**: `build_result_solid` emits an all-axis-aligned face soup that **drops the tilted cut face** and leaves stray fragments. At d1 = 0.5, d2 = 1.0 on a 2x2x2 box: `mesh_volume` reads 8.83, analytic `mass_properties` reads 11.5, the true answer is 7.5. All three disagree because the body itself is malformed.

The symmetric 45-degree chamfer works perfectly. `fuzz_boolean` over random transversal bodies is clean. **This is a `build_result_solid` (assembly) bug, not a classification or mass-properties bug.** `classify` and `select` are believed correct: the tilted fragment IS selected as a `KeptFace`. Assembly drops it.

This is a design recommendation grounded in production-kernel and mesh-arrangement practice, **not a transcription of any kernel's source**. It is to be validated against Keel's asymmetric-chamfer repro and the boolean regression suite.

---

## THE CENTERPIECE: drop-prevention invariant, identity-preserving assembly, and the soup-vs-Euler recommendation

Before the per-question detail, here is the load-bearing conclusion, stated once, explicitly.

### The drop-prevention invariant (shell-closure invariant)

> **Every coedge (fin) of every kept face has exactly one radial partner across a shared edge before the shell is declared closed (or, in the non-manifold case, a complete, consistent radial cycle around that edge). An unmatched coedge is an ERROR to surface, never a face to silently drop.**

A correct assembler does not "build a shell and hope every fragment got included." It asserts the invariant and refuses to finalize a shell with an unpaired coedge. A dropped face is exactly the symptom of an assembler that, instead of asserting this invariant, re-derived edge sharing geometrically, failed, and quietly omitted the fragment whose edges it could not match.

### The identity-preserving assembly algorithm (the fix)

The hypothesis to confirm is **confirmed**: production kernels do **not** re-derive edge sharing by coordinate dedup at assembly time. They **carry edge identity through from the imprint stage**. When `seam_curves` produced an intersection curve and `imprint_operand` split a face along it, that imprint **created the shared edge once** and registered both incident face fragments against it. Assembly is therefore a **coedge-pairing / radial-cycle-splice over edges that are already shared by construction**, not a geometric re-weld.

Concretely, the recommended assembly is:

1. **Imprint records identity.** Each seam edge gets a stable `EdgeId`. Every fragment produced by splitting along that seam stores, on each of its boundary coedges, the `EdgeId` of the edge it lies on (seam edges from imprint; original-face boundary edges inherit their parent edge's identity, sub-divided into pave blocks with stable child ids). No coordinates are consulted to establish sharing.
2. **Assembly groups coedges by `EdgeId`, not by coordinate.** For each `EdgeId`, collect all coedges of kept fragments referencing it. Manifold edge: exactly two, opposite-sense. Non-manifold edge: a radial cycle, sorted by dihedral angle around the edge axis.
3. **Radial splice.** Pair / cycle the coedges using the file-46 sense convention (below). Build the radial-edge structure directly from these groups.
4. **Assert closure.** Every coedge must land in exactly one pairing (or one consistent radial cycle). If any coedge has no partner under its `EdgeId`, that is a **hard error** (an assertion / diagnostic), not a dropped face. The tilted cut face cannot vanish because its coedges carry the seam `EdgeId` that its neighbors also carry; the pairing is structural.

The tilted fragment is dropped today precisely because `build_result_solid` throws away this imprint identity, reduces every fragment to a bag of `Vec3` rings, and re-establishes sharing by coordinate dedup. The oblique edge of the thin wedge then either (a) gets a vertex welded into the axis-aligned grid and loses its match, or (b) cannot be matched because the soup model has no place to record "this edge already had two known owners." The face with no matched edges is silently omitted.

### The feature-size-relative tolerance rule

> **Do not weld by coordinate at all in the common case. Use the identity from imprint. Reserve a weld tolerance only for genuine near-coincidence that the imprint flagged. When a weld is unavoidable, the weld epsilon must be a fraction of the LOCAL feature size, never a single global constant: `weld_eps_local = k * min(local_edge_lengths)` with k around 0.01 to 0.1, and welds across that local threshold are forbidden even if a global epsilon would have permitted them.**

A single global weld epsilon is a contradiction the soup path cannot resolve: it must be smaller than the smallest real feature (the 0.5-thick wedge) to avoid collapsing it, yet coincidence detection wants it larger to fuse genuinely-coincident geometry that drifted. Identity-from-imprint dissolves the contradiction because genuine coincidence was already detected at `preimprint` / `seam` time with full context, and assembly never needs a global weld.

### The recommendation: retire `build_result_solid`, route the planar path through `stitch_by_import`

> **The oriented polygon soup is the wrong architecture for a tolerant non-manifold kernel. The planar path should use the same identity-preserving, radial-edge incremental glue Keel already uses for curved results. Retire `build_result_solid`; make `stitch_by_import` (generalized to take the planar fragments with their imprint edge identity) the single assembly path.**

The reference kernels (OCCT BOPAlgo, the PADL/Requicha lineage, every mesh-arrangement assembler) do **not** build from polygon soup. They imprint-and-stitch incrementally on shared topology, then extract cells by labeling. The soup path's only appeal is directness for the all-planar common case; its fatal flaw is that it discards the imprint's edge identity and cannot represent non-manifold radial cycles or thin features robustly. A single, identity-preserving assembly path is simpler to keep correct than two paths that diverge exactly on the hard cases.

---

## Q1. The robust imprint-classify-stitch assembly algorithm

The invariant and algorithm are stated in full in the centerpiece above. Here are the sources that establish it.

### Requicha and Voelcker, "Boolean operations in solid modeling: boundary evaluation and merging algorithms" (Proc. IEEE, Jan 1985)
- **Citation.** A. A. G. Requicha and H. B. Voelcker, "Boolean operations in solid modeling: boundary evaluation and merging algorithms," Proceedings of the IEEE, 73(1), 1985.
- **Content.** The canonical formulation. Boundary evaluation is three stages: (1) **subdivision**: split each operand's boundary along its intersection with the other operand's boundary (this is exactly imprint); (2) **classification**: set-membership classification of each resulting piece as in / on / out of the other solid (neighborhood manipulation resolves on-on coincidence); (3) **reconstruction / merging**: gather the selected pieces and **connect** them into the output boundary. The pieces produced in stage (1) already share the edges created by the intersection. Stage (3) is a connection over those shared boundaries, not a re-derivation of them.
- **Kernel relevance.** This is the source of the "carry identity through" invariant. The subdivision stage created the shared edges; the merging stage must reuse them. Keel's `build_result_solid` collapses stages (1) and (3) into a coordinate re-weld and thereby violates the design Requicha and Voelcker laid out. `select_faces` (Requicha tables) is Keel's classification stage and is correct; the merge is where the regression lives.

### Mantyla, "An Introduction to Solid Modeling" (Computer Science Press, 1988)
- **Citation.** M. Mantyla, An Introduction to Solid Modeling, Computer Science Press, 1988.
- **Content.** The textbook treatment of boundary evaluation built on the half-edge model and **Euler operators**. Set operations are performed by splitting (via Euler operators that introduce edges and vertices on the intersection), classifying, and then knitting faces together with further Euler operators. Crucially, the topology is mutated by operators that **preserve the Euler invariant at every step** and operate on shared half-edges, so connectivity is never re-derived from coordinates.
- **Kernel relevance.** This is precisely the `stitch_by_import` (Euler import-and-glue) model Keel already has for curved results. The recommendation is to use it for the planar path too. Mantyla's half-edge knit-by-operator is the manifold special case of Weiler's radial-edge splice (next).

### Weiler, "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Modeling" (1988)
- **Citation.** K. Weiler, "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling," in Geometric Modeling for CAD Applications, 1988.
- **Content.** Generalizes Baumgart's winged edge to non-manifold geometry by recording the **radial ordering of faces around a non-manifold edge**. Distinguishes the abstract unoriented edge from oriented **edge-uses** (coedges / fins). The three cycle types are loop (around a face boundary), **radial** (faces around an edge), and disk. An edge with more than two incident faces is represented exactly by its radial cycle.
- **Kernel relevance.** This is Keel's own data model (PES / radial-edge). The invariant "every coedge has a radial partner / complete radial cycle" is Weiler's radial cycle being closed. The polygon soup cannot represent a radial cycle of more than two faces, which is failure mode (b) in Q2: when the welded thin-wedge edge has three or more candidate coedges, the soup has no structure to hold them and the surplus fragment is dropped. The assembler must build the radial cycle from the imprint's edge groups, which is what `stitch_by_import` does and `build_result_solid` does not.

---

## Q2. Drop failure modes in a polygon-soup stitcher

Three concrete mechanisms make a face vanish during vertex-dedup-plus-edge-sharing assembly. For the asymmetric-chamfer repro, (a) and (b) are the operative ones.

### (a) Vertex welding collapses a thin feature's distinct-but-close vertices
- **Cause.** The soup stitcher dedups vertices by coordinate under a global epsilon. The 0.5-thick wedge has two distinct vertices (the d1 = 0.5 setback and the d2 = 1.0 setback corners, or the thin edge of the wedge) separated by less than the epsilon needed to also fuse the genuinely-coincident box corners after numerical drift. Welding them collapses the tilted edge to zero length, the tilted face degenerates to a sliver or to nothing, and it is discarded as degenerate.
- **Robust fix.** Do not weld by coordinate. Use imprint identity (Q1). If a weld is genuinely required, make the epsilon feature-size-relative and **forbid welds that would shorten an edge below a local feature threshold** (Q4). A weld that would collapse a real feature is itself the bug, not a cleanup.

### (b) Ambiguous edge-pairing when more than two coedges share a welded edge
- **Cause.** After a coordinate weld, the oblique seam edge between the tilted cut face and an adjacent box face can end up sharing a welded vertex / edge with a third axis-aligned fragment. The soup model assumes manifold (exactly two coedges per edge). When it sees three candidate coedges it cannot form a valid radial cycle, picks a manifold pair, and **orphans the third** (the tilted face's coedge). The orphaned face is dropped.
- **Robust fix.** Represent the edge as a Weiler radial cycle. Group coedges by `EdgeId` from imprint and sort the whole group by dihedral angle around the edge axis to form the radial cycle; never assume two. This is structurally impossible in the soup model and is exactly why the architecture must change (Q3).

### (c) Winding / orientation rejection of the oblique face
- **Cause.** If the assembler recomputes each face's outward direction from its ring winding and then rejects faces whose normal disagrees with the axis-aligned majority (an implicit "snap to axis" heuristic), the lone tilted face is the minority and gets discarded.
- **Robust fix.** Never re-derive per-face orientation from ring winding at assembly time. Carry the `KeptFace.reversed` flag and the file-46 outward-normal convention (`n_out = sense x natural`) through the stitch (Q6). Orientation is decided at selection, not re-litigated at assembly.

---

## Q3. Soup-stitch vs incremental Euler / half-edge assembly

### The architectural question, answered
The oriented polygon soup is the **wrong architecture** for a tolerant non-manifold kernel. The recommendation is to **retire `build_result_solid`** and route the planar path through `stitch_by_import`.

- **Soup path appeal.** Simplicity and directness for the all-planar common case. One ring per face, dedup, done. This is why it was written.
- **Soup path fatal flaw.** It **discards the imprint's edge identity** (the single most valuable artifact the pipeline produced) and **cannot represent non-manifold radial cycles or thin features**. It re-solves, by fragile coordinate arithmetic, a sharing problem that imprint already solved exactly. The two failure modes (a) and (b) in Q2 are direct consequences.

### What the reference kernels actually do
- **OCCT BOPAlgo** (see Q5 source below): the builder constructs splits of faces (`BOPAlgo_BuilderFace`) from collected edges **with orientations and shared section edges carried from the intersection Data Structure**, tracks images and same-domain relations in maps (`myImages`, `myShapesSD`), then `BOPAlgo_BuilderSolid` assembles shells from faces that **already share edges by construction**. Section edges and paves provide stable identity; the builder never re-welds by coordinate.
- **PADL / Requicha lineage**: subdivision creates shared edges, merging connects over them.
- **Mesh-arrangement kernels** (Q5): build a complete arrangement (every fragment and intersection a node in one shared complex) and extract by labeling; no soup, no re-weld.

None of the reference kernels build the result from a polygon soup. They imprint-and-stitch incrementally on shared topology.

### Recommendation for Keel
Generalize `stitch_by_import` to accept the planar fragments **carrying their imprint `EdgeId`s**, and delete `build_result_solid`. One assembly path, identity-preserving, radial-edge native, asserting the closure invariant. This is strictly more correct than the soup and removes the planar / curved divergence that hides the bug (the symmetric 45-degree chamfer works only because its coordinate weld happens to land cleanly; the oblique case is where the soup's re-derivation fails).

---

## Q4. Tolerance model for thin features in assembly

The rule is stated in the centerpiece. The supporting reasoning and sources:

### The global-epsilon contradiction
A single global weld epsilon must be **smaller than the smallest real feature** (so the 0.5-thick wedge survives) yet **larger** than typical coincidence drift (so genuinely-coincident geometry fuses). For an input mixing a 2.0 box and a 0.5 feature these two requirements can conflict, and the soup path has no way to localize the threshold. This is unresolvable in the soup model.

### Feature-size-relative epsilon
Set `weld_eps_local = k * min(local_edge_lengths)` (k around 0.01 to 0.1) per locality, and **forbid any weld that shortens an edge below the local feature threshold**. A weld that would collapse a real feature is a detected error, not a silent cleanup. This is the C3D tolerant-edge model (file 42): a tolerant edge is a tube of radius t, and t is bounded by local geometry, not a global constant.

### Snap-rounding and why naive snapping cascades (tie to file 30)
- **Citation.** J. Hobby, "Practical segment intersection with finite precision output," Comp. Geom. 1999; L. Guibas and D. Marimont, "Rounding arrangements dynamically," 1998; D. Halperin and E. Packer, "Iterated snap rounding," Comp. Geom. 2002.
- **Content.** Snap rounding maps arrangement vertices to a fixed grid in a globally topologically consistent way. **Naive snapping cascades**: snapping one vertex moves an edge, which now passes too close to another vertex, which must also snap, and so on. Iterated snap rounding repeats until a minimum vertex-edge separation holds, but it can drift features and merge distinct vertices, which is exactly the thin-wedge collapse (file 30's snapping / zippering pitfall, the one-shot clustered merge).
- **Kernel relevance.** Snap rounding is a 2D arrangement tool; applying its philosophy (a global grid weld) to 3D boolean assembly is the root cause of the drop. The clean resolution is **do not snap / weld by coordinate at all**: use imprint identity, reserve tolerance for the genuine near-coincidences that `preimprint` / `seam_curves` already flagged with full geometric context.

### Qi and Shapiro, epsilon-solidity
- **Citation.** J. Qi and V. Shapiro, "Epsilon-solidity in geometric data translation," and "Epsilon-regular sets and intervals," Univ. of Wisconsin, 2004-2006.
- **Content.** Formalizes tolerant solidity: a shape is epsilon-solid if it is solid under finite-size-neighborhood (epsilon-topological) operations. Shows that classical exact-validity checks are **neither necessary nor sufficient** to maintain solidity under numerical inaccuracy, and that the right move is to reason with epsilon-neighborhoods sized to the data rather than to heal after the fact.
- **Kernel relevance.** Justifies the feature-size-relative tolerance: the epsilon must be a property of the local neighborhood, not a global constant. It also explains the deceptive "validates but is wrong" symptom: Keel's result passes the Euler check (it is a valid closed shell) yet is not epsilon-solid relative to the true geometry, because a face was dropped and others snapped. Euler-validity is necessary but not sufficient.

---

## Q5. Mesh-arrangement booleans: what transfers

The transferable principle: **build a complete arrangement and extract output cells by labeling, never by welding or discarding.** Completeness is structural, so a combinatorially-required face cannot be dropped by a numerical error.

### Zhou, Grinspun, Zorin, Jacobson, "Mesh Arrangements for Solid Geometry" (SIGGRAPH 2016)
- **Citation.** Q. Zhou, E. Grinspun, D. Zorin, A. Jacobson, "Mesh Arrangements for Solid Geometry," ACM TOG 35(4), 2016.
- **Content.** Resolve all self-intersections into one **arrangement** (a complete simplicial complex where every fragment and every intersection is represented). Assign a **winding-number vector** to every cell. Each boolean (union, difference, intersection, symmetric difference, and arbitrary expressions) is an **extraction** of cells whose winding vector satisfies a predicate. No cell is ever welded away or discarded; the output is selected from a complete partition. Used in Blender.
- **Kernel relevance.** This is the structural completeness Keel's assembly lacks. The fix is the B-rep analogue: the imprint produces the complete arrangement (all fragments share edges), and selection labels them. Drop-proofness comes from the arrangement being complete before labeling, exactly the invariant in Q1.

### Jacobson, Kavan, Sorkine-Hornung, "Robust Inside-Outside Segmentation Using Generalized Winding Numbers" (SIGGRAPH 2013)
- **Citation.** A. Jacobson, L. Kavan, O. Sorkine-Hornung, "Robust Inside-Outside Segmentation Using Generalized Winding Numbers," ACM TOG 32(4), 2013.
- **Content.** The generalized winding number is well-defined and well-behaved for arbitrary meshes (self-intersecting, non-manifold, open) with no special-case code. It gives a smooth inside-outside field whose 0.5 level set is the boundary.
- **Kernel relevance.** This is exactly Keel's `classify_faces` (winding number with a 0.25 band around w = 0.5). It is the labeling primitive. It is correct in Keel; the problem is downstream. Listed to confirm the classification stage is sound and the assembler must not re-decide orientation (Q6).

### Cherchi, Livesu, Scateni, Attene, "Fast and Robust Mesh Arrangements using Floating-point Arithmetic" (SIGGRAPH Asia 2020)
- **Citation.** G. Cherchi, M. Livesu, R. Scateni, M. Attene, ACM TOG 39(6), Article 250, 2020.
- **Content.** Transforms any set of triangles into a well-formed simplicial complex by representing each intersection point as an **unevaluated combination of input vertices** and using **indirect predicates** (Attene 2020) so all geometric tests are exact yet hardware-fast. Intersecting elements are identified, subdivided, and connected into a valid arrangement. Topologically sound partition by construction.
- **Kernel relevance.** The transferable idea is **deferred / unevaluated coordinates**: never collapse an intersection point to a rounded coordinate that a later stage re-derives. Keel's analog is `EdgeId` identity: the seam edge is named, not re-found. The exact-arithmetic part does **not** transfer to curved analytic B-rep (no exact predicates for surface-surface intersection in general), but the **identity-not-coordinate** principle transfers fully and is the core of the fix.

### Trettner, Nehring-Wirxel, Kobbelt, "EMBER: Exact Mesh Booleans via Efficient and Robust Local Arrangements" (SIGGRAPH 2022)
- **Citation.** P. Trettner, J. Nehring-Wirxel, L. Kobbelt, "EMBER: Exact Mesh Booleans via Efficient and Robust Local Arrangements," ACM TOG 41(4), 2022.
- **Content.** Plane-based mesh representation with homogeneous integer coordinates makes predicates exact. Robustness via generalized winding numbers and mesh arrangements. Performance via **adaptive recursive subdivision of the bounding box, building local arrangements on the fly**, avoiding a global acceleration structure. This is the EMBER "no global arrangement" lesson cited in `d-booleans-tolerant.md`: arrange **locally** where intersections occur, not globally.
- **Kernel relevance.** Two transfers. (1) Local arrangement: Keel already imprints locally (no global arrangement, per file d-booleans-tolerant); the fix is to **keep** that locality through assembly by carrying local edge identity, rather than throwing it away and re-globalizing via coordinate dedup. (2) Plane-based exactness does **not** transfer to curved analytic surfaces, but the labeling-not-welding extraction does.

### Sugihara and Iri, "Topology-Oriented Implementation: An Approach to Robust Geometric Algorithms" (Algorithmica 2000)
- **Citation.** K. Sugihara, M. Iri, et al., "Topology-Oriented Implementation: An Approach to Robust Geometric Algorithms," Algorithmica 27, 2000.
- **Content.** Decide topology **combinatorially first**, giving topological consistency the highest priority; use numerics only to choose among combinatorially-valid branches. The combinatorial part is never contaminated by numerical error, so the algorithm cannot crash or drop a required entity no matter how large the numerical error. Self-correcting.
- **Kernel relevance.** This is the deepest transferable idea, and it transfers **fully** to curved B-rep where exact arithmetic is unavailable. The combinatorial fact "the tilted face's coedges share these named edges with these neighbors" is decided at imprint and is true regardless of f64 error. Assembly must honor that combinatorial fact and **force geometry to conform**, never let a coordinate comparison override it and drop the face. A combinatorially-required face is structurally guaranteed to appear. This is precisely the closure invariant of Q1 restated as a design philosophy.

### What survives the move to curved analytic B-rep, and what does not
- **Transfers (survives without exact arithmetic):** completeness-then-labeling (Zhou); generalized winding number classification (Jacobson, already in Keel); identity-not-coordinate / deferred construction (Cherchi); local arrangement, keep locality (EMBER); **topology-decided-combinatorially, geometry-conforms (Sugihara-Iri)**, which is the strongest and the backbone of the fix.
- **Does not transfer:** exact integer / plane-based predicates (Cherchi, EMBER) and exact rational coordinates. Curved surface-surface intersection has no general exact predicate, so Keel must use tolerant numerics. This is **why** identity-from-imprint matters more for Keel than for a mesh kernel: Keel cannot fall back on exact re-derivation, so it must never re-derive at all.

---

## Q6. Orientation continuity for the kept reversed B-fragment

### The requirement
The tilted cut face in the repro is a kept **reversed** B-fragment (the new cavity wall). Its `KeptFace.reversed` flag must propagate into the fin / coedge orientation so the fragment's coedges run consistently with **material-outward**, and assembly must use the file-46 outward-normal convention (`n_out = sense x natural`, fin / material-primary, via the `canonicalize` / `outward_normal` helpers), **not a re-derived per-face winding**.

### Mechanism, tied to file 46
- File 46 unified `mass_properties` to a sense-based convention: `n_out = sense x natural`. The boolean stitch was **not** changed at that time. So today the stitch may still infer orientation from ring winding (`ResultPoly.outward` is a `Vec3` carried alongside the ring, which invites the assembler to re-derive winding rather than honor sense).
- For a **reversed** B-fragment in a Difference, the natural surface normal points into the removed material; the solid-outward normal is the **reverse**. The `reversed` flag captures exactly this. If assembly builds the loop from the ring in its natural winding and computes outward from that, it produces the wrong-side normal for the tilted face. The face is then either (i) mis-bounded (its coedges run the wrong way, breaking the radial pairing with neighbors, so the closure invariant fails and the face is dropped, Q2 mode b), or (ii) it bounds the void instead of the solid in the solid / void partition, so it is partitioned out.
- **Correct handling:** `select_faces` already produced `reversed`. Assembly must set fin orientation as `effective_sense = face.sense XOR kept.reversed`, then `n_out = effective_sense x natural`, and **build the loop direction from `effective_sense`**, never from ring winding. The `ResultPoly.outward` field should be derived from `effective_sense` (or removed in favor of carrying `sense` + `reversed`), not measured from the polygon.

### How inconsistent reversed-flag handling drops or mis-bounds the face
If two adjacent fragments around the seam edge disagree on sense (one honors `reversed`, one re-derives from winding), their coedges along the shared seam will not be antiparallel. The radial-cycle splice cannot pair them. Under the soup model this surfaces as an orphaned coedge, and the orphaned face (the tilted one, the minority orientation) is dropped. Under the recommended identity-preserving model, the closure invariant assertion fires and surfaces the inconsistency as an **error to fix**, not a face to drop. Either way, the cure is: orientation is decided once, at selection, via the file-46 convention, and is **carried**, never recomputed.

---

## Robust result assembly for Keel: synthesis

**Recommended assembly algorithm (single path, replacing `build_result_solid`):**

1. **Imprint assigns stable identity.** Every seam edge from `seam_curves` gets an `EdgeId`. Every original-edge pave block gets a stable child id. Every fragment coedge stores the `EdgeId` of the edge it lies on. No coordinates establish sharing.
2. **Selection carries orientation.** `select_faces` emits `KeptFace { operand, face, reversed }`. Compute `effective_sense = face.sense XOR reversed` and `n_out = effective_sense x natural` per the file-46 convention. Do not re-derive winding.
3. **Group coedges by `EdgeId`.** For each `EdgeId`, collect all kept-fragment coedges referencing it.
4. **Radial splice.** Manifold edge: exactly two antiparallel coedges, pair them. Non-manifold edge: sort the group by dihedral angle around the edge axis into a radial cycle (Weiler). Build the PES / radial-edge structure directly from these groups.
5. **Assert the closure invariant.** Every coedge lands in exactly one pairing / one consistent radial cycle. Any unmatched coedge is a hard error with a diagnostic (which `EdgeId`, which fragment), never a silent drop.
6. **Partition solid / void** using the carried `n_out`, not re-measured winding.
7. **Validate** Euler consistency **and** that the fragment count out equals the kept count in (a count check that would have caught this bug immediately).

**Why this fixes the asymmetric-chamfer repro.** The tilted cut face is dropped today because `build_result_solid` discards imprint identity, reduces the fragment to a `Vec3` ring, and re-establishes edge sharing by coordinate dedup, which fails on the oblique edge (the thin-wedge vertex welds into the axis-aligned grid, or the welded edge gathers three coedges the soup cannot represent, or the minority winding is rejected). Under the recommended algorithm, the tilted face's coedges carry the **same seam `EdgeId`** its neighbors carry. They are paired by **identity, not by geometry**. There is no coordinate comparison to fail, no axis-aligned majority to reject the oblique face, and no manifold-only assumption to orphan a third coedge. The closure invariant guarantees the tilted face's coedges find their radial partners or the build errors out loudly. The face cannot vanish. With orientation carried from `reversed` via the file-46 convention, it bounds the solid on the correct side, so `mass_properties` and `mesh_volume` both converge to the true 7.5.

**Retire `build_result_solid`?** Yes. Generalize `stitch_by_import` to take planar fragments with their imprint `EdgeId`s and make it the single assembly path. The soup path's only virtue is directness for the easy case; its architecture cannot represent the non-manifold radial cycles and thin features that the hard cases require, and maintaining two paths is what let the bug hide behind the passing 45-degree symmetric case. One identity-preserving, radial-edge-native, invariant-asserting path is both simpler and correct.

**Validation plan.** Run the asymmetric-chamfer repro (d1 = 0.5, d2 = 1.0, non-45-degree cut) and confirm `mass_properties == mesh_volume == 7.5`; re-run the symmetric 45-degree chamfer (must still pass); re-run `fuzz_boolean` (must stay clean); add a count-check assertion (kept faces in == faces out) and a thin-feature regression (sub-unit wedge into unit-scale box) to the boolean suite. This recommendation is a design grounded in production-kernel and mesh-arrangement practice, to be validated against these.

---

## References

1. A. A. G. Requicha, H. B. Voelcker. "Boolean operations in solid modeling: boundary evaluation and merging algorithms." Proceedings of the IEEE, 73(1), 1985. https://ieeexplore.ieee.org/abstract/document/1457376/
2. M. Mantyla. An Introduction to Solid Modeling. Computer Science Press, 1988. https://archive.org/details/introductiontoso0000mant
3. K. Weiler. "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling." Geometric Modeling for CAD Applications, 1988. https://people.eecs.berkeley.edu/~ddgarcia/cs184/kjw/bib.html
4. Q. Zhou, E. Grinspun, D. Zorin, A. Jacobson. "Mesh Arrangements for Solid Geometry." ACM TOG 35(4), 2016. https://www.cs.columbia.edu/cg/mesh-arrangements/
5. A. Jacobson, L. Kavan, O. Sorkine-Hornung. "Robust Inside-Outside Segmentation Using Generalized Winding Numbers." ACM TOG 32(4), 2013. https://igl.ethz.ch/projects/winding-number/
6. A. Jacobson. "Boolean Operations using Generalized Winding Numbers." arXiv:1601.07953, 2016. https://arxiv.org/pdf/1601.07953
7. G. Cherchi, M. Livesu, R. Scateni, M. Attene. "Fast and Robust Mesh Arrangements using Floating-point Arithmetic." ACM TOG 39(6), 2020. https://www.gianmarcocherchi.com/pdf/mesh_arrangement.pdf
8. M. Attene. "Indirect Predicates for Geometric Constructions." 2020. (Referenced by Cherchi et al. 2020.)
9. G. Cherchi, et al. "Interactive and Robust Mesh Booleans." arXiv:2205.14151, ACM TOG 2022. https://www.gianmarcocherchi.com/pdf/interactive_exact_booleans.pdf
10. P. Trettner, J. Nehring-Wirxel, L. Kobbelt. "EMBER: Exact Mesh Booleans via Efficient and Robust Local Arrangements." ACM TOG 41(4), 2022. https://www.graphics.rwth-aachen.de/media/papers/339/ember_exact_mesh_booleans_via_efficient_and_robust_local_arrangements.pdf
11. K. Sugihara, M. Iri, H. Inagaki, T. Imai. "Topology-Oriented Implementation: An Approach to Robust Geometric Algorithms." Algorithmica 27, 2000. https://link.springer.com/article/10.1007/s004530010002
12. K. Sugihara. "Robust Geometric Computation Based on Topological Consistency." 2001. https://link.springer.com/chapter/10.1007/3-540-45545-0_10
13. Open CASCADE Technology. "Boolean Operations" user guide (BOPAlgo_Builder, BOPAlgo_BuilderFace, BOPAlgo_BuilderSolid, same-domain faces). https://dev.opencascade.org/doc/occt-7.4.0/overview/html/occt_user_guides__boolean_operations.html
14. Open CASCADE Technology. BOPAlgo_Builder Class Reference. https://dev.opencascade.org/doc/occt-7.6.0/refman/html/class_b_o_p_algo___builder.html
15. J. Hobby. "Practical segment intersection with finite precision output." Computational Geometry, 1999.
16. L. Guibas, D. Marimont. "Rounding arrangements dynamically." Int. J. Comp. Geom. Appl., 1998.
17. D. Halperin, E. Packer. "Iterated snap rounding." Computational Geometry, 2002. https://www.cgl.cs.tau.ac.il/projects/iterated-snap-rounding/
18. "Snap Rounding: A Cautionary Tale." SoCG 2025, Schloss Dagstuhl LIPIcs. https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.SoCG.2025.57
19. J. Qi, V. Shapiro. "Epsilon-solidity in geometric data translation" / "Epsilon-regular sets and intervals." Univ. of Wisconsin, 2004-2006. https://spatial.engr.wisc.edu/wp-content/uploads/sites/715/2014/04/2004-2p2.pdf

### Related Keel dossiers (build on, not repeated)
- `docs/research/d-booleans-tolerant.md` (boolean pipeline, winding-number classification, local imprint, EMBER no-global-arrangement lesson)
- `docs/research/kernel/09-mesh-implicit-hybrid.md` (Zhou arrangements, Cherchi/Attene indirect predicates, EMBER)
- `docs/research/kernel/39-coincident-tangent-face-booleans.md` (coincident / tangent handling, two-sided neighborhood test)
- `docs/research/kernel/30-imperfect-brep-defect-handling.md` (snapping / zippering pitfalls, one-shot clustered merge)
- `docs/research/kernel/11-curved-exact-predicates.md` (degeneracy-dense input, filter failure)
- `docs/research/kernel/46-face-orientation-sense-convention.md` (n_out = sense x natural, fin / material-primary, canonicalize / outward_normal)
- `docs/research/kernel/42-russian-c3d-literature.md` (C3D tolerant edges = tube of radius t)
