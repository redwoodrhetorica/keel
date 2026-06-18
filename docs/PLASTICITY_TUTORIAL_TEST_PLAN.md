# Plasticity tutorial corpus to test plan

Derived from a corpus of 113 Plasticity (the CAD app) tutorial transcripts
(~360k words; channels: Nikita Kapustin, Pixel Fondue, Start 3D Modeling, Ryuu /
Blender Bros, Learn Everything About Design, TAKE REFUGE 3D, ...). The corpus is
scratch working data at `C:\Users\mcdon\plasticity-transcripts\` (not committed).
This file is the distilled, original synthesis: which kernel operations real
tutorials exercise, the realistic workflows, and the test scenarios they imply.
Per the inspired-not-copied doctrine, this extracts PRINCIPLES (which ops, which
op-sequences), not any tutorial's content.

## Coverage priority (mentions / distinct videos, of 113)

| op | mentions | videos | kernel entry |
|---|---|---|---|
| fillet | 825 | 74 | `fillet_edge` + variants (blend.rs) |
| boolean (union/sub/intersect) | 683 | 81 | `boolean` (boolean.rs) |
| extrude | 571 | 63 | `extrude` / `extrude_tapered` (construct.rs) |
| chamfer / bevel | 494 | 50 | `chamfer_edge` (chamfer.rs) |
| fill / patch / cap | 430 | 66 | `filled_sheet` / n-sided fill (foreign.rs) |
| loft | 334 | 50 | `loft` (construct.rs) |
| mirror | 268 | 46 | `mirrored` (transform.rs) |
| offset | 179 | 38 | `offset_body` / `offset_face` (tweak.rs) |
| array / pattern | 165 | 29 | app-level (kernel copy+transform) |
| bridge | 109 | 24 | surfacing (research-grade, dossiers) |
| tube / pipe | 93 | 23 | `sweep` of a circle (construct.rs) |
| trim | 92 | 22 | `trim_sheet_by_plane` (sheet.rs) |
| sweep | 79 | 17 | `sweep` (construct.rs) |
| split / slice | 60 | 27 | `section_by_plane` / slice (interrogate.rs) |
| extend | 57 | 25 | `extend_sheet` (sheet.rs, NEW) |
| revolve | 33 | 11 | `revolve` (construct.rs) |
| draft / taper | 31 | 10 | `draft_face` / `taper_face` (tweak.rs) |
| shell / hollow | 28 | 16 | `hollow` (shell.rs) |
| thicken | 27 | 7 | `thicken` (sheet.rs) |
| inset | 8 | 4 | (push/pull family) |
| delete-face / defeature | 3 | 2 | `delete_face` (tweak.rs) |
| untrim | 1 | 1 | `untrim_face` (tweak.rs, NEW) |

Takeaways: test value concentrates in **fillet, boolean, chamfer, fill, loft,
mirror** (each in 46 to 81 of 113 videos). The niche ops we recently built
(`untrim` 1 video, defeature 3) are genuinely rare in practice -- correct to
keep them decline-safe rather than over-invest. `push/pull` scored 0 because
Plasticity exposes it as move/offset-face, not a named "push pull".

## Dominant workflows (the realistic op-sequences to test)

1. **Boolean then fillet the seam** ("union boolean then add fillets to the
   edges"). THE highest-value stressor -- it is the multi-feature compound class
   that has exposed kernel bugs (the multi-cut cap-plane residual, LOG Add 294).
2. **Fillet many/all edges of a solid**, G1 and G2 continuity ("put a fillet on
   this face on this face...", "make that a G2 fillet").
3. **Loft between two edges/profiles** with G0/G1/G2 continuity ("select both
   edges and press L for loft ... G2 continuity").
4. **Mirror a solid across a plane**, then often union the halves ("select the
   object, mirror it over", "mirror plane").
5. **Chamfer edges**, including even / apex chamfer ("run a chamfer ... apex
   chamfer so it's even").
6. **Extrude a sketch then fillet then shell** (the basic part workflow).

## Test scenarios implied (each asserts validate() + mass == mesh + watertight)

- **boolean_then_fillet**: two overlapping primitives (block+block, cyl+block)
  unioned, then `fillet_edge` on the seam edge(s); assert the filleted union has
  mass == mesh and is valid. Difference + fillet of the resulting concave edge
  too (the harder concave-fillet case).
- **fillet_all_edges_box**: a block, fillet ALL 12 edges at a modest radius;
  assert the rounded box volume matches the analytic closed form. G1 and a G2
  variant.
- **loft_two_profiles**: loft between two parallel circle/rect profiles (G1),
  assert a closed solid with mass == mesh; a tapered loft variant.
- **mirror_solid_union**: mirror an asymmetric solid across a plane and union
  the halves; assert the symmetric body is valid, mass == 2x.
- **chamfer_edges**: chamfer a block edge (and an apex/even chamfer); assert
  volume = block minus the chamfer wedge.
- **extrude_fillet_shell**: extrude a planar sketch to a solid, fillet the top
  edges, hollow it; assert a valid thin-walled shell.

These become integration tests alongside `tests/parity_ops.rs` and
`tests/curved_volume_robustness.rs`. Workflows that DECLINE are themselves
findings: they feed the minimize-declines worklist (a tutorial-common workflow
that declines is a high-priority decline class, by definition of real usage).

## Next

Write the scenarios above as `crates/keel-topo/tests/tutorial_workflows.rs`,
starting with `boolean_then_fillet` (the highest-value, kernel-stressing case).
Each PASS is real coverage; each DECLINE is a prioritized gap grounded in actual
tutorial usage.
