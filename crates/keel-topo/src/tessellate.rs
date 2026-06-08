//! Coarse boundary tessellation of analytic faces into OUTWARD-oriented
//! triangles, for the generalized winding number (M6b). This is an
//! internal classification aid, not a user-facing facet product: it
//! needs only enough fidelity that the summed solid angle of points
//! well off the surface is robustly ~1 (inside) or ~0 (outside).
//!
//! M6b covers planar and spherical faces (box + sphere booleans).
//! Cylinder/cone/torus tessellation is M6c (block-cylinder booleans).

use crate::body::Body;
use crate::entity::FaceKey;
use keel_geom::surface::Surface3;
use keel_math::vec::Vec3;

impl Body {
    /// Outward-oriented triangles covering a face's trimmed region.
    /// Empty for unsupported (non-planar/non-spherical) faces in M6b.
    pub(crate) fn tessellate_face(&self, face: FaceKey) -> Vec<[Vec3; 3]> {
        let Some(surf) = self.face_surface3(face) else {
            return Vec::new();
        };
        let sense = self
            .faces
            .get(face)
            .and_then(|f| f.surface)
            .map(|(_, s)| s)
            .unwrap_or(true);
        match surf {
            Surface3::Plane(p) => self.tessellate_planar(face, p.frame.z, sense),
            Surface3::Sphere(s) => self.tessellate_sphere(face, s.frame.origin, s.radius, sense),
            _ => Vec::new(),
        }
    }

    /// Fan-triangulate a planar face's outer-loop polygon, oriented so
    /// each triangle normal points along the face's outward normal
    /// (`frame.z` adjusted by `sense`). Holes (inner loops) are ignored
    /// in M6b (the proof primitives have none).
    fn tessellate_planar(&self, face: FaceKey, nz: Vec3, sense: bool) -> Vec<[Vec3; 3]> {
        let ring = self.face_ring_points(face);
        if ring.len() < 3 {
            return Vec::new();
        }
        let outward = if sense { nz } else { nz * -1.0 };
        let mut tris = Vec::new();
        for i in 1..ring.len() - 1 {
            tris.push(orient([ring[0], ring[i], ring[i + 1]], outward));
        }
        tris
    }

    /// Lat-long tessellate a spherical face. When the face covers the
    /// whole sphere (the primitive), the full surface is meshed; a
    /// trimmed cap is meshed over its UV box (Task 4). Outward = radial.
    fn tessellate_sphere(
        &self,
        face: FaceKey,
        center: Vec3,
        radius: f64,
        sense: bool,
    ) -> Vec<[Vec3; 3]> {
        let Some(Surface3::Sphere(s)) = self.face_surface3(face) else {
            return Vec::new();
        };
        let (ex, ey, ez) = (s.frame.x, s.frame.y, s.frame.z);
        let pt = |theta: f64, phi: f64| -> Vec3 {
            center
                + (ex * (theta.sin() * phi.cos())
                    + ey * (theta.sin() * phi.sin())
                    + ez * theta.cos())
                    * radius
        };
        // theta in [0, pi] (polar), phi in [0, 2pi). Coarse grid.
        const NT: usize = 18;
        const NP: usize = 28;
        let tau = core::f64::consts::TAU;
        let pi = core::f64::consts::PI;
        let mut tris = Vec::new();
        let sgn = if sense { 1.0 } else { -1.0 };
        for i in 0..NT {
            let t0 = pi * i as f64 / NT as f64;
            let t1 = pi * (i + 1) as f64 / NT as f64;
            for j in 0..NP {
                let p0 = tau * j as f64 / NP as f64;
                let p1 = tau * (j + 1) as f64 / NP as f64;
                let a = pt(t0, p0);
                let b = pt(t1, p0);
                let c = pt(t1, p1);
                let d = pt(t0, p1);
                // Outward at each quad ~ radial from center.
                let out0 = (a - center) * sgn;
                let out1 = (c - center) * sgn;
                tris.push(orient([a, b, c], out0));
                tris.push(orient([a, c, d], out1));
            }
        }
        tris
    }
}

/// Order a triangle's vertices so its geometric normal points along
/// `outward`.
fn orient(tri: [Vec3; 3], outward: Vec3) -> [Vec3; 3] {
    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
    if n.dot(outward) < 0.0 {
        [tri[0], tri[2], tri[1]]
    } else {
        tri
    }
}
