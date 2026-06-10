//! Foreign geometry at the body level (parity items 114-116;
//! research/kernel/25 sec 17, research/kernel/08 sec 3.4 + rec 9).
//!
//! A host-implemented evaluator (`keel_geom::foreign`) is consumed by a
//! constructor that SAMPLES + FITS it to a certified NURBS face, stored
//! as a standard `SurfaceGeom::Nurbs`. The evaluator is the procedural
//! truth, the NURBS is the cache (the kernel's founding doctrine), so:
//! no new geometry variant, serde-safe (items 126/127 untouched), and
//! every existing NURBS path (tessellation, recover/simplify, booleans
//! after recovery) works unchanged. When the certified fit misses the
//! requested tolerance the constructor DECLINES (honesty gate) instead
//! of building a wrong body.
//!
//! Coverage: 114 (foreign surfaces and curves via evaluator callbacks),
//! 115 (the foreign-derived body is first-class: it validates,
//! tessellates, simplifies, and after native conversion feeds thicken /
//! booleans), 116 (foreign -> native: the fit converts the evaluator to
//! NURBS, `Body::simplify` recovers hidden analytics from it).

use crate::body::{Body, TopoError};
use crate::entity::{LoopKind, Side, SurfaceGeom};
use crate::lineage::Derivation;
use keel_geom::curve::Curve3;
use keel_geom::foreign::{ForeignCurve, ForeignSurface, fit_foreign_curve, fit_foreign_surface};

fn geom_err(_: keel_geom::GeomError) -> TopoError {
    TopoError::Precondition("foreign: geometry construction failed")
}

impl Body {
    /// Construct a SHEET body (one open, double-sided face) from a
    /// foreign surface evaluator, fitting it to a certified NURBS
    /// within `tol`. The face's four free edges carry the EXACT
    /// boundary iso-curves of the fitted patch, so face and edge
    /// geometry are consistent by construction. Errs (DECLINES) when
    /// the certified fit cannot meet `tol`, or when the patch boundary
    /// is degenerate (collapsed corners, e.g. poles -- MVP excludes
    /// degenerate-boundary patches).
    pub fn foreign_sheet(surf: &dyn ForeignSurface, tol: f64) -> Result<Body, TopoError> {
        let fit = fit_foreign_surface(surf, tol).map_err(geom_err)?;
        if fit.tol_achieved > tol {
            return Err(TopoError::Precondition(
                "foreign_sheet: evaluator does not fit to NURBS within tol (declined)",
            ));
        }
        let s = fit.surface;
        let ((u0, u1), (v0, v1)) = s.domain();
        // Ring corners (clamped surface: corners are exact net points).
        let corners = [
            s.point(u0, v0),
            s.point(u1, v0),
            s.point(u1, v1),
            s.point(u0, v1),
        ];
        for i in 0..4 {
            if (corners[i] - corners[(i + 1) % 4]).norm() <= 1e-9 {
                return Err(TopoError::Precondition(
                    "foreign_sheet: degenerate patch boundary (MVP)",
                ));
            }
        }
        // [u=u0 (param v), u=u1 (param v), v=v0 (param u), v=v1 (param u)]
        let [c_u0, c_u1, c_v0, c_v1] = s.boundary_curves();
        // Ring edge i runs corners[i] -> corners[i+1]; curve sense says
        // whether the iso-curve's parameter runs the same way.
        let ring = [(c_v0, true), (c_u1, true), (c_v1, false), (c_u0, false)];

        // Topology: identical shape to `planar_sheet` (one double-sided
        // face over the ambient void, four free edges).
        let mut b = Body::new();
        let void = b.infinite_region();
        let mut rec = b.begin_op();
        let face = b.new_face(&mut rec, void, void, Derivation::Created);
        let lp = b.new_loop(&mut rec, face, LoopKind::Outer, Derivation::Created);
        if let Some(f) = b.faces.get_mut(face) {
            f.loops.push(lp);
        }
        let verts: Vec<_> = corners.iter().map(|&p| b.new_vertex(&mut rec, p)).collect();
        let mut edges = Vec::with_capacity(4);
        let mut fins = Vec::with_capacity(4);
        for i in 0..4 {
            let e = b.new_edge(
                &mut rec,
                (verts[i], verts[(i + 1) % 4]),
                Derivation::Created,
            );
            let fin = b.new_fin(&mut rec, e, true, lp, Derivation::Created);
            if let Some(ed) = b.edges.get_mut(e) {
                ed.radial.push(fin); // free edge: a single coedge
            }
            edges.push(e);
            fins.push(fin);
        }
        for i in 0..4 {
            let nxt = fins[(i + 1) % 4];
            let prv = fins[(i + 3) % 4];
            if let Some(f) = b.fins.get_mut(fins[i]) {
                f.next = nxt;
                f.prev = prv;
            }
        }
        if let Some(l) = b.loops.get_mut(lp) {
            l.fin = Some(fins[0]);
        }
        for i in 0..4 {
            if let Some(v) = b.vertices.get_mut(verts[i]) {
                v.fin = Some(fins[i]);
            }
        }
        let shell = b.new_shell(&mut rec, void, Derivation::Created);
        if let Some(sh) = b.shells.get_mut(shell) {
            sh.faces.push((face, Side::Front));
            sh.faces.push((face, Side::Back));
        }
        if let Some(r) = b.regions.get_mut(void) {
            r.shells.push(shell);
        }
        let _ = rec.finish();

        b.attach_face_surface(face, SurfaceGeom::Nurbs(s), true);
        for (e, (curve, sense)) in edges.iter().zip(ring) {
            b.attach_edge_curve(*e, Curve3::Nurbs(curve), sense);
        }
        Ok(b)
    }

    /// Attach a foreign curve evaluator to an existing edge, fitting it
    /// to a certified NURBS within `tol` (the curve half of item 114).
    /// The fitted curve's endpoints must coincide with the edge's
    /// vertices within `tol` (either orientation; the sense is set
    /// accordingly). Errs (DECLINES) when the fit misses `tol` or the
    /// endpoints do not land on the edge's vertices. Returns the
    /// certified `tol_achieved`.
    pub fn attach_foreign_edge_curve(
        &mut self,
        edge: crate::entity::EdgeKey,
        curve: &dyn ForeignCurve,
        tol: f64,
    ) -> Result<f64, TopoError> {
        let fit = fit_foreign_curve(curve, tol).map_err(geom_err)?;
        if fit.tol_achieved > tol {
            return Err(TopoError::Precondition(
                "attach_foreign_edge_curve: evaluator does not fit within tol (declined)",
            ));
        }
        let (vk0, vk1) = self.edges.get(edge).ok_or(TopoError::StaleKey)?.bounds;
        let p0 = self.vertices.get(vk0).ok_or(TopoError::StaleKey)?.point;
        let p1 = self.vertices.get(vk1).ok_or(TopoError::StaleKey)?.point;
        let (a, b) = {
            let (t0, t1) = (0.0, 1.0); // lsq_fit domain is [0,1]
            (fit.curve.point(t0), fit.curve.point(t1))
        };
        let sense = if (a - p0).norm() <= tol && (b - p1).norm() <= tol {
            true
        } else if (a - p1).norm() <= tol && (b - p0).norm() <= tol {
            false
        } else {
            return Err(TopoError::Precondition(
                "attach_foreign_edge_curve: fitted endpoints miss the edge vertices",
            ));
        };
        self.attach_edge_curve(edge, Curve3::Nurbs(fit.curve), sense);
        Ok(fit.tol_achieved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boolean::{BoolOp, boolean};
    use keel_geom::surface::Surface3;
    use keel_math::vec::Vec3;

    /// z = 0.3 sin(x) cos(y) over [0,2]^2.
    struct Wavy;
    impl ForeignSurface for Wavy {
        fn domain(&self) -> ((f64, f64), (f64, f64)) {
            ((0.0, 2.0), (0.0, 2.0))
        }
        fn eval(&self, u: f64, v: f64) -> Vec3 {
            Vec3::new(u, v, 0.3 * u.sin() * v.cos())
        }
    }

    /// The z=0 plane patch [0,2]^2 under a non-affine parameterization
    /// (the fit must reproduce the parameterization, the geometry is
    /// still the plane).
    struct ParamPlane;
    impl ForeignSurface for ParamPlane {
        fn domain(&self) -> ((f64, f64), (f64, f64)) {
            ((0.0, 1.0), (0.0, 1.0))
        }
        fn eval(&self, u: f64, v: f64) -> Vec3 {
            Vec3::new(2.0 * (u + 0.3 * u * u) / 1.3, 2.0 * v, 0.0)
        }
    }

    /// Quarter cylinder, radius 2 about z, height 3.
    struct QuarterCyl;
    impl ForeignSurface for QuarterCyl {
        fn domain(&self) -> ((f64, f64), (f64, f64)) {
            ((0.0, core::f64::consts::FRAC_PI_2), (0.0, 1.0))
        }
        fn eval(&self, u: f64, v: f64) -> Vec3 {
            Vec3::new(2.0 * u.cos(), 2.0 * u.sin(), 3.0 * v)
        }
    }

    #[test]
    fn foreign_sheet_is_a_valid_nurbs_sheet() {
        // Item 114: evaluator in, certified NURBS sheet out. One
        // double-sided NURBS face, four free edges, validates, and
        // tessellates (participation in meshing, item 115).
        let b = Body::foreign_sheet(&Wavy, 1e-4).unwrap();
        assert!(
            b.validate().is_ok(),
            "foreign sheet invalid: {:?}",
            b.validate()
        );
        let faces = b.face_keys();
        assert_eq!(faces.len(), 1, "one face");
        assert!(
            matches!(b.face_surface_geom(faces[0]), Some(SurfaceGeom::Nurbs(_))),
            "face must carry the NURBS cache"
        );
        assert_eq!(b.edges.iter().count(), 4, "four free boundary edges");
        let tris = b.tessellate_face(faces[0]);
        assert!(!tris.is_empty(), "foreign NURBS face must tessellate");
        // Every tessellation vertex lies near the evaluator's image
        // (cache faithfulness, spot-checked through the corners).
        let c = Wavy.eval(0.0, 0.0);
        assert!(
            tris.iter().flatten().any(|p| (*p - c).norm() < 1e-9),
            "tessellation must reach the patch corner"
        );
    }

    #[test]
    fn foreign_sheet_declines_unfittable_evaluator() {
        // Honesty gate: a C0 crease cannot certify at 1e-9; the
        // constructor must DECLINE, never emit a wrong body.
        struct Crease;
        impl ForeignSurface for Crease {
            fn domain(&self) -> ((f64, f64), (f64, f64)) {
                ((0.0, 1.0), (0.0, 1.0))
            }
            fn eval(&self, u: f64, v: f64) -> Vec3 {
                Vec3::new(u, v, (u - 0.5).abs())
            }
        }
        assert!(Body::foreign_sheet(&Crease, 1e-9).is_err());
    }

    #[test]
    fn foreign_plane_converts_to_native_and_models() {
        // Items 115 + 116 end-to-end: foreign evaluator -> NURBS cache
        // (fit) -> native analytic plane (simplify) -> thicken -> boolean
        // difference, mass == mesh against the analytic answer.
        let mut sheet = Body::foreign_sheet(&ParamPlane, 1e-6).unwrap();
        let rep = sheet.simplify(1e-6);
        assert_eq!(rep.surfaces_recovered, 1, "plane must recover (116)");
        let face = sheet.face_keys()[0];
        assert!(
            matches!(
                sheet.face_surface_geom(face),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ),
            "face must be a native plane after simplify"
        );
        assert!(sheet.validate().is_ok(), "simplified sheet invalid");

        // Participation (115): the recovered sheet feeds the sheet ops
        // and the boolean pipeline.
        let slab = sheet.thicken(0.5).unwrap();
        let v = slab.mass_properties().unwrap().volume;
        let mv = slab.mesh_volume();
        assert!(
            (v - 2.0).abs() < 1e-9 && (mv - 2.0).abs() < 1e-9,
            "slab must be 2x2x0.5 = 2.0 with mass == mesh (got mass {v}, mesh {mv})"
        );
        // Corner-overlap difference (a proven transversal config; the
        // interior through-notch has its own regression in boolean.rs).
        // Removes the 0.5 x 0.5 x 0.5 part of the protruding cutter
        // that lies inside the slab.
        let mut drill = Body::new();
        drill
            .block(Vec3::new(1.5, 1.5, -0.5), 1.0, 1.0, 1.0)
            .unwrap();
        let res = boolean(&slab, &drill, BoolOp::Difference, 1e-7).unwrap();
        assert!(
            res.body.validate().is_ok(),
            "notched slab invalid: {:?}",
            res.faults
        );
        let vol = res.body.mass_properties().unwrap().volume;
        let mvol = res.body.mesh_volume();
        let want = 2.0 - 0.5 * 0.5 * 0.5;
        assert!(
            (vol - want).abs() < 1e-9 && (mvol - want).abs() < 1e-9,
            "notched slab volume must be {want} with mass == mesh (got mass {vol}, mesh {mvol})"
        );
    }

    #[test]
    fn foreign_cylinder_recovers_native_cylinder() {
        // Item 116, curved case: a foreign quarter-cylinder evaluator
        // converts to the native analytic cylinder via the certified
        // recover gate.
        let mut sheet = Body::foreign_sheet(&QuarterCyl, 1e-5).unwrap();
        let rep = sheet.simplify(1e-4);
        assert_eq!(rep.surfaces_recovered, 1, "cylinder must recover");
        let face = sheet.face_keys()[0];
        assert!(
            matches!(
                sheet.face_surface_geom(face),
                Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
            ),
            "face must be a native cylinder after simplify (got {:?})",
            sheet.face_surface_geom(face)
        );
        assert!(sheet.validate().is_ok(), "recovered cylinder sheet invalid");
    }

    #[test]
    fn foreign_edge_curve_attaches_with_sense() {
        // The curve half of 114: replace a planar sheet's straight edge
        // with a foreign evaluator of the same segment under a
        // non-affine parameterization; it must attach (endpoints hit the
        // edge vertices exactly: end-pinned fit), validate, and recover
        // to a native line via simplify.
        struct SegEval;
        impl ForeignCurve for SegEval {
            fn domain(&self) -> (f64, f64) {
                (0.0, 1.0)
            }
            fn eval(&self, t: f64) -> Vec3 {
                Vec3::new(2.0 * (t + 0.3 * t * t) / 1.3, 0.0, 0.0)
            }
        }
        let mut b = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ])
        .unwrap();
        // The edge from (0,0,0) to (2,0,0).
        let edge = b
            .edges
            .iter()
            .find(|(_, e)| {
                let p0 = b.vertices.get(e.bounds.0).unwrap().point;
                let p1 = b.vertices.get(e.bounds.1).unwrap().point;
                (p0 - Vec3::ZERO).norm() < 1e-12 && (p1 - Vec3::new(2., 0., 0.)).norm() < 1e-12
            })
            .map(|(k, _)| k)
            .unwrap();
        let achieved = b.attach_foreign_edge_curve(edge, &SegEval, 1e-6).unwrap();
        assert!(achieved <= 1e-6, "certified {achieved}");
        assert!(b.validate().is_ok(), "sheet with foreign edge invalid");
        let rep = b.simplify(1e-6);
        assert!(
            rep.curves_recovered >= 1,
            "foreign edge must recover to a line"
        );
    }
}
