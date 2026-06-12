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
        crate::profile::count(&crate::profile::TESS_FACE_CALLS);
        self.tessellate_face_opt(face, None)
    }

    /// Tessellate the whole body to outward-oriented triangles (parity
    /// item 94): every face's facets at the default density, or refined
    /// to chord tolerance `tol` on curved faces (the item-98 adaptive
    /// machinery). The public body-level facet output; `render_mesh` /
    /// `render_mesh_tol` add edge/silhouette lines on top of this.
    pub fn facets(&self, tol: Option<f64>) -> Vec<[Vec3; 3]> {
        self.face_keys()
            .into_iter()
            .flat_map(|f| self.tessellate_face_opt(f, tol))
            .collect()
    }

    /// Like `tessellate_face`, but tessellate curved analytic faces to a
    /// chord tolerance `tol` (parity item 98). The default-density path
    /// (`tessellate_face`) is unchanged, so the winding/volume oracle is
    /// untouched.
    pub(crate) fn tessellate_face_tol(&self, face: FaceKey, tol: f64) -> Vec<[Vec3; 3]> {
        crate::profile::count(&crate::profile::TESS_FACE_CALLS);
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
        // (The old first-arc shortcut that read edge.arc_sweep as an
        // AZIMUTH span is gone: arc_sweep is a CURVE-PARAMETER sweep
        // (the loop_polygon and massprops reading), and the two only
        // coincide for coaxial circles. Wide revolve sectors are
        // handled below: loop_polygon follows swept arcs and the
        // unwrapped cumulative span never crosses the atan2 branch
        // cut. Task 29 semantic reconciliation, step 1.)
        // Closed boundary edge (a full-circle rim): whole revolution.
        for &lk in &f.loops {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            while let Some(fin) = self.fins.get(cur) {
                if self.edges.get(fin.edge).map(|e| e.is_closed()) == Some(true) {
                    return (0.0, tau);
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        // Boundary SAMPLE angles (loop_polygon follows arcs, so the
        // span is right even when the trim is curved), skipping
        // near-axis points (a cone APEX has no angle and previously
        // polluted the span). The span is the complement of the
        // LARGEST angular gap, which is branch-cut-free (a plain
        // min..max breaks when the patch straddles +-pi).
        // A loop whose UNWRAPPED cumulative angle range covers a full
        // turn bounds a full revolution even when no single closed
        // edge survives (a seam imprint splits each rim into arcs):
        // the sample-gap complement below would otherwise eat one
        // sampling step out of the ring (the M5 gate caught exactly
        // this on the drilled-plate lateral: span tau - pi/8, mesh
        // short by 16 percent).
        for &lk in &f.loops {
            let (mut cum, mut prev) = (0.0f64, f64::NAN);
            let (mut clo, mut chi) = (0.0f64, 0.0f64);
            for p in self.loop_polygon(lk) {
                let w = p - origin;
                let w = w - ez * w.dot(ez);
                if w.norm() < 1e-9 * (1.0 + (p - origin).norm()) {
                    continue;
                }
                let a = w.dot(ey).atan2(w.dot(ex));
                if prev.is_nan() {
                    prev = a;
                    continue;
                }
                let mut d = a - prev;
                if d > core::f64::consts::PI {
                    d -= tau;
                }
                if d < -core::f64::consts::PI {
                    d += tau;
                }
                cum += d;
                prev = a;
                clo = clo.min(cum);
                chi = chi.max(cum);
            }
            if chi - clo >= tau - 1e-6 {
                return (0.0, tau);
            }
        }
        // EXACT per-loop azimuth INTERVALS (task 29 metric layer): each
        // loop's continuous unwrapped walk covers [a0+clo, a0+chi],
        // independent of sampling density: the old point-sample gap
        // complement ate one sampling step out of arc-bounded pieces.
        // The span is the complement of the largest gap between the
        // merged intervals.
        let mut intervals: Vec<(f64, f64)> = Vec::new();
        for &lk in &f.loops {
            let (mut cum, mut prev, mut a0) = (0.0f64, f64::NAN, f64::NAN);
            let (mut clo, mut chi) = (0.0f64, 0.0f64);
            for p in self.loop_polygon(lk) {
                let w = p - origin;
                let w = w - ez * w.dot(ez);
                if w.norm() < 1e-9 * (1.0 + (p - origin).norm()) {
                    continue; // on the axis (apex/pole): no angle
                }
                let a = w.dot(ey).atan2(w.dot(ex));
                if prev.is_nan() {
                    prev = a;
                    a0 = a;
                    continue;
                }
                let mut d = a - prev;
                if d > core::f64::consts::PI {
                    d -= tau;
                }
                if d < -core::f64::consts::PI {
                    d += tau;
                }
                cum += d;
                prev = a;
                clo = clo.min(cum);
                chi = chi.max(cum);
            }
            if !a0.is_nan() && chi > clo {
                intervals.push((a0 + clo, a0 + chi));
            }
        }
        if intervals.is_empty() {
            return (0.0, tau);
        }
        if intervals.iter().any(|(lo, hi)| hi - lo >= tau - 1e-6) {
            return (0.0, tau);
        }
        // Merge modulo tau, then take the complement of the widest gap.
        let mut iv: Vec<(f64, f64)> = intervals
            .iter()
            .map(|&(lo, hi)| {
                let l = lo.rem_euclid(tau);
                (l, l + (hi - lo))
            })
            .collect();
        iv.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (l, h) in iv {
            if let Some(last) = merged.last_mut()
                && l <= last.1 + 1e-9
            {
                last.1 = last.1.max(h);
            } else {
                merged.push((l, h));
            }
        }
        // Wrap-around merge: the last interval may reach past tau into
        // the first.
        if merged.len() > 1
            && let (Some(&(fl, fh)), Some(&(_, lh))) = (merged.first(), merged.last())
            && lh >= fl + tau - 1e-9
        {
            let nh = (lh - tau).max(fh);
            merged[0] = (fl, nh);
            merged.pop();
        }
        if merged.len() == 1 {
            let (l, h) = merged[0];
            if h - l >= tau - 1e-6 {
                return (0.0, tau);
            }
            return (l, h);
        }
        // Several disjoint covered intervals: the face's span is the
        // complement of the WIDEST gap (the contiguous cover holding
        // every interval).
        let m = merged.len();
        let (mut gap, mut gap_at) = (f64::NEG_INFINITY, 0usize);
        for i in 0..m {
            let (l_next, h_this) = (
                if i + 1 < m {
                    merged[i + 1].0
                } else {
                    merged[0].0 + tau
                },
                merged[i].1,
            );
            let g = l_next - h_this;
            if g > gap {
                gap = g;
                gap_at = i;
            }
        }
        let start = if gap_at + 1 < m {
            merged[gap_at + 1].0
        } else {
            merged[0].0
        };
        let end = merged[gap_at].1;
        let width = (end - start).rem_euclid(tau);
        let width = if width <= 1e-9 { tau } else { width };
        (start, start + width)
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
        // SSI arcs); a vertex-trimmed patch with fewer than two circle
        // heights (the mitre blend, bounded by one cap arc + ellipse
        // sub-arcs) takes its band from the raw boundary vertices.
        let mut heights = self.cyl_circle_heights(face, origin, ez);
        // DISTINCT heights only: a single rim circle reports once per
        // fin, and [h, h] is no band (task 29 keeper fix).
        heights.sort_by(f64::total_cmp);
        heights.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        if heights.len() < 2 {
            // Fin CURVE samples, not just vertices (the vertex-only
            // trap: a crossing-pair bowtie piece has ALL its vertices
            // at one height while its arcs bulge across the band).
            for lk in self
                .faces
                .get(face)
                .map(|f| f.loops.clone())
                .unwrap_or_default()
            {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                while let Some(fin) = self.fins.get(cur) {
                    for p in self.fin_curve_samples(cur, 8).unwrap_or_default() {
                        heights.push((p - origin).dot(ez));
                    }
                    if let Some(v) = self.fin_start_vertex(cur)
                        && let Some(x) = self.vertices.get(v)
                    {
                        heights.push((x.point - origin).dot(ez));
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
            }
        }
        if heights.len() < 2 {
            return Vec::new();
        }
        let hlo = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hhi = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if hhi - hlo <= 0.0 {
            return Vec::new();
        }
        let (plo, phi_hi) = self.cyl_angular_span(face, origin, ex, ey, ez);
        // Oblique boundary planes (ellipse arcs / tilted circle arcs:
        // a mitre seam or a partial-span stop) clamp each ruling, as in
        // tessellate_cone.
        let mut cap_planes: Vec<(Vec3, Vec3)> = Vec::new();
        for lk in self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default()
        {
            for e in self.ring_edges(lk) {
                let Some((ck, _)) = self.edges.get(e).and_then(|x| x.curve) else {
                    continue;
                };
                match self.curves.get(ck) {
                    Some(keel_geom::curve::Curve3::Ellipse(el)) => {
                        cap_planes.push((el.center, el.x_axis.cross(el.y_axis)));
                    }
                    Some(keel_geom::curve::Curve3::Circle(ci)) => {
                        let n = ci.x_axis.cross(ci.y_axis);
                        if n.dot(ez).abs() < 1.0 - 1e-9 {
                            cap_planes.push((ci.center, n));
                        }
                    }
                    _ => {}
                }
            }
        }
        const NV: usize = 16;
        // Adaptive angular count (item 98) from the cylinder radius and the
        // actual angular span; axial NV stays fixed (a cylinder is exact
        // along its axis).
        let np = arc_segments(phi_hi - plo, radius, tol, 64);
        let sgn = if sense { 1.0 } else { -1.0 };
        let pt = |phi: f64, v: f64| -> Vec3 {
            origin + (ex * phi.cos() + ey * phi.sin()) * radius + ez * v
        };
        // Which side of each clipping plane the FACE lives on (task 29
        // metric layer): the closest-extreme heuristic mis-assigned
        // planes near their crossings (the Steinmetz bowtie pinch).
        // The side WITNESS is whichever candidate point (the interior
        // point, or any boundary fin sample) sits FARTHEST from the
        // planes: a band's interior fallback can land exactly ON a
        // bounding curve, but its rim samples have full clearance;
        // the bowtie's rim-less boundary hugs the planes, but its
        // interior point has clearance.
        let p_int = {
            let mut candidates: Vec<Vec3> = Vec::new();
            if let Some(p) = self.face_interior_point(face) {
                candidates.push(p);
            }
            if !cap_planes.is_empty() {
                for lk in self
                    .faces
                    .get(face)
                    .map(|f| f.loops.clone())
                    .unwrap_or_default()
                {
                    let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                        continue;
                    };
                    let mut cur = entry;
                    while let Some(fin) = self.fins.get(cur) {
                        candidates.extend(self.fin_curve_samples(cur, 6).unwrap_or_default());
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                }
            }
            let clearance = |p: &Vec3| -> f64 {
                cap_planes
                    .iter()
                    .map(|(q, n)| ((*p - *q).dot(*n)).abs())
                    .fold(f64::INFINITY, f64::min)
            };
            candidates.into_iter().max_by(|a, b| {
                clearance(a)
                    .partial_cmp(&clearance(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        };
        if std::env::var("KEEL_TESS_DEBUG").is_ok() {
            eprintln!(
                "  tess_cyl {face:?}: {} cap planes {:?} witness {p_int:?}",
                cap_planes.len(),
                cap_planes
                    .iter()
                    .map(|(_, n)| (n.x, n.y, n.z))
                    .collect::<Vec<_>>()
            );
        }
        let ruling_band = |phi: f64| -> (f64, f64) {
            if cap_planes.is_empty() {
                return (hlo, hhi);
            }
            let radial = ex * phi.cos() + ey * phi.sin();
            // A clipping plane REPLACES the vertex band on its end (the
            // mitre ellipse legitimately bulges past the extreme
            // boundary vertices); several planes on ONE end combine to
            // the BINDING innermost one.
            let (mut lo_clip, mut hi_clip): (Option<f64>, Option<f64>) = (None, None);
            for (q, n) in &cap_planes {
                let dv = ez.dot(*n);
                if dv.abs() < 1e-12 {
                    continue;
                }
                let base = (origin + radial * radius - *q).dot(*n);
                let hc = -base / dv;
                let to_lo = match p_int {
                    // The face survives where (h*dv + base) has the
                    // interior's sign: a LOWER bound when that sign and
                    // dv agree.
                    Some(p) => ((p - *q).dot(*n) >= 0.0) == (dv > 0.0),
                    None => (hc - hlo).abs() < (hc - hhi).abs(),
                };
                if to_lo {
                    lo_clip = Some(lo_clip.map_or(hc, |x: f64| x.max(hc)));
                } else {
                    hi_clip = Some(hi_clip.map_or(hc, |x: f64| x.min(hc)));
                }
            }
            let l = lo_clip.unwrap_or(hlo);
            let h = hi_clip.unwrap_or(hhi);
            if l > h { (l, l) } else { (l, h) }
        };
        let mut tris = Vec::new();
        for j in 0..np {
            let p0 = plo + (phi_hi - plo) * j as f64 / np as f64;
            let p1 = plo + (phi_hi - plo) * (j + 1) as f64 / np as f64;
            let (l0, h0) = ruling_band(p0);
            let (l1, h1) = ruling_band(p1);
            for i in 0..NV {
                let (f0, f1) = (i as f64 / NV as f64, (i + 1) as f64 / NV as f64);
                let a = pt(p0, l0 + (h0 - l0) * f0);
                let b = pt(p0, l0 + (h0 - l0) * f1);
                let c = pt(p1, l1 + (h1 - l1) * f1);
                let d = pt(p1, l1 + (h1 - l1) * f0);
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
            // No (or one) circle rim: take the band from the RAW
            // boundary vertex heights (a vertex-bounded cone patch like
            // the item-48 variable-radius blend face, or a pole-reaching
            // revolve cone whose pole vertex bounds the seam edge). The
            // raw edge bounds, NOT loop_polygon, whose closed-circle
            // fallback would return only rim samples and lose the pole.
            for lk in self
                .faces
                .get(face)
                .map(|f| f.loops.clone())
                .unwrap_or_default()
            {
                for e in self.ring_edges(lk) {
                    if let Some(ed) = self.edges.get(e) {
                        for v in [ed.bounds.0, ed.bounds.1] {
                            if let Some(p) = self.vertices.get(v).map(|x| x.point) {
                                heights.push((p - origin).dot(ez));
                            }
                        }
                    }
                }
            }
            let hl = heights.iter().cloned().fold(f64::INFINITY, f64::min);
            let hh = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if heights.len() < 2 || hh - hl <= 1e-12 {
                heights.push(v_apex);
            }
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
        // A cone patch bounded by two ELLIPSE arcs (the variable-radius
        // blend face, item 48) ends on TILTED cap planes, not constant-
        // height circles: clamp each ruling to its exact cap-plane
        // intersections (pt(phi, v) is linear in v, so each cap is a
        // scalar linear solve) so the band meets the caps watertight.
        let mut cap_planes: Vec<(Vec3, Vec3)> = Vec::new();
        for lk in self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default()
        {
            for e in self.ring_edges(lk) {
                let Some((ck, _)) = self.edges.get(e).and_then(|x| x.curve) else {
                    continue;
                };
                match self.curves.get(ck) {
                    Some(keel_geom::curve::Curve3::Ellipse(el)) => {
                        cap_planes.push((el.center, el.x_axis.cross(el.y_axis)));
                    }
                    // A TILTED circle boundary (the partial-span blend's
                    // stop arc lies in the plane perpendicular to the
                    // EDGE, not to the cone axis) also bounds the band;
                    // axis-perpendicular rims stay with the height path.
                    Some(keel_geom::curve::Curve3::Circle(ci)) => {
                        let n = ci.x_axis.cross(ci.y_axis);
                        if n.dot(ez).abs() < 1.0 - 1e-9 {
                            cap_planes.push((ci.center, n));
                        }
                    }
                    _ => {}
                }
            }
        }
        const NV: usize = 16;
        // Adaptive angular count (item 98) from the cone's LARGEST band
        // radius and the angular span; the slant (NV) stays fixed (linear).
        let np = arc_segments(phi_hi - plo, r_at(hlo).max(r_at(hhi)), tol, 64);
        let sgn = if sense { 1.0 } else { -1.0 };
        let pt = |phi: f64, v: f64| -> Vec3 {
            origin + (ex * phi.cos() + ey * phi.sin()) * r_at(v) + ez * v
        };
        // Per-ruling height bounds: the legacy constant band, or the
        // exact cap-plane intersections for the two-ellipse blend patch
        // (pt(phi, v) is linear in v, so each cap is a linear solve).
        let ruling_band = |phi: f64| -> (f64, f64) {
            if cap_planes.is_empty() {
                return (hlo, hhi);
            }
            let radial = ex * phi.cos() + ey * phi.sin();
            let (mut l, mut h) = (hlo, hhi);
            for (q, n) in &cap_planes {
                let base = (origin + radial * cone.radius - *q).dot(*n);
                let dv = (radial * slope + ez).dot(*n);
                if dv.abs() < 1e-12 {
                    continue;
                }
                let hc = -base / dv;
                // The plane clamps the band end it is nearer to.
                if (hc - l).abs() < (hc - h).abs() {
                    l = hc;
                } else {
                    h = hc;
                }
            }
            (l.min(h), l.max(h))
        };
        let mut tris = Vec::new();
        for j in 0..np {
            let p0 = plo + (phi_hi - plo) * j as f64 / np as f64;
            let p1 = plo + (phi_hi - plo) * (j + 1) as f64 / np as f64;
            let (l0, h0) = ruling_band(p0);
            let (l1, h1) = ruling_band(p1);
            for i in 0..NV {
                let (f0, f1) = (i as f64 / NV as f64, (i + 1) as f64 / NV as f64);
                let a = pt(p0, l0 + (h0 - l0) * f0);
                let b = pt(p0, l0 + (h0 - l0) * f1);
                let c = pt(p1, l1 + (h1 - l1) * f1);
                let d = pt(p1, l1 + (h1 - l1) * f0);
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
        // RING faces (holes) get a true polygon-with-holes triangulation
        // (ring bridged into the outer loop, then ear-clipped): the old
        // reversed-fan path is exact for volume/winding by SIGNED
        // CANCELLATION but its triangles COVER the hole, which renders
        // wrongly in any viewer consuming the worker mesh (task 34's
        // drill gif showed a capped bore). Falls back to the fan path if
        // the bridge cannot be placed.
        if loops.len() > 1
            && let Some(tris) = self.tessellate_planar_holed(&loops, nz, outward)
        {
            return tris;
        }
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

    /// True triangulation of a planar face with inner rings: each ring
    /// is spliced into the outer polygon through a mutually visible
    /// bridge vertex pair (the classic hole-cutting reduction), and the
    /// resulting simple polygon ear-clips. `None` when a bridge cannot
    /// be placed (the caller falls back to the signed-fan path).
    fn tessellate_planar_holed(
        &self,
        loops: &[crate::entity::LoopKey],
        nz: Vec3,
        outward: Vec3,
    ) -> Option<Vec<[Vec3; 3]>> {
        let seed = if nz.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let u = (seed - nz * seed.dot(nz)).try_normalize()?;
        let w = nz.cross(u);
        let to2 = |q: Vec3| [q.dot(u), q.dot(w)];
        let signed_area = |p: &[Vec3]| -> f64 {
            let n = p.len();
            (0..n)
                .map(|i| {
                    let a = to2(p[i]);
                    let b = to2(p[(i + 1) % n]);
                    a[0] * b[1] - a[1] * b[0]
                })
                .sum::<f64>()
                * 0.5
        };
        // Outer loop CCW in (u, w); rings CW (holes wind opposite).
        let mut outer = self.loop_polygon(loops[0]);
        if outer.len() < 3 {
            return None;
        }
        if signed_area(&outer) < 0.0 {
            outer.reverse();
        }
        let mut scale = 0.0f64;
        for p in &outer {
            let q = to2(*p);
            scale = scale.max(q[0].abs()).max(q[1].abs());
        }
        let eps = 1e-12 * scale.max(1.0);
        let cross2 = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
            (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
        };
        // Strict segment-segment crossing (shared endpoints don't count).
        let seg_cross = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| -> bool {
            let (a, b, c, d) = (to2(a), to2(b), to2(c), to2(d));
            let d1 = cross2(c, d, a);
            let d2 = cross2(c, d, b);
            let d3 = cross2(a, b, c);
            let d4 = cross2(a, b, d);
            ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps))
                && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps))
        };
        let mut rings: Vec<Vec<Vec3>> = Vec::new();
        for lk in &loops[1..] {
            let mut ring = self.loop_polygon(*lk);
            if ring.len() < 3 {
                return None;
            }
            if signed_area(&ring) > 0.0 {
                ring.reverse();
            }
            rings.push(ring);
        }
        // Splice rings one at a time, rightmost-first (in u), so later
        // bridges see the already-spliced boundary.
        rings.sort_by(|r1, r2| {
            let m1 = r1
                .iter()
                .map(|p| to2(*p)[0])
                .fold(f64::NEG_INFINITY, f64::max);
            let m2 = r2
                .iter()
                .map(|p| to2(*p)[0])
                .fold(f64::NEG_INFINITY, f64::max);
            m2.partial_cmp(&m1).unwrap_or(std::cmp::Ordering::Equal)
        });
        for ring in &rings {
            // Bridge from the ring's max-u vertex to the nearest outer
            // vertex the bridge segment can reach without crossing the
            // current boundary or the ring itself.
            let mi = (0..ring.len())
                .max_by(|&i, &j| {
                    to2(ring[i])[0]
                        .partial_cmp(&to2(ring[j])[0])
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(0);
            let m = ring[mi];
            let mut cands: Vec<usize> = (0..outer.len()).collect();
            cands.sort_by(|&i, &j| {
                let di = (outer[i] - m).norm();
                let dj = (outer[j] - m).norm();
                di.partial_cmp(&dj).unwrap_or(std::cmp::Ordering::Equal)
            });
            let visible = |pi: usize| -> bool {
                let p = outer[pi];
                let n = outer.len();
                for i in 0..n {
                    let (a, b) = (outer[i], outer[(i + 1) % n]);
                    if i != pi && (i + 1) % n != pi && seg_cross(m, p, a, b) {
                        return false;
                    }
                }
                let rn = ring.len();
                for i in 0..rn {
                    let (a, b) = (ring[i], ring[(i + 1) % rn]);
                    if i != mi && (i + 1) % rn != mi && seg_cross(m, p, a, b) {
                        return false;
                    }
                }
                true
            };
            let pi = cands.into_iter().find(|&pi| visible(pi))?;
            // outer[..=pi] ++ ring[mi..] ++ ring[..=mi] ++ [outer[pi]] ++ rest
            let mut next = Vec::with_capacity(outer.len() + ring.len() + 2);
            next.extend_from_slice(&outer[..=pi]);
            for k in 0..=ring.len() {
                next.push(ring[(mi + k) % ring.len()]);
            }
            next.push(outer[pi]);
            next.extend_from_slice(&outer[pi + 1..]);
            outer = next;
        }
        let pts2: Vec<[f64; 2]> = outer.iter().map(|&q| to2(q)).collect();
        let tris2 = earclip_2d_eps(&pts2, eps);
        if tris2.is_empty() {
            return None;
        }
        Some(
            tris2
                .into_iter()
                .map(|[ia, ib, ic]| orient([outer[ia], outer[ib], outer[ic]], outward))
                .collect(),
        )
    }

    /// A loop's boundary polygon: its fins' start vertices for a straight
    /// polygon (box faces), or the boundary circle sampled into points
    /// for a single closed-circle loop (disc caps, holes).
    pub(crate) fn loop_polygon(&self, lk: crate::entity::LoopKey) -> Vec<Vec3> {
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
            // Open ELLIPSE arc edges (the variable-radius fillet's cap
            // sections, item 48): sample the short span so the polygon
            // follows the conic, not its chord.
            if let Some(fin) = self.fins.get(cur)
                && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                && let Some(Curve3::Ellipse(el)) = self.curves.get(ck)
                && self.edges.get(fin.edge).map(|e| !e.is_closed()) == Some(true)
                && let (Some(ps), Some(pe)) = (
                    self.fin_start_vertex(cur)
                        .and_then(|v| self.vertices.get(v))
                        .map(|v| v.point),
                    self.fin_end_vertex(cur)
                        .and_then(|v| self.vertices.get(v))
                        .map(|v| v.point),
                )
            {
                let el = *el;
                let ang = |p: Vec3| {
                    let w = p - el.center;
                    (w.dot(el.y_axis) / el.b).atan2(w.dot(el.x_axis) / el.a)
                };
                let ts = ang(ps);
                let pi = core::f64::consts::PI;
                // An explicit recorded sweep (the crossing-pair arcs,
                // whose antipodal halves are direction-AMBIGUOUS at
                // exactly pi) wins over the short-span default.
                let d = if let Some(sweep) = self.edges.get(fin.edge).and_then(|e| e.arc_sweep) {
                    if fin.forward { sweep } else { -sweep }
                } else {
                    let mut d = ang(pe) - ts;
                    while d > pi {
                        d -= core::f64::consts::TAU;
                    }
                    while d <= -pi {
                        d += core::f64::consts::TAU;
                    }
                    d
                };
                let seg = ((d.abs() * 8.0 / pi).ceil() as usize).max(8);
                for k in 1..seg {
                    verts.push(el.point(ts + d * (k as f64 / seg as f64)));
                }
            }
            // Open NURBS arc edges (the conic blend's cap sections,
            // item 49): sample the curve domain, oriented to this fin.
            // DEGREE-1 NURBS is a straight segment (the boolean's seam
            // edges): its vertices suffice, and sampling would trace
            // the parent's FULL span on a split survivor (split_edge
            // keeps the whole curve on both halves), folding spurious
            // points into the polygon.
            if let Some(fin) = self.fins.get(cur)
                && let Some((ck, _)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                && let Some(Curve3::Nurbs(nc)) = self.curves.get(ck)
                && nc.degree() > 1
                && self.edges.get(fin.edge).map(|e| !e.is_closed()) == Some(true)
                && let Some(ps) = self
                    .fin_start_vertex(cur)
                    .and_then(|v| self.vertices.get(v))
                    .map(|v| v.point)
            {
                let nc = nc.clone();
                let (t0, t1) = nc.domain();
                let forward = (nc.point(t0) - ps).norm() <= (nc.point(t1) - ps).norm();
                const SEG: usize = 8;
                for k in 1..SEG {
                    let f = k as f64 / SEG as f64;
                    let t = if forward {
                        t0 + (t1 - t0) * f
                    } else {
                        t1 - (t1 - t0) * f
                    };
                    verts.push(nc.point(t));
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
        // A spherical POLYGON face (the vertex-blend octant, item 51):
        // bounded by OPEN circle arcs, each lying in a plane; keep the
        // triangles on the face side of EVERY arc plane (side = where
        // the boundary-vertex average lies).
        let mut arc_planes: Vec<(Vec3, Vec3, f64)> = Vec::new();
        if cap.is_none() {
            let mut planes: Vec<(Vec3, Vec3)> = Vec::new();
            let mut avg = Vec3::ZERO;
            let mut n_pts = 0usize;
            for lk in self
                .faces
                .get(face)
                .map(|f| f.loops.clone())
                .unwrap_or_default()
            {
                for e in self.ring_edges(lk) {
                    let Some(ed) = self.edges.get(e) else {
                        continue;
                    };
                    if ed.is_closed() {
                        continue;
                    }
                    if let Some((ck, _)) = ed.curve
                        && let Some(keel_geom::curve::Curve3::Circle(ci)) = self.curves.get(ck)
                    {
                        planes.push((ci.center, ci.x_axis.cross(ci.y_axis)));
                    }
                    for v in [ed.bounds.0, ed.bounds.1] {
                        if let Some(p) = self.vertices.get(v).map(|x| x.point) {
                            avg = avg + p;
                            n_pts += 1;
                        }
                    }
                }
            }
            if planes.len() >= 2 && n_pts > 0 {
                let avg = avg * (1.0 / n_pts as f64);
                for (q, n) in planes {
                    let s = (avg - q).dot(n);
                    if s.abs() > 1e-12 {
                        arc_planes.push((q, n, s.signum()));
                    }
                }
            }
        }
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
            let cap_ok = match cap {
                Some((cc, ax, side)) => ((q - cc).dot(ax) * side) >= 0.0,
                None => true,
            };
            cap_ok
                && arc_planes
                    .iter()
                    .all(|(c, n, s)| ((q - *c).dot(*n)) * s >= 0.0)
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

/// Ear clipping with an EPS-tolerant containment test: a bridge-spliced
/// polygon (hole cutting) carries DUPLICATE vertices lying exactly on
/// other edges, which a boundary-inclusive blocking test deadlocks on.
/// Points within eps of an ear's boundary do not block; near-zero-area
/// ears (the bridge passage itself) are clipped without emitting a
/// triangle. Returns empty on failure (the caller falls back).
fn earclip_2d_eps(p: &[[f64; 2]], eps: f64) -> Vec<[usize; 3]> {
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
    let strictly_in = |q: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        let (d1, d2, d3) = (cross(a, b, q), cross(b, c, q), cross(c, a, q));
        if ccw {
            d1 > eps && d2 > eps && d3 > eps
        } else {
            d1 < -eps && d2 < -eps && d3 < -eps
        }
    };
    let mut idx: Vec<usize> = (0..n).collect();
    let mut tris = Vec::new();
    let mut guard = 0usize;
    while idx.len() > 3 {
        guard += 1;
        if guard > n * n + 16 {
            return Vec::new();
        }
        let m = idx.len();
        let mut found = None;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (p[ia], p[ib], p[ic]);
            let turn = cross(a, b, c);
            if (ccw && turn < -eps) || (!ccw && turn > eps) {
                continue; // reflex vertex: not an ear tip
            }
            let degenerate = turn.abs() <= eps;
            let mut clean = true;
            for &j in &idx {
                if j != ia && j != ib && j != ic && strictly_in(p[j], a, b, c) {
                    clean = false;
                    break;
                }
            }
            if clean {
                found = Some((i, degenerate, [ia, ib, ic]));
                break;
            }
        }
        let Some((i, degenerate, tri)) = found else {
            return Vec::new();
        };
        if !degenerate {
            tris.push(tri);
        }
        idx.remove(i);
    }
    let (a, b, c) = (p[idx[0]], p[idx[1]], p[idx[2]]);
    if cross(a, b, c).abs() > eps {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    tris
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
            break; // degenerate / self-intersecting: SIGNED-FAN the rest below
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
            None => break, // no clean ear: SIGNED-FAN the rest below
        }
    }
    if idx.len() == 3 {
        tris.push([idx[0], idx[1], idx[2]]);
    } else if idx.len() > 3 {
        // No clean ear remained (a tolerance-scale imprint leaves a
        // vertex 1e-7 from a corner, and that twin sits ON every
        // candidate ear's edge, so in_tri vetoes them all; the old
        // silent partial output DROPPED the remainder's area: the
        // oracle-trial-15219 mesh wrong-positive, a 6-gon emitting 2
        // triangles). Finish with a SIGNED FAN from the first
        // remaining vertex: by the shoelace identity a fan from any
        // vertex reproduces a simple polygon's signed measure
        // EXACTLY, concave or degenerate, so mesh volume, flux, and
        // winding consumers are exact; only unsigned raster coverage
        // of pathological remainders is approximate.
        for i in 1..idx.len() - 1 {
            tris.push([idx[0], idx[i], idx[i + 1]]);
        }
    }
    tris
}

#[cfg(test)]
mod tests {
    use crate::Body;
    use keel_geom::surface::Frame3;
    use keel_math::vec::Vec3;

    #[test]
    fn facets_refine_to_tolerance() {
        // Item 94: body-level facet output; a tighter chord tolerance
        // yields more triangles and a closer volume on a sphere.
        let mut b = Body::new();
        b.sphere(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
        )
        .unwrap();
        let coarse = b.facets(None);
        let fine = b.facets(Some(2e-4));
        assert!(!coarse.is_empty(), "default facets");
        assert!(
            fine.len() > coarse.len(),
            "tolerance must refine ({} vs {})",
            fine.len(),
            coarse.len()
        );
        let vol: f64 = fine.iter().map(|t| t[0].dot(t[1].cross(t[2])) / 6.0).sum();
        let want = 4.0 / 3.0 * core::f64::consts::PI;
        assert!(
            (vol - want).abs() < 0.01 * want,
            "fine facet volume {vol} != ~{want}"
        );
    }
}
