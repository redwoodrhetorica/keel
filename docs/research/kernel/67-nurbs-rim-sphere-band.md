# 67 - NURBS-rim sphere band (the dominant curved-boolean decline)

Research prompt. Status: OPEN, research-grade, multi-session. Written 2026-06-17
after the multi-cut cap-plane root fix (LOG Add 294) closed the dominant
mass!=mesh residual, which re-exposed this as the new dominant class.

## Problem

The dominant remaining boolean decline class is the non-coaxial
**sphere-vs-{cylinder,cone} WRAP**: a cylinder or cone whose cross-section the
sphere fully ENCIRCLES (the sphere swallows the bar, or the bar pierces the
sphere off-axis). Measured on the 2x20000 evolve soak (both seeds, commit
b79d898), of 26,554 total declines:

- cyl/sph + sph/cyl `UnassemblableSeam`, all 3 ops: ~6,260
- cone/sph + sph/cone `AssemblyFailed`, all 3 ops: ~2,960
- combined ~9,200 ( ~35% of the whole decline corpus )

All DECLINE-safe today (WRONG=0). Goal: convert to PASS (mass==mesh==truth).

## What already works (do NOT redo)

The coaxial and single-window sub-cases PASS exact:

- Coaxial rod-through-ball: `cylinder_sphere` / `cone_sphere` SSI exact circles
  (Add 267/268).
- Single-window graze (sphere touches the lateral in one non-encircling loop):
  window-disc edge-conforming fan + radial subdivision (Add 281); tests
  `cyl_sphere_window_four_ops_pass_exact`, `cone_sphere_window_four_ops_pass_exact`.
- Planar-cap intersection (sphere truncated by a cylinder/cone END): Add 282.
- The WRAP SSI itself is SOLVED: `quadratic_branch_field` (ssi.rs) returns the
  two encircling NON-planar NURBS loops, verified on both surfaces to 1e-5
  (Add 276/280).

Proven recipe for the solved cases: {generalized SSI branch field} + {NURBS-loop
classify on both faces} + {per-pair tight volume oracle} + {edge-conforming
tessellation}.

## The wall (the actual research target)

The WRAP differs from the WINDOW in one structural way that breaks three
downstream stages: the seam is two closed NON-planar NURBS loops that ENCIRCLE
the sphere (not a small contractible window), splitting the sphere into a BAND
(between the two rims) plus two caps. The CYLINDER side is handled (Add 283:
`imprint_cylinder_wrap_bands`, gated behind KEEL_WRAP_FLOW, meshes exact). The
SPHERE side is unimplemented for arbitrary NURBS rims:

1. Frame-robust sphere NURBS-cut CLASSIFY. `sphere_face_interior_point`
   (boolean.rs) supplies the inside/outside test point and the tessellation
   anchor. Its band fallback is CIRCLE-RIM-specific and ORIENTATION-SENSITIVE
   (Add 267/268 lesson): reorienting the sphere frame drops the band/caps from
   classify; an arbitrary NURBS rim returns None or a degenerate pole. NEEDED: a
   frame-AGNOSTIC interior point for a spherical region bounded by arbitrary
   closed NURBS loops -- parametrize the loop on the sphere (u,v) domain, pick
   the max-margin interior (u,v) on the correct side, avoiding the u=0 seam and
   the v=+-pi/2 poles.

2. Multi-hole curved MASS (Green-slab) for arbitrary NURBS sphere loops. The
   generic-NURBS boundary-fin arm exists (Add 271, cylinder). The sphere
   band/cap faces bounded by NURBS rims integrate WRONG (probe_wrap: I mass
   16.07 vs 17.17; U 67.58 vs 121). The band is a winding-2 region between two
   encircling rims (cf. the cyl/cyl barrel per-fin winding flip, Add 275).
   References: dossier #60 sec 3 (multi-loop / iso-rect over-count gate) + #66
   (multiloop face mass).

3. Tessellation watertightness at the shared NURBS rim between the sphere
   band/cap and the cylinder/cone wall. The window case's edge-conforming fan
   (`fin_curve_samples`; a shared seam edge yields the SAME point set, just
   fwd-reversed, so it is watertight by construction) is the template;
   generalize to the band (two rims) and give `tessellate_sphere` a general
   NURBS-trim path (it currently has the circle-rim and window paths only).

## Dead ends (proven, do NOT re-attempt)

- Containment-DROP of whole seams to suppress phantoms (Add 260/261): UNSOUND,
  regressed the soak (FAIL 0->114 / 0->166). NOTE: the Add-294 fix was a NARROW
  per-face-overlap completion for circular-disk PLANES, NOT a seam drop -- a
  different grain; do not conflate.
- mev/mef antipode SPUR for the non-planar wrap imprint (Add 273): builds a
  collapsed zero-area band. Superseded on the cylinder by the slit-closing band
  split (Add 275, `imprint_cylinder_wrap_bands`); the sphere needs the analogous
  periodic-domain split (dossier #64), NOT a spur.
- Circle-rim-specific sphere band/clip (Add 267/268): correct for coaxial
  circles, does NOT generalize to NURBS rims -- that generalization IS this work.

## Resources

- Probes: `probe_wrap` (resume from a mesh-correct wrap under KEEL_WRAP_FLOW),
  `probe_csu`, `probe_cysph_fail`. Use a GENERIC sphere frame; a rod through the
  sphere POLE is a degenerate (u,v) sub-case.
- Code: boolean.rs `sphere_face_interior_point`, the wrap gate in `seam_curves`
  (~6469); massprops.rs `integrate_face_green`; tessellate.rs `tessellate_sphere`;
  imprint.rs `imprint_cylinder_wrap_bands` (the cylinder-side template).
- Dossiers: #59 (curved assembly = EdgeId radial-splice + two imprint
  obligations), #60 (curved mass: generic-NURBS integrand + multi-loop gate),
  #64 (periodic-domain seam split), #65 (canonical curved-seam identity: a shared
  child EdgeId so both operands pair by identity -> UnassemblableSeam cannot
  occur), #66 (multiloop face mass).
- The tight oracle `quadric_sphere_op_volume` backs the gate, so this is SAFE to
  develop un-gated: a WRONG result self-declines on mass!=oracle.

## Validation gate (non-negotiable)

DECLINE-never-WRONG. Any change to imprint/classify/mass/tessellate MUST pass the
full evolve soak FAIL=0 on BOTH seeds (20000 each) before shipping -- the
honesty-net soak caught both prior reverted attempts. Success = cyl/sph and
cone/sph WRAP PASS mass==mesh==truth for all 3 ops on probe_wrap-class geometry,
the tight oracle agreeing, soak FAIL=0, and no PASS-count regression.

## Why this is the highest-leverage target

It unblocks ~9,200 declines ( ~35% of the corpus ), and the machinery it builds
(frame-robust NURBS-loop classify + multi-hole curved mass + canonical seam
identity) is the SAME stack the general quartic classes (skew cyl/cyl ~1,480,
cone/cyl ~2,510) and the cone/block parabola/hyperbola (~3,330) all need. The
sphere band is the gateway to the entire non-coaxial curved frontier.
