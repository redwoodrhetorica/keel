//! The manifold Euler operator core (gate design 2.1; Mantyla/GWB).
//! All topology mutation flows through these and the ops module; by
//! Mantyla's completeness result every higher operation is a sequence
//! of them. Each operator is transactional: on Err the body is
//! unchanged. Debug builds validate after every operator.

use crate::body::{Body, TopoError};
use crate::entity::{
    EdgeKey, FaceKey, FinKey, LoopKey, LoopKind, RegionKey, ShellKey, Side, SurfaceKey, VertexKey,
};
use crate::lineage::{Derivation, OpReport};
use keel_math::vec::Vec3;

pub struct MvfsOut {
    pub vertex: VertexKey,
    pub face: FaceKey,
    pub interior: RegionKey,
    pub shell_outer: ShellKey,
    pub shell_inner: ShellKey,
    pub report: OpReport,
}

pub struct MevOut {
    pub edge: EdgeKey,
    pub vertex: VertexKey,
    pub report: OpReport,
}

pub struct MefOut {
    pub edge: EdgeKey,
    pub face: FaceKey,
    pub report: OpReport,
}

pub struct MfkrhOut {
    pub face: FaceKey,
    pub report: OpReport,
}

/// Where MEV attaches its new spur.
#[derive(Clone, Copy, Debug)]
pub enum MevSite {
    /// Upgrade a vertex loop: new edge from the loop's vertex to the
    /// new vertex; the loop becomes a two-fin ring (out and back).
    VertexLoop(LoopKey),
    /// Insert a spur after `fin`: new edge from end-vertex(fin) to the
    /// new vertex; loop order becomes ... fin, out, back, fin.next ...
    AfterFin(FinKey),
}

impl Body {
    /// Seed a minimal closed body in `region` (gate 2.1): vertex v,
    /// face f whose single loop is the vertex loop at v, one shell per
    /// side, and a new interior region with solidity !region.solid.
    /// Front of f faces `region` (outward); Back faces the interior.
    pub fn mvfs(&mut self, region: RegionKey, p: Vec3) -> Result<MvfsOut, TopoError> {
        if !self.regions.contains(region) {
            return Err(TopoError::StaleKey);
        }
        let parent_solid = self.regions.get(region).map(|r| r.solid).unwrap_or(false);
        let mut rec = self.begin_op();
        let v = self.new_vertex(&mut rec, p);
        let interior = self.new_region(&mut rec, !parent_solid, Derivation::Created);
        let shell_outer = self.new_shell(&mut rec, region, Derivation::Created);
        let shell_inner = self.new_shell(&mut rec, interior, Derivation::Created);
        let face = self.new_face(&mut rec, region, interior, Derivation::Created);
        let lp = self.new_loop(&mut rec, face, LoopKind::Outer, Derivation::Created);
        if let Some(l) = self.loops.get_mut(lp) {
            l.vertex = Some(v);
        }
        if let Some(f) = self.faces.get_mut(face) {
            f.loops.push(lp);
        }
        if let Some(s) = self.shells.get_mut(shell_outer) {
            s.faces.push((face, Side::Front));
        }
        if let Some(s) = self.shells.get_mut(shell_inner) {
            s.faces.push((face, Side::Back));
        }
        if let Some(r) = self.regions.get_mut(region) {
            r.shells.push(shell_outer);
        }
        if let Some(r) = self.regions.get_mut(interior) {
            r.shells.push(shell_inner);
        }
        let report = rec.finish();
        self.debug_validate();
        Ok(MvfsOut {
            vertex: v,
            face,
            interior,
            shell_outer,
            shell_inner,
            report,
        })
    }

    /// Destroy a minimal body: `face` must have exactly one loop, a
    /// vertex loop, and both its shells must contain only this face.
    /// Merges the interior region away. Strict inverse of mvfs.
    pub fn kvfs(&mut self, face: FaceKey) -> Result<OpReport, TopoError> {
        let f = self.faces.get(face).ok_or(TopoError::StaleKey)?;
        if f.loops.len() != 1 {
            return Err(TopoError::Precondition("kvfs: face must have one loop"));
        }
        let lp = f.loops[0];
        let l = self.loops.get(lp).ok_or(TopoError::StaleKey)?;
        let Some(v) = l.vertex else {
            return Err(TopoError::Precondition("kvfs: loop must be a vertex loop"));
        };
        let (front_region, back_region) = (f.front_region, f.back_region);
        let interior = back_region; // mvfs convention
        let r = self.regions.get(interior).ok_or(TopoError::StaleKey)?;
        if r.infinite {
            return Err(TopoError::Precondition("kvfs: interior cannot be infinite"));
        }
        if r.shells.len() != 1 {
            return Err(TopoError::Precondition(
                "kvfs: interior must have one shell",
            ));
        }
        let shell_inner = r.shells[0];
        // Find the outer shell: the one in front_region holding (face, Front).
        let outer_candidates: Vec<ShellKey> = self
            .regions
            .get(front_region)
            .ok_or(TopoError::StaleKey)?
            .shells
            .iter()
            .copied()
            .filter(|&sk| {
                self.shells
                    .get(sk)
                    .is_some_and(|s| s.faces.contains(&(face, Side::Front)))
            })
            .collect();
        let [shell_outer] = outer_candidates[..] else {
            return Err(TopoError::Precondition("kvfs: outer shell not unique"));
        };
        for sk in [shell_outer, shell_inner] {
            let s = self.shells.get(sk).ok_or(TopoError::StaleKey)?;
            if s.faces.len() != 1 || !s.wires.is_empty() || s.acorn.is_some() || s.genus != 0 {
                return Err(TopoError::Precondition(
                    "kvfs: shells must hold only this face",
                ));
            }
        }
        if self
            .vertices
            .get(v)
            .is_some_and(|vv| vv.fin.is_some() || !vv.groups.is_empty())
        {
            return Err(TopoError::Precondition("kvfs: vertex must be isolated"));
        }

        // Commit point: all preconditions hold; mutations are infallible.
        let mut rec = self.begin_op();
        for id in [
            self.loops.get(lp).map(|x| x.id),
            self.faces.get(face).map(|x| x.id),
            self.shells.get(shell_outer).map(|x| x.id),
            self.shells.get(shell_inner).map(|x| x.id),
            self.vertices.get(v).map(|x| x.id),
            self.regions.get(interior).map(|x| x.id),
        ]
        .into_iter()
        .flatten()
        {
            self.unregister(&mut rec, id);
        }
        self.loops.remove(lp);
        self.faces.remove(face);
        self.shells.remove(shell_outer);
        self.shells.remove(shell_inner);
        self.vertices.remove(v);
        self.regions.remove(interior);
        if let Some(r) = self.regions.get_mut(front_region) {
            r.shells.retain(|&s| s != shell_outer);
        }
        let report = rec.finish();
        self.debug_validate();
        Ok(report)
    }

    /// Make edge and vertex (gate 2.1). See [`MevSite`].
    pub fn mev(&mut self, site: MevSite, p: Vec3) -> Result<MevOut, TopoError> {
        match site {
            MevSite::VertexLoop(lp) => {
                let l = self.loops.get(lp).ok_or(TopoError::StaleKey)?;
                let Some(v) = l.vertex else {
                    return Err(TopoError::Precondition("mev: not a vertex loop"));
                };
                let mut rec = self.begin_op();
                let w = self.new_vertex(&mut rec, p);
                let e = self.new_edge(&mut rec, (v, w), Derivation::Created);
                let a = self.new_fin(&mut rec, e, true, lp, Derivation::Created);
                let b = self.new_fin(&mut rec, e, false, lp, Derivation::Created);
                // Two-fin ring: out (v->w) then back (w->v).
                if let Some(fa) = self.fins.get_mut(a) {
                    fa.next = b;
                    fa.prev = b;
                }
                if let Some(fb) = self.fins.get_mut(b) {
                    fb.next = a;
                    fb.prev = a;
                }
                if let Some(edge) = self.edges.get_mut(e) {
                    edge.radial = vec![a, b];
                }
                if let Some(l) = self.loops.get_mut(lp) {
                    l.fin = Some(a);
                    l.vertex = None;
                }
                if let Some(vv) = self.vertices.get_mut(v) {
                    vv.fin = Some(a);
                }
                if let Some(ww) = self.vertices.get_mut(w) {
                    ww.fin = Some(b);
                }
                let report = rec.finish();
                self.debug_validate();
                Ok(MevOut {
                    edge: e,
                    vertex: w,
                    report,
                })
            }
            MevSite::AfterFin(fk) => {
                let f = self.fins.get(fk).ok_or(TopoError::StaleKey)?;
                let lp = f.owner;
                let old_next = f.next;
                let v = self
                    .fin_end_vertex(fk)
                    .ok_or(TopoError::Precondition("mev: fin has no end vertex"))?;
                let mut rec = self.begin_op();
                let w = self.new_vertex(&mut rec, p);
                let e = self.new_edge(&mut rec, (v, w), Derivation::Created);
                let a = self.new_fin(&mut rec, e, true, lp, Derivation::Created);
                let b = self.new_fin(&mut rec, e, false, lp, Derivation::Created);
                // Splice: fk -> a -> b -> old_next.
                if let Some(x) = self.fins.get_mut(fk) {
                    x.next = a;
                }
                if let Some(x) = self.fins.get_mut(a) {
                    x.prev = fk;
                    x.next = b;
                }
                if let Some(x) = self.fins.get_mut(b) {
                    x.prev = a;
                    x.next = old_next;
                }
                if let Some(x) = self.fins.get_mut(old_next) {
                    x.prev = b;
                }
                if let Some(edge) = self.edges.get_mut(e) {
                    edge.radial = vec![a, b];
                }
                if let Some(ww) = self.vertices.get_mut(w) {
                    ww.fin = Some(b);
                }
                let report = rec.finish();
                self.debug_validate();
                Ok(MevOut {
                    edge: e,
                    vertex: w,
                    report,
                })
            }
        }
    }

    /// Kill a spur edge and its far vertex. The far vertex must be
    /// incident to this edge only. Strict inverse of mev.
    pub fn kev(&mut self, edge: EdgeKey) -> Result<OpReport, TopoError> {
        let e = self.edges.get(edge).ok_or(TopoError::StaleKey)?;
        if e.is_closed() {
            return Err(TopoError::Precondition("kev: closed edge is not a spur"));
        }
        let [a, b] = e.radial[..] else {
            return Err(TopoError::Precondition("kev: edge must have two fins"));
        };
        let (fa, fb) = match (self.fins.get(a), self.fins.get(b)) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err(TopoError::StaleKey),
        };
        if fa.owner != fb.owner {
            return Err(TopoError::Precondition("kev: fins must share a loop"));
        }
        let lp = fa.owner;
        // Identify spur orientation: out then back adjacent in the loop.
        let (out, back) = if fa.next == b {
            (a, b)
        } else if fb.next == a {
            (b, a)
        } else {
            return Err(TopoError::Precondition(
                "kev: fins not adjacent (not a spur)",
            ));
        };
        let far = self
            .fin_end_vertex(out)
            .ok_or(TopoError::Precondition("kev: no far vertex"))?;
        let near = self
            .fin_start_vertex(out)
            .ok_or(TopoError::Precondition("kev: no near vertex"))?;
        // Far vertex must be incident to this edge only.
        let far_incident = self
            .edges
            .iter()
            .filter(|(ek, ee)| *ek != edge && (ee.bounds.0 == far || ee.bounds.1 == far))
            .count();
        if far_incident != 0 {
            return Err(TopoError::Precondition("kev: far vertex not isolated"));
        }

        let mut rec = self.begin_op();
        let out_prev = self
            .fins
            .get(out)
            .map(|x| x.prev)
            .ok_or(TopoError::StaleKey)?;
        let back_next = self
            .fins
            .get(back)
            .map(|x| x.next)
            .ok_or(TopoError::StaleKey)?;
        let loop_shrinks_to_vertex = out_prev == back && back_next == out;
        if loop_shrinks_to_vertex {
            if let Some(l) = self.loops.get_mut(lp) {
                l.fin = None;
                l.vertex = Some(near);
            }
            if let Some(vv) = self.vertices.get_mut(near) {
                vv.fin = None;
            }
        } else {
            if let Some(x) = self.fins.get_mut(out_prev) {
                x.next = back_next;
            }
            if let Some(x) = self.fins.get_mut(back_next) {
                x.prev = out_prev;
            }
            if let Some(l) = self.loops.get_mut(lp)
                && (l.fin == Some(out) || l.fin == Some(back))
            {
                l.fin = Some(out_prev);
            }
            if let Some(vv) = self.vertices.get_mut(near)
                && (vv.fin == Some(out) || vv.fin == Some(back))
            {
                vv.fin = Some(out_prev);
            }
        }
        for id in [
            self.fins.get(out).map(|x| x.id),
            self.fins.get(back).map(|x| x.id),
            self.edges.get(edge).map(|x| x.id),
            self.vertices.get(far).map(|x| x.id),
        ]
        .into_iter()
        .flatten()
        {
            self.unregister(&mut rec, id);
        }
        self.fins.remove(out);
        self.fins.remove(back);
        self.edges.remove(edge);
        self.vertices.remove(far);
        let report = rec.finish();
        self.debug_validate();
        Ok(report)
    }

    /// Make edge and face (gate 2.1): split a loop with a new edge
    /// from end-vertex(fin_a) to end-vertex(fin_b), both fins in the
    /// SAME loop. Fins fin_a.next ..= fin_b move to the NEW face's
    /// loop. fin_a == fin_b makes a CLOSED self-loop edge seamed at
    /// end-vertex(fin_a), the new face's loop being the single new
    /// fin. The new face joins the same two shells as the old face,
    /// side-matched; regions unchanged.
    pub fn mef(
        &mut self,
        fin_a: FinKey,
        fin_b: FinKey,
        surface: Option<(SurfaceKey, bool)>,
    ) -> Result<MefOut, TopoError> {
        let fa = self.fins.get(fin_a).ok_or(TopoError::StaleKey)?;
        let fb = self.fins.get(fin_b).ok_or(TopoError::StaleKey)?;
        if fa.owner != fb.owner {
            return Err(TopoError::Precondition("mef: fins must share a loop"));
        }
        let lp = fa.owner;
        let old_face = self
            .loops
            .get(lp)
            .map(|l| l.face)
            .ok_or(TopoError::StaleKey)?;
        let va = self
            .fin_end_vertex(fin_a)
            .ok_or(TopoError::Precondition("mef: fin_a end vertex"))?;
        let vb = self
            .fin_end_vertex(fin_b)
            .ok_or(TopoError::Precondition("mef: fin_b end vertex"))?;
        let (front_region, back_region) = {
            let f = self.faces.get(old_face).ok_or(TopoError::StaleKey)?;
            (f.front_region, f.back_region)
        };
        let (shell_front, shell_back) = self.shells_of_face(old_face)?;

        let mut rec = self.begin_op();
        let old_face_id = self
            .faces
            .get(old_face)
            .map(|x| x.id)
            .unwrap_or_default_id();
        let new_face = self.new_face(
            &mut rec,
            front_region,
            back_region,
            Derivation::Generated { from: old_face_id },
        );
        let new_loop = self.new_loop(&mut rec, new_face, LoopKind::Outer, Derivation::Created);
        if let Some(f) = self.faces.get_mut(new_face) {
            f.loops.push(new_loop);
            f.surface = surface;
        }
        let e = self.new_edge(&mut rec, (va, vb), Derivation::Created);
        // Chain continuity: in the OLD loop the closer follows fin_a
        // (which ends at va), so g runs va -> vb (forward). In the NEW
        // loop the closer follows fin_b (ends at vb), so h runs
        // vb -> va (backward).
        let g = self.new_fin(&mut rec, e, true, lp, Derivation::Created);
        let h = self.new_fin(&mut rec, e, false, new_loop, Derivation::Created);
        if let Some(edge) = self.edges.get_mut(e) {
            edge.radial = vec![g, h];
        }

        if fin_a == fin_b {
            // Closed self-loop edge at va (== vb). Old loop gains g
            // after fin_a; new loop is just h.
            let old_next = self
                .fins
                .get(fin_a)
                .map(|x| x.next)
                .ok_or(TopoError::StaleKey)?;
            if let Some(x) = self.fins.get_mut(fin_a) {
                x.next = g;
            }
            if let Some(x) = self.fins.get_mut(g) {
                x.prev = fin_a;
                x.next = old_next;
            }
            if let Some(x) = self.fins.get_mut(old_next) {
                x.prev = g;
            }
            if let Some(x) = self.fins.get_mut(h) {
                x.next = h;
                x.prev = h;
            }
            if let Some(l) = self.loops.get_mut(new_loop) {
                l.fin = Some(h);
            }
        } else {
            // General split. Move fins fin_a.next ..= fin_b to new_loop.
            let a_next = self
                .fins
                .get(fin_a)
                .map(|x| x.next)
                .ok_or(TopoError::StaleKey)?;
            let b_next = self
                .fins
                .get(fin_b)
                .map(|x| x.next)
                .ok_or(TopoError::StaleKey)?;
            // Old loop ring: fin_a -> g -> b_next ...
            if let Some(x) = self.fins.get_mut(fin_a) {
                x.next = g;
            }
            if let Some(x) = self.fins.get_mut(g) {
                x.prev = fin_a;
                x.next = b_next;
            }
            if let Some(x) = self.fins.get_mut(b_next) {
                x.prev = g;
            }
            // New loop ring: fin_b -> h -> a_next ...
            if let Some(x) = self.fins.get_mut(fin_b) {
                x.next = h;
            }
            if let Some(x) = self.fins.get_mut(h) {
                x.prev = fin_b;
                x.next = a_next;
            }
            if let Some(x) = self.fins.get_mut(a_next) {
                x.prev = h;
            }
            if let Some(l) = self.loops.get_mut(new_loop) {
                l.fin = Some(h);
            }
            // Re-own the moved fins (walk the new ring).
            let mut cur = h;
            loop {
                let next = match self.fins.get_mut(cur) {
                    Some(x) => {
                        x.owner = new_loop;
                        x.next
                    }
                    None => return Err(TopoError::StaleKey),
                };
                cur = next;
                if cur == h {
                    break;
                }
            }
            // Keep the old loop's entry valid (it may have moved).
            if let Some(l) = self.loops.get_mut(lp) {
                l.fin = Some(g);
            }
        }

        // The new face joins the same shells, side-matched.
        if let Some(s) = self.shells.get_mut(shell_front) {
            s.faces.push((new_face, Side::Front));
        }
        if let Some(s) = self.shells.get_mut(shell_back) {
            s.faces.push((new_face, Side::Back));
        }
        let report = rec.finish();
        self.debug_validate();
        Ok(MefOut {
            edge: e,
            face: new_face,
            report,
        })
    }

    /// MEF from a vertex loop: closed self-loop edge seamed at the
    /// loop's vertex; the vertex loop upgrades to a one-fin ring and
    /// the new face's loop is the other fin.
    pub fn mef_on_vertex_loop(
        &mut self,
        lp: LoopKey,
        surface: Option<(SurfaceKey, bool)>,
    ) -> Result<MefOut, TopoError> {
        let l = self.loops.get(lp).ok_or(TopoError::StaleKey)?;
        let Some(v) = l.vertex else {
            return Err(TopoError::Precondition(
                "mef_on_vertex_loop: not a vertex loop",
            ));
        };
        let old_face = l.face;
        let (front_region, back_region) = {
            let f = self.faces.get(old_face).ok_or(TopoError::StaleKey)?;
            (f.front_region, f.back_region)
        };
        let (shell_front, shell_back) = self.shells_of_face(old_face)?;

        let mut rec = self.begin_op();
        let old_face_id = self
            .faces
            .get(old_face)
            .map(|x| x.id)
            .unwrap_or_default_id();
        let new_face = self.new_face(
            &mut rec,
            front_region,
            back_region,
            Derivation::Generated { from: old_face_id },
        );
        let new_loop = self.new_loop(&mut rec, new_face, LoopKind::Outer, Derivation::Created);
        if let Some(f) = self.faces.get_mut(new_face) {
            f.loops.push(new_loop);
            f.surface = surface;
        }
        let e = self.new_edge(&mut rec, (v, v), Derivation::Created);
        let g = self.new_fin(&mut rec, e, true, lp, Derivation::Created);
        let h = self.new_fin(&mut rec, e, false, new_loop, Derivation::Created);
        if let Some(edge) = self.edges.get_mut(e) {
            edge.radial = vec![g, h];
        }
        // Both rings are single-fin self-loops.
        for (fin, owner) in [(g, lp), (h, new_loop)] {
            if let Some(x) = self.fins.get_mut(fin) {
                x.next = fin;
                x.prev = fin;
                x.owner = owner;
            }
        }
        if let Some(l) = self.loops.get_mut(lp) {
            l.fin = Some(g);
            l.vertex = None;
        }
        if let Some(l) = self.loops.get_mut(new_loop) {
            l.fin = Some(h);
        }
        if let Some(vv) = self.vertices.get_mut(v) {
            vv.fin = Some(g);
        }
        if let Some(s) = self.shells.get_mut(shell_front) {
            s.faces.push((new_face, Side::Front));
        }
        if let Some(s) = self.shells.get_mut(shell_back) {
            s.faces.push((new_face, Side::Back));
        }
        let report = rec.finish();
        self.debug_validate();
        Ok(MefOut {
            edge: e,
            face: new_face,
            report,
        })
    }

    /// Kill edge, kill face: inverse of mef. The edge's two fins lie
    /// in the outer loops of two DIFFERENT faces sharing both shells;
    /// the loops merge into the loop of the SURVIVING face (the fin
    /// `g`'s owner) and the other face dies.
    pub fn kef(&mut self, edge: EdgeKey) -> Result<OpReport, TopoError> {
        let e = self.edges.get(edge).ok_or(TopoError::StaleKey)?;
        let [g, h] = e.radial[..] else {
            return Err(TopoError::Precondition("kef: edge must have two fins"));
        };
        let (fg, fh) = match (self.fins.get(g), self.fins.get(h)) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err(TopoError::StaleKey),
        };
        let (lp_keep, lp_kill) = (fg.owner, fh.owner);
        if lp_keep == lp_kill {
            return Err(TopoError::Precondition("kef: fins share a loop (use kemr)"));
        }
        let face_keep = self
            .loops
            .get(lp_keep)
            .map(|l| l.face)
            .ok_or(TopoError::StaleKey)?;
        let face_kill = self
            .loops
            .get(lp_kill)
            .map(|l| l.face)
            .ok_or(TopoError::StaleKey)?;
        if face_keep == face_kill {
            return Err(TopoError::Precondition(
                "kef: loops must be on distinct faces",
            ));
        }
        if self.faces.get(face_kill).map(|f| f.loops.len()) != Some(1) {
            return Err(TopoError::Precondition(
                "kef: dying face must have one loop",
            ));
        }
        let kill_is_outer = self.loops.get(lp_kill).map(|l| l.kind) == Some(LoopKind::Outer);
        if !kill_is_outer {
            return Err(TopoError::Precondition("kef: dying loop must be outer"));
        }
        let (sf_keep, sb_keep) = self.shells_of_face(face_keep)?;
        let (sf_kill, sb_kill) = self.shells_of_face(face_kill)?;
        if (sf_keep, sb_keep) != (sf_kill, sb_kill) {
            return Err(TopoError::Precondition("kef: faces must share both shells"));
        }

        let mut rec = self.begin_op();
        let g_prev = self
            .fins
            .get(g)
            .map(|x| x.prev)
            .ok_or(TopoError::StaleKey)?;
        let g_next = self
            .fins
            .get(g)
            .map(|x| x.next)
            .ok_or(TopoError::StaleKey)?;
        let h_prev = self
            .fins
            .get(h)
            .map(|x| x.prev)
            .ok_or(TopoError::StaleKey)?;
        let h_next = self
            .fins
            .get(h)
            .map(|x| x.next)
            .ok_or(TopoError::StaleKey)?;

        let keep_id = self
            .faces
            .get(face_keep)
            .map(|x| x.id)
            .unwrap_or_default_id();
        let _ = keep_id;
        // Merge rings: splice the kill loop's fins (minus h) into the
        // keep loop where g sat. Cases: single-fin rings degenerate.
        let keep_becomes_vertex_loop = g_prev == g && h_prev == h;
        if keep_becomes_vertex_loop {
            // Both loops were single-fin self-loops over this closed
            // edge: the merged result is a vertex loop at the seam.
            let seam = self
                .fin_start_vertex(g)
                .ok_or(TopoError::Precondition("kef: seam vertex"))?;
            if let Some(l) = self.loops.get_mut(lp_keep) {
                l.fin = None;
                l.vertex = Some(seam);
            }
            if let Some(vv) = self.vertices.get_mut(seam)
                && (vv.fin == Some(g) || vv.fin == Some(h))
            {
                vv.fin = None;
            }
        } else if g_prev == g {
            // Keep ring was just g: result is the kill ring minus h.
            if let Some(x) = self.fins.get_mut(h_prev) {
                x.next = h_next;
            }
            if let Some(x) = self.fins.get_mut(h_next) {
                x.prev = h_prev;
            }
            if let Some(l) = self.loops.get_mut(lp_keep) {
                l.fin = Some(h_prev);
            }
        } else if h_prev == h {
            // Kill ring was just h: drop g from the keep ring.
            if let Some(x) = self.fins.get_mut(g_prev) {
                x.next = g_next;
            }
            if let Some(x) = self.fins.get_mut(g_next) {
                x.prev = g_prev;
            }
            if let Some(l) = self.loops.get_mut(lp_keep)
                && l.fin == Some(g)
            {
                l.fin = Some(g_prev);
            }
        } else {
            // General: ... g_prev, [h_next ... h_prev], g_next ...
            if let Some(x) = self.fins.get_mut(g_prev) {
                x.next = h_next;
            }
            if let Some(x) = self.fins.get_mut(h_next) {
                x.prev = g_prev;
            }
            if let Some(x) = self.fins.get_mut(h_prev) {
                x.next = g_next;
            }
            if let Some(x) = self.fins.get_mut(g_next) {
                x.prev = h_prev;
            }
            if let Some(l) = self.loops.get_mut(lp_keep)
                && l.fin == Some(g)
            {
                l.fin = Some(g_prev);
            }
        }
        // Re-own all fins of the merged ring.
        if let Some(l) = self.loops.get(lp_keep)
            && let Some(entry) = l.fin
        {
            let mut cur = entry;
            loop {
                let next = match self.fins.get_mut(cur) {
                    Some(x) => {
                        x.owner = lp_keep;
                        x.next
                    }
                    None => return Err(TopoError::StaleKey),
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
        }
        // Fix vertex fin references that pointed at g or h.
        self.repoint_vertex_fins(&[g, h], lp_keep);
        // Remove the dying face from its shells.
        for sk in [sf_keep, sb_keep] {
            if let Some(s) = self.shells.get_mut(sk) {
                s.faces.retain(|&(fk, _)| fk != face_kill);
            }
        }
        for id in [
            self.fins.get(g).map(|x| x.id),
            self.fins.get(h).map(|x| x.id),
            self.edges.get(edge).map(|x| x.id),
            self.loops.get(lp_kill).map(|x| x.id),
            self.faces.get(face_kill).map(|x| x.id),
        ]
        .into_iter()
        .flatten()
        {
            self.unregister(&mut rec, id);
        }
        self.fins.remove(g);
        self.fins.remove(h);
        self.edges.remove(edge);
        self.loops.remove(lp_kill);
        self.faces.remove(face_kill);
        let report = rec.finish();
        self.debug_validate();
        Ok(report)
    }

    /// Kill edge, make ring (gate 2.1): the edge's two fins lie in the
    /// SAME loop (a bridge). Removing it splits the loop; the piece
    /// that does not contain the loop entry becomes an INNER ring of
    /// the same face. `fin` selects the bridge edge by either fin.
    pub fn kemr(&mut self, fin: FinKey) -> Result<OpReport, TopoError> {
        let f = self.fins.get(fin).ok_or(TopoError::StaleKey)?;
        let edge = f.edge;
        let lp = f.owner;
        let e = self.edges.get(edge).ok_or(TopoError::StaleKey)?;
        let [a, b] = e.radial[..] else {
            return Err(TopoError::Precondition("kemr: edge must have two fins"));
        };
        let same_loop = self.fins.get(a).map(|x| x.owner) == Some(lp)
            && self.fins.get(b).map(|x| x.owner) == Some(lp);
        if !same_loop {
            return Err(TopoError::Precondition(
                "kemr: fins must share a loop (use kef)",
            ));
        }
        if self.fins.get(a).map(|x| x.next) == Some(b)
            || self.fins.get(b).map(|x| x.next) == Some(a)
        {
            return Err(TopoError::Precondition("kemr: spur edge (use kev)"));
        }
        let face = self
            .loops
            .get(lp)
            .map(|l| l.face)
            .ok_or(TopoError::StaleKey)?;

        let mut rec = self.begin_op();
        let a_prev = self
            .fins
            .get(a)
            .map(|x| x.prev)
            .ok_or(TopoError::StaleKey)?;
        let a_next = self
            .fins
            .get(a)
            .map(|x| x.next)
            .ok_or(TopoError::StaleKey)?;
        let b_prev = self
            .fins
            .get(b)
            .map(|x| x.prev)
            .ok_or(TopoError::StaleKey)?;
        let b_next = self
            .fins
            .get(b)
            .map(|x| x.next)
            .ok_or(TopoError::StaleKey)?;
        // Splice out both fins: ring 1 = a_prev -> b_next ...; ring 2 =
        // b_prev -> a_next ...
        if let Some(x) = self.fins.get_mut(a_prev) {
            x.next = b_next;
        }
        if let Some(x) = self.fins.get_mut(b_next) {
            x.prev = a_prev;
        }
        if let Some(x) = self.fins.get_mut(b_prev) {
            x.next = a_next;
        }
        if let Some(x) = self.fins.get_mut(a_next) {
            x.prev = b_prev;
        }
        let lp_id = self.loops.get(lp).map(|x| x.id).unwrap_or_default_id();
        let ring = self.new_loop(
            &mut rec,
            face,
            LoopKind::Inner,
            Derivation::Generated { from: lp_id },
        );
        if let Some(fc) = self.faces.get_mut(face) {
            fc.loops.push(ring);
        }
        // The piece containing a_prev keeps the old loop; the other
        // piece becomes the ring.
        if let Some(l) = self.loops.get_mut(lp) {
            l.fin = Some(a_prev);
        }
        if let Some(l) = self.loops.get_mut(ring) {
            l.fin = Some(b_prev);
        }
        // Re-own both rings.
        for (entry, owner) in [(a_prev, lp), (b_prev, ring)] {
            let mut cur = entry;
            loop {
                let next = match self.fins.get_mut(cur) {
                    Some(x) => {
                        x.owner = owner;
                        x.next
                    }
                    None => return Err(TopoError::StaleKey),
                };
                cur = next;
                if cur == entry {
                    break;
                }
            }
        }
        self.repoint_vertex_fins(&[a, b], lp);
        for id in [
            self.fins.get(a).map(|x| x.id),
            self.fins.get(b).map(|x| x.id),
            self.edges.get(edge).map(|x| x.id),
        ]
        .into_iter()
        .flatten()
        {
            self.unregister(&mut rec, id);
        }
        self.fins.remove(a);
        self.fins.remove(b);
        self.edges.remove(edge);
        let report = rec.finish();
        self.debug_validate();
        Ok(report)
    }

    /// Make edge, kill ring: inverse of kemr. Bridges an inner ring of
    /// a face back into the loop `fin_outer` belongs to, with a new
    /// edge from end-vertex(fin_outer) to end-vertex(fin_ring).
    pub fn mekr(&mut self, fin_outer: FinKey, fin_ring: FinKey) -> Result<MevOut, TopoError> {
        let fo = self.fins.get(fin_outer).ok_or(TopoError::StaleKey)?;
        let fr = self.fins.get(fin_ring).ok_or(TopoError::StaleKey)?;
        let (lp_outer, lp_ring) = (fo.owner, fr.owner);
        if lp_outer == lp_ring {
            return Err(TopoError::Precondition("mekr: fins share a loop"));
        }
        let face = self
            .loops
            .get(lp_outer)
            .map(|l| l.face)
            .ok_or(TopoError::StaleKey)?;
        if self.loops.get(lp_ring).map(|l| (l.face, l.kind)) != Some((face, LoopKind::Inner)) {
            return Err(TopoError::Precondition(
                "mekr: second loop must be an inner ring of the same face",
            ));
        }
        let va = self
            .fin_end_vertex(fin_outer)
            .ok_or(TopoError::Precondition("mekr: fin_outer end vertex"))?;
        let vb = self
            .fin_end_vertex(fin_ring)
            .ok_or(TopoError::Precondition("mekr: fin_ring end vertex"))?;

        let mut rec = self.begin_op();
        let e = self.new_edge(&mut rec, (va, vb), Derivation::Created);
        let p = self.new_fin(&mut rec, e, true, lp_outer, Derivation::Created);
        let q = self.new_fin(&mut rec, e, false, lp_outer, Derivation::Created);
        if let Some(edge) = self.edges.get_mut(e) {
            edge.radial = vec![p, q];
        }
        let o_next = self
            .fins
            .get(fin_outer)
            .map(|x| x.next)
            .ok_or(TopoError::StaleKey)?;
        let r_next = self
            .fins
            .get(fin_ring)
            .map(|x| x.next)
            .ok_or(TopoError::StaleKey)?;
        // Merged ring: ... fin_outer, p, r_next ... fin_ring, q, o_next ...
        if let Some(x) = self.fins.get_mut(fin_outer) {
            x.next = p;
        }
        if let Some(x) = self.fins.get_mut(p) {
            x.prev = fin_outer;
            x.next = r_next;
        }
        if let Some(x) = self.fins.get_mut(r_next) {
            x.prev = p;
        }
        if let Some(x) = self.fins.get_mut(fin_ring) {
            x.next = q;
        }
        if let Some(x) = self.fins.get_mut(q) {
            x.prev = fin_ring;
            x.next = o_next;
        }
        if let Some(x) = self.fins.get_mut(o_next) {
            x.prev = q;
        }
        // Re-own the merged ring; delete the ring loop.
        let mut cur = p;
        loop {
            let next = match self.fins.get_mut(cur) {
                Some(x) => {
                    x.owner = lp_outer;
                    x.next
                }
                None => return Err(TopoError::StaleKey),
            };
            cur = next;
            if cur == p {
                break;
            }
        }
        if let Some(fc) = self.faces.get_mut(face) {
            fc.loops.retain(|&l| l != lp_ring);
        }
        if let Some(id) = self.loops.get(lp_ring).map(|x| x.id) {
            self.unregister(&mut rec, id);
        }
        self.loops.remove(lp_ring);
        let report = rec.finish();
        self.debug_validate();
        Ok(MevOut {
            edge: e,
            vertex: vb,
            report,
        })
    }

    /// Kill face, make ring, make hole (gate 2.1): kill a single-loop
    /// face, attach its loop as an inner ring of `face_keep`, and
    /// raise the genus of the shell pair. The faces must share both
    /// shells. Inverse: mfkrh.
    pub fn kfmrh(&mut self, face_kill: FaceKey, face_keep: FaceKey) -> Result<OpReport, TopoError> {
        if face_kill == face_keep {
            return Err(TopoError::Precondition("kfmrh: faces must differ"));
        }
        if self.faces.get(face_kill).map(|f| f.loops.len()) != Some(1) {
            return Err(TopoError::Precondition(
                "kfmrh: dying face must have one loop",
            ));
        }
        let (sf_kill, sb_kill) = self.shells_of_face(face_kill)?;
        let (sf_keep, sb_keep) = self.shells_of_face(face_keep)?;
        if (sf_kill, sb_kill) != (sf_keep, sb_keep) {
            return Err(TopoError::Precondition(
                "kfmrh: faces must share both shells",
            ));
        }
        let lp = self
            .faces
            .get(face_kill)
            .map(|f| f.loops[0])
            .ok_or(TopoError::StaleKey)?;

        let mut rec = self.begin_op();
        let keep_id = self
            .faces
            .get(face_keep)
            .map(|x| x.id)
            .unwrap_or_default_id();
        if let Some(l) = self.loops.get_mut(lp) {
            l.face = face_keep;
            l.kind = LoopKind::Inner;
        }
        if let Some(f) = self.faces.get_mut(face_keep) {
            f.loops.push(lp);
        }
        // Record the surviving loop as modified by this op.
        if let Some(l) = self.loops.get(lp) {
            rec.report.modified.push((keep_id, l.id));
        }
        for sk in [sf_kill, sb_kill] {
            if let Some(s) = self.shells.get_mut(sk) {
                s.faces.retain(|&(fk, _)| fk != face_kill);
                s.genus += 1;
            }
        }
        if let Some(id) = self.faces.get(face_kill).map(|x| x.id) {
            self.unregister(&mut rec, id);
        }
        self.faces.remove(face_kill);
        let report = rec.finish();
        self.debug_validate();
        Ok(report)
    }

    /// Make face, kill ring, kill hole: inverse of kfmrh. Detach an
    /// inner ring into the outer loop of a brand-new face on the same
    /// shells, lowering the genus.
    pub fn mfkrh(
        &mut self,
        ring: LoopKey,
        surface: Option<(SurfaceKey, bool)>,
    ) -> Result<MfkrhOut, TopoError> {
        let l = self.loops.get(ring).ok_or(TopoError::StaleKey)?;
        if l.kind != LoopKind::Inner {
            return Err(TopoError::Precondition("mfkrh: loop must be an inner ring"));
        }
        let host = l.face;
        let (front_region, back_region) = {
            let f = self.faces.get(host).ok_or(TopoError::StaleKey)?;
            (f.front_region, f.back_region)
        };
        let (sf, sb) = self.shells_of_face(host)?;
        if self.shells.get(sf).map(|s| s.genus) == Some(0) {
            return Err(TopoError::Precondition(
                "mfkrh: shell has no genus to remove",
            ));
        }

        let mut rec = self.begin_op();
        let host_id = self.faces.get(host).map(|x| x.id).unwrap_or_default_id();
        let new_face = self.new_face(
            &mut rec,
            front_region,
            back_region,
            Derivation::Generated { from: host_id },
        );
        if let Some(f) = self.faces.get_mut(host) {
            f.loops.retain(|&lk| lk != ring);
        }
        if let Some(l) = self.loops.get_mut(ring) {
            l.face = new_face;
            l.kind = LoopKind::Outer;
        }
        if let Some(f) = self.faces.get_mut(new_face) {
            f.loops.push(ring);
            f.surface = surface;
        }
        for sk in [sf, sb] {
            if let Some(s) = self.shells.get_mut(sk) {
                s.genus -= 1;
            }
        }
        if let Some(s) = self.shells.get_mut(sf) {
            s.faces.push((new_face, Side::Front));
        }
        if let Some(s) = self.shells.get_mut(sb) {
            s.faces.push((new_face, Side::Back));
        }
        let report = rec.finish();
        self.debug_validate();
        Ok(MfkrhOut {
            face: new_face,
            report,
        })
    }

    // ---- shared helpers ---------------------------------------------------

    /// The two shells holding (face, Front) and (face, Back).
    pub(crate) fn shells_of_face(&self, face: FaceKey) -> Result<(ShellKey, ShellKey), TopoError> {
        let mut front = None;
        let mut back = None;
        for (sk, s) in self.shells.iter() {
            for &(fk, side) in &s.faces {
                if fk == face {
                    match side {
                        Side::Front => front = Some(sk),
                        Side::Back => back = Some(sk),
                    }
                }
            }
        }
        match (front, back) {
            (Some(f), Some(b)) => Ok((f, b)),
            _ => Err(TopoError::Precondition("face sides not found in shells")),
        }
    }

    /// After deleting `dead` fins, re-point any vertex.fin references
    /// at them to a surviving fin of the given loop (or None).
    fn repoint_vertex_fins(&mut self, dead: &[FinKey], lp: LoopKey) {
        let replacement = self.loops.get(lp).and_then(|l| l.fin);
        let vkeys: Vec<VertexKey> = self
            .vertices
            .iter()
            .filter(|(_, v)| v.fin.is_some_and(|f| dead.contains(&f)))
            .map(|(k, _)| k)
            .collect();
        for vk in vkeys {
            if let Some(v) = self.vertices.get_mut(vk) {
                v.fin = replacement;
            }
        }
    }
}

// Helpers for sentinel/defaults used above.
trait UnwrapDefaultId {
    fn unwrap_or_default_id(self) -> crate::entity::EntityId;
}
impl UnwrapDefaultId for Option<crate::entity::EntityId> {
    fn unwrap_or_default_id(self) -> crate::entity::EntityId {
        self.unwrap_or(crate::entity::EntityId(u64::MAX))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// The fin of `lp` whose end vertex is `v` (test addressing aid).
    pub(crate) fn fin_ending_at(b: &Body, lp: LoopKey, v: VertexKey) -> FinKey {
        let entry = b
            .loop_(lp)
            .and_then(|l| l.fin)
            .unwrap_or_else(|| panic!("loop has no fins"));
        let mut cur = entry;
        loop {
            if b.fin_end_vertex(cur) == Some(v) {
                return cur;
            }
            cur = b
                .fin(cur)
                .map(|f| f.next)
                .unwrap_or_else(|| panic!("ring broken"));
            if cur == entry {
                panic!("no fin in loop ends at the vertex");
            }
        }
    }

    /// Build the canonical unit cube via Euler operators, asserting
    /// validity at every step. Returns the body plus the six faces in
    /// creation order [bottom, s0, s1, s2, s3-top-closer].
    pub(crate) fn cube() -> (Body, Vec<FaceKey>) {
        let mut b = Body::new();
        let r = b.infinite_region();
        let p = Vec3::new;
        let seed = b.mvfs(r, p(0., 0., 0.)).unwrap_or_else(|e| panic!("{e:?}"));
        let lp = b
            .face(seed.face)
            .map(|f| f.loops[0])
            .unwrap_or_else(|| panic!());
        let v0 = seed.vertex;
        // Bottom square rim.
        let m1 = b
            .mev(MevSite::VertexLoop(lp), p(1., 0., 0.))
            .unwrap_or_else(|e| panic!("{e:?}"));
        let f1 = fin_ending_at(&b, lp, m1.vertex);
        let m2 = b
            .mev(MevSite::AfterFin(f1), p(1., 1., 0.))
            .unwrap_or_else(|e| panic!("{e:?}"));
        let f2 = fin_ending_at(&b, lp, m2.vertex);
        let m3 = b
            .mev(MevSite::AfterFin(f2), p(0., 1., 0.))
            .unwrap_or_else(|e| panic!("{e:?}"));
        let f3 = fin_ending_at(&b, lp, m3.vertex);
        let f0 = fin_ending_at(&b, lp, v0);
        let bottom = b.mef(f3, f0, None).unwrap_or_else(|e| panic!("{e:?}"));
        // The seed face keeps loop lp (now the top rim); `bottom.face`
        // carries the bottom square. Verticals from each rim vertex.
        let rim = [v0, m1.vertex, m2.vertex, m3.vertex];
        let mut tops = Vec::new();
        for (i, &rv) in rim.iter().enumerate() {
            let at = fin_ending_at(&b, lp, rv);
            let mv = b
                .mev(MevSite::AfterFin(at), p(rim_x(i), rim_y(i), 1.))
                .unwrap_or_else(|e| panic!("{e:?}"));
            tops.push(mv.vertex);
        }
        // Side faces: connect consecutive top vertices.
        let mut faces = vec![bottom.face];
        for i in 0..4 {
            let a = fin_ending_at(&b, lp, tops[i]);
            let c = fin_ending_at(&b, lp, tops[(i + 1) % 4]);
            let side = b.mef(a, c, None).unwrap_or_else(|e| panic!("{e:?}"));
            faces.push(side.face);
        }
        assert!(b.validate().is_ok());
        (b, faces)
    }

    fn rim_x(i: usize) -> f64 {
        [0., 1., 1., 0.][i]
    }
    fn rim_y(i: usize) -> f64 {
        [0., 0., 1., 1.][i]
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{cube, fin_ending_at};
    use super::*;

    #[test]
    fn mvfs_seeds_minimal_body() {
        let mut b = Body::new();
        let r = b.infinite_region();
        let out = b.mvfs(r, Vec3::ZERO).unwrap();
        assert!(b.validate().is_ok());
        assert!(b.region(out.interior).map(|x| x.solid).unwrap_or(false));
        let f = b.face(out.face).unwrap();
        assert_ne!(f.front_region, f.back_region);
        // v, face, loop, 2 shells, region
        assert_eq!(out.report.created.len(), 6);
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f, c.regions), (1, 0, 1, 2));
    }

    #[test]
    fn kvfs_inverts_mvfs() {
        let mut b = Body::new();
        let r = b.infinite_region();
        let out = b.mvfs(r, Vec3::ZERO).unwrap();
        b.kvfs(out.face).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.entity_count(), 1); // infinite region only
    }

    #[test]
    fn mev_from_vertex_loop_then_spur_then_kev() {
        let mut b = Body::new();
        let r = b.infinite_region();
        let seed = b.mvfs(r, Vec3::ZERO).unwrap();
        let lp = b.face(seed.face).unwrap().loops[0];
        let m1 = b
            .mev(MevSite::VertexLoop(lp), Vec3::new(1., 0., 0.))
            .unwrap();
        assert!(b.validate().is_ok());
        let site = fin_ending_at(&b, lp, m1.vertex);
        let m2 = b
            .mev(MevSite::AfterFin(site), Vec3::new(2., 0., 0.))
            .unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts().e, 2);
        b.kev(m2.edge).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts().e, 1);
        // And all the way back down to the vertex loop.
        b.kev(m1.edge).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts().e, 0);
        assert!(b.loop_(lp).map(|l| l.vertex.is_some()).unwrap_or(false));
    }

    #[test]
    fn cube_by_euler_operators() {
        let (b, faces) = cube();
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (8, 12, 6));
        assert_eq!(c.inner_rings, 0);
        assert_eq!(c.genus, 0);
        assert_eq!(c.regions, 2);
        assert_eq!(faces.len(), 5); // plus the seed face = 6 total
    }

    #[test]
    fn kef_inverts_mef_on_cube_step() {
        // Build the bottom rim + bottom face, then undo the mef.
        let mut b = Body::new();
        let r = b.infinite_region();
        let seed = b.mvfs(r, Vec3::ZERO).unwrap();
        let lp = b.face(seed.face).unwrap().loops[0];
        let m1 = b
            .mev(MevSite::VertexLoop(lp), Vec3::new(1., 0., 0.))
            .unwrap();
        let f1 = fin_ending_at(&b, lp, m1.vertex);
        let m2 = b.mev(MevSite::AfterFin(f1), Vec3::new(1., 1., 0.)).unwrap();
        let f2 = fin_ending_at(&b, lp, m2.vertex);
        let f0 = fin_ending_at(&b, lp, seed.vertex);
        let counts_before = b.counts();
        let tri = b.mef(f2, f0, None).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts().f, counts_before.f + 1);
        b.kef(tri.edge).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts(), counts_before);
    }

    #[test]
    fn kemr_and_mekr_round_trip() {
        // Cube, then bridge a face: mev+mev+mef makes structure; the
        // simplest bridge: split a face with mef then kill that edge
        // as a bridge is not possible (fins in different loops). Build
        // a genuine bridge: spur + mef from the spur tip back to its
        // own loop... Simplest: take a cube face, mev a spur into it,
        // mef from the spur to itself is closed-edge; instead make a
        // dangling square inside the face and bridge it.
        let (mut b, faces) = cube();
        let face = faces[0];
        let lp = b.face(face).map(|f| f.loops[0]).unwrap_or_else(|| panic!());
        let entry = b.loop_(lp).and_then(|l| l.fin).unwrap_or_else(|| panic!());
        // Spur into the face interior, then a closed edge at its tip
        // makes a new face; killing the SPUR as a bridge is invalid
        // (it is a spur). So: extend the spur and connect back to the
        // spur root with mef, forming a triangle hanging by the spur
        // edge; the spur edge is then a genuine bridge.

        let s1 = b
            .mev(MevSite::AfterFin(entry), Vec3::new(0.4, 0.4, 0.0))
            .unwrap();
        let s1f = fin_ending_at(&b, lp, s1.vertex);
        let s2 = b
            .mev(MevSite::AfterFin(s1f), Vec3::new(0.6, 0.4, 0.0))
            .unwrap();
        let s2f = fin_ending_at(&b, lp, s2.vertex);
        let s3 = b
            .mev(MevSite::AfterFin(s2f), Vec3::new(0.5, 0.6, 0.0))
            .unwrap();
        let s3f = fin_ending_at(&b, lp, s3.vertex);
        let back = fin_ending_at(&b, lp, s1.vertex);
        let _ = back;
        // Close the triangle s1-s2-s3.
        let to_s1 = {
            // fin ending at s1.vertex on the return path: after s3 the
            // loop returns through the spur; mef from s3f to the fin
            // ending at s1.vertex.
            fin_ending_at(&b, lp, s1.vertex)
        };
        let tri = b.mef(s3f, to_s1, None).unwrap();
        assert!(b.validate().is_ok());
        // The mef moved the wrap-around fins (including both spur
        // fins) into the NEW face's loop, where the spur edge is now a
        // genuine bridge: both fins in one loop, not adjacent.
        let c_before = b.counts();
        let bridge_fin = b
            .edge(s1.edge)
            .map(|e| e.radial[0])
            .unwrap_or_else(|| panic!());
        let bridge_face = b
            .fin(bridge_fin)
            .and_then(|f| b.loop_(f.owner))
            .map(|l| l.face)
            .unwrap_or_else(|| panic!());
        assert_eq!(bridge_face, tri.face);
        b.kemr(bridge_fin).unwrap();
        assert!(b.validate().is_ok());
        let c_after = b.counts();
        assert_eq!(c_after.inner_rings, c_before.inner_rings + 1);
        assert_eq!(c_after.e, c_before.e - 1);
        // And bridge it back.
        let (outer_lp, ring) = {
            let f = b.face(bridge_face).unwrap_or_else(|| panic!());
            (f.loops[0], f.loops[f.loops.len() - 1])
        };
        let outer_fin = b
            .loop_(outer_lp)
            .and_then(|l| l.fin)
            .unwrap_or_else(|| panic!());
        let ring_fin = b
            .loop_(ring)
            .and_then(|l| l.fin)
            .unwrap_or_else(|| panic!());
        b.mekr(outer_fin, ring_fin).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts().inner_rings, c_before.inner_rings);
        let _ = face;
    }

    #[test]
    fn closed_solids_skeletons() {
        // Balloon: V1 E1 F2.
        let mut b = Body::new();
        let r = b.infinite_region();
        let seed = b.mvfs(r, Vec3::ZERO).unwrap();
        let lp = b.face(seed.face).unwrap().loops[0];
        b.mef_on_vertex_loop(lp, None).unwrap();
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f, c.genus), (1, 1, 2, 0));

        // Sphere: V2 E1 F1 (seam meridian).
        let mut b = Body::new();
        let r = b.infinite_region();
        let seed = b.mvfs(r, Vec3::ZERO).unwrap();
        let lp = b.face(seed.face).unwrap().loops[0];
        b.mev(MevSite::VertexLoop(lp), Vec3::new(0., 0., 2.))
            .unwrap();
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f, c.genus), (2, 1, 1, 0));

        // Cylinder: V2 E3 F3 (two cap circles + seam).
        let mut b = Body::new();
        let r = b.infinite_region();
        let seed = b.mvfs(r, Vec3::ZERO).unwrap();
        let lp = b.face(seed.face).unwrap().loops[0];
        let bottom = b.mef_on_vertex_loop(lp, None).unwrap();
        let _ = bottom;
        let seam = b
            .mev(
                MevSite::AfterFin(b.loop_(lp).and_then(|l| l.fin).unwrap_or_else(|| panic!())),
                Vec3::new(0., 0., 1.),
            )
            .unwrap();
        let at_top = fin_ending_at(&b, lp, seam.vertex);
        b.mef(at_top, at_top, None).unwrap();
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f, c.genus), (2, 3, 3, 0));

        // Genus-1 (torus skeleton): V2 E2 F1 with one ring, G1.
        let mut b = Body::new();
        let r = b.infinite_region();
        let seed = b.mvfs(r, Vec3::ZERO).unwrap();
        let fa = seed.face;
        let lp = b.face(fa).unwrap().loops[0];
        b.mev(MevSite::VertexLoop(lp), Vec3::new(1., 0., 0.))
            .unwrap();
        let a = b.loop_(lp).and_then(|l| l.fin).unwrap_or_else(|| panic!());
        let mef = b.mef(a, a, None).unwrap();
        b.kfmrh(mef.face, fa).unwrap();
        let c = b.counts();
        assert_eq!((c.v, c.e, c.f), (2, 2, 1));
        assert_eq!(c.inner_rings, 1);
        assert_eq!(c.genus, 1);
        assert!(b.validate().is_ok());
    }

    #[test]
    fn kfmrh_mfkrh_round_trip() {
        let mut b = Body::new();
        let r = b.infinite_region();
        let seed = b.mvfs(r, Vec3::ZERO).unwrap();
        let fa = seed.face;
        let lp = b.face(fa).unwrap().loops[0];
        b.mev(MevSite::VertexLoop(lp), Vec3::new(1., 0., 0.))
            .unwrap();
        let a = b.loop_(lp).and_then(|l| l.fin).unwrap_or_else(|| panic!());
        let mef = b.mef(a, a, None).unwrap();
        let before = b.counts();
        b.kfmrh(mef.face, fa).unwrap();
        assert_eq!(b.counts().genus, 1);
        // Reverse: detach the ring back into its own face.
        let ring = b
            .face(fa)
            .map(|f| f.loops[f.loops.len() - 1])
            .unwrap_or_else(|| panic!());
        b.mfkrh(ring, None).unwrap();
        assert!(b.validate().is_ok());
        assert_eq!(b.counts(), before);
    }
}
