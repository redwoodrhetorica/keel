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

    /// Save at a chosen FORMAT VERSION (item 128, read-old-write-target).
    /// Version 1 is the bare item-126 body document (so older readers
    /// keep working); version 2 wraps it in a self-describing envelope.
    /// An unsupported target is an honest error, never a silent fallback.
    pub fn save_versioned(&self, target_version: u32) -> Result<String, SaveError> {
        match target_version {
            1 => self.to_json().map_err(SaveError::Serde),
            2 => {
                let doc = VersionedDoc {
                    keel_save_version: 2,
                    body: self.clone(),
                };
                serde_json::to_string(&doc).map_err(SaveError::Serde)
            }
            v => Err(SaveError::UnsupportedVersion(v)),
        }
    }

    /// Load any supported save version (item 128): a version-2 envelope,
    /// or a bare version-1 document (everything item 126 ever wrote).
    /// A document stamped with a NEWER version than this build supports
    /// errs honestly instead of misreading it.
    pub fn load_versioned(s: &str) -> Result<Body, SaveError> {
        if let Ok(doc) = serde_json::from_str::<VersionedDoc>(s) {
            if doc.keel_save_version > SAVE_FORMAT_VERSION {
                return Err(SaveError::UnsupportedVersion(doc.keel_save_version));
            }
            return Ok(doc.body);
        }
        Body::from_json(s).map_err(SaveError::Serde)
    }
}

/// Newest save format this build writes (item 128).
pub const SAVE_FORMAT_VERSION: u32 = 2;

/// The version-2 save envelope: a version stamp plus the body document.
#[derive(serde::Serialize, serde::Deserialize)]
struct VersionedDoc {
    keel_save_version: u32,
    body: Body,
}

/// Versioned save/load errors (item 128).
#[derive(Debug)]
pub enum SaveError {
    Serde(serde_json::Error),
    UnsupportedVersion(u32),
}

/// Session precision configuration (item 113): the session-level knobs
/// that default every tolerance-taking operation run through a
/// [`Session`]. Per-entity local tolerances (tolerant edges, 110-112)
/// override these locally; this is the SESSION layer of the precision
/// stack.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionConfig {
    /// Default linear tolerance for booleans/imprints/knits.
    pub linear_tolerance: f64,
    /// Default angular tolerance (radians) for tangency/parallel tests.
    pub angular_tolerance: f64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            linear_tolerance: 1e-7,
            angular_tolerance: 1e-9,
        }
    }
}

/// A modeling session (item 122): the start/stop/configure container
/// that owns partitions (item 123) and the session precision config
/// (item 113). Operations run through the session pick up its
/// configured tolerances; `stop` hands the partitions back for
/// persistence.
#[derive(Clone, Debug, Default)]
pub struct Session {
    config: SessionConfig,
    partitions: Vec<crate::partition::Partition>,
}

impl Session {
    /// Start a session with the given configuration.
    pub fn start(config: SessionConfig) -> Self {
        Self {
            config,
            partitions: Vec::new(),
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Reconfigure the running session (item 122 "configure"); rejects
    /// non-finite or non-positive tolerances.
    pub fn configure(&mut self, config: SessionConfig) -> Result<(), TopoError> {
        let ok = config.linear_tolerance.is_finite()
            && config.linear_tolerance > 0.0
            && config.angular_tolerance.is_finite()
            && config.angular_tolerance > 0.0;
        if !ok {
            return Err(TopoError::Precondition("configure: bad tolerances"));
        }
        self.config = config;
        Ok(())
    }

    /// Create a new (empty) partition owned by this session.
    pub fn new_partition(&mut self) -> usize {
        self.partitions.push(crate::partition::Partition::new());
        self.partitions.len() - 1
    }

    pub fn partition(&self, id: usize) -> Option<&crate::partition::Partition> {
        self.partitions.get(id)
    }

    pub fn partition_mut(&mut self, id: usize) -> Option<&mut crate::partition::Partition> {
        self.partitions.get_mut(id)
    }

    /// Boolean through the session: the session's configured linear
    /// tolerance is the boolean tolerance (item 113 in action).
    pub fn boolean(
        &self,
        a: &Body,
        b: &Body,
        op: crate::boolean::BoolOp,
    ) -> Result<crate::boolean::BoolResult, crate::boolean::BoolFault> {
        crate::boolean::boolean(a, b, op, self.config.linear_tolerance)
    }

    /// Stop the session, handing back its partitions for persistence.
    pub fn stop(self) -> Vec<crate::partition::Partition> {
        self.partitions
    }
}

/// Site addressing by VALUE (EntityIds): keys are transient, ids are
/// the durable names, so journals survive serialization and replay on
/// a fresh body.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SiteDescriptor {
    VertexLoop(EntityId),
    AfterFin(EntityId),
}

/// Recorded operation descriptor, sufficient for replay.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Serialize an operation journal to a deterministic JSON document
/// (parity item 129, persistent journaling). The journal addresses
/// entities by durable `EntityId` and its f64 point params round-trip
/// EXACTLY (serde_json/ryu), so `load_journal` + `replay` on a fresh
/// body reproduces the original topology hash.
pub fn save_journal(journal: &[OpDescriptor]) -> Result<String, serde_json::Error> {
    serde_json::to_string(journal)
}

/// Parse an operation journal from `save_journal` output (parity item 129).
pub fn load_journal(s: &str) -> Result<Vec<OpDescriptor>, serde_json::Error> {
    serde_json::from_str(s)
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

    #[test]
    fn session_lifecycle_and_precision() {
        // Items 122 + 113: start/configure/stop, with the configured
        // linear tolerance flowing into a session-run boolean.
        use crate::boolean::BoolOp;
        let mut s = Session::start(SessionConfig::default());
        assert_eq!(s.config().linear_tolerance, 1e-7);
        s.configure(SessionConfig {
            linear_tolerance: 1e-6,
            angular_tolerance: 1e-8,
        })
        .unwrap();
        assert!(
            s.configure(SessionConfig {
                linear_tolerance: f64::NAN,
                angular_tolerance: 1e-9,
            })
            .is_err(),
            "non-finite tolerance must be rejected"
        );
        let pid = s.new_partition();
        let mut a = Body::new();
        a.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(1.0, 0.5, 0.5), 2.0, 2.0, 2.0).unwrap();
        let res = s.boolean(&a, &b, BoolOp::Union).unwrap();
        let v = res.body.mass_properties().unwrap().volume;
        // Transversal corner overlap: 8 + 8 - 1*1.5*1.5 = 13.75.
        assert!(
            (v - 13.75).abs() < 1e-6,
            "session union volume {v} != 13.75"
        );
        s.partition_mut(pid).unwrap().add(res.body);
        let parts = s.stop();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].bodies().len(), 1);
    }

    #[test]
    fn versioned_save_reads_old_writes_target() {
        // Item 128: v1 (bare item-126 doc) and v2 (envelope) both load;
        // a v1 written today is readable by the OLD reader (from_json);
        // a FUTURE version errs honestly.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 1.0, 2.0, 3.0).unwrap();
        let want = b.mass_properties().unwrap().volume;

        let v1 = b.save_versioned(1).unwrap();
        let old_reader = Body::from_json(&v1).unwrap();
        assert!((old_reader.mass_properties().unwrap().volume - want).abs() < 1e-12);
        let from_v1 = Body::load_versioned(&v1).unwrap();
        assert!((from_v1.mass_properties().unwrap().volume - want).abs() < 1e-12);

        let v2 = b.save_versioned(2).unwrap();
        let from_v2 = Body::load_versioned(&v2).unwrap();
        assert!((from_v2.mass_properties().unwrap().volume - want).abs() < 1e-12);
        assert!(from_v2.validate().is_ok());

        assert!(matches!(
            b.save_versioned(99),
            Err(SaveError::UnsupportedVersion(99))
        ));
        let future = v2.replacen("\"keel_save_version\":2", "\"keel_save_version\":3", 1);
        assert!(matches!(
            Body::load_versioned(&future),
            Err(SaveError::UnsupportedVersion(3))
        ));
    }

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

    #[test]
    fn journal_serde_round_trips_and_replays() {
        // Record a short journal, serialize it to JSON, reload, and replay
        // on a fresh body (parity item 129). The reloaded journal equals
        // the original (exact, incl. f64 params) and replays to the same
        // topology hash as the directly-built body.
        let mut journal = VecJournal::default();
        let mut b = Body::new();
        let r = b.infinite_region();
        let r_id = b.region(r).map(|x| x.id).unwrap_or_else(|| panic!());
        journal.record(&OpDescriptor::Mvfs {
            region: r_id,
            p: [0.5, -1.0, 2.0],
        });
        let seed = b
            .mvfs(r, Vec3::new(0.5, -1.0, 2.0))
            .unwrap_or_else(|e| panic!("{e:?}"));
        let lp = b
            .face(seed.face)
            .map(|f| f.loops[0])
            .unwrap_or_else(|| panic!());
        let lp_id = b.loop_(lp).map(|l| l.id).unwrap_or_else(|| panic!());
        journal.record(&OpDescriptor::Mev {
            site: SiteDescriptor::VertexLoop(lp_id),
            p: [1.5, -1.0, 2.0],
        });
        b.mev(MevSite::VertexLoop(lp), Vec3::new(1.5, -1.0, 2.0))
            .unwrap_or_else(|e| panic!("{e:?}"));

        let json = save_journal(&journal.0).unwrap_or_else(|e| panic!("{e:?}"));
        let j2 = load_journal(&json).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(journal.0, j2, "journal serde round-trip not exact");
        let replayed = replay(&j2).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(
            b.topology_hash(),
            replayed.topology_hash(),
            "replayed-from-JSON topology hash differs"
        );
    }
}
