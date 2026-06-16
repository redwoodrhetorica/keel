//! Reproduce a soak cyl/sph UnassemblableSeam decline (the dominant remaining
//! class). Usage: cargo run --release -p keel-topo --example probe_csu
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn main() {
    let mut a = Body::new();
    a.cylinder(
        Frame3::from_z(
            Vec3::new(1.3064377801479816, 0.5832084997504405, -0.6807861100727424),
            Vec3::new(0.2744426133693232, -0.9798475776504711, -0.41869765869072206),
        )
        .unwrap(),
        1.2460690422398706,
        1.7964461360635742,
    )
    .unwrap();
    let mut b = Body::new();
    b.sphere(
        Frame3::from_z(
            Vec3::new(-1.1069488854252911, -1.157117455924471, 1.0352584893476449),
            Vec3::new(-0.7462293633441708, 0.31372210979799786, 0.764759764049276),
        )
        .unwrap(),
        2.4409545110458803,
    )
    .unwrap();
    for (lbl, op) in [
        ("I cyl/sph", BoolOp::Intersection),
        ("D cyl/sph", BoolOp::Difference),
        ("U cyl/sph", BoolOp::Union),
    ] {
        match boolean(&a, &b, op, 1e-7) {
            Err(e) => println!("{lbl}: DECLINED {e:?}"),
            Ok(r) => {
                let m = r.body.mass_properties().map(|x| x.volume).unwrap_or(f64::NAN);
                println!(
                    "{lbl}: mass={m:.4} mesh={:.4} valid={} faults={:?}",
                    r.body.mesh_volume(),
                    r.body.validate().is_ok(),
                    r.faults
                );
            }
        }
    }
}
