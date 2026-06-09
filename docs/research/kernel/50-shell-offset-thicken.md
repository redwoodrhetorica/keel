# 50. Shell / Hollow / Thicken / Whole-Body Offset (Phase-4 family, roadmap 41-45)

**Scope.** This dossier is the implementation-grade specification for Keel's solid-offsetting family: shell/hollow (item 41), pierce-open faces (item 42), per-face thickness (item 43), sheet thicken (item 44), and whole-body offset (item 45). It treats the **offset-topology** half of the problem, which is independently researchable now even though the surface-generation and boolean-assembly halves sit downstream of the boolean-result-assembly fix (`47-boolean-result-assembly.md`, which these operations consume for general-position booleans).

This file **builds on and does not repeat** the following:
- `nurbs/05-offsets-blends-sweeps.md`: offset non-rationality (`S_d = S + d·N`, the unit-normal square root), the offset-approximation method portfolio (Tiller-Hanson for low degree, sampled-and-refit for high), the canal/PN exact-offset escape hatches, the basic shelling-by-face-offset-and-stitch survey (its Section 10), and the medial-axis/self-intersection connection (its Section 9). Those are assumed. This file specifies the **topology operations** that survey leaves abstract.
- `kernel/10-minkowski-medial-midsurface.md`: the medial axis, MAT, local feature size (LFS), the wall-thickness field `w = 2·dist(·,MA)`.
- `kernel/41-blend-overflow-radius-feature-failures.md`: the medial `r_max(t) = lfs = dist(spine, MA)` predictor and the reduce/reject/report policy. **We reuse that exact predictor here** as the shell self-intersection oracle. The single change of variable is that the "spine" becomes "every point of every offset face," and `r` becomes the wall thickness `t`.
- `kernel/03-local-operations-direct-editing.md`: tweak = `change_face_surfaces(faces, new_surfaces, topology_policy)` driving a generic 1-ring extend-and-reintersect pass. **The shell inner-wall rebuild is a multi-face tweak.** We reuse that machinery rather than inventing a parallel one.
- `kernel/13-import-healing-extension.md`: surface extension/extrapolation (analytic-exact first, bounded de Boor natural extension for NURBS, bail to fallback on long extensions). The reintersect step depends on it.
- `kernel/51-sheet-bodies.md` (the sheet-bodies dossier): non-manifold and double-sided sheet bodies, on which item 44 (thicken) depends. Cross-referenced in Section 5.

The five centerpiece deliverables: (1) the **offset-and-reintersect shell algorithm** expressed as a tweak client (Section 1); (2) the **`t_max` collapse detector** = the `r_max` predictor re-aimed at walls (Section 2); (3) the **per-face / pierce** options (Section 3); (4) the **whole-body winding-number offset trim** (Section 4); (5) the **sheet-thicken rim-wall build** (Section 5). A closing synthesis and the shelled-box / pierced-box validation follow.

A note on terminology: **outer shell** is the original body's boundary; **inner shell** is the inward-offset boundary at distance `t`; **rim wall** is the bridging face between them at a pierced/open boundary. **Wall thickness** `t` is the offset distance. All offsets here are *normal* offsets (along the surface normal), distinct from geodesic/in-surface offsets.

Per project convention this document uses no em-dash characters.

---

## Section 1. The shell algorithm: offset-and-reintersect as a tweak client

### 1.1 The operation, stated precisely

Hollow/shell takes a solid `B` and a wall thickness `t > 0` and produces a thin-walled solid whose boundary is two nested shells: the original outer boundary `∂B`, and an inner boundary offset inward by `t`, with the interior between them being the new material and everything inside the inner shell being a void. If a non-empty set of faces `P ⊆ Faces(B)` is selected to *pierce*, those faces are removed from both shells so the void opens to the outside through them (the result is a cup, not a closed void): see Section 3.

The naive mental model "subtract an inward-offset copy of the solid" (the regularized-boolean formulation, Rossignac-Requicha) is correct in the limit but is **not** how a production kernel builds it, because (a) it requires a robust offset *solid* which is the very thing that self-intersects, and (b) it discards the topology correspondence between outer and inner shells that makes the result clean. The production algorithm is **face-wise offset plus reintersect plus bridge**, due to Forsyth (1995), and it is a direct client of the tweak/reintersect core.

### 1.2 The algorithm (Forsyth-style, tweak-backed)

```
INPUT:  solid B, thickness t (>0 inward; <0 outward), pierce set P (possibly empty),
        optional per-face thickness map tf: Face -> t   (Section 3)
OUTPUT: shelled solid, or a best-effort partial with diagnostics

S0. FEASIBILITY (Section 2). Run the t_max predictor over B. If any face's
    required offset exceeds the local t_max, flag the collapse/self-intersect
    region and choose policy (reject / collapse-merge / local-reduce / report).

S1. OFFSET EACH FACE. For every face F not in P, build its offset surface
    Off(F) = surface(F) displaced by -t along its normal (inward).
      - analytic faces (plane/cylinder/cone/sphere/torus): EXACT offset
        (plane->parallel plane; cylinder r->r-t; sphere r->r-t; cone shifts
        along axis; torus tube R_tube->R_tube-t). nurbs/05 Section 3 / PN-canal.
      - NURBS faces: approximate offset to tolerance (Tiller-Hanson / sampled-
        and-refit, nurbs/05 Section 2), carry the exact procedural source as
        the truth and the NURBS as a cache.
    Do NOT yet trim Off(F): it is an untrimmed (or loosely trimmed) carrier
    surface. The original face's own trim loops are about to be invalidated
    because its neighbors moved too.

S2. REBUILD INNER-SHELL TOPOLOGY BY REINTERSECTION  (this IS a tweak).
    The inner shell has the SAME face/edge/vertex graph as the outer shell
    (Section 1.3), one inner face per offset outer face. So:
      - allocate inner faces F'_i mirroring outer faces F_i, each carrying
        surface Off(F_i);
      - for each inner edge E' (mirror of outer edge E shared by F_i,F_j):
        recompute E' as the intersection curve of Off(F_i) and Off(F_j),
        extending the two offset surfaces as needed (kernel/13) exactly as
        tweak's 1-ring reintersect does;
      - for each inner vertex V' (mirror of outer vertex V where F_i,F_j,F_k
        meet): recompute V' as the curve-curve / curve-surface intersection
        of the incident recomputed inner edges.
    This is precisely change_face_surfaces(all inner faces, their offsets,
    topology_policy) from kernel/03: every inner face's surface changed at
    once, and the shared edges/vertices are regenerated by the generic
    extend-and-reintersect pass. The shell is a MAXIMAL multi-face tweak.

S3. ORIENT THE INNER SHELL INWARD. Reverse the sense of every inner face so
    its outward normal points into the wall material (toward the outer shell),
    i.e. the inner shell bounds the void. (Sense bookkeeping per kernel/46.)

S4. BRIDGE OUTER TO INNER ALONG PIERCED / OPEN FACES (rim walls, Section 3).
    For each pierced face F in P, F is absent from both shells; the hole it
    leaves in the outer shell and the matching hole in the inner shell are
    joined by a RIM WALL around their shared boundary loop (Section 3.2). If
    P is empty the inner shell is a closed void and no rim walls are built:
    the result is a hollow solid with a fully enclosed cavity.

S5. ASSEMBLE AND VALIDATE. Stitch outer shell + inner shell (+ rim walls) into
    one solid via the boolean-result-assembly machinery (kernel/47): match
    coincident rim edges, build the shell/region graph, check Euler-Poincare
    and watertightness. Snap tolerant edges. Emit diagnostics.
```

### 1.3 Why the inner shell mirrors the outer shell (the topology correspondence)

The inner shell is a *combinatorial copy* of the (un-pierced part of the) outer shell. This is the single most important structural fact and it is what lets the rebuild be a tweak rather than a fresh boolean:

- **Faces.** One inner face `F'_i` per offset outer face `F_i`. Offsetting `S -> S + d·N` is a homeomorphism of each face away from self-intersection (it preserves the surface's parameter domain), so the face set is preserved one-for-one.
- **Edges.** One inner edge `E'` per outer edge `E`. An outer edge `E = F_i ∩ F_j` (intersection of two outer faces) maps to `E' = Off(F_i) ∩ Off(F_j)`. The *adjacency* is identical: `E'` is shared by exactly `F'_i` and `F'_j`. Convexity flips sign relative to the offset direction (a convex outer edge becomes a concave inner edge and vice versa, which is exactly why concave outer regions are the ones that collapse, Section 2).
- **Vertices.** One inner vertex `V'` per outer vertex `V`, with the same incidence (`V'` joins the same n inner edges that `V` joined outer edges).
- **Loops.** Each inner face's trim loops mirror the outer face's loops, re-expressed in the offset surface's parameter space. Inner loops (holes in a face) stay inner loops.

So the *graph* is copied verbatim and only the *geometry* (surfaces, curves, points) is recomputed by reintersection. Two outer faces that met along an edge will, after offsetting, in general no longer meet there: their offset surfaces must be extended (kernel/13) until they re-intersect, and the new intersection curve is the inner edge. This is identical to tweak's behavior when a face's surface is replaced and its neighbors must be re-met (kernel/03 Section 1). The shell is the case where *every* face is "replaced" by its offset simultaneously, so the entire boundary is one big reintersect.

Topology *changes* away from this clean mirror only where the offset self-intersects or a face collapses (Section 2): there the one-for-one correspondence breaks (a face vanishes, two inner faces cross and must be merged or trimmed against each other). That is the hard part and the subject of Section 2.

### 1.4 Topology operations created, enumerated

For a clean shell with pierce set `P` (Euler operators in the kernel/01 sense):
- **Created faces:** `|Faces(B)| - |P|` inner faces (one per non-pierced outer face); plus one rim-wall face per edge of the boundary loops of `P` (Section 3.2), or a single rim face per pierced-face boundary if rim walls are built as ruled/offset strips.
- **Created edges:** one inner edge per non-pierced outer edge; plus, per pierced face, the rim edges joining outer-hole boundary to inner-hole boundary (2 per pierced-boundary edge if the rim is a strip of quads, or the shared seam edges of ruled rim faces).
- **Created vertices:** one inner vertex per non-pierced outer vertex; plus rim corner vertices where rim walls meet at pierced-boundary vertices.
- **Created shells/voids:** the inner shell becomes a void boundary if `P` is empty (the body gains one internal void = one extra shell in the region graph, kernel/47); if `P` is non-empty the inner shell connects to the outer shell through the rim walls and the body remains a single connected shell with the cavity open to the outside.

The Euler-Poincare check after assembly (`V - E + F = 2(s - h)`, with `s` shells and `h` through-holes/genus) is the cheap global validation that the operator counts balanced.

### 1.5 Where this differs from a boolean

Doing the shell as `B - inward_offset_solid(B)` (regularized difference) is *equivalent in the result* but inferior in practice:
- it forces construction of the offset solid as a standalone watertight body first, which is precisely where global self-intersection (Section 4) bites, so you pay the hardest sub-problem up front;
- it runs a full general boolean (kernel/39, kernel/47) instead of a bounded local reintersect, losing the topology correspondence and the persistent-naming continuity between outer and inner faces;
- it is harder to support per-face thickness and pierce within.
The face-wise tweak path keeps the work *local and bounded* per edge (Forsyth's contribution) and only escalates to global self-intersection resolution (Section 4) when the local predictor says the offset is unsafe. This local-first, global-fallback structure mirrors the blend pipeline of kernel/41 (predict, build locally, escalate on failure).

---

## Section 2. Faces that vanish or self-intersect: the `t_max` collapse detector

### 2.1 The limit is the same medial limit as filleting (reuse kernel/41)

Offsetting the boundary inward by `t` self-intersects exactly when `t` reaches the distance to the medial axis. This is *definitionally identical* to the blend self-intersection limit of `kernel/41` Section 2: the medial axis is the locus where inward normals of two boundary pieces first meet, and offsetting by `t` collapses two boundary pieces onto a common point precisely when `t` equals their shared distance to a medial point. So the shell feasibility predictor is the **`r_max` predictor with `r := t` and `spine := the whole offset face**:

```
t_max(p) = lfs(p) = dist(p, MA(B))            for every boundary point p
```

and the local (cheap, conservative) pre-filter is the principal-curvature bound:

```
t_max_local(p) = 1 / kappa_concave_max(p)     (concave-facing principal curvature)
```

with `t_max <= t_max_local` always (global subsumes local). This is exactly the result that production CAD exposes to users as the rule **"shell thickness must be less than the smallest concave radius of curvature"** (SOLIDWORKS "Check > minimum radius of curvature"; Onshape: *"ensure [min curvature radius] is at least as large as your shell thickness... when shell thickness exceeds a feature's radius of curvature, offset surfaces intersect and the operation fails"*). The thin-wall form is the same field specialized: **opposing walls closer than `2t` collide** (Onshape), i.e. `t > w/2 = dist(·,MA)`, the rib-pinch limit of kernel/41 Section 2.2.

Net for Keel: do not build a second predictor. The shell calls the **same medial/LFS feasibility pass** as the blend engine, sampling every face (densified near concave edges and thin walls) instead of along a single spine, and returns a per-region `t_max` ceiling.

### 2.2 Two distinct collapse events

The single predicate `t > t_max(p)` manifests as two topology events that need different handling:

**(A) A face shrinks to nothing (vanishes).** A finite face whose offset, at distance `t`, has its trim boundary contract past zero area. Canonical case: a small chamfer or narrow face between two larger faces; offsetting inward moves the two neighbors toward each other until the narrow face is squeezed out. Detection: the recomputed inner trim loops of `F'_i` enclose zero (or negative-orientation) area, i.e. the reintersected inner edges of `F'_i` cross each other before bounding any region. Equivalent medial statement: the medial sheet generated by `F_i`'s two flanking neighbors is reached at offset `t`, so `F_i`'s inner image has nowhere to live.

**(B) Two offset faces cross (self-intersect).** Two inner faces `F'_i, F'_j` that are *not* neighbors in the outer shell nonetheless overlap in space after offsetting (a concave pocket whose two facing walls offset toward each other and pass through). Detection: a face-face intersection between non-adjacent inner faces is non-empty, or globally, the inner-shell offset surface has a self-intersection curve (the Maekawa/Patrikalakis offset singularity locus, nurbs/05 Section 9). This is the *global* failure that the local edge-by-edge reintersect of Section 1 cannot see, and it is the trigger to escalate to the whole-body winding-number trim of Section 4.

### 2.3 The reject-or-collapse policy (recommended default for Keel)

Tied to the best-effort doctrine (kernel/41 Section 4, kernel/29):

1. **Predict first.** Run the `t_max` pass (2.1) before offsetting any face. Never discover collapse only by a downstream intersection failure.
2. **Clean vanish -> collapse-merge.** For a face that vanishes (event A) where its two flanking neighbors meet cleanly, *let it collapse*: drop the vanished inner face, and recompute the shared inner edge directly as `Off(F_left) ∩ Off(F_right)` (the two flanking offsets meet at a now-sharp inner edge). This is the offset analogue of a blend's notch/cap and is the *correct* answer for a squeezed-out sliver: the inner shell simply has one fewer face than the outer shell there, and the Euler counts adjust (one face, and its bounding edges/vertex, removed). This is a supported, benign topology change, exactly as tweak supports edges/faces disappearing (kernel/03 Section 1.1, the Parasolid multi-face tweak that lets faces vanish).
3. **Crossing / global self-intersect -> winding-number trim or local reduce.** For event B (two non-adjacent inner faces cross), either (a) escalate to the whole-body winding-number offset trim (Section 4), which extracts the valid inner shell from the self-intersecting raw offset by winding number, or (b) if the crossing is confined to a small region and the caller permits, *locally reduce* `t` to `(1-eps)·t_max` over that region, producing a variable-thickness wall there (the radius-limit analogue of kernel/41 Section 4.1), and *report* the reduction. Reject only as a last resort, and never silently.
4. **Always return a valid body with disclosure.** As in kernel/41's `BlendResult`: a `ShellResult` is always a valid solid (the original where shelling was impossible) plus a per-region diagnostic listing each face's outcome (offset / collapsed-merged / reduced-thickness / skipped) and the binding `t_max`. This is strictly stronger than a throw-or-fail kernel and on par with Parasolid/ACIS, while exposing the LFS feasibility profile those kernels keep internal.

**Ordering corollary (production practice).** Because `t_max` is set by the *smallest concave radius*, the standard fix users apply is *shell before filleting*: add small fillets *after* the shell so they do not lower `t_max` (Onshape, SOLIDWORKS: "add fillets post-shell rather than before," "delicate features should be modeled after shelling"). Keel's feature engine should encode this as a sequencing hint: when a small concave fillet would block a requested shell, suggest reordering the shell ahead of that fillet.

---

## Section 3. Per-face thickness (item 43) and pierce-open faces (item 42)

### 3.1 Per-face (multi-thickness) shell

The shell thickness need not be uniform. Both Parasolid (*"create a shell solid by applying different wall thickness values to the solid faces"*) and ACIS support a default thickness plus per-face overrides; SOLIDWORKS exposes this as multi-thickness shell. Implementation in the Section 1 algorithm:

- Carry a thickness map `tf: Face -> t`, defaulting to the global `t`. In S1, face `F_i` is offset by `tf(F_i)` instead of the global `t`.
- The reintersect in S2 is unaffected in structure: the inner edge `E' = Off(F_i, tf(F_i)) ∩ Off(F_j, tf(F_j))` is the intersection of two *differently-offset* surfaces. The two offsets being at different distances is fine for the reintersect; it just means the inner edge no longer mirrors the outer edge's shape, only its adjacency.
- **Feasibility per face.** `t_max` is queried with each face's own `tf(F_i)`. A thick wall on one face neighboring a thin wall can self-intersect where a uniform thickness would not, so the predictor runs with the per-face map.
- **Step at thickness change.** Where two adjacent faces have different thicknesses, the inner shell has a step; the reintersect produces the connecting inner edge automatically (the two offset surfaces simply meet at a different curve). No special operator is needed; this falls out of the per-face reintersect.

### 3.2 Pierce-open faces: rim-wall construction

A *pierced* (open, removed) face is one omitted from both shells so the void communicates with the outside through it: the result is a cup/box-open-on-top rather than a sealed hollow. Parasolid: *"an option parameter may be set to pierce some of the faces of the resulting body, opening up the interior void."* The faces in pierce set `P` are simply not offset, and the holes they leave in the two shells are bridged by **rim walls**.

For each pierced face `F in P`:
- `F` is removed from the outer shell, leaving a hole bounded by the outer boundary loop `L_out` = the edges where `F` met its (non-pierced) neighbors.
- The corresponding inner face `F'` is also absent, leaving a hole in the inner shell bounded by the inner boundary loop `L_in` = the reintersected inner edges mirroring `L_out`.
- **The rim wall bridges `L_out` to `L_in`.** Geometrically the rim wall sits on the surface of `F` itself (a pierced face contributes a *wall of thickness t around its rim*): the rim wall is the strip of `F`'s surface between the outer boundary loop and the inner boundary loop, i.e. the portion of `F` that lies within distance `t` of `F`'s edges. Equivalently and more robustly, the rim wall for a *straight-walled* pierced face (the common case: pierce the top of a box) is a ruled/offset strip joining `L_out` to `L_in`, one rim face per boundary edge of `F`, each a small planar or ruled quad with two new rim edges (the seam where it meets the adjacent un-pierced face's outer and inner faces) and reusing `L_out`/`L_in` edges as its other two sides.

Topology created per pierced face (matching S4): the rim faces (one per boundary edge of `F`, or one ruled face per boundary segment), the rim edges joining outer to inner along each boundary vertex, and the rim corner vertices. The result keeps the body a single connected shell (the void is no longer closed) and the Euler check accounts for the open cavity (the inner shell is not a separate void but part of the same shell as the outer, joined through the rim).

**Common special case (validated below): pierce the top face of a box.** `P = {top}`. The top is not offset; its four bounding edges become the outer rim loop; the inner shell's four top edges become the inner rim loop; four rim-wall quads (the inner faces of the four side walls' top strips) bridge them. The result is an open box (a tray) with uniform wall thickness `t`.

### 3.3 Interaction of pierce and per-face thickness

Pierced faces have no thickness (they are absent), so they are excluded from the `tf` map and from the `t_max` pass. A pierced face *relaxes* feasibility for its neighbors (one fewer offset surface to collide with), which is why "omit the problem face from the shell" is a documented shell-failure remedy (Onshape). Keel can surface this: if a face cannot be shelled at any thickness because it is geometrically too tight, offer to pierce it.

---

## Section 4. Whole-body offset (item 45): global self-intersection by winding number

### 4.1 The problem: the raw offset self-intersects globally

A *whole-body offset* grows or shrinks an entire solid by `d` (positive = grow/dilate, negative = shrink/erode), the Minkowski sum/difference of the body with a ball of radius `|d|` (Rossignac-Requicha 1986). Building it face-wise as in Section 1 produces a **raw offset**: every face displaced by `d·N`, every edge replaced by a cylindrical/blend strip of radius `d` (a canal surface, nurbs/05 Section 4: edges grow into pipe-surface arcs, convex vertices into sphere octants), every concave edge a self-intersecting fold. The raw offset is correct *locally* but globally **self-intersects** wherever `d` exceeds the local feature size: the offset surface passes through itself, and the naive union is not a valid solid boundary.

The valid offset is the subset of the raw offset that actually bounds the offset solid. The robust way to extract it is by **winding number**.

### 4.2 Chen-McMains: polygon offsetting by computing winding numbers (the 2D template)

**Citation.** Chen, X., & McMains, S. (2005). Polygon offsetting by computing winding numbers. *Proceedings of the ASME 2005 IDETC/CIE, 31st Design Automation Conference*, Vol. 2, 565-575. (mcmains.me.berkeley.edu/pubs/DAC05OffsetPolygon.pdf)

**Content.** Offsets multiple non-overlapping polygons with arbitrary holes. The method: (1) construct the **raw offset curve** by offsetting every edge by `d` and joining consecutive offset edges with arcs (convex vertices) or letting them cross (concave vertices), producing a curve that *self-intersects* at the concave folds; (2) feed this raw offset curve to a tessellator (the paper uses GLU's tessellator) that computes, for each connected region of the arrangement, its **winding number** = the signed number of times the raw offset curve wraps around the region; (3) **extract the valid offset region by a winding-number rule**: for an outward offset, keep regions with winding number `>= 1` (inside at least once); the self-intersecting loops at concave folds have winding number `0` or negative and are discarded. The boundary of the kept region is the valid, self-intersection-free offset. The winding-number test is what cleanly removes both *local* self-intersection (the little reversed loops at sharp concave corners) and *global* self-intersection (distant parts of the offset passing through each other) in one uniform criterion, with no special-case geometry.

**Kernel relevance.** This is the 2D template for Keel's offset trimmer. The crucial design idea: *do not try to detect and surgically remove self-intersections pairwise; instead build the self-intersecting raw offset deliberately, compute winding numbers over its arrangement, and keep the region by a winding rule.* Robust, uniform, no special cases. Keel's 2D offsetter (sketch offset, the inner loop of per-face trimming) should use exactly this.

### 4.3 Jacobson-Kavan-Sorkine-Hornung: generalized winding numbers (the 3D lift)

**Citation.** Jacobson, A., Kavan, L., & Sorkine-Hornung, O. (2013). Robust inside-outside segmentation using generalized winding numbers. *ACM Transactions on Graphics (SIGGRAPH), 32*(4). (igl.ethz.ch/projects/winding-number)

**Content.** Generalizes the winding number to arbitrary 3D triangle meshes: the **generalized winding number** at a point `p` is the sum over all triangles of the signed solid angle each triangle subtends at `p`, divided by `4π`. For a watertight surface this is exactly the integer winding number (1 inside, 0 outside); for a self-intersecting, open, or non-manifold surface it is a smooth real-valued field that still segments inside from outside robustly, and it equals the *number of times `p` is enclosed*. This is the exact 3D analogue of Chen-McMains: build the raw (self-intersecting) offset surface, compute the generalized winding number field over it, and **extract the valid offset solid as the region where the winding number `>= 1`** (for grow) or by the appropriate level set. It is robust to exactly the degeneracies a raw offset produces: open boundaries, non-manifold folds, and surfaces passing through themselves.

**Kernel relevance.** This is Keel's whole-body offset trimmer. Pipeline for item 45:
```
1. RAW OFFSET. Displace every face by d·N (exact for analytic, approximate for
   NURBS); fill convex edges with pipe-surface arcs (radius |d|) and convex
   vertices with sphere patches (Rossignac-Requicha growing); let concave
   regions self-intersect (do NOT trim yet).
2. WINDING FIELD. Compute the generalized winding number of the raw offset
   (on a tessellation of it for the field evaluation; the exact surfaces are
   the truth, the tessellation is a cache, nurbs/05 discipline).
3. EXTRACT. Keep the region with winding number >= 1 (grow) or <= -1 / the
   right level set (shrink). The boundary of the kept region is the valid,
   self-intersection-free offset boundary.
4. RE-FIT TO B-REP. Snap the extracted boundary back onto the exact offset
   surfaces and pipe/sphere patches where it coincides with them, recovering
   an exact-or-tolerant B-rep rather than leaving a mesh. Use the boolean-
   result-assembly machinery (kernel/47) to stitch and the canonical-recovery
   pass (M8) to re-snap analytic patches.
```
The winding-number trim is the *global* self-intersection resolver that the *local* edge-by-edge reintersect of Section 1 cannot do. The local path (Section 1) is preferred when the `t_max` predictor says the offset is safe; the winding-number path (this section) is the robust fallback when it is not, and is the primary method for whole-body grow/shrink where self-intersection is expected by design.

### 4.4 Pavic-Kobbelt: volumetric offset as a robustness backstop

**Citation.** Pavic, D., & Kobbelt, L. (2008). High-resolution volumetric computation of offset surfaces with feature preservation. *Computer Graphics Forum, 27*(2), 165-174.

**Content.** Computes self-intersection-free offset surfaces for polygonal meshes by a **volumetric** approach: define the offset as the union of spheres (at vertices), cylinders (at edges), and prisms (at faces), evaluate the **unsigned distance field** of this union on a high-resolution adaptive grid, and extract the `d`-level set. Because it works from a distance field, it is robust to *any* topological inconsistency in the input (non-manifold, degenerate triangles, self-intersections) and never misses small/thin components; sharp features (shocks of the distance field, which are the medial axis) are detected and preserved by a feature-sensitive extraction.

**Kernel relevance.** This is the **last-resort** offset path for Keel, the analogue of the nTop field fallback in kernel/41 Section 5.4: when the exact face-wise offset plus winding-number trim defeats the geometry (very dirty imported bodies, pathological self-intersection), drop to a distance-field offset and re-mesh, accepting loss of exactness for a guaranteed valid result. The shocks-are-the-medial-axis observation also gives a second, field-based estimator of `t_max` (Section 2): the distance field's ridge set is the medial axis, so the inscribed-sphere thickness field used for the `t_max` pass and this volumetric offset share one data structure.

### 4.5 Mesh-offset self-intersection removal by region growing (the simpler alternative)

**Citation.** Jung, W., Shin, H., & Choi, B. K. (2004). Self-intersection removal in triangular mesh offsetting. *Computer-Aided Design and Applications, 1*(1-4), 477-484. (cad-journal.net) Related: Qu, X., & Stucker, B. (2003). A 3D surface offset method for STL-format models. *Rapid Prototyping Journal, 9*(3), 133-141; and Chen, Y., & Wang, C. C. L. A point-based offsetting method of polygonal meshes (faculty.cc.gatech.edu/~jarek/papers/OffsetYong.pdf).

**Content.** A lighter alternative to winding numbers for meshes: build the raw offset mesh (Qu-Stucker offset each *vertex* by the weighted-normal-sum direction, which avoids cracks better than offsetting facets), then **remove self-intersection by region growing**: pick a seed triangle known to be on the valid outer boundary, grow the valid region to neighbors, stopping at self-intersection curves; the valid region is the set of triangles reachable without crossing a self-intersection, defining the outer boundary of the offset volume. Chen-Wang's point-based method offsets sample points and reconstructs, similarly robust to folds.

**Kernel relevance.** Region growing is the cheaper, more local self-intersection remover when a full winding-number field is overkill (e.g. a shell inner wall with only a few isolated concave folds). Keel can offer it as a mid-tier between local reintersect (Section 1) and global winding-number trim (Section 4.3). Qu-Stucker vertex-offset (weighted normals at vertices, not facets) is the right way to build the raw offset mesh in the first place, avoiding the cracks that facet-offset leaves at edges.

---

## Section 5. Thicken a sheet (item 44): rim walls around the open boundary

### 5.1 The operation (depends on sheet bodies, kernel/51)

A *sheet body* is a zero-thickness body: a connected set of faces with a free (open) boundary, not enclosing a volume (Parasolid sheet body; ACIS sheet; kernel/51). **Thicken** turns a sheet into a solid of wall thickness `t`. ACIS names this `shell:sheet-thicken` (*"creates a solid body from a sheet"*). It is the natural dual of shell: shell removes material from a solid to leave a thin wall; thicken adds material around a sheet to make a thin wall.

### 5.2 The algorithm: offset both sides, bridge the rim

```
INPUT:  sheet body H (faces with a free boundary loop B_free), thickness t,
        mode in { two-sided (+/- t/2), one-sided (+t or -t) }
OUTPUT: solid body

T1. OFFSET BOTH SIDES. For two-sided thicken, offset every face of H by +t/2
    along its normal (the "top" shell) and by -t/2 against its normal (the
    "bottom" shell). For one-sided, offset by +t (or -t) for one shell and use
    H itself as the other shell. Each offset is exact for analytic faces and
    approximate for NURBS (Section 1, S1). The two offset shells are
    combinatorial copies of H (the same correspondence as Section 1.3).

T2. REINTERSECT EACH SHELL INTERNALLY. Within the top shell, recompute shared
    edges/vertices between adjacent offset faces by reintersection (the tweak
    pass of Section 1, S2); likewise the bottom shell. For a single-face sheet
    there is nothing to reintersect; for a multi-face sheet the internal edges
    are rebuilt exactly as in shell.

T3. ORIENT. The top shell keeps H's orientation; the bottom shell is reversed
    so the two shells face away from each other across the new wall (the solid
    is between them).

T4. BUILD RIM WALLS AROUND THE FREE BOUNDARY. The open boundary loop B_free of
    H has no neighbor to reintersect against, so it must be CLOSED by a rim
    wall. For each edge e of B_free:
      - it has a top image e_top (on the top shell) and a bottom image e_bot
        (on the bottom shell);
      - build a rim face spanning e_top to e_bot: a ruled surface between the
        two offset curves (for a straight or planar boundary this is a planar
        or ruled strip; in general a ruled/loft between e_top and e_bot),
        perpendicular to the sheet to thickness t;
      - the rim face's four sides are e_top, e_bot, and two rim edges joining
        the endpoints (shared with the adjacent rim faces around the loop).
    The chain of rim faces forms a closed band around B_free, sealing the top
    shell to the bottom shell so the result is a closed solid.

T5. ASSEMBLE. Stitch top shell + bottom shell + rim band into one solid
    (kernel/47); the result is watertight, genus equal to H's (a sheet with a
    hole thickens to a solid with a through-hole). Validate Euler-Poincare.
```

### 5.3 Topology and the dependency on sheet bodies

- **Faces created:** `|Faces(H)|` (top) + `|Faces(H)|` (bottom) + one rim face per edge of `B_free` (or one ruled rim per boundary segment). For a single planar sheet disc: 1 top + 1 bottom + 1 rim band (or n rim quads) = a thin disc solid.
- **Edges/vertices:** the rim edges and corners join top to bottom around the boundary, mirroring Section 3.2's pierced-face rim exactly. *Thicken's rim wall and shell's pierced-face rim wall are the same construction*: both bridge an outer-shell boundary loop to an inner/opposite-shell boundary loop with a perpendicular band. Keel should implement one `build_rim_wall(loop_a, loop_b)` primitive used by both.
- **Sheet-body dependency (kernel/51).** Thicken consumes a sheet body and its free-boundary loop, both of which are sheet-body concepts. The sheet must be *manifold with boundary* for a clean thicken (a non-manifold sheet, where three faces meet at an edge, thickens to a non-manifold solid and needs the PES/non-manifold handling of kernel/51 and kernel/02). The mid-surface case (a non-manifold sheet midway through a thin-walled part, kernel/10 / kernel/03 Theme 8) is the inverse operation: thicken reconstructs an approximate solid from a mid-surface, which is the round-trip dual of mid-surfacing. Cross-reference kernel/51 for the sheet representation, free-boundary extraction, and double-sided-face conventions that T1-T4 assume.

### 5.4 Self-intersection of thicken (same `t_max` predictor)

Thicken self-intersects on the *concave* side of a curved sheet exactly when `t/2` (two-sided) or `t` (one-sided) exceeds the sheet's local concave radius of curvature: the inner offset folds, identical to Section 2's `t_max`. A sheet with a tight concave bend cannot be thickened past `t = radius` on the concave side. The same predictor and the same reject/collapse/winding-number-trim policy apply. The rim band can also self-intersect if `B_free` has a concave corner tighter than `t`; that is the 2D offset self-intersection of the boundary loop (Chen-McMains, Section 4.2), trimmed by winding number.

---

## Section 6. Validation

### 6.1 Shelled box, uniform wall thickness (the closed-void base case)

Input: an axis-aligned box `[0,a]x[0,b]x[0,c]`, 6 planar faces, 12 edges, 8 vertices (`V-E+F = 8-12+6 = 2`, genus 0). Shell inward by `t`, no pierce.

- S1: each planar face offsets to a parallel plane at distance `t` inward (exact). The cylinder/sphere offset cases do not arise (all planar).
- S2 (tweak reintersect): the 6 inner planes reintersect to a smaller box `[t,a-t]x[t,b-t]x[t,c-t]`: 6 inner faces, 12 inner edges, 8 inner vertices. Each inner edge is the intersection of two inner planes (e.g. `x=t ∩ y=t`), each inner vertex the intersection of three. The correspondence of Section 1.3 holds exactly: 6<->6, 12<->12, 8<->8.
- S3: inner faces reversed to bound the void.
- S5: outer box (6,12,8) + inner box (6,12,8) = 12 faces, 24 edges, 16 vertices, **two shells** (outer + inner void). Euler-Poincare for a solid with one internal void: `V - E + F = 16 - 24 + 12 = 4 = 2·(s) ` with `s = 2` shells (the formula `V-E+F = 2(s-h)` gives `4 = 2(2-0)`). Watertight, two nested boxes, the canonical hollow box. PASS.
- **Feasibility:** `t_max = min(a,b,c)/2` (the box's medial axis is its central skeleton; the nearest opposite wall is at distance = half the smallest dimension). `t < min(a,b,c)/2` succeeds; `t = min/2` collapses the inner box to a degenerate sheet/line (the inner shell pinches shut), correctly flagged by the predictor. This is the textbook reuse of the kernel/41 medial limit.

### 6.2 Box with one pierced face (the cup / open-tray case)

Same box, `t` inward, `P = {top face (z=c)}`.

- The top face is not offset. The other 5 faces offset inward (S1), reintersect (S2) to 5 inner faces forming an open inner box (bottom + 4 sides, no inner top): 5 inner faces, with the top 4 inner edges forming the inner rim loop `L_in` at `z = c` (the side walls' inner faces run up to `z=c`, not stopping at `z=c-t`, because the top is open).
- S4 rim wall: the top of the box loses its face; the outer rim loop `L_out` is the 4 top edges of the box at `z=c`; the inner rim loop `L_in` is the 4 top edges of the inner side faces. Four rim-wall quads (the top strips of the 4 side walls, each of width `t`) bridge `L_out` to `L_in`, forming the rim of the cup.
- Result: an open box (tray) with uniform wall thickness `t` and an open top. Single connected shell (the void communicates with outside through the open top), genus 0. Faces: 5 outer (bottom + 4 sides) + 5 inner + 4 rim = 14; the Euler check balances for a single shell with the cavity open (no separate void). PASS.
- This is the `P = {top}` special case of Section 3.2, and it is the everyday "shell a box into a tray" operation that every CAD shell tool must get right.

### 6.3 Adversarial cases (tie to kernel/34, kernel/41 Section 6)

- **Thickness sweep through `t_max`:** sweep `t` from `< min/2` to `> min/2` on the box; assert clean shell below, inner-box-collapse handling at, and reject/reduce above, with the transition `t` equal to the predicted `t_max = min/2` (the medial oracle is a differential test, exactly as kernel/41 Section 6).
- **Concave fillet then shell:** a box with a small concave fillet of radius `rho < t` on one edge; assert the predictor flags `t_max = rho` at the fillet and the policy either reduces `t` locally, collapses the fillet's inner face, or recommends shell-before-fillet reordering (Section 2.3).
- **Thin rib:** a slab of thickness `w`; shell with `t > w/2`; assert opposing-inner-wall crossing (event B) detected and routed to winding-number trim or partial.
- **Sheet thicken with concave bend:** a curved sheet of concave radius `rho`; thicken with `t/2 > rho`; assert concave-side self-intersection flagged and trimmed (Section 5.4).
- **Whole-body grow with re-entrant feature:** grow a body by `d` larger than a slot width; assert the raw offset self-intersects in the slot and the winding-number trim (Section 4.3) extracts the valid filled-in offset (the slot closes over), matching the Minkowski-dilation ground truth.

---

## Shell / offset / thicken for Keel (closing synthesis)

1. **Shell = a maximal multi-face tweak.** Offset every face inward by `t` (exact for analytic, approximate-to-tolerance for NURBS, truth-as-procedural/cache-as-NURBS per nurbs/05), then rebuild the inner shell by the *same* extend-and-reintersect machinery as tweak (kernel/03): the inner shell is a combinatorial copy of the outer shell (one inner face/edge/vertex per outer, identical adjacency, convexity flipped), so only geometry is recomputed, not topology. This is Forsyth's face-wise offset-and-reintersect, kept local and bounded, in preference to the equivalent-but-inferior `B - offset_solid(B)` boolean. Orient the inner shell inward, bridge pierced faces with rim walls, assemble via kernel/47.

2. **The collapse detector is the kernel/41 `r_max` predictor with `r := t`.** `t_max(p) = lfs(p) = dist(p, MA(B))`, with `1/kappa_concave_max` as the cheap local pre-filter and `w/2` as the thin-wall form. This is the exact medial limit the blend engine already uses, and it is the user-facing rule "shell thickness < smallest concave radius of curvature" (SOLIDWORKS/Onshape). Reuse the one feasibility pass; do not build a second predictor. Two collapse events: a face vanishes (squeezed-out sliver -> collapse-merge its flanking offsets to a sharp inner edge, a supported benign topology change) and two non-adjacent offset faces cross (global self-intersect -> escalate to the winding-number trim of Section 4 or locally reduce `t`). Policy: predict first, collapse-merge clean vanishes, winding-trim or reduce crossings, always return a valid body with per-region disclosure, reject only as last resort. Encode the shell-before-fillet ordering hint.

3. **Per-face thickness and pierce fall out of the same algorithm.** Per-face thickness: offset each face by its own `tf(F_i)`; the reintersect of differently-offset surfaces needs no new operator and yields the inner step automatically; run `t_max` per-face. Pierce: omit the face from both shells and bridge its outer-boundary loop to the inner-boundary loop with a rim wall (`build_rim_wall(loop_a, loop_b)`), turning a closed void into an open cup. A pierced face relaxes neighbor feasibility, so "pierce the tight face" is a built-in shell-failure remedy.

4. **Whole-body offset is built self-intersecting then trimmed by winding number.** Grow/shrink (Minkowski dilation/erosion, Rossignac-Requicha): displace faces by `d`, grow convex edges into pipe-surface arcs and convex vertices into sphere patches, let concave regions self-intersect, then extract the valid offset by the **generalized winding number** (Jacobson 2013, the 3D lift of Chen-McMains 2005): keep the region with winding number `>= 1`, which removes local and global self-intersection in one uniform criterion with no special cases, then re-fit the extracted boundary to exact/tolerant B-rep (kernel/47 + M8 canonical recovery). Region-growing self-intersection removal (Jung/Qu-Stucker) is the lighter mid-tier; the Pavic-Kobbelt volumetric distance-field offset is the nTop-style last-resort that never fails at the cost of exactness, and its distance-field ridge doubles as the medial/`t_max` estimator.

5. **Thicken a sheet = offset both sides + rim band, the dual of shell.** Offset the sheet by `+/- t/2` (two-sided) or `+t` (one-sided) into top and bottom shells, reintersect each internally (tweak), reverse the bottom, and seal the *free boundary* loop with a perpendicular rim band (the same `build_rim_wall` primitive as the pierced-face rim). Depends on the sheet-body representation, free-boundary extraction, and double-sided-face conventions of kernel/51 (cross-referenced); a non-manifold mid-surface thickens to a non-manifold solid (kernel/02/51). Same `t_max` self-intersection limit on the concave side.

6. **One offset module, four operations, tiered fallback.** Shell, per-face shell, pierce, whole-body offset, and thicken all share: the face-offset primitive (analytic-exact / NURBS-approximate), the tweak-based extend-and-reintersect, the `build_rim_wall` bridge, the medial `t_max` feasibility pass, and the assembly/validation of kernel/47. The robustness tiering mirrors kernel/41: **predict (medial `t_max`) -> build locally (face-wise reintersect, the safe common case) -> escalate to global (winding-number trim) -> last resort (volumetric distance-field offset)**, always returning a valid best-effort body with full disclosure. This is a design recommendation to validate against Keel's tests; the shelled-box and pierced-box cases of Section 6 are the first two oracles (with the `t_max` sweep as the differential test against the medial prediction).

---

## References

1. Forsyth, M. E. (1995). Shelling and offsetting bodies. *Proceedings of the 3rd ACM Symposium on Solid Modeling and Applications (SM '95)*, 373-381. (Face/face re-intersection and blending to offset all or a subset of a B-rep solid's faces; the canonical shell-by-reintersection algorithm.) https://dl.acm.org/doi/10.1145/218013.218088
2. Rossignac, J. R., & Requicha, A. A. G. (1986). Offsetting operations in solid modelling. *Computer Aided Geometric Design, 3*(2), 129-148. (Growing/shrinking = Minkowski sum/difference with a ball; constant-radius blending and shelling as derived offsets; offset/medial/self-intersection relationship.) https://faculty.cc.gatech.edu/~jarek/papers/Offsets.pdf
3. Chen, X., & McMains, S. (2005). Polygon offsetting by computing winding numbers. *Proc. ASME 2005 IDETC/CIE, 31st Design Automation Conf.*, Vol. 2, 565-575. (Raw offset curve -> winding number per region -> keep winding `>= 1`; removes local and global self-intersection uniformly.) https://mcmains.me.berkeley.edu/pubs/DAC05OffsetPolygon.pdf ; https://asmedigitalcollection.asme.org/IDETC-CIE/proceedings-abstract/IDETC-CIE2005/4739X/565/314012
4. Jacobson, A., Kavan, L., & Sorkine-Hornung, O. (2013). Robust inside-outside segmentation using generalized winding numbers. *ACM TOG (SIGGRAPH), 32*(4). (Signed-solid-angle generalized winding number; robust inside/outside for self-intersecting/open/non-manifold meshes; the 3D lift of Chen-McMains for offset extraction.) https://igl.ethz.ch/projects/winding-number/
5. Pavic, D., & Kobbelt, L. (2008). High-resolution volumetric computation of offset surfaces with feature preservation. *Computer Graphics Forum, 27*(2), 165-174. (Union of spheres/cylinders/prisms, unsigned distance field, level-set extraction; robust to all topological inconsistencies; shocks = medial axis, features preserved.) https://www.graphics.rwth-aachen.de/publication/0360/
6. Jung, W., Shin, H., & Choi, B. K. (2004). Self-intersection removal in triangular mesh offsetting. *Computer-Aided Design and Applications, 1*(1-4), 477-484. (Raw offset mesh, seed triangle, region-grow the valid region stopping at self-intersection curves.) https://www.cad-journal.net/files/vol_1/CAD_1(1-4)_2004_477-484.pdf
7. Qu, X., & Stucker, B. (2003). A 3D surface offset method for STL-format models. *Rapid Prototyping Journal, 9*(3), 133-141. (Offset vertices by weighted-normal-sum direction, not facets, to avoid cracks; inward/outward STL offsets.) https://www.semanticscholar.org/paper/A-3D-surface-offset-method-for-STL%E2%80%90format-models-Qu-Stucker/10afe412feffcd0537ec470f047305c213db01c8
8. Chen, Y., & Wang, C. C. L. A point-based offsetting method of polygonal meshes. (Point/sample offset and reconstruct; robust to folds.) https://faculty.cc.gatech.edu/~jarek/papers/OffsetYong.pdf
9. Maekawa, T. (1999). An overview of offset curves and surfaces. *Computer-Aided Design, 31*(3), 165-173. (Offset curvature relations; offset singular where `d = 1/kappa`; self-intersection and the focal/medial set.) https://doi.org/10.1016/S0010-4485(99)00013-5
10. Patrikalakis, N. M., Maekawa, T., & Cho, W. *Shape Interrogation for Computer Aided Design and Manufacturing*, Ch. 11.3 (offset singularities, self-intersection criteria and tracing). https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/
11. Siemens / Parasolid. *Functional Description: Operations on Bodies (Hollow / Offset / Thicken Bodies)* and PK reference. (`PK_BODY_hollow` single wall-thickness argument; negative thickness = outward; option to pierce faces opening the interior void; offset/hollow/thicken with different per-face wall thickness values; mix of classic and facet faces.) http://www.q-solid.com/Parasolid_Docs/chapters/kd_chap.07.html ; v33.1 highlights: https://blogs.sw.siemens.com/plm-components/parasolid-v33-1-release-highlights/
12. Spatial / ACIS. *Shelling Component: shell:hollow-body and shell:sheet-thicken.* (`hollow-body` = thin-walled solid shell from a solid, controllable per-face wall thickness; `sheet-thicken` = solid body from a sheet.) http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/SHL/SHL/02SC/0001.HTM ; .../0004.HTM
13. Dassault Systemes / SOLIDWORKS. *Shell Exceptions* and shell best practice. (Multi-thickness shell; shell thickness must be less than the smallest radius of curvature; concave/thin regions where inner offset crosses outer fail; "Check > minimum radius of curvature".) https://help.solidworks.com/2022/english/SolidWorks/sldworks/c_Shell_Exceptions.htm
14. Onshape (PTC). *Troubleshooting shell failures in 3D modeling.* (Thickness vs minimum curvature radius rule; opposing walls closer than `2t` collide; remedies: reduce thickness, omit the problem face, shell before adding fillets.) https://www.onshape.com/en/resource-center/tech-tips/troubleshooting-shell-failures-3d-modeling
15. "Shelling and offsetting bodies." (1995). *Proc. 3rd ACM Symposium on Solid Modeling and Applications*, 95-104. (Companion survey of the shell-by-face-offset-and-stitch algorithm; cross-referenced from nurbs/05 Section 10.) https://dl.acm.org/doi/10.1145/218013.218088
16. Dey, T. K., & Zhao, W. *Approximate medial axis for CAD models.* ACM Solid Modeling. (Practical approximate MAT for the `t_max` / LFS field over a B-rep, reused from kernel/41.) https://dl.acm.org/doi/pdf/10.1145/781606.781652
17. Amenta, N., & Bern, M. Surface reconstruction by Voronoi filtering. (`lfs(p) = dist(p, MA)`; poles approximate the medial axis; the definition `t_max = lfs` rests on.) See also Local feature size: https://en.wikipedia.org/wiki/Local_feature_size
