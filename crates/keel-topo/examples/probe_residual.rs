//! Rigorous correctness check of the dominant residual nested "declines": are
//! the gate-PASSING bodies that carry a soft fault (Coincident, or the
//! "unlocated seam component (non-planar multi-cut face)" AssemblyFailed)
//! actually CORRECT? Each top-7 bug_extract signature is a block +/- primitive
//! sequence, so its final body has an INDEPENDENT Monte-Carlo truth (analytic
//! point membership, no kernel involvement). If mass == mesh == MC-truth across
//! all of them despite the fault, the fault is a false alarm on a verified body
//! and clearing it on gate-pass converts a spurious decline into a pass without
//! any risk to DECLINE-never-WRONG (the body is byte-identical either way).
//! Run: cargo run --release -p keel-topo --example probe_residual
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

#[derive(Clone, Copy)]
enum Prim {
    Blk { o: Vec3, d: Vec3 },
    Sph { c: Vec3, r: f64 },
    Cyl { p: Vec3, ax: Vec3, r: f64, h: f64 },
    Cone { p: Vec3, ax: Vec3, r: f64, h: f64 },
}

impl Prim {
    fn body(&self) -> Body {
        let mut b = Body::new();
        match *self {
            Prim::Blk { o, d } => b.block(o, d.x, d.y, d.z).unwrap(),
            Prim::Sph { c, r } => b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r).unwrap(),
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
            Prim::Sph { c, r } => (q - c).norm() <= r,
            Prim::Cyl { p, ax, r, h } => {
                let t = (q - p).dot(ax);
                if t < 0. || t > h {
                    return false;
                }
                (q - p - ax * t).norm() <= r
            }
            Prim::Cone { p, ax, r, h } => {
                let t = (q - p).dot(ax);
                if t < 0. || t > h {
                    return false;
                }
                (q - p - ax * t).norm() <= r * (1. - t / h)
            }
        }
    }
}

/// A stock primitive followed by an ordered list of (tool, op) features.
struct Seq {
    label: &'static str,
    stock: Prim,
    feats: Vec<(Prim, BoolOp)>,
}

fn inside_seq(s: &Seq, q: Vec3) -> bool {
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

fn mc_truth(s: &Seq) -> f64 {
    // Bounding box from the stock block, padded for any poke-out dome/boss.
    let Prim::Blk { o, d } = s.stock else { panic!("stock must be a block") };
    let lo = o - Vec3::new(0.2, 0.2, 0.2);
    let hi = o + d + Vec3::new(0.2, 0.2, 3.0);
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
        if inside_seq(s, p) {
            hit += 1;
        }
    }
    vb * hit as f64 / n as f64
}

fn run(s: &Seq) {
    let mut body = s.stock.body();
    let mut last_faults = String::from("[]");
    let mut ok_all = true;
    for (i, (t, op)) in s.feats.iter().enumerate() {
        match boolean(&body, &t.body(), *op, 1e-7) {
            Err(e) => {
                println!("{:<22} op {i}: HARD-DECLINE {e:?}", s.label);
                ok_all = false;
                break;
            }
            Ok(r) => {
                last_faults = format!("{:?}", r.faults);
                body = r.body;
            }
        }
    }
    if !ok_all {
        return;
    }
    let truth = mc_truth(s);
    let valid = body.validate().is_ok();
    let m = body.mass_properties().map(|x| x.volume).unwrap_or(f64::NAN);
    let mesh = body.mesh_volume();
    let err = (m - truth).abs() / (1.0 + truth);
    let verdict = if valid && err < 2e-2 { "CORRECT" } else { "** SUSPECT **" };
    println!(
        "{:<22} {verdict}  valid={valid} mass={m:.3} mesh={mesh:.3} MC={truth:.3} m-MC={err:.4}  faults={last_faults}",
        s.label
    );
}

fn main() {
    let z = Vec3::new(0., 0., 1.);
    let zd = Vec3::new(0., 0., -1.);
    let x = Vec3::new(1., 0., 0.);
    let seqs = vec![
        Seq {
            label: "#1 Coincident hole2",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(18., 22.5, 2.562) },
            feats: vec![
                (Prim::Sph { c: Vec3::new(1.914, 2.292, 2.562), r: 0.935 }, BoolOp::Union),
                (Prim::Cyl { p: Vec3::new(2.724, 6.862, -0.5), ax: z, r: 1.089, h: 3.562 }, BoolOp::Difference),
                (Prim::Cyl { p: Vec3::new(2.618, 11.029, -0.5), ax: z, r: 0.691, h: 3.562 }, BoolOp::Difference),
            ],
        },
        Seq {
            label: "#2 boss-cyl Union",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(13.5, 18., 2.707) },
            feats: vec![
                (Prim::Sph { c: Vec3::new(2.496, 2.532, 2.243), r: 1.159 }, BoolOp::Difference),
                (Prim::Cyl { p: Vec3::new(2.514, 7.068, 2.687), ax: z, r: 1.193, h: 0.653 }, BoolOp::Union),
            ],
        },
        Seq {
            label: "#3 blind-hole Diff",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 18., 2.712) },
            feats: vec![
                (Prim::Cyl { p: Vec3::new(-0.817, 2.202, 1.170), ax: x, r: 0.445, h: 5.5 }, BoolOp::Difference),
                (Prim::Cyl { p: Vec3::new(2.278, 6.502, 1.079), ax: z, r: 0.461, h: 2.233 }, BoolOp::Difference),
            ],
        },
        Seq {
            label: "#4 countersink cone",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 13.5, 2.982) },
            feats: vec![(Prim::Cone { p: Vec3::new(2.731, 2.159, 3.032), ax: zd, r: 0.601, h: 1.428 }, BoolOp::Difference)],
        },
        Seq {
            label: "#5 cross-hole Diff",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 18., 2.025) },
            feats: vec![
                (Prim::Cyl { p: Vec3::new(1.899, 2.046, 1.372), ax: z, r: 0.523, h: 1.254 }, BoolOp::Difference),
                (Prim::Cyl { p: Vec3::new(-0.106, 6.467, 0.918), ax: x, r: 0.570, h: 5.5 }, BoolOp::Difference),
            ],
        },
        Seq {
            label: "#6 pocket Diff",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 18., 3.851) },
            feats: vec![
                (Prim::Sph { c: Vec3::new(1.774, 1.888, 3.851), r: 0.845 }, BoolOp::Union),
                (Prim::Cyl { p: Vec3::new(2.784, 11.183, 1.867), ax: z, r: 1.281, h: 2.584 }, BoolOp::Difference),
                (Prim::Blk { o: Vec3::new(1.564, 15.128, 2.580), d: Vec3::new(0.798, 1.502, 1.871) }, BoolOp::Difference),
            ],
        },
        Seq {
            label: "#7 boss-block Union",
            stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 22.5, 3.043) },
            feats: vec![
                (Prim::Cyl { p: Vec3::new(2.112, 2.377, 2.390), ax: z, r: 0.588, h: 1.253 }, BoolOp::Difference),
                (Prim::Cyl { p: Vec3::new(2.197, 6.628, 3.023), ax: z, r: 1.031, h: 1.855 }, BoolOp::Union),
                (Prim::Blk { o: Vec3::new(1.760, 10.496, 3.023), d: Vec3::new(2.062, 2.030, 2.485) }, BoolOp::Union),
            ],
        },
    ];
    for s in &seqs {
        run(s);
    }
}
