//! Characterize the two seed-1 cyl/sphere failures newly EXPOSED when the
//! non-coaxial cyl/sphere SSI started returning Empty (no lateral hit)
//! instead of the old blanket Err -> IntersectionFailed decline. Both
//! spheres sit inside the cylinder radially but poke out a cap. Goal:
//! true geometry WRONG (mass wrong vs an independent Monte-Carlo truth) or
//! a tessellation artifact (mass right, curved mesh under-resolved)?
//!
//! Usage: cargo run --release -p keel-topo --example probe_cysph_fail
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

struct Cyl {
    pos: Vec3,
    axis: Vec3,
    r: f64,
    h: f64,
}
struct Sph {
    pos: Vec3,
    r: f64,
}

fn in_cyl(c: &Cyl, p: Vec3) -> bool {
    let ax = c.axis.try_normalize().unwrap();
    let w = p - c.pos;
    let t = w.dot(ax);
    if t < 0.0 || t > c.h {
        return false;
    }
    (w - ax * t).norm() <= c.r
}
fn in_sph(s: &Sph, p: Vec3) -> bool {
    (p - s.pos).norm() <= s.r
}

/// Monte-Carlo truth over a generous AABB; deterministic LCG.
fn mc_volume(lo: Vec3, hi: Vec3, n: usize, inside: impl Fn(Vec3) -> bool) -> f64 {
    let mut st = 0x1234_5678_9abc_def0u64;
    let mut next = || {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let vol_box = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let mut hit = 0usize;
    for _ in 0..n {
        let p = Vec3::new(
            lo.x + (hi.x - lo.x) * next(),
            lo.y + (hi.y - lo.y) * next(),
            lo.z + (hi.z - lo.z) * next(),
        );
        if inside(p) {
            hit += 1;
        }
    }
    vol_box * hit as f64 / n as f64
}

fn run(label: &str, c: &Cyl, s: &Sph, op: BoolOp) {
    let mut ba = Body::new();
    ba.cone_or_cyl(c);
    let mut bb = Body::new();
    bb.sphere(Frame3::from_z(s.pos, Vec3::new(0., 0., 1.)).unwrap(), s.r).unwrap();
    // MC truth over the union AABB (sphere is the smaller; cylinder bounds it).
    let pad = 0.05;
    let lo = Vec3::new(
        (c.pos.x - c.r - c.h).min(s.pos.x - s.r) - pad,
        (c.pos.y - c.r - c.h).min(s.pos.y - s.r) - pad,
        (c.pos.z - c.r - c.h).min(s.pos.z - s.r) - pad,
    );
    let hi = Vec3::new(
        (c.pos.x + c.r + c.h).max(s.pos.x + s.r) + pad,
        (c.pos.y + c.r + c.h).max(s.pos.y + s.r) + pad,
        (c.pos.z + c.r + c.h).max(s.pos.z + s.r) + pad,
    );
    let truth = match op {
        BoolOp::Intersection => mc_volume(lo, hi, 4_000_000, |p| in_cyl(c, p) && in_sph(s, p)),
        BoolOp::Difference => mc_volume(lo, hi, 4_000_000, |p| in_cyl(c, p) && !in_sph(s, p)),
        BoolOp::Union => mc_volume(lo, hi, 4_000_000, |p| in_cyl(c, p) || in_sph(s, p)),
    };
    print!("{label}: MC-truth={truth:.4}  ");
    match boolean(&ba, &bb, op, 1e-7) {
        Err(e) => println!("DECLINED {e:?}"),
        Ok(r) => {
            let mesh = r.body.mesh_volume();
            let valid = r.body.validate().is_ok();
            match r.body.mass_properties() {
                Ok(m) => println!(
                    "mass={:.4} mesh={mesh:.4} valid={valid} faults={:?} | mass-vs-truth={:.4} mass-vs-mesh={:.4}",
                    m.volume, r.faults,
                    (m.volume - truth).abs() / (1.0 + truth),
                    (m.volume - mesh).abs() / (1.0 + m.volume),
                ),
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4} valid={valid} faults={:?}", r.faults),
            }
        }
    }
}

trait MakeCyl {
    fn cone_or_cyl(&mut self, c: &Cyl);
}
impl MakeCyl for Body {
    fn cone_or_cyl(&mut self, c: &Cyl) {
        self.cylinder(Frame3::from_z(c.pos, c.axis).unwrap(), c.r, c.h).unwrap();
    }
}

fn main() {
    // Failure 1: I cyl/sph
    let c1 = Cyl {
        pos: Vec3::new(-0.7282314989287573, 0.2578193561387194, -1.4989243807508887),
        axis: Vec3::new(-0.10491245566834961, 0.8316768206114489, 0.7683821733488301),
        r: 2.964423919723465,
        h: 1.796689666271305,
    };
    let s1 = Sph { pos: Vec3::new(-0.8752042306236147, 1.904556614958362, -1.1882431622168834), r: 0.9596500794485675 };
    run("F1 I cyl/sph", &c1, &s1, BoolOp::Intersection);

    // Failure 2: D cyl/sph
    let c2 = Cyl {
        pos: Vec3::new(1.7352039553190197, -1.1508489235514279, -1.3565663195374296),
        axis: Vec3::new(0.12258749927789836, 0.9935169698499271, -0.9112537422005302),
        r: 2.128908685039337,
        h: 0.9959355834482874,
    };
    let s2 = Sph { pos: Vec3::new(0.3415645037649271, 1.1172203853690066, 2.6228008161874143), r: 1.813092620620746 };
    run("F2 D cyl/sph", &c2, &s2, BoolOp::Difference);
}
