# Canonical Geometry Recovery: Recognizing Analytic Geometry Hidden in Splines

Research file 24 for the Keel kernel. This file consolidates the literature on **canonical geometry recovery**, also called entity simplification or geometry rationalization: deciding whether a B-spline curve or surface is "really" a line, arc, circle, ellipse, plane, cylinder, cone, sphere, torus, surface of revolution, extrusion, ruled surface, or helical surface, and if so, replacing it with the exact analytic primitive plus a certificate that the replacement is within tolerance.

## Why this matters and where it surfaces

The question "is this B-spline actually a cylinder?" recurred as a loose end across three prior Keel research tracks:

1. **Import healing.** Translators routinely splinify analytic geometry. IGES in particular lacks rich entity coverage, so many exporters emit even a plain cylinder as a rational or non-rational B-spline surface. Systems that internally splinify everything (some legacy and surface-modeling-first systems) erase analytic intent on the way out. A healing pipeline therefore needs a "simplify" phase that recovers analytics. This is documented behavior in ACIS HEAL, CADfix, and OCCT.

2. **Feature recognition.** Machining-feature and design-feature recognition keys off exact surface type. A hole is a cylinder, a chamfer is a plane or cone, a fillet is a constant-radius blend. If the face geometry is an opaque spline, the recognizer is blind. Type classification must precede feature semantics.

3. **Post-translation design intent.** Booleans, blends, draft, shelling, and intersection code all have special-case fast paths and exact intersection curves for analytic pairs (plane/plane = line, cylinder/plane = ellipse or line pair, two coaxial cylinders, etc.). When everything is a spline, those special cases are lost: intersections fall back to slow, less robust marching solvers, and the results are spline approximations rather than exact circles and lines, which compounds across operations.

The recovery problem decomposes into four sub-problems that this file treats in turn: **recognize** (what type is this?), **fit** (what are the best parameters?), **certify** (is the swap within tolerance?), and **substitute** (swap the geometry while keeping topology valid). A fifth concern, **constraint-aware recovery** (making nearly-coaxial recovered cylinders exactly coaxial), ties recovery to beautification.

---

## Theme 1: Industrial reality of the simplify phase

### Spatial / 3D ACIS Modeler: HEAL and the geometry simplification phase

**Citation.** Spatial Corporation. (n.d.). *3D ACIS Modeler: HEAL component documentation* (Healing R10, Chapter 1: Healing Component). Spatial Corp. Also: Spatial Corp. (n.d.). *Healing and preserving 3D model intent* and *How 3D ACIS Modeler improves the quality of translated 3D data* [Blog posts]. Retrieved from blog.spatial.com.

**Method.** ACIS HEAL detects and corrects accuracy problems in imported geometry in three ordered phases: (1) **stitching**, which restores topological completeness by unifying coincident edges and vertices so faces form continuous sheets or solids; (2) **geometry simplification**, which replaces spline surfaces with their corresponding analytic forms (plane, cylinder, cone, sphere, torus) wherever possible; and (3) **gap tightening / precision control**, which resolves small inaccuracies between adjacent faces by recomputing intersections within tolerance. The documentation is explicit that imported models "often contain surfaces that appear analytic when displayed but are actually represented as splines," especially from IGES "where even simple shapes are converted into spline approximations." HEAL analyzes the input and *suggests appropriate tolerances*, and exposes user-controllable tolerances. Simplification is justified on three grounds: it preserves design intent (a translated cylinder that became "two planar faces plus a spline" is restored to true analytic form), it reduces data size, and it improves robustness of downstream booleans, filleting, and feature recognition, all of which "place strong demands on topological consistency and geometric continuity" and fail more often on hidden splines.

**Limitations.** The phase ordering matters: simplification runs after stitching (you need correct topology to know which faces to test) but the gap-tightening pass afterward implies simplification alone does not guarantee a watertight analytic model. Documentation is behavioral, not algorithmic: it does not publish the recognition tests or fitting math. Tolerance is a single global knob (with auto-suggestion), so per-feature control is limited.

**Kernel relevance.** This is the canonical (pun intended) industrial reference for *where* simplify runs in a healing pipeline and *why*. Keel should mirror the ordering: stitch first, simplify second, tighten third. The "appears analytic but is a spline" framing and the IGES motivation are exactly Keel's import-healing use case.

### CADfix (ITI, a Wipro company): geometry rationalization

**Citation.** ITI / Wipro. (n.d.). *CADfix: CAD simplification and defeaturing for CAE* and *CAD model simplification* [Product documentation]. iti-global.com. See also CADinterop CADfix DX / PPS documentation, cadinterop.com.

**Method.** CADfix is a CAD-to-CAE translation, healing, and simplification toolkit. Its rationalization explicitly includes "simplification of rational and non-rational B-spline curves to analytic curves (line, circle, ellipse)" and "simplification of swept and spun surfaces to analytic surfaces (plane, cylinder, sphere, cone, torus)." It reads and writes IGES, STEP, ACIS, Parasolid, and others, repairs poor-quality geometry, and defeatures for FEA/CFD/CEM. The notable detail is the explicit call-out of **swept and spun surfaces** as inputs to rationalization, indicating CADfix recognizes surfaces of revolution and extrusions, not only the five standard quadric/torus analytics.

**Limitations.** Commercial, closed. The CAE orientation means rationalization is tuned for downstream meshing and defeaturing rather than exact CAD round-trip, and tolerances are chosen with analysis, not exact modeling, in mind.

**Kernel relevance.** Confirms that production rationalization covers both the analytic-quadric family *and* swept/spun surfaces, and that **curve** simplification (spline to line/circle/ellipse) is a first-class part of the job, not an afterthought. Keel's recovery service should treat curve recovery and surface recovery as peers.

---

## Theme 2: Exact recognition from B-spline structure

The forward direction is classical: Piegl and Tiller's *The NURBS Book* shows how to represent conics, circles, quadrics, and surfaces of revolution *exactly* as rational B-splines via projective images of piecewise quadratics, with specific weight patterns (for a circular arc, the interior weight is cos(half the segment angle)). The inverse direction (given the rational B-spline, recover the analytic) exploits the same structure.

### Piegl & Tiller: exact NURBS representations (forward) and recognition for offsets (inverse)

**Citation.** Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer. See also Piegl, L., & Tiller, W. (1999). Computing offsets of NURBS curves and surfaces. *Computer-Aided Design, 31*(2), 147-156.

**Method.** Forward: conics arise as rational quadratic Bezier/B-spline segments; a circle is a specific arrangement of control triangles with weight cos(theta/2) at the apex; spheres, cylinders, cones, and tori arise as surfaces of revolution of such conic generatrices, giving recognizable rational bi-quadratic (or quadratic-by-linear) control nets with characteristic weight grids. Inverse, in the offset paper: step 1 of the offset pipeline is "recognition of special curves and surfaces," and the authors note a surface is converted to analytic form "if such conversion introduces very little accuracy loss, for example no more than 0.001%." Crucially they reduce the 2D surface recognition problem to **two 1D curve recognition problems** (recognize the profile and the sweep direction independently), which is the structural shortcut for tensor-product surfaces.

**Limitations.** Exact-structure tests are brittle to reparameterization, knot insertion, degree elevation, and floating-point drift. A cylinder that has been knot-refined, reparameterized, or trimmed no longer matches the canonical control net even though it is geometrically the same surface. Pure structural matching therefore must be backed by a numerical fallback.

**Kernel relevance.** The "two 1D problems for one 2D surface" decomposition is directly implementable and cheap: recover the profile curve type and the rail/axis, then assemble (revolution of a line = cylinder/cone, revolution of a circle = sphere/torus, extrusion of a line = plane). Keel can use exact-structure detection as a fast first pass and fall back to numerical recognition when structure is obscured.

---

## Theme 3: Numerical recognition within tolerance (differential-geometric tests)

When structure is obscured, recognition becomes a sampling-and-classification problem keyed on local differential invariants, principally the two principal curvatures k1, k2.

### Benko & Varady: direct segmentation and surface-type classification

**Citation.** Benko, P., & Varady, T. (2002). Direct segmentation of smooth, multiple point regions. In *Proceedings of Geometric Modeling and Processing (GMP 2002)* (pp. 169-178). IEEE. And: Benko, P., & Varady, T. (2004). Segmentation methods for smooth point regions of conventional engineering objects. *Computer-Aided Design, 36*(6), 511-523.

**Method.** "Direct" segmentation classifies surface type from local differential invariants *before* committing to a fit, rather than the older grow-then-test loop. At each sample the principal curvatures (k1, k2) are estimated and the point is classified by their pattern: k1 = k2 = 0 implies plane; k1 = k2 = const != 0 implies sphere; one principal curvature zero (k2 = 0) implies a developable / cylinder or cone (cylinder if the nonzero curvature is constant along the ruling, cone if it varies linearly); both nonzero and unequal but with the right functional relationship implies torus. Cones versus cylinders are separated by whether the nonzero principal curvature is constant (cylinder) or varies (cone) along the parameter line. The 2004 paper extends this with robust estimators and translational/rotational tests for engineering objects. Region boundaries fall where the invariant signature changes.

**Limitations.** Principal-curvature estimation from sampled or noisy data is the hard part: second-derivative estimation amplifies noise, and near umbilics or flat regions the classification is unstable. Thresholds for "approximately zero" and "approximately constant" curvature couple directly to sampling density and noise.

**Kernel relevance.** This is the core numerical recognizer for Keel's simplify phase: sample the NURBS surface on a UV grid (Keel can sample exactly, with no scan noise, which removes the worst source of error), estimate (k1, k2) analytically from the spline (exact first and second fundamental forms, not finite differences), and classify. Sampling a *known analytic-or-spline* surface is far easier than scan segmentation because curvature is computed in closed form.

### Lukacs, Marshall & Martin: faithful least-squares fitting of quadrics and tori

**Citation.** Lukacs, G., Marshall, A. D., & Martin, R. R. (1998). Faithful least-squares fitting of spheres, cylinders, cones and tori for reliable segmentation. In *Computer Vision: ECCV'98* (LNCS 1406, pp. 671-686). Springer. (Companion technical report: Geometric least-squares fitting of spheres, cylinders, cones and tori.)

**Method.** Presents geometric (not algebraic) least-squares fits for sphere, cylinder, cone, and torus using distance functions and parameterizations chosen so the fit is **faithful**: as the principal curvatures of the data decrease or become more equal, the fitted surface degrades gracefully toward the correct "simpler" type (a near-flat cylinder tends toward a plane; a near-spherical torus toward a sphere) rather than diverging as radii blow up to infinity, which is the failure mode of naive parameterizations. The distance metric is a good approximation to true orthogonal (geometric) distance, making the residual meaningful as a deviation.

**Limitations.** Nonlinear, requires good initial estimates (supplied by the curvature-based recognition stage). Torus fitting in particular has many local minima. The faithful behavior helps recognition (the residual tells you when you are near a degenerate/simpler case) but type *decision* still needs a separate criterion.

**Kernel relevance.** This is Keel's **fitter**: once recognition proposes "cylinder," faithful least squares produces the optimal axis, radius, and (for cone) half-angle. The graceful-degradation property is exactly what a recover-or-keep decision wants: a meaningful residual that does not explode near boundaries between types. The residual feeds directly into certification.

### Gauss map analysis

**Citation.** (Cross-cutting result, see Pottmann/Wallner and Benko/Varady above.) The Gauss image (the set of unit normals) of a plane is a single point; of a cylinder, a great-circle arc (planar Gauss image through the origin); of a cone, a circle (planar but not through origin); of a surface of revolution, a region symmetric about an axis; of a sphere, a region of the full sphere.

**Method.** Map sampled surface normals onto the unit sphere and analyze the dimensionality and shape of the image. A 0-dimensional image (one point) is a plane; a 1-dimensional image lying on a great circle is a cylinder (the normal stays perpendicular to the axis); a 1-dimensional image on a non-great circle is a cone. This is a robust, parameterization-independent discriminator for the developable family.

**Limitations.** Needs reliable normals (easy for Keel, exact from the spline) and enough sampling to resolve the image dimension. Degenerate for very small patches where the Gauss image is tiny and dimension is ambiguous.

**Kernel relevance.** A cheap, robust complement to curvature classification, especially good at separating plane/cylinder/cone, and it is the conceptual bridge to the line-geometry methods below (which formalize "normals lie in a linear complex").

---

## Theme 4: Recognition of swept/ruled/kinematic structure via line geometry

The deepest and most elegant body of work, due largely to Pottmann and collaborators, classifies **kinematic surfaces** (cylinder, surface of revolution, helical surface, and their special cases) by the geometry of their *surface normals* treated as lines in projective line space.

### Pottmann & Wallner: line geometry and the linear complex characterization

**Citation.** Pottmann, H., & Wallner, J. (1999). An introduction to line geometry with applications. *Computer-Aided Design, 31*(1), 3-16. See also Pottmann, H., & Wallner, J. (2001). *Computational Line Geometry*. Springer.

**Method.** A line in 3-space is represented by 6 Plücker coordinates obeying the quadratic Plücker relation (the Klein quadric). A *linear line complex* is the set of lines whose Plücker coordinates satisfy one linear equation; geometrically it is the set of path-normals of a one-parameter rigid motion (a helical motion, with pure rotation and pure translation as limits). The central theorem: **the surface normals of a C1 surface lie in a linear line complex if and only if the surface is a cylinder, a surface of revolution, or a helical surface.** Fitting a linear complex to the sampled normal lines therefore both *recognizes* this family and *recovers its axis and pitch*: the complex's associated screw motion gives the axis direction, a point on the axis, and the pitch p. Pitch p = 0 means surface of revolution; p = infinity (pure translation) means general cylinder; finite nonzero p means helical surface.

**Limitations.** Requires a linear-complex fit to noisy normal data; the fit is an eigenproblem but the eigenvector must be projected back onto the Klein quadric to be a valid line/complex, costing some accuracy. Distinguishing "approximately revolution" from "approximately helical with tiny pitch" is numerically delicate.

**Kernel relevance.** This is the *unifying* recognizer for revolution/extrusion/helical surfaces: one linear-complex fit classifies all three plus recovers the axis. For Keel, exact normals from the spline make the fit clean. The pitch value is the single number that branches the substitution (revolution vs extrusion vs helix).

### Pottmann & Randrup: rotational and helical surface reconstruction

**Citation.** Pottmann, H., & Randrup, T. (1998). Rotational and helical surface approximation for reverse engineering. *Computing, 60*(4), 307-322.

**Method.** Concretizes the above for reverse engineering. The axis is found as the line "as coplanar as possible" with the surface normal lines: each face/sample contributes a normal line through its center, and the method finds the linear complex best approximating that set of lines under a line-geometric (Klein) metric. Computationally it forms a 6x6 symmetric matrix whose smallest nontrivial eigenvalues/eigenvectors give Plücker coordinates of candidate axes/complexes; the eigenvector is refined to satisfy the Plücker condition. From the recovered linear complex the axis, and (for helical) the pitch, are extracted, then a profile/meridian curve is fit in the rotated frame to complete the surface.

**Limitations.** Sensitive to normal-estimation error and to non-uniform sampling weighting (the chosen norm matters); requires the surface to actually be a kinematic surface for the complex fit to be meaningful (a free-form surface yields a poor fit, which is itself the "do not recover" signal).

**Kernel relevance.** Provides the concrete eigenvalue algorithm Keel implements for axis-finding. The residual of the linear-complex fit doubles as a recognition confidence: large residual = not a kinematic surface = keep the spline.

### Peternell & Pottmann: recognition and reconstruction of special surfaces from point clouds

**Citation.** Pottmann, H., Hofer, M., Odehnal, B., & Wallner, J. (2002/2004). Line geometry for 3D shape understanding and reconstruction. In *ECCV/Springer* (see Springer chapter, 2004). Related: Peternell, M., & Pottmann, H. work on developable surface fitting and "Recognition and reconstruction of special surfaces from point clouds," DMG TU Wien.

**Method.** Extends linear-complex recognition to a broader catalogue: rotational, helical, developable, and translational surfaces are each characterized by a configuration of their (locally intersecting) surface normals, analyzed in a modified Klein model of line space. A vector field tangent to the surface at every point exists exactly for the kinematic family; its character (rotation, translation, screw) again classifies the type. Developables are handled via the tangent-plane / dual (support-function) representation: a developable's tangent planes form a curve in the dual space, distinguishing it from doubly-curved surfaces.

**Limitations.** Developable and translational recognition add cases that are individually less robust than the rotation/cylinder core; the dual/support-function machinery is heavier to implement.

**Kernel relevance.** Rounds out recovery to cover developables and translational (extrusion-of-a-curve) surfaces, which a CAD kernel sees often (sheet metal, swept profiles). For Keel these are lower priority than the quadric family but matter for full coverage.

---

## Theme 5: Certification (is the swap within tolerance?)

Recognition + fitting yields a *candidate* analytic surface. Substitution is only legal if the maximum deviation between the original spline and the candidate is below tolerance. This is a one-sided Hausdorff / max-distance computation.

### Certified Hausdorff distance between freeform and analytic geometry

**Citation.** Kim, Y.-J., Oh, Y.-T., Yoon, S.-H., Kim, M.-S., & Elber, G. (2013). Efficient Hausdorff distance computation for freeform geometric models in close proximity. *Computer-Aided Design, 45*(2), 270-276. Related: Krishnamurthy, A., et al. GPU-accelerated Hausdorff distance between NURBS surfaces; and various minimum-distance-to-NURBS works.

**Method.** Computes a certified maximum deviation by adaptive subdivision of the NURBS surface combined with Newton iteration in flat-enough leaves, using curvature-based bounds to guarantee user-defined tolerance. Bounding the distance from a sampled-and-linearized surface to the true surface uses the maximum deviation of the linear approximation, itself curvature-bounded. The result is upper and lower bounds bracketing the true max deviation.

**Limitations.** Full certified Hausdorff is expensive; for a recover/keep decision a *guaranteed upper bound* suffices and is cheaper than the exact distance. Tight certification near tangential contact is the slow case.

**Kernel relevance.** Keel needs a *certified upper bound* on max deviation from spline to candidate analytic, not the exact value: if the upper bound is below tolerance, recover; otherwise keep the spline. The cheap version is dense sampling of the spline against the analytic's closed-form distance function (point-to-cylinder, point-to-sphere are trivial), refined by subdivision only where samples approach tolerance. This is much cheaper than spline-to-spline Hausdorff because the analytic side has a closed-form signed distance.

---

## Theme 6: Constraint-aware recovery (the beautification connection)

Recovering each face independently produces cylinders that are *nearly* coaxial, planes that are *nearly* parallel, axes that are *nearly* perpendicular. Design intent says they should be exactly so. This is the geometric-substitution side of beautification.

### Benko, Kos, Varady, Martin & Andor: constrained fitting in reverse engineering

**Citation.** Benko, P., Kos, G., Varady, T., Andor, L., & Martin, R. R. (2002). Constrained fitting in reverse engineering. *Computer Aided Geometric Design, 19*(3), 173-205.

**Method.** Simultaneously fits multiple analytic surfaces to data subject to constraints among their parameters: coaxiality, parallelism, orthogonality, equal radius, concentricity, fixed angles, and fixed-radius rolling-ball blends. Constraints may be auto-generated (typically over-generated, more than can be jointly satisfied) and the method includes a numerical scheme to detect and resolve conflicts, solving the consistent subset. Targets planes, cylinders, cones, spheres, tori, swept surfaces, and blends. The output is models accurate enough to import into a CAD system and faithful to engineering regularities.

**Limitations.** Constraint conflict resolution is heuristic (priorities decide which constraints win). Simultaneous nonlinear fitting is more expensive and more prone to local minima than independent fits.

**Kernel relevance.** After per-face recovery, Keel should run a *constrained re-fit*: detect approximate regularities among recovered primitives and snap them exactly (two cylinders within angular/positional tolerance become exactly coaxial). This produces clean, editable, history-friendly models instead of a pile of slightly-misaligned analytics. It directly enables exact special-case intersections downstream.

### Langbein, Marshall & Martin: beautification by consistent regularity selection

**Citation.** Langbein, F. C., Marshall, A. D., & Martin, R. R. (2004). Choosing consistent constraints for beautification of reverse engineered geometric models. *Computer-Aided Design, 36*(3), 261-278. Related: Langbein, F. C., Gao, C. H., Mills, B. I., Marshall, A. D., & Martin, R. R. (2004). Topological and geometric beautification of reverse engineered geometric models. *ACM Solid Modeling / EG SM 2004.*

**Method.** Detects geometric regularities approximately present in a B-rep (symmetries, parallelism, equal dimensions, special angles, alignments), then selects a *consistent* subset likely to reflect the original ideal design intent. Consistency/solvability is analyzed via degrees of freedom of the constraint system; priorities break ties when regularities conflict; the chosen constraint set is solved numerically and an improved model is rebuilt. The companion paper adds topological beautification (merging/splitting faces to match intended topology).

**Limitations.** The full regularity detection + selection is a large machine; over-aggressive beautification can impose intent that was not there. Constraint-system solvability analysis is intricate.

**Kernel relevance.** Beautification is the conceptual umbrella over constraint-aware recovery. For Keel the *geometric-substitution* slice is what matters here: detect candidate regularities among recovered analytics and enforce a consistent subset, with priorities and a do-not-over-constrain guard. (Constraint solving itself is covered in another Keel research file; this file owns the geometry-substitution input to it.)

---

## Theme 7: Reverse-engineering segmentation overlap (brief)

### Varady, Martin & Cox: reverse engineering of geometric models, an introduction

**Citation.** Varady, T., Martin, R. R., & Cox, J. (1997). Reverse engineering of geometric models: an introduction. *Computer-Aided Design, 29*(4), 255-268.

**Method.** The seminal survey of the RE pipeline: data acquisition, segmentation, surface fitting, model creation, with these stages overlapping. Introduces functional decomposition, ignore areas, and constrained fitting of free-form features. The segmentation-then-classify-then-fit structure is exactly the recognize-fit pipeline this file applies to splines rather than scans.

**Limitations.** A survey; predates the line-geometry and learned methods. Scan-oriented (noise, incompleteness) where Keel's input is clean exact geometry.

**Kernel relevance.** Confirms recovery shares its core machinery with RE: the difference is Keel's input is a noise-free spline, so segmentation is trivial (faces already exist) and classification/fitting are easier. Keel reuses the RE recognition tests minus the scan-noise headaches.

### Schnabel, Wahl & Klein: efficient RANSAC for primitive detection

**Citation.** Schnabel, R., Wahl, R., & Klein, R. (2007). Efficient RANSAC for point-cloud shape detection. *Computer Graphics Forum, 26*(2), 214-226.

**Method.** Randomized sampling detects planes, spheres, cylinders, cones, and tori in unorganized point clouds, decomposing the cloud into primitive proxies plus a remainder, robust to heavy noise and outliers, scaling to millions of points in under a minute. Minimal sample sets instantiate candidate primitives; a scoring function with localized sampling and a lazy score-update scheme makes it efficient.

**Limitations.** Stochastic (non-deterministic results), tuned for noisy scans; for clean spline-sampled data its robustness machinery is overkill and its non-determinism is undesirable for a kernel.

**Kernel relevance.** A fallback / cross-check, not the primary path: when structural and curvature methods are ambiguous, a deterministic-seeded RANSAC over densely sampled spline points can propose a primitive type to then verify exactly. Mostly relevant if Keel ever ingests meshes/scans.

---

## Theme 8: Modern and learned approaches (2018-2025)

### Jayaraman et al.: UV-Net, learning from boundary representations

**Citation.** Jayaraman, P. K., Sanghi, A., Lambourne, J. G., Willis, K. D. D., Davies, T., Shayani, H., & Morris, N. (2021). UV-Net: Learning from boundary representations. In *CVPR 2021* (pp. 11703-11712). Related: Lambourne, J. G., et al. (2021). BRepNet: A topological message passing system for solid models. *CVPR 2021.*

**Method.** Represents each B-rep face as a 2D grid of points/normals/visibility sampled on its UV parameter domain, runs 2D CNNs per face, and aggregates over the face-adjacency graph with a graph network. Learns per-face and per-model features usable for classification and segmentation, including surface-type-relevant tasks. BRepNet instead defines convolution directly on B-rep topological walks (coedges), encoding surface and curve type plus topology.

**Limitations.** Learned classification is approximate and uncertified: it can propose "this looks like a cylinder" but cannot guarantee tolerance. Requires training data and generalizes imperfectly to unseen styles. A learned answer is a *proposal*, never a *certificate*.

**Kernel relevance.** Fits the **neuro-symbolic** pattern Keel uses for feature recognition: a learned classifier proposes the type cheaply (especially for ambiguous trimmed faces), then the exact recognizer + faithful fit + certified-deviation check *verify* it. Learning narrows the search; classical math certifies. UV-grid sampling is also exactly how Keel would feed a spline face to a classifier.

### Li et al.; Sharma et al.; Liu et al.: SPFN, ParSeNet, Point2CAD

**Citation.** Li, L., Sung, M., Dubrovina, A., Yi, L., & Guibas, L. (2019). Supervised fitting of geometric primitives to 3D point clouds (SPFN). *CVPR 2019.* Sharma, G., et al. (2020). ParSeNet: A parametric surface fitting network for 3D point clouds. *ECCV 2020.* Liu, Y., et al. (2024). Point2CAD: Reverse engineering CAD models from 3D point clouds. *arXiv:2312.04962.*

**Method.** SPFN is an end-to-end network detecting a varying number of primitives at multiple scales, supervised with ground-truth primitive membership. ParSeNet decomposes a point cloud into parametric patches including B-spline patches *and* basic primitives, with a segmentation module (labels each point by primitive type) and a fitting module (predicts primitive parameters), beating RANSAC and SPFN on segmentation. Point2CAD adds a full pipeline to a CAD-like model. All three jointly learn segmentation and type.

**Limitations.** Point-cloud (scan) oriented; produce approximate fits with local deviations (Point2CAD explicitly trades accuracy for speed and needs post-processing). Not certified, not exact.

**Kernel relevance.** Primarily relevant if Keel ingests scans/meshes; for spline simplification their *type-proposal* heads are the transferable idea. Reinforces the propose-then-verify split: the network labels, the kernel certifies.

---

## Theme 9: Implementation reality check (OSS)

### OpenCASCADE Technology: canonical recognition

**Citation.** Open Cascade SAS. (n.d.). *Canonical Recognition component* (occt3d.com/components/canonical-recognition-component-sdk) and `ShapeAnalysis_CanonicalRecognition` class reference, plus ShapeCustom / Convert_* and Shape Healing user guide. OCCT 7.x.

**Method.** OCCT ships an explicit canonical-recognition facility (matured in the 7.x series). `ShapeAnalysis_CanonicalRecognition` analyzes shape geometry and converts to canonical form: NURBS **curves** to line, circle/arc, or ellipse/elliptical arc; NURBS **surfaces** to plane, cylinder, cone, sphere, or torus. Recognition is governed by a **maximal deviation criterion**: the maximum distance between the original and the canonical object must be below a user-given tolerance. It can operate per-edge and per-face, and crucially can *coordinate* across topology: it can check all faces of a shell to convert them to the *same* analytic surface, or all surfaces sharing an edge to convert one to the analytic most-consistent with its neighbor. Supporting machinery: `ShapeCustom` (BSpline/Bezier-to-analytic conversion, scaling, etc.), `Convert_ElementarySurfaceToBSplineSurface` (the inverse), and `GeomLib` / `GeomConvert` utilities (e.g. planarity tests).

**Limitations.** Single max-deviation tolerance per call (recognition and approximation phases share adjustable but coarse tolerances). The deviation criterion is documented but the underlying recognition algorithm is not published in detail. Practical reports note it works well on cleanly-translated geometry but can miss heavily reparameterized or trimmed surfaces, and its torus/cone recognition is the most fragile. FreeCAD and other OSS consume OCCT's healing/canonical utilities rather than reimplementing them.

**Kernel relevance.** OCCT is the closest open-source reference implementation and a behavioral spec for Keel's recovery API. Two design lessons worth copying: (1) the **maximal-deviation criterion** as the single recover/keep gate; (2) **topology-coordinated** recognition (convert neighboring faces to the *same* analytic, or align across a shared edge) so that recovery improves topology consistency rather than producing inconsistent neighbors. Keel can do better than OCCT by exposing per-tolerance-class control and by always emitting a certified deviation bound, not just a pass/fail.

---

## Canonical recovery service for Keel

Synthesizing the literature into a concrete kernel service.

### Pipeline: recognize, fit, certify, substitute

1. **Recognize (cheap to expensive cascade).**
   - *Exact-structure pass (Piegl/Tiller).* If the rational B-spline still has canonical control-net + weight structure (circle weights cos(theta/2), quadric control grids), read off the type directly. Decompose tensor-product surfaces into two 1D curve recognitions (profile + rail).
   - *Differential pass (Benko/Varady, Gauss map).* Sample the face on a UV grid, compute (k1, k2) and normals **in closed form from the spline** (no scan noise, no finite differences). Classify by curvature signature; use Gauss-image dimension to separate plane/cylinder/cone.
   - *Line-geometry pass (Pottmann/Randrup/Wallner).* Fit a linear line complex to the sampled normal lines (6x6 eigenproblem, Plücker-project the eigenvector). A good fit means cylinder / surface-of-revolution / helical; the recovered pitch p branches the type (p=0 revolution, p=infinity extrusion/cylinder, finite helix) and yields the axis. Fit residual = recognition confidence.
   - *Optional learned proposal (UV-Net style).* For ambiguous trimmed faces, a learned classifier proposes a type to seed verification. Always treated as a proposal, never a certificate.

2. **Fit (faithful least squares, Lukacs/Marshall/Martin).** Given the proposed type, fit optimal parameters (axis, radius, half-angle, major/minor radii). Faithful parameterization degrades gracefully near type boundaries, giving a meaningful residual.

3. **Certify (certified deviation, Kim/Elber-style bound).** Compute a *guaranteed upper bound* on the max deviation from the original spline to the candidate analytic, using the analytic's closed-form distance function plus adaptive subdivision where samples approach tolerance. If the bound is below the active tolerance, accept; else **keep the spline** (the explicit do-not-replace decision).

4. **Substitute (topology-preserving swap).** Replace the face's surface (or edge's curve) while keeping the topology. Refit pcurves of the bounding edges onto the new analytic surface. For closed analytics (full cylinder, sphere, torus) introduce a seam edge. Recognize when a feature was split (a cylinder arriving as two half-cylinder faces) and **merge** the adjacent faces into one analytic face. For edges, recover spline-to-line/arc/ellipse with the same certify gate.

5. **Constraint-aware re-fit (Benko/Kos/Varady/Martin + Langbein).** After independent recovery, detect approximate regularities among recovered primitives (coaxial, parallel, perpendicular, equal-radius, concentric) and enforce a *consistent, non-over-constrained* subset by simultaneous constrained fitting, snapping near-regular configurations to exact ones. This yields clean editable models and, critically, enables exact analytic-analytic intersections downstream.

### Where it runs

- **Import / translation healing.** The primary site, mirroring ACIS HEAL ordering: stitch first (need valid topology to know the faces), then **simplify** (this service), then gap-tighten. This is where IGES-splinified and "splinify-everything" models get their analytic intent back.
- **Post-boolean cleanup.** Booleans can produce spline approximations of what should be analytic intersection geometry; a lightweight recovery pass can re-canonicalize edges (back to circles/lines) and faces after the operation, keeping the model from drifting toward all-spline over an operation history.
- **On demand for feature recognition.** Run (or trust cached results) before machining/design feature recognition so the recognizer sees exact surface types.

### Interaction with tolerances and naming

- **Tolerances.** The recover/keep gate is the certified max-deviation bound versus Keel's active tolerance (ideally a tolerance-*class*, e.g. modeling tolerance, looser than display tolerance). Recovery must never silently exceed tolerance; the certificate is mandatory, not advisory. Recovered analytics typically *tighten* the model (exact geometry replaces an approximation), but the swap's deviation is itself a tolerance the kernel must track on the swapped entity.
- **Naming / persistent IDs.** Surface-swap and face-merge change geometry and can change topology (seam introduction, two faces becoming one). The naming/persistent-ID subsystem must treat canonical recovery as a tracked operation: a merged half-cylinder pair needs a stable identity for downstream feature references; a newly-seamed closed surface introduces an edge that must be named. Recovery is therefore not a purely geometric edit; it is a topological event the naming layer observes.

### Build priority for Keel

1. Plane, cylinder, sphere, cone recovery (curvature + Gauss + faithful fit + certified bound). Highest payoff, covers most IGES damage.
2. Curve recovery (spline to line/circle/ellipse) with the same gate.
3. Torus, then surface-of-revolution / extrusion via the linear-complex recognizer.
4. Face-merge (split-cylinder) and seam handling, wired into naming.
5. Constraint-aware re-fit (coaxiality snapping) once independent recovery is solid.
6. Helical / developable / translational and learned proposals last.

---

## References

- Benko, P., Kos, G., Varady, T., Andor, L., & Martin, R. R. (2002). Constrained fitting in reverse engineering. *Computer Aided Geometric Design, 19*(3), 173-205.
- Benko, P., & Varady, T. (2002). Direct segmentation of smooth, multiple point regions. *Geometric Modeling and Processing (GMP 2002)*, 169-178. IEEE.
- Benko, P., & Varady, T. (2004). Segmentation methods for smooth point regions of conventional engineering objects. *Computer-Aided Design, 36*(6), 511-523.
- ITI / Wipro. (n.d.). *CADfix: CAD simplification and defeaturing for CAE* and *CAD model simplification* [Product documentation]. iti-global.com; cadinterop.com.
- Jayaraman, P. K., Sanghi, A., Lambourne, J. G., Willis, K. D. D., Davies, T., Shayani, H., & Morris, N. (2021). UV-Net: Learning from boundary representations. *CVPR 2021*, 11703-11712.
- Kim, Y.-J., Oh, Y.-T., Yoon, S.-H., Kim, M.-S., & Elber, G. (2013). Efficient Hausdorff distance computation for freeform geometric models in close proximity. *Computer-Aided Design, 45*(2), 270-276.
- Lambourne, J. G., Willis, K. D. D., Jayaraman, P. K., Sanghi, A., Meltzer, P., & Shayani, H. (2021). BRepNet: A topological message passing system for solid models. *CVPR 2021*.
- Langbein, F. C., Gao, C. H., Mills, B. I., Marshall, A. D., & Martin, R. R. (2004). Topological and geometric beautification of reverse engineered geometric models. *EG Symposium on Solid Modeling 2004*.
- Langbein, F. C., Marshall, A. D., & Martin, R. R. (2004). Choosing consistent constraints for beautification of reverse engineered geometric models. *Computer-Aided Design, 36*(3), 261-278.
- Li, L., Sung, M., Dubrovina, A., Yi, L., & Guibas, L. (2019). Supervised fitting of geometric primitives to 3D point clouds (SPFN). *CVPR 2019*.
- Liu, Y., et al. (2024). Point2CAD: Reverse engineering CAD models from 3D point clouds. *arXiv:2312.04962*.
- Lukacs, G., Marshall, A. D., & Martin, R. R. (1998). Faithful least-squares fitting of spheres, cylinders, cones and tori for reliable segmentation. *Computer Vision: ECCV'98*, LNCS 1406, 671-686. Springer.
- Open Cascade SAS. (n.d.). *Canonical Recognition component* and `ShapeAnalysis_CanonicalRecognition` / ShapeCustom / Shape Healing documentation. OCCT 7.x. occt3d.com; dev.opencascade.org.
- Peternell, M., & Pottmann, H. (n.d.). Recognition and reconstruction of special surfaces from point clouds; Developable surface fitting to point clouds. DMG, TU Wien.
- Piegl, L., & Tiller, W. (1997). *The NURBS Book* (2nd ed.). Springer.
- Piegl, L., & Tiller, W. (1999). Computing offsets of NURBS curves and surfaces. *Computer-Aided Design, 31*(2), 147-156.
- Pottmann, H., Hofer, M., Odehnal, B., & Wallner, J. (2004). Line geometry for 3D shape understanding and reconstruction. Springer (ECCV-associated chapter).
- Pottmann, H., & Randrup, T. (1998). Rotational and helical surface approximation for reverse engineering. *Computing, 60*(4), 307-322.
- Pottmann, H., & Wallner, J. (1999). An introduction to line geometry with applications. *Computer-Aided Design, 31*(1), 3-16.
- Pottmann, H., & Wallner, J. (2001). *Computational Line Geometry*. Springer.
- Schnabel, R., Wahl, R., & Klein, R. (2007). Efficient RANSAC for point-cloud shape detection. *Computer Graphics Forum, 26*(2), 214-226.
- Sharma, G., Liu, D., Maji, S., Kalogerakis, E., Chaudhuri, S., & Mech, R. (2020). ParSeNet: A parametric surface fitting network for 3D point clouds. *ECCV 2020*.
- Spatial Corporation. (n.d.). *3D ACIS Modeler: HEAL component documentation*; *Healing and preserving 3D model intent*; *How 3D ACIS Modeler improves the quality of translated 3D data* [Blog posts]. blog.spatial.com.
- Varady, T., Martin, R. R., & Cox, J. (1997). Reverse engineering of geometric models: an introduction. *Computer-Aided Design, 29*(4), 255-268.
