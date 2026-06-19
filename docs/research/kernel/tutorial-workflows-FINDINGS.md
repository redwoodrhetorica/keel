# Faithful tutorial-workflow coverage: the REAL pass/decline count

Test file: `crates/keel-topo/tests/tutorial_workflows.rs`
Spec: `docs/PLASTICITY_TUTORIAL_TEST_PLAN.md`
Run: `cd crates/keel-topo && cargo test --release --test tutorial_workflows -- --nocapture`

## Why this is the real metric (not realsoak)

`examples/realsoak.rs` is a FUZZER: it samples a learned grammar with RANDOM
primitive dimensions, so the bulk of its ~65k declines are
geometrically-impossible random primitives, NOT a measure of whether real
tutorial workflows work. This suite is the opposite: the six dominant workflow
CLASSES distilled from 113 Plasticity tutorials, each run with SENSIBLE FIXED
parameters (a ~40 mm part, 3..5 mm fillet radii, 3 mm walls), deterministic,
NOT random. Each workflow/variant is its own `#[test]`, so a decline does not
abort the others, and the decline reason (`Err`) is captured in the failure
message. A PASS is real coverage; a DECLINE is a prioritized incomplete-item
grounded in actual usage.

Oracle per workflow (mirrors `realsoak::audit`): `validate()` Ok
+ analytic `mass_properties().volume` inside an INDEPENDENT analytic bound
+ mass == mesh (all-planar => tight 1% band; any curved face => 3% band for the
documented chordal/tessellation deficit). Watertightness is asserted as
`validate()` Ok (its `check_boundary_chains` rejects open/dangling edges)
COMBINED with mass == mesh, because the crate's `mesh_open_ratio` is
`pub(crate)` and an external integration test cannot call it.

## HEADLINE

**7 of 12 faithful tutorial workflows PASS, 5 DECLINE.** (Was 6/12; #12
`extrude_fillet_shell` is now FIXED -- see the RESOLVED section at the end.)

(12 = the 6 classes expanded into the variants the plan lists: boolean+fillet
has 3, fillet-all-edges has G1+G2, loft has rect/circle/tapered, mirror 1,
chamfer single+even, extrude+fillet+shell 1.)

## Per-workflow table

| # | Workflow (`#[test]`) | Class | Result | Decline reason (the `Err`) | Op implicated |
|---|---|---|---|---|---|
| 1 | `boolean_then_fillet_block_block_union` | 1 boolean_then_fillet | **PASS** | -- | `boolean` Union + `fillet_edge` (planar seam) |
| 2 | `boolean_then_fillet_cyl_block_union` | 1 boolean_then_fillet | DECLINE | `mass_properties()` Err `Precondition("curved face without pcurve bounds")` | `boolean` Union + `fillet_edge` on a CURVED (cyl/plate) junction |
| 3 | `difference_then_concave_fillet` | 1 boolean_then_fillet | **PASS** | -- | `boolean` Difference + concave `fillet_edge` |
| 4 | `fillet_all_edges_box_g1` | 2 fillet_all_edges | DECLINE | `fillet_edge` Err `Precondition("fillet: spring misses cap edge (overflow?)")` on edge 3 of 12 | `fillet_edge` adjacent/sequential corner interaction |
| 5 | `fillet_all_edges_box_g2` | 2 fillet_all_edges | DECLINE | no G2 API: `fillet_edge` is radius-only (circular/G1) | `fillet_edge` (missing G2/curvature-continuous control) |
| 6 | `loft_two_profiles_rect` | 3 loft | **PASS** | -- | `loft` (polygon, planar) |
| 7 | `loft_two_profiles_circle` | 3 loft | **PASS** | -- | `loft_circles` (exact cylinder rung) |
| 8 | `loft_two_profiles_tapered` | 3 loft | **PASS** | -- | `loft` (square frustum) |
| 9 | `mirror_solid_union` | 4 mirror_solid_union | DECLINE | `boolean` Union Err `AssemblyFailed("degenerate or self-inconsistent result (mass != mesh)")` | `boolean` Union of COINCIDENT/COPLANAR faces (the shared mirror plane) |
| 10 | `chamfer_single_edge` | 5 chamfer_edges | **PASS** | -- | `chamfer_edge` (single planar edge) |
| 11 | `chamfer_even_top_edges` | 5 chamfer_edges | DECLINE | `chamfer_edge` Err `Precondition("chamfer: cut failed")` on the 2nd of 4 adjacent top edges | `chamfer_edge` adjacent/compound chamfer |
| 12 | `extrude_fillet_shell` | 6 extrude_fillet_shell | **PASS** (was DECLINE `"offset_body: non-planar face"`) | -- | `hollow` now offsets CURVED fillet faces + nested two-shell `combine_containment` (see RESOLVED section) |

Note on `mirror_solid_union`: `mirrored()` ITSELF SUCCEEDS on the all-planar
extruded L (the test gets past the mirror call); it is the SELF-UNION across the
shared symmetry plane that declines. An earlier construction that built the L via
a boolean union surfaced a SECOND mirror gap -- `mirrored()` rejects a
boolean-built solid with `Precondition("transform: NURBS curves are a
follow-up")`, because a planar-only boolean union still stores a seam curve as a
NURBS that `apply_isometry` declines. The committed test uses a direct
`sweep`/extrude so the reported decline is the cleaner coplanar-union one.

## What the (now 5) declines implicate (the prioritized worklist)

(#12 `extrude_fillet_shell` is RESOLVED -- removed from this list; see the
RESOLVED section at the end. Gap class 2 below is now HALF-resolved: the
`hollow`/`offset_body` curved-face half is fixed; the `mass_properties` curved
pcurve-bounds half (#2) remains.)

Distinct kernel gap classes, ranked by how many tutorial workflows they block:

1. **`fillet_edge` on adjacent/sequential edges (corner interaction)** -- blocks
   the all-12-edges fillet (#4) and is the same family as the multi-fillet
   corner blends that recur in tutorials. Symptom: `"fillet: spring misses cap
   edge (overflow?)"` once a neighbouring edge has already been filleted.
2. **Curved-face followthrough after a boolean/fillet** -- a curved (cylinder)
   face emerging from `boolean`+`fillet_edge` is left WITHOUT pcurve bounds, so
   `mass_properties` declines (#2). [HALF-RESOLVED: the sibling symptom --
   `hollow`/`offset_body` tripping on an upstream fillet face (#6) -- is now
   FIXED via the generalized curved `offset_body_with` + `combine_containment`;
   see the RESOLVED section. The remaining open half is the `mass_properties`
   curved pcurve-bounds gap on a boolean+fillet cyl/plate junction.]
3. **Coincident/coplanar `boolean` Union** -- mirror-then-union across the
   symmetry plane (#9) hits the coplanar-face union gap (`AssemblyFailed
   "degenerate or self-inconsistent result (mass != mesh)"`). This is the same
   coincident-face class the `BoolFault::Coincident` variant names.
4. **Adjacent `chamfer_edge`** -- chamfering a second edge sharing a corner with
   an already-chamfered one fails the cut (#11), mirroring gap class 1 for
   chamfer.
5. **No G2 fillet** (#5) -- the public API has only radius (G1/circular)
   `fillet_edge` and `fillet_edge_conic`; curvature-continuous (G2) fillets are
   unimplemented. (Lowest priority -- G2 is rarer in the corpus than the corner
   and curved-followthrough gaps above.)

## What PASSES (real coverage, confirmed exact)

- `boolean` Union/Difference of PLANAR blocks, then a single `fillet_edge` on a
  resulting seam -- INCLUDING the harder concave (material-adding) fillet.
- `chamfer_edge` on a single planar block edge (exact wedge: 64000 -> 63280).
- `loft` of polygon profiles (prism and square frustum, exact) and the analytic
  `loft_circles` cylinder rung.

These three classes (single-feature boolean+fillet, single chamfer, all loft
variants) are solid. The declines concentrate exactly where tutorials pile
MULTIPLE curved features on one solid: many fillets meeting at corners, fillets
feeding a hollow, and unions on coincident planes.

## RESOLVED: #12 `extrude_fillet_shell` -- hollow on upstream curved fillet faces

**Status: FIXED.** `extrude_fillet_shell` now PASSES (the `#[ignore]` is removed,
so it is a permanent regression guard). **7 of 12 faithful tutorial workflows
now pass.** A new lib regression test `shell::tests::hollow_filleted_box_shells`
guards the same capability at the unit level.

### Diagnosis (the failing face type)

The body the test feeds `hollow` -- a 40x40x30 block with its four TOP edges
filleted at r=4 -- is simpler than feared: **6 planes + 4 cylindrical fillet
faces, NO torus or sphere corner patches.** Every vertex is 3-valent. The 16
vertices fall into three incidence classes:

- `PPP` (4) -- the bottom corners (three planes);
- `CPP` (4) -- a fillet cylinder meeting two side planes;
- `CCP` (8) -- a top corner where two perpendicular fillet cylinders meet the
  top plane.

The four adjacent fillet cylinders meet pairwise at the top corners in a
**bicylinder (Steinmetz) ELLIPSE** edge (semi-axes r and r*sqrt(2)), not a torus.
`offset_body`/`offset_body_with` declined immediately with `"offset_body:
non-planar face"` because it required EVERY face to be planar and re-solved each
corner as a three-PLANE meet.

### The fix (two parts, both DECLINE-never-WRONG)

1. **`tweak.rs` -- generalized `offset_body_with` to curved analytic faces.**
   - `offset_analytic_surface`: a plane offsets by translation; a CONVEX
     cylinder/sphere/torus offsets by its principal radius (cylinder/sphere
     radius +- dist by sense; torus minor/tube radius). Cones and NURBS still
     decline.
   - `solve_three_surfaces`: each corner is re-solved as the intersection of its
     three incident OFFSET surfaces by **Levenberg-Marquardt on the surfaces'
     signed `implicit`s** (gradients = `implicit_gradient`). The damping is
     essential: at the inner top corner the top plane and BOTH fillet cylinders
     share a vertical normal at the pre-offset seed (a mutual tangency), so the
     plain three-surface Jacobian is rank-deficient there -- LM still takes a
     well-defined step and converges to the true (4,4,27)-type corner. The planar
     three-plane meet is its linear special case, so PPP/CPP/CCP corners are all
     handled by one routine. Non-convergence -> decline.
   - `offset_edge_curve`: rebuilds each edge carrier on the offset surfaces while
     PRESERVING the curve's parametric orientation (so the recorded `arc_sweep`
     stays valid -- orientation and endpoint angles are invariant under a
     concentric radial scale): lines through the moved endpoints; cap circles at
     the cylinder's new radius centred on the axis; bicylinder ellipses with
     semi-axes scaled by the radius ratio and re-centred through the moved
     endpoint.

   With this, the inner shell of the filleted box is built EXACTLY: a valid,
   mass==mesh smaller filleted box (r 4->1).

2. **`boolean.rs` -- `combine_containment`, the nested two-shell assembly.**
   The remaining blocker was the boolean DIFFERENCE `outer \ inner`: the two
   nested filleted bodies produce a spurious `UnassemblableSeam` in the
   nested-curved SSI (the inner top plane vs the outer fillet cylinders'
   extended surfaces). But an INWARD whole-body offset leaves `inner` strictly
   inside `outer` with NO real boundary crossing, so the hollow is just the two
   shells stitched verbatim -- the outer outward + the inner REVERSED (its front
   facing the new cavity) -- and `finalize_imported_assembly`'s enclosed-void
   region partition groups them correctly (outer -> infinite, inner -> void).
   This is the curved-shell counterpart of `combine_disjoint`; it bypasses the
   SSI/seam machinery entirely.

   One subtlety against the LOCKED green-slab mass integrator: the global
   coincident-vertex merge in `merge_and_glue_imported` would collapse each
   shell's closed-arc SEAM vertex pair (two distinct vertices at one point) into
   a single CLOSED edge, which the integrator then mis-reads as a spurious
   full-revolution band (winding +-1 on a cylinder) and declines. Since the two
   shells are disjoint and complete, `combine_containment` runs the new
   `finalize_imported_assembly_glued(.., glue=false)` variant, skipping the merge
   and preserving each shell's exact topology. (massprops.rs was NOT touched.)

   `hollow_per_face` tries `combine_containment` first, gated on a containment
   PRECONDITION (the inner must be a positive solid strictly smaller than the
   outer -- this rejects an over-thick offset whose inner shell inverts past the
   medial axis) plus `validate()` + mass==mesh; on any failure it falls back to
   the exact boolean difference, which declines the degenerate case honestly. The
   over-thick decline (`hollow_declines_when_too_thick`), the planar box/prism
   shells, the multi-thickness shell, and the pierced tray all still pass.

### Result

`extrude_fillet_shell`: wall mass 19661.7, mesh 19739.4 (rel 0.4%, inside the
3% curved band), `validate()` Ok. Full lib suite 298 pass; all integration tests
green (`cyl_union_mass_witness`, `post_fillet_mass`, `fillet_surgery_robustness`,
`curved_volume_robustness`, `steinmetz` included); clippy clean. The fix also
generalizes: any solid whose faces are planes/cylinders/spheres/tori with
3-valent corners can now be shelled, not just the box.

### Remaining curved-shell scope (honest limits)

- Corners of valence > 3, or faces the offset does not model (cone, NURBS),
  still decline in `offset_body_with`.
- A fillet whose corner blend is a genuine TORUS patch (rolling-ball blends that
  meet 3+ fillets at a point) is now offsettable surface-wise (torus minor
  radius), but its corner-vertex solve and edge recompute have not been
  exercised by a test; treat as untested until a torus-corner fixture exists.
- Concave shells and per-face thicknesses that change the topology (steps) ride
  the same containment path only while the inner stays a single contained solid.
