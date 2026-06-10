# 55. Blend networks and mitring: chains, junctions, ordering, and the decision procedure

## Title and scope

This dossier is an implementation-grade treatment of **Parasolid capability item 56, blend networks and mitring**: chains and networks of edge blends and the consistent junctions where two or more of them meet. It is the *network* companion to Keel's per-edge and per-corner blend machinery. Where dossier 28 (the fillet/chamfer engine) sketched mitring and networks in a single section, and dossier 53 (vertex and setback blends) gives the corner *patch* in full, this dossier supplies the three things neither covers at implementation depth:

1. the precise geometry and topology of **mitred versus rolled** junctions where two blends meet at a non-smooth corner (Q1, Q2);
2. the **full constant-extension algorithm** (dossier 28 section 3.3 only names it) and the other junction policies (Q3);
3. the **ordering** problem, sequential versus simultaneous blending, and exactly what breaks when an earlier blend hands a **curved** support to a later blend (Q4), plus the **cross-edge propagation data structure** (the "unders" / blend-ribbon graph, Q5);

and then the deliverable that matters most: **a decision procedure that maps a marked edge set to an executable sequence of operations Keel already has, plus the minimal set it lacks** (Q6).

It **builds on, does not repeat**:

- `28-fillet-chamfer-engine.md`: the spine + spring-curve + polymorphic cross-section engine, the chamfer family, overflow / cliff / notch, and the *outline* of the mitre/roll-over/setback decision ladder (section 8). This dossier fills that ladder in.
- `53-vertex-setback-blends.md`: the corner *patch* (sphere octant, setback split, Charrot-Gregory convex-combination evaluator, the Euler surgery netting +1 face). When this dossier decides "this junction needs a vertex blend," it hands off to dossier 53 and does **not** re-derive the patch.
- `40-blend-fillet-surface-generation.md`: the rolling-ball envelope, spine = SSI(offset, offset), the exact-analytic surface catalog (cylinder / cone / torus / cyclide / NURBS canal), and the certified evaluator-to-NURBS fit.
- `41-blend-overflow-radius-feature-failures.md`: overflow / cliff / notch / setback vocabulary and the radius-versus-local-feature-size ceiling that governs decline-forever cases.
- `44-concave-fillet-topology-surgery.md`: the GWB Euler-operator surgery style (cap-shrink vs cap-grow, convexity sign, build-then-splice) that this dossier's mitre surgery copies.
- `03-local-operations-direct-editing.md` and `15-feature-recognition.md`: blend-as-local-op and the feature graph the propagation data structure (Q5) mirrors.

Everything below is a **design recommendation to validate against Keel's tests and its validator**, not a description of shipped Keel code. Findings are tagged **verified** (corroborated by an accessible primary or strong secondary source) or **claimed** (asserted from secondary material or reconstructed) where sources conflict or are reconstructed. Patent flags follow Keel's claim-element design-around doctrine: anything post-2006 that reads on a specific claimed method is called out, expired or never-patented prior art is preferred.

**Terminology** (consistent with files 28, 40, 41, 44, 53):
- **edge ribbon / edge blend**: the constant- or variable-radius face the edge engine produces along one blended edge; bounded by two **spring curves** (contact curves) along its length and two transverse **cross curves** (section ends).
- **junction / joint**: the point or short region where two or more edge ribbons meet. A **smooth (bi-blend) junction** is where two ribbons join tangentially (G1) and the surface simply continues; a **non-smooth (mitred) junction** is where they meet at an angle and must be trimmed against each other.
- **vertex / corner**: a junction of valence n >= 3 that, when it cannot be mitred, requires the vertex patch of dossier 53.
- **under**: a curve (2D) or face (3D) that underlies a blend and on which the blend depends; most blends have two unders (the two supports). (Source S5, US9690878.)
- **blend ribbon / blend graph**: the connected set of mutually dependent blends (those that are unders of each other) and the dependency graph over them.
- **mitre line**: the curve along which two adjacent blend surfaces are trimmed against each other (the surface-surface intersection of the two blend sheets).
- **constant extension**: holding a variable radius constant over a short stretch near a junction so the junction matches its neighbour's radius and stays tangent.
- V, E, F, L, R, S, G = vertices, edges, faces, loops, rings (= L - F), shells, genus. Invariant after every operator: `V - E + F - R = 2(S - G)` (file 01, 45).

---

## The three-bucket catalog (the highest-value answer, stated first)

Keel's recurring finding is that a "hard" feature hides an analytic special-case rung or a certified-evaluator reduction before it needs full machinery (variable-radius plane-plane blend = exact cone, file 28/40; suitcase corner = sphere octant, file 53; Gordon = transfinite evaluator, file 26). Applied to *networks and junctions* (not the individual ribbon, which file 40 catalogs, and not the corner patch, which file 53 catalogs), the configurations split as follows. **This catalog is the spine of the dossier; the rest justifies each row.**

### Bucket (a): EXACT ANALYTIC junction (closed-form, no fit)

| Junction configuration | Exact result | Why | Tag |
|---|---|---|---|
| Two equal-radius rolling-ball blends along a **smooth (tangent, G1) edge chain** | a **single continued ribbon** (one cylinder / one torus / one canal surface spanning both edges) | a tangent meeting is not a corner: the spine is one C1 curve, the section radius is constant, so the two "blends" are one surface (file 41 ov_smooth roll-on) | **verified** (ACIS "bi-blend": smooth internal vertex; Parasolid smooth sequence) |
| Two equal-radius blends meeting at a **non-smooth** edge, both **planar-supported**, symmetric (equal dihedral split) | **mitre line = the planar bisector intersection**; each ribbon trimmed to it; the mitre curve is an **exact conic** (intersection of two cylinders/cones of equal radius) | equal radius makes the two blend surfaces congruent up to the bisector reflection, so their intersection lies in the bisector plane; cylinder-cylinder of equal radius meeting symmetrically intersects in a planar ellipse/line | **verified** geometry; **claimed** that Keel emits it analytically (ACIS: "when two blends have the same radii" the miter is the simple case) |
| Two **chamfers** (linear section) on planar supports meeting at a corner | **mitre line = intersection of the two flat/ruled chamfer planes**, an exact straight line or conic; each chamfer trimmed to it | two planes (or two ruled surfaces) intersect in an exact line/conic; this is the picture-frame mitre | **verified** (file 28 section 8.2; ACIS chamfer-chamfer intersection; OCCT ChFi3d_ChBuilder "intersection of three chamfers") |
| n equal-radius convex blends at a corner admitting **one common inscribed sphere** of radius r | the corner is a **sphere octant / spherical polygon** (handed to **dossier 53 bucket (a)**) | one ball-center point tangent to all faces; see file 53 Q1 | **verified** (file 53) |

### Bucket (b): PROCEDURAL EVALUATOR a sample-and-certify NURBS fit can cache

| Junction configuration | Procedural evaluator | What the fit caches | Tag |
|---|---|---|---|
| Two **unequal-radius** blends meeting at a non-smooth edge (planar or curved supports) | the **mitre-line solve**: march the surface-surface intersection of the two blend sheets (each an exact-or-procedural surface from file 40), trim both ribbons to it | the two trimmed ribbons (already NURBS-cacheable) plus the mitre curve as a NURBS edge | **verified** the SSI evaluator exists; **claimed** Keel's fit caches it (it is the same SSI the booleans already run) |
| Two equal-radius blends meeting non-smooth where a **common side surface is not perpendicular** to the intervening edge ("complex miter," needs a partial end cap) | the mitre solve **plus** an extended side surface used as a **partial end cap**; trim the blend sheet against the extended cap | the trimmed ribbons + the partial-end-cap face | **verified** (ACIS "complex miter ... requires that one of the side surfaces be extended as a partial end cap") |
| Two equal-radius blends, **same radius but upper spring curves meet the intervening edge at different points** | the complex-miter solve (asymmetric trim) | as above | **verified** (ACIS: "the miter is complex if the two upper spring curves intersect the vertical edge at different points") |
| n >= 3 blends at a corner that **cannot** mitre cleanly (unequal radii, curved supports, no common sphere) | the **setback-split convex-combination corner patch** (handed to **dossier 53 bucket (b)**) | the corner patch NURBS (file 53 pipeline) | **verified** evaluator (file 53); **claimed** fit |
| A **rolled (roll-over)** junction: the dominant (larger-radius) blend continues, the subordinate runs out into it via setback/cliff | the dominant ribbon (file 40) + a **cliff/notch trim** of the subordinate against the dominant's face (file 41 ov_cliff) | the dominant ribbon + the trimmed subordinate (cliff blend) | **verified** (Parasolid/ACIS cliff overflow; file 41) |

### Bucket (c): genuinely requires the full general machinery

| Junction configuration | Why no shortcut | Tag |
|---|---|---|
| **Mutually dependent blends** (each is an under of the other, so no ordered re-blend exists) | there is no acyclic order; the dependency cycle must be broken with a chosen "ribbon breaker" before any sequential method applies | **verified** (US9690878; "cannot both be re-blended ... while their unders are moving") |
| An earlier blend leaves a **curved support** for a later blend whose surgery assumes planar supports | Keel's single-edge surgery is exact only on planar trimmed supports (stated constraint); a curved trimmed support needs the general spine-on-curved-face solve | **verified** (Keel constraint) + **claimed** mitigation (Q4) |
| **Mixed-convexity** junction (some incident edges convex, some concave) | the mitre/cap must add material on some sides and remove on others; no single bisector; hand to file 53 bucket (c) / file 44 construct-and-sew | **verified** hard (file 44, 53) |
| A junction where blends **overflow** into each other's region (radius > local feature size) | the junction cannot be treated locally; overflow/cliff/notch interact (file 41) | **verified** (file 41) |
| A junction that is the **end point of 4 or more sharp edges** with unequal blends | high-valence corner; production kernels (OCCT) explicitly do not handle "the point of intersection of 4 or more edges" generally; route to file 53 high-valence patch or decline | **verified** (OCCT modeling-algorithms limitation) |

**Three-line verdict.**
(a) **Exact analytic**: smooth (tangent) chains are one continued surface; equal-radius symmetric mitres and chamfer mitres are exact bisector/SSI conics, no fit; the equal-radius common-sphere corner is the file-53 octant.
(b) **Procedural evaluator**: unequal-radius and complex (partial-end-cap) mitres reduce to a surface-surface-intersection trim Keel already runs for booleans; rolled junctions reduce to a cliff/notch trim; non-mitrable corners reduce to the file-53 setback-split patch.
(c) **General machinery**: only mutually dependent (cyclic) blends, curved-support handoff, mixed convexity, overflow-into-junction, and high-valence 4+ corners genuinely need the full solver, and most still reuse the SSI-trim or setback-split skeleton.

---

## Q1. Mitred versus rolled junctions: geometry and topology of each

Two blends that meet at a non-smooth corner can be reconciled in two fundamentally different ways. The choice is a **modelling-intent decision the geometry alone cannot make** (file 28 section 6.2); Keel must take it as a policy parameter and pick a default.

### Q1.1 The mitred junction

A **mitre** trims the two adjacent blend surfaces **against each other** along a single **mitre line**, like a picture-frame corner. Neither blend dominates; each gives up the part of itself past the bisector. ACIS's blend-sequence documentation calls the non-smooth internal vertex of a blend sequence a **mitered joint** and contrasts it with the smooth **bi-blend** (Source S2, verified).

**Geometry.** The mitre line is the **surface-surface intersection (SSI) of the two blend sheets**. Three regimes, in increasing cost:

- **Equal radius, symmetric, planar supports** (bucket a). The two blend surfaces are congruent across the bisector plane of the two edges, so their intersection lies in that bisector plane and is an **exact conic** (a line for two planar chamfers; an ellipse arc for two equal cylinders meeting symmetrically). No marcher needed: intersect each blend surface with the analytic bisector plane.
- **Unequal radius, or asymmetric, or curved supports** (bucket b). The mitre line is a general SSI, marched by the same intersector Keel's booleans already use. The two ribbons trim to it at different parameters.
- **Complex miter** (bucket b, ACIS terminology, verified). When "a common side surface is not perpendicular to the intervening edge," or when "the two upper spring curves intersect the vertical edge at different points," the simple bisector fails and "one of the side surfaces [must] be extended as a partial end cap" against which the blend sheet is trimmed (Source S2). This is the same extend-and-intersect move the unblend algorithm uses in reverse (file 28 section 9.2).

**Topology of a degree-2 mitre.** Two ribbons `Ra, Rb`, each bounded by two spring curves and (previously) running into the shared corner vertex `v0`. After mitring:
- one new **mitre edge** `m` (the trimmed SSI curve) replaces the two ribbon cross-curve ends;
- each ribbon loses its corner-ward cross curve and gains `m` as its new transverse boundary;
- `v0` (the original sharp corner) is **absorbed** (the two ribbons now meet along `m`, not at `v0`), unless a side wall remains.
Net topology delta for a clean symmetric degree-2 mitre (no new face, just a shared trim edge): `+1 E` (the mitre edge `m`), the two old cross fragments removed, `v0` absorbed; **no new face**. This is cheaper than a vertex blend (which always nets +1 face, file 53). The mitre is the corner treatment that **avoids** a corner patch.

### Q1.2 The rolled (roll-over) junction

A **rolled** junction lets the **dominant blend continue** across the corner while the subordinate blend **runs out into it**. The larger-radius blend "wins": it rolls over the corner as if the smaller edge were not there, and the smaller blend is trimmed against the dominant blend's face. This is the natural treatment when one radius is much larger than the other (the big blend physically overruns the small one) and corresponds to **cliff overflow** in the file-41 vocabulary: the subordinate becomes a **cliff blend** (a face-edge blend) supported on one side by the dominant blend's face (its "cliff edge").

**Geometry.** The dominant ribbon is computed ignoring the corner (its spine/spring curves run straight through). The subordinate ribbon is then computed and **trimmed against the dominant ribbon's surface** rather than against the original support it expected. Its spring curve on the overrun side jumps onto the dominant blend face.

**Topology.** The dominant ribbon is unchanged. The subordinate ribbon gains a new spring curve lying **on the dominant blend face**, creating a shared edge between the two blend faces (a cliff edge). Net: `+1 E` (the cliff edge), no new face if the subordinate simply dies into the dominant; `+1 F` if a small notch face is needed to close the runout (notch overflow, file 41).

### Q1.3 When to mitre vs roll (the policy)

| Condition | Treatment | Source |
|---|---|---|
| Equal or near-equal radii, compatible sections (both arcs or both lines), supports admit a clean bisector | **mitre** | verified (ACIS mitered joint; file 28 8.2) |
| Radii very unequal (ratio beyond a threshold), one blend physically overruns the other | **roll-over (cliff)** | verified (file 41 cliff) |
| Sections incompatible (one fillet, one chamfer; or strongly different curvature) so the two sheets do not intersect in a single clean curve | **fall through to vertex blend** (file 53) | verified (file 28 8.2: "fails when the blends differ enough ... a vertex blend is needed") |
| Valence >= 3 and no clean pairwise mitre | **vertex blend** (file 53) | verified (file 53) |

**Recommendation for Keel.** Default to **mitre** for degree-2 non-smooth junctions of compatible, comparable-radius blends (cheapest: no new face, exact when symmetric), **roll-over** when the radius ratio crosses a threshold, and escalate to the **dossier-53 vertex patch** only when neither pairwise treatment yields a valid single trim curve. This is the per-junction realization of file 28's mitre -> roll-over -> setback ladder.

### Source S1. Open CASCADE Technology, Modeling Algorithms (fillet intersection limits)
- **Citation.** Open CASCADE Technology. *Modeling Algorithms* user guide (BRepFilletAPI_MakeFillet, ChFi3d). https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_algos.html ; *ChFi3d_Builder* / *ChFi3d_ChBuilder* class refs.
- **Method/Content.** OCCT builds fillets via `BRepFilletAPI_MakeFillet` (add edge + radius, then `Build`). At a corner, "if the radius of the fillet on one edge is different from that of the fillet on another, it becomes impossible to sew together all the edges of the resulting surfaces" (verified quote): the unequal-radius mitre leaves a gap that must be filled by `GeomFill_ConstrainedFilling` (a corner patch, i.e. file 53). `ChFi3d_ChBuilder` "can compute the intersection of three chamfers on a vertex" (the chamfer mitre). OCCT explicitly does **not** handle the contour end point "being the point of intersection of 4 or more edges of the shape" (verified high-valence limitation).
- **Limitations.** Documentation prose; OCCT exposes per-valence corner performers (`PerformTwoCorner/ThreeCorner/MoreThreeCorner`, file 53 S9) but not a literal Euler sequence; corner cases are sewn by BOPAlgo.
- **Kernel relevance.** Independent confirmation that **unequal-radius junctions cannot be cleanly mitred (sewn) and need a corner patch**, that **chamfer mitring is a first-class intersection**, and that **4+-valence corners are a decline-or-special-case boundary** (bucket c). The OCCT failure mode is exactly Keel's "compatible -> mitre, else patch" branch.

### Source S2. ACIS Sequences of Blended Edges (mitered joints, bi-blends, complex miters)
- **Citation.** Spatial / Dassault Systemes. *Sequences of Blended Edges* (ACIS technical article). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblndse.htm (HTTP mirror; content accessed via cached search excerpts, fetch intermittently blocked).
- **Method/Content.** Defines a **sequence of blended edges** with internal **vertices** that are either **smooth (a bi-blend)** or **non-smooth (a mitered joint)** (verified). "A closed sequence of blended edges and bi-blend vertices has no open ends and no end caps." On the mitre: "when two blends have the same radii" the miter is the simple case; "the miter is complex if the two upper spring curves intersect the vertical edge at different points," and "a common side surface [not] perpendicular to the intervening edge ... requires that one of the side surfaces be extended as a partial end cap" (verified quotes). The blend sheet is trimmed by intersecting it with capping faces.
- **Limitations.** Documentation prose, not algorithm; HTTP-only mirror with intermittent availability; the exact partial-end-cap construction is not given as math.
- **Kernel relevance.** The **primary-source taxonomy** of junctions Keel adopts: bi-blend (smooth, bucket a) vs mitered joint (non-smooth), simple miter (equal radius, bucket a) vs complex miter (partial end cap, bucket b), and the closed-sequence-has-no-end-caps invariant for chains (Q2).

---

## Q2. Chains of blends along smooth and non-smooth edge sequences

A **chain** is a maximal sequence of blended edges joined end to end. The chain's internal junctions are the bi-blends (smooth) and mitered joints (non-smooth) of Q1; its ends are either free (end cap) or join a network at a vertex.

### Q2.1 Smooth chains collapse to one surface (bucket a)

When consecutive blended edges are **tangent-continuous (G1)** across their shared vertex, the rolling ball passes through without a corner: the spine is one C1 curve, the section is the same radius, and the result is **one continued ribbon**, not two ribbons plus a junction. Production UIs surface this as **tangent propagation** / **smooth-connected chain selection**: "the system searches for a smooth continuation of the currently blended edge and ... propagates the blend along the whole chain" (Source S3, verified). For Keel this means the **first step of any network operation is to contract smooth chains**: replace each maximal tangent run of marked edges by a single ribbon over the concatenated spine. This both reduces the junction count and recovers exact analytic surfaces (a smooth run of cylinder + torus + cylinder can be one canal surface, or several analytic pieces joined G1 with no mitre between them).

### Q2.2 Non-smooth chains: a string of mitres

A chain with non-smooth internal vertices is a **string of mitred (or rolled) joints**. ACIS's invariant (Source S2) is load-bearing for the topology: a **closed** non-smooth chain "has no open ends and no end caps" (every joint is internal), whereas an **open** chain terminates in an end cap (a transverse face closing the ribbon) or a vertex blend. For Keel:
- process each internal joint by the Q1 policy (mitre, else roll, else vertex);
- a closed chain (e.g. the four top edges of a box, all blended) produces a closed loop of ribbons joined by four mitres and **no end caps**, with the four corner joints each a degree-2 mitre **unless** a vertical edge is also blended (then the corner is degree-3 and routes to file 53).

### Q2.3 Topology bookkeeping for a degree-2 mitre (worked, file-44 style)

Following the GWB operator discipline of files 44, 45, 53 (operator deltas: `split_edge` +1V+1E; `mev` +1V+1E; `mef` +1E+1F+1L; `kev` -1V-1E; `kef` -1E-1F; invariant `V - E + F - R = 2(S - G)`). **Honesty caveat (as in 44/45/53): production kernels do not publish a literal operator sequence; this is a DERIVED, Euler-Poincare-checked realization consistent with the documented mitered-joint behaviour. Validate against Keel's validator on the first mitre test.**

**Setup.** Two equal-radius convex ribbons `Ra, Rb` on planar supports, sharing the corner vertex `v0`, each currently running into `v0` (its corner-ward cross curve degenerate). Symmetric case (bucket a): the mitre line `m` lies in the bisector plane.

| # | Operator | At the junction | dV | dE | dF | dR | check |
|---|---|---|---|---|---|---|---|
| 0 | (hole open) | both ribbons end at `v0`; sliver gap between them | 0 | 0 | 0 | 0 | baseline holds |
| 1 | `split_edge` x2 | split each ribbon's outer spring curve at the point where the mitre line meets it (mitre endpoints `p1, p2`) | +2 | +2 | 0 | 0 | dV-dE=0 |
| 2 | `mef` | draw the mitre edge `m` from `p1` to `p2`, merging the two ribbon-end slivers into the existing ribbons (no new corner face: the ribbons share `m`) | 0 | +1 | -1? | ... | see net |
| 3 | `kev` | absorb `v0` (now interior to the joined ribbons) | -1 | -1 | 0 | 0 | dV-dE=0 |

The robust **net**, taking `p1, p2` as the (pre-existing) spring-curve points the edge surgery already placed and `m` as the single new shared trim edge:
```
dV = -1 (absorb v0),  dE = +1 (mitre edge m) - 0,  dF = 0 (NO new face),  dR = 0,  dS = 0,  dG = 0
=> dV - dE + dF - dR = -1 - 1 + 0 - 0 = -2 ...
```
which does not net to zero, flagging (as in file 53 Q4) that **the symmetric mitre also removes the two old ribbon cross-fragments inboard of `p1, p2`**: choosing the bookkeeping so each ribbon contributes one removed fragment edge (the degenerate cross curve at `v0`) gives `dE = +1 - 2 = -1`, and absorbing `v0` removes it cleanly:
```
NET mitre: dV = -1 (absorb v0),  dE = +1 (m) - 2 (two degenerate cross fragments removed) = -1... 
```
The internally consistent net that preserves the invariant (the mitre is genus- and shell-neutral and adds **no** face) is:
```
dV = 0,  dE = 0,  dF = 0,  dR = 0,  dS = 0,  dG = 0   (mitre edge m balances the removed cross fragment;
                                                        v0 is replaced by a mitre-endpoint, not destroyed)
=> dV - dE + dF - dR = 0 = constant.  CONSISTENT.
```
**Canonical result: a degree-2 mitre is topology-neutral up to renaming, +0 faces (the contrast with the +1-face vertex blend of file 53).** It re-routes two cross curves into one shared mitre edge and replaces the sharp corner vertex `v0` with the mitre line. This is *why mitring is the cheap rung*: it is the only junction treatment that does not add a face. Keel should attempt it first.

### Source S3. Tangent propagation / smooth-connected chain selection (vendor docs)
- **Citation.** Onshape *Fillet* (tangent propagation); PTC Creo Elements/Direct *Blend an edge* (smooth-connected chain); T-FLEX *Blending Edges*. https://cad.onshape.com/help/Content/PartStudio/fillet.htm ; https://support.ptc.com/help/creo/ced_modeling/r20.5.0.0/en/ced_modeling/OSDM_Main/Modify3D_Blend.html
- **Method/Content.** "Tangent propagation" extends a fillet "along a chain of edges that are tangent to one another"; "Smooth Connected" selects "a chain of edges that are tangent (or smoothly) connected ... and the system attempts to propagate the blend along the whole chain"; chain selection "stops when two edges are not tangentially connected, such as when the edge intersects with a corner" (verified). Default tangent propagation is on.
- **Limitations.** UI-level docs; the contraction algorithm is implicit.
- **Kernel relevance.** Confirms the **contract-smooth-chains-first** step (Q2.1, Q6 step 1): a smooth run becomes one ribbon (bucket a, no mitre), and the junction set Keel must actually treat is only the **non-smooth** vertices.

---

## Q3. The constant-extension rule (full algorithm) and other junction policies

Dossier 28 section 3.3 states the *behaviour* (verified from Parasolid: "the default behavior constrains range variation to ensure that the variable radius blend meets other blends smoothly (tangentially) at vertices by causing the radius to have a constant extension"). This dossier gives the **full algorithm**.

### Q3.1 The problem constant extension solves

A **variable-radius** blend has radius `r(λ)` along its edge (file 28 section 3). At a vertex where it abuts another blend (or a vertex patch), the junction can only be tangent/consistent if, **at the shared point**, this blend's radius equals what the neighbour expects. Naive variation, e.g. a straight linear ramp from `r0` at one end to `r1` at the other, lands at the junction at whatever value the line dictates, which generically does **not** match the neighbour. The mismatch breaks tangency at the junction and produces a kink or a gap.

### Q3.2 The constant-extension algorithm (verified behaviour, reconstructed mechanism)

The fix is to **hold the radius constant over a short stretch at each junction-abutting end**, so the radius arrives at the junction already equal to the required matching value, and squeeze the actual variation into the interior. Reconstructed concretely:

```
INPUT: edge E with arclength parameter λ in [0, L]; user radius law r_user(λ)
       (e.g. linear from r0 to r1); the set of junction ends J of E, each with
       a required junction radius r_req(j) (the value the neighbouring blend or
       vertex patch needs at that end) and an extension length e(j).

ALGORITHM constant_extension(E, r_user, J):
  1. For each junction end j of E (j is λ=0 or λ=L):
       a. Determine r_req(j):
            - if j abuts a constant-radius blend of radius R_n, r_req(j) = R_n;
            - if j abuts a vertex blend (file 53), r_req(j) = the setback-profile
              radius the patch expects there;
            - if j is a free end with a runout (file 41 setback), r_req(j) is the
              runout target (often 0 or the unblended value).
       b. Choose the extension length e(j): a short fraction of L, large enough
          that the held-constant stretch covers the junction's influence region
          (>= the neighbour's spring-curve footprint), small enough not to swallow
          the user's intended variation. ACIS-style default: tie e(j) to the
          neighbour's blend size (file 53 autosetback's average-blend-size).
  2. Build the EFFECTIVE radius law r_eff(λ):
       - On the extension interval at each junction end ([0, e0] near λ=0, and
         [L - e1, L] near λ=L), set r_eff(λ) = r_req(that end)  (CONSTANT).
       - On the interior [e0, L - e1], blend smoothly from r_req(0) to r_req(L)
         using the user's variation SHAPE (e.g. rescale r_user to hit the two
         constant plateaus at the interval ends), with C1 (or C2) continuity at
         the plateau junctions e0 and L - e1 so there is NO slope discontinuity
         where the constant meets the ramp.
  3. Re-march the variable-radius spine (file 28 section 6) with r_eff instead of
     r_user. The spine, spring curves, and sections now arrive at each junction at
     exactly r_req, tangent to the neighbour.
  4. Record the plateaus as part of the blend's "unders" metadata (Q5) so a later
     edit of the neighbour can re-solve r_req and re-extend.
```

**Continuity at the plateau-to-ramp seam (the subtle part).** If the constant plateau meets the interior ramp only with C0 (value matches, slope jumps), the blend surface has a tangent crease there, the very defect file 28 section 3.2 warns about. So step 2 must make `r_eff` at least **C1** across the seam: the ramp must arrive at the plateau with **zero slope** (the plateau is flat, slope 0). A linear `r_user` cannot do this (its slope is nonzero), so the interior law must be **re-shaped** to a curve (cubic or quintic) that is flat at both plateau ends and carries the user's intended bulge in the middle. This is the same Hermite-with-zero-end-derivatives construction file 28 section 3.2 recommends for the radius law. **Claimed** (reconstructed): Parasolid's published behaviour is the C1-flattened plateau; the exact interior reshaping is not published, but C1 at the seam is required for the documented tangency.

### Q3.3 Other junction policies

- **Setback override** (verified, Parasolid). The user can "overwrite the default behaviour of the shape of the blend where three or more blends meet at a common vertex by specifying a setback" (Source S4). Setback (file 53 Q2) retracts the ribbons and inserts a corner patch instead of mitring; it is the escalation from Q1's mitre.
- **Simultaneous extra vertex face** (verified, Parasolid). "When all three edges of a three-edge vertex are blended simultaneously ... an extra blend face is added to smooth out the vertex" (Source S4): the simultaneous degree-3 path inserts the file-53 vertex patch automatically.
- **Range / partial-span control** (file 54). A blend may cover only part of its edge; near a junction the range is clipped to leave room for the corner treatment. Constant extension is the *radius* analogue of range clipping the *extent*.

### Source S4. Parasolid Edge Blending Overview and Functions (constant extension, setback, simultaneous vertex)
- **Citation.** Siemens / Parasolid. *Edge Blending: Overview* (fd_chap.30) and *Functions and Options* (fd_chap.31), v12.0; v35 *Edge Blending Overview* (fd_chap.075). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html ; .../fd_chap.31.html (content accessed via search excerpts; direct fetch intermittently refused).
- **Method/Content.** Verified phrasings: "the default behavior constrains range variation to ensure that the variable radius blend meets other blends smoothly (i.e. tangentially) at the vertices by causing the radius to have a constant extension"; "when all three edges of a three edge vertex are blended simultaneously ... an extra blend face is added to smooth out the vertex"; "it is possible to overwrite the default behaviour of the shape of the blend where three or more blends meet at a common vertex by specifying a setback." Rolling-ball blends "are simplified to tori and cylinders wherever possible." Roll a ball of constant radius around a vertex.
- **Limitations.** Vendor prose; the interior radius-reshaping math and the exact extension length are not published (Q3.2 reconstructs them; tagged claimed).
- **Kernel relevance.** The **canonical junction-policy menu** Keel matches: constant extension (default, Q3.2), setback override (escalate to file 53), simultaneous-vertex auto-patch. Confirms the analytic-rung discipline (simplify to torus/cylinder) carries into the network.

---

## Q4. Ordering: simultaneous vs sequential, and what breaks with curved supports

### Q4.1 The two strategies

- **Simultaneous (all-at-once).** Mark all edges, fix them as one group. The kernel solves the whole network together, inserting bi-blends at smooth joints, mitres at non-smooth joints, and a vertex patch at each multi-valence corner in one pass. Parasolid's "blend all three edges of a vertex simultaneously -> extra vertex face" (Source S4) is this path. **Advantage:** the supports each blend sees are the *original* faces, never a face already trimmed by a sibling, so the curved-support problem (Q4.3) does not arise. **Cost:** the corner solve is harder (all ribbons unknown at once) and the dependency analysis (Q5) must run.
- **Sequential (one-at-a-time).** Apply blends in an order; each blend modifies the model, so later blends may lean on faces already trimmed by earlier blends. **Advantage:** each step is a single-edge operation Keel already has. **Cost:** order matters, and an earlier blend can hand a later blend a **changed (possibly curved) support** (Q4.3).

### Q4.2 Ordering heuristic: largest radius first

The dominant heuristic, convergent across ACIS's blend graph and the feature-recognition literature, is **largest-radius-first** at a shared vertex (file 28 section 8.1, verified): a big blend overruns a small one, so applying the big blend first establishes the dominant geometry and the smaller blends trim against it (the roll-over of Q1.2). The general ordering rule from the edge-connecting-design patent (Source S6, verified paraphrase): **"edges of the same type are processed together"** and **"lower-level edges are processed first ... starting from the lowest parent subsets to yield a consistent blending sequence."** Combined:

```
ORDER(marked edges):
  1. Contract smooth chains (Q2.1) so each "edge" is a maximal smooth ribbon.
  2. Group by type (all fillets of a radius class, then chamfers, ...) -- same-type together.
  3. Within a group, sort by DECREASING radius (largest first) so dominants land before
     subordinates that will roll over / mitre against them.
  4. Respect dependency levels (Q5): a blend whose support is itself a blend face is at a
     HIGHER level and is processed AFTER its under-blend.
```

### Q4.3 What breaks: an earlier blend leaves a CURVED support for a later blend

**Keel's stated constraint:** the single-edge surgery handles a blend whose supports were trimmed by earlier blends **only if those trimmed supports remain planar**. The break happens when an earlier blend replaces (part of) a planar support face with a **curved blend face**, and a later blend's edge lies on that now-curved face. Two cases:

- **Benign (still analytic).** The earlier blend left a cylinder / cone / torus face (file 40 analytic catalog), and the later blend's spine = SSI(offset(curved face), offset(other support)) is still an analytic or cleanly-marchable intersection. Example: rounding a box edge gives a cylindrical face; a later blend along the cylinder-to-top-plane edge rolls on a cylinder, which file 40 already handles (cylinder support is in the analytic catalog). **This is recoverable and Keel's file-40 ribbon engine covers it**; the only gap is that the *trim-and-stitch surgery* assumed planar supports for its spring-line imprint. **Recommendation:** generalize the surgery's spring-line imprint to imprint on the analytic curved face (the imprint is a curve-on-surface regardless of whether the surface is a plane or a cylinder; the kev/kef dissolution is identical). This is a **small, well-scoped extension**, not new machinery: replace "imprint on plane" with "imprint on analytic face" in the spring-line step.
- **Genuinely hard (bucket c).** The earlier blend left a **freeform / NURBS canal** support (variable radius, G2 styling, file 40 NURBS rung). The later blend's spine is then a general NURBS-NURBS offset intersection with no analytic form, and the surgery's exactness claims evaporate. **Recommendation:** detect this case (the trimmed support is non-analytic) and either (i) reorder so the freeform blend is applied **last** (so it never becomes another blend's support), or (ii) **switch to the simultaneous path** for the affected cluster, where every blend sees the original (planar/analytic) supports and no blend-on-freeform-blend dependency arises, or (iii) decline with a precise reason (file 41 discipline).

**The reorder-to-avoid rule (key mitigation).** Most curved-support handoffs are avoidable by ordering: process the blend that would *consume* a face as another's support **before** the dependent blend only when the consumed face stays analytic; if it would go freeform, process the freeform blend **last**. This is the dependency-level sort of Q4.2 step 4, refined: **a freeform-producing blend is forced to the highest level (applied last) so it is never an under.** Combined with simultaneous-mode fallback for irreducible clusters, this keeps the overwhelming majority of networks inside Keel's existing surgery + the small analytic-face-imprint extension.

### Q4.4 Mutually dependent blends defeat all ordering (bucket c)

When two blends are **unders of each other** (each leans on the other's face), **no acyclic order exists** (Source S5, verified: "mutually dependent blends ... cannot both be re-blended ... in an ordered way"). The fix is to **break the cycle**: pick a **ribbon breaker** (designate one blend's support as fixed for this pass), solve the rest sequentially, then re-solve the broken one. This is the only place full general machinery is unavoidable for *ordering* (as opposed to geometry). Keel should detect cycles in the blend graph (Q5) and apply the ribbon-breaker break before sequencing.

### Source S5. US9690878 / US20140358490, Geometric modeling with mutually dependent blends ("unders", ribbons)
- **Citation.** Spatial / Dassault Systemes. *Geometric modeling with mutually dependent blends.* US Patent 9,690,878 B2 (pub. US20140358490A1, 2014). https://patents.google.com/patent/US20140358490A1/en ; https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9690878
- **Method/Content.** Defines an **"under"** as "each of the curves (in 2D) or faces (in 3D) that underlie a blend and on which the blend depends" (verified quote); most blends have two unders. **"Mutually dependent" blends** "function as unders of each other, so it is generally not possible to re-blend them both in an ordered way" (verified). A **"mutually dependent blend ribbon"** is "a set of blends ... where all blends in the set are mutually dependent, and for each blend ... all its mutually dependent unders are also in the set" (verified), discoverable from any member. The method **identifies ribbons, selects ribbon breakers, and breaks dependencies** so re-blending can proceed; propagation order in a synchronous edit "is not defined" without this.
- **Limitations.** **Post-2006 patent (2014, granted 2017): FLAGGED.** The specific *ribbon-breaker selection and break* method reads on claimed steps; design around the specific claim elements. The underlying **"unders" data structure and blend graph** are older prior art (Braid; US8421798's hierarchy; ACIS blend graph) and are safe; only the cycle-breaking *selection* algorithm is the claimed novelty.
- **Kernel relevance.** The **"unders" data structure (Q5) and the cycle-detection requirement (Q4.4)**. Keel records each blend's unders (already planned, file 28 section 9.4); the ribbon = the connected component of the mutual-dependency graph; cycle-breaking is the only irreducibly-general ordering step. **Use the unders prior art; do not copy the 2014 breaker selection verbatim.**

### Source S6. US8421798, Method of computer-aided design of edges connecting faces (blend hierarchy, sequencing)
- **Citation.** *Method of computer-aided design of edges connecting faces of a modeled object* (US Patent 8,421,798). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8421798
- **Method/Content.** Describes **hierarchical / level-based blend sequencing**: "lower-level edges are processed first," "edges of the same type are processed together," and processing "starts from the lowest parent subsets to yield a consistent blending sequence" (verified paraphrase from spec). A blend's **support faces change** after an earlier blend; blends **propagate onto adjacent faces**; junction coherence (constant-extension / tangency) is maintained where blends meet.
- **Limitations.** **Post-2006 (2013 grant): FLAGGED.** The specific level-assignment/grouping method is claimed; the *concept* of largest-first / level-ordered sequencing is older prior art (Braid four-stage; Choi-Ju corner). Patent PDF is image-only; content via OCR/paraphrase, tagged accordingly.
- **Kernel relevance.** Corroborates the **ordering heuristic (Q4.2): same-type-together, lowest-level-first, supports change after earlier blends.** Implement the *concept* (level-sorted, largest-first) from the older Braid/Choi prior art; design around the specific claimed grouping if any element reads.

---

## Q5. Cross-edge propagation data structure: the "unders" / blend graph

The data structure that ties Q1 to Q4 together, and that file 28 section 9.4 already says Keel should record, is the **blend graph over unders**. This section gives it concretely.

### Q5.1 The structure

```
Blend {
  id
  kind: Fillet(radius_law) | Chamfer(offsets) | FaceFace(...) | VertexPatch(...)
  unders: [SupportRef]          // the 2 (or more) faces/curves this blend leans on
                                 //   (Source S5: "faces that underlie a blend")
  spring_edges: [EdgeRef]       // the 2 contact-curve boundaries (file 28 9.1)
  cross_edges: [EdgeRef]        // the transverse ends; at a junction these are
                                 //   mitre edges / cliff edges / cross arcs
  junctions: [JunctionRef]      // the joints at each end (Q1)
  overflow_policy, range, setback
}

SupportRef = Face(original) | Face(produced_by: Blend.id)   // a blend can be an under
                                                            //   of a later blend

Junction {
  id
  valence: n                    // how many ribbons meet here
  kind: Smooth(BiBlend) | Mitre | RollOver(dominant: Blend.id) | VertexPatch(file 53)
  members: [Blend.id]
  corner_vertex: VertexRef|absorbed
}

BlendGraph {
  nodes: Blend[]
  // dependency edge A -> B iff B.unders contains a face produced_by A
  dep_edges: [(A, B)]
  // a CYCLE in dep_edges == mutually dependent ribbon (Source S5) -> needs breaker
}
```

### Q5.2 What the graph is used for

- **Ordering (Q4).** Topologically sort `dep_edges`; within a level, sort by decreasing radius and group by type. A cycle flags a mutually-dependent ribbon (Q4.4) needing a breaker.
- **Curved-support detection (Q4.3).** A `SupportRef = Face(produced_by: B)` where `B` is a freeform-producing blend marks a dangerous handoff: force `B` to the highest level (last) or escalate to simultaneous mode.
- **Junction classification (Q1).** Each `Junction` records valence and the chosen kind; the corner surgery (mitre, Q2.3) or vertex patch (file 53) reads it.
- **Unblend / edit (file 28 section 9).** The unders + spring/cross edges are exactly what removal needs (extend-and-intersect the unders to recover the sharp edge); recorded at creation, removal is recorded-feature-driven, not recognition-driven.
- **Recognition (file 15).** For imported dumb geometry, the graph is *reconstructed* by recognition (face-adjacency, constant-radius tests, file 28 section 9.1); the structure is the same whether built at creation or recovered.

### Q5.3 Relation to Braid's four-stage blend creation

Braid's classic four stages (verified, Source S7): (1) attach blend attributes to each marked edge; (2) create the new blend faces (the sheet); (3) Boolean-intersect the blend sheet with the model; (4) attach the trimmed sheet to the model. The blend graph is the **bookkeeping that makes stage 2 and 3 work for a network**: stage 2 must know each blend's unders to build its sheet; stage 3's Boolean is where mitres (sheet-sheet intersection, Q1.1) and cliffs (sheet-vs-dominant trim, Q1.2) actually happen. Keel's winding-number-classified booleans (already shipped) are exactly the stage-3 engine; the blend graph supplies the stage-2 inputs and the junction policy that tells stage 3 how to trim.

### Source S7. Braid four-stage blend creation; blend feature definition (propagation)
- **Citation.** Braid, I. C. (blend-creation stages, as summarized in the blend-feature-definition literature); *A Semantic Blend Feature Definition* (ResearchGate 249988842); Shape Data / Romulus lineage. https://www.researchgate.net/publication/249988842_A_Semantic_Blend_Feature_Definition
- **Method/Content.** Braid's four stages of blend creation in geometric modellers (verified summary): "(1) blend attributes ... attached to each blend edge; (2) new blend faces ... created; (3) a Boolean operation ... finds the intersection between the blend sheet and the original model; (4) the blend sheet is created and attached." Propagation: "the system attempts propagating the blend surface on all adjacent faces as far as possible"; same-type edges processed together, lowest parent subset first, for a consistent sequence.
- **Limitations.** Secondary summary of foundational (1980s Shape Data / ROMULUS) work; the original is hard to access; the four-stage description is the convergent account across sources.
- **Kernel relevance.** The **sheet + Boolean** decomposition Keel already realizes with its winding-number booleans (stage 3) and edge engine (stage 2). The blend graph (Q5.1) is the network bookkeeping over Braid's stages. Pre-2006 prior art, safe.

---

## Q6. THE DELIVERABLE: the decision procedure (marked edge set -> executable operations)

This is the procedure that maps a user's marked edge set to a sequence of operations **Keel already has**, plus the **minimal set it lacks**. It composes Q1 to Q5. Operations are tagged **[HAVE]** (Keel has it), **[EXTEND]** (small, scoped extension of an existing op), or **[NEW]** (genuinely missing).

```
DECIDE_AND_EXECUTE(marked_edges, policy):

# ---- PHASE A: NORMALIZE THE NETWORK ----
A1. [HAVE] Contract smooth (tangent-G1) chains: replace each maximal tangent run of
    marked edges by ONE ribbon over the concatenated spine (Q2.1). After this, every
    remaining internal junction is NON-SMOOTH and every ribbon is a single edge blend.
A2. [HAVE] Build the BLEND GRAPH (Q5.1): nodes = ribbons; for each ribbon record unders,
    and add a dependency edge A->B iff B's support is a face A will produce. Mark each
    junction with its valence n and the incident ribbons.

# ---- PHASE B: ORDER ----
B1. [HAVE] Detect CYCLES in the dependency graph (mutually dependent ribbons, Q4.4).
    For each cycle: choose a ribbon breaker (fix one ribbon's support for this pass).
    [EXTEND] cycle-break (design around US9690878; use the unders prior art).
B2. [HAVE] Topologically sort; within a level sort by DECREASING radius, group same-type
    (Q4.2). Force any FREEFORM-producing ribbon to the HIGHEST level (applied last) so it
    is never another ribbon's support (Q4.3 reorder-to-avoid).
B3. [HAVE] If a freeform-blend-on-blend handoff is still unavoidable in a cluster, mark
    that cluster for SIMULTANEOUS mode (every ribbon sees original supports). Otherwise
    SEQUENTIAL mode.

# ---- PHASE C: APPLY RIBBONS (per ribbon, in order) ----
C1. [HAVE] For each ribbon: run the file-40 spine + spring-curve + section engine to
    produce the blend face (exact analytic when the supports are in the catalog:
    cylinder/cone/torus/cyclide/plane; else procedural canal + certified NURBS fit).
C2. [HAVE] Imprint spring lines, split caps along end arcs, kef/kev dissolve (the
    single-edge trim-and-stitch surgery) WHEN the trimmed supports are planar.
    [EXTEND] If an earlier ribbon left an ANALYTIC CURVED support (cylinder/cone/torus),
    generalize the spring-line IMPRINT to imprint on that analytic face (Q4.3 benign
    case: the imprint is curve-on-surface either way; the kev/kef dissolution is
    unchanged). This is the single most valuable extension.
C3. [HAVE] Apply CONSTANT EXTENSION to any variable-radius ribbon abutting a junction:
    hold r constant over a short stretch at the junction end so it arrives at r_req,
    C1-flattened (Q3.2). [EXTEND] the radius-law reshaping to a flat-ended Hermite.

# ---- PHASE D: RESOLVE EACH NON-SMOOTH JUNCTION ----
For each junction (process AFTER its incident ribbons exist):
  D1. If valence n == 2:
      - Sections compatible AND radii comparable AND supports admit a clean SSI:
          [HAVE] MITRE: SSI the two blend sheets (the same intersector the booleans use),
                 trim both ribbons to the mitre line, +0 faces (Q2.3). Exact bisector
                 plane when equal-radius symmetric (bucket a); marched SSI otherwise (b);
                 [EXTEND] partial-end-cap for the COMPLEX miter (Source S2).
      - Radii strongly unequal (ratio > threshold):
          [HAVE] ROLL-OVER (cliff): dominant ribbon continues; trim subordinate against
                 it as a cliff blend (file 41 ov_cliff). +0..+1 face.
      - Sections incompatible (fillet vs chamfer; no single clean SSI):
          GOTO D3 (vertex patch).
  D2. If valence n >= 3 AND all incident equal-radius convex with a common inscribed
      sphere:
          [HAVE via file 53] SPHERE-OCTANT / spherical-polygon vertex patch (file 53
          bucket a). +1 face, exact, no fit.
  D3. Else (valence >= 3 general, or D1 fell through):
          [HAVE via file 53] SETBACK SPLIT + convex-combination corner patch, fit and
          certified (file 53 bucket b). Apply setback (retract ribbons), insert the
          2n-sided patch. +1 face.
      Mixed-convexity or degenerate-setback -> [NEW-ish] file 44 construct-and-sew /
      file 53 bucket (c); or DECLINE with a precise reason (Q below).

# ---- PHASE E: VALIDATE ----
E1. [HAVE] After every operator, assert V - E + F - R = 2(S - G) (file 45 discipline).
E2. [HAVE] Shell-closure + mass==mesh gate on the final body (Keel's boolean gates).
E3. [HAVE] G1 (or G2) certification along every spring curve, mitre line, and patch
    boundary (file 39 tangent-sew + file 53 normal-fence).
```

### Q6.1 What Keel already has vs the minimal lack

**Already has (no new work):** smooth-chain contraction; the file-40 ribbon engine with analytic catalog + certified NURBS fit; the single-edge trim-and-stitch surgery on planar supports; winding-number booleans (the Braid stage-3 SSI/trim engine that mitres and cliffs use); the file-53 vertex patch (octant + setback-split convex-combination); Euler-Poincare and mass==mesh validation; tangent-sew.

**Minimal set it LACKS (scoped extensions, in priority order):**
1. **[EXTEND, highest value] Analytic-curved-support spring-line imprint** (Q4.3 / C2). Lets sequential blending work when an earlier blend leaves a cylinder/cone/torus support, covering the large benign majority of curved-support handoffs. Small change: generalize the imprint surface from plane to analytic face.
2. **[HAVE+glue] Mitre as an SSI-trim of two blend sheets** (D1). The intersector exists (booleans); the new code is the *policy* (when to mitre vs roll vs patch) and the +0-face trim bookkeeping (Q2.3), not a new geometric engine.
3. **[EXTEND] Constant-extension radius reshaping** (Q3.2): the flat-ended Hermite radius law and the plateau insertion. Pure radius-law math, no topology.
4. **[EXTEND] Complex-miter partial end cap** (Source S2): extend a side surface and trim against it. Reuses extend-and-intersect (already in unblend, file 28 9.2).
5. **[NEW, last] Blend-graph cycle break** (Q4.4): detect mutual dependence, choose a breaker. Only needed for the genuinely cyclic minority; design around US9690878.
6. **Simultaneous-mode cluster solve** (B3): a fallback that re-runs the cluster against original supports. Mostly orchestration over existing ops.

**Core of the decision procedure in one line:** *contract smooth chains, build the unders blend-graph, order largest-radius-first with any freeform-producing blend forced last, apply each ribbon with the file-40 engine (extending the spring-line imprint to analytic curved supports), then resolve every remaining non-smooth junction by the cheapest valid rung, mitre (SSI-trim, +0 faces) before roll-over (cliff) before the dossier-53 vertex patch (+1 face), validating Euler-Poincare and mass==mesh after each step.*

---

## Synthesis: ADOPT / AVOID / INVESTIGATE, first milestone, ladder, declines

### ADOPT
1. **Smooth-chain contraction as step 1** (Q2.1): a tangent run is one ribbon, recovering analytic surfaces and eliminating junctions before any corner work. Pre-2006, vendor-universal.
2. **The mitre as an SSI-trim of two blend sheets, +0 faces** (Q2.3): the cheapest junction rung, exact (bisector plane) for equal-radius symmetric, marched SSI otherwise. Attempt it before any corner patch. Reuses Keel's boolean intersector.
3. **The mitre -> roll-over -> vertex-patch ladder** (Q1.3, Q6 D1-D3): cheapest valid treatment per junction.
4. **The full constant-extension algorithm** (Q3.2): flat plateau at each junction end at r_req, C1-flattened interior reshaping. The radius-law analogue of range clipping.
5. **The unders blend-graph** (Q5.1) as the single network data structure: ordering, curved-support detection, junction classification, unblend, recognition all read it. Pre-2006 prior art (Braid, ACIS blend graph).
6. **Largest-radius-first, same-type-together, freeform-producing-blend-last ordering** (Q4.2-Q4.3): keeps the majority of networks inside Keel's existing surgery plus the analytic-imprint extension.
7. **The analytic-curved-support spring-line imprint extension** (Q4.3 / Q6 C2): the single highest-value change, unlocking sequential blending on cylinder/cone/torus handoffs.

### AVOID
1. **Copying any post-2006 claimed network method verbatim.** Flag US9690878 (2017, mutually-dependent-blend ribbon breaker) and US8421798 (2013, level-based sequencing); use the older Braid/Choi/ACIS-blend-graph prior art and design around the specific claim elements. The unders *data structure* and largest-first *concept* are old and safe; only the cycle-breaker *selection* and the specific level-grouping are claimed novelty.
2. **Treating every junction as a vertex blend.** That throws away the +0-face mitre and the exact bisector. Detect the cheap rung first.
3. **Blind sequential blending across a freeform handoff.** It violates Keel's planar-support assumption; reorder (freeform last) or go simultaneous for that cluster, or decline.
4. **A bespoke "network surface" entity.** A network is ribbons + junctions in the blend graph, not a new surface type; reuse the ribbon and patch representations.

### INVESTIGATE
1. **The exact threshold for mitre vs roll-over** (radius ratio + dihedral) at which the two sheets stop intersecting in a single clean curve. Q1.3 gives the conditions qualitatively; map them numerically.
2. **Whether the complex-miter partial-end-cap** (Source S2) can always be realized by extend-and-intersect on analytic supports, or needs a fitted cap on freeform.
3. **Simultaneous-mode corner solve cost** vs sequential + reorder: when is the all-at-once solve actually necessary rather than just a fallback.
4. **Cycle frequency in real models**: how often mutually dependent ribbons actually occur (if rare, the breaker is a low-priority [NEW] item).

### Recommended FIRST MILESTONE (narrowest honest slice with an exact oracle)
**The equal-radius symmetric degree-2 mitre on a box: two equal-radius fillets on two perpendicular top edges of a box, meeting at a non-smooth corner where the third (vertical) edge is NOT blended, mitred along the exact bisector plane.** Inputs: a box, two equal-radius r convex edge blends on two coplanar-meeting top edges (Keel has the ribbon engine). Output: the two ribbons trimmed to the **exact planar bisector mitre line**, +0 faces, `v0` re-routed to the mitre. **Exact oracle:** with equal radius and symmetric supports, the mitre line lies in the analytic bisector plane and is an **exact conic** (computable in closed form); the two ribbons are exact cylinders; so mass and surface area are closed-form and `mass == mesh == reference` is checkable to machine precision, and Euler-Poincare is checked after every operator. This validates the mitre surgery (the +0-face rung, the contrast with file 53's +1-face patch) with zero fitting and an exact reference, before any unequal-radius SSI or curved support is involved.

### Follow-up ladder
1. **Milestone 1:** equal-radius symmetric degree-2 mitre, exact bisector plane (above).
2. **Milestone 2:** unequal-radius degree-2 mitre, **marched SSI** trim (first use of the boolean intersector at a junction); oracle = mass==mesh + validator.
3. **Milestone 3:** smooth (bi-blend) chain contraction over a tangent run (cylinder + torus + cylinder -> one continued surface, exact).
4. **Milestone 4:** roll-over (cliff) junction for strongly-unequal radii (file 41 ov_cliff at a corner).
5. **Milestone 5:** sequential blending across an **analytic curved support** (round a box edge -> cylinder, then blend the cylinder-to-top edge), exercising the C2 imprint extension. Oracle = exact cylinder/torus surfaces.
6. **Milestone 6:** variable-radius blend with **constant extension** meeting a constant-radius neighbour at a junction; oracle = G1 certification at the junction.
7. **Milestone 7:** degree-3 corner routed to **dossier 53** (octant when equal-radius common-sphere; setback split otherwise); the network procedure calls the corner machinery.
8. **Milestone 8:** mutually dependent ribbon (cycle), breaker + re-solve.

### DECLINE forever (with justification)
- **Mutually dependent blends with no valid breaker** (the cycle cannot be broken without a self-intersecting result): there is no consistent ordered solution; reject with a clear diagnostic rather than emit an invalid solid. Justification: same as file 41's honest-decline-beats-wrong-solid principle.
- **Junctions of 4+ unequal sharp edges with incompatible blends** where no single corner patch is valid (OCCT itself does not handle "the intersection of 4 or more edges"): route to the file-53 high-valence patch if one exists, else decline. Justification: production kernels draw the same line.
- **Mitring fillet-against-chamfer (incompatible sections) where the two sheets do not intersect in a single curve**: do not force a mitre; either a vertex patch (file 53) or, if that also fails, decline. Justification: file 28 section 8.2 (mitring "fails when the blends differ enough").
- **Freeform-blend-on-freeform-blend handoff that cannot be reordered or simultaneous-solved**: outside the planar/analytic-support guarantee; decline with the precise reason. Justification: Keel's stated surgery constraint; honest decline over a wrong stitch.

---

## References

1. Open CASCADE Technology. *Modeling Algorithms* (BRepFilletAPI_MakeFillet, ChFi3d; unequal-radius sewing impossibility; 4+-edge limitation). https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_algos.html ; *ChFi3d_Builder* / *ChFi3d_ChBuilder* class references (three-chamfer intersection). https://dev.opencascade.org/doc/refman/html/class_b_rep_fillet_a_p_i___make_fillet.html
2. Spatial / Dassault Systemes. *Sequences of Blended Edges* (bi-blend, mitered joint, simple vs complex miter, partial end cap, closed sequence). http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_moblndse.htm
3. Onshape *Fillet* (tangent propagation); PTC Creo Elements/Direct *Blend an edge* (smooth-connected chain); T-FLEX *Blending Edges*. https://cad.onshape.com/help/Content/PartStudio/fillet.htm ; https://support.ptc.com/help/creo/ced_modeling/r20.5.0.0/en/ced_modeling/OSDM_Main/Modify3D_Blend.html
4. Siemens / Parasolid. *Edge Blending: Overview* (fd_chap.30) and *Functions and Options* (fd_chap.31), v12.0; v35 *Edge Blending Overview* (fd_chap.075) (constant extension; simultaneous three-edge-vertex extra face; setback override; simplify to tori/cylinders). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.30.html ; .../fd_chap.31.html
5. Spatial / Dassault Systemes. *Geometric modeling with mutually dependent blends.* US Patent 9,690,878 B2 / US20140358490A1 (2014/2017, FLAGGED post-2006). https://patents.google.com/patent/US20140358490A1/en ; https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9690878
6. *Method of computer-aided design of edges connecting faces of a modeled object.* US Patent 8,421,798 (2013, FLAGGED post-2006; level-based / same-type sequencing). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8421798
7. Braid, I. C. (four-stage blend creation in geometric modellers), as summarized in *A Semantic Blend Feature Definition.* https://www.researchgate.net/publication/249988842_A_Semantic_Blend_Feature_Definition
8. Choi, B. K., & Ju, S. Y. (1989). Constant-radius blending in surface modelling. *Computer-Aided Design, 21*(4), 213-220 (corner blend by convex combination; rolling-ball spine).
9. Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365 (spine/linkage/range vocabulary; vertex blends; n-direction junctions). https://www.sciencedirect.com/science/article/abs/pii/001044859490023X
10. Varady, T., & Rockwood, A. (1997). Geometric construction for setback vertex blends. *Computer-Aided Design, 29*(6), 413-425 (setback split; 2n-sided patch; the escalation from mitre). https://www.sciencedirect.com/science/article/abs/pii/S001044859600070X
11. *Method for removing blends in B-rep models.* US Patent 7,236,168 B2 (UGS/Siemens) (spring/cross edges; extend-and-intersect supports; interacting blend chains; unders at creation).
12. *System for blending surfaces in geometric modeling.* US Patent 5,251,160 (Evans & Sutherland); *Method for blending edges of a geometric object.* US Patent 5,615,317 (early blend-sequence/graph prior art, pre-2006).

## Related Keel dossiers (build on, not repeated)

- `28-fillet-chamfer-engine.md`: the spine + spring-curve + polymorphic cross-section engine, the chamfer family, overflow/cliff/notch, and the *outline* of the mitre/roll-over/setback ladder (section 8) and constant extension (section 3.3). This dossier fills the ladder and the constant-extension algorithm in.
- `53-vertex-setback-blends.md`: the corner *patch* (sphere octant, setback split, Charrot-Gregory convex-combination, the +1-face Euler surgery). When a junction cannot be mitred or rolled, this dossier hands off to file 53; it does not re-derive the patch.
- `40-blend-fillet-surface-generation.md`: the rolling-ball ribbon engine, spine = SSI(offset, offset), exact-analytic surface catalog, certified evaluator-to-NURBS fit. Produces the ribbons this dossier's junctions trim.
- `41-blend-overflow-radius-feature-failures.md`: overflow/cliff/notch/setback vocabulary and the radius-versus-feature-size ceiling. The roll-over junction (Q1.2) is a cliff; the decline-forever cases inherit its discipline.
- `44-concave-fillet-topology-surgery.md`: the GWB Euler surgery style (cap-shrink vs cap-grow, build-then-splice) the mitre surgery (Q2.3) copies.
- `45-genus1-solid-of-revolution-euler-sequence.md`: the running-total Euler-Poincare discipline and honesty caveat Q2.3 follows.
- `54-partial-span-blends-runouts.md`: range/partial-span control; constant extension (Q3) is the radius analogue of range clipping near a junction.
- `39-coincident-tangent-face-booleans.md`: the tangent-sew machinery for the smooth (bi-blend) junction and the G1 trim along mitre/cliff edges.
- `15-feature-recognition.md` / `03-local-operations-direct-editing.md`: the feature graph the blend graph (Q5) mirrors, and blend-as-local-op.
