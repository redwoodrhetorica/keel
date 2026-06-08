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
        let Some((sk, sense)) = self.faces.get(face).and_then(|f| f.surface) else {
            return Vec::new();
        };
        match self.surfaces.get(sk) {
            Some(crate::entity::SurfaceGeom::Analytic(surf)) => match surf {
                Surface3::Plane(p) => self.tessellate_planar(face, p.frame.z, sense),
                Surface3::Sphere(s) => {
                    self.tessellate_sphere(face, s.frame.origin, s.radius, sense)
                }
                Surface3::Cylinder(c) => self.tessellate_cylinder(face, &c.clone(), sense),
                _ => Vec::new(),
            },
            Some(crate::entity::SurfaceGeom::Nurbs(n)) => {
                self.tessellate_nurbs(face, &n.clone(), sense)
            }
            None => Vec::new(),
        }
    }

    /// Grid-tessellate a NURBS face over its parameter domain into
    /// outward-oriented triangles (outward = the surface `local_geometry`
    /// normal, sense-adjusted; the cell-center fallback handles
    /// parameterization singularities like sphere poles). Whole-surface
    /// faces only in M7a; trimmed NURBS fragments are M7a Task 4 /
    /// M7b.
    pub(crate) fn tessellate_nurbs(
        &self,
        face: FaceKey,
        nurbs: &keel_geom::nurbs_surface::NurbsSurface,
        sense: bool,
    ) -> Vec<[Vec3; 3]> {
        let ((u0, u1), (v0, v1)) = nurbs.domain();
        const NU: usize = 40;
        const NV: usize = 28;
        let sgn = if sense { 1.0 } else { -1.0 };
        // A trimmed cap fragment keeps only triangles on the cap side of
        // its bounding SSI circle plane (the NURBS analogue of the sphere
        // cap-side filter); a whole-surface face meshes fully.
        let cap = self.nurbs_cap_trim(face);
        let on_cap = |q: Vec3| -> bool {
            match cap {
                Some((cc, ax, side)) => ((q - cc).dot(ax) * side) >= 0.0,
                None => true,
            }
        };
        let mut tris = Vec::new();
        let pt = |i: usize, n: usize, lo: f64, hi: f64| lo + (hi - lo) * i as f64 / n as f64;
        for i in 0..NU {
            let ua = pt(i, NU, u0, u1);
            let ub = pt(i + 1, NU, u0, u1);
            for j in 0..NV {
                let va = pt(j, NV, v0, v1);
                let vb = pt(j + 1, NV, v0, v1);
                let a = nurbs.point(ua, va);
                let b = nurbs.point(ub, va);
                let c = nurbs.point(ub, vb);
                let d = nurbs.point(ua, vb);
                let (uc, vc) = (0.5 * (ua + ub), 0.5 * (va + vb));
                let outward = nurbs
                    .local_geometry(uc, vc)
                    .ok()
                    .map(|g| g.normal * sgn)
                    .unwrap_or_else(|| (b - a).cross(d - a) * sgn);
                for tri in [[a, b, c], [a, c, d]] {
                    let cen = (tri[0] + tri[1] + tri[2]) * (1.0 / 3.0);
                    if on_cap(cen) {
                        tris.push(orient(tri, outward));
                    }
                }
            }
        }
        tris
    }

    /// If a NURBS `face` is a cap trimmed by a CLOSED SSI circle, return
    /// the circle plane (point, unit normal) and the cap-side sign.
    fn nurbs_cap_trim(&self, face: FaceKey) -> Option<(Vec3, Vec3, f64)> {
        for lk in self.faces.get(face).map(|f| f.loops.clone())? {
            let entry = self.loops.get(lk).and_then(|l| l.fin)?;
            let mut cur = entry;
            loop {
                let fin = self.fins.get(cur)?;
                let closed = self.edges.get(fin.edge).map(|e| e.is_closed()) == Some(true);
                if closed
                    && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                    && let Some(cv) = self.curves.get(ck)
                    && let Some((center_c, ax)) = crate::boolean::closed_curve_center_axis(cv)
                {
                    let apex = self.face_interior_point(face)?;
                    let side = if (apex - center_c).dot(ax) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    return Some((center_c, ax, side));
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        None
    }

    /// Lat-band tessellate a cylindrical face. The axial band [hlo,hhi]
    /// is bounded by the face's CLOSED circle edges (the cap circles of
    /// the whole lateral, or the SSI + cap circles of a trimmed band);
    /// the lateral is full-wrap so no angular trim. Outward = radial.
    fn tessellate_cylinder(
        &self,
        face: FaceKey,
        cyl: &keel_geom::surface::Cylinder3,
        sense: bool,
    ) -> Vec<[Vec3; 3]> {
        let (origin, ex, ey, ez, radius) = (
            cyl.frame.origin,
            cyl.frame.x,
            cyl.frame.y,
            cyl.frame.z,
            cyl.radius,
        );
        // Axial band from the face's circle/arc edges (cap circles and
        // SSI arcs).
        let heights = self.cyl_circle_heights(face, origin, ez);
        if heights.len() < 2 {
            return Vec::new();
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if hhi - hlo <= 0.0 {
            return Vec::new();
        }
        const NV: usize = 16;
        const NP: usize = 64;
        let tau = core::f64::consts::TAU;
        let sgn = if sense { 1.0 } else { -1.0 };
        let pt = |phi: f64, v: f64| -> Vec3 {
            origin + (ex * phi.cos() + ey * phi.sin()) * radius + ez * v
        };
        let mut tris = Vec::new();
        for i in 0..NV {
            let v0 = hlo + (hhi - hlo) * i as f64 / NV as f64;
            let v1 = hlo + (hhi - hlo) * (i + 1) as f64 / NV as f64;
            for j in 0..NP {
                let p0 = tau * j as f64 / NP as f64;
                let p1 = tau * (j + 1) as f64 / NP as f64;
                let a = pt(p0, v0);
                let b = pt(p0, v1);
                let c = pt(p1, v1);
                let d = pt(p1, v0);
                let rad = |q: Vec3| -> Vec3 {
                    let w = q - origin;
                    (w - ez * w.dot(ez)) * sgn
                };
                tris.push(orient([a, b, c], rad((a + b + c) * (1.0 / 3.0))));
                tris.push(orient([a, c, d], rad((a + c + d) * (1.0 / 3.0))));
            }
        }
        tris
    }

    /// Triangulate a planar face by fanning each loop's boundary polygon
    /// (built by sampling the loop's edge CURVES, so circle- and
    /// NURBS-bounded discs work, not just straight polygons) from its
    /// centroid. The outer loop fans outward; inner-ring (hole) loops fan
    /// with reversed orientation so their solid-angle / volume
    /// contributions subtract the hole. Star-convex loops only (the M6c
    /// primitive caps and the box faces are).
    fn tessellate_planar(&self, face: FaceKey, nz: Vec3, sense: bool) -> Vec<[Vec3; 3]> {
        let outward = if sense { nz } else { nz * -1.0 };
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        let mut tris = Vec::new();
        for (li, lk) in loops.iter().enumerate() {
            let poly = self.loop_polygon(*lk);
            if poly.len() < 3 {
                continue;
            }
            let n = poly.len();
            let centroid = poly.iter().fold(Vec3::ZERO, |a, p| a + *p) * (1.0 / n as f64);
            // Inner rings (holes) get reversed orientation to subtract.
            let loop_out = if li == 0 { outward } else { outward * -1.0 };
            for i in 0..n {
                tris.push(orient([centroid, poly[i], poly[(i + 1) % n]], loop_out));
            }
        }
        tris
    }

    /// A loop's boundary polygon: its fins' start vertices for a straight
    /// polygon (box faces), or the boundary circle sampled into points
    /// for a single closed-circle loop (disc caps, holes).
    fn loop_polygon(&self, lk: crate::entity::LoopKey) -> Vec<Vec3> {
        use keel_geom::curve::Curve3;
        let mut verts: Vec<Vec3> = Vec::new();
        let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
            return verts;
        };
        let mut cur = entry;
        let mut circle_edge = None;
        loop {
            if let Some(p) = self
                .fin_start_vertex(cur)
                .and_then(|v| self.vertices.get(v))
                .map(|v| v.point)
                && verts.last().map(|q| (*q - p).norm() > 1e-9).unwrap_or(true)
            {
                verts.push(p);
            }
            if let Some(fin) = self.fins.get(cur)
                && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                && let Some(Curve3::Circle(c)) = self.curves.get(ck)
            {
                circle_edge = Some(*c);
            }
            let Some(next) = self.fins.get(cur).map(|f| f.next) else {
                break;
            };
            cur = next;
            if cur == entry {
                break;
            }
        }
        if verts.len() >= 3 {
            return verts;
        }
        // Degenerate vertex polygon (a disc bounded by one closed
        // circle): sample the circle.
        if let Some(c) = circle_edge {
            const N: usize = 32;
            return (0..N)
                .map(|i| c.point(core::f64::consts::TAU * i as f64 / N as f64))
                .collect();
        }
        verts
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
        // If the face is a trimmed cap (bounded by an SSI circle),
        // keep only triangles on the cap side of that circle's plane;
        // a whole-sphere face (no circle edge) meshes fully.
        let cap = self.sphere_cap_trim(face);
        // theta in [0, pi] (polar), phi in [0, 2pi). Coarse grid.
        const NT: usize = 40;
        const NP: usize = 60;
        let tau = core::f64::consts::TAU;
        let pi = core::f64::consts::PI;
        let mut tris = Vec::new();
        let sgn = if sense { 1.0 } else { -1.0 };
        let on_cap = |q: Vec3| -> bool {
            match cap {
                Some((cc, ax, side)) => ((q - cc).dot(ax) * side) >= 0.0,
                None => true,
            }
        };
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
                for tri in [[a, b, c], [a, c, d]] {
                    let cen = (tri[0] + tri[1] + tri[2]) * (1.0 / 3.0);
                    if on_cap(cen) {
                        tris.push(orient(tri, (cen - center) * sgn));
                    }
                }
            }
        }
        tris
    }

    /// If `face` is a spherical cap trimmed by an SSI circle, return the
    /// circle plane (point on it, unit normal) and the sign such that
    /// `(q - point).dot(normal) * sign >= 0` selects the cap side.
    fn sphere_cap_trim(&self, face: FaceKey) -> Option<(Vec3, Vec3, f64)> {
        let loops = self.faces.get(face).map(|f| f.loops.clone())?;
        for lk in loops {
            let entry = self.loops.get(lk).and_then(|l| l.fin)?;
            let mut cur = entry;
            loop {
                let fin = self.fins.get(cur)?;
                // Only a CLOSED circle edge is an SSI seam; the sphere's
                // own meridian is an open pole-to-pole circle and must
                // not be mistaken for a cap trim.
                let closed = self.edges.get(fin.edge).map(|e| e.is_closed()) == Some(true);
                if closed
                    && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                    && let Some(cv) = self.curves.get(ck)
                    && let Some((center_c, ax)) = crate::boolean::closed_curve_center_axis(cv)
                {
                    let apex = self.face_interior_point(face)?;
                    let side = if (apex - center_c).dot(ax) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    return Some((center_c, ax, side));
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        None
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
