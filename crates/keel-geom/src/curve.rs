//! Analytic curve types and the exhaustive curve dispatch enum
//! (spec D4: analytics are first-class, never silently NURBS).

use crate::GeomError;
use crate::nurbs_curve::NurbsCurve;
use keel_math::bbox::Aabb3;
use keel_math::vec::Vec3;

/// Parameter domain of a curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Domain {
    /// Closed interval [a, b] (possibly infinite for lines).
    Finite { a: f64, b: f64 },
    /// Periodic with the given period, canonical range [0, period).
    Periodic { period: f64 },
}

/// Infinite straight line: point(t) = origin + t * dir (dir unit, so
/// t is arc length). Carried unbounded; edges bound it in topology.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line3 {
    pub origin: Vec3,
    /// Unit direction.
    pub dir: Vec3,
}

impl Line3 {
    pub fn new(origin: Vec3, dir: Vec3) -> Result<Self, GeomError> {
        let dir = dir.try_normalize().ok_or(GeomError::Degenerate)?;
        Ok(Self { origin, dir })
    }
    #[inline]
    pub fn point(&self, t: f64) -> Vec3 {
        self.origin + self.dir * t
    }
    /// Exact closest parameter to p.
    #[inline]
    pub fn project(&self, p: Vec3) -> f64 {
        (p - self.origin).dot(self.dir)
    }
}

/// Circle: point(theta) = center + r cos(theta) x + r sin(theta) y,
/// with x, y unit and orthogonal. theta in [0, 2*pi).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle3 {
    pub center: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub radius: f64,
}

impl Circle3 {
    pub fn new(
        center: Vec3,
        x_axis: Vec3,
        y_axis: Vec3,
        radius: f64,
    ) -> Result<Self, GeomError> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(GeomError::Degenerate);
        }
        let x = x_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        let y = y_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        if x.dot(y).abs() > 1e-12 {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { center, x_axis: x, y_axis: y, radius })
    }
    #[inline]
    pub fn point(&self, theta: f64) -> Vec3 {
        self.center
            + self.x_axis * (self.radius * theta.cos())
            + self.y_axis * (self.radius * theta.sin())
    }
    /// Closest parameter to p. Degenerate case (p on the circle axis):
    /// every point is equidistant; returns theta = 0 by convention.
    pub fn project(&self, p: Vec3) -> f64 {
        let d = p - self.center;
        let (cx, cy) = (d.dot(self.x_axis), d.dot(self.y_axis));
        if cx == 0.0 && cy == 0.0 {
            return 0.0;
        }
        let theta = cy.atan2(cx);
        if theta < 0.0 { theta + core::f64::consts::TAU } else { theta }
    }
}

/// Ellipse: point(theta) = center + a cos(theta) x + b sin(theta) y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse3 {
    pub center: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub a: f64,
    pub b: f64,
}

impl Ellipse3 {
    pub fn new(
        center: Vec3,
        x_axis: Vec3,
        y_axis: Vec3,
        a: f64,
        b: f64,
    ) -> Result<Self, GeomError> {
        if !(a.is_finite() && b.is_finite() && a > 0.0 && b > 0.0) {
            return Err(GeomError::Degenerate);
        }
        let x = x_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        let y = y_axis.try_normalize().ok_or(GeomError::Degenerate)?;
        if x.dot(y).abs() > 1e-12 {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { center, x_axis: x, y_axis: y, a, b })
    }
    #[inline]
    pub fn point(&self, theta: f64) -> Vec3 {
        self.center
            + self.x_axis * (self.a * theta.cos())
            + self.y_axis * (self.b * theta.sin())
    }
    #[inline]
    fn deriv(&self, theta: f64) -> Vec3 {
        self.x_axis * (-self.a * theta.sin()) + self.y_axis * (self.b * theta.cos())
    }
    /// Closest parameter to p: bracket sign changes of the
    /// orthogonality function g(theta) = (E - p) . E' over a uniform
    /// scan, polish with bracketed Newton, keep the best candidate.
    pub fn project(&self, p: Vec3) -> f64 {
        use keel_math::newton::solve_bracketed;
        const SCAN: usize = 32;
        let tau = core::f64::consts::TAU;
        let g = |th: f64| {
            let e = self.point(th) - p;
            let d1 = self.deriv(th);
            let d2 =
                self.x_axis * (-self.a * th.cos()) + self.y_axis * (-self.b * th.sin());
            (e.dot(d1), d1.dot(d1) + e.dot(d2))
        };
        let mut best = (0.0_f64, (self.point(0.0) - p).norm_sq());
        let mut prev_th = 0.0;
        let mut prev_g = g(0.0).0;
        for i in 1..=SCAN {
            let th = i as f64 * tau / SCAN as f64;
            let gv = g(th).0;
            if prev_g == 0.0 || (prev_g > 0.0) != (gv > 0.0) {
                if let Some(root) = solve_bracketed(g, prev_th, th, 1e-14, 64) {
                    let d = (self.point(root) - p).norm_sq();
                    if d < best.1 {
                        best = (root.rem_euclid(tau), d);
                    }
                }
            }
            prev_th = th;
            prev_g = gv;
        }
        best.0
    }
}

/// Exhaustive curve dispatch (compile error at every non-exhaustive
/// match when a curve type is added).
#[derive(Clone, Debug)]
pub enum Curve3 {
    Line(Line3),
    Circle(Circle3),
    Ellipse(Ellipse3),
    Nurbs(NurbsCurve),
}

impl Curve3 {
    pub fn point(&self, t: f64) -> Vec3 {
        match self {
            Curve3::Line(c) => c.point(t),
            Curve3::Circle(c) => c.point(t),
            Curve3::Ellipse(c) => c.point(t),
            Curve3::Nurbs(c) => c.point(t),
        }
    }

    /// Derivatives 0..=d at t. Analytic types are exact at any order.
    pub fn derivatives(&self, t: f64, d: usize) -> Vec<Vec3> {
        match self {
            Curve3::Line(c) => {
                let mut out = Vec::with_capacity(d + 1);
                out.push(c.point(t));
                if d >= 1 {
                    out.push(c.dir);
                }
                while out.len() <= d {
                    out.push(Vec3::ZERO);
                }
                out
            }
            Curve3::Circle(c) => (0..=d)
                .map(|k| {
                    if k == 0 {
                        c.point(t)
                    } else {
                        // Successive derivatives advance phase by 90
                        // degrees: d^k/dt^k of (cos, sin) at t equals
                        // (cos, sin) at t + k*pi/2 scaled by r.
                        let th = t + k as f64 * core::f64::consts::FRAC_PI_2;
                        c.x_axis * (c.radius * th.cos()) + c.y_axis * (c.radius * th.sin())
                    }
                })
                .collect(),
            Curve3::Ellipse(c) => (0..=d)
                .map(|k| {
                    if k == 0 {
                        return c.point(t);
                    }
                    match k % 4 {
                        1 => c.x_axis * (-c.a * t.sin()) + c.y_axis * (c.b * t.cos()),
                        2 => c.x_axis * (-c.a * t.cos()) + c.y_axis * (-c.b * t.sin()),
                        3 => c.x_axis * (c.a * t.sin()) + c.y_axis * (-c.b * t.cos()),
                        _ => c.x_axis * (c.a * t.cos()) + c.y_axis * (c.b * t.sin()),
                    }
                })
                .collect(),
            Curve3::Nurbs(c) => c.derivatives(t, d),
        }
    }

    pub fn domain(&self) -> Domain {
        match self {
            Curve3::Line(_) => Domain::Finite {
                a: f64::NEG_INFINITY,
                b: f64::INFINITY,
            },
            Curve3::Circle(_) | Curve3::Ellipse(_) => Domain::Periodic {
                period: core::f64::consts::TAU,
            },
            Curve3::Nurbs(c) => {
                let (a, b) = c.domain();
                Domain::Finite { a, b }
            }
        }
    }

    /// Bounding box. Lines are unbounded; topology-level code bounds
    /// them via edges and must not call bbox on a bare line (the
    /// origin's box is returned as a defined placeholder).
    pub fn bbox(&self) -> Aabb3 {
        match self {
            Curve3::Line(c) => Aabb3::from_points([c.origin]),
            Curve3::Circle(c) => {
                // Extent of r*(cos*x + sin*y) along axis i is
                // r * sqrt(x_i^2 + y_i^2).
                let r = c.radius;
                let e = Vec3::new(
                    (c.x_axis.x.powi(2) + c.y_axis.x.powi(2)).sqrt(),
                    (c.x_axis.y.powi(2) + c.y_axis.y.powi(2)).sqrt(),
                    (c.x_axis.z.powi(2) + c.y_axis.z.powi(2)).sqrt(),
                ) * r;
                Aabb3 { min: c.center - e, max: c.center + e }
            }
            Curve3::Ellipse(c) => {
                let e = Vec3::new(
                    ((c.a * c.x_axis.x).powi(2) + (c.b * c.y_axis.x).powi(2)).sqrt(),
                    ((c.a * c.x_axis.y).powi(2) + (c.b * c.y_axis.y).powi(2)).sqrt(),
                    ((c.a * c.x_axis.z).powi(2) + (c.b * c.y_axis.z).powi(2)).sqrt(),
                );
                Aabb3 { min: c.center - e, max: c.center + e }
            }
            Curve3::Nurbs(c) => Aabb3::from_points(c.control_points()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_project_roundtrip() {
        let l = Line3::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 2.0, 0.0)).unwrap();
        let p = l.point(3.5) + Vec3::new(5.0, 0.0, 0.0); // offset normal to dir
        assert!((l.project(p) - 3.5).abs() < 1e-14);
    }

    #[test]
    fn circle_project_roundtrip() {
        let c = Circle3::new(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            2.0,
        )
        .unwrap();
        let theta = 2.3;
        // Radial and axial offsets leave the projection unchanged.
        let p = c.point(theta) + (c.point(theta) - c.center) * 0.4
            + Vec3::new(0.0, 0.0, 9.0);
        assert!((c.project(p) - theta).abs() < 1e-12);
    }

    #[test]
    fn ellipse_project_known_cases() {
        let e = Ellipse3::new(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            3.0,
            1.0,
        )
        .unwrap();
        // On-axis exterior point projects to theta = 0.
        assert!(e.project(Vec3::new(10.0, 0.0, 0.0)).abs() < 1e-9);
        // A small normal offset projects back at least as close as
        // the originating parameter.
        let th = 0.9_f64;
        let d1 = e.deriv(th);
        let n = Vec3::new(0.0, 0.0, 1.0).cross(d1).try_normalize().unwrap();
        let p = e.point(th) + n * 0.2;
        let got = e.project(p);
        assert!(
            (e.point(got) - p).norm() <= (e.point(th) - p).norm() + 1e-12,
            "projection not closer: got {got} want {th}"
        );
    }

    #[test]
    fn circle_bbox_contains_samples() {
        let x = Vec3::new(1.0, 1.0, 0.0).try_normalize().unwrap();
        let y = Vec3::new(0.0, 0.0, 1.0).cross(x).try_normalize().unwrap();
        let c = Circle3::new(Vec3::new(1.0, 2.0, 3.0), x, y, 1.7).unwrap();
        let bb = Curve3::Circle(c).bbox().expanded(1e-12);
        for i in 0..64 {
            let th = i as f64 * core::f64::consts::TAU / 64.0;
            assert!(bb.contains(c.point(th)), "theta={th}");
        }
    }

    #[test]
    fn circle_dispatch_derivatives_match_fd() {
        let c = Circle3::new(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            2.0,
        )
        .unwrap();
        let cv = Curve3::Circle(c);
        let t = 1.1;
        // h near eps^(1/4): the optimum for central second
        // differences, whose rounding noise scales as eps/h^2.
        let h = 1e-4;
        let d = cv.derivatives(t, 2);
        let fd1 = (cv.point(t + h) - cv.point(t - h)) / (2.0 * h);
        assert!((d[1] - fd1).norm() < 1e-7);
        let fd2 = (cv.point(t + h) + cv.point(t - h) - cv.point(t) * 2.0) / (h * h);
        assert!((d[2] - fd2).norm() < 1e-5, "{:?} vs {:?}", d[2], fd2);
    }
}
