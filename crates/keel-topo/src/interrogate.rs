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

    /// Area of a single face (parity interrogation), summed over its
    /// outward triangles. Exact for planar faces; the tessellation
    /// approximation for curved faces (consistent with the curved volume
    /// oracle -- exact analytic area is a later refinement).
    pub fn face_area(&self, face: crate::entity::FaceKey) -> f64 {
        self.tessellate_face(face)
            .iter()
            .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
            .sum()
    }

    /// Total surface area of the body (parity interrogation): the sum of
    /// every face's area. Exact for all-planar bodies.
    pub fn surface_area(&self) -> f64 {
        self.face_keys().iter().map(|&f| self.face_area(f)).sum()
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

    /// Do two bodies clash / interfere (parity item 102)? True if their
    /// surfaces cross or touch (the SSI yields seam curves, or a
    /// coincident/tangent contact), or one is nested in the other (a
    /// surface point of one lies inside the other by the generalized
    /// winding number). A bounding-box miss is the cheap reject. Fast:
    /// the analytic SSI plus two winding-number probes, not an O(n*m)
    /// tessellation sweep.
    pub fn clashes(&self, other: &Body) -> bool {
        use crate::boolean::BoolFault;
        if !self.bounding_box().intersects(other.bounding_box()) {
            return false;
        }
        // Surfaces cross or are in coincident/tangent contact.
        let (seams, faults) = crate::boolean::seam_curves(self, other, 1e-7);
        if !seams.is_empty()
            || faults
                .iter()
                .any(|f| matches!(f, BoolFault::Coincident(..) | BoolFault::Tangent(..)))
        {
            return true;
        }
        // No surface contact: test nesting with one representative point
        // from each body against the other's interior.
        if let Some(p) = self.all_triangles().first().map(|t| t[0])
            && other.generalized_winding_number(p) > 0.5
        {
            return true;
        }
        if let Some(p) = other.all_triangles().first().map(|t| t[0])
            && self.generalized_winding_number(p) > 0.5
        {
            return true;
        }
        false
    }

    /// Non-destructive section of the body by a plane (parity item 75):
    /// the ordered polygon where the plane cuts the body's straight
    /// edges. For a convex polyhedron this is the cross-section outline.
    /// (Curved-edge crossings and multi-loop sections are a later slice.)
    pub fn section_by_plane(&self, plane_point: Vec3, plane_normal: Vec3) -> Vec<Vec3> {
        let Some(n) = plane_normal.try_normalize() else {
            return Vec::new();
        };
        let d = n.dot(plane_point);
        let mut pts: Vec<Vec3> = Vec::new();
        for (_, e) in self.edges.iter() {
            let (v0, v1) = e.bounds;
            let (Some(a), Some(b)) = (self.vertices.get(v0), self.vertices.get(v1)) else {
                continue;
            };
            let (s0, s1) = (n.dot(a.point) - d, n.dot(b.point) - d);
            // Edge straddles the plane: linear crossing.
            if (s0 > 0.0) != (s1 > 0.0) && (s0 - s1).abs() > 1e-12 {
                let t = s0 / (s0 - s1);
                let p = a.point + (b.point - a.point) * t;
                if !pts.iter().any(|q| (*q - p).norm() < 1e-7) {
                    pts.push(p);
                }
            }
        }
        if pts.len() < 3 {
            return pts;
        }
        // Order around the centroid in the cutting plane.
        let c = pts.iter().fold(Vec3::ZERO, |s, &p| s + p) * (1.0 / pts.len() as f64);
        let seed = if n.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let u = (seed - n * seed.dot(n)).try_normalize().unwrap_or(seed);
        let w = n.cross(u);
        pts.sort_by(|p, q| {
            let ap = (*p - c).dot(w).atan2((*p - c).dot(u));
            let aq = (*q - c).dot(w).atan2((*q - c).dot(u));
            ap.partial_cmp(&aq).unwrap_or(std::cmp::Ordering::Equal)
        });
        pts
    }

    /// Planar slices at a list of offsets along `normal` from `base`
    /// (parity item 77, additive-manufacturing slicing): one section
    /// polygon per offset. Empty slices (offset misses the body) are kept
    /// as empty vectors so the result aligns with `offsets`.
    pub fn planar_slices(&self, base: Vec3, normal: Vec3, offsets: &[f64]) -> Vec<Vec<Vec3>> {
        let n = normal.try_normalize().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        offsets
            .iter()
            .map(|&o| self.section_by_plane(base + n * o, n))
            .collect()
    }

    /// Coarse geometric/topological equivalence (parity item 108): equal
    /// entity counts, genus, and (within `tol`) bounding box and volume.
    /// This is the cheap CAx-IF validation-property comparison stage
    /// (research file 22), position-sensitive; an exact B-rep equality
    /// oracle is a later refinement.
    pub fn approx_equals(&self, other: &Body, tol: f64) -> bool {
        let (ca, cb) = (self.counts(), other.counts());
        if ca.v != cb.v
            || ca.e != cb.e
            || ca.f != cb.f
            || ca.regions != cb.regions
            || ca.genus != cb.genus
        {
            return false;
        }
        let (ba, bb) = (self.bounding_box(), other.bounding_box());
        if (ba.min - bb.min).norm() > tol || (ba.max - bb.max).norm() > tol {
            return false;
        }
        let (va, vb) = (self.tessellated_volume(), other.tessellated_volume());
        (va - vb).abs() <= tol * (1.0 + va.abs())
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
    fn surface_area_of_block_is_exact() {
        // 2x3x4 block: area = 2(2*3 + 3*4 + 2*4) = 2(6+12+8) = 52.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        let a = b.surface_area();
        assert!((a - 52.0).abs() < 1e-9, "block surface area {a} != 52");
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

    #[test]
    fn clash_detection() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        // Overlapping -> clash.
        assert!(a.clashes(&z_sphere(Vec3::new(1.0, 0.0, 0.0), 1.0)));
        // Separated -> no clash.
        assert!(!a.clashes(&z_sphere(Vec3::new(5.0, 0.0, 0.0), 1.0)));
        // Fully nested (small inside big, no surface contact) -> clash.
        let big = z_sphere(Vec3::ZERO, 2.0);
        assert!(big.clashes(&z_sphere(Vec3::ZERO, 0.5)));
    }

    #[test]
    fn section_of_block_is_a_square() {
        // Section a 2x2x2 block at z=1: a 2x2 square (4 points, area 4).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let poly = b.section_by_plane(Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(
            poly.len(),
            4,
            "square section has 4 corners, got {}",
            poly.len()
        );
        // Shoelace area in the z=1 plane.
        let mut area = 0.0;
        for i in 0..poly.len() {
            let p = poly[i];
            let q = poly[(i + 1) % poly.len()];
            area += p.x * q.y - q.x * p.y;
        }
        assert!(
            (area.abs() * 0.5 - 4.0).abs() < 1e-9,
            "section area {} != 4",
            area.abs() * 0.5
        );
    }

    #[test]
    fn planar_slices_of_block() {
        // Slice a 2x2x2 block at z = 0.5, 1.0, 1.5: each a 2x2 square.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let slices = b.planar_slices(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), &[0.5, 1.0, 1.5]);
        assert_eq!(slices.len(), 3);
        for s in &slices {
            assert_eq!(s.len(), 4, "each interior slice is a square");
        }
        // A slice above the block is empty.
        let empty = b.planar_slices(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), &[5.0]);
        assert!(empty[0].is_empty(), "slice above the block is empty");
    }

    #[test]
    fn body_equivalence() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        assert!(a.approx_equals(&a.clone(), 1e-6), "body equals its clone");
        // Different radius -> not equivalent (volume + box differ).
        assert!(!a.approx_equals(&z_sphere(Vec3::ZERO, 2.0), 1e-6));
        // Two identically-built blocks are equivalent.
        let mut p = Body::new();
        p.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        let mut q = Body::new();
        q.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        assert!(p.approx_equals(&q, 1e-6), "identical blocks equivalent");
    }
}
