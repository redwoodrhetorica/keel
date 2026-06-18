//! Root-cause B: is a `mass != mesh` curved-compound decline a CORRECT body
//! rejected by a too-tight self-consistency check (coarse chordal mesh), or a
//! genuinely inconsistent result? Build a curved-compound body, run the failing
//! op, and compare mass / mesh / an independent Monte-Carlo truth.
//! Run: KEEL_BOOL_DEBUG=1 cargo run --release -p keel-topo --example probe_massmesh
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}
fn cyl(pos: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, Vec3::new(0., 0., 1.)).unwrap(), r, h)
        .unwrap();
    b
}
fn sph(c: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(c, Vec3::new(0., 0., 1.)).unwrap(), r)
        .unwrap();
    b
}

fn main() {
    // block + dome (union) -> curved-compound body, then drill a through-hole.
    let (w, d, h) = (10.0, 10.0, 3.0);
    let plate = blk(Vec3::ZERO, w, d, h);
    let dome = sph(Vec3::new(3.0, 3.0, h), 1.5);
    let b1 = boolean(&plate, &dome, BoolOp::Union, 1e-7).unwrap().body;
    println!(
        "after dome: valid={} mass={:?}",
        b1.body_valid(),
        b1.mass_properties().map(|m| m.volume)
    );
    let hole = cyl(Vec3::new(7.0, 7.0, -0.5), 1.0, h + 1.0);

    // Independent MC truth of (block U dome) - hole.
    let in_blk = |p: Vec3| p.x >= 0. && p.x <= w && p.y >= 0. && p.y <= d && p.z >= 0. && p.z <= h;
    let in_dome = |p: Vec3| (p - Vec3::new(3., 3., h)).norm() <= 1.5;
    let in_hole =
        |p: Vec3| ((p.x - 7.).powi(2) + (p.y - 7.).powi(2)).sqrt() <= 1.0 && p.z >= 0. && p.z <= h;
    let mut st = 0x1234_5678u64;
    let mut nx = || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        (st >> 11) as f64 / (1u64 << 53) as f64
    };
    let (lo, hi) = (Vec3::new(-0.2, -0.2, -0.2), Vec3::new(10.2, 10.2, 4.7));
    let vb = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let n = 8_000_000usize;
    let mut hit = 0usize;
    for _ in 0..n {
        let p = Vec3::new(
            lo.x + (hi.x - lo.x) * nx(),
            lo.y + (hi.y - lo.y) * nx(),
            lo.z + (hi.z - lo.z) * nx(),
        );
        if (in_blk(p) || in_dome(p)) && !in_hole(p) {
            hit += 1;
        }
    }
    let truth = vb * hit as f64 / n as f64;
    println!("MC truth (block U dome - hole) = {truth:.4}");
    match boolean(&b1, &hole, BoolOp::Difference, 1e-7) {
        Err(e) => println!("hole: DECLINED {e:?}"),
        Ok(r) => {
            let m = r.body.mass_properties().map(|x| x.volume);
            println!(
                "hole: OK faults={:?} valid={} mass={:?} mesh={:.4}",
                r.faults,
                r.body.validate().is_ok(),
                m,
                r.body.mesh_volume()
            );
        }
    }
}

trait V {
    fn body_valid(&self) -> bool;
}
impl V for Body {
    fn body_valid(&self) -> bool {
        self.validate().is_ok()
    }
}
