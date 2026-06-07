//! Analytic surfaces and shared local differential geometry.

use crate::GeomError;
use keel_math::vec::Vec3;

/// Second-order local geometry of a parametric surface at (u, v):
/// derivatives, unit normal, fundamental forms, curvatures, and
/// principal directions. The contract M5 (Krawczyk tracing) and the
/// kernel/24 canonical-recovery service consume.
#[derive(Clone, Debug)]
pub struct SurfaceLocalGeometry {
    pub point: Vec3,
    pub du: Vec3,
    pub dv: Vec3,
    pub duu: Vec3,
    pub duv: Vec3,
    pub dvv: Vec3,
    /// Unit normal, du x dv normalized.
    pub normal: Vec3,
    /// First fundamental form E, F, G.
    pub e: f64,
    pub f: f64,
    pub g: f64,
    /// Second fundamental form L, M, N.
    pub l: f64,
    pub m: f64,
    pub n: f64,
    pub gaussian: f64,
    pub mean: f64,
    /// Principal curvatures, k1 >= k2.
    pub k1: f64,
    pub k2: f64,
    /// Unit principal directions in 3D for k1 and k2. At an umbilic
    /// (k1 == k2 to working precision) any orthonormal tangent pair is
    /// principal; we return normalized du and its in-plane
    /// perpendicular, deterministically.
    pub dir1: Vec3,
    pub dir2: Vec3,
}

/// Build local geometry from raw derivatives. Degenerate when the
/// normal vanishes (collapsed or singular parameterization).
pub(crate) fn local_geometry_from_ders(
    point: Vec3,
    du: Vec3,
    dv: Vec3,
    duu: Vec3,
    duv: Vec3,
    dvv: Vec3,
) -> Result<SurfaceLocalGeometry, GeomError> {
    let raw_n = du.cross(dv);
    let nn = raw_n.norm();
    let scale = du.norm().max(dv.norm());
    // Relative degeneracy test: |du x dv| tiny against the tangent
    // scale squared means the tangents are parallel or vanish. The
    // NaN-propagating comparison also rejects non-finite derivatives.
    if scale == 0.0 || nn.is_nan() || nn <= 1e-14 * scale * scale {
        return Err(GeomError::Degenerate);
    }
    let normal = raw_n * (1.0 / nn);
    let (e, f, g) = (du.dot(du), du.dot(dv), dv.dot(dv));
    let (l, m, n) = (duu.dot(normal), duv.dot(normal), dvv.dot(normal));
    let det1 = e * g - f * f; // == nn^2 > 0 here
    let gaussian = (l * n - m * m) / det1;
    let mean = (e * n - 2.0 * f * m + g * l) / (2.0 * det1);
    // Guard tiny negative discriminants from roundoff at umbilics.
    let disc = (mean * mean - gaussian).max(0.0).sqrt();
    let (k1, k2) = (mean + disc, mean - disc);
    let (dir1, dir2) = principal_dirs(k1, k2, e, f, g, l, m, n, du, dv, normal);
    Ok(SurfaceLocalGeometry {
        point,
        du,
        dv,
        duu,
        duv,
        dvv,
        normal,
        e,
        f,
        g,
        l,
        m,
        n,
        gaussian,
        mean,
        k1,
        k2,
        dir1,
        dir2,
    })
}

/// Right-handed orthonormal frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame3 {
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Frame3 {
    /// Build from origin and z axis; x is chosen deterministically as
    /// the normalized rejection of the global axis least aligned with z.
    pub fn from_z(origin: Vec3, z: Vec3) -> Result<Self, GeomError> {
        if !origin.is_finite() {
            return Err(GeomError::NonFinitePoint);
        }
        let z = z.try_normalize().ok_or(GeomError::Degenerate)?;
        let (ax, ay, az) = (z.x.abs(), z.y.abs(), z.z.abs());
        let h = if ax <= ay && ax <= az {
            Vec3::new(1., 0., 0.)
        } else if ay <= az {
            Vec3::new(0., 1., 0.)
        } else {
            Vec3::new(0., 0., 1.)
        };
        let x = (h - z * h.dot(z))
            .try_normalize()
            .ok_or(GeomError::Degenerate)?;
        let y = z.cross(x);
        Ok(Self { origin, x, y, z })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plane3 {
    pub frame: Frame3,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Cylinder3 {
    pub frame: Frame3,
    pub radius: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Cone3 {
    pub frame: Frame3,
    /// Radius at v = 0.
    pub radius: f64,
    /// Half-angle; the radius grows by tan(half_angle) per unit height.
    pub half_angle: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Sphere3 {
    pub frame: Frame3,
    pub radius: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Torus3 {
    pub frame: Frame3,
    pub major: f64,
    pub minor: f64,
}

impl Plane3 {
    pub fn new(frame: Frame3) -> Self {
        Self { frame }
    }
}
impl Cylinder3 {
    pub fn new(frame: Frame3, radius: f64) -> Result<Self, GeomError> {
        if !radius.is_finite() || radius <= 0.0 {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { frame, radius })
    }
}
impl Cone3 {
    /// `radius` at v = 0; half_angle nonzero, in (-pi/2, pi/2).
    pub fn new(frame: Frame3, radius: f64, half_angle: f64) -> Result<Self, GeomError> {
        if !radius.is_finite()
            || radius <= 0.0
            || !half_angle.is_finite()
            || half_angle == 0.0
            || half_angle.abs() >= core::f64::consts::FRAC_PI_2
        {
            return Err(GeomError::Degenerate);
        }
        Ok(Self {
            frame,
            radius,
            half_angle,
        })
    }
}
impl Sphere3 {
    pub fn new(frame: Frame3, radius: f64) -> Result<Self, GeomError> {
        if !radius.is_finite() || radius <= 0.0 {
            return Err(GeomError::Degenerate);
        }
        Ok(Self { frame, radius })
    }
}
impl Torus3 {
    /// Ring torus only: major > minor > 0.
    pub fn new(frame: Frame3, major: f64, minor: f64) -> Result<Self, GeomError> {
        if !major.is_finite() || !minor.is_finite() || minor <= 0.0 || major <= minor {
            return Err(GeomError::Degenerate);
        }
        Ok(Self {
            frame,
            major,
            minor,
        })
    }
}

/// First-class analytic surfaces; never silently splinified (spec D4).
///
/// Parameterizations (X, Y, Z frame axes, o the origin):
/// plane S = o + uX + vY; cylinder S = o + r cos(u)X + r sin(u)Y + vZ;
/// cone S = o + (r0 + v tan(alpha))(cos(u)X + sin(u)Y) + vZ;
/// sphere S = o + r cos(v)(cos(u)X + sin(u)Y) + r sin(v)Z, v latitude;
/// torus S = o + (R + r cos(v))(cos(u)X + sin(u)Y) + r sin(v)Z.
#[derive(Clone, Debug, PartialEq)]
pub enum Surface3 {
    Plane(Plane3),
    Cylinder(Cylinder3),
    Cone(Cone3),
    Sphere(Sphere3),
    Torus(Torus3),
}

/// Result of projecting a point onto a surface.
#[derive(Clone, Debug)]
pub struct SurfaceProjection {
    pub u: f64,
    pub v: f64,
    pub point: Vec3,
    pub distance: f64,
}

/// Raw derivative bundle through second order.
pub(crate) struct Ders2 {
    pub s: Vec3,
    pub su: Vec3,
    pub sv: Vec3,
    pub suu: Vec3,
    pub suv: Vec3,
    pub svv: Vec3,
}

impl Surface3 {
    pub fn point(&self, u: f64, v: f64) -> Vec3 {
        self.ders2(u, v).s
    }

    /// Local geometry from the exact closed-form derivatives.
    /// Degenerate at parameterization singularities (sphere poles,
    /// cone apex).
    pub fn local_geometry(&self, u: f64, v: f64) -> Result<SurfaceLocalGeometry, GeomError> {
        let d = self.ders2(u, v);
        local_geometry_from_ders(d.s, d.su, d.sv, d.suu, d.suv, d.svv)
    }

    pub(crate) fn ders2(&self, u: f64, v: f64) -> Ders2 {
        match self {
            Surface3::Plane(pl) => {
                let f = &pl.frame;
                Ders2 {
                    s: f.origin + f.x * u + f.y * v,
                    su: f.x,
                    sv: f.y,
                    suu: Vec3::ZERO,
                    suv: Vec3::ZERO,
                    svv: Vec3::ZERO,
                }
            }
            Surface3::Cylinder(c) => {
                let f = &c.frame;
                let (cu, su_) = (u.cos(), u.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                Ders2 {
                    s: f.origin + rad * c.radius + f.z * v,
                    su: tan * c.radius,
                    sv: f.z,
                    suu: rad * (-c.radius),
                    suv: Vec3::ZERO,
                    svv: Vec3::ZERO,
                }
            }
            Surface3::Cone(c) => {
                let f = &c.frame;
                let m = c.half_angle.tan();
                let (cu, su_) = (u.cos(), u.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                let r = c.radius + v * m;
                Ders2 {
                    s: f.origin + rad * r + f.z * v,
                    su: tan * r,
                    sv: rad * m + f.z,
                    suu: rad * (-r),
                    suv: tan * m,
                    svv: Vec3::ZERO,
                }
            }
            Surface3::Sphere(sp) => {
                let f = &sp.frame;
                let r = sp.radius;
                let (cu, su_) = (u.cos(), u.sin());
                let (cv, sv_) = (v.cos(), v.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                Ders2 {
                    s: f.origin + (rad * cv + f.z * sv_) * r,
                    su: tan * (r * cv),
                    sv: (f.z * cv - rad * sv_) * r,
                    suu: rad * (-r * cv),
                    suv: tan * (-r * sv_),
                    svv: (rad * cv + f.z * sv_) * (-r),
                }
            }
            Surface3::Torus(t) => {
                let f = &t.frame;
                let (cu, su_) = (u.cos(), u.sin());
                let (cv, sv_) = (v.cos(), v.sin());
                let rad = f.x * cu + f.y * su_;
                let tan = f.y * cu - f.x * su_;
                let ring = t.major + t.minor * cv;
                Ders2 {
                    s: f.origin + rad * ring + f.z * (t.minor * sv_),
                    su: tan * ring,
                    sv: rad * (-t.minor * sv_) + f.z * (t.minor * cv),
                    suu: rad * (-ring),
                    suv: tan * (-t.minor * sv_),
                    svv: rad * (-t.minor * cv) + f.z * (-t.minor * sv_),
                }
            }
        }
    }

    /// Exact closest-point projection. Axis-ambiguous inputs resolve
    /// deterministically: u = 0 on a symmetry axis, v = 0 at the
    /// sphere center and on the torus tube-center circle.
    pub fn project(&self, p: Vec3) -> Result<SurfaceProjection, GeomError> {
        if !p.is_finite() {
            return Err(GeomError::NonFinitePoint);
        }
        let tau = core::f64::consts::TAU;
        match self {
            Surface3::Plane(pl) => {
                let f = &pl.frame;
                let w = p - f.origin;
                let (u, v) = (w.dot(f.x), w.dot(f.y));
                let q = f.origin + f.x * u + f.y * v;
                Ok(SurfaceProjection {
                    u,
                    v,
                    point: q,
                    distance: (p - q).norm(),
                })
            }
            Surface3::Cylinder(c) => {
                let f = &c.frame;
                let w = p - f.origin;
                let h = w.dot(f.z);
                let (rx, ry) = (w.dot(f.x), w.dot(f.y));
                let u = if rx * rx + ry * ry > 0.0 {
                    ry.atan2(rx).rem_euclid(tau)
                } else {
                    0.0
                };
                let q = f.origin + (f.x * u.cos() + f.y * u.sin()) * c.radius + f.z * h;
                Ok(SurfaceProjection {
                    u,
                    v: h,
                    point: q,
                    distance: (p - q).norm(),
                })
            }
            Surface3::Sphere(sp) => {
                let f = &sp.frame;
                let w = p - f.origin;
                let wn = w.norm();
                if wn == 0.0 {
                    // Center: deterministic (u, v) = (0, 0).
                    let q = f.origin + f.x * sp.radius;
                    return Ok(SurfaceProjection {
                        u: 0.0,
                        v: 0.0,
                        point: q,
                        distance: sp.radius,
                    });
                }
                let dirn = w * (1.0 / wn);
                let (dx, dy, dz) = (dirn.dot(f.x), dirn.dot(f.y), dirn.dot(f.z));
                let u = if dx * dx + dy * dy > 0.0 {
                    dy.atan2(dx).rem_euclid(tau)
                } else {
                    0.0
                };
                let v = dz.clamp(-1.0, 1.0).asin();
                let q = f.origin + dirn * sp.radius;
                Ok(SurfaceProjection {
                    u,
                    v,
                    point: q,
                    distance: (wn - sp.radius).abs(),
                })
            }
            Surface3::Torus(t) => {
                let f = &t.frame;
                let w = p - f.origin;
                let (rx, ry) = (w.dot(f.x), w.dot(f.y));
                let u = if rx * rx + ry * ry > 0.0 {
                    ry.atan2(rx).rem_euclid(tau)
                } else {
                    0.0
                };
                let rad = f.x * u.cos() + f.y * u.sin();
                let center = f.origin + rad * t.major; // tube center at u
                let d = p - center;
                let dn = d.norm();
                let (v, q) = if dn == 0.0 {
                    (0.0, center + rad * t.minor)
                } else {
                    let mr = d.dot(rad);
                    let mz = d.dot(f.z);
                    let v = mz.atan2(mr).rem_euclid(tau);
                    (v, center + d * (t.minor / dn))
                };
                Ok(SurfaceProjection {
                    u,
                    v,
                    point: q,
                    distance: (p - q).norm(),
                })
            }
            Surface3::Cone(c) => {
                let f = &c.frame;
                let m = c.half_angle.tan();
                let w = p - f.origin;
                let h = w.dot(f.z);
                let (rx, ry) = (w.dot(f.x), w.dot(f.y));
                let rr = (rx * rx + ry * ry).sqrt();
                let u = if rr > 0.0 {
                    ry.atan2(rx).rem_euclid(tau)
                } else {
                    0.0
                };
                // Meridian half-plane: the profile is the line
                // (rho, h) = (r0 + t m, t); project (rr, h) onto it.
                // M3: clamp at the apex once bounded surfaces exist.
                let t = ((rr - c.radius) * m + h) / (1.0 + m * m);
                let rad = f.x * u.cos() + f.y * u.sin();
                let q = f.origin + rad * (c.radius + t * m) + f.z * t;
                Ok(SurfaceProjection {
                    u,
                    v: t,
                    point: q,
                    distance: (p - q).norm(),
                })
            }
        }
    }
}

/// Principal directions: null vectors of (II - k I) in the {du, dv}
/// basis. Pick the larger row for stability; at an umbilic both rows
/// vanish and we fall back to the deterministic orthonormal pair.
#[allow(clippy::too_many_arguments)]
fn principal_dirs(
    k1: f64,
    k2: f64,
    e: f64,
    f: f64,
    g: f64,
    l: f64,
    m: f64,
    n: f64,
    du: Vec3,
    dv: Vec3,
    normal: Vec3,
) -> (Vec3, Vec3) {
    let tangent_dir = |k: f64| -> Option<Vec3> {
        let (r1a, r1b) = (l - k * e, m - k * f);
        let (r2a, r2b) = (m - k * f, n - k * g);
        let (a, b) = if r1a * r1a + r1b * r1b >= r2a * r2a + r2b * r2b {
            (r1b, -r1a)
        } else {
            (r2b, -r2a)
        };
        let d = du * a + dv * b;
        let dn = d.norm();
        // Reject when the row is numerically zero relative to the
        // form scale: umbilic.
        let row_scale = (e + g) * (1.0 + k.abs());
        if dn > 1e-10 * row_scale {
            Some(d * (1.0 / dn))
        } else {
            None
        }
    };
    match (tangent_dir(k1), tangent_dir(k2)) {
        (Some(d1), Some(d2)) => (d1, d2),
        (Some(d1), None) => (d1, normal.cross(d1)),
        (None, Some(d2)) => (d2.cross(normal), d2),
        (None, None) => {
            // Umbilic: deterministic orthonormal tangent pair.
            let d1 = du * (1.0 / du.norm());
            (d1, normal.cross(d1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn frame_from_z_is_orthonormal() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(1., 2., 3.)).unwrap();
        assert!((f.x.norm() - 1.0).abs() < 1e-14);
        assert!(f.x.dot(f.z).abs() < 1e-14);
        assert!((f.x.cross(f.y) - f.z).norm() < 1e-14);
        assert!(Frame3::from_z(Vec3::ZERO, Vec3::ZERO).is_err());
    }

    #[test]
    fn analytic_curvatures_match_closed_forms() {
        let f = Frame3::from_z(Vec3::new(1., -2., 0.5), Vec3::new(0., 0., 1.)).unwrap();
        // Cylinder r = 2: K = 0, |H| = 1/(2r).
        let cyl = Surface3::Cylinder(Cylinder3::new(f.clone(), 2.0).unwrap());
        let lg = cyl.local_geometry(0.7, 1.3).unwrap();
        assert!(lg.gaussian.abs() < 1e-12);
        assert!((lg.mean.abs() - 0.25).abs() < 1e-12);
        // Sphere r = 3: K = 1/r^2, |H| = 1/r, umbilic everywhere.
        let sph = Surface3::Sphere(Sphere3::new(f.clone(), 3.0).unwrap());
        let lg = sph.local_geometry(0.4, 0.2).unwrap();
        assert!((lg.gaussian - 1.0 / 9.0).abs() < 1e-12);
        assert!((lg.mean.abs() - 1.0 / 3.0).abs() < 1e-12);
        // Umbilic split is sqrt-amplified roundoff: sqrt(eps * K) scale.
        assert!((lg.k1 - lg.k2).abs() < 1e-7);
        // Torus R = 3, r = 1: K = cos(v) / (r (R + r cos v)).
        let tor = Surface3::Torus(Torus3::new(f, 3.0, 1.0).unwrap());
        let v = 0.9f64;
        let lg = tor.local_geometry(1.1, v).unwrap();
        let want_k = v.cos() / (3.0 + v.cos());
        assert!((lg.gaussian - want_k).abs() < 1e-12);
    }

    #[test]
    fn sphere_pole_is_degenerate() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let sph = Surface3::Sphere(Sphere3::new(f, 1.0).unwrap());
        assert_eq!(
            sph.local_geometry(0.3, core::f64::consts::FRAC_PI_2).unwrap_err(),
            GeomError::Degenerate
        );
    }

    #[test]
    fn analytic_projections_are_exact() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let cyl = Surface3::Cylinder(Cylinder3::new(f.clone(), 2.0).unwrap());
        let pr = cyl.project(Vec3::new(4.0, 0.0, 5.0)).unwrap();
        assert!((pr.distance - 2.0).abs() < 1e-14);
        assert!((pr.point - Vec3::new(2.0, 0.0, 5.0)).norm() < 1e-14);
        // Axis point: deterministic u = 0 convention.
        let pr = cyl.project(Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert_eq!(pr.u, 0.0);
        assert!((pr.distance - 2.0).abs() < 1e-14);
        let tor = Surface3::Torus(Torus3::new(f.clone(), 3.0, 1.0).unwrap());
        let pr = tor.project(Vec3::new(5.0, 0.0, 0.0)).unwrap();
        assert!((pr.distance - 1.0).abs() < 1e-14);
        assert!((pr.point - Vec3::new(4.0, 0.0, 0.0)).norm() < 1e-14);
        let con = Surface3::Cone(Cone3::new(f, 1.0, core::f64::consts::FRAC_PI_4).unwrap());
        // Point one unit out along the wall normal at (1, 0, 0): the
        // wall direction in the rz half-plane is (1, 1)/sqrt(2), its
        // outward normal (1, -1)/sqrt(2).
        let d = core::f64::consts::SQRT_2;
        let pr = con
            .project(Vec3::new(1.0 + 1.0 / d, 0.0, -1.0 / d))
            .unwrap();
        assert!((pr.distance - 1.0).abs() < 1e-12);
        assert!((pr.point - Vec3::new(1.0, 0.0, 0.0)).norm() < 1e-12);
    }

    #[test]
    fn projected_points_lie_on_surface() {
        let f = Frame3::from_z(Vec3::new(0.3, -0.7, 2.0), Vec3::new(1., 1., 1.)).unwrap();
        let surfaces = [
            Surface3::Cylinder(Cylinder3::new(f.clone(), 1.5).unwrap()),
            Surface3::Sphere(Sphere3::new(f.clone(), 2.5).unwrap()),
            Surface3::Torus(Torus3::new(f.clone(), 3.0, 0.8).unwrap()),
            Surface3::Cone(Cone3::new(f, 1.0, 0.5).unwrap()),
        ];
        let p = Vec3::new(4.0, -1.0, 3.0);
        for s in &surfaces {
            let pr = s.project(p).unwrap();
            // The reported (u, v) reproduces the reported point.
            assert!(
                (s.point(pr.u, pr.v) - pr.point).norm() < 1e-10,
                "uv roundtrip failed for {s:?}"
            );
            assert!((pr.point - p).norm() <= pr.distance + 1e-12);
        }
    }

    proptest! {
        // Projection oracle: the projected point beats a dense sample.
        #[test]
        fn torus_projection_beats_dense_sampling(
            px in -6.0..6.0f64, py in -6.0..6.0f64, pz in -3.0..3.0f64,
        ) {
            let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
            let tor = Surface3::Torus(Torus3::new(f, 3.0, 1.0).unwrap());
            let p = Vec3::new(px, py, pz);
            let pr = tor.project(p).unwrap();
            let tau = core::f64::consts::TAU;
            for i in 0..32 {
                for j in 0..32 {
                    let q = tor.point(tau * i as f64 / 32.0, tau * j as f64 / 32.0);
                    prop_assert!((q - p).norm() >= pr.distance - 1e-9);
                }
            }
        }
    }
}
