//! Mass properties via the divergence theorem (M4 Task 6).
//!
//! THE ORIENTATION AUDIT: volumes are computed with NO sign fudge.
//! Per-face orientation is the solid-OUTWARD normal n_out = sense *
//! natural (research file 46: the single sense-based authority shared
//! with the mesh path), folded together with each face's own loop
//! winding. Region solidity only validates that a face bounds exactly
//! one solid region; it no longer sets the sign on its own (that was
//! correct only while sense agreed with it, and silently wrong on a
//! reversed-sense cavity wall / genus-1 inner band). A negative volume
//! here is a real orientation bug, never something to abs() away.
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
            // An interior partition wall (both sides solid, item 29)
            // contributes equal and opposite flux from its two cells:
            // net zero. Skip it; the outer boundary carries the mass.
            if (fs, bs) == (true, true) {
                continue;
            }
            // Validity: a solid face bounds exactly one solid region (a
            // double-sided / lamina face has no single material side).
            if (fs, bs) != (false, true) && (fs, bs) != (true, false) {
                return Err(TopoError::Precondition(
                    "mass_properties: face does not bound exactly one solid region",
                ));
            }
            let Some((sk, sense)) = face.surface else {
                return Err(TopoError::Precondition(
                    "mass_properties: face without surface",
                ));
            };
            let Some(SurfaceGeom::Analytic(surf)) = self.surfaces.get(sk) else {
                return Err(TopoError::Precondition(
                    "mass_properties: NURBS faces are M5",
                ));
            };
            // Outward normal = sense * natural (research file 46): the SOLE
            // orientation authority, consistent with the mesh path. (Region
            // solidity ALONE was correct only while sense agreed with it; a
            // reversed-sense face -- a cavity wall / genus-1 inner band --
            // needs the sense.) Each integrator folds sense_sign in with the
            // face's own loop winding.
            let sense_sign = if sense { 1.0 } else { -1.0 };
            match surf {
                Surface3::Plane(_) => self.integrate_planar_face(fk, surf, sense_sign, &mut m)?,
                _ => self.integrate_curved_face(fk, surf, sense_sign, &mut m)?,
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

    /// Planar face: integrate the trimmed UV region bounded by ALL the
    /// face's loops (outer plus inner rings). Each loop is sampled to a
    /// UV polyline, wound to match its kind (outer counterclockwise =
    /// positive, inner ring clockwise = negative), and signed-triangle-
    /// fanned; the degree-5 triangle rule is exact for the cubic
    /// integrands, so the hole subtracts exactly. (Green's theorem in
    /// triangulated form.)
    fn integrate_planar_face(
        &self,
        fk: FaceKey,
        surf: &Surface3,
        sense_sign: f64,
        m: &mut Moments,
    ) -> Result<(), TopoError> {
        let Surface3::Plane(plane) = surf else {
            return Err(TopoError::Precondition("not a plane"));
        };
        let f = &plane.frame;
        let at = |u: f64, v: f64| -> Vec3 { f.origin + f.x * u + f.y * v };
        let face = self.faces.get(fk).ok_or(TopoError::StaleKey)?;
        // Fast path: a single-loop disc (one circular edge, e.g. a
        // cylinder/cone cap) integrates EXACTLY via polar quadrature
        // (GL8 radial x periodic-trapezoid angular), far better than a
        // sampled polygon. Annulus/polygon faces fall through to the
        // general signed-fan.
        // Outward normal = sense * natural (file 46). The polar quadrature
        // below traverses CCW (positive area), so no winding factor is
        // needed here: normal = f.z * sense_sign.
        if face.loops.len() == 1
            && let Some(circle) = self.single_circle_disc(face.loops[0])
        {
            let normal = f.z * sense_sign;
            let (cu, cv, r) = circle;
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
            return Ok(());
        }
        let rule = triangle_rule();
        let signed_area = |poly: &[(f64, f64)]| -> f64 {
            (0..poly.len())
                .map(|i| {
                    let a = poly[i];
                    let b = poly[(i + 1) % poly.len()];
                    a.0 * b.1 - b.0 * a.1
                })
                .sum::<f64>()
                * 0.5
        };
        // The OUTER loop keeps its NATURAL winding (whatever the fins
        // produce); the normal folds that winding in so the integrand is
        // taken about the true outward normal. outward = sense * natural,
        // and the signed fan carries an extra outer_sign factor, so
        // normal = f.z * sense_sign * outer_sign. Inner rings wind
        // OPPOSITE to the outer loop so the fan sum subtracts the hole.
        let outer_sign = face
            .loops
            .first()
            .and_then(|&l0| self.loop_uv_polyline_planar(l0, f).ok())
            .map(|(p, _)| signed_area(&p).signum())
            .unwrap_or(1.0);
        let normal = f.z * sense_sign * outer_sign;
        for (li, &lk) in face.loops.iter().enumerate() {
            let (mut poly, mut lunes) = self.loop_uv_polyline_planar(lk, f)?;
            if poly.len() < 3 {
                return Err(TopoError::Precondition("degenerate planar loop"));
            }
            if li > 0 {
                // Inner ring: force opposite to the outer winding.
                if signed_area(&poly).signum() == outer_sign {
                    poly.reverse();
                    for l in &mut lunes {
                        l.2 = -l.2;
                    }
                }
            }
            for i in 1..poly.len() - 1 {
                let (a, b, c) = (poly[0], poly[i], poly[i + 1]);
                let tri_area = 0.5 * ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0));
                for (bary, w) in rule {
                    let u = bary[0] * a.0 + bary[1] * b.0 + bary[2] * c.0;
                    let v = bary[0] * a.1 + bary[1] * b.1 + bary[2] * c.1;
                    m.add(at(u, v), normal, w * tri_area);
                }
            }
            // The chord polygon misses the LUNES between each chord and
            // its true arc; the signed quadrature samples close that gap
            // exactly (the formerly documented chordal residual on arc-
            // bounded planar faces carrying flux).
            for (u, v, w) in lunes {
                m.add(at(u, v), normal, w);
            }
        }
        Ok(())
    }

    /// If the loop is a single fin over a closed circular edge, return
    /// its (center_u, center_v, radius) in the plane frame (via the
    /// edge's pcurve, which is a UV circle). None otherwise.
    pub(crate) fn single_circle_disc(&self, lk: crate::entity::LoopKey) -> Option<(f64, f64, f64)> {
        let entry = self.loops.get(lk).and_then(|l| l.fin)?;
        // Exactly one fin in the loop.
        let f = self.fins.get(entry)?;
        if f.next != entry {
            return None;
        }
        let (ck, _) = f.pcurve?;
        match self.curves.get(ck)? {
            Curve3::Circle(c) => Some((c.center.x, c.center.y, c.radius)),
            _ => None,
        }
    }

    /// Sample a planar face loop into a UV polyline, plus SIGNED lune
    /// quadrature samples (u, v, weight) for the regions between each
    /// chord and its true circle/ellipse arc. The fan over the chord
    /// polygon plus the lunes integrates arc-bounded planar faces
    /// EXACTLY (the map (theta, t) -> lerp(chord, arc) carries a
    /// signed Jacobian, so convex and concave arcs, full rings, and
    /// reversed fins all fall out of one formula). NURBS fins above
    /// degree 1 stay chordal. Vertex UVs come from projecting the 3D
    /// vertex into the plane frame.
    #[allow(clippy::type_complexity)]
    fn loop_uv_polyline_planar(
        &self,
        lk: crate::entity::LoopKey,
        f: &keel_geom::surface::Frame3,
    ) -> Result<(Vec<(f64, f64)>, Vec<(f64, f64, f64)>), TopoError> {
        let entry = self
            .loops
            .get(lk)
            .and_then(|l| l.fin)
            .ok_or(TopoError::Precondition("vertex loop face"))?;
        let uv = |p: Vec3| -> (f64, f64) {
            let w = p - f.origin;
            (w.dot(f.x), w.dot(f.y))
        };
        let uv_vec = |w: Vec3| -> (f64, f64) { (w.dot(f.x), w.dot(f.y)) };
        let mut poly = Vec::new();
        let mut lunes: Vec<(f64, f64, f64)> = Vec::new();
        let lune = |pt: &dyn Fn(f64) -> Vec3,
                    dpt: &dyn Fn(f64) -> Vec3,
                    th_a: f64,
                    th_b: f64,
                    out: &mut Vec<(f64, f64, f64)>| {
            let dth = th_b - th_a;
            if dth == 0.0 {
                return;
            }
            let q0 = uv(pt(th_a));
            let q1 = uv(pt(th_b));
            let qd = ((q1.0 - q0.0) / dth, (q1.1 - q0.1) / dth);
            for (xj, wj) in GL8_X.iter().zip(GL8_W) {
                let th = 0.5 * (th_a + th_b) + 0.5 * dth * xj;
                let p = uv(pt(th));
                let pd = uv_vec(dpt(th));
                let s = (th - th_a) / dth;
                let q = (q0.0 + s * (q1.0 - q0.0), q0.1 + s * (q1.1 - q0.1));
                let mt = (p.0 - q.0, p.1 - q.1);
                for (xk, wk) in GL8_X.iter().zip(GL8_W) {
                    let t = 0.5 * (xk + 1.0);
                    let mu = ((1.0 - t) * q.0 + t * p.0, (1.0 - t) * q.1 + t * p.1);
                    let mth = ((1.0 - t) * qd.0 + t * pd.0, (1.0 - t) * qd.1 + t * pd.1);
                    // The (theta, t) patch is the lune traversed chord-
                    // forward + arc-backward, MINUS the loop's own
                    // orientation (chord-backward + arc-forward): negate.
                    let det = mth.1 * mt.0 - mth.0 * mt.1;
                    out.push((mu.0, mu.1, wj * (0.5 * dth) * wk * 0.5 * det));
                }
            }
        };
        let mut cur = entry;
        const SAMPLES: usize = 24;
        loop {
            let fin = self.fins.get(cur).ok_or(TopoError::StaleKey)?;
            // Curvedness is a GEOMETRIC property of the edge's 3D
            // curve, not the pcurve's enum variant (a degree-1 NURBS
            // pcurve is straight). Sample only genuinely curved edges.
            let curved = self
                .edges
                .get(fin.edge)
                .and_then(|e| e.curve)
                .and_then(|(ck, _)| self.curves.get(ck))
                .map(|c| match c {
                    Curve3::Line(_) => false,
                    Curve3::Nurbs(n) => n.degree() > 1,
                    _ => true,
                })
                .unwrap_or(false);
            if curved {
                // Sample the 3D edge curve (it lies in the plane). An
                // OPEN circle/ellipse arc samples its TRUE EXTENT by
                // endpoint angles + arc_sweep, in the fin's traversal
                // direction; the full-periodic sweep here corrupted
                // any planar polygon with an open-arc boundary (the
                // fillet end caps), visible whenever such a face
                // carries x-flux.
                if let Some((eck, sense)) = self.edges.get(fin.edge).and_then(|e| e.curve)
                    && let Some(ec) = self.curves.get(eck)
                {
                    let edge = self.edges.get(fin.edge).ok_or(TopoError::StaleKey)?;
                    let (b0, b1) = edge.bounds;
                    let p0 = self
                        .vertices
                        .get(b0)
                        .map(|x| x.point)
                        .ok_or(TopoError::StaleKey)?;
                    let p1 = self
                        .vertices
                        .get(b1)
                        .map(|x| x.point)
                        .ok_or(TopoError::StaleKey)?;
                    let tau = core::f64::consts::TAU;
                    let arc_range = |ang0: f64, ang1: f64| -> (f64, f64) {
                        // Edge-direction sweep (bounds.0 -> bounds.1),
                        // honoring an explicit arc_sweep; then flip for
                        // a reversed fin.
                        let sweep_edge = edge.arc_sweep.unwrap_or_else(|| {
                            let mut d = ang1 - ang0;
                            let pi = core::f64::consts::PI;
                            while d <= -pi {
                                d += tau;
                            }
                            while d > pi {
                                d -= tau;
                            }
                            d
                        });
                        if fin.forward {
                            (ang0, sweep_edge)
                        } else {
                            (ang1, -sweep_edge)
                        }
                    };
                    match ec {
                        Curve3::Circle(c) if b0 != b1 => {
                            let ang = |p: keel_math::vec::Vec3| {
                                let d = p - c.center;
                                d.dot(c.y_axis).atan2(d.dot(c.x_axis))
                            };
                            let (t0, sweep) = arc_range(ang(p0), ang(p1));
                            let pt = |t: f64| c.point(t);
                            let dpt = |t: f64| (c.y_axis * t.cos() - c.x_axis * t.sin()) * c.radius;
                            for i in 0..SAMPLES {
                                let t = t0 + sweep * i as f64 / SAMPLES as f64;
                                poly.push(uv(c.point(t)));
                                let tn = t0 + sweep * (i + 1) as f64 / SAMPLES as f64;
                                lune(&pt, &dpt, t, tn, &mut lunes);
                            }
                        }
                        Curve3::Ellipse(e) if b0 != b1 => {
                            let ang = |p: keel_math::vec::Vec3| {
                                let d = p - e.center;
                                (d.dot(e.y_axis) / e.b).atan2(d.dot(e.x_axis) / e.a)
                            };
                            let (t0, sweep) = arc_range(ang(p0), ang(p1));
                            let pt = |t: f64| e.point(t);
                            let dpt =
                                |t: f64| e.y_axis * (e.b * t.cos()) - e.x_axis * (e.a * t.sin());
                            for i in 0..SAMPLES {
                                let t = t0 + sweep * i as f64 / SAMPLES as f64;
                                poly.push(uv(e.point(t)));
                                let tn = t0 + sweep * (i + 1) as f64 / SAMPLES as f64;
                                lune(&pt, &dpt, t, tn, &mut lunes);
                            }
                        }
                        _ => {
                            let smap = |i: usize| {
                                let s = i as f64 / SAMPLES as f64;
                                if fin.forward == sense { s } else { 1.0 - s }
                            };
                            for i in 0..SAMPLES {
                                let s = smap(i);
                                let p = match ec {
                                    Curve3::Circle(c) => c.point(tau * s),
                                    Curve3::Ellipse(e) => e.point(tau * s),
                                    Curve3::Nurbs(n) => {
                                        let (a, b) = n.domain();
                                        n.point(a + s * (b - a))
                                    }
                                    Curve3::Line(l) => l.point(s),
                                };
                                poly.push(uv(p));
                                // Closed rings (annulus boundaries) get the
                                // same lune closure; NURBS above degree 1
                                // stay chordal.
                                match ec {
                                    Curve3::Circle(c) => {
                                        let pt = |t: f64| c.point(t);
                                        let dpt = |t: f64| {
                                            (c.y_axis * t.cos() - c.x_axis * t.sin()) * c.radius
                                        };
                                        lune(&pt, &dpt, tau * s, tau * smap(i + 1), &mut lunes);
                                    }
                                    Curve3::Ellipse(e) => {
                                        let pt = |t: f64| e.point(t);
                                        let dpt = |t: f64| {
                                            e.y_axis * (e.b * t.cos()) - e.x_axis * (e.a * t.sin())
                                        };
                                        lune(&pt, &dpt, tau * s, tau * smap(i + 1), &mut lunes);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            } else {
                let p = self
                    .fin_start_vertex(cur)
                    .and_then(|v| self.vertices.get(v).map(|x| x.point))
                    .ok_or(TopoError::StaleKey)?;
                poly.push(uv(p));
            }
            cur = fin.next;
            if cur == entry {
                break;
            }
        }
        Ok((poly, lunes))
    }

    /// UV bounds of a curved face whose boundary is ISO-RECTANGULAR:
    /// every boundary edge projects to an iso-u or iso-v line of the
    /// surface (a fillet band, a torus rim ring, an octant cylinder
    /// band). Periodic coordinates are spanned by the LARGEST-GAP
    /// complement over all boundary samples (full ring when no gap),
    /// and iso checks run in the span frame so seam-crossing bands
    /// resolve. `None` when any edge is not iso-parameter (the sphere
    /// octant, the mitre's ellipse-bounded band): those decline.
    fn projected_rect_bounds(
        &self,
        fk: FaceKey,
        surf: &Surface3,
    ) -> Option<((f64, f64), (f64, f64))> {
        let tau = core::f64::consts::TAU;
        let (u_per, v_per) = match surf {
            Surface3::Cylinder(_) | Surface3::Cone(_) | Surface3::Sphere(_) => (true, false),
            Surface3::Torus(_) => (true, true),
            _ => (false, false),
        };
        let face = self.faces.get(fk)?;
        let mut edges_uv: Vec<Vec<(f64, f64)>> = Vec::new();
        for &lk in &face.loops {
            // Vertex loops (a pole) bound no edges and constrain no UV
            // direction; skip them.
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            loop {
                // Sample the edge's TRUE extent: open circle/ellipse
                // arcs by endpoint angles + arc_sweep (the generic
                // fin sampler sweeps the FULL periodic curve for
                // those, which would balloon the angular span).
                let fin = self.fins.get(cur)?;
                let edge = self.edges.get(fin.edge)?;
                let (a, b) = edge.bounds;
                let pa = self.vertices.get(a)?.point;
                let pb = self.vertices.get(b)?.point;
                let curve = edge.curve.and_then(|(ck, _)| self.curves.get(ck));
                // Closed rings sample DENSELY (64): the full-ring
                // detection reads the largest sample gap, so sparse
                // ring samples would fake a boundary gap.
                let pts: Vec<Vec3> = match curve {
                    Some(Curve3::Circle(ci)) => {
                        let ang = |p: Vec3| {
                            let d = p - ci.center;
                            d.dot(ci.y_axis).atan2(d.dot(ci.x_axis))
                        };
                        if a == b {
                            (0..64).map(|k| ci.point(tau * k as f64 / 64.0)).collect()
                        } else {
                            let sweep = edge.arc_sweep.unwrap_or_else(|| {
                                let mut d = ang(pb) - ang(pa);
                                let pi = core::f64::consts::PI;
                                while d <= -pi {
                                    d += tau;
                                }
                                while d > pi {
                                    d -= tau;
                                }
                                d
                            });
                            let t0 = ang(pa);
                            (0..=8)
                                .map(|k| ci.point(t0 + sweep * k as f64 / 8.0))
                                .collect()
                        }
                    }
                    Some(Curve3::Ellipse(el)) => {
                        let ang = |p: Vec3| {
                            let d = p - el.center;
                            (d.dot(el.y_axis) / el.b).atan2(d.dot(el.x_axis) / el.a)
                        };
                        if a == b {
                            (0..64).map(|k| el.point(tau * k as f64 / 64.0)).collect()
                        } else {
                            let mut sweep = ang(pb) - ang(pa);
                            let pi = core::f64::consts::PI;
                            while sweep <= -pi {
                                sweep += tau;
                            }
                            while sweep > pi {
                                sweep -= tau;
                            }
                            let t0 = ang(pa);
                            (0..=8)
                                .map(|k| el.point(t0 + sweep * k as f64 / 8.0))
                                .collect()
                        }
                    }
                    Some(Curve3::Nurbs(_)) => self.fin_curve_samples(cur, 9)?,
                    _ => (0..=4).map(|t| pa + (pb - pa) * (t as f64 / 4.0)).collect(),
                };
                let mut uvs = Vec::with_capacity(pts.len());
                for p in pts {
                    let pr = surf.project(p).ok()?;
                    uvs.push((pr.u, pr.v));
                }
                edges_uv.push(uvs);
                cur = self.fins.get(cur)?.next;
                if cur == entry {
                    break;
                }
            }
        }
        if edges_uv.is_empty() {
            return None;
        }
        let span_of = |idx: usize, periodic: bool| -> Option<(f64, f64)> {
            let mut vals: Vec<f64> = edges_uv
                .iter()
                .flatten()
                .map(|uv| if idx == 0 { uv.0 } else { uv.1 })
                .collect();
            if !periodic {
                let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                return (lo.is_finite() && hi > lo).then_some((lo, hi));
            }
            for v in vals.iter_mut() {
                *v = v.rem_euclid(tau);
            }
            vals.sort_by(f64::total_cmp);
            let n = vals.len();
            let mut best_gap = tau - (vals[n - 1] - vals[0]);
            let mut lo = vals[0];
            for i in 1..n {
                let g = vals[i] - vals[i - 1];
                if g > best_gap {
                    best_gap = g;
                    lo = vals[i];
                }
            }
            // Full ring iff the largest gap is explained by the ring
            // sampling density (64 per closed edge): a genuine
            // boundary gap is wider.
            if best_gap <= 1.6 * tau / 64.0 {
                return Some((0.0, tau));
            }
            Some((lo, lo + (tau - best_gap)))
        };
        let (u0, u1) = span_of(0, u_per)?;
        let (v0, v1) = span_of(1, v_per)?;
        let remap = |x: f64, lo: f64, periodic: bool| {
            if periodic {
                lo + (x - lo).rem_euclid(tau)
            } else {
                x
            }
        };
        // Wrap-tolerant remap: a sample exactly at the span END maps to
        // lo + ~tau, not lo; accept both frames in the iso test.
        const ISO_TOL: f64 = 1e-6;
        for uvs in &edges_uv {
            let us: Vec<f64> = uvs.iter().map(|&(u, _)| remap(u, u0, u_per)).collect();
            let vs: Vec<f64> = uvs.iter().map(|&(_, v)| remap(v, v0, v_per)).collect();
            let near = |a: f64, b: f64, periodic: bool| {
                let d = (a - b).abs();
                if periodic {
                    d.min((d - tau).abs()) <= ISO_TOL
                } else {
                    d <= ISO_TOL
                }
            };
            let const_u = us.iter().all(|&u| near(u, us[0], u_per));
            let const_v = vs.iter().all(|&v| near(v, vs[0], v_per));
            if !(const_u || const_v) {
                return None;
            }
        }
        Some(((u0, u1), (v0, v1)))
    }

    /// Curved face: composite GL over the parameter rectangle. The
    /// parameter rectangle [u0,u1]x[v0,v1] is integrated in the natural
    /// (increasing) direction, so the natural normal is du x dv and the
    /// outward normal is sense * natural (file 46) -- no winding factor.
    fn integrate_curved_face(
        &self,
        fk: FaceKey,
        surf: &Surface3,
        sense_sign: f64,
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
            // Bounds from the pcurve polylines, GUARDED for staleness:
            // surgery can rebind a face to a new surface (the cap-rim
            // kef merge leaves cylinder-space pcurves on the torus
            // ring), so a pcurve only counts if its endpoints EVALUATE
            // onto the fin's 3D endpoints through THIS surface. Faces
            // whose pcurves fail the guard (or have none) fall to the
            // projected ISO-RECTANGLE bounds below (the corpus-audit
            // blend-pcurve milestone).
            let face = self.faces.get(fk).ok_or(TopoError::StaleKey)?;
            let mut lo = (f64::INFINITY, f64::INFINITY);
            let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
            let mut stale = false;
            for &lk in &face.loops {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                while let Some(fin) = self.fins.get(cur) {
                    if let Some((ck, _)) = fin.pcurve
                        && let Some(Curve3::Nurbs(n)) = self.curves.get(ck)
                    {
                        // Staleness guard: the pcurve endpoint must
                        // evaluate (through THIS surface) onto one of
                        // the fin's 3D endpoints.
                        let va = self
                            .fin_start_vertex(cur)
                            .and_then(|v| self.vertices.get(v))
                            .map(|x| x.point);
                        let vb = self
                            .fin_end_vertex(cur)
                            .and_then(|v| self.vertices.get(v))
                            .map(|x| x.point);
                        for t in [0.0, 1.0] {
                            let p = n.point(t);
                            // A pole-adjacent evaluation failure is
                            // INCONCLUSIVE, not stale.
                            if let Ok(lg) = surf.local_geometry(p.x, p.y) {
                                let on_a = va.map(|q| (lg.point - q).norm() < 1e-6);
                                let on_b = vb.map(|q| (lg.point - q).norm() < 1e-6);
                                if on_a != Some(true) && on_b != Some(true) {
                                    stale = true;
                                }
                            }
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
            if stale || !(lo.0.is_finite() && hi.0.is_finite()) {
                // Stale or missing pcurves: the PROJECTED-BOUNDS rung
                // (corpus-audit blend-pcurve milestone). When every
                // boundary edge is an ISO-PARAMETER line of the
                // surface, the face IS its UV rectangle and the
                // rectangle integral is exact; bounds come from
                // projecting boundary samples, with periodic
                // directions resolved by the largest-gap span (the
                // cyl_angular_span idea). Non-rectangular curved faces
                // keep declining.
                match self.projected_rect_bounds(fk, surf) {
                    Some(b) => b,
                    None => {
                        // Non-iso-rectangle CYLINDER trims (the mitre's
                        // ellipse seams, the oblique-end cap) integrate
                        // via the Green-slab boundary path; other
                        // surfaces keep declining.
                        if matches!(surf, Surface3::Cylinder(_)) {
                            return self.integrate_cylinder_face_green(fk, surf, sense_sign, m);
                        }
                        return Err(TopoError::Precondition("curved face without pcurve bounds"));
                    }
                }
            } else {
                ((lo.0, hi.0), (lo.1, hi.1))
            }
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
                        let n = lg.du.cross(lg.dv) * sense_sign;
                        let w = wu * wv * 0.25 * (ub - ua) * (vb - va);
                        m.add(lg.point, n, w);
                    }
                }
            }
        }
        Ok(())
    }

    /// GREEN-SLAB integrator for a cylinder face whose UV trim is NOT
    /// an iso-rectangle (the mitre's ellipse seams, the oblique-end
    /// cap). The region integral folds onto the boundary,
    /// `int_R F du dv = -loop_int G du` with `G(u, v) = int_{v0}^{v}
    /// F(u, s) ds`, and each boundary Gauss node carries an inner
    /// v-slab of quadrature samples, so the ordinary Moments::add
    /// machinery integrates any trim whose fins are evaluable with
    /// derivatives: lines and degree-1 NURBS are rulings and
    /// contribute zero through u' = 0; circles and ellipses carry the
    /// flux. The integrand is u-periodic, so fins need only LOCAL
    /// angle continuity (no global seam unwrap), and weights are
    /// normalized by the sign of the enclosed UV area to match the
    /// positively-covered rectangle convention of the iso path.
    /// Curved NURBS fins decline.
    fn integrate_cylinder_face_green(
        &self,
        fk: FaceKey,
        surf: &Surface3,
        sense_sign: f64,
        m: &mut Moments,
    ) -> Result<(), TopoError> {
        let Surface3::Cylinder(cyl) = surf else {
            return Err(TopoError::Precondition("green-slab: not a cylinder"));
        };
        let (o, ex, ey, ez, r) = (
            cyl.frame.origin,
            cyl.frame.x,
            cyl.frame.y,
            cyl.frame.z,
            cyl.radius,
        );
        enum FinCurve {
            Seg(Vec3, Vec3),
            Circ(keel_geom::curve::Circle3, f64, f64),
            Ell(keel_geom::curve::Ellipse3, f64, f64),
        }
        let tau = core::f64::consts::TAU;
        let face = self.faces.get(fk).ok_or(TopoError::StaleKey)?;
        let mut fins_c: Vec<FinCurve> = Vec::new();
        let mut v_base = f64::INFINITY;
        for &lk in &face.loops {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            loop {
                let fin = self.fins.get(cur).ok_or(TopoError::StaleKey)?;
                let edge = self.edges.get(fin.edge).ok_or(TopoError::StaleKey)?;
                let (b0, b1) = edge.bounds;
                let p0 = self
                    .vertices
                    .get(b0)
                    .map(|x| x.point)
                    .ok_or(TopoError::StaleKey)?;
                let p1 = self
                    .vertices
                    .get(b1)
                    .map(|x| x.point)
                    .ok_or(TopoError::StaleKey)?;
                v_base = v_base.min((p0 - o).dot(ez)).min((p1 - o).dot(ez));
                // Fin-ordered parameter range for an angle-parameterized
                // conic: edge-direction sweep (bounds.0 -> bounds.1,
                // honoring an explicit arc_sweep), flipped when the fin
                // runs the edge backward.
                let range = |a0: f64, a1: f64| -> (f64, f64) {
                    let s = edge.arc_sweep.unwrap_or_else(|| {
                        let mut d = a1 - a0;
                        let pi = core::f64::consts::PI;
                        while d <= -pi {
                            d += tau;
                        }
                        while d > pi {
                            d -= tau;
                        }
                        d
                    });
                    if fin.forward {
                        (a0, a0 + s)
                    } else {
                        (a1, a1 - s)
                    }
                };
                match edge.curve.and_then(|(ck, _)| self.curves.get(ck)) {
                    Some(Curve3::Circle(c)) => {
                        let ang = |p: Vec3| {
                            (p - c.center)
                                .dot(c.y_axis)
                                .atan2((p - c.center).dot(c.x_axis))
                        };
                        if b0 == b1 {
                            let a0 = ang(p0);
                            let s = if fin.forward { tau } else { -tau };
                            fins_c.push(FinCurve::Circ(*c, a0, a0 + s));
                        } else {
                            let (t0, t1) = range(ang(p0), ang(p1));
                            fins_c.push(FinCurve::Circ(*c, t0, t1));
                        }
                    }
                    Some(Curve3::Ellipse(el)) => {
                        let ang = |p: Vec3| {
                            let w = p - el.center;
                            (w.dot(el.y_axis) / el.b).atan2(w.dot(el.x_axis) / el.a)
                        };
                        if b0 == b1 {
                            let a0 = ang(p0);
                            let s = if fin.forward { tau } else { -tau };
                            fins_c.push(FinCurve::Ell(*el, a0, a0 + s));
                        } else {
                            let (t0, t1) = range(ang(p0), ang(p1));
                            fins_c.push(FinCurve::Ell(*el, t0, t1));
                        }
                    }
                    Some(Curve3::Nurbs(n)) if n.degree() > 1 => {
                        return Err(TopoError::Precondition(
                            "green-slab: curved NURBS boundary fin",
                        ));
                    }
                    _ => {
                        let (ps, pe) = if fin.forward { (p0, p1) } else { (p1, p0) };
                        fins_c.push(FinCurve::Seg(ps, pe));
                    }
                }
                cur = fin.next;
                if cur == entry {
                    break;
                }
            }
        }
        if !v_base.is_finite() {
            return Err(TopoError::Precondition("green-slab: empty boundary"));
        }
        let mut acc: Vec<(f64, f64, f64)> = Vec::new();
        let mut area = 0.0;
        let mut emit = |p: Vec3, dp: Vec3, wt: f64| {
            let w = p - o;
            let (x, y) = (w.dot(ex), w.dot(ey));
            let nrm = (x * x + y * y).sqrt();
            if nrm < 1e-12 {
                return;
            }
            let (cu, su) = (x / nrm, y / nrm);
            let theta_hat = ey * cu - ex * su;
            let up = dp.dot(theta_hat) / r;
            let u = su.atan2(cu);
            let v_t = w.dot(ez);
            let wu = wt * up;
            area -= wu * (v_t - v_base);
            let half = 0.5 * (v_t - v_base);
            for (xk, wk) in GL8_X.iter().zip(GL8_W) {
                let s = v_base + half * (xk + 1.0);
                acc.push((u, s, -wu * wk * half));
            }
        };
        for fc in &fins_c {
            match fc {
                FinCurve::Seg(a, b2) => {
                    for (xj, wj) in GL8_X.iter().zip(GL8_W) {
                        let t = 0.5 * (xj + 1.0);
                        emit(*a + (*b2 - *a) * t, *b2 - *a, wj * 0.5);
                    }
                }
                FinCurve::Circ(c, t0, t1) => {
                    let panels = ((t1 - t0).abs() / core::f64::consts::FRAC_PI_4)
                        .ceil()
                        .max(1.0) as usize;
                    for ip in 0..panels {
                        let a0 = t0 + (t1 - t0) * ip as f64 / panels as f64;
                        let a1 = t0 + (t1 - t0) * (ip + 1) as f64 / panels as f64;
                        for (xj, wj) in GL8_X.iter().zip(GL8_W) {
                            let t = 0.5 * (a0 + a1) + 0.5 * (a1 - a0) * xj;
                            let dp = (c.y_axis * t.cos() - c.x_axis * t.sin()) * c.radius;
                            emit(c.point(t), dp, wj * 0.5 * (a1 - a0));
                        }
                    }
                }
                FinCurve::Ell(el, t0, t1) => {
                    let panels = ((t1 - t0).abs() / core::f64::consts::FRAC_PI_4)
                        .ceil()
                        .max(1.0) as usize;
                    for ip in 0..panels {
                        let a0 = t0 + (t1 - t0) * ip as f64 / panels as f64;
                        let a1 = t0 + (t1 - t0) * (ip + 1) as f64 / panels as f64;
                        for (xj, wj) in GL8_X.iter().zip(GL8_W) {
                            let t = 0.5 * (a0 + a1) + 0.5 * (a1 - a0) * xj;
                            let dp = el.y_axis * (el.b * t.cos()) - el.x_axis * (el.a * t.sin());
                            emit(el.point(t), dp, wj * 0.5 * (a1 - a0));
                        }
                    }
                }
            }
        }
        if area.abs() < 1e-12 {
            return Err(TopoError::Precondition("green-slab: degenerate UV region"));
        }
        let flip = area.signum();
        for (u, s, w) in acc {
            let Ok(lg) = surf.local_geometry(u, s) else {
                continue;
            };
            let n = lg.du.cross(lg.dv) * sense_sign;
            m.add(lg.point, n, w * flip);
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
