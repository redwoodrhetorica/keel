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

/// Chirality of an analytic surface's parametrization frame: `+1.0` for a
/// right-handed (proper) frame, `-1.0` for a left-handed (REFLECTED) frame.
/// The curved mass integrators build the natural surface normal from
/// `du x dv`, whose sign tracks the frame handedness (a cross product is a
/// pseudovector). A mirror sends a frame left-handed, flipping `du x dv` so
/// it points INWARD; folding this factor restores the geometry-outward
/// orientation the mesh uses (dossier 72). For every non-mirrored body the
/// frame is right-handed and this is `+1.0` (a no-op).
fn frame_handedness(surf: &Surface3) -> f64 {
    let f = match surf {
        Surface3::Plane(p) => &p.frame,
        Surface3::Cylinder(c) => &c.frame,
        Surface3::Cone(c) => &c.frame,
        Surface3::Sphere(s) => &s.frame,
        Surface3::Torus(t) => &t.frame,
    };
    if f.x.dot(f.y.cross(f.z)) >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

impl Body {
    /// Accumulate the divergence-theorem moments over every solid
    /// boundary face. The outward normal is `sense * natural`
    /// (research file 46), folded with each face's own loop winding (and,
    /// for curved faces, the surface-frame handedness, dossier 72);
    /// interior partition walls cancel and are skipped.
    fn accumulate_moments(&self) -> Result<Moments, TopoError> {
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
            let v_before = m.v;
            match surf {
                Surface3::Plane(_) => self.integrate_planar_face(fk, surf, sense_sign, &mut m)?,
                // CURVED faces take the surface-frame HANDEDNESS factor (dossier
                // 72): the curved integrators build the natural normal from
                // `du x dv`, a pseudovector that flips sign when the surface
                // frame is REFLECTED (a mirrored operand, det < 0). The mesh
                // path orients curved facets from GEOMETRY (e.g. `q - centre`),
                // which does NOT flip, so without this factor a mirrored curved
                // lump's analytic flux points inward and SUBTRACTS (the sphere
                // mirror+union collapse). Folding `handedness = sign(det frame)`
                // makes `handedness * (du x dv)` frame-chirality-invariant, so
                // it agrees with the mesh for both proper (det +1, a NO-OP that
                // leaves every existing curved case bit-identical) and reflected
                // (det -1) frames.
                _ => {
                    let h = frame_handedness(surf);
                    self.integrate_curved_face(fk, surf, sense_sign * h, &mut m)?
                }
            }
            if std::env::var("KEEL_MASS_DEBUG").is_ok() {
                eprintln!("  mass face {fk:?} sense {sense} dv {}", m.v - v_before);
            }
        }
        Ok(m)
    }

    /// Mass properties of the body's solid regions (unit density).
    pub fn mass_properties(&self) -> Result<MassProps, TopoError> {
        let _prof = crate::profile::Scope::new(&crate::profile::MASS_NS);
        crate::profile::count(&crate::profile::MASS_CALLS);
        let m = self.accumulate_moments()?;
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
                            // Ring density (64 per full turn): a rim
                            // SPLIT into open halves must not sample
                            // sparser than an intact ring, or the
                            // largest-gap full-wrap detection reads
                            // the sampling gap as a real one (task 36:
                            // the union cap's u span lost pi/8).
                            let n = ((sweep.abs() * 64.0 / tau).ceil() as usize).max(8);
                            (0..=n)
                                .map(|k| ci.point(t0 + sweep * k as f64 / n as f64))
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
                            // Ring density: see the circle arm.
                            let n = ((sweep.abs() * 64.0 / tau).ceil() as usize).max(8);
                            (0..=n)
                                .map(|k| el.point(t0 + sweep * k as f64 / n as f64))
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
        // Full-coverage short-circuits only for genuinely CLOSED
        // surfaces. An OPEN surface (cylinder/cone) can read as
        // "covers" when its rim circles are per-face closed edges
        // (the drilled-plate lateral, whose rims pair with the cap
        // annuli as coincident closed edges rather than shared ones):
        // those faces are bounded bands and take the bounds path.
        let closed_cover = self.face_covers_closed_surface(fk)
            && matches!(surf, Surface3::Sphere(_) | Surface3::Torus(_));
        let ((u0, u1), (v0, v1)) = if closed_cover {
            match surf {
                Surface3::Sphere(_) => (
                    (0.0, tau),
                    (-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2),
                ),
                Surface3::Torus(_) => ((0.0, tau), (0.0, tau)),
                _ => unreachable!(),
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
            // A fin whose pcurve varies in BOTH u and v is not an
            // iso-parameter line: the face is not its endpoint box
            // (task 29: the bicylinder bowtie's pole-to-pole arcs gave
            // a zero-height v box and a silent dv = 0).
            let mut non_iso = false;
            // Vertex-projected v-extent: the second staleness witness.
            // A fragment can carry its PARENT's pcurves whose endpoints
            // happen to sit on fin endpoints of sibling fins (the
            // drilled-plate lateral kept the whole drill's [0, 2] span
            // while its own rims sit at [0.5, 1.5]): the pcurve box
            // must not exceed where the boundary vertices actually are.
            let mut vert_v = (f64::INFINITY, f64::NEG_INFINITY);
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
                        let (p0, pm, p1) = (n.point(0.0), n.point(0.5), n.point(1.0));
                        let us = [p0.x, pm.x, p1.x];
                        let vs = [p0.y, pm.y, p1.y];
                        let spread = |s: [f64; 3]| {
                            s.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                                - s.iter().cloned().fold(f64::INFINITY, f64::min)
                        };
                        if spread(us) > 1e-7 && spread(vs) > 1e-7 {
                            non_iso = true;
                        }
                    }
                    // Vertex witness for the v-extent guard.
                    for vk in [self.fin_start_vertex(cur), self.fin_end_vertex(cur)] {
                        if let Some(p) = vk.and_then(|v| self.vertices.get(v)).map(|x| x.point)
                            && let Ok(pr) = surf.project(p)
                        {
                            vert_v = (vert_v.0.min(pr.v), vert_v.1.max(pr.v));
                        }
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
            }
            // PERIODIC u: the pcurve endpoint box can span two
            // revolutions when the two rims parameterize opposite
            // turns (the drilled-plate lateral: one rim [-tau, 0],
            // the other [0, tau]); a face never covers more than one.
            if hi.0 - lo.0 > tau {
                hi.0 = lo.0 + tau;
            }
            // The pcurve box exceeding the vertex extent is the second
            // staleness witness (parent pcurves on a kept fragment).
            if vert_v.0.is_finite() && (lo.1 < vert_v.0 - 1e-6 || hi.1 > vert_v.1 + 1e-6) {
                stale = true;
            }
            // A DEGENERATE box (zero width or height) cannot be the
            // face's region: a split sphere piece's only pcurves are
            // its rim arcs, all at ONE latitude, and the GL rectangle
            // integrated dv = 0 SILENTLY (the task-36 trap, defect 1).
            let degenerate_box = lo.0.is_finite() && (hi.0 - lo.0 <= 1e-9 || hi.1 - lo.1 <= 1e-9);
            if std::env::var("KEEL_MASS_DEBUG").is_ok() {
                eprintln!(
                    "  mass curved {fk:?} stale {stale} non_iso {non_iso} degenerate {degenerate_box} lo {lo:?} hi {hi:?} vert_v {vert_v:?}"
                );
            }
            if stale || non_iso || degenerate_box || !(lo.0.is_finite() && hi.0.is_finite()) {
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
                        // Non-iso-rectangle CYLINDER and SPHERE trims
                        // (the mitre's ellipse seams, the oblique-end
                        // cap, the corner sphere triangle) integrate
                        // via the Green-slab boundary path; other
                        // surfaces keep declining.
                        if matches!(
                            surf,
                            Surface3::Cylinder(_) | Surface3::Sphere(_) | Surface3::Cone(_)
                        ) {
                            return self.integrate_face_green(fk, surf, sense_sign, m);
                        }
                        return Err(TopoError::Precondition("curved face without pcurve bounds"));
                    }
                }
            } else {
                ((lo.0, hi.0), (lo.1, hi.1))
            }
        };
        // RECTANGLE WITNESS for sphere trims (task 36): an iso pcurve
        // box is only the face's region if its spherical area matches
        // the face's own tessellation. The union cap's two half-rim
        // pcurves have endpoints at u = 0 and pi only, so the endpoint
        // box spanned HALF the period and integrated half the flux
        // (a wrong-positive the mass==mesh band did not catch).
        if !closed_cover && let Surface3::Sphere(s) = surf {
            let rect_area = (u1 - u0).abs() * (v1.sin() - v0.sin()).abs() * s.radius * s.radius;
            let tess_area: f64 = self
                .tessellate_face(fk)
                .iter()
                .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
                .sum();
            if (rect_area - tess_area).abs() > 5e-2 * (1.0 + tess_area) {
                return self.integrate_face_green(fk, surf, sense_sign, m);
            }
        }
        // RECTANGLE WITNESS for cone trims (dossier 60 sec 3, the cone dual
        // of the sphere witness above, gap c). A notched / partial-azimuth
        // cone fragment whose cut edges carry no NURBS pcurve keeps the
        // FULL-azimuth iso box and the iso-rectangle integral OVER-counts the
        // removed wedge (the block/cone over-count, mass 17.07 vs true 11.85).
        // The analytic band lateral area is |u1-u0| * sqrt(1+slope^2) *
        // integral_v (r0 + v*slope); when it disagrees with the face's own
        // tessellation the box is not the face's region, so integrate over the
        // actual boundary via the Green-slab (now cone-apex-anchored). A clean
        // full band/tip matches within the tessellation coarseness and stays
        // on the exact iso path.
        if !closed_cover && let Surface3::Cone(c) = surf {
            let slope = c.half_angle.tan();
            let r_int = c.radius * (v1 - v0) + 0.5 * slope * (v1 * v1 - v0 * v0);
            let rect_area = ((u1 - u0) * (1.0 + slope * slope).sqrt() * r_int).abs();
            let tess_area: f64 = self
                .tessellate_face(fk)
                .iter()
                .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
                .sum();
            if (rect_area - tess_area).abs() > 6e-2 * (1.0 + tess_area) {
                return self.integrate_face_green(fk, surf, sense_sign, m);
            }
        }
        // RECTANGLE WITNESS for cylinder trims (dossier 66, the cylinder dual
        // of the cone/sphere witnesses above). A HOLED wall (a lateral carrying
        // inner bore-hole loops) or a partial-azimuth band whose cut edges have
        // no pcurve keeps the FULL iso box, and the iso-rectangle integral
        // OVER-counts the removed material (the cyl/cyl over-read: A u B mass
        // 17.09 = V(A)+V(B) with the bore not subtracted). The analytic band
        // lateral area is |u1-u0| * r * |v1-v0| (the cylinder's unit Jacobian r
        // is constant; a full band is 2*pi*r*h). When it disagrees with the
        // face's own tessellation the box is NOT the face's region, so
        // integrate over the ACTUAL loops via the Green-slab (which iterates
        // every loop, so CW inner-hole loops subtract their flux). A clean full
        // or partial band matches within the tessellation coarseness and stays
        // on the exact iso path.
        if !closed_cover && let Surface3::Cylinder(c) = surf {
            let rect_area = ((u1 - u0) * c.radius * (v1 - v0)).abs();
            let tess_area: f64 = self
                .tessellate_face(fk)
                .iter()
                .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
                .sum();
            if (rect_area - tess_area).abs() > 6e-2 * (1.0 + tess_area) {
                return self.integrate_face_green(fk, surf, sense_sign, m);
            }
        }
        if std::env::var("KEEL_MASS_DEBUG").is_ok() {
            eprintln!("  rect {fk:?} u [{u0:.4}, {u1:.4}] v [{v0:.4}, {v1:.4}]");
        }
        // Panels per axis. The u-integrand carries the azimuthal harmonics
        // (cos/sin products up to ~degree 3 for the inertia tensor), so it
        // keeps the subdivision. The v-integrand is a low-degree POLYNOMIAL for
        // a cylinder (radius constant -> moments degree <=2 in v) and a cone
        // (radius linear -> degree <=4), which a SINGLE GL8 panel (exact to
        // degree 15) integrates EXACTLY; only the sphere's latitude (trig in v)
        // needs v-subdivision. (A plain cylinder lateral was 16x16x64 = 16384
        // nodes -> 2.1 ms; nv=1 makes it 1024 nodes, exact, the OPT win.)
        let nu = 16usize;
        let nv = match surf {
            // v-integrand is a low-degree POLYNOMIAL -> one GL8 panel is exact.
            Surface3::Cylinder(_) | Surface3::Cone(_) => 1usize,
            // Sphere (cos v) and torus (minor-circle trig) need v-subdivision.
            _ => 16usize,
        };
        for iu in 0..nu {
            let ua = u0 + (u1 - u0) * iu as f64 / nu as f64;
            let ub = u0 + (u1 - u0) * (iu + 1) as f64 / nu as f64;
            for iv in 0..nv {
                let va = v0 + (v1 - v0) * iv as f64 / nv as f64;
                let vb = v0 + (v1 - v0) * (iv + 1) as f64 / nv as f64;
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

    /// GREEN-SLAB integrator for a cylinder or sphere face whose UV
    /// trim is NOT an iso-rectangle (the mitre's ellipse seams, the
    /// oblique-end cap, the corner sphere triangle). The region
    /// integral folds onto the boundary,
    /// `int_R F du dv = -loop_int G du` with `G(u, v) = int_{vb}^{v}
    /// F(u, s) ds`, and each boundary Gauss node carries an inner
    /// v-slab of quadrature samples, so the ordinary Moments::add
    /// machinery integrates any trim whose fins are evaluable with
    /// derivatives: lines and degree-1 NURBS are cylinder rulings and
    /// contribute zero through u' = 0; circles and ellipses carry the
    /// flux. The integrand is u-periodic, so fins need only LOCAL
    /// angle continuity (no global seam unwrap), and weights are
    /// normalized by the sign of the enclosed UV area to match the
    /// positively-covered rectangle convention of the iso path.
    ///
    /// The slab base `vb` matters exactly when the boundary WINDS in
    /// u (a sphere trim enclosing a pole): then `loop_int g(u) du`
    /// of the base shift does not vanish, and the base must sit at
    /// the enclosed pole. Total winding 0 (cylinder trims, sphere
    /// bands) takes the boundary minimum; winding +-1 anchors at the
    /// NORTH pole of the surface frame (the corner-patch constructors
    /// put the patch's interior pole at frame +z; a trim enclosing
    /// only the south pole is out of contract); |winding| > 1 and
    /// curved NURBS fins decline.
    fn integrate_face_green(
        &self,
        fk: FaceKey,
        surf: &Surface3,
        sense_sign: f64,
        m: &mut Moments,
    ) -> Result<(), TopoError> {
        let (frame, is_sphere, cone_apex) = match surf {
            Surface3::Cylinder(c) => (&c.frame, false, None),
            // The cone shares the cylinder's parameterization (u = azimuth,
            // v = axial height); its v-dependent radius lives in
            // local_geometry, which the generic flux integrand below samples.
            // The APEX is the axial height where the radius r0 + v*tan(alpha)
            // vanishes: v_apex = -r0 / tan(alpha). It anchors a full-revolution
            // cone-tip face the way the sphere pole anchors a polar cap
            // (dossier 60 section 1).
            Surface3::Cone(c) => {
                let m = c.half_angle.tan();
                let apex = if m.abs() > 1e-12 {
                    Some(-c.radius / m)
                } else {
                    None
                };
                (&c.frame, false, apex)
            }
            Surface3::Sphere(s) => (&s.frame, true, None),
            _ => return Err(TopoError::Precondition("green-slab: unsupported surface")),
        };
        let (o, ex, ey, ez) = (frame.origin, frame.x, frame.y, frame.z);
        enum FinCurve {
            Seg(Vec3, Vec3),
            Circ(keel_geom::curve::Circle3, f64, f64),
            Ell(keel_geom::curve::Ellipse3, f64, f64),
            // A general NURBS boundary fin (the curved SSI seam of two
            // crossing quadrics fit to a NURBS): integrated by GL8 over its
            // own parameter, using point + first derivative as the tangent.
            Nurbs(keel_geom::nurbs_curve::NurbsCurve, f64, f64),
        }
        // UV mapping: u = azimuth about ez; v = axial height
        // (cylinder) or latitude (sphere), matching local_geometry.
        let vmap = |w: Vec3| -> f64 {
            if is_sphere {
                let nrm = (w - ez * w.dot(ez)).norm();
                w.dot(ez).atan2(nrm)
            } else {
                w.dot(ez)
            }
        };
        let tau = core::f64::consts::TAU;
        let face = self.faces.get(fk).ok_or(TopoError::StaleKey)?;
        // Each entry tagged with its LOOP ordinal (debug attribution).
        let mut fins_c: Vec<(usize, FinCurve)> = Vec::new();
        let mut v_min = f64::INFINITY;
        let mut v_max = f64::NEG_INFINITY;
        for (li, &lk) in face.loops.iter().enumerate() {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            // Collect the loop's fins first. A SLIT pair (the same edge
            // traversed once each way within one loop: the seam bridge
            // running from a rim down to the enclosed pole on a split
            // sphere piece) contributes equal and opposite boundary
            // integrals: skip both fins, which also keeps the anchor
            // pole off the effective boundary (task 36, defect 2).
            let mut walk: Vec<crate::entity::FinKey> = Vec::new();
            let mut cur = entry;
            loop {
                walk.push(cur);
                let Some(f) = self.fins.get(cur) else { break };
                cur = f.next;
                if cur == entry {
                    break;
                }
            }
            let mut skip: Vec<crate::entity::FinKey> = Vec::new();
            for (i, &fa) in walk.iter().enumerate() {
                for &fb in walk.iter().skip(i + 1) {
                    let (Some(xa), Some(xb)) = (self.fins.get(fa), self.fins.get(fb)) else {
                        continue;
                    };
                    if xa.edge == xb.edge
                        && xa.forward != xb.forward
                        && !skip.contains(&fa)
                        && !skip.contains(&fb)
                    {
                        skip.push(fa);
                        skip.push(fb);
                    }
                }
            }
            for &cur in &walk {
                if skip.contains(&cur) {
                    continue;
                }
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
                for p in [p0, p1] {
                    let v = vmap(p - o);
                    v_min = v_min.min(v);
                    v_max = v_max.max(v);
                }
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
                            fins_c.push((li, FinCurve::Circ(*c, a0, a0 + s)));
                        } else {
                            let (t0, t1) = range(ang(p0), ang(p1));
                            fins_c.push((li, FinCurve::Circ(*c, t0, t1)));
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
                            fins_c.push((li, FinCurve::Ell(*el, a0, a0 + s)));
                        } else {
                            let (t0, t1) = range(ang(p0), ang(p1));
                            fins_c.push((li, FinCurve::Ell(*el, t0, t1)));
                        }
                    }
                    Some(Curve3::Nurbs(n)) if n.degree() > 1 => {
                        // Parameter range over the NURBS domain, matched to the
                        // edge direction (p0 -> p1) and flipped for a backward
                        // fin. Closed loops (b0 == b1) take the full domain.
                        let (da, db) = n.domain();
                        let fwd = (n.point(da) - p0).norm() <= (n.point(da) - p1).norm();
                        let (s0, s1) = if fwd { (da, db) } else { (db, da) };
                        let (t0, t1) = if fin.forward { (s0, s1) } else { (s1, s0) };
                        fins_c.push((li, FinCurve::Nurbs(n.clone(), t0, t1)));
                    }
                    _ => {
                        // Straight fins lie on a cylinder only (as
                        // rulings); a sphere boundary must be circles.
                        if is_sphere {
                            return Err(TopoError::Precondition(
                                "green-slab: non-circular sphere boundary fin",
                            ));
                        }
                        let (ps, pe) = if fin.forward { (p0, p1) } else { (p1, p0) };
                        fins_c.push((li, FinCurve::Seg(ps, pe)));
                    }
                }
            }
        }
        if !v_min.is_finite() {
            return Err(TopoError::Precondition("green-slab: empty boundary"));
        }
        // DEDUP doubled boundary fins (Add 292): a multi-cut stitch can emit a
        // loop that traverses its rim curve TWICE -- a sphere ball-cavity face
        // arrived with 4 circle fins covering [0,2pi] then [0,2pi] again, so the
        // boundary winding read +-2 (no slab anchor) AND the flux double-counted.
        // The second traversal is geometrically redundant: drop a fin whose
        // ORDERED 3-sample signature matches an earlier fin in the SAME loop.
        // That restores winding +-1 (the pole/apex anchor) and the single-cover
        // flux. Direction-sensitive: a forward+backward pair (a different
        // degeneracy) has its endpoints swapped, so it is NOT collapsed.
        {
            let sig = |fc: &FinCurve| -> [Vec3; 3] {
                match fc {
                    FinCurve::Seg(a, b) => [*a, (*a + *b) * 0.5, *b],
                    FinCurve::Circ(c, t0, t1) => {
                        [c.point(*t0), c.point(0.5 * (t0 + t1)), c.point(*t1)]
                    }
                    FinCurve::Ell(e, t0, t1) => {
                        [e.point(*t0), e.point(0.5 * (t0 + t1)), e.point(*t1)]
                    }
                    FinCurve::Nurbs(n, t0, t1) => {
                        [n.point(*t0), n.point(0.5 * (t0 + t1)), n.point(*t1)]
                    }
                }
            };
            let mut kept: Vec<(usize, FinCurve)> = Vec::with_capacity(fins_c.len());
            let mut sigs: Vec<(usize, [Vec3; 3])> = Vec::new();
            for (li, fc) in fins_c.into_iter() {
                let s = sig(&fc);
                let dup = sigs.iter().any(|(kli, ks)| {
                    *kli == li
                        && ks
                            .iter()
                            .zip(s.iter())
                            .all(|(a, b)| (*a - *b).norm() < 1e-7)
                });
                if dup {
                    continue;
                }
                sigs.push((li, s));
                kept.push((li, fc));
            }
            fins_c = kept;
        }
        // PASS 1: boundary Gauss nodes (u, v_t, du-weight) and the net
        // u-winding. The per-point u' = dp.theta_hat / rho uses the
        // point's own equatorial radius rho, which is the cylinder
        // radius on a cylinder and R cos(lat) on a sphere.
        let mut nodes: Vec<(f64, f64, f64)> = Vec::new();
        let mut net_du = 0.0;
        // Per-loop AND per-fin winding attribution (the orientation
        // normalization below and the debug rail).
        let cur_li = core::cell::Cell::new(0usize);
        let cur_fin = core::cell::Cell::new(0usize);
        let mut loop_du: Vec<f64> = vec![0.0; face.loops.len()];
        let mut fin_du: Vec<f64> = vec![0.0; fins_c.len()];
        let mut node_li: Vec<usize> = Vec::new();
        let mut node_fin: Vec<usize> = Vec::new();
        let mut node = |p: Vec3, dp: Vec3, wt: f64| {
            let w = p - o;
            let (x, y) = (w.dot(ex), w.dot(ey));
            let nrm = (x * x + y * y).sqrt();
            if nrm < 1e-12 {
                return;
            }
            let (cu, su) = (x / nrm, y / nrm);
            let theta_hat = ey * cu - ex * su;
            let wu = wt * dp.dot(theta_hat) / nrm;
            net_du += wu;
            loop_du[cur_li.get()] += wu;
            fin_du[cur_fin.get()] += wu;
            node_li.push(cur_li.get());
            node_fin.push(cur_fin.get());
            nodes.push((su.atan2(cu), vmap(w), wu));
        };
        for (fi, (li, fc)) in fins_c.iter().enumerate() {
            cur_li.set(*li);
            cur_fin.set(fi);
            match fc {
                FinCurve::Seg(a, b2) => {
                    for (xj, wj) in GL8_X.iter().zip(GL8_W) {
                        let t = 0.5 * (xj + 1.0);
                        node(*a + (*b2 - *a) * t, *b2 - *a, wj * 0.5);
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
                            node(c.point(t), dp, wj * 0.5 * (a1 - a0));
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
                            node(el.point(t), dp, wj * 0.5 * (a1 - a0));
                        }
                    }
                }
                FinCurve::Nurbs(n, t0, t1) => {
                    // GL8 over the NURBS parameter; tangent = first derivative.
                    // Panel count tied to the control-point count so a wiggly
                    // fit is sampled densely enough for the boundary flux.
                    let panels = (n.control_points().len() * 2).max(8);
                    for ip in 0..panels {
                        let a0 = t0 + (t1 - t0) * ip as f64 / panels as f64;
                        let a1 = t0 + (t1 - t0) * (ip + 1) as f64 / panels as f64;
                        for (xj, wj) in GL8_X.iter().zip(GL8_W) {
                            let t = 0.5 * (a0 + a1) + 0.5 * (a1 - a0) * xj;
                            let d = n.derivatives(t, 1);
                            let dp = d.get(1).copied().unwrap_or(Vec3::ZERO);
                            node(d[0], dp, wj * 0.5 * (a1 - a0));
                        }
                    }
                }
            }
        }
        // ORIENTATION NORMALIZATION (task 41): the rep does not enforce
        // ring-vs-outer loop winding on curved faces (tessellation and
        // validate never read it), so a band can arrive with both loops
        // winding the SAME way (the crossing-cylinder union's upper
        // band measured +2 tau). Green needs net winding 0 on a
        // cylinder trim, and ANY sign assignment achieving it gives the
        // correct integral after the area-sign normalization below
        // (flipping every loop plus the global sign is the identity):
        // greedily flip RING loops that co-wind with the net until the
        // net vanishes. Spheres keep their pole logic untouched; a net
        // that will not normalize declines exactly as before.
        if !is_sphere {
            let mut net = (net_du / tau).round() as i64;
            let mut flip = vec![false; loop_du.len()];
            for li in 1..loop_du.len() {
                if net == 0 {
                    break;
                }
                let w = (loop_du[li] / tau).round() as i64;
                if w != 0 && w.signum() == net.signum() {
                    flip[li] = true;
                    net -= 2 * w;
                }
            }
            if flip.iter().any(|&f| f) {
                net_du = 0.0;
                for (i, n) in nodes.iter_mut().enumerate() {
                    if flip[node_li[i]] {
                        n.2 = -n.2;
                    }
                    net_du += n.2;
                }
            }
        }
        // PER-FIN fallback (the cyl/cyl barrel, dossier 64): when a SINGLE
        // loop carries two same-winding full-revolution wrap fins (the band
        // between two encircling seams), the per-loop flip above cannot fix it
        // (no ring loop to flip), so the net stays +-2 tau and a cylinder
        // declines. Flip individual co-winding full-revolution fins until the
        // net vanishes; the area-sign normalization below absorbs the global
        // sign, so any net-zeroing choice integrates correctly.
        if !is_sphere {
            let mut net = (net_du / tau).round() as i64;
            // ONLY the genuine multi-wrap band (|net| >= 2: a cylinder/cone
            // barrel bounded by two same-winding full-revolution wraps). A net
            // of +-1 is a cone-tip / sphere-cap that the apex / pole anchor in
            // PASS 2 handles; flipping a rim fin here would zero the net and
            // STEAL that anchor case, mis-integrating it (the block/cone union
            // regression). Leave +-1 to the anchors.
            if net.abs() >= 2 {
                let mut flipf = vec![false; fin_du.len()];
                for fi in 0..fin_du.len() {
                    if net == 0 {
                        break;
                    }
                    let w = (fin_du[fi] / tau).round() as i64;
                    if w != 0 && w.signum() == net.signum() {
                        flipf[fi] = true;
                        net -= 2 * w;
                    }
                }
                if flipf.iter().any(|&f| f) {
                    net_du = 0.0;
                    for (i, n) in nodes.iter_mut().enumerate() {
                        if flipf[node_fin[i]] {
                            n.2 = -n.2;
                        }
                        net_du += n.2;
                    }
                }
            }
        }
        // PASS 2: choose the slab base by the total winding, then turn
        // each boundary node into an inner v-slab of samples.
        let winding = (net_du / tau).round();
        if std::env::var("KEEL_MASS_DEBUG").is_ok() {
            eprintln!(
                "  green-pass1 {fk:?} net_du {net_du:.4} winding {winding} v [{v_min:.3}, {v_max:.3}] fins {} loop_du {:?}",
                fins_c.len(),
                loop_du
                    .iter()
                    .map(|d| (d / tau * 100.0).round() / 100.0)
                    .collect::<Vec<_>>()
            );
            for (i, (li, fc)) in fins_c.iter().enumerate() {
                let d = match fc {
                    FinCurve::Seg(..) => "seg".to_string(),
                    FinCurve::Circ(_, a, b) => format!("circ {a:.3}..{b:.3}"),
                    FinCurve::Ell(_, a, b) => format!("ell {a:.3}..{b:.3}"),
                    FinCurve::Nurbs(_, a, b) => format!("nurbs {a:.3}..{b:.3}"),
                };
                eprintln!("    fin {i} (loop {li}): {d}");
            }
        }
        let v_base = if winding.abs() < 0.5 {
            v_min
        } else if is_sphere && (winding.abs() - 1.0).abs() < 0.25 {
            // The trim encloses exactly ONE pole, and the anchor must
            // sit at the ENCLOSED one (anchoring at the other pole
            // integrates the complement: the trap's defect 2, where the
            // REST face read the cap's flux). The boundary nodes decide
            // it: area(vb) = -sum wu (v_t - vb) is the patch area for
            // the right anchor and the complement's negative for the
            // wrong one, so the candidate whose magnitude matches the
            // face's own tessellated area wins.
            let pole = core::f64::consts::FRAC_PI_2;
            // ENCLOSED-POLE anchor. The boundary nodes span a latitude BAND
            // [v_min, v_max]; the face's interior reaches past it to exactly ONE
            // frame pole, where the anchor must sit (the other pole integrates
            // the COMPLEMENT -- the trap's defect 2). PRIMARY test (frame-robust):
            // the face interior point lies on the enclosed-pole side of the band.
            // A TILTED planar cap cut (a cylinder/cone end-cap not square to the
            // sphere axis) leaves the SSI circle at mid latitudes while the big
            // cap reaches a frame pole, so the interior latitude falls OUTSIDE
            // [v_min, v_max] toward the enclosed pole. The area-vs-tessellation
            // witness below mis-picks here (it reads the COMPLEMENT pole when the
            // tessellation meshed the wrong cap side -- the F1 cyl/sphere
            // planar-cap mass = 1.25 vs true 2.85 bug).
            let ip_pole = self.face_interior_point(fk).and_then(|ip| {
                let vlat = vmap(ip - o);
                if vlat < v_min - 1e-9 {
                    Some(-pole)
                } else if vlat > v_max + 1e-9 {
                    Some(pole)
                } else {
                    None
                }
            });
            // SPHERICAL area from the boundary nodes (dA = r^2 cos v
            // dv du, so the inner integral is sin v_t - sin vb): the
            // same units as the tessellated area witness. FALLBACK only,
            // when the interior latitude sits inside the boundary band.
            let r2 = match surf {
                Surface3::Sphere(s) => s.radius * s.radius,
                _ => 1.0,
            };
            let area_at = |vb: f64| -> f64 {
                let mut s = 0.0;
                for &(_, v_t, wu) in &nodes {
                    s -= wu * (v_t.sin() - vb.sin());
                }
                (s * r2).abs()
            };
            let chosen = ip_pole.unwrap_or_else(|| {
                let tess_area: f64 = self
                    .tessellate_face(fk)
                    .iter()
                    .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
                    .sum();
                if (area_at(pole) - tess_area).abs() <= (area_at(-pole) - tess_area).abs() {
                    pole
                } else {
                    -pole
                }
            });
            if (chosen > 0.0 && v_max > pole - 1e-6) || (chosen < 0.0 && v_min < -pole + 1e-6) {
                return Err(TopoError::Precondition(
                    "green-slab: boundary touches the anchor pole",
                ));
            }
            chosen
        } else if let Some(v_apex) = cone_apex.filter(|_| (winding.abs() - 1.0).abs() < 0.25) {
            // CONE-APEX anchor (dossier 60 section 1, the dual of the sphere
            // pole anchor above): a full-revolution cone-tip face (winding
            // +-1, one rim circle) closes its boundary loop at the apex.
            // Anchor the v-slab at the apex axial height; the apex Jacobian
            // |P_u x P_v| = v cos(alpha) vanishes there (a removable
            // singularity), and the slab quadrature samples strictly between
            // v_apex and the rim (GL nodes are panel-interior), so the
            // integrand is never evaluated AT the apex. A rim sitting on the
            // apex is a degenerate (apex-cut) face: decline.
            if v_min <= v_apex + 1e-9 && v_max >= v_apex - 1e-9 {
                return Err(TopoError::Precondition(
                    "green-slab: cone boundary touches the apex",
                ));
            }
            v_apex
        } else {
            return Err(TopoError::Precondition(
                "green-slab: unsupported boundary winding",
            ));
        };
        if std::env::var("KEEL_MASS_DEBUG").is_ok() {
            eprintln!(
                "  green {fk:?} winding {winding} v_base {v_base:.4} v_min {v_min:.4} v_max {v_max:.4} nodes {} fins {}",
                nodes.len(),
                fins_c.len()
            );
        }
        // HOLE-ORIENTATION normalization (the cone/sphere window mass): an
        // inner-ring loop is a HOLE and must REDUCE the region, so its (u,v)
        // area contribution must OPPOSE the outer loop's. A non-wrapping window
        // (du ~ 0) is invisible to the task-41 winding flip above, and the
        // cone's apex-side anchor flips a window's (u,v) handedness relative to
        // the cylinder's base-side anchor -- so a window imprinted consistently
        // in 3D arrives CO-oriented with the outer loop on the cone and ADDS
        // its apex-wedge (the cone-rest read 13.72 vs the true 11.42). Flip any
        // inner ring (li>=1) whose area co-signs with the outer (li=0).
        {
            let nl = loop_du.len();
            if nl >= 2 {
                let mut larea = vec![0.0f64; nl];
                for (i, &(_, v_t, wu)) in nodes.iter().enumerate() {
                    larea[node_li[i]] -= wu * (v_t - v_base);
                }
                let outer_sign = larea[0].signum();
                for (i, n) in nodes.iter_mut().enumerate() {
                    let li = node_li[i];
                    if li >= 1 && larea[li].abs() > 1e-12 && larea[li].signum() == outer_sign {
                        n.2 = -n.2;
                    }
                }
            }
        }
        let mut area = 0.0;
        let mut acc: Vec<(f64, f64, f64)> = Vec::new();
        for &(u, v_t, wu) in &nodes {
            area -= wu * (v_t - v_base);
            // Composite GL8 down the slab: the sphere integrand is
            // trigonometric in latitude, so one panel per quarter-pi
            // keeps the inner quadrature at machine precision.
            let span = v_t - v_base;
            let panels = (span.abs() / core::f64::consts::FRAC_PI_4).ceil().max(1.0) as usize;
            for ip in 0..panels {
                let s0 = v_base + span * ip as f64 / panels as f64;
                let s1 = v_base + span * (ip + 1) as f64 / panels as f64;
                let half = 0.5 * (s1 - s0);
                for (xk, wk) in GL8_X.iter().zip(GL8_W) {
                    let s = 0.5 * (s0 + s1) + half * xk;
                    acc.push((u, s, -wu * wk * half));
                }
            }
        }
        if area.abs() < 1e-12 {
            return Err(TopoError::Precondition("green-slab: degenerate UV region"));
        }
        let flip = area.signum();
        // COMPLEMENT detection (task 36, the sphere-sphere rest face):
        // a boundary that does not wrap u and does not enclose a pole
        // is an ISLAND in UV, and the slab integrates the ISLAND'S
        // region (the lens cap), even when the face is everything BUT
        // that island. The face's own tessellated area decides: if it
        // matches sphere-total minus the enclosed area, integrate the
        // FULL sphere and subtract the island.
        let complement = if is_sphere && winding.abs() < 0.5 {
            let r2 = match surf {
                Surface3::Sphere(s) => s.radius * s.radius,
                _ => 1.0,
            };
            let mut enc = 0.0;
            for &(_, v_t, wu) in &nodes {
                enc -= wu * (v_t.sin() - v_base.sin());
            }
            let enc = (enc * r2).abs();
            let total = 2.0 * tau * r2; // 4 pi r^2
            let tess_area: f64 = self
                .tessellate_face(fk)
                .iter()
                .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
                .sum();
            ((total - enc) - tess_area).abs() < (enc - tess_area).abs()
        } else {
            false
        };
        let island_sign = if complement { -1.0 } else { 1.0 };
        for (u, s, w) in acc {
            let Ok(lg) = surf.local_geometry(u, s) else {
                continue;
            };
            let n = lg.du.cross(lg.dv) * sense_sign;
            m.add(lg.point, n, w * flip * island_sign);
        }
        if complement {
            // The full closed sphere's flux over the canonical
            // rectangle (the closed-cover quadrature), with this
            // face's orientation.
            let (u0, u1) = (0.0, tau);
            let (v0, v1) = (-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2);
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
                                continue;
                            };
                            let n = lg.du.cross(lg.dv) * sense_sign;
                            let w = wu * wv * 0.25 * (ub - ua) * (vb - va);
                            m.add(lg.point, n, w);
                        }
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
    fn cone_apex_anchor_green_matches_cone_volume() {
        // The Green-slab cone-apex anchor (dossier 60 section 1) integrates a
        // full-revolution cone-tip face to the exact cone volume. The lateral
        // carries the whole volume flux (the base normal is axial, n_x = 0, so
        // the base contributes 0 to integral x n_x dA). Call the boundary-flux
        // path directly to exercise the apex anchor (the iso path would handle
        // a clean tip otherwise).
        let (r, h) = (1.5, 2.0);
        let mut b = Body::new();
        let out = b
            .cone(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                r,
                h,
            )
            .unwrap();
        let lateral = out.faces[0];
        let (sk, sense) = b.faces.get(lateral).unwrap().surface.unwrap();
        let SurfaceGeom::Analytic(surf) = b.surfaces.get(sk).unwrap().clone() else {
            panic!("cone lateral is not an analytic surface");
        };
        let sense_sign = if sense { 1.0 } else { -1.0 };
        let mut m = Moments::default();
        b.integrate_face_green(lateral, &surf, sense_sign, &mut m)
            .unwrap();
        let exact = core::f64::consts::PI / 3.0 * r * r * h;
        assert!(
            (m.v - exact).abs() < 1e-9 * exact,
            "apex-anchored cone lateral v = {} want {exact}",
            m.v
        );
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
