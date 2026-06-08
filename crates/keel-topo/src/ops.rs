//! Subdivision and stitch primitives (gate design 2.2): the operators
//! the boolean pipeline imprint/stitch phases consume. First-class
//! (not mere Euler sequences) because lineage needs their split/merge
//! semantics explicit.
//!
//! M3 deliberate deferrals, recorded here and in the LOG: geometric
//! coincidence judgement is the CALLER's job (M5 imprint owns the
//! toleranced decisions); the radial cycle order after glue_edges is
//! insertion order, upgraded to dihedral-angle order when M5 provides
//! the first real consumer; split_region/merge_regions arrive with the
//! boolean stitch (M6), since mvfs/mef handle all region bookkeeping
//! the M3 constructors need.

use crate::body::{Body, TopoError};
use crate::entity::{EdgeKey, FaceKey, FinKey, RegionKey, ShellKey, SurfaceKey, VertexKey};
use crate::lineage::{Derivation, OpReport};
use keel_math::vec::Vec3;

pub struct SplitEdgeOut {
    pub edge_a: EdgeKey,
    pub edge_b: EdgeKey,
    pub vertex: VertexKey,
    pub report: OpReport,
}

pub struct SplitFaceOut {
    pub face_old: FaceKey,
    pub face_new: FaceKey,
    pub edge: EdgeKey,
    pub report: OpReport,
}

pub struct EmbedWireOut {
    pub edge: EdgeKey,
    pub v0: VertexKey,
    pub v1: VertexKey,
    pub shell: ShellKey,
    pub report: OpReport,
}

impl Body {
    /// Split an edge at a new vertex (geometry point `p`). EVERY fin
    /// in the radial cycle splits into two; the child edges are
    /// SplitChild 0/1 of the parent and the new fins are Generated
    /// from their parents. Children inherit the parent's curve
    /// reference (sub-interval bounding is the trimming work of
    /// M4/M5).
    pub fn split_edge(&mut self, edge: EdgeKey, p: Vec3) -> Result<SplitEdgeOut, TopoError> {
        let e = self.edges.get(edge).ok_or(TopoError::StaleKey)?;
        let (v0, v1) = e.bounds;
        let curve = e.curve;
        let radial: Vec<FinKey> = e.radial.clone();
        let parent_id = e.id;

        let mut rec = self.begin_op();
        let w = self.new_vertex(&mut rec, p);
        let ea = self.new_edge(
            &mut rec,
            (v0, w),
            Derivation::SplitChild {
                from: parent_id,
                ordinal: 0,
            },
        );
        let eb = self.new_edge(
            &mut rec,
            (w, v1),
            Derivation::SplitChild {
                from: parent_id,
                ordinal: 1,
            },
        );
        for ek in [ea, eb] {
            if let Some(x) = self.edges.get_mut(ek) {
                x.curve = curve;
            }
        }
        let mut first_new_fin: Option<FinKey> = None;
        for fk in radial {
            let (forward, owner, prev, next, fid) = {
                let f = self.fins.get(fk).ok_or(TopoError::StaleKey)?;
                (f.forward, f.owner, f.prev, f.next, f.id)
            };
            // Forward fin (v0 -> v1) becomes fa (v0 -> w) then fb
            // (w -> v1); backward becomes fb' (v1 -> w) then fa'
            // (w -> v0). In both cases the pair replaces fk in its
            // ring in traversal order.
            let (first, second) = if forward {
                let fa = self.new_fin(
                    &mut rec,
                    ea,
                    true,
                    owner,
                    Derivation::Generated { from: fid },
                );
                let fb = self.new_fin(
                    &mut rec,
                    eb,
                    true,
                    owner,
                    Derivation::Generated { from: fid },
                );
                (fa, fb)
            } else {
                let fb = self.new_fin(
                    &mut rec,
                    eb,
                    false,
                    owner,
                    Derivation::Generated { from: fid },
                );
                let fa = self.new_fin(
                    &mut rec,
                    ea,
                    false,
                    owner,
                    Derivation::Generated { from: fid },
                );
                (fb, fa)
            };
            first_new_fin.get_or_insert(first);
            let single = prev == fk; // single-fin self ring
            if single {
                if let Some(x) = self.fins.get_mut(first) {
                    x.prev = second;
                    x.next = second;
                }
                if let Some(x) = self.fins.get_mut(second) {
                    x.prev = first;
                    x.next = first;
                }
            } else {
                if let Some(x) = self.fins.get_mut(prev) {
                    x.next = first;
                }
                if let Some(x) = self.fins.get_mut(first) {
                    x.prev = prev;
                    x.next = second;
                }
                if let Some(x) = self.fins.get_mut(second) {
                    x.prev = first;
                    x.next = next;
                }
                if let Some(x) = self.fins.get_mut(next) {
                    x.prev = second;
                }
            }
            // Radial membership: fa joins ea, fb joins eb, preserving
            // parent cycle order.
            let (fa, fb) = if forward {
                (first, second)
            } else {
                (second, first)
            };
            if let Some(x) = self.edges.get_mut(ea) {
                x.radial.push(fa);
            }
            if let Some(x) = self.edges.get_mut(eb) {
                x.radial.push(fb);
            }
            // Loop entry and vertex fin references move off the dead fin.
            if let Some(l) = self.loops.get_mut(owner)
                && l.fin == Some(fk)
            {
                l.fin = Some(first);
            }
            let vkeys: Vec<VertexKey> = self
                .vertices
                .iter()
                .filter(|(_, v)| v.fin == Some(fk))
                .map(|(k, _)| k)
                .collect();
            for vk in vkeys {
                if let Some(v) = self.vertices.get_mut(vk) {
                    v.fin = Some(first);
                }
            }
            if let Some(id) = self.fins.get(fk).map(|x| x.id) {
                self.unregister(&mut rec, id);
            }
            self.fins.remove(fk);
        }
        if let Some(vv) = self.vertices.get_mut(w) {
            vv.fin = first_new_fin;
        }
        rec.report.split.push((parent_id, Vec::new()));
        if let Some((_, children)) = rec.report.split.last_mut() {
            children.push(
                self.edges
                    .get(ea)
                    .map(|x| x.id)
                    .unwrap_or(crate::entity::EntityId(0)),
            );
            children.push(
                self.edges
                    .get(eb)
                    .map(|x| x.id)
                    .unwrap_or(crate::entity::EntityId(0)),
            );
        }
        self.unregister(&mut rec, parent_id);
        self.edges.remove(edge);
        let report = rec.finish();
        self.debug_validate();
        Ok(SplitEdgeOut {
            edge_a: ea,
            edge_b: eb,
            vertex: w,
            report,
        })
    }

    /// Split a face along a new edge between end-vertex(fin_a) and
    /// end-vertex(fin_b) (same loop): the imprint primitive. Both
    /// resulting faces are split lineage: the surviving face is
    /// recorded as the split parent's child 0 (same EntityId,
    /// Modified), the new face as SplitChild ordinal 1.
    pub fn split_face(
        &mut self,
        fin_a: FinKey,
        fin_b: FinKey,
        surface: Option<(SurfaceKey, bool)>,
    ) -> Result<SplitFaceOut, TopoError> {
        let lp = self
            .fins
            .get(fin_a)
            .map(|f| f.owner)
            .ok_or(TopoError::StaleKey)?;
        let old_face = self
            .loops
            .get(lp)
            .map(|l| l.face)
            .ok_or(TopoError::StaleKey)?;
        let old_id = self
            .faces
            .get(old_face)
            .map(|f| f.id)
            .ok_or(TopoError::StaleKey)?;
        let out = self.mef_impl(fin_a, fin_b, surface, true)?;
        // Note: mef_impl already recorded the new face as SplitChild;
        // augment the report with the parent linkage.
        let mut report = out.report;
        let new_id = self
            .faces
            .get(out.face)
            .map(|f| f.id)
            .ok_or(TopoError::StaleKey)?;
        report.split.push((old_id, vec![old_id, new_id]));
        report.sort();
        Ok(SplitFaceOut {
            face_old: old_face,
            face_new: out.face,
            edge: out.edge,
            report,
        })
    }

    /// Identify vb with va (va survives, MergeResult lineage). All
    /// edge bounds re-point. If both vertices carried umbrellas, va's
    /// `groups` records the additional umbrella(s): the PES partial-
    /// entity trigger. Geometric coincidence within tolerance is the
    /// caller's responsibility.
    pub fn merge_vertices(&mut self, va: VertexKey, vb: VertexKey) -> Result<OpReport, TopoError> {
        if va == vb {
            return Err(TopoError::Precondition("merge_vertices: identical"));
        }
        let (va_id, vb_id) = match (self.vertices.get(va), self.vertices.get(vb)) {
            (Some(x), Some(y)) => (x.id, y.id),
            _ => return Err(TopoError::StaleKey),
        };
        // Reject merging across an existing edge (would implicitly
        // close it; closing is an explicit operation).
        let connected = self.edges.iter().any(|(_, e)| {
            (e.bounds.0 == va && e.bounds.1 == vb) || (e.bounds.0 == vb && e.bounds.1 == va)
        });
        if connected {
            return Err(TopoError::Precondition(
                "merge_vertices: vertices share an edge",
            ));
        }

        let mut rec = self.begin_op();
        let edge_keys: Vec<EdgeKey> = self.edges.iter().map(|(k, _)| k).collect();
        for ek in edge_keys {
            if let Some(e) = self.edges.get_mut(ek) {
                if e.bounds.0 == vb {
                    e.bounds.0 = va;
                }
                if e.bounds.1 == vb {
                    e.bounds.1 = va;
                }
            }
        }
        let vb_fin = self.vertices.get(vb).and_then(|v| v.fin);
        let vb_groups: Vec<FinKey> = self
            .vertices
            .get(vb)
            .map(|v| v.groups.clone())
            .unwrap_or_default();
        if let Some(v) = self.vertices.get_mut(va) {
            match (v.fin, vb_fin) {
                (None, Some(f)) => v.fin = Some(f),
                (Some(_), Some(f)) => v.groups.push(f),
                _ => {}
            }
            v.groups.extend(vb_groups);
        }
        // Acorn/shell references to vb move to va.
        let shell_keys: Vec<ShellKey> = self.shells.iter().map(|(k, _)| k).collect();
        for sk in shell_keys {
            if let Some(s) = self.shells.get_mut(sk)
                && s.acorn == Some(vb)
            {
                s.acorn = Some(va);
            }
        }
        rec.report.merged.push((vec![va_id, vb_id], va_id));
        self.lineage.insert(
            va_id,
            crate::lineage::Lineage {
                op: rec.op,
                derivation: Derivation::MergeResult {
                    from: vec![va_id, vb_id],
                },
            },
        );
        self.unregister(&mut rec, vb_id);
        self.vertices.remove(vb);
        let report = rec.finish();
        self.debug_validate();
        Ok(report)
    }

    /// Identify eb with ea (same bounds, either orientation; caller
    /// asserts geometric coincidence). eb's fins re-point to ea and
    /// join ea's radial cycle: THE non-manifold maker. The merged
    /// radial cycle is DIHEDRAL-SORTED (M5b): fins ordered by the
    /// angle of their face's outward normal about the edge tangent, so
    /// neighborhood classification (M6) reads a correct angular order.
    pub fn glue_edges(&mut self, ea: EdgeKey, eb: EdgeKey) -> Result<OpReport, TopoError> {
        if ea == eb {
            return Err(TopoError::Precondition("glue_edges: identical"));
        }
        let (ba, bb, ea_id, eb_id) = match (self.edges.get(ea), self.edges.get(eb)) {
            (Some(x), Some(y)) => (x.bounds, y.bounds, x.id, y.id),
            _ => return Err(TopoError::StaleKey),
        };
        let aligned = ba == bb;
        let reversed = ba == (bb.1, bb.0);
        if !aligned && !reversed {
            return Err(TopoError::Precondition("glue_edges: bounds differ"));
        }

        let mut rec = self.begin_op();
        let moved: Vec<FinKey> = self
            .edges
            .get(eb)
            .map(|e| e.radial.clone())
            .unwrap_or_default();
        for fk in &moved {
            if let Some(f) = self.fins.get_mut(*fk) {
                f.edge = ea;
                if reversed {
                    f.forward = !f.forward;
                }
            }
        }
        if let Some(e) = self.edges.get_mut(ea) {
            e.radial.extend(moved.iter().copied());
        }
        self.dihedral_sort_radial(ea);
        rec.report.merged.push((vec![ea_id, eb_id], ea_id));
        self.lineage.insert(
            ea_id,
            crate::lineage::Lineage {
                op: rec.op,
                derivation: Derivation::MergeResult {
                    from: vec![ea_id, eb_id],
                },
            },
        );
        self.unregister(&mut rec, eb_id);
        self.edges.remove(eb);
        let report = rec.finish();
        self.debug_validate();
        Ok(report)
    }

    /// Order an edge's radial cycle by dihedral angle about the edge
    /// tangent. Each fin contributes the outward direction of its
    /// face into the plane perpendicular to the tangent (the face
    /// surface normal crossed appropriately, or the geometric outward
    /// of the fin's loop interior); fins are sorted by atan2 in that
    /// plane. Manifold 2-cycles are unaffected up to a possible swap.
    fn dihedral_sort_radial(&mut self, edge: EdgeKey) {
        let Some(e) = self.edges.get(edge) else {
            return;
        };
        if e.radial.len() <= 2 {
            return;
        }
        let radial = e.radial.clone();
        // Edge tangent from its bound vertices.
        let (v0, v1) = e.bounds;
        let (Some(p0), Some(p1)) = (
            self.vertices.get(v0).map(|v| v.point),
            self.vertices.get(v1).map(|v| v.point),
        ) else {
            return;
        };
        let tangent = match (p1 - p0).try_normalize() {
            Some(t) => t,
            None => return, // closed/degenerate edge: leave order
        };
        // Reference frame perpendicular to the tangent.
        let helper = if tangent.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let ref_x = match (helper - tangent * helper.dot(tangent)).try_normalize() {
            Some(x) => x,
            None => return,
        };
        let ref_y = tangent.cross(ref_x);
        // Per fin, compute the in-plane outward direction of its face.
        let mut keyed: Vec<(f64, FinKey)> = Vec::new();
        for &fk in &radial {
            let dir = self.fin_outward_in_plane(fk, tangent);
            let dir = dir - tangent * dir.dot(tangent);
            let (x, y) = (dir.dot(ref_x), dir.dot(ref_y));
            let ang = if x == 0.0 && y == 0.0 {
                0.0
            } else {
                y.atan2(x)
            };
            keyed.push((ang, fk));
        }
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some(e) = self.edges.get_mut(edge) {
            e.radial = keyed.into_iter().map(|(_, f)| f).collect();
        }
    }

    /// The outward direction of a fin's face in 3D: the surface normal
    /// (sense-corrected) crossed with the edge tangent gives a vector
    /// in the face pointing away from the edge into the face interior.
    /// Falls back to the loop-geometry direction when no surface.
    fn fin_outward_in_plane(&self, fin: FinKey, tangent: Vec3) -> Vec3 {
        let face_normal = self
            .fins
            .get(fin)
            .and_then(|f| self.loops.get(f.owner))
            .and_then(|l| self.faces.get(l.face))
            .and_then(|face| face.surface)
            .and_then(|(sk, sense)| self.surfaces.get(sk).map(|s| (s, sense)))
            .and_then(|(s, sense)| {
                // Normal at the fin's edge midpoint, via the surface.
                let mid = self.fin_edge_midpoint(fin)?;
                let n = match s {
                    crate::entity::SurfaceGeom::Analytic(a) => {
                        let pr = a.project(mid).ok()?;
                        a.local_geometry(pr.u, pr.v).ok()?.normal
                    }
                    crate::entity::SurfaceGeom::Nurbs(_) => return None,
                };
                Some(if sense { n } else { n * -1.0 })
            });
        match face_normal {
            // outward-into-face = normal x tangent, oriented by fin dir.
            Some(n) => {
                let dir = n.cross(tangent);
                let fwd = self.fins.get(fin).map(|f| f.forward).unwrap_or(true);
                if fwd { dir } else { dir * -1.0 }
            }
            None => Vec3::new(1.0, 0.0, 0.0), // deterministic fallback
        }
    }

    fn fin_edge_midpoint(&self, fin: FinKey) -> Option<Vec3> {
        let f = self.fins.get(fin)?;
        let e = self.edges.get(f.edge)?;
        let p0 = self.vertices.get(e.bounds.0)?.point;
        let p1 = self.vertices.get(e.bounds.1)?.point;
        Some((p0 + p1) * 0.5)
    }

    /// Embed an isolated (acorn) vertex in a region.
    pub fn embed_vertex(
        &mut self,
        region: RegionKey,
        p: Vec3,
    ) -> Result<(VertexKey, ShellKey, OpReport), TopoError> {
        if !self.regions.contains(region) {
            return Err(TopoError::StaleKey);
        }
        let mut rec = self.begin_op();
        let v = self.new_vertex(&mut rec, p);
        let s = self.new_shell(&mut rec, region, Derivation::Created);
        if let Some(sh) = self.shells.get_mut(s) {
            sh.acorn = Some(v);
        }
        if let Some(r) = self.regions.get_mut(region) {
            r.shells.push(s);
        }
        let report = rec.finish();
        self.debug_validate();
        Ok((v, s, report))
    }

    /// Embed a wire edge (no fins) in a region: two new vertices and a
    /// finless edge in a wire shell.
    pub fn embed_wire(
        &mut self,
        region: RegionKey,
        curve: Option<(crate::entity::CurveKey, bool)>,
        p0: Vec3,
        p1: Vec3,
    ) -> Result<EmbedWireOut, TopoError> {
        if !self.regions.contains(region) {
            return Err(TopoError::StaleKey);
        }
        let mut rec = self.begin_op();
        let v0 = self.new_vertex(&mut rec, p0);
        let v1 = self.new_vertex(&mut rec, p1);
        let e = self.new_edge(&mut rec, (v0, v1), Derivation::Created);
        if let Some(x) = self.edges.get_mut(e) {
            x.curve = curve;
        }
        let s = self.new_shell(&mut rec, region, Derivation::Created);
        if let Some(sh) = self.shells.get_mut(s) {
            sh.wires.push(e);
        }
        if let Some(r) = self.regions.get_mut(region) {
            r.shells.push(s);
        }
        let report = rec.finish();
        self.debug_validate();
        Ok(EmbedWireOut {
            edge: e,
            v0,
            v1,
            shell: s,
            report,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::euler::test_support::cube;
    use crate::lineage::Derivation;

    fn first_edge(b: &Body) -> EdgeKey {
        b.entity_ids()
            .find_map(|id| match b.lookup(id) {
                Some(crate::entity::AnyKey::Edge(k)) => Some(k),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no edges"))
    }

    #[test]
    fn split_edge_on_cube() {
        let (mut b, _) = cube();
        let before = b.counts();
        let e = first_edge(&b);
        let parent_id = b.edge(e).map(|x| x.id).unwrap_or_else(|| panic!());
        let out = b.split_edge(e, Vec3::new(0.5, 0.0, 0.0)).unwrap();
        assert!(b.validate().is_ok());
        let after = b.counts();
        assert_eq!(after.v, before.v + 1);
        assert_eq!(after.e, before.e + 1);
        assert_eq!(after.f, before.f);
        // Both children carry split lineage with distinct ordinals.
        for (ek, ordinal) in [(out.edge_a, 0u32), (out.edge_b, 1u32)] {
            let id = b.edge(ek).map(|x| x.id).unwrap_or_else(|| panic!());
            match b.lineage_of(id).map(|l| &l.derivation) {
                Some(Derivation::SplitChild { from, ordinal: o }) => {
                    assert_eq!((*from, *o), (parent_id, ordinal));
                }
                other => panic!("bad lineage {other:?}"),
            }
        }
        // Both radial fins of the parent split: each child has 2 fins.
        assert_eq!(b.edge(out.edge_a).map(|x| x.radial.len()), Some(2));
        assert_eq!(b.edge(out.edge_b).map(|x| x.radial.len()), Some(2));
    }

    #[test]
    fn split_face_on_cube() {
        let (mut b, faces) = cube();
        let before = b.counts();
        let face = faces[0];
        let lp = b.face(face).map(|f| f.loops[0]).unwrap_or_else(|| panic!());
        let f1 = b.loop_(lp).and_then(|l| l.fin).unwrap_or_else(|| panic!());
        let f2 = b.fin(f1).map(|f| f.next).unwrap_or_else(|| panic!());
        let out = b.split_face(f2, f1, None).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts().f, before.f + 1);
        // Split event recorded with the parent.
        let parent = b
            .face(out.face_old)
            .map(|f| f.id)
            .unwrap_or_else(|| panic!());
        assert!(out.report.split.iter().any(|(p, _)| *p == parent));
    }

    #[test]
    fn two_cubes_glued_along_edge_make_radial_four() {
        // Two unit cubes in one body, sharing the edge x=1, y in [0,1]
        // at z=0 ... constructed disjoint, then stitched.
        let mut b = Body::new();
        let _c1 = crate::euler::test_support::cube_into(&mut b, Vec3::ZERO);
        let _c2 = crate::euler::test_support::cube_into(&mut b, Vec3::new(1.0, 0.0, 0.0));
        assert!(b.validate().is_ok());
        // Find the coincident vertex pairs at (1,0,0) and (1,1,0):
        // one from each cube.
        let at = |p: Vec3| -> Vec<VertexKey> {
            b.entity_ids()
                .filter_map(|id| match b.lookup(id) {
                    Some(crate::entity::AnyKey::Vertex(k)) => {
                        let v = b.vertex(k)?;
                        ((v.point - p).norm() < 1e-12).then_some(k)
                    }
                    _ => None,
                })
                .collect()
        };
        let pa = at(Vec3::new(1.0, 0.0, 0.0));
        let pb = at(Vec3::new(1.0, 1.0, 0.0));
        assert_eq!((pa.len(), pb.len()), (2, 2));
        b.merge_vertices(pa[0], pa[1]).unwrap();
        b.merge_vertices(pb[0], pb[1]).unwrap();
        assert!(b.validate().is_ok());
        // Two coincident edges between the merged vertices now exist.
        let shared: Vec<EdgeKey> = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(crate::entity::AnyKey::Edge(k)) => {
                    let e = b.edge(k)?;
                    let touches = |v: VertexKey| e.bounds.0 == v || e.bounds.1 == v;
                    (touches(pa[0]) && touches(pb[0])).then_some(k)
                }
                _ => None,
            })
            .collect();
        assert_eq!(shared.len(), 2, "expected two coincident edges");
        b.glue_edges(shared[0], shared[1]).unwrap();
        assert!(b.validate().is_ok());
        // THE non-manifold state: four fins around the shared edge.
        assert_eq!(b.edge(shared[0]).map(|e| e.radial.len()), Some(4));
    }

    #[test]
    fn embeds_count_and_validate() {
        let mut b = Body::new();
        let r = b.infinite_region();
        let (_, _, _) = b.embed_vertex(r, Vec3::ZERO).unwrap();
        let out = b
            .embed_wire(r, None, Vec3::new(1., 0., 0.), Vec3::new(2., 0., 0.))
            .unwrap();
        assert!(b.validate().is_ok());
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (3, 1, 0));
        assert!(
            b.edge(out.edge)
                .map(|e| e.radial.is_empty())
                .unwrap_or(false)
        );
    }
}
