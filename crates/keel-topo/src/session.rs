//! Session hooks (gate design 3.4; kernel/07 mandates 9-13 seed):
//! snapshot/restore and the operation journal with deterministic
//! replay. M3 snapshots are deep clones (correct first); the API
//! admits a COW/persistent-structure upgrade without signature
//! change. Full Parasolid-style partitions/deltas come later.

use crate::body::{Body, TopoError};
use crate::entity::{AnyKey, EntityId, FinKey, LoopKey};
use crate::euler::MevSite;
use keel_math::vec::Vec3;

/// An immutable saved body state.
#[derive(Clone, Debug)]
pub struct Snapshot(Body);

impl Body {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot(self.clone())
    }

    pub fn restore(snapshot: Snapshot) -> Body {
        snapshot.0
    }

    /// Serialize the whole body to a deterministic JSON document (parity
    /// item 126, persistent save/restore). serde_json round-trips f64
    /// EXACTLY (ryu shortest-round-trip), and the generational arena keys
    /// and generations serialize verbatim, so every topology reference
    /// stays valid across a round-trip -- no key remapping. The document
    /// is self-describing and stable for a given body (deterministic field
    /// and entity order).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Restore a body from `to_json` output (parity item 126). Returns a
    /// serde error for a malformed document; callers may `validate()` the
    /// result (a faithful round-trip of a valid body is itself valid).
    pub fn from_json(s: &str) -> Result<Body, serde_json::Error> {
        serde_json::from_str(s)
    }
}

/// Site addressing by VALUE (EntityIds): keys are transient, ids are
/// the durable names, so journals survive serialization and replay on
/// a fresh body.
#[derive(Clone, Debug, PartialEq)]
pub enum SiteDescriptor {
    VertexLoop(EntityId),
    AfterFin(EntityId),
}

/// Recorded operation descriptor, sufficient for replay.
#[derive(Clone, Debug, PartialEq)]
pub enum OpDescriptor {
    Mvfs {
        region: EntityId,
        p: [f64; 3],
    },
    Mev {
        site: SiteDescriptor,
        p: [f64; 3],
    },
    Mef {
        fin_a: EntityId,
        fin_b: EntityId,
    },
    MefOnVertexLoop {
        lp: EntityId,
    },
    Kev {
        edge: EntityId,
    },
    Kef {
        edge: EntityId,
    },
    Kemr {
        fin: EntityId,
    },
    Kfmrh {
        face_kill: EntityId,
        face_keep: EntityId,
    },
    SplitEdge {
        edge: EntityId,
        p: [f64; 3],
    },
}

/// Journal sink; the body records every replayable operation.
pub trait OpJournal {
    fn record(&mut self, op: &OpDescriptor);
}

/// The obvious in-memory journal.
#[derive(Default)]
pub struct VecJournal(pub Vec<OpDescriptor>);

impl OpJournal for VecJournal {
    fn record(&mut self, op: &OpDescriptor) {
        self.0.push(op.clone());
    }
}

/// Replay a journal on a fresh body. Because EntityId assignment is
/// deterministic, ids recorded at journal time resolve identically at
/// replay time; the result must hash-match the original (the M3
/// determinism proof, kernel/07 mandate 13).
pub fn replay(journal: &[OpDescriptor]) -> Result<Body, TopoError> {
    let mut b = Body::new();
    for op in journal {
        apply(&mut b, op)?;
    }
    Ok(b)
}

fn apply(b: &mut Body, op: &OpDescriptor) -> Result<(), TopoError> {
    let v3 = |p: &[f64; 3]| Vec3::new(p[0], p[1], p[2]);
    let fin = |b: &Body, id: EntityId| -> Result<FinKey, TopoError> {
        match b.lookup(id) {
            Some(AnyKey::Fin(k)) => Ok(k),
            _ => Err(TopoError::StaleKey),
        }
    };
    let lp = |b: &Body, id: EntityId| -> Result<LoopKey, TopoError> {
        match b.lookup(id) {
            Some(AnyKey::Loop(k)) => Ok(k),
            _ => Err(TopoError::StaleKey),
        }
    };
    match op {
        OpDescriptor::Mvfs { region, p } => {
            let r = match b.lookup(*region) {
                Some(AnyKey::Region(k)) => k,
                _ => return Err(TopoError::StaleKey),
            };
            b.mvfs(r, v3(p)).map(|_| ())
        }
        OpDescriptor::Mev { site, p } => {
            let site = match site {
                SiteDescriptor::VertexLoop(id) => MevSite::VertexLoop(lp(b, *id)?),
                SiteDescriptor::AfterFin(id) => MevSite::AfterFin(fin(b, *id)?),
            };
            b.mev(site, v3(p)).map(|_| ())
        }
        OpDescriptor::Mef { fin_a, fin_b } => {
            let (fa, fb) = (fin(b, *fin_a)?, fin(b, *fin_b)?);
            b.mef(fa, fb, None).map(|_| ())
        }
        OpDescriptor::MefOnVertexLoop { lp: lid } => {
            let l = lp(b, *lid)?;
            b.mef_on_vertex_loop(l, None).map(|_| ())
        }
        OpDescriptor::Kev { edge } => match b.lookup(*edge) {
            Some(AnyKey::Edge(k)) => b.kev(k).map(|_| ()),
            _ => Err(TopoError::StaleKey),
        },
        OpDescriptor::Kef { edge } => match b.lookup(*edge) {
            Some(AnyKey::Edge(k)) => b.kef(k).map(|_| ()),
            _ => Err(TopoError::StaleKey),
        },
        OpDescriptor::Kemr { fin: fid } => {
            let f = fin(b, *fid)?;
            b.kemr(f).map(|_| ())
        }
        OpDescriptor::Kfmrh {
            face_kill,
            face_keep,
        } => {
            let (fk, fp) = match (b.lookup(*face_kill), b.lookup(*face_keep)) {
                (Some(AnyKey::Face(a)), Some(AnyKey::Face(c))) => (a, c),
                _ => return Err(TopoError::StaleKey),
            };
            b.kfmrh(fk, fp).map(|_| ())
        }
        OpDescriptor::SplitEdge { edge, p } => match b.lookup(*edge) {
            Some(AnyKey::Edge(k)) => b.split_edge(k, v3(p)).map(|_| ()),
            _ => Err(TopoError::StaleKey),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::AnyKey;

    /// Drive the cube construction while writing a journal by hand
    /// (the constructor-integrated journaling lands with Task 8's
    /// public constructors; this test proves the replay machinery).
    #[test]
    fn journal_replay_reproduces_topology_hash() {
        let mut journal = VecJournal::default();
        let mut b = Body::new();
        let r = b.infinite_region();
        let r_id = b.region(r).map(|x| x.id).unwrap_or_else(|| panic!());
        journal.record(&OpDescriptor::Mvfs {
            region: r_id,
            p: [0., 0., 0.],
        });
        let seed = b.mvfs(r, Vec3::ZERO).unwrap_or_else(|e| panic!("{e:?}"));
        let lp = b
            .face(seed.face)
            .map(|f| f.loops[0])
            .unwrap_or_else(|| panic!());
        let lp_id = b.loop_(lp).map(|l| l.id).unwrap_or_else(|| panic!());
        journal.record(&OpDescriptor::Mev {
            site: SiteDescriptor::VertexLoop(lp_id),
            p: [1., 0., 0.],
        });
        b.mev(MevSite::VertexLoop(lp), Vec3::new(1., 0., 0.))
            .unwrap_or_else(|e| panic!("{e:?}"));
        // Close a balloon over it via the closed-edge mef from a fin.
        let f = b.loop_(lp).and_then(|l| l.fin).unwrap_or_else(|| panic!());
        let f_id = b.fin(f).map(|x| x.id).unwrap_or_else(|| panic!());
        journal.record(&OpDescriptor::Mef {
            fin_a: f_id,
            fin_b: f_id,
        });
        b.mef(f, f, None).unwrap_or_else(|e| panic!("{e:?}"));
        // Split the closed edge made by the mef.
        let closed_edge = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(AnyKey::Edge(k)) => b.edge(k).and_then(|e| e.is_closed().then_some((id, k))),
                _ => None,
            })
            .next()
            .unwrap_or_else(|| panic!("no closed edge"));
        journal.record(&OpDescriptor::SplitEdge {
            edge: closed_edge.0,
            p: [0.5, 0.5, 0.],
        });
        b.split_edge(closed_edge.1, Vec3::new(0.5, 0.5, 0.))
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert!(b.validate().is_ok());

        let replayed = replay(&journal.0).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(b.topology_hash(), replayed.topology_hash());
    }

    #[test]
    fn snapshot_restore_round_trips_hash() {
        let (mut b, _) = crate::euler::test_support::cube();
        let snap = b.snapshot();
        let h = b.topology_hash();
        // Mutate: split an edge.
        let e = b
            .entity_ids()
            .find_map(|id| match b.lookup(id) {
                Some(AnyKey::Edge(k)) => Some(k),
                _ => None,
            })
            .unwrap_or_else(|| panic!());
        b.split_edge(e, Vec3::new(0.5, 0., 0.))
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_ne!(b.topology_hash(), h);
        let restored = Body::restore(snap);
        assert_eq!(restored.topology_hash(), h);
        assert!(restored.validate().is_ok());
    }

    #[test]
    fn json_save_restore_round_trips_exactly() {
        use keel_geom::surface::Frame3;
        // Planar body (block) + curved body (cylinder): both serialize to
        // JSON and restore to a valid body with identical topology hash and
        // bit-exact geometry (mass_properties matches).
        let mut block = Body::new();
        block
            .block(Vec3::new(1.0, 2.0, 3.0), 2.0, 3.0, 4.0)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let mut cyl = Body::new();
        cyl.cylinder(
            Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
            1.5,
            3.0,
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        for b in [&block, &cyl] {
            let json = b.to_json().unwrap_or_else(|e| panic!("{e:?}"));
            let r = Body::from_json(&json).unwrap_or_else(|e| panic!("{e:?}"));
            assert!(r.validate().is_ok(), "restored body invalid");
            assert_eq!(
                b.topology_hash(),
                r.topology_hash(),
                "topology hash differs"
            );
            assert_eq!(b.counts(), r.counts(), "counts differ");
            let v0 = b
                .mass_properties()
                .unwrap_or_else(|e| panic!("{e:?}"))
                .volume;
            let v1 = r
                .mass_properties()
                .unwrap_or_else(|e| panic!("{e:?}"))
                .volume;
            assert_eq!(v0.to_bits(), v1.to_bits(), "volume not bit-exact");
        }
        // The block's volume is the known 24 (sanity that geometry survived).
        assert!((block.mass_properties().unwrap().volume - 24.0).abs() < 1e-12);
    }
}
