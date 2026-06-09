//! Rigid body transform (rotation + translation) and mirror (reflection).
//! The same isometry carries every vertex point and every analytic
//! surface/curve frame; radii and angles are invariant because the motion
//! is rigid, and pcurves (parameter space) are untouched.
//!
//! Reflection (mirror) needs NO special orientation handling: reflecting
//! every frame directly sends frame.z -> M(frame.z), so the outward normal
//! (mass_properties uses frame.z x region-solidity; tessellation uses
//! frame.z x sense) maps to M(N); and because M is orthogonal the face's
//! (u, v) coordinates are preserved, so pcurves stay valid. The frame goes
//! left-handed, but nothing relies on its chirality. Transforming NURBS
//! geometry is the one follow-up.

use crate::Body;
use crate::body::TopoError;
use crate::entity::SurfaceGeom;
use keel_geom::curve::{Circle3, Curve3, Ellipse3, Line3};
use keel_geom::surface::{Cone3, Cylinder3, Frame3, Plane3, Sphere3, Surface3, Torus3};
use keel_math::mat::Mat3;
use keel_math::transform::Transform3;
use keel_math::vec::Vec3;
use std::collections::HashSet;

impl Body {
    /// Apply a RIGID motion (rotation + translation) to a copy of the
    /// body. Errors if `t` is not a proper rotation -- a reflection (use
    /// [`Body::mirrored`]) or a scale/shear -- or if the body carries
    /// NURBS geometry.
    pub fn transformed(&self, t: &Transform3) -> Result<Body, TopoError> {
        match Self::isometry_kind(t)? {
            IsometryKind::Rotation => self.apply_isometry(t),
            IsometryKind::Reflection => Err(TopoError::Precondition(
                "transform: reflection -- use Body::mirrored",
            )),
        }
    }

    /// Mirror a copy of the body across the plane through `plane_point`
    /// with unit-able normal `plane_normal`. Works for analytic faces
    /// (planar and curved); NURBS-carried geometry is a follow-up.
    pub fn mirrored(&self, plane_point: Vec3, plane_normal: Vec3) -> Result<Body, TopoError> {
        let m = plane_normal
            .try_normalize()
            .ok_or(TopoError::Precondition("mirror: degenerate plane normal"))?;
        // Householder reflection R(x) = (I - 2 m m^T) x + 2 (plane_point . m) m.
        let linear = Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0) - m * (2.0 * m.x),
            Vec3::new(0.0, 1.0, 0.0) - m * (2.0 * m.y),
            Vec3::new(0.0, 0.0, 1.0) - m * (2.0 * m.z),
        );
        let t = Transform3 {
            linear,
            translation: m * (2.0 * plane_point.dot(m)),
        };
        self.apply_isometry(&t)
    }

    /// Uniformly scale a copy of the body about `center` by `factor` > 0.
    /// Planar-faced bodies only: mass_properties and tessellation sample the
    /// scaled geometry directly, so the stored pcurves (left as-is) do not
    /// affect them. Curved surfaces (whose radii would scale) and circular/
    /// elliptic/NURBS edges are a follow-up.
    pub fn scaled(&self, center: Vec3, factor: f64) -> Result<Body, TopoError> {
        if !(factor.is_finite() && factor > 0.0) {
            return Err(TopoError::Precondition(
                "scale: factor must be finite and > 0",
            ));
        }
        let mut out = self.clone();
        let vkeys: Vec<_> = out.vertices.iter().map(|(k, _)| k).collect();
        for k in vkeys {
            if let Some(v) = out.vertices.get_mut(k) {
                v.point = center + (v.point - center) * factor;
            }
        }
        let skeys: HashSet<_> = out
            .faces
            .iter()
            .filter_map(|(_, f)| f.surface.map(|(sk, _)| sk))
            .collect();
        for k in skeys {
            let Some(SurfaceGeom::Analytic(Surface3::Plane(p))) = out.surfaces.get(k).cloned()
            else {
                return Err(TopoError::Precondition(
                    "scale: curved/NURBS surfaces are a follow-up",
                ));
            };
            let frame = Frame3 {
                origin: center + (p.frame.origin - center) * factor,
                x: p.frame.x,
                y: p.frame.y,
                z: p.frame.z,
            };
            if let Some(slot) = out.surfaces.get_mut(k) {
                *slot = SurfaceGeom::Analytic(Surface3::Plane(Plane3 { frame }));
            }
        }
        let ckeys: HashSet<_> = out
            .edges
            .iter()
            .filter_map(|(_, e)| e.curve.map(|(ck, _)| ck))
            .collect();
        for k in ckeys {
            let Some(Curve3::Line(l)) = out.curves.get(k).cloned() else {
                return Err(TopoError::Precondition(
                    "scale: curved edges are a follow-up",
                ));
            };
            let nl = Line3 {
                origin: center + (l.origin - center) * factor,
                dir: l.dir,
            };
            if let Some(slot) = out.curves.get_mut(k) {
                *slot = Curve3::Line(nl);
            }
        }
        Ok(out)
    }

    /// Classify a transform's linear part. Errors on scale/shear (non-
    /// orthonormal), where radii would not be preserved.
    fn isometry_kind(t: &Transform3) -> Result<IsometryKind, TopoError> {
        let xx = t.apply_vector(Vec3::new(1.0, 0.0, 0.0));
        let yy = t.apply_vector(Vec3::new(0.0, 1.0, 0.0));
        let zz = t.apply_vector(Vec3::new(0.0, 0.0, 1.0));
        let orthonormal = (xx.norm() - 1.0).abs() < 1e-9
            && (yy.norm() - 1.0).abs() < 1e-9
            && (zz.norm() - 1.0).abs() < 1e-9
            && xx.dot(yy).abs() < 1e-9
            && xx.dot(zz).abs() < 1e-9
            && yy.dot(zz).abs() < 1e-9;
        if !orthonormal {
            return Err(TopoError::Precondition(
                "transform: rigid (rotation/reflection) only -- scale/shear unsupported",
            ));
        }
        Ok(if xx.cross(yy).dot(zz) >= 0.0 {
            IsometryKind::Rotation
        } else {
            IsometryKind::Reflection
        })
    }

    /// Shared isometry application: carry every vertex point and every
    /// referenced analytic surface/curve frame by `t`. Works identically
    /// for rotations and reflections (see the module note).
    fn apply_isometry(&self, t: &Transform3) -> Result<Body, TopoError> {
        let mut out = self.clone();
        let fr = |f: &Frame3| Frame3 {
            origin: t.apply_point(f.origin),
            x: t.apply_vector(f.x),
            y: t.apply_vector(f.y),
            z: t.apply_vector(f.z),
        };

        let vkeys: Vec<_> = out.vertices.iter().map(|(k, _)| k).collect();
        for k in vkeys {
            if let Some(v) = out.vertices.get_mut(k) {
                v.point = t.apply_point(v.point);
            }
        }

        // Only geometry REFERENCED by the topology -- surfaces by faces,
        // curves by edges. Pcurves (fin parameter-space curves, in the same
        // arena) and orphan geometry are deliberately left alone.
        let skeys: HashSet<_> = out
            .faces
            .iter()
            .filter_map(|(_, f)| f.surface.map(|(sk, _)| sk))
            .collect();
        for k in skeys {
            let Some(s) = out.surfaces.get(k).cloned() else {
                continue;
            };
            let SurfaceGeom::Analytic(a) = s else {
                return Err(TopoError::Precondition(
                    "transform: NURBS surfaces are a follow-up",
                ));
            };
            let na = match a {
                Surface3::Plane(p) => Surface3::Plane(Plane3 {
                    frame: fr(&p.frame),
                }),
                Surface3::Cylinder(c) => Surface3::Cylinder(Cylinder3 {
                    frame: fr(&c.frame),
                    radius: c.radius,
                }),
                Surface3::Cone(c) => Surface3::Cone(Cone3 {
                    frame: fr(&c.frame),
                    radius: c.radius,
                    half_angle: c.half_angle,
                }),
                Surface3::Sphere(c) => Surface3::Sphere(Sphere3 {
                    frame: fr(&c.frame),
                    radius: c.radius,
                }),
                Surface3::Torus(c) => Surface3::Torus(Torus3 {
                    frame: fr(&c.frame),
                    major: c.major,
                    minor: c.minor,
                }),
            };
            if let Some(slot) = out.surfaces.get_mut(k) {
                *slot = SurfaceGeom::Analytic(na);
            }
        }

        let ckeys: HashSet<_> = out
            .edges
            .iter()
            .filter_map(|(_, e)| e.curve.map(|(ck, _)| ck))
            .collect();
        for k in ckeys {
            let Some(c) = out.curves.get(k).cloned() else {
                continue;
            };
            let nc = match c {
                Curve3::Line(l) => Curve3::Line(Line3 {
                    origin: t.apply_point(l.origin),
                    dir: t.apply_vector(l.dir),
                }),
                Curve3::Circle(ci) => Curve3::Circle(Circle3 {
                    center: t.apply_point(ci.center),
                    x_axis: t.apply_vector(ci.x_axis),
                    y_axis: t.apply_vector(ci.y_axis),
                    radius: ci.radius,
                }),
                Curve3::Ellipse(e) => Curve3::Ellipse(Ellipse3 {
                    center: t.apply_point(e.center),
                    x_axis: t.apply_vector(e.x_axis),
                    y_axis: t.apply_vector(e.y_axis),
                    a: e.a,
                    b: e.b,
                }),
                Curve3::Nurbs(_) => {
                    return Err(TopoError::Precondition(
                        "transform: NURBS curves are a follow-up",
                    ));
                }
            };
            if let Some(slot) = out.curves.get_mut(k) {
                *slot = nc;
            }
        }
        Ok(out)
    }
}

enum IsometryKind {
    Rotation,
    Reflection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_box_rigid_preserves_volume() {
        // Rotate a 2^3 block a quarter turn about z, then translate +5x.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let t = Transform3::from_rotation(Vec3::new(0.0, 0.0, 1.0), core::f64::consts::FRAC_PI_2)
            .unwrap()
            .then(Transform3::from_translation(Vec3::new(5.0, 0.0, 0.0)));
        let m = b.transformed(&t).unwrap();
        assert!(m.validate().is_ok(), "transformed box invalid");
        let v = m.mass_properties().unwrap().volume;
        assert!(
            (v - 8.0).abs() < 1e-9,
            "rigid transform changed volume: {v}"
        );
        // [0,2]^3 -> (x,y,z)->(-y,x,z) -> +5x: x in [3,5], y in [0,2], z in [0,2].
        let bb = m.bounding_box();
        assert!(
            (bb.min - Vec3::new(3.0, 0.0, 0.0)).norm() < 1e-9,
            "bbox min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(5.0, 2.0, 2.0)).norm() < 1e-9,
            "bbox max {:?}",
            bb.max
        );
    }

    #[test]
    fn transform_rejects_scale_and_reflection() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        assert!(
            b.transformed(&Transform3::from_uniform_scale(2.0)).is_err(),
            "scale should reject"
        );
        let reflect = Transform3 {
            linear: Mat3::from_cols(
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            translation: Vec3::ZERO,
        };
        assert!(
            b.transformed(&reflect).is_err(),
            "transformed must reject a reflection (mirror has its own entry)"
        );
    }

    #[test]
    fn mirror_box_across_plane() {
        // Reflect a 2^3 block across x = 0: [0,2]^3 -> [-2,0]x[0,2]x[0,2],
        // volume invariant, a valid (re-oriented) solid.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let m = b.mirrored(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)).unwrap();
        assert!(m.validate().is_ok(), "mirrored box invalid");
        let v = m.mass_properties().unwrap().volume;
        assert!((v - 8.0).abs() < 1e-9, "mirror changed volume: {v}");
        let bb = m.bounding_box();
        assert!(
            (bb.min - Vec3::new(-2.0, 0.0, 0.0)).norm() < 1e-9,
            "bbox min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(0.0, 2.0, 2.0)).norm() < 1e-9,
            "bbox max {:?}",
            bb.max
        );
    }

    #[test]
    fn mirror_curved_body() {
        // An offset cylinder (axis through x = 2, radius 1, height 2)
        // mirrored across x = 0 -> axis through x = -2; direct frame
        // reflection handles the curved face with no special casing.
        let frame = Frame3::from_z(Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.cylinder(frame, 1.0, 2.0).unwrap();
        let m = b.mirrored(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)).unwrap();
        assert!(m.validate().is_ok(), "mirrored cylinder invalid");
        let v = m.mesh_volume();
        let expect = 2.0 * core::f64::consts::PI; // pi r^2 h = pi*1*2
        assert!(
            (v - expect).abs() < expect * 0.02,
            "mirrored cylinder volume {v} != ~{expect}"
        );
        let bb = m.bounding_box();
        assert!(
            (bb.min - Vec3::new(-3.0, -1.0, 0.0)).norm() < 1e-9
                && (bb.max - Vec3::new(-1.0, 1.0, 2.0)).norm() < 1e-9,
            "mirrored cylinder bbox [{:?},{:?}]",
            bb.min,
            bb.max
        );
    }

    #[test]
    fn scale_box_cubes_the_volume() {
        // [0,2]^3 (volume 8) scaled 2x about the origin -> [0,4]^3 (volume
        // 64 = 8 * 2^3), a valid solid.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let s = b.scaled(Vec3::ZERO, 2.0).unwrap();
        assert!(s.validate().is_ok(), "scaled box invalid");
        let v = s.mass_properties().unwrap().volume;
        assert!((v - 64.0).abs() < 1e-9, "scale^3 volume {v} != 64");
        let bb = s.bounding_box();
        assert!(
            (bb.min - Vec3::ZERO).norm() < 1e-9
                && (bb.max - Vec3::new(4.0, 4.0, 4.0)).norm() < 1e-9,
            "scaled bbox [{:?},{:?}]",
            bb.min,
            bb.max
        );
    }

    #[test]
    fn scale_rejects_curved_and_bad_factor() {
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut cyl = Body::new();
        cyl.cylinder(frame, 1.0, 2.0).unwrap();
        assert!(
            cyl.scaled(Vec3::ZERO, 2.0).is_err(),
            "curved body scale is a follow-up"
        );
        let mut bx = Body::new();
        bx.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        assert!(bx.scaled(Vec3::ZERO, 0.0).is_err(), "zero factor rejects");
    }
}
