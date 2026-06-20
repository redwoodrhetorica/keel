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

**10 of 12 faithful tutorial workflows PASS, 2 DECLINE.** (Was 6/12; #12
`extrude_fillet_shell`, #13 `mirror_solid_union`, #11 `chamfer_even_top_edges`,
and #2 `boolean_then_fillet_cyl_block_union` (the concave boss cap-rim fillet)
are now FIXED -- see the RESOLVED sections at the end.) The remaining 2 declines
are #4 `fillet_all_edges_box_g1` (the 3-edge octant corner mid-sequence) and #5
`fillet_all_edges_box_g2` (no G2 API).

(12 = the 6 classes expanded into the variants the plan lists: boolean+fillet
has 3, fillet-all-edges has G1+G2, loft has rect/circle/tapered, mirror 1,
chamfer single+even, extrude+fillet+shell 1.)

## Per-workflow table

| # | Workflow (`#[test]`) | Class | Result | Decline reason (the `Err`) | Op implicated |
|---|---|---|---|---|---|
| 1 | `boolean_then_fillet_block_block_union` | 1 boolean_then_fillet | **PASS** | -- | `boolean` Union + `fillet_edge` (planar seam) |
| 2 | `boolean_then_fillet_cyl_block_union` | 1 boolean_then_fillet | **PASS** (was DECLINE) | -- | `boolean` Union + concave `fillet_edge` (boss cap-rim, OUTWARD torus) -- see RESOLVED section |
| 3 | `difference_then_concave_fillet` | 1 boolean_then_fillet | **PASS** | -- | `boolean` Difference + concave `fillet_edge` |
| 4 | `fillet_all_edges_box_g1` | 2 fillet_all_edges | DECLINE (PARTIAL FIX) | 2-edge adjacent corners now EXACT (spring-reuse); declines at the first 3-EDGE corner (edge 4 of 12) `Precondition("fillet: three-blend (adjacent) corner, exact cyl-cyl-cyl surgery follow-up")` | `fillet_edge` three-blend corner (sphere-octant cap follow-up) -- see Addendum |
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
   corner blends that recur in tutorials. PARTIALLY FIXED (see Addendum): the
   2-edge adjacent corner (one prior fillet) is now exact via spring-vertex
   reuse; the 3-edge corner (the last edge into a corner, two neighbour cylinder
   caps) still declines pending the sphere-octant cap surgery. Old symptom
   `"fillet: spring misses cap edge (overflow?)"` is resolved; the residual
   decline is `"fillet: three-blend (adjacent) corner..."`.
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

---

## Addendum: adjacent/sequential blend corner interaction (gap class 1 + 4)

Investigation of `fillet_all_edges_box_g1` (#4) and `chamfer_even_top_edges`
(#11) -- the "round/chamfer ALL the edges" workflow, the single most basic
tutorial operation. Both DECLINE because a blend on an edge whose endpoint
corner was already reshaped by a prior adjacent blend hits a surgery the kernel
did not handle. Branch `swap-m39-close` worktree.

### Root cause (fillet)

`fillet_edge` trims each support to its spring line in `imprint_spring_line`
(blend.rs). It finds the spring crossing on the support's CAP edge with
`line_crosses_edge`, which requires the spring to STRADDLE the cap-edge segment
(endpoints on opposite sides). After a prior fillet on a corner-sharing edge,
that cap edge has been SHORTENED so its end IS the prior fillet's tangent vertex
(the two equal-radius springs meet exactly there). The new spring then crosses
the cap edge precisely AT its endpoint, the straddle test rejects it, and the op
declines with `"fillet: spring misses cap edge (overflow?)"` -- at edge 3 of 12.
Concretely: round-2 target edge `(0,40,40)->(0,5,40)`; its shared-corner cap
edge on the top face is the prior fillet's diagonal arc-chord `(0,5,40)->(5,0,40)`;
the new spring (x=5 on the top face) meets it exactly at the endpoint `(5,0,40)`.

### Fix landed: spring-vertex REUSE -> 2-edge adjacent corners are EXACT

`imprint_spring_line` now calls a new `spring_meets_cap_edge`, which returns
`Reuse(vertex)` when the spring meets the cap edge at (within tol of) an existing
endpoint (the adjacent-blend corner) instead of requiring a fresh interior
`Split`. The surgery then REUSES the prior blend's tangent vertex as the new
spring vertex (a zero-width strip corner) rather than declining. With this,
sequential fillets on edges sharing a corner with ONE prior fillet (a "2-edge
corner", a single neighbour cylinder cap) succeed and are ANALYTICALLY EXACT:
independent Monte-Carlo of the two-adjacent-edges body gives 63581.9; the kernel
analytic mass is 63588.5 (mass-MC = +6.6, ~0.01%); mass==mesh within the curved
band. This is the dominant adjacent case and is now correct.

### What still DECLINES: the 3-edge (three-blend) corner

The LAST edge filleted into any corner faces a "3-edge corner": BOTH its
endpoints, or one endpoint, are capped by TWO neighbour fillet CYLINDERS (the
other two edges of that cube corner already filleted). This routes to the cap
`Roof` branch, which was written for a STRAIGHT model ridge between two PLANAR
caps. Against two cylinders the geometry is different:

* the "ridge" between the two neighbour cylinders is their CURVED
  (cylinder-cylinder ellipse) seam, not a straight edge, so the straight-line
  quadratic ridge-crossing is wrong; and
* the correct end arcs are the cylinder-cylinder bisecting-plane ellipses meeting
  at the three-cylinder TRIPLE point.

The exact corner is a SPHERE OCTANT (centre = the inscribed-sphere centre M
where all three blend spines cross; radius r): each blend cylinder, its axis
through M, meets the sphere along a CIRCLE arc, and the corner patch is the
spherical triangle. This is EXACTLY the geometry the test's analytic formula
assumes (`v_corners = 8*(1-pi/6) r^3`) and is already implemented, tested to
`mass` exact to 1e-9, by `fillet_corner_octant` -- but that op consumes a SHARP
trihedral corner (all 3 edges sharp), so it cannot be invoked mid-sequence on a
corner that already has 2 fillets. Completing the corner sphere on the 3rd
`fillet_edge` call (adapting the octant sphere-cap surgery to the
partially-filleted state) is the remaining work.

Two attempts were made and REVERTED as not yet correct: (a) computing the true
cylinder-cylinder bisecting-plane ellipse end arcs, and (b) splitting the curved
ridge at the closed-form triple point and carving with those ellipses. The
GEOMETRY was verified correct (triple point lands at r(1-1/sqrt2) from the cube
corner, ellipse semi-axes (r, r*sqrt2) match the section), but the local
trim/dissolve SURGERY against the neighbour cylinder caps produced non-watertight
corners (mesh overshoot then mass crash; mass 59672 / mesh 61638 vs the true
~61657). The sphere-octant capping (circle arcs, not ellipses) is the cleaner
path and matches the test formula; it is the recommended follow-up.

DECLINE-never-WRONG: rather than emit the topologically-valid-but-geometrically-
WRONG cylinder-cylinder corner (analytic mass ~1.7% low for the whole box; the
soak's curved-WRONG class), the `Roof` branch now DECLINES precisely the
both-caps-are-cylinders sub-case with `"fillet: three-blend (adjacent) corner,
exact cyl-cyl-cyl surgery follow-up"`. So `fillet_all_edges_box_g1` declines at
the first 3-edge corner (round 4 of 12) instead of returning a wrong body, and
stays `#[ignore]`d.

### Chamfer adjacent: a coincident-face boolean gap (same class as #9)

`chamfer_edge` is a boolean Difference of a cutting prism. The second adjacent
top-edge chamfer declines with the UNDERLYING fault
`AssemblyFailed("unmatched coedge: shell-closure invariant violated")` (wrapped
as `"chamfer: cut failed"`): the over-extended cutter's chamfer plane meets the
PRIOR chamfer face at the shared corner, a coincident/grazing-face boolean
configuration the assembler cannot match. This is the SAME coincident-face
boolean frontier as `mirror_solid_union` (#9), not a chamfer-specific bug;
chamfer-all is gated on that boolean robustness work. `chamfer_even_top_edges`
stays `#[ignore]`d.

### Net of this pass

* fillet on edges sharing a corner with ONE prior fillet (2-edge corner):
  FIXED, exact (the spring-reuse change in `imprint_spring_line`).
* fillet 3-edge (three-blend) corner: characterized + cleanly DECLINED
  (sphere-octant capping mid-sequence is the follow-up).
* chamfer adjacent: characterized as the coincident-face boolean gap (#9 class).
* No regressions: full `keel-topo` suite green (22 binaries, incl. the WRONG
  locks `cyl_union_mass_witness`, `post_fillet_mass`, `fillet_surgery_robustness`,
  `three_bucket`, `scan_wrong`, `profile_oracle`); clippy clean.

---

## Addendum 2: 2-edge corner made hollow-compatible; 3-edge octant drafted (4/8 exact), held back

Branch `swap-m39-close` worktree (continued). Goal: fillet ALL 12 edges of a
40 mm cube at r=5 into the exact rounded cuboid (`fillet_all_edges_box_g1`),
WITHOUT regressing `extrude_fillet_shell` (the hollow-of-a-filleted-box test
that PASSES on master).

### The critical constraint, root-caused: spring-REUSE vs the bicylinder corner

The cherry-picked `64cac2d` spring-vertex REUSE made the 2-edge adjacent fillet
corner mass-exact, but it **collapsed the corner to a degenerate 4-valent
point** -- two blend cylinders + the two side planes meeting at ONE vertex, with
NO top-plane patch and NO bicylinder seam. The curved-face `offset_body`
(`tweak.rs`, the `hollow` engine) requires EVERY vertex 3-valent
(`offset_body: non-simple vertex`), so that reuse **broke `extrude_fillet_shell`**
(confirmed: with `64cac2d`, `hollow` declined; that is exactly why Fix A was
dropped).

Master already builds the CORRECT 2-edge corner: a 3-valent CCP vertex with a
**bicylinder (Steinmetz) ellipse seam** between the two perpendicular fillet
cylinders, which `offset_body` shells fine. The defect was only that master's
cap-edge crossing (`line_crosses_edge`, a strict straddle) REJECTS the case
where the new spring meets the prior blend's tangent vertex exactly AT a cap-edge
endpoint -- so a general-position fillet succeeds at one such corner and DECLINES
at its mirror twin (bottom vs top of a box), purely on the sign of the far
endpoint's signed distance. (`fillet_all_edges_box_g1` stopped at edge 2/12 on
master with `"fillet: spring misses cap edge"`.)

### Landed: endpoint-inclusive spring crossing (the right 2-edge corner)

`spring_crosses_cap_edge` (blend.rs), used only by `imprint_spring_line`: the
same crossing as `line_crosses_edge` for an interior straddle, but when the
spring meets the cap edge within a length-scaled tolerance of an ENDPOINT it
returns that endpoint POINT (not a vertex reuse). `split_edge` then makes a fresh
coincident vertex + zero-length stub that Phase 3's `kev`/`kef` dissolve fuses,
leaving the SAME exact 3-valent bicylinder corner the straddle path yields when
it happens to succeed -- the one `offset_body` accepts. This is strictly better
than BOTH master (robust at every 2-edge corner now) and the reuse (the shell
stays watertight + offsettable).

Result: `extrude_fillet_shell` PASSES (hollow mass 19661.75, identical to master).
`fillet_all_edges_box_g1` now completes 4/12 and DECLINES cleanly at the first
3-EDGE corner (no WRONG body). Verified the bicylinder corner is 3-valent at
every 2-edge corner via a valence probe; the shell's 16 vertices stay 3-valent.

### Drafted (then held back): the mid-sequence 3-edge sphere-octant

A full mid-sequence octant surgery was written and DOES run: detect the 3-edge
corner (two prior blend cylinders meet at the corner, across a coincident
pinch cluster that is first fused into one vertex), solve M = the inscribed-
sphere centre (intersection of the three blend spines) and the three triple
points T_i (each cylinder-cylinder-sphere coincidence, on the sphere at r from
M), trim the two supports to the new edge's springs (crossing the prior blends'
spring edges at two of the triples), build the far-end cap normally, carve each
of the three cylinder bands (C0, C1, the new C4) along its great-circle quarter
arc between two triples, and merge the three octant-side slivers + the pinch
into one spherical-triangle face (kef two remnants, kev the third to remove the
old corner vertex). It self-checks (`validate()` + mass==mesh within 2%) and
declines on any imperfection, so it NEVER emits a WRONG body.

It is **mass-EXACT for 4 of the 8 corners** (the four x=0 corners, whose two
prior fillets were full adjacent fillets): mass==mesh to ~0.13%, watertight,
the sphere octant integrates correctly (the Green-slab sphere path, `dv` matching
the closed form). This validates the geometry and the core surgery.

It does NOT yet complete the box. The cube's spatial fillet ORDER interleaves
the corners: the four x=0 corners octant first (exact), then the LATER edges
(8-11) reach the x=40 corners as ordinary 2-edge fillets -- but those edges'
ENDPOINTS now sit MID-ARC on a prior fillet's far-end quarter-circle CAP face,
not at a clean trihedral vertex. The GENERIC cap surgery (`Single`/`Roof` +
`conic_for`) does not bound the new edge's own cylinder band against such a
CURVED cap boundary: the new band is left spanning its full pre-trim extent and
integrates a large wrong volume (probe: edge-8's own cylinder `dv = +6827`
instead of ~-160; whole-body mass drifts to 0.76% then 1.66%). This is a
GENERIC-path limitation (a fillet whose end lands on a prior curved cap), not an
octant-specific bug, and it would block the box even with a perfect octant.

DECLINE-never-WRONG: keeping the octant would let the box REACH edge 8, where
the generic fillet returns a 0.76%-wrong body from `fillet_edge` -- a contract
violation. So the octant was REMOVED for now (it is a clean, characterized
follow-up), and a `both_cyl` DECLINE guard was added to the generic Roof branch
(a 3-edge corner reaching the straight-ridge surgery declines rather than emit
the ~2%-wrong cyl-cyl corner). `fillet_all_edges_box_g1` stays `#[ignore]`d,
completing 4/12 and declining cleanly.

### The follow-up to land the full box (two coupled pieces)

1. Re-land the mid-sequence octant (exact for 4/8 already) AND make its carve of
   the prior cylinders leave their FAR ends in a clean, re-fillet-able state.
2. Generalize the generic fillet end cap (`conic_for` / `split_blend_cap`) to a
   prior fillet's CURVED cap boundary, so an edge whose endpoint lands mid-arc on
   a neighbour's far-end quarter circle still bounds its own band exactly. This
   is the real blocker for edges 8-11 and is independent of the octant.

### Verification (this pass)

- `extrude_fillet_shell` PASS (un-ignored, hollow mass 19661.75 == master).
- `fillet_all_edges_box_g1` declines cleanly at the first 3-edge corner (4/12),
  stays `#[ignore]`d (no WRONG body; partial body mass==mesh 0.08%).
- `cargo test --release`: lib 298 pass; tutorial_workflows 8 pass / 4 ignored;
  WRONG-locks green (`post_fillet_mass`, `fillet_surgery_robustness`,
  `cyl_union_mass_witness`); the 30 blend tests green.
- `cargo clippy --release`: 18 keel-topo warnings = origin baseline (no new).

## chamfer_even_top_edges -- LANDED (the adjacent-chamfer miter seam)

`chamfer_even_top_edges` (chamfer the 4 top edges of a 40 mm cube by 5 mm each,
in sequence) is now un-ignored and PASSES: validate() Ok, mass == mesh, volume
62000 inside the [61500, 63000] bound (4 wedges removed, corners shared). The
tutorial scoreboard moves 8/12 -> 9/12.

### Symptom

The 2nd adjacent chamfer faulted `chamfer: cut failed`. Under `KEEL_BOOL_DEBUG`
the stitched body carried five radial-1 (lone-fin) coedges, all on the top plane
z=40: the perimeter strip the chamfer should have removed -- `(35,5)->(40,5)`,
`(40,5)->(40,40)`, `(40,40)->(35,40)` -- plus the new chamfer face's own top
boundary at x=35, split at a phantom vertex y=28.73:
`(35,5)->(35,28.73)` and `(35,28.73)->(35,40)`. The top face was kept WHOLE
(spanning to x=40) instead of being split at x=35 and dropping the x>35 strip, so
the new chamfer face's x=35 edge had nothing to pair with -> `unmatched coedge:
shell-closure invariant violated`, surfaced as `chamfer: cut failed`.

### Root cause -- `clip_line_to_planar_face` mishandles a line ON a polygon edge

The diagnosis chain (env-gated probes): the chamfer-1 cutter is a triangular
prism along the +x top edge (which chamfer-0 had shortened to length 35, running
y in [5,40]). The cutter chamfer plane meets the top plane along the line x=35,
z=40 -- the miter chord that must split the top face from y=5 to y=40. The SSI
seam was instead emitted as y in [5, 28.73].

`KEEL_CLIP_DEBUG`/`KEEL_CLIP2_DEBUG` localized it to `clip_line_to_planar_face`
(boolean.rs:681), which clips the unbounded plane-plane SSI line to each trimmed
face by convex half-plane intersection. The cutter chamfer face is a rectangle
(its 7.07 mm wide x 80 mm long parallelogram); the SSI line lies EXACTLY ON that
rectangle's long boundary edge (the cut chord runs along the cutter face's own
edge). For that parallel edge `denom = n . d` is zero in exact arithmetic but
rounds to ~2e-14. The old parallel guard was `denom.abs() < 1e-300`, so the
~2e-14 slipped through and `t = -num/denom` (a 0/0 of two ~2e-14 quantities)
produced a GARBAGE finite half-plane bound, t = -28.73, which spuriously
truncated the chord at the phantom y=28.73. Truncating the chord left the top
face unsplit and orphaned the seam coedges.

This is a latent numerical bug, not chamfer-specific: ANY transversal cut whose
SSI chord runs along a cutter face's own boundary edge can trip it (the adjacent
chamfer is the first faithful workflow that does, because the prior chamfer
shortens the next edge so the cut chord coincides with the cutter rectangle's
border). The first chamfer succeeded only because its edge was full-length and
the rounding happened to fall favorably.

### Fix (WRONG-safe, boolean.rs:681)

Scale the parallel test to the geometry: `par_eps = (|n| * |d|).max(1.0) * 1e-9`
(|n| is the edge length, |d| the line's 2D speed -- their product is the natural
magnitude of `denom`). When `|denom| <= par_eps` the line is parallel to the
edge: an on/inside origin (`num >= -|n|*1e-7`) imposes NO half-plane constraint;
a strictly-outside origin returns None. This drops the spurious bound, so the
chord clips to the full y in [5,40], the top face splits at x=35, the x>35 strip
classifies InsideOther and is dropped, and the chamfer face's x=35 edge pairs
with the kept top -> watertight.

No mass path touched (massprops.rs untouched); the fix only corrects which seam
segment is imprinted, which is a pure topology decision.

### Verification (this pass)

- `chamfer_even_top_edges` PASS (un-ignored): mass 62000, mass == mesh, valid.
- `extrude_fillet_shell` still PASS; chamfer unit tests
  (`chamfer_box_edge_removes_wedge`, `asymmetric_chamfer_assembles_to_true_volume`)
  PASS.
- `cargo test --release`: full suite green (lib 298 pass; tutorial_workflows
  9 pass / 3 ignored; all WRONG-lock binaries green: `scan_wrong`,
  `union_wrong_repro`, `cyl_union_mass_witness`, `post_fillet_mass`,
  `fillet_surgery_robustness`, `three_bucket`).
- `three_bucket` oracle (N=300): strict 286 PASS / 14 DECLINE / **WRONG 0**;
  tolerant 75 PASS / **WRONG 0** -- the clip is on the hot boolean path, so this
  confirms no WRONG regression.
- `cargo clippy --release`: 18 keel-topo warnings = baseline (no new).

---

## boolean_then_fillet_cyl_block_union -- the CONCAVE boss cap-rim fillet (LANDED)

Branch `swap-m39-close` worktree. The "boss + fillet" gap (#2) is now un-ignored
and PASSES: a cylinder (r=12, h=30) unioned onto a 60x60x10 plate, then the
boss/plate seam filleted at r=3 -- analytic mass 45201.5, mesh 45170.7 (rel
0.07%, inside [44500, 45700]), `validate()` Ok. **Tutorial scoreboard 9/12 ->
10/12.** A concave fillet ADDS the reentrant-corner fillet, so the union's ~45047
rises ~150 to ~45185.

### The three structural changes (all in blend.rs except the tessellator span)

The prior characterization (the cyl+block section above) named three blockers --
the outward torus, the convexity misread, and the convex-specific surgery. A
fourth, NOT previously noted, turned out to be the dominant one and is recorded
here: **the boolean Union splits the bore circle into TWO ARCS**, so the seam is
NOT a single closed rim like the convex cap-end (a standalone cylinder's rim is
one closed self-loop edge). The four fixes:

1. **Outward torus geometry (`blend_torus_for_edge`).** A GWN probe just OUTSIDE
   the cylinder on the MATERIAL side of the plane distinguishes the concave boss
   (plate overruns the cylinder => MATERIAL => `concave=true`) from the convex
   cap-end (VOID). For concave: `major = r_cyl + radius`, `h_off = hp + sgn*r`
   (tube centre ABOVE the plate, OUTSIDE the cylinder); the `R > 2r` guard is
   convex-only (the outward `major = r_cyl + r > r` always). Verified for the
   test: `major=15, minor=3, origin z=13`, spring_plane (r=15, z=10), spring_cyl
   (r=12, z=13). A `concave` flag is added to `EdgeBlendTorus`.

2. **Convexity read (`edge_is_convex`).** Rewritten as a direct GWN DIHEDRAL
   measurement: sample the generalized winding number on a small circle in the
   plane PERPENDICULAR to the edge tangent at an ON-edge point; the fraction
   inside the solid is the interior dihedral / 2pi (convex < half, concave/reflex
   > half). This needs only the edge tangent and the exact GWN -- NO face
   centroid (an ANNULAR plate-top mis-locates its centroid over the bore, the old
   misread that returned `Some(true)`), NO winding-convention assumption, NO
   single-representative-normal (a cylinder's normal varies around it). For a
   CIRCLE-arc edge the on-edge point and tangent come from the circle geometry
   (the chord midpoint is the axis, off the curve; `curve_point(0.5)` walks the
   FULL circle, not the arc). Boss seam now reads `Some(false)` (18/24 inside =
   270 deg); the L-prism convex/concave planar reads and all 30 blend tests stay
   green.

3. **Bore-rim NORMALIZATION (`tweak.rs::merge_two_arcs_to_closed`).** The Union
   splits the bore at the cylinder seam endpoint (valence-3) and its antipode
   (valence-2, two cocircular arcs). A new Euler op dissolves the valence-2
   antipode, fusing the two arcs into ONE closed circle edge (the circular dual
   of `merge_collinear_edges_at`, which rejects the a==b closed case). It handles
   the 2-fin bore-ring loop collapsing to a single self-loop fin. This restores
   the single-closed-rim topology the cap-rim surgery wants.

4. **Concave surgery (`fillet_cap_rim_concave`) + tessellator span.** Mirrors the
   convex `fillet_cap_rim` with the loop roles inverted: the planar support is an
   ANNULUS whose rim is an INNER hole, so after imprinting spring_plane the bore
   (seam) hole is moved to the BAND (the r<spring disc), the band's spring loop is
   OUTER and the rim INNER (the mekr args invert vs convex), and the OUTWARD torus
   ring's sense is REVERSED (its outward normal faces the cavity). The kef then
   merges the cap band and the cylinder lower band into the torus ring exactly as
   convex. The tessellator's `torus_tube_span` was the last bug: its naive
   min/max of boundary tube-angles picks the WRONG (complementary 270 deg) arc
   when the v-span straddles atan2's branch cut at +-pi (the concave quarter is
   v in [pi, 3pi/2], whose 3pi/2 point reads -pi/2). Replaced with the periodic
   LARGEST-GAP span (mirrors massprops' `projected_rect_bounds`), so the mesh
   trims to the SAME quarter the analytic mass integrates. With the right span the
   convex cap-rim tessellation is unchanged (its span does not wrap).

### DECLINE-never-WRONG

`fillet_cap_rim_concave` ends with a self-check: `validate()` + analytic
`mass_properties` == `mesh_volume` within the 3% curved band. Any imperfection
(a non-normalizable rim, a failed imprint/mekr/kef, a mass!=mesh) returns a
`Precondition` DECLINE -- it never emits a wrong body. The convex cap-end path
(`fillet_cap_rim`, the 3 `fillet_cylinder_*_rim_to_torus` tests, the Pappus mass
check in `blend_faces_integrate_analytically_via_projected_bounds`) is untouched
and green (the concave branch forks at the top on `blend.concave`).

### Verification

- `cargo build --release` green; `cargo clippy --release` introduces NO new
  warnings in the edited regions (all remaining warnings are pre-existing,
  outside the diff hunks).
- `cargo test --release` (keel-topo): full suite green -- 298 lib + the 30 blend
  tests + the 3 `fillet_cylinder_*_rim_to_torus` + every integration binary
  (`scan_wrong`, `union_wrong_repro`, `cyl_union_mass_witness`, `post_fillet_mass`,
  `fillet_surgery_robustness`, `curved_volume_robustness`, `steinmetz`,
  `three_bucket` oracle WRONG=0).
- `tutorial_workflows`: **10 passed, 2 ignored** (`boolean_then_fillet_cyl_block_union`
  un-ignored; `extrude_fillet_shell` still green).

### Honest scope of the concave cap-rim fillet

The surgery is specific to the boss/plate configuration the test exercises: a
single cylinder perpendicular to a planar plate, seam split into exactly two
cocircular arcs by one cylinder seam line. A rim split into MORE than two arcs
(multiple boolean seams crossing the bore), a non-perpendicular plane-cylinder
(cyclide), or a bore that is not a clean circle still decline (the normalization
or the imprint preconditions fail, caught by the self-check). The convex R>2r
guard does not apply to the outward torus, but a radius so large the spring_plane
(r_cyl + r) overruns the plate edge would fail the imprint and decline.
