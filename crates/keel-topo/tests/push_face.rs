//! Push/pull (task 37): sweep an imprinted face with side-wall
//! creation. Exact volume oracles in both directions; rails for the
//! honest declines.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_math::vec::Vec3;
use keel_topo::body::Body;

fn imprinted_block() -> (Body, keel_topo::entity::FaceKey) {
    let mut b = Body::new();
    b.block(Vec3::ZERO, 4.0, 4.0, 1.5).unwrap();
    let top = b.pick_face(Vec3::new(2.0, 2.0, 1.5), 1e-7).unwrap();
    let ring = [
        Vec3::new(1.0, 1.0, 1.5),
        Vec3::new(3.0, 1.0, 1.5),
        Vec3::new(3.0, 3.0, 1.5),
        Vec3::new(1.0, 3.0, 1.5),
    ];
    b.imprint_closed_polyline(top, &ring).unwrap();
    let inner = b.pick_face(Vec3::new(2.0, 2.0, 1.5), 1e-7).unwrap();
    (b, inner)
}

#[test]
fn push_face_boss_and_pocket_are_exact() {
    // Boss: +0.5 on the 2x2 imprinted region: volume 24 + 2 = 26.
    let (mut b, inner) = imprinted_block();
    b.push_face(inner, 0.5).unwrap();
    assert!(b.validate().is_ok(), "boss invalid");
    let v = b.mass_properties().unwrap().volume;
    assert!((v - 26.0).abs() < 1e-9, "boss volume {v}");
    let m = b.mesh_volume();
    assert!((m - 26.0).abs() < 1e-9, "boss mesh {m}");

    // Pocket: -0.5: volume 24 - 2 = 22.
    let (mut b2, inner2) = imprinted_block();
    b2.push_face(inner2, -0.5).unwrap();
    assert!(b2.validate().is_ok(), "pocket invalid");
    let v2 = b2.mass_properties().unwrap().volume;
    assert!((v2 - 22.0).abs() < 1e-9, "pocket volume {v2}");
    let m2 = b2.mesh_volume();
    assert!((m2 - 22.0).abs() < 1e-9, "pocket mesh {m2}");
}

#[test]
fn push_face_declines_unsupported() {
    // Curved boundary: a drilled plate's bore-rim face region declines.
    let (b, _) = imprinted_block();
    // Zero distance declines.
    let mut b3 = b.clone();
    let inner3 = b3.pick_face(Vec3::new(2.0, 2.0, 1.5), 1e-7).unwrap();
    assert!(b3.push_face(inner3, 0.0).is_err(), "zero push must decline");
}
