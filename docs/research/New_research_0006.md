# New Research Requests 0006 (queued for the research agent)

Drafted 2026-06-18. Implementation-grade dossier requests for the **compound-operand
robustness frontier** -- the dominant *remaining* decline class once the
realistic-workflow soak's mass-WRONG classes are solved (round 0005, LOG Add 296,
WRONG=0 at 10k). The prior curved-boolean rounds (dossiers 47, 58-69) hardened
booleans of PRIMITIVE PAIRS (cyl/sphere/cone against cyl/sphere/cone). The
realsoak census shows the live frontier has moved: it is now booleans and cuts of
a simple tool into an ALREADY-COMPOUND, multi-feature body (post-extrude,
post-boolean, post-fillet) -- the second-most-common real CAD motion after fillet.
Round 0006 asks for the algorithm to make those assemble, or decline precisely.

**Keel state (grounding, post-0005).** Tolerant-f64 non-manifold radial-edge
B-rep; Euler operators; arenas + generational keys. Analytic surfaces
plane/cylinder/cone/sphere/torus + NURBS. Boolean pipeline: `seam_curves
(analytic SSI + recovered-NURBS) -> imprint_operand(a/b) -> classify_faces
(generalized winding, 0.25 band) -> select_faces (Requicha) -> stitch_by_import ->
finalize_imported_assembly (Weiler region walk)`; an AABB-separated pair
short-circuits (`combine_disjoint`). The CORRECTNESS PROGRAM IS COMPLETE (Add. 193:
2.24B fuzz, WRONG=0) and now holds on REALISTIC long-chain workflows too (Add. 296:
realsoak 10k, WRONG=0, the 18 mass-violations fixed). Contract = DECLINE-never-WRONG.
The active program is turning DECLINES into PASSES without ever emitting a WRONG.

**What landed since 0005 (so the agent does NOT re-derive it).**
- The two realsoak mass-WRONG classes are SOLVED (Add. 296, branch fillet-mass-frontier
  -> master): post-fillet over-count was a B-rep SURGERY bug (split_face stranded
  inner-hole loops; `redistribute_inner_loops`), NOT the green-slab integrator;
  disjoint mirror collapse was reflected-frame handedness (`frame_handedness` folded
  into the curved mass dispatch). −296 fillet declines now PASS; WRONG=0 at 10k.
- The realsoak fuzzer (`examples/realsoak.rs`) is the live oracle: grammar-driven
  long chains on a growing solid, three-bucket PASS/DECLINE/WRONG. Its 10k census
  (seed 1) is the worklist this round draws from.
- DECLINE-never-WRONG floor gates (curved mass==mesh self-check in fillet's return
  and the disjoint-Union arm) stay as the safety net.

**The wall this round must break (the 10k realsoak census, seed 1).** After the
mass fixes, the largest KERNEL-FRONTIER declines (66,736 across 209 classes) are,
in order, all the same family -- a tool boolean'd or cut INTO a compound body:
- `sketch->extrude : extrudeU : AssemblyFailed` (~2,670) and `extrude->extrude`,
  `fillet->extrude`, `boolean->extrude` extrudeU:AssemblyFailed (~2,305 / 1,990 /
  1,668): a union of a fresh tool into a multi-feature body fails to ASSEMBLE.
- `extrude->boolean : booleanI : AssemblyFailed` (~1,630) and `fillet->extrude :
  extrudeU : UnassemblableSeam` (~1,580): the SEAM stage itself fails on a compound
  operand.
- `fillet->chamfer : chamfer : Precondition (cut failed)` (~1,974): chamfer is a
  `boolean(self, prism, Difference)` (chamfer.rs:103); on a compound body that
  Difference fails -- the SAME assembler frontier through a THIN/SLIVER cutter.
- `fillet->shell : shell : Precondition` (~1,528): hollow/offset of a multi-feature
  body -- the adjacent OFFSET frontier.
- `no-edge` (~2,345, fuzzer artifact: realsoak offers only straight 2-face edges)
  and `fillet:Precondition` (~1,526, the fragmented end-cap tail) are NOT in scope.

Distinct from rounds 58-69: those addressed primitive-pair SSI and curved-rim
mass/tessellation. This is a SIMPLE tool (block/cyl peg) whose SSI is easy, against
a body that ALREADY carries planar faces + curved blend faces + prior boolean
seams -- the failure is in the SEAM completeness and the ASSEMBLY/stitch over that
compound face set, and in the THIN-CUTTER Difference. Repro seeds above (replay:
`KEEL_REPRO=<seed> ./target/release/examples/realsoak.exe`).

**Dead ends (proven; the agent must NOT propose these).**
- Do NOT re-derive the primitive-pair curved SSI (dossiers 58-69) -- that layer
  works; the tool's SSI is the easy part here. The failure is downstream
  (seam-on-compound / assembly / thin-cutter), on a tool whose own surfaces are
  simple planes/cylinders.
- The DECLINE-never-WRONG floor gates and the curved post-condition gate stay; a
  fix that makes a compound boolean ASSEMBLE must still pass mass==mesh==watertight
  or self-decline. Re-proposing a guard is not progress.
- Containment-DROP of phantom seams regressed the soak historically (Add. 260/261);
  the Add-294 disk-plane fix was a narrow completion, not a seam drop -- do not
  conflate or revisit a global seam-drop.

**Recommended send order:** #73 (compound-operand assembly -- the dominant
AssemblyFailed/UnassemblableSeam class), then #74 (the thin/sliver-cutter
Difference -- chamfer-cut and its kin), then #75 (compound-body shell/offset).

---

## #73 -- Compound-operand boolean assembly: why a simple tool unioned/cut into a multi-feature body fails to seam or assemble, and how to make it (or decline precisely)

**Context.** The realistic motion `boolean/extrude into a body that already has
planar faces + curved blend faces + prior boolean seams` is the largest decline
class (`extrudeU:AssemblyFailed`, `booleanI:AssemblyFailed`, `extrudeU:UnassemblableSeam`,
~8,000 combined). The tool is a simple block/cylinder peg (its own SSI is trivial);
the failure is in `seam_curves`/`imprint_operand` completing the seam across the
compound body's many faces, and in `finalize_imported_assembly` (the Weiler region
walk) stitching the result. `assemble_boolean`'s post-condition then declines
(AssemblyFailed) rather than emit a non-watertight or wrong body.

**Questions (implementation-grade).**
1. **Where it breaks.** For a tool unioned into a multi-feature body, classify the
   failure: (a) the seam is incomplete/inconsistent where the tool crosses a prior
   blend face or a prior boolean seam (UnassemblableSeam), vs (b) the seam is fine
   but the region walk / stitch fails on the compound face set (AssemblyFailed).
   Give the diagnostic that separates these on a given case, and the dominant mode.
2. **Seam-on-compound completeness.** When the tool's face crosses an EXISTING
   seam edge or a curved blend face of the target, what is the rule to imprint a
   consistent, watertight seam loop (the tool-vs-blend SSI is plane/cyl-vs-cyl,
   already in the SSI matrix; the issue is junctioning the new seam with the
   target's existing edges at crossing vertices)?
3. **Assembly over a compound face set.** State the robustness fix for
   `finalize_imported_assembly` / the Weiler region walk when the operand carries
   many faces of mixed type: the classification/selection consistency rule, and the
   precondition under which the result is genuinely unassemblable -> DECLINE.
4. **Production-kernel realization.** How do Parasolid (`PK_BODY_boolean` on
   general bodies), ACIS, and OpenCascade (`BRepAlgoAPI_BooleanOperation` on
   compound shapes) keep the seam and assembly robust when a tool is combined with
   an arbitrarily complex target; what are their documented failure/decline modes.
5. **Validation oracle.** A battery from the realsoak repro seeds (union/diff/intersect
   a peg into a block-then-filleted body, and into a two-boolean body): assert
   validate() + mass==mesh + watertight on the representable cases, and a precise,
   predicted DECLINE (never a throw or wrong body) on the genuinely-unassemblable
   ones. The realsoak `extrudeU:AssemblyFailed` seeds must move from DECLINE to PASS.

**Deliverable.** `docs/research/kernel/73-compound-operand-boolean-assembly.md`.

---

## #74 -- The thin/sliver-cutter Difference: robust `boolean(body, prism, Difference)` for chamfer and degenerate cutters

**Context.** `chamfer_edge` cuts by `boolean(self, &cutter, Difference)` with a
thin triangular prism cutter (chamfer.rs:99-103); on a compound body this Difference
fails (`chamfer: cut failed`, ~1,974). A thin/sliver cutter is a stress case for the
assembler (near-tangent faces, slivers). This is both a chamfer-robustness fix and a
general thin-cutter-Difference fix; it may also motivate a LOCAL chamfer surgery
(imprint + trim, like the fillet engine) that avoids a full boolean entirely.

**Questions (implementation-grade).**
1. **Thin-cutter Difference failure mode.** Why a thin triangular prism Difference
   fails on a compound body: sliver faces, near-tangent SSI, the cutter apex sizing
   (chamfer.rs:84-90 `big`/`margin`). Is it the cutter geometry (fixable by sizing)
   or the assembler on slivers?
2. **Local chamfer surgery (the alternative).** Specify a local imprint+trim chamfer
   (the chamfer analog of the fillet engine's spring-line imprint + dissolve) that
   replaces the full-boolean cut, avoiding the assembler entirely: the setback-plane
   imprint on each support, the Euler dissolve, the end conditions. Cost/benefit vs
   hardening the boolean Difference.
3. **Decline boundary.** When a chamfer setback genuinely overflows the local
   geometry (cf. the blend-overflow dossiers 41/56), DECLINE precisely.
4. **Validation oracle.** The realsoak `chamfer: cut failed` repro seeds move to
   PASS (mass == block-minus-wedge closed form) or a predicted DECLINE; the
   clean-primitive chamfer baselines do not regress.

**Deliverable.** `docs/research/kernel/74-thin-cutter-difference-chamfer.md`.

---

## #75 -- Compound-body shell / offset: robust hollow of a multi-feature body

**Context.** `hollow` (shell to a wall thickness) declines on compound bodies
(`shell:Precondition`, ~1,528) -- the offset frontier (dossier 50 territory) on a
body with curved blend faces and prior seams. Offsetting a multi-feature body
requires offsetting each face and re-intersecting/healing the offset face set.

**Questions (implementation-grade).**
1. **Compound-face offset.** The rule to offset a body whose faces include planes,
   cylinders (blend faces), and prior-boolean faces: per-face offset surface, the
   re-intersection of adjacent offset faces (including the curved blends), and the
   self-intersection / collapse detection (offset past the medial axis).
2. **Where it must decline.** The precondition under which the inner offset
   self-intersects or a face collapses (thickness exceeds the local feature) ->
   DECLINE precisely (the offset analog of blend overflow).
3. **Production realization + oracle.** How Parasolid/ACIS/OCCT shell a
   multi-feature body; a validation battery (extrude->fillet->shell, the realsoak
   `fillet->shell` seeds) asserting a valid thin-walled solid (mass == outer minus
   inner) or a predicted DECLINE.

**Deliverable.** `docs/research/kernel/75-compound-body-shell-offset.md`.

---

**The one-line ask.** Rounds 58-69 hardened primitive-pair curved booleans and
0005 fixed the realistic mass-WRONGs; round 0006 takes on the COMPOUND-OPERAND
frontier the realsoak census now shows is dominant -- a simple tool unioned/cut/
shelled into an already-multi-feature body (#73 assembly, #74 thin-cutter Difference
/ chamfer, #75 shell/offset) -- which together convert the largest remaining
realistic-workflow decline classes (~11,000+) from DECLINE into PASS, never WRONG.
