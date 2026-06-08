//! Offset surfaces (parity Phase 0b): the surface displaced by a signed
//! distance `d` along its outward normal. The shared geometric primitive
//! under shelling/hollowing, body offset, offset-face, and the blend
//! engine's spring-curve construction. For the analytic family the
//! offset is EXACT and closed-form (a plane stays a plane, a quadric
//! stays the same quadric with shifted parameters); the NURBS offset
//! (sample-displace-refit-certify, Piegl-Tiller) is a follow-up.
//!
//! Sign convention: `d > 0` displaces along the OUTWARD normal (away
//! from a solid's interior) -- radius-increasing for sphere/cylinder/
//! cone/torus, `frame.z`-ward for a plane.

use crate::surface::{Cone3, Cylinder3, Plane3, Sphere3, Surface3, Torus3};

/// The exact offset of an analytic surface by signed distance `d`, or
/// `None` if the offset is degenerate (a radius collapses to <= 0, or a
/// torus tube reaches its major radius and self-intersects).
pub fn offset_surface(s: &Surface3, d: f64) -> Option<Surface3> {
    if !d.is_finite() {
        return None;
    }
    match s {
        Surface3::Plane(p) => {
            // A parallel plane shifted by d along the normal (frame.z).
            let mut frame = p.frame.clone();
            frame.origin = frame.origin + frame.z * d;
            Some(Surface3::Plane(Plane3::new(frame)))
        }
        Surface3::Sphere(sph) => {
            let r = sph.radius + d;
            (r > 0.0).then(|| {
                Surface3::Sphere(Sphere3 {
                    frame: sph.frame.clone(),
                    radius: r,
                })
            })
        }
        Surface3::Cylinder(c) => {
            let r = c.radius + d;
            (r > 0.0).then(|| {
                Surface3::Cylinder(Cylinder3 {
                    frame: c.frame.clone(),
                    radius: r,
                })
            })
        }
        Surface3::Cone(c) => {
            // The normal is tilted by the half-angle from radial, so the
            // radius at v=0 grows by d/cos(half_angle); axis and angle
            // are unchanged (derived: the offset of a cone is a coaxial
            // cone of the same half-angle).
            let r = c.radius + d / c.half_angle.cos();
            (r > 0.0).then(|| {
                Surface3::Cone(Cone3 {
                    frame: c.frame.clone(),
                    radius: r,
                    half_angle: c.half_angle,
                })
            })
        }
        Surface3::Torus(t) => {
            // Offsetting along the outward normal grows the tube (minor)
            // radius; the major radius and axis are unchanged.
            let minor = t.minor + d;
            (minor > 0.0 && minor < t.major).then(|| {
                Surface3::Torus(Torus3 {
                    frame: t.frame.clone(),
                    major: t.major,
                    minor,
                })
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Frame3;
    use keel_math::vec::Vec3;

    fn x_frame(origin: Vec3) -> Frame3 {
        Frame3 {
            origin,
            x: Vec3::new(1., 0., 0.),
            y: Vec3::new(0., 1., 0.),
            z: Vec3::new(0., 0., 1.),
        }
    }

    /// Verify the defining property: the offset surface passes through
    /// `point + d * outward_normal`, and `point` itself sits at distance
    /// |d| from the offset.
    fn check(s: &Surface3, d: f64, point: Vec3, normal: Vec3) {
        let off = offset_surface(s, d).expect("offset exists");
        let on_off = point + normal * d;
        let dev = off.project(on_off).unwrap().distance;
        assert!(dev < 1e-9, "offset surface misses point+d*n by {dev}");
        let back = off.project(point).unwrap().distance;
        assert!(
            (back - d.abs()).abs() < 1e-9,
            "point-to-offset {back} != |d| {}",
            d.abs()
        );
    }

    #[test]
    fn plane_offset() {
        let s = Surface3::Plane(Plane3::new(x_frame(Vec3::new(1., 2., 3.))));
        check(&s, 0.7, Vec3::new(1., 2., 3.), Vec3::new(0., 0., 1.));
        check(&s, -0.4, Vec3::new(1., 2., 3.), Vec3::new(0., 0., 1.));
    }

    #[test]
    fn sphere_offset() {
        let s = Surface3::Sphere(Sphere3 {
            frame: x_frame(Vec3::new(0.5, -1., 2.)),
            radius: 2.0,
        });
        let c = Vec3::new(0.5, -1., 2.);
        check(&s, 0.5, c + Vec3::new(2., 0., 0.), Vec3::new(1., 0., 0.));
        check(&s, -0.5, c + Vec3::new(0., 2., 0.), Vec3::new(0., 1., 0.));
    }

    #[test]
    fn cylinder_offset() {
        let s = Surface3::Cylinder(Cylinder3 {
            frame: x_frame(Vec3::ZERO),
            radius: 1.5,
        });
        // Point on the cylinder at radius 1.5 along +x, normal radial +x.
        check(&s, 0.6, Vec3::new(1.5, 0., 1.0), Vec3::new(1., 0., 0.));
    }

    #[test]
    fn cone_offset() {
        let alpha = 0.4_f64;
        let r0 = 1.0;
        let s = Surface3::Cone(Cone3 {
            frame: x_frame(Vec3::ZERO),
            radius: r0,
            half_angle: alpha,
        });
        // Point at v=0 (axial origin), radius r0 along +x; outward normal
        // cos(a)*radial - sin(a)*axis.
        let p = Vec3::new(r0, 0., 0.);
        let n = Vec3::new(alpha.cos(), 0., -alpha.sin());
        check(&s, 0.3, p, n);
    }

    #[test]
    fn torus_offset() {
        let s = Surface3::Torus(Torus3 {
            frame: x_frame(Vec3::ZERO),
            major: 3.0,
            minor: 0.8,
        });
        // Outer equator point at radius major+minor along +x, normal +x.
        check(&s, 0.4, Vec3::new(3.8, 0., 0.), Vec3::new(1., 0., 0.));
    }

    #[test]
    fn degenerate_offset_declined() {
        let s = Surface3::Sphere(Sphere3 {
            frame: x_frame(Vec3::ZERO),
            radius: 1.0,
        });
        assert!(
            offset_surface(&s, -1.0).is_none(),
            "radius -> 0 must decline"
        );
        assert!(
            offset_surface(&s, -2.0).is_none(),
            "radius < 0 must decline"
        );
    }
}
