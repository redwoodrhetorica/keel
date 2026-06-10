//! Partitions, pmarks/rollback, and transactions (parity items 123, 124,
//! 125). Research: dossier 14 (determinism + exact serialization).
//!
//! A PARTITION (123) is the top-level rollback container: a set of bodies
//! that are snapshotted, serialized, and rolled back as ONE unit (Parasolid's
//! partition is the undo unit holding bodies). PMARKS (124) are named
//! rollback points in the partition's history; `roll_to(name)` reverts (or
//! advances) every body in the partition to its state when that mark was set
//! -- both rollback and rollforward, since marks are kept. TRANSACTIONS (125)
//! group operations atomically: `begin` saves the state, `abort` reverts to
//! it, `commit` accepts it (nestable via a stack).
//!
//! The three share one substrate (a body-state snapshot) but are distinct
//! user-facing capabilities -- a rollback container, navigable history marks,
//! and atomic op grouping -- exactly as the Parasolid map lists them. The
//! partition serializes with exact f64 round-trip via the body serde
//! (dossier 14); incremental DELTA save (item 127) is a follow-on.

use crate::Body;

/// Index of a body within a [`Partition`].
pub type BodyId = usize;

/// A named rollback point: the partition's body state when it was set.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Pmark {
    name: String,
    bodies: Vec<Body>,
}

/// The top-level rollback container (item 123): a set of bodies with named
/// rollback marks (124) and atomic transactions (125).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Partition {
    bodies: Vec<Body>,
    marks: Vec<Pmark>,
    /// Open-transaction saved states (a stack; not serialized -- a persisted
    /// partition has no open transactions).
    #[serde(skip)]
    txns: Vec<Vec<Body>>,
}

impl Partition {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a body; returns its id.
    pub fn add(&mut self, body: Body) -> BodyId {
        self.bodies.push(body);
        self.bodies.len() - 1
    }

    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }
    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.get(id)
    }
    pub fn body_mut(&mut self, id: BodyId) -> Option<&mut Body> {
        self.bodies.get_mut(id)
    }
    pub fn len(&self) -> usize {
        self.bodies.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    // ---- pmarks / rollback (item 124) -----------------------------------

    /// Set a named rollback mark capturing the current body state.
    pub fn set_pmark(&mut self, name: &str) {
        self.marks.push(Pmark {
            name: name.to_string(),
            bodies: self.bodies.clone(),
        });
    }

    /// Roll the whole partition to the named mark: every body reverts to (or
    /// advances to) its state when that mark was set. Marks are retained, so
    /// this is both rollback and rollforward. Returns false if unknown.
    pub fn roll_to(&mut self, name: &str) -> bool {
        if let Some(m) = self.marks.iter().rev().find(|m| m.name == name) {
            self.bodies = m.bodies.clone();
            true
        } else {
            false
        }
    }

    /// The names of the rollback marks, oldest first.
    pub fn pmarks(&self) -> Vec<&str> {
        self.marks.iter().map(|m| m.name.as_str()).collect()
    }

    // ---- transactions (item 125) ----------------------------------------

    /// Begin an atomic transaction: save the current state (nestable).
    pub fn begin(&mut self) {
        self.txns.push(self.bodies.clone());
    }

    /// Abort the innermost open transaction: revert to its begin state.
    /// Returns false if no transaction is open.
    pub fn abort(&mut self) -> bool {
        if let Some(saved) = self.txns.pop() {
            self.bodies = saved;
            true
        } else {
            false
        }
    }

    /// Commit the innermost open transaction: accept the current state.
    /// Returns false if no transaction is open.
    pub fn commit(&mut self) -> bool {
        self.txns.pop().is_some()
    }

    pub fn open_transactions(&self) -> usize {
        self.txns.len()
    }

    // ---- serialization (the partition as a unit, dossier 14) ------------

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn from_json(s: &str) -> Result<Partition, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_math::vec::Vec3;

    fn box_of(s: f64) -> Body {
        let mut b = Body::new();
        b.block(Vec3::ZERO, s, s, s).unwrap();
        b
    }

    fn vol(p: &Partition, id: BodyId) -> f64 {
        p.body(id).unwrap().mass_properties().unwrap().volume
    }

    #[test]
    fn partition_holds_bodies_and_serializes() {
        // item 123: a partition is a container of bodies, serialized as a
        // unit with exact round-trip.
        let mut p = Partition::new();
        let a = p.add(box_of(2.0)); // vol 8
        let b = p.add(box_of(3.0)); // vol 27
        assert_eq!(p.len(), 2);
        let json = p.to_json().unwrap();
        let q = Partition::from_json(&json).unwrap();
        assert_eq!(q.len(), 2);
        assert!((vol(&q, a) - 8.0).abs() < 1e-12);
        assert!((vol(&q, b) - 27.0).abs() < 1e-12);
    }

    #[test]
    fn pmark_rolls_the_partition_back() {
        // item 124: a named rollback point reverts every body.
        let mut p = Partition::new();
        let id = p.add(box_of(2.0)); // vol 8
        p.set_pmark("start");
        // Edit: replace the body with a chamfered (smaller) one.
        *p.body_mut(id).unwrap() = box_of(4.0); // vol 64
        assert!((vol(&p, id) - 64.0).abs() < 1e-9);
        assert!(p.roll_to("start"), "mark must exist");
        assert!((vol(&p, id) - 8.0).abs() < 1e-9, "roll_to must revert");
    }

    #[test]
    fn transaction_abort_reverts_commit_keeps() {
        // item 125: begin/abort reverts; begin/commit keeps.
        let mut p = Partition::new();
        let id = p.add(box_of(2.0)); // vol 8
        p.begin();
        *p.body_mut(id).unwrap() = box_of(4.0); // vol 64
        assert!(p.abort());
        assert!((vol(&p, id) - 8.0).abs() < 1e-9, "abort must revert to 8");
        p.begin();
        *p.body_mut(id).unwrap() = box_of(3.0); // vol 27
        assert!(p.commit());
        assert!((vol(&p, id) - 27.0).abs() < 1e-9, "commit must keep 27");
        assert_eq!(p.open_transactions(), 0);
    }
}
