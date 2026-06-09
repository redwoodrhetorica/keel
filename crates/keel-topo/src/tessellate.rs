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

/// Number of chord segments to span an `span`-radian arc of radius
/// `radius` so the chord deviates from the arc by at most `tol`
/// (parity item 98, adaptive tessellation). `None` -> the fixed
/// `default` (the legacy density, so the default tessellation -- and the
/// volume oracle -- is unchanged). The chord error of one step d(phi) is
/// radius*(1 - cos(d(phi)/2)) ~ radius*d(phi)^2/8, giving
/// n >= span * sqrt(radius / (8*tol)).
fn arc_segments(span: f64, radius: f64, tol: Option<f64>, default: usize) -> usize {
    match tol {
        Some(t) if t > 0.0 && radius > 0.0 && span > 0.0 => {
            let n = (span.abs() * (radius / (8.0 * t)).sqrt()).ceil() as usize;
            n.clamp(8, 4096)
        }
        _ => default,
    }
}

impl Body {
    /// Outward-oriented triangles covering a face's trimmed region.
    /// Empty for unsupported (non-planar/non-spherical) faces in M6b.
    pub(crate) fn tessellate_face(&self, face: FaceKey) -> Vec<[Vec3; 3]> {
        self.tessellate_face_opt(face, None)
    }

    /// Like `tessellate_face`, but tessellate curved analytic faces to a
    /// chord tolerance `tol` (parity item 98). The default-density path
    /// (`tessellate_face`) is unchanged, so the winding/volume oracle is
    /// untouched.
    pub(crate) fn tessellate_face_tol(&self, face: FaceKey, tol: f64) -> Vec<[Vec3; 3]> {
        self.tessellate_face_opt(face, Some(tol))
    }

    fn tessellate_face_opt(&self, face: FaceKey, tol: Option<f64>) -> Vec<[Vec3; 3]> {
        let Some((sk, sense)) = self.faces.get(face).and_then(|f| f.surface) else {
            return Vec::new();
        };
        match self.surfaces.get(sk) {
            Some(crate::entity::SurfaceGeom::Analytic(surf)) => match surf {
                Surface3::Plane(p) => self.tessellate_planar(face, p.frame.z, sense),
                Surface3::Sphere(s) => {
                    self.tessellate_sphere(face, s.frame.origin, s.radius, sense, tol)
                }
                Surface3::Cylinder(c) => self.tessellate_cylinder(face, &c.clone(), sense, tol),
                Surface3::Cone(c) => self.tessellate_cone(face, &c.clone(), sense, tol),
                Surface3::Torus(t) => self.tessellate_torus(face, &t.clone(), sense, tol),
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

    /// The tube-angle [v_lo, v_hi] span a torus face occupies. The full
    /// tube (0, TAU) when the face covers its whole closed surface (the
    /// torus primitive); otherwise the min/max boundary-vertex tube angle
    /// (a partial-tube patch such as a fillet's quarter-torus ring).
    fn torus_tube_span(&self, face: FaceKey, t: &keel_geom::surface::Torus3) -> (f64, f64) {
        let tau = core::f64::consts::TAU;
        if self.face_covers_closed_surface(face) {
            return (0.0, tau);
        }
        let (c, ez, rmaj) = (t.frame.origin, t.frame.z, t.major);
        let (mut lo, mut hi, mut any) = (f64::INFINITY, f64::NEG_INFINITY, false);
        for &lk in self.faces.get(face).map(|f| &f.loops).into_iter().flatten() {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            while let Some(fin) = self.fins.get(cur) {
                if let Some(p) = self
                    .fin_end_vertex(cur)
                    .and_then(|vk| self.vertices.get(vk))
                    .map(|v| v.point)
                {
                    let w = (p - c) - ez * (p - c).dot(ez);
                    if let Some(radial) = w.try_normalize() {
                        let d = p - (c + radial * rmaj);
                        let v = d.dot(ez).atan2(d.dot(radial));
                        lo = lo.min(v);
                        hi = hi.max(v);
                        any = true;
                    }
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        if any && hi > lo { (lo, hi) } else { (0.0, tau) }
    }

    /// Tessellate a ring torus. point(u, v) = c + (R + rr cos v)*
    /// (ex cos u + ey sin u) + ez * rr sin v; outward points away from the
    /// tube centreline, sense-adjusted. Full revolution in u; the tube
    /// angle v is trimmed to the face's span (so a fillet's quarter-torus
    /// ring meshes only its tube quarter).
    fn tessellate_torus(
        &self,
        face: FaceKey,
        torus: &keel_geom::surface::Torus3,
        sense: bool,
        tol: Option<f64>,
    ) -> Vec<[Vec3; 3]> {
        let (c, ex, ey, ez, rmaj, rmin) = (
            torus.frame.origin,
            torus.frame.x,
            torus.frame.y,
            torus.frame.z,
            torus.major,
            torus.minor,
        );
        let tau = core::f64::consts::TAU;
        let sgn = if sense { 1.0 } else { -1.0 };
        let (vlo, vhi) = self.torus_tube_span(face, torus);
        // Adaptive (item 98): major-ring count from the outer radius, tube
        // count from the minor radius. The full major span (tau) is a
        // conservative bound for partial-major patches.
        let nu = arc_segments(tau, rmaj + rmin, tol, 64);
        let nv = arc_segments((vhi - vlo).abs(), rmin, tol, 32);
        let pt = |u: f64, v: f64| -> Vec3 {
            let radial = ex * u.cos() + ey * u.sin();
            c + radial * (rmaj + rmin * v.cos()) + ez * (rmin * v.sin())
        };
        let nrm = |u: f64, v: f64| -> Vec3 {
            let radial = ex * u.cos() + ey * u.sin();
            (radial * v.cos() + ez * v.sin()) * sgn
        };
        let mut tris = Vec::new();
        for i in 0..nu {
            let u0 = tau * i as f64 / nu as f64;
            let u1 = tau * (i + 1) as f64 / nu as f64;
            for j in 0..nv {
                let v0 = vlo + (vhi - vlo) * j as f64 / nv as f64;
                let v1 = vlo + (vhi - vlo) * (j + 1) as f64 / nv as f64;
                let a = pt(u0, v0);
                let b = pt(u0, v1);
                let cc = pt(u1, v1);
                let d = pt(u1, v0);
                tris.push(orient([a, b, cc], nrm(u0 + 0.0, 0.5 * (v0 + v1))));
                tris.push(orient([a, cc, d], nrm(0.5 * (u0 + u1), 0.5 * (v0 + v1))));
            }
        }
        tris
    }

    /// The angular [phi_lo, phi_hi] span a cylindrical face occupies,
    /// in the frame's (ex, ey) basis. Returns the full wrap (0, TAU) when
    /// the face carries a CLOSED circle edge (a whole lateral or SSI-
    /// trimmed band); otherwise the min/max boundary-vertex angle (a
    /// partial-angle patch such as a fillet's quarter-cylinder blend).
    pub(crate) fn cyl_angular_span(
        &self,
        face: FaceKey,
        origin: Vec3,
        ex: Vec3,
        ey: Vec3,
        ez: Vec3,
    ) -> (f64, f64) {
        let tau = core::f64::consts::TAU;
        let Some(f) = self.faces.get(face) else {
            return (0.0, tau);
        };
        // If a boundary arc carries an explicit signed sweep (a wide-angle
        // partial revolve), the span is start_angle .. start_angle + sweep,
        // taken continuously -- so a sector wider than pi is exact and the
        // atan2 branch cut is never crossed.
        for &lk in &f.loops {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            while let Some(fin) = self.fins.get(cur) {
                if let Some(e) = self.edges.get(fin.edge)
                    && let Some(sweep) = e.arc_sweep
                    && let Some(p0) = self.vertices.get(e.bounds.0).map(|v| v.point)
                {
                    let w = p0 - origin;
                    let w = w - ez * w.dot(ez);
                    let a = w.dot(ey).atan2(w.dot(ex));
                    return if sweep >= 0.0 {
                        (a, a + sweep)
                    } else {
                        (a + sweep, a)
                    };
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        let (mut lo, mut hi, mut any) = (f64::INFINITY, f64::NEG_INFINITY, false);
        for &lk in &f.loops {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            while let Some(fin) = self.fins.get(cur) {
                if self.edges.get(fin.edge).map(|e| e.is_closed()) == Some(true) {
                    return (0.0, tau);
                }
                if let Some(p) = self
                    .fin_end_vertex(cur)
                    .and_then(|vk| self.vertices.get(vk))
                    .map(|v| v.point)
                {
                    let w = p - origin;
                    let w = w - ez * w.dot(ez);
                    let phi = w.dot(ey).atan2(w.dot(ex));
                    lo = lo.min(phi);
                    hi = hi.max(phi);
                    any = true;
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        if any && hi > lo { (lo, hi) } else { (0.0, tau) }
    }

    /// Lat-band tessellate a cylindrical face. The axial band [hlo,hhi]
    /// is bounded by the face's CLOSED circle edges (the cap circles of
    /// the whole lateral, or the SSI + cap circles of a trimmed band).
    /// A face with no closed circle edge (a partial-angle patch, e.g. a
    /// fillet's quarter-cylinder blend) is ANGULARLY TRIMMED to the
    /// [phi_lo, phi_hi] span of its boundary vertices. Outward = radial.
    fn tessellate_cylinder(
        &self,
        face: FaceKey,
        cyl: &keel_geom::surface::Cylinder3,
        sense: bool,
        tol: Option<f64>,
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
        let (plo, phi_hi) = self.cyl_angular_span(face, origin, ex, ey, ez);
        const NV: usize = 16;
        // Adaptive angular count (item 98) from the cylinder radius and the
        // actual angular span; axial NV stays fixed (a cylinder is exact
        // along its axis).
        let np = arc_segments(phi_hi - plo, radius, tol, 64);
        let sgn = if sense { 1.0 } else { -1.0 };
        let pt = |phi: f64, v: f64| -> Vec3 {
            origin + (ex * phi.cos() + ey * phi.sin()) * radius + ez * v
        };
        let mut tris = Vec::new();
        for i in 0..NV {
            let v0 = hlo + (hhi - hlo) * i as f64 / NV as f64;
            let v1 = hlo + (hhi - hlo) * (i + 1) as f64 / NV as f64;
            for j in 0..np {
                let p0 = plo + (phi_hi - plo) * j as f64 / np as f64;
                let p1 = plo + (phi_hi - plo) * (j + 1) as f64 / np as f64;
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

    /// Lat-band tessellate a conical face. The axial band is bounded by
    /// the face's CLOSED circle edges and, where the face reaches the
    /// apex, the apex height (radius -> 0). Radius varies linearly with
    /// axial parameter v: r(v) = radius + v*tan(half_angle). Outward is
    /// radial (the dominant component of the cone normal), sense-adjusted.
    fn tessellate_cone(
        &self,
        face: FaceKey,
        cone: &keel_geom::surface::Cone3,
        sense: bool,
        tol: Option<f64>,
    ) -> Vec<[Vec3; 3]> {
        let (origin, ex, ey, ez) = (cone.frame.origin, cone.frame.x, cone.frame.y, cone.frame.z);
        let slope = cone.half_angle.tan();
        if slope == 0.0 {
            return Vec::new();
        }
        let r_at = |v: f64| (cone.radius + v * slope).max(0.0);
        // Band bounds: circle-edge heights, plus the apex if the face
        // reaches it (only one circle edge -> the other end is the apex).
        let mut heights = self.cyl_circle_heights(face, origin, ez);
        let v_apex = -cone.radius / slope;
        if heights.len() < 2 {
            heights.push(v_apex);
        }
        if heights.len() < 2 {
            return Vec::new();
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if hhi - hlo <= 0.0 {
            return Vec::new();
        }
        // A partial-angle cone patch (a partial-revolve cone-sector band)
        // has no closed circle edge -> trim to its boundary's phi span,
        // exactly as the cylinder lateral does.
        let (plo, phi_hi) = self.cyl_angular_span(face, origin, ex, ey, ez);
        const NV: usize = 16;
        // Adaptive angular count (item 98) from the cone's LARGEST band
        // radius and the angular span; the slant (NV) stays fixed (linear).
        let np = arc_segments(phi_hi - plo, r_at(hlo).max(r_at(hhi)), tol, 64);
        let sgn = if sense { 1.0 } else { -1.0 };
        let pt = |phi: f64, v: f64| -> Vec3 {
            origin + (ex * phi.cos() + ey * phi.sin()) * r_at(v) + ez * v
        };
        let mut tris = Vec::new();
        for i in 0..NV {
            let v0 = hlo + (hhi - hlo) * i as f64 / NV as f64;
            let v1 = hlo + (hhi - hlo) * (i + 1) as f64 / NV as f64;
            for j in 0..np {
                let p0 = plo + (phi_hi - plo) * j as f64 / np as f64;
                let p1 = plo + (phi_hi - plo) * (j + 1) as f64 / np as f64;
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
            let loop_out = if li == 0 { outward } else { outward * -1.0 };
            if li == 0 {
                // Outer loop: ear-clip, so NON-star-convex faces (an L-cap,
                // a boolean fragment) triangulate correctly -- a centroid
                // fan is only valid for star-convex loops.
                for [ia, ib, ic] in earclip_3d(&poly, nz) {
                    tris.push(orient([poly[ia], poly[ib], poly[ic]], loop_out));
                }
            } else {
                // Inner rings (holes): reversed centroid fan to subtract
                // their solid-angle / volume contribution.
                let n = poly.len();
                let centroid = poly.iter().fold(Vec3::ZERO, |a, p| a + *p) * (1.0 / n as f64);
                for i in 0..n {
                    tris.push(orient([centroid, poly[i], poly[(i + 1) % n]], loop_out));
                }
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
                let c = *c;
                circle_edge = Some(c);
                // Sample an OPEN arc edge (a fillet cap's spring/end arc, a
                // partial-revolve band arc) so the polygon follows the
                // curve, not its chord (closed full-circle edges use the
                // fallback). The default is the SHORT span; an edge with an
                // explicit signed sweep (wide-angle revolve) takes that
                // instead, so arcs beyond pi are followed correctly.
                if self.edges.get(fin.edge).map(|e| !e.is_closed()) == Some(true)
                    && let (Some(ps), Some(pe)) = (
                        self.fin_start_vertex(cur)
                            .and_then(|v| self.vertices.get(v))
                            .map(|v| v.point),
                        self.fin_end_vertex(cur)
                            .and_then(|v| self.vertices.get(v))
                            .map(|v| v.point),
                    )
                {
                    let ang = |p: Vec3| {
                        (p - c.center)
                            .dot(c.y_axis)
                            .atan2((p - c.center).dot(c.x_axis))
                    };
                    let ts = ang(ps);
                    let d = if let Some(sweep) = self.edges.get(fin.edge).and_then(|e| e.arc_sweep)
                    {
                        // Signed sweep is bounds.0 -> bounds.1; flip it when
                        // this fin runs the edge backward.
                        if fin.forward { sweep } else { -sweep }
                    } else {
                        let mut d = ang(pe) - ts;
                        let pi = core::f64::consts::PI;
                        while d > pi {
                            d -= core::f64::consts::TAU;
                        }
                        while d <= -pi {
                            d += core::f64::consts::TAU;
                        }
                        d
                    };
                    // More segments for wider arcs so chord error stays
                    // bounded as the sweep approaches 2pi.
                    let seg = ((d.abs() * 8.0 / core::f64::consts::PI).ceil() as usize).max(8);
                    for k in 1..seg {
                        verts.push(c.point(ts + d * (k as f64 / seg as f64)));
                    }
                }
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
        tol: Option<f64>,
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
        let tau = core::f64::consts::TAU;
        let pi = core::f64::consts::PI;
        // Adaptive (item 98): polar count over [0,pi] and azimuth count
        // over [0,tau], both from the sphere radius.
        let nt = arc_segments(pi, radius, tol, 40);
        let np = arc_segments(tau, radius, tol, 60);
        let mut tris = Vec::new();
        let sgn = if sense { 1.0 } else { -1.0 };
        let on_cap = |q: Vec3| -> bool {
            match cap {
                Some((cc, ax, side)) => ((q - cc).dot(ax) * side) >= 0.0,
                None => true,
            }
        };
        for i in 0..nt {
            let t0 = pi * i as f64 / nt as f64;
            let t1 = pi * (i + 1) as f64 / nt as f64;
            for j in 0..np {
                let p0 = tau * j as f64 / np as f64;
                let p1 = tau * (j + 1) as f64 / np as f64;
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

/// Ear-clip a planar 3D polygon (in the plane of unit normal `nz`) into
/// triangles, returned as index triples into `poly`. Handles non-convex
/// (non-star-convex) simple polygons; orientation of the output triangles
/// is left to the caller's `orient`.
fn earclip_3d(poly: &[Vec3], nz: Vec3) -> Vec<[usize; 3]> {
    let seed = if nz.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = (seed - nz * seed.dot(nz))
        .try_normalize()
        .unwrap_or(Vec3::new(1.0, 0.0, 0.0));
    let w = nz.cross(u);
    let p: Vec<[f64; 2]> = poly.iter().map(|&q| [q.dot(u), q.dot(w)]).collect();
    earclip_2d(&p)
}

fn earclip_2d(p: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let n = p.len();
    if n < 3 {
        return Vec::new();
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let area2: f64 = (0..n)
        .map(|i| cross([0.0, 0.0], p[i], p[(i + 1) % n]))
        .sum();
    let ccw = area2 > 0.0;
    let in_tri = |q: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        let (d1, d2, d3) = (cross(a, b, q), cross(b, c, q), cross(c, a, q));
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    };
    let mut idx: Vec<usize> = (0..n).collect();
    let mut tris = Vec::new();
    let mut guard = 0usize;
    while idx.len() > 3 {
        guard += 1;
        if guard > n * n + 16 {
            break; // degenerate / self-intersecting: bail with a partial fan
        }
        let m = idx.len();
        let mut found = None;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (p[ia], p[ib], p[ic]);
            let turn = cross(a, b, c);
            // Reflex vertex (wrong turn for the winding) is not an ear tip.
            if (ccw && turn <= 0.0) || (!ccw && turn >= 0.0) {
                continue;
            }
            let mut clean = true;
            for &j in &idx {
                if j != ia && j != ib && j != ic && in_tri(p[j], a, b, c) {
                    clean = false;
                    break;
                }
            }
            if clean {
                found = Some(i);
                break;
            }
        }
        match found {
            Some(i) => {
                let m = idx.len();
                tris.push([idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]]);
                idx.remove(i);
            }
            None => break,
        }
    }
    if idx.len() == 3 {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    tris
}
