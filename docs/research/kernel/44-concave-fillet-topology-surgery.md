# 44. Concave (Reentrant) Fillet Topology Surgery and the Euler-Operator Sequence

**Scope.** This dossier treats one narrow, load-bearing problem: the *local B-rep topology surgery* that inserts a constant-radius edge fillet, and specifically the difference between a **convex** edge (a round, material removed) and a **concave / reentrant** edge (a fillet, material added). It is the topology companion to `40-blend-fillet-surface-generation.md` (spine = intersection of the two offset support surfaces; exact-analytic catalog: line spine to cylinder, circle spine to torus) and `41-blend-overflow-radius-feature-failures.md` (overflow / runout / radius-versus-feature failure modes). Surface geometry and failure modes are **solved there and not re-derived here.** The operator vocabulary is the GWB / Mantyla set established in `01-topology-euler-operators.md`: `mvfs` / `kvfs`, `mev` / `kev`, `mef` / `kef`, `kemr` / `mekr`, `kfmrh` / `mfkrh`, plus the two primitive subdivision helpers `split_edge` (insert a vertex on an edge) and `split_face` (insert an edge across a face, the `mef` realization). Topology is the radial-edge / partial-entity-structure (PES) non-manifold B-rep of `02-nonmanifold-cellular.md`, with fin = coedge = half-edge = oriented edge-use.

A note on terminology used throughout (consistent with files 28, 40, 41):
- **spine**: locus of the rolling-ball center, the offset-surface intersection curve.
- **spring curves** (= spring edges, trimlines, contact curves): the two loci where the ball touches each support face; they bound the blend ribbon and become the two tangent boundaries of the inserted blend face.
- **cross curves / cross edges**: the section-end boundaries of the blend ribbon (the transverse ends), where the blend meets a **cap** / transverse face.
- **support faces**: the two faces meeting at the blended edge.
- **cap face / transverse face**: a face transverse to the spine that bounds the end of the blend ribbon. On a box-like degree-3 corner the cap face is the third face of the corner.
- V, E, F, L, S = vertices, edges, faces, loops, shells. The invariant asserted after every step is `V - E + F = 2(S - G) + (L - F)` (file 01, Section 3), with R = L - F the ring count.

---

## The convex baseline (stated precisely, as the thing the concave answer must fit)

The convex case works end to end as a purely local surgery (file 28 Section 9; file 40). For a single convex prismatic edge `e` shared by support faces `F1`, `F2`, ending at transverse cap faces `Fc` (e.g. the two end faces of a box-like corner):

1. **Imprint each spring curve onto its support.** For each support `F_i`: `split_edge` at the point where the spring curve crosses each bounding edge of `F_i` (introducing the spring-curve endpoints as new vertices), then `split_face` (`mef`) along the spring curve, dividing `F_i` into a keep-strip (toward the far side) and a trim-strip (the sliver between spring curve and the old sharp edge `e`).
2. **Trim the supports back to the spring curves.** The trim-strip is removed: `F_i` now ends at its spring curve.
3. **Delete the sharp edge.** `e` (and its two vertices, once spurred) is removed with `kef` / `kev`, opening the gap between the two spring curves.
4. **Cap-corner dissolve (the convex move).** On each transverse cap face `Fc`, the blend's circular end-arc lies **inside** `Fc` (the round eats into the solid). The small corner triangle of `Fc` bounded by the old sharp corner and the new arc is **split off and dissolved into the blend**: `split_face` off the triangle, then `kef` (kill-edge-merge-faces) to merge it into the growing blend face, then `kev` to remove the now-isolated spur vertex/edge. The cap face **shrinks**.
5. **Attach the exact analytic blend surface** (file 40) to the new face bounded by the two spring curves and the two end-arcs.

Net for the convex degree-3 corner: the support faces lose their trim-strips, the cap faces lose their corner triangles (cap-shrink), one new blend face is inserted, Euler-Poincare is preserved at every `split_*` / `kef` / `kev` step. The signature convex move is **cap-shrink by split-off-triangle + kef + kev dissolve.**

**The precise gap.** For a concave (reentrant) edge the fillet is a **material addition**: the bead bulges into the notch. On each transverse cap face the blend's end-arc lies **outside** the original cap face (in the void of the notch), so there is **no corner triangle inside `Fc` to dissolve.** The reentrant corner vertex of `Fc` must instead become **interior**, and `Fc`'s boundary loop must **extend outward** to the arc, *adding* the sliver region between the old sharp corner and the arc. The convex split-off-and-dissolve treatment is geometrically wrong here: applied blindly it yields a valid solid with the **wrong volume** (it removes material where material should be added). The cap must **grow**, not shrink. The five questions below give the canonical grow-to-arc surgery, confirm it against the literature, and recommend a concrete Keel sequence.

The framing the team asked us to test against the literature is **confirmed, not merely asserted**: Parasolid states it verbatim, *"A blend on a convex edge removes material from the model, while a blend on a concave edge adds material to the model"* (q-solid Functional Description, PK_BLENDSF), and OCCT classifies the two cases by an orientation test (`ChFi3d::ConcaveSide`, below). The convex round is locally a material-*removal* (subtract a rounding tool); the concave fillet is a material-*addition* (union a fillet bead); the concave cap-grow is the dual of the convex cap-shrink.

---

## Q1. The grow-to-arc cap operation

**Question.** In a radial-edge / half-edge / PES B-rep, what is the exact Euler operator (or sequence) that replaces a reentrant boundary-corner vertex of a face with a new outer boundary edge (the blend end-arc), making the old corner vertex interior and growing the face by the enclosed sliver region?

**Answer: there is no single named primitive for "grow a face past its corner to an external arc."** The convex dissolve had a single named realization (`kef` merges the split-off triangle into the blend) precisely because the triangle was *already inside* the face. For the concave case the added sliver is *new topology* that must be **built and spliced in**, because the arc and the sliver region do not exist in the model yet. The canonical practice (confirmed by the OCCT FilBuilder extremity handlers, the Parasolid cap-by-face-extension behavior, and the notch-blend patent US 8,935,130) is **construct-the-added-region-then-splice**, realized with the make-operators followed by kill-of-the-swallowed-corner. The convex move is *kill-dominant* (kef/kev dissolve existing topology); the concave move is *make-dominant* (mev/mef build new topology), with a single kev to absorb the swallowed corner. This is the precise sense in which concave cap-grow is the **dual** of convex cap-shrink.

### Q1 recipe (degree-3 prismatic corner, plane-plane-plane "box notch")

Setup at one transverse end of the blend ribbon. Let the cap face be `Fc` with reentrant corner vertex `v0` (the old sharp box corner). The two spring curves arrive at `Fc`'s boundary at two new vertices `vs1` (on the `F1`-side cap edge) and `vs2` (on the `F2`-side cap edge); these are created by `split_edge` on `Fc`'s two boundary edges that emanated from `v0`. The blend's circular end-arc runs from `vs1` to `vs2`, **bulging away from `Fc` into the notch void.** The sliver region to be added to `Fc` is the lune bounded by: edge `vs1->v0`, edge `v0->vs2` (the two old boundary fragments inboard of the spring endpoints), and the new arc `vs2->vs1`.

Precondition (shared with convex step 1-3): the two spring curves are already imprinted on `F1`, `F2` (via `split_edge` + `split_face`), the supports are trimmed to them, and the sharp edge `e` between the supports has been removed (`kef` + `kev`). The blend face `B` does not yet exist as a closed face; we build it and the cap-grow together.

Per-cap-end sequence (counts shown as deltas; assert the invariant after each):

| Step | Operator | What it does at this cap end | dV | dE | dF | dL | dS |
|---|---|---|---|---|---|---|---|
| a | `split_edge` x2 | Insert `vs1`, `vs2` on `Fc`'s two boundary edges out of `v0` (the spring-curve landing points). Each `split_edge` adds 1 vertex and 1 edge to the same loop. | +2 | +2 | 0 | 0 | 0 |
| b | `mev` (make-edge-vertex) | From `vs1`, draw the new arc edge `A` outward to a new vertex `va` placed at the `vs2` location-to-be (the arc is a single curved edge; introduce it as an edge+vertex spur first). | +1 | +1 | 0 | 0 | 0 |
| c | `mef` (= `split_face`) | Close the arc by drawing edge from `va` to `vs2`, splitting off the new **blend-cap region**: this `mef` makes the arc into a closed loop and creates the new face `B` (the blend ribbon end / blend face). The arc `vs1->va->vs2` plus the two spring edges plus the far cross-edge bound `B`. | 0 | +1 | +1 | 0 | 0 |
| d | `kev` (kill-edge-vertex) | Merge the artificial split `va` back so the arc is one edge `vs1->vs2` (remove the helper vertex introduced only to let `mev`/`mef` build a curved boundary in two strokes). On a kernel whose `mev` can introduce a curved edge to an *existing* target vertex in one stroke, steps b-d collapse to a single `mef` and this kev vanishes. | -1 | -1 | 0 | 0 | 0 |
| e | `kef` (kill-edge-merge-faces) | **The grow move.** Kill the two old boundary fragments `vs1->v0` and `v0->vs2` of `Fc` by merging the lune sliver into `B` (or, equivalently, into `Fc` depending on which face is the "added" one). Each fragment is a `kef` that dissolves the wall between the lune and the cap, so the lune is absorbed and `v0` is left as a **spur** (an edge-end no longer separating two faces). Two `kef` here (one per fragment) merge the lune across both old edges. | 0 | -2 | -1 | 0 | 0 |
| f | `kev` (kill-edge-vertex) | Remove the now-interior spur vertex `v0` and its dangling stub. After the two `kef` merges, `v0` is a valence-2 colinear or fully interior vertex with no separating role; `kev` absorbs it, making `v0` **interior to the grown face** exactly as required. | -1 | -1 | 0 | 0 | 0 |

Aggregate over the per-cap-end sequence a-f: dV = +2 +1 +0 -1 +0 -1 = **+1**; dE = +2 +1 +1 -1 -2 -1 = **0**; dF = 0 +0 +1 +0 -1 +0 = **0**. So one cap-end grow nets **+1 vertex, 0 edges, 0 faces** in isolation (it converts a corner into a spring-curve landing plus an interior absorption). The +1 net vertex is the pair `vs1`,`vs2` minus the absorbed `v0` (and the blend face `B` is created once and shared by both ends, so on the *second* cap end step c is a `mef` that does **not** create a new face but instead closes `B`'s loop, i.e. dF = 0 on the second end and the per-end face delta is +1 only on the first end). Across **both** cap ends plus the body of the ribbon, the whole concave insertion nets exactly **+1 face** (the single blend face `B`), the same global face delta as the convex case; the difference is entirely in the *sign of the cap area change* (grow vs shrink) and in make-dominant vs kill-dominant local steps.

**Euler-Poincare check.** Every operator above is a GWB operator with a known fixed delta (file 01 Section 3.2, Sequin): `split_edge` = (+1V, +1E); `mev` = (+1V, +1E); `mef`/`split_face` = (+1E, +1F); `kev` = (-1V, -1E); `kef` = (-1E, -1F). Each preserves `V - E + F = 2(S - G) + (L - F)` by construction because each is a manifold-preserving make/kill pair member. No `kemr`/`mekr` (ring) or `kfmrh`/`mfkrh` (handle) operator is needed for the simple box notch: the cap-grow does not create an inner ring or a handle, so S, G, R are unchanged. (A ring operator *is* needed only in the variant where the blend end-arc closes a loop that becomes an inner boundary of a face, e.g. a fillet whose bead fully encircles a boss; that is the `kemr` case, noted in Q5.)

**Why this is the dual of convex.** Convex: split off an *interior* triangle (`split_face`), `kef` to dissolve it into the blend, `kev` the spur. Concave: build an *exterior* lune (`mev` + `mef`), `kef` to merge it in, `kev` the swallowed corner. Same kef + kev tail; the head flips from "split an existing interior region" to "make a new exterior region." The corner vertex `v0` goes from being *removed at the boundary* (convex) to being *swallowed into the interior* (concave). This is the operator-level statement of cap-shrink-vs-cap-grow duality.

---

## Q2. Convex vs concave: unified or separate pipelines?

**Answer: production kernels run a single imprint / trim / stitch *pipeline* with a sign/sense branch at exactly one place: the extremity (cap) treatment, governed by an orientation invariant. The body of the surgery (spine, spring curves, support trim, sharp-edge removal, blend-face attach) is identical; only the cap step forks (shrink-dissolve vs grow-splice).** This is confirmed three ways:

1. **OCCT** carries a single `ChFi3d_Builder` with one builder per *blend kind* (FilBuilder for fillets, ChBuilder for chamfers), **not** one per convexity. Convexity is handled *inside* the common path by a classification predicate, `ChFi3d::ConcaveSide`, documented verbatim as: *"Returns Reversed in Or1 and(or) Or2 if the concave edge defined by the interior of faces F1 and F2, in the neighbourhood of their boundary E is of the edge opposite to the normal of their surface support."* The convex/concave decision is reduced to a **sign of orientation** (the edge's sense versus the support-surface normals), and that sign then drives the extremity logic (`ExtentOneCorner` / `ExtentTwoCorner` / `ExtentThreeCorner`). One pipeline, one orientation bit.

2. **Parasolid** uses one `PK_EDGE_set_blend_*` API for both; convexity is a *property of the edge*, not a different function. The functional description states the material-sign rule (*convex removes, concave adds*) as a consequence of the **offset sign**, not a separate code path: the spine is the intersection of the two radius-offset surfaces, and the offset is taken *into the material* (convex) or *into the void* (concave). The single quantity that flips is the **sign of the offset distance** carried through to the trim-vs-extend decision at the extremity.

3. **The notch-blend patent (US 8,935,130)** treats the concave (notch) case as a variant of the same blend insertion, differing in that the cap faces are **extended** (grown) rather than trimmed (shrunk), and that the blend is integrated by sewing rather than pure in-place dissolve (Q3).

**The orientation invariant, stated precisely.** Let `n_i` be the outward surface normal of support `F_i` at a point on the sharp edge `e`, and let `t` be the edge tangent oriented consistently with `F1`'s coedge (fin) sense on `e`. Define the **dihedral sense** by the sign of `(n_1 x n_2) . t` evaluated on the *solid* side. Equivalently and more robustly (this is OCCT's `ConcaveSide` test): take the edge's coedge in `F1`'s loop and ask whether the support-face interior lies on the side toward which `n_1` points across `e`. Then:

- **Convex edge** (round, material removed): the solid's dihedral angle across `e` is **< 180 degrees**; the offset of each support is taken **minus r into the material**; the blend end-arc lies **inside** the cap face; cap step = **shrink** (split-off-triangle + kef + kev dissolve).
- **Concave edge** (fillet, material added): the dihedral angle is **> 180 degrees** (reentrant); the offset is **plus r into the void**; the end-arc lies **outside** the cap face; cap step = **grow** (mev + mef build + kef + kev swallow, Q1).

The single signed value `sigma = sign((n_1 x n_2) . t)` (the **convexity sign**, computed once per blended edge, per parameter for an inflecting edge per file 41 Section 1.8) is the invariant. The spine and spring curves are *already* sign-correct for both (the offset sign flips automatically when `sigma` flips, file 40); the **only** place `sigma` must be consulted again is the extremity: `sigma > 0` -> cap-shrink-dissolve; `sigma < 0` -> cap-grow-splice. Everything between (imprint, support trim, sharp-edge kill, blend-face attach with consistent coedge sense) is identical.

**Sewing-with-consistent-coedge-sense** is the second half of the invariant: the inserted blend face `B` must be oriented so its outward normal agrees with the solid's outside on *both* spring curves. Because the spring-curve coedges are shared with the (trimmed) supports, `B`'s coedge on each spring curve must be the **twin** (opposite sense) of the support's coedge there. This is automatic in a fin/coedge structure if `B` is built by `mef` with the loop traversed so the solid interior is consistently on the same side; the convexity sign does *not* change this rule, which is why the same stitch code handles both.

**Verdict for Keel: UNIFY, with `sigma` as the single sense branch at the cap.** See the closing synthesis for the exact unified sequence.

---

## Q3. The bead construct-and-sew alternative

**Question.** Is the concave fillet more robustly done by constructing the bead as a separate sheet/solid (blend surface plus planar end-caps coplanar with the body's cap faces) and sewing / uniting it onto the trimmed supports along the spring curves?

**Answer: yes for the hard cases, and this is exactly what the notch-blend patent and the Parasolid cap-by-face-extension behavior do.** The notch-blend patent (US 8,935,130) describes the concave blend as **constructed as a separate surface sheet and integrated by a sewing operation**, with the cap faces grown by *extending adjacent faces to meet the new boundaries and trimming where faces overlap*, rather than by in-place corner dissolve. Parasolid's documented cap behavior forms the cap by **extending at most two adjoining faces to meet** (file 41 Section 1.6), which is structurally a construct-and-sew of a cap region rather than an in-place dissolve. So the construct-and-sew is not merely an alternative; it is the **production-standard mechanism for the concave / notch extremity.**

**Bead construction.** Build the bead as a sheet body (or thin solid) consisting of: (1) the exact analytic blend surface (file 40: cylinder for a line spine, torus for a circular spine) trimmed to the two spring curves and the two cross curves; (2) two planar end-cap faces coplanar with the body's transverse cap faces `Fc`, closing the bead's transverse ends so the bead is a watertight local sheet. The bead is then **sewn onto the trimmed supports along the two spring curves**, and its end-cap faces are **merged (coplanar-coincident union) with the body's cap faces**, which is what *grows* `Fc` by the lune (the bead's end-cap is the lune).

**Tangency at the spring curves under the sew.** The bead is **G1-tangent** to each support along its spring curve by construction (the rolling ball touches the support there; the blend surface and the support share a tangent plane along the spring curve). The sew must therefore join two faces that are **tangent, not transverse**, along their shared edge. This is precisely the coincident/tangent-face sewing problem of `39-coincident-tangent-face-booleans.md`: the spring curve is a **tangent (G1) junction edge**, and the two-sided neighborhood treatment of file 39 applies. The sew must:
- recognize the spring curve as a **smooth (tangent) shared edge**, not a sharp one, so downstream operations (shelling, further blends, tessellation crease detection) treat it as G1;
- set the edge's **convexity attribute to "tangent/smooth"** (not convex/concave) so it is a candidate for further roll-on (file 41 Section 1.3);
- maintain consistent coedge sense across the join (Q2).

**Tolerant-edge treatment at the tangent junction.** A G1 tangent junction is numerically delicate: the two surfaces graze, so the shared edge's 3D curve and the two pcurves can disagree within tolerance, and the "which side is interior" test degenerates because the surfaces are nearly coincident in a neighborhood. The robust treatment is a **tolerant edge**: per the C3D tolerant-topology model of `42-russian-c3d-literature.md` (Golovanov, Source 3), a tolerant edge **is a tube of radius `t`** around an exact carried curve; the geometry stays mathematically exact and the tolerance lives on the topological entity. So the spring-curve junction is represented as a tolerant edge of radius `t` = the sew tolerance, carrying the **exact** spring curve (which is exact for the analytic cases of file 40), with the two pcurves (on the support and on the blend) both certified to lie within the tube. This absorbs the float-level disagreement between the support-side and blend-side spring-curve representations **without perturbing geometry**, which is exactly the failure mode tangent sews hit. The tolerant edge is the right model here precisely because the junction is tangent: a sharp edge could be a sliver-tolerance edge, but a tangent edge *needs* the tube because the surfaces are close over a 2D neighborhood, not just along the curve.

**Is bead-as-local-union more robust than in-place loop-regrow for the concave case, and when?**
- **In-place loop-regrow (Q1)** is the leaner operation: fewer entities, no transient sheet body, no general sew, and it keeps everything exact with no tolerant edge if the spring curves are clean. It is the right default for the **simple, clean, plane-plane concave edge with degree-3 ends** where the cap face is a single planar face and the lune is a simple region.
- **Bead-construct-and-sew** is more robust when the cap region is **not a single clean face**: when the cap is formed by *multiple* faces, when the end-cap must be made by *extending two faces to meet* (Parasolid behavior), when the concave blend meets *another blend or another feature* at the end (Q5), or when the spring-curve junction is numerically marginal and needs tolerant-edge absorption. Construct-and-sew localizes all the robustness pain into one well-understood operation (the tangent sew of file 39, with tolerant edges of file 42), instead of a long in-place operator chain that can leave the model transiently invalid if any single step's geometry is marginal.

**Verdict:** prefer **in-place loop-regrow for the clean degree-3 plane-plane concave cap** (it is the dual of the convex dissolve and shares its code), and **fall back to bead-construct-and-sew** for general caps, multi-face caps, blend-meets-blend / blend-meets-feature ends, and numerically marginal tangent junctions. This mirrors how Parasolid/ACIS keep a fast in-place path and a general capping/sewing path.

---

## Q4. How Parasolid / ACIS / OCCT actually do it

**Parasolid.** One blend API for both convexities (`PK_EDGE_set_blend_constant` etc.); convexity is an edge property, and the material-sign rule is documented verbatim: *"A blend on a convex edge removes material from the model, while a blend on a concave edge adds material to the model"* (PK_BLENDSF Functional Description). The spine is the **intersection of the two radius-offset surfaces**; spring edges are the contact curves; support faces support the rolling ball. At the **extremity**, Parasolid **caps** by *extending at most two adjoining faces to meet* (file 41 Section 1.6), and exposes named overflow behaviors `ov_smooth` / `ov_cliff` / `ov_cliff_end` / `ov_notch` plus `PK_BODY_fix_blends` with `preserve_notch`. The **notch** behavior is the concave-specific extremity: it fires *"when all of one side of a blend face at the end of the blend is trimmed away"* and **extends the blend along the faces in the remaining wall**, i.e. it *grows* the blend across the cap rather than shrinking a corner. *Documented:* the convex/concave material-sign rule, capping-by-face-extension, `ov_notch`, `preserve_notch`. *Inferred:* the exact internal Euler-operator sequence (Parasolid does not publish it; the public behavior is consistent with the grow-splice / cap-by-extend of Q1/Q3).

**ACIS.** Convex (round) and concave (fillet) both flow through the blend graph (network of edges/vertices each carrying a radius); convexity is not a separate recipe but a property consumed by the blend-surface and capping logic. ACIS's documented difficulty is *"complicated mixed convexity capping cases ... difficulties in intersecting the spring curves with the near faces at a mixed convexity end"* (Standard Blending Error Messages), which is direct evidence that (a) capping is where convex/concave forks, and (b) the fork is implemented by **intersecting the spring curves with the near (cap) faces** and is hardest exactly at a **mixed-convexity end** (where one support is convex and the other concave). ACIS's `ChFiDS`-style structures carry the spring curves and cross curves; vertex ends use **setback / autosetback** and mitring. *Documented:* the blend graph, spring/cross curves, mixed-convexity capping as the hard case, setback/autosetback, vertex blends. *Inferred:* the precise operator-level cap-grow for the pure concave end.

**OCCT.** `ChFi3d_Builder` is the common base; `ChFi3d_FilBuilder` (fillets) and `ChFi3d_ChBuilder` (chamfers) specialize it. Convexity is classified by the static predicate **`ChFi3d::ConcaveSide`** (*"Returns Reversed ... if the concave edge defined by the interior of faces F1 and F2 ... is opposite to the normal of their surface support"*) and reconciled across adjacent blends by **`ChFi3d::SameSide`** (*"determine ... if two fillets or chamfers constituting a face have identic or opposed concave edges"*). The **extremity** is handled by the three pure-virtual corner extenders **`ExtentOneCorner` / `ExtentTwoCorner` / `ExtentThreeCorner`** (one per number of incident edges at the end vertex), implemented in FilBuilder/ChBuilder; these are exactly the methods that **extend** the blend surface and the supports at the end vertex, i.e. the *grow* side of the cap. The per-blend geometry lives in `ChFiDS_SurfData` and the boundary state in `ChFiDS_State` (the enum that records, per stripe end, whether the extremity is on a free boundary, tangent, closed, broken, etc.), which is OCCT's encoding of "trim back vs extend at this end." *Documented:* `ConcaveSide`, `SameSide`, `Extent{One,Two,Three}Corner`, `ChFiDS_SurfData`, `ChFiDS_State` exist and serve these roles. *Inferred:* the exact internal stitching is OCCT's BOPAlgo/sewing, not pure published Euler ops.

**Synthesis across the three.** All three use **one pipeline with a convexity classifier** (`ConcaveSide` in OCCT; an edge-convexity property in Parasolid/ACIS) and fork **only at the extremity** into trim-back (convex) vs extend/grow (concave). The concave end is universally the **harder** case and is handled by **extending support/cap faces to meet** (Parasolid cap-by-extension, OCCT `Extent*Corner`, ACIS spring-curve-to-near-face intersection), which is the construct-and-grow of Q1/Q3, not the dissolve of the convex case. None publish a literal `mvfs/mev/mef/...` sequence; the Keel sequence in Q1 is the operator-level realization consistent with all three documented behaviors.

---

## Q5. The general reentrant case beyond a single prismatic edge

For a concave edge whose two end vertices are *general* (not the box-like degree-3 corner), the end condition is governed by **what the concave blend runs into at its end**, and differs from the convex end handling of file 41 mainly in **sign and in trim-vs-extend**:

- **Concave blend runs into another face (transverse termination).** The blend bead's end must be closed against a face that is *not* coplanar with a simple cap. Production behavior: **extend the blend (and, if needed, up to two adjoining faces) to meet the obstructing face**, then cap (Parasolid cap-by-extension; OCCT `ExtentOneCorner`). Operator-level: this is the Q1 grow-splice with the end-cap's far boundary being the *intersection curve of the bead with the obstructing face* instead of a planar lune edge; `split_edge`/`split_face` imprint that intersection on the obstructing face, then `mef` closes the bead end against it. Versus convex (file 41): convex ends **trim back** to a cliff or cap and the corner triangle is dissolved *inward*; concave ends **extend out** to the obstructing face and the lune is added *outward*.

- **Concave blend runs into another blend (blend-blend at a vertex).** If both edges into the end vertex are blended, the two beads must be **reconciled** by setback + an n-sided vertex patch (file 41 Section 1.8, file 28 vertex ladder; OCCT `ExtentTwoCorner` / `ExtentThreeCorner`; ACIS autosetback + suitcase-corner octant patch). For concave-concave the patch is a *material-adding* corner blend (a spherical-octant-like fillet vertex), the dual of the convex suitcase corner. Operator-level: setback trims each spring curve back by the setback distance (`split_edge` on the spring curves, `kev`/`kef` to retract), opening an n-sided hole at the vertex, which is filled by an n-sided patch face inserted with `mef` per added boundary edge (file 26 transfinite n-sided surfacing supplies the surface; topology is n `mef`s closing the loop). The **mixed-convexity vertex** (one edge convex, one concave) is the documented hard case (ACIS "mixed convexity capping"): the patch must add material on the concave side and remove it on the convex side, so the setback hole's boundary has mixed sense and the patch is sense-mixed; this is where construct-and-sew (Q3) is strongly preferred over in-place regrow.

- **Concave blend runs into a smooth (tangent) continuation.** The end boundary the spring curve crosses is tangent, so the ball **rolls on** onto the neighbor (file 41 Section 1.3, `ov_smooth`/roll-on; extended-rolling-ball of Choi-Lee, US 5,774,359, which extends the offset/envelope intersection *across* the boundary so spine and spring curves continue onto the neighbor). There is **no cap at all** at such an end: the blend face simply continues, the spine re-defines (one offset surface changes), and the two ribbon segments are stitched at the transfer parameter with a `split_face`/`mef` at the cross-edge. This case is **sign-agnostic**: roll-on works identically for convex and concave, because tangency means no material-sign discontinuity at the crossing. The only concave-specific subtlety is that the *radius-versus-local-feature-size* limit (file 41 Section 2) binds harder on the concave side (inner-offset self-intersection), so a concave roll-on is more likely to hit the LFS ceiling and trigger radius-limiting before it completes.

- **Runout / setback at a free end (no obstruction).** Same as convex (file 28 Section 5.3, file 41 Section 4.2): taper the radius to zero over a setback length so the spring curves converge back to the original edge. Sign-agnostic in topology; the surface just ramps `r(t) -> 0`.

**Net difference from the convex end handling of file 41.** The *menu* of end conditions (into-face, into-blend, into-tangent, free runout) is the same; the *cap* sub-case flips from **trim-back-and-dissolve-inward** (convex) to **extend-out-and-add-outward** (concave), and the mixed-convexity end is the genuinely new hard case that has no convex-only analogue and is best handled by construct-and-sew.

---

## Per-source entries

### S1. Mantyla, *An Introduction to Solid Modeling* (1988)
- **Citation.** Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press. https://archive.org/details/introductiontoso0000mant
- **Content.** Canonical GWB Euler-operator set (mvfs, mev, mef, kev, kef, kemr/mekr, kfmrh/mfkrh) over a half-edge structure, with the completeness/soundness theorem: any valid 2-manifold B-rep is reachable from the empty model by a finite operator sequence, and every sequence preserves validity. Gives the fixed count delta of each operator and the half-edge realization of `split_edge`/`split_face` as the mev/mef pair.
- **Kernel relevance.** The license and the bookkeeping for Q1: every step of the grow-to-arc sequence is a GWB operator with a known delta, so Euler-Poincare is preserved by construction. The convex dissolve and concave splice are both expressible in this vocabulary.

### S2. Mantyla and Sulonen, GWB (1982)
- **Citation.** Mantyla, M., & Sulonen, R. (1982). GWB: A solid modeler with Euler operators. *IEEE CG&A, 2*(7), 17-31.
- **Content.** Original operator partition: skeleton (mvfs/kvfs), local connectivity (mev/kev, mef/kef, kemr/mekr), global/handle (kfmrh/mfkrh). Establishes that local surgery is mev/mef-dominant (make) or kev/kef-dominant (kill).
- **Kernel relevance.** The make-dominant (concave) vs kill-dominant (convex) characterization of the cap operation (Q1) is exactly this partition applied to the cap.

### S3. Weiler, radial-edge structure (1986/1988)
- **Citation.** Weiler, K. (1988). The radial edge structure: A topological representation for non-manifold geometric boundary modeling. In *Geometric Modeling for CAD Applications* (pp. 3-36). North-Holland. PhD: RPI 1986.
- **Content.** Use-based hierarchy (model/region/shell/faceuse/loopuse/edgeuse/vertexuse) with the **radial cycle** linking all face-uses around an edge; a minimal NMG operator set (make-region, make-edge, make-face, kill-edge) and the radial-cycle splice for inserting a face-use into an edge's radial order.
- **Kernel relevance.** The blend face `B` is spliced into the spring-curve edges' radial cycles; for the *manifold* concave fillet the radial cycle has exactly two face-uses per spring edge (support + blend), so the splice is the ordinary half-edge twin pairing. Non-manifold radial splice is only needed if the concave edge is itself non-manifold (rare for fillets).

### S4. Lee and Lee, partial entity structure (2001)
- **Citation.** Lee, S. H., & Lee, K. (2001). Partial entity structure: A compact non-manifold boundary representation based on partial topological entities. *Proc. ACM Solid Modeling Symposium*; *J. Comput. Inf. Sci. Eng., 1*(4), 356. https://asmedigitalcollection.asme.org/computingengineering/article/1/4/356/471622
- **Content.** Half the storage of the radial-edge structure via *partial* topological entities (partial-vertex, partial-edge, partial-face) while preserving full adjacency; an accompanying generalized Euler-operator set over partial entities.
- **Kernel relevance.** Keel's PES topology (file 02): the Q1 sequence's `mev`/`mef`/`kef`/`kev` map onto the PES partial-entity operators; the cap-grow does not introduce non-manifold partial entities for the simple case, so the lighter PES operators suffice.

### S5. Masuda, non-manifold topological operators (1993)
- **Citation.** Masuda, H. (1993). Topological operators and Boolean operations for complex-based non-manifold geometric models. *Computer-Aided Design, 25*(2), 119-129.
- **Content.** A complex-based non-manifold model with topological operators and a boolean engine built on them; shows booleans (including local material add/remove) reduce to non-manifold Euler-operator sequences.
- **Kernel relevance.** Direct support for the Q2 "concave fillet = local boolean (material addition)" framing: a material-adding local union is realizable as a non-manifold operator sequence, the same machinery as the cap-grow splice.

### S6. Stroud, *Boundary Representation Modelling Techniques* (2006)
- **Citation.** Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer. https://link.springer.com/book/10.1007/978-1-84628-616-2 ; Internet Archive: https://archive.org/details/boundaryrepresen0000stro
- **Content.** Detailed Euler-operator recipes for modeling operations, with explicit chapters on **blending and chamfering** as operations on special models, plus joining/splitting along edges and planar sectioning. Treats blends as operator-driven local edits on the boundary model, with the operator validity guaranteed by manipulating element combinations (not isolated elements).
- **Kernel relevance.** The most directly applicable textbook source for an *operator-level* blend recipe; confirms that blend insertion is expressed as Euler-operator sequences on the boundary model and that the convex/concave distinction is a local-edit sign, supporting the Q1 sequence and the Q2 unify verdict.

### S7. Braid, non-local blending of boundary models (1997)
- **Citation.** Braid, I. C. (1997). Non-local blending of boundary models. *Computer-Aided Design, 29*(2), 89-100. https://www.sciencedirect.com/science/article/abs/pii/S0010448596000383
- **Content.** From the ROMULUS/Parasolid lineage author: blends as operations on the boundary model where the blend region replaces a hierarchy of primary surfaces, connecting features (fillets), and vertex blends under a well-defined topological structure; "non-local" handles blends whose extent exceeds the immediate edge neighborhood (roll-on, blend-blend).
- **Kernel relevance.** Authority for treating the fillet as a *structured local boundary edit* with primary/connecting/vertex layers, and for the non-local (roll-on, blend-blend) end conditions of Q5. The ROMULUS-to-Parasolid lineage makes this the conceptual root of the Parasolid behavior cited in Q4.

### S8. Parasolid Edge Blending Functional Description (q-solid mirror)
- **Citation.** Siemens / Parasolid. *Edge Blending: Overview; Functions and Options; Edge Blend Overflows; PK_BLENDSF.* http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html , .31.html ; V35: http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.11.html
- **Content.** Verbatim: spine = intersection of the two radius-offset surfaces; spring edges = contact curves; support faces support the ball; **"A blend on a convex edge removes material from the model, while a blend on a concave edge adds material to the model."** Cap = extend at most two adjoining faces to meet; overflow behaviors `ov_smooth/ov_cliff/ov_cliff_end/ov_notch`; `ov_notch` extends the blend along the remaining wall when one side is trimmed away; `PK_BODY_fix_blends` / `preserve_notch`.
- **Kernel relevance.** Primary-source confirmation of the convex/concave material-sign rule (Q2), the single-API unification (Q4), and the cap-by-face-extension grow behavior (Q1, Q3). The `ov_notch` behavior is the concave extremity in production vocabulary.

### S9. OCCT ChFi3d class reference (ConcaveSide / SameSide / NextSide)
- **Citation.** Open CASCADE Technology. *ChFi3d Class Reference.* https://dev.opencascade.org/doc/occt-6.9.1/refman/html/class_ch_fi3d.html
- **Content.** `ConcaveSide(...)`: *"Returns Reversed in Or1 and(or) Or2 if the concave edge defined by the interior of faces F1 and F2, in the neighbourhood of their boundary E is of the edge opposite to the normal of their surface support."* `SameSide(...)`: *"determine ... if two fillets or chamfers constituting a face have identic or opposed concave edges."*
- **Kernel relevance.** The exact orientation invariant for Q2: convex/concave is decided by the edge sense *versus the support-surface normal*, a single sign that then drives the extremity logic. `SameSide` is how adjacent blends reconcile convexity at a shared vertex (Q5 mixed-convexity).

### S10. OCCT ChFi3d_Builder reference (Extent*Corner, SurfData, State)
- **Citation.** Open CASCADE Technology. *ChFi3d_Builder Class Reference.* https://dev.opencascade.org/doc/occt-6.9.0/refman/html/class_ch_fi3d___builder.html
- **Content.** Pure-virtual `ExtentOneCorner(V,S)`, `ExtentTwoCorner(V,LS)`, `ExtentThreeCorner(V,LS)` (extend the blend at end vertices of valence 1/2/3), implemented in FilBuilder/ChBuilder; corner performers `PerformTwoCorner` / `PerformThreeCorner` / `PerformMoreThreeCorner`; references `ChFiDS_State` and edge/face maps.
- **Kernel relevance.** The extremity is where convex/concave forks and where the concave end *extends* (grows) the blend; `ChFiDS_State` is OCCT's per-end trim-vs-extend record. Directly supports the Q4 "fork only at the extremity" finding and the Q5 vertex cases.

### S11. OCCT ChFi3d_FilBuilder / ChBuilder (kind split)
- **Citation.** Open CASCADE Technology. *ChFi3d_ChBuilder / ChFi3d_FilBuilder Class References.* https://dev.opencascade.org/doc/occt-7.5.0/refman/html/class_ch_fi3d___ch_builder.html
- **Content.** Separate builders per blend *kind* (fillet vs chamfer), both subclassing `ChFi3d_Builder`; convexity handled inside via `ConcaveSide`, not by a separate builder.
- **Kernel relevance.** Confirms the unification axis: kernels separate fillet-vs-chamfer (surface kind) but **unify convex-vs-concave** (sign), the opposite of what one might naively expect. Keel should likewise split by section primitive (file 28) and unify by convexity sign.

### S12. ACIS Blending Component and Standard Blending Error Messages
- **Citation.** Spatial / Dassault. *Blending Component (BLND/01CMP).* http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF ; *Standard Blending Error Messages.* http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblnder.htm
- **Content.** Blend graph (network of edges/vertices with radii); spring curves and cross sections; **"complicated mixed convexity capping cases ... difficulties in intersecting the spring curves with the near faces at a mixed convexity end"**; vertex blends with setback/autosetback; mitring of blend sequences.
- **Kernel relevance.** Evidence that (a) capping is the convex/concave fork point, (b) the cap is built by **intersecting spring curves with the near (cap) faces** (the construct-and-sew of Q3), and (c) mixed-convexity ends are the hard case (Q5) where in-place regrow should yield to construct-and-sew.

### S13. ACIS Variable Radius Blending / Blend Cross Section
- **Citation.** Spatial / ACIS. *Variable Radius Blending; Blend Cross Section.* http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0008.HTM ; .../ABL/ABL/01CMP/0002.HTM
- **Content.** Rolling-ball marching to find spring curves and surface; constant round -> circular cross curve; spring curves identical across cross-section shape variants, only the section differs.
- **Kernel relevance.** Confirms the file-28 "one engine, polymorphic section, shared spring curves" decomposition and that the *topology* (spring curves, cross edges) is independent of section shape, so the Q1/Q2 topology is section-agnostic.

### S14. Notch blends in B-rep models (US 8,935,130)
- **Citation.** *Notch blends in BRep models.* US Patent 8,935,130. https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8935130
- **Content.** Concave (notch) blend constructed as a **separate surface sheet** and integrated by a **sewing operation**; cap faces **grown by extending adjacent faces to meet new boundaries and trimming overlaps**; validity maintained by consistent orientation and a sew that preserves the closed manifold.
- **Kernel relevance.** The single most direct source for Q3: the concave/notch extremity is production-implemented as **bead-construct-and-sew with cap-by-face-extension**, not pure in-place dissolve. Authority for the construct-and-sew fallback recommendation.

### S15. Choi and Lee, extended rolling-ball fillet (US 5,774,359)
- **Citation.** Choi, B. K., & Lee, C. S. *Method for generating an extended rolling ball fillet surface between two surfaces.* US Patent 5,774,359 (Autodesk). https://patents.google.com/patent/US5774359A/en
- **Content.** Offset of `F1` intersected with the *envelope* of `F2`'s boundary; spine fitted to the intersection; contact curves projected; arcs generated tangent at the spring curves; the construction **extends past the original face boundary** so spine/spring curves continue onto a neighbor.
- **Kernel relevance.** The roll-on / smooth-continuation end condition of Q5 (sign-agnostic); the extension-across-boundary is how a concave blend carries onto a tangent neighbor rather than capping.

### S16. C3D / Golovanov tolerant-topology model (file 42, Source 3)
- **Citation.** Golovanov, N. (and C3D Labs technical materials). *Geometric Modeling* / C3D tolerant geometry. Summarized in `42-russian-c3d-literature.md`, Source 3.
- **Content.** Tolerance lives on the topological entity: a tolerant **vertex is a sphere of radius t**, a tolerant **edge is a tube of radius t**, while the carried curve/surface stays mathematically exact.
- **Kernel relevance.** The model for the G1 spring-curve junction in Q3: represent the tangent sew seam as a tolerant edge (tube of radius t) carrying the exact spring curve, absorbing float-level pcurve disagreement without perturbing geometry. Essential because tangent junctions graze over a 2D neighborhood.

### S17. Keel dossier 39, coincident/tangent-face booleans
- **Citation.** `docs/research/kernel/39-coincident-tangent-face-booleans.md`.
- **Content.** Two-sided neighborhood treatment for sewing coincident/tangent faces; classification of shared edges as smooth vs sharp; consistent coedge sense across coincident joins.
- **Kernel relevance.** The sew machinery the bead-construct-and-sew (Q3) reuses: the spring curve is a tangent shared edge, sewn with the file-39 two-sided neighborhood method and tagged smooth for downstream roll-on.

### S18. Keel dossiers 01 / 02 / 40 / 41 (in-repo baseline)
- **Citation.** `01-topology-euler-operators.md`, `02-nonmanifold-cellular.md`, `40-blend-fillet-surface-generation.md`, `41-blend-overflow-radius-feature-failures.md`.
- **Content.** Operator vocabulary and Euler-Poincare invariant (01); PES/radial topology (02); spine-as-offset-intersection and analytic surface catalog, offset sign flips for convex/concave (40); overflow / cap / notch / setback vocabulary and the convex cap baseline (41).
- **Kernel relevance.** The baseline this dossier dovetails with: the concave surgery reuses 40's geometry, 41's overflow menu, and 01's operators and invariant unchanged; only the cap step is new.

---

## Recommended concave-fillet cap surgery for Keel (synthesis)

**Decision 1: UNIFY the convex and concave pipelines, forking only at the cap, on the single convexity sign `sigma`.** Compute `sigma = sign((n_1 x n_2) . t)` per blended edge (per parameter for inflecting edges, file 41 Section 1.8), the Keel equivalent of OCCT `ChFi3d::ConcaveSide`. The shared body of the surgery (imprint spring curves with `split_edge` + `split_face`; trim supports; kill the sharp edge with `kef` + `kev`; attach the analytic blend surface from file 40 with consistent coedge sense) is identical for both signs because the spine, spring curves, and offset sign are already sign-correct from file 40. **Only the cap step consults `sigma`.** This is what Parasolid, ACIS, and OCCT all do (one API/builder, a convexity classifier, a single extremity fork).

**Decision 2: default to in-place loop-regrow for the clean degree-3 plane-plane concave cap; fall back to bead-construct-and-sew otherwise.** The in-place regrow is the dual of the convex dissolve and reuses its `kef`/`kev` tail, so it is cheap and exact for the common case. Switch to construct-and-sew when the cap is multi-face, the end runs into another face/blend, the end is mixed-convexity, or the spring-curve junction is numerically marginal (the cases ACIS flags as "mixed convexity capping" and the notch-blend patent solves by sewing).

### Explicit unified cap sequence (one transverse end; assert `V - E + F = 2(S - G) + (L - F)` after each line)

```
GIVEN: support faces F1, F2 trimmed to spring curves; sharp edge e removed;
       spring curves land on cap face Fc at vertices vs1 (created by split_edge on Fc edge from F1 side)
       and vs2 (created by split_edge on Fc edge from F2 side); reentrant corner vertex v0 between them.
       sigma < 0 (concave). [If sigma > 0, take the convex branch: split-off-triangle + kef + kev dissolve.]

CONCAVE BRANCH (grow-to-arc, in-place loop-regrow):
  1. split_edge on Fc's F1-side boundary edge at vs1        # +1V +1E      (skip if already placed in body imprint)
  2. split_edge on Fc's F2-side boundary edge at vs2        # +1V +1E
  3. mev: spur the end-arc edge A outward from vs1 to helper va (va at vs2 location)   # +1V +1E
  4. mef (= split_face): close A from va to vs2, creating blend face B (first end only;
        on the second end this mef closes B's existing loop, dF=0)                      # +1E (+1F first end)
  5. kev: merge helper va so the arc vs1->vs2 is a single curved edge                   # -1V -1E
        # (steps 3-5 collapse to a single mef if the kernel's edge-make accepts a curved
        #  edge to an existing target vertex in one stroke)
  6. kef on fragment vs1->v0: merge the lune sliver across the old Fc boundary into B    # -1E -1F
  7. kef on fragment v0->vs2: merge the second fragment, isolating v0 as a spur          # -1E -1F  (rejoins, net wall removal)
  8. kev on v0: absorb the swallowed corner, making v0 INTERIOR to the grown cap region  # -1V -1E
  9. radial-cycle splice: pair B's spring-curve coedges as twins of the support coedges
        (sense opposite, solid interior consistent) -- automatic in the fin structure.
 10. attach analytic blend surface to B (file 40); tag spring curves SMOOTH (file 39).
```

Per-end net (concave branch, steps 1-8): dV = +1+1+1+0-1+0+0-1 = **+1**; dE = +1+1+1+1-1-1-1-1 = **0**; dF = first end +1, second end 0. Across both ends plus the ribbon body the global insertion nets **+1 face** (the single blend face B) and preserves S, G, R (no ring or handle operator needed for the box notch). **Euler-Poincare is preserved at every step** because each line is a GWB operator with a fixed validity-preserving delta (S1, S6).

### Bead-construct-and-sew fallback (general / multi-face / mixed-convexity / marginal end)
```
  A. Build bead sheet: analytic blend surface (file 40) + two planar end-cap faces coplanar with Fc.
  B. Sew bead onto trimmed supports along the two spring curves using the file-39 two-sided
     tangent-sew; represent each spring-curve seam as a TOLERANT EDGE = tube of radius t carrying
     the exact spring curve (file 42 / S16), tagged SMOOTH, coedge sense consistent.
  C. Coplanar-union the bead's end-cap faces with Fc (this GROWS Fc by the lune); if the end runs
     into another face, intersect the bead with it and imprint (split_edge/split_face) instead.
  D. For blend-blend / mixed-convexity vertex ends: setback (retract spring curves) + n-sided
     vertex patch (file 26 surface, n mef's to close the loop); prefer this path for mixed sense.
```

**When to prefer which (one line):** in-place loop-regrow for the clean, single-cap, single-convexity prismatic concave edge (fast, exact, code-shared with convex via `sigma`); bead-construct-and-sew with tolerant tangent seams for everything harder (multi-face caps, into-face / into-blend ends, mixed convexity, marginal tangency), matching the documented Parasolid `ov_notch` / cap-by-extension and the notch-blend patent. Either way the global topology delta is +1 blend face and Euler-Poincare holds; the only thing that ever flips is the **sign of the cap-area change** (grow for concave, shrink for convex) selected by the single convexity invariant `sigma`.

---

## References

1. Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press. https://archive.org/details/introductiontoso0000mant
2. Mantyla, M., & Sulonen, R. (1982). GWB: A solid modeler with Euler operators. *IEEE CG&A, 2*(7), 17-31.
3. Weiler, K. (1988). The radial edge structure: A topological representation for non-manifold geometric boundary modeling. In *Geometric Modeling for CAD Applications* (pp. 3-36). North-Holland. (PhD: RPI, 1986.)
4. Lee, S. H., & Lee, K. (2001). Partial entity structure: A compact non-manifold boundary representation based on partial topological entities. *J. Comput. Inf. Sci. Eng., 1*(4), 356. https://asmedigitalcollection.asme.org/computingengineering/article/1/4/356/471622
5. Masuda, H. (1993). Topological operators and Boolean operations for complex-based non-manifold geometric models. *Computer-Aided Design, 25*(2), 119-129.
6. Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer. https://link.springer.com/book/10.1007/978-1-84628-616-2 ; https://archive.org/details/boundaryrepresen0000stro
7. Braid, I. C. (1997). Non-local blending of boundary models. *Computer-Aided Design, 29*(2), 89-100. https://www.sciencedirect.com/science/article/abs/pii/S0010448596000383
8. Siemens / Parasolid. *Edge Blending Functional Description; PK_BLENDSF; Edge Blend Overflows.* http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html ; .31.html ; V35 overview: http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.11.html
9. Open CASCADE Technology. *ChFi3d Class Reference* (ConcaveSide, SameSide, NextSide). https://dev.opencascade.org/doc/occt-6.9.1/refman/html/class_ch_fi3d.html
10. Open CASCADE Technology. *ChFi3d_Builder Class Reference* (ExtentOneCorner/TwoCorner/ThreeCorner, ChFiDS_State). https://dev.opencascade.org/doc/occt-6.9.0/refman/html/class_ch_fi3d___builder.html
11. Open CASCADE Technology. *ChFi3d_ChBuilder / ChFi3d_FilBuilder Class References.* https://dev.opencascade.org/doc/occt-7.5.0/refman/html/class_ch_fi3d___ch_builder.html
12. Spatial / ACIS. *Blending Component (BLND/01CMP); Standard Blending Error Messages.* http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF ; http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblnder.htm
13. Spatial / ACIS. *Variable Radius Blending; Blend Cross Section.* http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0008.HTM ; .../ABL/ABL/01CMP/0002.HTM
14. *Notch blends in BRep models.* US Patent 8,935,130. https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8935130
15. Choi, B. K., & Lee, C. S. *Method for generating an extended rolling ball fillet surface between two surfaces.* US Patent 5,774,359. https://patents.google.com/patent/US5774359A/en
16. Golovanov, N. / C3D Labs. *Geometric Modeling; C3D tolerant geometry* (tolerant edge = tube of radius t). Summarized in `docs/research/kernel/42-russian-c3d-literature.md`, Source 3.
17. Keel dossier. `docs/research/kernel/39-coincident-tangent-face-booleans.md`.
18. Keel dossiers. `docs/research/kernel/01-topology-euler-operators.md`, `02-nonmanifold-cellular.md`, `40-blend-fillet-surface-generation.md`, `41-blend-overflow-radius-feature-failures.md`.
