# The Complete Fillet and Chamfer Engine: Industrial Edge-Finishing Operations

Research dossier 28 for the Keel B-rep solid modeling kernel. This file completes the fillet/chamfer treatment with implementation-grade detail. An earlier dossier, nurbs/05, established the blending theory (Vida-Martin-Varady taxonomy, rolling-ball and canal-surface foundations, variable-radius blends in the abstract, setback vertex blends). A Parasolid capability audit (kernel/25) then surfaced specific shipped operations that nurbs/05 treats thinly or not at all: the chamfer family (equal-offset, two-offset, offset-angle, apex/range chamfers), blend-to-hold-line, range-controlled blends, blend networks with mitring, and blend overflow handling.

This document does not re-derive the blending taxonomy. It focuses on the **engine mechanics**: the unified spine/section/trim machinery that serves both fillets and chamfers, the chamfer family in full, blend overflow and runout, hold-line construction, range and setback control, variable-radius depth, face-face blends across a gap, blend networks and mitring, recognition and removal (unblend), G2 cross-sections, and the documented failure modes of commercial filleting. The closing synthesis proposes a single engine architecture for Keel.

A framing fact, restated from nurbs/05 and now made concrete: the rolling-ball blend surface is a **canal surface** (envelope of a one-parameter sphere family). For a constant-radius blend on analytic faces it can sometimes be an exact analytic surface (a torus, cylinder, sphere, or cyclide); in general it is a procedural canal surface that the kernel caches as a NURBS approximation. The chamfer is the **ruled-surface degenerate** of the same machinery: instead of sweeping a circular arc between the two contact curves, you sweep a straight line. This single observation, that fillet and chamfer share spine and contact-curve computation and differ only in the cross-section primitive, is the spine of the whole engine and of this document.

---

## 1. The Unified Spine / Section / Trim-Curve Engine

Every edge-finishing operation in a Parasolid-class kernel reduces to the same three geometric objects, computed in the same order, regardless of whether the result is a fillet, a chamfer, a variable-radius blend, or a face-face blend:

1. **The spine (centre) curve.** The locus of the centre of the rolling ball (for fillets) or the locus that drives the cross-section (for chamfers). For a constant-radius rolling-ball fillet of radius R between faces A and B, the spine is the intersection of the two **offset surfaces** A+R*n_A and B+R*n_B. Where that intersection exists and is regular, the ball of radius R touches both faces simultaneously and the centre lies on the spine. This is the Choi-Ju / Chuang-Hwang construction.

2. **The two contact (spring) curves.** Parasolid calls them **spring curves**: the loci where the rolling ball touches face A and face B respectively. The contact point on A is the spine point minus R times the unit normal of the offset surface A+R*n_A (equivalently, the foot of the perpendicular from the ball centre to face A). ACIS calls the same loci the spring curves and stores them as the trimming boundary of the blend. The blend surface is the part of the canal/ruled surface lying **between** the two spring curves.

3. **The cross-section.** Sweep a profile from spring-curve-A point to spring-curve-B point in the plane normal to the spine. For a fillet it is a circular arc of radius R centred on the spine; for a G2 blend it is a conic or spline; for a chamfer it is a straight line. The blend surface is the one-parameter family of these sections.

The Parasolid documentation states this almost verbatim: *spring curves follow the point of contact between the pipe and the two faces; the portion of a pipe surface that lies between the spring curves is used to form the round blend face*. The "pipe surface" is the constant-radius canal surface; the spring curves trim it.

This is the **key architectural decision for Keel**: implement spine + spring-curve + section once, parameterised by (a) the section primitive (arc / conic / line / spline) and (b) the radius or offset law. Fillets and chamfers then become configurations of one engine, not two code paths. Sections 2 through 11 are all specialisations or hard-case extensions of this engine.

### 1.1 The rolling-ball contact equations restated for the engine

Let A(u,v) and B(s,t) be the supporting faces with unit normals n_A, n_B. For a rolling ball of radius r the centre c satisfies, simultaneously:

    | c - A(u,v) | = r,   (c - A) parallel to n_A   (contact with A)
    | c - B(s,t) | = r,   (c - B) parallel to n_B   (contact with B)

Eliminating gives c on the intersection of the two offset surfaces A + r*n_A and B + r*n_B. The two contact points are

    p_A = c - r*n_A,    p_B = c - r*n_B.

For **constant r** this is a clean offset-surface intersection (Section 4 of nurbs/05). For **variable r = r(λ)** along the edge, the offsets become variable-distance offsets and the spine is no longer a plain surface-surface intersection: it must be traced by **marching**, advancing λ and solving the contact equations at each step (Section 6). For a **chamfer**, you do not roll a ball at all: you intersect two offset surfaces (offset by the two chamfer distances) to find the spine, then project to get the two contact curves, then rule a line between them (Section 2).

---

## 2. The Chamfer Family

A chamfer is a blend whose cross-section is a **straight line segment** rather than a circular arc. Geometrically the chamfer surface for a straight edge between two planar faces is a **plane** (a flat cut); for a curved edge or non-planar faces it is a **ruled surface** swept by the line joining the two contact points as they travel along their respective contact curves. ACIS states this directly: *a chamfer blend refers to the ruled surface swept out by the line drawn between the two contact points of the rolling ball*.

The chamfer family is defined by **how the two contact curves are located**, which is what the user specifies:

### 2.1 Equal-offset (symmetric 45-degree) chamfer

One distance d. Both faces are offset inward by d; the two contact curves lie at distance d (measured along each face) from the edge. For two planar faces meeting at 90 degrees this produces the familiar symmetric 45-degree flat. This is the default and simplest chamfer. In the engine: offset face A by d, offset face B by d, intersect the offsets to get the spine, project back to get the two contact curves, rule between them.

### 2.2 Two-distance (asymmetric offset) chamfer

Two distances d1, d2. Face A's contact curve sits at d1, face B's at d2. ACIS describes the analogue: *an asymmetric chamfer results from two balls of different radii rolling together*. The chamfer face is still ruled, but the ruling line is no longer at 45 degrees. Engine change: offset A by d1 and B by d2 (different offsets), then proceed identically.

### 2.3 Offset-and-angle chamfer

One distance d (on the reference face A) plus an angle θ. The first contact curve is fixed at d on face A; the second contact curve is determined by driving the ruling line at angle θ from face A until it meets face B. This is convenient on castings where one face is the datum and the cut angle is specified by the drawing. Engine: place contact curve A at offset d, then for each section solve for where a line at angle θ to the local tangent plane of A intersects face B; that intersection traces contact curve B.

### 2.4 Parasolid's two chamfer categories: face-offset vs apex-range

Parasolid groups the above into two implementation categories:

- **Face-offset chamfers.** The chamfer is defined by offsets measured **on the supporting faces** (the equal-offset and two-offset variants). Parasolid's documented construction: *the extent of chamfer blends is determined by the offset surfaces being intersected, the resulting curves being projected onto the underlying surface, between which the chamfer blend is constructed*. That is exactly the spine-then-project-then-rule pipeline of Section 1, with offset distances instead of a radius.

- **Apex-range chamfers.** The chamfer is defined relative to the **apex** (the original sharp edge) and a **range** along each face. This is the natural parameterisation when the edge is a true geometric apex and the user thinks in terms of "cut back this far from the corner". The range_1, range_2 real values define where the chamfer surface meets each supporting surface.

### 2.5 Chamfer as a limit of a blend, or its own construction

Two defensible representations, and Keel should be explicit about the choice:

- **Chamfer as degenerate blend.** Treat the chamfer cross-section as the limiting "infinite-radius" or zero-bulge member of the conic cross-section family (a conic with rho -> the linear-chord limit). This unifies code: one section evaluator, a parameter that collapses the arc to its chord. Risk: numerical conditioning near the degenerate limit, and a circular arc evaluator does not naturally produce an exact straight ruling.
- **Chamfer as its own construction.** A first-class ruled surface between two contact curves. Cleaner geometry (the ruling is exactly straight, the surface is exactly ruled and often exactly planar or developable), simpler exactness claims, no conditioning issue. Costs a second section primitive.

Recommendation for Keel: **share spine + contact-curve computation, but make the section a polymorphic primitive** (arc / conic / spline / line). The chamfer gets a genuine straight-line section that produces an exact ruled (and, for straight edges between planes, exact planar) surface. This keeps exactness and conditioning clean while still reusing 80 percent of the engine.

### 2.6 The chamfer surface type by edge/face geometry

- **Straight edge, two planar faces:** the chamfer surface is an exact **plane**. Store analytically.
- **Curved edge (planar faces, or one curved face):** the chamfer surface is a **ruled surface** between the two offset trim curves. For special cases (circular edge between a plane and a cylinder) it can be an exact **cone** or **cylinder**. Otherwise it is a general ruled surface, exactly representable as a NURBS surface ruled between two NURBS trim curves (ruled surfaces are exact NURBS; see nurbs/05).
- **Edge between two non-planar (freeform) faces:** the chamfer is ruled between two general trim curves lying on freeform faces. The trim curves are not rational in general (they are offset-style curves on the faces), so the chamfer is a **NURBS approximation** ruled between approximated trim curves. Variable-distance chamfers (d varying along the edge) push the trim curves further from rational and force approximation.

---

## 3. Variable-Radius Fillets in Depth

A variable-radius fillet lets the radius be a function r(λ) of arclength or parameter λ along the edge. The Parasolid "vary" option specifies that the radius *varies linearly along the length of the edge*; richer systems allow radius set by control points, by a law curve, or by interpolation through user "variable radius points".

### 3.1 Why the surface is no longer a clean canal/offset-intersection

For constant r the spine is the plain intersection of two parallel-offset surfaces and the blend surface is a canal surface (envelope of equal spheres). For variable r the spheres have **different radii**, so the surface is a canal surface with a varying radius function. The spine is the intersection of two **variable-offset** surfaces A + r(λ)*n_A and B + r(λ)*n_B, which is not a fixed surface-surface intersection. Consequently:

- The spine must be **traced by marching**: advance λ, solve the two contact equations for the current r(λ), step. (Kosters, and Lukacs's differential-geometry treatment, formalise this; see the source entries.)
- The cross-section circle radius changes section to section, so the surface is not a constant-radius pipe and rarely has an exact analytic form. It is stored as a NURBS skin lofted/swept through the variable-radius circular sections.

### 3.2 Continuity of r and its effect on the surface

The smoothness of the fillet surface is bounded by the smoothness of r(λ):

- r piecewise-linear (C0 at the breakpoints) gives a fillet surface with **tangent creases** where the radius slope changes. Acceptable for engineering, ugly for styling.
- r C1 (e.g. cubic radius law) gives a tangent-continuous fillet along the spine direction.
- r C2 gives a curvature-continuous radius progression, needed when the fillet feeds into a G2 styling surface.

Practical rule for Keel: the radius law's continuity is a hard ceiling on the blend surface's cross-spine continuity; expose the law as a NURBS/Bezier scalar function so the user controls C1/C2 explicitly.

### 3.3 Tangency to neighbours and the constant-extension rule

When a variable-radius blend meets another blend at a vertex, naive linear variation can make the radius wrong at the junction and break tangency. Parasolid's documented mitigation: *the default behavior constrains range variation to ensure the variable radius blend meets other blends smoothly (tangentially) at vertices by causing the radius to have a constant extension*. In other words, near the vertex the radius is held constant ("constant extension") so the junction matches its neighbour, and the variation is squeezed into the interior. Keel needs the same: a junction-aware radius law that flattens to constant over a short extension at each end that abuts another blend.

---

## 4. Hold-Line (Hold-Curve) Blends

A hold-line blend constrains the **tangency curve** (one spring curve) to lie on a **user-specified curve drawn on a face**, rather than letting the rolling ball decide where it touches. The radius then becomes a **dependent, variable quantity**: at each section the radius is whatever value makes the blend tangent to the face exactly at the hold line.

### 4.1 Construction

Given hold curve H on face A and the opposing face B:

1. At each section parameter λ, take the hold point p_A = H(λ) on face A, with face-A tangent plane and unit normal n_A there.
2. The blend must be tangent to face A at p_A, so the cross-section circle is tangent to face A's tangent plane at p_A; its centre lies along n_A at distance r(λ) (the unknown).
3. Impose the second condition: the circle must also be tangent to face B. This determines r(λ) and the contact point p_B on B. Solve the one-unknown tangency equation per section.
4. Sweep the resulting variable-radius circular sections between the hold curve (spring curve A, fixed = H) and the computed spring curve B.

So a hold-line blend is a **variable-radius blend whose radius is solved, not specified**, with one spring curve pinned to the hold curve. The SolidEdge documentation phrases the behaviour exactly: *the blend surface rolls along the tangent hold line at every point, varying the radius as necessary to maintain tangency between the faces*.

### 4.2 Why it is wanted

The hold line guarantees the blend tangency lands on a designer-chosen feature line (a styling crease, a parting line, the boundary of a finished region) regardless of local curvature. It decouples "where the blend ends on this face" from "how big the radius is". It also enables **face blends with a hold line** where the tangency on one or both faces is pinned, used heavily in automotive Class-A work to lock blend boundaries to character lines.

### 4.3 Engine relevance

In the unified engine, a hold-line blend is the same spine/section/trim pipeline with the spine/radius solve **inverted**: instead of (radius given -> solve contact), it is (one contact curve given = hold line -> solve radius and the other contact). Keel should expose the section solver in both directions so hold-line and ordinary radius blends share code.

---

## 5. Range and Setback Control

### 5.1 Range along the edge

A blend need not span the whole edge. **Range** limits the blend to a parameter interval [λ0, λ1] of the edge. Parasolid's range_1, range_2 reals also serve a second meaning for chamfer and variable blends: *they define where the blend surface meets the supporting surfaces*, i.e. how far up each face the blend reaches. So "range" covers both along-edge extent and across-face extent depending on context.

### 5.2 Setback (smooth runout) at blend ends

A blend that simply stops mid-edge leaves an abrupt step. **Setback** ramps the blend down over a short distance so it runs out smoothly into the unblended edge: the radius (or offset) tapers to zero, and the spring curves converge back to the original edge, over the setback length. This is the same mechanism as the constant-extension rule (Section 3.3) but used at a free end rather than a junction.

### 5.3 Retractable edge blends for vertex blends

Setback vertex blending (covered in depth in nurbs/05 and revisited in the sources here) **requires** that the edge blends meeting at a vertex be **retractable**: each edge blend must be able to pull its end back from the vertex by the setback distance, exposing a clean region for the vertex patch to fill. If an edge blend cannot retract (its end is pinned by an adjacent feature), the setback vertex construction fails. Keel's edge-blend representation must therefore support a parameterised end-retraction as a first-class operation, not a post-hoc trim, so that vertex blends can request "give me your last s millimetres back".

---

## 6. Blend Overflow and Runout

Overflow is what happens when a blend, as it grows, **runs off the end of a supporting face or onto an adjacent face**. It is, by universal report, one of the two or three dominant sources of commercial filleting failure. The rolling ball reaches the edge of the face it was rolling on, and the engine must decide what to do.

### 6.1 The documented overflow behaviours (Parasolid / ACIS)

Both kernels expose a small menu of overflow policies, with closely matching names:

- **Smooth overflow** (Parasolid ov_smooth; ACIS Smooth). The blend **rolls onto the adjacent face** and continues, smoothly tangent across the boundary it overflowed. The spring curve crosses onto the neighbour. Used when the neighbouring face is a legitimate continuation surface.
- **Cliff overflow** (Parasolid ov_cliff, ov_cliff_end; ACIS Cliff). The blend is **trimmed against the cliff edge**: where the blend would overflow, it is cut off by the sharp boundary ("cliff") of the face, producing a **cliff blend** (sometimes called a face-edge blend), which has a support face on one side and an **edge curve** (the cliff edge) on the other. Parasolid's documentation: *when a portion of a blend chain rolls along an edge it results in intermediate face-edge blends known as cliff blends; the edges that support the blends are termed cliff edges*. The ov_cliff_end variant controls cliff handling specifically at the end of the blend.
- **Notch overflow** (Parasolid ov_notch; ACIS Notch). For **notch/recess** situations where two blends or a blend and a wall collide in a narrow pocket, the blend is handled as a notch: the overflowing blend meets its opposing geometry inside the recess and is trimmed against it, leaving a notch blend rather than a clean roll-on. Notch re-blend is a documented follow-up operation that re-blends the leftover notch.
- **Auto.** The kernel picks among the above per situation. Convenient, but the source of "why did my fillet do that" surprises; production code usually wants explicit control on hard edges.

### 6.2 Why overflow is hard

- The blend's topology changes mid-edge (one face becomes two, or a spring curve jumps surfaces), so the result is not a single canal-surface patch but a **chain** of patches with different supports.
- Choosing smooth vs cliff is a **modelling-intent** decision the geometry alone cannot make; the kernel must take it as a parameter and degrade gracefully when the chosen policy is infeasible.
- Overflow interacts with range and setback: an overflowing blend near a vertex must coordinate its overflow with the vertex blend's retraction.

### 6.3 Kernel relevance

Keel must model the blend result as a **chain of faces** with per-segment support data ("unders": the faces and edges each segment leans on), exactly as the blend-removal literature describes the inverse. The overflow policy is a parameter on the blend feature; cliff and notch produce extra topology that must be recorded so the feature can be edited, suppressed, or unblended later.

---

## 7. Face-Face Blends Without a Shared Edge

An edge blend rounds an existing edge. A **face-face blend** blends between two face sets that **need not share an edge, need not be adjacent, and need not even be in the same body**. Parasolid: *the sets of faces to be blended do not need to be adjacent, or even in the same body; face-face blends can be used to join together disjoint sheet bodies*. The blend bridges the gap.

### 7.1 Construction and the range surface

Without a shared edge there is no apex to roll around, so the engine rolls the ball **between the two face sets directly**: the spine is the locus of ball centres equidistant (at radius r, or at the two offsets for a chamfer) from both face sets, found by intersecting the offset surfaces of the two sets. The blend is trimmed by the two spring curves where the ball touches each set. Because the bridged region may not be bounded by a natural edge, face-face blends use **range surfaces** (or range curves): auxiliary bounding surfaces that delimit where the blend starts and stops across the gap. The range_1/range_2 parameters set where the blend meets each supporting surface.

### 7.2 Fixed-only and gap semantics

Parasolid notes a practical constraint: *face-face blends are created and fixed in a single operation; there is no such thing as an unfixed face-face blend*. Unlike edge blends (which can exist provisionally as "unfixed" features to be attached later), a face-face blend is computed and committed at once because its supports are less constrained. For Keel this means face-face blends are an **eager** operation in the feature model, whereas edge blends can be **lazy/deferred**.

### 7.3 Kernel relevance

Face-face blending is the general setup of which edge blending is the special case where the two face sets happen to share an edge. Building the engine face-face-first (with edge blending as the shared-edge specialisation) is cleaner than the reverse. It also subsumes gap-bridging, sheet-joining, and many "blend between these two bosses" pocket cases.

---

## 8. Blend Networks and Mitring

When several blends meet, the engine must decide **order** and **how the patches join**.

### 8.1 The precedence/sequencing problem

At a vertex where three or more blended edges meet, the blends interact: each blend's overflow and end-retraction depend on its neighbours. The kernel must choose a **sequence**: which blend is applied first, which leans on which. Larger-radius blends typically dominate (a big blend overruns a small one), so a common heuristic is **largest-radius-first** at a shared vertex, with the smaller blends trimmed against the larger. ACIS organises this with a **blend graph** that records the blends and their interactions; the sequence is derived from the graph. Getting the order wrong produces self-intersections or gaps at the corner.

### 8.2 Mitring

**Mitring** is the corner treatment where two adjacent blends (typically two chamfers, or two equal-radius fillets) meet and are **trimmed against each other along a miter line**, like a picture-frame corner, rather than one rolling over the other or a separate vertex patch being inserted. The miter is the bisector intersection of the two blend surfaces. Mitring is the cheap, clean corner when the two blends are compatible (same or similar cross-section); it fails when the blends differ enough that their surfaces do not intersect in a single clean curve, at which point a **vertex blend** (setback patch, nurbs/05) is needed instead.

### 8.3 The decision ladder at a vertex

In order of increasing cost and generality:

1. **Mitre** the two/three blends against each other (works for compatible, similar blends).
2. **Roll-over**: the dominant (larger) blend continues and the others run out into it via setback.
3. **Setback vertex blend**: retract all edge blends and fill the exposed n-sided region with a dedicated 2n-sided vertex patch (the Varady setback construction).

Keel should implement this ladder, choosing the cheapest treatment that yields a valid, tangent (or G2, if requested) corner.

---

## 9. Blend Recognition and Removal (Unblend / Defeaturing)

The inverse problem: given "dumb" B-rep geometry (e.g. imported STEP), **find** the fillet/chamfer faces and **remove** them, re-extending and re-intersecting the supports to recover the sharp model. This is essential for defeaturing before meshing/CAE and for feature-based reverse engineering.

### 9.1 Recognising a blend face

The recognition tests, drawn from the reverse-engineering literature:

- **Constant-radius cross-section test.** A constant-radius rolling-ball blend face has a **circular cross-section of constant radius** in every plane normal to its spine. Sample sections, fit circles, check radius constancy and that section planes are normal to a common spine.
- **Rolling-ball verification.** Confirm the candidate face is tangent to its two neighbours along its two spring edges and that a ball of the fitted radius rolls in contact with both: this distinguishes a genuine fillet from a coincidentally curved face.
- **Edge classification.** Classify the blend face's bounding edges into **spring edges** (tangent boundaries to the supports), **cross edges** (the section-end boundaries), and other edges. This is the data the removal step needs.
- **Chamfer recognition.** A chamfer face is **ruled / planar with linear cross-section**, tangent or angled to its neighbours along two spring edges; recognise by the straight section and the ruled-surface fit.

Modern work adds learning-based recognition (FilletRec, 2025) using pose-invariant intrinsic features (curvature) so that recognition generalises across orientations and complex blend chains; useful as a heuristic front-end but Keel should keep the exact geometric verifier as ground truth.

### 9.2 Removing a blend (the unblend algorithm)

The canonical removal pipeline (US7236168 and the CAD-journal suppression work):

1. Run blend recognition to get, for each blend face in the chain, its **support faces, spring edges, and cross edges**.
2. **Extend and intersect the two support faces** to recover the original sharp edge curve the blend replaced.
3. **Compute the end vertices** by extending and intersecting the recovered edge curve with the neighbouring surfaces.
4. Handle **interacting blend chains**: determine which other blends touch this chain, separate them, and order the removals so dependent blends come off in the right sequence.
5. Delete the blend faces and stitch the re-extended supports, predicting the final topology from the recorded blend structure **with minimal heuristics**, which keeps the number of (expensive) surface-surface and curve-surface intersections low.

### 9.3 Suppression for defeaturing

Suppression is removal-for-analysis: identify blend chains via recognition, then delete the blend faces and heal, producing a simplified model for meshing while keeping the original for manufacturing. The blend's **underlying data (unders)** recorded at creation makes suppression cheap when the blend is a live feature; recognition is the fallback when it is not.

### 9.4 Kernel relevance

Keel should record blend **unders** at creation (supports, spring edges, cross edges, radius/offset law, overflow policy) so that removal/suppression is a recorded-feature operation, not a re-recognition problem, on native geometry. Recognition is reserved for imported dumb geometry, where the exact constant-radius/ruled tests plus rolling-ball verification recover the structure.

---

## 10. G2 (Curvature-Continuous) Blends

A G1 (tangent) fillet has a curvature jump at its spring curves: the curvature steps from the face's value to the arc's 1/r. Under reflective lighting (Class-A surfacing) this jump shows as a visible line. **G2 blends** remove it by using a cross-section that **matches the curvature** of each face at its spring curve and varies smoothly across the blend.

### 10.1 Conic cross-sections and the rho parameter

The workhorse G2-capable section is the **conic**, controlled by a shape parameter **rho** in (0,1): rho = 0.5 is a parabola, rho < 0.5 is elliptical (flatter), rho > 0.5 is hyperbolic (more pointed). A "conic rho" fillet sweeps a conic of chosen rho instead of a circular arc, giving an asymmetric, shape-controllable profile. With endpoint tangents matched to the faces and rho chosen to match curvature, the conic gives improved continuity over a circle; for full G2 the section is generally a curvature-matched **spline** whose end curvatures equal the adjacent faces' and which blends to the section apex curvature in between.

### 10.2 The surface construction

The G2 blend surface is the sweep of curvature-continuous sections along the spine. It is not a canal surface (the section is not a circle) and not analytic; it is a **NURBS skin** through the sections, constructed so that along the spring curves the surface's cross-curvature equals the supporting faces' surface curvature. The chamfer is the opposite extreme of the same section family (the linear/degenerate conic); the conic family thus spans chamfer (linear) -> circular fillet (rho for a circle) -> styled G2 blend (curvature-matched spline) as one parameterised section primitive, which is exactly the polymorphic-section design recommended in Section 2.5.

### 10.3 Kernel relevance

Keel's section primitive should support: line (chamfer), circular arc (standard fillet), conic-by-rho (shaped fillet), and curvature-matched spline (G2 blend). All four ride the same spine/spring engine; only the section evaluator and its end-condition solver differ. Exposing rho and a "continuity = G1/G2" flag covers the engineering-to-styling range with one code path.

---

## 11. Robustness and Failure Modes

The literature and vendor docs converge on a short list of dominant failure cases in commercial filleting, with their mitigations.

- **Vertex blends.** Where three-plus blended edges meet, especially with unequal radii and convex/concave mixing. Mitigations: the mitre/roll-over/setback decision ladder (Section 8.3); retractable edge blends (Section 5.3); fall back to a setback n-sided patch when mitring fails.
- **Overflow / runout.** Blend runs off its support or onto a neighbour (Section 6). Mitigations: explicit overflow policy (smooth/cliff/notch), per-segment support tracking, blend-as-face-chain representation.
- **Small radius vs tolerance.** When the fillet radius approaches the modelling tolerance, spring curves and the spine become numerically indistinguishable from the edge and from each other; offset-surface intersections become unreliable. Mitigation: tolerance-aware guards, refuse or warn below a radius/tolerance ratio, use higher-precision spine tracing near the limit.
- **Self-intersection of the fillet surface.** When the radius exceeds the local concave radius of curvature of a support, the canal surface folds (the rolling ball no longer fits); the blend surface self-intersects. Mitigation: curvature check r < min concave radius of curvature along the edge; clip or fail where violated; this is the same medial-axis/offset-self-intersection problem as in nurbs/05 Section 9.
- **Tangent-face pileups.** Several nearly-tangent faces around the edge make the support ambiguous and the offset intersections ill-conditioned. Mitigation: robust support selection, possibly user disambiguation.
- **Spine-tracing failure on variable radius.** The marching solver for variable-r spines can diverge or skip where the radius changes fast relative to curvature. Mitigation: adaptive step, curvature-aware stepping, continuity limits on r(λ).

The unifying robustness principle: the blend is **only valid where the rolling ball physically fits and the offset surfaces intersect cleanly**; the engine must detect the boundaries of that validity (curvature limit, tolerance limit, overflow boundary) and either trim, switch policy, or fail with a precise reason, rather than emitting a self-intersecting or near-degenerate surface.

---

## 12. Manufacturing Semantics: Why Both Fillet and Chamfer Exist

Briefly, since the engine serves a manufacturing purpose:

- **Fillets** (rounds) replace sharp internal corners with a radius. They **spread stress over an arc**, reducing the stress-concentration factor (roughly 40 to 60 percent versus a sharp corner; the concentration factor drops from ~2.5-3.0 toward ~1.5-2.0). They are **required for casting/molding** internal corners, where sharp corners impede material flow, wear the tooling, and cause defects. Cost: radius-specific tooling (ball-end mills, corner-rounding cutters), so more machining time.
- **Chamfers** are angled flats on **external** edges. They **deburr, ease assembly (lead-ins), and protect edges**, and are **cheaper to machine** (a single chamfer tool makes many sizes) but **concentrate stress along a line**, so they are inappropriate for highly loaded internal corners.

Design practice uses both deliberately: fillets at load-bearing internal corners, shaft shoulders, and cast internal corners; chamfers at external edges, hole entries, and assembly interfaces. This is why a complete kernel must ship **both** as first-class operations, not treat the chamfer as a mere fillet special case at the UI level even if they share an engine internally.

---

## Per-Source Entries

### S1. Vida, Martin, Varady (1994): the blending-methods survey, re-read for engine mechanics

**Citation.** Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365.

**Construction.** The canonical taxonomy and terminology source. Re-read here not for the taxonomy (covered in nurbs/05) but for the engine vocabulary it standardises: the **spine/centre curve**, the **linkage (contact/spring) curves** where the blend meets the supports, the **range/trimlines** delimiting the blend, the distinction between **supporting** and **surrounding** faces, and the classification of blends into rolling-ball, trimline-based, and functional (potential-method) families. It frames the rolling-ball blend as a canal surface trimmed by its two linkage curves and lays out why constructing a blend is fundamentally "find spine, find contact curves, sweep section, trim". It also catalogues the hard cases (vertex blends, n-direction blends, range control) that the rest of the engine literature attacks.

**Limitations.** A 1994 survey: parametric methods only (no later subdivision/T-spline blends), and it predates the now-standard vendor overflow taxonomy. Descriptive, not a recipe.

**Kernel relevance.** The naming and decomposition Keel should adopt internally (spine, spring curves, range, supports/surrounds), so its API and data model match thirty years of literature and vendor docs.

### S2. Choi & Ju (1989): constant-radius blending in surface modelling

**Citation.** Choi, B. K., & Ju, S. Y. (1989). Constant-radius blending in surface modelling. *Computer-Aided Design, 21*(4), 213-220.

**Construction.** A foundational rolling-ball construction on general parametric surfaces. Any pair of rectangular parametric patches can be blended provided their offset surfaces (offset by the blend radius R) are smooth. The edge blend is built by **sweeping rational quadratic (conic) cross-section curves** between the contact points: the spine is the offset-surface intersection, the contact curves are the feet on each face, and each section is a rational quadratic arc, giving an exact circular section in rational form. Corner blends where three surfaces meet are filled by a **convex combination of linear Taylor interpolants** (an early vertex-blend scheme).

**Limitations.** Constant radius only; requires the radius-R offsets to be smooth (fails near tight curvature, i.e. the self-intersection limit); the conic-section sweep is G1, not G2; corner scheme is simple.

**Kernel relevance.** Establishes the rational-quadratic section sweep that Keel can use for the exact-circular-arc section, and the offset-intersection spine that is the engine's core. The corner convex-combination is the ancestor of the setback vertex patch.

### S3. Chuang, Hwang (and related): fillet/intersection by rolling balls

**Citation.** Chuang, J.-H., & Hwang, W.-C. (1997). Variable-radius blending of parametric surfaces. *The Visual Computer, 13*(7), 316-329. (And the related rolling-ball fillet/intersection line of work, e.g. *Fillet and surface intersections defined by rolling balls*, CAD/CAGD, early 1990s.)

**Construction.** Treats both surface intersection and filleting as **rolling-ball** problems. The blend is the region swept by a ball maintaining two tangential contacts; the centre traces a spine, the contacts trace linkage curves. The variable-radius extension makes the ball radius a function along the edge and computes the spine by intersecting **variable-offset** surfaces, traced by marching, with circular sections of changing radius lofted between the moving contact curves.

**Limitations.** Marching solvers need good seeding and adaptive stepping; variable radius breaks the exact-canal form and forces NURBS approximation; fast radius variation relative to curvature can destabilise the trace.

**Kernel relevance.** Directly the variable-radius spine-marching engine (Section 3, Section 6). The unification of intersection and filleting under one rolling-ball model is a useful Keel internal abstraction.

### S4. US5774359 (Autodesk): extended rolling-ball fillet between two surfaces

**Citation.** Mokris, J. (1998). *Method for generating an extended rolling ball fillet surface between two surfaces* (US Patent 5,774,359). Autodesk, Inc.

**Construction.** A complete, implementable rolling-ball fillet algorithm. (1) Offset base surface A by R along its normal: A_o = A + R*a_n. (2) Build an **envelope (duct) surface** E by sweeping a radius-R sphere along a boundary curve C(w) of surface B. (3) The **spine** is the intersection A_o = E, sampled at K points by chordal-deviation tolerance. (4) Project to the two **contact curves**: C1 = A_o - R*a_n on A, C2 = E - R*E_n on B. (5) At each spine point build a **circular arc** from the C1 point to the C2 point centred on the spine; fit each arc as a B-spline segment. (6) Build the final fillet as a NURBS surface F(p,q) = sum Y_ij N_i(p) N_j(q) interpolating the K arcs, with centripetal parameterisation across sections, maintaining **G1** to surface A. (7) **Extension/overflow**: by using the envelope of the boundary curve rather than the curve itself, and reversing the roles of A and B, the fillet is **extended to the boundaries of both surfaces** even where a naive intersection would stop short.

**Limitations.** Patent-scoped; constant radius in the core description; G1 only; the envelope-of-boundary trick is one specific overflow handling, not the full smooth/cliff/notch menu.

**Kernel relevance.** The most explicit end-to-end recipe available: offset -> spine -> contact curves -> arc sections -> NURBS skin, plus a concrete extension/overflow mechanism. A good template for Keel's constant-radius edge-blend path and its NURBS-cache construction.

### S5. US5345546 (Hewlett-Packard): fillet surface between two surfaces

**Citation.** *Method and apparatus for generating fillet surface between two surfaces* (US Patent 5,345,546).

**Construction.** Another rolling-ball fillet patent: offset both surfaces by the fillet radius, intersect to get the centre/spine, find the tangency contact points on each surface, and generate the fillet as the surface swept by the arc through the two contact points centred on the spine. Adds practical handling for sampling and surface fitting.

**Limitations.** Constant radius; patent-scoped; overlaps S4.

**Kernel relevance.** Corroborates the offset-intersection spine construction as the industry-standard core; useful as a second independent description of the same engine.

### S6. Lukacs (1998): differential geometry of G1 variable-radius rolling-ball blends

**Citation.** Lukacs, G. (1998). Differential geometry of G1 variable radius rolling ball blend surfaces. *Computer Aided Geometric Design, 15*(6), 585-613.

**Construction.** The rigorous differential-geometric account of the variable-radius rolling-ball blend. Defines the spine and the variable radius function r, derives the conditions under which the swept circular sections form a **G1** surface tangent to both supports, and relates the surface to the **canal surface** of a varying-radius sphere family. Gives the contact-curve (spring-curve) equations and the constraints on r(λ) for the surface to be regular (no self-intersection): r must respect the local curvature bound.

**Limitations.** G1 focus (not G2); theoretical, assumes well-behaved analytic supports; the regularity conditions are necessary criteria, not a robust numerical procedure.

**Kernel relevance.** The exact conditions Keel needs for **validity checking** of variable-radius blends: when does the swept surface stay regular, where does it self-intersect, what continuity does a given r(λ) yield. Ties the self-intersection failure mode (Section 11) to a checkable curvature inequality.

### S7. Variable-radius blending of parametric surfaces (Visual Computer)

**Citation.** Chuang, J.-H., & Hwang, W.-C. (1997). Variable-radius blending of parametric surfaces. *The Visual Computer, 13*(7), 316-329. (See S3; listed separately for its explicit variable-radius marching detail.)

**Construction.** Gives the explicit marching procedure: from a pair of base surfaces in parametric form, a reference curve, and a radius function r of the rolling sphere, compute an exact representation of the variable-radius **spine** and march along it, at each step solving the two-contact equations for the current radius and recording the moving contact curves; then loft variable-radius circular sections into the blend surface.

**Limitations.** Marching robustness; approximation of the resulting non-canal surface; reference-curve dependence.

**Kernel relevance.** The concrete recipe for Section 6's variable-radius spine marcher; pairs with Lukacs (S6) for the validity theory.

### S8. Varady et al. (1996): geometric construction for setback vertex blending

**Citation.** Varady, T., Hermann, T., et al. (1996). Geometric construction for setback vertex blending. *Computer-Aided Design, 28*(6-7), 587-597.

**Construction.** The definitive setback vertex-blend construction. Edge blends meeting at a vertex are **retracted (set back)** by chosen distances, exposing an n-sided region. The vertex is filled by a **2n-sided patch** built from a **setback split**: standard polynomial patches combined according to the split, with free interior parameters. A repeated-chamfering strategy builds the **control frame**; the patch interpolates the retracted edge-blend boundaries with tangency. Handles convex/concave mixes and tangential/cuspate corners.

**Limitations.** Construction complexity; many free parameters to set; tangential and cuspate cases are special-cased; G1 (the basic scheme).

**Kernel relevance.** The third rung of Keel's vertex decision ladder (Section 8.3) and the reason edge blends must be **retractable** (Section 5.3). Defines the 2n-sided patch Keel needs for hard corners that mitring cannot handle.

### S9. Varady et al. (2009): setback vertex blends in digital shape reconstruction

**Citation.** Varady, T., et al. (2009). Setback vertex blends in digital shape reconstruction. In *Advances in Geometric Modeling and Processing* (LNCS, pp. 285-300). Springer.

**Construction.** Revisits setback vertex blends for **reverse engineering**: recognising and reconstructing setback vertex regions from scanned/imported data, fitting the retracted edge blends and the central 2n-sided patch to measured geometry.

**Limitations.** Reconstruction-oriented; depends on prior edge-blend recognition; fitting tolerances.

**Kernel relevance.** Bridges vertex blends to Keel's recognition/removal path (Section 9): not just creating vertex blends but identifying them in dumb geometry for defeaturing.

### S10. Parasolid Edge Blending documentation (Overview + Functions/Options)

**Citation.** Siemens Digital Industries Software. *Parasolid Functional Description: Edge Blending Overview* and *Edge Blending Functions and Options* (v12.0, chapters 30-31; v35 Edge Blending Overview). Retrieved from q-solid.com Parasolid documentation mirror.

**Construction.** The shipped-kernel reference. Rolling-ball blends have a **circular cross-section** from rolling a sphere along the two adjoining faces; **spring curves** follow the contact points; the portion of the **pipe (canal) surface** between the spring curves forms the blend face. **Variable** rolling-ball blends vary the circular-section radius along the edge (vary option, linear by default). **Chamfers** have a **linear cross-section** in two flavours: **face-offset chamfers** (extent from intersecting offset surfaces and projecting the result onto the underlying surface, blend constructed between the projected curves) and **apex-range chamfers** (relative to the apex, controlled by range_1/range_2). **Overflow** is controlled by ov_smooth, ov_cliff, ov_cliff_end, ov_notch (smooth roll-on, cliff trim producing cliff/face-edge blends on cliff edges, notch handling in recesses). Variable blends use a **constant extension** near vertices to stay tangent to neighbours. Set-blend functions PK_EDGE_set_blend_chamfer and PK_EDGE_set_blend_variable attach blend attributes before fixing.

**Limitations.** Vendor docs, not algorithms; option semantics described, not the math; mirror is an older version snapshot.

**Kernel relevance.** The **canonical option set and terminology Keel should match**: spring curves, pipe surface, face-offset vs apex-range chamfer, the four overflow policies, constant extension, the set-then-fix lifecycle. This is the de facto spec for a Parasolid-class blending API.

### S11. Parasolid Face-Face Blending documentation

**Citation.** Siemens Digital Industries Software. *Parasolid Functional Description: Face-Face Blending* (v12.0, chapter 34). Retrieved from q-solid.com.

**Construction.** Face-face blends bridge two face sets that **need not be adjacent or in the same body** (can join disjoint sheet bodies). Uses **range surfaces/curves** to delimit the blend across the gap; range_1/range_2 define where the blend meets each support. Created and **fixed in a single operation** (no unfixed face-face blend).

**Limitations.** Eager-only; requires range geometry to bound the bridge; vendor description.

**Kernel relevance.** Section 7. Face-face is the general engine of which edge blending is the shared-edge special case; the eager-fix semantics inform Keel's feature-model design.

### S12. ACIS Blending Component documentation (BLND)

**Citation.** Spatial Corp. (Dassault Systemes). *ACIS Blending (BLND) Component Reference* (R10/R17). Retrieved from the Arizona ISL ACIS documentation mirror and q-solid ACIS R17 docs.

**Construction.** The other major shipped kernel. Constant-radius blends are stored as a **rolling-ball spline surface (rb_blend_spl_sur)**, the envelope of the rolling ball, simplified to an analytic surface when possible. **Spring curves** are the trim boundaries. **Chamfer** blend = the **ruled surface swept by the line between the two contact points**; **asymmetric** chamfer from two balls of different radii. **Variable-radius** blends supported. **Entity-entity** blends (face-face). **Vertex** blends. **Hold-line** blends configurable via blend options. **Overflow**: Auto, Cliff, Smooth, Notch. Blends organised via a **blend graph** that drives sequencing.

**Limitations.** Vendor docs; rb_blend_spl_sur internals proprietary; older mirror.

**Kernel relevance.** Independent corroboration of the entire engine: spline-surface canal storage with analytic simplification (the procedural-with-NURBS-cache idea), ruled-line chamfer, the same overflow menu, and the blend-graph sequencing model (Section 8). Confirms the design is industry-convergent, not idiosyncratic.

### S13. US7236168 (UGS/Siemens): method for removing blends in B-rep models

**Citation.** *Method for removing blends in B-rep models* (US Patent 7,236,168 B2).

**Construction.** The unblend algorithm. Blend recognition classifies each blend face's edges into **spring edges, cross edges, and other edges** and identifies the **support faces**. Removal: **extend and intersect the two support faces** to recover the sharp edge curve; **extend and intersect** that curve with neighbouring surfaces to recover the end vertices; determine and **separate interacting blend chains**, then delete blend faces in dependency order. Final topology is predicted from the recorded blend structure with **minimal heuristics**, minimising costly curve-surface and surface-surface intersections.

**Limitations.** Patent-scoped; relies on good recognition; complex interaction cases between chains are the hard part.

**Kernel relevance.** Section 9's removal recipe and the **unders** data model (supports + spring/cross edges) Keel should record at creation so removal/suppression is recorded-feature-driven, not recognition-driven, on native geometry.

### S14. Recovering constant-radius rolling-ball blends in reverse engineering

**Citation.** Lukacs, G., Marshall, A. D., & Martin, R. R. (1998). Methods to recover constant radius rolling ball blends in reverse engineering. *Computer Aided Geometric Design, 15*(?), 663-? (Special issue.) [ScienceDirect S0167839699000436.]

**Construction.** Recognises constant-radius blends in scanned/dumb geometry: a blend face must **match the shape of a rolling ball**, i.e. circular cross-section of constant radius, tangent to both neighbours; fit the radius and verify rolling-ball consistency. Recovers the spine and spring curves from the data.

**Limitations.** Constant radius only; sensitive to scan noise and tolerance; chamfers and variable blends need separate tests.

**Kernel relevance.** The **recognition tests** of Section 9.1 (constant-radius cross-section + rolling-ball verification) for Keel's import/defeature path.

### S15. Algorithms for reverse engineering boundary representation models

**Citation.** Benko, P., Martin, R. R., & Varady, T. (2001). Algorithms for reverse engineering boundary representation models. *Computer-Aided Design, 33*(11), 839-851.

**Construction.** Broad reverse-engineering pipeline including blend/fillet detection and reconstruction within full B-rep recovery from point data, integrating segmentation, surface fitting, and blend recognition.

**Limitations.** End-to-end RE focus; blend detection is one stage; depends on upstream segmentation quality.

**Kernel relevance.** Situates blend recognition (Section 9) inside the larger import-healing pipeline Keel must interoperate with (kernel/13).

### S16. CAD-journal: recognizing and suppressing blend features

**Citation.** (2004). An efficient algorithm for recognizing and suppressing blend features. *Computer-Aided Design and Applications, 1*(1-4), 421-428.

**Construction.** Practical recognition + suppression for defeaturing: identify **blend chains** via a recognition algorithm, then **delete each blend face** in the chain and heal, producing a simplified model for downstream feature recognition/CAE. Emphasises chain-level handling rather than face-by-face.

**Limitations.** Heuristic chain identification; healing after deletion is where robustness is tested.

**Kernel relevance.** Section 9.3 suppression-for-defeaturing; the chain-level view matches Keel's blend-as-face-chain representation.

### S17. FilletRec (2025): learning-based fillet recognition

**Citation.** Gao, J., Liu, T., Ye, H., & Chen, J. (2025). FilletRec: A Lightweight Graph Neural Network with Intrinsic Features for Automated Fillet Recognition. arXiv:2511.05561.

**Construction.** A GNN over the B-rep face-adjacency graph using **pose-invariant intrinsic features (curvature)** so recognition generalises across orientation and complex chains; recovers fillet faces for simplification. Uses 0.2-5.4 percent of baseline parameters with higher accuracy, plus a benchmark dataset.

**Limitations.** Learning-based (no exactness guarantee); needs training data; recognition only (simplification handed off).

**Kernel relevance.** A heuristic **front-end** for Keel's recognition (Section 9) on messy imports, with the exact constant-radius/rolling-ball verifier (S14) as the ground-truth backstop.

### S18. Whited & Rossignac: relative blending / rounding

**Citation.** Whited, B., & Rossignac, J. (2009). Relative blending. *Computer-Aided Design, 41*(6), 456-462.

**Construction.** Generalises rolling-ball/canal blends by defining the blend via a **relative** (ball-radius-as-fraction, or distance-field) formulation rather than a fixed metric ball, giving more control over how the blend distributes between the two faces and smoother behaviour where curvature varies. Reframes the contact and spine in relative terms.

**Limitations.** A generalisation, heavier than a plain rolling ball; less directly tied to the analytic canal exactness.

**Kernel relevance.** An alternative section/spine parameterisation worth knowing when the equal-ball rolling model gives poor blends on strongly unequal-curvature supports; informs Keel's choice of section-placement law.

### S19. Conic / G2 fillet cross-section practice (vendor + community references)

**Citation.** Onshape/Autodesk Alias/SolidWorks fillet documentation and engineering references on **conic-rho** and **curvature-continuous (G2)** fillets. (Onshape *Fillet*; Autodesk *Alias Multi-Surface Fillet*; engineering articles.)

**Construction.** Documents the conic-rho section (rho < 0.5 elliptical, 0.5 parabolic, 0.5-1.0 hyperbolic) and curvature-continuous spline sections whose end curvature matches the adjacent faces and blends to the apex curvature, removing the G1 curvature jump. Asymmetric conic profiles via two distances plus rho.

**Limitations.** Application docs, not derivations; G2 section construction details are vendor-specific.

**Kernel relevance.** Section 10's polymorphic section primitive: line / arc / conic-by-rho / curvature-matched spline as one parameterised family spanning chamfer to G2 styling blend.

### S20. Hold-line (tangent hold line) blend documentation

**Citation.** Siemens *Solid Edge: Tangent Hold Lines* documentation; Onshape *Face Blend* (hold-line options). Retrieved from vendor help.

**Construction.** A tangent hold line pins the blend's tangency to a user curve on a face: *the blend surface rolls along the tangent hold line at every point, varying the radius as necessary to maintain tangency between the faces*. The radius is the dependent unknown solved per section to keep tangency at the hold curve; the opposite spring curve and radius are computed.

**Limitations.** Vendor description; the per-section solve and degeneracy handling (where no radius satisfies tangency) are unstated.

**Kernel relevance.** Section 4's hold-line construction: the **inverted** section solve (contact given, radius solved) that Keel's section solver must support bidirectionally.

### S21. Fillet-vs-chamfer manufacturing references

**Citation.** Xometry, Wevolver, and CNC-manufacturing technical guides (2023-2025). *Fillet vs Chamfer* engineering guides.

**Construction.** Engineering rationale: fillets spread stress over an arc (concentration factor down ~40-60 percent vs sharp corner; ~2.5-3.0 to ~1.5-2.0), required for casting internal corners; chamfers concentrate stress on a line but deburr, ease assembly, and machine cheaper (one tool, many sizes). Both used deliberately: fillets internal/load-bearing/cast corners, chamfers external edges/hole entries/lead-ins.

**Limitations.** Application-level, not geometric; rules of thumb.

**Kernel relevance.** Section 12: why Keel ships both as first-class operations even though they share an engine.

---

## Fillet/Chamfer Engine Architecture for Keel

**One engine, polymorphic section.** Implement a single edge-finishing engine with three reusable stages and a swappable section primitive:

1. **Spine stage.** Given supports A, B and a radius/offset law, compute the spine: offset-surface intersection for constant radius (exact when supports are analytic); variable-offset marching for variable radius and hold-line (radius solved per section). Face-face blends use the same spine between two face sets with range surfaces bounding the bridge; edge blends are the shared-edge special case.
2. **Spring-curve stage.** Project the spine to the two contact (spring) curves on A and B; record them as the trim boundaries.
3. **Section stage (polymorphic).** Sweep a section between the spring curves, normal to the spine. The section primitive is one of: **line** (chamfer, exact ruled/planar surface), **circular arc** (standard fillet, rational-quadratic, canal surface), **conic-by-rho** (shaped fillet), **curvature-matched spline** (G2 styling blend). Chamfer is therefore not a separate code path but the linear member of the section family, while still producing a genuinely straight, exact ruling.

**Representation: procedural canal surface with NURBS cache.** Store every blend as a **procedural surface** (spine + section law + supports = enough to re-evaluate exactly) plus a **cached NURBS approximation** to tolerance for downstream consumers (intersection, tessellation, export). Constant-radius fillets and equal-offset planar chamfers detect and store an **exact analytic** surface (torus/cylinder/sphere/cone/cyclide/plane) when one exists; everything else is procedural-canal + NURBS cache. This mirrors ACIS's rb_blend_spl_sur-with-analytic-simplification and is consistent with nurbs/05's general "exact-procedural plus tolerance NURBS" policy.

**Chamfer family as section + offset configuration.** Equal-offset, two-offset, offset-angle, face-offset, and apex-range chamfers are all the line-section engine with different **contact-curve placement** (equal offsets, unequal offsets, angle-driven, apex-relative ranges). One construction, parameterised.

**Order/mitring model.** Represent a blended region as a **blend graph** (after ACIS): nodes are blends, edges are interactions at shared vertices/faces. Derive sequencing (largest-radius-first as default) from the graph. At each vertex apply the **decision ladder**: mitre against neighbours -> roll the dominant blend over -> setback 2n-sided vertex patch (Varady). Edge blends must be **retractable** so vertex blends can request end-retraction. Record every blend's **unders** (supports, spring edges, cross edges, radius/offset law, overflow policy) so the feature is editable, suppressible, and removable.

**Overflow as an explicit policy.** Expose smooth / cliff / notch (plus auto) as a per-blend parameter. Model the overflowing blend as a **chain of faces** with per-segment supports; cliff overflow produces cliff (face-edge) blends, notch overflow produces notch blends that may be re-blended. Never silently emit an overflowing or self-intersecting surface.

**Recognition and removal (unblend).** For native geometry, drive removal/suppression from recorded unders: extend-and-intersect the supports to recover the sharp edge, recompute end vertices, separate interacting chains, delete in dependency order. For imported dumb geometry, recognise via the exact tests (constant-radius circular cross-section + rolling-ball verification for fillets; ruled/linear section for chamfers), optionally front-ended by a learning recogniser (FilletRec) but always confirmed by the exact verifier.

**Hard-case checklist (must be handled or cleanly refused):**

- Radius exceeds local concave radius of curvature -> canal self-intersects -> trim or refuse with reason (validity inequality from Lukacs).
- Radius near modelling tolerance -> spine/spring curves degenerate -> tolerance guard, refuse below a radius/tolerance ratio.
- Blend overflows its support -> apply chosen smooth/cliff/notch policy, model as face chain.
- Variable-radius spine marcher divergence -> adaptive, curvature-aware stepping; cap r(λ) variation by continuity limits.
- Vertex with three-plus unequal/mixed-sign blends -> mitre/roll-over/setback ladder; require retractable edge blends.
- Variable blend meeting a neighbour at a vertex -> constant-extension flattening near the junction for tangency.
- Tangent-face pileup around the edge -> robust support selection, possible user disambiguation.
- Hold-line tangency with no feasible radius at a section -> detect and report the degeneracy rather than emitting garbage.
- G2 requested but a support's curvature cannot be matched -> degrade to best-achievable continuity with a warning, not a silent G1.

In one line: **Keel's edge-finishing engine is spine + spring curves + a polymorphic section, storing each result as a procedural canal/ruled surface with a NURBS cache (analytic when exact), sequenced by a blend graph with a mitre/roll-over/setback corner ladder, with overflow as an explicit policy and full unblend/recognition support, and a hard-case checklist that trims or refuses rather than emitting invalid geometry.**

---

## References

1. Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365.
2. Choi, B. K., & Ju, S. Y. (1989). Constant-radius blending in surface modelling. *Computer-Aided Design, 21*(4), 213-220.
3. Chuang, J.-H., & Hwang, W.-C. (1997). Variable-radius blending of parametric surfaces. *The Visual Computer, 13*(7), 316-329.
4. *Fillet and surface intersections defined by rolling balls.* (1992). Computer Aided Geometric Design / CAD. [ScienceDirect 016783969290016I.]
5. Mokris, J. (1998). *Method for generating an extended rolling ball fillet surface between two surfaces* (US Patent 5,774,359). Autodesk, Inc.
6. *Method and apparatus for generating fillet surface between two surfaces* (US Patent 5,345,546).
7. Lukacs, G. (1998). Differential geometry of G1 variable radius rolling ball blend surfaces. *Computer Aided Geometric Design, 15*(6), 585-613.
8. Varady, T., Hermann, T., et al. (1996). Geometric construction for setback vertex blending. *Computer-Aided Design, 28*(6-7), 587-597.
9. Varady, T., et al. (2009). Setback vertex blends in digital shape reconstruction. In *Advances in Geometric Modeling and Processing* (LNCS, pp. 285-300). Springer.
10. Siemens Digital Industries Software. *Parasolid Functional Description: Edge Blending Overview & Functions* (v12.0 ch. 30-31; v35 Edge Blending Overview). q-solid.com mirror.
11. Siemens Digital Industries Software. *Parasolid Functional Description: Face-Face Blending* (v12.0 ch. 34). q-solid.com mirror.
12. Spatial Corp. *ACIS Blending (BLND) Component Reference* (R10/R17). Arizona ISL / q-solid ACIS mirror.
13. *Method for removing blends in B-rep models* (US Patent 7,236,168 B2). UGS/Siemens.
14. Lukacs, G., Marshall, A. D., & Martin, R. R. (1998). Methods to recover constant radius rolling ball blends in reverse engineering. *Computer Aided Geometric Design, 15*. [ScienceDirect S0167839699000436.]
15. Benko, P., Martin, R. R., & Varady, T. (2001). Algorithms for reverse engineering boundary representation models. *Computer-Aided Design, 33*(11), 839-851.
16. (2004). An efficient algorithm for recognizing and suppressing blend features. *Computer-Aided Design and Applications, 1*(1-4), 421-428.
17. Gao, J., Liu, T., Ye, H., & Chen, J. (2025). FilletRec: A Lightweight Graph Neural Network with Intrinsic Features for Automated Fillet Recognition. arXiv:2511.05561.
18. Whited, B., & Rossignac, J. (2009). Relative blending. *Computer-Aided Design, 41*(6), 456-462.
19. Onshape, Autodesk Alias, SolidWorks fillet documentation: conic-rho and curvature-continuous (G2) fillet sections. Vendor help (2023-2025).
20. Siemens *Solid Edge: Tangent Hold Lines*; Onshape *Face Blend* hold-line options. Vendor help.
21. Xometry, Wevolver, and CNC-manufacturing technical guides (2023-2025). *Fillet vs Chamfer* engineering references.

---

*Cross-references: nurbs/05 (offsets, blends, sweeps: rolling-ball theory, canal surfaces, vertex blends, offset self-intersection / medial axis), kernel/03 (local operations and direct editing: blend as a local op), kernel/13 (import healing: recognition in dumb geometry), kernel/15 (feature recognition), kernel/25 (Parasolid capability map: the audit that scoped this file).*
