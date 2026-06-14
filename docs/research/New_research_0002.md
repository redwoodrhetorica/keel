# New Research Requests 0002 (queued for the research agent)

Drafted 2026-06-14. Implementation-grade dossier requests for the CURVED-boolean
frontier -- the decline classes that dominate the evolutionary-explorer corpus
(54,313 declines) after the disjoint-union-of-curved fix (LOG Addendum 250).
Grounded in Keel's actual state.

**Keel state (grounding).** Tolerant-f64, non-manifold PES / radial-edge B-rep;
Euler operators; arenas + generational keys + `EntityId` identity. Analytic
surfaces plane / cylinder / cone / sphere / torus + NURBS curves/surfaces.
SSI dispatch `analytic_analytic` (crates/keel-geom/src/ssi.rs) currently has
ONLY: plane-sphere, sphere-sphere, plane-cylinder, cylinder-cylinder, plane-cone.
Boolean pipeline: `preimprint_coincident_overlaps -> seam_curves (analytic SSI +
recovered-NURBS) -> imprint_operand(a/b) -> classify_faces (generalized winding
number, 0.25 band around w=0.5) -> select_faces (Requicha tables) ->
stitch_by_import -> finalize_imported_assembly (Weiler face-side region walk)`.
The polygon-soup stitcher is RETIRED; ALL results now go through
stitch_by_import + finalize. Mass: divergence-theorem `mass_properties`
(per-face, sense-exact); curved trims use the Green-slab boundary integrator
`integrate_face_green`. The CORRECTNESS PROGRAM IS COMPLETE (Add. 193: 2.24
billion fuzz executions, WRONG = 0 under a three-bucket oracle PASS / DECLINE /
WRONG; the contract is DECLINE-never-WRONG). Parity ~137/144. The active program
is turning DECLINES into PASSES without ever emitting a WRONG.

**Why these six.** Grounding the cone work in the corpus (LOG Add. 250) showed
the curved frontier is gated, in order, by: (a) MISSING SSI rungs -- overlapping
curved pairs cannot even find a seam; (b) the curved-seam assembly stitch;
(c) curved-trim mass integration (the mass==mesh gate that guards every result).
#1 is the highest blast radius -- nothing overlapping-curved can assemble
without it. #4-#6 are independent and individually shippable.

**Recommended send order:** #1 first (the SSI matrix -- unblocks #2 and #3),
then #2 (curved assembly), then #3 (curved mass), then #4 (disjointness
certificate), #5 (sphere-difference defect), #6 (tessellation accuracy).

---

## #1 -- Complete the analytic surface-surface-intersection matrix (cyl-sphere, cone-sphere, cone-cylinder, cone-cone, torus pairs) -- HIGHEST LEVERAGE

**Context.** `analytic_analytic` in ssi.rs dispatches a closed-form SSI per
surface-type pair and returns `SsiResult::Curves(Vec<Curve3>)` (with branch /
arc-sweep metadata) or declines. Implemented rungs: **plane-sphere** (circle),
**sphere-sphere** (circle), **plane-cylinder** (ellipse / line pair / circle),
**cylinder-cylinder** (the certified closed-form branch field, per-ruling
quadratic, q2 = 1), **plane-cone** (the conic family: circle / ellipse /
parabola / hyperbola / line pair). **MISSING** (fall through to the default arm
-> Unsupported -> the boolean finds no seam): **cylinder-sphere**,
**cone-sphere**, **cone-cylinder**, **cone-cone**, and EVERY **torus** pair
(torus-plane, torus-cylinder, torus-sphere, torus-cone, torus-torus).

**The impact (concrete).** When two OVERLAPPING curved solids hit a missing
rung, `seam_curves` returns empty, the no-seam shortcut mis-reads them as
disjoint/nested, and the result is either a mass!=mesh decline or (worse path) a
spurious clone. The explorer corpus shows exactly this: large cone/sphere,
cone/cone, cone/cylinder, cylinder/sphere overlapping buckets that decline.
These are not assembly bugs -- the kernel literally cannot compute the
intersection curve.

**Questions (implementation-grade).**
1. **Cylinder-sphere.** The intersection is a quartic space curve in general
   position; coaxial -> exact circle(s); axis-through-center cases. What is the
   robust closed-form / low-degree representation? Keel has no general
   space-curve type beyond NURBS -- should these be recovered as degree-? NURBS
   to a tolerance, or is there an exact rational form? Branch structure
   (how many components, when they split/merge) and the arc-sweep metadata
   needed so the downstream stitch and the Green-slab can integrate them.
2. **Cone-sphere, cone-cylinder, cone-cone.** Each is a degree-4 space curve in
   general position with degenerate sub-cases (coaxial, apex-on-surface,
   tangent, equal-half-angle cone-cone -> conic). Give the case taxonomy, the
   exact form where one exists (coaxial cone-sphere -> circle; equal-cone
   tangent -> line/conic), and the recovery-to-NURBS strategy otherwise, with
   the tolerance bound and the component/branch enumeration.
3. **Representation choice.** Keel's `Curve3` is Line / Circle / Ellipse / NURBS
   (no Hyperbola, no general quartic). What minimal curve-type additions or
   NURBS-recovery conventions make the new rungs first-class for the stitch
   (shared cross-operand seam edges must compare equal) and for mass
   integration (the boundary integrator needs point + tangent at parameter t)?
4. **Torus rungs.** Torus-plane (Villarceau / general quartic), torus-cylinder,
   torus-sphere, torus-cone, torus-torus -- the practical closed-form-or-recover
   strategy and degenerate cases. Lower priority than the cone/cyl/sphere
   rungs but needed for torus parity.
5. **Robustness near tangency / degeneracy.** How production kernels certify the
   branch count and avoid dropping a near-tangent grazing component (the
   soundness analog of cylinder_cylinder's certified branches), so a missed
   component never silently under-cuts.
6. **Validation oracle.** For each rung, the analytic test cases (coaxial,
   general, tangent, apex/axis-coincident) with known curve types and a
   Monte-Carlo cross-check of the resulting boolean volume.

**Deliverable.** `docs/research/kernel/NN-ssi-curved-matrix.md`: per-pair
closed-form / recovery algorithm, case taxonomy, the curve-representation
recommendation, branch-soundness guarantee, and a validation table. Unblocks
the entire overlapping-curved boolean frontier (cone/cyl/sphere/torus families).

---

## #2 -- Curved boolean assembly: stitching cross-operand CURVED seams into a valid shell

**Context.** Once #1 yields seam curves, the boolean must imprint them onto both
operands, classify the fragments, and stitch. Keel's `stitch_by_import` +
`finalize_imported_assembly` already assemble PLANAR results and the
single-cut curved cases (the cone-oracle drilling/countersink passes), but the
explorer shows `UnassemblableSeam` (cone/cyl) and faulted / mass!=mesh declines
for multi-seam curved overlaps. Add. 250 also documented that the disconnected /
cellular finalize path mis-built curved components (now bypassed for the
PROVABLY-disjoint case via `combine_disjoint`, but the genuinely-overlapping
curved assembly remains).

**Questions.**
1. **Curved seam imprint.** Splitting a periodic curved face (cone lateral with
   a seam-slit + apex; cylinder lateral; sphere with a pole seam) along an
   imprinted conic / quartic seam, keeping pcurves consistent so the fragment
   tessellates and integrates. The seam-slit + apex topology specifically (the
   cone lateral is ONE face whose loop runs rim -> seam-up -> apex -> seam-down).
2. **Cross-operand seam pairing.** After importing fragments from both operands,
   the shared seam edge must pair 1:1 (radial-2). What canonical seam
   subdivision / identity makes a conic or recovered-NURBS seam from operand A
   compare equal to the same seam from operand B (the curved analog of the
   planar `subdivide_seam_ring`)?
3. **Disconnected & cellular curved finalize.** `connected_face_components` +
   the Weiler region walk over curved fragments: how to derive one solid cell
   per connected component (and enclosed-void cavities) robustly when faces are
   periodic / apex-bearing. (Add. 250: a clean disjoint import already resolves
   correctly; the open case is overlapping curved cells.)
4. **The drop-proof invariant** for curved fragments (the curved analog of
   research 0001 #1): every selected curved fragment lands in the output shell.
5. **Decline policy** for the genuinely unrepresentable (self-intersecting seam,
   sub-tolerance sliver): keep DECLINE-never-WRONG, but maximize the
   representable set.

**Deliverable.** `docs/research/kernel/NN-curved-boolean-assembly.md`, validated
against an overlapping cone+cylinder union and a sphere+cone difference, with
mass==mesh==Monte-Carlo.

---

## #3 -- Curved-trim mass integration completeness (the Green-slab gaps)

**Context.** Every assembled result is gated by mass==mesh (2% curved
self-consistency band). For curved trims Keel integrates per face via the
Green-slab boundary integrator `integrate_face_green` (boundary flux, GL8,
`local_geometry(u,v)` for cone/cyl/sphere; a `node(point, tangent, weight)`
helper accumulates the u-winding and slab samples). It has real gaps that block
correctly-assembled curved results from passing the gate.

**The gaps (concrete, from LOG Add. 250 KL3/KL4).**
- (a) A **winding +-1 full-revolution cone-APEX face** (the tip above a cut)
  declines "unsupported boundary winding". The sphere case has a POLE anchor;
  the cone apex needs the analogous anchor.
- (b) A **high-degree NURBS / hyperbolic boundary arc** is rejected at the
  `Curve3::Nurbs(n) if n.degree() > 1` guard. A plane parallel to a cone axis
  cuts a HYPERBOLA (no Hyperbola variant -> stored as deg>1 NURBS), so a notched
  cone fragment cannot be integrated.
- (c) **Notched / partial-azimuth cone fragments** take the full-azimuth
  iso-rectangle path and OVER-count the removed wedge (synthetic probe: mass
  17.07 vs true 11.85) because non_iso detection misses pcurve-less cut edges.

**Questions.**
1. The cone-apex anchor for a divergence/boundary-flux integrator (the cone
   analog of the sphere pole), including the complement/island detection.
2. A **generic boundary-arc** integration arm: GL quadrature sampling an
   arbitrary `Curve3` (NURBS / conic) for (point, tangent) -> the node()
   contribution, so any seam curve from #1 integrates. Convergence / panel
   count for conics and recovered quartics.
3. The robust **partial-fragment witness**: detect that an iso-rectangle box is
   NOT the face's region (e.g. analytic band area vs tessellated area, the
   cone analog of the existing sphere rectangle-witness) and route to the
   boundary integrator. Tolerance vs cone tessellation coarseness.
4. Whether to add a **Hyperbola** `Curve3` variant (exact) vs always recovering
   to NURBS -- impact on the integrator and the seam-equality compare in #2.

**Deliverable.** `docs/research/kernel/NN-curved-mass-integration.md`, validated
on a notched cone fragment, a cone tip above a cut, and a hyperbolic-cut cone
band, each mass == Monte-Carlo.

---

## #4 -- A PROVABLE disjointness certificate (safely extend the disjoint-union combine)

**Context.** Add. 250 added `combine_disjoint`: a provably-separated union skips
imprint/SSI and imports both operands verbatim (correct by construction). It
fires only when the AABBs are apart by a margin. Pairs whose AABBs OVERLAP but
whose SOLIDS do not touch still hit the broken overlapping path. Extending the
combine there is UNSAFE without a non-intersection PROOF: neither the op-volume
bound nor mass==mesh catches a missed intersection (two overlapping closed
surfaces each integrate/tessellate to their full v_a+v_b, the union's upper
bound, so a double-counted overlap looks in-band). Keel's existing `min_distance`
is tessellation-based and only vertex-to-triangle (misses edge-edge minima ->
NOT a sound lower bound); `clashes()` relies on SSI completeness.

**Questions.**
1. An **analytic / certified lower bound** on the distance between two B-rep
   solids (or a separating-axis / interval certificate) usable as a rigorous
   "they do not touch" proof, for analytic + NURBS faces. Sound (never reports a
   gap that is not there), cheap enough for the broad phase.
2. Alternatively, **SSI completeness**: under what conditions is "seam_curves
   returned empty" itself a proof of non-intersection? The tilted-cylinder/
   cylinder class shows SSI can currently MISS a real intersection -- what
   certification (interval Newton, degree bounds) closes that gap so the no-seam
   shortcut is sound for curved operands?
3. The interaction with the tolerant model: how the certificate composes with
   tolerant-f64 vertices / edges so a genuine sub-tolerance touch is not
   certified as disjoint.

**Deliverable.** `docs/research/kernel/NN-disjointness-certificate.md`, with the
certified-separation test and the SSI-completeness conditions, validated against
interleaved-AABB-but-disjoint curved pairs and the tilted-cyl/cyl
missed-intersection case.

---

## #5 -- The sphere/sphere DIFFERENCE malformed result (the sphere-split integration trap)

**Context.** Overlapping sphere - sphere returns a MALFORMED body: mass declines
("non-positive volume, orientation conventions violated") and mesh ~ 0, yet the
boolean returns Ok -- a SILENT-MALFORMED that the assemble gate passes (it is
the only residual class that escapes DECLINE-never-WRONG as a wrong-positive
mesh). It is the two FAILs in the latest explorer sweep. Internally mapped as
three stacked defects (the spherical cap trim's pcurve/seam handling, the
region/sense of the carved cavity wall, and the Green-slab anchor for the
remainder face).

**Questions.**
1. The correct cap-trim topology for a sphere carved by another sphere (the lens
   cap removed): seam circle, the kept remainder face, pcurves, and the
   reversed cavity-wall orientation/sense.
2. Why mass integrates to a non-positive volume here (orientation convention
   violated) while the planar analog is clean -- the curved sense convention at
   a carved concave spherical wall.
3. The minimal fix that makes sphere - sphere either PASS (mass==mesh==truth) or
   DECLINE honestly -- never return the malformed Ok body.

**Deliverable.** `docs/research/kernel/NN-sphere-difference-trap.md`, validated
on overlapping sph - sph (proper cavity) and the near-tangent thin-lens case.

---

## #6 -- Curved tessellation accuracy vs the self-consistency gate (perf-budgeted)

**Context.** The mass==mesh gate uses a 2% curved band. Cone (and other curved)
tessellation runs ~0.5-1.7% off mass, worse near the apex; coarse curved meshes
can false-decline a correct result, and a denser blanket mesh would regress the
optimization leg (boxes 0.9 ms; the perf target is Parasolid-class).

**Questions.**
1. **Curvature-adaptive** tessellation density (sagitta / chord-error bound) for
   cone / cylinder / sphere / torus / NURBS so the tessellated volume is within a
   target fraction of the exact mass, with bounded triangle counts.
2. Apex / pole / high-curvature special handling (the apex triangles dominate
   the cone error).
3. Whether to gate on an **analytic** area/volume reference instead of the
   tessellation where one exists (decouple the gate from mesh density).
4. The perf budget: density vs the winding/mesh/boolean hot loops; what the
   reference kernels use for the mass-gate vs the display mesh.

**Deliverable.** `docs/research/kernel/NN-adaptive-curved-tessellation.md`, with
the error-bounded density rule and a perf analysis, validated on cone/sphere
volume convergence.
