//! Edge blend / fillet surface generation (parity items 47-61), the
//! geometry stage of research file 40. This module implements the
//! centerpiece procedure for the highest-value rung of the exact-analytic
//! ladder: a constant-radius rolling-ball fillet on a CONVEX edge between
//! two PLANAR faces, whose spine is a line and whose blend surface is an
//! exact native cylinder.
//!
//! File 40 §1.2 (the single most important procedural fact):
//!   spine = SSI( offset(S_a, r), offset(S_b, r) ).
//! For two planes, each offset is a parallel plane and their intersection
//! is a straight line parallel to the sharp edge; the envelope of equal
//! spheres of radius r centred on that line is a cylinder of radius r
//! about it (§1.4 Case A). The two spring curves are the feet of the
//! perpendiculars from the spine onto each plane (§1.3) -- the spine
//! translated by +r*n -- and the cylinder is tangent to each support
//! plane along its spring line (G1 by construction).
//!
//! This file delivers and certifies that geometry. The trim-and-stitch
//! local operation that inserts the blend face into the body (file 40
//! §3) is the next stage; circle-spine tori, cyclides, and NURBS canals
//! (the higher rungs) are follow-ups.

use crate::Body;
use crate::body::TopoError;
use crate::entity::{EdgeKey, SurfaceGeom};
use keel_geom::curve::Line3;
use keel_geom::surface::{Cylinder3, Frame3, Surface3};
use keel_math::vec::Vec3;

/// The analytic data of a constant-radius rolling-ball fillet on a
/// plane-plane edge: the spine line (ball-centre locus), the two spring
/// lines (tangency loci on each support plane), and the exact cylinder
/// blend surface (axis = spine, radius = r).
#[derive(Clone, Debug)]
pub struct EdgeBlend {
    /// Locus of the rolling-ball centre (the cylinder axis).
    pub spine: Line3,
    /// Tangency line on the first support plane.
    pub spring_a: Line3,
    /// Tangency line on the second support plane.
    pub spring_b: Line3,
    /// The exact blend surface: a cylinder of radius `r` about the spine.
    pub surface: Cylinder3,
}

impl Body {
    /// Generate the exact-cylinder blend surface for a convex `edge`
    /// between two planar faces, rolling a ball of `radius` r (file 40
    /// rung 1). Returns the spine, both spring lines, and the cylinder.
    /// Errors if the edge is not bounded by exactly two planar faces or
    /// the configuration is degenerate (parallel planes / r <= 0).
    ///
    /// This is the geometry stage only; it does not yet modify the body
    /// (the trim-and-stitch insertion is file 40 §3, a follow-up).
    pub fn blend_cylinder_for_edge(
        &self,
        edge: EdgeKey,
        radius: f64,
    ) -> Result<EdgeBlend, TopoError> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(TopoError::Precondition("blend: radius must be positive"));
        }
        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("blend: edge needs two faces"));
        }
        // Both supports must be planar (rung 1).
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition(
                    "blend: non-planar support (follow-up)",
                ));
            }
        }
        // Outward normals and a point on each support plane.
        let n1 = self
            .face_outward_normal(faces[0])
            .ok_or(TopoError::Precondition("blend: face normal"))?;
        let n2 = self
            .face_outward_normal(faces[1])
            .ok_or(TopoError::Precondition("blend: face normal"))?;
        let p1 = self
            .face_outer_loop_points(faces[0])
            .first()
            .copied()
            .ok_or(TopoError::Precondition("blend: face point"))?;
        let p2 = self
            .face_outer_loop_points(faces[1])
            .first()
            .copied()
            .ok_or(TopoError::Precondition("blend: face point"))?;

        // Offset each support plane by r toward the material (the side
        // opposite its outward normal): { x : n . x = n . p - r }. The
        // spine is the intersection line of the two offset planes.
        let d1 = n1.dot(p1) - radius;
        let d2 = n2.dot(p2) - radius;
        let dir = n1
            .cross(n2)
            .try_normalize()
            .ok_or(TopoError::Precondition("blend: parallel supports"))?;
        // Point on both offset planes nearest the origin (two-plane
        // intersection; n1, n2 unit so denom = 1 - cos^2 = sin^2).
        let c = n1.dot(n2);
        let denom = 1.0 - c * c;
        if denom.abs() < 1e-12 {
            return Err(TopoError::Precondition("blend: parallel supports"));
        }
        let spine_pt = (n1 * ((d1 - d2 * c) / denom)) + (n2 * ((d2 - d1 * c) / denom));
        let spine = Line3::new(spine_pt, dir)
            .map_err(|_| TopoError::Precondition("blend: degenerate spine"))?;

        // Spring lines: feet of perpendiculars from the spine onto each
        // support plane (the spine translated by +r toward the face).
        let foot = |p_plane: Vec3, n: Vec3| spine_pt - n * ((spine_pt - p_plane).dot(n));
        let spring_a = Line3::new(foot(p1, n1), dir)
            .map_err(|_| TopoError::Precondition("blend: degenerate spring"))?;
        let spring_b = Line3::new(foot(p2, n2), dir)
            .map_err(|_| TopoError::Precondition("blend: degenerate spring"))?;

        // Blend surface: cylinder of radius r about the spine. Its frame
        // x-axis points at the first spring line so the kept quarter is
        // unambiguous; z = spine direction.
        let fx = (foot(p1, n1) - spine_pt)
            .try_normalize()
            .ok_or(TopoError::Precondition("blend: degenerate frame"))?;
        let frame = Frame3 {
            origin: spine_pt,
            x: fx,
            y: dir.cross(fx),
            z: dir,
        };
        let surface = Cylinder3::new(frame, radius)
            .map_err(|_| TopoError::Precondition("blend: bad cylinder"))?;

        Ok(EdgeBlend {
            spine,
            spring_a,
            spring_b,
            surface,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The top-right edge of an axis-aligned block (faces with outward
    /// normals +z and +x), as in chamfer's test.
    fn top_right_edge(b: &Body) -> EdgeKey {
        let ekeys: Vec<EdgeKey> = b.edges.iter().map(|(k, _)| k).collect();
        for e in ekeys {
            let fs = b.faces_around_edge(e);
            if fs.len() != 2 {
                continue;
            }
            let (Some(a), Some(c)) = (b.face_outward_normal(fs[0]), b.face_outward_normal(fs[1]))
            else {
                continue;
            };
            if (a.z > 0.9 || c.z > 0.9) && (a.x > 0.9 || c.x > 0.9) {
                return e;
            }
        }
        panic!("no top-right edge");
    }

    #[test]
    fn plane_plane_blend_is_tangent_cylinder() {
        // Block [0,2]^3, fillet the top-right edge with r = 0.5. The
        // supports are z = 2 (n = +z) and x = 2 (n = +x); the spine is the
        // line {x = 1.5, z = 1.5} parallel to y, the cylinder has radius
        // 0.5 about it, tangent to both planes (axis 0.5 from each).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let blend = b.blend_cylinder_for_edge(e, 0.5).unwrap();

        // Spine at x = z = 1.5, direction along y.
        assert!(
            (blend.spine.origin.x - 1.5).abs() < 1e-9,
            "spine x {}",
            blend.spine.origin.x
        );
        assert!(
            (blend.spine.origin.z - 1.5).abs() < 1e-9,
            "spine z {}",
            blend.spine.origin.z
        );
        assert!(
            blend.surface.frame.z.cross(Vec3::new(0.0, 1.0, 0.0)).norm() < 1e-9,
            "spine not along y"
        );
        assert!((blend.surface.radius - 0.5).abs() < 1e-12);

        // Tangency: the axis is exactly r from each support plane.
        let axis_pt = blend.spine.origin;
        assert!((2.0 - axis_pt.z).abs() - 0.5 < 1e-9, "not tangent to z=2");
        assert!((2.0 - axis_pt.x).abs() - 0.5 < 1e-9, "not tangent to x=2");

        // Spring lines lie ON the support planes (z=2 and x=2) at the
        // tangency points (x=1.5 and z=1.5 respectively).
        assert!(
            (blend.spring_a.origin.z - 2.0).abs() < 1e-9,
            "spring_a not on z=2"
        );
        assert!((blend.spring_a.origin.x - 1.5).abs() < 1e-9);
        assert!(
            (blend.spring_b.origin.x - 2.0).abs() < 1e-9,
            "spring_b not on x=2"
        );
        assert!((blend.spring_b.origin.z - 1.5).abs() < 1e-9);

        // The spring points are exactly r from the spine (the ball touches
        // each face at distance r from its centre).
        assert!(((blend.spring_a.origin - axis_pt).norm() - 0.5).abs() < 1e-9);
        assert!(((blend.spring_b.origin - axis_pt).norm() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn blend_rejects_bad_radius() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        assert!(b.blend_cylinder_for_edge(e, 0.0).is_err());
        assert!(b.blend_cylinder_for_edge(e, -1.0).is_err());
    }
}
