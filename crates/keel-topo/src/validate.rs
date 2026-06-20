//! Validation (gate design 2.3) and the deterministic topology hash.
//!
//! The scalar Euler-Poincare identity governs the manifold complexes;
//! the non-manifold cases fall back to structural checks plus the
//! boundary-chain (d-of-d) oracle. Debug builds run validate() after
//! every public operation.

use crate::body::Body;
use crate::entity::{AnyKey, CurveGeom, FinKey, LoopKind, Side, SurfaceGeom, VertexKey};

/// A specific way a body failed [`Body::validate`]. The error list names
/// every invariant that was broken, so a consumer can report or debug a
/// malformed body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    /// A stored key points at a deleted or foreign entity.
    StaleReference(&'static str),
    /// The id-to-entity map disagrees with the arenas.
    IdMapInconsistent,
    /// A loop's fin ring is not a closed doubly-linked cycle.
    FinRingBroken(&'static str),
    /// An edge's radial cycle of fins is malformed.
    RadialCycleBroken(&'static str),
    /// A loop's structure (entry fin / vertex loop) is inconsistent.
    LoopInconsistent(&'static str),
    /// A shell/region back-reference is inconsistent.
    ShellRegionInconsistent(&'static str),
    /// A face's boundary chain does not close (the d-of-d oracle):
    /// dangling or open edges, the signature of a non-watertight solid.
    BoundaryChainBroken,
    /// The Euler-Poincare identity does not hold (`lhs` != `rhs`).
    EulerPoincareViolated { lhs: i64, rhs: i64 },
}

/// Combinatorial counts of a body's topology, used by tests and the
/// Euler-Poincare check. Returned by [`Body::counts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    /// Vertex count.
    pub v: usize,
    /// Edge count.
    pub e: usize,
    /// Face count.
    pub f: usize,
    /// Number of inner (hole) loops across all faces.
    pub inner_rings: usize,
    /// Region count (including the infinite region).
    pub regions: usize,
    /// Shell count.
    pub shells: usize,
    /// Total genus (handles) of the body's shells.
    pub genus: u32,
}

impl Body {
    /// Combinatorial entity counts (vertices, edges, faces, loops,
    /// regions, shells, genus). See [`Counts`].
    pub fn counts(&self) -> Counts {
        Counts {
            v: self.vertices.len(),
            e: self.edges.len(),
            f: self.faces.len(),
            inner_rings: self
                .loops
                .iter()
                .filter(|(_, l)| l.kind == LoopKind::Inner)
                .count(),
            regions: self.regions.len(),
            shells: self.shells.len(),
            genus: self.shells.iter().map(|(_, s)| s.genus).sum::<u32>() / 2,
        }
    }

    /// Check every structural invariant of the body.
    ///
    /// Verifies the id map, fin rings, radial cycles, loops,
    /// shell/region links, boundary-chain closure, and the
    /// Euler-Poincare identity. A solid that passes is watertight: the
    /// boundary-chain check rejects open or dangling edges. This is the
    /// public-API expression of "is this body well-formed".
    ///
    /// # Errors
    ///
    /// Returns `Err(Vec<ValidationError>)` listing every invariant that
    /// failed. An empty body and any well-formed body return `Ok(())`.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errs = Vec::new();
        self.check_id_map(&mut errs);
        self.check_fin_rings(&mut errs);
        self.check_radial_cycles(&mut errs);
        self.check_loops(&mut errs);
        self.check_shells_regions(&mut errs);
        self.check_boundary_chains(&mut errs);
        self.check_euler_poincare(&mut errs);
        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }

    /// Panic with the error list in debug builds; used by operators.
    pub(crate) fn debug_validate(&self) {
        #[cfg(debug_assertions)]
        if let Err(errs) = self.validate() {
            panic!("topology validation failed: {errs:?}");
        }
    }

    fn check_id_map(&self, errs: &mut Vec<ValidationError>) {
        let live = self.vertices.len()
            + self.edges.len()
            + self.fins.len()
            + self.loops.len()
            + self.faces.len()
            + self.shells.len()
            + self.regions.len();
        if live != self.ids.len() {
            errs.push(ValidationError::IdMapInconsistent);
        }
        for (id, key) in &self.ids {
            let ok = match key {
                AnyKey::Vertex(k) => self.vertices.get(*k).map(|x| x.id) == Some(*id),
                AnyKey::Edge(k) => self.edges.get(*k).map(|x| x.id) == Some(*id),
                AnyKey::Fin(k) => self.fins.get(*k).map(|x| x.id) == Some(*id),
                AnyKey::Loop(k) => self.loops.get(*k).map(|x| x.id) == Some(*id),
                AnyKey::Face(k) => self.faces.get(*k).map(|x| x.id) == Some(*id),
                AnyKey::Shell(k) => self.shells.get(*k).map(|x| x.id) == Some(*id),
                AnyKey::Region(k) => self.regions.get(*k).map(|x| x.id) == Some(*id),
            };
            if !ok {
                errs.push(ValidationError::IdMapInconsistent);
                return;
            }
        }
    }

    fn check_fin_rings(&self, errs: &mut Vec<ValidationError>) {
        for (lk, l) in self.loops.iter() {
            let Some(entry) = l.fin else { continue };
            let mut cur = entry;
            let mut steps = 0usize;
            loop {
                let Some(f) = self.fins.get(cur) else {
                    errs.push(ValidationError::FinRingBroken("stale fin in ring"));
                    return;
                };
                if f.owner != lk {
                    errs.push(ValidationError::FinRingBroken("fin owner mismatch"));
                    return;
                }
                let Some(nf) = self.fins.get(f.next) else {
                    errs.push(ValidationError::FinRingBroken("stale next"));
                    return;
                };
                if nf.prev != cur {
                    errs.push(ValidationError::FinRingBroken("next.prev mismatch"));
                    return;
                }
                cur = f.next;
                steps += 1;
                if cur == entry {
                    break;
                }
                if steps > self.fins.len() {
                    errs.push(ValidationError::FinRingBroken("ring does not close"));
                    return;
                }
            }
        }
    }

    fn check_radial_cycles(&self, errs: &mut Vec<ValidationError>) {
        use std::collections::BTreeMap;
        let mut seen: BTreeMap<FinKey, usize> = BTreeMap::new();
        for (ek, e) in self.edges.iter() {
            for &fk in &e.radial {
                let Some(f) = self.fins.get(fk) else {
                    errs.push(ValidationError::RadialCycleBroken("stale fin in radial"));
                    return;
                };
                if f.edge != ek {
                    errs.push(ValidationError::RadialCycleBroken(
                        "radial member edge mismatch",
                    ));
                    return;
                }
                *seen.entry(fk).or_insert(0) += 1;
            }
        }
        for (fk, f) in self.fins.iter() {
            if seen.get(&fk) != Some(&1) {
                let _ = f;
                errs.push(ValidationError::RadialCycleBroken(
                    "fin not in exactly one radial cycle",
                ));
                return;
            }
        }
    }

    fn check_loops(&self, errs: &mut Vec<ValidationError>) {
        for (lk, l) in self.loops.iter() {
            match (l.fin, l.vertex) {
                (Some(_), None) | (None, Some(_)) => {}
                _ => {
                    errs.push(ValidationError::LoopInconsistent(
                        "loop must be a fin loop xor a vertex loop",
                    ));
                    return;
                }
            }
            let Some(face) = self.faces.get(l.face) else {
                errs.push(ValidationError::LoopInconsistent("stale face"));
                return;
            };
            if !face.loops.contains(&lk) {
                errs.push(ValidationError::LoopInconsistent("face does not list loop"));
                return;
            }
        }
        for (_, face) in self.faces.iter() {
            match face.loops.first().and_then(|k| self.loops.get(*k)) {
                Some(l0) if l0.kind == LoopKind::Outer => {}
                _ => {
                    errs.push(ValidationError::LoopInconsistent(
                        "face loops[0] must be the outer loop",
                    ));
                    return;
                }
            }
        }
    }

    fn check_shells_regions(&self, errs: &mut Vec<ValidationError>) {
        use std::collections::BTreeMap;
        let mut infinite = 0usize;
        for (rk, r) in self.regions.iter() {
            if r.infinite {
                infinite += 1;
                if r.solid {
                    errs.push(ValidationError::ShellRegionInconsistent(
                        "infinite region must be void",
                    ));
                }
            }
            for &sk in &r.shells {
                match self.shells.get(sk) {
                    Some(s) if s.region == rk => {}
                    _ => {
                        errs.push(ValidationError::ShellRegionInconsistent(
                            "region shell back-reference broken",
                        ));
                        return;
                    }
                }
            }
        }
        if infinite != 1 {
            errs.push(ValidationError::ShellRegionInconsistent(
                "exactly one infinite region required",
            ));
        }
        // Every (face, side) in exactly one shell; region links match.
        let mut uses: BTreeMap<(crate::entity::FaceKey, Side), usize> = BTreeMap::new();
        for (sk, s) in self.shells.iter() {
            let Some(region) = self.regions.get(s.region) else {
                errs.push(ValidationError::ShellRegionInconsistent(
                    "stale shell region",
                ));
                return;
            };
            if !region.shells.contains(&sk) {
                errs.push(ValidationError::ShellRegionInconsistent(
                    "shell not listed by its region",
                ));
                return;
            }
            for &(fk, side) in &s.faces {
                *uses.entry((fk, side)).or_insert(0) += 1;
                let Some(face) = self.faces.get(fk) else {
                    errs.push(ValidationError::ShellRegionInconsistent("stale shell face"));
                    return;
                };
                let expect = match side {
                    Side::Front => face.front_region,
                    Side::Back => face.back_region,
                };
                if expect != s.region {
                    errs.push(ValidationError::ShellRegionInconsistent(
                        "face side region does not match owning shell region",
                    ));
                    return;
                }
            }
        }
        for (fk, _) in self.faces.iter() {
            for side in [Side::Front, Side::Back] {
                if uses.get(&(fk, side)) != Some(&1) {
                    errs.push(ValidationError::ShellRegionInconsistent(
                        "face side must appear in exactly one shell",
                    ));
                    return;
                }
            }
        }
    }

    /// Boundary chain continuity: in every fin loop the end vertex of
    /// each fin is the start vertex of the next. This is the practical
    /// d-of-d = 0 oracle (a continuous closed chain has zero mod-2
    /// vertex boundary) and catches fin-surgery errors the Euler
    /// formula cannot see.
    fn check_boundary_chains(&self, errs: &mut Vec<ValidationError>) {
        for (_, l) in self.loops.iter() {
            let Some(entry) = l.fin else { continue };
            let mut cur = entry;
            loop {
                let Some(f) = self.fins.get(cur) else { return };
                let Some(nf) = self.fins.get(f.next) else {
                    return;
                };
                let (Some(end), Some(start)) = (self.fin_end_vertex(cur), {
                    let _ = nf;
                    self.fin_start_vertex(f.next)
                }) else {
                    errs.push(ValidationError::BoundaryChainBroken);
                    return;
                };
                if end != start {
                    errs.push(ValidationError::BoundaryChainBroken);
                    return;
                }
                cur = f.next;
                if cur == entry {
                    break;
                }
            }
        }
    }

    /// `V - E + F = 2*(S_closed - G) + R` on purely manifold bodies
    /// (every edge radial count exactly 2, no wires, no acorns, no
    /// non-manifold vertex groups). Skipped otherwise per the gate.
    fn check_euler_poincare(&self, errs: &mut Vec<ValidationError>) {
        let manifold = self.edges.iter().all(|(_, e)| e.radial.len() == 2)
            && self
                .shells
                .iter()
                .all(|(_, s)| s.wires.is_empty() && s.acorn.is_none())
            && self.vertices.iter().all(|(_, v)| v.groups.is_empty());
        if !manifold {
            return;
        }
        let c = self.counts();
        let s_closed = c.regions.saturating_sub(1) as i64;
        let lhs = c.v as i64 - c.e as i64 + c.f as i64;
        let rhs = 2 * (s_closed - c.genus as i64) + c.inner_rings as i64;
        if lhs != rhs {
            errs.push(ValidationError::EulerPoincareViolated { lhs, rhs });
        }
    }

    // ---- fin direction helpers (shared with operators) -------------------

    pub(crate) fn fin_start_vertex(&self, fk: FinKey) -> Option<VertexKey> {
        let f = self.fins.get(fk)?;
        let e = self.edges.get(f.edge)?;
        Some(if f.forward { e.bounds.0 } else { e.bounds.1 })
    }

    pub(crate) fn fin_end_vertex(&self, fk: FinKey) -> Option<VertexKey> {
        let f = self.fins.get(fk)?;
        let e = self.edges.get(f.edge)?;
        Some(if f.forward { e.bounds.1 } else { e.bounds.0 })
    }

    // ---- deterministic topology hash --------------------------------------

    /// FNV-1a over the entity tower in EntityId order. Structural
    /// references hash as EntityIds (never arena indices), geometry as
    /// exact f64 bits: equal bodies hash equal regardless of arena
    /// slot history.
    pub fn topology_hash(&self) -> u64 {
        let mut h = Fnv::new();
        for (id, key) in &self.ids {
            h.u64(id.0);
            match key {
                AnyKey::Vertex(k) => {
                    let Some(v) = self.vertices.get(*k) else {
                        continue;
                    };
                    h.tag(1);
                    h.f64(v.point.x);
                    h.f64(v.point.y);
                    h.f64(v.point.z);
                    h.f64(v.tolerance);
                    h.opt_id(v.fin.and_then(|f| self.fins.get(f).map(|x| x.id)));
                }
                AnyKey::Edge(k) => {
                    let Some(e) = self.edges.get(*k) else {
                        continue;
                    };
                    h.tag(2);
                    h.opt_id(self.vertices.get(e.bounds.0).map(|x| x.id));
                    h.opt_id(self.vertices.get(e.bounds.1).map(|x| x.id));
                    for &fk in &e.radial {
                        h.opt_id(self.fins.get(fk).map(|x| x.id));
                    }
                    h.f64(e.tolerance);
                    if let Some((ck, sense)) = e.curve {
                        h.tag(if sense { 3 } else { 4 });
                        if let Some(c) = self.curves.get(ck) {
                            hash_curve(&mut h, c);
                        }
                    }
                }
                AnyKey::Fin(k) => {
                    let Some(f) = self.fins.get(*k) else { continue };
                    h.tag(5);
                    h.opt_id(self.edges.get(f.edge).map(|x| x.id));
                    h.tag(if f.forward { 6 } else { 7 });
                    h.opt_id(self.loops.get(f.owner).map(|x| x.id));
                    h.opt_id(self.fins.get(f.next).map(|x| x.id));
                    h.opt_id(self.fins.get(f.prev).map(|x| x.id));
                }
                AnyKey::Loop(k) => {
                    let Some(l) = self.loops.get(*k) else {
                        continue;
                    };
                    h.tag(8);
                    h.opt_id(self.faces.get(l.face).map(|x| x.id));
                    h.opt_id(l.fin.and_then(|f| self.fins.get(f).map(|x| x.id)));
                    h.opt_id(l.vertex.and_then(|v| self.vertices.get(v).map(|x| x.id)));
                    h.tag(if l.kind == LoopKind::Outer { 9 } else { 10 });
                }
                AnyKey::Face(k) => {
                    let Some(f) = self.faces.get(*k) else {
                        continue;
                    };
                    h.tag(11);
                    for &lk in &f.loops {
                        h.opt_id(self.loops.get(lk).map(|x| x.id));
                    }
                    h.opt_id(self.regions.get(f.front_region).map(|x| x.id));
                    h.opt_id(self.regions.get(f.back_region).map(|x| x.id));
                    if let Some((sk, sense)) = f.surface {
                        h.tag(if sense { 12 } else { 13 });
                        if let Some(s) = self.surfaces.get(sk) {
                            hash_surface(&mut h, s);
                        }
                    }
                }
                AnyKey::Shell(k) => {
                    let Some(s) = self.shells.get(*k) else {
                        continue;
                    };
                    h.tag(14);
                    h.opt_id(self.regions.get(s.region).map(|x| x.id));
                    for &(fk, side) in &s.faces {
                        h.opt_id(self.faces.get(fk).map(|x| x.id));
                        h.tag(if side == Side::Front { 15 } else { 16 });
                    }
                    for &ek in &s.wires {
                        h.opt_id(self.edges.get(ek).map(|x| x.id));
                    }
                    h.opt_id(s.acorn.and_then(|v| self.vertices.get(v).map(|x| x.id)));
                    h.u64(s.genus as u64);
                }
                AnyKey::Region(k) => {
                    let Some(r) = self.regions.get(*k) else {
                        continue;
                    };
                    h.tag(17);
                    h.tag(if r.solid { 18 } else { 19 });
                    h.tag(if r.infinite { 20 } else { 21 });
                    for &sk in &r.shells {
                        h.opt_id(self.shells.get(sk).map(|x| x.id));
                    }
                }
            }
        }
        h.finish()
    }
}

/// Minimal FNV-1a, deterministic and dependency-free.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf29ce484222325)
    }
    fn byte(&mut self, b: u8) {
        self.0 ^= b as u64;
        self.0 = self.0.wrapping_mul(0x100000001b3);
    }
    fn u64(&mut self, x: u64) {
        for b in x.to_le_bytes() {
            self.byte(b);
        }
    }
    fn f64(&mut self, x: f64) {
        self.u64(x.to_bits());
    }
    fn tag(&mut self, t: u8) {
        self.byte(t);
    }
    fn opt_id(&mut self, id: Option<crate::entity::EntityId>) {
        match id {
            Some(i) => self.u64(i.0 + 1),
            None => self.u64(0),
        }
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

fn hash_vec3(h: &mut Fnv, v: keel_math::vec::Vec3) {
    h.f64(v.x);
    h.f64(v.y);
    h.f64(v.z);
}

fn hash_curve(h: &mut Fnv, c: &CurveGeom) {
    use keel_geom::curve::Curve3;
    match c {
        Curve3::Line(l) => {
            h.tag(30);
            hash_vec3(h, l.origin);
            hash_vec3(h, l.dir);
        }
        Curve3::Circle(c) => {
            h.tag(31);
            hash_vec3(h, c.center);
            hash_vec3(h, c.x_axis);
            hash_vec3(h, c.y_axis);
            h.f64(c.radius);
        }
        Curve3::Ellipse(e) => {
            h.tag(32);
            hash_vec3(h, e.center);
            hash_vec3(h, e.x_axis);
            hash_vec3(h, e.y_axis);
            h.f64(e.a);
            h.f64(e.b);
        }
        Curve3::Nurbs(n) => {
            h.tag(33);
            h.u64(n.degree() as u64);
            for &k in n.knot_vector().knots() {
                h.f64(k);
            }
            for c in n.homogeneous_control() {
                h.f64(c.x);
                h.f64(c.y);
                h.f64(c.z);
                h.f64(c.w);
            }
        }
    }
}

fn hash_surface(h: &mut Fnv, s: &SurfaceGeom) {
    use keel_geom::surface::Surface3;
    match s {
        SurfaceGeom::Analytic(a) => {
            let (tag, frame, params): (u8, &keel_geom::surface::Frame3, Vec<f64>) = match a {
                Surface3::Plane(p) => (40, &p.frame, vec![]),
                Surface3::Cylinder(c) => (41, &c.frame, vec![c.radius]),
                Surface3::Cone(c) => (42, &c.frame, vec![c.radius, c.half_angle]),
                Surface3::Sphere(sp) => (43, &sp.frame, vec![sp.radius]),
                Surface3::Torus(t) => (44, &t.frame, vec![t.major, t.minor]),
            };
            h.tag(tag);
            hash_vec3(h, frame.origin);
            hash_vec3(h, frame.x);
            hash_vec3(h, frame.y);
            hash_vec3(h, frame.z);
            for p in params {
                h.f64(p);
            }
        }
        SurfaceGeom::Nurbs(n) => {
            h.tag(45);
            h.u64(n.kv_u().degree() as u64);
            h.u64(n.kv_v().degree() as u64);
            for &k in n.kv_u().knots() {
                h.f64(k);
            }
            for &k in n.kv_v().knots() {
                h.f64(k);
            }
            for c in n.homogeneous_control() {
                h.f64(c.x);
                h.f64(c.y);
                h.f64(c.z);
                h.f64(c.w);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::body::Body;

    #[test]
    fn empty_body_validates_and_hashes_deterministically() {
        let a = Body::new();
        let b = Body::new();
        assert!(a.validate().is_ok());
        assert_eq!(a.topology_hash(), b.topology_hash());
    }
}
