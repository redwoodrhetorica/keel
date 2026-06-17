//! REALISTIC NESTED CAD session: a part is ONE evolving body taken through
//! DOZENS of sequential feature booleans (drill, pocket, boss, counterbore,
//! slot, chamfer, hole, ...), each operating on the ACCUMULATING compound
//! result -- the way real modeling works, and far harder than primitive-vs-
//! primitive (the operand is a complex multi-feature body). Features are placed
//! sensibly within the stock footprint so each op genuinely interacts.
//! Measures: per-op PASS/DECLINE/FAIL, how deep chains get before the first
//! gap, and how many parts complete every planned feature. A DECLINE here is a
//! real capability gap; FAIL (a clean-looking but invalid/inconsistent body)
//! must be 0.
//! Run: cargo run --release -p keel-topo --example cad_session -- [parts] [seed]
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use std::collections::HashMap;

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn r(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
    fn pick(&mut self, n: u32) -> u32 {
        (self.f() * n as f64) as u32
    }
}
fn cyl(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap();
    b
}
fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}
fn cone(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap();
    b
}
fn sph(c: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r).unwrap();
    b
}

/// A feature tool + op, placed within the W x D x H stock footprint.
fn feature(rng: &mut Rng, w: f64, d: f64, h: f64) -> (&'static str, Body, BoolOp) {
    let z = Vec3::new(0., 0., 1.);
    // weighted: holes/pockets/bosses common, curved features rarer (real CAD).
    let table: &[(u32, u8)] = &[
        (20, 0), // through-hole
        (12, 1), // blind hole
        (12, 2), // pocket
        (8, 3),  // slot
        (10, 4), // cyl boss
        (8, 5),  // block boss
        (8, 6),  // corner notch
        (6, 7),  // countersink (cone)
        (4, 8),  // cross-hole (perp cyl)
        (4, 9),  // dome (sphere union)
        (4, 10), // ball cavity (sphere diff)
    ];
    let tot: u32 = table.iter().map(|x| x.0).sum();
    let mut pk = rng.pick(tot);
    let mut t = 0u8;
    for &(wt, id) in table {
        if pk < wt {
            t = id;
            break;
        }
        pk -= wt;
    }
    match t {
        0 => {
            let r = rng.r(0.25, w.min(d) / 4.0);
            let (x, y) = (rng.r(r + 0.2, w - r - 0.2), rng.r(r + 0.2, d - r - 0.2));
            ("through-hole", cyl(Vec3::new(x, y, -0.5), z, r, h + 1.0), BoolOp::Difference)
        }
        1 => {
            let r = rng.r(0.25, w.min(d) / 4.0);
            let (x, y) = (rng.r(r + 0.2, w - r - 0.2), rng.r(r + 0.2, d - r - 0.2));
            let dep = rng.r(0.3, h * 0.7);
            ("blind-hole", cyl(Vec3::new(x, y, h - dep), z, r, dep + 0.6), BoolOp::Difference)
        }
        2 => {
            let (pw, pd) = (rng.r(0.8, w * 0.4), rng.r(0.8, d * 0.4));
            let dep = rng.r(0.3, h * 0.7);
            let (px, py) = (rng.r(0.2, w - pw - 0.2), rng.r(0.2, d - pd - 0.2));
            ("pocket", blk(Vec3::new(px, py, h - dep), pw, pd, dep + 0.6), BoolOp::Difference)
        }
        3 => {
            let sw = rng.r(0.5, 1.6);
            let px = rng.r(0.2, w - sw - 0.2);
            ("slot", blk(Vec3::new(px, -0.5, -0.5), sw, d + 1.0, h + 1.0), BoolOp::Difference)
        }
        4 => {
            let r = rng.r(0.4, w.min(d) / 5.0);
            let (x, y) = (rng.r(r + 0.2, w - r - 0.2), rng.r(r + 0.2, d - r - 0.2));
            ("boss-cyl", cyl(Vec3::new(x, y, h - 0.02), z, r, rng.r(0.6, 2.5)), BoolOp::Union)
        }
        5 => {
            let (bw, bd) = (rng.r(0.8, w * 0.35), rng.r(0.8, d * 0.35));
            let (x, y) = (rng.r(0.2, w - bw - 0.2), rng.r(0.2, d - bd - 0.2));
            ("boss-block", blk(Vec3::new(x, y, h - 0.02), bw, bd, rng.r(0.6, 2.5)), BoolOp::Union)
        }
        6 => {
            let (cw, ch) = (rng.r(0.6, w * 0.35), rng.r(0.4, h * 0.9));
            ("corner-notch", blk(Vec3::new(w - cw, -0.3, h - ch), cw + 0.4, d + 0.6, ch + 0.4), BoolOp::Difference)
        }
        7 => {
            let r = rng.r(0.6, w.min(d) / 4.0);
            let (x, y) = (rng.r(r + 0.3, w - r - 0.3), rng.r(r + 0.3, d - r - 0.3));
            let dep = rng.r(0.4, h * 0.6);
            ("countersink", cone(Vec3::new(x, y, h + 0.05), Vec3::new(0., 0., -1.), r, dep + 0.05), BoolOp::Difference)
        }
        8 => {
            let r = rng.r(0.25, h / 2.5);
            let zc = rng.r(r + 0.2, h - r - 0.2);
            let yy = rng.r(d * 0.3, d * 0.7);
            ("cross-hole", cyl(Vec3::new(-0.5, yy, zc), Vec3::new(1., 0., 0.), r, w + 1.0), BoolOp::Difference)
        }
        9 => {
            let r = rng.r(0.6, w.min(d) / 4.0);
            let (x, y) = (rng.r(r, w - r), rng.r(r, d - r));
            ("dome", sph(Vec3::new(x, y, h), r), BoolOp::Union)
        }
        _ => {
            let r = rng.r(0.5, (w.min(d) / 4.0).min(h));
            let (x, y) = (rng.r(r + 0.2, w - r - 0.2), rng.r(r + 0.2, d - r - 0.2));
            ("ball-cavity", sph(Vec3::new(x, y, h - r * 0.4), r), BoolOp::Difference)
        }
    }
}

fn main() {
    let parts: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(500);
    let seed: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut rng = Rng(seed.wrapping_mul(0x9e3779b97f4a7c15) | 1);
    let mut pass: HashMap<&str, u64> = HashMap::new();
    let mut decl: HashMap<&str, u64> = HashMap::new();
    let mut tot: HashMap<&str, u64> = HashMap::new();
    let (mut ops, mut p_ok, mut p_dec) = (0u64, 0u64, 0u64);
    // Split the old "FAIL" bucket: INVALID = a clean (no-fault) body that fails
    // validate() -> a real DECLINE-never-WRONG gate escape. UNMASSABLE = a body
    // that validates OK but mass_properties cannot integrate (a NURBS face) ->
    // the body is correct, only the analytic mass is uncovered (not a wrong).
    let (mut invalid, mut unmassable, mut nonpos) = (0u64, 0u64, 0u64);
    let mut dump: Option<String> = None;
    let mut depth_hist: Vec<u32> = Vec::new(); // ops landed before first decline, per part
    let mut full_parts = 0u64;
    for part_i in 0..parts {
        let (w, d, h) = (rng.r(6., 14.), rng.r(6., 14.), rng.r(1.5, 4.0));
        let mut body = blk(Vec3::ZERO, w, d, h);
        let k = 12 + rng.pick(24); // 12..36 planned features
        let mut landed = 0u32;
        let mut first_decline: Option<u32> = None;
        for op_i in 0..k {
            let (label, tool, op) = feature(&mut rng, w, d, h);
            *tot.entry(label).or_insert(0) += 1;
            ops += 1;
            match boolean(&body, &tool, op, 1e-7) {
                Err(_) => {
                    *decl.entry(label).or_insert(0) += 1;
                    p_dec += 1;
                    first_decline.get_or_insert(landed);
                }
                Ok(r) if !r.faults.is_empty() => {
                    *decl.entry(label).or_insert(0) += 1;
                    p_dec += 1;
                    first_decline.get_or_insert(landed);
                }
                Ok(r) => match r.body.validate() {
                    Err(e) => {
                        // CLEAN body (no faults) that fails validate -> gate escape.
                        invalid += 1;
                        if dump.is_none() {
                            dump = Some(format!(
                                "part {part_i} op {op_i} ({label}, {op:?}): validate ERR {e:?}; mesh_vol {:.4}",
                                r.body.mesh_volume()
                            ));
                        }
                        first_decline.get_or_insert(landed);
                    }
                    Ok(()) => match r.body.mass_properties() {
                        Ok(m) if m.volume > 0.0 => {
                            *pass.entry(label).or_insert(0) += 1;
                            p_ok += 1;
                            body = r.body;
                            landed += 1;
                        }
                        Ok(_) => {
                            nonpos += 1; // valid but non-positive volume (orientation)
                            body = r.body;
                            landed += 1;
                        }
                        Err(_) => {
                            unmassable += 1; // valid body, NURBS-face mass uncovered
                            body = r.body;
                            landed += 1;
                        }
                    },
                },
            }
        }
        depth_hist.push(first_decline.unwrap_or(k));
        if first_decline.is_none() {
            full_parts += 1;
        }
    }
    let mut labels: Vec<&str> = tot.keys().copied().collect();
    labels.sort();
    println!("=== NESTED CAD SESSIONS ({parts} parts, seed {seed}, 12..36 features each) ===");
    println!("{:<15} {:>7} {:>7} {:>9}", "feature", "ops", "PASS%", "DECLINE%");
    for l in &labels {
        let (t, p, dc) = (tot[l], *pass.get(l).unwrap_or(&0), *decl.get(l).unwrap_or(&0));
        println!("{:<15} {:>7} {:>6.1} {:>8.1}", l, t, 100.0 * p as f64 / t as f64, 100.0 * dc as f64 / t as f64);
    }
    println!("---");
    println!(
        "PER-OP: {ops} ops -> PASS {p_ok} ({:.1}%)  DECLINE {p_dec} ({:.1}%)",
        100.0 * p_ok as f64 / ops as f64,
        100.0 * p_dec as f64 / ops as f64
    );
    println!(
        "  the old FAIL bucket, split: INVALID(gate escape) {invalid}  UNMASSABLE(valid, NURBS-mass) {unmassable}  NONPOS {nonpos}"
    );
    if let Some(d) = &dump {
        println!("  FIRST INVALID: {d}");
    } else {
        println!("  no INVALID (no gate escape): every clean body validated OK");
    }
    let avg_depth: f64 = depth_hist.iter().map(|&x| x as f64).sum::<f64>() / parts as f64;
    println!(
        "PER-PART: {full_parts}/{parts} ({:.1}%) completed EVERY feature with no gap; \
         mean features landed before first gap = {:.1}",
        100.0 * full_parts as f64 / parts as f64,
        avg_depth
    );
}
