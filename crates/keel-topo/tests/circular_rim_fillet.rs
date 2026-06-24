//! P0 (fieldforge worklist): fillet/chamfer a CLOSED CIRCULAR rim edge -- the lip of
//! a bored hole (planar top face meets cylindrical bore wall). The single most common
//! edge-rounding op in real CAD. Must produce a valid analytic blend (torus segment for
//! the fillet, cone frustum for the chamfer): watertight, validate() Ok, mass == mesh,
//! and the FULL 360 deg loop rounded (the bore rim is two arcs split at the cylinder
//! seam). Faithful repro of the consumer's exact case: it picks a rim point blindly with
//! nearest_edge -- including the seam endpoint -- then calls fillet_edge / chamfer_edge.

use keel_geom::surface::Frame3;
use keel_topo::body::Body;
use keel_topo::boolean::{boolean, BoolOp};
use keel_math::vec::Vec3;

const TOL: f64 = 1e-7;

/// 2.4 x 1.6 x 0.4 plate with a r=0.3 bore on +Z through (0.6, 0.35).
fn bored_plate() -> Body {
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 2.4, 1.6, 0.4).unwrap();
    let f = Frame3::from_z(Vec3::new(0.6, 0.35, -0.1), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut cyl = Body::new();
    cyl.cylinder(f, 0.3, 0.6).unwrap();
    boolean(&plate, &cyl, BoolOp::Difference, TOL).expect("bore difference").body
}

/// validate() Ok AND analytic mass == tessellated mesh volume (the contract's
/// dual gate); returns the certified volume.
fn assert_solid(b: &Body, what: &str) -> f64 {
    b.validate().unwrap_or_else(|e| panic!("{what}: validate failed: {e:?}"));
    let mass = b.mass_properties().unwrap_or_else(|e| panic!("{what}: mass Err: {e:?}")).volume;
    let mesh = b.mesh_volume();
    let rel = (mass - mesh).abs() / mass.abs().max(mesh.abs()).max(1e-9);
    assert!(rel <= 0.03, "{what}: mass {mass} != mesh {mesh} (rel {rel:.4})");
    mass
}

/// Rounding the convex lip REMOVES material around the whole rim. The removed
/// volume must be positive (not a no-op / single-arc partial) and small (a 0.05
/// lip on a r=0.3 bore is ~0.001-0.003; a wrong-side build would be gross).
fn assert_lip_trimmed(before: f64, after: f64, what: &str) {
    let removed = before - after;
    assert!(
        removed > 1e-5 && removed < 0.05,
        "{what}: implausible volume change (before {before}, after {after}, removed {removed})"
    );
}

#[test]
fn fillet_bored_hole_top_rim() {
    let b = bored_plate();
    let v0 = assert_solid(&b, "bored plate (pre-fillet)");
    let rim = b.nearest_edge(Vec3::new(0.9, 0.35, 0.4)).expect("top rim edge");
    let f = b
        .fillet_edge(rim, 0.05)
        .unwrap_or_else(|e| panic!("fillet_edge on bore rim DECLINED: {e:?}"));
    let v1 = assert_solid(&f, "filleted bore rim");
    assert_lip_trimmed(v0, v1, "fillet bore rim");
}

/// WF279 regression (self-consistent-wrong): filleting the 4 outer-top edges of a
/// corner-counterbored plate must NEVER emit a body whose volume INCREASED (a
/// convex fillet cannot add material). The volume-direction guard converts a
/// mis-stitched over-volume result to a clean DECLINE. A DECLINE is acceptable
/// (the safe floor); a returned body that added material is the forbidden WRONG.
#[test]
fn counterbored_plate_outer_fillet_never_adds_material() {
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 120.0, 60.0, 10.0).unwrap();
    for (x, y) in [(15.0, 15.0), (105.0, 15.0), (15.0, 45.0), (105.0, 45.0)] {
        let hf = Frame3::from_z(Vec3::new(x, y, -1.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut hole = Body::new();
        hole.cylinder(hf, 4.0, 12.0).unwrap();
        let cf = Frame3::from_z(Vec3::new(x, y, 5.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut cb = Body::new();
        cb.cylinder(cf, 6.0, 6.0).unwrap();
        plate = boolean(&plate, &hole, BoolOp::Difference, TOL).unwrap().body;
        plate = boolean(&plate, &cb, BoolOp::Difference, TOL).unwrap().body;
    }
    let top = [
        Vec3::new(60.0, 0.0, 10.0),
        Vec3::new(60.0, 60.0, 10.0),
        Vec3::new(0.0, 30.0, 10.0),
        Vec3::new(120.0, 30.0, 10.0),
    ];
    let mut cur = plate;
    for s in top {
        let prev = cur.mass_properties().unwrap().volume;
        match cur.fillet_edge(cur.nearest_edge(s).expect("edge"), 4.0) {
            Ok(f) => {
                let v = f.mass_properties().unwrap().volume;
                assert!(
                    v <= prev + 1.0,
                    "convex fillet ADDED material ({v} > {prev}): self-consistent-wrong escaped the guard"
                );
                cur = f;
            }
            Err(_) => {} // clean DECLINE is the safe floor
        }
    }
}

/// WF188 regression (self-consistent-wrong): filleting the OUTER top rim of a
/// BORED cylinder must remove only the small toroidal wedge, not over-cut ~10x.
/// The disc-cap fillet path used to strand the bore-hole loop, leaving the kept
/// inner disc covering the bore -> a 10x over-cut that validate() + mass==mesh
/// both accepted. Now the stray hole loop is relocated to the inner disc.
#[test]
fn fillet_outer_rim_of_bored_cylinder() {
    let mut cyl = Body::new();
    let cf = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
    cyl.cylinder(cf, 40.0, 50.0).unwrap();
    let mut bore = Body::new();
    let bf = Frame3::from_z(Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    bore.cylinder(bf, 12.0, 52.0).unwrap();
    let bored = boolean(&cyl, &bore, BoolOp::Difference, TOL).expect("bore").body;
    let v0 = bored.mass_properties().unwrap().volume;
    let rim = bored.nearest_edge(Vec3::new(0.0, 40.0, 50.0)).expect("outer rim");
    let f = bored
        .fillet_edge(rim, 5.0)
        .unwrap_or_else(|e| panic!("fillet outer rim of bored cyl DECLINED: {e:?}"));
    assert_solid(&f, "filleted bored outer rim");
    let removed = v0 - f.mass_properties().unwrap().volume;
    // closed-form toroidal wedge: r_f^2 * (1 - pi/4) * 2*pi*R_out.
    let want = 25.0 * (1.0 - std::f64::consts::PI / 4.0) * 2.0 * std::f64::consts::PI * 40.0;
    assert!(
        (removed - want).abs() < 0.5 * want,
        "bored outer-rim fillet over/under-cut: removed {removed} vs closed-form {want}"
    );
}

#[test]
fn chamfer_bored_hole_top_rim() {
    let b = bored_plate();
    let v0 = assert_solid(&b, "bored plate (pre-chamfer)");
    let rim = b.nearest_edge(Vec3::new(0.9, 0.35, 0.4)).expect("top rim edge");
    let c = b
        .chamfer_edge(rim, 0.05)
        .unwrap_or_else(|e| panic!("chamfer_edge on bore rim DECLINED: {e:?}"));
    let v1 = assert_solid(&c, "chamfered bore rim");
    assert_lip_trimmed(v0, v1, "chamfer bore rim");
}

/// Concave reentrant rim: the FLOOR of a blind hole (cylindrical wall meets the
/// flat floor disc). The disc-cap surgery does not yet build this correctly --
/// it can keep the wrong region (a body that validate()s but FILLS the bore, a
/// gross +vol). DECLINE-never-WRONG: fillet/chamfer here must either DECLINE
/// (the safe floor) or return a CORRECT small concave wedge -- NEVER a wrong
/// body. The fillet disc path's self-check (mass-declined / mass!=mesh / volume
/// direction) enforces this; this test locks it so the wrong body can never
/// silently ship again. (When the concave surgery is fixed, the Ok branch's
/// bound proves it is the correct small wedge, not a refill.)
#[test]
fn concave_blind_floor_fillet_never_wrong() {
    let mut cube = Body::new();
    cube.block(Vec3::new(-5.0, -5.0, -5.0), 10.0, 10.0, 10.0).unwrap();
    let mut bore = Body::new();
    bore.cylinder(
        Frame3::from_z(Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        3.0,
        6.0,
    )
    .unwrap();
    let blind = boolean(&cube, &bore, BoolOp::Difference, TOL)
        .expect("blind bore")
        .body;
    let v0 = blind.mesh_volume();
    let rim = blind
        .nearest_edge(Vec3::new(3.0, 0.0, -1.0))
        .expect("blind-floor rim");
    for r in [0.5_f64, 1.0] {
        if let Ok(f) = blind.fillet_edge(rim, r) {
            f.validate().expect("concave floor fillet: returned an invalid body");
            // A correct concave wedge changes volume by O(r^2 * 2*pi*R) ~ a few
            // units; the wrong "refilled bore" body jumped +128. Reject the gross.
            assert!(
                (f.mesh_volume() - v0).abs() < 20.0,
                "concave floor fillet (r={r}) shipped a grossly wrong volume: {v0} -> {}",
                f.mesh_volume()
            );
        }
        if let Ok(c) = blind.chamfer_edge(rim, r) {
            c.validate().expect("concave floor chamfer: returned an invalid body");
            assert!(
                (c.mesh_volume() - v0).abs() < 20.0,
                "concave floor chamfer (d={r}) shipped a grossly wrong volume: {v0} -> {}",
                c.mesh_volume()
            );
        }
    }
}
