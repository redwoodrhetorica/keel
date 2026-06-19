//! Standalone probe of the seed-11400715918834826715 compound-operand failing op
//! (dossier 76 per-face planar overlay). Reconstructs the body (box + boss on +x,
//! a two-boolean compound body) and the Difference tool from the instrumented
//! chain dump, runs the boolean, and reports watertightness + mass==mesh against
//! an independent Monte-Carlo CSG truth.
//!
//! Run: cargo run --release -p keel-topo --example probe_sb3
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn blk(o: Vec3, d: Vec3) -> Body {
    let mut b = Body::new();
    b.block(o, d.x, d.y, d.z).unwrap();
    b
}

#[derive(Clone, Copy)]
struct Box3 {
    o: Vec3,
    d: Vec3,
}
impl Box3 {
    fn has(&self, q: Vec3) -> bool {
        q.x >= self.o.x - 1e-9
            && q.x <= self.o.x + self.d.x + 1e-9
            && q.y >= self.o.y - 1e-9
            && q.y <= self.o.y + self.d.y + 1e-9
            && q.z >= self.o.z - 1e-9
            && q.z <= self.o.z + self.d.z + 1e-9
    }
}

fn main() {
    // Body g0 f11: box x[-9.5518,9.5518] y[-11.6659,11.6659] z[-15.2814,15.2814]
    // Unioned with a boss on its +x face x[9.5518,23.8057] y[-7.0890,6.1187]
    // z[-9.6479,3.1316]. (The +x plane x=9.5518 carries the boss-footprint hole.)
    let x0 = 9.551779708383604;
    let y0 = 11.665942029090884;
    let z0 = 15.281438357132698;
    let base = blk(
        Vec3::new(-x0, -y0, -z0),
        Vec3::new(2.0 * x0, 2.0 * y0, 2.0 * z0),
    );
    let boss_o = Vec3::new(x0, -7.088981022063896, -9.647871036389681);
    let boss_d = Vec3::new(
        23.805674267424912 - x0,
        6.118691584333003 - (-7.088981022063896),
        3.1315919809062702 - (-9.647871036389681),
    );
    let boss = blk(boss_o, boss_d);
    let r1 = boolean(&base, &boss, BoolOp::Union, 1e-7).expect("boss union");
    assert!(r1.faults.is_empty(), "boss union faults {:?}", r1.faults);
    let body = r1.body;
    let c = body.counts();
    println!("body g{} f{} (realsoak target: g0 f11)", c.genus, c.f);

    // Tool f6 (box), the failing Difference. Bounds reconstructed from the
    // chain/loop dump (x in {0.8417,15.5058}, y up to 27.197, z in {-9.166,8.974}).
    let tool_o = Vec3::new(0.8416865823442778, -3.4814833444455537, -9.166169799785951);
    let tool_hi = Vec3::new(15.50577871691214, 27.197447619469333, 8.974436478810794);
    let tool_d = tool_hi - tool_o;
    let tool = blk(tool_o, tool_d);

    for op in [BoolOp::Difference, BoolOp::Intersection] {
        match boolean(&body, &tool, op, 1e-7) {
            Err(e) => println!("{op:?}: HARD-DECLINE {e:?}"),
            Ok(r) => {
                let valid = r.body.validate().is_ok();
                let mass = r
                    .body
                    .mass_properties()
                    .map(|m| m.volume)
                    .unwrap_or(f64::NAN);
                let mesh = r.body.mesh_volume();
                // Independent MC CSG truth.
                let base_b = Box3 {
                    o: Vec3::new(-x0, -y0, -z0),
                    d: Vec3::new(2.0 * x0, 2.0 * y0, 2.0 * z0),
                };
                let boss_b = Box3 {
                    o: boss_o,
                    d: boss_d,
                };
                let tool_b = Box3 {
                    o: tool_o,
                    d: tool_d,
                };
                let inside = |q: Vec3| {
                    let body_in = base_b.has(q) || boss_b.has(q);
                    match op {
                        BoolOp::Difference => body_in && !tool_b.has(q),
                        BoolOp::Intersection => body_in && tool_b.has(q),
                        BoolOp::Union => body_in || tool_b.has(q),
                    }
                };
                let lo = Vec3::new(-x0 - 1.0, -y0 - 1.0, -z0 - 1.0);
                let hi = Vec3::new(23.9, 27.3, z0 + 1.0);
                let vb = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
                let mut st = 0x2545_f491_4f6c_dd1du64;
                let mut nx = || {
                    st ^= st << 13;
                    st ^= st >> 7;
                    st ^= st << 17;
                    (st >> 11) as f64 / (1u64 << 53) as f64
                };
                let n = 8_000_000usize;
                let mut hit = 0usize;
                for _ in 0..n {
                    let p = Vec3::new(
                        lo.x + (hi.x - lo.x) * nx(),
                        lo.y + (hi.y - lo.y) * nx(),
                        lo.z + (hi.z - lo.z) * nx(),
                    );
                    if inside(p) {
                        hit += 1;
                    }
                }
                let mc = vb * hit as f64 / n as f64;
                println!(
                    "{op:?}: valid={valid} faults={:?}\n   mass={mass:.4} mesh={mesh:.4} MC={mc:.2}  m-mesh={:.4} m-MC={:.4}",
                    r.faults,
                    (mass - mesh).abs(),
                    (mass - mc).abs() / (1.0 + mc)
                );
            }
        }
    }
}
