//! The Steinmetz oracle (task 29, pinned BEFORE the crossing-seam
//! imprint is built: oracle-first). Two equal perpendicular cylinders:
//! the intersection (bicylinder) has the exact volume (16/3) r^3, no
//! pi anywhere: the classic result. Today the boolean DECLINES this
//! class honestly (the crossing-seam imprint is unbuilt); the moment it
//! assembles, it must be RIGHT.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::body::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn crossing_cylinders() -> (Body, Body) {
    let r = 1.0;
    let mut a = Body::new();
    a.cylinder(
        Frame3::from_z(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        r,
        4.0,
    )
    .unwrap();
    let mut b = Body::new();
    b.cylinder(
        Frame3::from_z(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)).unwrap(),
        r,
        4.0,
    )
    .unwrap();
    (a, b)
}

#[test]
fn steinmetz_declines_today_and_must_be_exact_when_it_lands() {
    let (a, b) = crossing_cylinders();
    let exact = 16.0 / 3.0; // (16/3) r^3, r = 1
    match boolean(&a, &b, BoolOp::Intersection, 1e-7) {
        Err(_) => {} // the honest decline (current state)
        Ok(r) => {
            // The crossing-seam imprint has landed: the bicylinder must
            // be exact (this is the wrong-positive the decline guards:
            // the old attempt read 12.5).
            assert!(r.body.validate().is_ok(), "bicylinder invalid");
            let v = r.body.mass_properties().unwrap().volume;
            assert!(
                (v - exact).abs() < 1e-9,
                "bicylinder volume {v} vs exact {exact}"
            );
            let m = r.body.mesh_volume();
            assert!(
                (m - v).abs() <= 2e-2 * (1.0 + v.abs()),
                "bicylinder mesh {m} vs mass {v}"
            );
        }
    }
}
