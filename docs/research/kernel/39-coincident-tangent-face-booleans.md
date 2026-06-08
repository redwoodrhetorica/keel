# 39. Coincident, Coplanar, and Tangent Face Handling in B-rep Boolean Operations

**Scope.** This dossier is the implementation-grade bridge from Keel's winding-number-vector boolean concept to a working curved-B-rep handler for the three degeneracy families that dominate boolean failure: coincident/coplanar overlapping faces, tangent (touching, non-crossing) faces, and their curved-surface generalizations. It does not re-derive the boolean pipeline (see `docs/research/d-booleans-tolerant.md`), the winding-number / mesh-implicit background (`kernel/09-mesh-implicit-hybrid.md`), or the dirty-geometry doctrine (`kernel/29`, `kernel/30`). It assumes Keel's pipeline as given: BVH broad phase, local imprint along intersection curves only (no global arrangement, the EMBER lesson), winding-number-vector classification (Zhou-Jacobson style), tolerant stitch.

The centerpiece is **Section 2**: directly-encodable keep/drop/orient tables for union, intersection, and difference as a function of relative orientation (same-sense vs opposite-sense) of two coincident faces. The other four sections supply the imprint machinery (1), tangency handling (3), production-kernel structure (4), and the analytic-then-NURBS coincidence-detection ladder (5).

A note on terminology used throughout. "On-on" denotes a face fragment of body A that lies on the boundary of body B and is itself on the boundary of A (Requicha's set-membership term). "Same-sense" means the two coincident faces' outward normals agree on the overlap region; "opposite-sense" means they oppose. "Imprint" means splitting a face by a curve lying in it without changing its geometry.

---

## Section 1. Coincident-Surface Overlap: Imprint and Classification

### 1.1 The problem

Two faces F_A (on body A) and F_B (on body B) share the *same carrier surface* on some region: two coplanar planes overlap; two coaxial equal-radius cylinders overlap; two concentric equal-radius spheres overlap; two NURBS faces coincide within tolerance. The surface-surface intersection (SSI) of F_A and F_B is not a curve where the surfaces *cross*, it is a two-dimensional *overlap region* where they are identical. Ordinary SSI marching produces nothing usable here: there is no transversal intersection curve, the marching direction (cross product of normals) is identically zero over the whole region, and a naive arrangement either drops the faces or duplicates them. This is the single highest-risk boolean sub-problem (per Keel's completeness audit) precisely because the standard SSI + classify pipeline silently degenerates.

### 1.2 Overlap-region boundary computation

The overlap region O = F_A ∩ F_B is the set of points that lie inside the trimmed domain of *both* faces and on the common carrier surface. Its boundary is not an SSI curve; it is the **2D intersection of the two trimmed parameter domains**, mapped through the shared surface:

1. **Establish a common parameterization.** For analytic carriers, map both faces' trim loops into one canonical parameter space of the shared surface (e.g. plane (u,v); cylinder (theta, z); sphere (phi, lambda)). For coincident NURBS this requires common refinement (Section 5).
2. **2D boolean of the trim loops.** Compute O's parameter-space boundary as the *planar* set intersection of trimmed-domain(F_A) and trimmed-domain(F_B). This is a 2D polygon/curve-loop boolean in (u,v), one dimension lower than the 3D problem, so it is tractable and robust. The result is a set of loops bounding O in parameter space.
3. **Lift to 3D.** Map O's boundary loops back through the carrier surface to 3D curves. These are the imprint curves.

The key insight: a coincident-overlap "intersection" is solved as a **2D arrangement in the shared surface's parameter domain**, never as a 3D SSI. This is also why recognizing a shared analytic carrier (Section 5, `kernel/24`) is what makes the whole case tractable.

### 1.3 Imprinting onto both faces

Each overlap boundary loop is imprinted onto **both** F_A and F_B (the curves lie identically in both, being on the shared carrier):

- F_A splits into `F_A_on` (fragments inside O, coincident with B's surface) and `F_A_off` (fragments outside O).
- F_B splits into `F_B_on` (inside O) and `F_B_off` (outside O).

`F_A_on` and `F_B_on` are *geometrically identical face fragments* sitting on the same surface patch. They differ only in which body they came from and (possibly) in orientation. The classification and keep/drop logic of Section 2 operates on exactly this pair.

### 1.4 Why the winding number is ambiguous *on* the coincidence, and how to resolve it

Keel classifies using a winding-number vector: for a sample point p and each input body B_i, w_i(p) is the (generalized) winding number of B_i's boundary about p. A cell is "inside B_i" iff w_i is nonzero (for closed orientable input, w_i in {0,1}). Boolean membership is a per-cell predicate on the vector (w_1, ..., w_n).

**The ambiguity.** On the coincident overlap O, a representative point p of `F_A_on` lies *exactly on* the boundary of B. The winding number of B about a point on B's own boundary is undefined (the integral has a jump discontinuity there; formally it is the average of the two side limits, e.g. 1/2, which is not a clean inside/outside answer). So evaluating w_B at a point of the coincident face gives a degenerate value. This is the winding-number expression of Requicha's classical observation that an on-on point cannot be classified by the in/out test alone; it requires a *neighborhood* test.

**The resolution: two-sided neighborhood evaluation along the face normal.** Pick the representative point p on `F_A_on` and the *outward normal* n of F_A there. Evaluate the winding-number vector at the two perturbed points

```
p_plus  = p + eps * n      (just outside A across this face)
p_minus = p - eps * n      (just inside A across this face)
```

with eps a few multiples of the local tolerance, small enough not to cross any other feature. Now w_B(p_plus) and w_B(p_minus) are *well-defined* (the points are off B's boundary). The pair (membership at p_minus, membership at p_plus) is exactly the **transition** that classical neighborhood classification records: it tells you whether crossing F_A's surface at this overlap takes you from inside-B to outside-B or vice versa, and on which side of F_A the material of A lies. This two-sided test is the winding-number-vector instantiation of Mantyla's `nbr` (neighborhood) classification and Requicha's neighborhood model. Section 2 then reduces to: given the per-side membership in B and the relative orientation of F_A and F_B, decide keep/drop/reverse for the operation.

**Connecting representation to classification.** In the winding-number-vector view, the per-cell vector already encodes "inside body i" for every cell *off* the coincidence. The only thing the two-sided test adds is the membership for the two cells immediately adjacent to a coincident face, which the plain winding number cannot supply because the face lies on the discontinuity. So coincident-face handling is a *local patch* to winding-number classification: everywhere except on shared boundaries the winding-number vector is sufficient; on shared boundaries you substitute the two-sided neighborhood test. No global neighborhood walk is needed (the EMBER/arrangement-free property is preserved).

---

## Section 2. The Keep/Drop + Orientation Rules per Operation (Centerpiece)

This section reproduces the canonical on-on neighborhood classification and presents it as directly-encodable tables. The orientation convention throughout: **face normals point outward (away from solid material)**; a kept face's normal must point out of the result solid.

### 2.1 The classification of an A-fragment relative to B

Every fragment of A's boundary is classified into one of these mutually exclusive categories (Laidlaw-Trumbore-Hughes terminology, equivalent to Requicha's set-membership ON-subcategories):

| Category | Meaning |
|---|---|
| **A_outB** | A-fragment lies strictly *outside* solid B (w_B = 0 on it). |
| **A_inB** | A-fragment lies strictly *inside* solid B (w_B != 0 on it). |
| **A_onB+** | A-fragment is *coincident* with a B-fragment, normals **agree** (same-sense). |
| **A_onB-** | A-fragment is *coincident* with a B-fragment, normals **oppose** (opposite-sense). |

Symmetric categories B_outA, B_inA, B_onA+, B_onA- apply to B's fragments. The `+/-` is determined by the dot product of the two coincident faces' outward normals on O: positive => same-sense (`+`), negative => opposite-sense (`-`). The in/out categories are read directly off the winding-number vector; the on categories come from the two-sided test of Section 1.4 plus the normal-agreement check.

### 2.2 Laidlaw-Trumbore-Hughes selection rules (the canonical explicit source)

Laidlaw, Trumbore & Hughes (1986) give the boolean result as a glued union of selected fragment classes. Reproduced exactly (the `(*)` is the gluing/assembly of fragment sets; superscript `-1` means **reverse all face orientations** of that set):

```
A UNION B      =  A_outB  (*)  B_outA  (*)  A_onB+
A INTERSECT B  =  A_inB   (*)  B_inA   (*)  A_onB+
A DIFF B (A-B) =  A_outB  (*)  (B_inA)^-1  (*)  A_onB-
```

Reading these:

- **Union** keeps the outside-other parts of both bodies, and keeps **one** copy of each same-sense coincident overlap (A_onB+, the duplicate B_onA+ is dropped). Opposite-sense overlaps (A_onB-) appear in neither selected set => they **cancel** (interior wall between two abutting solids, correctly removed).
- **Intersection** keeps the inside-other parts of both bodies, and keeps **one** copy of each same-sense overlap. Opposite-sense overlaps cancel again.
- **Difference A-B** keeps A's outside-B part, keeps B's inside-A part **with reversed normals** (B's inner wall becomes the new outward boundary of the cavity), and keeps the **opposite-sense** coincident overlap (A_onB-) because where A and B abut with opposing normals, that wall survives as part of A-B's boundary; the same-sense overlap (A_onB+) is dropped because there A and B agree and B eats that face away.

### 2.3 Directly-encodable per-fragment decision tables

The formulas above, rewritten as a per-fragment switch Keel can encode. For an **A-fragment** of class C, and for a **B-fragment** of class C, the action is `KEEP` (emit as-is), `KEEP_REV` (emit with reversed normal), or `DROP`.

**UNION (A ∪ B):**

| Fragment class | A-fragment action | B-fragment action |
|---|---|---|
| outside other (out)      | KEEP      | KEEP      |
| inside other (in)        | DROP      | DROP      |
| coincident same-sense (on+) | KEEP (one copy) | DROP (duplicate) |
| coincident opposite-sense (on-) | DROP | DROP |

**INTERSECTION (A ∩ B):**

| Fragment class | A-fragment action | B-fragment action |
|---|---|---|
| outside other (out)      | DROP      | DROP      |
| inside other (in)        | KEEP      | KEEP      |
| coincident same-sense (on+) | KEEP (one copy) | DROP (duplicate) |
| coincident opposite-sense (on-) | DROP | DROP |

**DIFFERENCE (A − B):**

| Fragment class | A-fragment action | B-fragment action |
|---|---|---|
| outside other (out)      | KEEP      | DROP      |
| inside other (in)        | DROP      | KEEP_REV  |
| coincident same-sense (on+) | DROP | DROP |
| coincident opposite-sense (on-) | KEEP (one copy, from A) | DROP (duplicate) |

**Tie-break for "one copy" of coincident fragments.** When a coincident overlap must contribute exactly one face to the result (on+ in union/intersection; on- in difference), keep the **A-fragment** by convention and drop the geometrically identical B-fragment. The kept face's orientation is A's native orientation for union/intersection; for difference's on- case A's native orientation is already correct (it is the wall facing out of A-B). If only B carries the overlap (e.g. B-B-A configurations in n-ary), keep B's. Determinism requires a stable rule: keep the fragment from the **lower-indexed body**, which generalizes cleanly to n-ary.

### 2.4 Requicha / Tilove set-membership ("on") treatment

Tilove (1980) formalized boolean evaluation as **set membership classification (SMC)**: classify each candidate boundary piece against the other operand as IN, OUT, or ON. Requicha & Voelcker (1985) gave the boundary-evaluation and merging algorithm used in PADL, where the IN/OUT classification is insufficient for ON pieces, so each ON piece carries a **neighborhood** N(p) (the local solid/non-solid structure around p). The boolean of two solids reduces, on ON-ON pieces, to a boolean of their neighborhoods:

```
membership_of_p_in (A op B)  =  N_A(p)  op  N_B(p)
```

evaluated locally. For a coincident face, N_A and N_B are each a half-space (material on one side of the shared surface). Same-sense means both half-spaces are on the **same** side => their intersection is that half-space (so on+ survives intersection and union as one face) and their union is also that half-space. Opposite-sense means the half-spaces are on **opposite** sides => union fills the whole neighborhood (face is interior, drop it) while difference leaves exactly the wall (on- survives difference). This is identical in outcome to the LTH tables above; it is the principled derivation of them. Keel's two-sided winding-number test computes exactly N_A(p) `op` N_B(p) numerically: the (p_minus, p_plus) membership pair *is* the neighborhood.

### 2.5 Mantyla boundary-evaluation neighborhood (`nbr`) classification

Mantyla ("An Introduction to Solid Modeling", 1988) implements the same idea concretely in a half-edge B-rep: boundary evaluation generates *tentative* faces (the imprinted fragments), then a `nbr` routine examines each fragment's 3D neighborhood and classifies the transition as solid->solid, solid->void, void->solid, or void->void, deciding keep/drop per operation. Mantyla's neighborhood is examined by sampling just inside and just outside the candidate face, which is exactly the eps-offset along the normal of Section 1.4. Keel inherits the mechanism but replaces the explicit topological neighborhood walk with the winding-number-vector two-sided evaluation, which is more robust (no reliance on consistent local topology, tolerant of dirty input) and yields n-ary booleans for free.

---

## Section 3. Tangent Faces (Touch Without Crossing)

### 3.1 What tangency is and how to detect it

A tangency is a place where two surfaces *touch but do not transversally cross*: a sphere resting on a plane (point contact), two equal-or-unequal cylinders touching along a line (line contact), a fillet meeting its supporting face along its tangent edge (curve contact). The defining signature, from the SSI literature (Sederberg's collinear-normal points; loop/singularity detection, `docs/research/.../SSI`):

- **Parallel/collinear surface normals at the contact.** At a tangency the two surfaces' normals are parallel (dot product = +-1), so the SSI marching direction `n1 x n2` vanishes. The SSI Jacobian becomes ill-conditioned (near singular).
- **Non-transversal (even-order) contact.** Transversal crossings change the *sign* of the signed distance between surfaces as you pass through; a tangency has the signed-distance function touch zero with **even-order** contact (it does not change sign). Detecting even-order contact (the signed gap has a local extremum of value ~0 rather than a sign change) distinguishes a true tangency from a grazing crossing.

Practically: run SSI; where it reports a point/curve at which the normals are collinear and the signed-gap does not change sign across the contact, flag it as a tangency rather than a transversal intersection. Tie to the normal-cone / loop-detection work already in the SSI dossier: a normal-cone test cheaply rules out tangencies over a patch (if the two patches' normal cones do not contain antipodal/parallel directions, no tangency is possible), and collinear-normal-point solving locates the tangency when one is possible.

### 3.2 When a tangency edge is real vs spurious

The rule: **a tangency curve becomes a real edge of the result only if it actually separates a kept region from a dropped region; otherwise it is spurious and must not be imprinted.**

- **Point/line tangency that does not bound any material transition (the surfaces touch but the solids stay on the same side):** spurious. Example: sphere sitting on a plane, *united*. The point of contact is interior to neither result face boundary and separates nothing; imprinting a vertex there would create a non-manifold pinch point with no modeling value. Drop it.
- **Tangency along which material genuinely changes (a fillet tangent to its base face, where on one side is the fillet face and the other the base face):** real. It is a smooth (G1) edge of the result and must be kept (it is the tangent edge between the two faces). This is the normal designed outcome and Keel must preserve it.
- **Tangency that is the *limit* of an overlap region** (two coaxial cylinders of equal radius touch along a line only in the degenerate equal-radius coincident case, otherwise tangent unequal cylinders touch along a line): if equal-radius, it is the coincident-overlap case (Section 5), not a tangency; handle by Section 1/2. If unequal-radius tangent, the line is a real edge only if it bounds a kept/dropped transition.

### 3.3 How tangency interacts with each operation

| Configuration | Union | Intersection | Difference |
|---|---|---|---|
| Sphere tangent to plane at a point | Point contact, no real edge; result is the two solids touching at a non-manifold point (often disallowed; Keel should produce a non-manifold vertex only if non-manifold output is requested, else reject/separate). | Intersection is a single point (degenerate, empty-interior). Typically returns empty solid plus a witness point; usually reported as "no volumetric intersection." | A − B unaffected away from the point; the point is not a real edge. |
| Two cylinders tangent along a line | Line contact, no material crossing => generally no edge in union unless it bounds a face split. | Empty or measure-zero intersection along the line. | Subtraction unaffected; tangent line is not an edge unless it splits a face. |
| Fillet tangent to supporting face along a curve | Tangent (G1) edge kept; the two faces meet smoothly. | n/a (same body usually). | Kept. |

The governing principle is the same two-sided neighborhood test: evaluate membership an eps to each side of the tangency *along the contact's bisector*. If both sides have the same boolean membership, the tangency separates nothing => spurious. If they differ, it is a real edge.

### 3.4 The robustness pitfall: near-tangency

The dangerous case is a configuration that is numerically *between* tangent and transversally crossing: two surfaces that almost-but-not-quite touch, or just-barely cross. The signed gap is within tolerance of zero but its sign behavior is uncertain, so the algorithm cannot reliably decide "touch" vs "cross." Consequences: SSI may produce a tiny spurious loop, a sliver face, or an inconsistent in/out classification that flips with floating-point noise. Mitigations Keel should adopt:

- **Snap to exact tangency when within tolerance.** If the minimum gap is below tolerance and normals are collinear within tolerance, *treat it as exactly tangent* (the preserve-design-intent stance, Section 4): replace the near-tangency with a clean tangent edge or vertex rather than letting SSI invent a crossing. This is the analog of ACIS's "prepare phase" that makes near-coincident faces exactly coincident before booleaning.
- **Interval / validated arithmetic at the contact** to bound the sign of the gap; if the interval straddles zero, escalate to the tolerant path (`kernel/29`, `kernel/37`) rather than committing to a brittle decision.
- **Reject sliver loops below the feature-size floor.** A tangency-induced loop smaller than tolerance is almost always spurious; suppress it.

---

## Section 4. How Production Kernels Structure This

### 4.1 OCCT BOPAlgo and the gluing mode

**Citation.** Open CASCADE Technology, *Boolean Operations* user guide and Reference Manual (`BOPAlgo_Builder`, `BOPAlgo_PaveFiller`, `BOPAlgo_GlueEnum`). https://github.com/Open-Cascade-SAS/OCCT/wiki/boolean_operations and dev.opencascade.org reference manual.

**Mechanism.** OCCT's boolean component is layered on a **General Fuse Algorithm (GFA)**, `BOPAlgo_Builder`. Two parts: the **Intersection Part** (`BOPAlgo_PaveFiller`) computes all pairwise interferences in strict dimension order, vertex/vertex, vertex/edge, edge/edge, vertex/face, edge/face, **face/face**, then non-BRep (solid) interferences; lower-dimensional interferences are resolved first so higher-level computations are not redundant. Face/face produces section curves and points; "pave blocks" (segments between pave vertices on curves) become split edges. The **Building Part** then assembles results bottom-up: vertices -> edges -> wires -> faces -> shells -> solids. Crucially, OCCT explicitly detects **Same-Domain Faces (SDF)**: split-face pairs that share geometry are found via an `AreFacesSameDomain` test, grouped into connectivity chains, and **each chain yields a single unified face**. This is OCCT's coincident-face resolution: overlapping faces from the two arguments are merged into one shared face, which is exactly the "keep one copy" rule of Section 2.

**The gluing mode (`BOPAlgo_GlueEnum`).** A performance/robustness option set via `SetGlue()`:

- `BOPAlgo_GlueOff` (default): full intersection of all pairs; detects both partial and full coincidence the slow, general way.
- `BOPAlgo_GlueShift`: glue option for shapes with **partial coincidence** (shifted/abutting shapes that touch along boundaries but have **no real interior face/face intersections**). It tells the algorithm the coincident sub-shapes are coincident, so it **skips the face/face intersection** for them and treats them as shared. The OCCT docs report this saves about **90%** of the operation time when there are no real intersections.
- `BOPAlgo_GlueFull`: glue option for shapes with **full coincidence**: all coincident sub-shapes are assumed fully shared, so face/face (and curve/surface) intersection is bypassed entirely for matching faces.

**The point for Keel.** Gluing is a *fast path for known-coincident input*. When the caller knows (or Keel detects up front) that certain faces are coincident, you do not run SSI on them at all; you declare them shared and go straight to the keep/drop logic. This is the production answer to "coincident faces are design intent": OCCT does not perturb them away, it recognizes them and shares them. **Limitation:** gluing assumes the coincidence is exact (or pre-snapped); if you set GlueFull on shapes that are only near-coincident, you get wrong topology. So gluing pairs naturally with a *coincidence-detection / pre-snap* step. **Kernel relevance:** Keel should expose an equivalent "known-coincident" hint and an internal gluing fast path that, on flagged or detected coincident faces, skips SSI, computes the 2D overlap (Section 1.2), and dispatches to the Section 2 tables.

### 4.2 Parasolid: tolerant booleans and coincident faces

**Citation.** Siemens Parasolid Functional Description, *Boolean Operations* chapter; Parasolid tolerant-modeling documentation. (Parasolid v12/v35 docs, q-solid mirror and Siemens.)

**Mechanism.** Parasolid uses **tolerant modeling**: each vertex, edge, and face carries its own local tolerance (an edge is a "tube" of its tolerance radius, a vertex a "sphere"), rather than a single global epsilon. The boolean algorithm performs intersection and **coincidence tests to the appropriate local tolerance**, and represents the result with this richer structure. For coincident-face configurations, Parasolid lets the caller declare **matched regions**: "a region of overlap between two topological entities that are geometrically coincident to a specified tolerance," with match type **exact** or **overlapping**. Declaring matched regions makes the boolean both more reliable and faster (it is Parasolid's gluing analog: tell the algorithm the faces coincide instead of making it discover it). Parasolid additionally **merges coincident geometry** in the result where feasible (e.g. two half-cylinders unified into one cylinder), the curved analog of OCCT's SDF unification.

**Limitation/relevance.** Parasolid's documented internals are thin (proprietary), but the externally visible contract is clear and worth copying: local per-entity tolerances + a caller-supplied matched-region hint + result-side geometry merging. Keel's tolerant-edge work (M7b) already provides the local-tolerance substrate; the matched-region hint is the same "known-coincident fast path" as OCCT gluing.

### 4.3 ACIS: the face-face intersection graph and the prepare phase

**Citation.** Spatial/ACIS Kernel documentation, *Tolerant Modeling* (Chapter 6); Spatial blog "What to do when your Boolean Operations Fail"; US Patent 5,649,084 (graph-theoretic boolean method).

**Mechanism.** ACIS builds booleans on a **graph-theoretic** core: take a face from each body, intersect them, and "generate a graph edge from the pieces of the intersection track(s) inside both faces"; the union of all such graph edges over all face/face pairs is the **intersection graph**. Classification then traverses this graph to label pieces inside/outside/on for the operation. For coincident faces ACIS faces the same fundamental ambiguity Keel does: "the Boolean API cannot determine whether two near-coincident faces are actually coincident (no intersection to compute) or not (an intersection must be computed)." ACIS resolves it with an **incremental boolean workflow** that, on failure, runs a **prepare phase** to *make the two near-coincident faces exactly coincident* before re-running the boolean. Spatial reports ~70% of failed booleans are fixed by this prepare step, usually in a single iteration. ACIS uses `SPAresabs` plus per-entity tolerances for coincidence tests, and supports booleans where a cylindrical face is coincident or tangent to a cylindrical/planar face (with limitations).

**Relevance.** Two transferable ideas: (1) the **prepare/snap phase** that converts near-coincidence to exact coincidence *before* classification (do not ask SSI to resolve a near-degenerate face/face pair, snap it first), and (2) the explicit acknowledgment that coincident vs not-coincident is undecidable from geometry alone at tolerance scale, so it must be a *modeling decision* (snap or do not), reinforcing the preserve-coincidence stance.

### 4.4 The symbolic-perturbation alternative and the CAD caveat

**Citation.** Edelsbrunner & Mucke, "Simulation of Simplicity: a technique to cope with degenerate cases in geometric algorithms," *ACM Trans. Graph.* 9(1):66-104, 1990. Plus the exact-arithmetic mesh-boolean line: Zhou, Grinspun, Zorin & Jacobson, "Mesh Arrangements for Solid Geometry," *ACM Trans. Graph.* 35(4), 2016; Cherchi, Pellacini, Attene & Livesu, "Interactive and Robust Mesh Booleans," *ACM Trans. Graph.* 2022.

**Mechanism (SoS).** Simulation of Simplicity makes degeneracies *disappear* by perturbing every input coordinate by a distinct symbolic infinitesimal: coordinate i,j becomes x_ij + eps^(2^(i*n+j)) or similar, an indeterminate eps assumed positive and arbitrarily small, never numerically applied. The perturbation is engineered so that **no two points are ever exactly coincident, no three collinear, no four coplanar**: every geometric predicate that was zero (degenerate) now has a definite, consistent sign decided by the lowest-order eps term. The result: the algorithm never sees a degenerate case, so coincident/coplanar/tangent configurations are resolved *as if* they were a slightly-perturbed generic configuration. The exact-arithmetic mesh booleans (Zhou; Cherchi/Attene) combine exact predicates with SoS-style consistent tie-breaking: coplanar overlapping triangles are handled by a consistent symbolic rule that keeps a single canonical representative, and inside/outside is assigned by the winding-number vector per arrangement cell.

**The crucial CAD caveat.** SoS's whole premise is "perturb the coincidence away." In **CAD, a coincident face is usually design intent**, a flush mating face, a shared wall between two parts, a press-fit. Perturbing it away produces a *wrong* model: a hairline gap or a sliver overlap where the designer intended exact contact, plus spurious micro-faces and edges. The mesh-boolean community can perturb freely because their inputs are not semantically required to stay coincident; a CAD kernel cannot. **Contrast:** SoS = "make coincidences vanish so the generic algorithm always applies"; CAD/Keel = "**recognize and preserve** exact coincidence as a first-class case, share the face, and classify with the neighborhood test." 

**Verdict for Keel: preserve, do not perturb.** Keel must treat exact coincidence as design intent and preserve it (share the face via the Section 2 tables), not symbolically perturb it away. SoS-style consistent *tie-breaking* is still useful as a determinism device for choosing *which* coincident copy survives and for breaking ties in the 2D overlap arrangement (Section 2.3's "lowest-indexed body" rule is exactly such a consistent symbolic tie-break). But the global "perturb everything apart" philosophy is rejected: it conflicts with CAD's contract that mating faces stay mated.

### 4.5 Zhou et al. winding-number-vector model (the representation Keel uses)

**Citation.** Zhou, Grinspun, Zorin & Jacobson 2016 (above).

**Mechanism.** Resolve all self-intersections into an arrangement of cells; assign each cell a **winding-number vector** (w_1, ..., w_n), one component per input. Each boolean is an *extraction*: a per-cell predicate on the vector. Union = cells where any w_i != 0; intersection = all w_i != 0; A−B = w_A != 0 and w_B = 0; "inside at least k of n" = at least k nonzero (free n-ary). Output faces are the arrangement facets separating a kept cell from a dropped cell, oriented by the winding-number gradient. They use the **generalized** winding number so non-watertight ("PWN", piecewise-constant winding number) input still works. Coincident/duplicate facets are resolved by keeping one canonical representative.

**Relevance.** This is Keel's classification engine. The only gap, the reason this dossier exists, is that the plain winding number is undefined *on* a shared boundary (Section 1.4); Keel patches that with the two-sided neighborhood test and the Section 2 tables, and (unlike Zhou's global arrangement) imprints only locally along intersection/overlap boundaries (the EMBER lesson).

---

## Section 5. Generalization from Coplanar Planes to Coincident Curved Surfaces

The coplanar machinery (Sections 1-2) is carrier-agnostic in structure: recognize a shared carrier, compute the overlap in the carrier's parameter space, determine same/opposite sense, dispatch to the tables. What changes per surface type is (a) the **coincidence test** and (b) the **parameter space** for the overlap.

### 5.1 Coincidence test and overlap, per analytic type

Coincidence detection is tractable precisely because the surfaces are *canonical* (analytic), which is what `kernel/24` (canonical recovery) buys you: recognizing that two faces share an analytic carrier turns 3D coincidence into a finite parameter comparison.

| Surface type | Coincidence test (within tol) | Overlap parameter space | Same/opposite sense |
|---|---|---|---|
| **Plane** | Same unit normal (up to sign) and same signed offset d. | (u,v) in the plane; 2D loop boolean of trims. | sign of n_A . n_B. |
| **Cylinder** | Coaxial (axis lines coincide: same direction up to sign, same axis point within tol) **and equal radius**. | (theta, z) unrolled; 2D loop boolean. | Surface normal points radially out; sense = agreement of radial-outward directions = sign of (axis-direction agreement) combined with parameter orientation. Equivalently sign of n_A . n_B on the overlap. |
| **Sphere** | Concentric (centers within tol) **and equal radius**. | (phi, lambda) spherical params; 2D loop boolean. | sign of n_A . n_B (radial out vs in). |
| **Cone** | Same apex, same axis, **same half-angle** (and same nappe). | (theta, v) along-axis param; 2D loop boolean. | sign of n_A . n_B. |
| **Torus** | Same axis, same center, **equal major and minor radii**. | (u,v) the two angular params; 2D loop boolean. | sign of n_A . n_B. |

The pattern is uniform: an analytic coincidence test (a handful of scalar/vector comparisons within tolerance), then a **2D trim-loop boolean** in the natural parameter domain to get the overlap region, then a **dot-product of outward normals** on the overlap to set the same/opposite sense bit, then the Section 2 tables. Watch the parameterization seams (cylinder/cone theta wrap at 2pi, sphere poles); the 2D boolean must be done on the periodic domain or on an unrolled covering that handles the seam.

### 5.2 The NURBS case

For coincident *trimmed NURBS* faces there is no closed-form coincidence test; it is the hard end of the ladder:

1. **Cheap reject by canonical recognition first.** Before any NURBS-vs-NURBS work, run canonical recovery (`kernel/24`): if both faces' carriers are recognized as the *same analytic surface*, drop into the analytic path of 5.1. Most real coincident faces in CAD are analytic (planar mating faces, coaxial holes), so this disposes of the common cases cheaply and exactly.
2. **Sampling-based coincidence test.** If both remain genuinely free-form, sample a grid of points on the overlapping parameter region of F_A, project each onto F_B's surface, and check the max deviation against tolerance; also check normal agreement at the samples (for the sense bit and to reject anti-coincident sheets). Cheap, catches most non-coincidence quickly, but sampling can miss a localized deviation.
3. **Control-net comparison after common refinement.** For a rigorous within-tolerance verdict, bring both NURBS to a **common knot vector / common refinement** (knot insertion so both share the same basis), then compare corresponding control points. If every corresponding control point pair is within tolerance (and weights match within tolerance for rationals), the surfaces coincide on the common domain to a bound derived from the convex-hull property. This is the NURBS analog of "same plane equation."
4. **Overlap-region boundary computation (the genuinely hard part).** Unlike analytic surfaces, the two trimmed NURBS domains are related by an unknown reparameterization, so the overlap boundary is not a clean 2D loop boolean in one shared domain. You must establish the parameter correspondence (from the common refinement of step 3, or by projecting one trim loop onto the other surface and pulling back), then intersect the trim loops in that correspondence. This is error-prone and expensive.
5. **When to give up: the tolerant path.** If common refinement is unstable, the correspondence is ill-conditioned, or the deviation is borderline (near-coincident but not within tolerance), **do not force an exact coincident-overlap result.** Fall back to the tolerant path (`kernel/29`, `kernel/30`): either (a) snap the two faces to an exact shared canonical/refined surface first (ACIS-style prepare phase, Section 4.3) and then treat as coincident, or (b) treat them as near-coincident and let the tolerant stitch absorb the mismatch into tolerant edges, accepting a tolerant (not exact) shared boundary. The choice is governed by whether the input declares the faces as design-intent-coincident (snap) or merely happens to be close (tolerant).

### 5.3 The coincidence-detection ladder (summary)

```
1. Canonical recovery: do the two faces share a recognized analytic carrier?
     yes -> analytic coincidence test (Sec 5.1) -> exact overlap -> Sec 2 tables.
2. Else NURBS sampling test: grossly coincident within tol?
     no  -> not coincident; proceed with ordinary SSI (transversal) or tangency (Sec 3).
3. Else control-net compare after common refinement: rigorously coincident within tol?
     yes -> overlap via parameter correspondence -> Sec 2 tables.
4. Else (borderline / ill-conditioned) -> tolerant path:
     design-intent-coincident? snap to shared surface (prepare phase) then treat as coincident.
     merely near?              treat as near-coincident, tolerant stitch.
```

---

## Coincident and Tangent Handling for Keel (Synthesis)

1. **Imprint-overlap procedure.** Do not run 3D SSI on coincident faces. Recognize the shared carrier (canonical recovery first, then NURBS tests), map both faces' trims into the carrier's parameter space, compute the overlap region O as a **2D trim-loop boolean** in that domain, lift O's boundary to 3D, and imprint it onto **both** faces, splitting each into on-overlap and off-overlap fragments.

2. **Winding-number two-sided resolution.** The winding number is undefined exactly on a shared boundary, so for each on-overlap fragment evaluate the winding-number vector at `p +- eps*n` along the face normal. The (inside, outside) membership pair is the neighborhood classification (Requicha N, Mantyla `nbr`); combined with the same/opposite-sense bit (sign of n_A . n_B on O) it selects the fragment's fate. Everywhere off coincidences, the plain winding-number vector suffices; coincidence handling is a *local patch*, preserving the arrangement-free (EMBER) property and n-ary-for-free property.

3. **Encoded classification tables.** Encode Section 2.3 verbatim. The canonical formulas (Laidlaw-Trumbore-Hughes), as the ground truth:
   - `A ∪ B = A_outB ∪ B_outA ∪ A_onB+`
   - `A ∩ B = A_inB ∪ B_inA ∪ A_onB+`
   - `A − B = A_outB ∪ (B_inA)^reversed ∪ A_onB-`
   Same-sense coincident overlap survives **union and intersection** (kept once, duplicate dropped); opposite-sense coincident overlap survives **difference** (kept once). Tie-break "which copy" by lowest-indexed body for determinism.

4. **Preserve coincidence, do not perturb (verdict).** Reject the Simulation-of-Simplicity "perturb coincidences away" philosophy at the global level: in CAD a coincident face is design intent (a mating face) that must stay exactly coincident. Adopt the production-kernel stance instead, recognize and **share** the coincident face. Retain SoS-style *consistent symbolic tie-breaking* only as a determinism device (which copy survives, 2D-arrangement tie-breaks).

5. **Gluing-style fast path for known-coincident input.** Mirror OCCT `BOPAlgo_GlueShift`/`GlueFull` and Parasolid matched regions: expose a "these faces are coincident" hint (and an up-front detector). On flagged/detected coincident faces, **skip SSI entirely**, go straight to the 2D overlap + Section 2 tables. This is both a large speedup (OCCT cites ~90%) and a robustness win (avoids degenerate SSI). Pair it with an ACIS-style **prepare/snap phase** that converts near-coincidence to exact coincidence before classification.

6. **Tangency handling.** Detect tangencies in SSI by collinear normals (vanishing `n1 x n2`) plus even-order (non-sign-changing) contact of the signed gap; use normal-cone tests to cheaply rule tangency out over a patch. A tangency edge/vertex is **real iff it separates a kept region from a dropped region** (two-sided test along the contact bisector), otherwise spurious and suppressed. Keep tangent (G1) edges (fillet-to-base); drop measure-zero point/line touches that bound nothing. For near-tangency, snap-to-exact-tangent within tolerance and use validated arithmetic to bound the gap sign; suppress sub-tolerance sliver loops.

7. **Analytic-then-NURBS coincidence ladder.** Always try canonical recognition first (analytic coincidence is a finite scalar test and gives an exact overlap), fall back to NURBS sampling, then control-net comparison after common refinement, and finally to the tolerant path (snap if design-intent, tolerant-stitch if merely near). This ladder is what makes curved coincidence tractable, and it is why `kernel/24` canonical recovery is a hard dependency of robust curved booleans.

---

## References

1. Laidlaw, D. H., Trumbore, W. B., & Hughes, J. F. (1986). Constructive solid geometry for polyhedral objects. *Computer Graphics (SIGGRAPH '86)*, 20(4), 161-170. https://cs.brown.edu/people/jhughes/papers/Laidlaw-CSG-1986/main.htm
2. Requicha, A. A. G., & Voelcker, H. B. (1985). Boolean operations in solid modeling: Boundary evaluation and merging algorithms. *Proceedings of the IEEE*, 73(1), 30-44.
3. Tilove, R. B. (1980). Set membership classification: A unified approach to geometric intersection problems. *IEEE Transactions on Computers*, C-29(10), 874-883.
4. Requicha, A. A. G. (1977). *Mathematical models of rigid solid objects* (Tech. Memo 28). Production Automation Project, University of Rochester.
5. Requicha, A. A. G. (1980). Representations for rigid solids: Theory, methods, and systems. *ACM Computing Surveys*, 12(4), 437-464.
6. Mantyla, M. (1988). *An Introduction to Solid Modeling*. Computer Science Press. (Boundary evaluation / neighborhood `nbr` classification.)
7. Hoffmann, C. M. (1989). *Geometric and Solid Modeling: An Introduction*. Morgan Kaufmann. (Ch. on booleans and on-set classification.) https://www.cs.purdue.edu/homes/cmh/distribution/books/geo.html
8. Open CASCADE Technology. *Boolean Operations* user guide and Reference Manual (BOPAlgo_Builder, BOPAlgo_PaveFiller, BOPAlgo_GlueEnum). https://github.com/Open-Cascade-SAS/OCCT/wiki/boolean_operations
9. Open CASCADE Technology. BOPAlgo_GlueEnum.hxx reference (GlueOff/GlueShift/GlueFull). https://dev.opencascade.org/doc/refman/html/_b_o_p_algo___glue_enum_8hxx.html
10. Open CASCADE forum. Gluing Options in Boolean Component. https://dev.opencascade.org/index.php?q=node/1179
11. Siemens Parasolid. *Boolean Operations* (Functional Description, ch. 10). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.10.html
12. Siemens Parasolid. *Overview of Parasolid* (tolerant modeling). http://www.q-solid.com/Parasolid_Docs_V35/pdf/ov.pdf
13. Spatial Corp. (ACIS). *Tolerant Modeling* (Kernel R10, Ch. 6). http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/06TMOD.PDF
14. Spatial Corp. Boolean 3D Modeling: What to do when your Boolean Operations Fail (incremental boolean / prepare phase). https://blog.spatial.com/what-to-do-when-your-3d-modeling-boolean-operations-fail
15. US Patent 5,649,084. Method for performing Boolean operations on geometric objects in a computer-aided design system (graph-theoretic face-face intersection). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/5649084
16. Edelsbrunner, H., & Mucke, E. P. (1990). Simulation of Simplicity: A technique to cope with degenerate cases in geometric algorithms. *ACM Transactions on Graphics*, 9(1), 66-104. https://www.sandia.gov/files/samitch/unm_math_579/p66_edelsbrunner_simulation_of_simplicity.pdf
17. Edelsbrunner, H., & Mucke, E. P. (1994/arXiv). Simulation of simplicity (arXiv reprint). https://arxiv.org/pdf/math/9410209
18. Zhou, Q., Grinspun, E., Zorin, D., & Jacobson, A. (2016). Mesh arrangements for solid geometry. *ACM Transactions on Graphics*, 35(4), 39. https://www.cs.columbia.edu/cg/mesh-arrangements/
19. Cherchi, G., Pellacini, F., Attene, M., & Livesu, M. (2022). Interactive and robust mesh booleans. *ACM Transactions on Graphics*, 41(6). https://arxiv.org/pdf/2205.14151
20. Cherchi, G., Livesu, M., Scateni, R., & Attene, M. (2020). Fast and robust mesh arrangements using floating-point arithmetic. https://www.gianmarcocherchi.com/pdf/mesh_arrangement.pdf
21. Sederberg, T. W., et al. Loop and singularity detection in surface intersection (collinear normal points). Detection of loops and singularities of surface intersections, *Computer-Aided Design*. https://www.sciencedirect.com/science/article/abs/pii/S0010448598000566
22. Analysis Situs. On recognizing canonical surfaces: cylinders (canonical surface recognition for coincidence). https://analysis-situs.medium.com/on-recognizing-canonical-surfaces-cylinders-9e0a5be2831e
23. Rossignac, J. Solid and Physical Modeling (boundary evaluation, neighborhood classification survey). https://faculty.cc.gatech.edu/~jarek/papers/SPM.pdf
24. Indira (Cornell ECE CS490). CSG Theory and Implementation (reproduces Laidlaw AonB+/AonB- selection formulas). https://people.ece.cornell.edu/land/OldStudentProjects/cs490-96to97/INDIRA/CSG/theory.html
```
