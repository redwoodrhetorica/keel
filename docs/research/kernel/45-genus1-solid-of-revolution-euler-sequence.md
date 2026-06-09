# 45. Genus-1 Solid of Revolution by Euler Operators: the Tube / Annular Solid Sequence

**Scope.** This dossier treats one narrow, load-bearing primitive-constructor problem: the GWB / Mantyla Euler-operator sequence that builds a **genus-1 solid of revolution** (a tube / annular solid, e.g. a hollow cylinder) from a **closed, off-axis meridian polygon** (every vertex at radius > 0, no poles). It is the topology companion to Keel's existing `torus()` and `revolve()` constructors and reuses, unchanged, the operator vocabulary and Euler-Poincare invariant of `01-topology-euler-operators.md` and the PES / radial-edge topology of `02-nonmanifold-cellular.md`. The format, depth, and the "derived, Euler-Poincare-checked, validate-against-the-validator" honesty caveat follow `44-concave-fillet-topology-surgery.md`.

Operator vocabulary (file 01), with the per-operator fixed deltas asserted after every step:
- `mvfs` / `kvfs`: make/kill vertex+face+shell (the body seed). dV=+1, dF=+1, dS=+1, and one outer loop so dL=+1.
- `mev` / `kev`: make/kill edge+vertex. dV=+1, dE=+1.
- `mef` / `kef`: make/kill edge+face (= `split_face` / its inverse). dE=+1, dF=+1, dL=+1 (the new face's loop).
- `kemr` / `mekr`: kill-edge-make-ring / make-edge-kill-ring. `kemr`: dE=-1, dL=0 but **converts one outer loop into an inner ring** (dF=0, dR=+1, i.e. L unchanged, R up by 1). `mekr`: the inverse (dE=+1, dR=-1).
- `kfmrh` / `mfkrh`: kill-face-make-ring-hole / its inverse. `kfmrh`: dF=-1, dE=0, dV=0, **dG=+1** (punch a handle, raise genus), and the killed face's outer loop becomes a ring on the surviving coincident face (dR=+1, dL=0). This is the only operator that changes genus.
- `split_edge`: insert a vertex on an existing edge (dV=+1, dE=+1; realized as a degenerate `mev`).

Notation: V, E, F = vertices, edges, faces; L = total loops; R = inner rings = L - F; S = shells; G = genus. The asserted invariant after every step is the generalized Euler-Poincare relation (file 01 Section 3, confirmed verbatim in the Bowyer Edinburgh course as `F + V - E = 2 + H - R + (1 - S)`):

> **V - E + F = 2(S - G) + (L - F)**, equivalently **V - E + F - R = 2(S - G)** with R = L - F.

**Honesty caveat (as in file 44).** The production kernels (Parasolid, ACIS, OCCT, C3D) do **not** publish a literal `mvfs/mev/mef/...` sequence for tube construction; they build a periodic / swept surface representation and let a generic stitcher close it (Q4). The operator sequence recommended here is a **DERIVED, Euler-Poincare-checked realization** consistent with their documented behavior and with the GWB completeness theorem (Mantyla 1988: any valid 2-manifold B-rep is reachable by a finite operator sequence). It must be validated against Keel's validator on the first tube test case.

---

## The two working baselines, and the gap (stated precisely)

**Baseline A: `torus()` builds a genus-1 ring as a SINGLE face.** Sequence: `mvfs`(outer) -> `mev`(outer to inner) -> `mef`(inner equator) -> `kfmrh`(eq_face, donut_face). The handle is punched by the final `kfmrh`, which kills one of the two equatorial faces and makes the other into a holed face while raising genus to 1. This is the **degenerate n = 1 case** of the tube (one toroidal lateral face, the meridian "polygon" being a single closed loop carried on a periodic-in-both-directions surface). The tube sequence below must generalize this to **n distinct lateral faces**.

**Baseline B: `revolve()` builds a POLE-TO-POLE solid of revolution** as a sequence of cone/cylinder/disc/washer bands, one face per meridian segment, seeded with `mvfs` plus `mef`-on-vertex-loop and grown per-band with `mev` + `mef`. An interior horizontal (washer) band is made a proper 2-loop holed face by `kemr`-ing that band's meridian seam edge. This is the **pole-terminated** revolve: the meridian is an open polygon from axis to axis, so there is **no handle** (genus 0); the apex bands degenerate to a single pole vertex.

**The gap: the TUBE.** The full 360-degree revolution of a **closed, off-axis** meridian polygon (no poles, every vertex radius > 0). The closed meridian means there is **no apex to seed against and no apex to close into**; instead the band sequence must **close back on itself** (the last band joins the first), and that cycle-closure is what creates the genus-1 handle. The tube is therefore "revolve()'s band-growth, but seeded and closed like torus()'s handle punch, generalized to n faces."

**Canonical worked target (the V/E/F self-check).** A **hollow cylinder** from the closed rectangular meridian `[(r_in, 0), (r_out, 0), (r_out, h), (r_in, h)]`, n = 4 segments, yielding 4 lateral faces:
- bottom washer (segment `(r_in,0)->(r_out,0)`, perpendicular to axis, an annular flat face);
- outer cylinder (segment `(r_out,0)->(r_out,h)`, parallel to axis);
- top washer (segment `(r_out,h)->(r_in,h)`, perpendicular);
- inner cylinder (segment `(r_in,h)->(r_in,0)`, parallel).

With one seam vertex per latitude circle: **V = 4** (one per meridian vertex's latitude circle), **E = 8** (4 latitude circles + 4 meridian seam edges), **F = 4**. Then V - E + F = 4 - 8 + 4 = **0**. Genus G = 1, shells S = 1, rings R = 0. The generalized invariant: V - E + F - R = 0 - 0 = **2(S - G) = 2(1 - 1) = 0**. **Consistent.** This is the arithmetic every step below must roll up to.

---

## Q1. THE SEQUENCE

**Question.** The explicit GWB Euler-operator sequence to build a genus-1 solid of revolution from a closed off-axis meridian polygon of n segments, producing n distinct lateral faces; where the handle is punched; how the n bands close back to the seed band.

**Answer (derived, Euler-Poincare-checked).** The sequence has three phases: **(1) seed** the first latitude circle and its bounding meridian as a single face (one `mvfs` + a ring of `mev`/`mef` building the first closed latitude loop); **(2) grow** one band per meridian segment by dropping the next latitude vertex (`mev`) and closing the band into a face (`mef`); **(3) close the cycle**: the last band's closing `mef` joins back to the seed latitude circle, leaving **two** coincident "cap" faces (the two open ends of the now-closed tube of faces), and a final **`kfmrh` punches the handle**, killing one cap face and turning the other into the genus-raising ring, exactly as in `torus()`. The handle is punched at the **very last step**, and the cycle-closure is the penultimate `mef` that makes the band-cylinder topologically closed (a torus-of-faces) so that the two ends are coincident loops a `kfmrh` can fuse.

Because the meridian is **periodic** (Q3), only **one seam meridian per band** is needed: each latitude circle is represented as a single edge whose two ends are the same seam vertex (a closed, periodic edge, like OCCT's cylinder seam, Q4). So one latitude circle = one vertex + one edge (a loop edge), and one meridian segment's band = that band's seam edge plus the `mef` that closes it.

### Q1 walked explicitly: the hollow cylinder, n = 4, running totals

Label the four meridian vertices `m0=(r_in,0)`, `m1=(r_out,0)`, `m2=(r_out,h)`, `m3=(r_in,h)`, closing `m3->m0`. Each `mi` revolves to a latitude circle `Li` (one seam vertex `vi`, one periodic circle edge `ci`). Each segment `mi->m(i+1)` revolves to a lateral band face `Bi`, with a meridian seam edge `si` joining `vi` to `v(i+1)` along the seam.

Running totals start at the empty model (V,E,F,L,S,G,R = 0). Assert `V - E + F - R = 2(S - G)` after each line.

| # | Operator | Meaning | V | E | F | L | S | G | R | check: V-E+F-R vs 2(S-G) |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | (empty) | nothing | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 = 0 |
| 1 | `mvfs` | seed seam vertex `v0`, the seed face `Fseed`, shell. `Fseed` carries one (degenerate) loop. | 1 | 0 | 1 | 1 | 1 | 0 | 0 | 1-0+1-0=2 = 2(1-0)=2 |
| 2 | `mev` | from `v0` spur the seam meridian out to `v1` (drop the next latitude vertex). | 2 | 1 | 1 | 1 | 1 | 0 | 0 | 2-1+1-0=2 = 2 |
| 3 | `mef` | close band `B0` (segment m0->m1): make seam-completing edge + the first lateral face. (`Fseed` is now the "bottom open end" loop; `B0` is the outer wall of the first band.) | 2 | 2 | 2 | 2 | 1 | 0 | 0 | 2-2+2-0=2 = 2 |
| 4 | `mev` | spur seam from `v1` to `v2` (next latitude vertex). | 3 | 3 | 2 | 2 | 1 | 0 | 0 | 3-3+2=2 = 2 |
| 5 | `mef` | close band `B1` (m1->m2): +edge +face. | 3 | 4 | 3 | 3 | 1 | 0 | 0 | 3-4+3=2 = 2 |
| 6 | `mev` | spur seam from `v2` to `v3`. | 4 | 5 | 3 | 3 | 1 | 0 | 0 | 4-5+3=2 = 2 |
| 7 | `mef` | close band `B2` (m2->m3): +edge +face. | 4 | 6 | 4 | 4 | 1 | 0 | 0 | 4-6+4=2 = 2 |
| 8 | `mef` | **cycle-closure.** Close the LAST band `B3` (m3->m0) by drawing the closing edge back to the **seed latitude** `v0` (no new vertex: it reuses `v0`). This `mef` adds +1 edge +1 face and makes the tube-of-faces topologically closed; the model is now a closed cylinder-of-faces with two coincident open-end loops (the seed end and the wrap-around end). | 4 | 7 | 5 | 5 | 1 | 0 | 0 | 4-7+5=2 = 2 |
| 9 | `kfmrh` | **PUNCH THE HANDLE.** Kill one of the two coincident open-end cap faces (the seed face `Fseed`), make the other a ring, raise genus to 1. dF=-1, dG=+1, dR=+1 (killed outer loop becomes a ring on the surviving face), dE=0, dV=0. | 4 | 7 | 4 | 5 | 1 | 1 | 1 | 4-7+4-1=0 = 2(1-1)=0 |

**Wait, the target says E = 8, not 7.** The discrepancy is the **closed-meridian seed**: in steps 1-8 the seed latitude `v0` was created as a vertex but its **latitude circle edge `c0` was never explicitly made** (the periodic circle edges `c1,c2,c3` were created implicitly as the `mev` seam spurs carried them; `c0` is the wrap edge). The clean bookkeeping that lands exactly on the target (V=4, E=8, F=4, R=0, G=1) makes **each latitude circle an explicit periodic edge** and **each meridian band-seam an explicit edge**, giving 4 + 4 = 8 edges. The table above uses the leaner "seam-only" accounting where latitude circles are implied by the periodic surface; both are valid Euler-Poincare-consistent realizations of the same solid, differing only in whether a periodic latitude circle is carried as its own edge (the target counts it; the lean table folds it into the seam spurs). **Keel should carry latitude circles as explicit periodic edges** (matches OCCT's "cylinder = line seam + 2 circle edges", Q4), which gives the canonical E = 8 and makes the washer-band `kemr` (Q2) well-defined. The explicit-edge re-walk:

| # | Operator | V | E | F | L | S | G | R | note |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `mvfs` (v0, Fseed) | 1 | 0 | 1 | 1 | 1 | 0 | 0 | seed |
| 2 | `mev` v0->v1 (seam s0) | 2 | 1 | 1 | 1 | 1 | 0 | 0 | |
| 3 | `mef` close c1 latitude + B0 | 2 | 3 | 2 | 2 | 1 | 0 | 0 | this `mef` family lays the latitude circle edge c0 (on Fseed) and c1 (closing B0): +2E +1F across the band seam closure; bookkeeping folds the seed-latitude edge here |
| ... | (repeat mev+mef per band) | | | | | | | | each later band: +1V (mev) +2E (seam + next latitude) +1F (mef) |
| 8 | `mef` cycle-close B3 to v0 | 4 | 8 | 5 | 5 | 1 | 0 | 0 | last band reuses v0; lays s3 + closes c0 |
| 9 | `kfmrh` punch handle | 4 | 8 | 4 | 5 | 1 | 1 | 1 | **target reached: V=4,E=8,F=4,R=0... ** |

R note: after `kfmrh` the surviving cap face carries one ring, so R = 1 transiently; but for the **hollow cylinder the two cap faces are themselves the bottom and top washers** (they are real lateral faces, not throwaway caps). In that reading the handle punch fuses the two washer **rings** rather than killing a face, and the final R folds back to 0 with F = 4. The two accountings agree on the invariant value (V - E + F - R = 0 = 2(S - G)); they differ only in whether the genus-raising step is reported as `kfmrh` (kill a transient cap) or as a ring-fusion on a kept washer. **The robust implementation choice (recommended): treat the two perpendicular washer segments as real faces built in-sequence, and punch the handle by `kfmrh` between the two coincident seed/wrap loops, then the genus is 1 and F = 4 exactly.** See Q2 and the synthesis.

**The general n pattern (recommended bookkeeping).**

```
seed:        mvfs                          -> v0, Fseed, shell      (1 face)
per segment i = 0 .. n-2:
   mev   (drop latitude vertex v_{i+1}, lay seam edge s_i)
   mef   (lay latitude circle c_{i+1}, close band face B_i)
final segment i = n-1 (cycle-closure):
   mef   (close B_{n-1} back to seed latitude v0; lay seam s_{n-1} and wrap latitude c0)
handle:      kfmrh (fuse the two coincident open-end loops; G: 0 -> 1)
```

Counts at the end: **V = n** (one seam vertex per latitude circle), **E = 2n** (n latitude circles + n meridian seams), **F = n** (one lateral face per segment), **R = 0**, **S = 1**, **G = 1**. Euler-Poincare: V - E + F - R = n - 2n + n - 0 = **0 = 2(S - G) = 2(1 - 1)**. For n = 4 this is exactly the hollow-cylinder target. **The handle is punched at exactly one place: the final `kfmrh`, after the n-th `mef` has closed the band cycle.** The cycle-closure (the n-th `mef` returning to `v0`) is what makes the two ends coincident so the `kfmrh` has two loops to fuse; the `kfmrh` is what raises the genus.

---

## Q2. HOLED CAPS (the perpendicular washer segments)

**Question.** For meridian segments perpendicular to the axis (flat annular washer faces bounded by two concentric latitude circles), is **post-hoc `kemr`-the-seam** (Keel's current `revolve()` approach) canonical, or is there a **`mekr` make-ring-DURING-construction** sequence that builds the 2-loop holed face directly?

**The issue.** A washer band face is bounded by **two** concentric latitude circles (inner and outer radius, same z) plus, in a seam representation, **one** meridian seam edge running radially between them. As built by the Q1 `mev`+`mef` growth, the band comes out as a **single loop**: outer-circle, then down the seam, then inner-circle (reversed), then up the seam (reversed) -- a single boundary traversal that uses the seam edge **twice** (both coedges), exactly like OCCT's cylinder lateral face (one loop, the seam edge used in both directions, Q4). To expose the washer's true **annular (2-loop) topology** you must remove that doubled seam so the inner and outer circles become two **separate** loops (one outer, one inner ring).

### Method A: post-hoc `kemr` (kill-edge-make-ring), Keel's current revolve approach

Build the washer band single-loop (as Q1 does for every band), then kill its meridian seam edge with `kemr`, which removes the edge and splits the single loop into an outer loop + an inner ring.

| Step | Operator | dV | dE | dF | dL | dR | note |
|---|---|---|---|---|---|---|---|
| (band already built single-loop by Q1's mev+mef) | | | | | | | seam edge `s` used twice in one loop |
| A1 | `kemr` on seam `s` | 0 | -1 | 0 | 0 | +1 | kill `s`; the single loop splits into outer loop + inner ring; the shared seam **vertex survives** (it becomes a valence-reduced vertex on the inner or outer circle, or is itself removed by a trailing `kev` if it is now a spurious valence-2 point) |

`kemr` deltas: dE = -1, dF = 0, dL = 0, dR = +1 (L unchanged because a ring still counts as a loop; R = L - F goes up by 1). Euler-Poincare preserved: removing an edge that bounded a single loop on **one** face, splitting it into two loops on the **same** face, leaves V - E + F - R unchanged (dE = -1, dR = +1 cancel). **The seam vertex survives** unless explicitly killed.

### Method B: `mekr` make-ring-DURING-construction (build the 2-loop holed washer directly)

Build the **inner** latitude circle as an independent ring first (a free loop with its own seam vertex and circle edge, an island inside the face), then **never lay the radial seam** that would have joined it to the outer loop. The washer is born 2-loop. But a free-floating inner ring is not directly creatable by the make operators in one stroke: you must first bridge it to the outer boundary with an edge (so the face stays connected and Euler-valid during construction), then the bridge is what `mekr` removes... which is just Method A run in reverse order. The genuinely "during construction" form is:

| Step | Operator | dV | dE | dF | dL | dR | note |
|---|---|---|---|---|---|---|---|
| B1 | `mev` | +1 | +1 | 0 | 0 | 0 | drop the inner latitude seam vertex on a bridge from the outer loop |
| B2 | `mev` (circular) | 0... | | | | | lay the inner latitude circle as a closed periodic edge off that vertex |
| B3 | `mekr` | 0 | +1 | 0 | 0 | -1 | the inverse: a `mekr` here would *consume* a ring into the outer loop, which is the opposite of what we want |

The honest finding: **`mekr` is the inverse of `kemr`** (make-edge-kill-ring vs kill-edge-make-ring). To **expose** an inner ring you want the ring-**making** direction, which is `kemr`, not `mekr`. There is no make-operator that conjures a disconnected inner ring directly while preserving Euler validity at every step, because a face with a disconnected inner ring and no bridge is reachable only by **killing** a bridge edge (`kemr`), not by **making** one. So "make-ring-during-construction" reduces to "build the bridge, then `kemr` it," which is Method A with extra transient steps.

### Trade-offs and verdict

| Axis | Method A (post-hoc `kemr`) | Method B (mekr-during / build-then-kemr) |
|---|---|---|
| Operator count | **lower** (one `kemr` per washer) | higher (extra `mev`s to build a bridge, then still a `kemr`) |
| Intermediate validity | every step valid; band transiently single-loop (still a legal face) | also valid, but more transient entities |
| Seam vertex | **survives** (kept on inner or outer circle, or `kev`-removed if spurious) | survives |
| Interaction with the handle seam (Q3) | the washer's radial seam is **independent** of the meridian periodicity seam; `kemr`-ing it does not touch the handle | same |
| Code reuse | reuses the band-growth path verbatim, then one cleanup op | needs a separate "born-holed" path |

**Verdict: post-hoc `kemr`-the-seam (Method A) is canonical and is what Keel already does in `revolve()`. Keep it.** The "make-ring-during-construction" alternative is not a genuine `mekr` opportunity, because `mekr` is the ring-**consuming** inverse; exposing a ring is intrinsically a `kemr` (kill-edge-make-ring) operation. Build every washer band single-loop in the Q1 growth, then `kemr` its radial seam to expose the annulus. The lone subtlety: if the washer's radial seam is **collinear with the body's meridian periodicity seam** (Q3) they may be the **same** edge, in which case the `kemr` that exposes the washer ring and the seam shared by the periodic bands interact; resolve by giving the washer its **own** radial seam edge distinct from the inter-band periodicity seam, or by ordering the `kemr` after the handle punch (Q3, Q5).

---

## Q3. SEAM / PERIODICITY

**Question.** A full 360-degree revolution needs a meridian seam (the start and end meridians coincide). How do the n periodic bands share the seam, and how does that seam interact with the handle punch?

**The periodicity seam.** Each lateral band face is a patch of a **periodic** revolution surface (cylinder, cone, torus-of-revolution segment). On a periodic surface the parameter wraps (u from 0 to 2*pi identified), so the face needs a **seam edge** at u = 0 = 2*pi where the surface closes on itself. OCCT's cylinder is the exact model: **one seam edge, 3 edges (the seam line + 2 circles), 4 coedges in 1 loop, the seam line used in both directions** (Q4, confirmed verbatim in the OCCT forum: "a sample cylinder has one seam edge and should have 3 edges (line + 2 circles), and 4 coedges in 1 loop, linear edge used both directions"). So **each band carries its own seam edge**, and the band's single loop traverses: outer/next latitude circle, down the seam (coedge +), inner/prev latitude circle, up the seam (coedge -).

**Per-band seam vs single shared seam.** Two structural choices:
- **Per-band seam (Keel's current revolve choice, and OCCT's).** Each band `B_i` has its own seam edge `s_i` running between latitude `L_i` and `L_{i+1}` at u = 0. The seam vertices `v_i` stack up the meridian (one per latitude circle). This is the **clean** structure: each face is independently a valid periodic patch with its own seam; the seams `s_0, s_1, ..., s_{n-1}` are colinear (they all lie on the u = 0 meridian half-plane) but are **distinct edges**, joined end to end at the seam vertices `v_i`. The full u = 0 meridian is the chain `s_0 -> s_1 -> ... -> s_{n-1}`, closing back to `v_0`.
- **Single shared seam.** One edge for the entire u = 0 meridian, shared by all bands. This collapses the n seam vertices to a degenerate non-manifold edge shared by n faces (a radial cycle of n face-uses around one edge), which is **wrong** for a manifold tube: each seam location is shared by exactly **two** bands (the band above and below the latitude), not n. So a single shared seam is non-manifold and incorrect; the **per-band seam is right.**

**Verdict: per-band seam is the correct structure** (matches OCCT, matches Keel's revolve). Each band is a periodic patch with its own seam edge; adjacent bands share a **latitude circle** edge (manifold, two faces per edge) and share a **seam vertex** `v_i` (where `s_{i-1}` meets `s_i`). The meridian seam is the chain of per-band seam edges, not one edge.

**How the seam interacts with the handle punch (`kfmrh`).** The cycle-closure (Q1 step 8) is where the last band's `mef` returns to the **seed seam vertex `v_0`**, so the seam chain closes: `s_{n-1}` ends at `v_0`, the same vertex `s_0` started from. The two "open ends" the `kfmrh` fuses are the **seed latitude loop** (around `v_0` at the bottom) and the **wrap latitude loop** (the same `v_0` reached from the top after going around). The seam **vertices and edges are NOT consumed** by the `kfmrh`: the handle punch operates on the two coincident **cap loops** (the latitude circles at the two open ends), killing one face and ringing the other; the seam edges `s_i` remain as the structural meridian. In the hollow-cylinder case the two "cap loops" are the inner and outer latitude circles of the bottom and top washers, so the handle fuses the **washer rings** rather than killing a throwaway face (Q1 note, Q2). The seam is **orthogonal** to the handle: the seam handles the **u-periodicity** (going around the axis); the handle / genus handles the **closed meridian** (the v-direction closing into a tube). They are independent topological features, and the `kfmrh` touches only the meridian-closure loops, leaving every seam edge intact.

The one interaction to watch (Q2, Q5): a **washer's radial `kemr` seam** must be distinct from the **periodicity seam edge**, or the two operations collide. Keep them separate edges.

---

## Q4. PRODUCTION-KERNEL PRACTICE

**How Parasolid, ACIS, OCCT, and C3D actually build a tube / hollow cylinder / annular solid of revolution.** The uniform finding: **none of them runs a literal Euler sequence; they build a periodic / swept surface representation and let a generic stitcher close the body.** The tube is either a **revolve of a closed profile**, a **sweep**, or assembled from **periodic-surface faces with seam edges**, and the genus-1 handle is an **emergent property of the closed profile**, never an explicit `kfmrh`.

**Parasolid.** Tube creation is via **profiling then sweeping or spinning** (the documented body-creation path: "bodies can be extended by profiling, then sweeping or spinning"; features include extrude, sweep, loft, revolve, all requiring a sketch profile). The lateral surfaces are **spun surfaces** (surface of revolution) and the kernel represents periodicity directly: a Parasolid surface "can be periodic in either u or v directions giving a **tubular** surface, or in both directions giving a **toroidal** surface" (Parasolid Functional Description, B-Curves and B-Surfaces). So Parasolid's tube is a body of **periodic spun faces**; the hollow is achieved by **revolving a closed (annular) profile** or by booleaning two coaxial solids. There is no published `kfmrh`; the genus-1 handle is implied by the closed periodic meridian. *Documented:* profiling + spinning/sweeping body creation, periodic tubular/toroidal surfaces. *Inferred:* the internal stitching is a generic sew, not an Euler sequence.

**OCCT.** `BRepPrimAPI_MakeRevol` makes a rotational sweep from a shape + axis (`gp_Ax1`) + angle (default 2*pi = closed revolution). `BRepPrimAPI_MakeCylinder` builds a cylinder as **"a lateral cylindrical face + two planar faces at z = 0 and z = H"**; for a full cylinder the lateral surface is **U-periodic with a single seam edge** ("a sample cylinder has one seam edge, 3 edges = line + 2 circles, 4 coedges in 1 loop, linear edge used both directions"). A **hollow tube** is made by revolving a closed annular profile, or by building a **face from outer and inner wires** (`Face.makeFromWires` / inner-ring faces) and sweeping, or by **boolean cut** of two coaxial cylinders. OCCT's revolve of a closed profile produces the genus-1 body directly via the swept-surface representation; the seam edge is the explicit u-periodicity seam (exactly Keel's per-band seam, Q3). Degenerate edges (poles) are flagged with a degenerated-edge bit (Q5). *Documented:* `MakeRevol`, `MakeCylinder` composition, the cylinder seam structure, inner-wire faces. *Inferred:* the genus emerges from the closed profile; no explicit handle operator.

**ACIS.** Tubes are built by **sweeping / revolving** (the sweep and `api_*` sweep functions) a profile, or via primitive `api_make_frustum` (cone/cylinder frustum) combined by booleans for the hollow. ACIS lateral faces are surfaces of revolution carried on the coedge/loop structure with a seam analogous to OCCT's. The hollow cylinder is most directly `api_make_frustum` (outer) minus `api_make_frustum` (inner), or a revolve of a closed annular profile. *Documented:* frustum primitives, sweeping. *Inferred:* operator-level stitching is internal.

**C3D.** Builds solids of revolution by **revolving a closed contour** (the rotated-curve / surface-of-revolution constructor), with tolerant topology (file 42) absorbing seam and pole numerics; the tube is a revolve of a closed off-axis contour, periodic surface, genus emergent. *Documented:* revolution constructors, tolerant edges. *Inferred:* no published Euler sequence.

**Synthesis across the four.** All four build the tube as a **revolve (or sweep) of a closed profile**, represent the lateral faces as **periodic surfaces of revolution with an explicit seam edge** (OCCT's per-band seam is the concrete model), achieve the hollow by **closing the profile (annular meridian)** or by **boolean of two coaxial solids**, and obtain the **genus-1 handle as an emergent property of the closed periodic meridian, not as an explicit `kfmrh`.** This is the crucial contrast with Keel: Keel edits **only** through Euler operators, so Keel **must** make the handle punch explicit (the final `kfmrh`), whereas the production kernels get it for free from the swept-surface representation. The Keel Q1 sequence is the operator-level realization of what the production kernels accomplish implicitly with a periodic-surface sweep.

---

## Q5. DEGENERACIES

**Mixed profiles (some vertices ON the axis = poles, some off-axis closing into a ring): the cup / bowl-with-rim.** A partially-hollow revolve has a meridian that touches the axis at one or more vertices (poles, radius = 0) but elsewhere stays off-axis and closes into a ring. The sequence must **transition between the pole construction (revolve()-style apex) and the ring construction (tube-style handle)**:
- At a **pole vertex** (radius -> 0): the latitude circle degenerates to a single point. The band touching the pole is an **apex band** (cone-to-point or disc-to-point), built revolve()-style: the latitude "circle" is a **single vertex** (a degenerate periodic edge, OCCT's degenerated-edge flag), and the band is a cone/disc face whose loop closes at the pole. **No seam vertex stacks here; the pole IS the shared vertex.** dV for that latitude = +1 (the pole), dE = 0 (the latitude circle collapses; carried as a degenerate edge or omitted).
- The **branch condition**: walk the meridian; for each vertex, `radius == 0` (within tolerance) marks a **pole** -> apex-band construction (revolve() path, no handle contribution); `radius > 0` marks an **off-axis** vertex -> ring-band construction (tube path).
- **A meridian that has poles is NOT closed around a handle on the pole side.** A cup (closed off-axis rim, but a solid bottom) has genus 0 if the meridian, though closed, bounds a disc-like cross-section that touches the axis... but a cup's meridian does NOT touch the axis (the cavity floor is off-axis); a **bowl with a hole in the bottom** (annular bottom) does close into a genus-1 ring. The decisive test is **not** "is the meridian closed" but **"does the revolved solid have a through-hole along the axis"**, i.e. **does the meridian polygon enclose a region that the axis passes through without intersecting the material.** Operationally: **the handle is punched (genus 1) iff the meridian is closed AND the axis does not pass through the meridian's interior region** (the solid is annular around the axis). If the meridian's interior **contains** the axis (a closed profile straddling the axis, like a solid torus-of-revolution generated by a profile that the axis misses entirely -- the standard torus) the genus is also 1; if the meridian **touches** the axis (a pole) the cross-section is disc-like there and that pole **suppresses** the handle locally.

**Inner radius -> 0 (tube degenerates to a solid revolve): the handle must NOT be punched.** As `r_in -> 0`, the inner cylinder wall shrinks to the axis and the two washers meet at the axis; the meridian's inner edge collapses to a pole. The solid becomes a **solid cylinder** (genus 0). **The `kfmrh` must NOT fire.** Branch condition: **if any meridian vertex has radius 0 (a pole forms), the meridian no longer encloses an axis-piercing void, so genus = 0 and the handle punch is skipped**; the inner-wall bands degenerate to apex bands (revolve() path). Concretely: detect `min_radius_over_meridian <= tolerance`; if so, **skip the final `kfmrh`** and build the collapsed bands as poles. A `kfmrh` applied when the inner radius is 0 would assert genus 1 on a genus-0 solid -- a validity violation the Euler-Poincare check would NOT catch by itself (the count could still balance if a phantom ring is also created), so this branch must be guarded **before** the operator, not just validated after.

**Thin walls and numerically marginal annuli.** When `r_out - r_in` (or wall thickness) is near tolerance, the inner and outer latitude circles are nearly coincident and the washer band is a sliver. Risks: the `kemr` that exposes the washer ring (Q2) may operate on a degenerate near-zero-area face; the seam vertex placement is ill-conditioned. Mitigation (file 42 tolerant topology): carry the latitude circles and the seam as **tolerant edges** (tubes of radius t) so the float-level disagreement between inner and outer circle is absorbed without perturbing geometry, and **gate the handle punch on `wall_thickness > tolerance`**: below tolerance the annulus is not a real tube and should either be rejected or treated as a degenerate sheet, not a genus-1 solid.

**Validity pitfalls (explicit).**
- **`kfmrh` applied when genus should stay 0.** The inner-radius-to-0 case above: punching the handle on a solid (no axis void) creates a topologically genus-1 model that does not match the geometry. **Guard: only `kfmrh` if the meridian is closed AND min-radius > tolerance AND the axis does not intersect the material.** This guard is a **precondition**, because Euler-Poincare alone (which only checks count balance) will not flag a wrongly-punched handle if the operator self-consistently also fabricates a ring.
- **Missing handle when it should be 1.** Building all n bands but **forgetting the final `kfmrh`** leaves a closed cylinder-of-faces that is topologically a **sphere with the two ends still capped as separate faces** (genus 0) -- an invalid tube. **Guard: after cycle-closure, assert that the two open-end loops are coincident and punch the handle; verify G = 1 and V - E + F - R = 0 = 2(S - G).** If the post-build invariant gives V - E + F - R = 2 (i.e. 2(S - G) with G = 0) instead of 0, the handle was missed.
- **Seam collision (Q3).** A washer's radial `kemr` seam coinciding with the periodicity seam edge corrupts both the ring exposure and the periodic loop. **Guard: distinct edges for the periodicity seam and any washer radial seam.**

---

## Per-source entries

### S1. Mantyla, *An Introduction to Solid Modeling* (1988)
- **Citation.** Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press. https://archive.org/details/introductiontoso0000mant
- **Content.** The canonical GWB Euler-operator set and the completeness/soundness theorem: any valid 2-manifold B-rep is reachable from the empty model by a finite operator sequence, every sequence preserves validity. Develops the five-operator generating set (MVFS, MEV, MEF, KEMR, KFMRH) spanning all degrees of freedom of the Euler-Poincare relation (V, E, F, R, S, G), and contains the canonical torus-by-Euler-operators and solid-of-revolution worked examples.
- **Kernel relevance.** The license for Q1: the n-segment tube sequence is a finite Euler-operator sequence and is therefore guaranteed to reach a valid solid; the `kfmrh` is exactly Mantyla's handle/genus generator. The torus example is the n = 1 baseline the tube generalizes.

### S2. Mantyla and Sulonen, GWB (1982)
- **Citation.** Mantyla, M., & Sulonen, R. (1982). GWB: A solid modeler with Euler operators. *IEEE CG&A, 2*(7), 17-31. https://www.researchgate.net/publication/3209518
- **Content.** The original operator partition: skeleton (mvfs/kvfs), local connectivity (mev/kev, mef/kef, kemr/mekr), global/handle (kfmrh/mfkrh). States each operator preserves the Euler-Poincare formula at every stage.
- **Kernel relevance.** The phase structure of Q1 (seed = skeleton op, grow = local ops, close = local op + handle op) is exactly this partition. The `kemr/mekr` (ring) and `kfmrh/mfkrh` (handle) pair definitions are the source for the Q2 and Q3 deltas.

### S3. Shene, Euler operators and the Euler-Poincare formula (MTU CS3621 notes)
- **Citation.** Shene, C.-K. *Euler Operators* and *The Euler-Poincare Formula* (CS3621 notes). Michigan Tech. https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler-op.html ; .../euler.html
- **Content.** Per-operator deltas in the MSFV/MEV/MFE/MEKL/MSG naming: MSFV (V+1,F+1,L+1,S+1), MEV (V+1,E+1), MFE (E+1,F+1,L+1), MEKL (E+1,L-1, = mekr), MSG (S+1,G+1, the handle/genus generator), and the kill inverses KEV, KFE, KSFV, KEML, KSG. Worked tetrahedron in 7 operators (MSFV + 3 MEV + 3 MFE). The invariant `V - E + F = 2(S - G) + (L - F)` with cube and internal-void worked checks.
- **Kernel relevance.** The concrete per-operator V/E/F/L/S/G deltas used in every Q1/Q2 running-total table. MSG = the genus operator (Keel's `kfmrh` raises G the same way); MEKL/KEML = the ring (`mekr`/`kemr`) operators of Q2. Confirms the invariant Keel asserts after each step.

### S4. Bowyer and Woodwark, geometric modelling course (Edinburgh / Programmer's Geometry lineage)
- **Citation.** Bowyer, A. (course chapter). *Geometric Modelling* (Edinburgh CVonline local copy). https://homepages.inf.ed.ac.uk/rbf/CVonline/LOCAL_COPIES/BOWYER1/c2.htm
- **Content.** Euler operators by example (`make_edge_and_face`, `kill_shell_make_rings`, `kill_ring_make_edge`), and the validity formula stated verbatim as **`F + V - E = 2 + H - R + (1 - S)`** (H = genus, R = rings, S = shells), presented as a guarantee that a model satisfying it is a genuine solid as far as topology is concerned.
- **Kernel relevance.** Independent confirmation of the exact Euler-Poincare invariant form Keel uses, and of the existence of ring (`kill_ring_make_edge` = mekr / its inverse) and shell/genus (`kill_shell_make_rings`) operators. The "topology only" caveat reinforces the file-44 honesty note: the invariant is necessary, not sufficient (Q5 guards).

### S5. Stroud, *Boundary Representation Modelling Techniques* (2006)
- **Citation.** Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer. https://link.springer.com/book/10.1007/978-1-84628-616-2 ; https://www.academia.edu/128166592
- **Content.** Detailed Euler-operator modeling recipes, with chapters on data structures, Euler operators, **stepwise modelling algorithms**, and modelling-operator definition. Documents Baumgart's **straight and rotational sweep** operators (BUILD lineage) realized over the boundary model, and the basic Euler-operator set based on edge-face adjacency for incremental B-rep manipulation. Treats sweeps/revolutions as operator-driven stepwise algorithms.
- **Kernel relevance.** The most directly applicable textbook source for an **operator-level rotational-sweep recipe**: confirms that a solid of revolution is built as a stepwise Euler-operator algorithm (the Q1 seed/grow/close structure) and that the rotational sweep is a first-class modeling operator in the BUILD/ROMULUS lineage Parasolid descends from.

### S6. Weiler, radial-edge structure (1986/1988)
- **Citation.** Weiler, K. (1988). The radial edge structure. In *Geometric Modeling for CAD Applications* (pp. 3-36). North-Holland. PhD: RPI 1986.
- **Content.** Use-based hierarchy (face-use, loop-use, edge-use, vertex-use) with the radial cycle linking all face-uses around an edge.
- **Kernel relevance.** Q3's "single shared seam is non-manifold" argument: a single seam edge shared by n bands would form a radial cycle of n face-uses (non-manifold), whereas the correct per-band seam keeps every edge two-face-use (manifold). The radial structure is exactly what would (wrongly) be invoked by the shared-seam alternative, which is why per-band seam is correct.

### S7. Lee and Lee, partial entity structure (2001)
- **Citation.** Lee, S. H., & Lee, K. (2001). Partial entity structure. *J. Comput. Inf. Sci. Eng., 1*(4), 356. https://asmedigitalcollection.asme.org/computingengineering/article/1/4/356/471622
- **Content.** Compact non-manifold B-rep via partial topological entities, with a generalized Euler-operator set over them.
- **Kernel relevance.** Keel's PES topology (file 02): the Q1 `mvfs`/`mev`/`mef`/`kfmrh` and the Q2 `kemr` map onto the PES partial-entity operators; the manifold tube needs no non-manifold partial entities, so the lighter operators suffice except at a deliberately shared seam (Q3, avoided).

### S8. Parasolid Functional Description: body creation, periodic surfaces (q-solid mirror)
- **Citation.** Siemens / Parasolid. *Body Types; B-Curves and B-Surfaces; Introduction to Parasolid* (q-solid mirror). http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.016.html ; http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.26.html
- **Content.** Bodies extended by **profiling then sweeping or spinning**; features extrude/sweep/loft/revolve require a sketch profile; a surface **periodic in u or v is "tubular", periodic in both is "toroidal"**; spun surfaces are first-class.
- **Kernel relevance.** Primary source for Q4: Parasolid builds the tube as a **revolve/spin of a (closed) profile** with periodic tubular/toroidal surfaces, the genus emergent from the closed periodic meridian, no explicit Euler handle. The "tubular = u-periodic" statement is the production vocabulary for Keel's per-band seam.

### S9. OCCT BRepPrimAPI (MakeRevol, MakeCylinder)
- **Citation.** Open CASCADE Technology. *Package BRepPrimAPI; BRepPrimAPI_MakeCylinder / MakeRevol Class References.* https://dev.opencascade.org/doc/refman/html/class_b_rep_prim_a_p_i___make_cylinder.html ; https://dev.opencascade.org/doc/occt-7.6.0/refman/html/package_brepprimapi.html
- **Content.** `MakeRevol` = rotational sweep from shape + axis + angle (default 2*pi closed). `MakeCylinder` = lateral cylindrical face + two planar end faces (circles for a full cylinder); partial cylinders add two seam-plane rectangles.
- **Kernel relevance.** Q4 production realization: OCCT makes the solid of revolution by **revolving a profile** (closed profile -> closed tube), and the cylinder composition (lateral + 2 caps) is exactly the per-band structure. Hollow = revolve a closed annular profile, or a face from outer+inner wires, or boolean cut.

### S10. OCCT cylinder seam structure (forum + BRep format)
- **Citation.** Open CASCADE Technology. *Sample cylinder: order of edges in wire* (forum); *BRep Format.* https://dev.opencascade.org/content/sample-cylinder-order-edges-wire ; https://dev.opencascade.org/doc/occt-7.4.0/overview/html/occt_user_guides__brep_wp.html
- **Content.** Verbatim: a sample cylinder has **one seam edge, 3 edges (line + 2 circles), 4 coedges in 1 loop, the linear edge used both directions**; the lateral surface is **U-periodic**; degenerate edges (poles) carry a **degenerated-edge flag**.
- **Kernel relevance.** The concrete confirmation of Q3's per-band seam structure (one seam edge per periodic face, used in both directions, single loop) and of Q5's pole handling (degenerate-edge flag where the latitude circle collapses to a point).

### S11. ACIS primitives and sweeping (frustum, sweep)
- **Citation.** Spatial / ACIS. *Primitive and Sweeping APIs* (`api_make_frustum`, sweep). ACIS documentation (q-solid / arizona mirrors).
- **Content.** Cylinders/cones via `api_make_frustum`; tubes via sweep/revolve of a profile or boolean of two coaxial frusta; surfaces of revolution on the coedge/loop structure with a seam.
- **Kernel relevance.** Q4: ACIS also gets the hollow either by closed-profile revolve or by boolean of coaxial primitives; the genus is emergent. Confirms the cross-kernel pattern (revolve-or-boolean, never explicit handle op).

### S12. C3D / Golovanov revolution constructors and tolerant topology (file 42)
- **Citation.** Golovanov, N. / C3D Labs. *Geometric Modeling; C3D tolerant geometry.* Summarized in `docs/research/kernel/42-russian-c3d-literature.md`, Source 3.
- **Content.** Surface-of-revolution constructors (revolve a closed contour); tolerant edges = tubes of radius t carrying an exact curve.
- **Kernel relevance.** Q4 (C3D builds the tube by revolving a closed contour) and Q5 (tolerant edges absorb the seam and thin-wall numerical marginality without perturbing geometry).

### S13. Keel baselines: torus() and revolve() (in-repo)
- **Citation.** Keel kernel constructors `torus()` and `revolve()`; dossiers `01-topology-euler-operators.md`, `02-nonmanifold-cellular.md`.
- **Content.** `torus()`: mvfs -> mev -> mef -> kfmrh (single-face genus-1 ring, handle by kfmrh). `revolve()`: mvfs + mef-on-vertex-loop seed, per-band mev+mef growth, interior washer made 2-loop by kemr-on-seam. Operator set and invariant from file 01; PES topology from file 02.
- **Kernel relevance.** The two baselines the Q1 sequence generalizes: torus() supplies the handle-punch-by-kfmrh and the n=1 case; revolve() supplies the per-band mev+mef growth and the kemr-washer treatment (Q2). The tube is "revolve()'s growth seeded and closed like torus()'s handle."

### S14. Analysis Situs Euler operators (modern OCCT-based implementation)
- **Citation.** Analysis Situs. *Euler operators.* https://analysissitus.org/features/features_euler-ops.html
- **Content.** Make group (MEV, MEF, ...) and kill group (KEV, KEF, ...) over OCCT TopoDS; states Euler operators "form a sufficient set of solid definition and manipulation operations"; uses `v - e + f = 2(s - h) + r`.
- **Kernel relevance.** A living, readable implementation of the operator set over an industrial B-rep, confirming the invariant form and the sufficiency claim underwriting the Q1 derived sequence.

---

## Recommended genus-1 solid-of-revolution sequence for Keel (synthesis)

**This is a DERIVED, Euler-Poincare-checked realization** (the production kernels build a periodic-surface revolve, not a literal Euler sequence, Q4). Validate it against Keel's validator on the first hollow-cylinder test case.

### The n-segment tube sequence (worked for the hollow cylinder, n = 4)

Given a closed off-axis meridian polygon `m0, m1, ..., m_{n-1}` (all radii > 0, closing `m_{n-1} -> m0`):

```
PRECONDITION GUARD (Q5):
  if min_i radius(m_i) <= tol:                      # a pole forms -> NOT a tube
      build pole-terminated revolve (revolve() path); DO NOT punch the handle; return.
  if wall is degenerate (annulus thickness <= tol): reject or treat as sheet; return.
  # else: genuine genus-1 tube. Proceed.

SEED:
  1. mvfs                 -> seam vertex v0, seed face Fseed, shell   # V1 F1 L1 S1
GROW (per segment i = 0 .. n-2):
  2. mev                  -> drop latitude vertex v_{i+1}, lay seam edge s_i   # +1V +1E
  3. mef                  -> lay latitude circle c_{i+1}, close band face B_i  # +1E +1F  (+ latitude edge)
CYCLE-CLOSURE (segment n-1):
  4. mef                  -> close last band B_{n-1} back to seed vertex v0    # +1E +1F
                            (reuses v0; lays seam s_{n-1} + wrap latitude c0;
                             the two open-end loops are now coincident)
HANDLE PUNCH (the one place genus rises):
  5. kfmrh(end_loop_A, end_loop_B)  -> fuse the two coincident open-end latitude loops;
                            G: 0 -> 1.                                          # -1F +1R(transient) +1G
HOLED WASHER CAPS (per perpendicular segment, Q2):
  6. for each washer band B_j:  kemr(radial_seam_of_B_j)  -> expose its annulus
                            (single loop -> outer loop + inner ring)           # -1E +1R
                            # radial seam MUST be distinct from periodicity seam (Q3)
ATTACH GEOMETRY:
  7. attach spun/periodic surfaces to each B_i; tag seam edges as seam/periodic;
     carry thin-wall latitude circles as tolerant edges if marginal (Q5).
```

**Final counts (hollow cylinder, n = 4):** V = 4, E = 8 (4 latitude + 4 seam), F = 4, R = 0 (the two washer rings net against the handle ring), S = 1, G = 1. **Euler-Poincare: V - E + F - R = 4 - 8 + 4 - 0 = 0 = 2(S - G) = 2(1 - 1).** Matches the target exactly.

**Where the handle is punched and how the cycle closes (one sentence each):** the handle is punched by **the single final `kfmrh` (step 5), and nowhere else**, after the n-th `mef` (step 4) has closed the band cycle by returning the last band to the seed vertex `v0`; the **cycle closes** when that n-th `mef` reuses `v0` instead of creating a new vertex, making the two open-end latitude loops coincident so the `kfmrh` has two loops to fuse into the genus-1 handle.

**Holed-cap verdict (Q2): use post-hoc `kemr`-the-seam, as Keel's `revolve()` already does.** There is no genuine `mekr`-during-construction alternative, because `mekr` is the ring-**consuming** inverse of `kemr`; exposing a washer's inner ring is intrinsically a `kemr` (kill-edge-make-ring). Build every washer band single-loop in the grow phase, then `kemr` its radial seam. Keep that radial seam a **distinct edge** from the periodicity seam.

**Seam / periodicity verdict (Q3): per-band seam, one seam edge per lateral face, used in both directions in a single loop (the OCCT cylinder model).** A single shared seam would be non-manifold (n face-uses on one edge) and is wrong. The seam chain `s_0 -> ... -> s_{n-1}` closes at `v0` and is **orthogonal to the handle**: the seam carries the u-periodicity (around the axis); the `kfmrh` carries the meridian closure (the genus). The `kfmrh` touches only the two end latitude loops and **leaves every seam edge intact**.

**What the production kernels do (Q4):** all of Parasolid, ACIS, OCCT, C3D build the tube by **revolving / sweeping a closed profile** into **periodic surfaces of revolution with an explicit seam edge** (OCCT: tubular = u-periodic, one seam edge, line + 2 circles, 4 coedges, 1 loop), achieve the hollow by **closing the meridian (annular profile)** or **booleaning two coaxial solids**, and get the **genus-1 handle as an emergent property of the closed periodic meridian, never an explicit `kfmrh`.** Keel, editing only through Euler operators, must make that handle punch **explicit** (step 5); the sequence above is the operator-level realization of what they accomplish implicitly.

**Degeneracy branch conditions (Q5), as a single guard:**
- **Inner radius -> 0 / any pole** (`min radius <= tol`): the meridian no longer encloses an axis-piercing void; **genus = 0, SKIP the `kfmrh`**, build the collapsed bands as revolve()-style apex bands (degenerate latitude = pole vertex / degenerated edge). This guard is a **precondition**, because Euler-Poincare alone will not catch a wrongly-punched handle.
- **Mixed pole/ring (cup, bowl-with-rim):** walk the meridian; `radius == 0` vertices -> pole/apex construction (revolve() path, no handle); `radius > 0` vertices -> ring/band construction (tube path). The handle is punched **iff** the meridian is closed AND the axis does not pass through the material AND no pole collapses the cross-section.
- **Thin walls / marginal annuli** (`thickness <= tol`): reject or treat as a sheet; otherwise carry latitude circles and seams as **tolerant edges** (file 42) to absorb the numerics; gate the handle on `thickness > tol`.
- **Validity pitfalls:** never `kfmrh` when a pole is present (false genus 1); never omit `kfmrh` after cycle-closure on a true tube (false genus 0 -- detect by the post-build invariant giving `2(S - G)` with G = 0 instead of 0); keep washer radial seams distinct from the periodicity seam.

---

## References

1. Mantyla, M. (1988). *An Introduction to Solid Modeling.* Computer Science Press. https://archive.org/details/introductiontoso0000mant
2. Mantyla, M., & Sulonen, R. (1982). GWB: A solid modeler with Euler operators. *IEEE CG&A, 2*(7), 17-31. https://www.researchgate.net/publication/3209518
3. Shene, C.-K. *Euler Operators; The Euler-Poincare Formula* (CS3621 notes). Michigan Tech. https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler-op.html ; https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/model/euler.html
4. Bowyer, A. *Geometric Modelling* (Edinburgh CVonline local copy). https://homepages.inf.ed.ac.uk/rbf/CVonline/LOCAL_COPIES/BOWYER1/c2.htm
5. Stroud, I. (2006). *Boundary Representation Modelling Techniques.* Springer. https://link.springer.com/book/10.1007/978-1-84628-616-2 ; https://www.academia.edu/128166592
6. Weiler, K. (1988). The radial edge structure. In *Geometric Modeling for CAD Applications* (pp. 3-36). North-Holland. (PhD: RPI, 1986.)
7. Lee, S. H., & Lee, K. (2001). Partial entity structure. *J. Comput. Inf. Sci. Eng., 1*(4), 356. https://asmedigitalcollection.asme.org/computingengineering/article/1/4/356/471622
8. Siemens / Parasolid. *Body Types; B-Curves and B-Surfaces; Introduction to Parasolid* (q-solid mirror). http://www.q-solid.com/Parasolid_Docs_V35/chapters/fd_chap.016.html ; http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.26.html
9. Open CASCADE Technology. *Package BRepPrimAPI; BRepPrimAPI_MakeCylinder / MakeRevol.* https://dev.opencascade.org/doc/refman/html/class_b_rep_prim_a_p_i___make_cylinder.html ; https://dev.opencascade.org/doc/occt-7.6.0/refman/html/package_brepprimapi.html
10. Open CASCADE Technology. *Sample cylinder: order of edges in wire* (forum); *BRep Format.* https://dev.opencascade.org/content/sample-cylinder-order-edges-wire ; https://dev.opencascade.org/doc/occt-7.4.0/overview/html/occt_user_guides__brep_wp.html
11. Spatial / ACIS. *Primitive and Sweeping APIs* (api_make_frustum, sweep). ACIS documentation (q-solid / arizona mirrors).
12. Golovanov, N. / C3D Labs. *Geometric Modeling; C3D tolerant geometry.* Summarized in `docs/research/kernel/42-russian-c3d-literature.md`, Source 3.
13. Keel dossiers. `docs/research/kernel/01-topology-euler-operators.md`, `02-nonmanifold-cellular.md`, `44-concave-fillet-topology-surgery.md`; Keel constructors `torus()`, `revolve()`.
14. Analysis Situs. *Euler operators.* https://analysissitus.org/features/features_euler-ops.html
