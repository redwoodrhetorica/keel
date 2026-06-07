#![allow(clippy::unwrap_used)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::project::project_point;
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

criterion_group!(benches, bench_eval, bench_project);
criterion_main!(benches);
