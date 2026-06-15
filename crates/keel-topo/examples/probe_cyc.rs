//! Coaxial cone + cylinder booleans, exercising the new exact
//! cone_cylinder SSI rung (single seam circle where cone radius == cyl
//! radius). Cone base r=2 at z=0, apex at z=3. Coaxial cylinder r=1,
//! z in [0,3]. Seam circle: z=1.5, radius 1.
//!
//! Truths:  cone vol = 4pi, cyl vol = 3pi, inter = 2pi.
//!   cone n cyl = 2pi   cone u cyl = 5pi   cone - cyl = 2pi   cyl - cone = pi
//!
//! Usage: cargo run --release -p keel-topo --example probe_cyc
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn zf() -> Frame3 {
    Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap()
}
fn cone(r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(zf(), r, h).unwrap();
    b
}
fn cyl(r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(zf(), r, h).unwrap();
    b
}
fn cyl_at(z0: f64, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(Vec3::new(0., 0., z0), Vec3::new(0., 0., 1.)).unwrap(), r, h)
        .unwrap();
    b
}

fn run(label: &str, a: &Body, b: &Body, op: BoolOp, truth: f64) {
    print!("{label}: truth={truth:.4}  ");
    match boolean(a, b, op, 1e-7) {
        Err(e) => println!("DECLINED {e:?}"),
        Ok(r) => {
            let mesh = r.body.mesh_volume();
            let valid = r.body.validate().is_ok();
            match r.body.mass_properties() {
                Ok(m) => {
                    let pass = r.faults.is_empty()
                        && valid
                        && (m.volume - mesh).abs() < 3e-2 * (1.0 + m.volume)
                        && (m.volume - truth).abs() < 3e-2 * (1.0 + truth);
                    println!(
                        "mass={:.4} mesh={mesh:.4} valid={valid} faults={:?} -> {}",
                        m.volume,
                        r.faults,
                        if pass { "PASS" } else { "OFF" }
                    );
                }
                Err(e) => println!("mass DECLINED {e:?} mesh={mesh:.4} valid={valid}"),
            }
        }
    }
}

fn main() {
    let pi = std::f64::consts::PI;
    // Cone base r=2 at z=0, apex z=3 (slope m=-2/3). Cylinder r=1, z in [0,2]
    // (top cap clear of the apex). Seam circle at z=1.5 (cone radius==1).
    // inter = 1.5pi + integral_{1.5}^{2} pi*(2-2z/3)^2 dz = (1.5 + 28.5/81)pi.
    let inter = (1.5 + 28.5 / 81.0) * pi; // ~5.8177
    let cone_v = 4.0 * pi;
    let cyl_v = 2.0 * pi;
    let cn = cone(2.0, 3.0);
    let cl = cyl(1.0, 2.0);
    println!("== coaxial cone (r2,h3) + cylinder (r1,h2), SHARED base z=0 ==");
    run("cone n cyl", &cn, &cl, BoolOp::Intersection, inter);
    run("cone u cyl", &cn, &cl, BoolOp::Union, cone_v + cyl_v - inter);
    run("cone - cyl", &cn, &cl, BoolOp::Difference, cone_v - inter);
    run("cyl - cone", &cl, &cn, BoolOp::Difference, cyl_v - inter);

    // Offset base: cylinder z in [0.3,2], no coplanar faces -> isolates the
    // lateral seam. inter2 = 1.2pi + 28.5/81 pi. cyl vol = 1.7pi.
    let inter2 = (1.2 + 28.5 / 81.0) * pi;
    let cyl2_v = 1.7 * pi;
    let cl2 = cyl_at(0.3, 1.0, 1.7);
    println!("== coaxial cone + cylinder z in [0.3,2], OFFSET base ==");
    run("cone n cyl", &cn, &cl2, BoolOp::Intersection, inter2);
    run("cone u cyl", &cn, &cl2, BoolOp::Union, cone_v + cyl2_v - inter2);
    run("cone - cyl", &cn, &cl2, BoolOp::Difference, cone_v - inter2);
    run("cyl - cone", &cl2, &cn, BoolOp::Difference, cyl2_v - inter2);
}
