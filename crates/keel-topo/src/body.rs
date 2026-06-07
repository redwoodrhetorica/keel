//! The Body: arenas, identity allocation, lineage storage, attributes.
//! All mutation goes through the operator modules (euler, ops,
//! construct); this module owns storage and bookkeeping only.

use std::collections::BTreeMap;

use crate::arena::Arena;
use crate::entity::{
    AnyKey, AttrValue, CurveGeom, CurveKey, Edge, EdgeKey, EntityId, Face, FaceKey, Fin, FinKey,
    Loop, LoopKey, Region, RegionKey, Shell, ShellKey, SurfaceGeom, SurfaceKey, Vertex, VertexKey,
};
use crate::lineage::{Derivation, Lineage, OpId, OpReport};
use keel_math::tolerance::Tolerances;
use keel_math::vec::Vec3;

/// Session tolerance floor applied to new entities.
fn default_linear_tolerance() -> f64 {
    Tolerances::default().linear
}

/// Errors from topology construction and mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopoError {
    /// A key did not resolve (stale or foreign).
    StaleKey,
    /// Operation preconditions not met (documented per operator).
    Precondition(&'static str),
    /// The body failed validation (debug builds panic instead).
    Invalid,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub(crate) vertices: Arena<Vertex>,
    pub(crate) edges: Arena<Edge>,
    pub(crate) fins: Arena<Fin>,
    pub(crate) loops: Arena<Loop>,
    pub(crate) faces: Arena<Face>,
    pub(crate) shells: Arena<Shell>,
    pub(crate) regions: Arena<Region>,
    pub(crate) curves: Arena<CurveGeom>,
    pub(crate) surfaces: Arena<SurfaceGeom>,
    next_entity_id: u64,
    next_op_id: u64,
    /// Identity -> live entity. THE deterministic iteration order.
    pub(crate) ids: BTreeMap<EntityId, AnyKey>,
    pub(crate) lineage: BTreeMap<EntityId, Lineage>,
    pub(crate) attrs: BTreeMap<EntityId, BTreeMap<String, AttrValue>>,
    infinite_region: RegionKey,
}

impl Body {
    /// New body: a single infinite void region under op 0.
    pub fn new() -> Self {
        let mut b = Self {
            vertices: Arena::new(),
            edges: Arena::new(),
            fins: Arena::new(),
            loops: Arena::new(),
            faces: Arena::new(),
            shells: Arena::new(),
            regions: Arena::new(),
            curves: Arena::new(),
            surfaces: Arena::new(),
            next_entity_id: 0,
            next_op_id: 0,
            ids: BTreeMap::new(),
            lineage: BTreeMap::new(),
            attrs: BTreeMap::new(),
            // Placeholder; replaced below once allocated.
            infinite_region: RegionKey::sentinel(),
        };
        let op = b.alloc_op();
        let id = b.alloc_id();
        let key = b.regions.insert(Region {
            id,
            solid: false,
            infinite: true,
            shells: Vec::new(),
        });
        b.ids.insert(id, AnyKey::Region(key));
        b.lineage.insert(
            id,
            Lineage {
                op,
                derivation: Derivation::Created,
            },
        );
        b.infinite_region = key;
        b
    }

    #[inline]
    pub fn infinite_region(&self) -> RegionKey {
        self.infinite_region
    }

    pub(crate) fn alloc_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    pub(crate) fn alloc_op(&mut self) -> OpId {
        let op = OpId(self.next_op_id);
        self.next_op_id += 1;
        op
    }

    // ---- read accessors -------------------------------------------------

    pub fn vertex(&self, k: VertexKey) -> Option<&Vertex> {
        self.vertices.get(k)
    }
    pub fn edge(&self, k: EdgeKey) -> Option<&Edge> {
        self.edges.get(k)
    }
    pub fn fin(&self, k: FinKey) -> Option<&Fin> {
        self.fins.get(k)
    }
    pub fn loop_(&self, k: LoopKey) -> Option<&Loop> {
        self.loops.get(k)
    }
    pub fn face(&self, k: FaceKey) -> Option<&Face> {
        self.faces.get(k)
    }
    pub fn shell(&self, k: ShellKey) -> Option<&Shell> {
        self.shells.get(k)
    }
    pub fn region(&self, k: RegionKey) -> Option<&Region> {
        self.regions.get(k)
    }
    pub fn curve(&self, k: CurveKey) -> Option<&CurveGeom> {
        self.curves.get(k)
    }
    pub fn surface(&self, k: SurfaceKey) -> Option<&SurfaceGeom> {
        self.surfaces.get(k)
    }

    pub fn lookup(&self, id: EntityId) -> Option<AnyKey> {
        self.ids.get(&id).copied()
    }

    /// Live entity ids in ascending order: THE deterministic output order.
    pub fn entity_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.ids.keys().copied()
    }

    /// Number of live topological entities (geometry not counted).
    pub fn entity_count(&self) -> usize {
        self.ids.len()
    }

    pub fn lineage_of(&self, id: EntityId) -> Option<&Lineage> {
        self.lineage.get(&id)
    }

    // ---- lineage queries (gate design 3.3) ------------------------------

    /// Entities created/derived by the given operation.
    pub fn created_by(&self, op: OpId) -> Vec<EntityId> {
        self.lineage
            .iter()
            .filter(|(id, l)| l.op == op && self.ids.contains_key(id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Live entities whose lineage chain reaches any of `sources`.
    pub fn derived_from_any(&self, sources: &[EntityId]) -> Vec<EntityId> {
        use std::collections::BTreeSet;
        let mut reach: BTreeSet<EntityId> = sources.iter().copied().collect();
        // Lineage parents always have smaller ids than children (ids
        // are monotonic and parents exist first), so one ascending
        // pass over the lineage map reaches the full closure.
        for (id, l) in &self.lineage {
            let parents: Vec<EntityId> = match &l.derivation {
                Derivation::Created => Vec::new(),
                Derivation::Modified { from }
                | Derivation::Generated { from }
                | Derivation::SplitChild { from, .. } => vec![*from],
                Derivation::MergeResult { from } => from.clone(),
            };
            if parents.iter().any(|p| reach.contains(p)) {
                reach.insert(*id);
            }
        }
        reach
            .into_iter()
            .filter(|id| self.ids.contains_key(id) && !sources.contains(id))
            .collect()
    }

    // ---- attributes ------------------------------------------------------

    pub fn set_attr(&mut self, id: EntityId, key: &str, value: AttrValue) {
        self.attrs.entry(id).or_default().insert(key.into(), value);
    }

    pub fn attr(&self, id: EntityId, key: &str) -> Option<&AttrValue> {
        self.attrs.get(&id).and_then(|m| m.get(key))
    }

    // ---- geometry attachment --------------------------------------------

    pub fn add_curve(&mut self, c: CurveGeom) -> CurveKey {
        self.curves.insert(c)
    }
    pub fn add_surface(&mut self, s: SurfaceGeom) -> SurfaceKey {
        self.surfaces.insert(s)
    }

    // ---- entity creation/deletion helpers (operators only) ---------------

    pub(crate) fn new_vertex(&mut self, rec: &mut OpRecorder, point: Vec3) -> VertexKey {
        let id = self.alloc_id();
        let key = self.vertices.insert(Vertex {
            id,
            point,
            tolerance: default_linear_tolerance(),
            fin: None,
            groups: Vec::new(),
        });
        self.register(rec, id, AnyKey::Vertex(key), Derivation::Created);
        key
    }

    pub(crate) fn new_edge(
        &mut self,
        rec: &mut OpRecorder,
        bounds: (VertexKey, VertexKey),
        derivation: Derivation,
    ) -> EdgeKey {
        let id = self.alloc_id();
        let key = self.edges.insert(Edge {
            id,
            curve: None,
            bounds,
            radial: Vec::new(),
            tolerance: default_linear_tolerance(),
        });
        self.register(rec, id, AnyKey::Edge(key), derivation);
        key
    }

    pub(crate) fn new_fin(
        &mut self,
        rec: &mut OpRecorder,
        edge: EdgeKey,
        forward: bool,
        owner: LoopKey,
        derivation: Derivation,
    ) -> FinKey {
        let id = self.alloc_id();
        // next/prev are self-links until the caller splices the ring.
        let key = self.fins.insert(Fin {
            id,
            edge,
            forward,
            owner,
            next: FinKey::sentinel(),
            prev: FinKey::sentinel(),
            pcurve: None,
        });
        // Self-link so the ring is never dangling.
        if let Some(f) = self.fins.get_mut(key) {
            f.next = key;
            f.prev = key;
        }
        self.register(rec, id, AnyKey::Fin(key), derivation);
        key
    }

    pub(crate) fn new_loop(
        &mut self,
        rec: &mut OpRecorder,
        face: FaceKey,
        kind: crate::entity::LoopKind,
        derivation: Derivation,
    ) -> LoopKey {
        let id = self.alloc_id();
        let key = self.loops.insert(Loop {
            id,
            face,
            fin: None,
            vertex: None,
            kind,
        });
        self.register(rec, id, AnyKey::Loop(key), derivation);
        key
    }

    pub(crate) fn new_face(
        &mut self,
        rec: &mut OpRecorder,
        front_region: RegionKey,
        back_region: RegionKey,
        derivation: Derivation,
    ) -> FaceKey {
        let id = self.alloc_id();
        let key = self.faces.insert(Face {
            id,
            surface: None,
            loops: Vec::new(),
            front_region,
            back_region,
        });
        self.register(rec, id, AnyKey::Face(key), derivation);
        key
    }

    pub(crate) fn new_shell(
        &mut self,
        rec: &mut OpRecorder,
        region: RegionKey,
        derivation: Derivation,
    ) -> ShellKey {
        let id = self.alloc_id();
        let key = self.shells.insert(Shell {
            id,
            region,
            faces: Vec::new(),
            wires: Vec::new(),
            acorn: None,
            genus: 0,
        });
        self.register(rec, id, AnyKey::Shell(key), derivation);
        key
    }

    pub(crate) fn new_region(
        &mut self,
        rec: &mut OpRecorder,
        solid: bool,
        derivation: Derivation,
    ) -> RegionKey {
        let id = self.alloc_id();
        let key = self.regions.insert(Region {
            id,
            solid,
            infinite: false,
            shells: Vec::new(),
        });
        self.register(rec, id, AnyKey::Region(key), derivation);
        key
    }

    fn register(&mut self, rec: &mut OpRecorder, id: EntityId, key: AnyKey, d: Derivation) {
        self.ids.insert(id, key);
        match &d {
            Derivation::Created => rec.report.created.push(id),
            Derivation::Modified { from } => rec.report.modified.push((*from, id)),
            Derivation::Generated { from } => rec.report.generated.push((*from, id)),
            Derivation::SplitChild { from, .. } => rec.note_split(*from, id),
            Derivation::MergeResult { from } => rec.report.merged.push((from.clone(), id)),
        }
        self.lineage.insert(
            id,
            Lineage {
                op: rec.op,
                derivation: d,
            },
        );
    }

    /// Remove an entity from the id map and record the deletion. The
    /// caller removes the record from its arena.
    pub(crate) fn unregister(&mut self, rec: &mut OpRecorder, id: EntityId) {
        self.ids.remove(&id);
        self.attrs.remove(&id);
        rec.report.deleted.push(id);
    }

    /// Begin a recorded operation.
    pub(crate) fn begin_op(&mut self) -> OpRecorder {
        let op = self.alloc_op();
        OpRecorder {
            op,
            report: OpReport::new(op),
        }
    }

    #[cfg(test)]
    pub fn debug_raw_vertex(&mut self, p: Vec3) -> VertexKey {
        let mut rec = self.begin_op();
        let k = self.new_vertex(&mut rec, p);
        let _ = rec.finish();
        k
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::new()
    }
}

/// Collects an operation's report and stamps lineage as entities are
/// created and deleted. Exactly one per public mutation.
pub(crate) struct OpRecorder {
    pub(crate) op: OpId,
    pub(crate) report: OpReport,
}

impl OpRecorder {
    fn note_split(&mut self, parent: EntityId, child: EntityId) {
        if let Some((_, children)) = self.report.split.iter_mut().find(|(p, _)| *p == parent) {
            children.push(child);
        } else {
            self.report.split.push((parent, vec![child]));
        }
    }

    pub(crate) fn finish(mut self) -> OpReport {
        self.report.sort();
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ids_are_monotonic_and_stable() {
        let mut b = Body::new();
        let r = b.infinite_region();
        let id0 = b.region(r).map(|x| x.id).unwrap_or(EntityId(u64::MAX));
        let v = b.debug_raw_vertex(Vec3::ZERO);
        let id1 = b.vertex(v).map(|x| x.id).unwrap_or(EntityId(0));
        assert!(id1 > id0);
        assert_eq!(b.lookup(id1), Some(AnyKey::Vertex(v)));
        assert_eq!(b.entity_count(), 2);
    }

    #[test]
    fn new_body_has_one_infinite_void_region() {
        let b = Body::new();
        let r = b.region(b.infinite_region()).map(|x| (x.infinite, x.solid));
        assert_eq!(r, Some((true, false)));
        assert_eq!(b.entity_count(), 1);
    }
}
