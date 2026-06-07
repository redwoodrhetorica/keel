//! Certified-global closest-point projection onto NURBS curves:
//! Bezier decomposition, convex-hull branch-and-bound pruning, and
//! bracketed-Newton polish (Ma-Hewitt / Selimovic pattern, research
//! dossier nurbs/04 section 5).

use crate::GeomError;
use crate::nurbs_curve::{BezierSegment, NurbsCurve};
use crate::nurbs_surface::{BezierPatch, NurbsSurface};
use crate::surface::SurfaceProjection;
use keel_math::bbox::Aabb3;
use keel_math::vec::Vec3;

/// Result of a closest-point query.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    /// Parameter of the closest point in the curve's domain.
    pub u: f64,
    /// The closest point.
    pub point: Vec3,
    /// Euclidean distance.
    pub distance: f64,
}

/// Distance from p to an AABB (zero inside): the pruning lower bound,
/// valid because the curve lies in the hull of its control points.
fn dist_to_aabb(p: Vec3, bb: &Aabb3) -> f64 {
    let dx = (bb.min.x - p.x).max(0.0).max(p.x - bb.max.x);
    let dy = (bb.min.y - p.y).max(0.0).max(p.y - bb.max.y);
    let dz = (bb.min.z - p.z).max(0.0).max(p.z - bb.max.z);
    Vec3::new(dx, dy, dz).norm()
}

/// Globally closest point on `curve` to `p`.
///
/// Algorithm: decompose to Bezier segments; branch-and-bound with the
/// control-AABB lower bound against the best sample upper bound,
/// subdividing surviving segments until their parameter width is below
/// `width_tol`; polish each survivor with bracketed Newton on
/// g(u) = (C(u) - p) . C'(u) and keep the global best.
pub fn project_point(curve: &NurbsCurve, p: Vec3, width_tol: f64) -> Projection {
    let (dom_a, dom_b) = curve.domain();
    // Seed best with the domain endpoints.
    let pa = curve.point(dom_a);
    let mut best = Projection {
        u: dom_a,
        point: pa,
        distance: (pa - p).norm(),
    };
    let pb = curve.point(dom_b);
    let db = (pb - p).norm();
    if db < best.distance {
        best = Projection {
            u: dom_b,
            point: pb,
            distance: db,
        };
    }

    let mut stack: Vec<BezierSegment> = curve.to_beziers();
    let mut survivors: Vec<BezierSegment> = Vec::new();
    while let Some(seg) = stack.pop() {
        let bb = Aabb3::from_points(seg.control_points());
        if dist_to_aabb(p, &bb) > best.distance + 1e-12 {
            continue; // provably no closer point in this segment
        }
        // Tighten the upper bound from the segment midpoint.
        let mid = seg.point(0.5);
        let dm = (mid - p).norm();
        if dm < best.distance {
            best = Projection {
                u: 0.5 * seg.u0 + 0.5 * seg.u1,
                point: mid,
                distance: dm,
            };
        }
        if seg.u1 - seg.u0 <= width_tol {
            survivors.push(seg);
        } else {
            let (l, r) = seg.subdivide(0.5);
            stack.push(l);
            stack.push(r);
        }
    }

    // Newton polish on each survivor, in the parent parameter.
    for seg in survivors {
        let g = |u: f64| {
            let d = curve.derivatives(u, 2);
            let e = d[0] - p;
            (e.dot(d[1]), d[1].dot(d[1]) + e.dot(d[2]))
        };
        let (g0, _) = g(seg.u0);
        let (g1, _) = g(seg.u1);
        let candidate = if g0 == 0.0 {
            Some(seg.u0)
        } else if g1 == 0.0 {
            Some(seg.u1)
        } else if (g0 > 0.0) != (g1 > 0.0) {
            keel_math::newton::solve_bracketed(g, seg.u0, seg.u1, 1e-14, 64)
        } else {
            None
        };
        if let Some(u) = candidate {
            let q = curve.point(u);
            let d = (q - p).norm();
            if d < best.distance {
                best = Projection {
                    u,
                    point: q,
                    distance: d,
                };
            }
        }
    }
    best
}

/// Globally closest point on a NURBS surface to `p`: Bezier patch
/// decomposition, branch-and-bound on control AABBs (lower bound:
/// distance to the box; upper bound: best polished sample), damped
/// 2x2 Newton on g = ((S - p) . S_u, (S - p) . S_v) clamped to each
/// surviving patch rectangle. Mirrors the curve projector.
pub fn project_point_surface(
    srf: &NurbsSurface,
    p: Vec3,
    width_tol: f64,
) -> Result<SurfaceProjection, GeomError> {
    let patches = srf.to_bezier_patches()?;
    let mut best = SurfaceProjection {
        u: 0.0,
        v: 0.0,
        point: Vec3::ZERO,
        distance: f64::INFINITY,
    };
    // Boundary minima first: where the closest point lies on a surface
    // edge the orthogonality system has no interior root and clamped
    // Newton under-optimizes the tangential coordinate; the certified
    // curve projector on the four boundary iso-curves covers exactly
    // those candidates.
    let ((u0, u1), (v0, v1)) = srf.domain();
    for (idx, bc) in srf.boundary_curves().iter().enumerate() {
        let pr = project_point(bc, p, width_tol);
        if pr.distance < best.distance {
            let (u, v) = match idx {
                0 => (u0, pr.u),
                1 => (u1, pr.u),
                2 => (pr.u, v0),
                _ => (pr.u, v1),
            };
            best = SurfaceProjection {
                u,
                v,
                point: pr.point,
                distance: pr.distance,
            };
        }
    }
    // Seed the interior upper bound from every patch midpoint (polished).
    for pc in &patches {
        polish_and_update(srf, pc, p, &mut best);
    }
    let mut stack: Vec<BezierPatch> = patches;
    let mut guard = 0usize;
    while let Some(pc) = stack.pop() {
        guard += 1;
        if guard > 100_000 {
            break; // pathological subdivision; return best-so-far
        }
        let (lo, hi) = pc.control_aabb();
        if dist_to_aabb(p, &Aabb3 { min: lo, max: hi }) > best.distance + 1e-12 {
            continue; // provably no closer point in this patch
        }
        let du = pc.u1 - pc.u0;
        let dv = pc.v1 - pc.v0;
        if du <= width_tol && dv <= width_tol {
            polish_and_update(srf, &pc, p, &mut best);
            continue;
        }
        let (a, b) = if du >= dv {
            pc.subdivide_u(0.5)
        } else {
            pc.subdivide_v(0.5)
        };
        for half in [a, b] {
            polish_and_update(srf, &half, p, &mut best);
            let (lo, hi) = half.control_aabb();
            if dist_to_aabb(p, &Aabb3 { min: lo, max: hi }) <= best.distance + 1e-12 {
                stack.push(half);
            }
        }
    }
    Ok(best)
}

/// Damped 2x2 Newton on the orthogonality system from the patch
/// midpoint, clamped to the patch rectangle; update the global best.
fn polish_and_update(srf: &NurbsSurface, pc: &BezierPatch, p: Vec3, best: &mut SurfaceProjection) {
    let mut u = 0.5 * pc.u0 + 0.5 * pc.u1;
    let mut v = 0.5 * pc.v0 + 0.5 * pc.v1;
    for _ in 0..30 {
        let d = srf.derivatives(u, v, 2);
        let dl = d[0][0] - p;
        let (su, sv) = (d[1][0], d[0][1]);
        let g1 = dl.dot(su);
        let g2 = dl.dot(sv);
        let j11 = su.dot(su) + dl.dot(d[2][0]);
        let j12 = su.dot(sv) + dl.dot(d[1][1]);
        let j22 = sv.dot(sv) + dl.dot(d[0][2]);
        let det = j11 * j22 - j12 * j12;
        if det.abs() < 1e-300 {
            break;
        }
        // Solve J [du dv]^T = -[g1 g2]^T (Cramer).
        let step_u = (-g1 * j22 + g2 * j12) / det;
        let step_v = (-g2 * j11 + g1 * j12) / det;
        let nu_ = (u + step_u).clamp(pc.u0, pc.u1);
        let nv_ = (v + step_v).clamp(pc.v0, pc.v1);
        let moved = (nu_ - u).abs() + (nv_ - v).abs();
        u = nu_;
        v = nv_;
        if moved < 1e-14 * (1.0 + u.abs() + v.abs()) {
            break;
        }
    }
    let q = srf.point(u, v);
    let dist = (q - p).norm();
    if dist < best.distance {
        *best = SurfaceProjection {
            u,
            v,
            point: q,
            distance: dist,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nurbs_curve::NurbsCurve;
    use proptest::prelude::*;

    fn circle() -> NurbsCurve {
        NurbsCurve::full_circle(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            2.0,
        )
        .unwrap()
    }

    #[test]
    fn projects_onto_circle_radially() {
        let c = circle();
        // A point at radius 5 in the plane projects to radius 2 along
        // the same ray.
        let p = Vec3::new(3.0, 4.0, 0.0); // norm 5, direction (0.6, 0.8)
        let pr = project_point(&c, p, 1e-4);
        let expected = Vec3::new(1.2, 1.6, 0.0);
        assert!((pr.point - expected).norm() < 1e-8, "{pr:?}");
        assert!((pr.distance - 3.0).abs() < 1e-8);
    }

    #[test]
    fn on_curve_point_projects_to_itself() {
        let c = circle();
        let q = c.point(0.37);
        let pr = project_point(&c, q, 1e-4);
        assert!(pr.distance < 1e-9);
        assert!((pr.point - q).norm() < 1e-9);
    }

    #[test]
    fn projects_onto_revolved_sphere_radially() {
        let profile = NurbsCurve::circular_arc(
            Vec3::ZERO,
            Vec3::new(0., 0., -1.),
            Vec3::new(1., 0., 0.),
            2.0,
            core::f64::consts::PI,
        )
        .unwrap();
        let s = crate::nurbs_surface::revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.))
            .unwrap();
        let pr = project_point_surface(&s, Vec3::new(5.0, 0.0, 0.0), 1e-4).unwrap();
        assert!((pr.distance - 3.0).abs() < 1e-7, "{pr:?}");
        assert!(
            (pr.point - Vec3::new(2.0, 0.0, 0.0)).norm() < 1e-6,
            "{pr:?}"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 16, ..ProptestConfig::default() })]
        // Dense-sampling global-optimality oracle for surfaces.
        #[test]
        fn surface_projection_is_global(
            (s, _, _) in crate::nurbs_surface::test_support::arb_surface_uv(),
            px in -12.0..12.0f64,
            py in -12.0..12.0f64,
            pz in -12.0..12.0f64,
        ) {
            let p = Vec3::new(px, py, pz);
            let pr = project_point_surface(&s, p, 1e-4).unwrap();
            let ((u0, u1), (v0, v1)) = s.domain();
            for i in 0..=24 {
                for j in 0..=24 {
                    let q = s.point(
                        u0 + (u1 - u0) * i as f64 / 24.0,
                        v0 + (v1 - v0) * j as f64 / 24.0,
                    );
                    prop_assert!(
                        (q - p).norm() >= pr.distance - 1e-6,
                        "sample beats projection: {} < {}", (q - p).norm(), pr.distance
                    );
                }
            }
        }
    }

    proptest! {
        // Sample the curve densely; the projection distance must not
        // exceed the distance to any sample (global optimality).
        #[test]
        fn projection_beats_dense_sampling(
            c in crate::nurbs_curve::test_support::arb_nurbs(),
            px in -150.0..150.0f64,
            py in -150.0..150.0f64,
            pz in -150.0..150.0f64,
        ) {
            let p = Vec3::new(px, py, pz);
            let pr = project_point(&c, p, 1e-5);
            for i in 0..=200 {
                let t = i as f64 / 200.0;
                let d = (c.point(t) - p).norm();
                prop_assert!(
                    pr.distance <= d + 1e-6,
                    "projection {} beaten at t={t} with {}", pr.distance, d
                );
            }
        }
    }
}
