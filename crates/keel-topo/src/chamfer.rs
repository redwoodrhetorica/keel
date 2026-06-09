//! Edge chamfer (parity item 52; the first blend-family operation).
//! A constant-setback chamfer on a convex edge between two PLANAR faces,
//! done as a transversal boolean: subtract a prism whose chamfer face
//! slices the corner at the setback, with its other faces lying outside
//! the body so the cut stays transversal (no coincident-face handling
//! needed). Curved-face chamfers, variable/asymmetric setback, and
//! fillets (arc cross-section) are follow-ups.

use crate::Body;
use crate::body::TopoError;
use crate::boolean::{BoolOp, boolean};
use crate::entity::{EdgeKey, SurfaceGeom};
use keel_geom::surface::Surface3;

impl Body {
    /// Chamfer `edge` with equal setback `distance` on each adjacent
    /// face. Returns the chamfered body. Errors if the edge is not
    /// bounded by exactly two planar faces, or the cut fails.
    pub fn chamfer_edge(&self, edge: EdgeKey, distance: f64) -> Result<Body, TopoError> {
        let (v0, v1) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let p0 = self.vertices.get(v0).ok_or(TopoError::StaleKey)?.point;
        let p1 = self.vertices.get(v1).ok_or(TopoError::StaleKey)?.point;
        let len = (p1 - p0).norm();
        let t = (p1 - p0)
            .try_normalize()
            .ok_or(TopoError::Precondition("chamfer: degenerate edge"))?;

        let faces = self.faces_around_edge(edge);
        if faces.len() != 2 {
            return Err(TopoError::Precondition("chamfer: edge needs two faces"));
        }
        for &f in &faces {
            if !matches!(
                self.face_surface_geom(f),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ) {
                return Err(TopoError::Precondition("chamfer: non-planar adjacent face"));
            }
        }
        let n1 = self
            .face_outward_normal(faces[0])
            .ok_or(TopoError::Precondition("chamfer: face normal"))?;
        let n2 = self
            .face_outward_normal(faces[1])
            .ok_or(TopoError::Precondition("chamfer: face normal"))?;

        // In-face directions from the edge into each face's interior
        // (perpendicular to the edge, oriented away from the other face).
        let mut u1 = t.cross(n1);
        if u1.dot(n2) > 0.0 {
            u1 = u1 * -1.0;
        }
        let mut u2 = t.cross(n2);
        if u2.dot(n1) > 0.0 {
            u2 = u2 * -1.0;
        }
        let bisect = (n1 + n2)
            .try_normalize()
            .ok_or(TopoError::Precondition("chamfer: degenerate corner"))?;

        // Cutting prism: cross-section triangle in the plane through p0
        // perpendicular to the edge -- two chamfer-line points (a, b) on
        // the faces, and an apex (c) well OUTSIDE the body along the
        // outward bisector so only the chamfer face (a-b) cuts the body.
        let bb = self.bounding_box();
        let big = (bb.max - bb.min).norm() * 2.0 + 10.0;
        let margin = len * 0.5 + distance + 1e-3;
        let base = p0 - t * margin;
        let a = base + u1 * distance;
        let b = base + u2 * distance;
        let c = base + bisect * big;
        // Order the profile counterclockwise about the extrusion direction.
        let profile = if (b - a).cross(c - a).dot(t) >= 0.0 {
            vec![a, b, c]
        } else {
            vec![a, c, b]
        };
        let dir = t * (len + 2.0 * margin);

        let mut cutter = Body::new();
        cutter
            .prism(&profile, dir)
            .map_err(|_| TopoError::Precondition("chamfer: bad cutting prism"))?;
        boolean(self, &cutter, BoolOp::Difference, 1e-7)
            .map(|r| r.body)
            .map_err(|_| TopoError::Precondition("chamfer: cut failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_math::vec::Vec3;

    /// The block edge whose two faces have outward normals +z and +x
    /// (the top-right edge of an axis-aligned block).
    fn top_right_edge(b: &Body) -> EdgeKey {
        let ekeys: Vec<EdgeKey> = b.edges.iter().map(|(k, _)| k).collect();
        for e in ekeys {
            let fs = b.faces_around_edge(e);
            if fs.len() != 2 {
                continue;
            }
            let n0 = b.face_outward_normal(fs[0]);
            let n1 = b.face_outward_normal(fs[1]);
            if let (Some(a), Some(c)) = (n0, n1) {
                let has_z = a.z > 0.9 || c.z > 0.9;
                let has_x = a.x > 0.9 || c.x > 0.9;
                if has_z && has_x {
                    return e;
                }
            }
        }
        panic!("no top-right edge found");
    }

    #[test]
    fn chamfer_box_edge_removes_wedge() {
        // Chamfer one top edge of a 2x2x2 block by 0.5: removes a
        // right-triangular-prism wedge of cross-section 0.5*0.5/2 = 0.125
        // over length 2 => 0.25. Volume 8 -> 7.75.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let chamfered = b.chamfer_edge(e, 0.5).unwrap();
        assert!(chamfered.validate().is_ok(), "chamfered body invalid");
        let v = chamfered.mass_properties().unwrap().volume;
        assert!((v - 7.75).abs() < 1e-6, "chamfer volume {v} != 7.75");
    }

    #[test]
    #[ignore = "known bug (task #16, dossier 47): build_result_solid drops the oblique \
                cut face on a non-45-degree wedge. Un-ignore when the planar path routes \
                through the identity-preserving stitch_by_import assembly."]
    fn asymmetric_chamfer_must_not_return_wrong_body() {
        // Asymmetric chamfer of a 2^3 box top edge: setbacks d1=0.5 on one
        // face, d2=1.0 on the other. The removed wedge is a right triangle
        // (legs 0.5, 1.0 -> area 0.25) over length 2 => 0.5 removed, so the
        // true volume is 8 - 0.5 = 7.5. The oblique cut face is a thin,
        // non-45-degree fragment that build_result_solid drops (it returns
        // mass 11.5 / mesh 8.83 -- both wrong). CONTRACT: the boolean must
        // return the correct 7.5 OR decline (Err) -- never a wrong-positive
        // body. This reconstructs the chamfer_edge cutter with independent
        // setbacks (d1 != d2).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let e = top_right_edge(&b);
        let (v0, v1) = b.edges.get(e).unwrap().bounds;
        let p0 = b.vertices.get(v0).unwrap().point;
        let p1 = b.vertices.get(v1).unwrap().point;
        let len = (p1 - p0).norm();
        let t = (p1 - p0).try_normalize().unwrap();
        let faces = b.faces_around_edge(e);
        let n1 = b.face_outward_normal(faces[0]).unwrap();
        let n2 = b.face_outward_normal(faces[1]).unwrap();
        let mut u1 = t.cross(n1);
        if u1.dot(n2) > 0.0 {
            u1 = u1 * -1.0;
        }
        let mut u2 = t.cross(n2);
        if u2.dot(n1) > 0.0 {
            u2 = u2 * -1.0;
        }
        let bisect = (n1 + n2).try_normalize().unwrap();
        let (d1, d2) = (0.5_f64, 1.0_f64);
        let bb = b.bounding_box();
        let big = (bb.max - bb.min).norm() * 2.0 + 10.0;
        let margin = len * 0.5 + d1.max(d2) + 1e-3;
        let base = p0 - t * margin;
        let a = base + u1 * d1;
        let bpt = base + u2 * d2;
        let c = base + bisect * big;
        let profile = if (bpt - a).cross(c - a).dot(t) >= 0.0 {
            vec![a, bpt, c]
        } else {
            vec![a, c, bpt]
        };
        let dir = t * (len + 2.0 * margin);
        let mut cutter = Body::new();
        cutter.prism(&profile, dir).unwrap();
        if let Ok(res) = boolean(&b, &cutter, BoolOp::Difference, 1e-7) {
            let v = res.body.mass_properties().unwrap().volume;
            let mv = res.body.mesh_volume();
            assert!(
                (v - 7.5).abs() < 1e-6 && (mv - 7.5).abs() < 0.05,
                "asymmetric chamfer must be 7.5 (got mass {v}, mesh {mv}) or decline"
            );
        }
    }
}
