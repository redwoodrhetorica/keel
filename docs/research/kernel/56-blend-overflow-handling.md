# 56. Blend Overflow Handling: Construction Algorithms for Cliff, Notch, Smooth, and Cap

**Title and scope.** This dossier gives the *handling algorithms* for blend overflow: the concrete geometry-plus-topology constructions that build a valid body when a blend is too big for its supports. It is the constructive sequel to `41-blend-overflow-radius-feature-failures.md`, which catalogued the eleven failure modes F1 to F11 with their detection predicates and the medial-axis `r_max(t) = lfs(spine(t))` ceiling. File 41 told Keel *when* and *why* a blend overflows and recommended a reduce/reject/report policy; it deliberately stopped at "detect and decline." **This file consumes that catalog and supplies the missing constructions**: for each overflow kind it gives the surface geometry and the GWB Euler-operator topology surgery (with V/E/F/R bookkeeping), maps Parasolid's named `ov_*` controls onto those constructions with patent care, gives the cheapest *complete* predicate set to decide which handler fires, isolates the overflow cases that stay analytic, and (the deliverable that matters most) picks the single most common case in CAD practice and works it fully end to end. Keel today *detects* overflow via spring-line containment and *declines honestly* (file 41 Section 3.2, the best-effort doctrine of file 29); this dossier is the design recommendation that turns that honest decline into an honest *handle*, to be validated against Keel's exact-triplet (`mass == mesh == ref`) tests.

This builds on, and does not repeat, what Keel already has: winding-number-classified booleans (imprint, classify, select, import-and-glue stitch with `mass == mesh` and shell-closure gates); the trim-and-stitch single-edge blend surgery (imprint spring lines, split caps along end arcs, `kef`/`kev` dissolution) carrying exact cylinder, cone (linear variable radius), torus, rational-quadratic conic, and G2 quintic sections; hold-line and face-face rungs; blend recognition and exact unblend; the certified-evaluator-to-NURBS fit pipeline; canonical recovery; tolerant edges; and non-manifold PES topology. The Euler operators are the GWB / Mantyla set fixed in file 01: `mvfs`, `mev`, `mef`, `kev`, `kef`, `kemr`, `mekr`, `kfmrh`, `mfkrh`, plus the helpers `split_edge` and `split_face`, with the invariant `V - E + F = 2(S - G) + (L - F)` (R = L - F the ring count) asserted after every step.

Terminology is consistent with files 28, 40, 41, 44: **spine** = rolling-ball center locus (offset-surface intersection); **spring curves** = the two contact loci on the supports; **cross curves** = the transverse section-end boundaries; **support faces** = the two faces meeting at the blended edge; **cap / transverse face** = a face transverse to the spine bounding an end of the ribbon; **overflow** = a spring curve leaving the trimmed region of its support (file 41 Section 1.1).

A labeling note used throughout: claims drawn verbatim from a primary doc are tagged **[verified]**; claims reconstructed from secondary material or inferred from documented public behavior are tagged **[claimed]**.

---

## Section 0. The handling-decision spine (overview before the parts)

Production kernels do not pick an overflow handler ad hoc; they apply a **fixed priority ladder** at each overflow event. T-FLEX states it verbatim: *"The order of applying possible solutions in this case will be first the smooth overflow, then the overflow with cliff edges, and, finally, the notch"* **[verified]**. Restated as Keel's decision spine, per overflow parameter `t` where a spring curve crosses a support boundary edge `b`:

```
classify_overflow(t, crossing boundary edge b):
  if b is a TANGENT (G1) edge with a face beyond that the ball can roll onto:
      if the face beyond is itself a rounding/blend surface:  -> SMOOTH   (Sec 3)
      else:                                                   -> ROLL-ON  (Sec 3, the carry-through)
  elif b is a CONVEX, NON-TANGENT (sharp) edge with a drop beyond:
      if the event is at a terminating end vertex:            -> CLIFF_END (Sec 1)
      else:                                                   -> CLIFF     (Sec 1)
  elif one whole side of the blend face is trimmed away (supports pinch):
                                                              -> NOTCH     (Sec 2)
  else (no smooth continuation, no clean drop, ribbon end open):
                                                              -> CAP       (Sec 2)
```

The ladder is a *preference order*, not a mutually-exclusive partition: smooth is tried first because it is the premium (fully tangent) outcome, cliff next because it terminates cleanly against a real model edge, notch last because it is the most delicate (it bridges a collapsed region). Cap is the universal fallback that always yields a watertight body. The predicate set that evaluates the four branches cheaply and completely is Section 5; the per-kind constructions are Sections 1 to 3; the most-common case worked fully is Section 7.

---

## Section 1. Cliff-edge overflow: the construction

**What a cliff is (file 41 Section 1.5, made precise here).** Parasolid's definition, recovered verbatim: *"A cliff edge blend is a special case of a blended edge on which the newly created blend face is tangent to only one of the faces adjacent to the original edge and runs along an edge in the other face"* **[verified]**. So a cliff blend is **half-tangent**: G1 to *one* support, and *bounded by an existing sharp model edge* on the other side instead of by a second spring curve. The ball cannot roll onto the cliff edge's far side (no tangency, file 41 Section 1.3) and there is no more face there (unlike a roll-on), so the blend surface terminates *along the cliff edge itself*, leaving that edge sharp.

**The construction (geometry).** Two regimes, both documented:

1. *Full cliff (the spring curve never re-lands).* The blend cross-section is built so it *spans between the contact line on the tangent support and the cliff edge on the other side*. T-FLEX: for a cliff blend the cross section spans "between the wall and the line of contact on one side and the cliff edge on the other" **[verified, secondary]**. Concretely: keep the spring curve on support `F1` (the tangent side) exactly as the rolling ball gives it; on the `F2` side, *replace* the off-face spring curve with the **cliff edge curve** `b` itself. The blend ribbon is the surface ruled/swept between spring-curve-on-`F1` and `b`. For Keel's analytic catalog this is still a cylinder (line spine) or torus (circular spine) **clipped by the plane/surface containing the cliff edge** rather than by the natural second spring curve, then trimmed to `b`. The blend stays exact; only its `F2`-side trim boundary changes from "natural spring curve" to "model edge `b`."

2. *Creeping cliff (the ball rolls partly on top of the cliff edge).* T-FLEX, verbatim: when the blend creeps over a cliff edge, *"the rounding surface is constructed near the zone of contact with the edge by the rolling ball method. The surface is swept by a ball rolling on top of the 'constraining' edge, touching one of the original faces"* **[verified, secondary]**. Here the contact transfers from face `F2` onto the *one-dimensional* cliff edge `b`: the ball now touches `F1` (surface contact, ordinary spring curve) and rides *on the edge `b`* (curve contact). This is the rolling-ball-on-an-edge degenerate canal surface. A further documented refinement handles the case where *only a portion* of the cliff end creeps onto the constraining edge **[verified, secondary]**.

**The topology surgery (cliff, full regime).** Setup: support faces `F1` (tangent side) and `F2` (cliff side); the blended sharp edge `e` between them; the cliff edge `b` is a boundary edge of `F2`. The spring curve on `F1` is `s1`; on the `F2` side the trim boundary is `b` (no `s2`).

| Step | Operator | Effect | dV | dE | dF |
|---|---|---|---|---|---|
| 1 | `split_edge` (xk) | Imprint `s1`'s endpoints on `F1`'s bounding edges; imprint where `s1` meets `b` at the ribbon ends. | +k | +k | 0 |
| 2 | `split_face` (`mef`) on `F1` | Cut `s1` across `F1`, separating keep-strip from trim-strip. | 0 | +1 | +1 |
| 3 | `kef` + `kev` | Dissolve `F1`'s trim-strip (the sliver between `s1` and old `e`). | -1 | -2 | -1 |
| 4 | `kef` + `kev` | Remove the sharp edge `e` and its spur vertices, opening the gap. | -2 | -3 | -1 |
| 5 | `mef` | Build the blend face `B` bounded by `s1` (twin of `F1`'s keep coedge), the cliff edge `b` (shared with `F2`, sense consistent), and the two cross edges. | 0 | +1 | +1 |
| 6 | radial splice + attach | Pair `B`'s `s1` coedge as twin of `F1`'s; **`B`'s `b` coedge becomes the second use of the existing edge `b`** (now `b` is shared by `F2` and `B`, sharp on the `F2` side, tangent on the `B`-to-`F1` side). Attach the analytic surface (Sec 6). | 0 | 0 | 0 |

The signature of the cliff move: **the second spring curve is replaced by an existing model edge**, so `B` gains a *shared* boundary with the cliff-side neighbor rather than a fresh trimline. The cliff edge `b` stays sharp (its convexity attribute is unchanged); the `s1` edge is tagged SMOOTH (file 39). Net for a single cliff segment: one blend face added, the cliff edge promoted from a one-face boundary to a two-face shared edge, S/G/R unchanged.

**Cliff_end** is the same construction localized at a terminating vertex: the cliff condition coincides with the blend's end vertex, so step 5's cross edge degenerates and the ribbon closes onto the vertex. Parasolid names this `ov_cliff_end` distinctly precisely because the end-vertex degeneracy needs separate code **[verified, file 41 primary-source verification]**.

**Patent posture.** The cliff construction (clip the analytic blend by an existing model edge; keep the edge sharp) is *old and unencumbered*: it is the natural trim of a swept surface against a boundary, prior art well before 2006 (Rossignac-Requicha 1986; Vida-Martin-Varady 1994). The creeping-cliff "ball rolling on top of the constraining edge" is a degenerate canal surface, also classical. **No live patent claim is implicated by the full-cliff clip-and-trim.** The creeping variant should be implemented from the public T-FLEX/Parasolid *behavior description* (ball-on-edge canal), not from any vendor's specific algorithmic claim; the construction itself (canal of a ball constrained to one surface and one curve) is general mathematics and not claimable.

---

## Section 2. Notch and cap overflow: the constructions

### 2.1 Notch

**What a notch is (file 41 Section 1.6, made constructive).** A notch overflow fires when, as the blend grows, *one whole side of the blend face at the end of the blend is trimmed away*: the two supports pinch together and the ribbon collapses on that side **[verified, Parasolid]**. It is the concave dual of the cliff (cliff = "face ends in a drop"; notch = "supports pinch and the ribbon collapses").

**The construction (geometry).** T-FLEX, verbatim: in the notch approach *"the rounding is constructed in such a way that the element affected by the overflow is ignored. The rounding surface is not deformed in this case, and the surfaces of the notch element are trimmed or extended to the rounding"* **[verified, secondary]**. So the *blend surface stays undeformed* (still the exact analytic ribbon); it is the *model* that yields: the notch-element faces are **trimmed or extended to meet the rounding**. Parasolid's `ov_notch` *"extends the blend along all the faces in the remaining ('right') wall"* so the collapsed region is bridged rather than left as a gap **[verified, file 41]**. `PK_BODY_fix_blends` exposes `preserve_notch`: `false` draws a simple bridging curve within the blend surface to bridge a laminar notch; `true` projects the notch shape normally onto the blend surface to bridge while preserving the notch's form **[verified]**.

**The topology surgery (notch bridge).** The collapsed side has its spring curve width going to zero at some `t = t_c`. The handling:

| Step | Operator | Effect | dV | dE | dF |
|---|---|---|---|---|---|
| 1 | `split_edge` | Imprint the bridging curve's landing points on the remaining-wall faces. | +k | +k | 0 |
| 2 | `split_face` (`mef`) per remaining-wall face | Extend the blend along each face of the right wall, imprinting the projected notch shape (`preserve_notch=true`) or a simple bridge curve (`false`). | 0 | +m | +m |
| 3 | `kef` (xj) | Merge the bridged slivers into the continuing blend face so the collapsed side is absorbed, not left open. | 0 | -j | -j |
| 4 | attach | The blend surface is unchanged (undeformed); only its trim loop extends across the bridge. | 0 | 0 | 0 |

The defining property: **the surface is not rebuilt, the topology extends the blend's trim loop across the pinch.** This is why notch is last on the priority ladder: extending a trim loop across a collapsing region is geometrically delicate (the bridge curve can be near-degenerate), and `preserve_notch` exists precisely to let the caller choose fidelity (preserve the notch form) versus robustness (simple bridge).

**Patent posture.** The notch/grow-the-cap-by-extending-faces construction is the subject of **US 8,935,130 "Notch blends in BRep models"** (file 44 S14): concave blend constructed as a separate sheet and integrated by sewing, cap faces grown by *extending adjacent faces to meet new boundaries and trimming overlaps*. This is a **post-2006 patent that reads on the specific "construct-separate-sheet + sew + extend-faces-to-meet" mechanism.** Keel's design-around: the *outcome* (a valid bridged notch) is not claimable, but the specific *separate-sheet-then-sew* mechanism is. Keel should prefer the **in-place trim-loop extension** of the table above (extend the *existing* blend face's trim loop via `mef`/`kef` on the model faces, no separate sheet body), which is the file-44 in-place loop-regrow path and predates the patent's sheet-and-sew claim as ordinary Euler surgery (Stroud 2006; Mantyla 1988). Reserve the separate-sheet sew only for the genuinely multi-face cap where in-place regrow leaves transient invalidity, and there read US 8,935,130's claims element-by-element before implementing.

### 2.2 Cap

**What a cap is.** When neither smooth nor roll-on nor a clean cliff applies and a ribbon end is left open, a **cap** closes it. Parasolid forms the cap by **extending at most two of the adjoining faces to meet**, closing the ribbon end with existing-geometry faces rather than a synthetic patch **[verified, file 41 Section 1.6]**. ACIS, verbatim: *"when no model faces intersect the blend sheet, ACIS will attempt to extend existing model faces to close off the end of the blend"*, and a `cap` instruction tells the blend *"to begin capping the existing blend face rather than to roll onto the new entities"* **[verified, secondary]**. So cap is the explicit *stop-rolling-and-close* instruction, the dual of roll-on.

**The construction.** Extend up to two adjoining faces `Fa`, `Fb` until they intersect, imprint their intersection curve, and trim the ribbon end against the extended faces. ACIS, verbatim: *"a face or faces in the model will be intersected with the blend sheet to trim the sheet exactly to the model"* **[verified, secondary]**. Topology: `split_edge`/`split_face` to imprint the extended-face intersection on the ribbon end, then `mef` to close the cap loop, then `kef`/`kev` to dissolve any sliver. Net +1 cap region (often shared with an extended existing face, so frequently +0 new faces), watertight, non-tangent boundary at the cap. **Patent posture: cap-by-face-extension is old prior art** (extend two planes to meet is elementary); advanced capping was a Parasolid V13.0 *enhancement* but the basic mechanism is unencumbered.

---

## Section 3. Smooth overflow onto the adjacent face: the deepest case

This is the premium outcome and the deepest construction: the blend re-supports on the next face and continues *fully tangent*. Two sub-cases that the ladder (Section 0) separates: **roll-on** (the face beyond the tangent edge is an ordinary face) and **smooth** (the face beyond is itself a rounding the blend merges into).

### 3.1 Roll-on: re-support on the next face

**Geometry: how the spine and springs re-solve at the transition.** The spine is the intersection of the two offset surfaces `offset(F1, r) ∩ offset(F2, r)`. When spring curve `s2` reaches a tangent edge `b12` shared by `F2` and its tangent neighbor `F2'`, the contact transfers from `F2` onto `F2'`. **The spine re-solves by swapping one offset surface**: across the transfer parameter `t*` the spine changes from `offset(F1,r) ∩ offset(F2,r)` to `offset(F1,r) ∩ offset(F2',r)`. The extended-rolling-ball construction (Choi-Lee, US 5,774,359) formalizes the continuation: *"the intersection between an offset of the first surface and an envelope of the second surface"* is extended **across** the original face boundary so spine and spring curves continue smoothly onto the neighbor **[verified]**. Because `F2` and `F2'` are tangent along `b12`, the two offset surfaces `offset(F2,r)` and `offset(F2',r)` are *also* tangent along their corresponding offset of `b12`, so the spine is **G1 continuous through the transfer** (it kinks in *definition* but not in *tangent*). The `s1` spring curve on `F1` is continuous (its support never changed); only `s2` transfers, tracing `s2` on `F2` up to `t*` then `s2'` on `F2'` after.

**The topology of the transition curve.** At `t*` the rolling ball touches `F2` and `F2'` simultaneously *at the tangent edge `b12`*. The transition is marked by a single **cross edge** `x*` on the blend face running from the spine point at `t*` to the point where the ball straddles `b12`. The blend ribbon is split into two faces `B` (supported by `F1, F2`) and `B'` (supported by `F1, F2'`) joined along `x*`; `x*` lies *in the blend surface* (it is an isoparametric-like section curve, G1 within the blend) and lands on the model at the point where `s2` meets `b12`. Critically: **`x*` is a tangent (G1) seam inside the blend, not a sharp edge.** The blend surface itself can be kept as one face if the analytic type does not change (cylinder rolling onto a coplanar continuation stays one cylinder); `x*` is only forced when the support swap changes the surface type.

**The topology surgery (roll-on).**

| Step | Operator | Effect | dV | dE | dF |
|---|---|---|---|---|---|
| 1 | `split_edge` on `b12` | Imprint the point `p*` where `s2` meets the tangent edge `b12`. | +1 | +1 | 0 |
| 2 | `split_face` (`mef`) on `F2'` | Imprint the continued spring curve `s2'` onto the neighbor `F2'`. | 0 | +1 | +1 |
| 3 | `kef`/`kev` | Trim `F2`'s and `F2'`'s slivers back to `s2`, `s2'`. | -1 | -2 | -1 |
| 4 | `mef` (x2) | Build `B` (over `F1,F2`) and `B'` (over `F1,F2'`), joined along the in-surface cross edge `x*`. | 0 | +2 | +2 |
| 5 | `mef` realizing `x*` | If the analytic type changes across `t*`, split the ribbon at `x*`; else keep one face (no op). | 0 | +1 | +1 |
| 6 | radial splice | Pair all spring-curve coedges as twins of trimmed supports; tag `s1, s2, s2', x*` SMOOTH. | 0 | 0 | 0 |

The cascade case (file 41 Section 1.3) repeats steps 1 to 6 for each tangent edge crossed: roll from `F2` to `F2'` to `F2''`, one transfer cross edge per crossing, the spine piecewise with a tangent kink at each.

### 3.2 Smooth: merge into another rounding (deform the transition zone)

When the face beyond the tangent edge is *itself a rounding*, the outcome is `ov_smooth`: the blend *merges into* the other rounding. T-FLEX, verbatim: smooth overflow applies when *"the rounding surface smoothly merges into another rounding belonging to the solid body being processed. The surface is deformed in the transition zone in order to provide tangency to both the original face and the surface being merged into"* **[verified, secondary]**. Parasolid `ov_smooth`: *"deforms the rounding in the transition zone to stay tangent to both the original face and the surface it merges into"* **[verified, file 41]**.

**The key distinction from roll-on:** roll-on keeps the blend surface *exact* (it just re-supports on a flat/analytic neighbor); smooth **deforms the blend surface in a transition zone** to achieve tangency to a *second curved rounding*. This deformation is what takes smooth out of the exact-analytic regime for the *transition patch* specifically (the bodies away from the transition stay analytic). The transition patch is a G1 (ideally G2, per the Pegna-Wolter linkage-curve condition, file 40) blend-of-blends: a small surface tangent to both roundings along their respective merge curves. This is a **vertex-blend-like n-sided fill** (file 40 vertex blends; file 26 transfinite surfacing supplies the surface).

**Topology of the smooth transition:** the two roundings each terminate at a merge curve; the transition patch fills the lune between them, tangent to both. Operator-level it is the same n-sided-patch insertion as a vertex blend: imprint the two merge curves (`split_edge`/`split_face`), retract each rounding to its merge curve, and insert the transition patch with one `mef` per boundary edge closing the n-sided loop. The patch surface is *not analytic*; it is the procedural-evaluator case (Section 4 bucket b): a G1/G2 fill sampled and certified, then NURBS-fit by Keel's existing certified-evaluator-to-NURBS pipeline.

**Patent posture.** The extended-rolling-ball roll-on is **US 5,774,359 (Choi-Lee / Autodesk, filed 1995)**. Filed 1995, this patent has **expired** (20-year term from filing well past), so its method is now **free prior art** Keel may implement directly. This is the single most important patent-timing fact in this dossier: the canonical roll-on construction is unencumbered. The `ov_smooth` deform-the-transition-zone is a *behavior* (achieve tangency to both); the specific deformation algorithm is vendor-internal and undocumented, so Keel should build the transition as a standard tangent n-sided fill (file 26/40 machinery, classical) rather than reverse-engineering any vendor claim. **No live patent is implicated** if Keel uses the expired extended-rolling-ball for roll-on and its own tangent-fill for the smooth transition patch.

---

## Section 4. The three-bucket catalog (the highest-value ask)

For every overflow configuration, which bucket does the *handling* fall into?

### Bucket (a): EXACT ANALYTIC handling

The blend surface and its overflow trim stay in Keel's closed-form catalog (cylinder/cone/torus/conic/quadric); the overflow changes only the *trim boundary* or *swaps one analytic offset surface*, never the surface family.

- **Cliff overflow, full regime (F2/F5).** The analytic ribbon (cylinder for line spine, torus for circular spine) is **clipped by the plane or surface containing the cliff edge** and trimmed to the existing model edge `b`. Surface unchanged; only the `F2`-side trim boundary becomes a model edge. **Exact.**
- **Roll-on onto a coplanar / co-analytic neighbor (F3).** A cylinder blend rolling from `F2` onto a *coplanar* `F2'`, or onto a `F2'` that yields the same analytic offset, keeps the spine as one analytic curve and the blend as one analytic surface. The classic case: a cylindrical fillet overflowing a perpendicular planar face onto the next coplanar planar configuration (Section 6). **Exact.**
- **Notch with a simple planar bridge (F6, `preserve_notch=false`).** The undeformed analytic ribbon plus a planar bridge sliver; bridge is a line/plane trim. **Exact** when the remaining wall is planar.
- **Cap by extending two planar faces to meet (F2/F5 termination).** Plane-plane intersection is a line; the cap is exact. **Exact** for planar caps.

### Bucket (b): PROCEDURAL-EVALUATOR (sample-and-certify NURBS fit can cache)

The handling has a well-defined pointwise evaluator (the ball position is solvable at each `t`) but no closed-form surface; Keel's certified-evaluator-to-NURBS pipeline samples and certifies, then caches the fit.

- **Creeping cliff (ball rolling on top of the constraining edge, F5 variant).** The ball constrained to one surface and one *curve* sweeps a degenerate canal surface; evaluable at each `t` (solve the two-contact constraint) but not generally analytic. **Procedural-evaluator.**
- **Smooth merge transition patch (F3-into-rounding, `ov_smooth`).** The deform-to-tangent transition patch is a G1/G2 n-sided fill: evaluable (tangent constraints to both roundings define it pointwise) but not analytic. **Procedural-evaluator.**
- **Roll-on onto a curved (non-co-analytic) neighbor (F3).** When `F2'` is a different curved surface, the spine `offset(F1,r) ∩ offset(F2',r)` is a general intersection curve and the blend a general canal; evaluable, not analytic. **Procedural-evaluator.**
- **Notch bridge with projected notch shape (F6, `preserve_notch=true`).** The projected notch curve is an evaluable projection, the bridge a general trim. **Procedural-evaluator.**

### Bucket (c): GENUINELY general machinery

Requires the full overflow apparatus (multi-support spine re-solve plus self-intersection trimming plus general sewing), no shortcut.

- **Cascading roll-on across a chain of differing curved faces (F3 cascade).** Each transfer re-solves the spine on a new offset surface, with the blend possibly self-intersecting between transfers; needs the Pekerman-Elber self-intersection trim (subdivide at self-intersections, purge segments closer than `r`) plus per-segment re-fit. **General.**
- **Mixed-convexity overflow ends (F4/F10, file 44 Q5).** One support convex, one concave at the same end: the cap has mixed sense, in-place regrow is unsafe, and the construct-and-sew path with tolerant tangent seams is required. **General.**
- **Blend-blend interference resolved by mitring (F9) and vertex overflow with setback (F10).** Multi-ribbon reconciliation, n-sided vertex patches, autosetback. **General.**
- **Swallowed-feature route-around (F8) where the feature must be preserved.** The ribbon must carry *around* a hole/boss as an inner trim loop (needs `kemr`/`mekr` ring operators), with general re-trimming. **General.**

**Three-bucket verdict (the headline):** **(a)** cliff-clip, co-analytic roll-on, planar notch-bridge, planar cap, all stay exact, this is where Keel should land its first milestones; **(b)** creeping cliff, smooth-merge transition, curved roll-on, projected notch, all reduce to one pointwise evaluator Keel already knows how to sample-certify-and-cache; **(c)** cascading curved roll-on, mixed-convexity ends, blend-blend mitre, swallowed-feature route-around, are the genuine general-machinery tail to DECLINE until the analytic and procedural buckets are solid.

---

## Section 5. The cheapest complete predicate set (which handler applies)

Keel today detects overflow by spring-line containment and declines. To *choose a handler* it needs to classify the crossing. The cheapest *complete* set (complete = distinguishes all of smooth / roll-on / cliff / cliff_end / notch / cap) is **four predicates evaluated only at the crossing**, reusing data Keel already computes:

1. **Tangency-of-crossed-edge predicate (`is_tangent(b)`).** Is the support boundary edge `b` that the spring curve crosses a G1 (smooth) edge? This is a single dihedral-angle test between the two faces meeting at `b`: `|n_left . n_right - 1| < tol`. Keel already has edge convexity attributes (file 44 `sigma`); a SMOOTH/tangent tag on `b` answers this with **zero new geometry**. *Splits {smooth, roll-on} (tangent) from {cliff, cliff_end, notch, cap} (non-tangent).*

2. **Face-beyond-is-rounding predicate (`is_blend_face(F2')`).** Of the tangent branch, is the face beyond `b` itself a blend/rounding surface? Keel has blend *recognition* already (the unblend pipeline). A recognized-blend flag on `F2'` answers this with **zero new geometry**. *Splits smooth (rounding beyond) from roll-on (ordinary face beyond).*

3. **Convexity-of-crossed-edge predicate (`is_convex(b)`).** Of the non-tangent branch, is `b` convex (a sharp drop, cliff) or does the ribbon side collapse (notch)? The convexity sign `sigma` of `b` (file 44) plus the spring-width-to-zero test (one scalar: does the cross-section width vanish at this `t`?). Both are already computed in tracing. *Splits {cliff, cliff_end} (convex drop) from notch (width to zero); cap is the residual when neither a clean drop nor a pinch is present.*

4. **End-vertex-coincidence predicate (`at_end_vertex(t)`).** Of the cliff branch, does the crossing parameter coincide with the blend's terminating end vertex? One parameter comparison `|t - t_end| < tol`. *Splits cliff from cliff_end.*

**Completeness argument.** The four predicates form a decision tree whose leaves are exactly {smooth, roll-on, cliff, cliff_end, notch, cap}, matching the priority ladder of Section 0. Predicate 1 is the root (tangent vs not); 2 refines the tangent branch; 3 refines the non-tangent branch; 4 refines the cliff branch; cap is the explicit residual leaf. **Cheapness argument:** predicates 1, 2, 3 reuse attributes Keel *already maintains* (edge tangency tag, blend-recognition flag, convexity sign, spring-width during tracing); only predicate 4 is a fresh scalar comparison. No medial-axis query, no new surface intersection: the *detection* (where overflow happens) already came free from spring-line containment (file 41 Section 3.2); the *classification* (which handler) is four cheap tests at that one crossing point. This is strictly cheaper than re-deriving the medial field, because classification is local to the crossing edge whereas the `r_max(t)` predictor (file 41 Section 2.6) is global, the predictor answers *whether* to handle, this set answers *how*.

---

## Section 6. Overflow between planar supports that stays analytic

The team's specific question: which overflow cases between planar supports stay analytic? This is bucket (a), made concrete for the all-planar world that dominates mechanical CAD.

**The canonical analytic-overflow configuration.** A constant-radius fillet on the edge between two perpendicular planes `P1` and `P2` is an **exact cylinder** of radius `r`, axis parallel to the edge, tangent to both planes (Keel already carries this, file 40). Now let `P2` be *finite* and end at a tangent or sharp boundary onto a third plane `P3`:

- **`P3` coplanar with `P2` (the trivial roll-on).** The fillet cylinder continues unchanged onto `P3`: same plane, same offset, same cylinder. Spine is one straight line; blend is one cylinder. **Fully exact, zero re-solve**, the spring curve `s2` simply continues straight across the (artificial) `P2`/`P3` boundary. This is the cheapest possible overflow handle: do nothing to the surface, only re-imprint the spring curve across the boundary.

- **`P3` perpendicular to `P2`, `P2` ends in a sharp convex edge (cliff onto a planar drop).** The cylinder is **clipped by the plane `P3`** and trimmed to the sharp edge `P2 ∩ P3`. The clip of a cylinder by a plane is a conic (ellipse) section curve, *exact and closed-form*. The blend stays a cylinder; the cliff trim boundary is an exact ellipse arc. **Fully exact.** This is the planar cliff, bucket (a).

- **Roll-on onto a perpendicular plane `P2'` tangent to `P2` is impossible** (perpendicular planes are not tangent), so a perpendicular neighbor is always a *cliff* (sharp), never a roll-on, never a smooth. This is why **between planar supports the only non-trivial analytic overflow is the cliff-clip**: coplanar continuation is trivial roll-on (no re-solve), and any non-coplanar planar neighbor is non-tangent, hence cliff, hence an exact conic-clip. **There is no procedural-evaluator or general case purely between planes** until a *curved* face enters or a *pinch* (notch) or *vertex* congestion appears.

**Net statement for planar supports:** overflow between planar supports is *always* either (i) trivial coplanar roll-on (re-imprint spring curve, surface unchanged) or (ii) cliff-clip of the analytic blend by the neighboring plane (exact conic trim). Both are bucket (a), exact. **Planar-support overflow never needs procedural or general machinery.** This is a strong, testable invariant and the right first conquest (Section 7).

---

## Section 7. The single most common case, worked fully

### 7.1 Which case, and why

**The single most common overflow in CAD practice is the cliff overflow of a constant-radius fillet between two planar (or one planar, one cylindrical) supports onto a perpendicular planar face: "the fillet is wider than the face and runs off the end onto a sharp drop."** Justification:

1. **Mechanical parts are overwhelmingly planar-and-cylindrical** (machined blocks, brackets, housings); the dominant blended edge is a prismatic edge between two planes, and the dominant *failure* is the fillet running off a short transverse face (file 41 Section 1.2, the "fillet wider than the face").
2. **The neighbor a planar fillet runs onto is almost always perpendicular, not tangent** (box corners, steps, pockets), so by Section 6 the overflow is a *cliff*, not a roll-on or smooth. Roll-on and smooth require tangency, which is the exception in machined geometry, not the rule.
3. **It stays exact** (Section 6): it is the highest-value-per-effort handle, the most common case *and* in the cheapest bucket. Smooth-into-rounding (Section 3.2) is deeper but rarer; notch and vertex overflow are rarer still. The cliff between planar/cylindrical supports is the Pareto-dominant first conquest.

So the worked case: **constant-radius `r` cylinder fillet on the convex edge `e` between plane `P1` (the wall, tangent side) and plane `P2` (the floor), where `P2` is finite and ends at a sharp convex edge `b = P2 ∩ P3` with `P3` perpendicular (the cliff drop).** The fillet is too wide: its spring curve `s2` on `P2` would cross `b`. Handler: **cliff**.

### 7.2 Geometry

- Blend surface `B`: a circular cylinder of radius `r`, axis the line `L` parallel to `e` at distance `r` from both `P1` and `P2` (Keel's exact line-spine-to-cylinder, file 40). Spine = `L` (a straight line).
- Spring curve `s1` on `P1`: a straight line parallel to `e` at distance `r·` (the contact line on the wall). Kept entire.
- Natural spring curve `s2` on `P2`: a straight line parallel to `e`; but it crosses the cliff edge `b`. **Replaced** by the cliff trim.
- Cliff trim boundary: the cylinder `B` intersected with plane `P3` is an **ellipse** (a plane cutting a cylinder obliquely; here `P3 ⟂ P2` cuts the cylinder in an ellipse arc) `c3 = B ∩ P3`, exact and closed-form. The blend's `P2`-side boundary becomes: `s2` from the start of the blend up to where `s2` would meet `b`, then follows `b` (the sharp edge) where the cylinder overhangs `P3`. The portion of the cylinder beyond `P2` (over the `P3` drop) is **clipped off at `b`** and the blend terminates along `b`, leaving `b` sharp.

The blend surface stays an **exact cylinder**; only its `P2`-side trim loop changes from "straight spring line `s2`" to "spring line `s2` then sharp edge `b`." This is bucket (a), exact, no NURBS, no evaluator.

### 7.3 Topology surgery with full V/E/F/R bookkeeping

Initial local configuration (the relevant corner before blending): three planar faces `P1` (wall), `P2` (floor), `P3` (drop) meeting at the convex sharp edges `e = P1 ∩ P2`, `b = P2 ∩ P3`, `d = P1 ∩ P3`, sharing the box corner vertex `v0`. Assert `V - E + F = 2(S - G) + (L - F)` after every step.

| Step | Operator | What it does | dV | dE | dF | dL |
|---|---|---|---|---|---|---|
| 1 | `split_edge` on `e` (x2) | Place the two endpoints `p1s`, `p1e` of spring curve `s1` along edge `e`'s bounding edges on `P1`. | +2 | +2 | 0 | 0 |
| 2 | `split_edge` on `b` | Place point `q` where the cylinder's clip ellipse `c3` (equivalently where `s2` would have met `b`) lands on the cliff edge `b`. | +1 | +1 | 0 | 0 |
| 3 | `split_face` (`mef`) on `P1` | Imprint `s1` across `P1`, separating wall keep-strip from the trim sliver between `s1` and `e`. | 0 | +1 | +1 | +1 |
| 4 | `split_face` (`mef`) on `P2` | Imprint `s2` (the floor spring line, up to `q`) across `P2`, separating floor keep-region from the trim sliver between `s2` and `e`. | 0 | +1 | +1 | +1 |
| 5 | `kef` + `kev` | Dissolve `P1`'s trim sliver into the opening gap (`kef` merges, `kev` removes the freed spur vertex). | -1 | -2 | -1 | -1 |
| 6 | `kef` + `kev` | Dissolve `P2`'s trim sliver likewise. | -1 | -2 | -1 | -1 |
| 7 | `kef` + `kev` | Remove the sharp blended edge `e` and its now-spur vertices, opening the gap between `s1` and `s2`. | -2 | -3 | -1 | 0 |
| 8 | `mef` | Build the blend face `B`: loop = `s1` (twin of `P1` keep coedge) + cross edge at the start + `s2` (twin of `P2` keep coedge, up to `q`) + the cliff edge segment `b` from `q` (the second use of existing edge `b`) + cross edge at the end. This single `mef` creates `B`. | 0 | +1 | +1 | +1 |
| 9 | radial splice | Pair `B`'s `s1` and `s2` coedges as twins of the trimmed `P1`, `P2` keep coedges (tag SMOOTH); make `B`'s `b`-segment coedge the **second radial use of the existing sharp edge `b`** (so `b` is now shared by `P3` and `B`, sharp on `P3` side). | 0 | 0 | 0 | 0 |
| 10 | attach | Attach the exact cylinder surface to `B`; trim its `P2`-side boundary to `s2`-then-`b`; the clip ellipse `c3 = B ∩ P3` is implicit in the `b`-segment trim. | 0 | 0 | 0 | 0 |

**Aggregate deltas (steps 1 to 8):** dV = +2 +1 +0 +0 -1 -1 -2 +0 = **-1**; dE = +2 +1 +0 +1 +1 -2 -2 -3 +1 = **-1**; dF = 0 +0 +0 +1 +1 -1 -1 -1 +1 = **0**; net L tracks F (R = L - F unchanged for this simple, ring-free, handle-free surgery). The Euler-Poincare invariant `V - E + F = 2(S - G) + (L - F)` is preserved at every line because each operator is a GWB make/kill pair member with a fixed validity-preserving delta (file 01 Section 3.2). S and G are unchanged (no shell split, no handle); R is unchanged (no inner ring: the cliff trim adds a *boundary segment* to `B`'s single outer loop, not an inner loop). The clean global statement: **one blend face added, the cliff edge `b` promoted from a `P2/P3` boundary to a `P3/B` shared edge, body stays a valid closed solid, `mass == mesh` provable because the surface is an exact cylinder clipped by exact planes.**

**Why this is the right first milestone:** the surgery is the *convex baseline* of file 44 (cap-shrink dissolve via `kef`/`kev`) with the single modification that the `P2`-side cap boundary follows the existing cliff edge `b` instead of a fresh end-arc. It reuses Keel's existing trim-and-stitch single-edge blend surgery *verbatim*, changing only the `F2`-side trim target. The oracle is exact (Section 7.4).

### 7.4 The exact oracle (Keel's test contract)

The worked case has a **closed-form volume**, so it admits Keel's `mass == mesh == ref` exact-triplet gate:

- `ref`: the unblended box-corner volume minus the exact fillet-cut volume. The fillet cut from a 90-degree convex edge of length `Lc` is the prism `(1 - π/4) r² · Lc` (the square-minus-quarter-circle cross-section swept along the edge). The cliff truncates the fillet at the ellipse `c3`; the truncated portion's volume is a closed-form solid (cylinder segment cut by two perpendicular planes), computable exactly.
- `mass`: Keel's mass-properties on the resulting B-rep (exact cylinder + planes), folded with face senses (the resolved massprops sense/region machinery, memory note).
- `mesh`: the tessellated volume, which must match within tessellation tolerance (watch the curved-boundary chord-vs-arc trap, memory note: the cliff ellipse must be sampled as an arc, not a chord, or the mesh volume will mis-attribute).

The differential test from file 41 Section 6 applies: sweep the fillet radius `r` from below to above the cliff-triggering value; below, full fillet (no overflow); at the trigger, `s2` exactly reaches `b`; above, the cliff handler fires and the body stays valid with `mass == mesh == ref` at every `r`. The trigger radius is itself an oracle (it equals the `P2` transverse extent, the LFS at that section, file 41).

---

## Section 8. How Parasolid's overflow options map (handled with patent care)

Parasolid exposes overflow as named behaviors on `PK_EDGE_set_blend_*` and fixes them via `PK_BODY_fix_blends`. The mapping onto Keel's constructions, with patent posture:

| Parasolid control | Keel construction | Bucket | Patent posture |
|---|---|---|---|
| `ov_smooth` | Smooth merge into rounding: deform transition patch tangent to both (Sec 3.2); or roll-on for non-rounding tangent neighbor (Sec 3.1) | (b) transition / (a) co-analytic roll-on | Roll-on via expired US 5,774,359 (free); transition patch via classical tangent n-sided fill. No live patent. |
| `ov_cliff` | Cliff clip: analytic blend trimmed to existing sharp edge `b`, half-tangent (Sec 1) | (a) exact | Old prior art (trim swept surface to boundary). Unencumbered. |
| `ov_cliff_end` | Cliff localized at terminating end vertex (Sec 1) | (a) exact | Unencumbered. |
| `ov_notch` | Notch bridge: undeformed blend, extend remaining-wall faces to the rounding (Sec 2.1); `preserve_notch` for simple vs projected bridge | (a) planar bridge / (b) projected | **US 8,935,130 reads on separate-sheet-sew.** Keel uses in-place trim-loop extension (file 44 Euler surgery, predates the claim). Design-around required. |
| cap (V13.0 advanced capping) | Cap by extending up to two adjoining faces to meet (Sec 2.2) | (a) planar | Basic cap is old prior art; advanced capping is a vendor enhancement, not a blocking patent for the basic mechanism. |
| `propagate` | Roll-on across tangent edges / past blends at would-be-invalid 3-edge vertices (Sec 3.1); invalid for cliff | (a)/(b)/(c) | Behavior, not a claimable mechanism. Implement via expired extended-rolling-ball. |
| `preserve_notch` (`PK_BODY_fix_blends`) | Fidelity switch on the notch bridge: simple bridging curve (false) vs projected notch shape (true) | (a)/(b) | Option semantics, not patented. |
| auto / priority order | The Section 0 ladder: smooth, then cliff, then notch, then cap | mixed | The *ordering* (T-FLEX-documented) is not claimable; implement the ladder directly. |

**General patent doctrine for this area (the team's caution):** Parasolid's overflow *controls* are a likely patent-sensitive area, but the named behaviors are *outcomes* described in public docs, not claimable algorithms; the claimable artifacts are the *specific construction patents* (US 8,935,130 notch sheet-sew, post-2006, **live, design around**; US 5,774,359 extended rolling ball, pre-1996, **expired, free to use**). Keel's safe path: implement cliff/cap/roll-on from the **expired or never-patented** constructions, implement notch via **in-place Euler surgery** (not the patented separate-sheet-sew), and read US 8,935,130 claim-element-by-claim-element before ever building a separate-sheet notch path. Prefer the analytic and in-place constructions throughout, which moots most patent exposure because they are classical.

---

## Synthesis: ADOPT / AVOID / INVESTIGATE, first milestone, ladder, decline-forever

### ADOPT

- **The Section 0 priority ladder** (smooth, then cliff, then notch, then cap) as the overflow-handler dispatcher, evaluated by the four cheap predicates of Section 5. This is the documented production behavior and reuses attributes Keel already maintains.
- **Cliff-clip as the workhorse** (Section 1, Section 7): trim the exact analytic blend to the existing model edge, half-tangent, surface unchanged. Bucket (a), exact, the most common case.
- **Co-analytic roll-on** (Section 3.1, Section 6) via the **expired** extended-rolling-ball (US 5,774,359, free): swap one offset surface across the tangent transfer, keep the blend analytic when the neighbor is co-analytic.
- **In-place notch and cap via Euler surgery** (Section 2): extend the *existing* blend's trim loop / extend up to two model faces to meet, no separate sheet body, sidestepping US 8,935,130.
- **The exact-oracle contract** (Section 7.4): every analytic-overflow handle must pass `mass == mesh == ref` with the cliff ellipse sampled as an arc.

### AVOID

- **The separate-sheet-then-sew notch mechanism** as a *first* implementation: it reads on live US 8,935,130 and is heavier than in-place regrow. Reserve it for genuinely multi-face caps, and only after a claim read.
- **Reverse-engineering the `ov_smooth` transition-zone deformation** from any vendor: build the transition as a classical tangent n-sided fill instead.
- **The medial-axis predictor as a *classifier*.** It answers *whether* to handle (file 41), not *which* handler; the four local predicates (Section 5) are far cheaper for the *which*.

### INVESTIGATE

- **Creeping cliff** (ball on the constraining edge, bucket b): the degenerate canal evaluator and whether Keel's certified-evaluator-to-NURBS pipeline fits it cleanly. Defer until full-cliff is solid.
- **Smooth merge into a rounding** (Section 3.2, bucket b): the tangent transition patch and its G2 (Pegna-Wolter) quality. Deeper, rarer, second-wave.
- **Curved-neighbor roll-on** (bucket b): general-canal spine re-solve and the Pekerman-Elber self-intersection trim.

### Recommended FIRST MILESTONE (narrowest honest slice with an exact oracle)

**The planar/cylindrical cliff overflow of Section 7: a constant-radius cylinder fillet between two planar supports running off the floor onto a perpendicular planar drop, handled by clipping the cylinder to the sharp cliff edge.** This is the single most common overflow, stays fully exact (bucket a), reuses Keel's existing trim-and-stitch single-edge surgery verbatim (only the `F2`-side trim target changes), and has a closed-form `ref` volume for the `mass == mesh == ref` gate. The oracle is the radius sweep through the cliff-trigger radius (Section 7.4). Ship this, with the four-predicate classifier (Section 5) wired but only the cliff branch active, before any procedural or general work.

### Follow-up ladder (in order)

1. **Co-analytic planar roll-on** (Section 6 coplanar continuation): trivial re-imprint, surface unchanged, exact. Adds the smooth branch's easy half.
2. **Planar notch bridge** (`preserve_notch=false`, bucket a): in-place Euler extension, planar bridge, exact.
3. **Planar cap** (extend two planes to meet, bucket a): the universal watertight fallback, exact.
4. **Creeping cliff and curved roll-on** (bucket b): certified-evaluator-to-NURBS, the procedural wave.
5. **Smooth merge transition patch** (bucket b): tangent n-sided fill into a rounding.
6. **Mixed-convexity ends, blend-blend mitre, vertex setback, swallowed-feature route-around** (bucket c): the general tail.

### Configuration to DECLINE forever (with justification)

**Cascading roll-on across a long chain of differing curved faces with intermediate self-intersection, combined with mixed-convexity vertex ends (the bucket-c tail when it co-occurs).** Justification: this requires simultaneous general spine re-solve on each new offset surface, Pekerman-Elber self-intersection trimming between transfers, *and* mixed-sense vertex patching, with no exact oracle and no procedural shortcut. It is rare in practice (it needs a chain of tangent curved faces of differing type meeting a mixed-convexity vertex, an unusual confluence), it is where even production kernels emit blend errors, and Keel's honest-decline doctrine (file 29, file 41 Section 4.6) covers it correctly: return the valid unblended body plus a precise per-interval diagnostic rather than a fragile or wrong result. Declining it forever costs almost nothing in coverage and removes the unbounded-complexity tail; the medial predictor already flags it cheaply (file 41), so the decline is *predicted*, never *thrown*.

**The honest one-line summary:** the cliff between planar/cylindrical supports is the most common overflow and is exact, so handle it first and exactly; roll-on (via the expired extended-rolling-ball), planar notch, and planar cap follow as exact or in-place Euler constructions; the curved/smooth/creeping cases reduce to one pointwise evaluator Keel already samples-and-certifies; and the cascading-curved mixed-convexity tail is declined forever with a predicted, disclosed diagnostic, never silently.

---

## References

1. Vida, J., Martin, R. R., & Varady, T. (1994). *A survey of blending methods that use parametric surfaces.* Computer-Aided Design, 26(5), 341-365. https://www.sciencedirect.com/science/article/abs/pii/001044859490023X ; SZTAKI GML 1992/4: https://eprints.sztaki.hu/682/
2. Choi, B. K., & Lee, C. S. (1998). *Method for generating an extended rolling ball fillet surface between two surfaces.* US Patent 5,774,359 (Autodesk), filed 1995, **expired**. https://patents.google.com/patent/US5774359A/en ; PDF: https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/5774359
3. *Notch blends in BRep models.* US Patent 8,935,130 (post-2006, **live**, design-around required). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8935130
4. Siemens / Parasolid. *Edge Blending: Overview; Functions and Options; Edge Blend Overflows; PK_BODY_fix_blends* (`ov_smooth`, `ov_cliff`, `ov_cliff_end`, `ov_notch`, `propagate`, `preserve_notch`, cap-by-face-extension; "cliff edge blend ... tangent to only one of the faces ... runs along an edge in the other face"). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html ; .31.html ; PK_BODY_fix_blends: http://www.q-solid.com/Parasolid_Docs/headers/pk_body_fix_blends.html ; V35 Blending overview: http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.11.html
5. T-FLEX CAD. *Blending Edges* (verbatim smooth/cliff/notch construction mechanisms; priority order "first smooth, then cliff, then notch"; ball rolling on top of the constraining edge; keep all topological entities by trimming or extending). https://www.tflex.com/help/eng/T-FLEX%20CAD/16/blending_edges.htm ; *Blending Faces*: https://www.tflex.com/help/eng/T-FLEX%20CAD/15/blending_faces.htm
6. Spatial / ACIS. *Sequences of Blended Edges; Vertex Blends; Variable Radius Blending; Standard Blending Error Messages* (cap instruction vs roll-on; "extend existing model faces to close off the end of the blend"; "intersect a face or faces in the model with the blend sheet to trim the sheet exactly"; spring-curve intersection at vertices; setback/autosetback; mixed-convexity capping as the hard case). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblndse.htm ; Vertex Blends: http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0010.HTM ; Blending Component: http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF ; Error messages: http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblnder.htm
7. Open CASCADE Technology. *ChFi3d_Builder Class Reference* (ExtentOneCorner/TwoCorner/ThreeCorner extend the spine of stripes at an extremity vertex; ChFiDS_State trim-vs-extend record). https://dev.opencascade.org/doc/occt-6.9.0/refman/html/class_ch_fi3d___builder.html ; ChFi3d (ConcaveSide/SameSide): https://dev.opencascade.org/doc/occt-6.9.1/refman/html/class_ch_fi3d.html ; FilBuilder/ChBuilder: https://dev.opencascade.org/doc/occt-7.5.0/refman/html/class_ch_fi3d___ch_builder.html
8. Pekerman, D., Elber, G., & Kim, M.-S. (2008). *Self-intersection detection and elimination in freeform curves and surfaces.* Computer-Aided Design, 40(2), 150-159 (binormal-line criterion; subdivide at self-intersections, purge offset segments closer than `r` than the original). https://www.sciencedirect.com/science/article/pii/S0010448507002357 ; https://dl.acm.org/doi/10.1016/j.cad.2007.10.004
9. Farouki, R. T. (1992). *Rolling ball blends and self-intersection* (SPIE) / *The approximation of non-degenerate offset surfaces.* https://www.spiedigitallibrary.org/conference-proceedings-of-spie/1830/0000/Rolling-ball-blends-and-self-intersection/10.1117/12.131744.short
10. *Fillet and surface intersections defined by rolling balls.* Computer-Aided Design / CAGD (1992). https://www.sciencedirect.com/science/article/abs/pii/016783969290016I
11. Braid, I. C. (1997). *Non-local blending of boundary models.* Computer-Aided Design, 29(2), 89-100 (ROMULUS/Parasolid lineage: roll-on and blend-blend as non-local edits). https://www.sciencedirect.com/science/article/abs/pii/S0010448596000383
12. Whited, B., & Rossignac, J. *Relative blending.* Georgia Tech. https://faculty.cc.gatech.edu/~jarek/papers/relativerounding.pdf
13. Rossignac, J. R., & Requicha, A. A. G. (1986). *Constant-radius blending in solid modelling.* Computers in Mechanical Engineering. https://www.semanticscholar.org/paper/CONSTANT-RADIUS-BLENDING-IN-SOLID-MODELLING-Rossignac-Requicha/553014d71e15e44f3585377bc9fd017d4f823b3d
14. Pegna, J., & Wolter, F.-E. (1992). *Geometrical criteria to guarantee curvature continuity of blend surfaces* (Linkage Curve theorem). https://www.semanticscholar.org/paper/Geometrical-Criteria-to-Guarantee-Curvature-of-Pegna-Wolter/1e2c0f2f1ae3f562587bd86b68421141dbef8c13
15. Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer (Euler-operator recipes for blending/chamfering as local boundary edits). https://link.springer.com/book/10.1007/978-1-84628-616-2
16. Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press (GWB operator set, fixed deltas, Euler-Poincare preservation). https://archive.org/details/introductiontoso0000mant
17. SM38 / IIT-B. *Removal of Blends from Boundary Representation Models* (blend recognition/unblend, feature angles; the inverse direction informing detection). https://www.cse.iitb.ac.in/~sohoni/delblend.pdf
18. Patrikalakis, N. M., Maekawa, T., & Cho, W. *Shape Interrogation for Computer Aided Design and Manufacturing*, Ch. 11.3 (offset singularities and self-intersection tracing, underlying the curved roll-on trim). https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node225.html
19. Onshape / SOLIDWORKS / NX. *Edge fillet overflow controls* ("Allow edge overflow"; conic constant-elliptical cross-section; smooth-chain overflow), corroborating the public overflow-option vocabulary across CAD UIs. https://cad.onshape.com/help/Content/PartStudio/fillet.htm ; https://www.onshape.com/en/resource-center/tech-tips/fillet-face-blend-surface-design-tools
20. *An Efficient Algorithm for Recognizing and Suppressing Blend Features.* Computer-Aided Design and Applications, 1(1-4), 421-428 (2004) (blend recognition predicates that double as overflow-classification inputs). https://www.cad-journal.net/files/vol_1/CAD_1(1-4)_2004_421-428.pdf

## Related Keel dossiers (build on, not repeated)

- `41-blend-overflow-radius-feature-failures.md`: the failure catalog F1 to F11, detection predicates, and the medial-axis `r_max(t) = lfs(spine(t))` ceiling. **This dossier consumes that and supplies the handling constructions; it does not re-derive detection.**
- `44-concave-fillet-topology-surgery.md`: the cap-shrink (convex) vs cap-grow (concave) Euler surgery, the convexity sign `sigma`, and the construct-and-sew fallback. The cliff/notch/cap surgeries here reuse its operator vocabulary and the `sigma` branch.
- `40-blend-fillet-surface-generation.md`: the spine-as-offset-intersection, the exact-analytic surface catalog (line to cylinder, circle to torus, quadric pairs to Dupin cyclide, else NURBS canal), variable-radius, vertex blends. The overflow handlers trim and re-support *these* surfaces.
- `28-fillet-chamfer-engine.md`: the engine-level smooth/cliff/notch overflow menu and blend-graph sequencing this dossier makes constructive.
- `29-dirty-geometry-graceful-degradation.md`: the best-effort-with-disclosure doctrine the decline-forever and partial-handle policies plug into.
- `39-coincident-tangent-face-booleans.md`: the two-sided tangent-sew and tolerant-edge treatment reused at the SMOOTH spring-curve and transition seams of roll-on and smooth overflow.
