//! Triage a soak `declines.jsonl` into WHY each request was declined, by an
//! INDEPENDENT Monte-Carlo overlap oracle (no kernel calls -- pure analytic
//! primitives). The DECLINE bucket conflates two very different things:
//!   - CORRECT decline (the request is ill-posed: a degenerate primitive, or
//!     bodies that do not genuinely interpenetrate -> the answer is trivial or
//!     empty, and declining is not a kernel failure), versus
//!   - INADEQUACY (the bodies genuinely overlap, a complete kernel MUST produce
//!     a non-trivial result, and Keel could not) -- the real minimize-declines
//!     worklist.
//! Run: cargo run --release -p keel-topo --example decline_triage -- <declines.jsonl> [more...]
#![allow(clippy::unwrap_used)]
use keel_math::vec::Vec3;
use std::collections::HashMap;
use std::io::BufRead;

struct Prim {
    shape: u8, // 0 box, 1 cyl, 2 cone, 3 sphere
    pos: Vec3,
    a: f64,
    b: f64,
    c: f64,
    axis: Vec3,
}
fn unit(v: Vec3) -> Vec3 {
    let n = v.norm();
    if n < 1e-12 {
        Vec3::new(0., 0., 1.)
    } else {
        v * (1.0 / n)
    }
}
impl Prim {
    fn vol(&self) -> f64 {
        let pi = std::f64::consts::PI;
        match self.shape {
            0 => self.a * self.b * self.c,
            1 => pi * self.a * self.a * self.b,
            2 => pi / 3.0 * self.a * self.a * self.b,
            _ => 4.0 / 3.0 * pi * self.a * self.a * self.a,
        }
    }
    fn contains(&self, p: Vec3) -> bool {
        match self.shape {
            0 => {
                p.x >= self.pos.x
                    && p.x <= self.pos.x + self.a
                    && p.y >= self.pos.y
                    && p.y <= self.pos.y + self.b
                    && p.z >= self.pos.z
                    && p.z <= self.pos.z + self.c
            }
            1 => {
                let ax = unit(self.axis);
                let w = p - self.pos;
                let t = w.dot(ax);
                t >= 0.0 && t <= self.b && (w - ax * t).norm() <= self.a
            }
            2 => {
                let ax = unit(self.axis);
                let w = p - self.pos;
                let t = w.dot(ax);
                t >= 0.0
                    && t <= self.b
                    && (w - ax * t).norm() <= self.a * (1.0 - t / self.b).max(0.0)
            }
            _ => (p - self.pos).norm() <= self.a,
        }
    }
    /// Generous bounding box (contains the primitive).
    fn aabb(&self) -> (Vec3, Vec3) {
        let (c, r) = match self.shape {
            0 => {
                let half = Vec3::new(self.a, self.b, self.c) * 0.5;
                (self.pos + half, half.norm())
            }
            1 | 2 => {
                let ax = unit(self.axis);
                let c = self.pos + ax * (self.b * 0.5);
                (
                    c,
                    (self.a * self.a + (self.b * 0.5) * (self.b * 0.5)).sqrt(),
                )
            }
            _ => (self.pos, self.a),
        };
        (c - Vec3::new(r, r, r), c + Vec3::new(r, r, r))
    }
}

fn prim(v: &serde_json::Value) -> Prim {
    let f = |k: &str| v[k].as_f64().unwrap_or(0.0);
    let arr = |k: &str| {
        let a = &v[k];
        Vec3::new(
            a[0].as_f64().unwrap_or(0.0),
            a[1].as_f64().unwrap_or(0.0),
            a[2].as_f64().unwrap_or(0.0),
        )
    };
    let shape = match v["shape"].as_str().unwrap_or("") {
        "block" => 0,
        "cyl" => 1,
        "cone" => 2,
        _ => 3,
    };
    Prim {
        shape,
        pos: arr("pos"),
        a: f("a"),
        b: f("b"),
        c: f("c"),
        axis: arr("axis"),
    }
}

fn main() {
    let mut st = 0x9e3779b97f4a7c15u64;
    let mut rng = move || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let n_samp = 20000usize;
    let (mut degen, mut disjoint, mut nested, mut overlap, mut total) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut overlap_sig: HashMap<String, u64> = HashMap::new();
    for path in std::env::args().skip(1) {
        let file = std::fs::File::open(&path).unwrap();
        for line in std::io::BufReader::new(file).lines() {
            let Ok(line) = line else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let g = &v["genome"];
            let (pa, pb) = (prim(&g["a"]), prim(&g["b"]));
            let sig = v["sig"].as_str().unwrap_or("?").to_string();
            total += 1;
            let (va, vb) = (pa.vol(), pb.vol());
            if va < 1e-2 || vb < 1e-2 {
                degen += 1;
                continue;
            }
            // MC box = intersection of the two generous AABBs (contains A n B).
            let (la, ha) = pa.aabb();
            let (lb, hb) = pb.aabb();
            let lo = Vec3::new(la.x.max(lb.x), la.y.max(lb.y), la.z.max(lb.z));
            let hi = Vec3::new(ha.x.min(hb.x), ha.y.min(hb.y), ha.z.min(hb.z));
            if lo.x >= hi.x || lo.y >= hi.y || lo.z >= hi.z {
                disjoint += 1; // AABBs do not even overlap
                continue;
            }
            let bv = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
            let mut hits = 0usize;
            for _ in 0..n_samp {
                let p = Vec3::new(
                    lo.x + (hi.x - lo.x) * rng(),
                    lo.y + (hi.y - lo.y) * rng(),
                    lo.z + (hi.z - lo.z) * rng(),
                );
                if pa.contains(p) && pb.contains(p) {
                    hits += 1;
                }
            }
            let vi = bv * hits as f64 / n_samp as f64;
            let ratio = vi / va.min(vb);
            if ratio < 0.01 {
                disjoint += 1;
            } else if ratio > 0.99 {
                nested += 1;
            } else {
                overlap += 1;
                *overlap_sig.entry(sig).or_insert(0) += 1;
            }
        }
    }
    let pct = |x: u64| 100.0 * x as f64 / total.max(1) as f64;
    println!("=== DECLINE TRIAGE ({total} declines, MC {n_samp}/genome) ===");
    println!("CORRECT (not kernel inadequacy):");
    println!("  degenerate primitive   {degen:>7} ({:.1}%)", pct(degen));
    println!(
        "  disjoint / no overlap  {disjoint:>7} ({:.1}%)",
        pct(disjoint)
    );
    println!("  nested (trivial result){nested:>7} ({:.1}%)", pct(nested));
    println!("INADEQUACY (genuine partial overlap, the worklist):");
    println!(
        "  partial overlap        {overlap:>7} ({:.1}%)",
        pct(overlap)
    );
    println!("--- top INADEQUACY signatures ---");
    let mut sigs: Vec<_> = overlap_sig.into_iter().collect();
    sigs.sort_by(|a, b| b.1.cmp(&a.1));
    for (sig, n) in sigs.into_iter().take(20) {
        println!("  {n:>6}  {sig}");
    }
}
