//! Realistic nested CAD sessions with DELIBERATE feature placement: each part
//! is one evolving body, features laid out on a jittered grid (one per cell, so
//! they are distinct, non-overlapping, and not exactly coplanar -- the way a
//! real model is built), then taken through dozens of sequential booleans. The
//! random-placement session (cad_session) manufactured collisions; this one
//! does not, so PASS% is the honest "can the kernel sustain a real multi-feature
//! part" capability. Reports the gate-escape count (INVALID) separately --
//! must be 0.
//! Run: cargo run --release -p keel-topo --example cad_session_real -- [parts] [seed]
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

/// A feature centred (with jitter) in a grid cell of half-size `s` at (cx, cy),
/// sized to fit within the cell so it never overlaps a neighbour, on a plate of
/// thickness h.
fn feature(rng: &mut Rng, cx: f64, cy: f64, s: f64, h: f64) -> (&'static str, Body, BoolOp) {
    let z = Vec3::new(0., 0., 1.);
    let jit = (s * 0.25).max(0.0);
    let (x, y) = (cx + rng.r(-jit, jit), cy + rng.r(-jit, jit));
    let rmax = (s * 0.6).max(0.3);
    let table: &[(u32, u8)] = &[
        (22, 0), // through-hole
        (12, 1), // blind hole
        (12, 2), // pocket
        (12, 3), // cyl boss
        (8, 4),  // block boss
        (8, 5),  // counterbore (two coaxial diffs in one feature step is 2 ops; here single)
        (8, 6),  // countersink (cone)
        (6, 7),  // dome
        (6, 8),  // ball cavity
        (6, 9),  // cross-hole (perp)
    ];
    let tot: u32 = table.iter().map(|x| x.0).sum();
    let mut pk = rng.pick(tot);
    let mut t = 0u8;
    for &(w, id) in table {
        if pk < w {
            t = id;
            break;
        }
        pk -= w;
    }
    match t {
        0 => {
            let r = rng.r(0.3, rmax);
            ("through-hole", cyl(Vec3::new(x, y, -0.5), z, r, h + 1.0), BoolOp::Difference)
        }
        1 => {
            let r = rng.r(0.3, rmax);
            let dep = rng.r(0.3, h * 0.7);
            ("blind-hole", cyl(Vec3::new(x, y, h - dep), z, r, dep + 0.6), BoolOp::Difference)
        }
        2 => {
            let (pw, pd) = (rng.r(0.6, s), rng.r(0.6, s));
            let dep = rng.r(0.3, h * 0.7);
            ("pocket", blk(Vec3::new(x - pw / 2.0, y - pd / 2.0, h - dep), pw, pd, dep + 0.6), BoolOp::Difference)
        }
        3 => {
            let r = rng.r(0.3, rmax);
            ("boss-cyl", cyl(Vec3::new(x, y, h - 0.02), z, r, rng.r(0.6, 2.5)), BoolOp::Union)
        }
        4 => {
            let (bw, bd) = (rng.r(0.6, s), rng.r(0.6, s));
            ("boss-block", blk(Vec3::new(x - bw / 2.0, y - bd / 2.0, h - 0.02), bw, bd, rng.r(0.6, 2.5)), BoolOp::Union)
        }
        5 => {
            // counterbore proxy: a larger shallow hole (single op here)
            let r = rng.r(0.5, rmax);
            let dep = rng.r(0.3, h * 0.5);
            ("counterbore", cyl(Vec3::new(x, y, h - dep), z, r, dep + 0.6), BoolOp::Difference)
        }
        6 => {
            let r = rng.r(0.5, rmax);
            let dep = rng.r(0.4, h * 0.6);
            ("countersink", cone(Vec3::new(x, y, h + 0.05), Vec3::new(0., 0., -1.), r, dep + 0.05), BoolOp::Difference)
        }
        7 => {
            let r = rng.r(0.5, rmax);
            ("dome", sph(Vec3::new(x, y, h), r), BoolOp::Union)
        }
        8 => {
            let r = rng.r(0.5, rmax.min(h));
            ("ball-cavity", sph(Vec3::new(x, y, h - r * 0.4), r), BoolOp::Difference)
        }
        _ => {
            let r = rng.r(0.25, (rmax).min(h / 2.5));
            let zc = rng.r(r + 0.2, h - r - 0.2).max(r + 0.2);
            ("cross-hole", cyl(Vec3::new(x - s - 0.5, y, zc), Vec3::new(1., 0., 0.), r, 2.0 * s + 1.0), BoolOp::Difference)
        }
    }
}

fn main() {
    let parts: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(400);
    let seed: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut rng = Rng(seed.wrapping_mul(0x9e3779b97f4a7c15) | 1);
    let mut pass: HashMap<&str, u64> = HashMap::new();
    let mut decl: HashMap<&str, u64> = HashMap::new();
    let mut tot: HashMap<&str, u64> = HashMap::new();
    let (mut ops, mut p_ok, mut p_dec, mut invalid, mut unmassable) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut dump: Option<String> = None;
    let mut full = 0u64;
    let mut depth_sum = 0f64;
    for part_i in 0..parts {
        let cell = 4.5; // half-cells of ~2.25; features sized to fit
        let s = cell / 2.0;
        let (nx, ny) = (3 + rng.pick(4) as usize, 3 + rng.pick(4) as usize); // 3..6 each -> 9..36 cells
        let (w, d, h) = (nx as f64 * cell, ny as f64 * cell, rng.r(2.0, 4.0));
        let mut body = blk(Vec3::ZERO, w, d, h);
        let mut landed = 0u32;
        let mut planned = 0u32;
        let mut broke = false;
        for ix in 0..nx {
            for iy in 0..ny {
                let (cx, cy) = ((ix as f64 + 0.5) * cell, (iy as f64 + 0.5) * cell);
                let (label, tool, op) = feature(&mut rng, cx, cy, s, h);
                planned += 1;
                *tot.entry(label).or_insert(0) += 1;
                ops += 1;
                match boolean(&body, &tool, op, 1e-7) {
                    Err(_) => {
                        *decl.entry(label).or_insert(0) += 1;
                        p_dec += 1;
                        broke = true;
                    }
                    Ok(r) if !r.faults.is_empty() => {
                        *decl.entry(label).or_insert(0) += 1;
                        p_dec += 1;
                        broke = true;
                    }
                    Ok(r) => match r.body.validate() {
                        Err(e) => {
                            invalid += 1;
                            broke = true;
                            if dump.is_none() {
                                dump = Some(format!("part {part_i} ({label} {op:?}): validate ERR {e:?}"));
                            }
                        }
                        Ok(()) => {
                            let massable = matches!(r.body.mass_properties(), Ok(m) if m.volume > 0.0);
                            if !massable {
                                unmassable += 1;
                            } else {
                                *pass.entry(label).or_insert(0) += 1;
                            }
                            p_ok += if massable { 1 } else { 0 };
                            body = r.body;
                            landed += 1;
                        }
                    },
                }
            }
        }
        depth_sum += landed as f64 / planned.max(1) as f64;
        if !broke {
            full += 1;
        }
    }
    let mut labels: Vec<&str> = tot.keys().copied().collect();
    labels.sort();
    println!("=== REALISTIC NESTED CAD (deliberate placement; {parts} parts, seed {seed}) ===");
    println!("{:<14} {:>7} {:>7} {:>9}", "feature", "ops", "PASS%", "DECLINE%");
    for l in &labels {
        let (t, p, dc) = (tot[l], *pass.get(l).unwrap_or(&0), *decl.get(l).unwrap_or(&0));
        println!("{:<14} {:>7} {:>6.1} {:>8.1}", l, t, 100.0 * p as f64 / t as f64, 100.0 * dc as f64 / t as f64);
    }
    println!("---");
    println!(
        "PER-OP: {ops} ops -> PASS {p_ok} ({:.1}%)  DECLINE {p_dec} ({:.1}%)  | UNMASSABLE(valid,NURBS) {unmassable}  INVALID(gate escape) {invalid}",
        100.0 * p_ok as f64 / ops as f64,
        100.0 * p_dec as f64 / ops as f64
    );
    let landed_or_valid = p_ok + unmassable;
    println!(
        "  counting valid-but-unmassable as success: {:.1}% of ops produced a VALID body",
        100.0 * landed_or_valid as f64 / ops as f64
    );
    println!(
        "PER-PART: {full}/{parts} ({:.1}%) completed EVERY feature clean; mean fraction of features landed = {:.1}%",
        100.0 * full as f64 / parts as f64,
        100.0 * depth_sum / parts as f64
    );
    if let Some(d) = &dump {
        println!("FIRST INVALID: {d}");
    } else {
        println!("no INVALID (gate escapes): every clean body validated OK");
    }
}
