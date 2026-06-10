//! General topology replace (parity item 138; dossier 25 sec 22
//! "Replace / general topology queries"): swap the GEOMETRY carried by
//! existing topology, with a consistency guard -- the new geometry must
//! describe the same locus as the boundary it supports, within `tol`
//! (sampled), so a replace can re-represent (e.g. analytic -> the exact
//! item-137 NURBS) or refine, but never silently relocate a face or
//! edge. The reintersecting replace-and-move form is the tweak family
//! (items 35-39); this is the representation-level replace under it.
//! Queries are the existing topology navigation (item 23).

use crate::body::{Body, TopoError};
use crate::entity::{EdgeKey, FaceKey, SurfaceGeom};
use keel_geom::curve::Curve3;
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::project::project_point_surface_fast;

impl Body {
    /// Replace a face's surface geometry, guarded: every boundary
    /// sample of the face must lie on the new surface within `tol`.
    /// The face's sense is preserved.
    pub fn replace_face_surface_checked(
        &mut self,
        face: FaceKey,
        geom: SurfaceGeom,
        tol: f64,
    ) -> Result<(), TopoError> {
        if !(tol.is_finite() && tol > 0.0) {
            return Err(TopoError::Precondition("replace: bad tol"));
        }
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .ok_or(TopoError::StaleKey)?;
        let mut pts = Vec::new();
        for lk in loops {
            pts.extend(self.loop_polygon(lk));
        }
        if pts.is_empty() {
            return Err(TopoError::Precondition("replace: face has no boundary"));
        }
        for p in &pts {
            let d = match &geom {
                SurfaceGeom::Analytic(s) => s
                    .project(*p)
                    .map(|pr| pr.distance)
                    .map_err(|_| TopoError::Precondition("replace: projection failed"))?,
                SurfaceGeom::Nurbs(n) => project_point_surface_fast(n, *p).distance,
            };
            if d > tol {
                return Err(TopoError::Precondition(
                    "replace: new surface misses the face boundary",
                ));
            }
        }
        let sense = self
            .faces
            .get(face)
            .and_then(|f| f.surface)
            .map(|(_, s)| s)
            .unwrap_or(true);
        self.attach_face_surface(face, geom, sense);
        Ok(())
    }

    /// Replace an edge's curve with a NURBS curve, guarded: the curve's
    /// endpoints must land on the edge's vertices within `tol` (either
    /// orientation; a closed edge requires a closed curve through its
    /// seam vertex). Returns the sense it was attached with.
    pub fn replace_edge_curve_checked(
        &mut self,
        edge: EdgeKey,
        curve: NurbsCurve,
        tol: f64,
    ) -> Result<bool, TopoError> {
        if !(tol.is_finite() && tol > 0.0) {
            return Err(TopoError::Precondition("replace: bad tol"));
        }
        let (vk0, vk1) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let p0 = self.vertices.get(vk0).ok_or(TopoError::StaleKey)?.point;
        let p1 = self.vertices.get(vk1).ok_or(TopoError::StaleKey)?.point;
        let (t0, t1) = curve.domain();
        let (a, b) = (curve.point(t0), curve.point(t1));
        if vk0 == vk1 {
            // Closed edge: closed curve through the seam vertex.
            if (a - b).norm() > tol || (a - p0).norm() > tol {
                return Err(TopoError::Precondition(
                    "replace: closed edge needs a closed curve through its seam vertex",
                ));
            }
            self.attach_edge_curve(edge, Curve3::Nurbs(curve), true);
            return Ok(true);
        }
        let sense = if (a - p0).norm() <= tol && (b - p1).norm() <= tol {
            true
        } else if (a - p1).norm() <= tol && (b - p0).norm() <= tol {
            false
        } else {
            return Err(TopoError::Precondition(
                "replace: curve endpoints miss the edge vertices",
            ));
        };
        self.attach_edge_curve(edge, Curve3::Nurbs(curve), sense);
        Ok(sense)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::{Frame3, Plane3, Surface3};
    use keel_math::vec::Vec3;

    #[test]
    fn replace_face_with_exact_nurbs_round_trips() {
        // Item 138 + 137 closure: a box face re-represented as its exact
        // bilinear NURBS patch stays a valid body with the same volume,
        // and simplify recovers the plane again.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let face = b.face_keys()[0];
        let n = b.face_to_nurbs(face).unwrap();
        b.replace_face_surface_checked(face, SurfaceGeom::Nurbs(n), 1e-9)
            .unwrap();
        assert!(b.validate().is_ok(), "replaced body invalid");
        let v = b.tessellated_volume();
        assert!((v - 8.0).abs() < 1e-9, "volume changed: {v}");
        let rep = b.simplify(1e-9);
        assert_eq!(rep.surfaces_recovered, 1, "plane must recover");
    }

    #[test]
    fn replace_rejects_relocated_geometry() {
        // The guard: a SHIFTED plane misses the face boundary -> Err.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let face = b.face_keys()[0];
        let shifted =
            Plane3::new(Frame3::from_z(Vec3::new(0.0, 0.0, 50.0), Vec3::new(0., 0., 1.)).unwrap());
        assert!(
            b.replace_face_surface_checked(
                face,
                SurfaceGeom::Analytic(Surface3::Plane(shifted)),
                1e-6
            )
            .is_err(),
            "a relocating replace must be refused"
        );
    }

    #[test]
    fn replace_edge_curve_sets_sense_and_guards() {
        use keel_geom::convert::curve_to_nurbs;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        // Pick any edge and re-represent its line as an exact NURBS.
        let edge = b.edges.iter().map(|(k, _)| k).next().unwrap();
        let (vk0, vk1) = b.edge(edge).unwrap().bounds;
        let (p0, p1) = (b.vertex(vk0).unwrap().point, b.vertex(vk1).unwrap().point);
        let line = keel_geom::curve::Line3::new(p0, p1 - p0).unwrap();
        let n = curve_to_nurbs(&Curve3::Line(line), 0.0, (p1 - p0).norm()).unwrap();
        let sense = b.replace_edge_curve_checked(edge, n, 1e-9).unwrap();
        assert!(sense, "forward parameterization -> forward sense");
        assert!(b.validate().is_ok());
        // A curve landing elsewhere is refused.
        let off = keel_geom::curve::Line3::new(p0 + Vec3::new(0., 0., 9.), p1 - p0).unwrap();
        let bad = curve_to_nurbs(&Curve3::Line(off), 0.0, (p1 - p0).norm()).unwrap();
        assert!(b.replace_edge_curve_checked(edge, bad, 1e-9).is_err());
    }
}
