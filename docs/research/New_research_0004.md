# New Research Requests 0004 (queued for the research agent)

Drafted 2026-06-17. Implementation-grade dossier requests for the ONE remaining
gateway to the dominant non-coaxial curved-boolean decline: the **NURBS-rim
sphere band** -- generalising the landed circle-rim sphere machinery (classify,
mass, tessellation) to arbitrary NURBS seam loops. Round 0003 (dossiers #64
periodic-domain seam split, #65 canonical curved-seam identity, #66 curved
multi-loop-face mass) prescribed and landed the CYLINDER prototype; round 0004
asks for the SPHERE generalisation those three explicitly left as a follow-on.

**Keel state (grounding, post-0003).** Tolerant-f64 non-manifold radial-edge
B-rep; Euler operators; arenas + generational keys + `EntityId`. Analytic
surfaces plane/cylinder/cone/sphere/torus + NURBS. Boolean pipeline:
`seam_curves (analytic SSI + recovered-NURBS) -> imprint_operand(a/b) ->
classify_faces (generalized winding number, 0.25 band) -> select_faces (Requicha)
-> stitch_by_import -> finalize_imported_assembly (Weiler region walk)`. The
CORRECTNESS PROGRAM IS COMPLETE (Add. 193: 2.24B fuzz, WRONG=0, three-bucket
oracle PASS/DECLINE/WRONG; contract = DECLINE-never-WRONG). The active program is
turning DECLINES into PASSES without ever emitting a WRONG. A `sphere` face is
ONE periodic (u,v) domain: u in [0, 2*pi) identified at the seam slit, v in
[-pi/2, +pi/2] collapsing to a pole at each end.

**What landed since 0003 (so the agent does NOT re-derive it).**
- The cyl/cyl periodic split (#64) LANDED (Add. 275, commit 0b2b119):
  `imprint_cylinder_wrap_bands` splits a cylinder lateral by an encircling
  non-planar NURBS wrap via a SLIT-CLOSING band split (NOT the antipode spur,
  which collapses to zero area). 3 of 4 unequal-perpendicular cyl/cyl ops PASS
  exact. The canonical seam identity (#65) turned out UNNEEDED for cyl/cyl (the
  slit-closing edges pair directly in the existing stitcher) but remains the
  prescription for the multi-operand SPHERE case. The cyl multi-loop mass witness
  (#66) landed (rectangle-witness + a gated per-fin winding flip for the genuine
  2-wrap barrel).
- The multi-cut cap-plane phantom (LOG Add. 294, commit 83487c3) is FIXED:
  `curve_face_overlap` now trim-checks a circular-DISK plane (was stubbed to
  `All` because a disk has < 3 outer-loop vertices), so a tool cap's SSI section
  of a far cylinder/sphere is clipped before it imprints a phantom doubled rim.
  +506 soak passes, FAIL=0 both seeds. This CLOSED the prior dominant mass!=mesh
  class and re-exposed the sphere band as the new dominant one.
- The coaxial sphere-vs-{cyl,cone} family, the single-window graze, and the
  planar-cap intersection all PASS exact (Add. 267/268/281/282). The WRAP SSI is
  SOLVED: `quadratic_branch_field` (ssi.rs) emits the two encircling non-planar
  NURBS loops, verified on both surfaces to 1e-5 (Add. 276/280). The CYLINDER
  side of the wrap is done (`imprint_cylinder_wrap_bands`, gated KEEL_WRAP_FLOW,
  meshes exact). The tight oracle `quadric_sphere_op_volume` backs the gate, so
  this class is SAFE to develop UN-gated: a wrong result self-declines.

**The wall this round must break (sharpened by a real attempt, Add. 283).** The
non-coaxial **sphere-vs-{cylinder,cone} WRAP** (probe_wrap, probe_csu): the
sphere is split by two encircling non-planar NURBS loops into a BAND (between the
rims) + two caps. Everything up to the sphere downstream works (SSI, cylinder
imprint). The SPHERE side is CIRCLE-RIM-SPECIFIC and breaks three ways, each
confirmed by direct attempt:
- **CLASSIFY**: `sphere_face_interior_point` assumes latitude-CIRCLE rims and is
  ORIENTATION-SENSITIVE -- reorient the sphere frame and the band/caps drop from
  classify; an arbitrary NURBS rim returns None or a degenerate pole.
- **MASS**: the NURBS-rim band/cap faces integrate WRONG in the Green-slab
  (probe_wrap: INTERSECT mass 16.07 vs 17.17; UNION 67.58 vs 121).
- **TESSELLATION**: `tessellate_sphere` has only a circle-rim path and the
  single-window-disc fan; a general NURBS rim is not meshed edge-conformingly
  with the cyl/cone wall.

These are the SPHERE generalisation of the cylinder pieces #64/#66 plus the
classify the cylinder received in 0003. Solving them turns cyl/sph (~6,260) +
cone/sph (~2,960) = ~9,200 declines (~35% of the corpus, 2x20000 both seeds) from
DECLINE into PASS, and the SAME NURBS-rim machinery is reused by the general
quartic classes (skew cyl/cyl ~1,480, cone/cyl ~2,510) and the plane-cone
parabola/hyperbola (cone/block ~3,330).

**Dead ends (proven; the agent must NOT propose these).**
- Containment-DROP of whole phantom seams (Add. 260/261): regressed the soak
  (FAIL 0->114 / 0->166). NOTE the Add-294 fix was a NARROW disk-plane overlap
  completion, a different grain, NOT a seam drop -- do not conflate.
- mev/mef antipode SPUR for the non-planar wrap (Add. 273): zero-area bands.
  Superseded by the slit-closing band split (#64) on the cylinder; the sphere
  needs the analogous periodic-domain re-thread, not a spur.
- Circle-rim-specific band/clip (Add. 267/268): correct for coaxial circles, the
  exact thing being generalised here.

**Recommended send order:** #1 (classify -- the frame-robust interior point gates
both `select_faces` and the tessellation anchor), then #2 (mass -- the WRONG-risk
precondition before any guard removal), then #3 (tessellation watertightness).

---

## #1 -- Frame-robust sphere NURBS-cut classify: a frame-agnostic interior point for a spherical region bounded by arbitrary closed NURBS loops

**Context.** `sphere_face_interior_point` (boolean.rs) returns a point strictly
inside a trimmed sphere face; `classify_faces` uses it for the inside/outside
winding test and `tessellate_sphere` uses it as the fan anchor. Its band fallback
was built for latitude-CIRCLE rims (Add. 267/268) and is frame-sensitive: when
the sphere FRAME places the u=0 seam meridian or the v=+-pi/2 poles inside the
trimmed region, a seam-only or latitude-only metric picks a degenerate or
exterior point, so the band/caps drop from classify (a silent wrong-positive that
the gate then catches as decline). A non-coaxial wrap rim is a non-planar NURBS
loop, not a latitude circle, so the circle-rim path returns None.

**Questions (implementation-grade).**
1. **Region-from-loops on the periodic sphere.** Given a sphere (u,v) domain and
   K closed seam pcurves (each an encircling NURBS loop wrapping u as v varies, or
   a non-wrapping loop), what is the robust rule to identify each trimmed region
   (cap = a pole-containing region bounded by one loop; band = a region between
   two loops containing NO pole) and emit a strictly-interior (u,v) for each, with
   max margin from every bounding loop AND from the u=0 seam slit AND from both
   poles? Give the point-in-region test on the periodic domain (winding of the
   loop's pcurve, wrap-aware).
2. **Frame-agnostic construction.** State the construction so the answer is
   invariant to the sphere's parametric frame (the bug: an X-pole frame put the
   band ring through the poles). Prefer a 3D-geometric anchor (e.g. the loop
   centroid direction for a cap; the average of the two rim mean-directions for a
   band) projected to the surface and verified in-domain, falling back to a
   (u,v) grid search only with the seam/pole exclusion.
3. **Band vs cap discrimination + the pole question.** How to decide, from the
   loop set alone, whether a region is a cap (contains a pole) or a band (does
   not), and how a band that is itself split by the seam slit is handled. The
   degenerate sub-case (a rim passing through a pole) -> representable or
   DECLINE-forever?
4. **Production-kernel realisation.** How do OCCT (`BRepTopAdaptor_FClass2d` /
   `IntTools` point-in-face on a periodic sphere with NURBS pcurve trims) and ACIS
   pick a reliable point-in-face for a periodic sphere face with arbitrary trim
   loops, and how do they treat the seam and poles?
5. **Validation oracle.** A test battery: a sphere cut by two non-coaxial NURBS
   wrap loops (a band + two caps), under SEVERAL sphere frames (axis-aligned,
   tilted, X-pole), asserting the returned point is on the surface, in the correct
   region (verified against an independent solid-angle / 3D containment test), and
   margin-positive. Frame-invariance is the headline assertion.

**Deliverable.** `docs/research/kernel/67-sphere-nurbs-cut-classify.md`: the
region-from-loops algorithm on the periodic sphere domain, the frame-agnostic
interior-point construction, the band/cap/pole taxonomy, the production-kernel
notes, and the frame-invariant validation battery.

---

## #2 -- NURBS-rim sphere band/cap mass: the Green-slab generalisation from circle rims to arbitrary NURBS loops (the sphere dual of #66)

**Context.** The generic boundary-arc Green-slab arm `integrate_face_green` /
`FinCurve::Nurbs` exists (Add. 271, samples any `Curve3` for point+tangent+
weight), and the cylinder multi-loop rectangle-witness landed (#66). But the
sphere band/cap faces bounded by NURBS rims still integrate WRONG (probe_wrap:
I 16.07 vs true 17.17; U 67.58 vs 121). The band between two encircling rims is a
winding-2 region (cf. the cyl/cyl barrel, which needed a per-fin winding flip,
Add. 275), and a cap bounded by a NURBS loop needs the pole anchor chosen by the
region (the circle-rim anchor logic does not apply).

**Questions (implementation-grade).**
1. **Boundary-flux over a NURBS-bounded spherical region.** State the exact
   Green/divergence surface-integral for the enclosed volume contributed by a
   spherical face bounded by arbitrary closed NURBS loops, including the anchor
   term: a CAP (contains a pole) vs a BAND (no pole) -- which anchor (pole vs a
   loop-derived reference) keeps the flux exact, and the sign/orientation rule per
   loop (outer vs inner).
2. **The winding-2 encircling band.** Diagnose and specify the band case: two
   encircling rims, the region wraps the full u-period, analogous to the cylinder
   barrel that required `|net| >= 2` per-fin winding handling. When is the flip
   needed vs when does it break the cap/pole anchor (the cyl gate was `|net|>=2`)?
3. **Multi-hole caps.** A cap carrying inner NURBS loops (a bore through the cap):
   outer-loop flux MINUS each inner loop, with inner-loop orientation. The sphere
   analog of #66's inner-hole integration.
4. **The area-witness (the WRONG-risk gate).** The sphere dual of the cyl/cone
   rectangle-witness: analytic cap/band area vs the tessellated face area, the
   tolerance tied to tessellation coarseness, the route to the boundary
   integrator on mismatch. This witness MUST be in place before any wrap decline
   guard is removed, else a self-consistent WRONG could slip within the
   op-volume-bound slack.
5. **Validation oracle.** Per-fragment mass == mesh == closed-form for: a sphere
   cut by two coaxial circles (three latitude bands, the circle-rim baseline that
   must NOT regress), and a sphere cut by two NON-coaxial NURBS wraps (barrel +
   two caps), the tight `quadric_sphere_op_volume` agreeing.

**Deliverable.** `docs/research/kernel/68-sphere-nurbs-rim-mass.md`: the
NURBS-bounded spherical boundary-flux formula with the cap/band anchor + winding
rules, the multi-hole generalisation, the area-witness with its guard-removal
soundness argument, and the per-fragment oracle.

---

## #3 -- NURBS-rim sphere tessellation watertightness: edge-conforming mesh at the shared NURBS seam with the cyl/cone wall

**Context.** `tessellate_sphere` (tessellate.rs) has a circle-rim clip and the
single-window-disc fan (Add. 281: a fan from an on-surface anchor to the loop's
canonical `fin_curve_samples`, watertight because a shared seam edge yields the
SAME point set, just fwd-reversed). A general NURBS WRAP rim (the band's two rims)
has no path, so the band meshes to the full sphere or overshoots, and the shared
edge with the cyl/cone wall is non-watertight -> the watertightness net declines.

**Questions (implementation-grade).**
1. **The band mesh.** The edge-conforming scheme for a sphere BAND bounded by two
   non-planar NURBS rims: a strip between the two `fin_curve_samples` rim
   polylines (the cyl/cone wall returns the SAME rim points), interior rows
   face-private so only the two shared rims must match. Give the row/column
   construction and the orientation.
2. **The general NURBS-trim path.** Generalise the circle-rim clip to an
   arbitrary closed NURBS loop on the sphere: an even-odd (u,v) stencil (the
   sphere analog of the landed `tessellate_cylinder` NURBS stencil), wrap-aware
   across the u=0 seam and degenerate at the poles.
3. **Watertightness proof.** The argument that two faces sharing a seam EDGE
   return an identical point set (fwd vs 1-fwd) so `mesh_open_ratio` ~ 0 by
   construction, extended from the window-disc case to the band's two rims and to
   the cap-with-NURBS-rim case.
4. **Validation.** `mesh_open_ratio` ~ 0 and `mass == mesh` on probe_wrap-class
   geometry for all 3 ops, across multiple sphere frames; the circle-rim and
   window baselines must NOT regress.

**Deliverable.** `docs/research/kernel/69-sphere-nurbs-rim-tessellation.md`: the
band-strip + general-NURBS-stencil tessellation for the sphere, the
edge-conforming watertightness proof, and the multi-frame validation.

---

**The one-line ask.** Round 0003 landed the cylinder periodic-split + multi-loop
mass; round 0004 asks for the SPHERE generalisation that left open -- frame-robust
NURBS-cut classify (#1), NURBS-rim band/cap mass with its area-witness (#2), and
NURBS-rim edge-conforming tessellation (#3) -- which together turn the dominant
non-coaxial sphere-band decline classes (~9,200, ~35% of the corpus) from
unrepresentable into FIXABLE, never WRONG, and hand the same NURBS-rim machinery
to the quartic and plane-cone classes behind them.
