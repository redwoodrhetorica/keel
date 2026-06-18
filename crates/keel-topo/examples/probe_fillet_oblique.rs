//! Reproduce the pre-existing fillet PANIC on oblique compound bodies (the
//! never-crash gap the dossier-70 agent surfaced: a Single-cap dissolve whose
//! mid-surgery debug_validate fails BoundaryChainBroken in DEBUG builds; release
//! declines via the final validate). Randomized stress sweep (the agent's
//! approach): random tilted peg (cylinder or prism box) unioned into a block,
//! every edge filleted under catch_unwind, panics counted and the first
//! panicking (seed, edge) reported.
//!   cargo run -p keel-topo --example probe_fillet_oblique -- [trials] [seed]   # DEBUG: asserts fire
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::{AnyKey, EdgeKey};
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
}

fn edge_keys(b: &Body) -> Vec<EdgeKey> {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Edge(k)) => Some(k),
            _ => None,
        })
        .collect()
}
fn edge_len(b: &Body, e: EdgeKey) -> f64 {
    match b.edge(e) {
        Some(ed) => {
            let (va, vb) = ed.bounds;
            match (b.vertex(va), b.vertex(vb)) {
                (Some(a), Some(c)) => (c.point - a.point).norm(),
                _ => 0.0,
            }
        }
        None => 0.0,
    }
}

/// Build one random oblique compound body (block + tilted peg union), or None.
fn build(rng: &mut Lcg) -> Option<Body> {
    let mut block = Body::new();
    block
        .block(Vec3::new(-20.0, -20.0, -20.0), 40.0, 40.0, 40.0)
        .ok()?;
    // Tilted axis: random direction off +z, tilt up to ~75 degrees.
    let th = rng.range(0.0, 1.3);
    let phi = rng.range(0.0, std::f64::consts::TAU);
    let axis = Vec3::new(th.sin() * phi.cos(), th.sin() * phi.sin(), th.cos());
    // Base offset so the peg crosses a top edge/corner region (3+-face seam ends).
    let ox = rng.range(-16.0, 16.0);
    let oy = rng.range(-16.0, 16.0);
    let base = Vec3::new(ox, oy, rng.range(-8.0, 8.0));
    let f = Frame3::from_z(base, axis).ok()?;
    let mut peg = Body::new();
    if rng.f() < 0.5 {
        peg.cylinder(f.clone(), rng.range(3.0, 9.0), 60.0).ok()?;
    } else {
        // Tilted box prism: square cross-section in the frame's x-y, extruded z.
        let w = rng.range(3.0, 9.0);
        let (u, v, n) = (f.x, f.y, f.z);
        let c = f.origin;
        let prof = vec![
            c + u * w + v * w,
            c - u * w + v * w,
            c - u * w - v * w,
            c + u * w - v * w,
        ];
        peg.prism(&prof, n * 60.0).ok()?;
    }
    boolean(&block, &peg, BoolOp::Union, 1e-7)
        .ok()
        .map(|r| r.body)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let trials: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(400);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    std::panic::set_hook(Box::new(|_| {}));
    let (mut built, mut declined, mut panics) = (0u64, 0u64, 0u64);
    let mut first_panic: Option<(u64, usize)> = None;
    for t in 0..trials {
        let s = seed.wrapping_mul(0x100000001B3).wrapping_add(t);
        let mut rng = Lcg(0x2545F4914F6CDD1D ^ s.wrapping_mul(0x9E3779B97F4A7C15));
        let Some(body) = build(&mut rng) else {
            continue;
        };
        for (i, &e) in edge_keys(&body).iter().enumerate() {
            if edge_len(&body, e) <= 1e-6 {
                continue;
            }
            let r = edge_len(&body, e) * rng.range(0.02, 0.08);
            match catch_unwind(AssertUnwindSafe(|| body.fillet_edge(e, r))) {
                Err(_) => {
                    panics += 1;
                    if first_panic.is_none() {
                        first_panic = Some((s, i));
                    }
                }
                Ok(Ok(_)) => built += 1,
                Ok(Err(_)) => declined += 1,
            }
        }
    }
    let _ = std::panic::take_hook();
    println!("{trials} trials: built {built} / declined {declined} / PANICKED {panics}");
    match first_panic {
        Some((s, i)) => println!("REPRODUCED: first panic at body-seed {s}, edge idx {i}"),
        None => println!("no panic reproduced in this sweep"),
    }
}
