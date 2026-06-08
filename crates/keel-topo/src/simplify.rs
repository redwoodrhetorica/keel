//! Canonical-recovery "simplify" pass (M8): the public HEAL operation
//! that recognizes analytic geometry hidden in a body's NURBS faces and
//! edges and replaces it with the exact primitive, in place, preserving
//! topology and orientation. Mirrors ACIS HEAL's simplify phase and
//! OCCT `ShapeAnalysis_CanonicalRecognition`. The recover/keep decision
//! is the certified gate in `keel_geom::recover`; here we only perform
//! the topology-preserving substitution.

use crate::Body;
use crate::entity::SurfaceGeom;
use keel_geom::curve::Curve3;
use keel_geom::recover::{recover_curve, recover_surface};

/// What `simplify` changed.
#[derive(Clone, Debug, Default)]
pub struct SimplifyReport {
    pub surfaces_recovered: usize,
    pub curves_recovered: usize,
    /// The largest certified deviation introduced by any substitution.
    pub max_deviation: f64,
}

impl Body {
    /// Recover analytic surfaces/curves hidden in NURBS geometry, swapping
    /// them in place within `tol`. Topology is untouched; face/edge
    /// orientation is preserved (the swapped surface's sense is chosen so
    /// the outward normal is unchanged). Geometry is only ever TIGHTENED
    /// (an exact primitive replaces an approximation), and every swap is
    /// certified within `tol` by the recovery gate.
    pub fn simplify(&mut self, tol: f64) -> SimplifyReport {
        let mut rep = SimplifyReport::default();

        // Faces: NURBS -> analytic, preserving the outward normal.
        for face in self.face_keys() {
            let Some(SurfaceGeom::Nurbs(n)) = self.face_surface_geom(face) else {
                continue;
            };
            let Some(rec) = recover_surface(&n, tol) else {
                continue;
            };
            let ((u0, u1), (v0, v1)) = n.domain();
            let (um, vm) = (0.5 * (u0 + u1), 0.5 * (v0 + v1));
            let Ok(lg_n) = n.local_geometry(um, vm) else {
                continue;
            };
            let p = n.point(um, vm);
            let old_sense = self
                .faces
                .get(face)
                .and_then(|f| f.surface)
                .map(|(_, s)| s)
                .unwrap_or(true);
            let old_out = if old_sense { lg_n.normal } else { -lg_n.normal };
            // Choose the new sense so the analytic's outward normal agrees
            // with the old one at the same geometric point.
            let new_sense = match rec
                .surface
                .project(p)
                .ok()
                .and_then(|pr| rec.surface.local_geometry(pr.u, pr.v).ok())
            {
                Some(lg_a) => lg_a.normal.dot(old_out) >= 0.0,
                None => old_sense,
            };
            rep.max_deviation = rep.max_deviation.max(rec.deviation);
            self.attach_face_surface(face, SurfaceGeom::Analytic(rec.surface), new_sense);
            rep.surfaces_recovered += 1;
        }

        // Edges: NURBS -> line/circle, keeping the parameter sense.
        let edge_keys: Vec<_> = self.edges.iter().map(|(k, _)| k).collect();
        for edge in edge_keys {
            let Some((ck, sense)) = self.edges.get(edge).and_then(|e| e.curve) else {
                continue;
            };
            let Some(Curve3::Nurbs(c)) = self.curves.get(ck).cloned() else {
                continue;
            };
            let Some(rec) = recover_curve(&c, tol) else {
                continue;
            };
            rep.max_deviation = rep.max_deviation.max(rec.deviation);
            self.attach_edge_curve(edge, rec.curve, sense);
            rep.curves_recovered += 1;
        }

        rep
    }
}

#[cfg(test)]
mod tests {
    use crate::Body;
    use keel_geom::surface::{Frame3, Surface3};
    use keel_math::vec::Vec3;

    fn z_frame(center: Vec3) -> Frame3 {
        Frame3 {
            origin: center,
            x: Vec3::new(0., 1., 0.),
            y: Vec3::new(0., 0., 1.),
            z: Vec3::new(1., 0., 0.),
        }
    }

    #[test]
    fn simplify_recovers_nurbs_sphere() {
        // A NURBS-faced sphere simplifies to an analytic sphere: the face
        // surface becomes Surface3::Sphere, the body stays valid, and the
        // tessellated volume is preserved (geometry only tightened).
        let mut body = Body::new();
        body.nurbs_sphere(z_frame(Vec3::new(0.2, -0.3, 0.5)), 1.3)
            .unwrap();
        let v_before = body.tessellated_volume();

        let rep = body.simplify(1e-6);
        assert_eq!(rep.surfaces_recovered, 1, "sphere face must recover");
        assert!(rep.max_deviation < 1e-6, "deviation {}", rep.max_deviation);

        // The face is now an analytic sphere.
        let face = body.face_keys()[0];
        match body.face_surface_geom(face) {
            Some(SurfaceGeom::Analytic(Surface3::Sphere(_))) => {}
            other => panic!("expected analytic sphere, got {other:?}"),
        }
        assert!(
            body.validate().is_ok(),
            "simplified body invalid: {:?}",
            body.validate()
        );
        let v_after = body.tessellated_volume();
        assert!(
            (v_after - v_before).abs() < 0.02 * v_before.abs().max(1e-9),
            "volume changed: {v_before} -> {v_after}"
        );
    }

    use crate::entity::SurfaceGeom;
}
