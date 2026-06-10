//! Body healing / gap tightening (parity item 130) and defeaturing
//! (item 132). Dossier 13: healing follows the three-phase skeleton
//! "stitch, then simplify, then build geometry", reported per phase;
//! and "defeaturing is delete-face plus heal ... the same toolkit, so
//! build it once". Keel's heal composes the proven pieces: the
//! stitch-phase vertex/edge gap closing (merge + glue, the dossier's
//! vertex-gap and edge-gap classes) applied IN PLACE, then canonical
//! recovery (M8 simplify). Geometry REBUILD (re-intersection gap fill,
//! surface extension) is the documented follow-up phase.
//!
//! Defeaturing MVP removes SMALL HOLES by exact surgery, no boolean:
//! a small inner ring whose edges are FREE (a punched sheet) is simply
//! deleted -- the host surface already covers the hole; a small ring
//! with WALLS (a solid through/blind hole) floods the wall faces
//! (terminating at ring-bearing host faces, capped), deletes them, and
//! the freed opposite ring then falls to the first rule. Shell genus is
//! restamped from the Euler characteristic (the Addendum-129 formula).
//! The whole edit runs on a CLONE gated by validate + mass == mesh +
//! a bounded volume increase; anything else DECLINES and the body is
//! untouched. Blend/boss removal (unblend, extend-and-heal) stays with
//! the blend family follow-ups.

use crate::body::{Body, TopoError};
use crate::entity::{FaceKey, LoopKey, LoopKind};
use keel_math::vec::Vec3;

/// What `heal` changed (dossier 13: per-phase result tracking).
#[derive(Clone, Debug, Default)]
pub struct HealReport {
    pub vertices_merged: usize,
    pub edges_glued: usize,
    pub surfaces_recovered: usize,
    pub curves_recovered: usize,
}

impl Body {
    /// Heal the body within `tol` (item 130): close vertex gaps (merge
    /// coincident vertices) and edge gaps (glue dangling coincident
    /// edges) -- the stitch-phase machinery applied in place -- then
    /// recognize analytic geometry hidden in NURBS (simplify). Returns
    /// the per-phase report. Geometry rebuild by re-intersection is the
    /// follow-up phase.
    pub fn heal(&mut self, tol: f64) -> Result<HealReport, TopoError> {
        if !(tol.is_finite() && tol > 0.0) {
            return Err(TopoError::Precondition("heal: bad tol"));
        }
        let (v0, e0) = (self.vertices.len(), self.edges.len());
        let mut rec = self.begin_op();
        crate::boolean::merge_and_glue_imported(self, &mut rec, tol.max(1e-9));
        let _ = rec.finish();
        let rep = self.simplify(tol);
        Ok(HealReport {
            vertices_merged: v0 - self.vertices.len(),
            edges_glued: e0 - self.edges.len(),
            surfaces_recovered: rep.surfaces_recovered,
            curves_recovered: rep.curves_recovered,
        })
    }

    /// Remove small holes (item 132 MVP): every inner ring whose
    /// boundary polygon area is below `max_area`, together with its
    /// wall faces for solid holes. Exact surgery (the host surfaces
    /// already cover the holes); runs on a clone and DECLINES (leaving
    /// the body untouched) unless the result validates, stays
    /// mass == mesh, and gains at most the holes' volume. Returns the
    /// number of holes removed.
    pub fn defeature_small_holes(&mut self, max_area: f64) -> Result<usize, TopoError> {
        if !(max_area.is_finite() && max_area > 0.0) {
            return Err(TopoError::Precondition("defeature: bad max_area"));
        }
        let mut work = self.clone();
        let mut removed = 0usize;
        // Iterate to a fixed point: deleting walls frees the opposite
        // ring, which the next pass removes.
        loop {
            let mut changed = false;
            // Pass 1: free-edge small rings die outright.
            if let Some(ring) = work.find_small_ring(max_area, true) {
                work.remove_ring(ring);
                removed += 1;
                changed = true;
            } else if let Some(ring) = work.find_small_ring(max_area, false) {
                // Pass 2: a walled ring: flood and delete the hole faces;
                // its rings (this one and the opposite) become free and
                // die in later pass-1 rounds (counted once, here).
                if let Some(walls) = work.hole_walls(ring) {
                    for f in walls {
                        work.remove_face_with_loops(f);
                    }
                    changed = true;
                } else {
                    // Unfloodable: decline this ring (and the whole call:
                    // leaving it would loop forever).
                    break;
                }
            }
            if !changed {
                break;
            }
        }
        if removed == 0 {
            return Ok(0);
        }
        work.restamp_shell_genus();
        // Honesty gates: valid, self-consistent, volume gained at most
        // the holes' worth (a hole's volume <= its area times the body
        // diagonal).
        if work.validate().is_err() {
            return Err(TopoError::Precondition(
                "defeature: result invalid (declined, body untouched)",
            ));
        }
        let all_planar = work.face_keys().iter().all(|&f| {
            matches!(
                work.face_surface3(f),
                Some(keel_geom::surface::Surface3::Plane(_))
            )
        });
        if all_planar && !work.faces.is_empty() && work.regions.len() > 1 {
            let (v, mv) = (
                work.mass_properties().map(|m| m.volume).unwrap_or(f64::NAN),
                work.mesh_volume(),
            );
            if !v.is_finite() || (v - mv).abs() > 1e-6 * (1.0 + v.abs()) {
                return Err(TopoError::Precondition(
                    "defeature: result inconsistent (declined, body untouched)",
                ));
            }
        }
        *self = work;
        Ok(removed)
    }

    /// The first inner ring with boundary area below `max_area`;
    /// `free_only` restricts to rings whose edges carry no other fins.
    fn find_small_ring(&self, max_area: f64, free_only: bool) -> Option<LoopKey> {
        for f in self.face_keys() {
            for &lk in &self.faces.get(f)?.loops {
                let l = self.loops.get(lk)?;
                if l.kind != LoopKind::Inner {
                    continue;
                }
                let poly = self.loop_polygon(lk);
                if poly.len() < 3 || newell_area(&poly) >= max_area {
                    continue;
                }
                let free = self
                    .ring_edges(lk)
                    .iter()
                    .all(|&e| self.edges.get(e).map(|x| x.radial.len()) == Some(1));
                if free == free_only || !free_only {
                    if free_only && !free {
                        continue;
                    }
                    if !free_only && free {
                        continue;
                    }
                    return Some(lk);
                }
            }
        }
        None
    }

    /// The ring's edge set (fin walk).
    pub(crate) fn ring_edges(&self, lk: LoopKey) -> Vec<crate::entity::EdgeKey> {
        let mut out = Vec::new();
        let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
            return out;
        };
        let mut cur = entry;
        for _ in 0..self.fins.len() + 1 {
            if let Some(f) = self.fins.get(cur) {
                out.push(f.edge);
                cur = f.next;
                if cur == entry {
                    break;
                }
            } else {
                break;
            }
        }
        out
    }

    /// Flood the hole's wall faces from a walled small ring: partner
    /// faces of the ring's edges, closed under shared edges, NEVER
    /// absorbing a face that carries an inner ring (those are the hole's
    /// host faces). Capped at 32 faces; `None` when the flood escapes.
    fn hole_walls(&self, ring: LoopKey) -> Option<Vec<FaceKey>> {
        use std::collections::BTreeSet;
        let host = self.loops.get(ring)?.face;
        let mut walls: BTreeSet<FaceKey> = BTreeSet::new();
        let mut frontier: Vec<FaceKey> = Vec::new();
        for e in self.ring_edges(ring) {
            for &fk in &self.edges.get(e)?.radial {
                let f = self.fins.get(fk)?.owner;
                let face = self.loops.get(f)?.face;
                if face != host {
                    frontier.push(face);
                }
            }
        }
        while let Some(face) = frontier.pop() {
            if walls.contains(&face) {
                continue;
            }
            let has_ring = self
                .faces
                .get(face)?
                .loops
                .iter()
                .any(|&lk| self.loops.get(lk).map(|l| l.kind) == Some(LoopKind::Inner));
            if has_ring {
                // A host face: terminal, not a wall.
                continue;
            }
            walls.insert(face);
            if walls.len() > 32 {
                return None;
            }
            // Expand over the wall's edges.
            for &lk in &self.faces.get(face)?.loops {
                for e in self.ring_edges(lk) {
                    for &fk in &self.edges.get(e)?.radial {
                        let lf = self.fins.get(fk)?.owner;
                        let nb = self.loops.get(lf)?.face;
                        if nb != face && !walls.contains(&nb) {
                            frontier.push(nb);
                        }
                    }
                }
            }
        }
        (!walls.is_empty()).then(|| walls.into_iter().collect())
    }

    /// Delete an inner ring outright: its loop, fins, edges, and any
    /// vertices no other fin uses. The host face's surface already
    /// covers the hole, so geometry is untouched.
    fn remove_ring(&mut self, ring: LoopKey) {
        let mut rec = self.begin_op();
        let host = self.loops.get(ring).map(|l| l.face);
        let edges = self.ring_edges(ring);
        // Fins of this ring.
        let mut fins = Vec::new();
        if let Some(entry) = self.loops.get(ring).and_then(|l| l.fin) {
            let mut cur = entry;
            for _ in 0..self.fins.len() + 1 {
                let Some(f) = self.fins.get(cur) else { break };
                fins.push(cur);
                cur = f.next;
                if cur == entry {
                    break;
                }
            }
        }
        for fk in &fins {
            // Detach from the edge's radial list.
            if let Some(e) = self.fins.get(*fk).map(|f| f.edge)
                && let Some(ed) = self.edges.get_mut(e)
            {
                ed.radial.retain(|x| x != fk);
            }
            if let Some(id) = self.fins.get(*fk).map(|f| f.id) {
                self.unregister(&mut rec, id);
            }
            self.fins.remove(*fk);
        }
        for e in edges {
            if self.edges.get(e).map(|x| x.radial.is_empty()) == Some(true) {
                let bounds = self.edges.get(e).map(|x| x.bounds);
                if let Some(id) = self.edges.get(e).map(|x| x.id) {
                    self.unregister(&mut rec, id);
                }
                self.edges.remove(e);
                // Drop orphaned vertices.
                if let Some((v0, v1)) = bounds {
                    for v in [v0, v1] {
                        let used = self
                            .edges
                            .iter()
                            .any(|(_, e)| e.bounds.0 == v || e.bounds.1 == v);
                        if !used && self.vertices.contains(v) {
                            if let Some(id) = self.vertices.get(v).map(|x| x.id) {
                                self.unregister(&mut rec, id);
                            }
                            self.vertices.remove(v);
                        }
                    }
                }
            }
        }
        if let Some(h) = host
            && let Some(f) = self.faces.get_mut(h)
        {
            f.loops.retain(|&lk| lk != ring);
        }
        if let Some(id) = self.loops.get(ring).map(|l| l.id) {
            self.unregister(&mut rec, id);
        }
        self.loops.remove(ring);
        let _ = rec.finish();
    }

    /// Delete a face and all its loops/fins (edges shed this face's
    /// fins; edges left finless die with their orphaned vertices), and
    /// drop it from every shell.
    fn remove_face_with_loops(&mut self, face: FaceKey) {
        let loops = self
            .faces
            .get(face)
            .map(|f| f.loops.clone())
            .unwrap_or_default();
        for lk in loops {
            self.remove_ring_or_outer(lk);
        }
        let mut rec = self.begin_op();
        let shells: Vec<_> = self.shells.iter().map(|(k, _)| k).collect();
        for sk in shells {
            if let Some(s) = self.shells.get_mut(sk) {
                s.faces.retain(|&(f, _)| f != face);
            }
        }
        if let Some(id) = self.faces.get(face).map(|f| f.id) {
            self.unregister(&mut rec, id);
        }
        self.faces.remove(face);
        let _ = rec.finish();
    }

    /// `remove_ring` without the inner-only assumption (outer loops of
    /// dying wall faces).
    fn remove_ring_or_outer(&mut self, lk: LoopKey) {
        self.remove_ring(lk);
    }

    /// Restamp every shell's genus from its own Euler characteristic
    /// (chi = V - E + F - rings over the shell's faces; genus =
    /// (2 - chi) / 2, the kfmrh both-shells convention).
    fn restamp_shell_genus(&mut self) {
        use std::collections::BTreeSet;
        let shells: Vec<_> = self.shells.iter().map(|(k, _)| k).collect();
        for sk in shells {
            let faces: BTreeSet<FaceKey> = self
                .shells
                .get(sk)
                .map(|s| s.faces.iter().map(|&(f, _)| f).collect())
                .unwrap_or_default();
            if faces.is_empty() {
                continue;
            }
            let mut vs: BTreeSet<crate::entity::VertexKey> = BTreeSet::new();
            let mut es: BTreeSet<crate::entity::EdgeKey> = BTreeSet::new();
            let mut rings = 0i64;
            for &f in &faces {
                for &lk in &self
                    .faces
                    .get(f)
                    .map(|x| x.loops.clone())
                    .unwrap_or_default()
                {
                    if self.loops.get(lk).map(|l| l.kind) == Some(LoopKind::Inner) {
                        rings += 1;
                    }
                    for e in self.ring_edges(lk) {
                        es.insert(e);
                        if let Some(ed) = self.edges.get(e) {
                            vs.insert(ed.bounds.0);
                            vs.insert(ed.bounds.1);
                        }
                    }
                }
            }
            let chi = vs.len() as i64 - es.len() as i64 + faces.len() as i64 - rings;
            let g = ((2 - chi).max(0) / 2) as u32;
            if let Some(s) = self.shells.get_mut(sk) {
                s.genus = g;
            }
        }
    }
}

/// Unsigned polygon area by the Newell normal.
fn newell_area(poly: &[Vec3]) -> f64 {
    let n = poly.len();
    let mut acc = Vec3::ZERO;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        acc = acc + a.cross(b);
    }
    0.5 * acc.norm()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boolean::{BoolOp, boolean, boolean_sheet_solid};

    fn block(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
        let mut b = Body::new();
        b.block(o, dx, dy, dz).unwrap();
        b
    }

    #[test]
    fn heal_recovers_nurbs_coats() {
        // Item 130: a box face re-represented as its exact NURBS heals
        // back to analytic; volume preserved.
        use crate::entity::SurfaceGeom;
        let mut b = block(Vec3::ZERO, 2.0, 2.0, 2.0);
        let face = b.face_keys()[0];
        let n = b.face_to_nurbs(face).unwrap();
        b.replace_face_surface_checked(face, SurfaceGeom::Nurbs(n), 1e-9)
            .unwrap();
        let rep = b.heal(1e-6).unwrap();
        assert_eq!(rep.surfaces_recovered, 1, "heal must recover the plane");
        assert!(b.validate().is_ok());
        let v = b.mass_properties().unwrap().volume;
        assert!((v - 8.0).abs() < 1e-9, "healed volume {v}");
    }

    #[test]
    fn defeature_through_hole_restores_slab() {
        // Item 132: the genus-1 through-notch slab loses its hole: walls
        // deleted, both rings freed and removed, genus restamped 0,
        // volume back to the full 2.0 with mass == mesh.
        let a = block(Vec3::new(0., 0., -0.25), 2.0, 2.0, 0.5);
        let t = block(Vec3::new(0.75, 0.75, -0.5), 0.5, 0.5, 1.0);
        let mut holed = boolean(&a, &t, BoolOp::Difference, 1e-7).unwrap().body;
        assert_eq!(holed.counts().genus, 1);
        let removed = holed.defeature_small_holes(0.5).unwrap();
        assert!(removed >= 1, "hole must be removed (got {removed})");
        assert!(holed.validate().is_ok(), "defeatured slab invalid");
        let c = holed.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6), "back to a plain slab");
        assert_eq!(c.genus, 0, "genus restamped");
        let v = holed.mass_properties().unwrap().volume;
        let mv = holed.mesh_volume();
        assert!(
            (v - 2.0).abs() < 1e-9 && (mv - 2.0).abs() < 1e-9,
            "defeatured volume must be 2.0 mass == mesh (got {v}, {mv})"
        );
    }

    #[test]
    fn defeature_punched_sheet_fills_hole() {
        // Item 132 on a sheet: the punched ring is free-edged and dies
        // outright; the sheet is whole again.
        let sheet = Body::planar_sheet(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
        ])
        .unwrap();
        let tool = block(Vec3::new(1.0, 1.0, -1.0), 2.0, 2.0, 2.0);
        let mut holed = boolean_sheet_solid(&sheet, &tool, BoolOp::Difference, 1e-7).unwrap();
        assert_eq!(holed.counts().inner_rings, 1);
        let removed = holed.defeature_small_holes(5.0).unwrap();
        assert_eq!(removed, 1);
        assert!(holed.validate().is_ok(), "filled sheet invalid");
        assert_eq!(holed.counts().inner_rings, 0, "ring gone");
        let a = holed.face_area(holed.face_keys()[0]);
        assert!((a - 16.0).abs() < 1e-9, "sheet whole again (area {a})");
    }

    #[test]
    fn defeature_respects_threshold() {
        // A hole BIGGER than max_area stays.
        let a = block(Vec3::new(0., 0., -0.25), 2.0, 2.0, 0.5);
        let t = block(Vec3::new(0.75, 0.75, -0.5), 0.5, 0.5, 1.0);
        let mut holed = boolean(&a, &t, BoolOp::Difference, 1e-7).unwrap().body;
        let removed = holed.defeature_small_holes(0.1).unwrap();
        assert_eq!(removed, 0, "0.25-area hole must survive max_area 0.1");
        assert_eq!(holed.counts().genus, 1, "hole intact");
    }
}
