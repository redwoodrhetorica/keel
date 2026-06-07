#![no_main]
use keel_topo::Body;
use keel_topo::entity::{AnyKey, EdgeKey, FaceKey, FinKey, LoopKey};
use keel_topo::euler::MevSite;
use libfuzzer_sys::fuzz_target;

// Interpret fuzzer bytes as an operator program. After every applied
// operation the body must validate; a FAILED operation must not
// mutate (checked via topology hash). This fuzzes the operator core
// the way the curve/surface targets fuzz constructors.
fuzz_target!(|program: Vec<u8>| {
    if program.len() > 96 {
        return;
    }
    let mut b = Body::new();
    let r = b.infinite_region();
    let Ok(_) = b.mvfs(r, keel_math::vec::Vec3::ZERO) else {
        return;
    };
    let mut step = 0u64;
    for chunk in program.chunks(2) {
        let byte = chunk[0];
        let sel = *chunk.get(1).unwrap_or(&0) as usize;
        step += 1;
        let p = keel_math::vec::Vec3::new(step as f64, (byte % 11) as f64, (byte % 5) as f64);
        let fins: Vec<FinKey> = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(AnyKey::Fin(k)) => Some(k),
                _ => None,
            })
            .collect();
        let loops: Vec<LoopKey> = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(AnyKey::Loop(k)) => Some(k),
                _ => None,
            })
            .collect();
        let edges: Vec<EdgeKey> = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(AnyKey::Edge(k)) => Some(k),
                _ => None,
            })
            .collect();
        let faces: Vec<FaceKey> = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(AnyKey::Face(k)) => Some(k),
                _ => None,
            })
            .collect();
        let before = b.topology_hash();
        let failed = match byte % 8 {
            0 => {
                let vl = loops
                    .iter()
                    .find(|&&lk| b.loop_(lk).is_some_and(|l| l.vertex.is_some()));
                match vl {
                    Some(&lk) => b.mev(MevSite::VertexLoop(lk), p).is_err(),
                    None if !fins.is_empty() => b
                        .mev(MevSite::AfterFin(fins[sel % fins.len()]), p)
                        .is_err(),
                    None => false,
                }
            }
            1 if !fins.is_empty() => {
                let fa = fins[sel % fins.len()];
                let fb = fins[(sel / 2 + byte as usize) % fins.len()];
                b.mef(fa, fb, None).is_err()
            }
            2 if !edges.is_empty() => b.kev(edges[sel % edges.len()]).is_err(),
            3 if !edges.is_empty() => b.kef(edges[sel % edges.len()]).is_err(),
            4 if !fins.is_empty() => b.kemr(fins[sel % fins.len()]).is_err(),
            5 if faces.len() >= 2 => {
                let fk = faces[sel % faces.len()];
                let fp = faces[(sel + 1) % faces.len()];
                b.kfmrh(fk, fp).is_err()
            }
            6 if !edges.is_empty() => b.split_edge(edges[sel % edges.len()], p).is_err(),
            7 if !loops.is_empty() => {
                let lk = loops[sel % loops.len()];
                if b.loop_(lk).is_some_and(|l| l.vertex.is_some()) {
                    b.mef_on_vertex_loop(lk, None).is_err()
                } else {
                    false
                }
            }
            _ => false,
        };
        if failed {
            // Atomicity: a failed operation must not mutate.
            assert_eq!(b.topology_hash(), before, "failed op mutated the body");
        }
        assert!(b.validate().is_ok(), "invalid after step {step}");
    }
});
