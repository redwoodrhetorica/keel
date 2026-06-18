//! cone/sphere WINDOW: a sphere grazing a cone's lateral in a single loop.
//! Usage: cargo run --release -p keel-topo --example probe_cone_win
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn in_cone(o: Vec3, ax: Vec3, r0: f64, h: f64, p: Vec3) -> bool {
    let v = (p - o).dot(ax);
    if v < 0.0 || v > h {
        return false;
    }
    let perp = ((p - o) - ax * v).norm();
    perp <= r0 * (1.0 - v / h)
}
fn in_sph(c: Vec3, r: f64, p: Vec3) -> bool {
    (p - c).norm() <= r
}
fn mc(o: Vec3, ax: Vec3, r0: f64, h: f64, sc: Vec3, sr: f64, op: BoolOp) -> f64 {
    let lo = Vec3::new(-3.0, -3.0, -1.0);
    let hi = Vec3::new(3.0, 3.0, 3.5);
    let mut st = 0xabcdef0123456789u64;
    let mut nx = || {
        st = st
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let vb = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let n = 5_000_000usize;
    let mut hit = 0usize;
    for _ in 0..n {
        let p = Vec3::new(
            lo.x + (hi.x - lo.x) * nx(),
            lo.y + (hi.y - lo.y) * nx(),
            lo.z + (hi.z - lo.z) * nx(),
        );
        let (ia, ib) = (in_cone(o, ax, r0, h, p), in_sph(sc, sr, p));
        let inside = match op {
            BoolOp::Intersection => ia && ib,
            BoolOp::Union => ia || ib,
            BoolOp::Difference => ia && !ib,
        };
        if inside {
            hit += 1;
        }
    }
    vb * hit as f64 / n as f64
}

fn main() {
    let (o, ax, r0, h) = (Vec3::ZERO, Vec3::new(0., 0., 1.), 2.0, 3.0);
    let sc = Vec3::new(2.0, 0., 1.0);
    let sr = 0.8;
    let mut a = Body::new();
    a.cone(Frame3::from_z(o, ax).unwrap(), r0, h).unwrap();
    let mut b = Body::new();
    b.sphere(Frame3::from_z(sc, Vec3::new(0., 0., 1.)).unwrap(), sr)
        .unwrap();
    let run = |lbl: &str, x: &Body, y: &Body, op: BoolOp, truth: f64| {
        print!("{lbl}: MC-truth={truth:.4} -> ");
        match boolean(x, y, op, 1e-7) {
            Err(e) => println!("DECLINED {e:?}"),
            Ok(r) => match r.body.mass_properties() {
                Ok(m) => println!(
                    "valid={} faults={:?} mass={:.4} mesh={:.4} | m-truth={:.3}",
                    r.body.validate().is_ok(),
                    r.faults,
                    m.volume,
                    r.body.mesh_volume(),
                    (m.volume - truth).abs() / (1.0 + truth)
                ),
                Err(e) => println!("mass ERR({e:?}) faults={:?}", r.faults),
            },
        }
    };
    let inter = mc(o, ax, r0, h, sc, sr, BoolOp::Intersection);
    let cone_v = std::f64::consts::PI / 3.0 * r0 * r0 * h;
    let sph_v = 4.0 / 3.0 * std::f64::consts::PI * sr * sr * sr;
    run("cone - sph", &a, &b, BoolOp::Difference, cone_v - inter);
    run("sph - cone", &b, &a, BoolOp::Difference, sph_v - inter);
    run("cone u sph", &a, &b, BoolOp::Union, cone_v + sph_v - inter);
    run("cone n sph", &a, &b, BoolOp::Intersection, inter);
}
