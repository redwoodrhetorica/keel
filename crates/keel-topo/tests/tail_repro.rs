//! Regression for the tolerant oracle's former 13-trial decline tail
//! (Addendum 185): twelve snapped-contact configurations that declined
//! (or carried Topo faults) because the dossier-39 sec 3.2 boundary-
//! coincident seam filter was scoped to closed-loop groups only. With
//! the universal drop, every one returns a clean result whose only
//! faults are informational Coincident notes.

use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean_tolerant};

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

#[allow(clippy::expect_used)] // test-only helper; the lint exemption covers #[test] fns only
fn trial_bodies(trial: usize) -> (Body, Body, BoolOp, f64, usize) {
    let mut rng = Lcg(0x9E37_79B9_7F4A_7C15);
    for _ in 0..trial * 12 {
        rng.f();
    }
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
    (a, b, op, delta, axis)
}

#[test]
fn tolerant_contact_tail_regression() {
    for trial in [
        179usize, 203, 443, 851, 955, 999, 1115, 1367, 1371, 1559, 1563, 1895,
    ] {
        let (a, b, op, delta, axis) = trial_bodies(trial);
        let r = boolean_tolerant(&a, &b, op, 1e-7, 1e-6);
        match r {
            Err(e) => panic!("trial {trial} {op:?} d{delta:+.0e} ax{axis}: declined: {e:?}"),
            Ok((res, _conf)) => {
                let informational = res
                    .faults
                    .iter()
                    .all(|f| matches!(f, keel_topo::boolean::BoolFault::Coincident(..)));
                assert!(
                    informational,
                    "trial {trial} {op:?} d{delta:+.0e} ax{axis}: non-informational faults {:?}",
                    res.faults
                );
                let mass = res.body.mass_properties().map(|m| m.volume).unwrap_or(0.0);
                let mesh = res.body.mesh_volume();
                assert!(
                    (mass - mesh).abs() <= 1e-7 * (1.0 + mass.abs()),
                    "trial {trial}: mass {mass} != mesh {mesh}"
                );
            }
        }
    }
}
