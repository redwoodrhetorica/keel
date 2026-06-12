//! Curved STEP export: every analytic quadric primitive exports and its
//! surface geometry survives the importer's parse layer exactly.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_geom::surface::{Frame3, Surface3};
use keel_math::vec::Vec3;
use keel_topo::body::Body;
use keel_topo::step_export::to_step_string;
use keel_topo::step_import::{ImportedSurface, surfaces_from_step};

fn analytic_surfaces(step: &str) -> Vec<Surface3> {
    surfaces_from_step(step)
        .expect("parse layer reads our own output")
        .into_iter()
        .filter_map(|s| match s {
            ImportedSurface::Analytic(a) => Some(a),
            ImportedSurface::Nurbs(_) => None,
        })
        .collect()
}

#[test]
fn sphere_torus_frustum_export_and_reimport_exactly() {
    let frame = || Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();

    let mut s = Body::new();
    s.sphere(frame(), 1.5).unwrap();
    let step = to_step_string(&s).unwrap();
    let r = analytic_surfaces(&step)
        .iter()
        .find_map(|s| match s {
            Surface3::Sphere(sp) => Some(sp.radius),
            _ => None,
        })
        .expect("spherical surface");
    assert!((r - 1.5).abs() < 1e-12);

    let mut t = Body::new();
    t.torus(frame(), 3.0, 1.0).unwrap();
    let step = to_step_string(&t).unwrap();
    let (maj, min) = analytic_surfaces(&step)
        .iter()
        .find_map(|s| match s {
            Surface3::Torus(to) => Some((to.major, to.minor)),
            _ => None,
        })
        .expect("toroidal surface");
    assert!((maj - 3.0).abs() < 1e-12);
    assert!((min - 1.0).abs() < 1e-12);

    let mut c = Body::new();
    c.loft_circles(frame(), 2.0, 1.0, 2.0).unwrap();
    let step = to_step_string(&c).unwrap();
    let (radius, semi) = analytic_surfaces(&step)
        .iter()
        .find_map(|s| match s {
            Surface3::Cone(co) => Some((co.radius, co.half_angle)),
            _ => None,
        })
        .expect("conical surface");
    assert!((radius - 2.0).abs() < 1e-12);
    assert!((semi.abs() - (0.5_f64).atan()).abs() < 1e-12);
}
