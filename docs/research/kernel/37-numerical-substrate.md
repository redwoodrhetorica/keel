# The Numerical Substrate of the Keel Kernel

Research dossier for the Keel kernel on the **math-layer infrastructure** that sits beneath every geometric operation: dense small-matrix linear algebra, sparse constrained linear solvers, least-squares stability, eigenvalue/SVD computation, automatic versus analytic differentiation, interval and affine arithmetic, robust nonlinear (Newton/Gauss-Newton/Levenberg-Marquardt) solving, quadrature, conditioning discipline, and transcendental accuracy. This is the substrate that the constrained-surfacing (kernel/33), variational-surfacing (kernel/32), intersection, projection, mass-property (kernel/22), and predicate-cascade (kernel/11) layers all call into.

**Relationship to sibling dossiers.** This file is deliberately *below* the geometry. It does **not** re-derive the tolerance model or the exact-predicate cascade (kernel/11, a-robust-numerics own that): it treats the predicate cascade as a *client* that needs interval and exact arithmetic from `keel-math`, and the tolerance model as a *given* scaling discipline. It does **not** re-derive the continuity algebra of kernel/33 or the fairness functionals of kernel/32: it treats the assembled systems `(N^T W N + lambda K) x = b` and the KKT saddle systems as *given sparse linear-algebra problems* and asks how to solve them stably and fast. Where kernel/32-33 ask "what equations," this file asks "what solver, what arithmetic, what conditioning."

The central engineering question for Keel: which numerical primitives does `keel-math` **own** (small dense LA, predicates, interval arithmetic, root finders, Newton, quadrature) for full control of epsilons and determinism, and which does it **wrap** (the supernodal sparse Cholesky / sparse QR that would be insane to reimplement), and what are the stability policies (QR-vs-normal-equations, AD-vs-analytic, scaling) that keep the whole kernel out of the ill-conditioned ditch.

---

## 1. Small dense linear algebra: the 2x2 to 6x6 hot path

A B-rep kernel spends most of its numerical time not in big sparse solves but in a torrent of *tiny* dense solves: every Newton step of a curve/surface intersection is a 2x2 or 3x3 solve; every point-projection footpoint solve is 1x1 or 2x2; plane-plane-plane intersection is a 3x3; frame/axis computation, the local fillet cross-section solve, the blend-spine solve, all live in the 2x2 to 6x6 range. These are called billions of times over a kernel's life, so their per-call cost and their per-call robustness both matter.

### 1.1 Cramer versus LU at tiny sizes, and the cross-product forms

For a 2x2 system `[[a,b],[c,d]] x = r`, Cramer's rule gives `det = a*d - b*c`, `x0 = (r0*d - b*r1)/det`, `x1 = (a*r1 - r0*c)/det`. For 3x3 the determinant and the three numerator determinants are scalar triple products. Cramer is branch-light, vectorizes, and at n<=3 is competitive with or faster than LU with partial pivoting, which is why kernels use it for the inner loop. **But** Cramer is *not backward stable*: Higham (Section 9) gives Cramer's rule for a 2x2 system as the textbook example of a method that is forward stable but **not backward stable**. The danger is not the arithmetic count but cancellation in `a*d - b*c` when the two products are close (a near-singular, ill-conditioned system) and the absence of pivoting. The kernel's defense is **two-pronged**: (a) use a fused-multiply-add or a Kahan-style 2x2 determinant (`fma(a, d, -b*c)` computes `a*d - b*c` with one rounding in the cross term) to kill the cancellation in the determinant itself; (b) gate on a *scaled* determinant test, comparing `|det|` against `eps * (|a|+|b|)*(|c|+|d|)` or a row-norm product, not against an absolute epsilon, so that near-singularity is detected relative to the entries. For the 3x3 case the plane-intersection solve is best written as the **homogeneous / cross-product form**: the intersection of three planes `n_i . x = d_i` is `x = (d1 (n2 x n3) + d2 (n3 x n1) + d3 (n1 x n2)) / (n1 . (n2 x n3))`, which is Cramer expressed through cross products. The line of intersection of two planes is `n1 x n2` directly. These cross-product forms are not more accurate than Cramer (they *are* Cramer), but they make the geometric degeneracy (parallel planes, `n1 x n2 ~ 0`) explicit and testable, and they avoid forming and pivoting a matrix.

### 1.2 When to escalate to LU/QR

For the small but ill-conditioned solve (a grazing intersection, a nearly tangent projection where the 2x2 Newton matrix is near-singular), the kernel should escalate from the fast Cramer path to a **pivoted** path: LU with partial pivoting for the square solve, or a 2x2/3x3 Givens-based QR / SVD for the near-rank-deficient solve where a minimum-norm step is wanted (Section 4). The policy is a *cascade*, mirroring the predicate cascade: fast Cramer with FMA in the determinant and a scaled singularity gate; on gate failure, a pivoted/orthogonal fallback that either solves stably or reports rank deficiency to the caller (a tangency, which the intersection code handles geometrically rather than numerically).

**Kernel relevance.** `keel-math` owns small fixed-size `Mat2`, `Mat3`, `Mat4`, `Mat6` solvers with: FMA-fused determinants, scaled singularity tests tied to the tolerance model, explicit cross-product 3x3 forms for plane geometry, and a documented escalation to a pivoted fallback. This is hot-path code; it is `#[inline]`, allocation-free, and const-generic over dimension where it pays. No third-party dense library is wrapped here, because the kernel needs to control exactly which epsilon decides singular and to fuse the determinant.

---

## 2. Sparse direct solvers for the surfacing/fitting/variational systems

The surfacing and fitting layers (kernel/32-33) assemble *large sparse* systems. Two shapes dominate: the **regularized normal-equations / fairness system** `(N^T W N + lambda K) x = b`, which is sparse, symmetric, and positive (semi)definite (SPD), and the **KKT / saddle-point system** that arises when hard constraints are imposed by Lagrange multipliers,

```
[ H   C^T ] [ x      ]   [ b ]
[ C   0   ] [ lambda ] = [ d ]
```

which is sparse, symmetric, but **indefinite** (it has both positive and negative eigenvalues). The substrate question is direct versus iterative, and which factorization.

### 2.1 CHOLMOD: supernodal sparse Cholesky

**Citation.** Chen, Y., Davis, T. A., Hager, W. W., & Rajamanickam, S. (2008). Algorithm 887: CHOLMOD, supernodal sparse Cholesky factorization and update/downdate. *ACM Transactions on Mathematical Software, 35*(3), Article 22. https://dl.acm.org/doi/10.1145/1391989.1391995 ; user guide https://people.clas.ufl.edu/hager/files/cholmod_alg.pdf

**Method.** CHOLMOD factors a sparse SPD matrix `A = L L^T` (or `A = L D L^T`). It runs a **symbolic analysis** first (a fill-reducing permutation, by default AMD/METIS nested dissection, then an elimination tree and a supernodal structure), then a **numeric** factorization. The supernodal step groups columns with identical-or-nested sparsity patterns into dense **supernodes** and factors each with Level-3 BLAS (dense LAPACK/`dgemm`), so it reaches a large fraction of peak floating-point throughput, unlike a scalar up-looking code. CHOLMOD also ships a non-supernodal up-looking path (better for very sparse matrices that get no BLAS benefit) and chooses between them by a flop-count heuristic. It supports **update/downdate** (rank-1 modification of `L` when a row/column of `A` changes), which is exactly what an interactive surfacing edit (move one constraint, refactor cheaply) wants. It also has an `LDL^T` mode that can factor **symmetric indefinite** matrices, relevant to the KKT case (though indefinite KKT is better served by a true symmetric-indefinite/Bunch-Kaufman code or by MINRES, Section 2.4).

**Numerical properties.** Cholesky of an SPD matrix is **unconditionally backward stable without pivoting** (Higham): no growth factor problem, no pivoting needed, which is why the SPD fairness system is the friendly case. The conditioning of the *solve* is `kappa(A)`; for the normal-equations form `A = N^T W N` that is `kappa(N)^2`, the squaring penalty that Section 3 addresses. The symbolic phase is reusable: if only the *numeric* values change but the sparsity pattern is fixed (re-fitting with new data on the same control net and knot vector), the expensive ordering/symbolic step is computed once and amortized.

**Kernel relevance.** CHOLMOD (or a faer-native equivalent, Section 2.5) is the **default solver for the SPD surfacing/fairing system**. Keel wraps it rather than reimplementing supernodal Cholesky: a correct, fast supernodal multifrontal code with good orderings is many engineer-years of work, the algorithm is well specified, and the wrap boundary is clean (CSC matrix in, factor object out, triangular solves). The update/downdate and reusable-symbolic features map directly onto interactive re-solve.

### 2.2 SuiteSparseQR: sparse rank-revealing QR for least squares

**Citation.** Davis, T. A. (2011). Algorithm 915, SuiteSparseQR: Multifrontal multithreaded rank-revealing sparse QR factorization. *ACM Transactions on Mathematical Software, 38*(1), Article 8. https://dl.acm.org/doi/10.1145/2049662.2049670

**Method.** SuiteSparseQR computes `A = Q R` for a sparse, possibly rectangular, possibly rank-deficient `A` using the **multifrontal** method: it builds a column elimination tree, organizes the factorization into a tree of dense **frontal matrices**, and applies Householder reflections within each front using LAPACK and multithreaded BLAS, with parallelism across independent fronts (Intel TBB). Crucially it finds the column elimination tree and front structure **without forming the pattern of `A^T A`**, so it never materializes the normal-equations sparsity (which can be far denser than `A`). It is **rank-revealing**: rank detection happens inside each front by Heath's method (no global column pivoting needed), so it returns a usable factorization and an estimated rank even when `A` is rank-deficient.

**Numerical properties.** Solving least squares `min ||A x - b||` via `R x = Q^T b` from a Householder QR has condition number `kappa(A)`, **not** `kappa(A)^2`. This is the headline reason to prefer QR over normal equations for ill-conditioned fitting: it avoids squaring the condition number, so it retains roughly twice as many correct digits when `A` is ill-conditioned (a high-degree fit, near-collinear data, an almost-degenerate parameterization). The cost is higher than Cholesky-on-normal-equations (QR does more flops and `Q` is bulkier), which is the tradeoff.

**Kernel relevance.** SuiteSparseQR is the **stability fallback for the fitting solve**: when the fitting matrix `N` is well-conditioned, the kernel uses the cheap normal-equations + sparse Cholesky path; when conditioning is poor or rank deficiency is suspected (Section 9 condition estimate trips), it escalates to sparse QR, which both solves stably and *reports the numerical rank*, telling the surfacing layer that the control net has redundant degrees of freedom (it should reduce the net or add regularization, Section 3).

### 2.3 Direct versus iterative at kernel problem sizes: the Botsch result

**Citation.** Botsch, M., Bommes, D., & Kobbelt, L. (2005). Efficient linear system solvers for mesh processing. In *Mathematics of Surfaces XI* (LNCS 3604, pp. 62-83). Springer. https://www.graphics.rwth-aachen.de/media/papers/solvers1.pdf

**Method / finding.** Botsch, Bommes, and Kobbelt benchmark the sparse SPD systems that pervade geometry processing (Laplacian editing, fairing, parameterization, the exact analogues of Keel's fairness systems) across solver families: iterative conjugate gradient (with Jacobi and incomplete-Cholesky preconditioners), **multigrid** (then considered the gold standard for these elliptic systems), and **sparse direct** Cholesky (their wrapped library was a SuiteSparse-lineage supernodal code). Their central, somewhat surprising conclusion: for the matrix sizes typical of interactive mesh/surface editing (thousands to low millions of unknowns), **a good sparse direct solver beat even a highly tuned multigrid solver**, and was far easier to use and implement. The decisive advantage is **factor reuse**: in an interactive editing loop the matrix sparsity is fixed and often the numeric matrix is fixed while only the right-hand side changes (the user drags a handle), so the expensive factorization is computed *once* and each interactive solve is a cheap pair of triangular back-substitutions. Iterative methods must restart their full iteration on every right-hand side.

**Numerical properties.** Direct solvers give the solution to working precision in one shot (no convergence tolerance to tune, no preconditioner to design), and SPD Cholesky needs no pivoting. The cost is memory: the factor `L` fills in beyond `A`'s sparsity, and for very large 3D-volumetric systems the fill can become prohibitive, the regime where iterative/multigrid wins again.

**Kernel relevance.** This paper is the **direct evidence** for Keel's policy: at the problem sizes a B-rep surfacing/fairing operation produces (a single trimmed NURBS face's control net, a local patch, a feature region, almost never more than ~10^5 unknowns), **sparse direct factorization is the default**, with factor reuse exploited across interactive re-solves. Iterative methods (Section 2.4) are reserved for the indefinite KKT case and for any unusually large volumetric system. This justifies wrapping CHOLMOD/SuiteSparseQR rather than building a multigrid stack.

### 2.4 Iterative methods for the KKT/saddle case: CG, MINRES, preconditioning

The KKT saddle system is **symmetric indefinite**, so plain Cholesky does not apply and **conjugate gradient (CG) is not valid** (CG requires SPD). The right Krylov method is **MINRES** (minimum residual), which works for any symmetric (possibly indefinite) system and minimizes the residual norm over the Krylov subspace. For the SPD fairness system, **preconditioned CG** is the iterative option when direct fill is too large; a good preconditioner (incomplete Cholesky, or a multigrid V-cycle as preconditioner) is essential because the fairness operator (a discrete bi-Laplacian / thin-plate operator) is poorly conditioned (`kappa` grows like `h^{-4}` in the mesh spacing for fourth-order fairing). For the KKT system, options are: (a) solve it directly with a **symmetric-indefinite (Bunch-Kaufman) sparse factorization**; (b) use MINRES with a block preconditioner; or (c) eliminate the constraints first (nullspace or Schur-complement reduction, Section 4.3) to get back to an SPD system solvable by Cholesky. Keel's default for moderate constraint counts is **constraint elimination to SPD then Cholesky** (robust, reuses the SPD path); MINRES is the fallback for large KKT systems where the reduced Hessian is too dense.

### 2.5 Rust ecosystem: faer, nalgebra, sprs

**Citation.** El Kazdadi (Quinones), S. (2024). faer: A linear algebra library for the Rust programming language. *Journal of Open Source Software* (review #6099). https://github.com/sarah-quinones/faer-rs ; benchmarks and paper at https://github.com/sarah-quinones/faer-rs/blob/main/paper.md

**Method / state of the ecosystem.**
- **faer** is the most serious native-Rust dense+sparse linear algebra effort. Its dense kernels (LLT/LDLT Cholesky, Bunch-Kaufman, LU, QR, SVD, eigendecomposition) are hand-tuned with its own micro-kernels and **match or beat Eigen and OpenBLAS** on many sizes per its published benchmarks, and decisively beat nalgebra and ndarray on larger dense problems. faer also implements **sparse** Cholesky, LU, and QR (the supernodal foundations were built on top of its dense kernels), making it the candidate to provide *both* the SPD sparse Cholesky and the sparse QR Keel needs **in pure Rust**, avoiding the C FFI to SuiteSparse.
- **nalgebra** is the established general-purpose Rust LA crate: excellent small fixed-size types (`Matrix2`..`Matrix6`, const-generic), good for the dense hot path of Section 1, but its dense factorizations are not competitive at scale and its sparse story is thin.
- **sprs** is a sparse-matrix data-structure crate (CSC/CSR, sparse-sparse products, a basic sparse Cholesky via `ldl`) useful as a CSC container and for assembly, but not a high-performance supernodal solver on its own.

**Numerical properties.** A native-Rust solver (faer) buys Keel **determinism and epsilon control** (no hidden BLAS dispatch picking different kernels and thus different roundings on different CPUs, the determinism concern of kernel/14) and **no unsafe C FFI / build-system pain**. The risk is maturity: SuiteSparse has decades of hardening and rank-revealing/ordering sophistication that a younger library may not fully match on pathological matrices.

**Kernel relevance.** The recommended posture: **`keel-math` owns the small dense LA itself** (Section 1, on nalgebra-style fixed-size types or its own). For the **sparse** surfacing solves it **wraps faer first** (pure Rust, deterministic, single-language build), keeping a **CHOLMOD/SuiteSparseQR FFI behind a feature flag** as a battle-tested fallback and as a cross-check oracle during development. Eigen/LAPACK is the conceptual benchmark, not a dependency.

---

## 3. Least-squares formulations and stability

Fitting (kernel/23) and surfacing (kernel/32-33) are least-squares at heart. The formulation choice governs how many digits survive.

### 3.1 Normal equations versus QR versus SVD

To solve `min ||N x - b||_2`:
- **Normal equations**: form `N^T N` (SPD), factor by Cholesky, solve. Cheapest (`N^T N` is small if `x` is small; Cholesky is half the flops of QR). **But** `kappa(N^T N) = kappa(N)^2`: forming the normal-equations matrix **squares the condition number**, so a fit with `kappa(N) ~ 10^7` (entirely plausible for a high-degree fit or near-collinear samples) gives `kappa(N^T N) ~ 10^14`, at the edge of double precision, losing all accuracy. Normal equations are fine **only when `N` is well-conditioned**.
- **QR**: `N = QR`, solve `R x = Q^T b`. Condition number is `kappa(N)`, **not squared**. Backward stable. The default for anything that might be ill-conditioned. About twice the cost of normal equations.
- **SVD**: `N = U Sigma V^T`, `x = V Sigma^+ U^T b`. Most expensive, but **the only method that gracefully handles rank deficiency** and gives the minimum-norm solution and the full singular-value spectrum (hence the exact numerical rank and condition number). Used when rank deficiency is expected or must be diagnosed.

**Policy for Keel.** Default the surfacing fit to the **regularized normal-equations + sparse Cholesky** path (Section 2.1) because the fairness term `lambda K` (Section 3.3) *regularizes away* the conditioning problem, making `N^T W N + lambda K` well-conditioned even when `N` alone is not. Escalate to **sparse QR** when there is no regularization, when `lambda` is tiny, or when the condition estimate (Section 9) is poor. Reserve dense **SVD** for the small-but-critical solves where rank must be known exactly (e.g. the pose/PCA fit, the degenerate-Jacobian intersection step of Section 7).

### 3.2 Rank deficiency and the minimum-norm (pseudoinverse) solution

**Citation.** Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.). Johns Hopkins University Press. (Chapters 5-6 on QR, least squares, and the SVD.)

When `N` is rank-deficient (the control net has more degrees of freedom than the data constrains, common in under-determined surfacing), the least-squares solution is **not unique**: any vector in the nullspace of `N` can be added. The canonical choice is the **minimum-norm solution** `x = N^+ b`, where the Moore-Penrose pseudoinverse `N^+ = V Sigma^+ U^T` zeroes the reciprocals of the (numerically) zero singular values: `Sigma^+_{ii} = 1/sigma_i` if `sigma_i > tol`, else `0`. This picks the solution with smallest `||x||`, which geometrically is the *least wiggly* control net consistent with the data, a desirable default. **Truncated SVD (TSVD)** is the same idea as a regularizer: drop the components with `sigma_i < tol` to suppress amplification of noise by tiny singular values.

### 3.3 Tikhonov regularization: the fairness term as regularization

**Citation.** (Tikhonov regularization is standard; see Golub & Van Loan above and Hansen, P. C. (1998). *Rank-Deficient and Discrete Ill-Posed Problems*. SIAM.)

Tikhonov regularization solves `min ||N x - b||^2 + lambda ||L x||^2`, whose normal equations are `(N^T N + lambda L^T L) x = N^T b`. **This is exactly the kernel/32-33 fairness system** with `L^T L = K` the fairness/stiffness operator (the discretized thin-plate or membrane energy) and `lambda` the fairness weight. The SVD view: Tikhonov applies the **filter factor** `phi_i = sigma_i^2 / (sigma_i^2 + lambda)` to each singular component, so large singular values pass (`phi ~ 1`) and small ones are damped (`phi ~ sigma_i^2/lambda -> 0`), smoothly instead of TSVD's hard cutoff. The effect on conditioning is decisive: even if `N` is rank-deficient or wildly ill-conditioned, `N^T N + lambda K` is **SPD and well-conditioned** for any `lambda > 0` (the regularizer lifts the small/zero singular values away from zero), which is *why* the surfacing system is solvable by plain Cholesky with no pivoting. As `lambda -> 0` the solution approaches the minimum-norm least-squares solution; as `lambda` grows the solution is pulled toward the smoothest (lowest-energy) shape. The fairness weight is thus simultaneously a **shape control** and a **numerical conditioner**, a happy coincidence the surfacing layer exploits.

### 3.4 Constrained least squares: nullspace, weighting, KKT

When some conditions are **hard** (interpolate this point exactly, be exactly G1 to that face) rather than least-squares, the problem is constrained least squares: `min ||N x - b||^2 s.t. C x = d`. Three solution methods, with different conditioning:
- **Lagrange / KKT**: append multipliers, solve the saddle system of Section 2.4. Exact constraint satisfaction; indefinite system.
- **Nullspace method**: parameterize `x = x_p + Z y` where `C x_p = d` (a particular solution) and `Z` is a basis for the nullspace of `C`, then solve the **reduced unconstrained** problem `min ||N(x_p + Z y) - b||` for `y`, which is SPD and Cholesky-friendly. Numerically clean and recovers the SPD path; cost is forming `Z` (a QR of `C^T`).
- **Weighting / penalty**: replace the hard constraint by a heavily weighted soft constraint, `min ||N x - b||^2 + w ||C x - d||^2` with `w` huge. Simplest (just more rows), but **deliberately ill-conditions** the system (the large `w` blows up `kappa`), trading exactness for conditioning; usable only with a stable solver (QR) and moderate `w`.

**Policy for Keel.** Prefer the **nullspace reduction to an SPD system** for hard geometric constraints (it keeps the friendly Cholesky path and does not corrupt conditioning), falling back to the **KKT/MINRES** path when the constraint count is large enough that forming `Z` is expensive. Avoid the naive penalty method except for very soft "preferences."

---

## 4. Eigenvalue and SVD computation where the kernel needs it

The kernel needs small eigen/SVD in several distinct places, each with its own stability subtlety.

### 4.1 Principal curvatures: symmetric 2x2/3x3 eigenproblems

Principal curvatures and directions are the eigenvalues/eigenvectors of the **shape operator** (a 2x2 symmetric matrix `S = I^{-1} II`, the first fundamental form inverse times the second), or equivalently of a 3x3 symmetric curvature tensor when working in ambient coordinates. A 2x2 symmetric eigenproblem `[[a,b],[b,c]]` has the **closed form** `kappa_{1,2} = (a+c)/2 +/- sqrt(((a-c)/2)^2 + b^2)`, with eigenvectors from the Jacobi rotation angle `theta = 0.5 atan2(2b, a-c)`. This closed form is fine **if** the `sqrt` discriminant is computed without cancellation (it is a sum of squares, so it is safe) and the eigenvectors are taken from `atan2` (numerically robust) rather than from a division that can blow up when `a ~ c`.

**Citation.** Smith, O. K. (1961). Eigenvalues of a symmetric 3x3 matrix. *Communications of the ACM, 4*(4), 168. ; and the robust modern treatments: Eberly, D. (Geometric Tools), *A Robust Eigensolver for 3x3 Symmetric Matrices*, https://www.geometrictools.com/Documentation/RobustEigenSymmetric3x3.pdf ; Habera, M. et al. (2026), Numerically stable evaluation of closed-form expressions for eigenvalues of 3x3 matrices, *Numerical Algorithms*, https://link.springer.com/article/10.1007/s11075-026-02328-5 .

**Method / numerical properties.** For 3x3 symmetric eigenvalues the classical route is **Cardano/Viete trigonometric** formulas (eigenvalues of a symmetric 3x3 are real, so the cubic has three real roots expressible via `cos`): subtract the trace/3, compute the deviatoric invariants, and read the three eigenvalues as `2 sqrt(p) cos(phi + 2 pi k/3)`. The well-known trap: this **trigonometric closed form is numerically unstable when two eigenvalues are nearly equal** (the `acos` argument approaches `+/-1`, where its derivative is infinite, so a tiny input error produces a large angle error). Robust eigensolvers (Smith's method, Scherzinger-Dohrmann, Eberly's Geometric Tools code, and the 2026 Habera et al. stabilization) handle this by careful scaling, by clamping the `acos` argument, and by computing the eigenvectors as **cross products of `(A - kappa_i I)` columns** with selection of the numerically largest column to avoid cancellation, rather than from a possibly-singular linear solve. For Keel the **2x2 shape-operator closed form covers ordinary surface curvature**; the robust 3x3 path is needed for the inertia tensor (Section 4.4) and for ambient curvature tensors.

### 4.2 Polynomial root finding via the companion matrix

**Citation.** Edelman, A., & Murakami, H. (1995). Polynomial roots from companion matrix eigenvalues. *Mathematics of Computation, 64*(210), 763-776. https://www.osti.gov/biblio/181773 ; balancing: Parlett, B. N., & Reinsch, C. (1969). Balancing a matrix for calculation of eigenvalues and eigenvectors. *Numerische Mathematik, 13*, 293-304.

**Method.** The roots of a degree-`n` polynomial `p(x)` are the eigenvalues of its **companion matrix** `C` (an `n x n` matrix with the normalized coefficients in one row/column and a sub/super-diagonal of ones). Compute them with the standard QR eigenvalue algorithm (Francis double-shift). This is the method MATLAB's `roots` uses, and it is the kernel's general-purpose root finder for the cases where the structure-exploiting analytic root finder (Section 7 hodographs, or the Bezier-clipping/subdivision root isolator) is not available, in particular for the **resultant-based** elimination that reduces a curve/surface intersection to a single-variable polynomial whose roots are the intersection parameters (ties to the algebraic-number machinery of kernel/11).

**Numerical properties.** Edelman and Murakami's first-order error analysis shows the companion-matrix method is **backward stable in a precisely defined sense** (the computed roots are exact roots of a slightly perturbed polynomial), but the **conditioning of the roots themselves** can be poor (Wilkinson's classic ill-conditioned polynomial), which is a property of the problem, not the method. **Balancing** the companion matrix before QR (Parlett-Reinsch: a diagonal similarity that equalizes row and column norms) is **essential**: an unbalanced companion matrix can have wildly different row/column scales (because polynomial coefficients span many orders of magnitude), and balancing markedly improves the accuracy of the computed eigenvalues. Newer `O(n^2)` backward-stable companion-QR variants (Aurentz, Mach, Vandebril, Watkins) exist if root-finding throughput ever matters.

**Kernel relevance.** `keel-math` owns a small companion-matrix root finder **with mandatory balancing** for the moderate-degree univariate polynomials that fall out of resultants and of analytic intersection setups, used as the algebraic fallback when subdivision/clipping isolation is not preferred. It is paired with interval refinement (Section 6) when certified root enclosures are required by the predicate cascade.

### 4.3 PCA for pose and fitting; inertia-tensor diagonalization

**Principal component analysis** (eigen-decomposition of a 3x3 covariance matrix) gives the best-fit plane/line/frame for a point set: the smallest-eigenvalue eigenvector is the plane normal, the largest is the principal axis. This is the workhorse for **initial pose estimation** in fitting (kernel/23, canonical-geometry recovery kernel/24) and for oriented bounding boxes. Numerically it is the same **symmetric 3x3 eigenproblem** of Section 4.1, so it shares the robust solver. The **inertia tensor** of a solid (kernel/22 mass properties) is a symmetric 3x3 matrix whose eigenvalues are the **principal moments of inertia** and whose eigenvectors are the **principal axes**; diagonalizing it uses exactly the same robust symmetric 3x3 eigensolver. The lesson: a single, well-tested, numerically robust symmetric 3x3 eigensolver in `keel-math` serves curvature, PCA/pose, and inertia, so it is worth getting right once.

---

## 5. Automatic differentiation versus analytic derivatives

Newton-type solvers (intersection, projection, constraint, fitting) all need **Jacobians**, and curvature needs second derivatives. Two strategies: hand-coded analytic derivatives or automatic differentiation (AD).

### 5.1 Dual numbers and forward-mode AD

**Citation.** Griewank, A., & Walther, A. (2008). *Evaluating Derivatives: Principles and Techniques of Algorithmic Differentiation* (2nd ed.). SIAM. ; Rust realization: the `num-dual` crate, https://docs.rs/num-dual ; survey of Rust AD: ad-trait (2025), https://arxiv.org/html/2504.15976v1 .

**Method.** Forward-mode AD evaluates a function on **dual numbers** `a + b epsilon` with `epsilon^2 = 0`: arithmetic carries the value in the real part and the derivative in the `epsilon` part, so `f(x + 1*epsilon)` returns `f(x) + f'(x) epsilon` automatically by operator overloading, exact to machine precision (no finite-difference truncation error and no step-size choice). For a function `R^n -> R^m`, forward mode costs about `n` times the function evaluation (one sweep per input direction) and is **most efficient when `n` is small**, which is precisely the kernel's situation: intersection/projection Jacobians are 2x2 or 3x3 (`n <= 3`). **Higher-order** AD (hyper-dual numbers, or nested duals, or Taylor-mode) gives second derivatives for **curvature** (the shape operator needs `S_uu, S_uv, S_vv`). Rust's `num-dual` provides exactly these: scalar duals for first derivatives, vector duals for gradients/Jacobians, and second/third-order duals for Hessians/curvature, all generic over the scalar type so the same geometry code differentiates itself.

**Numerical properties.** AD derivatives are **exact** (to rounding), eliminating the dominant error and tuning headache of finite differences. The overhead is constant-factor (operator overloading, possible loss of vectorization), and forward-mode's `O(n)` cost is negligible at `n<=3`.

### 5.2 When analytic derivatives from the spline structure win

Hand-coded analytic derivatives win when the geometry has **exploitable structure** that AD cannot see. The premier example is the **hodograph**: the derivative of a degree-`p` Bezier/B-spline curve is *itself* a degree-`(p-1)` Bezier/B-spline with control points `p (P_{i+1} - P_i)/(knot span)`, a closed form that is **cheaper and more accurate** than differentiating the de Boor evaluation by AD, because it differentiates the *representation* analytically and then evaluates, rather than carrying duals through the whole evaluation. The same holds for surface partials (hodographs in `u` and `v`) and for rational NURBS (quotient rule on the homogeneous hodograph). Analytic derivatives are also better where the **same derivative is reused many times** (precompute the hodograph control net once, evaluate cheaply at many parameters) and where **determinism/auditability** matters (a closed form is inspectable; AD is correct-by-construction but opaque).

**Policy for Keel.** Use **analytic hodograph derivatives for spline curve/surface evaluation** (they are exact, cheaper, and reuse the de Boor machinery), and use **forward-mode dual-number AD for everything else**: the Jacobians of *assembled* constraint/intersection/projection systems where the function is a composition of many primitives and hand-deriving is error-prone and a maintenance liability. This is the standard "analytic where structured, AD where composed" split. Curvature uses analytic second-order hodographs for splines and second-order duals for general implicit/composed surfaces. Keep an AD path available as a **correctness oracle** to test the hand-coded analytic derivatives against (differencing AD vs analytic catches hodograph bugs immediately).

---

## 6. Interval and affine arithmetic

The predicate cascade (kernel/11), validated marching (kernel/35), and certified root isolation need **self-validating arithmetic** that returns rigorous enclosures, not point estimates. `keel-math` must own this, because it is below the predicates that depend on it.

### 6.1 Rounded interval arithmetic and the rounding-mode problem

**Citation.** Moore, R. E., Kearfott, R. B., & Cloud, M. J. (2009). *Introduction to Interval Analysis*. SIAM. ; the IEEE standard: IEEE Std 1788-2015 (interval arithmetic) and 1788.1-2017 (simplified). ; Rust realization: the `inari` crate, https://github.com/unageek/inari .

**Method.** Interval arithmetic replaces each number by an interval `[lo, hi]` guaranteed to contain the true value, and each operation by an enclosing interval operation: `[a,b] + [c,d] = [a+c, b+d]`, etc. To remain a **rigorous** enclosure under floating point, the lower bound must be computed with rounding **toward `-inf`** and the upper bound with rounding **toward `+inf`** (outward/directed rounding), so the true result is never excluded. This is the *inclusion property*, the whole point of interval arithmetic, and it **requires control of the FPU rounding mode**.

**Numerical properties / Rust realities.** Controlling the rounding mode in Rust is awkward: the standard library exposes **no portable rounding-mode API**, and the LLVM optimizer assumes round-to-nearest, so naively setting the hardware mode via FFI (`fesetround`) can be reordered or constant-folded incorrectly. The robust implementations sidestep this in one of two ways: (a) **MPFR** with explicit directed rounding (what `inari` does via `gmp-mpfr-sys` for correctness, at a speed cost), or (b) the "**round-to-nearest-only**" trick: compute in round-to-nearest and then widen each bound by one unit-in-the-last-place (or by a small multiple of the result magnitude), which is portable, fast, and still rigorous (slightly looser). WebAssembly has *no* rounding-mode instructions at all, forcing the ULP-widening approach. The principal weakness of naive interval arithmetic is the **dependency problem** (it ignores correlations between subexpressions, e.g. `x - x` yields `[-w, w]` not `[0,0]`) and the **wrapping effect** (a rotated box is enclosed by a larger axis-aligned box), which together cause intervals to **explode** over long computations.

**Kernel relevance.** `keel-math` owns an interval type with rigorous directed rounding (the ULP-widening, round-to-nearest variant by default for speed and portability and determinism, with an MPFR-backed exact mode available for the hardest predicate cases). This is the **filter and fallback arithmetic for the predicate cascade** (kernel/11): a predicate first runs in fast intervals; if the interval straddles zero (the sign is uncertain), it escalates to exact/MPFR. Determinism (kernel/14) favors the portable ULP-widening implementation whose result does not depend on the host FPU mode.

### 6.2 Affine arithmetic: combating the dependency problem

**Citation.** Comba, J. L. D., & Stolfi, J. (1993). Affine arithmetic and its applications to computer graphics. *Proceedings of SIBGRAPI '93*. ; de Figueiredo, L. H., & Stolfi, J. (2004). Affine arithmetic: Concepts and applications. *Numerical Algorithms, 37*(1-4), 147-158. https://lhf.impa.br/ftp/oral/aa.pdf

**Method.** Affine arithmetic (AA) represents a quantity as a **first-order affine form** `x = x_0 + x_1 eps_1 + x_2 eps_2 + ... + x_k eps_k`, where each `eps_i in [-1,1]` is a shared **noise symbol** and `x_0` is the central value. The crucial point: noise symbols are *shared* across quantities, so AA **tracks linear correlations** between subexpressions that interval arithmetic throws away. When `x - x` is computed, the shared `eps_i` terms cancel exactly, giving the tight `[x_0, x_0]` (no spurious width). Affine operations (`+`, `-`, scaling) are exact in the noise symbols; nonlinear operations (`*`, `sqrt`, `sin`) introduce a *new* noise symbol absorbing the linearization error, so the representation grows but stays rigorous. The result is dramatically **tighter enclosures than interval arithmetic over chained computations**, defeating the error explosion that plagues naive intervals, exactly the regime of polynomial/surface range estimation and subdivision.

**Numerical properties / cost.** AA enclosures are tight near-linearly but the affine form **grows** (new noise symbol per nonlinear op), so periodic **condensation** (merging small terms into a single interval term) is needed to bound cost; AA is therefore more expensive per operation and more complex to implement than intervals. **Taylor models** (a constant/polynomial part plus an interval remainder, Makino-Berz) generalize AA to higher order for even tighter enclosures of smooth functions at still higher cost.

**Kernel relevance.** Affine arithmetic is the **range-estimation tool for robust subdivision and validated marching** (kernel/35): computing a tight enclosure of a Bezier/NURBS patch's position and normal over a parameter box, for surface-surface intersection subdivision, for offset/bisector/medial-axis robustness (the de Figueiredo-Stolfi application domain), and for guaranteeing a subdivision cell contains no root before discarding it. Keel implements AA as an **opt-in tier above intervals**: intervals are the cheap default; AA is invoked where the dependency problem makes intervals uselessly wide (long chained spline evaluations, intersection enclosures). Taylor models are noted as a future tier, not a v1 commitment.

---

## 7. Robust Newton and nonlinear solving

Intersection, projection/point-inversion, and the nonlinear fitting/surfacing energies are all **nonlinear root-finding or minimization**, where plain Newton can diverge, oscillate, or hit a singular Jacobian.

### 7.1 Globalized/safeguarded Newton for intersection and projection

**Citation.** Nocedal, J., & Wright, S. J. (2006). *Numerical Optimization* (2nd ed.). Springer. (Line search Ch. 3, trust region Ch. 4, Gauss-Newton/Levenberg-Marquardt Ch. 10.) ; geometric specifics: Patrikalakis, N. M., & Maekawa, T. (2002). *Shape Interrogation for Computer Aided Design and Manufacturing*. Springer.

**Method.** A surface-surface intersection footpoint or a curve-surface intersection is solved by Newton on the residual system `F(u,v,s,t) = S1(u,v) - S2(s,t) = 0` (or point-inversion `min ||S(u,v) - p||`). Plain Newton `x_{k+1} = x_k - J^{-1} F(x_k)` converges quadratically **near** a root but is unreliable far away. **Globalization** makes it robust: (a) **line search** scales the Newton step by `alpha in (0,1]` chosen (backtracking, Armijo condition) so the residual norm actually decreases, preventing overshoot; (b) **trust region** restricts the step to a ball where the local model is trusted, shrinking the ball on poor agreement, which **handles a singular or near-singular Jacobian gracefully** (the trust-region subproblem stays well-posed even when `J` is rank-deficient, unlike `J^{-1}`). For the geometry case the Jacobian columns are the surface partials `S_u, S_v` (hodographs, Section 5.2, available analytically), and **a singular Jacobian signals tangency** (the surfaces touch rather than cross), which the geometric layer must detect and handle specially rather than push the numerics through, the non-iterative SVD/eigenvalue methods of Xiao et al. are an alternative there.

### 7.2 Continuation/homotopy for hard starts

When no good initial guess is available (the global "find all intersection branches" problem), **continuation/homotopy** deforms an easy problem with a known solution into the target problem, tracking the solution along the way. In intersection this is the **marching** scheme: find one point on the intersection curve, then trace the curve by predictor-corrector steps (Euler predictor along the tangent `t = n1 x n2`, Newton corrector back onto both surfaces), which is homotopy in the arc-length parameter. Robust marching uses **validated/interval ODE** steps (kernel/35) to guarantee the traced curve stays on the true intersection and to step over near-tangencies.

### 7.3 Gauss-Newton and Levenberg-Marquardt for nonlinear fitting and energies

**Citation.** Nocedal & Wright (2006), Ch. 10; Marquardt, D. W. (1963). An algorithm for least-squares estimation of nonlinear parameters. *SIAM J. Appl. Math., 11*(2), 431-441.

**Method.** Nonlinear least squares `min sum ||r_i(x)||^2` (fitting a NURBS to data with the **footpoint parameters unknown**, the nonlinear geometric-energy surfacing of kernel/32) is solved by **Gauss-Newton**: approximate the Hessian by `J^T J` (dropping the second-derivative term, valid near a good fit with small residuals), and solve the linear least-squares step `J^T J dx = -J^T r` each iteration (itself a Section 3 solve, by QR for stability or Cholesky on the normal-equations form when well-conditioned). Gauss-Newton **fails when `J^T J` is singular or the residuals are large**. **Levenberg-Marquardt (LM)** fixes this by solving the **damped** system `(J^T J + mu I) dx = -J^T r`: the damping `mu` interpolates between Gauss-Newton (`mu -> 0`, fast quadratic-ish convergence near the solution) and gradient descent (`mu` large, small safe steps far away), and is adapted up/down by a trust-region-style ratio test of predicted-vs-actual reduction. LM is the **modern view as a trust-region method** and is the robust default for nonlinear geometric fitting because it tolerates rank-deficient Jacobians (the `+ mu I` lifts the singularity exactly as Tikhonov lifts the surfacing system, Section 3.3) and globalizes automatically.

**Numerical properties / convergence.** Gauss-Newton converges quadratically for zero-residual problems and linearly otherwise; LM is **globally convergent to a stationary point** under mild conditions and degrades gracefully on rank deficiency. The damped solve is always SPD, so each LM step uses the friendly Cholesky path.

**Kernel relevance.** **Levenberg-Marquardt is the default nonlinear solver for fitting with free footpoints and for the nonlinear surfacing energies** (kernel/32-23); globalized (line-search or trust-region) Newton is the default for the **square** intersection/projection root systems; both pull the Jacobian from analytic hodographs and fall back to AD for composed residuals; both lean on the Section 9 conditioning checks and the Section 2 linear solvers for their inner step.

---

## 8. Quadrature: the integration substrate

Mass properties (kernel/22), energy functionals (kernel/32 fairness), and arc-length/area computations are integrals over Bezier/NURBS spans.

### 8.1 Gauss-Legendre on Bezier spans

The default is **Gauss-Legendre quadrature**: an `n`-point Gauss rule integrates polynomials up to degree `2n-1` exactly, so a degree-`p` integrand (a product of spline basis functions, or `position x normal` for a surface integral) is integrated **exactly** by `ceil((p+1)/2)` points **per polynomial span**. The kernel integrates **span by span** (between knots), because the spline is only `C^{p-k}` across knots and Gauss-Legendre assumes smoothness within the panel; integrating across a knot with a single rule loses the exactness. This per-span Gauss rule is the bread-and-butter integrator.

### 8.2 Smoothness-aware quadrature: the IGA half-point rule

**Citation.** Hughes, T. J. R., Reali, A., & Sangalli, G. (2010). Efficient quadrature for NURBS-based isogeometric analysis. *Computer Methods in Applied Mechanics and Engineering, 199*(5-8), 301-313. https://www.oden.utexas.edu/media/reports/2008/0827.pdf

**Method / finding.** Standard Gauss-Legendre applied span-by-span **wastes points** because it ignores the inter-element **smoothness** of the spline space: a maximally-smooth (`C^{p-1}`) spline basis has far fewer independent degrees of freedom than `p+1` per span, so fewer quadrature points suffice for exact integration of the mass/stiffness entries. Hughes, Reali, and Sangalli show that **optimal rules use a number of points roughly equal to half the number of degrees of freedom** (the "**half-point rule**"), independent of polynomial order, yielding up to a **5x reduction** in quadrature points in 3D versus full Gauss-Legendre for even-degree rules (and another 2x via odd-degree reduced integration). Later work (Calabro, Sangalli, Tani; Barton, Calo) gives explicit optimal/generalized-Gauss rules tailored to the knot vector.

**Kernel relevance.** For the **fairness/energy assembly** (kernel/32), where the same integrals are computed over a whole control net many times during interactive editing, the IGA half-point rule **cuts quadrature cost several-fold** at no accuracy loss, which matters for interactivity. For one-off mass properties, plain per-span Gauss-Legendre is simpler and fine; the half-point rule is the optimization reserved for the hot assembly loop.

### 8.3 Adaptive quadrature and divergence-theorem surface integrals

For **non-polynomial** integrands (rational NURBS, where the weight denominator makes the integrand non-polynomial, or trimmed regions, or energy densities involving `sqrt` of the metric), no fixed Gauss rule is exact, so the kernel uses **adaptive quadrature**: subdivide the panel, apply Gauss on each subpanel, compare a higher-order and lower-order estimate (Gauss-Kronrod gives the error estimate cheaply by reusing the Gauss points), and refine where the local error exceeds tolerance. **Mass properties** (volume, centroid, inertia, kernel/22) are computed via the **divergence theorem**, converting the volume integral to a **surface integral** over the B-rep faces: `Volume = (1/3) integral_{boundary} (x . n) dA`, and similarly the centroid and inertia moments are surface integrals of polynomial-in-position integrands against the outward normal. Each trimmed face's surface integral is then a 2D quadrature over the face's parameter domain (adaptive across the trim boundary), summed over the shell. This is why the surface integrator must handle trimmed domains and rational integrands robustly, and why adaptive Gauss-Kronrod with a relative-error tolerance (Section 9) is the workhorse for mass properties.

---

## 9. Conditioning, scaling, and normalization discipline

The cheapest robustness win in a kernel is **not** a cleverer algorithm but **good scaling**.

**Citation.** Higham, N. J. (2002). *Accuracy and Stability of Numerical Algorithms* (2nd ed.). SIAM. ; Trefethen, L. N., & Bau, D. (1997). *Numerical Linear Algebra*. SIAM.

**Method / discipline.**
- **Normalize the model to a unit box.** The kernel already normalizes coordinates into a bounded box (tying to the tolerance model): this keeps all magnitudes `O(1)`, so absolute and relative tolerances coincide, exponent ranges stay tame, and the `eps`-scaled singularity tests of Section 1 are meaningful. Working with raw millimeter coordinates that span `10^6` would silently waste digits.
- **Scale the variables in a solve.** When a Newton/least-squares system mixes quantities of very different magnitude (a parameter `u in [0,1]` against a coordinate in model units, or position against angle), **column/row scaling** (equilibration) of the Jacobian before factoring brings the condition number down, often by orders of magnitude. The fitting and intersection Jacobians should be equilibrated.
- **Backward error and the condition number.** Higham's framework: a computed solution with small **backward error** is the exact answer to a slightly perturbed problem; the **forward error** is bounded by `condition number x backward error`. So even a backward-stable solver (QR, Cholesky) gives an inaccurate answer when the problem is ill-conditioned, the kernel must *detect* this, not just compute. Higham notes Cramer's rule (Section 1.1) is the classic example of a method that is forward-stable but not backward-stable, a caution the small-matrix path heeds.
- **Estimate and report ill-conditioning.** Rather than the expensive exact condition number, use a **condition-number estimator** (the LAPACK-style `1`-norm estimator of Hager-Higham, or the smallest singular value from the factorization, or the ratio of diagonal magnitudes in the LU/Cholesky factor) to get `kappa` cheaply after factoring. When `kappa` exceeds a threshold (e.g. `1/sqrt(eps) ~ 10^8` for "half the digits lost"), the solver **reports ill-conditioning to the caller** (the surfacing layer adds regularization or reduces degrees of freedom; the intersection layer flags a near-tangency; the predicate cascade escalates to exact arithmetic).
- **Relative versus absolute error.** Convergence tests, quadrature tolerances, and singularity gates should be **relative** (scaled by the magnitudes involved) wherever a meaningful scale exists, with an absolute floor near zero to avoid division blow-up, the standard `|err| <= rtol * |x| + atol` discipline.

**Kernel relevance.** Conditioning discipline is a **cross-cutting policy of `keel-math`, not a single function**: unit-box normalization (shared with tolerance), Jacobian equilibration in every solver, a cheap post-factorization condition estimate exposed on every factorization object, a documented ill-conditioning threshold that triggers caller-visible escalation, and relative-plus-absolute tolerances throughout. This is what keeps the kernel out of the silent-garbage regime.

---

## 10. Special-function and transcendental accuracy

**Citation.** Daramy-Loirat, C., Defour, D., de Dinechin, F., et al. CR-LIBM, a library of correctly-rounded elementary functions. https://ens-lyon.hal.science/ensl-01529804v1/document ; the CORE-MATH project, Sibidanov, Zimmermann, et al., https://core-math.gitlabpages.inria.fr/ .

**Method / properties.** A small number of kernel computations are sensitive to the **last bit** of `sin`, `cos`, `atan2`, `sqrt`, `exp`, `log`: analytic surface evaluation (a torus/sphere uses trig of the parameters), exact angle computations (dihedral angles, the `atan2` in the 2x2 eigenvector of Section 4.1), and arc-length. The danger is twofold: (a) a sloppy `libm` loses accuracy in the rare hard-to-round cases, and (b) more importantly for a kernel, **different platforms' `libm` implementations differ in the last bits**, breaking the bit-for-bit **determinism** the kernel promises (kernel/14): the same model evaluated on two machines could take different branches in a tolerance test if `sin` differs in the last ULP. **Correctly-rounded** libraries, CR-LIBM and the newer **CORE-MATH** (which targets glibc, Intel, AMD math libraries) and the **RLIBM** project, guarantee the result is the correctly-rounded (to-nearest, or any IEEE mode) value of the infinitely-precise function, so they are both **maximally accurate and identical across platforms**. CORE-MATH verifies correct rounding for univariate `binary32` exhaustively and for `binary64` against known hard cases.

**Kernel relevance.** For **determinism** (kernel/14) more than for accuracy per se, `keel-math` should route the kernel's transcendental calls through a **correctly-rounded, platform-independent libm** (a CORE-MATH-derived implementation, or a vendored deterministic libm) rather than the system `libm`, so that a tolerance comparison downstream of a `sin`/`atan2`/`sqrt` is reproducible on every target. `sqrt` is already correctly rounded by IEEE-754 hardware (and is the one transcendental-ish operation that is portable for free); the trig/exp/log family is where a deterministic libm earns its place. This is a determinism investment, the accuracy is a bonus.

---

## 11. Numerical substrate for Keel: synthesis

**What `keel-math` owns versus wraps.**
- **Owns (for epsilon control and determinism):** the small fixed-size dense LA (`Mat2..Mat6`: FMA-fused Cramer with scaled singularity gates and cross-product 3x3 plane forms, escalating to pivoted LU / 2x2-3x3 SVD); the **interval and affine arithmetic** types (the predicate-cascade and validated-marching substrate); the **robust symmetric 2x2/3x3 eigensolver** (curvature, PCA/pose, inertia, one well-tested routine); the **companion-matrix polynomial root finder with balancing**; the **globalized Newton / Gauss-Newton / Levenberg-Marquardt** nonlinear solvers; the **Gauss-Legendre, IGA-half-point, and adaptive Gauss-Kronrod quadrature**; the **conditioning discipline** (unit-box normalization, Jacobian equilibration, post-factorization condition estimate, relative+absolute tolerances); and the **deterministic correctly-rounded libm** routing.
- **Wraps (insane to reimplement, clean boundary):** the **supernodal sparse Cholesky** and **sparse rank-revealing QR** for the surfacing/fitting/variational systems. First choice is **faer** (pure Rust, deterministic, single-language build, competitive with Eigen), with a **CHOLMOD/SuiteSparseQR FFI behind a feature flag** as a hardened fallback and development oracle.

**The sparse-solver choice for surfacing.** **Sparse direct factorization is the default**, justified by Botsch-Bommes-Kobbelt (direct beat even tuned multigrid at these sizes, and factor reuse makes interactive re-solves cheap): SPD fairness systems `(N^T W N + lambda K)` go through **sparse Cholesky** (no pivoting needed, unconditionally stable); the **symbolic factorization is reused** across interactive re-solves with a fixed sparsity pattern; **update/downdate** handles single-constraint edits. The **indefinite KKT/saddle** systems are reduced to SPD by **nullspace/Schur elimination of the hard constraints** and then Cholesky, with **MINRES** as the fallback for large KKT systems. Iterative/multigrid is reserved for unusually large volumetric systems.

**QR versus normal equations policy.** **Default to regularized normal equations + Cholesky** because the fairness term `lambda K` is a Tikhonov regularizer that makes `N^T W N + lambda K` well-conditioned and SPD, dissolving the condition-squaring concern. **Escalate to sparse QR** when there is little/no regularization or when the post-factorization condition estimate is poor: QR avoids squaring `kappa(N)` and is rank-revealing, so it both solves stably and reports redundant degrees of freedom. Reserve dense **SVD/pseudoinverse** for the small solves where exact rank is needed (pose/PCA, degenerate-Jacobian intersection steps), taking the **minimum-norm solution** for the under-determined surfacing case.

**AD versus analytic policy.** **Analytic hodograph derivatives for spline curve/surface evaluation** (exact, cheaper, reuse de Boor, deterministic, auditable). **Forward-mode dual-number AD (num-dual style) for the Jacobians of assembled/composed constraint-intersection-projection systems** where hand-deriving is error-prone, with second-order duals for general curvature. Keep AD as a **correctness oracle** that the analytic derivatives are differenced against in tests.

**Interval-arithmetic plan.** **Intervals (round-to-nearest with ULP widening) as the fast, portable, deterministic default** for the predicate-cascade filter and certified root isolation, with an **MPFR-backed exact mode** for the hardest predicate cases (handing off to the exact arithmetic kernel/11 already specifies). **Affine arithmetic as an opt-in tier above intervals** for range estimation in robust subdivision, surface-surface intersection enclosures, and offset/medial-axis robustness, where the dependency problem makes plain intervals uselessly wide. Taylor models noted as a future tier.

**Conditioning discipline.** Unit-box normalization shared with the tolerance model; **equilibrate every Jacobian** before factoring; expose a **cheap condition estimate on every factorization object** and trip a **documented ill-conditioning threshold** (`~1/sqrt(eps)`) that escalates to the caller (regularize, reduce DOF, flag tangency, or go exact); **relative-plus-absolute tolerances** everywhere. This discipline, more than any single algorithm, is what keeps the kernel's numerics trustworthy.

---

## References

1. Chen, Y., Davis, T. A., Hager, W. W., & Rajamanickam, S. (2008). Algorithm 887: CHOLMOD, supernodal sparse Cholesky factorization and update/downdate. *ACM Transactions on Mathematical Software, 35*(3), Article 22. https://dl.acm.org/doi/10.1145/1391989.1391995
2. Davis, T. A. (2011). Algorithm 915, SuiteSparseQR: Multifrontal multithreaded rank-revealing sparse QR factorization. *ACM Transactions on Mathematical Software, 38*(1), Article 8. https://dl.acm.org/doi/10.1145/2049662.2049670
3. Botsch, M., Bommes, D., & Kobbelt, L. (2005). Efficient linear system solvers for mesh processing. In *Mathematics of Surfaces XI* (LNCS 3604, pp. 62-83). Springer. https://www.graphics.rwth-aachen.de/media/papers/solvers1.pdf
4. El Kazdadi (Quinones), S. (2024). faer: A linear algebra library for the Rust programming language. *Journal of Open Source Software* (review #6099). https://github.com/sarah-quinones/faer-rs ; paper: https://github.com/sarah-quinones/faer-rs/blob/main/paper.md
5. Higham, N. J. (2002). *Accuracy and Stability of Numerical Algorithms* (2nd ed.). SIAM. https://nhigham.com/2020/08/04/what-is-numerical-stability/
6. Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.). Johns Hopkins University Press.
7. Trefethen, L. N., & Bau, D. (1997). *Numerical Linear Algebra*. SIAM.
8. Hansen, P. C. (1998). *Rank-Deficient and Discrete Ill-Posed Problems*. SIAM.
9. Smith, O. K. (1961). Eigenvalues of a symmetric 3x3 matrix. *Communications of the ACM, 4*(4), 168.
10. Eberly, D. *A Robust Eigensolver for 3x3 Symmetric Matrices*. Geometric Tools. https://www.geometrictools.com/Documentation/RobustEigenSymmetric3x3.pdf
11. Habera, M., et al. (2026). Numerically stable evaluation of closed-form expressions for eigenvalues of 3x3 matrices. *Numerical Algorithms*. https://link.springer.com/article/10.1007/s11075-026-02328-5
12. Edelman, A., & Murakami, H. (1995). Polynomial roots from companion matrix eigenvalues. *Mathematics of Computation, 64*(210), 763-776. https://www.osti.gov/biblio/181773
13. Parlett, B. N., & Reinsch, C. (1969). Balancing a matrix for calculation of eigenvalues and eigenvectors. *Numerische Mathematik, 13*, 293-304.
14. Griewank, A., & Walther, A. (2008). *Evaluating Derivatives: Principles and Techniques of Algorithmic Differentiation* (2nd ed.). SIAM.
15. num-dual crate (forward-mode dual-number AD in Rust). https://docs.rs/num-dual ; ad-trait survey (2025). https://arxiv.org/html/2504.15976v1
16. Moore, R. E., Kearfott, R. B., & Cloud, M. J. (2009). *Introduction to Interval Analysis*. SIAM. ; IEEE Std 1788-2015 / 1788.1-2017 (interval arithmetic).
17. inari crate (IEEE 1788 interval arithmetic in Rust). https://github.com/unageek/inari
18. Comba, J. L. D., & Stolfi, J. (1993). Affine arithmetic and its applications to computer graphics. *SIBGRAPI '93*.
19. de Figueiredo, L. H., & Stolfi, J. (2004). Affine arithmetic: Concepts and applications. *Numerical Algorithms, 37*(1-4), 147-158. https://lhf.impa.br/ftp/oral/aa.pdf
20. Nocedal, J., & Wright, S. J. (2006). *Numerical Optimization* (2nd ed.). Springer.
21. Marquardt, D. W. (1963). An algorithm for least-squares estimation of nonlinear parameters. *SIAM Journal on Applied Mathematics, 11*(2), 431-441.
22. Patrikalakis, N. M., & Maekawa, T. (2002). *Shape Interrogation for Computer Aided Design and Manufacturing*. Springer.
23. Xiao, S., et al. (2019). A non-iterative method for robustly computing the intersections between a line and a curve or surface. *International Journal for Numerical Methods in Engineering*. https://arxiv.org/abs/1902.01814
24. Hughes, T. J. R., Reali, A., & Sangalli, G. (2010). Efficient quadrature for NURBS-based isogeometric analysis. *Computer Methods in Applied Mechanics and Engineering, 199*(5-8), 301-313. https://www.oden.utexas.edu/media/reports/2008/0827.pdf
25. Daramy-Loirat, C., Defour, D., de Dinechin, F., et al. CR-LIBM: a library of correctly-rounded elementary functions. https://ens-lyon.hal.science/ensl-01529804v1/document
26. Sibidanov, A., Zimmermann, P., et al. The CORE-MATH project (correctly-rounded math functions). https://core-math.gitlabpages.inria.fr/
