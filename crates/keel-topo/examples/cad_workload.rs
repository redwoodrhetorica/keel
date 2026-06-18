//! REALISTIC CAD workload: a large batch of PLAUSIBLE, VALID operations a user
//! would actually request (drilled holes, pockets, bosses, tubes, counterbores,
//! countersinks, ball cavities, cross-holes), with sensible sizes / positions /
//! orientations so every op genuinely interacts. Unlike the adversarial
//! novelty-search explorer, here a DECLINE is an unambiguous capability gap
//! (there is no ill-posed garbage to "correctly refuse"), so PASS% is the
//! real-world capability rate and FAIL must be 0.
//! Run: cargo run --release -p keel-topo --example cad_workload -- [N] [seed]
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
}

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}
fn cyl(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, axis).unwrap(), r, h)
        .unwrap();
    b
}
fn cone(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap();
    b
}
fn sph(c: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r)
        .unwrap();
    b
}

/// (template label, stock, tool, op). Each is a guaranteed-interacting,
/// sensibly-proportioned feature. Weighted ~ real mechanical-CAD frequency.
fn make(rng: &mut Rng) -> (&'static str, Body, Body, BoolOp) {
    // Weighted template pick.
    let table: &[(u32, u8)] = &[
        (18, 0), // through-hole
        (10, 1), // blind hole
        (12, 2), // pocket
        (7, 3),  // slot
        (7, 4),  // corner notch
        (8, 5),  // cylindrical boss
        (6, 6),  // block boss / pad
        (8, 7),  // tube / bore (coaxial)
        (5, 8),  // shaft collar (coaxial)
        (5, 9),  // countersink (block - cone)
        (3, 10), // rounded end (cyl + hemisphere)
        (3, 11), // dome (block + sphere)
        (3, 12), // ball cavity (block - sphere)
        (5, 13), // cross-hole (perpendicular cyl/cyl)
    ];
    let tot: u32 = table.iter().map(|x| x.0).sum();
    let mut pick = (rng.f() * tot as f64) as u32;
    let mut t = 0u8;
    for &(w, id) in table {
        if pick < w {
            t = id;
            break;
        }
        pick -= w;
    }
    let z = Vec3::new(0., 0., 1.);
    match t {
        0 => {
            // through-hole: block - cylinder, axis +z, inside a face, all the way
            let (w, d, h) = (rng.r(4., 10.), rng.r(4., 10.), rng.r(1., 4.));
            let r = rng.r(0.3, w.min(d) / 3.0);
            let (x, y) = (rng.r(r + 0.3, w - r - 0.3), rng.r(r + 0.3, d - r - 0.3));
            (
                "through-hole",
                blk(Vec3::ZERO, w, d, h),
                cyl(Vec3::new(x, y, -0.5), z, r, h + 1.0),
                BoolOp::Difference,
            )
        }
        1 => {
            let (w, d, h) = (rng.r(4., 10.), rng.r(4., 10.), rng.r(2., 4.));
            let r = rng.r(0.3, w.min(d) / 3.0);
            let (x, y) = (rng.r(r + 0.3, w - r - 0.3), rng.r(r + 0.3, d - r - 0.3));
            let depth = rng.r(0.5, h * 0.8);
            (
                "blind-hole",
                blk(Vec3::ZERO, w, d, h),
                cyl(Vec3::new(x, y, h - depth), z, r, depth + 0.6),
                BoolOp::Difference,
            )
        }
        2 => {
            let (w, d, h) = (rng.r(5., 10.), rng.r(5., 10.), rng.r(2., 4.));
            let (pw, pd) = (rng.r(1., w * 0.6), rng.r(1., d * 0.6));
            let depth = rng.r(0.5, h * 0.8);
            let (px, py) = (rng.r(0.3, w - pw - 0.3), rng.r(0.3, d - pd - 0.3));
            (
                "pocket",
                blk(Vec3::ZERO, w, d, h),
                blk(Vec3::new(px, py, h - depth), pw, pd, depth + 0.6),
                BoolOp::Difference,
            )
        }
        3 => {
            // slot: a narrow through-cut across the block
            let (w, d, h) = (rng.r(5., 10.), rng.r(5., 10.), rng.r(1., 3.));
            let sw = rng.r(0.6, 2.0);
            let px = rng.r(0.3, w - sw - 0.3);
            (
                "slot",
                blk(Vec3::ZERO, w, d, h),
                blk(Vec3::new(px, -0.5, -0.5), sw, d + 1.0, h + 1.0),
                BoolOp::Difference,
            )
        }
        4 => {
            let (w, d, h) = (rng.r(4., 9.), rng.r(4., 9.), rng.r(2., 5.));
            let (cw, cd, ch) = (rng.r(1., w * 0.5), rng.r(1., d * 0.5), rng.r(1., h * 0.9));
            (
                "corner-notch",
                blk(Vec3::ZERO, w, d, h),
                blk(
                    Vec3::new(w - cw, d - cd, h - ch),
                    cw + 0.5,
                    cd + 0.5,
                    ch + 0.5,
                ),
                BoolOp::Difference,
            )
        }
        5 => {
            let (w, d, h) = (rng.r(4., 9.), rng.r(4., 9.), rng.r(1., 3.));
            let r = rng.r(0.5, w.min(d) / 3.0);
            let (x, y) = (rng.r(r + 0.3, w - r - 0.3), rng.r(r + 0.3, d - r - 0.3));
            (
                "boss-cyl",
                blk(Vec3::ZERO, w, d, h),
                cyl(Vec3::new(x, y, h - 0.02), z, r, rng.r(0.8, 3.0)),
                BoolOp::Union,
            )
        }
        6 => {
            let (w, d, h) = (rng.r(5., 10.), rng.r(5., 10.), rng.r(1., 3.));
            let (bw, bd) = (rng.r(1., w * 0.5), rng.r(1., d * 0.5));
            let (x, y) = (rng.r(0.3, w - bw - 0.3), rng.r(0.3, d - bd - 0.3));
            (
                "boss-block",
                blk(Vec3::ZERO, w, d, h),
                blk(Vec3::new(x, y, h - 0.02), bw, bd, rng.r(0.8, 3.0)),
                BoolOp::Union,
            )
        }
        7 => {
            let (rr, h) = (rng.r(1.5, 4.), rng.r(2., 8.));
            let r = rng.r(0.4, rr * 0.8);
            (
                "tube-bore",
                cyl(Vec3::ZERO, z, rr, h),
                cyl(Vec3::new(0., 0., -0.5), z, r, h + 1.0),
                BoolOp::Difference,
            )
        }
        8 => {
            let (rr, h) = (rng.r(0.6, 2.5), rng.r(3., 8.));
            let cr = rng.r(rr + 0.4, rr + 2.5);
            let cz = rng.r(0.5, h - 1.5);
            (
                "collar",
                cyl(Vec3::ZERO, z, rr, h),
                cyl(Vec3::new(0., 0., cz), z, cr, rng.r(0.5, 1.5)),
                BoolOp::Union,
            )
        }
        9 => {
            // countersink: a conical recess from the top face (centered, meets
            // only the top plane -> a circle, the well-supported plane/cone case)
            let (w, d, h) = (rng.r(5., 10.), rng.r(5., 10.), rng.r(2., 4.));
            let r = rng.r(1., w.min(d) / 3.0);
            let (x, y) = (w / 2.0, d / 2.0);
            let depth = rng.r(0.6, h * 0.7);
            // cone apex down: base radius r at top, narrowing into the block
            (
                "countersink",
                blk(Vec3::ZERO, w, d, h),
                cone(
                    Vec3::new(x, y, h + 0.05),
                    Vec3::new(0., 0., -1.),
                    r,
                    depth + 0.05,
                ),
                BoolOp::Difference,
            )
        }
        10 => {
            let (rr, h) = (rng.r(1., 3.), rng.r(2., 6.));
            (
                "rounded-end",
                cyl(Vec3::ZERO, z, rr, h),
                sph(Vec3::new(0., 0., h), rr),
                BoolOp::Union,
            )
        }
        11 => {
            let (w, d, h) = (rng.r(5., 10.), rng.r(5., 10.), rng.r(1., 3.));
            let r = rng.r(1., w.min(d) / 3.0);
            (
                "dome",
                blk(Vec3::ZERO, w, d, h),
                sph(Vec3::new(w / 2.0, d / 2.0, h), r),
                BoolOp::Union,
            )
        }
        12 => {
            let (w, d, h) = (rng.r(5., 10.), rng.r(5., 10.), rng.r(2., 4.));
            let r = rng.r(1., (w.min(d) / 3.0).min(h));
            (
                "ball-cavity",
                blk(Vec3::ZERO, w, d, h),
                sph(Vec3::new(w / 2.0, d / 2.0, h - r * 0.4), r),
                BoolOp::Difference,
            )
        }
        _ => {
            // cross-hole: a perpendicular hole bored through a shaft
            let (rr, h) = (rng.r(1.5, 4.), rng.r(3., 8.));
            let r = rng.r(0.4, rr * 0.7);
            let cz = rng.r(rr, h - rr);
            (
                "cross-hole",
                cyl(Vec3::ZERO, z, rr, h),
                cyl(
                    Vec3::new(-rr - 0.5, 0., cz),
                    Vec3::new(1., 0., 0.),
                    r,
                    2.0 * rr + 1.0,
                ),
                BoolOp::Difference,
            )
        }
    }
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(30000);
    let seed: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut rng = Rng(seed.wrapping_mul(0x9e3779b97f4a7c15) | 1);
    let mut pass: HashMap<&str, u64> = HashMap::new();
    let mut decl: HashMap<&str, u64> = HashMap::new();
    let mut fail: HashMap<&str, u64> = HashMap::new();
    let mut tot: HashMap<&str, u64> = HashMap::new();
    for _ in 0..n {
        let (label, a, b, op) = make(&mut rng);
        *tot.entry(label).or_insert(0) += 1;
        match boolean(&a, &b, op, 1e-7) {
            Err(_) => *decl.entry(label).or_insert(0) += 1,
            Ok(r) if !r.faults.is_empty() => *decl.entry(label).or_insert(0) += 1,
            Ok(r) => {
                let valid = r.body.validate().is_ok();
                let m = r.body.mass_properties().map(|x| x.volume);
                let mesh = r.body.mesh_volume();
                match m {
                    Ok(mv) if valid && (mv - mesh).abs() <= 2e-2 * (1.0 + mv) && mv > 0.0 => {
                        *pass.entry(label).or_insert(0) += 1
                    }
                    _ => *fail.entry(label).or_insert(0) += 1, // clean-looking but inconsistent: a silent-wrong candidate
                }
            }
        }
    }
    let (mut tp, mut td, mut tf) = (0u64, 0u64, 0u64);
    let mut labels: Vec<&str> = tot.keys().copied().collect();
    labels.sort();
    println!("=== REALISTIC CAD WORKLOAD ({n} ops, seed {seed}) ===");
    println!(
        "{:<16} {:>6} {:>7} {:>8} {:>6}",
        "template", "ops", "PASS%", "DECLINE%", "FAIL"
    );
    for l in &labels {
        let (t, p, d, f) = (
            tot[l],
            *pass.get(l).unwrap_or(&0),
            *decl.get(l).unwrap_or(&0),
            *fail.get(l).unwrap_or(&0),
        );
        tp += p;
        td += d;
        tf += f;
        println!(
            "{:<16} {:>6} {:>6.1} {:>7.1} {:>6}",
            l,
            t,
            100.0 * p as f64 / t as f64,
            100.0 * d as f64 / t as f64,
            f
        );
    }
    let nt = n as f64;
    println!("---");
    println!(
        "OVERALL: PASS {tp} ({:.1}%)  DECLINE {td} ({:.1}%)  FAIL {tf} ({:.2}%)",
        100.0 * tp as f64 / nt,
        100.0 * td as f64 / nt,
        100.0 * tf as f64 / nt
    );
}
