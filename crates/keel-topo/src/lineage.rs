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
    Created,
    Modified { from: EntityId },
    Generated { from: EntityId },
    SplitChild { from: EntityId, ordinal: u32 },
    MergeResult { from: Vec<EntityId> },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Lineage {
    pub op: OpId,
    pub derivation: Derivation,
}

/// Total per-operation report: the OCCT Modified/Generated/Deleted
/// contract made native, plus first-class split/merge events.
#[derive(Clone, Debug, Default)]
pub struct OpReport {
    pub op: OpId,
    pub created: Vec<EntityId>,
    pub deleted: Vec<EntityId>,
    pub modified: Vec<(EntityId, EntityId)>,
    pub generated: Vec<(EntityId, EntityId)>,
    pub split: Vec<(EntityId, Vec<EntityId>)>,
    pub merged: Vec<(Vec<EntityId>, EntityId)>,
}

impl OpReport {
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
