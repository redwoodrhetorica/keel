//! SGC MERGE (dossier 57 Rung 5): the Rossignac-O'Connor Merge
//! operator over Keel's cellular results, collapsing a non-regularized
//! body toward the regularized one. (Distinct from `simplify.rs`, the
//! M8 canonical-recovery HEAL pass; this module merges CELLS, that one
//! recovers GEOMETRY.) Rossignac's predicate: combine a k-cell with
//! its two incident (k+1)-cells when both share the same active
//! classification and the k-cell bounds nothing else. The 2-cell
//! instance implemented here: an interior interface WALL whose two
//! sides bound two distinct SOLID 3-cells dissolves; the regions merge
//! through the opened doorway, the two host shells fuse, and every
//! wall rim drops one entry from its radial cycle (radial-3 junctions
//! return to manifold pairs).
//!
//! Scope (the narrow honest slice): single-loop walls whose every rim
//! edge keeps at least one other fin. Walls with dangling rims, ring-
//! carrying walls, and the lower-dimensional Merge instances (edge and
//! vertex collapse, kept wires and lamina) are the mixed-dimension
//! follow-up; they DECLINE rather than guess.

use crate::body::{Body, TopoError};
use crate::entity::{FaceKey, FinKey};

impl Body {
    /// Dissolve ONE interior interface wall (both sides solid,
    /// distinct regions): the Rossignac Merge for a 2-cell between two
    /// same-classification 3-cells. The wall's fins leave their radial
    /// cycles, vertex fin-references re-anchor to surviving fins, the
    /// back region's shells and faces repoint to the front region, the
    /// two host shells fuse (genus adds), and the wall entities plus
    /// the dead region unregister.
    pub fn dissolve_interior_wall(&mut self, wall: FaceKey) -> Result<(), TopoError> {
        let f = self.faces.get(wall).ok_or(TopoError::StaleKey)?;
        let (r_keep, r_kill) = (f.front_region, f.back_region);
        if r_keep == r_kill {
            return Err(TopoError::Precondition("merge: not an interface wall"));
        }
        if self.regions.get(r_keep).map(|x| x.solid) != Some(true)
            || self.regions.get(r_kill).map(|x| x.solid) != Some(true)
        {
            return Err(TopoError::Precondition(
                "merge: wall must bound two solid cells",
            ));
        }
        if f.loops.len() != 1 {
            return Err(TopoError::Precondition(
                "merge: ring-carrying wall (follow-up)",
            ));
        }
        let loops = f.loops.clone();
        // The wall's fins in loop order.
        let mut wall_fins: Vec<FinKey> = Vec::new();
        for &lk in &loops {
            let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                continue;
            };
            let mut cur = entry;
            loop {
                wall_fins.push(cur);
                let next = self.fins.get(cur).ok_or(TopoError::StaleKey)?.next;
                cur = next;
                if cur == entry {
                    break;
                }
            }
        }
        // Every rim edge must keep at least one fin after the wall
        // leaves (a dangling rim is the genuine mixed-dimension case:
        // decline, never guess).
        for &fk in &wall_fins {
            let e = self
                .fins
                .get(fk)
                .map(|x| x.edge)
                .ok_or(TopoError::StaleKey)?;
            let total = self.edges.get(e).map(|x| x.radial.len()).unwrap_or(0);
            let mine = wall_fins
                .iter()
                .filter(|&&g| self.fins.get(g).map(|x| x.edge) == Some(e))
                .count();
            if total <= mine {
                return Err(TopoError::Precondition(
                    "merge: dangling wall rim (follow-up)",
                ));
            }
        }
        let (sf, sb) = self.shells_of_face(wall)?;
        if sf == sb {
            return Err(TopoError::Precondition(
                "merge: wall hosts a single shell (follow-up)",
            ));
        }

        let mut rec = self.begin_op();
        // 1. Radial detach.
        for &fk in &wall_fins {
            let e = self
                .fins
                .get(fk)
                .map(|x| x.edge)
                .ok_or(TopoError::StaleKey)?;
            if let Some(ed) = self.edges.get_mut(e) {
                ed.radial.retain(|&g| g != fk);
            }
        }
        // 2. Vertex fin-reference repair: any vertex anchored on a
        // dying fin re-anchors to a surviving fin of an incident edge.
        let dead: std::collections::BTreeSet<FinKey> = wall_fins.iter().copied().collect();
        let mut rim_vertices: Vec<crate::entity::VertexKey> = Vec::new();
        for &fk in &wall_fins {
            let e = self
                .fins
                .get(fk)
                .map(|x| x.edge)
                .ok_or(TopoError::StaleKey)?;
            let (v0, v1) = self
                .edges
                .get(e)
                .map(|x| x.bounds)
                .ok_or(TopoError::StaleKey)?;
            for v in [v0, v1] {
                if !rim_vertices.contains(&v) {
                    rim_vertices.push(v);
                }
            }
        }
        for &v in &rim_vertices {
            let needs = self
                .vertices
                .get(v)
                .and_then(|x| x.fin)
                .map(|fk| dead.contains(&fk))
                .unwrap_or(false);
            if needs {
                let mut replacement = None;
                'scan: for (_, ed) in self.edges.iter() {
                    if ed.bounds.0 == v || ed.bounds.1 == v {
                        for &g in &ed.radial {
                            if !dead.contains(&g) {
                                replacement = Some(g);
                                break 'scan;
                            }
                        }
                    }
                }
                if let Some(x) = self.vertices.get_mut(v) {
                    x.fin = replacement;
                }
            }
            if let Some(x) = self.vertices.get_mut(v) {
                x.groups.retain(|g| !dead.contains(g));
            }
        }
        // 3. Remove the wall from both shells, then fuse the back
        // shell into the front one (the merged region's boundary is
        // one connected shell once the doorway opens; genus adds).
        for sk in [sf, sb] {
            if let Some(s) = self.shells.get_mut(sk) {
                s.faces.retain(|&(fk, _)| fk != wall);
            }
        }
        let (moved_faces, moved_wires, moved_acorn, moved_genus) = {
            let s = self.shells.get_mut(sb).ok_or(TopoError::StaleKey)?;
            (
                std::mem::take(&mut s.faces),
                std::mem::take(&mut s.wires),
                s.acorn.take(),
                s.genus,
            )
        };
        if let Some(s) = self.shells.get_mut(sf) {
            s.faces.extend(moved_faces);
            s.wires.extend(moved_wires);
            if s.acorn.is_none() {
                s.acorn = moved_acorn;
            }
            s.genus += moved_genus;
        }
        // 4. Region merge: r_kill's remaining shells move to r_keep;
        // every face pointing at r_kill repoints.
        let kill_shells = self
            .regions
            .get(r_kill)
            .map(|r| r.shells.clone())
            .unwrap_or_default();
        for sk in kill_shells {
            if sk == sb {
                continue;
            }
            if let Some(s) = self.shells.get_mut(sk) {
                s.region = r_keep;
            }
            if let Some(r) = self.regions.get_mut(r_keep) {
                r.shells.push(sk);
            }
        }
        for fk in self.face_keys() {
            if let Some(fc) = self.faces.get_mut(fk) {
                if fc.front_region == r_kill {
                    fc.front_region = r_keep;
                }
                if fc.back_region == r_kill {
                    fc.back_region = r_keep;
                }
            }
        }
        // 5. Unregister and remove the wall complex, the fused-away
        // shell, and the dead region.
        for &fk in &wall_fins {
            if let Some(id) = self.fins.get(fk).map(|x| x.id) {
                self.unregister(&mut rec, id);
            }
            self.fins.remove(fk);
        }
        for &lk in &loops {
            if let Some(id) = self.loops.get(lk).map(|x| x.id) {
                self.unregister(&mut rec, id);
            }
            self.loops.remove(lk);
        }
        if let Some(id) = self.faces.get(wall).map(|x| x.id) {
            self.unregister(&mut rec, id);
        }
        self.faces.remove(wall);
        if let Some(id) = self.shells.get(sb).map(|x| x.id) {
            self.unregister(&mut rec, id);
        }
        self.shells.remove(sb);
        if let Some(id) = self.regions.get(r_kill).map(|x| x.id) {
            self.unregister(&mut rec, id);
        }
        self.regions.remove(r_kill);
        let _ = rec.finish();
        self.debug_validate();
        Ok(())
    }

    /// SGC Merge over the whole body: dissolve every interior
    /// interface wall (the non-regularized to regularized collapse).
    /// Returns how many walls dissolved; stops with an error if a
    /// wall declines, so the caller sees the genuine mixed-dimension
    /// case rather than a partial silent collapse.
    pub fn merge_cells(&mut self) -> Result<usize, TopoError> {
        let mut count = 0usize;
        loop {
            let next = self.face_keys().into_iter().find(|&f| {
                self.is_interior_wall(f)
                    && self
                        .faces
                        .get(f)
                        .map(|x| x.front_region != x.back_region)
                        .unwrap_or(false)
            });
            let Some(wall) = next else { break };
            self.dissolve_interior_wall(wall)?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use crate::body::Body;
    use crate::boolean::{BoolOp, BooleanOptions, boolean, boolean_with, partition_by_sheet};
    use keel_math::vec::Vec3;

    #[test]
    fn merge_collapses_the_cellular_union_to_the_regularized_one() {
        // The Rung-5 differential oracle: non-regularized union +
        // Merge == regularized union, exactly. Two abutting unit
        // cubes: the cellular result carries one interface wall and
        // two solid cells; Merge dissolves the wall, fuses the cells,
        // returns every radial-3 rim to a manifold pair, and the
        // result matches the regularized union in regions, faces,
        // mass, and mesh at 1e-9.
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let mut cel = boolean_with(
            &a,
            &b,
            BoolOp::Union,
            1e-7,
            BooleanOptions { regularize: false },
        )
        .unwrap()
        .body;
        let dissolved = cel.merge_cells().unwrap();
        assert_eq!(dissolved, 1, "one wall dissolves");
        assert!(cel.validate().is_ok(), "merged body invalid");
        let reg = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap().body;
        let solids = |x: &Body| x.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(solids(&cel), 1, "one solid cell after Merge");
        assert_eq!(solids(&cel), solids(&reg));
        assert_eq!(cel.face_keys().len(), reg.face_keys().len(), "face count");
        let walls = cel
            .face_keys()
            .iter()
            .filter(|&&f| cel.is_interior_wall(f))
            .count();
        assert_eq!(walls, 0, "no interface walls remain");
        let radial3 = cel
            .edges
            .iter()
            .filter(|(_, e)| e.radial.len() >= 3)
            .count();
        assert_eq!(radial3, 0, "all rims back to manifold pairs");
        let (vc, mc) = (cel.mass_properties().unwrap().volume, cel.mesh_volume());
        let vr = reg.mass_properties().unwrap().volume;
        assert!((vc - 2.0).abs() < 1e-9, "mass {vc}");
        assert!((mc - 2.0).abs() < 1e-9, "mesh {mc}");
        assert!((vc - vr).abs() < 1e-12, "differential vs regularized");
        // Idempotence.
        assert_eq!(cel.merge_cells().unwrap(), 0);
    }

    #[test]
    fn merge_undoes_the_sheet_knife_partition() {
        // partition_by_sheet splits the cube into two cells along an
        // interior wall; Merge must give back a single-cell solid of
        // the cube's exact volume (the imprint scars on the outer
        // faces legitimately remain as real edges).
        let mut cube = Body::new();
        cube.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let sheet = Body::rectangular_sheet(
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            4.0,
            4.0,
        )
        .unwrap();
        let mut cel = partition_by_sheet(&cube, &sheet, 1e-7).unwrap();
        let pre_walls = cel
            .face_keys()
            .iter()
            .filter(|&&f| cel.is_interior_wall(f))
            .count();
        assert_eq!(pre_walls, 1, "the knife wall");
        let dissolved = cel.merge_cells().unwrap();
        assert_eq!(dissolved, 1);
        assert!(cel.validate().is_ok(), "unpartitioned body invalid");
        let solids = cel.regions.iter().filter(|(_, r)| r.solid).count();
        assert_eq!(solids, 1, "one solid cell");
        let v = cel.mass_properties().unwrap().volume;
        assert!((v - 8.0).abs() < 1e-9, "mass {v}");
        let m = cel.mesh_volume();
        assert!((m - 8.0).abs() < 1e-9, "mesh {m}");
    }
}
