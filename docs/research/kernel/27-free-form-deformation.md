# Free-Form Deformation and Shape Deformation Operations

Research file 27 for the Keel kernel. This file consolidates the literature on **space deformation and shape deformation** of CAD geometry: free-form deformation (FFD) lattices, direct manipulation, cage-based deformation with generalized barycentric coordinates, axial and curve-driven deformation, Barr's global bend/twist/taper operators, surface-based and variational (energy-minimizing) deformation, and the emboss/wrap family. The unifying property is that deformation is a **warp of the embedding space (or of the control net) that preserves topology**: the body's faces, edges, vertices, and their adjacency are untouched. Only the geometry of each face/edge moves, by re-evaluating control points or sample points through a deformation map. This makes deformation categorically distinct from booleans, local operations (file 03), and direct editing (file 12), all of which can change topology.

## Why this matters for Keel

The Parasolid capability audit (file 25) lists free-form space deformation, bend, twist, emboss, and wrap. Parasolid itself delegates some of these to D-Cubed or the host application, but the NURBS applications file (nurbs/06, section 6.1) reached a stronger conclusion from the ship-hull and automotive-styling literature: a Parasolid-class kernel that wants to serve naval and automotive design should expose **free-form deformation of NURBS bodies as a first-class kernel service** ("deform-while-preserving-parameterization"), alongside a fairing/energy-minimization operator over control points. Deformation is also the standard **parameterization for shape optimization** (CFD and structural), where a low-dimensional lattice or cage drives an expensive analysis loop. So Keel needs deformation not as a UI convenience but as a kernel-level geometric service with a clear validity contract.

Three cross-cutting concerns recur throughout this file and shape the closing synthesis:

1. **Type promotion.** Deforming an analytic face (plane, cylinder, cone, sphere, torus) almost always produces a free-form surface. A deformed plane is no longer planar. The kernel must promote the face geometry to NURBS before (or as part of) deformation. This is the inverse of the canonical-geometry-recovery service (file 24): deformation destroys analytic intent, recovery restores it.
2. **Trim and topology preservation.** A trimmed-NURBS face carries trim loops in parameter space (pcurves). If deformation re-parameterizes the surface, the pcurves must stay valid; if it only moves control points without changing the knot vectors or parameter domain, pcurves are automatically preserved. This is the decisive reason to prefer **control-net deformation** over **sample-and-refit** for B-rep bodies.
3. **Re-validation.** Deformation can break watertightness (shared edges drift apart), violate tolerances, and introduce self-intersection. After any deformation the kernel must re-establish the watertight/tolerance invariants and, ideally, prove injectivity of the deformation map.

---

## Theme 1: Lattice free-form deformation (FFD)

The lattice family embeds the body inside a trivariate volume (a 3D analog of a tensor-product surface), moves the volume's control points, and re-evaluates. The body never knows it has been deformed: each of its points is just re-located by the volume map.

### Sederberg & Parry: free-form deformation of solid geometric models (the foundational paper)

**Citation.** Sederberg, T. W., & Parry, S. R. (1986). Free-form deformation of solid geometric models. *Computer Graphics (Proceedings of SIGGRAPH '86), 20*(4), 151-160.

**Method.** Impose a local coordinate system on a parallelepiped region of space, anchored at a corner `X0` with three edge vectors `S`, `T`, `U`. Any point `X` in the box has local coordinates `(s,t,u)` recovered by the projection formulas

```
s = (T x U) . (X - X0) / ((T x U) . S)
t = (S x U) . (X - X0) / ((S x U) . T)
u = (S x T) . (X - X0) / ((S x T) . U)
```

so that `X = X0 + s S + t T + u U` with `0 <= s,t,u <= 1` for points inside the box. A grid of control points `P_ijk` is laid on the box, `P_ijk = X0 + (i/l) S + (j/m) T + (k/n) U` for `i in [0,l], j in [0,m], k in [0,n]`. The deformed position of `X` is the trivariate Bernstein (Bezier) tensor product

```
X_ffd = sum_{i=0}^{l} sum_{j=0}^{m} sum_{k=0}^{n}
        B_i^l(s) B_j^m(t) B_k^n(u) P_ijk
```

where `B_i^l(s) = C(l,i) s^i (1-s)^(l-i)` are Bernstein polynomials. In the undeformed state `P_ijk` sit on the regular grid and `X_ffd = X` exactly (the Bezier volume reproduces the identity affine map). The user **moves** the `P_ijk` and re-evaluates. The **embed-deform-evaluate pipeline** is: (1) embed, compute `(s,t,u)` for every point of the body once; (2) deform, displace lattice control points; (3) evaluate, recompute `X_ffd` from the stored `(s,t,u)` and the new `P_ijk`.

Two results give the paper its lasting value. First, **derivative continuity is inherited from the basis**: because the deformation is a polynomial map, a `C^k` surface stays `C^k`, and continuity between lattices can be controlled by matching control-point rows across lattice boundaries (move a boundary plane of control points and adjacent lattices to keep `C^0`, `C^1`, etc.). Second, **volume can be preserved**: the Jacobian of the deformation integrates to the volume change, and the paper derives the condition (a polynomial constraint on the `P_ijk`) under which the deformation is volume-preserving.

**Limitations.** The lattice is a rigid parallelepiped, so the region of influence is box-shaped and axis-aligned to `S,T,U`; deforming a curved region requires either a coarse box (loss of local control) or many boxes. The map is global within the box: moving one control point perturbs the whole box (Bernstein basis has global support). There is no direct manipulation: the user edits control points, not surface points. Self-intersection is possible and unprevented (see Gain & Dodgson, Theme 7). The body must be densely sampled (tessellated) or carry exactly evaluable points; for a NURBS body the natural points to push through are the surface control points (see Theme 6).

**Kernel relevance.** This is the canonical algorithm Keel should implement for "free-form space deformation." The decisive design choice for a B-rep kernel: do **not** sample the surface and refit. Instead push the **NURBS control points** of each face through the FFD map. Because FFD is an affine-invariant, convex-combination map (Bernstein weights sum to one), the deformed control net defines a valid NURBS surface with the **same knot vectors and parameter domain**, so all pcurves/trim loops remain valid by construction. The cost is one FFD evaluation per control point, not per tessellation vertex. Type promotion: any analytic face must first be expressed as NURBS (file 24 forward direction) before its control points can be deformed.

### Griessmair & Purgathofer: deformation of solids with trivariate B-splines (B-spline FFD)

**Citation.** Griessmair, J., & Purgathofer, W. (1989). Deformation of solids with trivariate B-splines. In *Proceedings of Eurographics '89* (pp. 137-148). North-Holland.

**Method.** Replace the trivariate Bernstein basis of Sederberg-Parry with a trivariate (non-rational) **B-spline** basis: `X_ffd = sum_ijk N_i^p(s) N_j^q(t) N_k^r(u) P_ijk`, where `N` are B-spline basis functions over knot vectors in each parametric direction. The key gain is **local support**: a B-spline basis function is nonzero only over `p+1` knot spans, so moving a control point perturbs only a local neighborhood of the deformation, not the whole box. This decouples the resolution of the lattice (number of control points, for local control) from its polynomial degree (for smoothness).

**Limitations.** Computing `(s,t,u)` for a B-spline lattice is no longer a simple projection; it requires inverting the B-spline volume map (a root-find / Newton iteration per point) unless the lattice is kept axis-aligned and uniform so the inversion factors per axis. Knot placement adds parameters the user must understand.

**Kernel relevance.** Local support is exactly what hull and Class-A styling want: tweak the bow without moving the stern. Keel's FFD service should offer a **B-spline lattice** as the default and treat the single-Bezier-block (Sederberg-Parry) case as the degenerate one-span special case. Uniform-knot axis-aligned lattices keep the inverse map cheap.

### Coquillart: extended free-form deformation (EFFD), non-parallelepiped lattices

**Citation.** Coquillart, S. (1990). Extended free-form deformation: A sculpturing tool for 3D geometric modeling. *Computer Graphics (Proceedings of SIGGRAPH '90), 24*(4), 187-196.

**Method.** EFFD removes the requirement that the lattice be a parallelepiped. The control lattice can be an arbitrary, possibly non-convex, prismatic or cylindrical arrangement of cells (for example a lattice shaped like a torus segment or a bump). Because the cells are no longer boxes, the local-coordinate computation `(s,t,u)` cannot be done by the closed-form projection; EFFD computes it by a **numerical inversion** (Newton iteration on the cell's trivariate map) to find the parametric preimage of each point. The deformation evaluation is the same tensor-product sum once `(s,t,u)` is known. Coquillart also introduces tools to build the lattice to conform to a feature (extruding a 2D prototype lattice along a path).

**Limitations.** The numerical preimage solve is per-point and can fail or be multivalued for strongly non-convex cells; cell continuity (matching `(s,t,u)` across cell boundaries) needs bookkeeping. More expressive lattices are harder for a user to author.

**Kernel relevance.** EFFD is the bridge to **feature-shaped deformation**: a lattice that hugs the region to deform (a fillet run, an embossed boss). For Keel, EFFD-style conforming lattices are the foundation under a kernel-level emboss/bead operator (Theme 6). The Newton-inversion machinery overlaps with point-inversion code Keel already needs for surface projection (nurbs/04).

### MacCracken & Joy: FFD with lattices of arbitrary topology (subdivision FFD)

**Citation.** MacCracken, R., & Joy, K. I. (1996). Free-form deformations with lattices of arbitrary topology. In *Proceedings of SIGGRAPH '96* (pp. 181-188). ACM.

**Method.** Generalize the lattice from a tensor-product grid (which forces rectangular topology) to an **arbitrary-topology control lattice** by using a **Catmull-Clark-style subdivision** of the lattice volume. The lattice is repeatedly subdivided; a point of the body is associated with the subdivided cell that contains it, and its deformed position is computed from the limit of the subdivision applied to the (user-moved) lattice. This allows lattices with extraordinary vertices, non-cuboidal cells, and branching, so the deformation tool can match objects that a single box cannot (a hand, a branching mechanical part).

**Limitations.** No closed-form basis; evaluation is via iterated subdivision to a tolerance, which is slower and complicates exact derivative (normal) transformation. Behavior near extraordinary vertices is only `C^1` and harder to reason about.

**Kernel relevance.** Arbitrary-topology lattices are powerful but heavy. For a CAD kernel whose bodies are mostly built from analytic and tensor-product NURBS faces, the simpler B-spline / EFFD lattices usually suffice. MacCracken-Joy matters as the answer when a single tensor-product lattice cannot wrap the region; Keel can treat it as an advanced lattice type rather than the default.

---

## Theme 2: Direct manipulation deformation

The lattice papers make the user edit control points, which is unintuitive: the user wants to grab a point *on the surface* and drag it. Direct manipulation inverts the FFD map: solve for the control-point displacements that achieve a prescribed surface-point displacement.

### Hsu, Hughes & Kaufman: direct manipulation of free-form deformations (DMFFD)

**Citation.** Hsu, W. M., Hughes, J. F., & Kaufman, H. (1992). Direct manipulation of free-form deformations. *Computer Graphics (Proceedings of SIGGRAPH '92), 26*(2), 177-184.

**Method.** Fix the FFD lattice and its basis. The deformed position of a surface point with stored local coordinates `(s,t,u)` is linear in the stacked control-point vector `P`: `X_ffd = B(s,t,u) . P`, where `B` is the row vector of tensor-product basis products `B_i^l(s) B_j^m(t) B_k^n(u)`. The user grabs `X_ffd` and asks for a displacement `dX`. The required control-point change `dP` satisfies the linear system

```
dX = B dP
```

This is **underdetermined**: `B` is one (or a few) rows, `dP` has `3 (l+1)(m+1)(n+1)` unknowns. Hsu et al. select the **minimum-norm** solution via the Moore-Penrose pseudoinverse,

```
dP = B^+ dX,   with   B^+ = B^T (B B^T)^{-1}
```

which moves the control points as little as possible (least squares of `||dP||`) while exactly achieving `dX`. With multiple simultaneous constraints (drag several surface points), stack the rows: `dX = B dP` with `B` now `3c x 3n` for `c` constraints, and the same pseudoinverse applies (or a damped/regularized version if `B B^T` is ill-conditioned). The user thus "directly manipulates" the surface; the control-point math is hidden.

**Limitations.** The minimum-norm choice is geometrically arbitrary; it can spread a local edit across distant control points (mitigated by B-spline local support). Over-constraining (more constraints than usefully independent rows) makes `B B^T` singular and needs regularization. It inherits all FFD limitations (self-intersection, box-shaped influence). Constraints are positional; tangent/normal constraints need derivative rows of `B`.

**Kernel relevance.** This is the interaction model CAD users expect ("push the surface here"). Keel should expose a constrained-deformation API: input is a set of (surface point, target position, optional tangent) constraints, the solver returns the deformed control net via the regularized pseudoinverse. Because the unknowns are lattice control points and the body's surface control points are pushed through the lattice, trim curves stay valid as in plain FFD. The same least-squares machinery generalizes to **directly constraining surface control points** rather than lattice points, which is the link to fairing (move some control points, least-squares-fair the rest).

---

## Theme 3: Cage-based deformation (generalized barycentric coordinates)

Cage-based deformation replaces the volumetric lattice with a closed **cage** mesh surrounding the object. Each object point is written as a (generalized barycentric) combination of cage vertices; moving the cage moves the object. The cage need not be a grid, so it can hug the shape. The family is distinguished by *which* coordinate function is used, and the coordinate choice determines the deformation's quality (smoothness, locality, shape preservation, negativity).

### Ju, Schaefer & Warren: mean value coordinates for closed triangular meshes (MVC)

**Citation.** Ju, T., Schaefer, S., & Warren, J. (2005). Mean value coordinates for closed triangular meshes. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '05), 24*(3), 561-566.

**Method.** Generalize 2D polygon mean value coordinates to a closed triangle-mesh cage in 3D. For an interior point `x` and cage vertex `v_i`, the weight `w_i` is computed by integrating a kernel over the unit sphere around `x` (projecting cage triangles onto the sphere and accumulating signed solid-angle / tangent terms). The coordinates `lambda_i = w_i / sum_j w_j` are a partition of unity that **reproduces affine functions** (`sum_i lambda_i v_i = x` on the original cage). Deformation: `x_def = sum_i lambda_i v_i'` with the moved cage vertices `v_i'`. The coordinates have a **closed form** (no linear solve), are `C^infinity` in the interior, and are defined everywhere in space (also outside the cage), which is convenient for deforming points slightly outside the cage.

**Limitations.** MVC can be **negative** for non-convex cages, which produces counter-intuitive deformation (a vertex pulls a point the "wrong" way) and possible fold-over. They depend only on cage *vertex* positions, not face orientations, so they do not preserve local shape under cage rotation (see Green coordinates).

**Kernel relevance.** MVC give a fast, solve-free cage deformation with global smoothness, suitable as a baseline cage operator. The negativity in concave regions is a real defect for CAD where predictable, non-self-intersecting behavior is required; prefer harmonic or Green coordinates when the cage is concave.

### Joshi, Meyer, DeRose, Green & Sanocki: harmonic coordinates

**Citation.** Joshi, P., Meyer, M., DeRose, T., Green, B., & Sanocki, T. (2007). Harmonic coordinates for character articulation. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '07), 26*(3), 71.

**Method.** Define each coordinate function `h_i(x)` as the solution of **Laplace's equation** `Delta h_i = 0` inside the cage volume with boundary condition `h_i = 1` at cage vertex `i` and falling linearly to 0 along cage faces (a "hat" boundary condition). The `h_i` are computed numerically by discretizing the cage interior (a voxel/tetrahedral grid) and solving the Laplace system, then cached. Deformation is again `x_def = sum_i h_i(x) v_i'`. Harmonic coordinates are the first generalized barycentric coordinates that are **non-negative even for strongly concave cages**, and their influence **falls off with interior distance** (a vertex on one side of a concave cage does not leak influence across a gap).

**Limitations.** No closed form: requires a volumetric discretization and a linear solve per cage, so it is precompute-heavy and tied to the discretization resolution (accuracy/memory trade-off). The coordinates are only as smooth as the discretization; evaluation interpolates the grid.

**Kernel relevance.** Non-negativity and distance-based falloff make harmonic coordinates the **well-behaved cage option for concave regions**, where MVC misbehaves. The volumetric precompute is the cost. For Keel, harmonic coordinates suit a "wrap a cage around a feature and deform" workflow where the cage is reused across many edits, amortizing the solve.

### Lipman, Levin & Cohen-Or: Green coordinates (shape-preserving / conformal)

**Citation.** Lipman, Y., Levin, D., & Cohen-Or, D. (2008). Green coordinates. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '08), 27*(3), 78.

**Method.** Derived from **Green's third integral identity**, which expresses a harmonic function inside a domain as a boundary integral of both its values and its normal derivatives. The key move is that Green coordinates use **both cage vertex positions and cage face normals**:

```
F(x) = sum_{i in vertices} phi_i(x) v_i + sum_{j in faces} psi_j(x) s_j n_j
```

where `phi_i` are vertex coordinate functions, `psi_j` are face coordinate functions, `n_j` is the outward normal of cage face `j`, and `s_j` is a **per-face stretch factor** (the ratio of the deformed face's scale to the original) that the deformation introduces automatically. Closed-form expressions for `phi` and `psi` are given. Because the face-normal term reacts to cage face orientation, the deformation is **shape-preserving**: it induces a **conformal map in 2D** and a **quasi-conformal map in 3D** (angles and local shape are preserved up to a bounded distortion), so features rotate and bend without shearing. This is precisely the property MVC lacks: MVC has only the vertex term and so smears local shape under cage rotation.

**Limitations.** The deformation is **not interpolating**: because of the normal term, `F` does not pass exactly through the moved cage vertices (the cage is a control structure, not a constraint the surface honors exactly), which can surprise users expecting "the surface goes where I put the cage." The `s_j` scale factors are derived, not user-set, so direct positional control is indirect. 3D maps are quasi-conformal (bounded but nonzero distortion), not exactly conformal.

**Kernel relevance.** Green coordinates are the **most CAD-relevant cage scheme** because shape preservation is exactly the styling requirement: deform a fender or a hull section and keep the local character lines from shearing. For Keel, Green-coordinate cage deformation is the candidate "shape-preserving morph" operator. The non-interpolating property is the main usability caveat to surface in the API.

---

## Theme 4: Axial, curve-driven, and global (Barr) deformation

This family parameterizes deformation by a **curve or axis** (intuitive for elongated parts) or by a **closed-form global map** (Barr's bend/twist/taper, the operators the Parasolid audit names directly).

### Barr: global and local deformations of solid primitives (bend, twist, taper, scale)

**Citation.** Barr, A. H. (1984). Global and local deformations of solid primitives. *Computer Graphics (Proceedings of SIGGRAPH '84), 18*(3), 21-30.

**Method.** A deformation is a map `X -> F(X)` applied to every point. Barr gives closed-form `F` for the standard operators. **Tapering** along the z-axis scales x and y by a function of z: `F(x,y,z) = (f(z) x, f(z) y, z)`. **Twisting** about z rotates the xy-plane by an angle that grows with z: with `theta = k z`, `F = (x cos theta - y sin theta, x sin theta + y cos theta, z)`. **Bending** about an axis remaps a coordinate through a circular arc of curvature `k` over a bend region, with rigid extension outside the region. The paper's central technical result is the **normal transformation rule**: if `J = dF/dX` is the Jacobian of the deformation, then a surface normal `n` transforms not by `J` but by the inverse-transpose times the determinant,

```
n_deformed = det(J) (J^{-1})^T n
```

(the standard rule for transforming normals under a nonlinear map). Because `J` is given in closed form for each operator, normals of the deformed surface are computed directly from the undeformed normals and `J`, with **no re-tessellation needed to get correct shading/offsets**. Deformations **compose hierarchically**: nesting taper-then-bend multiplies the Jacobians, costing one extra matrix multiply per normal per level.

**Limitations.** The operators are global functions of one axis; they are not free-form (you get exactly bend/twist/taper/scale, not arbitrary shapes). Bending introduces a piecewise definition (bent region plus rigid extensions) whose `C^1` junction needs care. Strong taper (`f(z) -> 0`) or strong bend can be non-injective (fold-over). The map is defined on points, so for a B-rep body you must decide what to push through it (control points, with type promotion).

**Kernel relevance.** These are the named operators (bend, twist, taper) in the Parasolid capability audit, and Barr's closed forms are directly implementable. The **normal/Jacobian transformation rule is the single most important takeaway for a kernel**: it tells Keel how to keep surface normals (and therefore offsets, draft analysis, and shading) correct under any differentiable deformation, FFD included (FFD's `J` is the Jacobian of the trivariate map). For analytic faces, bend/twist/taper promote them to NURBS; the clean way is to apply Barr's map to the NURBS control points and rely on the affine-ish structure, accepting that bend's nonlinearity means the deformed control net is an approximation whose tolerance must be checked (knot-refine the surface first so the polygon tracks the bent surface within tolerance).

### Lazarus, Coquillart & Jancene: axial deformations

**Citation.** Lazarus, F., Coquillart, S., & Jancene, P. (1994). Axial deformations: An intuitive deformation technique. *Computer-Aided Design, 26*(8), 607-613.

**Method.** Define an **axis curve** `A(t)` (the "initial axis") threaded through the object. Each object point `X` is bound to the axis by finding its nearest parameter `t(X)` and recording its offset in the **moving frame** of the axis at `t(X)` (a Frenet-like frame: tangent, plus a chosen normal/binormal). The user edits the axis into a **deformed axis** `A'(t)`; the object point is re-placed at the same frame offset on the new axis. Bending, twisting (by rotating the frame about the tangent as a function of `t`), stretching (re-spacing `t`), and tapering (scaling the offset with `t`) all fall out of editing the axis and its frame. The method is independent of the object representation.

**Limitations.** Requires a coherent moving frame; Frenet frames flip at inflection points and are undefined on straight segments, so a rotation-minimizing frame is needed in practice. Points far from the axis deform less predictably; the nearest-point binding can be ambiguous for thick objects.

**Kernel relevance.** Axial deformation is the natural model for **swept and elongated parts** (pipes, beams, ducts, hull keels) where the design intent is "follow this spine." Keel can implement it on NURBS control points with a rotation-minimizing frame (the same frame math the sweeping code needs, nurbs/05). It complements lattice FFD: axis for spine-driven edits, lattice for regional edits.

### Singh & Fiume: Wires, a geometric deformation technique

**Citation.** Singh, K., & Fiume, E. (1998). Wires: A geometric deformation technique. In *Proceedings of SIGGRAPH '98* (pp. 405-414). ACM.

**Method.** A **wire** is a free-floating curve `W` (with a reference copy `W_0`) plus a scalar **radius/influence function** `r` and a deformation magnitude. Each object point is influenced by the wire according to its distance to `W_0` filtered through `r` (points within the radius deform, points outside do not). Deforming the wire (`W_0 -> W`) drags nearby object points by the wire's local displacement, blended by the falloff. Multiple wires combine by a weighted blend of their individual deformations. **Domain curves** localize a wire's effect to a region. Wires sketch object features (a brow ridge, a character line) and deform them directly.

**Limitations.** Blending multiple overlapping wires is heuristic and can produce bulges where influence regions overlap; the falloff is hand-tuned. As a sculpting tool it is excellent but it gives less predictable, less reproducible results than a constrained solve, which matters for parametric CAD.

**Kernel relevance.** Wires are the curve-on-surface analog of FFD and a good model for **character-line editing** in styling. For Keel they are lower priority than lattice/cage/axis (more art-directed than engineering-precise) but the falloff-blend idea is reusable for any localized deformation operator.

---

## Theme 5: Surface-based and variational (physically-based) deformation

This family operates on the **surface mesh / control net directly** by minimizing a deformation energy under user constraints, rather than warping the embedding space. It is mostly mesh literature, but the control-net analog is what matters for B-rep: treat the NURBS control polygon as the "mesh" and minimize a fairness/rigidity energy over it.

### Sorkine & Alexa: as-rigid-as-possible (ARAP) surface modeling

**Citation.** Sorkine, O., & Alexa, M. (2007). As-rigid-as-possible surface modeling. In *Proceedings of the Eurographics Symposium on Geometry Processing (SGP '07)* (pp. 109-116). Eurographics Association.

**Method.** Minimize an energy that penalizes deviation of each one-ring (the star of triangles around a vertex) from a **rigid transformation** of its rest shape:

```
E = sum_i sum_{j in N(i)} w_ij || (p_i' - p_j') - R_i (p_i - p_j) ||^2
```

where `p` are rest positions, `p'` deformed positions, `N(i)` the neighbors of `i`, `w_ij` cotangent weights, and `R_i` the best-fit rotation for vertex `i`'s neighborhood. This is nonlinear (the `R_i` are unknown rotations). Sorkine-Alexa solve it by **local/global alternation**: the **local step** fixes positions and computes each `R_i` by an SVD of the `3x3` covariance matrix of the one-ring edge vectors (`R_i` = the rotation from the SVD); the **global step** fixes the `R_i` and solves a sparse **linear** system (the cotangent Laplacian) for the positions `p'`. Iterating converges and the energy decreases monotonically. The result preserves local detail by allowing rotation but penalizing stretch and shear.

**Limitations.** Mesh-centric: needs a connectivity graph and cotangent weights, so it presumes a discretized surface, not a continuous NURBS face. The energy is detail-preserving but not curvature-fair (it does not minimize bending energy, so it is not a fairing operator). Convergence can be slow for large deformations; degenerate triangles break cotangent weights.

**Kernel relevance.** ARAP is the gold standard for **detail-preserving handle-based deformation** of meshes, so it is directly useful for Keel's *mesh/facet* deformation (file 09, file 05) and for editing a tessellated proxy. For NURBS faces, the analog is an **as-rigid-as-possible / minimal-distortion deformation of the control polygon**, which is worth offering as a high-quality alternative to raw FFD when shape preservation matters and a cage is not available. The local/global SVD-then-linear-solve template is a reusable solver pattern.

### Sorkine et al.: Laplacian surface editing (differential-coordinate deformation)

**Citation.** Sorkine, O., Cohen-Or, D., Lipman, Y., Alexa, M., Rossl, C., & Seidel, H.-P. (2004). Laplacian surface editing. In *Proceedings of the Eurographics/ACM Symposium on Geometry Processing (SGP '04)* (pp. 175-184).

**Method.** Represent the surface in **differential (Laplacian) coordinates** `delta_i = L p_i = p_i - sum_{j in N(i)} w_ij p_j`, which encode local detail (the offset of each vertex from the average of its neighbors). Editing fixes handle vertices to new positions and **reconstructs** the surface by solving the sparse least-squares system `min || L p' - delta' ||^2` subject to the handle constraints, where the `delta'` are the (rotated) differential coordinates. Because the `delta` carry local detail, the reconstruction preserves geometric detail while satisfying the handles. A core subtlety is **rotating the differential coordinates** to follow the deformation (otherwise detail does not rotate with the surface); the paper estimates per-vertex transformations implicitly from the deformation.

**Limitations.** Linearized rotation handling is only first-order accurate, so large rotations distort detail (ARAP's explicit per-vertex SVD rotation fixes this). Mesh-centric, like ARAP.

**Kernel relevance.** Differential-coordinate editing is the linear, single-solve cousin of ARAP and underlies gradient-domain methods. For Keel it is relevant as the theory behind **detail-preserving control-net editing**: the control polygon's discrete Laplacian is a fairing/detail operator, and constrained reconstruction is the fairing solve that nurbs/06 asked for.

### Yu et al.: Poisson mesh editing (gradient-domain manipulation)

**Citation.** Yu, Y., Zhou, K., Xu, D., Shi, X., Bao, H., Guo, B., & Shum, H.-Y. (2004). Mesh editing with Poisson-based gradient field manipulation. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '04), 23*(3), 644-651.

**Method.** Edit the **gradient field** of the coordinate functions rather than positions. Apply local transforms (rotations/scales derived from the handle motion) to the per-triangle gradients of `x,y,z`, producing a target (generally non-integrable) **guidance gradient field** `w`. Reconstruct the deformed coordinates by solving the **Poisson equation** `Delta f = div w` (the Euler-Lagrange equation of `min || grad f - w ||^2`) with boundary conditions from the handles. The discrete system is the cotangent-Laplacian linear solve. This unifies deformation, merging, and smoothing as gradient-field operations.

**Limitations.** Requires a consistent triangle mesh and a scheme to propagate the local transforms across the surface; the result depends on that propagation. Mesh-centric; a single global solve does not localize cheaply.

**Kernel relevance.** Same family and same solver as Laplacian editing/ARAP; for Keel it reinforces that **gradient-domain / Poisson solves over a discretized surface** are the engine for high-quality, detail-preserving deformation and for **seamless merging** of patches (relevant when re-stitching after deformation). It is a mesh tool, useful for Keel's faceted layer and for fairing, not a NURBS primitive on its own.

---

## Theme 6: Emboss and wrap operations

Emboss and wrap are the CAD-feature face of deformation: project a 2D profile onto a curved face and either offset it (emboss/deboss) or map a flat sketch onto the curve (wrap). They combine projection, local offsetting, and a flatten/unflatten map, tying back to the developable/flattening research (kernel/06, kernel/13).

### Emboss / deboss / scribe onto faces

**Citation.** Composite of CAD-system behavior: Autodesk Fusion *Emboss* documentation; Dassault SOLIDWORKS *Wrap* feature (analytical and spline-surface methods); Onshape and SOLIDWORKS emboss/deboss tutorials. (Vendor documentation; the internal algorithms are unpublished.)

**Method.** Given a target face and a profile (text, logo, pattern) on a sketch, the operation: (1) **maps the profile onto the face** by projecting (along a direction or normally) or by a surface map; (2) builds **offset surfaces** of the target face at the emboss depth (positive for raised emboss, negative for deboss, zero for scribe/imprint); (3) **trims** the offset and original surfaces by the mapped profile loop and stitches in **side walls** (ruled or draft-angled surfaces connecting the original face to the offset), producing a watertight local solid feature. "Scribe" stops at step 1+trim: it only imprints the profile as new edges on the face without raising or lowering it (a pure split, no deformation). The hard sub-problems are accurate **offset of a free-form face** (nurbs/05) and a profile mapping that keeps consistent depth and does not self-intersect on high-curvature faces.

**Limitations.** Self-intersection of the offset on regions tighter than the offset radius; profile distortion when projecting (rather than conformally mapping) onto a strongly curved face; degenerate side walls at profile corners. Vendors expose multiple methods (analytical for planar/cylindrical/conical, spline-surface for general faces) precisely because no single method is robust everywhere.

**Kernel relevance.** Emboss/deboss is **offset + trim + stitch**, all kernel primitives Keel already needs (offset from nurbs/05, trim and imprint from local ops file 03, stitch from healing file 13). Keel should implement emboss as a composed operator over those primitives, with the profile-mapping step shared with wrap. Scribe (imprint only) is the cheap special case and a natural first deliverable.

### Wrap: mapping a planar sketch onto a curved face

**Citation.** Composite of CAD-system behavior: SOLIDWORKS *Wrap* (Analytical method for planar/cylindrical/conical faces; Spline-surface method for general faces, limited to <180 degrees of wrap); Fusion/Onshape *Emboss-with-wrap*. Underlying theory: surface flattening / developable-surface parameterization (kernel/06).

**Method.** Wrap is the **inverse of flattening**. To wrap a flat sketch onto a face: (1) compute a **flattening map** of the target face to the plane (for a developable face, cylinder or cone, this is the exact unrolling; for a general face it is a least-distortion parameterization that minimizes area/angle stretch); (2) place the flat sketch in the flattened domain; (3) **invert the map** to carry the sketch curves back onto the curved face, yielding pcurves; (4) feed those pcurves to an emboss/deboss/scribe. The **Analytical method** uses the exact developable unrolling and so is restricted to planar/cylindrical/conical faces where the map is isometric and has no distortion. The **Spline-surface method** uses the surface's own parameterization (or a conformal/low-distortion parameterization) for arbitrary faces but distorts for non-developable faces and breaks down past 180 degrees of wrap (parameterization wraps onto itself).

**Limitations.** For non-developable faces the wrap **must distort** (Gauss's Theorema Egregium: you cannot flatten a doubly-curved surface without stretching), so text/patterns deform; the only choices are which distortion to minimize (angle vs area vs length). Wrap past a half-revolution needs a periodic or seam-aware parameterization.

**Kernel relevance.** Wrap directly reuses Keel's **flattening / developable-surface service** (kernel/06) run in reverse, then composes with emboss. This is a strong argument to build flattening as a bidirectional map (flatten and unflatten) rather than a one-way unroll. For developable faces wrap is exact; for general faces Keel should expose the distortion-metric choice and warn that the map is non-isometric.

---

## Theme 7: B-rep-specific concerns and injectivity

Deforming a *body* (not a mesh or a single surface) raises validity questions that the graphics literature mostly ignores: trim curves, watertightness, tolerances, type promotion, and self-intersection.

### Gain & Dodgson: preventing self-intersection under free-form deformation (injectivity)

**Citation.** Gain, J. E., & Dodgson, N. A. (2001). Preventing self-intersection under free-form deformation. *IEEE Transactions on Visualization and Computer Graphics, 7*(4), 289-298.

**Method.** A deformation map `F` is free of (local) self-intersection iff it is **injective**, and a sufficient condition for local injectivity is that the **Jacobian determinant `det(J)` stays strictly positive** everywhere (the map never folds the space inside-out). Gain and Dodgson start from a clean (self-intersection-free) initial mesh, then derive **conditions on the lattice control-point displacements** that keep `det(J) > 0` for a B-spline/Bezier FFD. Because `det(J)` of a tensor-product FFD is itself a polynomial in `(s,t,u)` whose coefficients are functions of the control points, they bound it (using the convex-hull property of the Bezier form of the Jacobian) and give a **per-step constraint** on how far each control point may move before the Jacobian's lower bound crosses zero. They pair this with **adaptive refinement** of the deformed mesh so the rendered/evaluated geometry tracks the true deformed surface.

**Limitations.** The Jacobian-positivity bound is **sufficient but not necessary** (conservative: it forbids some safe deformations). It prevents *local* fold-over but not *global* self-intersection (two distant parts of the surface colliding), which needs a separate global collision test. The bound is per-step, so large deformations must be applied incrementally.

**Kernel relevance.** This is the **validity certificate Keel needs for any deformation operator**. Keel should: (1) express the deformation Jacobian in Bezier form and check its sign via the convex-hull bound, rejecting or clamping deformations that would make `det(J) <= 0`; (2) treat the positive-Jacobian test as the deformation analog of the orientation/manifold checks the boolean code uses. Combined with a global self-intersection test (surface-surface intersection of the deformed faces, nurbs/04), this gives a sound "is the deformed body valid?" answer.

### Deforming a trimmed-NURBS body while preserving trim and topology

**Citation.** Synthesis of: Sederberg & Parry (1986) control-point pushing; Piegl & Tiller (1997) *The NURBS Book* (knot refinement, affine invariance); and the file-24 / file-13 healing literature.

**Method.** The robust recipe for B-rep deformation that preserves topology and trim:

1. **Type-promote** every analytic face to NURBS (file 24 forward direction). A deformed plane/cylinder is no longer analytic, so it must be NURBS before deforming.
2. **Knot-refine** each face so its control polygon is dense enough that pushing control points through the (possibly nonlinear) deformation map approximates the true deformed surface within the body tolerance. For an affine FFD-of-Bezier of matching degree the control polygon maps exactly; for B-spline FFD or Barr bend the polygon is an approximation whose error shrinks with refinement.
3. **Deform the surface control points** (not tessellation vertices) through the deformation map. Because the deformation is applied per control point and the knot vectors and parameter domain are unchanged, the surface's **parameter domain is invariant**, so all **pcurves / trim loops remain valid by construction** (they live in `(u,v)` space, which did not move). This is the central reason to deform control nets, not sampled points.
4. **Deform shared edges consistently.** An edge is shared by two faces; if each face's control points are pushed independently through the *same* deformation map, the shared edge stays geometrically coincident (the map is single-valued in space), preserving **watertightness**. The 3D edge curve and its two pcurves should be re-evaluated and the edge tolerance re-checked.
5. **Re-validate**: recompute edge/vertex tolerances, run the Jacobian-positivity (injectivity) test, and run a self-intersection check. If watertightness drifted (it should not for a single-valued space map applied to control points, but can for sample-and-refit or for per-face local deformations), re-stitch and tighten gaps (file 13).

**Limitations.** Knot refinement inflates control-point counts (cost grows). Per-face *local* deformations (different map per face) break the shared-edge guarantee and force re-stitching. Strongly nonlinear maps (sharp bend) need heavy refinement to stay in tolerance, or a sample-and-refit fallback that then must repair trim.

**Kernel relevance.** This recipe **is** the deformation contract for Keel. The headline rule: **deform control nets, keep the parameter domain fixed, and trim survives for free**. Type promotion and re-validation bracket the operation. The contrast with naive "tessellate, deform, retriangulate" is stark: the latter destroys exact NURBS faces, loses trim, and produces a faceted body.

---

## Theme 8: Applications and quality (fairness, continuity, shape optimization)

### FFD as the parameterization for shape optimization (CFD, structural, aerospace)

**Citation.** Representative: Samareh, J. A. (2004). Aerodynamic shape optimization based on free-form deformation. *AIAA 2004-4630*. And: Sieger, D., Menzel, S., & Botsch, M. (2015). On shape deformation techniques for simulation-based design optimization. And: hull-FFD studies (e.g. *J. Marine Sci. Eng., 10*(5):686, 2022, cited in nurbs/06).

**Method.** FFD provides a **low-dimensional shape parameterization** for expensive analysis loops. A lattice envelopes the region of interest (wing section, hull bow); only a few interior control points are activated as **design variables** (often <50), while boundary control points are frozen to preserve continuity with the rest of the body and watertightness. Each optimization iteration moves the design control points, deforms the **CFD/FEA volume mesh** by the same FFD map (so re-meshing is avoided, just morph the existing mesh), and runs the analysis. Gradient-based optimization needs the **derivative of the mesh with respect to the design variables**, which for FFD is just the (constant, precomputed) Bernstein/B-spline basis matrix `B`, making sensitivities cheap. Mesh morphing of millions of vertices runs in seconds versus re-meshing.

**Limitations.** Lattice choice biases the achievable shapes (a coarse lattice cannot express fine features; a fine lattice has too many variables). FFD couples regions a designer might want independent. Frozen-boundary continuity is `C^0`/`C^1` only if the lattice rows are matched. Large deformations can invalidate the morphed mesh (negative-volume cells), tying back to the Jacobian-positivity requirement.

**Kernel relevance.** This is the **primary engineering justification** for FFD as a kernel service (nurbs/06's conclusion). Keel should expose: (1) FFD with selectable active/frozen control points; (2) the basis matrix `B` as a queryable sensitivity (so an external optimizer can get shape derivatives); (3) the same FFD map applied to *both* the NURBS body and any associated analysis mesh, keeping them consistent. The frozen-boundary convention is how watertightness and continuity-to-the-rest-of-the-body are maintained during optimization.

### Continuity, fairness, and curvature preservation under deformation

**Citation.** Synthesis of: Barr (1984) Jacobian/normal rule; Sederberg-Parry (1986) inherited continuity; the fairing literature in nurbs/06 (strain-energy minimization); Class-A surfacing practice.

**Method.** Polynomial deformations **inherit the smoothness of the basis**: a `C^k` FFD map keeps a `C^k` surface `C^k`. Maintaining `G1/G2` *between* faces under deformation requires the deformation to act **consistently across the shared edge** (same map on both sides, plus matched cross-boundary derivatives), which the control-net recipe (Theme 7) provides for a single global map but not for independent per-face deformations. **Fairness** (low, smoothly varying curvature, the ship-hull and Class-A requirement) is **not automatic** under deformation: an aggressive FFD can introduce curvature wiggles. The remedy is to follow deformation with a **fairing pass** (minimize strain energy `integral (kappa_1^2 + kappa_2^2) dA` or a bending-energy surrogate over the control net, nurbs/06), or to constrain the deformation itself to a fairness-preserving subspace. Curvature combs / porcupine plots are the diagnostic.

**Limitations.** Fairing and shape-fidelity trade off; over-fairing erases intended features. `G2` across edges under deformation is fragile and may need an explicit cross-boundary continuity solve after deforming.

**Kernel relevance.** Keel's deformation service should be paired with the **fairing/energy-minimization operator** nurbs/06 already called for, and should expose **curvature evaluation** as a query so callers can diagnose fairness after a deformation. For styling-critical (Class-A, hull) work, offer Green-coordinate or ARAP-style shape-preserving deformation (Themes 3, 5) as the low-distortion alternative to raw FFD, and run fairing as a finishing pass.

---

## Deformation services for Keel (synthesis)

**Which deformation families to expose.** A Parasolid-class kernel should offer a small, layered deformation toolkit rather than one monolithic operator:

1. **Lattice FFD** (B-spline lattice default, single-Bezier-block and EFFD conforming-lattice variants). The workhorse for regional free-form deformation and the parameterization for shape optimization. Sederberg-Parry + Griessmair-Purgathofer + Coquillart.
2. **Direct manipulation** over the lattice (Hsu-Hughes-Kaufman pseudoinverse solve), so users push surface points and the kernel finds the control-point change. This is the expected CAD interaction.
3. **Global operators** (Barr bend / twist / taper / scale), the named Parasolid-audit operators, implemented as closed-form maps over control points with Barr's Jacobian-based normal rule.
4. **Axial / curve-driven deformation** (Lazarus axial, optionally Singh-Fiume wires) for spine-driven edits of elongated parts, sharing rotation-minimizing-frame code with sweeping.
5. **Cage-based deformation** with a choice of coordinate scheme: **Green coordinates** for shape-preserving (Class-A/hull) edits, **harmonic coordinates** for well-behaved concave cages, **mean value coordinates** for the fast solve-free baseline.
6. **Variational / detail-preserving deformation** (ARAP, Laplacian/Poisson) on the faceted layer and as a high-quality control-net option, plus the **fairing** energy-minimization operator nurbs/06 asked for.
7. **Emboss / deboss / scribe and wrap** as composed operators (offset + trim + stitch, plus the bidirectional flattening map from kernel/06), not new primitives.

**How they interact with control nets, trim, and tolerance.** The invariant across all families is **deform the control net, keep the parameter domain fixed**. Because pcurves and trim loops live in `(u,v)` space and the knot vectors/domain do not move, **trim survives deformation by construction**. Shared edges stay coincident because a single-valued space map applied to both adjacent faces' control points moves them identically along the seam, preserving **watertightness**. Tolerances must be **re-checked** after deformation (edge fit, vertex coincidence) and tightened (file 13) if a per-face or local deformation drifted them.

**Type-promotion contract.** Any analytic face (plane, cylinder, cone, sphere, torus) that is deformed **must be promoted to NURBS first**, because a deformed analytic surface is generally free-form. Deformation is the inverse of canonical-geometry recovery (file 24): it erases analytic intent. After a deformation that happens to land back on an analytic shape (rare, e.g. a pure translation), the recovery service can re-detect and restore the analytic form. Promotion typically requires **knot refinement** so the deformed control polygon tracks the true deformed surface within tolerance for nonlinear maps (B-spline FFD, bend).

**Re-validation contract.** Every deformation operator returns a body only after: (1) **injectivity** is established by the Gain-Dodgson Jacobian-positivity test (Bezier-form bound on `det(J) > 0`), rejecting or incrementally clamping fold-over; (2) a **global self-intersection** check (deformed surface-surface intersection, nurbs/04) rules out distant collisions; (3) **watertightness and tolerances** are re-verified and re-stitched/tightened if needed (file 13); (4) **normals** are transformed by Barr's `det(J) (J^{-1})^T` rule so offsets, draft, and shading stay correct; (5) optionally a **fairing** pass and curvature diagnostic for styling-critical surfaces. This five-point contract is what makes deformation a *kernel* service rather than a UI gesture, and it is the gap the graphics literature leaves for a CAD kernel to fill.

---

## References

1. Sederberg, T. W., & Parry, S. R. (1986). Free-form deformation of solid geometric models. *Computer Graphics (Proceedings of SIGGRAPH '86), 20*(4), 151-160.
2. Barr, A. H. (1984). Global and local deformations of solid primitives. *Computer Graphics (Proceedings of SIGGRAPH '84), 18*(3), 21-30.
3. Griessmair, J., & Purgathofer, W. (1989). Deformation of solids with trivariate B-splines. In *Proceedings of Eurographics '89* (pp. 137-148). North-Holland.
4. Coquillart, S. (1990). Extended free-form deformation: A sculpturing tool for 3D geometric modeling. *Computer Graphics (Proceedings of SIGGRAPH '90), 24*(4), 187-196.
5. MacCracken, R., & Joy, K. I. (1996). Free-form deformations with lattices of arbitrary topology. In *Proceedings of SIGGRAPH '96* (pp. 181-188). ACM.
6. Hsu, W. M., Hughes, J. F., & Kaufman, H. (1992). Direct manipulation of free-form deformations. *Computer Graphics (Proceedings of SIGGRAPH '92), 26*(2), 177-184.
7. Lamousin, H. J., & Waggenspack, W. N. (1994). NURBS-based free-form deformations. *IEEE Computer Graphics and Applications, 14*(6), 59-65.
8. Ju, T., Schaefer, S., & Warren, J. (2005). Mean value coordinates for closed triangular meshes. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '05), 24*(3), 561-566.
9. Joshi, P., Meyer, M., DeRose, T., Green, B., & Sanocki, T. (2007). Harmonic coordinates for character articulation. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '07), 26*(3), 71.
10. Lipman, Y., Levin, D., & Cohen-Or, D. (2008). Green coordinates. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '08), 27*(3), 78.
11. Lazarus, F., Coquillart, S., & Jancene, P. (1994). Axial deformations: An intuitive deformation technique. *Computer-Aided Design, 26*(8), 607-613.
12. Singh, K., & Fiume, E. (1998). Wires: A geometric deformation technique. In *Proceedings of SIGGRAPH '98* (pp. 405-414). ACM.
13. Sorkine, O., & Alexa, M. (2007). As-rigid-as-possible surface modeling. In *Proceedings of the Eurographics Symposium on Geometry Processing (SGP '07)* (pp. 109-116).
14. Sorkine, O., Cohen-Or, D., Lipman, Y., Alexa, M., Rossl, C., & Seidel, H.-P. (2004). Laplacian surface editing. In *Proceedings of the Eurographics/ACM Symposium on Geometry Processing (SGP '04)* (pp. 175-184).
15. Yu, Y., Zhou, K., Xu, D., Shi, X., Bao, H., Guo, B., & Shum, H.-Y. (2004). Mesh editing with Poisson-based gradient field manipulation. *ACM Transactions on Graphics (Proceedings of SIGGRAPH '04), 23*(3), 644-651.
16. Gain, J. E., & Dodgson, N. A. (2001). Preventing self-intersection under free-form deformation. *IEEE Transactions on Visualization and Computer Graphics, 7*(4), 289-298.
17. Samareh, J. A. (2004). Aerodynamic shape optimization based on free-form deformation. *AIAA Paper 2004-4630* (10th AIAA/ISSMO Multidisciplinary Analysis and Optimization Conference).
18. Sieger, D., Menzel, S., & Botsch, M. (2015). On shape deformation techniques for simulation-based design optimization. In *New Challenges in Grid Generation and Adaptivity for Scientific Computing* (SEMA SIMAI Springer Series).
19. Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer.
20. Perez-Arribas, F., et al. (2022). FFD-based and NURBS-based parametric ship hull form studies. *Journal of Marine Science and Engineering, 10*(5), 686. (And related hull FFD/fairing literature, cross-referenced from nurbs/06.)
21. Sederberg, T. W. (n.d.). *Free-Form Deformations* (course notes), and UC Davis IDAV On-Line Computer Graphics Notes: Free-Form Deformations. (Tutorial expositions of the Sederberg-Parry math.)
22. Dassault Systemes SOLIDWORKS, Autodesk Fusion, and Onshape product documentation on Emboss/Deboss/Scribe and Wrap (analytical and spline-surface methods). (Vendor behavioral documentation; internal algorithms unpublished.)
