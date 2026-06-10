//! Analytic -> NURBS conversion for export (parity item 137; dossier 25
//! sec 22 "NURBS conversion: convert any face/edge geometry to NURBS").
//! The FORWARD direction of M8's recover (which goes NURBS -> analytic):
//! every analytic surface here has an EXACT rational form (quadrics and
//! tori by revolving an exact profile, NURBS Book ch. 8 via
//! `revolve_full`; planes bilinear), and every analytic curve an exact
//! degree-1/rational-quadratic form, so conversion is exact to machine
//! precision rather than a tolerance question.

use crate::GeomError;
use crate::curve::{Circle3, Curve3, Ellipse3};
use crate::nurbs_curve::NurbsCurve;
use crate::nurbs_surface::{NurbsSurface, revolve_full};
use crate::surface::{Cone3, Cylinder3, Plane3, Sphere3, Torus3};
use keel_math::vec::{Vec3, Vec4};

/// Convert an analytic curve to NURBS over the parameter range
/// `[t0, t1]` (line: arc length; circle/ellipse: RADIANS of sweep
/// starting at the x_axis). A NURBS input passes through unchanged.
/// The converted curve covers the same locus; its own parameterization
/// is the standard NURBS one for that form.
pub fn curve_to_nurbs(c: &Curve3, t0: f64, t1: f64) -> Result<NurbsCurve, GeomError> {
    if !(t0.is_finite() && t1.is_finite()) || t0 >= t1 {
        return Err(GeomError::OutOfDomain);
    }
    match c {
        Curve3::Line(l) => NurbsCurve::new(
            1,
            vec![0.0, 0.0, 1.0, 1.0],
            vec![l.origin + l.dir * t0, l.origin + l.dir * t1],
            None,
        ),
        Curve3::Circle(ci) => circle_arc_nurbs(ci, t0, t1),
        Curve3::Ellipse(e) => ellipse_arc_nurbs(e, t0, t1),
        Curve3::Nurbs(n) => Ok(n.clone()),
    }
}

/// Exact rational arc of `ci` from angle t0 to t1 (<= full circle).
fn circle_arc_nurbs(ci: &Circle3, t0: f64, t1: f64) -> Result<NurbsCurve, GeomError> {
    let sweep = (t1 - t0).min(core::f64::consts::TAU);
    // Rotate the start onto the arc's start angle.
    let (c0, s0) = (t0.cos(), t0.sin());
    let x = ci.x_axis * c0 + ci.y_axis * s0;
    let y = ci.y_axis * c0 - ci.x_axis * s0;
    NurbsCurve::circular_arc(ci.center, x, y, ci.radius, sweep)
}

/// Exact rational ellipse arc: the unit-circle rational arc mapped by
/// the (a, b) axis scaling (an affine map of a rational NURBS is the
/// rational NURBS of the mapped locus, weights unchanged).
fn ellipse_arc_nurbs(e: &Ellipse3, t0: f64, t1: f64) -> Result<NurbsCurve, GeomError> {
    let sweep = (t1 - t0).min(core::f64::consts::TAU);
    let unit = NurbsCurve::circular_arc(
        Vec3::ZERO,
        Vec3::new(t0.cos(), t0.sin(), 0.0),
        Vec3::new(-t0.sin(), t0.cos(), 0.0),
        1.0,
        sweep,
    )?;
    let ctrl: Vec<Vec4> = unit
        .homogeneous_control()
        .iter()
        .map(|h| {
            let w = h.w;
            let p = e.center + e.x_axis * (e.a * (h.x / w)) + e.y_axis * (e.b * (h.y / w));
            Vec4::new(p.x * w, p.y * w, p.z * w, w)
        })
        .collect();
    NurbsCurve::from_homogeneous(unit.knot_vector().clone(), ctrl)
}

/// Bilinear NURBS patch of a plane over the frame-coordinate rectangle
/// `[u0,u1] x [v0,v1]`. Exact.
pub fn plane_to_nurbs(
    p: &Plane3,
    (u0, u1): (f64, f64),
    (v0, v1): (f64, f64),
) -> Result<NurbsSurface, GeomError> {
    if !(u0.is_finite() && u1.is_finite() && v0.is_finite() && v1.is_finite())
        || u0 >= u1
        || v0 >= v1
    {
        return Err(GeomError::OutOfDomain);
    }
    let at = |u: f64, v: f64| p.frame.origin + p.frame.x * u + p.frame.y * v;
    let k = vec![0.0, 0.0, 1.0, 1.0];
    NurbsSurface::new(
        1,
        1,
        k.clone(),
        k,
        vec![at(u0, v0), at(u0, v1), at(u1, v0), at(u1, v1)],
        None,
    )
}

/// Full-circumference exact NURBS cylinder band between heights h0 < h1
/// (frame-z heights).
pub fn cylinder_to_nurbs(c: &Cylinder3, h0: f64, h1: f64) -> Result<NurbsSurface, GeomError> {
    if !(h0.is_finite() && h1.is_finite()) || h0 >= h1 {
        return Err(GeomError::OutOfDomain);
    }
    let f = &c.frame;
    let profile = NurbsCurve::new(
        1,
        vec![0.0, 0.0, 1.0, 1.0],
        vec![
            f.origin + f.x * c.radius + f.z * h0,
            f.origin + f.x * c.radius + f.z * h1,
        ],
        None,
    )?;
    revolve_full(&profile, f.origin, f.z)
}

/// Full-circumference exact NURBS cone band between heights h0 < h1
/// (radius grows by tan(half_angle) per unit height; must stay >= 0).
pub fn cone_to_nurbs(c: &Cone3, h0: f64, h1: f64) -> Result<NurbsSurface, GeomError> {
    if !(h0.is_finite() && h1.is_finite()) || h0 >= h1 {
        return Err(GeomError::OutOfDomain);
    }
    let f = &c.frame;
    let r_at = |h: f64| c.radius + h * c.half_angle.tan();
    if r_at(h0) < 0.0 || r_at(h1) < 0.0 {
        return Err(GeomError::OutOfDomain);
    }
    let profile = NurbsCurve::new(
        1,
        vec![0.0, 0.0, 1.0, 1.0],
        vec![
            f.origin + f.x * r_at(h0) + f.z * h0,
            f.origin + f.x * r_at(h1) + f.z * h1,
        ],
        None,
    )?;
    revolve_full(&profile, f.origin, f.z)
}

/// Exact full NURBS sphere (revolved rational semicircle).
pub fn sphere_to_nurbs(s: &Sphere3) -> Result<NurbsSurface, GeomError> {
    let f = &s.frame;
    let profile =
        NurbsCurve::circular_arc(f.origin, f.z * -1.0, f.x, s.radius, core::f64::consts::PI)?;
    revolve_full(&profile, f.origin, f.z)
}

/// Exact full NURBS torus (revolved rational minor circle).
pub fn torus_to_nurbs(t: &Torus3) -> Result<NurbsSurface, GeomError> {
    let f = &t.frame;
    let profile = NurbsCurve::full_circle(f.origin + f.x * t.major, f.x, f.z, t.minor)?;
    revolve_full(&profile, f.origin, f.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Frame3;

    fn zf() -> Frame3 {
        Frame3::from_z(Vec3::new(0.5, -1.0, 2.0), Vec3::new(0.0, 0.0, 1.0)).unwrap()
    }

    #[test]
    fn line_and_circle_convert_exactly() {
        let l = crate::curve::Line3::new(Vec3::ZERO, Vec3::new(0., 1., 0.)).unwrap();
        let n = curve_to_nurbs(&Curve3::Line(l), 1.0, 4.0).unwrap();
        let (a, b) = n.domain();
        assert!((n.point(a) - Vec3::new(0., 1., 0.)).norm() < 1e-15);
        assert!((n.point(b) - Vec3::new(0., 4., 0.)).norm() < 1e-15);

        let ci = Circle3::new(
            Vec3::new(1., 2., 3.),
            Vec3::new(1., 0., 0.),
            Vec3::new(0., 1., 0.),
            2.0,
        )
        .unwrap();
        let n = curve_to_nurbs(&Curve3::Circle(ci), 0.0, core::f64::consts::TAU).unwrap();
        let (a, b) = n.domain();
        for i in 0..=16 {
            let p = n.point(a + (b - a) * i as f64 / 16.0);
            let r = ((p.x - 1.0).powi(2) + (p.y - 2.0).powi(2)).sqrt();
            assert!((r - 2.0).abs() < 1e-12 && (p.z - 3.0).abs() < 1e-12);
        }
    }

    #[test]
    fn ellipse_converts_exactly() {
        let e = Ellipse3 {
            center: Vec3::new(0., 0., 1.),
            x_axis: Vec3::new(1., 0., 0.),
            y_axis: Vec3::new(0., 1., 0.),
            a: 3.0,
            b: 1.5,
        };
        let n = curve_to_nurbs(&Curve3::Ellipse(e), 0.0, core::f64::consts::TAU).unwrap();
        let (a, b) = n.domain();
        for i in 0..=24 {
            let p = n.point(a + (b - a) * i as f64 / 24.0);
            let res = (p.x / 3.0).powi(2) + (p.y / 1.5).powi(2);
            assert!((res - 1.0).abs() < 1e-12, "ellipse residual {res}");
        }
    }

    #[test]
    fn surfaces_convert_exactly() {
        // Cylinder band: radius residual ~0 over the grid.
        let cyl = Cylinder3 {
            frame: zf(),
            radius: 1.5,
        };
        let n = cylinder_to_nurbs(&cyl, 0.0, 3.0).unwrap();
        let ((u0, u1), (v0, v1)) = n.domain();
        for i in 0..=8 {
            for j in 0..=8 {
                let p = n.point(
                    u0 + (u1 - u0) * i as f64 / 8.0,
                    v0 + (v1 - v0) * j as f64 / 8.0,
                );
                let d = p - cyl.frame.origin;
                let r = (d.dot(cyl.frame.x).powi(2) + d.dot(cyl.frame.y).powi(2)).sqrt();
                assert!((r - 1.5).abs() < 1e-12, "cylinder residual {r}");
            }
        }
        // Cone band: radius matches r(h) at each height.
        let cone = Cone3 {
            frame: zf(),
            radius: 1.0,
            half_angle: 0.4636476090008061, // tan = 0.5
        };
        let n = cone_to_nurbs(&cone, 0.0, 2.0).unwrap();
        for j in 0..=8 {
            let p = n.point(0.21, j as f64 / 8.0);
            let d = p - cone.frame.origin;
            let h = d.dot(cone.frame.z);
            let r = (d.dot(cone.frame.x).powi(2) + d.dot(cone.frame.y).powi(2)).sqrt();
            assert!((r - (1.0 + 0.5 * h)).abs() < 1e-12, "cone residual");
        }
        // Sphere and torus: implicit residuals ~0.
        let sp = Sphere3 {
            frame: zf(),
            radius: 2.0,
        };
        let n = sphere_to_nurbs(&sp).unwrap();
        let ((u0, u1), (v0, v1)) = n.domain();
        for i in 0..=8 {
            for j in 0..=8 {
                let p = n.point(
                    u0 + (u1 - u0) * i as f64 / 8.0,
                    v0 + (v1 - v0) * j as f64 / 8.0,
                );
                assert!(((p - sp.frame.origin).norm() - 2.0).abs() < 1e-12);
            }
        }
        let to = Torus3 {
            frame: zf(),
            major: 3.0,
            minor: 1.0,
        };
        let n = torus_to_nurbs(&to).unwrap();
        for i in 0..=8 {
            for j in 0..=8 {
                let p = n.point(i as f64 / 8.0, j as f64 / 8.0);
                let d = p - to.frame.origin;
                let ring = (d.dot(to.frame.x).powi(2) + d.dot(to.frame.y).powi(2)).sqrt() - 3.0;
                let res = ring * ring + d.dot(to.frame.z).powi(2) - 1.0;
                assert!(res.abs() < 1e-11, "torus residual {res}");
            }
        }
        // Plane patch corners.
        let pl = Plane3::new(zf());
        let n = plane_to_nurbs(&pl, (-1.0, 2.0), (0.0, 4.0)).unwrap();
        let c = n.point(0.0, 0.0);
        assert!((c - (pl.frame.origin + pl.frame.x * -1.0)).norm() < 1e-14);
    }
}
