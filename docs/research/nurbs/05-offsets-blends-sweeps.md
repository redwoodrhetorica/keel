# Offsets, Blends/Fillets, Sweeps, and Constructed Surfaces in NURBS/Spline Geometry

Research dossier 05 for the Keel B-rep kernel. This file investigates the families of operations that dominate production solid modeling: offsetting (shelling, thickening, draft), blending and filleting (the single largest consumer of geometry-engine effort in a real kernel), sweeping along trajectories, and the lofting/skinning/ruling constructions that build surfaces from curves. These operations are unified by a single uncomfortable fact: most of them produce surfaces that are **not exactly representable as NURBS**. The kernel must therefore choose, per operation, between an exact non-NURBS representation (a procedural or algebraic surface) and a tolerance-controlled NURBS approximation. The literature below is the basis for making that choice deliberately rather than by accident.

Scope of this document:

- Offset curves and surfaces: the non-rationality problem, the approximation-method survey literature, tolerance-controlled approximation, self-intersection trimming, and offset topology change.
- Pythagorean-hodograph (PH) curves: exact rational offsets, PH quintic Hermite interpolation, spatial PH via quaternions, and toolpath applications.
- Pipe/canal surfaces and their rational parameterizations.
- Blending and filleting: rolling-ball theory, variable-radius blends, vertex blends and setback corner patches, the blending-method taxonomy.
- Sweeps: trajectory sweeping, rotation-minimizing frames (RMF) and the double-reflection method, Frenet vs RMF, swept-volume boundaries.
- Skinning/lofting: cross-section compatibility, the knot-merging blowup, approximate skinning, developable design.
- Ruled surfaces, surfaces of revolution, extrusions as exact NURBS.
- Shelling/thickening of solids.
- Medial-axis connections to offset self-intersection.
- Toolpath generation (constant scallop, pencil milling) where offsets meet manufacturing.

A closing synthesis proposes how Keel should represent and approximate each construction.

---

## 1. Offset Curves and Surfaces: the Fundamental Problem

### 1.1 Why offsets are not rational

Given a regular parametric curve **c**(t) with unit normal **n**(t), the planar offset at signed distance d is

    c_d(t) = c(t) + d * n(t),    n(t) = ( c'(t) rotated 90 deg ) / |c'(t)|.

The normal involves dividing by the speed |c'(t)| = sqrt( x'(t)^2 + y'(t)^2 ). That square root is in general **not a polynomial or rational function of t**, so c_d is not a polynomial/rational (NURBS) curve even when c is. The analogous surface statement: the offset of surface **S**(u,v) at distance d is

    S_d(u,v) = S(u,v) + d * N(u,v),

with unit normal N = (S_u x S_v) / |S_u x S_v|. The magnitude |S_u x S_v| is again a square root and S_d is not generally a NURBS surface.

Three structural consequences follow, all of which drive the rest of this document:

1. **Representation.** Any kernel that stores offsets as NURBS is storing an approximation. The only exact rational offsets come from special curves (Pythagorean-hodograph; Section 2) and special surfaces (canal/pipe surfaces and rational-offset surfaces; Section 3).
2. **Self-intersection / topology change.** Where the offset distance d exceeds the local radius of curvature, the offset folds back on itself and develops cusps and loops. The valid offset is the trimmed envelope, and the trimming locus is intimately tied to the **medial axis** (Section 9).
3. **Degree growth.** Even where the exact offset is rational (PH curves), the rational offset of a degree-n PH curve has higher degree than the source, and NURBS approximations of generic offsets need many spans to meet tolerance.

### 1.2 The exact-rational-offset escape hatches

The literature identifies the few cases where offsets ARE rational and a kernel can store them exactly:

- **PH curves** (Farouki): |c'(t)| is a polynomial by construction, so c_d is rational. See Section 2.
- **Pipe surfaces** (constant-radius offset of a curve / canal surface with constant radius) and more generally **canal surfaces** with rational spine and rational radius (Peternell & Pottmann). See Section 3.
- **Rational-offset (PN) surfaces**: surfaces with a Pythagorean normal vector field, for which N is rational. Quadrics, the cyclides, and certain blending surfaces fall here.

Everything else is approximation territory.

---

## 2. Offset Approximation: the Method-Survey Literature

### 2.1 Elber, Lee & Kim (1997): the comparison paper

**Citation.** Elber, G., Lee, I.-K., & Kim, M.-S. (1997). Comparing offset curve approximation methods. *IEEE Computer Graphics and Applications, 17*(3), 62-71. https://doi.org/10.1109/38.586019

**Method.** This is the canonical "which offset method should I use" paper. It collects the main planar offset-approximation strategies and compares them on identical inputs (Bezier/B-spline curves of varying degree) at fixed tolerance, reporting the number of control points each needs to meet that tolerance. The methods compared:

- **Tiller & Hanson (1984): offset the control polygon.** Offset each leg of the control polygon by d along its normal and intersect adjacent offset legs to get new control points; keep the original knot vector. Cheap, no degree change. Excellent for piecewise-linear and quadratic curves, poor for high degree because polygon offset diverges from true offset where the polygon is far from the curve.
- **Coquillart (1987): offset with error correction.** Sample the true offset, offset control points, then add correction vectors at points of maximal deviation to pull the approximation onto the offset. Improves accuracy at cost of more points.
- **Klass (1983): cubic Hermite / curvature-based.** Approximate the offset with a cubic using offset position and tangent (and curvature) data at the segment ends.
- **Hoschek (1988): least-squares with optimization.** Fit the offset by least squares, optionally optimizing knot placement and endpoints; reparameterization to reduce error.
- **Elber & Cohen (control-point / "offset by interpolation").** Interpolate or approximate the true offset sampled densely, with adaptive refinement until the deviation bound is met.
- **Offset-circle approximation (the paper's featured idea).** Instead of approximating the offset curve directly, approximate the **offset operator** by approximating the unit circle / the rotation that produces the normal. One convolves the curve with an approximation of the offset circle. This decouples accuracy from the curve's own complexity.

**Accuracy and robustness.** The headline empirical finding: for **quadratic** (and piecewise-quadratic) curves the simple **Tiller-Hanson** method is the best tradeoff and beats the more elaborate methods, despite being the cheapest. Its advantage erodes as degree rises; for high-degree curves the sampling/least-squares and offset-circle approaches win on control-point economy at a given tolerance. All methods are driven by a measured Hausdorff-type deviation between the approximation and the true offset (the true offset evaluated pointwise via c(t)+d*n(t)).

**Limitations.** The comparison is planar and does not resolve self-intersection (that is treated as a separate post-process). No single method dominates across all degrees, which is itself the practical lesson: a kernel needs a small portfolio of offset methods selected by curve degree and shape.

**Kernel relevance.** Direct. Keel's 2D offsetter (used in sketch offset, draft, and as the inner loop of shelling) should implement Tiller-Hanson as the default for the low-degree spans that dominate sketch geometry, with an adaptive sampled-and-refit fallback for high-degree or high-curvature spans, all governed by a single chord/deviation tolerance. The "approximate the operator, not the curve" idea from the offset-circle method is a good design principle: it keeps the approximation error independent of input complexity.

### 2.2 Maekawa (1999): overview of offset curves and surfaces

**Citation.** Maekawa, T. (1999). An overview of offset curves and surfaces. *Computer-Aided Design, 31*(3), 165-173. https://doi.org/10.1016/S0010-4485(99)00013-5

**Method.** A literature survey covering work after the earlier 1992 reviews, organized into five active areas: (1) representing exact offsets in Bezier/B-spline form (i.e., the PH and rational-offset special cases), (2) approximation methods, (3) self-intersection detection and trimming, (4) geodesic offsets (offsetting along a surface, used in toolpaths and seam allowances), and (5) general offsets (variable distance, oriented offsets). It catalogs the differential-geometry machinery: the offset's curvature relates to the base curve's via kappa_d = kappa / (1 - d*kappa) in 2D, which makes explicit that the offset is singular when d = 1/kappa (the center of curvature), the seed of all self-intersection.

**Accuracy and robustness.** As a survey it reports others' bounds. The key reusable fact is the curvature relation above and its surface analogue with principal curvatures kappa_1, kappa_2: an offset surface is singular where d = 1/kappa_1 or d = 1/kappa_2 (the focal surfaces / evolute), i.e., the offset degenerates exactly on the source surface's focal set.

**Limitations.** No new algorithm; a map of the territory as of 1999. Post-1999 advances (distance-map trimming, PH splines, modern shelling) are not covered.

**Kernel relevance. ** This is the right "table of contents" for Keel's offset module. The curvature/focal-set relations give a cheap a-priori test: before offsetting at distance d, evaluate max principal curvature; if d >= 1/kappa_max anywhere, self-intersection/topology change is guaranteed and the trimming path must run.

### 2.3 B-spline offset surface approximation comparisons

**Citation.** Kumar, G. V. V. R., Shastry, K. G., & Prakash, B. G. (2002 and related). Qualitative and quantitative comparisons of B-spline offset surface approximation methods. *Computer-Aided Design, 34*(forthcoming pages). (ScienceDirect S0010448500001470.)

**Method.** The surface analogue of Elber-Lee-Kim. Compares control-net offsetting, sampling-plus-fit, and normal-perturbation methods for tensor-product B-spline surfaces, measuring deviation from the true offset surface across the parameter domain.

**Accuracy and robustness.** Confirms that the simplest control-net offset is acceptable only for gently curved surfaces; high-curvature regions need adaptive refinement (knot insertion where deviation exceeds tolerance) to control error, with the tradeoff being control-net growth.

**Limitations.** Tensor-product only; trimmed surfaces (the actual faces in a B-rep) need their trim curves re-offset and re-projected, which the comparison does not fully address.

**Kernel relevance.** Surface offset is the core of shelling/thickening (Section 8). The practical recipe: offset the underlying (untrimmed) surface with adaptive knot insertion to tolerance, then re-trim.

---

## 3. Pythagorean-Hodograph Curves and Rational Offsets

### 3.1 Farouki: the PH curve framework

**Citation.** Farouki, R. T. (2008). *Pythagorean-Hodograph Curves: Algebra and Geometry Inseparable*. Geometry and Computing, Vol. 1. Springer. (Foundational monograph.) See also Farouki, R. T., & Sakkalis, T. (1990). Pythagorean hodographs. *IBM Journal of Research and Development, 34*(5), 736-752.

**Method.** A planar polynomial curve **r**(t) = (x(t), y(t)) has a *Pythagorean hodograph* if its derivative components satisfy

    x'(t)^2 + y'(t)^2 = sigma(t)^2

for some polynomial sigma(t); i.e., the parametric speed is a polynomial, not a square root. The general planar solution is generated from two polynomials u(t), v(t):

    x'(t) = u(t)^2 - v(t)^2,    y'(t) = 2 u(t) v(t),    sigma(t) = u(t)^2 + v(t)^2.

Integrating gives the PH curve. Because |r'(t)| = sigma(t) is polynomial:

- The **arc length** s(t) = integral of sigma is a polynomial (exact, closed-form length).
- The **unit tangent** and **unit normal** are rational, so the **offset** r_d(t) = r(t) + d * n(t) is an **exact rational** (NURBS-representable) curve.
- The curvature kappa(t) = (x' y'' - y' x'') / sigma^3 is rational.

The lowest useful degree is the **PH quintic**: PH cubics are too rigid (no inflections), but PH quintics have inflections and enough freedom (eight effective shape DOF) for general G1/C1 Hermite interpolation.

**Accuracy and robustness.** PH quintic C1 Hermite interpolation has approximation order 4 for the curve and order 3 for its offset (Farouki & Neff). The PH structure makes length and offset *exact* rather than approximate, which is the whole point: no chord-length tables, no offset refit.

**Limitations.** PH curves are a *constrained* family. Converting an arbitrary design curve to PH form is itself an approximation problem (PH Hermite or PH spline fitting). The Hermite interpolant is not unique (a multi-valued solution family; choosing the "good" one needs a fairness/winding criterion). Degree rises: rational offset of a PH quintic is higher degree.

**Kernel relevance.** Strong but selective. Keel will not store every curve as PH. But for **toolpath curves, exact-length features, and constant-radius pipe spines**, representing the spine as a PH spline buys exact arc length (feedrate control) and exact rational offsets (cutter compensation) for free. A PH-spline fitter is a high-value optional module.

### 3.2 Spatial PH via quaternions

**Citation.** Farouki, R. T., Giannelli, C., Manni, C., & Sestini, A.; and Choi, H. I., Lee, D. S., & Han, H. P. (2002). Euler-Rodrigues frames on spatial Pythagorean-hodograph curves. *Computer Aided Geometric Design*; plus Farouki (2008) ch. on spatial PH. Survey: Farouki, R. T. (2018/2008). Pythagorean-hodograph curves: a survey of recent advances. (ResearchGate 267024627.)

**Method.** A spatial PH curve has r'(t) with |r'(t)|^2 a perfect square. The clean generator uses a **quaternion polynomial** A(t): r'(t) = A(t) i A*(t) (quaternion product, A* conjugate), which automatically yields a Pythagorean hodograph in 3D. Spatial PH quintic Hermite interpolation reduces to three quadratic quaternion equations with a closed-form, two-(or more)-parameter solution family. Spatial PH curves carry a natural rational **Euler-Rodrigues frame** and admit closed-form **rotation-minimizing frames** for low degrees (see Section 5 for the connection to sweeps).

**Accuracy and robustness.** Exact polynomial arc length and rational adapted frames in 3D; closed-form interpolation. The quaternion form makes coordinate transforms and frame computation algebraically simple.

**Limitations.** Solution multiplicity (must pick the least-twisted/most-fair member); higher degree than ordinary cubics for equivalent flexibility.

**Kernel relevance.** Spatial PH spines are the ideal trajectory for sweeps and pipes when exact length and a rational RMF-like frame are wanted (e.g., wire/tube routing, additive-manufacturing deposition paths). The quaternion machinery overlaps with the RMF and frame work in Section 5.

### 3.3 PH and biarc offset approximation of general curves

**Citation.** Sir, Z., Feichtinger, R., & Juttler, B. (2006). Approximating curves and their offsets using biarcs and Pythagorean hodograph quintics. *Computer-Aided Design, 38*(6), 608-618. https://doi.org/10.1016/j.cad.2006.02.003

**Method.** To get exact rational offsets for an arbitrary curve, first **approximate the curve itself** by a PH quintic spline (or by biarcs, which also have exact rational offsets, since circular arcs do). Then the offset of the approximation is exact and rational. Compares biarc fitting (G1, simple, ubiquitous in CNC) against PH quintic fitting (G1/G2, higher order, fewer segments).

**Accuracy and robustness.** PH quintic fits reach a tolerance with far fewer segments than biarcs at the same continuity, and give G2 where biarcs give only G1. Both deliver offsets with no refit error.

**Limitations.** Two-stage error: the curve approximation error propagates to the offset. Still cheaper overall than offsetting then refitting a generic spline.

**Kernel relevance.** This is the practical bridge from "arbitrary sketch curve" to "exact-offset representation." For features where offset fidelity matters (thin-wall shelling, cutter comp), fit the spine to a PH/biarc form first.

---

## 4. Pipe and Canal Surfaces: Rational Representations

### 4.1 Peternell & Pottmann: rational canal surfaces

**Citation.** Peternell, M., & Pottmann, H. (1997). Computing rational parametrizations of canal surfaces. *Journal of Symbolic Computation, 23*(2-3), 255-266. https://doi.org/10.1006/jsco.1996.0087

**Method.** A **canal surface** is the envelope of a one-parameter family of spheres with center (spine) m(t) and radius r(t). A **pipe surface** is the special constant-radius case. The paper shows: **if m(t) is rational and r(t) is rational, the canal surface has a rational parametrization**, and gives an algorithm to compute one of low degree. The construction uses the **cyclographic model of Laguerre geometry**: a sphere (center m, radius r) maps to the point (m, r) in R^4 (Minkowski space), and the canal surface's envelope condition becomes a condition on the spine's image curve. The **characteristic circle** at parameter t (where the envelope touches the sphere) is computed from m(t), r(t), and their derivatives; the surface is the union of these circles. Each characteristic circle is rationally parameterized (a circle is rational), and sweeping the circle's rational parameterization along t (also rational) gives a rational surface. A Pythagorean-type condition on the spine appears in getting the lowest degree and avoiding spurious square roots.

**Accuracy and robustness.** The parametrization is **exact** (not an approximation) whenever spine and radius are rational. The paper emphasizes minimizing the degree of the result; later work (Landsmann, Peternell, Pottmann; and "Minimal rational parametrizations of canal surfaces," Computing 2006) pushes degree minimality further.

**Limitations.** Requires rational spine and radius. The minimal-degree parametrization can still be high degree; conversion to NURBS may need care at the characteristic circles' poles. Self-intersection of the pipe (when r exceeds spine curvature radius) still must be trimmed, just like curve offsets.

**Kernel relevance.** Pipe/canal surfaces are the **3D analogue of constant/variable-radius offsets** and underlie rolling-ball fillets (Section 6): a constant-radius rolling-ball blend surface IS (a trimmed region of) a pipe/canal surface whose spine is the blend's spine curve. Storing fillets as exact canal surfaces, with NURBS only as a tessellation/IO layer, is a serious architectural option for Keel. At minimum, constant-radius pipe surfaces should be an exact procedural surface type.

### 4.2 Rational offset surfaces (PN surfaces)

**Citation.** Pottmann, H. (1995). Rational curves and surfaces with rational offsets. *Computer Aided Geometric Design, 12*(2), 175-192. Related: Krasauskas, R., & Peternell, M. Rational offset surfaces and their modeling applications. (TU Wien preprint.)

**Method.** A surface has rational offsets iff it has a **rational unit normal field** (Pythagorean-normal, "PN"). Pottmann characterizes such surfaces via the **dual / line-geometry** representation: represent the surface by its tangent planes (the dual hypersurface in the space of planes); PN surfaces correspond to rational curves/surfaces in a quadric model. Quadrics, tori, cyclides, and surfaces built from PH curves are PN. The construction lets one *design directly in PN form* so that all offsets are rational by construction.

**Accuracy and robustness.** Exact rational offsets at every distance d, simultaneously (a whole rational family in d).

**Limitations.** PN surfaces are a constrained design space; not every freeform surface is PN, and approximating a freeform by a PN surface is its own fitting problem.

**Kernel relevance.** If Keel ever wants exact thick-shell offsets for a class of faces (e.g., analytic faces, fillets), keeping those faces in PN/canal form gives exact offsets and avoids the approximate-offset refit entirely. A pragmatic policy: analytic and fillet faces -> exact offset; freeform faces -> approximate offset.

---

## 5. Sweeps and Rotation-Minimizing Frames

### 5.1 Wang, Juttler, Zheng & Liu: double-reflection RMF

**Citation.** Wang, W., Juttler, B., Zheng, D., & Liu, Y. (2008). Computation of rotation minimizing frames. *ACM Transactions on Graphics, 27*(1), Article 2, 1-18. https://doi.org/10.1145/1330511.1330513

**Method.** An adapted orthonormal frame {t, r, s} along a curve x(u) (t = unit tangent) is a **rotation-minimizing frame (RMF)** if it has zero angular velocity about t, i.e., the reference direction r satisfies

    r'(u) = - ( r(u) . t'(u) ) t(u),

so r only turns as much as forced by the tangent's change and never twists about t. RMFs are the "minimally twisted" frames and are what you want for sweep surfaces, generalized cylinders, tubes, camera paths, and 5-axis tool orientation, because they avoid the spurious roll that Frenet frames inject.

The **double-reflection method** computes the next frame from the current one using two reflections (Householder reflections), which exactly preserve orthonormality:

1. Reflect the current frame's vectors in the bisecting plane of the segment x_i -> x_{i+1} (reflection across the plane with normal v1 = x_{i+1} - x_i). This maps t_i to a reflected tangent.
2. Reflect again in the bisecting plane between that reflected tangent and the true next tangent t_{i+1} (normal v2 = t_{i+1} - t_i^L). 

The composition carries (t_i, r_i, s_i) to (t_{i+1}, r_{i+1}, s_{i+1}) with the RMF property to high order. The two reflections are pure vector operations: no trigonometry, no matrix exponential, guaranteed orthonormal output.

**Accuracy and robustness.** The method is **fourth-order accurate, O(h^4)** in step size, versus O(h^2) for the classical projection (point-projection / "rotation by projection") method and the basic rotation method. It is unconditionally stable, handles inflection points gracefully (unlike Frenet), and is cheap. This paper is the de facto standard RMF algorithm.

**Limitations.** It is a discrete sampling method: it produces a sequence of frames, not a closed-form rational frame. For an exact rational frame one needs PH/quaternion curves (Section 3.2) or rational RMF curves (a special subfamily). Boundary/closed-curve frame consistency (the holonomy/total-twist mismatch on closed loops) must be handled by distributing the closure defect.

**Kernel relevance.** Core sweep infrastructure. Keel's sweep operator should default to double-reflection RMF for orienting the profile along the trajectory, sampling the trajectory adaptively and lofting the profile through the frame sequence (then fitting a NURBS surface to tolerance). Use Frenet only when the user explicitly wants curvature-aligned orientation; never use raw Frenet near straight or inflecting segments.

### 5.2 Frenet vs RMF and the sweep construction

**Citation.** (Foundational, multiple.) Bloomenthal, J. (1990). Calculation of reference frames along a space curve. *Graphics Gems*; Klok, F. (1986). Two moving coordinate frames for sweeping along a 3D trajectory. *CAGD, 3*(3), 217-229.

**Method.** A swept surface is S(u,v) = trajectory(u) + Frame(u) * profile(v), where Frame(u) is a moving orthonormal frame (3x3 rotation) along the trajectory and profile(v) is the cross-section in the frame's plane. Klok's formulation gives the surface and its derivatives. The **Frenet frame** {T, N, B} is determined by curvature/torsion but is undefined at inflection points (N flips), introduces torsion-driven roll, and is therefore unsuitable as a general sweep frame. The **RMF** removes the roll. A third option is a **fixed/reference-vector frame** (project a constant up-vector), simple but degenerate when the tangent aligns with the up-vector.

**Accuracy and robustness.** The sweep surface inherits all of the trajectory's and profile's non-rationality through the frame: even with a NURBS trajectory and NURBS profile, the swept surface is generally **not NURBS** (the frame is not rational). Hence the surface is computed as a procedural sweep and then approximated by NURBS to tolerance via skinning the framed profiles.

**Limitations.** Profile-trajectory compatibility: the profile must stay non-self-intersecting under the frame's motion (a profile larger than the trajectory's curvature radius produces self-intersecting sweeps, the 3D analogue of offset self-intersection). Scaling/twisting laws along the trajectory add further DOF and failure modes.

**Kernel relevance.** Defines the sweep data model for Keel: {trajectory spline, profile spline(s), frame rule (RMF default), optional scale/twist law}. Store sweeps procedurally; bake to NURBS on demand.

### 5.3 Exact rational RMF and PH curves

**Citation.** Farouki, R. T., & Sakkalis, T. (2010). Rational rotation-minimizing frames on polynomial space curves of arbitrary degree. *Journal of Symbolic Computation*; and Farouki et al. on RRMF quintics.

**Method.** Certain spatial PH curves admit an **exactly rational RMF** (the "RRMF" curves), where the minimal-twist frame is rational and needs no numerical integration. Characterized via the quaternion coefficients satisfying an additional constraint.

**Accuracy and robustness.** Exact, closed-form, rational frame: a sweep along an RRMF spine with a rational profile is exactly rational (NURBS) end to end.

**Limitations.** RRMF curves are an even more constrained subfamily than PH curves; fitting to one is restrictive.

**Kernel relevance.** Niche but powerful: for the specific case of exact rational tube/pipe sweeps, RRMF spines give a fully exact NURBS sweep. Optional advanced module.

### 5.4 Swept-volume boundaries

**Citation.** Abdel-Malek, K., Blackmore, D., & Joy, K. (2006). Swept volumes: foundations, perspectives, and applications. *International Journal of Shape Modeling*; Blackmore, D., Leu, M. C., & Wang, L. P. (1997). The sweep-envelope differential equation algorithm. *CAD*.

**Method.** The boundary of the *volume* swept by a moving solid (not just a surface profile) is the **envelope** of the moving body, found by the sweep-envelope differential equation: points on the boundary are where the body's surface normal is orthogonal to the instantaneous velocity (the "grazing"/rank-deficiency condition of the sweep Jacobian). The boundary consists of ingress/egress surfaces plus envelope surfaces.

**Accuracy and robustness.** The envelope is generally non-NURBS and may have self-intersections and degeneracies; computed numerically by tracing the grazing set.

**Limitations.** Robustness is hard: detecting and trimming self-overlap of the swept volume is the dominant difficulty, closely related to offset self-intersection.

**Kernel relevance.** Needed for machining-simulation/material-removal features and for "sweep a tool" operations. Lower priority than profile sweeps for an initial kernel, but the grazing condition is the right formal basis if/when implemented.

---

## 6. Blending and Filleting

### 6.1 Vida, Martin & Varady: the blending survey

**Citation.** Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365. https://doi.org/10.1016/0010-4485(94)90023-X

**Method.** The reference taxonomy of blending. It defines a **blend** as a surface that smoothly connects two (or more) "primary" surfaces across their intersection, replacing a sharp edge with a smooth transition that meets each primary with prescribed continuity (G1 tangent, often G2 curvature) along **linkage/trim curves**. Classification axes:

- **Rolling-ball blends.** Imagine a ball of radius r rolling in contact with both primary surfaces; its center traces the **spine curve** (the locus of centers equidistant-by-r from both surfaces), and the contact points trace the two **trimlines** on the primaries. The blend surface is the envelope of the ball, i.e., the strip of the **pipe/canal surface** between the two trimlines. Subtypes:
  - **Constant-radius** (r fixed): blend surface is part of a constant-radius pipe/canal surface; the cross-section normal to the spine is a circular arc of radius r. (This is the everyday "fillet.")
  - **Variable-radius** (r = r(s) a function along the blend): the ball grows/shrinks; the envelope is a general canal surface; cross-sections are arcs of varying radius.
- **Spine-based vs trimline-based** construction: either compute the spine first (offset-intersection of the two primaries offset inward by r) then the trimlines, or fix the trimlines first then build the connecting surface.
- **Rolling-ball vs other cross-sections:** the cross-section need not be circular; conic or general profile cross-sections give styling control while keeping G1/G2.
- **Standing vs rolling:** whether the blend is anchored to fixed boundary curves (standing) or generated by a rolling element.
- **Vertex blends / n-way corners:** where three or more edge-blends meet, an **n-sided patch** must fill the corner with matching continuity to all incident edge blends (Section 6.3).

The survey details continuity bookkeeping (matching tangent planes / curvatures across trimlines), the role of the **spine** and **range/linkage** curves, and the families of surfaces used (rational tensor-product, Gregory patches, transfinite/Coons-type blends).

**Accuracy and robustness.** Constant-radius rolling-ball blends are exactly canal surfaces (rational; Section 4.1); the difficulty is computing the spine robustly (an offset-surface intersection) and the trimlines, and handling the cases where the blend self-intersects or runs off the edge of a primary. Variable-radius blends are harder: the spine is a more complex offset-intersection and the cross-section must vary smoothly.

**Limitations (as a 1994 survey).** Predates much of the robust spine-tracing and modern industrial filleting work; treats topology change (when a fillet swallows a face or two fillets collide) lightly.

**Kernel relevance.** This is the blueprint for Keel's fillet engine. The crucial architectural takeaway: a **constant-radius fillet is a trimmed canal/pipe surface**, so the natural exact representation is a procedural rolling-ball blend (spine + radius + two trimlines + two support faces), with NURBS produced on demand. The spine is an offset-surface intersection, which ties filleting directly to the offset and surface-surface-intersection (SSI) machinery.

### 6.2 Variable-radius and pseudo-rolling-ball blends

**Citation.** Lukacs, G. (1998). Differential geometry of G1 variable radius rolling ball blend surfaces. *CAGD*; Kim, K.-J., & others. C2 pseudo rolling ball filleting. *Computing, 79* (2007). (Springer s00607-006-0195-x.)

**Method.** Variable-radius blends let r vary along the spine (e.g., to taper a fillet or to match different edge widths at the two ends). The rolling-ball model is generalized so the swept sphere changes radius; G1 continuity to the primaries is maintained by keeping the contact (trimline) condition. **Pseudo-rolling-ball** methods relax the exact-ball constraint to gain **C2/G2** continuity and better shape, computing a blend whose cross-sections approximate but are not exactly circular, trading exactness for higher continuity and fairness.

**Accuracy and robustness.** Variable-radius envelopes are general canal surfaces (still rational if spine and r(s) are rational, per Peternell-Pottmann). Pseudo-rolling-ball gives smoother joins (no curvature discontinuity that a pure circular cross-section can cause at the blend boundary) at the cost of being an approximation.

**Limitations.** Variable-radius spine computation is fragile where r changes fast relative to surface curvature; self-intersection risk rises.

**Kernel relevance.** Production kernels must offer variable-radius and curvature-continuous (G2) fillets. Keel should support a radius law r(s) on the blend and offer a G2 cross-section option (conic or pseudo-rolling-ball) in addition to circular G1.

### 6.3 Vertex blends and setback corner patches

**Citation.** Varady, T., Hermann, T. (1996). Best fit surface approximation; and Varady, T., et al. Geometric construction for setback vertex blending. *Computer-Aided Design, 28*(9) (1996), 705-... (ScienceDirect S001044859600070X). Also: Varady, T. (2016). generalized Bezier (GB) multi-sided patches.

**Method.** Where three (or more) filleted edges meet at a vertex, the edge-fillets leave an **n-sided hole** that a **vertex blend** must fill with G1/G2 continuity to every incident fillet. **Setback** vertex blends improve the result by **setting back** (retracting) each edge fillet a short distance from the vertex, so the corner patch has cleaner, longer boundary curves to interpolate. The resulting corner is typically a **2n-sided** region (each setback edge contributes two boundary curves), filled by a multi-sided patch (Gregory patch, transfinite Coons-Gregory blend, or generalized Bezier/GB patch). A **control-frame** construction (repeated chamfering of the corner) approximates the vertex blend and exposes free parameters for interior shape control.

**Accuracy and robustness.** Setback dramatically improves robustness and shape over naive corner filling because it avoids the degenerate short edges and high-curvature pinch at the bare vertex. Multi-sided patches with proper twist-compatibility (the Gregory trick) achieve G1 across all boundaries.

**Limitations.** Twist compatibility at patch corners is delicate; n-sided patches can develop ripples; choosing setback distances is heuristic. Curvature (G2) corners are still an active research area.

**Kernel relevance.** Vertex blending is the single hardest part of a fillet engine and the most common source of fillet failures in commercial kernels. Keel should plan for setback corner handling from the start: edge fillets must be trimmable/retractable, and a multi-sided (Gregory or GB) patch filler must be available for n-way corners.

### 6.4 Symbolic and exact freeform blends

**Citation.** Elber, G., & Kim, M.-S. (1997). A symbolic approach to freeform parametric surface blends. *Journal of Visualization and Computer Animation, 8*(2), 69-80. https://doi.org/10.1002/(SICI)1099-1778(199703)8:2<69::AID-VIS162>3.0.CO;2-D

**Method.** Uses symbolic computation on the B-spline representation (exact products, differences, and composition of B-splines) to construct blends between freeform surfaces, computing trimlines and cross-boundary tangent fields symbolically so the G1 conditions hold by construction rather than by fitting.

**Accuracy and robustness.** Tighter continuity guarantees than sampled fitting; the symbolic B-spline arithmetic keeps everything in exact NURBS where possible.

**Limitations.** Symbolic B-spline arithmetic causes degree and knot growth; can be expensive.

**Kernel relevance.** The symbolic-B-spline toolkit (exact product, derivative, composition operators) is broadly useful in Keel beyond blends (it underlies offset error measurement, distance fields, and curvature). Worth building as shared infrastructure.

---

## 7. Skinning, Lofting, and Developable Design

### 7.1 Piegl & Tiller: skinning and approximate lofting

**Citation.** Piegl, L. A., & Tiller, W. (1996/2002). Surface skinning revisited. *The Visual Computer*; and Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.), Springer, chapters on skinning/lofting and on reducing control points in interpolation.

**Method.** **Skinning (lofting)** passes a tensor-product B-spline surface through a sequence of section curves C_k(u). The standard construction: (1) make all sections **compatible** (same degree, same knot vector) by degree-elevating and knot-merging; (2) treat each control-point row as data and interpolate across the sections in the v-direction. The fatal practical problem is step (1): **merging the knot vectors of all sections unions all their knots**, so a loft of m sections with distinct knots produces a surface whose u-knot vector is the union of all section knots, i.e., the control-net width **explodes** (every section is refined up to the global union). **Approximate skinning / lofting** fixes this: instead of exact compatibility, choose a single **common (reduced) knot vector** and **approximate** each section on it (least squares with knot removal / common-knot determination), so the surface stays small while staying within tolerance of each section. Piegl & Tiller also give knot-removal-based control-point reduction after interpolation.

**Accuracy and robustness.** Exact skinning is interpolatory but blows up in size; approximate skinning trades a controlled deviation (tolerance epsilon from each section) for a dramatically smaller, fairer surface. Approximate lofting is what production kernels actually use.

**Limitations.** Choosing the common knot vector well is the crux; poor choice either misses tolerance or wastes control points. Section ordering, parameterization (the v-knots), and orientation consistency all affect fairness and can introduce twists/wrinkles.

**Kernel relevance.** Loft/sweep-with-multiple-profiles are core modeling features and both rest on skinning. Keel must implement **approximate skinning** with a common-knot strategy and a deviation tolerance, not naive knot-union skinning, or surfaces will bloat. This same machinery bakes procedural sweeps to NURBS (Section 5).

### 7.2 Cross-section compatibility and the knot-merging blowup

**Citation.** (See 7.1; plus) Tai, C.-L., & Loe, K.-F. and later "Reducing control points in lofted B-spline surface interpolation using common knot vector determination" (ResearchGate 220584278).

**Method.** Formalizes the common-knot-vector determination: pick knots that (a) cover the union's significant features and (b) keep count low, then approximate every section within tolerance on that knot vector. Some methods detect "dominant" knots shared across sections and keep only those.

**Kernel relevance.** Concrete algorithm for the blowup fix; this is the piece to implement inside Keel's loft.

### 7.3 Developable surface design

**Citation.** Pottmann, H., & Wallner, J. (1999). Approximation algorithms for developable surfaces. *Computer Aided Geometric Design, 16*(6), 539-556. https://doi.org/10.1016/S0167-8396(99)00012-6 . Also: Pottmann, H., & Farin, G. (1995). Developable rational Bezier and B-spline surfaces. *CAGD, 12*; Aumann, G. developable Bezier patches.

**Method.** A **developable surface** is a ruled surface with zero Gaussian curvature (it unrolls to the plane without stretching), important for sheet metal, fabric, plywood, composites. Pottmann & Wallner exploit **projective duality**: a developable surface is the envelope of a one-parameter family of planes, so it is dual to a **curve in the dual space of planes**. Designing the developable becomes designing a NURBS curve in dual space (with an appropriate metric), after which the surface is recovered in **dual rational B-spline** form. They give linear approximation algorithms (including multiscale) and control the **curve of regression** (the cuspidal edge where the developable folds).

**Accuracy and robustness.** Duality makes the developability constraint *automatic* (it is built into the dual representation), avoiding the nonlinear constraint that direct primal design fights. Approximation is linear and stable.

**Limitations.** The dual metric and the regression curve need care; not all desired shapes are developable, and joining developable strips with continuity is constrained.

**Kernel relevance.** If Keel targets sheet-metal/composite workflows, developable surfaces deserve a first-class dual representation. Otherwise this is a specialized add-on. The duality idea is also conceptually useful for the canal/offset families.

---

## 8. Ruled, Revolved, and Extruded Surfaces: Exact NURBS

**Citation.** Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.), Springer, ch. 8 (construction of common surfaces).

**Method.** Several constructed surfaces ARE exactly NURBS and should be stored exactly:

- **Ruled surface** between two NURBS curves C0(u), C1(u): make the two curves compatible (degree, knots), then S(u,v) = (1-v) C0(u) + v C1(u). Exact NURBS, bilinear in v.
- **Surface of revolution**: rotate a NURBS profile about an axis. A full circle is exactly a **rational quadratic NURBS** (degree 2, 9 control points with weights, or 7-point/4-arc variants), so the revolved surface is an exact NURBS surface of revolution. This is the textbook win: cylinders, cones, spheres, tori are exact NURBS via the rational-circle generator.
- **Extrusion (general cylinder)**: sweep a NURBS profile along a straight vector; exact NURBS (linear in the sweep direction).

**Accuracy and robustness.** Exact, no tolerance needed. The only subtlety is the rational-circle weights (w = cos(half-angle) at the midpoints) and the pole/seam handling on full revolutions (degenerate edges at the poles of a sphere).

**Limitations.** Poles create degenerate (zero-length) edges and parameterization singularities the topology layer must handle. Full closed revolutions need periodic or seamed knot vectors.

**Kernel relevance.** These are the "free" exact surfaces. Keel should generate revolutions/extrusions/rulings directly as NURBS (or keep analytic types: plane, cylinder, cone, sphere, torus, as first-class procedural surfaces with NURBS only for IO). Robust pole/seam handling at the topology layer is mandatory.

---

## 9. Self-Intersection, Trimming, and the Medial Axis

### 9.1 Local and global self-intersection of offsets

**Citation.** Pekerman, D., Elber, G., & Kim, M.-S. (2008). Self-intersection detection and elimination in freeform curves and surfaces. *Computer-Aided Design, 40*(2), 150-159; and Seong, J.-K., Elber, G., & Kim, M.-S. Trimming local and global self-intersections in offset curves/surfaces using distance maps. *Computer-Aided Design, 38*(3) (2006), 183-193. https://doi.org/10.1016/j.cad.2005.08.002

**Method.** Two failure modes:

- **Local self-intersection**: where the offset distance exceeds the radius of curvature, **d > 1/kappa**, the offset develops a cusp/loop locally. Detection is cheap: compute kappa(t) (rational if curve is PH, else sampled) and flag where d*kappa(t) >= 1. In 3D, the analogous condition is d >= 1/kappa_max (max principal curvature), i.e., the offset hits the focal surface.
- **Global self-intersection**: distant parts of the offset cross each other even where each is locally fine (e.g., a narrow neck offset inward). Detection needs a global test, not a local curvature test.

The **distance-map** approach computes a scalar field of distance from the original geometry and uses its structure to identify and **trim** the invalid loops: the valid offset is the set of points at exactly distance d whose nearest footpoint is unique and on the correct side; multiply-covered regions (where two footpoints are at distance d) are exactly the self-overlap to remove. Trimming clips the offset at the self-intersection points and discards the inner loops.

**Accuracy and robustness.** Distance maps catch both local and global self-intersection in one framework and produce a clean trimmed offset. Robustness depends on accurately locating the self-intersection points (a curve/curve or surface/surface intersection problem).

**Limitations.** Surface self-intersection trimming is expensive and numerically delicate; near-tangential self-overlaps are hard to resolve cleanly.

**Kernel relevance.** This is the trimming half of every offset/shell/fillet operation. Keel needs: (1) a fast local curvature pre-check (d vs 1/kappa_max), (2) a global self-intersection detector, and (3) a trimmer. These are shared by 2D offset, surface offset, shelling, and rolling-ball spine computation.

### 9.2 The medial axis connection

**Citation.** Patrikalakis, N. M., & Maekawa, T. (2002). *Shape Interrogation for Computer Aided Design and Manufacturing*. Springer. (See the focal-surface / medial-axis chapters.) Plus Wolter, F.-E. on medial axis and offsets.

**Method.** The **medial axis** of a region is the locus of centers of maximal inscribed balls (equivalently, points with at least two equidistant nearest-boundary points). The set of self-intersections of inward offsets, as d increases, **traces the medial axis**: each medial-axis point is reached by the offset at d equal to its distance to the boundary, and that is exactly where two parts of the offset meet. So "where does the offset self-intersect / what is the maximum safe inward offset" is answered by the medial axis and its **radius function**. The minimum medial radius is the largest wall thickness that can be removed before the offset degenerates; the medial axis also predicts **offset topology changes** (when an inward offset splits a region into two, or a hole closes up, the change happens at a medial-axis branch/junction).

**Accuracy and robustness.** The medial axis is notoriously unstable to boundary perturbation (small bumps spawn spurious branches), which is why direct medial computation is avoided in kernels; but the *relationship* is the right mental model for predicting offset/shell topology change.

**Limitations.** Exact medial axis is hard and unstable; usually only the relevant local structure (max safe distance, branch near a self-intersection) is computed, not the full medial axis.

**Kernel relevance.** Two uses: (1) the minimum distance from a face to the rest of the boundary bounds the safe shell thickness; (2) when a shell/offset produces a topology change (a face disappears, a region splits), the event is a medial-axis junction crossing, which is the signal the topology layer must detect and handle.

---

## 10. Shelling and Thickening of Solids

**Citation.** Various; representative: "Shelling and offsetting bodies," *Proceedings of the 3rd ACM Symposium on Solid Modeling and Applications* (1995), 95-104 (ACM 218013.218088); and "Shelling Algorithm in Solid Modeling," *Lecture Notes (CGGM)* (Springer 978-3-540-30497-5_46).

**Method.** **Shelling** hollows a solid to a thin wall of thickness t (with selected faces removed as openings). The standard B-rep algorithm:

1. **Offset each face** of the B-rep inward (or outward) by t, producing an offset surface per face (Section 2.3 / Section 4 for exact cases).
2. **Reconnect**: adjacent offset faces no longer share edges; convex original edges leave **gaps** (filled by extending/intersecting the offset faces, or by adding fillet/rounded material), and concave original edges cause offset faces to **overlap/self-intersect** (resolved by intersecting and trimming).
3. **Rebuild topology**: compute the new edges as offset-face/offset-face intersections, trim, and stitch into a valid B-rep; the result is the original boundary plus the offset boundary plus opening faces.
4. **Boolean closure**: some formulations realize the shell as a regularized Boolean (original solid minus the inward-offset solid), which inherently handles the gap/overlap bookkeeping but needs a robust offset solid first.

**Accuracy and robustness.** The hard part is step 2/3: gaps at convex edges and self-intersections at concave edges, plus **topology changes** when t exceeds local feature size (a thin region's two walls collide, a small face vanishes). This is exactly the offset self-intersection (Section 9) and medial-axis (Section 9.2) problem at the solid level. Modern approaches inject **signed distance fields (SDF)** to make the self-intersection resolution robust (compute the offset as an SDF level set, which is self-intersection-free by construction, then re-extract a B-rep), at the cost of an approximation step.

**Limitations.** "Notoriously difficult to implement robustly" is the consensus: commercial kernels (Parasolid, ACIS) accumulated hundreds of special-case handlers for shell topology changes over decades. Pure-NURBS face-offset-and-stitch is brittle near small features; SDF/level-set is robust but approximate and needs re-fitting.

**Kernel relevance.** Shelling is a flagship feature and a robustness graveyard. Keel's strategy should be: (1) exact offsets for analytic/canal faces, approximate offsets for freeform faces; (2) a robust offset-face intersection + trim pipeline reusing Section 9; (3) explicit topology-change detection (face vanish, wall collision) keyed to local feature size / medial radius; (4) consider an SDF-based fallback for the hardest cases, with NURBS re-extraction to tolerance. Plan the special-case handling as an extensible rule set, not a monolith.

---

## 11. Toolpath Generation: Where Offsets Meet Manufacturing

### 11.1 Constant scallop-height toolpaths

**Citation.** Lee, E. (2003). Contour offset approach to spiral toolpath generation with constant scallop height. *Computer-Aided Design, 35*(6) (S0010448501001361); and Feng, H.-Y., & Li, H. (2002). Constant scallop-height tool path generation for three-axis sculptured surface machining. *Computer-Aided Design, 34*(9), 647-654.

**Method.** In 3-axis ball-end milling, the ridge of uncut material between adjacent passes is the **scallop**; keeping scallop height h constant gives the shortest total path for a given finish. The construction uses **two offset surfaces of the design surface**: the **tool-center surface** (the design surface offset outward by the ball radius R, i.e., where the cutter center must lie) and the **scallop surface** (offset by R minus h). Successive toolpaths are generated so that the geometric **path interval** (sideways step) varies with surface curvature to hold h constant: on flat regions the step is wide, on curved regions narrow. Computing the next path is itself a **geodesic-offset / constant-distance** problem on the surface.

**Accuracy and robustness.** Iso-scallop paths are 22-50% shorter than iso-planar (constant-step) paths for the same finish. The challenge is uneven path spacing and interference removal (the cutter must not gouge), both offset-surface problems.

**Kernel relevance.** A CAM-facing kernel benefits from a geodesic-offset operator on surfaces (offsetting curves *within* a surface, distinct from normal offset). Keel's offset module should expose both normal offset (for shells) and geodesic/in-surface offset (for toolpaths and seam allowances).

### 11.2 PH curves for feedrate-controlled toolpaths

**Citation.** Farouki, R. T., Manni, C., Sestini, A., et al. Real-time CNC interpolators for Pythagorean-hodograph curves. *(multiple, CAGD/IJMTM).*

**Method.** Because PH curves have **polynomial arc length**, the CNC interpolator can compute the exact parameter value for a desired arc-length increment in real time (constant feedrate, or a programmed feedrate law), without the chord-length lookup tables generic NURBS toolpaths require. The rational offset gives exact cutter compensation.

**Kernel relevance.** Reinforces Section 3: PH spline representation of toolpath spines is the right exact form when feedrate fidelity and exact cutter comp matter.

---

## 12. Constructed-Surface Architecture for Keel

The literature points to one overriding design principle: **store constructed surfaces in their most exact natural form, and produce tolerance-controlled NURBS only at the boundary (display, export, intersection, and persistence) where a downstream consumer truly needs polynomials.** Concretely:

**Representation policy (a small surface/curve type hierarchy):**

1. **Exact NURBS by construction** (store as NURBS, no approximation): ruled surfaces, extrusions, surfaces of revolution, and all analytic primitives (plane, cylinder, cone, sphere, torus) via the rational-circle generator (Section 8). Keep analytics as procedural types with cached NURBS for IO.
2. **Exact non-NURBS, rational where possible** (store procedurally; bake to NURBS on demand):
   - **Constant- and variable-radius fillets** as rolling-ball/canal surfaces: {spine, radius law r(s), two support faces, two trimlines}, knowing the surface is a (trimmed) canal surface, rational when spine and r are rational (Sections 4.1, 6). 
   - **Pipe/tube surfaces** as canal surfaces; use **PH / RRMF spines** when exact length and exact rational frames are wanted (Sections 3, 5.3).
   - **Sweeps** as {trajectory, profile(s), frame rule = RMF via double reflection, optional scale/twist} (Section 5).
   - **Offsets of analytic / PN / canal faces**: exact rational offset (Sections 1.2, 4.2).
3. **Tolerance-controlled NURBS approximation** (the genuinely non-rational cases): offsets of freeform curves/surfaces, generic sweep and skin surfaces baked from procedural form, vertex/corner patches. Always carry the deviation tolerance and the procedural source so the surface can be re-baked finer.

**Shared infrastructure to build once and reuse everywhere:**

- **Offset core**: 2D offset (Tiller-Hanson default + adaptive refit fallback, Section 2.1); surface normal offset with adaptive knot insertion (Section 2.3); geodesic/in-surface offset (Section 11.1).
- **Self-intersection + trimming**: local curvature pre-check d vs 1/kappa_max, global self-intersection detection, distance-map trimming (Section 9.1). This single module serves 2D offset, surface offset, shelling, and fillet-spine computation.
- **Medial / feature-size estimator**: max safe inward offset and topology-change prediction from min boundary distance (Section 9.2), feeding shelling.
- **Frame engine**: double-reflection RMF (Section 5.1) plus optional Frenet and fixed-up frames; exact rational frames for PH/RRMF spines (Section 5.3).
- **Skinning engine**: approximate skinning with common-knot determination (Sections 7.1-7.2); used by loft and to bake sweeps to NURBS.
- **Symbolic B-spline arithmetic** (exact product/derivative/composition, Section 6.4): underpins offset error measurement, distance fields, curvature evaluation, and symbolic blends.

**Fillet engine (the highest-effort subsystem):**

- Compute the **spine** as an offset-surface intersection (two support faces each offset inward by r, intersected), reusing the offset + SSI machinery.
- Build the constant-radius blend as a trimmed canal surface (exact), with a G2/conic cross-section option (Section 6.2) and a variable-radius law.
- Handle **vertex blends with setback** from day one: edge fillets must be retractable, and an n-sided (Gregory / generalized-Bezier) corner patch filler must exist (Section 6.3). Plan setback as the default corner strategy.
- Treat fillet **topology change** (fillet swallows a face, two fillets collide) as the rule-extensible hard part, paralleling shelling.

**Shelling:**

- Per-face offset (exact for analytic/canal/PN faces, approximate for freeform), then robust offset-face intersection + trim + stitch (Section 10), with explicit topology-change detection keyed to medial/feature size. Keep an SDF-based level-set fallback for the worst cases, with NURBS re-extraction to tolerance.

**Tolerance discipline:** every approximate construction carries (a) the procedural/exact source, (b) the achieved deviation, and (c) the requested tolerance, so the kernel can re-bake at higher fidelity for downstream intersection or export without losing the design intent. The recurring lesson from Elber-Lee-Kim, Piegl-Tiller, and the shelling literature is that **the approximation is a cache, not the truth**: keep the truth.

---

## References

1. Abdel-Malek, K., Blackmore, D., & Joy, K. (2006). Swept volumes: foundations, perspectives, and applications. *International Journal of Shape Modeling, 12*(1), 87-127.
2. Blackmore, D., Leu, M. C., & Wang, L. P. (1997). The sweep-envelope differential equation algorithm and its application to NC machining verification. *Computer-Aided Design, 29*(9), 629-637.
3. Choi, H. I., Lee, D. S., & Han, H. P. (2002). Euler-Rodrigues frames on spatial Pythagorean-hodograph curves. *Computer Aided Geometric Design, 19*(8), 603-620.
4. Coquillart, S. (1987). Computing offsets of B-spline curves. *Computer-Aided Design, 19*(6), 305-309.
5. Elber, G., & Kim, M.-S. (1997). A symbolic approach to freeform parametric surface blends. *Journal of Visualization and Computer Animation, 8*(2), 69-80.
6. Elber, G., Lee, I.-K., & Kim, M.-S. (1997). Comparing offset curve approximation methods. *IEEE Computer Graphics and Applications, 17*(3), 62-71.
7. Farouki, R. T. (2008). *Pythagorean-Hodograph Curves: Algebra and Geometry Inseparable* (Geometry and Computing, Vol. 1). Springer.
8. Farouki, R. T. (2008/2018). Pythagorean-hodograph curves: a survey of recent advances. (Survey.)
9. Farouki, R. T., & Neff, C. A. (1990). Algebraic properties of plane offset curves. *Computer Aided Geometric Design, 7*(1-4), 101-127.
10. Farouki, R. T., & Sakkalis, T. (1990). Pythagorean hodographs. *IBM Journal of Research and Development, 34*(5), 736-752.
11. Farouki, R. T., & Sakkalis, T. (2010). Rational rotation-minimizing frames on polynomial space curves of arbitrary degree. *Journal of Symbolic Computation, 45*(8), 844-856.
12. Feng, H.-Y., & Li, H. (2002). Constant scallop-height tool path generation for three-axis sculptured surface machining. *Computer-Aided Design, 34*(9), 647-654.
13. Hoschek, J. (1988). Spline approximation of offset curves. *Computer Aided Geometric Design, 5*(1), 33-40.
14. Klass, R. (1983). An offset spline approximation for plane cubic splines. *Computer-Aided Design, 15*(5), 297-299.
15. Klok, F. (1986). Two moving coordinate frames for sweeping along a 3D trajectory. *Computer Aided Geometric Design, 3*(3), 217-229.
16. Kumar, G. V. V. R., Shastry, K. G., & Prakash, B. G. (2002). Qualitative and quantitative comparisons of B-spline offset surface approximation methods. *Computer-Aided Design, 34*(5), 351-363.
17. Lee, E. (2003). Contour offset approach to spiral toolpath generation with constant scallop height. *Computer-Aided Design, 35*(6), 511-518.
18. Lukacs, G. (1998). Differential geometry of G1 variable radius rolling ball blend surfaces. *Computer Aided Geometric Design, 15*(6), 585-613.
19. Maekawa, T. (1999). An overview of offset curves and surfaces. *Computer-Aided Design, 31*(3), 165-173.
20. Patrikalakis, N. M., & Maekawa, T. (2002). *Shape Interrogation for Computer Aided Design and Manufacturing*. Springer.
21. Pekerman, D., Elber, G., & Kim, M.-S. (2008). Self-intersection detection and elimination in freeform curves and surfaces. *Computer-Aided Design, 40*(2), 150-159.
22. Peternell, M., & Pottmann, H. (1997). Computing rational parametrizations of canal surfaces. *Journal of Symbolic Computation, 23*(2-3), 255-266.
23. Piegl, L. A., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer.
24. Piegl, L. A., & Tiller, W. (2002). Surface skinning revisited. *The Visual Computer, 18*(4), 273-283.
25. Pottmann, H. (1995). Rational curves and surfaces with rational offsets. *Computer Aided Geometric Design, 12*(2), 175-192.
26. Pottmann, H., & Farin, G. (1995). Developable rational Bezier and B-spline surfaces. *Computer Aided Geometric Design, 12*(5), 513-531.
27. Pottmann, H., & Wallner, J. (1999). Approximation algorithms for developable surfaces. *Computer Aided Geometric Design, 16*(6), 539-556.
28. Seong, J.-K., Elber, G., & Kim, M.-S. (2006). Trimming local and global self-intersections in offset curves/surfaces using distance maps. *Computer-Aided Design, 38*(3), 183-193.
29. Sir, Z., Feichtinger, R., & Juttler, B. (2006). Approximating curves and their offsets using biarcs and Pythagorean hodograph quintics. *Computer-Aided Design, 38*(6), 608-618.
30. Tiller, W., & Hanson, E. G. (1984). Offsets of two-dimensional profiles. *IEEE Computer Graphics and Applications, 4*(9), 36-46.
31. Varady, T., et al. (1996). Geometric construction for setback vertex blending. *Computer-Aided Design, 28*(9), 705-718.
32. Vida, J., Martin, R. R., & Varady, T. (1994). A survey of blending methods that use parametric surfaces. *Computer-Aided Design, 26*(5), 341-365.
33. Wang, W., Juttler, B., Zheng, D., & Liu, Y. (2008). Computation of rotation minimizing frames. *ACM Transactions on Graphics, 27*(1), Article 2, 1-18.
34. "Shelling and offsetting bodies." (1995). *Proceedings of the 3rd ACM Symposium on Solid Modeling and Applications*, 95-104.
