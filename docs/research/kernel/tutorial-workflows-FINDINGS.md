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

**8 of 12 faithful tutorial workflows PASS, 4 DECLINE.** (Was 6/12; #12
`extrude_fillet_shell` and #13 `mirror_solid_union` are now FIXED -- see the
RESOLVED sections at the end.)

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

---

## #13 mirror_solid_union: the mirror-orientation normal bug (RESOLVED)

Branch `swap-m39-close`. Took `mirror_solid_union` from DECLINE to PASS and, as a
free consequence, converted a previously-malformed WRONG-locked chain into a
correct watertight result. Tutorial count 7/12 -> **8/12**.

### Symptom

`boolean(L, mirror(L), Union)` of an L-profile extrusion meeting its mirror image
flush on x=0 faulted `AssemblyFailed("degenerate or self-inconsistent result
(mass != mesh)")`. (Note: the task brief expected `"unmatched coedge"`; on this
base, e4f4558, the coincident-face machinery already runs and stitches the body,
so the failure had moved DOWNSTREAM to the final mass==mesh gate. The shared x=0
interface wall was being RETAINED as an interior partition instead of dissolved,
so mass != mesh.)

### Root cause (confirmed by classification dump)

`KEEL_BOOL_DEBUG=1` showed BOTH x=0 interface faces (the original's and the
mirror's) classified `OnOther(Same)`. They must be `OnOther(Opposite)`: two
solids meeting face-to-face have outward normals pointing in OPPOSITE directions.
For Union, `select_faces` keeps A's `OnOther(Same)` and drops B's `OnOther`; with
the sense read as `Same` it kept ONE copy of the interface wall instead of
dropping BOTH. The kept wall is interior to the union => mass != mesh.

The sense came out `Same` because **`Body::mirrored` leaves every reflected
surface frame LEFT-handed.** `transform::apply_isometry` reflects all three frame
axes directly (`x'=M(x), y'=M(y), z'=M(z)`), which is geometrically exact and
keeps pcurves valid, but sends the frame left-handed. The kernel's orientation
authority is `outward = sense * natural` (research file 46), where for an analytic
surface `natural` is the `+frame.z` sense -- and `mass_properties`/`tessellate`
read `frame.z` DIRECTLY, so they stay correct on a mirrored body. But
`boolean::face_outward_normal_at` derived the natural normal from
`local_geometry().normal`, which is the `su x dv` cross product (a PSEUDOVECTOR).
On a left-handed frame `su x dv = -frame.z`, so `face_outward_normal` was INVERTED
for every mirrored face, planar or curved. The boolean classifier was the only
consumer using `su x dv` instead of `frame.z`, so only it saw the wrong sign.

This is the EXACT problem `massprops::frame_handedness` already solves (dossier
72): for a reflected frame `du x dv` points inward, so massprops folds in a
`-1.0` handedness factor on its curved integrators.

### Fix (WRONG-safe, surgical)

`crates/keel-topo/src/boolean.rs`:
- New private free fn `surface_frame_handedness(&Surface3) -> f64` (`+1` proper,
  `-1` reflected), a local copy of massprops' `frame_handedness` (massprops.rs is
  locked, so it could not be shared).
- `face_outward_normal_at` now multiplies the analytic `local_geometry().normal`
  by that handedness before folding `sense`. NURBS faces are unchanged (their
  control net is mapped homogeneously, so `su x dv` already reflects the
  geometry; factor = 1.0). For every non-mirrored (right-handed) body the factor
  is exactly +1.0 -- a provable no-op, so no non-mirror path can change.

After the fix the x=0 faces classify `OnOther(Opposite)`, `select_faces` drops
BOTH copies (the regularized Union rule), the interface dissolves, and the union
is one watertight solid: mass == mesh == 20000.0 exactly.

### Free consequence: a WRONG-locked chain became correct

`tests/union_wrong_repro.rs` reproduces `block -> hollow -> mirror+union ->
fillet -> union(cylinder)` (soak seed 11400715918834827198). It was pinned to
DECLINE because the final union produced mass 10009 vs mesh 5242 (a 48% mesh
collapse). That malformation originated at the chain's OWN mirror+union (step 3),
i.e. THIS bug. With the fix:
- step-3 mirror-union: mass == mesh == 5456.89, valid;
- final union: **Ok, mass 10009.65 == mesh 9995.07 (0.15% rel, valid).**

So the body the regression "pinned as must-decline" was malformed only because of
the mirror bug; it is now a correct solid. The DECLINE-never-WRONG contract test
(`..._declines_not_wrong`, which accepts EITHER a decline OR mass==mesh) still
passes. The outcome-PIN test (`..._is_declined`) was updated to assert the now-
correct Ok (watertight + mass==mesh). **No gate was loosened or removed**; the
curved gate (incl. the recent `mesh_volume` check) is untouched and the body
earns the pass by being correct. This is strictly better than the old decline.

### chamfer_even_top_edges: NOT the same class -- left declining (characterized)

The brief expected the adjacent-chamfer case to be unblocked by the same fix.
It is NOT: it is a different and deeper gap.

- The 2nd of 4 top-edge chamfers fails `Precondition("chamfer: cut failed")`,
  whose underlying `BoolFault` IS `unmatched coedge: shell-closure invariant
  violated` (radial-1 dangling edges), reproduced with `KEEL_BOOL_DEBUG=1`.
- But the classification dump shows ALL fragments `InsideOther`/`OutsideOther`,
  **zero `OnOther`** -- so it is NOT a coincident-face classification problem.
  The mirror fix does not touch it (confirmed: still declines after the fix).
- It is a general-position TRANSVERSAL Difference whose cutting prism interacts
  with the PRIOR chamfer's oblique face at the shared top corner: the
  import-and-glue stitcher leaves unpaired seam coedges (radial 1) along the
  y=5 setback line and the z=40 top where the new cut abuts the prior bevel.
  The dangling edges are at the partial-overlap seam the seam-subdivision does
  not split symmetrically on both operands.
- This is the adjacent-feature seam-pairing class (a chamfer/fillet cutting
  across a previously-cut non-axis-aligned face), squarely the "import-glue seam
  on cut-over-prior-feature" assembly gap -- out of scope for the coincident-face
  union task. It STAYS `#[ignore]`d. (Sibling-agent root-cause attribution to the
  coincident-face boolean does not hold on this base.)

### Verification

- `cargo build --release` green; `cargo clippy --release` clean (no new warnings;
  the 23 pre-existing style warnings in keel-geom/boolean.rs are unrelated and at
  lines far from the edit).
- `mirror_solid_union` PASS (un-ignored). `cargo test --release`: 298 lib tests +
  all integration suites green, including the named WRONG-locks `scan_wrong`,
  `union_wrong_repro` (both tests), `cyl_union_mass_witness`, `three_bucket`,
  `post_fillet_mass`.
- Tutorial workflows: **8 passed, 4 ignored** (8/12).

---

## `boolean_then_fillet_cyl_block_union` (boss + fillet): root cause is the CONCAVE plane-cylinder fillet, not pcurve bounds

### TL;DR

The "boss + fillet" gap (#5) was diagnosed as a missing-pcurve-bounds problem
("curved face without pcurve bounds" out of `mass_properties`). It is NOT. The
real cause is that the cap-rim torus fillet (`fillet_cap_rim` /
`blend_torus_for_edge` in `crates/keel-topo/src/blend.rs`) **only implements the
CONVEX cap-end case** (rounding a cylinder's flat-top rim, an inward torus). The
boss-on-plate seam is the **CONCAVE** case (an outward torus filling the
reentrant corner where a cylinder rises out of a plate), which needs the
mirror-image trim-and-stitch surgery. Unguarded, the convex code built an
inward torus (`major = r_cyl - radius`) and stapled it to the **annular plate-top
face**, leaving a body that passed `validate()` but DECLINED in
`mass_properties` with the misleading pcurve message. Implementing the concave
case correctly is a milestone-scale change (the imprint, the mekr loop roles,
and the ring-face selection all assume the convex disc-cap topology); it stays a
DECLINE. The landed change makes that decline **honest and early** (at
`fillet_edge`) instead of a broken body + misleading late mass error.

### The investigation chain (what the instrumentation showed)

1. `mass_properties` fails on the post-fillet body at curved face `Key(0g0)`:
   `KEEL_MASS_DEBUG` -> `mass curved Key(0g0) stale true ... lo (0,0) hi (tau,60)
   vert_v (0, 1.5708)`. The pcurve box `v=60` (a PLATE length, not an angle)
   exceeds the vertex extent `v=pi/2`, so the bounds path flags STALE and falls
   to `projected_rect_bounds`, which returns `None`; the surface is a `Torus`,
   not in `integrate_face_green`'s {cylinder, sphere, cone}, so it returns
   `Precondition("curved face without pcurve bounds")` (massprops.rs:918). The
   guard is correct -- the face genuinely has no readable UV rectangle.
2. `KEEL_PRB_DEBUG` (temporary dump in `projected_rect_bounds`) showed the failing
   torus face's loop is NOT a clean two-spring band: it carried the **60x60 plate
   pcurves** (`v=0..60`), cylinder pcurves (`v=10`, `v=30`), my two spring
   circles, AND a `curve=none` **mekr seam bridge** whose chord projects to
   varying u AND v (non-iso). So the torus surface was attached to the
   plate-top face.
3. `blend_torus_for_edge` computes `major = r_cyl - radius = 9` and the spring
   on the plane at `r = major = 9`. For the boss, `r=9 < r_cyl=12` is INSIDE the
   boss footprint -- not on the plate-top annulus (`r>12`) at all. `imprint_ring`
   then split the plate-top into a disc `r<15`... wrong region. `CAPRIM` dump:
   `convex=Some(true)` -- the generic `edge_is_convex` MISREADS the corner
   (it takes the in-plane direction from the plane face's outer-loop centroid,
   which for the boss annulus sits over the bore and flips inward), so even with
   a correct concave torus the convex surgery ran.

### Why the convex path is right and the boss is the concave dual

The PASSING unit test `blend::tests::fillet_cylinder_cap_rim_to_torus` fillets
the top rim of a STANDALONE cylinder (a disc cap bounded BY the cylinder): the
ball rolls in the material inside the cylinder, `major = r_cyl - radius`, and the
resulting torus ring integrates EXACTLY (Pappus, `< 1e-9`). That is the convex
cap-end. The boss is its dual: the planar support is a **plate that overruns the
cylinder** and is pierced by the cylinder footprint; the ball rolls in the void
OUTSIDE the cylinder; the correct torus is OUTWARD (`major = r_cyl + radius`,
tube centre offset into the void, spring on the plane at `r_cyl + radius`). The
geometry for the outward torus was derived and verified
(`major=15, minor=3, origin z=13`, springs at `(r=12,z=13)` and `(r=15,z=10)`),
but the surgery that trims and stitches it (imprint, mekr, kef, ring selection)
is convex-specific and produced a mis-attached face.

### What landed (WRONG-safe, low-risk)

`blend_torus_for_edge` now DETECTS the concave case and DECLINES with the true
reason `"blend_torus: concave plane-cylinder (boss) rim fillet unimplemented"`.
The detector is a generalized-winding-number probe just OUTSIDE the cylinder, on
the MATERIAL side of the plane (opposite the plane's outward normal): MATERIAL
there means the plane overruns the cylinder (boss => concave); VOID there means
the plane ends at the cylinder (cap-end => convex). This correctly distinguishes
the two without relying on `edge_is_convex` (which misreads annular supports).

- The convex cap-end path is untouched: all 30 `blend` lib tests pass, including
  the three `fillet_cylinder_*_rim_to_torus` cases and the Pappus mass check.
- The boss fillet now declines cleanly at `fillet_edge` (no broken body, no
  misleading late `mass_properties` error). `boolean_then_fillet_cyl_block_union`
  stays `#[ignore]`d (its annotation updated to the true reason).

### The follow-up to actually pass this test (the concave cap-rim milestone)

To un-ignore `boolean_then_fillet_cyl_block_union`, implement the concave
plane-cylinder torus fillet end-to-end:
1. `blend_torus_for_edge`: build the OUTWARD torus for the concave branch
   (`major = r_cyl + radius`, `h_off = hp + sgn*radius`; the spring on the plane
   at `r_cyl + radius`, the spring on the cylinder at `hp + sgn*radius`; skip the
   `R > 2r` guard, which is only needed for the inward torus). (Derived + verified
   in this investigation.)
2. The trim-and-stitch surgery for the concave topology: the planar support is an
   annulus where the seam (rim) is an INNER hole, not the outer loop, and the
   spring circle (`r_cyl + radius`) bounds a BAND with the rim (not a disc inside
   the spring). `imprint_ring`, the `rim_loop`/`ring_loop` roles fed to `mekr`
   (the spring loop is OUTER, the rim loop INNER -- inverted from the convex
   case), and the post-`kef` `ring`-face selection all need the mirror-image
   logic.
3. Then attach the torus ring's pcurve bounds (iso-v springs) so the green-slab
   integrator reads `u in [0, tau], v in [v0, v0+pi/2]` -- the small, originally
   anticipated piece, which only matters once (1) and (2) produce a clean
   two-spring torus band.

### Verification (this change)

- `cargo build --release` green; `cargo clippy --release` introduces **no new
  warnings** (origin baseline = 18 keel-topo warnings, unchanged; none in the
  edited region).
- `cargo test --release` (keel-topo): 298 lib tests + every integration suite
  green, including WRONG-locks `scan_wrong`, `union_wrong_repro`,
  `cyl_union_mass_witness`, `three_bucket`, `post_fillet_mass`, and
  `fillet_surgery_robustness`.
- Tutorial workflows: still **8 passed, 4 ignored** (no regression; the boss
  fillet's decline is now honest).
