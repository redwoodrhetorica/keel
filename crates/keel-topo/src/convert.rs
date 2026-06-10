//! Body-level NURBS conversion for export (parity item 137; dossier 25
//! sec 22). Face geometry converts to its EXACT NURBS form via
//! `keel_geom::convert`, with the parameter bounds derived from the
//! face's own boundary; NURBS faces pass through unchanged. Together
//! with `Body::simplify` (M8, NURBS -> analytic) this closes the
//! conversion loop in both directions.

use crate::body::Body;
use crate::entity::{FaceKey, SurfaceGeom};
use keel_geom::convert::{
    cone_to_nurbs, cylinder_to_nurbs, plane_to_nurbs, sphere_to_nurbs, torus_to_nurbs,
};
use keel_geom::nurbs_surface::NurbsSurface;
use keel_geom::surface::Surface3;

impl Body {
    /// Convert a face's surface to NURBS for export (item 137). Exact
    /// for every analytic kind: plane = bilinear patch over the
    /// outer-loop frame bounding box; cylinder/cone = full-circumference
    /// band over the face's height range; sphere/torus = the full
    /// closed revolved surface. `None` for a face without geometry or
    /// with a degenerate boundary.
    pub fn face_to_nurbs(&self, face: FaceKey) -> Option<NurbsSurface> {
        let geom = self.face_surface_geom(face)?;
        let s = match geom {
            SurfaceGeom::Nurbs(n) => return Some(n),
            SurfaceGeom::Analytic(s) => s,
        };
        // Boundary samples across ALL loops (curved edges sampled), plus
        // the raw loop vertices: loop_polygon's closed-circle fallback
        // returns only ONE rim's samples for a two-vertex barrel loop,
        // which would lose the other rim's height.
        let loops = self.faces.get(face).map(|f| f.loops.clone())?;
        let mut pts = Vec::new();
        for lk in loops {
            pts.extend(self.loop_polygon(lk));
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            for _ in 0..self.fins.len() + 1 {
                if let Some(p) = self
                    .fin_start_vertex(cur)
                    .and_then(|v| self.vertices.get(v))
                    .map(|v| v.point)
                {
                    pts.push(p);
                }
                let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                    break;
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
        }
        match s {
            Surface3::Plane(p) => {
                if pts.len() < 3 {
                    return None;
                }
                let (mut u0, mut u1) = (f64::INFINITY, f64::NEG_INFINITY);
                let (mut v0, mut v1) = (f64::INFINITY, f64::NEG_INFINITY);
                for q in &pts {
                    let w = *q - p.frame.origin;
                    let (u, v) = (w.dot(p.frame.x), w.dot(p.frame.y));
                    u0 = u0.min(u);
                    u1 = u1.max(u);
                    v0 = v0.min(v);
                    v1 = v1.max(v);
                }
                plane_to_nurbs(&p, (u0, u1), (v0, v1)).ok()
            }
            Surface3::Cylinder(c) => {
                let (h0, h1) = height_range(&pts, c.frame.origin, c.frame.z)?;
                cylinder_to_nurbs(&c, h0, h1).ok()
            }
            Surface3::Cone(c) => {
                let (h0, h1) = height_range(&pts, c.frame.origin, c.frame.z)?;
                cone_to_nurbs(&c, h0, h1).ok()
            }
            Surface3::Sphere(sp) => sphere_to_nurbs(&sp).ok(),
            Surface3::Torus(t) => torus_to_nurbs(&t).ok(),
        }
    }
}

/// Min/max height of the samples along `axis` through `origin`; `None`
/// when degenerate (a band needs extent).
fn height_range(
    pts: &[keel_math::vec::Vec3],
    origin: keel_math::vec::Vec3,
    axis: keel_math::vec::Vec3,
) -> Option<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for p in pts {
        let h = (*p - origin).dot(axis);
        lo = lo.min(h);
        hi = hi.max(h);
    }
    (hi - lo > 1e-12).then_some((lo, hi))
}

#[cfg(test)]
mod tests {
    use keel_geom::surface::Frame3;
    use keel_math::vec::Vec3;

    use crate::Body;
    use crate::entity::SurfaceGeom;

    #[test]
    fn block_face_converts_to_exact_bilinear_patch() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        for face in b.face_keys() {
            let n = b.face_to_nurbs(face).expect("plane converts");
            // Every patch corner lies on the face's plane and the patch
            // reproduces the surface: spot-check the residual to the
            // analytic plane at the patch midpoint.
            let surf = b.face_surface3(face).unwrap();
            let ((u0, u1), (v0, v1)) = n.domain();
            let p = n.point(0.5 * (u0 + u1), 0.5 * (v0 + v1));
            let pr = surf.project(p).unwrap();
            assert!(pr.distance < 1e-12, "midpoint off-plane {}", pr.distance);
        }
    }

    #[test]
    fn cylinder_face_converts_and_recovers_round_trip() {
        // Item 137 forward + M8 recover backward: barrel -> exact NURBS
        // band -> recover_surface certifies a cylinder again.
        let mut b = Body::new();
        b.cylinder(
            Frame3::from_z(Vec3::new(1.0, 1.0, 0.0), Vec3::new(0., 0., 1.)).unwrap(),
            1.5,
            3.0,
        )
        .unwrap();
        let barrel = b
            .face_keys()
            .into_iter()
            .find(|&f| {
                matches!(
                    b.face_surface3(f),
                    Some(keel_geom::surface::Surface3::Cylinder(_))
                )
            })
            .expect("cylinder body has a barrel face");
        let n = b.face_to_nurbs(barrel).expect("barrel converts");
        // Exact band: radius residual at samples ~ 0.
        let ((u0, u1), (v0, v1)) = n.domain();
        for i in 0..=8 {
            for j in 0..=8 {
                let p = n.point(
                    u0 + (u1 - u0) * i as f64 / 8.0,
                    v0 + (v1 - v0) * j as f64 / 8.0,
                );
                let r = ((p.x - 1.0).powi(2) + (p.y - 1.0).powi(2)).sqrt();
                assert!((r - 1.5).abs() < 1e-12, "band residual {r}");
                assert!((-1e-12..=3.0 + 1e-12).contains(&p.z), "band height {}", p.z);
            }
        }
        let rec = keel_geom::recover::recover_surface(&n, 1e-9).expect("band recovers");
        assert!(
            matches!(rec.surface, keel_geom::surface::Surface3::Cylinder(_)),
            "round-trip must recover the cylinder"
        );
    }

    #[test]
    fn nurbs_face_passes_through() {
        let mut b = Body::new();
        b.nurbs_sphere(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
        )
        .unwrap();
        let face = b.face_keys()[0];
        let Some(SurfaceGeom::Nurbs(want)) = b.face_surface_geom(face) else {
            panic!("nurbs face expected");
        };
        let got = b.face_to_nurbs(face).unwrap();
        assert_eq!(got, want, "NURBS face must pass through unchanged");
    }
}
