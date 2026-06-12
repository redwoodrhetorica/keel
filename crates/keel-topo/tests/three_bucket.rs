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
//! KEEL_ORACLE_START offsets the deterministic trial window (the LCG
//! is seekable), so the gate shards across processes: shard i runs
//! START = i*chunk, N = chunk, and the buckets sum exactly to the
//! single-process run. Every WRONG prints its full trial context.

use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean, boolean_tolerant};

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

/// The CONE sector (task 38: gate coverage for the cone vocabulary the
/// task-30 slice added). Random block-countersink configurations (an
/// axis-aligned frustum crossing the block top transversally), where
/// the reference volume is the EXACT closed-form frustum slice:
///
///   overlap = pi/3 * h_in * (r0^2 + r0*rt + rt^2),  rt = r(block top)
///
/// Buckets: PASS = clean result, mass within 1e-9 of the reference and
/// mesh within the curved CHORDAL band (2e-2: adaptive tessellation's
/// worst legitimate deviation); DECLINE = Err / faulted / mass
/// gracefully declined; WRONG = a clean result disagreeing with the
/// closed form. The gate: WRONG == 0, always. Separate LCG stream:
/// the box oracle's buckets stay bit-identical.
#[test]
#[ignore = "completion-gate instrument; run: cargo test --release -p keel-topo --test three_bucket -- --ignored --nocapture, scaled by KEEL_ORACLE_N"]
fn three_bucket_cone_oracle() {
    use keel_geom::surface::Frame3;
    let pi = core::f64::consts::PI;
    let n: usize = std::env::var("KEEL_ORACLE_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let start: usize = std::env::var("KEEL_ORACLE_START")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut rng = Lcg(0xC0DE_5EC7_0123_4567);
    for _ in 0..start * 9 {
        rng.f();
    }
    let (mut pass, mut decline, mut wrong) = (0usize, 0usize, 0usize);
    let mut first_wrong = String::new();
    for trial in start..start + n {
        let mut v = [0.0f64; 9];
        for x in v.iter_mut() {
            *x = rng.f();
        }
        let ext = [2.0 + 8.0 * v[0], 2.0 + 8.0 * v[1], 2.0 + 8.0 * v[2]];
        // Frustum base strictly inside the block, top above the block
        // top (the transversal countersink class; same clamps as
        // fuzz_cone_boolean).
        let base = 0.3 + (ext[2] - 0.6) * v[3];
        let height = (ext[2] - base) + 0.3 + 5.0 * v[4];
        // The WHOLE frustum (including the part above the block top)
        // stays inside the wall footprint: a wide overhanging top makes
        // the wall/lateral AABBs overlap and the plane-parallel-to-axis
        // SSI (a hyperbola section) declines the trial before the
        // countersink machinery runs.
        let rmax = (ext[0].min(ext[1]) * 0.5 - 0.3).max(0.3);
        let r0 = 0.2 + (rmax - 0.2).max(0.0) * v[5];
        let r1 = 0.2 + (rmax - 0.2).max(0.0) * v[6];
        let rt = r0 + (r1 - r0) * (ext[2] - base) / height;
        let rmaxz = r0.max(r1);
        let dbg = std::env::var("KEEL_ORACLE_DEBUG").is_ok();
        if (r1 - r0).abs() < 1e-3 {
            decline += 1; // a cylinder in disguise, not the cone class
            continue;
        }
        let cx = (rmaxz + 0.1) + (ext[0] - 2.0 * (rmaxz + 0.1)) * v[7];
        let cy = (rmaxz + 0.1) + (ext[1] - 2.0 * (rmaxz + 0.1)) * v[8];
        let mut a = Body::new();
        a.block(Vec3::new(0.0, 0.0, 0.0), ext[0], ext[1], ext[2])
            .unwrap();
        let frame = Frame3::from_z(Vec3::new(cx, cy, base), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        if b.loft_circles(frame, r0, r1, height).is_err() {
            decline += 1;
            continue;
        }
        let op = match trial % 3 {
            0 => BoolOp::Union,
            1 => BoolOp::Intersection,
            _ => BoolOp::Difference,
        };
        let va = ext[0] * ext[1] * ext[2];
        let frustum = |ra: f64, rb: f64, h: f64| pi / 3.0 * h * (ra * ra + ra * rb + rb * rb);
        let overlap = frustum(r0, rt, ext[2] - base);
        let vb = frustum(r0, r1, height);
        let reference = match op {
            BoolOp::Union => va + vb - overlap,
            BoolOp::Intersection => overlap,
            BoolOp::Difference => va - overlap,
        };
        match boolean(&a, &b, op, 1e-7) {
            Err(e) => {
                decline += 1;
                if dbg && trial < start + 20 {
                    eprintln!("  cone {trial} {op:?}: boolean Err {e:?}");
                }
            }
            Ok(res) => {
                let informational = res
                    .faults
                    .iter()
                    .all(|f| matches!(f, keel_topo::boolean::BoolFault::Coincident(..)));
                if !informational {
                    decline += 1;
                    if dbg && trial < start + 20 {
                        eprintln!("  cone {trial} {op:?}: faulted {:?}", res.faults);
                    }
                    continue;
                }
                let mesh = res.body.mesh_volume();
                let mesh_ok = (mesh - reference).abs() <= 2e-2 * (1.0 + reference);
                match res.body.mass_properties().map(|m| m.volume) {
                    Ok(m) => {
                        if (m - reference).abs() <= 1e-9 * (1.0 + reference) && mesh_ok {
                            pass += 1;
                        } else {
                            wrong += 1;
                            eprintln!(
                                "WRONG cone trial {trial} {op:?}: ref {reference}, mass {m}, mesh {mesh}; ext {ext:?} base {base} h {height} r0 {r0} r1 {r1} c ({cx},{cy})"
                            );
                            if first_wrong.is_empty() {
                                first_wrong =
                                    format!("cone trial {trial} {op:?} ref {reference} mass {m}");
                            }
                        }
                    }
                    // Graceful mass decline: honest, but the mesh must
                    // not silently disagree with the closed form.
                    Err(_) if mesh_ok => decline += 1,
                    Err(e) => {
                        wrong += 1;
                        eprintln!(
                            "WRONG cone trial {trial} {op:?}: mass declined ({e:?}) AND mesh {mesh} vs ref {reference}; ext {ext:?} base {base} h {height} r0 {r0} r1 {r1}"
                        );
                        if first_wrong.is_empty() {
                            first_wrong = format!("cone trial {trial} mesh {mesh} vs {reference}");
                        }
                    }
                }
            }
        }
    }
    eprintln!("cone oracle: N {n}: PASS {pass} / DECLINE {decline} / WRONG {wrong}");
    assert_eq!(wrong, 0, "WRONG must be zero: {first_wrong}");
}

#[test]
#[ignore = "completion-gate instrument (~150ms/trial debug); run: cargo test --release -p keel-topo --test three_bucket -- --ignored --nocapture, scaled by KEEL_ORACLE_N"]
fn three_bucket_boolean_oracle() {
    let n: usize = std::env::var("KEEL_ORACLE_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let start: usize = std::env::var("KEEL_ORACLE_START")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut rng = Lcg(0x9E37_79B9_7F4A_7C15);
    for _ in 0..start * 12 {
        rng.f();
    }
    let (mut pass, mut decline, mut wrong) = (0usize, 0usize, 0usize);
    let (mut t_pass, mut t_decline, mut t_wrong) = (0usize, 0usize, 0usize);
    let mut first_wrong = String::new();
    let mut breakdown: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut tally = |lane: &str, op: BoolOp, delta: f64, outcome: &str| {
        *breakdown
            .entry(format!("{lane} {op:?} d{delta:+.0e} {outcome}"))
            .or_insert(0) += 1;
    };
    for trial in start..start + n {
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
        // Every fourth trial is a CONTACT configuration: B abuts A's
        // +face on one axis with a sub-fuzz delta (0 = exact touch,
        // positive = gap, negative = penetration); the cross axes are
        // forced to overlap so the mating faces genuinely share area.
        // Random floats never touch, so without this lane the tolerant
        // tier's whole class would be unmeasured.
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
            .unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(b0[0], b0[1], b0[2]), bd[0], bd[1], bd[2])
            .unwrap();
        let op = match trial % 3 {
            0 => BoolOp::Union,
            1 => BoolOp::Intersection,
            _ => BoolOp::Difference,
        };
        let va: f64 = ad.iter().product();
        let interval_ref = |b0: &[f64; 3], bd: &[f64; 3]| -> f64 {
            let overlap: f64 = (0..3)
                .map(|i| (0f64).max((a0[i] + ad[i]).min(b0[i] + bd[i]) - a0[i].max(b0[i])))
                .product();
            let vb: f64 = bd.iter().product();
            match op {
                BoolOp::Union => va + vb - overlap,
                BoolOp::Intersection => overlap,
                BoolOp::Difference => va - overlap,
            }
        };
        let reference = interval_ref(&b0, &bd);
        let judge = |result: Result<&keel_topo::boolean::BoolResult, ()>,
                     reference: f64,
                     slack: f64|
         -> (bool, Option<String>) {
            // A Coincident fault is an informational note ("handled a
            // coincident contact"), not a partial failure: such
            // results are judged on their volumes like any other.
            let informational = |res: &keel_topo::boolean::BoolResult| {
                res.faults
                    .iter()
                    .all(|f| matches!(f, keel_topo::boolean::BoolFault::Coincident(..)))
            };
            match result {
                Err(()) => (false, None),
                Ok(res) if !informational(res) => (false, None),
                Ok(res) => {
                    // Mass is the exact sense-integrated gate (1e-9);
                    // mesh is the independent dropped-face net, given
                    // sliver headroom (ear-clip float noise on
                    // tolerance-scale sliver faces reaches ~1e-8
                    // relative; a genuinely dropped face is feature-
                    // scale, many orders above 1e-7).
                    let tol = 1e-9 * (1.0 + reference) + slack;
                    let mesh_tol = 1e-7 * (1.0 + reference) + slack;
                    let mass = res.body.mass_properties().map(|m| m.volume);
                    let mesh = res.body.mesh_volume();
                    let ok = match mass {
                        Ok(m) => {
                            (m - reference).abs() <= tol && (mesh - reference).abs() <= mesh_tol
                        }
                        // The clean empty body: right when the
                        // reference is empty up to the contact slack
                        // (a sub-tolerance sliver legitimately glues
                        // to the empty touching configuration).
                        Err(_) => reference <= 1e-9 + slack && mesh.abs() <= 1e-9 + slack,
                    };
                    if ok {
                        (true, None)
                    } else {
                        (
                            false,
                            Some(format!(
                                "{op:?}: ref {reference}, mass {mass:?}, mesh {mesh}"
                            )),
                        )
                    }
                }
            }
        };
        // STRICT lane: judged against the literal configuration with
        // the EPSILON-SOLIDITY allowance (Qi-Shapiro): a contact
        // within the op tolerance may legitimately resolve as any
        // coincidence-glued configuration, all of which lie within
        // op_tol x contact_area of the literal volume.
        let contact_slack = if contact && delta.abs() <= 1e-7 {
            // The glue may grow/shrink EITHER mating face onto the
            // other's plane: the volume moves by up to op_tol times
            // the larger FULL face area (not just the overlap).
            let face_area =
                |d: &[f64; 3]| -> f64 { (0..3).filter(|&i| i != axis).map(|i| d[i]).product() };
            2e-7 * (1.0 + face_area(&ad).max(face_area(&bd)))
        } else {
            0.0
        };
        let strict = boolean(&a, &b, op, 1e-7);
        match &strict {
            Err(_) => {
                decline += 1;
                if contact {
                    tally("strict", op, delta, "decline");
                }
            }
            Ok(res) => match judge(Ok(res), reference, contact_slack) {
                (true, _) => {
                    pass += 1;
                    if contact {
                        tally("strict", op, delta, "pass");
                    }
                }
                (false, None) => {
                    decline += 1;
                    if contact {
                        tally("strict", op, delta, "decline");
                    }
                }
                (false, Some(msg)) => {
                    wrong += 1;
                    eprintln!(
                        "WRONG strict trial {trial} contact {contact} d{delta:+.0e} ax{axis}: {msg}; a {a0:?}+{ad:?} b {b0:?}+{bd:?}"
                    );
                    if first_wrong.is_empty() {
                        first_wrong = format!("strict trial {trial} {msg}");
                    }
                }
            },
        }
        // TOLERANT lane on contact trials: judged against the SNAPPED
        // configuration (face-snap semantics: B's mating face moves
        // onto A's plane, its far side stays, so B's axis extent
        // becomes bd + delta), and salvage must be reported iff the
        // configuration needed it.
        if contact {
            let fuzz = 1e-6;
            let mut b0s = b0;
            let mut bds = bd;
            b0s[axis] = a0[axis] + ad[axis];
            bds[axis] = bd[axis] + delta;
            let ref_snapped = interval_ref(&b0s, &bds);
            match boolean_tolerant(&a, &b, op, 1e-7, fuzz) {
                Err(e) => {
                    t_decline += 1;
                    if std::env::var("KEEL_ORACLE_DEBUG").is_ok() && t_decline <= 12 {
                        eprintln!("t-decline {trial} {op:?} delta {delta}: {e:?}");
                    }
                }
                Ok((res, conf)) => {
                    let salvage_ok = conf.salvaged == (delta != 0.0);
                    // EPSILON-SOLIDITY for the tolerant lane (the 1M
                    // gate's 1-in-a-million catch, trial 813159): the
                    // prepare may legitimately snap ANY near-coincident
                    // plane pair within fuzz, not just the contact
                    // axis the snapped reference models (two cross-axis
                    // planes 9.4e-7 apart by random chance). The
                    // reported contract bounds the volume deviation by
                    // achieved_tolerance x total surface area.
                    let area = |d: &[f64; 3]| 2.0 * (d[0] * d[1] + d[1] * d[2] + d[0] * d[2]);
                    let t_slack = conf.achieved_tolerance * (area(&ad) + area(&bds));
                    match judge(Ok(&res), ref_snapped, t_slack) {
                        (true, _) if salvage_ok => {
                            t_pass += 1;
                            tally("tolerant", op, delta, "pass");
                        }
                        (true, _) => {
                            t_wrong += 1;
                            eprintln!(
                                "WRONG tolerant trial {trial}: salvage flag (delta {delta}, salvaged {}); a {a0:?}+{ad:?} b {b0:?}+{bd:?}",
                                conf.salvaged
                            );
                            if first_wrong.is_empty() {
                                first_wrong = format!(
                                    "tolerant trial {trial}: salvage flag wrong (delta {delta}, salvaged {})",
                                    conf.salvaged
                                );
                            }
                        }
                        (false, None) => {
                            t_decline += 1;
                            tally("tolerant", op, delta, "decline");
                            if std::env::var("KEEL_ORACLE_DEBUG").is_ok() {
                                eprintln!(
                                    "t-judge-decline {trial} {op:?} d{delta:+.0e} faults {:?} a {a0:?}+{ad:?} b {b0:?}+{bd:?}",
                                    res.faults
                                );
                            }
                        }
                        (false, Some(msg)) => {
                            t_wrong += 1;
                            eprintln!(
                                "WRONG tolerant trial {trial} {msg}; a {a0:?}+{ad:?} b {b0:?}+{bd:?}"
                            );
                            if first_wrong.is_empty() {
                                first_wrong = format!("tolerant trial {trial} {msg}");
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "three-bucket oracle: N {n}: strict PASS {pass} / DECLINE {decline} / WRONG {wrong}; tolerant PASS {t_pass} / DECLINE {t_decline} / WRONG {t_wrong}"
    );
    for (k, v) in &breakdown {
        eprintln!("  contact breakdown {k}: {v}");
    }
    assert_eq!(wrong + t_wrong, 0, "WRONG must be zero: {first_wrong}");
}
