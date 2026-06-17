//! Boolean-level check of the cone-face SSI increment: once a countersink (cone)
//! exists in the body, does a 2nd feature crossing/near its cone lateral now
//! work (or at least not IntersectionFailed)? Cases from the bug_extract cone
//! residual. Independent MC truth confirms never-wrong.
//! Run: cargo run --release -p keel-topo --example probe_conecyl
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

#[derive(Clone, Copy)]
enum Prim {
    Blk { o: Vec3, d: Vec3 },
    Cyl { p: Vec3, ax: Vec3, r: f64, h: f64 },
    Cone { p: Vec3, ax: Vec3, r: f64, h: f64 },
}
impl Prim {
    fn body(&self) -> Body {
        let mut b = Body::new();
        match *self {
            Prim::Blk { o, d } => b.block(o, d.x, d.y, d.z).unwrap(),
            Prim::Cyl { p, ax, r, h } => b.cylinder(Frame3::from_z(p, ax).unwrap(), r, h).unwrap(),
            Prim::Cone { p, ax, r, h } => b.cone(Frame3::from_z(p, ax).unwrap(), r, h).unwrap(),
        };
        b
    }
    fn contains(&self, q: Vec3) -> bool {
        match *self {
            Prim::Blk { o, d } => {
                q.x >= o.x && q.x <= o.x + d.x && q.y >= o.y && q.y <= o.y + d.y && q.z >= o.z && q.z <= o.z + d.z
            }
            Prim::Cyl { p, ax, r, h } => {
                let t = (q - p).dot(ax);
                t >= 0. && t <= h && (q - p - ax * t).norm() <= r
            }
            Prim::Cone { p, ax, r, h } => {
                let t = (q - p).dot(ax);
                t >= 0. && t <= h && (q - p - ax * t).norm() <= r * (1. - t / h)
            }
        }
    }
}

struct Seq {
    label: &'static str,
    stock: Prim,
    feats: Vec<(Prim, BoolOp)>,
}

fn inside(s: &Seq, q: Vec3) -> bool {
    let mut m = s.stock.contains(q);
    for (t, op) in &s.feats {
        let ti = t.contains(q);
        m = match op {
            BoolOp::Union => m || ti,
            BoolOp::Difference => m && !ti,
            BoolOp::Intersection => m && ti,
        };
    }
    m
}

fn mc(s: &Seq) -> f64 {
    let Prim::Blk { o, d } = s.stock else { panic!() };
    let lo = o - Vec3::new(0.3, 0.3, 0.3);
    let hi = o + d + Vec3::new(0.3, 0.3, 0.3);
    let vb = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let mut st = 0x2545_f491_4f6c_dd1du64;
    let mut nx = || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let n = 12_000_000usize;
    let mut hit = 0usize;
    for _ in 0..n {
        let p = Vec3::new(lo.x + (hi.x - lo.x) * nx(), lo.y + (hi.y - lo.y) * nx(), lo.z + (hi.z - lo.z) * nx());
        if inside(s, p) {
            hit += 1;
        }
    }
    vb * hit as f64 / n as f64
}

fn run(s: &Seq) {
    let mut body = s.stock.body();
    for (i, (t, op)) in s.feats.iter().enumerate() {
        match boolean(&body, &t.body(), *op, 1e-7) {
            Err(e) => {
                println!("{:<26} op {i}: HARD-DECLINE {e:?}", s.label);
                return;
            }
            Ok(r) => {
                if !r.faults.is_empty() {
                    println!("{:<26} op {i}: faults {:?}", s.label, r.faults);
                    return;
                }
                body = r.body;
            }
        }
    }
    let truth = mc(s);
    let m = body.mass_properties().map(|x| x.volume).unwrap_or(f64::NAN);
    let mesh = body.mesh_volume();
    let err = (m - truth).abs() / (1.0 + truth);
    let v = if body.validate().is_ok() && err < 2e-2 { "PASS-CORRECT" } else { "** CHECK **" };
    println!("{:<26} {v}  mass={m:.3} mesh={mesh:.3} MC={truth:.3} m-MC={err:.4}", s.label);
}

fn main() {
    let zd = Vec3::new(0., 0., -1.);
    let z = Vec3::new(0., 0., 1.);
    let seqs = vec![
        // #2: countersink then a far through-hole (cone x cyl, non-intersecting).
        Seq {
            label: "#2 countersink+farhole",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 13.5, 2.982) },
            feats: vec![
                (Prim::Cone { p: Vec3::new(2.731, 2.159, 3.032), ax: zd, r: 0.601, h: 1.428 }, BoolOp::Difference),
                (Prim::Cyl { p: Vec3::new(1.742, 6.870, -0.5), ax: z, r: 1.082, h: 3.982 }, BoolOp::Difference),
            ],
        },
        // #1: countersink then a far 2nd countersink (cone x cone, non-intersecting).
        Seq {
            label: "#1 countersink+farcone",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 13.5, 2.982) },
            feats: vec![
                (Prim::Cone { p: Vec3::new(2.731, 2.159, 3.032), ax: zd, r: 0.601, h: 1.428 }, BoolOp::Difference),
                (Prim::Cone { p: Vec3::new(7.158, 2.208, 3.032), ax: zd, r: 1.089, h: 1.482 }, BoolOp::Difference),
            ],
        },
        // Deliberate OVERLAP: a wide countersink, then a vertical hole that
        // clips its cone wall (cone x cyl, intersecting -> real seam curve).
        Seq {
            label: "C overlap cone+hole",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(12.0, 12.0, 4.0) },
            feats: vec![
                (Prim::Cone { p: Vec3::new(6., 6., 4.2), ax: zd, r: 2.5, h: 4.0 }, BoolOp::Difference),
                (Prim::Cyl { p: Vec3::new(7.5, 6., -0.5), ax: z, r: 0.8, h: 5.0 }, BoolOp::Difference),
            ],
        },
    ];
    for s in &seqs {
        run(s);
    }
}
