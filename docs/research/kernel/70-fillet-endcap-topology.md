# 70. Fillet end-cap topology beyond Single and Roof: the multi-face cap and the parallel-cap extend on compound bodies

**Title and scope.** This dossier is the implementation-grade design for the two end-cap topologies Keel's fillet engine currently declines: the **multi-face cap** (the blend tube ends at a model vertex where K >= 3 non-support faces meet) and the **parallel-cap extend** (the end cap is parallel to the spine, so the blend must lengthen to reach it, the classic blend overflow). These are the dominant fillet decline class on **compound** (post-boolean) bodies: after the first boolean, a filleted seam edge routinely terminates at a peg-meets-block corner (3+ faces) or at an end cap parallel to the seam, so the clean-primitive Single and Roof cases barely occur. This file makes the runout/overflow family of dossiers 41/54/56 concrete for `split_cap` (blend.rs:765), giving the exact surgery, the GWB Euler-operator sequence with V/E/F bookkeeping that generalizes the `kef`/`kev` dissolve at blend.rs:865-889, the precondition under which each case is bounded versus DECLINE-forever, and an exact-oracle validation battery on compound bodies.

It builds on, and does not repeat, its siblings. The overflow *catalog* (F1 to F11, the medial-axis `r_max(t) = lfs(spine(t))` ceiling, detection predicates) is dossier 41. The overflow *handling constructions* (cliff-clip, notch bridge, cap-by-face-extension, roll-on via the expired extended-rolling-ball, the four-predicate classifier, the priority ladder smooth then cliff then notch then cap) are dossier 56, and this file consumes that vocabulary rather than re-deriving it. The *interior* mid-edge stop and the cone-sphere runout oracle are dossier 54. The vertex *setback/retraction* and *mitring* of converging blends are dossiers 53/55. The exact-analytic surface catalog (line spine to cylinder, circle spine to torus, the offset-intersection spine) is dossier 40, and the cap-shrink (convex) versus cap-grow (concave) base surgery with the convexity sign `sigma` is dossier 44. The GWB operator set (`mvfs`, `mev`, `mef`, `kev`, `kef`, `kemr`, `mekr`, `kfmrh`, `split_edge`, `split_face`) and the invariant `V - E + F = 2(S - G) + (L - F)` asserted after every step are fixed in dossier 01. The design recommendation at the close is to validate against Keel's exact-triplet (`mass == mesh == ref`) tests with the clean-primitive Single/Roof baselines held as a non-regression gate.

Labels: claims drawn verbatim from a primary doc are tagged **[verified]**; claims reconstructed from secondary material, from public behavior, or assembled by the author from the operator algebra are tagged **[claimed]**.

---

## Section 0. The state of `split_cap` and the exact gap to close

Keel's end-cap planner is `split_cap` (blend.rs:765). At each end vertex `v_corner` of the filleted edge it collects the **cap faces**: the faces at `v_corner` that are NOT the two trimmed supports `strip1`, `strip2` (blend.rs:770-774). It then matches on the count:

- `[cap]` -> **Single** (blend.rs:790-795): one transverse cap face. The blend cylinder's end-arc (`conic_for`) splits that one cap into a kept region and a sliver; `split_blend_cap` imprints the arc; the dissolve folds the sliver in.
- `[c0, c1]` -> **Roof** (blend.rs:796-851): two cap faces meeting at a **ridge**. The ridge edge is found (the edge shared by both caps and incident to `v_corner`, blend.rs:813-823), the first crossing of the ridge with the blend cylinder is solved as the smaller quadratic root (blend.rs:837-846, the corner is outside so the near root enters on the material quarter), a new vertex `vr` is imprinted there, and **each** cap trims to its own conic sub-arc (`arc_a` on `cap_a`, `arc_b` on `cap_b`, blend.rs:847-850), the two arcs chaining at `vr`.
- `_` -> `Err(Precondition("fillet: multi-face cap (follow-up)"))` (blend.rs:853-855): **3+ cap faces, declined.**

The per-face conic is `conic_for` (blend.rs:~730-763): the blend end-circle intersected with the cap plane is a **circle** when the cap is perpendicular to the spine (`d.abs()` near 1, the arc axis branch, blend.rs:743-746), an **ellipse** when oblique (blend.rs:747-758, `radius / d.abs()` major axis, where `d = dir.dot(n_cap)` is the cosine between spine and cap normal), and when `d.abs()` is near zero, the cap is **parallel to the spine** and `conic_for` returns `Err("fillet: cap face parallel to the spine (extend follow-up)")` (blend.rs:759-763). The dissolve that turns the trimmed fragments into one blend face is `kef(edge)` then, per end, `kef(stub)` plus a Single `kev(spur)` or a Roof `kef(stub2)` + `kev(rspur)` (blend.rs:865-889).

The two gaps, exactly:

1. **Multi-face cap.** `split_cap` handles `caps.len() in {1, 2}`. The generalization to `K >= 3` is a **chain of K conic sub-arcs** around the tube end, one per crossed cap face, joined at K imprinted crossing vertices, with a dissolve that is the Roof pattern iterated K-1 times (Section 1). On a boolean body, a transversal peg into a block produces a corner where the seam edge ends at exactly such a 3-or-4-face vertex (Section 3).
2. **Parallel-cap extend.** When `conic_for` hits `d.abs() ~ 0` the rolling ball's end-circle never meets the cap (they are parallel), so there is no conic to trim against. The fix is to **lengthen the blend cylinder along the spine** until its swept body reaches the parallel cap, then trim against the now-reached cap (Section 2). This is the Parasolid/ACIS blend overflow.

Both gaps are *end-cap* problems: the blend *surface* stays the exact cylinder Keel already builds (blend.rs:898-902); only the *trim loop* at the tube end and the dissolve bookkeeping change. That is the load-bearing simplification this dossier exploits throughout.

---

## Section 1. The multi-face cap (3+ faces at the end vertex)

### 1.1 The geometry: Roof generalized to a chain of K conic sub-arcs

**Setup.** The blend cylinder `B` (radius `r`, axis `dir`, the exact line-spine cylinder of dossier 40) ends at the model vertex `v_corner`. The supports `strip1`, `strip2` are trimmed away. The remaining cap faces, in the order the tube boundary visits them, are `c_0, c_1, ..., c_{K-1}` (K >= 3). The blend's terminating cross-section is a **circle** `E` of radius `r` centered at `pc` (the corner point) in the plane normal to `dir` (the rolling-ball end-sphere's great circle on the cylinder). The cap surgery asks: where does each cap face cut that end region, and how do the cuts chain around the tube?

**The crossing structure.** Each cap face `c_i` is a portion of a model surface (plane or analytic) bounded by model edges incident to `v_corner`. Two consecutive caps `c_i`, `c_{i+1}` share a model edge `ridge_i` running out of `v_corner` (the generalization of the single Roof ridge, blend.rs:813). The blend cylinder crosses `ridge_i` at a parameter solved exactly as in Roof: project the ridge direction perpendicular to `dir`, intersect with the radius-`r` circle, take the near quadratic root (blend.rs:834-846). That crossing point is a new vertex `vr_i`. Between `vr_{i-1}` and `vr_i` the tube boundary lies **on cap face `c_i`**, and there it trims to a conic sub-arc:

- `conic_for(c_i, ...)` gives a **circular** arc when `c_i` is perpendicular to the spine (`|dir . n_i| ~ 1`),
- an **elliptical** arc when `c_i` is oblique (`0 < |dir . n_i| < 1`, major axis `r / |dir . n_i|`),
- and the **parallel** case (`|dir . n_i| ~ 0`) is the Section 2 extend, which a multi-face corner can also contain on one of its faces (handle per-face, Section 1.5).

So the tube end is closed by a **closed chain of K conic sub-arcs** `arc_0, arc_1, ..., arc_{K-1}`, where `arc_i` lies on `c_i` and runs from crossing vertex `vr_{i-1}` to `vr_i`, with the two extreme arcs anchored at the spring ends `a_end`, `b_end` (the points where the two spring curves arrive at the corner, blend.rs:780-789). This is **verified by construction** as the exact generalization of the two-arc Roof chain: Roof is the K = 2 special case (one ridge, two arcs, one new vertex `vr`), and Single is K = 1 (no ridge, one arc).

### 1.2 The ordering rule (which face the tube enters and exits)

The chain must be ordered so consecutive arcs share a crossing vertex and the whole chain closes from `a_end` to `b_end`. The ordering rule, made precise (the generalization of blend.rs:805-810's `holds` test):

1. **Anchor the ends.** `cap_a` is the cap face whose loop has a fin ending at `a_end` (the spring-a arrival, blend.rs:798-809's `holds`); `cap_b` is the one whose loop ends at `b_end`. These are the two extreme caps of the chain.
2. **Walk the ridge fan.** Starting from `cap_a`, repeatedly take the shared ridge edge incident to `v_corner` between the current cap and an unvisited neighbor cap, in the rotational order around `v_corner` given by the radial-edge ordering at the vertex (Keel's non-manifold radial edge ordering, dossier 01). This yields the sequence `cap_a = c_0, c_1, ..., c_{K-1} = cap_b`.
3. **The tube enters at `cap_a` (anchored to `a_end`) and exits at `cap_b` (anchored to `b_end`).** Intermediate caps `c_1..c_{K-2}` are crossed fully (entry crossing `vr_{i-1}`, exit crossing `vr_i`).

The rotational walk is the same primitive Keel already uses to find the Roof ridge (the `face_edge_set` intersection at blend.rs:813-823); for K faces it is iterated, producing K-1 ridges `ridge_0..ridge_{K-2}` and K-1 crossing vertices `vr_0..vr_{K-2}`. **Verified** that this terminates: the cap faces form a fan around `v_corner` bounded by the two supports, so the radial order is a finite open walk from `cap_a` to `cap_b`.

### 1.3 The Euler operator sequence (generalizing blend.rs:865-889)

The Roof dissolve does: `kef(edge)` (merge the two strips across the sharp blended edge), then per end `kef(stub)` (fold the spring stub), `kef(stub2)` (fold the second roof stub), `kev(rspur)` (kill the ridge spur vertex). The multi-face generalization inserts **K-1 crossing imprints** before the dissolve and **K-1 ridge-stub folds** in the dissolve. Per end vertex with K cap faces:

| Step | Operator | Effect | dV | dE | dF |
|---|---|---|---|---|---|
| 1 | `split_edge` x (K-1) | Imprint each ridge crossing `vr_0..vr_{K-2}` on its ridge edge (blend.rs:846 done K-1 times). | +(K-1) | +(K-1) | 0 |
| 2 | `split_blend_cap` x K | Imprint the conic sub-arc `arc_i` on each cap `c_i`, splitting it into kept-region and tube-sliver (blend.rs:793/848/850 done K times). Each `split_blend_cap` is internally `split_face`/`mef`-class: +1 E, +1 F. | 0 | +K | +K |
| 3 | `kef(edge)` | Merge the two trimmed support strips across the old sharp blended edge (blend.rs:865, once per whole edge, shared by both ends). | 0 | -1 | -1 |
| 4 | `kef(stub)` x (per end) | Fold the spring stub `a_end -> v_corner` into the opening blend face (blend.rs:870). | 0 | -1 | -1 |
| 5 | `kef(ridge-stub)` x (K-1) | Fold each of the K-1 cap slivers' shared ridge stubs into the continuing blend face (the Roof `kef(stub2)` at blend.rs:881, iterated K-1 times). | 0 | -(K-1) | -(K-1) |
| 6 | `kev` x (K-1) | Kill each ridge-spur vertex `vr_i` once its two incident sliver edges are gone, plus the corner spur (the Roof `kev(rspur)` at blend.rs:886, iterated). | -(K-1) | -(K-1) | 0 |

The defining property: **K cap faces produce K conic sub-arcs and K-1 ridge crossings; the dissolve folds K-1 ridge stubs (versus Roof's exactly one) and kills K-1 ridge spurs.** Setting K = 2 recovers Roof line-for-line (one ridge crossing, one `kef(stub2)`, one `kev(rspur)`); K = 1 recovers Single (no ridge, no `stub2`, the `kev(spur)` of blend.rs:874-876). The blend surface is unchanged: it stays the one exact cylinder attached at blend.rs:898-902, with K conic arcs forming its end trim loop. The net per-end effect mirrors Roof: the K cap slivers and the two support strips dissolve into the single continuing blend face, the corner vertex and its K-1 ridge spurs are removed, S and G unchanged, R unchanged (the arc chain is a boundary segment of the blend face's single outer loop, not an inner ring).

**Claimed**, reconstructed from the Roof code by induction on K: the only new code is (a) the rotational ridge walk of Section 1.2 (iterate the existing single-ridge find), (b) a loop over the K caps calling the existing `conic_for` + `split_blend_cap`, and (c) a `for` over K-1 ridge stubs in the dissolve calling the existing `kef`/`kev`. No new geometric primitive is introduced; the per-face conic is the same `conic_for` that Single and Roof already call.

### 1.4 The new vertices and the per-face conic, made exact

Each crossing vertex `vr_i` is the near quadratic root of the radius-`r` circle against ridge `i` (blend.rs:837-846), imprinted by `split_edge` at `pc + (pr_i - pc) * t_i` (blend.rs:846). The validity gate is the same as Roof: `qa > 1e-18 && disc > 0` (the ridge is not parallel to the spine and does cross the circle, blend.rs:839) and `t_i in (0, 1)` (the crossing is on the ridge segment, not its extension, blend.rs:843). The per-face conic on `c_i` is `conic_for(c_i, pc, end_i)`:

- **Perpendicular cap** (`|d_i| = |dir . n_i| ~ 1`): a circular arc of radius `r`, axis `dir`, centered at the spine point (blend.rs:743-746). This is the clean "tube ends square against a face perpendicular to it" case.
- **Oblique cap** (`0 < |d_i| < 1`): an elliptical arc, center `spine_pt + dir * ((pc - spine_pt).n_i / d_i)`, minor axis `r` along `dir.cross(n_i)`, major axis `r / |d_i|` along `n_i.cross(minor)` (blend.rs:748-758). The cylinder cut by an oblique plane is exactly an ellipse; Keel carries it as an exact `Ellipse3`.

Both stay in Keel's exact-analytic catalog, so the multi-face cap inherits the `mass == mesh == ref` exactness of Single/Roof (Section 5). **Verified** from `conic_for` source.

### 1.5 Where it is genuinely unrepresentable: DECLINE-forever

The multi-face cap is representable iff the rolling ball can **reach** the corner and each cap crossing is a real conic on a real cap face. Three configurations are genuinely unrepresentable and must stay DECLINE-forever:

1. **The corner the ball cannot reach (local feature size below `r`).** If `v_corner` is a sharp re-entrant corner whose local feature size `lfs(pc) < r` (dossier 41's medial ceiling), the rolling ball of radius `r` cannot seat at the corner: the blend overflows the corner *itself*, not just one face. There is no end-circle to trim because the ball never reaches `pc`. This is the F4/F10 vertex-congestion case of dossier 41 and the mixed-convexity vertex of dossier 56 bucket (c). **DECLINE-forever** (predicted by the medial ceiling, disclosed, never thrown).
2. **A cap crossing with no real root** (`disc <= 0` on some ridge, or `t_i` off the segment, blend.rs:839/843). The ridge does not cross the radius-`r` circle on its real extent: the tube end does not actually visit that cap, or the corner geometry is degenerate. This is the existing Roof guard generalized; on failure, decline rather than fabricate a crossing.
3. **Mixed convexity at the corner** (some caps meet the supports convexly, some concavely, at the same `v_corner`). The end region has mixed sense; the in-place dissolve of Section 1.3 can pass through a transiently invalid (self-overlapping) state, and the correct construction needs the construct-and-sew vertex patch of dossiers 44/55 (the setback vertex blend, Section 3.3). **DECLINE-forever for the in-place path**; defer to the setback-vertex follow-up ladder, which is its own milestone.

These three are exactly the cases where the *outcome is not a chain of conic arcs on flat-or-analytic caps*. Everything else (K planar/analytic caps the ball reaches, uniform convexity) is the representable multi-face cap of Sections 1.1 to 1.4.

> **Annotated source block (multi-face corner, production realization).**
> **Citation.** Open CASCADE Technology, *ChFi3d_Builder Class Reference* (`PerformOneCorner`, `PerformTwoCorner`, `PerformThreeCorner`, `PerformMoreThreeCorner`, `ExtentOneCorner/TwoCorner/ThreeCorner`, `PerformExtremity`). https://dev.opencascade.org/doc/occt-6.9.0/refman/html/class_ch_fi3d___builder.html
> **Method/Content.** OCCT closes a blend tube at a vertex with a **family of corner solvers keyed on the number of faces/fillets meeting**: `PerformOneCorner`, `PerformTwoCorner`, `PerformThreeCorner`, and `PerformMoreThreeCorner` for four or more meeting edges; `PerformExtremity` does the geometry at the endpoint vertex of a spine, and `ExtentOneCorner/TwoCorner/ThreeCorner` **extend the blend stripe geometry at the vertex**. This is the production confirmation that the end-cap is dispatched on K (the meeting count) exactly as Keel's `[cap]` / `[c0,c1]` / `_` match does, and that a distinct path is needed for K = 1, 2, 3, and "more than three." Keel's Single = `PerformOneCorner`, Roof = `PerformTwoCorner`, and the declined multi-face cap is `PerformThreeCorner`/`PerformMoreThreeCorner`.
> **Limitations.** OCCT's corner solvers are notoriously fragile on the K >= 3 cases (the well-documented `ChFi3d` corner failures); the API confirms the *dispatch* but not a robust *construction*. Keel's exact-conic-chain (Section 1.1) is more constrained and therefore more robust where it applies, at the cost of declining the cases OCCT also struggles with.
> **Kernel relevance.** Validates the K-keyed dispatch and the "extend the stripe at the extremity" primitive Keel needs for Section 2. The conic-chain generalization of Roof is Keel's robust subset of `PerformMoreThreeCorner`.

---

## Section 2. The parallel-cap extend (blend overflow)

### 2.1 The trigger and the geometry

The trigger is exact and already detected: `conic_for` returns `Err("fillet: cap face parallel to the spine (extend follow-up)")` when `d = dir.dot(n_cap)` is below `1e-9` (blend.rs:759-763). Geometrically, the cap face's normal `n_cap` is perpendicular to the spine direction `dir`, so the cap plane is **parallel to the spine** (contains the spine direction). The rolling ball's end-circle lies in a plane normal to `dir`, which is *perpendicular* to the cap, so the end-circle and the cap are parallel and never meet: there is no conic to trim against. The tube, swept along the spine, runs *alongside* the cap, not *into* it.

The handling is the classic **blend overflow / extend**: lengthen the blend along the spine until the swept cylinder **reaches** the parallel cap, then trim against it. Because the cap is parallel to the spine, the cylinder (whose axis is parallel to the spine, offset by `r`) meets the cap not in an end-circle but along a **ruling line** of the cylinder: the line on the cylinder closest to (or touching) the cap plane. The blend extends until that ruling line lands on the cap, and the trim boundary at that end is a **straight line** (the cylinder ruling that lies in the cap plane), not a conic arc. This is the analytic, exact case: cylinder meeting a plane parallel to its axis is a line (when tangent) or two lines (when secant); the relevant one is the ruling where the spring curve would otherwise have left the support.

### 2.2 The extend construction and how far to extend

**The surface carried by the extension: the same cylinder, lengthened.** The extend does not change the blend surface. Keel's blend cylinder (blend.rs:898-902) is unbounded as a surface; only its end *trim* is at issue. The extension moves the end trim from "the (non-existent) end-circle at `v_corner`" to "the cylinder ruling line where the cylinder meets the parallel cap." This is dossier 56's bucket (a) exact case: the blend surface is unchanged, only the trim boundary moves to a model feature (here a ruling line on the cap, the parallel-cap analogue of the cliff edge).

**How far: the extend distance `L_ext`.** Extend the spine from `v_corner` along `dir` until the cylinder first touches the cap plane. With the cap plane `pi_cap` (point `pc`, normal `n_cap`, with `dir . n_cap ~ 0`), the cylinder axis line `L_s` (through `spine_pt`, direction `dir`) is parallel to `pi_cap`. The cylinder of radius `r` about `L_s` touches `pi_cap` where the distance from `L_s` to `pi_cap` equals `r`. Two regimes:

- **Bounded extend (the cap closes the tube).** If the axis-to-cap distance is exactly `r` (the cap is tangent to the cylinder) or the cap plane cuts the cylinder, the extend reaches it at a finite `L_ext`: extend the spine to the parameter where the spring curve's projection meets the cap edge, imprint the ruling line, trim. `L_ext` is bounded by the cap face's own extent along the spine direction (the cap is a finite face, so the tube reaches its bounding edge in finite length). **Bounded, exact, representable.**
- **Runaway extend (the cap never closes).** If the cap plane is farther than `r` from the axis on the wrong side (the cylinder never reaches it) or the cap face is unbounded in the spine direction with no transverse closing edge, the extend runs forever: there is no finite `L_ext` at which the tube closes. **DECLINE-forever** (the cap never closes; dossier 41's "blend grows without bound" F-class).

**The precondition, stated as a predicate.** Extend is bounded iff the parallel cap face has a **bounding edge transverse to the spine within reach of the radius-`r` cylinder**: `exists` an edge `e_close` of the cap with `dist(L_s, e_close) <= r` and `e_close` crossing the spine sweep. This is checkable from the cap face's loop with one distance test per bounding edge, reusing the spring-line-containment machinery of dossier 41. If no such edge exists, decline.

### 2.3 The Euler surgery for the extend end

The extend end replaces the Single/Roof cap dissolve with a **ruling-line trim**, structurally the cliff-clip of dossier 56 Section 1 (the trim boundary becomes an existing/imprinted model edge rather than a fresh conic). Per parallel-cap end:

| Step | Operator | Effect | dV | dE | dF |
|---|---|---|---|---|---|
| 1 | `split_edge` on the cap's closing edge `e_close` | Imprint the two points where the extended cylinder's ruling meets `e_close` (the tube's footprint on the cap). | +2 | +2 | 0 |
| 2 | `split_face`/`mef` on the cap | Imprint the ruling line across the cap, separating the kept cap region from the tube footprint. | 0 | +1 | +1 |
| 3 | `kef` + `kev` | Dissolve the cap sliver (tube footprint) into the opening blend gap; the blend end now follows the ruling line. | -1 | -2 | -1 |
| 4 | `kef(edge)` + spring-stub fold | The shared support-strip merge and spring-stub fold as in Single (blend.rs:865-876), the spur killed at the ruling end. | -1 | -2 | -1 |

The blend surface stays the one cylinder; its end trim at the extend side is the **ruling line on the cap** (a straight edge shared between the blend face and the cap face, the parallel-cap analogue of the Roof ridge but straight, not a crossing vertex). **Claimed**, assembled from the cliff-clip surgery (dossier 56 Section 1/7) with the trim target being a cap ruling line instead of a cliff edge.

> **Annotated source block (production blend overflow / extend).**
> **Citation.** Siemens / Parasolid, *Edge Blending: Overview; Edge Blend Overflows* ("cap the end of the blend with existing faces, extending at most two of the adjoining faces"; "many blends cannot fit entirely inside one or both of the faces ... strategies that enable such blends to be fixed ... known as creating blend overflows"). http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.11.html ; v12 mirror http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html . Spatial / ACIS, *Blending Component: Sequences of Blended Edges* ("when no model faces intersect the blend sheet, ACIS will attempt to extend existing model faces to close off the end of the blend"; a `cap` instruction tells the blend "to begin capping the existing blend face rather than to roll onto the new entities"). http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF
> **Method/Content.** Both kernels handle the parallel-cap by **extending model faces to meet the blend sheet** (Parasolid: extend at most two adjoining faces; ACIS: extend existing model faces to close off the end, or intersect a face in the model with the blend sheet to trim the sheet exactly to the model). The blend surface is not deformed; the *model* is extended until it intersects the (lengthened) blend, then the blend is trimmed to that intersection. This is exactly Keel's extend-and-trim-to-ruling of Section 2.2 to 2.3.
> **Limitations.** "Extend at most two" is a hard production cap: more than two adjoining faces to extend is where Parasolid itself begins to decline or require manual overflow controls (`ov_*`). The parallel cap that needs three-plus extended faces is the boundary with the multi-face cap of Section 1 and inherits its decline set.
> **Kernel relevance.** Confirms the extend is bounded by the cap's reachable bounding edge and that "extend at most two faces" is the safe envelope. Keel's bounded/runaway predicate (Section 2.2) is the explicit form of Parasolid's implicit "can these faces be extended to meet" test.

**Patent posture.** Cap-by-face-extension and trim-blend-sheet-to-extended-faces are **old prior art** (extend two planes to meet is elementary; dossier 56 Section 2.2). Parasolid's *advanced capping* (the V13.0 enhancement removing the restriction that the extended cap edges may not intersect) is a **vendor enhancement, not a blocking patent for the basic mechanism** [verified, dossier 56]. The parallel-cap extend as Keel builds it (lengthen the unbounded cylinder, trim to a ruling line where the cap is reached) is the cliff-clip of a swept surface to a boundary, classical and unencumbered. **Avoid** reading any specific post-2006 overflow-control claim into the construction; build from the public *behavior* description (extend faces to meet, trim sheet to model), not any vendor algorithm.

---

## Section 3. The vertex where the filleted edge meets a boolean seam (the real workflow)

### 3.1 The canonical case: boolean then fillet the seam

The dominant compound case is "union a transversal peg into a block, then fillet the seam circle (or seam edges) where the peg meets the block." The seam is the intersection curve the boolean imprinted; its edges end at vertices the boolean created. Characterize the local topology at such a seam-end vertex:

- **A peg-into-face union** (a cylindrical or prismatic peg standing on a block face): the seam is a closed loop (a circle for a round peg) with **no end vertex**, the easy sub-case. Filleting a closed seam loop has *no* Single/Roof/multi-face cap at all: the blend is a closed annular ribbon (a torus for a circular seam, dossier 40), it closes on itself. This is the cleanest compound fillet and should be a first-milestone baseline (Section 5).
- **A peg-into-edge or peg-meeting-a-step** (the peg's side coincides with or crosses a block edge): the seam edge **ends at a vertex where the peg face, the block top, and the block side all meet**, K = 3 cap faces, uniform convexity if the peg sits proud. This is the **multi-face cap of Section 1 with K = 3**, the most common honest multi-face case on boolean bodies.
- **A peg corner (transversal peg whose end cap is parallel to the seam).** A peg lying *along* a block face, filleted on its line-of-contact seam, ends where the peg's flat end cap stands parallel to the seam edge: the **parallel-cap extend of Section 2**. The peg's end cap is parallel to the spine of the seam blend, so the blend must extend to reach it.

So the boolean-seam vertex reduces to exactly the two cases this dossier adds: a K >= 3 corner (Section 1) when the peg meets a block edge/step, or a parallel-cap extend (Section 2) when the peg's end cap is parallel to the seam. The clean-primitive Single/Roof cases arise only when the peg meets a single transverse face squarely (Single) or a two-face ridge (Roof), which is precisely why they "barely occur after the first op."

### 3.2 The convexity mix

A boolean union seam is **convex** on the outside (the peg adds material, the fillet rounds a convex edge), so the peg-proud case is uniform-convex and the in-place multi-face dissolve of Section 1.3 is valid. A boolean *subtraction* (a peg-shaped pocket) gives a **concave** seam, and the cap-grow surgery of dossier 44 applies instead of cap-shrink; the multi-face generalization is the same chain-of-arcs but with the concave `sigma` branch (dossier 44's cap-grow), and the dissolve grows the cap rather than shrinking it. **Mixed convexity at one vertex** (a peg that is proud on one face and recessed on the neighbor, e.g. a peg straddling a block edge) is the DECLINE-forever case of Section 1.5 item 3: the end region has mixed sense and needs the setback vertex patch.

### 3.3 The setback/mitre interplay when the adjacent edge is also filleted

If the edge *adjacent* to the seam at the corner is also to be filleted (two blends converging at the corner), the end-cap problem becomes a **blend-network mitre / setback vertex blend** (dossiers 53/55), which is strictly harder than a single blend's end cap. The two converging blend ribbons must be reconciled at the vertex by a central vertex patch rather than each capping independently. The production structure (verified from the setback-vertex literature):

> **Annotated source block (setback vertex blend, the converging-blends corner).**
> **Citation.** Varady, T., & Rockwood, A. (1997). *Geometric construction for setback vertex blending.* Computer-Aided Design, 29(6), 413-425. https://www.sciencedirect.com/science/article/abs/pii/S001044859600070X . Related survey: Varady, T., et al., *N-sided surfaces: a survey* (2n-sided setback patch). https://www.researchgate.net/publication/235146565_N-sided_surfaces_A_survey
> **Method/Content.** Where n edge blends meet at a vertex, the **setback** retracts each edge blend's end **back from the vertex by a setback distance**, terminating each blend's boundary before it reaches the vertex; the retracted ends are joined by **spring curves**, and a single larger **2n-sided central patch** is inserted to fill the vertex, alternating **n spring curves and n profile curves** around its boundary (the most general setback-type vertex blend has maximum 2n sides). Special cases give an odd side count. The central patch is built by a control-frame construction following a repeated-chamfering strategy.
> **Limitations.** The 2n-sided central patch is **not analytic** (a general n-sided fill, dossier 56 bucket b/c, dossier 40 vertex blends); it is the procedural-evaluator-to-NURBS case, outside Keel's exact-conic regime. Setback distances are heuristic (free parameters to adjust interior shape), so there is no exact closed-form oracle for the central patch.
> **Kernel relevance.** This is the **DECLINE-forever boundary** for Keel's single-edge end-cap engine: a single filleted edge ending at a corner is Section 1 (K conic arcs, exact) or Section 2 (extend, exact); but *two-or-more converging filleted edges* at the corner is a setback vertex blend (2n-sided non-analytic patch), which Keel declines until a dedicated vertex-blend milestone (dossier 55). The interplay rule: **if exactly one edge at the corner is filleted, use Section 1/2 (exact, in-scope); if two-or-more, decline to the setback-vertex follow-up.** Keel's `split_cap` already implicitly assumes the single-edge case (it caps against *model* faces, not against another blend ribbon); the mitre case must be detected (a cap face that is itself a blend face, dossier 56's `is_blend_face` predicate) and declined distinctly from the multi-face cap.

The setback/mitre detection reuses dossier 56's `is_blend_face(F)` predicate at zero new cost: if any cap face at `v_corner` is a recognized blend/rounding face, the corner is a converging-blends mitre (setback vertex), not a model-face multi-face cap, and is declined to the dossier-55 follow-up. This cleanly separates the **representable** multi-face cap (K model faces, Section 1) from the **deferred** mitre (a blend face among the caps).

---

## Section 4. How production kernels close the tube against a multi-face corner and a parallel cap

Consolidated from Sections 1.5, 2.3, 3.3 source blocks plus dossier 56 Section 8.

| Kernel | Multi-face corner (K >= 3) | Parallel cap (extend) | The "extend at most two faces" rule |
|---|---|---|---|
| **Parasolid** (`PK_BODY_fix_blends`, `ov_*`) | Cap by **extending at most two adjoining faces to meet**; three-edge vertex blended as a group adds an **extra blend face** to smooth the vertex [verified, search]. Beyond two-face extend or three-edge group, requires explicit overflow controls or declines. | Blend overflow: extend faces until the (lengthened) blend sheet is reached, trim sheet exactly to the model. V13.0 advanced capping removed the restriction that the **extended cap edges may not intersect** [verified, search], enabling some K=3-ish caps. | **Hard production envelope.** "At most two adjoining faces" is the documented safe cap; Keel adopts it as the boundary between Section 2 (one-or-two-face extend, exact) and the declined multi-extend. |
| **ACIS** (`blend`, setbacks) | **Setback vertex blend**: retract each blend before the vertex, join with spring curves, insert the n-sided/2n-sided central vertex patch [Varady-Rockwood]. Mixed-convexity capping is the documented hard case. | "When no model faces intersect the blend sheet, ACIS will **extend existing model faces to close off the end** of the blend"; `cap` instruction stops roll-on and caps; "intersect a face or faces in the model with the blend sheet to trim the sheet exactly to the model" [verified, dossier 56]. | Same extend-to-meet mechanism; setbacks position the cross edge that closes the end. |
| **OpenCascade** (`ChFi3d_Builder`) | **K-keyed corner solvers**: `PerformOneCorner`, `PerformTwoCorner`, `PerformThreeCorner`, `PerformMoreThreeCorner`; `ExtentOneCorner/TwoCorner/ThreeCorner` extend the stripe at the extremity; `PerformExtremity` does the endpoint-vertex geometry [verified]. Fragile for K >= 3 (documented `ChFi3d` corner failures). | `ExtentOneCorner/TwoCorner/ThreeCorner` **extend the spine of the stripes at an extremity vertex** [verified, dossier 56 ref 7]; `ChFiDS_State` records trim-vs-extend per end. | The Extent* family is the explicit "extend the stripe at the corner" primitive Keel mirrors in Section 2. |

**The shared production pattern (the headline):** all three kernels (i) **dispatch the end cap on the meeting count K** (Single/Roof/multi mirror One/Two/Three/MoreThreeCorner), (ii) **extend faces (at most two) to close a tube that does not naturally meet a cap** (the parallel-cap extend), and (iii) **fall back to a setback vertex patch** when *multiple blends converge* (not a model-face cap but a blend-blend corner). Keel's design tracks this exactly: Section 1 = the K-keyed conic-chain (the robust subset of MoreThreeCorner), Section 2 = the at-most-two-face extend, Section 3.3 = decline the converging-blends mitre to the setback follow-up.

---

## Section 5. The validation oracle (compound-body battery)

Every new handler must pass Keel's exact triplet `validate() + mass == mesh == ref` and stay **watertight**, with the clean-primitive Single/Roof baselines held as a **non-regression** gate. The battery, in milestone order:

**B0. Non-regression baselines (must not move).** The existing clean-primitive Single and Roof end-cap fillets (block-edge fillet ending at a single transverse face; ending at a two-face ridge). Assert `mass == mesh == ref` unchanged, bit-for-bit where the exact volume is closed-form. This is the gate every change below runs against first.

**B1. Closed seam loop (the easiest compound case, no end cap).** Union a round peg onto a block face; fillet the closed circular seam. The blend is a **torus** (circular spine, dossier 40), closes on itself, no Single/Roof/multi cap fires. `ref` = the exact torus-fillet volume removed/added at a circular convex seam (closed form). This validates the compound *pipeline* (boolean then fillet) before any cap surgery, and is the honest first compound milestone.

**B2. Multi-face cap, K = 3, uniform convex (the core new case).** Union a transversal prismatic peg crossing a block edge so the seam edge ends at a 3-face corner (peg face + block top + block side). Fillet the seam edge. The end cap is the K = 3 conic chain of Section 1: three conic sub-arcs (circular where a cap is perpendicular to the spine, elliptical where oblique), two ridge crossings. `ref` = block-plus-peg union volume minus the exact fillet-cut volume (the cut is the cylinder-minus-quarter swept along the seam, truncated by the three exact cap planes, a closed-form solid). Assert `validate() + mass == mesh == ref + watertight`. **This is the first-milestone oracle.**

**B3. Multi-face cap, K = 4.** A peg meeting a block *corner* (four cap faces). Same conic-chain surgery, three ridge crossings. Confirms the K-generalization loop (Section 1.3) is count-agnostic. Exact `ref` as in B2.

**B4. Parallel-cap extend (the overflow).** Fillet an edge whose end cap is **parallel** to the edge (a peg lying along a block face, filleted on its line-of-contact seam, the peg's flat end cap parallel to the seam). The blend extends along the spine to the cap ruling line (Section 2). `ref` = the exact extended-cylinder-fillet volume trimmed at the ruling line (cylinder cut by a plane parallel to its axis is a line/two-lines, exact). Assert the **extend distance `L_ext`** equals the closed-form axis-to-cap reach as a secondary oracle. Sweep a parameter (peg position) through the bounded-to-runaway boundary: below, bounded extend, valid; at the boundary, tangent; beyond, the cap never closes, **declines** with the precise diagnostic (Section 2.2), never throws.

**B5. Setback/mitre decline (the boundary, must DECLINE not WRONG).** Two adjacent edges sharing a corner, **both** filleted (Section 3.3). Assert Keel **declines** with the converging-blends-mitre diagnostic (the `is_blend_face` cap detection fires), returns the valid unblended-corner body, and never produces a wrong or non-watertight result. This guards the DECLINE-never-WRONG contract at the exact boundary of the new scope.

**B6. Mixed-convexity vertex decline.** A peg straddling a block edge (proud on one face, recessed on the neighbor), single seam edge filleted, ending at a mixed-sense corner (Section 1.5 item 3). Assert **decline** (predicted by the mixed-`sigma` test), valid unblended body returned.

The differential discipline from dossier 41/56: each oracle sweeps the controlling parameter (radius, peg position) through the trigger value, asserting validity and the exact triplet on the representable side and a *predicted, disclosed decline* on the unrepresentable side, never a throw or a wrong volume. The curved-boundary chord-vs-arc trap (memory note, dossier 54) applies: every conic cap arc and every cylinder ruling must be sampled as an arc/line, not a chord, or `mesh` will mis-attribute.

---

## Synthesis: ADOPT / AVOID / INVESTIGATE, first milestone, follow-up ladder, decline-forever

### ADOPT

- **The K-keyed conic-chain as the multi-face cap** (Section 1): generalize Roof to K cap faces as a closed chain of K conic sub-arcs joined at K-1 ridge crossings, with the dissolve folding K-1 ridge stubs and killing K-1 ridge spurs. It reuses Keel's existing `conic_for`, `split_blend_cap`, `split_edge`, `kef`, `kev` verbatim, adds no new geometric primitive, and recovers Single (K=1) and Roof (K=2) line-for-line. Bucket (a) exact, mirrors OCCT's `PerformMoreThreeCorner` as its robust subset.
- **The parallel-cap extend via ruling-line trim** (Section 2): lengthen the unbounded blend cylinder along the spine until it reaches the parallel cap, trim to the cylinder ruling line on the cap. The surface is unchanged; only the end trim moves to a model feature. This is the cliff-clip of dossier 56 with a ruling-line target. Build from the public extend-faces-to-meet behavior, classical and unencumbered.
- **The bounded/runaway extend predicate** (Section 2.2): extend is bounded iff the parallel cap has a bounding edge transverse to the spine within radius `r`; else decline. One distance test per cap edge, reusing spring-line containment.
- **The `is_blend_face` mitre split** (Section 3.3): if any cap face is a recognized blend face, the corner is a converging-blends setback vertex, declined distinctly from the model-face multi-face cap. Zero new cost (dossier 56 predicate).
- **The compound-body exact-triplet battery** (Section 5) with the Single/Roof baselines as a hard non-regression gate.

### AVOID

- **Building the setback vertex (2n-sided central) patch in the single-edge end-cap engine.** It is non-analytic, heuristic (setback distances), has no exact oracle, and belongs to the dedicated vertex-blend milestone (dossier 55). Detect and decline it, do not attempt it inside `split_cap`.
- **Extending more than two adjoining faces** for the parallel cap. "At most two" is the documented production safe envelope; multi-extend is the declined tail.
- **Reverse-engineering any post-2006 overflow-control claim.** Build extend and cap from the public extend-faces-to-meet behavior and the classical cliff-clip; the multi-face conic chain is ordinary Euler surgery on exact conics, classical (Stroud 2006, Mantyla 1988).

### INVESTIGATE

- **The K >= 3 concave (cap-grow) multi-face cap** (dossier 44 cap-grow branch): the same conic chain with the concave `sigma` dissolve. Second wave, after the convex K-chain is solid.
- **The creeping/oblique extend** where the parallel cap is reached on a *curved* cap (the ruling becomes a general curve): bucket (b) certified-evaluator-to-NURBS.
- **The setback vertex blend itself** (dossier 55 milestone): the 2n-sided central patch via certified-evaluator-to-NURBS, the converging-blends corner Keel currently declines.

### Recommended FIRST MILESTONE (narrowest honest slice with an exact oracle)

**The K = 3 uniform-convex multi-face cap on a compound body: union a transversal prismatic peg crossing a block edge, fillet the seam edge that ends at the 3-face corner, close it with the three-conic-arc chain of Section 1.** This is the single most common honest decline on boolean bodies (Section 3.1), stays fully exact (three conic arcs on three planar/analytic caps, bucket a), reuses `conic_for` + `split_blend_cap` + the Roof dissolve generalized once (K=3 means one extra ridge crossing and one extra `kef`/`kev` versus Roof), and has a closed-form `ref` volume for the `mass == mesh == ref` gate (battery B2). Ship it with B0/B1 as the non-regression gate and B5/B6 wired as decline tests, before any extend or higher-K work. It directly converts the largest fillet-Precondition decline class (the multi-face cap at blend.rs:853) into a handled, exact case for the dominant K=3 peg-meets-block-edge geometry.

### Follow-up ladder (in order)

1. **K = 4 and general K multi-face cap** (B3): the count-agnostic loop, exact.
2. **Parallel-cap extend, bounded** (Section 2, B4): the ruling-line trim, exact, with the bounded/runaway predicate.
3. **Concave (cap-grow) multi-face cap** (dossier 44 branch): the conic chain with the concave dissolve.
4. **Curved-cap extend** (bucket b): certified-evaluator-to-NURBS where the cap is curved.
5. **Setback vertex blend** (dossier 55): the 2n-sided central patch, the converging-blends corner.

### Configuration to DECLINE forever (with justification)

**The mixed-convexity multi-blend vertex: a corner where two-or-more filleted edges converge AND the supports/caps have mixed convex/concave sense (a peg straddling a block edge, both its edges filleted).** Justification: it requires the non-analytic 2n-sided setback central patch (no exact oracle, heuristic setbacks) *and* mixed-sense reconciliation (the in-place dissolve passes through transient invalidity), with no exact closed form and no procedural shortcut that fits Keel's exact-triplet contract. It is exactly where production kernels (ACIS mixed-convexity capping, OCCT `ChFi3d` corner) themselves emit blend errors. Keel's honest-decline doctrine covers it: detect it cheaply (the `is_blend_face` mitre test plus the mixed-`sigma` test, both already available), return the valid unblended corner plus a precise per-corner diagnostic, and never ship a fragile or wrong result. Declining it forever costs almost nothing in real coverage (it is the rare confluence of converging blends and mixed convexity at one vertex) and removes the unbounded-complexity tail; the decline is *predicted* by local predicates, never *thrown* mid-surgery.

**The honest one-line summary:** the multi-face cap is Roof generalized to a closed chain of K conic sub-arcs (one ridge crossing and one `kef`/`kev` per extra cap, exact, the K=3 peg-meets-block-edge case shipped first), the parallel-cap extend lengthens the unchanged cylinder to a ruling-line trim on the reached cap (bounded iff the cap has a transverse closing edge within radius `r`), the converging-blends and mixed-convexity vertices are declined to the setback follow-up with predicted diagnostics, and every representable case carries an exact `mass == mesh == ref` oracle on a compound body.

---

## References

1. Open CASCADE Technology. *ChFi3d_Builder Class Reference* (`PerformOneCorner`, `PerformTwoCorner`, `PerformThreeCorner`, `PerformMoreThreeCorner`, `ExtentOneCorner/TwoCorner/ThreeCorner`, `PerformExtremity`, `PerformIntersectionAtEnd`: K-keyed corner solvers; extend the stripe spine at an extremity vertex). https://dev.opencascade.org/doc/occt-6.9.0/refman/html/class_ch_fi3d___builder.html ; ChBuilder (`PerformThreeCorner` "computes the intersection of three chamfers on the vertex"): https://dev.opencascade.org/doc/occt-7.5.0/refman/html/class_ch_fi3d___ch_builder.html ; Package ChFi3d: https://dev.opencascade.org/doc/refman/html/package_chfi3d.html
2. Siemens / Parasolid. *Edge Blending: Overview; Blending; Edge Blend Overflows; Functional Enhancements* ("cap the end of the blend with existing faces, extending at most two of the adjoining faces"; "blends cannot fit entirely inside ... strategies ... known as creating blend overflows"; three-edge vertex group adds "an extra blend face to smooth out the vertex"; V13.0 removed the restriction that extended cap edges may not intersect). V35 overview: http://www.q-solid.com/Parasolid_Docs_V35/chapters/ov_chap.11.html ; v12 functional desc: http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html ; functions/options: http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.31.html ; enhancements: http://www.q-solid.com/Parasolid_Docs/chapters/rn_chap.07.html ; PK_BODY_fix_blends: http://www.q-solid.com/Parasolid_Docs/headers/pk_body_fix_blends.html
3. Spatial / ACIS. *Blending Component Reference: Sequences of Blended Edges; Vertex Blends; Setbacks* ("when no model faces intersect the blend sheet, ACIS will attempt to extend existing model faces to close off the end of the blend"; `cap` instruction stops roll-on and caps; "intersect a face or faces in the model with the blend sheet to trim the sheet exactly to the model"; start/end setbacks position the cross edge; mixed-convexity capping the hard case). http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF ; Setbacks: http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0011.HTM
4. Varady, T., & Rockwood, A. (1997). *Geometric construction for setback vertex blending.* Computer-Aided Design, 29(6), 413-425 (n edge blends retracted by a setback, joined by spring curves, filled by a 2n-sided central patch alternating n spring and n profile curves). https://www.sciencedirect.com/science/article/abs/pii/S001044859600070X
5. Varady, T., Salvi, P., et al. *N-sided surfaces: a survey* (the 2n-sided setback vertex patch; the n>4 multi-sided fill bibliography). https://www.researchgate.net/publication/235146565_N-sided_surfaces_A_survey
6. Vida, J., Martin, R. R., & Varady, T. (1994). *A survey of blending methods that use parametric surfaces.* Computer-Aided Design, 26(5), 341-365 (the rolling-ball blend, vertex blends, compatibility at multi-valent corners). https://www.sciencedirect.com/science/article/abs/pii/001044859490023X ; SZTAKI GML: https://eprints.sztaki.hu/682/
7. Choi, B. K., & Lee, C. S. (1998). *Method for generating an extended rolling ball fillet surface between two surfaces.* US Patent 5,774,359 (Autodesk), filed 1995, **expired** (free prior art for roll-on/extend). https://patents.google.com/patent/US5774359A/en
8. *Notch blends in BRep models.* US Patent 8,935,130 (post-2006, **live**: construct-separate-sheet + sew + extend-faces-to-meet; design-around required, prefer in-place Euler surgery). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8935130
9. T-FLEX CAD. *Blending Edges* (smooth/cliff/notch overflow constructions; trim-or-extend the model to the rounding; priority order smooth then cliff then notch; ball rolling on top of the constraining edge). https://www.tflex.com/help/eng/T-FLEX%20CAD/16/blending_edges.htm
10. Rossignac, J. R., & Requicha, A. A. G. (1986). *Constant-radius blending in solid modelling.* Computers in Mechanical Engineering (the rolling-ball trim of a swept surface to a model boundary, the unencumbered prior art for cliff/extend). https://www.semanticscholar.org/paper/CONSTANT-RADIUS-BLENDING-IN-SOLID-MODELLING-Rossignac-Requicha/553014d71e15e44f3585377bc9fd017d4f823b3d
11. Braid, I. C. (1997). *Non-local blending of boundary models.* Computer-Aided Design, 29(2), 89-100 (ROMULUS/Parasolid lineage: roll-on, blend-blend, and vertex caps as non-local boundary edits). https://www.sciencedirect.com/science/article/abs/pii/S0010448596000383
12. Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer (Euler-operator recipes for blend/chamfer end caps as local boundary edits, the classical multi-face dissolve). https://link.springer.com/book/10.1007/978-1-84628-616-2
13. Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press (GWB operator set with fixed deltas; Euler-Poincare preservation asserted per step). https://archive.org/details/introductiontoso0000mant
14. Pegna, J., & Wolter, F.-E. (1992). *Geometrical criteria to guarantee curvature continuity of blend surfaces* (Linkage Curve theorem, the G2 condition for the converging-blend transition patch). https://www.semanticscholar.org/paper/Geometrical-Criteria-to-Guarantee-Curvature-of-Pegna-Wolter/1e2c0f2f1ae3f562587bd86b68421141dbef8c13
15. Pekerman, D., Elber, G., & Kim, M.-S. (2008). *Self-intersection detection and elimination in freeform curves and surfaces.* Computer-Aided Design, 40(2), 150-159 (the self-intersection trim for an extended/overflowed blend tube before capping). https://www.sciencedirect.com/science/article/pii/S0010448507002357
16. Farouki, R. T. (1992). *Rolling ball blends and self-intersection* (SPIE) (the rolling-ball envelope and its end-cap degeneracies). https://www.spiedigitallibrary.org/conference-proceedings-of-spie/1830/0000/Rolling-ball-blends-and-self-intersection/10.1117/12.131744.short
17. Patrikalakis, N. M., Maekawa, T., & Cho, W. *Shape Interrogation for Computer Aided Design and Manufacturing*, Ch. 11.3 (cylinder/plane and conic intersection forms underlying the per-face cap conic and the parallel-cap ruling). https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node225.html
18. *Methods, apparatus and computer program products that model three-dimensional surface structures.* US Patent 8,004,517 (n-sided / vertex-region surface modeling; vertex-blend patch construction context). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8004517
19. *Method for recognizing blends in solid models.* US Patent 6,760,038 (blend recognition predicates, the `is_blend_face` test that separates a model-face cap from a converging-blend mitre). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/6760038
20. Onshape / SOLIDWORKS / NX. *Edge fillet overflow controls; Face Blend; partial fillet* ("Allow edge overflow"; the public overflow-option and vertex-cap vocabulary across CAD UIs corroborating the extend-and-cap behavior). https://cad.onshape.com/help/Content/PartStudio/fillet.htm ; Face Blend: https://cad.onshape.com/help/Content/face-blend.htm

## Related Keel dossiers (build on, not repeated)

- `41-blend-overflow-radius-feature-failures.md`: the overflow failure catalog F1 to F11, the medial-axis `r_max(t) = lfs(spine(t))` ceiling, and detection predicates. **This dossier consumes that ceiling (the corner-the-ball-cannot-reach decline, Section 1.5) and the spring-line containment (the bounded/runaway extend predicate, Section 2.2); it does not re-derive detection.**
- `56-blend-overflow-handling.md`: the handling constructions (cliff-clip, notch bridge, cap-by-face-extension, roll-on via the expired extended-rolling-ball), the four-predicate classifier, the priority ladder. **The parallel-cap extend reuses its cliff-clip surgery with a ruling-line target; the mitre split reuses its `is_blend_face` predicate. Not repeated.**
- `54-partial-span-blends-runouts.md`: the interior mid-edge stop (cap-into-supports), the cone-sphere runout exact oracle, the square-stop/feather/vertex-runout termination styles. **The end-cap-against-model-faces here is the complementary extremity type; the exact-oracle discipline is shared.**
- `53-vertex-setback-blends.md` and `55-blend-networks-mitring.md`: the setback retraction and the converging-blends mitre. **Section 3.3 declines the converging-blends corner TO these; the 2n-sided central patch is their milestone, not this one's.**
- `44-concave-fillet-topology-surgery.md`: the cap-shrink (convex) vs cap-grow (concave) Euler surgery and the convexity sign `sigma`. **The multi-face cap reuses its operator vocabulary; the concave K-chain is the cap-grow branch (follow-up ladder item 3).**
- `40-blend-fillet-surface-generation.md`: the spine-as-offset-intersection, the exact line-to-cylinder and circle-to-torus catalog, vertex blends. **The blend surface the end caps trim is exactly this catalog; the closed-seam torus baseline (B1) is its circle-spine case.**
- `28-fillet-chamfer-engine.md`: the engine-level end-cap and overflow menu this dossier makes constructive for `split_cap`.
- `29-dirty-geometry-graceful-degradation.md`: the best-effort-with-disclosure doctrine the DECLINE-forever set (Section 1.5, B5/B6) plugs into.
