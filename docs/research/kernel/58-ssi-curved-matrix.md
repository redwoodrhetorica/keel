# 58. Curved surface-surface-intersection matrix: completing the analytic SSI grid for the overlapping-curved boolean frontier

## Title and scope

This dossier is the implementation-grade literature foundation for **filling the missing analytic surface-surface-intersection (SSI) rungs** that currently fall through Keel's `analytic_analytic` dispatch in `crates/keel-geom/src/ssi.rs` and leave the boolean engine with no seam. Keel today implements, exactly and certified, only: plane-plane (line), plane-sphere (circle), sphere-sphere (circle), plane-cylinder (ellipse / line-pair / circle), cylinder-cylinder (the certified closed-form per-ruling branch field, `q2=1`), and plane-cone (the conic family: circle / ellipse, with parabola and hyperbola declined). Every other curved pair returns `GeomError::Degenerate`, so the boolean finds no intersection seam and either keeps both operands whole or produces a wrong body. The missing rungs are: **cylinder-sphere, cone-sphere, cone-cylinder, cone-cone, and the entire torus row** (torus-plane, torus-cylinder, torus-sphere, torus-cone, torus-torus).

This is the single highest-leverage gap on the curved frontier: it is the difference between "Keel can boolean two analytic solids whenever their surfaces happen to be a plane, sphere, or two parallel/equal cylinders" and "Keel can boolean any pair of natural-quadric-or-torus solids." The contract is the project standard: **DECLINE-never-WRONG**. A rung is allowed to decline a configuration (the boolean reports no result, conservatively), but it must never emit a seam that is geometrically wrong or, worse, drop a component so the boolean silently under-cuts.

The dossier does **not** repeat the exact-predicate substrate (file 11), canonical recovery (file 24), the blend-surface generation ladder (file 40), the coincident/tangent boolean handling (file 39), or the non-manifold boolean topology (file 57). It builds on the cylinder-cylinder certified-fit pattern already in the code and on file 11's algebraic-number wall, and reuses the existing tier-2/tier-3 recovered-NURBS seam path (the bivariate-Bernstein implicitization tracer in `ssi.rs`) as the fallback for everything that has no closed form.

The structure mirrors files 01 to 57: annotated source blocks (Citation / Method / Limitations / Kernel relevance), explicit verified-vs-claimed labels, patent flags, and a closing synthesis with ADOPT / AVOID / INVESTIGATE, a first milestone with an exact oracle, and the follow-up ladder. The design recommendation at the end is to be validated against Keel's `ssi.rs` test module (the `check_curve_on_both_tol` implicit-residual harness already present).

A note on degrees, used throughout. Two quadrics (plane is degree 1, the natural quadrics sphere/cylinder/cone are degree 2) intersect in general position in a **degree-4 (quartic) space curve**. A torus is a degree-4 surface, so torus-quadric is degree 8 and torus-torus is degree 16 in general position. The whole engineering problem is: which configurations collapse that high-degree curve to something Keel's `Curve3` (Line / Circle / Ellipse / NURBS) can hold exactly, which collapse to something a certified NURBS fit can hold to tolerance, and which need a general tracer.

---

## 1. Cylinder-sphere

### 1.1 The case taxonomy

The intersection of a right circular cylinder (radius `r_c`, axis `a`) and a sphere (center `S`, radius `r_s`) is in general a **quartic space curve** (verified, multiple sources). The degenerate sub-cases:

- **Coaxial / axis-through-center**: when the sphere center `S` lies on the cylinder axis. Substituting the cylinder constraint `dist_to_axis = r_c` into the sphere equation collapses the axial coordinate to `z = z_S +/- sqrt(r_s^2 - r_c^2)`, giving **0, 1, or 2 exact circles** of radius `r_c` centered on the axis at those two heights (verified: "if the center of the sphere lies on the axis of the cylinder of revolution, then the intersection curve degenerates into two circles"). One circle (tangent) when `r_s = r_c` at the equator, i.e. `z_S` on axis and `r_s = r_c` gives a single circle; `r_s < r_c` gives empty; `r_s > r_c` gives two circles.
- **Tangent (grazing)**: the surfaces share a common tangent plane along the curve. The quartic acquires a **double point**; the two branches pinch to a figure-eight or a single self-touching loop. This is the soundness-critical case (section 5).
- **General position**: the sphere center is off-axis. The curve is one of: a single closed loop (sphere pokes partway through the cylinder wall, "partial intersection"), two disjoint closed loops (sphere passes fully through, "complete intersection," the two loops mirror-images across the plane through `S` perpendicular to... actually across the plane containing the axis and `S`), or a one-double-point curve at the tangent transition (verified taxonomy: "one-branch curve in the case of partial intersection, a two-branch curve in the case of complete intersection or a curve with one double point if the surfaces have a common tangent plane").

> **Citation.** *Sphere-cylinder intersection*, Wikipedia / Scientific Library mirror, and Wolfram MathWorld, *Cylinder-Sphere Intersection*. URLs: https://en.wikipedia.org/wiki/Sphere%E2%80%93cylinder_intersection , https://mathworld.wolfram.com/Cylinder-SphereIntersection.html . Viviani's curve (the `r_s = 2 r_c`, center-on-surface special case) is the classical named instance: https://www.geogebra.org/m/cQRfypKU .
> **Method / Content.** Put the cylinder in its own frame: a surface point is `P(theta, v) = O_c + r_c(cos theta e_x + sin theta e_y) + v a`. This is the cylinder's **ruling parameterization** (each `theta` fixes a ruling line, `v` runs along it). Substitute into the sphere implicit `|P - S|^2 - r_s^2 = 0`. Because `P` is **linear in `v`**, the result is a **quadratic in `v`** for each `theta`: `v^2 + 2 b(theta) v + c(theta) = 0`, with `b(theta) = (Q(theta) - S) . a` and `c(theta) = |Q(theta) - S|^2 - r_s^2`, where `Q(theta)` is the rim point `O_c + r_c(cos theta e_x + sin theta e_y)` (the `a` axis is a unit vector so the leading `v^2` coefficient is exactly 1). The two roots `v(theta) = -b +/- sqrt(b^2 - c)` are the two branches. The discriminant `D(theta) = b^2 - c` is itself a trigonometric polynomial of degree 2 in `(cos theta, sin theta)`; its sign over `theta in [0, 2pi)` fixes the branch topology exactly as cylinder-cylinder's `D(theta)` already does in `ssi.rs`.
> **Limitations.** Viviani and the named special cases are documented; the general off-axis case is described qualitatively in the encyclopedic sources, not with a packaged closed-form parameterization. The closed form above (verified by direct substitution, reconstructed here) is the same structure Keel already ships for cylinder-cylinder, so it is not novel and carries no patent risk.
> **Kernel relevance.** This is **the** observation that makes cylinder-sphere a drop-in for Keel: it is structurally identical to the existing `cylinder_cylinder` Rung 3. The cylinder is parameterized by its ruling; the *other* surface (here a sphere instead of a cylinder) is substituted to give a per-`theta` quadratic in the ruling parameter `v`; the discriminant `D(theta)` classifies the branches; each branch is a smooth periodic curve fit by the existing certified-NURBS `fit_curve` closure. The only change from `cylinder_cylinder` is the `coeffs(theta)` closure body.

### 1.2 Representation and branch / arc-sweep metadata

- **Coaxial**: emit exact `Curve3::Circle` (one or two), `closed: true`, `tol_achieved: 0.0`. This is the honest exact oracle for the rung.
- **General**: recover each branch as a certified `Curve3::Nurbs` by the cylinder-cylinder ladder (Chebyshev-spaced samples of `eval_branch`, LSQ cubic fit, deviation certificate `x 2` safety, escalate control count, DECLINE if the certificate misses `tol`). The quartic is smooth and periodic in `theta`, so the `mid + half*cos(phi)` regularizing substitution already in `ssi.rs` for the bite ends applies verbatim where `D(theta)` has zeros.
- **Branch / arc-sweep metadata for the stitch**: the `SsiCurve` already carries `closed` and `tangential`. The downstream stitch needs **shared cross-operand seam edges to compare EQUAL**. For the exact-circle coaxial case this is automatic (two circles, deterministic from the axis and radii, identical from both operands' point of view). For the recovered-NURBS general case, the seam edge is the *same fitted curve object* shared between the two faces (cylinder face and sphere face), so equality is identity, not numerical comparison: the SSI is computed once and both half-edges reference it. The mass integrator's "point + tangent at parameter `t`" is served by the NURBS `point(t)` and its first derivative, both already implemented.

---

## 2. Cone-sphere, cone-cylinder, cone-cone

These are the remaining quadric-quadric rungs. All three are degree-4 in general position with rich degenerate sub-cases. The governing literature is the **natural-quadric geometric school** (Miller; Miller-Goldman; Shene-Johnstone) plus the **algebraic pencil school** (Levin; Wang-Goldman-Tu / QI).

### 2.1 The natural-quadric geometric approach (the ADOPT spine)

> **Citation.** J. R. Miller, "Geometric approaches to nonplanar quadric surface intersection curves," *ACM Transactions on Graphics* 6(4), 1987, pp. 274-307. https://dl.acm.org/doi/abs/10.1145/35039.35041 . J. R. Miller and R. N. Goldman, "Geometric algorithms for detecting and calculating all conic sections in the intersection of any two natural quadric surfaces," *Graphical Models and Image Processing* 57(1), 1995. https://www.researchgate.net/publication/222480796 . C.-K. Shene and J. K. Johnstone, "On the lower degree intersections of two natural quadrics," *ACM Transactions on Graphics* 13(4), 1994. https://dl.acm.org/doi/10.1145/195826.197316 .
> **Method / Content.** The natural quadrics are sphere, right circular cylinder, right circular cone. The geometric school exploits their defining property: each is a **surface of revolution about an axis with a simple distance-to-axis profile** (constant for cylinder, linear for cone, the sphere a degenerate case). The central results Keel needs:
> - **All planar (conic) components are detected and computed using only degree-<=2 solves**, never a degree-4 polynomial (verified: Miller-Goldman "do not require solutions to polynomials of degree higher than 2"). The configurations that yield a planar conic are special and frequent in CAD (coaxial, equal-half-angle cones, shared apex, tangency).
> - **Coaxial sphere-cone**: the cone's distance-to-axis is `rho(z) = |z - z_apex| tan(alpha)`; intersect with the sphere's `rho(z)^2 = r_s^2 - (z - z_S)^2`. Both sides are quadratic in `z`, so `z` solves a quadratic, and each real root gives an **exact circle** of radius `rho(z)` at that height. 0, 1, or 2 circles. This is the exact oracle for cone-sphere.
> - **Coaxial cone-cylinder**: cylinder `rho = r_c` (constant); cone `rho(z) = |z - z_apex| tan(alpha)` (linear). Setting equal gives **one or two heights `z`** linearly, each an **exact circle** of radius `r_c`. Exact oracle for cone-cylinder.
> - **Equal-half-angle cone-cone with parallel axes**: the two cones differ by a translation; the intersection lies in the **plane that bisects them** and is a **conic** (a hyperbola or ellipse or line-pair depending on the offset). When the axes are parallel and the half-angles equal, the quartic factors and one factor is a plane, so the residual is a plane conic (Shene-Johnstone "lower degree intersections": the degenerations are exactly enumerated).
> - **Shared-apex cone-cone (concentric)**: two cones with a common apex intersect in **lines through the apex** (0, 1, or 2 rulings) or coincide. Pure line solve.
> - **Tangent cone-cone / cone-cylinder / cone-sphere**: the quartic gains a double point; geometrically this is a shared tangent plane along a curve that pinches. Detected by the discriminant going to a tangency.
> **Limitations.** The geometric algorithms are **case-by-case**: each pair of quadric types has its own detector and its own conic extractor. Miller-Goldman's own framing is that the planar-conic cases are "very special"; the *general* (non-coaxial, unequal) configuration is still a genuine quartic that the geometric method hands off to a marcher or to the algebraic method. The completeness of the degenerate enumeration is the value; the general case is not closed-form here.
> **Kernel relevance.** This is the **spine of Keel's cone rungs**. The plan: each rung first runs the *geometric degenerate detector* (coaxial? equal-angle? shared-apex? tangent?) using only sqrt-and-quadratic solves, and when it fires, emits an **exact `Curve3::Circle` / `Curve3::Line` / `Curve3::Ellipse`**. When no degeneracy fires, the rung falls to the **per-ruling closed-form branch field** (section 2.3) exactly like cylinder-cylinder. This gives Keel the same DECLINE-never-WRONG posture: the exact branches are exact, the general branches are certified-NURBS, and anything whose certificate misses declines.

### 2.2 The algebraic pencil approach (Levin, QI): the completeness reference, not the shipping path

> **Citation.** J. Z. Levin, "A parametric algorithm for drawing pictures of solid objects composed of quadric surfaces," *Comm. ACM* 19(10), 1976; and "Mathematical models for determining the intersections of quadric surfaces," *CGIP* 11, 1979. Y.-K. Choi, W. Wang, et al., "Enhancing Levin's method for computing quadric-surface intersections," *CAGD* 20(7), 2003. https://i.cs.hku.hk/~ykchoi/quadrics/CAGD_Levin_Enhanced.pdf . W. Wang, R. Goldman, C. Tu, "Enhancing Levin's method..." and the QI work: L. Dupont, D. Lazard, S. Lazard, S. Petitjean, "Near-optimal parameterization of the intersection of quadrics," parts I-III; implemented as **QI** (Lazard, Penaranda, Petitjean, 2006). https://inria.hal.science/inria-00104003/document , https://gamble.loria.fr/qi/ .
> **Method / Content.** Levin: in the **pencil** `lambda A + mu B` of two quadrics there always exists a **ruled quadric** (verified: "there always exists a ruled quadric in the pencil"). Parameterize that ruled quadric by its family of lines; each line meets the other quadric in a **quadratic** (degree 2), giving the intersection as `s -> (line(s), root of a quadratic in the ruling)`, i.e. a parameterization involving **one square root of a degree-4 polynomial** in `s`. The enhanced version classifies the morphology (number/type of components) and yields a **rational** parameterization exactly when the quartic is singular (a singular quartic, a cubic + line, or two conics). QI makes this **exact** over rational input coefficients: the parameterization is rational whenever one exists, and otherwise involves `sqrt(poly)` with a **near-minimal number of square roots** (verified: "the number of square roots... is minimal except in a small number of cases where there may be an extra square root").
> **Limitations.** Levin's original is "numerically sensitive and based on solutions to fourth-degree polynomial equations" (verified). QI is exact but heavy: it is a full algebraic-geometry engine over arbitrary-precision rationals (GMP), GPL-class licensing, C++, and its output is a `sqrt(quartic)` parameterization, **not** a Keel `Curve3`. Adopting QI wholesale would import file 11's algebraic-number wall into the SSI path.
> **Kernel relevance.** This is the **completeness oracle and the morphology dictionary**, not the shipping code. Keel uses Levin/QI two ways: (1) as the *theoretical guarantee* that the per-ruling-quadratic structure (one square root of a degree-4 in the ruling parameter) is the **correct and general** shape of every quadric-quadric branch, which justifies the cylinder-cylinder pattern as the universal quadric-quadric template; (2) as the **branch-count certificate source**: QI's signature-sequence classification (next source) tells Keel, for given input, how many components of which type to expect, which is the soundness check that no near-tangent component is dropped.

> **Citation.** W. Wang, B. Joe, R. Goldman, "Computing quadric surface intersections based on an analysis of plane cubic curves," *Graphical Models* 64, 2002; and the signature-sequence classifier: C. Tu, W. Wang, B. Mourrain, et al., "Using signature sequences to classify intersection curves of two quadrics," *CAGD*, 2009. https://i.cs.hku.hk/~ykchoi/quadrics/cagd_qsic.pdf . Topological classifier: "Topological classification of the intersection curves of two quadrics using a set of discriminants," *CAGD* 2023. https://www.sciencedirect.com/science/article/abs/pii/S0167839623000766 .
> **Method / Content.** The **characteristic polynomial** `det(lambda A + mu B)` of the pencil is a degree-4 binary form; the **signature sequence** of the quadrics in the pencil (the inertia of `lambda A + mu B` as `lambda:mu` varies through the real roots of the characteristic polynomial) determines the **morphology of the intersection curve** completely: number of real components, whether it is a smooth quartic, a nodal/cuspidal quartic, a cubic-plus-line, two conics, two lines, a conic-plus-two-lines, etc. The 2023 discriminant set reduces this to evaluating signs of a fixed set of discriminant polynomials in the coefficients.
> **Limitations.** Exact classification needs exact sign evaluation of the discriminants (file 11 territory) to be certified; in floating point it is a robust *predictor* but not a proof.
> **Kernel relevance.** This is the **branch-count certificate**. For each cone rung, Keel can compute the characteristic polynomial of the (implicit) quadric pair cheaply, evaluate the discriminant signs with an interval filter, and obtain the **expected component count and types**. The marcher/fitter must then produce exactly that many components; a mismatch means a grazing component was missed and the rung **DECLINEs** rather than emit an under-cut. This is the cone-rung analog of cylinder-cylinder's `D(theta)` certified-branch guarantee, lifted to the pairs where there is no single clean `theta` discriminant.

### 2.3 The shipping form for cone rungs: per-ruling quadratic on the cone

The cone has a ruling parameterization too: `P(theta, v) = apex + v (cos alpha (cos theta e_x + sin theta e_y) + sin alpha a)`, where each `theta` selects a ruling line through the apex and `v` runs along it. Substituting this into the **other surface's** implicit:

- **into a sphere** (`|P - S|^2 = r_s^2`): linear-in-`v` point gives a **quadratic in `v`**, discriminant `D(theta)`. Identical structure to cylinder-sphere. (Verified by substitution.)
- **into a cylinder** (`dist_to_other_axis^2 = r_c^2`): again `P` linear in `v`, quadratic in `v`. (Verified.)
- **into another cone** (`(dist_to_other_axis)^2 = ((z - z_apex2) tan alpha2)^2`): both sides quadratic in `v`, so the residual is a **quadratic in `v`** (the `v^2` terms have `theta`-dependent coefficients now, so the leading coefficient `q2(theta)` is no longer constant 1, unlike cylinder-cylinder's `q2=1`). The discriminant is `q1^2 - 4 q2 q0`. (Verified by substitution.)

This means **all three cone rungs reduce to the exact same engine as `cylinder_cylinder` Rung 3**, with three changes: the parameterized surface is the cone (ruling through apex, not parallel), the `coeffs(theta)` closure body changes per partner, and for cone-cone `q2(theta)` is a function of `theta` rather than the constant 1, so the `q2 <= 1e-18` guard becomes a per-`theta` guard (decline `theta` ranges where `q2(theta)` vanishes, which correspond to rulings parallel to the partner cone, the parabolic-analog degeneracy). The branch topology is read from `D(theta)`'s sign pattern, certified by the QI signature-sequence component count as the cross-check.

> **Verified vs claimed.** The per-ruling-quadratic reduction is **verified** (direct substitution, and it is the documented basis of both Eberly's cylinder notes and Levin's method). The exact-circle/line degenerate forms (coaxial, shared-apex, equal-angle) are **verified** from Miller-Goldman and Shene-Johnstone. The claim that the cone-cone `q2(theta)`-variable case fits the existing `fit_curve` ladder without numerical surprises is **claimed** (reconstructed from the cylinder-cylinder pattern); it must be validated against the `check_curve_on_both_tol` harness, and the `mid + half*cos(phi)` regularization near `D=0` is expected to be necessary there as it is for cylinder-cylinder bites.

### 2.4 Eberly's engineering notes (the practical degenerate handling)

> **Citation.** D. Eberly, Geometric Tools: *Intersection of Cylinders* (https://geometrictools.com/Documentation/IntersectionOfCylinders.pdf), *Intersection of a Sphere and a Cone* (https://geometrictools.com/Documentation/IntersectionSphereCone.pdf), and *Geometric Tools for Computer Graphics* (Eberly & Schneider, Morgan Kaufmann). 
> **Method / Content.** Eberly packages the ruling/separating-axis machinery into production C++ with explicit attention to the **boolean-relevant predicates** (does the sphere intersect the *finite truncated solid* cone, on which side, tangent or transverse). The cone-sphere note gives the exact conditions for a sphere to miss / touch / cross an infinite or truncated cone in terms of the apex-to-center vector projected on the axis versus the half-angle, which is precisely the cheap **reject / tangent-detect** test a boolean wants before computing a seam.
> **Limitations.** Eberly's notes are intersection *tests and queries* (does it hit, where roughly), tuned for collision/graphics, not exact CAD seam curves; the curve extraction is the ruling-substitution above, which he sets up but leaves at the polynomial level. Public-domain / Boost-licensed: **no patent risk**.
> **Kernel relevance.** Use Eberly's reject/tangent predicates as the **front gate** of each cone rung (cheap empty/tangent classification before the seam solve), and his separating-axis bounds as the broad-phase consistency check against Keel's BVH. The exact seam still comes from the Miller-Goldman degenerate forms plus the per-ruling field.

---

## 3. Representation choice: minimal `Curve3` additions vs NURBS-recovery conventions

Keel's `Curve3` is `Line / Circle / Ellipse / NURBS` (no Hyperbola, no general quartic). The question is what minimal change makes the new rungs first-class for (a) the stitch (shared seam edges must compare equal) and (b) mass integration (point + tangent at `t`).

### 3.1 The verdict: do not add a Hyperbola or quartic variant; standardize on two paths

> **Citation.** L. Piegl, W. Tiller, *The NURBS Book*, 2nd ed., Springer 1997, ch. 7 (Conics and Circles): exact rational-quadratic Bezier representation of any conic arc (circle, ellipse, **parabola, hyperbola**) via a single rational quadratic with the appropriate middle weight (`w < 1` ellipse, `w = 1` parabola, `w > 1` hyperbola). Springer chapter: https://link.springer.com/chapter/10.1007/978-3-642-59223-2_7 . Course notes mirror: https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/spline/NURBS/RB-conics.html .
> **Method / Content.** Every conic arc, including the hyperbola and parabola that Keel currently declines in `plane_cone`, has an **exact finite rational-quadratic NURBS** representation (three control points, one interior weight). So a "hyperbola" does not need a new `Curve3` enum arm: it is **already representable exactly** as `Curve3::Nurbs` with degree 2 and the right weight, `tol_achieved: 0.0`. This is exact, not a fit.
> **Limitations.** A hyperbola/parabola is unbounded; only a **bounded arc** of it (the trimmed portion that actually bounds a face) is the rational-quadratic Bezier. For SSI seams this is exactly what is wanted (the seam is always a bounded arc on a trimmed face). A full branch needs the trim interval from the face boundary, which the boolean supplies.
> **Kernel relevance.** **Recommendation: add no new exact enum arm.** Instead:
> - **Path A, exact analytic seam**: when a rung produces a true conic (circle/ellipse already native; **parabola/hyperbola as an exact degree-2 rational `Curve3::Nurbs`** with `tol_achieved: 0.0`). This upgrades `plane_cone` to stop declining the parabolic/hyperbolic slices: emit the exact rational-quadratic NURBS arc instead of `GeomError::Degenerate`. It also covers the equal-angle cone-cone hyperbola.
> - **Path B, certified recovered NURBS seam**: when a rung produces a genuine quartic branch (general cylinder-sphere, general cone-anything), recover a `Curve3::Nurbs` by the **existing cylinder-cylinder `fit_curve` ladder** (Chebyshev samples, LSQ, deviation certificate `x 2`, escalate, DECLINE on miss). `tol_achieved` carries the certificate.
>
> Both paths land in the **same `Curve3::Nurbs` arm**, which means **one stitch code path and one integrator path** for all new rungs. The stitch's "compare EQUAL" requirement is met by **identity sharing**: the SSI between face F_a and face F_b is computed once and the resulting `SsiCurve` is referenced by both half-edges, so equality is pointer/`EntityId` identity, never a fuzzy curve-equality predicate. (This matches file 47's boolean-assembly seam-sharing and file 39's imprint-once discipline.) The mass integrator gets point + tangent from the rational/recovered NURBS `point(t)` and derivative, both already implemented and used by the cylinder-cylinder rung today.

### 3.2 NURBS-recovery conventions for cache hits

The existing tier-2/tier-3 path already caches recovered NURBS seams. To make the new analytic rungs cache-compatible:

- **Canonical parameterization**: parameterize each recovered branch by a canonical angle (`theta` on the cylinder/cone ruling, or the toric `phi`) so that the same configuration recovers the **same** control points deterministically (file 14 determinism). This makes cross-operand seams bit-identical, not just within-tolerance, which is what file 39's seam-dedup wants.
- **Degree convention**: exact conics at degree 2 rational; recovered quartics at degree 3 (cubic) NURBS as the cylinder-cylinder ladder does (a smooth quartic space curve fits to a cubic NURBS within tolerance after enough knots, because the curve is smooth and the fit error divides by ~16 per doubling).
- **Certificate convention**: `tol_achieved = max implicit-residual-as-signed-distance against both surfaces` (the `check_curve_on_both_tol` metric), `x 2` safety, so the stitch can trust `tol_achieved` as a true upper bound.

---

## 4. Torus rungs

The torus row is lower priority (needed for torus parity, item on the capability map) and genuinely harder: a torus is degree 4, so torus-quadric is degree 8 and torus-torus degree 16 in general position. The strategy is **exact where a classical degeneration exists, certified-NURBS-recovery otherwise, general tracer as the floor.**

### 4.1 Torus-plane (toric sections)

> **Citation.** L. Moroni, "The toric sections: a simple introduction," arXiv:1708.00803, 2017. https://arxiv.org/pdf/1708.00803 and https://www.lucamoroni.it/toric-sections/ . Wikipedia: *Toric section*, *Spiric section*, *Villarceau circles*.
> **Method / Content.** The toric section in the cutting plane `(x, y)` has the **explicit quartic** (verified, from Moroni): `(x^2 + y^2 + rho^2 + R^2 - r^2)^2 - 4 R^2 (x^2 + (rho cos phi - y sin phi)^2) = 0`, with `R` major radius, `r` minor radius, `rho` plane offset, `phi` plane tilt. Cases:
> - **Plane perpendicular to axis** (`phi = 0`): two concentric circles (or one), **exact**, radii `R +/- sqrt(r^2 - h^2)` analog. Exact `Curve3::Circle`.
> - **Plane parallel to axis** (spiric sections): Cassini ovals (`rho = r`), Bernoulli lemniscate (`R = 2r`, focal condition), hippopedes of Proclus (tangent to inner circle, `rho = R - r`). These are **genuine quartics**, not conics: recover as NURBS.
> - **Villarceau circles**: the **bitangent plane** (touches the torus at two isolated points, tilt such that the plane is tangent to both the inner and outer equator simultaneously) cuts the torus in **two exact circles** of radius `R`. Condition: `sin phi = r / R` and the plane passes through the center. The quartic factors into two congruent conics (verified: "a quartic with more than three double points must factor... two congruent conics, the Villarceau circles"). Exact `Curve3::Circle` x 2.
> - **General oblique**: a genuine quartic (one or two ovals). Recover as NURBS.
> **Limitations.** The explicit quartic is for the section in the cutting-plane frame; mapping to Keel's torus `(u, v)` parameters for the pcurve needs the inverse torus parameterization (closed-form: `u = atan2` of the major angle, `v` of the minor). The Villarceau and perpendicular cases are exact; spiric and general are quartic.
> **Kernel relevance.** Torus-plane gets an **exact tier** (perpendicular circles, Villarceau circles, axis-containing plane gives two circles) and a **recovered-NURBS tier** (spiric and general quartics). The detector checks: plane perpendicular to axis -> circles; plane through center with `sin phi = r/R` -> Villarceau; else recover. This is the torus analog of `plane_cone`'s conic-vs-decline split, but here the decline becomes recover (because the bivariate-Bernstein implicit tracer already handles torus, see `compose_implicit_surface`'s torus arm in `ssi.rs`).

### 4.2 Torus-sphere

> **Citation.** K.-J. Kim, M.-S. Kim, "A simple algorithm for torus/sphere intersection," and the earlier "Torus/sphere intersection based on a configuration-space approach," POSTECH. https://www.researchgate.net/publication/2508780 , https://www.academia.edu/13832981 .
> **Method / Content.** Exploit the **symmetry of the relative configuration**. A sphere's implicit substituted into the torus parameterization `P(u, v) = (R + r cos v)(cos u e_x + sin u e_y) + r sin v a` gives, after the `|P - S|^2 = r_s^2` substitution, an equation that is **biquadratic** and (when the sphere center is on the torus axis or in the symmetry plane) **separates** into a low-degree solve. The algorithm classifies and detects all intersection types **including all degenerate conic sections (circles) and singular intersections** (verified). Coaxial torus-sphere (sphere center on torus axis) gives **exact circles** (the `u`-symmetry makes the section independent of `u`, so the `v`-equation is a quartic in `cos v` whose roots give circles of revolution).
> **Limitations.** The "simple algorithm" is still a configuration-space classification, not a one-line closed form for the general case; the general curve is degree 8.
> **Kernel relevance.** Exact tier: coaxial (sphere center on torus axis) -> exact circles. Recover tier: everything else via the implicit tracer (torus implicit already composed in `ssi.rs`). The configuration-space classifier supplies the **component count** for the soundness cross-check.

### 4.3 Torus-cylinder, torus-cone, torus-torus

> **Citation.** Torus-torus and torus-quadric general theory: R. T. Farouki et al. and the Dupin-cyclide literature (M. Paluszny; L. Druoton et al., "Dupin cyclide blends between quadric surfaces," *Computers & Graphics*, 2014; https://www.sciencedirect.com/science/article/abs/pii/S0097849314000417). Wikipedia: *Dupin cyclide*.
> **Method / Content.** Coaxial torus-cylinder and coaxial torus-cone give **exact circles** by the same revolution-symmetry collapse (substitute the constant/linear `rho` profile into the torus `rho`-profile, solve a quartic in `v`, each root is a circle). The general cases are degree 8 (torus-quadric) and degree 16 (torus-torus): **no closed form**, recover-or-trace only. The **Dupin cyclide** connection is the structural insight: cyclides are inversions of tori/cylinders/cones and their lines of curvature are circles, so blends and some intersections among these surfaces are circle-foliated; but for general boolean seams this is a special-configuration win, not a general method.
> **Limitations.** Torus-torus general position is the hardest analytic SSI in the kernel (degree 16); there is no practical exact form. The certified-NURBS recovery of a degree-16 curve needs many knots and the certificate may often miss `tol`, triggering DECLINE.
> **Kernel relevance.** Exact tier: coaxial torus-cylinder, coaxial torus-cone -> exact circles. Recover tier: general torus-cylinder/cone via the implicit tracer (both surfaces implicitizable). Torus-torus: **recover for near-coaxial, DECLINE the wild general case** until the general tracer (tier 3) is hardened; this is an honest decline, justified by degree 16.

---

## 5. Robustness near tangency / degeneracy: never drop a grazing component

This is the soundness core. The failure mode is: a near-tangent configuration where one intersection component is a tiny grazing loop, the marcher does not seed it, and the boolean silently **under-cuts** (removes less material than it should). DECLINE-never-WRONG forbids this: better to decline the whole boolean than to emit a body missing a hole.

> **Citation.** C. L. Bajaj, C. M. Hoffmann, J. E. Hopcroft, R. E. Lynch, "Tracing surface intersections," *CAGD* 5, 1988. N. M. Patrikalakis, T. Maekawa, *Shape Interrogation for Computer Aided Design and Manufacturing*, Springer 2002 (hyperbook: https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/). Signature-sequence / discriminant classifiers (section 2.2).
> **Method / Content.** The production discipline is **classify-then-verify**:
> 1. **Up-front dimensionality + component classification** (Patrikalakis-Maekawa): decide Empty / Points / Curves / Coincident, and within Curves the expected **number and type of components**, *before* tracing. `ssi.rs` already does the Empty/Points/Curves/Coincident split; this extends it with a component count.
> 2. **Algebraic component count as a certificate** (signature sequence / discriminant set for quadrics; configuration-space classifier for torus): the count is computed from the input coefficients, independent of the tracer.
> 3. **Bajaj et al. significant-point seeding**: seed the tracer at **all critical points** (turning points, singular points, border crossings), not just border crossings, so an interior closed loop (a grazing component with no border crossing) is still seeded. `ssi.rs`'s `collect_critical_points` (via `solve_system` over the Bernstein system `f = f_v = 0` and `f = f_u = 0`) already does this for the implicit tracer; the analytic rungs get the analog from the discriminant `D(theta)`'s zeros and the geometric degenerate detector.
> 4. **Count reconciliation**: after tracing/fitting, the number of recovered components must **equal** the certified count. If fewer (a component was missed) or the fit certificate misses `tol`, **DECLINE**.
> **Limitations.** The certified count is only as good as the sign evaluation of the discriminants; in pure floating point a near-tangent can be misclassified. The mitigation is an **interval/conservative** evaluation of the discriminant near zero: if the discriminant interval straddles zero (tangency cannot be ruled out), treat the configuration as tangent and either emit the tangent component flagged `tangential: true` or DECLINE.
> **Kernel relevance.** This makes the cone/torus rungs **as sound as cylinder-cylinder is today**. Cylinder-cylinder already certifies its branch count from `D(theta)`'s sign pattern (full-wrap = 2 branches, each bite = 1 closed curve) and declines on certificate miss. The general rule for the new rungs: **(a)** compute the expected component count from the algebraic classifier (quadric pairs) or the geometric detector (degenerate cases) or the discriminant `D(theta)` (per-ruling rungs); **(b)** seed at all critical points including interior loop seeds; **(c)** reconcile counts; **(d)** DECLINE on any mismatch or certificate miss. A near-tangent grazing component is never dropped because the classifier predicted it and the count reconciliation catches its absence.

---

## 6. Validation oracle

Per rung, the test matrix mirrors the existing `ssi.rs` test module's `check_curve_on_both_tol` (implicit-residual-as-signed-distance, exact curves at `1e-9`, recovered curves at their `tol_achieved`). For each rung, the cases below have a **known curve type** (the analytic oracle) plus a **Monte-Carlo boolean-volume cross-check** (the project's mesh-vs-mass oracle, file 35): build two solids, boolean them, and compare the analytic mass to a dense surface-sampled mesh volume; PASS = `mass == mesh == ref`, DECLINE counted separately, WRONG = zero.

| Rung | Coaxial / axis-coincident | General | Tangent | Apex/special |
|---|---|---|---|---|
| cylinder-sphere | 2 exact circles (center on axis) | quartic, 1 or 2 NURBS loops | 1 NURBS loop w/ double point, `tangential` | Viviani (`r_s=2r_c`, center on surface): figure-eight |
| cone-sphere | exact circle(s) (coaxial) | quartic NURBS | tangent sphere: point or pinched loop | sphere through apex: apex point + loop |
| cone-cylinder | exact circle(s) (coaxial) | quartic NURBS | tangent: pinched | cylinder axis through apex |
| cone-cone | lines through shared apex; exact circle (coaxial equal) | quartic NURBS; **equal-angle parallel-axis = exact conic (hyperbola as rational-quad NURBS)** | tangent: line/pinch | shared apex: 0/1/2 lines |
| torus-plane | perpendicular: circles; through-center: 2 circles | spiric quartic NURBS | bitangent: **Villarceau 2 circles** | lemniscate (`R=2r`), Cassini (`rho=r`) |
| torus-sphere | exact circles (coaxial) | degree-8 NURBS | tangent: pinch | center in symmetry plane |
| torus-cylinder/cone | exact circles (coaxial) | recover or DECLINE | tangent | --- |
| torus-torus | exact circles (coaxial) | DECLINE (degree 16) | --- | --- |

**Exact oracles available now (no Monte-Carlo needed, residual `< 1e-9`):** every coaxial circle case, the Villarceau circles, the perpendicular toric circles, the shared-apex cone lines, the equal-angle cone-cone conic. These are the **first-milestone gate**: they have closed-form answers and the `check_curve_on_both` harness verifies them exactly. The general quartic cases are validated by `check_curve_on_both_tol` at `tol_achieved` plus the boolean-volume Monte-Carlo.

> **Citation.** The oracle methodology is file 35 (verification / validated numerics / oracles) and the existing `ssi.rs` `check_curve_on_both_tol`; the boolean three-bucket (PASS/DECLINE/WRONG) is the project's fuzz-soak completion gate (MEMORY: fuzz-soak-completion-gate).
> **Kernel relevance.** The exact-oracle subset is what makes the **first milestone honest**: it is the narrowest slice (coaxial cylinder-sphere and coaxial cone-sphere) where the answer is two exact circles and the implicit residual is `1e-9`, identical in form to the already-passing plane-sphere and sphere-sphere tests.

---

## Cross-cutting three-bucket catalog

Answering the project's recurring question: which configurations admit (a) exact analytic curves, (b) certified NURBS-recovery, (c) a genuine general tracer.

**(a) EXACT analytic curve (circle / conic / rational, residual `1e-9`, `tol_achieved = 0`):**
all **coaxial** quadric-quadric and torus-quadric cases (cylinder-sphere, cone-sphere, cone-cylinder, coaxial torus-cylinder/sphere/cone) -> circles; **shared-apex** cone-cone -> lines; **equal-half-angle parallel-axis** cone-cone -> a conic (ellipse/hyperbola-as-rational-quadratic-NURBS/line-pair); **perpendicular** torus-plane -> circles; **Villarceau bitangent** torus-plane -> two circles; and (upgrading the current decline) **all `plane_cone` parabolic/hyperbolic slices** -> exact rational-quadratic NURBS arcs.

**(b) Certified NURBS-recovery (sample-and-fit, certificate `x 2`, DECLINE on miss, cached by the existing recovered-NURBS seam path):**
all **general-position quadric-quadric** quartics (off-axis cylinder-sphere, general cone-sphere/cone-cylinder/cone-cone) via the per-ruling closed-form branch field + `fit_curve` ladder (the cylinder-cylinder engine, generalized); **spiric and general-oblique torus-plane** quartics; **general torus-sphere / torus-cylinder / torus-cone** degree-8 curves via the bivariate-Bernstein implicit tracer already in `ssi.rs`; **near-coaxial torus-torus**.

**(c) Genuine general numeric tracer (marching / interval, the tier-3 floor):**
**wild general torus-torus** (degree 16) and any quadric/torus pair whose certified count cannot be established or whose fit certificate misses `tol`. These **DECLINE** under the current contract; the tier-3 Krawczyk-verified marcher in `ssi.rs` (`march_ssi`) is the staged upgrade that converts some of these declines to certified recoveries later, gated by the component-count reconciliation of section 5.

---

## Synthesis: ADOPT / AVOID / INVESTIGATE

**ADOPT.**
1. **The per-ruling-quadratic engine as the universal quadric-quadric template.** Generalize the existing `cylinder_cylinder` Rung 3 into a shared `ruling_quadric_ssi` helper: parameterize one quadric by its ruling (cylinder by parallel rulings, cone by apex rulings), substitute the partner's implicit to get the per-`theta` quadratic `q2(theta) v^2 + q1(theta) v + q0(theta)`, classify branches from `D(theta) = q1^2 - 4 q2 q0`, fit each branch with the certified `fit_curve` ladder. This covers cylinder-sphere, cone-sphere, cone-cylinder, cone-cone with one engine. (Verified structure; Levin/Eberly basis.)
2. **The Miller-Goldman / Shene-Johnstone degenerate detectors as the exact front gate** of every cone rung: coaxial -> circles, shared-apex -> lines, equal-angle -> conic, all with degree-<=2 solves.
3. **No new `Curve3` enum arm.** Represent parabola/hyperbola seams as exact degree-2 **rational NURBS** (Piegl-Tiller), recovered quartics as degree-3 NURBS. One stitch path, one integrator path. Upgrade `plane_cone` to emit the exact rational-quadratic arc instead of declining parabolic/hyperbolic slices.
4. **Identity-shared seam edges** (compute SSI once, both half-edges reference it) so the stitch "compare EQUAL" is `EntityId` identity, not a fuzzy predicate.
5. **The QI signature-sequence / discriminant classifier as the branch-count certificate**, reconciled against the recovered component count, with DECLINE on mismatch (section 5). This is the soundness guarantee against dropped grazing components.

**AVOID.**
- **Adopting QI/Levin as the shipping engine.** It imports file 11's algebraic-number wall, is GPL-class C++, and outputs `sqrt(quartic)` parameterizations, not `Curve3`. Use it only as the completeness oracle and morphology dictionary (offline, for test-oracle generation and the classifier).
- **A general quartic `Curve3` arm.** Unnecessary given exact rational-conic NURBS + certified cubic recovery; it would multiply the stitch and integrator paths.
- **Shipping general torus-torus** (degree 16) before the tier-3 marcher is count-certified. DECLINE it honestly.

**INVESTIGATE.**
- Whether the cone-cone `q2(theta)`-variable case needs the `mid + half*cos(phi)` regularization at *every* `D=0` end or only at the `q2=0` parabolic-analog ends (validate against `check_curve_on_both_tol`).
- Whether the Dupin-cyclide circle-foliation gives an exact tier for a useful slice of torus-cylinder/cone blends-as-intersections (file 40 cross-reference), beyond the coaxial case.
- Interval-arithmetic evaluation of the quadric discriminant set so the tangency classification is *certified* (file 11's filter substrate), not just a floating-point predictor.

**Recommended FIRST MILESTONE (narrowest honest slice, exact oracle):**
**Coaxial cylinder-sphere and coaxial cone-sphere -> exact circles.** These are the smallest additions, structurally identical to the already-passing plane-sphere / sphere-sphere rungs, with a closed-form answer (0/1/2 circles from a single quadratic in the axial coordinate) and an exact `1e-9` implicit-residual oracle via the existing `check_curve_on_both` harness. They unblock the most common real boolean (a drilled/bossed cylindrical or conical part meeting a spherical cap) and prove the dispatch wiring before the general quartic work.

**Follow-up ladder:**
1. Coaxial cylinder-sphere, coaxial cone-sphere (exact circles) -- the first milestone.
2. Coaxial cone-cylinder (exact circles); shared-apex and equal-angle cone-cone (exact lines / conic, the latter via rational-quadratic NURBS); upgrade `plane_cone` to emit parabola/hyperbola arcs exactly.
3. General cylinder-sphere via the shared `ruling_quadric_ssi` engine (the first certified-NURBS quartic rung, validated against cylinder-cylinder's own pattern).
4. General cone-sphere, cone-cylinder, cone-cone via the same engine, each with the QI count certificate.
5. Torus-plane (perpendicular + Villarceau exact; spiric/general recovered).
6. Torus-sphere (coaxial exact; general recovered).
7. Torus-cylinder, torus-cone (coaxial exact; general recovered).
8. Torus-torus (coaxial exact; near-coaxial recovered; wild general DECLINE pending tier-3 count-certified marcher).

**Configuration to DECLINE forever (justified):**
**Wild general-position torus-torus** (degree-16 space curve, both surfaces far from coaxial, no symmetry collapse). Justification: there is no practical exact form, the certified-NURBS recovery of a degree-16 curve will routinely miss `tol`, and emitting an under-resolved seam would risk a WRONG body. Declining it is honest and rare in real CAD (two general tori rarely boolean in production parts). It moves from permanent-decline to staged-recovery only if the tier-3 marcher gains a torus-torus component-count certificate from the configuration-space classifier; until then, DECLINE is the correct floor.

---

## References

1. J. R. Miller, "Geometric approaches to nonplanar quadric surface intersection curves," *ACM Transactions on Graphics* 6(4):274-307, 1987. https://dl.acm.org/doi/abs/10.1145/35039.35041
2. J. R. Miller and R. N. Goldman, "Geometric algorithms for detecting and calculating all conic sections in the intersection of any two natural quadric surfaces," *Graphical Models and Image Processing* 57(1), 1995. https://www.researchgate.net/publication/222480796
3. C.-K. Shene and J. K. Johnstone, "On the lower degree intersections of two natural quadrics," *ACM Transactions on Graphics* 13(4), 1994. https://dl.acm.org/doi/10.1145/195826.197316
4. J. Z. Levin, "Mathematical models for determining the intersections of quadric surfaces," *Computer Graphics and Image Processing* 11(1):73-87, 1979; and *Comm. ACM* 19(10), 1976.
5. Y.-K. Choi, W. Wang, et al., "Enhancing Levin's method for computing quadric-surface intersections," *Computer Aided Geometric Design* 20(7):401-422, 2003. https://i.cs.hku.hk/~ykchoi/quadrics/CAGD_Levin_Enhanced.pdf
6. W. Wang, B. Joe, R. Goldman, "Computing quadric surface intersections based on an analysis of plane cubic curves," *Graphical Models* 64(6):335-367, 2002. https://www.sciencedirect.com/science/article/abs/pii/S1077316902000187
7. C. Tu, W. Wang, B. Mourrain, et al., "Using signature sequences to classify intersection curves of two quadrics," *Computer Aided Geometric Design*, 2009. https://i.cs.hku.hk/~ykchoi/quadrics/cagd_qsic.pdf
8. L. Dupont, D. Lazard, S. Lazard, S. Petitjean, "Near-optimal parameterization of the intersection of quadrics (I, II, III)," *Journal of Symbolic Computation*, 2008. https://inria.hal.science/inria-00186089v1/document
9. S. Lazard, L. M. Penaranda, S. Petitjean, "Intersecting quadrics: an efficient and exact implementation (QI)," 2006. https://inria.hal.science/inria-00104003/document ; software: https://gamble.loria.fr/qi/
10. "Topological classification of the intersection curves of two quadrics using a set of discriminants," *Computer Aided Geometric Design*, 2023. https://www.sciencedirect.com/science/article/abs/pii/S0167839623000766
11. D. Eberly, *Intersection of Cylinders*, Geometric Tools. https://geometrictools.com/Documentation/IntersectionOfCylinders.pdf
12. D. Eberly, *Intersection of a Sphere and a Cone*, Geometric Tools. https://geometrictools.com/Documentation/IntersectionSphereCone.pdf
13. P. Schneider and D. Eberly, *Geometric Tools for Computer Graphics*, Morgan Kaufmann, 2002. http://lib.ysu.am/open_books/312121.pdf
14. *Sphere-cylinder intersection*, Wikipedia. https://en.wikipedia.org/wiki/Sphere%E2%80%93cylinder_intersection
15. *Cylinder-Sphere Intersection*, Wolfram MathWorld. https://mathworld.wolfram.com/Cylinder-SphereIntersection.html
16. L. Moroni, "The toric sections: a simple introduction," arXiv:1708.00803, 2017. https://arxiv.org/pdf/1708.00803 ; https://www.lucamoroni.it/toric-sections/
17. *Toric section*, Wikipedia. https://en.wikipedia.org/wiki/Toric_section
18. *Spiric section*, Wikipedia. https://en.wikipedia.org/wiki/Spiric_section
19. *Villarceau circles*, Wikipedia. https://en.wikipedia.org/wiki/Villarceau_circles
20. K.-J. Kim and M.-S. Kim, "A simple algorithm for torus/sphere intersection," POSTECH. https://www.researchgate.net/publication/2508780 ; and "Torus/sphere intersection based on a configuration-space approach," *Graphical Models and Image Processing*, 1998. https://www.academia.edu/13832981
21. L. Druoton et al., "Dupin cyclide blends between quadric surfaces for shape modeling," *Computers & Graphics*, 2014. https://www.sciencedirect.com/science/article/abs/pii/S0097849314000417 ; *Dupin cyclide*, Wikipedia. https://en.wikipedia.org/wiki/Dupin_cyclide
22. L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., Springer, 1997, ch. 7 (Conics and Circles). https://link.springer.com/chapter/10.1007/978-3-642-59223-2_7 ; notes: https://pages.mtu.edu/~shene/COURSES/cs3621/NOTES/spline/NURBS/RB-conics.html
23. C. L. Bajaj, C. M. Hoffmann, J. E. Hopcroft, R. E. Lynch, "Tracing surface intersections," *Computer Aided Geometric Design* 5(4):285-307, 1988.
24. N. M. Patrikalakis and T. Maekawa, *Shape Interrogation for Computer Aided Design and Manufacturing*, Springer, 2002. https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/
25. R. T. Farouki, "The characterization of parametric surface sections," *Computer Vision, Graphics, and Image Processing*, 1986 (toric and quadric section structure).

---

## Related Keel dossiers (build on, not repeated)

- **11. Exact and filtered geometric predicates for curved geometry** -- the algebraic-number wall; why exact quadric/torus seam coordinates are degree-8/16 algebraic numbers and must be routed through filters or declined. This dossier's certified-NURBS recovery is the engineering escape from that wall for bulk seam geometry, reserving exact comparison for topology.
- **24. Canonical geometry recovery** -- the sample-and-fit-with-certificate discipline; the SSI recovered-NURBS seam uses the same certificate convention.
- **39. Coincident, coplanar, and tangent face booleans** -- the tangency handling and imprint-once / seam-dedup discipline this dossier's section 5 plugs into; tangent SSI components flow into the file-39 tangent-face tables.
- **40. Blend / fillet surface generation** -- the exact-vs-NURBS decision ladder and the Dupin-cyclide canal-surface connection cross-referenced in section 4.3.
- **47. Boolean result assembly** -- the seam-sharing / identity-edge mechanism this dossier relies on for the stitch "compare EQUAL" requirement.
- **57. Booleans on general / non-manifold bodies** -- the downstream consumer; the new seams feed the dimension-pair selection table there.
- **35. Verification, validated numerics, oracles** -- the three-bucket (PASS / DECLINE / WRONG) boolean-volume oracle used for section 6 validation.
