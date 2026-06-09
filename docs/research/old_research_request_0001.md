# New Research Requests (queued for the research agent)

Drafted 2026-06-09. Implementation-grade dossier requests for the remaining
hard/frontier items, grounded in Keel's actual state (tolerant-f64, non-manifold
PES / radial-edge B-rep; Euler operators; arenas + generational keys + `EntityId`
identity; analytic surfaces plane/cyl/cone/sphere/torus + NURBS curves/surfaces;
analytic + recovered-NURBS SSI; generalized-winding-number classification;
divergence-theorem mass properties; constructors block/prism/cyl/cone/sphere/torus/
tube/revolve/revolve_closed/revolve_partial/loft/loft_sections/extrude_tapered/
sweep_along_path; chamfer + cylinder-rim fillet; M1-M8 done; parity 80/144).

**Recommended send order:** #1 first (highest blast radius), then #2/#3
(construction breadth, shippable solo once they land), then #4-#6 (deeper
architecture). Persistence (126) is intentionally NOT requested -- it is
mechanical and fully scoped in the live parity task.

---

## #1 -- Robust boolean result assembly (the `build_result_solid` face-drop) -- HIGHEST LEVERAGE

**Context.** Keel's boolean pipeline is: `preimprint coincident overlaps -> seam_curves
(analytic SSI) -> imprint_operand(a)/imprint_operand(b)` (split faces along seams)
`-> classify_faces` (generalized-winding-number: inside/outside/on with a 0.25 band
around w=0.5) `-> select_faces` (Requicha tables -> `KeptFace { operand, face, reversed }`)
`-> assemble`. Assembly forks: an **all-planar** result goes through `build_result_solid`
-- a polygon-soup stitcher that takes `ResultPoly { ring: Vec<Vec3>, outward: Vec3,
surface }`, dedups vertices, shares edges, builds fins/loops/faces directly in the
arenas, and partitions into solid+void regions; a **curved** result goes through
`stitch_by_import` (Euler import-and-glue).

**The bug (concrete, reproducible).** A non-transversal / thin-tilted-feature difference
drops a kept face. Repro: subtract a thin tilted wedge prism from a box corner (an
*asymmetric* chamfer, setbacks d1 != d2, cut plane at a non-45-degree angle). The kept
B-fragment is the single tilted cut face; the result *validates* (Euler-consistent) but
is geometrically **wrong** -- `build_result_solid` emits an all-axis-aligned face soup
that **drops the tilted cut face** and leaves stray fragments. At d1=0.5, d2=1.0 on a
2^3 box: mesh_volume 8.83, analytic mass_properties 11.5, true answer 7.5 -- all three
disagree. The symmetric (45-degree) chamfer works perfectly, so it is specific to the
thin/oblique fragment, not the pipeline at large. fuzz_boolean (random transversal
bodies) is clean.

**Questions (implementation-grade):**
1. **The robust imprint-classify-stitch assembly algorithm:** how do production kernels
   assemble the result B-rep from classified face fragments so that *no kept fragment is
   ever dropped*, specifically for thin/sliver/oblique features? What invariant
   guarantees every selected fragment lands in the output shell?
2. **Drop failure modes in a polygon-soup stitcher:** what causes a face to vanish during
   vertex-dedup + edge-sharing assembly -- vertex welding tolerance collapsing a thin
   feature's distinct vertices, ambiguous edge-pairing when >2 candidate coedges share a
   welded edge, or winding/orientation rejection? The robust fixes for each.
3. **Soup-stitch vs incremental Euler/half-edge assembly:** is building from an oriented
   polygon soup the wrong architecture (vs the radial-edge incremental glue Keel already
   uses for curved results)? Should the planar path also use `stitch_by_import`?
   Trade-offs, and what the reference kernels actually do.
4. **Tolerance model for thin features in assembly:** how to set the vertex-weld /
   edge-merge tolerance *relative to local feature size* so a 0.5x1.0 wedge isn't
   collapsed, while still welding genuine coincidences. Snap-rounding vs
   feature-size-relative epsilon.
5. **Mesh-arrangement booleans (Zhou et al. 2016 libigl; Cherchi/Attene 2020; EMBER):**
   what do their arrangement->cell-extraction assemblers do that's drop-proof, and which
   ideas transfer to a *curved* B-rep stitch?
6. **Orientation continuity:** ensuring a kept *reversed* B-fragment (the new cavity wall)
   carries a consistent solid-outward normal through the stitch (ties to dossier 46's
   sense convention).

**Deliverable:** `docs/research/kernel/NN-boolean-result-assembly.md` giving the robust
assembly algorithm, the drop-prevention guarantee, the feature-size-relative tolerance
rule, and a recommendation on soup-stitch vs Euler-glue for Keel's planar path --
validated against the reproducible asymmetric-chamfer case. Unblocks shell, thicken,
asymmetric chamfer, and the blend family (~15-20 map items).

---

## #2 -- NURBS skinning / lofting with cross-section continuity (items 66-67, 69)

**Context.** Keel has `loft_sections` (K planar polygon sections, *planar* side quads,
faceted) and `sweep_along_path` (translational). It has NURBS curves/surfaces, SSI, and a
NURBS recovery path. Dossier 26 covers n-sided transfinite fill but **explicitly defers
lofting/skinning to "separate lofting research"** -- that's the gap. The smooth,
continuity-controlled skin (67) needs real NURBS surfaces, not facets.

**Questions.** (1) The B-spline **skinning** algorithm: compatible-knot-vector merging
across sections, degree elevation, section reparameterization so isoparms align, the
lofting linear-solve for the v-direction control net. (2) **Continuity**: tangent/curvature
(G1/G2) end conditions and matching to **guide rails** (Gordon-surface direction).
(3) Sections with **differing** vertex/knot counts -- refit/compatibility. (4) **Ruled**
(69) as the degree-1 linear case; **closed/periodic** sections. (5) Resulting degree/
knot-growth control and tolerance-bounded approximation.

**Deliverable:** `docs/research/kernel/NN-nurbs-skinning-lofting.md`, validated against a
3-section circular->square skin with tangent ends.

---

## #3 -- Sweep along an arbitrary path with rotation-minimizing frames (item 63, full)

**Context.** Keel's `sweep_along_path` is translational (profile stays parallel). The real
sweep orients the profile along the path. Dossiers 01/26 name "trajectory + profile + RMF
frame rule" but there is no dedicated sweep dossier.

**Questions.** (1) **Rotation-minimizing frame** transport (Wang et al. double-reflection)
along a NURBS/polyline spine, vs Frenet (and why RMF avoids twist/flips). (2) The
**swept-surface representation** -- procedural (spine+profile+frame) baked to NURBS at
tolerance, vs direct. (3) **Self-intersection** when path curvature radius < profile radius
-- detection and the trim/decline policy. (4) Variable **scale/twist** laws along the path;
end caps; **G1 at polyline path joints** (miter vs round).

**Deliverable:** `docs/research/kernel/NN-sweep-rmf.md`, validated against a circular
profile swept along an L-path and a helix.

---

## #4 -- Shell / hollow / thicken / whole-body offset (items 41-45)

**Context.** Keel has `offset_surface` (analytic, Addendum 37) and tweak reintersection.
Shell/thicken/offset are the Phase-4 family; they sit downstream of #1 (they consume
general-position booleans) but the **offset-topology** half is independently researchable.

**Questions.** (1) The **shell** algorithm: offset each face inward by t, **re-intersect
adjacent offset faces** to rebuild the inner-shell topology (reusing tweak reintersection),
then bridge outer<->inner along pierced/open faces. (2) Faces that **vanish or
self-intersect** when offset past the local concave radius -- detection + topology collapse
handling. (3) **Per-face thickness** (43) and **pierce-open faces** (42). (4) Whole-body
offset (45) **global self-intersection resolution** (Chen-McMains winding-number trimming).
(5) **Thicken a sheet** (44) -- offset both sides + rim walls (depends on #5).

**Deliverable:** `docs/research/kernel/NN-shell-offset-thicken.md`, validated against a
shelled box (uniform wall) and a box with one pierced face.

---

## #5 -- Sheet / open-body topology and sheet operations (items 70-72, 76)

**Context.** Keel is non-manifold PES with first-class regions; current bodies are closed
solids with a solid region + void. **Sheet bodies** (open shells / lamina faces -- the
"double-sided" faces dossier 46 flagged) aren't yet a modeled body kind. Sheet ops gate
thicken (44), surface-from-boundary, and import healing.

**Questions.** (1) Does the **PES region model** represent a sheet body cleanly (no
enclosed solid region; double-sided faces with no material side) -- and the **validity
rules** for sheet bodies (vs solids). (2) **Knit/sew** (71): join coincident sheet edges
within tolerance into a shell/solid, and when knitting closes a void -> promote to solid.
(3) **Surface extension** (70) -- the shared core of trim/untrim/delete-face-heal (dossier
13) -- extend a NURBS/analytic face past its boundary. (4) **Trim** (72) / **split** (76) a
sheet by a curve/surface, reconciling pcurves + tolerant edges.

**Deliverable:** `docs/research/kernel/NN-sheet-bodies-ops.md`, validated against knitting
six planar sheets into a closed cube and trimming a sheet by a plane.

---

## #6 -- Persistent topological naming (items 82-85, 124)

**Context.** Keel has deterministic `EntityId` (stable *within* a build) and a `lineage`
map (derivation events). But across **edits** (booleans, tweaks regenerate topology) a
downstream reference to "the face created in step 3" must survive regeneration -- the
classic persistent-naming problem, which gates parametric/feature-tree hosts and
edit-surviving assembly instances (82-85).

**Questions.** (1) The canonical approaches -- **Kripac**, **Capoyleas-Chen-Hoffmann**,
**Bidarra-Bronsvoort** -- and which fits a tolerant non-manifold kernel. (2) Naming
entities by **generative history** (which inputs/operations produced them) so references
re-resolve after regeneration, **leveraging Keel's existing lineage**. (3) **Ambiguity**
from splits/merges (one face -> many, many -> one) and the resolution rules. (4) Matching
after a **boolean** specifically (where most renaming churn happens).

**Deliverable:** `docs/research/kernel/NN-persistent-naming.md`, validated against renaming
survival through a box->chamfer->hole edit chain.
