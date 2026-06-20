# Methodology and References

This document records the methods Keel actually implements and the literature they
follow, mapped to the module where each is used. It is curated against the source, not
from memory: every entry below was confirmed in the code at the cited location.

Phrasing convention: "follows X" / "per X" means Keel implements an established method
from the literature. Keel claims the engineering and the integration, not the invention
of these methods. Where a method is standard textbook material, no paper is attributed.

---

## Methodology

**Exact combinatorics over tolerant metric geometry.** The core design inversion:
combinatorial topology decisions (which edge bounds which face, which side of a surface a
point lies on) are certified with exact or filtered-exact predicates, while metric
geometry (coordinates, parameter values) uses `f64` with explicit per-entity tolerances
from a single tolerance policy. This is the exact-geometric-computation (EGC) discipline:
construct from `f64` treated as the exact dyadic rationals it is, decide with an exact
battery behind a cheap floating-point filter. (`keel-math/src/algebraic.rs`,
`keel-math/src/predicates.rs`, `keel-math/src/tolerance.rs`.)

**Validation against exact closed-form references.** Correctness is checked by a
three-bucket oracle (PASS / DECLINE / WRONG) in which PASS requires agreement with an
*independently* derived reference, ideally an exact closed form rather than a second
approximate code path. This removes the differential-testing weakness of trusting the
test itself. Example: the Steinmetz bicylinder (two equal perpendicular cylinders) has
intersection volume exactly 16/3 with no factor of pi; the kernel assembles and
integrates it to that value. (`keel-topo/tests/`, `keel-topo/examples/realsoak.rs`.)

---

## Numeric foundation (`keel-math`)

- **Exact geometric predicates** follow Shewchuk's adaptive-precision orientation
  predicates, used here via the `robust` crate behind a certified `Sign` enum (the
  filtered-predicate / EPICK discipline). Confirmed: `src/predicates.rs` (`orient_2d`,
  `orient_3d`, Shewchuk's below-plane convention noted in tests).
  - J. R. Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust
    Geometric Predicates," Discrete & Computational Geometry, 1997.

- **Exact comparison of one-root algebraic numbers** (coordinates of circle/circle and
  same-family conic intersections, of the form (a + b sqrt(c)) / d over exact integers)
  follows the Devillers-Fronville-Mourrain-Teillaud comparison recipe: decisions reduce to
  sign batteries on integer expressions, with no square root ever evaluated exactly. The
  API is shaped after the CGAL algebraic kernel's `Ak_1` (isolate / compare / sign_at /
  approximate). Confirmed: `src/algebraic.rs` (`OneRoot`), `src/bigint.rs`.
  - O. Devillers, A. Fronville, B. Mourrain, M. Teillaud, "Algebraic methods and
    arithmetic filtering for exact predicates on circle arcs," Computational Geometry,
    2002.

- **Interval arithmetic** is conservative, with one-ulp outward widening so the true real
  result of any operation on members of the operand intervals is contained in the result.
  Confirmed: `src/interval.rs`.

- **Polynomial roots.** The quadratic uses the cancellation-avoiding stable form (Blinn /
  Numerical Recipes). The cubic uses a monotonic-interval strategy (Yuksel). Confirmed:
  `src/poly.rs` (`solve_quadratic`, `solve_cubic`).
  - J. Blinn, "How to Solve a Quadratic Equation," IEEE CG&A, 2005-2006.
  - C. Yuksel, "High-Performance Polynomial Root Finding for Graphics," Proc. ACM on
    Computer Graphics and Interactive Techniques (HPG), 2022.

- **Bernstein-basis arithmetic and root isolation.** Exact Bernstein product per
  Farouki-Rajan, with variation-diminishing root isolation by subdivision. Confirmed:
  `src/bernstein.rs`.
  - R. T. Farouki, V. T. Rajan, "Algorithms for polynomials in Bernstein form,"
    Computer Aided Geometric Design, 1988.

- **Multivariate polynomial system solving** uses the Projected Polyhedron global root
  solver over the unit box. Confirmed: `src/multibernstein.rs` (`solve_system`).
  - E. C. Sherbrooke, N. M. Patrikalakis, "Computation of the solutions of nonlinear
    polynomial systems," Computer Aided Geometric Design, 1993.

- **Univariate refinement** uses a safeguarded (bracketed/hybrid) Newton iteration.
  Confirmed: `src/newton.rs` (`solve_bracketed`).

- **Rotations** use Rodrigues' rotation formula (textbook). Confirmed: `src/transform.rs`.

---

## Curves and surfaces (`keel-geom`)

- **NURBS evaluation** uses in-place de Boor corner cutting (The NURBS Book algorithm
  A3.1) over homogeneous 4D control points, with Bezier decomposition by knot insertion.
  Confirmed: `src/nurbs_curve.rs`, `src/basis.rs`.
  - L. Piegl, W. Tiller, "The NURBS Book," 2nd ed., Springer, 1997.

- **Global closest-point projection** decomposes to Bezier segments, prunes with an AABB
  branch-and-bound, and polishes with bracketed Newton. Confirmed: `src/curve.rs`.

- **Local differential geometry** (first and second fundamental forms, principal /
  Gaussian / mean curvature) follows standard differential-geometry-of-surfaces results
  (textbook). Confirmed: `src/surface.rs` and the surface local-geometry path.

---

## Topology and booleans (`keel-topo`)

- **Non-manifold B-rep** uses a radial-edge representation (Weiler) with first-class
  space-partitioning regions. The boolean region extraction is a Weiler radial sector
  walk. Confirmed: `src/entity.rs`, `src/boolean.rs`.
  - K. Weiler, "The Radial Edge Structure: A Topological Representation for Non-Manifold
    Geometric Boundary Modeling," in Geometric Modeling for CAD Applications, 1988.

- **Regularized set operations.** Boolean results are regularized r-sets, with the on-on
  (coincident-face) selection resolved by the Requicha / Laidlaw-Trumbore-Hughes selection
  tables. Confirmed: `src/boolean.rs` (regularized selection tables).
  - A. A. G. Requicha, H. B. Voelcker, "Boolean operations in solid modeling," Proc.
    IEEE, 1985.
  - D. H. Laidlaw, W. B. Trumbore, J. F. Hughes, "Constructive Solid Geometry for
    Polyhedral Objects," SIGGRAPH, 1986.

- **Point and region classification** uses the generalized winding number as the primary,
  surface-type-agnostic classifier: w(p) = (1/4pi) times the sum of signed solid angles of
  the outward-oriented boundary, which is ~1 inside, ~0 outside, and degrades gracefully
  near the surface where ray-cast classification is fragile. The per-triangle signed solid
  angle uses the Van Oosterom-Strackee formula. Confirmed: `src/winding.rs`
  (`gwn_over`, `tri_solid_angle`).
  - A. Jacobson, L. Kavan, O. Sorkine-Hornung, "Robust Inside-Outside Segmentation using
    Generalized Winding Numbers," ACM Transactions on Graphics (SIGGRAPH), 2013.
  - A. Van Oosterom, J. Strackee, "The Solid Angle of a Plane Triangle," IEEE
    Transactions on Biomedical Engineering, 1983.

---

## Interrogation

- **Mass properties** (volume, area, centroid, inertia) are computed by converting the
  volume integral to a boundary surface-flux integral via the divergence theorem (the
  "green-slab" integrator), evaluated analytically per face. This is standard vector
  calculus (Gauss / Green); no paper is attributed. The integrator is proven correct and
  locked by a regression test. Confirmed: `src/massprops.rs`,
  `tests/cyl_union_mass_witness.rs`.

---

## Notes

- This list covers the methods with a clear external reference. Standard textbook
  techniques (Rodrigues rotation, divergence-theorem integration, fundamental forms) are
  noted as such without attribution.
- If a citation here ever drifts from the code, the code is the source of truth; please
  open an issue. The README's high-level Architecture section summarizes the same material
  in prose; `docs/ARCHITECTURE.md` is the deeper source-grounded overview.
