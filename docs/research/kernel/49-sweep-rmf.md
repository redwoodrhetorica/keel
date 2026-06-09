# Sweep Along an Arbitrary Path with Rotation-Minimizing Frames

Research dossier 49 for the Keel B-rep kernel (tolerant-f64, non-manifold PES, Rust, Parasolid-class). Roadmap item 63: the full general sweep. This is the implementation-grade companion to the survey-level treatment in `docs/research/nurbs/05-offsets-blends-sweeps.md`, which already establishes the double-reflection RMF (Wang, Juttler, Zheng, Liu 2008) and the swept/canal-surface families at the literature-survey level. This document does not repeat that survey: it specifies the operation Keel must build.

## Scope and the problem statement

Keel's current `sweep_along_path` is a TRANSLATIONAL sweep: the profile is rigidly translated along the path and stays parallel to itself, so the result is a generalized cylinder S(u,v) = A(u) + C(v) where A is the spine and C the profile. This is correct only for a straight or near-straight path and gives geometrically wrong, self-shadowing surfaces on a curved path. The general sweep ORIENTS the profile: it places the profile in a moving orthonormal frame F(u) = {t(u), r(u), s(u)} carried along the spine, so

    S(u, v) = A(u) + F(u) [0, C_x(v), C_y(v)]^T
            = A(u) + C_x(v) r(u) + C_y(v) s(u)

where t(u) is the unit spine tangent and {r, s} span the plane normal to t. The entire design problem is: which frame field F(u), how is it represented and baked to NURBS, when does the swept surface self-intersect, and how are scale/twist laws and polyline joints handled. The four numbered sections answer exactly those.

Two adjacent dossiers bound this one. `docs/research/kernel/10-minkowski-medial-midsurface.md` gives the swept-volume boundary as an envelope and the grazing/self-intersection condition via the medial axis. `docs/research/kernel/41-blend-overflow-radius-feature-failures.md` gives the r_max = local-curvature-radius failure pattern for blends; the sweep self-intersection rule in Section 3 is the same inequality applied to the profile radius. The procedural-baked-to-NURBS representation in Section 2 ties to the skinning/lofting machinery (the sibling skinning dossier, planned as file 48; until it lands, treat the loft step here as self-contained).

Everything below is a DESIGN RECOMMENDATION to validate against Keel's existing tests, in particular the translational-sweep regression suite and the L-path / helix cases stated at the end.

---

## 1. Rotation-minimizing frame transport: double reflection vs Frenet

### 1.1 What "rotation minimizing" means

An adapted orthonormal frame on a curve A(u) has its first axis equal to the unit tangent t(u) = A'(u)/|A'(u)|; the other two axes {r, s} are any orthonormal basis of the normal plane. Among all adapted frames, the rotation-minimizing frame (RMF) is the one whose normal-plane axes do not spin about t: the frame's angular velocity omega has zero component along t,

    omega . t = 0,

i.e. r' and s' have no rotation about the tangent, only the unavoidable tilt forced by the curve bending. Equivalently r(u) is RELATIVELY PARALLEL: r' is parallel to t (it changes only to stay perpendicular to the turning tangent). This is the Bishop / parallel-transport frame. It minimizes total angular twist integral of (omega . t) du, which for the RMF is zero by construction.

### 1.2 Why not the Frenet frame

The Frenet frame uses {t, N, B} with the principal normal N = (t')/|t'| and binormal B = t x N. Its transport equations are

    t' = kappa * v * N
    N' = v * (-kappa * t + tau * B)
    B' = -tau * v * N

with v = |A'|, curvature kappa, torsion tau. The N, B rows contain the torsion term tau: the Frenet frame rotates about the tangent at angular speed tau even when nothing requires it. That extra twist is the defect. Three concrete failure modes make the Frenet frame unusable as the sweep frame:

1. Inflection points. Where kappa -> 0, the principal normal N = t'/|t'| is the ratio of two vanishing quantities; N flips by 180 degrees through the inflection. A profile carried on N visibly snaps over.
2. Straight segments. On any straight span kappa = 0 identically, so N is undefined (0/0). A polyline path or a path with a straight portion has no Frenet frame on those spans at all. This is fatal for the L-path and for any architectural / piping spine.
3. Spurious twist. Even on a smooth, everywhere-curved path, the accumulated torsion integral tau v du rotates the profile about the spine. A circular tube looks fine, but any non-symmetric profile (an I-beam, a keyway) is delivered rotated by the wrong angle.

The RMF has none of these: omega . t = 0 means no torsion term, so it is smooth through inflections, well defined on straight spans (it simply parallel-transports the previous {r, s} unchanged when t is constant), and introduces zero unwanted twist. Any deliberate twist is then added on top as an explicit law (Section 4), not inherited as an artifact.

### 1.3 The double-reflection method (Wang, Juttler, Zheng, Liu 2008)

The RMF has no closed form for a general NURBS spine, so it is computed by discrete transport: sample the spine at u_0 < u_1 < ... < u_n giving points x_i = A(u_i) and unit tangents t_i = A'(u_i)/|A'(u_i)|, then carry the frame {x_i, t_i, r_i, s_i} forward one step at a time. The double-reflection method advances {r_i} to {r_{i+1}} using two reflections, which is exact for the RMF of a circular arc and gives O(h^4) global error (O(h^5) per step), versus O(h^2) for the older projection (Klok) and rotation (Bloomenthal) methods at essentially the same per-step cost.

Reflection of a vector X in the plane through the origin with unit-or-general normal vector w:

    Reflect_w(X) = X - 2 * (w . X) / (w . w) * w.

The step from frame i to i+1 (carrying r_i, with s_i = t_i x r_i):

1. First reflection in the bisecting plane of the two sample points. Let
        v1 = x_{i+1} - x_i,   c1 = v1 . v1.
   Reflect both the incoming normal axis and the incoming tangent:
        r_i^L = r_i - (2 / c1) * (v1 . r_i) * v1
        t_i^L = t_i - (2 / c1) * (v1 . t_i) * v1.
   After this reflection t_i^L is the mirror image of t_i; it is NOT yet aligned with t_{i+1}.

2. Second reflection to align the tangent with the true outgoing tangent. Let
        v2 = t_{i+1} - t_i^L,   c2 = v2 . v2.
   Reflect the once-reflected normal axis:
        r_{i+1} = r_i^L - (2 / c2) * (v2 . r_i^L) * v2.
   Then complete the frame:
        s_{i+1} = t_{i+1} x r_{i+1}.

The composition of two reflections is a rotation, so {t_{i+1}, r_{i+1}, s_{i+1}} is orthonormal and right-handed by construction (no Gram-Schmidt re-normalization drift). The first reflection maps t_i onto the chord direction's mirror; the second maps that onto t_{i+1}; the net rotation is the minimal rotation taking t_i to t_{i+1}, which is precisely relatively-parallel transport. This is why the method nails the circular-arc RMF exactly and why it is the recommended transport for Keel.

Guard cases: if c1 or c2 underflows (coincident samples, or t_{i+1} == t_i^L meaning the tangent did not change, i.e. a straight span), skip that reflection and carry the axis through unchanged. A zero second reflection is the correct behavior on straight segments: parallel transport with no rotation.

### 1.4 Sources for Section 1

Citation. Wang, W., Juttler, B., Zheng, D., & Liu, Y. (2008). Computation of rotation minimizing frames. ACM Transactions on Graphics, 27(1), Article 2. https://doi.org/10.1145/1330511.1330513
Content. Defines the RMF by the zero-tangential-angular-velocity property omega . t = 0; introduces the double-reflection update via two reflections Reflect_w(X) = X - 2 (w.X)/(w.w) w with v1 = x_{i+1}-x_i and v2 = t_{i+1}-t_i^L as the two reflection normals; proves O(h^4) global / O(h^5) local accuracy, an order better than the O(h^2) projection and rotation methods at the same per-frame cost; notes the method is exact on circular arcs.
Kernel relevance. This is the transport algorithm Keel implements verbatim for general-sweep framing. The exact-on-arcs property lets Keel validate the helix case (constant-curvature) to machine precision.

Citation. Wang, W., & Zheng, J. (1997 / reprinted). Computation of rotation minimizing frame in computer graphics. (survey/reprint of the double-reflection result.) https://www.semanticscholar.org/paper/779374741a387cff101e70c0ad13282b7adc5ded
Content. Restates the double-reflection update and benchmarks it against Frenet, projection, and rotation transport; reiterates that Frenet twist is the torsion term tau.
Kernel relevance. Confirmatory secondary source for the accuracy ordering used to justify choosing double reflection over Klok in Keel.

Citation. Klok, F. (1986). Two moving coordinate frames for sweeping along a 3D trajectory. Computer Aided Geometric Design, 3(3), 217-229. https://doi.org/10.1016/0167-8396(86)90039-7
Content. Originates the requirement that a sweep frame must be independent of parametrization and of the trajectory's spatial orientation; gives the projection (RMF) method, where the previous normal axis is projected onto the new normal plane and renormalized, with O(h^2) error.
Kernel relevance. Defines the correctness criteria (parametrization- and orientation-independence) that Keel's sweep must satisfy. The projection method is Keel's fallback if double reflection ever needs a cheaper inner loop; the O(h^2) vs O(h^4) gap justifies preferring double reflection.

Citation. Bloomenthal, J. (1990). Calculation of reference frames along a space curve. In Graphics Gems (pp. 567-571). Academic Press. https://webhome.cs.uvic.ca/~blob/courses/305/notes/pdf/ref-frames.pdf
Content. The rotation method: carry the previous frame forward by rotating about the axis t_i x t_{i+1} through the angle between successive tangents; gives Frenet definitions T = V/|V|, N = (V x Q x V)/|...|, B = T x N and documents the Frenet normal flip at inflections and its undefined state on straight segments.
Kernel relevance. The rotation method is the conceptual sibling of double reflection (both produce parallel transport); Keel uses Bloomenthal's explicit catalog of Frenet failure modes as the test checklist for the L-path (straight spans) and any inflecting spine.

Citation. Wang, W., & Joe, B. (1997). Robust computation of the rotation minimizing frame for sweep surface modeling. Computer-Aided Design, 29(5), 379-391. https://doi.org/10.1016/S0010-4485(96)00077-2
Content. Approximates the spine by a G1 arc spline so the RMF can be computed essentially exactly (arc RMF is closed form), then builds the sweep S(u,v) = A(u) + F(u) C(v); shows the result is a NURBS surface when C is NURBS, and handles smooth closed spines (frame closure).
Kernel relevance. This is the blueprint for Keel's "bake to NURBS" path of Section 2 and for the closed-loop spine case; the arc-spline trick is the alternative to double reflection when an exact frame is wanted on a tolerant arc approximation of the spine.

---

## 2. The swept-surface representation: procedural vs baked NURBS

### 2.1 Two representations

Procedural (lazy). Store the operation: spine A, profile C, frame field F (the RMF transport rule), plus optional scale and twist laws. Evaluate S(u,v) on demand. Exact, compact, no approximation error, but every downstream operation (intersection, tessellation, boolean) must understand the procedural surface. For a tolerant non-manifold kernel that already does NURBS intersection everywhere, a procedural surface is a second code path to maintain in the SSI/SSI-trim machinery.

Baked NURBS (eager). Sample frames {x_i, t_i, r_i, s_i} along the spine, place a copy of the profile C in each frame to get a row of section curves C_i(v) = x_i + scale_i (cos th_i r_i + sin th_i s_i)-rotated profile, then SKIN/LOFT those sections into a single NURBS surface to a stated tolerance. The result is an ordinary trimmed NURBS face that every existing Keel operation already handles. This is the Wang-Joe construction and is the recommended default for Keel: it keeps the sweep inside the one geometry type the kernel is built around, at the cost of a controlled approximation.

Recommendation: bake to NURBS by default; keep the procedural form only as an internal evaluator used during sampling and for high-accuracy point/normal queries while baking. This matches dossier 05's general principle (store the operation, approximate to NURBS at a tolerance) and reuses the skinning code rather than adding a procedural surface kind.

### 2.2 Baking pipeline

1. Choose spine samples u_0..u_n adaptively (Section 2.3).
2. Transport the RMF by double reflection to get F_i at each u_i.
3. Apply scale law sigma(u_i) and twist law phi(u_i) (Section 4) to the placed profile, producing section curve C_i.
4. Make the sections compatible: common degree, merged knot vector in v (the knot-merge step; this is the blowup point flagged in dossier 05 and in skinning).
5. Skin the sections: interpolate or approximate a NURBS surface S(u,v) through/near the C_i with a v-knot vector from step 4 and a u-knot vector from the sample parameters, controlling the u-direction (along-spine) error to tolerance.
6. Add end caps and trim (Sections 3 and 4).

The v-direction is exact (it is the profile, a NURBS curve). All approximation is in the u-direction: how faithfully the lofted surface follows the true swept frame field between samples.

### 2.3 Sampling-density rule for a tolerance bound

The along-spine error of the baked surface is controlled by how densely the spine is sampled before lofting. The dominant error sources are (a) the chord/loft deviation between true frame placement and the interpolated surface, and (b) the frame-transport error, which double reflection makes negligible at O(h^4). So (a) dominates, and it behaves like a curve-approximation error governed by the spine curvature and the profile extent.

A working bound: place samples so that on each span the deviation between the true swept section and the lofted surface is below the surface tolerance eps_s. For a profile of maximum radius R_p (max distance from the profile's frame origin to the profile) swept on a spine with maximum curvature kappa_max, the worst-case point traces an arc of radius up to (1/kappa_max + R_p). Bounding the chord-height (sagitta) of that arc by eps_s gives a maximum step in arc length

    h <= 2 * sqrt( 2 * eps_s * (1/kappa_max + R_p) ) approx 2 * sqrt(2 * eps_s / kappa_eff),
    kappa_eff = 1 / (1/kappa_max + R_p) = kappa_max / (1 + R_p * kappa_max).

Equivalently, sample more densely where the spine bends hard or the profile is large. Refine adaptively: bisect any span whose measured midpoint deviation (true procedural S at the span midpoint vs the lofted surface at the same parameter) exceeds eps_s, until all spans pass. This is the same adaptive-refinement loop dossier 05 specifies for offsets; here the error metric is the procedural-vs-lofted Hausdorff sample. Put twist into the bound too: a twist law of total angle Phi adds an effective along-spine angular rate that must also be resolved, so cap h additionally by h_phi <= 2 sqrt(2 eps_s / R_p) / |phi'| where the profile is non-circular.

### 2.4 Sources for Section 2

Citation. Wang, W., & Joe, B. (1997). Robust computation of the rotation minimizing frame for sweep surface modeling. Computer-Aided Design, 29(5), 379-391. https://doi.org/10.1016/S0010-4485(96)00077-2
Content. S(u,v) = A(u) + F(u) C(v) is a NURBS surface when C is NURBS and the frame is sampled and lofted; gives the arc-spline route to a robust frame and the closed-spine frame-closure condition.
Kernel relevance. The canonical "sweep -> NURBS" recipe Keel bakes to; supplies the guarantee that a NURBS profile yields a NURBS swept face.

Citation. Siltanen, P., & Woodward, C. (1992). Normal orientation methods for 3D offset curves, sweep surfaces and skinning. Computer Graphics Forum (Eurographics), 11(3), 449-458. https://doi.org/10.1111/1467-8659.1130449
Content. Presents discrete normal-orientation (rotation) transport for offset curves, sweep surfaces, and skinning; the same frame field drives all three constructions; profile is kept perpendicular to the spine tangent.
Kernel relevance. Unifies sweep framing with the skinning code Keel reuses in the bake step; confirms one frame field serves both the sweep and the loft.

Citation. Piegl, L., & Tiller, W. (1996). Algorithm for approximate NURBS skinning. Computer-Aided Design, 28(9), 699-706. https://doi.org/10.1016/0010-4485(96)00012-1
Content. Fits a NURBS surface through a set of cross-section curves to a stated tolerance, controlling knot count; rebuilds the input sections within the error bound and merges section knot vectors.
Kernel relevance. This is the skinning kernel Keel's step 4-5 calls; it provides the tolerance-controlled loft and the knot-merge that turns the placed sections into one NURBS swept face.

Citation. Bloomenthal, J., & Riesenfeld, R. (1991). Approximation of sweep surfaces by tensor product B-splines / generalized-cylinder modeling (Graphics Gems line). https://webhome.cs.uvic.ca/~blob/courses/305/notes/pdf/ref-frames.pdf
Content. Builds the generalized cylinder by stacking framed profile copies and tensor-product fitting; sampling density set by curve flatness.
Kernel relevance. Supports the adaptive sampling rule of Section 2.3: density follows spine flatness and profile extent.

---

## 3. Self-intersection when path curvature radius < profile radius

### 3.1 The grazing condition

A sweep folds onto itself, locally, exactly where the profile is large relative to how tightly the spine bends. The cleanest statement is the canal/pipe-surface regularity condition: a tube of radius r swept on a spine of curvature kappa is free of LOCAL self-intersection iff

    r * kappa < 1     i.e.     r < 1/kappa = rho,   the radius of curvature.

When r = rho the offset on the concave side of the bend collapses to a point (the center of curvature); when r > rho it crosses through the center and the surface folds back, producing a cusp/loop. For a general (non-circular) profile, R_p = max radius of the profile measured from the frame origin in the bend's concave direction plays the role of r. The implementable rule:

    LOCAL self-intersection wherever  R_p(direction of concavity) >= 1/kappa(u) = rho(u),
    define  r_max(u) = 1/kappa(u) = local radius of curvature of the spine.

This is the same r_max = local-curvature-radius rule as the blend overflow analysis in `docs/research/kernel/41-blend-overflow-radius-feature-failures.md` (a fillet of radius r cannot fit where the local feature curvature gives rho < r): the medial / curvature-radius ceiling. It is also the Section-10 grazing condition: the swept surface degenerates where the envelope normal grazes, which is the Jacobian rank-deficiency / singular set of the sweep map (Abdel-Malek). The first fundamental check is therefore per-sample and cheap: compute kappa(u_i) from the spine, compare R_p against 1/kappa(u_i).

### 3.2 Local vs global

- Local self-intersection: the r < rho test above; detectable analytically per spine sample. The surface Jacobian S_u x S_v vanishes where (1 - kappa * c) changes sign (c the in-plane offset toward the center of curvature). This is the dominant, cheap-to-catch case.
- Global self-intersection: distant parts of the sweep overlap even though every local patch is regular (a tube on a spine that loops back near itself, like a tight helix whose pitch is small relative to its radius+R_p). This needs a global surface-surface / self-SSI test or a swept-volume boundary computation; it is not caught by the curvature rule.

### 3.3 Trim-or-decline policy for Keel

1. Pre-flight curvature scan (cheap, always run). For each spine sample compute rho_i = 1/kappa_i. Let R_p be the profile's max in-plane radius (toward the concave side; for a symmetric/circular profile just the max radius). If R_p < min_i rho_i with margin (e.g. R_p <= (1 - delta) min rho_i), the sweep is locally regular; proceed to bake.
2. Local fold detected (R_p >= rho_i somewhere). Two policies, chosen by caller intent:
   - DECLINE (default, safest for a kernel): reject the operation with a precise diagnostic, "profile radius R_p exceeds spine radius of curvature rho = 1/kappa at u = u*; sweep would self-intersect," and report u*. This mirrors blend overflow's decline-rather-than-produce-garbage stance in dossier 41.
   - TRIM: build the procedural/lofted surface anyway, then compute the self-intersection locus (the fold curve = where the surface Jacobian is singular, the envelope's characteristic) and trim the surface to its valid (outer-envelope) portion, discarding the folded sheet. This is the swept-volume boundary extraction of Abdel-Malek (substitute the singular set into the sweep equation to get the true boundary). Only offer TRIM when the caller asked for a swept VOLUME boundary, where the fold is interior and discardable; for a swept SURFACE the fold is a genuine defect and DECLINE is correct.
3. Global overlap. Always run a coarse self-bounding-box / self-SSI check on the baked surface after step 1-2 passes; if distant patches overlap, DECLINE with a "global self-intersection" diagnostic (do not attempt automatic global trimming in v1).

### 3.4 Sources for Section 3

Citation. Wikipedia / Hartmann, channel (canal) surfaces; and Maekawa, T., Patrikalakis, N.M., et al. (1998). Analysis and applications of pipe surfaces. Computer Aided Geometric Design, 15(5), 437-458. https://doi.org/10.1016/S0167-8396(97)00042-3
Content. Canal surface as the envelope of spheres f(x,u) = |x - c(u)|^2 - r(u)^2 = 0 with f_u = 0; regularity requires |r'| < |c'| and, for the pipe (constant r), r < radius of curvature of the spine; the parametric form carries a sqrt(1 - r'^2/|c'|^2) factor that turns imaginary at the singular condition. Pipe-surface paper analyzes both local and global self-intersection.
Kernel relevance. Supplies the exact r * kappa < 1 inequality that is Keel's local-fold test; the local/global split structures Section 3.2-3.3.

Citation. Abdel-Malek, K., Yang, J., Blackmore, D., & Joy, K. (2006 review; and 2000 implicit-surface note). Swept volumes: foundations, perspectives, and applications. Plus: Geometric representation of the swept volume using Jacobian rank-deficiency conditions, Computer-Aided Design, 29(6), 1997. https://doi.org/10.1016/S0010-4485(96)00097-8
Content. The swept-volume boundary is found by imposing rank deficiency on the sweep Jacobian to get the singular set, then substituting it back into the sweep equation; this yields the envelope and AVOIDS separate trimming for self-intersecting cases.
Kernel relevance. The rank-deficiency singular set is exactly the fold curve Keel trims to in the TRIM policy; gives the principled boundary for swept-volume mode.

Citation. (Cross-reference) Keel dossier 10, Minkowski / medial / midsurface, and dossier 41, blend overflow and radius-vs-feature failures.
Content. Dossier 10: swept-volume boundary as envelope, grazing condition via medial axis. Dossier 41: r_max = local curvature radius; a feature of radius r cannot be realized where rho < r.
Kernel relevance. The sweep self-intersection rule R_p >= 1/kappa is the same ceiling as blend overflow; reuse dossier 41's r_max machinery and dossier 10's envelope/grazing test rather than writing new self-intersection code.

---

## 4. Scale and twist laws, end caps, and polyline joints

### 4.1 Variable scale and twist laws

Generalize the placement to

    S(u, v) = A(u) + sigma(u) * R(phi(u)) * F(u) * C(v)

where sigma(u) is a scalar SCALE law, phi(u) is a TWIST angle law, and R(phi) is rotation by phi in the (r, s) normal plane:

    placed profile axis = cos(phi(u)) r(u) + sin(phi(u)) s(u)  (and the orthogonal partner),
    radial extent scaled by sigma(u).

Practical laws: sigma and phi as 1D B-spline / linear functions of arc length (so they are parametrization-independent). The twist phi is ADDED ON TOP of the RMF; because the RMF itself contributes zero twist, phi(u) is the exact, predictable twist the user asked for. Total twist Phi = phi(u_n) - phi(u_0) distributed by the law (evenly if linear in arc length). Important: feed phi and sigma into the sampling-density rule (Section 2.3): a steep twist on a large profile needs denser samples to keep the loft within tolerance.

### 4.2 End caps

The swept surface S is open at u_0 and u_n. To make a solid, cap each end with the planar (or profile-spanning) face bounded by the first/last section curve C_0, C_n. For a planar profile the cap is the trimmed plane of that profile placed in F(u_0) / F(u_n); for a non-planar profile the cap is a surface filling the section loop (n-sided fill, dossier 26). In the PES, the cap face shares the boundary edge with the swept face; orient the cap normal outward (away from the spine interior). For a CLOSED spine (loop) there are no caps; instead the frame must CLOSE: the transported F(u_n) generally differs from F(u_0) by a residual twist (the holonomy of parallel transport around the loop), so distribute a corrective counter-twist -holonomy/length along the spine to make the surface seam G1/periodic (Wang-Joe closed-spine handling).

### 4.3 G1 at polyline path joints: miter vs round

A polyline (or piecewise-G1) spine has tangent discontinuities at its joints. The RMF transport handles the STRAIGHT spans trivially (parallel transport, no rotation, no Frenet problem). The question is how to carry the frame and the profile ACROSS a sharp corner where t jumps from t_in to t_out. Two policies:

- MITER (sharp corner kept). Place a single profile at the joint in the BISECTOR plane: the profile section sits in the plane whose normal is the normalized bisector (t_in + t_out)/|t_in + t_out|, and the two adjacent swept faces meet at the mitered section, exactly like a pipe elbow cut on the angle bisector. The frame is carried across by reflecting r through the bisector plane (the same single reflection as the double-reflection step's second reflection with v2 = t_out - t_in), which is the minimal rotation taking t_in to t_out and keeps r continuous. This preserves a sharp G0 crease at the joint and is correct for "keep corners sharp" (Mitre) requests. Self-intersection at the inside of the miter is bounded by the same R_p < rho rule with rho -> infinity on the straight legs, so the only fold risk is if the profile is larger than the inside miter pocket: check R_p against the corner half-angle.
- ROUND (fillet the joint). Replace the sharp corner with a small G1 circular arc of radius rho_joint blended into the two legs, then the spine is C1 and the RMF transports smoothly with no special joint handling. This requires rho_joint > R_p (the arc must not itself induce a fold, Section 3) and is the recommendation when the user wants a smooth swept solid through the bend. CAD systems expose exactly this miter-vs-round choice at sweep corners.

Recommendation for Keel: default to ROUND when the caller supplies a corner radius or requests a smooth sweep (it sidesteps both the frame discontinuity and the crease), and MITER (bisector-plane placement, single reflection of the frame) when the caller requests sharp corners or supplies a true polyline with the "keep sharp" flag. In both cases the across-joint frame update is the single reflection in the bisector plane, so it is one line of the existing double-reflection code.

### 4.4 Sources for Section 4

Citation. Plasticity / Shapr3D / IronCAD sweep documentation (production CAD behavior). https://doc.plasticity.xyz/solid/sweep ; https://support.shapr3d.com/hc/en-us/articles/7874456833948-Sweep
Content. Twist rotates the profile by a total angle distributed evenly along the path; scale grows/shrinks the profile along the path; corner type Mitre keeps corners sharp vs rounded; sharp-corner paths can self-intersect and may need fillets.
Kernel relevance. Confirms the user-facing scale/twist/corner model Keel should expose and the miter-vs-round corner policy of Section 4.3.

Citation. Wang, W., & Joe, B. (1997). Robust computation of the rotation minimizing frame for sweep surface modeling. Computer-Aided Design, 29(5), 379-391. https://doi.org/10.1016/S0010-4485(96)00077-2
Content. Closed-spine frame closure: the transported frame must be made periodic by distributing the parallel-transport holonomy as a counter-twist around the loop.
Kernel relevance. Drives Keel's closed-spine end handling (no caps; periodic seam) in Section 4.2.

Citation. Pottmann, H., & Wallner, J. (2001). Computational Line Geometry. Springer. ISBN 978-3-540-42058-3. https://doi.org/10.1007/978-3-642-04018-4
Content. Canal-surface and moving-frame theory; rational sweep/canal parameterizations; envelope of a sphere family; the kinematics of a profile moved by a rigid motion along a spine.
Kernel relevance. Theory backstop for the moving-frame placement S = A + sigma R(phi) F C and for the exact rational cases (constant-radius tube on a rational spine) where Keel could store the sweep exactly rather than baking.

---

## Sweep for Keel (synthesis)

Concrete, testable recommendations for roadmap item 63. Validate each against Keel's existing tests; this is a design proposal, not a verified implementation.

- RMF transport choice. Use the WANG-JUTTLER-ZHENG-LIU DOUBLE-REFLECTION method to transport the frame along the spine. Two reflections per step (Reflect_w(X) = X - 2 (w.X)/(w.w) w, normals v1 = x_{i+1}-x_i and v2 = t_{i+1}-t_i^L), O(h^4) global error, exact on circular arcs, no Gram-Schmidt drift. Do NOT use the Frenet frame: it flips at inflections, is undefined on straight spans (fatal for the L-path), and injects spurious torsion twist. Keep Klok projection (O(h^2)) only as a cheap fallback.

- Representation. PROCEDURAL EVALUATOR BAKED TO NURBS. Place the (scaled, twisted) profile in each sampled frame to get section curves, then tolerance-controlled skin (Piegl-Tiller approximate skinning) into one NURBS swept face so the kernel's existing SSI/boolean/tessellation code applies unchanged. Keep the procedural form only as the internal high-accuracy evaluator used while sampling. Sampling density from the chord-height bound h <= 2 sqrt(2 eps_s (1/kappa_max + R_p)), refined adaptively by procedural-vs-lofted midpoint deviation, with extra density where twist or profile extent is large.

- Self-intersection rule. The r_max = LOCAL CURVATURE RADIUS ceiling: local fold iff R_p >= rho(u) = 1/kappa(u), i.e. r * kappa >= 1. This is the same inequality as blend overflow (dossier 41) and the grazing/Jacobian-singular condition (dossier 10, Abdel-Malek). Pre-flight curvature scan per sample; DECLINE with the offending u for a swept SURFACE; offer TRIM to the rank-deficiency singular set only for swept-VOLUME boundary mode. Always run a coarse global self-SSI check and decline on global overlap.

- Polyline-joint handling. Straight spans transport trivially (no Frenet issue). At a sharp joint, carry the frame across by a SINGLE REFLECTION in the bisector plane (normal proportional to t_in + t_out), which is the minimal rotation taking t_in to t_out and keeps r continuous. Offer MITER (profile in the bisector plane, sharp G0 crease, the default for "keep sharp") and ROUND (replace the corner with a G1 arc of radius rho_joint > R_p, the default for a smooth sweep). End caps: planar/n-sided fill of the end sections oriented outward; closed spines take no caps but require holonomy counter-twist for a periodic seam.

### Validation cases

- Circular profile along an L-shaped path. The two legs are straight: the Frenet frame is undefined here, the RMF parallel-transports the profile unchanged along each leg, so a circular tube comes through straight with no twist. At the corner: ROUND with a fillet radius rho_joint > tube radius gives a smooth elbow; MITER gives a bisector-plane joint (a mitered elbow). Expected result: a constant-radius tube, no self-intersection provided the elbow fillet radius exceeds the tube radius (R_p < rho_joint); volume/area match the analytic two-legs-plus-quarter-torus value within eps_s. This case specifically proves the RMF beats Frenet, since Frenet has no frame on the straight legs.
- Circular profile along a helix. The helix has CONSTANT curvature and torsion; double reflection is exact on the osculating arcs, so the RMF is reproduced to machine precision and the swept tube is the exact helical pipe (a known canal surface). Frenet would add the torsion twist tau s, visibly rotating a non-circular profile; for the circular profile it would still be the right surface but with a wrongly-parametrized frame. Self-intersection check: the tube is regular iff tube radius < helix radius of curvature rho = (a^2 + b^2)/a for helix radius a and pitch parameter b; Keel's r >= rho rule must DECLINE when the tube is fatter than that. Validate the baked NURBS tube against the analytic helical-pipe area/volume within eps_s, and confirm zero accumulated twist of a marked profile axis (RMF) versus the nonzero Frenet twist.

---

## References

1. Wang, W., Juttler, B., Zheng, D., & Liu, Y. (2008). Computation of rotation minimizing frames. ACM Transactions on Graphics, 27(1), Article 2. https://doi.org/10.1145/1330511.1330513
2. Wang, W., & Zheng, J. Computation of rotation minimizing frame in computer graphics. https://www.semanticscholar.org/paper/779374741a387cff101e70c0ad13282b7adc5ded
3. Klok, F. (1986). Two moving coordinate frames for sweeping along a 3D trajectory. Computer Aided Geometric Design, 3(3), 217-229. https://doi.org/10.1016/0167-8396(86)90039-7
4. Bloomenthal, J. (1990). Calculation of reference frames along a space curve. Graphics Gems, 567-571. https://webhome.cs.uvic.ca/~blob/courses/305/notes/pdf/ref-frames.pdf
5. Wang, W., & Joe, B. (1997). Robust computation of the rotation minimizing frame for sweep surface modeling. Computer-Aided Design, 29(5), 379-391. https://doi.org/10.1016/S0010-4485(96)00077-2
6. Siltanen, P., & Woodward, C. (1992). Normal orientation methods for 3D offset curves, sweep surfaces and skinning. Computer Graphics Forum, 11(3), 449-458. https://doi.org/10.1111/1467-8659.1130449
7. Piegl, L., & Tiller, W. (1996). Algorithm for approximate NURBS skinning. Computer-Aided Design, 28(9), 699-706. https://doi.org/10.1016/0010-4485(96)00012-1
8. Maekawa, T., Patrikalakis, N. M., et al. (1998). Analysis and applications of pipe surfaces. Computer Aided Geometric Design, 15(5), 437-458. https://doi.org/10.1016/S0167-8396(97)00042-3
9. Hartmann, E. / Wikipedia. Channel (canal) surface. https://en.wikipedia.org/wiki/Channel_surface
10. Abdel-Malek, K., & Yeh, H. J. (1997). Geometric representation of the swept volume using Jacobian rank-deficiency conditions. Computer-Aided Design, 29(6), 457-468. https://doi.org/10.1016/S0010-4485(96)00097-8
11. Abdel-Malek, K., Yang, J., Blackmore, D., & Joy, K. Swept volumes: foundations, perspectives, and applications. https://user.engineering.uiowa.edu/~amalek/papers/swept-volume-review.pdf
12. Abdel-Malek, K., et al. On swept volume formulations: implicit surfaces. Computer-Aided Design. https://doi.org/10.1016/S0010-4485(00)00065-8
13. Pottmann, H., & Wallner, J. (2001). Computational Line Geometry. Springer. https://doi.org/10.1007/978-3-642-04018-4
14. Bloomenthal, J., & Riesenfeld, R. Approximation of sweep surfaces by tensor-product B-splines (generalized cylinders).
15. Plasticity sweep documentation. https://doc.plasticity.xyz/solid/sweep
16. Shapr3D sweep documentation. https://support.shapr3d.com/hc/en-us/articles/7874456833948-Sweep
17. Mossman, A., et al. (2023). Balancing rotation minimizing frames with additional objectives. Computer Graphics Forum, 42. https://doi.org/10.1111/cgf.14979

Cross-references within Keel research: dossier 05 (offsets/blends/sweeps survey: double-reflection RMF, canal surfaces), dossier 10 (Minkowski/medial/midsurface: swept-volume envelope, grazing), dossier 26 (transfinite n-sided surfacing: non-planar end caps), dossier 41 (blend overflow, r_max = curvature radius), and the planned skinning/lofting dossier (the loft step in Section 2).
