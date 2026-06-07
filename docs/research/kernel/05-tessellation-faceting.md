# Tessellation and Faceting of B-rep Models as a Kernel Service

Research dossier for the Keel kernel. This file covers the kernel-side faceter: the
service that converts a B-rep (faces bounded by trimmed parametric surfaces, edges on
3D curves, shared topology) into an indexed triangle mesh suitable for display, FEA
preprocessing, 3D printing, collision, and measurement. This is distinct from the
graphics-side, view-dependent, GPU rendering of trimmed NURBS that was covered in a
prior file. Parasolid, ACIS, and OCCT all ship such a kernel faceter, and it is one of
the most heavily exercised kernel services in industrial CAD.

The faceter has a small number of hard requirements that drive its whole design:

1. The mesh must respect user tolerances (chord/sag deviation, normal/angular
   deviation, max edge length) so that downstream consumers can trust the
   approximation error.
2. The mesh must be watertight across shared edges: two faces meeting at an edge must
   emit the identical sequence of vertices along that edge, or downstream STL / FEA /
   collision will see cracks.
3. It must be robust on dirty input: gappy B-reps, near-degenerate trims, poles,
   seams, slivers.
4. It should attribute every triangle back to its source B-rep face and every boundary
   vertex back to its source edge, for picking and selective re-meshing.
5. It should be fast and ideally parallel per-face, while remaining deterministic.

---

## Thematic overview

The mainstream kernel approach factors the problem into three layers.

- Edge layer. Discretize every model edge once, in 3D, to a polyline that satisfies the
  curve tolerance and angular tolerance. Because the edge is shared by its adjacent
  faces, this single discretization is the mechanism that guarantees watertightness:
  both faces consume the same polyline.
- Face layer. For each face, pull its outer and inner trimming loops into the surface
  parameter (u,v) domain (mapping the shared edge polylines through the face's
  pcurves), then triangulate that 2D trimmed domain with a constrained method, then
  refine the interior to satisfy the surface (sag) and normal tolerances, then lift
  every (u,v) sample to 3D through the surface evaluator.
- Assembly layer. Concatenate per-face meshes into one indexed mesh with
  per-triangle face attribution and per-vertex edge attribution; weld only along shared
  edges (which already match by construction).

Three theoretical traditions feed the face layer: constrained Delaunay triangulation
and Delaunay refinement (Chew, Ruppert, Shewchuk) for guaranteed-quality 2D meshing;
curvature-adaptive parametric sampling (Piegl and Tiller and their lineage) for placing
samples where the surface bends; and anisotropic / metric-based meshing (Borouchaki,
Frey, George) for accounting for the fact that the (u,v) parameterization distorts
lengths and angles, so an isotropic 2D mesh becomes a stretched 3D mesh unless you mesh
under the surface metric (the first fundamental form).

---

## Part 1: Trimmed parametric surface triangulation

### Shewchuk, Triangle and constrained/conforming Delaunay theory

Citation: Shewchuk, J. R. (1996). Triangle: Engineering a 2D quality mesh generator and
Delaunay triangulator. In Applied Computational Geometry: Towards Geometric Engineering
(LNCS 1148, pp. 203-222). Springer.

Method: Triangle implements the planar primitives a faceter needs in the (u,v) domain:
the Delaunay triangulation, the constrained Delaunay triangulation (CDT) that forces
specified segments (the trim-loop edges) to appear in the mesh, the conforming Delaunay
variant, and Ruppert-style Delaunay refinement for quality. It uses adaptive exact
geometric predicates (orient2d, incircle) so the in-circle and orientation tests are
robust against floating-point error, which matters enormously for a kernel that must
not crash on near-degenerate trims. The CDT is the workhorse: you feed it the loop
polylines as constraint segments plus a "hole" seed point inside each inner loop, and
it returns a triangulation of exactly the trimmed region.

Guarantees: exact Delaunay/CDT; with refinement, a provable lower bound on minimum
angle (see Ruppert below). Robust predicates give exact combinatorial output.

Limitations: purely planar. It knows nothing about the surface metric, so a uniform
Triangle mesh in (u,v) is not uniform in 3D. It is a 2D library, not a faceter; the
kernel must build the domain, supply the metric-aware point set, and lift to 3D.

Kernel relevance: very high. Triangle (or a Rust reimplementation of CDT plus exact
predicates) is the natural core of Keel's face layer. The robust-predicate discipline
should be adopted wholesale: do orientation and in-circle tests with adaptive exact
arithmetic (analogous to Shewchuk's predicates, available in Rust via the robust crate
or a port).

### Chew, guaranteed-quality triangular meshing

Citation: Chew, L. P. (1989/1993). Guaranteed-quality triangular meshes; and Guaranteed-
quality mesh generation for curved surfaces. Proceedings of the 9th Annual Symposium on
Computational Geometry.

Method: Chew's first algorithm produces a constrained Delaunay mesh in which every
triangle has all angles between roughly 30 and 120 degrees by inserting circumcenters
of skinny triangles (Delaunay refinement) while keeping a uniform target edge length h.
His later work extends guaranteed-quality refinement directly onto curved surfaces by
working with surface Delaunay criteria rather than purely planar ones, which is exactly
the regime a kernel faceter lives in.

Guarantees: bounded minimum angle (about 30 degrees in the uniform-density version),
termination, size within a constant of optimal for the uniform case.

Limitations: the strict uniform-density guarantee is less flexible than graded meshing;
the curved-surface extension is more complex and assumes good surface evaluation.

Kernel relevance: Chew establishes that circumcenter insertion gives angle guarantees,
and that the idea transfers to curved domains. Keel's CAE-quality mode can adopt
graded Delaunay refinement (Ruppert/Shewchuk) but should remember Chew's curved-surface
insight: the refinement criterion should be evaluated using the surface metric, not the
raw (u,v) plane.

### Anisotropic / metric-based surface meshing (first fundamental form)

Citation: Borouchaki, H., Frey, P. J., George, P. L., and colleagues (1997-2000).
Surface mesh generation based on a Riemannian metric / Anisotropic mesh generation;
and Tristano, Owen, Canann (1998), Advancing front surface mesh generation in
parametric space using a Riemannian surface definition.

Method: The surface S(u,v) induces a metric on the (u,v) domain via the first
fundamental form, with coefficients E = S_u . S_u, F = S_u . S_v, G = S_v . S_v. A
small step (du, dv) in parameter space has squared 3D length
ds^2 = E du^2 + 2F du dv + G dv^2. Meshing the domain under this Riemannian metric M =
[[E, F],[F, G]] makes triangles that are unit/isotropic in the metric map back to
near-isotropic triangles in 3D, automatically correcting for parameterization stretch
(severe near poles and on highly non-uniform NURBS). One can superimpose a second
metric from surface curvature (principal curvatures kappa1, kappa2) so that element
size also follows a sag/normal criterion; the two metrics are intersected. Delaunay or
advancing-front meshers are then run in the metric (the Bowyer-Watson in-circle test is
replaced by an in-ellipse test under M).

Guarantees: produces meshes whose 3D element size and shape track the requested 3D
tolerance and anisotropy; the metric formalism unifies sag, normal-deviation, and
aspect-ratio control.

Limitations: needs reliable first and second derivatives of the surface; metric
intersection and anisotropic Delaunay are more complex and can be fragile near
singularities where E, F, G degenerate.

Kernel relevance: this is the principled answer to "how do I sample (u,v) so the 3D
mesh is good." Keel should compute the first fundamental form from its surface
evaluator and use a metric-aware refinement criterion, at least in the CAE-quality
mode. The display-quality mode can use a cheaper curvature heuristic.

### Frontal-Delaunay and harmonic-map surface remeshing (the Gmsh lineage)

Citation: Remacle, J.-F., Henrotte, F., Carrier-Baudouin, T., Bechet, E., Marchandise,
E., Geuzaine, C., Mouton, T. (2013). A frontal Delaunay quad/tri mesh generator using
the Linfinity norm. Int. J. Numer. Methods Eng.; and Marchandise, Remacle, Geuzaine
(2010-2014), High-quality surface remeshing using harmonic maps.

Method: Frontal-Delaunay combines the point-placement quality of advancing-front
methods with the connectivity robustness of Delaunay: candidate points are placed at
ideal frontal positions but accepted via Delaunay/empty-circle criteria, all under a
prescribed mesh-size field. The harmonic-map work addresses surfaces with bad native
parameterizations (or discrete/STL input with none) by computing a fresh conformal or
harmonic parameterization to a disk, meshing that clean 2D domain, and lifting back,
which avoids the distortion and degeneracies of CAD pcurves.

Guarantees: high element quality (near-equilateral), good gradation control; harmonic
remeshing decouples mesh quality from the often-pathological native parameterization.

Limitations: reparameterization adds cost and requires cutting high-genus / large
surfaces into disk-like charts; frontal placement is more involved than plain
refinement.

Kernel relevance: Gmsh is the reference open-source CAD-to-mesh pipeline and its
Frontal-Delaunay surface algorithm is a proven model for Keel's CAE mode. The
harmonic-map idea is the fallback for faces whose native parameterization is so
distorted that metric meshing struggles.

---

## Part 2: Chordal and angular tolerance control

### Piegl and Tiller, geometry-based triangulation of trimmed NURBS

Citation: Piegl, L. A., and Tiller, W. (1998). Geometry-based triangulation of trimmed
NURBS surfaces. Computer-Aided Design, 30(1), 11-18.

Method: The defining idea is to drive subdivision by geometry (model space), not by
parameterization. The surface is recursively subdivided into rectangular sub-patches
until each sub-patch is flat enough by a chord/sag test measured in 3D, then triangles
are formed in parameter space using the parametric corners of the sub-rectangles. The
method assumes only C0 of the parametric representation, so it tolerates the kinks and
non-uniform knot spacing of real NURBS. Trimming is handled in parameter space against
the subdivision grid.

Guarantees: facets satisfy a geometric (3D) deviation bound regardless of how the
surface is parameterized, which is the property a tolerance-driven kernel faceter needs.

Limitations: grid-based subdivision can over-refine and produce more triangles than a
fully unstructured CDT; trim handling against a grid is less clean than direct CDT of
the trimmed domain.

Kernel relevance: this is the canonical "kernel faceter" paper and its geometry-driven
(not parameter-driven) philosophy is exactly Keel's required mindset. Keel can keep the
geometric sag criterion while replacing grid triangulation with metric-aware CDT.

### Piegl and Richard, adaptive tessellation of trimmed NURBS

Citation: Piegl, L. A., and Richard, A. M. (1995). Tessellating trimmed NURBS surfaces.
Computer-Aided Design, 27(1), 16-26.

Method: An adaptive scheme that bounds triangle size using analytic bounds on the
surface derivatives (a triangle-size bound derived from second-derivative magnitude),
so the sampling density follows curvature. Trim curves are tessellated to tolerance and
their points are inserted so the tessellation conforms to the trim boundary. The work
gives practical estimates for how finely to sample to meet a chord-height epsilon.

Guarantees: chord deviation kept under a target epsilon via derivative-bound-driven
sampling; trim conformance.

Limitations: derivative bounds can be conservative (over-tessellation); 1995-era
robustness assumptions are weaker than modern exact-predicate practice.

Kernel relevance: provides the curvature-to-sampling-density formula that the display
mode needs. The sag relation underlying all of these is, for a curve of radius of
curvature R approximated by a chord of length L, the sag (chord height) is
approximately L^2 / (8R); inverting gives the max chord length for a sag tolerance
delta as L <= sqrt(8 R delta). The same R = 1/kappa from principal curvature drives
surface sag.

### Tolerance criteria as used by industrial faceters (summary of the criteria set)

Across Parasolid, ACIS, and OCCT the operative criteria are a small, stable set:

- Curve / linear deflection (chord tolerance): max distance from a curve to its chord
  approximation; controls edge polyline density. Sag relation as above.
- Surface / chordal deflection (sag): max distance from the surface to a facet plane;
  controls interior density.
- Angular / normal deviation: max angle between the surface (or curve) normals/tangents
  at the ends of a facet edge; this is what keeps silhouettes smooth on low-curvature-
  but-large faces and on cylinders where sag alone under-samples around the axis.
- Max edge length and min size: caps to bound element size for FEA and to stop runaway
  refinement near singularities.

The normal-deviation criterion is essential and independent of sag: a long thin facet
can satisfy sag yet have a large normal swing, producing visible shading bands and poor
FEA elements. A robust faceter applies the union (most-refining) of sag, normal, and
edge-length tests.

---

## Part 3: Watertight faceting across shared edges

### Stoger and Kurka, watertight tessellation using connectivity

Citation: Stoger, B., and Kurka, G. (approx. 2008). Watertight tessellation of B-rep
NURBS CAD models using connectivity information. (WSCG / journal of CAD.)

Method: Surfaces are first split into nearly flat regions. Trim curves are split at
patch borders, and intersection points on a shared boundary are transferred to the
opposite surface using a priori topological connectivity, so both adjacent faces place
vertices at identical 3D points along the common edge. This is a "shared edge sampled
once, consumed by both faces" strategy made explicit.

Guarantees: no cracks along shared edges; topologically consistent boundary
discretization between neighbors.

Limitations: relies on trustworthy connectivity (a clean B-rep with correct
edge-to-face sharing); does not by itself heal genuinely gappy input.

Kernel relevance: directly validates Keel's intended architecture, discretize the
shared edge once and feed both faces. With correct topology the watertightness is by
construction, not by post-hoc merging.

### Sealing faceted surfaces (Sandia/CGM, gap-tolerant healing)

Citation: e.g. Sandia / CUBIT-CGM authors (2010). Sealing faceted surfaces to achieve
watertight CAD models. Proceedings of the International Meshing Roundtable (Springer).

Method: A post-tessellation healing approach for when faces were faceted independently
(or imported as separate facet sets). Boundary points of neighboring faces are merged
along shared edges within a tolerance so that all boundary vertices coincide, stitching
the patches into one watertight mesh. The method reasons about which boundary loops
correspond and snaps/merges their vertices.

Guarantees: produces a watertight stitched mesh from independently faceted (and even
imperfect) faces, within the merge tolerance.

Limitations: merging within tolerance can introduce tiny slivers or move vertices off
the true geometry; correspondence detection can be ambiguous on near-coincident
boundaries; it is a repair, not a guarantee.

Kernel relevance: this is Keel's fallback path for imported / dirty B-reps where the
shared-edge-once strategy is not available because topology is missing or wrong. Keep
both: by-construction watertightness for clean kernel topology, tolerance merge for
dirty imports.

### Crack-free / no-crack surface approximation (patent and practice)

Citation: Various, including US Patent 6,707,452 (Method and apparatus for surface
approximation without cracks) and the general "T-junction avoidance" literature.

Method: When neighboring patches subdivide to different depths, the coarser side must
adopt the finer side's boundary samples (or both adopt a common boundary tessellation)
so no T-junction (hanging node) is left, since a T-junction is a crack. Boundary
tessellation is therefore computed before and independently of interior subdivision.

Guarantees: eliminates cracks from mismatched refinement depth at shared boundaries.

Kernel relevance: reinforces the rule that the edge polyline is authoritative and the
interior of each face must conform to it, never the reverse.

---

## Part 4: Quality meshing for downstream CAE

### Ruppert, Delaunay refinement

Citation: Ruppert, J. (1995). A Delaunay refinement algorithm for quality 2-dimensional
mesh generation. Journal of Algorithms, 18(3), 548-585.

Method: Maintain a constrained Delaunay triangulation of the input segments (trim
loops). Repeatedly: if a segment is encroached (some vertex lies inside its diametral
circle) split it at its midpoint; otherwise pick a skinny triangle (one whose
circumradius-to-shortest-edge ratio exceeds a threshold B) and insert its circumcenter,
unless that circumcenter would encroach a segment, in which case split the segment
instead. Terminate when no segment is encroached and no triangle is skinny.

Guarantees: with the radius-edge bound B >= sqrt(2), all angles are at least about 20.7
degrees and the algorithm terminates; output size is within a constant factor of any
mesh meeting that quality bound (size-optimal up to a constant). No new small angles are
created.

Limitations: the angle guarantee degrades and termination is not assured when input
contains small angles between segments (acute features in the trim loop or between
adjacent edges); standard fix is to "protect" small-angle vertices with concentric
shell splitting. Pure midpoint splitting can over-refine near small input features.

Kernel relevance: the backbone of Keel's CAE-quality mode. Run Ruppert/Shewchuk
refinement in the surface metric so the quality and size bounds hold in 3D. Implement
small-angle protection for sharp trim corners.

### Shewchuk, Delaunay refinement variants and termination

Citation: Shewchuk, J. R. (1997). Delaunay Refinement Mesh Generation. PhD thesis, CMU;
and Shewchuk, J. R. (2002). Delaunay refinement algorithms for triangular mesh
generation. Computational Geometry: Theory and Applications, 22(1-3), 21-74.

Method: A unified theory of Delaunay refinement covering off-center insertion (Ungor's
improvement that inserts a point nearer the short edge to reduce element count),
improved termination proofs, and the local feature size analysis that explains why
refinement terminates and why output is graded to the local feature size. Establishes
the constrained Delaunay refinement variant used when segments must be preserved.

Guarantees: stronger and clearer bounds than Ruppert; off-centers give meshes meeting
the same angle bound with fewer triangles; grading to local feature size.

Limitations: same small-angle caveat; the better bounds require careful insertion-point
selection.

Kernel relevance: adopt off-center insertion to keep CAE meshes small, and use the
local-feature-size framing to grade element size automatically near small geometric
features.

### Gmsh as a reference CAD-to-mesh pipeline

Citation: Geuzaine, C., and Remacle, J.-F. (2009). Gmsh: a three-dimensional finite
element mesh generator with built-in pre- and post-processing facilities. Int. J.
Numer. Methods Eng., 79(11), 1309-1331.

Method: Gmsh meshes each CAD surface in its parametric domain using a chosen 2D
algorithm (MeshAdapt for robustness on hard curved surfaces, Frontal-Delaunay for
quality), governed by a mesh-size field that can combine curvature, user fields, and a
background mesh. Curves are meshed first to size, then surfaces, then volumes; surface
quality is improved by local mesh modifications (edge swaps, vertex smoothing via
optimal repositioning inside the cavity of surrounding triangles). It has a built-in
OCCT-based CAD kernel for reading B-reps.

Guarantees: practical high-quality graded surface and volume meshes; not a hard
mathematical bound but strong engineering quality with curvature adaptation.

Limitations: relies on usable CAD parameterizations (hence the harmonic-map work for
bad ones); historically sensitive to dirty geometry, handled by upstream healing.

Kernel relevance: the closest open-source analogue to what Keel's CAE mode should do.
Its "size field + per-surface parametric meshing + local-op cleanup" structure is a
proven blueprint.

### NETGEN, advancing-front meshing from geometry

Citation: Schoberl, J. (1997). NETGEN: An advancing front 2D/3D mesh generator based on
abstract rules. Computing and Visualization in Science, 1, 41-52.

Method: The pipeline is special-point computation, edge following, surface meshing, then
volume meshing, with surface and volume meshing by an advancing-front method governed by
rule sets. Input is CSG (and later STL/OCC). Several mesh-optimization passes (smoothing,
swapping) follow generation.

Guarantees: good-quality, well-graded meshes in practice via advancing-front placement
plus optimization; strong volume-meshing reputation.

Limitations: advancing-front robustness depends on consistent surface input; less
formal angle guarantees than Delaunay-refinement theory.

Kernel relevance: an alternative face-meshing engine (advancing front in the metric
domain) and a model for the staged pipeline. Useful as a second strategy when Delaunay
refinement struggles.

### Guo et al., automatic high-quality surface mesh generation for CAD

Citation: Guo, J., Ding, F., Jia, X., Yan, D.-M., et al. (2019). Automatic and
high-quality surface mesh generation for CAD models. Computer-Aided Design, 109,
65-78.

Method: A fully automatic CAD-to-mesh framework that rebuilds the parametric domain for
each surface patch from an initial triangulation, retriangulates patch boundaries with
Constrained Delaunay Triangulation, and remeshes each patch independently in its
parametric domain. Because remeshing is per-patch and local, the step is parallelized
with OpenMP, and the local techniques are deterministic, avoiding cracks while
producing high-quality triangles.

Guarantees: watertight, high-quality, deterministic per-patch results suitable for
simulation.

Limitations: depends on a good initial triangulation and clean patch boundaries.

Kernel relevance: a recent, direct confirmation that per-face parametric remeshing +
shared-boundary CDT + OpenMP parallel + deterministic is the right industrial recipe,
which is exactly Keel's target architecture.

---

## Part 5: Degenerate and singular patches

### Poles, seams, and slivers

Citation: Practitioner and tooling sources, e.g. CGAL Polygon Mesh Processing manual
(degenerate-element detection and repair); multiscale meshing on the sphere literature;
and general kernel practice.

Method and issues:

- Poles (sphere/cone apex): an entire isoparametric row of (u,v) samples collapses to a
  single 3D point. Naive quads/triangles there become zero-area slivers with undefined
  normals. The standard fix is to emit a triangle fan at the pole, with one pole vertex
  as the apex shared by the ring of triangles, and to take the normal from the surface
  (analytic for sphere/cone) rather than from cross products of degenerate edges.
- Seam edges (closed surfaces, e.g. full cylinder/sphere periodic in u): the parameter
  domain has a seam where u = 0 and u = u_max map to the same 3D curve. The faceter must
  either treat the seam as a real shared edge (sample once, weld) or keep the domain
  open and identify the seam vertices, so the closed surface meshes without a visible
  slit and with consistent vertices across the seam.
- Tiny / sliver faces and short edges: faces smaller than the min-size / tolerance can
  collapse. The faceter should respect a MinSize floor (as OCCT does, see below), snap
  sub-tolerance features, and detect/remove degenerate triangles (zero area, collinear
  vertices, coincident vertices) before output.
- Degenerate trim edges: B-reps often carry zero-length "degenerate edges" marking
  poles in topology; the faceter must recognize these and not try to discretize them as
  ordinary curves.

Guarantees: none formal; this is robustness engineering.

Kernel relevance: critical. Keel must special-case poles (fans + analytic normals),
seams (identify and weld), and degenerate topology edges, and must enforce a min-size
floor. These are the cases that crash naive faceters.

---

## Part 6: Industrial faceter documentation

### Parasolid faceting (PK_TOPOL_render_facet / PK_TOPOL_facet)

Citation: Siemens Parasolid documentation, Rendering and Faceting chapters; PK_TOPOL_
render_facet, PK_TOPOL_facet, and the o_t option structs (e.g. PK_TOPOL_render_facet_
o_t). Mirrored at the q-solid Parasolid v12 docs.

Method: Faceting produces a planar-facet approximation under a user-supplied curve
tolerance (how closely the boundary of each face mesh follows the edge geometry) and a
surface tolerance (how closely each facet follows the surface). An angular tolerance
bounds the angular error between a curve and its approximating chord (an upper bound on
the sum of the two tangent-to-chord angles at the chord ends). Facet size is bounded by
a max number of sides per facet and a max facet width. Output is delivered either
streamed through the Graphical Output interface (render mode) or as facet tables: a
tabular representation in terms of connected facet, fin, and vertex indices, i.e. an
indexed mesh with topology attribution. The HOOPS Parasolid FacetTessellationKit wraps
the same option struct for application use.

Guarantees: facets meet the curve and surface tolerances; shared edges are handled via
the topology so adjacent faces are consistent (watertight by construction in the tables).

Limitations: documented behavior, internals proprietary; tolerances interact with model
tolerance (very tight requests are clamped to shape tolerance).

Kernel relevance: Parasolid's API shape is the target spec for Keel: curve tol +
surface tol + angular tol + max width/sides, and an indexed facet/fin/vertex table with
back-attribution. Keel should expose the same control surface and the same table-style
output with fins (half-edges) so attribution to faces and edges is explicit.

### ACIS faceter

Citation: Spatial 3D ACIS faceting documentation: The Faceting Algorithm; Normal
Tolerance; the af_/REFINEMENT and grid-mode references.

Method: ACIS faceting controls tessellation of solids and "generates polygonal
representations of object faces while maintaining edge consistency between adjacent
faces." It is grid-capable: in grid modes (AF_GRID_INTERIOR, AF_GRID_ONE_DIR,
AF_GRID_TO_EDGES) the faceter chooses the number of u,v isoparametric lines from the
normal tolerance, surface tolerance, max facet edge length, and max/min grid-line
counts; isolines are equally spaced for analytic surfaces and variably spaced for
splines. Normal tolerance is the max angle (degrees) between surface normals at adjacent
facet nodes. In triangulation mode (when not AF_GRID_TO_EDGES) triangles are subdivided
under surface tolerance, normal tolerance, and max edge length. Edges are faceted to be
shared by adjacent faces, again giving consistency across the shared edge.

Guarantees: meets surface, normal, and edge-length tolerances; consistent shared-edge
facets (watertight across faces).

Limitations: grid modes can over-tessellate; documented, internals proprietary.

Kernel relevance: confirms the criteria triple (surface tol, normal tol, max edge
length) and the "edge faceted once, shared by faces" rule, and shows that a grid mode is
a legitimate fast option for analytic surfaces. Keel can offer an analytic fast path
(grid) alongside the general CDT path.

### OCCT BRepMesh

Citation: Open Cascade Technology, Mesh user guide (dox/user_guides/mesh/mesh.md);
BRepMesh_IncrementalMesh class reference.

Method: An incremental algorithm that splits triangles until the result satisfies the
deflection criteria, using Watson's Delaunay algorithm. Two primary controls: linear
deflection (max distance between a curve and its tessellation) and angular deflection
(max angle between successive polyline segments). A redesign added interior controls:
DeflectionInterior (distance between triangles and the surface interior) and
AngleInterior (for B-spline surfaces, the angle between node normals along each triangle
link), plus a MinSize floor. The pipeline is six stages: data-structure creation, edge
discretization, discrete-model healing, preprocessing, face discretization,
postprocessing. Edges are discretized first (shared, ensuring watertightness), then
faces are tessellated against those constraint contours. If a requested linear
deflection is below the shape tolerance, the algorithm clamps to the shape tolerance.

Guarantees: meets linear and angular deflection where geometry allows; shared-edge
discretization yields watertight results on clean shapes.

Limitations: BRepMesh has a long history of robustness complaints on dirty/imported
geometry and self-intersecting trims; the documented redesign improved tolerance
adherence but the older versions were a frequent source of cracks and failures, which
is precisely the cautionary tale for Keel. Watson insertion is less robust than
exact-predicate CDT under degeneracy.

Kernel relevance: OCCT is the open-source baseline Keel will be compared against. Adopt
its good ideas (edge-first discretization, six-stage pipeline with an explicit healing
stage, separate interior deflection/angle controls, MinSize floor) and avoid its
weaknesses (use exact predicates and a tolerant fallback so dirty input does not crack
or fail).

---

## Part 7: Robustness on dirty CAD (gap-tolerant fallback)

### fTetWild and the epsilon-envelope philosophy

Citation: Hu, Y., Schneider, T., Wang, B., Zorin, D., and Panozzo, D. (2020). Fast
tetrahedral meshing in the wild. ACM Transactions on Graphics, 39(4). (And the earlier
TetWild, Hu et al. 2018.)

Method: Aimed at "triangle soup" input (no clean topology, with gaps, self-intersections,
duplicates). It builds a volumetric mesh while allowing small, controlled deviation from
the input surface inside an epsilon-envelope (default epsilon = bounding-box diagonal /
1000). Insertions are relaxed with a snapping tolerance permitted only because the
envelope bounds the error, and the method keeps a valid floating-point tet mesh at every
stage (no exact-rational construction, unlike TetWild), making it fast and fully robust.

Guarantees: always outputs a valid, watertight, self-intersection-free mesh whose
surface lies within epsilon of the input, regardless of input quality.

Limitations: it deviates from the exact input by up to epsilon (not surface-exact), and
it is a volumetric remesher, heavier than a surface faceter; it does not preserve sharp
CAD features unless told to.

Kernel relevance: the model for Keel's last-resort tolerant mode. When a B-rep is too
dirty to facet face-by-face (bad pcurves, gaps wider than tolerance, broken topology),
fall back to an envelope-based tolerant (re)mesher that guarantees a watertight,
self-intersection-free result within a stated epsilon, accepting bounded geometric
deviation. This is the "heal-by-remeshing" escape hatch versus the precise faceter.

### Robust conforming NURBS tessellation / no self-intersection

Citation: e.g. A robust conforming NURBS tessellation for industrial applications based
on a mesh-generation approach (CAD literature); and Robust tessellation of CAD models
without self-intersections (J. Comput. Design Eng., 2026, 13(1), 239+).

Method: Treat tessellation as a constrained mesh-generation problem that explicitly
forbids self-intersection and enforces conformity (shared boundaries match), rather than
sampling each face naively. Boundary curves are tessellated and reconciled first, then
faces are filled conformingly, with checks that the resulting 3D triangles do not
penetrate each other.

Guarantees: conforming (watertight) and self-intersection-free output on industrial
models.

Limitations: more expensive than naive faceting; self-intersection checking adds cost.

Kernel relevance: validates making conformity and non-self-intersection first-class goals
of the faceter, not afterthoughts. Keel's output contract should promise watertight and
(at least optionally) self-intersection-free meshes.

### Heal-then-mesh (CADfix-style) versus tolerant remesh

Citation: General industrial practice (CADfix, CGM/CUBIT healing, Simmetrix tolerant
meshing) plus the Sealing Faceted Surfaces work above.

Method: Two complementary strategies for dirty input. (1) Heal first: close gaps, merge
near-coincident edges, fix trim curves, rebuild topology, then run the precise faceter.
(2) Tolerant mesh: never trust the topology, mesh within a geometric tolerance band
(Simmetrix tolerant meshing, fTetWild envelope) and accept bounded deviation.

Kernel relevance: Keel should offer both. A healing pass (the OCCT "discrete model
healing" stage and the sealing/merge approach) for moderately dirty input, and an
envelope tolerant remesher for hopeless input. The faceter should report which path it
took and the achieved deviation.

---

## Part 8: Mesh data structure and attribution

A kernel faceter's output is not just triangles. The required structure, informed by
Parasolid's facet/fin/vertex tables and ACIS's mesh:

- Indexed mesh: a shared vertex array (3D position, optional surface normal, optional
  (u,v)), and a triangle index array. Welding only along shared edges keeps it compact
  and watertight.
- Per-triangle face attribution: every triangle stores the B-rep face id it came from.
  Enables picking ("which face did the user click") and selective re-faceting.
- Per-boundary-vertex edge attribution: vertices on a model edge store that edge id (and
  ideally the curve parameter t), so picking an edge and re-meshing a neighbor is
  possible.
- Fins / half-edges: Parasolid exposes "fins"; carrying half-edge adjacency in the
  output makes watertightness checkable and supports downstream topology queries.
- Persistent ids: attribution must survive local edits so that after a feature change
  only affected faces are re-faceted and the rest of the mesh (and any user data keyed to
  it) is preserved.

Kernel relevance: high. Keel should define this attributed indexed mesh as the faceter's
public output type from day one, because retrofitting attribution is painful and it is
what makes the mesh useful for picking, FEA group assignment, and incremental update.

---

## Part 9: Incremental, multi-resolution, and parallel faceting

### Incremental tessellation and re-meshing after edits

Citation: e.g. Incremental tessellation of trimmed parametric surfaces (Computer-Aided
Design, 1999, ScienceDirect S0010448599000895); plus general view-dependent LOD work.

Method: After a local model edit, only the faces (and their immediate neighbors across
shared edges) that changed are re-faceted; the rest of the attributed mesh is reused.
Multi-resolution faceting caches several tolerance levels or a refinable hierarchy so a
coarse mesh can be produced fast and refined on demand. View-dependent faceting (more a
display concern but applicable) refines based on screen-space error.

Guarantees: large speedups for interactive editing; consistent boundaries because shared
edges are re-discretized consistently and neighbors conform.

Limitations: requires the persistent attribution above and careful invalidation logic;
shared edges touched by an edit force re-faceting of both adjacent faces.

Kernel relevance: Keel's faceter should be incremental: keyed on face/edge ids, it
re-facets the dirty set and re-welds. This is a major usability win for an interactive
kernel.

### Parallel per-face faceting and determinism

Citation: Guo et al. 2019 (above, OpenMP per-patch); Simmetrix parallel meshing;
general parallel-meshing literature.

Method: Because the edge layer is computed first and shared, each face's interior
triangulation is independent and embarrassingly parallel (one task per face). The
key correctness condition is that the shared edge polylines are fixed before face tasks
start, so faces never disagree on a boundary. Determinism requires that each face task
produce identical output regardless of thread scheduling: use a fixed, total order for
point insertion (e.g. sort candidate points by a deterministic key), avoid
order-dependent tie-breaking in Delaunay (use exact predicates with a deterministic
symbolic perturbation), and seed any randomized step per-face from a fixed seed.

Guarantees: near-linear speedup in face count; bit-reproducible output if the above
disciplines are followed.

Limitations: load imbalance (one giant face dominates) needs work-stealing or face
splitting; non-deterministic Delaunay tie-breaking or floating-point reduction order can
break reproducibility if not controlled.

Kernel relevance: Keel is in Rust, so per-face parallelism via rayon is natural. Build
determinism in from the start: discretize edges first (single-threaded or deterministic),
then run face tasks with deterministic insertion order and exact predicates. Reproducible
meshes matter for testing, diffing, and regulated industries.

---

## Faceter architecture for Keel

Bringing the literature together, the recommended design:

1. Output type first. Define the attributed indexed mesh (vertices with position /
   normal / (u,v); triangles with face id; boundary vertices with edge id and curve t;
   optional half-edge/fin adjacency; persistent ids). This is the contract, modeled on
   Parasolid facet/fin/vertex tables.

2. Edge layer (watertightness by construction). Discretize every model edge once in 3D
   to satisfy curve tolerance + angular tolerance (chord sag L <= sqrt(8 R delta),
   tangent-deviation cap). Store the polyline plus per-vertex curve parameter. Both
   adjacent faces consume this exact polyline. Handle seams as shared edges; recognize
   degenerate (pole) topology edges and skip them.

3. Face layer (metric-aware CDT + refinement). Per face, in parallel:
   a. Pull each loop's shared-edge polyline into (u,v) via the pcurve.
   b. Constrained Delaunay triangulate the trimmed domain with exact predicates
      (Shewchuk/Triangle-style), with hole seeds for inner loops.
   c. Refine the interior to meet surface (sag) + normal-deviation + max-edge-length,
      evaluated under the first fundamental form metric so the 3D mesh, not the (u,v)
      mesh, meets tolerance. For CAE mode use Ruppert/Shewchuk Delaunay refinement with
      off-centers and small-angle protection for sharp trim corners (guaranteed min
      angle approx 20-30 degrees).
   d. Special-case poles (triangle fan, analytic normal) and clamp to MinSize.
   e. Lift all (u,v) to 3D through the surface evaluator; set normals analytically where
      possible.

4. Two quality modes. Display mode: curvature-heuristic adaptive sampling (Piegl-Tiller
   geometry-driven sag, Piegl-Richard derivative bounds), cheap, fast, possibly an
   analytic grid fast path (ACIS-style) for planes/cylinders/spheres. CAE mode:
   metric-based Delaunay refinement with element-quality guarantees (Frontal-Delaunay /
   Ruppert in the metric, Gmsh/NETGEN style), optional anisotropy.

5. Assembly. Concatenate face meshes; weld only along shared edges (they already match);
   verify watertightness via half-edge parity; drop degenerate triangles.

6. Robustness ladder for dirty input. (a) Clean kernel topology: shared-edge-once gives
   watertight by construction. (b) Mildly dirty: a healing stage (OCCT-style discrete
   healing, Sandia sealing/merge of boundary vertices within tolerance). (c) Hopeless
   input: an envelope tolerant remesher (fTetWild-style, deviation <= epsilon) that
   guarantees a watertight, self-intersection-free result at the cost of bounded
   geometric deviation. Always report the path taken and achieved deviation.

7. Incremental + parallel + deterministic. Key the cache on face/edge ids; re-facet only
   the dirty set plus neighbors across touched edges. Parallelize face tasks with rayon
   after the edge layer is fixed. Enforce determinism: deterministic point-insertion
   order, exact predicates with symbolic perturbation, per-face fixed seeds, so output is
   bit-reproducible for testing and regulated use.

The single most important architectural commitment is edge-first discretization: it is
what makes watertightness a property of the algorithm rather than a fragile post-process,
and every industrial faceter (Parasolid, ACIS, OCCT) and the watertight-tessellation
literature converge on it.

---

## References

1. Shewchuk, J. R. (1996). Triangle: Engineering a 2D quality mesh generator and Delaunay triangulator. Applied Computational Geometry (LNCS 1148), 203-222. https://people.eecs.berkeley.edu/~jrs/papers/triangle.pdf
2. Chew, L. P. (1993). Guaranteed-quality mesh generation for curved surfaces. Proc. 9th Symp. Computational Geometry.
3. Borouchaki, H., Frey, P. J., George, P. L. (1997-2000). Surface mesh generation based on a Riemannian metric / Anisotropic mesh generation.
4. Tristano, J. R., Owen, S. J., Canann, S. A. (1998). Advancing front surface mesh generation in parametric space using a Riemannian surface definition. Proc. International Meshing Roundtable.
5. Remacle, J.-F., et al. (2013). A frontal Delaunay quad mesh generator using the Linfinity norm. Int. J. Numer. Methods Eng. https://onlinelibrary.wiley.com/doi/abs/10.1002/nme.4458
6. Marchandise, E., Remacle, J.-F., Geuzaine, C. (2010). High-quality surface remeshing using harmonic maps. Int. J. Numer. Methods Eng. https://www.ljll.fr/~frey/papers/meshing/Remacle%20J.F.,%20High%20quality%20surface%20remeshing%20using%20harmonic%20maps.pdf
7. Piegl, L. A., Tiller, W. (1998). Geometry-based triangulation of trimmed NURBS surfaces. Computer-Aided Design, 30(1), 11-18. https://www.sciencedirect.com/science/article/abs/pii/S001044859700047X
8. Piegl, L. A., Richard, A. M. (1995). Tessellating trimmed NURBS surfaces. Computer-Aided Design, 27(1), 16-26.
9. Stoger, B., Kurka, G. Watertight tessellation of B-rep NURBS CAD models using connectivity information. https://www.researchgate.net/publication/221297128
10. Sealing faceted surfaces to achieve watertight CAD models. Proc. International Meshing Roundtable (Springer). https://link.springer.com/chapter/10.1007/978-3-642-15414-0_11
11. Method and apparatus for surface approximation without cracks. US Patent 6,707,452.
12. Ruppert, J. (1995). A Delaunay refinement algorithm for quality 2-dimensional mesh generation. Journal of Algorithms, 18(3), 548-585.
13. Shewchuk, J. R. (1997). Delaunay Refinement Mesh Generation. PhD thesis, CMU. http://www.cs.cmu.edu/~quake-papers/delaunay-refinement.pdf
14. Shewchuk, J. R. (2002). Delaunay refinement algorithms for triangular mesh generation. Computational Geometry: Theory and Applications, 22(1-3), 21-74.
15. Geuzaine, C., Remacle, J.-F. (2009). Gmsh: a three-dimensional finite element mesh generator. Int. J. Numer. Methods Eng., 79(11), 1309-1331. https://gmsh.info/doc/preprints/gmsh_paper_preprint.pdf
16. Schoberl, J. (1997). NETGEN: An advancing front 2D/3D mesh generator based on abstract rules. Computing and Visualization in Science, 1, 41-52. https://link.springer.com/article/10.1007/s007910050004
17. Guo, J., et al. (2019). Automatic and high-quality surface mesh generation for CAD models. Computer-Aided Design, 109, 65-78. https://jianweiguo.net/publications/papers/2019_CAD_CADMeshing.pdf
18. CGAL Polygon Mesh Processing user manual (degenerate-element detection/repair). https://doc.cgal.org/latest/Polygon_mesh_processing/index.html
19. Siemens Parasolid documentation: Rendering and Faceting; PK_TOPOL_render_facet / PK_TOPOL_facet. http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.49.html and fd_chap.55.html
20. Spatial 3D ACIS documentation: The Faceting Algorithm; Normal Tolerance; grid modes. http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_difacetfa.htm
21. Open Cascade Technology: Mesh user guide / BRepMesh_IncrementalMesh. https://github.com/Open-Cascade-SAS/OCCT/blob/master/dox/user_guides/mesh/mesh.md
22. Hu, Y., Schneider, T., Wang, B., Zorin, D., Panozzo, D. (2020). Fast tetrahedral meshing in the wild. ACM Trans. Graph., 39(4). https://arxiv.org/abs/1908.03581
23. Robust tessellation of CAD models without self-intersections (2026). J. Comput. Design Eng., 13(1), 239+. https://academic.oup.com/jcde/article/13/1/239/8383411
24. A robust conforming NURBS tessellation for industrial applications based on a mesh-generation approach. https://www.researchgate.net/publication/271016604
25. Incremental tessellation of trimmed parametric surfaces (1999). Computer-Aided Design. https://www.sciencedirect.com/science/article/abs/pii/S0010448599000895
26. Simmetrix parallel meshing technologies. https://www.simmetrix.com/index.php/technologies/parallel-meshing
