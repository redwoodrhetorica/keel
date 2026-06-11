//! Lists STRICT-lane declining trial numbers over a window (judge
//! logic mirrors three_bucket.rs). Diff two builds' outputs to find
//! exactly which trials an optimization flipped. KEEL_SCAN_START /
//! KEEL_SCAN_END control the window.

use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

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
#[ignore = "diagnosis instrument"]
fn scan_strict_declines() {
    let start: usize = std::env::var("KEEL_SCAN_START")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let end: usize = std::env::var("KEEL_SCAN_END")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let mut rng = Lcg(0x9E37_79B9_7F4A_7C15);
    for _ in 0..start * 12 {
        rng.f();
    }
    for trial in start..end {
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
        let mut delta = 0.0f64;
        let axis = trial % 3;
        if contact {
            delta = match (trial / 4) % 5 {
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
        let mut a = Body::new();
        a.block(Vec3::new(a0[0], a0[1], a0[2]), ad[0], ad[1], ad[2])
            .expect("block a");
        let mut b = Body::new();
        b.block(Vec3::new(b0[0], b0[1], b0[2]), bd[0], bd[1], bd[2])
            .expect("block b");
        let op = match trial % 3 {
            0 => BoolOp::Union,
            1 => BoolOp::Intersection,
            _ => BoolOp::Difference,
        };
        let va: f64 = ad.iter().product();
        let overlap: f64 = (0..3)
            .map(|i| (0f64).max((a0[i] + ad[i]).min(b0[i] + bd[i]) - a0[i].max(b0[i])))
            .product();
        let vb: f64 = bd.iter().product();
        let reference = match op {
            BoolOp::Union => va + vb - overlap,
            BoolOp::Intersection => overlap,
            BoolOp::Difference => va - overlap,
        };
        let contact_slack = if contact && delta.abs() <= 1e-7 {
            let face_area =
                |d: &[f64; 3]| -> f64 { (0..3).filter(|&i| i != axis).map(|i| d[i]).product() };
            2e-7 * (1.0 + face_area(&ad).max(face_area(&bd)))
        } else {
            0.0
        };
        let decline_reason: Option<String> = match boolean(&a, &b, op, 1e-7) {
            Err(e) => Some(format!("{e:?}")),
            Ok(res) => {
                let informational = res
                    .faults
                    .iter()
                    .all(|f| matches!(f, keel_topo::boolean::BoolFault::Coincident(..)));
                if !informational {
                    Some(format!("faults {:?}", res.faults))
                } else {
                    let tol = 1e-9 * (1.0 + reference) + contact_slack;
                    let mesh_tol = 1e-7 * (1.0 + reference) + contact_slack;
                    let mass = res.body.mass_properties().map(|m| m.volume);
                    let mesh = res.body.mesh_volume();
                    let ok = match mass {
                        Ok(m) => {
                            (m - reference).abs() <= tol && (mesh - reference).abs() <= mesh_tol
                        }
                        Err(_) => {
                            reference <= 1e-9 + contact_slack && mesh.abs() <= 1e-9 + contact_slack
                        }
                    };
                    if ok { None } else { Some("judge".into()) }
                }
            }
        };
        if let Some(r) = decline_reason {
            eprintln!("DECLINE {trial} {op:?} contact {contact} d{delta:+.0e} ax{axis}: {r}");
        }
    }
    eprintln!("scan-declines done [{start},{end})");
}
