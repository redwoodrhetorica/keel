# Agent status: hollow/offset_body on curved fillet faces

[15:14] task received: make hollow/offset handle curved (fillet) faces so a filleted box can be shelled
[15:18] repro confirmed: hollow(3.0) on extrude+fillet box fails "offset_body: non-planar face"
[15:20] failing face type: cylindrical FILLET faces (r=4); body = 6 planes + 4 cylinders, no torus; all vertices 3-valent (PPP/CPP/CCP); edges 16 line, 4 circle (caps), 4 ellipse (bicylinder corner)
[15:26] offset for curved face drafted: offset_analytic_surface (plane/cyl/sphere/torus) + 3-surface LM-Newton vertex solve + per-edge curve recompute
[15:32] build green; inner offset body valid + mass==mesh, but boolean difference faults UnassemblableSeam (nested-curved SSI gap)
[15:38] added combine_containment (verbatim two-shell assembly, no SSI) + no-glue finalize variant (seam-pair preservation for green-slab)
[15:39] build green
[15:40] tutorial test passes: extrude_fillet_shell OK (mass 19661.7 ~ mesh 19739.4, validate true)
[15:42] un-ignored extrude_fillet_shell; added shell unit test hollow_filleted_box_shells
[15:44] fixed over-thick decline regression (containment precondition gate: inner must be positive solid < outer)
[15:50] WRONG-locks + suite green: lib 298 pass / integration all green (tutorial 7, cyl_union_mass_witness 3, post_fillet_mass 2, fillet_surgery 3, steinmetz, curved_volume 11); over-thick + box + prism + multi-thickness + tray all pass
[15:52] clippy clean (net zero new warnings: 18 keel-topo, all pre-existing); probe example removed
