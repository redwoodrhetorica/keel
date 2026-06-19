# 77. The WRONG-safe watertightness witness: a direct radial-count watertightness net plus an independent generalized-winding-number volume for safe assemble-gate recovery

## Title and scope

This dossier is the implementation-grade literature backing for request batch 0007 #77,
the **prerequisite** that makes any later boolean-assembler advance (the seam-junction
work in 0007 #76, the per-face overlay imprint) safe to land. The 73-FINDINGS series
(73, 73b, 73c, and the LOG Add 296-297 GWN addendum) proved two things that, taken
together, force this dossier to exist before any gate relaxation can be attempted:

1. Keel's `mesh_open_ratio` watertightness net returned **0 (WATERTIGHT)** on a body
   that carried **nine radial-1 (boundary) edges**, a genuinely open protrusion sheet.
   The net is a signed-area-vector sum, and the open sheet's two opposite walls had
   near-cancelling area vectors, so the sum was ~0. The ONLY check that caught that
   malformed body was the analytic-mass vs tessellation gate. A watertightness net that
   can return WATERTIGHT on an open body is a **false negative**, and it is the single
   most dangerous defect in a DECLINE-never-WRONG kernel, because a downstream
   relaxation that trusts it will admit a wrong body.

2. The dual mass-vs-tessellation gate is **load-bearing for WRONG-prevention**. Relaxing
   it to a single reference (accept when analytic mass equals the robust `mesh_volume`)
   produced WRONG = 1 on a real soak seed. A generalized-winding-number (GWN) Monte-Carlo
   volume was the **independent** third integrator that correctly adjudicated the
   mass-vs-mesh disagreement: it matched the analytic mass (5474 vs 5480, 0.12%), proving
   the green-slab mass integrator is CORRECT and the signed-tetra `mesh_volume` (6647) was
   over-reading the dangling sheet. (See 73-FINDINGS section 10.)

This dossier does NOT re-derive the mass integrator (proven correct, locked by
`tests/cyl_union_mass_witness.rs`), does NOT propose forcing mass to the mesh, and does
NOT propose relaxing the mass==mesh gate to pass malformed bodies (proven WRONG). It
specifies, with literature backing, two new mechanisms and one decision table:

- **A.** A **direct topological watertightness net** (a per-edge radial-count / half-edge
  twin-matching predicate) that detects a radial-1 edge or open sheet DIRECTLY, replacing
  the unsound signed-area-vector test as the primary watertightness signal.
- **B.** An **independent volume witness** (a GWN solid-angle Monte-Carlo or quadrature
  volume) that is independent of BOTH the analytic mass and the tessellation, so agreement
  among the three is real verification rather than one measurement reported twice.
- **C.** A **three-way adjudication table** that recovers the 9% "watertight mass==mesh
  declined on a tessellation f64 blow-up" slice to PASS ONLY when the independent witness
  confirms, without admitting a wrong body.

It builds on, and does not repeat: dossier 35 (the oracle problem, metamorphic relations,
validated numerics), 09 (winding numbers, GWN, exact mesh booleans), 47 (the shell-closure
invariant and the drop-prevention assembly), 30 (watertightness and the GWN robustness
floor), 22 (model-diff validation properties). It is a design recommendation to validate
against Keel's tests (`cyl_union_mass_witness.rs`, the realsoak WRONG=0 invariant, the
533-case suite), not a transcription of any kernel's source.

House style: no em-dashes anywhere; ranges written with "to"; "verified" vs "claimed"
labels on every load-bearing assertion.

---

## The load-bearing conclusion, stated once

> **The signed net-area-vector sum is a NECESSARY but NOT SUFFICIENT watertightness test:
> a closed surface has zero total vector area (verified, divergence theorem on a constant
> field), but the converse is false, so an open body can have a zero net area vector by
> cancellation (verified, the seed-719 nine-radial-1 protrusion). Replace it as the PRIMARY
> signal with a DIRECT predicate: build the half-edge twin pairing of the result shell under
> tolerance, and require every edge to be radial-matched (every directed half-edge has an
> oppositely-directed twin); any unmatched directed edge is a boundary edge and the body is
> NON-watertight. Keep an INDEPENDENT volume witness for the gate: a generalized-winding-number
> (Jacobson-Kavan-Sorkine 2013) solid-angle volume, evaluated by Monte-Carlo membership over
> the body AABB, which depends on NO divergence/flux reduction and so is a genuine third
> integrator alongside the analytic mass and the signed-tetra mesh. Invoke the GWN witness
> ONLY on a mass-vs-mesh disagreement (it is slow), and use the THREE-WAY table below: accept
> (drop the spurious decline) ONLY when the result is radial-watertight AND the analytic mass
> agrees with the GWN witness within the curved band; decline whenever the topological net
> fails OR mass disagrees with the independent witness. This recovers the 9% f64-blow-up slice
> (mass==mesh==gwn, tess alone blew up) WITHOUT admitting the WRONG that a pure mass==mesh
> relaxation admitted.**

---

## Part 1. THE WATERTIGHTNESS NET: a direct radial-count predicate, not a cancelling area sum

The defect is precise. Keel's current net (`mesh_open_ratio`) integrates the signed area
vector of the boundary and reports open when the magnitude exceeds a threshold. This is the
divergence-theorem identity in reverse: for a CLOSED orientable surface the total vector
area is identically zero. The contrapositive is sound (nonzero net area implies open), but the
forward direction is not: a zero net area vector does NOT imply closed. The seed-719 body is
the witness: an open tube-like protrusion whose two opposite walls have near-equal and
opposite area vectors that cancel to ~0, so the net reported WATERTIGHT on a body with nine
boundary edges. This dossier's first deliverable is to replace the primary signal with a
predicate that cannot cancel.

### 1.1 The exact predicate (the one-line net)

For a closed orientable 2-manifold (or a valid non-manifold radial-edge body) every edge
must carry a **matched, even set of incident directed half-edges**: each directed edge
`(u, v)` traversed along a face loop must be paired with an oppositely-directed twin `(v, u)`
from an adjacent face. Formally, for the multiset H of directed half-edges of the shell,
build the canonical undirected key `key = (min(u,v), max(u,v))` per half-edge (under
coordinate tolerance) and require, for every key, that the count of `(u,v)`-oriented
half-edges equals the count of `(v,u)`-oriented half-edges. The body is watertight iff every
key is balanced; any key with an imbalance carries a **boundary (radial-1) edge** and the body
is NON-watertight. For a clean 2-manifold this collapses to "every edge has exactly two
incident half-edges, one in each direction"; the radial-edge generalization allows any even
balanced count (a non-manifold edge of radial-4 is balanced 2-and-2), which is exactly Keel's
PES/radial-edge model.

This is the standard manifold/closedness criterion. CGAL's `is_valid_polygon_mesh`
encodes precisely this: each edge has at most two incident faces AND is visited in opposite
directions along the two face boundaries (verified, CGAL PMP docs). Open3D's `is_watertight`
/ `is_edge_manifold` encodes "each edge has exactly two incident faces, no boundary edges"
(verified, Open3D docs). The criterion is a topological count, not a geometric sum, so it
cannot cancel.

> Citation: CGAL Editorial Board, "Polygon Mesh Processing: Combinatorial Repair,"
> CGAL 6.x User Manual, 2024.
> https://doc.cgal.org/latest/Polygon_mesh_processing/index.html
>
> Method/Content: `is_valid_polygon_mesh` returns true iff each edge has at most two
> incident faces and is visited in opposite direction along the two face boundaries, no
> polygon repeats a vertex, and the soup describes a manifold surface. `is_closed`
> (verified) is the watertight specialization: every halfedge has a non-border opposite,
> i.e. no border edge exists. The non-manifold detection (`non_manifold_vertices`,
> manifoldness repair by vertex duplication) is the companion that distinguishes a valid
> non-manifold radial-edge join from a genuine open boundary.
>
> Limitations: a pure combinatorial closedness test does NOT detect self-intersection or
> a geometrically inverted-but-topologically-closed shell (that is the witness in Part 2).
> Tolerance handling (coordinate-merged vertices) is the implementer's responsibility; CGAL
> assumes a consistent index soup.
>
> Kernel relevance: this is the exact predicate Keel needs as its PRIMARY watertightness
> signal, generalized from radial-2 (manifold) to balanced-even (PES radial-edge). It is a
> direct count, not a cancelling sum, so it catches the seed-719 radial-1 sheet that
> `mesh_open_ratio` false-negatived.

> Citation: Open3D, "Mesh" tutorial and `TriangleMesh.is_watertight` / `is_edge_manifold`,
> Open3D documentation, 2024.
> https://www.open3d.org/docs/latest/tutorial/Basic/mesh.html
>
> Method/Content (verified): a mesh is edge-manifold if each edge bounds one or two
> triangles; it is watertight if it is edge-manifold, vertex-manifold, and has no boundary
> edges (each edge has exactly two incident faces) and no self-intersection. The boundary
> test is `get_non_manifold_edges(allow_boundary_edges=False)`: any edge whose incident
> triangle count differs from two is returned.
>
> Limitations: Open3D's watertight bundles a self-intersection test that is O(n^2) naive;
> Keel should keep the topological net cheap (the radial count) and reserve self-intersection /
> inversion checks for the independent witness.
>
> Kernel relevance: confirms the production-standard decomposition: edge-manifold +
> no-boundary-edge is the topological half, an inside-outside / volume witness is the
> geometric half. Keel currently has only a (broken) geometric half; this dossier adds the
> topological half as primary and fixes the geometric half to be independent.

### 1.2 Why the net-area-vector sum is unsound (necessary, not sufficient)

The identity behind the current test: applying the divergence theorem to a constant vector
field `F = c` over a closed surface S gives `integral_S (c . n) dA = integral_V div(c) dV = 0`
for all c, hence the total vector area `integral_S n dA = 0` for any closed surface (verified,
standard vector calculus; the total vector area of a closed surface is zero). This makes a
nonzero net area a SUFFICIENT condition for openness (the contrapositive). It does not make a
zero net area sufficient for closedness, because the boundary of an open surface can still have
a zero net area vector when its boundary-bounded area contributions cancel. The seed-719
protrusion is exactly this degenerate cancellation. The lesson generalizes to all
flux/divergence summaries: a single signed integral that can cancel is a NECESSARY-not-sufficient
witness, and the kernel's DECLINE-never-WRONG floor must never rest its watertightness decision
on one. (This is the same necessary-not-sufficient trap dossier 35 catalogues for single-oracle
validation, and dossier 22 for single-metric model diff.)

> Citation: D. Stutz, "A Formal Definition of Watertight Meshes," 2018.
> https://davidstutz.de/a-formal-definition-of-watertight-meshes/
>
> Method/Content (claimed, well-reasoned): watertightness is defined combinatorially as
> a mesh whose underlying surface is a closed 2-manifold without boundary, i.e. every edge is
> shared by exactly two faces with consistent orientation. The note distinguishes
> "intersection-free" from "watertight": a mesh can be watertight (no boundary edges) yet
> self-intersecting, and vice versa, so neither implies the other. This is the formal warrant
> for splitting Keel's net into a topological half (Part 1) and a geometric half (Part 2).
>
> Limitations: a definition note, not an algorithm; it does not address tolerance or the
> non-manifold (radial > 2) generalization Keel needs.
>
> Kernel relevance: pins the exact gap in the old `mesh_open_ratio`: it conflated "zero net
> flux" with "closed," which is neither the combinatorial closedness above nor intersection-freeness.

### 1.3 The robust construction (half-edge twin matching under tolerance, boundary-loop extraction)

The construction has three steps, each with a literature anchor.

1. **Tolerant vertex identification.** Snap-merge vertices within Keel's working tolerance so
   that two coedges that meet at a numerically-coincident point share an index. Half-edge twin
   matching is only as good as the vertex identification; the standard pitfall (dossier 30,
   snapping section) is that a too-loose merge welds distinct features and a too-tight merge
   leaves a phantom crack. Keel already has the per-face canonical seam dedup
   (`imprint_operand`, the [[tilted-cut-boolean-bug]] LAYER 1 fix); reuse that canonicalization
   for the net's vertex keys so the net agrees with the assembler's own coincidence decisions.

2. **Twin pairing.** For each directed half-edge, look up its reverse under the canonical key.
   A clean interior edge finds exactly one oppositely-directed twin (radial-2); a non-manifold
   edge finds an even balanced set (radial-2k); a boundary edge finds an imbalance. ManifoldPlus
   and the half-edge literature describe the duplicate-into-four-half-edges handling for genuine
   non-manifold voxel edges, which is the construction Keel's radial-edge model already implements
   in its coedge arena.

3. **Boundary-loop extraction (the diagnostic).** When the net fails, walk the unmatched directed
   half-edges into boundary loops: provided the mesh is otherwise manifold, boundary edges form
   cycles and each vertex is incident on exactly zero or two boundary edges (verified, the
   boundary-cycle property). Reporting the boundary loops (the seed-719 nine-edge loop around the
   protrusion) gives the assembler an actionable defect locus, not just a boolean.

> Citation: J. Huang, Y. Zhou, M. Niessner, J. R. Shewchuk, L. Guibas, "ManifoldPlus: A Robust
> and Scalable Watertight Manifold Surface Generation," arXiv:2005.11621, 2020.
> https://arxiv.org/pdf/2005.11621
>
> Method/Content: constructs a watertight manifold by an octree with explicit half-edge
> bookkeeping; non-manifold edges are duplicated as four half-edges with each same-cell pair
> marked as twins (verified). The watertightness loss / criterion penalizes any edge whose
> incident-half-edge count is other than the matched even count. The pipeline demonstrates the
> twin-matching construction at scale.
>
> Limitations: it is a remeshing-to-watertight method (it changes geometry to force closure),
> which Keel must NOT do at the gate; Keel uses only the half-edge twin-matching PREDICATE, not
> the remeshing. The octree is overkill for a single boolean result shell.
>
> Kernel relevance: the duplicate-into-four-twins handling is exactly the non-manifold
> radial-edge balance the net must respect so a legitimate radial-4 join is not mis-flagged as open.

> Citation: M. Botsch, L. Kobbelt, M. Pauly, P. Alliez, B. Levy, "Polygon Mesh Processing,"
> A K Peters / CRC Press, 2010 (half-edge data structure, boundary detection).
> (Reference text; halfedge `is_border`, `opposite`, `next`/`prev` traversal.)
>
> Method/Content: the half-edge structure pairs each directed edge with an `opposite`;
> `is_border(h)` is true when `h` (or its opposite) has no incident face. Boundary loops are
> extracted by following `next` across border half-edges. Provided the mesh is manifold,
> boundary edges form closed cycles and a vertex is incident on zero or two boundary edges
> (verified).
>
> Limitations: the classic half-edge structure is strictly manifold (radial-2); Keel's radial
> edge generalizes `opposite` to a radial ring, so "border" becomes "imbalanced radial ring."
>
> Kernel relevance: the canonical reference for the twin pairing and boundary-loop walk Keel's
> net needs; the radial generalization is the only delta.

---

## Part 2. THE INDEPENDENT WITNESS: a generalized-winding-number volume, independent of mass and mesh

The topological net (Part 1) catches an open shell. It does NOT catch a body that is
topologically closed yet geometrically wrong: a self-intersecting or inverted-but-closed shell
whose analytic mass and signed-tetra mesh volume can AGREE on an impossible value (the #48
self-consistent-WRONG phenomenon the gate's own comments cite). For that the gate needs a
volume integrator that is independent of BOTH existing measures. The 73-FINDINGS GWN addendum
already proved this works: the generalized winding number is the third integrator.

### 2.1 Why GWN is genuinely independent

The analytic mass (`integrate_face_green`) and the tessellated volumes (`tessellated_volume`,
`mesh_volume`) are both **divergence/flux reductions**: they reduce a volume to a signed sum of
boundary contributions (a boundary x-flux for the mass, signed tetrahedra for the mesh). Two flux
sums over the same triangles are one measurement reported twice; their agreement is not
verification (73-FINDINGS section 10.1). The generalized winding number is a DIFFERENT object: it
is the signed solid angle the boundary subtends at a point, summed per triangle by the
Van Oosterom-Strackee formula, with NO divergence reduction. A volume computed as the measure of
the set `{ x : w(x) >= 0.5 }` therefore tests membership pointwise and is independent of any flux
sum. Agreement of analytic-mass, mesh, and GWN is three independent integrators agreeing, which is
real verification.

> Citation: A. Jacobson, L. Kavan, O. Sorkine-Hornung, "Robust Inside-Outside Segmentation Using
> Generalized Winding Numbers," ACM Transactions on Graphics 32(4), SIGGRAPH 2013 (Test-of-Time
> Award 2024).
> https://igl.ethz.ch/projects/winding-number/
>
> Method/Content (verified): the generalized winding number at a point p is
> `w(p) = (1 / 4*pi) * sum_t solid_angle(t, p)`, where the per-triangle signed solid angle
> `Omega` is the Van Oosterom-Strackee closed form
> `tan(Omega/2) = (a . (b x c)) / (|a||b||c| + (a.b)|c| + (b.c)|a| + (c.a)|b|)`, with a, b, c the
> triangle vertices relative to p. For a watertight mesh w is the exact integer winding number
> (1 inside, 0 outside); for a defective mesh w is a smooth harmonic field that is "well-behaved
> otherwise," and the inside set is `{ w >= 0.5 }`. This is the canonical defect-tolerant
> inside-outside test.
>
> Limitations: w is exact only off the surface; on the surface it is the half-integer ambiguous
> value, so a volume estimator must avoid sampling on faces (a non-issue for random AABB samples,
> measure zero). The naive per-point cost is O(#triangles); see 2.3 for when to pay it.
>
> Kernel relevance: this is the independent witness. The 73-FINDINGS GWN addendum used exactly
> this (`winding.rs::gwn_over`, the Van Oosterom-Strackee solid-angle sum) and it adjudicated
> seed-719 correctly (gwn 5474 tracked analytic mass 5480, not mesh 6647), proving the mass
> integrator correct.

### 2.2 The GWN volume formula, tolerance, variance, and cost

**Monte-Carlo membership volume.** Sample N points uniformly in the body AABB of volume
`V_box`. For each sample x compute `w(x)`; count `k = #{ x : w(x) >= 0.5 }`. The volume estimate
is `V_hat = V_box * (k / N)`. This is a Bernoulli-proportion estimator with `p = V_true / V_box`.

- **Variance and tolerance.** `Var(V_hat) = V_box^2 * p(1-p) / N`, so the standard error is
  `SE = V_box * sqrt(p(1-p)/N)`. The relative error on the volume scales as
  `SE / V_true = sqrt((1-p)/(p*N))`, i.e. O(N^{-1/2}) (verified, Monte-Carlo convergence is
  O(N^{-1/2}) independent of dimension). To adjudicate a mass-vs-mesh gap of, say, 18% the
  witness needs SE well under that gap: for the seed-719 AABB fill fraction p the addendum's
  400k samples gave 0.12% agreement, comfortably tight. A practical rule: choose N so that
  `SE/V_true <= band/3` where `band` is the curved gate tolerance, then the witness resolves the
  disagreement with margin. Tighten the AABB to the body's actual bounds (not the world box) to
  keep p large and N small, the membership analog of the per-component recentering `mesh_volume`
  already does (LOG Add 264).
- **Cost.** Naive cost is `O(N * #triangles)`. This is the reason the witness is a
  LAST-RESORT adjudicator, not a per-op check: on a long op-chain it is far too slow to run every
  boolean. Invoke it ONLY when the cheap signals already disagree (Part 3 gates it on a
  mass-vs-mesh disagreement). For acceleration when N must be large, use the Barill et al. 2018
  hierarchical (Barnes-Hut) GWN to reduce per-point cost to roughly O(log #triangles).

> Citation: A. Van Oosterom, J. Strackee, "The Solid Angle of a Plane Triangle," IEEE
> Transactions on Biomedical Engineering BME-30(2), 1983, 125-126.
> https://pubmed.ncbi.nlm.nih.gov/6832789/
>
> Method/Content (verified): the closed-form signed solid angle of a plane triangle at a point,
> the formula in 2.1. Numerically stable via `atan2` of the numerator/denominator pair (avoids
> the division blow-up when the denominator is near zero). This is the per-triangle kernel of the
> GWN sum.
>
> Limitations: assumes the triangle is non-degenerate at p; a sample landing on the supporting
> plane is the measure-zero exception handled by re-sampling.
>
> Kernel relevance: the exact per-triangle term Keel's `gwn_over` already uses; cited here as the
> independence warrant (solid angle, not flux).

> Citation: G. Barill, N. Dickson, R. Schmidt, D. I. W. Levin, A. Jacobson, "Fast Winding Numbers
> for Soups and Clouds," ACM Transactions on Graphics 37(4), SIGGRAPH 2018.
> https://www.dgp.toronto.edu/projects/fast-winding-numbers/fast-winding-numbers-for-soups-and-clouds-siggraph-2018-compressed-barill-et-al.pdf
>
> Method/Content (verified): a boundary-element-style hierarchical (Barnes-Hut) approximation of
> the generalized winding number; a tree of triangle clusters with a multipole-like far-field
> expansion drops per-point cost from O(#triangles) to roughly O(log #triangles), reported up to
> 1000x faster average evaluation. Works on triangle soups, point clouds, and defective meshes.
>
> Limitations: the approximation has a controllable error tolerance (the opening-angle / beta
> parameter); for an adjudication witness Keel should set it tight or fall back to exact summation
> on the small result shells typical of a single boolean, where exact is already fast enough.
>
> Kernel relevance: the accelerator IF the witness ever needs large N on a large result; for the
> single-boolean shells in the 9% slice, exact summation at modest N is sufficient and simpler.

> Citation: PBR-Book, "Monte Carlo Integration: Basics," and CMU 10-701 Monte Carlo notes (variance
> and O(N^{-1/2}) convergence).
> https://pbr-book.org/4ed/Monte_Carlo_Integration/Monte_Carlo_Basics
>
> Method/Content (verified): the Monte-Carlo estimator error decreases as O(N^{-1/2}) independent
> of dimension; variance is `sigma^2 / N`. For a membership (indicator) integrand the variance is the
> Bernoulli `p(1-p)`, giving the SE in 2.2. Stratified / quasi-Monte-Carlo sampling lowers the
> constant but not the rate.
>
> Limitations: rejection/membership sampling is inefficient when p is tiny (the curse of
> dimensionality for high-D, not an issue for a 3D AABB tightened to the body); quasi-random points
> do not always help indicator integrands. Keep the AABB tight to keep p large.
>
> Kernel relevance: gives the exact N-vs-tolerance budget for sizing the witness so it resolves the
> gate band with margin.

### 2.3 The alternative independent witness: a tight per-fragment op-volume bound

Where GWN is too slow even as a last resort, a cheaper independent signal is a per-fragment
op-volume bound, the third reference the assembler gate already carries (73-FINDINGS section 1).
For a regularized boolean `A op B`, the result volume is bounded by interval arithmetic on the
operand volumes: `vol(A and B) <= min(vol A, vol B)`, `vol(A or B) <= vol A + vol B`,
`vol(A - B) in [vol A - vol B, vol A]` (with the obvious clamps). These bounds are independent of
the result's own tessellation and mass because they are computed from the OPERANDS. They are
looser than GWN (a bound, not a value) but O(1) and so usable as a cheap first independent gate;
the WRONG that the 73-FINDINGS fix attempt admitted was exactly a downstream fillet whose mesh
exceeded such an op-volume bound (`mesh 116378 > 110369 + slack`), so this bound is already a
proven WRONG-catcher. Use the op-volume bound as the cheap independent guard on every op, and
escalate to the GWN value-witness only when the bound is satisfied but mass and mesh still disagree.

> Citation: dossier 35 (this series), "Verification, Validated Numerics, and Oracles," section on
> interval arithmetic and metamorphic relations; and Tucker, "Validated Numerics," Princeton, 2011.
>
> Method/Content: interval / affine arithmetic produces guaranteed enclosures; a metamorphic
> relation (here the set-operation volume inequalities) is an oracle that needs no ground truth. The
> op-volume inequalities are metamorphic relations on the boolean: they must hold for ANY correct
> result regardless of the specific bodies.
>
> Limitations: a bound rejects gross errors but cannot confirm a value to the band; it is a
> screen, not the adjudicator.
>
> Kernel relevance: the cheap, always-on independent guard; the GWN value-witness is the precise
> adjudicator reserved for the residual disagreement.

---

## Part 3. SAFE GATE RECOVERY: the three-way adjudication table

With the topological net (Part 1) and the independent witness (Part 2) in place, the gate can
recover the 9% "watertight mass==mesh declined on a tessellation f64 blow-up" slice without
admitting a wrong body. The recovery is a STRICT rule: accept a self-consistent result and drop a
now-spurious decline ONLY when the independent witness confirms.

### 3.1 The three measures

- **M (analytic mass):** `mass_properties` green-slab boundary x-flux (verified correct, locked
  by `tests/cyl_union_mass_witness.rs`). Trusted integrator.
- **T (tessellated/mesh volume):** the signed-tetra divergence sum (`tessellated_volume`
  single-reference, `mesh_volume` per-component recentered). The same family of flux sum, the one
  that can blow up under f64 cancellation (the seed where `tess 2618` vs `mass/mesh ~460`) or
  over-read a malformed sheet (seed-719 `mesh 6647`).
- **W (independent witness):** the GWN solid-angle Monte-Carlo volume (Part 2), independent of
  M and T; plus the cheap op-volume bound as the always-on screen.
- **Net (topological watertightness):** the radial-count predicate (Part 1), a boolean that is
  the precondition for any acceptance.

### 3.2 The accept / decline decision table

Order of evaluation: topological net first (cheapest and a hard precondition), then the cheap
op-volume bound, then M-vs-T agreement, and the GWN value-witness ONLY when M and T disagree.

| Net (radial-watertight) | op-volume bound | M vs T | GWN witness W | Decision | Rationale |
|---|---|---|---|---|---|
| FAIL (radial-1 edge) | -- | -- | -- | **DECLINE** | open body, caught DIRECTLY (seed-719), not via a cancelling sum |
| PASS | violated | -- | -- | **DECLINE** | result volume outside operand bound, a proven WRONG-catcher |
| PASS | ok | agree (<= band) | not invoked | **ACCEPT** | two integrators of different family agree on a closed body; the common case |
| PASS | ok | disagree (T blew up) | W == M (<= band) | **ACCEPT (recover)** | the 9% slice: tess f64-blew-up, but the INDEPENDENT witness confirms mass; drop the spurious decline |
| PASS | ok | disagree | W == T (<= band) | **DECLINE** | the genuine green-slab class: the witness sides with the mesh, the analytic mass mis-integrates; dossier-71 territory, stays declined |
| PASS | ok | M == T but W disagrees | -- | **DECLINE** | the #48 self-consistent-WRONG: mass and mesh agree on an impossible value, the independent witness catches it |
| PASS | ok | disagree | W matches neither | **DECLINE** | unresolved three-way disagreement, a malformed body, decline by the floor |

The single new ACCEPT path (row 4) is the recovery: it fires ONLY when the topological net
passes AND the op-volume bound holds AND the independent GWN witness confirms the analytic mass.
The row that the 73-FINDINGS fix attempt got wrong was admitting on `M == T` alone (it skipped the
independent witness, so it could not see the downstream malformed fillet). Here `M == T` alone is
NOT sufficient: row 6 declines an `M == T` body when the independent witness disagrees, which is
exactly the WRONG the naive relaxation admitted. The dual reference is preserved AND extended: the
gate now has three independent integrators, and acceptance requires the independent witness, never
two flux sums alone.

### 3.3 Why this is WRONG-safe where the prior attempt was not

The 73-FINDINGS fix accepted when mass agreed with the robust `mesh_volume`, which is a SECOND
flux sum, so it admitted a body whose two flux sums coincidentally agreed (the differently-grown
union that drove a downstream fillet out of bound, WRONG = 1). This table never accepts on two
flux sums: row 4 requires the GWN witness (a non-flux integrator) to confirm, and row 6 declines
even a mass==mesh body when the GWN witness dissents. The independence of W from both M and T is
the whole safety argument; it is the same independence the seed-719 addendum demonstrated
empirically (W tracked M, not T, on a malformed body).

---

## Part 4. VALIDATION: the four claims to discharge against Keel's tests

The recommendation is to validate, in order, against Keel's existing harness:

1. **The radial-1 open body is declined by the watertightness net DIRECTLY.** Replay seed
   11400715918834826719 (the nine-radial-1 protrusion). The new net must report NON-watertight
   (boundary loop of nine edges) and DECLINE on the topological signal, not only via `mass != mesh`.
   The witness `tests/cyl_union_mass_witness.rs` already locks that the mass integrator is exact on
   the cylinder bands; the new test asserts the net flags the open sheet. Verified target: the
   decline is now attributed to watertightness (the assembler frontier), where the real fix lives,
   not mis-attributed to massprops.

2. **The f64-blow-up slice recovers to PASS.** The pure `tessellated_volume` blow-up cases
   (the ~8 per 3717 gate-declines, `open <= 0.01`, `M == mesh`, `tess` off > 25%, e.g. the
   `mesh 459.84, tess 2618.08, mass 461.32` body) must ACCEPT via row 4: net passes, op-volume bound
   holds, M and mesh agree, T blew up, and the GWN witness confirms M. Verified target: this 9%
   slice (in the 8.9% watertight `mass==mesh` bucket of 73-FINDINGS section 4.1) recovers without a
   WRONG.

3. **A battery of known-wrong bodies stays declined.** Re-run the soak seed that the prior fix
   broke, seed 11400715918834829047, and confirm row 6 (or the op-volume bound, row 2) declines the
   differently-grown union / downstream fillet so the WRONG does NOT reappear. Add the #48
   self-consistent-WRONG sphere/sphere lens cases as explicit known-wrong fixtures: M == T but the
   GWN witness dissents, decline. Verified target: every body the dual reference used to catch stays
   caught.

4. **realsoak WRONG = 0.** The full 3000-project soak (the 73-FINDINGS baseline, PASS 2361/3000,
   WRONG 0) must hold WRONG = 0 with the new net and witness, and PASS must not regress below
   baseline (it should rise by the recovered f64-blow-up slice). Plus the 533-case suite green,
   `cargo fmt --check`, `cargo clippy -D warnings`. Verified target: WRONG 0, PASS >= baseline.

---

## PATENT AWARENESS

- **Generalized winding number (Jacobson-Kavan-Sorkine 2013; Barill et al. 2018; Zhou et al. 2016).**
  These are academic publications with open reference implementations in libigl (MPL/MIT-style).
  No blocking patent on the GWN inside-outside test or the solid-angle volume is known to the
  author (claimed; not a legal opinion). The Van Oosterom-Strackee solid-angle formula (1983) is
  long expired and in the public domain (verified by age). Keel already uses the GWN internally,
  so this dossier adds no new exposure; the witness is the same `gwn_over` already in `winding.rs`.
- **Mesh-arrangement booleans (Zhou 2016, Cherchi 2020, Attene 2020 indirect predicates).** These
  are post-2006 methods with active research code (often GPL/LGPL for the implementations, e.g.
  Cinolib, Indirect_Predicates). Keel uses NONE of their CODE; it uses only the published PREDICATE
  and the watertightness criterion, which are mathematical facts, not patentable expressions. The
  recommended design-around posture: cite the criterion, implement the radial-count net and the GWN
  witness from the public formulas, and keep clear of any specific exact-arithmetic library license.
  Prefer the expired/no-patent Van Oosterom-Strackee closed form and the public divergence-theorem
  identity as the load-bearing math.
- **CGAL / Open3D closedness predicates.** Used here only as corroborating references for the
  textbook criterion (every edge balanced/radial-2). The criterion itself is not ownable; Keel
  implements it natively over its PES arena. CGAL is GPL/commercial-dual; do not link it. Open3D is
  MIT; usable but unnecessary.

No claim-reading of any specific patent was performed; this section flags the landscape and the
design-around, consistent with the project's [[inspired-not-copied]] and patent-fence posture.

---

## ADOPT / AVOID / INVESTIGATE

**ADOPT (verified, low risk):**
- The **direct radial-count watertightness net** (Part 1.1) as the PRIMARY watertightness signal,
  replacing `mesh_open_ratio` as the load-bearing test. Keep the area-vector sum only as a cheap
  corroborating screen, never as the sole signal. This directly fixes the seed-719 false negative.
- The **Van Oosterom-Strackee GWN Monte-Carlo volume** as the independent witness, reusing the
  existing `gwn_over`. It is already proven to adjudicate seed-719 correctly.
- The **op-volume bound** as the always-on cheap independent screen (already present; it caught the
  prior WRONG).
- The **three-way adjudication table** (Part 3.2), with acceptance gated on the independent witness,
  never on two flux sums.

**AVOID:**
- Resting watertightness on any single signed/flux integral that can cancel (the original defect).
- Accepting a self-consistent result on `M == T` alone (the proven WRONG = 1 path).
- Forcing the analytic mass to the mesh, or relaxing the mass==mesh gate to pass malformed bodies
  (both proven WRONG / explicitly out of scope).
- Remeshing-to-watertight (ManifoldPlus style) at the gate: the gate must DECLINE an open body, not
  silently repair it into a different one.

**INVESTIGATE:**
- The **Barill 2018 hierarchical GWN** if any witness invocation ever needs large N on a large
  result shell; for the single-boolean shells in the 9% slice, exact summation at modest N is
  enough, so this is a later optimization, not a milestone.
- **Deterministic quadrature** (stratified or a low-discrepancy sequence) for the membership
  volume to lower the variance constant and make the witness reproducible run-to-run (a
  determinism property dossier 14 cares about); a fixed-seed Monte-Carlo is the simpler first cut.
- Whether the **green-slab `mass != mesh` remainder** (the genuine dossier-71 class the witness
  sends to row 5) is worth a per-face flux witness landing in `massprops.rs`; that is dossier 71's
  target, surfaced but not owned here.

## Recommended FIRST MILESTONE (narrowest honest slice with an exact oracle)

**The all-planar radial-count net, validated on seed-719's planar protrusion analog and the
all-planar f64-blow-up cases, with an EXACT oracle (no chordal slack).** Specifically:

1. Implement the radial-count predicate over the result shell's coedge arena (reuse the
   `imprint_operand` canonical vertex keys). On an ALL-PLANAR result the watertightness decision is
   exact: every edge is radial-balanced or it is not, no tolerance band. Oracle: the body is
   watertight iff every edge key is balanced. Assert the net DECLINES a hand-built planar open sheet
   (a box missing one face) and PASSES a closed planar box, exactly.
2. On the all-planar f64-blow-up cases (where the result is genuinely a closed polyhedron and the
   true volume is computable EXACTLY by the signed-tetra sum in rationals), the recovery row 4 must
   ACCEPT, and the exact rational volume is the oracle that confirms M and the GWN witness.

This slice is the narrowest because the all-planar case has an EXACT volume oracle (rational
signed-tetra) and an EXACT watertightness oracle (integer radial counts), so neither the net nor the
witness needs a tolerance band. It directly fixes the false negative and recovers the planar share of
the 9% slice, with WRONG-safety provable rather than soak-estimated.

## Follow-up ladder

1. **(Milestone)** All-planar radial-count net + exact-oracle recovery (above).
2. Extend the net to curved bodies (the radial count is unchanged; only the vertex tolerance keying
   needs the curved-seam canonical identity from dossier 65). Add the GWN witness with the
   N-vs-band budget (Part 2.2).
3. Wire the full three-way table into `assemble_boolean`, gate the GWN witness on a mass-vs-mesh
   disagreement, validate realsoak WRONG = 0 and PASS >= baseline.
4. (Optional) Barill hierarchical GWN if witness cost becomes a soak bottleneck; deterministic
   quadrature for run-to-run reproducibility.
5. Hand the row-5 genuine green-slab remainder to dossier 71's per-face mass witness.

## DECLINE-forever set (bodies this witness will, correctly, never accept)

- Any body with a genuine radial-1 (boundary) edge: an open shell. Declined by the topological net.
  This is correct; an open body has no well-defined enclosed volume.
- Any body where the independent GWN witness matches neither M nor T (three-way disagreement): a
  malformed body the floor cannot adjudicate. Declined.
- Any body whose result volume violates the operand op-volume bound: a gross boolean error. Declined.
- Self-intersecting-but-closed bodies whose GWN field is multivalued beyond the band: the witness
  itself is ambiguous, so the floor declines rather than guess. (These are dossier 30 / 47 repair
  candidates, not gate-recovery candidates.)

These are not failures of the witness; they are the DECLINE-never-WRONG floor doing exactly its job
on bodies that have no honest PASS.

---

## References

1. A. Jacobson, L. Kavan, O. Sorkine-Hornung. "Robust Inside-Outside Segmentation Using Generalized
   Winding Numbers." ACM TOG 32(4), SIGGRAPH 2013 (Test-of-Time Award 2024).
   https://igl.ethz.ch/projects/winding-number/
2. A. Van Oosterom, J. Strackee. "The Solid Angle of a Plane Triangle." IEEE Trans. Biomed. Eng.
   BME-30(2), 1983, 125-126. https://pubmed.ncbi.nlm.nih.gov/6832789/
3. G. Barill, N. Dickson, R. Schmidt, D. I. W. Levin, A. Jacobson. "Fast Winding Numbers for Soups
   and Clouds." ACM TOG 37(4), SIGGRAPH 2018.
   https://www.dgp.toronto.edu/projects/fast-winding-numbers/fast-winding-numbers-for-soups-and-clouds-siggraph-2018-compressed-barill-et-al.pdf
4. Q. Zhou, E. Grinspun, D. Zorin, A. Jacobson. "Mesh Arrangements for Solid Geometry." ACM TOG
   35(4), SIGGRAPH 2016. http://www.cs.columbia.edu/cg/mesh-arrangements/
5. G. Cherchi, M. Livesu, R. Scateni, M. Attene. "Fast and Robust Mesh Arrangements using
   Floating-point Arithmetic." ACM TOG 39(6), SIGGRAPH Asia 2020.
   https://www.gianmarcocherchi.com/pdf/mesh_arrangement.pdf
6. G. Cherchi, F. Pellacini, M. Livesu, M. Attene. "Interactive and Robust Mesh Booleans." ACM TOG
   41(6), SIGGRAPH Asia 2022. https://arxiv.org/pdf/2205.14151
7. M. Attene. "Indirect Predicates for Geometric Constructions." Computer-Aided Design 126, 2020.
   https://arxiv.org/abs/2105.09772
8. CGAL Editorial Board. "Polygon Mesh Processing: Combinatorial Repair / is_valid_polygon_mesh /
   is_closed." CGAL 6.x User Manual, 2024.
   https://doc.cgal.org/latest/Polygon_mesh_processing/index.html
9. Open3D. "Mesh" tutorial: `is_watertight`, `is_edge_manifold`, `get_non_manifold_edges`. Open3D
   docs, 2024. https://www.open3d.org/docs/latest/tutorial/Basic/mesh.html
10. J. Huang, Y. Zhou, M. Niessner, J. R. Shewchuk, L. Guibas. "ManifoldPlus: A Robust and Scalable
    Watertight Manifold Surface Generation." arXiv:2005.11621, 2020. https://arxiv.org/pdf/2005.11621
11. M. Botsch, L. Kobbelt, M. Pauly, P. Alliez, B. Levy. "Polygon Mesh Processing." A K Peters / CRC
    Press, 2010 (half-edge structure, boundary-loop extraction).
12. D. Stutz. "A Formal Definition of Watertight Meshes." 2018.
    https://davidstutz.de/a-formal-definition-of-watertight-meshes/
13. A. Jacobson et al. "Boolean Operations using Generalized Winding Numbers." CoRR
    arXiv:1601.07953, 2016. https://arxiv.org/pdf/1601.07953
14. Matt Pharr, Wenzel Jakob, Greg Humphreys. "Physically Based Rendering" (Monte Carlo Integration:
    Basics; O(N^{-1/2}) convergence, indicator variance).
    https://pbr-book.org/4ed/Monte_Carlo_Integration/Monte_Carlo_Basics
15. CMU 10-701, "Monte Carlo Methods" (rejection / membership sampling, variance).
    https://www.cs.cmu.edu/~epxing/Class/10701-08s/recitation/mcm.pdf
16. W. Tucker. "Validated Numerics: A Short Introduction to Rigorous Computations." Princeton
    University Press, 2011 (interval enclosures; the op-volume bound as a metamorphic relation).
17. Kitware. "Polyhedron Processing Improvements in VTK" (divergence-theorem cell volume; total
    vector area of a closed surface is zero). https://www.kitware.com/polyhedron-processing-improvements-in-vtk/
18. A. Jacobson. "Solid angle at mesh vertices" (signed solid angle = generalized winding number;
    total vector area of a closed surface is zero). https://www.alecjacobson.com/weblog/?p=4538
19. M. Attene. "Polygon Mesh Repairing: An Application Perspective." ACM Computing Surveys 45(2),
    2013 (the watertight-vs-intersection-free distinction; repair taxonomy).
    https://www.researchgate.net/publication/262175346_Polygon_Mesh_Repairing_An_Application_Perspective
20. M. Diazzi, D. Panozzo, A. Vaxman, M. Attene. "Convex Polyhedral Meshing for Robust Solid
    Modeling." arXiv:2109.14434, 2021 (exact closedness and inside-outside in a solid-modeling
    pipeline). https://arxiv.org/pdf/2109.14434

---

## Related Keel dossiers (build on, not repeated)

- **09. Mesh / implicit hybrid** -- winding numbers, GWN, defect-tolerant mesh booleans. This
  dossier reuses 09's GWN as the INDEPENDENT witness and does not re-cover the GWN field theory.
- **35. Verification, validated numerics, oracles** -- the oracle problem, metamorphic relations,
  the necessary-not-sufficient single-oracle trap. The op-volume bound here is one of 35's
  metamorphic relations; the three-way table is a multi-oracle agreement in 35's sense.
- **47. Boolean result assembly / shell-closure invariant** -- the drop-prevention invariant and
  the import-and-glue stitch whose `unmatched coedge` failure the topological net here detects
  positively. The net is the runtime check of 47's invariant.
- **30. Imperfect B-rep defect handling** -- watertightness as a defect class and GWN as the
  robustness floor; the tolerant snapping pitfalls the net's vertex keying must respect.
- **22. Model diff / comparison** -- the validation-property framing; the three-way agreement is a
  diff-style multi-measure consistency check.
- **71. Post-fillet curved mass** -- the genuine green-slab `mass != mesh` remainder the witness
  routes to (row 5), the dossier-71 target proper, surfaced here but not owned.
- **73 / 73b / 73c (FINDINGS)** -- the instrumented attribution that PROVED both motivating facts:
  the `mesh_open_ratio` false negative on seed-719's nine radial-1 edges, and the WRONG = 1 from a
  single-reference relaxation. This dossier is the WRONG-safe witness those findings called for
  (73-FINDINGS section 7 item 3, section 10.3 lead 2).
- **LOG Add 288, 296, 297** -- the dual-reference load-bearing lesson and the GWN adjudication of
  seed-719 (mass correct, mesh over-read), the empirical ground this dossier formalizes.
