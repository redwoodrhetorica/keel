use criterion::{Criterion, black_box, criterion_group, criterion_main};
use keel_math::bernstein::Bernstein;
use keel_math::poly::solve_cubic;
use keel_math::predicates::orient_3d;
use keel_math::vec::Vec3;

fn bench_orient3d(c: &mut Criterion) {
    let a = Vec3::new(0.1, 0.2, 0.3);
    let b = Vec3::new(1.0, 0.1, -0.2);
    let p = Vec3::new(-0.3, 0.9, 0.5);
    let d = Vec3::new(0.4, 0.4, 0.40000001);
    c.bench_function("orient3d_generic", |bch| {
        bch.iter(|| orient_3d(black_box(a), black_box(b), black_box(p), black_box(d)))
    });
}

fn bench_cubic(c: &mut Criterion) {
    c.bench_function("solve_cubic_three_roots", |bch| {
        bch.iter(|| {
            solve_cubic(
                black_box(1.0),
                black_box(-4.0),
                black_box(-7.0),
                black_box(10.0),
            )
        })
    });
}

fn bench_bernstein(c: &mut Criterion) {
    let p = Bernstein::from_power(&[-0.09, 0.73, -1.6, 1.0]).unwrap();
    c.bench_function("bernstein_eval_deg3", |bch| {
        bch.iter(|| black_box(&p).eval(black_box(0.37)))
    });
    c.bench_function("bernstein_roots_deg3", |bch| {
        bch.iter(|| black_box(&p).roots(1e-12))
    });
}

criterion_group!(benches, bench_orient3d, bench_cubic, bench_bernstein);
criterion_main!(benches);
