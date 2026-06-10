//! The completion-gate THREE-BUCKET boolean oracle (dossier 57 Rung 5
//! oracle / the standing completion plan): random axis-aligned box
//! pairs, where the reference volume is EXACT interval arithmetic, so
//! every trial lands in exactly one bucket:
//!
//!   PASS    = clean result with STRICT mass == mesh == reference
//!             (axis-aligned planar results tessellate exactly, so
//!             both gates are tight);
//!   DECLINE = Err or a faulted partial result (counted, never
//!             penalized: DECLINE-never-WRONG);
//!   WRONG   = a clean result whose volumes disagree with the
//!             reference. The gate: WRONG == 0, always.
//!
//! N scales by KEEL_ORACLE_N (CI default keeps the suite fast); the
//! completion-gate run is the same binary at millions of trials:
//!   KEEL_ORACLE_N=1000000 cargo test --release -p keel-topo
//!     --test three_bucket -- --nocapture

use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

/// Deterministic 64-bit LCG (no rand dependency; reproducible runs).
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
#[ignore = "completion-gate instrument (~150ms/trial debug); run: cargo test --release -p keel-topo --test three_bucket -- --ignored --nocapture, scaled by KEEL_ORACLE_N"]
fn three_bucket_boolean_oracle() {
    let n: usize = std::env::var("KEEL_ORACLE_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let mut rng = Lcg(0x9E37_79B9_7F4A_7C15);
    let (mut pass, mut decline, mut wrong) = (0usize, 0usize, 0usize);
    let mut first_wrong = String::new();
    for trial in 0..n {
        let mut v = [0.0f64; 12];
        for x in v.iter_mut() {
            *x = rng.f();
        }
        let org = |x: f64| -5.0 + 10.0 * x;
        let ext = |x: f64| 0.5 + 5.0 * x;
        let a0 = [org(v[0]), org(v[1]), org(v[2])];
        let ad = [ext(v[3]), ext(v[4]), ext(v[5])];
        let b0 = [org(v[6]), org(v[7]), org(v[8])];
        let bd = [ext(v[9]), ext(v[10]), ext(v[11])];
        let mut a = Body::new();
        a.block(Vec3::new(a0[0], a0[1], a0[2]), ad[0], ad[1], ad[2])
            .unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(b0[0], b0[1], b0[2]), bd[0], bd[1], bd[2])
            .unwrap();
        let op = match trial % 3 {
            0 => BoolOp::Union,
            1 => BoolOp::Intersection,
            _ => BoolOp::Difference,
        };
        // Exact interval reference.
        let overlap: f64 = (0..3)
            .map(|i| (0f64).max((a0[i] + ad[i]).min(b0[i] + bd[i]) - a0[i].max(b0[i])))
            .product();
        let va: f64 = ad.iter().product();
        let vb: f64 = bd.iter().product();
        let reference = match op {
            BoolOp::Union => va + vb - overlap,
            BoolOp::Intersection => overlap,
            BoolOp::Difference => va - overlap,
        };
        match boolean(&a, &b, op, 1e-7) {
            Err(_) => decline += 1,
            Ok(res) if !res.faults.is_empty() => decline += 1,
            Ok(res) => {
                let tol = 1e-9 * (1.0 + reference);
                let mass = res.body.mass_properties().map(|m| m.volume);
                let mesh = res.body.mesh_volume();
                let ok = match mass {
                    Ok(m) => (m - reference).abs() <= tol && (mesh - reference).abs() <= tol,
                    // An empty result is only right when the reference
                    // is empty (a vanishing intersection/difference).
                    Err(_) => reference <= 1e-9 && mesh.abs() <= 1e-9,
                };
                if ok {
                    pass += 1;
                } else {
                    wrong += 1;
                    if first_wrong.is_empty() {
                        first_wrong = format!(
                            "trial {trial} {op:?}: ref {reference}, mass {mass:?}, mesh {mesh}, a {a0:?}+{ad:?}, b {b0:?}+{bd:?}"
                        );
                    }
                }
            }
        }
    }
    eprintln!("three-bucket oracle: N {n}: PASS {pass} / DECLINE {decline} / WRONG {wrong}");
    assert_eq!(wrong, 0, "WRONG must be zero: {first_wrong}");
}
