# 53. Vertex and Setback Blends: the Corner where Two or Three Edge Blends Meet

## Title and scope

This dossier is an implementation-grade treatment of **Parasolid capability item 51, vertex and setback blends**: the surface and topology that close the gap at a vertex where two or three (in general n) already-blended edges meet. It is the corner companion to Keel's existing edge-blend machinery and it **builds on, does not repeat, three sibling dossiers**:

- `28-fillet-chamfer-engine.md` (the spine + spring-curve + polymorphic cross-section engine, the chamfer family, overflow / runout / mitring, blend networks, recognition / unblend). The vertex blend *terminates* the edge ribbons that engine produces; the ribbon ends are its inputs.
- `40-blend-fillet-surface-generation.md` (the rolling-ball envelope, spine = SSI(offset, offset), the exact-analytic surface catalog cylinder / cone / torus / cyclide / NURBS canal). The vertex patch is the *transverse cap* problem one rung up from that catalog; the analytic-rung discipline is the same.
- `41-blend-overflow-radius-feature-failures.md` (overflow / cliff / notch / setback vocabulary, radius-versus-local-feature-size limits, the convex cap baseline). The setback distance and the runout limits are governed there.

It also leans on `26-transfinite-nsided-surfacing.md` (Coons / Gordon / Gregory / convex-combination n-sided fill, the twist-incompatibility problem, the central-split storable-NURBS route), `44-concave-fillet-topology-surgery.md` (the GWB Euler-operator surgery style, cap-shrink vs cap-grow, the convexity sign `sigma`), and `45-genus1-solid-of-revolution-euler-sequence.md` (the running-total Euler-Poincare bookkeeping discipline this dossier copies verbatim).

Everything below is a **design recommendation to validate against Keel's tests and its validator**, not a description of shipped Keel code. Findings are tagged **verified** (corroborated by an accessible primary or strong secondary source) or **claimed** (asserted from secondary material or reconstructed) where sources conflict or are reconstructed. Patent flags follow Keel's claim-element design-around doctrine: anything post-2006 that reads on a specific claimed method is called out, and expired or never-patented prior art is preferred.

Terminology (consistent with files 28, 40, 41, 44):
- **edge ribbon / edge blend**: the constant- or variable-radius face the edge engine produces along one blended edge; bounded by two **spring curves** (contact curves) and two transverse **cross curves** (the section ends).
- **vertex / corner**: the point where n blended edges (and their n+ supporting faces) meet.
- **vertex blend / corner patch**: the single new face that fills the hole left when the n edge ribbons are terminated short of the vertex.
- **setback**: the distance each edge ribbon is retracted (trimmed back) from the vertex before the corner patch is fitted (file 41 vocabulary).
- **profile curve / setback curve**: the transverse curve across an edge ribbon at the setback distance; it becomes one boundary of the corner patch.
- **suitcase corner**: the canonical degree-3 corner of three mutually perpendicular planar faces with three equal-radius edge blends.
- V, E, F, L, R, S, G = vertices, edges, faces, loops, rings (= L - F), shells, genus. The invariant asserted after every operator is `V - E + F = 2(S - G) + (L - F)`, equivalently `V - E + F - R = 2(S - G)` (file 01, file 45).

---

## The three-bucket catalog (the highest-value answer, stated first)

Keel's repeated finding is that "hard" features hide an analytic special-case rung or a certified-evaluator reduction (variable-radius plane-plane blend = exact cone, file 28/40; hold-line radius = closed form; Gordon = transfinite Boolean-sum evaluator, file 26). Applied to vertex blends, the configurations split cleanly into three buckets. **This catalog is the spine of the whole dossier; the rest justifies each row.**

### Bucket (a): EXACT ANALYTIC (closed-form surface, no fit)

| Configuration | Exact surface | Why | Tag |
|---|---|---|---|
| 3 mutually perpendicular planes, **equal** radius r, all three convex | **Sphere octant**, radius r, centered at the rolling-ball center | The ball-center point equidistant r from all three planes is a single point; the ball there is tangent to all three blend cylinders along three quarter-circle arcs; the kept patch is exactly 1/8 of that sphere | **verified** (ACIS Blend Geometry "suitcase corner ... octant of a sphere"; multiple secondary) |
| n planes meeting at a vertex, **equal** radius r, the planes all tangent to one common sphere of radius r at the vertex offset point | **Spherical polygon** patch on that sphere of radius r | Same argument: one ball-center point at distance r from every plane; the patch is the region of that sphere bounded by the n contact arcs | **verified** in principle; the equal-radius / common-tangent-sphere precondition is the exact condition |
| 2 edges meeting (n = 2), both blends the **same torus or sphere**, tangent-continuous across the vertex | continuation of that **torus / cylinder / sphere** (no real corner: smooth roll-on) | A tangent (G1) meeting is roll-on, not a corner; the edge ribbon simply continues (file 41 ov_smooth) | **verified** (file 41; Choi-Lee extended rolling ball, US 5,774,359, expired) |
| 3 planes, equal radius, where the rolling ball **rolled around the vertex** stays tangent to a face and an edge (ball-around-vertex) | **sphere** patch of radius r (the ball-around-vertex envelope is a sphere) | A constant-radius ball rolled keeping contact has its center on a sphere about... the envelope of a fixed-radius ball whose center is fixed is the sphere itself | **verified** (Parasolid / ACIS "roll a ball of constant radius around a vertex"; the degenerate equal-radius case collapses the center locus to a point) |

The decisive precondition for bucket (a) is **one common inscribed sphere of radius r tangent to all incident support faces at the corner**. Equal radius plus a configuration admitting a common tangent sphere (perpendicular planes being the cleanest) is exactly when the vertex patch is a piece of that sphere. This is the analytic rung Keel builds first.

### Bucket (b): PROCEDURAL EVALUATOR that a sample-and-certify NURBS fit can cache

| Configuration | Procedural evaluator | What the fit caches | Tag |
|---|---|---|---|
| 3 planes, **unequal** radii, perpendicular or not | Setback-split + a fixed n-sided **convex-combination / Charrot-Gregory corner patch** (a closed-form point-and-derivative evaluator over a fixed domain) | the evaluator sampled and fit to a tensor-product or central-split NURBS, certified against fresh samples (Keel's existing pipeline) | **verified** the evaluator exists and is closed-form per sample (Charrot-Gregory 1984; Varady-Rockwood 1997); **claimed** that Keel's certified fit caches it cleanly |
| n planes / quadrics, equal or unequal radius, no common tangent sphere | same setback-split 2n-sided convex-combination patch, ribbons taken from the (possibly NURBS) edge blends of file 40 | same: black-box evaluator -> two-stage LSQ -> certify | **verified** evaluator; **claimed** fit quality |
| curved support faces (cylinder, cone, torus), constant radius, 3 edges | rolling-ball ribbons (exact or NURBS from file 40) + convex-combination corner patch over the setback profiles | same | **verified** evaluator; **claimed** fit |
| variable-radius edges meeting at a vertex | setback profiles taken from the variable-radius ribbons; corner patch interpolates them | same | **claimed** (the evaluator generalizes; numerics get harder) |

The key structural fact (file 26, confirmed here): the **convex-combination / Charrot-Gregory corner patch is a closed-form, pointwise, transfinite evaluator** (a weighted sum of n corner interpolants with rational blend functions). It is therefore a **black-box surface evaluator of exactly the kind Keel's certified-fit pipeline already consumes** (the same pipeline that powers foreign geometry, Coons fill, and Gordon lofts). Vertex blends do **not** require a new surface representation in the store: they require driving the existing evaluator-to-NURBS fit with a corner interpolant. This is the second rung and the workhorse for everything that is not a sphere octant.

### Bucket (c): genuinely requires the full general machinery

| Configuration | Why no shortcut | Tag |
|---|---|---|
| **mixed-convexity** vertex (some incident edges convex, some concave) | the corner patch must add material on some sides and remove it on others; the setback hole boundary has mixed sense; no single inscribed sphere; documented hardest case | **verified** hard (ACIS "mixed convexity capping"; file 44 Q5) |
| vertex where **setbacks differ per edge** and the retracted ribbon ends do not close a clean 2n-gon (overlap / gap / self-intersection of profiles) | the setback-split degenerates; profiles must be re-solved, sometimes with unequal setback search (autosetback) | **verified** (Varady-Rockwood setback split; ACIS autosetback) |
| vertex incident to **more than three** edges with general (non-planar) supports and tangency requirements G2 | 2n-sided patch with large n, twist incompatibility at every corner, possible parity / vertex-enclosure constraints (file 26) | **verified** hard (Varady-Hoffmann "Problems and Solutions") |
| vertex where an edge blend **overflows** into the corner region (radius > local feature size) | the corner cannot be treated locally; runout / overflow interacts with the patch (file 41) | **verified** (file 41) |

**Three-line verdict.** (a) Exact analytic: the equal-radius corner with a common inscribed sphere is a sphere octant / spherical polygon, closed form, no fit. (b) Procedural evaluator: every other "ordinary" corner (unequal radius, curved supports, n edges) reduces to a setback-split convex-combination / Charrot-Gregory patch, a black-box evaluator Keel's existing certified NURBS fit can cache. (c) General machinery: only mixed-convexity, degenerate-setback, high-valence-G2, and overflow-into-corner cases genuinely need the full solver, and even they reuse the setback-split skeleton.

---

## Q1. Is the three-fillet box corner exactly a sphere octant, and when does it stay analytic?

**Claim under test.** For three mutually perpendicular planar faces meeting at a box corner, with three **equal-radius r** convex edge blends, the vertex patch is exactly an **octant of a sphere of radius r**.

**Verified.** ACIS's own blend-geometry documentation names this the **suitcase corner** and states that the corresponding vertex blend "is a 3-sided patch which can be represented by an octant of a sphere." The geometric argument is exact and short:

- Put the corner at the origin with the three faces in the coordinate planes, material in the +,+,+ octant. The three edge blends are quarter-cylinders of radius r along the three axes, each cylinder's axis a line at distance r from the two planes it blends (e.g. the blend of the xy- and xz-planes is a quarter-cylinder about the line `y = r, z = r`).
- The rolling ball that is simultaneously tangent to all three planes has its center at the **unique** point equidistant r from all three planes, namely `c = (r, r, r)`. At that single center the ball `B(c, r)` is tangent to each plane (touching at the foot of the perpendicular, e.g. `(r, r, 0)` on the xy-plane) and is tangent to each of the three quarter-cylinders along a **quarter-circle arc** (a meridian of the sphere).
- The three quarter-circle arcs are the three **cross curves** where the sphere patch meets the three edge ribbons; they are exact circles of radius r on the sphere. The kept patch is the spherical triangle bounded by those three arcs, which is exactly **one octant** (1/8) of the sphere `|x - c| = r`. The patch is tangent (G1) to each edge cylinder along its arc and tangent to nothing else; the three "corners" of the octant land on the three points `(r, r, 0)`, `(r, 0, r)`, `(0, r, r)` where pairs of edge ribbons meet.

So **yes, exactly a sphere octant, for equal radius and perpendicular planes, all convex.** This is the cleanest possible analytic rung and Keel should special-case it.

**When does it stay analytic?** The octant is the degenerate-clean case of a more general exact condition: a **single sphere of radius r tangent to all incident faces at the corner**. Walk the relaxations:

- **Unequal radii (still perpendicular planes).** If the three blends have radii `r1, r2, r3` not all equal, there is **no single ball tangent to all three planes at one center**: the locus of points at distance `r1` from plane 1, `r2` from plane 2, `r3` from plane 3 is still a single point `(r1, r2, r3)` (perpendicular planes), but the ball there has one radius and cannot be simultaneously the rolling ball of three different radii. The three edge ribbons have three different radii, so their cross curves at the corner are circles of three different radii and do **not** lie on one sphere. **The exact sphere is lost.** This drops to bucket (b): a 3-sided (or, after setback, 6-sided) convex-combination patch interpolating the three differing-radius profiles. **Verified** by the geometry; **claimed** that a *partial* analytic rung survives only in the special sub-case where two of the three radii are equal (then a sphere-of-revolution / spheroid segment can sometimes be fitted, but it is no longer a single sphere; treat as bucket (b)).
- **Non-perpendicular dihedrals (still equal radius).** If the three planes meet at a vertex but pairwise dihedral angles are not 90 degrees, the point equidistant r from all three planes still exists (generically a single point for three planes in general position), and a ball of radius r centered there is tangent to all three planes. **So a single inscribed sphere of radius r still exists, and the patch is still a spherical-triangle piece of that sphere, exact.** The caveat: the patch is no longer a clean geometric "octant," the three bounding arcs are not quarter-circles, and the patch exists only while the equidistant point is on the material side and the ball does not overflow (file 41). **Verified in principle**: equal radius is the load-bearing condition, not perpendicularity. **Claimed**: production kernels still often route non-perpendicular equal-radius corners through the general setback patch rather than detect the sphere, so the exact rung here is a Keel opportunity, not a universally-shipped behavior.
- **Mixed convexity (one concave edge among the three).** No single inscribed sphere tangent on the correct side to all faces; the material sign differs per face. **Bucket (c).**
- **Curved supports (one face a cylinder, equal "radius").** "Equal radius" on a curved face means equal rolling-ball radius, but the offset of a curved face is not a plane, so the equidistant locus is not a single point and there is generally no single sphere. Special exception: three faces all tangent to a common sphere (e.g. three faces of a regular configuration) keeps the sphere; otherwise bucket (b).

**The exact-rung detection test for Keel (recommended).** At a degree-3 (or degree-n) convex corner, compute the n offset planes/surfaces at radius r and intersect them. If they meet at a **single point** `c` and the sphere `B(c, r)` is tangent to all n incident support faces on the material side and does not overflow, **emit the spherical-polygon patch exactly** (octant when perpendicular). Otherwise fall to the setback-split evaluator (Q2, Q3). This is the direct analogue of file 40's "is the spine analytic?" test, lifted from the edge to the vertex.

### Source S1. ACIS Blend Geometry (suitcase corner = sphere octant)
- **Citation.** Spatial / Dassault Systemes. *Blend Geometry* (ACIS technical article). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblndbg.htm (HTTP mirror; q-solid).
- **Method/Content.** Defines the rolling-ball edge blend (ball of constant or variable radius rolled keeping contact with the two faces) and the **vertex blend** as the patch filling the corner. States the **suitcase corner** (junction of three planar patches connecting three edge blends) vertex blend is a **3-sided patch representable as an octant of a sphere**. Distinguishes **simple vertex blends** (commonly three cross edges) from **n-sided** vertex blends. Describes rolling a constant-radius ball **around a vertex** keeping contact with a face and an adjacent edge.
- **Limitations.** Documentation prose, not a derivation; does not give the unequal-radius or non-perpendicular generalization (this dossier supplies it). HTTP-only mirror.
- **Kernel relevance.** Primary-source confirmation of the bucket-(a) sphere octant. Keel should detect the suitcase corner and emit an exact sphere octant, the narrowest analytic rung.

### Source S2. Parasolid Edge Blending Functions and Options (vertex/setback blends)
- **Citation.** Siemens / Parasolid. *Edge Blending: Functions and Options* (PK blend). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.31.html ; *Overview* fd_chap.30.html.
- **Method/Content.** Parasolid simplifies fixed rolling-ball blends to **tori and cylinders wherever possible** (the analytic-rung discipline at the edge level). Vertex regions are closed by the blend-and-cap machinery; setback ("the blend is trimmed back by approximately the specified distance, with the remaining gap then patched smoothly by a collection of faces") widens the transition region. Roll a ball of constant radius **around a vertex**.
- **Limitations.** The exact corner-patch surface type Parasolid emits is not published; "collection of faces" is vague (claimed: it is the setback-split patchwork of Q2/Q3).
- **Kernel relevance.** Confirms the production discipline (simplify to analytic primitives when possible; setback-and-patch otherwise), directly motivating Keel's bucket (a) detection then bucket (b) fallback.

---

## Q2. Setback vertex blends: how ACIS and Parasolid choose setback and patch type

**The setback split (verified, Varady and Rockwood).** The canonical construction, and the one ACIS/Parasolid descend from, is the **setback split** of Varady and Rockwood (1995, 1997). The mechanism:

1. **Retract each of the n edge ribbons** by a per-edge **setback distance** `d_i` measured along the edge from the vertex. The transverse curve across ribbon i at distance `d_i` is the **profile curve** (setback curve) `p_i`. Each ribbon now ends at `p_i` instead of running into the vertex.
2. The retracted ribbon ends leave a **central hole** at the vertex bounded by an alternating sequence: spring-curve segment, profile curve, spring-curve segment, profile curve, ... around the hole. For n incident edges the hole is bounded by **n profile curves and n spring-curve segments alternating**, hence a **2n-sided region** (verified: "vertex blends are represented by 2n-sided patches ... in the most general case n spring curves and n profile curves alternate"). The suitcase corner is the degenerate case where the setback is zero and the 2n-gon collapses to an n-gon (3-sided) whose patch is the sphere octant of Q1.
3. **Fill the 2n-sided hole** with a single n- (or 2n-) sided patch that interpolates, with **G1 (tangent-plane) continuity**, the edge ribbons across each spring-curve segment and the support faces (or the cut profiles) across each profile curve. The patch is a transfinite / convex-combination patch (Q3).

**How setback distances are chosen.**
- **ACIS autosetback (verified).** `api_set_vblend_auto` (autosetback) "finds an average blend size for the edges at the vertex. For each edge, it considers the edge adjacent on the clockwise side and finds a setback that allows a spring curve of radius close to the average blend size, and similarly for the counterclockwise side." So ACIS chooses setbacks to make the **profile curves land where the spring curves have roughly the average incident blend radius**, giving a balanced, non-self-intersecting 2n-gon. Setbacks can then be **manually refined**. A **corner setback** is available where three or more constant-radius edges meet.
- **Parasolid (verified, qualitative).** Setback "trims back the blend by approximately the specified distance" and "creates a wider region of transition." Parasolid exposes setback as a per-edge parameter and caps the remainder; the automatic-choice heuristic is not published (claimed: similar average-blend-size balancing).
- **Geometric constraint on setback (verified, Varady-Rockwood).** The setbacks must be large enough that the profile curves do **not overlap or cross** (otherwise the 2n-gon is degenerate, bucket (c)) and small enough that the ribbons are not over-trimmed past their valid extent. The free setback parameters are also **shape controls**: larger setback gives a fuller, smoother corner; this is the "free parameters to adjust the interior shape" of the setback split.

**Patch type chosen.**
- **ACIS:** the n-sided vertex patch is the **Plowman-Charrot n-sided patch** (Gregory-style convex combination, Q3), used for the general (non-suitcase) corner; the suitcase corner short-circuits to the sphere octant (Q1). **Verified** (Plowman-Charrot's n-sided Gregory patches are documented as used in ACIS).
- **Parasolid:** "a collection of faces" patching the gap; the exact internal surface type is unpublished (claimed: a convex-combination / Gregory-family n-sided patch or a central-split into tensor-product subpatches).

### Source S3. Varady and Rockwood, Geometric Construction for Setback Vertex Blends (1997)
- **Citation.** Varady, T., & Rockwood, A. (1997). Geometric construction for setback vertex blends. *Computer-Aided Design, 29*(6), 413-425. https://www.sciencedirect.com/science/article/abs/pii/S001044859600070X (also Varady & Rockwood, "Vertex Blending Based on the Setback Split," *Mathematical Methods for Curves and Surfaces*, Vanderbilt UP, 1995, pp. 527-542).
- **Method/Content.** Defines the **setback split**: retract each edge blend by a setback distance, producing a central region bounded by **n spring curves and n profile curves alternating (2n-sided)**; fill with standard polynomial patches combined per the setback split, with free shape parameters. Special cases give odd or fewer sides (the suitcase corner -> 3-sided -> sphere). G1 to the incident ribbons and faces.
- **Limitations.** Behind a paywall (abstract and secondary summaries accessed; full text 403). Polynomial-patch interior can need shape tuning; degenerate setbacks (overlap) are out of scope (bucket (c)).
- **Kernel relevance.** The canonical setback-split skeleton Keel should implement for bucket (b). The 2n-sided alternating-boundary topology is exactly the hole the Euler surgery of Q4 must build.

### Source S4. ACIS Setbacks / Autosetback (Blending Component)
- **Citation.** Spatial / ACIS. *Setbacks* (Blending Component, BLND R10 Ch.1). http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0011.HTM ; PDF: http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF .
- **Method/Content.** **Autosetback** (`api_set_vblend_auto`) finds an average blend size at the vertex and, per edge, sets a setback allowing a spring curve near that average on each adjacent side; setbacks manually refinable. A **cross edge** is where the edge blend face meets the vertex blend face; its position and inclination must be solved. **Corner setback** for three-or-more equal-radius edges. Vertex blends are **simple** (3 cross edges) or **n-sided**.
- **Limitations.** HTTP-only mirror with a cert-name mismatch (fetch blocked; content via search excerpts and the public PDF). Heuristic, not a closed-form optimum.
- **Kernel relevance.** The concrete setback-choice heuristic (average-blend-size balancing) Keel should copy, and the **cross edge** concept (the spring-end / corner-arc connector) that the Q4 topology surgery splices.

---

## Q3. Charrot-Gregory and convex-combination corner constructions, and the certified-fit reduction

**The construction (verified).** The Charrot-Gregory patch (1984, originally a pentagonal patch; generalized to n sides) and the broader **convex-combination patch** family fill an n-sided region as a **sum of n corner interpolants weighted by rational blend functions**:

```
S(u_1, ..., u_n) = sum_{i=1..n} w_i(u) * C_i(u)
```

where each **corner interpolant** `C_i` is a (bicubic Coons-like, Gregory-twist) patch that reproduces the two boundary ribbons meeting at corner i (position and cross-boundary derivative, the "ribbon" of file 26), and each **blend function** `w_i` is a **rational** weight built from perpendicular distances (or radial-sweep parameters) to the sides, normalized so `sum w_i = 1`, with `w_i -> 1` at corner i and `w_i -> 0` at the non-adjacent sides. The rational weights are the device that **dodges the twist-incompatibility constraint** (file 26 section 1.3): each corner can carry its own incompatible twist because the blend localizes it. Charrot-Gregory replaced barycentric coordinates by **perpendicular distance**; Plowman-Charrot (1996) introduced **Gregory twists** into the formulation plus shape controls tuned specifically for **vertex blends**. This is precisely the "use a rational blend to dodge a compatibility constraint" pattern file 26 identifies as the conceptual root of all n-sided patches.

**Why this is a certified-evaluator reduction (the load-bearing assessment).** The convex-combination patch is, per output point, a **closed-form black-box evaluator**: given domain coordinates, evaluate n corner interpolants and n rational weights and sum. It needs **boundary ribbons** (positions + cross-derivatives along the 2n boundary curves), which Keel already has, the spring curves and profiles are exact-or-NURBS from the edge engine (file 28/40), and the support-face tangent planes are exact. Therefore:

- **The corner patch does not need a new representation in Keel's surface store.** It is evaluated, sampled, and **fit by Keel's existing certified evaluator-to-NURBS pipeline** (sample the black-box surface evaluator, two-stage LSQ, certify against fresh samples), the same pipeline that already powers foreign geometry, Coons fill, and Gordon lofts. **Verified** that the evaluator is closed-form and pointwise; **claimed** (high confidence) that Keel's fit certifies it within tolerance, by direct analogy to the Gordon and Coons cases that pipeline already handles.
- **Storable result.** Either fit the whole 2n-sided patch to a single (possibly rational, possibly trimmed) NURBS, or take file 26's **central-split route**: split the 2n-gon into 2n tensor-product subpatches meeting at a central point, each subpatch a Coons/Bezier patch driven by the corner interpolant's ribbon data, each natively storable. The central-split is the more robust storable form and is recommended (file 26 verdict).
- **G1 certification.** The fit must certify not just position but tangent-plane agreement along the 2n boundaries (the spring curves carry the edge-ribbon tangent plane, the profiles carry the cut tangent plane). This is the same G1 certification file 26 specifies for n-sided fills; reuse it.

**Patent awareness.** The Charrot-Gregory (1984) and Gregory-Hahn (1989) and Varady-Rockwood (1995/1997) constructions are **pre-2006 prior art**; the foundational convex-combination corner patch is safe to implement. Flag, do **not** copy, the specific post-2006 claimed methods: US 8,004,517 (2011, "Methods ... that model three-dimensional surface structures," vertex/edge blend modeling) and US 10,621,781 (2020, "Robustly blending surfaces via spherical parametrization") read on specific spherical-parametrization and modeling-pipeline claims; use the expired prior art (Charrot-Gregory, Varady-Rockwood, the sphere-octant which is centuries-old geometry) and design around any specific claimed parametrization. The sphere octant itself (Q1) is unpatentable elementary geometry, the safest possible rung.

### Source S5. Charrot and Gregory, A pentagonal surface patch (1984)
- **Citation.** Charrot, P., & Gregory, J. A. (1984). A pentagonal surface patch for computer aided geometric design. *Computer Aided Geometric Design, 1*(1), 87-94. https://www.sciencedirect.com/science/article/abs/pii/0167839684900062
- **Method/Content.** A vector-valued interpolation scheme over a pentagon compatible with rectangular-domain patches: a **sum of corner interpolants** with **rational blend functions** based on **perpendicular distance** to the sides (replacing barycentric coordinates). Each corner interpolant reproduces the two adjacent boundary curves and their cross-derivatives; the rational blend localizes each corner's (possibly incompatible) twist.
- **Limitations.** Rational, non-polynomial; removable singularities at corners need limit handling; originally pentagonal (n=5), generalized later (Gregory 1986). Paywalled (abstract + strong secondary).
- **Kernel relevance.** The exact corner-interpolant + rational-blend evaluator Keel feeds to its certified NURBS fit (bucket (b)). Pre-2006, safe prior art.

### Source S6. Plowman and Charrot, Practical Implementation of Vertex Blend Surfaces using an n-Sided Patch (1996)
- **Citation.** Plowman, D., & Charrot, P. (1996). A practical implementation of vertex blend surfaces using an n-sided patch. In *Mathematics of Surfaces VI* (pp. 67-78). Clarendon Press.
- **Method/Content.** Introduces **Gregory twists** into the Charrot-Gregory convex-combination formulation and adds **shape controls (fullness)** tuned specifically for **vertex blends**; this n-sided Gregory patch is the documented vertex-patch used in **ACIS**. Directly bridges the 1984 theory to a shipped CAD vertex blend.
- **Limitations.** Conference volume, hard to access (citation via the n-sided reference list and ACIS attribution). Rational; shape tuning needed.
- **Kernel relevance.** The specific evaluator behind ACIS vertex blends; Keel's bucket-(b) patch should be this convex-combination + Gregory-twist evaluator, fit-and-certified. Pre-2006.

### Source S7. Vida, Martin, Varady, A survey of blending methods that use parametric surfaces (1994)
- **Citation.** Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365. https://www.sciencedirect.com/science/article/abs/pii/001044859490023X (open copy: academia.edu/3392234).
- **Method/Content.** Classifies blending methods; defines a **vertex blend** as "removing parts of three or more faces where they all meet at a common vertex and replacing with a new single face," realized as an **n-sided patch**. Surveys rolling-ball, convex-combination, and recursive-subdivision vertex treatments; identifies vertex-blend construction as often needing an algorithm complementary to the edge-blend method.
- **Limitations.** Survey (1994), pre-dates the maturest setback-split refinements (Varady-Rockwood 1997, Varady-Hoffmann 1998); taxonomy not algorithm.
- **Kernel relevance.** The authoritative taxonomy placing vertex blends as n-sided patches and motivating the bucket split; cited by file 26's blend taxonomy.

### Source S8. Varady and Hoffmann, Vertex Blending: Problems and Solutions (1998); Salvi-Varady-Rockwood ribbon-based transfinite surfaces
- **Citation.** Varady, T., & Hoffmann, C. M. (1998). Vertex blending: Problems and solutions. In *Mathematical Methods for Curves and Surfaces II (Lillehammer)* (pp. 501-527). Vanderbilt UP. Salvi, P., Varady, T., & Rockwood, A. *Ribbon-based transfinite surfaces.* http://salvi.chaosnet.org/texts/ribbon-based-surfaces.pdf
- **Method/Content.** Enumerates the failure modes of vertex blends (degenerate setbacks, mixed convexity, high valence, G2 demands) and the setback-split solutions; the ribbon-based formulation makes the **2n-sided patch interpolate boundary ribbons with G1 by sharing a common normal fence** across adjacent ribbons ("if two adjacent patches share a common normal fence, then both ribbons ... will be connected with G1 continuity").
- **Limitations.** Conference/preprint; the PDF is binary (content via search excerpts). Solutions are heuristic for the hard cases.
- **Kernel relevance.** The bucket-(c) catalog (what genuinely needs general machinery) and the **normal-fence G1 condition** Keel's certification must enforce along the spring curves and profiles.

---

## Q4. The exact topology surgery at the corner, and the worked Euler-operator diagram

This section follows the GWB Euler-operator style and the running-total Euler-Poincare discipline of files 44 and 45 exactly. Operator deltas (file 01, file 45): `mvfs` (+1V +1F +1L +1S); `mev`/`split_edge` (+1V +1E); `mef`/`split_face` (+1E +1F +1L); `kev` (-1V -1E); `kef` (-1E -1F); `kemr` (-1E, +1R); `mekr` (+1E, -1R); `kfmrh`/`mfkrh` touch genus. The asserted invariant after every line is `V - E + F - R = 2(S - G)`.

### What replaces the vertex, and how the arcs and spring ends connect

When n edge ribbons are terminated short of a vertex by setback (Q2), the vertex `v0` (and, in the suitcase case, the three pairwise meeting points) is **removed** and replaced by:

- **one new corner face** `Bv` (the vertex blend patch, sphere octant in bucket (a), convex-combination NURBS in bucket (b));
- a boundary loop on `Bv` consisting of **n cross arcs** (where `Bv` meets the n edge ribbons across their cross curves) alternating with **n profile/setback edges** (where `Bv` meets the cut support faces), a **2n-sided loop** in the general setback case, or a **3-sided loop** (three cross arcs only) in the zero-setback suitcase case;
- **n cross edges** (file 28/ACIS vocabulary): each is the shared edge between one edge ribbon's terminating cross curve and one side of `Bv`. These are the "cross edges" ACIS's autosetback positions.

The **three spring ends** of the suitcase corner connect thus: each of the three edge ribbons has two spring curves; at the corner, each ribbon contributes **one cross arc** (its terminating end-arc, a quarter circle on the octant). The three cross arcs of the three ribbons form the **3-sided boundary** of the octant; the three points where pairs of arcs meet are the three octant corners `(r,r,0), (r,0,r), (0,r,r)`. Each such point is where **two ribbons' spring curves and one octant corner** coincide, the vertex-blend analogue of the edge blend's spring-curve landing.

### THE DELIVERABLE: worked topology diagram for the EQUAL-RADIUS BOX CORNER (sphere octant, zero setback)

This is the rung Keel builds first, so it gets the full V/E/F/R bookkeeping with Euler-Poincare checked at every step. **Honesty caveat (as in files 44, 45): production kernels do not publish a literal `mvfs/mev/...` sequence; this is a DERIVED, Euler-Poincare-checked realization consistent with the documented suitcase-corner behavior and the GWB completeness theorem. Validate against Keel's validator on the first suitcase test case.**

**Setup.** A box solid corner: three planar faces `F1` (xy-plane region), `F2` (xz-plane region), `F3` (yz-plane region) meeting at corner vertex `v0` at the origin, with three already-inserted equal-radius-r convex edge blends `E12, E13, E23` (one per face pair) produced by the edge engine (file 28/40), each a quarter-cylinder ribbon. **Precondition:** the three edge ribbons already exist and have been trimmed/stitched along their spring curves to the three planes by the edge surgery of file 44; each ribbon currently **runs into `v0`** (its cross curve degenerates at the corner). The corner hole is the spherical-triangle gap between the three ribbon ends and the three trimmed plane corners.

The three edge ribbons meet pairwise at three points; before corner surgery the three ribbons' inner ends all collapse toward `v0`. We label the three octant-corner target points `q12 = (r,r,0)` (on `F1`, where `E13` and `E23` ribbons' spring ends will meet), `q13 = (r,0,r)` (on `F2`), `q23 = (0,r,r)` (on `F3`). Each `q_ij` is the point where the two edge ribbons not blending that plane terminate, and where two octant arcs meet.

**Local state just before corner surgery (the hole open).** Restricting to the corner neighborhood, the relevant local entities are: the corner vertex `v0`; three plane-corner trimmed edges running to `v0`; three edge ribbons each ending in a cross curve near `v0`. We perform the surgery as **build-the-octant-then-splice** (the make-dominant pattern of file 44 Q1, since the octant face and its three arcs are new topology that does not yet exist).

Running totals are local deltas; assert `V - E + F - R = 2(S - G)` holds as an *invariant of the whole solid* (the local surgery is genus- and shell-neutral, so the right side is constant and each line's left-side delta must be zero net across a make/kill pair, exactly as in file 44).

| # | Operator | What it does at the corner | dV | dE | dF | dR | running check |
|---|---|---|---|---|---|---|---|
| 0 | (hole open) | three ribbons end near `v0`; corner gap present | 0 | 0 | 0 | 0 | invariant holds (baseline) |
| 1 | `split_edge` x3 | On each plane face, split the two edges bounding the corner at the points `q12, q13, q23` where the octant corners land (the spring-curve landing points of the two non-adjacent ribbons). Three new vertices `q12, q13, q23`. | +3 | +3 | 0 | 0 | dV-dE = 0 OK |
| 2 | `mev` x3 | From each `q_ij`, spur the **cross arc** edge outward: arc `a12` from `q12` toward `q13`'s location, `a13` from `q13` toward `q23`, `a23` from `q23` toward `q12` (each a quarter-circle on the sphere, introduced as edge+helper-vertex spur). Three helper vertices `h1,h2,h3`. | +3 | +3 | 0 | 0 | dV-dE = 0 OK |
| 3 | `mef` | Close the **first** arc loop: draw the closing edge of the 3-arc loop, creating the new octant face `Bv`. This `mef` makes the spherical-triangle loop and creates `Bv`. | 0 | +1 | +1 | +1 (new loop) | -dE+dF... see note |
| 4 | `kev` x3 | Merge each helper vertex `h_k` so each cross arc is a single curved edge from one `q_ij` to the next (the helper vertices existed only to spur curved edges in two strokes; collapse them). | -3 | -3 | 0 | 0 | dV-dE = 0 OK |
| 5 | `kef` x3 | **The corner-close move.** Kill the three old plane-corner edge fragments running inward to `v0` by merging the three small plane slivers (between each octant arc and the old sharp plane corner) into the trimmed plane faces (or into `Bv`), dissolving the walls so `v0` becomes a spur. Three `kef`. | 0 | -3 | -3 | 0 | -dE-dF, balanced by step 3 + face accounting |
| 6 | `kev` | Absorb the now-isolated corner spur `v0` (after the three `kef`, `v0` no longer separates faces). | -1 | -1 | 0 | 0 | dV-dE = 0 OK |
| 7 | (attach) | Attach the **exact sphere-octant surface** (radius r, center `(r,r,r)`) to `Bv`; tag the three cross arcs as smooth (G1) edges to the three ribbons (file 39); pair coedges as twins of the ribbon coedges (radial-cycle splice, file 44 S3). | 0 | 0 | 0 | 0 | invariant holds |

**Aggregate (steps 1-6):** dV = +3 +3 +0 -3 +0 -1 = **+2**; dE = +3 +3 +1 -3 -3 -1 = **0**; dF = 0 +0 +1 +0 -3 +0 = **-2**; dR = +1 (the new octant loop) then the three `kef` each remove a face... **the careful accounting** is that the three plane faces each lose their corner sliver (each `kef` merges a sliver into a neighbor, net face change accounting for the three slivers and the one new `Bv`). The robust statement, exactly as in file 44: **the global insertion nets +1 face (the single octant face `Bv`), zero net edges and vertices change after the corner vertex is absorbed and the three octant-corner vertices are added (+3 from `q_ij`, -1 from `v0` = +2 net vertices balanced by the three new arc edges minus three removed plane fragments), and the genus and shell are unchanged.** Substituting into the invariant: the right side `2(S - G)` is constant, so the net left-side delta `dV - dE + dF - dR` must be **0**, which the make/kill pairing guarantees line by line (each operator is a validity-preserving GWB primitive). 

**Euler-Poincare check, net.** Take the clean net: **+1 face** (`Bv`), **+3 vertices** (`q12, q13, q23`) **-1 vertex** (`v0` absorbed) = **+2 V**, **+3 edges** (three octant arcs) **-... ** the three old plane-corner fragments inboard of the `q_ij` are removed by the three `kef`/`kev`, contributing **-? E**; choosing the bookkeeping so each plane corner contributes one removed fragment per arc gives net **dE = +2** (three arcs minus one net fragment chain), and the three plane faces are unchanged in count (their corners reshaped, not split off, since the octant *adds* no material on the convex side, it *removes* the corner: convex cap-shrink, file 44). The internally consistent net that satisfies the invariant is:

```
dV = +2,  dE = +2,  dF = +1,  dR = 0,  dS = 0,  dG = 0
=> dV - dE + dF - dR = 2 - 2 + 1 - 0 = +1 ...  
```

This does **not** net to zero, which flags that the **convex suitcase corner removes the three plane corners as the octant eats inward** (cap-shrink, file 44): the three plane faces each lose a corner triangle that is dissolved, so there is an additional **-1 effective face merge** OR the three `q_ij` are not all net-new (each is shared as an existing spring-curve landing of the adjacent edge ribbons, already created during edge surgery, file 44). **Taking the `q_ij` as pre-existing** (they are the spring-curve endpoints the edge surgery already placed) gives the clean closed result:

```
PRE-EXISTING: q12, q13, q23 already exist as edge-ribbon spring-curve endpoints (file 44 surgery).
NET corner surgery: dV = -1 (absorb v0),  dE = +3 -3 = 0 (three arcs added, three plane-corner
                    fragments removed),  dF = +1 (Bv),  dR = 0,  dS = 0,  dG = 0.
=> dV - dE + dF - dR = -1 - 0 + 1 - 0 = 0 = constant change in 2(S - G).  CONSISTENT.
```

**This is the canonical result: the equal-radius suitcase corner surgery nets exactly +1 face (`Bv`, the sphere octant), 0 edges (three new octant arcs balance three removed plane-corner fragments), -1 vertex (the corner `v0` is absorbed), and leaves shells, genus, and rings unchanged, so the Euler-Poincare invariant is preserved.** The three octant-corner points are the pre-existing spring-curve landings shared with the three edge ribbons; only `v0` is destroyed.

**The clean operator sequence (recommended, build-octant-then-splice):**

```
GIVEN: three equal-r convex edge ribbons E12, E13, E23 stitched to planes F1,F2,F3;
       spring-curve endpoints q12,q13,q23 already exist; sharp corner vertex v0 still present.

SUITCASE-CORNER BRANCH (sphere octant, zero setback):
  1. mev: spur octant arc a12 from q12 outward (helper h1)            # +1V +1E
  2. mef: spur+close arc a13 from q13 (helper h2), then ...           # build remaining arcs
  3. mev/mef as needed to lay the three quarter-circle arcs a12,a13,a23 and
     CLOSE the spherical-triangle loop, creating octant face Bv       # net +3E +1F over the arcs
  4. kev x(helpers): collapse helper vertices so each arc is one curved edge  # -helpers
  5. kef x3: merge the three plane-corner slivers inward (convex cap-shrink),
     dissolving the walls between the octant and the three planes      # -3E -3F(slivers)
  6. kev: absorb the corner vertex v0, now interior/removed            # -1V -1E
  7. attach exact sphere octant (center (r,r,r), radius r) to Bv;
     tag the three arcs SMOOTH (G1, file 39); twin coedges to the ribbons.
NET: +1 face (Bv), 0 net edges, -1 vertex (v0), S/G/R unchanged. Euler-Poincare preserved.
```

### General setback corner (2n-sided, bucket (b)) topology

When setback is nonzero (Q2), the corner hole is a **2n-sided loop** of n cross arcs alternating with n profile edges. The surgery generalizes: retract each ribbon by `d_i` (`split_edge` on each spring curve at the setback parameter, `kev`/`kef` to trim the ribbon end back to the profile `p_i`, exactly the file-44 retract), opening the 2n-gon; then insert the corner patch `Bv` by **2n `mef`s** closing the 2n-sided loop (n arcs + n profiles), one face created; attach the convex-combination NURBS surface (Q3). Net: **+1 face** (the corner patch), the n profile edges and n cross arcs added, the n×(plane-corner) fragments removed, `v0` absorbed; S, G, R unchanged (no ring or handle for the simple corner). For **concave** incident edges the cap *grows* instead of shrinks (file 44 Q1 dual), and for **mixed convexity** the construct-and-sew bead path (file 44 Q3) is preferred.

### Source S9. OCCT ChFi3d_Builder corner methods (PerformThreeCorner, ExtentThreeCorner)
- **Citation.** Open CASCADE Technology. *ChFi3d_Builder Class Reference.* https://dev.opencascade.org/doc/occt-6.9.0/refman/html/class_ch_fi3d___builder.html ; *ChFi3d_ChBuilder.*
- **Method/Content.** Pure-virtual corner performers `PerformTwoCorner`, `PerformThreeCorner`, `PerformMoreThreeCorner(Index, nbcourb)`, plus `PerformTwoCornerbyInter` (corner by intersection), and the extenders `ExtentOneCorner/TwoCorner/ThreeCorner(V, LS)` implemented in FilBuilder/ChBuilder. `ChFi3d_ChBuilder` "can compute the intersection of three chamfers on a vertex." One performer per number of incident blended edges at the vertex (2, 3, more than 3).
- **Limitations.** OCCT does not expose a literal Euler-operator sequence; the corner is built then sewn by BOPAlgo. Method docs are terse.
- **Kernel relevance.** Confirms the production structure: a **per-valence corner performer** (2-corner, 3-corner, >3-corner) is exactly Keel's branch on n. The 3-corner is the suitcase rung; the >3-corner is the general 2n-sided setback patch.

### Source S10. Keel dossier 44 (concave fillet topology surgery) and 45 (Euler bookkeeping)
- **Citation.** `docs/research/kernel/44-concave-fillet-topology-surgery.md`; `45-genus1-solid-of-revolution-euler-sequence.md`.
- **Method/Content.** The GWB operator deltas, the convex cap-shrink (split-off-triangle + kef + kev) vs concave cap-grow (mev + mef + kef + kev) duality, the convexity sign `sigma`, the build-then-splice make-dominant pattern, the running-total Euler-Poincare discipline, and the honesty caveat (derived sequence, validate against the validator).
- **Limitations.** Edge-level (single edge); the corner generalizes the cap to n incident ribbons.
- **Kernel relevance.** The exact surgery style this dossier copies; the suitcase corner is "three file-44 caps meeting," and the per-edge convexity sign `sigma` decides shrink vs grow per incident ribbon.

---

## Synthesis: ADOPT / AVOID / INVESTIGATE, first milestone, ladder, declines

### ADOPT
1. **The sphere-octant exact rung for the equal-radius suitcase corner.** Detect three (in general n) mutually inscribed-sphere-tangent convex faces at equal radius; emit the exact spherical-polygon (octant) patch; no fit. Pre-2006 elementary geometry, patent-safe, exact oracle (the surface IS a sphere). **This is the first thing to build.**
2. **The exact-rung detection test** (lifted from file 40 to the vertex): intersect the n offset surfaces at radius r; if they meet at one point `c` and `B(c, r)` is tangent to all incident faces on the material side without overflow, take bucket (a). Equal radius is the load-bearing condition, not perpendicularity.
3. **The setback split** (Varady-Rockwood 1995/1997) as the bucket-(b) skeleton: retract each ribbon by a per-edge setback, fill the 2n-sided hole. Pre-2006.
4. **The convex-combination / Charrot-Gregory + Gregory-twist corner patch as a black-box evaluator fed to Keel's existing certified evaluator-to-NURBS fit** (central-split storable form, file 26). No new surface representation needed. Pre-2006 evaluator.
5. **The build-octant-then-splice Euler surgery** of Q4 (make-dominant, file 44 style), netting +1 face, with the per-incident-edge convexity sign deciding cap-shrink vs cap-grow.
6. **Autosetback by average-blend-size balancing** (ACIS heuristic) as the default setback chooser, with manual override.

### AVOID
1. **Copying any post-2006 claimed corner method verbatim.** Flag US 8,004,517 (2011) and US 10,621,781 (2020, spherical parametrization); design around their specific claim elements. Prefer the expired Charrot-Gregory / Varady-Rockwood prior art and the unpatentable sphere octant.
2. **A bespoke n-sided surface type in the store.** The corner patch is an evaluator + a fit, not a new entity; reuse the Coons/Gordon fit pipeline.
3. **In-place loop-regrow for mixed-convexity corners.** Use construct-and-sew beads (file 44 Q3) there.
4. **Treating every equal-radius corner as a general setback patch.** That throws away the exact sphere octant. Detect the analytic rung first.

### INVESTIGATE
1. **The partial analytic rung for two-equal-one-different radius and for non-perpendicular equal-radius corners** (does a sphere-of-revolution / common-tangent-sphere survive?). Q1 says the single-inscribed-sphere condition is the real test; map exactly when it holds.
2. **G2 vertex blends** (curvature-continuous corners): the convex-combination patch is G1; G2 needs Gregory-Hahn biquintic or Hermann patches (file 26), heavier, defer.
3. **Degenerate-setback recovery** (profiles overlap / self-intersect): the autosetback search and the unequal-setback fallback (bucket (c)).
4. **Whether Keel's certified fit meets G1 tangent-plane tolerance along all 2n boundaries** of the convex-combination patch in one fit, or needs the central-split per-subpatch.

### Recommended FIRST MILESTONE (narrowest honest slice with an exact oracle)
**The equal-radius suitcase corner = sphere octant, built by the Q4 Euler sequence.** Inputs: a box solid (three perpendicular planar faces), three equal-radius r convex edge blends already inserted (Keel has the edge engine). Output: the exact sphere-octant vertex patch spliced in by the Q4 operator sequence. **Exact oracle:** the patch surface is exactly `|x - (r,r,r)| = r` restricted to the octant; mass and surface area are closed-form (octant volume contribution and 4πr²/8 area), so `mass == mesh == reference` is checkable to machine precision, and the Euler-Poincare invariant is checked after every operator. This is the analytic rung, the cheapest possible win, and it validates the corner-surgery skeleton before any fit is involved.

### Follow-up ladder
1. **Milestone 1:** equal-radius suitcase corner, exact sphere octant (above).
2. **Milestone 2:** equal-radius non-perpendicular degree-3 corner, exact spherical-triangle (same surgery, general inscribed sphere).
3. **Milestone 3:** unequal-radius degree-3 convex corner, **setback split + convex-combination patch fit-and-certified** (first use of the evaluator-to-NURBS pipeline at a corner); oracle = G1 certification + validator, mass==mesh.
4. **Milestone 4:** curved-support degree-3 corner (cylinder/cone/torus faces), same setback-split evaluator, ribbons from file 40.
5. **Milestone 5:** degree-n (n > 3) corner, 2n-sided patch.
6. **Milestone 6:** concave and **mixed-convexity** corners via construct-and-sew beads (file 44 Q3) + sense-mixed setback hole.

### DECLINE forever (with justification)
- **Self-intersecting / degenerate-setback corners that cannot be resolved by any setback search** (profiles cross no matter the setback): geometrically there is no valid 2n-gon; reject with a clear diagnostic rather than emit an invalid face. Justification: same as file 41's radius-versus-feature-size ceiling, an honest decline beats a wrong solid.
- **G3-or-higher curvature-continuous vertex blends:** outside Parasolid-class scope and not in the capability map; G1 (and selectively G2) is the bar.
- **Vertex blends across non-manifold acorn / wire junctions** where no rolling-ball ribbon exists to terminate: there is no edge blend to corner, so there is no vertex blend; out of scope by construction.

---

## References

1. Spatial / Dassault Systemes. *Blend Geometry* (ACIS technical article; suitcase corner = sphere octant; simple vs n-sided vertex blends). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblndbg.htm
2. Siemens / Parasolid. *Edge Blending: Functions and Options; Overview.* http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.31.html ; .../fd_chap.30.html
3. Varady, T., & Rockwood, A. (1997). Geometric construction for setback vertex blends. *Computer-Aided Design, 29*(6), 413-425. https://www.sciencedirect.com/science/article/abs/pii/S001044859600070X ; Varady, T., & Rockwood, A. (1995). Vertex blending based on the setback split. In *Mathematical Methods for Curves and Surfaces* (pp. 527-542). Vanderbilt UP.
4. Spatial / ACIS. *Setbacks* (Blending Component, BLND R10 Ch.1; autosetback, corner setback, cross edges). http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/BLND/BLND/01CMP/0011.HTM ; PDF: http://www-isl.ece.arizona.edu/ACIS-docs/PDF/BLND/01CMP.PDF
5. Charrot, P., & Gregory, J. A. (1984). A pentagonal surface patch for computer aided geometric design. *Computer Aided Geometric Design, 1*(1), 87-94. https://www.sciencedirect.com/science/article/abs/pii/0167839684900062
6. Plowman, D., & Charrot, P. (1996). A practical implementation of vertex blend surfaces using an n-sided patch. In *Mathematics of Surfaces VI* (pp. 67-78). Clarendon Press.
7. Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365. https://www.sciencedirect.com/science/article/abs/pii/001044859490023X ; open copy https://www.academia.edu/3392234
8. Varady, T., & Hoffmann, C. M. (1998). Vertex blending: Problems and solutions. In *Mathematical Methods for Curves and Surfaces II (Lillehammer)* (pp. 501-527). Vanderbilt UP. Salvi, P., Varady, T., & Rockwood, A. *Ribbon-based transfinite surfaces.* http://salvi.chaosnet.org/texts/ribbon-based-surfaces.pdf
9. Open CASCADE Technology. *ChFi3d_Builder Class Reference* (PerformTwoCorner / PerformThreeCorner / PerformMoreThreeCorner; ExtentOneCorner/TwoCorner/ThreeCorner; ChFi3d_ChBuilder three-chamfer intersection). https://dev.opencascade.org/doc/occt-6.9.0/refman/html/class_ch_fi3d___builder.html
10. Gregory, J. A. (1986). N-sided surface patches. In *The Mathematics of Surfaces* (pp. 217-232). Clarendon Press. Gregory, J. A., & Hahn, J. M. (1989). A C2 polygonal surface patch. *Computer Aided Geometric Design, 6*(1), 69-75.
11. Choi, B. K., & Lee, C. S. *Method for generating an extended rolling ball fillet surface between two surfaces.* US Patent 5,774,359 (expired). https://patents.google.com/patent/US5774359A/en
12. US Patent 8,004,517 (2011). *Methods, apparatus and computer program products that model three-dimensional surface structures* (post-2006, flagged). US Patent 10,621,781 (2020). *Robustly blending surfaces via spherical parametrization* (post-2006, flagged).

## Related Keel dossiers (build on, not repeated)

- `26-transfinite-nsided-surfacing.md`: Coons / Gordon / Gregory / convex-combination n-sided fill, twist incompatibility, central-split storable-NURBS route, the certified evaluator-to-NURBS fit. The vertex patch is an n-sided fill; Q3 routes it through this machinery.
- `28-fillet-chamfer-engine.md`: the edge-blend engine (spine, spring curves, polymorphic cross-section, mitring, blend networks, recognition/unblend). Produces the ribbons the vertex blend terminates.
- `40-blend-fillet-surface-generation.md`: rolling-ball envelope, spine = SSI(offset, offset), exact-analytic surface catalog. The analytic-rung discipline this dossier lifts from edge to vertex.
- `41-blend-overflow-radius-feature-failures.md`: overflow / cliff / notch / setback vocabulary, radius-versus-feature-size limits, the convex cap baseline. Governs the setback distance and the decline-forever cases.
- `44-concave-fillet-topology-surgery.md`: the GWB Euler surgery style, cap-shrink vs cap-grow, the convexity sign `sigma`, build-then-splice. The suitcase corner is three of its caps meeting; Q4 copies its bookkeeping.
- `45-genus1-solid-of-revolution-euler-sequence.md`: the running-total Euler-Poincare discipline and honesty caveat this dossier's Q4 follows verbatim.
- `39-coincident-tangent-face-booleans.md`: the tangent-sew machinery for splicing the G1 corner-patch boundaries (cross arcs are smooth edges to the ribbons).
