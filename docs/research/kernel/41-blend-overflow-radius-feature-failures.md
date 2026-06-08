# 41. Blend Overflow and Radius-versus-Local-Feature Failure Handling

**Scope.** This dossier treats blend *overflow / runout* and *radius-versus-local-feature-size* as the two dominant, first-class failure modes of B-rep edge filleting, with concrete detection predicates and concrete recovery strategies. It is the failure-mode companion to file `40-blend-fillet-surface-generation.md`, which covers the rolling-ball blend surface, the spine-as-offset-intersection construction, the exact-analytic surface catalog (line-spine to cylinder, circle to torus, natural-quadric pairs to Dupin cyclide, else NURBS canal), vertex blends, and the basic radius-too-large rejection. Those mechanisms are assumed here and **not** repeated. This file goes deep where file 40 stops: what happens when the blend will not fit, how to predict it before generation, how to detect it during generation, and how to recover rather than merely reject.

Adjacent files, referenced but not duplicated:
- `28-fillet-chamfer-engine.md`: the engine-level smooth/cliff/notch overflow menu and blend-graph sequencing.
- `05-offsets-blends-sweeps.md` (NURBS): offset self-intersection and the medial-axis connection at the curve/surface level.
- `10-minkowski-medial-midsurface.md`: the medial axis, the medial axis transform (MAT), local feature size, and the wall-thickness field.
- `29-dirty-geometry-graceful-degradation.md`: the best-effort / partial-result doctrine this dossier's recovery policy plugs into.
- `34-test-corpus-adversarial-engineering.md`: the corpus/testing doctrine the adversarial section ties into.

The centerpiece deliverables are (a) the **medial-axis `r_max(t)` predictor** developed concretely in Section 2, (b) the **failure-mode catalog table** (failure mode to detection predicate to recovery options) in Section 3, and (c) the **reduce/reject/report default policy** for Keel in Section 4 and the closing synthesis.

A note on terminology used throughout: the **spine** is the locus of the rolling-ball center (the offset-surface intersection curve). The two **spring curves** (also "trimlines" or "contact curves") are the loci where the rolling ball touches each support face; they bound the blend ribbon on the two supports. The **support faces** are the two faces meeting at the blended edge. **Overflow** is defined precisely in Section 1.

---

## Section 1. Blend Overflow / Runout Taxonomy

### 1.1 Precise definition of overflow

A constant- or variable-radius edge blend is constructed by rolling a ball of radius `r(t)` in contact with both support faces; the ball center traces the spine, and the two contact points trace the two spring curves on the support faces. The blend surface is the ribbon swept between the spring curves.

**Overflow (runout) is the condition that a spring curve, or a blend boundary, leaves the valid (trimmed) region of its support face.** Concretely, parameterize the blend by arc length `t` along the spine. At parameter `t` the rolling ball touches face `F_i` at the contact point `c_i(t)`. The blend is *containable* at `t` if and only if **both** contact points `c_1(t)` and `c_2(t)` lie strictly inside the trimmed boundary of their respective faces (inside the face's outer loop and outside every inner loop). Overflow occurs at `t` when a contact point reaches or crosses a face boundary edge:

```
overflow(t)  iff  c_1(t) ∉ interior(trim(F_1))  OR  c_2(t) ∉ interior(trim(F_2))
```

Equivalently and more usefully for implementation: overflow is the event where the spring curve, as it is being traced, hits an edge of the support face's trimming loop. What lies *across* that edge determines which overflow *kind* applies and which recovery is valid. The taxonomy below enumerates those kinds by what the spring curve runs into.

### 1.2 Spring curve runs off the END of a support face (face too short)

The support face is too short, in the direction transverse to the blended edge, for the requested radius. As `r` grows, the contact point `c_i(t)` migrates away from the blended edge toward the far boundary of `F_i`. When `c_i(t)` reaches the far boundary, the ribbon has no more face to sit on. Geometrically this is the classic "the fillet is wider than the face." The boundary the spring curve hits is a *free* boundary (a boundary not shared as a smooth continuation) or a boundary beyond which there is simply nothing. This is the canonical *cliff/cap* situation when the far boundary is a sharp drop (Section 1.5), and the canonical *partial-blend or radius-limit* situation otherwise.

### 1.3 Blend rolls ONTO an adjacent face (roll-on / walk-onto-neighbor)

Here the boundary that the spring curve crosses is a **smooth (tangent) edge** shared with an adjacent face `F_i'`. Because the surfaces are tangent across that edge, the rolling ball can continue rolling: it simply transfers contact from `F_i` onto `F_i'` and keeps going. The blend "rolls onto" the neighbor. The blend surface must be re-trimmed: the portion of the ribbon whose contact has moved onto `F_i'` is now supported by `F_i'`, the spine kinks (its definition changes because one of the two offset surfaces being intersected changes from `offset(F_i, r)` to `offset(F_i', r)`), and the two segments are stitched at the parameter where the transfer happens. Production kernels call this case the **roll-on** overflow. It is benign and *desirable*: the correct answer is to carry the blend across the shared edge rather than to stop. The extended-rolling-ball construction (Choi/Lee, US 5,774,359) formalizes this by extending the offset/envelope intersection across the boundary curve so the spine and spring curves continue smoothly onto the neighbor rather than terminating at the original face's intersection curve.

Roll-on can cascade: the blend can roll from `F_i` onto `F_i'` and then onto `F_i''`, walking along a chain of tangent faces. Each transfer is a re-trim with a spine-definition change.

### 1.4 Blend runs ACROSS another edge or feature (third edge cuts the region)

A third edge, not one of the two support boundaries, passes through the region the blend wants to occupy. Examples: a small boss, a hole, a pocket edge, or a second blended edge crossing the first. Unlike roll-on, the crossed edge is generally **not tangent**, so the ball cannot smoothly continue across it; instead the blend region is interrupted. Two sub-cases:
- The third edge is *convex/sharp* and protrudes into the ribbon: the blend ribbon is sliced by it, producing a non-smooth interruption that must be resolved by capping, notching, or by blending the interaction (a blend-blend or blend-edge interaction, see 1.8).
- The third edge bounds a *hole or pocket* that the ribbon would otherwise cover: the blend would *swallow* the feature (Section 2.4). Detection of this is the "swallows-small-feature" predicate (Section 3.6).

### 1.5 Cliff edges (support face ends in a sharp drop)

A **cliff edge** is a *convex, non-tangent* boundary of the support face: past it the material drops away (a sharp edge of the solid). When the spring curve reaches a cliff edge, the ball cannot roll on (no tangent continuation, 1.3) and there is no more face (unlike a short face that continues elsewhere). The ball would fall off the cliff. The valid resolutions are:
- **cliff (smooth-off-the-cliff):** let the blend surface run off the cliff edge tangentially, i.e. the blend boundary follows the cliff edge, leaving the cliff edge sharp. The blend simply terminates against the drop, its boundary coincident with the cliff edge.
- **cliff_end:** the variant where the cliff condition occurs at the *end* of the blend (at a terminating vertex) rather than along its length.
- **cap:** insert a small cap face that closes the open end of the ribbon (Section 1.6).

Parasolid's vocabulary names these explicitly as overflow behaviours `ov_cliff` and `ov_cliff_end`.

### 1.6 Notch and cap handling

**Cap face.** When a blend ribbon has an open transverse end (the ball ran off the end of the face, 1.2, or off a cliff, 1.5), the ribbon edge is left dangling. A **cap** closes it with a small inserted face. In Parasolid the cap is formed by *extending at most two of the adjoining faces* to meet, closing the ribbon end with existing-geometry faces rather than a synthetic patch. A cap produces a watertight result at the cost of a small extra face and a non-smooth boundary at the cap.

**Notch blend.** A **notch** arises where the blend, as it grows, *trims away all of one side*: at some parameter the entire width of the blend face on one support is consumed, so the two spring curves on that side collapse and the supports themselves would meet. A notch overflow occurs "when all of one side of a blend face at the end of the blend is trimmed away" (Parasolid). The notch handling option, when enabled, **extends the blend along all the faces in the remaining ("right") wall** so the notch is bridged rather than left as a gap. Parasolid's `PK_BODY_fix_blends` exposes a `preserve_notch` control: with `preserve_notch = false` it draws a simple curve within the blend surface to bridge a laminar notch; with `preserve_notch = true` it projects the notch shape normally onto the blend surface to bridge the gap while preserving the notch's form. The notch is the concave dual of the cliff: a cliff is "face ends in a drop," a notch is "supports pinch together and the ribbon collapses."

### 1.7 The smooth / cliff / notch / cap / roll-on option vocabulary

The production vocabulary (Parasolid `ov_*` overflow behaviours; ACIS analogous controls) of what to do when a blend will not fit inside a face:

| Option | What it produces | When it is valid / chosen |
|---|---|---|
| **smooth** (`ov_smooth`) | The blend rolls onto and *merges* into another smooth surface (typically another rounding); the blend surface is deformed in the transition zone to stay tangent to both the original face and the surface it merges into. | The boundary crossed leads to another *tangent* rounding/surface; gives a fully smooth (tangent-continuous) result. The premium outcome. |
| **roll-on** | The blend transfers contact onto the adjacent tangent face and continues; ribbon re-trimmed and spine re-defined across the shared edge. | The crossed boundary is a tangent (smooth) edge with a face beyond it that the ball can roll onto. |
| **cliff** (`ov_cliff`) | The blend runs off the cliff edge, boundary coincident with the sharp edge; cliff stays sharp. | The crossed boundary is a *convex, non-tangent* (sharp) edge with a drop beyond. |
| **cliff_end** (`ov_cliff_end`) | As cliff, but the condition is at the terminating end of the blend (at a vertex). | Cliff condition coincides with a blend end vertex. |
| **notch** (`ov_notch`) | Bridges a collapsed ("notch") region by extending the blend along the opposite wall's faces. | The ribbon's one side is fully trimmed away (supports pinch). |
| **cap** | Inserts a cap face (often by extending up to two adjoining faces) to close an open ribbon end. | A ribbon end is open and a watertight closure is required; used when smooth/roll-on are not available. |

`smooth` and `roll-on` are the "carry the blend through" outcomes (preferred where geometry allows); `cliff`, `notch`, `cap` are the "terminate the blend cleanly" outcomes. The engine chooses per overflow event based on what lies across the crossed boundary (tangent rounding to merge into to smooth; tangent face to roll-on; sharp drop to cliff; pinch to notch; otherwise cap).

### 1.8 Overflow at convex-to-concave transitions and at vertices

**Convex-to-concave transition along an edge.** A single blended edge can change sign of curvature: convex (a round) over part of its length and concave (a fillet) over another. At the inflection, the rolling-ball geometry is continuous, but the *failure regime flips*: on the convex stretch, overflow is governed by running off finite faces and by cliffs (the ball sits outside the material); on the concave stretch, the dominant limit is `r` versus the *concave* radius of curvature and versus the local feature size (Section 2): the inner offset self-intersects and the ball gets pinched. So the same edge can be overflow-limited on one part and curvature/LFS-limited on another. The detection pass must therefore be evaluated *per parameter*, not once for the whole edge.

**Overflow where the blend meets a vertex.** At a vertex where several blended edges converge, each edge's ribbon approaches the vertex and the ribbons must be reconciled. If radii are compatible and the vertex is simple, a **vertex blend** (file 40) closes the corner. Overflow at a vertex is the case where the ribbons *do not fit*: the spring curves of adjacent edge blends cross before reaching the vertex, or a ribbon would overshoot the vertex onto an unrelated face. The standard remedy is **setback** (Section 4.2): terminate each edge blend short of the vertex (the spring-curve endpoints are pulled back by a setback distance) and fill the resulting hole with an n-sided vertex patch. ACIS exposes setback both as explicit per-edge setback parameters and as **autosetback**, where the kernel computes setbacks automatically (or runs with no parameters and works them out). The "suitcase corner" (three mutually perpendicular planes, three edge blends meeting at a vertex) is the textbook vertex-blend-with-setback case, resolved by an octant-of-a-sphere 3-sided patch joining the three spring-curve ends.

---

## Section 2. Radius versus Local Feature Size (the deep limit)

### 2.1 The fundamental limit via the medial axis

Define, for a solid `S` with boundary `∂S`, the **medial axis** `MA(S)` as the locus of centers of maximal inscribed balls (balls contained in `S` that touch `∂S` at two or more points, or tangentially at a curvature extremum). The **medial axis transform (MAT)** pairs each medial point with its inscribed-ball radius. The **local feature size** at a boundary point `p` is

```
lfs(p) = dist( p, MA(S) )      [Amenta-Bern: lfs(p) = distance from p to the medial axis]
```

the distance from `p` to the nearest point of the medial axis.

**The maximum safe blend radius equals the local feature size.** For an edge `e` to be filleted with a rolling ball of radius `r`, the ball must fit inside the local concavity (for a fillet) without colliding with the opposite wall, and the inner offset of the concave region must not self-intersect. Both conditions are exactly captured by the medial axis:

```
r_max(p) = lfs(p) = dist(p, MA(S))      (concave / fillet case)
```

**Why the medial axis is exactly the predictor.** Two complementary derivations:

1. *Offset self-intersection.* The inner offset of `∂S` by distance `r` (push the boundary inward by `r`) self-intersects exactly when `r` reaches the distance to the medial axis. This is definitional: the medial axis is the set of points equidistant from two or more boundary points, i.e. the locus where inward normals of distinct boundary pieces first meet. Offsetting by `r` collapses two boundary pieces onto the same point precisely when `r` equals their common distance to a medial point. Patrikalakis-Maekawa show the local version: an offset surface becomes **singular** when the offset distance equals the reciprocal of a principal curvature, `r = 1/κ` (a cusp forms at `r = 1/κ_max` of a concave region), and **globally self-intersects** at pairs of points whose inward normals cross, which is the medial axis. Thus the curvature condition `r < 1/κ_max` is the *local* necessary condition and `r < lfs = dist(·, MA)` is the *global* necessary-and-sufficient condition (global subsumes local: the medial radius is `≤ 1/κ_max` wherever two walls are closer than the local curvature radius).

2. *Where the rolling ball gets stuck.* The rolling ball of radius `r` is, by definition, an inscribed ball constrained to touch the two support faces. The maximal inscribed ball at a medial point has radius equal to the MAT radius. When the requested `r` exceeds the MAT radius along the spine, the ball cannot remain a *single* inscribed ball touching only the two intended supports: it must also touch a *third* boundary piece (the one defining the nearer medial sheet), which is exactly the collision / pinch / swallow event. So the medial axis is where the rolling ball "gets stuck," and `r ≤ lfs` is the feasibility certificate.

Net statement for Keel: **`r_max(t) = lfs(spine(t)) = dist(spine(t), MA(S))`**, with the local curvature bound `r_max_local(t) = 1/κ_concave_max(t)` as a cheaper conservative pre-filter (`r_max ≤ r_max_local` always; the two coincide when the nearest wall is the locally most-curved one).

### 2.2 Thin-wall and rib pinch

For a thin wall (a slab of local thickness `w(p)` between two roughly-parallel faces), the medial axis runs up the middle of the wall, so `lfs(p) ≈ w(p)/2` for a point on either face. Filleting an edge of a thin wall (or a rib) with `r > w/2` makes the fillet from one side meet or cross the fillet (or the opposite face) from the other side: the wall is pinched through. **Detection via the wall-thickness / medial field:** the thickness field `w(·)` is exactly `2·dist(·, MA)` between parallel walls (sphere/inscribed-ball thickness, as used in CAD wall-thickness analyzers and casting/molding sink-mark detectors). So the rib-pinch limit is

```
r_max_wall(t) = w(spine(t)) / 2 = dist(spine(t), MA(S))
```

which is again the LFS, specialized to the thin-wall geometry. Practically: precompute `w(·)` with an inscribed-sphere (sphere-method) thickness pass over the model; `r > w/2` along the spine flags rib-pinch before any surface is built.

### 2.3 Distance to neighboring features

When the limiting medial sheet is generated not by an opposite wall but by a nearby *feature* (an adjacent edge, vertex, hole, boss, or face), `lfs(spine(t))` is the distance to that feature. Exceeding it means the blend reaches the feature: it runs into the adjacent edge (1.4), or swallows a small feature (2.4). The single quantity `dist(spine(t), MA(S))` captures *all* of these because the medial axis is generated by *whatever* is nearest, opposite wall or feature alike. This is the unifying power of the medial predictor: one field, one query, all neighbor-distance failure modes.

### 2.4 Swallowing a small feature

A *small feature* (small hole, small boss, short edge) sitting within distance `< r` of the spine gets engulfed by the ribbon. The medial axis near a small feature has small MAT radius (the feature is "close to itself"), so `lfs` near it is small and the predictor flags it. The recovery is feature-aware: either reduce `r` locally below the feature distance, or carry the blend *around* the feature (re-trim, like a hole in the ribbon), or report the feature as un-blendable-at-this-radius.

### 2.5 Blend-blend interference

Two blends on two nearby edges collide when the sum of their radii exceeds the edge spacing. For edges `e_a` and `e_b` with closest approach `d(e_a, e_b)`, the ribbons interfere when

```
r_a + r_b  >  d(e_a, e_b)            (concave-facing pair, ribbons grow toward each other)
```

More generally, blend-blend interference is itself a medial-axis event: the medial axis *between the two blend surfaces* has radius `(r_a + r_b - overlap)`; interference is when that medial radius goes to zero. The robust detection is a proximity query between the two spring-curve ribbons (Section 3.5), with the linear `r_a + r_b > d` test as a cheap necessary pre-filter. Recovery is to *interact* the blends (mitre them together into a smooth junction) where they meet, or to reduce one or both radii.

### 2.6 The medial-axis-as-predictor approach (centerpiece)

**Idea.** Before attempting any surface generation, run a *feasibility pass* that produces a per-parameter radius ceiling `r_max(t)` along the spine and flags the parameters and kinds of every predicted overflow/pinch. This converts filleting from "try and fail" into "predict, then build only what will succeed (or build a radius-limited variant that is guaranteed to succeed)."

**Concrete procedure.**

1. **Build (or approximate) the medial field.** Full exact MAT of a B-rep is expensive and brittle, so use one of:
   - an *approximate medial axis* from a Voronoi/sampling construction over `∂S` (Dey's approximate MAT for CAD models; Amenta-Bern poles), or
   - a *distance field / inscribed-sphere thickness field* sampled on a voxel or octree grid (the wall-thickness-analysis route), which gives `2·lfs` directly between walls, or
   - for the *local* bound only, the analytic principal curvatures of the support faces (cheap, gives `1/κ`).
   Keel does not need the full medial axis as a clean geometric object; it needs the *scalar field* `m(p) = dist(p, MA(S))`. A signed-distance / nearest-other-feature query suffices.

2. **Sample the spine.** Take the candidate spine (it can be computed from the requested nominal `r`, or from a small probe radius just to get a curve along the edge) and sample it at parameters `t_0, t_1, ..., t_n`, adaptively densified near curvature extrema and near where neighboring features are close.

3. **Query the ceiling at each sample.** At each `t_k`, compute

   ```
   r_max(t_k) = min(
       dist(spine(t_k), MA(S)),                       # global LFS / nearest-wall-or-feature
       1 / kappa_concave_max(t_k),                     # local concave curvature bound (cheap pre-filter)
       0.5 * w(spine(t_k)),                            # thin-wall bound (redundant with LFS, kept for clarity / cheap field)
       dist(spine(t_k), nearest_other_blend_ribbon)    # blend-blend bound, if other blends present
   )
   ```

   The first term subsumes the second through fourth in exact arithmetic; in practice each term is computed by a *different, cheaper* oracle, and the `min` is robust to any one oracle being approximate or unavailable.

4. **Classify each sample.** Compare requested `r(t_k)` to `r_max(t_k)`:
   - `r(t_k) < r_max(t_k) - margin`: feasible at `t_k`.
   - `r(t_k) ≈ r_max(t_k)`: marginal; mark for densified sampling and for the tangent-continuity-aware recovery.
   - `r(t_k) > r_max(t_k)`: predicted failure at `t_k`; tag the *kind* by which oracle was binding (curvature to self-intersect; nearest wall to pinch; nearest feature to swallow; other ribbon to blend-blend).

5. **Emit the feasibility profile.** Produce intervals of `t` that are feasible, marginal, or failing, each failing interval annotated with its kind and with the binding `r_max(t)`. This profile *is* the input to the recovery decision (Section 4) and to the generation pipeline of file 40: the analytic-surface catalog is invoked only on feasible intervals at the (possibly reduced) `r(t)`.

This pre-generation feasibility pass is the single most valuable engineering idea in this dossier: it predicts where and how the blend will fail, with a per-parameter radius ceiling, *before* committing to surface construction, and it does so with one scalar field query per spine sample.

---

## Section 3. Detection Algorithms (one per failure mode, codeable)

Each predicate below is given as an evaluable geometric test, with where it runs in the pipeline.

### 3.1 Offset self-intersection / curvature test  (pre-generation, cheap)
Predicate: along the concave portion of the spine, `r > 1/κ_concave_max(t)` for some `t`, where `κ_concave_max(t)` is the maximum (most curved) concave normal curvature of the supports in the relevant section. Local necessary condition for the inner offset to be regular. Cheap: needs only the support faces' second fundamental forms sampled along the edge. Runs in the feasibility pass as the cheap pre-filter for `r_max_local`.

### 3.2 Spring-curve-contained-in-face test  (pre-generation, or during tracing)
Predicate: for each support `F_i` and each spine sample `t`, the contact point `c_i(t)` lies in `interior(trim(F_i))` (inside the outer loop, outside all inner loops). Point-in-trimmed-face test (project to face parameter space, even-odd / winding against the trim loops). The first `t` at which it fails marks an overflow event; the *boundary edge* it fails across classifies the overflow kind (tangent edge to roll-on; convex sharp edge to cliff; hole loop to swallow). Runs during spring-curve tracing; can be pre-screened by the LFS pass.

### 3.3 Opposing-spring-curves-cross test (thin-wall pinch)  (pre-generation)
Predicate: for a thin wall, the inward contact point from one side, `c_1(t)`, and the contact point of the opposite-side blend (or the opposite face itself), `c_1'(t)`, satisfy `dist(c_1(t), opposite_wall) < r` or the two ribbons overlap. Reduces to `r > w(spine(t))/2`, i.e. `r > lfs`. Implemented as the thin-wall term of the feasibility pass.

### 3.4 Blend-runs-off-face-end test  (pre-generation / during)
Predicate: the contact point `c_i(t)` reaches the *far free boundary* of `F_i` (the boundary opposite the blended edge), with no tangent face beyond. Equivalent to: transverse face extent at the section through `t` is `< ` the ribbon half-width needed for `r`. A specialization of 3.2 where the crossed boundary is free/cliff rather than tangent.

### 3.5 Blend-blend proximity test  (pre-generation)
Predicate: minimum distance between the two blends' spring-curve ribbons `< 0` (overlap), with cheap pre-filter `r_a + r_b > d(e_a, e_b)`. Compute by sampling both ribbons and running a closest-pair / interval proximity query; flag overlapping parameter intervals on both edges. Runs in the feasibility pass when multiple edges are blended together (the blend network case).

### 3.6 Swallows-small-feature test  (pre-generation)
Predicate: a model feature `f` (hole loop, small boss face, short edge, vertex) lies within `dist(spine(t), f) < r` for some `t`, i.e. the feature falls inside the ribbon footprint. Compute by querying the LFS field (a small feature creates a low-LFS region) and/or by a direct distance query from spine samples to the feature's geometry. Flags the feature and the affected `t`-interval.

All six predicates are unified by the medial/LFS pass (3.1, 3.3, 3.5, 3.6 are all `r > dist(spine, ·)` against different "·"; 3.2 and 3.4 are the face-containment realization of the same events). The recommended pipeline runs the LFS feasibility pass *pre-generation* to predict, then re-checks 3.2/3.4 *during* spring-curve tracing as the exact, authoritative guard.

---

## Section 4. Recovery Strategies (not just rejection)

The team's explicit requirement: handle these failures, do not merely reject. For each failure mode, the recovery options and their tradeoffs.

### 4.1 Local radius reduction / radius limiting
Where the requested constant `r` exceeds `r_max(t)`, **automatically shrink `r(t)` to `min(r_requested, r_max(t) - margin)`**, producing a *variable-radius* blend that respects the ceiling everywhere. Algorithm:
1. From the feasibility profile, take `r_max(t)` over the failing intervals.
2. Define a target radius law `r(t) = min(r_req, (1 - ε)·r_max(t))`, then *smooth* it: clamp the derivative `|dr/dt|` and round the corners (e.g. a `C^2` monotone-cubic or a small-window smoothing) so the radius law is at least `G^1` (preferably `G^2`) in `t`. Abrupt radius changes create surface kinks and curvature spikes, which violate the continuity goals of file 40.
3. Re-trace the spring curves with the new `r(t)` and feed the variable-radius construction of file 40.
**Continuity cost.** Radius limiting trades the requested constant radius for a guaranteed-buildable variable radius. The surface remains tangent (`G^1`) to the supports by construction; curvature continuity (`G^2`) across the blend depends on the smoothness of `r(t)` and on the Pegna-Wolter linkage-curve condition (curvature continuity is obtained if normal curvatures agree along the spring curve in a direction other than its tangent). Keep `r(t)` smooth to preserve `G^2`; accept `G^1` in the worst reduced zones. The downside is a visually variable fillet where the user asked for constant; this must be *reported* (Section 4.6).

### 4.2 Setback and blend retraction near the failing region
Instead of (or in addition to) reducing radius, **retract the blend**: terminate the spring curves short of the failing region and leave the supports unblended there. **Setback** is the vertex-specific form: pull each converging edge blend back from the vertex by a setback distance and fill with an n-sided vertex patch (ACIS setback / autosetback; file 40 vertex blends). Retraction is the along-edge form: stop the ribbon before the cliff/notch and cap or smooth the end. Tradeoff: a clean, buildable result that does not blend the whole edge; the un-blended stub must be reported. Setback is the standard, robust vertex-overflow remedy and should be Keel's default at congested vertices.

### 4.3 Roll-on / re-trim onto the adjacent face
When the crossed boundary is tangent (1.3), **carry the blend across** using the extended-rolling-ball construction: extend the offset/envelope intersection past the shared edge so the spine and spring curves continue onto the neighbor (US 5,774,359), then re-trim the ribbon so each segment is supported by the correct face and stitch at the transfer parameter. Tradeoff: more complex topology (the blend now spans multiple supports and the spine is piecewise) but the *correct* and fully smooth answer where tangency permits. Prefer roll-on/smooth over capping whenever a tangent continuation exists.

### 4.4 Capping and notching the blend end
When neither smooth nor roll-on applies (the boundary is a sharp cliff or the ribbon collapses):
- **cap:** close the open ribbon end with a cap face (extend up to two adjoining faces to meet, Parasolid-style) to keep the body watertight. Tradeoff: a small extra face and a non-tangent boundary at the cap.
- **notch:** bridge a collapsed (pinched) region by extending the blend along the opposite wall (Parasolid `preserve_notch`: simple bridging curve, or projected-notch-shape bridging). Tradeoff: bridges the gap but the notch region is geometrically delicate.
Both are *termination* recoveries: they produce a valid solid with a clean but non-smooth boundary where the blend had to stop.

### 4.5 Partial blend
Blend only the feasible intervals of the edge, leave the failing intervals unblended (sharp), and give each blended sub-interval a clean, capped/smooth boundary. The result is a *partial* fillet with explicit per-region diagnostics. This is the graceful-degradation answer when neither radius reduction nor roll-on can salvage a failing interval (e.g. a true thin-wall pinch where any positive radius pinches). Tradeoff: the edge is not fully filleted, but the caller gets a valid body plus a precise report of what was skipped and why.

### 4.6 The reduce-vs-reject-vs-report policy (recommended default for Keel)

Recommended default, tied to the best-effort doctrine of file 29:

1. **Predict first.** Always run the LFS feasibility pass (Section 2.6) before generation. Never discover failure only by a thrown exception.
2. **Prefer carry-through.** Where a tangent continuation exists, **roll-on / smooth** (4.3). This changes topology, not the user's radius, and gives the best surface.
3. **Then reduce.** Where carry-through does not apply and the failure is a soft overflow or mild pinch, **radius-limit** to `r_max(t)` (4.1), producing a smooth variable-radius blend, *and record* that the radius was reduced and over which interval.
4. **Then retract/setback.** At congested vertices, **setback** (4.2); near hard cliffs/notches, **cap/notch** (4.4) to keep the body valid.
5. **Then partial + report.** Where an interval cannot be blended at any acceptable radius (true pinch, swallowed feature the user wants preserved), **partial-blend** it (4.5): blend the rest, leave that interval sharp, and return it in the diagnostics.
6. **Reject only as last resort, and never silently.** Return a `BlendResult` that is *always a valid body* (possibly the original, unblended where impossible) plus a structured per-region diagnostic: for each edge/parameter-interval, the outcome (full / radius-limited / rolled-on / capped / setback / skipped), the binding failure kind, and the effective `r(t)`. This mirrors OCCT's `NbFaultyContours` / per-contour status idea but goes further by returning a *usable partial body* rather than a not-done flag.

The policy is **best-effort with full disclosure**: do the most the geometry allows, keep the body valid, and tell the caller exactly what was changed and what was skipped.

---

## Section 5. How Production Kernels Handle Overflow and Over-Large Radius

### 5.1 Parasolid
Parasolid treats overflow as a first-class, named set of behaviours. Many blends "cannot fit entirely inside one or both of the faces adjacent to the blended edge"; Parasolid has explicit strategies to fix such blends, the process it calls **creating blend overflows**. The documented overflow behaviours include `ov_smooth`, `ov_cliff`, `ov_cliff_end`, and `ov_notch` (Edge Blend Overflows chapter). `ov_smooth` deforms the rounding in the transition zone to stay tangent to both the original face and the surface it merges into. Capping closes a ribbon end by extending at most two adjoining faces. `PK_BODY_fix_blends` exposes `preserve_notch` (false: simple bridging curve within the blend; true: projected notch shape) for laminar-notch bridging. Parasolid also has a dedicated **blend-error-code chapter** (Interpreting Edge Blending Error Codes) and a blend-network treatment for interacting blends; later versions added robustness to notch-overflow blends on mixed geometry. Recovery-vs-rejection philosophy: Parasolid is strongly recovery-oriented, with a rich documented menu of overflow fixes and a separate fix-blends pass, rather than failing on a blend that does not fit. *Documented:* the `ov_*` behaviours, capping by face extension, `preserve_notch`, the error-code chapter. *Inferred:* the precise internal auto-limiting of radius (Parasolid will fail a blend whose requested radius is too large for the local geometry rather than silently shrinking it; the variable-radius and overflow machinery is the user-facing route to handle large radii).

### 5.2 ACIS
ACIS works on a **network of edges and vertices**, each carrying a blend radius/chamfer distance; the presented edges/vertices "should not form disjoint networks." Interaction and interference between blend geometry and the model are explicit concepts: the blend graph manages how blends interact with each other and with the model. **Vertex blends** are first-class, with shape controls including **setbacks**, which may be passed explicitly or computed automatically (**autosetback**, or "no parameters at all"); the suitcase corner is the canonical 3-sided vertex patch (octant of a sphere). ACIS sequences of blended edges are *mitred* where adjacent blends meet, and the blend-network/sequence handling resolves interacting and overflowing blends. ACIS ships a documented set of **standard blending error messages** and advanced-blending error appendices. Recovery-vs-rejection philosophy: ACIS, like Parasolid, is recovery-oriented for *interaction* and *vertex* cases (autosetback, mitring, network resolution) but will report a blend error where geometry truly cannot support the requested blend. *Documented:* the edge/vertex network, autosetback, vertex-blend shape controls, mitring of sequences, error-message catalogs. *Inferred:* exact internal overflow-onto-neighbor handling (analogous to Parasolid roll-on) and any automatic radius reduction.

### 5.3 OCCT (Open CASCADE) `BRepFilletAPI_MakeFillet`
OCCT's fillet is the documented-fragile contrast case. It is **sequential**: edges are processed in order, so which edges fail can depend on the order they are added. On too-large radius or unsupported configuration, `Build()` may **throw** (e.g. `StdFail_NotDone`, "BRep_API: command not done") or leave the operation not-done; users routinely wrap `Build()` in try/catch. Reported behaviour: a radius that succeeds at 1 mm can fail above ~2 mm on the same edge (a raw radius-vs-feature limit with no auto-reduction). Detection/diagnostics: check `IsDone()`; do **not** use the result if not done; `NbFaultyContours()` / faulty-contour and faulty-vertex queries report which contours failed; "There are no suitable edges for chamfer or fillet" indicates an empty suitable-edge list (e.g. seam or neutral edges). Best practice is *pre-validation* (ShapeAnalysis) to drop unsuitable edges before `Add()`, since detection should happen before `Build()`. The OCCT 2D fillet (`BRepFilletAPI_MakeFillet2d`) returns `ChFi2d_ComputationError` when the radius is too large to build between adjacent edges, vs `ChFi2d_IsDone`. Recovery-vs-rejection philosophy: OCCT is **rejection-oriented** with weak recovery: no documented automatic radius limiting, no roll-on menu, throw-or-not-done on overflow, and per-contour fault reporting at best. The community-built "fallback returning which edges fail" prototypes (build123d issue #1224) exist precisely because the kernel does not gracefully degrade on its own. *Documented:* IsDone/NbFaultyContours/exceptions, sequential order-dependence, the 2D `ChFi2d_ComputationError`. *Inferred:* the absence of internal radius auto-limiting (consistent with reported hard failures at modest radii).

### 5.4 nTop (implicit / field-based, contrast)
nTop's filleting is built on an implicit/field data structure rather than B-rep trimmed surfaces. Its claim is that fillets "do not fail" and "always return a valid, solid model" even at large radii that "would normally cause a model to completely fail within a traditional CAD environment due to interior surfaces colliding and intersecting." It does *not* document explicit LFS/medial detection or radius thresholds; instead the field representation absorbs self-intersection (the union/blend of fields is always a valid solid). The lesson for Keel: a field/implicit fallback (file 09, mesh/implicit/hybrid) is a robust *last-resort* recovery for blends that defeat the exact B-rep construction, at the cost of leaving the exact-analytic regime. Recovery philosophy: maximally recovery-oriented (never fail), at the cost of exactness.

### 5.5 Cross-kernel synthesis
| Kernel | Overflow model | Vertex/interaction | Over-large radius | Philosophy |
|---|---|---|---|---|
| Parasolid | Named `ov_smooth/cliff/cliff_end/notch` + cap; `fix_blends`/`preserve_notch` | Blend network; mitred sequences | User handles via variable-radius/overflow; rich fix menu | Recovery-oriented, fully documented |
| ACIS | Overflow handled in edge/vertex network; mitring | Vertex blends, setback/**autosetback** | Reported error where unsupportable | Recovery-oriented for interaction/vertices |
| OCCT | None (throw / not-done) | Limited | Hard fail at modest radii, no auto-limit | Rejection-oriented, fragile |
| nTop | Field absorbs self-intersection | Implicit | "Never fails" | Recovery-by-representation, non-exact |

Keel should adopt Parasolid/ACIS-style named overflow recovery and setback, add an *explicit LFS feasibility predictor* (which the documented kernels do internally but do not expose), and keep an implicit fallback (nTop-style) as the last-resort recovery, avoiding OCCT's throw-or-die fragility.

---

## Section 6. Testing and Adversarial Cases (tie-in to file 34)

Because overflow and radius-vs-feature are the dominant fillet failure modes, the corpus must *target* them. Adversarial generators (per file 34's doctrine):

- **Radius sweep through `r_max`.** For each test edge, sweep `r` from below to above the computed `r_max(t)` and assert: below, full blend; at, marginal handling; above, the predicted recovery (reduce/roll-on/cap/partial) fires and the body stays valid. The transition radius is an oracle (it should equal the LFS at the binding parameter).
- **Edges near features.** Generate edges at parametric distance `d` from a hole / boss / second edge, sweep `d` and `r` so `r` crosses `d`; assert swallow detection and feature-preserving recovery.
- **Thin walls / ribs.** Slabs of thickness `w` swept down; fillet an edge with `r` crossing `w/2`; assert opposing-spring-cross detection and pinch recovery (partial or reduced).
- **Closely spaced edges (blend-blend).** Two parallel edges at spacing `s`, radii `r_a, r_b` with `r_a + r_b` crossing `s`; assert blend-blend interference detection and mitre/reduce recovery.
- **Cliffs, notches, roll-on faces.** Construct each overflow kind explicitly (short face to cliff; tangent neighbor to roll-on; pinch to notch) and assert the *correct* overflow behaviour is selected, not merely *a* valid result.
- **Convex-to-concave edges.** Edges that inflect, asserting the per-parameter feasibility pass switches failure regime at the inflection.
- **Vertices.** Suitcase corners and worse (4+ edges, unequal radii) to exercise setback/autosetback and vertex-blend overflow.

Oracle hooks: the LFS feasibility profile is itself a checkable artifact (its predicted failure parameters should match where generation actually fails when recovery is disabled), giving a strong differential test between prediction and generation.

---

## Failure-Mode Catalog (centerpiece)

| # | Failure mode | Geometry | Detection predicate (codeable) | When it runs | Recovery options (preferred to last-resort) |
|---|---|---|---|---|---|
| F1 | Inner-offset self-intersection (concave curvature) | Concave section, `r` past curvature radius | `r > 1/κ_concave_max(t)` (local), subsumed by `r > dist(spine(t), MA)` | Pre-gen feasibility pass | Radius-limit to `r_max(t)`; else partial-blend |
| F2 | Spring runs off face END (short face) | Face too short transversely; far boundary free/cliff | `c_i(t) ∉ interior(trim(F_i))` with far boundary free | During tracing; pre-screened by LFS | Cliff/cap; radius-limit; partial |
| F3 | Roll-on onto adjacent (tangent) face | Crossed boundary is a smooth/tangent edge | `c_i(t)` crosses a *tangent* edge of `F_i` | During tracing | **Roll-on / smooth** (extend across, re-trim, stitch) |
| F4 | Runs across a third edge/feature | Non-tangent edge cuts the ribbon | `c_i(t)` crosses a non-tangent interior edge / hole loop | During tracing | Cap/notch; blend the interaction; partial |
| F5 | Cliff (sharp drop at face end) | Convex non-tangent boundary, drop beyond | `c_i(t)` reaches a convex sharp boundary | During tracing | **cliff / cliff_end**; cap |
| F6 | Notch (ribbon side fully trimmed / supports pinch) | One side of blend face consumed | One spring-curve width to 0 at some `t`; supports meet | During tracing / pre-gen | **notch** bridging (`preserve_notch`); partial |
| F7 | Thin-wall / rib pinch | `r` past half local wall thickness | `r > w(spine(t))/2 = dist(spine(t), MA)` | Pre-gen (thickness field) | Radius-limit to `w/2`; else partial (sharp interval) |
| F8 | Swallows small feature | Feature within `r` of spine | `dist(spine(t), feature) < r`; low-LFS region | Pre-gen (LFS / feature query) | Radius-limit below feature distance; route around; partial |
| F9 | Blend-blend interference | Two near edges, radii too large | `r_a + r_b > d(e_a,e_b)` pre-filter; ribbon overlap exact | Pre-gen (network) | Mitre/interact; reduce one or both radii |
| F10 | Vertex overflow | Converging edge blends do not fit at vertex | Spring-curve ends cross before vertex; ribbon overshoots | Pre-gen / at vertex closure | **setback / autosetback** + n-sided vertex patch |
| F11 | Over-large constant radius (global) | Requested `r` exceeds `r_max(t)` anywhere | `r > r_max(t) = min(LFS, 1/κ, w/2, ribbon-dist)` for some `t` | Pre-gen feasibility pass | Radius-limit to variable `r(t)`; report reduction |

All detection rows reduce, at root, to the single medial predicate `r > dist(spine(t), MA(S))` against the relevant nearest geometry (wall, feature, or other ribbon), with the face-containment tests (F2-F6) as the exact realization during tracing and the curvature test (F1) as the cheap local pre-filter.

---

## The medial-axis `r_max(t)` predictor (concrete, restated for use)

```
INPUT:  solid S, blended edge e, requested radius law r(t) (constant or variable)
FIELD:  m(p) = dist(p, MA(S))            # LFS field; from approx-MAT or inscribed-sphere thickness
        kappa_cc(p)                       # max concave normal curvature of supports near e
        ribbons[]                         # other active blend ribbons (network case)

1. spine = trace_spine(e, probe_or_requested_radius)
2. T = adaptive_sample(spine)             # densify near curvature extrema & near close features
3. for t in T:
     r_max[t] = min(
        m(spine(t)),                       # nearest wall OR feature  (covers F2,F4,F5,F7,F8)
        1.0 / kappa_cc(t),                 # local self-intersection   (covers F1)
        min_j dist(spine(t), ribbons[j])   # blend-blend               (covers F9)
     )
4. for t in T:
     if r(t) <  r_max[t] - margin:  mark(t, FEASIBLE)
     elif r(t) <= r_max[t]:         mark(t, MARGINAL)   # densify, G1 fallback
     else:                          mark(t, FAILING, kind = argmin oracle)
5. intervals = coalesce(T by mark)         # feasible / marginal / failing(kind) runs
6. emit FeasibilityProfile{ intervals, r_max(.) }   # drives recovery (Sec 4) + generation (file 40)
```

`r_max(t)` is the per-parameter radius ceiling; `FeasibilityProfile` is the single artifact passed to both the recovery decision and the file-40 surface generator (which builds only feasible intervals at the possibly-reduced `r(t)`).

---

## Overflow and Radius-Limit Handling for Keel (closing synthesis)

1. **Pre-generation feasibility pass (mandatory).** Before any surface is built, run the medial-axis `r_max(t)` predictor above. Source the LFS field `m(p)` from an approximate-MAT (Dey-style, for CAD B-reps) or an inscribed-sphere thickness field (wall-thickness route); use analytic `1/κ` as the cheap local pre-filter; include other ribbons for the network case. Output the `FeasibilityProfile`: feasible/marginal/failing intervals with per-parameter ceiling and binding failure kind. This makes failure *predicted*, never merely *thrown*.

2. **Per-failure-mode detection-and-recovery playbook** (catalog above), applied per `t`-interval:
   - tangent boundary crossed to **roll-on / smooth** (F3): extend across, re-trim, stitch (extended-rolling-ball);
   - soft overflow / mild concave or thin-wall excess to **radius-limit** to a smooth variable `r(t) = min(r_req, (1-ε)·r_max(t))` (F1, F2, F7, F8, F11), preserving `G^1` always and `G^2` where `r(t)` stays smooth (Pegna-Wolter linkage condition);
   - cliff / pinch to **cliff / notch / cap** termination (F4, F5, F6);
   - vertex congestion to **setback / autosetback** + n-sided vertex patch (F10);
   - blend-blend to **mitre / interact**, else reduce (F9);
   - irrecoverable interval to **partial-blend**: blend the rest, leave it sharp, report it.

3. **Default policy: reduce / reject / report = best-effort with full disclosure.** Order of preference: predict to carry-through (roll-on/smooth) to reduce (radius-limit) to retract (setback/cap/notch) to partial+report to (last resort, never silent) reject. The kernel **always returns a valid body** (unblended where impossible) plus a structured per-region diagnostic: outcome, binding failure kind, and effective `r(t)` for every edge/interval. This is the file-29 graceful-degradation doctrine specialized to filleting, and it is strictly stronger than OCCT's not-done/throw model and on par with Parasolid/ACIS recovery, while exposing the LFS feasibility profile those kernels keep internal.

4. **Integration with the file-40 pipeline.** The `FeasibilityProfile` is the gate in front of file 40's generation: the analytic-surface catalog (line-spine to cylinder, circle to torus, natural-quadric pairs to Dupin cyclide, else NURBS canal) and the variable-radius construction are invoked only on feasible (or radius-limited) intervals, at the ceiling-respecting `r(t)`. Roll-on emits multi-support spine segments that file 40 stitches; setback emits vertex patches via file 40's vertex-blend path; partial-blend emits capped sub-ribbons. The basic radius-too-large rejection in file 40 becomes the *fallback* when the feasibility pass cannot find any recovery, not the first line of defense. An implicit/field fallback (file 09, nTop-style) is the documented last resort for blends that defeat the exact B-rep construction entirely, accepting loss of exactness to keep the operation succeeding.

---

## References

1. Vida, J., Martin, R. R., & Varady, T. (1994). *A survey of blending methods that use parametric surfaces.* Computer-Aided Design, 26(5), 341-365. https://www.sciencedirect.com/science/article/abs/pii/001044859490023X (also SZTAKI GML 1992/4 report: https://eprints.sztaki.hu/682/)
2. Pegna, J., & Wolter, F.-E. (1992). *Geometrical criteria to guarantee curvature continuity of blend surfaces* (Linkage Curve theorem; second-order smoothness criterion, orig. 15th Design Automation Conf., 1989). https://www.semanticscholar.org/paper/Geometrical-Criteria-to-Guarantee-Curvature-of-Pegna-Wolter/1e2c0f2f1ae3f562587bd86b68421141dbef8c13
3. Maekawa, T., & Patrikalakis, N. M. *Computation of singularities and intersections of offsets of planar curves.* https://www.semanticscholar.org/paper/Computation-of-singularities-and-intersections-of-Maekawa-Patrikalakis/3de9f1b6615d607305178b52d6b4c90f8380d12b
4. Patrikalakis, N. M., Maekawa, T., & Cho, W. *Shape Interrogation for Computer Aided Design and Manufacturing*, Chapter 11.3 (Offset singularities, self-intersection criteria, tracing of self-intersection curves). MIT hyperbook. https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/node225.html , .../node227.html , .../node229.html
5. *Self-intersection detection and elimination in freeform curves and surfaces.* Computer-Aided Design (2008). https://dl.acm.org/doi/10.1016/j.cad.2007.10.004
6. Rossignac, J. R., & Requicha, A. A. G. (1986). *Constant-radius blending in solid modelling.* Computers in Mechanical Engineering. https://www.semanticscholar.org/paper/CONSTANT-RADIUS-BLENDING-IN-SOLID-MODELLING-Rossignac-Requicha/553014d71e15e44f3585377bc9fd017d4f823b3d
7. Rossignac, J. R., & Requicha, A. A. G. *Offsetting operations in solid modelling.* https://faculty.cc.gatech.edu/~jarek/papers/Offsets.pdf
8. Whited, B., & Rossignac, J. *Relative blending.* Georgia Tech. https://faculty.cc.gatech.edu/~jarek/papers/relativerounding.pdf
9. Choi, B. K., & Lee, C. S. *Method for generating an extended rolling ball fillet surface between two surfaces.* US Patent 5,774,359 (Autodesk). https://patents.google.com/patent/US5774359A/en
10. *Medial axis of a planar region by offset self-intersections.* Computer-Aided Design (2009). https://www.sciencedirect.com/science/article/abs/pii/S0010448509002176
11. *Computation of medial axis and offset curves of curved boundaries in planar domain.* https://www.researchgate.net/publication/223524237_Computation_of_medial_axis_and_offset_curves_of_curved_boundaries_in_planar_domain
12. Amenta, N., & Bern, M. *Surface reconstruction by Voronoi filtering* (local feature size = distance to medial axis; poles approximate MA). See also Local feature size, Wikipedia: https://en.wikipedia.org/wiki/Local_feature_size
13. Dey, T. K., & Zhao, W. *Approximate medial axis for CAD models.* ACM Solid Modeling. https://dl.acm.org/doi/pdf/10.1145/781606.781652
14. *Medial axis extraction and thickness measurement of formed sheet metal parts* (MAT-based thickness field). https://www.researchgate.net/publication/296062403_Medial_Axis_Extraction_and_Thickness_Measurement_of_Formed_Sheet_Metal_Parts
15. Geometric Global / GeomCaliper. *Efficient wall thickness analysis for casting parts* (inscribed-sphere thickness = 2·LFS; rib-to-wall ratio rules). https://geomcaliper.geometricglobal.com/files/2017/05/Whitepaper-Efficient-Wall-Thickness-Analysis.pdf
16. Siemens / Parasolid. *Edge Blending: Overview; Functions and Options; Edge Blend Overflows; Interpreting Edge Blending Error Codes* (`ov_smooth`, `ov_cliff`, `ov_cliff_end`, `ov_notch`, capping, `PK_BODY_fix_blends` / `preserve_notch`). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html , .31.html , .33.html , .34.html ; V35: http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.075.html ; http://www.q-solid.com/Parasolid_Docs/headers/pk_body_fix_blends.html
17. Spatial / ACIS. *Blending Component; Modeling Operations > Blending; Vertex Blends; Sequences of Blended Edges; Standard Blending Error Messages* (edge/vertex network, autosetback, mitring, vertex-blend shape controls). http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF ; http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblnd.htm ; .../SPAacisuser_moblndse.htm ; .../SPAacisuser_moblnder.htm ; Vertex Blends: http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0010.HTM
18. Open CASCADE Technology. *BRepFilletAPI_MakeFillet* failure handling (IsDone, NbFaultyContours, exceptions, sequential order-dependence; 2D `ChFi2d_ComputationError`). Forum: https://dev.opencascade.org/content/error-control-brepfilletapimakefillet ; https://dev.opencascade.org/content/brepfilletapimakefillet-fillet-failed ; build123d per-edge fallback issue: https://github.com/gumyr/build123d/issues/1224 ; 2D ref: https://old.opencascade.com/doc/occt-7.2.0/refman/html/class_b_rep_fillet_a_p_i___make_fillet2d.html
19. nTopology. *The art of the fillet: How nTop automated one of CAD's oldest problems* (field-based filleting, "never fails," large-radius self-intersection absorbed by representation). https://ntop.com/blog/automated-filleting-that-wont-break-your-model

## Primary-source verification (2026-06-08)

The Parasolid overflow and setback vocabulary in this dossier was originally drawn from search-engine extractions of the q-solid mirror (the live pages refused automated fetch). It has since been checked against the primary Parasolid v12 Functional Description pages (Edge Blending Overview, Edge Blending Functions and Options, Interpreting Edge Blending Error Codes, Face-Face Blending, PK_BODY_fix_blends), fetched manually from the q-solid mirror for verification. The vocabulary is confirmed accurate:

- The four blend-overflow behaviors are exactly `ov_smooth`, `ov_cliff`, `ov_cliff_end`, `ov_notch` (the `ov_cliff_end` value, previously flagged as uncertain, is genuine). They apply to `PK_EDGE_set_blend_constant`, `PK_EDGE_set_blend_chamfer`, and `PK_EDGE_set_blend_variable`. Overflow is defined as the blend, per its basic parameters, lying outside the faces adjacent to the blended edge.
- `preserve_notch` is a real `PK_BODY_fix_blends` option.
- Setback: a setback-specified edge is trimmed back by approximately the given distance and the remaining gap is patched smoothly by a collection of faces; a setback blend applies only when all edges into a vertex of valency 3 or more are blended; setback data is supplied at fix time via `PK_BODY_fix_blends` with the `which_end` option; and if no setback information is supplied Parasolid will itself create a setback blend where it judges that the best surface. So Parasolid does perform automatic setback, though it does not use the literal term "autosetback" (that label is ACIS, reference 17); the failure-catalog entries above that read "setback / autosetback" should be read as Parasolid-auto-setback plus the ACIS-named feature.
- Capping is a real behavior, and advanced capping was an enhancement added at Parasolid V13.0 (the `update` option can disable it for V12.1-compatible rebuilds).
- `propagate` (blend continues over tangent edges, or past other blends where a 3-edge vertex would otherwise be invalid; not valid for cliff blends) and `vary` (variable rolling-ball blend, by default constrained to meet neighboring blends tangentially via a constant radius extension) are confirmed as described.

Net: no substantive correction was needed; the one nuance is the "autosetback" naming above. The overflow detail lives in the chapter titled "Edge Blend Overflows" (the q-solid mirror numbers it inconsistently across its doc sets).
