//! Surface-surface intersection engine (M5a). Three-tier strategy
//! (nurbs/04 section 10): exact analytic-analytic; lower-dimensional
//! analytic x spline via bivariate Bernstein composition + certified
//! 2D tracing; general spline x spline via Gauss-map separability and
//! Krawczyk-verified marching.
//!
//! Solution dimensionality is classified up front (Patrikalakis-
//! Maekawa): Empty / Points / Curves / Coincident. Coincident overlap
//! regions are detected and returned, never mis-traced as transversal.
//!
//! This file: Task 1 (classification + exact analytic tier). Tiers 2
//! and 3 land in tasks 4-5 within this module.

use crate::GeomError;
use crate::curve::{Circle3, Curve3, Ellipse3, Line3};
use crate::nurbs_surface::NurbsSurface;
use crate::surface::Surface3;
use keel_math::vec::Vec3;

/// One isolated touch point.
#[derive(Clone, Debug)]
pub struct SsiPoint {
    pub point: Vec3,
    pub uv_a: (f64, f64),
    pub uv_b: (f64, f64),
}

/// One intersection branch.
#[derive(Clone, Debug)]
pub struct SsiCurve {
    pub curve: Curve3,
    pub closed: bool,
    pub tangential: bool,
    pub tol_achieved: f64,
}

#[derive(Clone, Debug)]
pub enum SsiResult {
    Empty,
    Points(Vec<SsiPoint>),
    Curves(Vec<SsiCurve>),
    /// Surfaces overlap on a region; the coincidence handler (M5b/M6)
    /// owns it.
    Coincident,
}

/// Surface operand (mirrors intersect::SurfaceRef).
pub enum SurfaceRef<'a> {
    Analytic(&'a Surface3),
    Nurbs(&'a NurbsSurface),
}

/// Default coincidence tolerance scaling.
const COINCIDENCE_ANG: f64 = 1e-9;

/// Intersect two surfaces.
pub fn intersect_surfaces(
    a: &SurfaceRef<'_>,
    b: &SurfaceRef<'_>,
    tol: f64,
) -> Result<SsiResult, GeomError> {
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    match (a, b) {
        (SurfaceRef::Analytic(sa), SurfaceRef::Analytic(sb)) => analytic_analytic(sa, sb, tol),
        // Tiers 2 and 3 arrive in tasks 4-5.
        _ => Err(GeomError::Degenerate),
    }
}

// =====================================================================
// Tier 1: exact analytic-analytic

fn analytic_analytic(a: &Surface3, b: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    use Surface3::*;
    match (a, b) {
        (Plane(_), Plane(_)) => plane_plane(a, b, tol),
        (Plane(_), Sphere(_)) => plane_sphere(a, b, tol),
        (Sphere(_), Plane(_)) => plane_sphere(b, a, tol),
        (Sphere(_), Sphere(_)) => sphere_sphere(a, b, tol),
        (Plane(_), Cylinder(_)) => plane_cylinder(a, b, tol),
        (Cylinder(_), Plane(_)) => plane_cylinder(b, a, tol),
        // Other analytic pairs route to tier 2 (one side implicitized)
        // in Task 4; until then, unsupported.
        _ => Err(GeomError::Degenerate),
    }
}

fn plane_of(s: &Surface3) -> (&crate::surface::Frame3,) {
    match s {
        Surface3::Plane(p) => (&p.frame,),
        _ => unreachable!("plane_of on non-plane"),
    }
}

fn plane_plane(a: &Surface3, b: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (fa,) = plane_of(a);
    let (fb,) = plane_of(b);
    let dir = fa.z.cross(fb.z);
    let dn = dir.norm();
    if dn <= COINCIDENCE_ANG {
        // Parallel planes: coincident if the offset between them is
        // within tolerance, else empty.
        let gap = (fb.origin - fa.origin).dot(fa.z).abs();
        return Ok(if gap <= tol {
            SsiResult::Coincident
        } else {
            SsiResult::Empty
        });
    }
    let dir = dir * (1.0 / dn);
    // A point on the intersection line: solve the 2x2 system of the
    // two plane equations in the plane spanned by the two normals.
    let na = fa.z;
    let nb = fb.z;
    let da = na.dot(fa.origin);
    let db = nb.dot(fb.origin);
    // p = alpha na + beta nb (component in the na,nb plane).
    let naa = na.dot(na);
    let nab = na.dot(nb);
    let nbb = nb.dot(nb);
    let det = naa * nbb - nab * nab;
    if det.abs() <= 1e-300 {
        return Err(GeomError::Degenerate);
    }
    let alpha = (da * nbb - db * nab) / det;
    let beta = (db * naa - da * nab) / det;
    let origin = na * alpha + nb * beta;
    let line = Line3::new(origin, dir)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Line(line),
        closed: false,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

fn plane_sphere(plane: &Surface3, sphere: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (f,) = plane_of(plane);
    let Surface3::Sphere(s) = sphere else {
        unreachable!()
    };
    let signed = (s.frame.origin - f.origin).dot(f.z);
    let d = signed.abs();
    if d > s.radius + tol {
        return Ok(SsiResult::Empty);
    }
    let center = s.frame.origin - f.z * signed;
    if (d - s.radius).abs() <= tol {
        // Tangent: single touch point.
        return Ok(SsiResult::Points(vec![SsiPoint {
            point: center,
            uv_a: plane_uv(f, center),
            uv_b: sphere_uv(s, center),
        }]));
    }
    let r = (s.radius * s.radius - signed * signed).max(0.0).sqrt();
    let circle = Circle3::new(center, f.x, f.y, r)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Circle(circle),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

fn sphere_sphere(a: &Surface3, b: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (Surface3::Sphere(sa), Surface3::Sphere(sb)) = (a, b) else {
        unreachable!()
    };
    let axis = sb.frame.origin - sa.frame.origin;
    let d = axis.norm();
    if d <= COINCIDENCE_ANG {
        return Ok(if (sa.radius - sb.radius).abs() <= tol {
            SsiResult::Coincident
        } else {
            SsiResult::Empty
        });
    }
    if d > sa.radius + sb.radius + tol || d < (sa.radius - sb.radius).abs() - tol {
        return Ok(SsiResult::Empty);
    }
    let u = axis * (1.0 / d);
    // Distance from sa center to the radical plane.
    let x = (d * d + sa.radius * sa.radius - sb.radius * sb.radius) / (2.0 * d);
    let center = sa.frame.origin + u * x;
    let r2 = sa.radius * sa.radius - x * x;
    if r2.abs() <= tol * tol || r2 <= 0.0 {
        return Ok(SsiResult::Points(vec![SsiPoint {
            point: center,
            uv_a: sphere_uv(sa, center),
            uv_b: sphere_uv(sb, center),
        }]));
    }
    let frame = crate::surface::Frame3::from_z(center, u)?;
    let circle = Circle3::new(center, frame.x, frame.y, r2.sqrt())?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Circle(circle),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

fn plane_cylinder(plane: &Surface3, cyl: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (f,) = plane_of(plane);
    let Surface3::Cylinder(c) = cyl else {
        unreachable!()
    };
    let axis = c.frame.z;
    let cos = axis.dot(f.z).abs();
    if cos <= COINCIDENCE_ANG {
        // Plane parallel to the axis: 0/1/2 ruling lines.
        let dist = (c.frame.origin - f.origin).dot(f.z).abs();
        if dist > c.radius + tol {
            return Ok(SsiResult::Empty);
        }
        // Lines parallel to the axis at the chord offsets.
        let in_plane_perp =
            f.z.cross(axis)
                .try_normalize()
                .ok_or(GeomError::Degenerate)?;
        // Foot of the axis on the plane.
        let foot = c.frame.origin - f.z * (c.frame.origin - f.origin).dot(f.z);
        let half = (c.radius * c.radius - dist * dist).max(0.0).sqrt();
        if half <= tol {
            let line = Line3::new(foot, axis)?;
            return Ok(SsiResult::Curves(vec![SsiCurve {
                curve: Curve3::Line(line),
                closed: false,
                tangential: true,
                tol_achieved: 0.0,
            }]));
        }
        let mut curves = Vec::new();
        for s in [half, -half] {
            let line = Line3::new(foot + in_plane_perp * s, axis)?;
            curves.push(SsiCurve {
                curve: Curve3::Line(line),
                closed: false,
                tangential: false,
                tol_achieved: 0.0,
            });
        }
        return Ok(SsiResult::Curves(curves));
    }
    // General slice: an ellipse. Semi-minor = radius (perpendicular to
    // the tilt), semi-major = radius / cos(tilt). Center = axis meets
    // the plane.
    let denom = axis.dot(f.z);
    let t = (f.origin - c.frame.origin).dot(f.z) / denom;
    let center = c.frame.origin + axis * t;
    // Minor direction: in-plane and perpendicular to the axis
    // projection; major direction: in-plane along the axis tilt.
    let minor_dir =
        f.z.cross(axis)
            .try_normalize()
            .ok_or(GeomError::Degenerate)?;
    let major_dir =
        f.z.cross(minor_dir)
            .try_normalize()
            .ok_or(GeomError::Degenerate)?;
    let a_semi = c.radius / cos;
    let b_semi = c.radius;
    let ellipse = Ellipse3::new(center, major_dir, minor_dir, a_semi, b_semi)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Ellipse(ellipse),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

// ---- pcurve helpers --------------------------------------------------

fn plane_uv(f: &crate::surface::Frame3, p: Vec3) -> (f64, f64) {
    let w = p - f.origin;
    (w.dot(f.x), w.dot(f.y))
}

fn sphere_uv(s: &crate::surface::Sphere3, p: Vec3) -> (f64, f64) {
    let w = (p - s.frame.origin) * (1.0 / s.radius.max(1e-300));
    let (dx, dy, dz) = (w.dot(s.frame.x), w.dot(s.frame.y), w.dot(s.frame.z));
    let u = if dx == 0.0 && dy == 0.0 {
        0.0
    } else {
        dy.atan2(dx).rem_euclid(core::f64::consts::TAU)
    };
    (u, dz.clamp(-1.0, 1.0).asin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::{Cylinder3, Frame3, Plane3, Sphere3};

    const TOL: f64 = 1e-9;

    fn plane_at(origin: Vec3, normal: Vec3) -> Surface3 {
        Surface3::Plane(Plane3::new(Frame3::from_z(origin, normal).unwrap()))
    }

    fn on_implicit(s: &Surface3, p: Vec3) -> f64 {
        s.implicit(p)
    }

    fn check_curve_on_both(a: &Surface3, b: &Surface3, c: &Curve3, n: usize) {
        let sample = |t: f64| match c {
            Curve3::Line(l) => l.point(t),
            Curve3::Circle(ci) => ci.point(core::f64::consts::TAU * t),
            Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
            Curve3::Nurbs(nb) => {
                let (a, b) = nb.domain();
                nb.point(a + t * (b - a))
            }
        };
        for k in 0..=n {
            let p = sample(k as f64 / n as f64);
            let scale = 1.0 + p.norm() * p.norm();
            assert!(
                on_implicit(a, p).abs() < 1e-9 * scale,
                "off A: {}",
                on_implicit(a, p)
            );
            assert!(
                on_implicit(b, p).abs() < 1e-9 * scale,
                "off B: {}",
                on_implicit(b, p)
            );
        }
    }

    #[test]
    fn plane_plane_line() {
        let a = plane_at(Vec3::ZERO, Vec3::new(0., 0., 1.));
        let b = plane_at(Vec3::ZERO, Vec3::new(0., 1., 0.));
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), TOL).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 1);
        check_curve_on_both(&a, &b, &cs[0].curve, 10);
        // Parallel distinct: empty; coincident: coincident.
        let c = plane_at(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&c), TOL).unwrap(),
            SsiResult::Empty
        ));
        let d = plane_at(Vec3::ZERO, Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&d), TOL).unwrap(),
            SsiResult::Coincident
        ));
    }

    #[test]
    fn plane_sphere_circle_tangent_miss() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let sph = Surface3::Sphere(Sphere3::new(f, 2.0).unwrap());
        // Cutting plane z = 1: circle of radius sqrt(3).
        let p = plane_at(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.));
        let r = intersect_surfaces(&SurfaceRef::Analytic(&p), &SurfaceRef::Analytic(&sph), TOL)
            .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        check_curve_on_both(&p, &sph, &cs[0].curve, 16);
        // Tangent plane z = 2.
        let pt = plane_at(Vec3::new(0., 0., 2.), Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&pt), &SurfaceRef::Analytic(&sph), TOL)
                .unwrap(),
            SsiResult::Points(_)
        ));
        // Miss z = 3.
        let pm = plane_at(Vec3::new(0., 0., 3.), Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&pm), &SurfaceRef::Analytic(&sph), TOL)
                .unwrap(),
            SsiResult::Empty
        ));
    }

    #[test]
    fn sphere_sphere_circle() {
        let mk = |cx: f64, r: f64| {
            Surface3::Sphere(
                Sphere3::new(
                    Frame3::from_z(Vec3::new(cx, 0., 0.), Vec3::new(0., 0., 1.)).unwrap(),
                    r,
                )
                .unwrap(),
            )
        };
        let a = mk(0., 2.);
        let b = mk(3., 2.);
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), TOL).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        check_curve_on_both(&a, &b, &cs[0].curve, 16);
        // Disjoint.
        assert!(matches!(
            intersect_surfaces(
                &SurfaceRef::Analytic(&mk(0., 1.)),
                &SurfaceRef::Analytic(&mk(5., 1.)),
                TOL
            )
            .unwrap(),
            SsiResult::Empty
        ));
        // Concentric equal: coincident.
        assert!(matches!(
            intersect_surfaces(
                &SurfaceRef::Analytic(&mk(0., 2.)),
                &SurfaceRef::Analytic(&mk(0., 2.)),
                TOL
            )
            .unwrap(),
            SsiResult::Coincident
        ));
    }

    #[test]
    fn plane_cylinder_ellipse_and_lines() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let cyl = Surface3::Cylinder(Cylinder3::new(f, 1.5).unwrap());
        // Tilted plane: ellipse.
        let tilt = plane_at(Vec3::ZERO, Vec3::new(0., 1., 1.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&tilt),
            &SurfaceRef::Analytic(&cyl),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert!(matches!(cs[0].curve, Curve3::Ellipse(_)));
        check_curve_on_both(&tilt, &cyl, &cs[0].curve, 24);
        // Axis-parallel plane cutting two rulings (x = 1, inside r=1.5).
        let para = plane_at(Vec3::new(1., 0., 0.), Vec3::new(1., 0., 0.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&para),
            &SurfaceRef::Analytic(&cyl),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 2);
        for c in &cs {
            check_curve_on_both(&para, &cyl, &c.curve, 6);
        }
        // Tangent plane x = 1.5.
        let tan = plane_at(Vec3::new(1.5, 0., 0.), Vec3::new(1., 0., 0.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&tan),
            &SurfaceRef::Analytic(&cyl),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 1);
        assert!(cs[0].tangential);
    }
}
