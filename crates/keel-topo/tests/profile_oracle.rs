//! OPT-M1 profiling harness (task 20): replays the three-bucket
//! oracle's exact trial mix with outer phase timers, and prints the
//! kernel's KEEL_PROFILE stage breakdown. Run:
//!   KEEL_PROFILE=1 cargo test --release -p keel-topo
//!     --test profile_oracle -- --ignored --nocapture
//! Optionally KEEL_PROFILE_N to change the trial count (default 200).

use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean, boolean_tolerant};
use std::time::Instant;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
#[ignore = "profiling instrument; run with KEEL_PROFILE=1 --release --nocapture"]
fn profile_oracle_workload() {
    let n: usize = std::env::var("KEEL_PROFILE_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let mut rng = Lcg(0x9E37_79B9_7F4A_7C15);
    keel_topo::profile::reset();
    let (mut t_build, mut t_bool, mut t_tolerant, mut t_judge) = (0.0f64, 0.0, 0.0, 0.0);
    let wall = Instant::now();
    for trial in 0..n {
        let mut v = [0.0f64; 12];
        for x in v.iter_mut() {
            *x = rng.f();
        }
        let org = |x: f64| -5.0 + 10.0 * x;
        let ext = |x: f64| 0.5 + 5.0 * x;
        let a0 = [org(v[0]), org(v[1]), org(v[2])];
        let ad = [ext(v[3]), ext(v[4]), ext(v[5])];
        let mut b0 = [org(v[6]), org(v[7]), org(v[8])];
        let bd = [ext(v[9]), ext(v[10]), ext(v[11])];
        let contact = trial % 4 == 3;
        let axis = trial % 3;
        if contact {
            let delta = match (trial / 4) % 5 {
                0 => 0.0,
                1 => 1e-9,
                2 => -1e-9,
                3 => 1e-7,
                _ => -1e-7,
            };
            for j in 0..3 {
                if j == axis {
                    b0[j] = a0[j] + ad[j] + delta;
                } else {
                    b0[j] = a0[j] + 0.5 * v[6 + j] * ad[j];
                }
            }
        }
        let t = Instant::now();
        let mut a = Body::new();
        a.block(Vec3::new(a0[0], a0[1], a0[2]), ad[0], ad[1], ad[2])
            .expect("block a");
        let mut b = Body::new();
        b.block(Vec3::new(b0[0], b0[1], b0[2]), bd[0], bd[1], bd[2])
            .expect("block b");
        t_build += t.elapsed().as_secs_f64();
        let op = match trial % 3 {
            0 => BoolOp::Union,
            1 => BoolOp::Intersection,
            _ => BoolOp::Difference,
        };
        let t = Instant::now();
        let strict = boolean(&a, &b, op, 1e-7);
        t_bool += t.elapsed().as_secs_f64();
        if let Ok(res) = &strict {
            let t = Instant::now();
            let _ = res.body.mass_properties().map(|m| m.volume);
            let _ = res.body.mesh_volume();
            t_judge += t.elapsed().as_secs_f64();
        }
        if contact {
            let t = Instant::now();
            let _ = boolean_tolerant(&a, &b, op, 1e-7, 1e-6);
            t_tolerant += t.elapsed().as_secs_f64();
        }
    }
    let total = wall.elapsed().as_secs_f64();
    eprintln!(
        "profile workload N={n}: wall {:.2}s ({:.1} ms/trial)\n  operand build {:.2}s\n  strict boolean {:.2}s\n  judge mass+mesh {:.2}s\n  tolerant lane {:.2}s",
        total,
        total * 1000.0 / n as f64,
        t_build,
        t_bool,
        t_judge,
        t_tolerant
    );
    eprintln!("{}", keel_topo::profile::report());
}
