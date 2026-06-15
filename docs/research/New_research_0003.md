# New Research Requests 0003 (queued for the research agent)

Drafted 2026-06-15. Implementation-grade dossier requests for the ONE remaining
gateway to the whole non-coaxial curved-boolean frontier: the **seam-aware
periodic-face split + canonical curved-seam identity at the IMPRINT stage**.
Round 0002 (dossiers #58 SSI matrix, #59 curved assembly, #60 curved mass)
PRESCRIBED this work; round 0003 asks for the three open INVESTIGATE items that
#59's own synthesis flagged as the genuinely-unsolved pre-coding unknowns, now
sharpened by a direct implementation attempt that ruled out the naive approach.

**Keel state (grounding, post-0002).** Tolerant-f64 non-manifold radial-edge
B-rep; Euler operators; arenas + generational keys + `EntityId`. Analytic
surfaces plane/cylinder/cone/sphere/torus + NURBS. Boolean pipeline:
`seam_curves (analytic SSI + recovered-NURBS) -> imprint_operand(a/b) ->
classify_faces (generalized winding number, 0.25 band) -> select_faces (Requicha)
-> stitch_by_import -> finalize_imported_assembly (Weiler region walk)`. The
CORRECTNESS PROGRAM IS COMPLETE (Add. 193: 2.24B fuzz, WRONG=0, three-bucket
oracle PASS/DECLINE/WRONG; contract = DECLINE-never-WRONG). The active program is
turning DECLINES into PASSES without ever emitting a WRONG.

**What landed since 0002 (so the agent does NOT re-derive it).**
- Dossier #60 mass-side is FULLY IMPLEMENTED: cone-apex anchor (Add. 252-255),
  the **generic boundary-arc Green-slab arm** `integrate_face_green` /
  `FinCurve::Nurbs` (Add. 271, sampling any `Curve3` for point+tangent+weight),
  and the **cone rectangle-witness** (massprops.rs ~905, the cone dual of the
  sphere witness, routes a partial-azimuth fragment to the boundary integrator).
- Cyl/cyl pieces 2+3 (Add. 272, commit ff6abbf, soak FAIL=0 both seeds,
  pass-NEUTRAL): `cylinder_face_interior_point` now avoids inner-loop HOLES
  (a multi-loop wall's interior point was landing in a bore hole), and
  `tessellate_cylinder` has an even-odd (theta,h) **stencil** for degree>=2
  NURBS-trimmed faces (the general analogue of the sphere multi-rim clip).
- The PLANAR two-conic periodic split WORKS: `cyl/cone INTERSECT slab` and
  `- slab` assemble three bands via the existing
  `imprint_closed_curve_crossing` (a planar circle/ellipse wrapping the lateral,
  split at the seam crossing P and antipode Q). So multi-band assembly is sound
  for PLANAR seams; the gap is purely NON-planar.

**The wall this round must break (sharpened by a real attempt, Add. 273).** The
unequal-radius perpendicular **cylinder/cylinder** boolean is the minimal
non-coaxial curved case (probe_cc2: A z-cyl r1, B x-cyl r0.6). Its SSI is two
closed NON-planar NURBS loops (x = +-sqrt(z^2+0.64)); on A's z-wall each is a
small theta-localised HOLE, on B's x-wall each ENCIRCLES the axis (wraps phi as
z spans the tube). Mass (exact), classify, and tessellation are DONE; the boolean
still DECLINES because of two coupled IMPRINT-stage gaps, both directly attempted
and both confirmed to need the genuinely-new machinery #59 named:
- **Piece 4 (seam-aware periodic split).** Generalising
  `imprint_closed_curve_crossing` to non-planar P/Q (P = the curve's crossing of
  the seam-line generatrix, Q = the antipodal curve parameter) and routing the
  encircling NURBS loop to it DID split B's wall, but produced DEGENERATE,
  zero-area band faces -- even for a SINGLE loop, and a loop-selection fix did not
  help. The incremental `mev`/`mef` SPUR surgery (split seam at P, mev to Q, mef)
  is FUNDAMENTALLY UNSUITED to non-planar wraps: the full-revolution (theta,h)
  band loop collapses. The fix is NOT a fin/loop tweak -- it is the principled
  #59 Q1 step-3 re-threading: partition the periodic (u,v) domain by the seam
  curve(s) and construct each band's closed pcurve loop explicitly.
- **Piece 5 (canonical seam identity).** Even with a clean split, A's hole-oval
  (imprinted as one closed ring edge) and B's band-boundary oval (split at its
  OWN seam-line crossing, a DIFFERENT point on the same shared curve) carry
  different vertices -> "unmatched coedge". Both operands must subdivide the ONE
  shared seam curve at the SAME canonical nodes and share the `EdgeId`.

These same two gaps block EVERY non-coaxial quadric quartic (cyl/sphere ~2509,
cone/sphere ~683 -- the dominant decline classes) and the parabola/hyperbola
plane-cone (cone/block ~750). Solving them on cyl/cyl is the prototype that
unlocks the frontier (the sphere/cone downstream then needs its circle-rim
machinery generalised to NURBS rims -- a follow-on, not this round).

**Recommended send order:** #1 first (the periodic-domain arrangement -- the only
genuinely-new algorithm, and the centerpiece), then #2 (canonical seam identity
-- depends on #1's domain representation), then #3 (the curved multi-loop-face
mass witness -- the WRONG-risk precondition before the decline guard is removed).

---

## #1 -- The seam-aware periodic-domain split: partition a periodic (cone/cylinder/sphere) parameter domain by one or more seam curves and re-thread each fragment's closed pcurve loop -- THE genuinely-new algorithm

**Context.** A cone/cylinder lateral (and a sphere) is ONE periodic face. Its
(u,v) domain is a rectangle u in [0, 2*pi) (identified at the seam slit), v axial
(the cone collapsing to the apex at one end, the sphere to poles). Round 0002's
dossier #59 Q1 prescribed "lift the seam to its closed-form pcurve preimage,
detect seam-slit/apex crossings, subdivide, RE-THREAD each fragment loop closed,
build the 3D edge once." Keel currently does this only via incremental Euler
surgery (`split_edge` + `mev` + `mef` in `imprint_closed_curve_crossing` /
`imprint_crossing_pair`), which works for PLANAR conic wraps but is confirmed
DEGENERATE for non-planar encircling NURBS seams and for the general multi-seam
case (Add. 273). The dossier explicitly named the 2D periodic-domain arrangement
"the only genuinely-new algorithm, needed at the last rung."

**Questions (implementation-grade).**
1. **The parametric arrangement.** Given a periodic face's (u,v) rectangle (with
   the u-seam identification and the apex/pole degenerate v-edge) and a set of K
   seam pcurves (closed encircling loops that wrap u, open arcs that cross the
   seam slit, arcs that terminate at the apex/pole), what is the robust algorithm
   to partition the domain into the resulting regions and emit one B-rep face per
   region with a CLOSED, correctly-oriented pcurve loop? Cover the cylinder
   (K parallel encircling loops -> K+1 bands), the cone (loops + an apex-reaching
   arc), and the sphere (loops + a pole-reaching arc). How do production kernels
   (OCCT `BOPAlgo_BuilderFace` over a periodic face, ACIS) realise this -- do they
   build faces from the 2D arrangement directly, or via constrained Euler
   operators, and exactly how is the seam-slit re-threading done so a band that
   wraps the full u-period is a valid (non-degenerate, nonzero-area) face?
2. **Why the incremental spur collapses (and the correct alternative).** Diagnose
   the confirmed failure: a non-planar encircling loop imprinted via
   split-seam-at-P + mev-to-antipode-Q + mef yields a zero-area band whose
   interior point projects off the surface. Is the root the full-revolution
   (u,v) loop's winding/area degeneracy, the spur's arc-bounds vs the full
   attached curve, or the antipode choice? Give the construction that does NOT
   collapse -- the explicit fragment-loop re-threading (rim sub-arc + seam-slit
   sub-edge + the encircling seam arc), with the orientation/sense rule per face.
3. **Closed-form parametric preimages.** For step-1 to be EXACT (the dossier's
   "AVOID marching a seam that has a closed-form preimage"), give the closed-form
   pcurve u(v) or v(u) on each surface for each analytic seam class Keel supports:
   plane-cylinder (ellipse/line), plane-cone (circle/ellipse/parabola/hyperbola),
   cylinder-cylinder (the per-ruling quartic), cone-cylinder, cone-sphere,
   cylinder-sphere, sphere-plane. Where no exact preimage exists, the
   certified-NURBS-recovery convention and the SameParameter tolerance bound.
4. **Apex / pole / seam-slit special cases.** The degenerate v-edge (apex/pole)
   as a legal loop member (a fragment whose loop PASSES THROUGH the apex once, not
   a doubled cover); a seam that lands exactly on the existing u-seam slit; a
   loop tangent to the apex. Which are representable (re-thread) vs DECLINE-forever
   (degenerate conic through the apex, #59 Q5).
5. **Validation oracle.** The exact analytic surface area and enclosed volume of
   each band/fragment for: a cylinder cut by two parallel oblique planes (two
   ellipse wraps -> three bands), a cone cut by two coaxial circles (three
   frustum bands incl. the tip), and the unequal cyl/cyl (two non-planar quartic
   wraps -> barrel + two stubs). mass == mesh == closed-form per fragment, and
   fragment-count-out == kept-face-count-in.

**Deliverable.** `docs/research/kernel/NN-periodic-domain-seam-split.md`: the
periodic (u,v) arrangement + fragment-loop re-threading algorithm, the
collapse diagnosis + correct construction, the closed-form preimage table, the
apex/pole/slit case taxonomy, and the per-fragment validation oracle. This is the
gateway to the entire non-coaxial curved frontier.

---

## #2 -- Canonical curved-seam identity: the tolerant EdgeId hash and the shared cross-operand subdivision

**Context.** Dossier #59 Q2 named this "the single most important new piece," and
Add. 273 confirmed the failure mode: Keel ALREADY computes each seam once
(`seam_curves` produces one shared `SeamCurve` with the same `Curve3`), but each
operand's `imprint_operand` then SUBDIVIDES that curve at its OWN seam-line
crossing -- different points on the same curve -- so operand A's seam edges and
operand B's never share vertices and the radial cycle cannot close
("unmatched coedge", the Hoffmann divorce). The PLANAR Steinmetz case escapes
this only because the two ellipses cross each other at two shared points both
operands split at; the general (unequal, non-crossing) case has no such shared
points. The planar analog `subdivide_seam_ring` exists; the curved analog does
not.

**Questions.**
1. **The canonical subdivision.** What deterministic rule subdivides ONE shared
   seam curve at the UNION of all nodes both operands need (each operand's
   periodic seam-slit crossings, apex/pole nodes, and any other-seam crossings),
   assigning each sub-edge a stable child id, so BOTH operands' fragments
   reference the SAME child `EdgeId` and pair by identity -- not by coordinate
   compare at stitch time? How do OCCT (`AttachExistingPCurve`) and ACIS realise
   "one section edge shared to both faces with a consistent pcurve each"?
2. **The tolerant `EdgeId` hash (the stability condition).** Two geometrically
   equal seam computations (or the same shared curve sampled in two traversal
   senses) must hash to the SAME id under tolerant-f64 perturbation. Give the
   canonical orientation key (e.g. ordered by a frame axis, ties by the surfaces'
   `EntityId` tuple) and the tolerant node-snapping done ONCE at canonicalisation
   (the dossier's allowed use of tolerance), with the proof that geometrically
   equal seams cannot hash apart and geometrically distinct seams cannot collide.
3. **Sense relative to each incident face.** Once the seam edge is one canonical
   entity with multiple pcurves, how is each fragment's coedge sense derived from
   that face's natural normal (not from traversal order), so the radial-2 pairing
   is consistent across the two operands and the Weiler region walk is unchanged.
4. **Interaction with #1.** The subdivision nodes feed the periodic-domain
   arrangement; specify the contract between #1 (which regions exist) and #2
   (which canonical sub-edges bound them) so the two compose without a second
   reconciliation pass.

**Deliverable.** `docs/research/kernel/NN-canonical-curved-seam-identity.md`: the
canonical subdivision + orientation rule, the tolerant `EdgeId` hash with its
stability proof, the per-face sense derivation, and the #1-interface contract.
Validated on the unequal cyl/cyl (A's hole-ovals pair 1:1 with B's band-boundary
ovals; shell closes; mass == mesh).

---

## #3 -- The curved multi-loop-face mass witness (cylinder dual of the landed cone/sphere rectangle-witness) -- the WRONG-risk precondition before the decline guard is removed

**Context.** Pieces 2+3 made cyl/cyl classify + tessellation correct, but with
the decline guard temporarily off the UNION and DIFFERENCE mass OVER-READS:
`A UNION B` mass = 17.09 = V(A)+V(B) and `A - B` mass = 12.566 = V(A), because the
kept multi-loop WALL face (the lateral with two oval HOLES) is integrated by the
Green-slab over its OUTER loop without subtracting the inner hole loops -- exactly
the dossier #60 #3 iso-rectangle over-count, but for a CYLINDER and for INNER
loops. The cone and sphere already have rectangle-witnesses (massprops.rs ~877,
~905) that route a region-mismatched face to the boundary integrator; the
CYLINDER has none, and the multi-loop (holed) case is not handled. This is the
one gap that, once the cyl/cyl guard is removed, could let a SELF-CONSISTENT
WRONG slip (if a tessellation also over-counts within the op-volume-bound slack).

**Questions.**
1. The cylinder analog of the cone/sphere rectangle-witness: the analytic
   full-band lateral area (`2*pi*r*h`) vs the tessellated face area, the
   tolerance tied to tessellation coarseness, and the route to the boundary
   integrator when they disagree.
2. The INNER-LOOP (hole) generalisation: a periodic lateral with K closed inner
   seam loops (bore holes) must integrate as outer-loop flux MINUS each inner
   loop's flux, with the correct inner-loop orientation. State the boundary-flux
   formula over a multi-loop curved face and how the Green-slab `node()`
   accumulation handles inner loops (the cylinder/cone/sphere cases).
3. The composition with #1/#2: once the periodic split + canonical identity make
   the holed wall a proper multi-loop face, confirm the mass integrator and the
   `mass==mesh` gate agree to the curved band, so the guard removal is sound
   (DECLINE-never-WRONG preserved).

**Deliverable.**
`docs/research/kernel/NN-curved-multiloop-face-mass.md`: the cylinder rectangle
witness, the multi-loop inner-hole boundary-flux integration, and the
guard-removal soundness argument. Validated on the unequal cyl/cyl UNION and
DIFFERENCE (mass == mesh == closed-form), and a radially-bored cylinder.

---

**The one-line ask.** Round 0002 proved curved assembly is canonical-seam-identity
+ seam-aware periodic split at the IMPRINT stage feeding the UNCHANGED stitcher;
round 0003 asks for the three pieces that proof left open -- the periodic-domain
arrangement (#1), the tolerant canonical `EdgeId` (#2), and the multi-loop-face
mass witness (#3) -- which together turn the dominant non-coaxial curved decline
classes from unrepresentable into FIXABLE, never WRONG.
