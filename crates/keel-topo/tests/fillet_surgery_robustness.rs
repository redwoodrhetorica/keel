//! Fillet SURGERY robustness on MULTIPLY-CONNECTED supports (realsoak decline
//! frontier; same family as dossier 71's inner-loop stranding).
//!
//! The realsoak census (a full-message tally over a 3000-project soak) showed
//! the dominant `fillet:Precondition` Euler-op sub-causes were NOT independent
//! bugs but one frontier: the fillet surgery assumed simply-connected, box-like
//! supports, and a boolean-built body whose support face carries a pre-existing
//! bore/window (or whose blended rim is the boundary of a HOLE) tripped a chain
//! of Euler-op preconditions:
//!
//!   * `mekr: second loop must be an inner ring of the same face` (~313): the
//!     cap-rim torus surgery (`fillet_cap_rim`) bridged the spring hole into the
//!     rim with `mekr`, but selected the bridge loop as "the first OTHER loop"
//!     of the cap annulus. A multiply-connected cap carries EXTRA loops, so it
//!     grabbed a stray hole (or the outer boundary when the rim was itself a
//!     hole-rim), which mekr rejects. FIX: select the loop carrying the just-
//!     imprinted spring circle.
//!   * `kef: dying face must have one loop` (~54): the subsequent `kef` that
//!     dissolves the sharp rim then saw a holed dying face. FIX:
//!     `rehome_dying_face_holes` moves the dying face's inner loops onto the
//!     survivor before the kef (the cap-rim analog of `redistribute_inner_loops`).
//!   * `fillet: no cap edge at va` (~81) and `no fin ends at vertex` (~104):
//!     the linear-fillet spring imprint / cap split scanned only `loops[0]`
//!     (the outer loop) for the cap edge / end-vertex fins, so a fillet whose
//!     sharp edge lay on an INNER loop spuriously reported the edge/fins
//!     missing. FIX: `boundary_edge_at_vertex_excluding`, `split_blend_cap`, and
//!     the spring-line split now scan ALL loops and operate on the loop that
//!     actually carries the sharp edge.
//!
//! Every fix is additive and the result is gated by `validate()` plus the
//! mass==mesh floor, so a case the surgery still cannot complete DECLINEs
//! precisely (never a wrong / non-watertight body). These tests assert the
//! fixed multiply-connected cases PASS (valid + mass==mesh) and that the plain
//! simply-connected cases do not regress.
#![allow(clippy::unwrap_used)]

use keel_geom::surface::{Frame3, Surface3};
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::{AnyKey, EdgeKey, FaceKey, SurfaceGeom};

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

fn face_is_cyl(b: &Body, f: FaceKey) -> bool {
    matches!(
        b.face(f)
            .and_then(|x| x.surface)
            .and_then(|(sk, _)| b.surface(sk)),
        Some(SurfaceGeom::Analytic(Surface3::Cylinder(_)))
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
    // Curved body (the torus blend / cylinder wall): the mesh is the coarse
    // party, so allow the curved band the soak's tessellation note uses.
    assert!(
        (mass - mesh).abs() < 6e-2 * (1.0 + mass),
        "{ctx}: analytic mass {mass} != mesh {mesh} (gap {:.3}%) -- non-watertight / mis-integrated",
        100.0 * (mass - mesh).abs() / mass
    );
}

/// A coaxially-bored tube: a big cylinder (R) with a smaller coaxial cylinder
/// (r) drilled through it. Its TOP and BOTTOM caps are ANNULI (each a planar
/// face with an outer rim loop and the bore's inner hole loop) -- multiply
/// connected. Filleting the OUTER cap rim routes to the torus cap-rim surgery
/// (`fillet_cap_rim`), whose `mekr`/`kef` previously assumed a simply-connected
/// cap and declined; the loop-selection + rehome fixes make it complete.
#[test]
fn fillet_outer_cap_rim_of_bored_tube_mass_eq_mesh() {
    let big = cyl(
        Vec3::new(0.0, 0.0, -5.0),
        Vec3::new(0.0, 0.0, 1.0),
        10.0,
        10.0,
    );
    let bore = cyl(
        Vec3::new(0.0, 0.0, -6.0),
        Vec3::new(0.0, 0.0, 1.0),
        3.0,
        12.0,
    );
    let tube = boolean(&big, &bore, BoolOp::Difference, 1e-7)
        .expect("bore declined")
        .body;
    assert!(tube.validate().is_ok(), "bored tube invalid");
    assert_mass_eq_mesh(&tube, "bored tube (pre-fillet)");
    let v_tube = tube.mass_properties().unwrap().volume;

    // The OUTER cap rim: the circular edge at z = +5 of radius R = 10 between
    // the (annular) top cap and the outer cylinder wall. A cap-rim fillet here
    // exercises the multiply-connected cap surgery.
    let radius = 1.5;
    let mut filleted = None;
    for e in edge_keys(&tube) {
        let Some((a, b)) = edge_endpoints(&tube, e) else {
            continue;
        };
        // The outer rim is a CLOSED edge (a circle): its endpoints coincide at
        // a seam, both at z = +5, radius ~10.
        let at_top = (a.z - 5.0).abs() < 1e-6 && (b.z - 5.0).abs() < 1e-6;
        let r_outer = a.x.hypot(a.y) > 8.0;
        let faces = tube.faces_around_edge(e);
        let is_cap_rim = faces.len() == 2
            && faces.iter().any(|&f| face_is_planar(&tube, f))
            && faces.iter().any(|&f| face_is_cyl(&tube, f));
        if at_top
            && r_outer
            && is_cap_rim
            && let Ok(nb) = tube.fillet_edge(e, radius)
        {
            filleted = Some(nb);
            break;
        }
    }
    let fb = filleted.expect("outer cap rim of the bored tube could not be filleted");
    assert_mass_eq_mesh(&fb, "filleted bored-tube outer cap rim");
    // A cap-rim round removes a small toroidal wedge: volume drops slightly.
    let v_fil = fb.mass_properties().unwrap().volume;
    assert!(
        v_fil > 0.85 * v_tube && v_fil <= v_tube + 1e-6,
        "filleted tube volume {v_fil} not a small reduction of {v_tube}"
    );
}

/// Control: a cap-rim fillet on a PLAIN (simply-connected) cylinder cap must
/// still pass -- the multiply-connected loop-selection / rehome path is a no-op
/// there, so this guards the common cap-rim case against regression.
#[test]
fn fillet_cap_rim_of_plain_cylinder_mass_eq_mesh() {
    let c = cyl(
        Vec3::new(0.0, 0.0, -5.0),
        Vec3::new(0.0, 0.0, 1.0),
        10.0,
        10.0,
    );
    let radius = 1.5;
    let mut filleted = None;
    for e in edge_keys(&c) {
        let faces = c.faces_around_edge(e);
        let is_cap_rim = faces.len() == 2
            && faces.iter().any(|&f| face_is_planar(&c, f))
            && faces.iter().any(|&f| face_is_cyl(&c, f));
        if is_cap_rim && let Ok(nb) = c.fillet_edge(e, radius) {
            filleted = Some(nb);
            break;
        }
    }
    let fb = filleted.expect("plain cylinder cap rim could not be filleted");
    assert_mass_eq_mesh(&fb, "filleted plain cylinder cap rim");
}

/// A linear (plane-plane) fillet on a TOP edge of a through-drilled block whose
/// support TOP face is multiply connected. This is the `boundary_edge` /
/// `split_blend_cap` all-loops path: the cap edge and the end-arc split fins are
/// found across every loop, not only the outer one. (Mirrors the dossier-71
/// post_fillet_mass case; asserted here as a SURGERY-completes regression.)
#[test]
fn fillet_linear_edge_on_drilled_block_completes() {
    let mut block = Body::new();
    block
        .block(Vec3::new(-10.0, -10.0, -5.0), 20.0, 20.0, 10.0)
        .unwrap();
    let drill = cyl(
        Vec3::new(0.0, 0.0, -6.0),
        Vec3::new(0.0, 0.0, 1.0),
        3.0,
        12.0,
    );
    let holed = boolean(&block, &drill, BoolOp::Difference, 1e-7)
        .expect("drill declined")
        .body;

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
    assert_mass_eq_mesh(&fb, "filleted drilled block (linear top edge)");
}
