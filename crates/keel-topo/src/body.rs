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

    /// Remove one attribute; returns the previous value if present.
    pub fn remove_attr(&mut self, id: EntityId, key: &str) -> Option<AttrValue> {
        self.attrs.get_mut(&id).and_then(|m| m.remove(key))
    }

    /// All attribute keys on an entity (deterministic order).
    pub fn attr_keys(&self, id: EntityId) -> Vec<String> {
        self.attrs
            .get(&id)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    // Key -> stable EntityId, so callers can attribute entities they hold
    // a key to (items 117-120).
    pub fn face_id(&self, f: FaceKey) -> Option<EntityId> {
        self.faces.get(f).map(|x| x.id)
    }
    pub fn edge_id(&self, e: EdgeKey) -> Option<EntityId> {
        self.edges.get(e).map(|x| x.id)
    }
    pub fn vertex_id(&self, v: VertexKey) -> Option<EntityId> {
        self.vertices.get(v).map(|x| x.id)
    }

    // System attributes (item 118/120): well-known names with typed
    // convenience accessors. Colors are RGB in [0,1].
    pub fn set_color(&mut self, id: EntityId, rgb: [f64; 3]) {
        self.set_attr(id, "color", AttrValue::Vec3(rgb));
    }
    pub fn color(&self, id: EntityId) -> Option<[f64; 3]> {
        match self.attr(id, "color") {
            Some(AttrValue::Vec3(c)) => Some(*c),
            _ => None,
        }
    }
    pub fn set_name(&mut self, id: EntityId, name: &str) {
        self.set_attr(id, "name", AttrValue::Str(name.into()));
    }
    pub fn name(&self, id: EntityId) -> Option<&str> {
        match self.attr(id, "name") {
            Some(AttrValue::Str(s)) => Some(s),
            _ => None,
        }
    }
    pub fn set_density(&mut self, id: EntityId, density: f64) {
        self.set_attr(id, "density", AttrValue::F64(density));
    }
    pub fn density(&self, id: EntityId) -> Option<f64> {
        match self.attr(id, "density") {
            Some(AttrValue::F64(d)) => Some(*d),
            _ => None,
        }
    }
    /// Per-entity raw user field (item 119).
    pub fn set_user_field(&mut self, id: EntityId, bytes: Vec<u8>) {
        self.set_attr(id, "user", AttrValue::Bytes(bytes));
    }
    pub fn user_field(&self, id: EntityId) -> Option<&[u8]> {
        match self.attr(id, "user") {
            Some(AttrValue::Bytes(b)) => Some(b),
            _ => None,
        }
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
            arc_sweep: None,
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
        // Attribute propagation (parity item 121): a split piece or a
        // modified entity inherits its parent's attributes; a merge
        // result inherits from its primary source. A `Generated` entity
        // (a lower-dimensional spawn, e.g. a fin off a face) does NOT
        // inherit -- it is a different entity. The child's own later
        // sets override. Done at register time, before the parent is
        // unregistered, so the source attributes are still present.
        let inherit_from = match &d {
            Derivation::Modified { from } | Derivation::SplitChild { from, .. } => Some(*from),
            Derivation::MergeResult { from } => from.first().copied(),
            Derivation::Created | Derivation::Generated { .. } => None,
        };
        if let Some(src) = inherit_from
            && let Some(parent) = self.attrs.get(&src).cloned()
        {
            let child = self.attrs.entry(id).or_default();
            for (k, v) in parent {
                child.entry(k).or_insert(v);
            }
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
    fn attributes_typed_set_get_remove_list() {
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let fid = b.face_id(out.faces[0]).unwrap();
        b.set_attr(fid, "count", AttrValue::I64(7));
        b.set_color(fid, [1.0, 0.0, 0.0]);
        b.set_name(fid, "top");
        b.set_density(fid, 2.7);
        b.set_user_field(fid, vec![9, 8, 7]);
        assert_eq!(b.attr(fid, "count"), Some(&AttrValue::I64(7)));
        assert_eq!(b.color(fid), Some([1.0, 0.0, 0.0]));
        assert_eq!(b.name(fid), Some("top"));
        assert_eq!(b.density(fid), Some(2.7));
        assert_eq!(b.user_field(fid), Some(&[9u8, 8, 7][..]));
        let mut keys = b.attr_keys(fid);
        keys.sort();
        assert_eq!(keys, vec!["color", "count", "density", "name", "user"]);
        assert_eq!(b.remove_attr(fid, "count"), Some(AttrValue::I64(7)));
        assert!(b.attr(fid, "count").is_none());
    }

    #[test]
    fn attributes_propagate_on_split() {
        // Item 121: a split edge's children inherit the parent's
        // attributes through the lineage register hook.
        let mut b = Body::new();
        let out = b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let ek = out.edges[0];
        let eid = b.edge_id(ek).unwrap();
        b.set_color(eid, [0.0, 1.0, 0.0]);
        b.set_name(eid, "seam");
        let (v0, v1) = b.edges.get(ek).unwrap().bounds;
        let mid = (b.vertices.get(v0).unwrap().point + b.vertices.get(v1).unwrap().point) * 0.5;
        let sp = b.split_edge(ek, mid).unwrap();
        for child in [sp.edge_a, sp.edge_b] {
            let cid = b.edge_id(child).unwrap();
            assert_eq!(
                b.color(cid),
                Some([0.0, 1.0, 0.0]),
                "split child inherits color"
            );
            assert_eq!(b.name(cid), Some("seam"), "split child inherits name");
        }
    }

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
