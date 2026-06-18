# New Research Requests 0005 (queued for the research agent)

Drafted 2026-06-18. Implementation-grade dossier requests for the **compound-body
fillet/mass frontier** -- the dominant decline-and-wrong source on REALISTIC
modeling workflows, as opposed to the random-primitive corpus rounds 0003/0004
addressed. A new grammar-driven soak (`examples/realsoak.rs`, distilled from 113
Plasticity tutorial transcripts: median-14 op chains, form->combine->detail->finish
arc, the `boolean then fillet the seam` backbone) ran 10,000 long op-chains on a
growing solid under the three-bucket oracle and concentrated the kernel frontier
into three named problems, all on the #1 real op (FILLET) applied to COMPOUND
(post-boolean) bodies: an end-cap TOPOLOGY decline class, a post-fillet curved
MASS class, and a disjoint curved-union MASS class. Round 0005 asks for the
algorithm for each.

**Keel state (grounding, post-0004).** Tolerant-f64 non-manifold radial-edge
B-rep; Euler operators; arenas + generational keys + `EntityId`. Analytic surfaces
plane/cylinder/cone/sphere/torus + NURBS. Boolean pipeline: `seam_curves
(analytic SSI + recovered-NURBS) -> imprint_operand(a/b) -> classify_faces
(generalized winding number, 0.25 band) -> select_faces (Requicha) ->
stitch_by_import -> finalize_imported_assembly (Weiler region walk)`; an
AABB-separated pair short-circuits (`combine_disjoint` for a disconnected union).
Fillet engine (`blend.rs`): a rolling-ball blend cylinder per convex/concave edge,
spring-line imprint on each support, an END-CAP planner per end vertex, then
Euler dissolve (`kef`/`kev`) to one blend face. Mass via the analytic Green-slab
boundary-flux (`integrate_face_green`, massprops.rs) with the divergence theorem;
tessellation cross-checks it. The CORRECTNESS PROGRAM IS COMPLETE (Add. 193: 2.24B
fuzz, WRONG=0, three-bucket oracle PASS/DECLINE/WRONG; contract =
DECLINE-never-WRONG). The active program is turning DECLINES into PASSES without
ever emitting a WRONG -- now measured on REALISTIC workflows, not just primitives.

**What landed since 0004 (so the agent does NOT re-derive it).**
- The realistic-workflow soak `examples/realsoak.rs` landed: a deterministic
  grammar generator (seed reproduces a project; `KEEL_REPRO=<seed>` replays one
  verbosely) + a parallel worker pool + the three-bucket oracle generalized to a
  growing solid carrying an independent volume interval. 10,000 projects, seed 1:
  after the floor fix below, 7,867 PASS / 2,133 STALL / **0 WRONG** / 0 TIMEOUT;
  ~67,000 kernel-frontier declines across ~210 classes are the worklist this round
  draws from.
- A DECLINE-never-WRONG FLOOR fix landed (commit 3d49fee): the soak's FIRST pass
  surfaced 18 WRONG (NOT wrong bodies -- wrong analytic `mass_properties` on
  CORRECT geometry, mesh right, that the kernel returned without self-detecting).
  A curved-lenient (>25%, the oracle's own curved-WRONG threshold) `mass==mesh`
  self-check now DECLINES them: in `fillet_edge`'s return, and in the disjoint-Union
  arm of `boolean_with`. Verified WRONG 18->0 at 10k, no regression. This round
  asks to UPGRADE those declines to PASSES by fixing the root mass bugs (#2, #3).
- The sphere NURBS-rim round (0004 dossiers #67/#68/#69) groundwork is in: the
  cyl/sphere WRAP passes all three ops (`imprint_cylinder_wrap_bands`, the
  frame-free sphere NURBS-cut classify, the NURBS-rim sphere tessellation), tight
  oracle `quadric_sphere_op_volume` backing the gate.
- The multi-cut cap-plane phantom (Add. 294) is fixed; coaxial sphere/cyl/cone
  families PASS exact.

**The wall this round must break (each sharpened by a direct attempt this round).**
The realistic soak proves that clean PRIMITIVE fillets work but COMPOUND-body
fillets (the universal real workflow) hit three walls:

- **END-CAP TOPOLOGY (the dominant DECLINE class).** `fillet:Precondition` is among
  the largest kernel-frontier classes (~3,000+ across `extrude->fillet`,
  `fillet->fillet`, `chamfer->fillet` chains). Root cause confirmed by reading the
  surgery: the end-cap planner `split_cap` (blend.rs:765) handles only TWO end
  topologies -- `Single` (one transverse cap face) and `Roof` (two cap faces at a
  ridge) -- and DECLINES everything else: `"multi-face cap (follow-up)"`
  (blend.rs:853, the end vertex has 3+ faces) and `"cap face parallel to the spine
  (extend follow-up)"` (blend.rs:760, the blend must EXTEND to reach a parallel
  cap). On a post-boolean body the filleted edge routinely ends at exactly such
  vertices (a peg meeting a block, 3+ faces; or an end cap parallel to the seam),
  so the clean-primitive cases barely occur after the first op.
- **POST-FILLET CURVED MASS (a WRONG class, now floored: 16 of the 18).** Repro
  seed `11400715918834829014`: a fillet on a compound body computes analytic mass
  8405 vs the true ~6170 (mesh). Diagnosed by per-face flux dump
  (`KEEL_MASS_DEBUG`): the BLEND face itself integrates correctly (small flux); the
  over-count is the fillet-TRIMMED support cylinder face, which switches from the
  EXACT iso-rectangle integration path (correct, full untrimmed cylinder) to the
  GREEN-SLAB path after the spring-line imprint, and the green-slab mis-integrates
  that post-fillet loop/pcurve configuration. Critically: the MESH is correct and
  uses the same loops, so the surgery GEOMETRY is sound (a bad winding would
  corrupt the tessellation too) -- the bug is purely in the analytic integration of
  a geometrically-correct trimmed-cylinder face.
- **DISJOINT CURVED-UNION MASS (a WRONG class, now floored: 2 of the 18).** Repro
  seed `11400715918834829910`: a sphere mirror+union (two disjoint curved lumps)
  computes analytic mass 238 vs the true 14441 (mesh). The `combine_disjoint` fast
  path (AABB-separated union) produces a topologically valid disconnected body, but
  `mass_properties` COLLAPSES on it while the render mesh is correct. (Dossier #60
  curved-mass-integration and the prior disjoint-union-of-curved work fixed many
  cases; this mirror+union sub-case still collapses.)

**Dead ends (proven this round; the agent must NOT propose these).**
- The DECLINE-never-WRONG floor (the `mass==mesh` self-check, commit 3d49fee) is a
  SAFETY net, NOT a solve: it converts WRONG to DECLINE. Re-proposing a guard is
  not progress; the ask is to make these PASS.
- A naive per-face "analytic dv vs mesh dv" localizer is CONFOUNDED: per-face flux
  contributions use different origin references and interior partition walls are
  skipped by the analytic path but not the naive mesh sum, so they do not match
  per-face even when both are correct. The reliable signal is the WHOLE-body
  mass-vs-mesh gap plus the iso->green-slab path switch on the trimmed face.
- Editing the green-slab integrator blind is forbidden: it is the most
  delicately-tuned code in the kernel (task-36/41 winding normalization,
  dossier-60/64 anchors, the per-fin `|net|>=2` flip). Any change MUST come with
  the regression argument that the coaxial-circle, cyl/cyl-barrel, and
  sphere-band baselines do not move.

**Recommended send order:** #1 (end-cap topology -- the largest decline class, an
ADDITIVE surgery extension with low regression risk), then #2 (post-fillet curved
mass -- the WRONG-class precondition, research-first before touching the
integrator), then #3 (disjoint curved-union mass).

---

## #1 -- Fillet end-cap topology beyond Single/Roof: the multi-face cap and the parallel-cap extend, on compound (post-boolean) bodies

**Context.** The fillet end-cap planner `split_cap` (blend.rs:765) closes each end
of the blend tube against the model. It collects the cap faces at the end vertex
(those NOT the two trimmed supports) and handles exactly two topologies:
`[cap]` -> `Single` (one transverse cap face, the blend's conic end-arc splits it)
and `[c0,c1]` -> `Roof` (two cap faces meeting at a ridge, the ridge splits where
it first crosses the blend cylinder). It DECLINES on `_` (zero or 3+ cap faces:
`"multi-face cap (follow-up)"`, blend.rs:853) and when a cap face is PARALLEL to
the blend spine (`conic_for`, blend.rs:760: `"cap face parallel to the spine
(extend follow-up)"` -- the blend would have to EXTEND to reach it, prior art's
classic blend-overflow). On compound bodies these are the COMMON cases, hence the
dominant `fillet:Precondition` decline class. This is the runout/overflow family of
dossiers #41/#54/#56 made CONCRETE for Keel's end-cap surgery.

**Questions (implementation-grade).**
1. **The multi-face cap (3+ faces at the end vertex).** Given a blend cylinder
   ending at a model vertex where K>=3 non-support faces meet, what is the surgery
   to cap the tube? Specify the generalization of `Roof`: the blend cylinder's end
   circle crosses several model faces; each crossed face trims to its own conic
   sub-arc and the arcs chain around the tube end. Give the ordering rule (which
   face the tube enters/exits), the per-face conic (circle when the cap is
   perpendicular to the spine, ellipse when oblique), the new vertices at each
   crossing, and the Euler operator sequence (the generalization of the `kef`/`kev`
   dissolve at blend.rs:865-889). Where is it genuinely unrepresentable (a vertex
   the rolling ball cannot reach) -> DECLINE-forever?
2. **The parallel-cap extend.** When the end cap is PARALLEL to the spine, the
   blend must extend along the spine to meet it (the classic blend overflow). State
   the extend construction: how far to extend, the surface the extended blend
   carries (still the same cylinder, lengthened), the trim against the now-reached
   cap, and the precondition under which extend is bounded vs runs away (a cap that
   never closes -> DECLINE). Relate to prior-art overflow handling (Parasolid
   "blend overflow", ACIS).
3. **Vertex where the filleted edge meets a boolean seam.** The canonical
   real-workflow case: `boolean then fillet the seam`, where the seam edge ends at
   a vertex created by the boolean (a peg corner). Characterize the local topology
   (faces, convexity mix) and which of the above two cases it reduces to, and the
   setback/mitre interplay if the adjacent edge is also to be filleted (dossiers
   #53/#55).
4. **Production-kernel realization.** How do Parasolid (`PK_FACE_make_blend`
   overflow), ACIS (`blend_overflow`), and OpenCascade (`ChFi3d` cap/corner
   handling) close a blend tube against a multi-face corner and against a parallel
   cap, including the "extend at most two adjoining faces" rule the current
   Single/Roof split already cites?
5. **Validation oracle.** A battery on compound bodies: union a transversal peg
   into a block then fillet the seam edges (multi-face corners), fillet an edge
   whose end cap is parallel to the edge (extend), and fillet two adjacent edges
   sharing a corner (setback). Assert validate() + mass==mesh + watertight, and
   that the previously-declining `KEEL_REPRO` realsoak seeds for `extrude->fillet`
   / `fillet->fillet` now PASS. The clean-primitive Single/Roof baselines must NOT
   regress.

**Deliverable.** `docs/research/kernel/70-fillet-endcap-topology.md`: the
multi-face-cap and parallel-cap-extend surgeries with their Euler sequences, the
boolean-seam corner reduction, the production-kernel notes, and the compound-body
validation battery.

---

## #2 -- Post-fillet curved-face mass: why the Green-slab mis-integrates a fillet-trimmed cylinder, and the fix (the dual of #66 for blend-trimmed supports)

**Context.** A fillet on a compound body returns a geometrically-correct body
(validate() passes, mesh volume correct) whose ANALYTIC mass is wrong (seed
`11400715918834829014`: 8405 vs true ~6170). The blend face integrates correctly;
the over-count is the fillet-TRIMMED support cylinder face. Before the fillet it
is an untrimmed full-wrap cylinder integrated by the EXACT iso-rectangle path;
after the spring-line imprint it carries a partial trim loop and falls to the
GREEN-SLAB path (`integrate_face_green`, massprops.rs:1007: winding-0 -> `v_base =
v_min`), which mis-integrates that specific post-fillet loop/pcurve configuration.
The mesh is correct on the SAME loops, so the trim geometry is sound; the defect
is in the analytic flux for this configuration. This is the blend-trimmed-support
analog of #66 (curved multi-loop-face mass) and lives in the same integrator the
sphere-band round just extended.

**Questions (implementation-grade).**
1. **Diagnose the green-slab on a fillet-trimmed cylinder.** Given a cylinder
   lateral face trimmed by a spring-line imprint (a straight spring edge along the
   axis plus the blend's end arcs, leaving a non-wrapping partial loop), state the
   correct boundary-flux value and identify the failure mode of the current
   winding-0 `v_base = v_min` slab anchor on this loop: is it the anchor choice,
   the pcurve the fillet attaches (or fails to attach) to the trimmed face, the
   loop orientation the slab reads, or the iso-vs-green path-selection threshold
   itself? Give the precise condition under which a trimmed cylinder face must take
   the green-slab vs a (possibly recoverable) reduced iso-rectangle.
2. **The pcurve question.** `fillet_edge` attaches the blend cylinder SURFACE
   (blend.rs:~898) but the trimmed-support face's loop pcurves after
   `imprint_spring_line` may be incomplete (the engine notes "blend-face pcurves
   ... are follow-ups"). Does the green-slab require an exact (u,v) pcurve for the
   spring edge and the end arcs on the trimmed cylinder, and what is the correct
   pcurve for each (the spring line is an axis-parallel segment; the end arc is the
   blend's conic on the support)? State the construction.
3. **Path-selection robustness.** The iso-rectangle path is EXACT and cheap; the
   green-slab is general but fragile here. Give the rule for when a fillet-trimmed
   cylinder can still be integrated by a (clipped) iso path vs must use the slab,
   so correct cases avoid the fragile path entirely.
4. **The area/volume witness (WRONG-risk gate).** Independent of the fix: the
   per-face analytic-flux vs tessellated-face check that must guard the blend's
   trimmed faces so a self-consistent WRONG cannot slip (the floor fix is a
   whole-body >25% gate; specify the tighter per-face witness that lets the gate be
   relaxed once the integrator is correct). Note the per-face origin-reference
   confound (different references per face; interior walls skipped) and give a
   witness that avoids it (e.g. face-local flux vs face-local tessellation in the
   same reference).
5. **Validation oracle.** Per-face mass==mesh==closed-form on: an analytic
   filleted block edge (the trimmed-plane-support baseline that must not regress),
   a filleted cylinder cap rim, and the compound `KEEL_REPRO`
   `11400715918834829014` body that now declines under the floor gate -- which must
   become a PASS with correct mass. The coaxial-circle and cyl-barrel green-slab
   baselines must NOT move.

**Deliverable.** `docs/research/kernel/71-post-fillet-curved-mass.md`: the
green-slab diagnosis for fillet-trimmed cylinder faces, the trimmed-support pcurve
construction, the iso-vs-slab path-selection rule, the per-face witness with its
gate-relaxation soundness argument, and the per-face oracle.

---

## #3 -- Disjoint curved-union mass: why `mass_properties` collapses on a `combine_disjoint` result, and the fix

**Context.** A union of two AABB-separated bodies short-circuits to
`combine_disjoint` (boolean.rs), producing a valid disconnected (multi-shell,
multi-region) body. On CURVED lumps the analytic `mass_properties` COLLAPSES (seed
`11400715918834829910`, a sphere mirror+union: 238 vs the true 14441) while the
render mesh is correct -- so the per-shell or per-region flux accumulation is
losing one lump. Dossier #60 (curved mass integration) and #61 (disjointness
certificate) cover the single-lump and the certificate; this is the MULTI-lump
accumulation on a disconnected curved body.

**Questions (implementation-grade).**
1. **Per-region flux on a disconnected curved body.** State how the divergence
   boundary-flux must accumulate over a body with multiple disjoint solid regions
   each bounded by its own shell (the sphere-pair case): each region's faces
   contribute to that region's volume; the body volume is the sum. Identify the
   collapse mode in `mass_properties`' face loop (massprops.rs:111): is a second
   shell's faces' sense/region-solidity read wrong (the `(front,back)` solidity
   test at :137-146), or does a pole/seam anchor get shared across lumps, or does
   the interior-wall skip (`(true,true) continue`) wrongly fire across lumps?
2. **The mirror-produced geometry.** `combine_disjoint` after a `mirrored` copy:
   the reflected lump has reflected orientation. Does the reflection invert a
   handedness the mass path assumes (sense vs natural normal, dossier #46), so the
   second lump's flux subtracts instead of adds? Give the orientation rule for a
   reflected curved shell.
3. **Interaction with the floor gate.** The floor fix (commit 3d49fee) declines a
   disjoint union whose analytic mass disagrees with mesh by >25%. State the fix so
   the analytic mass is correct and the gate passes, and confirm the planar
   disjoint-union case (exact today) is untouched.
4. **Validation oracle.** mass==mesh==closed-form (sum of the two lumps) for: two
   disjoint spheres, two disjoint cylinders, a sphere mirror+union
   (`11400715918834829910`), and a planar two-block disjoint union (the baseline);
   the tight `quadric_sphere_op_volume` agreeing where applicable.

**Deliverable.** `docs/research/kernel/72-disjoint-curved-union-mass.md`: the
per-region flux accumulation for disconnected curved bodies, the reflected-shell
orientation rule, the diagnosis of the collapse mode, and the multi-lump oracle.

---

**The one-line ask.** Rounds 0003/0004 hardened the random-primitive curved
booleans; round 0005 turns to the REALISTIC-workflow frontier the new grammar soak
exposed -- the compound-body fillet end-cap topology (#1, the dominant decline
class, an additive surgery extension), the post-fillet curved-face mass (#2, the
WRONG class now floored, the green-slab on a blend-trimmed cylinder), and the
disjoint curved-union mass (#3, the other floored WRONG) -- which together convert
the largest realistic-workflow decline-and-wrong classes from DECLINE into PASS,
never WRONG, and finish making `boolean then fillet the seam` (the single most
common real workflow) a first-class operation.
