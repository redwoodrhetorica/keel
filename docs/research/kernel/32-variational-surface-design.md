# Variational and Energy-Minimization Surface Design

**Purpose.** A deep literature dossier on the variational surfacing paradigm: building one high-quality surface as the minimizer of a fairness/energy functional subject to constraints, rather than by interpolation or construction. This is the distinctive technical core identified in the XNURBS capability audit (kernel/31): a single global energy-minimization solve that absorbs arbitrary mixed constraints and returns the smoothest feasible NURBS surface. This dossier covers the variational method itself (the energy functionals, the deformable-model lineage, the discretization to a control-point quadratic, and the numerics). A companion dossier (kernel/33) covers the constraint and continuity machinery (G0/G1/G2/G3 enforcement, conflict handling). It complements rather than repeats the transfinite/N-sided construction path in kernel/26 and goes deeper than the fairing mentions in nurbs/03 section 7.

**Scope of this file.**
- The variational paradigm and its contrast with classical constructive surfacing.
- Fairness energy functionals stated explicitly: membrane, thin-plate, simplified quadratic surrogates, true geometric (curvature) energies, minimum variation surfaces, Willmore energy.
- The foundational deformable-model / variational lineage (Terzopoulos, Celniker-Gossard, Welch-Witkin, Moreton-Sequin, Greiner, Kallay, Hagen).
- Discretization: reducing the energy to a quadratic form P^T K P in control points (the finite-element / stiffness-matrix view), and how constraints turn it into a constrained quadratic program.
- The (N^T N + lambda K) unification with the nurbs/03 penalized-least-squares fairing engine.
- Modern real-time variational editing (Botsch-Kobbelt, Kobbelt discrete fairing, Halstead-Kass-DeRose), thin-plate / RBF surfaces (Duchon).
- Quality and failure modes (the wavy-interior failure XNURBS reviewers report).
- Numerics: conditioning, sparsity, solver choice, nonlinearity of true geometric energy.

---

## 1. The variational paradigm

### 1.1 Definition

A constructive surfacing operator *interpolates* or *sweeps* a clean input: a Coons patch blends four boundary curves, a Gordon surface fits a rectangular curve network, a loft skins an ordered set of sections, a sweep moves a profile along rails. The output is a deterministic function of topologically regular input, and the operator fails (or produces garbage) when the input is not clean: gapped boundaries, the wrong number of curves, non-contiguous edges, or an irregular network all break it.

A variational surfacing operator instead defines the surface implicitly as the solution of an optimization:

```
  minimize   E_fair(S)         (a fairness / energy functional)
  subject to C(S) = 0 (or <= tol)   (interpolation, boundary, continuity constraints)
```

over a chosen surface representation S (here a NURBS / B-spline patch). The surface is "the smoothest one that satisfies the constraints." This is exactly the published XNURBS framing (kernel/31, Technical Approach): the wooden-batten analogy is the 1D thin-beam bending energy whose 2D analog is thin-plate energy, and the surface returned is the minimum-energy feasible member of a family.

### 1.2 Why it absorbs messy input

The architectural payoff is that input stops being *topology that must be clean* and becomes *constraints that can be soft or hard*. A gap between two boundary curves is not a fatal "loop not closed" error; it is simply a region where no position constraint is active, and the fairness term fills it with the smoothest interpolant. Overlapping or redundant curves are extra (possibly inconsistent) constraints that the solve reconciles in a least-squares sense. Open boundaries are a constraint set that does not enclose a region; the energy term still has a unique minimizer once enough of the surface is pinned. This is why XNURBS tolerates gapped, open, disjoint, and overlapping input that breaks classical loft/network builders (kernel/31 section 3): a constructor demands a clean recipe; an optimizer accepts a constraint soup and returns the best surface for it.

The contrast with kernel/26 (transfinite / N-sided) is the key architectural decision point. Transfinite methods (Coons, Gregory, Gordon-Coons, midpoint/Charrot-Gregory, side-vertex) are *constructive*: closed-form blends of boundary data, exact and cheap, but they require well-formed N-sided boundary loops and give limited interior control. The variational path is *optimization-based*: one global solve, more expensive, but it absorbs imperfect input and gives a directly controllable interior fairness. Keel wants both: transfinite for clean, fast, exact N-sided fills, variational for the messy XNURBS-class cases.

---

## 2. Fairness energy functionals

This section states the functionals explicitly. Let S(u,v) be the surface (a vector-valued map of the parameter domain into R^3), with subscripts denoting partial derivatives.

### 2.1 Membrane (stretching) energy, first order

```
  E_membrane(S) = integral_D ( |S_u|^2 + |S_v|^2 ) du dv
```

This penalizes stretching. Its Euler-Lagrange equation is the Laplace equation Delta S = 0 (harmonic surfaces, "soap-film" / minimal-area-like behavior in the linearized sense). A pure membrane surface is taut: it can have tangent-plane discontinuities at constraints (it minimizes area-like energy, so it pulls toward constraints with creases). It is rarely used alone for fair design but is a stabilizing term and the natural energy when only C0 behavior is wanted.

### 2.2 Thin-plate (bending) energy, second order

```
  E_thinplate(S) = integral_D ( |S_uu|^2 + 2 |S_uv|^2 + |S_vv|^2 ) du dv
```

This penalizes bending (second derivatives). Its Euler-Lagrange equation is the biharmonic equation Delta^2 S = 0. This is the workhorse fairness functional: it produces visually smooth, gently curved surfaces and is the parametric analog of the wooden-batten / thin-elastic-plate physical energy. It is the standard XNURBS-style "surface energy." Note it is a *parametric* (parameterization-dependent) energy, not a true geometric one: it is quadratic in S and therefore cheap, but its value changes under reparameterization, so it is only an approximation of true geometric fairness, valid near an arc-length / isometric parameterization.

### 2.3 Blended first-plus-second order (the practical default)

Most practical engines minimize a weighted sum:

```
  E(S) = integral_D ( alpha (|S_u|^2 + |S_v|^2)
                    + beta  (|S_uu|^2 + 2 |S_uv|^2 + |S_vv|^2) ) du dv
```

with weights alpha (membrane / tension) and beta (plate / stiffness). This is the Celniker-Gossard and Welch-Witkin form, and it is what "tension" sliders adjust in commercial tools. Generalizing to order k gives the polyharmonic energy with Euler-Lagrange equation Delta^k S = 0 (membrane k=1, thin-plate k=2, used by Botsch-Kobbelt for C0/C1/C2 boundary behavior).

### 2.4 Simplified quadratic surrogates (Greiner)

Greiner's insight: true curvature-based fairness is nonlinear and expensive, but a *uniformly distributed curvature* can be approximated by quadratic functionals that minimize at interactive speed. Greiner's surrogates are simplified thin-plate energies for parametric surfaces (or simplified MVC functionals), constructed so that minimizing them is a single linear solve while still yielding fair, even-curvature surfaces. These are the energies actually used in practice when speed matters, and they are precisely the quadratic P^T K P forms of section 4.

### 2.5 True geometric energy (total curvature), nonlinear

The geometrically correct (reparameterization-invariant) bending energy is the total curvature:

```
  E_geom(S) = integral_S ( kappa_1^2 + kappa_2^2 ) dA
```

where kappa_1, kappa_2 are the principal curvatures and dA is the surface area element. Equivalently, since kappa_1^2 + kappa_2^2 = 4H^2 - 2K (H mean curvature, K Gaussian curvature), this is closely related to the Willmore energy. This is the "right" fairness measure: invariant to how the surface is parameterized. But it is *nonlinear* in the control points (the curvatures involve a ratio of first and second fundamental forms and a square root in the area element), so minimizing it requires iteration (Gauss-Newton / Newton, or a curvature flow). The thin-plate energy of 2.2 is the linearization of E_geom about an isometric parameterization, which is why kernels default to thin-plate and only escalate to the geometric energy for the highest quality.

### 2.6 Minimum variation surfaces (Moreton-Sequin)

Moreton and Sequin argued that even total curvature is not the best fairness measure for highlight-clean Class-A surfaces; what designers want is *constant* curvature, i.e. minimal *variation* of curvature. Their MVS functional integrates the squares of the derivatives of the principal curvatures taken in their own principal directions:

```
  E_MVS(S) = integral_S ( (d kappa_1 / d e_1)^2 + (d kappa_2 / d e_2)^2 ) dA
```

where e_1, e_2 are the principal directions. This is a third-order geometric penalty (a sixth-order Euler-Lagrange equation), strongly nonlinear, and very expensive, but it produces the most predictable, intuitive, highlight-smooth surfaces. It is the gold standard for fairness quality and the conceptual target XNURBS gestures at with "Class A," even if commercial tools fall back to cheaper quadratic surrogates.

### 2.7 Willmore energy

```
  E_Willmore(S) = integral_S H^2 dA
```

the integral of squared mean curvature. Conformally invariant; minimized by the round sphere (value 4 pi) among closed surfaces. Its gradient flow (Willmore flow) is a fourth-order geometric PDE used for surface fairing and hole filling with curvature continuity. In CAGD it is one of the principled nonlinear fairness energies; in practice it is discretized and flowed rather than solved in closed form.

**Ordering for a kernel.** Cheapest and linear: membrane, thin-plate, Greiner quadratic surrogates (one linear solve). More expensive and nonlinear, higher quality: total curvature, Willmore, MVS (iterated solves / flows). A production engine offers the quadratic surrogate as default and the geometric energy as an optional high-quality mode.

---

## 3. The deformable-model and variational lineage (per-source)

### 3.1 Terzopoulos, Platt, Barr & Fleischer (1987) - Elastically deformable models

**Citation.** Terzopoulos, D., Platt, J., Barr, A., & Fleischer, K. (1987). Elastically deformable models. *Computer Graphics (Proceedings of ACM SIGGRAPH 87)*, 21(4), 205-214.

**Formulation.** The origin of physically based / variational shape in graphics. A deformable curve, surface, or solid is governed by a potential energy of deformation built from differential geometry: the membrane (stretching, first fundamental form) and bending (flexing, second fundamental form) energies, written for a surface essentially as the integral of weighted squared first and second derivatives (the alpha/beta form of section 2.3, generalized with tensorial stiffness coefficients). The shape evolves by solving the Euler-Lagrange / Lagrangian equations of motion m x_tt + damping + (delta E / delta x) = f, where delta E / delta x is the variational derivative of the deformation energy and f are applied forces and constraints. They discretize with finite differences / finite elements and integrate in time, so the surface "relaxes" toward minimum energy under loads and constraints.

**Limitations.** Dynamic (time-stepped) rather than a direct equilibrium solve; aimed at animation and physical simulation, not at producing an exact NURBS for CAD. The energy is parameterization-dependent.

**Kernel relevance.** Establishes the entire conceptual frame Keel's variational surfacer sits in: a surface as the equilibrium of an energy functional under constraints. The static (equilibrium) special case, delta E / delta x = 0 subject to constraints, is exactly the variational surface solve. Keel does not need the time-dependent dynamics; it needs the equilibrium solve, but this is the paper that named the energies.

### 3.2 Celniker & Gossard (1991) - Deformable curve and surface finite elements ("ShapeWright")

**Citation.** Celniker, G., & Gossard, D. (1991). Deformable curve and surface finite-elements for free-form shape design. *Computer Graphics (Proceedings of ACM SIGGRAPH 91)*, 25(4), 257-266. (Based on Celniker's MIT PhD thesis, *ShapeWright: Finite Element Based Free-Form Shape Design*, 1990.)

**Formulation.** Brings Terzopoulos energies into a CAD finite-element framework. The surface is built from C1 finite elements (cubic in position, parabolic in normal) and deforms to minimize a quadratic energy

```
  E = integral ( alpha_11 |S_u|^2 + alpha_22 |S_v|^2
              + beta_11 |S_uu|^2 + 2 beta_12 |S_uv|^2 + beta_22 |S_vv|^2 ) dA
              - work done by applied loads,
```

subject to user geometric constraints (interpolated points, curves) and "loads" (point/pressure forces, springs). Substituting the FE shape functions reduces E to a quadratic form in the nodal degrees of freedom: E = (1/2) U^T K U - U^T F, with K the element-assembled stiffness matrix. Minimization subject to constraints is the linear system K U = F with constraints applied (Lagrange multipliers or reduction). The user works in three phases (define mesh, apply constraints/loads, solve), and shapes deform like elastic membranes and plates.

**Limitations.** Custom C1 finite elements, not standard NURBS, so output needed conversion for a NURBS kernel; quadratic (linearized) energy only; interactivity limited by 1991 solver speed.

**Kernel relevance.** This is the canonical "energy becomes P^T K P, minimization becomes a linear solve" template, and it states the stiffness-matrix view explicitly. Keel's variational engine is the NURBS-basis version of exactly this: assemble K from the fairness functional, assemble constraints, solve. The alpha/beta weights are the tension/stiffness sliders.

### 3.3 Welch & Witkin (1992) - Variational surface modeling

**Citation.** Welch, W., & Witkin, A. (1992). Variational surface modeling. *Computer Graphics (Proceedings of ACM SIGGRAPH 92)*, 26(2), 157-166.

**Formulation.** The seminal CAD-side statement of the paradigm. The user is presented an "infinitely malleable" surface with no fixed control mesh; they attach point and curve constraints that become direct-manipulation handles. Within those constraints the shape is fully determined by a fairness criterion. The two stated energies are exactly:

```
  E_membrane  = integral_D ( |S_u|^2 + |S_v|^2 ) du dv
  E_thinplate = integral_D ( |S_uu|^2 + 2 |S_uv|^2 + |S_vv|^2 ) du dv
```

(typically a weighted blend). The surface is represented as a B-spline, and crucially they support *nonuniform subdivision* of the B-spline so degrees of freedom are added adaptively where constraints demand more shape control. Substituting the B-spline form into the quadratic energy reduces it to a quadratic in the control-point coordinates; minimization subject to the (linear) interpolation and control constraints is a constrained quadratic program, solved as a sparse linear system (the energy gives a symmetric positive (semi)definite stiffness matrix, constraints via Lagrange multipliers). The result re-solves interactively as the user drags constraints.

**Limitations.** Quadratic (parameterization-dependent) energies, so not true geometric fairness; the adaptive subdivision and re-solve are the expensive parts; degenerate / under-constrained cases need the membrane term or extra regularization to stay well-posed.

**Kernel relevance.** This is the closest published analog to the XNURBS core and the reference design for Keel's variational surfacer: B-spline surface, thin-plate-plus-membrane energy, point/curve constraints as handles, reduction to a constrained sparse linear solve, adaptive DOF to meet precision. Keel should treat Welch-Witkin as the baseline algorithm and layer continuity constraints (kernel/33) and the geometric-energy upgrade on top.

### 3.4 Moreton & Sequin (1992) - Functional optimization for fair surface design

**Citation.** Moreton, H. P., & Sequin, C. H. (1992). Functional optimization for fair surface design. *Computer Graphics (Proceedings of ACM SIGGRAPH 92)*, 26(2), 167-176. (See also Moreton, H. P. (1993). *Minimum Curvature Variation Curves, Networks, and Surfaces for Fair Free-Form Shape Design*, PhD thesis, UC Berkeley, Tech. Report UCB/CSD-93-219.)

**Formulation.** Introduces the minimum-variation family: MVC (curves), MVN (networks), MVS (surfaces). The surface minimizes E_MVS (section 2.6), the integral of squared derivatives of principal curvatures in their principal directions, subject to interpolating position and optionally normal and curvature constraints. Because the functional is geometric and high order, the optimization is nonlinear and solved by general nonlinear optimization over the surface degrees of freedom (a fine bicubic mesh), with the constraints (G2 position/normal/curvature interpolation) imposed during the solve.

**Limitations.** Very large computation times (a sixth-order geometric penalty); not interactive; needs careful initialization. The cost is the direct motivation for Greiner's quadratic surrogates.

**Kernel relevance.** Defines the quality ceiling. The "uniform curvature, highlight-clean" surfaces designers call Class-A correspond to minimizing curvature variation, not just curvature. Keel can offer an MVS-style high-quality mode (nonlinear, slow) for finishing, while defaulting to quadratic surrogates for interactive work, and can borrow MVS's constraint set (position + normal + curvature) directly for G2 boundary matching.

### 3.5 Greiner (1994) - Variational design and fairing of spline surfaces

**Citation.** Greiner, G. (1994). Variational design and fairing of spline surfaces. *Computer Graphics Forum (Proceedings of Eurographics 94)*, 13(3), 143-154.

**Formulation.** The pragmatic bridge. Greiner observes that the "right" fairness functional matters but that the expensive geometric ones (MVS, total curvature) can be replaced by *simplified quadratic functionals* that approximate uniform curvature and minimize at interactive speed. These surrogates are simplified thin-plate energies for parametric surfaces (or simplified MVC functionals). For a B-spline surface S = sum_ij P_ij N_i(u) N_j(v), the quadratic energy becomes a quadratic form in the stacked control points P:

```
  E(P) = P^T K P,   K_ab = integral ( derivative products of basis functions ) du dv
```

with K the fairness (stiffness) matrix, sparse and banded because of B-spline local support. Pure fairing minimizes P^T K P subject to the data/boundary constraints; design adds interpolation constraints. The minimizer solves a sparse symmetric linear system. The same machinery does both *design* (fair surface from constraints) and *fairing* (smooth an existing surface).

**Limitations.** The surrogate is parameterization-dependent and only approximates true geometric fairness; quality depends on a reasonable parameterization; over-fairing flattens features.

**Kernel relevance.** This is the functional Keel should ship as the default variational energy: a quadratic P^T K P that gives one linear solve, fair surfaces, and interactive speed, with MVS/geometric energy reserved for the high-quality mode. Greiner is also the explicit statement that design and fairing are *the same solve* with different constraint sets, which is the consolidation point in section 5.

### 3.6 Kallay (1993) - Constrained optimization in surface design

**Citation.** Kallay, M. (1993). Constrained optimization in surface design. In B. Falcidieno & T. L. Kunii (Eds.), *Modeling in Computer Graphics* (pp. 85-93). Springer. (Related: Kallay, M., & Ravani, B. (1990). Optimal twist vectors as a tool for interpolating a network of curves with a minimum-energy surface. *CAGD*, 7, 465-473.)

**Formulation.** Kallay formalizes curve/surface editing as constrained minimization of a "pseudo-energy" change: among all control-point configurations meeting the user's edit constraints, pick the one that minimizes a quadratic energy (a measure of bending or of *change* from the original). For curves the energy is integral kappa^2 ds (approximated quadratically); for surfaces, twist-vector and boundary energies are minimized to fill a curve network with a minimum-energy surface. The result is a small constrained quadratic program solved in closed form (KKT / Lagrange-multiplier linear system). The Kallay-Ravani twist-vector work specifically chooses the interior twists of a bicubic patch network to minimize energy, a tiny but high-leverage variational sub-solve.

**Limitations.** Localized / small-DOF (editing and twist selection), not a full global freeform solver; quadratic energy only.

**Kernel relevance.** Two uses. First, the "minimize change subject to edit constraints" formulation is the right model for *direct editing* of an existing NURBS face (drag a point, keep the rest fair), tying into kernel/27 free-form deformation and kernel/03 local operations. Second, energy-minimizing twist selection is a cheap, robust default for any patch-network fill, useful even in the transfinite path (kernel/26).

### 3.7 Hagen & Schulze / Hoschek-Lasser / Sapidis - fairing surveys and functionals

**Citation.** Hagen, H., & Schulze, G. (1987). Automatic smoothing with geometric surface patches. *CAGD*, 4, 231-236. Hoschek, J., & Lasser, D. (1993). *Fundamentals of Computer Aided Geometric Design* (A. K. Peters), fairing chapters. Sapidis, N. (Ed.) (1994). *Designing Fair Curves and Surfaces*. SIAM. (See nurbs/03 section 7 for the fairing-side treatment.)

**Formulation.** These survey and standardize the fairness functionals: membrane (tension of a thin membrane), thin-plate (bending of a thin plate), thin-plate-in-tension (the blended alpha/beta energy), and the geometric curvature energies, with the explicit statement that variational principles drive fairing and that the choice of functional is the key design decision. They also document the geometric-vs-parametric distinction and linearization assumptions (parametric energies are valid near arc-length parameterization).

**Limitations.** Surveys; they catalog rather than provide one canonical solver.

**Kernel relevance.** These are the reference texts for *which* functional Keel implements and the standard names for the tension/stiffness weights. They confirm that one penalized energy with adjustable alpha/beta covers membrane-through-plate behavior, supporting a single configurable solver.

---

## 4. Discretization to a control-point quadratic (the FEM view)

The mechanism that turns the continuous variational problem into a kernel-implementable solve, common to Celniker-Gossard, Welch-Witkin, Greiner, and the XNURBS-style engine.

**Step 1, representation.** Write the surface in the chosen basis:

```
  S(u,v) = sum_{i,j} P_ij B_i(u) B_j(v)
```

with B the B-spline basis and P_ij the control points (the unknowns). Stack all control-point coordinates into a vector P.

**Step 2, substitute into the energy.** Every term in a quadratic fairness functional (section 2.2-2.4) is an integral of products of derivatives of S, hence of products of derivatives of basis functions. Linearity in P makes the energy a quadratic form:

```
  E(P) = P^T K P,
  K_{(ij),(kl)} = integral_D [ alpha (B_i' B_j B_k' B_l + B_i B_j' B_k B_l')
                            + beta (B_i'' B_j B_k'' B_l + 2 B_i' B_j' B_k' B_l'
                                    + B_i B_j'' B_k B_l'') ] du dv.
```

K is the fairness / stiffness matrix. It is symmetric, positive semidefinite, and *sparse and banded* because B-spline basis functions have local support: K_ab is nonzero only when control points a and b share supported spans. K is assembled exactly via Gauss quadrature per knot span (the FE assembly loop). This is the direct NURBS analog of Celniker-Gossard's FE stiffness matrix.

**Step 3, constraints.** Linear constraints (interpolation S(u_k,v_k) = q_k, boundary control points fixed, point/curve passes-through, and the linearized parts of continuity) are written A P = b. Soft/least-squares constraints (data fitting, near-passes, gapped tolerances) become an added quadratic data term ||N P - Q||^2 with N the basis-evaluation (collocation) matrix.

**Step 4, the constrained quadratic program.** Minimizing E(P) = P^T K P subject to A P = b is a quadratic program with the KKT linear system

```
  [ 2K   A^T ] [ P      ]   [ 0 ]
  [ A    0   ] [ lambda ] = [ b ]
```

(lambda are Lagrange multipliers). If constraints are folded in as penalties or least-squares, it collapses to a single sparse SPD solve (section 5). For pure (parameterization-dependent) energies this is *one* sparse linear solve; for true geometric energy (section 2.5-2.6) K depends on the current surface, so it becomes an outer Gauss-Newton / Newton iteration, each step a solve of this form.

This is the whole engine: assemble K (fairness), assemble N / A (constraints), solve. Degree, knot count, and adaptive subdivision (Welch-Witkin) control the DOF count and hence the achievable precision.

---

## 5. The unified fairing/fitting engine: (N^T N + lambda K)

This is the architectural consolidation point and the explicit tie to nurbs/03 section 4.4 and 7.1.

The plain least-squares NURBS fit (nurbs/03 section 4.2) solves the normal equations

```
  (N^T N) P = N^T Q,
```

with N the basis-evaluation matrix at the data sites and Q the data. The P-spline / penalized smoother (Eilers-Marx, nurbs/03 section 4.4) adds a smoothness penalty:

```
  (N^T N + lambda D^T D) P = N^T Q,
```

where D^T D is a discrete difference (roughness) operator. The energy-fairing solve (Greiner, nurbs/03 section 7.1) replaces D^T D by the continuous fairness matrix K of section 4:

```
  (N^T N + lambda K) P = N^T Q.
```

The pure variational surface solve (Welch-Witkin, XNURBS core) is the limit where there is no dense data term, only constraints plus the fairness energy: minimize P^T K P subject to A P = b. All three are the same machinery:

- **data weight high, fairness weight low** -> interpolation / tight fitting (honor the data, the nurbs/03 fitter);
- **data weight and fairness weight balanced** -> smoothing / approximation of noisy data (the P-spline smoother);
- **no data term, fairness only, constraints hard** -> pure variational surfacing (the XNURBS-class fair-fill solve).

So *data-fitting, fairing, and variational surface design are one solver with different weights*. Keel should implement a single penalized / constrained least-squares engine,

```
  ( N^T W N + lambda K ) P = N^T W Q,   subject to A P = b,
```

with W a diagonal constraint-weight matrix, K the assembled fairness stiffness, and A the hard-constraint matrix, and let the surfacing, smoothing, and fitting subsystems all call it. This is the same conclusion nurbs/03 reached from the fitting side ("one penalized-least-squares engine"), now reached from the variational side, which is strong confirmation that it is the correct architectural primitive.

---

## 6. Modern real-time variational editing and subdivision (per-source)

### 6.1 Halstead, Kass & DeRose (1993) - Efficient, fair interpolation using Catmull-Clark surfaces

**Citation.** Halstead, M., Kass, M., & DeRose, T. (1993). Efficient, fair interpolation using Catmull-Clark surfaces. *Computer Graphics (Proceedings of ACM SIGGRAPH 93)*, 27, 35-44.

**Formulation.** Builds a control mesh whose Catmull-Clark subdivision-surface limit interpolates given vertices (and optionally normals) of an arbitrary-topology mesh while minimizing a fairness norm that is a *linear combination of thin-plate and membrane energies* (the alpha/beta energy of section 2.3, evaluated on the subdivision limit surface). Despite Catmull-Clark surfaces having no closed-form parameterization, the energy and its derivatives are computed exactly using eigen-analysis of the subdivision matrix, so minimization is again a sparse linear solve for the control mesh.

**Limitations.** Subdivision-surface output, not NURBS; thin-plate/membrane (quadratic) energy only.

**Kernel relevance.** Demonstrates the variational-fairness recipe on a non-NURBS representation and proves the energies can be assembled even without a closed-form basis, relevant if Keel ever offers subdivision surfaces (kernel/09 hybrid) alongside NURBS. The fairness functional and solve structure carry over unchanged.

### 6.2 Kobbelt et al. - Discrete fairing, variational subdivision, geometric fairing of meshes

**Citation.** Kobbelt, L. (2000). Discrete fairing and variational subdivision for freeform surface design. *The Visual Computer*, 16, 142-158. Kobbelt, L., Campagna, S., Vorsatz, J., & Seidel, H.-P. (1998). Interactive multi-resolution modeling on arbitrary meshes. *SIGGRAPH 98*. Schneider, R., & Kobbelt, L. (2001). Geometric fairing of irregular meshes for free-form surface design. *CAGD*, 18(4), 359-379.

**Formulation.** Discretizes the fairness energy or its Euler-Lagrange equation by replacing differential operators with divided-difference (umbrella / Laplacian) operators on the mesh. The fourth-order thin-plate problem Delta^2 x = 0 is factored into two second-order (Laplacian) problems, each a sparse linear solve; alternating refinement and smoothing gives variational subdivision that emits fair high-resolution meshes at thousands of triangles per second. Schneider-Kobbelt add a *geometric* (curvature-based, nonlinear) fairing for true G1/curvature continuity. The umbrella operator must be tangent-aware to avoid in-surface vertex drift on irregular meshes.

**Limitations.** Mesh / discrete setting, not parametric NURBS; the geometric variant is nonlinear and iterative; discrete-Laplacian quality depends on mesh regularity.

**Kernel relevance.** The factor-the-biharmonic-into-two-Laplacians trick and the multigrid/hierarchical solve strategy directly inform how Keel solves large variational systems efficiently. Also the reference for any mesh-side fairing (tessellation cleanup, kernel/05; healed-import smoothing, kernel/13).

### 6.3 Botsch & Kobbelt (2004) - An intuitive framework for real-time freeform modeling

**Citation.** Botsch, M., & Kobbelt, L. (2004). An intuitive framework for real-time freeform modeling. *ACM Transactions on Graphics (Proceedings of SIGGRAPH 2004)*, 23(3), 630-634.

**Formulation.** Constraint-based shape optimization on triangle meshes. The deformed region minimizes an order-k thin-plate-type energy whose Euler-Lagrange equation is Delta^k x = 0 (k chosen to give C0, C1, or C2 boundary behavior, with smoothness continuously tunable between them). The key real-time idea: for a *fixed* support region and boundary topology, the constrained minimization is linear, so the system matrix is fixed and only the right-hand side (driven by the user's handle motion) changes. They precompute a sparse Cholesky factorization (or precompute basis functions / use multigrid), so each frame of dragging is a fast back-substitution rather than a fresh solve, achieving real-time interactive variational editing.

**Limitations.** Mesh-based; linear (quadratic-energy) deformation; re-factorization needed if the support region or constraints change topology.

**Kernel relevance.** This is the recipe for *interactive* variational editing: hold the fairness matrix fixed, prefactor it, and update only the constraint RHS as the user drags. It is exactly how Keel can make the variational surfacer feel real-time (the XNURBS "interactive slider / drag" behavior, kernel/31 section 5), and it pairs with kernel/27 free-form deformation. The C0/C1/C2-by-order trick maps onto Keel's per-edge continuity selection.

### 6.4 Duchon (1977) - Thin-plate splines / minimizing rotation-invariant seminorms

**Citation.** Duchon, J. (1977). Splines minimizing rotation-invariant semi-norms in Sobolev spaces. In *Constructive Theory of Functions of Several Variables* (Lecture Notes in Mathematics 571, pp. 85-100). Springer.

**Formulation.** The closed-form variational solution for *scattered-data* fairness. Among all functions f interpolating data at scattered sites, the one minimizing the thin-plate seminorm

```
  J(f) = integral_R2 ( f_xx^2 + 2 f_xy^2 + f_yy^2 ) dx dy
```

is the thin-plate spline: a sum of the radial basis function phi(r) = r^2 log r centered at the data sites plus a low-degree polynomial,

```
  f(x) = sum_k w_k phi(|x - x_k|) + a_0 + a_1 x + a_2 y,
```

with weights from a (dense) linear system enforcing interpolation and the side conditions sum w_k = 0, sum w_k x_k = 0. Generalizing the seminorm order gives polyharmonic / RBF splines (Delta^m f = 0). This is the radial-basis-function (RBF) view of the *same* thin-plate energy: instead of a B-spline control grid, the variational minimizer is expressed in a basis of fundamental solutions centered at the data.

**Limitations.** Functional (height-field) form, not a parametric surface; the system is dense (global support of r^2 log r), so it scales poorly without fast-multipole / compactly-supported-RBF acceleration.

**Kernel relevance.** Two roles. First, it is the principled engine for *scattered-point surface fitting and hole filling* where no clean parameter grid exists (the height-field sub-problem in nurbs/03 section 6, defeaturing residuals, scan displacement fields). Second, it is the same thin-plate energy as the parametric solver, just in an RBF basis, so it confirms the energy choice and provides an alternative discretization when the data is scattered rather than gridded. The RBF (Duchon) form and the B-spline (Welch-Witkin) form are two discretizations of one variational problem.

---

## 7. Quality and behavior

**Why energy minimization yields fair interiors.** Minimizing thin-plate / curvature energy directly penalizes the wiggles and curvature spikes that designers see as "unfair" in a curvature comb or zebra plot. Because the solve is global, the fairness is spread across the whole surface rather than patched locally, which is what gives the Class-A-like, highlight-clean interiors the method is prized for. With a geometric or MVS energy (sections 2.5-2.6) the curvature is driven toward uniform, which is the literal Class-A criterion.

**The documented failure mode (wavy / heavy interior).** This is exactly what XNURBS reviewers report (kernel/31 section 6 and Caveats) and it follows directly from the formulation. When boundary continuity (G1/G2) is imposed as *hard* constraints and interior fairness is a *traded-off* energy term, the solver will satisfy the boundary tightly and leave the interior as wavy as the constraint set and weights allow. If lambda (the fairness weight) is too low relative to the constraints, or the constraints are dense and inconsistent (many control points inherited from dense input edges), the interior carries that density and is not independently guaranteed to be fair. The cure is to raise the fairness weight, reduce DOF (knot/control-point reduction, nurbs/03 section 7.2), or escalate to a geometric energy, all of which trade precision/closeness for interior fairness. Keel should expose this trade explicitly (a fairness-vs-fidelity weight, the alpha/beta tension/stiffness sliders) rather than hiding it.

**Control-point / knot count selection.** Precision to the constraints is bounded by DOF: too few control points cannot reach G0/G1/G2 tolerances; too many let the interior wander and bloat the output. Welch-Witkin add DOF adaptively (nonuniform subdivision) where constraints are tight; XNURBS picks the control-point/knot count to meet the tolerance and (later versions) minimizes it afterward (kernel/31 section 6, 8). The principle: use the *fewest* DOF that meet the precision tolerance, then knot-remove (nurbs/03 section 7.2) to compact. This is the DOF-vs-smoothness tradeoff: more DOF = closer fit but less inherent smoothness, fewer DOF = smoother but possibly cannot satisfy tight constraints.

**Over-/under-determined cases.** If constraints over-determine the surface (more independent constraints than DOF, or contradictory constraints), there is no exact feasible surface; the solve must fall back to least-squares (soft constraints) and report the residual / conflict (XNURBS's V7.0 conflicting-constraint display, kernel/31 section 3). If constraints under-determine it (too few to pin the surface), the fairness energy supplies the missing degrees of freedom and uniquely determines the smoothest completion, provided the energy is positive-definite on the unconstrained subspace (the membrane term is what guarantees this when the plate term alone has a null space of affine functions). Keel's solver must detect both regimes: rank-deficient constraints -> least-squares + conflict report; rank-deficient energy -> add the membrane regularizer.

---

## 8. Numerical considerations

**Conditioning.** Forming the normal equations N^T N squares the condition number (nurbs/03 section 4.2); adding lambda K both regularizes (improves conditioning, the penalty lifts small eigenvalues) and, if lambda is large, can dominate. The thin-plate stiffness K itself is ill-conditioned for fine grids (the biharmonic operator has condition number growing like h^-4 in mesh size h), so large dense variational systems need care. Mitigations: solve the rectangular least-squares system by QR rather than forming N^T N when conditioning is poor; scale/normalize the parameterization; use the membrane term to remove the affine null space of the plate energy.

**System size and sparsity.** With a B-spline basis, K and N^T N are sparse and banded (bandwidth set by degree and grid size) thanks to local support. A surface with thousands of control points gives a sparse SPD system of a few thousand unknowns, well within direct sparse solver range.

**Solver choice.**
- *Direct sparse* (sparse Cholesky / LDL^T on the SPD or KKT system): the default for one-shot solves and the basis of Botsch-Kobbelt's prefactor-once real-time editing. Robust and exact; the factorization is reusable across RHS changes (interactive dragging) as long as the matrix is fixed.
- *Iterative* (preconditioned conjugate gradient, or geometric multigrid): better for very large systems and for the hierarchical / variational-subdivision approach (Kobbelt). Multigrid is the route to real-time on large meshes.
- *KKT / Lagrange-multiplier* solve for hard constraints (the saddle-point system of section 4) vs *penalty / weighted-least-squares* folding constraints into the SPD system; the latter is simpler and keeps the matrix SPD (and prefactorable) at the cost of only approximately satisfying constraints.

**Nonlinearity of true geometric energy.** When the fairness energy is geometric (total curvature, Willmore, MVS, sections 2.5-2.6), K depends on the current surface (the fundamental forms), so the problem is nonlinear. It is solved by an outer iteration: Gauss-Newton or Newton on the energy, or a curvature-flow (gradient-descent) time stepping (as in Willmore flow / Kobbelt geometric fairing), each step being a linear solve of the section-4 form with K re-assembled at the current iterate. Convergence needs a good initial guess (the linear thin-plate solution is the natural warm start) and damping/line-search. This is the cost of true Class-A fairness and why kernels default to the linear quadratic surrogate and only escalate when asked.

**Robustness.** The solve must handle: rank-deficient constraints (switch to least-squares, report conflicts), a rank-deficient plate energy (add membrane regularization), degenerate parameterization (reparameterize / add tension), and ill-conditioning (QR or regularization). A production variational surfacer is as much about these fallbacks as about the core solve, which is precisely why XNURBS markets robustness and why reviewers find its predictability imperfect.

---

## 9. Variational surfacing for Keel (synthesis)

**The one solver.** Keel should implement a single energy-plus-constraint variational surfacer with this shape: represent the surface as a NURBS patch; assemble a fairness stiffness matrix K from a configurable quadratic energy (membrane alpha-term plus thin-plate beta-term, the Greiner surrogate as default); express interpolation, boundary, point, curve, and (linearized) continuity conditions as hard constraints A P = b and/or soft least-squares terms; and solve the resulting sparse constrained quadratic program

```
  minimize  P^T K P + || N P - Q ||_W^2   subject to  A P = b,
```

as a sparse SPD (or KKT) linear system, prefactored for interactive re-solve. Escalate to a nonlinear geometric energy (total curvature / MVS) via Gauss-Newton, warm-started from the linear solution, for a high-quality "Class-A" mode.

**Relationship to the nurbs/03 fairing engine.** This solver *is* the nurbs/03 penalized-least-squares engine (N^T N + lambda K) seen from the surfacing side (section 5). Keel builds it once and shares it across: NURBS fitting (data weight high), noisy-scan smoothing / P-splines (balanced), energy fairing of existing faces (Greiner), and pure variational surfacing (fairness-only with hard constraints). The fairness matrix K, the basis-evaluation matrix N, and the constraint matrix A are the shared primitives. This consolidation, reached independently from the fitting side (nurbs/03) and the variational side (this dossier), is a genuine architectural simplification and should be a load-bearing decision in Keel's design.

**Relationship to the kernel/26 transfinite path.** Variational and transfinite are complementary, not competing. Transfinite (Coons/Gregory/Gordon, kernel/26) is constructive: closed-form, exact, cheap, ideal for clean N-sided boundary fills where the loop is well-formed and speed matters. Variational is optimization-based: one global solve, more expensive, but it absorbs gapped/open/overlapping/messy input and gives directly controllable interior fairness and arbitrary continuity matching. Keel should offer transfinite as the fast path for clean input and variational as the robust path for the XNURBS-class messy cases, and can even seed the variational solve with a transfinite surface as the initial guess.

**When to use which.** Clean, well-formed N-sided loop, need it fast and exact -> transfinite (kernel/26). Messy, gapped, over/under-determined, mixed boundary+internal+point constraints, need G2 matching to neighbors and a fair interior -> variational (this dossier). Existing surface that is unfair -> the same K as a fairing pass (nurbs/03 section 7). Scattered points with no parameter grid -> the Duchon RBF / thin-plate discretization of the same energy.

**The XNURBS-class capability this unlocks.** Implementing the variational surfacer gives Keel the distinctive XNURBS core (kernel/31): a single global energy-minimization solve that produces one watertight, fair NURBS surface from an arbitrary mix of boundary curves, edges, internal curves, and points, with per-edge G0/G1/G2 continuity to neighbors, tolerant of gapped and inconsistent input. The known failure mode (wavy/heavy interior when fairness is under-weighted relative to dense constraints) is understood and controllable through the fairness weight, DOF reduction, and the geometric-energy upgrade. With the continuity machinery from the companion dossier (kernel/33), this is the complete XNURBS-class surfacing capability, built on the same penalized-least-squares engine Keel already needs for fitting and fairing.

---

## References

- Botsch, M., & Kobbelt, L. (2004). An intuitive framework for real-time freeform modeling. *ACM Transactions on Graphics (SIGGRAPH 2004)*, 23(3), 630-634. https://www.cs.toronto.edu/~jacobson/seminar/botsch-and-kobbelt-2004.pdf
- Celniker, G., & Gossard, D. (1991). Deformable curve and surface finite-elements for free-form shape design. *Computer Graphics (SIGGRAPH 91)*, 25(4), 257-266. https://dl.acm.org/doi/10.1145/127719.122746
- Duchon, J. (1977). Splines minimizing rotation-invariant semi-norms in Sobolev spaces. *Constructive Theory of Functions of Several Variables* (LNM 571, pp. 85-100). Springer. https://www.sciepub.com/reference/53293
- Eilers, P. H. C., & Marx, B. D. (1996). Flexible smoothing with B-splines and penalties. *Statistical Science*, 11(2), 89-121. (see nurbs/03 section 4.4)
- Greiner, G. (1994). Variational design and fairing of spline surfaces. *Computer Graphics Forum (Eurographics 94)*, 13(3), 143-154. https://onlinelibrary.wiley.com/doi/10.1111/1467-8659.1330143
- Hagen, H., & Schulze, G. (1987). Automatic smoothing with geometric surface patches. *CAGD*, 4, 231-236.
- Halstead, M., Kass, M., & DeRose, T. (1993). Efficient, fair interpolation using Catmull-Clark surfaces. *Computer Graphics (SIGGRAPH 93)*, 27, 35-44. https://dl.acm.org/doi/pdf/10.1145/166117.166121
- Hoschek, J., & Lasser, D. (1993). *Fundamentals of Computer Aided Geometric Design*. A. K. Peters. (fairing chapters)
- Kallay, M. (1993). Constrained optimization in surface design. In *Modeling in Computer Graphics* (pp. 85-93). Springer. https://link.springer.com/chapter/10.1007/978-3-642-78114-8_5
- Kallay, M., & Ravani, B. (1990). Optimal twist vectors as a tool for interpolating a network of curves with a minimum-energy surface. *CAGD*, 7, 465-473.
- Kobbelt, L. (2000). Discrete fairing and variational subdivision for freeform surface design. *The Visual Computer*, 16, 142-158. https://link.springer.com/article/10.1007/s003710050204
- Kobbelt, L., Campagna, S., Vorsatz, J., & Seidel, H.-P. (1998). Interactive multi-resolution modeling on arbitrary meshes. *SIGGRAPH 98*. http://mesh.brown.edu/en193s08-2003/refs/Kobbelt-etal-sg98.pdf
- Moreton, H. P., & Sequin, C. H. (1992). Functional optimization for fair surface design. *Computer Graphics (SIGGRAPH 92)*, 26(2), 167-176. https://www.researchgate.net/publication/220720399_Functional_optimization_for_fair_surface_design
- Moreton, H. P. (1993). *Minimum Curvature Variation Curves, Networks, and Surfaces for Fair Free-Form Shape Design*. PhD thesis, UC Berkeley, UCB/CSD-93-219. https://www2.eecs.berkeley.edu/Pubs/TechRpts/1993/5219.html
- Sapidis, N. (Ed.) (1994). *Designing Fair Curves and Surfaces*. SIAM. https://epubs.siam.org/doi/abs/10.1137/1.9781611971521.ch6
- Schneider, R., & Kobbelt, L. (2001). Geometric fairing of irregular meshes for free-form surface design. *CAGD*, 18(4), 359-379. https://www.graphics.rwth-aachen.de/media/papers/geom_fair1.pdf
- Terzopoulos, D., Platt, J., Barr, A., & Fleischer, K. (1987). Elastically deformable models. *Computer Graphics (SIGGRAPH 87)*, 21(4), 205-214. https://dl.acm.org/doi/10.1145/37402.37427
- Welch, W., & Witkin, A. (1992). Variational surface modeling. *Computer Graphics (SIGGRAPH 92)*, 26(2), 157-166. https://www.ri.cmu.edu/publications/variational-surface-modeling/
- Willmore energy / Willmore flow (surface fairing background). G2 surface modeling using minimal mean-curvature-variation flow, *CAD* (2007). https://www.sciencedirect.com/science/article/abs/pii/S0010448507000449

**Cross-references within this repository.**
- kernel/31-xnurbs-capability-map.md (the capability this dossier explains)
- kernel/33 (companion: constraint and continuity machinery)
- kernel/26-transfinite-nsided-surfacing.md (the complementary constructive path)
- kernel/27-free-form-deformation.md (interactive editing connection)
- nurbs/03-fitting-interpolation.md sections 4.4, 7 (the shared penalized-least-squares engine)
