# Geometric Interrogation, Silhouettes, and Hidden Line Removal for B-rep/NURBS Models

Research dossier supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition. This file covers the interrogation side of the kernel (differential geometry on NURBS, surface quality analysis, silhouettes and draft analysis, geodesics, developability checks, curvature extrema) and the drawing-generation side (hidden line removal and projection pipelines). Interrogation backs manufacturing checks (draft, wall thickness, moldability) and Class-A surfacing review (isophotes, reflection lines, zebra, curvature combs). HLR backs 2D drawing/drafting generation from 3D models.

## Scope and structure

1. Differential geometry computation on NURBS (fundamental forms, principal curvatures, Gaussian/mean curvature, umbilics, lines of curvature)
2. Surface quality interrogation (isophotes, reflection lines, highlight lines, zebra, curvature combs)
3. Silhouettes and characteristic curves (silhouette computation, isoclines, draft angle, parting lines, mold accessibility)
4. Geodesics and curve-on-surface (parametric geodesics, exact mesh geodesics MMP/heat method, pcurves)
5. Surface flattening and developability checks (sheet metal)
6. Inflection and extremum interrogation (curvature extrema via spline constraint solving)
7. Hidden line removal (classic algorithms, curved-surface HLR, OCCT module, drawing pipelines)
8. Visibility and quantitative invisibility along curves
9. Minimum radius, clearance, wall thickness interrogation
10. Synthesis: Interrogation and HLR architecture for Keel

A consistent notation is used throughout. A NURBS surface is **S(u,v)**; first parametric derivatives are **S_u, S_v**; second derivatives **S_uu, S_uv, S_vv**; the unit surface normal is **n = (S_u x S_v) / |S_u x S_v|**.

---

## 1. Differential geometry computation on NURBS

The foundation for all curvature-based interrogation is the classical theory of surfaces evaluated on NURBS parametric derivatives. NURBS derivatives are themselves NURBS (or rational expressions in NURBS), so all the quantities below can be computed exactly at a parameter value, or represented symbolically as new spline fields.

### 1.1 Fundamental forms, principal curvatures, Gaussian and mean curvature (MIT Shape Interrogation Hyperbook, Ch. 3)

**Citation.** Patrikalakis, N. M., Maekawa, T., and Cho, W. (2009). *Shape Interrogation for Computer Aided Design and Manufacturing* (Hyperbook edition). Massachusetts Institute of Technology. Retrieved from https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/

**Method.** The first fundamental form coefficients are the dot products of the parametric tangents:

- E = S_u . S_u, F = S_u . S_v, G = S_v . S_v.

The first fundamental form I = E du^2 + 2F du dv + G dv^2 gives the surface metric (arc length, angles, area: dA = sqrt(EG - F^2) du dv). The second fundamental form coefficients project the second derivatives onto the normal:

- L = S_uu . n, M = S_uv . n, N = S_vv . n.

The normal curvature in a tangent direction (du, dv) is kappa_n = II/I = (L du^2 + 2M du dv + N dv^2) / (E du^2 + 2F du dv + G dv^2). The extreme normal curvatures (principal curvatures) are the roots of the quadratic

- kappa^2 - 2 H kappa + K = 0,

so kappa_1 = H + sqrt(H^2 - K), kappa_2 = H - sqrt(H^2 - K), where

- **Gaussian curvature** K = (L N - M^2) / (E G - F^2),
- **Mean curvature** H = (E N - 2 F M + G L) / (2 (E G - F^2)).

The sign of K classifies the local shape: K > 0 elliptic (convex/concave dome), K < 0 hyperbolic (saddle), K = 0 parabolic (one principal curvature zero, the developable signature). Principal directions are the eigenvectors of the shape operator (Weingarten map) and are mutually orthogonal away from umbilics. Lines of curvature are the integral curves of the principal direction fields, obtained by integrating dv/du from the relation (E M - F L) du^2 + (E N - G L) du dv + (F N - G M) dv^2 = 0.

**Limitations.** Near singular parametrizations (degenerate poles where S_u x S_v = 0, e.g., the apex of a cone or a collapsed surface edge) E G - F^2 vanishes and curvatures are undefined or numerically unstable. Self-intersections in derivative evaluation at multiple knots reduce continuity: at a C^1 knot line, second-derivative-based quantities (L, M, N, K, H) are generally discontinuous, which is exactly what continuity interrogation must detect, not smooth over.

**Kernel relevance.** This is the bedrock numerical layer for Keel's interrogation module. Provide `SurfaceLocalGeometry` returning (E,F,G,L,M,N,K,H,kappa1,kappa2,dir1,dir2) at any (u,v). Curvature fields feed manufacturing checks, Class-A review, and meshing (sizing by curvature). Guard against EG-F^2 near zero with a degeneracy tolerance and report parametric singularities explicitly.

### 1.2 Umbilics and lines of curvature (Maekawa & Patrikalakis)

**Citation.** Maekawa, T., Wolter, F.-E., and Patrikalakis, N. M. (1996). Umbilics and lines of curvature for shape interrogation. *Computer Aided Geometric Design*, 13(2), 133-161. (See also MIT Hyperbook Ch. 9.)

**Method.** An umbilic is a point where kappa_1 = kappa_2, so the second fundamental form is proportional to the first: L/E = M/F = N/G. At an umbilic the principal directions are indeterminate and the orthogonal net of lines of curvature is singular. The method locates all isolated umbilics on a polynomial/rational parametric surface robustly, then classifies them. The technique transforms the surface locally into **Monge form** z = f(x,y) with a Taylor expansion about the umbilic, and from the cubic terms of f computes the angles of the tangent lines of the lines of curvature passing through the umbilic. Generic umbilics have index +1/2 or -1/2 and fall into three patterns named by Darboux: **lemon** (one line of curvature through the umbilic, index +1/2), **monstar** (three lines, index +1/2, rare), and **star** (three lines, index -1/2). Umbilics depend only on surface shape, not parametrization, and act like fingerprints for shape recognition. Lines of curvature are traced by numerically integrating the principal direction ODE, switching the integrated branch consistently and slowing the step near umbilics where the field rotates rapidly.

**Limitations.** Robust isolation of all umbilics requires solving a system of nonlinear equations (the proportionality conditions) over the whole domain, which is costly; degenerate (non-isolated) umbilics on spheres/planes are whole regions and break the isolated-root assumption. Line-of-curvature tracing must handle the direction-field sign ambiguity and the singularity at the umbilic, where naive integrators stall or jump branches.

**Kernel relevance.** Lines of curvature and umbilic patterns are premium Class-A review tools (used in ship-hull and automotive surfacing) and feed structured quad-meshing aligned to curvature. Keel should expose umbilic detection as an optional advanced interrogation backed by the multivariate solver (Section 6), returning classified umbilics plus traced lines of curvature as pcurves.

---

## 2. Surface quality interrogation

These methods turn invisible curvature/continuity properties into visible characteristic curves whose distortions reveal defects. The governing principle: the geometric continuity of a characteristic curve is one order lower than the surface, so a C^k surface yields C^(k-1) characteristic curves, which makes a G1-but-not-G2 junction show up as a kink (G0) in the curves.

### 2.1 Isophotes (Poeschl 1984)

**Citation.** Poeschl, T. (1984). Detecting surface irregularities using isophotes. *Computer Aided Geometric Design*, 1(2), 163-168.

**Method.** An isophote is the locus of surface points whose unit normal makes a constant angle with a fixed light direction **v**: the set { S(u,v) : n(u,v) . v = c }, where c = cos(phi) is the constant brightness level. Sweeping c gives a family of isophote curves. Because the isophote condition involves the normal (a first-derivative quantity), the isophote is one differentiability order below the surface: a tangent-continuous (G1) but not curvature-continuous (G2) seam produces isophotes that meet but kink, immediately visible to the eye. Poeschl showed isophotes detect surface irregularities that pass unnoticed in shaded or wireframe views.

**Limitations.** Isophotes are view/light-direction dependent; a single light direction can miss defects aligned with it, so multiple directions are interrogated. Computing isophotes is an implicit-curve tracing problem (marching on n.v - c = 0 in parameter space) with the usual branching/closed-loop topology concerns.

**Kernel relevance.** Cheap to implement on top of the normal field; offer isophote families as a Class-A interrogation output (curves in parameter space lifted to pcurves). Useful as a fast visual G2 check before committing to full curvature-comb analysis.

### 2.2 Reflection lines (Klass 1980)

**Citation.** Klass, R. (1980). Correction of local surface irregularities using reflection lines. *Computer-Aided Design*, 12(2), 73-76.

**Method.** Reflection lines simulate the image of a family of parallel straight light lines (like a striped ceiling or showroom lighting) mirrored in the surface. A point lies on a reflection line if the reflected eye ray (reflected about the surface normal) intersects one of the light lines. Because reflection depends on the normal, reflection lines, like isophotes, are one order less continuous than the surface, so discontinuities in the normal (the surface not being C^2) show as breaks or sudden direction changes in the reflection lines. Klass used them both to detect and to guide manual correction of local irregularities.

**Limitations.** View-dependent (depend on eye position and the light-line geometry), so they are an inspection tool, not a view-independent invariant. Computation requires solving the reflection condition per light line, more expensive than isophotes.

**Kernel relevance.** Reflection lines are the historical core of automotive Class-A review and are what zebra (Section 2.4) approximates interactively. Keel can compute them analytically for high-fidelity reports; for interactive viewers, zebra/environment-map shading on a fine tessellation is the practical route.

### 2.3 Highlight lines (Beier & Chen 1994)

**Citation.** Beier, K.-P., and Chen, Y. (1994). Highlight-line algorithm for real-time surface-quality assessment. *Computer-Aided Design*, 26(4), 268-277.

**Method.** A highlight line is defined relative to a linear light source (an infinite straight light line in space). A surface point belongs to a highlight line if the extended surface normal line passes through (comes within tolerance of) the light line, i.e., the perpendicular distance from the light line to the normal line at the point is zero. Unlike reflection lines, highlight lines are **view-independent** (they do not depend on eye position), which makes them a more stable quality metric. The 1994 contribution was a real-time algorithm so designers could drag the light line and watch the highlight pattern update, giving immediate feedback on fairness and continuity. As with the others, the highlight line is one continuity order below the surface, so G1/G2 defects appear as kinks/jumps.

**Limitations.** Still light-line-orientation dependent (a defect parallel to the light line may be missed; sweep orientations). Real-time performance historically relied on tessellation; exact extraction on trimmed NURBS is an implicit tracing problem.

**Kernel relevance.** Highlight lines are the best single "fairness" interrogation because they are view-independent. Recommended as Keel's primary analytic surface-quality curve, with the light-line direction as a parameter the caller sweeps.

### 2.4 Zebra analysis and curvature combs/porcupines

**Citation.** (Industrial/standard technique; documented across Autodesk Alias, Rhino, SolidWorks, Onshape, Plasticity.) Representative: McNeel & Associates, Rhinoceros Zebra command documentation; Autodesk Alias surface continuity tutorials.

**Method.** **Zebra** projects alternating black/white stripes (an environment map of parallel bands) onto the surface and shows their mirror image, a real-time, GPU-friendly approximation of reflection lines. Reading the stripes across a seam diagnoses continuity directly: at a **G0** (position-only) seam the stripes do not line up and are offset; at **G1** (tangent) the stripes meet at the edge but turn sharply (a corner in each stripe); at **G2** (curvature) the stripes flow through smoothly with matched slope; at **G3** they flow through with matched curvature of the stripe. **Curvature combs** (for curves) plot, at sampled points along a curve, a "tooth" of length proportional to curvature magnitude, normal to the curve; the envelope (comb back) reveals inflections (comb crosses the curve, curvature sign flip), flat spots, and discontinuities (comb jumps). The 3D analog on surfaces is the **porcupine** plot (normal-curvature spikes). Curvature combs are the standard fairness tool for sketch/profile curves.

**Limitations.** Zebra is a visual/tessellation tool, accuracy limited by mesh density near the seam; it diagnoses but does not quantify. Comb scaling is arbitrary and must be tuned so defects are legible without saturating.

**Kernel relevance.** Keel should emit the raw data: per-sample curvature (and its derivative) for combs, and a tessellation with accurate normals for zebra shading in viewers. The kernel owns the math; the UI owns the stripe rendering. Continuity classification (G0/G1/G2) across edges is a kernel-level query the viewer visualizes.

---

## 3. Silhouettes and characteristic curves

### 3.1 Silhouette (contour generator) computation on NURBS

**Citation.** Elber, G., and Cohen, E. (1990s). Silhouette/contour extraction in IRIT; and Hornung-style/parametric methods surveyed in Patrikalakis-Maekawa-Cho. Representative recent: Kim, J.-K. et al., Perspective silhouette of a general swept volume, *The Visual Computer*, 22(2006). General references via NURBS projection literature (Piegl, *On NURBS: A Survey*).

**Method.** For **parallel (orthographic) projection** with view direction **d**, the silhouette (contour generator) is the curve on the surface where the normal is perpendicular to the view: n(u,v) . d = 0, equivalently (S_u x S_v) . d = 0. For **perspective projection** from eye point **e**, the condition is view-point dependent: n(u,v) . (S(u,v) - e) = 0. In both cases the contour is the zero set of a scalar field over the parameter domain. For NURBS, this scalar field is itself a (rational) spline (since S_u, S_v, n components are splines), so the silhouette is the zero-set of a known spline scalar field, traced robustly by: (1) finding zero-crossing seeds via subdivision/sign analysis of the control coefficients, (2) marching the implicit curve with Newton correction, (3) joining components and handling closed loops and tangential touchings. Trimming curves clip the silhouette to the visible (untrimmed) face region.

**Limitations.** Robustness near where the silhouette is tangent to itself or to trim boundaries; topology (number and connectivity of loops) changes as the view rotates (visual events: cusps, T-junctions). Perspective silhouettes move with the eye, so they must be recomputed per view. Numerically, the field n.d can be flat over large near-parallel regions, making zero-isolation ill-conditioned.

**Kernel relevance.** Silhouettes are dual-purpose: they are the core input to HLR (Section 7) and to mold/draft analysis (3.2, 3.3). Implement silhouette extraction once as zero-set tracing of a spline scalar field, parameterized by parallel-vs-perspective, returning pcurves on each face plus their 3D lifts. This is one of the highest-leverage routines in the kernel.

### 3.2 Isoclines and draft angle analysis (Elber et al.)

**Citation.** Elber, G., and Cohen, E. (1993). Second-order surface analysis using hybrid symbolic and numeric operators. *ACM Transactions on Graphics*, 12(2), 160-178. (Curvature/region partition machinery underlying draft and isocline work in IRIT.)

**Method.** An **isocline** generalizes the silhouette: it is the locus where the normal makes a fixed angle alpha with the draw (pull) direction **d**: n(u,v) . d = cos(alpha). The silhouette is the special case alpha = 90 degrees (n.d = 0). The **draft angle** at a point is 90 degrees minus the angle between the normal and the draw direction, i.e., the angle between the surface and the pull direction. A part is moldable in direction d only where the draft angle is at least a minimum (commonly 1-3 degrees); regions with negative draft are **undercuts**. Draft analysis colors the surface by the scalar n.d: positive = on the "core" side draws cleanly, negative = "cavity" side, near-zero = near-vertical wall at risk. Elber and Cohen's hybrid symbolic/numeric approach builds the scalar field n.d as a spline symbolically (exact), then extracts iso-contours and sign regions numerically, so draft zones and isoclines are computed as exact spline level sets.

**Limitations.** Requires a chosen draw direction; finding a feasible draw direction is itself a search (3.3). Symbolic construction of n.d inflates degree (products of splines), so the resulting field is high degree and benefits from refinement/refit. Trimmed regions and seams complicate region partitioning.

**Kernel relevance.** Draft analysis is a flagship manufacturing check. Keel should compute the n.d spline field for a given pull direction and return: per-face draft min/max, undercut regions (n.d sign), and isocline curves at the minimum draft threshold. Reuse the silhouette tracer (3.1) with a nonzero right-hand side.

### 3.3 Parting lines and mold accessibility (Elber, Chen & Cohen 2005)

**Citation.** Chen, X., Elber, G., and Cohen, E. (2005). Mold accessibility via Gauss map analysis. *ASME Journal of Computing and Information Science in Engineering*, 5(2), 79-89.

**Method.** A two-piece mold must separate cleanly: both halves pull out in opposite directions (+d and -d) without undercuts. The **parting line** is the boundary between the two mold halves, naturally the silhouette loop(s) for direction d (where draft passes through zero). The key theoretical result: a valid two-piece partition exists for a compact surface of genus g if and only if there is a view (draw) direction from which the silhouette consists of exactly g+1 nonsingular disjoint loops. This reduces mold separability to a topological condition on silhouettes, and the search for a feasible draw direction is performed via **Gauss map analysis**: each surface patch maps to a region on the unit sphere (the set of its normals); a candidate direction d is undercut-free for one half if the corresponding hemisphere of the Gauss map is covered consistently. Decomposing the Gauss map and intersecting accessibility cones yields feasible directions and parting lines.

**Limitations.** Restricted to two-piece molds; complex parts need side actions/multi-piece molds beyond this analysis. Gauss-map region computation for trimmed freeform surfaces is expensive and the silhouette-loop-counting condition assumes nonsingular silhouettes (tangencies are degenerate cases). Genus computation requires correct B-rep topology.

**Kernel relevance.** Directly serves Keel's moldability checks: given a candidate draw direction, classify undercuts and extract parting-line loops; optionally search for feasible directions via Gauss map. Builds on silhouette extraction, draft fields, and the kernel's topology (genus, loop counting).

---

## 4. Geodesics and curve-on-surface

### 4.1 Geodesics on parametric surfaces: initial-value and boundary-value forms (MIT Hyperbook Ch. 10)

**Citation.** Patrikalakis, N. M., Maekawa, T., and Cho, W. (2009). *Shape Interrogation for CAD and Manufacturing*, Ch. 10 (Geodesics). MIT Hyperbook. See also Patrikalakis & Bardis on geodesic offsets, and Maekawa on shortest-path relaxation.

**Method.** A geodesic is a curve of zero geodesic curvature (locally shortest, "straightest" path on the surface). For a surface S(u(t), v(t)) the geodesic equations are a coupled second-order ODE system in the parameters, written with the **Christoffel symbols of the second kind** Gamma^k_ij (functions of E,F,G and their first derivatives):

- u'' + Gamma^1_11 (u')^2 + 2 Gamma^1_12 u' v' + Gamma^1_22 (v')^2 = 0,
- v'' + Gamma^2_11 (u')^2 + 2 Gamma^2_12 u' v' + Gamma^2_22 (v')^2 = 0,

(when t is arc length, or proportional to it). The **initial-value problem (IVP)** gives a start point and start tangent direction and integrates forward (e.g., Runge-Kutta), used by Patrikalakis & Bardis to shoot geodesics normal to a progenitor curve to build **geodesic offsets** (linkage curves for blends). The **boundary-value problem (BVP)** asks for the geodesic (shortest path) between two given points (or a point and a curve), solved by **shooting** (vary the initial direction until the far end hits the target) or **relaxation** (finite-difference discretize the path and iterate to satisfy the geodesic equations and boundary conditions, Maekawa's approach).

**Limitations.** Geodesics are only locally shortest; multiple geodesics can connect two points and the global shortest must be selected. Shooting is sensitive to the initial guess and can fail to converge across high-curvature or trimmed regions; relaxation needs a reasonable initial path. Christoffel symbols blow up near parametric singularities (EG-F^2 -> 0). Crossing trim boundaries or surface seams requires stitching across faces.

**Kernel relevance.** Geodesics support fiber/composite layup paths, shortest tool paths, geodesic-offset blend construction, and "shortest path on shell" queries. Keel should provide both IVP integration (start + direction + length) and BVP shortest-path (two points) on a single trimmed surface, with a meshed fallback (4.2) for multi-face shells where the exact parametric BVP is impractical.

### 4.2 Exact and fast mesh geodesics: MMP, Surazhsky et al., heat method (Crane et al.)

**Citation (MMP).** Mitchell, J. S. B., Mount, D. M., and Papadimitriou, C. H. (1987). The discrete geodesic problem. *SIAM Journal on Computing*, 16(4), 647-668.
**Citation (improved/approx).** Surazhsky, V., Surazhsky, T., Kirsanov, D., Gortler, S. J., and Hoppe, H. (2005). Fast exact and approximate geodesics on meshes. *ACM Transactions on Graphics (SIGGRAPH)*, 24(3), 553-560.
**Citation (heat method).** Crane, K., Weischedel, C., and Wardetzky, M. (2013). Geodesics in heat: A new approach to computing distance based on heat flow. *ACM Transactions on Graphics*, 32(5), Article 152.

**Method.** **MMP** computes exact polyhedral (piecewise-linear) geodesic distance from a source to all mesh vertices by propagating "windows" (intervals of edges over which the distance function is a known one-parameter family) in wavefront (Dijkstra-like) order with a priority queue, in O(n^2 log n). **Surazhsky et al.** showed MMP runs sub-quadratically in practice and gave an approximate O(n log n) variant with guaranteed error bounds by merging windows, plus exact-geodesic-path backtracing. The **heat method** sidesteps wavefront propagation with three linear-algebra steps:
1. Heat flow: integrate the heat equation for a short time t by one backward-Euler step, (Id - t L) u = delta_source, where L is the cotangent Laplacian and delta is the source indicator (a heat "spike").
2. Normalize: compute the gradient of u, then form the unit field X = - grad u / |grad u|, which points along geodesics (away from the source).
3. Poisson solve: recover distance phi by solving the Poisson equation L phi = div X (so that grad phi matches X).
The recommended time step is t = m h^2 (h = mean edge length, m a small constant near 1). Both linear systems share the Laplacian and can be prefactored once (Cholesky), then each new source is two back-substitutions, near-linear and reusable.

**Limitations.** MMP/window methods are exact but hard to parallelize (wavefront ordering) and costly to amortize over many sources. The heat method is approximate (smooths over the true distance, accuracy depends on t and mesh quality), can lose accuracy near the cut locus, and needs a Laplacian that is well-behaved (badly shaped triangles or non-Delaunay meshes degrade it; intrinsic Delaunay helps). All mesh methods inherit tessellation error relative to the true NURBS surface.

**Kernel relevance.** For Keel's multi-face shells and triangulated B-reps, mesh geodesics are the practical workhorse: heat method for fast all-pairs/field distance queries (geodesic distance fields, Voronoi on surface, remeshing sizing), MMP/Surazhsky when exact single-source paths or guaranteed-error paths are required. Pair with the parametric IVP/BVP (4.1) for single-face exactness. A prefactored cotangent-Laplacian module is reusable across the heat method, smoothing, and parameterization (Section 5).

### 4.3 Curve-on-surface representation (pcurves)

**Citation.** Industry-standard B-rep practice (Parasolid, ACIS, OCCT); summarized in CAD interoperability and B-rep literature. See also NURBS approximation of intersection curves (Purdue CS tech reports).

**Method.** A curve lying on a surface (a trim boundary, silhouette, isophote, geodesic, etc.) is best stored as a **pcurve**: a 2D curve C(t) = (u(t), v(t)) in the surface's parameter domain, whose 3D realization is S(C(t)). The 3D space curve and the pcurve are dual representations of the same edge. **Exact** representation is possible only when the on-surface curve is genuinely a NURBS in parameter space (e.g., an iso-parameter line); general intersection/silhouette curves are **transcendental** and must be **approximated** by fitting a NURBS pcurve (and/or a NURBS 3D curve) to traced sample points within a chord/parametric tolerance. B-rep edges therefore carry both a 3D curve and one pcurve per adjacent face, with a tolerance budget reconciling them at the shared boundary (the classic "the two faces disagree at the edge by epsilon" problem).

**Limitations.** Approximate pcurves introduce a gap between the 3D curve and S(pcurve); keeping these consistent under modeling operations (the central robustness headache of tolerant B-reps) requires careful tolerance management. High-curvature or near-singular regions need dense fitting. Reparametrization of the surface invalidates pcurves.

**Kernel relevance.** Pcurves are foundational to Keel's B-rep edge data structure. Every interrogation curve that lives on a face (silhouette, draft isocline, isophote, geodesic) should be returned as a fitted pcurve plus 3D curve with an explicit tolerance, consistent with how trim edges are stored, so downstream HLR and drawing code treats interrogation curves uniformly.

---

## 5. Surface flattening and developability checks (sheet metal)

### 5.1 Gaussian curvature as the obstruction to flattening

**Citation.** Classical (Gauss's Theorema Egregium); applied in CAD: do Carmo, *Differential Geometry of Curves and Surfaces*; developability criteria summarized in the surface-flattening literature.

**Method.** A surface is **developable** (flattenable to a plane without stretching) if and only if its Gaussian curvature K = 0 everywhere; by Gauss's Theorema Egregium, K is an isometry invariant, so it is precisely the obstruction to distortion-free flattening. Developability interrogation therefore reduces to mapping K(u,v) over the surface (Section 1.1) and flagging where |K| exceeds a tolerance: such regions cannot be flattened without strain (stretch/shrink) and will wrinkle or tear in sheet metal/fabric. For a NURBS surface, K is a known rational spline field, so developability is checkable exactly as a level-set/sign query on K.

**Limitations.** Zero discrete Gaussian curvature is necessary but tolerancing it on freeform surfaces is delicate (K is a high-degree rational field). True developability also requires zero K along the whole surface, not just at samples; a surface can be locally near-developable yet globally non-flattenable due to boundary constraints.

**Kernel relevance.** Provide a developability report: K field, max strain estimate, and developable/non-developable region partition. Direct input to sheet-metal unfold feasibility before attempting a flatten.

### 5.2 Flattening algorithms: energy-based, ARAP, and length-preserving methods

**Citation (energy-based / sheet metal).** Wang, C. C. L., et al. work on flattenable mesh surfaces; energy-based flattening for sheet-metal flat patterns (e.g., *Towards flattenable mesh surfaces*, Computer-Aided Design). McCartney, J., Hinds, B. K., and Seow, B. L. (1999). The flattening of triangulated surfaces incorporating darts and gussets. *Computer-Aided Design*, 31(4), 249-260.
**Citation (ARAP parameterization).** Liu, L., Zhang, L., Xu, Y., Gotsman, C., and Gortler, S. J. (2008). A local/global approach to mesh parameterization (as-rigid-as-possible). *Computer Graphics Forum (SGP)*, 27(5), 1495-1504.
**Citation (length-preserving).** Wang, C. C. L., et al. WireWarping: a fast surface flattening with length-preserved feature curves.
**Citation (Bennis).** Bennis, C., Vezien, J.-M., and Iglesias, G. (1991). Piecewise surface flattening for non-distorted texture mapping. *ACM SIGGRAPH Computer Graphics*, 25(4), 237-246.

**Method.** When a surface is not developable, flattening minimizes distortion energy. **McCartney et al.** flatten a triangulated surface by an energy-relaxation/spring model, inserting **darts and gussets** (cuts and inserts) to absorb the strain that double curvature would otherwise impose, the standard sheet-metal/garment trick. **ARAP (as-rigid-as-possible)** parameterization (Liu et al.) is the modern general approach: it alternates a **local** step (fit the best rotation to each triangle's 2D image) and a **global** step (solve a sparse linear system, again the cotangent Laplacian, for vertex positions that best match those rotations), minimizing local stretch/shear so each triangle stays as close to rigid as possible. **Bennis et al.** pioneered piecewise flattening guided by lines of curvature for low-distortion mapping. **WireWarping** preserves the lengths of designated feature curves during flattening. Distortion maps (per-element stretch/compression) are reported so the user sees where material must give.

**Limitations.** Non-developable surfaces cannot flatten without distortion: the algorithms trade off where the unavoidable strain goes; dart placement is heuristic. ARAP can flip triangles or get stuck in local minima and needs a good initialization and cut/seam choice. All operate on a mesh, inheriting tessellation error; the resulting flat pattern must be related back to the NURBS surface for manufacturing.

**Kernel relevance.** Keel's sheet-metal unfold should: (1) check developability (5.1); (2) for developable/near-developable cases, unfold along rulings; (3) for general doubly-curved cases, run ARAP/energy flattening on a mesh and return the flat pattern plus a distortion map, reusing the cotangent-Laplacian solver shared with the heat method (4.2). Dart/gusset insertion is an application-level policy on top.

---

## 6. Inflection and extremum interrogation (curvature extrema via spline constraint solving)

**Citation.** Elber, G., and Kim, M.-S. (2001). Geometric constraint solver using multivariate rational spline functions. *Proceedings of the 6th ACM Symposium on Solid Modeling and Applications*, 1-10. (And the IRIT multivariate solver; Elber & Cohen 1993 for the symbolic-curvature machinery.)

**Method.** Many interrogation queries are root-finding/extremum problems over splines: curvature extrema and inflections on a curve (where kappa' = 0, or kappa = 0 for inflections), flat spots, ridges (extrema of principal curvature along lines of curvature), closest points, self-intersections, silhouettes, and draft thresholds. Elber's "interrogation by solver" approach builds the relevant scalar/vector property as an **exact spline field** via symbolic operators (derivatives, products, dot/cross products of NURBS yield NURBS), then solves the resulting system of polynomial/rational spline equations for **all** real roots using a **multivariate spline solver**. The solver exploits the convex-hull and subdivision properties of the spline basis: it subdivides the domain, discards subdomains whose control coefficients are strictly one sign (no root by the convex-hull/variation-diminishing property), and refines the rest to isolate every root with guaranteed coverage, then Newton-polishes. This yields robust **multi-extremum** queries (find every curvature maximum, every inflection) rather than a single local result.

**Limitations.** Symbolic construction inflates degree (e.g., curvature involves products and a square root, so kappa^2 or signed-curvature numerator/denominator splines are used), increasing solve cost; high-dimensional systems (surface ridges, umbilics) are expensive. Tolerance and multiplicity handling near tangential roots needs care. Square-root terms (true curvature) are handled by working with squared/rational forms.

**Kernel relevance.** This is the unifying engine for Keel's interrogation: a single robust multivariate spline solver underpins curvature extrema, inflections, umbilics (Section 1.2), silhouettes/isoclines (Section 3), closest-point/point-inversion, and intersection seeding. Building this solver well (subdivision + convex-hull pruning + Newton) pays off across the whole kernel and matches how IRIT/Parasolid-class systems achieve robustness. Prioritize it.

---

## 7. Hidden line removal (HLR)

### 7.1 Quantitative invisibility (Appel 1967)

**Citation.** Appel, A. (1967). The notion of quantitative invisibility and the machine rendering of solids. *Proceedings of the ACM National Conference*, 387-393.

**Method.** Appel assigns each point on a projected edge a **quantitative invisibility (QI)**: the integer count of front-facing surfaces that lie between the point and the eye (QI = 0 means visible). The algorithm: (1) project all edges; classify faces as front- or back-facing; the candidate visible lines are sharp edges and **silhouette** (contour) edges, where front meets back. (2) Compute the QI at one reference point by counting obscuring faces (ray test). (3) **Propagate** QI along each edge cheaply: QI only changes when the edge crosses, in the projection, a silhouette edge of the model. At each such crossing, a sign rule (does the crossed silhouette pass in front or behind, and is it entering or leaving an occluding region) increments or decrements QI by 1. Segments with QI = 0 are drawn; QI > 0 are hidden (or drawn dashed). This converts a global visibility problem into local updates at silhouette crossings, far cheaper than testing every point against every face.

**Limitations.** Needs correct silhouette/contour topology and robust detection of all curve-curve crossings in the projection plane; a single missed or mis-signed crossing corrupts the QI of an entire downstream segment. The base QI must be seeded correctly. Originally for polyhedra; curved surfaces require first extracting smooth silhouettes (Section 3.1) and treating them as the QI-changing curves.

**Kernel relevance.** QI propagation is the algorithmic heart of Keel's exact HLR. Combined with analytic silhouette extraction on NURBS, it gives clean, exact drawing edges. The robustness of the 2D arrangement (Section 8) is the make-or-break detail.

### 7.2 Roberts and Weiler-Atherton

**Citation.** Roberts, L. G. (1963). *Machine perception of three-dimensional solids* (Technical Report TR-315). MIT Lincoln Laboratory. // Weiler, K., and Atherton, P. (1977). Hidden surface removal using polygon area sorting. *Computer Graphics (SIGGRAPH)*, 11(2), 214-222.

**Method.** **Roberts** gave the first general HLR algorithm: represent each convex solid by its bounding planes, and for every edge test it against every volume, using a parametric line/volume formulation to compute which portions are obscured (an edge segment is hidden if it lies behind some solid as seen from the eye). **Weiler-Atherton** is an object-space (exact) hidden-surface method using **polygon area sorting**: recursively clip polygons against each other in priority (depth) order, subdividing the image into polygon-shaped windows until depth order within a window is resolved; it handles concave polygons and holes by working in true object space with a clipping (Weiler-Atherton) polygon-clipping core, producing exact visible polygon boundaries (and thus hidden lines as the complement).

**Limitations.** Roberts' method assumes convex solids and is O(edges x solids), poor scaling. Weiler-Atherton's recursive clipping is geometrically intricate and numerically sensitive at polygon intersections (the same robustness concerns as all exact-arithmetic clipping); it is a polygon method, so curved surfaces must be tessellated or handled by silhouette decomposition.

**Kernel relevance.** Historical grounding; the Weiler-Atherton polygon-clipping core is reusable for the **polyhedral (fast) HLR** path and for section-view hatching/region operations. The exact path should prefer the silhouette + QI approach (7.1, 7.4).

### 7.3 OCCT HLR module (exact vs polyhedral)

**Citation.** Open CASCADE Technology documentation: `HLRBRep_Algo`, `HLRBRep_PolyAlgo`, `HLRAlgo` class references; OCCT Modeling Algorithms / Visualization user guides. https://dev.opencascade.org/doc/

**Method.** OCCT provides two HLR engines that take a 3D shape plus a projection (view) and return characteristic 2D lines classified as visible or hidden. **HLRBRep_Algo** is **exact**: it operates on the actual B-rep geometry (true curves and surfaces), computing real silhouette/outline curves and exact visibility, giving precise drawing edges suited to production drawings, at higher cost (slow for complex shapes). **HLRBRep_PolyAlgo** is **polyhedral**: it works on a triangulated simplification of the shape and returns polygonal segments, much faster but approximate. Output edge categories include sharp edges, smooth (silhouette/outline) edges, sewn edges, and isoparametric lines, each tagged visible or hidden. Downstream **extraction** utilities (e.g., `HLRBRep_HLRToShape`) reconstruct a new 2D shape (edges in the projection plane) from selected results, which becomes the drawing view. This two-engine design (exact for final drawings, poly for interactive/preview) is the de facto open-source reference architecture.

**Limitations.** Exact HLR is computationally heavy and historically a robustness hotspot (silhouette computation and 2D arrangement on real geometry). Poly HLR's accuracy depends on tessellation quality and shows faceting on curved silhouettes. Both need correct, clean B-rep input (valid topology, consistent face orientations).

**Kernel relevance.** OCCT is the closest open-source analog to what Keel needs and validates the **dual exact/polyhedral** strategy. Keel should mirror it: a polyhedral HLR for fast preview (tessellate, project, QI/area-sort) and an exact HLR (analytic silhouettes + QI propagation + 2D arrangement) for production drawings, with a uniform classified-edge output (visible/hidden, sharp/smooth/outline) feeding the drawing module.

### 7.4 HLR for curved surfaces and drawing/drafting pipelines

**Citation.** Composite of the above plus drafting-pipeline practice (view generation, section views) in CAD systems; technical-illustration HLR literature (e.g., Hertzmann-style and OCCT drafting).

**Method.** Exact HLR for curved (NURBS) models proceeds: (1) extract all candidate edges, real B-rep edges plus **smooth silhouette curves** per face (Section 3.1) for the given projection; (2) project them to the view plane; (3) compute the **arrangement** (all intersections) of the projected curves; (4) assign QI to each resulting sub-segment via Appel propagation, seeding with ray tests and updating at silhouette crossings; (5) output visible (QI = 0) segments as solid drawing lines, hidden as dashed. The **drafting pipeline** wraps this: a drawing view is a (parallel or perspective) projection of the model; **section views** first cut the solid with a plane (a boolean/section operation producing section faces), hatch the cut faces, then HLR the remaining geometry behind the cut. Multiple standard views (front/top/right/iso), dimensions, and centerlines are assembled into a drawing sheet.

**Limitations.** The 2D arrangement of projected free-form curves is the central robustness problem (Section 8): near-tangent intersections, cusps of the silhouette, and curves that touch without crossing all stress floating-point intersection. Sectioning needs robust solid-plane boolean. Hidden-line dashing and centerlines are presentation policy.

**Kernel relevance.** This pipeline is exactly Keel's drawing-generation feature. Architect HLR as: silhouette extraction (shared with interrogation), projection, robust 2D arrangement, QI propagation, classified-edge output. Section views reuse the kernel's boolean/section. Keep geometry (kernel) separate from annotation/sheet layout (application).

---

## 8. Visibility and quantitative invisibility along curves; 2D arrangement robustness

**Citation.** Synthesis of Appel (1967, Section 7.1) with computational-geometry arrangement robustness practice (exact/adaptive predicates; CGAL arrangement design notes).

**Method.** Along any projected curve, visibility is piecewise constant in QI and changes only at well-defined events: crossings with silhouette curves (QI +/-1) and at silhouette **cusps** and **T-junctions** (visual events where silhouette topology changes). Computing visibility along a curve therefore means: build the planar **arrangement** of all projected curves (vertices = intersections/cusps, edges = curve sub-segments, faces = regions), then label each edge with QI by propagation from a seed. Robustness hinges on the arrangement: intersection points must be computed and ordered consistently so that the combinatorial structure (who crosses whom, in what order) is correct even when numerically two curves are nearly tangent or nearly coincident. Industrial practice uses adaptive-precision or exact geometric predicates for the **orientation** and **on-curve ordering** decisions while allowing approximate coordinates, the "exact predicates, inexact constructions" discipline.

**Limitations.** Free-form curve-curve intersection has no closed form; intersections are found numerically with tolerance, so the arrangement is only as robust as the intersection routine and the consistency of tolerance decisions. Near-tangencies and high-multiplicity crossings are the classic failure points; a single inconsistent decision can break QI propagation globally.

**Kernel relevance.** Keel needs a **robust 2D curve arrangement** with consistent (ideally exact) predicates as a core service, shared by HLR, hatching/section regions, and 2D sketch constraint resolution. Investing here directly determines whether exact HLR is reliable. Snap-rounding or tolerance-consistent construction is the pragmatic path; full exact arithmetic on rational curves is the gold standard but heavy.

---

## 9. Minimum radius, clearance, and wall thickness interrogation

**Citation.** Industrial methods documented by Geometric GeomCaliper (*Efficient Wall Thickness Analysis Methods* whitepaper), Analysis Situs (thickness distribution feature), and voxel/sphere thickness literature (e.g., voxel-based thickness analysis). Medial-axis foundations: Blum's medial axis transform.

**Method.** Two complementary industrial techniques:
- **Ray casting (opposite-wall) thickness:** from a surface point, shoot a ray along the inward normal and measure the distance to the first opposing surface hit. Fast, intuitive for nominally uniform sheet/cast walls, but normal-direction-dependent and can miss true minimum thickness on oblique or curved walls.
- **Shrinking-ball / maximum-inscribed-sphere (MIS) thickness:** at a point, grow/shrink the largest sphere that stays inside the solid and touches the surface; its diameter is the thickness (this matches the mechanical-drawing thickness definition and is robust on complex closed geometry). The locus of MIS centers is the **medial axis/medial surface**; thus thickness and the medial axis are the same computation viewed two ways. **Minimum radius / clearance** queries use the same machinery: the minimum internal radius is the smallest MIS over a region (sharp internal corners), and **clearance** between two parts is a closest-distance/MIS query in the gap.
- Implementation is often **voxel- or mesh-based** (rasterize the solid, sphere/distance-transform), or ray-based on the B-rep, trading exactness for robustness on dirty geometry.

**Limitations.** Ray casting can report wrong thickness where the opposite wall is not along the normal. Sphere/medial methods are sensitive to discretization (voxel resolution) and the medial axis is notoriously unstable to small surface perturbations (a tiny bump spawns medial branches). Exact medial axis of freeform B-reps is very hard; most production tools approximate via voxels/meshes.

**Kernel relevance.** Keel should offer wall-thickness/clearance/min-radius as a meshing-or-voxel-backed interrogation returning a thickness field and min/max with locations, plus a ray-cast quick mode. Exact B-rep distance (closest-point via the multivariate solver, Section 6) covers clearance between specific faces; the sphere/medial method covers global thickness maps. These drive castability and injection-molding (sink-mark) checks.

---

## 10. Interrogation and HLR architecture for Keel

Pulling the sources together, the recommended architecture for Keel's interrogation and drawing subsystems:

**A. Local differential-geometry layer (foundation).** A `surface_local_geometry(u,v)` returning E,F,G,L,M,N and derived K, H, principal curvatures/directions, with explicit handling of parametric degeneracies (EG-F^2 -> 0). All higher interrogation builds on this. (Sources 1.1, 1.2.)

**B. Symbolic spline-field + multivariate solver engine (the unifier).** Build property fields (n.d for draft/silhouette, curvature numerators, n.v for isophotes) as exact spline expressions, then solve/level-set them with a subdivision + convex-hull-pruning + Newton multivariate solver. This single engine powers silhouettes, isoclines, draft, isophotes, curvature extrema, inflections, umbilics, closest-point, and intersection seeding. It is the highest-leverage component and should be prioritized. (Sources 3.1-3.3, 6, 1.2.)

**C. Characteristic-curve and quality interrogation.** Isophotes (n.v = c), reflection/highlight lines, zebra data, curvature combs/porcupines, lines of curvature, umbilic classification. Output as fitted pcurves + 3D curves with tolerance, plus accurate-normal tessellation for viewer-side zebra/env-map shading. Emphasis on highlight lines (view-independent) as the primary analytic fairness curve, with G0/G1/G2 cross-edge continuity queries. (Sources 1.2, 2.1-2.4, 4.3.)

**D. Manufacturing interrogation.** Draft analysis and undercut regions (n.d field), parting lines and two-piece mold accessibility (silhouette-loop topology + Gauss map), wall thickness/clearance/min-radius (ray + shrinking-ball/medial, mesh/voxel backed), developability (K field) feeding sheet-metal unfold (ARAP/energy flattening with distortion maps). (Sources 3.2, 3.3, 5, 9.)

**E. Geodesics and curve-on-surface.** Parametric IVP (start+direction) and BVP (shortest path) on single faces via Christoffel-symbol ODEs (shooting/relaxation); mesh geodesics (heat method for fast distance fields, MMP/Surazhsky for exact paths) on multi-face shells, sharing a prefactored cotangent-Laplacian module with flattening. All on-surface curves stored as pcurves. (Sources 4.1-4.3, 5.2.)

**F. HLR and drawing pipeline.** Dual engines mirroring OCCT: polyhedral HLR (tessellate, project, area-sort/QI) for fast preview, exact HLR (analytic silhouettes + projection + robust 2D arrangement + Appel QI propagation) for production drawings. Output uniformly classified edges (visible/hidden, sharp/smooth/outline). Section views reuse the kernel's solid-plane boolean and hatching; sheet/annotation layout stays in the application. (Sources 7.1-7.4, 8.)

**G. Cross-cutting robustness.** A robust 2D curve arrangement with consistent/exact predicates ("exact predicates, inexact constructions") underpins exact HLR, section hatching, and 2D sketching. Pcurve-vs-3D-curve tolerance management underpins all on-surface interrogation curves. These two robustness investments determine whether the exact paths are trustworthy. (Sources 4.3, 8.)

**Shared-component summary.** Three reusable engines serve almost everything: (1) the multivariate spline solver (B), (2) the cotangent-Laplacian linear-algebra module (heat geodesics + ARAP flattening), and (3) the robust 2D arrangement (HLR + hatching + sketching). Silhouette extraction is computed once and reused by HLR, draft, and mold analysis. Building these three engines and the silhouette tracer well gives Keel Parasolid-class interrogation and drawing breadth from a small, sound core.

---

## References

1. Patrikalakis, N. M., Maekawa, T., & Cho, W. (2009). *Shape Interrogation for Computer Aided Design and Manufacturing* (Hyperbook ed.). MIT. https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/
2. Maekawa, T., Wolter, F.-E., & Patrikalakis, N. M. (1996). Umbilics and lines of curvature for shape interrogation. *Computer Aided Geometric Design*, 13(2), 133-161.
3. Poeschl, T. (1984). Detecting surface irregularities using isophotes. *Computer Aided Geometric Design*, 1(2), 163-168.
4. Klass, R. (1980). Correction of local surface irregularities using reflection lines. *Computer-Aided Design*, 12(2), 73-76.
5. Beier, K.-P., & Chen, Y. (1994). Highlight-line algorithm for real-time surface-quality assessment. *Computer-Aided Design*, 26(4), 268-277.
6. McNeel & Associates. *Rhinoceros Zebra command* documentation; Autodesk *Alias* surface continuity tutorials (zebra, curvature combs, reflection/iso-angle).
7. Elber, G., & Cohen, E. (1993). Second-order surface analysis using hybrid symbolic and numeric operators. *ACM Transactions on Graphics*, 12(2), 160-178.
8. Kim, J.-K., et al. (2006). Perspective silhouette of a general swept volume. *The Visual Computer*, 22(9-11).
9. Piegl, L. *On NURBS: A Survey*. (and Piegl & Tiller, *The NURBS Book*).
10. Chen, X., Elber, G., & Cohen, E. (2005). Mold accessibility via Gauss map analysis. *ASME Journal of Computing and Information Science in Engineering*, 5(2), 79-89.
11. Elber, G., & Kim, M.-S. (2001). Geometric constraint solver using multivariate rational spline functions. *Proc. 6th ACM Symposium on Solid Modeling and Applications*, 1-10.
12. Mitchell, J. S. B., Mount, D. M., & Papadimitriou, C. H. (1987). The discrete geodesic problem. *SIAM Journal on Computing*, 16(4), 647-668.
13. Surazhsky, V., Surazhsky, T., Kirsanov, D., Gortler, S. J., & Hoppe, H. (2005). Fast exact and approximate geodesics on meshes. *ACM TOG (SIGGRAPH)*, 24(3), 553-560.
14. Crane, K., Weischedel, C., & Wardetzky, M. (2013). Geodesics in heat: A new approach to computing distance based on heat flow. *ACM Transactions on Graphics*, 32(5), Art. 152. https://www.cs.cmu.edu/~kmcrane/Projects/HeatMethod/
15. Appel, A. (1967). The notion of quantitative invisibility and the machine rendering of solids. *Proc. ACM National Conference*, 387-393.
16. Roberts, L. G. (1963). *Machine perception of three-dimensional solids* (TR-315). MIT Lincoln Laboratory.
17. Weiler, K., & Atherton, P. (1977). Hidden surface removal using polygon area sorting. *Computer Graphics (SIGGRAPH)*, 11(2), 214-222.
18. Open CASCADE Technology. HLRBRep_Algo / HLRBRep_PolyAlgo / HLRAlgo class references and Modeling Algorithms guide. https://dev.opencascade.org/doc/
19. McCartney, J., Hinds, B. K., & Seow, B. L. (1999). The flattening of triangulated surfaces incorporating darts and gussets. *Computer-Aided Design*, 31(4), 249-260.
20. Liu, L., Zhang, L., Xu, Y., Gotsman, C., & Gortler, S. J. (2008). A local/global approach to mesh parameterization (ARAP). *Computer Graphics Forum (SGP)*, 27(5), 1495-1504.
21. Bennis, C., Vezien, J.-M., & Iglesias, G. (1991). Piecewise surface flattening for non-distorted texture mapping. *ACM SIGGRAPH Computer Graphics*, 25(4), 237-246.
22. Wang, C. C. L., et al. *Towards flattenable mesh surfaces* (Computer-Aided Design) and *WireWarping* length-preserving flattening.
23. Geometric (GeomCaliper). *Efficient Wall Thickness Analysis Methods* whitepaper; Analysis Situs thickness-distribution feature; voxel/sphere thickness analysis literature.
24. Blum, H. (1967). A transformation for extracting new descriptors of shape (medial axis transform). In *Models for the Perception of Speech and Visual Form*.
