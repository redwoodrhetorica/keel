//! Reproduce the 5 seed-1 v3 soak failures (FAIL:mass-mesh I sph/sph) with the
//! CORRECT genome sphere frames (Frame3::from_z(pos, axis) -- the axis places
//! the u=0 seam + poles, the variable the lens assembly is sensitive to). For
//! each: validate / faults / mass / mesh and an independent MC truth, to see
//! the actual failure mode (malformed lens vs coarse mesh vs wrong mass).
//!
//! Usage: cargo run --release -p keel-topo --example probe_ss_fail
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn sph(pos: Vec3, axis: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(pos, axis).unwrap(), r).unwrap();
    b
}

fn mc_inter(ca: Vec3, ra: f64, cb: Vec3, rb: f64) -> f64 {
    let lo = Vec3::new(
        (ca.x - ra).max(cb.x - rb),
        (ca.y - ra).max(cb.y - rb),
        (ca.z - ra).max(cb.z - rb),
    );
    let hi = Vec3::new(
        (ca.x + ra).min(cb.x + rb),
        (ca.y + ra).min(cb.y + rb),
        (ca.z + ra).min(cb.z + rb),
    );
    if hi.x <= lo.x || hi.y <= lo.y || hi.z <= lo.z {
        return 0.0;
    }
    let mut st = 0x9e3779b97f4a7c15u64;
    let mut nx = || {
        st = st
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let vb = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let n = 4_000_000usize;
    let mut hit = 0usize;
    for _ in 0..n {
        let p = Vec3::new(
            lo.x + (hi.x - lo.x) * nx(),
            lo.y + (hi.y - lo.y) * nx(),
            lo.z + (hi.z - lo.z) * nx(),
        );
        if (p - ca).norm() <= ra && (p - cb).norm() <= rb {
            hit += 1;
        }
    }
    vb * hit as f64 / n as f64
}

fn main() {
    // (posA, axisA, rA, posB, axisB, rB) from soak_v3_s1/failures.jsonl
    let cases = [
        (
            Vec3::new(1.1696134306592736, 1.6513444071844452, 1.3008800075815423),
            Vec3::new(-0.5213418867719675, 0.9950279413418788, 0.692654206642032),
            3.8381762412586053,
            Vec3::new(1.0164236648568568, 2.964234020264538, -2.0329330428436796),
            Vec3::new(-0.3747134969120147, 0.763269045269624, 1.265654325735374),
            2.1169738858579388,
        ),
        (
            Vec3::new(1.8497047026033688, 1.8314925930221482, 1.7843913699454053),
            Vec3::new(
                -0.18267337176502751,
                0.5829760363309815,
                0.21890444503071427,
            ),
            3.5992937606016686,
            Vec3::new(0.9547656873662721, 1.2636198211146779, -2.045060158005836),
            Vec3::new(0.8812766081700043, 0.5305969616371224, 1.5836370877791393),
            3.0791045145785687,
        ),
        (
            Vec3::new(1.1567600515657541, 1.4968072866278486, 1.5941608537093397),
            Vec3::new(-0.5213418867719675, 0.9950279413418788, 0.692654206642032),
            3.639899462773032,
            Vec3::new(1.171794556133998, 2.9686803754866067, -1.438804958843038),
            Vec3::new(-0.3747134969120147, 0.763269045269624, 1.265654325735374),
            2.6981844204159278,
        ),
        (
            Vec3::new(1.8497047026033688, 1.8314925930221482, 1.7843913699454053),
            Vec3::new(
                -0.18267337176502751,
                0.5829760363309815,
                0.21890444503071427,
            ),
            3.407366744642806,
            Vec3::new(0.9547656873662721, 1.2636198211146779, -2.045060158005836),
            Vec3::new(0.8748938451889727, 0.41641752535955523, 1.6025623523265022),
            3.0791045145785687,
        ),
        (
            Vec3::new(1.748717428601268, -0.7502201271292335, -1.854822199101193),
            Vec3::new(2.048324965647873, 0.8515231913693622, 0.3927250040472694),
            3.102080188953758,
            Vec3::new(0.4790619973253392, -2.5360053562902327, -1.9682662981263723),
            Vec3::new(
                0.3526844134361564,
                -0.2969002733741696,
                -0.20859747915538573,
            ),
            1.2644672217887745,
        ),
    ];
    for (i, &(pa, aa, ra, pb, ab, rb)) in cases.iter().enumerate() {
        let a = sph(pa, aa, ra);
        let b = sph(pb, ab, rb);
        let truth = mc_inter(pa, ra, pb, rb);
        print!(
            "case {i}: rA={ra:.3} rB={rb:.3} dist={:.3} MC-truth={truth:.4} -> ",
            (pa - pb).norm()
        );
        match boolean(&a, &b, BoolOp::Intersection, 1e-7) {
            Err(e) => println!("DECLINED {e:?}"),
            Ok(r) => {
                let valid = r.body.validate().is_ok();
                let mesh = r.body.mesh_volume();
                match r.body.mass_properties() {
                    Ok(m) => println!(
                        "valid={valid} faults={:?} mass={:.4} mesh={mesh:.4} | m-truth={:.3} m-mesh={:.3}",
                        r.faults,
                        m.volume,
                        (m.volume - truth).abs() / (1.0 + truth),
                        (m.volume - mesh).abs() / (1.0 + m.volume)
                    ),
                    Err(e) => println!(
                        "valid={valid} faults={:?} mass=ERR({e:?}) mesh={mesh:.4}",
                        r.faults
                    ),
                }
            }
        }
    }
}
