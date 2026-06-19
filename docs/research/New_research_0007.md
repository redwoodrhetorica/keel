# New Research Requests 0007 (queued for the research agent)

Drafted 2026-06-19. The compound-operand boolean is the dominant remaining decline
class on the realistic-workflow soak (LOG Add 296 onward; ~6k `extrudeU:AssemblyFailed`
/ `UnassemblableSeam` of the 66k kernel-frontier at 10k). Round 0006 (#73) and the
follow-on instrumentation rounds CHARACTERIZED it precisely and landed the simple
rungs; round 0007 asks for the one engine that clears the bulk: a **per-face planar
overlay** for the imprint, plus the **WRONG-safe watertightness witness** the gate
needs to recover safely. The all-planar case has an EXACT closed-form oracle, so
this is verifiable to machine precision.

**Keel state (grounding, post the assembler-rung work).** Tolerant-f64 non-manifold
radial-edge B-rep; Euler operators; boolean pipeline `seam_curves -> imprint_operand
-> classify_faces -> select_faces -> stitch_by_import -> finalize_imported_assembly`.
CORRECTNESS PROGRAM COMPLETE (Add. 193, WRONG=0) and holding on realistic workflows
(realsoak 10k WRONG=0). Contract = DECLINE-never-WRONG. The active program is turning
DECLINES into PASSES without ever emitting a WRONG.

**What landed since 0006 (so the agent does NOT re-derive it; see the FINDINGS docs).**
- **The mass integrator is CORRECT, proven** (kernel/73-...-FINDINGS sec 10 + tests/cyl_union_mass_witness.rs): the "watertight mass!=mesh" declines are the MESH over-reading a genuinely MALFORMED body (a false `mesh_open_ratio==0` on radial-1 edges), NOT a green-slab bug. A generalized-winding-number (GWN) Monte-Carlo volume adjudicates and matches the analytic mass. Do NOT touch the green-slab; do NOT force mass to the mesh.
- **The dual mass-vs-tessellation gate is LOAD-BEARING for WRONG-prevention** (73-FINDINGS sec 7): relaxing it produced WRONG=1. Any gate recovery needs an independent WRONG-safe witness first.
- **Two assembler rungs landed** (73b, 73c, boolean.rs, WRONG=0, exact all-planar oracle): the **T-junction imprint** (a tool wall cut by two body faces; split the boundary edge at the mid-edge junction; −724 kernel-frontier at 10k) and the **isolated seam-crosses-a-hole** (a seam dipping through one inner hole and back: `mekr`-bridge the hole into the outer loop, then `split_face`). Both are GATED to their simple structure and fall through to a clean decline otherwise.
- **The residual** (73c): the REAL compound cases (e.g. seed 11400715918834826715) stack a hole-crossing AND a sibling-seam T-junction on ONE face. The one-shape-at-a-time rungs cannot compose; each freed imprint just reaches the next seam stage (73b/73c soak: unmatched-coedge down, UnassemblableSeam up). This is why a UNIFIED overlay is needed.

**The wall this round must break.** On a compound operand, a single face arrives at
the imprint carrying an outer loop, zero-or-more inner hole loops, AND several seam
chains (the tool's intersection curves) that may cross each other, the holes, and the
outer loop at shared vertices. The current `imprint_open_chain` / `split_face` resolve
ONE chain against ONE loop at a time and cannot handle the stacked case; the body
comes out with absent/radial-1 fragments and the (correct) shell-closure check
declines. The fix is to compute the FULL planar arrangement of {outer loop, hole
loops, all seam chains} on the face's surface and re-form the correct sub-faces in one
pass. The all-planar instance is the tractable, exactly-verifiable beachhead.

**Dead ends (proven; the agent must NOT propose these).**
- Do NOT "fix the mass" / relax the mass==mesh gate to make malformed bodies pass (proven WRONG, 73-FINDINGS).
- Do NOT extend the one-shape rungs to compose by hand (73c proved they don't; the overlay subsumes them).
- Do NOT touch massprops.rs/blend.rs; the integrator is correct and locked by a test.

**Recommended send order:** #77 (the WRONG-safe witness, it is the prerequisite that
makes any assembler advance safely acceptable) then #76 (the overlay engine).

---

## #76 -- The per-face planar-overlay imprint: arrange {outer loop, hole loops, all seam chains} on one face and re-form the correct sub-faces in one pass

**Context.** Replace the per-chain `imprint_open_chain`/`split_face` (boolean.rs) with
a per-face planar arrangement when a face carries multiple seam chains and/or inner
holes crossed by seams. The landed rungs (73b T-junction, 73c hole-dip) are special
cases the overlay must subsume. The all-planar case (straight loops + straight seams)
has an exact closed-form result volume.

**Questions (implementation-grade).**
1. **The arrangement.** On a face's 2D surface parameter domain, given the outer loop,
   K inner hole loops, and M seam polylines (with their shared crossing vertices,
   already imprinted by Phase 1), compute the planar subdivision into faces. State the
   robust construction (a DCEL/half-edge overlay with the kernel's exact/tolerant
   predicates), how shared vertices (seam-on-loop, seam-on-seam) are handled, and how
   each resulting sub-face's loops are read back.
2. **Re-form the B-rep faces via Euler ops.** Map the arrangement's sub-faces back to
   the radial-edge B-rep: which `split_face`/`mef`/`mekr`/`kemr` sequence re-knits the
   face into the sub-faces, with the V-E-F bookkeeping, generalizing the 73b `mekr`-bridge
   + `split_face` to N crossings and K holes.
3. **Subsumption + gating.** Show the overlay reduces to the landed T-junction and
   hole-dip results on those inputs (non-regression). Where is it genuinely
   intractable (a self-touching seam, a degenerate tangential crossing) -> DECLINE.
4. **Production realization.** How OCCT (`BOPAlgo`/`ShapeUpgrade` face splitting) and
   ACIS imprint multiple intersection curves onto one face with holes; the arrangement/
   overlay primitive they use.
5. **Validation oracle.** The all-planar battery with EXACT closed-form volumes:
   seed 11400715918834826715 (hole + sibling T-junction on one face) ASSEMBLES
   watertight, mass==mesh==ref; plus a face with 2 holes crossed by 2 seams. The 73b/73c
   isolated cases must not regress; the realsoak `extrudeU:AssemblyFailed` /
   `UnassemblableSeam` classes must drop with WRONG=0.

**Deliverable.** `docs/research/kernel/76-per-face-planar-overlay-imprint.md`.

---

## #77 -- The WRONG-safe watertightness witness: fix the false-negative `mesh_open_ratio` and add an independent volume witness for safe gate recovery

**Context.** Two findings make this the prerequisite for any assembler advance: (a)
`mesh_open_ratio` returned 0 (watertight) on a GENUINELY OPEN body (radial-1 edges, a
net-area-vector cancellation false negative -- 73-FINDINGS); the only thing catching
that malformed body was the mass!=mesh gate. (b) The dual mass-vs-tessellation gate is
load-bearing (relaxing it -> WRONG=1), and the GWN Monte-Carlo volume was the
independent integrator that correctly adjudicated mass-vs-mesh.

**Questions (implementation-grade).**
1. **The watertightness net.** Fix `mesh_open_ratio` (or add a topological companion)
   so a radial-1 edge / open sheet is detected directly (a per-edge radial-count /
   boundary-loop check), not via a cancelling area sum. State the exact predicate.
2. **The independent witness.** Specify a WRONG-safe witness for the assemble gate: a
   generalized-winding-number (solid-angle, divergence-free) volume, or a tight
   per-fragment op-volume bound, that is INDEPENDENT of both the analytic mass and the
   tessellation (so agreement among the three is real verification). Its tolerance and
   cost (GWN is slow per long chain -- when to invoke it).
3. **Safe gate recovery.** With the witness in place, the rule to ACCEPT a self-consistent
   result (and drop a now-spurious decline) only when the independent witness confirms,
   so the 9% "watertight mass==mesh declined on a tessellation f64 blow-up" slice
   (73-FINDINGS) recovers WITHOUT admitting a wrong body.
4. **Validation.** The radial-1 open body is now declined by the watertightness net
   directly (not only by mass!=mesh); the f64-blow-up slice recovers to PASS; a battery
   of known-wrong bodies stays declined; realsoak WRONG=0.

**Deliverable.** `docs/research/kernel/77-watertightness-witness-safe-recovery.md`.

---

**The one-line ask.** The assembler is the dominant remaining decline lever, and the
day's rungs proved the real compound cases need a UNIFIED per-face planar overlay (#76)
rather than one-shape patches, gated behind a WRONG-safe watertightness witness (#77)
so the gate can recover safely; together they clear the bulk of the compound-operand
`AssemblyFailed`/`UnassemblableSeam` class from DECLINE into PASS, never WRONG, with an
exact all-planar oracle backing every step.
