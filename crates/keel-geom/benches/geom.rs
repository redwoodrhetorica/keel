#![allow(clippy::unwrap_used)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::nurbs_surface::{NurbsSurface, revolve_full};
use keel_geom::project::{project_point, project_point_surface};
use keel_math::vec::Vec3;

fn circle() -> NurbsCurve {
    NurbsCurve::full_circle(
        Vec3::ZERO,
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        2.0,
    )
    .unwrap()
}

fn bench_eval(c: &mut Criterion) {
    let curve = circle();
    c.bench_function("nurbs_circle_point", |b| {
        b.iter(|| black_box(&curve).point(black_box(0.37)))
    });
    c.bench_function("nurbs_circle_derivs2", |b| {
        b.iter(|| black_box(&curve).derivatives(black_box(0.37), 2))
    });
}

fn bench_project(c: &mut Criterion) {
    let curve = circle();
    let p = Vec3::new(3.0, 4.0, 1.0);
    c.bench_function("nurbs_circle_project", |b| {
        b.iter(|| project_point(black_box(&curve), black_box(p), 1e-5))
    });
}

fn torus() -> NurbsSurface {
    let profile = NurbsCurve::full_circle(
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        1.0,
    )
    .unwrap();
    revolve_full(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap()
}

fn bench_surface(c: &mut Criterion) {
    let s = torus();
    c.bench_function("nurbs_torus_point", |b| {
        b.iter(|| black_box(&s).point(black_box(0.37), black_box(0.61)))
    });
    c.bench_function("nurbs_torus_local_geometry", |b| {
        b.iter(|| black_box(&s).local_geometry(black_box(0.37), black_box(0.61)))
    });
    c.bench_function("nurbs_torus_project", |b| {
        b.iter(|| project_point_surface(black_box(&s), black_box(Vec3::new(5.0, 1.0, 2.0)), 1e-4))
    });
}

fn bench_ssi(c: &mut Criterion) {
    use keel_geom::ssi::{SurfaceRef, intersect_surfaces};
    use keel_geom::surface::{Frame3, Plane3, Sphere3, Surface3};
    let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    let sph = Surface3::Sphere(Sphere3::new(f.clone(), 2.0).unwrap());
    let plane = Surface3::Plane(Plane3::new(
        Frame3::from_z(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.)).unwrap(),
    ));
    c.bench_function("ssi_plane_sphere_exact", |b| {
        b.iter(|| {
            intersect_surfaces(
                black_box(&SurfaceRef::Analytic(&plane)),
                black_box(&SurfaceRef::Analytic(&sph)),
                1e-7,
            )
        })
    });
    // Sphere-as-NURBS vs plane (tier 2).
    let profile = NurbsCurve::circular_arc(
        Vec3::ZERO,
        Vec3::new(0., 0., -1.),
        Vec3::new(1., 0., 0.),
        2.0,
        core::f64::consts::PI,
    )
    .unwrap();
    let nsph = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    c.bench_function("ssi_nurbs_sphere_plane_tier2", |b| {
        b.iter(|| {
            intersect_surfaces(
                black_box(&SurfaceRef::Analytic(&plane)),
                black_box(&SurfaceRef::Nurbs(&nsph)),
                1e-5,
            )
        })
    });
}

criterion_group!(benches, bench_eval, bench_project, bench_surface, bench_ssi);
criterion_main!(benches);
