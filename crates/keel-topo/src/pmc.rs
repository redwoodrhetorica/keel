//! Point membership classification (M4 Tasks 4-5; gate design and the
//! 2026-06-07 PMC performance research).
//!
//! Parameter-space containment uses WINDING NUMBERS over pcurve
//! polylines (robust interface for M5 trims; Liu et al. 2025). Faces
//! covering their whole closed surface (every boundary edge is a seam:
//! all radial fins owned by this face) are trivially In everywhere,
//! which is exactly how the sphere and torus primitive faces behave;
//! this sidesteps the genus-1 universal-cover bookkeeping until real
//! trims exist. Loops over collapsed boundaries (cone apex) close
//! implicitly in the winding accumulation.
//!
//! 3D classification: nearest-face signed classification is PRIMARY
//! (project, contain the foot, read the sign against the face normal,
//! name the region from the face's region links). The deterministic
//! ray ladder survives as the degenerate-incidence fallback. No random
//! perturbation, ever (determinism contract).

use crate::body::{Body, TopoError};
use crate::entity::{CurveGeom, FaceKey, FinKey, RegionKey, SurfaceGeom};
use keel_geom::curve::{Curve3, Domain, Line3};
use keel_geom::intersect::{SurfaceRef, intersect_curve_surface};
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::surface::Surface3;
use keel_math::vec::Vec3;

/// Result of classifying a point against a body.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Containment {
    /// Inside the given region (never the infinite region).
    In(RegionKey),
    /// Within tolerance of the given face.
    On(FaceKey),
    /// In the infinite region.
    Out,
}

/// Parameter-space verdict for a (u, v) against a face's loops.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UvClass {
    In,
    OnBoundary,
    Out,
}

/// Samples per pcurve when polylining for the winding test.
const PCURVE_SAMPLES: usize = 16;

impl Body {
    /// Attach a straight-segment pcurve (degree-1 NURBS in (u, v, 0)
    /// parameter space) to a fin. Convention: pcurves live in the
    /// face's unwrapped parameter cover.
    pub fn attach_pcurve_segment(&mut self, fin: FinKey, uv0: (f64, f64), uv1: (f64, f64)) {
        let Ok(seg) = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![Vec3::new(uv0.0, uv0.1, 0.0), Vec3::new(uv1.0, uv1.1, 0.0)],
            None,
        ) else {
            return;
        };
        let key = self.add_curve(Curve3::Nurbs(seg));
        if let Some(f) = self.fins.get_mut(fin) {
            f.pcurve = Some((key, true));
        }
    }

    /// Attach an arbitrary pcurve (e.g. a full circle in UV for a
    /// planar cap's rim).
    pub fn attach_pcurve(&mut self, fin: FinKey, curve: CurveGeom, sense: bool) {
        let key = self.add_curve(curve);
        if let Some(f) = self.fins.get_mut(fin) {
            f.pcurve = Some((key, sense));
        }
    }

    /// True when every edge used by this face's loops has its ENTIRE
    /// radial cycle owned by this face: the face covers its whole
    /// closed surface (sphere, torus primitives).
    pub fn face_covers_closed_surface(&self, face: FaceKey) -> bool {
        let Some(f) = self.faces.get(face) else {
            return false;
        };
        let mut any_edge = false;
        for &lk in &f.loops {
            let Some(l) = self.loops.get(lk) else {
                return false;
            };
            let Some(entry) = l.fin else { continue };
            let mut cur = entry;
            loop {
                let Some(fin) = self.fins.get(cur) else {
                    return false;
                };
                any_edge = true;
                let Some(edge) = self.edges.get(fin.edge) else {
                    return false;
                };
                // A genuinely closed surface has no boundary: every edge
                // is an interior seam appearing TWICE in this face. A
                // DANGLING rim (radial-1, e.g. an unglued SSI arc on a
                // kept sphere piece) is a boundary, and promoting such a
                // face to full closed-surface integration produced the
                // socket-carve wrong volume.
                if edge.radial.len() < 2 {
                    return false;
                }
                for &rf in &edge.radial {
                    let owner_face = self
                        .fins
                        .get(rf)
                        .and_then(|x| self.loops.get(x.owner))
                        .map(|x| x.face);
                    if owner_face != Some(face) {
                        return false;
                    }
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        any_edge
    }

    /// Polyline of a loop's pcurves in the parameter cover, in loop
    /// order. Gaps between consecutive fins (collapsed boundaries like
    /// the cone apex) and the final closure participate implicitly in
    /// the winding sum.
    fn loop_uv_polyline(&self, entry: FinKey) -> Vec<(f64, f64)> {
        let mut pts = Vec::new();
        let mut cur = entry;
        while let Some(fin) = self.fins.get(cur) {
            if let Some((ck, sense)) = fin.pcurve
                && let Some(c) = self.curves.get(ck)
            {
                for i in 0..PCURVE_SAMPLES {
                    let s = i as f64 / PCURVE_SAMPLES as f64;
                    let s = if sense { s } else { 1.0 - s };
                    let p = match c {
                        Curve3::Nurbs(n) => {
                            let (a, b) = n.domain();
                            n.point(a + s * (b - a))
                        }
                        Curve3::Circle(ci) => ci.point(core::f64::consts::TAU * s),
                        Curve3::Line(l) => l.point(s),
                        Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * s),
                    };
                    pts.push((p.x, p.y));
                }
            }
            cur = fin.next;
            if cur == entry {
                break;
            }
        }
        pts
    }

    /// Winding-number containment of (u, v) against a face's loops.
    /// `ptol` is the parameter-space boundary tolerance.
    pub fn point_in_face_uv(&self, face: FaceKey, uv: (f64, f64), ptol: f64) -> UvClass {
        if self.face_covers_closed_surface(face) {
            return UvClass::In;
        }
        let Some(f) = self.faces.get(face) else {
            return UvClass::Out;
        };
        let mut inside = false;
        for (i, &lk) in f.loops.iter().enumerate() {
            let Some(l) = self.loops.get(lk) else {
                return UvClass::Out;
            };
            let Some(entry) = l.fin else { continue };
            let poly = self.loop_uv_polyline(entry);
            if poly.len() < 2 {
                continue;
            }
            if near_polyline(&poly, uv, ptol) {
                return UvClass::OnBoundary;
            }
            let w = winding_number(&poly, uv);
            if i == 0 {
                // Outer loop: inside iff it winds around the point.
                inside = w != 0;
            } else if w != 0 {
                // Inside an inner ring: outside the face.
                return UvClass::Out;
            }
        }
        if inside { UvClass::In } else { UvClass::Out }
    }
}

/// Integer winding number of a closed polyline about a point
/// (signed angle accumulation, rounded; orientation-agnostic callers
/// compare against zero).
fn winding_number(poly: &[(f64, f64)], q: (f64, f64)) -> i32 {
    let mut total = 0.0f64;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let va = (a.0 - q.0, a.1 - q.1);
        let vb = (b.0 - q.0, b.1 - q.1);
        let cross = va.0 * vb.1 - va.1 * vb.0;
        let dot = va.0 * vb.0 + va.1 * vb.1;
        total += cross.atan2(dot);
    }
    (total / core::f64::consts::TAU).round() as i32
}

fn near_polyline(poly: &[(f64, f64)], q: (f64, f64), tol: f64) -> bool {
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let (abx, aby) = (b.0 - a.0, b.1 - a.1);
        let len2 = abx * abx + aby * aby;
        let t = if len2 > 0.0 {
            (((q.0 - a.0) * abx + (q.1 - a.1) * aby) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (cx, cy) = (a.0 + t * abx - q.0, a.1 + t * aby - q.1);
        if (cx * cx + cy * cy).sqrt() <= tol {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------
// 3D classification

/// Deterministic ray ladder (never random; determinism contract).
const RAY_LADDER: [[f64; 3]; 6] = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.7396, 0.5293, 0.4156],
    [-0.4276, 0.8133, 0.3946],
    [0.3061, -0.4882, 0.8172],
];

impl Body {
    /// Classify a point against the body's regions.
    pub fn classify_point(&self, p: Vec3) -> Result<Containment, TopoError> {
        if !p.is_finite() {
            return Err(TopoError::Precondition("classify_point: non-finite"));
        }
        // Nearest-face pass: project onto every face surface.
        struct Foot {
            face: FaceKey,
            dist: f64,
            uv: (f64, f64),
            class: UvClass,
            tol: f64,
        }
        let mut feet: Vec<Foot> = Vec::new();
        let faces: Vec<FaceKey> = self
            .entity_ids()
            .filter_map(|id| match self.lookup(id) {
                Some(crate::entity::AnyKey::Face(k)) => Some(k),
                _ => None,
            })
            .collect();
        for fk in &faces {
            let Some(face) = self.faces.get(*fk) else {
                continue;
            };
            let Some((sk, _)) = face.surface else {
                continue;
            };
            let Some(SurfaceGeom::Analytic(surf)) = self.surfaces.get(sk) else {
                continue; // NURBS faces arrive with M5 trims
            };
            let Ok(pr) = surf.project(p) else { continue };
            let ptol = uv_tolerance(surf, (pr.u, pr.v), face_tolerance(self, *fk));
            let class = self.point_in_face_uv(*fk, (pr.u, pr.v), ptol);
            feet.push(Foot {
                face: *fk,
                dist: pr.distance,
                uv: (pr.u, pr.v),
                class,
                tol: face_tolerance(self, *fk),
            });
        }
        if feet.is_empty() {
            return Ok(Containment::Out);
        }
        feet.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        // On verdict: within tolerance of a face whose foot is on it.
        if let Some(f) = feet
            .iter()
            .find(|f| f.dist <= f.tol && f.class != UvClass::Out)
        {
            return Ok(Containment::On(f.face));
        }
        // Primary: nearest face with an INTERIOR foot decides by sign.
        if let Some(f) = feet.iter().find(|f| f.class == UvClass::In) {
            // Only trust it if no boundary foot is meaningfully closer
            // (a nearer edge foot means local information is ambiguous).
            let nearest_boundary = feet
                .iter()
                .filter(|x| x.class == UvClass::OnBoundary)
                .map(|x| x.dist)
                .fold(f64::INFINITY, f64::min);
            if f.dist <= nearest_boundary {
                let Some(face) = self.faces.get(f.face) else {
                    return Err(TopoError::StaleKey);
                };
                let Some((sk, _)) = face.surface else {
                    return Err(TopoError::StaleKey);
                };
                let Some(SurfaceGeom::Analytic(surf)) = self.surfaces.get(sk) else {
                    return Err(TopoError::StaleKey);
                };
                // A Degenerate foot (pole/apex parameterization
                // singularity) falls through to the ray ladder.
                if let Ok(lg) = surf.local_geometry(f.uv.0, f.uv.1) {
                    let side = (p - lg.point).dot(lg.normal);
                    let region = if side >= 0.0 {
                        face.front_region
                    } else {
                        face.back_region
                    };
                    return Ok(self.region_verdict(region));
                }
            }
        }
        // Fallback: deterministic ray ladder.
        self.classify_by_ray(p, &faces)
    }

    fn region_verdict(&self, region: RegionKey) -> Containment {
        match self.regions.get(region) {
            Some(r) if r.infinite => Containment::Out,
            Some(_) => Containment::In(region),
            None => Containment::Out,
        }
    }

    fn classify_by_ray(&self, p: Vec3, faces: &[FaceKey]) -> Result<Containment, TopoError> {
        'ladder: for dir in RAY_LADDER {
            let d = Vec3::new(dir[0], dir[1], dir[2]);
            let Ok(line) = Line3::new(p, d) else { continue };
            let ray = Curve3::Line(line);
            let mut first: Option<(f64, FaceKey, Vec3)> = None;
            for fk in faces {
                let Some(face) = self.faces.get(*fk) else {
                    continue;
                };
                let Some((sk, _)) = face.surface else {
                    continue;
                };
                let Some(SurfaceGeom::Analytic(surf)) = self.surfaces.get(sk) else {
                    continue;
                };
                let tol = face_tolerance(self, *fk);
                let hits = match intersect_curve_surface(
                    &ray,
                    Domain::Finite {
                        a: 10.0 * tol,
                        b: 1e12,
                    },
                    &SurfaceRef::Analytic(surf),
                    tol,
                ) {
                    Ok(h) => h,
                    Err(_) => continue 'ladder, // degenerate: next rung
                };
                for h in hits {
                    if h.tangential {
                        continue 'ladder;
                    }
                    let ptol = uv_tolerance(surf, h.uv, tol);
                    match self.point_in_face_uv(*fk, h.uv, ptol) {
                        UvClass::In => {
                            if first.as_ref().is_none_or(|(t, _, _)| h.t < *t) {
                                let n = match surf.local_geometry(h.uv.0, h.uv.1) {
                                    Ok(lg) => lg.normal,
                                    Err(_) => continue 'ladder,
                                };
                                first = Some((h.t, *fk, n));
                            }
                        }
                        UvClass::OnBoundary => continue 'ladder,
                        UvClass::Out => {}
                    }
                }
            }
            return Ok(match first {
                None => Containment::Out,
                Some((_, fk, normal)) => {
                    let Some(face) = self.faces.get(fk) else {
                        return Err(TopoError::StaleKey);
                    };
                    let along = d.dot(normal);
                    if along.abs() <= 1e-12 {
                        continue 'ladder;
                    }
                    // Travelling WITH the normal exits through Front:
                    // p sits on the Back side, and vice versa.
                    let region = if along > 0.0 {
                        face.back_region
                    } else {
                        face.front_region
                    };
                    self.region_verdict(region)
                }
            });
        }
        Err(TopoError::Precondition(
            "classify_point: ray ladder exhausted",
        ))
    }
}

fn face_tolerance(b: &Body, fk: FaceKey) -> f64 {
    // Per-entity face tolerance lands when faces carry one; until then
    // the session floor via vertex defaults is the body-wide answer.
    let _ = (b, fk);
    1e-8
}

/// Map a spatial tolerance to parameter space via the first
/// fundamental form scale at (u, v).
fn uv_tolerance(s: &Surface3, uv: (f64, f64), tol: f64) -> f64 {
    match s.local_geometry(uv.0, uv.1) {
        Ok(lg) => {
            let scale = lg.e.sqrt().max(lg.g.sqrt()).max(1e-12);
            (tol / scale).max(1e-12)
        }
        Err(_) => 1e-9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;

    fn frame() -> Frame3 {
        Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap()
    }

    #[test]
    fn classify_against_all_primitives() {
        // Block.
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 2., 2., 2.).unwrap();
        let _ = out;
        assert!(matches!(
            b.classify_point(Vec3::new(1., 1., 1.)).unwrap(),
            Containment::In(_)
        ));
        assert_eq!(
            b.classify_point(Vec3::new(5., 5., 5.)).unwrap(),
            Containment::Out
        );
        assert!(matches!(
            b.classify_point(Vec3::new(2., 1., 1.)).unwrap(),
            Containment::On(_)
        ));

        // Cylinder.
        let mut b = Body::new();
        b.cylinder(frame(), 1.5, 3.0).unwrap();
        assert!(matches!(
            b.classify_point(Vec3::new(0.3, 0.2, 1.0)).unwrap(),
            Containment::In(_)
        ));
        assert_eq!(
            b.classify_point(Vec3::new(3., 0., 1.)).unwrap(),
            Containment::Out
        );
        assert_eq!(
            b.classify_point(Vec3::new(0., 0., 9.)).unwrap(),
            Containment::Out
        );
        assert!(matches!(
            b.classify_point(Vec3::new(1.5, 0., 1.)).unwrap(),
            Containment::On(_)
        ));

        // Cone (base r=1.5 at z=0, apex at z=3).
        let mut b = Body::new();
        b.cone(frame(), 1.5, 3.0).unwrap();
        assert!(matches!(
            b.classify_point(Vec3::new(0., 0., 1.0)).unwrap(),
            Containment::In(_)
        ));
        assert_eq!(
            b.classify_point(Vec3::new(1.4, 0., 2.5)).unwrap(),
            Containment::Out
        );

        // Sphere.
        let mut b = Body::new();
        b.sphere(frame(), 2.0).unwrap();
        assert!(matches!(
            b.classify_point(Vec3::new(0.5, -0.3, 0.8)).unwrap(),
            Containment::In(_)
        ));
        assert_eq!(
            b.classify_point(Vec3::new(0., 0., 3.)).unwrap(),
            Containment::Out
        );
        assert!(matches!(
            b.classify_point(Vec3::new(2., 0., 0.)).unwrap(),
            Containment::On(_)
        ));

        // Torus (R=3, r=1).
        let mut b = Body::new();
        b.torus(frame(), 3.0, 1.0).unwrap();
        assert!(matches!(
            b.classify_point(Vec3::new(3., 0., 0.)).unwrap(),
            Containment::In(_)
        ));
        assert_eq!(b.classify_point(Vec3::ZERO).unwrap(), Containment::Out);
        assert_eq!(
            b.classify_point(Vec3::new(0., 0., 5.)).unwrap(),
            Containment::Out
        );
    }

    #[test]
    fn uv_containment_on_cylinder_lateral() {
        let mut b = Body::new();
        let out = b.cylinder(frame(), 1.0, 2.0).unwrap();
        let lateral = out.faces[0];
        assert!(!b.face_covers_closed_surface(lateral));
        assert_eq!(b.point_in_face_uv(lateral, (1.0, 1.0), 1e-9), UvClass::In);
        assert_eq!(b.point_in_face_uv(lateral, (1.0, 5.0), 1e-9), UvClass::Out);
        assert_eq!(
            b.point_in_face_uv(lateral, (1.0, 0.0), 1e-9),
            UvClass::OnBoundary
        );
    }

    #[test]
    fn sphere_and_torus_faces_cover_closed_surfaces() {
        let mut b = Body::new();
        let s = b.sphere(frame(), 1.0).unwrap();
        assert!(b.face_covers_closed_surface(s.faces[0]));
        let mut b = Body::new();
        let t = b.torus(frame(), 3.0, 1.0).unwrap();
        assert!(b.face_covers_closed_surface(t.faces[0]));
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig {
            cases: 64, ..proptest::prelude::ProptestConfig::default()
        })]
        // Oracle: classification agrees with the implicit sign for the
        // sphere away from the tolerance band.
        #[test]
        fn sphere_classification_matches_implicit_sign(
            px in -3.0..3.0f64, py in -3.0..3.0f64, pz in -3.0..3.0f64,
        ) {
            let p = Vec3::new(px, py, pz);
            let r = p.norm();
            proptest::prop_assume!((r - 2.0).abs() > 1e-6);
            let mut b = Body::new();
            b.sphere(frame(), 2.0).unwrap();
            let got = b.classify_point(p).unwrap();
            if r < 2.0 {
                proptest::prop_assert!(matches!(got, Containment::In(_)), "{got:?} at {p:?}");
            } else {
                proptest::prop_assert!(got == Containment::Out, "{got:?} at {p:?}");
            }
        }
    }
}
