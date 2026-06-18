//! Model interrogation queries (parity Phase: interrogation). Bounding
//! boxes and minimum distance between bodies, built on the same
//! outward-triangle tessellation the winding classifier uses.

use crate::Body;
use keel_math::bbox::Aabb3;
use keel_math::vec::Vec3;

/// One face's draft-angle range relative to a pull direction (parity item
/// 107). `min`/`max` are signed radians: arcsin(outward_normal . pull),
/// so +pi/2 = facing fully toward the pull, -pi/2 = fully away, 0 = a
/// vertical wall (zero draft). Planar faces have min == max; curved faces
/// give the range over the face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceDraft {
    pub face: crate::entity::FaceKey,
    pub min: f64,
    pub max: f64,
}

/// One render-ready facet (parity item 95): an outward-oriented triangle
/// and its unit outward normal (flat shading). The triangle is wound CCW
/// about `normal`, matching the tessellation the volume oracle uses.
#[derive(Clone, Copy, Debug)]
pub struct RenderFacet {
    pub tri: [Vec3; 3],
    pub normal: Vec3,
}

/// Render data for a body (parity item 95, render facets + lines): shaded
/// facets plus the wireframe polylines of its topological edges (one
/// polyline per edge, exact for straight edges, sampled for curved).
#[derive(Clone, Debug, Default)]
pub struct RenderMesh {
    pub facets: Vec<RenderFacet>,
    pub edges: Vec<Vec<Vec3>>,
}

/// Hidden-line-removed wireframe for a view (parity item 96): the body's
/// edge segments split into those the viewer can see and those the solid
/// occludes.
#[derive(Clone, Debug, Default)]
pub struct HlrWireframe {
    pub visible: Vec<[Vec3; 2]>,
    pub hidden: Vec<[Vec3; 2]>,
}

/// A section view of a body cut by a plane (parity item 99): the ordered
/// cross-section outline (item 75) plus the filled cut face triangulated
/// for rendering, and the cut-plane normal the fill faces.
#[derive(Clone, Debug, Default)]
pub struct SectionView {
    pub outline: Vec<Vec3>,
    pub facets: Vec<[Vec3; 3]>,
    pub normal: Vec3,
}

/// Moller-Trumbore ray/triangle hit: the parameter t > eps where
/// `orig + t*dir` meets `tri`, or None. `dir` need not be unit (t is in
/// `dir` lengths); the eps skips faces the origin already lies on.
fn ray_tri_hit(orig: Vec3, dir: Vec3, tri: &[Vec3; 3]) -> Option<f64> {
    let e1 = tri[1] - tri[0];
    let e2 = tri[2] - tri[0];
    let pvec = dir.cross(e2);
    let det = e1.dot(pvec);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tvec = orig - tri[0];
    let u = tvec.dot(pvec) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(e1);
    let w = dir.dot(qvec) * inv;
    if w < 0.0 || u + w > 1.0 {
        return None;
    }
    let t = e2.dot(qvec) * inv;
    (t > 1e-6).then_some(t)
}

/// Closest point on triangle `[a, b, c]` to `p` (Ericson, Real-Time
/// Collision Detection).
fn closest_on_tri(p: Vec3, tri: &[Vec3; 3]) -> Vec3 {
    let (a, b, c) = (tri[0], tri[1], tri[2]);
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a; // vertex region A
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b; // vertex region B
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return a + ab * v; // edge AB
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c; // vertex region C
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return a + ac * w; // edge AC
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b + (c - b) * w; // edge BC
    }
    // Interior: barycentric projection onto the plane.
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    a + ab * v + ac * w
}

/// Distance from `p` to triangle `[a, b, c]`.
fn point_tri_distance(p: Vec3, tri: &[Vec3; 3]) -> f64 {
    (p - closest_on_tri(p, tri)).norm()
}

impl Body {
    /// All outward triangles of the body (the tessellation the winding
    /// classifier and volume use). Interior partition walls (both sides
    /// solid, item 29) are not part of the outer boundary; including
    /// them would corrupt the divergence-theorem volume.
    fn all_triangles(&self) -> Vec<[Vec3; 3]> {
        self.face_keys()
            .iter()
            .filter(|&&f| !self.is_interior_wall(f))
            .flat_map(|&f| self.tessellate_face(f))
            .collect()
    }

    /// Render facets + wireframe lines (parity item 95). Facets are the
    /// same outward tessellation the volume oracle uses (each triangle CCW
    /// about its outward normal); the wireframe is one polyline per
    /// topological edge -- exact endpoints for straight edges, 32-segment
    /// samples for circular/elliptic arcs and NURBS edges.
    pub fn render_mesh(&self) -> RenderMesh {
        self.render_mesh_opt(None)
    }

    /// Render facets + wireframe at a chord tolerance (parity item 98,
    /// adaptive/incremental tessellation): curved analytic faces
    /// (cylinder/cone/sphere/torus) are faceted finely enough that each
    /// triangle stays within `chord_tol` of the true surface (finer tol =>
    /// more facets). The wireframe edges are unchanged (already exact or
    /// arc-sampled). NURBS faces use their default grid (a curvature-
    /// adaptive NURBS faceter is a follow-up).
    pub fn render_mesh_tol(&self, chord_tol: f64) -> RenderMesh {
        self.render_mesh_opt(Some(chord_tol))
    }

    fn render_mesh_opt(&self, tol: Option<f64>) -> RenderMesh {
        let facets = self
            .face_keys()
            .iter()
            .flat_map(|&f| match tol {
                Some(t) => self.tessellate_face_tol(f, t),
                None => self.tessellate_face(f),
            })
            .map(|tri| {
                let normal = (tri[1] - tri[0])
                    .cross(tri[2] - tri[0])
                    .try_normalize()
                    .unwrap_or(Vec3::ZERO);
                RenderFacet { tri, normal }
            })
            .collect();
        let edges = self
            .edges
            .iter()
            .filter_map(|(k, _)| self.edge_polyline(k))
            .collect();
        RenderMesh { facets, edges }
    }

    /// Sample one topological edge into a 3D wireframe polyline (>= 2
    /// points), used by `render_mesh`. Straight edges (line / degree-1
    /// NURBS) give their two endpoints; circular/elliptic arcs sample the
    /// arc between their endpoint parameters (a full revolution for a
    /// closed edge); NURBS edges sample their parameter domain.
    fn edge_polyline(&self, edge: crate::entity::EdgeKey) -> Option<Vec<Vec3>> {
        use keel_geom::curve::Curve3;
        let e = self.edges.get(edge)?;
        let (v0, v1) = e.bounds;
        let p0 = self.vertices.get(v0)?.point;
        let p1 = self.vertices.get(v1)?.point;
        let straight = vec![p0, p1];
        let Some((ck, _)) = e.curve else {
            return Some(straight);
        };
        let Some(curve) = self.curves.get(ck) else {
            return Some(straight);
        };
        const N: usize = 32;
        let tau = core::f64::consts::TAU;
        // Periodic arc span [t0, t1]: a closed edge (coincident endpoint
        // parameters) is a full revolution; otherwise the arc in the
        // increasing-parameter direction.
        let span = |t0: f64, mut t1: f64| -> (f64, f64) {
            if (t1 - t0).abs() < 1e-9 {
                t1 = t0 + tau;
            } else {
                if t1 < t0 {
                    t1 += tau;
                }
                if t1 - t0 > tau {
                    t1 -= tau;
                }
            }
            (t0, t1)
        };
        // A recorded arc_sweep IS the edge's arc identity (bounds.0-
        // relative): no disambiguation needed. Distinct endpoints whose
        // carrier projections COINCIDE mean the stored conic cannot
        // parameterize this edge at all (the corner-blend spring edges):
        // draw through the trim pcurve instead of a ghost full circle.
        let sweep = e.arc_sweep;
        let degenerate_proj = |a0: f64, a1: f64| -> bool {
            let mut d = (a1 - a0).rem_euclid(tau);
            if d > tau * 0.5 {
                d = tau - d;
            }
            v0 != v1 && d < 1e-9
        };
        if std::env::var("KEEL_WIRE_DEBUG").is_ok() {
            let kind = match curve {
                Curve3::Line(_) => "line",
                Curve3::Circle(_) => "circle",
                Curve3::Ellipse(_) => "ellipse",
                Curve3::Nurbs(_) => "nurbs",
            };
            let (a0, a1) = match curve {
                Curve3::Circle(c) => (c.project(p0), c.project(p1)),
                Curve3::Ellipse(el) => (el.project(p0), el.project(p1)),
                _ => (0.0, 0.0),
            };
            eprintln!(
                "  wire {edge:?} {kind} closed {} a0 {a0:.3} a1 {a1:.3} sweep {sweep:?} p0 {p0:?} p1 {p1:?}",
                v0 == v1
            );
        }
        let out = match curve {
            Curve3::Line(_) => straight,
            Curve3::Nurbs(n) if n.degree() <= 1 => straight,
            Curve3::Circle(c) => {
                let c = *c;
                let (a0, a1) = (c.project(p0), c.project(p1));
                if let Some(s) = sweep {
                    (0..=N)
                        .map(|i| c.point(a0 + s * i as f64 / N as f64))
                        .collect()
                } else if degenerate_proj(a0, a1) {
                    self.pcurve_polyline(edge).unwrap_or(straight)
                } else {
                    let (t0, t1) = self.true_arc_span(edge, span(a0, a1), |t| c.point(t));
                    (0..=N)
                        .map(|i| c.point(t0 + (t1 - t0) * i as f64 / N as f64))
                        .collect()
                }
            }
            Curve3::Ellipse(el) => {
                let el = *el;
                let (a0, a1) = (el.project(p0), el.project(p1));
                if let Some(s) = sweep {
                    (0..=N)
                        .map(|i| el.point(a0 + s * i as f64 / N as f64))
                        .collect()
                } else if degenerate_proj(a0, a1) {
                    self.pcurve_polyline(edge).unwrap_or(straight)
                } else {
                    let (t0, t1) = self.true_arc_span(edge, span(a0, a1), |t| el.point(t));
                    (0..=N)
                        .map(|i| el.point(t0 + (t1 - t0) * i as f64 / N as f64))
                        .collect()
                }
            }
            Curve3::Nurbs(n) => {
                let (a, b) = n.domain();
                (0..=N)
                    .map(|i| n.point(a + (b - a) * i as f64 / N as f64))
                    .collect()
            }
        };
        Some(out)
    }

    /// Sample an edge through one of its fins' trim pcurves, evaluated
    /// on that fin's face surface: the drawing authority when the 3D
    /// carrier cannot parameterize the edge (coincident endpoint
    /// projections).
    fn pcurve_polyline(&self, edge: crate::entity::EdgeKey) -> Option<Vec<Vec3>> {
        use keel_geom::curve::Curve3;
        const N: usize = 32;
        let e = self.edges.get(edge)?;
        for &fk in &e.radial {
            let Some(fin) = self.fins.get(fk) else {
                continue;
            };
            let Some((pk, _)) = fin.pcurve else {
                continue;
            };
            let Some(Curve3::Nurbs(n)) = self.curves.get(pk) else {
                continue;
            };
            let Some(face) = self.loops.get(fin.owner).map(|l| l.face) else {
                continue;
            };
            let Some((sk, _)) = self.faces.get(face).and_then(|f| f.surface) else {
                continue;
            };
            let Some(crate::entity::SurfaceGeom::Analytic(s)) = self.surfaces.get(sk) else {
                continue;
            };
            let (a, b) = n.domain();
            let mut out = Vec::with_capacity(N + 1);
            for i in 0..=N {
                let q = n.point(a + (b - a) * i as f64 / N as f64);
                let Ok(lg) = s.local_geometry(q.x, q.y) else {
                    break;
                };
                out.push(lg.point);
            }
            if out.len() == N + 1 {
                return Some(out);
            }
        }
        None
    }

    /// Silhouette / outline edges of the body for an orthographic view
    /// direction `view` (parity item 97). Returns the segments where the
    /// surface turns away from the viewer: an edge is on the silhouette
    /// when its two incident facets face opposite ways relative to `view`
    /// (one toward, one away). EXACT for polyhedral models (the segments
    /// are real model edges); tessellation-resolution for curved faces
    /// (each smooth silhouette curve becomes a polyline; exact analytic
    /// silhouette curves are a later refinement). `view` points from the
    /// scene toward the eye; an unnormalizable view returns nothing.
    pub fn silhouette(&self, view: Vec3) -> Vec<[Vec3; 2]> {
        use std::collections::BTreeMap;
        let Some(v) = view.try_normalize() else {
            return Vec::new();
        };
        // Weld vertices to a 1e-6 grid so facet edges that share a point
        // get the same key (the tessellation shares model vertices exactly).
        let q = |p: Vec3| -> (i64, i64, i64) {
            const S: f64 = 1e6;
            (
                (p.x * S).round() as i64,
                (p.y * S).round() as i64,
                (p.z * S).round() as i64,
            )
        };
        type Key = (i64, i64, i64);
        // Per-welded-edge accumulator: its two endpoints, the normal-dot-
        // view of each incident facet, and how many facets share it.
        struct Acc {
            a: Vec3,
            b: Vec3,
            signs: [f64; 2],
            count: usize,
        }
        let mut edges: BTreeMap<(Key, Key), Acc> = BTreeMap::new();
        for f in self.render_mesh().facets {
            let d = f.normal.dot(v);
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let (mut pa, mut pb) = (f.tri[i], f.tri[j]);
                let (mut ka, mut kb) = (q(pa), q(pb));
                if ka > kb {
                    std::mem::swap(&mut ka, &mut kb);
                    std::mem::swap(&mut pa, &mut pb);
                }
                let e = edges.entry((ka, kb)).or_insert(Acc {
                    a: pa,
                    b: pb,
                    signs: [0.0; 2],
                    count: 0,
                });
                if e.count < 2 {
                    e.signs[e.count] = d;
                }
                e.count += 1;
            }
        }
        edges
            .into_values()
            .filter(|e| e.count == 2 && e.signs[0] * e.signs[1] < 0.0)
            .map(|e| [e.a, e.b])
            .collect()
    }

    /// Hidden-line-removed wireframe for an orthographic view direction
    /// `view` (parity item 96). Each topological edge is sampled into
    /// segments; a segment is HIDDEN when a ray from its midpoint toward
    /// the eye (along +view) strikes a facet of the body (the solid
    /// occludes it), else VISIBLE. Midpoint classification: a segment
    /// straddling an occlusion boundary is a later refinement -- exact for
    /// segments wholly in front of or behind the solid (the common case at
    /// fine sampling). A degenerate view returns an empty wireframe.
    pub fn hidden_line_wireframe(&self, view: Vec3) -> HlrWireframe {
        let Some(v) = view.try_normalize() else {
            return HlrWireframe::default();
        };
        let facets: Vec<[Vec3; 3]> = self.render_mesh().facets.iter().map(|f| f.tri).collect();
        let mut out = HlrWireframe::default();
        for (k, _) in self.edges.iter() {
            let Some(poly) = self.edge_polyline(k) else {
                continue;
            };
            for w in poly.windows(2) {
                let m = (w[0] + w[1]) * 0.5;
                let occluded = facets.iter().any(|t| ray_tri_hit(m, v, t).is_some());
                if occluded {
                    out.hidden.push([w[0], w[1]]);
                } else {
                    out.visible.push([w[0], w[1]]);
                }
            }
        }
        out
    }

    /// Area of a single face (parity interrogation), summed over its
    /// outward triangles. Exact for planar faces; the tessellation
    /// approximation for curved faces (consistent with the curved volume
    /// oracle -- exact analytic area is a later refinement).
    pub fn face_area(&self, face: crate::entity::FaceKey) -> f64 {
        if let Some(a) = self.analytic_curved_area(face) {
            return a;
        }
        self.tessellate_face(face)
            .iter()
            .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
            .sum()
    }

    /// Disambiguate which of the two candidate arcs between an edge's
    /// endpoints is the MATERIAL one: the increasing-parameter pick is
    /// arbitrary (the carrier records no half), and the wrong side draws
    /// ghost arcs outside the body (the fillet-gif cap circles). The
    /// true arc's midpoint lies on an adjacent face's trimmed region;
    /// the complement's floats in air.
    fn true_arc_span(
        &self,
        edge: crate::entity::EdgeKey,
        (t0, t1): (f64, f64),
        eval: impl Fn(f64) -> Vec3,
    ) -> (f64, f64) {
        let tau = core::f64::consts::TAU;
        if (t1 - t0 - tau).abs() < 1e-9 {
            return (t0, t1); // full revolution: nothing to pick
        }
        let mut tris: Vec<[Vec3; 3]> = Vec::new();
        if let Some(e) = self.edges.get(edge) {
            let mut seen = Vec::new();
            for &fk in &e.radial {
                if let Some(face) = self
                    .fins
                    .get(fk)
                    .and_then(|f| self.loops.get(f.owner))
                    .map(|l| l.face)
                    && !seen.contains(&face)
                {
                    seen.push(face);
                    tris.extend(self.tessellate_face(face));
                }
            }
        }
        if tris.is_empty() {
            return (t0, t1);
        }
        let dist = |p: Vec3| -> f64 {
            tris.iter()
                .map(|t| (closest_on_tri(p, t) - p).norm())
                .fold(f64::INFINITY, f64::min)
        };
        let d_inc = dist(eval(0.5 * (t0 + t1)));
        let alt = (t1, t0 + tau);
        let d_alt = dist(eval(0.5 * (alt.0 + alt.1)));
        if d_alt + 1e-9 < d_inc { alt } else { (t0, t1) }
    }

    /// Exact analytic area of a CURVED analytic face, reusing the same
    /// angular/height trim the tessellator uses (so trimmed fillet bands
    /// and primitive faces alike are exact). `None` -> use tessellation:
    /// planar faces (already exact that way), NURBS faces, and trimmed
    /// sphere/torus patches (whose partial area needs a UV integral).
    fn analytic_curved_area(&self, face: crate::entity::FaceKey) -> Option<f64> {
        use keel_geom::surface::Surface3;
        let pi = core::f64::consts::PI;
        let (sk, _) = self.faces.get(face).and_then(|f| f.surface)?;
        let crate::entity::SurfaceGeom::Analytic(s) = self.surfaces.get(sk)? else {
            return None;
        };
        let span = |o: Vec3, ex: Vec3, ey: Vec3, ez: Vec3| {
            let (lo, hi) = self.cyl_angular_span(face, o, ex, ey, ez);
            hi - lo
        };
        // A PARTIAL ellipse arc disqualifies the rectangle formula: an
        // ellipse CENTER height is the band's mean only when the edge
        // covers the full period (the mitre band, where the tilted
        // cut's added and removed wedges cancel over a full ring). A
        // half-arc (the crossing-pair band) made the formula read a
        // phantom flat bound at the center height (task 29: the bands
        // integrated to exactly r*span*2).
        let partial_ellipse = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default()
            .into_iter()
            .any(|lk| {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    return false;
                };
                let mut cur = entry;
                while let Some(fin) = self.fins.get(cur) {
                    if let Some(e) = self.edges.get(fin.edge)
                        && e.bounds.0 != e.bounds.1
                        && matches!(
                            e.curve.and_then(|(ck, _)| self.curves.get(ck)),
                            Some(keel_geom::curve::Curve3::Ellipse(_))
                        )
                        && e.arc_sweep
                            .map(|s| s.abs() < core::f64::consts::TAU - 1e-9)
                            .unwrap_or(true)
                    {
                        return true;
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
                false
            });
        let height_range = |o: Vec3, ez: Vec3, extra: Option<f64>| {
            if partial_ellipse {
                return None;
            }
            // A TILTED partial circle arc disqualifies the formula the
            // same way (task 44: the partial fillet's runout cone is
            // stopped by a quarter arc whose plane is perpendicular to
            // the EDGE, not the cone axis; its center height is a
            // phantom band bound and the analytic area read 35 percent
            // high). Axis-perpendicular partial arcs (the fillet band
            // end arcs) keep their exact heights.
            let tilted_partial = self
                .faces
                .get(face)
                .map(|f| f.loops.clone())
                .unwrap_or_default()
                .into_iter()
                .any(|lk| {
                    let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                        return false;
                    };
                    let mut cur = entry;
                    while let Some(fin) = self.fins.get(cur) {
                        if let Some(e) = self.edges.get(fin.edge)
                            && e.bounds.0 != e.bounds.1
                            && let Some(keel_geom::curve::Curve3::Circle(ci)) =
                                e.curve.and_then(|(ck, _)| self.curves.get(ck))
                            && e.arc_sweep
                                .map(|s| s.abs() < core::f64::consts::TAU - 1e-9)
                                .unwrap_or(true)
                            && ci.x_axis.cross(ci.y_axis).dot(ez).abs() < 1.0 - 1e-9
                        {
                            return true;
                        }
                        cur = fin.next;
                        if cur == entry {
                            break;
                        }
                    }
                    false
                });
            if tilted_partial {
                return None;
            }
            let mut h = self.cyl_circle_heights(face, o, ez);
            // DISTINCT heights only: one rim seen from both fins is
            // [h, h], a zero band that integrated curved areas to 0.
            h.sort_by(f64::total_cmp);
            h.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            if let Some(e) = extra
                && h.len() < 2
            {
                h.push(e);
            }
            if h.len() < 2 {
                return None;
            }
            let lo = h.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = h.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            Some((lo, hi))
        };
        match s {
            Surface3::Plane(_) => {
                // A disc / annulus (every loop a single circle) integrates
                // exactly to pi*r^2 per loop, outer adding and inner-ring
                // holes subtracting. A polygon loop -> None (tessellation is
                // already exact for polygons).
                let loops = self.faces.get(face).map(|f| f.loops.clone())?;
                let mut area = 0.0;
                for (li, lk) in loops.iter().enumerate() {
                    let (_, _, r) = self.single_circle_disc(*lk)?;
                    let disc = pi * r * r;
                    area += if li == 0 { disc } else { -disc };
                }
                Some(area)
            }
            Surface3::Cylinder(c) => {
                let f = &c.frame;
                let (hlo, hhi) = height_range(f.origin, f.z, None)?;
                Some(c.radius * span(f.origin, f.x, f.y, f.z) * (hhi - hlo))
            }
            Surface3::Cone(c) => {
                let f = &c.frame;
                let slope = c.half_angle.tan();
                if slope == 0.0 {
                    return None;
                }
                // Add the apex height (radius -> 0) if the face reaches it.
                let (hlo, hhi) = height_range(f.origin, f.z, Some(-c.radius / slope))?;
                let r_at = |h: f64| (c.radius + h * slope).abs();
                let (r1, r2) = (r_at(hlo), r_at(hhi));
                let slant = ((r2 - r1).powi(2) + (hhi - hlo).powi(2)).sqrt();
                Some(0.5 * span(f.origin, f.x, f.y, f.z) * (r1 + r2) * slant)
            }
            Surface3::Sphere(sp) if self.face_covers_closed_surface(face) => {
                Some(4.0 * pi * sp.radius * sp.radius)
            }
            Surface3::Torus(t) if self.face_covers_closed_surface(face) => {
                Some(4.0 * pi * pi * t.major * t.minor)
            }
            Surface3::Sphere(_) | Surface3::Torus(_) => None,
        }
    }

    /// Total surface area of the body (parity interrogation): the sum of
    /// every face's area. Exact for all-planar bodies.
    pub fn surface_area(&self) -> f64 {
        self.face_keys().iter().map(|&f| self.face_area(f)).sum()
    }

    /// Closest point on the body's surface to an external point `p`, with
    /// its distance (parity interrogation). Exact for planar faces;
    /// tessellation-resolution approximate for curved faces (exact
    /// face-surface projection is a later refinement). `None` for an empty
    /// body.
    pub fn closest_point(&self, p: Vec3) -> Option<(Vec3, f64)> {
        self.all_triangles()
            .iter()
            .map(|t| {
                let q = closest_on_tri(p, t);
                (q, (p - q).norm())
            })
            .min_by(|x, y| x.1.total_cmp(&y.1))
    }

    /// Radius of the largest inscribed sphere tangent at surface point
    /// `p` with outward normal `outward`: the DISTANCE TO THE MEDIAL
    /// AXIS at p (corpus-audit medial MVP; dossiers 10 / 41 / 50). The
    /// sphere centred at p - n r is empty iff the closest surface
    /// distance from its centre stays r (the tangent contact itself);
    /// emptiness is monotone in r, so bisection converges. Resolution
    /// follows `closest_point` (exact planar, tessellation-resolution
    /// curved). This is the shared feasibility field: shell t_max, the
    /// blend overflow ceiling, and defeature safety all query it.
    pub fn inscribed_radius(&self, p: Vec3, outward: Vec3) -> Option<f64> {
        let n = outward.try_normalize()?;
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for (_, v) in self.vertices.iter() {
            for c in [v.point.x, v.point.y, v.point.z] {
                lo = lo.min(c);
                hi = hi.max(c);
            }
        }
        if !lo.is_finite() {
            return None;
        }
        let r_max = ((hi - lo) * 3.0_f64.sqrt()).max(1e-9);
        let empty = |r: f64| -> bool {
            let c = p - n * r;
            match self.closest_point(c) {
                Some((_, d)) => d >= r * (1.0 - 1e-9) - 1e-9,
                None => false,
            }
        };
        if !empty(r_max * 1e-7) {
            return Some(0.0);
        }
        if empty(r_max) {
            return Some(r_max);
        }
        let (mut a, mut b) = (r_max * 1e-7, r_max);
        for _ in 0..60 {
            let mid = 0.5 * (a + b);
            if empty(mid) {
                a = mid;
            } else {
                b = mid;
            }
        }
        Some(0.5 * (a + b))
    }

    /// Minimum wall thickness over the body, 2x the smallest inscribed
    /// radius at each face's interior point (the rolling-ball thickness
    /// CAE wall checks report; edge-adjacent thinning is the medial
    /// field's nature and is probed by `inscribed_radius` directly).
    pub fn min_wall_thickness(&self) -> Option<f64> {
        let mut best: Option<f64> = None;
        for f in self.face_keys() {
            let Some(p) = self.face_interior_point(f) else {
                continue;
            };
            let Some(n) = self.face_outward_normal(f) else {
                continue;
            };
            if let Some(r) = self.inscribed_radius(p, n) {
                best = Some(match best {
                    Some(b) => b.min(2.0 * r),
                    None => 2.0 * r,
                });
            }
        }
        best
    }

    /// Principal curvatures (k1, k2) of a face's surface at the surface
    /// point nearest `p` (parity item 107, surface analysis). In 1/length
    /// units, exact for analytic surfaces: plane -> (0, 0); cylinder radius
    /// r -> {0, 1/r}; sphere radius r -> (1/r, 1/r); cone/torus vary with
    /// position. `None` if the face has no surface or the projection is
    /// degenerate.
    pub fn face_curvature(&self, face: crate::entity::FaceKey, p: Vec3) -> Option<(f64, f64)> {
        let (sk, _) = self.faces.get(face).and_then(|f| f.surface)?;
        match self.surfaces.get(sk)? {
            crate::entity::SurfaceGeom::Analytic(s) => {
                let pr = s.project(p).ok()?;
                let lg = s.local_geometry(pr.u, pr.v).ok()?;
                Some((lg.k1, lg.k2))
            }
            crate::entity::SurfaceGeom::Nurbs(n) => {
                let pr = keel_geom::project::project_point_surface_fast(n, p);
                let lg = n.local_geometry(pr.u, pr.v).ok()?;
                Some((lg.k1, lg.k2))
            }
        }
    }

    /// Draft analysis (parity item 107): per-face signed draft angle range
    /// relative to a `pull` direction, for moldability / pull-direction
    /// checks. Each face's draft is arcsin(outward_normal . pull_hat),
    /// taken over its outward tessellation triangles (constant for a
    /// planar face, a range for a curved one). A near-zero draft is a
    /// vertical wall (undercut risk); a NEGATIVE-to-POSITIVE range means
    /// the face is undercut for that pull. Empty `pull` or faces that
    /// tessellate empty are skipped.
    pub fn draft_analysis(&self, pull: Vec3) -> Vec<FaceDraft> {
        let Some(d) = pull.try_normalize() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for f in self.face_keys() {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for t in self.tessellate_face(f) {
                if let Some(n) = (t[1] - t[0]).cross(t[2] - t[0]).try_normalize() {
                    let s = n.dot(d).clamp(-1.0, 1.0).asin();
                    lo = lo.min(s);
                    hi = hi.max(s);
                }
            }
            if hi >= lo {
                out.push(FaceDraft {
                    face: f,
                    min: lo,
                    max: hi,
                });
            }
        }
        out
    }

    /// Volume from the body's outward tessellation via the divergence
    /// theorem, (1/6) sum a.(b x c) over outward triangles. Exact for
    /// all-planar bodies; tessellation-approximate for curved faces. A
    /// pcurve-free companion to the analytic mass_properties().volume.
    pub fn mesh_volume(&self) -> f64 {
        let _prof = crate::profile::Scope::new(&crate::profile::MESHVOL_NS);
        crate::profile::count(&crate::profile::MESHVOL_CALLS);
        // Signed-tetra divergence volume, summed PER CONNECTED COMPONENT and
        // recentered about each component's OWN centroid. The closed-mesh
        // sum is translation-invariant in exact arithmetic, but in f64 the
        // raw form cancels catastrophically far from the reference (terms
        // ~ coord^3 summing to a tiny volume). A SINGLE global reference
        // cannot keep coordinates small across a body whose components are
        // far apart: a disjoint union spanning several units leaves the far
        // component's terms large, and a flat cone ~6 units from the
        // reference read ~7% low, false-flagging the soak's mass==mesh band.
        // Each connected component is independently closed (encloses its own
        // signed volume; an inner void contributes negatively), so summing
        // them with per-component local references is exact and keeps every
        // coordinate small.
        let comps = self.connected_components();
        let mut face_comp: std::collections::BTreeMap<crate::entity::FaceKey, usize> =
            std::collections::BTreeMap::new();
        for (ci, comp) in comps.iter().enumerate() {
            for &sk in comp {
                if let Some(shell) = self.shells.get(sk) {
                    for &(fk, _) in &shell.faces {
                        face_comp.entry(fk).or_insert(ci);
                    }
                }
            }
        }
        let mut buckets: Vec<Vec<[Vec3; 3]>> = vec![Vec::new(); comps.len() + 1];
        for f in self.face_keys() {
            if self.is_interior_wall(f) {
                continue;
            }
            let ci = face_comp.get(&f).copied().unwrap_or(comps.len());
            buckets[ci].extend(self.tessellate_face(f));
        }
        let mut total = 0.0;
        for tris in &buckets {
            if tris.is_empty() {
                continue;
            }
            let r = tris.iter().flatten().fold(Vec3::ZERO, |a, &p| a + p) / (3 * tris.len()) as f64;
            total += tris
                .iter()
                .map(|t| (t[0] - r).dot((t[1] - r).cross(t[2] - r)))
                .sum::<f64>()
                / 6.0;
        }
        total
    }

    /// Watertightness residual: ||sum of triangle area-vectors|| / (sum of
    /// triangle areas). A CLOSED oriented mesh has zero net area-vector (the
    /// surface integral of a constant over a closed boundary), so this is ~0;
    /// an OPEN mesh (cracks, missing/dropped faces, mis-stitched seams) leaves
    /// a residual proportional to the open-boundary span. Built from EDGE
    /// vectors only, so it is translation-invariant -- no far-from-origin
    /// cancellation (unlike the signed volume). Used by the boolean gate to
    /// decline a non-watertight result that the mass==mesh self-consistency
    /// check cannot catch (mass and a non-watertight mesh can agree on a WRONG
    /// value -- the #48 silent class, e.g. a large offset sphere/sphere lens
    /// reading ~18-33% over the exact volume).
    pub(crate) fn mesh_open_ratio(&self) -> f64 {
        let tris = self.all_triangles();
        let mut net = Vec3::ZERO;
        let mut area2 = 0.0;
        for t in &tris {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            net = net + n;
            area2 += n.norm();
        }
        if area2 <= 1e-30 {
            return 0.0;
        }
        net.norm() / area2
    }

    /// Axis-aligned bounding box of the body (parity item 105). Tight
    /// from the tessellation: exact for planar faces, tessellation-tight
    /// for curved (a fast refinement to exact analytic extrema is a
    /// later improvement). Empty bodies return an inverted/empty box.
    pub fn bounding_box(&self) -> Aabb3 {
        let pts: Vec<Vec3> = self.all_triangles().into_iter().flatten().collect();
        Aabb3::from_points(pts)
    }

    /// Minimum distance between the surfaces of two bodies (parity item
    /// 101). Computed symmetrically as the min over each body's
    /// tessellation vertices of the point-to-triangle distance to the
    /// other body; 0 (within tessellation resolution) when they touch or
    /// interpenetrate. This is a tessellation-resolution approximation of
    /// the exact surface min-distance (exact min-distance via face-pair
    /// surface projection is a later refinement).
    pub fn min_distance(&self, other: &Body) -> f64 {
        let a = self.all_triangles();
        let b = other.all_triangles();
        if a.is_empty() || b.is_empty() {
            return f64::INFINITY;
        }
        let mut best = f64::INFINITY;
        for tri in &a {
            for &v in tri {
                for tb in &b {
                    best = best.min(point_tri_distance(v, tb));
                }
            }
        }
        for tri in &b {
            for &v in tri {
                for ta in &a {
                    best = best.min(point_tri_distance(v, ta));
                }
            }
        }
        best
    }

    /// Do two bodies clash / interfere (parity item 102)? True if their
    /// surfaces cross or touch (the SSI yields seam curves, or a
    /// coincident/tangent contact), or one is nested in the other (a
    /// surface point of one lies inside the other by the generalized
    /// winding number). A bounding-box miss is the cheap reject. Fast:
    /// the analytic SSI plus two winding-number probes, not an O(n*m)
    /// tessellation sweep.
    pub fn clashes(&self, other: &Body) -> bool {
        use crate::boolean::BoolFault;
        if !self.bounding_box().intersects(other.bounding_box()) {
            return false;
        }
        // Surfaces cross or are in coincident/tangent contact.
        let (seams, faults) = crate::boolean::seam_curves(self, other, 1e-7);
        if !seams.is_empty()
            || faults
                .iter()
                .any(|f| matches!(f, BoolFault::Coincident(..) | BoolFault::Tangent(..)))
        {
            return true;
        }
        // No surface contact: test nesting with one representative point
        // from each body against the other's interior.
        if let Some(p) = self.all_triangles().first().map(|t| t[0])
            && other.generalized_winding_number(p) > 0.5
        {
            return true;
        }
        if let Some(p) = other.all_triangles().first().map(|t| t[0])
            && self.generalized_winding_number(p) > 0.5
        {
            return true;
        }
        false
    }

    /// Non-destructive section of the body by a plane (parity item 75):
    /// the ordered polygon where the plane cuts the body's straight
    /// edges. For a convex polyhedron this is the cross-section outline.
    /// (Curved-edge crossings and multi-loop sections are a later slice.)
    pub fn section_by_plane(&self, plane_point: Vec3, plane_normal: Vec3) -> Vec<Vec3> {
        let Some(n) = plane_normal.try_normalize() else {
            return Vec::new();
        };
        let d = n.dot(plane_point);
        let mut pts: Vec<Vec3> = Vec::new();
        for (_, e) in self.edges.iter() {
            let (v0, v1) = e.bounds;
            let (Some(a), Some(b)) = (self.vertices.get(v0), self.vertices.get(v1)) else {
                continue;
            };
            let (s0, s1) = (n.dot(a.point) - d, n.dot(b.point) - d);
            // Edge straddles the plane: linear crossing.
            if (s0 > 0.0) != (s1 > 0.0) && (s0 - s1).abs() > 1e-12 {
                let t = s0 / (s0 - s1);
                let p = a.point + (b.point - a.point) * t;
                if !pts.iter().any(|q| (*q - p).norm() < 1e-7) {
                    pts.push(p);
                }
            }
        }
        if pts.len() < 3 {
            return pts;
        }
        // Order around the centroid in the cutting plane.
        let c = pts.iter().fold(Vec3::ZERO, |s, &p| s + p) * (1.0 / pts.len() as f64);
        let seed = if n.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let u = (seed - n * seed.dot(n)).try_normalize().unwrap_or(seed);
        let w = n.cross(u);
        pts.sort_by(|p, q| {
            let ap = (*p - c).dot(w).atan2((*p - c).dot(u));
            let aq = (*q - c).dot(w).atan2((*q - c).dot(u));
            ap.partial_cmp(&aq).unwrap_or(std::cmp::Ordering::Equal)
        });
        pts
    }

    /// Section VIEW of the body cut by a plane (parity item 99): the
    /// item-75 cross-section outline plus the filled cut face, triangulated
    /// for rendering (a section/detail view's solid hatch region), oriented
    /// by the plane normal. Convex cross-sections fill exactly via a fan;
    /// concave / multi-loop / curved-boundary sections need the full 2D
    /// arrangement region engine (research file 06 interrogation/HLR; file
    /// 01 synthesis 2D-arrangement + winding for section hatching) and are
    /// a later slice -- consistent with section_by_plane's convex scope.
    pub fn section_view(&self, plane_point: Vec3, plane_normal: Vec3) -> SectionView {
        let outline = self.section_by_plane(plane_point, plane_normal);
        let normal = plane_normal.try_normalize().unwrap_or(Vec3::ZERO);
        let mut facets = Vec::new();
        // Convex cross-section -> fan triangulation from the first vertex.
        for i in 1..outline.len().saturating_sub(1) {
            facets.push([outline[0], outline[i], outline[i + 1]]);
        }
        SectionView {
            outline,
            facets,
            normal,
        }
    }

    /// Planar slices at a list of offsets along `normal` from `base`
    /// (parity item 77, additive-manufacturing slicing): one section
    /// polygon per offset. Empty slices (offset misses the body) are kept
    /// as empty vectors so the result aligns with `offsets`.
    pub fn planar_slices(&self, base: Vec3, normal: Vec3, offsets: &[f64]) -> Vec<Vec<Vec3>> {
        let n = normal.try_normalize().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        offsets
            .iter()
            .map(|&o| self.section_by_plane(base + n * o, n))
            .collect()
    }

    /// Coarse geometric/topological equivalence (parity item 108): equal
    /// entity counts, genus, and (within `tol`) bounding box and volume.
    /// This is the cheap CAx-IF validation-property comparison stage
    /// (research file 22), position-sensitive; an exact B-rep equality
    /// oracle is a later refinement.
    pub fn approx_equals(&self, other: &Body, tol: f64) -> bool {
        let (ca, cb) = (self.counts(), other.counts());
        if ca.v != cb.v
            || ca.e != cb.e
            || ca.f != cb.f
            || ca.regions != cb.regions
            || ca.genus != cb.genus
        {
            return false;
        }
        let (ba, bb) = (self.bounding_box(), other.bounding_box());
        if (ba.min - bb.min).norm() > tol || (ba.max - bb.max).norm() > tol {
            return false;
        }
        let (va, vb) = (self.tessellated_volume(), other.tessellated_volume());
        (va - vb).abs() <= tol * (1.0 + va.abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;

    #[test]
    fn inscribed_radius_is_the_medial_distance() {
        // 4 x 4 x 1 slab: at the top centre the inscribed sphere fills
        // the slab (r = 0.5); near a rim the medial distance shrinks to
        // the rim distance (the field is honest about edges); at a side
        // centre the vertical clearance governs (0.5).
        let mut slab = Body::new();
        slab.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let r = slab
            .inscribed_radius(Vec3::new(2.0, 2.0, 1.0), Vec3::new(0., 0., 1.))
            .unwrap();
        assert!((r - 0.5).abs() < 1e-6, "slab medial {r}");
        let r_rim = slab
            .inscribed_radius(Vec3::new(0.2, 2.0, 1.0), Vec3::new(0., 0., 1.))
            .unwrap();
        assert!((r_rim - 0.2).abs() < 1e-6, "rim medial {r_rim}");
        let r_side = slab
            .inscribed_radius(Vec3::new(4.0, 2.0, 0.5), Vec3::new(1., 0., 0.))
            .unwrap();
        assert!((r_side - 0.5).abs() < 1e-6, "side medial {r_side}");
        // Hollowed 4^3 box with 1-thick walls: the cavity bounds the
        // sphere from inside (outer-face centre r = 0.5), and the
        // cavity wall sees the outer face (also 0.5).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 4.0, 4.0).unwrap();
        let h = b.hollow(1.0).unwrap();
        let r_out = h
            .inscribed_radius(Vec3::new(2.0, 2.0, 4.0), Vec3::new(0., 0., 1.))
            .unwrap();
        assert!((r_out - 0.5).abs() < 1e-6, "hollow outer medial {r_out}");
        let r_cav = h
            .inscribed_radius(Vec3::new(2.0, 2.0, 3.0), Vec3::new(0., 0., -1.))
            .unwrap();
        assert!((r_cav - 0.5).abs() < 1e-6, "cavity medial {r_cav}");
        // The face-sampled aggregate: bounded by the true wall and
        // positive (its sample point placement is the face interior
        // point, documented as sample-dependent).
        let t = h.min_wall_thickness().unwrap();
        assert!(t > 0.0 && t <= 1.0 + 1e-6, "hollow wall aggregate {t}");
    }

    fn z_sphere(center: Vec3, r: f64) -> Body {
        let mut b = Body::new();
        let frame = Frame3 {
            origin: center,
            x: Vec3::new(0., 1., 0.),
            y: Vec3::new(0., 0., 1.),
            z: Vec3::new(1., 0., 0.),
        };
        b.sphere(frame, r).unwrap();
        b
    }

    #[test]
    fn bounding_box_of_block_is_exact() {
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 2.0, 3.0), 4.0, 5.0, 6.0).unwrap();
        let bb = b.bounding_box();
        assert!(
            (bb.min - Vec3::new(1.0, 2.0, 3.0)).norm() < 1e-9,
            "min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(5.0, 7.0, 9.0)).norm() < 1e-9,
            "max {:?}",
            bb.max
        );
    }

    #[test]
    fn non_star_convex_prism_volume_needs_earclip() {
        // A U-shaped prism: its cap centroid (~1.5, 1.3) lands in the
        // NOTCH (outside the material), so the old centroid-fan mis-areas
        // the cap. Ear-clipping triangulates it correctly. U-area = 3x3
        // outer minus the [1,2]x[1,3] notch (=2) = 7; height 1 -> vol 7.
        let u = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(3.0, 3.0, 0.0),
            Vec3::new(2.0, 3.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(1.0, 3.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ];
        let mut b = Body::new();
        b.prism(&u, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert!(b.validate().is_ok(), "U-prism invalid");
        let v = b.mesh_volume();
        assert!(
            (v - 7.0).abs() < 1e-9,
            "U-prism mesh_volume {v} != 7 (ear-clip)"
        );
        // Surface area is also exact: 2 caps (7 each) + the U perimeter
        // (3+3+1+2+1+1+2+3 = 16) * height 1.
        assert!(
            (b.surface_area() - (14.0 + 16.0)).abs() < 1e-9,
            "U-prism area {}",
            b.surface_area()
        );
    }

    #[test]
    fn cone_is_first_class_in_tessellation() {
        // Before tessellate_cone, the lateral contributed no triangles ->
        // bbox/area were wrong. Cone radius 1, height 1, apex at z=1.
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.cone(frame, 1.0, 1.0).unwrap();
        let bb = b.bounding_box();
        assert!(
            (bb.min - Vec3::new(-1.0, -1.0, 0.0)).norm() < 1e-2,
            "cone bbox min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(1.0, 1.0, 1.0)).norm() < 1e-2,
            "cone bbox max {:?}",
            bb.max
        );
        // Area = base disc (pi) + lateral (pi * r * slant, slant = sqrt(2)).
        let area = b.surface_area();
        let expect = core::f64::consts::PI * (1.0 + core::f64::consts::SQRT_2);
        assert!(
            (area - expect).abs() < 0.05,
            "cone area {area} != ~{expect}"
        );
    }

    #[test]
    fn torus_is_first_class_in_tessellation() {
        // Before tessellate_torus the lateral gave no triangles. Torus
        // major 3, minor 1: bbox xy +/-4, z +/-1; volume 2 pi^2 R r^2.
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.torus(frame, 3.0, 1.0).unwrap();
        let bb = b.bounding_box();
        assert!(
            (bb.min - Vec3::new(-4.0, -4.0, -1.0)).norm() < 1e-2,
            "torus bbox min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(4.0, 4.0, 1.0)).norm() < 1e-2,
            "torus bbox max {:?}",
            bb.max
        );
        let v = b.mesh_volume();
        let expect = 2.0 * core::f64::consts::PI.powi(2) * 3.0 * 1.0;
        assert!(
            (v - expect).abs() < expect * 0.01,
            "torus mesh_volume {v} != ~{expect}"
        );
    }

    #[test]
    fn surface_area_of_block_is_exact() {
        // 2x3x4 block: area = 2(2*3 + 3*4 + 2*4) = 2(6+12+8) = 52.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        let a = b.surface_area();
        assert!((a - 52.0).abs() < 1e-9, "block surface area {a} != 52");
    }

    #[test]
    fn exact_curved_surface_area() {
        use crate::entity::SurfaceGeom;
        use keel_geom::surface::Surface3;
        let pi = core::f64::consts::PI;
        let cf = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        // Cylinder r=2 h=3: lateral = 2 pi r h = 12pi, now EXACT (analytic,
        // not the tessellation undershoot).
        let mut cyl = Body::new();
        cyl.cylinder(cf.clone(), 2.0, 3.0).unwrap();
        let lat = cyl
            .face_keys()
            .into_iter()
            .find(|&f| {
                matches!(
                    cyl.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                )
            })
            .unwrap();
        assert!(
            (cyl.face_area(lat) - 12.0 * pi).abs() < 1e-9,
            "cylinder lateral area {} != 12pi",
            cyl.face_area(lat)
        );
        // Whole-body surface area is now exact too: caps 2*(pi*4) + lateral
        // 12pi = 20pi (the disc caps are exact pi*r^2, not a 32-gon).
        assert!(
            (cyl.surface_area() - 20.0 * pi).abs() < 1e-9,
            "cylinder surface area {} != 20pi",
            cyl.surface_area()
        );
        // Cone r=2 h=3: lateral = pi r slant, slant = sqrt(4+9) = sqrt(13).
        let mut cone = Body::new();
        cone.cone(cf, 2.0, 3.0).unwrap();
        let cl = cone
            .face_keys()
            .into_iter()
            .find(|&f| {
                matches!(
                    cone.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Cone(_)))
                )
            })
            .unwrap();
        let expect = pi * 2.0 * 13.0_f64.sqrt();
        assert!(
            (cone.face_area(cl) - expect).abs() < 1e-9,
            "cone lateral area {} != pi*r*sqrt(13)",
            cone.face_area(cl)
        );
    }

    #[test]
    fn face_curvature_cylinder_and_sphere() {
        use crate::entity::SurfaceGeom;
        use keel_geom::surface::Surface3;
        // Cylinder radius 2: principal curvatures {0, 1/2} on the lateral.
        let cf = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut cyl = Body::new();
        cyl.cylinder(cf, 2.0, 3.0).unwrap();
        let lat = cyl
            .face_keys()
            .into_iter()
            .find(|&f| {
                matches!(
                    cyl.face_surface_geom(f),
                    Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
                )
            })
            .expect("cylinder lateral face");
        let (k1, k2) = cyl.face_curvature(lat, Vec3::new(2.0, 0.0, 1.5)).unwrap();
        let (kmax, kmin) = (k1.abs().max(k2.abs()), k1.abs().min(k2.abs()));
        assert!((kmax - 0.5).abs() < 1e-6, "cylinder kmax {kmax} != 1/2");
        assert!(kmin < 1e-6, "cylinder kmin {kmin} != 0");

        // Sphere radius 2: both principal curvatures 1/2. (z_sphere's pole
        // axis is x, so sample the equator at (0,2,0), not the pole.)
        let s = z_sphere(Vec3::ZERO, 2.0);
        let sf = s.face_keys()[0];
        let (s1, s2) = s.face_curvature(sf, Vec3::new(0.0, 2.0, 0.0)).unwrap();
        assert!(
            (s1.abs() - 0.5).abs() < 1e-6 && (s2.abs() - 0.5).abs() < 1e-6,
            "sphere curvatures ({s1}, {s2}) != (1/2, 1/2)"
        );
    }

    #[test]
    fn closest_point_on_box() {
        // [0,2]^3; point at (5,1,1) -> closest surface point (2,1,1) on the
        // +x face, distance 3.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let (q, d) = b.closest_point(Vec3::new(5.0, 1.0, 1.0)).unwrap();
        assert!((d - 3.0).abs() < 1e-9, "closest distance {d} != 3");
        assert!(
            (q - Vec3::new(2.0, 1.0, 1.0)).norm() < 1e-9,
            "closest point {q:?}"
        );
    }

    #[test]
    fn draft_analysis_box() {
        use core::f64::consts::FRAC_PI_2;
        // Pull +z: top/bottom caps are +/- pi/2 (fully drafted), the four
        // side walls are 0 (vertical / zero draft). Each is planar so the
        // range collapses (min == max).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let drafts = b.draft_analysis(Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(drafts.len(), 6, "a box has 6 faces");
        let (mut caps, mut walls) = (0, 0);
        for d in &drafts {
            assert!(
                (d.min - d.max).abs() < 1e-9,
                "planar face draft is constant"
            );
            if (d.min.abs() - FRAC_PI_2).abs() < 1e-9 {
                caps += 1;
            } else if d.min.abs() < 1e-9 {
                walls += 1;
            }
        }
        assert_eq!((caps, walls), (2, 4), "box draft buckets vs +z pull");
    }

    #[test]
    fn draft_analysis_cylinder_lateral_is_zero() {
        use core::f64::consts::FRAC_PI_2;
        // Pull +z (the axis): caps +/- pi/2, the lateral face is 0 draft
        // ALL the way around (radial normals are perpendicular to z) -- an
        // undraftable vertical wall.
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.cylinder(frame, 1.0, 2.0).unwrap();
        let drafts = b.draft_analysis(Vec3::new(0.0, 0.0, 1.0));
        assert!(
            drafts
                .iter()
                .any(|d| (d.min - FRAC_PI_2).abs() < 1e-6 && (d.max - FRAC_PI_2).abs() < 1e-6),
            "top cap fully drafted"
        );
        assert!(
            drafts.iter().any(|d| (d.min + FRAC_PI_2).abs() < 1e-6),
            "bottom cap fully drafted"
        );
        assert!(
            drafts
                .iter()
                .any(|d| d.min.abs() < 1e-6 && d.max.abs() < 1e-6),
            "lateral wall is zero draft all around"
        );
    }

    #[test]
    fn bounding_box_of_sphere_is_tight() {
        let b = z_sphere(Vec3::new(0.5, -1.0, 2.0), 2.0);
        let bb = b.bounding_box();
        // Tessellation-tight: within a small fraction of the radius.
        assert!(
            (bb.min - Vec3::new(-1.5, -3.0, 0.0)).norm() < 0.05,
            "min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(2.5, 1.0, 4.0)).norm() < 0.05,
            "max {:?}",
            bb.max
        );
    }

    #[test]
    fn min_distance_between_separated_spheres() {
        // Centres 5 apart, radii 1 and 1.5 -> surface gap ~2.5.
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(5.0, 0.0, 0.0), 1.5);
        let d = a.min_distance(&b);
        assert!((d - 2.5).abs() < 0.1, "min distance {d} vs ~2.5");
    }

    #[test]
    fn min_distance_zero_when_overlapping() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(1.0, 0.0, 0.0), 1.0);
        assert!(a.min_distance(&b) < 0.1, "overlapping spheres should be ~0");
    }

    #[test]
    fn clash_detection() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        // Overlapping -> clash.
        assert!(a.clashes(&z_sphere(Vec3::new(1.0, 0.0, 0.0), 1.0)));
        // Separated -> no clash.
        assert!(!a.clashes(&z_sphere(Vec3::new(5.0, 0.0, 0.0), 1.0)));
        // Fully nested (small inside big, no surface contact) -> clash.
        let big = z_sphere(Vec3::ZERO, 2.0);
        assert!(big.clashes(&z_sphere(Vec3::ZERO, 0.5)));
    }

    #[test]
    fn section_of_block_is_a_square() {
        // Section a 2x2x2 block at z=1: a 2x2 square (4 points, area 4).
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let poly = b.section_by_plane(Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(
            poly.len(),
            4,
            "square section has 4 corners, got {}",
            poly.len()
        );
        // Shoelace area in the z=1 plane.
        let mut area = 0.0;
        for i in 0..poly.len() {
            let p = poly[i];
            let q = poly[(i + 1) % poly.len()];
            area += p.x * q.y - q.x * p.y;
        }
        assert!(
            (area.abs() * 0.5 - 4.0).abs() < 1e-9,
            "section area {} != 4",
            area.abs() * 0.5
        );
    }

    #[test]
    fn planar_slices_of_block() {
        // Slice a 2x2x2 block at z = 0.5, 1.0, 1.5: each a 2x2 square.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let slices = b.planar_slices(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), &[0.5, 1.0, 1.5]);
        assert_eq!(slices.len(), 3);
        for s in &slices {
            assert_eq!(s.len(), 4, "each interior slice is a square");
        }
        // A slice above the block is empty.
        let empty = b.planar_slices(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), &[5.0]);
        assert!(empty[0].is_empty(), "slice above the block is empty");
    }

    #[test]
    fn body_equivalence() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        assert!(a.approx_equals(&a.clone(), 1e-6), "body equals its clone");
        // Different radius -> not equivalent (volume + box differ).
        assert!(!a.approx_equals(&z_sphere(Vec3::ZERO, 2.0), 1e-6));
        // Two identically-built blocks are equivalent.
        let mut p = Body::new();
        p.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        let mut q = Body::new();
        q.block(Vec3::ZERO, 2.0, 3.0, 4.0).unwrap();
        assert!(p.approx_equals(&q, 1e-6), "identical blocks equivalent");
    }

    #[test]
    fn render_mesh_block_and_cylinder() {
        // Block: 6 quads x 2 triangles = 12 facets, 12 straight edges (2
        // points each); every facet normal is unit length.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let rm = b.render_mesh();
        assert_eq!(rm.facets.len(), 12, "block facets");
        assert_eq!(rm.edges.len(), 12, "block edges");
        for f in &rm.facets {
            assert!((f.normal.norm() - 1.0).abs() < 1e-9, "facet normal unit");
        }
        for e in &rm.edges {
            assert_eq!(e.len(), 2, "block edge is a straight segment");
        }
        // Cylinder: the circular rim edges sample into many-point polylines
        // whose points lie on radius 1 about the axis; facets are present.
        let mut c = Body::new();
        c.cylinder(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
            1.0,
            2.0,
        )
        .unwrap();
        let rmc = c.render_mesh();
        assert!(rmc.facets.len() > 12, "cylinder facets present");
        assert!(
            rmc.edges.iter().any(|e| e.len() > 2),
            "cylinder has a sampled curved rim"
        );
        for e in rmc.edges.iter().filter(|e| e.len() > 2) {
            for &p in e {
                let r = (p.x * p.x + p.y * p.y).sqrt();
                assert!((r - 1.0).abs() < 1e-6, "rim point off radius: r={r}");
            }
        }
        for f in &rmc.facets {
            assert!(
                (f.normal.norm() - 1.0).abs() < 1e-9,
                "cyl facet normal unit"
            );
        }
    }

    #[test]
    fn silhouette_of_cube_is_a_hexagon() {
        // A cube viewed from a generic (corner-on) direction has 3 front
        // faces and 3 back faces; the outline is the 6 edges separating
        // them -- a hexagonal silhouette. Each segment is a real cube edge
        // (length 2). No face is edge-on for (1,2,3), so the count is exact.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let sil = b.silhouette(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(sil.len(), 6, "cube silhouette is a hexagon");
        for s in &sil {
            assert!(
                ((s[1] - s[0]).norm() - 2.0).abs() < 1e-9,
                "silhouette segment is a unit cube edge"
            );
        }
        // Translation-invariant count.
        let mut t = Body::new();
        t.block(Vec3::new(10.0, -5.0, 3.0), 2.0, 2.0, 2.0).unwrap();
        assert_eq!(t.silhouette(Vec3::new(1.0, 2.0, 3.0)).len(), 6);
        // A degenerate (zero) view yields nothing.
        assert!(b.silhouette(Vec3::ZERO).is_empty());
    }

    #[test]
    fn hlr_cube_hides_far_corner_edges() {
        // Cube [0,2]^3 viewed from (1,2,3): the far corner (0,0,0)'s three
        // edges are occluded by the solid; the other 9 are visible.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let hlr = b.hidden_line_wireframe(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            hlr.visible.len() + hlr.hidden.len(),
            12,
            "all cube edges classified"
        );
        assert_eq!(hlr.hidden.len(), 3, "far-corner edges hidden");
        assert_eq!(hlr.visible.len(), 9, "front edges visible");
        // Degenerate view -> empty.
        assert!(b.hidden_line_wireframe(Vec3::ZERO).visible.is_empty());
    }

    #[test]
    fn section_view_of_cube_is_a_filled_square() {
        // Cube [0,2]^3 cut by the mid plane z = 1: the cross-section is a
        // 2x2 square (4 outline points), filled by 2 fan triangles whose
        // areas sum to 4.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let sv = b.section_view(Vec3::new(1.0, 1.0, 1.0), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(sv.outline.len(), 4, "square cross-section outline");
        assert_eq!(sv.facets.len(), 2, "fan fill of a quad = 2 triangles");
        let area: f64 = sv
            .facets
            .iter()
            .map(|t| 0.5 * (t[1] - t[0]).cross(t[2] - t[0]).norm())
            .sum();
        assert!((area - 4.0).abs() < 1e-9, "cut-face area {area} != 4");
        assert!((sv.normal - Vec3::new(0.0, 0.0, 1.0)).norm() < 1e-12);
    }

    #[test]
    fn render_mesh_tol_refines_curved_faces() {
        // A unit-radius, height-2 cylinder faceted at a coarse vs a fine
        // chord tolerance: finer tol yields strictly more facets, and the
        // fine facet (divergence) volume is within 1% of the analytic
        // pi r^2 h = 2 pi. The default render_mesh is unaffected.
        let mut b = Body::new();
        b.cylinder(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
            1.0,
            2.0,
        )
        .unwrap();
        let coarse = b.render_mesh_tol(0.05);
        let fine = b.render_mesh_tol(0.0005);
        assert!(
            fine.facets.len() > coarse.facets.len(),
            "finer tol -> more facets (fine {} vs coarse {})",
            fine.facets.len(),
            coarse.facets.len()
        );
        let vol = |m: &RenderMesh| {
            m.facets
                .iter()
                .map(|f| f.tri[0].dot(f.tri[1].cross(f.tri[2])))
                .sum::<f64>()
                / 6.0
        };
        let want = core::f64::consts::PI * 2.0;
        assert!(
            (vol(&fine) - want).abs() < 0.01 * want,
            "fine facet volume {} != ~{want}",
            vol(&fine)
        );
        assert!(
            !b.render_mesh().facets.is_empty(),
            "default mesh still works"
        );
    }
}
