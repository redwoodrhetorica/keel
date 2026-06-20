//! Lineage and operation reporting (gate design section 3; spec D9).
//! Every public mutation produces exactly one OpReport; no operation
//! may mint anonymous topology.

use crate::entity::EntityId;

/// Monotonic per-body operation identity.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct OpId(pub u64);

/// How an entity came to exist (the naming vocabulary shared by
/// Kripac, FreeCAD MappedNames, OnShape qCreatedBy, Cascaval lineage).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Derivation {
    /// Created fresh by its operation (no parent).
    Created,
    /// A modified continuation of one prior entity.
    Modified {
        /// The prior entity this one continues.
        from: EntityId,
    },
    /// Spawned by, but distinct from, a prior entity (e.g. a fin off a
    /// face); does not inherit attributes.
    Generated {
        /// The entity this one was generated from.
        from: EntityId,
    },
    /// One piece of a split entity.
    SplitChild {
        /// The entity that was split.
        from: EntityId,
        /// This child's index among the split pieces.
        ordinal: u32,
    },
    /// The result of merging several prior entities.
    MergeResult {
        /// The entities that were merged (primary first).
        from: Vec<EntityId>,
    },
}

/// The lineage of one entity: the operation that produced it and how.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Lineage {
    /// The operation that produced the entity.
    pub op: OpId,
    /// How the entity was derived.
    pub derivation: Derivation,
}

/// Total per-operation report of what changed: the OCCT
/// Modified/Generated/Deleted contract made native, plus first-class
/// split/merge events. Returned (directly or in
/// [`PrimitiveOut`](crate::construct::PrimitiveOut)) by mutating
/// operations so a consumer can track downstream selections.
#[derive(Clone, Debug, Default)]
pub struct OpReport {
    /// The operation this report describes.
    pub op: OpId,
    /// Entities created fresh by the operation.
    pub created: Vec<EntityId>,
    /// Entities deleted by the operation.
    pub deleted: Vec<EntityId>,
    /// (old, new) pairs for entities modified in place.
    pub modified: Vec<(EntityId, EntityId)>,
    /// (source, spawned) pairs for generated entities.
    pub generated: Vec<(EntityId, EntityId)>,
    /// (parent, children) for entities that were split.
    pub split: Vec<(EntityId, Vec<EntityId>)>,
    /// (sources, result) for entities produced by a merge.
    pub merged: Vec<(Vec<EntityId>, EntityId)>,
}

impl OpReport {
    /// An empty report for operation `op`.
    pub fn new(op: OpId) -> Self {
        Self {
            op,
            ..Default::default()
        }
    }

    /// Canonical order for deterministic output and tests.
    pub fn sort(&mut self) {
        self.created.sort_unstable();
        self.deleted.sort_unstable();
        self.modified.sort_unstable();
        self.generated.sort_unstable();
        self.split.sort_unstable_by_key(|(p, _)| *p);
        self.merged.sort_unstable_by_key(|(_, c)| *c);
    }
}
