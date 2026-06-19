//! Post-fillet mass on a MULTIPLY-CONNECTED support (dossier 71, realsoak seed
//! 11400715918834829014).
//!
//! The bug: `fillet_edge` imprints a spring line on the support face and splits
//! it via `split_face` (an outer-loop-only Euler MEF). When the support is
//! multiply connected (it carries interior hole loops -- a drilled bore, a
//! window), `split_face` left ALL the support's inner loops on `face_old`, which
//! can be the thin spring STRIP rather than the surviving support. The later
//! `kef` then merged the strip (carrying the stranded holes) into the blend
//! face, so a tiny blend cylinder ended up owning the support's holes -- loops
//! that lie OFF the blend's surface. The B-rep stayed `validate()`-clean, but
//! the mesh was no longer watertight, and the analytic `mass_properties`
//! (a boundary x-flux integral, exact only on a CLOSED shell) over-read grossly
//! (seed 11400715918834829014: analytic 8405 vs the true ~6170).
//!
//! The fix (blend.rs `redistribute_inner_loops`, called from both
//! `imprint_spring_line` and `split_blend_cap`): after a fillet/chamfer splits
//! a face, reassign each inner-hole loop to whichever child geometrically
//! contains it (even-odd test in the support surface's (u, v) plane), so the
//! support's holes follow the support and never strand on the spring strip or
//! the cap sliver. With the holes back where they belong, the body is
//! watertight and the analytic mass equals the mesh.
//!
//! These tests assert the blended holed body reports `mass == mesh` (the
//! integrator is correct; the surgery must hand it a watertight body), and a
//! plain (simply-connected) fillet still passes (no regression).
#![allow(clippy::unwrap_used)]

use keel_geom::surface::{Frame3, Surface3};
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::{AnyKey, EdgeKey, FaceKey, SurfaceGeom};

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}

fn cyl(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, axis).unwrap(), r, h)
        .unwrap();
    b
}

fn edge_keys(b: &Body) -> Vec<EdgeKey> {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Edge(k)) => Some(k),
            _ => None,
        })
        .collect()
}

fn face_is_planar(b: &Body, f: FaceKey) -> bool {
    matches!(
        b.face(f)
            .and_then(|x| x.surface)
            .and_then(|(sk, _)| b.surface(sk)),
        Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
    )
}

fn edge_endpoints(b: &Body, e: EdgeKey) -> Option<(Vec3, Vec3)> {
    let edge = b.edge(e)?;
    let (va, vb) = edge.bounds;
    Some((b.vertex(va)?.point, b.vertex(vb)?.point))
}

/// Assert a body's analytic mass equals its tessellated mesh volume to the
/// curved band (the DECLINE-never-WRONG self-consistency the floor gate guards).
fn assert_mass_eq_mesh(b: &Body, ctx: &str) {
    assert!(b.validate().is_ok(), "{ctx}: invalid body");
    let mass = b
        .mass_properties()
        .unwrap_or_else(|e| panic!("{ctx}: mass declined: {e:?}"))
        .volume;
    let mesh = b.mesh_volume();
    assert!(mass > 0.0, "{ctx}: non-positive mass {mass}");
    assert!(
        (mass - mesh).abs() < 2e-2 * (1.0 + mass),
        "{ctx}: analytic mass {mass} != mesh {mesh} (gap {:.3}%) -- non-watertight / mis-integrated",
        100.0 * (mass - mesh).abs() / mass
    );
}

/// A through-drilled block: the TOP and BOTTOM faces are multiply connected
/// (each carries the bore's rim loop). Filleting a top edge splits the holed
/// top support; the bore must stay on the support, not migrate to the blend.
#[test]
fn fillet_edge_on_drilled_block_top_mass_eq_mesh() {
    // 20 x 20 x 10 block centered at origin, drilled by a vertical cylinder
    // (r = 3) through the full height: a clean through-hole.
    let block = blk(Vec3::new(-10.0, -10.0, -5.0), 20.0, 20.0, 10.0);
    let drill = cyl(
        Vec3::new(0.0, 0.0, -6.0),
        Vec3::new(0.0, 0.0, 1.0),
        3.0,
        12.0,
    );
    let holed = boolean(&block, &drill, BoolOp::Difference, 1e-7)
        .expect("drill declined")
        .body;
    assert!(holed.validate().is_ok(), "holed block invalid");
    // Sanity: the drilled block is itself watertight and correct.
    assert_mass_eq_mesh(&holed, "drilled block (pre-fillet)");
    let v_holed = holed.mass_properties().unwrap().volume;

    // Fillet a TOP edge (z = +5). A top edge is a straight planar-planar edge
    // whose endpoints both sit at z = +5; its support faces are the (holed) top
    // face and a side face.
    let radius = 1.5;
    let mut filleted = None;
    for e in edge_keys(&holed) {
        let Some((a, b)) = edge_endpoints(&holed, e) else {
            continue;
        };
        let top_edge = (a.z - 5.0).abs() < 1e-9 && (b.z - 5.0).abs() < 1e-9;
        let faces = holed.faces_around_edge(e);
        let two_planar = faces.len() == 2 && faces.iter().all(|&f| face_is_planar(&holed, f));
        if top_edge
            && two_planar
            && (b - a).norm() > 4.0
            && let Ok(nb) = holed.fillet_edge(e, radius)
        {
            filleted = Some(nb);
            break;
        }
    }
    let fb = filleted.expect("no top edge of the drilled block could be filleted");

    // The headline assertion: the filleted HOLED body reports mass == mesh.
    // Before the inner-loop redistribution fix, the bore migrated to the blend
    // face, the body went non-watertight, and the analytic mass over-read by
    // ~36% (the floor gate then DECLINED it). After the fix it PASSes.
    assert_mass_eq_mesh(&fb, "filleted drilled block (top edge)");

    // A fillet removes a small convex wedge: the volume drops slightly but stays
    // close to the pre-fillet holed volume (a sanity bound on the magnitude, so
    // a self-consistent-but-wrong mass cannot pass on mass==mesh alone).
    let v_fil = fb.mass_properties().unwrap().volume;
    assert!(
        v_fil > 0.9 * v_holed && v_fil <= v_holed + 1e-6,
        "filleted volume {v_fil} not a small reduction of {v_holed}"
    );
}

/// Control: a fillet on a SIMPLY-connected support (a plain block, no hole)
/// must still pass -- the inner-loop redistribution is a no-op there, so this
/// guards against any regression to the common case.
#[test]
fn fillet_edge_on_plain_block_mass_eq_mesh() {
    let block = blk(Vec3::new(-10.0, -10.0, -5.0), 20.0, 20.0, 10.0);
    let radius = 1.5;
    let mut filleted = None;
    for e in edge_keys(&block) {
        let Some((a, b)) = edge_endpoints(&block, e) else {
            continue;
        };
        let faces = block.faces_around_edge(e);
        let two_planar = faces.len() == 2 && faces.iter().all(|&f| face_is_planar(&block, f));
        if two_planar
            && (b - a).norm() > 4.0
            && let Ok(nb) = block.fillet_edge(e, radius)
        {
            filleted = Some(nb);
            break;
        }
    }
    let fb = filleted.expect("no block edge could be filleted");
    assert_mass_eq_mesh(&fb, "filleted plain block");
}
