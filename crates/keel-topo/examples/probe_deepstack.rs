//! Direct repro for the deep-chain stack-overflow (recursion hunt).
//!
//! Grows a single solid through a long chain of transversal boolean tools
//! (the realsoak "growing solid" pattern: a peg crossing the body, offset to
//! protrude, general position), then exercises each candidate kernel op inside
//! a SMALL-stack worker thread to isolate WHICH routine recurses with body
//! complexity and overflows. A stack overflow ABORTS (not catch_unwind-able),
//! so each candidate runs in its own thread: the one that does not return is
//! the culprit.
//!
//! Run: cargo run --release -p keel-topo --example probe_deepstack -- [stack_kib] [chain]
#![allow(clippy::unwrap_used)]
// `step % 2 == 0` reads clearer than .is_multiple_of in this geometric probe.
#![allow(clippy::manual_is_multiple_of)]
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}

/// One transversal box peg crossing the body along an axis, protruding one
/// end, with a small lateral jitter -- general position so each op cleanly
/// imprints + assembles, growing the face/edge count step by step.
fn peg(bb_min: Vec3, bb_max: Vec3, step: usize) -> (Body, BoolOp) {
    let size = bb_max - bb_min;
    let center = (bb_min + bb_max) * 0.5;
    let ax = step % 3;
    let cf_a = 0.45 + 0.1 * ((step as f64 * 0.37).sin() * 0.5 + 0.5);
    let cf_b = 0.45 + 0.1 * ((step as f64 * 0.61).cos() * 0.5 + 0.5);
    let (e0, e1, e2) = (size.x.max(1.0), size.y.max(1.0), size.z.max(1.0));
    let axext = [e0, e1, e2][ax];
    let len_ax = axext * 1.6;
    let protrude = axext * 0.4;
    let jit = |ext: f64, cf: f64, k: f64| (ext * (1.0 - cf) * 0.5) * 0.3 * k;
    let (mut offset, mut lat) = (Vec3::ZERO, Vec3::ZERO);
    let s = (step as f64 * 0.9).sin();
    match ax {
        0 => {
            offset.x = protrude;
            lat.y = jit(e1, cf_a, s);
            lat.z = jit(e2, cf_b, -s);
        }
        1 => {
            offset.y = protrude;
            lat.x = jit(e0, cf_a, s);
            lat.z = jit(e2, cf_b, -s);
        }
        _ => {
            offset.z = protrude;
            lat.x = jit(e0, cf_a, s);
            lat.y = jit(e1, cf_b, -s);
        }
    }
    let pcenter = center + offset + lat;
    let (px, py) = match ax {
        0 => (e1 * cf_a, e2 * cf_b),
        1 => (e0 * cf_a, e2 * cf_b),
        _ => (e0 * cf_a, e1 * cf_b),
    };
    let (dx, dy, dz) = match ax {
        0 => (len_ax, px, py),
        1 => (px, len_ax, py),
        _ => (px, py, len_ax),
    };
    let corner = pcenter - Vec3::new(dx, dy, dz) * 0.5;
    // Alternate union/difference so faces accumulate (a difference punches a
    // pocket -> more faces; a union adds a boss -> more faces).
    let op = if step % 2 == 0 {
        BoolOp::Union
    } else {
        BoolOp::Difference
    };
    (blk(corner, dx, dy, dz), op)
}

fn cyl(pos: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(
        keel_geom::surface::Frame3::from_z(pos, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        r,
        h,
    )
    .unwrap();
    b
}

/// Grow a HEAVY plate by punching/adding many small well-spaced features in a
/// grid, over as many rounds as needed to realize `chain` ops. Well-spaced
/// transversal features land ~100% (the probe_nested pattern), so the body's
/// face/edge count climbs reliably -- the "heavy grown body" that drives the
/// kernel's recursion deep.
fn grow(chain: usize) -> Body {
    // A large, thin plate gives a big top/bottom face to host many features.
    let span = 200.0;
    let mut body = blk(Vec3::new(-span * 0.5, -span * 0.5, -5.0), span, span, 10.0);
    let mut realized = 0usize;
    let mut idx = 0usize;
    // Grid of feature sites, 6 units apart, inset from the rim.
    let n = ((span - 12.0) / 6.0) as i64;
    let try_feat = |body: &Body, tool: &Body, op: BoolOp| -> Option<Body> {
        match boolean(body, tool, op, 1e-7) {
            Ok(r)
                if r.faults
                    .iter()
                    .all(|f| matches!(f, keel_topo::boolean::BoolFault::Coincident(..))) =>
            {
                if r.body.validate().is_ok() {
                    return Some(r.body);
                }
                None
            }
            _ => None,
        }
    };
    'outer: for round in 0..6 {
        for ix in 0..n {
            for iy in 0..n {
                if realized >= chain {
                    break 'outer;
                }
                let x = -span * 0.5 + 6.0 + ix as f64 * 6.0;
                let y = -span * 0.5 + 6.0 + iy as f64 * 6.0;
                idx += 1;
                // Vary feature kind by round+parity so faces accumulate in
                // several styles (through-hole, blind pocket, boss, box hole).
                let kind = (round + (ix + iy) as usize) % 4;
                let cand = match kind {
                    0 => (cyl(Vec3::new(x, y, -6.0), 0.9, 12.0), BoolOp::Difference),
                    1 => (cyl(Vec3::new(x, y, 4.0), 1.0, 4.0 + round as f64), BoolOp::Union),
                    2 => (blk(Vec3::new(x - 1.0, y - 1.0, 1.0 - round as f64 * 0.3), 2.0, 2.0, 6.0), BoolOp::Difference),
                    _ => (blk(Vec3::new(x - 1.2, y - 1.2, 4.5), 2.4, 2.4, 3.0), BoolOp::Union),
                };
                if let Some(nb) = try_feat(&body, &cand.0, cand.1) {
                    body = nb;
                    realized += 1;
                }
            }
        }
    }
    let _ = idx;
    let c = body.counts();
    eprintln!(
        "grown body: realized {realized}/{chain} ops  v={} e={} f={} g{}",
        c.v, c.e, c.f, c.genus
    );
    body
}

/// Grow a body via repeated MIRROR + union -- each mirror roughly DOUBLES the
/// face/edge count, so after k mirrors the body has O(2^k) faces. This is the
/// realsoak Mirror op, and it is the only realistic way a chain explodes a body
/// to a face count far beyond what transversal features reach -- the candidate
/// for driving a linear-in-faces recursion deep enough to overflow.
fn grow_mirror(rounds: usize) -> Body {
    // Seed: a small asymmetric notched block so mirroring actually adds faces.
    let mut body = blk(Vec3::new(0.0, 0.0, 0.0), 10.0, 8.0, 6.0);
    if let Some(nb) = {
        let hole = blk(Vec3::new(2.0, 2.0, -1.0), 2.0, 2.0, 8.0);
        match boolean(&body, &hole, BoolOp::Difference, 1e-7) {
            Ok(r) if r.body.validate().is_ok() => Some(r.body),
            _ => None,
        }
    } {
        body = nb;
    }
    for r in 0..rounds {
        let bb = body.bounding_box();
        // Mirror across the far face on a rotating axis so the copy is adjacent.
        let (pt, n) = match r % 3 {
            0 => (Vec3::new(bb.max.x, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            1 => (Vec3::new(0.0, bb.max.y, 0.0), Vec3::new(0.0, 1.0, 0.0)),
            _ => (Vec3::new(0.0, 0.0, bb.max.z), Vec3::new(0.0, 0.0, 1.0)),
        };
        let Ok(m) = body.mirrored(pt, n) else {
            eprintln!("  mirror round {r}: mirrored() declined; stopping");
            break;
        };
        match boolean(&body, &m, BoolOp::Union, 1e-7) {
            Ok(res)
                if res
                    .faults
                    .iter()
                    .all(|f| matches!(f, keel_topo::boolean::BoolFault::Coincident(..)))
                    && res.body.validate().is_ok() =>
            {
                body = res.body;
                let c = body.counts();
                eprintln!("  mirror round {r}: f={} e={} g{}", c.f, c.e, c.genus);
            }
            other => {
                eprintln!("  mirror round {r}: union declined/faulted ({:?}); stopping", other.map(|r| r.faults).err());
                break;
            }
        }
    }
    let c = body.counts();
    eprintln!(
        "grown (mirror) body: v={} e={} f={} g{}",
        c.v, c.e, c.f, c.genus
    );
    body
}

/// Run `f` in a thread with a `kib`-KiB stack. Returns Some(label) if it
/// COMPLETED (returned normally), None if the thread aborted/overflowed
/// (join Err) -- the overflow culprit.
fn try_small_stack<F: FnOnce() + Send + 'static>(kib: usize, label: &str, f: F) -> bool {
    let h = std::thread::Builder::new()
        .name(label.to_string())
        .stack_size(kib * 1024)
        .spawn(f)
        .unwrap();
    match h.join() {
        Ok(()) => {
            eprintln!("  [{label}] COMPLETED on {kib} KiB stack");
            true
        }
        Err(_) => {
            eprintln!("  [{label}] PANICKED/overflowed on {kib} KiB stack");
            false
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let kib: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(512);
    let chain: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
    // mode: "feature" (default, transversal grid) or "mirror" (exponential).
    let mode = args.get(3).cloned().unwrap_or_else(|| "feature".into());

    // Build the grown body ON THE MAIN thread (big default stack) so the
    // growth itself never confounds the per-op probe.
    let body = if mode == "mirror" {
        grow_mirror(chain)
    } else {
        grow(chain)
    };

    eprintln!("\nprobing each op on a {kib} KiB stack (None == the recursion):");

    // 1. clone
    {
        let b = body.clone();
        try_small_stack(kib, "clone", move || {
            let c = b.clone();
            std::hint::black_box(&c);
        });
    }
    // 2. drop (drop the clone inside the small thread)
    {
        let b = body.clone();
        try_small_stack(kib, "drop", move || {
            drop(b);
        });
    }
    // 3. validate
    {
        let b = body.clone();
        try_small_stack(kib, "validate", move || {
            let _ = b.validate();
        });
    }
    // 4. mass_properties
    {
        let b = body.clone();
        try_small_stack(kib, "mass_properties", move || {
            let _ = b.mass_properties();
        });
    }
    // 5. mesh_volume / tessellation
    {
        let b = body.clone();
        try_small_stack(kib, "mesh_volume", move || {
            let _ = b.mesh_volume();
        });
    }
    // 6. one more boolean against a fresh peg
    {
        let b = body.clone();
        try_small_stack(kib, "boolean", move || {
            let bb = b.bounding_box();
            let (tool, op) = peg(bb.min, bb.max, 1);
            let _ = boolean(&b, &tool, op, 1e-7);
        });
    }
    // 7. topology_hash (touches every entity)
    {
        let b = body.clone();
        try_small_stack(kib, "topology_hash", move || {
            let _ = b.topology_hash();
        });
    }
}
