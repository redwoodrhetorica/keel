//! Is the two-dimple sphere-sphere difference (committed/old band logic) a
//! WRONG-POSITIVE? core(R1) - s1(z=1.5,R1) - s2(z=-1.5,R1). True volume =
//! core - 2 lenses = 4pi/3 - 2*0.36 = 3.469.
//! Usage: cargo run --release -p keel-topo --example probe_twodimple
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean_multi};

fn zsphere(c: Vec3, r: f64) -> Body {
    // Match the boolean test's z_sphere: pole along +X (tilted frame).
    let mut b = Body::new();
    let frame = Frame3 {
        origin: c,
        x: Vec3::new(0., 1., 0.),
        y: Vec3::new(0., 0., 1.),
        z: Vec3::new(1., 0., 0.),
    };
    b.sphere(frame, r).unwrap();
    b
}

fn main() {
    let pi = std::f64::consts::PI;
    let core_v = 4.0 * pi / 3.0;
    // lens of two unit spheres at distance d=1.5: pi(4R+d)(2R-d)^2/12.
    let lens = pi * (4.0 + 1.5) * (2.0 - 1.5_f64).powi(2) / 12.0;
    let truth = core_v - 2.0 * lens;
    println!("core={core_v:.4} lens={lens:.4} truth(2 dimples)={truth:.4}");
    let core = zsphere(Vec3::ZERO, 1.0);
    let s1 = zsphere(Vec3::new(0., 0., 1.5), 1.0);
    let s2 = zsphere(Vec3::new(0., 0., -1.5), 1.0);
    match boolean_multi(&core, &[&s1, &s2], BoolOp::Difference, 1e-7) {
        Err(e) => println!("DECLINED {e:?}"),
        Ok(r) => {
            let mass = r.body.mass_properties().map(|m| m.volume);
            let mesh = r.body.mesh_volume();
            let tess = r.body.tessellated_volume();
            println!(
                "mass={mass:?} mesh={mesh:.4} tess={tess:.4} valid={} faults={:?}",
                r.body.validate().is_ok(),
                r.faults
            );
            if let Ok(m) = mass {
                println!("  mass vs truth {:.2}%", (m - truth) / truth * 100.0);
            }
        }
    }
}
