//! Rigid body transform (rotation + translation): moving a body in space.
//! The same isometry carries every vertex point and every analytic
//! surface/curve frame; radii and angles are invariant because the motion
//! is rigid, and pcurves (parameter space) are untouched. Reflections
//! (mirror) flip handedness and need a sense flip -- a follow-up; so is
//! transforming NURBS-carried geometry (control-point image).

use crate::Body;
use crate::body::TopoError;
use crate::entity::SurfaceGeom;
use keel_geom::curve::{Circle3, Curve3, Ellipse3, Line3};
use keel_geom::surface::{Cone3, Cylinder3, Frame3, Plane3, Sphere3, Surface3, Torus3};
use keel_math::transform::Transform3;
use keel_math::vec::Vec3;
use std::collections::HashSet;

impl Body {
    /// Apply a RIGID motion (rotation + translation) to a copy of the
    /// body. Errors if `t` is not a proper isometry -- a reflection would
    /// flip orientation (mirror is a follow-up) and a scale/shear would
    /// change radii -- or if the body carries NURBS geometry.
    pub fn transformed(&self, t: &Transform3) -> Result<Body, TopoError> {
        // The linear part must be a proper rotation: basis images stay
        // orthonormal (rigid, so radii are preserved) and right-handed
        // (no reflection).
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
                "transform: rigid (rotation+translation) only -- scale/shear unsupported",
            ));
        }
        if xx.cross(yy).dot(zz) < 0.0 {
            return Err(TopoError::Precondition(
                "transform: reflection (mirror) is a follow-up",
            ));
        }

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
            let ns = match s {
                SurfaceGeom::Analytic(a) => SurfaceGeom::Analytic(match a {
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
                }),
                SurfaceGeom::Nurbs(_) => {
                    return Err(TopoError::Precondition(
                        "transform: NURBS surfaces are a follow-up",
                    ));
                }
            };
            if let Some(slot) = out.surfaces.get_mut(k) {
                *slot = ns;
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
        use keel_math::mat::Mat3;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        // Uniform scale is not rigid.
        assert!(
            b.transformed(&Transform3::from_uniform_scale(2.0)).is_err(),
            "scale should reject"
        );
        // Reflection across x = 0 (det -1) is mirror -- a follow-up.
        let reflect = Transform3 {
            linear: Mat3::from_cols(
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            translation: Vec3::ZERO,
        };
        assert!(b.transformed(&reflect).is_err(), "reflection should reject");
    }
}
