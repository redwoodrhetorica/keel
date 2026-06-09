# 51. Sheet / Open-Body Topology and Sheet Operations

## Title and Scope

This dossier is the implementation-grade design for SHEET BODIES (open shells, lamina faces, the double-sided faces that dossier 46 flagged as having no material side) and the four sheet operations that gate the next roadmap tier: KNIT/SEW (item 71), SURFACE EXTENSION (item 70), TRIM (item 72), and SPLIT (item 76). Sheet bodies are the prerequisite body kind for THICKEN (item 44, dossier 50), surface-from-boundary, and import healing (dossier 13): each of those either consumes or produces an open two-dimensional body that is not a closed solid.

Keel today is a non-manifold partial-entity-structure (PES) kernel with FIRST-CLASS REGIONS (dossier 02), edited through GWB Euler operators (dossier 01), with tolerant f64 edges (M7b) and a sense/orientation convention now unified on FIN/MATERIAL-PRIMARY with a derived `sense` cache (dossier 46). Every current body is a closed solid: one solid region plus the surrounding void region, every edge bordered by exactly two coedges, every face single-sided with a defined outward normal. A SHEET BODY breaks three of those assumptions at once: it has no enclosed solid region, it carries faces that are double-sided (no material side), and it has FREE EDGES bordered by a single coedge. This file states the sheet-body validity predicate in PES terms, the knit-and-promote algorithm, the surface-extension core shared with dossier 13's healing, and the trim/split operations, and it validates the design against two canonical cases: knitting six planar sheets into a closed cube (and detecting the closure to promote the result to a solid), and trimming a single sheet by a plane.

This is a DESIGN RECOMMENDATION grounded in production-kernel practice (Parasolid general/sheet bodies, OCCT `BRepBuilderAPI_Sewing`, ACIS double-sided faces, Weiler RES regions, Lee-Lee PES). It must be validated against Keel's debug validator and its existing region, boolean, and mass-property test suites before adoption. Per the house style: no em-dashes anywhere; this has been checked.

It builds on, and does not repeat: dossier 02 (PES, lamina faces, mixed-dimension bodies, Weiler radial-edge, first-class regions), dossier 46 (double-sided faces and the sense convention when there is no single material side), dossier 13 (stitch/sew and surface extension at the healing level), dossier 01 (Euler operators and fins).

---

## 1. Does the PES Region Model Represent a Sheet Body Cleanly

### 1.1 The body-kind lattice and where sheets sit

Parasolid's body taxonomy is the reference vocabulary, and Keel should adopt it verbatim because import/export and user mental models already speak it. A WIRE body is topologically one-dimensional (edges and vertices, no faces). A SHEET body is topologically two-dimensional (one or more faces forming an open shell, not enclosing any solid region). A SOLID body is topologically three-dimensional (faces enclosing at least one solid region). A GENERAL body is the non-manifold/mixed-dimension union of any of the above sharing topology in one valid body. A MANIFOLD body is the restricted subset where every edge borders exactly two faces and every face is single-sided. The decisive Parasolid definitions for our purposes: a region is "an open connected subset of 3-dimensional space whose boundary is a collection of vertices, edges, and oriented faces," and "regions are either solid or void (empty), and they may be non-manifold." The promotion rule is implicit in those two sentences and is the spine of section 2: a body is a SOLID exactly when one of its regions is marked SOLID, and a region can be solid only if it is a bounded (closed) subset of space, i.e. fully enclosed by faces.

The good news for Keel: the PES already has FIRST-CLASS REGIONS (dossier 02, Weiler RES lineage where "regions are explicit 3D cells of space, solid or void, making RES inherently cellular"). A sheet body is therefore not a new kind of TOPOLOGY; it is a new REGION CONFIGURATION over the topology Keel already has. No new entity type is required. What changes are the INVARIANTS, enumerated next.

### 1.2 What a sheet body looks like in the PES

A closed solid in Keel has: a solid region R_s, the surrounding void region R_v, every face bordering R_s on one side and R_v (or another region) on the other, every edge with exactly two coedges (one per adjacent face-side), and a Euler characteristic obeying the closed-shell formula V - E + F = 2(s - h) with s shells and h handles (dossier 01, dossier 45).

A SHEET body differs structurally:

- **No enclosed solid region.** The sheet does not partition space into an inside and an outside. There is exactly ONE region in the body's neighborhood: the single ambient VOID that surrounds the sheet on both sides. The sheet faces are immersed in that one void; they do not separate it into two. This is the cleanest PES statement of "no material side": a sheet face borders the SAME region (the ambient void) on both of its sides.

- **Double-sided faces (dossier 46).** Each sheet face is double-sided: there is no single material side, so `n_out` is not a single well-defined outward direction. Under Keel's FIN/MATERIAL-PRIMARY convention (dossier 46), the face still has a NATURAL normal from its surface and a FIN winding, but the region on both sides is the same void, so `sign(dot(n_out_material, n_nat))` is UNDEFINED. The face is tagged DOUBLE_SIDED and consumers must not query a material side. Two sub-flavors, exactly as ACIS distinguishes (dossier 46): a "both-void" double-sided face (a free-floating lamina, our sheet case) and a "both-solid" double-sided face (an interior membrane embedded in solid). Sheets are the both-void case.

- **Free edges with one coedge.** A boundary edge of the open shell is bordered by exactly ONE coedge (one face-side), not two. This is the topological signature OCCT's sewing calls a "free edge" ("edge shared by one face"), Barequet-Sharir call a gap-boundary edge ("edges incident to only one face"), and Weiler RES handles natively (the radial cycle around such an edge has length one). In Keel's PES, the partial-edge for a free edge carries a radial cycle of length one. Interior edges of the sheet (shared by two sheet faces) have a radial cycle of length two, exactly like a manifold solid edge. Non-manifold sheet edges (three or more faces meeting, e.g. a T-junction where a third lamina abuts) have radial cycle length >= 3, which the PES already supports.

### 1.3 The validity invariants that CHANGE for sheet bodies

State the deltas precisely against the closed-solid invariants:

1. **Region/material invariant (CHANGED).** Closed solid: every face separates two DISTINCT regions and has a defined material side. Sheet: a double-sided face borders the SAME region (the ambient void) on both sides, OR borders a non-region (the face-side points "into nothing" because the sheet does not bound a cell). The predicate must accept "same region both sides" as valid for double-sided faces and REJECT it for single-sided faces (where it would mean a zero-thickness self-touching solid, a defect).

2. **Coedge-count invariant (CHANGED).** Closed solid: every edge has an EVEN radial cycle, and for a manifold solid exactly two coedges. Sheet: free edges have radial cycle length ONE (odd), which is legal. The "every edge has two coedges" assertion that the solid validator runs must be RELAXED to "every edge has at least one coedge" for sheet/general bodies, with free edges explicitly permitted and counted.

3. **Euler-Poincare bookkeeping (CHANGED to the open-shell form).** For a single closed orientable shell, V - E + F = 2 - 2g (g = genus), equivalently the dossier 01 form V - E + F = 2(s - h). For an OPEN shell (a sheet with boundary), the surface has b boundary loops (the free-edge cycles), and the relation becomes V - E + F = 2 - 2g - b, i.e. the Euler characteristic of a surface with boundary is chi = 2 - 2g - b for one connected sheet (a disk: g=0, b=1, chi=1; an annulus: g=0, b=2, chi=0; a sphere with a hole: chi=1). The bookkeeping invariant the validator enforces is therefore: for each connected sheet component, chi(component) = 2 - 2g - b where b = number of free-edge boundary loops, counted from the radial-length-one edges. This degenerates to the closed-shell 2 - 2g when b = 0 (which is the moment the shell closes, section 2).

4. **Orientability/coherence invariant (UNCHANGED in spirit, adapted).** A sheet must still be COHERENTLY ORIENTABLE across its interior edges: adjacent face-sides across a shared (two-coedge) interior edge must traverse that edge in OPPOSITE directions (the half-edge consistency invariant of dossier 46/38). Because a sheet face is double-sided, "coherent orientation" no longer means "material consistently on one side"; it means the FIN winding is consistent so the two face-uses pick a consistent global "up" for the sheet where one exists. A Mobius-band sheet is non-orientable and must be flagged, not rejected outright (Parasolid permits non-orientable sheets; Keel should accept-and-tag rather than reject, deferring the hard decision to operations that actually need an orientation, e.g. thicken).

5. **Closure/solidity invariant (NEW).** A region may be marked SOLID only if it is bounded by a CLOSED set of oriented faces with NO free edges on its boundary (every boundary edge has its two sides accounted for and the shell is watertight in the tolerant sense). A sheet body has NO solid region precisely because its shell has b >= 1 free-edge loops. This is the invariant whose VIOLATION-then-repair is what knit-and-promote detects (section 2).

### 1.4 The sheet-body validity predicate

Putting it together, the predicate Keel's debug validator should run for a body claiming to be a SHEET (the both-void open-shell case):

```
is_valid_sheet_body(B):
  # (S0) kind: at least one face, no solid region
  assert B.faces is non-empty
  assert every region in B.regions is VOID            # no enclosed solid region
  assert B has exactly one ambient void touching the sheet on both sides
                                                       # (or: faces border the same void on both sides)

  # (S1) faces are double-sided, both-void
  for F in B.faces:
      assert F.sidedness == DOUBLE_SIDED
      assert region_on_side(F, +) == region_on_side(F, -)   # same void both sides
      # natural normal exists where surface is regular; no material side is queried

  # (S2) coedge/radial counts: free edges allowed
  for E in B.edges:
      r = len(radial_cycle(E))
      assert r >= 1                                   # free (r==1), interior (r==2), non-manifold (r>=3)
  assert exists E with len(radial_cycle(E)) == 1      # a sheet HAS a boundary; else it is closed -> not a sheet

  # (S3) Euler-Poincare, open-shell form, per connected component
  for C in connected_components(B):
      b = count_free_edge_loops(C)                     # boundary loops from r==1 edges
      g = genus(C)
      assert V(C) - E(C) + F(C) == 2 - 2*g - b

  # (S4) coherent orientation across interior (r==2) edges
  for E with len(radial_cycle(E)) == 2:
      assert the two coedges traverse E in opposite directions   # half-edge consistency
  # non-orientable components are TAGGED non_orientable, not rejected

  # (S5) tolerant-edge / pcurve coherence (shared with solid validator)
  for E in B.edges:
      assert every coedge of E has a pcurve consistent with E's 3D curve within E.tolerance
```

Note the single load-bearing line distinguishing a sheet from a solid: `assert exists E with radial_cycle == 1`. The instant that ceases to hold (no free edges remain), the body is a CLOSED shell and is a candidate for promotion to solid (S5 of section 1.3, detected in section 2.3). A GENERAL body relaxes S0/S1 to permit mixed solid and void regions and mixed single/double-sided faces in one body; the sheet predicate is the pure two-dimensional both-void special case.

### 1.5 Sources for section 1

**Citation.** Parasolid v12 / v35 documentation, "General Bodies" and "Body Types" chapters. http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.08.html ; http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.016.html
**Content.** "A wire body is a topologically one-dimensional body, while a sheet body is topologically two-dimensional." "General bodies ... can contain generalized topology that allows any combination of non-manifold, cellular, or mixed-dimension parts in a single, valid body." "A region is an open connected subset of 3-dimensional space whose boundary is a collection of vertices, edges, and oriented faces. Regions are either solid or void (empty), and they may be non-manifold." Operations: `PK_EDGE_make_faces_from_wire` attaches rubber faces to loops; "the manifold components of a general body can be extracted ... using `PK_BODY_make_manifold_bodies`."
**Kernel relevance.** This is the authoritative body-kind lattice and region definition Keel adopts. The "regions are either solid or void" sentence is the entire promotion mechanism: solidity is a per-region flag, set only when the region is a bounded enclosed cell. The general-body definition tells Keel that sheets need no new topology, only a new region/face-flag configuration over the existing PES.

**Citation.** Weiler, K. (1986/1988), radial edge structure (full citation in dossier 02).
**Content.** RES makes regions explicit 3D cells (solid or void). The radial cycle around an edge holds one edge-use per incident face-side; an edge incident to a single face has a radial cycle of length one, which is exactly a free/boundary edge.
**Kernel relevance.** Confirms free edges (radial length one) are a native, valid RES/PES configuration, not an error state. Keel's PES inherits this directly; the only work is relaxing the SOLID validator's "two coedges per edge" assertion for sheet/general bodies.

**Citation.** Lee, S. H., & Lee, K. (2001), partial entity structure (full citation in dossier 02).
**Content.** PES materializes partial entities only where non-manifold conditions occur and keeps a half-edge representation elsewhere, at half the storage of RES, deriving all adjacency without loss of efficiency. Hierarchy: region, shell, face, loop, edge, vertex, plus partial-vertex/edge/face.
**Kernel relevance.** A sheet's free edge is the simplest non-manifold-ish case (radial length one); PES handles it with a degenerate partial-edge. A sheet that is everywhere two-coedge interior plus a free boundary is mostly the fast half-edge path with partial-edges only on the boundary, so sheets are cheap in PES.

**Citation.** ACIS double-sided faces / sidedness (dossier 46, ACIS Kernel R10 docs).
**Content.** A SINGLE_SIDED face has material on one side; a DOUBLE_SIDED face has material on both sides or neither, so "away from material" is undefined. ACIS marks such faces explicitly so consumers do not assume a material side. The both-void variant is a free lamina/sheet.
**Kernel relevance.** Directly justifies invariant S1: sheet faces are DOUBLE_SIDED, both-void. Keel's dossier-46 convention already stores enough to tag this; the validator must enforce that a double-sided face borders the same region on both sides.

---

## 2. KNIT / SEW (item 71): Join Coincident Sheet Edges, and Promote on Closure

### 2.1 Problem statement

KNIT (Parasolid's term; OCCT calls it SEWING, healing literature calls it STITCHING) takes a collection of sheet bodies, or one sheet with free edges that should be shared, and JOINS coincident free edges within tolerance so that two single-coedge free edges become one shared two-coedge interior edge. The result is a larger sheet (if boundaries remain) or, when knitting eliminates ALL free edges and the now-closed shell bounds a region, a SOLID after PROMOTION. This is the operation dossier 13 covers at the polyhedral level (Barequet-Sharir) and that OCCT ships as `BRepBuilderAPI_Sewing`; here we specify it for Keel's tolerant-f64 PES.

### 2.2 The matching-and-merge algorithm

The algorithm has the same three phases OCCT's sewing uses (analysis, matching, merging), specialized to Keel's tolerant edges and PES.

**Phase A: collect free boundaries.** Enumerate all FREE edges (radial cycle length one) across the input sheets. These are the only edges eligible to be sewn; interior and already-non-manifold edges are left alone. Build the set of free-edge boundary loops per component (the b loops of section 1).

**Phase B: vertex assembly (cluster coincident vertices).** Before matching edges, cluster free-edge endpoints that lie within the merge tolerance into shared vertices (OCCT's "vertex assembling merges nearby vertices within tolerance"). Use a spatial hash / BVH over endpoints; union endpoints within `tol`. The resulting vertex tolerance is widened to cover the cluster radius (tolerant-vertex, M7b). This fixes vertex gaps (dossier 13 taxonomy) before edge matching so that matched edges share endpoints exactly in topology.

**Phase C: candidate matching.** For each free edge, find candidate partner free edges:
1. **Spatial pre-filter.** Query a BVH over free-edge bounding boxes inflated by `tol`; only edges whose boxes overlap are candidates. This is the scalable replacement for the O(n^2) all-pairs test.
2. **Geometric scoring.** For each candidate pair (E1, E2), score by: endpoint correspondence (both endpoints within `tol` after Phase B clustering), maximal gap between the two 3D curves sampled along their shared parameter span (must be <= working tolerance), and ANGULAR agreement of curve tangents (anti-parallel for a shared boundary, since the two faces lie on opposite sides). OCCT: matching uses "angular evaluation of adjacent edges, distance calculations between edge endpoints, orientation determination." Partial overlap (one edge spans only part of the other, the Barequet-Sharir PARTIAL CURVE MATCHING case) is handled by splitting the longer edge at the projection of the shorter edge's endpoints (the "cutting phase": subdivide edges at intersection/projection points) so the result is full-length matches.
3. **Filtering.** Keep the best-scoring partner per edge; reject if best gap > working tolerance or tangents are not anti-parallel within angular tolerance. OCCT: "find a set of merge candidates for each free boundary, filter them according to certain criteria, and finally merge."

**Phase D: merge.** For each accepted match (E1 on face F1, E2 on face F2):
1. Choose a SURVIVING edge (or build a new tolerant edge whose 3D curve is the average/median of the two within tolerance; M7b tolerant-edge construction). Its edge tolerance is set to cover the maximal deviation of both original curves and both faces' pcurves from the surviving curve.
2. RE-PARENT both coedges (the one from F1, the one from F2) onto the surviving edge: the radial cycle of the surviving edge now has length TWO. F1's free edge and F2's free edge are gone; one shared interior edge remains. Each coedge keeps (or re-projects) its pcurve in its own face's UV; pcurves are reconciled to the surviving 3D curve within tolerance (this is the tolerant-edge / pcurve reconciliation of M7b and section 4.2).
3. Enforce half-edge consistency: the two coedges must traverse the surviving edge in OPPOSITE directions (section 1 S4). If the inputs were oriented inconsistently (one face's normal flips relative to its neighbor), either flip one face's sense to make the shell coherently orientable, or, if the global orientation is contradictory (Mobius), tag the component non-orientable.

Minimal tolerance guard (OCCT's "minimal tolerance defines the size of the smallest element"): do not create a surviving edge shorter than `min_tol`; if matching would, collapse it to a vertex instead (degenerate). This prevents sliver edges in the sewn sheet.

After Phase D, the body's free-edge set has shrunk. Recompute connected components and per-component free-edge loops.

### 2.3 Close-detection and PROMOTION to solid

This is the part beyond plain sewing and the reason knit gates solid construction. After merge:

```
promote_if_closed(B):
  for C in connected_components(B):
      if count_free_edges(C) == 0:                 # no radial-length-one edges remain
          # C is a closed shell. Does it bound a region?
          assert chi(C) == 2 - 2*g(C)              # closed-shell Euler check (b == 0)
          regions = classify_regions_by_shell(C)   # ray-cast / winding from the now-closed shell
          enclosed = the bounded region(s) C separates from the ambient void
          if enclosed is non-empty and shell is coherently orientable:
              # PROMOTE
              mark enclosed region SOLID
              for F in C.faces:
                  F.sidedness = SINGLE_SIDED        # was DOUBLE_SIDED
                  orient F so n_out points away from the solid region   # dossier 46 canonicalization
              recompute sense cache for all F in C   # FIN/MATERIAL-PRIMARY (dossier 46)
              body_kind(C) = SOLID
```

Close-detection is purely topological and cheap: a connected component with ZERO free edges (every edge radial length >= 2) and chi == 2 - 2g is a closed orientable shell. Whether it ENCLOSES a solid region is a region-classification question Keel already answers for booleans (dossier 02 first-class regions, dossier 47 boolean result assembly): the closed shell partitions its neighborhood of space into a bounded inside and the unbounded ambient void. The bounded region flips from VOID to SOLID, the faces flip from double-sided to single-sided, and their sense cache is recomputed so the outward normal points away from the new solid (dossier 46). The KEY DESIGN POINT: solidity is a per-region flag (section 1, Parasolid "regions are either solid or void"), so promotion is a flag flip plus a face-sidedness flip plus a sense-cache recompute, NOT a rebuild. No geometry changes; only region solidity, face sidedness, and the derived sense cache.

A subtlety: a closed shell could be an inside-out orientation (it bounds the void it sits in, not a solid). The orientation coherence plus "which side is bounded" test (ray casting from a face outward; if it escapes to infinity that side is the ambient void) disambiguates which region becomes solid. If neither side is bounded the shell is degenerate; reject.

### 2.4 Sources for section 2

**Citation.** OpenCASCADE, `BRepBuilderAPI_Sewing` class reference. https://dev.opencascade.org/doc/refman/html/class_b_rep_builder_a_p_i___sewing.html ; OCCT Modeling Algorithms user guide. https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_algos.html
**Content.** The algorithm finds merge candidates for each free boundary, filters them, then merges and builds the sewn shape. Three tolerances: working/standard (default 1.0e-6, max distance between elements that can be sewn), minimal (smallest edge allowed, below which topology degenerates to a vertex), maximal. Edge classes: FREE (shared by one face), CONTIGUOUS (shared by two faces), MULTIPLE (shared by more than two faces, avoided in manifold mode). Phases: face analysis (degenerate detection), free-boundary detection, vertex assembling (merge nearby vertices), candidate matching (angular and distance evaluation, orientation), merging (same-parameter edge creation), optional cutting (subdivide edges at intersections). `SewedShape()` may return a face, shell, solid, or compound; solid arises when input geometry closes.
**Kernel relevance.** This is the direct template for Keel's knit. The three-tolerance scheme maps onto Keel's tolerant-edge model (working = match tolerance, minimal = sliver guard). The phase structure (analysis, vertex assembly, matching, merge, cut) is adopted verbatim. The note that the result is a SOLID when geometry closes is exactly Keel's promotion step, which Keel makes EXPLICIT and region-aware rather than incidental.

**Citation.** Barequet, G., & Sharir, M. (1995). Filling gaps in the boundary of a polyhedron. Computer Aided Geometric Design, 12(2), 207-229. https://doi.org/10.1016/0167-8396(94)00011-G ; https://www.sciencedirect.com/science/article/pii/016783969400011G
**Content.** Input is a boundary with gaps bounded by edges incident to ONLY ONE face. Two engines: PARTIAL CURVE MATCHING (score how well two boundary chains correspond, allowing sub-chain matches) and OPTIMAL TRIANGULATION of residual 3D polygonal holes. Finding a consistent maximum-score set of partial matches is NP-hard, so the practical algorithm is a heuristic over the matching graph plus hole triangulation.
**Kernel relevance.** Defines the free-edge ("incident to one face") notion Keel's Phase A uses and the partial-overlap matching Phase C handles via edge splitting. The NP-hardness result is why Keel uses a greedy best-partner-per-edge heuristic with a spatial pre-filter rather than global optimization. Keel does NOT triangulate residual holes (that is a mesh answer); residual unmatched free edges simply remain as the sheet's boundary, which is valid (a sheet is allowed to have a boundary), or are reported for the user to extend/fill (section 3).

**Citation.** Parasolid general-body operations (q-solid mirror, fd_chap.08, as in section 1.5).
**Content.** `PK_EDGE_make_faces_from_wire` attaches rubber faces; bodies change kind (wire -> general) as topology is added; `PK_BODY_make_manifold_bodies` extracts manifold components. Solidity is a region flag.
**Kernel relevance.** Confirms the production pattern: body KIND is a derived consequence of the current region/face/edge configuration, recomputed after each operation, not a fixed type set at creation. Keel's `promote_if_closed` and a symmetric `demote_if_opened` (after trim/split, section 4) follow this pattern: re-derive body kind after every sheet op.

---

## 3. SURFACE EXTENSION (item 70): The Shared Core of Trim / Untrim / Delete-Face-Heal

### 3.1 Why extension is the shared core

Three operations need the SAME primitive: push a face's underlying surface PAST its current boundary so neighbors can be re-intersected. DELETE-FACE-HEAL (dossier 13) removes a face and extends its neighbors to re-meet and close the wound. UNTRIM resets a trimmed face to its full natural surface domain. TRIM/SPLIT (section 4) may need to extend a too-short trimming surface so it fully crosses the sheet. Extension is the load-bearing companion to all of them, and dossier 13 already specifies its dangers (polynomial extrapolation degrades fast, rational weights can go negative, naive extension folds the surface). This section states the construction and the reach/validity limits; it cross-references dossier 13 part 2 rather than repeating its survey.

### 3.2 The two extension modes

**Analytic continuation (analytic surfaces).** For analytic surfaces (plane, cylinder, cone, sphere, torus) extension is EXACT and unbounded in the natural directions: a plane extends to all of R^2, a cylinder/cone extends along its axis indefinitely and around its full angular period, a sphere/torus is already closed in its periodic directions. Extension is simply WIDENING the surface's parameter domain (untrimming the analytic). There is no extrapolation error. The only limits are intrinsic: a cone cannot extend past its apex (the surface degenerates to a point), and periodic directions wrap rather than extend. This mode is preferred whenever dossier 24's canonical-geometry recovery has recognized a face as analytic.

**Natural / unclamped extension (NURBS surfaces).** For a B-spline/NURBS face, the robust construction is UNCLAMPED (natural) extension: modify the knot vector so there are no longer full-multiplicity (clamped) knots at the end being extended, which makes the surface interpolate beyond its former end control points along the natural continuation of its end-segment polynomial. The Ayam/standard-NURBS `unclamp` tooling and the de Boor recurrence extrapolation are the reference: "based on the unclamping algorithm for B-spline curves, algorithms extend B-spline curves by extrapolating using the recurrence property of the de Boor algorithm." Procedure for a surface, extending in +u:
1. Unclamp the u knot vector at the high end (remove the end-knot multiplicity), which does not change the surface shape but repositions/exposes the trailing control points so the surface is C^(p-1) continuable.
2. Append a new knot span of the chosen reach `du` and the control points the de Boor recurrence places to continue the trailing polynomial segment. The extension is the analytic prolongation of the LAST polynomial segment, so it is exactly tangent and curvature-continuous with the original boundary (G^2/C^(p-1)).
3. For rational surfaces, propagate weights the same way and GUARD against weights going non-positive (dossier 13): if an extrapolated weight <= 0, clamp the reach or refuse the extension, because a non-positive weight makes the surface non-convex-combination and can produce a pole or fold.

### 3.3 Reach and validity limits

Extension is local extrapolation and degrades superlinearly with reach. The validity limits Keel must enforce:

- **Reach bound.** Limit `du` to a small multiple of the end span length (a common heuristic is <= 1x to 2x the last knot-span's parametric length, or a 3D distance comparable to the local radius of curvature). Beyond that, polynomial extrapolation overshoots wildly.
- **No self-intersection / no fold.** After extension, verify the surface Jacobian `dS/du x dS/dv` does not vanish or flip sign anywhere in the extended region (no fold). If it does, reduce reach or fail. This is the same regularity test the sheet validator uses for the natural normal.
- **Weight positivity (rational).** All weights in the extended region must stay > 0.
- **Curvature sanity.** The extended region's curvature should not exceed a multiple of the boundary curvature; a blow-up signals unstable extrapolation.
- **Analytic preferred.** If dossier 24 recognized the face as analytic, ALWAYS use exact analytic continuation; never unclamp-extrapolate an analytic that has an exact prolongation.

Extension returns the extended surface plus the parametric reach actually achieved; callers (heal, untrim, trim) re-intersect against it and then RE-TRIM to the new boundary, discarding the unused extended region. The extension is a TEMPORARY scaffold for re-intersection, not a permanent enlargement, except in untrim where the enlarged domain is the intended result.

### 3.4 Sources for section 3

**Citation.** NURBS curve/surface extension by unclamping and de Boor extrapolation; Ayam NURBS modelling tools `unclamp`/`clamp`; Octave/Forge `nrbclamp`/`nrbunclamp`. https://ayam.sourceforge.net/docs/ayam-5.html ; https://octave.sourceforge.io/nurbs/function/nrbclamp.html ; "Algorithm of NURBS curve and surface extension based on characteristic-points fitting" https://www.researchgate.net/publication/286318023
**Content.** Unclamping modifies the knot vector to remove end-knot multiplicity without changing shape, repositioning end control points so the surface interpolates beyond its former ends. Extension extrapolates using the de Boor recurrence so the new segment is the analytic prolongation of the last polynomial segment (tangent/curvature continuous). Characteristic-point-fitting extension is an alternative that fits the extension to sampled continuation points.
**Kernel relevance.** This is the concrete NURBS extension construction for Keel. Unclamp-then-extrapolate is the robust default; characteristic-point fitting is a fallback when a target shape/length is specified. Keel implements unclamping on its NURBS substrate (dossier 37) and reuses the de Boor evaluator it already has.

**Citation.** Dossier 13 (Import Healing Pipelines and Surface Extension), Part 2: surface extension, extrapolation stability, untrimming.
**Content.** Delete-face healing, tweak re-intersection, and untrimming all require pushing a surface beyond its parametric domain; polynomial extrapolation degrades fast, rational weights can go negative, naive extension folds the surface.
**Kernel relevance.** Section 3 is the implementation-grade restatement of dossier 13's extension requirements, specialized to the sheet operations. The reach/fold/weight guards are dossier 13's stability requirements made into concrete validator checks. Do not duplicate dossier 13's broader healing survey; this section is only the extension CORE that trim/untrim/heal share.

**Citation.** Dossier 24 (Canonical Geometry Recovery).
**Content.** Recognizes when a face's surface is exactly analytic (plane/cyl/cone/sphere/torus) rather than an approximating NURBS.
**Kernel relevance.** The branch in section 3.2: analytic-recognized faces take exact analytic continuation (unbounded, error-free); only genuinely free-form NURBS take unclamp-extrapolation with its reach limits. This makes the common case (mechanical parts dominated by analytics) exact.

---

## 4. TRIM (item 72) and SPLIT (item 76): Cut a Sheet by a Curve or a Surface

### 4.1 Trim vs split, defined

SPLIT (item 76) imprints a dividing curve (or the intersection curve with a cutting surface) onto a sheet and PARTITIONS the sheet's faces into two sets, KEEPING BOTH, so the sheet now has an interior edge where the imprint ran. No material is removed; topology is added (one face becomes two, a new edge and possibly new vertices appear). TRIM (item 72) does the same imprint but then DISCARDS one side (the removed part), keeping only the kept part. Trim = split + delete-one-side. So the shared primitive is IMPRINT, and trim adds a side-selection and deletion. Both are the sheet analogue of the boolean imprint Keel already runs on solids (dossier 47), restricted to a single sheet body and its trimming entity.

### 4.2 The imprint algorithm

**Step 1: compute the trim curve(s) in 3D.** If the trimmer is a CURVE, project/embed it onto each sheet face it crosses. If the trimmer is a SURFACE (or plane), intersect it with each sheet face's surface to get the 3D intersection curve(s) (the surface-surface intersection Keel already has for booleans; for a plane vs a planar sheet the intersection is an analytic line). Clip each intersection curve to the face's current trimmed boundary so only the portion lying ON the sheet is kept.

**Step 2: imprint (split the boundary and the face).** For each face crossed:
1. Find where the trim curve ENTERS and EXITS the face's existing boundary loops (intersect the trim curve with the face's edges in UV). Insert new VERTICES at those crossings, splitting the crossed boundary edges (tolerant-edge split, M7b: the original edge becomes two, tolerances inherited and the split vertex tolerance set to cover the crossing uncertainty).
2. Introduce the trim curve as a NEW EDGE (or chain of edges) running through the face interior from entry vertex to exit vertex. Build its 3D tolerant edge and, crucially, its PCURVE in the face's UV (the trim curve's image in the face surface's parameter domain). For a surface-surface intersection both faces involved get a pcurve; for a curve imprint only the one face does.
3. SPLIT the face along the new edge: the single face's loops are re-partitioned into two loops bounding two new faces, each referencing the SAME underlying surface (no new geometry, only new trimming). This is an Euler operator sequence (dossier 01: make-edge-vertex / split-face, the MEV/MEF family) so the PES stays valid throughout. Each new face inherits the parent's sidedness (DOUBLE_SIDED for a sheet) and a freshly computed sense cache.

**Step 3: reconcile pcurves and tolerant edges (the load-bearing M7b step).** The new edge along the imprint is shared (after split) by the two new faces (or, for a surface trimmer that also cut a second sheet face, by faces on both). Reconcile exactly as in knit Phase D: one 3D tolerant curve, a pcurve per adjacent face-side consistent with it within the edge tolerance, half-edge consistency on traversal direction. Where the trimmer is a surface that does NOT exactly pass through Keel's representation of the intersection (tolerant case), set the edge tolerance to cover the surface-vs-curve deviation rather than forcing the surfaces to be modified. This is precisely dossier 13's tolerant-imprint philosophy ("tolerant imprinting uses the merge tolerance to determine coincident and overlap entities ... can imprint curves onto surfaces provided the tolerance ... falls within the merge tolerance").

**Step 4 (TRIM only): select and delete the removed side.** Classify the two (or more) sub-faces into KEPT and REMOVED by the caller's side selection (a seed point, a keep-which-side flag, or "keep the smaller/larger"). DELETE the removed faces with an Euler kill-face sequence, leaving their shared imprint edge as a NEW FREE EDGE of the kept sheet (radial cycle drops from two back to one). Re-derive body kind: the kept body is still a sheet (it has free edges); if the trim happened to close a hole it could promote (section 2.3), but the usual outcome is a smaller sheet.

### 4.3 What changes in the PES and the validity re-check

Trim/split is local surgery (dossier 44 lineage). After it:
- F count increases by the number of faces split (split), then decreases by the removed faces (trim).
- E, V increase by the imprint edges and crossing vertices.
- Each connected component's open-shell Euler relation (section 1, S3) must still hold; the validator re-runs `chi == 2 - 2g - b` on the affected components.
- Free-edge count rises on a TRIM (the cut boundary becomes free), so a previously closed component can be DEMOTED from solid to sheet if trimming opens it (the symmetric counterpart to promotion: `demote_if_opened`). This is why trim is a SHEET operation even though it can be applied to a solid's faces.
- All new edges carry reconciled pcurves and tolerances; the tolerant-edge/pcurve coherence invariant (S5) is re-checked on them.

### 4.4 Sources for section 4

**Citation.** Tolerant imprinting / imprint-curve-on-face references: ACIS Kernel R10 Ch.6 Tolerant Modeling (http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/06TMOD.PDF); Sandia Cubit imprint/merge docs (https://www.sandia.gov/files/cubit/15.3/help_manual/WebHelp/geometry/imprint_merge/imprint.htm); Onshape/Parasolid face-imprint discussion (https://forum.onshape.com/discussion/18189).
**Content.** Tolerant imprinting uses the merge tolerance to determine coincident and overlap entities and can imprint curves onto surfaces provided the surface-curve tolerance is within the merge tolerance. Imprinting splits faces using curves (SplitFaceOnParam / ImprintCurve). Parasolid exposes face-imprint-curve internally.
**Kernel relevance.** Justifies Keel's tolerant-imprint step 3: rather than modifying surfaces to make a trim curve lie exactly on them, widen the new edge's tolerance to cover the deviation (M7b tolerant edges). This is the production-correct way to imprint a plane's intersection onto a NURBS sheet without geometry surgery.

**Citation.** Dossier 47 (Boolean Result Assembly) and dossier 01 (Euler Operators).
**Content.** Boolean imprint splits faces along intersection curves and reassembles regions; Euler operators (MEV, MEF, split-face, kill-face) keep the B-rep valid through incremental topology edits.
**Kernel relevance.** Trim/split reuse the boolean imprint machinery restricted to one sheet plus a trimmer, and the face-split/face-kill is expressed as Euler sequences so the PES stays valid at every step. This is why item 76 (split) is cheap once booleans exist: it is the imprint half of a boolean with no region recombination.

**Citation.** OCCT `BRepFeat_SplitShape` / `BRepAlgoAPI_Section` (forum and pythonocc docs, from the imprint search).
**Content.** `BRepAlgoAPI_Section` produces the intersection curve between shapes; `BRepFeat_SplitShape` splits a face/shell along imprinted edges, with care needed on pcurves.
**Kernel relevance.** Confirms the two-step structure Keel uses (section to get the curve, split to partition the face) and the standing warning that PCURVES are the fragile part of imprint, reinforcing step 3's reconciliation as the place to spend engineering care.

---

## 5. Sheet Bodies for Keel (Synthesis)

### 5.1 The sheet-body validity rules in PES

A sheet body is NOT new topology in Keel's PES; it is a new REGION/FACE configuration over the existing first-class-region structure (dossier 02). The rules that change from the closed-solid invariants:

1. No enclosed solid region: every region touching the sheet is the same ambient VOID; a sheet face borders that same region on BOTH sides.
2. Faces are DOUBLE_SIDED, both-void: no material side, so no single outward normal; the natural normal and fin winding still exist where the surface is regular.
3. Free edges are legal: radial cycle length ONE. Relax the solid validator's "two coedges per edge" to "at least one," and require that a sheet HAS at least one free edge (else it is closed).
4. Euler-Poincare uses the OPEN-shell form per connected component: V - E + F = 2 - 2g - b, with b = number of free-edge boundary loops; this collapses to 2 - 2g exactly when the shell closes.
5. Coherent orientation across interior (two-coedge) edges via half-edge consistency; non-orientable sheets are TAGGED, not rejected.
6. Solidity is a per-region FLAG, set only when a region is bounded by a closed (b = 0) coherently orientable watertight shell.

The single sheet-body predicate is section 1.4; its one load-bearing discriminator is `exists edge with radial_cycle == 1`.

### 5.2 The knit-and-promote algorithm

Three OCCT-style phases plus an explicit promotion step: (A) collect free edges; (B) cluster coincident endpoints into shared tolerant vertices; (C) match free edges by spatial pre-filter + endpoint/gap/anti-parallel-tangent scoring, splitting for partial overlaps (Barequet-Sharir); (D) merge each accepted pair onto one surviving tolerant edge, re-parenting both coedges so its radial cycle becomes two, reconciling pcurves and enforcing half-edge consistency, with a minimal-tolerance sliver guard. Then PROMOTE: any connected component with ZERO free edges and chi = 2 - 2g is a closed shell; classify the region it bounds, flip that region VOID -> SOLID, flip its faces DOUBLE_SIDED -> SINGLE_SIDED, recompute the sense cache (dossier 46) so normals point away from the new solid. Promotion is a flag flip plus a sense recompute, never a geometry rebuild, because solidity is a per-region flag.

### 5.3 The surface-extension core

One primitive shared by trim, untrim, and delete-face-heal (dossier 13): extend a face's surface past its boundary. Analytic surfaces use EXACT analytic continuation (widen the parameter domain; unbounded except at degeneracies like a cone apex). NURBS surfaces use UNCLAMPED (natural) extension: unclamp the end knots without changing shape, then de-Boor-extrapolate the trailing polynomial segment for tangent/curvature-continuous prolongation, guarding rational weights to stay positive. Reach is bounded to ~1x to 2x the end span; reject on fold (Jacobian sign change), non-positive weight, or curvature blow-up. Prefer exact analytic continuation whenever dossier 24 recognized the face as analytic.

### 5.4 Trim and split

Shared primitive is IMPRINT: compute the trim curve (curve embed, or surface-surface intersection, e.g. plane-vs-sheet line), insert crossing vertices that split boundary edges, add the trim curve as a new tolerant edge with a pcurve per adjacent face, split the crossed face into two faces on the same surface via Euler operators, and reconcile pcurves and tolerances (M7b tolerant imprint: widen the edge tolerance rather than modify surfaces). SPLIT keeps both sides; TRIM additionally classifies and deletes the removed side via a kill-face sequence, turning the imprint edge into a new free edge (radial two -> one) and possibly DEMOTING a solid to a sheet. Re-derive body kind and re-run the open-shell Euler and pcurve/tolerance invariants on affected components after every trim/split.

### 5.5 Validation A: knit six planar sheets into a closed cube, detect closure, promote

Start with six square sheet faces, one per cube face, each a DOUBLE_SIDED face on a plane, each bounded by four free edges (radial cycle one). Body is a sheet (or six sheet components): 6 faces, 24 free edges, 24 vertices, each component chi = 1 (disk), b = 1.

Knit Phase B clusters the cube's 8 corners (each shared by 3 faces' endpoints) and the 12 edge-midpoint endpoints into shared tolerant vertices: 24 original vertices collapse toward 8 cube-corner vertices. Phase C matches the 24 free edges into 12 pairs (each cube edge is the seam between two adjacent square sheets), tangents anti-parallel, gaps within tolerance. Phase D merges each pair onto one surviving tolerant edge, re-parenting both coedges so each of the 12 cube edges has radial cycle TWO, reconciling the two adjacent faces' pcurves.

After merge: V = 8, E = 12, F = 6, free-edge count = 0. Close-detection fires: chi = 8 - 12 + 6 = 2 = 2 - 2(0), a closed genus-0 orientable shell. Region classification finds the bounded interior; PROMOTE it VOID -> SOLID, flip all six faces DOUBLE_SIDED -> SINGLE_SIDED, recompute their sense cache so the six outward normals point away from the interior (dossier 46). The body is now a valid SOLID CUBE: 8 vertices, 12 edges, 6 faces, one solid region plus the ambient void, every edge two coedges, V - E + F = 2. This exercises the entire knit-and-promote path and lands on a body that passes the SOLID validator, confirming the design closes the loop sheet -> solid.

### 5.6 Validation B: trim a single sheet by a plane

Start with one rectangular planar sheet (1 double-sided face, 4 free edges, 4 vertices, chi = 1, b = 1) and a cutting plane crossing it. Imprint: intersect the cutting plane with the sheet's plane to get a 3D line; clip it to the rectangle, giving an entry point on one boundary edge and an exit point on the opposite boundary edge. Insert two crossing vertices, splitting those two boundary edges (now 6 boundary edges, 6 vertices). Add the trim line as a new tolerant edge with a pcurve in the sheet's UV, and SPLIT the face into two rectangular sub-faces on the same plane via an Euler split-face. State after split: F = 2, E = 7 (4 original boundary segments + 2 split halves... recount: 6 boundary edges + 1 interior imprint edge = 7), V = 6. Each component still satisfies the open-shell Euler relation; the imprint edge has radial cycle two (shared by the two sub-faces), the 6 boundary edges remain free.

For TRIM, select the removed side and kill its face. The kept body is one rectangular sub-face: the imprint edge drops from radial two to radial ONE (it is now a free boundary of the kept sheet), giving a smaller rectangular sheet (1 face, 4 free edges, 4 vertices, chi = 1). Body kind re-derived: still a SHEET (free edges present). This exercises imprint, the tolerant-edge/pcurve reconciliation, the Euler split-face and kill-face, and the radial-two-to-one transition, and confirms trim = split + delete-one-side with body-kind re-derivation.

### 5.7 Design recommendation status

Everything above is a DESIGN RECOMMENDATION to validate against Keel's tests. Concretely, before adoption: (1) extend the debug validator with `is_valid_sheet_body` (section 1.4) and the open-shell Euler check, and confirm it accepts hand-built sheet fixtures and still rejects malformed ones; (2) implement knit with the three-tolerance scheme and run the six-sheet-cube fixture, asserting the result passes the SOLID validator and that `mass_properties` (sense-aware, dossier 46) returns the correct cube volume after promotion; (3) implement extension with the analytic/unclamped split and fold/weight guards and test against delete-face-heal fixtures from dossier 13; (4) implement imprint/trim/split reusing the boolean imprint and Euler operators, and run the trimmed-sheet fixture asserting the open-shell Euler relation and pcurve coherence hold. The promotion/demotion of body kind must be tested for idempotence (knit then trim then knit returns to the closed solid). Watch the dossier-46 sense convention closely on promotion: the face-sidedness flip and sense recompute are exactly where a regression could reintroduce the split-convention bug that dossier solved.

---

## References

1. Weiler, K. J. (1986). Topological structures for geometric modeling (Doctoral dissertation). Rensselaer Polytechnic Institute. And Weiler, K. (1988). The radial edge structure. In Geometric Modeling for CAD Applications, 3-36. North-Holland. (Free/boundary edge = radial cycle length one; explicit solid/void regions.) https://dl.acm.org/doi/pdf/10.1145/304012.304042 ; https://people.eecs.berkeley.edu/~ddgarcia/cs184/kjw/bib.html
2. Lee, S. H., & Lee, K. (2001). Partial entity structure: A compact boundary representation for non-manifold geometric modeling. J. Comput. Inf. Sci. Eng., 1(4), 356-365; and SMA '01, 159-170. https://asmedigitalcollection.asme.org/computingengineering/article-abstract/1/4/356/471622 ; https://researchr.org/publication/LeeL01:12
3. Barequet, G., & Sharir, M. (1995). Filling gaps in the boundary of a polyhedron. Computer Aided Geometric Design, 12(2), 207-229. https://doi.org/10.1016/0167-8396(94)00011-G ; https://www.sciencedirect.com/science/article/pii/016783969400011G
4. OpenCASCADE Technology. BRepBuilderAPI_Sewing class reference. https://dev.opencascade.org/doc/refman/html/class_b_rep_builder_a_p_i___sewing.html ; older: https://old.opencascade.com/doc/occt-7.2.0/refman/html/class_b_rep_builder_a_p_i___sewing.html
5. OpenCASCADE Technology. Modeling Algorithms user guide (sewing, free/contiguous/multiple edges). https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_algos.html
6. OpenCASCADE Technology. Shape Healing user guide. https://documentation.help/Open-Cascade/occt_user_guides__shape_healing.html
7. Parasolid v12.0 / v35 documentation. General Bodies; Body Types; Model Structure. http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.08.html ; http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.016.html ; http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.04.html
8. Parasolid v12.0 Wire Modeling (rubber faces, make_faces_from_wire, body-kind transitions). http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.38.html
9. Spatial / ACIS. Kernel R10 Ch.6 Tolerant Modeling (tolerant imprinting, double-sided faces, sidedness). http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/06TMOD.PDF
10. NURBS curve/surface extension by unclamping and de Boor extrapolation. "Algorithm of NURBS curve and surface extension based on characteristic-points fitting." https://www.researchgate.net/publication/286318023_Algorithm_of_NURBS_curve_and_surface_extension_based_on_characteristic-points_fitting
11. Ayam NURBS Modelling Tools (unclamp/clamp surface). https://ayam.sourceforge.net/docs/ayam-5.html
12. Octave-Forge nurbs package: nrbclamp / nrbunclamp. https://octave.sourceforge.io/nurbs/function/nrbclamp.html ; https://octave.sourceforge.io/nurbs/overview.html
13. Sandia Cubit. Imprinting Geometry (tolerant imprint, merge tolerance). https://www.sandia.gov/files/cubit/15.3/help_manual/WebHelp/geometry/imprint_merge/imprint.htm
14. Onshape forum. Parasolid face imprint curve discussion. https://forum.onshape.com/discussion/18189/are-there-any-plans-to-expose-parasolid-face-imprint-curve
15. OpenCASCADE forum / pythonocc. BRepAlgoAPI_Section, BRepFeat_SplitShape, face pcurves. https://dev.opencascade.org/content/one-cylinder-trims-another-cylinder ; https://pythonocc-documentation.readthedocs.io/en/review-gen-apidoc-rtd/apidoc/OCC.Core.BRepBuilderAPI.html
16. MIT OCW 2.158J Computational Geometry, Lectures 14-15 (radial edge, non-manifold topology). https://ocw.mit.edu/courses/2-158j-computational-geometry-spring-2003/f04f923ac8e0af56d19095b4de8dea3c_lecnotes14_fixed.pdf
17. US Army Research Lab. Combinatorial Solid Geometry, Boundary Representations, and Non-Manifold Geometry (radial-edge regions, NMG). https://ftp.arl.army.mil/~mike/papers/90nmg/joined.html

Internal cross-references: dossier 02 (non-manifold/cellular PES, lamina faces, first-class regions), dossier 46 (face orientation / sense convention, double-sided faces), dossier 13 (import healing, stitching, surface extension), dossier 01 (Euler operators / fins), dossier 24 (canonical geometry recovery), dossier 44 (local topology surgery), dossier 47 (boolean result assembly), dossier 50 (thicken), dossier 37 (numerical/NURBS substrate).
