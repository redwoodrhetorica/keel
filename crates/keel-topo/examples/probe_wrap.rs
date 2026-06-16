//! Off-axis rod-through-ball: the cyl/sphere WRAP class (sphere swallows the
//! cylinder cross-section -> two encircling NURBS loops -> a sphere band). The
//! coaxial version PASSES (Add 267); this is the off-axis NURBS-rim analogue.
//! Run: KEEL_WRAP_FLOW=1 cargo run --release -p keel-topo --example probe_wrap
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn main() {
    // Sphere R=3 at origin; rod r=1 along z, offset x=1 (delta=1), z in [-4,4].
    // delta+r = 2 <= R=3 -> WRAP (cross-section swallowed). Rod pokes out z=+-4.
    let sc = Vec3::ZERO;
    let sr = 3.0_f64;
    let cx = 1.0_f64;
    let cr = 1.0_f64;
    let (czlo, czhi) = (-4.0_f64, 4.0_f64);
    let mut a = Body::new();
    a.cylinder(
        Frame3::from_z(Vec3::new(cx, 0., czlo), Vec3::new(0., 0., 1.)).unwrap(),
        cr,
        czhi - czlo,
    )
    .unwrap();
    let mut b = Body::new();
    // Sphere frame axis NOT aligned with the rod (z): the rod poke-out then
    // lands near the sphere's (u,v) equator, away from its pole singularity
    // (an axis-aligned poke-out would pass through the pole -- a degenerate
    // parametrization sub-case, not representative of the generic soak wrap).
    b.sphere(Frame3::from_z(sc, Vec3::new(1., 0., 0.)).unwrap(), sr).unwrap();

    let in_cyl = |p: Vec3| -> bool {
        let r = ((p.x - cx).powi(2) + p.y * p.y).sqrt();
        r <= cr && p.z >= czlo && p.z <= czhi
    };
    let in_sph = |p: Vec3| -> bool { (p - sc).norm() <= sr };
    let mut st = 0x2545_f491_4f6c_dd1du64;
    let mut nx = || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let (lo, hi) = (Vec3::new(-3.1, -3.1, -4.1), Vec3::new(3.1, 3.1, 4.1));
    let vb = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let n = 6_000_000usize;
    let (mut hi_i, mut hi_d, mut hi_u) = (0usize, 0usize, 0usize);
    for _ in 0..n {
        let p = Vec3::new(
            lo.x + (hi.x - lo.x) * nx(),
            lo.y + (hi.y - lo.y) * nx(),
            lo.z + (hi.z - lo.z) * nx(),
        );
        let (ia, ib) = (in_cyl(p), in_sph(p));
        if ia && ib {
            hi_i += 1;
        }
        if ia && !ib {
            hi_d += 1;
        }
        if ia || ib {
            hi_u += 1;
        }
    }
    let truth = |h: usize| vb * h as f64 / n as f64;
    let run = |lbl: &str, op: BoolOp, t: f64| {
        print!("{lbl}: MC-truth={t:.4} -> ");
        match boolean(&a, &b, op, 1e-7) {
            Err(e) => println!("DECLINED {e:?}"),
            Ok(r) => {
                let m = r.body.mass_properties().map(|x| x.volume).unwrap_or(f64::NAN);
                println!(
                    "mass={m:.4} mesh={:.4} valid={} faults={:?} | m-truth={:.3}",
                    r.body.mesh_volume(),
                    r.body.validate().is_ok(),
                    r.faults,
                    (m - t).abs() / (1.0 + t)
                );
            }
        }
    };
    run("I cyl/sph", BoolOp::Intersection, truth(hi_i));
    run("D cyl/sph", BoolOp::Difference, truth(hi_d));
    run("U cyl/sph", BoolOp::Union, truth(hi_u));
}
