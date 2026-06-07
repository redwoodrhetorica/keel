# Metrology-Grade Geometric Fitting Algorithms

Research file 23 for the Keel kernel. Deep treatment of computational-metrology fitting: the objective-function taxonomy (least squares, minimum zone, maximum-inscribed, minimum-circumscribed), least-squares fitting of analytic primitives, exact computational-geometry minimum-zone algorithms, smallest-enclosing and largest-inscribed solvers, robust fitting, reference software and validation, uncertainty propagation, primitive detection from dense data, constrained simultaneous fitting, and freeform deviation evaluation. The companion GD&T file established that datum simulation requires Chebyshev / min-zone / max-inscribed / min-circumscribed fits and named Shakarji's NIST work as the anchor. This file is the engineering-depth treatment, serving four Keel consumers at once: datum simulation, healing/simplify, reverse engineering, and canonical geometry recovery.

## Scope and why this matters for a B-rep kernel

A solid modeling kernel rarely fits geometry to noisy point data in its core Boolean/Euler machinery. But Keel's ambition (import, heal, reverse-engineer, recover canonical analytics, support inspection workflows) pulls fitting into the kernel as a first-class subsystem. Four distinct demands appear:

1. Datum simulation and GD&T evaluation need fits with a *specified objective* dictated by standards. Least squares is wrong for many callouts: form tolerances need minimum zone (L-infinity), datum features of size need tangent / outside-material fits (minimum-circumscribed for external, maximum-inscribed for internal).
2. Import healing and simplify need to recognize when a sloppy spline or faceted surface is "really" a plane, cylinder, cone, sphere, or torus, and to snap it.
3. Reverse engineering of scanned parts needs robust fits that reject outliers and segment a dense cloud into primitive patches.
4. Canonical recovery needs simultaneous constrained fitting so that recovered primitives respect perpendicularity, coaxiality, and concentricity exactly.

The objective taxonomy is the organizing idea. The same primitive (a cylinder, say) supports at least five different "best" answers depending on the norm and one-sidedness, and metrology software disagreement is overwhelmingly traceable to picking the wrong one.

---

## Theme 1: The fitting-objective taxonomy

### Least squares (L2 / Gaussian)
Minimizes the sum of squared orthogonal distances. Smooth, differentiable, unique for well-posed data, statistically optimal under Gaussian noise. It is the ISO default association criterion for many features and the universal initial guess for every other objective. But it is a *two-sided* fit balanced through the material, so it systematically misrepresents form tolerances and never touches material-side fits.

### Minimum zone / Chebyshev (L-infinity)
Minimizes the maximum absolute deviation, equivalently the peak-to-valley separation of two parallel/concentric/coaxial nominal features that bracket all points. This is the standardized criterion for form tolerances (straightness, flatness, roundness, cylindricity, profile). It is a non-smooth min-max problem; the solution is characterized by a small set of *contacting points* in an alternation/exchange configuration.

### Maximum-inscribed (MIC) and minimum-circumscribed (MCC)
One-sided fits that keep all points on one side. Minimum-circumscribed (smallest enclosing cylinder/circle/sphere) simulates an external mating feature (a ring gauge, a collet); maximum-inscribed (largest contained) simulates an internal mating feature (a pin gauge, a shaft in a hole). These are the "outside-material" / "inside-material" fits that ASME and ISO datum simulation invoke for datum features of size.

### Which standard wants which objective
- ISO 14405-1 size: default two-point (local) size; with the envelope modifier (E) the size is governed by an MCC/MIC envelope. ISO 1101 form tolerances are evaluated against the minimum zone by definition of "tolerance zone."
- ISO 5459 / ASME Y14.5 datum simulation: a datum simulator is the *mating* feature, so external datum features call for minimum-circumscribed, internal for maximum-inscribed, with the constraint that the simulator may not enter the material. ASME Y14.5.1-2019 gives the mathematical definitions; the practical default that standards committees adopted for datum establishment is Shakarji and Srinivasan's *constrained* least squares (L2 best fit subject to the no-penetration / outside-material constraint), which is more stable than pure MCC/MIC while honoring the material side.
- ASME vs ISO difference: ASME historically leans on the constrained-L2 datum and an explicit mathematical-definitions companion standard (Y14.5.1); ISO GPS leans on the GPS matrix with ISO 14405 (size), ISO 1101 (geometric tolerances), ISO 5459 (datums), and ISO 12780/12781/12180/12181 (the straightness/flatness/cylindricity/roundness families) each naming default and optional association criteria.

The practical kernel consequence: a fitting API must accept the objective as an explicit parameter, never hardcode L2.

Sources: see Theme entries below for ISO 14405 / 1101 / ASME discussion, and the constrained-LS NIST entry.

---

## Theme 2: Least-squares fitting of analytic primitives

### Shakarji (1998), NIST reference least-squares algorithms
**Citation.** Shakarji, C. M. (1998). Least-squares fitting algorithms of the NIST Algorithm Testing System. *Journal of Research of the National Institute of Standards and Technology, 103*(6), 633-641. https://doi.org/10.6028/jres.103.043

**Method.** The single most load-bearing reference for primitive least squares in metrology. All fits are *orthogonal distance regression*: minimize J = sum of d_i^2, where d_i is the true geometric (perpendicular) distance from point i to the surface. Linear geometries (plane, line) are solved as constrained eigenproblems: a plane is point x plus unit normal a with d = a . (x_i - x); the optimal normal is the eigenvector of the centered data covariance with the smallest eigenvalue (line: largest eigenvalue gives direction), solved by SVD to avoid forming M^T M. Nonlinear geometries (sphere, circle-3D, cylinder, cone, torus) use Levenberg-Marquardt: the update solves (F0^T F0 + lambda D^T D) x = -F0^T d(p0), with lambda interpolating between Gauss-Newton and gradient descent. Explicit distance functions and first-derivative Jacobians are given for every primitive. Cylinder: axis point x, unit direction A, radius r, with d = f_i - r where f_i is the distance from the point to the axis line. Cone: x, A, half-angle psi, offset s, with d = f_i cos(psi) + g_i sin(psi) - s, where g_i is signed distance along the axis. Torus: d = sqrt(g_i^2 + (f_i - r)^2) - R (major radius r, tube radius R). Direction vectors are re-normalized after every iteration so the optimization stays unconstrained while a stays a unit vector. Initial guesses: SVD eigenvectors for line/plane; for cylinder/cone a constrained pre-solve (e.g. fit the axis direction first) followed by a *full 3D minimization restart* over all parameters to escape local minima; 3D circle projects to the LS plane, fits a 2D circle, then refines.

**Limitations.** Convergence failure observed only for pathological inputs (circle through nearly collinear points). The paper explicitly notes that some commercial 3D-circle implementations skip the final full optimization and therefore do not return the true least-squares circle, an early documentation of inter-software disagreement.

**Kernel relevance.** This is the spec to implement verbatim for Keel's primitive LS fitter. The parameterizations and Jacobians transfer directly to Rust; the restart strategy for cylinder/cone is essential for reliability. These fits are also the mandatory warm start for every min-zone / MIC / MCC solve.

### Eberly, Geometric Tools least-squares fitting documents
**Citation.** Eberly, D. (2020, and earlier revisions). *Least squares fitting of data by linear or quadratic structures.* Geometric Tools, LLC. https://www.geometrictools.com/Documentation/LeastSquaresFitting.pdf (plus companion documents for cylinders and cones).

**Method.** A freely available, implementation-oriented companion to Shakarji with full pseudocode. Lines/planes via PCA eigenvectors of the covariance matrix (smallest eigenvalue normal for plane, largest for line). A *non-iterative algebraic* circle/sphere/hypersphere fit using differences of squared lengths, giving bounded, fast computation suitable as a warm start. The cylinder treatment is notable: it reduces the six-parameter cylinder fit to a *one-dimensional search over axis direction only*; for each candidate direction, the optimal center and radius are solved in closed form by projecting points onto the plane perpendicular to the direction and doing a 2D circle fit, then the residual energy is minimized over the direction (a unit-sphere search). Cones are handled in staged fashion: estimate axis direction, then vertex and half-angle.

**Limitations.** The algebraic circle/sphere fits carry bias on short arcs (see Theme 2 circle entry); the cylinder direction-search needs a good seed or a grid over the hemisphere to avoid local minima. Documents are engineering notes, not peer-reviewed, but are widely validated in practice.

**Kernel relevance.** Eberly's direction-reduction for cylinders is an excellent robust seeding strategy that complements Shakarji's restart. The non-iterative sphere/circle fits give Keel cheap, bounded-time warm starts. Geometric Tools is permissively licensed reference code to crib parameterizations from (re-implement in Rust, do not copy).

### Algebraic circle fits: Kasa, Pratt, Taubin, and bias behavior
**Citation.** Al-Sharadqah, A., & Chernov, N. (2009). Error analysis for circle fitting algorithms. *Electronic Journal of Statistics, 3*, 886-911. (Pratt, V. (1987), *SIGGRAPH*; Taubin, G. (1991), *IEEE TPAMI*; Kasa, I. (1976), *IEEE Trans. Instrum. Meas.* are the originals.) https://arxiv.org/pdf/0907.0421

**Method.** Algebraic circle fits minimize an algebraic (not geometric) residual and give closed-form linear solutions. Kasa minimizes sum of (x_i^2 + y_i^2 + Dx_i + Ey_i + F)^2, the simplest but most biased. Pratt and Taubin add normalizations that make the estimator approximately invariant and far less biased on partial arcs (Pratt constrains B^2 - 4AC; Taubin uses a gradient-weighted normalization). Chernov's error analysis shows all fits share the same leading-order variance; differences live in the *essential bias* (the second leading term in the mean-square-error expansion). Ranking: Kasa poor (biased toward smaller circles on short arcs), Pratt moderate, Taubin good, geometric (orthogonal-distance) best. Hyperaccurate and HyperLS fits reduce essential bias to zero / order sigma^4.

**Limitations.** Algebraic fits never match the true geometric optimum; Kasa is unsafe for short arcs (common in CMM probing of partial features). All are sensitive to outliers (no robustness).

**Kernel relevance.** Use Taubin as the algebraic warm start for the geometric circle fit, never Kasa. The short-arc bias warning matters directly: Keel often fits circles to small captured arcs (fillet cross-sections, hole edges).

### Lukacs, Marshall, Martin: faithful least-squares fitting
**Citation.** Lukacs, G., Marshall, A. D., & Martin, R. R. (1998). Faithful least-squares fitting of spheres, cylinders, cones and tori for reliable segmentation. In *Computer Vision - ECCV '98* (LNCS 1406, pp. 671-686). Springer. https://doi.org/10.1007/BFb0055697

**Method.** The landmark "faithful" fitting paper. Introduces parameterizations and distance approximations for sphere/cylinder/cone/torus that *degrade gracefully* as the surface approaches a degenerate (lower-curvature) type. As principal curvatures decrease or equalize, a cylinder fit naturally tends to a plane, a cone to a cylinder, a torus to a cylinder or sphere, rather than blowing up with huge radii and ill-conditioned Jacobians. They use a linear approximation of the orthogonal distance (a first-order signed-distance surrogate) that stays numerically stable across the degeneracy, paired with a segmentation framework that grows regions and tests competing primitive types.

**Limitations.** The linearized distance is an approximation, so a final exact orthogonal-distance refinement (Shakarji-style) is still advisable. Tuned for range-map reverse engineering, not metrology certification.

**Kernel relevance.** Critical for Keel's healing and canonical recovery: the graceful-degeneracy property is exactly what you want when deciding "is this barely-curved face a cylinder or a plane." It prevents the classic failure where a near-flat cylinder fit returns a radius of 10^9. Pair it with a type-selection test that prefers the simpler primitive when residuals are statistically indistinguishable.

### Lukacs, Martin, Marshall: robust segmentation under geometric degeneracy
**Citation.** Marshall, D., Lukacs, G., & Martin, R. (2001). Robust segmentation of primitives from range data in the presence of geometric degeneracy. *IEEE Transactions on Pattern Analysis and Machine Intelligence, 23*(3), 304-314. https://doi.org/10.1109/34.910883

**Method.** Extends the faithful-fitting idea into a full segmentation pipeline that simultaneously decides primitive type and membership while remaining robust where the geometry is degenerate (regions that could be several primitive types). Uses the curvature-aware faithful fits as the per-region scoring function so that type ambiguity does not destabilize the segmentation.

**Limitations.** Range-data focused; assumes reasonably dense, organized data. Computationally heavier than greedy region growing.

**Kernel relevance.** A reference design for Keel's reverse-engineering segmentation stage, which must hand clean per-primitive point partitions to the exact fitter.

---

## Theme 3: Minimum-zone (Chebyshev) fitting

### Roundness via minimum-width annulus (computational geometry, exact 2D)
**Citation.** Garcia-Lopez, J., Ramos, P. A., & Snoeyink, J. (1998). Fitting a set of points by a circle / On the width and roundness of a set of points in the plane. *Discrete & Computational Geometry / SoCG.* (See also experts.umn.edu listing.)

**Method.** The exact 2D roundness (minimum radial separation) problem equals finding the minimum-width annulus containing all points. Key theorem: either roundness equals the set width, or the annulus center is a vertex of the *nearest-point* Voronoi diagram, a vertex of the *farthest-point* Voronoi diagram, or an intersection of an edge from each. This gives an exact O(n^2) (and with care better) algorithm: overlay the two Voronoi diagrams and test the candidate centers. The earlier widely-cited characterization was incomplete; this work corrects it. Straightness in 2D similarly reduces to the convex hull (min-width slab via rotating calipers).

**Limitations.** Exact only in 2D. Sensitive to a single outlier (no robustness, as metrology requires). The overlay of two Voronoi diagrams is fiddly to implement robustly with floating point.

**Kernel relevance.** This is the certifiable, exact roundness/straightness evaluator for Keel's GD&T module. Worth implementing on top of a robust Voronoi/convex-hull predicate layer. The Voronoi-vertex characterization is also the certificate: you can verify a returned annulus is optimal by confirming its center is one of the enumerated candidates.

### Carr and Ferreira: minimum-zone via sequential linear programming (3D straightness, flatness, cylindricity)
**Citation.** Carr, K., & Ferreira, P. (1995). Verification of form tolerances part II: Cylindricity and straightness of a median line. *Precision Engineering, 17*(2), 144-156. (Part I covers straightness and flatness.)

**Method.** Formulates flatness, straightness, and cylindricity minimum-zone problems as nonlinear programs, then linearizes about the current estimate and solves a *sequence of linear programs* that converges to a local optimum. With an adequate initial guess (the least-squares fit), the local optimum is the global minimum zone. For a plane, the LP at each step finds the perturbation of plane parameters that minimizes the worst-case deviation subject to linearized constraints; for cylindricity, perturbations of axis position and direction. The contacting-point set (which points touch the two bounding planes/cylinders) emerges from the active constraints of the final LP.

**Limitations.** Local convergence: a bad seed can converge to a non-global zone. Cylindricity is non-convex; the LP relaxation is only guaranteed locally. Verification that the answer is truly global needs the contacting-configuration check.

**Kernel relevance.** The practical engine for 3D min-zone flatness and cylindricity in Keel. Sequential LP is easy to wire to any LP backend; warm-started from the LS fit it is reliable for well-sampled features. Pair it with a global verification pass.

### Exact min-zone characterization and exchange algorithms
**Citation.** Roy, U., & Zhang, X. (1992) and related; and the general Chebyshev-approximation framing via the Remez exchange algorithm. (See also Hermann, G. (2007), *Simple procedure for minimum zone evaluation*, SAMI; and the survey context in the Chebyshev metrology literature.)

**Method.** Minimum-zone form fitting is Chebyshev (L-infinity) approximation. The optimal zone is characterized by an alternation/contact condition: a minimum number of points must lie on the two bounding features in a specific interlocking configuration (e.g. roundness needs at least four contacting points alternating inner-outer-inner-outer; flatness needs a 2-1 or 1-2 or 3-1 contact pattern; the classic Tschebyscheff condition). Exchange algorithms (the metrology analogue of Remez) iteratively swap a candidate contact point for one that increases the zone, converging to the alternation configuration that certifies optimality. This contact characterization is what lets you *verify* a min-zone result independently of how it was computed.

**Limitations.** Combinatorial in the worst case; degenerate / cocircular configurations need careful tie-breaking. Implementations differ in how they enumerate contact patterns.

**Kernel relevance.** Provides the *verification certificate* Keel needs: after any min-zone solve (SLP, metaheuristic, SOCP), confirm the contacting-point alternation condition to certify the answer is a true minimum zone, not a local artifact. This is how Keel can claim metrology-grade correctness.

### Metaheuristic and convex-relaxation approaches
**Citation.** Representative: genetic-algorithm roundness MZT (Rossi et al., *Measurement*, 2011); particle-swarm and simulated-annealing min-zone solvers; support-vector / SOCP relaxations for straightness and flatness. (See the minimum-zone survey results in this file's references.)

**Method.** Metaheuristics (GA, PSO, SA) directly search the low-dimensional parameter space (e.g. axis direction for cylindricity) minimizing the peak-to-valley, avoiding gradient issues of the non-smooth objective. Convex-relaxation methods cast min-zone as a second-order cone program (SOCP) or LP and solve to global optimality for the convex sub-cases.

**Limitations.** Metaheuristics give no optimality certificate and are slow; relaxations are exact only when the relaxation is tight (true for straightness/flatness as LP, not for cylindricity directly).

**Kernel relevance.** Useful fallback when SLP fails to converge, but never the primary engine for a kernel that wants reproducibility. Keel should prefer deterministic SLP plus contact-condition verification, reserving metaheuristics for pathological seeds.

---

## Theme 4: Maximum-inscribed and minimum-circumscribed fitting

### Welzl: smallest enclosing circle / ball (minimum-circumscribed)
**Citation.** Welzl, E. (1991). Smallest enclosing disks (balls and ellipsoids). In *New Results and New Trends in Computer Science* (LNCS 555, pp. 359-370). Springer.

**Method.** A randomized incremental algorithm computing the exact smallest enclosing circle (2D) or ball (3D) in expected O(n) time. Adds points in random order; if a new point is inside the current ball, nothing changes; otherwise that point must lie on the boundary of the new optimal ball, so recurse with it pinned to the boundary set. The base cases use the fact that a minimal enclosing ball in d dimensions is determined by at most d+1 boundary points (2 or 3 in 2D, up to 4 in 3D). Move-to-front and pivoting variants (Gartner) improve practical robustness and speed.

**Limitations.** Solves the *sphere/circle* MCC, not the cylinder MCC (smallest enclosing cylinder is a separate, harder problem with axis-direction search). Floating-point degeneracy near cocircular/cospherical points needs careful predicates. Not robust to outliers (exactly as metrology wants for MCC).

**Kernel relevance.** The exact engine for minimum-circumscribed circle (roundness MCC, datum simulation of an external cylindrical datum's cross-section) and minimum-circumscribed sphere. Welzl/Gartner is the right algorithm; Keel should implement it on robust predicates. For the smallest enclosing *cylinder*, wrap an axis-direction search (hemisphere) around a per-direction 2D smallest-enclosing-circle of the projected points.

### Maximum-inscribed circle / cylinder and cylindricity MIC/MCC variants
**Citation.** Shakarji, C. M., & Srinivasan, V. (2007 onward, NIST). Reference algorithms for Chebyshev and one-sided data fitting for coordinate metrology. *CIRP Annals, 56*(1), 469-472. https://doi.org/10.1016/j.cirp.2007.05.112

**Method.** Provides reference algorithms for Chebyshev (min-zone), maximum-inscribed, and minimum-circumscribed fits for lines, planes, circles, spheres, cylinders, and cones. The strategy: start from the least-squares fit, then iterate toward the one-sided or min-max objective. Maximum-inscribed circle/cylinder is the dual of MCC: find the largest circle/cylinder containing no points (all points outside or on it), characterized by contacting points constraining its growth. The smallest-enclosing and largest-inscribed problems for circles/spheres are polynomial; cylinder variants require axis-direction optimization, making them harder. Complexity results: 2D MIC/MCC for circles are linear/near-linear via LP-type or Voronoi methods; cylinder MCC in 3D is the smallest-enclosing-cylinder problem, known to be solvable but expensive (related to LP-type problems of higher combinatorial dimension).

**Limitations.** Cylinder one-sided fits are the hardest and where commercial software diverges most. Iterating from LS can stall on flat regions of the objective.

**Kernel relevance.** This CIRP paper is the metrology spec for Keel's MIC/MCC/Chebyshev module across all standard primitives, complementing the 1998 LS paper. Together the two Shakarji papers define Keel's entire primitive fitting objective set. The companion observation, that reference results differ markedly from industrial software, motivates building and validating against NIST/PTB datasets.

### Constrained least squares for datum establishment
**Citation.** Shakarji, C. M., & Srinivasan, V. (2013/2016 NIST). On algorithms and heuristics for constrained least-squares fitting of circles and spheres to support standards. NIST publication. https://www.nist.gov/publications/algorithms-and-heuristics-constrained-least-squares-fitting-circles-and-spheres-support

**Method.** Defines constrained-L2 fitting: minimize sum of squared distances subject to the constraint that the fitted feature does not penetrate the material (all points on the outside for an external feature, inside for internal). This is the datum-establishment default adopted by standards committees: more stable than pure MCC/MIC (which can pivot wildly on a single high point) yet honoring the material side that pure unconstrained L2 ignores. Solved as a constrained nonlinear program (KKT conditions) with active-set handling of the touching points.

**Limitations.** The active set (which points are on the constraint) can be ambiguous; heuristics needed for robustness. Not yet universal across all primitives.

**Kernel relevance.** This is the *default datum simulator* Keel should implement for datum features of size, distinct from both the form-tolerance min-zone and the gauge-mating MCC/MIC. Encodes the ASME/ISO consensus.

---

## Theme 5: Robust fitting (outlier handling)

### Schnabel, Wahl, Klein: efficient RANSAC for point-cloud shape detection
**Citation.** Schnabel, R., Wahl, R., & Klein, R. (2007). Efficient RANSAC for point-cloud shape detection. *Computer Graphics Forum, 26*(2), 214-226. https://doi.org/10.1111/j.1467-8659.2007.01016.x

**Method.** The standard primitive-detection algorithm for dense scans. Detects planes, spheres, cylinders, cones, and tori. Key efficiency ideas: (1) minimal sampling, e.g. a plane needs 3 points with normals, a sphere/cylinder/cone needs as few as 1-2 oriented points using estimated surface normals, drastically cutting the sample size; (2) localized sampling, drawing candidate samples from spatial neighborhoods (an octree) since real primitives are connected, raising the hit rate enormously; (3) a score that counts inliers within distance and normal-deviation thresholds, plus a connected-component test so a candidate scores only on a contiguous patch; (4) a lazy, probabilistic score evaluation that estimates inlier counts on random subsets and refines only promising candidates. Robust to heavy noise and outliers; decomposes million-point clouds in under a minute into primitives plus a residual remainder.

**Limitations.** Threshold-sensitive (distance, normal-angle, min cluster size). Greedy: detection order affects results; thin or small features can be missed. Returns an inlier partition and a coarse fit; a final exact orthogonal-distance refit is needed for accuracy.

**Kernel relevance.** Keel's front-end for reverse engineering: turn a raw scan into per-primitive inlier sets, then hand each set to the Shakarji exact fitter, then to constrained simultaneous fitting for canonical cleanup. The minimal-sample-with-normals trick and octree localization are the pieces to port. Crucially this is the *reverse-engineering* path where outlier rejection is welcome, the opposite of metrology certification.

### MLESAC and the RANSAC family
**Citation.** Torr, P. H. S., & Zisserman, A. (2000). MLESAC: A new robust estimator with application to estimating image geometry. *Computer Vision and Image Understanding, 78*(1), 138-156. https://doi.org/10.1006/cviu.1999.0832

**Method.** Generalizes RANSAC's scoring. RANSAC counts inliers under a hard threshold; MLESAC instead maximizes the likelihood of the data under a mixture model (inliers Gaussian, outliers uniform), giving a continuous, better-discriminating cost. Same hypothesize-and-test sampling, smarter evaluation. Related descendants: MSAC (truncated quadratic cost), PROSAC (progressive sampling by quality), LO-RANSAC (local optimization).

**Limitations.** Needs an inlier-noise estimate; mixture parameters add tuning. Still random, so non-deterministic across runs unless seeded.

**Kernel relevance.** When Keel's RANSAC primitive detection sits near a borderline (noisy scan, close-competing primitive types), MLESAC's likelihood score reduces spurious detections. Seed the RNG for reproducibility in a kernel context.

### M-estimators, IRLS, and L1 fitting
**Citation.** Huber, P. J. (1964/1981). *Robust Statistics.* Wiley. (Plus standard treatments of Tukey biweight, Cauchy, Geman-McClure; IRLS convergence: Aftab & Hartley, 2015.)

**Method.** M-estimators replace the squared residual with a robust loss rho(r) that grows sub-quadratically (Huber: quadratic near zero, linear in the tails) or saturates (Tukey biweight, Cauchy, Geman-McClure suppress outliers entirely). Minimized by *iteratively reweighted least squares*: each iteration solves a weighted LS fit with weights w_i = psi(r_i)/r_i derived from the loss, downweighting large residuals, then recomputes residuals and reweights to convergence. L1 (least absolute deviations) is the limiting robust case, solvable as an LP; it tolerates up to ~50% outliers but is non-smooth.

**Limitations.** Redescending losses (Tukey) have multiple local minima and need a good seed (typically an LS or RANSAC fit) and a robust scale estimate (MAD). IRLS can oscillate; convergence guarantees are conditional.

**Kernel relevance.** The refinement stage after RANSAC in reverse engineering: RANSAC gives the inlier set and a coarse fit, IRLS-with-Huber/Tukey polishes it while staying robust to residual outliers. *Forbidden* on the metrology path: GD&T standards require all measured points to be honored, so no outlier rejection or downweighting is permitted when certifying a tolerance. Keel must gate M-estimators behind an explicit "reverse-engineering mode" flag.

---

## Theme 6: Reference software, validation, and inter-software disagreement

### NIST Algorithm Testing System and reference datasets
**Citation.** Shakarji, C. M. (1998), op. cit.; plus the NIST ATS / Internet-based proficiency-test datasets and the documented B89.4.10 / ASME activities.

**Method.** NIST maintains reference implementations and certified test datasets ("softgauges") with known answers for LS, Chebyshev, MIC, and MCC fits of standard primitives. CMM-software vendors and labs run the data and compare. The 1998 and 2007 papers both report that reference results differ significantly from algorithms in industrial use, especially for one-sided and min-zone fits.

**Limitations.** Datasets cover canonical primitives, not arbitrary freeform; access historically gated.

**Kernel relevance.** Keel's fitting test suite should ingest NIST reference datasets as golden tests. Passing them is the credible claim of metrology-grade fitting.

### PTB TraCIM validation service
**Citation.** Forbes, A. B., et al. / PTB (2015 onward). TraCIM (Traceability for Computationally-Intensive Metrology). See JSSS 10, 289 (2021) and PTB TraCIM service pages. https://www.ptb.de/cms/en/ptb/fachabteilungen/abt5/fb-53/ag-532/tracim.html

**Method.** An online validation service: the client requests a test dataset (point clouds for line, plane, circle, cylinder, cone, sphere) for a chosen criterion (Gauss LS or Chebyshev), runs its software, uploads results, and TraCIM compares against PTB reference values within a customer-specified numerical tolerance, issuing a certificate. In use since 2015; 90+ certifications across 21 countries; extended to screw-thread and freeform/asphere reference softgauges.

**Limitations.** Per-criterion, per-primitive scope; a paid service; certifies numerical agreement, not the appropriateness of the chosen criterion.

**Kernel relevance.** The external certification path for Keel's fitting subsystem. Even without paying for certificates, the TraCIM data-model and tolerance philosophy (compare to reference within a stated numerical bound) is the right validation contract for Keel's CI.

### Inter-comparison of commercial CMM software
**Citation.** Representative: NIST studies (early 2000s) and the GIDEP alert tradition; e.g. comparative analyses of CMM form-fitting algorithms (ResearchGate / IMEKO surveys) and the 1988 GIDEP alert.

**Method.** Feed identical point sets to multiple commercial CMM packages and compare reported sizes and form errors. The 1988 GIDEP alert first flagged that different least-squares implementations gave different results for the same data. NIST's 2002-era work found serious problems specifically in commercial MIC, MCC, and min-zone fits. Least-squares form evaluation, being two-sided, systematically over-estimates form tolerances relative to min-zone and can reject good parts.

**Limitations.** Findings age as software improves, but the structural cause (criterion choice plus solver quality) persists.

**Kernel relevance.** The empirical justification for everything in this file: choosing the right objective and a certified solver is not academic, it changes pass/fail verdicts. Keel differentiates by being explicit and reference-validated where commercial tools are silently divergent.

---

## Theme 7: Uncertainty of fitted parameters

### GUM-style propagation and sampling-strategy effects (ISO 10360, BS 7172)
**Citation.** ISO 10360-5:2020 (probing-error sampling on reference spheres); BS 7172:1989 (guide to assessment of position, size, departure from nominal form of geometric features); JCGM 100 (GUM).

**Method.** Two routes to fitted-parameter uncertainty. Analytical GUM propagation: linearize the fit at the solution; the parameter covariance is sigma^2 (J^T J)^{-1} where J is the fit Jacobian and sigma^2 the residual variance, then propagate to derived quantities (e.g. diameter, axis tilt). Sampling strategy drives uncertainty directly: ISO 10360-5 prescribes well-distributed points covering at least a hemisphere of a test sphere (25-point form tests, 125-probing multi-stylus tests); BS 7172 gives guidance on the number and distribution of probe points per feature so the fit is well-conditioned (too few or clustered points inflate (J^T J)^{-1}). Under-sampling a cylinder along too short an axis length, for instance, makes axis-direction uncertainty explode.

**Limitations.** Analytical propagation assumes linearity near the solution and Gaussian noise; it ignores systematic CMM errors. Sampling guidance is feature-specific and partly heuristic.

**Kernel relevance.** Keel's fitter should *return the parameter covariance* alongside the fit (it falls out of the Jacobian already computed), so downstream code can report fitted-feature uncertainty and flag ill-conditioned fits (near-degenerate primitives, insufficient/clustered sampling).

### Virtual CMM / simulation-based uncertainty (ISO 15530-4)
**Citation.** ISO/TS 15530-4:2008. Geometrical product specifications (GPS) - CMMs: Technique for determining the uncertainty of measurement - Part 4: Evaluating task-specific measurement uncertainty using simulation. https://www.iso.org/standard/38696.html

**Method.** Monte-Carlo "virtual CMM": instead of an analytical model, perturb each commanded measurement point by a sampled error drawn from a model of the CMM's error sources, re-run the entire evaluation (including the fit) hundreds/thousands of times, and take the spread of the result as the task-specific uncertainty. Naturally captures how the *fitting algorithm* and *sampling strategy* propagate measurement error into the final tolerance verdict, with no closed-form model required.

**Limitations.** Needs a validated CMM error model; computationally heavy (thousands of full evaluations); only as good as the perturbation model.

**Kernel relevance.** If Keel ever drives or simulates an inspection pipeline, the virtual-CMM pattern (perturb inputs, re-fit, aggregate) is the model. Even offline, running Keel's fitter inside a Monte-Carlo loop gives task-specific uncertainty for free, since the fitter is deterministic and fast.

---

## Theme 8: Primitive detection / segmentation from dense data

Covered substantially under Theme 5 (efficient RANSAC, MLESAC) and Theme 2 (faithful fitting, robust segmentation). The exact-fitting interface is the load-bearing concern for Keel: a detector/segmenter (RANSAC, region growing with curvature classification, or a learned model of the ParSeNet/HPNet class, treated in another file) produces (primitive-type, inlier-set) pairs, and the *only contract Keel's exact fitter needs* is that input. Curvature-based region growing classifies each point by estimated principal curvatures (k1, k2): both near zero implies planar, one near zero cylindrical, equal nonzero spherical, distinct nonzero toroidal/freeform, then grows connected regions of consistent classification. Hough-transform variants accumulate votes in primitive-parameter space (plane normal-distance, sphere center-radius) and are robust but memory-heavy in high dimensions, so they suit planes/circles more than cones/tori. Learned detectors output the same (type, membership) handoff. Keel should define a stable internal `PrimitivePatch { kind, point_indices }` interface so any detector, classical or learned, can feed the same exact fitter.

---

## Theme 9: Constrained and simultaneous fitting

### Benko, Kos, Varady, Andor, Martin: constrained fitting in reverse engineering
**Citation.** Benko, P., Kos, G., Varady, T., Andor, L., & Martin, R. (2002). Constrained fitting in reverse engineering. *Computer Aided Geometric Design, 19*(3), 173-205. https://doi.org/10.1016/S0167-8396(01)00085-1

**Method.** Fits multiple curves/surfaces simultaneously while enforcing inter-feature constraints (parallelism, perpendicularity, coaxiality, concentricity, equal-radius, symmetry, fixed angles) so the recovered model has CAD-clean regularities and tight tolerances. Auxiliary/explicit constraint formulation; constraints enter as equality conditions in a combined nonlinear least-squares system over all primitives' parameters jointly, solved together rather than primitive-by-primitive. Because automatic regularity detection over-generates constraints (more than can hold simultaneously), the paper contributes a numerical method to *resolve conflicts*, selecting a consistent maximal subset and distributing residual error.

**Limitations.** The joint system is larger and can be ill-conditioned; constraint selection is heuristic; needs good per-primitive seeds (from unconstrained fits) to converge.

**Kernel relevance.** This is Keel's *canonical recovery* engine. After unconstrained per-primitive fits, run constrained simultaneous fitting to snap axes to exact perpendicularity/coaxiality and radii to equality, producing a model whose regularities are exact (so downstream Booleans and feature recognition behave). Directly ties to the beautification goal.

### Beautification: choosing consistent constraints
**Citation.** Langbein, F. C., Marshall, A. D., & Martin, R. R. (2004). Choosing consistent constraints for beautification of reverse engineered geometric models. *Computer-Aided Design, 36*(3), 261-278. https://doi.org/10.1016/S0010-4485(03)00108-8

**Method.** The companion to constrained fitting: detect candidate regularities (approximate symmetries, alignments, equal dimensions) across the recovered model, score them, and select a *consistent, non-conflicting* set to impose. Handles dependency between regularities (imposing one may imply or contradict another), choosing weakly-dependent major regularities that can be satisfied together, then handing the chosen set to constrained fitting for enforcement.

**Limitations.** Combinatorial constraint-selection; thresholds for "approximately equal/parallel" are tuning-sensitive.

**Kernel relevance.** The decision layer above constrained fitting in Keel's canonical-recovery and simplify pipelines: it decides *which* regularities to enforce before the solver enforces them. Important so healing does not over-constrain (snapping near-but-intentionally-different features together).

---

## Theme 10: Freeform deviation evaluation

### Registration plus minimum-zone profile evaluation for freeform surfaces
**Citation.** Representative: Li, Y., & Gu, P. (2005). Free-form surface inspection techniques (CAD survey); plus min-zone freeform works such as the primal-dual interior-point NURBS min-zone method and PCA-based deviation-zone fitting. See: "A registration method for profile error inspection of complex surface under minimum zone criterion," *IJPEM* (2019); "Fast evaluation of minimum zone form errors of freeform NURBS surfaces," *Procedia CIRP* (2015).

**Method.** The freeform inspection pipeline has two coupled stages. (1) *Localization / registration*: align the measured cloud to the nominal CAD (NURBS) surface by solving for the rigid transform (3 rotations, 3 translations) that minimizes point-to-surface distance, typically ICP variants where the "closest point" is the orthogonal projection onto the NURBS surface (the point-to-NURBS distance from the projection research). (2) *Deviation evaluation*: with points registered, compute signed orthogonal deviations and report the profile error. Per ISO 1101 the profile tolerance is a min-zone quantity, so the proper evaluation finds the rigid transform minimizing the *peak-to-valley* of signed deviations (a min-zone over the registration parameters), not merely the LS-aligned RMS. Practical methods decompose the NURBS into Bezier patches, do ICP + orthogonal-distance refinement, then a primal-dual interior-point min-zone solve; PCA-based methods accelerate the deviation-zone fit.

**Limitations.** Registration and min-zone are coupled and non-convex; improper registration causes false rejection of good parts (a documented open problem). Point-to-NURBS projection is itself an iterative root-find (Newton on the surface), expensive over large clouds.

**Kernel relevance.** This closes the loop with Keel's point-to-NURBS projection research: profile-tolerance evaluation *is* localization + projection + min-zone. Keel already needs robust point-to-NURBS distance for many tasks; here it becomes the inner loop of an inspection-grade profile evaluator. The min-zone-over-registration framing (not LS) is the correctness requirement for profile tolerances.

---

## Fitting subsystem design for Keel

The research points to a single, layered fitting library, parameterized by objective, serving all four consumers. Recommended architecture:

**1. One core, objective as a parameter.** A `fit(primitive_kind, points, objective, constraints) -> Fit` API where `objective` is an enum: `LeastSquares`, `MinZone`, `MinCircumscribed`, `MaxInscribed`, `ConstrainedLeastSquares` (datum). Never hardcode L2. This is the lesson of the entire objective taxonomy and the CMM-disagreement literature.

**2. Layered solvers, LS always the seed.**
- Algebraic warm start: Taubin circle/sphere, SVD/PCA line/plane, Eberly direction-reduction for cylinder. Bounded time, no iteration.
- Exact LS: Shakarji 1998 orthogonal-distance + Levenberg-Marquardt with explicit Jacobians and per-iteration direction renormalization; cylinder/cone restart strategy. This is the universal refinement and the seed for all one-sided/min-zone objectives.
- One-sided / Chebyshev: Shakarji-Srinivasan 2007 iterate-from-LS for MIC/MCC/min-zone of primitives; Welzl/Gartner for smallest-enclosing circle/sphere; Carr-Ferreira sequential LP for 3D flatness/cylindricity min-zone; exact Voronoi-annulus for certifiable 2D roundness/straightness.
- Constrained: Shakarji constrained-LS for datums; Benko et al. simultaneous constrained fitting for canonical recovery, with Langbein constraint selection above it.

**3. Faithful degeneracy handling everywhere.** Use Lukacs-Marshall-Martin faithful parameterizations so cylinder-tends-to-plane, cone-tends-to-cylinder, torus-tends-to-sphere degrade gracefully. Pair with a type-selection test that prefers the simpler primitive when residuals are statistically indistinguishable. This is the shared backbone for healing, simplify, and canonical recovery.

**4. Two explicit modes gating robustness.**
- *Metrology mode*: all points honored, no outlier rejection, deterministic, objective dictated by the GD&T callout, result accompanied by a min-zone contact-condition certificate or a Voronoi-vertex certificate. M-estimators and RANSAC forbidden here.
- *Reverse-engineering mode*: RANSAC/MLESAC detection (octree-localized minimal sampling with normals) to partition the cloud, then exact LS refit per patch, then IRLS (Huber/Tukey) polish, then constrained simultaneous fitting for cleanup. Outlier rejection welcomed.
A single flag selects the mode; the core solvers are shared, only the pre/post robustness stages differ.

**5. Always return covariance and a certificate.** Every fit returns parameter covariance sigma^2 (J^T J)^{-1} (free from the Jacobian) for uncertainty reporting and ill-conditioning detection, and min-zone/one-sided fits return their contact configuration as an optimality certificate.

**6. Validate against references.** NIST ATS reference datasets and the TraCIM data-model as golden CI tests. Passing them is Keel's metrology-grade claim; the documented commercial-software divergence is the differentiator.

**7. Shared interfaces tie subsystems together.** A `PrimitivePatch { kind, point_indices }` handoff from any detector (classical or learned) to the exact fitter; the point-to-NURBS projection (separate research) plugs in as the inner loop of freeform profile evaluation (localization + projection + min-zone). Constrained fitting is the single engine behind both datum simulation (no-penetration constraint) and canonical recovery (inter-primitive regularity constraints).

The unifying insight: datum simulation, healing/simplify, reverse engineering, and canonical recovery are not four fitting libraries. They are four configurations (objective, robustness mode, constraint set) of one library built on Shakarji's orthogonal-distance core, with the objective always explicit and the result always certified.

---

## References

1. Shakarji, C. M. (1998). Least-squares fitting algorithms of the NIST Algorithm Testing System. *Journal of Research of the National Institute of Standards and Technology, 103*(6), 633-641. https://doi.org/10.6028/jres.103.043
2. Shakarji, C. M., & Srinivasan, V. (2007). Reference algorithms for Chebyshev and one-sided data fitting for coordinate metrology. *CIRP Annals, 56*(1), 469-472. https://doi.org/10.1016/j.cirp.2007.05.112
3. Shakarji, C. M., & Srinivasan, V. (2016). On algorithms and heuristics for constrained least-squares fitting of circles and spheres to support standards. NIST publication. https://www.nist.gov/publications/algorithms-and-heuristics-constrained-least-squares-fitting-circles-and-spheres-support
4. Eberly, D. (2020). *Least squares fitting of data by linear or quadratic structures.* Geometric Tools, LLC. https://www.geometrictools.com/Documentation/LeastSquaresFitting.pdf
5. Al-Sharadqah, A., & Chernov, N. (2009). Error analysis for circle fitting algorithms. *Electronic Journal of Statistics, 3*, 886-911. https://arxiv.org/pdf/0907.0421 (Kasa 1976; Pratt 1987; Taubin 1991 originals.)
6. Lukacs, G., Marshall, A. D., & Martin, R. R. (1998). Faithful least-squares fitting of spheres, cylinders, cones and tori for reliable segmentation. *ECCV '98*, LNCS 1406, 671-686. https://doi.org/10.1007/BFb0055697
7. Marshall, D., Lukacs, G., & Martin, R. (2001). Robust segmentation of primitives from range data in the presence of geometric degeneracy. *IEEE TPAMI, 23*(3), 304-314. https://doi.org/10.1109/34.910883
8. Garcia-Lopez, J., Ramos, P. A., & Snoeyink, J. (1998). On the width and roundness of a set of points / Fitting a set of points by a circle. *Discrete & Computational Geometry / SoCG.*
9. Carr, K., & Ferreira, P. (1995). Verification of form tolerances: straightness/flatness (Part I) and cylindricity/median-line straightness (Part II). *Precision Engineering, 17*(2), 144-156.
10. Hermann, G. (2007). Simple procedure for minimum zone evaluation of geometric features. *SAMI.* https://conf.uni-obuda.hu/sami2007/39_Hermann.pdf
11. Welzl, E. (1991). Smallest enclosing disks (balls and ellipsoids). *New Results and New Trends in Computer Science*, LNCS 555, 359-370.
12. Schnabel, R., Wahl, R., & Klein, R. (2007). Efficient RANSAC for point-cloud shape detection. *Computer Graphics Forum, 26*(2), 214-226. https://doi.org/10.1111/j.1467-8659.2007.01016.x
13. Torr, P. H. S., & Zisserman, A. (2000). MLESAC: A new robust estimator with application to estimating image geometry. *Computer Vision and Image Understanding, 78*(1), 138-156. https://doi.org/10.1006/cviu.1999.0832
14. Huber, P. J. (1981). *Robust Statistics.* Wiley. (Huber, Tukey biweight, Cauchy losses; IRLS.)
15. Benko, P., Kos, G., Varady, T., Andor, L., & Martin, R. (2002). Constrained fitting in reverse engineering. *Computer Aided Geometric Design, 19*(3), 173-205. https://doi.org/10.1016/S0167-8396(01)00085-1
16. Langbein, F. C., Marshall, A. D., & Martin, R. R. (2004). Choosing consistent constraints for beautification of reverse engineered geometric models. *Computer-Aided Design, 36*(3), 261-278. https://doi.org/10.1016/S0010-4485(03)00108-8
17. Forbes, A. B., et al. / PTB (2015 onward). TraCIM: Traceability for Computationally-Intensive Metrology. https://www.ptb.de/cms/en/ptb/fachabteilungen/abt5/fb-53/ag-532/tracim.html ; JSSS 10, 289 (2021).
18. ISO/TS 15530-4:2008. GPS - CMMs: Evaluating task-specific measurement uncertainty using simulation. https://www.iso.org/standard/38696.html
19. ISO 10360-5:2020. GPS - Acceptance and reverification tests for CMS - Part 5: CMMs using contacting probing systems (point sampling). https://www.iso.org/standard/73431.html ; BS 7172:1989 (assessment of position, size, form of geometric features).
20. ASME Y14.5.1-2019, Mathematical Definition of Dimensioning and Tolerancing Principles; ISO 14405-1 (size), ISO 1101 (geometric tolerances), ISO 5459 (datums). Discussion: Salsbury / Mitutoyo Y14.5-2018 overview; ZEISS ASME-vs-ISO-GPS comparison.
21. Freeform profile evaluation: registration + minimum-zone for complex surfaces. Representative: *A registration method for profile error inspection of complex surface under minimum zone criterion*, IJPEM (2019); *Fast evaluation of minimum zone form errors of freeform NURBS surfaces*, Procedia CIRP (2015); Li & Gu (2005), free-form surface inspection survey, CAD.
