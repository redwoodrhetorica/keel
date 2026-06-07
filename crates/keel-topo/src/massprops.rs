//! Mass properties via the divergence theorem (M4 Task 6).
//!
//! THE ORIENTATION AUDIT: volumes are computed with NO sign fudge.
//! The per-face orientation comes purely from the M3 region-solidity
//! conventions (front faces the parent region); a negative volume
//! here is a bug in M3's conventions, never something to abs() away.
//!
//! Integration dispatch: planar faces integrate their UV region
//! exactly-enough (triangle fan with a degree-5 rule for polygon
//! loops; periodic-trapezoid x Gauss-Legendre polar for disc loops);
//! curved faces integrate their parameter rectangle with composite
//! Gauss-Legendre (full-coverage faces use the canonical rectangle).
//! General trimmed regions arrive with M5 trims.

use crate::body::{Body, TopoError};
use crate::entity::{FaceKey, SurfaceGeom};
use keel_geom::curve::Curve3;
use keel_geom::surface::Surface3;
use keel_math::vec::Vec3;

#[derive(Clone, Debug)]
pub struct MassProps {
    pub volume: f64,
    pub centroid: Vec3,
    /// Inertia tensor about the centroid (unit density).
    pub inertia: [[f64; 3]; 3],
}

/// Accumulated divergence-theorem moments (about the origin).
#[derive(Clone, Copy, Debug, Default)]
struct Moments {
    v: f64,
    mx: f64,
    my: f64,
    mz: f64,
    ixx: f64,
    iyy: f64,
    izz: f64,
    pxy: f64,
    pxz: f64,
    pyz: f64,
}

impl Moments {
    /// Accumulate one sample: position s, UNNORMALIZED area-weighted
    /// normal n (already orientation-corrected), quadrature weight w.
    fn add(&mut self, s: Vec3, n: Vec3, w: f64) {
        self.v += w * s.x * n.x;
        self.mx += w * 0.5 * s.x * s.x * n.x;
        self.my += w * 0.5 * s.y * s.y * n.y;
        self.mz += w * 0.5 * s.z * s.z * n.z;
        self.ixx += w * (s.y * s.y * s.y * n.y + s.z * s.z * s.z * n.z) / 3.0;
        self.iyy += w * (s.x * s.x * s.x * n.x + s.z * s.z * s.z * n.z) / 3.0;
        self.izz += w * (s.x * s.x * s.x * n.x + s.y * s.y * s.y * n.y) / 3.0;
        self.pxy += w * 0.5 * s.x * s.x * s.y * n.x;
        self.pxz += w * 0.5 * s.x * s.x * s.z * n.x;
        self.pyz += w * 0.5 * s.y * s.y * s.z * n.y;
    }
}

/// 8-point Gauss-Legendre nodes/weights on [-1, 1] (standard
/// tabulated values; full published digits kept for auditability).
#[allow(clippy::excessive_precision)]
const GL8_X: [f64; 8] = [
    -0.9602898564975363,
    -0.7966664774136267,
    -0.5255324099163290,
    -0.1834346424956498,
    0.1834346424956498,
    0.5255324099163290,
    0.7966664774136267,
    0.9602898564975363,
];
#[allow(clippy::excessive_precision)]
const GL8_W: [f64; 8] = [
    0.1012285362903763,
    0.2223810344533745,
    0.3137066458778873,
    0.3626837833783620,
    0.3626837833783620,
    0.3137066458778873,
    0.2223810344533745,
    0.1012285362903763,
];

/// Degree-5 symmetric triangle rule (7 points), exact for our cubic
/// integrands on planar faces.
fn triangle_rule() -> [([f64; 3], f64); 7] {
    let a = (6.0 - 15.0f64.sqrt()) / 21.0;
    let b = (6.0 + 15.0f64.sqrt()) / 21.0;
    let wa = (155.0 - 15.0f64.sqrt()) / 1200.0;
    let wb = (155.0 + 15.0f64.sqrt()) / 1200.0;
    [
        ([1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0], 9.0 / 40.0),
        ([1.0 - 2.0 * a, a, a], wa),
        ([a, 1.0 - 2.0 * a, a], wa),
        ([a, a, 1.0 - 2.0 * a], wa),
        ([1.0 - 2.0 * b, b, b], wb),
        ([b, 1.0 - 2.0 * b, b], wb),
        ([b, b, 1.0 - 2.0 * b], wb),
    ]
}

impl Body {
    /// Mass properties of the body's solid regions (unit density).
    pub fn mass_properties(&self) -> Result<MassProps, TopoError> {
        let mut m = Moments::default();
        let faces: Vec<FaceKey> = self
            .entity_ids()
            .filter_map(|id| match self.lookup(id) {
                Some(crate::entity::AnyKey::Face(k)) => Some(k),
                _ => None,
            })
            .collect();
        for fk in faces {
            let face = self.faces.get(fk).ok_or(TopoError::StaleKey)?;
            let fs = self
                .regions
                .get(face.front_region)
                .map(|r| r.solid)
                .ok_or(TopoError::StaleKey)?;
            let bs = self
                .regions
                .get(face.back_region)
                .map(|r| r.solid)
                .ok_or(TopoError::StaleKey)?;
            // Outward orientation from region solidity ALONE (the M3
            // audit): the parametric normal points out of Front.
            let orient = match (fs, bs) {
                (false, true) => 1.0,  // solid behind: normal is outward
                (true, false) => -1.0, // solid in front: flip
                _ => {
                    return Err(TopoError::Precondition(
                        "mass_properties: face does not bound exactly one solid region",
                    ));
                }
            };
            let Some((sk, _)) = face.surface else {
                return Err(TopoError::Precondition(
                    "mass_properties: face without surface",
                ));
            };
            let Some(SurfaceGeom::Analytic(surf)) = self.surfaces.get(sk) else {
                return Err(TopoError::Precondition(
                    "mass_properties: NURBS faces are M5",
                ));
            };
            match surf {
                Surface3::Plane(_) => self.integrate_planar_face(fk, surf, orient, &mut m)?,
                _ => self.integrate_curved_face(fk, surf, orient, &mut m)?,
            }
        }
        if m.v <= 0.0 {
            // The audit itself: conventions produced a non-positive
            // volume. Fail loudly; the fix belongs in M3.
            return Err(TopoError::Precondition(
                "mass_properties: non-positive volume (orientation conventions violated)",
            ));
        }
        let centroid = Vec3::new(m.mx / m.v, m.my / m.v, m.mz / m.v);
        // Parallel-axis transfer to the centroid.
        let (cx, cy, cz) = (centroid.x, centroid.y, centroid.z);
        let ixx = m.ixx - m.v * (cy * cy + cz * cz);
        let iyy = m.iyy - m.v * (cx * cx + cz * cz);
        let izz = m.izz - m.v * (cx * cx + cy * cy);
        let pxy = m.pxy - m.v * cx * cy;
        let pxz = m.pxz - m.v * cx * cz;
        let pyz = m.pyz - m.v * cy * cz;
        Ok(MassProps {
            volume: m.v,
            centroid,
            inertia: [[ixx, -pxy, -pxz], [-pxy, iyy, -pyz], [-pxz, -pyz, izz]],
        })
    }

    /// Planar face: integrate the UV region from its pcurves.
    fn integrate_planar_face(
        &self,
        fk: FaceKey,
        surf: &Surface3,
        orient: f64,
        m: &mut Moments,
    ) -> Result<(), TopoError> {
        let Surface3::Plane(plane) = surf else {
            return Err(TopoError::Precondition("not a plane"));
        };
        let f = &plane.frame;
        let normal = f.z * orient;
        let at = |u: f64, v: f64| -> Vec3 { f.origin + f.x * u + f.y * v };
        let face = self.faces.get(fk).ok_or(TopoError::StaleKey)?;
        if face.loops.len() != 1 {
            return Err(TopoError::Precondition(
                "mass_properties: planar face with rings is M5 work",
            ));
        }
        let lp = face.loops[0];
        let entry = self
            .loops
            .get(lp)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("vertex loop face"))?;
        // Disc (single closed-circle pcurve) or polygon?
        let fins: Vec<crate::entity::FinKey> = {
            let mut out = Vec::new();
            let mut cur = entry;
            while let Some(fin) = self.fins.get(cur) {
                out.push(cur);
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
            out
        };
        let single_circle = fins.len() == 1
            && fins.first().is_some_and(|&fk2| {
                self.fins
                    .get(fk2)
                    .and_then(|fin| fin.pcurve)
                    .and_then(|(ck, _)| self.curves.get(ck))
                    .is_some_and(|c| matches!(c, Curve3::Circle(_)))
            });
        if single_circle {
            let (ck, _) = self
                .fins
                .get(fins[0])
                .and_then(|fin| fin.pcurve)
                .ok_or(TopoError::StaleKey)?;
            let Some(Curve3::Circle(uvc)) = self.curves.get(ck) else {
                return Err(TopoError::StaleKey);
            };
            let (cu, cv, r) = (uvc.center.x, uvc.center.y, uvc.radius);
            // Polar: periodic trapezoid in theta (exact for trig
            // polynomials), GL8 radial.
            let nt = 32usize;
            for it in 0..nt {
                let theta = core::f64::consts::TAU * it as f64 / nt as f64;
                let wt = core::f64::consts::TAU / nt as f64;
                for (xi, wi) in GL8_X.iter().zip(GL8_W) {
                    let rho = 0.5 * r * (xi + 1.0);
                    let wr = 0.5 * r * wi;
                    let (u, v) = (cu + rho * theta.cos(), cv + rho * theta.sin());
                    m.add(at(u, v), normal, wt * wr * rho);
                }
            }
        } else {
            // Polygon: vertex UVs in walk order; triangle fan.
            let mut poly: Vec<(f64, f64)> = Vec::new();
            for &fin in &fins {
                let p = self
                    .fin_start_vertex(fin)
                    .and_then(|v| self.vertices.get(v).map(|x| x.point))
                    .ok_or(TopoError::StaleKey)?;
                let w = p - f.origin;
                poly.push((w.dot(f.x), w.dot(f.y)));
            }
            if poly.len() < 3 {
                return Err(TopoError::Precondition("degenerate planar loop"));
            }
            let rule = triangle_rule();
            for i in 1..poly.len() - 1 {
                let (a, b, c) = (poly[0], poly[i], poly[i + 1]);
                // Signed AREA (half the cross product): the rule's
                // weights sum to 1, so the factor is the area itself;
                // its sign handles fan orientation.
                let area = 0.5 * ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0));
                for (bary, w) in rule {
                    let u = bary[0] * a.0 + bary[1] * b.0 + bary[2] * c.0;
                    let v = bary[0] * a.1 + bary[1] * b.1 + bary[2] * c.1;
                    m.add(at(u, v), normal, w * area);
                }
            }
        }
        Ok(())
    }

    /// Curved face: composite GL over the parameter rectangle.
    fn integrate_curved_face(
        &self,
        fk: FaceKey,
        surf: &Surface3,
        orient: f64,
        m: &mut Moments,
    ) -> Result<(), TopoError> {
        let tau = core::f64::consts::TAU;
        let ((u0, u1), (v0, v1)) = if self.face_covers_closed_surface(fk) {
            match surf {
                Surface3::Sphere(_) => (
                    (0.0, tau),
                    (-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2),
                ),
                Surface3::Torus(_) => ((0.0, tau), (0.0, tau)),
                _ => {
                    return Err(TopoError::Precondition(
                        "full-coverage face of an open surface",
                    ));
                }
            }
        } else {
            // Bounds from the pcurve polylines.
            let face = self.faces.get(fk).ok_or(TopoError::StaleKey)?;
            let mut lo = (f64::INFINITY, f64::INFINITY);
            let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
            for &lk in &face.loops {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                while let Some(fin) = self.fins.get(cur) {
                    if let Some((ck, _)) = fin.pcurve
                        && let Some(Curve3::Nurbs(n)) = self.curves.get(ck)
                    {
                        for t in [0.0, 1.0] {
                            let p = n.point(t);
                            lo = (lo.0.min(p.x), lo.1.min(p.y));
                            hi = (hi.0.max(p.x), hi.1.max(p.y));
                        }
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
            }
            if !(lo.0.is_finite() && hi.0.is_finite()) {
                return Err(TopoError::Precondition("curved face without pcurve bounds"));
            }
            ((lo.0, hi.0), (lo.1, hi.1))
        };
        let panels = 16usize;
        for iu in 0..panels {
            let ua = u0 + (u1 - u0) * iu as f64 / panels as f64;
            let ub = u0 + (u1 - u0) * (iu + 1) as f64 / panels as f64;
            for iv in 0..panels {
                let va = v0 + (v1 - v0) * iv as f64 / panels as f64;
                let vb = v0 + (v1 - v0) * (iv + 1) as f64 / panels as f64;
                for (xu, wu) in GL8_X.iter().zip(GL8_W) {
                    let u = 0.5 * (ua + ub) + 0.5 * (ub - ua) * xu;
                    for (xv, wv) in GL8_X.iter().zip(GL8_W) {
                        let v = 0.5 * (va + vb) + 0.5 * (vb - va) * xv;
                        let Ok(lg) = surf.local_geometry(u, v) else {
                            continue; // pole-adjacent node: measure zero
                        };
                        let n = lg.du.cross(lg.dv) * orient;
                        let w = wu * wv * 0.25 * (ub - ua) * (vb - va);
                        m.add(lg.point, n, w);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;

    fn frame() -> Frame3 {
        Frame3::from_z(Vec3::new(0.5, -1.0, 2.0), Vec3::new(0., 0., 1.)).unwrap()
    }

    #[test]
    fn block_mass_properties_exact() {
        let mut b = Body::new();
        b.block(Vec3::new(1., 2., 3.), 2.0, 3.0, 4.0).unwrap();
        let mp = b.mass_properties().unwrap();
        assert!((mp.volume - 24.0).abs() < 1e-10, "V = {}", mp.volume);
        assert!((mp.centroid - Vec3::new(2.0, 3.5, 5.0)).norm() < 1e-10);
        // Inertia of a box about its centroid: V/12 * (b^2 + c^2).
        let v = 24.0;
        assert!((mp.inertia[0][0] - v / 12.0 * (9.0 + 16.0)).abs() < 1e-8);
        assert!((mp.inertia[1][1] - v / 12.0 * (4.0 + 16.0)).abs() < 1e-8);
        assert!((mp.inertia[2][2] - v / 12.0 * (4.0 + 9.0)).abs() < 1e-8);
        assert!(mp.inertia[0][1].abs() < 1e-8);
    }

    #[test]
    fn sphere_mass_properties() {
        let mut b = Body::new();
        b.sphere(frame(), 2.0).unwrap();
        let mp = b.mass_properties().unwrap();
        let want = 4.0 / 3.0 * core::f64::consts::PI * 8.0;
        assert!(
            (mp.volume - want).abs() < 1e-9 * want,
            "V = {} want {want}",
            mp.volume
        );
        assert!((mp.centroid - Vec3::new(0.5, -1.0, 2.0)).norm() < 1e-9);
        let ixx = 0.4 * mp.volume * 4.0; // 2/5 V r^2
        assert!((mp.inertia[0][0] - ixx).abs() < 1e-6 * ixx);
    }

    #[test]
    fn cylinder_cone_torus_volumes() {
        let pi = core::f64::consts::PI;
        let mut b = Body::new();
        b.cylinder(frame(), 1.5, 3.0).unwrap();
        let mp = b.mass_properties().unwrap();
        let want = pi * 1.5 * 1.5 * 3.0;
        assert!(
            (mp.volume - want).abs() < 1e-9 * want,
            "cyl V = {}",
            mp.volume
        );
        // Izz about the axis: V r^2 / 2 (z axis through centroid).
        assert!((mp.inertia[2][2] - 0.5 * want * 2.25).abs() < 1e-6 * want);

        let mut b = Body::new();
        b.cone(frame(), 1.5, 3.0).unwrap();
        let mp = b.mass_properties().unwrap();
        let want = pi * 1.5 * 1.5 * 3.0 / 3.0;
        assert!(
            (mp.volume - want).abs() < 1e-9 * want,
            "cone V = {}",
            mp.volume
        );
        // Centroid at h/4 above the base.
        assert!(
            (mp.centroid.z - (2.0 + 0.75)).abs() < 1e-8,
            "{:?}",
            mp.centroid
        );

        let mut b = Body::new();
        b.torus(frame(), 3.0, 1.0).unwrap();
        let mp = b.mass_properties().unwrap();
        let want = 2.0 * pi * pi * 3.0 * 1.0;
        assert!(
            (mp.volume - want).abs() < 1e-9 * want,
            "torus V = {}",
            mp.volume
        );
        // Izz about the torus axis: V (R^2 + 3/4 r^2).
        let izz = want * (9.0 + 0.75);
        assert!((mp.inertia[2][2] - izz).abs() < 1e-6 * izz);
    }

    #[test]
    fn pentagon_prism_volume_matches_shoelace() {
        let mut b = Body::new();
        let profile: Vec<Vec3> = (0..5)
            .map(|i| {
                let a = core::f64::consts::TAU * i as f64 / 5.0;
                Vec3::new(2.0 * a.cos(), 2.0 * a.sin(), 0.0)
            })
            .collect();
        b.prism(&profile, Vec3::new(0., 0., 3.)).unwrap();
        let mp = b.mass_properties().unwrap();
        // Shoelace area of the pentagon.
        let mut area2 = 0.0;
        for i in 0..5 {
            let p = profile[i];
            let q = profile[(i + 1) % 5];
            area2 += p.x * q.y - q.x * p.y;
        }
        let want = 0.5 * area2.abs() * 3.0;
        assert!((mp.volume - want).abs() < 1e-9 * want, "V = {}", mp.volume);
    }
}
