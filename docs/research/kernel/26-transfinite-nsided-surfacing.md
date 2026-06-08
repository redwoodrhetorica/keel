# Transfinite and N-Sided Surface Construction

Research dossier for the Keel kernel on building a surface that fills a region bounded by N curves with prescribed cross-boundary continuity, and on surfacing networks of feature curves (Class-A style). This is the dedicated treatment of the Parasolid-audited capability "surface from boundary curves (n-sided)". Lofting/skinning (file 25 audit, separate lofting research) and vertex-blend Gregory patches in the fillet sense are covered elsewhere; here the focus is the general n-sided fill and curve-network surfacing problem.

The central engineering question for Keel: given a closed loop of N boundary curves (and optionally cross-boundary derivative fields and surrounding faces to be continuous with), produce a surface that (a) interpolates the boundary exactly, (b) meets neighbours with G1 (tangent plane) or G2 (curvature) continuity, and (c) can be represented in the kernel's surface store, which is tensor-product NURBS. The literature offers two broad answers: single multi-sided patches (transfinite or control-point) that are not natively tensor-product, and subdivision schemes that split the n-gon into n tensor-product (Coons/Bezier/NURBS) subpatches meeting at a central point. Keel will almost certainly ship the subdivision route as the storable result, using transfinite theory to define the cross-derivative data.

---

## 1. Coons patches and the transfinite viewpoint

### 1.1 Bilinearly blended Coons patch

**Citation.** Coons, S. A. (1967). *Surfaces for computer-aided design of space forms* (MIT Project MAC report MAC-TR-41). Massachusetts Institute of Technology. (Also developed in Coons' earlier 1964 MIT note.)

**Construction.** Given four boundary curves bounding a unit square in (u,v), with the two u-direction curves c0(u)=S(u,0), c1(u)=S(u,1) and the two v-direction curves d0(v)=S(0,v), d1(v)=S(1,v), the bilinearly blended Coons patch is the Boolean sum of two ruled (lofted) surfaces minus a bilinear correction:

  S(u,v) = [ (1-v) c0(u) + v c1(u) ]
         + [ (1-u) d0(v) + u d1(v) ]
         - [ (1-u)(1-v) S(0,0) + (1-u) v S(0,1) + u (1-v) S(1,0) + u v S(1,1) ].

The first bracket is the ruled surface in v interpolating the u-curves; the second is the ruled surface in u interpolating the v-curves; the third (the tensor product of the two blend pairs applied to the four corners) subtracts the corner data double-counted by the first two terms. This is the prototypical Boolean sum P[c] (+) P[d] - P[c]P[d]. The blend functions here are linear: alpha0=1-t, alpha1=t.

**Transfinite viewpoint.** Coons interpolation is "transfinite" because it interpolates an entire continuum (an infinite, "transfinite" set) of data: the whole boundary curves, not a finite point grid. This framing recurs throughout the n-sided literature: a patch is defined by interpolating curve-valued and derivative-valued data along the boundary.

**Limitations.** Only C0 between adjacent patches; the interior can be poor (flat or distorted) far from the boundary; gives no control of cross-boundary tangents, so it cannot build smooth composite surfaces by itself.

**Kernel relevance.** The Boolean-sum structure (sum of lofts minus correction) is the algebraic skeleton reused by Gordon surfaces, by every n-sided transfinite scheme, and directly by the practical NURBS hole-fill (each quad subpatch is a Coons patch). Keel needs a robust bicubic Coons primitive as a building block.

### 1.2 Bicubically blended Coons patch, twists, and incompatibility

**Citation.** Coons, S. A. (1967), as above; see also Farin, G. (2002). *Curves and Surfaces for CAGD: A Practical Guide* (5th ed.), Ch. 22 (Coons patches), Morgan Kaufmann.

**Construction.** Replace the linear blends with cubic Hermite blends H0, H1, H2, H3 (the Hermite basis with H0(0)=1, H1(1)=1, and derivative basis H2, H3). The patch then interpolates, on each boundary, both the curve and the cross-boundary derivative (the "ribbon" S and S_n along that edge). The corner data required is, per corner: position S, the two boundary tangents S_u and S_v, and the **twist** S_uv (the mixed second partial). The bicubic Coons patch is the Boolean sum of two Hermite-blended interpolants minus a bicubic Hermite tensor-product correction built from this corner data (positions, tangents, twists).

**The twist incompatibility (compatibility) problem.** The correction term assumes a single well-defined twist S_uv at each corner. But if the cross-boundary derivative fields along the two edges meeting at a corner are prescribed independently, the twist implied by differentiating one ribbon need not equal the twist implied by the other: S_uv from edge-u differs from S_vu from edge-v. For a single tensor-product surface these mixed partials must be equal (Schwarz/Clairaut), so independently chosen ribbons are generally **incompatible**, and forcing a single twist value distorts the patch. Choosing the twist also visibly changes the interior even when tangents are fixed.

**Kernel relevance.** Any G1 hole-fill that prescribes cross-boundary derivatives on the boundary curves will hit twist incompatibility at the corners where the curves meet. The two standard cures are (a) compatibility correction (adjust the data so a single twist works, e.g. Gregory's "compatibly corrected" interpolant, or quintic degree-raising to gain twist freedom) and (b) variable/rational twist (the Gregory patch). Keel must pick one; the practical literature (Piegl, Hahn) leans on quintic degree-raising of the boundary to gain the freedom to set compatible twists.

### 1.3 Gregory's compatibly corrected interpolant

**Citation.** Gregory, J. A. (1974). Smooth interpolation without twist constraints. In R. E. Barnhill & R. F. Riesenfeld (Eds.), *Computer Aided Geometric Design* (pp. 71-87). Academic Press.

**Construction.** Gregory replaced the constant corner twist by a position-dependent twist that varies across the patch, blending the two incompatible corner twists so that near each edge the twist that is "seen" matches that edge's ribbon. Concretely the twist term used in the correction is a rational interpolant of the form

  twist(u,v) = ( v * T_u + u * T_v ) / ( u + v )   near a corner,

where T_u and T_v are the two incompatible twist values from the two edges. This rational blend reproduces T_u as you approach along one edge and T_v along the other, removing the need for them to be equal. The cost is that S_uv is no longer constant; the surface is rational and has a removable singularity at the corner (handled by the u+v denominator construction).

**Limitations.** Rational, non-polynomial; the corner needs careful limit handling; it is a fix for the 4-sided case that becomes the seed of the Gregory patch.

**Kernel relevance.** This is the conceptual root of all "use a rational blend to dodge a compatibility constraint" tricks that pervade n-sided patches. If Keel uses Gregory-style ribbons it inherits rationality and the need to convert to (rational) NURBS by sampling/approximation.

---

## 2. Gordon surfaces (curve-network interpolation)

**Citation.** Gordon, W. J. (1969/1971). Spline-blended surface interpolation through curve networks. *Journal of Mathematics and Mechanics*, 18(10), 931-952. See also Gordon, W. J. (1971). Blending-function methods of bivariate and multivariate interpolation and approximation. *SIAM Journal on Numerical Analysis*, 8(1), 158-177.

**Construction.** Gordon generalizes the Coons patch from four boundary curves to a full **network (grid) of curves**: a family of m "u-curves" at parameters v1...vm and a family of n "v-curves" at parameters u1...un. The Gordon surface is again a Boolean sum:

  S(u,v) = sum_i  c_i(u) * A_i(v)     (lofts through the u-curve family)
         + sum_j  d_j(v) * B_j(u)     (lofts through the v-curve family)
         - sum_i sum_j  S(u_j, v_i) * B_j(u) * A_i(v)   (tensor-product correction),

where A_i and B_j are univariate interpolation basis functions (typically B-spline or cardinal/Lagrange blends) with the cardinal property A_i(v_k)=delta_ik. The first sum interpolates all u-curves, the second all v-curves, and the correction removes the doubly counted network intersection points S(u_j,v_i). Lofting (skinning) is the **degenerate Gordon case** with only one family of curves: drop the second and third terms.

**Compatibility condition.** The network must be **compatible**: every u-curve must intersect every v-curve, and the intersection of curve i (one family) with curve j (the other) must occur at the same parameter pair (u_j, v_i) for both. The interpolated grid points must be consistent. If the input curves do not actually meet at common parameters they must be reparameterized/refit first; in practice this is the hardest part of building a Gordon surface and is why few systems ship it.

**Gordon-Coons.** Using only the four outer curves plus their cross-derivatives, with Hermite blends, recovers the bicubic Coons patch as a special Gordon case, hence "Coons-Gordon" surfaces.

**Limitations.** Requires a topologically rectangular (grid) network; cannot directly handle irregular networks where curves meet at valence != 4 nodes; compatibility/reparametrization is delicate.

**Kernel relevance.** Gordon is the right tool when the curve network IS a grid (e.g. reverse-engineered turbomachinery blades, hull surfaces). It converts cleanly to NURBS (see Section 7: Lin and Hewitt). For Keel, Gordon is the curve-network surfacing primitive for the regular-grid case; irregular networks need the G2 curve-network methods of Section 6.

---

## 3. Gregory patches (twist-compatible 4-sided and irregular-mesh G1)

### 3.1 Rational Gregory patch / Gregory's square

**Citation.** Gregory, J. A. (1974), as above; and Chiyokura, H., & Kimura, F. (1983). Design of solids with free-form surfaces. *Computer Graphics (SIGGRAPH '83)*, 17(3), 289-298.

**Construction.** The Gregory patch ("Gregory's square") is a rational Bezier-form patch of bidegree (3,3) in which the four interior control points are each **split into two**, and recombined by a rational blend depending on the parameters. Where a tensor-product bicubic Bezier patch has interior control points b11, b12, b21, b22, the Gregory patch replaces each by a parameter-dependent point, e.g.

  b11(u,v) = ( u * b11^u + v * b11^v ) / ( u + v ),

and analogously for the other three corners with the appropriate (1-u),(1-v) factors. The two versions b^u and b^v of each interior point encode the twist implied by the two edges meeting at that corner; the rational blend lets each edge's cross-boundary derivative be set independently while the surface still interpolates both. This is exactly the four-corner generalization of Gregory's 1974 corner twist blend. The denominator vanishes only at the corner (a removable singularity).

**Limitations.** Rational; the boundary curves are ordinary cubic Beziers but the surface interior is rational (cannot be written as a single polynomial Bezier/NURBS without approximation); first-derivative-only control (G1).

**Kernel relevance.** The Gregory patch is the workhorse 4-sided G1 patch in many kernels because it removes twist incompatibility "for free". To store it in a NURBS core, Keel must either approximate-convert it to a (4,4) or (5,5) NURBS (Chiyokura's conversion, Section 7) or keep it as a special rational surface type.

### 3.2 Chiyokura-Kimura G1 interpolation over irregular meshes

**Citation.** Chiyokura, H., & Kimura, F. (1983). Design of solids with free-form surfaces. *Computer Graphics*, 17(3), 289-298.

**Construction.** Given an irregular control mesh (vertices of arbitrary valence, faces that are 3- or 4-sided), build one Gregory patch per face so that adjacent patches are G1. The key is the **cross-boundary derivative (CBD) construction**: for a shared edge with boundary curve b(t), Chiyokura-Kimura write the cross-boundary derivative as

  S_n(t) = k(t) * b'(t) + h(t) * T(t),

where b'(t) is the boundary tangent, T(t) is an auxiliary vector field along the edge, and k(t), h(t) are scalar (typically linear) blend functions. G1 between the two patches sharing the edge requires that the two patches' cross-boundary derivatives and the common boundary tangent be coplanar at every t: a relation of the form

  S_n^left(t) + S_n^right(t) = mu(t) * b'(t)

(the tangent-plane / "G1 connection" condition). Because in the Gregory patch the CBD along each edge is set independently (the split interior points), this condition can be satisfied edge by edge without the twists clashing. This is the canonical method for filling/interpolating an irregular quad mesh with G1 Gregory patches, and underlies many CAD free-form surface features.

**Limitations.** G1 only; requires consistent assignment of the auxiliary tangent fields T around each vertex (the vertex-consistency problem, Section 6); rational result.

**Kernel relevance.** This is the textbook recipe for "smooth surface over an irregular network of 4-sided faces", directly relevant to Keel's network surfacing. Sederberg/Chiyokura conversion to rational Bezier (Section 7) makes it kernel-storable.

---

## 4. N-sided patch families (the central topic)

The n-sided fill problem: interpolate a loop of N boundary curves (and cross-derivative ribbons) by a single logical surface that is G1 or G2 to each boundary. Approaches split into (A) single multi-sided patches over an n-gon domain and (B) subdivision into n tensor-product subpatches around a central point.

### 4.1 Charrot-Gregory convex-combination n-sided patch

**Citation.** Charrot, P., & Gregory, J. A. (1984). A pentagonal surface patch for computer aided geometric design. *Computer Aided Geometric Design*, 1(1), 87-94. Generalized in Gregory, J. A. (1989). *Smooth parametric surfaces and n-sided patches* (Brunel TR/07/89; also in W. Dahmen et al. (Eds.), *Computation of Curves and Surfaces*, Kluwer, 1990, pp. 457-498).

**Construction.** The Charrot-Gregory patch is a **convex combination of corner interpolants**. For an n-sided patch, define at each corner i a local biparametric coordinate pair (the two adjacent side parameters), and a corner interpolant C_i(u,v) that is itself a (Coons/Boolean-sum) surface matching the two boundary curves and their cross-derivatives meeting at corner i. The patch is

  S = sum_i  w_i(u,v) * C_i(u,v),   with  sum_i w_i = 1,

where the weight functions w_i are rational, built from products of squared distance functions to the non-adjacent sides:

  w_i = ( prod_{j != i, i-1}  d_j^2 ) / ( sum_k prod_{j != k, k-1} d_j^2 ),

with d_j the parametric distance to side j. Each w_i is 1 at corner i, vanishes (to second order, hence G1) on all sides not adjacent to corner i, and the squared distances guarantee that the corner interpolant's cross-derivative is the one that survives at each boundary. Result: S interpolates every boundary curve and matches every prescribed cross-boundary ribbon with G1 (tangent-plane) continuity. The 1984 paper does the pentagon; the 1989 report generalizes to arbitrary n and to higher continuity.

**Limitations.** Rational and not tensor-product; G1 in the basic form (G2 needs parabolic/curvature ribbons); the domain is a regular n-gon and parameterization (distance functions) must be defined carefully; singularities at corners need limits.

**Kernel relevance.** The archetypal "single n-sided patch by blended corner interpolants". Keel can use the construction to define the surface, then sample-and-fit to NURBS subpatches. Its weight structure reappears in Varady-Salvi ribbon methods.

### 4.2 Hahn: n rectangular patches at a common vertex

**Citation.** Hahn, J. M. (1989). Filling polygonal holes with rectangular patches. In W. Strasser & H.-P. Seidel (Eds.), *Theory and Practice of Geometric Modeling* (pp. 81-91). Springer.

**Construction.** The central-split / midpoint scheme. Subdivide the n-gon hole into n quadrilateral subregions by introducing a **central point** and n "spokes" from the boundary midpoints (or corners) to the centre, so n rectangular (tensor-product) patches meet at the common central vertex. Each rectangular patch is a Coons/Hermite patch interpolating its share of the boundary plus internally generated spoke curves and cross-derivatives. The construction chooses the spoke curves, the central point position, and the central derivative/twist data so that adjacent subpatches join with geometric continuity of arbitrary order, and so that each outer edge meets the surrounding surface with the required continuity. This is the dominant *practical* family because every subpatch is an ordinary tensor-product patch.

**Limitations.** The central point is a singular point of valence n where ensuring Gk and avoiding flat-spots / ripples is delicate; data at the centre must satisfy the vertex-enclosure parity conditions (Section 6); quality near the centre is the main risk.

**Kernel relevance.** This is essentially what Keel should implement: split into n quad NURBS patches around a central point, each stored as tensor-product NURBS. The whole question of "single patch impossible -> subdivide" resolves here.

### 4.3 Sabin and the non-rectangular B-spline-embeddable patch

**Citation.** Sabin, M. A. (1983). Non-rectangular surface patches suitable for inclusion in a B-spline surface. In P. ten Hagen (Ed.), *Proceedings of Eurographics '83* (pp. 57-69). North-Holland.

**Construction.** Sabin sought n-sided patches whose boundary and continuity behaviour let them drop into an existing tensor-product B-spline surface (i.e. a non-rectangular patch that "speaks B-spline" on its edges). The construction matches B-spline boundary conditions on each side so the n-sided fill is C1/C2 compatible with neighbouring rectangular B-spline patches.

**Limitations.** Restricted shapes; predates the rich transfinite machinery; mainly of historical and design-intent importance.

**Kernel relevance.** Frames the requirement Keel actually cares about: the n-sided result must be edge-compatible with the surrounding NURBS faces (matching degree and knot structure on shared edges), which constrains the subpatch degrees.

### 4.4 Loop-DeRose S-patches

**Citation.** Loop, C. T., & DeRose, T. D. (1989). A multisided generalization of Bezier surfaces. *ACM Transactions on Graphics*, 8(3), 204-234. Modern treatment: Hormann, K., & Sukumar, N. (Eds.) and the S-patch revisit by Schaefer et al. (Generalized Barycentric Coordinates context); Salvi, P. (2020), "G1 hole filling with S-patches made easy", arXiv:2002.11109.

**Construction.** An S-patch is a **Bezier simplex composed with a generalized-barycentric map**. Take a convex n-gon domain. Assign generalized barycentric coordinates (originally Wachspress) phi_1...phi_n at each domain point, so the point maps to a point on the (n-1)-simplex. Compose this with a Bezier simplex of **depth** d (a Bezier function over the simplex of degree d). Symbolically,

  S(p) = sum_{|a|=d}  P_a * B_a^d( phi_1(p), ..., phi_n(p) ),

where a ranges over multi-indices summing to depth d, B_a^d are the multivariate Bernstein basis on the simplex, and phi_i(p) are the barycentric coordinates of domain point p. Depth d controls flexibility; a side of the n-gon carries a Bezier curve of degree d, so matching a degree-k boundary needs depth d >= k. The number of control points P_a grows combinatorially with n and d (this is the main cost). G1 hole filling: choose depth and control points so each boundary curve and its cross-derivative match the neighbours; Salvi's 2020 note gives a streamlined recipe for the G1 case.

**Limitations.** Control-point count explodes with depth and sides (e.g. depth-5 hexagonal S-patch has hundreds of control points); requires a convex domain in the classic Wachspress form; not directly tensor-product.

**Kernel relevance.** S-patches are a clean theoretical unifier (they contain tensor-product and triangular Bezier as special cases) but the control-point blow-up makes them heavy for a kernel. Useful as a reference for what is representable; Keel would more likely subdivide than store an S-patch.

### 4.5 Krasauskas toric patches

**Citation.** Krasauskas, R. (2002). Toric surface patches. *Advances in Computational Mathematics*, 17(1-2), 89-113.

**Construction.** A toric patch generalizes rational Bezier surfaces using the geometry of a **lattice polytope**. Pick a convex lattice polygon A (its lattice points index the control points). For each lattice point a in A define a toric Bernstein basis function

  beta_a(x) = c_a * prod_k  l_k(x)^{h_k(a)},

where the l_k(x) are the linear functions defining the polygon's edges, h_k(a) is the lattice distance from a to edge k, and c_a are positive coefficients. The patch is the rational map

  S(x) = ( sum_a  w_a * P_a * beta_a(x) ) / ( sum_a  w_a * beta_a(x) ).

The polygon shape sets the number of sides; choosing A a square recovers tensor-product rational Bezier, a triangle recovers triangular rational Bezier. Toric patches thus give multi-sided rational surfaces with a Bezier-like control net over an arbitrary convex polygon.

**Limitations.** Convex domain; rational; the toric Bernstein functions have non-trivial exponents (not standard NURBS basis), so exact conversion to NURBS is not generally available; mainly a research representation.

**Kernel relevance.** Theoretically elegant and links to the generalized-barycentric and GB families, but the non-standard basis means Keel cannot store a toric patch as NURBS directly. Cited for completeness and as the algebraic backdrop to multi-sided Bezier.

### 4.6 Generalized barycentric coordinate patches (Wachspress, mean value, harmonic)

**Citation.** Wachspress, E. L. (1975). *A Rational Finite Element Basis*. Academic Press. Floater, M. S. (2003). Mean value coordinates. *Computer Aided Geometric Design*, 20(1), 19-27. Floater, M. S. (2015). Generalized barycentric coordinates and applications. *Acta Numerica*, 24, 161-214. Joshi, P., Meyer, M., DeRose, T., Green, B., & Sanocki, T. (2007). Harmonic coordinates for character articulation. *ACM TOG (SIGGRAPH)*, 26(3).

**Construction.** Generalized barycentric coordinates (GBC) phi_i over an n-gon are the parametrization machinery underneath S-patches, GB patches, and Kato/transfinite schemes. Each phi_i is 1 at vertex i, 0 at the others, varies linearly along edges, is non-negative inside, and sums to one. Wachspress coordinates are rational (ratios of products of triangle areas/cotangents) and valid for convex polygons; mean value coordinates (Floater) use the tangent of half-angles and extend to arbitrary simple polygons (lose positivity on concave ones); harmonic coordinates solve Laplace's equation in the cage and stay non-negative even on strongly concave domains but have no closed form. From the phi_i one derives side parameters s_i and distance parameters d_i used by ribbon blends.

**Limitations.** Wachspress needs convexity; mean value can go negative on concave domains; harmonic needs a numerical solve (no closed form) and is therefore expensive and hard to convert exactly.

**Kernel relevance.** Whatever single-patch scheme Keel touches, the domain parametrization is GBC. For convex n-gons Wachspress or mean value suffice; concave holes need mean value or harmonic. These coordinates are rational/transcendental, reinforcing that the storable output should be subpatch NURBS, not the raw GBC patch.

---

## 5. Ribbon-based transfinite methods (Varady-Salvi-Rockwood)

This is the most active modern line and the cleanest way to specify n-sided fills by their boundary data. A **ribbon** R_i for side i is the boundary curve plus its cross-derivative field (and for G2, the second cross-derivative), i.e. a thin strip of surface along side i parameterized by (s_i, d_i): s_i runs along the side, d_i measures distance into the patch (d_i = 0 on the side). The patch is a blended sum of ribbons with a correction term, exactly the Boolean-sum idea generalized to n sides.

### 5.1 Transfinite interpolation over irregular n-sided domains; interior control

**Citation.** Varady, T., Rockwood, A., & Salvi, P. (2011). Transfinite surface interpolation over irregular n-sided domains. *Computer-Aided Design*, 43(11), 1330-1340. Varady, T., Salvi, P., & Rockwood, A. (2012). Transfinite surface interpolation with interior control. *Graphical Models*, 74(6), 311-320.

**Construction.** Over a regular convex n-gon domain with GBC-derived local parameters (s_i, d_i), the patch is

  S = sum_i  R_i(s_i, d_i) * B_i(d_1,...,d_n)  -  correction,

where R_i is the i-th ribbon (a Hermite-type interpolant: R_i(s_i, 0) is the boundary curve, partial R_i / partial d_i at d_i=0 is the prescribed cross-derivative), and B_i are **distance-based rational blending functions** normalized to sum to one. The blends are built so that B_i = 1 on side i and vanishes (to the order matching the continuity) on the other sides; a typical form is

  B_i = ( prod_{j != i} d_j^2 ) / ( sum_k prod_{j != k} d_j^2 )

for G1 (square powers), with cubed powers for G2. The correction term subtracts the corner contributions counted by both adjacent ribbons (the n-sided analogue of the Coons bilinear correction). The 2012 paper adds **interior control**: an extra interior control point / scalar field to push the middle of the patch around without disturbing the boundary ribbons.

**Limitations.** Rational, transfinite (no finite control net in the base form); blend singularities at corners need limits; parameterization choice strongly affects quality.

**Kernel relevance.** This is the recommended way to *specify* a Keel n-sided fill: the user/kernel supplies boundary curves and cross-derivative ribbons (from the surrounding faces for a hole-fill), and the transfinite formula defines the exact surface, which is then sampled into NURBS subpatches.

### 5.2 Ribbon-based transfinite surfaces (the systematic catalogue)

**Citation.** Salvi, P., Varady, T., & Rockwood, A. (2014). Ribbon-based transfinite surfaces. *Computer Aided Geometric Design*, 31(9), 613-630.

**Construction.** A unifying framework and comparison of ribbon schemes, all of the form "blend ribbons, subtract corner correction", differing in which ribbons and which blends:
- **Side-based** (Coons-like): one ribbon per side, blends vanish on opposite sides (the 5.1 scheme).
- **Corner-based** (Charrot-Gregory-like): interpolants per corner blending two adjacent ribbons.
- **Composite-ribbon (CR)** patches: each composite ribbon interpolates three consecutive sides, giving stronger ribbon-to-constraint coupling and better quality, removing the need for a constrained parameterization.
- **Midpoint patch**: uses weight-deficient (non-normalized) blends plus a central midpoint correction; the authors report it as one of their best in surface quality. Detailed and made efficient in Salvi, P. (2020/2022), Computationally efficient transfinite patches with fullness control, arXiv:2002.11212, which adds a **fullness/deficiency parameter** to control how "full" (bulging) versus "flat" the interior is.

For each, G1 is enforced by requiring each blend to vanish to first order off its side and each ribbon to carry the correct first cross-derivative; G2 by raising the vanishing order (cubic distance powers) and supplying second-derivative (curvature) ribbons.

**Limitations.** As above (rational, transfinite); quality differences between schemes are subtle and shape-dependent, so a kernel may need to offer several.

**Kernel relevance.** Salvi 2014 is the menu Keel should choose from. The midpoint patch with fullness control is the strongest default for quality; the side-based scheme is the simplest to implement and convert.

### 5.3 Generalized Bezier (GB) patch and enhancements

**Citation.** Varady, T., Salvi, P., & Kariko, G. (2016). A multi-sided Bezier patch with a simple control structure. *Computer Graphics Forum (Eurographics)*, 35(2), 307-317. Varady, T., Salvi, P., Kovacs, I., & Kariko, G. (2017). Enhancement of a multi-sided Bezier surface representation. *Computer Aided Geometric Design*, 52-53, 249-265. Salvi, P., & Varady, T. (2018) follow-ups; B-spline and curved-domain extensions: Varady, Salvi, Vaitkus (2020-2021).

**Construction.** The GB patch fuses the transfinite ribbon idea with an actual **control net**, giving a genuine multi-sided Bezier-like surface. Boundaries and cross-derivatives are specified as ordinary Bezier curves/ribbons of arbitrary degree per side; the surface is

  S(u) = sum_i sum_{j,k}  C_{i,j,k} * mu_{i,j,k}(u),

where C_{i,j,k} are control points organized in rows parallel to each side (index j along the side, k inward), and mu_{i,j,k} are **weighted biparametric Bernstein functions** evaluated at local parameters (s_i, d_i) derived from GBC, multiplied by the same distance-based normalized blends as the transfinite patch. The central control point(s) handle the interior; degree elevation adds interior control without changing the boundary. The 2017 enhancement improves parameterization (interconnected/cubic maps instead of overlapping ones) and quality; later work extends to concave domains (harmonic coordinates), curved/multiply-connected boundaries, and B-spline boundaries (with knot insertion preserving the surface). A major practical property: when boundaries are polynomial, a GB patch **converts exactly to a set of rational Bezier patches**.

**Limitations.** Control-point bookkeeping is more complex than tensor-product; default interior control-point placement needs heuristics; still fundamentally a multi-sided (not tensor-product) object until converted.

**Kernel relevance.** GB is the best modern single-patch representation if Keel wanted a native multi-sided type, and its exact convertibility to rational Bezier patches is the bridge to a NURBS core. It is the most likely "interior surface store" if Keel ever goes beyond pure subdivision.

### 5.4 Kato generalized Coons patch

**Citation.** Kato, K. (1991). Generation of n-sided surface patches with holes. *Computer-Aided Design*, 23(10), 676-683. (Kato's blending function is reused in the BME constrained-parameterization work.)

**Construction.** Kato builds an n-sided generalized Coons patch by a Boolean sum of side interpolants with a specific rational **Kato blending function** that gives good corner behaviour and can accommodate interior holes. The blend is the n-sided analogue of the Coons Hermite blend and is shown to be essentially equivalent to a Gregory-type construction (the Generalized Coons surface is "virtually the same as the Gregory patch"). It interpolates a loop of 3D boundary curves and cross-derivatives with G1.

**Limitations.** G1; rational; one of several near-equivalent generalized-Coons formulations.

**Kernel relevance.** The Kato blend is a reusable component (the BME group uses it inside their constrained-parameterization GBC patches). Mainly cited as the bridge between classical generalized Coons and the modern ribbon framework.

---

## 6. Continuity machinery and the vertex-enclosure problem

### 6.1 Enforcing G1 and G2 across independent patches

For two independently built patches sharing boundary curve b(t), G1 (tangent-plane continuity) requires that the two cross-boundary derivatives and the boundary tangent be coplanar with matched scaling:

  alpha(t) * S_v^left(t) + beta(t) * S_v^right(t) + gamma(t) * b'(t) = 0,

for scalar functions alpha, beta, gamma. G2 (curvature continuity) additionally requires the second cross-derivatives to match up to terms along the boundary, i.e. equal normal curvature and matched second fundamental form across the seam. The Chiyokura-Kimura CBD construction (Section 3.2) is the constructive way to satisfy the G1 relation edge by edge. For G2, ribbons must carry second-derivative (curvature) data and the blends must vanish to second order off their side.

### 6.2 The vertex-enclosure problem (parity / twist constraint)

**Citation.** Peters, J. (1991). Smooth interpolation of a mesh of curves. *Constructive Approximation*, 7(1), 221-246. Sarraga, R. F. (1987). G1 interpolation of generally unrestricted cubic Bezier curves. *Computer Aided Geometric Design*, 4(1-2), 23-39. Hermann, T., Peters, J., et al. on G2 nodepoint compatibility; Peters, J. (1991), Parametrizing singularly to enclose data points.

**Construction / result.** When several patches meet at a common vertex, the corner twist/curvature data they each carry cannot be chosen freely: going around the vertex and composing the cross-boundary continuity conditions must return consistently to the start. Peters proved the **vertex-enclosure constraint**: for a mesh point where an **even** number of curves meet, the curvatures of the emanating mesh curves must satisfy an extra linear constraint for a single-polynomial-per-face G1 interpolant to exist; for an **odd** number of neighbours the constraint is automatically satisfiable. This parity phenomenon is exactly why central-split schemes prefer an odd configuration or add a free central point: an n-gon split into n quads at a central vertex creates a valence-n interior node, and for even n the twist data at the centre must obey the enclosure constraint or the fill cannot be globally consistent without degree elevation or extra freedom. Sarraga and others give explicit twist-setting recipes; "Ensuring compatibility of G2-continuous surface patches around a nodepoint" (Hermann et al., CAGD 1996) handles the G2 case.

**Kernel relevance.** This is the deep reason Keel's n-sided fill cannot just bolt independent patches together: at the central vertex (and at boundary corners shared with surrounding faces) the twist/curvature data must satisfy parity/compatibility. The robust engineering answers are (a) degree elevation (cubic -> quintic boundaries) to buy the needed twist freedom, as Piegl and Hahn do, and (b) Gregory/rational blends to dodge the single-twist requirement. Keel must implement the enclosure check and one of these fixes.

---

## 7. Conversion to the kernel surface type (NURBS)

### 7.1 Coons-Gordon and Coons B-spline surfaces as NURBS

**Citation.** Lin, F., & Hewitt, W. T. (1994). Expressing Coons-Gordon surfaces as NURBS. *Computer-Aided Design*, 26(2), 145-155. Related: Coons-type blended B-spline (CNSBS) surface and its conversion to NURBS, *CAD* (1997).

**Construction.** A Coons or Gordon surface built from B-spline boundary curves and B-spline blending functions can be written exactly as a single NURBS surface by expressing the Boolean sum in B-spline form: take a common (merged) knot vector and degree across the contributing lofts, and the sum-minus-correction collapses into one tensor-product B-spline/NURBS surface. The degree is the max of the contributing degrees; the knot vector is the union (merge) of the input knot vectors, so the result can be high-degree with many knots. Rational blends (e.g. from Gregory-type twist correction) make the result a genuine NURBS (rational), otherwise it is a polynomial B-spline.

**Kernel relevance.** Confirms that the regular cases (Coons, Gordon, Coons B-spline) convert *exactly* to one NURBS surface, at the cost of degree = max and knots = union. This is the clean conversion path for Keel's grid-network and 4-sided fills.

### 7.2 Practical n-sided G1 fill as trimmed/subpatch NURBS (Piegl, Hahn, triangular Coons B-spline)

**Citation.** Piegl, L. A., & Tiller, W. (1999). Filling n-sided regions with NURBS patches. *The Visual Computer*, 15(2), 77-89. Varady, T., Rockwood, A. (rational extension): Yang, Y.-J., et al. (2007). A rational extension of Piegl's method for filling n-sided holes. *Computer-Aided Design*, 39(11). HAL/INRIA: Filling n-sided regions with G1 triangular Coons B-spline patches (inria-00517938).

**Construction.** The dominant practical recipe (the one Keel should mirror):
1. Take the n boundary curves (and cross-boundary derivative fields from surrounding faces) in B-spline form; degree-raise boundaries to quintic if needed for twist compatibility.
2. Compute a **central point** and n inner ("spoke") curves from the boundary midpoints to the centre.
3. Compute inner cross-boundary derivatives satisfying tangent AND twist compatibility at the centre and at the boundary corners (this is where the vertex-enclosure/twist conditions are discharged).
4. Fill each of the n quadrilateral (or triangular) subregions with a **bicubically blended Coons B-spline patch** (or a triangular Coons B-spline patch in the rational extension).
5. The subpatches are constructed to be G1 across the internal spokes and G1 to the boundary.

The result is **n tensor-product NURBS subpatches**, typically degree (3,3) for bicubic or (5,5) when quintic degree-raising was used; commercial systems often emit (5,5) trimmed surfaces. The rational extensions (Yang et al.) handle the corner incompatibility with rational ribbons so fewer compromises are needed.

**Kernel relevance.** This is the concrete, shippable operation. It produces exactly the kind of output Keel's NURBS store wants: a small set of tensor-product NURBS patches (n of them), each storable directly, joined G1, with the hard continuity bought by quintic degree-raising and careful central-data selection. Trimming is only needed if a single bounding surface is wanted; the natural output is n untrimmed quad NURBS faces.

### 7.3 Converting Gregory / GB patches to rational Bezier/NURBS

**Citation.** Takamura, T., Ohta, M., Toriya, H., & Chiyokura, H. (1990). A method to convert a Gregory patch and a rational boundary Gregory patch to a rational Bezier patch. In *Computer Graphics International / Modeling in Computer Graphics* (Springer), pp. 543-562. Also: Approximate conversion of a rational boundary Gregory patch to a non-uniform B-spline surface, *The Visual Computer*.

**Construction.** A (3,3) Gregory patch with its split interior points converts to a (4,4) rational Bezier patch (degree raised by one in each direction to absorb the rational split-point blend); the rational boundary Gregory patch (NURBS-boundary variant) converts approximately to a NURBS surface. GB patches with polynomial boundaries convert exactly to a set of rational Bezier patches (Section 5.3).

**Kernel relevance.** If Keel ever stores a Gregory or GB patch, this is the (sometimes exact, sometimes approximate) bridge to the NURBS core. Exact for GB-to-rational-Bezier; approximate for rational-boundary Gregory-to-NURBS.

---

## 8. Curve-network and Class-A surfacing (G2 networks)

**Citation.** Salvi, P., Varady, T. (2014). G2 surface interpolation over general topology curve networks. *Computer Graphics Forum (Pacific Graphics)*, 33(7), 151-160. Hermann, T., Lukacs, G., & Wesselink, W. (1996). Curve networks compatible with G2 surfacing. *Computer Aided Geometric Design* (and Pottmann/Hermann follow-ups). General Class-A background: Alias/Autodesk Class-A surfacing theory; Farin (2002), Ch. on quality.

**Construction.** Automotive Class-A surfacing surfaces a network of styling feature curves with **curvature continuity (G2)** so that reflection lines (highlights) flow unbroken across patch seams. The pipeline: (1) the designer's feature curves form an irregular network with nodes of arbitrary valence; (2) the network must be made **G2-compatible** (Hermann et al.: the curves and their associated surface curvature data around each node must satisfy compatibility relations analogous to the vertex-enclosure constraint, but for second order); (3) each network face (often n-sided) is filled by a G2 multi-sided patch (ribbon/GB transfinite with curvature ribbons, or G2 subpatch fills), so adjacent patches share not just tangent planes but curvature; (4) the result is a gap-free, curvature-continuous patchwork. Salvi-Varady 2014 give a transfinite G2 scheme over general-topology networks; the ribbons carry second-derivative data and the blends vanish to second order off their sides.

**Limitations.** G2 compatibility around high-valence nodes is restrictive (the designer's curves may need to be re-faired or re-fit to be G2-compatible); full G2 across an irregular network is genuinely hard and is the frontier of the field.

**Kernel relevance.** This is the "Class-A" ambition. For Keel a realistic first target is G1 network surfacing (Chiyokura-Kimura / GB-G1); G2 network surfacing (Salvi-Varady 2014, Hermann compatibility) is a later, harder milestone. The curvature-ribbon transfinite machinery (Section 5) is the route.

---

## 9. Quality: fairness, fullness, ripple avoidance

**Citation.** Salvi, P. (2020/2022). Computationally efficient transfinite patches with fullness control, arXiv:2002.11212. Surface energy minimization for bicubic Coons: Extremal solutions for surface energy minimization (bicubically blended Coons patches), *Symmetry* 15(6):1237 (2023). Moreton, H., & Sequin, C. (1992). Functional optimization for fair surface design. *SIGGRAPH '92*. Greiner, G., fairness energies.

**Construction.** Two levers control quality. (a) **Twist/interior optimization**: after building an interpolating fill, reposition interior control points (or set the free twist vectors) to minimize a fairness energy, typically the thin-plate / membrane bending energy E = integral (S_uu^2 + 2 S_uv^2 + S_vv^2) du dv, by solving a (linear, for the quadratic energy) system while holding boundary control points fixed. Twist vectors can be uniquely fixed by minimizing an approximate bending energy. (b) **Fullness control** (Salvi): a parameter on the blend functions sets how strongly the ribbons pull the interior outward, trading bulge against flatness, which is the main knob for avoiding ripples and flat spots near the central vertex of a split fill. Ripples on n-sided patches usually originate at the singular central point or from incompatible/un-faired boundary curves; the literature's consistent advice is to **fair the input boundary curves first** (a flawed boundary curve produces a flawed surface) and then minimize interior energy.

**Kernel relevance.** Keel's n-sided fill should expose (i) a fairness pass on the interior data (linear bending-energy minimization with boundary fixed) and (ii) a fullness parameter, and should fair/validate the boundary curves before filling. Curvature distribution should be checked with reflection-line / isophote interrogation (file 06 interrogation).

---

## 10. Surfacing construction for Keel (synthesis)

**Recommended representation strategy: subdivide, store as NURBS subpatches.** Keel's core surface type is tensor-product NURBS. None of the single multi-sided patches (Charrot-Gregory, S-patch, toric, GB, transfinite ribbon) is tensor-product NURBS; storing them natively would require a new rational multi-sided surface type plus all the conversion code. The pragmatic, Parasolid-class answer is the **central-split / midpoint scheme** (Hahn, Piegl, triangular Coons B-spline): split the n-gon into n quadrilateral (or triangular) subregions around a central point and store each as a tensor-product NURBS face. This is exactly what shipping kernels emit (often (5,5) trimmed/untrimmed patches).

**Use transfinite theory to define the data, subpatches to store it.** The transfinite ribbon framework (Varady-Salvi-Rockwood 2011-2014; midpoint patch with fullness control) is the cleanest way to *specify* the fill from boundary curves plus cross-derivative ribbons taken from the surrounding faces. Evaluate/sample that transfinite definition (or compute spoke curves and compatible inner derivatives directly per Piegl) and fit n NURBS subpatches to it.

**The G1/G2 hole-fill operation (the kernel primitive):**
1. Collect the n boundary curves of the hole and, from each adjacent face, the cross-boundary derivative field (G1) and second derivative (G2) along that edge: the ribbons.
2. Degree-raise boundary curves (cubic -> quintic) to gain twist freedom; merge knot vectors per shared edge for NURBS edge-compatibility (Sabin's requirement).
3. Check/discharge the vertex-enclosure (parity/twist) constraint at every corner and at the prospective central vertex (Peters/Sarraga); use Gregory-style rational ribbons or degree elevation where the single-twist condition fails.
4. Compute the central point, n spoke curves, and inner cross-boundary derivatives satisfying tangent and twist (and for G2, curvature) compatibility.
5. Fill each subregion with a bicubic (G1) or higher-degree (G2) Coons B-spline patch; join subpatches G1/G2 across spokes and to the boundary.
6. Fairness pass: minimize bending energy on interior control points with the boundary fixed; expose a fullness parameter; validate with reflection-line interrogation.

**Single patch vs subpatch decision.** A single tensor-product NURBS face suffices only for the 4-sided Coons/Gordon case and the regular grid Gordon case (Lin-Hewitt exact conversion). For n != 4, or where curves meet at irregular valence, subdivision is forced: there is no single tensor-product NURBS that is G1/G2 to N != 4 independent boundaries without splitting (a direct consequence of the four-corner twist structure and the vertex-enclosure parity result). So: 4 sides and grids -> one NURBS; everything else -> n subpatches.

**Curve-network surfacing.** Regular (grid) networks -> Gordon surface -> single NURBS. Irregular networks at G1 -> Chiyokura-Kimura / GB-G1 per face, then convert/subdivide to NURBS. Irregular networks at G2 (true Class-A) -> Salvi-Varady G2 transfinite with curvature ribbons plus Hermann G2-compatibility on the network; this is a later milestone and the hardest part of the field.

**What to implement, in order.**
1. Bicubic Coons B-spline patch primitive (the subpatch building block) and exact Coons/Gordon-to-NURBS (Lin-Hewitt).
2. Gordon surface for regular curve networks and lofting (degenerate Gordon).
3. Central-split n-sided G1 fill (Piegl/Hahn style) emitting n NURBS subpatches, with vertex-enclosure handling via quintic degree-raising and the fairness/fullness pass.
4. Chiyokura-Kimura G1 over irregular quad networks (with optional Gregory-to-rational-Bezier conversion if a Gregory store is added).
5. (Later) G2 n-sided and G2 curve-network surfacing (curvature ribbons, Salvi-Varady 2014, Hermann compatibility) for Class-A.

---

## References

1. Coons, S. A. (1967). *Surfaces for computer-aided design of space forms* (MIT Project MAC report MAC-TR-41). MIT.
2. Farin, G. (2002). *Curves and Surfaces for CAGD: A Practical Guide* (5th ed.). Morgan Kaufmann (Coons/Gordon chapters).
3. Gregory, J. A. (1974). Smooth interpolation without twist constraints. In Barnhill & Riesenfeld (Eds.), *Computer Aided Geometric Design* (pp. 71-87). Academic Press.
4. Gordon, W. J. (1969). Spline-blended surface interpolation through curve networks. *J. Mathematics and Mechanics*, 18(10), 931-952.
5. Gordon, W. J. (1971). Blending-function methods of bivariate and multivariate interpolation and approximation. *SIAM J. Numerical Analysis*, 8(1), 158-177.
6. Chiyokura, H., & Kimura, F. (1983). Design of solids with free-form surfaces. *Computer Graphics (SIGGRAPH '83)*, 17(3), 289-298.
7. Charrot, P., & Gregory, J. A. (1984). A pentagonal surface patch for computer aided geometric design. *Computer Aided Geometric Design*, 1(1), 87-94.
8. Gregory, J. A. (1989). *Smooth parametric surfaces and n-sided patches* (Brunel TR/07/89); in Dahmen et al. (Eds.), *Computation of Curves and Surfaces*, Kluwer, 1990.
9. Sabin, M. A. (1983). Non-rectangular surface patches suitable for inclusion in a B-spline surface. *Proc. Eurographics '83*, 57-69. North-Holland.
10. Hahn, J. M. (1989). Filling polygonal holes with rectangular patches. In Strasser & Seidel (Eds.), *Theory and Practice of Geometric Modeling* (pp. 81-91). Springer.
11. Loop, C. T., & DeRose, T. D. (1989). A multisided generalization of Bezier surfaces. *ACM Transactions on Graphics*, 8(3), 204-234.
12. Kato, K. (1991). Generation of n-sided surface patches with holes. *Computer-Aided Design*, 23(10), 676-683.
13. Peters, J. (1991). Smooth interpolation of a mesh of curves. *Constructive Approximation*, 7(1), 221-246 (vertex-enclosure constraint).
14. Sarraga, R. F. (1987). G1 interpolation of generally unrestricted cubic Bezier curves. *Computer Aided Geometric Design*, 4(1-2), 23-39.
15. Hermann, T., Lukacs, G., & Wesselink, W. (1996). Ensuring compatibility of G2-continuous surface patches around a nodepoint / Curve networks compatible with G2 surfacing. *Computer Aided Geometric Design*.
16. Takamura, T., Ohta, M., Toriya, H., & Chiyokura, H. (1990). A method to convert a Gregory patch and a rational boundary Gregory patch to a rational Bezier patch. *Modeling in Computer Graphics*, Springer, 543-562.
17. Lin, F., & Hewitt, W. T. (1994). Expressing Coons-Gordon surfaces as NURBS. *Computer-Aided Design*, 26(2), 145-155.
18. Piegl, L. A., & Tiller, W. (1999). Filling n-sided regions with NURBS patches. *The Visual Computer*, 15(2), 77-89.
19. Yang, Y.-J., et al. (2007). A rational extension of Piegl's method for filling n-sided holes. *Computer-Aided Design*, 39(11).
20. (INRIA) Filling n-sided regions with G1 triangular Coons B-spline patches, hal inria-00517938.
21. Krasauskas, R. (2002). Toric surface patches. *Advances in Computational Mathematics*, 17(1-2), 89-113.
22. Floater, M. S. (2003). Mean value coordinates. *Computer Aided Geometric Design*, 20(1), 19-27; Floater (2015), Generalized barycentric coordinates and applications, *Acta Numerica*, 24, 161-214.
23. Wachspress, E. L. (1975). *A Rational Finite Element Basis*. Academic Press.
24. Joshi, P., Meyer, M., DeRose, T., Green, B., & Sanocki, T. (2007). Harmonic coordinates for character articulation. *ACM TOG (SIGGRAPH)*, 26(3).
25. Varady, T., Rockwood, A., & Salvi, P. (2011). Transfinite surface interpolation over irregular n-sided domains. *Computer-Aided Design*, 43(11), 1330-1340.
26. Varady, T., Salvi, P., & Rockwood, A. (2012). Transfinite surface interpolation with interior control. *Graphical Models*, 74(6), 311-320.
27. Salvi, P., Varady, T., & Rockwood, A. (2014). Ribbon-based transfinite surfaces. *Computer Aided Geometric Design*, 31(9), 613-630.
28. Salvi, P., & Varady, T. (2014). G2 surface interpolation over general topology curve networks. *Computer Graphics Forum (Pacific Graphics)*, 33(7), 151-160.
29. Varady, T., Salvi, P., & Kariko, G. (2016). A multi-sided Bezier patch with a simple control structure. *Computer Graphics Forum (Eurographics)*, 35(2), 307-317.
30. Varady, T., Salvi, P., Kovacs, I., & Kariko, G. (2017). Enhancement of a multi-sided Bezier surface representation. *Computer Aided Geometric Design*, 52-53, 249-265.
31. Salvi, P. (2020). G1 hole filling with S-patches made easy. arXiv:2002.11109; Salvi, P. (2022). Computationally efficient transfinite patches with fullness control. arXiv:2002.11212.
32. Vaitkus, M., Varady, T., Salvi, P., et al. (2020-2021). Multi-sided B-spline surfaces over curved, multi-connected domains.
33. Moreton, H., & Sequin, C. (1992). Functional optimization for fair surface design. *SIGGRAPH '92*; Extremal solutions for surface energy minimization (bicubic Coons), *Symmetry* 15(6):1237 (2023).
34. Hormann, K., & Sukumar, N. (2017). *Generalized Barycentric Coordinates in Computer Graphics and Computational Mechanics*. CRC Press (S-patch GBC context).
