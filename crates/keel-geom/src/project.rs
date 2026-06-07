//! Certified-global closest-point projection onto NURBS curves:
//! Bezier decomposition, convex-hull branch-and-bound pruning, and
//! bracketed-Newton polish (Ma-Hewitt / Selimovic pattern, research
//! dossier nurbs/04 section 5).

use crate::nurbs_curve::{BezierSegment, NurbsCurve};
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
    let mut best = Projection { u: dom_a, point: pa, distance: (pa - p).norm() };
    let pb = curve.point(dom_b);
    let db = (pb - p).norm();
    if db < best.distance {
        best = Projection { u: dom_b, point: pb, distance: db };
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
                best = Projection { u, point: q, distance: d };
            }
        }
    }
    best
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
