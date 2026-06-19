# Union mass != mesh WRONG -> DECLINE (curved-gate hole)

**Date:** 2026-06-19
**Class:** DECLINE-never-WRONG violation (the kernel's prime directive).
**Surfaced by:** the faithful tutorial soak, seed `11400715918834827198`
(chain `shell -> mirror -> fillet -> union(cylinder)`).
**Status:** RESOLVED. The malformed union now DECLINES; WRONG = 0.

---

## 1. The violation

A realistic tutorial chain made the kernel EMIT a self-inconsistent body
(returned `Ok` instead of declining). At the UNION step the kernel returned a
genus-1 body with:

- analytic `mass_properties().volume` = **10009.65**
- tessellated `mesh_volume()` = **5242.54** (mass ~1.9x mesh; 4 curved faces)

The realsoak oracle (unchanged) flags it `FAIL mass!=mesh-curved:extrude`. The
kernel SHOULD have declined: its `AssemblyFailed("...mass != mesh")` gate fires
correctly for a *different* tool on the same step, but a *different union path*
returned `Ok` with the bad body. The gate had a hole.

The independent Monte-Carlo truth of `|A u B|` (400k samples, GWN-inside test
in each operand) is **~10000**, confirming the **mass is CORRECT** and the
**mesh_volume is the collapsed/wrong value** -- the body itself is malformed.

---

## 2. Direct-API repro (IRONCLAD -- no harness)

Reproduced through the PUBLIC api only (`crates/keel-topo/tests/union_wrong_repro.rs`),
so there is zero harness doubt. The exact geometry was extracted from the soak
via temporary `KEEL_DUMP_REPRO` instrumentation (since reverted):

```rust
// 1. block, centred at origin
block.block((-dx/2,-dy/2,-dz/2), 16.6158…, 25.1682…, 24.2372…)
// 2. shell (hollow), wall t = 1.0523…
let shelled = block.hollow(1.0523181503240229)?;
// 3. mirror across z = 12.1186…, then union with the original
let m = shelled.mirrored((0,0,12.1186…), (0,0,1))?;
let mirror_union = boolean(&shelled, &m, Union, 1e-7)?.body;
// 4. fillet the -y far edge, radius 0.8139…
let filleted = mirror_union.fillet_edge(edge_at(±7.2556,-11.5318,11.0663), 0.813899886354287)?;
// 5. union a z-axis cylinder, base (-1.6013,-2.3089,-10.4262), r=4.3459, len=79.8521
boolean(&filleted, &cyl, Union, 1e-7)   // <-- pre-fix: Ok with mass 10009.65 != mesh 5242.54
```

The direct call reproduced the WRONG with byte-identical values
(`mass 10009.649703662903 != mesh 5242.544488671887`, ratio 1.909). The
violation is in the kernel's boolean assembly, NOT the harness.

---

## 3. The gate hole (which union path bypassed the check)

`crates/keel-topo/src/boolean.rs`, the post-assembly self-consistency gate
(`finalize`-style block ~L5444-5577). The result has 4 curved faces, so it
takes the **CURVED branch** (`let curved = …; let ok = if curved { … }`,
~L5448).

Dissection of the malformed result (`examples/probe_union_wrong.rs`):

| body | comps | genus | mass | tessellated_volume | mesh_volume | open_ratio |
|------|-------|-------|------|--------------------|-------------|-----------|
| pre-union | 3 | 0 | 5458.95 | 5458.97 | 5458.97 | - |
| tool (cyl) | 1 | 0 | 4738.01 | 4722.81 | 4722.81 | - |
| **union result** | **3** | **1** | **10009.65** | **9995.07** | **5242.54** | **0.030** |

The defect: the tall cylinder physically bridges all three components, so the
union SHOULD be ONE connected component. Instead the result keeps **3
topologically disjoint components** that geometrically overlap, with a spurious
genus-1 handle. The mass integrator (correctly) reports the true ~10009 over
all the geometry, but the body is malformed.

Why the gate missed it -- the curved branch checks the WRONG mesh measure:

- `tessellated_volume()` (winding.rs:90) uses a **single global reference**
  (the first triangle vertex). On this body it reads **9995 ~ mass**.
- `mesh_volume()` (interrogate.rs:808) -- the USER-FACING + oracle measure --
  sums **per connected component**, recentring each on its own centroid. On
  this malformed body it **collapses to 5242**.

The curved branch's three sub-checks all passed:

1. `self_consistent`: compares `mass (10009)` vs **`tessellated_volume` (9995)**
   within 2% -> PASS (the single-reference tess agrees with mass).
2. `bound_ok`: op-volume band `[max(va,vb)-slack, va+vb+slack]` ~ `[4949, 10690]`;
   both mass (10009) and `mesh_volume` (5242) lie in band -> PASS (the collapsed
   5242 squeaked just above the 4949 union floor).
3. watertightness net: `mesh_open_ratio` 0.030 < 0.05 -> PASS.

**Root cause:** the curved branch NEVER compares `mass` against the user-facing
`mesh_volume()`. The planar branch (L5528-5542) does exactly that
(`(m.volume - mesh_vol).abs() <= …`), and so does the success-fault-clear block
(L5587-5593) -- but the curved branch substituted `tessellated_volume` and so
was blind to the per-component mesh collapse.

---

## 4. The fix (WRONG -> DECLINE, purely additive)

Add a `mesh_consistent` AND-condition to the curved branch that gates `mass`
against the USER-FACING `mesh_volume()`, declining the malformed result. To
DECLINE exactly the bodies the realsoak oracle calls WRONG (no more, no fewer),
the threshold MIRRORS the oracle's curved-WRONG condition (`realsoak.rs::audit`,
the `rel > 0.25` rung):

```rust
let mesh_consistent = match bm {
    Ok(mv) if mv > 0.0 => {
        let mesh = body.mesh_volume();
        let rel = (mv - mesh).abs() / mv.abs().max(mesh.abs()).max(1e-9);
        mesh.is_finite() && rel <= 0.25
    }
    _ => true,  // mass-declined: mesh can't be cross-checked; other gates carry it
};
self_consistent && bound_ok && tight_ok && mesh_consistent
```

`rel = |mass - mesh| / max(|mass|, |mesh|)`. The malformed union reads
`rel = 4767/10009 = 0.476 > 0.25` -> **DECLINE**. The result is now
`Err(AssemblyFailed("degenerate or self-inconsistent result (mass != mesh)"))`.

### Why 0.25 and not a tight band

A first attempt used a flat 2% mass-vs-mesh band. It correctly declined the
seed-198 body but FALSELY declined a CORRECT small bicylinder lens
(`cyl_cyl_band` Intersection: mass 2.155 = exact, `tessellated_volume` 2.129
within 1%, but `mesh_volume` 1.730 -- a 13.5% per-component coarse-tess deficit,
`rel = 0.197`). Correct curved bodies legitimately undershoot in the
per-component `mesh_volume` up to ~20%; the malformed collapse is ~48%. The
oracle's own boundary is `rel > 0.25` (worst correct curved primitive meshes
~4% under; the band 6-25% is a "tessellation note", not a wrong). Mirroring it
EXACTLY guarantees the gate declines precisely the oracle's curved WRONGs and
admits every body the oracle accepts.

**Nothing was loosened.** This is a new AND-condition: it can only DECLINE more,
never admit a body the prior gates rejected. The mass integrator
(`src/massprops.rs`) was NOT touched.

---

## 5. Verification

- **Direct-API repro now DECLINES** (`tests/union_wrong_repro.rs`, 2 tests):
  - `union_after_shell_mirror_fillet_declines_not_wrong` -- the DECLINE-never-WRONG
    contract (Err OR mass==mesh) holds via the Err branch.
  - `union_after_shell_mirror_fillet_is_declined` -- pins the exact outcome (Err).
- **Full `cargo test --release` GREEN** (keel-topo): 297 lib + all integration
  tests. The WRONG-locks pass: `scan_wrong`, `three_bucket`,
  `cyl_union_mass_witness`, `post_fillet_mass`.
- **`cyl_cyl_band` correct-lens preserved**: the Intersection lens still
  assembles (Ok with exact mass); no correct curved body regressed.
- **three-bucket oracle** (N=2000 smoke): `strict PASS 1913 / DECLINE 87 /
  WRONG 0; tolerant PASS 500 / DECLINE 0 / WRONG 0`. WRONG = 0 both lanes.
- **`cargo clippy --release` clean** on the changed files.
- **Faithful soak check**: `KEEL_FAITHFUL=1 KEEL_REPRO=11400715918834827198`
  now reports `realized=6 PASS` (was `realized=3 FAIL mass!=mesh-curved:extrude`).
  All three `[extrudeU ATTEMPT]` lines DECLINE; no FAIL/WRONG.

---

## 6. Files

- `crates/keel-topo/src/boolean.rs` -- the gate fix (curved branch
  `mesh_consistent` AND-condition).
- `crates/keel-topo/tests/union_wrong_repro.rs` -- direct-API regression
  (pins the DECLINE).
- `crates/keel-topo/examples/probe_union_wrong.rs` -- standalone dissection +
  Monte-Carlo truth of the malformed union (diagnostic artifact).
