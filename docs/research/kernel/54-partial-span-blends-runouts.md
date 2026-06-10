# 54. Range-controlled / partial-span blends and runouts (Parasolid item 55)

**Title and scope.** This dossier treats one narrow, load-bearing capability: a blend that runs over a *segment* of an edge and **terminates mid-edge**, rather than running cap-to-cap between two transverse end faces. Parasolid calls this a range-controlled blend (the blend trimmed to a parameter interval of the edge, or trimmed at a specified edge/face). SOLIDWORKS exposes it as a **Partial Edge** fillet/chamfer; Onshape as a **partial fillet** with start/end offsets; ACIS as **setbacks** plus blend stops. The defining new fact for Keel is topological: the blend face's end boundary lies **inside** the two support faces, not on a transverse cap. There is no third corner face to dissolve (the convex case of `44`) or to grow (the concave case of `44`); the spring lines must *start and stop at interior points of the supports*, and the original sharp edge must *resume* on either side of the blended segment.

This file builds on, and does not repeat, its siblings:
- `28-fillet-chamfer-engine.md` Sections 5 (range and setback control), 6 (overflow/runout): the engine-level range/setback vocabulary and the retractable-edge requirement.
- `40-blend-fillet-surface-generation.md` Area 1: the exact-analytic spine-to-surface catalog (line spine to cylinder, circle spine to torus, the offset-intersection spine procedure).
- `41-blend-overflow-radius-feature-failures.md` Sections 4.2/4.5: setback, blend retraction, and the **partial blend** recovery (blend the rest, leave an interval sharp).
- `44-concave-fillet-topology-surgery.md`: the cap-shrink (convex) and cap-grow (concave) Euler-operator surgeries, the GWB operator deltas, and the convexity sign `sigma`. The mid-edge stop of this dossier is a **third extremity type** beside cap-shrink and cap-grow: a **cap-into-supports** end, where the blend ribbon closes against its own two support faces rather than against a transverse face.
- `39-coincident-tangent-face-booleans.md`: the tangent-sew and tolerant-edge treatment of the G1 spring-curve seam, reused unchanged.

The design recommendation at the close is to validate against Keel's existing edge-blend tests by extending the cap-to-cap `imprint_spring_line` surgery with an **interior start/stop** mode and asserting `mass == mesh == ref` on a partial-span fillet whose exact reference volume is computed by closed form (the cone-sphere runout end gives an exact oracle).

---

## Q1. Termination styles: square stop, feathered/setback runout, runout into a vertex

Three distinct mid-edge termination styles appear across the kernels and CAD front-ends. They are not interchangeable; they differ in both surface geometry and end topology.

### 1.1 Square stop (constant radius up to a transverse end face)

The blend holds its **full radius** right up to a chosen along-edge parameter and stops abruptly with a transverse end. The blend face's end boundary is a single **cross curve**: the section circle (for a fillet) or section line (for a chamfer) at the stop parameter, lying in the plane normal to the spine. Material-wise the bead simply ends; the resulting end face is a small planar (or near-planar) patch whose boundary is the stop-section arc plus the two short fragments of the (now resumed) sharp edge. This is the cheapest and most common style and is what SOLIDWORKS **Partial Edge Parameters** with a **Distance Offset** or **Percentage Offset** produces (the fillet runs full-radius to the specified offset and stops). Onshape's partial fillet with a start/end **Offset** is the same. It is **verified** as the production default.

### 1.2 Feathered / setback runout (radius tapers to zero over a runout length)

Instead of stopping at full radius, the radius is **ramped down to zero** over a short runout length, so the two spring curves **converge back onto the original sharp edge** and the bead vanishes to a point (or to a knife-edge). This is exactly the setback mechanism of `28` Section 5.2 and `41` Section 4.2, applied at a *free* end rather than at a vertex junction: `r(t) -> 0` as `t` approaches the runout end, and the spring curves meet the sharp edge where `r = 0`. SOLIDWORKS achieves this with a **Split Line** breaking the edge where the radius should be zero, then a variable-radius fillet pinned to `0` at that point (verified from the GoEngineer/Rhino-forum practice: "break the edge being filleted where the radius should be zero, creating an endpoint there"). The runout end has **no transverse cross face at all**: the two spring curves and the sharp edge all coincide at the vanishing vertex. This is the smooth, manufacturing-clean termination (a feathered fillet runout, as on a forging or a turbine-blade root).

### 1.3 Runout into a vertex (the blend ends at an existing model vertex)

The blended segment ends exactly at one of the edge's own end vertices, but the *adjacent* edges at that vertex are **not** blended. This is the boundary case between a full edge blend and a partial one: one end is a genuine vertex (handled by the vertex ladder of `28` Section 8, or by `41` `ExtentOneCorner`-style extension), the other end is an interior mid-edge stop (1.1 or 1.2). It is included here because a partial blend frequently has one vertex end and one interior end, so the surgery must compose a vertex-extremity treatment on one side with an interior-stop treatment on the other.

### 1.4 What Parasolid and ACIS expose

- **Parasolid.** Edge blends "can be trimmed (terminated) at a specific edge or face, providing localized and robust control of complex blend interactions" (Edge Blending Overview, q-solid mirror). The `range_1`/`range_2` reals control the along-edge extent and the across-face reach. The end is built by Parasolid's standard cap-by-face-extension (extend at most two adjoining faces to meet, `41` Section 1.6) when the stop lands on a transverse feature; for a genuinely interior stop the blend face is trimmed against a section plane. *Verified* for the range/trim capability and `range_1/range_2`; the literal interior-cap operator sequence is *not published* (claimed, reconstructed in Q3).
- **ACIS.** **Setbacks** are the explicit mechanism: "start and end setbacks applied to each end of the blended edge" position the **cross edge** that closes the blend end. Zero setback intersects the spring curves with adjacent geometry to place the cross edge; a positive setback pulls the cross edge **back** from the vertex, which is precisely the runout/retraction needed for a clean mid-edge or vertex termination. ACIS thus exposes the **square stop** (cross edge at a chosen parameter) and the **setback runout** (cross edge pulled back, with the bead feathered into the retracted region) as first-class. *Verified* from the ACIS Setbacks reference. The feather-to-zero is implemented via variable-radius blends (ACIS supports both rolling-ball snapshot and rolling-ball envelope variable-radius surfaces) with `r -> 0` at the setback end.

**Annotated source block.**

> **Citation.** Spatial Corp. (Dassault Systemes). *ACIS Blending Component Reference: Setbacks.* http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0011.HTM ; and Siemens / Parasolid, *Edge Blending Overview* (q-solid mirror) http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html .
> **Method/Content.** ACIS positions the blend's **cross edge** with per-end **start/end setbacks**; with zero setback the cross edge is placed by intersecting the blend's spring curves with adjacent blended-edge spring curves (the further intersection from the vertex). A positive setback retracts the cross edge from the vertex, which is the mechanism for a clean mid-edge stop and for vertex runout. Parasolid exposes blend trimming at a specified edge/face and `range_1/range_2` for along-edge and across-face extent.
> **Limitations.** Vendor docs describe option semantics, not the interior-cap Euler sequence. ACIS setbacks are documented mainly in the vertex context; the pure free-end interior stop is inferred from the same cross-edge machinery.
> **Kernel relevance.** Confirms the three termination styles are production-standard and gives Keel the cross-edge / setback vocabulary. The square stop = cross edge at parameter; the feather = variable radius to zero with retracted cross edge; the vertex runout = setback retraction. *Verified* for capability; *claimed* for the operator-level realization.

---

## Q2. The exact geometry of a constant-radius blend that tapers to zero over a runout between two planes (the central analytic-rung question)

This is the highest-value question in the dossier. Consider the most common runout: a constant-radius rolling-ball fillet on the dihedral edge between **two planes**, which over a runout length tapers its radius **linearly to zero**. The body of the blend (constant radius) is, by `40` Case A, an **exact circular cylinder** of radius `r` whose axis is the spine line `L_s` (the offset-line intersection). The question is the **taper region**: as `r` shrinks linearly along the straight spine toward the runout point, is the envelope a cone, a general developable, or necessarily a spline?

### 2.1 Setup: the envelope of a sphere of linearly shrinking radius on a straight spine

The rolling-ball blend surface is the **canal surface**, the envelope of the one-parameter sphere family
```
S(t):  |x - c(t)|^2 = r(t)^2,
```
with center `c(t)` on the spine and radius `r(t)`. For the constant-radius body the spine is the straight line `L_s` (offset-plane intersection, `40` Section 1.2) and `r(t) = r` is constant, giving a cylinder. For the **taper region** the spine is still the straight line `L_s` (the offsets of the two planes at *reduced* radius still intersect in a line parallel to `L_s`, only displaced), and the radius is **linear**: `r(t) = r0 * (1 - t/L)` over runout length `L`, vanishing at `t = L`.

So the taper region is the **envelope of a sphere whose center moves along a straight line and whose radius decreases linearly to zero.** This is, by definition, a **cone-sphere**, and its lateral envelope is an exact **right circular cone**.

### 2.2 The math (rigorous)

The envelope of the sphere family is found from the sphere equation `F(x,t) = |x - c(t)|^2 - r(t)^2 = 0` together with the **characteristic condition** `F_t = 0`. Differentiating,
```
F_t = -2 (x - c(t)) . c'(t) - 2 r(t) r'(t) = 0,
```
so the characteristic point set at parameter `t` lies in the plane
```
(x - c(t)) . c'(t) = - r(t) r'(t).                        (the characteristic plane)
```
Take the spine as the `z`-axis: `c(t) = (0, 0, t)`, so `c'(t) = (0,0,1)` and `|c'| = 1`. The characteristic plane becomes `z - t = -r(t) r'(t)`, i.e. the **characteristic circle** for parameter `t` sits at signed distance
```
d(t) = r(t) r'(t)              (measured along the spine, from the sphere center)
```
from the center `c(t)`, in the plane normal to the spine, and has radius (from the sphere relation `|x-c|^2 = r^2` restricted to that plane)
```
rho(t) = sqrt( r(t)^2 - d(t)^2 ) = r(t) * sqrt( 1 - r'(t)^2 ).
```
(These are exactly the canal-surface characteristic-circle formulas: distance `d = r r' / |c'|`, radius `sqrt(r^2 - d^2)`, valid when `|r'| < |c'| = 1`; the channel-surface envelope exists iff `|r'(t)| < |c'(t)|`.)

Now impose **`r(t)` linear**: `r(t) = -m * t + b` with constant slope `r'(t) = -m` (here `m = r0/L > 0` for a shrink to zero at `t = L = b/m`). Then:
- The characteristic circle center sits at `z = t + d(t) = t - r(t) m`. Substituting `r(t) = -mt + b`: `z = t - m(-mt + b) = t(1 + m^2) - mb`, an **affine** function of `t`: the circle centers march along the `z`-axis at constant speed.
- The characteristic circle radius is `rho(t) = r(t) sqrt(1 - m^2) = (-mt + b) sqrt(1 - m^2)`, which is **linear in `t`** and **goes to zero exactly when `r(t) -> 0`** (at `t = b/m`).
- The half-angle of the surface: the characteristic circle radius `rho` decreases linearly while its center advances linearly along the axis, so the locus of circles is a surface whose meridian is a **straight line** making a constant angle with the axis. The half-angle `alpha` satisfies `sin(alpha) = r'(t) = m` (the tilt of the characteristic circle is governed by `r'`, and for constant `r'` the tilt and the meridian slope are constant). Equivalently `sin(alpha) = |r'| = m`, `cos(alpha) = sqrt(1 - m^2)`.

Constant half-angle + straight axis + circular cross-sections shrinking linearly to a point is the definition of a **right circular cone**. The apex is at the point where `r(t) = 0`, i.e. the runout vanishing point on the sharp edge; the cone's half-angle is `alpha = arcsin(m) = arcsin(r0/L)`.

**Therefore the taper region between two planes, constant-to-zero linear radius along a straight spine, is an EXACT right circular cone.** Not a general developable that happens to be ruled, not a spline: a native analytic cone, storable in Keel's cone primitive with no NURBS approximation. This is the analytic rung, and it is the direct cousin of `40` Section 1's catalog row "**Cone** half-angle `alpha`: coaxial cone offset" and of the `48`/memory note that a variable-radius blend between two planes is an exact cone.

### 2.3 Cone vs developable vs spline, decided

- **Cone (the answer for the canonical runout):** straight spine + linear radius + planar (or constant-curvature coaxial) supports. The cone is itself a developable surface, so this is the *developable* answer in its sharpest form: a specific developable, the cone. Half-angle `arcsin(r'/|c'|)`; exact.
- **General developable (a weaker rung):** if the spine is straight but the radius taper is **non-linear** (e.g. a smooth C1 ramp `r(t)` with `r'` varying), the meridian is no longer a straight line of constant slope, so the envelope is a **canal surface of varying half-angle**: still ruled by the characteristic circles' tangent cones locally, but **not a single cone**. It is developable only in the degenerate constant-`r'` (cone) and constant-`r` (cylinder) cases; for a curved `r(t)` on a straight spine the canal surface is **not developable in general** (channel surfaces are developable only when the directrix is a line and the radius is constant, giving a cylinder, per the channel-surface developability condition). So a non-linear taper is a true canal surface, not a developable.
- **Spline (the general rung):** if the supports are non-planar (so the spine is curved) **or** the radius taper is non-linear, the canal surface has no analytic-primitive form and is stored as a **certified NURBS canal approximation** (`40` Section 5), exactly as a variable-radius blend already is.

**One-line verdict for the canonical case: the zero-taper region between two planes with a linear radius ramp is an EXACT right circular cone (developable, no spline needed); curve the spine or curve the radius law and it degrades to a non-developable canal surface stored as a certified NURBS fit.**

**Annotated source block.**

> **Citation.** "Channel surface," Wikipedia, https://en.wikipedia.org/wiki/Channel_surface ; and Bronsvoort/Klok lineage on cone-spheres: J. Bonet et al. / "Cone-spheres," *ACM SIGGRAPH Computer Graphics* 24(4), 1990, https://dl.acm.org/doi/10.1145/97880.97885 ; and the canal-surface envelope derivation in "The implicit equation of a canal surface," arXiv:0806.4127.
> **Method/Content.** A channel (canal) surface is the envelope of a one-parameter sphere family with center `c(t)`, radius `r(t)`; the characteristic circle lies in the plane `(x-c).c' = -r r'`, at distance `d = r r'/|c'|` from the center with radius `sqrt(r^2 - d^2)`, existing iff `|r'| < |c'|`. Wikipedia lists verbatim: "**right circular cone (canal surface, directrix is a line (the axis), radii of the spheres not constant)**" and "right circular cylinder (pipe surface, directrix a line, radius constant)." A cone-sphere is "a surface generated by sweeping a sphere along a straight line segment with the radius changing linearly; thus a truncated circular cone capped by spheres at the two ends." Channel surfaces are developable only when the directrix is a line and the radius is constant (the cylinder).
> **Limitations.** The cone result requires planar (or coaxial-quadric) supports so the spine stays straight (or a circle) and the radius law be linear. Off these conditions the envelope is a non-developable canal surface. The capping spheres of the cone-sphere are not part of the blend (the blend is the lateral cone only); the apex/vanishing point is a genuine surface singularity (zero-radius point) that needs care in parameterization and tessellation.
> **Kernel relevance.** This is the analytic rung for the runout. The constant-radius body is a cylinder (`40` Case A); the linear-taper runout is an **exact cone** with apex at the vanishing point and half-angle `arcsin(r0/L)`. Keel stores both natively, with the cone trimmed between the two converging spring lines and the cylinder, joined G1 at the taper-start cross section (where the cone half-angle meets the cylinder's zero half-angle the surfaces are tangent only if the slope matches; see the runout-join note in Q5). *Verified* by the canal-surface math and the explicit "cone is a canal surface with linear radius on a line directrix" statement.

---

## Q3. The topology of a mid-edge stop: spring lines that start and stop at interior points, and the residual sharp edge

The new topological object is a blend ribbon whose **two transverse ends lie inside the two support faces**, with the original sharp edge `e` **resuming** beyond each end. Call this a **cap-into-supports** end (contrast `44`'s cap-shrink and cap-grow, where the end lands on a third, transverse face).

### 3.1 What terminates the blend face at an interior stop

At a square-stop end (Q1.1) the blend face `B` is bounded by:
- the two **spring curves** `s1` (on support `F1`) and `s2` (on support `F2`), which now **start at interior points** of `F1` and `F2` rather than at points on a cap edge;
- the transverse **cross curve** `x` at the stop section (the section arc/line in the plane normal to the spine);
- and, crucially, **two new short edges**: the residual sharp edge `e` must resume, so the surgery leaves an edge fragment of `e` on the unblended side. The stop section's two endpoints (where `s1` and `s2` meet the cross curve `x`) are **two new vertices** that sit on the original sharp-edge curve's position, and from them the **residual sharp edge** `e_res` runs off to the original end vertex.

So at one interior stop the topology introduces: two new vertices `p1` (end of `s1`) and `p2` (end of `s2`); the cross curve `x` joining them; and the residual sharp edge `e_res` continuing from the junction. Because `e` is a single curve and the blend consumes only its interior interval, the stop is where `e` is **split**: the blended interval is removed, the unblended interval `e_res` remains.

### 3.2 The key difference from cap-to-cap

In Keel's existing cap-to-cap surgery (and in `44`), each spring line runs from a vertex on one cap edge to a vertex on the other cap edge: it **starts and stops on the boundary of the support face**. For a mid-edge stop the spring line must **start and stop at an interior point of the support face** (a point with no pre-existing vertex or edge through it). Imprinting a curve that starts at an interior point requires a **vertex on a face interior**, which in GWB terms is introduced by `mev` from an existing boundary vertex into the face interior (a "spur"), or by `kemr`/`mekr` if the imprinted spring line forms an inner loop. For a partial blend with two interior stops the spring line is an **open** curve with both endpoints interior to the face: it is imprinted by a `split_face` whose new edge runs interior-to-interior, which in a half-edge kernel is realized as an `mev` spur to the first interior point, an `mef` to the second, then the residual cleanup. This is the generalization Q4 details.

### 3.3 Worked Euler sequence for one interior square stop (plane-plane fillet, one end interior, V/E/F/R bookkeeping)

Setup: a box edge `e` between planes `F1`, `F2`, end vertices `va` (left) and `vb` (right). We blend only the interval `[t0, t1]` of `e` with constant radius `r`; both ends `t0` and `t1` are **interior** (away from `va`, `vb`); the sharp edge resumes as `e_left` (from `va` to the `t0` stop) and `e_right` (from the `t1` stop to `vb`). We use the GWB operators of `01`/`44`: `split_edge` (+1V,+1E), `mev` (+1V,+1E), `mef`=`split_face` (+1E,+1F), `kev` (-1V,-1E), `kef` (-1E,-1F). Invariant `V - E + F = 2(S - G) + (L - F)` asserted after each line; here S, G unchanged and R = L - F tracked.

We first build the **left interior stop** (the `t0` end); the `t1` end is symmetric.

| Step | Operator | Action at the t0 (left) interior stop | dV | dE | dF | dR |
|---|---|---|---|---|---|---|
| 1 | `split_edge` on `e` at `t0` | Split the sharp edge `e` at the stop parameter, creating the junction vertex `q0` on `e`. `e` becomes `e_left` (va to q0) and the to-be-blended remainder. | +1 | +1 | 0 | 0 |
| 2 | `mev` into `F1` interior from `q0` | Spur the spring-curve landing `p1` into `F1`'s interior: make edge `q0->p1` and vertex `p1`. (`p1` is the interior point where spring `s1` begins.) | +1 | +1 | 0 | 0 |
| 3 | `mev` into `F2` interior from `q0` | Spur the spring-curve landing `p2` into `F2`'s interior: make edge `q0->p2` and vertex `p2`. | +1 | +1 | 0 | 0 |
| 4 | `mef` = `split_face` `p1->p2` across the corner | Draw the **cross curve** `x` (the stop-section arc) from `p1` to `p2`, splitting off the small **stop end region** and creating the cross edge. This `mef` adds the cross edge and (on the first interior end) opens the loop that will become blend face `B`. | 0 | +1 | +1 | 0 |
| 5 | `split_face` `s1` along `F1` | Imprint spring curve `s1` from `p1` running along `F1` toward the blended interval (the spring line proper). Splits `F1` into keep-strip and trim-strip. | 0 | +1 | +1 | 0 |
| 6 | `split_face` `s2` along `F2` | Imprint spring curve `s2` from `p2` along `F2`. Splits `F2`. | 0 | +1 | +1 | 0 |

Then the blended interval runs to the right interior stop `t1`, where the **symmetric** sequence (steps 1'-4', spurs `mev` to `p1'`,`p2'` from junction `q1`, `mef` for the right cross curve) closes the ribbon. Between the two stops, the spring curves `s1`, `s2` and the sharp-edge removal proceed as in the cap-to-cap surgery: the trim-strips between each spring curve and the (interior) sharp-edge interval are removed, and the **interior interval of `e`** (from `q0` to `q1`) is dissolved with `kef`/`kev`, leaving `e_left` and `e_right` as the resumed sharp edges. Finally:

| Step | Operator | Action | dV | dE | dF | dR |
|---|---|---|---|---|---|---|
| 7 | `kef` x2 (per stop) | Merge each trim-strip sliver into the blend face `B` (dissolve the wall between trim-strip and `B`), as in the convex baseline `44`. | 0 | -1 each | -1 each | 0 |
| 8 | `kev` on dissolved interior `e`-vertices | Remove the now-interior spur vertices left by the sharp-edge-interval dissolution. | -1 each | -1 each | 0 | 0 |
| 9 | attach analytic surface | Attach the exact cylinder (`40` Case A) to `B`, trimmed between `s1`, `s2`, and the two cross curves; tag `s1`, `s2` SMOOTH (`39`). | 0 | 0 | 0 | 0 |

**Global bookkeeping.** Across the whole partial-span insertion the net is the **same global delta as a full edge blend**: exactly **+1 face** (the single blend face `B`), with `S`, `G` unchanged and `R` unchanged (no inner ring, no handle) for the simple box edge. The difference from cap-to-cap is entirely local at the two ends: instead of consuming a corner triangle on a transverse cap face (convex `44`) or growing a lune (concave `44`), each end **splits the sharp edge** (`split_edge`, the `q0`/`q1` junction vertices), **spurs two interior landing vertices** (`mev` x2), and **closes a cross curve** (`mef`) that sits entirely inside the support-face corner. The residual sharp edges `e_left`, `e_right` are the surviving fragments of the split `e`. Euler-Poincare holds at every line because each operator is a GWB make/kill pair member with a fixed delta (`01` Section 3.2, `44`).

### 3.4 The runout (feathered) end variant

For the feathered runout end (Q1.2) the cross curve `x` **degenerates to a point**: the stop section radius is zero, so `p1`, `p2`, and the junction `q` all **coincide at the vanishing vertex** on the sharp edge. Steps 2-4 collapse: there is no cross edge and no separate stop region; instead the two spring curves `s1`, `s2` **converge to the single vanishing vertex** `q_v` on `e`, and the blend face `B` ends in a **triangular (3-sided) tip** bounded by `s1`, `s2` meeting at `q_v`. Topologically this is an `mev` spur to `q_v` is unnecessary, the two spring curves simply terminate at the same vertex; the surface is the **cone** of Q2 with its apex at `q_v`. The runout end therefore *adds no cross edge and no new transverse face*, it only adds the vanishing vertex `q_v` (which is the `split_edge` point on `e`). Net for a feathered end: `split_edge` on `e` at the runout point (+1V,+1E), spring curves run to it, surface = cone with apex there. This is topologically simpler than the square stop (no cross edge) and is the reason a feathered runout is often the **more robust** termination despite the zero-radius singularity.

**Annotated source block.**

> **Citation.** Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press, GWB Euler operators; and `44-concave-fillet-topology-surgery.md` (cap-shrink/cap-grow deltas); Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer (operator-level blend recipes).
> **Method/Content.** Every step is a GWB operator with a fixed validity-preserving count delta; the interior stop introduces a junction vertex (`split_edge` on the sharp edge), two interior spring-curve landing vertices (`mev` spurs into the face interiors), and a cross edge (`mef`) lying inside the support-face corner; the feathered variant collapses the cross edge to the apex vanishing vertex. The global delta matches a full edge blend (+1 face), differing only in the local end treatment.
> **Limitations.** The sequence is the operator-level reconstruction consistent with the documented Parasolid trim-at-edge/face and ACIS cross-edge/setback behavior; the vendors do not publish a literal operator list. The interior-to-interior imprint needs an `mev` spur because the spring line begins at a face-interior point with no pre-existing vertex; kernels with a one-stroke "imprint open curve into face interior" primitive collapse steps 1-3.
> **Kernel relevance.** This is the exact surgery Keel must add: a `cap_into_supports` end alongside the existing cap-to-cap path. The bookkeeping shows it preserves Euler-Poincare and nets +1 face, so the existing shell-closure and mass==mesh gates apply unchanged. *Verified* for the Euler algebra; *claimed* for the precise vendor realization.

---

## Q4. Generalizing Keel's `imprint_spring_line` (cap-to-cap) to interior start/stop

Keel's `imprint_spring_line` today spans cap-to-cap: each spring line runs from a vertex on one transverse cap edge to a vertex on the other cap edge, so **both endpoints already exist as boundary vertices** of the support face, and the imprint is a `split_face` whose new edge connects two pre-existing boundary vertices. The generalization to a mid-edge stop requires the spring line to **begin and/or end at an interior point** of the support face, which has three sub-cases:

### 4.1 The three endpoint kinds for a generalized spring line

1. **Boundary endpoint (existing).** The spring line ends on a bounding edge of the support face; introduce the landing vertex with `split_edge` on that bounding edge (current behavior), then `split_face` to the other endpoint.
2. **Interior endpoint at a sharp-edge stop (new).** The spring line ends at an interior point `p_i` that is the foot of the stop section on the support, with the junction vertex `q` on the (now split) sharp edge. Introduce `p_i` by `mev` spurring from `q` into the face interior (Q3 step 2/3). The spring line then `split_face`s from `p_i` along the support.
3. **Interior endpoint at a feathered runout (new).** The spring line ends at the **vanishing vertex** `q_v` on the sharp edge (radius zero), shared by both spring lines. No spur is needed; both spring lines terminate at `q_v`.

### 4.2 The minimal generalization

`imprint_spring_line(support, curve, start_endpoint, end_endpoint)` is generalized so that each of `start_endpoint`, `end_endpoint` carries a **kind tag**: `OnBoundary(edge)`, `AtSharpStop(junction_vertex)`, or `AtRunout(vanishing_vertex)`. The function dispatches:
- `OnBoundary` -> `split_edge` on the named bounding edge (existing path);
- `AtSharpStop` -> `mev` spur from the junction vertex into the face interior to create the landing vertex, then proceed;
- `AtRunout` -> reuse the shared vanishing vertex (created once by `split_edge` on the sharp edge), no spur.

The body of the imprint (the `split_face` carrying the spring curve and its pcurve, the trim-strip identification, the `kef`/`kev` dissolution of the trim-strip into the blend) is **unchanged**: it already operates on whatever endpoints the spring line has. The only new code is the **endpoint preparation** (the `mev` spur for the interior case and the `split_edge` on the sharp edge to make the junction/vanishing vertex). This is a small, well-contained extension, not a rewrite, which is the reason to do it in-place rather than building a separate partial-blend pipeline.

### 4.3 Composing with the existing trim-and-stitch

Because the spring-line imprint is the same `split_face` + trim-strip dissolve, the rest of Keel's trim-and-stitch (split caps along end arcs is replaced here by **close cross curve at the stop**, kef/kev dissolution of the sharp-edge interval) composes directly. The cap-to-cap "split caps along end arcs" step is the special case where the end arc lies on a transverse cap face; the mid-edge stop replaces it with "close the cross curve inside the support corner" (Q3 step 4) for a square stop, or "converge the spring lines to the apex vertex" for a feathered runout. The sharp-edge **interior interval** is dissolved exactly as the full sharp edge is dissolved today (`kef`/`kev`), the only difference being that `split_edge` at `t0`, `t1` first carves out `e_left`, `e_right` so the dissolution touches only the interior interval.

**Annotated source block.**

> **Citation.** Keel in-repo machinery (`imprint_spring_line`, trim-and-stitch edge-blend surgery; cap split along end arcs; kef/kev dissolution) as described in the dossier brief and `44`; generalization grounded in Weiler, K. (1988), radial-edge structure (interior-vertex spurs and face-interior imprint), and Lee & Lee (2001), partial entity structure (`02`).
> **Method/Content.** Tag each spring-line endpoint as OnBoundary / AtSharpStop / AtRunout; dispatch endpoint preparation (split_edge / mev-spur / shared-vertex) while leaving the split_face + trim-strip dissolve unchanged. The interior endpoint is the only genuinely new construction: an `mev` spur creates a face-interior landing vertex with no pre-existing topology.
> **Limitations.** The interior spur relies on the support face being a single face at the stop (not already subdivided by another feature at that point); if another imprint already crosses there, the landing vertex is an `OnBoundary` case against that imprint's edge instead. The generalization assumes the stop section is transverse to the spine (a clean cross curve); skew stops need the cross curve solved as the section at the stop parameter.
> **Kernel relevance.** Shows the partial-span capability is an additive extension of existing code (endpoint-kind dispatch + an `mev` spur), preserving the mass==mesh and shell-closure gates. *Verified* as a topological reduction; the exact API shape is a Keel design choice.

---

## Q5. Closed form for the zero-radius runout envelope

**Yes, there is a closed form, and it is the cone of Q2.** For the canonical runout (constant-radius fillet between two planes tapering linearly to zero over runout length `L`), the entire runout surface is an **exact right circular cone**:

- **Axis:** the spine line `L_s` (the intersection of the two reduced-offset planes; for two planes it is parallel to the full-radius spine, displaced as `r` shrinks; for the symmetric planar dihedral it is the **angle bisector line through the dihedral, converging to the sharp edge**).
- **Apex:** the vanishing point on the sharp edge `e` where `r = 0` (the runout vertex `q_v`).
- **Half-angle:** `alpha = arcsin(r'(t)) = arcsin(r0 / L)` for the linear ramp (slope `m = r0/L`), with the constraint `r0/L < 1` (otherwise `|r'| >= |c'|` and the envelope degenerates / self-intersects, which is the `|r'| < |c'|` existence condition).
- **Trim:** the cone is trimmed between the two **spring lines** (the contact curves on the two planes, which on a developable cone are themselves straight lines or conics depending on the support, and which converge to the apex), and at the taper-start by the cross section where it meets the constant-radius cylinder body.

The **constant-radius body is an exact cylinder** (`40` Case A), the **runout is an exact cone**, and the two meet at the taper-start section. Note the join is **G0 (positional) by construction but G1 only if the slopes match**: a cylinder has axis-parallel rulings (zero half-angle in the canal sense), while the cone has half-angle `arcsin(m) > 0`, so a *sharp* transition from cylinder to cone has a tangent crease at the taper-start. For a **G1 (tangent-continuous) runout** the radius law `r(t)` must have `r'(t) -> 0` at the taper-start (so the cone half-angle eases in from zero), which makes the taper region a **non-developable canal surface** (varying half-angle) rather than a single cone, stored as a certified NURBS fit. So there is a clean trade:

- **Linear taper (sharp half-angle):** exact cylinder + exact cone, with a tangent crease at the join. Closed form, exact oracle, but a visible crease (acceptable for engineering, not Class-A).
- **Eased taper (`r'` ramps from 0):** smooth G1 join, but the runout is a canal-surface NURBS fit, no single-primitive closed form.

For the **degenerate dihedral case** where the two supports are planes and the runout converges all the way to the sharp edge with the spring lines meeting at the apex, the cone degenerates to its apex and the runout is a **spherical-tip** (the final rolling ball shrinking to zero is a sphere of radius `-> 0`), so the very tip is a point singularity, the apex of the cone.

**Annotated source block.**

> **Citation.** Channel surface envelope theory (Wikipedia, arXiv:0806.4127, as in Q2); cone-sphere bounding-volume literature, "Cone-spheres," *SIGGRAPH Computer Graphics* 24(4), 1990, https://dl.acm.org/doi/10.1145/97880.97885 ; cross-reference `40` Section 1 (offset cone row) and the Keel memory note (variable-radius blend between planes is an exact cone).
> **Method/Content.** Closed form: cone with axis = spine line, apex = zero-radius vanishing point, half-angle = `arcsin(r0/L)`. Exists iff `r0/L < 1`. The cylinder-to-cone join is G0 for a linear taper (tangent crease), G1 only if `r'` eases to zero, which converts the runout to a non-developable canal-surface fit.
> **Limitations.** Exact cone only for planar (or coaxial-quadric) supports and linear radius. The apex is a point singularity (zero-radius rolling ball); tessellation and parameterization must handle the degenerate tip. The G1-eased variant has no single-primitive closed form.
> **Kernel relevance.** Gives Keel an **exact oracle** for a partial-span runout fillet: the swept volume removed/added is a closed-form cylinder-plus-cone, so mass can be computed analytically and asserted against mesh and against the reference. *Verified.*

---

## Cross-cutting three-bucket catalog (the highest-value ask)

The configurations of a partial-span / runout blend sort into three buckets by how their surface is best represented.

**(a) EXACT ANALYTIC (native primitive, no NURBS).**
- Constant-radius **body** between two planes (or plane-cylinder parallel-axis, etc.): exact **cylinder** (`40` Case A).
- Constant-radius **body** between configs with a circular spine (plane-sphere, boss-base, coaxial quadrics): exact **torus** (`40` Case B).
- **Linear-taper runout** between two planes (or any straight-spine config) tapering to zero: exact **right circular cone**, apex at the vanishing point, half-angle `arcsin(r0/L)` (Q2, Q5). This is the analytic rung of the dossier.
- **Square-stop end cross face** when the stop section lies in a plane: exact **planar** end patch (its boundary is the section arc plus two sharp-edge fragments).
- **Chamfer** partial span between planes: exact **plane** (body) and exact **plane/cone** runout (the ruled chamfer tapering to zero is a triangular planar or conical facet).

**(b) PROCEDURAL-EVALUATOR that a sample-and-certify NURBS fit can cache.**
- **Eased (G1) taper** runout (`r'` ramps to zero at the taper-start for a crease-free join): a non-developable canal surface of varying half-angle; procedural canal evaluator (spine + variable `r(t)` + supports), cached as a certified NURBS to tolerance.
- **Variable-radius partial blend** with a non-linear radius law over the span (taper not pure linear): canal surface, procedural evaluator, NURBS cache (same machinery as `28` Section 3 variable radius, restricted to the span).
- **Curved-spine runout** (supports not both planar/coaxial, so spine is curved): canal surface, procedural, NURBS cache.
- **Stop cross face on a curved support** where the section end is not planar: ruled/section evaluator, NURBS cache.

**(c) GENUINELY FULL GENERAL MACHINERY (no shortcut).**
- **Runout into a vertex with mixed convexity** (one adjacent edge convex, one concave at the runout-end vertex): the `44` mixed-convexity hard case composed with a runout; needs the full setback + n-sided vertex patch (`28` Section 8, `41` `Extent*Corner`) plus the bead-construct-and-sew of `44` Q3. No analytic rung.
- **Runout that overflows** its support mid-taper (the shrinking spring curve runs off a short face): full overflow handling (smooth/cliff/notch, `28` Section 6, `41`) layered on the taper.
- **Partial blend whose span boundary interacts with another feature** (another blend, a hole loop, a third edge crossed by the spring curve as it tapers): full feature-interaction / blend-graph sequencing (`28` Section 8).

The leverage: the **most common** partial-span request, a constant-radius fillet between planes that **runs out to zero**, is entirely in bucket (a), an exact cylinder-plus-cone, with an exact closed-form volume oracle. Keel should build that first and reach for buckets (b)/(c) only as the supports curve, the taper eases, or the ends get congested.

---

## Synthesis: ADOPT / AVOID / INVESTIGATE, first milestone, ladder, declines

**ADOPT.**
1. **A `cap_into_supports` end type** beside `44`'s cap-shrink and cap-grow, realized by generalizing `imprint_spring_line` to interior start/stop endpoints (Q4): tag endpoints OnBoundary / AtSharpStop / AtRunout, add an `mev` spur for the interior landing vertex, otherwise reuse the existing split_face + trim-strip dissolve. Additive, not a rewrite.
2. **The exact cone runout** (Q2/Q5): represent the linear-taper-to-zero region as a native cone (apex at the vanishing point, half-angle `arcsin(r0/L)`), joined to the exact cylinder body. Closed-form volume oracle.
3. **The square stop with a planar cross face** as the default mid-edge termination (Q1.1), matching SOLIDWORKS Partial Edge / Onshape partial fillet / Parasolid trim-at-edge-or-face / ACIS cross-edge.
4. **The feathered runout** as the second termination (Q1.2), built by the variable-radius blend with `r -> 0`, topologically simpler (no cross edge, spring lines converge to the apex vertex).
5. **ACIS-style setback** vocabulary: a per-end setback distance that retracts the cross edge, reused for both vertex runout and mid-edge stop.

**AVOID.**
1. **A separate partial-blend pipeline.** The mid-edge stop is the cap-to-cap surgery with a different end treatment; a parallel pipeline duplicates the trim-and-stitch and diverges under maintenance. Generalize in place.
2. **Forcing a G1 cylinder-to-cone join for a linear taper.** A linear taper has an honest tangent crease at the taper-start; do not approximate it away with a NURBS fit unless the user asks for an eased (Class-A) runout. Keep the exact cone for engineering runouts.
3. **Splining the simple runout.** The canonical runout is an exact cone; never store it as a NURBS canal fit (loses the exact oracle and the analytic recognition for later unblend).

**INVESTIGATE.**
1. **The eased-taper canal fit** (bucket b): the `r'`-ramps-to-zero G1 runout as a procedural canal evaluator with a certified NURBS cache, sharing the variable-radius machinery of `28` Section 3, restricted to the span. Worth it for Class-A runouts; defer until the exact cone path ships.
2. **Mixed-convexity runout-into-vertex** (bucket c): compose the `44` mixed-convexity bead-construct-and-sew with the setback runout. Hard; defer.
3. **Recognition of partial-span blends for unblend** (`28` Section 9): a partial fillet leaves a cone-plus-cylinder pair with a residual sharp edge on either side; the unblend recognizer must detect the cone runout (constant half-angle, apex on the resumed sharp edge) and re-extend the supports across the whole edge. The exact cone makes this recognizable; investigate the recognizer.

**Recommended FIRST MILESTONE (narrowest honest slice with an exact oracle).** A **constant-radius fillet on a single straight dihedral edge between two planes, blended over an interior interval `[t0, t1]`, with a square stop at `t0` and a linear feathered runout to zero at `t1`.** This exercises: (i) the interior-stop surgery (Q3/Q4) at `t0`, (ii) the exact cone runout (Q2/Q5) at `t1`, (iii) the exact cylinder body, (iv) the residual sharp edges `e_left`, `e_right`. The **exact oracle**: the material removed is a closed-form cylinder segment plus a cone segment plus the planar stop cap, whose volume is computed analytically; assert `mass == mesh == ref` and shell-closure, exactly Keel's existing gates. No NURBS, no marching, no vertex patch. This is the bucket-(a) slice and the honest minimum.

**Follow-up ladder (in order).**
1. Square stop at both ends (two interior stops, the symmetric Q3 sequence).
2. Partial span on a **curved spine** (plane-cylinder, circular spine): body becomes a torus segment (`40` Case B), runout becomes a non-developable canal NURBS fit (bucket b) or a cone only in the coaxial case.
3. Variable-radius partial blend over the span (non-linear law), bucket (b) procedural canal + NURBS cache.
4. Partial blend with **one vertex end and one interior end** (Q1.3 composition).
5. Eased (G1) Class-A runout (bucket b investigate item).
6. Overflow during taper and mixed-convexity runout-into-vertex (bucket c).
7. Recognition/unblend of partial-span blends.

**DECLINE forever (with justification).**
- **A "spline-only" partial-span engine that never specializes the cone/cylinder.** Justified: it throws away the exact oracle and the analytic recognition that make the common case both verifiable and reversible; the cone/cylinder specialization is cheap and high-leverage. This mirrors the Keel pattern of always taking the analytic rung when one exists.
- **G1-eased runouts as the default.** Justified: the honest linear taper is exact and crease-only-at-one-section; defaulting to an eased NURBS canal would silently drop exactness for a cosmetic gain most engineering parts do not need. Offer eased runouts as an explicit Class-A option, never the default.

---

## References

1. Spatial Corp. (Dassault Systemes). *ACIS Blending Component Reference: Setbacks.* http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0011.HTM
2. Spatial Corp. *ACIS Blending Component (BLND/01CMP)* and *Variable Radius Blending.* http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF
3. Siemens / Parasolid. *Edge Blending Overview; Functions and Options.* http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html ; .31.html ; V35: http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.075.html
4. SOLIDWORKS. *Advanced Part Modeling: Partial Fillet Option Through Partial Edge* (SOLIDWORKS 2019-2020, Partial Edge Parameters: Distance/Percentage/Reference Offset; Start/End Condition). https://help.solidworks.com/2019/English/WhatsNew/t_creating_partial_chamfers_and_fillets.htm ; GoEngineer, *SOLIDWORKS Partial Chamfers & Fillets Explained.* https://www.goengineer.com/blog/solidworks-partial-chamfers-and-fillets-explained
5. Onshape. *Fillet* (partial fillet, start/end position percentage/offset, second bound). https://cad.onshape.com/help/Content/fillet.htm
6. *Channel surface* (canal/channel surface envelope; characteristic circle `d = r r'/|c'|`, radius `sqrt(r^2 - d^2)`, existence `|r'| < |c'|`; cone listed as a canal surface with line directrix and non-constant radius; developability only for line directrix + constant radius). https://en.wikipedia.org/wiki/Channel_surface
7. *The implicit equation of a canal surface.* arXiv:0806.4127. https://arxiv.org/pdf/0806.4127
8. *Cone-spheres.* ACM SIGGRAPH Computer Graphics 24(4), 1990 (sphere swept along a line with linearly changing radius is a truncated circular cone capped by spheres; cone-spheres bound canal surfaces). https://dl.acm.org/doi/10.1145/97880.97885
9. McNeel / Rhino forum and GoEngineer SOLIDWORKS tutorial: variable-radius fillet to zero radius via split line at the vanishing point ("break the edge where the radius should be zero, creating an endpoint there"). https://discourse.mcneel.com/t/variable-radius-fillet-near-zero-radius-points/37928 ; https://www.goengineer.com/blog/solidworks-tutorial-using-split-lines-with-variable-size-fillets
10. Open CASCADE Technology. *BRepFilletAPI_MakeFillet* (SetRadius with Law_Function / SetLaw for variable radius along an edge parameter; ChFiDS_FilSpine). https://old.opencascade.com/doc/occt-7.0.0/refman/html/class_b_rep_fillet_a_p_i___make_fillet.html
11. Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press. (GWB Euler operators; fixed count deltas.) https://archive.org/details/introductiontoso0000mant
12. Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer. (Operator-level blend/chamfer recipes as Euler-operator sequences.) https://link.springer.com/book/10.1007/978-1-84628-616-2
13. Weiler, K. (1988). The radial edge structure. In *Geometric Modeling for CAD Applications*, 3-36. North-Holland. (Face-interior vertex spurs; radial splice.)
14. Lee, S. H., & Lee, K. (2001). Partial entity structure. *J. Comput. Inf. Sci. Eng., 1*(4), 356. (PES operators used by Keel.)
15. Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365. (Spine/spring/range vocabulary.)
16. Lukacs, G. (1998). Differential geometry of G1 variable radius rolling ball blend surfaces. *Computer Aided Geometric Design, 15*(6), 585-613. (Variable-radius canal regularity; the `|r'| < |c'|` style validity bound applied to the taper.)
17. Chuang, J.-H., & Hwang, W.-C. (1997). Variable-radius blending of parametric surfaces. *The Visual Computer, 13*(7), 316-329. (Variable-radius marching for the eased-taper bucket-b case.)
18. Siemens NX. *Edge Blend: Stop Short of Corner, Corner Setback* (NX exposes a stop-short-of-corner / setback control for partial termination). https://www.youtube.com/watch?v=K_8pqqW1geo
19. CATIA. *Variable Radius Fillet* (radius points along an edge; taper control). http://catiadoc.free.fr/online/fm1ug_C2/fm1ugbt0414.htm
20. *Notch blends in BRep models.* US Patent 8,935,130 (construct-and-sew bead, cap-by-face-extension; cross-referenced for the mixed-convexity runout-into-vertex bucket-c case). https://patents.google.com/patent/US8935130
21. Choi, B. K., & Lee, C. S. *Method for generating an extended rolling ball fillet surface between two surfaces.* US Patent 5,774,359 (Autodesk; offset/envelope spine extension, roll-on at a runout end). https://patents.google.com/patent/US5774359A/en

**Patent awareness.** The partial/range/setback *capability* is decades-old prior art (ACIS setbacks, Parasolid range_1/range_2, the rolling-ball constructions of US 5,774,359 / US 5,345,546, all 1990s and **expired**), so the core engine is unencumbered. The **exact cone runout** rests on classical canal-surface/cone-sphere geometry (public domain, 1990 SIGGRAPH and older differential geometry), not on any patented method. Modern CAD *UI* features (SOLIDWORKS Partial Edge 2019/2020, Onshape partial fillet) are recent but are **interface conveniences over the same old geometry**; Keel implements the underlying square-stop / feathered-runout surgery from first principles (Euler operators + canal-surface math), which is a claim-element design-around: it reads on no specific UI patent because it claims neither the UI nor any post-2006 surface-construction method. The notch/construct-and-sew patent US 8,935,130 (post-2006) is cited only for the mixed-convexity bucket-c fallback; for the bucket-(a) common case Keel uses the in-place loop-regrow of `44` (prior-art Euler-operator surgery), avoiding that claim entirely.

## Related Keel dossiers (build on, not repeated)

- `28-fillet-chamfer-engine.md`: Sections 5 (range/setback control, retractable edge blends), 6 (overflow/runout taxonomy), 3 (variable-radius depth). The engine vocabulary this dossier specializes to mid-edge termination.
- `40-blend-fillet-surface-generation.md`: Area 1 exact-analytic catalog (line spine to cylinder, circle spine to torus, the offset cone row). The body and the cone runout are entries in that catalog.
- `41-blend-overflow-radius-feature-failures.md`: Sections 4.2 (setback/retraction), 4.5 (partial blend recovery), the LFS feasibility bounds. The partial blend as a recovery and as a deliberate feature.
- `44-concave-fillet-topology-surgery.md`: the cap-shrink (convex) and cap-grow (concave) Euler surgeries and the `sigma` convexity sign; this dossier adds the third `cap_into_supports` extremity. The mixed-convexity runout-into-vertex reuses `44` Q3/Q5.
- `39-coincident-tangent-face-booleans.md`: the tangent-sew and tolerant-edge treatment of the G1 spring-curve seam, reused unchanged at every spring line.
