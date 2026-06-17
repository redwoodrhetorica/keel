//! Sheet bodies (kernel/51) and THICKEN (parity item 44).
//!
//! A SHEET body is an open, double-sided lamina: one (here planar) face
//! whose boundary edges are FREE (one coedge / radial-1 each) and which
//! borders the ambient void on BOTH sides (no enclosed solid region). This
//! is the first non-solid body kind in Keel. The existing validator already
//! admits it with no change: free edges make the body non-manifold, so the
//! closed-shell Euler check auto-skips, and `check_shells_regions` requires
//! only the single infinite void region (not a solid one), which the
//! double-sided face borders on both sides.
//!
//! Operations on this representation: THICKEN (44) -> a solid of wall
//! thickness `t`; TRIM (72) by a plane; EXTEND (70) the boundary outward;
//! and KNIT/SEW (71, `boolean::knit`) which joins sheets and promotes a
//! closed result to a solid (six squares -> a cube). The MVP covers single
//! planar faces; multi-face / curved sheets (thicken via offset-both-sides
//! + a rim band, curved extend/trim) are follow-ups.

use crate::body::{Body, TopoError};
use crate::entity::{LoopKind, Side, SurfaceGeom};
use crate::lineage::Derivation;
use keel_geom::curve::{Curve3, Line3};
use keel_geom::surface::{Frame3, Plane3, Surface3};
use keel_math::vec::Vec3;

impl Body {
    /// Construct a planar SHEET body (lamina) from a closed planar profile
    /// (counterclockwise about the desired up-normal; planarity and
    /// simplicity are the caller's contract). The result is one double-sided
    /// planar face bounded by `n` free edges, with no solid region.
    pub fn planar_sheet(profile: &[Vec3]) -> Result<Body, TopoError> {
        let n = profile.len();
        if n < 3 || profile.iter().any(|p| !p.is_finite()) {
            return Err(TopoError::Precondition(
                "planar_sheet: need 3+ finite points",
            ));
        }
        // Newell's normal (robust to non-flat input, sign = winding).
        let mut nrm = Vec3::ZERO;
        for i in 0..n {
            let a = profile[i];
            let b = profile[(i + 1) % n];
            nrm = nrm
                + Vec3::new(
                    (a.y - b.y) * (a.z + b.z),
                    (a.z - b.z) * (a.x + b.x),
                    (a.x - b.x) * (a.y + b.y),
                );
        }
        let normal = nrm
            .try_normalize()
            .ok_or(TopoError::Precondition("planar_sheet: degenerate profile"))?;

        let mut b = Body::new();
        let void = b.infinite_region();
        let mut rec = b.begin_op();
        // Double-sided face: both sides border the ambient void.
        let face = b.new_face(&mut rec, void, void, Derivation::Created);
        let lp = b.new_loop(&mut rec, face, LoopKind::Outer, Derivation::Created);
        if let Some(f) = b.faces.get_mut(face) {
            f.loops.push(lp);
        }
        let verts: Vec<_> = profile.iter().map(|&p| b.new_vertex(&mut rec, p)).collect();
        let mut edges = Vec::with_capacity(n);
        let mut fins = Vec::with_capacity(n);
        for i in 0..n {
            let e = b.new_edge(
                &mut rec,
                (verts[i], verts[(i + 1) % n]),
                Derivation::Created,
            );
            let fin = b.new_fin(&mut rec, e, true, lp, Derivation::Created);
            if let Some(ed) = b.edges.get_mut(e) {
                ed.radial.push(fin); // free edge: a single coedge
            }
            edges.push(e);
            fins.push(fin);
        }
        // Splice the fin ring around the loop.
        for i in 0..n {
            let nxt = fins[(i + 1) % n];
            let prv = fins[(i + n - 1) % n];
            if let Some(f) = b.fins.get_mut(fins[i]) {
                f.next = nxt;
                f.prev = prv;
            }
        }
        if let Some(l) = b.loops.get_mut(lp) {
            l.fin = Some(fins[0]);
        }
        for i in 0..n {
            if let Some(v) = b.vertices.get_mut(verts[i]) {
                v.fin = Some(fins[i]);
            }
        }
        // One shell in the void holding both sides of the double-sided face.
        let shell = b.new_shell(&mut rec, void, Derivation::Created);
        if let Some(s) = b.shells.get_mut(shell) {
            s.faces.push((face, Side::Front));
            s.faces.push((face, Side::Back));
        }
        if let Some(r) = b.regions.get_mut(void) {
            r.shells.push(shell);
        }
        let _ = rec.finish();

        // Geometry: the plane and the boundary lines.
        let frame = Frame3::from_z(profile[0], normal)
            .map_err(|_| TopoError::Precondition("planar_sheet: degenerate frame"))?;
        b.attach_face_surface(
            face,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(frame))),
            true,
        );
        for i in 0..n {
            if let Ok(line) = Line3::new(profile[i], profile[(i + 1) % n] - profile[i]) {
                b.attach_edge_curve(edges[i], Curve3::Line(line), true);
            }
        }
        Ok(b)
    }

    /// Rectangular sheet primitive (item 7): a w x h planar sheet at
    /// `origin`, sides along `u_dir` and `normal x u_dir`. Convenience
    /// over `planar_sheet`.
    pub fn rectangular_sheet(
        origin: Vec3,
        normal: Vec3,
        u_dir: Vec3,
        w: f64,
        h: f64,
    ) -> Result<Body, TopoError> {
        if !(w.is_finite() && w > 0.0 && h.is_finite() && h > 0.0) {
            return Err(TopoError::Precondition("rectangular_sheet: bad extents"));
        }
        let n = normal
            .try_normalize()
            .ok_or(TopoError::Precondition("rectangular_sheet: bad normal"))?;
        let u = (u_dir - n * u_dir.dot(n))
            .try_normalize()
            .ok_or(TopoError::Precondition("rectangular_sheet: bad u_dir"))?;
        let v = n.cross(u);
        Body::planar_sheet(&[
            origin,
            origin + u * w,
            origin + u * w + v * h,
            origin + v * h,
        ])
    }

    /// Circular disc sheet primitive (item 7): one double-sided planar
    /// face bounded by a single FREE closed circular edge (seam vertex
    /// at +x of the disc frame), no solid region. Same lamina shape as
    /// `planar_sheet`, with the polygon ring replaced by the closed-
    /// circle loop the flat-cap solids already use.
    pub fn disc_sheet(center: Vec3, normal: Vec3, radius: f64) -> Result<Body, TopoError> {
        let ok = radius.is_finite() && radius > 0.0 && center.is_finite();
        if !ok {
            return Err(TopoError::Precondition("disc_sheet: bad radius/center"));
        }
        let frame = Frame3::from_z(center, normal)
            .map_err(|_| TopoError::Precondition("disc_sheet: degenerate normal"))?;
        let mut b = Body::new();
        let void = b.infinite_region();
        let mut rec = b.begin_op();
        let face = b.new_face(&mut rec, void, void, Derivation::Created);
        let lp = b.new_loop(&mut rec, face, LoopKind::Outer, Derivation::Created);
        if let Some(f) = b.faces.get_mut(face) {
            f.loops.push(lp);
        }
        let seam = b.new_vertex(&mut rec, center + frame.x * radius);
        let e = b.new_edge(&mut rec, (seam, seam), Derivation::Created);
        let fin = b.new_fin(&mut rec, e, true, lp, Derivation::Created);
        if let Some(ed) = b.edges.get_mut(e) {
            ed.radial.push(fin); // free closed edge: a single coedge
        }
        if let Some(f) = b.fins.get_mut(fin) {
            f.next = fin;
            f.prev = fin;
        }
        if let Some(l) = b.loops.get_mut(lp) {
            l.fin = Some(fin);
        }
        if let Some(v) = b.vertices.get_mut(seam) {
            v.fin = Some(fin);
        }
        let shell = b.new_shell(&mut rec, void, Derivation::Created);
        if let Some(s) = b.shells.get_mut(shell) {
            s.faces.push((face, Side::Front));
            s.faces.push((face, Side::Back));
        }
        if let Some(r) = b.regions.get_mut(void) {
            r.shells.push(shell);
        }
        let _ = rec.finish();

        b.attach_face_surface(
            face,
            SurfaceGeom::Analytic(Surface3::Plane(Plane3::new(frame.clone()))),
            true,
        );
        if let Ok(circle) = keel_geom::curve::Circle3::new(center, frame.x, frame.y, radius) {
            b.attach_edge_curve(e, Curve3::Circle(circle), true);
        }
        Ok(b)
    }

    /// Thicken a planar sheet body into a solid of wall thickness `t`
    /// (parity item 44), two-sided (+/- t/2 about the sheet plane). MVP: a
    /// single planar face. The thickened solid is the sheet's boundary
    /// profile extruded to thickness `t`, centred on the sheet plane.
    pub fn thicken(&self, t: f64) -> Result<Body, TopoError> {
        if t <= 0.0 || !t.is_finite() {
            return Err(TopoError::Precondition("thicken: thickness must be > 0"));
        }
        let faces = self.face_keys();
        let [face] = faces[..] else {
            return Err(TopoError::Precondition(
                "thicken: only a single-face planar sheet is supported (MVP)",
            ));
        };
        if !matches!(self.face_surface3(face), Some(Surface3::Plane(_))) {
            return Err(TopoError::Precondition("thicken: non-planar sheet (MVP)"));
        }
        let normal = self
            .face_outward_normal(face)
            .ok_or(TopoError::Precondition("thicken: no face normal"))?;
        // Boundary profile: the outer-loop vertices in fin order.
        let profile = self.face_outer_loop_points(face);
        if profile.len() < 3 {
            return Err(TopoError::Precondition("thicken: degenerate boundary"));
        }
        // Extrude centred: base shifted by -t/2 along the normal, swept +t.
        let base: Vec<Vec3> = profile.iter().map(|&p| p - normal * (t * 0.5)).collect();
        let mut solid = Body::new();
        solid.prism(&base, normal * t)?;
        Ok(solid)
    }

    /// Trim a planar sheet by a cutting plane (parity item 72), keeping the
    /// portion on the BACK side of the plane (where `(p - plane_point) .
    /// plane_normal <= 0`). The sheet's boundary polygon is clipped to that
    /// half-space (Sutherland-Hodgman against the single plane) and the
    /// kept portion rebuilt as a sheet. MVP: single planar face / convex or
    /// simple boundary; returns Err if nothing remains. (Trim by a general
    /// surface, and trimming solid faces, are follow-ups on this same clip.)
    pub fn trim_by_plane(&self, plane_point: Vec3, plane_normal: Vec3) -> Result<Body, TopoError> {
        let n = plane_normal
            .try_normalize()
            .ok_or(TopoError::Precondition("trim: degenerate plane normal"))?;
        let faces = self.face_keys();
        let [face] = faces[..] else {
            return Err(TopoError::Precondition(
                "trim: only a single-face planar sheet is supported (MVP)",
            ));
        };
        if !matches!(self.face_surface3(face), Some(Surface3::Plane(_))) {
            return Err(TopoError::Precondition("trim: non-planar sheet (MVP)"));
        }
        let poly = self.face_outer_loop_points(face);
        if poly.len() < 3 {
            return Err(TopoError::Precondition("trim: degenerate boundary"));
        }
        // Signed distance to the plane (kept side: sd <= 0).
        let sd = |p: Vec3| (p - plane_point).dot(n);
        let mut out: Vec<Vec3> = Vec::new();
        let m = poly.len();
        for i in 0..m {
            let a = poly[i];
            let b = poly[(i + 1) % m];
            let (da, db) = (sd(a), sd(b));
            let a_in = da <= 1e-12;
            let b_in = db <= 1e-12;
            if a_in {
                out.push(a);
            }
            if a_in != b_in {
                // Edge crosses the plane: add the intersection point.
                let t = da / (da - db);
                out.push(a + (b - a) * t);
            }
        }
        // Drop near-duplicate consecutive vertices from the clip.
        let mut clipped: Vec<Vec3> = Vec::with_capacity(out.len());
        for p in out {
            if clipped
                .last()
                .map(|q| (*q - p).norm() > 1e-9)
                .unwrap_or(true)
            {
                clipped.push(p);
            }
        }
        if clipped.len() >= 2 && (clipped[0] - clipped[clipped.len() - 1]).norm() <= 1e-9 {
            clipped.pop();
        }
        if clipped.len() < 3 {
            return Err(TopoError::Precondition(
                "trim: nothing remains on the kept side",
            ));
        }
        Body::planar_sheet(&clipped)
    }

    /// Extend a planar sheet's boundary outward by `d` (parity item 70,
    /// surface extension): each boundary edge's supporting line is offset
    /// outward by `d` in the sheet plane and consecutive offset lines are
    /// reintersected at the new (sharp) corners -- the planar analogue of the
    /// surface-extension core (dossier 13), here extending the trim of an
    /// infinite plane. `d < 0` shrinks. MVP: single planar convex face;
    /// returns Err on degenerate/parallel corners.
    pub fn extend(&self, d: f64) -> Result<Body, TopoError> {
        if !d.is_finite() {
            return Err(TopoError::Precondition("extend: non-finite distance"));
        }
        let faces = self.face_keys();
        let [face] = faces[..] else {
            return Err(TopoError::Precondition(
                "extend: only a single-face planar sheet is supported (MVP)",
            ));
        };
        if !matches!(self.face_surface3(face), Some(Surface3::Plane(_))) {
            return Err(TopoError::Precondition("extend: non-planar sheet (MVP)"));
        }
        let n = self
            .face_outward_normal(face)
            .ok_or(TopoError::Precondition("extend: no face normal"))?;
        let poly = self.face_outer_loop_points(face);
        let m = poly.len();
        if m < 3 {
            return Err(TopoError::Precondition("extend: degenerate boundary"));
        }
        // Project to 2D in the plane (u, v with u x v = n).
        let o = poly[0];
        let seed = if n.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let u = (seed - n * seed.dot(n))
            .try_normalize()
            .ok_or(TopoError::Precondition("extend: axis"))?;
        let v = n.cross(u);
        let p2: Vec<[f64; 2]> = poly
            .iter()
            .map(|&p| [(p - o).dot(u), (p - o).dot(v)])
            .collect();
        // Outward edge normals (sign by polygon winding) and offset-line
        // constants `q . normal = c` (shifted outward by d).
        let area2: f64 = (0..m)
            .map(|i| p2[i][0] * p2[(i + 1) % m][1] - p2[(i + 1) % m][0] * p2[i][1])
            .sum();
        let s = if area2 >= 0.0 { 1.0 } else { -1.0 };
        let mut lines: Vec<([f64; 2], f64)> = Vec::with_capacity(m);
        for i in 0..m {
            let a = p2[i];
            let b = p2[(i + 1) % m];
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-12 {
                return Err(TopoError::Precondition("extend: degenerate edge"));
            }
            let nm = [dy / len * s, -dx / len * s]; // outward unit normal
            let c = a[0] * nm[0] + a[1] * nm[1] + d;
            lines.push((nm, c));
        }
        // New corner i = intersection of offset lines (i-1) and i.
        let mut out3d: Vec<Vec3> = Vec::with_capacity(m);
        for i in 0..m {
            let (na, ca) = lines[(i + m - 1) % m];
            let (nb, cb) = lines[i];
            let det = na[0] * nb[1] - na[1] * nb[0];
            if det.abs() < 1e-12 {
                return Err(TopoError::Precondition("extend: parallel corner"));
            }
            let x = (ca * nb[1] - cb * na[1]) / det;
            let y = (na[0] * cb - nb[0] * ca) / det;
            out3d.push(o + u * x + v * y);
        }
        Body::planar_sheet(&out3d)
    }

    /// Extend a SHEET body's free boundary outward along its surface by
    /// `distance` (parity item 70, surface extension). MVP: a single
    /// planar face -- analytic-plane sheets (`planar_sheet`,
    /// `rectangular_sheet`) AND geometrically-planar NURBS sheets
    /// (`filled_sheet` of a flat boundary, before any `simplify`). The
    /// plane is recovered from the face's outward normal and boundary
    /// loop, each boundary edge's supporting line is offset outward by
    /// `distance` in that plane (the analytic continuation of the
    /// plane's trim, dossier 13), and consecutive offset lines are
    /// reintersected at the new sharp corners. `distance < 0` shrinks.
    /// DECLINES (Err) a multi-face body, a genuinely non-planar
    /// (curved) sheet, or a degenerate/parallel corner. The result is a
    /// fresh analytic-plane sheet, so it validates and `thicken`s.
    pub fn extend_sheet(&self, distance: f64) -> Result<Body, TopoError> {
        if !distance.is_finite() {
            return Err(TopoError::Precondition("extend_sheet: non-finite distance"));
        }
        let faces = self.face_keys();
        let [face] = faces[..] else {
            return Err(TopoError::Precondition(
                "extend_sheet: only a single-face sheet is supported (MVP)",
            ));
        };
        // Plane recovery: the face outward normal works for both an
        // analytic plane and a (planar) NURBS surface (it samples the
        // surface normal). The boundary loop is then verified flat.
        let n = self
            .face_outward_normal(face)
            .ok_or(TopoError::Precondition("extend_sheet: no face normal"))?;
        let poly = self.face_outer_loop_points(face);
        let m = poly.len();
        if m < 3 {
            return Err(TopoError::Precondition(
                "extend_sheet: degenerate or non-polygonal boundary (MVP: planar polygon)",
            ));
        }
        // Reject a non-planar (curved) sheet: every boundary vertex must
        // lie in the plane through poly[0] with normal `n`. A curved
        // NURBS sheet (e.g. a saddle fill) fails here and is DECLINED.
        let o0 = poly[0];
        let flat = poly
            .iter()
            .all(|&p| (p - o0).dot(n).abs() <= 1e-6 * (1.0 + (p - o0).norm()));
        if !flat {
            return Err(TopoError::Precondition(
                "extend_sheet: non-planar (curved) sheet (MVP: planar only)",
            ));
        }
        // Project to 2D in the plane (u, v with u x v = n).
        let o = poly[0];
        let seed = if n.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let u = (seed - n * seed.dot(n))
            .try_normalize()
            .ok_or(TopoError::Precondition("extend_sheet: axis"))?;
        let v = n.cross(u);
        let p2: Vec<[f64; 2]> = poly
            .iter()
            .map(|&p| [(p - o).dot(u), (p - o).dot(v)])
            .collect();
        // Outward edge normals (sign by polygon winding) and offset-line
        // constants `q . normal = c` (shifted outward by `distance`).
        let area2: f64 = (0..m)
            .map(|i| p2[i][0] * p2[(i + 1) % m][1] - p2[(i + 1) % m][0] * p2[i][1])
            .sum();
        let s = if area2 >= 0.0 { 1.0 } else { -1.0 };
        let mut lines: Vec<([f64; 2], f64)> = Vec::with_capacity(m);
        for i in 0..m {
            let a = p2[i];
            let b = p2[(i + 1) % m];
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-12 {
                return Err(TopoError::Precondition("extend_sheet: degenerate edge"));
            }
            let nm = [dy / len * s, -dx / len * s]; // outward unit normal
            let c = a[0] * nm[0] + a[1] * nm[1] + distance;
            lines.push((nm, c));
        }
        // New corner i = intersection of offset lines (i-1) and i.
        let mut out3d: Vec<Vec3> = Vec::with_capacity(m);
        for i in 0..m {
            let (na, ca) = lines[(i + m - 1) % m];
            let (nb, cb) = lines[i];
            let det = na[0] * nb[1] - na[1] * nb[0];
            if det.abs() < 1e-12 {
                return Err(TopoError::Precondition("extend_sheet: parallel corner"));
            }
            let x = (ca * nb[1] - cb * na[1]) / det;
            let y = (na[0] * cb - nb[0] * ca) / det;
            out3d.push(o + u * x + v * y);
        }
        Body::planar_sheet(&out3d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_sheet_is_a_valid_open_body() {
        // A unit square sheet: one double-sided face, four free edges, no
        // solid region. It must validate (the non-manifold free edges make
        // the closed-shell Euler check auto-skip).
        let s = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 3.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ])
        .unwrap();
        assert!(
            s.validate().is_ok(),
            "planar sheet invalid: {:?}",
            s.validate()
        );
        // Open body: no enclosed solid, so zero solid volume by the mesh.
        assert_eq!(s.face_keys().len(), 1, "sheet must have one face");
    }

    #[test]
    fn rectangular_sheet_primitive() {
        // Item 7: the rect sheet primitive thickens to the exact slab.
        let s = Body::rectangular_sheet(
            Vec3::new(1.0, 1.0, 0.5),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            3.0,
            2.0,
        )
        .unwrap();
        assert!(s.validate().is_ok(), "rect sheet invalid");
        let v = s.thicken(0.5).unwrap().mass_properties().unwrap().volume;
        assert!(
            (v - 3.0).abs() < 1e-9,
            "3x2 sheet thickened by 0.5 must be 3.0 (got {v})"
        );
    }

    #[test]
    fn disc_sheet_primitive() {
        // Item 7: the disc sheet validates and tessellates to ~pi r^2
        // (chordal tessellation undershoots a curved boundary; allow 2%
        // -- the tessellation-tolerance lesson).
        let s = Body::disc_sheet(Vec3::new(0.5, -1.0, 2.0), Vec3::new(0.0, 0.0, 1.0), 1.5).unwrap();
        assert!(
            s.validate().is_ok(),
            "disc sheet invalid: {:?}",
            s.validate()
        );
        assert_eq!(s.face_keys().len(), 1, "one face");
        let want = core::f64::consts::PI * 1.5 * 1.5;
        let area = s.face_area(s.face_keys()[0]);
        assert!(
            (area - want).abs() < 0.02 * want,
            "disc area {area} != ~{want}"
        );
    }

    #[test]
    fn extend_sheet_grows_boundary() {
        // Extend a 2x2 sheet outward by 1 -> a 4x4 sheet (each edge moves out
        // by 1). Thicken by 1 to check the area: 4*4*1 = 16.
        let s = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ])
        .unwrap();
        let big = s.extend(1.0).unwrap();
        assert!(big.validate().is_ok(), "extended sheet invalid");
        let v = big.thicken(1.0).unwrap().mass_properties().unwrap().volume;
        assert!(
            (v - 16.0).abs() < 1e-9,
            "extended-then-thickened volume must be 16 (got {v})"
        );
    }

    #[test]
    fn trim_sheet_by_plane_keeps_half() {
        // Trim a 4x4 sheet (z=0, x,y in [0,4]) by the plane x=2 (normal +x):
        // the kept BACK side is x <= 2, a 2x4 sheet. Thicken it by 1 to check
        // the trimmed area: volume 2*4*1 = 8.
        let s = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
        ])
        .unwrap();
        let half = s
            .trim_by_plane(Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        assert!(half.validate().is_ok(), "trimmed sheet invalid");
        let slab = half.thicken(1.0).unwrap();
        let v = slab.mass_properties().unwrap().volume;
        assert!(
            (v - 8.0).abs() < 1e-9,
            "trimmed-then-thickened volume must be 8 (got {v})"
        );
    }

    #[test]
    fn knit_six_sheets_into_a_cube() {
        // Knit/sew (item 71): six planar square sheets, each oriented
        // outward, knit into a closed cube and PROMOTE to a solid. Cube
        // [0,2]^3 -> volume 8 (the closure->solid promotion is the point).
        use crate::boolean::knit;
        // Order four coplanar corners CCW about the outward normal so the
        // sheet's Newell normal points outward.
        fn ccw(mut c: Vec<Vec3>, n: Vec3) -> Vec<Vec3> {
            let cen = c.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / c.len() as f64);
            let seed = if n.x.abs() < 0.9 {
                Vec3::new(1.0, 0.0, 0.0)
            } else {
                Vec3::new(0.0, 1.0, 0.0)
            };
            let u = (seed - n * seed.dot(n)).try_normalize().unwrap();
            let v = n.cross(u);
            c.sort_by(|a, b| {
                let aa = (*a - cen).dot(v).atan2((*a - cen).dot(u));
                let bb = (*b - cen).dot(v).atan2((*b - cen).dot(u));
                aa.partial_cmp(&bb).unwrap()
            });
            c
        }
        let p = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
        let defs: [(Vec<Vec3>, Vec3); 6] = [
            (
                vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(0., 2., 0.)],
                p(0., 0., -1.),
            ),
            (
                vec![p(0., 0., 2.), p(2., 0., 2.), p(2., 2., 2.), p(0., 2., 2.)],
                p(0., 0., 1.),
            ),
            (
                vec![p(0., 0., 0.), p(0., 2., 0.), p(0., 2., 2.), p(0., 0., 2.)],
                p(-1., 0., 0.),
            ),
            (
                vec![p(2., 0., 0.), p(2., 2., 0.), p(2., 2., 2.), p(2., 0., 2.)],
                p(1., 0., 0.),
            ),
            (
                vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 0., 2.), p(0., 0., 2.)],
                p(0., -1., 0.),
            ),
            (
                vec![p(0., 2., 0.), p(2., 2., 0.), p(2., 2., 2.), p(0., 2., 2.)],
                p(0., 1., 0.),
            ),
        ];
        let sheets: Vec<Body> = defs
            .iter()
            .map(|(c, n)| Body::planar_sheet(&ccw(c.clone(), *n)).unwrap())
            .collect();
        let refs: Vec<&Body> = sheets.iter().collect();
        let cube = knit(&refs, 1e-7).expect("six closed sheets must knit into a solid cube");
        assert!(cube.validate().is_ok(), "knit cube invalid");
        let v = cube.mass_properties().unwrap().volume;
        let mv = cube.mesh_volume();
        assert!(
            (v - 8.0).abs() < 1e-6 && (mv - 8.0).abs() < 1e-6,
            "knit cube volume must be 8 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn thicken_sheet_to_slab() {
        // Thicken a 2x3 planar sheet by t=0.5 -> a slab of volume
        // 2*3*0.5 = 3.0, centred on the sheet plane.
        let s = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 3.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ])
        .unwrap();
        let solid = s.thicken(0.5).unwrap();
        assert!(solid.validate().is_ok(), "thickened slab invalid");
        let v = solid.mass_properties().unwrap().volume;
        let mv = solid.mesh_volume();
        assert!(
            (v - 3.0).abs() < 1e-9 && (mv - 3.0).abs() < 1e-9,
            "thickened slab volume must be 3.0 with mass == mesh (got mass {v}, mesh {mv})"
        );
    }

    #[test]
    fn extend_sheet_grows_filled_rectangle_area() {
        // Task item 70 (extend_sheet): build a planar w x h rectangular
        // sheet via `filled_sheet` of its four sides (a NURBS-backed
        // sheet, NOT an analytic plane), extend the free boundary
        // outward by d, and confirm the area grows from w*h to
        // (w+2d)*(h+2d). The result must be a valid sheet whose analytic
        // mass (after thicken) equals the tessellated mesh.
        let (w, h, d) = (3.0_f64, 2.0_f64, 0.5_f64);
        let p = |x: f64, y: f64| Vec3::new(x, y, 0.0);
        let straight = |a: Vec3, b: Vec3, n: usize| -> Vec<Vec3> {
            (0..n)
                .map(|i| a + (b - a) * (i as f64 / (n - 1) as f64))
                .collect()
        };
        let sides = vec![
            straight(p(0., 0.), p(w, 0.), 5),
            straight(p(w, 0.), p(w, h), 5),
            straight(p(w, h), p(0., h), 5),
            straight(p(0., h), p(0., 0.), 5),
        ];
        let sheet = Body::filled_sheet(&sides, 1e-9).unwrap();
        assert!(sheet.validate().is_ok(), "filled rectangular sheet invalid");
        let a0 = sheet.face_area(sheet.face_keys()[0]);
        assert!((a0 - w * h).abs() < 1e-6, "base area {a0} != {}", w * h);

        let big = sheet.extend_sheet(d).unwrap();
        assert!(
            big.validate().is_ok(),
            "extended sheet invalid: {:?}",
            big.validate()
        );
        let want_area = (w + 2.0 * d) * (h + 2.0 * d);
        let a1 = big.face_area(big.face_keys()[0]);
        assert!(
            (a1 - want_area).abs() < 1e-6,
            "extended area must be {want_area} (got {a1})"
        );

        // Self-consistency gate: thicken to a slab and require analytic
        // mass == tessellated mesh for the grown footprint.
        let slab = big.thicken(1.0).unwrap();
        let v = slab.mass_properties().unwrap().volume;
        let mv = slab.mesh_volume();
        assert!(
            (v - want_area).abs() < 1e-9 && (mv - want_area).abs() < 1e-9,
            "grown slab volume must be {want_area} with mass == mesh (mass {v}, mesh {mv})"
        );

        // Shrink also works (negative distance), back toward the core.
        let small = sheet.extend_sheet(-0.5).unwrap();
        let a2 = small.face_area(small.face_keys()[0]);
        let want_small = (w - 1.0) * (h - 1.0);
        assert!(
            (a2 - want_small).abs() < 1e-6,
            "shrunk area must be {want_small} (got {a2})"
        );
    }
}
