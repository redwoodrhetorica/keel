//! Direct-API regression for a DECLINE-never-WRONG violation surfaced by the
//! faithful tutorial soak (seed 11400715918834827198): the chain
//!
//!     block -> hollow (shell) -> mirror+union -> fillet_edge -> union(cylinder)
//!
//! produced, at the final UNION step, a genus-1 body the kernel returned as
//! `Ok` whose analytic mass (~10009.65) disagreed with its tessellated mesh
//! volume (~5242.54) by ~1.9x. The realsoak oracle (which compares against
//! `mesh_volume`) flagged it `FAIL mass!=mesh`. The kernel SHOULD have
//! declined: a curved union whose `tessellated_volume` agreed with mass while
//! the per-connected-component `mesh_volume` disagreed slipped the curved
//! assembly gate (the gate's self-consistency check compared mass to
//! `tessellated_volume`, never to the user-facing `mesh_volume`).
//!
//! This test reconstructs the EXACT operand geometry (extracted from the soak
//! via KEEL_DUMP_REPRO) through the PUBLIC api only, so there is zero harness
//! doubt. The CONTRACT (DECLINE-never-WRONG): the final union must EITHER
//! decline (`Err`) OR return a body whose mass equals its `mesh_volume`. It
//! must NEVER return Ok with mass != mesh. Before the gate fix this test
//! reproduced the WRONG; after the fix the union DECLINES (the correct
//! outcome for a malformed assembly).
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::{AnyKey, EdgeKey};
use keel_geom::surface::Frame3;

fn edge_keys(b: &Body) -> Vec<EdgeKey> {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Edge(k)) => Some(k),
            _ => None,
        })
        .collect()
}

fn edge_endpoints(b: &Body, e: EdgeKey) -> Option<(Vec3, Vec3)> {
    let edge = b.edge(e)?;
    let (va, vb) = edge.bounds;
    let pa = b.vertex(va)?.point;
    let pb = b.vertex(vb)?.point;
    Some((pa, pb))
}

/// Find the live edge whose endpoint pair matches (unordered) the two given
/// points within `tol`. Mirrors the soak's geometric edge selection.
fn find_edge(b: &Body, p: Vec3, q: Vec3, tol: f64) -> Option<EdgeKey> {
    let close = |a: Vec3, c: Vec3| (a - c).norm() <= tol;
    edge_keys(b).into_iter().find(|&e| {
        if let Some((a, c)) = edge_endpoints(b, e) {
            (close(a, p) && close(c, q)) || (close(a, q) && close(c, p))
        } else {
            false
        }
    })
}

/// Rebuild the exact failing chain through the public api. Returns the
/// post-fillet body and the cylinder tool, ready for the final union.
fn build_chain() -> (Body, Body) {
    // 1. Seed block (centred at origin), dims from the soak dump.
    let (dx, dy, dz) = (
        16.615817370044468,
        25.16818664099246,
        24.23722896202797,
    );
    let mut block = Body::new();
    block
        .block(Vec3::new(-dx * 0.5, -dy * 0.5, -dz * 0.5), dx, dy, dz)
        .expect("seed block");

    // 2. Shell (hollow) with the soak's wall thickness.
    let shelled = block.hollow(1.0523181503240229).expect("hollow");

    // 3. Mirror across z = 12.118614481013985 (the body's far +z face) and
    //    union with the original (== the soak's Mirror op).
    let mirrored = shelled
        .mirrored(
            Vec3::new(0.0, 0.0, 12.118614481013985),
            Vec3::new(0.0, 0.0, 1.0),
        )
        .expect("mirror");
    let mirror_union = boolean(&shelled, &mirrored, BoolOp::Union, 1e-7)
        .expect("mirror-union assembled")
        .body;

    // 4. Fillet the edge the soak filleted (matched by endpoints), radius from
    //    the dump.
    let e = find_edge(
        &mirror_union,
        Vec3::new(-7.255590534698211, -11.531775170172207, 11.066296330689962),
        Vec3::new(7.255590534698211, -11.531775170172207, 11.066296330689962),
        1e-6,
    )
    .expect("fillet target edge present");
    let filleted = mirror_union
        .fillet_edge(e, 0.813899886354287)
        .expect("fillet");

    // 5. The cylinder tool that the soak's union fused (the WRONG-producing
    //    placement). axis = +z, so the frame is from_z(base, +z).
    let base = Vec3::new(-1.6013419572753496, -2.3088891422783124, -10.426150362427773);
    let axis = Vec3::new(0.0, 0.0, 1.0);
    let rad = 4.345903311368783;
    let len = 79.852113674645;
    let mut tool = Body::new();
    tool.cylinder(Frame3::from_z(base, axis).expect("tool frame"), rad, len)
        .expect("tool cylinder");

    (filleted, tool)
}

/// The DECLINE-never-WRONG contract for the reproduced union.
///
/// Pre-fix this asserted the WRONG reproduces (Ok with mass != mesh); post-fix
/// it asserts the correct outcome: the malformed union DECLINES, never returns
/// a self-inconsistent body. The load-bearing invariant either way: the kernel
/// must NEVER return Ok with mass != mesh.
#[test]
fn union_after_shell_mirror_fillet_declines_not_wrong() {
    let (body, tool) = build_chain();

    // The pre-union body is itself sound (a sanity floor: the violation is in
    // the final union, not upstream).
    let bm = body.mass_properties().expect("pre-union mass").volume;
    let bv = body.mesh_volume();
    assert!(
        (bm - bv).abs() <= 1e-3 * (1.0 + bm.abs()),
        "pre-union body must be self-consistent (mass {bm} mesh {bv})"
    );

    let res = boolean(&body, &tool, BoolOp::Union, 1e-7);
    match res {
        Err(_) => {
            // CORRECT (post-fix): the malformed assembly DECLINED. This is the
            // sacred DECLINE-never-WRONG outcome -- the body's mesh collapsed to
            // ~5242 (an independent Monte-Carlo of |A u B| confirms the true
            // volume is ~10000, matching mass, so the MESH is the wrong value /
            // the body is malformed), and the curved gate now catches the
            // mass != mesh_volume disagreement instead of shipping it.
        }
        Ok(r) => {
            // The ONLY admissible Ok is a self-consistent body: mass == mesh
            // within the curved 2% band. A returned body whose mass disagrees
            // with its mesh by ~2x is the WRONG this regression pins.
            let m = r
                .body
                .mass_properties()
                .map(|mp| mp.volume)
                .expect("assembled mass");
            let mesh = r.body.mesh_volume();
            assert!(
                (m - mesh).abs() <= 2e-2 * (1.0 + m.abs()),
                "DECLINE-never-WRONG violation: union returned Ok with \
                 mass {m} != mesh {mesh} (ratio {:.3})",
                m / mesh
            );
        }
    }
}

/// Pin the EXACT post-fix outcome: this specific malformed union DECLINES
/// (returns Err). Separate from the contract test above (which would also pass
/// for a hypothetical clean assembly) so a regression that silently starts
/// returning Ok again -- even a self-consistent one -- is caught here as a
/// changed outcome and re-examined. The correct behavior for THIS body is a
/// clean decline.
#[test]
fn union_after_shell_mirror_fillet_is_declined() {
    let (body, tool) = build_chain();
    let res = boolean(&body, &tool, BoolOp::Union, 1e-7);
    assert!(
        res.is_err(),
        "expected the malformed multi-component curved union to DECLINE \
         (mass != mesh), but it returned Ok -- the curved-gate hole has \
         reopened (DECLINE-never-WRONG)"
    );
}
