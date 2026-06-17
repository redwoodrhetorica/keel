//! The dominant post-cone residual: AssemblyFailed("mass != mesh") on bodies
//! with curved cavities (ball-cavity + countersink, dome). Is the FAILING body
//! CORRECT (analytic mass == MC truth, only the chordal MESH is off -> a
//! tessellation gate gap) or genuinely inconsistent? Build each bug_extract
//! repro up to the failing op, run it with KEEL_BOOL_DEBUG=1 to see the gate's
//! tess/mass/mesh/open_ratio, and compare mass to an independent MC truth.
//! Run: KEEL_BOOL_DEBUG=1 cargo run --release -p keel-topo --example probe_mm
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
    Sph { c: Vec3, r: f64 },
}
impl Prim {
    fn body(&self) -> Body {
        let mut b = Body::new();
        match *self {
            Prim::Blk { o, d } => b.block(o, d.x, d.y, d.z).unwrap(),
            Prim::Cyl { p, ax, r, h } => b.cylinder(Frame3::from_z(p, ax).unwrap(), r, h).unwrap(),
            Prim::Cone { p, ax, r, h } => b.cone(Frame3::from_z(p, ax).unwrap(), r, h).unwrap(),
            Prim::Sph { c, r } => b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r).unwrap(),
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
            Prim::Sph { c, r } => (q - c).norm() <= r,
        }
    }
}

struct Seq {
    label: &'static str,
    stock: Prim,
    feats: Vec<(Prim, BoolOp)>,
}

fn mc(s: &Seq) -> f64 {
    let Prim::Blk { o, d } = s.stock else { panic!() };
    let lo = o - Vec3::new(0.3, 0.3, 0.3);
    let hi = o + d + Vec3::new(0.3, 0.3, 1.5);
    let vb = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let mut st = 0x2545_f491_4f6c_dd1du64;
    let mut nx = || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let n = 14_000_000usize;
    let mut hit = 0usize;
    for _ in 0..n {
        let p = Vec3::new(lo.x + (hi.x - lo.x) * nx(), lo.y + (hi.y - lo.y) * nx(), lo.z + (hi.z - lo.z) * nx());
        let mut m = s.stock.contains(p);
        for (t, op) in &s.feats {
            let ti = t.contains(p);
            m = match op {
                BoolOp::Union => m || ti,
                BoolOp::Difference => m && !ti,
                BoolOp::Intersection => m && ti,
            };
        }
        if m {
            hit += 1;
        }
    }
    vb * hit as f64 / n as f64
}

fn run(s: &Seq) {
    eprintln!("\n===== {} =====", s.label);
    let mut body = s.stock.body();
    let n = s.feats.len();
    for (i, (t, op)) in s.feats.iter().enumerate() {
        match boolean(&body, &t.body(), *op, 1e-7) {
            Err(e) => {
                println!("{:<22} op {i}/{n}: DECLINED {e:?}  (MC truth of full seq = {:.3})", s.label, mc(s));
                return;
            }
            Ok(r) => {
                let mok = r.body.mass_properties().is_ok();
                eprintln!("  step {i}: faults={} mass_ok={mok}", r.faults.len());
                if i + 1 == n {
                    let m = r.body.mass_properties().map(|x| x.volume).unwrap_or(f64::NAN);
                    println!(
                        "{:<22} OK faults={:?} valid={} mass={:.3} mesh={:.3} MC={:.3}",
                        s.label,
                        r.faults,
                        r.body.validate().is_ok(),
                        m,
                        r.body.mesh_volume(),
                        mc(s)
                    );
                }
                body = r.body;
            }
        }
    }
}

fn main() {
    let z = Vec3::new(0., 0., 1.);
    let zd = Vec3::new(0., 0., -1.);
    // bug_extract #1 (forward+backward / degenerate-UV): block - ball - hole -
    // cone, then blind-hole. Per-step trace finds WHICH op corrupts the sphere
    // face's loop into the degenerate fwd+bwd rim.
    run(&Seq {
        label: "#1 blind(ball+hole+cone)",
        stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 13.5, 3.923) },
        feats: vec![
            (Prim::Sph { c: Vec3::new(2.244, 2.120, 3.434), r: 1.221 }, BoolOp::Difference),
            (Prim::Cyl { p: Vec3::new(2.555, 6.236, -0.5), ax: z, r: 1.189, h: 4.923 }, BoolOp::Difference),
            (Prim::Cone { p: Vec3::new(2.540, 11.469, 3.973), ax: zd, r: 1.010, h: 1.479 }, BoolOp::Difference),
            (Prim::Cyl { p: Vec3::new(6.793, 11.515, 2.928), ax: z, r: 0.703, h: 1.595 }, BoolOp::Difference),
        ],
    });
    // #4: block + dome - through-hole, then boss-cyl union FAILS mass!=mesh.
    run(&Seq {
        label: "#4 boss after dome+hole",
        stock: Prim::Blk { o: Vec3::ZERO, d: Vec3::new(22.5, 22.5, 3.408) },
        feats: vec![
            (Prim::Sph { c: Vec3::new(2.469, 6.636, 3.408), r: 1.332 }, BoolOp::Union),
            (Prim::Cyl { p: Vec3::new(2.441, 20.581, -0.5), ax: z, r: 0.853, h: 4.408 }, BoolOp::Difference),
            (Prim::Cyl { p: Vec3::new(6.519, 1.743, 3.388), ax: z, r: 0.926, h: 0.826 }, BoolOp::Union),
        ],
    });
}
