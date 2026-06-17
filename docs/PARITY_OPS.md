# Keel kernel operations — Plasticity-parity catalog & test checklist

Every kernel-level modeling operation Keel exposes, the Plasticity command it
maps to, its entry point (file:line at the time of writing, on `master` after the
compound-robustness merge), and a per-op test-status column to fill as we build
out coverage. The goal: **test each operation** systematically toward parity.

Status legend: **HAVE** = implemented; **GAP** = missing (parity to-do).
Test legend: TODO = no dedicated test yet; PARTIAL = exercised indirectly (soak /
boolean tests); DONE = has a focused guard test.

> Framing: most Plasticity commands are *app-level* (sketch interaction, gizmos,
> materials, display, instances, arrays) and need no kernel op — see the bottom
> section. This catalog is the *kernel* surface that a Plasticity-style app
> (fieldforge) drives.

---

## 1. Primitives  (`construct.rs`)

| Op | Entry point | Test |
|---|---|---|
| Block / Box | `Body::block` (construct.rs:141) | TODO |
| Cylinder | `Body::cylinder` (construct.rs:1405) | TODO |
| Cone | `Body::cone` (construct.rs:1596) | TODO |
| Sphere | `Body::sphere` (construct.rs:1663) | TODO |
| Embedded wire | `Body::wire` (construct.rs:63) | TODO |

## 2. Booleans & imprint  (`boolean.rs`, `imprint.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Boolean (∪ ∩ −) | Boolean (Q) | `boolean::boolean` | PARTIAL (soak + curved_volume_robustness) |
| Multi-tool boolean | Boolean | `boolean::boolean_multi` (5257) | TODO |
| Cut by sheet/plane | Cut (C) | via `boolean` Difference / `partition_by_sheet` | TODO |
| Imprint body↔body | Imprint Body Body | `boolean::imprint` (5334), `Body::imprint_body` (6582) | TODO |
| Knit / sew sheets → solid | Join Sheets (J) | `boolean::knit` (3629) | DONE (parity_ops: explode↔knit round-trip) |
| Explode / unjoin → sheets | Unjoin Shells (Alt+J) | `boolean::explode` | DONE (parity_ops round-trip) |
| Imprint closed curve | Imprint Curve Body | `Body::imprint_closed_curve` (imprint.rs:35) | TODO |
| Imprint open curve | Imprint Curve Body | `Body::imprint_open_curve` (imprint.rs:108) | TODO |
| Imprint closed polyline | Imprint Curve Body | `Body::imprint_closed_polyline` (imprint.rs:180) | TODO |
| Imprint open polyline | Imprint Curve Body | `Body::imprint_open_polyline` (imprint.rs:275) | TODO |
| Imprint crossing curve | Imprint Curve Body | `Body::imprint_closed_curve_crossing` (imprint.rs:554) | TODO |
| Glue edges | (internal) | `Body::glue_edges` (ops.rs:352) | TODO |

## 3. Sweep / extrude / revolve / loft  (`construct.rs`, `foreign.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Extrude (tapered) | Extrude (E) | `Body::extrude_tapered` (construct.rs:685) | TODO |
| Sweep (profile, dir) | Extrude/Sweep | `Body::sweep` (construct.rs:88) | TODO |
| Sweep along path | Sweep (Shift+P) | `Body::sweep_along_path` (construct.rs:551) | TODO |
| Sweep w/ rotation-min frame (Pipe) | Pipe (P) | `Body::sweep_profile_rmf` (construct.rs:580) | TODO |
| Revolve | Revolve | `Body::revolve` (construct.rs:724) | TODO |
| Revolve closed | Revolve | `Body::revolve_closed` (construct.rs:950) | TODO |
| Revolve partial | Revolve | `Body::revolve_partial` (construct.rs:1166) | TODO |
| Loft (2 sections) | Loft (L) | `Body::loft` (construct.rs:330) | TODO |
| Loft (N sections) | Loft | `Body::loft_sections` (construct.rs:342) | TODO |
| Loft circles | Loft | `Body::loft_circles` (construct.rs:1497) | TODO |
| Loft w/ guides (Gordon) | Loft | `Body::lofted_sheet_with_guides` (foreign.rs:68) | TODO |

## 4. Fillet / blend  (`blend.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Constant-radius fillet | Fillet (B) | `Body::fillet_edge` (blend.rs:662) | TODO |
| Variable-radius fillet | Fillet | `Body::fillet_edge_variable` (blend.rs:1053) | TODO |
| G2 fillet | Fillet | `Body::fillet_edge_g2` (blend.rs:1154) | TODO |
| Conic fillet | Fillet | `Body::fillet_edge_conic` (blend.rs:1313) | TODO |
| Hold-line fillet | Fillet | `Body::fillet_edge_hold_line` (blend.rs:1468) | TODO |
| Cliff fillet | Fillet | `Body::fillet_edge_cliff` (blend.rs:1823) | TODO |
| Partial-extent fillet | Fillet | `Body::fillet_edge_partial` (blend.rs:2064) | TODO |
| Edge-chain fillet | Fillet | `Body::fillet_edge_chain` (blend.rs:2286) | TODO |
| Notch fillet | Fillet | `Body::fillet_edge_notch` (blend.rs:2521) | TODO |
| Cap-rim fillet | Fillet | `Body::fillet_cap_rim` (blend.rs:530) | TODO |
| Corner octant | Fillet (corner) | `Body::fillet_corner_octant` (blend.rs:3303) | TODO |
| Corner setback | Fillet (corner) | `Body::fillet_corner_setback` (blend.rs:3650) | TODO |
| Face-face blend | Fillet | `Body::blend_face_face` (blend.rs:1626) | TODO |
| (low-level blend prims) | — | `blend_cylinder/torus/cone_for_edge` | TODO |

## 5. Chamfer  (`chamfer.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Chamfer (symmetric) | Fillet/Chamfer (B) | `Body::chamfer_edge` (chamfer.rs:21) | TODO |
| Chamfer (asymmetric) | Chamfer | `Body::chamfer_edge_asymmetric` (chamfer.rs:33) | TODO |

## 6. Shell / hollow / thicken  (`shell.rs`, `sheet.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Hollow (uniform) | Hollow | `Body::hollow` (shell.rs:33) | TODO |
| Hollow (per-face thickness) | Hollow | `Body::hollow_per_face` (shell.rs:48) | TODO |
| Hollow (pierce faces) | Hollow | `Body::hollow_pierce` (shell.rs:73) | TODO |
| Thicken sheet → solid | Thicken | `Body::thicken` (sheet.rs:212) | TODO |

## 7. Local / face ops  (`tweak.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Draft face | Draft Face | `Body::draft_face` (tweak.rs:329) | TODO |
| Offset face | Offset Face | `Body::offset_face` (tweak.rs:373) | TODO |
| Taper face | (draft variant) | `Body::taper_face` (tweak.rs:483) | TODO |
| Offset whole body | Offset | `Body::offset_body` (tweak.rs:507) | TODO |
| Delete face | Delete Face | `Body::delete_face` (tweak.rs:594) | TODO |

## 8. Transform  (`transform.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| General transform (move/rotate) | Move (G) / Rotate (R) | `Body::transformed` (transform.rs:33) | TODO |
| Scale (uniform) | Scale (S) | `Body::scaled` (transform.rs:67) | TODO |
| Scale (non-uniform) | Scale | `Body::scaled_nonuniform` (transform.rs:144) | TODO |
| Mirror | Mirror (Alt+X) | `Body::mirrored` (transform.rs:45) | TODO |

## 9. Sheet / surface creation  (`foreign.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Patch / fill boundary | Patch | `Body::filled_sheet` (foreign.rs:53) | TODO |
| Foreign-evaluator sheet | (foreign surfaces) | `Body::foreign_sheet` (foreign.rs:39) | TODO |
| NURBS sheet from surface | — | `Body::nurbs_sheet_body` | TODO |

## 10. Analysis / interrogation  (`interrogate.rs`, `massprops.rs`, `winding.rs`, `validate.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| Mass properties (volume, CoM, inertia) | (mass) | `Body::mass_properties` (massprops.rs:111) | PARTIAL (curved_volume_robustness) |
| Tessellated volume | — | `Body::tessellated_volume` (winding.rs:90) | PARTIAL |
| Section by plane | Section Analysis | `Body::section_by_plane` (interrogate.rs:966) | TODO |
| Section view | Section Analysis | `Body::section_view` (interrogate.rs:1015) | TODO |
| Draft analysis | (draft analysis) | `Body::draft_analysis` (interrogate.rs:779) | TODO |
| Clash / interference | (clash) | `Body::clashes` (interrogate.rs:933) | TODO |
| Render mesh | (display) | `Body::render_mesh` / `_tol` (interrogate.rs:153/164) | PARTIAL |
| Validate | (internal) | `Body::validate` (validate.rs:52) | PARTIAL |
| Simplify (recover analytics) | Delete Redundant Topology (partial) | `Body::simplify` (simplify.rs:30) | TODO |

## 11. Geometry-level building blocks  (`keel-geom`)

| Op | Used by | Entry point | Test |
|---|---|---|---|
| Offset surface | Offset Face | `offset::offset_surface` (offset.rs:18) | TODO |
| Revolve full | Revolve | `nurbs_surface::revolve_full` (479) | TODO |
| Fill boundary (Coons) | Patch | `fill::fill_boundary` (fill.rs:39) | TODO |
| Extend analytic surface | Extend Sheet | `extend::extend_analytic` (extend.rs:40) | TODO |
| Extend NURBS curve/surface | Extend | `extend::extend_nurbs_curve/_surface` (127/146) | TODO |
| Knot insertion (curve) | Refine | `NurbsCurve::insert_knot` (355) | TODO |
| Knot insertion (surface u/v) | Refine | `NurbsSurface::insert_knot_u/_v` (251/294) | TODO |
| SSI (surface-surface intersect) | Boolean/Imprint | `ssi::intersect_surfaces` | PARTIAL (soak) |

## 12. I/O  (`step_*.rs`)

| Op | Plasticity | Entry point | Test |
|---|---|---|---|
| STEP export | (studio export) | `to_step_string` (step_export.rs:74) | PARTIAL (step_quadrics) |
| STEP import | (studio import) | `from_step_string` (step_import.rs:929) | PARTIAL |

---

## Parity GAPS — kernel ops to build (priority order)

**C. Robustness (highest priority — the real parity blocker).** Not a new op:
make the booleans + fillet suite reliable on messy multi-feature input. Open #1:
the multi-cut **stitch-assembly corruption** (a far curved face's loop gets a
forward+backward edge spur during multi-cut ops -> mass declines -> correct
DECLINE of a malformed body). Three downstream patches proven unsafe (LOG Add
292/293/294); needs the ROOT fix in the stitch, not a heal-after. The
decline-minimization soak corpus is the worklist.

**B. Topology editing / healing (mostly present; few genuine gaps).**

| Gap | Plasticity | Note |
|---|---|---|
| Untrim | Untrim (Alt+T) | recreate face from base surface's natural domain |
| Remove Fillets (defillet) | Remove Fillets from Shell | inverse of the fillet suite: delete blend + extend-reintersect neighbors |
| ~~Explode / Unjoin~~ | Unjoin Shells (Alt+J) | DONE: `boolean::explode` (body → per-face sheets) |
| Delete Redundant Topology | Delete Redundant Topology | merge co-faces / co-edges (`simplify` covers part) |
| Extend Sheet (solid-level) | Extend Sheet | wrap geom `extend_*` to a sheet command |
| Remove Nominal Surface | Remove Nominal Surface | — |

**A. Free-form NURBS surfacing (largest net-new; the organic/artist pillar).**
Building blocks exist (NURBS surfaces, knot insert, degree elevate, fill,
extend); the high-level tools do not.

| Gap | Plasticity |
|---|---|
| XNurbs-class fitting | XNurbs |
| Degree/span surface | Square |
| Constrained / Bridge surface | Constrained Surface, Bridge Surface |
| Match face / Rebuild face | Match Face, Rebuild Face |
| Raise surface degree (expose) | Raise Surface Degree (Shift+S) |
| Isoparametric lines | Isoparam (Ctrl+R) |
| Slide surface CV / Deform | Slide Surface CV, Deform |
| Mesh → NURBS | PolySplines |

---

## App-level (no kernel op needed)

Sketching/drawing (Line, Arc, Spline, Circle, Ellipse, Rectangle, Polygon,
Text), 2D constraint solving, transform *gizmos*, all Arrays (app orchestrates
kernel copy+transform), Materials, Dimensions, display toggles (curvature,
points), Instances/Realize (scene graph), Measure readouts, Publish/Share. The
kernel supplies transform application (§8), the curve/surface geometry, and the
analysis queries (§10) underneath these.
