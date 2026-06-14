# 63. Curvature-adaptive curved tessellation vs the mass gate: decoupling the self-consistency check from mesh density

## Title and scope

This dossier is the implementation-grade literature foundation for one specific defect in Keel's self-consistency gate: **a correct curved result can FALSE-DECLINE because the tessellated `mesh_volume` disagrees with the exact `mass_properties` by more than the 2% curved band, and the disagreement is an artifact of mesh density, not a real error.** Keel's gate is `mass == mesh` within a 2% curved band. `mass_properties` is the exact analytic / certified-quadrature side (dossier 60). `mesh_volume` tessellates each face on a per-face parameter grid and sums signed tetrahedra by the divergence theorem on the mesh. Cone and other curved faces tessellate ~0.5 to 1.7% off the true mass, worse near the apex, so a coarse mesh can push a correct body outside the band (false DECLINE), while a uniformly denser blanket mesh would regress the optimization leg (boxes at 0.9 ms). The tension is real and structural: the gate's two sides are not symmetric. One side is exact; the other is a density-dependent approximation that the gate then treats as ground truth for itself.

The scope is the **tessellation side of the gate and the gate's own design**, not the mass integrator (dossier 60 owns that), not the SSI seam (58), not the assembly stitch (59). Concretely this dossier answers: (1) the explicit sagitta / chord-error to segment-count rule per surface (cone, cylinder, sphere, torus, NURBS) and how chord error maps to VOLUME error including the inscribed-undercount and the area-preserving midpoint correction; (2) the apex / pole fan-triangulation error that dominates the cone case, and how to bound or kill it; (3) the highest-leverage move, **gate against an analytic reference where one exists instead of against the mesh**, argued through to the independent oracle that keeps the gate non-vacuous; (4) the perf budget, with what Parasolid / ACIS / OCCT actually use for mass versus display (they do not use the display mesh for mass, and that is the load-bearing finding).

House style: **no em-dashes anywhere, machine-checked; en-dashes are not used as separators either, ranges are written with "to".** Structure mirrors files 01 to 60: annotated source blocks (Citation / Method / Limitations / Kernel relevance), explicit verified-vs-claimed labels, patent flags, and a closing ADOPT / AVOID / INVESTIGATE with a first milestone, a follow-up ladder, and a DECLINE-forever set.

---

## The load-bearing conclusion, stated once

> **The mass-vs-mesh gate as currently wired is self-undermining for curved faces: it compares an EXACT number against a density-dependent approximation and then uses the 2% band to absorb the approximation's error, which means the band is doing double duty as both a real-error detector AND a tessellation-slop budget. Those two jobs are in direct conflict. The fix is to stop comparing exact-against-mesh and instead make the gate a THREE-way consistency check where each curved face has, in order of preference: (i) an EXACT analytic volume / area (natural quadrics and their bands: cylinder, cone, frustum, sphere zone, torus), in which case the gate is `analytic_mass == analytic_reference` and is independent of any mesh; (ii) a BOUNDED-error adaptive mesh whose chord tolerance is chosen so the proven volume-error bound is a fixed fraction (say 1/10) of the band, with the area-preserving midpoint correction applied so the coarse mesh's volume is second-order-exact rather than first-order-low; (iii) for high-degree NURBS with no closed form, a true chord-error-driven adaptive mesh whose cost is paid only on those faces. The independent oracle that prevents (i) from being vacuous is that the analytic reference is computed by a DIFFERENT route than `mass_properties`: `mass_properties` is the Green-slab boundary-flux integrator (dossier 60), the reference is the textbook closed form (Pappus / frustum / spherical-zone / Pappus-torus), so agreement is a genuine cross-check of two independent derivations, not a tautology. The mesh stays as a THIRD, looser witness (a coarse sanity mesh with a wider band) that catches gross topology errors the analytic pair would both miss, but it is no longer the tight gate. Net: the optimization leg keeps its coarse display mesh, the gate gets TIGHTER not looser, and the cone false-decline disappears because the cone tip is checked against `(1/3) pi r^2 h` to 1e-9, not against a fan of apex triangles that is 1.7% low.**

The defensible reason, in one paragraph. A polyhedral inscribed approximation of a curved convex solid systematically **under-counts** volume (every chord cuts inside the arc), and the under-count is second order in the chord angle: for a circle of radius `r` approximated by an inscribed regular `n`-gon, area is `(n/2) r^2 sin(2*pi/n) = pi r^2 (1 - (2*pi^2)/(3 n^2) + O(n^-4))`, so the relative volume error of a swept curved solid falls as `1/n^2` and is always NEGATIVE for an inscribed mesh (Han; SAS DO Loop; verified). That is exactly the observed "cone runs 0.5 to 1.7% low." The error is not random noise to be averaged out by the band; it is a deterministic, signed, computable bias. Anything deterministic and computable should be either (a) eliminated by comparing against the exact closed form instead of the biased mesh, or (b) corrected analytically (multiply the inscribed area by the known `(2*pi/n) / sin(2*pi/n)` factor, the area-preserving correction, making a coarse mesh volume-exact to fourth order). Using a 2% band to swallow a known `1/n^2` bias is the worst of both: it tolerates real errors up to 2% AND still false-declines when the bias plus a real sub-2% effect crosses the line. The literature on mass properties in production kernels is unanimous on the structural point: **mass properties are computed by exact / analytic integration over the B-rep, NEVER from the display triangulation** (Lee-Requicha; the constant-density analytic-inertia line; verified). Keel already does this on the mass side. The gate just has not finished the thought on the mesh side.

---

## 1. Curvature-adaptive density: the sagitta to segment-count rule and how chord error maps to VOLUME error

### 1.1 The sagitta and the per-circle segment-count formula

> **Citation.** Sagitta (geometry), Wikipedia, https://en.wikipedia.org/wiki/Sagitta_(geometry) ; Math Open Reference, "Sagitta", https://mathopenref.com/sagitta.html ; circle-tessellation-with-tolerance derivation, GameDev.net forum thread, https://gamedev.net/forums/topic/555281-circle-tessellation-with-tolerance/ ; "A Simple Circle Discretization Algorithm With Applications", arXiv:1308.2581, https://arxiv.org/pdf/1308.2581 .
> **Method / Content.** The **sagitta** `s` is the perpendicular distance from the midpoint of a chord to the arc it subtends. For a circle of radius `r` and a chord subtending half-angle `theta` (so the full segment angle is `2*theta` and there are `n = 2*pi / (2*theta) = pi/theta` segments around the full circle), the exact relations are:
> ```
> chord half-length  c/2 = r sin(theta)
> apothem (midpoint to center)  a = r cos(theta)
> sagitta  s = r - a = r (1 - cos(theta))
> ```
> The sagitta IS the chord-height tolerance (max distance from the polyline to the true arc). Inverting `s = r(1 - cos theta)` for the angle that meets a tolerance `tol`:
> ```
> cos(theta) = 1 - tol/r
> theta = acos(1 - tol/r)
> n_segments = ceil( pi / acos(1 - tol/r) )              (full circle)
> ```
> For `tol << r` the small-angle expansion `1 - cos theta ~ theta^2 / 2` gives the working approximation
> ```
> theta ~ sqrt(2 tol / r),    n ~ pi / sqrt(2 tol / r) = pi sqrt(r / (2 tol))
> ```
> So segment count scales as `sqrt(r / tol)`: doubling the radius needs `sqrt(2)` more segments, halving the tolerance needs `sqrt(2)` more. This is the canonical sag-to-density rule used by every faceting engine. The dual control is the **angular** bound: cap `theta` directly at `theta_max` (OCCT's angular deflection), giving `n = ceil(2*pi / theta_max)` independent of radius, used to keep small high-curvature features from being under-faceted when `tol` alone would allow a coarse fan.
> **Limitations.** This is the planar-circle rule (cylinder cross-section, sphere / cone latitude circle, torus tube and centerline circles). A general NURBS has no constant `r`; its local equivalent uses the radius of curvature `rho = 1 / kappa` in place of `r`, which varies along the curve, so the global `n` must use the MINIMUM `rho` (max curvature) or the density must be applied per-span adaptively (section 1.5).
> **Kernel relevance.** This is the exact formula Keel's `tessellate_cone` / `tessellate_torus` density should derive from, replacing any fixed segment count. Every curved Keel surface reduces to circles in at least one parameter direction (cylinder / cone / sphere / torus are all surfaces of revolution), so the azimuthal segment count is `ceil(pi / acos(1 - tol/r_max))` with `r_max` the largest circle radius on the face. **Verified** (elementary trigonometry, matches the cited derivations).

### 1.2 The per-surface density rules (cone, cylinder, sphere, torus)

> **Method / Content (synthesis, claimed as the Keel rule).** Apply section 1.1 per parameter direction, using the largest radius in that direction so the tolerance holds everywhere on the face.
> - **Cylinder** radius `R`, height `h`: azimuthal `n_u = ceil(pi / acos(1 - tol/R))`; axial direction is RULED (straight), so `n_v = 1` (a single quad strip in `v` is exact in the axial direction). The cylinder needs density in ONE direction only.
> - **Cone** half-angle `alpha`, slant `L`: the latitude circle radius grows linearly with slant distance, `r(v) = v cos(alpha)`, max at the rim `r_max = L cos(alpha)`. Azimuthal `n_u = ceil(pi / acos(1 - tol/r_max))`; the rulings are straight so `n_v = 1` in slant EXCEPT for the apex fan (section 2). The cone is the worst case for the gate precisely because the apex fan, not the chord error, dominates.
> - **Sphere** radius `R`: BOTH directions are curved. Longitude (azimuth) `n_u = ceil(pi / acos(1 - tol/R))`; latitude (polar) `n_v = ceil(pi / (2 acos(1 - tol/R)))` over the half-meridian, with pole handling (section 2). The sphere is the case where a naive grid wastes the most triangles near the poles (latitude circles shrink to zero but keep the full `n_u`), so adaptive `n_u(v) = ceil(pi / acos(1 - tol/(R sin(phi))))` per latitude band cuts triangle count without loss.
> - **Torus** major `R`, minor `a`: tube direction uses `r = a`, `n_tube = ceil(pi / acos(1 - tol/a))`; centerline direction uses the OUTER radius `R + a` (largest circle), `n_center = ceil(pi / acos(1 - tol/(R+a)))`. Both directions curved, neither degenerate (no pole, no apex), so the torus is the CLEANEST curved case for a mesh: a product grid with these two counts meets `tol` everywhere with no singular correction.
> **Limitations.** These give a chord (distance) bound, not directly a volume bound; section 1.3 converts. The cone and sphere have a singular direction (apex, pole) where the chord rule is necessary but not sufficient (section 2). The torus assumes the minor radius does not exceed the major (a ring torus); a self-intersecting horn / spindle torus is excluded upstream.
> **Kernel relevance.** Drop-in density for `tessellate_*`. **Claimed** (direct application of 1.1; validate triangle counts against the current fixed grid).

### 1.3 From chord error to VOLUME error: the inscribed under-count and the area-preserving correction

> **Citation.** K. P. Han, "Pi and Archimedes' Polygon Method", Dartmouth, https://math.dartmouth.edu/archive/m56s13/public_html/Han_proj.pdf ; R. Wicklin, "Polygons, pi, and linear approximations", SAS The DO Loop, https://blogs.sas.com/content/iml/2020/03/11/polygons-pi-linear-approx.html ; n-gon area identity, UTSA, https://mathresearch.utsa.edu/wiki/index.php?title=Properties_of_Polygons .
> **Method / Content.** The inscribed regular `n`-gon in a circle of radius `r` has area
> ```
> A_n = (n/2) r^2 sin(2*pi/n) = pi r^2 * [ sin(2*pi/n) / (2*pi/n) ]
> ```
> The bracket is `sinc(2*pi/n) = 1 - (2*pi^2)/(3 n^2) + O(n^-4) < 1`, so **an inscribed polygon ALWAYS under-counts area, and the relative under-count is `~ (2*pi^2)/(3 n^2)`**, second order in `1/n` and strictly negative. Numbers: `n = 12` gives `-1.14%`; `n = 16` gives `-0.64%`; `n = 24` gives `-0.29%`; `n = 32` gives `-0.16%`. This is EXACTLY the observed cone band offset of 0.5 to 1.7%: a cone faceted with `n` in the teens to low-20s sits 0.3 to 1.1% low from the latitude circles alone, before the apex fan adds more. The under-count carries directly into swept volume: a solid of revolution meshed with an inscribed azimuthal `n`-gon has its cross-sectional area under-counted by the same `sinc` factor, so the volume is low by `~ (2*pi^2)/(3 n^2)`.
> A **circumscribed** polygon (vertices where edge midpoints touch the circle, i.e. the apothem equals `r`) OVER-counts by the reciprocal factor `(2*pi/n) / sin(2*pi/n) - 1 ~ +(2*pi^2)/(3 n^2)`. So inscribed is a lower bound and circumscribed an upper bound on the true volume, bracketing it.
> The **area-preserving (midpoint) correction.** Because the inscribed under-count factor `sinc(2*pi/n)` is KNOWN in closed form, a coarse inscribed mesh can be made volume-exact to fourth order by scaling: multiply the meshed cross-sectional area (or equivalently the radial coordinate of each ring by `sqrt`) by
> ```
> k_correct(n) = (2*pi/n) / sin(2*pi/n) = 1 / sinc(2*pi/n)
> ```
> The corrected coarse-mesh volume then matches the true swept volume to `O(n^-4)`: `n = 12` corrected drops from `-1.14%` to about `+0.013%`, a 90x error reduction at ZERO extra triangles. Equivalently, place the ring vertices at the **radius-preserving radius** `r' = r * sqrt(k_correct)` so the polygon's area equals the circle's exactly (the "equal-area polygon"), making the mesh second-order-exact for the gate without densifying.
> **Limitations.** The correction is exact for a circular cross-section swept rigidly (cylinder, cone band, sphere zone if applied per latitude, torus tube). For a non-circular curved profile (general NURBS revolve, or a non-revolution NURBS patch) the `sinc` factor is not the right correction; there the per-span chord bound (section 1.5) must actually densify. The correction also assumes the mesh is used for VOLUME (a global integral); it does NOT make the mesh geometrically accurate for display or for boolean surface tests, so it must be a mass-mesh-only transform, kept separate from the display mesh.
> **Kernel relevance.** This is the single cheapest fix to the cone false-decline that keeps a mesh: apply `k_correct(n)` to the `mesh_volume` ring areas of revolution faces, turning a `-1.14%` coarse cone into a `+0.01%` one, well inside even a 0.5% band. But it is strictly dominated by section 3 (gate against the analytic reference and skip the mesh entirely for these faces), so it is the FALLBACK for faces that have no closed form yet have a circular cross-section. **Verified** under-count and bracket; the closed-form correction factor is **verified** (the `sinc` identity is exact); the recommendation to apply it as a mass-mesh-only transform is **claimed**.

### 1.4 The chord-error-to-deviation bound for general surfaces (Filip-Magedson-Markot)

> **Citation.** D. Filip, R. Magedson, R. Markot, "Surface algorithms using bounds on derivatives", *Computer Aided Geometric Design* 3(4):295-311, 1986, https://dl.acm.org/doi/10.1016/0167-8396(86)90005-1 (publisher record); summarized widely, e.g. tessellation-interval literature below.
> **Method / Content.** For a `C^2` parametric surface `S(u, v)`, the deviation of the surface from its planar triangle approximation over a parameter rectangle is bounded by the SECOND derivatives. The classic Filip-Magedson-Markot bound: over a parameter step `(du, dv)`, the chord (flatness) deviation `e` satisfies
> ```
> e <= (1/8) ( M_uu du^2 + 2 M_uv du dv + M_vv dv^2 )
> ```
> where `M_uu = max ||S_uu||`, `M_uv = max ||S_uv||`, `M_vv = max ||S_vv||` over the patch. Inverting for a target `tol` gives the parameter step (hence triangle count) that guarantees deviation `<= tol` everywhere, WITHOUT evaluating the surface at every candidate point. First derivatives bound the size criterion (edge lengths), second derivatives bound the deviation criterion (flatness). This is the rigorous generalization of the circle sagitta rule: for a circle `||S_uu|| = r`, and the bound reduces to `e <= (1/8) r du^2`, i.e. `du <= sqrt(8 tol / r)`, matching `theta ~ sqrt(2 tol/r)` up to the factor convention (the `1/8` is per full chord, the `1/2` was per sagitta).
> **Limitations.** The bound is conservative (uses global max second derivatives over the patch, so it over-tessellates where curvature is locally lower); adaptive subdivision (1.5) recovers the slack. It is a DEVIATION (distance) bound, so converting to a volume bound still goes through the `1/n^2` swept-error relation of 1.3. It needs the surface's second-derivative magnitudes, which for natural quadrics are closed form (so no sampling needed) and for NURBS come from the control-net second differences.
> **Kernel relevance.** This is the verified, citable warrant for the per-surface density rules of 1.2 and the principled way to set NURBS density (1.5). For natural quadrics, `M_uu` etc. are analytic, so the triangle count is a closed-form function of `tol` and the surface parameters, computed once with no sampling. **Verified** (the bound is the standard Filip-Magedson-Markot result).

### 1.5 NURBS: per-span chord-driven adaptive mesh

> **Citation.** S.-W. Cheng et al. / J. Zheng, T. Sederberg, "Estimating tessellation parameter intervals for rational curves and surfaces", *ACM TOG* 19(1), 2000, https://dl.acm.org/doi/10.1145/343002.343034 ; "Adaptive Tessellation for Trimmed NURBS Surface", Eurographics, https://diglib.eg.org/server/api/core/bitstreams/3faf6a3d-6dc3-454a-b654-b01cd65ab1f2/content ; recursive flatness subdivision background, NVIDIA GPU Gems 2 ch.7, https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-7-adaptive-tessellation-subdivision-surfaces .
> **Method / Content.** For a NURBS surface with no closed form, the density must be driven by a real flatness test, applied PER knot span / per Bezier sub-patch:
> 1. Decompose to Bezier patches (knot insertion).
> 2. Per patch, estimate flatness by the control-net deviation from its bilinear corners (the max distance of an interior control point to the corner-bilinear plane), or by the Filip-Magedson-Markot second-derivative bound over the span.
> 3. If flatness `> tol`, subdivide the span at its midpoint and recurse; else emit triangles. Halving a span divides the deviation by ~4 (second order), so depth is logarithmic in `tol`.
> Zheng-Sederberg give larger guaranteed-flat tessellation intervals (fewer triangles for the same tolerance) by a tighter rational-surface bound. Non-uniform / curvature-adaptive subdivision generates far fewer polygons than a uniform grid (the cited GPU work reports ~40% fewer for equal deviation) by concentrating triangles where curvature is high.
> **Limitations.** Adaptive subdivision needs a per-span error estimate and a recursion cap; if the cap is hit before `tol` is met (pathological high-degree NURBS), the face must DECLINE rather than emit an under-resolved mesh (DECLINE-never-WRONG). The volume error of the adaptive mesh still inherits the `1/n^2` swept bias of 1.3 and does NOT get the closed-form `sinc` correction (the cross-section is not a circle), so the tolerance must be set tighter to leave headroom, OR the NURBS face must be gated by a different oracle (section 3 admits these have no exact closed form, so they are the genuine perf-cost bucket).
> **Kernel relevance.** This is the only bucket where Keel genuinely must pay a chord-error-driven adaptive mesh cost. It is bounded to the NURBS faces (cross-cutting bucket c). The natural quadrics never need it. **Verified** (adaptive flatness subdivision is standard; Zheng-Sederberg intervals are the tighter rational bound).

---

## 2. Apex / pole / high-curvature special handling (the apex triangles dominate the cone error)

### 2.1 Why the apex / pole fan is the dominant cone-mesh error

> **Citation.** Fan triangulation, Wikipedia, https://en.wikipedia.org/wiki/Fan_triangulation ; cone parameterization and apex Jacobian, dossier 60 section 1.2 (Keel internal); "Improved Mesh Processing using Distorted Pole Spherical Coordinates", arXiv:2306.04625, https://arxiv.org/pdf/2306.04625 ; spherical parameterization pole distortion, arXiv:1810.09031, https://arxiv.org/pdf/1810.09031 .
> **Method / Content.** At a cone apex (and a sphere pole) the parameter grid is SINGULAR: the whole `u`-circle collapses to a single point, and a grid mesh produces a **fan** of long thin triangles from the apex to the first latitude ring. The volume error of this fan is the volume between the cone of straight rulings (correct: the cone IS ruled, so straight edges in `v` are exact) and the polygonal cap of the first ring (incorrect: the cap is a flat polygon, not the circular disk). For a cap of radius `r_1` at the first ring, the meshed cap area is the inscribed `n`-gon `A_n = pi r_1^2 sinc(2*pi/n)`, under-counting by the same `sinc` factor as 1.3, so the apex tip volume is under-counted by `~(2*pi^2)/(3 n^2)`. The apex fan does NOT add a NEW error class; it adds the SAME `sinc` under-count concentrated in the tip, where the cross-section is smallest and the relative effect on the tip's own volume is largest. The "worse near the apex" observation is the tip's small volume being dominated by its `sinc`-biased cap, plus any extra error if the first ring is placed by a `v`-uniform grid that puts few rings near the tip.
> **Limitations.** This assumes the cone rulings are meshed straight (correct for a true cone). If the tip is rounded by a blend (sphere / torus fillet) it is a different face with its own curvature, not an apex (dossier 40 / 60). A sphere pole is WORSE than a cone apex because the meridians are also curved (not ruled), so the pole fan has curvature error in BOTH directions.
> **Kernel relevance.** Confirms the cone false-decline is the `sinc` under-count, dominated at the tip, and therefore CURABLE by the same closed-form correction (1.3) or, better, by not meshing the tip at all and gating it against `(1/3) pi r^2 h` (section 3). **Verified** (fan geometry and the `sinc` cap under-count are elementary).

### 2.2 Bounding and correcting the apex / pole contribution

> **Method / Content (claimed, the Keel recipe).** Three options, in increasing quality:
> - **Densify the cap fan.** Increase `n_u` near the apex so the cap's `sinc` under-count drops below the band. Cost: more triangles, and it never reaches exact. Worst option; keep only as a last resort for an un-classified face.
> - **Equal-area cap correction.** Apply the `k_correct(n) = (2*pi/n)/sin(2*pi/n)` scale (1.3) to the apex cap's contribution to `mesh_volume`, making the tip volume fourth-order exact at the existing `n`. Cheap, keeps the mesh, no closed form needed beyond the factor.
> - **Analytic cap substitution (recommended).** Replace the apex tip's meshed contribution with its EXACT closed form: the cone tip from `v = 0` to the first ring `v_1` is a cone of volume `(1/3) pi (v_1 cos alpha)^2 (v_1 sin alpha)`; for a sphere pole cap the exact spherical cap volume `(pi h^2 / 3)(3R - h)` with `h` the cap height. The mesh covers only the well-conditioned band away from the singularity; the singular cap is handled by the formula. This is the mesh analog of the dossier-60 apex / pole ANCHOR (which does the same thing for the analytic boundary-flux integrator). It is exact and adds no triangles.
> **Limitations.** Analytic cap substitution requires KNOWING the face is a true quadric tip (classification), which Keel has (surface type is explicit). For an unknown / NURBS tip it falls back to the equal-area correction or densify. The substitution must be consistent with how the mass side closes the apex (dossier 60), so the two sides remain comparing the same region.
> **Kernel relevance.** The recommended apex handling is "do not let the mesh decide the tip": substitute the closed form for the singular cap, mesh only the conditioned band. This both removes the dominant error and aligns the mesh side with the analytic-anchor mass side. **Claimed** (construction); the closed forms it substitutes are **verified** (standard cone / spherical-cap volumes).

---

## 3. Gate against an ANALYTIC reference instead of the mesh (the highest-leverage recommendation)

### 3.1 The argument: the gate's two sides are asymmetric, so the mesh should not be the tight side

> **Citation.** Y. T. Lee, A. A. G. Requicha, "Algorithms for computing the volume and other integral properties of solids", *Comm. ACM* 25(9):635-650, 1982, https://dl.acm.org/doi/10.1145/358628.358643 ; B. C. Moulton, D. F. Hunsaker, "Analytic Solutions for Volume, Mass, Center of Gravity, and Inertia ...", *Aerospace* 11(6):492, 2024, https://www.mdpi.com/2226-4310/11/6/492 ; dossier 60 (Keel curved-mass integrator); dossier 35 (oracles).
> **Method / Content.** Lee-Requicha frame mass-property computation as "simple integrands on complicated domains" and establish the production-kernel doctrine: integral properties are computed by exact integration over the B-rep representation, with the modeling scheme dominating the algorithm design. The display triangulation is a SEPARATE, lossy artifact. The structural point for Keel's gate: a self-consistency gate is only meaningful if its two sides are INDEPENDENT and ideally both accurate. Today's `mass == mesh` pairs an exact side (`mass_properties`, Green-slab, dossier 60) with a biased side (`mesh_volume`, inscribed `sinc` under-count). The biased side is the limiting factor, and the band must be widened to tolerate the bias, which DEGRADES the gate's ability to catch real sub-band errors. The fix is to make BOTH tight sides analytic where a closed form exists, and demote the mesh to a third, loose witness:
> ```
> Side A (was "mass"):  Green-slab boundary-flux integrator over the trimmed face   [dossier 60]
> Side B (new tight):   textbook closed form for the face's exact volume/area        [analytic reference]
> Side C (loose witness): coarse display mesh, wide band, catches gross topology     [demoted]
> ```
> Gate: `A == B` to 1e-9 (the tight gate, mesh-independent), AND `C` within a loose band (say 5%) of `A` (the topology sanity witness). The cone tip is now `Green-slab == (1/3) pi r^2 h` to 1e-9, which PASSES, while the coarse mesh at 1.7% low is still inside the loose 5% witness band, so it does not decline.
> **Limitations.** Side B (the closed form) exists only for the faces in cross-cutting bucket (a): natural quadrics and their bands. For bucket (b) and (c) there is no closed form, so Side B is unavailable and the gate falls back to `A == bounded-mesh` with the chord-tolerance-set-to-fraction-of-band rule (1.3 to 1.5). The closed form must be selected by the same region the integrator used (same trim), or A and B compare different regions.
> **Kernel relevance.** This is the highest-leverage change and the recommended one. It directly kills the cone (and frustum, cylinder, sphere-zone, torus) false-decline, makes the gate TIGHTER (1e-9 not 2%) for the bucket-a faces, and lets the display mesh stay coarse (perf leg preserved). **Verified** (the production-kernel doctrine that mass uses exact integration not the display mesh, Lee-Requicha); the three-side gate design is **claimed**.

### 3.2 The independent oracle that keeps the gate non-vacuous

> **Citation.** Pappus's centroid theorem, https://en.wikipedia.org/wiki/Pappus%27s_centroid_theorem ; surface-of-revolution closed forms, https://mathworld.wolfram.com/SurfaceofRevolution.html ; dossier 35 (validated-numerics oracles); dossier 60 section 1.3 (cone / frustum / spherical-zone oracle).
> **Method / Content.** A self-consistency gate is VACUOUS if both sides are the same computation in two notations (they would agree even when both are wrong). The non-vacuity guarantee here is that **Side A and Side B are derived by genuinely different routes**:
> - **Side A** (`mass_properties`) is the divergence-theorem boundary-flux reduction: pick `F` with `div F = 1`, integrate `F . n` over the trimmed face boundary by GL quadrature (dossier 60). It "knows" only the surface parameterization and the trim loop.
> - **Side B** (the analytic reference) is the textbook solid-of-revolution formula derived from Pappus's theorem (volume = path-length-of-centroid times area, or the direct frustum / spherical-zone integral). It "knows" only the surface TYPE and its scalar parameters (radius, half-angle, two `v`-bounds, azimuth sweep).
> These share no code and no derivation: one is a numerical boundary integral over an explicit loop, the other is a closed-form plug-in of scalar parameters. Agreement to 1e-9 is therefore a real cross-check (it catches a bug in either the quadrature OR the parameter extraction), not a tautology. The independent third leg, Side C (the coarse mesh), guards the case both analytic sides could share: a WRONG trim region (if A integrated the wrong loop AND B was handed the wrong `v`-bounds, both could agree on a wrong region). The mesh, built from the ACTUAL face triangulation, would then disagree, catching the shared-region error. So the three sides are mutually non-redundant: A vs B catches integration / formula bugs, C vs A catches region / topology bugs.
> **Limitations.** Side B requires the face to be a recognized analytic type with extractable parameters; a recovered / refit NURBS that is "really" a quadric but stored as NURBS does not get Side B unless re-recognized. The mesh witness C must be coarse enough to be cheap yet fine enough to distinguish a wrong region (a `>5%` region error), which the loose band handles.
> **Kernel relevance.** This is the answer to "what prevents the gate from being vacuous": the analytic reference is computed from surface TYPE + scalars by Pappus / frustum formulas, a different derivation than the boundary-flux integrator, and the coarse mesh remains as the independent topology witness. **Verified** (Pappus and the closed forms; the independence argument is sound); the three-leg non-redundancy is **claimed** (the recommended gate architecture).

### 3.3 Which faces get Side B (the exact-reference catalog)

> **Method / Content (the deliverable mapping).**
> - **Cylinder band** (radius `R`, height `h`, azimuth `[u0,u1]`): exact volume contribution and lateral area `(u1-u0) R h` for the wedge, `2 pi R h` full. Side B available.
> - **Cone / frustum** (`v`-bounds `v0,v1`, half-angle `alpha`): tip `(1/3) pi r^2 h`, frustum `(pi h/3)(R^2 + R r + r^2)`, lateral `pi (R+r) l`. Side B available (the cone case that currently false-declines).
> - **Sphere zone / cap** (radius `R`, polar bounds): zone area `2 pi R h`, cap volume `(pi h^2/3)(3R - h)`, spherical-sector volume `(2/3) pi R^2 h`. Side B available.
> - **Torus** (major `R`, minor `a`, full or banded): full volume `2 pi^2 R a^2`, area `4 pi^2 R a` (Pappus); a partial torus band scales by the swept-angle fractions. Side B available.
> - **Plane** faces: trivially exact (planar polygon area / signed-tet volume), no curved band needed; the mesh IS exact here, so plane faces never false-decline and stay on the mesh path.
> - **NURBS** (general, high degree): NO closed form. Side B unavailable. Falls to bucket (b)/(c): bounded adaptive mesh with chord-tolerance-set-to-fraction-of-band, accepting the perf cost on those faces only.
> **Kernel relevance.** This catalog IS cross-cutting bucket (a). Every natural-quadric face and its azimuth / polar bands has an exact Side B, so the entire false-decline-prone set (cone, frustum, cylinder wedge, sphere zone, torus band) is moved off the mesh-as-tight-gate and onto the analytic pair. **Verified** (all closed forms standard).

---

## 4. The perf budget: density vs the hot loops, and what reference kernels actually do

### 4.1 Reference kernels do NOT use the display mesh for mass properties

> **Citation.** ACIS faceting docs, "Surface Tolerance" and "Normal Tolerance", http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/FCT/FCT/01CMP/0006.HTM and .../0007.HTM ; OCCT Mesh user guide (BRepMesh), https://dev.opencascade.org/doc/overview/html/occt_user_guides__mesh.html ; "STL File Tolerance: faceting and chord height", PADT, https://www.padtinc.com/2012/11/13/stl-file-tolerance-a-short-explanation-of-faceting-and-chord-height/ ; Lee-Requicha (above); Parasolid v29.1 overview, Engineering.com, https://www.engineering.com/look-parasolid-version-29-1/ .
> **Method / Content.** The decisive industry fact: **the display / STL triangulation and the mass-property computation are SEPARATE pipelines in every production kernel.**
> - **OCCT**: `BRepMesh_IncrementalMesh` (linear deflection = chord height, angular deflection = max normal angle) produces the DISPLAY / export mesh. Mass properties come from `BRepGProp` / `GProp_GProps`, which integrate over the analytic geometry of the B-rep (Gauss quadrature over faces), NOT over the `BRepMesh` triangles. The two have different, independent tolerances.
> - **ACIS**: faceting (`FCT`) with surface tolerance (chord) and normal tolerance (angle) is for display / interop; mass properties (`api_body_mass_props`) integrate the analytic faces.
> - **Parasolid**: `PK_TOPOL_facet` / rendering tolerances drive display facets; `PK_TOPOL_eval_mass_props` computes mass by analytic / adaptive integration over the surfaces with its own accuracy control, decoupled from facet density.
> The shared doctrine: chord-height (linear deflection) and angular deflection govern the DISPLAY mesh only; mass / volume / inertia are computed by exact or adaptive integration over the exact surfaces. The reason is exactly the `sinc` bias of section 1.3: nobody wants a 1% volume error baked in by the facet count.
> **Limitations.** OCCT's `BRepGProp` still has a tolerance / quadrature order, so it is "adaptive-exact" not "symbolic-exact"; but it never inherits the display facet density. This is the model Keel's mass side already follows (dossier 60); the gap is only that Keel's GATE still drags in the mesh.
> **Kernel relevance.** This is the load-bearing external warrant for section 3: the major kernels structurally separate mass from the display mesh, so Keel comparing exact-mass against the display mesh is the unusual choice, and the fix (gate analytic-against-analytic, demote the mesh) brings Keel IN LINE with Parasolid / ACIS / OCCT rather than diverging from them. **Verified** (the pipeline separation is documented for OCCT and ACIS; the Parasolid API split is documented; the doctrine is universal).

### 4.2 The perf budget and where density would and would not hurt

> **Citation.** dossier 36 (performance / parallelism / memory); dossier 05 (tessellation / faceting); OCCT BRepMesh parallel meshing notes (above).
> **Method / Content (synthesis).** The perf ledger:
> - **Densifying the display mesh** (a uniform blanket increase) regresses the optimization leg directly: it multiplies the per-face triangle count that the winding / point-in-solid / boolean fragment loops walk, and those are the hot loops (dossier 36). A blanket `2x` density is a `~4x` triangle count on curved faces (product grid), unacceptable.
> - **Densifying ONLY for the gate's volume check** (a throwaway fine mesh built per gate evaluation, discarded after) costs build time but not hot-loop time. Still wasteful if done blanket.
> - **The analytic gate (section 3)** costs O(1) per face (plug scalars into a closed form), strictly cheaper than ANY mesh, and removes the gate's mesh dependence entirely for bucket-a faces. This is the perf-positive move: it makes the gate cheaper AND tighter.
> - **The equal-area correction (1.3)** is O(1) per ring on the existing mesh, negligible, and keeps the existing coarse mesh.
> - **The adaptive NURBS mesh (1.5)** is the only real cost, bounded to NURBS faces (bucket c), paid only there, and capped (DECLINE on cap).
> Net budget: the recommended path (analytic gate for quadrics, equal-area correction as fallback, adaptive mesh only for NURBS) is PERF-POSITIVE for the common case (quadrics get cheaper) and bounded for the rare case (NURBS). The display mesh stays coarse, so boxes stay at 0.9 ms.
> **Limitations.** The analytic gate adds a classification step (recognize the face type and extract scalars), which Keel already has (explicit surface type). The adaptive NURBS mesh needs the recursion cap tuned so worst-case NURBS faces decline rather than blow the budget.
> **Kernel relevance.** Confirms the recommendation does not regress the optimization leg: it moves the common curved case OFF the mesh for the gate, leaving the hot loops' mesh density untouched. **Verified** (the hot-loop / density coupling, dossier 36); the budget synthesis is **claimed**.

---

## Cross-cutting three-bucket catalog

> **(a) EXACT analytic volume/area, tessellation unnecessary for the gate.** Plane, cylinder (and azimuth wedge), cone, frustum, sphere (zone / cap / sector), torus (full and banded by Pappus). These are the entire false-decline-prone set. They get Side B (section 3.3); the gate is `analytic == analytic` to 1e-9, mesh-independent. **This bucket is the whole point of the dossier:** every currently-false-declining curved face is in it.
> **(b) BOUNDED-error adaptive mesh needed (no closed form, but low curvature / well-behaved).** Quadric faces that have been refit to NURBS and not re-recognized, low-degree NURBS surfaces of revolution, blends / fillets that are sphere / torus pieces but stored generically. Use the Filip-Magedson-Markot density (1.4) plus the equal-area correction (1.3) where the cross-section is circular; set chord tolerance to ~1/10 of the band so the `1/n^2` swept bias is well inside.
> **(c) TRUE chord-error-driven mesh with a perf cost.** High-degree general NURBS patches with no closed form and varying curvature. Use per-span adaptive subdivision (1.5, Zheng-Sederberg intervals) with a recursion cap; DECLINE on cap. Perf cost is real but bounded to these faces only.

---

## PATENT AWARENESS

- **Sagitta / chord-height faceting and the `n = pi/acos(1 - tol/r)` segment rule**: classical (sagitta is centuries old; the tessellation-tolerance use predates 1990). No live patent risk. **PREFER.**
- **Filip-Magedson-Markot derivative-bound tessellation (1986)**: foundational, expired-era publication, freely citable and implementable. **PREFER.**
- **Zheng-Sederberg tessellation-interval estimation (2000)**: published method (ACM TOG); the paper is reference, not a patent grant, but the era is post-2000 so a defensive read of any associated assignee patents is prudent before copying the exact interval bound. Design-around: use the plain Filip-Magedson-Markot second-derivative bound (1.4) and recursive midpoint subdivision (pre-2000, classical) instead of the tighter rational interval, accepting a few more triangles. **INVESTIGATE then PREFER the classical fallback.**
- **Post-2006 dynamic / GPU tessellation patents** (e.g. US 8,217,936 "Minimizing tessellation of surfaces", US 5,261,029 dynamic tessellation, US20070018988 curved-boundary rasterization): these are claim-reading hazards if Keel were to implement hardware-style dynamic tessellation. Keel's CPU per-face adaptive mesh driven by the classical derivative bound is a clean design-around (different mechanism, pre-art math). **AVOID copying the dynamic / GPU-pipeline claims; PREFER the classical derivative-bound CPU path.**
- **Mass-properties-from-tessellation patents** (e.g. US 10,783,708, US 7,623,996 on tessellation-error-estimated mass properties): these claim computing mass FROM a mesh with error estimation. Keel's recommended direction is the OPPOSITE (analytic mass, demote the mesh), so it sits outside these claims by construction. **The recommendation is itself the design-around.**

---

## ADOPT / AVOID / INVESTIGATE

**ADOPT.**
1. **Gate against the analytic reference for bucket-a faces (section 3).** Replace `mass == mesh` (tight, 2% band) with `Green-slab-mass == Pappus/frustum/zone closed form` (tight, 1e-9) for every natural-quadric face and band, and demote the display mesh to a loose topology witness (~5% band). This is the highest-leverage move, kills the cone false-decline, makes the gate tighter, and is perf-positive. Brings Keel in line with Parasolid / ACIS / OCCT, which never use the display mesh for mass (section 4.1).
2. **Equal-area `sinc` correction `k_correct(n) = (2*pi/n)/sin(2*pi/n)` (section 1.3)** as the fallback for any revolution face with a circular cross-section that lacks a closed form (bucket b), applied as a mass-mesh-only transform. Turns a coarse `-1.1%` mesh into `+0.01%` at zero extra triangles.
3. **Analytic cap substitution at apex / pole (section 2.2)**, the mesh dual of the dossier-60 anchor: substitute the closed-form tip / cap volume for the singular fan, mesh only the conditioned band.
4. **Filip-Magedson-Markot derivative-bound density (1.4) and the sagitta segment rule (1.1, 1.2)** as the principled, closed-form-for-quadrics way to set any remaining mesh density.

**AVOID.**
1. **Widening the band to swallow the bias.** It tolerates real sub-2% errors and still false-declines on bias-plus-real. The bias is deterministic; eliminate or correct it, do not budget for it.
2. **Uniform blanket mesh densification.** Regresses the optimization leg (section 4.2); `2x` density is `~4x` triangles on curved faces in the hot loops.
3. **Copying GPU / dynamic-tessellation patented pipelines** (PATENT section). Use the classical CPU derivative-bound path.
4. **Making the mesh the tight gate at all for faces that have a closed form.** It is the unusual choice industry-wide and is the root of the false-decline.

**INVESTIGATE.**
1. **The Zheng-Sederberg tighter rational interval (1.5)** for the NURBS bucket-c perf, after a defensive patent read; fall back to classical midpoint subdivision if the read is unfavorable.
2. **Whether any current bucket-b faces are really re-recognizable quadrics** (refit-to-NURBS quadrics) that could be promoted to bucket-a and skip the mesh entirely.
3. **The loose-witness band value** (5% suggested): tune against the soak corpus so it is loose enough to never false-decline a correct coarse mesh yet tight enough to catch a `>region` topology error.

**Recommended FIRST MILESTONE (narrowest honest slice with an exact oracle).**
The **cone tip / frustum face**, the exact case that currently false-declines. Add Side B for the cone family: when a face is a cone or frustum band with extractable `(alpha, v0, v1, azimuth)`, compute the closed-form volume `(1/3) pi r^2 h` (tip) or `(pi h/3)(R^2+Rr+r^2)` (frustum) and lateral area, and gate `Green-slab-mass == closed-form` to 1e-9, with the display mesh demoted to the loose witness for that face. Oracle: the closed forms themselves, cross-checked by a hand-computed cone at known dimensions. This is the smallest change that converts a real false-decline into a PASS, is mesh-independent, has a 1e-9 exact oracle, and is the template the other bucket-a faces (cylinder, sphere zone, torus) follow.

**Follow-up ladder.**
1. Cone / frustum Side B (the first milestone).
2. Cylinder wedge + sphere zone / cap + torus band Side B (same pattern, section 3.3).
3. Equal-area `sinc` correction for bucket-b revolution faces (1.3).
4. Analytic apex / pole cap substitution wired to the dossier-60 anchor (2.2).
5. Filip-Magedson-Markot density for any remaining meshed faces (1.4).
6. Adaptive per-span NURBS mesh with recursion cap + DECLINE-on-cap for bucket-c (1.5), the only real perf cost, paid only on NURBS.
7. Tune the loose-witness band against the soak corpus.

**DECLINE-forever set.**
- A high-degree general NURBS face whose adaptive mesh cannot reach the chord tolerance within the recursion cap: DECLINE, never emit an under-resolved volume that could pass a wrong body (DECLINE-never-WRONG).
- A face whose surface type cannot be classified and has no circular cross-section (no Side B, no `sinc` correction): the gate stays mesh-vs-analytic with the bounded adaptive mesh; if even that cannot certify to the band, DECLINE.
- A degenerate apex / pole seam (cut exactly through the singularity): declined upstream (dossier 59), so the cap handling never sees it.

---

## References

1. Sagitta (geometry), Wikipedia. https://en.wikipedia.org/wiki/Sagitta_(geometry)
2. Sagitta, Math Open Reference. https://mathopenref.com/sagitta.html
3. "Circle Tessellation with tolerance", GameDev.net forum (segment-count derivation). https://gamedev.net/forums/topic/555281-circle-tessellation-with-tolerance/
4. "A Simple Circle Discretization Algorithm With Applications", arXiv:1308.2581. https://arxiv.org/pdf/1308.2581
5. K. P. Han, "Pi and Archimedes' Polygon Method", Dartmouth. https://math.dartmouth.edu/archive/m56s13/public_html/Han_proj.pdf
6. R. Wicklin, "Polygons, pi, and linear approximations", SAS The DO Loop. https://blogs.sas.com/content/iml/2020/03/11/polygons-pi-linear-approx.html
7. Properties of Polygons (n-gon area identity), UTSA. https://mathresearch.utsa.edu/wiki/index.php?title=Properties_of_Polygons
8. D. Filip, R. Magedson, R. Markot, "Surface algorithms using bounds on derivatives", CAGD 3(4):295-311, 1986. https://dl.acm.org/doi/10.1016/0167-8396(86)90005-1
9. J. Zheng, T. Sederberg, "Estimating tessellation parameter intervals for rational curves and surfaces", ACM TOG 19(1), 2000. https://dl.acm.org/doi/10.1145/343002.343034
10. "Adaptive Tessellation for Trimmed NURBS Surface", Eurographics. https://diglib.eg.org/server/api/core/bitstreams/3faf6a3d-6dc3-454a-b654-b01cd65ab1f2/content
11. "Adaptive Tessellation of Subdivision Surfaces with Displacement Mapping", NVIDIA GPU Gems 2, ch.7. https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-7-adaptive-tessellation-subdivision-surfaces
12. Fan triangulation, Wikipedia. https://en.wikipedia.org/wiki/Fan_triangulation
13. "Improved Mesh Processing using Distorted Pole Spherical Coordinates", arXiv:2306.04625. https://arxiv.org/pdf/2306.04625
14. "Spherical Parameterization Balancing Angle and Area Distortions", arXiv:1810.09031. https://arxiv.org/pdf/1810.09031
15. Y. T. Lee, A. A. G. Requicha, "Algorithms for computing the volume and other integral properties of solids", Comm. ACM 25(9):635-650, 1982. https://dl.acm.org/doi/10.1145/358628.358643
16. B. C. Moulton, D. F. Hunsaker, "Analytic Solutions for Volume, Mass, Center of Gravity, and Inertia of Wing Segments and Rotors", Aerospace 11(6):492, 2024. https://www.mdpi.com/2226-4310/11/6/492
17. Pappus's centroid theorem, Wikipedia. https://en.wikipedia.org/wiki/Pappus%27s_centroid_theorem
18. Surface of Revolution (closed forms), MathWorld. https://mathworld.wolfram.com/SurfaceofRevolution.html
19. OCCT Mesh user guide (BRepMesh, linear / angular deflection). https://dev.opencascade.org/doc/overview/html/occt_user_guides__mesh.html
20. ACIS faceting: Surface Tolerance / Normal Tolerance. http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/FCT/FCT/01CMP/0006.HTM
21. "STL File Tolerance: faceting and chord height", PADT. https://www.padtinc.com/2012/11/13/stl-file-tolerance-a-short-explanation-of-faceting-and-chord-height/
22. "A look at Parasolid version 29.1", Engineering.com. https://www.engineering.com/look-parasolid-version-29-1/

## Related Keel dossiers (build on, not repeated)

- **05-tessellation-faceting.md**: the base faceting / chord-height machinery; this dossier specializes it to the gate's volume-error and the equal-area correction. (Tessellation-tolerance trap: do not mis-attribute a chord-vs-arc approximation to a geometry bug, MEMORY.)
- **60-curved-mass-integration.md**: the exact / certified-quadrature MASS side (Green-slab, apex / pole ANCHOR, generic boundary-arc arm). This dossier owns the MESH side and the gate design; the apex CAP substitution (2.2) is the mesh dual of dossier 60's apex anchor, and Side B (section 3) is the analytic partner of dossier 60's integrator.
- **35-verification-validated-numerics-oracles.md**: the oracle discipline; Side B's closed forms are exact oracles in that program.
- **36-performance-parallelism-memory.md**: the hot-loop / mesh-density coupling that the recommendation is designed not to regress (section 4.2).
- **06-interrogation-hlr.md**: the display-faceting consumer that keeps the coarse mesh this dossier preserves.
