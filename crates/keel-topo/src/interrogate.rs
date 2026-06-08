//! Model interrogation queries (parity Phase: interrogation). Bounding
//! boxes and minimum distance between bodies, built on the same
//! outward-triangle tessellation the winding classifier uses.

use crate::Body;
use keel_math::bbox::Aabb3;
use keel_math::vec::Vec3;

/// Closest point on triangle `[a, b, c]` to `p` (Ericson, Real-Time
/// Collision Detection), then the distance.
fn point_tri_distance(p: Vec3, tri: &[Vec3; 3]) -> f64 {
    let (a, b, c) = (tri[0], tri[1], tri[2]);
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (p - a).norm(); // vertex region A
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (p - b).norm(); // vertex region B
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (p - (a + ab * v)).norm(); // edge AB
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (p - c).norm(); // vertex region C
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (p - (a + ac * w)).norm(); // edge AC
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (p - (b + (c - b) * w)).norm(); // edge BC
    }
    // Interior: barycentric projection onto the plane.
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    (p - (a + ab * v + ac * w)).norm()
}

impl Body {
    /// All outward triangles of the body (the tessellation the winding
    /// classifier and volume use).
    fn all_triangles(&self) -> Vec<[Vec3; 3]> {
        self.face_keys()
            .iter()
            .flat_map(|&f| self.tessellate_face(f))
            .collect()
    }

    /// Axis-aligned bounding box of the body (parity item 105). Tight
    /// from the tessellation: exact for planar faces, tessellation-tight
    /// for curved (a fast refinement to exact analytic extrema is a
    /// later improvement). Empty bodies return an inverted/empty box.
    pub fn bounding_box(&self) -> Aabb3 {
        let pts: Vec<Vec3> = self.all_triangles().into_iter().flatten().collect();
        Aabb3::from_points(pts)
    }

    /// Minimum distance between the surfaces of two bodies (parity item
    /// 101). Computed symmetrically as the min over each body's
    /// tessellation vertices of the point-to-triangle distance to the
    /// other body; 0 (within tessellation resolution) when they touch or
    /// interpenetrate. This is a tessellation-resolution approximation of
    /// the exact surface min-distance (exact min-distance via face-pair
    /// surface projection is a later refinement).
    pub fn min_distance(&self, other: &Body) -> f64 {
        let a = self.all_triangles();
        let b = other.all_triangles();
        if a.is_empty() || b.is_empty() {
            return f64::INFINITY;
        }
        let mut best = f64::INFINITY;
        for tri in &a {
            for &v in tri {
                for tb in &b {
                    best = best.min(point_tri_distance(v, tb));
                }
            }
        }
        for tri in &b {
            for &v in tri {
                for ta in &a {
                    best = best.min(point_tri_distance(v, ta));
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;

    fn z_sphere(center: Vec3, r: f64) -> Body {
        let mut b = Body::new();
        let frame = Frame3 {
            origin: center,
            x: Vec3::new(0., 1., 0.),
            y: Vec3::new(0., 0., 1.),
            z: Vec3::new(1., 0., 0.),
        };
        b.sphere(frame, r).unwrap();
        b
    }

    #[test]
    fn bounding_box_of_block_is_exact() {
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 2.0, 3.0), 4.0, 5.0, 6.0).unwrap();
        let bb = b.bounding_box();
        assert!(
            (bb.min - Vec3::new(1.0, 2.0, 3.0)).norm() < 1e-9,
            "min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(5.0, 7.0, 9.0)).norm() < 1e-9,
            "max {:?}",
            bb.max
        );
    }

    #[test]
    fn bounding_box_of_sphere_is_tight() {
        let b = z_sphere(Vec3::new(0.5, -1.0, 2.0), 2.0);
        let bb = b.bounding_box();
        // Tessellation-tight: within a small fraction of the radius.
        assert!(
            (bb.min - Vec3::new(-1.5, -3.0, 0.0)).norm() < 0.05,
            "min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(2.5, 1.0, 4.0)).norm() < 0.05,
            "max {:?}",
            bb.max
        );
    }

    #[test]
    fn min_distance_between_separated_spheres() {
        // Centres 5 apart, radii 1 and 1.5 -> surface gap ~2.5.
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(5.0, 0.0, 0.0), 1.5);
        let d = a.min_distance(&b);
        assert!((d - 2.5).abs() < 0.1, "min distance {d} vs ~2.5");
    }

    #[test]
    fn min_distance_zero_when_overlapping() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(1.0, 0.0, 0.0), 1.0);
        assert!(a.min_distance(&b) < 0.1, "overlapping spheres should be ~0");
    }
}
