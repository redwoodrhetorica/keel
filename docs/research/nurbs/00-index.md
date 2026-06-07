# NURBS Literature Research: Index

Deep literature research on NURBS and related geometry and mathematics, application-leaning with full theory coverage. This extends the earlier survey in `docs/research/b-nurbs-freeform.md` (Track B) from one-paragraph takeaways to full findings: per-source citations, explicit algorithms and equations, reported results, limitations, and kernel relevance. Roughly 200 cited sources across six files.

## Files

| File | Scope | Sources |
|---|---|---|
| [01-foundations-theory.md](01-foundations-theory.md) | B-spline/NURBS mathematics: Schoenberg origins, Cox-de Boor recurrence, Curry-Schoenberg and Schoenberg-Whitney theorems, basis properties, knot vector theory, Versprille's rational extension and projective geometry, conic/quadric representation, parametric vs geometric continuity, derivatives, blossoming, total positivity, basis conditioning | ~33 |
| [02-core-algorithms.md](02-core-algorithms.md) | Evaluation (de Boor, de Casteljau, rational derivatives), numerical stability literature (Farouki-Rajan, Mainar-Pena, compensated de Casteljau), knot insertion (Boehm, Oslo), knot removal (Lyche-Morken), degree elevation/reduction, Bezier extraction, basis conversion conditioning, library and GPU implementation studies | ~34 |
| [03-fitting-interpolation.md](03-fitting-interpolation.md) | Interpolation and approximation: parameterization (chord, centripetal, Foley-Nielson), knot placement, least squares with error-bounded refinement, P-spline penalties, lofting/skinning and the knot-merge explosion, multilevel B-splines, LSPIA/PIA family, reverse engineering, fairing, ML-adjacent fitting (NURBS-Diff, SplineGen) | ~36 |
| [04-intersection-projection.md](04-intersection-projection.md) | Curve-curve, curve-surface, and surface-surface intersection in algorithmic depth: implicitization and resultants, Bezier/cubic-hybrid clipping, subdivision solvers, marching with step control, loop detection (Sederberg-Meyers, Hohmeyer normal cones), validated interval-ODE tracing, topology-guaranteed SSI (TOG 2023), point projection and inversion, ray-NURBS, distance computation | 34 |
| [05-offsets-blends-sweeps.md](05-offsets-blends-sweeps.md) | Constructed surfaces: offset non-rationality and approximation methods, Pythagorean-hodograph curves, rational canal surfaces, blending/filleting taxonomy (Vida-Martin-Varady), rotation-minimizing frames and sweeps, skinning, developables, shelling, medial axis connections, toolpath applications | ~34 |
| [06-applications-extensions.md](06-applications-extensions.md) | IGA in depth, spline extensions (T-splines and the linear independence trap, LR B-splines, THB-splines, U-splines), trimmed tessellation and GPU rendering, STEP/IGES data exchange and healing, volumetric V-reps, industrial domains (ship hull, Class-A, aerospace, turbomachinery, architecture), ML on B-reps, open-source ecosystem survey | 38 |

## Headline findings across the collection

1. **Store in homogeneous 4D, run non-rational algorithms, divide last.** Confirmed across The NURBS Book and the MIT hyperbook as the single most leverage-giving architectural decision for a NURBS kernel (file 01).
2. **Basis conditioning degrades with degree, not knot non-uniformity** (kappa < p * 2^p, knot-independent). Argues for capping working degree and preferring per-span Bezier extraction for delicate numerics (files 01, 02).
3. **Three-tier SSI architecture** is the convergent recommendation of the 2023-2025 literature: Hohmeyer normal-cone loop detection for branch enumeration, fast marching generically with validated interval-ODE enclosure only near critical regions, then Hausdorff-bounded NURBS fitting of the result (file 04).
4. **Cubic hybrid clipping (order-4 convergence)** materially beats classic Bezier clipping at CAD tolerances and should be the default univariate root engine (file 04).
5. **A constant-radius fillet is a trimmed canal surface**: the natural exact representation for fillets, sweeps, and analytic faces is procedural (spine + radius + trimlines) with NURBS as a cache (file 05).
6. **Offset self-intersection traces the medial axis**, giving a free predictor of safe shell thickness and topology-change events; one shared self-intersection-and-trim core can serve 2D offset, surface offset, shelling, and fillet spines (file 05).
7. **One linear-system engine covers fitting, fairing, and smoothing**: P-spline penalty, energy fairing, and curvature smoothing all reduce to (N^T N + lambda K) P = N^T Q; LSPIA is provably gradient descent on the same objective, bridging to differentiable-NURBS ML layers (file 03).
8. **Lofting's knot-merge explosion** is the dominant surface-construction risk; Park-style common-knot reduction and approximate skinning are the mitigations (files 03, 05).
9. **Generalized winding numbers for trimmed-NURBS containment** (Spainhour & Weiss, TOG 2025) is the modern robust answer to point-in-solid on leaky geometry, with a BSD-licensed implementation in LLNL Axom available for reuse (file 06).
10. **The linear independence trap**: T-spline blending functions can be linearly dependent (Buffa 2010); any local-refinement basis must enforce analysis-suitability as a checkable invariant. THB-splines are safe by construction; U-splines are patent-encumbered (file 06).
11. **The OSS gap is precise**: OpenCASCADE is the only full open kernel; SISL, OpenNURBS, verb, geomdl, and tinynurbs are geometry libraries lacking the solid-modeling tier. That tier is exactly Keel's opening (file 06).

## Verification caveats

The research agents could not fetch several paywalled or scanned-image PDFs (ACM, ScienceDirect, some 1990s scans). In those cases algorithmic detail was reconstructed from accessible mirrors, the MIT shape interrogation hyperbook, abstracts, and established domain knowledge. All citations (author, title, venue, year) were verified via search, but before copying any formula into kernel code, spot-check it against the primary source, in particular:

- Exact constants in older scanned intersection papers (file 04)
- Piegl-Tiller algorithm details cited by number, e.g. A4.2 (file 02)
- Page numbers for a few secondary entries (files 02, 03)
- Formulas in file 01 should be checked against The NURBS Book before implementation

## Relationship to prior research

`docs/research/b-nurbs-freeform.md` remains the executive survey with ADOPT/AVOID/INVESTIGATE guidance. These six files are the supporting depth: when an item in Track B is promoted to implementation, the corresponding file here holds the algorithmic detail and the citation trail.
