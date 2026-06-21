# Keel Architecture Overview

A technical orientation for an engineer onboarding to the kernel: how it is
built, the key data structures, the operation pipelines, and the core
algorithms.

This document goes one level deeper than the crate summary in
[`README.md`](../README.md) (read its "Architecture" section first for the
high-level crate roles). It complements rather than repeats that prose:
expect data-model detail, pipeline stages, and algorithm sketches here. For
the operation surface and status legend see the README capabilities tables
and [`docs/PARITY_OPS.md`](PARITY_OPS.md); for the running engineering
narrative and the per-decision rationale see
[`docs/ENGINEERING-LOG.md`](ENGINEERING-LOG.md).

Every type, function, and module named below was verified against the source
in this tree.

---

## 1. Crate layout and dependency direction

Four crates, strictly layered. Dependencies point one way only; nothing lower
knows about anything higher.

```
keel-math   (no kernel deps)        numeric + predicate foundation
    |
    v
keel-geom   (depends on keel-math)  curves + surfaces
    |
    v
keel-topo   (depends on geom+math)  B-rep topology, operations, integrators
    |
    v
keel-wasm   (depends on keel-topo)  thin WebAssembly binding
```

What each crate owns:

- **`keel-math`** -- vectors / matrices / transforms (`vec.rs`, `mat.rs`,
  `transform.rs`), AABBs (`bbox.rs`), the single tolerance home
  (`tolerance.rs`), exact predicates (`predicates.rs`), outward-rounded
  intervals (`interval.rs`), polynomial root solvers (`poly.rs`,
  `bernstein.rs`, `multibernstein.rs`, `newton.rs`), and the exact algebraic
  one-root layer (`algebraic.rs`, `bigint.rs`).
- **`keel-geom`** -- the curve and surface dispatch enums and their geometry:
  NURBS (`nurbs_curve.rs`, `nurbs_surface.rs`, `basis.rs`, `knots.rs`),
  analytic curves and surfaces (`curve.rs`, `surface.rs`), differential
  geometry (in `surface.rs`), closest-point projection (`project.rs`),
  surface-surface intersection (`ssi.rs`, `intersect.rs`), and canonical
  recovery / fitting (`recover.rs`, `fit.rs`).
- **`keel-topo`** -- the topology data model (`entity.rs`, `body.rs`,
  `arena.rs`), Euler operators (`euler.rs`), the boolean pipeline
  (`boolean.rs`, `imprint.rs`, `coincident.rs`, `assembly.rs`), blends and
  chamfers (`blend.rs`, `chamfer.rs`), local edits (`tweak.rs`, `ops.rs`,
  `heal.rs`, `simplify.rs`, `shell.rs`), construction (`construct.rs`,
  `profile.rs`, `sheet.rs`), the mass integrator (`massprops.rs`), winding
  classification (`winding.rs`, `pmc.rs`), tessellation (`tessellate.rs`,
  `render.rs`), interrogation (`interrogate.rs`, `query.rs`), validation
  (`validate.rs`), lineage and sessions (`lineage.rs`, `session.rs`), and
  STEP I/O (`step_export.rs`, `step_import.rs`).
- **`keel-wasm`** -- a single `lib.rs` exposing a subset of `keel-topo` to
  browser / Node consumers. Spike-stage; not API-stable.

The public re-exports from `keel-topo` are deliberately small:
`Body`, `TopoError`, `EntityId`, `OpId`, `OpReport`.

---

## 2. The numeric and predicate foundation (`keel-math`)

### The core design inversion

Keel separates two kinds of decision and treats them with two different number
systems:

- **Combinatorial topology decisions** (which side of a plane a point is on,
  whether three points are collinear, the order of intersection coordinates)
  are made **exactly**. They are never allowed to depend on the sign of a raw
  `f64`.
- **Metric geometry** (coordinates, parameter values, tessellated lengths) is
  carried in **tolerant `f64`** with explicit per-entity tolerances.

The practical rule, stated in `predicates.rs`: *branching on a raw `f64` sign
anywhere outside the predicate / interval / algebraic layer is a defect.* Exact
combinatorics over tolerant metric coordinates is the whole reason the kernel
can offer a defined error contract.

### One home for every epsilon

`tolerance.rs` is the only module permitted to define epsilons. The
session-wide `Tolerances` struct carries `linear` (default `1e-8`) and
`angular` (default `1e-11` radians), the published Parasolid resolutions in a
normalized model box. Per-entity tolerances live on topology entities
(`Vertex::tolerance`, `Edge::tolerance`) and are always at least the session
floor. Equality helpers (`eq_linear`, `is_zero_linear`, `eq_angular`) live
here too, so "are these coincident?" has exactly one definition.

### Exact predicates

`predicates.rs` wraps the `robust` crate (Shewchuk adaptive predicates with
internal floating-point filters) behind a three-valued `Sign` enum
(`Negative` / `Zero` / `Positive`). `orient_2d` and `orient_3d` are the
primitives. One convention worth knowing: `robust::orient3d` uses Shewchuk's
"positive when `d` is below the plane" sign; Keel flips it to an
above-the-CCW-plane convention, and the `orient3d_basic_convention` unit test
is the arbiter of that choice.

### Interval arithmetic

`interval.rs` provides outward-rounded `Interval { lo, hi }` over the extended
reals. Every operation widens by one ULP (`next_down` / `next_up`) so the
enclosure is always sound, never too tight. The certified `sign()` returns
`Some(+/-1)` only when the interval is strictly one-signed (otherwise `None`),
and `checked_div` / `sqrt` refuse or clamp the indeterminate cases. Intervals
are the substrate for certified containment: NURBS evaluators are generic over
the scalar type precisely so the same de Casteljau code runs in interval
arithmetic during certification (`eval_homogeneous_interval`,
`point_enclosure` in `nurbs_surface.rs`).

### Polynomial and Bernstein root solvers

- `poly.rs` -- `solve_quadratic` (the stable Blinn / Numerical-Recipes form
  that never subtracts nearly equal quantities) and `solve_cubic` (Yuksel's
  monotonic-interval strategy: split the domain at the derivative's roots, run
  bracketed Newton on each sign change). Both `normalize` coefficients first
  and drop roots pushed beyond `f64` range by denormal coefficients -- fixes
  that came directly out of fuzzing.
- `bernstein.rs` -- `Bernstein` in the well-conditioned Bernstein basis:
  backward-stable de Casteljau `eval`, the exact Farouki-Rajan product `mul`
  (stays in basis, never converts to power form), `add` via degree elevation,
  and the subdivision root finder.
- `multibernstein.rs` -- `MultiBernstein`, the tensor-product form, with the
  projected-polyhedron multivariate subdivision solver for systems
  (root-enclosure boxes in `[0,1]^n`).
- `newton.rs` -- the bracketed hybrid Newton solver used as the polish step
  throughout (`solve_bracketed`).

### The exact algebraic one-root layer

`algebraic.rs` implements the Tier-1 exact algebraic number `OneRoot`:
`(a + b*sqrt(c)) / d` over exact `BigInt` integers (`bigint.rs`), with the
Devillers-Fronville-Mourrain-Teillaud comparison recipe. Decisions reduce to
**sign batteries on integer expressions** (squaring with sign case analysis);
no square root is ever evaluated exactly. Each public predicate runs the
standard EGC cascade: a cheap floating filter first, the exact integer battery
only when the filter is inconclusive. Scope is deliberately the degree-2
one-root numbers that cover circle/circle and same-family conic intersection
coordinates; the API is shaped after CGAL's `Ak_1` so a degree-4 conic-conic
tier can slot in behind the same interface later.

---

## 3. Curves and surfaces (`keel-geom`)

Geometry is **first-class analytic, never silently NURBS**. Two exhaustive
dispatch enums anchor the layer.

`Curve3` (`curve.rs`):

```
Curve3 = Line(Line3) | Circle(Circle3) | Ellipse(Ellipse3) | Nurbs(NurbsCurve)
```

`Surface3` (`surface.rs`):

```
Surface3 = Plane(Plane3) | Cylinder(Cylinder3) | Cone(Cone3)
         | Sphere(Sphere3) | Torus(Torus3)
```

Surfaces of revolution are realized through the NURBS branch (`revolve_full`
in `nurbs_surface.rs` produces a `NurbsSurface`), so the analytic enum stays
the five exact quadrics; the topology layer's `SurfaceGeom` wraps either an
`Analytic(Surface3)` or a `Nurbs(NurbsSurface)`.

Key facilities:

- **NURBS in homogeneous 4D.** `NurbsCurve` / `NurbsSurface` evaluate via de
  Boor / de Casteljau on `Vec4` control points, then project (`h.x/h.w`, ...).
  The homogeneous formulation gives exact circular arcs and full circles,
  rational derivatives (hodographs), knot insertion, and Bezier decomposition.
  Every evaluator is generic over the scalar type so interval enclosures share
  the code (see section 2).
- **Analytic curves and surfaces** carry their own frames and exact closest-
  point projection (e.g. `Line3::project` is a single dot product).
- **Local differential geometry.** `SurfaceLocalGeometry` (built by
  `local_geometry_from_ders`) carries first and second fundamental forms
  (`E,F,G` / `L,M,N`), Gaussian and mean curvature, principal curvatures
  `k1 >= k2`, and the two principal directions, with a relative degeneracy
  test on `|du x dv|` and a deterministic umbilic fallback.
- **Global closest-point projection** (`project.rs`): Bezier-decompose, prune
  with an AABB branch-and-bound, polish with bracketed Newton.
- **Surface-surface intersection** (`ssi.rs`) feeds the boolean seam stage;
  results carry a certified deviation bound (`tol_achieved`) that rides onto
  the imprinted edge.

---

## 4. The topology data model (`keel-topo`) -- the heart

Keel's B-rep is a **radial-edge / PES-class non-manifold** structure with
**first-class space-partitioning regions**. This is the most important section
to internalize.

### The entity tower

`entity.rs` defines the tower (Parasolid names):

```
Region -> Shell -> Face -> Loop -> Fin -> Edge -> Vertex
```

with `CurveGeom` (= `keel_geom::curve::Curve3`) on edges and `SurfaceGeom` on
faces. The structs:

- **`Vertex`** -- `point: Vec3`, `tolerance`, one representative incident
  `fin` for the manifold umbrella, plus `groups: Vec<FinKey>`, the PES
  partial-entity slots: one representative fin per *additional* umbrella at a
  non-manifold vertex (empty in the manifold case, populated by
  `merge_vertices`).
- **`Edge`** -- a `curve: Option<(CurveKey, bool)>` (geometry reference plus a
  sense bit), `bounds: (VertexKey, VertexKey)` (equal keys = a closed edge
  with a seam vertex), an optional `arc_sweep` (signed angular sweep that
  disambiguates minor vs major arc, set only where an arc may exceed pi, e.g.
  a wide-angle partial revolve), `tolerance`, and the **radial cycle**:
  `radial: Vec<FinKey>`, **all** fins using this edge in angular order around
  it. Manifold edges have exactly two; wire edges have none; a non-manifold
  edge has three or more. This radial fin set is what makes the structure
  non-manifold: an edge is not limited to bounding two faces.
- **`Fin`** -- the coedge / radial-edge element. A `Fin` is one directed use
  of an edge by a loop: `edge`, `forward` (true traverses `bounds.0 ->
  bounds.1`), `owner: LoopKey`, the loop cycle links `next` / `prev`, and a
  reserved `pcurve: Option<(CurveKey, bool)>` (the edge's curve in the owning
  face's surface parameter space). Fins are the spokes both of the loop cycle
  (around a face boundary, via `next`/`prev`) and of the radial cycle (around
  an edge, via `Edge::radial`).
- **`Loop`** -- `face`, an entry `fin` (None means a vertex loop, an isolated
  vertex in the face), an optional `vertex` (Some iff vertex loop), and a
  `kind` (`Outer` / `Inner`). `Face::loops[0]` is the outer loop by
  convention.
- **`Face`** -- `surface: Option<(SurfaceKey, bool)>`, its `loops`, and
  crucially **both** `front_region` and `back_region`. A face separates two
  regions; the surface normal points out of the `Front` side. (A face whose
  both sides bound solid material is an interior partition wall -- see the
  cellular-boolean note in section 6.)
- **`Shell`** -- `region`, its `faces: Vec<(FaceKey, Side)>` (each face used
  with a `Side`), `wires`, an optional `acorn` vertex, and a `genus`
  contribution maintained only by the handle operators. A closed manifold face
  set appears as two shells (one per side); a sheet face has both sides in one
  shell.
- **`Region`** -- `solid: bool`, `infinite: bool`, and its `shells`. Regions
  are first-class: space is partitioned into regions, exactly one of which is
  the infinite region, and a boolean result can legitimately carry several
  solid regions (non-manifold / cellular results are retained, not collapsed).

### Identity, arenas, and the lookup scheme

`body.rs` holds the `Body`, which stores each entity type in a typed `Arena`
(`arena.rs`). Two distinct kinds of name coexist:

- **Arena keys** (`VertexKey = Key<Vertex>`, etc., all in `entity.rs`) are
  transient addresses into an arena -- fast, but not stable across edits.
- **`EntityId(u64)`** is the stable identity: monotonic per body, never
  reused, serializable. It is the *name*; the arena key is the *address*.

The `Body` carries `ids: BTreeMap<EntityId, AnyKey>` -- identity to live
entity -- and this map is **the deterministic iteration order** for the whole
kernel. `AnyKey` is the untyped union used by the id map and lineage. Iterating
in `EntityId` order is what makes every operation reproducible regardless of
arena layout (the `entity_ids_are_monotonic_and_stable` test locks this).

`Body` also carries `lineage: BTreeMap<EntityId, Lineage>` and per-entity
`attrs` (typed `AttrValue`: `F64` / `I64` / `Bool` / `Str` / `Vec3` /
`Bytes`).

### Sessions, lineage, and transactional editing

Every public mutation produces exactly one `OpReport` (`lineage.rs`): no
operation may mint anonymous topology. `Derivation` records how an entity came
to exist (`Created` / `Modified` / `Generated` / `SplitChild` /
`MergeResult` -- the OCCT Modified/Generated/Deleted contract made native,
plus first-class split and merge events). `Body::begin_op` returns an
`OpRecorder` that gathers the report. `session.rs` (`Session`, `Pmark`) layers
transactional editing -- pmark / rollback -- on top, so a sequence of edits can
be wound back.

### Validation

`validate.rs` is the structural truth oracle. `Body::validate` runs the id-map
check, fin-ring and radial-cycle integrity, loop and shell/region consistency,
the boundary-chain (d-of-d) check, and the Euler-Poincare identity, returning a
`Vec<ValidationError>`. Debug builds call `debug_validate` after every public
operation, so a structurally broken body cannot survive a single op unnoticed
in development. Note this is *necessary, not sufficient*: a structurally valid
body can still be geometrically wrong, which is why the boolean and surgery
paths add the volume gates in sections 6, 8, and 10.

---

## 5. Euler operators (`euler.rs`)

All topology mutation goes through the Euler-operator family; nothing pokes the
arenas directly. Each operator preserves the Euler-Poincare invariant by
construction (verified by `validate()`), and each has an inverse so edits are
reversible.

| Operator | Role | Inverse |
|---|---|---|
| `mvfs` | make vertex, face, shell (seed a body from a region) | (kill counterpart) |
| `mev` | make edge + vertex (grow a boundary; site chosen via `MevSite`) | `kev` |
| `mef` | make edge + face (split a face by joining two fins) | `kef` |
| `mekr` | make edge, kill ring (bridge / connect an inner ring) | `kemr` |
| `mfkrh` | make face, kill ring + handle (close a handle, +genus) | `kfmrh` |

`MevSite` selects where `mev` attaches (`VertexLoop` for the first edge off an
isolated vertex, `AfterFin` to extend a boundary). The handle pair
(`mfkrh` / `kfmrh`) is the only code that maintains `Shell::genus`. The tests
`cube_by_euler_operators`, `kef_inverts_mef_on_cube_step`, and
`kemr_and_mekr_round_trip` demonstrate building and unwinding real topology
purely through these operators.

---

## 6. The boolean pipeline (`boolean.rs`)

The three regularized operations are `BoolOp::Union` / `Intersection` /
`Difference`. The public entry is `boolean(a, b, op, tol)`, which calls
`boolean_with` (the `BooleanOptions { regularize }` form). `boolean_tolerant`
adds a bounded snap tier; `boolean_multi` and `boolean_selective` are
multi-tool and face-pair-subset variants.

### Front-door checks and fast paths (in `boolean_with`)

Before any heavy machinery:

1. **Inside-out intake.** An operand with negative signed volume
   (`mesh_volume() < -1e-9`) declines immediately -- classification would read
   its inverted winding as legitimate and produce a self-consistent but wrong
   result.
2. **Broad phase (AABB separation).** If the operands' bounding boxes are
   provably separated (with a conservative margin that accounts for the
   tessellation-derived AABB under-reporting a curved body), the result is
   trivial: empty intersection, `a` unchanged for difference, or, for a
   disjoint union, `combine_disjoint(a, b, tol)` -- a clean verbatim merge of
   the two lumps into one disconnected body (this is the curved-disjoint-union
   fix; it cross-checks analytic mass against the render mesh and declines a
   gross disagreement).
3. **Coincident pre-imprint.** `preimprint_coincident_overlaps` finds coplanar
   overlapping planar face pairs (`coincident_face_pairs`) and imprints the
   overlap-boundary cuts onto both operands, so each resulting fragment is
   uniformly inside / outside / on the other body.
4. **No-boundary-interaction shortcut.** With no seams and no coincidence, the
   operands are disjoint or nested, decided by one unambiguous generalized-
   winding-number probe per side (probing guaranteed-interior points, never raw
   vertices). Curved results additionally must satisfy an op-volume bound or
   they decline.

### The core assembly stages (`assemble_boolean`)

When the operands genuinely interact, the pipeline runs:

```
  seam_curves(a, b)                 # SSI per face pair -> localized SeamCurves
        |
        v
  imprint_operand(a, ...)           # clone each operand, split the seams into it
  imprint_operand(b, ...)           #   -> two ImprintedOperands (+ seam_edges)
        |
        v
  classify_faces(ia.body vs b)      # GWN-classify every fragment of each side
  classify_faces(ib.body vs a)      #   -> FaceClass per face
        |
        v
  select_faces(op, class_a, class_b)# Requicha r-set selection -> Vec<KeptFace>
        |
        v
  stitch_by_import(ia, ib, kept)    # identity-preserving import-and-glue
        |
        v
  finalize_imported_assembly(...)   # regions, shells, sense; build the Body
        |
        v
  POST-CONDITION GATES              # volume / self-consistency (section 10)
```

Stage by stage:

- **`seam_curves`** runs SSI on each candidate face pair and returns
  `SeamCurve`s, each localized to its `(face_a, face_b)` with the intersection
  `curve`, a `closed` flag, the certified `tol`, and `on_boundary_a` /
  `on_boundary_b` flags (the seam may already lie on an existing boundary edge
  of one operand, in which case that side needs no imprint). Tangential and
  unassemblable-seam faults here cause a hard decline rather than a seamless
  (Euler-valid but geometrically wrong) body.
- **`imprint_operand`** clones an operand and splits the seam curves into the
  named face, returning an `ImprintedOperand { body, seam_edges }`. The
  per-face planar overlay imprint and curve splitting live in `imprint.rs`.
- **`classify_faces`** classifies every face fragment against the other
  operand solid. The **generalized winding number is the primary classifier**
  (robust at on-boundary and tangential contacts, surface-type-agnostic, no
  pcurve dependency): `w ~ 1` inside, `~ 0` outside, a band around `0.5` means
  on the boundary. Geometric coincidence is checked first
  (`coincident_sense_at`) because the winding number is undefined on the
  boundary; samples in the ambiguous band fall back to a two-sided probe along
  the face normal. The result is a `FaceClass`: `InsideOther` /
  `OutsideOther` / `OnOther(OnSense)` (`Same` / `Opposite` / `Unknown`) /
  `Unknown`. Zero-area fragments are gated out as `Unknown`.
- **`select_faces`** applies the Laidlaw-Trumbore-Hughes / Requicha r-set
  tables, returning `KeptFace { operand, face, reversed }`:
  - Union: A-faces `OutsideOther` (plus on-same) + B-faces `OutsideOther`.
  - Intersection: A-faces `InsideOther` (plus on-same) + B-faces
    `InsideOther`.
  - Difference A-B: A-faces `OutsideOther` (plus on-opposite) + B-faces
    `InsideOther`, with B **reversed** (the subtracted solid's walls flip
    inward). Coincident overlaps contribute one copy, kept from operand A by
    convention.
- **`stitch_by_import`** is the primary assembler: it imports each kept
  fragment carrying its operand's edge identity into a fresh body and glues
  only the genuinely coincident cross-operand seam (matching seam subdivision
  pairs the planar seam coedges 1:1). The older polygon-soup stitcher is
  retired: a stitch failure now **declines** rather than building a body the
  closure invariant cannot protect.
- **`finalize_imported_assembly`** assigns regions and shells, fixes senses,
  and produces the final `Body` (with the retained interface walls when
  `regularize: false`).

### Cellular (non-regularized) booleans

With `BooleanOptions { regularize: false }`, a union of two solids abutting on
a coincident face keeps that face as a **double-sided interior partition
wall** (the `OnOther(Opposite)` fragments become `walls`), and the result is a
cellular solid whose material is split into multiple solid regions -- exactly
what the first-class-regions model is for. The winding integrator skips
interior walls when measuring the outer boundary (`is_interior_wall` in
`winding.rs`).

The result type is `BoolResult { body, faults, op }`; faults are the
`BoolFault` enum (`Coincident` / `Tangent` / `IntersectionFailed` /
`UnassemblableSeam` / `Topo` / `AssemblyFailed`). Faults accumulate; a
recoverable run can report partial success, while the decline classes return
`Err`.

---

## 7. Blends and fillets (`blend.rs`, `chamfer.rs`)

The fillet model is **exact-analytic surgery**, following the rolling-ball
envelope construction. The centerpiece procedural fact (`blend.rs`) is:

```
spine = SSI( offset(S_a, r), offset(S_b, r) )
```

the spine (the rolling-ball-centre locus) is the intersection of the two
support surfaces each offset by the fillet radius `r`.

- **Plane-plane convex edge.** Each offset plane is parallel, their
  intersection is a line parallel to the sharp edge, and the envelope of equal
  radius-`r` spheres centred on that line is an exact native **cylinder**.
  `EdgeBlend` carries the `spine` line, the two `spring_a` / `spring_b`
  tangency lines (the feet of the perpendiculars from the spine onto each
  plane, i.e. the spine translated by `+/- r*n`), and the exact
  `Cylinder3` blend surface, tangent to each support plane along its spring
  line (G1 by construction).
- **Plane-cylinder cap rim.** Where a planar face meets a cylinder
  perpendicular to its axis, the spine is a circle and the envelope is an exact
  **torus** of minor radius `r`. `EdgeBlendTorus` carries the spine circle, the
  supports, and a `Torus3`. **Convex vs concave** is the radius bookkeeping:
  the inward (convex) torus needs `major = r_cyl - r > minor = r`, hence
  `r_cyl > 2r`; the outward (concave) torus has `major = r_cyl + r`. A
  non-perpendicular plane-cylinder pair is a cyclide and currently declines
  (follow-up).
- **Convexity test.** `edge_is_convex` probes the generalized winding number
  just off the edge along the in-face bisector: inside the material means
  convex. It handles straight and circle-arc edges (the on-arc midpoint and
  tangent are computed from the circle geometry, since a chord midpoint is not
  on the arc).

`chamfer.rs` carries the symmetric and asymmetric chamfer surgery (the setback
geometry), and corner handling joins adjacent blend faces at a vertex. The
trim-and-stitch local operation that inserts a blend face into the body builds
on the Euler operators and the imprint machinery; higher rungs (circle-spine
cyclides, NURBS canal surfaces, variable-radius spines) layer on top of the
same envelope idea.

---

## 8. The mass integrator (`massprops.rs`)

Mass properties are computed **analytically, per face**, by the divergence
theorem -- not from the mesh. `MassProps` carries `volume`, `centroid`, and the
`inertia` tensor about the centroid. The accumulator `Moments` adds one
quadrature sample at a time from a position and an *orientation-corrected*
area-weighted normal.

### The orientation audit (no sign fudge)

The load-bearing design rule: the per-face outward normal is
`n_out = sense * natural` (the single sense-based authority shared with the
mesh path), folded together with each face's own loop winding. Region solidity
only *validates* that a face bounds exactly one solid region; it no longer sets
the sign on its own. A negative volume is therefore a real orientation bug,
never something to `abs()` away. This is what lets a genus-1 tube or a
reversed-sense cavity wall integrate correctly.

### The Green-slab integrator

Iso-rectangular planar and curved faces integrate their parameter region
directly (triangle-fan degree-5 rule or polar trapezoid x Gauss-Legendre for
planar; composite Gauss-Legendre for full curved rectangles, using the
tabulated 8-point `GL8` nodes). But trimmed curved faces -- the mitre's
ellipse seams, oblique-end caps, corner sphere triangles -- have no iso-
rectangle parameter region. For those, `integrate_face_green` folds the region
integral onto its boundary:

```
int_R F du dv = - loop_int G du,   with G(u,v) = int_{vb}^{v} F(u,s) ds
```

Each boundary Gauss node carries an inner v-slab of quadrature samples, so the
ordinary `Moments::add` machinery integrates any trim whose fins are evaluable
with derivatives. Cylinder rulings (lines, degree-1 NURBS) contribute zero
through `u' = 0`; circles and ellipses carry the flux. The slab base `vb`
matters only when the boundary winds in `u` (a sphere trim enclosing a pole),
where it anchors at the enclosed pole; a cone-apex anchor handles the
full-revolution cone-tip face the same way.

### Why this is trustworthy

The Green-slab integrator is proven correct and **locked by a test**. When a
realsoak seed produced an analytic mass that disagreed with the mesh, the tie
was broken by an *independent* witness: a generalized-winding-number
Monte-Carlo volume (Van Oosterom-Strackee solid angles), an integrator that
shares no machinery with the divergence/flux volume that both the analytic mass
and the mesh tetra-sum use. It matched the analytic mass, not the mesh,
proving the integrator exact and the *body* malformed. That result is locked in
`crates/keel-topo/tests/cyl_union_mass_witness.rs`: the curved-face mass
integrator is exact and GWN-self-consistent on the multi-axial-band cylinder
configuration. (See also `cyl_cyl_band.rs`, `post_fillet_mass.rs`,
`disjoint_curved_mass.rs`.)

---

## 9. Tessellation (`tessellate.rs`)

Tessellation produces **outward-oriented triangles**, primarily as a
classification aid for the winding number, and secondarily as the user-facing
facet product (`facets`, with `render.rs` adding edge / silhouette lines on
top). Curved arcs are split adaptively: `arc_segments` chooses the chord count
so the chord deviates from the arc by at most a tolerance
(`n >= span * sqrt(radius / (8*tol))`), clamped to a sane range.

The defining property is the **edge-first watertight contract**: each face's
curved facets are fanned to the loop's *shared edge samples*, so two faces that
meet along an edge produce **identical boundary chords** and the mesh is
watertight across that seam by construction (rather than hoping two
independent tessellations happen to agree). The comments throughout
`tessellate.rs` track this invariant ("fan it to the loop's shared edge
samples -- watertight with the partner"). Cut edges that lie on a facet plane
(rim chords) seam watertight against the partner face that shares the same rim
polyline. This watertightness is what makes the mesh a valid second opinion in
the mass-vs-mesh gate (section 10).

---

## 10. The correctness architecture

Keel's contract is **DECLINE-never-WRONG**: every boolean, blend, and local
edit returns a topologically correct result or explicitly declines, and never
silently returns a wrong body. Several independent mechanisms compose to
enforce this.

### The three-bucket oracle

Randomized testing classifies every trial into exactly one of:

- **PASS** -- a clean result that matches an independently computed reference
  (an exact closed form, or a second code path).
- **DECLINE** -- the kernel refused (counted, never penalized).
- **WRONG** -- a clean result that disagrees with the reference. The gate is
  `WRONG == 0`, always.

`crates/keel-topo/tests/three_bucket.rs` is the canonical instrument: random
axis-aligned box pairs whose reference volume is **exact interval arithmetic**
(so axis-aligned planar results tessellate exactly and both gates are tight),
plus a cone sector whose reference is the exact closed-form frustum slice. A
deterministic seekable LCG makes the run reproducible and shardable across
processes. The companion scan tests are `scan_wrong.rs` and `scan_declines.rs`.

### The dual mass-vs-mesh gate

Before a boolean result is accepted, it must pass volume gates layered inside
`assemble_boolean` (and mirrored on the surgery paths):

- **Positive finite volume.** The scalar Euler identity is necessary but not
  sufficient, so a positive-volume floor rejects near-degenerate slivers.
- **Mass == mesh self-consistency.** For an all-planar result the mesh is
  exact, so the analytic mass and the tessellated volume must agree to roughly
  `1e-9`; for a curved result they must agree within the adaptive
  tessellation's worst chordal band (about 2%).
- **Independent op-volume bound.** The result volume must lie in `[lo, hi]`
  derived from the *exact operand volumes* (e.g. a union is in
  `[max(va,vb), va+vb]`), which catches a self-consistent wrong (mass and mesh
  agreeing on an impossible value).
- **User-facing mesh consistency.** The per-connected-component
  `mesh_volume` must not collapse relative to the analytic mass beyond the
  oracle's 25% curved-WRONG threshold -- this closes the case where a single
  global-reference tetra sum agrees with mass while the component-summed mesh
  does not.
- **Tight quadric-sphere oracle.** For a cylinder-or-cone + sphere pair, an
  exact 1D-integral op-volume (`quadric_sphere_op_volume`) provides ground
  truth even where the trimmed result has no closed form.

A failure of any gate is a **decline**, never a returned body.

### The GWN independent volume witness

The generalized winding number (`winding.rs`) is both the primary boolean
classifier and an *independent volume witness*. Because it sums signed solid
angles rather than surface flux, it shares no machinery with the analytic mass
or the mesh tetra-sum, so it can adjudicate a disagreement between them (the
role it played in locking the Green-slab integrator, section 8). `gwn_over`
evaluates a point against a precomputed outer-boundary triangle set;
`boundary_triangles` builds that set once per immutably-borrowed body and skips
interior partition walls.

### The realsoak faithful-workflow harness

`crates/keel-topo/examples/realsoak.rs` is the decline-frontier soak. Instead
of random two-primitive booleans, it samples the *learned grammar* of real
Plasticity (CAD app) modeling sessions -- distilled from 113 tutorial
transcripts -- and runs long op-chains (median ~14, mean ~24 ops) on a growing
solid, the distribution that finds real-usage bugs. Each produced body is run
through the same three-bucket posture: `validate()`, then the exact analytic
`mass_properties` checked against (1) an independent volume interval the
generator tracks across the chain and (2) the tessellated `mesh_volume`
(all-planar: a gap above 2% is WRONG; curved: a gross gap above 25% is WRONG).
Ops with no clean realization decline by design and the chain continues, which
is what yields the long realistic chains. Outputs are JSONL worklists
(`passes`, `declines`, `failures`, `projects`); `declines.jsonl` is the
minimize-declines worklist.

Together: the three-bucket oracle proves WRONG is zero against exact
references, the dual mass-vs-mesh gate turns silently-wrong bodies into
declines at runtime, the GWN witness adjudicates the integrators independently,
and the realsoak harness exercises the whole stack along realistic usage
trajectories. That composition is how DECLINE-never-WRONG is held.

---

## See also

- [`README.md`](../README.md) -- crate summary, capabilities tables, honesty
  contract, quickstart.
- [`docs/CAPABILITIES.md`](CAPABILITIES.md) -- the honest capabilities and
  limitations frontier: the faithful tutorial scoreboard, the decline taxonomy,
  and the WRONG=0 evidence chain.
- [`docs/PARITY_OPS.md`](PARITY_OPS.md) -- the operation parity matrix.
- [`docs/ENGINEERING-LOG.md`](ENGINEERING-LOG.md) --
  the running engineering narrative, per-decision rationale, and the current
  anchor.
