//! Evolutionary kernel explorer (spike): random structured scenes of two
//! primitives + a boolean op, mutated under NOVELTY-SEARCH selection, with
//! the op-gym judge as the failure detector. Two artifact streams, both
//! deterministic-replayable (the genome is the seed):
//!
//!   failures.jsonl  -- panics, invalid bodies, mass!=mesh, unbounded
//!                      volume: the things the decline-never-wrong
//!                      contract forbids.
//!   novelties.jsonl -- first sighting of each behavior signature
//!                      (outcome x shapes x op x fault x topology): the
//!                      kernel's behavior-space and capability frontier.
//!
//! Usage: cargo run --release -p keel-topo --example evolve -- [evals] [seed] [outdir]
//! Defaults: 20000 evals, seed 1, evolve-out/. Every line carries the full
//! genome, so a find replays bit-identically (the kernel is deterministic).
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use std::io::Write as _;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Deterministic 64-bit LCG (no rand / no clock: reproducible runs).
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
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    /// Approx standard normal via the central-limit sum of 6 uniforms.
    fn gauss(&mut self) -> f64 {
        let s: f64 = (0..6).map(|_| self.f()).sum();
        (s - 3.0) / 1.0
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Block,
    Cylinder,
    Cone,
    Sphere,
}

impl Shape {
    fn from_u(u: usize) -> Shape {
        match u % 4 {
            0 => Shape::Block,
            1 => Shape::Cylinder,
            2 => Shape::Cone,
            _ => Shape::Sphere,
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Shape::Block => "block",
            Shape::Cylinder => "cyl",
            Shape::Cone => "cone",
            Shape::Sphere => "sph",
        }
    }
}

#[derive(Clone, Copy)]
struct Prim {
    shape: Shape,
    pos: Vec3,
    // a/b/c reused per shape: block dims, or radius/height, or radius.
    a: f64,
    b: f64,
    c: f64,
    axis: Vec3,
}

#[derive(Clone, Copy)]
struct Genome {
    p: [Prim; 2],
    op: usize,
}

fn rand_prim(rng: &mut Lcg) -> Prim {
    Prim {
        shape: Shape::from_u(rng.pick(4)),
        pos: Vec3::new(
            rng.range(-3.0, 3.0),
            rng.range(-3.0, 3.0),
            rng.range(-3.0, 3.0),
        ),
        a: rng.range(0.4, 3.0),
        b: rng.range(0.4, 3.0),
        c: rng.range(0.4, 3.0),
        axis: Vec3::new(
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        ),
    }
}

fn rand_genome(rng: &mut Lcg) -> Genome {
    Genome {
        p: [rand_prim(rng), rand_prim(rng)],
        op: rng.pick(3),
    }
}

fn mutate(g: &Genome, rng: &mut Lcg) -> Genome {
    let mut g = *g;
    // Perturb a handful of genes; occasionally resample a whole field.
    for pi in 0..2 {
        let p = &mut g.p[pi];
        if rng.f() < 0.25 {
            p.shape = Shape::from_u(rng.pick(4));
        }
        let jitter = |v: f64, rng: &mut Lcg, s: f64| v + rng.gauss() * s;
        if rng.f() < 0.6 {
            p.pos = Vec3::new(
                jitter(p.pos.x, rng, 0.6),
                jitter(p.pos.y, rng, 0.6),
                jitter(p.pos.z, rng, 0.6),
            );
        }
        if rng.f() < 0.6 {
            p.a = (jitter(p.a, rng, 0.4)).clamp(0.2, 4.0);
            p.b = (jitter(p.b, rng, 0.4)).clamp(0.2, 4.0);
            p.c = (jitter(p.c, rng, 0.4)).clamp(0.2, 4.0);
        }
        if rng.f() < 0.4 {
            p.axis = Vec3::new(
                jitter(p.axis.x, rng, 0.5),
                jitter(p.axis.y, rng, 0.5),
                jitter(p.axis.z, rng, 0.5),
            );
        }
    }
    if rng.f() < 0.3 {
        g.op = rng.pick(3);
    }
    g
}

fn build(p: &Prim) -> Option<Body> {
    let mut b = Body::new();
    let frame = || Frame3::from_z(p.pos, p.axis).ok();
    let ok = match p.shape {
        Shape::Block => b.block(p.pos, p.a, p.b, p.c).is_ok(),
        Shape::Cylinder => frame()
            .map(|f| b.cylinder(f, p.a, p.b).is_ok())
            .unwrap_or(false),
        Shape::Cone => frame()
            .map(|f| b.cone(f, p.a, p.b).is_ok())
            .unwrap_or(false),
        Shape::Sphere => frame().map(|f| b.sphere(f, p.a).is_ok()).unwrap_or(false),
    };
    if ok { Some(b) } else { None }
}

/// EXACT analytic volume of a primitive (matches build()'s a/b/c mapping).
fn prim_vol(p: &Prim) -> f64 {
    let pi = std::f64::consts::PI;
    match p.shape {
        Shape::Block => p.a * p.b * p.c,
        Shape::Cylinder => pi * p.a * p.a * p.b,
        Shape::Cone => pi / 3.0 * p.a * p.a * p.b,
        Shape::Sphere => 4.0 / 3.0 * pi * p.a * p.a * p.a,
    }
}

#[derive(Clone, PartialEq)]
enum Class {
    Pass,
    Decline(String),
    Fail(String),
    Skip,
}

struct Eval {
    class: Class,
    signature: String,
    // A graded "closeness to a failure" in [0,1] used as a search bonus:
    // the mass-mesh residual scaled by the gate band (1.0 = at the edge).
    risk: f64,
}

fn classify(g: &Genome) -> Eval {
    let (Some(a), Some(b)) = (build(&g.p[0]), build(&g.p[1])) else {
        return Eval {
            class: Class::Skip,
            signature: "skip".to_string(),
            risk: 0.0,
        };
    };
    let op = [BoolOp::Union, BoolOp::Intersection, BoolOp::Difference][g.op % 3];
    let op_tag = ["U", "I", "D"][g.op % 3];
    let shapes = format!("{}/{}", g.p[0].shape.tag(), g.p[1].shape.tag());
    // Independent op-volume bounds from the EXACT primitive volumes (tighter
    // than the AABB sum): a clean result must satisfy vol(A op B) in
    // [vlo, vhi] regardless of geometry, so a violation is a WRONG even when
    // mass==mesh agree (the #48 disjoint-union class). Generous slack absorbs
    // curved chordal error so only gross violations flag.
    let (vp0, vp1) = (prim_vol(&g.p[0]), prim_vol(&g.p[1]));
    let (vlo, vhi) = match op {
        BoolOp::Union => (vp0.max(vp1), vp0 + vp1),
        BoolOp::Intersection => (0.0, vp0.min(vp1)),
        BoolOp::Difference => ((vp0 - vp1).max(0.0), vp0),
    };
    let vslack = 5e-2 * (1.0 + vhi);

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| boolean(&a, &b, op, 1e-7)));
    match res {
        Err(_) => Eval {
            class: Class::Fail("panic".to_string()),
            signature: format!("FAIL:panic {op_tag} {shapes}"),
            risk: 1.0,
        },
        Ok(Err(fault)) => {
            let kind = fault_kind(&fault);
            Eval {
                class: Class::Decline(kind.clone()),
                signature: format!("DECLINE:{kind} {op_tag} {shapes}"),
                risk: 0.0,
            }
        }
        Ok(Ok(r)) => {
            // Informational Coincident faults are not failures.
            let informational = r
                .faults
                .iter()
                .all(|f| matches!(f, keel_topo::boolean::BoolFault::Coincident(..)));
            if !informational {
                let kind = r.faults.first().map(fault_kind).unwrap_or_default();
                return Eval {
                    class: Class::Decline(format!("faulted:{kind}")),
                    signature: format!("DECLINE:faulted {op_tag} {shapes}"),
                    risk: 0.0,
                };
            }
            if r.body.validate().is_err() {
                return Eval {
                    class: Class::Fail("invalid".to_string()),
                    signature: format!("FAIL:invalid {op_tag} {shapes}"),
                    risk: 1.0,
                };
            }
            let mesh = r.body.mesh_volume();
            if !mesh.is_finite() || mesh < -1e-6 {
                return Eval {
                    class: Class::Fail("bad-mesh".to_string()),
                    signature: format!("FAIL:bad-mesh {op_tag} {shapes}"),
                    risk: 1.0,
                };
            }
            // Independent op-volume bound, two severities. mass is the
            // kernel's AUTHORITATIVE (exact analytic) volume; mesh is the
            // tessellated net.
            let oob = |x: f64| x > vhi + vslack || x < vlo - vslack;
            let mut risk = 0.0;
            match r.body.mass_properties().map(|m| m.volume) {
                Ok(m) => {
                    // mass==mesh self-consistency (necessary, not sufficient).
                    // Band = 6% to cover the coarsest legitimate mesh error:
                    // the cone PRIMITIVE's mesh_volume runs ~3.8% under its
                    // exact mass (coarse base-circle tessellation), so a 2%
                    // band false-flagged every cone the broad-phase returns
                    // for a disjoint difference. The vol-bound (5% slack) is
                    // the primary correctness net; gross wrongs are far beyond.
                    let band = 6e-2 * (1.0 + m.abs());
                    let gap = (m - mesh).abs();
                    risk = (gap / band).min(2.0);
                    if m > 0.0 && gap > band {
                        return Eval {
                            class: Class::Fail("mass!=mesh".to_string()),
                            signature: format!("FAIL:mass-mesh {op_tag} {shapes}"),
                            risk: 1.0,
                        };
                    }
                    // WRONG: the authoritative volume is impossible for this
                    // op (mass==mesh both outside is the #48 silent class).
                    if oob(m) {
                        return Eval {
                            class: Class::Fail("wrong".to_string()),
                            signature: format!("FAIL:wrong {op_tag} {shapes}"),
                            risk: 1.0,
                        };
                    }
                }
                // mass HONESTLY declined, but the boolean returned Ok and the
                // mesh is a confident value outside the op bounds: a MALFORMED
                // body (validate passed, mass cannot integrate it, mesh lies)
                // the boolean should have declined.
                Err(_) if oob(mesh) => {
                    return Eval {
                        class: Class::Fail("malformed".to_string()),
                        signature: format!("FAIL:malformed {op_tag} {shapes}"),
                        risk: 1.0,
                    };
                }
                Err(_) => {}
            }
            let c = r.body.counts();
            let fbucket = bucket(c.f);
            Eval {
                class: Class::Pass,
                signature: format!(
                    "PASS {op_tag} {shapes} g{} r{} s{} f{fbucket}",
                    c.genus, c.regions, c.shells
                ),
                risk,
            }
        }
    }
}

/// Variant name of a fault without its payload (the novelty key).
fn fault_kind(f: &keel_topo::boolean::BoolFault) -> String {
    let s = format!("{f:?}");
    s.split(['(', ' ']).next().unwrap_or("?").to_string()
}

/// Coarse face-count bucket so topology novelty is meaningful, not
/// per-integer noise.
fn bucket(n: usize) -> usize {
    match n {
        0..=6 => n,
        7..=12 => 10,
        13..=24 => 20,
        _ => 40,
    }
}

fn genome_json(g: &Genome) -> String {
    let pj = |p: &Prim| {
        format!(
            "{{\"shape\":\"{}\",\"pos\":[{},{},{}],\"a\":{},\"b\":{},\"c\":{},\"axis\":[{},{},{}]}}",
            p.shape.tag(),
            p.pos.x,
            p.pos.y,
            p.pos.z,
            p.a,
            p.b,
            p.c,
            p.axis.x,
            p.axis.y,
            p.axis.z
        )
    };
    format!(
        "{{\"op\":{},\"a\":{},\"b\":{}}}",
        g.op,
        pj(&g.p[0]),
        pj(&g.p[1])
    )
}

/// A reusable evaluation worker on its own thread. Genomes go in, Evals
/// come back; a genome that hangs the kernel leaves the worker stuck, so
/// the caller abandons it (the channel is dropped) and spawns a fresh one.
fn spawn_worker() -> (mpsc::Sender<Genome>, mpsc::Receiver<(Eval, u64)>) {
    let (gtx, grx) = mpsc::channel::<Genome>();
    let (etx, erx) = mpsc::channel::<(Eval, u64)>();
    std::thread::spawn(move || {
        while let Ok(g) = grx.recv() {
            let t = Instant::now();
            let e = classify(&g);
            let ms = t.elapsed().as_millis() as u64;
            if etx.send((e, ms)).is_err() {
                break;
            }
        }
    });
    (gtx, erx)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let evals: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20000);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let outdir = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "evolve-out".to_string());
    std::fs::create_dir_all(&outdir).unwrap();
    // Quiet the panic firehose; the genome replays any panic on demand.
    std::panic::set_hook(Box::new(|_| {}));

    let mut rng = Lcg(0x2545_F491_4F6C_DD1D ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let pop_size = 256usize;
    let mut pop: Vec<Genome> = (0..pop_size).map(|_| rand_genome(&mut rng)).collect();

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Findings stream to disk immediately (flushed per line), so partial
    // results are readable mid-run and survive an interrupt.
    let mut ffile = std::fs::File::create(Path::new(&outdir).join("failures.jsonl")).unwrap();
    let mut nfile = std::fs::File::create(Path::new(&outdir).join("novelties.jsonl")).unwrap();
    // Perf cliffs: a config that RETURNS but took unusually long. Distinct
    // from a failure (the result is correct, just slow) and from a hang
    // (which never returns).
    let mut sfile = std::fs::File::create(Path::new(&outdir).join("slow.jsonl")).unwrap();
    // Declines stream (the bulk: ~97% of evals). BUFFERED, not flushed per
    // line: the volume makes per-line syscalls the bottleneck, so it flushes
    // as the buffer fills and on the explicit flush at the end / process exit.
    let mut dfile = std::io::BufWriter::new(
        std::fs::File::create(Path::new(&outdir).join("declines.jsonl")).unwrap(),
    );
    let slow_ms = 250u64;
    let (mut n_pass, mut n_decline, mut n_fail, mut n_skip, mut n_novel, mut n_slow) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut done = 0usize;
    // Only a config that does not return within this generous budget is a
    // true HANG (a 2-primitive boolean normally finishes in milliseconds;
    // even the slow declines return under ~1s). Slow-but-finite configs go
    // to slow.jsonl, not failures.
    let eval_timeout = Duration::from_secs(20);
    let (mut wtx, mut wrx) = spawn_worker();
    let mut leaked = 0usize;

    'outer: loop {
        // Evaluate the population; keep elites by novelty + risk.
        let mut scored: Vec<(f64, Genome)> = Vec::with_capacity(pop.len());
        for g in &pop {
            if done >= evals {
                break 'outer;
            }
            done += 1;
            // Evaluate on the worker with a timeout; a hang is a failure.
            let (e, ms) = if wtx.send(*g).is_ok() {
                match wrx.recv_timeout(eval_timeout) {
                    Ok(pair) => pair,
                    Err(_) => {
                        // Abandon the stuck worker and replace it. Each
                        // leak is a thread still chewing the bad config,
                        // so cap them: many hangs is a systemic finding.
                        leaked += 1;
                        let (ntx, nrx) = spawn_worker();
                        wtx = ntx;
                        wrx = nrx;
                        (
                            Eval {
                                class: Class::Fail("hang".to_string()),
                                signature: format!(
                                    "FAIL:hang {} {}/{}",
                                    ["U", "I", "D"][g.op % 3],
                                    g.p[0].shape.tag(),
                                    g.p[1].shape.tag()
                                ),
                                risk: 1.0,
                            },
                            eval_timeout.as_millis() as u64,
                        )
                    }
                }
            } else {
                let (ntx, nrx) = spawn_worker();
                wtx = ntx;
                wrx = nrx;
                continue;
            };
            // Perf-cliff stream: a config that returned but was slow.
            if ms >= slow_ms && !matches!(e.class, Class::Fail(_) | Class::Skip) {
                n_slow += 1;
                let _ = writeln!(
                    sfile,
                    "{{\"ms\":{ms},\"sig\":\"{}\",\"genome\":{}}}",
                    e.signature,
                    genome_json(g)
                );
                let _ = sfile.flush();
            }
            match &e.class {
                Class::Pass => n_pass += 1,
                Class::Decline(reason) => {
                    n_decline += 1;
                    let _ = writeln!(
                        dfile,
                        "{{\"reason\":\"{reason}\",\"sig\":\"{}\",\"genome\":{}}}",
                        e.signature,
                        genome_json(g)
                    );
                }
                Class::Skip => n_skip += 1,
                Class::Fail(reason) => {
                    n_fail += 1;
                    let _ = writeln!(
                        ffile,
                        "{{\"reason\":\"{reason}\",\"sig\":\"{}\",\"genome\":{}}}",
                        e.signature,
                        genome_json(g)
                    );
                    let _ = ffile.flush();
                }
            }
            if leaked >= 16 {
                eprintln!("  stopping: {leaked} hangs (systemic); abandoning the search");
                break 'outer;
            }
            let novel = !seen.contains(&e.signature);
            if novel {
                seen.insert(e.signature.clone());
                if e.class != Class::Skip {
                    n_novel += 1;
                    let _ = writeln!(
                        nfile,
                        "{{\"sig\":\"{}\",\"genome\":{}}}",
                        e.signature,
                        genome_json(g)
                    );
                    let _ = nfile.flush();
                }
            }
            // Selection fitness: novelty dominates, near-failure risk and
            // declines (frontier) are bonuses, plain passes are dull.
            let fit = if matches!(e.class, Class::Fail(_)) {
                10.0
            } else {
                (if novel { 3.0 } else { 0.0 })
                    + e.risk
                    + if matches!(e.class, Class::Decline(_)) {
                        0.3
                    } else {
                        0.0
                    }
            };
            scored.push((fit, *g));
        }
        // Next generation: top-half by fitness as parents, mutated; refill
        // a quarter with fresh randoms to keep exploring.
        scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
        let keep = pop_size / 2;
        let mut next: Vec<Genome> = Vec::with_capacity(pop_size);
        for i in 0..pop_size {
            if i < pop_size / 4 {
                next.push(rand_genome(&mut rng));
            } else {
                let parent = scored[rng.pick(keep.max(1)).min(scored.len() - 1)].1;
                next.push(mutate(&parent, &mut rng));
            }
        }
        pop = next;
    }

    let _ = ffile.flush();
    let _ = nfile.flush();
    let _ = sfile.flush();
    let _ = dfile.flush();
    // Restore the default hook before printing the summary.
    let _ = std::panic::take_hook();
    println!(
        "evolve: {done} evals  PASS {n_pass} / DECLINE {n_decline} / FAIL {n_fail} / SKIP {n_skip}"
    );
    println!(
        "  distinct behaviors {} ({n_novel} recorded), failures {n_fail} (of which {leaked} hangs), slow {n_slow}",
        seen.len()
    );
    println!("  wrote {outdir}/{{failures,declines,novelties,slow}}.jsonl");
}
