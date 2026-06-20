//! WRONG-safety regression: a DEEP, HEAVY op-chain must never ABORT the
//! process via stack overflow. A crash bypasses the DECLINE-never-WRONG
//! contract entirely (the harness cannot catch a stack overflow), so it is
//! strictly worse than a decline. The kernel's boolean assembly path used to
//! contain unbounded recursion (a recursive union-find in
//! `connected_face_components`, and a recursive assembly-DAG walk); both are
//! now iterative, so the boolean/tessellation/validate paths run in O(1)
//! native stack regardless of body complexity.
//!
//! The guard here builds a genuinely heavy body (hundreds of faces, high
//! genus) by a long chain of clean boolean features, then runs the full set of
//! per-op kernel routines the realistic soak exercises -- boolean, validate,
//! mass_properties, mesh_volume -- inside a deliberately SMALL (256 KiB) worker
//! stack. With the former recursion these could overflow on a heavy grown body
//! (the realsoak band-aid was a 256 MB worker stack); now the thread JOINS
//! cleanly. The contract is "completes or declines, never aborts": the test
//! asserts the worker thread returns (join Ok) -- a stack overflow would abort
//! the whole process and fail the test by killing it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolFault, BoolOp, boolean};

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).expect("block");
    b
}

fn cyl(pos: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(
        Frame3::from_z(pos, Vec3::new(0.0, 0.0, 1.0)).expect("frame"),
        r,
        h,
    )
    .expect("cylinder");
    b
}

/// Build a heavy plate by punching/adding many well-spaced features in a grid
/// (well-spaced transversal features assemble cleanly, so the face/edge count
/// climbs reliably). Kept modest in size so the test runs in a few seconds
/// while still producing a body with hundreds of faces and high genus -- deep
/// enough that the former recursive union-find would risk a small-stack
/// overflow.
fn heavy_body(target_features: usize) -> Body {
    let span = 80.0;
    let mut body = blk(Vec3::new(-span * 0.5, -span * 0.5, -5.0), span, span, 10.0);
    let n = ((span - 8.0) / 6.0) as i64;
    let mut realized = 0usize;
    let try_feat = |body: &Body, tool: &Body, op: BoolOp| -> Option<Body> {
        match boolean(body, tool, op, 1e-7) {
            Ok(r)
                if r
                    .faults
                    .iter()
                    .all(|f| matches!(f, BoolFault::Coincident(..)))
                    && r.body.validate().is_ok() =>
            {
                Some(r.body)
            }
            _ => None,
        }
    };
    'outer: for round in 0..4 {
        for ix in 0..n {
            for iy in 0..n {
                if realized >= target_features {
                    break 'outer;
                }
                let x = -span * 0.5 + 5.0 + ix as f64 * 6.0;
                let y = -span * 0.5 + 5.0 + iy as f64 * 6.0;
                let kind = (round + (ix + iy) as usize) % 4;
                let (tool, op) = match kind {
                    0 => (cyl(Vec3::new(x, y, -6.0), 0.9, 12.0), BoolOp::Difference),
                    1 => (
                        cyl(Vec3::new(x, y, 4.0), 1.0, 4.0 + round as f64),
                        BoolOp::Union,
                    ),
                    2 => (
                        blk(
                            Vec3::new(x - 1.0, y - 1.0, 1.0 - round as f64 * 0.3),
                            2.0,
                            2.0,
                            6.0,
                        ),
                        BoolOp::Difference,
                    ),
                    _ => (
                        blk(Vec3::new(x - 1.2, y - 1.2, 4.5), 2.4, 2.4, 3.0),
                        BoolOp::Union,
                    ),
                };
                if let Some(nb) = try_feat(&body, &tool, op) {
                    body = nb;
                    realized += 1;
                }
            }
        }
    }
    body
}

#[test]
fn deep_heavy_chain_does_not_abort_on_a_small_stack() {
    // Build the heavy body on the test's normal stack; correctness of the
    // build itself is incidental -- we just need a complex body.
    let body = heavy_body(60);
    let c = body.counts();
    assert!(
        c.f >= 80,
        "expected a heavy body (>=80 faces) to stress the assembly path; got f={}",
        c.f
    );

    // Run the full per-op kernel routine set on a DELIBERATELY SMALL (256 KiB)
    // stack. With the former recursion in the boolean assembly path, a heavy
    // grown body could overflow this -- and a stack overflow ABORTS the whole
    // process (it is NOT catch_unwind-able), which would kill this test. With
    // the iterative conversions the worker simply runs and JOINS cleanly.
    let worker = std::thread::Builder::new()
        .name("deep-chain-small-stack".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            // Every routine the realistic soak runs per op, on the heavy body.
            assert!(body.validate().is_ok(), "heavy body must validate");
            let _ = body.mass_properties();
            let _ = body.mesh_volume();
            let _ = body.topology_hash();
            // One more boolean against a fresh transversal peg: the assembly
            // path (imprint + classify + connected_face_components + stitch)
            // must run without deep recursion on a heavy operand.
            let bb = body.bounding_box();
            let center = (bb.min + bb.max) * 0.5;
            let peg = cyl(Vec3::new(center.x, center.y, bb.min.z - 1.0), 2.0, 40.0);
            // Result is ignored: the test asserts only that the call RETURNS
            // (completes or declines) without overflowing the stack.
            let _ = boolean(&body, &peg, BoolOp::Difference, 1e-7);
        })
        .expect("spawn small-stack worker");

    // join() returns Err only if the worker PANICKED. A true stack overflow
    // would have aborted the process before we got here, failing the test by
    // termination. A clean join (Ok) proves the deep input did not abort.
    worker
        .join()
        .expect("deep heavy chain overflowed/panicked on a 256 KiB stack (regression: unbounded recursion in the boolean assembly path)");
}
